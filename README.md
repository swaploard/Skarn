<div align="center">

# 🛡️ Skarn

**A fast, OS-sandboxed Model Context Protocol gateway with an embedded Code Mode engine and shell-output token compression — in a single Rust binary.**

[![CI](https://github.com/Rani367/Skarn/actions/workflows/ci.yml/badge.svg)](https://github.com/Rani367/Skarn/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/rustc-1.95+-blue.svg)](https://www.rust-lang.org)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green.svg)](#license)
![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)

*Cut your agent's API bill while physically stopping it from wiping your disk or exfiltrating your secrets.*

</div>

---

## Why

Autonomous AI coding agents have three expensive, dangerous habits:

1. **Token bloat** — they pump raw `cargo test` / `npm install` output and *hundreds* of MCP tool schemas straight into the context window.
2. **The MCP scaling wall** — attach a few MCP servers and the model now carries the JSON Schemas of every tool on every single turn.
3. **Remote code execution by design** — letting an agent run shell commands or LLM-generated code *is* RCE on your machine.

**Skarn** is one binary that fixes all three:

- **Aggregates** your downstream MCP servers behind a tiny, constant tool surface (`search` / `read_tool_docs` / `execute`) using **Code Mode** — the model writes a short script that orchestrates tools in a sandbox, so megabyte intermediate payloads never reach the context window.
- **Compresses** shell output with declarative, per-tool filters — typically **70–90% fewer tokens**, while *guaranteeing* errors and warnings survive.
- **Sandboxes** everything it executes with **OS-native kernel primitives** — Seatbelt on macOS, Landlock + seccomp on Linux, AppContainer on Windows — with **no Docker, no daemon, no VM**.

```
            ┌──────────────────────────── Skarn (one binary) ────────────────────────────┐
            │                                                                                │
  AI agent ─┼─▶  search / execute  ──▶  Code Mode isolate  ──▶  skarn.callTool() ──┐         │
 (Claude    │      (≈1k tokens,            (QuickJS, hermetic,    │                 │         │
  Code,     │       not 30k)               OS-sandboxed)          ▼                 │         │
  Cursor…)  │                                              ┌────────────┐   ┌────────────┐   │
            │   compressed 15-token  ◀──  return summary ──┤ MCP client │…  │ MCP client │   │
            │   result, not a 15k      (big data stays     └─────┬──────┘   └─────┬──────┘   │
            │   log dump                inside the box)          ▼                ▼          │
            └────────────────────────────────────────────── Postgres MCP    GitHub MCP ──────┘
```

## Install

```bash
# One line (macOS / Linux):
curl -fsSL https://raw.githubusercontent.com/Rani367/Skarn/main/install.sh | sh

# Or with Cargo:
cargo install skarn
```

This installs the `skarn` binary.

## Quickstart

```bash
# 1. Scaffold a config and see integration snippets
skarn init

# 2. Check which kernel sandbox is active on your machine
skarn doctor

# 3. Point your agent at the gateway (Claude Code / Cursor / Windsurf):
#    add to your MCP config:
#    { "mcpServers": { "skarn": { "command": "skarn", "args": ["serve"] } } }

# 4. (Optional) try a Code Mode script against your configured servers:
skarn exec --code 'return (await skarn.listTools()).length'

# 5. (Optional) compress + sandbox the agent's shell commands directly:
skarn run --net deny -- cargo test
```

Downstream servers can be local (`transport = "stdio"`) or remote
(`transport = "http"`, Streamable HTTP with optional bearer auth) — see
[`skarn.example.toml`](skarn.example.toml).

## What you get

### 1. Code Mode — give the agent an API, not a schema dump

Instead of injecting every tool's schema, the gateway exposes three meta-tools. The model calls `search()` to find tools, then writes a short script and hands it to `execute()`:

```ts
// The model writes this; Skarn runs it in a hermetic, OS-sandboxed isolate.
const issues = await skarn.server("github").search_issues({ q: "is:open label:bug" });
const stale  = issues.filter(i => daysSince(i.updated_at) > 90);   // filtering happens HERE
await skarn.server("slack").post_message({ channel: "#triage", text: summarize(stale) });
return { staleCount: stale.length };                                // only this returns to the model
```

The 1,000-row intermediate result never touches the context window.

| Scenario | Classic MCP (input tokens) | Skarn Code Mode | Reduction |
|---|---:|---:|---:|
| 16 servers / 508 tools, multi-step task | ~150,000 | ~2,000 | **~99%** |
| Single 3-tool workflow | ~20,700 | ~1,100 | **~95%** |

*(Figures from the published Code Mode literature — see [docs/adr/0001](docs/adr/0001-quickjs-over-deno-core.md). Your mileage varies with catalog size.)*

### 2. Token compression for raw shell output

```bash
skarn run --stats -- cargo test
```

| Command | Raw tokens | Compressed | Reduction |
|---|---:|---:|---:|
| `cargo test` | ~25,000 | ~2,500 | **~90%** |
| `npm install` | ~16,000 | ~3,200 | **~80%** |
| `git diff` | ~10,000 | ~2,500 | **~75%** |
| `ls` / `tree` | ~2,000 | ~400 | **~80%** |

Errors, warnings, and failures are *always* kept — even rescued out of a truncated middle.

### 3. OS-native sandboxing — no Docker required

`skarn run -- <cmd>` confines the command to your project directory and denies network egress, enforced by the kernel:
