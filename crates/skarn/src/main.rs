//! `skarn` — the Skarn command-line interface.
//!
//! A single binary that is, depending on how you invoke it:
//!   * an MCP **gateway** (`skarn serve`) that aggregates downstream servers
//!     behind a Code Mode tool surface,
//!   * a **sandboxing, output-compressing shell wrapper** (`skarn run -- …`)
//!     designed to be dropped into an agent's PreToolUse hook,
//!   * a **Code Mode runner** (`skarn exec`) for trying scripts against your
//!     configured servers.

mod commands;
mod scaffold;

use clap::{Parser, Subcommand};

/// Skarn: a fast, OS-sandboxed MCP gateway with Code Mode and token compression.
#[derive(Parser, Debug)]
#[command(name = "skarn", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
