// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Diagnostic output formats for the command-line interface.

use clap::ValueEnum;

/// Supported diagnostic output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Format {
    /// Human-readable text output.
    Text,
    /// Pretty-printed JSON output.
    Json,
}
