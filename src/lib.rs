// qubit-style: allow all
//! Fixed first-phase Rust style checks used by Qubit Rust projects.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use syn::{Item, Visibility};
use walkdir::WalkDir;

/// A machine-readable style diagnostic.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
    /// Version of the serialized diagnostic schema.
    pub schema_version: u8,
    /// Stable rule identifier, such as `STYLE004`.
    pub code: String,
    /// Diagnostic severity, currently `error`.
    pub severity: String,
    /// Project-relative source path.
    pub path: String,
    /// One-based source line, or zero for file-level diagnostics.
    pub line: usize,
    /// Human-readable explanation of the violation.
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    #[serde(default)]
    workspace_default_members: Vec<String>,
    #[serde(default)]
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    manifest_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RootKind {
    Source,
    Tests,
}

/// Checks the configured source and test trees without invoking rustfmt.
///
/// When no directories are supplied, Cargo workspace default members are
/// discovered and each package's `src`, `src/tests`, and `tests` trees are
/// checked. Supplying either directory selects the legacy explicit-directory
/// mode and derives crate-internal tests from the selected source directory.
/// Filesystem or Cargo metadata errors are returned as `Err`.
pub fn check(
    project: &Path,
    source_dir: Option<&Path>,
    test_dir: Option<&Path>,
) -> Result<Vec<Diagnostic>> {
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
    for (source, tests) in roots {
        check_package_roots(project, &source, &tests, &mut diagnostics)?;
    }
    Ok(diagnostics)
}

/// Runs rustfmt in check mode and then evaluates the fixed project style rules.
///
/// The rustfmt subprocess runs in `project` and returns an error when the
/// project is not formatted or Cargo/rustfmt cannot be started. Custom style
/// diagnostics are returned only after rustfmt succeeds.
pub fn check_project(
    project: &Path,
    source_dir: Option<&Path>,
    test_dir: Option<&Path>,
) -> Result<Vec<Diagnostic>> {
    run_cargo_fmt(project, true)?;
    check(project, source_dir, test_dir)
}

/// Prints style diagnostics as text or pretty JSON.
///
/// JSON serialization errors are returned as `Err`; text output is written to
/// standard output and does not itself fail.
pub fn print_diagnostics(diagnostics: &[Diagnostic], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(diagnostics)?);
    } else {
        for diagnostic in diagnostics {
            if diagnostic.line == 0 {
                println!("error: {}: {}", diagnostic.path, diagnostic.message);
            } else {
                println!(
                    "error: {}:{}: {}",
                    diagnostic.path, diagnostic.line, diagnostic.message
                );
            }
        }
        if diagnostics.is_empty() {
            println!("Rust style checks passed.");
        } else {
            println!(
                "Rust style checks failed with {} issue(s).",
                diagnostics.len()
            );
        }
    }
    Ok(())
}

/// Formats a project and validates its default workspace style rules.
///
/// This compatibility wrapper uses workspace discovery. Use
/// [`fix_project`] when a migration or caller supplies explicit source/test
/// directories.
pub fn fix(project: &Path, dry_run: bool) -> Result<()> {
    fix_project(project, None, None, dry_run)
}

/// Formats a project and validates the selected style roots.
///
/// The `source_dir` and `test_dir` values are interpreted relative to
/// `project`, matching the CLI options. In dry-run mode no files are changed
/// and the rustfmt command is printed instead. Errors from Cargo, rustfmt, or
/// the style checks are returned as `Err`.
pub fn fix_project(
    project: &Path,
    source_dir: Option<&Path>,
    test_dir: Option<&Path>,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("cargo fmt --all");
        return Ok(());
    }
    run_cargo_fmt(project, false)?;
    let diagnostics = check(project, source_dir, test_dir)?;
    if !diagnostics.is_empty() {
        anyhow::bail!("style checks still report {} issue(s)", diagnostics.len());
    }
    Ok(())
}

fn check_package_roots(
    project: &Path,
    source: &Path,
    tests: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<()> {
    check_root(project, source, RootKind::Source, diagnostics)?;
    let internal_tests = source.join("tests");
    if internal_tests.is_dir() {
        check_internal_test_module(project, source, diagnostics);
        check_root(project, &internal_tests, RootKind::Tests, diagnostics)?;
    }
    if tests != internal_tests {
        check_root(project, tests, RootKind::Tests, diagnostics)?;
    }
    Ok(())
}

fn workspace_roots(project: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(project)
        .output()
        .context("failed to start cargo metadata")?;
    if !output.status.success() {
        anyhow::bail!(
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

fn check_internal_test_module(project: &Path, source: &Path, diagnostics: &mut Vec<Diagnostic>) {
    let internal_tests = source.join("tests");
    let has_module = [source.join("lib.rs"), source.join("main.rs")]
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .any(|text| text.contains("#[cfg(test)]") && text.contains("mod tests"));
    if !has_module {
        let relative = internal_tests
            .strip_prefix(project)
            .unwrap_or(internal_tests.as_path())
            .display()
            .to_string();
        add(
            diagnostics,
            "STYLE012",
            &relative,
            0,
            "src/tests must be connected from the crate root with #[cfg(test)] mod tests;",
        );
    }
}

fn run_cargo_fmt(project: &Path, check_only: bool) -> Result<()> {
    let mut command = Command::new("cargo");
    command.arg("fmt").arg("--all");
    if check_only {
        command.args(["--", "--check"]);
    }
    let description = if check_only {
        "cargo fmt --all -- --check"
    } else {
        "cargo fmt --all"
    };
    let status = command
        .current_dir(project)
        .status()
        .with_context(|| format!("failed to start {description}"))?;
    if !status.success() {
        anyhow::bail!("{description} failed");
    }
    Ok(())
}

fn check_root(
    project: &Path,
    root: &Path,
    kind: RootKind,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
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
        if !entry.file_type().is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("rs")
        {
            continue;
        }
        let text = fs::read_to_string(path)?;
        let relative = path
            .strip_prefix(project)
            .unwrap_or(path)
            .display()
            .to_string();
        match kind {
            RootKind::Source => check_source_file(&relative, path, &text, diagnostics),
            RootKind::Tests => check_test_file(&relative, &text, diagnostics),
        }
    }
    Ok(())
}

fn check_source_file(relative: &str, path: &Path, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if !allowed(text, "coverage-cfg") {
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
    check_imports(relative, text, diagnostics);
    check_aggregation(relative, text, diagnostics);
    check_type_layout(relative, path, text, diagnostics);
}

fn check_test_file(relative: &str, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if !relative.ends_with("_tests.rs") && !relative.ends_with("/mod.rs") {
        add(
            diagnostics,
            "STYLE001",
            relative,
            0,
            "test files should be named '*_tests.rs' or 'mod.rs'",
        );
    }
    if allowed(text, "test-redirect") {
        return;
    }
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

fn check_imports(relative: &str, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if allowed(text, "explicit-imports") {
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
        if let Some(path) = trimmed
            .strip_prefix("use ")
            .and_then(|value| value.strip_suffix(';'))
        {
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
            let group = if path.starts_with("std::")
                || path.starts_with("core::")
                || path.starts_with("alloc::")
            {
                0
            } else if path.starts_with("crate::")
                || path.starts_with("self::")
                || path.starts_with("super::")
            {
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
}

fn check_aggregation(relative: &str, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if !relative.ends_with("/lib.rs") && !relative.ends_with("/mod.rs") {
        return;
    }
    if allowed(text, "aggregation-files") {
        return;
    }
    for (line, value) in text.lines().enumerate() {
        let trimmed = value.trim_start();
        if trimmed.starts_with("//")
            || trimmed.starts_with("pub mod ")
            || trimmed.starts_with("mod ")
            || trimmed.starts_with("pub use ")
            || trimmed.starts_with("use ")
            || trimmed.starts_with("#")
            || trimmed.is_empty()
        {
            continue;
        }
        if [
            "fn ",
            "pub fn ",
            "struct ",
            "pub struct ",
            "enum ",
            "pub enum ",
            "trait ",
            "pub trait ",
            "type ",
            "pub type ",
            "const ",
            "static ",
            "impl ",
            "macro_rules!",
        ]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
        {
            add(
                diagnostics,
                "STYLE009",
                relative,
                line + 1,
                "lib.rs and mod.rs files must only declare modules and re-export items",
            );
        }
    }
}

fn check_type_layout(relative: &str, path: &Path, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let file_name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if matches!(file_name, "lib" | "main" | "mod" | "macros") || allowed(text, "public-type-layout")
    {
        return;
    }
    let Ok(parsed) = syn::parse_file(text) else {
        return;
    };
    let types: Vec<_> = parsed
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(item) if matches!(item.vis, Visibility::Public(_)) => {
                Some(("struct", item.ident.to_string()))
            }
            Item::Enum(item) if matches!(item.vis, Visibility::Public(_)) => {
                Some(("enum", item.ident.to_string()))
            }
            Item::Trait(item) if matches!(item.vis, Visibility::Public(_)) => {
                Some(("trait", item.ident.to_string()))
            }
            _ => None,
        })
        .collect();
    if types.len() > 1 {
        add(
            diagnostics,
            "STYLE010",
            relative,
            0,
            "file contains multiple public top-level types; split them or add a reviewed allowlist entry",
        );
        return;
    }
    if let Some((kind, name)) = types.first() {
        let expected = snake_case(name);
        if expected != file_name && !allowed(text, "type-file-name") {
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

fn allowed(text: &str, rule: &str) -> bool {
    text.lines().any(|line| {
        line.contains("qubit-style: allow all")
            || line.contains(&format!("qubit-style: allow {rule}"))
    })
}

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
    use super::*;

    #[test]
    fn reports_type_file_and_wildcard_import_violations() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("src");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("wrong.rs"),
            "use std::*;\npub struct GoodName;\n",
        )
        .unwrap();
        let diagnostics = check(directory.path(), None, None).unwrap();
        assert!(diagnostics.iter().any(|item| item.code == "STYLE004"));
        assert!(diagnostics.iter().any(|item| item.code == "STYLE011"));
    }

    #[test]
    fn accepts_test_allow_comment() {
        let directory = tempfile::tempdir().unwrap();
        let tests = directory.path().join("tests");
        fs::create_dir_all(&tests).unwrap();
        fs::write(
            tests.join("legacy_tests.rs"),
            "// qubit-style: allow test-redirect\ninclude!(\"legacy_impl.rs\");\n",
        )
        .unwrap();
        assert!(check(directory.path(), None, None).unwrap().is_empty());
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
        fs::write(
            directory.path().join("src/lib.rs"),
            "pub fn value( ) ->i32{1}\n",
        )
        .expect("source");

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
        fs::write(
            directory.path().join("crate/src/lib.rs"),
            "#[cfg(test)] mod tests;\n",
        )
        .expect("crate root");
        fs::write(
            directory.path().join("crate/src/tests/redirect.rs"),
            "include!(\"fixture.rs\");\n",
        )
        .expect("internal test");

        let diagnostics = check(directory.path(), None, None).expect("style check");

        assert!(
            diagnostics.iter().any(|item| {
                item.code == "STYLE002" && item.path == "crate/src/tests/redirect.rs"
            })
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
        fs::write(
            directory.path().join("src/lib.rs"),
            "pub fn value() -> i32 { 1 }\n",
        )
        .expect("Cargo source");
        fs::write(
            directory.path().join("custom-src/wrong.rs"),
            "use std::*;\n",
        )
        .expect("custom source");

        let result = fix_project(
            directory.path(),
            Some(Path::new("custom-src")),
            Some(Path::new("custom-tests")),
            false,
        );

        assert!(result.is_err(), "custom source violations must be checked");
    }
}
