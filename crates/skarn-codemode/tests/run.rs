//! End-to-end Code Mode execution tests (real QuickJS isolate + in-process bridge).
//!
//! rquickjs' async runtime is `!Send`, so these run on a current-thread runtime.

use std::sync::Arc;
use std::time::Duration;

use skarn_codemode::{Engine, ExecLimits, InProcessBridge, ToolBridge};

fn math_bridge() -> Arc<dyn ToolBridge> {
    Arc::new(
        InProcessBridge::new()
            .with_tool("math", "add", "Add two numbers", |args| {
                let v: serde_json::Value = serde_json::from_str(args).map_err(|e| e.to_string())?;
                let a = v["a"].as_i64().unwrap_or(0);
                let b = v["b"].as_i64().unwrap_or(0);
                Ok(serde_json::json!(a + b).to_string())
            })
            .with_tool("math", "double", "Double a number", |args| {
                let v: serde_json::Value = serde_json::from_str(args).map_err(|e| e.to_string())?;
                let n = v["n"].as_i64().unwrap_or(0);
                Ok(serde_json::json!(n * 2).to_string())
            }),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn runs_a_pure_script() {
    let engine = Engine::with_defaults();
    let bridge: Arc<dyn ToolBridge> = Arc::new(InProcessBridge::new());
    let out = engine
        .run("const x = 20; return x + 22;", bridge)
        .await
        .unwrap();
    assert!(out.ok, "error: {:?}", out.error);
    assert_eq!(out.value, serde_json::json!(42));
    assert_eq!(out.tool_calls, 0);
}

#[tokio::test(flavor = "current_thread")]
async fn calls_tools_and_aggregates_locally() {
    let engine = Engine::with_defaults();
