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

use std::fs;
use std::process;

use anyhow::Result;
use clap::Parser;
use qubit_infra_style::CheckFailure;
use qubit_infra_style::Diagnostic;
use qubit_infra_style::check_project;
use qubit_infra_style::fix_project;
use qubit_infra_style::print_diagnostics;

use crate::cli::Cli;
use crate::command::Command;
use crate::format::Format;

/// Parses arguments and executes the requested style operation.
fn main() {
    let cli = Cli::parse();
    if let Command::Analyze {
        files_from,
        standards,
        repository_readme,
    } = &cli.command
    {
        let result = (|| -> Result<i32> {
            let files: Vec<String> = serde_json::from_slice(&fs::read(files_from)?)?;
            let mut report = qubit_infra_style::analysis::analyze(&cli.project, &files, standards.as_deref())?;
            if *repository_readme {
                qubit_infra_style::analysis::analyze_repository_readme(&cli.project, &mut report)?;
            }
            if cli.format == Format::Json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for finding in report.diagnostics.iter().chain(&report.repository_diagnostics) {
                    println!(
                        "{}: {}:{}:{} [{} / {}] {}",
                        finding.severity,
                        finding.path,
                        finding.line,
                        finding.column,
                        finding.rule_id,
                        finding.check_id,
                        finding.message
                    );
                }
                for blocked in &report.blocked {
                    eprintln!("blocked: {}: {}", blocked.path, blocked.reason);
                }
                println!("Mechanical analysis only; semantic review remains required.");
            }
            Ok(report.exit_code())
        })();
        match result {
            Ok(code) => process::exit(code),
            Err(error) => {
                eprintln!("analysis failed: {error:#}");
                process::exit(2);
            }
        }
    }
    let json = cli.format == Format::Json;
    let is_check = matches!(&cli.command, Command::Check);
    match run(cli) {
        Ok(success) => {
            if json {
                let icon = if success { "✅" } else { "❌" };
                eprintln!(
                    "{icon} Rust mechanical style checks {}; semantic review remains separate.",
                    if success { "passed" } else { "failed" }
                );
            } else if success && !is_check {
                println!("✅ Rust style operation completed successfully.");
            }
            if !success {
                process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("❌ Rust style operation failed: {error:#}");
            let exit_code = if let Some(failure) = error.downcast_ref::<CheckFailure>() {
                if json && let Err(output_error) = print_diagnostics(failure.diagnostics(), true) {
                    eprintln!("failed to serialize style diagnostics: {output_error:#}");
                    process::exit(2);
                }
                failure.exit_code()
            } else {
                if json {
                    let diagnostics = [Diagnostic {
                        schema_version: 1,
                        code: "CHECK-ERROR".into(),
                        severity: "error".into(),
                        path: String::new(),
                        line: 0,
                        message: format!("{error:#}"),
                    }];
                    if let Err(output_error) = print_diagnostics(&diagnostics, true) {
                        eprintln!("failed to serialize style diagnostics: {output_error:#}");
                    }
                }
                2
            };
            process::exit(exit_code);
        }
    }
}

/// Executes one formatting operation and the shared mechanical rule gate.
///
/// # Parameters
///
/// * `cli` - Parsed project, output and operation options.
///
/// # Returns
///
/// Returns whether completed mechanical checks have no proved violations.
/// Semantic review signals do not fail the gate.
///
/// # Errors
///
/// Returns a typed gate failure or an operational error; neither is a passing
/// result. The caller preserves typed diagnostic arrays and exit-code priority.
fn run(cli: Cli) -> Result<bool> {
    let project = fs::canonicalize(&cli.project)?;
    match cli.command {
        Command::Analyze { .. } => unreachable!("analysis is handled separately"),
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
