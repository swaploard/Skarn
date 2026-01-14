//! macOS backend — Apple's Seatbelt sandbox via the `sandbox_init(3)` API.
//!
//! `sandbox_init` is deprecated by Apple but remains the only supported way to
//! sandbox a CLI/daemon process and is used in production by Chromium, Cursor,
//! the OpenAI Codex CLI, and Claude Code. We bind it directly through libSystem
//! (no `#[link]` needed) and feed it a generated SBPL (Sandbox Profile
//! Language) policy string.

use std::ffi::{CStr, CString, c_char};

use skarn_common::{Error, Result};

use crate::{Backend, NetPolicy, Policy, RestrictionReport, RestrictionStatus};

// SAFETY: these symbols are provided by libSystem and resolve at link time.
unsafe extern "C" {
    fn sandbox_init(profile: *const c_char, flags: u64, errorbuf: *mut *mut c_char) -> i32;
    fn sandbox_free_error(errorbuf: *mut c_char);
}

/// System directories programs are executed from.
const SYSTEM_EXEC_SUBPATHS: &[&str] = &["/usr/bin", "/bin", "/usr/sbin", "/sbin", "/usr/local/bin"];

