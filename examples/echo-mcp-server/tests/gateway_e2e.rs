//! End-to-end test of the whole Skarn stack.
//!
//! The gateway launches `echo-mcp-server` as a real stdio subprocess, lists and
//! namespaces its tools, and a Code Mode script calls those tools through the
//! `skarn` bridge — exercising: downstream stdio transport, tool aggregation,
//! the QuickJS isolate, the host bridge, and result extraction. We also drive
//! the gateway's *upstream* MCP surface (`search` / `execute`) with an
//! in-memory client.

use std::collections::BTreeMap;

use skarn_codemode::ExecLimits;
use skarn_gateway::{GatewayConfig, GatewaySettings, Isolation, ServerConfig, TransportConfig};

const ECHO_BIN: &str = env!("CARGO_BIN_EXE_echo-mcp-server");

fn config() -> GatewayConfig {
    let mut servers = BTreeMap::new();
    servers.insert(
        "echo".to_string(),
        ServerConfig {
            enabled: true,
            transport: TransportConfig::Stdio {
                command: ECHO_BIN.to_string(),
                args: vec![],
                env: BTreeMap::new(),
                cwd: None,
            },
        },
    );
