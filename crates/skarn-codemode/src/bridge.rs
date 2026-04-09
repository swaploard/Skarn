//! The bridge between a Code Mode isolate and the real MCP servers.
//!
//! The isolate is hermetic: it has no filesystem, no network, no `fetch`. Its
//! *only* way to affect the outside world is the [`ToolBridge`] — a small set of
//! async operations the host fulfils. In production these are forwarded over a
//! pipe to the parent gateway (which holds the MCP clients and credentials); in
//! tests an in-process implementation is used. Either way, credentials, file
//! paths, and connection state never enter the sandbox.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A single tool exposed by a downstream server, as the isolate sees it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDescriptor {
    /// The downstream server alias the tool belongs to.
    pub server: String,
    /// The (un-namespaced) tool name on that server.
    pub name: String,
    /// Human-readable description (often used as a JSDoc comment in `.d.ts`).
    #[serde(default)]
    pub description: String,
    /// The tool's JSON Schema for its arguments.
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

/// The host operations a Code Mode script can invoke.
///
/// All methods return JSON-encoded strings (or an error string) so that no
/// `serde`↔JS value conversion is needed across the FFI boundary — the injected
/// `skarn` JS shim does `JSON.parse`/`JSON.stringify`.
///
/// The trait is `?Send`: the QuickJS isolate is single-threaded, and the
/// production bridge wraps a (single-threaded) local MCP client, so requiring
/// `Send` would be both unnecessary and impossible.
#[async_trait(?Send)]
pub trait ToolBridge {
    /// Call `tool` on `server` with `args_json` (a JSON object). Returns the
    /// tool result as a JSON string, or an error message.
    async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        args_json: &str,
    ) -> std::result::Result<String, String>;

