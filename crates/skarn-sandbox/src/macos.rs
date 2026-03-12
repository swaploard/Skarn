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

    // /dev/null & friends are needed for almost everything; allow read+write.
    p.push_str(
        "(allow file-write-data file-read-data\n  (literal \"/dev/null\")\n  (literal \"/dev/zero\")\n  (literal \"/dev/random\")\n  (literal \"/dev/urandom\"))\n",
    );

    // Network.
    match policy.net {
        NetPolicy::DenyAll => { /* denied by default */ }
        NetPolicy::AllowLoopback => {
            p.push_str("(allow network* (local ip \"localhost:*\") (remote ip \"localhost:*\"))\n");
            p.push_str("(allow network-bind (local ip \"localhost:*\"))\n");
        }
        NetPolicy::AllowOutbound => {
            p.push_str("(allow network-outbound)\n");
            p.push_str("(allow network-inbound (local ip \"localhost:*\"))\n");
        }
        NetPolicy::AllowAll => {
            p.push_str("(allow network*)\n");
        }
    }

    p
}

/// Quote a string for SBPL (double-quoted, backslash-escaped).
fn sbpl_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// Apply the policy to the current process via `sandbox_init`.
pub fn apply(policy: &Policy) -> Result<RestrictionReport> {
    let profile = profile_sbpl(policy);
    let c_profile = CString::new(profile.clone())
        .map_err(|_| Error::sandbox("SBPL profile contained an interior NUL byte"))?;

    let mut errbuf: *mut c_char = std::ptr::null_mut();
    // SAFETY: `c_profile` is a valid NUL-terminated C string; `errbuf` is a
    // valid out-pointer. `sandbox_init` either returns 0 (success, *errbuf left
    // NULL) or non-zero and allocates an error string we must free.
    let rc = unsafe { sandbox_init(c_profile.as_ptr(), 0, &mut errbuf) };
    if rc != 0 {
        let msg = if errbuf.is_null() {
            format!("sandbox_init failed (rc={rc})")
        } else {
            // SAFETY: non-null errbuf points to a NUL-terminated string owned by
            // the sandbox library; we copy it then free it.
            let m = unsafe { CStr::from_ptr(errbuf) }
                .to_string_lossy()
                .into_owned();
            unsafe { sandbox_free_error(errbuf) };
            m
        };
        return Err(Error::Sandbox(format!("sandbox_init: {msg}")));
    }

    Ok(
        RestrictionReport::new(Backend::Seatbelt, RestrictionStatus::FullyEnforced)
            .note("Seatbelt profile applied to current process via sandbox_init"),
    )
}

/// Probe support without applying. On macOS `sandbox_init` is always present, so
/// the only question is whether we are on macOS at all (we are, by cfg).
pub fn probe() -> RestrictionReport {
    RestrictionReport::new(Backend::Seatbelt, RestrictionStatus::FullyEnforced)
        .note("macOS Seatbelt (sandbox_init) available")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_default_present() {
        let p = Policy::builder().workspace("/tmp").build();
        let sbpl = profile_sbpl(&p);
        assert!(sbpl.starts_with("(version 1)\n(deny default)"));
    }

    #[test]
    fn workspace_is_writable() {
        // /tmp exists, so canonicalization keeps it.
        let p = Policy::builder().workspace("/tmp").build();
        let sbpl = profile_sbpl(&p);
        assert!(sbpl.contains("(allow file-write*"));
        assert!(sbpl.contains("/tmp") || sbpl.contains("/private/tmp"));
    }

    #[test]
    fn deny_all_omits_network_allow() {
        let p = Policy::builder().workspace("/tmp").build();
        let sbpl = profile_sbpl(&p);
        assert!(!sbpl.contains("(allow network"));
    }

    #[test]
    fn allow_all_network() {
        let p = Policy::builder()
            .workspace("/tmp")
            .net(NetPolicy::AllowAll)
            .build();
        let sbpl = profile_sbpl(&p);
        assert!(sbpl.contains("(allow network*)"));
    }

    #[test]
    fn quoting_escapes_quotes_and_backslashes() {
        assert_eq!(sbpl_quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
