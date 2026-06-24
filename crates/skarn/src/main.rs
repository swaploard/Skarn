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
pub struct Cli {
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
    /// Print a shell completion script for the given shell.
    Completions(commands::CompletionsArgs),
    /// Internal: the OS-sandboxed Code Mode worker (driven by `skarn serve`).
    /// Reads its job from stdin; not intended for direct use.
    #[command(name = "__worker", hide = true)]
    Worker,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match cli.command {
        // Synchronous commands. `run` and `__worker` must stay single-threaded
        // until they apply the sandbox so the self-/post-fork application is safe.
        Command::Run(args) => commands::run(args),
        Command::Doctor => commands::doctor(),
        Command::Init(args) => commands::init(args),
        Command::Hook => commands::hook(),
        Command::Worker => commands::worker(),
        Command::Completions(args) => commands::completions(args),
        // Async commands run on a normal multi-threaded runtime; the gateway
        // confines the `!Send` Code Mode isolate to its own thread internally.
        Command::Serve(args) => block_on(commands::serve(args)),
        Command::Exec(args) => block_on(commands::exec(args)),
    }
}

/// Run an async command to completion on a multi-threaded Tokio runtime.
fn block_on<F>(fut: F) -> anyhow::Result<()>
where
    F: std::future::Future<Output = anyhow::Result<()>>,
{
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(fut)
}

/// Initialize tracing. Logs always go to **stderr** so they never corrupt the
/// stdio MCP channel used by `skarn serve`.
fn init_tracing(verbose: bool) {
    use tracing_subscriber::{EnvFilter, fmt};
    let default = if verbose { "skarn=debug,info" } else { "warn" };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap_complete::{Shell, generate};

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn completion_scripts_generate_for_all_shells() {
        for shell in [
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::PowerShell,
            Shell::Elvish,
        ] {
            let mut buf = Vec::new();
            generate(shell, &mut Cli::command(), "skarn", &mut buf);
            assert!(!buf.is_empty(), "empty completion script for {shell:?}");
        }
    }
}
