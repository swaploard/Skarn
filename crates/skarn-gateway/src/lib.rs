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
