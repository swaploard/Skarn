//! Token estimation.
//!
//! By default we use a cheap, dependency-free heuristic (≈4 bytes per token,
//! the well-known rule of thumb for English + code with the cl100k/o200k family
//! of tokenizers). Enable the `tiktoken` feature for exact counts.

use serde::{Deserialize, Serialize};

