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
    // cache via paths that move between OS releases (Cryptexes, firmlinks, …),
    // so an allow-list of system subpaths reliably breaks `execve` ("works on
    // my macOS version" is not good enough for a security tool). Instead, when
    // system reads are permitted we allow reading the whole filesystem but then
    // explicitly **deny the user's home directory** (where SSH keys, cloud
    // credentials, and dotfiles live) and re-allow only the workspace below.
    //
    // The confidentiality story holds because the real anti-exfiltration
    // control is the network policy (denied by default): even a read of a
    // secret cannot leave the box. Writes remain confined to the workspace.
    if policy.allow_read_system {
        p.push_str("(allow file-read* (subpath \"/\"))\n");
        // Carve out the user's secret stores (and any caller-specified secrets).
        // Workspace paths are re-allowed afterwards, so a project inside a denied
        // tree still works.
        for path in &policy.fs_deny_read {
            p.push_str(&format!(
                "(deny file-read* (subpath {}))\n",
                sbpl_quote(&path.to_string_lossy())
            ));
        }
    }

    // Execution: by default allow exec broadly (the danger we guard against is
    // writes / network / reading secrets, not exec itself), but if the caller
    // pinned an exec allow-list, honor it strictly.
    if policy.fs_exec.is_empty() {
        p.push_str("(allow process-exec*)\n");
    } else {
        p.push_str("(allow process-exec*\n");
        for sub in SYSTEM_EXEC_SUBPATHS {
            p.push_str(&format!("  (subpath {})\n", sbpl_quote(sub)));
        }
        for path in &policy.fs_exec {
            p.push_str(&format!(
                "  (subpath {})\n",
                sbpl_quote(&path.to_string_lossy())
            ));
        }
        p.push_str(")\n");
    }

    // Readable subtrees.
    if !policy.fs_read.is_empty() || !policy.fs_read_write.is_empty() {
        p.push_str("(allow file-read*\n");
        for path in policy.fs_read.iter().chain(policy.fs_read_write.iter()) {
            p.push_str(&format!(
                "  (subpath {})\n",
                sbpl_quote(&path.to_string_lossy())
            ));
        }
        p.push_str(")\n");
    }

    // Writable subtrees (also implies create/unlink within them).
    if !policy.fs_read_write.is_empty() {
        p.push_str("(allow file-write*\n");
        for path in &policy.fs_read_write {
            p.push_str(&format!(
                "  (subpath {})\n",
                sbpl_quote(&path.to_string_lossy())
            ));
        }
        p.push_str(")\n");
    }
