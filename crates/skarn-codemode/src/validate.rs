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
    "Function",
    "require",
    "process",
    "Deno",
    "Bun",
    "global",
    "globalThis",
    "self",
    "window",
    "WebAssembly",
    "importScripts",
    "XMLHttpRequest",
    "module",
    "__dirname",
    "__filename",
    // Reflection / shared-memory primitives: `Reflect.get(x, "constructor")` is a
    // string-keyed hop around the property ban, and Atomics/SharedArrayBuffer are
    // timing/side-channel primitives a Code Mode script never needs.
    "Reflect",
    "Atomics",
    "SharedArrayBuffer",
];

/// Member-access property names that must never appear (reflection escapes).
const BANNED_PROPERTIES: &[&str] = &["constructor", "__proto__", "prototype"];

/// Validate `source`, strip TypeScript, and return runnable JavaScript.
///
/// The source is wrapped in an `async function __skarn_main()` *before* parsing
/// so that a top-level `return value;` in the user's script is legal.
pub fn validate_and_transpile(source: &str) -> Result<String> {
    let wrapped = format!("async function __skarn_main() {{\n{source}\n}}");

    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parsed = Parser::new(&allocator, &wrapped, source_type).parse();

    if parsed.panicked || !parsed.errors.is_empty() {
        let msg = parsed
            .errors
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(Error::CodeModeRejected(format!(
            "syntax error: {}",
            if msg.is_empty() {
                "parse failed".into()
            } else {
                msg
            }
        )));
    }

    // Security walk.
    let mut validator = Validator {
        violations: Vec::new(),
    };
    validator.visit_program(&parsed.program);
    if !validator.violations.is_empty() {
        // De-duplicate while preserving order.
        let mut seen = std::collections::HashSet::new();
        let unique: Vec<String> = validator
            .violations
            .into_iter()
            .filter(|v| seen.insert(v.clone()))
            .collect();
        return Err(Error::CodeModeRejected(unique.join("; ")));
    }

    // Strip TypeScript types -> JavaScript.
    let mut program = parsed.program;
    let scoping = SemanticBuilder::new()
        .build(&program)
        .semantic
        .into_scoping();
    let options = TransformOptions::default();
    let result = Transformer::new(&allocator, Path::new("script.ts"), &options)
        .build_with_scoping(scoping, &mut program);
    if !result.errors.is_empty() {
        let msg = result
            .errors
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(Error::CodeModeRejected(format!("transform error: {msg}")));
    }

    let js = Codegen::new().build(&program).code;
    Ok(js)
}

struct Validator {
    violations: Vec<String>,
}

impl Validator {
    fn flag(&mut self, msg: impl Into<String>) {
        self.violations.push(msg.into());
    }
}

/// Extract a statically-known string from a computed-member key expression: a
/// string literal (`"x"`) or a no-substitution template (`` `x` ``). Returns
/// `None` for dynamic keys, which cannot be reasoned about statically.
fn static_string_key<'b, 'a>(expr: &'b Expression<'a>) -> Option<&'b str> {
    match expr {
        Expression::StringLiteral(lit) => Some(lit.value.as_str()),
        Expression::TemplateLiteral(tpl) if tpl.expressions.is_empty() && tpl.quasis.len() == 1 => {
            tpl.quasis[0].value.cooked.as_ref().map(|c| c.as_str())
        }
        _ => None,
    }
}

impl<'a> Visit<'a> for Validator {
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        if BANNED_IDENTIFIERS.contains(&it.name.as_str()) {
            self.flag(format!("use of forbidden identifier `{}`", it.name));
        }
        walk_identifier_reference(self, it);
    }

    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        if BANNED_PROPERTIES.contains(&it.property.name.as_str()) {
            self.flag(format!(
                "access to forbidden property `.{}`",
                it.property.name
            ));
        }
        walk_static_member_expression(self, it);
    }

    fn visit_computed_member_expression(&mut self, it: &ComputedMemberExpression<'a>) {
        // Bracket access with a statically-known string key (`x["constructor"]`
        // or `` x[`constructor`] ``) is the same reflection hop as dot access, so
        // it must be caught here too.
        if let Some(key) = static_string_key(&it.expression)
            && BANNED_PROPERTIES.contains(&key)
        {
            self.flag(format!("access to forbidden property `[{key:?}]`"));
        }
        walk_computed_member_expression(self, it);
    }

    fn visit_import_declaration(&mut self, _it: &ImportDeclaration<'a>) {
        self.flag("`import` declarations are not allowed in Code Mode scripts");
    }

    fn visit_import_expression(&mut self, it: &ImportExpression<'a>) {
        self.flag("dynamic `import()` is not allowed");
        walk_import_expression(self, it);
    }

    fn visit_export_named_declaration(&mut self, _it: &ExportNamedDeclaration<'a>) {
        self.flag("`export` is not allowed in Code Mode scripts");
    }

    fn visit_export_all_declaration(&mut self, _it: &ExportAllDeclaration<'a>) {
        self.flag("`export` is not allowed in Code Mode scripts");
    }

    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        if let Expression::Identifier(id) = &it.callee
            && id.name.as_str() == "Function"
        {
            self.flag("`new Function(...)` is not allowed");
        }
        walk_new_expression(self, it);
    }

    fn visit_meta_property(&mut self, it: &MetaProperty<'a>) {
        self.flag(format!(
            "meta property `{}.{}` is not allowed",
            it.meta.name, it.property.name
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejected(src: &str) -> String {
        match validate_and_transpile(src) {
            Err(Error::CodeModeRejected(m)) => m,
            Err(e) => panic!("wrong error: {e}"),
            Ok(_) => panic!("expected rejection for: {src}"),
        }
    }

    #[test]
    fn accepts_plain_script() {
        let js = validate_and_transpile("const x = 1 + 2; return x;").unwrap();
        assert!(js.contains("__skarn_main"));
        assert!(js.contains("return"));
    }

    #[test]
    fn strips_typescript_types() {
        let js = validate_and_transpile("const x: number = 1; const y: string = 'a'; return x;")
            .unwrap();
        assert!(!js.contains(": number"));
        assert!(!js.contains(": string"));
    }

    #[test]
    fn rejects_eval_and_aliases() {
        assert!(rejected("eval('1')").contains("eval"));
        assert!(rejected("const e = eval; e('1');").contains("eval"));
    }

    #[test]
    fn rejects_function_constructor() {
        assert!(rejected("const f = new Function('return 1'); f();").contains("Function"));
        assert!(rejected("Function('return 1')()").contains("Function"));
    }

    #[test]
    fn rejects_constructor_hop() {
        assert!(
            rejected("const p = [].constructor.constructor; p('return 1')();")
                .contains("constructor")
        );
    }

    #[test]
    fn rejects_computed_property_hop() {
        // Bracket access with a string/template key must be caught like dot access.
        assert!(rejected(r#"const c = [] ["constructor"]; c;"#).contains("constructor"));
