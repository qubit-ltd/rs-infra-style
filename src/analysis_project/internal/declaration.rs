// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! An observed type or alias declaration used to qualify ownership evidence.

/// An observed type or alias declaration used to qualify ownership evidence.
#[derive(Clone)]
pub(crate) struct Declaration {
    /// Project-relative source containing this item.
    pub(crate) path: String,
    /// Declared type or alias identifier.
    pub(crate) name: String,
    /// One-based declaration line.
    pub(crate) line: usize,
    /// Whether unresolved conditional syntax affects the declaration.
    pub(crate) conditional: bool,
    /// Whether the declaration carries an explicit must_use attribute.
    pub(crate) must_use: bool,
}
