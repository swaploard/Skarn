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

    /// Increase log verbosity (also honors `RUST_LOG`). Logs go to stderr.
    #[arg(long, short, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the MCP gateway (stdio transport) for an AI client to connect to.
    Serve(commands::ServeArgs),
    /// Run a shell command inside an OS-native sandbox and compress its output.
    Run(commands::RunArgs),
    /// Execute a Code Mode script against the configured downstream servers.
    Exec(commands::ExecArgs),
    /// Report the active sandbox backend and subsystem status.
    Doctor,
    /// Scaffold an `skarn.toml` and print client integration snippets.
    Init(commands::InitArgs),
    /// Print a Claude Code PreToolUse hook that routes shell commands through skarn.
    Hook,
    /// Internal: the OS-sandboxed Code Mode worker (driven by `skarn serve`).
    /// Reads its job from stdin; not intended for direct use.
    #[command(name = "__worker", hide = true)]
    Worker,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

