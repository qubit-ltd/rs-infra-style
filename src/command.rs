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
    /// Analyze an exact source list without formatting or building.
    Analyze {
        /// JSON array of project-relative Rust source paths.
        #[arg(long)]
        files_from: std::path::PathBuf,
        /// Immutable standard snapshot to validate against the compiled
        /// authority.
        #[arg(long, alias = "rules-directory")]
        standards: Option<std::path::PathBuf>,
        /// Also check repository README structure in separate report fields.
        #[arg(long)]
        repository_readme: bool,
    },
    /// Format the project and validate style rules.
    Fix {
        /// Print the formatting command without changing files.
        #[arg(long)]
        dry_run: bool,
    },
}
