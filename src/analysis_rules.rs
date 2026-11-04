// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Syntax-backed local style checks. Semantic uncertainty is reported
//! explicitly.

use std::path::Path;

use proc_macro2::Span;
use quote::ToTokens;
use syn::Attribute;
use syn::Meta;
use syn::visit::Visit;

use self::internal::inspector::Inspector;
use self::internal::literal_lines::LiteralLines;
use crate::analysis::Finding;
use crate::style_exceptions::ExceptionConfig;

// Holds source-local traversal and literal-span implementation details.
mod internal;

/// Analyzes local syntax without expanding macros or guessing name resolution.
#[cfg(test)]
pub(crate) fn inspect(path: &str, source: &str, ast: &syn::File, exceptions: &ExceptionConfig) -> Vec<Finding> {
    inspect_with_role(path, source, ast, exceptions, None)
}

/// Analyzes a file using Cargo-resolved production classification when
/// available.
#[cfg(test)]
pub(crate) fn inspect_with_role(
    path: &str,
    source: &str,
    ast: &syn::File,
    exceptions: &ExceptionConfig,
    production_override: Option<bool>,
) -> Vec<Finding> {
    inspect_with_context(path, source, ast, exceptions, production_override, None)
}

/// Analyzes syntax using explicitly resolved production and test roles.
///
/// Source and AST inputs are borrowed immutably; all findings retain their
/// source positions and distinguish proved violations from unresolved semantic
/// choices.
///
/// # Parameters
///
/// * `path` - Project-relative source path included in every finding.
/// * `source` - Complete source text corresponding to the parsed AST.
/// * `ast` - Parsed Rust file retaining original source spans.
/// * `exceptions` - Validated exact-path style exceptions.
/// * `production_override` - Cargo-resolved production role, or path inference.
/// * `test_override` - Cargo-resolved test role, or path inference.
///
/// # Returns
///
/// Returns deterministic errors and explicit semantic review signals.
pub(crate) fn inspect_with_context(
    path: &str,
    source: &str,
    ast: &syn::File,
    exceptions: &ExceptionConfig,
    production_override: Option<bool>,
    test_override: Option<bool>,
) -> Vec<Finding> {
    let components: Vec<_> = path.split('/').collect();
    let test_tree = test_override.unwrap_or_else(|| components.contains(&"tests"));
    let production = components.contains(&"src")
        && !test_tree
        && !components.iter().any(|p| matches!(*p, "benches" | "fuzz" | "examples"));
    let production = production_override.unwrap_or(production);
    let support = components
        .iter()
        .any(|p| matches!(*p, "support" | "common" | "fixtures"));
    let mut literals = LiteralLines::default();
    literals.collect(ast.to_token_stream());
    let allow_coverage_comment = source.lines().enumerate().any(|(index, line)| {
        line.trim_start().starts_with("//")
            && line.contains("qubit-style: allow coverage-cfg")
            && !literals
                .0
                .iter()
                .any(|(start, end)| *start <= index + 1 && index < *end)
    });
    let mut inspector = Inspector {
        path,
        source,
        exceptions,
        findings: Vec::new(),
        production,
        test_context: test_tree || is_test_attrs(&ast.attrs),
        support,
        module_depth: 0,
        allow_coverage_comment,
        types: Vec::new(),
        conditional: has_cfg(&ast.attrs),
    };
    if test_tree && !exceptions.allows("test-file-name", path) {
        let concrete_tests = concrete_test_items(&ast.items);
        let name = Path::new(path).file_name().and_then(|x| x.to_str()).unwrap_or("");
        if !matches!(name, "mod.rs" | "main.rs") && !name.ends_with("_tests.rs") {
            inspector.issue(
                "TEST-003",
                "test-file-name",
                if support || !concrete_tests { "review" } else { "error" },
                Span::call_site(),
                name,
                (
                    "Concrete test files must end in _tests.rs.",
                    "Rename the test module and update its module declarations.",
                ),
            );
        }
    }
    if production
        && !test_tree
        && Path::new(path).file_name().is_some_and(|name| name == "mod.rs")
        && !exceptions.allows("explicit-imports", path)
        && ast
            .items
            .iter()
            .all(|item| matches!(item, syn::Item::Mod(_) | syn::Item::Use(_) | syn::Item::ExternCrate(_)))
    {
        for item in &ast.items {
            if let syn::Item::Use(import) = item
                && matches!(import.vis, syn::Visibility::Inherited)
            {
                inspector.issue(
                    "IMPORT-001",
                    "aggregation-private-imports",
                    if has_cfg(&import.attrs) { "review" } else { "error" },
                    import.use_token.span,
                    &import.to_token_stream().to_string(),
                    (
                        "Pure mod.rs aggregation files must re-export imports with explicit visibility.",
                        "Use a visible re-export or record a reasoned explicit-imports exception.",
                    ),
                );
            }
        }
    }
    inspector.check_layout(&ast.items, true);
    inspector.visit_file(ast);
    if inspector.types.len() > 1
        && !exceptions.allows("public-type-layout", path)
        && !exceptions.allows("multiple-public-types", path)
    {
        let conditional = inspector.types.iter().any(|(_, _, cfg)| *cfg);
        let span = inspector.types[1].1;
        let evidence = inspector
            .types
            .iter()
            .map(|(name, _, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        inspector.issue(
            "ORG-001",
            "one-type-per-file",
            if conditional { "review" } else { "error" },
            span,
            &evidence,
            (
                if conditional {
                    "Conditional production types require configuration-aware ownership review."
                } else {
                    "Multiple production types share one source file."
                },
                "Place each struct, enum and trait in a separate snake_case file.",
            ),
        );
    }
    inspector.findings
}
/// Identifies concrete test entry points without mistaking helper functions for
/// tests.
///
/// # Parameters
///
/// * `items` - Parsed declarations, including nested inline modules.
fn concrete_test_items(items: &[syn::Item]) -> bool {
    items.iter().any(|item| match item {
        syn::Item::Fn(function) => is_test_function(&function.attrs),
        syn::Item::Mod(module) => module
            .content
            .as_ref()
            .is_some_and(|(_, items)| concrete_test_items(items)),
        _ => false,
    })
}

/// Detects a brace list or wildcard anywhere within one use tree.
///
/// # Parameters
///
/// * `tree` - Parsed import tree whose path prefixes may contain nested
///   imports.
fn contains_group_or_glob(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Group(_) | syn::UseTree::Glob(_) => true,
        syn::UseTree::Path(path) => contains_group_or_glob(&path.tree),
        _ => false,
    }
}
/// Classifies an import root using the normative standard/external/current
/// order.
///
/// # Parameters
///
/// * `tree` - Parsed import whose leading identifier selects its group.
fn import_group(tree: &syn::UseTree) -> u8 {
    let ident = match tree {
        syn::UseTree::Path(x) => x.ident.to_string(),
        syn::UseTree::Name(x) => x.ident.to_string(),
        syn::UseTree::Rename(x) => x.ident.to_string(),
        _ => return 1,
    };
    match ident.as_str() {
        "std" | "core" | "alloc" => 0,
        "crate" | "self" | "super" => 2,
        _ => 1,
    }
}
/// Parses comma-separated nested metadata without evaluating conditions.
///
/// # Parameters
///
/// * `meta` - Attribute metadata that may contain a parenthesized argument
///   list.
fn meta_children(meta: &Meta) -> Vec<Meta> {
    if let Meta::List(list) = meta {
        use syn::parse::Parser;
        syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated
            .parse2(list.tokens.clone())
            .map(|x| x.into_iter().collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}
/// Finds an attribute introduced by nested cfg_attr metadata.
///
/// # Parameters
///
/// * `meta` - Metadata that may conditionally apply another attribute.
/// * `name` - Exact attribute identifier being searched for.
fn conditional_meta(meta: &Meta, name: &str) -> bool {
    meta.path().is_ident("cfg_attr") && meta_children(meta).iter().skip(1).any(|m| meta_has_attr(m, name))
}
/// Recognizes a direct attribute or its conditional equivalent.
///
/// # Parameters
///
/// * `meta` - Metadata to inspect without expanding procedural attributes.
/// * `name` - Exact attribute identifier required by the caller.
fn meta_has_attr(meta: &Meta, name: &str) -> bool {
    meta.path().is_ident(name) || conditional_meta(meta, name)
}
/// Recognizes unused_imports suppression, including conditional suppression.
///
/// # Parameters
///
/// * `meta` - Parsed metadata whose allow arguments are checked recursively.
fn prohibited_unused_imports(meta: &Meta) -> bool {
    (meta.path().is_ident("allow") && meta_children(meta).iter().any(|m| m.path().is_ident("unused_imports")))
        || (meta.path().is_ident("cfg_attr") && meta_children(meta).iter().skip(1).any(prohibited_unused_imports))
}
/// Recognizes coverage-specific names in parsed cfg metadata.
///
/// # Parameters
///
/// * `meta` - Cfg or cfg_attr syntax containing potential coverage predicates.
fn is_coverage_cfg(meta: &Meta) -> bool {
    if !matches!(
        meta.path().get_ident().map(ToString::to_string).as_deref(),
        Some("cfg" | "cfg_attr")
    ) {
        return false;
    }
    /// Finds known coverage configuration identifiers in nested predicates.
    ///
    /// # Parameters
    ///
    /// * `meta` - Predicate or nested metadata being searched recursively.
    fn contains(meta: &Meta) -> bool {
        meta.path().segments.iter().any(|s| {
            matches!(
                s.ident.to_string().as_str(),
                "coverage" | "coverage_nightly" | "tarpaulin" | "tarpaulin_include"
            )
        }) || meta_children(meta).iter().any(contains)
    }
    contains(meta)
}
/// Recognizes actual test attributes independently of test-only compilation.
///
/// # Parameters
///
/// * `attrs` - Function attributes that may apply a test or rstest entry point.
fn is_test_function(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path()
            .segments
            .last()
            .is_some_and(|s| matches!(s.ident.to_string().as_str(), "test" | "rstest"))
            || conditional_meta(&a.meta, "test")
    })
}
/// Determines whether an item is known to exist only in a test context.
///
/// # Parameters
///
/// * `attrs` - Attributes containing a test marker or a predicate requiring
///   test.
fn is_test_attrs(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path()
            .segments
            .last()
            .is_some_and(|s| matches!(s.ident.to_string().as_str(), "test" | "rstest"))
            || (a.path().is_ident("cfg") && meta_children(&a.meta).iter().any(cfg_requires_test))
    })
}
/// Checks whether a cfg predicate logically requires the test flag.
///
/// # Parameters
///
/// * `meta` - Predicate composed of test, all, or any metadata.
fn cfg_requires_test(meta: &Meta) -> bool {
    meta.path().is_ident("test")
        || (meta.path().is_ident("all") && meta_children(meta).iter().any(cfg_requires_test))
        || (meta.path().is_ident("any") && {
            let children = meta_children(meta);
            !children.is_empty() && children.iter().all(cfg_requires_test)
        })
}
/// Reports whether item attributes introduce conditional compilation.
///
/// # Parameters
///
/// * `attrs` - Attributes belonging to the current declaration.
fn has_cfg(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
}
/// Collects literal Rustdoc text without pretending macros are evaluated.
///
/// # Parameters
///
/// * `attrs` - Item attributes whose literal doc values are joined in source
///   order.
fn doc_text(attrs: &[Attribute]) -> String {
    attrs
        .iter()
        .filter_map(|a| {
            if a.path().is_ident("doc") {
                if let Meta::NameValue(value) = &a.meta
                    && let syn::Expr::Lit(lit) = &value.value
                    && let syn::Lit::Str(value) = &lit.lit
                {
                    return Some(value.value());
                }
                None
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
/// Checks for an exact first-level Rustdoc contract heading.
///
/// # Parameters
///
/// * `text` - Literal Rustdoc text with its original line boundaries.
/// * `heading` - Heading title without the Markdown marker.
fn has_heading(text: &str, heading: &str) -> bool {
    text.lines().any(|line| line.trim() == format!("# {heading}"))
}
/// Extracts the source text covered by an original token span.
///
/// # Parameters
///
/// * `source` - Complete original UTF-8 source text.
/// * `span` - Token coordinates used to select a multiline source excerpt.
fn source_span(source: &str, span: Span) -> String {
    let start = span.start();
    let end = span.end();
    source
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let line_no = i + 1;
            if line_no < start.line || line_no > end.line {
                return None;
            }
            let a = if line_no == start.line { start.column } else { 0 };
            let b = if line_no == end.line { end.column } else { line.len() };
            line.get(a..b).map(str::to_owned)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
/// Maps Rust type identifiers to filename spelling, preserving acronym
/// boundaries.
///
/// # Parameters
///
/// * `value` - Declared type identifier, optionally carrying a raw-identifier
///   prefix.
fn snake_case(value: &str) -> String {
    let value = value.strip_prefix("r#").unwrap_or(value);
    let chars: Vec<_> = value.chars().collect();
    let mut result = String::new();
    for (i, ch) in chars.iter().enumerate() {
        if ch.is_uppercase()
            && i > 0
            && (chars[i - 1].is_lowercase()
                || chars[i - 1].is_numeric()
                || chars.get(i + 1).is_some_and(|c| c.is_lowercase()) && chars[i - 1].is_uppercase())
        {
            result.push('_');
        }
        result.extend(ch.to_lowercase());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::inspect;
    use super::inspect_with_role;
    use crate::analysis::Finding;
    use crate::style_exceptions::ExceptionConfig;
    fn findings(path: &str, source: &str) -> Vec<Finding> {
        inspect(
            path,
            source,
            &syn::parse_file(source).unwrap(),
            &ExceptionConfig::default(),
        )
    }
    fn has(findings: &[Finding], check: &str) -> bool {
        findings.iter().any(|f| f.check_id == check && f.severity == "error")
    }

    #[test]
    fn test_ignores_strings_and_comments_but_checks_nested_ast_imports() {
        let found = findings(
            "src/lib.rs",
            "// use foo::*;\nconst S: &str = \"include!(x); use x::*;\";\nmod nested { use foo::{Bar, Baz}; }\n",
        );
        assert_eq!(found.iter().filter(|f| f.check_id == "single-object-import").count(), 1);
        assert!(!has(&found, "test-source-redirection"));
    }
    #[test]
    fn test_conditional_attributes_are_checked() {
        let found = findings(
            "src/lib.rs",
            "#[cfg_attr(feature = \"x\", allow(unused_imports))]\nuse foo::Bar;\n#[cfg_attr(coverage, inline)] fn f() {}\n",
        );
        assert!(has(&found, "unused-imports-allow"));
        assert!(has(&found, "coverage-cfg"));
    }
    #[test]
    fn test_checks_private_types_and_conditional_variants_conservatively() {
        assert!(has(&findings("src/a.rs", "struct A; struct B;"), "one-type-per-file"));
        let found = findings(
            "src/a.rs",
            "#[cfg(feature=\"x\")] struct A; #[cfg(not(feature=\"x\"))] struct A;",
        );
        assert!(!has(&found, "one-type-per-file"));
        assert!(
            found
                .iter()
                .any(|f| f.check_id == "one-type-per-file" && f.severity == "review")
        );
    }
    #[test]
    fn test_inline_test_context_exempts_production_docs() {
        let found = findings(
            "src/lib.rs",
            "#[cfg(test)] mod tests { struct Helper; #[test] fn parses() {} }\n",
        );
        assert!(!has(&found, "rustdoc-presence"));
        assert!(has(&found, "test-function-name"));
    }
    #[test]
    fn test_checks_trait_attributes_and_test_redirection() {
        assert!(has(
            &findings("src/a.rs", "trait A { #[inline(always)] fn f(); }"),
            "trait-inline-without-body"
        ));
        assert!(has(
            &findings(
                "tests/a_tests.rs",
                "#[path=\"../x.rs\"] mod x; fn f() { include!(\"x.rs\"); }"
            ),
            "test-source-redirection"
        ));
        assert!(!has(
            &findings("tests/common/mod.rs", "include!(\"x.rs\");"),
            "test-source-redirection"
        ));
    }
    #[test]
    fn test_support_directories_are_reviewed_until_configuration_proves_exclusion() {
        let found = findings("tests/common/shared.rs", r#"include!("data.rs");"#);
        for check in ["test-file-name", "test-source-redirection"] {
            assert!(found.iter().any(|f| f.check_id == check && f.severity == "review"));
            assert!(!has(&found, check));
        }
    }

    #[test]
    fn test_documents_tuple_fields_variants_private_functions_and_type_aliases() {
        let found = findings(
            "src/a.rs",
            "enum A { B(u8), C { value: u8 } } type Alias = A; fn helper() {} impl A { fn new() -> Self { Self::B(0) } }",
        );
        assert_eq!(found.iter().filter(|f| f.check_id == "rustdoc-presence").count(), 8);
    }
    #[test]
    fn test_counts_types_across_inline_modules_and_function_bodies() {
        assert!(has(
            &findings("src/lib.rs", "mod one { struct A; } mod two { struct B; }"),
            "one-type-per-file"
        ));
        assert!(has(
            &findings("src/lib.rs", "struct A; fn f() { struct B; }"),
            "one-type-per-file"
        ));
        let found = findings(
            "src/lib.rs",
            "#[cfg(feature=\"a\")] mod one { struct A; } #[cfg(not(feature=\"a\"))] mod two { struct B; }",
        );
        assert!(!has(&found, "one-type-per-file"));
    }
    #[test]
    fn test_cargo_role_override_applies_production_documentation_to_custom_roots() {
        let source = "fn helper() {}";
        let ast = syn::parse_file(source).unwrap();
        assert!(has(
            &inspect_with_role("custom/api.rs", source, &ast, &ExceptionConfig::default(), Some(true)),
            "rustdoc-presence"
        ));
        assert!(!has(
            &inspect_with_role("src/bench.rs", source, &ast, &ExceptionConfig::default(), Some(false)),
            "rustdoc-presence"
        ));
    }
    #[test]
    fn test_macros_and_configuration_are_explicit_review_signals() {
        let found = findings("src/lib.rs", "make_type!(A); #[cfg(feature=\"a\")] fn f() {} ");
        assert!(
            found
                .iter()
                .any(|f| f.check_id == "unexpanded-macro" && f.severity == "review")
        );
        assert!(
            found
                .iter()
                .any(|f| f.check_id == "conditional-configuration" && f.severity == "review")
        );
    }

    #[test]
    fn test_only_helpers_are_not_test_functions() {
        let found = findings(
            "src/a.rs",
            "struct A; #[cfg(test)] struct Helper; #[cfg(test)] fn helper() {} #[test] fn actual() {}",
        );
        assert!(!has(&found, "one-type-per-file"));
        assert_eq!(found.iter().filter(|f| f.check_id == "test-function-name").count(), 1);
    }
    #[test]
    fn test_builder_receiver_checks_respect_explicit_ownership() {
        assert!(has(
            &findings(
                "src/a_builder.rs",
                "struct ABuilder; impl ABuilder { fn build(&self) {} }"
            ),
            "builder-build-receiver"
        ));
        for receiver in ["self", "mut self", "self: Self"] {
            assert!(!has(
                &findings(
                    "src/a_builder.rs",
                    &format!("struct ABuilder; impl ABuilder {{ fn build({receiver}) {{}} }}")
                ),
                "builder-build-receiver"
            ));
        }
        let found = findings(
            "src/a_builder.rs",
            "struct ABuilder; impl ABuilder { fn build(self: Box<Self>) {} }",
        );
        assert!(!has(&found, "builder-build-receiver"));
        assert!(
            found
                .iter()
                .any(|f| f.check_id == "builder-build-receiver" && f.severity == "review")
        );
    }
    #[test]
    fn test_ignores_test_only_impl_docs_and_hidden_is_not_documentation() {
        assert!(!has(
            &findings("src/lib.rs", "#[cfg(test)] impl Example { fn helper() {} }"),
            "rustdoc-presence"
        ));
        assert!(has(
            &findings("src/a.rs", "#[doc(hidden)] struct A;"),
            "rustdoc-presence"
        ));
    }

    #[test]
    fn test_validates_empty_bodies_and_import_groups() {
        assert!(!has(
            &findings("src/a.rs", "struct A {\n    // empty\n}"),
            "empty-type-body"
        ));
        assert!(has(&findings("src/a.rs", "struct A {}"), "empty-type-body"));
        assert!(has(
            &findings("src/lib.rs", "use std::fmt;\nuse foo::Bar;"),
            "import-groups"
        ));
        assert!(!has(
            &findings("src/lib.rs", "use std::fmt;\n\nuse foo::Bar;\n\nuse crate::Baz;"),
            "import-groups"
        ));
    }
}
