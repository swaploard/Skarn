//! Implementations of the `skarn` subcommands.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, anyhow};
use clap::{Args, CommandFactory, ValueEnum};
use clap_complete::Shell;
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
pub struct CompletionsArgs {
    /// The shell to generate a completion script for (bash, zsh, fish, powershell).
    #[arg(value_enum)]
    pub shell: Shell,
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
                if sandboxed {
                    "sandboxed"
                } else {
                    "UNSANDBOXED"
                },
            );
        }
    }

    std::process::exit(output.status.code().unwrap_or(1));
}

#[cfg(unix)]
fn run_capture(
    policy: Option<&Policy>,
    spec: &CommandSpec,
) -> std::io::Result<(std::process::Output, bool)> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    let mut cmd = std::process::Command::new(&spec.program);
    cmd.args(&spec.args);
    if let Some(cwd) = &spec.cwd {
        cmd.current_dir(cwd);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let sandboxed = policy.is_some();
    if let Some(policy) = policy {
        let policy = policy.clone();
        // SAFETY: `apply_to_current_process` is run in the forked child before
        // exec. The parent is single-threaded here (the `run` path uses no async
        // runtime), so this avoids the fork+non-async-signal-safe deadlock.
        unsafe {
            cmd.pre_exec(move || {
                policy
                    .apply_to_current_process()
                    .map(|_| ())
                    .map_err(|e| std::io::Error::other(e.to_string()))
            });
        }
    }

    Ok((cmd.output()?, sandboxed))
}

#[cfg(windows)]
fn run_capture(
    policy: Option<&Policy>,
    spec: &CommandSpec,
) -> std::io::Result<(std::process::Output, bool)> {
    use std::os::windows::process::ExitStatusExt;

    match policy {
        // A sandbox was requested: launch into an AppContainer with captured
        // stdio. Any failure propagates (fail closed) rather than running
        // unconfined.
        Some(policy) => {
            let child = skarn_sandbox::spawn_appcontainer(policy, spec)
                .map_err(|e| std::io::Error::other(format!("AppContainer sandbox: {e}")))?;
            let captured = child
                .wait_with_output()
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            let output = std::process::Output {
                status: std::process::ExitStatus::from_raw(captured.code as u32),
                stdout: captured.stdout,
                stderr: captured.stderr,
            };
            Ok((output, true))
        }
        // `--no-sandbox`: run unconfined.
        None => Ok((unconfined_output(spec)?, false)),
    }
}

#[cfg(not(any(unix, windows)))]
fn run_capture(
    policy: Option<&Policy>,
    spec: &CommandSpec,
) -> std::io::Result<(std::process::Output, bool)> {
    match policy {
        Some(_) => Err(std::io::Error::other(
            "OS sandboxing is unavailable on this platform; re-run with \
             --no-sandbox to run unconfined (NOT recommended)",
        )),
        None => Ok((unconfined_output(spec)?, false)),
    }
}

#[cfg(not(unix))]
fn unconfined_output(spec: &CommandSpec) -> std::io::Result<std::process::Output> {
    let mut cmd = std::process::Command::new(&spec.program);
    cmd.args(&spec.args);
    if let Some(cwd) = &spec.cwd {
        cmd.current_dir(cwd);
    }
    cmd.output()
}

// ---------------------------------------------------------------------------
// doctor / init / hook
// ---------------------------------------------------------------------------

pub fn doctor() -> anyhow::Result<()> {
    let report = skarn_sandbox::backend_report();
    println!("Skarn v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Sandbox backend : {} [{:?}]", report.backend, report.status);
    for note in &report.notes {
        println!("    • {note}");
    }
    println!();
    let compressor = Compressor::builtin();
    println!(
        "Compression     : ready ({} tool profiles)",
        compressor.ruleset().profiles.len()
    );
    println!("Code Mode       : ready (QuickJS isolate + oxc validation)");
    println!("Gateway         : ready (rmcp 1.8; stdio + http transports)");
    Ok(())
}

pub fn init(args: InitArgs) -> anyhow::Result<()> {
    let path = PathBuf::from("skarn.toml");
    if path.exists() && !args.force {
        println!("skarn.toml already exists (use --force to overwrite).");
    } else {
        std::fs::write(&path, crate::scaffold::SKARN_TOML_TEMPLATE)?;
        println!("Wrote {}.", path.display());
    }
    println!();
    println!("{}", crate::scaffold::INTEGRATION_SNIPPETS);
    Ok(())
}

pub fn hook() -> anyhow::Result<()> {
    println!("{}", crate::scaffold::CLAUDE_HOOK_SNIPPET);
    Ok(())
}

pub fn completions(args: CompletionsArgs) -> anyhow::Result<()> {
    let mut cmd = crate::Cli::command();
    clap_complete::generate(args.shell, &mut cmd, "skarn", &mut std::io::stdout());
    Ok(())
}

/// The hidden `__worker` subcommand: run one Code Mode job inside an OS sandbox,
/// reading the job from stdin and reporting on stdout. Driven by `skarn serve`.
pub fn worker() -> anyhow::Result<()> {
    skarn_gateway::run_worker_job().map_err(|e| anyhow!("worker: {e}"))
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn load_config(path: Option<&PathBuf>) -> anyhow::Result<GatewayConfig> {
    match path {
        Some(p) => GatewayConfig::load(p).map_err(|e| anyhow!("{e}")),
        None => {
            let default = PathBuf::from("skarn.toml");
            if default.exists() {
                GatewayConfig::load(&default).map_err(|e| anyhow!("{e}"))
            } else {
                tracing::warn!("no skarn.toml found; starting with no downstream servers");
                Ok(GatewayConfig::default())
            }
        }
    }
}
