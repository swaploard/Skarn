# ADR 0006: Add a Streamable HTTP downstream transport

**Status:** Accepted

## Context

The gateway initially aggregated only stdio child-process MCP servers. Many
real-world servers are remote and speak MCP over HTTP. The MCP spec defines a
**Streamable HTTP** transport (a single endpoint that handles JSON request /
response and an SSE stream for server-initiated messages); `rmcp` 1.8 implements
the client side as `StreamableHttpClientTransport` and consumes the SSE stream
internally. There is no separate "SSE client" transport in `rmcp` 1.8 — the
Streamable HTTP transport is the one client surface.

## Decision

- **Add one `TransportConfig::Http` variant** (serde tag `"http"`) alongside
  `Stdio`, with `url`, `auth_bearer`, `auth_bearer_env`, and `headers`. Tokens are
  preferentially read from an environment variable (`auth_bearer_env`) so secrets
  stay out of `skarn.toml`.
- **Keep the rest of the manager transport-agnostic.** Both transports erase to
  `RunningService<RoleClient, ()>`, so only `connect_one` branches; aggregation,
  search, `call`, and resource reads are unchanged.
- **Use reqwest + rustls with the `ring` provider** (not aws-lc-rs): a smaller,
  more cross-compile-friendly crypto backend with no cmake/C build, keeping the
  single-binary goal and a permissive license tree. The provider is installed as
  the process default before the first HTTPS request.

## Consequences

- Remote MCP servers can be aggregated exactly like local ones.
