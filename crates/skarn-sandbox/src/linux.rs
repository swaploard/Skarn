//! Linux backend — Landlock LSM for filesystem/network, seccomp-bpf to deny a
//! curated set of dangerous syscalls.
//!
//! Landlock (kernel 5.13+) lets an *unprivileged* process restrict its own
//! filesystem and network access. We use "best effort" compatibility so that on
//! an older kernel we degrade gracefully (and, if `fail_closed` is set, the
//! caller refuses to run). seccomp adds defense-in-depth by killing syscalls
//! Landlock cannot reason about (`ptrace`, `mount`, `bpf`, module loading, …).
//!
//! Note: Landlock network filtering is *port*-based — it cannot distinguish
//! loopback from the internet — so [`NetPolicy::AllowLoopback`] degrades to
//! "allow outbound" here and a note is attached to the report.

use landlock::{
    ABI, Access, AccessFs, AccessNet, BitFlags, CompatLevel, Compatible, PathBeneath, PathFd,
    Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus,
};
use skarn_common::{Error, Result};

use crate::{Backend, NetPolicy, Policy, RestrictionReport, RestrictionStatus};

/// System directories programs need to read/execute to start.
const SYSTEM_READ: &[&str] = &["/usr", "/lib", "/lib64", "/bin", "/sbin", "/etc"];
const SYSTEM_DEV_READ: &[&str] = &["/dev/null", "/dev/zero", "/dev/random", "/dev/urandom"];
const PROC_SELF: &str = "/proc/self";

/// Syscalls we deny outright via seccomp regardless of Landlock support.
/// These are operations a sandboxed code/command runner never legitimately
/// needs, and which are common privilege-escalation / escape primitives.
fn dangerous_syscalls() -> &'static [libc::c_long] {
    &[
        libc::SYS_ptrace,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_init_module,
        libc::SYS_finit_module,
        libc::SYS_delete_module,
        libc::SYS_kexec_load,
