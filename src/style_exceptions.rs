// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

const CONFIG_PATH: &str = ".infra/style/exceptions.toml";
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

/// Describes one permitted style exception for a project file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StyleException {
    /// Names the style rule that is allowed to have an exception.
    pub(crate) rule: String,
    /// Gives the project-relative path to which the exception applies.
    pub(crate) path: String,
    /// Explains why this specific rule and path are exempted.
    pub(crate) reason: String,
}

impl ExceptionConfig {
    /// Loads and validates the style exception configuration for a project.
    ///
    /// If the configuration file is absent, this returns an empty configuration
    /// using the current format. File access and TOML parsing errors are
    /// returned with the configuration path attached as context.
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

    /// Checks that the format and every exception are valid for the project.
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

    /// Reports whether the exact rule and project-relative path are exempted.
    pub(crate) fn allows(&self, rule: &str, path: &str) -> bool {
        self.exceptions
            .iter()
            .any(|exception| exception.rule == rule && exception.path == path)
    }
}

/// Determines whether a rule identifier is recognized by the style checker.
pub(crate) fn known_rule(rule: &str) -> bool {
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
    fn loads_exact_path_exception() {
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
    fn rejects_unknown_rule_and_missing_path() {
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
