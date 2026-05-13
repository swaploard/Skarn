//! Running a Code Mode script — in-process, or in a dedicated OS-sandboxed
//! worker subprocess.
//!
//! Two execution strategies share one servicer pattern (fulfil host tool calls
//! against the [`DownstreamManager`] that owns the MCP clients):
//!
//! * **In-process** ([`execute_in_process`]): the `!Send` QuickJS isolate runs
//!   on a dedicated thread with its own current-thread runtime, bridged back to
//!   the main runtime over an mpsc channel. The isolate is hermetic but shares
//!   the gateway's address space.
//!
//! * **Worker** ([`execute_worker`], Unix only): the isolate runs in a child
//!   process that confines *itself* with the OS-native sandbox before touching
//!   the script, bridged back to the parent over its stdio pipes
//!   (newline-delimited JSON, see [`crate::worker_proto`]). A hypothetical
//!   isolate escape lands in a kernel-confined process with no network and no
//!   workspace writes.
//!
//! [`execute_code`] picks between them based on [`Isolation`] and whether an OS
//! sandbox backend is available.

use std::sync::Arc;

use async_trait::async_trait;
use skarn_codemode::{Engine, ExecLimits, Outcome, ToolBridge};
use skarn_common::{Error, Result};
use skarn_sandbox::Backend;
use tokio::sync::{mpsc, oneshot};

use crate::config::Isolation;
use crate::downstream::DownstreamManager;

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

/// Run `code` against `manager` using the requested [`Isolation`] strategy.
pub async fn execute_code(
    manager: Arc<DownstreamManager>,
    limits: ExecLimits,
    code: String,
    isolation: Isolation,
) -> Result<Outcome> {
    let use_worker = match isolation {
        Isolation::InProcess => false,
        Isolation::Worker => {
            if !worker_available() {
                return Err(Error::CodeMode(
                    "isolation = \"worker\" was requested but the cross-process \
                     OS-sandboxed worker is unavailable on this platform; set \
                     isolation = \"in_process\" to run the hermetic isolate alone"
                        .to_string(),
                ));
            }
            true
        }
        Isolation::Auto => worker_available(),
    };

    #[cfg(unix)]
    if use_worker {
        return execute_worker(manager, limits, code).await;
    }
    #[cfg(not(unix))]
    let _ = use_worker;

    execute_in_process(manager, limits, code).await
}

/// Whether the cross-process OS-sandboxed worker can run here. It is Unix-only
/// (the worker self-applies the sandbox; on Windows a process cannot move itself
/// into an AppContainer, so in-gateway execution uses the hermetic isolate).
fn worker_available() -> bool {
    cfg!(unix) && skarn_sandbox::backend() != Backend::None
}

// ---------------------------------------------------------------------------
// In-process execution
// ---------------------------------------------------------------------------

/// One host operation requested by the isolate, with a reply channel.
struct BridgeRequest {
    op: BridgeOp,
    reply: oneshot::Sender<std::result::Result<String, String>>,
}

enum BridgeOp {
    CallTool {
        server: String,
        tool: String,
        args: String,
    },
    ReadResource {
        server: String,
        uri: String,
    },
    ListTools,
}

/// A [`ToolBridge`] that forwards every call over an mpsc channel to a servicer
/// running on the main runtime. Lives on the dedicated isolate thread.
struct ChannelBridge {
    tx: mpsc::UnboundedSender<BridgeRequest>,
}

#[async_trait(?Send)]
impl ToolBridge for ChannelBridge {
    async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        args_json: &str,
    ) -> std::result::Result<String, String> {
        self.send(BridgeOp::CallTool {
            server: server.to_string(),
            tool: tool.to_string(),
            args: args_json.to_string(),
        })
        .await
    }

    async fn read_resource(&self, server: &str, uri: &str) -> std::result::Result<String, String> {
        self.send(BridgeOp::ReadResource {
            server: server.to_string(),
            uri: uri.to_string(),
        })
        .await
    }

    async fn list_tools(&self) -> std::result::Result<String, String> {
        self.send(BridgeOp::ListTools).await
    }
}

impl ChannelBridge {
    async fn send(&self, op: BridgeOp) -> std::result::Result<String, String> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(BridgeRequest { op, reply })
            .map_err(|_| "gateway bridge closed".to_string())?;
        rx.await
            .map_err(|_| "gateway bridge dropped the request".to_string())?
    }
}

/// Validate, transpile, and run `code` in-process against the downstream servers
/// in `manager`, returning the script's outcome.
pub async fn execute_in_process(
    manager: Arc<DownstreamManager>,
    limits: ExecLimits,
    code: String,
) -> Result<Outcome> {
    let (tx, mut rx) = mpsc::unbounded_channel::<BridgeRequest>();

    // Servicer: fulfils bridge requests on the main runtime, where the MCP
    // clients live. Calls go directly to the manager (Send futures), never
    // through a `!Send` bridge.
    let servicer_manager = manager.clone();
    let servicer = tokio::spawn(async move {
        while let Some(req) = rx.recv().await {
            let result = match req.op {
                BridgeOp::CallTool { server, tool, args } => servicer_manager
                    .call(&server, &tool, &args)
                    .await
                    .map_err(|e| e.to_string()),
                BridgeOp::ReadResource { server, uri } => servicer_manager
                    .read_resource(&server, &uri)
                    .await
                    .map_err(|e| e.to_string()),
                BridgeOp::ListTools => {
                    serde_json::to_string(&servicer_manager.registry().descriptors())
                        .map_err(|e| e.to_string())
                }
            };
            let _ = req.reply.send(result);
        }
    });

    // Run the isolate on a dedicated blocking thread with its own runtime.
    let join = tokio::task::spawn_blocking(move || -> Result<Outcome> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::CodeMode(format!("isolate runtime: {e}")))?;
        let bridge: Arc<dyn ToolBridge> = Arc::new(ChannelBridge { tx });
        runtime.block_on(Engine::new(limits).run(&code, bridge))
    })
    .await;

    servicer.abort();

    match join {
        Ok(result) => result,
        Err(e) => Err(Error::CodeMode(format!("isolate thread failed: {e}"))),
    }
}

// ---------------------------------------------------------------------------
// Cross-process worker execution (parent side, Unix only)
// ---------------------------------------------------------------------------

/// The OS-sandbox policy the worker confines itself with: deny network, no
/// writable workspace (the isolate needs neither), system reads allowed so a
/// dynamically-linked binary can run, and known secret stores denied.
#[cfg(unix)]
fn isolate_policy() -> skarn_sandbox::Policy {
    skarn_sandbox::Policy {
        fs_deny_read: skarn_sandbox::default_secret_paths(),
        ..skarn_sandbox::Policy::default()
    }
}

/// Locate the worker binary: an explicit override (used by tests) or this very
/// executable, which carries the hidden `__worker` subcommand.
#[cfg(unix)]
fn worker_binary() -> Result<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("SKARN_WORKER_BIN") {
        return Ok(path.into());
    }
    std::env::current_exe().map_err(|e| Error::CodeMode(format!("locating worker binary: {e}")))
}

#[cfg(unix)]
fn to_reply(result: Result<String>) -> (bool, String) {
    match result {
        Ok(payload) => (true, payload),
        Err(e) => (false, e.to_string()),
    }
}

/// Spawn the OS-sandboxed worker, hand it the job, and service its tool calls
/// over its stdio pipes until it returns a result.
#[cfg(unix)]
async fn execute_worker(
    manager: Arc<DownstreamManager>,
    limits: ExecLimits,
    code: String,
) -> Result<Outcome> {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};

    use crate::worker_proto::{BridgeOpWire, JobMsg, ReplyMsg, WorkerMsg};

    let bin = worker_binary()?;
    let mut child = tokio::process::Command::new(&bin)
        .arg("__worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            Error::CodeMode(format!("spawning Code Mode worker {}: {e}", bin.display()))
        })?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| Error::CodeMode("worker stdin unavailable".to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::CodeMode("worker stdout unavailable".to_string()))?;
    let mut lines = BufReader::new(stdout).lines();

    // Hand over the job.
