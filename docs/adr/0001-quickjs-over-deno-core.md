# ADR 0001: Use rquickjs (QuickJS) for the Code Mode engine, not deno_core (V8)

**Status:** Accepted

## Context

The Code Mode engine must execute untrusted, LLM-generated JS/TS that calls
async host functions bridging to MCP tools. The realistic Rust options are
`rquickjs` (QuickJS-NG bindings), `deno_core` (raw V8), and `boa` (pure Rust).

A core product promise is a **small, single, cross-platform binary with no heavy
