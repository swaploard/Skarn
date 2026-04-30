# Architecture Decision Records

These document the significant engineering choices behind Skarn and the
reasoning (including the trade-offs we accepted).

| # | Decision |
|---|---|
| [0001](0001-quickjs-over-deno-core.md) | Use rquickjs (QuickJS) for Code Mode, not deno_core (V8) |
| [0002](0002-spawn-based-sandbox-model.md) | Apply the OS sandbox in a child via spawn/`pre_exec` |
