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
