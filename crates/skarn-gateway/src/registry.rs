//! The namespaced tool registry and the search index used for discovery.

use serde::Serialize;
use skarn_codemode::ToolDescriptor;

/// One downstream tool, with its gateway-facing namespaced name.
#[derive(Clone, Debug)]
pub struct NamespacedTool {
    /// Downstream server alias.
    pub server: String,
    /// Original tool name on that server.
