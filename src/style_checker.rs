// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fixed first-phase Rust style checks used by Qubit Rust projects.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;
use syn::Item;
use syn::Visibility;
use syn::parse_file;
use walkdir::WalkDir;

use crate::Diagnostic;
use crate::style_exceptions::ExceptionConfig;

/// Cargo metadata needed to locate the packages checked by this crate.
#[derive(Debug, Deserialize)]
struct CargoMetadata {
    /// Package records returned by Cargo.
    packages: Vec<CargoPackage>,
    /// Workspace members selected by Cargo as default targets.
    #[serde(default)]
    workspace_default_members: Vec<String>,
    /// All workspace members reported by Cargo.
    #[serde(default)]
    workspace_members: Vec<String>,
}

/// Minimal Cargo package metadata used to derive source roots.
#[derive(Debug, Deserialize)]
struct CargoPackage {
    /// Stable package identifier used by workspace membership lists.
    id: String,
    /// Absolute manifest path returned by Cargo.
    manifest_path: PathBuf,
}

/// Kind of Rust tree being inspected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RootKind {
    /// Production source files.
    Source,
    /// External or crate-internal test files.
    Tests,
    /// Benchmarks, fuzz targets, examples, and other project-owned Rust files.
    Other,
}

/// Checks the configured source and test trees without invoking rustfmt.
///
/// When no directories are supplied, Cargo workspace default members are
/// discovered and each package's `src`, `src/tests`, and `tests` trees are
/// checked. Supplying either directory selects the legacy explicit-directory
/// mode and derives crate-internal tests from the selected source directory.
/// Filesystem or Cargo metadata errors are returned as `Err`.
/// The project path, optional source path, and optional test path determine
/// the roots that are checked.
///
/// # Parameters
///
/// * `project` - Project root used to resolve manifests and relative roots.
/// * `source_dir` - Optional production source directory relative to `project`.
/// * `test_dir` - Optional external test directory relative to `project`.
///
/// # Returns
///
/// Returns all diagnostics found in the selected source and test trees.
///
/// # Errors
///
/// Returns an error when Cargo metadata cannot be obtained or a selected file
/// cannot be read.
pub fn check(project: &Path, source_dir: Option<&Path>, test_dir: Option<&Path>) -> Result<Vec<Diagnostic>> {
    let exceptions = ExceptionConfig::load(project)?;
    let mut diagnostics = Vec::new();
    let roots = if source_dir.is_some() || test_dir.is_some() {
        vec![(
            project.join(source_dir.unwrap_or(Path::new("src"))),
            project.join(test_dir.unwrap_or(Path::new("tests"))),
        )]
    } else if project.join("Cargo.toml").is_file() {
        workspace_roots(project)?
    } else {
        vec![(project.join("src"), project.join("tests"))]
    };
    let enforce_headers = project.join("Cargo.toml").is_file();
    for (source, tests) in roots {
        check_package_roots(project, &source, &tests, &exceptions, enforce_headers, &mut diagnostics)?;
    }
    Ok(diagnostics)
}

/// Runs rustfmt in check mode and then evaluates the fixed project style rules.
///
/// The rustfmt subprocess runs in `project` and returns an error when the
/// project is not formatted or Cargo/rustfmt cannot be started. Custom style
/// diagnostics are returned only after rustfmt succeeds.
/// The project path and optional roots determine the formatting and checks.
///
/// # Parameters
///
/// * `project` - Project root passed to Cargo and used to resolve roots.
/// * `source_dir` - Optional production source directory relative to `project`.
/// * `test_dir` - Optional external test directory relative to `project`.
///
/// # Returns
///
/// Returns custom style diagnostics after rustfmt succeeds.
///
/// # Errors
///
/// Returns an error when rustfmt fails, Cargo cannot be started, or a selected
/// project file cannot be read.
pub fn check_project(project: &Path, source_dir: Option<&Path>, test_dir: Option<&Path>) -> Result<Vec<Diagnostic>> {
    run_cargo_fmt(project, true)?;
    check(project, source_dir, test_dir)
}

/// Prints style diagnostics as text or pretty JSON.
///
/// JSON serialization errors are returned as `Err`; text output is written to
/// standard output and does not itself fail.
/// The diagnostics are rendered as text or JSON according to json.
///
/// # Parameters
///
/// * `diagnostics` - Diagnostics to render in their existing order.
/// * `json` - Whether to render pretty-printed JSON instead of text.
///
/// # Errors
///
/// Returns an error only when JSON serialization fails.
pub fn print_diagnostics(diagnostics: &[Diagnostic], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(diagnostics)?);
    } else {
        for diagnostic in diagnostics {
            if diagnostic.line == 0 {
                println!("error: {}: {}", diagnostic.path, diagnostic.message);
            } else {
                println!("error: {}:{}: {}", diagnostic.path, diagnostic.line, diagnostic.message);
            }
        }
        if diagnostics.is_empty() {
            println!("Rust style checks passed.");
        } else {
            println!("Rust style checks failed with {} issue(s).", diagnostics.len());
        }
    }
    Ok(())
}

/// Formats a project and validates its default workspace style rules.
///
/// This compatibility wrapper uses workspace discovery. Use
/// [`fix_project`] when a migration or caller supplies explicit source/test
/// directories.
/// The project is formatted and then checked unless dry_run is enabled.
///
/// # Parameters
///
/// * `project` - Project root passed to Cargo.
/// * `dry_run` - Prints the formatting command without changing files when
///   true.
///
/// # Errors
///
/// Returns an error when rustfmt or the subsequent style check fails.
pub fn fix(project: &Path, dry_run: bool) -> Result<()> {
    fix_project(project, None, None, dry_run)
}

/// Formats a project and validates the selected style roots.
///
/// The `source_dir` and `test_dir` values are interpreted relative to
/// `project`, matching the CLI options. In dry-run mode no files are changed
/// and the rustfmt command is printed instead. Errors from Cargo, rustfmt, or
/// the style checks are returned as `Err`.
/// The project and selected roots are formatted and validated.
///
/// # Parameters
///
/// * `project` - Project root passed to Cargo and used to resolve roots.
/// * `source_dir` - Optional production source directory relative to `project`.
/// * `test_dir` - Optional external test directory relative to `project`.
/// * `dry_run` - Prints the formatting command without changing files when
///   true.
///
/// # Errors
///
/// Returns an error when rustfmt fails, diagnostics remain after formatting,
/// or a selected project file cannot be read.
pub fn fix_project(project: &Path, source_dir: Option<&Path>, test_dir: Option<&Path>, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("{}", cargo_fmt_description(&cargo_fmt_command(project, false)));
        return Ok(());
    }
    run_cargo_fmt(project, false)?;
    let diagnostics = check(project, source_dir, test_dir)?;
    if !diagnostics.is_empty() {
        bail!("style checks still report {} issue(s)", diagnostics.len());
    }
    Ok(())
}

/// Checks one package's production and test roots.
///
/// The internal test tree is checked only when it exists, and an external test
/// tree is skipped when it is the same path.
///
/// # Parameters
///
/// * `project` - Project root used to produce relative diagnostic paths.
/// * `source` - Production source root to inspect.
/// * `tests` - External test root to inspect.
/// * `diagnostics` - Mutable diagnostic collection to append to.
///
/// # Errors
///
/// Returns an error when a selected source or test file cannot be read.
fn check_package_roots(
    project: &Path,
    source: &Path,
    tests: &Path,
    exceptions: &ExceptionConfig,
    enforce_headers: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<()> {
    check_root(
        project,
        source,
        RootKind::Source,
        exceptions,
        enforce_headers,
        diagnostics,
    )?;
    let internal_tests = source.join("tests");
    if internal_tests.is_dir() {
        check_internal_test_module(project, source, exceptions, diagnostics);
        check_root(
            project,
            &internal_tests,
            RootKind::Tests,
            exceptions,
            enforce_headers,
            diagnostics,
        )?;
    }
    if tests != internal_tests {
        check_root(
            project,
            tests,
            RootKind::Tests,
            exceptions,
            enforce_headers,
            diagnostics,
        )?;
    }
    let package_root = source.parent().unwrap_or(source);
    for directory in ["benches", "fuzz", "examples"] {
        check_root(
            project,
            &package_root.join(directory),
            RootKind::Other,
            exceptions,
            enforce_headers,
            diagnostics,
        )?;
    }
    check_source_test_pairs(project, source, tests, exceptions, diagnostics)?;
    Ok(())
}

/// Enforces the legacy opt-in source-to-integration-test pairing rule.
fn check_source_test_pairs(
    project: &Path,
    source: &Path,
    tests: &Path,
    exceptions: &ExceptionConfig,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<()> {
    if !env_flag("STYLE_ENFORCE_SOURCE_TEST_PAIRS", false) || !source.is_dir() || !tests.is_dir() {
        return Ok(());
    }
    let test_files = rust_files(tests)?;
    for file in rust_files(source)? {
        let relative = project_relative_path(project, &file);
        let source_relative = file.strip_prefix(source).unwrap_or(file.as_path());
        let file_name = file.file_name().and_then(|value| value.to_str()).unwrap_or_default();
        if matches!(file_name, "lib.rs" | "main.rs" | "mod.rs" | "macros.rs")
            || file_name.ends_with("_tests.rs")
            || exceptions.allows("source-test-pair", &relative)
            || type_alias_only(&file)?
        {
            continue;
        }
        let Some(stem) = file.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let expected_name = format!("{stem}_tests.rs");
        let expected = tests.join(source_relative).with_file_name(&expected_name);
        let sibling = file.with_file_name(&expected_name);
        let has_matching_test = test_files
            .iter()
            .any(|test| test.file_name().and_then(|value| value.to_str()) == Some(expected_name.as_str()));
        let mut parent = source_relative.parent().and_then(Path::parent);
        let mut has_parent_test = false;
        while let Some(directory) = parent {
            if let Some(module) = directory.file_name().and_then(|value| value.to_str()) {
                let module_source = source.join(directory).with_extension("rs");
                let module_test = format!("{module}_tests.rs");
                if module_source.is_file()
                    && test_files
                        .iter()
                        .any(|test| test.file_name().and_then(|value| value.to_str()) == Some(module_test.as_str()))
                {
                    has_parent_test = true;
                    break;
                }
            }
            parent = directory.parent();
        }
        if !expected.is_file() && !has_matching_test && !sibling.is_file() && !has_parent_test {
            add(
                diagnostics,
                "STYLE014",
                &relative,
                0,
                &format!("missing corresponding test file 'tests/{expected_name}'"),
            );
        }
    }
    Ok(())
}

/// Returns Rust files below a directory in deterministic order.
fn rust_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !root.is_dir() {
        return Ok(files);
    }
    for entry in WalkDir::new(root).into_iter().filter_entry(|entry| {
        !entry.file_type().is_dir() || !matches!(entry.file_name().to_str(), Some("target" | ".git"))
    }) {
        let entry = entry.context("failed to walk style source tree")?;
        if entry.file_type().is_file() && entry.path().extension().and_then(|value| value.to_str()) == Some("rs") {
            files.push(entry.into_path());
        }
    }
    files.sort();
    Ok(files)
}

/// Identifies files containing only one or more top-level type aliases.
fn type_alias_only(path: &Path) -> Result<bool> {
    let text = fs::read_to_string(path)?;
    let Ok(file) = parse_file(&text) else {
        return Ok(false);
    };
    Ok(!file.items.is_empty() && file.items.iter().all(|item| matches!(item, Item::Type(_))))
}

/// Resolves workspace package roots through Cargo metadata.
///
/// # Parameters
///
/// * `project` - Workspace root passed to Cargo metadata.
///
/// # Returns
///
/// Returns source and external-test roots for the selected workspace packages.
///
/// # Errors
///
/// Returns an error when Cargo cannot be started, exits unsuccessfully, or
/// produces invalid metadata.
fn workspace_roots(project: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(project)
        .output()
        .context("failed to start cargo metadata")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let metadata: CargoMetadata =
        serde_json::from_slice(&output.stdout).context("failed to parse cargo metadata output")?;
    let selected_ids = if metadata.workspace_default_members.is_empty() {
        &metadata.workspace_members
    } else {
        &metadata.workspace_default_members
    };
    let packages = metadata
        .packages
        .into_iter()
        .filter(|package| selected_ids.is_empty() || selected_ids.contains(&package.id))
        .filter_map(|package| {
            let package_root = package.manifest_path.parent()?;
            Some((package_root.join("src"), package_root.join("tests")))
        })
        .collect();
    Ok(packages)
}

/// Verifies that an existing `src/tests` tree is connected to the crate root.
///
/// Adds `STYLE012` when neither `lib.rs` nor `main.rs` declares the
/// conventional test module entry point.
///
/// # Parameters
///
/// * `project` - Project root used to format the diagnostic path.
/// * `source` - Source root containing the internal test tree.
/// * `diagnostics` - Mutable diagnostic collection to append to.
fn check_internal_test_module(
    project: &Path,
    source: &Path,
    exceptions: &ExceptionConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let internal_tests = source.join("tests");
    let has_module = [source.join("lib.rs"), source.join("main.rs")]
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .any(|text| text.contains("#[cfg(test)]") && text.contains("mod tests"));
    if !has_module {
        let relative = project_relative_path(project, &internal_tests);
        if !exceptions.allows("internal-test-module", &relative) {
            add(
                diagnostics,
                "STYLE012",
                &relative,
                0,
                "src/tests must be connected from the crate root with #[cfg(test)] mod tests;",
            );
        }
    }
}

/// Runs Cargo's formatter in either checking or rewriting mode.
///
/// # Parameters
///
/// * `project` - Project root in which Cargo runs.
/// * `check_only` - Whether rustfmt should validate without rewriting files.
///
/// # Errors
///
/// Returns an error when Cargo or rustfmt cannot be started or exits with a
/// failure status.
fn run_cargo_fmt(project: &Path, check_only: bool) -> Result<()> {
    run_formatter_command(cargo_fmt_command(project, check_only), project)?;
    let fuzz_manifest = project.join("fuzz/Cargo.toml");
    if fuzz_manifest.is_file() {
        run_formatter_command(cargo_fuzz_fmt_command(&fuzz_manifest, check_only), project)?;
    }
    Ok(())
}

/// Runs one Cargo formatter command and propagates its exact failure.
fn run_formatter_command(mut command: Command, project: &Path) -> Result<()> {
    let description = cargo_fmt_description(&command);
    let status = command
        .current_dir(project)
        .status()
        .with_context(|| format!("failed to start {description}"))?;
    if !status.success() {
        bail!("{description} failed");
    }
    Ok(())
}

/// Builds a formatter command using optional toolchain and configuration
/// overrides.
///
/// `check_only` selects validation instead of rewriting. Environment values are
/// passed as individual arguments; relative configuration paths are resolved by
/// rustfmt from the project directory. Empty values retain Cargo's defaults.
fn cargo_fmt_command(project: &Path, check_only: bool) -> Command {
    let mut command = Command::new("cargo");
    if let Some(toolchain) = std::env::var_os("RS_INFRA_STYLE_TOOLCHAIN").filter(|value| !value.is_empty()) {
        let mut argument = std::ffi::OsString::from("+");
        argument.push(toolchain);
        command.arg(argument);
    }
    command.args(["fmt", "--all", "--manifest-path"]);
    command.arg(project.join("Cargo.toml"));
    let config = std::env::var_os("RS_INFRA_STYLE_RUSTFMT_CONFIG").filter(|value| !value.is_empty());
    if check_only || config.is_some() {
        command.arg("--");
    }
    if check_only {
        command.arg("--check");
    }
    if let Some(config) = config {
        command.arg("--config-path").arg(config);
    }
    command
}

/// Builds the legacy second formatter invocation for a standalone fuzz crate.
fn cargo_fuzz_fmt_command(manifest: &Path, check_only: bool) -> Command {
    let mut command = Command::new("cargo");
    if let Some(toolchain) = std::env::var_os("RS_INFRA_STYLE_TOOLCHAIN").filter(|value| !value.is_empty()) {
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
    if let Some(config) = std::env::var_os("RS_INFRA_STYLE_RUSTFMT_CONFIG").filter(|value| !value.is_empty()) {
        command.args(["--config-path"]);
        command.arg(config);
    }
    command
}

/// Describes a formatter command for dry runs and failure diagnostics.
///
/// Arguments containing whitespace are quoted for readability; the returned
/// text is never passed to a shell.
fn cargo_fmt_description(command: &Command) -> String {
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

/// Walks a configured root and dispatches each Rust file to its role checks.
///
/// Non-Rust files and missing roots are ignored. Source-tree `tests` entries
/// are handled separately as crate-internal tests.
///
/// # Parameters
///
/// * `project` - Project root used to produce relative diagnostic paths.
/// * `root` - Filesystem root to walk.
/// * `kind` - Whether the root contains production or test files.
/// * `diagnostics` - Mutable diagnostic collection to append to.
///
/// # Errors
///
/// Returns an error when a Rust source file cannot be read.
fn check_root(
    project: &Path,
    root: &Path,
    kind: RootKind,
    exceptions: &ExceptionConfig,
    enforce_headers: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in WalkDir::new(root).into_iter().filter_entry(|entry| {
        !entry.file_type().is_dir() || !matches!(entry.file_name().to_str(), Some("target" | ".git"))
    }) {
        let entry = entry.context("failed to walk style source tree")?;
        let path = entry.path();
        if kind == RootKind::Source
            && path
                .strip_prefix(root)
                .ok()
                .and_then(|relative| relative.components().next())
                .is_some_and(|component| component.as_os_str() == "tests")
        {
            continue;
        }
        if !entry.file_type().is_file() || path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(path)?;
        let relative = project_relative_path(project, path);
        match kind {
            RootKind::Source => check_source_file(&relative, path, &text, exceptions, enforce_headers, diagnostics),
            RootKind::Tests => check_test_file(&relative, &text, exceptions, enforce_headers, diagnostics),
            RootKind::Other => check_common_file(&relative, &text, exceptions, enforce_headers, diagnostics),
        }
    }
    Ok(())
}

/// Returns a project-relative diagnostic path using forward slashes.
///
/// If `path` is not rooted beneath `project`, its normalized absolute path is
/// returned so path-scoped exceptions cannot accidentally match it.
///
/// # Parameters
///
/// * `project` - Canonical project root used as the path prefix.
/// * `path` - Filesystem path to render in a diagnostic.
///
/// # Returns
///
/// The path relative to `project`, or a normalized absolute path when it is
/// outside the project.
fn project_relative_path(project: &Path, path: &Path) -> String {
    let project = normalize_path_separators(&project.to_string_lossy());
    let path = normalize_path_separators(&path.to_string_lossy());
    let project = project.trim_end_matches('/');
    let prefix = path.get(..project.len()).filter(|prefix| {
        if is_windows_path(project) && is_windows_path(&path) {
            prefix.eq_ignore_ascii_case(project)
        } else {
            *prefix == project
        }
    });
    prefix
        .and_then(|_| path.get(project.len()..))
        .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
        .map(|suffix| suffix.trim_start_matches('/').to_owned())
        .unwrap_or(path)
}

fn is_windows_path(path: &str) -> bool {
    path.starts_with("//") || (path.as_bytes().get(1) == Some(&b':') && path.as_bytes().get(2) == Some(&b'/'))
}

fn normalize_path_separators(path: &str) -> String {
    let mut normalized = String::with_capacity(path.len());
    let mut previous_was_separator = false;
    let preserve_unc_prefix = path.starts_with("//") || path.starts_with(r"\\");
    for character in path.chars() {
        if character == '/' || character == '\\' {
            if !previous_was_separator || (preserve_unc_prefix && normalized == "/") {
                normalized.push('/');
            }
            previous_was_separator = true;
        } else {
            normalized.push(character);
            previous_was_separator = false;
        }
    }
    if let Some(unc_path) = normalized.strip_prefix("//?/UNC/") {
        return format!("//{unc_path}");
    }
    if let Some(verbatim_path) = normalized.strip_prefix("//?/") {
        return verbatim_path.to_owned();
    }
    normalized
}

/// Applies production-source rules to one parsed source file.
///
/// The function records diagnostics for the supplied file and never changes
/// its contents.
///
/// # Parameters
///
/// * `relative` - Project-relative path used in diagnostics.
/// * `path` - Filesystem path used for filename checks.
/// * `text` - Complete source contents to inspect.
/// * `exceptions` - Exact project-local exceptions for supported rules.
/// * `diagnostics` - Mutable diagnostic collection to append to.
fn check_source_file(
    relative: &str,
    path: &Path,
    text: &str,
    exceptions: &ExceptionConfig,
    enforce_headers: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_common_file(relative, text, exceptions, enforce_headers, diagnostics);
    if !coverage_exception_allowed(relative, text, exceptions) {
        for (line, value) in text.lines().enumerate() {
            if value.trim_start().starts_with("#[cfg") && value.contains("coverage")
                || value.trim_start().starts_with("#[cfg_attr") && value.contains("coverage")
            {
                add(
                    diagnostics,
                    "STYLE008",
                    relative,
                    line + 1,
                    "coverage-specific cfg is not allowed in source",
                );
            }
        }
    }
    check_aggregation(relative, text, exceptions, diagnostics);
    check_type_layout(relative, path, text, exceptions, diagnostics);
}

/// Applies checks that are mechanically valid for every project-owned Rust
/// file.
fn check_common_file(
    relative: &str,
    text: &str,
    exceptions: &ExceptionConfig,
    enforce_headers: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if enforce_headers {
        check_file_header(relative, text, diagnostics);
    }
    check_imports(relative, text, exceptions, diagnostics);
}

/// Checks the exact accepted Rust file header from the repository standard.
fn check_file_header(relative: &str, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let lines: Vec<_> = text.lines().take(8).collect();
    let valid = lines.len() >= 7
        && lines[0] == "// ============================================================================="
        && valid_copyright_line(lines[1])
        && lines[2] == "//"
        && lines[3] == "//    SPDX-License-Identifier: Apache-2.0"
        && lines[4] == "//"
        && lines[5] == "//    Licensed under the Apache License, Version 2.0."
        && lines[6] == "// =============================================================================";
    if !valid {
        add(
            diagnostics,
            "DOC-001",
            relative,
            1,
            "Rust files must begin with the accepted seven-line copyright and license header",
        );
    }
}

fn valid_copyright_line(line: &str) -> bool {
    let Some(value) = line.strip_prefix("//    Copyright (c) ") else {
        return false;
    };
    let Some(years) = value.strip_suffix(" Haixing Hu.") else {
        return false;
    };
    let parts: Vec<_> = years.split(" - ").collect();
    (parts.len() == 1 || parts.len() == 2)
        && parts
            .iter()
            .all(|part| part.len() == 4 && part.parse::<u16>().is_ok_and(|year| year >= 2025))
}

fn coverage_exception_allowed(relative: &str, text: &str, exceptions: &ExceptionConfig) -> bool {
    exceptions.allows("coverage-cfg", relative)
        && text
            .lines()
            .any(|line| line.contains("qubit-style: allow coverage-cfg"))
}

/// Applies naming and source-redirection rules to one test file.
///
/// The function records diagnostics for the supplied file and never changes
/// its contents.
///
/// # Parameters
///
/// * `relative` - Project-relative path used in diagnostics.
/// * `text` - Complete test source contents to inspect.
/// * `exceptions` - Exact project-local exceptions for supported rules.
/// * `diagnostics` - Mutable diagnostic collection to append to.
fn check_test_file(
    relative: &str,
    text: &str,
    exceptions: &ExceptionConfig,
    enforce_headers: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_common_file(relative, text, exceptions, enforce_headers, diagnostics);
    let is_support_file = relative
        .split('/')
        .any(|part| matches!(part, "support" | "common" | "fixtures" | "coverage_support"));
    let contains_tests = contains_test_functions(text) || contains_test_attributes(text);
    if !is_support_file
        && contains_tests
        && !relative.ends_with("_tests.rs")
        && !relative.ends_with("/mod.rs")
        && !exceptions.allows("test-file-name", relative)
    {
        add(
            diagnostics,
            "STYLE001",
            relative,
            0,
            "test files should be named '*_tests.rs' or 'mod.rs'",
        );
    }
    if !is_support_file && !exceptions.allows("test-redirect", relative) {
        for (line, value) in text.lines().enumerate() {
            let trimmed = value.trim_start();
            if trimmed.starts_with("include!(") || trimmed.starts_with("#[path") {
                add(
                    diagnostics,
                    "STYLE002",
                    relative,
                    line + 1,
                    "test files must contain concrete tests directly; redirects are not allowed",
                );
            }
        }
    }
}

/// Detects test attributes even when a file only declares an external module.
fn contains_test_attributes(text: &str) -> bool {
    text.lines().any(|value| {
        let trimmed = value.trim_start();
        trimmed.starts_with("#[cfg(test")
            || trimmed.starts_with("#[test")
            || trimmed.starts_with("#[rstest")
            || trimmed.contains("::test]")
            || trimmed.contains("::test(")
    })
}

/// Detects conventional test functions, including functions in inline modules.
fn contains_test_functions(text: &str) -> bool {
    let Ok(file) = parse_file(text) else {
        return false;
    };
    items_contain_test_functions(&file.items)
}

/// Recursively checks Rust items for test and rstest function attributes.
fn items_contain_test_functions(items: &[syn::Item]) -> bool {
    items.iter().any(|item| match item {
        syn::Item::Fn(function) => {
            function.attrs.iter().any(|attribute| {
                attribute
                    .path()
                    .segments
                    .last()
                    .is_some_and(|segment| matches!(segment.ident.to_string().as_str(), "test" | "rstest"))
            }) || function.attrs.iter().any(|attribute| {
                attribute.path().is_ident("cfg")
                    && attribute
                        .meta
                        .require_list()
                        .is_ok_and(|list| list.tokens.to_string().contains("test"))
            })
        }
        syn::Item::Mod(module) => module
            .content
            .as_ref()
            .is_some_and(|(_, nested)| items_contain_test_functions(nested)),
        _ => false,
    })
}

/// Reads a legacy boolean style switch from the process environment.
fn env_flag(name: &str, default: bool) -> bool {
    match std::env::var(name).ok().as_deref() {
        Some("0") | Some("false") | Some("False") | Some("FALSE") => false,
        Some("1") | Some("true") | Some("True") | Some("TRUE") => true,
        Some(_) => default,
        None => default,
    }
}

/// Checks import shape and ordering in one project-owned Rust file.
///
/// Imports are classified into standard-library, external-crate, and
/// current-crate groups before ordering and blank-line diagnostics are added.
///
/// # Parameters
///
/// * `relative` - Project-relative path used in diagnostics.
/// * `text` - Complete source contents to inspect.
/// * `exceptions` - Exact project-local exceptions for supported rules.
/// * `diagnostics` - Mutable diagnostic collection to append to.
fn check_imports(relative: &str, text: &str, exceptions: &ExceptionConfig, diagnostics: &mut Vec<Diagnostic>) {
    if exceptions.allows("explicit-imports", relative) {
        return;
    }
    let mut last_group = None;
    let mut blank_lines = 0;
    for (line, value) in text.lines().enumerate() {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            if last_group.is_some() {
                blank_lines += 1;
            }
            continue;
        }
        if trimmed.starts_with("#[") && last_group.is_some() {
            continue;
        }
        let import = trimmed
            .strip_prefix("use ")
            .or_else(|| trimmed.strip_prefix("pub use "));
        if let Some(path) = import.and_then(|value| value.strip_suffix(';')) {
            if path.contains('{') {
                add(
                    diagnostics,
                    "STYLE003",
                    relative,
                    line + 1,
                    "use declarations must import one object; brace lists are not allowed",
                );
            }
            if path.contains('*') {
                add(
                    diagnostics,
                    "STYLE004",
                    relative,
                    line + 1,
                    "wildcard imports are not allowed",
                );
            }
            let group = if path.starts_with("std::") || path.starts_with("core::") || path.starts_with("alloc::") {
                0
            } else if path.starts_with("crate::") || path.starts_with("self::") || path.starts_with("super::") {
                2
            } else {
                1
            };
            if let Some(previous) = last_group {
                if group < previous {
                    add(
                        diagnostics,
                        "STYLE005",
                        relative,
                        line + 1,
                        "imports must be ordered standard, external, then current-crate",
                    );
                } else if group != previous && blank_lines != 1 {
                    add(
                        diagnostics,
                        "STYLE006",
                        relative,
                        line + 1,
                        "import groups must be separated by exactly one blank line",
                    );
                }
            }
            last_group = Some(group);
            blank_lines = 0;
            continue;
        }
        last_group = None;
        blank_lines = 0;
        if trimmed.starts_with("#[allow") && trimmed.contains("unused_imports") {
            add(
                diagnostics,
                "STYLE007",
                relative,
                line + 1,
                "unused_imports allows are not allowed",
            );
        }
    }
    if relative.ends_with("/mod.rs") && !has_aggregation_items(text) {
        for (line, value) in text.lines().enumerate() {
            let trimmed = value.trim_start();
            if trimmed.starts_with("use ") {
                add(
                    diagnostics,
                    "STYLE016",
                    relative,
                    line + 1,
                    "aggregation-only mod.rs files must not collect private imports for child modules",
                );
            }
        }
    }
}

/// Returns whether an aggregation file contains a concrete top-level item.
fn has_aggregation_items(text: &str) -> bool {
    text.lines().any(|value| {
        let trimmed = value.trim_start();
        [
            "async fn ",
            "fn ",
            "struct ",
            "enum ",
            "trait ",
            "type ",
            "const ",
            "static ",
            "impl ",
            "macro_rules!",
            "pub fn ",
            "pub struct ",
            "pub enum ",
            "pub trait ",
            "pub type ",
            "pub const ",
            "pub static ",
            "pub impl ",
        ]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    })
}

/// Ensures aggregation files contain only declarations and re-exports.
///
/// The check applies only to `lib.rs` and `mod.rs` files and records any
/// top-level implementation item as a diagnostic.
///
/// # Parameters
///
/// * `relative` - Project-relative path used in diagnostics.
/// * `text` - Complete source contents to inspect.
/// * `diagnostics` - Mutable diagnostic collection to append to.
fn check_aggregation(relative: &str, text: &str, exceptions: &ExceptionConfig, diagnostics: &mut Vec<Diagnostic>) {
    if !relative.ends_with("/lib.rs") && !relative.ends_with("/mod.rs") {
        return;
    }
    if exceptions.allows("aggregation-files", relative) {
        return;
    }
    let Ok(file) = parse_file(text) else {
        return;
    };
    for item in file.items {
        let allowed = matches!(&item, Item::Mod(_) | Item::Use(_))
            || matches!(&item, Item::Fn(function) if function.attrs.iter().any(|attribute| {
                attribute.path().segments.last().is_some_and(|segment| {
                    matches!(segment.ident.to_string().as_str(), "proc_macro" | "proc_macro_attribute" | "proc_macro_derive")
                })
            }));
        if !allowed {
            add(
                diagnostics,
                "STYLE009",
                relative,
                0,
                "lib.rs and mod.rs files must only declare modules and re-export items",
            );
        }
    }
}

/// Checks public type count and filename alignment in one source file.
///
/// A parse failure is ignored here because syntax diagnostics belong to the
/// compiler; successfully parsed public top-level types are checked against
/// the file stem.
///
/// # Parameters
///
/// * `relative` - Project-relative path used in diagnostics.
/// * `path` - Filesystem path whose stem is checked.
/// * `text` - Complete source contents to parse.
/// * `diagnostics` - Mutable diagnostic collection to append to.
fn check_type_layout(
    relative: &str,
    path: &Path,
    text: &str,
    exceptions: &ExceptionConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let file_name = path.file_stem().and_then(|value| value.to_str()).unwrap_or_default();
    if matches!(file_name, "lib" | "main" | "mod" | "macros") {
        return;
    }
    let Ok(parsed) = parse_file(text) else {
        return;
    };
    let types: Vec<_> = parsed
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(item) if matches!(item.vis, Visibility::Public(_)) => Some(("struct", item.ident.to_string())),
            Item::Enum(item) if matches!(item.vis, Visibility::Public(_)) => Some(("enum", item.ident.to_string())),
            Item::Trait(item) if matches!(item.vis, Visibility::Public(_)) => Some(("trait", item.ident.to_string())),
            _ => None,
        })
        .collect();
    if types.len() > 1 {
        if exceptions.allows("public-type-layout", relative) || exceptions.allows("multiple-public-types", relative) {
            return;
        }
        add(
            diagnostics,
            "STYLE010",
            relative,
            0,
            "file contains multiple public top-level types; split them or add a reviewed .infra/style/exceptions.toml entry",
        );
        return;
    }
    if let Some((kind, name)) = types.first() {
        let expected = snake_case(name);
        if expected != file_name && !exceptions.allows("type-file-name", relative) {
            add(
                diagnostics,
                "STYLE011",
                relative,
                0,
                &format!("{kind} '{name}' should live in '{expected}.rs', not '{file_name}.rs'"),
            );
        }
    }
}

/// Converts a Rust type name into the expected snake-case filename stem.
///
/// Uppercase characters after the first character receive an underscore, and
/// Unicode lowercase mappings are preserved.
///
/// # Parameters
///
/// * `value` - Rust type name to convert.
///
/// # Returns
///
/// Returns the expected snake-case filename stem.
fn snake_case(value: &str) -> String {
    let mut output = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            output.push('_');
        }
        output.extend(character.to_lowercase());
    }
    output
}

/// Appends a normalized diagnostic record to the current result set.
///
/// The diagnostic always uses schema version 1 and error severity.
///
/// # Parameters
///
/// * `diagnostics` - Mutable diagnostic collection to append to.
/// * `code` - Stable style rule identifier.
/// * `path` - Project-relative source path.
/// * `line` - One-based source line, or zero for a file-level diagnostic.
/// * `message` - Human-readable explanation of the violation.
fn add(diagnostics: &mut Vec<Diagnostic>, code: &str, path: &str, line: usize, message: &str) {
    diagnostics.push(Diagnostic {
        schema_version: 1,
        code: code.into(),
        severity: "error".into(),
        path: path.into(),
        line,
        message: message.into(),
    });
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::check;
    use super::check_project;
    use super::fix_project;

    #[test]
    fn reports_type_file_and_wildcard_import_violations() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("src");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("wrong.rs"), "use std::*;\npub struct GoodName;\n").unwrap();
        let diagnostics = check(directory.path(), None, None).unwrap();
        assert!(diagnostics.iter().any(|item| item.code == "STYLE004"));
        assert!(diagnostics.iter().any(|item| item.code == "STYLE011"));
    }

    #[test]
    fn configured_exception_matches_only_the_exact_path_and_rule() {
        let directory = tempfile::tempdir().unwrap();
        let tests = directory.path().join("tests");
        fs::create_dir_all(&tests).unwrap();
        fs::create_dir_all(directory.path().join(".infra/style")).unwrap();
        fs::write(
            directory.path().join(".infra/style/exceptions.toml"),
            "format = 1\n\n[[exceptions]]\nrule = \"test-redirect\"\npath = \"tests/wrong_name.rs\"\nreason = \"This fixture intentionally redirects to shared test code.\"\n",
        )
        .unwrap();
        fs::write(tests.join("legacy_tests.rs"), "include!(\"legacy_impl.rs\");\n").unwrap();
        fs::write(
            tests.join("wrong_name.rs"),
            "#[test]\nfn test_example() {}\ninclude!(\"other_impl.rs\");\n",
        )
        .unwrap();
        fs::write(
            tests.join("other_tests.rs"),
            "// qubit-style: allow test-redirect\ninclude!(\"legacy_impl.rs\");\n",
        )
        .unwrap();
        fs::create_dir_all(tests.join("target")).unwrap();
        fs::write(
            tests.join("target/generated.rs"),
            "use std::*;\ninclude!(\"generated.rs\");\n",
        )
        .unwrap();
        fs::create_dir_all(directory.path().join(".infra/style")).unwrap();
        fs::write(
            directory.path().join(".infra/style/exceptions.toml"),
            "format = 1\n\n[[exceptions]]\nrule = \"test-redirect\"\npath = \"tests/wrong_name.rs\"\nreason = \"This fixture intentionally redirects to shared test code.\"\n",
        )
        .unwrap();

        let diagnostics = check(directory.path(), None, None).unwrap();

        assert_eq!(3, diagnostics.len());
        assert!(
            diagnostics
                .iter()
                .any(|item| { item.path == "tests/wrong_name.rs" && item.code == "STYLE001" })
        );
        assert!(
            diagnostics
                .iter()
                .any(|item| { item.path == "tests/other_tests.rs" && item.code == "STYLE002" })
        );
        assert!(
            diagnostics
                .iter()
                .any(|item| { item.path == "tests/legacy_tests.rs" && item.code == "STYLE002" })
        );
    }

    #[test]
    fn test_filename_rule_applies_only_to_files_containing_tests() {
        let directory = tempfile::tempdir().unwrap();
        let tests = directory.path().join("tests");
        fs::create_dir_all(tests.join("support")).unwrap();
        fs::write(tests.join("support/fixture.rs"), "pub fn fixture() {}\n").unwrap();
        fs::write(tests.join("wrong_name.rs"), "#[test]\nfn test_example() {}\n").unwrap();

        let diagnostics = check(directory.path(), None, None).unwrap();

        assert_eq!(1, diagnostics.len());
        assert_eq!("STYLE001", diagnostics[0].code);
        assert_eq!("tests/wrong_name.rs", diagnostics[0].path);
    }

    #[test]
    fn test_redirect_is_reported_even_when_file_contains_a_test() {
        let directory = tempfile::tempdir().unwrap();
        let tests = directory.path().join("tests");
        fs::create_dir_all(&tests).unwrap();
        fs::write(
            tests.join("redirect_tests.rs"),
            "#[test]\nfn test_example() {}\ninclude!(\"shared.rs\");\n",
        )
        .unwrap();

        let diagnostics = check(directory.path(), None, None).unwrap();

        assert!(
            diagnostics
                .iter()
                .any(|item| { item.path == "tests/redirect_tests.rs" && item.code == "STYLE002" })
        );
    }

    #[test]
    fn checks_benchmark_files_for_explicit_imports() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("src")).unwrap();
        fs::create_dir_all(directory.path().join("benches")).unwrap();
        fs::write(directory.path().join("src/lib.rs"), "pub fn value() {}\n").unwrap();
        fs::write(directory.path().join("benches/value.rs"), "use std::*;\n").unwrap();

        let diagnostics = check(directory.path(), None, None).unwrap();

        assert!(
            diagnostics
                .iter()
                .any(|item| { item.path == "benches/value.rs" && item.code == "STYLE004" })
        );
    }

    #[test]
    fn missing_rust_header_is_reported() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("src")).unwrap();
        fs::write(
            directory.path().join("Cargo.toml"),
            "[package]\nname = \"header-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(directory.path().join("src/lib.rs"), "pub fn value() {}\n").unwrap();

        let diagnostics = check(directory.path(), None, None).unwrap();

        assert!(
            diagnostics
                .iter()
                .any(|item| { item.path == "src/lib.rs" && item.code == "DOC-001" })
        );
    }

    #[test]
    fn coverage_exception_requires_source_allow_comment() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("src");
        fs::create_dir_all(directory.path().join(".infra/style")).unwrap();
        fs::create_dir_all(&source).unwrap();
        fs::write(
            directory.path().join("Cargo.toml"),
            "[package]\nname = \"coverage-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            directory.path().join(".infra/style/exceptions.toml"),
            "format = 1\n\n[[exceptions]]\nrule = \"coverage-cfg\"\npath = \"src/lib.rs\"\nreason = \"Coverage-only compatibility branch.\"\n",
        )
        .unwrap();
        fs::write(
            source.join("lib.rs"),
            "// =============================================================================\n//    Copyright (c) 2025 - 2026 Haixing Hu.\n//\n//    SPDX-License-Identifier: Apache-2.0\n//\n//    Licensed under the Apache License, Version 2.0.\n// =============================================================================\n#[cfg(coverage)]\npub fn value() {}\n",
        )
        .unwrap();

        let diagnostics = check(directory.path(), None, None).unwrap();

        assert!(
            diagnostics
                .iter()
                .any(|item| { item.path == "src/lib.rs" && item.code == "STYLE008" })
        );
    }

    #[test]
    fn project_relative_paths_match_mixed_windows_separators() {
        let project = Path::new(r"D:\a\rs-fs\rs-fs");
        let file = Path::new(r"D:/\/a/rs-fs/rs-fs/tests/example_tests.rs");

        assert_eq!("tests/example_tests.rs", super::project_relative_path(project, file));
    }

    #[test]
    fn project_relative_paths_match_windows_case_insensitive_roots() {
        let project = Path::new(r"D:\A\Repo");
        let file = Path::new(r"d:/a/repo/tests/Example.rs");

        assert_eq!("tests/Example.rs", super::project_relative_path(project, file));
    }

    #[test]
    fn project_relative_paths_match_unc_roots_case_insensitively() {
        let project = Path::new(r"\\SERVER\Share\Repo");
        let file = Path::new(r"\\server\share\repo\tests\Example.rs");

        assert_eq!("tests/Example.rs", super::project_relative_path(project, file));
    }

    #[test]
    fn project_relative_paths_match_verbatim_drive_roots() {
        let project = Path::new(r"\\?\D:\A\Repo");
        let file = Path::new(r"D:/a/repo/tests/Example.rs");

        assert_eq!("tests/Example.rs", super::project_relative_path(project, file));
    }

    #[test]
    fn project_check_rejects_unformatted_rust() {
        let directory = tempfile::tempdir().expect("temporary project");
        fs::create_dir_all(directory.path().join("src")).expect("source directory");
        fs::write(
            directory.path().join("Cargo.toml"),
            "[package]\nname = \"format-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("manifest");
        fs::write(directory.path().join("src/lib.rs"), "pub fn value( ) ->i32{1}\n").expect("source");

        let result = check_project(directory.path(), None, None);

        assert!(result.is_err(), "unformatted Rust must fail project check");
    }

    #[test]
    fn workspace_check_includes_crate_internal_tests() {
        let directory = tempfile::tempdir().expect("temporary workspace");
        fs::create_dir_all(directory.path().join("crate/src/tests")).expect("source directories");
        fs::write(
            directory.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate\"]\nresolver = \"3\"\n",
        )
        .expect("workspace manifest");
        fs::write(
            directory.path().join("crate/Cargo.toml"),
            "[package]\nname = \"crate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("package manifest");
        fs::write(directory.path().join("crate/src/lib.rs"), "#[cfg(test)] mod tests;\n").expect("crate root");
        fs::write(
            directory.path().join("crate/src/tests/redirect.rs"),
            "include!(\"fixture.rs\");\n",
        )
        .expect("internal test");

        let diagnostics = check(directory.path(), None, None).expect("style check");

        assert!(
            diagnostics
                .iter()
                .any(|item| { item.code == "STYLE002" && item.path == "crate/src/tests/redirect.rs" })
        );
    }

    #[test]
    fn fix_project_honors_explicit_source_and_test_directories() {
        let directory = tempfile::tempdir().expect("temporary project");
        fs::create_dir_all(directory.path().join("custom-src")).expect("source directory");
        fs::create_dir_all(directory.path().join("custom-tests")).expect("test directory");
        fs::write(
            directory.path().join("Cargo.toml"),
            "[package]\nname = \"custom-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("manifest");
        fs::create_dir_all(directory.path().join("src")).expect("Cargo source directory");
        fs::write(directory.path().join("src/lib.rs"), "pub fn value() -> i32 { 1 }\n").expect("Cargo source");
        fs::write(directory.path().join("custom-src/wrong.rs"), "use std::*;\n").expect("custom source");

        let result = fix_project(
            directory.path(),
            Some(Path::new("custom-src")),
            Some(Path::new("custom-tests")),
            false,
        );

        assert!(result.is_err(), "custom source violations must be checked");
    }
}
