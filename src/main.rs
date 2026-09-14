// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Command-line entry point for the style checker.

mod cli;
mod command;
mod format;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use command::Command;
use format::Format;
use qubit_infra_style::check_project;
use qubit_infra_style::fix_project;
use qubit_infra_style::print_diagnostics;

/// Parses arguments and executes the requested style operation.
fn main() -> Result<()> {
    let cli = Cli::parse();
    let project = std::fs::canonicalize(&cli.project)?;
    match cli.command {
        Command::Check => {
            let diagnostics =
                check_project(&project, cli.source_dir.as_deref(), cli.test_dir.as_deref())?;
            print_diagnostics(&diagnostics, cli.format == Format::Json)?;
            if diagnostics.is_empty() {
                Ok(())
            } else {
                std::process::exit(1)
            }
        }
        Command::Fix { dry_run } => fix_project(
            &project,
            cli.source_dir.as_deref(),
            cli.test_dir.as_deref(),
            dry_run,
        ),
    }
}
