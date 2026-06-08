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

