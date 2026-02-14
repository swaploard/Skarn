//! The line-processing pipeline that turns a noisy stream into a compact one.

use regex::RegexSet;

use crate::rules::Rules;

/// A compiled, ready-to-run version of [`Rules`].
pub struct CompiledProfile {
    strip_ansi: bool,
    collapse_carriage_returns: bool,
    collapse_blank_lines: bool,
