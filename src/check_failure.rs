// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Structured failures from the unified mechanical gate.

use std::fmt;

use crate::Diagnostic;

/// A mechanical check failure retaining diagnostics and its CLI exit status.
#[derive(Debug)]
pub struct CheckFailure {
    /// Exit status: one for violations, two for incomplete checks.
    code: i32,
    /// Proven violations and incomplete-check explanations.
    diagnostics: Vec<Diagnostic>,
}

impl CheckFailure {
    /// Constructs a failure after all available checks have completed.
    ///
    /// # Parameters
    ///
    /// * `code` - Mechanical CLI failure status.
    /// * `diagnostics` - Compatible diagnostic array retained by this failure.
    pub(crate) fn new(code: i32, diagnostics: Vec<Diagnostic>) -> Self {
        Self { code, diagnostics }
    }

    /// Returns the process status for this mechanical failure.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        self.code
    }

    /// Returns the existing diagnostic-array representation of the failure.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for CheckFailure {
    /// Describes the failure without discarding individual diagnostic details.
    ///
    /// # Parameters
    ///
    /// * `formatter` - Destination receiving the diagnostic descriptions.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for diagnostic in &self.diagnostics {
            writeln!(
                formatter,
                "{}:{}: {}",
                diagnostic.path, diagnostic.line, diagnostic.message
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for CheckFailure {
    // empty
}
