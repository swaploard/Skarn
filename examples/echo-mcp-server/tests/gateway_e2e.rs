//! End-to-end test of the whole Skarn stack.
//!
//! The gateway launches `echo-mcp-server` as a real stdio subprocess, lists and
//! namespaces its tools, and a Code Mode script calls those tools through the
//! `skarn` bridge — exercising: downstream stdio transport, tool aggregation,
//! the QuickJS isolate, the host bridge, and result extraction. We also drive
//! the gateway's *upstream* MCP surface (`search` / `execute`) with an
//! in-memory client.

use std::collections::BTreeMap;

use skarn_codemode::ExecLimits;
use skarn_gateway::{GatewayConfig, GatewaySettings, Isolation, ServerConfig, TransportConfig};

const ECHO_BIN: &str = env!("CARGO_BIN_EXE_echo-mcp-server");

fn config() -> GatewayConfig {
    let mut servers = BTreeMap::new();
    servers.insert(
        "echo".to_string(),
        ServerConfig {
            enabled: true,
            transport: TransportConfig::Stdio {
                command: ECHO_BIN.to_string(),
                args: vec![],
                env: BTreeMap::new(),
                cwd: None,
            },
        },
    );
    GatewayConfig {
        // These tests exercise the gateway + in-process isolate directly; the
        // cross-process worker (which needs the `skarn` binary) is covered by the
        // CLI integration tests.
        gateway: GatewaySettings {
            isolation: Isolation::InProcess,
            ..GatewaySettings::default()
        },
        servers,
    }
}

/// Run a future on a multi-threaded runtime (the gateway confines `!Send` work
/// to its own thread internally).
fn run_local<F: std::future::Future<Output = ()>>(fut: F) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut);
}

#[test]
fn diag_connect_call_drop() {
    use skarn_gateway::DownstreamManager;
    run_local(async {
        let manager = DownstreamManager::connect(&config()).await.unwrap();
        let r = manager
            .call("echo", "add", r#"{"a":2,"b":3}"#)
            .await
            .unwrap();
        println!("DIAG call result: {r}");
        assert!(r.contains("\"sum\":5"));
        drop(manager);
        tokio::task::yield_now().await;
    });
}

#[test]
fn code_mode_calls_downstream_tools_through_the_gateway() {
    run_local(async {
        let script = r#"
            const r1 = await skarn.callTool("echo", "add", { a: 2, b: 3 });    // {sum:5}
            const r2 = await skarn.server("echo").add({ a: r1.sum, b: 10 });   // {sum:15}
            const e = await skarn.server("echo").echo({ text: "hi" });         // {echoed:"hi"}
            skarn.log("partial sums", r1.sum, r2.sum);
            return { total: r2.sum, echoed: e.echoed, calls: 3 };
        "#;

        let outcome = skarn_gateway::run_script(&config(), ExecLimits::default(), script)
            .await
            .expect("run_script");

        assert!(outcome.ok, "script error: {:?}", outcome.error);
        assert_eq!(outcome.value["total"], serde_json::json!(15));
        assert_eq!(outcome.value["echoed"], serde_json::json!("hi"));
        assert_eq!(outcome.tool_calls, 3, "three downstream calls were made");
        assert!(outcome.logs.iter().any(|l| l.contains("partial sums 5 15")));
    });
}

/// Exercise the cross-process OS-sandboxed worker end-to-end: spawn the real
/// `skarn __worker`, confine it, and bridge a downstream tool call back over its
/// stdio pipes. Unix-only (the worker self-applies the sandbox).
#[cfg(unix)]
#[test]
fn code_mode_runs_in_the_sandboxed_worker() {
