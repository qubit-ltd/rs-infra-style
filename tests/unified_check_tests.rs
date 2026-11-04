// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! CLI regression contracts for the unified formatting and mechanical gate.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::process::Output;

const HEADER: &str = "// =============================================================================\n//    Copyright (c) 2025 - 2026 Haixing Hu.\n//\n//    SPDX-License-Identifier: Apache-2.0\n//\n//    Licensed under the Apache License, Version 2.0.\n// =============================================================================\n";
const DOCUMENTED_VALUE: &str = "/// Returns the fixture identity.\n///\n/// # Returns\n///\n/// Returns the stable identity value one.\n#[must_use]\n#[inline]\npub fn value() -> u8 {\n    1\n}\n";

/// Creates a dependency-free, formatted source project and its tooling policy.
fn project() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temporary source project");
    let root = directory.path();
    fs::create_dir_all(root.join("src")).expect("source directory");
    fs::create_dir_all(root.join(".infra/style")).expect("style directory");
    fs::create_dir_all(root.join(".infra/tools")).expect("tool defaults directory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"unified-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("manifest");
    fs::write(root.join(".infra/style/rustfmt.toml"), "edition = \"2024\"\n").expect("format policy");
    fs::write(
        root.join(".infra/tools/defaults.toml"),
        "nightly_toolchain = \"nightly-2026-06-05\"\n",
    )
    .expect("toolchain policy");
    fs::write(
        root.join("src/lib.rs"),
        format!("{HEADER}//! Fixture public surface.\n\n/// Exposes the fixture value.\npub mod value;\n"),
    )
    .expect("library");
    fs::write(root.join("src/value.rs"), format!("{HEADER}{DOCUMENTED_VALUE}")).expect("documented source");
    directory
}

/// Runs the public executable without inheriting formatter overrides.
fn run(root: &Path, operation: &str, json: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rs-infra-style"));
    command.arg("--project").arg(root);
    if json {
        command.args(["--format", "json"]);
    }
    command
        .arg(operation)
        .env_remove("RS_INFRA_STYLE_TOOLCHAIN")
        .env_remove("RS_INFRA_STYLE_RUSTFMT_CONFIG")
        .output()
        .expect("run unified gate")
}

#[test]
fn test_formatted_undocumented_function_fails_analyzer_without_writing_source() {
    let directory = project();
    let path = directory.path().join("src/value.rs");
    let source = format!("{HEADER}pub fn value() -> u8 {{\n    1\n}}\n");
    fs::write(&path, &source).expect("undocumented source");
    let output = run(directory.path(), "check", true);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let diagnostics: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).expect("legacy JSON array");
    assert!(
        diagnostics.iter().any(|item| item["path"] == "src/value.rs"
            && (item["code"].as_str().unwrap().contains("DOC-002")
                || item["message"].as_str().unwrap().contains("Rustdoc"))),
        "{diagnostics:?}"
    );
    for item in &diagnostics {
        assert_eq!(item["schema_version"], 1);
        assert_eq!(item["severity"], "error");
        for field in ["code", "path", "message"] {
            assert!(item[field].is_string(), "{item}");
        }
        assert!(item["line"].is_number(), "{item}");
    }
    assert_eq!(fs::read_to_string(path).expect("source after check"), source);
}

#[test]
fn test_semantic_review_signals_do_not_fail_the_gate() {
    let directory = project();
    fs::write(
        directory.path().join("src/value.rs"),
        format!("{HEADER}/// Represents one fixture identity.\npub struct Value;\n"),
    )
    .expect("type lacking an optional core-type example");
    let files = directory.path().join("analysis-files.json");
    fs::write(&files, r#"["src/lib.rs", "src/value.rs"]"#).expect("analysis manifest");
    let analysis = Command::new(env!("CARGO_BIN_EXE_rs-infra-style"))
        .arg("--project")
        .arg(directory.path())
        .args(["--format", "json", "analyze", "--files-from"])
        .arg(&files)
        .env_remove("RS_INFRA_STYLE_TOOLCHAIN")
        .output()
        .expect("rich analysis report");
    assert!(
        analysis.status.success(),
        "{}",
        String::from_utf8_lossy(&analysis.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&analysis.stdout).expect("analysis JSON object");
    assert!(
        report["diagnostics"]
            .as_array()
            .expect("analysis diagnostics")
            .iter()
            .any(|item| item["severity"] == "info" && item["check_id"] == "public-type-examples")
    );
    let output = run(directory.path(), "check", true);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let diagnostics: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).expect("check JSON diagnostics");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn test_fix_formats_but_does_not_repair_documentation() {
    let directory = project();
    let path = directory.path().join("src/value.rs");
    fs::write(&path, format!("{HEADER}pub fn value( )-> u8 {{1}}\n")).expect("unformatted source");
    let output = run(directory.path(), "fix", true);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let diagnostics: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).expect("fix error JSON diagnostics");
    assert!(!diagnostics.is_empty());
    assert_eq!(
        fs::read_to_string(path).expect("formatted source"),
        format!("{HEADER}pub fn value() -> u8 {{\n    1\n}}\n")
    );
}

#[test]
fn test_missing_tools_and_parse_failures_are_incomplete_not_clean() {
    for malformed in [false, true] {
        let directory = project();
        if malformed {
            fs::write(
                directory.path().join("src/value.rs"),
                format!("{HEADER}pub fn value(\n"),
            )
            .expect("malformed fixture");
        } else {
            fs::remove_file(directory.path().join(".infra/tools/defaults.toml")).expect("missing tool defaults");
        }
        let output = run(directory.path(), "check", true);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let diagnostics: Vec<serde_json::Value> =
            serde_json::from_slice(&output.stdout).expect("incomplete check JSON diagnostics");
        assert!(!diagnostics.is_empty());
    }
}

#[test]
fn test_format_failure_preserves_json_array_and_source_bytes() {
    let directory = project();
    let path = directory.path().join("src/value.rs");
    let source = format!(
        "{HEADER}{}",
        DOCUMENTED_VALUE.replace("pub fn value() -> u8 {", "pub fn value( )-> u8 {")
    );
    fs::write(&path, &source).expect("format-only issue");
    let output = run(directory.path(), "check", true);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let diagnostics: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).expect("formatter output must not corrupt diagnostic JSON");
    assert!(diagnostics.iter().any(|item| item["code"] == "FMT"), "{diagnostics:?}");
    assert_eq!(fs::read_to_string(path).expect("source after check"), source);
}
