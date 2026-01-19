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
    {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            cwd: None,
            env: Vec::new(),
        }
    }

    /// Parse a spec from an argv-style slice (`["cargo", "test", "--quiet"]`).
    ///
    /// Returns `None` if the slice is empty.
    pub fn from_argv<S: AsRef<str>>(argv: &[S]) -> Option<Self> {
        let (program, rest) = argv.split_first()?;
        Some(Self {
            program: program.as_ref().to_string(),
            args: rest.iter().map(|s| s.as_ref().to_string()).collect(),
            cwd: None,
            env: Vec::new(),
        })
    }

    /// Set the working directory.
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Add an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Render the command back to a human-readable shell-ish string (for logs).
    pub fn display(&self) -> String {
        let mut out = self.program.clone();
        for a in &self.args {
            out.push(' ');
            if a.contains(char::is_whitespace) {
                out.push('"');
                out.push_str(a);
                out.push('"');
            } else {
                out.push_str(a);
            }
        }
        out
    }

    /// The base name of [`Self::program`], lowercased, with any path and a
    /// trailing `.exe` removed. Used to pick a compression profile.
    pub fn tool_name(&self) -> String {
        let base = self
            .program
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&self.program);
        base.trim_end_matches(".exe").to_ascii_lowercase()
    }
}

/// A coarse classification of a program, used to select compression heuristics
/// and default sandbox hints. This is best-effort and purely advisory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramClass {
    /// Rust build / test tooling (`cargo`, `rustc`).
    Rust,
    /// Python build / test tooling (`python`, `pytest`, `pip`, `uv`).
    Python,
    /// JavaScript / Node tooling (`npm`, `pnpm`, `yarn`, `node`, `bun`).
    Node,
    /// Version control (`git`, `jj`).
    Vcs,
    /// Filesystem listing (`ls`, `tree`, `find`).
    Listing,
    /// Search (`grep`, `rg`, `ag`).
    Search,
    /// Anything else.
    Other,
}

/// Classify a program by its (base) name.
pub fn classify_program(tool_name: &str) -> ProgramClass {
    match tool_name {
        "cargo" | "rustc" | "rustup" | "clippy-driver" => ProgramClass::Rust,
        "python" | "python3" | "pytest" | "pip" | "pip3" | "uv" | "poetry" | "ruff" => {
            ProgramClass::Python
        }
        "npm" | "pnpm" | "yarn" | "node" | "bun" | "npx" | "tsc" | "vite" | "webpack" => {
            ProgramClass::Node
        }
        "git" | "jj" | "hg" => ProgramClass::Vcs,
        "ls" | "tree" | "find" | "fd" | "exa" | "eza" => ProgramClass::Listing,
        "grep" | "rg" | "ag" | "ack" => ProgramClass::Search,
        _ => ProgramClass::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_round_trip() {
        let spec = CommandSpec::from_argv(&["cargo", "test", "--quiet"]).unwrap();
        assert_eq!(spec.program, "cargo");
        assert_eq!(spec.args, vec!["test", "--quiet"]);
        assert_eq!(spec.display(), "cargo test --quiet");
    }

    #[test]
    fn tool_name_strips_path_and_exe() {
        let spec = CommandSpec::new("/usr/bin/Cargo.exe", ["x"]);
        assert_eq!(spec.tool_name(), "cargo");
        assert_eq!(classify_program(&spec.tool_name()), ProgramClass::Rust);
    }

    #[test]
    fn empty_argv_is_none() {
