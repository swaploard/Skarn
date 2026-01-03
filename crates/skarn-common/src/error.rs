//! The shared error type for Skarn crates.

use std::fmt;

/// A convenient `Result` alias used across the workspace.
pub type Result<T> = std::result::Result<T, Error>;

