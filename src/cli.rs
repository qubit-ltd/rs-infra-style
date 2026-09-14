// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Command-line argument definitions for the style checker.

use std::path::PathBuf;

use clap::Parser;

use crate::command::Command;
use crate::format::Format;

/// Command-line options for the style checker.
#[derive(Debug, Parser)]
#[command(name = "rs-infra-style")]
pub(crate) struct Cli {
    /// Project root to inspect.
    #[arg(long, default_value = ".")]
    pub(crate) project: PathBuf,
    /// Source directory relative to the project root.
    #[arg(long)]
    pub(crate) source_dir: Option<PathBuf>,
    /// Test directory relative to the project root.
    #[arg(long)]
    pub(crate) test_dir: Option<PathBuf>,
    /// Diagnostic output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub(crate) format: Format,
    /// Requested style operation.
    #[command(subcommand)]
    pub(crate) command: Command,
}
