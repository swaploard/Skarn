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
pub enum NetArg {
    /// Deny all network access (default).
    Deny,
    /// Allow loopback only.
    Loopback,
    /// Allow outbound connections.
    Outbound,
    /// Allow all network access.
    All,
}

impl From<NetArg> for NetPolicy {
    fn from(n: NetArg) -> Self {
        match n {
            NetArg::Deny => NetPolicy::DenyAll,
            NetArg::Loopback => NetPolicy::AllowLoopback,
            NetArg::Outbound => NetPolicy::AllowOutbound,
            NetArg::All => NetPolicy::AllowAll,
        }
    }
}

// ---------------------------------------------------------------------------
// serve
// ---------------------------------------------------------------------------

pub async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    let mut config = load_config(args.config.as_ref())?;
    if args.passthrough {
        config.gateway.passthrough = true;
    }
    tracing::info!(
        servers = config.enabled_servers().count(),
        "starting Skarn gateway on stdio"
    );
    let server = skarn_gateway::build_server(&config, args.limits.to_limits())
        .await
        .map_err(|e| anyhow!("building gateway: {e}"))?;
    skarn_gateway::serve_stdio(server)
        .await
        .map_err(|e| anyhow!("serving gateway: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// exec
// ---------------------------------------------------------------------------

pub async fn exec(args: ExecArgs) -> anyhow::Result<()> {
    let config = load_config(args.config.as_ref())?;
    let code = read_script(&args)?;

    let outcome = skarn_gateway::run_script(&config, args.limits.to_limits(), &code)
        .await
        .map_err(|e| anyhow!("running script: {e}"))?;

    for line in &outcome.logs {
        eprintln!("[log] {line}");
    }
    eprintln!("[tool calls: {}]", outcome.tool_calls);

    if outcome.ok {
        println!("{}", serde_json::to_string_pretty(&outcome.value)?);
        Ok(())
    } else {
        Err(anyhow!(
            "script error: {}",
            outcome.error.unwrap_or_else(|| "unknown".into())
        ))
    }
}

fn read_script(args: &ExecArgs) -> anyhow::Result<String> {
    if let Some(code) = &args.code {
        return Ok(code.clone());
    }
    match &args.file {
        Some(p) if p.as_os_str() == "-" => {
            use std::io::Read;
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            Ok(s)
        }
        Some(p) => std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display())),
        None => Err(anyhow!("provide a script with --code or --file")),
    }
}

// ---------------------------------------------------------------------------
// run (sandbox + compress a shell command)
// ---------------------------------------------------------------------------

pub fn run(args: RunArgs) -> anyhow::Result<()> {
    let spec = CommandSpec::from_argv(&args.command).context("no command provided after `--`")?;

    let policy = if args.no_sandbox {
        None
    } else {
        let workspace = match &args.workspace {
            Some(w) => w.clone(),
            None => std::env::current_dir()?,
        };
        Some(
            Policy::builder()
                .workspace(&workspace)
                .net(args.net.into())
                .build(),
        )
    };

    let (output, sandboxed) = run_capture(policy.as_ref(), &spec)
        .with_context(|| format!("running `{}`", spec.display()))?;

    if args.no_compress {
        use std::io::Write;
        std::io::stdout().write_all(&output.stdout)?;
        std::io::stderr().write_all(&output.stderr)?;
    } else {
        let compressor = Compressor::builtin();
        let compressed = compressor.compress(&spec, &output.stdout, &output.stderr);
        print!("{}", compressed.text);
        if !compressed.text.is_empty() && !compressed.text.ends_with('\n') {
            println!();
        }
        if args.stats {
            eprintln!(
                "skarn: {} → {} tokens ({}% saved) · profile={} · {}",
                compressed.savings.before,
                compressed.savings.after,
                compressed.savings.percent(),
                compressed.profile,
