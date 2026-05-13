# ADR 0004: Namespace downstream tools and expose a constant meta-tool surface

**Status:** Accepted

## Context

MCP tool names are only unique *per server*, so two aggregated servers can both
expose `search`. The MCP spec does not define aggregation, namespacing, or
discovery — a gateway must provide them. Exposing every downstream tool's schema
upstream also reintroduces exactly the context bloat we set out to remove.

## Decision

1. **Namespacing.** Each downstream tool is exposed as `"<server><sep><tool>"`
   (default separator `__`, configurable; both `__` and `.` are legal in the MCP
   name charset). A reverse map (`Registry::resolve`) restores `(server, tool)`
   for routing. Names stay within the 128-char limit.
2. **Constant upstream surface.** Regardless of catalog size, the gateway exposes
   exactly three meta-tools — `search`, `read_tool_docs`, `execute` — plus, only
   if `passthrough = true`, the namespaced tools directly. This keeps the
   per-turn schema footprint ~constant.
3. **Progressive disclosure.** The full tool catalog is reachable via `search`
   (ranked) and `read_tool_docs` (exact schema on demand). A generated
