//! Static templates emitted by `skarn init` and `skarn hook`.

/// A starter `skarn.toml`.
pub const SKARN_TOML_TEMPLATE: &str = r#"# Skarn gateway configuration.
#
# `skarn serve` reads this file, connects to each downstream MCP server below,
# and exposes them to your AI client through the Code Mode meta-tools
# (`search`, `read_tool_docs`, `execute`).

