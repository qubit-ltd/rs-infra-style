// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Project context, ownership, and module-reachability evidence.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use quote::ToTokens;
use syn::spanned::Spanned;
use syn::visit::Visit;

use crate::analysis::AnalysisIssue;
use crate::analysis::AnalysisReport;
use crate::analysis::Coverage;
use crate::analysis::Finding;
use crate::analysis::Location;

mod internal;
use internal::complexity::Complexity;
use internal::import_inventory::ImportInventory;
use internal::index::Index;
use internal::method_inventory::MethodInventory;

/// Constructs a located diagnostic while distinguishing proved errors from
/// review signals.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn finding(
    path: &str,
    rule: &str,
    check: &str,
    line: usize,
    message: &str,
    evidence: String,
    severity: &str,
) -> Finding {
    Finding {
        path: path.into(),
        rule_id: rule.into(),
        check_id: check.into(),
        line,
        column: 1,
        severity: severity.into(),
        message: message.into(),
        evidence,
        help: "Review the associated source context and preserve the existing API and behavior".into(),
        related_locations: Vec::new(),
    }
}

/// Finds the nearest enclosing Cargo manifest within the project boundary.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn package_root(project: &Path, path: &str) -> PathBuf {
    let mut current = project.join(path).parent().unwrap().to_path_buf();
    while current.starts_with(project) {
        if current.join("Cargo.toml").is_file() {
            return current;
        }
        if !current.pop() {
            break;
        }
    }
    project.to_path_buf()
}

/// Reads UTF-8 project context without following links or escaping the project.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn safe_text(project: &Path, path: impl AsRef<Path>) -> std::io::Result<String> {
    let path = path.as_ref();
    let relative = path
        .strip_prefix(project)
        .map_err(std::io::Error::other)?
        .to_string_lossy()
        .replace('\\', "/");
    let bytes =
        crate::analysis::read_safe(project, &relative).map_err(|error| std::io::Error::other(error.to_string()))?;
    String::from_utf8(bytes).map_err(std::io::Error::other)
}

/// Propagates explicit Cargo target roles through the parsed module graph.
///
/// # Parameters
///
/// * `project` - Project boundary for Cargo target discovery.
/// * `sources` - Read source text indexed by relative path.
/// * `parsed` - Source ASTs parsed once for this analysis.
pub(crate) fn source_roles(
    project: &Path,
    sources: &BTreeMap<String, String>,
    parsed: &BTreeMap<String, syn::File>,
) -> BTreeMap<String, (bool, bool)> {
    let mut graph = BTreeMap::new();
    let mut test_edges = BTreeSet::new();
    for (path, ast) in parsed {
        let base = if target_root(project, path) {
            Some(Path::new(path).parent().unwrap_or(Path::new("")))
        } else {
            None
        };
        module_edges(path, &ast.items, sources, &mut graph, base);
        collect_test_edges(path, &ast.items, sources, &mut test_edges, base, false);
    }
    let mut propagated: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for path in parsed.keys() {
        if !target_root(project, path) {
            continue;
        }
        let role = source_role(project, path);
        let mut pending = vec![(path.clone(), role)];
        let mut seen = BTreeSet::new();
        while let Some((current, mut role)) = pending.pop() {
            if !seen.insert((current.clone(), role)) {
                continue;
            }
            if parsed.get(&current).is_some_and(|ast| test_attributes(&ast.attrs)) {
                role = (false, true);
            }
            let previous = propagated.entry(current.clone()).or_insert((false, false));
            previous.0 |= role.0;
            previous.1 |= role.1;
            for child in graph.get(&current).into_iter().flatten() {
                let child_role = if test_edges.contains(&(current.clone(), child.clone())) {
                    (false, true)
                } else {
                    role
                };
                pending.push((child.clone(), child_role));
            }
        }
    }
    for path in parsed.keys() {
        propagated
            .entry(path.clone())
            .or_insert_with(|| source_role(project, path));
    }
    for role in propagated.values_mut() {
        if role.0 {
            role.1 = false;
        }
    }
    propagated
}

/// Proves that an attribute set is restricted to test configurations.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn test_attributes(attrs: &[syn::Attribute]) -> bool {
    /// Checks whether a cfg expression necessarily requires the test flag.
    ///
    /// # Parameters
    ///
    /// Arguments provide the source, project context or analysis state
    /// described by this operation.
    fn requires_test(meta: &syn::Meta) -> bool {
        if meta.path().is_ident("test") {
            return true;
        }
        let syn::Meta::List(list) = meta else {
            return false;
        };
        use syn::parse::Parser;
        let children = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
            .parse2(list.tokens.clone())
            .map(|items| items.into_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        if meta.path().is_ident("all") {
            children.iter().any(requires_test)
        } else if meta.path().is_ident("cfg") || meta.path().is_ident("any") {
            !children.is_empty() && children.iter().all(requires_test)
        } else {
            false
        }
    }
    attrs.iter().any(|attr| requires_test(&attr.meta))
}

/// Marks external module edges whose declarations are restricted to test
/// contexts.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn collect_test_edges(
    path: &str,
    items: &[syn::Item],
    sources: &BTreeMap<String, String>,
    edges: &mut BTreeSet<(String, String)>,
    base: Option<&Path>,
    test_context: bool,
) {
    for item in items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        let tests = test_context || test_attributes(&module.attrs);
        if tests {
            let mut children = BTreeMap::new();
            module_edges(path, std::slice::from_ref(item), sources, &mut children, base);
            for child in children.into_values().flatten() {
                edges.insert((path.into(), child));
            }
        } else if let Some((_, items)) = &module.content {
            let directory = base
                .map(Path::to_path_buf)
                .unwrap_or_else(|| {
                    let file = Path::new(path);
                    let parent = file.parent().unwrap_or(Path::new(""));
                    if matches!(
                        file.file_stem().and_then(|part| part.to_str()),
                        Some("lib" | "main" | "mod")
                    ) {
                        parent.to_path_buf()
                    } else {
                        parent.join(file.file_stem().unwrap())
                    }
                })
                .join(module.ident.to_string());
            collect_test_edges(path, items, sources, edges, Some(&directory), false);
        }
    }
}

/// Recognizes conventional and explicitly declared Cargo target entry files.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn target_root(project: &Path, path: &str) -> bool {
    let root = package_root(project, path);
    let prefix = root.strip_prefix(project).unwrap_or(Path::new(""));
    let relative = Path::new(path)
        .strip_prefix(prefix)
        .unwrap_or(Path::new(path))
        .to_string_lossy()
        .replace('\\', "/");
    if matches!(relative.as_str(), "src/lib.rs" | "src/main.rs") {
        return true;
    }
    let manifest = safe_text(project, root.join("Cargo.toml"))
        .ok()
        .and_then(|text| toml::from_str::<toml::Value>(&text).ok());
    if [
        ("tests/", "autotests"),
        ("benches/", "autobenches"),
        ("examples/", "autoexamples"),
        ("src/bin/", "autobins"),
    ]
    .iter()
    .any(|(prefix, flag)| {
        let automatic = manifest
            .as_ref()
            .and_then(|manifest| manifest.get("package"))
            .and_then(|package| package.get(*flag))
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        automatic
            && relative.strip_prefix(prefix).is_some_and(|rest| {
                !rest.contains('/') || (rest.matches('/').count() == 1 && rest.ends_with("/main.rs"))
            })
    }) {
        return true;
    }
    if let Some(manifest) = manifest.as_ref() {
        if manifest
            .get("lib")
            .and_then(|value| value.get("path"))
            .and_then(toml::Value::as_str)
            == Some(relative.as_str())
        {
            return true;
        }
        for kind in ["bin", "test", "bench", "example"] {
            if let Some(targets) = manifest.get(kind).and_then(toml::Value::as_array)
                && targets
                    .iter()
                    .any(|target| target.get("path").and_then(toml::Value::as_str) == Some(relative.as_str()))
            {
                return true;
            }
        }
    }
    false
}

/// Resolves explicit Cargo target roles without invoking Cargo or dependencies.
///
/// # Parameters
///
/// * `project` - Project boundary for Cargo context discovery.
/// * `path` - Relative source file whose target role is inspected.
pub(crate) fn source_role(project: &Path, path: &str) -> (bool, bool) {
    let root = package_root(project, path);
    let manifest = safe_text(project, root.join("Cargo.toml"))
        .ok()
        .and_then(|text| toml::from_str::<toml::Value>(&text).ok());
    let relative = Path::new(path)
        .strip_prefix(root.strip_prefix(project).unwrap_or(Path::new("")))
        .unwrap_or(Path::new(path));
    let relative = relative.to_string_lossy().replace('\\', "/");
    if let Some(manifest) = manifest.as_ref() {
        for target in ["test", "bench", "example", "bin"] {
            if let Some(entries) = manifest.get(target).and_then(toml::Value::as_array) {
                for entry in entries {
                    if entry.get("path").and_then(toml::Value::as_str) == Some(relative.as_str()) {
                        return (target == "bin", target == "test");
                    }
                }
            }
        }
        if manifest
            .get("lib")
            .and_then(|value| value.get("path"))
            .and_then(toml::Value::as_str)
            == Some(relative.as_str())
        {
            return (true, false);
        }
    }
    let tests = path.split('/').any(|part| part == "tests");
    (
        path.split('/').any(|part| part == "src")
            && !tests
            && !path
                .split('/')
                .any(|part| matches!(part, "benches" | "examples" | "fuzz")),
        tests,
    )
}

/// Resolves the package edition including inherited workspace metadata for
/// rustfmt.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn package_edition(project: &Path, path: &str) -> Option<String> {
    let root = package_root(project, path);
    let manifest = safe_text(project, root.join("Cargo.toml"))
        .ok()
        .and_then(|text| toml::from_str::<toml::Value>(&text).ok());
    let Some(package) = manifest.as_ref().and_then(|value| value.get("package")) else {
        return Some("2024".into());
    };
    match package.get("edition") {
        Some(toml::Value::String(edition)) => Some(edition.clone()),
        Some(toml::Value::Table(table)) if table.get("workspace").and_then(toml::Value::as_bool) == Some(true) => {
            for directory in root.ancestors().take_while(|directory| directory.starts_with(project)) {
                if let Some(edition) = safe_text(project, directory.join("Cargo.toml"))
                    .ok()
                    .and_then(|text| toml::from_str::<toml::Value>(&text).ok())
                    .and_then(|value| {
                        value
                            .get("workspace")
                            .and_then(|value| value.get("package"))
                            .and_then(|value| value.get("edition"))
                            .and_then(toml::Value::as_str)
                            .map(str::to_owned)
                    })
                {
                    return Some(edition);
                }
            }
            None
        }
        _ => Some("2015".into()),
    }
}

/// Finds conventional external module files without assuming one exists.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn module_child(path: &str, name: &str, source: &BTreeMap<String, String>) -> Vec<String> {
    let file = Path::new(path);
    let stem = file.file_stem().unwrap().to_string_lossy();
    let parent = file.parent().unwrap_or(Path::new(""));
    let directory = if matches!(stem.as_ref(), "lib" | "main" | "mod") {
        parent.to_path_buf()
    } else {
        parent.join(stem.as_ref())
    };
    [
        directory.join(format!("{name}.rs")),
        directory.join(name).join("mod.rs"),
    ]
    .into_iter()
    .map(|path| path.to_string_lossy().replace('\\', "/"))
    .filter(|path| source.contains_key(path))
    .collect()
}

/// Collects statically resolvable module edges without expanding macros or
/// selecting cfgs.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn module_edges(
    path: &str,
    items: &[syn::Item],
    sources: &BTreeMap<String, String>,
    graph: &mut BTreeMap<String, Vec<String>>,
    inline_directory: Option<&Path>,
) {
    for item in items {
        let syn::Item::Mod(module) = item else { continue };
        let name = module.ident.to_string();
        let parent_dir = inline_directory.map(Path::to_path_buf).unwrap_or_else(|| {
            let file = Path::new(path);
            let parent = file.parent().unwrap_or(Path::new(""));
            if matches!(file.file_stem().and_then(|x| x.to_str()), Some("lib" | "main" | "mod")) {
                parent.to_path_buf()
            } else {
                parent.join(file.file_stem().unwrap())
            }
        });
        if let Some((_, content)) = &module.content {
            module_edges(path, content, sources, graph, Some(&parent_dir.join(name)));
        } else {
            let mut targets = if inline_directory.is_none() {
                module_child(path, &name, sources)
            } else {
                [
                    parent_dir.join(format!("{name}.rs")),
                    parent_dir.join(&name).join("mod.rs"),
                ]
                .into_iter()
                .map(|x| x.to_string_lossy().replace('\\', "/"))
                .filter(|x| sources.contains_key(x))
                .collect()
            };
            if let Some(custom) = module.attrs.iter().find_map(|attr| {
                if !attr.path().is_ident("path") {
                    return None;
                }
                if let syn::Meta::NameValue(value) = &attr.meta
                    && let syn::Expr::Lit(value) = &value.value
                    && let syn::Lit::Str(value) = &value.lit
                {
                    return Some(value.value());
                }
                None
            }) {
                let candidate = Path::new(path)
                    .parent()
                    .unwrap()
                    .join(custom)
                    .to_string_lossy()
                    .replace('\\', "/");
                targets = if sources.contains_key(&candidate) {
                    vec![candidate]
                } else {
                    Vec::new()
                };
            }
            graph.entry(path.into()).or_default().extend(targets);
        }
    }
}

/// Adds project ownership, module, method and complexity evidence to the
/// requested source reports.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
pub(crate) fn inspect(
    project: &Path,
    requested: &BTreeSet<String>,
    sources: &BTreeMap<String, String>,
    parsed: &BTreeMap<String, syn::File>,
    roles: &BTreeMap<String, (bool, bool)>,
    report: &mut AnalysisReport,
) {
    let mut declarations = Vec::new();
    let mut graph = BTreeMap::new();
    for (path, ast) in parsed {
        let mut index = Index {
            path,
            declarations: Vec::new(),
            impls: Vec::new(),
            conditional: 0,
        };
        index.visit_file(ast);
        declarations.extend(index.declarations);
        module_edges(
            path,
            &ast.items,
            sources,
            &mut graph,
            if target_root(project, path) {
                Some(Path::new(path).parent().unwrap_or(Path::new("")))
            } else {
                None
            },
        );
    }
    for path in requested {
        let Some(ast) = parsed.get(path) else { continue };
        let Some(source) = sources.get(path) else { continue };
        let (production, tests) = roles.get(path).copied().unwrap_or((false, false));
        if tests && Path::new(path).file_name().is_some_and(|name| name == "main.rs") && !target_root(project, path) {
            report.diagnostics.push(finding(path,"TEST-003","test-file-name",1,"The main.rs naming exception requires a discovered Cargo test target entry",format!("{path} is not a conventional or explicitly registered target root; confirm its Cargo discovery before accepting the filename"),"info"));
        }
        if production {
            let mut index = Index {
                path,
                declarations: Vec::new(),
                impls: Vec::new(),
                conditional: 0,
            };
            index.visit_file(ast);
            for (implementation, conditional) in index.impls {
                let syn::Type::Path(target) = implementation.self_ty.as_ref() else {
                    continue;
                };
                if target.qself.is_some() || target.path.segments.len() != 1 {
                    continue;
                }
                let name = target.path.segments[0].ident.to_string();
                let candidates: Vec<_> = declarations
                    .iter()
                    .filter(|decl| {
                        decl.name == name && package_root(project, &decl.path) == package_root(project, path)
                    })
                    .collect();
                if candidates.len() != 1 {
                    continue;
                }
                let owner = candidates[0];
                if owner.path != *path {
                    let declared = graph.get(&owner.path).is_some_and(|children| children.contains(path));
                    // Name resolution, cfg, and aliases can alter ownership; absent proof is a
                    // review signal.
                    let mut diagnostic = finding(
                        path,
                        "ORG-002",
                        "inherent-owner",
                        implementation.span().start().line,
                        "An inherent implementation is outside its candidate defining file",
                        format!(
                            "impl {name}; candidate definition {}:{}; directly declared implementation module: {declared}; conditional declaration: {}; conditional impl: {conditional}",
                            owner.path, owner.line, owner.conditional
                        ),
                        "info",
                    );
                    diagnostic.related_locations.push(Location {
                        path: owner.path.clone(),
                        line: owner.line,
                        column: 1,
                    });
                    report.diagnostics.push(diagnostic);
                }
                let direct_owner=ast.items.iter().any(|item|matches!(item,syn::Item::Struct(value) if value.ident==name && value.attrs.iter().any(|attr|attr.path().is_ident("must_use")))||matches!(item,syn::Item::Enum(value) if value.ident==name && value.attrs.iter().any(|attr|attr.path().is_ident("must_use"))));
                let direct_impl = ast
                    .items
                    .iter()
                    .any(|item| matches!(item,syn::Item::Impl(value) if std::ptr::eq(value,implementation)));
                let generic_shadow = implementation
                    .generics
                    .params
                    .iter()
                    .any(|param| matches!(param,syn::GenericParam::Type(value) if value.ident==name));
                if owner.must_use
                    && conditional == 0
                    && !owner.conditional
                    && owner.path == *path
                    && direct_owner
                    && direct_impl
                    && !generic_shadow
                {
                    for item in &implementation.items {
                        let syn::ImplItem::Fn(method) = item else { continue };
                        let plain = method
                            .attrs
                            .iter()
                            .any(|attr| attr.path().is_ident("must_use") && matches!(attr.meta, syn::Meta::Path(_)));
                        let returns_self = matches!(&method.sig.output,syn::ReturnType::Type(_,ty) if matches!(ty.as_ref(),syn::Type::Path(value) if value.qself.is_none() && value.path.is_ident("Self")));
                        if plain && returns_self {
                            report.diagnostics.push(finding(
                                path,
                                "ATTR-001",
                                "double-must-use-self",
                                method.sig.ident.span().start().line,
                                "Function repeats the must_use contract of its Self return type",
                                format!("{} returns Self; type {name} already has #[must_use]", method.sig.ident),
                                "error",
                            ));
                        }
                    }
                }
            }
        }
        let mut methods = MethodInventory { entries: Vec::new() };
        methods.visit_file(ast);
        if !methods.entries.is_empty() {
            report.diagnostics.push(finding(
                path,
                "METHOD-001",
                "method-inventory",
                1,
                "Review inherent method ordering by constructor/accessor/builder semantics",
                methods.entries.join("\n"),
                "info",
            ));
        }
        let aggregate = matches!(
            Path::new(path).file_name().and_then(|x| x.to_str()),
            Some("lib.rs" | "mod.rs")
        );
        if production
            && aggregate
            && ast
                .items
                .iter()
                .any(|item| !matches!(item, syn::Item::Mod(_) | syn::Item::Use(_) | syn::Item::ExternCrate(_)))
        {
            report.diagnostics.push(finding(
                path,
                "ORG-003",
                "aggregation-items",
                1,
                "Aggregation file contains items requiring a business-boundary review",
                ast.items
                    .iter()
                    .filter(|item| !matches!(item, syn::Item::Mod(_) | syn::Item::Use(_)))
                    .map(|item| format!("line {}: {}", item.span().start().line, item_label(item)))
                    .collect::<Vec<_>>()
                    .join("\n"),
                "info",
            ));
        }
        let mut complexity = Complexity {
            branches: 0,
            loops: 0,
            depth: 0,
            maximum: 0,
        };
        complexity.visit_file(ast);
        if production && (complexity.branches > 0 || complexity.loops > 0) {
            report.diagnostics.push(finding(
                path,
                "COMP-001",
                "complexity-evidence",
                1,
                "Review domain responsibilities using these structural signals",
                format!(
                    "branches={}, loops={}, maximum block nesting={}; these are evidence, not rejection thresholds",
                    complexity.branches, complexity.loops, complexity.maximum
                ),
                "info",
            ));
        }
        if roles.get(path).is_some_and(|role| role.1) {
            let root = package_root(project, path);
            let relative_root = root
                .strip_prefix(project)
                .unwrap_or(Path::new(""))
                .to_string_lossy()
                .replace('\\', "/");
            let prefix = if relative_root.is_empty() {
                String::new()
            } else {
                format!("{relative_root}/")
            };
            let mut roots: BTreeSet<String> = BTreeSet::new();
            for candidate in [format!("{prefix}src/lib.rs"), format!("{prefix}src/main.rs")] {
                if sources.contains_key(&candidate) {
                    roots.insert(candidate);
                }
            }
            let manifest = safe_text(project, root.join("Cargo.toml"))
                .ok()
                .and_then(|text| toml::from_str::<toml::Value>(&text).ok());
            let autotests = manifest
                .as_ref()
                .and_then(|value| value.get("package"))
                .and_then(|value| value.get("autotests"))
                .and_then(toml::Value::as_bool)
                .unwrap_or(true);
            if autotests {
                for candidate in sources.keys() {
                    if let Some(rest) = candidate.strip_prefix(&format!("{prefix}tests/"))
                        && (!rest.contains('/') || (rest.matches('/').count() == 1 && rest.ends_with("/main.rs")))
                    {
                        roots.insert(candidate.clone());
                    }
                }
            }
            if let Some(targets) = manifest
                .as_ref()
                .and_then(|value| value.get("test"))
                .and_then(toml::Value::as_array)
            {
                for target in targets {
                    if let Some(target) = target.get("path").and_then(toml::Value::as_str) {
                        roots.insert(format!("{prefix}{target}"));
                    }
                }
            }
            let mut seen = BTreeSet::new();
            let mut pending: Vec<_> = roots.into_iter().collect();
            while let Some(current) = pending.pop() {
                if seen.insert(current.clone()) {
                    pending.extend(graph.get(&current).into_iter().flatten().cloned());
                }
            }
            if !seen.contains(path)
                && !path
                    .split('/')
                    .any(|part| matches!(part, "support" | "common" | "fixtures" | "internal"))
            {
                // cfg_attr path expansion and custom harness discovery require semantic
                // context.
                report.diagnostics.push(finding(path,"TEST-002","test-reachability",1,"Test source is not reachable through the statically resolved Cargo/module graph",format!("No statically resolved target/mod path reaches {path}; conditional or macro module declarations require review"),"info"));
            }
        }
        check_formatter(project, path, source, ast, report);
    }
}

/// Compares import order with read-only output from the configured installed
/// formatter.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn check_formatter(project: &Path, path: &str, source: &str, ast: &syn::File, report: &mut AnalysisReport) {
    let config = project.join(".infra/style/rustfmt.toml");
    if let Err(error) = crate::analysis::read_safe(project, ".infra/style/rustfmt.toml") {
        report.blocked.push(AnalysisIssue {
            path: path.into(),
            reason: format!("cannot safely read rustfmt config: {error:#}"),
        });
        return;
    }
    let defaults = project.join(".infra/tools/defaults.toml");
    let toolchain = std::env::var("RS_INFRA_STYLE_TOOLCHAIN")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            safe_text(project, &defaults)
                .ok()
                .and_then(|text| toml::from_str::<toml::Value>(&text).ok())
                .and_then(|value| {
                    value
                        .get("nightly_toolchain")
                        .and_then(toml::Value::as_str)
                        .map(str::to_owned)
                })
        });
    if !config.is_file() || toolchain.is_none() {
        report.coverage.push(Coverage {
            path: path.into(),
            rule_id: "IMPORT-001".into(),
            check_id: "rustfmt-import-order".into(),
            status: "blocked".into(),
            evidence: "Project rustfmt config or configured toolchain missing; no substitute string sort is used"
                .into(),
        });
        report.blocked.push(AnalysisIssue {
            path: path.into(),
            reason: "cannot check rustfmt import order: missing project rustfmt configuration or configured toolchain"
                .into(),
        });
        return;
    }
    let toolchain = toolchain.unwrap();
    if toolchain.starts_with(['+', '-']) || toolchain.chars().any(char::is_whitespace) {
        report.blocked.push(AnalysisIssue {
            path: path.into(),
            reason: "invalid configured rustfmt toolchain".into(),
        });
        return;
    }
    let Some(edition) = package_edition(project, path) else {
        report.blocked.push(AnalysisIssue {
            path: path.into(),
            reason: "workspace-inherited package edition cannot be resolved without guessing".into(),
        });
        return;
    };
    let output = (|| -> std::io::Result<std::process::Output> {
        let mut child = Command::new("rustup")
            .args([
                "run",
                &toolchain,
                "rustfmt",
                "--emit",
                "stdout",
                "--config",
                "skip_children=true",
                "--edition",
                &edition,
                "--config-path",
            ])
            .arg(config)
            .current_dir(project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        child.stdin.take().unwrap().write_all(source.as_bytes())?;
        child.wait_with_output()
    })();
    match output {
        Ok(output) if output.status.success() => {
            let formatted = String::from_utf8_lossy(&output.stdout);
            let original_imports = Some(imports_from_ast(ast));
            let formatted_imports = import_sequence(&formatted);
            if let (Some(before), Some(after)) = (original_imports, formatted_imports) {
                if before != after {
                    report.diagnostics.push(finding(
                        path,
                        "IMPORT-001",
                        "rustfmt-import-order",
                        1,
                        "Import order differs from the project's configured rustfmt output",
                        format!(
                            "original imports: {}; formatted imports: {}",
                            before.join("; "),
                            after.join("; ")
                        ),
                        "error",
                    ));
                }
                report.coverage.push(Coverage {path:path.into(),rule_id:"IMPORT-001".into(),check_id:"rustfmt-import-order".into(),status:if before==after {"pass"}else{"fail"}.into(),evidence:"Import AST order compared with read-only project rustfmt output; unrelated formatting is deferred to alignment".into()});
            } else {
                report.blocked.push(AnalysisIssue {
                    path: path.into(),
                    reason: "unable to parse project rustfmt output for import ordering".into(),
                });
            }
        }
        Ok(output) => report.blocked.push(AnalysisIssue {
            path: path.into(),
            reason: format!(
                "project rustfmt check failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        }),
        Err(error) => report.blocked.push(AnalysisIssue {
            path: path.into(),
            reason: format!("cannot execute configured rustfmt: {error}"),
        }),
    }
}

/// Collects normalized use declarations from an already parsed source file.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn imports_from_ast(ast: &syn::File) -> Vec<String> {
    let mut imports = ImportInventory { imports: Vec::new() };
    imports.visit_file(ast);
    imports.imports
}
/// Parses formatter output and extracts only its import declaration sequence.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn import_sequence(source: &str) -> Option<Vec<String>> {
    syn::parse_file(source).ok().map(|ast| imports_from_ast(&ast))
}

/// Produces concise item-kind evidence without copying full business
/// implementations.
///
/// # Parameters
///
/// Arguments provide the source, project context or analysis state described by
/// this operation.
fn item_label(item: &syn::Item) -> String {
    match item {
        syn::Item::Struct(value) => format!("struct {}", value.ident),
        syn::Item::Enum(value) => format!("enum {}", value.ident),
        syn::Item::Trait(value) => format!("trait {}", value.ident),
        syn::Item::Fn(value) => format!("fn {}", value.sig.ident),
        syn::Item::Impl(_) => "impl (see source at this line)".into(),
        syn::Item::Const(value) => format!("const {}", value.ident),
        syn::Item::Static(value) => format!("static {}", value.ident),
        syn::Item::Type(value) => format!("type {}", value.ident),
        syn::Item::Macro(value) => format!("macro {}", value.mac.path.to_token_stream()),
        _ => "other item (see source at this line)".into(),
    }
}
