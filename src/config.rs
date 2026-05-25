use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

/// Top-level configuration, mirrors `safe-cargo.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SafeCargoConfig {
    pub registry: RegistryConfig,
    pub policy: PolicyConfig,
    #[serde(default)]
    pub allow: HashMap<String, AllowEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct RegistryConfig {
    pub repo: String,
    pub branch: String,
    pub signing_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct PolicyConfig {
    pub min_release_age: String,
    pub max_score: u32,
    pub require_report: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AllowEntry {
    pub version: String,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            repo: String::from("user/safe-cargo"),
            branch: String::from("main"),
            signing_key: None,
        }
    }
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            min_release_age: String::from("3d"),
            max_score: 40,
            require_report: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

impl SafeCargoConfig {
    /// Search for `safe-cargo.toml` starting from the current directory and
    /// walking up to the filesystem root.  If nothing is found, fall back to
    /// `~/.config/safe-cargo/config.toml`.  Returns `Error::ConfigNotFound` if
    /// neither location contains a config file.
    pub fn load() -> Result<Self> {
        if let Some(path) = Self::find_in_ancestors()? {
            return Self::load_from(&path);
        }

        if let Some(path) = Self::global_path() {
            if path.exists() {
                return Self::load_from(&path);
            }
        }

        Err(Error::ConfigNotFound)
    }

    /// Load configuration from a specific file path.
    pub fn load_from(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path).map_err(|e| Error::ConfigParse {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

        let config: Self = toml::from_str(&contents).map_err(|e| Error::ConfigParse {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

        Ok(config)
    }

    /// Returns `true` if the crate at the given version is explicitly
    /// allow-listed in the `[allow]` table.
    pub fn is_allowed(&self, name: &str, version: &str) -> bool {
        match self.allow.get(name) {
            Some(entry) => entry.version == version,
            None => false,
        }
    }

    // -- private helpers ----------------------------------------------------

    /// Walk from the current directory upward looking for `safe-cargo.toml`.
    fn find_in_ancestors() -> Result<Option<PathBuf>> {
        let cwd = std::env::current_dir()?;
        let mut dir: Option<&Path> = Some(cwd.as_path());

        while let Some(d) = dir {
            let candidate = d.join("safe-cargo.toml");
            if candidate.exists() {
                return Ok(Some(candidate));
            }
            dir = d.parent();
        }

        Ok(None)
    }

    /// `~/.config/safe-cargo/config.toml`
    fn global_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("safe-cargo").join("config.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = SafeCargoConfig::default();
        assert_eq!(cfg.registry.repo, "user/safe-cargo");
        assert_eq!(cfg.registry.branch, "main");
        assert_eq!(cfg.policy.min_release_age, "3d");
        assert_eq!(cfg.policy.max_score, 40);
        assert!(cfg.policy.require_report);
        assert!(cfg.allow.is_empty());
    }

    #[test]
    fn is_allowed_matches() {
        let mut cfg = SafeCargoConfig::default();
        cfg.allow.insert(
            "openssl-sys".into(),
            AllowEntry {
                version: "0.9.102".into(),
                reason: "audited 2026-05-20".into(),
            },
        );

        assert!(cfg.is_allowed("openssl-sys", "0.9.102"));
        assert!(!cfg.is_allowed("openssl-sys", "0.9.103"));
        assert!(!cfg.is_allowed("serde", "1.0.0"));
    }

    #[test]
    fn deserialize_full_config() {
        let toml_str = r#"
[registry]
repo = "acme/reports"
branch = "prod"

[policy]
min_release_age = "1w"
max_score = 4
require_report = false

[allow]
openssl-sys = { version = "0.9.102", reason = "audited" }
"#;
        let cfg: SafeCargoConfig = toml::from_str(toml_str).expect("should parse");
        assert_eq!(cfg.registry.repo, "acme/reports");
        assert_eq!(cfg.registry.branch, "prod");
        assert_eq!(cfg.policy.min_release_age, "1w");
        assert_eq!(cfg.policy.max_score, 4);
        assert!(!cfg.policy.require_report);
        assert!(cfg.is_allowed("openssl-sys", "0.9.102"));
    }
}
