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

/// Generate the SBPL profile string for a policy.
///
/// This is a pure function so it can be unit-tested directly without touching
/// the kernel.
pub fn profile_sbpl(policy: &Policy) -> String {
    let policy = policy.canonicalized();
    let mut p = String::with_capacity(1024);
    p.push_str("(version 1)\n");
    p.push_str("(deny default)\n");

    // Baseline capabilities a normal process needs just to run.
    p.push_str("(allow process-fork)\n");
    p.push_str("(allow signal (target self))\n");
    p.push_str("(allow sysctl-read)\n");
    p.push_str("(allow mach-lookup)\n");
    p.push_str("(allow file-read-metadata)\n");

    // Filesystem reads.
    //
    // On modern macOS the dynamic loader pulls libraries from the dyld shared
