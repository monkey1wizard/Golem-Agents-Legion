//! Setup-domain health check (architect C-10).
//!
//! Read-only. Aggregation into `gal doctor` happens at T-031; until then the
//! check is exercised directly by unit tests.

use base::health::{DoctorFinding, HealthCheck};
use std::path::PathBuf;

/// Health over the machine-setup surface.
pub struct SetupHealthCheck {
    /// GAL repo / source root used to locate the legacy install script.
    pub repo_root: PathBuf,
}

impl HealthCheck for SetupHealthCheck {
    fn name(&self) -> &str {
        "setup"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let mut findings = Vec::new();

        // Machine config readable (missing is fine — defaults apply).
        if let Ok(path) = base::config::GalConfig::config_path() {
            if path.exists() && std::fs::read_to_string(&path).is_err() {
                findings.push(DoctorFinding::error(
                    format!("machine config is unreadable: {}", path.display()),
                    "fix permissions or re-create ~/.gal/config/gal.config.json",
                ));
            }
        }

        // ripgrep present (Ensure-Ripgrep parity — warning only).
        if which("rg").is_none() {
            findings.push(DoctorFinding::warning(
                "ripgrep (rg) not found on PATH — GAL search-dependent flows degrade",
            ));
        }

        // Legacy install script present.
        // NOTE: remove this check at T-024 when the install family goes native.
        let script = crate::legacy_plugins::script_path(&self.repo_root);
        if !script.is_file() {
            findings.push(DoctorFinding::error(
                format!("legacy install script missing: {}", script.display()),
                "restore the GAL source tree (scripts/Install-GalPlugins.*) — required until R-05 completes",
            ));
        }

        // Git filter registration state (T-020): only meaningful when the
        // repo root is an actual git checkout carrying the filter scripts.
        if self.repo_root.join(".git").exists()
            && self.repo_root.join("scripts").join("gal-clean.sh").is_file()
            && crate::git_filter::registered_clean_filter(&self.repo_root).is_none()
        {
            findings.push(DoctorFinding::warning(
                "gal-config git filter is not registered — run `gal setup` to register smudge/clean",
            ));
        }

        findings
    }
}

/// Minimal PATH lookup (avoids a new dependency).
fn which(binary: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let extensions: Vec<String> = if cfg!(windows) {
        vec![".exe".into(), ".cmd".into(), ".bat".into(), String::new()]
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path_var) {
        for ext in &extensions {
            let candidate = dir.join(format!("{binary}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::health::Severity;
    use std::fs;

    #[test]
    fn missing_legacy_script_is_an_error() {
        let temp = tempfile::tempdir().unwrap();
        let check = SetupHealthCheck {
            repo_root: temp.path().to_path_buf(),
        };
        let findings = check.check();
        assert!(
            findings
                .iter()
                .any(|f| f.severity == Severity::Error
                    && f.message.contains("legacy install script missing")),
            "expected missing-script error, got: {findings:?}"
        );
    }

    #[test]
    fn present_legacy_script_passes_that_check() {
        let temp = tempfile::tempdir().unwrap();
        let scripts = temp.path().join("scripts");
        fs::create_dir_all(&scripts).unwrap();
        fs::write(scripts.join("Install-GalPlugins.ps1"), "# stub").unwrap();
        fs::write(scripts.join("install-gal-plugins.sh"), "#!/usr/bin/env bash").unwrap();
        let check = SetupHealthCheck {
            repo_root: temp.path().to_path_buf(),
        };
        let findings = check.check();
        assert!(
            !findings
                .iter()
                .any(|f| f.message.contains("legacy install script missing")),
            "script present — must not flag: {findings:?}"
        );
    }

    #[test]
    fn name_is_setup() {
        let check = SetupHealthCheck {
            repo_root: PathBuf::from("."),
        };
        assert_eq!(check.name(), "setup");
    }
}
