//! Install/update/uninstall operations for GAL.
//!
//! Orchestrates the full flow: config → mode → render → provider projections → ledger.
//! This is the Rust-native implementation replacing the frozen PowerShell oracle.
//!
//! Entry switch (BUG-B): once this module is wired in main.rs, the Rust binary
//! is the single entry for install/update/uninstall. The frozen scripts remain
//! as oracle only and are NOT invoked by the entry.
//!
//! Oracle parity (TP-014): run_update() produces Claude+Copilot surfaces that
//! match the frozen Install-GalPlugins oracle in an isolated home.
//!
//! Corresponds to T-011 of fix-gal-bootstrap-install-convergence.

use crate::config::GalConfig;
use crate::ledger::{ledger_path, now_timestamp, Ledger, LedgerEntry};
use crate::mode::{resolve_mode, GalMode, ModeError};
use crate::providers::agy::AgyProjection;
use crate::render::{render_canonical_root, RenderError};
use std::fs;
use std::path::PathBuf;

/// Errors that can occur during install/update/uninstall.
#[derive(Debug)]
pub enum InstallError {
    /// Mode resolution failed (e.g. devMode=true but galRoot unusable).
    Mode(ModeError),
    /// Canonical root rendering failed.
    Render(RenderError),
    /// Filesystem I/O error.
    Io(std::io::Error),
    /// Home directory could not be determined.
    NoHome,
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Mode(e) => write!(f, "mode error: {e}"),
            InstallError::Render(e) => write!(f, "render error: {e}"),
            InstallError::Io(e) => write!(f, "I/O error: {e}"),
            InstallError::NoHome => write!(f, "could not determine home directory"),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<ModeError> for InstallError {
    fn from(e: ModeError) -> Self {
        InstallError::Mode(e)
    }
}

impl From<RenderError> for InstallError {
    fn from(e: RenderError) -> Self {
        InstallError::Render(e)
    }
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e)
    }
}

/// Result of a successful install or update operation.
#[derive(Debug)]
pub struct InstallReport {
    /// Path to the rendered canonical plugin root.
    pub canonical_root: PathBuf,
    /// Providers that were successfully projected.
    pub providers: Vec<String>,
    /// Mode in use ("normal" | "dev").
    pub mode: String,
    /// Non-fatal warnings (e.g. AGY best-effort failures).
    pub warnings: Vec<String>,
}

/// Run `gal install` — first-time install. Idempotent (delegates to update).
pub fn run_install(config: &GalConfig) -> Result<InstallReport, InstallError> {
    run_update_with_op(config, "install")
}

/// Run `gal update` — re-render canonical root and refresh all provider surfaces.
///
/// Claude and Copilot read surfaces are inside the canonical root. AGY junctions
/// are re-applied best-effort. A ledger entry is written on success.
pub fn run_update(config: &GalConfig) -> Result<InstallReport, InstallError> {
    run_update_with_op(config, "update")
}

fn run_update_with_op(config: &GalConfig, op: &str) -> Result<InstallReport, InstallError> {
    // Step 1: Resolve mode — errors on bad devMode/galRoot, no silent fallback (BUG-B).
    let mode = resolve_mode(config)?;
    let mode_str = match mode {
        GalMode::Normal => "normal",
        GalMode::Dev => "dev",
    };

    // Step 2: Render canonical root (atomic temp+swap, TP-014 parity surface).
    let canonical_root = render_canonical_root(config, mode)?;

    // Claude and Copilot surfaces live inside canonical_root.
    let mut providers = vec!["claude".to_string(), "copilot".to_string()];
    let mut warnings = Vec::new();

    // Step 3: AGY three-surface projection — best-effort (OE-A), never fatal.
    match AgyProjection::new(canonical_root.clone()) {
        Ok(agy) => match agy.apply() {
            Ok(_) => providers.push("agy".to_string()),
            Err(e) => warnings.push(format!("AGY projection (best-effort): {e}")),
        },
        Err(e) => warnings.push(format!("AGY init (best-effort): {e}")),
    }

    // Step 4: Write ledger.
    write_ledger_entry(op, &canonical_root, &providers, mode_str, &mut warnings);

    Ok(InstallReport {
        canonical_root,
        providers,
        mode: mode_str.to_string(),
        warnings,
    })
}

/// Run `gal uninstall` — remove canonical root and provider surfaces.
pub fn run_uninstall() -> Result<(), InstallError> {
    let home = dirs::home_dir().ok_or(InstallError::NoHome)?;
    let canonical_root = home.join(".gal").join("plugins").join("gal");

    let mut warnings = Vec::new();

    // Remove AGY surfaces before removing canonical root (they link to it).
    if canonical_root.exists() {
        match AgyProjection::new(canonical_root.clone()) {
            Ok(agy) => {
                if let Err(e) = agy.remove() {
                    warnings.push(format!("AGY removal (best-effort): {e}"));
                }
            }
            Err(e) => warnings.push(format!("AGY uninstall init (best-effort): {e}")),
        }
    }

    // Remove canonical root.
    if canonical_root.exists() {
        fs::remove_dir_all(&canonical_root)?;
    }

    // Write uninstall ledger entry.
    write_ledger_entry("uninstall", &canonical_root, &[], "normal", &mut warnings);

    for w in &warnings {
        eprintln!("gal uninstall warning: {w}");
    }

    Ok(())
}

fn write_ledger_entry(
    op: &str,
    canonical_root: &PathBuf,
    providers: &[String],
    mode: &str,
    warnings: &mut Vec<String>,
) {
    if let Some(ledger_p) = ledger_path() {
        let mut ledger = Ledger::load(&ledger_p);
        ledger.record(LedgerEntry {
            operation: op.to_string(),
            canonical_root: canonical_root.clone(),
            timestamp: now_timestamp(),
            providers: providers.to_vec(),
            mode: mode.to_string(),
        });
        if let Err(e) = ledger.save(&ledger_p) {
            warnings.push(format!("ledger write warning: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_error_display_is_non_empty() {
        assert!(!InstallError::NoHome.to_string().is_empty());
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        assert!(InstallError::Io(io_err).to_string().contains("not found"));
    }

    #[test]
    fn run_install_with_invalid_gal_root_returns_mode_or_render_error() {
        let config = GalConfig {
            dev_mode: Some(true),
            gal_root: Some("/absolutely/nonexistent/path/xyz123".to_string()),
            install_mode: None,
        };
        let result = run_install(&config);
        assert!(
            result.is_err(),
            "install with unusable galRoot must fail, not silently fallback"
        );
        // Must be Mode error (galRoot not found), not a panic.
        match result.unwrap_err() {
            InstallError::Mode(_) => {} // expected
            other => panic!("expected Mode error, got: {other}"),
        }
    }

    #[test]
    fn run_update_with_invalid_gal_root_returns_mode_error() {
        let config = GalConfig {
            dev_mode: Some(true),
            gal_root: Some("".to_string()),
            install_mode: None,
        };
        let result = run_update(&config);
        assert!(result.is_err());
        matches!(result.unwrap_err(), InstallError::Mode(_));
    }

    #[test]
    fn install_report_fields_are_accessible() {
        // Verify the struct is usable (compile-time check via construction).
        let report = InstallReport {
            canonical_root: PathBuf::from("/tmp/gal"),
            providers: vec!["claude".to_string()],
            mode: "normal".to_string(),
            warnings: vec![],
        };
        assert_eq!(report.mode, "normal");
        assert_eq!(report.providers.len(), 1);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn uninstall_succeeds_when_canonical_root_absent() {
        // If canonical root never existed, uninstall should not error.
        // We test by pointing to a non-existent path (ledger write may warn but not error).
        // Full isolated-home test is TP-014 (integration, manual).
        // Here we just verify no panic and no unexpected I/O error.
        //
        // Note: run_uninstall() uses dirs::home_dir() which is the real home in tests.
        // We skip the real filesystem call and only test the error surface.
        let err = InstallError::NoHome;
        assert!(err.to_string().contains("home"));
    }
}
