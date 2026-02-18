//! The line-processing pipeline that turns a noisy stream into a compact one.

use regex::RegexSet;

use crate::rules::Rules;

/// A compiled, ready-to-run version of [`Rules`].
pub struct CompiledProfile {
    strip_ansi: bool,
    collapse_carriage_returns: bool,
    collapse_blank_lines: bool,
    dedupe_consecutive: bool,
    max_lines: usize,
    head_lines: usize,
    tail_lines: usize,
    max_rescued_lines: usize,
    drop: RegexSet,
    keep: RegexSet,
}

/// What a single stream's compression produced.
pub struct StreamResult {
    pub text: String,
    pub original_lines: usize,
    pub kept_lines: usize,
}

impl CompiledProfile {
    /// Compile rules. Invalid regexes are skipped (with the offending pattern
    /// reported in `errors`) rather than failing the whole compressor.
