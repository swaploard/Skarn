//! Linux backend — Landlock LSM for filesystem/network, seccomp-bpf to deny a
//! curated set of dangerous syscalls.
//!
//! Landlock (kernel 5.13+) lets an *unprivileged* process restrict its own
//! filesystem and network access. We use "best effort" compatibility so that on
//! an older kernel we degrade gracefully (and, if `fail_closed` is set, the
//! caller refuses to run). seccomp adds defense-in-depth by killing syscalls
//! Landlock cannot reason about (`ptrace`, `mount`, `bpf`, module loading, …).
//!
