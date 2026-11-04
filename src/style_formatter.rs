// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Read-only and writing formatter commands shared by the mechanical gate.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;

/// Resolves the required formatter configuration installed in a project.
///
/// Returns an absolute path so Cargo can run from the project directory even
/// when `project` was supplied as a relative path. Missing configuration is an
/// error because the bootstrap updater owns this shared file.
///
/// # Parameters
///
/// * `project` - Project root used to resolve sources and tooling.
pub(crate) fn project_rustfmt_config(project: &Path) -> Result<PathBuf> {
    let path = project.join(".infra/style/rustfmt.toml");
    if !path.is_file() {
        bail!(
            "missing {}: run ./update-infra.sh to install the shared rustfmt configuration",
            path.display()
        );
    }
    path.canonicalize()
        .with_context(|| format!("failed to resolve formatter configuration {}", path.display()))
}

/// Resolves the rustfmt toolchain from the environment or installed defaults.
///
/// A nonempty environment override bypasses the shared project defaults.
/// Otherwise `.infra/tools/defaults.toml` is required; malformed or missing
/// configuration is reported instead of silently selecting Cargo's default.
///
/// # Parameters
///
/// * `project` - Project root used to resolve sources and tooling.
pub(crate) fn project_toolchain(project: &Path) -> Result<Option<std::ffi::OsString>> {
    if let Some(toolchain) = std::env::var_os("RS_INFRA_STYLE_TOOLCHAIN").filter(|value| !value.is_empty()) {
        return Ok(Some(toolchain));
    }

    let defaults_path = project.join(".infra/tools/defaults.toml");
    if !defaults_path.exists() {
        bail!(
            "missing {}; run ./update-infra.sh to install shared tool defaults",
            defaults_path.display()
        );
    }
    let contents =
        fs::read_to_string(&defaults_path).with_context(|| format!("failed to read {}", defaults_path.display()))?;
    let defaults: toml::Value =
        toml::from_str(&contents).with_context(|| format!("failed to parse {}", defaults_path.display()))?;
    let toolchain = defaults
        .get("nightly_toolchain")
        .and_then(toml::Value::as_str)
        .context("missing nightly_toolchain")?;
    if toolchain.is_empty() || toolchain.starts_with(['+', '-']) || toolchain.chars().any(char::is_whitespace) {
        bail!(
            "nightly_toolchain in {} must be a nonempty toolchain name without whitespace",
            defaults_path.display()
        );
    }
    Ok(Some(toolchain.into()))
}

/// Builds a formatter command using the project's installed configuration.
///
/// `check_only` selects validation instead of rewriting. `config` is an
/// absolute path obtained from the project before Cargo changes directory.
///
/// # Parameters
///
/// * `project` - Project root containing the Cargo manifest.
/// * `check_only` - Selects read-only validation.
/// * `config` - Absolute shared rustfmt configuration path.
/// * `toolchain` - Configured rustfmt toolchain.
pub(crate) fn cargo_fmt_command(
    project: &Path,
    check_only: bool,
    config: &Path,
    toolchain: Option<&std::ffi::OsStr>,
) -> Command {
    let mut command = Command::new("cargo");
    if let Some(toolchain) = toolchain {
        let mut argument = std::ffi::OsString::from("+");
        argument.push(toolchain);
        command.arg(argument);
    }
    command.args(["fmt", "--manifest-path"]);
    command.arg(project.join("Cargo.toml"));
    command.arg("--");
    if check_only {
        command.arg("--check");
    }
    command.arg("--config-path").arg(config);
    command
}

/// Builds the legacy second formatter invocation for a standalone fuzz crate.
///
/// # Parameters
///
/// * `manifest` - Standalone fuzz crate manifest.
/// * `check_only` - Selects read-only validation.
/// * `config` - Absolute shared rustfmt configuration path.
/// * `toolchain` - Configured rustfmt toolchain.
pub(crate) fn cargo_fuzz_fmt_command(
    manifest: &Path,
    check_only: bool,
    config: &Path,
    toolchain: Option<&std::ffi::OsStr>,
) -> Command {
    let mut command = Command::new("cargo");
    if let Some(toolchain) = toolchain {
        let mut argument = std::ffi::OsString::from("+");
        argument.push(toolchain);
        command.arg(argument);
    }
    command.args(["fmt", "--manifest-path"]);
    command.arg(manifest);
    command.arg("--");
    if check_only {
        command.arg("--check");
    }
    command.arg("--config-path").arg(config);
    command
}

/// Describes a formatter command for dry runs and failure diagnostics.
///
/// Arguments containing whitespace are quoted for readability; the returned
/// text is never passed to a shell.
///
/// # Parameters
///
/// * `command` - Formatter command to describe without executing a shell.
pub(crate) fn cargo_fmt_description(command: &Command) -> String {
    let mut description = String::from("cargo");
    for argument in command.get_args() {
        let argument = argument.to_string_lossy();
        description.push(' ');
        if argument.chars().any(char::is_whitespace) {
            description.push_str(&format!("{argument:?}"));
        } else {
            description.push_str(&argument);
        }
    }
    description
}
