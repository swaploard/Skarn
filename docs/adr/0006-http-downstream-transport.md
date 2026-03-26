# ADR 0006: Add a Streamable HTTP downstream transport

**Status:** Accepted

## Context

The gateway initially aggregated only stdio child-process MCP servers. Many
real-world servers are remote and speak MCP over HTTP. The MCP spec defines a
