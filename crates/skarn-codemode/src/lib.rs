//! Hermetic **Code Mode** execution: run sandboxed JS/TS that orchestrates MCP
//! tools without their (often huge) intermediate payloads ever touching the
//! model's context window.
//!
//! The flow:
//! 1. [`validate_and_transpile`] parses the script with `oxc`, rejects anything
//!    that could escape the isolate (`import`/`eval`/`process`/`.constructor`…),
//!    and strips TypeScript types.
//! 2. [`Engine::run`] executes the result inside a QuickJS isolate (via
//!    `rquickjs`) whose only egress is a [`ToolBridge`]. Memory, stack, wall
//!    clock, tool-call count, and output size are all bounded.
//! 3. The script returns a small value (and `skarn.log` lines); everything else
//!    stays in the sandbox.
//!
//! In production the isolate runs inside an OS-native sandbox (see
//! `skarn-sandbox`) for defense in depth, and the [`ToolBridge`] forwards calls
//! over a pipe to the parent gateway.

mod bridge;
