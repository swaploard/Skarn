# ADR 0006: Add a Streamable HTTP downstream transport

**Status:** Accepted

## Context

The gateway initially aggregated only stdio child-process MCP servers. Many
real-world servers are remote and speak MCP over HTTP. The MCP spec defines a
**Streamable HTTP** transport (a single endpoint that handles JSON request /
response and an SSE stream for server-initiated messages); `rmcp` 1.8 implements
the client side as `StreamableHttpClientTransport` and consumes the SSE stream
internally. There is no separate "SSE client" transport in `rmcp` 1.8 — the
Streamable HTTP transport is the one client surface.

## Decision

- **Add one `TransportConfig::Http` variant** (serde tag `"http"`) alongside
