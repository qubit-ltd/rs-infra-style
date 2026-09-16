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
use qubit_infra_style::check_project;
use qubit_infra_style::fix_project;
use qubit_infra_style::print_diagnostics;

use crate::cli::Cli;
use crate::command::Command;
use crate::format::Format;

/// Parses arguments and executes the requested style operation.
fn main() {
    let cli = Cli::parse();
    let json = cli.format == Format::Json;
    let is_check = matches!(&cli.command, Command::Check);
    match run(cli) {
        Ok(success) => {
            if json {
                eprintln!("Rust style checks {}.", if success { "passed" } else { "failed" });
            } else if success && !is_check {
                println!("Rust style operation completed successfully.");
            }
            if !success {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("Rust style operation failed: {error:#}");
            std::process::exit(1);
        }
    }
}

/// Executes one CLI operation and returns whether it passed its checks.
fn run(cli: Cli) -> Result<bool> {
    let project = std::fs::canonicalize(&cli.project)?;
    match cli.command {
        Command::Check => {
            let diagnostics = check_project(&project, cli.source_dir.as_deref(), cli.test_dir.as_deref())?;
            print_diagnostics(&diagnostics, cli.format == Format::Json)?;
            Ok(diagnostics.is_empty())
        }
        Command::Fix { dry_run } => {
            fix_project(&project, cli.source_dir.as_deref(), cli.test_dir.as_deref(), dry_run)?;
            Ok(true)
        }
    }
}
