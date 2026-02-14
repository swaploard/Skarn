//! Declarative, YAML-driven token compression for noisy shell output.
//!
//! When an AI agent runs `cargo test` or `npm install`, the raw stdout/stderr
//! it feeds back into the model is mostly noise: progress bars, "Compiling …"
//! spam, thousands of passing-test confirmations. [`Compressor`] strips that
//! down to the semantic signal — errors, warnings, failures — typically cutting
//! 70–90% of the tokens while *guaranteeing* error lines survive truncation.
//!
//! ```
