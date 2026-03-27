//! Declarative, YAML-driven token compression for noisy shell output.
//!
//! When an AI agent runs `cargo test` or `npm install`, the raw stdout/stderr
//! it feeds back into the model is mostly noise: progress bars, "Compiling …"
//! spam, thousands of passing-test confirmations. [`Compressor`] strips that
//! down to the semantic signal — errors, warnings, failures — typically cutting
//! 70–90% of the tokens while *guaranteeing* error lines survive truncation.
//!
//! ```
//! use skarn_common::CommandSpec;
//! use skarn_compress::Compressor;
//!
//! let c = Compressor::builtin();
//! let spec = CommandSpec::new("cargo", ["test"]);
//! let out = c.compress(&spec, b"   Compiling foo v0.1.0\nerror[E0001]: boom\n", b"");
//! assert!(out.text.contains("error[E0001]"));
//! assert!(!out.text.contains("Compiling"));
//! ```

mod engine;
mod rules;

use std::collections::BTreeMap;

use skarn_common::{CommandSpec, Savings, estimate_tokens};

pub use engine::CompiledProfile;
pub use rules::{ProfilePatch, RuleSet, Rules};

/// The result of compressing a command's output.
#[derive(Clone, Debug)]
pub struct Compressed {
    /// The compressed, agent-ready text (stdout, then stderr if non-empty).
    pub text: String,
    /// Token estimate before/after.
    pub savings: Savings,
    /// Total input lines across both streams.
    pub original_lines: usize,
    /// Lines retained after filtering (before truncation markers).
    pub kept_lines: usize,
    /// The profile (tool name) that was applied, or `"default"`.
    pub profile: String,
}

/// A reusable compressor with all profiles pre-compiled.
pub struct Compressor {
    ruleset: RuleSet,
    compiled: BTreeMap<String, CompiledProfile>,
    default: CompiledProfile,
    /// Any regexes that failed to compile, surfaced for diagnostics.
    pub warnings: Vec<String>,
}

impl Compressor {
    /// Build a compressor from the built-in rules.
    pub fn builtin() -> Compressor {
        Compressor::new(RuleSet::builtin())
    }

    /// Build a compressor from a custom rule set, pre-compiling every profile.
    pub fn new(ruleset: RuleSet) -> Compressor {
        let mut warnings = Vec::new();
        let (default, errs) = CompiledProfile::compile(&ruleset.default);
        warnings.extend(errs);

        let mut compiled = BTreeMap::new();
        for tool in ruleset.profiles.keys() {
            let rules = ruleset.resolve(tool);
            let (prof, errs) = CompiledProfile::compile(&rules);
            warnings.extend(errs);
            compiled.insert(tool.clone(), prof);
        }

        Compressor {
            ruleset,
            compiled,
            default,
            warnings,
        }
    }

    /// The rule set backing this compressor.
    pub fn ruleset(&self) -> &RuleSet {
        &self.ruleset
    }

    /// Compress a command's `stdout` and `stderr`.
    pub fn compress(&self, spec: &CommandSpec, stdout: &[u8], stderr: &[u8]) -> Compressed {
        let tool = spec.tool_name();
        let (profile_name, profile) = match self.compiled.get(&tool) {
            Some(p) => (tool.clone(), p),
            None => ("default".to_string(), &self.default),
        };

        let out = profile.run(stdout);
        let err = profile.run(stderr);

        let mut text = out.text.clone();
        if !err.text.trim().is_empty() {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            text.push_str("─── stderr ───\n");
            text.push_str(&err.text);
        }

        // Measure savings against the raw (UTF-8) input so the numbers reflect
        // what the agent would otherwise have paid for.
        let raw_before = format!(
            "{}{}",
            String::from_utf8_lossy(stdout),
            String::from_utf8_lossy(stderr)
        );
        let savings = Savings {
            before: estimate_tokens(&raw_before),
            after: estimate_tokens(&text),
        };

        Compressed {
            text,
            savings,
            original_lines: out.original_lines + err.original_lines,
            kept_lines: out.kept_lines + err.kept_lines,
            profile: profile_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
