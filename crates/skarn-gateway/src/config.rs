//! Gateway configuration, parsed from `skarn.toml`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use skarn_common::{Error, Result};

/// Top-level gateway configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayConfig {
    /// Upstream-facing settings.
    pub gateway: GatewaySettings,
    /// Downstream MCP servers to aggregate, keyed by alias.
    pub servers: BTreeMap<String, ServerConfig>,
}

/// Settings for the server Skarn presents to the AI client.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewaySettings {
    /// Also expose the namespaced downstream tools directly (in addition to the
    /// `search`/`execute` meta-tools), for clients that don't use Code Mode.
    pub passthrough: bool,
    /// The character sequence joining `server` and `tool` into a namespaced
    /// name (e.g. `github__search`). Must match `[A-Za-z0-9_.-]`.
    pub namespace_separator: String,
    /// How `execute` scripts are isolated. See [`Isolation`].
    pub isolation: Isolation,
}

impl Default for GatewaySettings {
    fn default() -> Self {
        Self {
            passthrough: false,
            namespace_separator: "__".to_string(),
            isolation: Isolation::default(),
        }
    }
}

/// How `execute` runs Code Mode scripts.
///
/// Both modes run the script inside the hermetic QuickJS isolate (no filesystem,
/// network, or `fetch`). The worker adds a second layer: the isolate runs in a
/// dedicated child process that confines *itself* with the OS-native sandbox
/// before touching the script, so a hypothetical isolate escape still lands in a
/// kernel-confined process with no network and no workspace writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Isolation {
    /// Use the cross-process OS-sandboxed worker when a sandbox backend is
    /// available on this platform; otherwise fall back to in-process.
    #[default]
    Auto,
    /// Always use the cross-process OS-sandboxed worker. Errors if no sandbox
    /// backend is available (fail closed).
    Worker,
    /// Always run in-process (hermetic QuickJS isolate only, no OS sandbox).
    InProcess,
}

/// A single downstream MCP server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Whether this server is connected on startup.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// How to reach the server.
    #[serde(flatten)]
    pub transport: TransportConfig,
}

/// The transport for a downstream server.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum TransportConfig {
    /// Launch a child process and speak MCP over its stdio.
    Stdio {
        /// The program to run.
        command: String,
        /// Arguments.
        #[serde(default)]
        args: Vec<String>,
        /// Extra environment variables.
        #[serde(default)]
        env: BTreeMap<String, String>,
        /// Working directory.
        #[serde(default)]
        cwd: Option<PathBuf>,
    },
    /// Speak MCP over Streamable HTTP (the SSE response stream is handled
    /// internally by the transport).
    Http {
        /// The MCP endpoint URL, e.g. `https://api.example.com/mcp`.
        url: String,
        /// Bearer token, sent as `Authorization: Bearer <token>`. Prefer
        /// `auth_bearer_env` to keep secrets out of `skarn.toml`.
        #[serde(default)]
        auth_bearer: Option<String>,
        /// Name of an environment variable to read the bearer token from at
