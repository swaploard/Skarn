# ADR 0005: Build on the official `rmcp` SDK; track the SDK's protocol revision

**Status:** Accepted

## Context

Several Rust MCP gateways roll their own protocol implementation. The official
SDK, `rmcp`, provides both a server (`ServerHandler`) and a client
(`().serve(...)` + `list_all_tools`/`call_tool`) over stdio and Streamable HTTP.
