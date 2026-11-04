// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! A reasoned exception for one recognized rule and project-relative file.

use serde::Deserialize;

/// Describes one permitted style exception for a project file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StyleException {
    /// Names the style rule that is allowed to have an exception.
    pub(crate) rule: String,
    /// Gives the project-relative path to which the exception applies.
    pub(crate) path: String,
    /// Explains why this specific rule and path are exempted.
    pub(crate) reason: String,
}
