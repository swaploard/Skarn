//! The namespaced tool registry and the search index used for discovery.

use serde::Serialize;
use skarn_codemode::ToolDescriptor;

/// One downstream tool, with its gateway-facing namespaced name.
#[derive(Clone, Debug)]
pub struct NamespacedTool {
    /// Downstream server alias.
    pub server: String,
    /// Original tool name on that server.
    pub tool: String,
    /// The namespaced name the gateway exposes (`server__tool`).
    pub namespaced: String,
    /// Description (may be empty).
    pub description: String,
    /// JSON Schema of the tool's arguments.
    pub input_schema: serde_json::Value,
}

impl NamespacedTool {
    pub fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            server: self.server.clone(),
            name: self.tool.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
        }
    }
}

/// An immutable snapshot of all downstream tools, with a reverse-routing map.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    tools: Vec<NamespacedTool>,
}

impl Registry {
    /// Build a registry from per-server tool lists, namespacing each tool.
    pub fn build(separator: &str, per_server: Vec<(String, Vec<ToolDescriptor>)>) -> Registry {
        let mut tools = Vec::new();
        for (server, descriptors) in per_server {
            for d in descriptors {
