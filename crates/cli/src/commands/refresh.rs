//! `gal refresh` — idempotent machine-level rebuild of the canonical plugin root,
//! parent Claude marketplace manifest, and four machine-runtime projections from
//! source (Copilot, Codex, Antigravity, and OpenCode).
//!
//! Doubles as the dev restore/rebuild primitive after derived content is broken or
//! deleted. All output is rebuildable from source; no backup needed.
//!
//! Boundary: writes only GAL's own `~/.gal` tree (canonical + marketplace.json).
//! Does NOT write Claude-managed `~/.claude/plugins/cache` or `installed_plugins.json`.
//! Claude re-copies via restart / re-add after a refresh.
//!
//! Internal binary subcommand; NOT a public `/gal` slash command.

use super::doctor::required_shared_skill_names;
use gal_engine::render::{
    materialize_embedded_source, render_canonical_root_from, resolve_source_from_cwd,
    resolve_source_from_exe_dir, RenderError,
};
use gal_engine::ExitCode;
use gal_foundation::health::HealthCheck;
use gal_foundation::paths::{gal_plugins_root, plugins_lock_path, user_home};
use projection::{
    machine_skill_options, run_update_commands, run_update_skills, AdapterError, ProjectionReport,
    SkillsProjectionHealthCheck,
};
use std::path::Path;

/// All errors that `refresh_with` can return.
#[derive(Debug)]
pub(crate) enum RefreshError {
    NoHome,
    Render(RenderError),
    Projection(AdapterError),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefreshError::NoHome => write!(f, "could not determine home directory"),
            RefreshError::Render(e) => write!(f, "render error: {e}"),
            RefreshError::Projection(e) => write!(f, "projection error: {e}"),
            RefreshError::Io(e) => write!(f, "I/O error: {e}"),
            RefreshError::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl From<RenderError> for RefreshError {
    fn from(e: RenderError) -> Self {
        RefreshError::Render(e)
    }
}

impl From<AdapterError> for RefreshError {
    fn from(e: AdapterError) -> Self {
        RefreshError::Projection(e)
    }
}

impl From<std::io::Error> for RefreshError {
    fn from(e: std::io::Error) -> Self {
        RefreshError::Io(e)
    }
}

impl From<serde_json::Error> for RefreshError {
    fn from(e: serde_json::Error) -> Self {
        RefreshError::Json(e)
    }
}

/// The canonical content of `~/.gal/plugins/.claude-plugin/marketplace.json`.
///
/// No `version` key — Claude uses the plugin content hash as the cache key
/// once git-source marketplace auto-refresh is available (pending public release).
pub(crate) fn marketplace_json() -> serde_json::Value {
    serde_json::json!({
        "name": "gal",
        "owner": { "name": "GAL" },
        "description": "GAL workflow command plugin — planning, pipeline, agents, and cross-runtime projection.",
        "plugins": [
            {
                "name": "gal",
                "source": "./gal",
                "description": "GAL workflow commands and golem agents."
            }
        ]
    })
}

/// Core refresh logic. Separated from the CLI entry so it is unit-testable.
///
/// Steps:
/// 1. Render canonical plugin root (`~/.gal/plugins/gal`).
/// 2. Project skills + commands to four machine runtimes
///    (copilot/codex/agy/opencode); Claude reads the canonical root.
/// 3. Write `~/.gal/plugins/.claude-plugin/marketplace.json`.
///
/// Deterministic: re-rendered content is byte-identical, including the
/// per-runtime plugin manifests — their `version` field is a content-digest
/// stamp (`<gal_version>-g<digest>`) of the portable render tree, not a
/// timestamp, so an unchanged source yields an unchanged `~/.gal` tree
/// between runs.
/// Fail-safe via the a2 `ManagedArtifactRegistry` lockfile guard — never deletes
/// non-GAL plugins.
pub(crate) fn refresh_with(source_root: &Path) -> Result<(), RefreshError> {
    // Use the same home-dir source as gal_foundation::paths helpers so render
    // and projection resolve to the same root (reads USERPROFILE/HOME, not the
    // Windows profile API, which makes this testable via env-var injection).
    let home = user_home().ok_or(RefreshError::NoHome)?;

    // Step 1 — render canonical root.
    let canonical = render_canonical_root_from(source_root, env!("CARGO_PKG_VERSION"))?;
    println!("  canonical root: {}", canonical.display());

    // Step 2 — project skills + commands to the four machine runtimes.
    let opts = machine_skill_options(source_root, &home)?;
    let skill_report = run_update_skills(&opts)?;
    let cmd_report = run_update_commands(&opts)?;

    let written = skill_report.written_files.len() + cmd_report.written_files.len();
    let created = skill_report.created_links.len() + cmd_report.created_links.len();
    let warnings = skill_report.warnings.len() + cmd_report.warnings.len();
    for w in skill_report
        .warnings
        .iter()
        .chain(cmd_report.warnings.iter())
    {
        eprintln!("  warning: {}", w.message);
    }
    println!("  projection: {written} written, {created} links created, {warnings} warning(s)");

    // Step 3 — write parent Claude marketplace manifest (GAL's own ~/.gal tree only).
    write_marketplace_json(&home)?;

    // Step 3b — write the standard Agent Plugins marketplace manifest alongside it.
    write_agents_marketplace_json(&home)?;

    // Step 4 — Codex shared-skill-surface summary (read-only; reuses the same
    // required-skill inventory health check `gal doctor` runs).
    print_codex_surface_summary(&canonical, &home, &skill_report, &cmd_report);

    Ok(())
}

fn print_codex_surface_summary(
    canonical_root: &Path,
    home: &Path,
    skill_report: &ProjectionReport,
    cmd_report: &ProjectionReport,
) {
    println!(
        "{}",
        build_codex_surface_summary(canonical_root, home, skill_report, cmd_report)
    );
}

/// Build a read-only Codex shared-skill-surface summary: required-skill
/// filesystem status, zombie-cleanup count, and budget footprint (all sourced
/// from [`SkillsProjectionHealthCheck`], the same machinery `gal doctor` uses),
/// followed by a fixed auto-detect-first verification instruction. Pure and
/// independently testable (extracted from [`print_codex_surface_summary`] so
/// content can be asserted without capturing real stdout).
///
/// Deliberately reports only filesystem-observable facts. `gal refresh`
/// cannot observe whether Codex actually surfaced a skill in its current
/// thread — claiming that would be a surfaced-success claim this function
/// must never make.
fn build_codex_surface_summary(
    canonical_root: &Path,
    home: &Path,
    skill_report: &ProjectionReport,
    cmd_report: &ProjectionReport,
) -> String {
    let mut lines = vec!["Codex skill surface:".to_string()];

    let required_names = required_shared_skill_names(canonical_root);
    let shared_skills_root = home.join(".agents").join("skills");
    let lockfile_path = plugins_lock_path();
    let findings = SkillsProjectionHealthCheck::with_inventory(
        shared_skills_root,
        required_names,
        lockfile_path,
    )
    .check();
    let (budget_findings, skill_findings): (Vec<_>, Vec<_>) = findings
        .into_iter()
        .partition(|f| f.message.contains("description footprint"));

    if skill_findings.is_empty() {
        lines.push("  required skills: all present and materialized".to_string());
    } else {
        for finding in &skill_findings {
            lines.push(format!("  {finding}"));
        }
    }
    if budget_findings.is_empty() {
        lines.push("  budget: within the initial-list context budget".to_string());
    } else {
        for finding in &budget_findings {
            lines.push(format!("  {finding}"));
        }
    }

    let zombies_removed = skill_report.removed_paths.len() + cmd_report.removed_paths.len();
    lines.push(format!(
        "  zombie cleanup: {zombies_removed} stale entr{} removed",
        if zombies_removed == 1 { "y" } else { "ies" }
    ));

    lines.push(
        "  Codex auto-detects skill changes; if not appearing, restart Codex or open a fresh \
         thread, then verify via `/skills` or `$gal-status`."
            .to_string(),
    );

    lines.join("\n")
}

/// Write `~/.gal/plugins/.claude-plugin/marketplace.json` atomically.
fn write_marketplace_json(home: &Path) -> Result<(), RefreshError> {
    let plugins_root = gal_plugins_root().unwrap_or_else(|| home.join(".gal").join("plugins"));
    let dir = plugins_root.join(".claude-plugin");
    std::fs::create_dir_all(&dir)?;

    let target = dir.join("marketplace.json");
    let content = serde_json::to_string_pretty(&marketplace_json())?;

    // Atomic write: tmp sibling → rename.
    let tmp = target.with_extension("json.tmp");
    std::fs::write(&tmp, &content)?;
    std::fs::rename(&tmp, &target)?;

    println!("  marketplace: {}", target.display());
    Ok(())
}

/// The canonical content of `~/.gal/plugins/.agents/plugins/marketplace.json` —
/// the standard Agent Plugins marketplace manifest, parallel to
/// [`marketplace_json`]'s Claude-specific one.
///
/// No `version` key, for the same reason as [`marketplace_json`].
pub(crate) fn agents_marketplace_json() -> serde_json::Value {
    serde_json::json!({
        "name": "gal",
        "plugins": [
            {
                "name": "gal",
                "source": { "source": "local", "path": "./gal" },
                "interface": { "displayName": "Golem Agents Legion" }
            }
        ]
    })
}

/// Write `~/.gal/plugins/.agents/plugins/marketplace.json` atomically.
fn write_agents_marketplace_json(home: &Path) -> Result<(), RefreshError> {
    let plugins_root = gal_plugins_root().unwrap_or_else(|| home.join(".gal").join("plugins"));
    let dir = plugins_root.join(".agents").join("plugins");
    std::fs::create_dir_all(&dir)?;

    let target = dir.join("marketplace.json");
    let content = serde_json::to_string_pretty(&agents_marketplace_json())?;

    // Atomic write: tmp sibling → rename.
    let tmp = target.with_extension("json.tmp");
    std::fs::write(&tmp, &content)?;
    std::fs::rename(&tmp, &target)?;

    println!("  agents marketplace: {}", target.display());
    Ok(())
}

/// cwd-first, exe-dir-fallback source resolution — pure and independently
/// testable (no env access). `gal refresh` run from within a repo checkout (or
/// a subdirectory of it) resolves from `cwd` without needing `--source`, even
/// when the installed binary lives elsewhere (e.g. `~/.cargo/bin`); falls back
/// to the packaged-binary resolution when `cwd` yields nothing.
fn resolve_refresh_source(
    cwd: Option<&Path>,
    exe_dir: Option<&Path>,
) -> Option<std::path::PathBuf> {
    cwd.and_then(resolve_source_from_cwd)
        .or_else(|| exe_dir.and_then(resolve_source_from_exe_dir))
}

/// Build the "could not locate GAL source root" diagnostic — pure and
/// independently testable, so the attempted-locations + `--source` hint
/// content is verifiable without depending on a real process's exe/cwd state.
fn refresh_source_not_found_message(cwd: Option<&Path>, exe_dir: Option<&Path>) -> String {
    let mut lines = vec!["gal refresh: could not locate GAL source root".to_string()];
    lines.push("  tried:".to_string());
    lines.push(match cwd {
        Some(c) => format!(
            "    - cwd and its ancestors (repo-checkout / flat layout): {}",
            c.display()
        ),
        None => "    - cwd: could not determine current directory".to_string(),
    });
    lines.push(match exe_dir {
        Some(d) => format!("    - binary directory and its ancestors: {}", d.display()),
        None => "    - binary directory: could not determine binary path".to_string(),
    });
    lines.push("    - embedded payload (Cargo channel): none baked into this binary".to_string());
    lines.push(
        "  pass --source <path> (e.g. --source ./plugins/gal-core, pointing at a repo \
         checkout's plugins/gal-core), or reinstall via a packaging channel."
            .to_string(),
    );
    lines.join("\n")
}

/// Strip the Windows verbatim `\\?\` prefix (and map `\\?\UNC\` back to `\\`)
/// that `std::fs::canonicalize` adds but `cmd` builtins (`mklink` / `rmdir`)
/// reject. No-op on a string without the prefix, so it is safe on all platforms.
fn strip_verbatim_prefix(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s.to_string()
    }
}

/// Resolve `p` to an absolute, native-separator path so the projection layer's
/// Windows junction calls (`cmd /C mklink /J` / `rmdir`) cannot mis-parse a
/// relative `/segment` (e.g. `./plugins/gal-core`) as a command switch.
/// Falls back to the input path if canonicalization fails.
fn normalize_source_root(p: std::path::PathBuf) -> std::path::PathBuf {
    match std::fs::canonicalize(&p) {
        Ok(c) => std::path::PathBuf::from(strip_verbatim_prefix(&c.to_string_lossy())),
        Err(_) => p,
    }
}

/// `gal refresh` CLI entry point.
///
/// Resolves the plugin source root from the binary location, then calls
/// `refresh_with`. Prints a one-line header and summary.
pub(crate) fn cmd_refresh(args: &[String]) -> ExitCode {
    // Accepted flags: `--help`, and `--source <path>` / `--source=<path>` to point
    // refresh at an explicit source root (e.g. a dev checkout's `plugins/gal-core`)
    // instead of resolving from the binary's own directory.
    let extra = &args[1..];
    let mut source_override: Option<std::path::PathBuf> = None;
    let mut i = 0;
    while i < extra.len() {
        let a = &extra[i];
        if a == "--help" || a == "-h" {
            println!(
                "gal refresh — rebuild canonical plugin root + runtime projections from source"
            );
            println!();
            println!("Usage: gal refresh [--source <path>]");
            println!();
            println!("  --source <path>   Use an explicit source root (a dir holding");
            println!("                    skills/ + agents/ + commands/, e.g. plugins/gal-core).");
            println!("                    Defaults to the layout beside the binary.");
            println!();
            println!(
                "Deterministic re-render; per-runtime manifest version fields update each run."
            );
            println!("Writes only GAL's own ~/.gal tree.");
            println!("Claude re-picks up the updated plugin after restart / re-add.");
            return ExitCode::Success;
        } else if let Some(v) = a.strip_prefix("--source=") {
            source_override = Some(std::path::PathBuf::from(v));
        } else if a == "--source" {
            i += 1;
            match extra.get(i) {
                Some(v) => source_override = Some(std::path::PathBuf::from(v)),
                None => {
                    eprintln!("gal refresh: --source requires a path argument");
                    return ExitCode::Usage;
                }
            }
        } else {
            eprintln!("gal refresh: unknown option '{a}'");
            return ExitCode::Usage;
        }
        i += 1;
    }

    let source_root = match source_override {
        Some(p) => {
            if !p.is_dir() {
                eprintln!(
                    "gal refresh: --source path is not a directory: {}",
                    p.display()
                );
                return ExitCode::Usage;
            }
            p
        }
        None => {
            let cwd = std::env::current_dir().ok();
            // Canonicalize the exe path first so a symlinked entry point resolves
            // to the real install dir — winget's user entry is a symlink at
            // `%LOCALAPPDATA%\...\WinGet\Links\gal.exe`, and current_exe() may
            // return that Links path, where the exe-side payload is absent.
            let exe_dir = std::env::current_exe()
                .ok()
                .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
                .and_then(|p| p.parent().map(|d| d.to_path_buf()));

            // cwd → exe-dir → embedded materialization (Cargo bare-binary case).
            match resolve_refresh_source(cwd.as_deref(), exe_dir.as_deref())
                .or_else(materialize_embedded_source)
            {
                Some(r) => r,
                None => {
                    eprintln!(
                        "{}",
                        refresh_source_not_found_message(cwd.as_deref(), exe_dir.as_deref())
                    );
                    return ExitCode::Error;
                }
            }
        }
    };

    // Absolutize + native-normalize before handing to the projection layer.
    // On Windows the projection creates junctions via `cmd /C mklink /J <link>
    // <target>`; a relative, forward-slash source (e.g. `./plugins/gal-core`)
    // makes the cmd builtin mis-parse `/plugins` as a switch
    // ("Invalid switch - plugins"). Junction targets must be absolute anyway.
    let source_root = normalize_source_root(source_root);

    println!("gal refresh");
    println!("  source: {}", source_root.display());

    match refresh_with(&source_root) {
        Ok(()) => {
            println!("done.");
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal refresh: {e}");
            ExitCode::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Shared process-global env guard (crate::commands::ENV_GUARD) serialises every
    // env/cwd-mutating test in this binary — refresh and restore must share ONE mutex
    // or they race on global HOME. Poison-tolerant via into_inner at each lock site.
    use crate::commands::ENV_GUARD;

    fn set_home(tmp: &Path) {
        #[cfg(windows)]
        std::env::set_var("USERPROFILE", tmp);
        #[cfg(not(windows))]
        std::env::set_var("HOME", tmp);
    }

    fn source_root() -> PathBuf {
        // Use the real repo's plugins/gal-core as source so render passes doc-sync validation.
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest
            .ancestors()
            .nth(2)
            .unwrap()
            .join("plugins")
            .join("gal-core")
    }

    #[test]
    fn refresh_with_writes_canonical_and_marketplace() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        set_home(tmp.path());

        let src = source_root();
        assert!(
            src.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
            src.display()
        );

        refresh_with(&src).expect("refresh_with should succeed");

        // Canonical root written.
        let canonical_skill = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join("gal")
            .join("commands")
            .join("gal-pipeline")
            .join("SKILL.md");
        assert!(
            canonical_skill.exists(),
            "canonical SKILL.md must be written"
        );

        // Marketplace written.
        let marketplace = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join(".claude-plugin")
            .join("marketplace.json");
        assert!(marketplace.exists(), "marketplace.json must be written");

        let raw = std::fs::read_to_string(&marketplace).unwrap();
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["name"], "gal");
        assert_eq!(val["plugins"][0]["source"], "./gal");
        assert!(val.get("version").is_none(), "no version key");

        // Standard Agent Plugins marketplace written alongside it.
        let agents_marketplace = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join(".agents")
            .join("plugins")
            .join("marketplace.json");
        assert!(
            agents_marketplace.exists(),
            "agents marketplace.json must be written"
        );

        let agents_raw = std::fs::read_to_string(&agents_marketplace).unwrap();
        let agents_val: serde_json::Value = serde_json::from_str(&agents_raw).unwrap();
        assert_eq!(agents_val["name"], "gal");
        assert_eq!(agents_val["plugins"][0]["source"]["source"], "local");
        assert_eq!(agents_val["plugins"][0]["source"]["path"], "./gal");
        assert_eq!(
            agents_val["plugins"][0]["interface"]["displayName"],
            "Golem Agents Legion"
        );
        assert!(
            agents_val.get("version").is_none(),
            "no version key in agents marketplace.json"
        );

        // Canonical .mcp.json carries the exact markitdown-mcp pin (copied
        // verbatim from plugins/gal-core/mcp.json — no @latest for markitdown).
        let canonical_mcp = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join("gal")
            .join(".mcp.json");
        assert!(
            canonical_mcp.exists(),
            "canonical .mcp.json must be written"
        );
        let mcp_raw = std::fs::read_to_string(&canonical_mcp).unwrap();
        assert!(
            mcp_raw.contains("markitdown-mcp@0.0.1a4"),
            "canonical .mcp.json must pin markitdown-mcp@0.0.1a4, got: {mcp_raw}"
        );
        assert!(
            !mcp_raw.contains("markitdown-mcp@latest"),
            "canonical .mcp.json must not carry markitdown-mcp@latest"
        );
    }

    #[test]
    fn refresh_with_idempotent() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        set_home(tmp.path());

        let src = source_root();
        assert!(
            src.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
            src.display()
        );

        refresh_with(&src).expect("first run");

        let marketplace = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join(".claude-plugin")
            .join("marketplace.json");
        let first = std::fs::read(&marketplace).unwrap();

        let agents_marketplace = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join(".agents")
            .join("plugins")
            .join("marketplace.json");
        let agents_first = std::fs::read(&agents_marketplace).unwrap();

        refresh_with(&src).expect("second run");
        let second = std::fs::read(&marketplace).unwrap();
        let agents_second = std::fs::read(&agents_marketplace).unwrap();

        // Scope: both marketplace manifests carry no version field, so they are
        // byte-identical on re-run. The per-runtime plugin manifests carry a
        // content-digest stamp (not a timestamp) and are byte-identical on re-run
        // too, but their determinism is covered by the gal-engine render tests
        // rather than here.
        assert_eq!(
            first, second,
            "marketplace.json must be byte-identical on re-run"
        );
        assert_eq!(
            agents_first, agents_second,
            "agents marketplace.json must be byte-identical on re-run"
        );
    }

    #[test]
    fn strip_verbatim_prefix_cases() {
        assert_eq!(
            strip_verbatim_prefix(r"\\?\C:\Code\plugins\gal-core"),
            r"C:\Code\plugins\gal-core"
        );
        assert_eq!(
            strip_verbatim_prefix(r"\\?\UNC\server\share\x"),
            r"\\server\share\x"
        );
        // No prefix → unchanged (covers the Unix path shape too).
        assert_eq!(
            strip_verbatim_prefix("/home/u/plugins/gal-core"),
            "/home/u/plugins/gal-core"
        );
    }

    #[test]
    fn refresh_with_preserves_non_gal_plugin() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        set_home(tmp.path());

        // Place a non-GAL plugin in ~/.gal/plugins before refresh.
        let external = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join("my-other-plugin");
        std::fs::create_dir_all(&external).unwrap();
        let sentinel = external.join("sentinel.txt");
        std::fs::write(&sentinel, "user-owned").unwrap();

        let src = source_root();
        assert!(
            src.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
            src.display()
        );

        refresh_with(&src).expect("refresh_with should not delete non-GAL plugins");

        assert!(
            sentinel.exists(),
            "non-GAL plugin file must survive refresh"
        );
    }

    // cwd-first source resolution — pure-function coverage.

    fn make_source_markers(root: &Path) {
        std::fs::create_dir_all(root.join("skills")).unwrap();
        std::fs::create_dir_all(root.join("agents")).unwrap();
        std::fs::create_dir_all(root.join("commands")).unwrap();
    }

    #[test]
    fn resolve_refresh_source_prefers_cwd_over_exe_dir() {
        let cwd_tmp = TempDir::new().unwrap();
        let cwd_source = cwd_tmp.path().join("plugins").join("gal-core");
        make_source_markers(&cwd_source);

        let exe_tmp = TempDir::new().unwrap();
        make_source_markers(exe_tmp.path());

        let resolved = resolve_refresh_source(Some(cwd_tmp.path()), Some(exe_tmp.path()));
        assert_eq!(
            resolved.as_deref(),
            Some(cwd_source.as_path()),
            "cwd resolution must win even when exe-dir would also resolve"
        );
    }

    #[test]
    fn resolve_refresh_source_falls_back_to_exe_dir_when_cwd_fails() {
        let cwd_tmp = TempDir::new().unwrap();
        let no_markers_cwd = cwd_tmp.path().join("nowhere");
        std::fs::create_dir_all(&no_markers_cwd).unwrap();

        let exe_tmp = TempDir::new().unwrap();
        make_source_markers(exe_tmp.path());

        let resolved = resolve_refresh_source(Some(&no_markers_cwd), Some(exe_tmp.path()));
        assert_eq!(resolved.as_deref(), Some(exe_tmp.path()));
    }

    #[test]
    fn resolve_refresh_source_none_when_both_fail() {
        let cwd_tmp = TempDir::new().unwrap();
        let no_markers_cwd = cwd_tmp.path().join("nowhere");
        std::fs::create_dir_all(&no_markers_cwd).unwrap();

        let exe_tmp = TempDir::new().unwrap();
        let no_markers_exe = exe_tmp.path().join("bin");
        std::fs::create_dir_all(&no_markers_exe).unwrap();

        assert!(resolve_refresh_source(Some(&no_markers_cwd), Some(&no_markers_exe)).is_none());
    }

    #[test]
    fn refresh_source_not_found_message_lists_attempted_locations_and_source_hint() {
        let cwd = Path::new("/some/cwd");
        let exe_dir = Path::new("/some/exe/dir");
        let message = refresh_source_not_found_message(Some(cwd), Some(exe_dir));

        assert!(
            message.contains("/some/cwd"),
            "must list the attempted cwd: {message}"
        );
        assert!(
            message.contains("/some/exe/dir"),
            "must list the attempted exe dir: {message}"
        );
        assert!(
            message.contains("--source"),
            "must include the --source usage hint: {message}"
        );
        assert!(
            message.contains("plugins/gal-core"),
            "the --source example must point at a repo checkout's plugins/gal-core: {message}"
        );
    }

    /// End-to-end proof that `cmd_refresh` (not just the pure resolver) really
    /// consults cwd first: run with no `--source` from a real repo-checkout cwd
    /// and confirm it succeeds and actually renders from the resolved source.
    #[test]
    fn cmd_refresh_resolves_and_succeeds_from_repo_checkout_cwd() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let home_tmp = TempDir::new().unwrap();
        set_home(home_tmp.path());

        let src = source_root();
        assert!(
            src.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
            src.display()
        );
        let repo_root = src.parent().and_then(Path::parent).unwrap(); // plugins/gal-core -> plugins -> repo root

        let orig_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo_root).unwrap();
        let result = cmd_refresh(&["refresh".to_string()]);
        std::env::set_current_dir(orig_cwd).unwrap();

        assert_eq!(result, ExitCode::Success);
        let canonical_skill = home_tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join("gal")
            .join("commands")
            .join("gal-pipeline")
            .join("SKILL.md");
        assert!(
            canonical_skill.exists(),
            "cwd-resolved refresh must actually render the canonical root"
        );
    }

    // Codex surface summary — snapshot content, no surfaced-success wording.

    #[test]
    fn codex_surface_summary_has_three_segments_and_no_surfaced_success_claim() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let home_tmp = TempDir::new().unwrap();
        set_home(home_tmp.path());

        let src = source_root();
        assert!(
            src.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
            src.display()
        );

        // Render + project first so the summary reflects a real projected state.
        let canonical =
            gal_engine::render::render_canonical_root_from(&src, env!("CARGO_PKG_VERSION"))
                .unwrap();
        let opts = projection::machine_skill_options(&src, home_tmp.path()).unwrap();
        let skill_report = projection::run_update_skills(&opts).unwrap();
        let cmd_report = projection::run_update_commands(&opts).unwrap();

        let summary =
            build_codex_surface_summary(&canonical, home_tmp.path(), &skill_report, &cmd_report);

        assert!(
            summary.contains("required skills:") || summary.contains("required GAL skill"),
            "must have a required-skill status segment: {summary}"
        );
        assert!(
            summary.contains("budget:") || summary.contains("footprint"),
            "must have a budget segment: {summary}"
        );
        assert!(
            summary.contains("zombie cleanup:"),
            "must have a zombie-cleanup segment: {summary}"
        );
        assert!(
            summary.contains("auto-detects skill changes"),
            "must have the auto-detect-first instruction segment: {summary}"
        );
        assert!(
            summary.contains("restart Codex") || summary.contains("fresh thread"),
            "must name the restart/fresh-thread fallback: {summary}"
        );

        let banned = [
            "surfaced successfully",
            "now visible in Codex",
            "Codex surfaced",
            "successfully surfaced",
        ];
        for phrase in banned {
            assert!(
                !summary.contains(phrase),
                "must never claim surfaced success ('{phrase}'): {summary}"
            );
        }
    }
}
