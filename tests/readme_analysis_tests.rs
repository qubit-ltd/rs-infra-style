// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Independent README scope regressions for metadata and Markdown boundaries.

use std::fs;
use std::path::Path;
use std::process::Command;

use qubit_infra_style::analysis::AnalysisReport;
use qubit_infra_style::analysis::analyze;
use qubit_infra_style::analysis::analyze_repository_readme;

fn manifest(root: &Path, name: &str, repository: &str) {
    fs::write(
        root.join("Cargo.toml"),
        format!("[package]\nname={name:?}\nversion=\"0.1.0\"\nrepository={repository:?}\nrust-version=\"1.94\"\n"),
    )
    .unwrap();
}

fn readmes(root: &Path, setext: bool, middle: &str) {
    let repository = "https://github.com/example/demo";
    let badges = [
        format!("[![Rust CI]({repository}/actions/workflows/ci.yml/badge.svg)]({repository}/actions/workflows/ci.yml)"),
        "[![Coverage](https://img.shields.io/endpoint?url=https://example.github.io/demo/coverage-badge.json)](https://example.github.io/demo/coverage/)".into(),
        "[![Crates.io](https://img.shields.io/crates/v/foo.svg?color=blue)](https://crates.io/crates/foo)".into(),
        "[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)".into(),
        "[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)".into(),
    ];
    for (path, chinese) in [("README.md", false), ("README.zh_CN.md", true)] {
        let language = if chinese {
            "[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)"
        } else {
            "[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)"
        };
        let footer = if chinese {
            ["测试", "许可证", "贡献", "作者"]
        } else {
            ["Testing", "License", "Contributing", "Author"]
        };
        let title = if setext { "Demo\n====" } else { "# Demo" };
        let sections = footer
            .iter()
            .map(|title| format!("## {title}\n\nBody."))
            .collect::<Vec<_>>()
            .join("\n\n");
        fs::write(
            root.join(path),
            format!(
                "{title}\n\n{}\n{language}\n\nIntroduction.\n{middle}\n{sections}\n",
                badges.join("\n")
            ),
        )
        .unwrap();
    }
}

fn report(root: &Path) -> AnalysisReport {
    let mut report = analyze(root, &[], None).unwrap();
    analyze_repository_readme(root, &mut report).unwrap();
    report
}

#[test]
fn test_ignored_cargo_metadata_is_bound_to_the_readme_digest() {
    let root = tempfile::tempdir().unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(root.path())
            .status()
            .unwrap()
            .success()
    );
    fs::write(root.path().join(".gitignore"), "Cargo.toml\n").unwrap();
    manifest(root.path(), "foo", "https://github.com/example/demo");
    readmes(root.path(), false, "");
    let before = report(root.path());
    assert_eq!(before.exit_code(), 0, "{:?}", before.blocked);
    assert!(before.context_files.iter().any(|entry| entry.path == "Cargo.toml"));
    manifest(root.path(), "bar", "https://github.com/example/demo");
    let after = report(root.path());
    assert_eq!(after.exit_code(), 1);
    assert_ne!(
        before.input_digest, after.input_digest,
        "Consumed ignored metadata must affect the report identity"
    );
}

#[test]
fn test_code_fence_language_text_does_not_close_a_fence() {
    let root = tempfile::tempdir().unwrap();
    manifest(root.path(), "foo", "https://github.com/example/demo");
    readmes(
        root.path(),
        false,
        "\n```text\n```rust\n# Not another title\n## Not another section\n```\n",
    );
    let report = report(root.path());
    assert_eq!(report.exit_code(), 0, "{:?}", report.repository_diagnostics);
    assert!(
        report
            .repository_coverage
            .iter()
            .filter(|entry| entry.check_id == "single-project-title")
            .all(|entry| entry.status == "pass")
    );
}

#[test]
fn test_setext_titles_use_the_heading_end_for_badge_placement() {
    let root = tempfile::tempdir().unwrap();
    manifest(root.path(), "foo", "https://github.com/example/demo");
    readmes(root.path(), true, "");
    let report = report(root.path());
    assert_eq!(report.exit_code(), 0, "{:?}", report.repository_diagnostics);
    assert!(
        report
            .repository_coverage
            .iter()
            .filter(|entry| entry.check_id == "opening-badge-block")
            .all(|entry| entry.status == "pass")
    );
}

#[test]
fn test_unresolved_badges_and_unchecked_footer_never_certify_the_whole_structure() {
    let root = tempfile::tempdir().unwrap();
    manifest(root.path(), "foo", "https://gitlab.com/example/demo");
    readmes(root.path(), false, "");
    let report = report(root.path());
    assert_eq!(report.exit_code(), 0, "{:?}", report.repository_diagnostics);
    for check in [
        "opening-badge-block",
        "readme-structure",
        "final-section-template-content",
    ] {
        assert!(
            report
                .repository_coverage
                .iter()
                .filter(|entry| entry.check_id == check)
                .all(|entry| entry.status == "needs_semantic_review"),
            "{check}"
        );
    }
}

#[test]
fn test_metadata_changed_between_source_and_readme_scopes_blocks_completion() {
    let root = tempfile::tempdir().unwrap();
    manifest(root.path(), "foo", "https://github.com/example/demo");
    readmes(root.path(), false, "");
    let mut report = analyze(root.path(), &[], None).unwrap();
    manifest(root.path(), "bar", "https://github.com/example/demo");
    analyze_repository_readme(root.path(), &mut report).unwrap();
    assert_eq!(report.exit_code(), 2);
    assert!(
        report
            .blocked
            .iter()
            .any(|entry| entry.path == "Cargo.toml" && entry.reason.contains("changed"))
    );
}

#[test]
fn test_tab_indented_code_does_not_create_a_second_title() {
    let root = tempfile::tempdir().unwrap();
    manifest(root.path(), "foo", "https://github.com/example/demo");
    readmes(root.path(), false, "\n\t# Literal indented code\n");
    let report = report(root.path());
    assert_eq!(report.exit_code(), 0, "{:?}", report.repository_diagnostics);
    assert!(
        report
            .repository_coverage
            .iter()
            .filter(|entry| entry.check_id == "single-project-title")
            .all(|entry| entry.status == "pass")
    );
}

#[test]
fn test_workspace_badge_metadata_requires_explicit_package_inheritance() {
    let root = tempfile::tempdir().unwrap();
    readmes(root.path(), false, "");
    let manifest = "[package]\nname=\"foo\"\nversion=\"0.1.0\"\n[workspace.package]\nrepository=\"https://github.com/example/demo\"\nrust-version=\"1.94\"\n";
    fs::write(root.path().join("Cargo.toml"), manifest).unwrap();
    let unresolved = report(root.path());
    assert_eq!(unresolved.exit_code(), 2);
    fs::write(
        root.path().join("Cargo.toml"),
        manifest.replace(
            "version=\"0.1.0\"",
            "version=\"0.1.0\"\nrepository.workspace=true\nrust-version.workspace=true",
        ),
    )
    .unwrap();
    let inherited = report(root.path());
    assert_eq!(inherited.exit_code(), 0, "{:?}", inherited.blocked);
}
