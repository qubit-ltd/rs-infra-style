// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! End-to-end contracts for read-only deterministic analysis.

use std::fs;
use std::path::Path;
use std::process::Command;

use qubit_infra_style::analysis::RULE_IDS;
use qubit_infra_style::analysis::analyze;
use qubit_infra_style::analysis::supported_standards_digest;

const HEADER: &str = "// =============================================================================\n//    Copyright (c) 2025 - 2026 Haixing Hu.\n//\n//    SPDX-License-Identifier: Apache-2.0\n//\n//    Licensed under the Apache License, Version 2.0.\n// =============================================================================\n";

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("src")).unwrap();
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
fn put(root: &Path, path: &str, text: &str) {
    fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
    fs::write(root.join(path), format!("{HEADER}{text}")).unwrap();
}

#[test]
fn test_complete_coverage_preserves_semantic_review_and_source_bytes() {
    let root = project();
    put(root.path(), "src/lib.rs", "//! Empty library.\n");
    let before = fs::read(root.path().join("src/lib.rs")).unwrap();
    let report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    assert_eq!(report.exit_code(), 0, "{:?}", report.blocked);
    assert_eq!(report.files, vec!["src/lib.rs"]);
    for rule in RULE_IDS {
        assert!(
            report
                .coverage
                .iter()
                .any(|coverage| coverage.path == "src/lib.rs" && coverage.rule_id == *rule)
        );
    }
    assert!(
        report
            .coverage
            .iter()
            .any(|coverage| coverage.rule_id == "DOC-002" && coverage.status == "needs_semantic_review")
    );
    assert_eq!(before, fs::read(root.path().join("src/lib.rs")).unwrap());
    assert_eq!(report.standards_digest, supported_standards_digest());
}

#[test]
fn test_headers_preserve_accepted_years_and_reject_placeholders() {
    let root = project();
    for year in ["2025 - 2026", "2025", "2030"] {
        fs::write(root.path().join("src/lib.rs"), HEADER.replace("2025 - 2026", year)).unwrap();
        let report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
        assert!(!report.diagnostics.iter().any(|d| d.rule_id == "DOC-001"), "{year}");
    }
    fs::write(
        root.path().join("src/lib.rs"),
        HEADER.replace("2025 - 2026", "2025 - {current_year}"),
    )
    .unwrap();
    assert!(
        analyze(root.path(), &["src/lib.rs".into()], None)
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "DOC-001")
    );
}

#[test]
fn test_parse_missing_and_symlink_inputs_fail_closed() {
    let root = project();
    put(root.path(), "src/lib.rs", "fn (");
    let report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    assert_eq!(report.exit_code(), 2);
    assert!(report.coverage.iter().all(|c| c.status == "blocked"));
    assert_eq!(
        analyze(root.path(), &["src/missing.rs".into()], None)
            .unwrap()
            .exit_code(),
        2
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("lib.rs", root.path().join("src/link.rs")).unwrap();
        assert_eq!(
            analyze(root.path(), &["src/link.rs".into()], None).unwrap().exit_code(),
            2
        );
    }
}

#[test]
fn test_standards_digest_matches_python_and_mismatch_blocks() {
    let root = project();
    put(root.path(), "src/lib.rs", "//! Empty.\n");
    let standard = Path::new(env!("CARGO_MANIFEST_DIR")).join("doc/style-standard");
    let report = analyze(root.path(), &["src/lib.rs".into()], Some(&standard)).unwrap();
    assert_eq!(report.standards_digest, report.supported_standards_digest);
    let python=Command::new("python3").args(["-c","import hashlib,json,pathlib,sys;p=pathlib.Path(sys.argv[1]);print(hashlib.sha256(json.dumps({x.relative_to(p).as_posix():hashlib.sha256(x.read_bytes()).hexdigest() for x in sorted(p.rglob('*')) if x.is_file()},sort_keys=True).encode()).hexdigest())"]).arg(&standard).output().unwrap();
    assert_eq!(
        String::from_utf8(python.stdout).unwrap().trim(),
        supported_standards_digest()
    );
    let wrong = tempfile::tempdir().unwrap();
    fs::write(wrong.path().join("rust-coding-standard.md"), "changed").unwrap();
    let report = analyze(root.path(), &["src/lib.rs".into()], Some(wrong.path())).unwrap();
    assert_eq!(report.exit_code(), 2);
    assert!(report.coverage.iter().all(|c| c.status == "blocked"));
}

#[test]
fn test_exact_source_list_includes_fuzz_and_nondefault_members_without_cargo_metadata() {
    let root = project();
    fs::write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"a\",\"b\"]\ndefault-members=[\"a\"]\n",
    )
    .unwrap();
    for path in ["a/src/lib.rs", "b/src/lib.rs", "fuzz/fuzz_targets/input.rs"] {
        put(root.path(), path, "use foo::*;\n");
    }
    let files = vec![
        "a/src/lib.rs".into(),
        "b/src/lib.rs".into(),
        "fuzz/fuzz_targets/input.rs".into(),
    ];
    let report = analyze(root.path(), &files, None).unwrap();
    for path in &files {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| &d.path == path && d.check_id == "single-object-import")
        );
    }
}

#[test]
fn test_context_changes_invalidate_input_digest() {
    let root = project();
    put(root.path(), "src/lib.rs", "mod thing;\n");
    put(root.path(), "src/thing.rs", "//! Thing.\n");
    let before = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    put(root.path(), "src/thing.rs", "//! Changed context.\n");
    let after = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    assert_ne!(before.input_digest, after.input_digest);
    assert!(after.context_files.iter().any(|file| file.path == "src/thing.rs"));
}

#[test]
fn test_unsupported_paths_are_rejected_and_generated_sources_are_excluded() {
    let root = project();
    for path in ["../outside.rs", "/absolute.rs", "src/../lib.rs", "src\\lib.rs"] {
        assert!(analyze(root.path(), &[path.into()], None).is_err());
    }
    put(root.path(), "generated/model.rs", "not Rust");
    let report = analyze(root.path(), &["generated/model.rs".into()], None).unwrap();
    assert_eq!(report.exit_code(), 0);
    assert_eq!(report.excluded.len(), 1);
    assert!(report.coverage.iter().all(|c| c.status == "not_applicable"));
}

#[test]
fn test_cli_accepts_options_after_subcommand_and_returns_structured_report() {
    let root = project();
    put(root.path(), "src/lib.rs", "//! Empty.\n");
    fs::write(root.path().join("files.json"), "[\"src/lib.rs\"]").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rs-infra-style"))
        .args(["analyze", "--project"])
        .arg(root.path())
        .args(["--files-from"])
        .arg(root.path().join("files.json"))
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["files"][0], "src/lib.rs");
}

#[test]
fn test_self_must_use_is_resolved_from_actual_local_definition() {
    let root = project();
    put(
        root.path(),
        "src/client.rs",
        "/// A client.\n#[must_use]\npub struct Client;\nimpl Client {\n/// Constructs a client.\n#[must_use]\nfn new() -> Self { Self }\n}\n",
    );
    let report = analyze(root.path(), &["src/client.rs".into()], None).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.check_id == "double-must-use-self" && d.severity == "error")
    );
}

#[test]
fn test_rustfmt_checks_import_order_without_rejecting_body_formatting() {
    let root = project();
    put(root.path(), "src/lib.rs", "use std::path::Path;\nuse std::fs;\n");
    let report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.check_id == "rustfmt-import-order" && d.severity == "error"),
        "{:?}",
        report.diagnostics
    );
    put(
        root.path(),
        "src/lib.rs",
        "/// Returns the answer.\nfn answer( ) ->i32{42}\n",
    );
    let report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    assert!(!report.diagnostics.iter().any(|d| d.check_id == "rustfmt-import-order"));
}

#[test]
fn test_missing_formatter_and_invalid_manifest_are_explicitly_blocked() {
    let root = project();
    put(root.path(), "src/lib.rs", "use std::fs;\n");
    fs::remove_file(root.path().join(".infra/style/rustfmt.toml")).unwrap();
    assert_eq!(
        analyze(root.path(), &["src/lib.rs".into()], None).unwrap().exit_code(),
        2
    );
    fs::write(root.path().join("Cargo.toml"), "invalid manifest = [").unwrap();
    let report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    assert!(
        report
            .blocked
            .iter()
            .any(|issue| issue.reason.contains("manifest cannot be parsed"))
    );
}

#[test]
fn test_custom_cargo_targets_override_directory_role() {
    let root = project();
    fs::write(root.path().join("Cargo.toml"),"[package]\nname=\"custom\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\npath=\"code/api.rs\"\n[[test]]\nname=\"custom\"\npath=\"checks/public_tests.rs\"\n").unwrap();
    put(root.path(), "code/api.rs", "fn undocumented() {}\n");
    put(root.path(), "checks/public_tests.rs", "include!(\"body.rs\");\n");
    let report = analyze(
        root.path(),
        &["code/api.rs".into(), "checks/public_tests.rs".into()],
        None,
    )
    .unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "code/api.rs" && d.check_id == "rustdoc-presence")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "checks/public_tests.rs" && d.check_id == "test-source-redirection")
    );
    assert!(
        report
            .coverage
            .iter()
            .any(|c| c.path == "checks/public_tests.rs" && c.rule_id == "TEST-004" && c.status == "fail")
    );
}

#[test]
fn test_nested_namespaces_do_not_guess_must_use_ownership() {
    let root = project();
    put(
        root.path(),
        "src/lib.rs",
        "mod marked { #[must_use] pub struct A; } struct B; use B as A; impl A { #[must_use] fn f()->Self { Self } }\n",
    );
    let report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.check_id == "double-must-use-self" && d.severity == "error")
    );
}

#[test]
fn test_mechanical_subchecks_have_explicit_pass_and_unknown_coverage() {
    let root = project();
    put(root.path(), "tests/safe_tests.rs", "#[test]\nfn test_ok() {}\n");
    let report = analyze(root.path(), &["tests/safe_tests.rs".into()], None).unwrap();
    assert!(
        report
            .coverage
            .iter()
            .any(|c| c.check_id == "single-object-import" && c.status == "pass")
    );
    assert!(
        report
            .coverage
            .iter()
            .any(|c| c.check_id == "test-source-redirection" && c.status == "pass")
    );
    assert!(
        report
            .coverage
            .iter()
            .any(|c| c.rule_id == "TEST-001" && c.status == "needs_semantic_review")
    );
}

#[test]
fn test_custom_test_modules_inherit_target_role() {
    let root = project();
    fs::write(root.path().join("Cargo.toml"),"[package]\nname=\"custom\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[[test]]\nname=\"custom\"\npath=\"checks/root_tests.rs\"\n").unwrap();
    put(root.path(), "checks/root_tests.rs", "mod helper_tests;\n");
    put(root.path(), "checks/helper_tests.rs", "include!(\"body.rs\");\n");
    let report = analyze(root.path(), &["checks/helper_tests.rs".into()], None).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.check_id == "test-source-redirection" && d.severity == "error")
    );
    assert!(
        report
            .coverage
            .iter()
            .any(|c| c.rule_id == "TEST-004" && c.check_id == "rule-coverage" && c.status == "fail")
    );
}

#[test]
fn test_support_uncertainty_does_not_certify_the_whole_rule() {
    let root = project();
    put(root.path(), "tests/common/shared.rs", "include!(\"body.rs\");\n");
    let report = analyze(root.path(), &["tests/common/shared.rs".into()], None).unwrap();
    for rule in ["TEST-003", "TEST-004"] {
        assert!(
            report
                .coverage
                .iter()
                .any(|c| c.rule_id == rule && c.check_id == "rule-coverage" && c.status == "needs_semantic_review")
        );
    }
}

#[test]
fn test_reasoned_header_policy_preserves_years_and_rejects_invalid_config() {
    let root = project();
    let policy = "format=1\n[header]\nowner=\"Example Team\"\nminimum_year=2020\nreason=\"Project-owned source uses its established copyright.\"\n";
    fs::write(root.path().join(".infra/style/analysis.toml"), policy).unwrap();
    for year in ["2020 - 2026", "2021"] {
        fs::write(
            root.path().join("src/lib.rs"),
            HEADER
                .replace("Haixing Hu", "Example Team")
                .replace("2025 - 2026", year),
        )
        .unwrap();
        let report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
        assert!(!report.diagnostics.iter().any(|d| d.rule_id == "DOC-001"));
        assert!(
            report
                .coverage
                .iter()
                .any(|c| c.check_id == "header-template" && c.evidence.contains("Project-owned source"))
        );
        assert!(
            report
                .context_files
                .iter()
                .any(|c| c.path == ".infra/style/analysis.toml")
        );
    }
    fs::write(
        root.path().join(".infra/style/analysis.toml"),
        policy.replace(
            "reason=\"Project-owned source uses its established copyright.\"",
            "reason=\"\"",
        ),
    )
    .unwrap();
    assert_eq!(
        analyze(root.path(), &["src/lib.rs".into()], None).unwrap().exit_code(),
        2
    );
}

#[cfg(unix)]
#[test]
fn test_ignored_symlink_exception_cannot_suppress_findings() {
    let root = project();
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(root.path())
        .status()
        .unwrap();
    fs::write(root.path().join(".gitignore"), ".infra/\n").unwrap();
    put(root.path(), "src/lib.rs", "use foo::*;\n");
    let outside = tempfile::tempdir().unwrap();
    let outside_config = outside.path().join("exceptions.toml");
    fs::write(&outside_config,"format=1\n[[exceptions]]\nrule=\"explicit-imports\"\npath=\"src/lib.rs\"\nreason=\"External override must not be followed\"\n").unwrap();
    std::os::unix::fs::symlink(&outside_config, root.path().join(".infra/style/exceptions.toml")).unwrap();
    let report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    assert_eq!(report.exit_code(), 2);
    assert!(
        report
            .blocked
            .iter()
            .any(|issue| issue.reason.contains("symbolic-link"))
    );
    assert!(report.coverage.iter().all(|coverage| coverage.status == "blocked"));
}

#[test]
fn test_external_cfg_test_modules_do_not_receive_production_doc_errors() {
    let root = project();
    put(root.path(), "src/lib.rs", "#[cfg(test)] mod private_tests;\n");
    put(
        root.path(),
        "src/private_tests.rs",
        "struct LocalHelper;\n#[test] fn test_local() {}\n",
    );
    let report = analyze(root.path(), &["src/private_tests.rs".into()], None).unwrap();
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "DOC-002" && d.severity == "error")
    );
    assert!(
        report
            .coverage
            .iter()
            .any(|c| c.rule_id == "TEST-004" && c.check_id == "rule-coverage" && c.status != "not_applicable")
    );
}

#[cfg(unix)]
#[test]
fn test_ignored_symlink_formatter_config_is_blocked_before_rustfmt() {
    let root = project();
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(root.path())
        .status()
        .unwrap();
    fs::write(root.path().join(".gitignore"), ".infra/\n").unwrap();
    put(root.path(), "src/lib.rs", "use std::fs;\n");
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("rustfmt.toml"), "edition=\"2024\"\n").unwrap();
    fs::remove_file(root.path().join(".infra/style/rustfmt.toml")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("rustfmt.toml"),
        root.path().join(".infra/style/rustfmt.toml"),
    )
    .unwrap();
    let report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    assert_eq!(report.exit_code(), 2);
    assert!(
        report
            .blocked
            .iter()
            .any(|issue| issue.reason.contains("cannot safely read rustfmt config"))
    );
}

#[test]
fn test_optional_repository_readme_scope_is_separate_and_digest_bound() {
    let root = project();
    put(root.path(), "src/lib.rs", "//! Library.\n");
    fs::write(root.path().join("Cargo.toml"),"[package]\nname=\"demo\"\nversion=\"0.1.0\"\nrust-version=\"1.94\"\nrepository=\"https://github.com/example/demo\"\n").unwrap();
    let mut report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    let source_digest = report.input_digest.clone();
    assert!(report.repository_diagnostics.is_empty());
    assert!(report.repository_coverage.is_empty());
    qubit_infra_style::analysis::analyze_repository_readme(root.path(), &mut report).unwrap();
    assert_eq!(report.exit_code(), 1);
    assert_eq!(report.repository_diagnostics.len(), 2);
    assert!(report.repository_diagnostics.iter().all(|d| d.rule_id == "README-001"));
    assert!(!report.diagnostics.iter().any(|d| d.rule_id == "README-001"));
    assert_ne!(source_digest, report.input_digest);
    fs::write(
        root.path().join("README.md"),
        "# Demo\n\n## Testing\n## License\n## Contributing\n## Author\n",
    )
    .unwrap();
    let mut second = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    qubit_infra_style::analysis::analyze_repository_readme(root.path(), &mut second).unwrap();
    assert_ne!(report.input_digest, second.input_digest);
    assert!(
        second
            .repository_diagnostics
            .iter()
            .any(|d| d.check_id == "opening-badge-block")
    );
}

#[test]
fn test_unresolved_workspace_edition_is_not_guessed() {
    let root = project();
    put(root.path(), "src/lib.rs", "//! Library.\n");
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"demo\"\nversion=\"0.1.0\"\nedition.workspace=true\n",
    )
    .unwrap();
    let report = analyze(root.path(), &["src/lib.rs".into()], None).unwrap();
    assert_eq!(report.exit_code(), 2);
    assert!(
        report
            .blocked
            .iter()
            .any(|issue| issue.reason.contains("edition cannot be resolved"))
    );
}

#[test]
fn test_nested_main_filename_requires_a_discovered_cargo_entry() {
    let root = project();
    let manifest = "[package]\nname=\"demo\"\nversion=\"0.1.0\"\nedition=\"2024\"\n";
    fs::write(root.path().join("Cargo.toml"), manifest).unwrap();
    put(root.path(), "tests/a/b/main.rs", "#[test] fn test_nested() {}\n");
    let unknown = analyze(root.path(), &["tests/a/b/main.rs".into()], None).unwrap();
    assert!(
        unknown
            .coverage
            .iter()
            .any(|c| c.rule_id == "TEST-003" && c.check_id == "rule-coverage" && c.status == "needs_semantic_review")
    );
    fs::write(
        root.path().join("Cargo.toml"),
        format!("{manifest}\n[[test]]\nname=\"nested\"\npath=\"tests/a/b/main.rs\"\n"),
    )
    .unwrap();
    let registered = analyze(root.path(), &["tests/a/b/main.rs".into()], None).unwrap();
    assert!(
        registered
            .coverage
            .iter()
            .any(|c| c.rule_id == "TEST-003" && c.check_id == "rule-coverage" && c.status == "pass")
    );
}

#[test]
fn test_disabled_cargo_autotests_does_not_certify_main_entry() {
    let root = project();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"demo\"\nversion=\"0.1.0\"\nedition=\"2024\"\nautotests=false\n",
    )
    .unwrap();
    put(root.path(), "tests/a/main.rs", "#[test] fn test_case() {}\n");
    let report = analyze(root.path(), &["tests/a/main.rs".into()], None).unwrap();
    assert!(
        report
            .coverage
            .iter()
            .any(|c| c.rule_id == "TEST-003" && c.check_id == "rule-coverage" && c.status == "needs_semantic_review")
    );
}

#[test]
fn test_ignored_cargo_manifest_bytes_bind_source_roles_and_input_digest() {
    let root = project();
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(root.path())
        .status()
        .unwrap();
    fs::write(root.path().join(".gitignore"), "Cargo.toml\n").unwrap();
    put(root.path(), "code/api.rs", "fn undocumented() {}\n");
    let manifest = "[package]\nname=\"demo\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\npath=\"code/api.rs\"\n";
    fs::write(root.path().join("Cargo.toml"), manifest).unwrap();
    let before = analyze(root.path(), &["code/api.rs".into()], None).unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        manifest.replace("code/api.rs", "other.rs"),
    )
    .unwrap();
    let after = analyze(root.path(), &["code/api.rs".into()], None).unwrap();
    assert_ne!(before.input_digest, after.input_digest);
    assert!(before.context_files.iter().any(|file| file.path == "Cargo.toml"));
    assert!(
        before
            .coverage
            .iter()
            .any(|coverage| coverage.check_id == "rustdoc-presence" && coverage.status == "fail")
    );
    assert!(
        after
            .coverage
            .iter()
            .any(|coverage| coverage.check_id == "rustdoc-presence" && coverage.status == "not_applicable")
    );
}

#[test]
fn test_formatter_environment_override_is_part_of_input_identity() {
    let root = project();
    put(root.path(), "src/lib.rs", "//! Library.\n");
    fs::write(root.path().join("files.json"), "[\"src/lib.rs\"]").unwrap();
    let run = |override_value: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_rs-infra-style"));
        command
            .args(["analyze", "--project"])
            .arg(root.path())
            .arg("--files-from")
            .arg(root.path().join("files.json"))
            .args(["--format", "json"]);
        command.env_remove("RS_INFRA_STYLE_TOOLCHAIN");
        if let Some(value) = override_value {
            command.env("RS_INFRA_STYLE_TOOLCHAIN", value);
        }
        let output = command.output().unwrap();
        assert!(output.status.success());
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let default = run(None);
    let explicit = run(Some("1.94.0"));
    assert_ne!(default["input_digest"], explicit["input_digest"]);
}

#[test]
fn test_fuzz_binary_targets_do_not_receive_production_doc_checks() {
    let root = project();
    for (package, source, metadata) in [
        ("fuzz", "fuzz_targets/input.rs", ""),
        (
            "property-harness",
            "src/main.rs",
            "[package.metadata]\ncargo-fuzz = true\n",
        ),
    ] {
        put(
            root.path(),
            &format!("{package}/{source}"),
            "//! Fuzz harness.\nenum Input { Value(u8) }\n/// Checks input.\nfn check<T>(value: T) {}\n",
        );
        fs::write(root.path().join(format!("{package}/Cargo.toml")),
            format!("[package]\nname = \"harness\"\nversion = \"0.1.0\"\n{metadata}\n[[bin]]\nname = \"input\"\npath = \"{source}\"\n")).unwrap();
        let report = analyze(root.path(), &[format!("{package}/{source}")], None).unwrap();
        for check in ["rustdoc-presence", "required-documentation-sections"] {
            assert!(
                !report.diagnostics.iter().any(|d| d.check_id == check),
                "{package}: {check}"
            );
            assert!(
                report
                    .coverage
                    .iter()
                    .any(|c| c.check_id == check && c.status == "not_applicable")
            );
        }
    }
}

#[test]
fn test_production_binary_keeps_documentation_checks() {
    let root = project();
    put(
        root.path(),
        "code/main.rs",
        "//! Application.\nenum Input { Value(u8) }\n/// Checks input.\nfn check(value: u8) {}\n",
    );
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n[[bin]]\nname = \"app\"\npath = \"code/main.rs\"\n",
    )
    .unwrap();
    let report = analyze(root.path(), &["code/main.rs".into()], None).unwrap();
    for check in ["rustdoc-presence", "required-documentation-sections"] {
        assert!(report.diagnostics.iter().any(|d| d.check_id == check), "{check}");
    }
}
