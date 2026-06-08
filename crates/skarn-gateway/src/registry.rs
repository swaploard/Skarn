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
            .filter_map(|t| {
                let score = score_tool(t, &terms);
                if score > 0 { Some((score, t)) } else { None }
            })
            .collect();

        // Highest score first; stable by namespaced name for determinism.
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.namespaced.cmp(&b.1.namespaced))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(score, t)| SearchHit {
                server: t.server.clone(),
                tool: t.tool.clone(),
                namespaced: t.namespaced.clone(),
                description: t.description.clone(),
                score,
            })
            .collect()
    }
}

/// A search result.
#[derive(Clone, Debug, Serialize)]
pub struct SearchHit {
    pub server: String,
    pub tool: String,
    pub namespaced: String,
    pub description: String,
    pub score: i32,
}

fn score_tool(tool: &NamespacedTool, terms: &[String]) -> i32 {
    if terms.is_empty() {
        return 1; // empty query matches everything weakly
    }
    let name = tool.tool.to_ascii_lowercase();
    let server = tool.server.to_ascii_lowercase();
    let desc = tool.description.to_ascii_lowercase();
    let mut score = 0;
    for term in terms {
        if name == *term {
            score += 10;
        } else if name.contains(term) {
            score += 6;
        }
        if server.contains(term) {
            score += 3;
        }
        if desc.contains(term) {
            score += 2;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(name: &str, description: &str) -> ToolDescriptor {
        ToolDescriptor {
            server: String::new(),
            name: name.to_string(),
            description: description.to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        }
    }

    fn registry() -> Registry {
        Registry::build(
            "__",
            vec![
                (
                    "github".to_string(),
                    vec![
                        desc("search_issues", "Search GitHub issues and pull requests"),
                        desc("create_issue", "Open a new issue"),
                    ],
                ),
                (
                    "db".to_string(),
                    vec![desc("query", "Run a SQL query against the database")],
                ),
            ],
        )
    }

    #[test]
    fn namespacing_and_resolution() {
        let r = registry();
        assert_eq!(r.len(), 3);
        assert_eq!(
            r.resolve("github__search_issues"),
            Some(("github", "search_issues"))
        );
        assert_eq!(r.resolve("db__query"), Some(("db", "query")));
        assert_eq!(r.resolve("nope__nope"), None);
        assert_eq!(r.server_names(), vec!["github", "db"]);
    }

    #[test]
    fn search_ranks_by_relevance() {
        let r = registry();
        let hits = r.search("issue", 10);
        assert!(!hits.is_empty());
        // The tool literally named with "issue" / describing issues should rank.
        assert!(hits.iter().any(|h| h.tool == "search_issues"));
        // A SQL tool should not match "issue".
        assert!(!hits.iter().any(|h| h.tool == "query"));
    }

    #[test]
    fn search_query_matches_sql() {
        let r = registry();
