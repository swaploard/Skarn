# ADR 0002: Apply the OS sandbox in a child via spawn/`pre_exec`, not in-process

**Status:** Accepted

## Context

To sandbox an *arbitrary* program (e.g. `cargo`, which we do not control) we must
apply the kernel sandbox to that program's process. macOS `sandbox_init` is
**per-process and irreversible**; Linux Landlock is per-thread but inherited
across `execve`; Windows AppContainer can only be applied at process creation.

A second hazard: applying a non-async-signal-safe function (like `sandbox_init`,
which allocates) in a `pre_exec` hook after `fork()` in a *multi-threaded* parent
can deadlock on the allocator lock held by another thread at fork time.

## Decision

- **Unix (`skarn run`):** spawn the target with `std::process::Command` and a
  `pre_exec` closure that calls `Policy::apply_to_current_process()` — Seatbelt on
  macOS, Landlock + seccomp on Linux. The confinement persists across `execve`.
  The `run` command path is kept **single-threaded** (no async runtime) so the
  parent is single-threaded at fork time, avoiding the deadlock.
- **Windows:** the parent launches the child directly into an AppContainer via
  `CreateProcessW` (a process cannot move itself into one).
- The same `Policy::apply_to_current_process()` is exercised directly by the
  `skarn-sandbox-probe` test binary, which self-applies in a fresh, single-
  threaded process.
