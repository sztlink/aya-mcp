use std::{
    collections::HashMap,
    env,
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use aya_workcell::{Clock, Scenario, SystemClock, prepare_synthetic_with_score};
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use rmcp::{
    Json, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{Mutex, oneshot},
    time::timeout,
};
use tokio_util::sync::CancellationToken;

const NATIVE_VERSION: &str = "2026-07-28";

#[derive(Debug, Deserialize, JsonSchema)]
struct DocumentRequest {
    document: Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WorkcellRequest {
    score: Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WorkcellIdRequest {
    workcell_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ValidationResponse {
    ok: bool,
    schema: Option<String>,
    sha256: Option<String>,
    errors: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct RequestResponse {
    accepted: bool,
    workcell_id: Option<String>,
    isolation_level: &'static str,
    errors: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct StatusResponse {
    found: bool,
    workcell_id: String,
    status: String,
    final_lifecycle_state: Option<String>,
    attempts: Option<u64>,
    process_tree_reaped: Option<bool>,
    errors: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct CancelResponse {
    found: bool,
    workcell_id: String,
    cancellation_requested: bool,
    status: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ReviewResponse {
    found: bool,
    ready: bool,
    workcell_id: String,
    receipt: Option<Value>,
    promotion_available: bool,
    errors: Vec<String>,
}

struct WorkcellRecord {
    status: String,
    final_lifecycle_state: Option<String>,
    attempts: Option<u64>,
    process_tree_reaped: Option<bool>,
    receipt: Option<Value>,
    errors: Vec<String>,
    cancellation: CancellationToken,
}

struct PublicConfig {
    base: PathBuf,
    worker: PathBuf,
    scenario: Scenario,
}

impl PublicConfig {
    fn from_environment() -> Result<Self> {
        let base = env::var_os("AYA_WORKCELL_BASE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp/aya-mcp-workcells"));
        let current = env::current_exe()?;
        let worker = current
            .parent()
            .context("Public MCP executable has no parent")?
            .join("aya-worker-mcp");
        if !worker.is_file() {
            bail!("Worker MCP is unavailable at {}", worker.display());
        }
        let scenario = env::var("AYA_SYNTHETIC_SCENARIO")
            .unwrap_or_else(|_| "success".to_owned())
            .parse()?;
        Ok(Self {
            base,
            worker,
            scenario,
        })
    }
}

#[derive(Clone)]
struct PublicServer {
    tool_router: ToolRouter<Self>,
    config: Arc<PublicConfig>,
    records: Arc<Mutex<HashMap<String, WorkcellRecord>>>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    shutting_down: Arc<AtomicBool>,
    counter: Arc<AtomicU64>,
}

impl PublicServer {
    fn new(config: PublicConfig) -> Self {
        Self {
            tool_router: Self::tool_router(),
            config: Arc::new(config),
            records: Arc::new(Mutex::new(HashMap::new())),
            tasks: Arc::new(Mutex::new(Vec::new())),
            shutting_down: Arc::new(AtomicBool::new(false)),
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    fn next_id(&self) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        format!("synthetic-{nanos:x}-{counter:x}")
    }

    async fn shutdown(&self) {
        let mut task_gate = self.tasks.lock().await;
        self.shutting_down.store(true, Ordering::SeqCst);
        for record in self.records.lock().await.values_mut() {
            if record.status == "running" {
                record.status = "cancelling".to_owned();
                record.cancellation.cancel();
            }
        }
        let tasks = std::mem::take(&mut *task_gate);
        drop(task_gate);
        for task in tasks {
            let _ = task.await;
        }
    }
}

#[tool_router(router = tool_router)]
impl PublicServer {
    #[tool(
        name = "aya_score_validate",
        description = "Validate an AYA Score v0 document and return its RFC 8785 SHA-256 digest"
    )]
    async fn validate_score(
        &self,
        Parameters(request): Parameters<DocumentRequest>,
    ) -> Json<ValidationResponse> {
        let schema = request
            .document
            .get("schema")
            .and_then(Value::as_str)
            .map(str::to_owned);
        match aya_contracts::validate_score(&request.document) {
            Ok(()) => Json(ValidationResponse {
                ok: true,
                schema,
                sha256: aya_contracts::sha256_json(&request.document).ok(),
                errors: Vec::new(),
            }),
            Err(errors) => Json(ValidationResponse {
                ok: false,
                schema,
                sha256: None,
                errors,
            }),
        }
    }

    #[tool(
        name = "aya_workcell_request",
        description = "Create one contract_only synthetic workcell using the pinned Worker and fake DCC"
    )]
    async fn request_workcell(
        &self,
        Parameters(request): Parameters<WorkcellRequest>,
    ) -> Json<RequestResponse> {
        if let Err(errors) = aya_contracts::validate_score(&request.score) {
            return Json(RequestResponse {
                accepted: false,
                workcell_id: None,
                isolation_level: "contract_only",
                errors,
            });
        }
        if request.score["isolationRequired"] != "contract_only" {
            return Json(RequestResponse {
                accepted: false,
                workcell_id: None,
                isolation_level: "contract_only",
                errors: vec!["synthetic Public MCP accepts only contract_only Scores".to_owned()],
            });
        }

        let mut task_gate = self.tasks.lock().await;
        if self.shutting_down.load(Ordering::SeqCst) {
            return Json(RequestResponse {
                accepted: false,
                workcell_id: None,
                isolation_level: "contract_only",
                errors: vec!["Public MCP is shutting down".to_owned()],
            });
        }

        let workcell_id = self.next_id();
        let root = self.config.base.join(&workcell_id);
        if root.exists() {
            return Json(RequestResponse {
                accepted: false,
                workcell_id: None,
                isolation_level: "contract_only",
                errors: vec!["generated workcell path already exists".to_owned()],
            });
        }
        if let Err(error) = tokio::fs::create_dir_all(&self.config.base).await {
            return Json(RequestResponse {
                accepted: false,
                workcell_id: None,
                isolation_level: "contract_only",
                errors: vec![error.to_string()],
            });
        }
        let admitted_score = request.score;
        let admitted_lease =
            match prepare_synthetic_with_score(&root, admitted_score.clone(), &workcell_id).await {
                Ok(lease) => lease,
                Err(error) => {
                    return Json(RequestResponse {
                        accepted: false,
                        workcell_id: None,
                        isolation_level: "contract_only",
                        errors: vec![error.to_string()],
                    });
                }
            };

        let cancellation = CancellationToken::new();
        self.records.lock().await.insert(
            workcell_id.clone(),
            WorkcellRecord {
                status: "running".to_owned(),
                final_lifecycle_state: None,
                attempts: None,
                process_tree_reaped: None,
                receipt: None,
                errors: Vec::new(),
                cancellation: cancellation.clone(),
            },
        );

        let records = self.records.clone();
        let worker = self.config.worker.clone();
        let scenario = self.config.scenario;
        let task_id = workcell_id.clone();
        let request_cancellation = cancellation.clone();
        let (ready_sender, ready_receiver) = oneshot::channel();
        let (completion_sender, completion_receiver) = oneshot::channel();
        let task = tokio::spawn(async move {
            async {
                let (worker_result, worker_process_reaped) = run_worker_mcp(
                    worker,
                    root.clone(),
                    task_id.clone(),
                    scenario,
                    cancellation.clone(),
                    ready_sender,
                )
                .await;

                let cancellation_accepted = {
                    let mut records = records.lock().await;
                    let Some(record) = records.get_mut(&task_id) else {
                        return;
                    };
                    if record.status == "cancelling" {
                        record.status = "finalizing_cancel".to_owned();
                        true
                    } else {
                        record.status = "finalizing".to_owned();
                        false
                    }
                };

                if cancellation_accepted {
                    let (prior_receipt, attempts, process_tree_reaped, worker_error) =
                        match &worker_result {
                            Ok(report) => {
                                if let Err(error) = validate_worker_report(
                                    &task_id,
                                    &root,
                                    &admitted_score,
                                    &admitted_lease,
                                    report,
                                )
                                .await
                                {
                                    (None, None, false, Some(error.to_string()))
                                } else {
                                    (
                                        report.get("receipt"),
                                        report["attempts"].as_u64(),
                                        report["process_tree_reaped"].as_bool().unwrap_or(false),
                                        None,
                                    )
                                }
                            }
                            Err(error) => {
                                (None, None, worker_process_reaped, Some(error.to_string()))
                            }
                        };
                    let cancelled = write_public_cancellation(
                        &task_id,
                        &root,
                        &admitted_score,
                        &admitted_lease,
                        prior_receipt,
                    )
                    .await;
                    let mut records = records.lock().await;
                    let Some(record) = records.get_mut(&task_id) else {
                        return;
                    };
                    match cancelled {
                        Ok(receipt) => {
                            record.status = "cancelled".to_owned();
                            record.final_lifecycle_state =
                                process_tree_reaped.then(|| "expired".to_owned());
                            record.attempts = attempts;
                            record.process_tree_reaped = Some(process_tree_reaped);
                            record.receipt = Some(receipt);
                            if let Some(error) = worker_error {
                                record.errors.push(error);
                            }
                        }
                        Err(error) => {
                            record.status = "failed".to_owned();
                            record.process_tree_reaped = Some(process_tree_reaped);
                            record.errors.push(error.to_string());
                            if let Some(worker_error) = worker_error {
                                record.errors.push(worker_error);
                            }
                        }
                    }
                    return;
                }

                let mut records = records.lock().await;
                let Some(record) = records.get_mut(&task_id) else {
                    return;
                };
                match worker_result {
                    Ok(report) => {
                        if let Err(error) = validate_worker_report(
                            &task_id,
                            &root,
                            &admitted_score,
                            &admitted_lease,
                            &report,
                        )
                        .await
                        {
                            record.status = "failed".to_owned();
                            record.errors.push(error.to_string());
                            return;
                        }
                        record.status = report["outcome"].as_str().unwrap_or("failed").to_owned();
                        record.final_lifecycle_state = report["states"]
                            .as_array()
                            .and_then(|states| states.last())
                            .and_then(Value::as_str)
                            .map(str::to_owned);
                        record.attempts = report["attempts"].as_u64();
                        record.process_tree_reaped = report["process_tree_reaped"].as_bool();
                        record.receipt = report.get("receipt").cloned();
                    }
                    Err(error) => {
                        record.status = "failed".to_owned();
                        record.errors.push(error.to_string());
                    }
                }
            }
            .await;
            let _ = completion_sender.send(());
        });
        task_gate.push(task);

        let readiness = ready_receiver.await;
        match readiness {
            Ok(Ok(())) => {
                drop(task_gate);
                Json(RequestResponse {
                    accepted: true,
                    workcell_id: Some(workcell_id),
                    isolation_level: "contract_only",
                    errors: Vec::new(),
                })
            }
            failed => {
                request_cancellation.cancel();
                if let Some(record) = self.records.lock().await.get_mut(&workcell_id)
                    && record.status == "running"
                {
                    record.status = "cancelling".to_owned();
                }
                let _ = completion_receiver.await;
                drop(task_gate);
                let error = match failed {
                    Ok(Err(error)) => error,
                    Err(_) => "Worker readiness channel closed".to_owned(),
                    Ok(Ok(())) => unreachable!(),
                };
                Json(RequestResponse {
                    accepted: false,
                    workcell_id: None,
                    isolation_level: "contract_only",
                    errors: vec![error],
                })
            }
        }
    }

    #[tool(
        name = "aya_workcell_status",
        description = "Read synthetic workcell status without changing it"
    )]
    async fn workcell_status(
        &self,
        Parameters(request): Parameters<WorkcellIdRequest>,
    ) -> Json<StatusResponse> {
        let records = self.records.lock().await;
        match records.get(&request.workcell_id) {
            Some(record) => Json(StatusResponse {
                found: true,
                workcell_id: request.workcell_id,
                status: record.status.clone(),
                final_lifecycle_state: record.final_lifecycle_state.clone(),
                attempts: record.attempts,
                process_tree_reaped: record.process_tree_reaped,
                errors: record.errors.clone(),
            }),
            None => Json(StatusResponse {
                found: false,
                workcell_id: request.workcell_id,
                status: "unknown".to_owned(),
                final_lifecycle_state: None,
                attempts: None,
                process_tree_reaped: None,
                errors: vec!["workcell not found".to_owned()],
            }),
        }
    }

    #[tool(
        name = "aya_workcell_cancel",
        description = "Request cancellation of a running synthetic workcell"
    )]
    async fn cancel_workcell(
        &self,
        Parameters(request): Parameters<WorkcellIdRequest>,
    ) -> Json<CancelResponse> {
        let mut records = self.records.lock().await;
        match records.get_mut(&request.workcell_id) {
            Some(record) if record.status == "running" => {
                record.cancellation.cancel();
                record.status = "cancelling".to_owned();
                Json(CancelResponse {
                    found: true,
                    workcell_id: request.workcell_id,
                    cancellation_requested: true,
                    status: "cancelling".to_owned(),
                })
            }
            Some(record) => Json(CancelResponse {
                found: true,
                workcell_id: request.workcell_id,
                cancellation_requested: false,
                status: record.status.clone(),
            }),
            None => Json(CancelResponse {
                found: false,
                workcell_id: request.workcell_id,
                cancellation_requested: false,
                status: "unknown".to_owned(),
            }),
        }
    }

    #[tool(
        name = "aya_candidate_review",
        description = "Return candidate receipt for external review; this tool cannot promote or overwrite"
    )]
    async fn candidate_review(
        &self,
        Parameters(request): Parameters<WorkcellIdRequest>,
    ) -> Json<ReviewResponse> {
        let records = self.records.lock().await;
        match records.get(&request.workcell_id) {
            Some(record) => Json(ReviewResponse {
                found: true,
                ready: record.receipt.is_some(),
                workcell_id: request.workcell_id,
                receipt: record.receipt.clone(),
                promotion_available: false,
                errors: record.errors.clone(),
            }),
            None => Json(ReviewResponse {
                found: false,
                ready: false,
                workcell_id: request.workcell_id,
                receipt: None,
                promotion_available: false,
                errors: vec!["workcell not found".to_owned()],
            }),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for PublicServer {}

fn safe_scoped_path(
    root: &std::path::Path,
    relative: &str,
    required_prefix: &str,
) -> Result<PathBuf> {
    let path = std::path::Path::new(relative);
    if !relative.starts_with(&format!("{required_prefix}/"))
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        bail!("Worker returned unsafe {required_prefix} path: {relative}");
    }
    Ok(root.join(path))
}

async fn verify_parent_directories(root: &std::path::Path, relative: &str) -> Result<()> {
    let mut cursor = root.to_path_buf();
    let components: Vec<_> = std::path::Path::new(relative).components().collect();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        cursor.push(component.as_os_str());
        let metadata = tokio::fs::symlink_metadata(&cursor).await?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            bail!("Unsafe directory component in {relative}");
        }
    }
    Ok(())
}

async fn verified_regular_file(
    root: &std::path::Path,
    relative: &str,
    required_prefix: &str,
) -> Result<PathBuf> {
    let path = safe_scoped_path(root, relative, required_prefix)?;
    verify_parent_directories(root, relative).await?;
    let metadata = tokio::fs::symlink_metadata(&path).await?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!("Worker path is not a regular file: {relative}");
    }
    Ok(path)
}

async fn verify_file(root: &std::path::Path, file: &Value) -> Result<()> {
    let relative = file["path"].as_str().context("file path missing")?;
    let expected = file["sha256"].as_str().context("file digest missing")?;
    let path = verified_regular_file(root, relative, "output").await?;
    let actual = aya_contracts::sha256_bytes(tokio::fs::read(path).await?);
    if actual != expected {
        bail!("Worker output digest mismatch: {relative}");
    }
    Ok(())
}

async fn validate_worker_report(
    workcell_id: &str,
    root: &std::path::Path,
    score: &Value,
    lease: &Value,
    report: &Value,
) -> Result<()> {
    let outcome = report["outcome"]
        .as_str()
        .context("Worker report outcome missing")?;
    if !matches!(
        outcome,
        "candidate_ready" | "cancelled" | "expired" | "failed"
    ) {
        bail!("Worker report outcome is unsupported");
    }
    let states = report["states"]
        .as_array()
        .context("Worker report states missing")?;
    let actual_states: Vec<_> = states
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()
        .context("Worker lifecycle contains a non-string state")?;
    let expected_states: &[&str] = match outcome {
        "candidate_ready" => &[
            "requested",
            "admitted",
            "running",
            "candidate_ready",
            "sealed",
            "expired",
        ],
        "cancelled" => &["requested", "admitted", "running", "cancelled", "expired"],
        "expired" => &["requested", "admitted", "running", "expired"],
        "failed" => &["requested", "admitted", "running", "failed", "expired"],
        _ => unreachable!(),
    };
    if actual_states != expected_states {
        bail!("Worker lifecycle is inconsistent with outcome {outcome}");
    }
    if report["process_tree_reaped"] != true {
        bail!("Worker report did not account for its process tree");
    }
    let expected_sources: HashMap<_, _> = score["inputs"]
        .as_array()
        .context("Score inputs missing")?
        .iter()
        .map(|input| {
            (
                input["path"].as_str().unwrap().to_owned(),
                input["sha256"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let before: HashMap<String, String> =
        serde_json::from_value(report["source_before_sha256"].clone())?;
    let after: HashMap<String, String> =
        serde_json::from_value(report["source_after_sha256"].clone())?;
    if before != expected_sources || after != expected_sources {
        bail!("Worker report violates source custody");
    }
    for (relative, expected) in &expected_sources {
        let path = verified_regular_file(root, relative, "input").await?;
        let actual = aya_contracts::sha256_bytes(tokio::fs::read(path).await?);
        if &actual != expected {
            bail!("Actual source digest violates custody: {relative}");
        }
    }

    let receipt = report.get("receipt").context("Worker receipt missing")?;
    aya_contracts::validate_document(receipt)
        .map_err(|errors| anyhow!("Worker receipt schema errors: {}", errors.join("; ")))?;
    if receipt["receiptId"] != format!("receipt-{workcell_id}") {
        bail!("Worker receipt does not bind the workcell id");
    }
    if receipt["scoreSha256"] != aya_contracts::sha256_json(score).map_err(anyhow::Error::msg)?
        || receipt["leaseSha256"]
            != aya_contracts::sha256_json(lease).map_err(anyhow::Error::msg)?
    {
        bail!("Worker receipt does not bind admitted Score and Lease");
    }
    if receipt["status"] != outcome {
        bail!("Worker receipt status disagrees with report outcome");
    }
    let chain_errors = aya_contracts::verify_receipt_chain(
        receipt["events"]
            .as_array()
            .context("Worker receipt events missing")?,
    );
    if !chain_errors.is_empty() {
        bail!(
            "Worker receipt event chain is invalid: {}",
            chain_errors.join("; ")
        );
    }
    if !aya_contracts::verify_receipt_digest(receipt).map_err(anyhow::Error::msg)? {
        bail!("Worker receipt digest is invalid");
    }

    let receipt_path = verified_regular_file(root, "output/receipt.json", "output").await?;
    let receipt_disk: Value = serde_json::from_slice(&tokio::fs::read(receipt_path).await?)?;
    if &receipt_disk != receipt {
        bail!("Worker receipt differs from on-disk receipt");
    }

    if outcome != "candidate_ready" {
        if !receipt["candidate"].is_null() {
            bail!("Terminal non-candidate receipt contains a candidate");
        }
        for relative in [
            "output/before.image.json",
            "output/after.image.json",
            "output/scene-summary.json",
            "output/candidate.scene.json",
            "output/candidate.sha256",
        ] {
            match tokio::fs::symlink_metadata(root.join(relative)).await {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Ok(_) => bail!("Non-candidate workcell retained output: {relative}"),
                Err(error) => return Err(error.into()),
            }
        }
        return Ok(());
    }
    if !states.iter().any(|state| state == "candidate_ready") {
        bail!("Candidate report lacks candidate_ready lifecycle state");
    }
    let candidate = &receipt["candidate"];
    let sources = candidate["sources"]
        .as_array()
        .context("Candidate sources missing")?;
    if sources.len() != expected_sources.len()
        || sources.iter().any(|source| {
            let path = source["path"].as_str().unwrap_or_default();
            expected_sources.get(path).is_none_or(|digest| {
                source["beforeSha256"] != *digest || source["afterSha256"] != *digest
            })
        })
    {
        bail!("Candidate source custody does not match admitted Score");
    }
    let required: std::collections::HashSet<_> = score["evidenceRequired"]
        .as_array()
        .context("Score evidence requirements missing")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let evidence = candidate["evidence"]
        .as_array()
        .context("Candidate evidence missing")?;
    let observed: std::collections::HashSet<_> = evidence
        .iter()
        .filter_map(|item| item["kind"].as_str())
        .collect();
    if !required.is_subset(&observed) {
        bail!("Candidate evidence is incomplete");
    }
    for artifact in candidate["artifacts"]
        .as_array()
        .context("Candidate artifacts missing")?
    {
        verify_file(root, artifact).await?;
    }
    for item in evidence {
        verify_file(root, item).await?;
    }
    let candidate_path =
        verified_regular_file(root, "output/candidate.scene.json", "output").await?;
    let scene: Value = serde_json::from_slice(&tokio::fs::read(candidate_path).await?)?;
    if scene != json!({"camera": "locked", "scale": 1, "tension": 3}) {
        bail!("Candidate scene violates the pinned synthetic Score");
    }
    Ok(())
}

async fn write_public_cancellation(
    workcell_id: &str,
    root: &std::path::Path,
    score: &Value,
    lease: &Value,
    prior_receipt: Option<&Value>,
) -> Result<Value> {
    for input in score["inputs"].as_array().context("Score inputs missing")? {
        let relative = input["path"].as_str().context("Score input path missing")?;
        let expected = input["sha256"]
            .as_str()
            .context("Score input digest missing")?;
        let path = verified_regular_file(root, relative, "input").await?;
        let actual = aya_contracts::sha256_bytes(tokio::fs::read(path).await?);
        if actual != expected {
            bail!("Source custody violation during cancellation");
        }
    }

    let mut removable = vec![
        "output/before.image.json".to_owned(),
        "output/after.image.json".to_owned(),
        "output/scene-summary.json".to_owned(),
        "output/candidate.scene.json".to_owned(),
        "output/candidate.sha256".to_owned(),
    ];
    if let Some(receipt) = prior_receipt {
        for collection in ["artifacts", "evidence"] {
            if let Some(items) = receipt["candidate"][collection].as_array() {
                removable.extend(
                    items
                        .iter()
                        .filter_map(|item| item["path"].as_str())
                        .map(str::to_owned),
                );
            }
        }
    }
    removable.sort();
    removable.dedup();
    for relative in removable {
        let path = safe_scoped_path(root, &relative, "output")?;
        verify_parent_directories(root, &relative).await?;
        match tokio::fs::remove_file(path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    let at_value = serde_json::to_value(SystemClock.now())?;
    let at = at_value
        .as_str()
        .context("Clock did not serialize as text")?;
    let mut receipt = prior_receipt.cloned().unwrap_or_else(|| {
        json!({
            "schema": "aya.receipt/v0",
            "receiptId": format!("receipt-{workcell_id}"),
            "scoreSha256": aya_contracts::ZERO_SHA256,
            "leaseSha256": aya_contracts::ZERO_SHA256,
            "status": "cancelled",
            "events": [],
            "candidate": null,
            "sealedAt": at,
            "receiptSha256": aya_contracts::ZERO_SHA256
        })
    });
    receipt["receiptId"] = format!("receipt-{workcell_id}").into();
    receipt["scoreSha256"] = aya_contracts::sha256_json(score)
        .map_err(anyhow::Error::msg)?
        .into();
    receipt["leaseSha256"] = aya_contracts::sha256_json(lease)
        .map_err(anyhow::Error::msg)?
        .into();
    receipt["status"] = "cancelled".into();
    receipt["candidate"] = Value::Null;
    receipt["sealedAt"] = at.into();
    receipt["receiptSha256"] = aya_contracts::ZERO_SHA256.into();
    let mut events: Vec<Value> = serde_json::from_value(receipt["events"].clone())?;
    aya_contracts::append_receipt_event(
        &mut events,
        at,
        "failure",
        "Cancellation accepted before Public publication; candidate outputs were withdrawn",
    )
    .map_err(anyhow::Error::msg)?;
    receipt["events"] = events.into();
    aya_contracts::seal_receipt(&mut receipt).map_err(anyhow::Error::msg)?;
    aya_contracts::validate_document(&receipt)
        .map_err(|errors| anyhow!("Cancellation receipt schema errors: {}", errors.join("; ")))?;
    if !aya_contracts::verify_receipt_digest(&receipt).map_err(anyhow::Error::msg)? {
        bail!("Cancellation receipt digest is invalid");
    }

    let receipt_path = safe_scoped_path(root, "output/receipt.json", "output")?;
    verify_parent_directories(root, "output/receipt.json").await?;
    match tokio::fs::remove_file(&receipt_path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&receipt_path)
        .await?;
    file.write_all(&serde_json::to_vec_pretty(&receipt)?)
        .await?;
    file.write_all(b"\n").await?;
    file.flush().await?;
    file.sync_all().await?;
    drop(file);
    let receipt_path = verified_regular_file(root, "output/receipt.json", "output").await?;
    let disk: Value = serde_json::from_slice(&tokio::fs::read(receipt_path).await?)?;
    if disk != receipt {
        bail!("Cancellation receipt differs after disk write");
    }
    Ok(receipt)
}

#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    params: Value,
}

struct WorkerClient {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Lines<BufReader<ChildStdout>>,
    next_id: u64,
    pid: i32,
    cancellation_watcher: Option<tokio::task::JoinHandle<()>>,
}

impl WorkerClient {
    async fn spawn(
        worker: PathBuf,
        root: PathBuf,
        workcell_id: String,
        scenario: Scenario,
        cancellation: CancellationToken,
    ) -> std::result::Result<Self, (anyhow::Error, bool)> {
        if cancellation.is_cancelled() {
            return Err((anyhow!("workcell cancelled before Worker start"), true));
        }
        let mut child = Command::new(worker)
            .env("AYA_WORKCELL_ROOT", root)
            .env("AYA_WORKCELL_ID", workcell_id)
            .env("AYA_SYNTHETIC_SCENARIO", scenario.as_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| (error.into(), true))?;
        let Some(stdin) = child.stdin.take() else {
            let _ = child.start_kill();
            let reaped = child.wait().await.is_ok();
            return Err((anyhow!("Worker stdin missing"), reaped));
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.start_kill();
            let reaped = child.wait().await.is_ok();
            return Err((anyhow!("Worker stdout missing"), reaped));
        };
        let Some(pid) = child.id() else {
            let _ = child.start_kill();
            let reaped = child.wait().await.is_ok();
            return Err((anyhow!("Worker pid missing"), reaped));
        };
        let pid = pid as i32;
        Ok(Self {
            child,
            stdin: Some(stdin),
            lines: BufReader::new(stdout).lines(),
            next_id: 1,
            pid,
            cancellation_watcher: None,
        })
    }

    fn arm_cancellation(&mut self, cancellation: CancellationToken) {
        let pid = self.pid;
        self.cancellation_watcher = Some(tokio::spawn(async move {
            cancellation.cancelled().await;
            let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
        }));
    }

    fn request_meta() -> Value {
        json!({
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": NATIVE_VERSION,
                "io.modelcontextprotocol/clientInfo": {
                    "name": "aya-public-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        })
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let request = RpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };
        let stdin = self.stdin.as_mut().context("Worker stdin closed")?;
        stdin.write_all(&serde_json::to_vec(&request)?).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        let line = timeout(Duration::from_secs(20), self.lines.next_line())
            .await
            .context("Worker MCP request timed out")??
            .context("Worker MCP closed before response")?;
        if line.len() > 2_097_152 {
            bail!("Worker MCP response exceeds 2 MiB");
        }
        let response: Value = serde_json::from_str(&line)?;
        if response["jsonrpc"] != "2.0" || response["id"] != id {
            bail!("Worker MCP response does not match request {id}");
        }
        if !response["error"].is_null() {
            bail!("Worker MCP error: {}", response["error"]);
        }
        Ok(response["result"].clone())
    }

    async fn call_tool(&mut self, name: &str) -> Result<Value> {
        let mut params = Self::request_meta();
        params["name"] = name.into();
        params["arguments"] = json!({});
        let result = self.request("tools/call", params).await?;
        let structured = result["structuredContent"].clone();
        if structured["ok"] != true {
            bail!("Worker tool {name} failed: {}", structured["errors"]);
        }
        Ok(structured)
    }

    async fn close(&mut self) -> Result<bool> {
        self.stdin.take();
        let status = timeout(Duration::from_secs(2), self.child.wait())
            .await
            .context("Worker MCP did not exit after stdin closed")??;
        if let Some(watcher) = self.cancellation_watcher.take() {
            watcher.abort();
        }
        Ok(status.success())
    }
}

impl Drop for WorkerClient {
    fn drop(&mut self) {
        if let Some(watcher) = self.cancellation_watcher.take() {
            watcher.abort();
        }
        let _ = self.child.start_kill();
    }
}

async fn run_worker_mcp(
    worker: PathBuf,
    root: PathBuf,
    workcell_id: String,
    scenario: Scenario,
    cancellation: CancellationToken,
    ready_sender: oneshot::Sender<std::result::Result<(), String>>,
) -> (Result<Value>, bool) {
    let mut ready_sender = Some(ready_sender);
    let mut client = match WorkerClient::spawn(
        worker,
        root,
        workcell_id,
        scenario,
        cancellation.clone(),
    )
    .await
    {
        Ok(client) => client,
        Err((error, process_tree_reaped)) => {
            if let Some(sender) = ready_sender.take() {
                let _ = sender.send(Err(error.to_string()));
            }
            return (Err(error), process_tree_reaped);
        }
    };
    let result: Result<Value> = async {
        let discovered = client
            .request("server/discover", WorkerClient::request_meta())
            .await?;
        if !discovered["supportedVersions"]
            .as_array()
            .is_some_and(|versions| versions.iter().any(|version| version == NATIVE_VERSION))
        {
            bail!("Worker MCP does not advertise {NATIVE_VERSION}");
        }
        client.arm_cancellation(cancellation.clone());
        if let Some(sender) = ready_sender.take() {
            let _ = sender.send(Ok(()));
        }
        let listed = client
            .request("tools/list", WorkerClient::request_meta())
            .await?;
        let mut names: Vec<_> = listed["tools"]
            .as_array()
            .context("Worker tools/list malformed")?
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        names.sort_unstable();
        if names != ["aya_capability_discover", "aya_capture", "aya_execute"] {
            bail!("Worker MCP exposed unexpected tools: {names:?}");
        }
        let capabilities = client.call_tool("aya_capability_discover").await?;
        if capabilities["isolation_level"] != "contract_only" {
            bail!("Worker claimed unexpected isolation level");
        }
        let executed = client.call_tool("aya_execute").await?;
        let captured = client.call_tool("aya_capture").await?;
        if executed["report"] != captured["report"] {
            bail!("Worker capture does not match execution report");
        }
        Ok(captured["report"].clone())
    }
    .await;
    if let Some(sender) = ready_sender.take() {
        let message = result
            .as_ref()
            .err()
            .map(ToString::to_string)
            .unwrap_or_else(|| "Worker readiness failed".to_owned());
        let _ = sender.send(Err(message));
    }
    if result.is_err() {
        cancellation.cancel();
    }
    let close_result = client.close().await;
    match (result, close_result) {
        (Ok(report), Ok(true)) => (Ok(report), true),
        (Ok(_), Ok(false)) => (Err(anyhow!("Worker MCP exited unsuccessfully")), false),
        (Err(error), Ok(_)) => (Err(error), false),
        (Err(error), Err(close_error)) => (
            Err(error.context(format!("Worker cleanup also failed: {close_error}"))),
            false,
        ),
        (Ok(_), Err(error)) => (Err(error), false),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = PublicConfig::from_environment()?;
    eprintln!(
        "AYA Public MCP v{} starting contract_only synthetic supervisor on stdio",
        env!("CARGO_PKG_VERSION")
    );
    let server = PublicServer::new(config);
    let service = server.clone().serve(stdio()).await?;
    #[cfg(unix)]
    let result = {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = service.waiting() => result.map(|_| ()),
            _ = sigterm.recv() => Ok(()),
        }
    };
    #[cfg(not(unix))]
    let result = service.waiting().await;
    server.shutdown().await;
    result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn public_surface_has_no_promotion_tool() {
        let names = [
            "aya_candidate_review",
            "aya_score_validate",
            "aya_workcell_cancel",
            "aya_workcell_request",
            "aya_workcell_status",
        ];
        assert!(names.iter().all(|name| !name.contains("promote")));
        assert_eq!(names.len(), 5);
    }
}
