//! Shared primitives for the Skarn workspace.
//!
//! This crate is intentionally tiny and dependency-light. It holds the handful
//! of types that are used across the sandbox, compression, code-mode, gateway,
//! and CLI crates so they do not have to depend on one another for trivial
//! data structures.

mod command;
mod error;
mod tokens;

pub use command::{CommandSpec, classify_program};
pub use error::{Error, Result};
pub use tokens::{Savings, estimate_tokens};
