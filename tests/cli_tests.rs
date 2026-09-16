// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::fs;
use std::process::Command;

use tempfile::tempdir;

#[test]
fn fix_cli_forwards_explicit_style_directories() {
    let directory = tempdir().expect("temporary project");
    fs::create_dir_all(directory.path().join("src")).expect("source directory");
    fs::create_dir_all(directory.path().join("custom-src")).expect("custom source directory");
    fs::create_dir_all(directory.path().join("custom-tests")).expect("custom test directory");
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"cli-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("manifest");
    fs::write(directory.path().join("src/lib.rs"), "pub fn value() -> i32 { 1 }\n").expect("crate root");
    fs::write(directory.path().join("custom-src/wrong.rs"), "use std::*;\n").expect("custom source");

    let result = Command::new(env!("CARGO_BIN_EXE_rs-infra-style"))
        .args([
            "--project",
            directory.path().to_str().expect("project path"),
            "--source-dir",
            "custom-src",
            "--test-dir",
            "custom-tests",
            "fix",
        ])
        .output()
        .expect("run style CLI");

    assert!(!result.status.success(), "custom source violation must fail");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("style checks still report"), "{stderr}");
}

/// Creates a dependency-free project for formatter subprocess tests.
fn formatter_project() -> tempfile::TempDir {
    let directory = tempdir().expect("temporary project");
    fs::create_dir(directory.path().join("src")).expect("source directory");
    fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"formatter-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("manifest");
    fs::write(
        directory.path().join("src/main.rs"),
        "fn main() {\n    let _value = 1;\n}\n",
    )
    .expect("source");
    directory
}

#[test]
fn test_formatter_config_controls_fix_and_check() {
    let project = formatter_project();
    let config = project.path().join("shared config.toml");
    fs::write(&config, "tab_spaces = 2\n").expect("formatter configuration");
    for (operation, success) in [("check", false), ("fix", true), ("check", true)] {
        let result = Command::new(env!("CARGO_BIN_EXE_rs-infra-style"))
            .arg("--project")
            .arg(project.path())
            .arg(operation)
            .env_remove("RS_INFRA_STYLE_TOOLCHAIN")
            .env("RS_INFRA_STYLE_RUSTFMT_CONFIG", "shared config.toml")
            .output()
            .expect("run formatter");
        assert_eq!(
            result.status.success(),
            success,
            "{operation}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert_eq!(
        fs::read_to_string(project.path().join("src/main.rs")).expect("formatted source"),
        "fn main() {\n  let _value = 1;\n}\n"
    );
}

#[cfg(unix)]
#[test]
fn test_formatter_forwards_toolchain_and_config_for_both_modes() {
    use std::os::unix::fs::PermissionsExt;

    let project = formatter_project();
    let bin = project.path().join("bin");
    fs::create_dir(&bin).expect("fake cargo directory");
    let cargo = bin.join("cargo");
    // Stop at the formatter boundary, before style's separate metadata call.
    fs::write(
        &cargo,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FMT_ARGUMENTS\"\nexit 42\n",
    )
    .expect("fake cargo");
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).expect("executable cargo");
    let output_path = project.path().join("arguments");
    for (toolchain, config) in [(false, false), (true, false), (false, true), (true, true)] {
        for operation in ["check", "fix"] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_rs-infra-style"));
            command
                .arg("--project")
                .arg(project.path())
                .arg(operation)
                .env("PATH", &bin)
                .env("FMT_ARGUMENTS", &output_path)
                .env_remove("RS_INFRA_STYLE_TOOLCHAIN")
                .env_remove("RS_INFRA_STYLE_RUSTFMT_CONFIG");
            let mut expected = Vec::new();
            if toolchain {
                command.env("RS_INFRA_STYLE_TOOLCHAIN", "nightly-2026-06-05");
                expected.push("+nightly-2026-06-05");
            }
            if config {
                command.env("RS_INFRA_STYLE_RUSTFMT_CONFIG", "shared config.toml");
            }
            expected.extend(["fmt", "--all"]);
            if operation == "check" || config {
                expected.push("--");
            }
            if operation == "check" {
                expected.push("--check");
            }
            if config {
                expected.extend(["--config-path", "shared config.toml"]);
            }
            let result = command.output().expect("run formatter");
            assert!(!result.status.success(), "formatter failure must propagate");
            assert_eq!(
                fs::read_to_string(&output_path).expect("recorded arguments"),
                format!("{}\n", expected.join("\n"))
            );
        }
    }
}

#[test]
fn test_formatter_dry_run_shows_contract_without_running_cargo() {
    let project = formatter_project();
    let result = Command::new(env!("CARGO_BIN_EXE_rs-infra-style"))
        .arg("--project")
        .arg(project.path())
        .args(["fix", "--dry-run"])
        .env("PATH", project.path())
        .env("RS_INFRA_STYLE_TOOLCHAIN", "nightly-2026-06-05")
        .env("RS_INFRA_STYLE_RUSTFMT_CONFIG", "shared config.toml")
        .output()
        .expect("dry run");
    assert!(result.status.success(), "dry run must not invoke cargo");
    assert_eq!(
        String::from_utf8(result.stdout).expect("command text"),
        "cargo +nightly-2026-06-05 fmt --all -- --config-path \"shared config.toml\"\nRust style operation completed successfully.\n"
    );
}
