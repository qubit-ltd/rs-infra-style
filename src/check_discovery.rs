// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Project-owned source discovery independent of Cargo default members.

use std::collections::BTreeSet;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use walkdir::WalkDir;

use crate::analysis;

/// Discovers all owned Rust inputs, narrowing only explicitly selected roots.
///
/// # Parameters
///
/// * `project` - Repository or standalone tree to inspect.
/// * `source_dir` - Optional production root relative to the project.
/// * `test_dir` - Optional test root relative to the project.
///
/// # Errors
///
/// Returns an error for unsafe roots, unreadable directories or non-UTF-8
/// paths.
pub(crate) fn discover(project: &Path, source_dir: Option<&Path>, test_dir: Option<&Path>) -> Result<Vec<String>> {
    let explicit = source_dir.is_some() || test_dir.is_some();
    let source = source_dir.unwrap_or(Path::new("src"));
    let siblings = source.parent().unwrap_or(Path::new(""));
    let extra_roots = [
        siblings.join("benches"),
        siblings.join("fuzz"),
        siblings.join("examples"),
    ];
    let roots = [source, test_dir.unwrap_or(Path::new("tests"))];
    if explicit {
        for root in roots {
            if root.components().any(|part| !matches!(part, Component::Normal(_))) {
                bail!("source and test roots must be project-relative directories");
            }
            analysis::validate_no_symlink(project, root.to_str().context("non-UTF-8 source root")?)?;
        }
    }
    let mut files = BTreeSet::new();
    let mut git_owned = false;
    let output = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["ls-files", "--cached", "--others", "--exclude-standard", "-z"])
        .output();
    if let Ok(output) = output
        && output.status.success()
    {
        git_owned = true;
        for entry in output.stdout.split(|byte| *byte == 0).filter(|entry| !entry.is_empty()) {
            let path = std::str::from_utf8(entry).context("non-UTF-8 Git source path")?;
            if fs::symlink_metadata(project.join(path)).is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            {
                continue;
            }
            files.insert(path.to_owned());
        }
    } else {
        for entry in WalkDir::new(project)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                entry.path().strip_prefix(project).is_ok_and(|path| {
                    !analysis::excluded(&path.to_string_lossy())
                        && !path.components().any(|part| part.as_os_str() == ".codex")
                })
            })
        {
            let entry = entry?;
            if entry.file_type().is_file() || entry.file_type().is_symlink() {
                files.insert(
                    entry
                        .path()
                        .strip_prefix(project)?
                        .to_str()
                        .context("non-UTF-8 source path")?
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut package_roots = BTreeSet::new();
    if !git_owned && !explicit && project.join("Cargo.toml").is_file() {
        for path in files
            .iter()
            .filter(|path| Path::new(path).file_name().is_some_and(|name| name == "Cargo.toml"))
        {
            let Ok(contents) = analysis::read_safe(project, path) else {
                continue;
            };
            let Some(manifest) = std::str::from_utf8(&contents)
                .ok()
                .and_then(|contents| toml::from_str::<toml::Value>(contents).ok())
            else {
                continue;
            };
            if manifest.get("package").is_none() {
                continue;
            }
            let package = Path::new(path).parent().unwrap_or(Path::new(""));
            package_roots
                .extend(["src", "tests", "benches", "fuzz", "examples"].map(|directory| package.join(directory)));
            let library = manifest.get("lib").into_iter();
            let binaries = manifest
                .get("bin")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten();
            for target in library.chain(binaries) {
                let Some(target_path) = target.get("path").and_then(toml::Value::as_str) else {
                    continue;
                };
                let target = Path::new(target_path);
                if target
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
                {
                    bail!("Cargo source target must stay inside its package: {path}: {target_path}");
                }
                let target: std::path::PathBuf = target
                    .components()
                    .filter_map(|part| {
                        if let Component::Normal(name) = part {
                            Some(name)
                        } else {
                            None
                        }
                    })
                    .collect();
                package_roots.insert(package.join(target.parent().unwrap_or(Path::new(""))));
            }
        }
    }
    Ok(files
        .into_iter()
        .filter(|path| {
            path.ends_with(".rs")
                && !analysis::excluded(path)
                && !path.split('/').any(|part| part == ".codex")
                && (git_owned
                    || explicit
                    || (!project.join("Cargo.toml").is_file()
                        && (roots.iter().any(|root| Path::new(path).starts_with(root))
                            || extra_roots.iter().any(|root| Path::new(path).starts_with(root))))
                    || (project.join("Cargo.toml").is_file()
                        && package_roots.iter().any(|root| Path::new(path).starts_with(root))))
                && (!explicit
                    || roots.iter().any(|root| Path::new(path).starts_with(root))
                    || extra_roots.iter().any(|root| Path::new(path).starts_with(root)))
        })
        .collect())
}
