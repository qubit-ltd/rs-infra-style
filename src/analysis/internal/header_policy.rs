// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Explicit, reasoned project overrides for the canonical copyright line.

use std::path::Path;

use anyhow::Result;
use anyhow::bail;

/// Defines the accepted copyright owner and year forms for source headers.
///
/// The default follows the shared template. A project override requires an
/// explicit versioned configuration and reason; neighboring source files do
/// not influence this policy.
pub(crate) struct HeaderPolicy {
    /// Literal owner inserted before the copyright line's final period.
    owner: String,
    /// Earliest accepted four-digit year, also the beginning of accepted
    /// ranges.
    minimum_year: u16,
    /// Source and reason recorded in coverage evidence.
    pub(crate) evidence: String,
}

impl Default for HeaderPolicy {
    /// Creates the shared owner and earliest-year policy without reading files.
    ///
    /// # Returns
    ///
    /// Returns the canonical Haixing Hu policy beginning in 2025.
    fn default() -> Self {
        Self {
            owner: "Haixing Hu".into(),
            minimum_year: 2025,
            evidence: "Canonical rust-file-header.md owner and accepted year forms".into(),
        }
    }
}

impl HeaderPolicy {
    /// Reads an explicit project override, preserving the canonical fallback.
    ///
    /// Missing configuration or a configuration without a header table uses
    /// the shared template. This function validates an override and records its
    /// reason without modifying project files.
    ///
    /// # Parameters
    ///
    /// * `project` - Project root used to locate `.infra/style/analysis.toml`.
    ///
    /// # Returns
    ///
    /// Returns the validated project policy or the canonical policy.
    ///
    /// # Errors
    ///
    /// Returns an error for symbolic-link configuration paths, unreadable or
    /// invalid UTF-8/TOML input, unsupported fields or format versions, and
    /// overrides missing a safe owner, four-digit year, or nonempty reason.
    pub(crate) fn load(project: &Path) -> Result<Self> {
        crate::analysis::validate_no_symlink(project, ".infra/style/analysis.toml")?;
        let path = project.join(".infra/style/analysis.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = String::from_utf8(crate::analysis::read_safe(project, ".infra/style/analysis.toml")?)?;
        let config: toml::Value = toml::from_str(&text)?;
        let Some(table) = config.as_table() else {
            bail!("analysis.toml must contain a configuration table");
        };
        if table.keys().any(|key| !matches!(key.as_str(), "format" | "header"))
            || config.get("format").and_then(toml::Value::as_integer) != Some(1)
        {
            bail!("analysis.toml requires format=1 and only the optional header table");
        }
        let Some(header) = table.get("header") else {
            return Ok(Self::default());
        };
        let Some(header) = header.as_table() else {
            bail!("analysis.toml header must be a table");
        };
        if header
            .keys()
            .any(|key| !matches!(key.as_str(), "owner" | "minimum_year" | "reason"))
        {
            bail!("analysis.toml header contains an unknown field");
        }
        let owner = header.get("owner").and_then(toml::Value::as_str).unwrap_or("");
        let reason = header.get("reason").and_then(toml::Value::as_str).unwrap_or("");
        let minimum = header
            .get("minimum_year")
            .and_then(toml::Value::as_integer)
            .unwrap_or(0);
        if owner.is_empty()
            || owner.trim() != owner
            || owner.chars().any(char::is_control)
            || owner.contains(['{', '}'])
            || reason.trim().is_empty()
            || !(1000..=9999).contains(&minimum)
        {
            bail!("header override requires a safe single-line owner, four-digit minimum_year and nonempty reason");
        }
        Ok(Self {
            owner: owner.into(),
            minimum_year: minimum as u16,
            evidence: format!(
                ".infra/style/analysis.toml [header]: owner={owner:?}, minimum_year={minimum}; reason: {reason}"
            ),
        })
    }

    /// Checks the header template without updating an accepted copyright year.
    ///
    /// The six fixed lines must match exactly. The copyright line accepts a
    /// single year or a range beginning at the policy's minimum year, with all
    /// displayed years containing four digits and meeting the minimum.
    ///
    /// # Parameters
    ///
    /// * `text` - Complete Rust source whose first seven lines are inspected.
    ///
    /// # Returns
    ///
    /// Returns `true` for a complete conforming header and `false` for a
    /// missing, incomplete, or nonconforming header. Source text is unchanged.
    #[must_use]
    pub(crate) fn accepts(&self, text: &str) -> bool {
        let lines: Vec<_> = text.lines().take(7).collect();
        let expected = [
            "// =============================================================================",
            "",
            "//",
            "//    SPDX-License-Identifier: Apache-2.0",
            "//",
            "//    Licensed under the Apache License, Version 2.0.",
            "// =============================================================================",
        ];
        if lines.len() != 7
            || lines
                .iter()
                .zip(expected)
                .enumerate()
                .any(|(index, (actual, expected))| index != 1 && *actual != expected)
        {
            return false;
        }
        let Some(year) = lines[1]
            .strip_prefix("//    Copyright (c) ")
            .and_then(|line| line.strip_suffix(&format!(" {}.", self.owner)))
        else {
            return false;
        };
        let valid = |value: &str| {
            value.len() == 4
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && value.parse::<u16>().is_ok_and(|year| year >= self.minimum_year)
        };
        year.strip_prefix(&format!("{} - ", self.minimum_year))
            .map_or_else(|| valid(year), valid)
    }
}
