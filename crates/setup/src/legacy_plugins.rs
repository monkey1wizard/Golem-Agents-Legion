//! Strangler seam for the legacy install orchestration (architect Ruling 1/2).
//!
//! This module is the ONLY place that spawns `Install-GalPlugins.ps1` /
//! `install-gal-plugins.sh`. The scripts remain live, fully-owned surfaces
//! until R-05 (T-022..T-024) proves install-family parity; T-024 repoints
//! exactly this module to the Rust install path.
//!
//! Contract (architect C-3):
//! - hard error with a clear message when the script is missing,
//! - the child's exit code is propagated verbatim,
//! - `GAL_BOOTSTRAP_INSTALL=1` is passed via the CHILD environment only —
//!   the parent process environment is never mutated.

use crate::session::SessionSelection;
use crate::{SetupError, SetupOptions};
use std::path::{Path, PathBuf};

/// A fully-constructed (but not yet spawned) legacy-script invocation.
/// Pure data so tests can assert command construction without spawning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
    /// Extra environment for the child only.
    pub env: Vec<(String, String)>,
}

/// Path of the platform's legacy install script under `repo_root`.
pub fn script_path(repo_root: &Path) -> PathBuf {
    if cfg!(windows) {
        repo_root.join("scripts").join("Install-GalPlugins.ps1")
    } else {
        repo_root.join("scripts").join("install-gal-plugins.sh")
    }
}

/// Shared argument list Setup-Machine forwards to Install-GalPlugins
/// (CLI-flag form; translated to PowerShell switches on Windows by
/// `build_invocation`).
pub fn build_shared_args(opts: &SetupOptions, selection: &SessionSelection) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if opts.uninstall {
        args.push("--uninstall".into());
    }
    if opts.replace {
        args.push("--replace".into());
    }
    if opts.dry_run {
        args.push("--dry-run".into());
    }
    if opts.reconfigure {
        args.push("--reconfigure".into());
    }
    if !opts.uninstall {
        args.push("--selected-runtimes".into());
        args.push(selection.selected_runtimes.join(","));
        args.push("--primary-runtime".into());
        args.push(selection.primary_runtime.clone());
    }
    if opts.bootstrap_install {
        args.push("--bootstrap-install".into());
    }
    if opts.purge {
        args.push("--purge".into());
    }
    if opts.confirm_purge {
        args.push("--confirm-purge".into());
    }
    args
}

/// Translate a CLI-style flag list into the invocation for this platform.
///
/// Windows uses `powershell -NoProfile -ExecutionPolicy Bypass -Command`
/// (not `-File`) so that `[string[]]` parameters such as `-SelectedRuntimes`
/// receive a real array from the comma-separated value. Runtime keys are
/// validated lowercase identifiers, so embedding them in the command text is
/// safe; the script path is single-quoted with PowerShell quote doubling.
pub fn build_invocation(
    repo_root: &Path,
    args: &[String],
    bootstrap_install: bool,
) -> Result<Invocation, SetupError> {
    let script = script_path(repo_root);
    if !script.is_file() {
        return Err(SetupError::Message(format!(
            "legacy install script not found: {} — the GAL source tree is incomplete (expected until R-05 deletes it).",
            script.display()
        )));
    }

    let env = if bootstrap_install {
        vec![("GAL_BOOTSTRAP_INSTALL".to_string(), "1".to_string())]
    } else {
        Vec::new()
    };

    if cfg!(windows) {
        let mut command = format!("& '{}'", script.display().to_string().replace('\'', "''"));
        let mut i = 0usize;
        while i < args.len() {
            let arg = &args[i];
            match arg.as_str() {
                "--uninstall" => command.push_str(" -Uninstall"),
                "--replace" => command.push_str(" -Replace"),
                "--dry-run" => command.push_str(" -DryRun"),
                "--reconfigure" => command.push_str(" -Reconfigure"),
                "--bootstrap-install" => command.push_str(" -BootstrapInstall"),
                "--purge" => command.push_str(" -Purge"),
                "--confirm-purge" => command.push_str(" -ConfirmPurge"),
                "--check" => command.push_str(" -Check"),
                "--selected-runtimes" => {
                    i += 1;
                    let value = args.get(i).cloned().unwrap_or_default();
                    command.push_str(&format!(" -SelectedRuntimes {value}"));
                }
                "--primary-runtime" => {
                    i += 1;
                    let value = args.get(i).cloned().unwrap_or_default();
                    command.push_str(&format!(" -PrimaryRuntime {value}"));
                }
                other => {
                    return Err(SetupError::Message(format!(
                        "internal error: unmapped legacy-script flag '{other}'"
                    )));
                }
            }
            i += 1;
        }
        command.push_str("; exit $LASTEXITCODE");
        Ok(Invocation {
            program: "powershell".into(),
            args: vec![
                "-NoProfile".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-Command".into(),
                command,
            ],
            env,
        })
    } else {
        let mut bash_args = vec![script.display().to_string()];
        bash_args.extend(args.iter().cloned());
        Ok(Invocation {
            program: "bash".into(),
            args: bash_args,
            env,
        })
    }
}

/// Spawn the legacy install script and return its exit code verbatim (C-3).
pub fn spawn_install_plugins(
    repo_root: &Path,
    args: &[String],
    bootstrap_install: bool,
) -> Result<i32, SetupError> {
    let invocation = build_invocation(repo_root, args, bootstrap_install)?;
    let mut command = std::process::Command::new(&invocation.program);
    command.args(&invocation.args);
    for (key, value) in &invocation.env {
        command.env(key, value); // child env only — parent is never mutated
    }
    let status = command.status().map_err(|e| {
        SetupError::Message(format!(
            "failed to spawn {}: {e}",
            invocation.program
        ))
    })?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn selection() -> SessionSelection {
        SessionSelection {
            selected_runtimes: vec!["claude".into(), "copilot".into()],
            primary_runtime: "copilot".into(),
        }
    }

    fn repo_with_script() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        let scripts = temp.path().join("scripts");
        fs::create_dir_all(&scripts).unwrap();
        fs::write(scripts.join("Install-GalPlugins.ps1"), "# stub").unwrap();
        fs::write(scripts.join("install-gal-plugins.sh"), "#!/usr/bin/env bash").unwrap();
        temp
    }

    #[test]
    fn missing_script_is_hard_error() {
        let temp = tempfile::tempdir().unwrap();
        let err = build_invocation(temp.path(), &[], false).unwrap_err();
        assert!(err.to_string().contains("legacy install script not found"));
    }

    #[test]
    fn shared_args_match_setup_machine_forwarding() {
        let opts = SetupOptions {
            replace: true,
            dry_run: true,
            ..Default::default()
        };
        let args = build_shared_args(&opts, &selection());
        assert_eq!(
            args,
            vec![
                "--replace",
                "--dry-run",
                "--selected-runtimes",
                "claude,copilot",
                "--primary-runtime",
                "copilot",
            ]
        );
    }

    #[test]
    fn uninstall_omits_runtime_args_and_passes_purge_chain() {
        let opts = SetupOptions {
            uninstall: true,
            purge: true,
            confirm_purge: true,
            ..Default::default()
        };
        let args = build_shared_args(&opts, &selection());
        assert_eq!(args, vec!["--uninstall", "--purge", "--confirm-purge"]);
    }

    #[test]
    fn bootstrap_env_is_child_only() {
        let temp = repo_with_script();
        let before = std::env::var("GAL_BOOTSTRAP_INSTALL").ok();
        let invocation = build_invocation(temp.path(), &[], true).unwrap();
        assert_eq!(
            invocation.env,
            vec![("GAL_BOOTSTRAP_INSTALL".to_string(), "1".to_string())]
        );
        // Parent process env must be untouched.
        assert_eq!(std::env::var("GAL_BOOTSTRAP_INSTALL").ok(), before);
    }

    #[cfg(windows)]
    #[test]
    fn windows_invocation_uses_command_mode_with_ps_switches() {
        let temp = repo_with_script();
        let args = vec![
            "--dry-run".to_string(),
            "--selected-runtimes".to_string(),
            "claude,copilot".to_string(),
            "--primary-runtime".to_string(),
            "copilot".to_string(),
        ];
        let invocation = build_invocation(temp.path(), &args, false).unwrap();
        assert_eq!(invocation.program, "powershell");
        let command = invocation.args.last().unwrap();
        assert!(command.contains("-DryRun"));
        assert!(command.contains("-SelectedRuntimes claude,copilot"));
        assert!(command.contains("-PrimaryRuntime copilot"));
        assert!(command.ends_with("; exit $LASTEXITCODE"));
        assert!(
            invocation.args.contains(&"-Command".to_string()),
            "must use -Command so [string[]] binds the CSV as an array"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_invocation_uses_bash_with_cli_flags() {
        let temp = repo_with_script();
        let args = vec!["--dry-run".to_string()];
        let invocation = build_invocation(temp.path(), &args, false).unwrap();
        assert_eq!(invocation.program, "bash");
        assert!(invocation.args[0].ends_with("install-gal-plugins.sh"));
        assert_eq!(invocation.args[1], "--dry-run");
    }
}
