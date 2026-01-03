//! A serializable description of a command to run.
//!
//! [`CommandSpec`] is the lingua franca between the CLI (which parses what the
//! user / agent wants to run), the sandbox (which decides how to confine it),
//! and the compression layer (which decides how to filter its output).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

