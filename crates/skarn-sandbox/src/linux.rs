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
        libc::SYS_bpf,
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
        libc::SYS_reboot,
        libc::SYS_swapon,
        libc::SYS_swapoff,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_pivot_root,
        libc::SYS_setns,
        libc::SYS_unshare,
    ]
}

pub fn apply(policy: &Policy) -> Result<RestrictionReport> {
    let policy = policy.canonicalized();
    let abi = ABI::V5; // FS read/write/exec + network (V4) + ioctl_dev (V5)

    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| Error::sandbox(format!("landlock handle fs: {e}")))?;

    // Landlock network filtering is port-based and cannot express "loopback
    // only", so we only enforce it for the strict DenyAll case. AllowLoopback
    // degrades to "unrestricted, with a note" (see below).
    let restrict_net = matches!(policy.net, NetPolicy::DenyAll);
    if restrict_net {
        ruleset = ruleset
            .handle_access(AccessNet::from_all(abi))
            .map_err(|e| Error::sandbox(format!("landlock handle net: {e}")))?;
    }

    let mut created = ruleset
        .create()
        .map_err(|e| Error::sandbox(format!("landlock create: {e}")))?;

    let read = AccessFs::from_read(abi);
    let read_write = AccessFs::from_read(abi) | AccessFs::from_write(abi);

    // System paths legitimately vary across distros/arches (e.g. no `/lib64`),
    // so their skips are not worth reporting; user-requested paths are.
    let mut sys_skipped = Vec::new();
    let mut skipped = Vec::new();

    // System read + exec.
    if policy.allow_read_system {
        for path in SYSTEM_READ {
            created = add_path_rule(created, path, read, &mut sys_skipped)?;
        }
        for path in SYSTEM_DEV_READ {
            created = add_path_rule(created, path, read | AccessFs::WriteFile, &mut sys_skipped)?;
        }
        created = add_path_rule(created, PROC_SELF, read, &mut sys_skipped)?;
    }

    for path in &policy.fs_read {
        created = add_path_rule(created, &path.to_string_lossy(), read, &mut skipped)?;
    }
