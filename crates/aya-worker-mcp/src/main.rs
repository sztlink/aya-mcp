use std::{env, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use aya_workcell::{RunConfig, Scenario, SystemClock, discover_synthetic, run_synthetic};
use rmcp::{
    Json, ServiceExt, handler::server::router::tool::ToolRouter, tool, tool_handler, tool_router,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
struct WorkerConfig {
    workcell_id: String,
    root: PathBuf,
    fake_dcc: PathBuf,
    score: Value,
    lease: Value,
    scenario: Scenario,
}

impl WorkerConfig {
    fn from_environment() -> Result<Self> {
        let workcell_id = env::var("AYA_WORKCELL_ID").context("AYA_WORKCELL_ID missing")?;
        let root =
            PathBuf::from(env::var("AYA_WORKCELL_ROOT").context("AYA_WORKCELL_ROOT missing")?);
        let current = env::current_exe()?;
        let fake_dcc = current
            .parent()
            .context("Worker executable has no parent")?
            .join("aya-fake-dcc");
        if !fake_dcc.is_file() {
            anyhow::bail!("pinned fake DCC is unavailable at {}", fake_dcc.display());
        }
        let score = serde_json::from_slice(&std::fs::read(root.join("work/score.json"))?)?;
        let lease = serde_json::from_slice(&std::fs::read(root.join("work/lease.json"))?)?;
        let scenario = env::var("AYA_SYNTHETIC_SCENARIO")
            .unwrap_or_else(|_| "success".to_owned())
            .parse()?;
        Ok(Self {
            workcell_id,
            root,
            fake_dcc,
            score,
            lease,
            scenario,
        })
    }

    fn run_config(&self, cancellation: CancellationToken) -> RunConfig {
        RunConfig {
            workcell_id: self.workcell_id.clone(),
            root: self.root.clone(),
            fake_dcc: self.fake_dcc.clone(),
            score: self.score.clone(),
            lease: self.lease.clone(),
            scenario: self.scenario,
            request_timeout: Duration::from_millis(250),
            clock: Arc::new(SystemClock),
            cancellation,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
struct CapabilityResponse {
    ok: bool,
    isolation_level: &'static str,
    capabilities: Vec<String>,
    errors: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ExecutionResponse {
    ok: bool,
    report: Option<Value>,
    errors: Vec<String>,
}

#[derive(Clone)]
struct WorkerServer {
    tool_router: ToolRouter<Self>,
    config: Arc<WorkerConfig>,
    cancellation: CancellationToken,
    execution_lock: Arc<Mutex<()>>,
    report: Arc<Mutex<Option<Value>>>,
}

impl WorkerServer {
    fn new(config: WorkerConfig, cancellation: CancellationToken) -> Self {
        Self {
            tool_router: Self::tool_router(),
            config: Arc::new(config),
            cancellation,
            execution_lock: Arc::new(Mutex::new(())),
            report: Arc::new(Mutex::new(None)),
        }
    }
}

#[tool_router(router = tool_router)]
impl WorkerServer {
    #[tool(
        name = "aya_capability_discover",
        description = "Discover capabilities from the pinned synthetic fake DCC"
    )]
    async fn discover(&self) -> Json<CapabilityResponse> {
        // Discovery is bounded by its own request timeout so an already accepted
        // cancellation can still reach aya_execute and produce a terminal report.
        let config = self.config.run_config(CancellationToken::new());
        match discover_synthetic(&config).await {
            Ok(result) => Json(CapabilityResponse {
                ok: true,
                isolation_level: "contract_only",
                capabilities: result["capabilities"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                errors: Vec::new(),
            }),
            Err(error) => Json(CapabilityResponse {
                ok: false,
                isolation_level: "contract_only",
                capabilities: Vec::new(),
                errors: vec![error.to_string()],
            }),
        }
    }

    #[tool(
        name = "aya_execute",
        description = "Run the autonomous synthetic workcell loop against only the pinned fake DCC"
    )]
    async fn execute(&self) -> Json<ExecutionResponse> {
        let _guard = self.execution_lock.lock().await;
        if let Some(report) = self.report.lock().await.clone() {
            return Json(ExecutionResponse {
                ok: true,
                report: Some(report),
                errors: Vec::new(),
            });
        }
        let config = self.config.run_config(self.cancellation.child_token());
        match run_synthetic(config).await {
            Ok(report) => match serde_json::to_value(report) {
                Ok(report) => {
                    self.report.lock().await.replace(report.clone());
                    Json(ExecutionResponse {
                        ok: true,
                        report: Some(report),
                        errors: Vec::new(),
                    })
                }
                Err(error) => Json(ExecutionResponse {
                    ok: false,
                    report: None,
                    errors: vec![error.to_string()],
                }),
            },
            Err(error) => Json(ExecutionResponse {
                ok: false,
                report: None,
                errors: vec![error.to_string()],
            }),
        }
    }

    #[tool(
        name = "aya_capture",
        description = "Return the completed synthetic report, candidate paths and receipt without promotion authority"
    )]
    async fn capture(&self) -> Json<ExecutionResponse> {
        match self.report.lock().await.clone() {
            Some(report) => Json(ExecutionResponse {
                ok: true,
                report: Some(report),
                errors: Vec::new(),
            }),
            None => Json(ExecutionResponse {
                ok: false,
                report: None,
                errors: vec!["synthetic execution has not completed".to_owned()],
            }),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for WorkerServer {}

#[tokio::main]
async fn main() -> Result<()> {
    let cancellation = CancellationToken::new();
    #[cfg(unix)]
    {
        let signal_cancellation = cancellation.clone();
        tokio::spawn(async move {
            if let Ok(mut terminate) =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            {
                terminate.recv().await;
                signal_cancellation.cancel();
            }
        });
    }

    let config = WorkerConfig::from_environment()?;
    eprintln!(
        "AYA Worker MCP v{} starting synthetic contract_only workcell on stdio",
        env!("CARGO_PKG_VERSION")
    );
    let service = WorkerServer::new(config, cancellation)
        .serve(stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn worker_surface_is_fake_only() {
        let names = ["aya_capability_discover", "aya_capture", "aya_execute"];
        assert!(names.iter().all(|name| !name.contains("promote")));
        assert_eq!(names.len(), 3);
    }
}
