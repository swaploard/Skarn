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
        runtime.set_max_stack_size(limits.max_stack_bytes).await;
        let deadline = Instant::now() + limits.wall_clock;
        runtime
            .set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)))
            .await;

        let context = AsyncContext::full(&runtime)
            .await
            .map_err(|e| Error::CodeMode(e.to_string()))?;

        let setup_source = format!("{PRELUDE_JS}\n{prepared_js}\n{RUNNER_JS}");

        // Phase 1: install host functions and kick off the async script.
        let bridge_for_js = bridge.clone();
        let counter_for_js = counter.clone();
        let max_calls = limits.max_tool_calls;
        let setup: std::result::Result<(), String> = async_with!(context => |ctx| {
            install_host(&ctx, bridge_for_js, counter_for_js, max_calls)
                .map_err(|e| e.to_string())?;
            ctx.eval::<(), _>(setup_source)
                .catch(&ctx)
                .map_err(|e| e.to_string())?;
            Ok(())
        })
        .await;
        setup.map_err(Error::CodeMode)?;

        // Phase 2: drive the runtime to completion (host calls + microtasks),
        // bounded by a wall-clock backstop in case a host call hangs.
        let grace = limits.wall_clock + Duration::from_secs(5);
        tokio::time::timeout(grace, runtime.idle())
            .await
            .map_err(|_| Error::CodeMode("execution timed out".to_string()))?;

        // Phase 3: read the JSON result the runner stored on the global object.
        let result_json: Option<String> = async_with!(context => |ctx| {
            ctx.globals().get::<_, String>("__skarn_result").ok()
        })
        .await;

        let raw = result_json.ok_or_else(|| {
            Error::CodeMode("script did not produce a result (likely timed out)".to_string())
        })?;

        if raw.len() > limits.max_output_bytes {
            return Err(Error::CodeMode(format!(
                "script output exceeded the {} byte limit",
                limits.max_output_bytes
            )));
        }

        let parsed: RawOutcome = serde_json::from_str(&raw)
            .map_err(|e| Error::CodeMode(format!("could not parse script result: {e}")))?;

        Ok(Outcome {
            ok: parsed.ok,
            value: parsed.value,
            error: parsed.error,
            logs: parsed.logs,
            tool_calls: counter.load(Ordering::SeqCst),
        })
    }
}

#[derive(Deserialize)]
struct RawOutcome {
    ok: bool,
    #[serde(default)]
    value: serde_json::Value,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    logs: Vec<String>,
}

/// Install the three host functions the `skarn` shim depends on.
fn install_host(
    ctx: &rquickjs::Ctx<'_>,
    bridge: Arc<dyn ToolBridge>,
    counter: Arc<AtomicUsize>,
    max_calls: usize,
) -> rquickjs::Result<()> {
    let globals = ctx.globals();

    {
        let bridge = bridge.clone();
        let counter = counter.clone();
        globals.set(
            "__skarn_call_tool",
            Func::from(Async(move |server: String, tool: String, args: String| {
                let bridge = bridge.clone();
                let counter = counter.clone();
                async move {
                    // Reserve a slot, then hand it back if we're over budget so
                    // the reported `tool_calls` reflects only accepted calls and
                    // never exceeds `max_calls`.
                    if counter.fetch_add(1, Ordering::SeqCst) >= max_calls {
                        counter.fetch_sub(1, Ordering::SeqCst);
                        return error_envelope(&format!(
                            "tool-call budget of {max_calls} exceeded"
                        ));
                    }
                    match bridge.call_tool(&server, &tool, &args).await {
                        Ok(result) => ok_envelope(&result),
                        Err(e) => error_envelope(&e),
                    }
                }
            })),
        )?;
    }

    {
        let bridge = bridge.clone();
        globals.set(
            "__skarn_read_resource",
            Func::from(Async(move |server: String, uri: String| {
                let bridge = bridge.clone();
                async move {
                    match bridge.read_resource(&server, &uri).await {
                        Ok(result) => ok_envelope(&result),
                        Err(e) => error_envelope(&e),
                    }
                }
            })),
        )?;
    }

    {
        let bridge = bridge.clone();
        globals.set(
            "__skarn_list_tools",
            Func::from(Async(move || {
                let bridge = bridge.clone();
                async move {
                    match bridge.list_tools().await {
                        Ok(result) => ok_envelope(&result),
                        Err(e) => error_envelope(&e),
                    }
                }
            })),
        )?;
    }

    Ok(())
}

/// Wrap a (valid JSON) result string in a success envelope.
fn ok_envelope(result_json: &str) -> String {
    let val: serde_json::Value = serde_json::from_str(result_json)
        .unwrap_or_else(|_| serde_json::Value::String(result_json.to_string()));
    serde_json::json!({ "ok": true, "result": val }).to_string()
}

fn error_envelope(msg: &str) -> String {
    serde_json::json!({ "ok": false, "error": msg }).to_string()
}

/// The JS shim that exposes the friendly `skarn` API on top of the raw host
/// functions. Injected before every script.
const PRELUDE_JS: &str = r#"
globalThis.__skarn_logs = [];
const skarn = {
  async callTool(server, tool, args) {
    const raw = await __skarn_call_tool(String(server), String(tool), JSON.stringify(args ?? {}));
    const r = JSON.parse(raw);
    if (!r.ok) throw new Error(r.error || "tool error");
    return r.result;
  },
  async readResource(server, uri) {
    const raw = await __skarn_read_resource(String(server), String(uri));
    const r = JSON.parse(raw);
    if (!r.ok) throw new Error(r.error || "resource error");
    return r.result;
  },
  async listTools() {
    const raw = await __skarn_list_tools();
    const r = JSON.parse(raw);
    if (!r.ok) throw new Error(r.error || "list error");
    return r.result;
  },
  log(...args) {
    globalThis.__skarn_logs.push(
      args.map((a) => (typeof a === "string" ? a : JSON.stringify(a))).join(" ")
    );
  },
  async parallel(calls, opts) {
    const concurrency = Math.max(1, (opts && opts.concurrency) || 8);
    const results = new Array(calls.length);
    let next = 0;
    async function worker() {
      while (next < calls.length) {
        const idx = next++;
        results[idx] = await calls[idx]();
      }
    }
    const n = Math.min(concurrency, calls.length);
    await Promise.all(Array.from({ length: n }, () => worker()));
    return results;
  },
  stash: (() => {
    const m = new Map();
    return {
      put: (k, v) => { m.set(String(k), v); },
      get: (k) => (m.has(String(k)) ? m.get(String(k)) : null),
      keys: () => Array.from(m.keys()),
    };
  })(),
  server(name) {
    return new Proxy({}, { get: (_t, tool) => (args) => skarn.callTool(name, String(tool), args) });
  },
};
globalThis.skarn = skarn;
"#;

/// The runner that invokes the user's `__skarn_main` and records the result.
const RUNNER_JS: &str = r#"
;(async () => {
  try {
    const value = await __skarn_main();
