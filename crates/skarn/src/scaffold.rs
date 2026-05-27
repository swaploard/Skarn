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
