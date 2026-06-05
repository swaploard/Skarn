//! Static templates emitted by `skarn init` and `skarn hook`.

/// A starter `skarn.toml`.
pub const SKARN_TOML_TEMPLATE: &str = r#"# Skarn gateway configuration.
#
# `skarn serve` reads this file, connects to each downstream MCP server below,
# and exposes them to your AI client through the Code Mode meta-tools
# (`search`, `read_tool_docs`, `execute`).

[gateway]
# Also expose the namespaced downstream tools directly (e.g. `fs__read_file`),
# in addition to the Code Mode meta-tools. Leave false to get the full
# token-saving benefit of Code Mode.
passthrough = false

# ---------------------------------------------------------------------------
# Downstream MCP servers. Each [servers.<alias>] is launched as a child process
# and aggregated. The alias becomes the namespace prefix (alias__tool).
# ---------------------------------------------------------------------------

# A local filesystem server (uncomment and adjust):
# [servers.fs]
# transport = "stdio"
# command = "npx"
# args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/project"]

# A GitHub server:
# [servers.github]
# transport = "stdio"
# command = "npx"
# args = ["-y", "@modelcontextprotocol/server-github"]
# env = { GITHUB_TOKEN = "ghp_..." }

# A remote server over Streamable HTTP. Prefer `auth_bearer_env` so the token is
# read from the environment at startup instead of being stored in this file:
# [servers.remote]
# transport = "http"
# url = "https://api.example.com/mcp"
# auth_bearer_env = "EXAMPLE_API_TOKEN"
# headers = { X-Org = "acme" }
"#;

/// Printed by `skarn init` after writing the config.
pub const INTEGRATION_SNIPPETS: &str = r#"Next steps
==========

