use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::{Arc, OnceLock},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, SecondsFormat, Utc};
use nix::{
    errno::Errno,
    sys::{
        prctl::set_child_subreaper,
        signal::{Signal, kill, killpg},
        wait::{WaitPidFlag, WaitStatus, waitpid},
    },
    unistd::Pid,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::{
    fs::OpenOptions,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::{Instant, sleep, timeout},
};
use tokio_util::sync::CancellationToken;

pub const SYNTHETIC_SOURCE: &str = "{\"camera\":\"locked\",\"scale\":1,\"tension\":0}\n";

static SUBREAPER: OnceLock<Result<(), Errno>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkcellState {
    Requested,
    Admitted,
    Running,
    CandidateReady,
    Sealed,
    Failed,
    Cancelled,
    Expired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkcellOutcome {
    CandidateReady,
    Failed,
    Cancelled,
    Expired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    Success,
    CrashOnce,
    Timeout,
    Orphan,
    DetachedOrphan,
    SourceMutation,
    MutationThenCrash,
    PartialOutput,
    PathEscape,
    InconsistentEvidence,
    CandidateMismatch,
    InconsistentReceipt,
}

impl Scenario {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::CrashOnce => "crash-once",
            Self::Timeout => "timeout",
            Self::Orphan => "orphan",
            Self::DetachedOrphan => "detached-orphan",
            Self::SourceMutation => "source-mutation",
            Self::MutationThenCrash => "mutation-then-crash",
            Self::PartialOutput => "partial-output",
            Self::PathEscape => "path-escape",
            Self::InconsistentEvidence => "inconsistent-evidence",
            Self::CandidateMismatch => "candidate-mismatch",
            Self::InconsistentReceipt => "inconsistent-receipt",
        }
    }
}

impl std::str::FromStr for Scenario {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "success" => Ok(Self::Success),
            "crash-once" => Ok(Self::CrashOnce),
            "timeout" => Ok(Self::Timeout),
            "orphan" => Ok(Self::Orphan),
            "detached-orphan" => Ok(Self::DetachedOrphan),
            "source-mutation" => Ok(Self::SourceMutation),
            "mutation-then-crash" => Ok(Self::MutationThenCrash),
            "partial-output" => Ok(Self::PartialOutput),
            "path-escape" => Ok(Self::PathEscape),
            "inconsistent-evidence" => Ok(Self::InconsistentEvidence),
            "candidate-mismatch" => Ok(Self::CandidateMismatch),
            "inconsistent-receipt" => Ok(Self::InconsistentReceipt),
            _ => bail!("unknown synthetic scenario {value}"),
        }
    }
}

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Debug)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub struct RunConfig {
    pub workcell_id: String,
    pub root: PathBuf,
    pub fake_dcc: PathBuf,
    pub score: Value,
    pub lease: Value,
    pub scenario: Scenario,
    pub request_timeout: Duration,
    pub clock: Arc<dyn Clock>,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Serialize)]
pub struct RunReport {
    pub states: Vec<WorkcellState>,
    pub outcome: WorkcellOutcome,
    pub attempts: u64,
    pub receipt: Value,
    pub receipt_path: PathBuf,
    pub source_before_sha256: HashMap<String, String>,
    pub source_after_sha256: HashMap<String, String>,
    pub process_tree_reaped: bool,
    pub observed_orphan_pids: Vec<u32>,
}

#[derive(Debug, Error)]
enum AttemptError {
    #[error("fake DCC process crashed: {0}")]
    Crash(String),
    #[error("fake DCC request timed out")]
    Timeout,
    #[error("workcell deadline expired")]
    Deadline,
    #[error("workcell cancelled")]
    Cancelled,
    #[error("fake DCC protocol error: {0}")]
    Protocol(String),
    #[error("workcell integrity violation: {0}")]
    Integrity(String),
    #[error("workcell I/O failure: {0}")]
    Io(String),
    #[error("fake DCC could not start: {0}")]
    Spawn(String),
}

impl AttemptError {
    fn retryable(&self) -> bool {
        matches!(self, Self::Crash(_) | Self::Timeout | Self::Spawn(_))
    }
}

#[derive(Debug, Serialize)]
struct DccRequest<'a> {
    protocol: &'static str,
    id: u64,
    method: &'a str,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct DccResponse {
    protocol: String,
    id: u64,
    ok: bool,
    #[serde(default)]
    result: Value,
    error: Option<String>,
}

struct DccClient {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    pgid: Pid,
    next_id: u64,
    request_timeout: Duration,
    cancellation: CancellationToken,
}

#[derive(Debug)]
struct ReapReport {
    process_group_gone: bool,
}

impl DccClient {
    async fn spawn(config: &RunConfig) -> Result<Self, AttemptError> {
        let marker = config.root.join("work/crash-once.marker");
        let mut command = Command::new(&config.fake_dcc);
        command
            .arg("--scenario")
            .arg(config.scenario.as_str())
            .arg("--marker")
            .arg(marker)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .process_group(0);
        let mut child = command
            .spawn()
            .map_err(|error| AttemptError::Spawn(error.to_string()))?;
        let id = child
            .id()
            .ok_or_else(|| AttemptError::Spawn("fake DCC has no process id".to_owned()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AttemptError::Spawn("missing DCC stdin".to_owned()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AttemptError::Spawn("missing DCC stdout".to_owned()))?;
        Ok(Self {
            child,
            stdin,
            lines: BufReader::new(stdout).lines(),
            pgid: Pid::from_raw(id as i32),
            next_id: 1,
            request_timeout: config.request_timeout,
            cancellation: config.cancellation.clone(),
        })
    }

    async fn classify_exit(&mut self, context: &str) -> AttemptError {
        match self.child.try_wait() {
            Ok(Some(status)) => AttemptError::Crash(format!("{context}; status {status}")),
            Ok(None) => AttemptError::Protocol(format!("connection closed during {context}")),
            Err(error) => AttemptError::Io(error.to_string()),
        }
    }

    async fn call(
        &mut self,
        method: &str,
        params: Value,
        deadline: Instant,
    ) -> Result<Value, AttemptError> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(AttemptError::Deadline)?;
        let call_budget = remaining.min(self.request_timeout);
        let deadline_is_limit = remaining <= self.request_timeout;
        let id = self.next_id;
        self.next_id += 1;
        let request = DccRequest {
            protocol: "aya.fake-dcc/v0",
            id,
            method,
            params,
        };
        let encoded = serde_json::to_vec(&request)
            .map_err(|error| AttemptError::Protocol(error.to_string()))?;
        if let Err(error) = self.stdin.write_all(&encoded).await {
            return Err(self
                .classify_exit(&format!("writing {method}: {error}"))
                .await);
        }
        if let Err(error) = self.stdin.write_all(b"\n").await {
            return Err(self
                .classify_exit(&format!("writing {method}: {error}"))
                .await);
        }
        if let Err(error) = self.stdin.flush().await {
            return Err(self
                .classify_exit(&format!("flushing {method}: {error}"))
                .await);
        }

        let response = tokio::select! {
            _ = self.cancellation.cancelled() => return Err(AttemptError::Cancelled),
            response = timeout(call_budget, self.lines.next_line()) => response,
        };
        let line = match response {
            Err(_) if deadline_is_limit => return Err(AttemptError::Deadline),
            Err(_) => return Err(AttemptError::Timeout),
            Ok(Err(error)) => return Err(AttemptError::Io(error.to_string())),
            Ok(Ok(None)) => return Err(self.classify_exit(method).await),
            Ok(Ok(Some(line))) => line,
        };
        if line.len() > 1_048_576 {
            return Err(AttemptError::Protocol("response exceeds 1 MiB".to_owned()));
        }
        let response: DccResponse = serde_json::from_str(&line)
            .map_err(|error| AttemptError::Protocol(format!("invalid response: {error}")))?;
        if response.protocol != "aya.fake-dcc/v0" || response.id != id {
            return Err(AttemptError::Protocol(format!(
                "response does not match request {id}"
            )));
        }
        if response.ok && response.error.is_some() {
            return Err(AttemptError::Protocol(
                "successful response contains an error".to_owned(),
            ));
        }
        if !response.ok {
            return Err(AttemptError::Protocol(
                response.error.unwrap_or_else(|| "unknown error".to_owned()),
            ));
        }
        Ok(response.result)
    }

    async fn reap(
        mut self,
        deadline: Instant,
        known_descendants: &[u32],
    ) -> Result<ReapReport, AttemptError> {
        let _ = self.call("shutdown", json!({}), deadline).await;
        drop(self.stdin);

        for pid in known_descendants {
            let _ = kill(Pid::from_raw(*pid as i32), Signal::SIGTERM);
        }
        let _ = killpg(self.pgid, Signal::SIGTERM);
        sleep(Duration::from_millis(20)).await;
        for pid in known_descendants {
            if process_is_alive(*pid) {
                let _ = kill(Pid::from_raw(*pid as i32), Signal::SIGKILL);
            }
        }
        if process_group_is_alive(self.pgid) {
            killpg(self.pgid, Signal::SIGKILL)
                .or_else(ignore_missing_process)
                .map_err(|error| AttemptError::Io(format!("killpg failed: {error}")))?;
        }

        timeout(Duration::from_secs(1), self.child.wait())
            .await
            .map_err(|_| AttemptError::Io("DCC leader did not exit".to_owned()))?
            .map_err(|error| AttemptError::Io(format!("waiting for DCC leader: {error}")))?;

        let cleanup_deadline = Instant::now() + Duration::from_secs(1);
        loop {
            drain_process_group(self.pgid)?;
            for pid in known_descendants {
                match waitpid(Pid::from_raw(*pid as i32), Some(WaitPidFlag::WNOHANG)) {
                    Ok(_) | Err(Errno::ECHILD) => {}
                    Err(error) => {
                        return Err(AttemptError::Io(format!(
                            "waiting for known descendant {pid}: {error}"
                        )));
                    }
                }
            }
            let descendants_gone = known_descendants.iter().all(|pid| !process_is_alive(*pid));
            if !process_group_is_alive(self.pgid) && descendants_gone {
                return Ok(ReapReport {
                    process_group_gone: true,
                });
            }
            if Instant::now() >= cleanup_deadline {
                return Err(AttemptError::Io(
                    "DCC process group survived cleanup".to_owned(),
                ));
            }
            let _ = killpg(self.pgid, Signal::SIGKILL);
            for pid in known_descendants {
                let _ = kill(Pid::from_raw(*pid as i32), Signal::SIGKILL);
            }
            sleep(Duration::from_millis(20)).await;
        }
    }
}

fn ignore_missing_process(error: Errno) -> Result<(), Errno> {
    if error == Errno::ESRCH {
        Ok(())
    } else {
        Err(error)
    }
}

fn process_group_is_alive(pgid: Pid) -> bool {
    kill(Pid::from_raw(-pgid.as_raw()), None).is_ok()
}

fn drain_process_group(pgid: Pid) -> Result<(), AttemptError> {
    loop {
        match waitpid(Pid::from_raw(-pgid.as_raw()), Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::StillAlive) | Err(Errno::ECHILD) => return Ok(()),
            Ok(_) => continue,
            Err(error) => {
                return Err(AttemptError::Io(format!(
                    "waitpid for process group failed: {error}"
                )));
            }
        }
    }
}

pub fn process_is_alive(pid: u32) -> bool {
    kill(Pid::from_raw(pid as i32), None).is_ok()
}

pub fn synthetic_score() -> Value {
    let source_sha256 = aya_contracts::sha256_bytes(SYNTHETIC_SOURCE.as_bytes());
    json!({
        "schema": "aya.score/v0",
        "scoreId": "synthetic-tension-001",
        "origin": {
            "author": "synthetic example",
            "gesture": "Increase tension while preserving camera and scale."
        },
        "intent": "Produce a derived synthetic scene after inspecting and correcting one weak attempt.",
        "inputs": [{"path": "input/source.scene.json", "sha256": source_sha256}],
        "invariants": ["Preserve camera", "Preserve scale", "Do not alter source"],
        "variationSpace": ["Tension may increase from zero to three"],
        "evidenceRequired": ["before_image", "after_image", "scene_summary", "artifact_hash"],
        "budget": {"wallTimeSeconds": 10, "maxAttempts": 2},
        "isolationRequired": "contract_only"
    })
}

pub async fn prepare_synthetic_with_score(
    root: &Path,
    score: Value,
    workcell_id: &str,
) -> Result<Value> {
    aya_contracts::validate_score(&score).map_err(|errors| anyhow!(errors.join("; ")))?;
    if score != synthetic_score() {
        bail!("Synthetic workcell accepts only the pinned synthetic Score");
    }
    if workcell_id.len() < 3
        || workcell_id.len() > 48
        || !workcell_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
    {
        bail!("invalid synthetic workcell id");
    }
    let expected_path = "input/source.scene.json";
    let expected_digest = aya_contracts::sha256_bytes(SYNTHETIC_SOURCE.as_bytes());
    if !score["inputs"].as_array().is_some_and(|inputs| {
        inputs
            .iter()
            .any(|input| input["path"] == expected_path && input["sha256"] == expected_digest)
    }) {
        bail!("Score does not bind the pinned synthetic source");
    }

    tokio::fs::create_dir_all(root.join("input")).await?;
    tokio::fs::create_dir_all(root.join("work")).await?;
    tokio::fs::create_dir_all(root.join("output")).await?;
    tokio::fs::write(root.join(expected_path), SYNTHETIC_SOURCE).await?;
    let now = Utc::now();
    let score_sha256 = aya_contracts::sha256_json(&score).map_err(anyhow::Error::msg)?;
    let wall_seconds = score["budget"]["wallTimeSeconds"].as_i64().unwrap_or(10);
    let lease = json!({
        "schema": "aya.lease/v0",
        "leaseId": format!("lease-{workcell_id}"),
        "scoreSha256": score_sha256,
        "workerId": "aya-synthetic-worker",
        "createdAt": (now - chrono::Duration::seconds(1)).to_rfc3339_opts(SecondsFormat::Secs, true),
        "expiresAt": (now + chrono::Duration::seconds(wall_seconds + 5)).to_rfc3339_opts(SecondsFormat::Secs, true),
        "isolationRequired": "contract_only",
        "network": "deny",
        "allowRawCode": false,
        "mounts": {"input": "input", "work": "work", "output": "output"}
    });
    tokio::fs::write(
        root.join("work/score.json"),
        serde_json::to_vec_pretty(&score)?,
    )
    .await?;
    tokio::fs::write(
        root.join("work/lease.json"),
        serde_json::to_vec_pretty(&lease)?,
    )
    .await?;
    Ok(lease)
}

pub async fn prepare_synthetic(root: &Path) -> Result<(Value, Value)> {
    let score = synthetic_score();
    let lease = prepare_synthetic_with_score(root, score.clone(), "synthetic-direct").await?;
    Ok((score, lease))
}

fn parse_time(value: &Value, field: &str) -> Result<DateTime<Utc>> {
    let text = value[field]
        .as_str()
        .ok_or_else(|| anyhow!("Lease {field} missing"))?;
    Ok(DateTime::parse_from_rfc3339(text)
        .with_context(|| format!("Lease {field} is invalid"))?
        .with_timezone(&Utc))
}

fn safe_relative_path(relative: &str, required_mount: &str) -> Result<()> {
    let path = Path::new(relative);
    if path.is_absolute()
        || relative.contains(['\\', ':', '\0'])
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir
                    | Component::CurDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
        || !relative.starts_with(&format!("{required_mount}/"))
    {
        bail!("unsafe path {relative}");
    }
    Ok(())
}

async fn reject_symlink_components(root: &Path, relative: &str) -> Result<PathBuf> {
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(component) = component else {
            bail!("unsafe path component in {relative}");
        };
        current.push(component);
        match tokio::fs::symlink_metadata(&current).await {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("symlink component rejected in {relative}");
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(current)
}

async fn reject_symlink_parents(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("path has no parent: {relative}"))?;
    reject_symlink_components(root, parent.to_string_lossy().as_ref()).await?;
    Ok(root.join(path))
}

async fn hash_file(path: &Path) -> Result<String> {
    Ok(aya_contracts::sha256_bytes(tokio::fs::read(path).await?))
}

async fn hash_sources(root: &Path, score: &Value) -> Result<HashMap<String, String>> {
    let mut hashes = HashMap::new();
    for input in score["inputs"]
        .as_array()
        .ok_or_else(|| anyhow!("Score inputs missing"))?
    {
        let relative = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Score input path missing"))?;
        safe_relative_path(relative, "input")?;
        let path = reject_symlink_components(root, relative).await?;
        hashes.insert(relative.to_owned(), hash_file(&path).await?);
    }
    Ok(hashes)
}

fn custody_error(
    score: &Value,
    before: &HashMap<String, String>,
    after: &HashMap<String, String>,
) -> Option<String> {
    for input in score["inputs"].as_array()? {
        let path = input["path"].as_str()?;
        let expected = input["sha256"].as_str()?;
        let before_digest = before.get(path).map(String::as_str);
        let after_digest = after.get(path).map(String::as_str);
        if before_digest != Some(expected) || after_digest != before_digest {
            return Some(format!("source custody violated for {path}"));
        }
    }
    None
}

fn now_text(clock: &dyn Clock) -> String {
    clock.now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn add_event(events: &mut Vec<Value>, clock: &dyn Clock, kind: &str, summary: &str) -> Result<()> {
    aya_contracts::append_receipt_event(events, &now_text(clock), kind, summary)
        .map_err(anyhow::Error::msg)
}

fn base_receipt(
    workcell_id: &str,
    score: &Value,
    lease: &Value,
    status: &str,
    events: Vec<Value>,
    candidate: Value,
    clock: &dyn Clock,
) -> Result<Value> {
    let mut receipt = json!({
        "schema": "aya.receipt/v0",
        "receiptId": format!("receipt-{workcell_id}"),
        "scoreSha256": aya_contracts::sha256_json(score).map_err(anyhow::Error::msg)?,
        "leaseSha256": aya_contracts::sha256_json(lease).map_err(anyhow::Error::msg)?,
        "status": status,
        "events": events,
        "candidate": candidate,
        "sealedAt": now_text(clock),
        "receiptSha256": aya_contracts::ZERO_SHA256
    });
    aya_contracts::seal_receipt(&mut receipt).map_err(anyhow::Error::msg)?;
    verify_receipt(&receipt)?;
    Ok(receipt)
}

fn verify_receipt(receipt: &Value) -> Result<()> {
    aya_contracts::validate_document(receipt).map_err(|errors| anyhow!(errors.join("; ")))?;
    let chain_errors = aya_contracts::verify_receipt_chain(
        receipt["events"]
            .as_array()
            .ok_or_else(|| anyhow!("Receipt events missing"))?,
    );
    if !chain_errors.is_empty() {
        bail!(chain_errors.join("; "));
    }
    if !aya_contracts::verify_receipt_digest(receipt).map_err(anyhow::Error::msg)? {
        bail!("Receipt digest is invalid");
    }
    Ok(())
}

async fn write_and_verify_receipt(root: &Path, receipt: &Value) -> Result<PathBuf> {
    safe_relative_path("work/receipt.json.tmp", "work")?;
    safe_relative_path("output/receipt.json", "output")?;
    let temporary = reject_symlink_components(root, "work/receipt.json.tmp").await?;
    let destination = reject_symlink_components(root, "output/receipt.json").await?;
    let bytes = serde_json::to_vec_pretty(receipt)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await?;
    file.write_all(&bytes).await?;
    file.sync_all().await?;
    drop(file);
    match tokio::fs::symlink_metadata(&destination).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Ok(_) => bail!("receipt destination already exists"),
        Err(error) => return Err(error.into()),
    }
    tokio::fs::rename(&temporary, &destination).await?;
    let on_disk: Value = serde_json::from_slice(&tokio::fs::read(&destination).await?)?;
    verify_receipt(&on_disk)?;
    if on_disk != *receipt {
        bail!("Receipt changed while writing to disk");
    }
    Ok(destination)
}

fn monotonic_deadline(config: &RunConfig) -> Result<Instant> {
    let created_at = parse_time(&config.lease, "createdAt")?;
    let expires_at = parse_time(&config.lease, "expiresAt")?;
    let now = config.clock.now();
    if expires_at <= created_at {
        bail!("Lease expiresAt must be later than createdAt");
    }
    if now < created_at {
        bail!("Lease is not active yet");
    }
    if now >= expires_at {
        bail!("Lease has expired");
    }
    if config.score["isolationRequired"] != "contract_only"
        || config.lease["isolationRequired"] != "contract_only"
    {
        bail!("Synthetic workcell supports only contract_only isolation");
    }
    for (name, expected) in [("input", "input"), ("work", "work"), ("output", "output")] {
        if config.lease["mounts"][name].as_str() != Some(expected) {
            bail!("Synthetic workcell requires {name} mount {expected}");
        }
    }
    let wall_seconds = config.score["budget"]["wallTimeSeconds"]
        .as_u64()
        .ok_or_else(|| anyhow!("Score wallTimeSeconds missing"))?;
    let lease_remaining = (expires_at - now)
        .to_std()
        .context("Lease remaining duration is invalid")?;
    Ok(Instant::now() + lease_remaining.min(Duration::from_secs(wall_seconds)))
}

struct AttemptFiles {
    before_relative: String,
    after_relative: String,
    summary_relative: String,
    candidate_relative: String,
}

impl AttemptFiles {
    fn new(attempt: u64) -> Self {
        let base = format!("work/attempt-{attempt}");
        Self {
            before_relative: format!("{base}/before.image.json"),
            after_relative: format!("{base}/after.image.json"),
            summary_relative: format!("{base}/scene-summary.json"),
            candidate_relative: format!("{base}/candidate.scene.json"),
        }
    }

    fn all(&self) -> [&str; 4] {
        [
            &self.before_relative,
            &self.after_relative,
            &self.summary_relative,
            &self.candidate_relative,
        ]
    }
}

async fn execute_attempt(
    dcc: &mut DccClient,
    config: &RunConfig,
    files: &AttemptFiles,
    source_path: &Path,
    deadline: Instant,
    events: &mut Vec<Value>,
    observed_orphan_pids: &mut Vec<u32>,
) -> Result<Value, AttemptError> {
    let discovered = dcc.call("discover", json!({}), deadline).await?;
    let capabilities = discovered["capabilities"]
        .as_array()
        .ok_or_else(|| AttemptError::Protocol("capability report malformed".to_owned()))?;
    for required in ["inspect", "execute", "capture", "save"] {
        if !capabilities
            .iter()
            .any(|value| value.as_str() == Some(required))
        {
            return Err(AttemptError::Protocol(format!(
                "fake DCC is missing {required}"
            )));
        }
    }

    dcc.call("inspect", json!({"source": source_path}), deadline)
        .await?;
    add_event(
        events,
        config.clock.as_ref(),
        "inspect",
        "Inspected synthetic source",
    )
    .map_err(|error| AttemptError::Io(error.to_string()))?;

    let before_path = config.root.join(&files.before_relative);
    let before = dcc
        .call(
            "capture",
            json!({"path": before_path, "label": "before"}),
            deadline,
        )
        .await?;
    let before_sha = hash_file(&before_path)
        .await
        .map_err(|error| AttemptError::Io(error.to_string()))?;
    if before["sha256"].as_str() != Some(before_sha.as_str()) {
        return Err(AttemptError::Integrity(
            "before evidence digest is inconsistent".to_owned(),
        ));
    }
    add_event(
        events,
        config.clock.as_ref(),
        "capture",
        "Captured before evidence",
    )
    .map_err(|error| AttemptError::Io(error.to_string()))?;

    dcc.call(
        "execute",
        json!({"targetTension": 1, "source": source_path}),
        deadline,
    )
    .await?;
    add_event(
        events,
        config.clock.as_ref(),
        "execute",
        "Applied first tension variation",
    )
    .map_err(|error| AttemptError::Io(error.to_string()))?;

    let probe = dcc
        .call("inspect", json!({"source": source_path}), deadline)
        .await?;
    if probe["scene"]["tension"].as_i64().unwrap_or_default() < 3 {
        add_event(
            events,
            config.clock.as_ref(),
            "diagnose",
            "First variation was below target tension",
        )
        .map_err(|error| AttemptError::Io(error.to_string()))?;
        add_event(
            events,
            config.clock.as_ref(),
            "correct",
            "Raised target tension while preserving invariants",
        )
        .map_err(|error| AttemptError::Io(error.to_string()))?;
        let corrected = dcc
            .call(
                "execute",
                json!({"targetTension": 3, "source": source_path}),
                deadline,
            )
            .await?;
        if let Some(pid) = corrected["orphanPid"].as_u64() {
            observed_orphan_pids.push(pid as u32);
        }
        add_event(
            events,
            config.clock.as_ref(),
            "execute",
            "Applied corrected tension variation",
        )
        .map_err(|error| AttemptError::Io(error.to_string()))?;
    }

    let final_scene = dcc
        .call("inspect", json!({"source": source_path}), deadline)
        .await?;
    if final_scene["scene"]["camera"] != "locked"
        || final_scene["scene"]["scale"] != 1
        || final_scene["scene"]["tension"] != 3
    {
        return Err(AttemptError::Integrity(
            "synthetic scene invariants or target failed".to_owned(),
        ));
    }

    let after_path = config.root.join(&files.after_relative);
    let after = dcc
        .call(
            "capture",
            json!({"path": after_path, "label": "after"}),
            deadline,
        )
        .await?;
    let after_sha = hash_file(&after_path)
        .await
        .map_err(|error| AttemptError::Io(error.to_string()))?;
    if after["sha256"].as_str() != Some(after_sha.as_str()) {
        return Err(AttemptError::Integrity(
            "after evidence digest is inconsistent".to_owned(),
        ));
    }
    add_event(
        events,
        config.clock.as_ref(),
        "capture",
        "Captured corrected evidence",
    )
    .map_err(|error| AttemptError::Io(error.to_string()))?;

    let summary_path = config.root.join(&files.summary_relative);
    tokio::fs::write(
        &summary_path,
        serde_json::to_vec_pretty(&final_scene)
            .map_err(|error| AttemptError::Io(error.to_string()))?,
    )
    .await
    .map_err(|error| AttemptError::Io(error.to_string()))?;

    let candidate_path = config.root.join(&files.candidate_relative);
    let saved = dcc
        .call(
            "save",
            json!({
                "path": candidate_path,
                "relativePath": files.candidate_relative
            }),
            deadline,
        )
        .await?;
    let returned = saved["artifactPath"]
        .as_str()
        .ok_or_else(|| AttemptError::Protocol("save omitted artifactPath".to_owned()))?;
    if returned != files.candidate_relative {
        return Err(AttemptError::Integrity(format!(
            "fake DCC returned unexpected artifact path {returned}"
        )));
    }
    safe_relative_path(returned, "work")
        .map_err(|error| AttemptError::Integrity(error.to_string()))?;
    reject_symlink_components(&config.root, returned)
        .await
        .map_err(|error| AttemptError::Integrity(error.to_string()))?;
    add_event(
        events,
        config.clock.as_ref(),
        "candidate",
        "Saved derived synthetic candidate",
    )
    .map_err(|error| AttemptError::Io(error.to_string()))?;
    Ok(final_scene["scene"].clone())
}

async fn remove_attempt(config: &RunConfig, attempt: u64) -> Result<()> {
    let relative = format!("work/attempt-{attempt}");
    safe_relative_path(&relative, "work")?;
    let path = reject_symlink_components(&config.root, &relative).await?;
    match tokio::fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn cleanup_unaccepted_output(root: &Path) -> Result<()> {
    reject_symlink_components(root, "output").await?;
    for relative in [
        "output/before.image.json",
        "output/after.image.json",
        "output/scene-summary.json",
        "output/candidate.scene.json",
        "output/candidate.sha256",
    ] {
        safe_relative_path(relative, "output")?;
        let path = reject_symlink_parents(root, relative).await?;
        match tokio::fs::remove_file(path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

async fn promote_and_hash(
    config: &RunConfig,
    files: &AttemptFiles,
) -> Result<HashMap<&'static str, String>> {
    let destinations = [
        (&files.before_relative, "output/before.image.json", "before"),
        (&files.after_relative, "output/after.image.json", "after"),
        (
            &files.summary_relative,
            "output/scene-summary.json",
            "summary",
        ),
        (
            &files.candidate_relative,
            "output/candidate.scene.json",
            "candidate",
        ),
    ];
    let mut hashes = HashMap::new();
    for (source, relative, key) in destinations {
        safe_relative_path(relative, "output")?;
        let destination = reject_symlink_components(&config.root, relative).await?;
        tokio::fs::rename(config.root.join(source), &destination).await?;
        hashes.insert(key, hash_file(&destination).await?);
    }
    let hash_path = reject_symlink_components(&config.root, "output/candidate.sha256").await?;
    let mut hash_file_handle = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&hash_path)
        .await?;
    hash_file_handle
        .write_all(format!("{}\n", hashes["candidate"]).as_bytes())
        .await?;
    hash_file_handle.sync_all().await?;
    drop(hash_file_handle);
    hashes.insert("artifact_hash", hash_file(&hash_path).await?);
    Ok(hashes)
}

fn candidate_value(
    score: &Value,
    source_before: &HashMap<String, String>,
    source_after: &HashMap<String, String>,
    hashes: &HashMap<&str, String>,
) -> Result<Value> {
    let mut sources = Vec::new();
    for input in score["inputs"]
        .as_array()
        .ok_or_else(|| anyhow!("Score inputs missing"))?
    {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Score input path missing"))?;
        sources.push(json!({
            "path": path,
            "beforeSha256": source_before.get(path).ok_or_else(|| anyhow!("missing before digest"))?,
            "afterSha256": source_after.get(path).ok_or_else(|| anyhow!("missing after digest"))?
        }));
    }
    Ok(json!({
        "sources": sources,
        "artifacts": [{
            "path": "output/candidate.scene.json",
            "sha256": hashes["candidate"]
        }],
        "evidence": [
            {"kind": "before_image", "path": "output/before.image.json", "sha256": hashes["before"]},
            {"kind": "after_image", "path": "output/after.image.json", "sha256": hashes["after"]},
            {"kind": "scene_summary", "path": "output/scene-summary.json", "sha256": hashes["summary"]},
            {"kind": "artifact_hash", "path": "output/candidate.sha256", "sha256": hashes["artifact_hash"]}
        ]
    }))
}

fn validate_candidate_context(score: &Value, lease: &Value, candidate: &Value) -> Result<()> {
    let input_mount = lease["mounts"]["input"].as_str().unwrap();
    let output_mount = lease["mounts"]["output"].as_str().unwrap();
    let expected_sources: HashMap<_, _> = score["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|input| {
            (
                input["path"].as_str().unwrap(),
                input["sha256"].as_str().unwrap(),
            )
        })
        .collect();
    let actual_sources = candidate["sources"]
        .as_array()
        .ok_or_else(|| anyhow!("candidate sources missing"))?;
    if actual_sources.len() != expected_sources.len() {
        bail!("candidate source set does not match Score");
    }
    for source in actual_sources {
        let path = source["path"]
            .as_str()
            .ok_or_else(|| anyhow!("candidate source path missing"))?;
        safe_relative_path(path, input_mount)?;
        let expected = expected_sources
            .get(path)
            .ok_or_else(|| anyhow!("unexpected candidate source {path}"))?;
        if source["beforeSha256"].as_str() != Some(*expected)
            || source["afterSha256"] != source["beforeSha256"]
        {
            bail!("candidate source custody failed for {path}");
        }
    }

    let evidence = candidate["evidence"]
        .as_array()
        .ok_or_else(|| anyhow!("candidate evidence missing"))?;
    for required in score["evidenceRequired"].as_array().unwrap() {
        if !evidence.iter().any(|item| item["kind"] == *required) {
            bail!("candidate is missing required evidence kind {required}");
        }
    }
    for item in candidate["artifacts"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(evidence)
    {
        let path = item["path"]
            .as_str()
            .ok_or_else(|| anyhow!("candidate item path missing"))?;
        safe_relative_path(path, output_mount)?;
    }
    Ok(())
}

async fn verify_candidate_on_disk(root: &Path, candidate: &Value) -> Result<()> {
    for item in candidate["artifacts"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(candidate["evidence"].as_array().into_iter().flatten())
    {
        let relative = item["path"]
            .as_str()
            .ok_or_else(|| anyhow!("candidate item path missing"))?;
        safe_relative_path(relative, "output")?;
        let path = reject_symlink_components(root, relative).await?;
        let actual = hash_file(&path).await?;
        if item["sha256"].as_str() != Some(actual.as_str()) {
            bail!("candidate item digest mismatch for {relative}");
        }
    }
    Ok(())
}

pub async fn record_prestart_cancellation(
    workcell_id: &str,
    root: &Path,
    score: &Value,
    lease: &Value,
    clock: &dyn Clock,
) -> Result<RunReport> {
    cleanup_unaccepted_output(root).await?;
    let receipt_path = reject_symlink_parents(root, "output/receipt.json").await?;
    match tokio::fs::remove_file(&receipt_path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let source_before = hash_sources(root, score).await?;
    let source_after = hash_sources(root, score).await?;
    if let Some(error) = custody_error(score, &source_before, &source_after) {
        bail!(error);
    }
    let mut events = Vec::new();
    add_event(
        &mut events,
        clock,
        "failure",
        "Workcell cancelled before Worker execution",
    )?;
    let receipt = base_receipt(
        workcell_id,
        score,
        lease,
        "cancelled",
        events,
        Value::Null,
        clock,
    )?;
    let receipt_path = write_and_verify_receipt(root, &receipt).await?;
    Ok(RunReport {
        states: vec![
            WorkcellState::Requested,
            WorkcellState::Admitted,
            WorkcellState::Cancelled,
            WorkcellState::Expired,
        ],
        outcome: WorkcellOutcome::Cancelled,
        attempts: 0,
        receipt,
        receipt_path,
        source_before_sha256: source_before,
        source_after_sha256: source_after,
        process_tree_reaped: true,
        observed_orphan_pids: Vec::new(),
    })
}

pub async fn discover_synthetic(config: &RunConfig) -> Result<Value> {
    SUBREAPER
        .get_or_init(|| set_child_subreaper(true))
        .as_ref()
        .map_err(|error| anyhow!("could not become child subreaper: {error}"))?;
    let deadline = Instant::now() + config.request_timeout.max(Duration::from_millis(100));
    let mut dcc = DccClient::spawn(config)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let result = dcc.call("discover", json!({}), deadline).await;
    let cleanup = dcc.reap(deadline, &[]).await;
    cleanup.map_err(|error| anyhow!(error.to_string()))?;
    result.map_err(|error| anyhow!(error.to_string()))
}

pub async fn run_synthetic(config: RunConfig) -> Result<RunReport> {
    SUBREAPER
        .get_or_init(|| set_child_subreaper(true))
        .as_ref()
        .map_err(|error| anyhow!("could not become child subreaper: {error}"))?;
    aya_contracts::validate_score(&config.score).map_err(|errors| anyhow!(errors.join("; ")))?;
    aya_contracts::validate_document(&config.lease).map_err(|errors| anyhow!(errors.join("; ")))?;
    let deadline = monotonic_deadline(&config)?;
    for mount in ["input", "work", "output"] {
        reject_symlink_components(&config.root, mount)
            .await
            .with_context(|| format!("unsafe synthetic {mount} mount"))?;
    }

    let mut states = vec![WorkcellState::Requested];
    let source_before = hash_sources(&config.root, &config.score).await?;
    let admitted_hashes: HashMap<_, _> = config.score["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|input| {
            (
                input["path"].as_str().unwrap().to_owned(),
                input["sha256"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    if source_before != admitted_hashes {
        bail!("synthetic source digests do not match Score");
    }
    let score_sha256 = aya_contracts::sha256_json(&config.score).map_err(anyhow::Error::msg)?;
    if config.lease["scoreSha256"].as_str() != Some(score_sha256.as_str()) {
        bail!("Lease does not bind the Score");
    }
    states.push(WorkcellState::Admitted);
    states.push(WorkcellState::Running);

    let max_attempts = config.score["budget"]["maxAttempts"].as_u64().unwrap_or(1);
    let source_path = config
        .root
        .join(config.score["inputs"][0]["path"].as_str().unwrap());
    let mut events = Vec::new();
    let mut observed_orphan_pids = Vec::new();
    let mut attempts = 0;
    let mut process_tree_reaped = true;
    let mut terminal_error = AttemptError::Protocol("attempt budget exhausted".to_owned());
    let mut expired = false;
    let mut cancelled = false;

    while attempts < max_attempts {
        if config.cancellation.is_cancelled() {
            terminal_error = AttemptError::Cancelled;
            cancelled = true;
            break;
        }
        if Instant::now() >= deadline {
            terminal_error = AttemptError::Deadline;
            expired = true;
            break;
        }
        attempts += 1;
        remove_attempt(&config, attempts).await?;
        tokio::fs::create_dir_all(config.root.join(format!("work/attempt-{attempts}"))).await?;
        let files = AttemptFiles::new(attempts);

        let mut dcc = match DccClient::spawn(&config).await {
            Ok(dcc) => dcc,
            Err(error) => {
                add_event(
                    &mut events,
                    config.clock.as_ref(),
                    "failure",
                    &format!("Attempt {attempts} failed: {error}"),
                )?;
                let retry = error.retryable() && attempts < max_attempts;
                terminal_error = error;
                let _ = remove_attempt(&config, attempts).await;
                if retry {
                    continue;
                }
                break;
            }
        };
        let attempt_result = execute_attempt(
            &mut dcc,
            &config,
            &files,
            &source_path,
            deadline,
            &mut events,
            &mut observed_orphan_pids,
        )
        .await;
        let reap_error = match dcc.reap(deadline, &observed_orphan_pids).await {
            Ok(report) => {
                process_tree_reaped &= report.process_group_gone;
                None
            }
            Err(error) => {
                process_tree_reaped = false;
                Some(error)
            }
        };

        let source_after_attempt = match hash_sources(&config.root, &config.score).await {
            Ok(hashes) => hashes,
            Err(error) => {
                let summary = format!("could not verify source custody: {error}");
                add_event(&mut events, config.clock.as_ref(), "violation", &summary)?;
                terminal_error = AttemptError::Integrity(summary);
                let _ = remove_attempt(&config, attempts).await;
                break;
            }
        };
        if let Some(error) = custody_error(&config.score, &source_before, &source_after_attempt) {
            add_event(&mut events, config.clock.as_ref(), "violation", &error)?;
            terminal_error = AttemptError::Integrity(error);
            let _ = remove_attempt(&config, attempts).await;
            break;
        }
        if let Some(error) = reap_error {
            add_event(
                &mut events,
                config.clock.as_ref(),
                "failure",
                &format!("Process cleanup failed: {error}"),
            )?;
            terminal_error = error;
            let _ = remove_attempt(&config, attempts).await;
            break;
        }

        match attempt_result {
            Ok(expected_scene) => {
                if Instant::now() >= deadline {
                    expired = true;
                    terminal_error = AttemptError::Deadline;
                    let _ = remove_attempt(&config, attempts).await;
                    break;
                }
                for relative in files.all() {
                    if let Err(error) = reject_symlink_components(&config.root, relative).await {
                        terminal_error = AttemptError::Integrity(error.to_string());
                        break;
                    }
                    if let Err(error) = hash_file(&config.root.join(relative)).await {
                        terminal_error = AttemptError::Integrity(format!(
                            "attempt output {relative} is unreadable: {error}"
                        ));
                        break;
                    }
                }
                if matches!(terminal_error, AttemptError::Integrity(_)) {
                    let _ = remove_attempt(&config, attempts).await;
                    break;
                }

                let finalized: Result<(Value, HashMap<String, String>)> = async {
                    let staged_candidate: Value = serde_json::from_slice(
                        &tokio::fs::read(config.root.join(&files.candidate_relative)).await?,
                    )?;
                    if staged_candidate != expected_scene {
                        bail!("saved candidate differs from the validated final scene");
                    }
                    if config.cancellation.is_cancelled() {
                        bail!("workcell cancelled before candidate promotion");
                    }
                    if Instant::now() >= deadline {
                        bail!("workcell deadline expired before candidate promotion");
                    }
                    let hashes = promote_and_hash(&config, &files).await?;
                    if config.cancellation.is_cancelled() {
                        bail!("workcell cancelled after candidate promotion");
                    }
                    remove_attempt(&config, attempts).await?;
                    let source_after = hash_sources(&config.root, &config.score).await?;
                    if let Some(error) = custody_error(&config.score, &source_before, &source_after)
                    {
                        bail!(error);
                    }
                    let candidate =
                        candidate_value(&config.score, &source_before, &source_after, &hashes)?;
                    validate_candidate_context(&config.score, &config.lease, &candidate)?;
                    verify_candidate_on_disk(&config.root, &candidate).await?;
                    let promoted_candidate: Value = serde_json::from_slice(
                        &tokio::fs::read(config.root.join("output/candidate.scene.json")).await?,
                    )?;
                    if promoted_candidate != expected_scene {
                        bail!("promoted candidate differs from the validated final scene");
                    }
                    Ok((candidate, source_after))
                }
                .await;
                let (candidate, _source_after) = match finalized {
                    Ok(result) => result,
                    Err(error) => {
                        cancelled = error.to_string().contains("workcell cancelled");
                        terminal_error = if cancelled {
                            AttemptError::Cancelled
                        } else {
                            AttemptError::Integrity(error.to_string())
                        };
                        let _ = remove_attempt(&config, attempts).await;
                        break;
                    }
                };
                if config.cancellation.is_cancelled() {
                    cancelled = true;
                    terminal_error = AttemptError::Cancelled;
                    break;
                }
                if Instant::now() >= deadline {
                    expired = true;
                    terminal_error = AttemptError::Deadline;
                    break;
                }
                states.push(WorkcellState::CandidateReady);
                let sealed: Result<(Value, PathBuf)> = async {
                    if config.cancellation.is_cancelled() {
                        bail!("workcell cancelled before receipt sealing");
                    }
                    if Instant::now() >= deadline {
                        bail!("workcell deadline expired before receipt sealing");
                    }
                    let mut receipt = base_receipt(
                        &config.workcell_id,
                        &config.score,
                        &config.lease,
                        "candidate_ready",
                        events.clone(),
                        candidate,
                        config.clock.as_ref(),
                    )?;
                    if config.scenario == Scenario::InconsistentReceipt {
                        receipt["events"][0]["eventSha256"] =
                            Value::String(aya_contracts::ZERO_SHA256.to_owned());
                    }
                    verify_receipt(&receipt).context("receipt consistency verification failed")?;
                    if config.cancellation.is_cancelled() {
                        bail!("workcell cancelled before receipt write");
                    }
                    if Instant::now() >= deadline {
                        bail!("workcell deadline expired before receipt write");
                    }
                    let receipt_path = write_and_verify_receipt(&config.root, &receipt).await?;
                    Ok((receipt, receipt_path))
                }
                .await;
                let (receipt, receipt_path) = match sealed {
                    Ok(result) => result,
                    Err(error) => {
                        expired = error.to_string().contains("deadline expired");
                        cancelled = error.to_string().contains("workcell cancelled");
                        terminal_error = if expired {
                            AttemptError::Deadline
                        } else if cancelled {
                            AttemptError::Cancelled
                        } else {
                            AttemptError::Integrity(error.to_string())
                        };
                        break;
                    }
                };
                let final_sources = match hash_sources(&config.root, &config.score).await {
                    Ok(hashes) => hashes,
                    Err(error) => {
                        let _ = tokio::fs::remove_file(&receipt_path).await;
                        terminal_error = AttemptError::Integrity(format!(
                            "final source custody verification failed: {error}"
                        ));
                        break;
                    }
                };
                if let Some(error) = custody_error(&config.score, &source_before, &final_sources) {
                    let _ = tokio::fs::remove_file(&receipt_path).await;
                    terminal_error = AttemptError::Integrity(error);
                    break;
                }
                if let Err(error) =
                    verify_candidate_on_disk(&config.root, &receipt["candidate"]).await
                {
                    let _ = tokio::fs::remove_file(&receipt_path).await;
                    terminal_error = AttemptError::Integrity(format!(
                        "final candidate verification failed: {error}"
                    ));
                    break;
                }
                if config.cancellation.is_cancelled() {
                    let _ = tokio::fs::remove_file(&receipt_path).await;
                    cancelled = true;
                    terminal_error = AttemptError::Cancelled;
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = tokio::fs::remove_file(&receipt_path).await;
                    expired = true;
                    terminal_error = AttemptError::Deadline;
                    break;
                }
                states.push(WorkcellState::Sealed);
                states.push(WorkcellState::Expired);
                return Ok(RunReport {
                    states,
                    outcome: WorkcellOutcome::CandidateReady,
                    attempts,
                    receipt,
                    receipt_path,
                    source_before_sha256: source_before,
                    source_after_sha256: final_sources,
                    process_tree_reaped,
                    observed_orphan_pids,
                });
            }
            Err(error) => {
                expired = matches!(error, AttemptError::Deadline);
                cancelled = matches!(error, AttemptError::Cancelled);
                add_event(
                    &mut events,
                    config.clock.as_ref(),
                    "failure",
                    &format!("Attempt {attempts} failed: {error}"),
                )?;
                let retry = error.retryable() && attempts < max_attempts && !expired && !cancelled;
                terminal_error = error;
                remove_attempt(&config, attempts).await?;
                if retry {
                    continue;
                }
                break;
            }
        }
    }

    cleanup_unaccepted_output(&config.root).await?;
    let source_after = match hash_sources(&config.root, &config.score).await {
        Ok(hashes) => {
            if let Some(error) = custody_error(&config.score, &source_before, &hashes) {
                add_event(&mut events, config.clock.as_ref(), "violation", &error)?;
                terminal_error = AttemptError::Integrity(error);
            }
            hashes
        }
        Err(error) => {
            add_event(
                &mut events,
                config.clock.as_ref(),
                "violation",
                &format!("final source custody verification failed: {error}"),
            )?;
            HashMap::new()
        }
    };
    let last_kind = events.last().and_then(|event| event["kind"].as_str());
    if !matches!(last_kind, Some("failure" | "violation")) {
        add_event(
            &mut events,
            config.clock.as_ref(),
            "failure",
            &terminal_error.to_string(),
        )?;
    }
    if cancelled {
        states.push(WorkcellState::Cancelled);
        if process_tree_reaped {
            states.push(WorkcellState::Expired);
        }
    } else if expired && process_tree_reaped {
        states.push(WorkcellState::Expired);
    } else {
        states.push(WorkcellState::Failed);
        if process_tree_reaped {
            states.push(WorkcellState::Expired);
        }
    }
    let status = if cancelled {
        "cancelled"
    } else if expired && process_tree_reaped {
        "expired"
    } else {
        "failed"
    };
    let receipt = base_receipt(
        &config.workcell_id,
        &config.score,
        &config.lease,
        status,
        events,
        Value::Null,
        config.clock.as_ref(),
    )?;
    let receipt_path = write_and_verify_receipt(&config.root, &receipt).await?;
    Ok(RunReport {
        states,
        outcome: if cancelled {
            WorkcellOutcome::Cancelled
        } else if expired && process_tree_reaped {
            WorkcellOutcome::Expired
        } else {
            WorkcellOutcome::Failed
        },
        attempts,
        receipt,
        receipt_path,
        source_before_sha256: source_before,
        source_after_sha256: source_after,
        process_tree_reaped,
        observed_orphan_pids,
    })
}
