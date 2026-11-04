// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Opt-in README structure evidence, with unresolved template content explicit.

use std::path::Path;

use serde_json::to_vec;
use toml::Value;
use toml::from_str;

use crate::analysis::AnalysisIssue;
use crate::analysis::AnalysisReport;
use crate::analysis::ContextFile;
use crate::analysis::Coverage;
use crate::analysis::Finding;
use crate::analysis::digest;
use crate::analysis::read_safe;

/// Creates a repository finding without attributing it to Rust source.
///
/// # Parameters
///
/// * `path` - Repository-relative README path.
/// * `check` - Stable structural subcheck identifier.
/// * `message` - Observable violation of the authoritative structure.
///
/// # Returns
///
/// Returns one error diagnostic for the repository report.
fn issue(path: &str, check: &str, message: &str) -> Finding {
    Finding {
        rule_id: "README-001".into(),
        check_id: check.into(),
        severity: "error".into(),
        path: path.into(),
        line: 1,
        column: 1,
        message: message.into(),
        evidence: format!("Repository README structure: {path}"),
        help: "Follow readme-structure.md with this project's actual metadata and entry points".into(),
        related_locations: Vec::new(),
    }
}

/// Records a bounded subcheck without upgrading the entire README rule.
///
/// # Parameters
///
/// * `report` - Report receiving the independent repository evidence.
/// * `path` - Repository-relative README path.
/// * `check` - Stable subcheck identifier.
/// * `status` - Result for this bounded subcheck.
/// * `evidence` - Observable result and remaining limitations.
fn cover(report: &mut AnalysisReport, path: &str, check: &str, status: &str, evidence: &str) {
    report.repository_coverage.push(Coverage {
        path: path.into(),
        rule_id: "README-001".into(),
        check_id: check.into(),
        status: status.into(),
        evidence: evidence.into(),
    });
}

/// Binds the bytes read in this scope, detecting changes since source analysis.
///
/// # Parameters
///
/// * `report` - Report whose context inventory is updated.
/// * `path` - Repository-relative input path.
/// * `bytes` - Exact bytes consumed by the README checker.
fn bind_context(report: &mut AnalysisReport, path: &str, bytes: &[u8]) {
    let current = digest(bytes);
    if let Some(previous) = report.context_files.iter_mut().find(|entry| entry.path == path) {
        if previous.digest != current {
            report.blocked.push(AnalysisIssue {
                path: path.into(),
                reason: "Context changed between source and README analysis".into(),
            });
            previous.digest = current;
        }
    } else {
        report.context_files.push(ContextFile {
            path: path.into(),
            digest: current,
        });
    }
}

/// Collects ATX and Setext headings outside fenced code and HTML comments.
///
/// The position is the final heading line, so Setext titles use their underline
/// when checking the following badge block. Unsupported HTML blocks are marked
/// uncertain rather than used to prove a missing heading.
///
/// # Parameters
///
/// * `text` - Complete README Markdown.
///
/// # Returns
///
/// Returns heading end positions, levels and titles, plus parsing uncertainty.
fn headings(text: &str) -> (Vec<(usize, usize, String)>, bool) {
    let mut fence: Option<(char, usize)> = None;
    let mut comment = false;
    let mut uncertain = false;
    let mut previous: Option<String> = None;
    let mut result = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if let Some((marker, size)) = fence {
            let length = trimmed.chars().take_while(|value| *value == marker).count();
            if indent <= 3 && length >= size && trimmed[length..].trim().is_empty() {
                fence = None;
            }
            continue;
        }
        if comment {
            comment = !trimmed.contains("-->");
            previous = None;
            continue;
        }
        if trimmed.starts_with("<!--") {
            comment = !trimmed.contains("-->");
            previous = None;
            continue;
        }
        if trimmed.starts_with('<') {
            uncertain = true;
            previous = None;
            continue;
        }
        if indent >= 4 || line[..indent].contains('\t') {
            previous = None;
            continue;
        }
        if let Some(marker) = trimmed.chars().next().filter(|marker| matches!(marker, '`' | '~')) {
            let length = trimmed.chars().take_while(|value| *value == marker).count();
            if length >= 3 && (marker != '`' || !trimmed[length..].contains('`')) {
                fence = Some((marker, length));
                previous = None;
                continue;
            }
        }
        let hashes = trimmed.bytes().take_while(|value| *value == b'#').count();
        if (1..=6).contains(&hashes) && (trimmed.len() == hashes || trimmed.as_bytes()[hashes].is_ascii_whitespace()) {
            let title = trimmed[hashes..].trim();
            let suffix = title.trim_end_matches('#');
            let title = if suffix.len() != title.len() && suffix.ends_with(char::is_whitespace) {
                suffix.trim_end()
            } else {
                title
            };
            result.push((index, hashes, title.to_owned()));
            previous = None;
            continue;
        }
        let underline = trimmed.trim_end();
        if !underline.is_empty()
            && (underline.bytes().all(|value| value == b'=') || underline.bytes().all(|value| value == b'-'))
            && let Some(title) = previous.take()
        {
            result.push((index, if underline.starts_with('=') { 1 } else { 2 }, title));
            continue;
        }
        previous = if trimmed.is_empty() || trimmed.starts_with(['>', '-', '*', '+', '[']) {
            None
        } else {
            Some(trimmed.to_owned())
        };
    }
    (result, uncertain)
}

/// Reads Cargo badge metadata and binds even Git-ignored manifest bytes.
///
/// # Parameters
///
/// * `project` - Root containing the package or workspace metadata.
/// * `report` - Report receiving context and read/parse failures.
///
/// # Returns
///
/// Returns parsed Cargo metadata, or no value after an explicit blocker.
fn metadata(project: &Path, report: &mut AnalysisReport) -> Option<Value> {
    let parsed = read_safe(project, "Cargo.toml").and_then(|bytes| {
        bind_context(report, "Cargo.toml", &bytes);
        let text = String::from_utf8(bytes)?;
        Ok(from_str::<Value>(&text)?)
    });
    match parsed {
        Ok(value) => Some(value),
        Err(error) => {
            report.blocked.push(AnalysisIssue {
                path: "Cargo.toml".into(),
                reason: format!("README metadata cannot be read safely: {error:#}"),
            });
            None
        }
    }
}

/// Checks the README pair while keeping unimplemented template checks pending.
///
/// # Parameters
///
/// * `project` - Root containing README files and Cargo metadata.
/// * `report` - Analysis report receiving separate repository findings.
pub(crate) fn inspect(project: &Path, report: &mut AnalysisReport) {
    let metadata = metadata(project, report);
    let package = metadata.as_ref().and_then(|value| value.get("package"));
    let workspace = metadata
        .as_ref()
        .and_then(|value| value.get("workspace"))
        .and_then(|value| value.get("package"));
    let value = |name: &str| {
        let field = package.and_then(|package| package.get(name));
        match field {
            Some(Value::String(value)) => Some(value.as_str()),
            Some(Value::Table(table)) if table.get("workspace").and_then(Value::as_bool) == Some(true) => {
                workspace.and_then(|package| package.get(name)).and_then(Value::as_str)
            }
            _ => None,
        }
    };
    let repository = value("repository").map(|value| value.trim_end_matches('/').trim_end_matches(".git"));
    let crate_name = package.and_then(|package| package.get("name")).and_then(Value::as_str);
    let rust_version = value("rust-version");
    let metadata_complete = repository.is_some() && crate_name.is_some() && rust_version.is_some();
    let mut states = Vec::new();
    for (path, chinese) in [("README.md", false), ("README.zh_CN.md", true)] {
        let bytes = match read_safe(project, path) {
            Ok(bytes) => bytes,
            Err(error) => {
                states.push((path, "missing-or-unreadable".to_owned()));
                if !project.join(path).exists() {
                    report.repository_diagnostics.push(issue(
                        path,
                        "readme-presence",
                        "Both English and Simplified Chinese README files are required",
                    ));
                    cover(
                        report,
                        path,
                        "readme-presence",
                        "fail",
                        "Required README counterpart is missing",
                    );
                } else {
                    report.blocked.push(AnalysisIssue {
                        path: path.into(),
                        reason: format!("Repository README cannot be read safely: {error:#}"),
                    });
                    cover(
                        report,
                        path,
                        "readme-presence",
                        "blocked",
                        "Required README cannot be read safely",
                    );
                }
                continue;
            }
        };
        states.push((path, digest(&bytes)));
        bind_context(report, path, &bytes);
        cover(
            report,
            path,
            "readme-presence",
            "pass",
            "Required README exists and its bytes were read safely",
        );
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                report.blocked.push(AnalysisIssue {
                    path: path.into(),
                    reason: format!("README is not UTF-8: {error}"),
                });
                continue;
            }
        };
        let (headers, uncertain) = headings(&text);
        let h1: Vec<_> = headers.iter().filter(|(_, level, _)| *level == 1).collect();
        if h1.len() != 1 && !uncertain {
            report.repository_diagnostics.push(issue(
                path,
                "single-project-title",
                "README must have exactly one H1 project title",
            ));
        }
        cover(
            report,
            path,
            "single-project-title",
            if uncertain {
                "needs_semantic_review"
            } else if h1.len() == 1 {
                "pass"
            } else {
                "fail"
            },
            "ATX and Setext headings outside fenced code/comments; unsupported HTML requires review",
        );
        let h2: Vec<_> = headers.iter().filter(|(_, level, _)| *level == 2).collect();
        let expected = if chinese {
            ["测试", "许可证", "贡献", "作者"]
        } else {
            ["Testing", "License", "Contributing", "Author"]
        };
        let correct_order = h2.len() >= 4
            && h2[h2.len() - 4..]
                .iter()
                .map(|(_, _, title)| title.as_str())
                .eq(expected);
        if !correct_order && !uncertain {
            report.repository_diagnostics.push(issue(
                path,
                "final-section-order",
                "The final four H2 sections must match the required testing/license/contributing/author order",
            ));
        }
        cover(
            report,
            path,
            "final-section-order",
            if uncertain {
                "needs_semantic_review"
            } else if correct_order {
                "pass"
            } else {
                "fail"
            },
            "Compared final four H2 headings; fixed section contents and trailing prose require separate review",
        );
        let badge_status = if !metadata_complete {
            report.blocked.push(AnalysisIssue {
                path: path.into(),
                reason: "README badge metadata unresolved: package name, repository URL and rust-version are required"
                    .into(),
            });
            "blocked"
        } else {
            let repository = repository.unwrap();
            let parts: Vec<_> = repository.rsplit('/').take(2).collect();
            if parts.len() != 2 || !repository.starts_with("https://github.com/") || uncertain || h1.len() != 1 {
                "needs_semantic_review"
            } else {
                let (name, owner) = (parts[0], parts[1]);
                let crate_name = crate_name.unwrap();
                let rust_version = rust_version.unwrap();
                let badges = [
                    format!(
                        "[![Rust CI]({repository}/actions/workflows/ci.yml/badge.svg)]({repository}/actions/workflows/ci.yml)"
                    ),
                    format!(
                        "[![Coverage](https://img.shields.io/endpoint?url=https://{owner}.github.io/{name}/coverage-badge.json)](https://{owner}.github.io/{name}/coverage/)"
                    ),
                    format!(
                        "[![Crates.io](https://img.shields.io/crates/v/{crate_name}.svg?color=blue)](https://crates.io/crates/{crate_name})"
                    ),
                    format!(
                        "[![Rust](https://img.shields.io/badge/rust-{rust_version}+-blue.svg?logo=rust)](https://www.rust-lang.org)"
                    ),
                    "[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)".into(),
                    if chinese {
                        "[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)"
                            .into()
                    } else {
                        "[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)".into()
                    },
                ];
                let line = h1[0].0;
                let lines: Vec<_> = text.lines().collect();
                let matches = lines.get(line + 1) == Some(&"")
                    && lines.get(line + 8) == Some(&"")
                    && badges
                        .iter()
                        .enumerate()
                        .all(|(index, badge)| lines.get(line + index + 2).copied() == Some(badge.as_str()));
                if !matches {
                    report.repository_diagnostics.push(issue(path, "opening-badge-block", "The opening badge block must match the six authoritative badges with resolved project metadata"));
                }
                if matches { "pass" } else { "fail" }
            }
        };
        cover(
            report,
            path,
            "opening-badge-block",
            badge_status,
            "Exact six-badge template compared only when project metadata and heading boundaries resolve",
        );
        let failed = report.repository_diagnostics.iter().any(|finding| finding.path == path);
        cover(
            report,
            path,
            "readme-structure",
            if failed {
                "fail"
            } else if !metadata_complete {
                "blocked"
            } else {
                "needs_semantic_review"
            },
            "Checked structural subchecks separately; complete fixed section bodies, project script entry points and trailing author content remain to be reviewed",
        );
        cover(
            report,
            path,
            "final-section-template-content",
            "needs_semantic_review",
            "Compare authoritative section text after resolving actual project script entry points; heading order alone does not prove template compliance",
        );
        cover(
            report,
            path,
            "bilingual-semantic-equivalence",
            "needs_semantic_review",
            "Feature, limitation, example and public behavior equivalence requires bilingual semantic review",
        );
    }
    report.context_files.sort_by(|left, right| left.path.cmp(&right.path));
    report.input_digest =
        digest(to_vec(&(&report.input_digest, "repository-readme", states, &report.context_files)).unwrap());
}
