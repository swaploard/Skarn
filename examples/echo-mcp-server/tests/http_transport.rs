//! Integration test for the Streamable HTTP downstream transport.
//!
//! Stands up [`EchoServer`] behind an rmcp `StreamableHttpService` on an
//! ephemeral loopback port, then connects to it through the gateway's `http`
