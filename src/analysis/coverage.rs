// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Coverage report contract.

use serde::Serialize;

/// One explicit rule/subcheck coverage decision.
///
/// # Examples
///
/// ```
/// use qubit_infra_style::analysis::Coverage;
/// fn observe(value: &Coverage) { let _ = value; }
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct Coverage {
    /// Source path to which this coverage applies.
    pub path: String,
    /// Normative rule identifier.
    pub rule_id: String,
    /// Stable subcheck identifier.
    pub check_id: String,
    /// `pass`, `fail`, `needs_semantic_review`, `not_applicable`, or `blocked`.
    pub status: String,
    /// Concrete reason for the decision.
    pub evidence: String,
}
