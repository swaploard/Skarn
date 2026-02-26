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
    pub fn compile(rules: &Rules) -> (CompiledProfile, Vec<String>) {
        let (drop, mut errors) = compile_set(&rules.drop);
        let (keep, more) = compile_set(&rules.keep);
        errors.extend(more);
        (
            CompiledProfile {
                strip_ansi: rules.strip_ansi,
                collapse_carriage_returns: rules.collapse_carriage_returns,
                collapse_blank_lines: rules.collapse_blank_lines,
                dedupe_consecutive: rules.dedupe_consecutive,
                max_lines: rules.max_lines,
                head_lines: rules.head_lines,
                tail_lines: rules.tail_lines,
                max_rescued_lines: rules.max_rescued_lines,
                drop,
                keep,
            },
            errors,
        )
    }

    /// Run the pipeline over one stream (stdout or stderr).
    pub fn run(&self, raw: &[u8]) -> StreamResult {
        // 1. Bytes -> text. We do NOT strip ANSI yet: the ANSI stripper also
        //    consumes carriage returns, which would defeat the progress-bar
        //    collapse below. CR handling first, ANSI strip per line after.
        let text = String::from_utf8_lossy(raw).into_owned();
