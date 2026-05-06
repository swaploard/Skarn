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
