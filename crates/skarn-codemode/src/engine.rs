//! The hermetic Code Mode runtime, built on an async QuickJS isolate.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rquickjs::prelude::{Async, Func};
use rquickjs::{AsyncContext, AsyncRuntime, CatchResultExt, async_with};
use serde::{Deserialize, Serialize};
use skarn_common::{Error, Result};

use crate::bridge::ToolBridge;

/// Resource limits for a single Code Mode execution.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ExecLimits {
    /// QuickJS heap limit in bytes.
    pub memory_bytes: usize,
    /// Maximum native stack in bytes.
    pub max_stack_bytes: usize,
    /// Wall-clock deadline for the whole run.
    pub wall_clock: Duration,
    /// Maximum number of host tool calls a script may make.
    pub max_tool_calls: usize,
    /// Maximum size of the returned result JSON (bytes) before it is rejected.
    pub max_output_bytes: usize,
}
