//! The MCP server Skarn presents to the AI client.
//!
//! Instead of forwarding hundreds of downstream tool schemas, it exposes a tiny,
//! constant surface — `search`, `read_tool_docs`, and `execute` — plus, if
//! configured, the namespaced downstream tools in passthrough mode.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use skarn_codemode::ExecLimits;

use crate::config::Isolation;
use crate::downstream::DownstreamManager;
use crate::execute::execute_code;

/// The gateway's upstream MCP server handler.
pub struct GatewayServer {
    manager: Arc<DownstreamManager>,
    limits: ExecLimits,
    passthrough: bool,
    isolation: Isolation,
    instructions: String,
}

impl GatewayServer {
    pub fn new(
        manager: Arc<DownstreamManager>,
        limits: ExecLimits,
        passthrough: bool,
        isolation: Isolation,
        instructions: String,
    ) -> Self {
        Self {
            manager,
            limits,
            passthrough,
            isolation,
            instructions,
        }
    }

    /// The fixed meta-tools, always exposed.
    fn meta_tools() -> Vec<Tool> {
        vec![
            Tool::new(
                "search",
                "Search the connected MCP servers for tools relevant to a task. \
                 Returns ranked tool names you can then call from `execute` via skarn.callTool().",
                schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "What you want to do" },
                        "limit": { "type": "integer", "description": "Max results (default 15)" }
                    },
                    "required": ["query"]
                })),
            ),
            Tool::new(
                "read_tool_docs",
                "Get the full JSON Schema and server for a namespaced tool \
                 (e.g. `github__search_issues`), for authoring an `execute` script.",
                schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Namespaced tool name" }
                    },
                    "required": ["name"]
                })),
            ),
            Tool::new(
                "execute",
                "Run a sandboxed JavaScript/TypeScript orchestration script. Use \
                 `await skarn.callTool(server, tool, args)` or `skarn.server(name).tool(args)` to \
                 call downstream tools, process results locally, and `return` a small summary. \
                 Only the returned value and skarn.log() lines come back — large intermediate \
                 data never enters your context.",
                schema(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "code": { "type": "string", "description": "The script body (async; use `return`)" },
                        "language": { "type": "string", "enum": ["js", "ts"], "description": "Defaults to ts" }
                    },
                    "required": ["code"]
                })),
            ),
        ]
    }

    async fn handle_search(&self, args: &serde_json::Value) -> CallToolResult {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(15)
            .clamp(1, 100) as usize;
        let hits = self.manager.registry().search(query, limit);
        let body = serde_json::json!({
            "query": query,
            "matches": hits,
            "hint": "Call these with skarn.callTool(server, tool, args) inside an `execute` script.",
        });
        CallToolResult::success(vec![Content::text(body.to_string())])
    }

    fn handle_read_tool_docs(&self, args: &serde_json::Value) -> CallToolResult {
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let registry = self.manager.registry();
        match registry.tools().iter().find(|t| t.namespaced == name) {
            Some(t) => {
                let body = serde_json::json!({
