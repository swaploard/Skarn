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
