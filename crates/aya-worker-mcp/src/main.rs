use rmcp::{
    Json, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, JsonSchema)]
struct ScoreRequest {
    score: Value,
}

#[derive(Debug, Serialize, JsonSchema)]
struct StatusResponse {
    role: &'static str,
    version: &'static str,
    isolation_level: &'static str,
    execution_enabled: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ScoreResponse {
    ok: bool,
    score_sha256: Option<String>,
    errors: Vec<String>,
}

#[derive(Clone)]
struct WorkerServer {
    tool_router: ToolRouter<Self>,
}

impl WorkerServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router(router = tool_router)]
impl WorkerServer {
    #[tool(
        name = "aya_worker_status",
        description = "Report Worker MCP status; this scaffold never claims operating-system confinement"
    )]
    async fn status(&self) -> Json<StatusResponse> {
        Json(StatusResponse {
            role: "worker",
            version: env!("CARGO_PKG_VERSION"),
            isolation_level: "contract_only",
            execution_enabled: false,
        })
    }

    #[tool(
        name = "aya_score_read",
        description = "Validate and read the score assigned to this experimental worker"
    )]
    async fn read_score(
        &self,
        Parameters(request): Parameters<ScoreRequest>,
    ) -> Json<ScoreResponse> {
        match aya_contracts::validate_score(&request.score) {
            Ok(()) => Json(ScoreResponse {
                ok: true,
                score_sha256: aya_contracts::sha256_json(&request.score).ok(),
                errors: Vec::new(),
            }),
            Err(errors) => Json(ScoreResponse {
                ok: false,
                score_sha256: None,
                errors,
            }),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for WorkerServer {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    eprintln!(
        "AYA Worker MCP v{} starting on stdio",
        env!("CARGO_PKG_VERSION")
    );
    let service = WorkerServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_scaffold_has_no_execution_before_confinement() {
        let mut names: Vec<_> = WorkerServer::new()
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["aya_score_read", "aya_worker_status"]);
        assert!(names.iter().all(|name| !name.contains("execute")));
    }
}
