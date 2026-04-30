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
