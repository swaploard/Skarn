//! Static validation + TypeScript stripping via `oxc`.
//!
//! Before any LLM-generated script runs, we parse it, walk the AST, and reject
//! anything that could escape the hermetic isolate — `import`/`require`/`eval`,
//! `new Function`, `process`/`Deno`, `Reflect`, `.constructor`/`.__proto__`
//! reflection hops, and so on. Because we ban the *identifiers* (not call
//! sites), alias hops like `const e = eval; e("…")` are caught too: `eval`
//! appears in the AST regardless of how it is later used. Banned property names
//! are caught whether accessed with dot (`x.constructor`) or bracket notation
//! (`x["constructor"]`, `` x[`constructor`] ``).
//!
//! This static pass is **defense in depth**, not the security boundary. The real
//! guarantees are (1) the hermetic QuickJS context — no filesystem, network, or
//! `fetch` bindings, so even arbitrary in-isolate code cannot reach the host —
//! and (2) the OS-native sandbox the execution host runs under. The validator
//! exists to reject obviously hostile scripts early with a clear message.
//!
//! If validation passes, the TypeScript types are stripped and the result is
//! emitted as plain JavaScript for QuickJS.

use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_ast_visit::walk::{
    walk_computed_member_expression, walk_identifier_reference, walk_import_expression,
    walk_new_expression, walk_static_member_expression,
};
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_transformer::{TransformOptions, Transformer};
use skarn_common::{Error, Result};

/// Identifiers that must never appear in a script.
const BANNED_IDENTIFIERS: &[&str] = &[
    "eval",
