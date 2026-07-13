//! Configuration parsing and management for GAL.
//!
//! Rust owns config truth. The config file is `~/.gal/config/config.json`.
//! Missing file or keys produce explicit defaults; no panics.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

// ── Machine path registry ─────────────────────────────────────────────────────

/// Maps retained machine-local path keys in `config.json` to UPPER_SNAKE
/// placeholder names.
///
/// The dot-path side is the authoritative `config.json` key; the UPPER_SNAKE
/// side is the placeholder name used in git filters and projections.
///
/// Only `galSkills` remains: it is the lone machine-local absolute path still
/// worth path substitution after the obsidian/research/tempDir retirement.
/// Credential keys (`*_API_KEY`, `*_TOKEN`, etc.) live in `config.json#secrets`
/// and are never path-substituted.
pub const MACHINE_PATH_REGISTRY: &[(&str, &str)] = &[("galSkills", "GAL_SKILLS")];

/// Look up a machine path value from a `config.json` root `serde_json::Value`.
///
/// `config_path` is a dot-separated key path (e.g. `"galSkills"`).
/// Returns the string value when present, `None` when absent or not a string.
pub fn machine_path_value(config: &Value, config_path: &str) -> Option<String> {
    let mut current = config;
    for segment in config_path.split('.') {
        current = current.get(segment)?;
    }
    current.as_str().map(str::to_owned)
}

/// The complete GAL configuration model.
///
/// Corresponds to `~/.gal/config/config.json`.
/// Unknown keys are silently ignored (forward-compatible).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GalConfig {}

impl GalConfig {
    /// Load configuration from `~/.gal/config/config.json`.
    ///
    /// Returns the default configuration if:
    /// - The file does not exist
    /// - The file cannot be read
    /// - The JSON is malformed
    ///
    /// No panics.
    pub fn load() -> Self {
        let config_path = match Self::config_path() {
            Ok(path) => path,
            Err(_) => return GalConfig::default(),
        };

        Self::load_from_path(&config_path)
    }

    /// Load configuration from a specific path.
    ///
    /// Returns the default configuration if the file does not exist, cannot
    /// be read, or contains invalid JSON.
    pub fn load_from_path(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str::<GalConfig>(&content).unwrap_or_default(),
            Err(_) => GalConfig::default(),
        }
    }

    /// Get the expected path to the config file: `~/.gal/config/config.json`.
    pub fn config_path() -> io::Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "cannot determine home directory")
        })?;

        Ok(home.join(".gal").join("config").join("config.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_unknown_keys_ignored() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, r#"{{"devMode": true, "galRoot": "/path/to/repo"}}"#).unwrap();
        temp.flush().unwrap();
        // Unknown/removed keys are silently ignored; load must succeed.
        let _config = GalConfig::load_from_path(temp.path());
    }

    #[test]
    fn test_load_missing_file_returns_default() {
        let _config = GalConfig::load_from_path(Path::new("/nonexistent/config.json"));
    }

    // ── MACHINE_PATH_REGISTRY + machine_path_value ───────────────────────────

    #[test]
    fn registry_contains_only_gal_skills() {
        assert_eq!(MACHINE_PATH_REGISTRY, &[("galSkills", "GAL_SKILLS")]);
    }

    #[test]
    fn registry_excludes_retired_keys() {
        let keys: Vec<&str> = MACHINE_PATH_REGISTRY.iter().map(|(cp, _)| *cp).collect();
        assert!(!keys.iter().any(|key| key.starts_with("obsidian.")));
        assert!(!keys.contains(&"research.localSearchProject"));
        assert!(!keys.contains(&"research.privateResearchDir"));
        assert!(!keys.contains(&"tempDir"));
    }

    #[test]
    fn registry_excludes_credential_keys() {
        for (_, snake) in MACHINE_PATH_REGISTRY {
            assert!(
                !snake.ends_with("_API_KEY") && !snake.ends_with("_TOKEN"),
                "credential key found in registry: {snake}"
            );
        }
    }

    #[test]
    fn registry_snake_for_gal_skills() {
        let found = MACHINE_PATH_REGISTRY
            .iter()
            .find(|(cp, _)| *cp == "galSkills");
        assert_eq!(found.map(|(_, s)| *s), Some("GAL_SKILLS"));
    }

    #[test]
    fn machine_path_value_flat_key() {
        let config = serde_json::json!({ "galSkills": "/home/user/.gal/skills" });
        assert_eq!(
            machine_path_value(&config, "galSkills"),
            Some("/home/user/.gal/skills".to_owned())
        );
    }

    #[test]
    fn machine_path_value_gal_skills() {
        let config = serde_json::json!({
            "galSkills": "/Users/user/.gal/skills"
        });
        assert_eq!(
            machine_path_value(&config, "galSkills"),
            Some("/Users/user/.gal/skills".to_owned())
        );
    }

    #[test]
    fn machine_path_value_missing_key_returns_none() {
        let config = serde_json::json!({});
        assert_eq!(machine_path_value(&config, "galSkills"), None);
    }

    #[test]
    fn machine_path_value_non_string_returns_none() {
        let config = serde_json::json!({ "galSkills": 42 });
        assert_eq!(machine_path_value(&config, "galSkills"), None);
    }

}
