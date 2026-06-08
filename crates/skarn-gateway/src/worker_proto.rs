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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkerMsg {
    /// A host operation, awaiting a [`ReplyMsg`] with the same `id`.
    Request { id: u64, op: BridgeOpWire },
    /// The script finished; this is the final message.
    Result { outcome: Outcome },
    /// The worker failed before producing a result (validation, sandbox apply,
    /// panic, …); this is the final message.
    Failed { error: String },
}

/// The parent's reply to a [`WorkerMsg::Request`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplyMsg {
    /// Matches the `id` of the request being answered.
    pub id: u64,
    /// Whether the operation succeeded.
    pub ok: bool,
    /// The result JSON (when `ok`) or the error message (when `!ok`).
    pub payload: String,
}
