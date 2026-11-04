// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Structural complexity signals that never impose rejection thresholds.

use syn::visit::Visit;

/// Structural complexity signals that never impose rejection thresholds.
pub(crate) struct Complexity {
    /// Observed branch and match-arm count.
    pub(crate) branches: usize,
    /// Observed loop count.
    pub(crate) loops: usize,
    /// Current block traversal depth.
    pub(crate) depth: usize,
    /// Maximum observed block nesting.
    pub(crate) maximum: usize,
}

impl<'ast> Visit<'ast> for Complexity {
    /// Tracks current and maximum block nesting during the structural walk.
    ///
    /// # Parameters
    ///
    /// Arguments provide the source, project context or analysis state
    /// described by this operation.
    fn visit_block(&mut self, item: &'ast syn::Block) {
        self.depth += 1;
        self.maximum = self.maximum.max(self.depth);
        syn::visit::visit_block(self, item);
        self.depth -= 1;
    }
    /// Counts a conditional branch without imposing a complexity threshold.
    ///
    /// # Parameters
    ///
    /// Arguments provide the source, project context or analysis state
    /// described by this operation.
    fn visit_expr_if(&mut self, item: &'ast syn::ExprIf) {
        self.branches += 1;
        syn::visit::visit_expr_if(self, item);
    }
    /// Counts match arms as review evidence without rejecting large matches.
    ///
    /// # Parameters
    ///
    /// Arguments provide the source, project context or analysis state
    /// described by this operation.
    fn visit_expr_match(&mut self, item: &'ast syn::ExprMatch) {
        self.branches += item.arms.len();
        syn::visit::visit_expr_match(self, item);
    }
    /// Counts for loops for responsibility and complexity review.
    ///
    /// # Parameters
    ///
    /// Arguments provide the source, project context or analysis state
    /// described by this operation.
    fn visit_expr_for_loop(&mut self, item: &'ast syn::ExprForLoop) {
        self.loops += 1;
        syn::visit::visit_expr_for_loop(self, item);
    }
    /// Counts while loops for responsibility and complexity review.
    ///
    /// # Parameters
    ///
    /// Arguments provide the source, project context or analysis state
    /// described by this operation.
    fn visit_expr_while(&mut self, item: &'ast syn::ExprWhile) {
        self.loops += 1;
        syn::visit::visit_expr_while(self, item);
    }
    /// Counts unconditional loops for responsibility and complexity review.
    ///
    /// # Parameters
    ///
    /// Arguments provide the source, project context or analysis state
    /// described by this operation.
    fn visit_expr_loop(&mut self, item: &'ast syn::ExprLoop) {
        self.loops += 1;
        syn::visit::visit_expr_loop(self, item);
    }
}
