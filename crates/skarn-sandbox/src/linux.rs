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
