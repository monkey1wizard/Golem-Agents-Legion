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
//!   4. Contains `commands/`, `agents/`, and `skills/` subdirectories, either
//!      directly (galRoot already points at gal-core) OR under `plugins/gal-core/`
//!      (galRoot is the repo root — auto-resolved, aligning with the frozen Bash oracle
//!      which appends `plugins/gal-core` internally via `provider-plugin.sh:413`).
//!
//! galRoot resolution (R-10/RC-6):
//!   `resolve_gal_source_root(galRoot)` returns the actual source directory:
//!   - If galRoot already contains the required dirs → returns galRoot (old form tolerated)
//!   - If galRoot/plugins/gal-core contains the required dirs → returns that sub-path
//!   - Otherwise → error
//!
//! Legacy `installMode` field:
//!   - Deprecated, only read for one-time migration
//!   - `installMode: "source"` → `devMode: true`
//!   - `installMode: "install"` → `devMode: false`

use crate::config::GalConfig;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

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

/// Check if a path is readable with a short timeout for network/UNC paths.
///
/// Local paths are checked inline. UNC paths (Windows `\\`) use a channel-based
/// 2-second timeout so a dead network path fails fast instead of blocking mode
/// resolution indefinitely (FU-04).
fn is_readable(path: &Path) -> bool {
    #[cfg(windows)]
    {
        if path.to_string_lossy().starts_with("\\\\") {
            let path_owned = path.to_owned();
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(fs::read_dir(&path_owned).is_ok());
            });
            rx.recv_timeout(Duration::from_secs(2)).unwrap_or(false)
        } else {
            fs::read_dir(path).is_ok()
        }
    }

    #[cfg(not(windows))]
    {
        fs::read_dir(path).is_ok()
    }
}

/// Required subdirectories that identify a GAL source (gal-core) root.
const REQUIRED_DIRS: &[&str] = &["commands", "agents", "skills"];

/// Returns the first missing required subdirectory name, or `None` if all present.
fn first_missing_dir(dir: &Path) -> Option<String> {
    REQUIRED_DIRS
        .iter()
        .find(|&&d| !dir.join(d).is_dir())
        .map(|&d| d.to_string())
}

/// Resolve the actual GAL source root from a raw `galRoot` config value.
///
/// Accepts two forms (R-10/RC-6):
/// - **gal-core form** (old): galRoot already points at `plugins/gal-core` (or any
///   directory that directly contains `commands/`, `agents/`, `skills/`). Returned as-is.
/// - **repo-root form** (canonical): galRoot points at the repo root, which contains
///   `plugins/gal-core/` with the required structure. The sub-path is returned.
///
/// This aligns with the frozen Bash oracle (`provider-plugin.sh:413`) which appends
/// `plugins/gal-core` internally, so both old configs and repo-root configs work.
///
/// Returns the resolved `PathBuf` on success, or the first failing `ModeError`.
pub fn resolve_gal_source_root(gal_root_str: &str) -> Result<PathBuf, ModeError> {
    if gal_root_str.trim().is_empty() {
        return Err(ModeError::GalRootEmpty);
    }

    let path = PathBuf::from(gal_root_str);

    if !path.is_dir() {
        return Err(ModeError::GalRootNotDirectory(path));
    }

    if !is_readable(&path) {
        return Err(ModeError::GalRootUnreadable(path));
    }

    // Old form: galRoot already points at gal-core (has required dirs directly).
    if let None = first_missing_dir(&path) {
        return Ok(path);
    }

    // Repo-root form: try plugins/gal-core sub-path (aligns with Bash oracle).
    let sub = path.join("plugins").join("gal-core");
    if sub.is_dir() {
        if let Some(missing) = first_missing_dir(&sub) {
            return Err(ModeError::GalRootMissingStructure {
                path: sub,
                missing,
            });
        }
        return Ok(sub);
    }

    // Neither form has the required structure — report from the original path.
    let missing = first_missing_dir(&path).unwrap_or_else(|| "commands".to_string());
    Err(ModeError::GalRootMissingStructure { path, missing })
}

/// Test if `galRoot` is usable (delegates to `resolve_gal_source_root`).
fn is_gal_root_usable(gal_root: &str) -> UsableResult {
    resolve_gal_source_root(gal_root).map(|_| ())
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

// T-009 reparent note: `scripts/tests/Test-InstallModeAuthority.ps1` is the oracle
// script for install-mode authority behavior.  The tests below are the fixture-based
// Rust replacement.  `cargo test` does NOT spawn the PS1 script — all coverage comes
// from TempDir-fixture-based assertions here (TP-10).
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
        fs::create_dir(base.join("agents")).unwrap();
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

        // Create only agents and skills, missing commands
        fs::create_dir(base.join("agents")).unwrap();
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
    fn test_is_gal_root_usable_missing_agents() {
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
            }) if missing == "agents"
        ));
    }

    #[test]
    fn test_is_gal_root_usable_missing_skills() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        fs::create_dir(base.join("commands")).unwrap();
        fs::create_dir(base.join("agents")).unwrap();

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
        fs::create_dir(base.join("agents")).unwrap();

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

    // ── R-10 / RC-6: resolve_gal_source_root repo-root form ─────────────────

    /// Helper: create a repo-root structure (source lives under plugins/gal-core/).
    fn create_repo_root_structure() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();
        let gal_core = base.join("plugins").join("gal-core");
        fs::create_dir_all(&gal_core).unwrap();
        fs::create_dir(gal_core.join("commands")).unwrap();
        fs::create_dir(gal_core.join("agents")).unwrap();
        fs::create_dir(gal_core.join("skills")).unwrap();
        temp_dir
    }

    #[test]
    fn test_resolve_gal_source_root_old_form_tolerated() {
        // galRoot already IS gal-core (old workaround form) — should return as-is.
        let temp_dir = create_gal_source_structure();
        let path_str = temp_dir.path().to_string_lossy().to_string();
        let result = resolve_gal_source_root(&path_str).unwrap();
        assert_eq!(result, temp_dir.path());
    }

    #[test]
    fn test_resolve_gal_source_root_repo_root_form() {
        // galRoot is repo root containing plugins/gal-core/ — should resolve to sub-path.
        let temp_dir = create_repo_root_structure();
        let repo_root = temp_dir.path().to_string_lossy().to_string();
        let result = resolve_gal_source_root(&repo_root).unwrap();
        assert_eq!(result, temp_dir.path().join("plugins").join("gal-core"));
    }

    #[test]
    fn test_resolve_gal_source_root_repo_root_no_double_append() {
        // galRoot is gal-core itself — should NOT append plugins/gal-core again.
        let temp_dir = create_gal_source_structure();
        let gal_core_str = temp_dir.path().to_string_lossy().to_string();
        let result = resolve_gal_source_root(&gal_core_str).unwrap();
        // Result must equal gal_core itself, not gal_core/plugins/gal-core.
        assert_eq!(result, temp_dir.path());
        assert!(!result.ends_with("plugins/gal-core"));
    }

    #[test]
    fn test_resolve_mode_dev_with_repo_root_gal_root() {
        // devMode=true, galRoot = repo root (not gal-core) — must still resolve to Dev.
        let temp_dir = create_repo_root_structure();
        let repo_root = temp_dir.path().to_string_lossy().to_string();
        let config = GalConfig {
            dev_mode: Some(true),
            gal_root: Some(repo_root),
            ..Default::default()
        };
        assert_eq!(resolve_mode(&config).unwrap(), GalMode::Dev);
    }

    #[test]
    fn test_tp09_repo_root_galroot_resolves_and_converges() {
        // TP-09: galRoot = repo root installs successfully; old gal-core form does not double-append.
        let repo_dir = create_repo_root_structure();
        let repo_root = repo_dir.path().to_string_lossy().to_string();

        // Repo-root form resolves to plugins/gal-core.
        let resolved_from_repo = resolve_gal_source_root(&repo_root).unwrap();
        assert_eq!(
            resolved_from_repo,
            repo_dir.path().join("plugins").join("gal-core")
        );

        // Old gal-core form resolves to itself (no double-append).
        let gal_core_str = resolved_from_repo.to_string_lossy().to_string();
        let resolved_from_gal_core = resolve_gal_source_root(&gal_core_str).unwrap();
        assert_eq!(resolved_from_gal_core, resolved_from_repo);
    }
}
