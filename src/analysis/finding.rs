// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Finding report contract.

use serde::Serialize;

use super::Location;

/// A proved violation or a clearly labelled semantic review signal.
///
/// # Examples
///
/// ```
/// use qubit_infra_style::analysis::Finding;
/// fn observe(value: &Finding) { let _ = value; }
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// Normative rule identifier.
    pub rule_id: String,
    /// Stable machine-check identifier within the rule.
    pub check_id: String,
    /// Either `error` or `info`; review signals never fail the mechanical gate.
    pub severity: String,
    /// Project-relative source path.
    pub path: String,
    /// One-based source line.
    pub line: usize,
    /// One-based source column.
    pub column: usize,
    /// Human-readable diagnostic.
    pub message: String,
    /// Observable evidence supporting the diagnostic.
    pub evidence: String,
    /// Suggested repair or review direction.
    pub help: String,
    /// Other source locations relevant to this diagnostic.
    pub related_locations: Vec<Location>,
}
