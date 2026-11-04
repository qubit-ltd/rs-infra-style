// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Source-position inventory of inherent method signatures for semantic
//! ordering review.

use quote::ToTokens;
use syn::spanned::Spanned;
use syn::visit::Visit;

/// Source-position inventory of inherent method signatures for semantic
/// ordering review.
pub(crate) struct MethodInventory {
    /// Located method signatures for the semantic reviewer.
    pub(crate) entries: Vec<String>,
}

impl<'ast> Visit<'ast> for MethodInventory {
    /// Records inherent method signatures and source positions for semantic
    /// ordering review.
    ///
    /// # Parameters
    ///
    /// Arguments provide the source, project context or analysis state
    /// described by this operation.
    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if item.trait_.is_none() {
            for method in &item.items {
                if let syn::ImplItem::Fn(method) = method {
                    let signature = format!(
                        "impl {}: {}",
                        item.self_ty.to_token_stream(),
                        method.sig.to_token_stream()
                    );
                    let summary: String = signature.chars().take(768).collect();
                    let suffix = if signature.chars().count() > 768 {
                        " [signature abbreviated; inspect source at this line]"
                    } else {
                        ""
                    };
                    self.entries
                        .push(format!("line {}: {summary}{suffix}", method.span().start().line));
                }
            }
        }
        syn::visit::visit_item_impl(self, item);
    }
}
