# ADR 0005: Build on the official `rmcp` SDK; track the SDK's protocol revision

**Status:** Accepted

## Context

Several Rust MCP gateways roll their own protocol implementation. The official
SDK, `rmcp`, provides both a server (`ServerHandler`) and a client
(`().serve(...)` + `list_all_tools`/`call_tool`) over stdio and Streamable HTTP.
At the time of writing, `rmcp` (1.8) negotiates protocol revision `2025-06-18`
while the published spec has advanced to `2025-11-25`.

## Decision

- **Build on `rmcp`.** Reusing the maintained SDK reduces protocol-drift risk and
  gives us both transports for free. We implement `ServerHandler` by hand (rather
