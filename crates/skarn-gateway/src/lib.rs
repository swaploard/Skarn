//! The Skarn gateway: aggregate downstream MCP servers behind a tiny Code
//! Mode tool surface (`search` / `read_tool_docs` / `execute`).
//!
//! ```no_run
//! # async fn run() -> skarn_common::Result<()> {
//! use skarn_codemode::ExecLimits;
//! use skarn_gateway::{GatewayConfig, build_server, serve_stdio};
//!
//! let config = GatewayConfig::load("skarn.toml")?;
//! let server = build_server(&config, ExecLimits::default()).await?;
//! serve_stdio(server).await?;
//! # Ok(()) }
//! ```
//!
//! Runs on a normal multi-threaded Tokio runtime. The `!Send` QuickJS isolate is
//! confined to a dedicated thread (see [`execute`]) and bridged back over
//! channels, so the MCP clients keep a stable reactor for their whole lifetime.

mod config;
mod downstream;
mod execute;
mod registry;
mod server;
pub mod worker_proto;

use std::sync::Arc;

use rmcp::ServiceExt;
use skarn_codemode::ExecLimits;
use skarn_common::{Error, Result};

pub use config::{GatewayConfig, GatewaySettings, Isolation, ServerConfig, TransportConfig};
pub use downstream::{DownstreamManager, GatewayBridge};
pub use execute::run_worker_job;
pub use registry::{NamespacedTool, Registry, SearchHit};
pub use server::GatewayServer;

/// Connect to the configured downstream servers and build a ready-to-serve
/// gateway handler.
pub async fn build_server(config: &GatewayConfig, limits: ExecLimits) -> Result<GatewayServer> {
    let manager = Arc::new(DownstreamManager::connect(config).await?);
    let descriptors = manager.registry().descriptors();
    let dts = skarn_codemode::generate_dts(&descriptors);
    let instructions = build_instructions(&dts, &manager);

    Ok(GatewayServer::new(
        manager,
