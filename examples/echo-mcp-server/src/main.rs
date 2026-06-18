//! The `echo-mcp-server` binary: serve [`EchoServer`] over stdio.
//!
//! Used by Skarn's examples and integration tests as a real subprocess the
//! gateway can aggregate.

use echo_mcp_server::EchoServer;
use rmcp::ServiceExt;
use rmcp::transport::stdio;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logs to stderr so they never corrupt the stdio MCP channel.
