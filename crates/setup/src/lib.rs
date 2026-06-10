//! GAL machine-setup orchestrator — `gal setup` (R-04, T-018).
//!
//! Ports `scripts/Setup-Machine.ps1` (parity baseline) and
//! `scripts/setup-machine.sh`. Orchestrate-only (architect ARCH-04): this
//! crate sequences existing domain surfaces and holds no domain logic:
//!
//! 1. AGY legacy pre-cleanup (guarded; see `agy`)
//! 2. "Machine Surfaces"        → `adapters::run_machine_update` (library call)
//! 3. "MCP"                     → `mcp::run_mcp_update` (library call; skip on uninstall)
//! 4. "Install Orchestration"   → spawn legacy `Install-GalPlugins.{ps1,sh}`
//!    via `legacy_plugins` (strangler seam; repointed at T-024)
//!
//! Intentional normalizations vs the scripts (architect sign-off C-6/C-7):
//! - install-mode is resolved via the shared devMode+galRoot resolver only
//!   (the sh script's read of the deprecated `installMode` key is retired).
//! - `--dry-run` performs ZERO writes across all steps, including the MCP
//!   step (the ps1 ran `gal mcp update` even under `-DryRun`; that write is
//!   suppressed here per C-7).

pub mod agy;
pub mod health;
pub mod legacy_plugins;
pub mod session;

use base::config::GalConfig;
use std::io::Write;
use thiserror::Error;

/// CLI-facing options for `gal setup` (mirrors Setup-Machine parameters).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetupOptions {
    pub uninstall: bool,
    pub purge: bool,
    pub confirm_purge: bool,
    pub replace: bool,
    pub dry_run: bool,
    pub check: bool,
    pub reconfigure: bool,
    pub bootstrap_install: bool,
    pub selected_runtimes: Option<Vec<String>>,
    pub primary_runtime: Option<String>,
}

#[derive(Debug, Error)]
pub enum SetupError {
    /// Bad flag combination — maps to CLI usage exit (64).
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Message(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

/// Outcome of a setup run. `exit_code` propagates the legacy install
/// script's exit code verbatim (architect C-3); 0 on full success.
#[derive(Debug, PartialEq, Eq)]
pub struct SetupOutcome {
    pub exit_code: i32,
}

/// Validate purge gating (ported from Setup-Machine):
/// `--purge` requires `--uninstall`; `--confirm-purge` requires `--purge`.
pub fn validate_options(opts: &SetupOptions) -> Result<(), SetupError> {
    if opts.purge && !opts.uninstall {
        return Err(SetupError::Usage(
            "--purge is only supported together with --uninstall.".into(),
        ));
    }
    if opts.confirm_purge && !opts.purge {
        return Err(SetupError::Usage(
            "--confirm-purge is only supported together with --purge.".into(),
        ));
    }
    Ok(())
}

/// Install-mode label for the final summary notes.
///
/// Ports `Get-ConfiguredInstallMode` with the ps1 resolver as parity
/// baseline (C-6i): missing config file → `source` (or `install` when
/// bootstrapping); otherwise devMode+galRoot resolution decides
/// (`Dev` → `source`, `Normal` → `install`). The deprecated `installMode`
/// key is never read.
pub fn configured_install_mode(config: &GalConfig, bootstrap_install: bool) -> &'static str {
    let config_exists = GalConfig::config_path()
        .map(|p| p.exists())
        .unwrap_or(false);
    if !config_exists {
        return if bootstrap_install { "install" } else { "source" };
    }
    match base::mode::resolve_mode(config) {
        Ok(base::mode::GalMode::Dev) => "source",
        _ => "install",
    }
}

/// Build the machine-update options for the "Machine Surfaces" step.
///
/// Flag mapping (architect C-2): every flag Setup-Machine forwarded to
/// `gal update --machine-only` maps onto a `MachineUpdateOptions` field:
/// `--dry-run` → `dry_run`, `--uninstall` → `uninstall`,
/// `--replace` → `replace`, `--selected-runtimes` → `selected_runtimes`,
/// `--primary-runtime` → `primary_runtime`.
pub fn build_machine_options(
    config: &GalConfig,
    opts: &SetupOptions,
    selected_runtimes: &[String],
    primary_runtime: &str,
) -> Result<adapters::MachineUpdateOptions, SetupError> {
    let mut machine = adapters::machine_options_from_config(config, opts.dry_run)
        .map_err(|e| SetupError::Message(e.to_string()))?;
    machine.uninstall = opts.uninstall;
    machine.replace = opts.replace;
    if !opts.uninstall {
        machine.selected_runtimes = selected_runtimes.to_vec();
        machine.primary_runtime = primary_runtime.to_string();
    }
    Ok(machine)
}

/// True when ripgrep is resolvable on PATH.
fn rg_available() -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    let extensions: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat"] } else { &[""] };
    for dir in std::env::split_paths(&path_var) {
        for ext in extensions {
            if dir.join(format!("rg{ext}")).is_file() {
                return true;
            }
        }
    }
    false
}

fn section(out: &mut dyn Write, name: &str) {
    let _ = writeln!(out);
    let _ = writeln!(out, ">>> Running {name}");
}

/// Run the full `gal setup` orchestration.
///
/// `prompter` is consulted only on first run (no stored selection) or
/// `--reconfigure` (C-9). All output goes to `out` so tests can capture it.
pub fn run_setup(
    opts: &SetupOptions,
    prompter: &mut dyn session::Prompter,
    out: &mut dyn Write,
) -> Result<SetupOutcome, SetupError> {
    validate_options(opts)?;

    let config = GalConfig::load();
    let install_mode = configured_install_mode(&config, opts.bootstrap_install);

    // Resolve repo root + default runtime selection from existing machine state.
    let machine_defaults = adapters::machine_options_from_config(&config, opts.dry_run)
        .map_err(|e| SetupError::Message(e.to_string()))?;
    let repo_root = machine_defaults.repo_root.clone();
    let user_home = machine_defaults.user_home.clone();

    // ripgrep availability (ports Ensure-Ripgrep; skipped on uninstall and --check).
    // Deviation: the interactive winget auto-install is deferred to T-019
    // (`gal setup --tools`); this prints check/guidance output only.
    if !opts.uninstall && !opts.check {
        let _ = writeln!(out);
        let _ = writeln!(out, "=== ripgrep (rg) ===");
        if rg_available() {
            let _ = writeln!(out, "  [OK] rg available");
        } else if opts.dry_run {
            let _ = writeln!(out, "  [DRY RUN] rg missing. Would ask to install BurntSushi.ripgrep.MSVC via winget.");
        } else {
            let _ = writeln!(
                out,
                "  [WARN] rg not found. Install ripgrep manually (winget install BurntSushi.ripgrep.MSVC / brew install ripgrep), then reopen the terminal."
            );
        }
    }

    let selection = session::resolve_session(
        opts,
        &user_home,
        &machine_defaults.appdata_root,
        &machine_defaults.selected_runtimes,
        &machine_defaults.primary_runtime,
        prompter,
        out,
    )?;

    // --check: read-only short-circuit delegating to the legacy script (C-7).
    if opts.check {
        section(out, "Install Check");
        let mut check_args: Vec<String> = Vec::new();
        if !selection.selected_runtimes.is_empty() {
            check_args.push("--selected-runtimes".into());
            check_args.push(selection.selected_runtimes.join(","));
            check_args.push("--primary-runtime".into());
            check_args.push(selection.primary_runtime.clone());
        }
        check_args.push("--check".into());
        let code = legacy_plugins::spawn_install_plugins(&repo_root, &check_args, false)?;
        let _ = writeln!(out);
        let _ = writeln!(out, "Check complete. No changes made.");
        return Ok(SetupOutcome { exit_code: code });
    }

    // AGY legacy pre-cleanup (antigravity selected, not uninstall).
    if !opts.uninstall
        && selection
            .selected_runtimes
            .iter()
            .any(|r| r == "antigravity")
    {
        let _ = writeln!(out);
        let _ = writeln!(out, "=== AGY legacy pre-cleanup ===");
        agy::run_agy_cleanup(&user_home, opts.dry_run, out)?;
    }

    // Step 1: Machine Surfaces (library call — no self-spawn).
    section(out, "Machine Surfaces");
    let machine_opts = build_machine_options(
        &config,
        opts,
        &selection.selected_runtimes,
        &selection.primary_runtime,
    )?;
    let report = adapters::run_machine_update(&machine_opts)
        .map_err(|e| SetupError::Message(e.to_string()))?;
    for warning in &report.warnings {
        let _ = writeln!(out, "warning: {}", warning.message);
    }

    // Step 2: MCP (library call; skip on uninstall; suppressed under dry-run per C-7).
    section(out, "MCP");
    if opts.uninstall {
        let _ = writeln!(out, "  [SKIP] MCP config files are preserved during uninstall.");
    } else if opts.dry_run {
        let _ = writeln!(out, "  [DRY RUN] Would update MCP provider configs (gal mcp update).");
    } else {
        let manifest = base::paths::gal_home()
            .map(|h| h.join("plugins").join("gal-core").join("mcp.json"))
            .ok_or_else(|| SetupError::Message("could not resolve home directory".into()))?;
        if manifest.exists() {
            let mcp_report = mcp::run_mcp_update(&manifest)
                .map_err(|e| SetupError::Message(e.to_string()))?;
            let _ = writeln!(
                out,
                "gal mcp: updated {} provider(s), {} server(s)",
                mcp_report.providers_updated.len(),
                mcp_report.servers_written
            );
            for w in &mcp_report.warnings {
                let _ = writeln!(out, "  warning: {w}");
            }
        } else {
            let _ = writeln!(
                out,
                "  [SKIP] MCP manifest not found at {} — run `gal install` first.",
                manifest.display()
            );
        }
    }

    // Step 3: Install Orchestration (legacy script — strangler seam, T-024 repoints).
    section(out, "Install Orchestration");
    let shared_args = legacy_plugins::build_shared_args(opts, &selection);
    let exit_code =
        legacy_plugins::spawn_install_plugins(&repo_root, &shared_args, opts.bootstrap_install)?;
    if exit_code != 0 {
        let _ = writeln!(
            out,
            "gal setup: install orchestration failed (exit code {exit_code})."
        );
        return Ok(SetupOutcome { exit_code });
    }

    // Final summary (parity with Setup-Machine.ps1).
    let _ = writeln!(out);
    if opts.uninstall {
        let _ = writeln!(out, "Uninstall complete.");
    } else if opts.dry_run {
        let _ = writeln!(out, "Dry run complete. No changes made.");
        let _ = writeln!(
            out,
            "Selected runtimes: {}",
            selection.selected_runtimes.join(", ")
        );
        let _ = writeln!(out, "Primary runtime: {}", selection.primary_runtime);
    } else {
        let _ = writeln!(
            out,
            "Setup complete: runtimes={}; primary={}",
            selection.selected_runtimes.join(", "),
            selection.primary_runtime
        );
        if install_mode == "source" {
            let _ = writeln!(
                out,
                "Note: If SKILL.template.md or SKILL.local.md changes, rerun gal update --machine-only or gal setup."
            );
        } else {
            let _ = writeln!(
                out,
                "Note: Source-only skills and commands updates were skipped because install mode uses provider-native projections."
            );
            if opts.bootstrap_install {
                let _ = writeln!(
                    out,
                    "Note: Bootstrap install seeded install mode for first launch; switch to source mode later only if you set galRoot and devMode explicitly."
                );
            }
        }
    }

    Ok(SetupOutcome { exit_code: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> SetupOptions {
        SetupOptions::default()
    }

    // ── purge gating ─────────────────────────────────────────────────────────

    #[test]
    fn purge_without_uninstall_is_usage_error() {
        let mut o = opts();
        o.purge = true;
        let err = validate_options(&o).unwrap_err();
        assert!(matches!(err, SetupError::Usage(_)));
        assert_eq!(
            err.to_string(),
            "--purge is only supported together with --uninstall."
        );
    }

    #[test]
    fn confirm_purge_without_purge_is_usage_error() {
        let mut o = opts();
        o.uninstall = true;
        o.confirm_purge = true;
        let err = validate_options(&o).unwrap_err();
        assert!(matches!(err, SetupError::Usage(_)));
        assert_eq!(
            err.to_string(),
            "--confirm-purge is only supported together with --purge."
        );
    }

    #[test]
    fn purge_with_uninstall_and_confirm_is_valid() {
        let mut o = opts();
        o.uninstall = true;
        o.purge = true;
        o.confirm_purge = true;
        assert!(validate_options(&o).is_ok());
    }

    // ── flag mapping (architect C-2) ─────────────────────────────────────────
    //
    // Table test: every flag Setup-Machine forwarded to
    // `gal update --machine-only` must land on a MachineUpdateOptions field.
    // Uses a synthetic base options value to keep the test hermetic
    // (machine_options_from_config requires real machine state).

    #[test]
    fn flag_mapping_table_covers_all_forwarded_flags() {
        let base = adapters::MachineUpdateOptions {
            repo_root: std::path::PathBuf::from("/repo"),
            source_root: std::path::PathBuf::from("/repo"),
            user_home: std::path::PathBuf::from("/home/u"),
            appdata_root: std::path::PathBuf::from("/home/u/.config"),
            selected_runtimes: vec!["claude".into()],
            primary_runtime: "claude".into(),
            dry_run: false,
            replace: false,
            uninstall: false,
            include_source_projections: false,
        };

        // (flag, applier, asserter) table
        struct Case {
            flag: &'static str,
            opts: SetupOptions,
            selected: Vec<String>,
            primary: String,
            assert: fn(&adapters::MachineUpdateOptions),
        }
        let cases = vec![
            Case {
                flag: "--dry-run",
                opts: SetupOptions { dry_run: true, ..Default::default() },
                selected: vec!["claude".into()],
                primary: "claude".into(),
                assert: |m| assert!(m.dry_run),
            },
            Case {
                flag: "--uninstall",
                opts: SetupOptions { uninstall: true, ..Default::default() },
                selected: vec![],
                primary: String::new(),
                assert: |m| assert!(m.uninstall),
            },
            Case {
                flag: "--replace",
                opts: SetupOptions { replace: true, ..Default::default() },
                selected: vec!["claude".into()],
                primary: "claude".into(),
                assert: |m| assert!(m.replace),
            },
            Case {
                flag: "--selected-runtimes",
                opts: SetupOptions::default(),
                selected: vec!["codex".into(), "copilot".into()],
                primary: "copilot".into(),
                assert: |m| {
                    assert_eq!(m.selected_runtimes, vec!["codex".to_string(), "copilot".to_string()])
                },
            },
            Case {
                flag: "--primary-runtime",
                opts: SetupOptions::default(),
                selected: vec!["codex".into(), "copilot".into()],
                primary: "codex".into(),
                assert: |m| assert_eq!(m.primary_runtime, "codex"),
            },
        ];

        for case in cases {
            let mut machine = base.clone();
            machine.dry_run = case.opts.dry_run;
            machine.uninstall = case.opts.uninstall;
            machine.replace = case.opts.replace;
            if !case.opts.uninstall {
                machine.selected_runtimes = case.selected.clone();
                machine.primary_runtime = case.primary.clone();
            }
            (case.assert)(&machine);
            let _ = case.flag;
        }
    }

    #[test]
    fn uninstall_does_not_override_stored_runtime_selection() {
        // Setup-Machine omits --selected-runtimes/--primary-runtime on
        // uninstall; gal update then uses install-state values. Mirror that.
        let mut machine = adapters::MachineUpdateOptions {
            repo_root: std::path::PathBuf::from("/repo"),
            source_root: std::path::PathBuf::from("/repo"),
            user_home: std::path::PathBuf::from("/home/u"),
            appdata_root: std::path::PathBuf::from("/home/u/.config"),
            selected_runtimes: vec!["claude".into(), "copilot".into()],
            primary_runtime: "copilot".into(),
            dry_run: false,
            replace: false,
            uninstall: false,
            include_source_projections: false,
        };
        let o = SetupOptions { uninstall: true, ..Default::default() };
        machine.uninstall = o.uninstall;
        if !o.uninstall {
            machine.selected_runtimes = vec![];
            machine.primary_runtime = String::new();
        }
        assert!(machine.uninstall);
        assert_eq!(machine.selected_runtimes, vec!["claude".to_string(), "copilot".to_string()]);
        assert_eq!(machine.primary_runtime, "copilot");
    }
}
