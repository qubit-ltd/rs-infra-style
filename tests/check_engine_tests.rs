// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared mechanical gate contracts for the public library wrappers.

use std::fs;
use std::path::Path;
use std::process::Command;

use qubit_infra_style::check;

const HEADER: &str = "// =============================================================================\n//    Copyright (c) 2025 - 2026 Haixing Hu.\n//\n//    SPDX-License-Identifier: Apache-2.0\n//\n//    Licensed under the Apache License, Version 2.0.\n// =============================================================================\n";

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join(".infra/style")).unwrap();
    fs::create_dir_all(root.path().join(".infra/tools")).unwrap();
    fs::write(root.path().join(".infra/style/rustfmt.toml"), "edition = \"2024\"\n").unwrap();
    fs::write(
        root.path().join(".infra/tools/defaults.toml"),
        "nightly_toolchain = \"1.94.0\"\n",
    )
    .unwrap();
    root
}

fn put(root: &Path, path: &str, body: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("{HEADER}{body}")).unwrap();
}

#[test]
fn test_check_runs_analyzer_on_nondefault_members_and_fixtures() {
    let root = project();
    Command::new("git")
        .args(["init", "-q"])
        .arg(root.path())
        .status()
        .unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"one\",\"two\"]\ndefault-members=[\"one\"]\nresolver=\"3\"\n",
    )
    .unwrap();
    for name in ["one", "two"] {
        fs::create_dir_all(root.path().join(name)).unwrap();
        fs::write(
            root.path().join(name).join("Cargo.toml"),
            format!("[package]\nname=\"{name}\"\nversion=\"0.1.0\"\nedition=\"2024\"\n"),
        )
        .unwrap();
    }
    put(root.path(), "one/src/lib.rs", "//! Primary member.\n");
    put(root.path(), "two/src/lib.rs", "pub fn undocumented() {}\n");
    put(root.path(), "fixtures/orphan.rs", "use std::*;\n");
    fs::write(root.path().join(".gitignore"), "ignored/\n").unwrap();
    put(root.path(), "ignored/broken.rs", "fn { invalid\n");
    let findings = check(root.path(), None, None).unwrap();
    assert!(
        findings
            .iter()
            .any(|finding| finding.path == "two/src/lib.rs" && finding.code.contains("DOC-002")),
        "{findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.path == "fixtures/orphan.rs" && finding.code == "STYLE004"),
        "{findings:?}"
    );
    assert!(!findings.iter().any(|finding| finding.path.starts_with("ignored/")));
}

#[test]
fn test_check_does_not_promote_semantic_aggregation_signal_to_error() {
    let root = project();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"aggregate\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .unwrap();
    put(
        root.path(),
        "src/lib.rs",
        "/// Returns a stable number.\n///\n/// # Returns\n///\n/// Returns one.\npub fn value() -> u8 { 1 }\n",
    );
    let findings = check(root.path(), None, None).unwrap();
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn test_check_reports_a_shared_import_violation_once() {
    let root = project();
    put(root.path(), "src/example.rs", "use std::*;\n");
    let findings = check(root.path(), None, None).unwrap();
    assert_eq!(
        findings.iter().filter(|finding| finding.code == "STYLE004").count(),
        1,
        "{findings:?}"
    );
}

#[test]
fn test_deleted_tracked_source_is_not_a_missing_input() {
    let root = project();
    Command::new("git")
        .args(["init", "-q"])
        .arg(root.path())
        .status()
        .unwrap();
    put(root.path(), "src/deleted.rs", "");
    put(root.path(), "src/lib.rs", "//! Existing library.\n");
    Command::new("git")
        .args(["-C"])
        .arg(root.path())
        .args(["add", "."])
        .status()
        .unwrap();
    fs::remove_file(root.path().join("src/deleted.rs")).unwrap();
    assert!(check(root.path(), None, None).unwrap().is_empty());
}

#[test]
fn test_test_helper_without_a_concrete_test_does_not_require_test_filename() {
    let root = project();
    put(root.path(), "tests/helper.rs", "pub fn helper() {}\n");
    assert!(check(root.path(), None, None).unwrap().is_empty());
}

#[test]
fn test_explicit_import_exception_preserves_grouping_exception() {
    let root = project();
    put(root.path(), "src/lib.rs", "use external::Thing;\nuse std::fs;\n");
    fs::write(root.path().join(".infra/style/exceptions.toml"), "format = 1\n[[exceptions]]\nrule = \"explicit-imports\"\npath = \"src/lib.rs\"\nreason = \"Import layout retained for compatibility with generated bindings\"\n").unwrap();
    assert!(check(root.path(), None, None).unwrap().is_empty());
}

#[test]
fn test_explicit_scope_preserves_sibling_auxiliary_trees() {
    let root = project();
    put(root.path(), "custom/code/lib.rs", "//! Selected root.\n");
    put(root.path(), "custom/benches/bench.rs", "use std::*;\n");
    put(root.path(), "other/src/ignored.rs", "use std::*;\n");
    let findings = check(
        root.path(),
        Some(Path::new("custom/code")),
        Some(Path::new("custom/tests")),
    )
    .unwrap();
    assert!(findings.iter().any(|finding| finding.path == "custom/benches/bench.rs"));
    assert!(!findings.iter().any(|finding| finding.path == "other/src/ignored.rs"));
}

#[test]
fn test_orphan_fixture_formatting_is_readonly_and_fix_formats_it() {
    let root = project();
    Command::new("git")
        .args(["init", "-q"])
        .arg(root.path())
        .status()
        .unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"all-files\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .unwrap();
    put(root.path(), "src/lib.rs", "//! Existing library.\n");
    put(root.path(), "fixtures/orphan.rs", "fn helper(){let _value=1;}\n");
    let source = fs::read(root.path().join("fixtures/orphan.rs")).unwrap();
    let findings = qubit_infra_style::check_project(root.path(), None, None).unwrap();
    assert!(
        findings
            .iter()
            .any(|finding| finding.code == "FMT" && finding.path == "fixtures/orphan.rs"),
        "{findings:?}"
    );
    assert_eq!(fs::read(root.path().join("fixtures/orphan.rs")).unwrap(), source);
    qubit_infra_style::fix(root.path(), false).unwrap();
    assert!(
        qubit_infra_style::check_project(root.path(), None, None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn test_nongit_standalone_scope_ignores_nested_documentation_source_tree() {
    let root = project();
    put(root.path(), "src/lib.rs", "//! Existing library.\n");
    put(root.path(), "docs/src/snippet.rs", "use std::*;\n");
    let findings = check(root.path(), None, None).unwrap();
    assert!(!findings.iter().any(|finding| finding.path == "docs/src/snippet.rs"));
}

#[test]
fn test_opt_in_pairing_accepts_nested_parent_module_test() {
    let root = project();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"nested-pair\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .unwrap();
    put(root.path(), "src/lib.rs", "//! Existing library.\n");
    put(
        root.path(),
        "src/outer/top.rs",
        "/// Marks the parent module boundary.\npub fn marker() {}\n",
    );
    put(
        root.path(),
        "src/outer/top/internal/thing.rs",
        "/// Performs the nested operation.\npub fn thing() {}\n",
    );
    put(
        root.path(),
        "tests/top_tests.rs",
        "// Parent module tests are paired by their boundary.\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rs-infra-style"))
        .arg("--project")
        .arg(root.path())
        .args(["--format", "json", "check"])
        .env("STYLE_ENFORCE_SOURCE_TEST_PAIRS", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_nongit_package_scope_ignores_nested_documentation_source_tree() {
    let root = project();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"package-scope\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )
    .unwrap();
    put(root.path(), "src/lib.rs", "//! Existing library.\n");
    put(root.path(), "docs/src/snippet.rs", "fn { invalid\n");
    assert!(check(root.path(), None, None).unwrap().is_empty());
}

#[test]
fn test_nongit_custom_cargo_target_parent_is_checked() {
    for target in [
        "[lib]\npath = \"code/entry.rs\"\n",
        "[[bin]]\nname = \"custom\"\npath = \"code/entry.rs\"\n",
    ] {
        let root = project();
        fs::write(
            root.path().join("Cargo.toml"),
            format!("[package]\nname=\"custom-target\"\nversion=\"0.1.0\"\nedition=\"2024\"\n{target}"),
        )
        .unwrap();
        put(root.path(), "code/entry.rs", "use std::*;\n");
        put(root.path(), "code/helper.rs", "use std::*;\n");
        let findings = check(root.path(), None, None).unwrap();
        for path in ["code/entry.rs", "code/helper.rs"] {
            assert!(
                findings
                    .iter()
                    .any(|finding| finding.path == path && finding.code == "STYLE004"),
                "{target}: {findings:?}"
            );
        }
    }
}
