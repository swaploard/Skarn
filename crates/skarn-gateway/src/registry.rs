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
                let namespaced = format!("{server}{separator}{}", d.name);
                tools.push(NamespacedTool {
                    server: server.clone(),
                    tool: d.name,
                    namespaced,
                    description: d.description,
                    input_schema: d.input_schema,
                });
            }
        }
        Registry { tools }
    }

    pub fn tools(&self) -> &[NamespacedTool] {
        &self.tools
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// The distinct downstream server names, first-seen order.
    pub fn server_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for t in &self.tools {
            if !names.contains(&t.server) {
                names.push(t.server.clone());
            }
        }
        names
    }

    /// Resolve a namespaced name back to `(server, tool)`.
    pub fn resolve(&self, namespaced: &str) -> Option<(&str, &str)> {
        self.tools
            .iter()
            .find(|t| t.namespaced == namespaced)
            .map(|t| (t.server.as_str(), t.tool.as_str()))
    }

    /// Convert to Code Mode tool descriptors (for `.d.ts` + `listTools`).
    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        self.tools.iter().map(|t| t.descriptor()).collect()
    }

    /// Rank tools by relevance to `query`. Returns up to `limit` matches.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let terms: Vec<String> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .map(|t| t.to_ascii_lowercase())
            .collect();

        let mut scored: Vec<(i32, &NamespacedTool)> = self
            .tools
            .iter()
