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
