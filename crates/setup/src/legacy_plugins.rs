//! Install-orchestration seam used by `gal setup`.
//!
//! R-04 kept this as the only legacy-script spawn site. R-05 repoints the seam
//! to the Rust install path so `gal setup` and `gal setup --check` no longer
//! depend on `Install-GalPlugins.*`.

use crate::{SetupError, SetupOptions};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn copilot_projection_root() -> Result<PathBuf, SetupError> {
    base::paths::user_home()
        .map(|home| {
            home.join(".copilot")
                .join("installed-plugins")
                .join("gal-copilot")
                .join("gal")
        })
        .ok_or_else(|| SetupError::Message("could not determine home directory".into()))
}

pub fn run_install_check(
    selected_runtimes: &[String],
    out: &mut dyn Write,
) -> Result<i32, SetupError> {
    let canonical_root = base::paths::gal_plugin_root("gal")
        .ok_or_else(|| SetupError::Message("could not determine GAL plugin root".into()))?;
    let copilot_selected = selected_runtimes.iter().any(|runtime| runtime == "copilot");
    let copilot_projection = copilot_projection_root()?;
    let canonical_manifest_path = canonical_root.join("copilot-manifest.json");
    let host_manifest_path = copilot_projection.join("copilot-manifest.json");

    writeln!(out).ok();
    writeln!(out, "=== GAL provider doctor ===").ok();
    writeln!(out, "CANONICAL:").ok();
    if canonical_root.exists() {
        writeln!(out, "  [OK] {} — canonical GAL plugin root present", canonical_root.display()).ok();
    } else {
        writeln!(out, "  [WARN] {} — canonical GAL plugin root missing", canonical_root.display()).ok();
    }

    writeln!(out, "EXPECTED PROJECTION:").ok();
    if copilot_selected {
        writeln!(out, "  [INFO] {} — expected Copilot plugin projection root", copilot_projection.display()).ok();
    } else {
        writeln!(out, "  [INFO] No Copilot provider selected for this doctor invocation.").ok();
    }

    writeln!(out, "HOST-MANAGED:").ok();
    if copilot_selected
        && copilot_projection.exists()
        && !base::platform::is_symlink_or_junction(&copilot_projection)
    {
        writeln!(out, "  [WARN] {} — Copilot host-managed copy detected", copilot_projection.display()).ok();
    } else {
        writeln!(out, "  [INFO] No Copilot host-managed copies detected.").ok();
    }

    writeln!(out, "STALE:").ok();
    if copilot_selected
        && copilot_projection.exists()
        && !base::platform::is_symlink_or_junction(&copilot_projection)
        && canonical_manifest_path.is_file()
        && host_manifest_path.is_file()
    {
        let canonical_version = manifest_version(&canonical_manifest_path)?;
        let host_version = manifest_version(&host_manifest_path)?;
        if canonical_version != host_version {
            writeln!(out, "  [WARN] {} — host copy diverges from canonical (canonical={}; host={})", copilot_projection.display(), canonical_version, host_version).ok();
        } else {
            writeln!(out, "  [OK] {} — host copy version matches canonical", copilot_projection.display()).ok();
        }
    } else {
        writeln!(out, "  [INFO] No stale host-copy findings detected.").ok();
    }

    Ok(0)
}

fn manifest_version(path: &std::path::Path) -> Result<String, SetupError> {
    let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)
        .map_err(|e| SetupError::Message(format!("invalid manifest {}: {e}", path.display())))?;
    Ok(value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string())
}

pub fn run_install_orchestration(opts: &SetupOptions) -> Result<i32, SetupError> {
    if opts.dry_run {
        return Ok(0);
    }

    let config = base::config::GalConfig::load();
    if opts.uninstall {
        gal_engine::install::run_uninstall()
            .map_err(|e| SetupError::Message(format!("gal uninstall failed: {e}")))?;
        return Ok(0);
    }

    gal_engine::install::run_install(&config)
        .map_err(|e| SetupError::Message(format!("gal install failed: {e}")))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_check_reports_without_selected_copilot() {
        let mut output = Vec::new();
        let code = run_install_check(&["claude".into()], &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert_eq!(code, 0);
        assert!(text.contains("No Copilot provider selected"));
    }
}
