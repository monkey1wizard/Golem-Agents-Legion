//! Configuration parsing and management for GAL.
//!
//! Rust owns config truth. The config file is `~/.gal/config/config.json`.
//! Missing file or keys produce explicit defaults; no panics.
//!
//! Mode authority (R-003):
//! - Normal mode: `devMode` absent or `false`. Renders from packaged source.
//! - Dev mode: `devMode: true` AND `galRoot` usable. Renders from working tree.
//! - `devMode: true` but `galRoot` unusable → explicit error, no fallback.
//!
//! The `installMode` field is deprecated. If present, it may be used for
//! migration hints but does not override `devMode`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The complete GAL configuration model.
///
/// Corresponds to `~/.gal/config/config.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct GalConfig {
    /// Developer mode flag. When `true`, GAL renders from the working tree
    /// specified by `galRoot`. When `false` or absent, GAL renders from
    /// packaged source (normal mode). `None` means the field was not present
    /// in the JSON file, which enables legacy migration logic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_mode: Option<bool>,

    /// Path to the GAL repository working tree. Only used when `dev_mode` is
    /// `true`. If `dev_mode` is `true` but `gal_root` is not usable, GAL will
    /// error rather than silently fall back to normal mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gal_root: Option<String>,

    /// Deprecated field. If present, it may be used for migration hints but
    /// does not control mode authority. `devMode` and `galRoot` are the
    /// authoritative fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_mode: Option<String>,
}

impl Default for GalConfig {
    /// Default configuration: normal mode (dev_mode = None), no galRoot.
    fn default() -> Self {
        GalConfig {
            dev_mode: None,
            gal_root: None,
            install_mode: None,
        }
    }
}

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
            Ok(content) => match serde_json::from_str::<GalConfig>(&content) {
                Ok(config) => config,
                Err(_) => GalConfig::default(),
            },
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

    /// Check if the configuration is in dev mode.
    pub fn is_dev_mode(&self) -> bool {
        self.dev_mode.unwrap_or(false)
    }

    /// Get the galRoot path if set.
    pub fn gal_root(&self) -> Option<&str> {
        self.gal_root.as_deref()
    }

    /// Check if the deprecated installMode field is present.
    ///
    /// Used for migration warnings in `gal doctor`.
    pub fn has_deprecated_install_mode(&self) -> bool {
        self.install_mode.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = GalConfig::default();
        assert!(!config.is_dev_mode());
        assert_eq!(config.gal_root(), None);
        assert!(!config.has_deprecated_install_mode());
    }

    #[test]
    fn test_load_missing_file() {
        let config = GalConfig::load_from_path(Path::new("/nonexistent/config.json"));
        assert!(!config.is_dev_mode());
        assert_eq!(config.gal_root(), None);
    }

    #[test]
    fn test_load_invalid_json() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "{{ invalid json").unwrap();
        temp.flush().unwrap();

        let config = GalConfig::load_from_path(temp.path());
        assert!(!config.is_dev_mode());
        assert_eq!(config.gal_root(), None);
    }

    #[test]
    fn test_load_normal_mode() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, r#"{{"devMode": false}}"#).unwrap();
        temp.flush().unwrap();

        let config = GalConfig::load_from_path(temp.path());
        assert!(!config.is_dev_mode());
        assert_eq!(config.gal_root(), None);
    }

    #[test]
    fn test_load_dev_mode_with_root() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(
            temp,
            r#"{{"devMode": true, "galRoot": "/path/to/repo"}}"#
        )
        .unwrap();
        temp.flush().unwrap();

        let config = GalConfig::load_from_path(temp.path());
        assert!(config.is_dev_mode());
        assert_eq!(config.gal_root(), Some("/path/to/repo"));
    }

    #[test]
    fn test_load_empty_object() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "{{}}").unwrap();
        temp.flush().unwrap();

        let config = GalConfig::load_from_path(temp.path());
        assert!(!config.is_dev_mode());
        assert_eq!(config.gal_root(), None);
    }

    #[test]
    fn test_deprecated_install_mode() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(
            temp,
            r#"{{"devMode": true, "galRoot": "/path", "installMode": "dev"}}"#
        )
        .unwrap();
        temp.flush().unwrap();

        let config = GalConfig::load_from_path(temp.path());
        assert!(config.is_dev_mode());
        assert!(config.has_deprecated_install_mode());
    }

    #[test]
    fn test_missing_keys_use_defaults() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, r#"{{"galRoot": "/some/path"}}"#).unwrap();
        temp.flush().unwrap();

        let config = GalConfig::load_from_path(temp.path());
        // devMode defaults to false when not present
        assert!(!config.is_dev_mode());
        assert_eq!(config.gal_root(), Some("/some/path"));
    }

    #[test]
    fn test_case_sensitivity() {
        // serde should handle camelCase properly
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, r#"{{"devMode": true, "galRoot": "/test"}}"#).unwrap();
        temp.flush().unwrap();

        let config = GalConfig::load_from_path(temp.path());
        assert!(config.is_dev_mode());
        assert_eq!(config.gal_root(), Some("/test"));
    }
}
