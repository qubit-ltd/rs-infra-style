// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Per-file declaration and inherent-implementation collector.

use syn::visit::Visit;

use super::declaration::Declaration;

/// Per-file declaration and inherent-implementation collector.
pub(crate) struct Index<'a> {
    /// Project-relative source containing this item.
    pub(crate) path: &'a str,
    /// Declarations collected from this file.
    pub(crate) declarations: Vec<Declaration>,
    /// Inherent implementations and their conditional nesting.
    pub(crate) impls: Vec<(&'a syn::ItemImpl, usize)>,
    /// Current nesting depth of declarations affected by conditional syntax.
    pub(crate) conditional: usize,
}

impl<'ast> Visit<'ast> for Index<'ast> {
    /// Collects declaration and inherent-implementation ownership evidence from
    /// parsed items.
    ///
    /// # Parameters
    ///
    /// Arguments provide the source, project context or analysis state
    /// described by this operation.
    fn visit_item(&mut self, item: &'ast syn::Item) {
        let (name, attrs) = match item {
            syn::Item::Struct(item) => (Some(&item.ident), &item.attrs),
            syn::Item::Enum(item) => (Some(&item.ident), &item.attrs),
            syn::Item::Trait(item) => (Some(&item.ident), &item.attrs),
            syn::Item::Type(item) => (Some(&item.ident), &item.attrs),
            syn::Item::Mod(item) => (None, &item.attrs),
            syn::Item::Impl(item) => (None, &item.attrs),
            _ => {
                syn::visit::visit_item(self, item);
                return;
            }
        };
        let gated = attrs.iter().any(|attr| {
            matches!(
                attr.path().get_ident().map(ToString::to_string).as_deref(),
                Some("cfg" | "cfg_attr")
            )
        });
        self.conditional += usize::from(gated);
        if let Some(name) = name {
            self.declarations.push(Declaration {
                path: self.path.into(),
                name: name.to_string(),
                line: name.span().start().line,
                conditional: self.conditional > 0,
                must_use: attrs.iter().any(|attr| attr.path().is_ident("must_use")),
            });
        }
        if let syn::Item::Impl(item) = item
            && item.trait_.is_none()
        {
            self.impls.push((item, self.conditional));
        }
        syn::visit::visit_item(self, item);
        self.conditional -= usize::from(gated);
    }
}
