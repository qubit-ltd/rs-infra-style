// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Literal token evidence used to validate source comments.

/// Records source-line ranges occupied by literal tokens, including macro
/// tokens.
///
/// This prevents text inside raw strings from impersonating an allow comment.
#[derive(Default)]
pub(in crate::analysis_rules) struct LiteralLines(
    /// Inclusive source-line ranges of real literal tokens.
    pub(in crate::analysis_rules) Vec<(usize, usize)>,
);
impl LiteralLines {
    /// Records literal spans recursively without expanding a macro body.
    ///
    /// # Parameters
    ///
    /// * `tokens` - Parsed tokens whose original spans still identify source
    ///   lines.
    pub(in crate::analysis_rules) fn collect(&mut self, tokens: proc_macro2::TokenStream) {
        for token in tokens {
            match token {
                proc_macro2::TokenTree::Literal(literal) => {
                    let span = literal.span();
                    self.0.push((span.start().line, span.end().line));
                }
                proc_macro2::TokenTree::Group(group) => self.collect(group.stream()),
                _ => {}
            }
        }
    }
}
