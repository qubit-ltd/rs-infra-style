// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! AnalysisReport report contract.

use serde::Serialize;

use super::AnalysisIssue;
use super::ContextFile;
use super::Coverage;
use super::Finding;

/// Versioned report that keeps mechanical checks separate from semantic review.
///
/// # Examples
///
/// ```
/// use qubit_infra_style::analysis::AnalysisReport;
/// fn observe(value: &AnalysisReport) { let _ = value; }
/// ```
#[derive(Debug, Serialize)]
pub struct AnalysisReport {
    /// Serialization contract version.
    pub schema_version: u8,
    /// Analyzer package version, for cache invalidation.
    pub analyzer_version: String,
    /// Digest of the supplied standard snapshot.
    pub standards_digest: String,
    /// Digest of the standard compiled into this analyzer.
    pub supported_standards_digest: String,
    /// Digest binding source, context, configuration, and analyzer identity.
    pub input_digest: String,
    /// Exact requested file set, sorted and deduplicated.
    pub files: Vec<String>,
    /// Proved violations and semantic review signals.
    pub diagnostics: Vec<Finding>,
    /// Per-file normative and machine-subcheck coverage.
    pub coverage: Vec<Coverage>,
    /// Opt-in repository findings, separate from source diagnostics.
    pub repository_diagnostics: Vec<Finding>,
    /// Opt-in repository structure and semantic coverage.
    pub repository_coverage: Vec<Coverage>,
    /// Analysis limitations which prevent a complete mechanical result.
    pub blocked: Vec<AnalysisIssue>,
    /// Explicitly excluded generated or third-party files.
    pub excluded: Vec<AnalysisIssue>,
    /// All context files read, including the requested sources.
    pub context_files: Vec<ContextFile>,
}

impl AnalysisReport {
    /// Returns 2 for incomplete analysis, 1 for violations, otherwise 0.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        if !self.blocked.is_empty() {
            2
        } else if self
            .diagnostics
            .iter()
            .chain(&self.repository_diagnostics)
            .any(|finding| finding.severity == "error")
        {
            1
        } else {
            0
        }
    }
}
