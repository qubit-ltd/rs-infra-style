// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Explicit opt-in source/test pairing policy using the shared parsed index.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use crate::analysis::AnalysisReport;
use crate::analysis::Coverage;
use crate::analysis::Finding;
use crate::style_exceptions::ExceptionConfig;

/// Applies the configured compatibility policy without reparsing any source.
///
/// # Parameters
///
/// * `project` - Project boundary for conventional test directories.
/// * `requested` - Source files included in the current report.
/// * `parsed` - Shared parsed project index.
/// * `roles` - Production and test roles resolved from Cargo and scope
///   overrides.
/// * `roots` - Optional explicit production and test directories.
/// * `exceptions` - Validated project exceptions with their reasons.
/// * `report` - Report receiving policy coverage and proven missing pairs.
pub(crate) fn inspect(
    project: &Path,
    requested: &BTreeSet<String>,
    parsed: &BTreeMap<String, syn::File>,
    roles: &BTreeMap<String, (bool, bool)>,
    roots: (Option<&Path>, Option<&Path>),
    exceptions: &ExceptionConfig,
    report: &mut AnalysisReport,
) {
    let (source_dir, test_dir) = roots;
    let enabled = std::env::var("STYLE_ENFORCE_SOURCE_TEST_PAIRS")
        .is_ok_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"));
    for path in requested {
        let production = roles.get(path).is_some_and(|role| role.0);
        let status = if report.excluded.iter().any(|issue| issue.path == *path) {
            "not_applicable"
        } else if !parsed.contains_key(path)
            || report
                .blocked
                .iter()
                .any(|issue| issue.path.is_empty() || issue.path == *path)
        {
            "blocked"
        } else if !enabled || !production || exceptions.allows("source-test-pair", path) {
            "not_applicable"
        } else {
            "pass"
        };
        let mut coverage = Coverage {
            path: path.clone(),
            rule_id: "TEST-002".into(),
            check_id: "source-test-pair".into(),
            status: status.into(),
            evidence: "Optional STYLE_ENFORCE_SOURCE_TEST_PAIRS policy; semantic test quality remains separate".into(),
        };
        if status == "pass"
            && let Some(ast) = parsed.get(path)
        {
            let file = Path::new(path);
            let name = file.file_name().and_then(|name| name.to_str()).unwrap_or("");
            let alias_only = !ast.items.is_empty() && ast.items.iter().all(|item| matches!(item, syn::Item::Type(_)));
            if !matches!(name, "lib.rs" | "main.rs" | "mod.rs" | "macros.rs")
                && !name.ends_with("_tests.rs")
                && !alias_only
            {
                let source = source_dir.map(Path::to_path_buf).or_else(|| {
                    file.ancestors()
                        .find(|directory| directory.file_name().is_some_and(|name| name == "src"))
                        .map(Path::to_path_buf)
                });
                if let Some(source) = source {
                    let tests = test_dir
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| source.parent().unwrap_or(Path::new("")).join("tests"));
                    let test_name = format!("{}_tests.rs", file.file_stem().unwrap().to_string_lossy());
                    let has_test_tree = project.join(&tests).is_dir();
                    let paired = parsed.keys().any(|candidate| {
                        let candidate = Path::new(candidate);
                        (candidate.starts_with(&tests) || candidate == file.with_file_name(&test_name))
                            && candidate.file_name().is_some_and(|name| name == test_name.as_str())
                    });
                    let parent_pair = file
                        .strip_prefix(&source)
                        .ok()
                        .and_then(Path::parent)
                        .and_then(Path::parent)
                        .into_iter()
                        .flat_map(Path::ancestors)
                        .any(|directory| {
                            let Some(name) = directory.file_name() else {
                                return false;
                            };
                            let parent_name = format!("{}_tests.rs", name.to_string_lossy());
                            parsed.contains_key(
                                &source
                                    .join(directory)
                                    .with_extension("rs")
                                    .to_string_lossy()
                                    .replace('\\', "/"),
                            ) && parsed.keys().any(|candidate| {
                                Path::new(candidate).starts_with(&tests)
                                    && Path::new(candidate)
                                        .file_name()
                                        .is_some_and(|name| name == parent_name.as_str())
                            })
                        });
                    if has_test_tree && !paired && !parent_pair {
                        coverage.status = "fail".into();
                        report.diagnostics.push(Finding {
                            rule_id: "TEST-002".into(),
                            check_id: "source-test-pair".into(),
                            severity: "error".into(),
                            path: path.clone(),
                            line: 1,
                            column: 1,
                            message: format!("missing corresponding test file '{}/{}'", tests.display(), test_name),
                            evidence: "Explicit source/test pairing policy enabled with no matching parsed test file"
                                .into(),
                            help:
                                "Add the corresponding test module or record an exact-path exception with its reason."
                                    .into(),
                            related_locations: Vec::new(),
                        });
                    }
                }
            }
        }
        report.coverage.push(coverage);
    }
}
