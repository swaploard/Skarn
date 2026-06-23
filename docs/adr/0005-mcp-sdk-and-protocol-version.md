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
  than via the attribute macros) for full control over the small meta-tool set.
- **Track the SDK's negotiated revision** rather than hand-rolling a newer one.
  Whatever revision `rmcp` advertises is what Skarn advertises; when `rmcp`
  adds `2025-11-25`, we get it on upgrade. We do not forge a protocol version the
  SDK cannot actually speak.
- **Do not enable the `local` feature.** It relaxes `Send` for a single-threaded
  deployment, but it made running the `!Send` Code Mode isolate alongside live
  MCP clients fragile (see ADR 0003). We run multi-threaded and isolate the
  engine on its own thread instead.

## Consequences

- We inherit `rmcp`'s transport support and protocol upgrades.
- Skarn's advertised protocol version lags the very newest spec until the SDK
  catches up — an acceptable, clearly-scoped trade-off documented here and
  surfaced by `skarn doctor`.
