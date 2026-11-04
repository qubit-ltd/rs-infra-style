// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Unified formatting and syntax-backed mechanical style checks.

use std::path::Path;

use anyhow::Result;

use crate::CheckFailure;
use crate::Diagnostic;
use crate::analysis;
use crate::check_discovery;
use crate::check_projection;
use crate::style_formatter;

/// Checks selected project-owned sources using the shared Analyzer.
///
/// # Parameters
///
/// * `project` - Repository root whose sources and context are read.
/// * `source_dir` - Optional explicit production source root.
/// * `test_dir` - Optional explicit test root.
///
/// # Returns
///
/// Returns only proven mechanical violations; semantic signals remain in
/// analyze reports.
///
/// # Errors
///
/// Returns a structured status-two failure when analysis is incomplete.
pub fn check(project: &Path, source_dir: Option<&Path>, test_dir: Option<&Path>) -> Result<Vec<Diagnostic>> {
    check_sources(project, source_dir, test_dir, false)
}

/// Runs shared analysis with optional full-file formatting comparison.
///
/// # Parameters
///
/// * `project` - Project root to inspect.
/// * `source_dir` - Optional production root.
/// * `test_dir` - Optional test root.
/// * `check_formatting` - Compares every selected source with rustfmt output.
fn check_sources(
    project: &Path,
    source_dir: Option<&Path>,
    test_dir: Option<&Path>,
    check_formatting: bool,
) -> Result<Vec<Diagnostic>> {
    let files = check_discovery::discover(project, source_dir, test_dir)?;
    let report = analysis::analyze_scoped(project, &files, None, source_dir, test_dir, check_formatting)?;
    let diagnostics = check_projection::diagnostics(&report);
    if report.exit_code() == 2 {
        return Err(CheckFailure::new(2, diagnostics).into());
    }
    Ok(diagnostics)
}

/// Runs read-only rustfmt checks and the shared Analyzer, retaining both
/// results.
///
/// # Parameters
///
/// * `project` - Repository root to inspect.
/// * `source_dir` - Optional explicit production source root.
/// * `test_dir` - Optional explicit test root.
///
/// # Returns
///
/// Returns definite Analyzer violations when formatting completed successfully.
///
/// # Errors
///
/// Returns a structured failure for format differences or incomplete checks.
pub fn check_project(project: &Path, source_dir: Option<&Path>, test_dir: Option<&Path>) -> Result<Vec<Diagnostic>> {
    let formatting = check_projection::format(project, true);
    let analysis = check_sources(project, source_dir, test_dir, true);
    check_projection::combine(formatting, analysis)
}

/// Prints the compatible diagnostic array or human-readable mechanical results.
///
/// # Parameters
///
/// * `diagnostics` - Proven violations or operational explanations.
/// * `json` - Selects the existing JSON diagnostic array.
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn print_diagnostics(diagnostics: &[Diagnostic], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(diagnostics)?);
    } else {
        for diagnostic in diagnostics {
            println!("error: {}:{}: {}", diagnostic.path, diagnostic.line, diagnostic.message);
        }
        if diagnostics.is_empty() {
            println!("Rust mechanical style checks passed; semantic review remains separate.");
        } else {
            println!(
                "Rust mechanical style checks failed with {} issue(s).",
                diagnostics.len()
            );
        }
    }
    Ok(())
}

/// Formats a project and validates the same mechanical rules as check.
///
/// # Parameters
///
/// * `project` - Repository root to format and inspect.
/// * `dry_run` - Prints the formatter command without writing sources.
///
/// # Errors
///
/// Returns a structured failure when formatting or subsequent analysis fails.
pub fn fix(project: &Path, dry_run: bool) -> Result<()> {
    fix_project(project, None, None, dry_run)
}

/// Formats selected project sources and reports remaining mechanical
/// violations.
///
/// # Parameters
///
/// * `project` - Repository root to format and inspect.
/// * `source_dir` - Optional explicit production source root.
/// * `test_dir` - Optional explicit test root.
/// * `dry_run` - Prints the formatter command without writing sources.
///
/// # Errors
///
/// Returns status one for remaining violations, or two for incomplete checks.
pub fn fix_project(project: &Path, source_dir: Option<&Path>, test_dir: Option<&Path>, dry_run: bool) -> Result<()> {
    if dry_run {
        let config = style_formatter::project_rustfmt_config(project)?;
        let toolchain = style_formatter::project_toolchain(project)?;
        println!(
            "{}",
            style_formatter::cargo_fmt_description(&style_formatter::cargo_fmt_command(
                project,
                false,
                &config,
                toolchain.as_deref()
            ))
        );
        return Ok(());
    }
    let formatting = check_projection::format(project, false)
        .and_then(|()| check_projection::format_sources(project, source_dir, test_dir));
    let analysis = check_sources(project, source_dir, test_dir, true);
    let diagnostics = check_projection::combine(formatting, analysis)?;
    if !diagnostics.is_empty() {
        return Err(CheckFailure::new(1, diagnostics).into());
    }
    Ok(())
}
