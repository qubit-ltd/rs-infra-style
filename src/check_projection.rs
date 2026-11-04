// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Compatibility diagnostics and formatter aggregation for the unified gate.

use std::path::Path;

use anyhow::Result;

use crate::CheckFailure;
use crate::Diagnostic;
use crate::analysis::AnalysisReport;
use crate::analysis::Finding;
use crate::style_formatter;

/// Creates a compatible definite diagnostic without changing its public schema.
///
/// # Parameters
///
/// * `code` - Stable compatibility or native diagnostic identifier.
/// * `path` - Project-relative source path.
/// * `line` - One-based source line, or zero for project-wide failures.
/// * `message` - Explanation of the proven violation or blocked check.
fn diagnostic(code: &str, path: &str, line: usize, message: String) -> Diagnostic {
    Diagnostic {
        schema_version: 1,
        code: code.into(),
        severity: "error".into(),
        path: path.into(),
        line,
        message,
    }
}

/// Preserves existing identifiers for mechanically equivalent native checks.
///
/// # Parameters
///
/// * `finding` - Native finding whose legacy identifier is selected.
fn legacy_code(finding: &Finding) -> String {
    let code = match finding.check_id.as_str() {
        "rustfmt-format" => "FMT",
        "header-template" => "DOC-001",
        "test-file-name" => "STYLE001",
        "test-source-redirection" => "STYLE002",
        "single-object-import" if finding.evidence.contains('*') => "STYLE004",
        "single-object-import" => "STYLE003",
        "import-groups" => {
            let groups = finding
                .evidence
                .strip_prefix("groups ")
                .and_then(|text| text.split_once(" -> "));
            if groups.is_some_and(|(first, rest)| first > rest.split(',').next().unwrap_or(rest)) {
                "STYLE005"
            } else {
                "STYLE006"
            }
        }
        "unused-imports-allow" => "STYLE007",
        "coverage-cfg" => "STYLE008",
        "one-type-per-file" => "STYLE010",
        "type-file-name" => "STYLE011",
        "source-test-pair" => "STYLE014",
        "aggregation-private-imports" => "STYLE016",
        _ => return format!("{}/{}", finding.rule_id, finding.check_id),
    };
    code.into()
}

/// Projects only proven violations and explicit blocked reasons into legacy
/// rows.
///
/// # Parameters
///
/// * `report` - Completed native report to project into compatible diagnostics.
pub(crate) fn diagnostics(report: &AnalysisReport) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|finding| finding.severity == "error")
        .map(|finding| {
            diagnostic(
                &legacy_code(finding),
                &finding.path,
                finding.line,
                finding.message.clone(),
            )
        })
        .collect();
    diagnostics.extend(
        report
            .blocked
            .iter()
            .map(|issue| diagnostic("CHECK-INCOMPLETE", &issue.path, 0, issue.reason.clone())),
    );
    diagnostics
}

/// Runs formatting without leaking subprocess output into JSON diagnostics.
///
/// # Parameters
///
/// * `project` - Project root used to resolve sources and tooling.
/// * `check_only` - Selects read-only validation rather than formatting.
pub(crate) fn format(project: &Path, check_only: bool) -> Result<()> {
    let project = project.canonicalize()?;
    let config = style_formatter::project_rustfmt_config(&project)?;
    let toolchain = style_formatter::project_toolchain(&project)?;
    let mut commands = vec![style_formatter::cargo_fmt_command(
        &project,
        check_only,
        &config,
        toolchain.as_deref(),
    )];
    let fuzz = project.join("fuzz/Cargo.toml");
    if fuzz.is_file() {
        commands.push(style_formatter::cargo_fuzz_fmt_command(
            &fuzz,
            check_only,
            &config,
            toolchain.as_deref(),
        ));
    }
    let mut failures = Vec::new();
    let mut code = 1;
    for mut command in commands {
        let description = style_formatter::cargo_fmt_description(&command);
        match command.current_dir(&project).output() {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let difference = check_only && (stdout.contains("Diff in ") || stderr.contains("Diff in "));
                if !difference {
                    code = 2;
                }
                failures.push(diagnostic(
                    if difference { "FMT" } else { "FMT-INCOMPLETE" },
                    "",
                    0,
                    format!("{description} failed\n{stdout}{stderr}"),
                ));
            }
            Err(error) => {
                code = 2;
                failures.push(diagnostic(
                    "FMT-INCOMPLETE",
                    "",
                    0,
                    format!("failed to start {description}: {error}"),
                ));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(CheckFailure::new(code, failures).into())
    }
}

/// Combines independent checks, giving incomplete analysis priority over
/// violations.
///
/// # Parameters
///
/// * `formatting` - Outcome of the configured formatter commands.
pub(crate) fn combine(formatting: Result<()>, analysis: Result<Vec<Diagnostic>>) -> Result<Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut code = 0;
    if let Err(error) = formatting {
        append_failure(error, &mut code, &mut diagnostics);
    }
    match analysis {
        Ok(findings) => diagnostics.extend(findings),
        Err(error) => append_failure(error, &mut code, &mut diagnostics),
    }
    let precise: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "FMT" && !diagnostic.path.is_empty())
        .map(|diagnostic| diagnostic.path.clone())
        .collect();
    diagnostics.retain(|diagnostic| {
        if diagnostic.code != "FMT" || !diagnostic.path.is_empty() {
            return true;
        }
        let paths: Vec<_> = diagnostic
            .message
            .lines()
            .filter_map(|line| line.strip_prefix("Diff in "))
            .filter_map(|location| location.trim_end_matches(':').rsplit_once(':').map(|(path, _)| path))
            .collect();
        paths.is_empty()
            || !paths
                .iter()
                .all(|path| precise.iter().any(|relative| path.ends_with(&format!("/{relative}"))))
    });
    if code == 0 {
        Ok(diagnostics)
    } else {
        Err(CheckFailure::new(code, diagnostics).into())
    }
}

/// Retains typed details and converts ordinary operational errors to status
/// two.
///
/// # Parameters
///
/// * `error` - Operational or structured check failure to retain.
/// * `code` - CLI exit status associated with these diagnostics.
/// * `diagnostics` - Compatible diagnostic rows retained by the failure.
fn append_failure(error: anyhow::Error, code: &mut i32, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(failure) = error.downcast_ref::<CheckFailure>() {
        *code = (*code).max(failure.exit_code());
        diagnostics.extend_from_slice(failure.diagnostics());
    } else {
        *code = 2;
        diagnostics.push(diagnostic("CHECK-INCOMPLETE", "", 0, format!("{error:#}")));
    }
}

/// Formats every selected source, including files without a Cargo target.
///
/// # Parameters
///
/// * `project` - Repository whose owned sources are formatted.
/// * `source_dir` - Optional production root.
/// * `test_dir` - Optional test root.
///
/// # Errors
///
/// Returns an operational error when a source or configured formatter cannot be
/// used.
pub(crate) fn format_sources(project: &Path, source_dir: Option<&Path>, test_dir: Option<&Path>) -> Result<()> {
    let project = project.canonicalize()?;
    let config = style_formatter::project_rustfmt_config(&project)?;
    let toolchain = style_formatter::project_toolchain(&project)?;
    for path in crate::check_discovery::discover(&project, source_dir, test_dir)? {
        crate::analysis::validate_no_symlink(&project, &path)?;
        let edition = crate::analysis_project::package_edition(&project, &path)
            .ok_or_else(|| anyhow::anyhow!("cannot resolve Rust edition for {path}"))?;
        let mut command = std::process::Command::new("rustup");
        command
            .arg("run")
            .arg(
                toolchain
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("missing formatter toolchain"))?,
            )
            .args([
                "rustfmt",
                "--config",
                "skip_children=true",
                "--edition",
                &edition,
                "--config-path",
            ])
            .arg(&config)
            .arg(project.join(&path));
        let output = command.current_dir(&project).output()?;
        if !output.status.success() {
            return Err(CheckFailure::new(
                2,
                vec![diagnostic(
                    "FMT-INCOMPLETE",
                    &path,
                    0,
                    format!("project rustfmt failed: {}", String::from_utf8_lossy(&output.stderr)),
                )],
            )
            .into());
        }
    }
    Ok(())
}
