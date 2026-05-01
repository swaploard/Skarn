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
    let src = r#"
        const a = await skarn.callTool("math", "add", { a: 2, b: 3 });   // 5
        const b = await skarn.server("math").double({ n: a });           // 10
        skarn.log("intermediate", a, b);
        return { a, b, total: a + b };
    "#;
    let out = engine.run(src, math_bridge()).await.unwrap();
    assert!(out.ok, "error: {:?}", out.error);
    assert_eq!(
        out.value,
        serde_json::json!({ "a": 5, "b": 10, "total": 15 })
    );
    assert_eq!(out.tool_calls, 2, "two downstream calls were made");
    assert!(out.logs.iter().any(|l| l.contains("intermediate 5 10")));
}

#[tokio::test(flavor = "current_thread")]
async fn parallel_helper_runs_calls() {
    let engine = Engine::with_defaults();
    let src = r#"
        const results = await skarn.parallel(
            [1, 2, 3, 4].map((n) => () => skarn.server("math").double({ n })),
            { concurrency: 2 }
        );
        return results;
    "#;
    let out = engine.run(src, math_bridge()).await.unwrap();
    assert!(out.ok, "error: {:?}", out.error);
    assert_eq!(out.value, serde_json::json!([2, 4, 6, 8]));
    assert_eq!(out.tool_calls, 4);
}

#[tokio::test(flavor = "current_thread")]
async fn thrown_errors_are_reported_not_panicked() {
    let engine = Engine::with_defaults();
    let out = engine
        .run("throw new Error('boom');", Arc::new(InProcessBridge::new()))
        .await
        .unwrap();
    assert!(!out.ok);
    assert!(out.error.unwrap().contains("boom"));
}

#[tokio::test(flavor = "current_thread")]
async fn tool_errors_surface_in_the_script() {
    let engine = Engine::with_defaults();
    let src = r#"
        try {
            await skarn.callTool("math", "nonexistent", {});
            return "should not reach";
        } catch (e) {
            return "caught: " + e.message;
        }
    "#;
    let out = engine.run(src, math_bridge()).await.unwrap();
    assert!(out.ok);
    assert!(out.value.as_str().unwrap().contains("caught"));
}

#[tokio::test(flavor = "current_thread")]
async fn infinite_loops_are_interrupted() {
    let limits = ExecLimits {
        wall_clock: Duration::from_millis(300),
        ..ExecLimits::default()
    };
    let engine = Engine::new(limits);
    let result = engine
        .run("while (true) {}", Arc::new(InProcessBridge::new()))
        .await;
    // The interrupt handler aborts the run; we surface it as an error.
    assert!(
        result.is_err(),
        "infinite loop must not run forever: {result:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn tool_call_budget_is_enforced() {
    let limits = ExecLimits {
        max_tool_calls: 3,
        ..ExecLimits::default()
    };
    let engine = Engine::new(limits);
    let src = r#"
        let n = 0;
        for (let i = 0; i < 10; i++) {
            try { await skarn.server("math").double({ n: i }); n++; }
            catch (e) { return { calls: n, stopped: e.message }; }
        }
        return { calls: n };
    "#;
    let out = engine.run(src, math_bridge()).await.unwrap();
    assert!(out.ok, "error: {:?}", out.error);
    // The 4th call trips the budget.
    let stopped = out.value["stopped"].as_str().unwrap_or("");
    assert!(
        stopped.contains("budget"),
        "expected budget error, got {:?}",
        out.value
    );
    // The rejected 4th call must not be counted: only accepted calls show up.
    assert_eq!(
        out.tool_calls, 3,
        "rejected calls must not inflate the count"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn output_cap_is_enforced() {
    let limits = ExecLimits {
        max_output_bytes: 256,
        ..ExecLimits::default()
    };
    let engine = Engine::new(limits);
    let result = engine
        .run("return 'x'.repeat(5000);", Arc::new(InProcessBridge::new()))
        .await;
    let err = result.expect_err("oversized output must be rejected");
    assert!(err.to_string().contains("byte limit"), "got: {err}");
}

#[tokio::test(flavor = "current_thread")]
