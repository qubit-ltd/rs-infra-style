// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Import order evidence without inspecting unrelated formatted bodies.

use quote::ToTokens;
use syn::visit::Visit;

/// Ordered import declarations encountered during AST traversal.
pub(crate) struct ImportInventory {
    /// Token-normalized use declarations in source traversal order.
    pub(crate) imports: Vec<String>,
}

impl<'ast> Visit<'ast> for ImportInventory {
    /// Records one use declaration without rewriting its token structure.
    ///
    /// # Parameters
    ///
    /// * `item` - Parsed use declaration visited in source order.
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.imports.push(item.to_token_stream().to_string());
    }
}
