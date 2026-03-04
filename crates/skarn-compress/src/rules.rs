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

/// A complete set of compression knobs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rules {
    /// Remove ANSI escape sequences (colors, cursor moves).
    #[serde(default = "yes")]
    pub strip_ansi: bool,
    /// Collapse carriage-return "progress bar" redraws to their final frame.
    #[serde(default = "yes")]
    pub collapse_carriage_returns: bool,
    /// Collapse runs of blank lines to a single blank line.
    #[serde(default = "yes")]
    pub collapse_blank_lines: bool,
    /// Collapse runs of identical adjacent lines to `line  (×N)`.
    #[serde(default = "yes")]
    pub dedupe_consecutive: bool,
    /// If the stream exceeds this many lines, truncate it (keeping head, tail,
    /// and any lines matching `keep`).
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
    /// Lines to keep from the start when truncating.
    #[serde(default = "default_head")]
    pub head_lines: usize,
    /// Lines to keep from the end when truncating.
    #[serde(default = "default_tail")]
    pub tail_lines: usize,
    /// At most this many "important" (keep-matching) lines are rescued from the
    /// elided middle when truncating.
    #[serde(default = "default_max_rescued")]
    pub max_rescued_lines: usize,
    /// Regexes for lines to drop (noise).
    #[serde(default)]
    pub drop: Vec<String>,
    /// Regexes for lines to always keep (errors, failures). `keep` beats `drop`.
    #[serde(default)]
    pub keep: Vec<String>,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            strip_ansi: true,
            collapse_carriage_returns: true,
            collapse_blank_lines: true,
            dedupe_consecutive: true,
            max_lines: default_max_lines(),
            head_lines: default_head(),
            tail_lines: default_tail(),
