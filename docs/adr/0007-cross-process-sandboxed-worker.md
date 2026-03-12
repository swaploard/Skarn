# ADR 0007: Run Code Mode in a cross-process OS-sandboxed worker

**Status:** Accepted

## Context

`skarn serve`'s `execute` originally ran the Code Mode isolate **in-process**:
