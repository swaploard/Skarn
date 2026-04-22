//! The MCP server Skarn presents to the AI client.
//!
//! Instead of forwarding hundreds of downstream tool schemas, it exposes a tiny,
//! constant surface — `search`, `read_tool_docs`, and `execute` — plus, if
//! configured, the namespaced downstream tools in passthrough mode.
