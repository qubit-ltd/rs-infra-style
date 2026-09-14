// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Operations exposed by the command-line interface.

use clap::Subcommand;

/// Operations exposed by the command-line interface.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Check formatting and style rules.
    Check,
    /// Format the project and validate style rules.
    Fix {
        /// Print the formatting command without changing files.
        #[arg(long)]
        dry_run: bool,
    },
}
