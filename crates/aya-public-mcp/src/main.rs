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
struct DocumentRequest {
    document: Value,
}

#[derive(Debug, Serialize, JsonSchema)]
struct StatusResponse {
    role: &'static str,
    version: &'static str,
    isolation_level: &'static str,
    runtime_available: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ValidationResponse {
    ok: bool,
    schema: Option<String>,
    sha256: Option<String>,
    errors: Vec<String>,
}

#[derive(Clone)]
struct PublicServer {
    tool_router: ToolRouter<Self>,
}

impl PublicServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router(router = tool_router)]
impl PublicServer {
    #[tool(
        name = "aya_public_status",
        description = "Report the experimental Public MCP process status without creating a workcell"
    )]
    async fn status(&self) -> Json<StatusResponse> {
        Json(StatusResponse {
            role: "public",
            version: env!("CARGO_PKG_VERSION"),
            isolation_level: "contract_only",
            runtime_available: false,
        })
    }

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
}

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for PublicServer {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    eprintln!(
        "AYA Public MCP v{} starting on stdio",
        env!("CARGO_PKG_VERSION")
    );
    let service = PublicServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_surface_is_narrow() {
        let mut names: Vec<_> = PublicServer::new()
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["aya_public_status", "aya_score_validate"]);
        assert!(names.iter().all(|name| !name.contains("execute")));
    }
}
