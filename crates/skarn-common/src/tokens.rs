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
