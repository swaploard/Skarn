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
    pub args: Vec<String>,
    /// Working directory. `None` means "inherit the parent's".
    pub cwd: Option<PathBuf>,
    /// Extra environment variables to set (added to the inherited environment).
    pub env: Vec<(String, String)>,
}

impl CommandSpec {
    /// Build a spec from a program name and an iterator of arguments.
    pub fn new<P, A, S>(program: P, args: A) -> Self
    where
        P: Into<String>,
        A: IntoIterator<Item = S>,
        S: Into<String>,
