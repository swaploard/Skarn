//! macOS backend — Apple's Seatbelt sandbox via the `sandbox_init(3)` API.
//!
//! `sandbox_init` is deprecated by Apple but remains the only supported way to
//! sandbox a CLI/daemon process and is used in production by Chromium, Cursor,
//! the OpenAI Codex CLI, and Claude Code. We bind it directly through libSystem
//! (no `#[link]` needed) and feed it a generated SBPL (Sandbox Profile
//! Language) policy string.
