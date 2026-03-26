//! Static validation + TypeScript stripping via `oxc`.
//!
//! Before any LLM-generated script runs, we parse it, walk the AST, and reject
//! anything that could escape the hermetic isolate — `import`/`require`/`eval`,
//! `new Function`, `process`/`Deno`, `Reflect`, `.constructor`/`.__proto__`
//! reflection hops, and so on. Because we ban the *identifiers* (not call
