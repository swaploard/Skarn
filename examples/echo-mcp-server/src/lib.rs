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
