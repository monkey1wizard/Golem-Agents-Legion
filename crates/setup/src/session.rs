//! Setup session resolution — ports the runtime-selection half of
//! `Initialize-SetupSession` / `Get-InstallSelectionState` from
//! `scripts/common/Common.{ps1,sh}` behind a `Prompter` seam (architect C-9).
//!
//! Resolution precedence:
//! 1. `--uninstall` → empty selection (the uninstall path never re-prompts).
//! 2. Explicit `--selected-runtimes` / `--primary-runtime` flags.
//! 3. Stored machine state (`~/.gal/install-state.json`), already loaded by
//!    `adapters::machine_options_from_config` — unless `--reconfigure`.
//! 4. First run (no stored selection) or `--reconfigure` → `Prompter`.
//!
//! First-run default selection ports `Get-DetectedRuntimeSelection` with
//! simplified probes (GAL marker existence instead of strict repo-link
//! verification — recorded as a deviation). Prompted selections are
//! persisted to `install-state.json` like the legacy
//! `Get-InstallSelectionState` (dry-run prints the would-save line instead).
//! The legacy gemini→antigravity selection migration is not ported.

use crate::{SetupError, SetupOptions};
use base::runtime::{default_primary_runtime, VALID_RUNTIMES};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Resolved runtime selection for this setup run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSelection {
    pub selected_runtimes: Vec<String>,
    pub primary_runtime: String,
}

/// Interactive seam: implementations answer the first-run / `--reconfigure`
/// runtime-selection questions. Tests inject a scripted prompter; the CLI
/// uses `StdinPrompter` when interactive and `NonInteractivePrompter`
/// otherwise (C-9).
pub trait Prompter {
    /// Choose the runtime set. `default_selection` is offered as the
    /// accept-default answer.
    fn select_runtimes(&mut self, default_selection: &[String]) -> Vec<String>;
    /// Choose the primary runtime among `selected`. `default_primary` is the
    /// accept-default answer.
    fn select_primary(&mut self, selected: &[String], default_primary: &str) -> String;
}

/// Accepts all defaults without asking (non-interactive mode, C-9).
pub struct NonInteractivePrompter;

impl Prompter for NonInteractivePrompter {
    fn select_runtimes(&mut self, default_selection: &[String]) -> Vec<String> {
        default_selection.to_vec()
    }
    fn select_primary(&mut self, _selected: &[String], default_primary: &str) -> String {
        default_primary.to_string()
    }
}

/// Reads answers from stdin (used by the CLI when a TTY is present).
pub struct StdinPrompter;

impl Prompter for StdinPrompter {
    fn select_runtimes(&mut self, default_selection: &[String]) -> Vec<String> {
        eprintln!(
            "  [PROMPT] Runtimes to install (comma-separated; valid: {}) [{}]:",
            VALID_RUNTIMES.join(", "),
            default_selection.join(",")
        );
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() || line.trim().is_empty() {
            return default_selection.to_vec();
        }
        normalize_runtimes(
            &line
                .split(',')
                .map(|s| s.trim().to_string())
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| default_selection.to_vec())
    }

    fn select_primary(&mut self, selected: &[String], default_primary: &str) -> String {
        eprintln!(
            "  [PROMPT] Primary runtime ({}) [{}]:",
            selected.join(", "),
            default_primary
        );
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() {
            return default_primary.to_string();
        }
        let answer = line.trim().to_ascii_lowercase();
        if answer.is_empty() || !selected.iter().any(|r| r == &answer) {
            default_primary.to_string()
        } else {
            answer
        }
    }
}

/// Validate and normalize a runtime list against `VALID_RUNTIMES`.
pub fn normalize_runtimes(raw: &[String]) -> Result<Vec<String>, SetupError> {
    let mut normalized: Vec<String> = Vec::new();
    for value in raw {
        let key = value.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        if !VALID_RUNTIMES.contains(&key.as_str()) {
            return Err(SetupError::Usage(format!(
                "unknown runtime '{key}'. Valid values: {}",
                VALID_RUNTIMES.join(", ")
            )));
        }
        if !normalized.contains(&key) {
            normalized.push(key);
        }
    }
    if normalized.is_empty() {
        return Err(SetupError::Usage(
            "at least one runtime must be selected".into(),
        ));
    }
    normalized.sort();
    Ok(normalized)
}

fn resolve_primary(
    explicit: Option<&str>,
    selected: &[String],
) -> Result<String, SetupError> {
    if let Some(primary) = explicit {
        let normalized = primary.trim().to_ascii_lowercase();
        if !selected.iter().any(|r| r == &normalized) {
            return Err(SetupError::Usage(format!(
                "primary runtime '{normalized}' is not in the selected runtime set ({})",
                selected.join(", ")
            )));
        }
        return Ok(normalized);
    }
    let refs: Vec<&str> = selected.iter().map(String::as_str).collect();
    default_primary_runtime(&refs)
        .map(str::to_string)
        .ok_or_else(|| SetupError::Message("could not resolve primary runtime".into()))
}

/// Probe which runtimes already carry GAL-managed machine surfaces.
///
/// Simplified port of `Get-DetectedRuntimeSelection`: checks for the
/// GAL-created marker paths per runtime (existence, not strict repo-link
/// resolution). Used only as the first-run prompt default.
pub fn detect_runtimes(user_home: &Path, appdata_root: &Path) -> Vec<String> {
    let mut detected: Vec<String> = Vec::new();
    let exists = |p: PathBuf| p.symlink_metadata().is_ok();

    if exists(user_home.join(".copilot").join("gal")) {
        detected.push("copilot".into());
    }
    if exists(user_home.join(".gemini").join("antigravity-cli").join("gal"))
        || exists(
            user_home
                .join(".gemini")
                .join("antigravity-cli")
                .join("plugins")
                .join("gal"),
        )
    {
        detected.push("antigravity".into());
    }
    if exists(user_home.join(".gemini").join("gal"))
        || exists(user_home.join(".gemini").join("gal-context.md"))
    {
        detected.push("gemini".into());
    }
    if exists(user_home.join(".codex").join("skills").join("gal")) {
        detected.push("codex".into());
    }
    if exists(appdata_root.join("opencode").join("skills").join("gal"))
        || exists(user_home.join(".config").join("opencode").join("skills").join("gal"))
    {
        detected.push("opencode".into());
    }
    if exists(user_home.join(".claude").join("skills").join("gal"))
        || exists(user_home.join(".gal").join("plugins").join("gal"))
    {
        detected.push("claude".into());
    }
    detected
}

/// Persist a prompted selection to `install-state.json`
/// (ports the legacy `Would save install state` / save behavior).
fn persist_selection(
    install_state: &Path,
    selected: &[String],
    primary: &str,
    dry_run: bool,
    out: &mut dyn Write,
) -> Result<(), SetupError> {
    if dry_run {
        let _ = writeln!(
            out,
            "  [DRY RUN] Would save install state: {}",
            install_state.display()
        );
        return Ok(());
    }
    let existing = base::json_util::read_json_map(install_state).unwrap_or_default();
    let now = chrono::Utc::now().to_rfc3339();
    let installed_at = existing
        .get("installedAt")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| now.clone());
    let mut state = serde_json::Map::new();
    state.insert("schemaVersion".into(), json!(1));
    state.insert("selectedRuntimes".into(), json!(selected));
    state.insert("primaryRuntime".into(), json!(primary));
    state.insert("installedAt".into(), json!(installed_at));
    state.insert("lastConfiguredAt".into(), json!(now));
    base::json_util::write_json_map(install_state, &state)?;
    let _ = writeln!(out, "  [OK] Saved install state: {}", install_state.display());
    Ok(())
}

/// Resolve the runtime selection for this run.
///
/// `stored_runtimes` / `stored_primary` come from
/// `adapters::machine_options_from_config` (i.e. `install-state.json`, with
/// the documented all-runtimes default when no state exists).
pub fn resolve_session(
    opts: &SetupOptions,
    user_home: &Path,
    appdata_root: &Path,
    stored_runtimes: &[String],
    stored_primary: &str,
    prompter: &mut dyn Prompter,
    out: &mut dyn Write,
) -> Result<SessionSelection, SetupError> {
    if opts.uninstall {
        // Ports Initialize-SetupSession: uninstall clears the selection.
        return Ok(SessionSelection {
            selected_runtimes: Vec::new(),
            primary_runtime: String::new(),
        });
    }

    // Explicit flags win.
    if let Some(raw) = &opts.selected_runtimes {
        let selected = normalize_runtimes(raw)?;
        let primary = resolve_primary(opts.primary_runtime.as_deref(), &selected)?;
        return Ok(SessionSelection {
            selected_runtimes: selected,
            primary_runtime: primary,
        });
    }

    let install_state = user_home.join(".gal").join("install-state.json");
    let has_stored_state = install_state.is_file();

    if has_stored_state && !opts.reconfigure {
        let _ = writeln!(out);
        let _ = writeln!(out, "=== GAL runtime selection ===");
        let _ = writeln!(
            out,
            "  [OK] Using saved install state from {}",
            install_state.display()
        );
        let _ = writeln!(
            out,
            "  [OK] Selected runtimes: {}",
            stored_runtimes.join(", ")
        );
        let _ = writeln!(out, "  [OK] Primary runtime: {stored_primary}");
        return Ok(SessionSelection {
            selected_runtimes: stored_runtimes.to_vec(),
            primary_runtime: stored_primary.to_string(),
        });
    }

    // First run or --reconfigure: ask the prompter.
    let _ = writeln!(out);
    let _ = writeln!(out, "=== GAL runtime selection ===");
    let default_selection: Vec<String> = if has_stored_state {
        stored_runtimes.to_vec()
    } else {
        // Ports Get-InstallSelectionState: detected runtimes when a legacy
        // GAL machine install exists, otherwise the full catalog.
        let detected = detect_runtimes(user_home, appdata_root);
        if detected.is_empty() {
            let _ = writeln!(
                out,
                "  [INFO] First-time GAL install detected. Choose where GAL should install runtime integrations."
            );
            VALID_RUNTIMES.iter().map(|s| (*s).to_string()).collect()
        } else {
            let _ = writeln!(
                out,
                "  [INFO] Detected a legacy GAL machine install without install-state. Confirm or change the runtime set before continuing."
            );
            detected
        }
    };
    let selected = normalize_runtimes(&prompter.select_runtimes(&default_selection))?;
    let default_primary = if selected.iter().any(|r| r == stored_primary) {
        stored_primary.to_string()
    } else {
        let refs: Vec<&str> = selected.iter().map(String::as_str).collect();
        default_primary_runtime(&refs)
            .map(str::to_string)
            .ok_or_else(|| SetupError::Message("could not resolve primary runtime".into()))?
    };
    let primary_answer = prompter.select_primary(&selected, &default_primary);
    let primary = resolve_primary(Some(&primary_answer), &selected)?;
    let _ = writeln!(out, "  [OK] Selected runtimes: {}", selected.join(", "));
    let _ = writeln!(out, "  [OK] Primary runtime: {primary}");
    if !opts.check {
        // --check is read-only end to end (C-7): never persist from it.
        persist_selection(&install_state, &selected, &primary, opts.dry_run, out)?;
    }
    Ok(SessionSelection {
        selected_runtimes: selected,
        primary_runtime: primary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct ScriptedPrompter {
        runtimes: Vec<String>,
        primary: String,
        runtime_calls: usize,
        primary_calls: usize,
    }

    impl Prompter for ScriptedPrompter {
        fn select_runtimes(&mut self, _default: &[String]) -> Vec<String> {
            self.runtime_calls += 1;
            self.runtimes.clone()
        }
        fn select_primary(&mut self, _selected: &[String], _default: &str) -> String {
            self.primary_calls += 1;
            self.primary.clone()
        }
    }

    fn opts() -> SetupOptions {
        SetupOptions::default()
    }

    #[test]
    fn uninstall_clears_selection_without_prompting() {
        let temp = tempfile::tempdir().unwrap();
        let mut prompter = ScriptedPrompter {
            runtimes: vec!["claude".into()],
            primary: "claude".into(),
            runtime_calls: 0,
            primary_calls: 0,
        };
        let mut out = Vec::new();
        let o = SetupOptions { uninstall: true, ..opts() };
        let s = resolve_session(
            &o,
            temp.path(),
            temp.path(),
            &["claude".into()],
            "claude",
            &mut prompter,
            &mut out,
        )
        .unwrap();
        assert!(s.selected_runtimes.is_empty());
        assert!(s.primary_runtime.is_empty());
        assert_eq!(prompter.runtime_calls, 0);
    }

    #[test]
    fn explicit_flags_win_and_are_validated() {
        let temp = tempfile::tempdir().unwrap();
        let mut prompter = NonInteractivePrompter;
        let mut out = Vec::new();
        let o = SetupOptions {
            selected_runtimes: Some(vec!["Copilot".into(), "codex".into()]),
            primary_runtime: Some("codex".into()),
            ..opts()
        };
        let s = resolve_session(&o, temp.path(), temp.path(), &[], "", &mut prompter, &mut out).unwrap();
        assert_eq!(s.selected_runtimes, vec!["codex".to_string(), "copilot".to_string()]);
        assert_eq!(s.primary_runtime, "codex");
    }

    #[test]
    fn explicit_unknown_runtime_is_usage_error() {
        let temp = tempfile::tempdir().unwrap();
        let mut prompter = NonInteractivePrompter;
        let mut out = Vec::new();
        let o = SetupOptions {
            selected_runtimes: Some(vec!["vscode".into()]),
            ..opts()
        };
        let err =
            resolve_session(&o, temp.path(), temp.path(), &[], "", &mut prompter, &mut out).unwrap_err();
        assert!(matches!(err, SetupError::Usage(_)));
    }

    #[test]
    fn explicit_primary_outside_selection_is_usage_error() {
        let temp = tempfile::tempdir().unwrap();
        let mut prompter = NonInteractivePrompter;
        let mut out = Vec::new();
        let o = SetupOptions {
            selected_runtimes: Some(vec!["claude".into()]),
            primary_runtime: Some("copilot".into()),
            ..opts()
        };
        let err =
            resolve_session(&o, temp.path(), temp.path(), &[], "", &mut prompter, &mut out).unwrap_err();
        assert!(matches!(err, SetupError::Usage(_)));
    }

    #[test]
    fn stored_state_is_used_without_prompting() {
        let temp = tempfile::tempdir().unwrap();
        let gal_dir = temp.path().join(".gal");
        fs::create_dir_all(&gal_dir).unwrap();
        fs::write(gal_dir.join("install-state.json"), "{}").unwrap();

        let mut prompter = ScriptedPrompter {
            runtimes: vec!["claude".into()],
            primary: "claude".into(),
            runtime_calls: 0,
            primary_calls: 0,
        };
        let mut out = Vec::new();
        let s = resolve_session(
            &opts(),
            temp.path(),
            temp.path(),
            &["claude".into(), "copilot".into()],
            "copilot",
            &mut prompter,
            &mut out,
        )
        .unwrap();
        assert_eq!(s.selected_runtimes, vec!["claude".to_string(), "copilot".to_string()]);
        assert_eq!(s.primary_runtime, "copilot");
        assert_eq!(prompter.runtime_calls, 0, "saved state must not prompt");
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("Using saved install state"));
    }

    #[test]
    fn first_run_without_state_prompts() {
        let temp = tempfile::tempdir().unwrap();
        let mut prompter = ScriptedPrompter {
            runtimes: vec!["claude".into(), "codex".into()],
            primary: "codex".into(),
            runtime_calls: 0,
            primary_calls: 0,
        };
        let mut out = Vec::new();
        let s = resolve_session(
            &opts(),
            temp.path(),
            temp.path(),
            &["claude".into()],
            "claude",
            &mut prompter,
            &mut out,
        )
        .unwrap();
        assert_eq!(prompter.runtime_calls, 1, "first run must prompt");
        assert_eq!(s.selected_runtimes, vec!["claude".to_string(), "codex".to_string()]);
        assert_eq!(s.primary_runtime, "codex");

        // Prompted selection is persisted (ports Get-InstallSelectionState save).
        let state_path = temp.path().join(".gal").join("install-state.json");
        let state = base::json_util::read_json_map(&state_path).unwrap();
        assert_eq!(
            state.get("selectedRuntimes").unwrap(),
            &serde_json::json!(["claude", "codex"])
        );
        assert_eq!(
            state.get("primaryRuntime").and_then(Value::as_str),
            Some("codex")
        );
    }

    #[test]
    fn first_run_dry_run_does_not_persist_selection() {
        let temp = tempfile::tempdir().unwrap();
        let mut prompter = NonInteractivePrompter;
        let mut out = Vec::new();
        let o = SetupOptions { dry_run: true, ..opts() };
        resolve_session(
            &o,
            temp.path(),
            temp.path(),
            &["claude".into()],
            "claude",
            &mut prompter,
            &mut out,
        )
        .unwrap();
        let state_path = temp.path().join(".gal").join("install-state.json");
        assert!(!state_path.exists(), "dry-run must not write install-state");
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("[DRY RUN] Would save install state"));
    }

    #[test]
    fn check_mode_never_persists_selection() {
        let temp = tempfile::tempdir().unwrap();
        let mut prompter = NonInteractivePrompter;
        let mut out = Vec::new();
        let o = SetupOptions { check: true, ..opts() };
        resolve_session(
            &o,
            temp.path(),
            temp.path(),
            &["claude".into()],
            "claude",
            &mut prompter,
            &mut out,
        )
        .unwrap();
        assert!(
            !temp.path().join(".gal").join("install-state.json").exists(),
            "--check must be read-only (C-7)"
        );
    }

    #[test]
    fn detect_runtimes_finds_gal_markers() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        fs::create_dir_all(home.join(".copilot").join("gal")).unwrap();
        fs::create_dir_all(home.join(".claude").join("skills").join("gal")).unwrap();
        let detected = detect_runtimes(home, home);
        assert_eq!(detected, vec!["copilot".to_string(), "claude".to_string()]);
    }

    #[test]
    fn detect_runtimes_empty_on_clean_machine() {
        let temp = tempfile::tempdir().unwrap();
        assert!(detect_runtimes(temp.path(), temp.path()).is_empty());
    }

    #[test]
    fn reconfigure_prompts_even_with_stored_state() {
        let temp = tempfile::tempdir().unwrap();
        let gal_dir = temp.path().join(".gal");
        fs::create_dir_all(&gal_dir).unwrap();
        fs::write(gal_dir.join("install-state.json"), "{}").unwrap();

        let mut prompter = ScriptedPrompter {
            runtimes: vec!["copilot".into()],
            primary: "copilot".into(),
            runtime_calls: 0,
            primary_calls: 0,
        };
        let mut out = Vec::new();
        let o = SetupOptions { reconfigure: true, ..opts() };
        let s = resolve_session(
            &o,
            temp.path(),
            temp.path(),
            &["claude".into()],
            "claude",
            &mut prompter,
            &mut out,
        )
        .unwrap();
        assert_eq!(prompter.runtime_calls, 1, "--reconfigure must prompt");
        assert_eq!(s.selected_runtimes, vec!["copilot".to_string()]);
    }

    #[test]
    fn non_interactive_prompter_accepts_defaults() {
        let mut p = NonInteractivePrompter;
        let defaults = vec!["claude".to_string()];
        assert_eq!(p.select_runtimes(&defaults), defaults);
        assert_eq!(p.select_primary(&defaults, "claude"), "claude");
    }
}
