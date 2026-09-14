use std::{path::PathBuf, sync::Arc, time::Duration};

use aya_workcell::{
    RunConfig, SYNTHETIC_SOURCE, Scenario, SystemClock, WorkcellOutcome, WorkcellState,
    prepare_synthetic, process_is_alive, run_synthetic,
};
use tempfile::TempDir;

fn fake_dcc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_aya-fake-dcc"))
}

async fn run(scenario: Scenario) -> (TempDir, aya_workcell::RunReport) {
    let root = TempDir::new().unwrap();
    let (score, lease) = prepare_synthetic(root.path()).await.unwrap();
    let report = run_synthetic(RunConfig {
        root: root.path().to_path_buf(),
        fake_dcc: fake_dcc(),
        score,
        lease,
        scenario,
        request_timeout: Duration::from_millis(150),
        clock: Arc::new(SystemClock),
    })
    .await
    .unwrap();
    (root, report)
}

fn assert_valid_receipt(report: &aya_workcell::RunReport) {
    assert!(
        aya_contracts::verify_receipt_chain(report.receipt["events"].as_array().unwrap())
            .is_empty()
    );
    assert!(aya_contracts::verify_receipt_digest(&report.receipt).unwrap());
    aya_contracts::validate_document(&report.receipt).unwrap();
    let on_disk: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report.receipt_path).unwrap()).unwrap();
    assert_eq!(on_disk, report.receipt);
}

#[tokio::test]
async fn synthetic_workcell_gate() {
    autonomous_multi_iteration_run().await;
    crash_once_is_recovered().await;
    orphan_is_reaped(Scenario::Orphan).await;
    orphan_is_reaped(Scenario::DetachedOrphan).await;
    adversarial_failures_are_accounted_for().await;
    symlink_destination_is_rejected().await;
    symlink_mount_is_rejected_before_admission().await;
    every_score_input_is_verified_and_receipted().await;
    stronger_isolation_requests_fail_closed().await;
    global_deadline_expires_the_workcell().await;
}

async fn autonomous_multi_iteration_run() {
    let (root, report) = run(Scenario::Success).await;
    assert_eq!(report.outcome, WorkcellOutcome::CandidateReady);
    assert_eq!(report.attempts, 1);
    assert_eq!(
        report.states,
        [
            WorkcellState::Requested,
            WorkcellState::Admitted,
            WorkcellState::Running,
            WorkcellState::CandidateReady,
            WorkcellState::Sealed,
            WorkcellState::Expired,
        ]
    );
    assert_eq!(report.source_before_sha256, report.source_after_sha256);
    assert_eq!(
        tokio::fs::read_to_string(root.path().join("input/source.scene.json"))
            .await
            .unwrap(),
        SYNTHETIC_SOURCE
    );
    let candidate: serde_json::Value = serde_json::from_slice(
        &tokio::fs::read(root.path().join("output/candidate.scene.json"))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(candidate["camera"], "locked");
    assert_eq!(candidate["scale"], 1);
    assert_eq!(candidate["tension"], 3);
    let summary: serde_json::Value = serde_json::from_slice(
        &tokio::fs::read(root.path().join("output/scene-summary.json"))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(summary["scene"]["tension"], 3);
    assert!(report.process_tree_reaped);
    assert_valid_receipt(&report);
    assert_eq!(
        report.receipt["candidate"]["sources"][0]["beforeSha256"],
        report.receipt["candidate"]["sources"][0]["afterSha256"]
    );
    let kinds: Vec<_> = report.receipt["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.windows(2).any(|pair| pair == ["diagnose", "correct"]));
}

async fn crash_once_is_recovered() {
    let (_root, report) = run(Scenario::CrashOnce).await;
    assert_eq!(report.outcome, WorkcellOutcome::CandidateReady);
    assert_eq!(report.attempts, 2);
    assert_eq!(report.source_before_sha256, report.source_after_sha256);
    assert!(
        report.receipt["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| {
                event["kind"] == "failure"
                    && event["summary"].as_str().unwrap().contains("Attempt 1")
            })
    );
    assert_valid_receipt(&report);
}

async fn orphan_is_reaped(scenario: Scenario) {
    let (_root, report) = run(scenario).await;
    assert_eq!(report.outcome, WorkcellOutcome::CandidateReady);
    assert!(report.process_tree_reaped);
    assert_eq!(report.observed_orphan_pids.len(), 1);
    let pid = report.observed_orphan_pids[0];
    for _ in 0..50 {
        if !process_is_alive(pid) {
            assert_valid_receipt(&report);
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        !process_is_alive(pid),
        "synthetic orphan {pid} survived expiry"
    );
}

async fn adversarial_failures_are_accounted_for() {
    for scenario in [
        Scenario::Timeout,
        Scenario::SourceMutation,
        Scenario::MutationThenCrash,
        Scenario::PartialOutput,
        Scenario::PathEscape,
        Scenario::InconsistentEvidence,
        Scenario::CandidateMismatch,
        Scenario::InconsistentReceipt,
    ] {
        let (root, report) = run(scenario).await;
        assert_eq!(report.outcome, WorkcellOutcome::Failed, "{scenario:?}");
        assert_eq!(report.states.last(), Some(&WorkcellState::Expired));
        assert_eq!(report.receipt["status"], "failed");
        assert!(report.receipt["candidate"].is_null());
        assert!(report.process_tree_reaped);
        assert_valid_receipt(&report);
        assert!(!root.path().join("output/candidate.scene.json").exists());
        assert!(!root.path().join("output/candidate.scene.partial").exists());
        assert!(!root.path().join("work/attempt-1").exists());
        assert!(
            !root
                .path()
                .parent()
                .unwrap()
                .join("escaped.scene.json")
                .exists(),
            "{scenario:?} unexpectedly wrote outside the workcell"
        );
        if matches!(
            scenario,
            Scenario::SourceMutation | Scenario::MutationThenCrash
        ) {
            assert_ne!(report.source_before_sha256, report.source_after_sha256);
            assert_eq!(report.attempts, 1, "custody violation must not retry");
            assert!(
                report.receipt["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|event| event["kind"] == "violation"
                        && event["summary"].as_str().unwrap().contains("custody"))
            );
        } else {
            assert_eq!(report.source_before_sha256, report.source_after_sha256);
        }
        if scenario == Scenario::PathEscape {
            assert!(
                report.receipt["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|event| {
                        event["summary"]
                            .as_str()
                            .unwrap()
                            .contains("unexpected artifact path")
                    })
            );
        }
    }
}

async fn symlink_destination_is_rejected() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let outside_file = outside.path().join("outside.scene.json");
    std::fs::write(&outside_file, b"outside\n").unwrap();
    let (score, lease) = prepare_synthetic(root.path()).await.unwrap();
    symlink(
        &outside_file,
        root.path().join("output/candidate.scene.json"),
    )
    .unwrap();
    let report = run_synthetic(RunConfig {
        root: root.path().to_path_buf(),
        fake_dcc: fake_dcc(),
        score,
        lease,
        scenario: Scenario::Success,
        request_timeout: Duration::from_millis(150),
        clock: Arc::new(SystemClock),
    })
    .await
    .unwrap();
    assert_eq!(report.outcome, WorkcellOutcome::Failed);
    assert_eq!(std::fs::read(&outside_file).unwrap(), b"outside\n");
    assert!(!root.path().join("output/candidate.scene.json").exists());
    assert_valid_receipt(&report);
}

async fn symlink_mount_is_rejected_before_admission() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let (score, lease) = prepare_synthetic(root.path()).await.unwrap();
    std::fs::remove_dir(root.path().join("output")).unwrap();
    symlink(outside.path(), root.path().join("output")).unwrap();
    let error = run_synthetic(RunConfig {
        root: root.path().to_path_buf(),
        fake_dcc: fake_dcc(),
        score,
        lease,
        scenario: Scenario::Success,
        request_timeout: Duration::from_millis(150),
        clock: Arc::new(SystemClock),
    })
    .await
    .unwrap_err();
    assert!(error.to_string().contains("unsafe synthetic output mount"));
    assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none());
}

async fn every_score_input_is_verified_and_receipted() {
    let root = TempDir::new().unwrap();
    let (mut score, mut lease) = prepare_synthetic(root.path()).await.unwrap();
    let second = b"{\"synthetic\":\"reference\"}\n";
    tokio::fs::write(root.path().join("input/reference.json"), second)
        .await
        .unwrap();
    score["inputs"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "path": "input/reference.json",
            "sha256": aya_contracts::sha256_bytes(second)
        }));
    lease["scoreSha256"] = aya_contracts::sha256_json(&score).unwrap().into();
    let report = run_synthetic(RunConfig {
        root: root.path().to_path_buf(),
        fake_dcc: fake_dcc(),
        score,
        lease,
        scenario: Scenario::Success,
        request_timeout: Duration::from_millis(150),
        clock: Arc::new(SystemClock),
    })
    .await
    .unwrap();
    assert_eq!(report.outcome, WorkcellOutcome::CandidateReady);
    assert_eq!(
        report.receipt["candidate"]["sources"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(report.source_before_sha256, report.source_after_sha256);
    assert_valid_receipt(&report);
}

async fn stronger_isolation_requests_fail_closed() {
    let root = TempDir::new().unwrap();
    let (mut score, mut lease) = prepare_synthetic(root.path()).await.unwrap();
    score["isolationRequired"] = "process_isolated".into();
    lease["isolationRequired"] = "process_isolated".into();
    lease["scoreSha256"] = aya_contracts::sha256_json(&score).unwrap().into();
    let error = run_synthetic(RunConfig {
        root: root.path().to_path_buf(),
        fake_dcc: fake_dcc(),
        score,
        lease,
        scenario: Scenario::Success,
        request_timeout: Duration::from_millis(150),
        clock: Arc::new(SystemClock),
    })
    .await
    .unwrap_err();
    assert!(error.to_string().contains("only contract_only"));
}

async fn global_deadline_expires_the_workcell() {
    let root = TempDir::new().unwrap();
    let (mut score, mut lease) = prepare_synthetic(root.path()).await.unwrap();
    score["budget"]["wallTimeSeconds"] = 1.into();
    lease["scoreSha256"] = aya_contracts::sha256_json(&score).unwrap().into();
    let started = std::time::Instant::now();
    let report = run_synthetic(RunConfig {
        root: root.path().to_path_buf(),
        fake_dcc: fake_dcc(),
        score,
        lease,
        scenario: Scenario::Timeout,
        request_timeout: Duration::from_secs(5),
        clock: Arc::new(SystemClock),
    })
    .await
    .unwrap();
    assert_eq!(report.outcome, WorkcellOutcome::Expired);
    assert_eq!(report.receipt["status"], "expired");
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_valid_receipt(&report);
}
