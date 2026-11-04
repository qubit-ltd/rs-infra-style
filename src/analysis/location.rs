// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Location report contract.

use serde::Serialize;

/// A source location associated with a finding.
///
/// # Examples
///
/// ```
/// use qubit_infra_style::analysis::Location;
/// fn observe(value: &Location) { let _ = value; }
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct Location {
    /// Project-relative source path.
    pub path: String,
    /// One-based line number.
    pub line: usize,
    /// One-based column number.
    pub column: usize,
}
