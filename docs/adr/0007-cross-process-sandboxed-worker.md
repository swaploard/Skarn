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

