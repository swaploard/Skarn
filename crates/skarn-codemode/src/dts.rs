//! Generate a TypeScript declaration file describing the `skarn` API and the
//! available downstream tools, so the LLM can author scripts against real types.

use crate::bridge::ToolDescriptor;

/// Produce a `.d.ts` for the given tool manifest.
///
/// The output documents the global `skarn` object plus a typed `skarn.server()`
/// surface per downstream server, with each tool's description carried through
/// as a JSDoc comment.
