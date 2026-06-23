//! Integration test for the Streamable HTTP downstream transport.
//!
//! Stands up [`EchoServer`] behind an rmcp `StreamableHttpService` on an
//! ephemeral loopback port, then connects to it through the gateway's `http`
//! transport and exercises tool listing + a tool call — the same downstream
//! surface the stdio transport provides, proving the manager is transport-
//! agnostic.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use echo_mcp_server::EchoServer;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use skarn_gateway::{
    DownstreamManager, GatewayConfig, GatewaySettings, ServerConfig, TransportConfig,
};

#[tokio::test(flavor = "multi_thread")]
async fn http_transport_lists_and_calls_tools() {
    // Host EchoServer over Streamable HTTP on an ephemeral loopback port.
    let service = StreamableHttpService::new(
        || Ok::<_, std::io::Error>(EchoServer),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let app = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    // Give the accept loop a moment to start.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Point the gateway's `http` transport at it.
    let mut servers = BTreeMap::new();
    servers.insert(
        "echo".to_string(),
        ServerConfig {
            enabled: true,
            transport: TransportConfig::Http {
                url: format!("http://{addr}/mcp"),
                auth_bearer: None,
                auth_bearer_env: None,
                headers: BTreeMap::new(),
            },
        },
    );
    let config = GatewayConfig {
        gateway: GatewaySettings::default(),
        servers,
    };

    let manager = DownstreamManager::connect(&config).await.unwrap();

    // The downstream tools are aggregated just like over stdio.
    let tools: Vec<String> = manager
        .registry()
        .tools()
        .iter()
        .map(|t| t.tool.clone())
        .collect();
    assert!(tools.contains(&"add".to_string()), "tools: {tools:?}");
    assert!(tools.contains(&"echo".to_string()), "tools: {tools:?}");

    // And a tool call round-trips over HTTP.
    let result = manager
        .call("echo", "add", r#"{"a":40,"b":2}"#)
        .await
        .unwrap();
    assert!(result.contains("\"sum\":42"), "result: {result}");
}
