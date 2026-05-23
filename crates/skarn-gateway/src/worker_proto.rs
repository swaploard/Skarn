//! The wire protocol between the gateway (parent) and the sandboxed Code Mode
//! worker (child).
//!
//! Framing is newline-delimited JSON. The parent writes a single [`JobMsg`] to
//! the worker's stdin, then services [`WorkerMsg::Request`] messages from the
//! worker's stdout by writing a [`ReplyMsg`] back to stdin for each, until the
//! worker emits a terminal [`WorkerMsg::Result`] or [`WorkerMsg::Failed`]. The
//! worker's stderr is left for human-readable logs.

use serde::{Deserialize, Serialize};
use skarn_codemode::{ExecLimits, Outcome};
use skarn_sandbox::Policy;

/// The job the parent hands the worker: the OS-sandbox policy the worker applies
/// to itself (Unix) or that the parent applied when launching it (Windows
/// AppContainer), the execution limits, and the script source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobMsg {
    /// The OS-sandbox policy confining the isolate.
    pub policy: Policy,
    /// Resource limits for the run.
    pub limits: ExecLimits,
    /// The Code Mode script source (pre-validation).
    pub code: String,
}

/// One host operation the worker's isolate needs the parent to fulfil.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum BridgeOpWire {
    /// Call a downstream tool.
    CallTool {
        server: String,
        tool: String,
        args: String,
    },
    /// Read a downstream resource by URI.
    ReadResource { server: String, uri: String },
    /// List all downstream tools.
    ListTools,
}

/// A message from the worker to the parent.
#[derive(Clone, Debug, Serialize, Deserialize)]
