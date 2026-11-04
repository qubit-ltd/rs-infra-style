// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Traversal state and local syntax checks for one immutable Rust file.

use std::path::Path;

use proc_macro2::Span;
use quote::ToTokens;
use syn::Attribute;
use syn::Item;
use syn::Meta;
use syn::spanned::Spanned;
use syn::visit::Visit;

use super::super::conditional_meta;
use super::super::contains_group_or_glob;
use super::super::doc_text;
use super::super::has_cfg;
use super::super::has_heading;
use super::super::import_group;
use super::super::is_coverage_cfg;
use super::super::is_test_attrs;
use super::super::is_test_function;
use super::super::meta_has_attr;
use super::super::prohibited_unused_imports;
use super::super::snake_case;
use super::super::source_span;
use crate::analysis::Finding;
use crate::style_exceptions::ExceptionConfig;

/// Carries source context while syntax checks traverse nested Rust items.
///
/// The borrowed inputs remain immutable; only diagnostics and scope state
/// change.
pub(in crate::analysis_rules) struct Inspector<'a> {
    /// Project-relative path used to bind findings to their source file.
    pub(in crate::analysis_rules) path: &'a str,
    /// Original source text used for exact whitespace and comment evidence.
    pub(in crate::analysis_rules) source: &'a str,
    /// Validated project exceptions that apply to exact paths and rules.
    pub(in crate::analysis_rules) exceptions: &'a ExceptionConfig,
    /// Accumulated proved violations and explicitly uncertain review signals.
    pub(in crate::analysis_rules) findings: Vec<Finding>,
    /// Whether Cargo context classifies this file as production code.
    pub(in crate::analysis_rules) production: bool,
    /// Whether the current traversal is inside a test-only item or module.
    pub(in crate::analysis_rules) test_context: bool,
    /// Whether a conventional support directory needs exclusion review.
    pub(in crate::analysis_rules) support: bool,
    /// Number of inline module scopes currently being traversed.
    pub(in crate::analysis_rules) module_depth: usize,
    /// Whether an actual source comment records a coverage exception.
    pub(in crate::analysis_rules) allow_coverage_comment: bool,
    /// Production type names, source spans, and inherited conditional flags.
    pub(in crate::analysis_rules) types: Vec<(String, Span, bool)>,
    /// Whether an ancestor or current item is conditionally compiled.
    pub(in crate::analysis_rules) conditional: bool,
}

impl Inspector<'_> {
    /// Records one location-bound diagnostic without changing source.
    ///
    /// # Parameters
    ///
    /// * `rule` - Normative rule identifier owning the diagnostic.
    /// * `check` - Stable deterministic subcheck identifier.
    /// * `severity` - Whether the finding is proved or needs semantic review.
    /// * `span` - Original token span used for one-based diagnostic
    ///   coordinates.
    /// * `evidence` - Observed syntax or source excerpt supporting this
    ///   finding.
    /// * `guidance` - Diagnostic message and concrete repair or review
    ///   direction.
    pub(in crate::analysis_rules) fn issue(
        &mut self,
        rule: &str,
        check: &str,
        severity: &str,
        span: Span,
        evidence: &str,
        guidance: (&str, &str),
    ) {
        let (message, help) = guidance;
        let start = span.start();
        self.findings.push(Finding {
            rule_id: rule.into(),
            check_id: check.into(),
            severity: severity.into(),
            path: self.path.into(),
            line: start.line.max(1),
            column: start.column + 1,
            message: message.into(),
            evidence: evidence.into(),
            help: help.into(),
            related_locations: Vec::new(),
        });
    }

    /// Checks production documentation presence and attribute ordering.
    ///
    /// # Parameters
    ///
    /// * `attrs` - Item attributes, including literal or conditional
    ///   documentation.
    /// * `span` - Original location used when documentation is missing.
    /// * `label` - Item identity included in the diagnostic evidence.
    fn docs(&mut self, attrs: &[Attribute], span: Span, label: &str) {
        if !self.production || self.test_context || is_test_attrs(attrs) {
            return;
        }
        if doc_text(attrs).trim().is_empty() {
            // Conditional documentation cannot be evaluated without selecting a cfg.
            let conditional = attrs.iter().any(|a| conditional_meta(&a.meta, "doc") || matches!(&a.meta, Meta::NameValue(v) if a.path().is_ident("doc") && !matches!(&v.value, syn::Expr::Lit(_))));
            self.issue(
                "DOC-002",
                "rustdoc-presence",
                if conditional { "review" } else { "error" },
                span,
                label,
                (
                    if conditional {
                        "Documentation is conditional; review the supported configurations."
                    } else {
                        "Production item has no Rustdoc."
                    },
                    "Explain this item's purpose, semantics and applicable constraints in English Rustdoc.",
                ),
            );
        }
        let mut seen_non_doc = false;
        for attr in attrs {
            if attr.path().is_ident("doc") {
                if seen_non_doc {
                    self.issue(
                        "DOC-003",
                        "documentation-attribute-order",
                        "error",
                        attr.span(),
                        label,
                        (
                            "Rustdoc must precede non-documentation attributes.",
                            "Move the Rustdoc before other attributes, preserving its text.",
                        ),
                    );
                    break;
                }
            } else {
                seen_non_doc = true;
            }
        }
    }

    /// Checks sections whose applicability follows directly from a function
    /// signature.
    ///
    /// # Parameters
    ///
    /// * `attrs` - Documentation attributes belonging to this function.
    /// * `sig` - Signature declaring parameters, generics, and safety
    ///   requirements.
    fn signature_docs(&mut self, attrs: &[Attribute], sig: &syn::Signature) {
        if !self.production || self.test_context {
            return;
        }
        let docs = doc_text(attrs);
        if docs.is_empty() {
            return;
        }
        let has_type_params = sig
            .generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Type(_)));
        let has_parameters = sig.inputs.iter().any(|p| matches!(p, syn::FnArg::Typed(_)));
        for (required, heading) in [
            (has_type_params, "Type Parameters"),
            (has_parameters, "Parameters"),
            (matches!(sig.safety, syn::Safety::Unsafe(_)), "Safety"),
        ] {
            if required && !has_heading(&docs, heading) {
                self.issue(
                    "DOC-003",
                    "required-documentation-sections",
                    "error",
                    sig.ident.span(),
                    &sig.ident.to_string(),
                    (
                        &format!("Rustdoc is missing # {heading}."),
                        "Document the applicable signature contract in the required section.",
                    ),
                );
            }
        }
    }

    /// Checks type documentation and records uncertainty about public core
    /// examples.
    ///
    /// # Parameters
    ///
    /// * `attrs` - Attributes carrying the type documentation.
    /// * `ident` - Declared type identifier and its source location.
    /// * `vis` - Declared visibility used to identify possible public core
    ///   types.
    /// * `generics` - Generic parameters that require documented type
    ///   constraints.
    fn type_docs(&mut self, attrs: &[Attribute], ident: &syn::Ident, vis: &syn::Visibility, generics: &syn::Generics) {
        self.docs(attrs, ident.span(), &ident.to_string());
        if !self.production || self.test_context {
            return;
        }
        let docs = doc_text(attrs);
        if !docs.is_empty()
            && generics.params.iter().any(|p| matches!(p, syn::GenericParam::Type(_)))
            && !has_heading(&docs, "Type Parameters")
        {
            self.issue(
                "DOC-003",
                "required-documentation-sections",
                "error",
                ident.span(),
                &ident.to_string(),
                (
                    "Generic type Rustdoc is missing # Type Parameters.",
                    "Describe the type parameters and applicable constraints.",
                ),
            );
        }
        if matches!(vis, syn::Visibility::Public(_)) && !has_heading(&docs, "Examples") {
            self.issue(
                "DOC-003",
                "public-type-examples",
                "review",
                ident.span(),
                &ident.to_string(),
                (
                    "Public type has no type-level # Examples; confirm whether it is a core public type.",
                    "For a public core type, provide a complete example through the public API.",
                ),
            );
        }
    }

    /// Checks a directly declared type against its source filename.
    ///
    /// # Parameters
    ///
    /// * `items` - Items declared directly in the current module scope.
    /// * `file_level` - Whether this scope is the source file rather than an
    ///   inline module.
    pub(in crate::analysis_rules) fn check_layout(&mut self, items: &[Item], file_level: bool) {
        if !self.production || self.test_context {
            return;
        }
        let types: Vec<_> = items
            .iter()
            .filter_map(|item| match item {
                Item::Struct(x) if !is_test_attrs(&x.attrs) => Some((&x.ident, &x.attrs)),
                Item::Enum(x) if !is_test_attrs(&x.attrs) => Some((&x.ident, &x.attrs)),
                Item::Trait(x) if !is_test_attrs(&x.attrs) => Some((&x.ident, &x.attrs)),
                _ => None,
            })
            .collect();
        if file_level && types.len() == 1 && !self.exceptions.allows("type-file-name", self.path) {
            let stem = Path::new(self.path).file_stem().and_then(|x| x.to_str()).unwrap_or("");
            if !matches!(stem, "lib" | "mod" | "main") {
                let expected = snake_case(&types[0].0.to_string());
                if stem != expected {
                    self.issue(
                        "ORG-001",
                        "type-file-name",
                        "error",
                        types[0].0.span(),
                        &format!("{} belongs in {expected}.rs", types[0].0),
                        (
                            "Production type does not match its snake_case filename.",
                            "Rename or split the source and update its module declarations.",
                        ),
                    );
                }
            }
        }
    }

    /// Checks the explicit comment contract for an empty named type body.
    ///
    /// # Parameters
    ///
    /// * `span` - Brace span including the original empty-body comments.
    /// * `label` - Type identifier used as diagnostic evidence.
    fn empty_body(&mut self, span: Span, label: &str) {
        if !self.production || self.test_context {
            return;
        }
        let text = source_span(self.source, span);
        let inner = text
            .trim()
            .strip_prefix('{')
            .and_then(|x| x.strip_suffix('}'))
            .unwrap_or("")
            .trim();
        if inner != "// empty" {
            self.issue(
                "ORG-001",
                "empty-type-body",
                "error",
                span,
                label,
                (
                    "Empty struct and trait bodies require an indented // empty line.",
                    "Write the braces on separate lines with an indented // empty comment.",
                ),
            );
        } else if !text.contains('\n')
            || !text
                .lines()
                .any(|line| line.starts_with(' ') && line.trim() == "// empty")
        {
            self.issue(
                "ORG-001",
                "empty-type-body",
                "error",
                span,
                label,
                (
                    "Empty body comment must occupy its own indented line.",
                    "Write the braces on separate lines with an indented // empty comment.",
                ),
            );
        }
    }

    /// Checks adjacent import groups using original source whitespace.
    ///
    /// # Parameters
    ///
    /// * `items` - Items whose consecutive imports share one module scope.
    fn imports(&mut self, items: &[Item]) {
        if self.exceptions.allows("explicit-imports", self.path) {
            return;
        }
        let uses: Vec<_> = items
            .iter()
            .filter_map(|item| if let Item::Use(x) = item { Some(x) } else { None })
            .collect();
        for pair in uses.windows(2) {
            let left = pair[0];
            let right = pair[1];
            // Attributes and intervening declarations change the layout context.
            if !left.attrs.is_empty() || !right.attrs.is_empty() {
                continue;
            }
            let end = left.span().end().line;
            let start = right.span().start().line;
            let middle: Vec<_> = self
                .source
                .lines()
                .skip(end)
                .take(start.saturating_sub(end + 1))
                .collect();
            if middle.iter().any(|line| !line.trim().is_empty()) {
                continue;
            }
            let a = import_group(&left.tree);
            let b = import_group(&right.tree);
            let expected = if a == b { 0 } else { 1 };
            if a > b || middle.len() != expected {
                self.issue("IMPORT-001", "import-groups", "error", right.span(), &format!("groups {a} -> {b}, {} blank lines", middle.len()), ("Imports must be grouped std/core/alloc, external, then crate/self/super; adjacent groups have exactly one blank line.", "Arrange the groups and let project rustfmt order imports within each group."));
            }
        }
    }
}

impl<'ast> Visit<'ast> for Inspector<'_> {
    /// Tracks inherited test and conditional scopes while inventorying
    /// production types.
    ///
    /// # Parameters
    ///
    /// * `node` - Item whose scope and nested declarations are traversed.
    fn visit_item(&mut self, node: &'ast syn::Item) {
        let attrs: &[Attribute] = match node {
            Item::Struct(x) => &x.attrs,
            Item::Enum(x) => &x.attrs,
            Item::Trait(x) => &x.attrs,
            Item::Impl(x) => &x.attrs,
            Item::Fn(x) => &x.attrs,
            Item::Mod(x) => &x.attrs,
            Item::Type(x) => &x.attrs,
            Item::Use(x) => &x.attrs,
            Item::Const(x) => &x.attrs,
            Item::Static(x) => &x.attrs,
            Item::Macro(x) => &x.attrs,
            _ => &[],
        };
        let old = self.test_context;
        let old_conditional = self.conditional;
        self.test_context |= is_test_attrs(attrs);
        self.conditional |= has_cfg(attrs);
        if has_cfg(attrs) {
            self.issue("ORG-001", "conditional-configuration", "review", node.span(), &attrs
                    .iter()
                    .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
                    .map(|a| a.meta.to_token_stream().to_string())
                    .collect::<Vec<_>>()
                    .join("; "), ("Conditional syntax is inspected in source form; active configuration and expansion require review.", "Review applicable configurations without treating inactive declarations as simultaneously active."));
        }
        if self.production && !self.test_context {
            let ident = match node {
                Item::Struct(x) => Some(&x.ident),
                Item::Enum(x) => Some(&x.ident),
                Item::Trait(x) => Some(&x.ident),
                _ => None,
            };
            if let Some(ident) = ident {
                self.types.push((ident.to_string(), ident.span(), self.conditional));
            }
        }
        syn::visit::visit_item(self, node);
        self.test_context = old;
        self.conditional = old_conditional;
    }

    /// Checks file-level imports before visiting nested syntax.
    ///
    /// # Parameters
    ///
    /// * `node` - Parsed file retaining original source spans.
    fn visit_file(&mut self, node: &'ast syn::File) {
        self.imports(&node.items);
        syn::visit::visit_file(self, node);
    }

    /// Checks suppression, test redirection, and coverage conditions in real
    /// attributes.
    ///
    /// # Parameters
    ///
    /// * `node` - Parsed attribute, including nested conditional attribute
    ///   metadata.
    fn visit_attribute(&mut self, node: &'ast Attribute) {
        if prohibited_unused_imports(&node.meta) && !self.exceptions.allows("explicit-imports", self.path) {
            self.issue(
                "IMPORT-001",
                "unused-imports-allow",
                "error",
                node.span(),
                &node.meta.to_token_stream().to_string(),
                (
                    "Do not suppress unused_imports.",
                    "Remove the suppression and remove or narrow the unused imports.",
                ),
            );
        }
        if self.test_context && meta_has_attr(&node.meta, "path") && !self.exceptions.allows("test-redirect", self.path)
        {
            self.issue(
                "TEST-004",
                "test-source-redirection",
                if self.support { "review" } else { "error" },
                node.span(),
                &node.meta.to_token_stream().to_string(),
                (
                    "Test sources must not redirect code with #[path].",
                    "Keep the test at its real module location and use normal module declarations.",
                ),
            );
        }
        if self.production
            && !self.test_context
            && is_coverage_cfg(&node.meta)
            && (!self.exceptions.allows("coverage-cfg", self.path) || !self.allow_coverage_comment)
        {
            self.issue(
                "ATTR-003",
                "coverage-cfg",
                "error",
                node.span(),
                &node.meta.to_token_stream().to_string(),
                (
                    "Coverage-specific cfg requires a source allow comment and a reasoned project allowlist entry.",
                    "Remove the coverage branch or supply both reviewed exception records.",
                ),
            );
        }
        syn::visit::visit_attribute(self, node);
    }

    /// Checks that an import selects exactly one object without a wildcard.
    ///
    /// # Parameters
    ///
    /// * `node` - Import declaration containing the parsed use tree.
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        if contains_group_or_glob(&node.tree) && !self.exceptions.allows("explicit-imports", self.path) {
            self.issue(
                "IMPORT-001",
                "single-object-import",
                "error",
                node.span(),
                &node.tree.to_token_stream().to_string(),
                (
                    "Each use must import exactly one object, without braces or wildcard.",
                    "Split brace imports into individual uses and replace wildcards with explicit objects.",
                ),
            );
        }
        syn::visit::visit_item_use(self, node);
    }

    /// Preserves test scope and checks imports within an inline module.
    ///
    /// # Parameters
    ///
    /// * `node` - Module declaration with optional inline items.
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let old = self.test_context;
        self.test_context |= is_test_attrs(&node.attrs);
        if let Some((_, items)) = &node.content {
            self.module_depth += 1;
            self.check_layout(items, false);
            self.imports(items);
        }
        syn::visit::visit_item_mod(self, node);
        if node.content.is_some() {
            self.module_depth -= 1;
        }
        self.test_context = old;
    }

    /// Checks struct documentation and the comment inside an empty named body.
    ///
    /// # Parameters
    ///
    /// * `node` - Struct declaration and its field syntax.
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        self.type_docs(&node.attrs, &node.ident, &node.vis, &node.generics);
        if let syn::Fields::Named(fields) = &node.fields
            && fields.named.is_empty()
        {
            self.empty_body(fields.brace_token.span.join(), &node.ident.to_string());
        }
        syn::visit::visit_item_struct(self, node);
    }
    /// Checks enum documentation before visiting variants and their fields.
    ///
    /// # Parameters
    ///
    /// * `node` - Enum declaration whose fields remain individually documented.
    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        self.type_docs(&node.attrs, &node.ident, &node.vis, &node.generics);
        syn::visit::visit_item_enum(self, node);
    }
    /// Checks trait documentation and the required empty-body comment.
    ///
    /// # Parameters
    ///
    /// * `node` - Trait declaration with optional associated items.
    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        self.type_docs(&node.attrs, &node.ident, &node.vis, &node.generics);
        if node.items.is_empty() {
            self.empty_body(node.brace_token.span.join(), &node.ident.to_string());
        }
        syn::visit::visit_item_trait(self, node);
    }
    /// Checks documentation for a standalone type alias.
    ///
    /// # Parameters
    ///
    /// * `node` - Alias declaration whose semantics still require review.
    fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
        self.type_docs(&node.attrs, &node.ident, &node.vis, &node.generics);
        syn::visit::visit_item_type(self, node);
    }
    /// Checks documentation for both named fields and tuple positions.
    ///
    /// # Parameters
    ///
    /// * `node` - Field declaration with its own attributes and type.
    fn visit_field(&mut self, node: &'ast syn::Field) {
        self.docs(
            &node.attrs,
            node.span(),
            &node
                .ident
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "tuple field".into()),
        );
        syn::visit::visit_field(self, node);
    }
    /// Checks variant documentation before inspecting variant fields.
    ///
    /// # Parameters
    ///
    /// * `node` - Enum variant that introduces one independently documented
    ///   case.
    fn visit_variant(&mut self, node: &'ast syn::Variant) {
        self.docs(&node.attrs, node.ident.span(), &node.ident.to_string());
        syn::visit::visit_variant(self, node);
    }
    /// Checks test naming and production documentation in the correct test
    /// scope.
    ///
    /// # Parameters
    ///
    /// * `node` - Free function, including helpers compiled only during tests.
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let old = self.test_context;
        let test = is_test_function(&node.attrs);
        self.test_context |= is_test_attrs(&node.attrs);
        if test && !node.sig.ident.to_string().starts_with("test_") {
            self.issue(
                "TEST-002",
                "test-function-name",
                "error",
                node.sig.ident.span(),
                &node.sig.ident.to_string(),
                (
                    "Test functions must use a behavior-oriented test_* name.",
                    "Rename the test to describe its behavior with the test_ prefix.",
                ),
            );
        }
        self.docs(&node.attrs, node.sig.ident.span(), &node.sig.ident.to_string());
        self.signature_docs(&node.attrs, &node.sig);
        syn::visit::visit_item_fn(self, node);
        self.test_context = old;
    }
    /// Checks explicit builder completion receivers without guessing custom
    /// ownership.
    ///
    /// # Parameters
    ///
    /// * `node` - Implementation whose inherent builder methods may consume
    ///   self.
    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if self.production
            && !self.test_context
            && node.trait_.is_none()
            && let syn::Type::Path(ty) = &*node.self_ty
            && ty
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident.to_string().ends_with("Builder"))
        {
            for item in &node.items {
                if let syn::ImplItem::Fn(method) = item
                    && method.sig.ident == "build"
                {
                    let severity = match method.sig.receiver().map(|r| &r.kind) {
                        Some(syn::ReceiverKind::Value) => None,
                        Some(syn::ReceiverKind::Typed(_, ty)) if matches!(&**ty, syn::Type::Path(p) if p.qself.is_none() && p.path.is_ident("Self")) => {
                            None
                        }
                        Some(syn::ReceiverKind::Typed(_, _)) => Some("review"),
                        _ => Some("error"),
                    };
                    if let Some(severity) = severity {
                        self.issue(
                            "ORG-001",
                            "builder-build-receiver",
                            severity,
                            method.sig.ident.span(),
                            &method.sig.to_token_stream().to_string(),
                            (
                                "Builder build must consume self; explicit receiver ownership requires review.",
                                "Use build(self) and preserve the builder's construction contract.",
                            ),
                        );
                    }
                }
            }
        }
        syn::visit::visit_item_impl(self, node);
    }

    /// Checks implementation method documentation and signature sections.
    ///
    /// # Parameters
    ///
    /// * `node` - Method with an implementation body and its own attributes.
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.docs(&node.attrs, node.sig.ident.span(), &node.sig.ident.to_string());
        self.signature_docs(&node.attrs, &node.sig);
        syn::visit::visit_impl_item_fn(self, node);
    }
    /// Checks trait documentation and rejects inline attributes on bodyless
    /// methods.
    ///
    /// # Parameters
    ///
    /// * `node` - Trait method with an optional default implementation.
    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        self.docs(&node.attrs, node.sig.ident.span(), &node.sig.ident.to_string());
        self.signature_docs(&node.attrs, &node.sig);
        if node.default.is_none() {
            for attr in &node.attrs {
                if meta_has_attr(&attr.meta, "inline") {
                    self.issue(
                        "ATTR-001",
                        "trait-inline-without-body",
                        "error",
                        attr.span(),
                        &node.sig.ident.to_string(),
                        (
                            "A trait method without a default body cannot carry inline or inline(always).",
                            "Remove the attribute here; judge the actual implementation body independently.",
                        ),
                    );
                }
            }
        }
        syn::visit::visit_trait_item_fn(self, node);
    }
    /// Records the boundary of syntax checks at an unexpanded item macro.
    ///
    /// # Parameters
    ///
    /// * `node` - Macro definition or invocation whose generated items are
    ///   unknown.
    fn visit_item_macro(&mut self, node: &'ast syn::ItemMacro) {
        self.issue(
            "ORG-001",
            "unexpanded-macro",
            "review",
            node.mac.span(),
            &node.mac.path.to_token_stream().to_string(),
            (
                "Item macro tokens are retained but not expanded; generated items require semantic review.",
                "Inspect the macro definition and actual call sites, including generated type attributes.",
            ),
        );
        syn::visit::visit_item_macro(self, node);
    }
    /// Checks documentation for an associated type declaration.
    ///
    /// # Parameters
    ///
    /// * `node` - Trait associated type with its documentation attributes.
    fn visit_trait_item_type(&mut self, node: &'ast syn::TraitItemType) {
        self.docs(&node.attrs, node.ident.span(), &node.ident.to_string());
        syn::visit::visit_trait_item_type(self, node);
    }
    /// Checks documentation for an associated type definition.
    ///
    /// # Parameters
    ///
    /// * `node` - Implementation alias whose meaning needs explicit
    ///   documentation.
    fn visit_impl_item_type(&mut self, node: &'ast syn::ImplItemType) {
        self.docs(&node.attrs, node.ident.span(), &node.ident.to_string());
        syn::visit::visit_impl_item_type(self, node);
    }
    /// Checks documentation and applicable sections for a foreign function.
    ///
    /// # Parameters
    ///
    /// * `node` - Foreign declaration whose contract is visible in its
    ///   signature.
    fn visit_foreign_item_fn(&mut self, node: &'ast syn::ForeignItemFn) {
        self.docs(&node.attrs, node.sig.ident.span(), &node.sig.ident.to_string());
        self.signature_docs(&node.attrs, &node.sig);
        syn::visit::visit_foreign_item_fn(self, node);
    }

    /// Checks real test include invocations while leaving other macro tokens
    /// unexpanded.
    ///
    /// # Parameters
    ///
    /// * `node` - Macro invocation retaining its actual parsed path and tokens.
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if self.test_context && node.path.is_ident("include") && !self.exceptions.allows("test-redirect", self.path) {
            self.issue(
                "TEST-004",
                "test-source-redirection",
                if self.support { "review" } else { "error" },
                node.span(),
                &node.to_token_stream().to_string(),
                (
                    "Concrete tests must not redirect source using include!.",
                    "Place tests in the real test module rather than including a separate source file.",
                ),
            );
        }
        syn::visit::visit_macro(self, node);
    }
}
