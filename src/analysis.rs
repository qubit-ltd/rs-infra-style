// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Read-only deterministic analysis and explicit semantic coverage.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use sha2::Digest;
use sha2::Sha256;
use walkdir::WalkDir;

use crate::analysis_project;
use crate::analysis_rules;
use crate::style_exceptions::ExceptionConfig;

mod internal;
use internal::header_policy::HeaderPolicy;

/// Normative rules covered by the analysis protocol.
pub const RULE_IDS: &[&str] = &[
    "ORG-001",
    "ORG-002",
    "ORG-003",
    "IMPORT-001",
    "DOC-001",
    "DOC-002",
    "DOC-003",
    "TEST-001",
    "TEST-002",
    "TEST-003",
    "TEST-004",
    "BENCH-001",
    "FUZZ-001",
    "METHOD-001",
    "ATTR-001",
    "ATTR-003",
    "COMP-001",
    "README-001",
    "VERIFY-001",
];

/// Computes SHA-256 of arbitrary bytes.
///
/// # Parameters
///
/// * `bytes` - Exact input bytes whose identity is bound to the report.
pub(crate) fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

/// Returns the authority digest compiled into this analyzer.
///
/// Uses the same sorted JSON-with-spaces encoding as the review snapshot
/// loader.
#[must_use]
pub fn supported_standards_digest() -> String {
    let documents = [
        ("README.md", include_str!("../doc/style-standard/README.md")),
        (
            "alignment-design.md",
            include_str!("../doc/style-standard/alignment-design.md"),
        ),
        ("consumption.md", include_str!("../doc/style-standard/consumption.md")),
        (
            "development-validation.md",
            include_str!("../doc/style-standard/development-validation.md"),
        ),
        (
            "readme-structure.md",
            include_str!("../doc/style-standard/readme-structure.md"),
        ),
        (
            "rust-coding-standard.md",
            include_str!("../doc/style-standard/rust-coding-standard.md"),
        ),
        (
            "rust-file-header.md",
            include_str!("../doc/style-standard/rust-file-header.md"),
        ),
    ];
    snapshot_map_digest(
        documents
            .into_iter()
            .map(|(name, text)| (name.to_owned(), digest(text)))
            .collect(),
    )
}

/// Encodes the sorted document digest map exactly like the Python snapshot
/// loader.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn snapshot_map_digest(files: BTreeMap<String, String>) -> String {
    let entries: Vec<_> = files
        .iter()
        .map(|(name, value)| {
            format!(
                "{}: {}",
                serde_json::to_string(name).unwrap(),
                serde_json::to_string(value).unwrap()
            )
        })
        .collect();
    digest(format!("{{{}}}", entries.join(", ")))
}

/// Validates and hashes every ordinary document in an immutable standard
/// snapshot.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn standard_digest(directory: &Path) -> Result<String> {
    let mut files = BTreeMap::new();
    for entry in WalkDir::new(directory).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            bail!("standard snapshot contains a symbolic link: {}", entry.path().display());
        }
        if entry.file_type().is_file() {
            let relative = entry
                .path()
                .strip_prefix(directory)?
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(relative, digest(fs::read(entry.path())?));
        } else if !entry.file_type().is_dir() {
            bail!("standard snapshot contains a non-regular entry");
        }
    }
    Ok(snapshot_map_digest(files))
}

/// Identifies generated, third-party and build directory components excluded by
/// policy.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
pub(crate) fn excluded(path: &str) -> bool {
    path.split('/').any(|part| {
        matches!(
            part,
            "vendor" | "vendored" | "generated" | "target" | ".git" | ".worktrees"
        )
    })
}

/// Rejects source paths that could escape the project or violate the list
/// protocol.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn validate_relative(path: &str) -> Result<()> {
    if path.is_empty()
        || path.contains('\\')
        || path.split('/').any(|part| part.is_empty())
        || Path::new(path)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || !path.ends_with(".rs")
    {
        bail!("invalid project-relative Rust path: {path}");
    }
    Ok(())
}

/// Rejects symbolic links in every existing component before consuming input.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
pub(crate) fn validate_no_symlink(project: &Path, relative: &str) -> Result<()> {
    let mut ancestor = project.to_path_buf();
    for part in Path::new(relative).components() {
        ancestor.push(part.as_os_str());
        match fs::symlink_metadata(&ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("symbolic-link input is not supported: {relative}")
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Reads ordinary project-local bytes after validating every path component.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
pub(crate) fn read_safe(project: &Path, relative: &str) -> Result<Vec<u8>> {
    validate_no_symlink(project, relative)?;
    let path = project.join(relative);
    if !fs::symlink_metadata(&path)?.file_type().is_file() {
        bail!("input is not a regular file: {relative}");
    }
    fs::read(path).with_context(|| format!("cannot read {relative}"))
}

/// Analyzes an exact source list without formatting, building, or changing
/// files.
///
/// # Parameters
///
/// * `project` - Project root containing the exact source list.
/// * `files` - Project-relative source paths, including non-default targets.
/// * `standards` - Optional immutable authority directory to verify.
///
/// # Returns
///
/// Returns separate mechanical diagnostics, semantic coverage and limitations.
///
/// # Errors
///
/// Returns an error for malformed input lists or an inaccessible project root.
/// Source/parse/tool/standard failures are represented in the report as
/// blocked.
pub fn analyze(project: &Path, files: &[String], standards: Option<&Path>) -> Result<AnalysisReport> {
    analyze_scoped(project, files, standards, None, None, false)
}

/// Analyzes the same rules with explicit compatibility source and test roots.
///
/// # Parameters
///
/// * `project` - Repository root used for sources and context.
/// * `files` - Exact requested source file list.
/// * `standards` - Optional immutable standard snapshot.
/// * `source_dir` - Optional explicit production root.
/// * `test_dir` - Optional explicit test root.
/// * `check_formatting` - Compares full-file formatting for the mechanical
///   gate.
pub(crate) fn analyze_scoped(
    project: &Path,
    files: &[String],
    standards: Option<&Path>,
    source_dir: Option<&Path>,
    test_dir: Option<&Path>,
    check_formatting: bool,
) -> Result<AnalysisReport> {
    if fs::symlink_metadata(project)?.file_type().is_symlink() {
        bail!("symbolic-link project root is not supported");
    }
    let project = project.canonicalize()?;
    let requested: BTreeSet<_> = files.iter().cloned().collect();
    for file in &requested {
        validate_relative(file)?;
    }
    let supported = supported_standards_digest();
    let mut report = AnalysisReport {
        schema_version: 1,
        analyzer_version: env!("CARGO_PKG_VERSION").to_owned(),
        standards_digest: supported.clone(),
        supported_standards_digest: supported,
        input_digest: String::new(),
        files: requested.iter().cloned().collect(),
        diagnostics: Vec::new(),
        repository_diagnostics: Vec::new(),
        repository_coverage: Vec::new(),
        coverage: Vec::new(),
        blocked: Vec::new(),
        excluded: Vec::new(),
        context_files: Vec::new(),
    };
    if let Some(directory) = standards {
        match standard_digest(directory) {
            Ok(actual) => {
                report.standards_digest = actual;
                if report.standards_digest != report.supported_standards_digest {
                    report.blocked.push(AnalysisIssue { path: String::new(), reason: "standard snapshot is not supported by this analyzer; rebuild/release against the selected authority".into() });
                }
            }
            Err(error) => report.blocked.push(AnalysisIssue {
                path: String::new(),
                reason: format!("cannot validate standard snapshot: {error:#}"),
            }),
        }
    }
    let header_policy = match HeaderPolicy::load(&project) {
        Ok(policy) => policy,
        Err(error) => {
            report.blocked.push(AnalysisIssue {
                path: String::new(),
                reason: format!("invalid .infra/style/analysis.toml: {error:#}"),
            });
            HeaderPolicy::default()
        }
    };
    let exception_path = project.join(".infra/style/exceptions.toml");
    let exception_config = if fs::symlink_metadata(&exception_path).is_ok() {
        read_safe(&project, ".infra/style/exceptions.toml").and_then(|_| ExceptionConfig::load(&project))
    } else {
        ExceptionConfig::load(&project)
    };
    let exceptions = match exception_config {
        Ok(config) => config,
        Err(error) => {
            report.blocked.push(AnalysisIssue {
                path: String::new(),
                reason: format!("invalid .infra/style/exceptions.toml configuration: {error:#}"),
            });
            ExceptionConfig::default()
        }
    };
    // Read context as well as requested sources, but parse each source only once.
    let mut candidates = requested.clone();
    let mut context_paths = BTreeSet::new();
    match Command::new("git")
        .args(["-C"])
        .arg(&project)
        .args(["ls-files", "--cached", "--others", "--exclude-standard", "-z"])
        .output()
    {
        Ok(output) if output.status.success() => {
            for entry in output.stdout.split(|byte| *byte == 0).filter(|entry| !entry.is_empty()) {
                match std::str::from_utf8(entry) {
                    Ok(path) => {
                        if fs::symlink_metadata(project.join(path))
                            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
                        {
                            continue;
                        }
                        context_paths.insert(path.to_owned());
                    }
                    Err(error) => report.blocked.push(AnalysisIssue {
                        path: String::new(),
                        reason: format!("non-UTF-8 context path: {error}"),
                    }),
                }
            }
        }
        _ => {
            // Standalone source trees need no Git repository; keep the fallback bounded.
            for entry in WalkDir::new(&project)
                .follow_links(false)
                .into_iter()
                .filter_entry(|entry| {
                    entry.path().strip_prefix(&project).ok().is_none_or(|path| {
                        !excluded(&path.to_string_lossy())
                            && !path.components().any(|part| part.as_os_str() == ".codex")
                    })
                })
            {
                match entry {
                    Ok(entry) if entry.file_type().is_file() => {
                        context_paths.insert(
                            entry
                                .path()
                                .strip_prefix(&project)?
                                .to_string_lossy()
                                .replace('\\', "/"),
                        );
                    }
                    Ok(_) => {}
                    Err(error) => report.blocked.push(AnalysisIssue {
                        path: String::new(),
                        reason: format!("context discovery failed: {error}"),
                    }),
                }
            }
        }
    }
    // Infrastructure config is often ignored by Git, but it is authoritative
    // context.
    for relative in [".infra/style", ".infra/tools"] {
        let directory = project.join(relative);
        if let Err(error) = validate_no_symlink(&project, relative) {
            report.blocked.push(AnalysisIssue {
                path: String::new(),
                reason: format!("configuration directory is not supported: {error:#}"),
            });
            continue;
        }
        if directory.is_dir() {
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                if entry.file_type()?.is_file() && entry.path().extension().is_some_and(|extension| extension == "toml")
                {
                    context_paths.insert(format!("{relative}/{}", entry.file_name().to_string_lossy()));
                }
            }
        }
    }
    // Cargo role/edition discovery consumes enclosing manifests even when Git
    // ignores them.
    let mut manifests = BTreeSet::new();
    for source in candidates
        .iter()
        .chain(context_paths.iter().filter(|path| path.ends_with(".rs")))
    {
        let absolute = project.join(source);
        for directory in absolute
            .parent()
            .into_iter()
            .flat_map(Path::ancestors)
            .take_while(|directory| directory.starts_with(&project))
        {
            let manifest = directory.join("Cargo.toml");
            if fs::symlink_metadata(&manifest).is_ok() {
                manifests.insert(manifest.strip_prefix(&project)?.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    context_paths.extend(manifests);
    for path in context_paths {
        if excluded(&path) || path.split('/').any(|part| part == ".codex") {
            continue;
        }
        if path.ends_with(".rs") {
            candidates.insert(path);
        } else if path.ends_with("Cargo.toml")
            || path.starts_with(".infra/style/")
            || path == ".infra/tools/defaults.toml"
            || path.starts_with("README")
        {
            match read_safe(&project, &path) {
                Ok(bytes) => {
                    if path.ends_with("Cargo.toml")
                        && let Err(error) = std::str::from_utf8(&bytes)
                            .map_err(|error| error.to_string())
                            .and_then(|text| toml::from_str::<toml::Value>(text).map_err(|error| error.to_string()))
                    {
                        report.blocked.push(AnalysisIssue {
                            path: path.clone(),
                            reason: format!("Cargo manifest cannot be parsed: {error}"),
                        });
                    }
                    report.context_files.push(ContextFile {
                        path,
                        digest: digest(bytes),
                    });
                }
                Err(error) => report.blocked.push(AnalysisIssue {
                    path,
                    reason: format!("cannot read configuration: {error:#}"),
                }),
            }
        }
    }
    let mut sources = BTreeMap::new();
    let mut parsed = BTreeMap::new();
    for path in candidates {
        if excluded(&path) {
            if requested.contains(&path) {
                report.excluded.push(AnalysisIssue {
                    path,
                    reason: "generated or third-party directory excluded by the source policy".into(),
                });
            }
            continue;
        }
        match read_safe(&project, &path) {
            Ok(bytes) => {
                report.context_files.push(ContextFile {
                    path: path.clone(),
                    digest: digest(&bytes),
                });
                match String::from_utf8(bytes) {
                    Ok(text) => {
                        match syn::parse_file(&text) {
                            Ok(ast) => {
                                parsed.insert(path.clone(), ast);
                            }
                            Err(error) if requested.contains(&path) => report.blocked.push(AnalysisIssue {
                                path: path.clone(),
                                reason: format!("Rust parser rejected source: {error}"),
                            }),
                            Err(error) => {
                                for requested_path in &requested {
                                    report.diagnostics.push(Finding {rule_id:"ORG-002".into(),check_id:"unparsed-context".into(),severity:"info".into(),path:requested_path.clone(),line:1,column:1,message:"Context source could not be parsed; ownership and module conclusions require semantic review".into(),evidence:format!("{path}: {error}"),help:"Inspect this context explicitly; do not infer complete ownership or module coverage".into(),related_locations:vec![Location {path:path.clone(),line:1,column:1}]});
                                }
                            }
                        }
                        sources.insert(path, text);
                    }
                    Err(error) => report.blocked.push(AnalysisIssue {
                        path,
                        reason: format!("source is not UTF-8: {error}"),
                    }),
                }
            }
            Err(error) => report.blocked.push(AnalysisIssue {
                path,
                reason: format!("cannot read source: {error:#}"),
            }),
        }
    }
    let mut roles = analysis_project::source_roles(&project, &sources, &parsed);
    if source_dir.is_some() || test_dir.is_some() {
        let source = source_dir.unwrap_or(Path::new("src"));
        let tests = test_dir.unwrap_or(Path::new("tests"));
        for path in sources.keys() {
            let path_ref = Path::new(path);
            if path_ref.starts_with(source) || path_ref.starts_with(tests) {
                let test = path_ref.starts_with(tests) || path_ref.starts_with(source.join("tests"));
                roles.insert(path.clone(), (!test && path_ref.starts_with(source), test));
            }
        }
    }
    if report.standards_digest == report.supported_standards_digest
        && !report.blocked.iter().any(|issue| issue.path.is_empty())
    {
        for path in &requested {
            if let (Some(text), Some(ast)) = (sources.get(path), parsed.get(path)) {
                check_header(path, text, &header_policy, &mut report);
                let (production, tests) = roles.get(path).copied().unwrap_or((false, false));
                report.diagnostics.extend(analysis_rules::inspect_with_context(
                    path,
                    text,
                    ast,
                    &exceptions,
                    Some(production),
                    Some(tests),
                ));
            }
        }
        analysis_project::inspect(
            &project,
            &requested,
            &sources,
            &parsed,
            &roles,
            &mut report,
            (&exceptions, check_formatting),
        );
        crate::analysis_pairs::inspect(
            &project,
            &requested,
            &parsed,
            &roles,
            (source_dir, test_dir),
            &exceptions,
            &mut report,
        );
    }
    for path in &requested {
        record_coverage(
            path,
            &parsed,
            roles.get(path).copied().unwrap_or((false, false)),
            &mut report,
        );
    }
    for finding in &mut report.diagnostics {
        if finding.severity == "review" {
            finding.severity = "info".into();
        }
    }
    report.context_files.sort_by(|left, right| left.path.cmp(&right.path));
    report.context_files.dedup_by(|left, right| left.path == right.path);
    report.diagnostics.sort_by(|a, b| {
        (&a.path, a.line, a.column, &a.rule_id, &a.check_id).cmp(&(&b.path, b.line, b.column, &b.rule_id, &b.check_id))
    });
    report.input_digest = digest(serde_json::to_vec(&(
        &report.analyzer_version,
        &report.standards_digest,
        &report.files,
        &report.context_files,
        std::env::var("RS_INFRA_STYLE_TOOLCHAIN").ok(),
        std::env::var("STYLE_ENFORCE_SOURCE_TEST_PAIRS").ok(),
        source_dir,
        test_dir,
        check_formatting,
    ))?);
    Ok(report)
}

/// Records the exact accepted header decision and the authority or project
/// override used.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn check_header(path: &str, text: &str, policy: &HeaderPolicy, report: &mut AnalysisReport) {
    let valid = policy.accepts(text);
    report.coverage.push(Coverage {
        path: path.into(),
        rule_id: "DOC-001".into(),
        check_id: "header-template".into(),
        status: if valid { "pass" } else { "fail" }.into(),
        evidence: policy.evidence.clone(),
    });
    if !valid {
        report.diagnostics.push(Finding { rule_id: "DOC-001".into(), check_id: "header-template".into(), severity: "error".into(), path: path.into(), line: 1, column: 1,
            message: "Rust source must begin with the accepted seven-line copyright and license header".into(), evidence:format!("{}; observed header:\n{}",policy.evidence,text.lines().take(7).collect::<Vec<_>>().join("\n")), help: "Repair an existing nonconforming block in place; preserve an accepted copyright year; add the template only if no header exists".into(), related_locations: Vec::new() });
    }
}

/// Combines proved findings, unresolved syntax and source roles into explicit
/// subcheck coverage.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn record_coverage(path: &str, parsed: &BTreeMap<String, syn::File>, role: (bool, bool), report: &mut AnalysisReport) {
    let excluded = report.excluded.iter().any(|issue| issue.path == path);
    let blocked = report
        .blocked
        .iter()
        .any(|issue| issue.path.is_empty() || issue.path == path)
        || (!excluded && !parsed.contains_key(path));
    let unexpanded = report.diagnostics.iter().any(|finding| {
        finding.path == path
            && matches!(
                finding.check_id.as_str(),
                "unexpanded-macro" | "conditional-configuration"
            )
    });
    for rule in RULE_IDS {
        let status = if blocked {
            "blocked"
        } else if excluded || matches!(*rule, "README-001" | "VERIFY-001") {
            "not_applicable"
        } else if matches!(*rule, "DOC-001" | "TEST-003" | "TEST-004") {
            "pass"
        } else {
            "needs_semantic_review"
        };
        let (_, applicable_test) = role;
        let status = if !blocked && !excluded && unexpanded && *rule == "TEST-004" {
            "needs_semantic_review"
        } else if !blocked
            && !excluded
            && (matches!(*rule, "TEST-003" | "TEST-004") && !applicable_test
                || *rule == "TEST-003" && !crate::analysis_rules::test_naming_scope(path))
        {
            "not_applicable"
        } else {
            status
        };
        let failed = report
            .diagnostics
            .iter()
            .any(|item| item.path == path && item.rule_id == *rule && item.severity == "error");
        report.coverage.push(Coverage {
            path: path.into(),
            rule_id: (*rule).into(),
            check_id: "rule-coverage".into(),
            status: if failed {
                "fail"
            } else if status == "pass"
                && report
                    .diagnostics
                    .iter()
                    .any(|finding| finding.path == path && finding.rule_id == *rule && finding.severity != "error")
            {
                "needs_semantic_review"
            } else {
                status
            }
            .into(),
            evidence: match status {
                "blocked" => "Source, authority, or analysis context could not be fully inspected",
                "not_applicable" => {
                    "Outside this source file's role; repository checks and verification belong to the coordinator"
                }
                "pass" => "Deterministic source subchecks completed; this does not certify other semantic rules",
                _ => "AST/source evidence collected; semantic requirements still require a full LLM review",
            }
            .into(),
        });
    }
    const SUBCHECKS: &[(&str, &str, &str)] = &[
        ("DOC-001", "header-template", "all"),
        ("IMPORT-001", "single-object-import", "all"),
        ("IMPORT-001", "import-groups", "all"),
        ("IMPORT-001", "unused-imports-allow", "all"),
        ("ORG-001", "one-type-per-file", "production"),
        ("ORG-001", "type-file-name", "production"),
        ("ORG-001", "empty-type-body", "production"),
        ("ORG-001", "builder-build-receiver", "production"),
        ("DOC-002", "rustdoc-presence", "production"),
        ("DOC-003", "documentation-attribute-order", "production"),
        ("DOC-003", "required-documentation-sections", "production"),
        ("DOC-003", "public-type-examples", "semantic"),
        ("TEST-002", "test-function-name", "all"),
        ("TEST-003", "test-file-name", "test-naming"),
        ("TEST-004", "test-source-redirection", "test"),
        ("ATTR-001", "trait-inline-without-body", "all"),
        ("ATTR-001", "double-must-use-self", "production"),
        ("ATTR-003", "coverage-cfg", "production"),
        ("ORG-002", "inherent-owner", "semantic"),
        ("ORG-003", "aggregation-items", "semantic"),
        ("TEST-002", "test-reachability", "semantic"),
        ("METHOD-001", "method-inventory", "semantic"),
        ("COMP-001", "complexity-evidence", "semantic"),
    ];
    let (production, tests) = role;
    for (rule, check, scope) in SUBCHECKS {
        if report
            .coverage
            .iter()
            .any(|item| item.path == path && item.rule_id == *rule && item.check_id == *check)
        {
            continue;
        }
        let findings: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|finding| finding.path == path && finding.rule_id == *rule && finding.check_id == *check)
            .collect();
        let status = if blocked {
            "blocked"
        } else if excluded
            || (*scope == "production" && !production)
            || (*scope == "test" && !tests)
            || (*scope == "test-naming" && (!tests || !crate::analysis_rules::test_naming_scope(path)))
        {
            "not_applicable"
        } else if findings.iter().any(|finding| finding.severity == "error") {
            "fail"
        } else if !findings.is_empty()
            || *scope == "semantic"
            || (unexpanded && *check != "header-template" && *check != "test-file-name")
        {
            "needs_semantic_review"
        } else {
            "pass"
        };
        report.coverage.push(Coverage {
            path: path.into(),
            rule_id: (*rule).into(),
            check_id: (*check).into(),
            status: status.into(),
            evidence: match status {
                "pass" => "Applicable AST/source subcheck traversed without a proved violation",
                "not_applicable" => "Subcheck is outside this source role or the source is explicitly excluded",
                "needs_semantic_review" => {
                    "Conditional, unresolved or semantic requirements remain; see available diagnostic evidence"
                }
                "blocked" => "Analysis could not establish this subcheck's result",
                _ => "See the corresponding proved diagnostic and source location",
            }
            .into(),
        });
    }
    let checks: BTreeSet<_> = report
        .diagnostics
        .iter()
        .filter(|finding| finding.path == path)
        .map(|finding| {
            (
                finding.rule_id.clone(),
                finding.check_id.clone(),
                finding.severity.clone(),
            )
        })
        .collect();
    for (rule, check, severity) in checks {
        if report
            .coverage
            .iter()
            .any(|item| item.path == path && item.rule_id == rule && item.check_id == check)
        {
            continue;
        }
        report.coverage.push(Coverage {
            path: path.into(),
            rule_id: rule,
            check_id: check,
            status: if severity == "error" {
                "fail"
            } else {
                "needs_semantic_review"
            }
            .into(),
            evidence: "See the corresponding diagnostic evidence".into(),
        });
    }
}

mod location;
pub use location::Location;

mod finding;
pub use finding::Finding;

mod coverage;
pub use coverage::Coverage;

mod analysis_issue;
pub use analysis_issue::AnalysisIssue;

mod context_file;
pub use context_file::ContextFile;

mod analysis_report;
pub use analysis_report::AnalysisReport;

/// Adds opt-in repository README structure checks to a source report.
///
/// # Parameters
///
/// * `project` - Root containing the README pair and Cargo project metadata.
/// * `report` - Analysis report whose separate repository fields are populated.
///
/// # Errors
///
/// Returns an error if the project root cannot be resolved. Per-file failures
/// are retained in the report and affect its exit code.
pub fn analyze_repository_readme(project: &Path, report: &mut AnalysisReport) -> Result<()> {
    let project = project.canonicalize()?;
    crate::analysis_readme::inspect(&project, report);
    Ok(())
}
