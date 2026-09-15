// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Machine-readable diagnostics produced by the style checker.

use serde::Serialize;

/// A machine-readable style diagnostic.
///
/// # Examples
///
/// ~~~
/// use qubit_infra_style::Diagnostic;
///
/// let diagnostic = Diagnostic {
///     schema_version: 1,
///     code: String::from("STYLE004"),
///     severity: String::from("error"),
///     path: String::from("src/lib.rs"),
///     line: 1,
///     message: String::from("example"),
/// };
/// assert_eq!(diagnostic.schema_version, 1);
/// ~~~
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
    /// Version of the serialized diagnostic schema.
    pub schema_version: u8,
    /// Stable rule identifier, such as `STYLE004`.
    pub code: String,
    /// Human-readable severity, currently `error`.
    pub severity: String,
    /// Project-relative source path using forward slashes.
    pub path: String,
    /// One-based source line, or zero for file-level diagnostics.
    pub line: usize,
    /// Human-readable explanation of the violation.
    pub message: String,
}
