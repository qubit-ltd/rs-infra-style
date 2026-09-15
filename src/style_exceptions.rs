use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

const CONFIG_PATH: &str = ".infra/style/exceptions.toml";
const CONFIG_FORMAT: u8 = 1;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExceptionConfig {
    pub(crate) format: u8,
    #[serde(default)]
    pub(crate) exceptions: Vec<StyleException>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StyleException {
    pub(crate) rule: String,
    pub(crate) path: String,
    pub(crate) reason: String,
}

impl ExceptionConfig {
    pub(crate) fn load(project: &Path) -> Result<Self> {
        let path = project.join(CONFIG_PATH);
        if !path.is_file() {
            return Ok(Self {
                format: CONFIG_FORMAT,
                exceptions: Vec::new(),
            });
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: Self =
            toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))?;
        config.validate(project)?;
        Ok(config)
    }

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
                bail!(
                    "{} contains unknown style rule '{}'",
                    CONFIG_PATH,
                    exception.rule
                );
            }
            if exception.path.is_empty()
                || Path::new(&exception.path).is_absolute()
                || exception
                    .path
                    .split('/')
                    .any(|part| part == ".." || part.is_empty())
                || exception.path.contains('\\')
            {
                bail!(
                    "{} contains invalid relative path '{}'",
                    CONFIG_PATH,
                    exception.path
                );
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
                bail!(
                    "{} refers to missing file '{}'",
                    CONFIG_PATH,
                    exception.path
                );
            }
        }
        Ok(())
    }

    pub(crate) fn allows(&self, rule: &str, path: &str) -> bool {
        self.exceptions
            .iter()
            .any(|exception| exception.rule == rule && exception.path == path)
    }
}

pub(crate) fn known_rule(rule: &str) -> bool {
    matches!(
        rule,
        "coverage-cfg"
            | "test-file-name"
            | "test-redirect"
            | "explicit-imports"
            | "aggregation-files"
            | "public-type-layout"
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
