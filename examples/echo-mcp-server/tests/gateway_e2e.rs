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
    // The gateway spawns the `skarn` binary (which carries `__worker`). Ensure it
    // is built, then point the worker locator at it.
    let target_dir = std::path::Path::new(ECHO_BIN).parent().unwrap();
    let skarn = target_dir.join("skarn");
    let built = std::process::Command::new(env!("CARGO"))
        .args(["build", "-p", "skarn"])
        .status()
        .expect("invoke cargo build");
    assert!(built.success(), "failed to build the skarn binary");
    assert!(
        skarn.exists(),
        "skarn binary missing at {}",
        skarn.display()
    );

    // SAFETY: this is the only test that touches SKARN_WORKER_BIN, and it runs
    // its worker calls before removing it.
    unsafe { std::env::set_var("SKARN_WORKER_BIN", &skarn) };

    let mut cfg = config();
    cfg.gateway.isolation = Isolation::Worker;

    run_local(async move {
        // Happy path: a downstream tool call routed through the sandboxed worker.
        let script = r#"
            const r = await skarn.server("echo").add({ a: 40, b: 2 });
            skarn.log("worker computed", r.sum);
            return { answer: r.sum };
        "#;
        let outcome = skarn_gateway::run_script(&cfg, ExecLimits::default(), script)
            .await
            .expect("run_script via worker");
        assert!(outcome.ok, "worker script error: {:?}", outcome.error);
        assert_eq!(outcome.value["answer"], serde_json::json!(42));
        assert_eq!(outcome.tool_calls, 1);
        assert!(
            outcome
                .logs
                .iter()
                .any(|l| l.contains("worker computed 42"))
        );

        // Timeout path: an infinite loop is interrupted and surfaces an error.
        let limits = ExecLimits {
            wall_clock: std::time::Duration::from_millis(300),
            ..ExecLimits::default()
        };
        let timed_out = skarn_gateway::run_script(&cfg, limits, "while (true) {}").await;
        assert!(
            timed_out.is_err(),
            "infinite loop in the worker must not run forever: {timed_out:?}"
        );
    });

    // SAFETY: paired with the set_var above.
    unsafe { std::env::remove_var("SKARN_WORKER_BIN") };
}

#[test]
fn gateway_upstream_surface_search_and_execute() {
    use rmcp::ServiceExt;
    use rmcp::model::CallToolRequestParams;

    run_local(async {
        let server = skarn_gateway::build_server(&config(), ExecLimits::default())
            .await
            .expect("build_server");

        // Wire an in-memory client <-> the gateway server. Both `serve` calls
        // block until the initialize handshake completes, so they must be driven
        // concurrently.
        let (server_io, client_io) = tokio::io::duplex(64 * 1024);
        let (sr, sw) = tokio::io::split(server_io);
        let (cr, cw) = tokio::io::split(client_io);

        let (server_res, client_res) = tokio::join!(server.serve((sr, sw)), ().serve((cr, cw)));
        let _running = server_res.expect("serve gateway");
        let client = client_res.expect("connect client");

        // The upstream surface is the small, fixed meta-tool set.
        let tools = client.list_all_tools().await.expect("list_all_tools");
        let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
        assert!(names.contains(&"search".to_string()));
        assert!(names.contains(&"execute".to_string()));
        assert!(names.contains(&"read_tool_docs".to_string()));

        // search() should surface the downstream `add` tool.
        let search = client
            .call_tool(
                CallToolRequestParams::new("search")
                    .with_arguments(json_obj(serde_json::json!({ "query": "add numbers" }))),
            )
            .await
            .expect("call search");
        let search_text = first_text(&search);
        assert!(search_text.contains("add"), "search result: {search_text}");

        // execute() should run a script that calls the downstream tool.
        let execute = client
            .call_tool(
                CallToolRequestParams::new("execute").with_arguments(json_obj(serde_json::json!({
                    "code": "const r = await skarn.server(\"echo\").add({ a: 40, b: 2 }); return r.sum;"
                }))),
            )
            .await
            .expect("call execute");
        let exec_text = first_text(&execute);
        assert!(exec_text.contains("42"), "execute result: {exec_text}");

        client.cancel().await.ok();
    });
}

fn json_obj(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    match v {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
