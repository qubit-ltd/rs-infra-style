// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! AnalysisIssue report contract.

use serde::Serialize;

/// An excluded input or a reason analysis could not be completed.
///
/// # Examples
///
/// ```
/// use qubit_infra_style::analysis::AnalysisIssue;
/// fn observe(value: &AnalysisIssue) { let _ = value; }
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisIssue {
    /// Source path, or an empty string for an analysis-wide issue.
    pub path: String,
    /// Explanation of the limitation.
    pub reason: String,
}
