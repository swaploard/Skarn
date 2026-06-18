# ADR 0001: Use rquickjs (QuickJS) for the Code Mode engine, not deno_core (V8)

**Status:** Accepted

## Context

The Code Mode engine must execute untrusted, LLM-generated JS/TS that calls
async host functions bridging to MCP tools. The realistic Rust options are
`rquickjs` (QuickJS-NG bindings), `deno_core` (raw V8), and `boa` (pure Rust).

A core product promise is a **small, single, cross-platform binary with no heavy
toolchain** — so embedding cost matters as much as raw throughput.

## Decision

Use **`rquickjs`** with the `futures` + `macro` features.

## Rationale

- **Binary size & build:** QuickJS adds ~1 MB and bundles its C source compiled
  via `cc` (no libclang/bindgen needed on common targets). `deno_core` pulls in
  the `v8` crate, producing 50–90 MB binaries with a heavy, cross-compile-hostile
  build — directly at odds with the project's goals.
- **Async bridging is first-class:** `AsyncRuntime`/`AsyncContext` plus the
  `Async`/`Promised` adapters convert Rust futures to JS promises and back, which
  is exactly the `skarn.callTool(...)` pattern we need.
- **Hard limits exist:** `set_memory_limit`, `set_max_stack_size`, and an
  interrupt handler (wall-clock deadline → uncatchable abort) cover the
  untrusted-execution requirements.
- **Startup:** fresh contexts are cheap, which suits a per-execution isolate.

`boa` (pure Rust) remains an attractive portability escape hatch but is slower
and has weaker untrusted-execution controls today; it can return behind a feature
flag if a no-C-compiler target ever requires it.

## Consequences

- We get a tiny, statically-linkable, cross-compilable engine.
