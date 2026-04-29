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
