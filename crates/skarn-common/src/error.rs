//! The shared error type for Skarn crates.

use std::fmt;

/// A convenient `Result` alias used across the workspace.
pub type Result<T> = std::result::Result<T, Error>;

/// The unified error type for Skarn.
///
/// Individual crates add their own variants via [`Error::Sandbox`],
/// [`Error::CodeMode`], etc., keeping a single error surface for the CLI while
/// still allowing each subsystem to be consumed independently.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A filesystem or process I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON (de)serialization error.
    #[error("json error: {0}")]
