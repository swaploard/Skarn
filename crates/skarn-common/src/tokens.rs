//! Token estimation.
//!
//! By default we use a cheap, dependency-free heuristic (≈4 bytes per token,
//! the well-known rule of thumb for English + code with the cl100k/o200k family
//! of tokenizers). Enable the `tiktoken` feature for exact counts.

use serde::{Deserialize, Serialize};

/// Estimate the number of LLM tokens in `text`.
///
/// The default implementation is intentionally fast and allocation-free: it is
/// used on every compressed command, so it must be cheap. With the `tiktoken`
/// feature enabled it instead uses the `o200k_base` tokenizer for exact counts.
pub fn estimate_tokens(text: &str) -> usize {
    #[cfg(feature = "tiktoken")]
    {
        token_count_tiktoken(text)
    }
    #[cfg(not(feature = "tiktoken"))]
    {
        heuristic_tokens(text)
    }
}

/// The byte-based heuristic, always available (also used as the unit-tested
/// reference implementation).
pub fn heuristic_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    // ~4 bytes per token, but never report fewer than 1 for non-empty input,
    // and add a small per-line surcharge because newlines/indentation tend to
    // tokenize less efficiently than prose.
    let bytes = text.len();
    let lines = text.bytes().filter(|&b| b == b'\n').count();
    (bytes / 4).max(1) + lines / 8
}

#[cfg(feature = "tiktoken")]
fn token_count_tiktoken(text: &str) -> usize {
    use std::sync::OnceLock;
    use tiktoken_rs::{CoreBPE, o200k_base};
    static BPE: OnceLock<CoreBPE> = OnceLock::new();
    let bpe = BPE.get_or_init(|| o200k_base().expect("o200k_base tokenizer loads"));
    bpe.encode_ordinary(text).len()
}

/// A before/after token comparison produced by the compression layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Savings {
    /// Estimated tokens before compression.
    pub before: usize,
    /// Estimated tokens after compression.
    pub after: usize,
}

impl Savings {
    /// Construct savings by estimating tokens for both strings.
    pub fn measure(before: &str, after: &str) -> Self {
        Self {
            before: estimate_tokens(before),
            after: estimate_tokens(after),
        }
    }

    /// Tokens saved (`before - after`, clamped at zero).
    pub fn saved(&self) -> usize {
        self.before.saturating_sub(self.after)
    }

    /// The reduction as a fraction in `0.0..=1.0`. Returns `0.0` if `before` is
    /// zero or if compression somehow grew the output.
    pub fn ratio(&self) -> f64 {
        if self.before == 0 || self.after >= self.before {
            return 0.0;
        }
        self.saved() as f64 / self.before as f64
    }

    /// The reduction as a rounded percentage (e.g. `90`).
    pub fn percent(&self) -> u8 {
        (self.ratio() * 100.0).round() as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        assert_eq!(heuristic_tokens(""), 0);
    }

    #[test]
    fn non_empty_is_at_least_one() {
        assert_eq!(heuristic_tokens("a"), 1);
    }

    #[test]
    fn savings_ratio_and_percent() {
        let s = Savings {
            before: 1000,
            after: 100,
        };
        assert_eq!(s.saved(), 900);
        assert!((s.ratio() - 0.9).abs() < 1e-9);
        assert_eq!(s.percent(), 90);
    }

    #[test]
    fn savings_never_negative() {
