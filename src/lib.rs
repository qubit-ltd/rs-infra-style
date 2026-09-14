// qubit-style: allow all
//! Fixed first-phase Rust style checks used by Qubit Rust projects.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;
use syn::{Item, Visibility};
use walkdir::WalkDir;

/// A machine-readable style diagnostic.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub schema_version: u8,
    pub code: String,
    pub severity: String,
    pub path: String,
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RootKind {
    Source,
    Tests,
}

pub fn check(
    project: &Path,
    source_dir: Option<&Path>,
    test_dir: Option<&Path>,
) -> Result<Vec<Diagnostic>> {
    let source = source_dir
        .map(|path| project.join(path))
        .unwrap_or_else(|| project.join("src"));
    let tests = test_dir
        .map(|path| project.join(path))
        .unwrap_or_else(|| project.join("tests"));
    let mut diagnostics = Vec::new();
    check_root(project, &source, RootKind::Source, &mut diagnostics)?;
    check_root(project, &tests, RootKind::Tests, &mut diagnostics)?;
    Ok(diagnostics)
}

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

pub fn fix(project: &Path, dry_run: bool) -> Result<()> {
    let commands = [
        ("cargo", vec!["fmt", "--all"]),
        (
            "cargo",
            vec![
                "clippy",
                "--fix",
                "--workspace",
                "--allow-dirty",
                "--allow-staged",
                "--all-targets",
                "--all-features",
            ],
        ),
    ];
    for (program, args) in commands {
        if dry_run {
            println!("{} {}", program, args.join(" "));
            continue;
        }
        let status = Command::new(program)
            .args(&args)
            .current_dir(project)
            .status()
            .with_context(|| format!("failed to start {program}"))?;
        if !status.success() {
            anyhow::bail!("{program} {} failed", args.join(" "));
        }
    }
    let diagnostics = check(project, None, None)?;
    if !diagnostics.is_empty() {
        anyhow::bail!("style checks still report {} issue(s)", diagnostics.len());
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
}
