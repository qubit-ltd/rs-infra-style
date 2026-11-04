// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! ContextFile report contract.

use serde::Serialize;

/// A content-addressed file read as analysis context.
///
/// # Examples
///
/// ```
/// use qubit_infra_style::analysis::ContextFile;
/// fn observe(value: &ContextFile) { let _ = value; }
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct ContextFile {
    /// Project-relative path.
    pub path: String,
    /// SHA-256 of the actual bytes read.
    pub digest: String,
}
