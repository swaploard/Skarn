# ADR 0007: Run Code Mode in a cross-process OS-sandboxed worker

**Status:** Accepted

## Context

`skarn serve`'s `execute` originally ran the Code Mode isolate **in-process**:
the hermetic QuickJS context shares the gateway's address space, protected only
by static validation and the absence of any dangerous bindings. SECURITY.md
listed a dedicated OS-sandboxed worker subprocess as the next hardening step: a
hypothetical isolate escape would then land in a kernel-confined process rather
than the gateway itself. The `ToolBridge` trait was always the seam for this —
its docs anticipated bridge calls being "forwarded over a pipe to the parent."

## Decision

- **Add a hidden `skarn __worker` subcommand.** It reads a job (policy + limits +
  script) from stdin, calls `Policy::apply_to_current_process()` to confine
  *itself* (deny network, no workspace writes — the isolate needs neither), then
  runs the isolate, bridging each `skarn.callTool` back to the parent over its
  stdio pipes as newline-delimited JSON (`worker_proto`). This mirrors the
  existing in-process channel servicer, with the OS process boundary replacing
  the dedicated-thread boundary of ADR 0003.
- **Gate selection on an `isolation` setting** (`auto` | `worker` | `in_process`,
  default `auto`). `auto` uses the worker when an OS sandbox backend is available.
- **Scope the worker to Unix for 1.0.** The worker self-applies the sandbox,
  which a process can do on macOS (Seatbelt) and Linux (Landlock + seccomp) but
  not on Windows (a process cannot move *itself* into an AppContainer). On Windows
  `execute` uses the in-process hermetic isolate; AppContainer is instead wired
  into `skarn run` for shell commands (where the parent launches the child).

## Consequences

- On macOS/Linux, in-gateway `execute` gains a second, kernel-enforced isolation
