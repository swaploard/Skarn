//! Connects to and aggregates downstream MCP servers.

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use async_trait::async_trait;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ReadResourceRequestParams, ResourceContents, Tool,
};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt};
use skarn_codemode::{ToolBridge, ToolDescriptor};
use skarn_common::{Error, Result};

use crate::config::{GatewayConfig, TransportConfig};
use crate::registry::Registry;

type JsonObject = serde_json::Map<String, serde_json::Value>;
type Client = RunningService<RoleClient, ()>;

/// Holds one MCP client per downstream server plus the namespaced registry.
pub struct DownstreamManager {
    clients: HashMap<String, Client>,
    registry: ArcSwap<Registry>,
    separator: String,
}

impl DownstreamManager {
    /// Connect to every enabled server in `config`, list its tools, and build
    /// the namespaced registry. Servers that fail to start are reported but do
    /// not abort the others.
    pub async fn connect(config: &GatewayConfig) -> Result<Self> {
        let separator = config.gateway.namespace_separator.clone();
        let mut clients = HashMap::new();
        let mut per_server: Vec<(String, Vec<ToolDescriptor>)> = Vec::new();

        for (alias, server) in config.enabled_servers() {
            match Self::connect_one(alias, &server.transport).await {
                Ok((client, descriptors)) => {
                    tracing::info!(
                        server = alias,
                        tools = descriptors.len(),
                        "connected downstream MCP server"
                    );
                    per_server.push((alias.clone(), descriptors));
                    clients.insert(alias.clone(), client);
                }
                Err(e) => {
                    tracing::error!(server = alias, error = %e, "failed to connect downstream MCP server");
                }
            }
        }

        let registry = Registry::build(&separator, per_server);
        Ok(Self {
            clients,
            registry: ArcSwap::from_pointee(registry),
            separator,
        })
    }

    async fn connect_one(
        alias: &str,
        transport: &TransportConfig,
    ) -> Result<(Client, Vec<ToolDescriptor>)> {
        // Both transports erase to `RunningService<RoleClient, ()>`, so the rest
        // of the manager is transport-agnostic.
        let client = match transport {
            TransportConfig::Stdio {
                command,
                args,
                env,
                cwd,
