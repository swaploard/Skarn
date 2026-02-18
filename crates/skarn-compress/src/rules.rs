//! The declarative rule model and the embedded built-in defaults.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The full configuration: a `default` rule set plus per-tool patches that
/// extend / override it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuleSet {
    /// Applied to every command unless a more specific profile matches.
    #[serde(default)]
    pub default: Rules,
    /// Keyed by tool name (see [`skarn_common::CommandSpec::tool_name`]).
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfilePatch>,
}

