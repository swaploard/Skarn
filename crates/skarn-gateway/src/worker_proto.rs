//! The wire protocol between the gateway (parent) and the sandboxed Code Mode
//! worker (child).
//!
//! Framing is newline-delimited JSON. The parent writes a single [`JobMsg`] to
