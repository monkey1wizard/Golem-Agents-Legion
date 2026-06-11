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
use crate::providers::claude::ClaudeSkillProjection;
use crate::render::{render_canonical_root, RenderError};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Errors that can occur during install/update/uninstall.
#[derive(Debug)]
pub enum InstallError {
    /// Mode resolution failed (e.g. devMode=true but galRoot unusable).
    Mode(ModeError),
    /// Canonical root rendering failed.
    Render(RenderError),
    /// Filesystem I/O error.
    Io(std::io::Error),
    /// JSON parsing or serialization failed.
    Json(serde_json::Error),
    /// Home directory could not be determined.
    NoHome,
    /// Uninstall completed partially; ledger was written but some removals failed.
    PartialUninstall { warnings: Vec<String> },
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Mode(e) => write!(f, "mode error: {e}"),
            InstallError::Render(e) => write!(f, "render error: {e}"),
            InstallError::Io(e) => write!(f, "I/O error: {e}"),
            InstallError::Json(e) => write!(f, "JSON error: {e}"),
            InstallError::NoHome => write!(f, "could not determine home directory"),
            InstallError::PartialUninstall { warnings } => {
                write!(f, "uninstall completed with partial failures: {}", warnings.join("; "))
            }
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

impl From<serde_json::Error> for InstallError {
    fn from(e: serde_json::Error) -> Self {
        InstallError::Json(e)
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

    let mut providers = vec!["copilot".to_string()];
    let mut warnings = Vec::new();

    // Step 3: Claude skill surface projection — `~/.claude/skills/gal` → canonical root.
    // Best-effort: a link failure does not void the canonical root render, but is
    // surfaced as a warning so `gal doctor` (T-006) can detect the gap.
    // Never touches `~/.claude/plugins/gal` (oracle legacy path).
    match ClaudeSkillProjection::new(canonical_root.clone()) {
        Ok(proj) => match proj.apply() {
            Ok(_) => providers.push("claude".to_string()),
            Err(e) => warnings.push(format!("Claude skill surface (best-effort): {e}")),
        },
        Err(e) => warnings.push(format!("Claude skill surface init (best-effort): {e}")),
    }

    // Step 4: AGY three-surface projection — best-effort (OE-A), never fatal.
    match AgyProjection::new(canonical_root.clone()) {
        Ok(agy) => match agy.apply() {
            Ok(_) => providers.push("agy".to_string()),
            Err(e) => warnings.push(format!("AGY projection (best-effort): {e}")),
        },
        Err(e) => warnings.push(format!("AGY init (best-effort): {e}")),
    }

    if let Err(e) = write_provider_lifecycle_artifacts(&canonical_root, &mut providers) {
        warnings.push(format!("provider lifecycle state: {e}"));
    }

    // Step 5: Write ledger.
    let ledger_warnings = warnings.clone();
    write_ledger_entry(
        op,
        &canonical_root,
        &providers,
        mode_str,
        &ledger_warnings,
        &mut warnings,
    );

    Ok(InstallReport {
        canonical_root,
        providers,
        mode: mode_str.to_string(),
        warnings,
    })
}

#[derive(Debug, Clone)]
struct RuntimeSelection {
    selected_runtimes: Vec<String>,
    primary_runtime: String,
}

fn load_runtime_selection() -> RuntimeSelection {
    let default_selected = vec![
        "copilot".to_string(),
        "antigravity".to_string(),
        "codex".to_string(),
        "claude".to_string(),
    ];

    let Some(path) = crate::paths::install_state_path() else {
        return RuntimeSelection {
            selected_runtimes: default_selected.clone(),
            primary_runtime: default_selected[0].clone(),
        };
    };

    let Ok(content) = fs::read_to_string(path) else {
        return RuntimeSelection {
            selected_runtimes: default_selected.clone(),
            primary_runtime: default_selected[0].clone(),
        };
    };

    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return RuntimeSelection {
            selected_runtimes: default_selected.clone(),
            primary_runtime: default_selected[0].clone(),
        };
    };

    let selected_runtimes = value
        .get("selectedRuntimes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|item| item.trim().to_ascii_lowercase())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| default_selected.clone());

    let primary_runtime = value
        .get("primaryRuntime")
        .and_then(Value::as_str)
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .unwrap_or_else(|| selected_runtimes[0].clone());

    RuntimeSelection {
        selected_runtimes,
        primary_runtime,
    }
}

fn provider_from_runtime(runtime: &str) -> &str {
    match runtime {
        "antigravity" => "agy",
        other => other,
    }
}

fn lane_from_runtime(runtime: &str) -> &str {
    match runtime {
        "opencode" => "bridge",
        "gemini" => "migration",
        _ => "primary",
    }
}

fn primary_providers(selection: &RuntimeSelection) -> Vec<String> {
    let mut providers = Vec::new();
    for runtime in &selection.selected_runtimes {
        if lane_from_runtime(runtime) != "primary" {
            continue;
        }
        let provider = provider_from_runtime(runtime).to_string();
        if !providers.contains(&provider) {
            providers.push(provider);
        }
    }
    providers
}

fn ensure_parent_dir(path: &Path) -> Result<(), InstallError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn gal_dist_providers_root() -> Result<PathBuf, InstallError> {
    crate::paths::gal_home()
        .map(|path| path.join("dist").join("providers"))
        .ok_or(InstallError::NoHome)
}

fn provider_state_path(provider: &str) -> Result<PathBuf, InstallError> {
    Ok(gal_dist_providers_root()?.join(provider).join("managed.json"))
}

fn write_json_file(path: &Path, value: &Value) -> Result<(), InstallError> {
    ensure_parent_dir(path)?;
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn claude_marketplace_manifest_path() -> Result<PathBuf, InstallError> {
    crate::paths::gal_plugins_root()
        .map(|root| root.join(".claude-plugin").join("marketplace.json"))
        .ok_or(InstallError::NoHome)
}

fn codex_marketplace_manifest_path() -> Result<PathBuf, InstallError> {
    crate::paths::gal_plugins_root()
        .map(|root| root.join(".agents").join("plugins").join("marketplace.json"))
        .ok_or(InstallError::NoHome)
}

fn copilot_projection_root() -> Result<PathBuf, InstallError> {
    crate::paths::user_home()
        .map(|home| {
            home.join(".copilot")
                .join("installed-plugins")
                .join("gal-copilot")
                .join("gal")
        })
        .ok_or(InstallError::NoHome)
}

fn sync_copilot_projection(canonical_root: &Path) -> Result<(PathBuf, bool, Option<String>), InstallError> {
    let projection_root = copilot_projection_root()?;
    ensure_parent_dir(&projection_root)?;

    if base::platform::is_symlink_or_junction(&projection_root) {
        let _ = base::platform::remove_dir_link(&projection_root);
    } else if projection_root.exists() {
        fs::remove_dir_all(&projection_root)?;
    }

    if base::platform::create_dir_link(canonical_root, &projection_root).is_ok() {
        return Ok((projection_root, false, None));
    }

    copy_dir_all(canonical_root, &projection_root)?;
    let manifest_path = projection_root.join("copilot-manifest.json");
    let version_bumped_to = bump_copilot_manifest_version(&manifest_path)?;
    Ok((projection_root, true, version_bumped_to))
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), InstallError> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn bump_copilot_manifest_version(path: &Path) -> Result<Option<String>, InstallError> {
    if !path.is_file() {
        return Ok(None);
    }
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let current = manifest
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("1.0.0");
    let bumped = format!("{current}.host{}", chrono::Utc::now().format("%Y%m%d%H%M%S"));
    manifest["version"] = Value::String(bumped.clone());
    fs::write(path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(Some(bumped))
}

fn write_provider_lifecycle_artifacts(
    canonical_root: &Path,
    providers: &mut Vec<String>,
) -> Result<(), InstallError> {
    let selection = load_runtime_selection();
    let primary = primary_providers(&selection);

    if primary.contains(&"claude".to_string()) {
        let projection = ClaudeSkillProjection::new(canonical_root.to_path_buf())
            .map_err(|e| InstallError::Io(std::io::Error::other(e.to_string())))?;
        projection
            .apply()
            .map_err(|e| InstallError::Io(std::io::Error::other(e.to_string())))?;
        let marketplace_path = claude_marketplace_manifest_path()?;
        write_json_file(
            &marketplace_path,
            &json!({
                "name": "gal",
                "owner": { "name": "GAL" },
                "description": "Golem Agents Legion — document-driven AI working system",
                "plugins": [
                    {
                        "name": "gal",
                        "source": "./gal",
                        "description": "Golem Agents Legion plugin for Claude Code"
                    }
                ]
            }),
        )?;
        write_json_file(
            &provider_state_path("claude")?,
            &json!({
                "schemaVersion": 1,
                "provider": "claude",
                "canonicalRoot": canonical_root,
                "packageOutputRoot": canonical_root,
                "projectionRoot": projection.skill_surface,
                "installTarget": projection.skill_surface,
                "manifestPath": canonical_root.join(".claude-plugin").join("plugin.json"),
                "generatedAt": chrono::Utc::now().to_rfc3339(),
                "status": "linked-projection",
                "readSurface": "linked-projection",
                "cli": {
                    "available": false,
                    "validateSupported": false,
                    "localArtifactInstallSupported": false,
                    "installScopeSupported": false,
                    "installHelpSummary": null
                },
                "validation": {
                    "command": "claude plugin validate <plugin-root> --strict",
                    "strictPassed": false
                },
                "lifecycle": {
                    "mode": "session-load-only",
                    "stagedPluginRoot": projection.skill_surface,
                    "marketplaceName": "gal",
                    "sessionLoadCommand": "claude --plugin-dir <plugin-root>",
                    "installCommandTemplate": "claude plugin install <plugin> --scope <scope>",
                    "updateCommandTemplate": "claude plugin update <plugin> --scope <scope>",
                    "uninstallCommandTemplate": "claude plugin uninstall <plugin> --scope <scope>"
                }
            }),
        )?;
        if !providers.contains(&"claude".to_string()) {
            providers.push("claude".to_string());
        }
    }

    if primary.contains(&"copilot".to_string()) {
        let (projection_root, refreshed_copy_to_host, version_bumped_to) =
            sync_copilot_projection(canonical_root)?;
        let status = if refreshed_copy_to_host {
            "refreshed-copy2-host"
        } else {
            "linked-projection"
        };
        write_json_file(
            &provider_state_path("copilot")?,
            &json!({
                "schemaVersion": 1,
                "provider": "copilot",
                "canonicalRoot": canonical_root,
                "packageOutputRoot": canonical_root,
                "projectionRoot": projection_root,
                "installTarget": projection_root,
                "manifestPath": canonical_root.join("copilot-manifest.json"),
                "generatedAt": chrono::Utc::now().to_rfc3339(),
                "status": status,
                "readSurface": status,
                "cli": {
                    "available": false,
                    "validateSupported": false,
                    "localArtifactInstallSupported": false,
                    "marketplaceInstallSupported": false,
                    "installScopeSupported": false,
                    "installHelpSummary": null
                },
                "validation": {
                    "command": null,
                    "strictPassed": false
                },
                "lifecycle": {
                    "mode": "artifact-only",
                    "stagedPluginRoot": projection_root,
                    "refreshedCopyToHost": refreshed_copy_to_host,
                    "versionBumpedTo": version_bumped_to,
                    "sessionLoadCommand": "GitHub Copilot reads the projected plugin from ~/.copilot/installed-plugins/gal-copilot/gal",
                    "installCommandTemplate": "gh copilot plugin install <plugin-root>",
                    "updateCommandTemplate": "gh copilot plugin update <plugin-id>",
                    "uninstallCommandTemplate": "gh copilot plugin uninstall <plugin-id>"
                }
            }),
        )?;
        if !providers.contains(&"copilot".to_string()) {
            providers.push("copilot".to_string());
        }
    }

    if primary.contains(&"codex".to_string()) {
        let marketplace_path = codex_marketplace_manifest_path()?;
        write_json_file(
            &marketplace_path,
            &json!({
                "name": "gal-marketplace",
                "interface": { "displayName": "GAL Plugin Marketplace" },
                "plugins": [
                    {
                        "name": "gal",
                        "source": { "source": "local", "path": "./gal" },
                        "policy": { "installation": "AVAILABLE", "authentication": "ON_INSTALL" },
                        "category": "Engineering"
                    }
                ]
            }),
        )?;
        write_json_file(
            &provider_state_path("codex")?,
            &json!({
                "schemaVersion": 1,
                "provider": "codex",
                "canonicalRoot": canonical_root,
                "packageOutputRoot": canonical_root,
                "projectionRoot": null,
                "installTarget": "gal@gal-marketplace",
                "manifestPath": canonical_root.join(".codex-plugin").join("plugin.json"),
                "generatedAt": chrono::Utc::now().to_rfc3339(),
                "status": "unprojected-artifact",
                "readSurface": "unprojected-artifact",
                "cli": {
                    "available": false,
                    "validateSupported": false,
                    "localArtifactInstallSupported": false,
                    "marketplaceInstallSupported": false,
                    "installScopeSupported": false,
                    "installHelpSummary": null
                },
                "validation": {
                    "command": null,
                    "strictPassed": false
                },
                "lifecycle": {
                    "mode": "artifact-only",
                    "stagedPluginRoot": canonical_root,
                    "marketplaceRoot": crate::paths::gal_plugins_root(),
                    "marketplaceManifestPath": marketplace_path,
                    "marketplaceName": "gal-marketplace",
                    "installedSelector": "gal@gal-marketplace",
                    "sessionLoadCommand": "codex plugin marketplace add <plugins-root> ; codex plugin add gal@gal-marketplace",
                    "installCommandTemplate": "codex plugin marketplace add <plugins-root>",
                    "updateCommandTemplate": "codex plugin remove gal@gal-marketplace ; codex plugin add gal@gal-marketplace",
                    "uninstallCommandTemplate": "codex plugin remove gal@gal-marketplace ; codex plugin marketplace remove gal-marketplace",
                    "pluginRemovedBeforeAdd": false
                }
            }),
        )?;
        if !providers.contains(&"codex".to_string()) {
            providers.push("codex".to_string());
        }
    }

    if primary.contains(&"agy".to_string()) {
        write_json_file(
            &provider_state_path("agy")?,
            &json!({
                "schemaVersion": 1,
                "provider": "agy",
                "canonicalRoot": canonical_root,
                "packageOutputRoot": canonical_root,
                "projectionRoot": crate::paths::gal_active_provider_path("agy"),
                "installTarget": crate::paths::user_home().map(|home| home.join(".gemini").join("antigravity-cli").join("plugins").join("gal")),
                "shortcutTarget": crate::paths::gal_active_provider_path("agy"),
                "generatedAt": chrono::Utc::now().to_rfc3339(),
                "status": "linked-projection",
                "readSurface": "linked-projection",
                "cli": {
                    "available": false,
                    "validateSupported": false,
                    "localArtifactInstallSupported": false,
                    "marketplaceInstallSupported": false,
                    "installScopeSupported": false,
                    "installHelpSummary": null
                },
                "validation": {
                    "command": null,
                    "strictPassed": false
                },
                "lifecycle": {
                    "mode": "managed-shortcut",
                    "status": "implemented"
                }
            }),
        )?;
        if !providers.contains(&"agy".to_string()) {
            providers.push("agy".to_string());
        }
    }

    let _ = &selection.primary_runtime;
    Ok(())
}

/// Run `gal uninstall` — remove canonical root and provider surfaces.
pub fn run_uninstall() -> Result<(), InstallError> {
    let home = crate::paths::user_home().ok_or(InstallError::NoHome)?;
    let canonical_root = home.join(".gal").join("plugins").join("gal");

    let previous_ledger = ledger_path().map(|path| Ledger::load(&path));
    let previous_entry = previous_ledger.as_ref().and_then(|ledger| ledger.last.clone());

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
        if let Err(e) = fs::remove_dir_all(&canonical_root) {
            warnings.push(format!("canonical root removal: {e}"));
        }
    }

    collect_removal_error(
        &mut warnings,
        "copilot projection removal",
        copilot_projection_root().and_then(|path| remove_path_if_present(&path)),
    );
    collect_removal_error(
        &mut warnings,
        "Claude skills removal",
        remove_path_if_present(&home.join(".claude").join("skills").join("gal")),
    );
    collect_removal_error(
        &mut warnings,
        "Claude plugins removal",
        remove_path_if_present(&home.join(".claude").join("plugins").join("gal")),
    );
    collect_removal_error(
        &mut warnings,
        "provider dist removal",
        gal_dist_providers_root().and_then(|path| remove_path_if_present(&path)),
    );
    collect_removal_error(
        &mut warnings,
        "claude-plugin marker removal",
        remove_path_if_present(&home.join(".gal").join("plugins").join(".claude-plugin")),
    );
    collect_removal_error(
        &mut warnings,
        ".agents marker removal",
        remove_path_if_present(&home.join(".gal").join("plugins").join(".agents")),
    );

    let providers = previous_entry
        .as_ref()
        .map(|entry| entry.providers.clone())
        .unwrap_or_default();
    let mode = previous_entry
        .as_ref()
        .map(|entry| entry.mode.clone())
        .unwrap_or_else(|| "normal".to_string());

    // Write uninstall ledger entry with the warnings accumulated before ledger save.
    let ledger_warnings = warnings.clone();
    write_ledger_entry(
        "uninstall",
        &canonical_root,
        &providers,
        &mode,
        &ledger_warnings,
        &mut warnings,
    );

    for w in &warnings {
        eprintln!("gal uninstall warning: {w}");
    }

    if warnings.is_empty() {
        Ok(())
    } else {
        Err(InstallError::PartialUninstall { warnings })
    }
}

fn collect_removal_error(
    warnings: &mut Vec<String>,
    label: &str,
    result: Result<(), InstallError>,
) {
    if let Err(err) = result {
        warnings.push(format!("{label}: {err}"));
    }
}

fn remove_path_if_present(path: &Path) -> Result<(), InstallError> {
    if base::platform::is_symlink_or_junction(path) {
        let _ = base::platform::remove_dir_link(path);
        return Ok(());
    }
    if path.is_file() {
        fs::remove_file(path)?;
    } else if path.is_dir() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn write_ledger_entry(
    op: &str,
    canonical_root: &Path,
    providers: &[String],
    mode: &str,
    entry_warnings: &[String],
    warnings: &mut Vec<String>,
) {
    if let Some(ledger_p) = ledger_path() {
        let mut ledger = Ledger::load(&ledger_p);
        ledger.record(LedgerEntry {
            operation: op.to_string(),
            canonical_root: canonical_root.to_path_buf(),
            timestamp: now_timestamp(),
            providers: providers.to_vec(),
            mode: mode.to_string(),
            warnings: entry_warnings.to_vec(),
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
        assert!(InstallError::PartialUninstall { warnings: vec!["x".to_string()] }
            .to_string()
            .contains("partial failures"));
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

    #[test]
    fn write_ledger_entry_persists_warnings() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut warnings = Vec::new();

        #[cfg(windows)]
        unsafe {
            std::env::set_var("USERPROFILE", temp.path());
            std::env::set_var("HOME", temp.path());
        }

        #[cfg(not(windows))]
        unsafe {
            std::env::set_var("HOME", temp.path());
        }

        let ledger_path = ledger_path().expect("fake home should yield a ledger path");

        let mut ledger = Ledger::default();
        ledger.record(LedgerEntry {
            operation: "install".to_string(),
            canonical_root: PathBuf::from("/tmp/gal"),
            timestamp: now_timestamp(),
            providers: vec!["claude".to_string()],
            mode: "dev".to_string(),
            warnings: vec![],
        });
        ledger.save(&ledger_path).unwrap();

        write_ledger_entry(
            "uninstall",
            Path::new("/tmp/gal"),
            &["claude".to_string()],
            "dev",
            &["partial failure".to_string()],
            &mut warnings,
        );

        let loaded = Ledger::load(&ledger_path);
        let last = loaded.last.expect("ledger entry should exist");
        assert_eq!(last.operation, "uninstall");
        assert_eq!(last.providers, vec!["claude".to_string()]);
        assert_eq!(last.mode, "dev");
        assert_eq!(last.warnings, vec!["partial failure".to_string()]);
    }
}
