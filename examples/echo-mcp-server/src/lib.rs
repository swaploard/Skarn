//! A minimal downstream MCP server exposing two tools, `echo` and `add`.
//!
//! Used by Skarn's examples and integration tests as a real server the
//! gateway can aggregate — over stdio (as the `echo-mcp-server` binary) or
//! in-process over HTTP (in tests).

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};

/// A tiny echo/add MCP server.
#[derive(Clone)]
pub struct EchoServer;

impl EchoServer {
    /// The tool manifest this server exposes.
    pub fn tools() -> Vec<Tool> {
        vec![
            Tool::new(
                "echo",
                "Echo back the provided text.",
                obj(serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                })),
            ),
            Tool::new(
                "add",
                "Add two integers and return the sum.",
                obj(serde_json::json!({
                    "type": "object",
                    "properties": { "a": { "type": "number" }, "b": { "type": "number" } },
                    "required": ["a", "b"]
                })),
            ),
        ]
    }
}

impl ServerHandler for EchoServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some("A tiny echo/add server for Skarn demos.".to_string());
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(Self::tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let args = request
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or(serde_json::Value::Null);
        match request.name.as_ref() {
            "echo" => {
                let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::json!({ "echoed": text }).to_string(),
                )]))
            }
            "add" => {
                let a = args.get("a").and_then(|v| v.as_i64()).unwrap_or(0);
                let b = args.get("b").and_then(|v| v.as_i64()).unwrap_or(0);
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::json!({ "sum": a + b }).to_string(),
                )]))
            }
            other => Err(McpError::invalid_params(
                format!("unknown tool `{other}`"),
                None,
            )),
        }
    }
}

fn obj(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    match v {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    }
}
