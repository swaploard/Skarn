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
