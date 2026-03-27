//! The bridge between a Code Mode isolate and the real MCP servers.
//!
//! The isolate is hermetic: it has no filesystem, no network, no `fetch`. Its
//! *only* way to affect the outside world is the [`ToolBridge`] — a small set of
//! async operations the host fulfils. In production these are forwarded over a
//! pipe to the parent gateway (which holds the MCP clients and credentials); in
//! tests an in-process implementation is used. Either way, credentials, file
//! paths, and connection state never enter the sandbox.

use async_trait::async_trait;
