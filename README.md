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
