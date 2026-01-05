//! A serializable description of a command to run.
//!
//! [`CommandSpec`] is the lingua franca between the CLI (which parses what the
//! user / agent wants to run), the sandbox (which decides how to confine it),
//! and the compression layer (which decides how to filter its output).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A fully-resolved command invocation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    /// The program to execute (looked up on `PATH` unless absolute).
    pub program: String,
    /// Arguments passed to the program, not including `program` itself.
