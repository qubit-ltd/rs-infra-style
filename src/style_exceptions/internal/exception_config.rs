// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Loads and validates explicitly justified, exact-path style exceptions.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

use super::style_exception::StyleException;

/// Project-owned exception file; shared updates do not override its contents.
const CONFIG_PATH: &str = ".infra/style/exceptions.toml";
/// Supported serialization version for exact rule/path exception records.
const CONFIG_FORMAT: u8 = 1;

/// Holds the validated style exceptions loaded from a project configuration.
///
/// The configuration format must match [`CONFIG_FORMAT`], and every exception
/// must refer to a known rule and an existing project file.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExceptionConfig {
    /// Identifies the schema version used by the configuration file.
    pub(crate) format: u8,
    /// Lists rule and path pairs whose style findings are intentionally
    /// allowed.
    #[serde(default)]
    pub(crate) exceptions: Vec<StyleException>,
}

impl ExceptionConfig {
    /// Loads and validates the style exception configuration for a project.
    ///
    /// If the configuration file is absent, this returns an empty configuration
    /// using the current format. File access and TOML parsing errors are
    /// returned with the configuration path attached as context.
    ///
    /// # Parameters
    ///
    /// * `project` - Project root containing the optional exception file.
    ///
    /// # Returns
    ///
    /// Returns an empty current-format policy when no file exists, or all
    /// validated, explicitly reasoned exception records.
    ///
    /// # Errors
    ///
    /// Returns an error for unreadable or malformed TOML, unsupported format,
    /// unknown rules, invalid paths, missing targets, duplicate records, or
    /// absent reasons.
    pub(crate) fn load(project: &Path) -> Result<Self> {
        let path = project.join(CONFIG_PATH);
        if !path.is_file() {
            return Ok(Self {
                format: CONFIG_FORMAT,
                exceptions: Vec::new(),
            });
        }
        let text = fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let config: Self = toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))?;
        config.validate(project)?;
        Ok(config)
    }

    /// Validates the format and the identity and justification of each record.
    ///
    /// # Parameters
    ///
    /// * `project` - Root used to verify that exception targets exist.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when every exception has a distinct recognized rule
    /// and path pair, an existing file target, and a nonempty reason.
    ///
    /// # Errors
    ///
    /// Returns an error identifying the first unsupported format or invalid
    /// exception record.
    fn validate(&self, project: &Path) -> Result<()> {
        if self.format != CONFIG_FORMAT {
            bail!(
                "{} uses unsupported format {}; expected {}",
                CONFIG_PATH,
                self.format,
                CONFIG_FORMAT
            );
        }
        let mut keys = HashSet::new();
        for exception in &self.exceptions {
            if !known_rule(&exception.rule) {
                bail!("{} contains unknown style rule '{}'", CONFIG_PATH, exception.rule);
            }
            if exception.path.is_empty()
                || Path::new(&exception.path).is_absolute()
                || exception.path.split('/').any(|part| part == ".." || part.is_empty())
                || exception.path.contains('\\')
            {
                bail!("{} contains invalid relative path '{}'", CONFIG_PATH, exception.path);
            }
            if exception.reason.trim().is_empty() {
                bail!(
                    "{} requires a non-empty reason for {}:{}",
                    CONFIG_PATH,
                    exception.rule,
                    exception.path
                );
            }
            let key = format!("{}\0{}", exception.rule, exception.path);
            if !keys.insert(key) {
                bail!(
                    "{} contains duplicate exception {}:{}",
                    CONFIG_PATH,
                    exception.rule,
                    exception.path
                );
            }
            if !project.join(&exception.path).is_file() {
                bail!("{} refers to missing file '{}'", CONFIG_PATH, exception.path);
            }
        }
        Ok(())
    }

    /// Checks for an explicitly justified exception for this exact rule and
    /// path.
    ///
    /// # Parameters
    ///
    /// * `rule` - Stable exception category recognized by the configuration.
    /// * `path` - Project-relative file path; no glob expansion is performed.
    ///
    /// # Returns
    ///
    /// Returns `true` only for an exact configured pair, otherwise `false`.
    #[must_use]
    pub(crate) fn allows(&self, rule: &str, path: &str) -> bool {
        self.exceptions
            .iter()
            .any(|exception| exception.rule == rule && exception.path == path)
    }
}

/// Recognizes supported project exception categories without granting them.
///
/// # Parameters
///
/// * `rule` - Exception category read from project configuration.
///
/// # Returns
///
/// Returns `true` for a category understood by the checker, otherwise `false`.
#[must_use]
fn known_rule(rule: &str) -> bool {
    matches!(
        rule,
        "coverage-cfg"
            | "inline-tests"
            | "source-test-pair"
            | "test-file-name"
            | "test-redirect"
            | "explicit-imports"
            | "aggregation-files"
            | "public-type-layout"
            | "multiple-public-types"
            | "type-file-name"
            | "internal-test-module"
    )
}

#[cfg(test)]
mod tests {
    use super::ExceptionConfig;

    #[test]
    fn test_loads_exact_path_exception() {
        let directory = tempfile::tempdir().unwrap();
        let config_dir = directory.path().join(".infra/style");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(directory.path().join("src")).unwrap();
        std::fs::write(directory.path().join("src/lib.rs"), "pub fn value() {}\n").unwrap();
        std::fs::write(
            config_dir.join("exceptions.toml"),
            "format = 1\n\n[[exceptions]]\nrule = \"type-file-name\"\npath = \"src/lib.rs\"\nreason = \"The public API is intentionally rooted here.\"\n",
        )
        .unwrap();

        let config = ExceptionConfig::load(directory.path()).unwrap();
        assert!(config.allows("type-file-name", "src/lib.rs"));
        assert!(!config.allows("type-file-name", "src/other.rs"));
    }

    #[test]
    fn test_rejects_unknown_rule_and_missing_path() {
        let directory = tempfile::tempdir().unwrap();
        let config_dir = directory.path().join(".infra/style");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("exceptions.toml"),
            "format = 1\n\n[[exceptions]]\nrule = \"unknown\"\npath = \"src/missing.rs\"\nreason = \"Not allowed.\"\n",
        )
        .unwrap();

        let error = ExceptionConfig::load(directory.path()).unwrap_err();
        assert!(error.to_string().contains("unknown style rule"));
    }
}
