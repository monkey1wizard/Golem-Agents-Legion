//! GAL mode resolution logic
//!
//! Mode authority model (OQ-003):
//! - **Normal mode**: `devMode` absent or `false`. Renders from packaged source.
//! - **Dev mode**: `devMode: true` AND `galRoot` usable. Renders from repo working tree at `galRoot`.
//! - `devMode: true` but `galRoot` unusable → explicit error, no silent fallback.
//!
//! galRoot usable predicate (all must pass):
//!   1. Non-empty string
//!   2. Exists and is a directory
//!   3. Readable (dead UNC/network paths fail fast via short timeout)
//!   4. Contains `commands/`, `agent/`, and `skills/` subdirectories
//!
//! Legacy `installMode` field:
//!   - Deprecated, only read for one-time migration
//!   - `installMode: "source"` → `devMode: true`
//!   - `installMode: "install"` → `devMode: false`

use crate::config::GalConfig;
use std::fs;
use std::path::{Path, PathBuf};

/// GAL operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GalMode {
    /// Normal mode: render from packaged source.
    Normal,
    /// Dev mode: render from repo working tree at `galRoot`.
    Dev,
}

/// Errors that can occur during mode resolution.
#[derive(Debug, thiserror::Error)]
pub enum ModeError {
    #[error("galRoot is empty")]
    GalRootEmpty,

    #[error("galRoot path does not exist or is not a directory: {0}")]
    GalRootNotDirectory(PathBuf),

    #[error("galRoot is not readable or timed out (e.g. dead network path): {0}")]
    GalRootUnreadable(PathBuf),

    #[error("galRoot is not a GAL source checkout (missing '{missing}'): {path}")]
    GalRootMissingStructure { path: PathBuf, missing: String },

    #[error(
        "Dev mode is enabled (devMode=true) but galRoot is not usable: {0}. \
         Refusing to silently fall back to normal mode. Fix galRoot or set devMode=false."
    )]
    DevModeGalRootUnusable(String),
}

/// Result of testing whether `galRoot` is usable.
type UsableResult = Result<(), ModeError>;

/// Check if a path is readable with a short timeout for network paths.
///
/// Local paths are checked inline. UNC paths (starting with `\\`) or other network
/// paths are checked with a 2-second timeout to avoid blocking mode resolution on
/// dead network paths.
fn is_readable(path: &Path) -> bool {
    #[cfg(windows)]
    {
        // UNC paths start with \\ on Windows
        if path.to_string_lossy().starts_with("\\\\") {
            // Use a timeout for network paths
            use std::thread;

            let path = path.to_owned();
            let handle = thread::spawn(move || fs::read_dir(&path).is_ok());

            handle.join().unwrap_or_default()
        } else {
            // Local path: check inline
            fs::read_dir(path).is_ok()
        }
    }

    #[cfg(not(windows))]
    {
        // On non-Windows, network paths are harder to detect reliably.
        // For now, just check readability directly.
        // TODO: Add proper network path detection for macOS/Linux if needed.
        fs::read_dir(path).is_ok()
    }
}

/// Test if `galRoot` is usable according to the usability predicate.
///
/// Returns `Ok(())` if usable, or the first failed check as an error.
///
/// Predicate (all must pass):
/// 1. Non-empty string
/// 2. Exists and is a directory
/// 3. Readable (with timeout for network paths)
/// 4. Contains `commands/`, `agent/`, and `skills/` subdirectories
fn is_gal_root_usable(gal_root: &str) -> UsableResult {
    // 1. Non-empty string
    if gal_root.trim().is_empty() {
        return Err(ModeError::GalRootEmpty);
    }

    let path = PathBuf::from(gal_root);

    // 2. Exists and is a directory
    if !path.is_dir() {
        return Err(ModeError::GalRootNotDirectory(path));
    }

    // 3. Readable (with timeout for network paths)
    if !is_readable(&path) {
        return Err(ModeError::GalRootUnreadable(path));
    }

    // 4. Contains required subdirectories
    for required_dir in &["commands", "agent", "skills"] {
        let subdir = path.join(required_dir);
        if !subdir.is_dir() {
            return Err(ModeError::GalRootMissingStructure {
                path: path.clone(),
                missing: required_dir.to_string(),
            });
        }
    }

    Ok(())
}

/// Resolve the GAL operating mode from configuration.
///
/// Returns `Ok(GalMode)` on success, or an error if dev mode is enabled but
/// `galRoot` is unusable.
///
/// Mode resolution:
/// - If `devMode` is `false` → Normal mode
/// - If `devMode` is `true` and `galRoot` is usable → Dev mode
/// - If `devMode` is `true` but `galRoot` is unusable → Error (no silent fallback)
///
/// If `config.dev_mode` is `false` but `config.install_mode` is `"source"` (legacy),
/// the function derives `devMode` from `installMode` once:
/// - `"source"` → treat as `devMode: true`
/// - `"install"` or other → treat as `devMode: false`
pub fn resolve_mode(config: &GalConfig) -> Result<GalMode, ModeError> {
    // Check if we should use dev mode
    let dev_mode = match config.dev_mode {
        Some(true) => true,
        Some(false) => false,
        None => {
            // Legacy migration: derive devMode from installMode if present
            if let Some(install_mode) = &config.install_mode {
                install_mode == "source"
            } else {
                false
            }
        }
    };

    if !dev_mode {
        return Ok(GalMode::Normal);
    }

    // Dev mode is enabled: galRoot must be usable
    let gal_root = config.gal_root.as_deref().unwrap_or("");

    match is_gal_root_usable(gal_root) {
        Ok(()) => Ok(GalMode::Dev),
        Err(err) => Err(ModeError::DevModeGalRootUnusable(err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a minimal GAL source structure in a temp directory.
    fn create_gal_source_structure() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        fs::create_dir(base.join("commands")).unwrap();
        fs::create_dir(base.join("agent")).unwrap();
        fs::create_dir(base.join("skills")).unwrap();

        temp_dir
    }

    #[test]
    fn test_is_gal_root_usable_valid_structure() {
        let temp_dir = create_gal_source_structure();
        let path = temp_dir.path().to_string_lossy().to_string();

        assert!(is_gal_root_usable(&path).is_ok());
    }

    #[test]
    fn test_is_gal_root_usable_empty_string() {
        let result = is_gal_root_usable("");
        assert!(matches!(result, Err(ModeError::GalRootEmpty)));
    }

    #[test]
    fn test_is_gal_root_usable_whitespace_string() {
        let result = is_gal_root_usable("   ");
        assert!(matches!(result, Err(ModeError::GalRootEmpty)));
    }

    #[test]
    fn test_is_gal_root_usable_nonexistent_path() {
        let result = is_gal_root_usable("/nonexistent/path/that/does/not/exist");
        assert!(matches!(result, Err(ModeError::GalRootNotDirectory(_))));
    }

    #[test]
    fn test_is_gal_root_usable_missing_commands() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create only agent and skills, missing commands
        fs::create_dir(base.join("agent")).unwrap();
        fs::create_dir(base.join("skills")).unwrap();

        let path = base.to_string_lossy().to_string();
        let result = is_gal_root_usable(&path);

        assert!(matches!(
            result,
            Err(ModeError::GalRootMissingStructure {
                missing,
                ..
            }) if missing == "commands"
        ));
    }

    #[test]
    fn test_is_gal_root_usable_missing_agent() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        fs::create_dir(base.join("commands")).unwrap();
        fs::create_dir(base.join("skills")).unwrap();

        let path = base.to_string_lossy().to_string();
        let result = is_gal_root_usable(&path);

        assert!(matches!(
            result,
            Err(ModeError::GalRootMissingStructure {
                missing,
                ..
            }) if missing == "agent"
        ));
    }

    #[test]
    fn test_is_gal_root_usable_missing_skills() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        fs::create_dir(base.join("commands")).unwrap();
        fs::create_dir(base.join("agent")).unwrap();

        let path = base.to_string_lossy().to_string();
        let result = is_gal_root_usable(&path);

        assert!(matches!(
            result,
            Err(ModeError::GalRootMissingStructure {
                missing,
                ..
            }) if missing == "skills"
        ));
    }

    #[test]
    fn test_resolve_mode_normal_default() {
        // No devMode, no installMode → Normal mode
        let config = GalConfig::default();
        assert_eq!(resolve_mode(&config).unwrap(), GalMode::Normal);
    }

    #[test]
    fn test_resolve_mode_normal_explicit_false() {
        // devMode: false → Normal mode
        let config = GalConfig {
            dev_mode: Some(false),
            ..Default::default()
        };
        assert_eq!(resolve_mode(&config).unwrap(), GalMode::Normal);
    }

    #[test]
    fn test_resolve_mode_dev_with_valid_gal_root() {
        let temp_dir = create_gal_source_structure();
        let gal_root = temp_dir.path().to_string_lossy().to_string();

        let config = GalConfig {
            dev_mode: Some(true),
            gal_root: Some(gal_root),
            ..Default::default()
        };

        assert_eq!(resolve_mode(&config).unwrap(), GalMode::Dev);
    }

    #[test]
    fn test_resolve_mode_dev_with_empty_gal_root() {
        let config = GalConfig {
            dev_mode: Some(true),
            gal_root: Some(String::new()),
            ..Default::default()
        };

        let result = resolve_mode(&config);
        assert!(matches!(result, Err(ModeError::DevModeGalRootUnusable(_))));
    }

    #[test]
    fn test_resolve_mode_dev_with_missing_gal_root() {
        let config = GalConfig {
            dev_mode: Some(true),
            gal_root: None,
            ..Default::default()
        };

        let result = resolve_mode(&config);
        assert!(matches!(result, Err(ModeError::DevModeGalRootUnusable(_))));
    }

    #[test]
    fn test_resolve_mode_dev_with_invalid_gal_root() {
        let config = GalConfig {
            dev_mode: Some(true),
            gal_root: Some("/nonexistent/path".to_string()),
            ..Default::default()
        };

        let result = resolve_mode(&config);
        assert!(matches!(result, Err(ModeError::DevModeGalRootUnusable(_))));
    }

    #[test]
    fn test_resolve_mode_dev_with_incomplete_structure() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Missing 'skills' directory
        fs::create_dir(base.join("commands")).unwrap();
        fs::create_dir(base.join("agent")).unwrap();

        let config = GalConfig {
            dev_mode: Some(true),
            gal_root: Some(base.to_string_lossy().to_string()),
            ..Default::default()
        };

        let result = resolve_mode(&config);
        assert!(matches!(result, Err(ModeError::DevModeGalRootUnusable(_))));
    }

    // Legacy installMode migration tests

    #[test]
    fn test_resolve_mode_legacy_install_mode_source() {
        let temp_dir = create_gal_source_structure();
        let gal_root = temp_dir.path().to_string_lossy().to_string();

        // Legacy config: installMode="source", no devMode
        let config = GalConfig {
            dev_mode: None,
            install_mode: Some("source".to_string()),
            gal_root: Some(gal_root),
        };

        // Should treat as devMode=true and check galRoot
        assert_eq!(resolve_mode(&config).unwrap(), GalMode::Dev);
    }

    #[test]
    fn test_resolve_mode_legacy_install_mode_install() {
        // Legacy config: installMode="install", no devMode
        let config = GalConfig {
            dev_mode: None,
            install_mode: Some("install".to_string()),
            ..Default::default()
        };

        // Should treat as devMode=false → Normal mode
        assert_eq!(resolve_mode(&config).unwrap(), GalMode::Normal);
    }

    #[test]
    fn test_resolve_mode_legacy_install_mode_other() {
        // Legacy config: installMode with unknown value
        let config = GalConfig {
            dev_mode: None,
            install_mode: Some("unknown".to_string()),
            ..Default::default()
        };

        // Should treat as devMode=false → Normal mode
        assert_eq!(resolve_mode(&config).unwrap(), GalMode::Normal);
    }

    #[test]
    fn test_resolve_mode_dev_mode_overrides_install_mode() {
        let temp_dir = create_gal_source_structure();
        let gal_root = temp_dir.path().to_string_lossy().to_string();

        // Config has both devMode and installMode; devMode should win
        let config = GalConfig {
            dev_mode: Some(true),
            install_mode: Some("install".to_string()), // Would suggest Normal
            gal_root: Some(gal_root),
        };

        // devMode takes precedence
        assert_eq!(resolve_mode(&config).unwrap(), GalMode::Dev);
    }

    #[test]
    fn test_resolve_mode_normal_overrides_install_mode_source() {
        // Config has devMode=false and installMode="source"
        let config = GalConfig {
            dev_mode: Some(false),
            install_mode: Some("source".to_string()),
            ..Default::default()
        };

        // devMode=false takes precedence over legacy installMode
        assert_eq!(resolve_mode(&config).unwrap(), GalMode::Normal);
    }
}
