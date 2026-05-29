//! Implementations of the `skarn` subcommands.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, anyhow};
use clap::{Args, ValueEnum};
use skarn_codemode::ExecLimits;
use skarn_common::CommandSpec;
use skarn_compress::Compressor;
use skarn_gateway::GatewayConfig;
use skarn_sandbox::{NetPolicy, Policy};

// ---------------------------------------------------------------------------
// Argument structs
// ---------------------------------------------------------------------------

#[derive(Args, Debug)]
pub struct ServeArgs {
    /// Path to the gateway config (default: ./skarn.toml if present).
    #[arg(long, short)]
    config: Option<PathBuf>,
    /// Expose the namespaced downstream tools directly, in addition to the meta-tools.
    #[arg(long)]
    passthrough: bool,
    #[command(flatten)]
    limits: LimitArgs,
}

#[derive(Args, Debug)]
pub struct ExecArgs {
    /// Path to the gateway config (default: ./skarn.toml if present).
    #[arg(long, short)]
    config: Option<PathBuf>,
    /// Inline script source.
    #[arg(long, short = 'e', conflicts_with = "file")]
    code: Option<String>,
    /// Read the script from a file (`-` for stdin).
    #[arg(long, short)]
    file: Option<PathBuf>,
    #[command(flatten)]
    limits: LimitArgs,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Network policy for the sandboxed command.
    #[arg(long, value_enum, default_value_t = NetArg::Deny)]
    net: NetArg,
    /// The writable workspace directory (default: current directory).
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Disable OS-native sandboxing (runs the command unconfined).
    #[arg(long)]
    no_sandbox: bool,
    /// Do not compress the command output.
    #[arg(long)]
    no_compress: bool,
    /// Print a one-line token-savings summary to stderr.
    #[arg(long)]
    stats: bool,
    /// The command to run, after `--`.
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Overwrite an existing skarn.toml.
    #[arg(long)]
    force: bool,
}

#[derive(Args, Debug)]
struct LimitArgs {
    /// QuickJS heap limit (MB) for `execute`.
    #[arg(long, default_value_t = 64)]
    mem_mb: usize,
    /// Wall-clock timeout (seconds) for `execute`.
    #[arg(long, default_value_t = 30)]
    timeout_secs: u64,
    /// Maximum downstream tool calls per `execute`.
    #[arg(long, default_value_t = 256)]
    max_tool_calls: usize,
}

impl LimitArgs {
    fn to_limits(&self) -> ExecLimits {
        ExecLimits {
            memory_bytes: self.mem_mb * 1024 * 1024,
            wall_clock: Duration::from_secs(self.timeout_secs),
            max_tool_calls: self.max_tool_calls,
            ..ExecLimits::default()
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
