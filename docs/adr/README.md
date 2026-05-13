# Architecture Decision Records

These document the significant engineering choices behind Skarn and the
reasoning (including the trade-offs we accepted).

| # | Decision |
|---|---|
| [0001](0001-quickjs-over-deno-core.md) | Use rquickjs (QuickJS) for Code Mode, not deno_core (V8) |
| [0002](0002-spawn-based-sandbox-model.md) | Apply the OS sandbox in a child via spawn/`pre_exec` |
| [0003](0003-code-mode-thread-isolation.md) | Run the Code Mode isolate on a dedicated thread, bridged by channels |
| [0004](0004-tool-namespacing-and-manifest.md) | Namespace downstream tools; expose a constant meta-tool surface |
