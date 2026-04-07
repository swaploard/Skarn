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

impl Default for ExecLimits {
    fn default() -> Self {
        Self {
            memory_bytes: 64 * 1024 * 1024,
            max_stack_bytes: 1024 * 1024,
            wall_clock: Duration::from_secs(30),
            max_tool_calls: 256,
            max_output_bytes: 1024 * 1024,
        }
    }
}

impl ExecLimits {
    /// Clamp the limits to safe floors so a misconfiguration (e.g. a zero memory
    /// or wall-clock value) can't make every script fail with a confusing error.
    ///
    /// `max_tool_calls` is intentionally *not* floored: `0` is a legitimate
    /// "this script may make no tool calls" policy.
    fn sanitized(self) -> Self {
        Self {
            memory_bytes: self.memory_bytes.max(1024 * 1024),
            max_stack_bytes: self.max_stack_bytes.max(64 * 1024),
            wall_clock: self.wall_clock.max(Duration::from_millis(100)),
            max_tool_calls: self.max_tool_calls,
            max_output_bytes: self.max_output_bytes.max(1024),
        }
    }
}

/// The result of running a Code Mode script.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outcome {
    /// Whether the script completed without throwing.
    pub ok: bool,
    /// The value the script returned (JSON `null` if it returned nothing).
    pub value: serde_json::Value,
    /// The error message + stack if the script threw.
    pub error: Option<String>,
    /// Lines emitted via `skarn.log(...)`.
    pub logs: Vec<String>,
    /// How many tool calls the script made.
    pub tool_calls: usize,
}

/// The Code Mode engine. Cheap to construct; one is reused per worker.
pub struct Engine {
    limits: ExecLimits,
}

impl Engine {
    pub fn new(limits: ExecLimits) -> Self {
        Self { limits }
    }

    pub fn with_defaults() -> Self {
        Self::new(ExecLimits::default())
    }

    /// Validate, transpile, and run `source` against `bridge`.
    pub async fn run(&self, source: &str, bridge: Arc<dyn ToolBridge>) -> Result<Outcome> {
        let prepared = crate::validate::validate_and_transpile(source)?;
        self.run_prepared(&prepared, bridge).await
    }

    /// Run already-validated JavaScript (the output of
    /// [`crate::validate::validate_and_transpile`]).
    pub async fn run_prepared(
        &self,
        prepared_js: &str,
        bridge: Arc<dyn ToolBridge>,
    ) -> Result<Outcome> {
        let limits = self.limits.sanitized();
        let counter = Arc::new(AtomicUsize::new(0));

        let runtime = AsyncRuntime::new().map_err(|e| Error::CodeMode(e.to_string()))?;
        runtime.set_memory_limit(limits.memory_bytes).await;
