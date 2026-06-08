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
                    "namespaced": t.namespaced,
                    "server": t.server,
                    "tool": t.tool,
                    "description": t.description,
                    "inputSchema": t.input_schema,
                });
                CallToolResult::success(vec![Content::text(body.to_string())])
            }
            None => CallToolResult::error(vec![Content::text(format!(
                "no tool named `{name}`. Use `search` to discover tool names."
            ))]),
        }
    }

    async fn handle_execute(&self, args: &serde_json::Value) -> CallToolResult {
        let code = match args.get("code").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return CallToolResult::error(vec![Content::text("missing `code` argument")]);
            }
        };
        match execute_code(
            self.manager.clone(),
            self.limits,
            code.to_string(),
            self.isolation,
        )
        .await
        {
            Ok(outcome) if outcome.ok => {
                let body = serde_json::json!({
                    "result": outcome.value,
                    "logs": outcome.logs,
                    "toolCalls": outcome.tool_calls,
                });
                CallToolResult::success(vec![Content::text(body.to_string())])
            }
            Ok(outcome) => {
                let mut msg = format!(
                    "Script error: {}",
                    outcome.error.unwrap_or_else(|| "unknown".into())
                );
                if !outcome.logs.is_empty() {
                    msg.push_str("\n\nlogs:\n");
                    msg.push_str(&outcome.logs.join("\n"));
                }
                CallToolResult::error(vec![Content::text(msg)])
            }
            Err(e) => CallToolResult::error(vec![Content::text(format!("Execution failed: {e}"))]),
        }
    }

    async fn handle_passthrough(&self, name: &str, args_json: &str) -> CallToolResult {
        let registry = self.manager.registry();
        match registry.resolve(name) {
            Some((server, tool)) => {
                let (server, tool) = (server.to_string(), tool.to_string());
                match self.manager.call(&server, &tool, args_json).await {
                    Ok(json) => CallToolResult::success(vec![Content::text(json)]),
                    Err(e) => CallToolResult::error(vec![Content::text(e.to_string())]),
                }
            }
            None => CallToolResult::error(vec![Content::text(format!("unknown tool `{name}`"))]),
        }
    }
}

impl ServerHandler for GatewayServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(self.instructions.clone());
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut tools = Self::meta_tools();
        if self.passthrough {
            for t in self.manager.registry().tools() {
                let desc = if t.description.is_empty() {
                    format!("(via {})", t.server)
                } else {
                    format!("{} (via {})", t.description, t.server)
                };
                tools.push(Tool::new(
                    t.namespaced.clone(),
                    desc,
                    schema(t.input_schema.clone()),
                ));
            }
        }
        Ok(ListToolsResult::with_all_items(tools))
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
        let name = request.name.as_ref();

        let result = match name {
            "search" => self.handle_search(&args).await,
            "read_tool_docs" => self.handle_read_tool_docs(&args),
            "execute" => self.handle_execute(&args).await,
            other if self.passthrough => self.handle_passthrough(other, &args.to_string()).await,
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown tool `{other}`"),
                    None,
                ));
            }
        };
        Ok(result)
    }
}

/// Coerce a JSON value into an object map (for tool input schemas).
fn schema(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    match v {
        serde_json::Value::Object(m) => m,
