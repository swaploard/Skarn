# ADR 0004: Namespace downstream tools and expose a constant meta-tool surface

**Status:** Accepted

## Context

MCP tool names are only unique *per server*, so two aggregated servers can both
expose `search`. The MCP spec does not define aggregation, namespacing, or
discovery — a gateway must provide them. Exposing every downstream tool's schema
upstream also reintroduces exactly the context bloat we set out to remove.

## Decision

