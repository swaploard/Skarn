# ADR 0002: Apply the OS sandbox in a child via spawn/`pre_exec`, not in-process

**Status:** Accepted

## Context

To sandbox an *arbitrary* program (e.g. `cargo`, which we do not control) we must
apply the kernel sandbox to that program's process. macOS `sandbox_init` is
