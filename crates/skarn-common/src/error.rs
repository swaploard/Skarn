//! The shared error type for Skarn crates.

use std::fmt;

/// A convenient `Result` alias used across the workspace.
pub type Result<T> = std::result::Result<T, Error>;

/// The unified error type for Skarn.
///
/// Individual crates add their own variants via [`Error::Sandbox`],
/// [`Error::CodeMode`], etc., keeping a single error surface for the CLI while
/// still allowing each subsystem to be consumed independently.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A filesystem or process I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON (de)serialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid or unsupported configuration.
    #[error("configuration error: {0}")]
    Config(String),

    /// The OS-native sandbox layer rejected or could not apply a policy.
    #[error("sandbox error: {0}")]
    Sandbox(String),

    /// The current platform / kernel does not support the requested sandbox and
    /// `fail_closed` was set, so execution was refused.
    #[error("sandbox unsupported on this platform: {0}")]
    SandboxUnsupported(String),

    /// A Code Mode script was rejected by static validation.
    #[error("code-mode validation rejected script: {0}")]
    CodeModeRejected(String),

    /// A Code Mode script failed at runtime (threw, timed out, or hit a limit).
    #[error("code-mode runtime error: {0}")]
    CodeMode(String),

    /// A downstream MCP server or transport error.
    #[error("mcp error: {0}")]
    Mcp(String),

    /// A tool was requested that the gateway does not know about.
    #[error("unknown tool: {0}")]
    UnknownTool(String),

    /// A catch-all for anything that does not fit the variants above.
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Construct a [`Error::Other`] from anything string-like.
    pub fn other(msg: impl fmt::Display) -> Self {
        Error::Other(msg.to_string())
    }

    /// Construct a [`Error::Config`] from anything string-like.
    pub fn config(msg: impl fmt::Display) -> Self {
        Error::Config(msg.to_string())
    }

    /// Construct a [`Error::Sandbox`] from anything string-like.
    pub fn sandbox(msg: impl fmt::Display) -> Self {
        Error::Sandbox(msg.to_string())
    }
