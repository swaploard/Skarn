# ADR 0003: Run the Code Mode isolate on a dedicated thread, bridged by channels

**Status:** Accepted

## Context

The QuickJS `AsyncRuntime` is `!Send`. The MCP clients (built on `rmcp`) and
their child-process transports must be polled — *and dropped* — on the
