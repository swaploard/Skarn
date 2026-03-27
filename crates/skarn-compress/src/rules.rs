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
            max_rescued_lines: default_max_rescued(),
            drop: Vec::new(),
            keep: Vec::new(),
        }
    }
}

/// A per-tool override. Scalar fields are `Option` so "unset" inherits from
/// `default`; `drop`/`keep` are additive (appended to the default lists).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProfilePatch {
    #[serde(default)]
    pub strip_ansi: Option<bool>,
    #[serde(default)]
    pub collapse_carriage_returns: Option<bool>,
    #[serde(default)]
    pub collapse_blank_lines: Option<bool>,
    #[serde(default)]
    pub dedupe_consecutive: Option<bool>,
    #[serde(default)]
    pub max_lines: Option<usize>,
    #[serde(default)]
    pub head_lines: Option<usize>,
    #[serde(default)]
    pub tail_lines: Option<usize>,
    #[serde(default)]
    pub max_rescued_lines: Option<usize>,
    #[serde(default)]
    pub drop: Vec<String>,
    #[serde(default)]
    pub keep: Vec<String>,
}

impl RuleSet {
    /// Resolve the effective [`Rules`] for a tool by layering its patch (if any)
    /// over the default.
    pub fn resolve(&self, tool: &str) -> Rules {
        let mut r = self.default.clone();
        if let Some(p) = self.profiles.get(tool) {
            if let Some(v) = p.strip_ansi {
                r.strip_ansi = v;
            }
            if let Some(v) = p.collapse_carriage_returns {
                r.collapse_carriage_returns = v;
            }
            if let Some(v) = p.collapse_blank_lines {
                r.collapse_blank_lines = v;
            }
            if let Some(v) = p.dedupe_consecutive {
                r.dedupe_consecutive = v;
            }
            if let Some(v) = p.max_lines {
                r.max_lines = v;
            }
            if let Some(v) = p.head_lines {
                r.head_lines = v;
            }
            if let Some(v) = p.tail_lines {
                r.tail_lines = v;
            }
            if let Some(v) = p.max_rescued_lines {
                r.max_rescued_lines = v;
            }
            r.drop.extend(p.drop.iter().cloned());
            r.keep.extend(p.keep.iter().cloned());
        }
        r
    }

    /// Load the built-in default rule set (embedded at compile time).
    pub fn builtin() -> RuleSet {
        serde_yaml_ng::from_str(BUILTIN_RULES_YAML).expect("built-in rules YAML is valid")
    }

    /// Parse a rule set from YAML.
    pub fn from_yaml(s: &str) -> Result<RuleSet, String> {
        serde_yaml_ng::from_str(s).map_err(|e| e.to_string())
    }

    /// Merge another rule set into this one: `other`'s default replaces nothing
    /// scalar-wise but its profiles override/add to ours (user overrides win).
