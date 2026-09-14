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
    fs::write(
        directory.path().join("src/lib.rs"),
        "pub fn value() -> i32 { 1 }\n",
    )
    .expect("crate root");
    fs::write(
        directory.path().join("custom-src/wrong.rs"),
        "use std::*;\n",
    )
    .expect("custom source");

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

    assert!(
        !result.status.success(),
        "custom source violation must fail"
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("style checks still report"), "{stderr}");
}
