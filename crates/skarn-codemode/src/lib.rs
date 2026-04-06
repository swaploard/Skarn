//! Hermetic **Code Mode** execution: run sandboxed JS/TS that orchestrates MCP
//! tools without their (often huge) intermediate payloads ever touching the
//! model's context window.
//!
//! The flow:
//! 1. [`validate_and_transpile`] parses the script with `oxc`, rejects anything
//!    that could escape the isolate (`import`/`eval`/`process`/`.constructor`…),
