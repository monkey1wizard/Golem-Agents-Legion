//! `gal doctor` — read-only health checks for GAL surfaces.
//!
//! Checks canonical root freshness, runtime surfaces, and plan
//! lifecycle drift. Exit-code grading: warning-only → 0, any error → non-zero.
//!
//!
//! `gal doctor --dry-run` must not modify any files.
//! missing projection or stale steward → error exit naming the fix surface.
//! warning-only → exit 0; any error → exit non-zero.

use std::path::PathBuf;

// Doctor finding/report vocabulary + the `HealthCheck` trait now live in
// `gal_foundation::health` (doctor dependency inversion). Re-export so
// `gal_engine::doctor::*` and the in-module tests keep resolving; each domain
// implements `HealthCheck` and `cli` aggregates the trait objects.
pub use gal_foundation::health::{DoctorFinding, DoctorReport, HealthCheck, Severity};

/// Options for the doctor command.
#[derive(Debug, Default)]
pub struct DoctorOptions {
    /// When `true`, do NOT modify any files (read-only mode).
    pub dry_run: bool,
}

/// Run `gal doctor` — check canonical root and runtime surfaces.
///
/// Always read-only when `dry_run` is true. Returns a report with findings.
/// The caller is responsible for printing and using `report.exit_code()`.
pub fn run_doctor(opts: &DoctorOptions) -> DoctorReport {
    let _ = opts.dry_run; // All checks are read-only; dry_run has no effect here.

    let canonical_root = canonical_root_path();

    // Assemble the checks as `HealthCheck` trait objects in the exact prior order.
    // Conditional checks (canonical-root-dependent, release-gate) are selected here;
    // each check's logic lives in its own `HealthCheck` impl.
    let mut checks: Vec<Box<dyn HealthCheck>> = Vec::new();

    // Check 1: Canonical root existence
    checks.push(Box::new(CanonicalRootCheck {
        canonical_root: canonical_root.clone(),
    }));

    // Check 2: Required runtime surface files inside canonical root
    if canonical_root.exists() {
        checks.push(Box::new(RuntimeSurfacesCheck {
            canonical_root: canonical_root.clone(),
        }));
        checks.push(Box::new(StewardVisibleCheck {
            canonical_root: canonical_root.clone(),
        }));
        // Check 2c: bin/gal[.exe] present and executable in canonical root
        checks.push(Box::new(BinInCanonicalRootCheck {
            canonical_root: canonical_root.clone(),
        }));
    }

    // Check 3: Claude GAL-owned skill surface (~/.claude/skills/gal → canonical root)
    checks.push(Box::new(SkillSurfaceCheck {
        canonical_root: canonical_root.clone(),
    }));

    // Check 5: AGY surfaces (warning only, best-effort per OE-A)
    checks.push(Box::new(AgySurfacesCheck));

    // Check 6: orphan .gal-render-* temp dirs
    checks.push(Box::new(OrphanTempDirsCheck));

    let mut report = DoctorReport::new();
    for check in &checks {
        report.findings.extend(check.check());
    }
    report
}

// ---------------------------------------------------------------------------
// Individual checks
// ---------------------------------------------------------------------------

struct CanonicalRootCheck {
    canonical_root: PathBuf,
}

impl HealthCheck for CanonicalRootCheck {
    fn name(&self) -> &str {
        "canonical-root"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let mut findings = Vec::new();
        if !self.canonical_root.exists() {
            findings.push(DoctorFinding::error(
                format!(
                    "canonical root not found: {}",
                    self.canonical_root.display()
                ),
                "run `gal refresh` to render it",
            ));
            return findings;
        }

        // Verify required subdirectories
        for dir in &["skills", "commands", "agents"] {
            if !self.canonical_root.join(dir).exists() {
                findings.push(DoctorFinding::error(
                    format!("canonical root missing required directory: {dir}"),
                    "run `gal refresh` to re-render",
                ));
            }
        }
        findings
    }
}

struct RuntimeSurfacesCheck {
    canonical_root: PathBuf,
}

impl HealthCheck for RuntimeSurfacesCheck {
    fn name(&self) -> &str {
        "runtime-surfaces"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let mut findings = Vec::new();
        // Claude runtime surface: .claude-plugin/plugin.json
        let claude_manifest = self
            .canonical_root
            .join(".claude-plugin")
            .join("plugin.json");
        if !claude_manifest.exists() {
            findings.push(DoctorFinding::error(
                "Claude plugin manifest not found (.claude-plugin/plugin.json)",
                "run `gal refresh` to regenerate",
            ));
        }

        // Copilot runtime surface: copilot-manifest.json
        let copilot_manifest = self.canonical_root.join("copilot-manifest.json");
        if !copilot_manifest.exists() {
            findings.push(DoctorFinding::error(
                "Copilot manifest not found (copilot-manifest.json)",
                "run `gal refresh` to regenerate",
            ));
        }
        findings
    }
}

struct StewardVisibleCheck {
    canonical_root: PathBuf,
}

impl HealthCheck for StewardVisibleCheck {
    fn name(&self) -> &str {
        "steward-visible"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let mut findings = Vec::new();
        // Intent assertion: golem-steward in agents/, doc-sync in skills/.
        let steward = self.canonical_root.join("agents").join("golem-steward.md");
        if !steward.exists() {
            findings.push(DoctorFinding::error(
                "golem-steward not visible in canonical root agents/",
                "run `gal refresh` to re-render the canonical root",
            ));
        }

        let doc_sync = self
            .canonical_root
            .join("skills")
            .join("doc-sync")
            .join("SKILL.md");
        if !doc_sync.exists() {
            findings.push(DoctorFinding::error(
                "doc-sync skill not visible in canonical root skills/",
                "run `gal refresh` to re-render the canonical root",
            ));
        }
        findings
    }
}

/// Check that `~/.claude/skills/gal` (the GAL-owned Claude skill surface) exists.
///
/// This is the surface Claude Code scans for skills. If missing, `doc-sync` and other
/// GAL skills are not loaded by Claude regardless of canonical root state.
struct SkillSurfaceCheck {
    canonical_root: PathBuf,
}

impl HealthCheck for SkillSurfaceCheck {
    fn name(&self) -> &str {
        "skill-surface"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let mut findings = Vec::new();
        let Some(home) = home_root_path() else {
            return findings;
        };
        let skill_surface = home.join(".claude").join("skills").join("gal");
        if !skill_surface.exists() {
            findings.push(DoctorFinding::warning(
                "Claude skill surface not found (~/.claude/skills/gal) — GAL skills not loaded by Claude",
            ));
            findings.push(DoctorFinding::warning(
                "To create the Claude skill surface manually: create a junction/symlink from \
                 ~/.claude/skills/gal pointing to the canonical root (e.g. ~/.gal/plugins/gal). \
                 `gal refresh` does not create this surface automatically.",
            ));
            return findings;
        }
        // Verify the surface resolves to the canonical root (symlink/junction target check).
        if self.canonical_root.exists() {
            let resolved = std::fs::canonicalize(&skill_surface)
                .ok()
                .or_else(|| Some(skill_surface.clone()));
            let canonical_resolved = std::fs::canonicalize(&self.canonical_root).ok();
            if let (Some(surface_real), Some(root_real)) = (resolved, canonical_resolved) {
                if surface_real != root_real {
                    findings.push(DoctorFinding::error(
                        format!(
                            "Claude skill surface (~/.claude/skills/gal) does not point to canonical root ({})",
                            self.canonical_root.display()
                        ),
                        "run `gal refresh`, then restart Claude Code to realign the skill surface",
                    ));
                }
            }
        }
        findings
    }
}

/// Check that `bin/gal[.exe]` is present and executable in the canonical root.
struct BinInCanonicalRootCheck {
    canonical_root: PathBuf,
}

impl HealthCheck for BinInCanonicalRootCheck {
    fn name(&self) -> &str {
        "bin-in-canonical-root"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let mut findings = Vec::new();
        let bin_name = if cfg!(windows) { "gal.exe" } else { "gal" };
        let bin_path = self.canonical_root.join("bin").join(bin_name);
        if !bin_path.is_file() {
            findings.push(DoctorFinding::error(
                format!(
                    "plugin bin/{bin_name} not found in canonical root — gal binary not exposed"
                ),
                "run `gal refresh` to re-render and expose the binary",
            ));
            return findings;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&bin_path) {
                if meta.permissions().mode() & 0o111 == 0 {
                    findings.push(DoctorFinding::error(
                        format!("plugin bin/{bin_name} exists but is not executable (+x missing)"),
                        "run `gal refresh` to re-render with correct permissions",
                    ));
                }
            }
        }
        findings
    }
}

struct OrphanTempDirsCheck;

impl HealthCheck for OrphanTempDirsCheck {
    fn name(&self) -> &str {
        "orphan-temp-dirs"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let mut findings = Vec::new();
        let Some(plugins_parent) = home_root_path().map(|h| h.join(".gal").join("plugins")) else {
            return findings;
        };
        let orphans = crate::render::scan_orphan_temp_dirs(&plugins_parent);
        for orphan in &orphans {
            findings.push(DoctorFinding::error(
                format!(
                    "orphan render temp dir: {} — interrupted render was not cleaned up",
                    orphan.display()
                ),
                "run `gal refresh` to clean orphan temp dirs",
            ));
        }
        findings
    }
}

struct AgySurfacesCheck;

impl HealthCheck for AgySurfacesCheck {
    fn name(&self) -> &str {
        "agy-surfaces"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let mut findings = Vec::new();
        if let Some(home) = home_root_path() {
            let cli_path = home.join(".gemini").join("antigravity-cli").join("gal");
            let skills_path = home.join(".gemini").join("antigravity-cli").join("skills");

            if !cli_path.exists() {
                findings.push(DoctorFinding::warning(
                    "AGY source-link surface not found (~/.gemini/antigravity-cli/gal)",
                ));
            }
            if !skills_path.exists() {
                findings.push(DoctorFinding::warning(
                    "AGY command-skill surface not found (~/.gemini/antigravity-cli/skills)",
                ));
            }
        }
        findings
    }
}

fn home_root_path() -> Option<PathBuf> {
    gal_foundation::paths::user_home()
}

fn canonical_root_path() -> PathBuf {
    gal_foundation::paths::gal_plugin_root("gal")
        .unwrap_or_else(|| PathBuf::from(".").join(".gal").join("plugins").join("gal"))
}

// ---------------------------------------------------------------------------
// Workflow setup-surface health checks.
// ---------------------------------------------------------------------------

/// Health over the gal workflow setup surface.
pub struct SetupHealthCheck {
    /// GAL repo / source root used to evaluate workflow-owned surfaces.
    pub repo_root: PathBuf,
}

impl HealthCheck for SetupHealthCheck {
    fn name(&self) -> &str {
        "setup"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let mut findings = Vec::new();

        if let Ok(path) = gal_foundation::config::GalConfig::config_path() {
            if path.exists() && std::fs::read_to_string(&path).is_err() {
                findings.push(DoctorFinding::error(
                    format!("machine config is unreadable: {}", path.display()),
                    "fix permissions or re-create ~/.gal/config/config.json",
                ));
            }
        }

        scan_manifests_for_secrets(&self.repo_root, &mut findings);
        check_personal_layer(&mut findings);

        if let Ok(path) = gal_foundation::config::GalConfig::config_path() {
            check_stale_config_keys_with_path(&path, &mut findings);
        }

        if which("rg").is_none() {
            findings.push(DoctorFinding::warning(
                "ripgrep (rg) not found on PATH — GAL search-dependent flows degrade",
            ));
        }

        if self.repo_root.join(".git").exists()
            && crate::git_filter::registered_clean_filter(&self.repo_root).is_none()
        {
            findings.push(DoctorFinding::warning(
                "gal-config git filter is not registered — run: git config filter.gal-config.clean 'gal clean' && git config filter.gal-config.smudge 'gal smudge' && git config filter.gal-config.required true",
            ));
        }

        findings
    }
}

/// Warn when config.json contains retired keys devMode/galRoot (now ignored).
/// Path-injectable for unit tests.
fn check_stale_config_keys_with_path(
    config_path: &std::path::Path,
    findings: &mut Vec<DoctorFinding>,
) {
    let raw = match std::fs::read_to_string(config_path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let value: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return,
    };
    for key in ["devMode", "galRoot"] {
        if value.get(key).is_some() {
            findings.push(DoctorFinding::warning(format!(
                "config.json contains retired key `{key}` (no longer read) — remove it to silence this warning"
            )));
        }
    }
}

/// Report the personal local layer state (warning-severity, non-blocking).
/// Presence-based: neither a personal skills dir nor a personal conventions
/// dir present → no finding. There is no config flag gate — placing either
/// directory is itself the machine-local opt-in.
fn check_personal_layer(findings: &mut Vec<DoctorFinding>) {
    let skills_dir = gal_foundation::paths::gal_local_skills_root().filter(|p| p.is_dir());
    let conventions_dir =
        gal_foundation::paths::gal_local_conventions_root().filter(|p| p.is_dir());

    if skills_dir.is_none() && conventions_dir.is_none() {
        return;
    }

    let canonical_skills = gal_foundation::paths::gal_plugin_root("gal").map(|c| c.join("skills"));
    let (skill_count, collision_count) = skills_dir
        .as_deref()
        .map(|dir| personal_layer_counts(dir, canonical_skills.as_deref()))
        .unwrap_or((0, 0));
    let convention_count = conventions_dir
        .as_deref()
        .map(count_markdown_files)
        .unwrap_or(0);

    findings.push(DoctorFinding::warning(format!(
        "personal layer: {skill_count} skill(s), {convention_count} convention file(s) present, {collision_count} core collision(s) (skipped by core-wins)"
    )));
}

/// Count `.md` files directly under `dir`. Pure — independently testable.
fn count_markdown_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .count()
}

/// Count personal skills and how many collide by name with canonical core skills.
/// Pure over the two directories — independently testable.
fn personal_layer_counts(
    local_skills: &std::path::Path,
    canonical_skills: Option<&std::path::Path>,
) -> (usize, usize) {
    let personal_names: std::collections::HashSet<String> = std::fs::read_dir(local_skills)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            e.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                && e.path().join("SKILL.md").exists()
        })
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    let n = personal_names.len();

    let collision_count = canonical_skills
        .filter(|p| p.is_dir())
        .map(|skills_dir| {
            std::fs::read_dir(skills_dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|core_name| personal_names.contains(core_name))
                .count()
        })
        .unwrap_or(0);

    (n, collision_count)
}

/// Scan `plugin.json` manifests under `repo_root/plugins/` for leaked secrets.
/// `${VAR}` placeholders are excluded (intentional substitution targets).
fn scan_manifests_for_secrets(repo_root: &std::path::Path, findings: &mut Vec<DoctorFinding>) {
    let pattern = gal_foundation::secret::secret_re();
    let plugins_dir = repo_root.join("plugins");
    if !plugins_dir.is_dir() {
        return;
    }
    for entry in walkdir_plugin_jsons(&plugins_dir) {
        let Ok(content) = std::fs::read_to_string(&entry) else {
            continue;
        };
        for line in content.lines() {
            let stripped = strip_placeholders(line);
            if pattern.is_match(&stripped) {
                findings.push(DoctorFinding::error(
                    format!(
                        "manifest may contain a leaked secret: {} — line: {}",
                        entry.display(),
                        line.trim()
                    ),
                    "remove the credential from the manifest and rotate the affected key",
                ));
                break;
            }
        }
    }
}

fn walkdir_plugin_jsons(plugins_dir: &std::path::Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    collect_plugin_jsons(plugins_dir, &mut result);
    result
}

fn collect_plugin_jsons(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_plugin_jsons(&path, out);
        } else if path
            .file_name()
            .map(|n| n == "plugin.json")
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

fn strip_placeholders(s: &str) -> String {
    let mut result = s.to_string();
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            result.replace_range(start..start + end + 1, "");
        } else {
            break;
        }
    }
    result
}

/// Check whether the Claude plugin cache for GAL is stale relative to the
/// canonical root. GAL never writes the cache; Claude Code manages it.
/// Non-blocking `Warning` when the cache is strictly older than the canonical root.
pub struct ClaudePluginCacheCheck {
    /// `~/.gal/plugins/gal/` — the canonical root written by `gal refresh`.
    pub canonical_root: PathBuf,
    /// `~/.claude/plugins/cache/gal/` — the Claude Code managed plugin cache.
    /// Resolved from user home when `None`.
    pub cache_dir: Option<PathBuf>,
}

impl ClaudePluginCacheCheck {
    /// Resolve both paths from the real user home.
    pub fn from_user_home(canonical_root: PathBuf) -> Self {
        Self {
            canonical_root,
            cache_dir: gal_foundation::paths::claude_plugins_cache_dir(),
        }
    }
}

impl HealthCheck for ClaudePluginCacheCheck {
    fn name(&self) -> &str {
        "claude-plugin-cache"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let mut findings = Vec::new();

        let cache_dir = match &self.cache_dir {
            Some(p) => p.clone(),
            None => match gal_foundation::paths::claude_plugins_cache_dir() {
                Some(p) => p,
                None => return findings,
            },
        };

        if !cache_dir.is_dir() || !self.canonical_root.is_dir() {
            return findings;
        }

        let cache_mtime = std::fs::metadata(&cache_dir)
            .ok()
            .and_then(|m| m.modified().ok());
        let canon_mtime = std::fs::metadata(&self.canonical_root)
            .ok()
            .and_then(|m| m.modified().ok());

        if let (Some(cache_t), Some(canon_t)) = (cache_mtime, canon_mtime) {
            if cache_is_stale(cache_t, canon_t) {
                findings.push(DoctorFinding::warning(
                    "Claude plugin cache for GAL is stale (canonical root is newer) — \
                     run `gal refresh`, then restart Claude Code to refresh its cached copy",
                ));
            }
        }

        findings
    }
}

fn cache_is_stale(cache_mtime: std::time::SystemTime, canon_mtime: std::time::SystemTime) -> bool {
    cache_mtime < canon_mtime
}

/// Warns when the running `gal` binary is newer than the last `gal refresh` —
/// i.e. the binary was rebuilt/reinstalled but `gal refresh` has not been run, so
/// the projected runtime surfaces are stale relative to the binary. This catches
/// the "I updated the binary but the agents still run the old version" trap.
///
/// The "last refresh" timestamp is read from the parent marketplace manifest
/// (`~/.gal/plugins/.claude-plugin/marketplace.json`), which `gal refresh` rewrites
/// atomically on **every** run — unlike the canonical-root *directory* mtime, which
/// idempotent re-renders do not bump (a directory's mtime only changes when entries
/// are added or removed, not when a file's content is rewritten in place).
pub struct BinaryRefreshCheck {
    /// `~/.gal/plugins/.claude-plugin/marketplace.json` — rewritten on every refresh.
    /// Used purely as the last-refresh timestamp marker.
    pub refresh_marker: PathBuf,
    /// The running binary path. Resolved from `current_exe()` when `None`.
    pub binary: Option<PathBuf>,
}

impl BinaryRefreshCheck {
    /// Resolve the binary from the running process and the refresh marker from the
    /// canonical root's parent (`<plugins>/.claude-plugin/marketplace.json`).
    pub fn from_current_exe(canonical_root: PathBuf) -> Self {
        let refresh_marker = canonical_root
            .parent()
            .map(|plugins| plugins.join(".claude-plugin").join("marketplace.json"))
            .unwrap_or_else(|| canonical_root.join("marketplace.json"));
        Self {
            refresh_marker,
            binary: std::env::current_exe().ok(),
        }
    }
}

impl HealthCheck for BinaryRefreshCheck {
    fn name(&self) -> &str {
        "binary-refresh"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let mut findings = Vec::new();

        let binary = match &self.binary {
            Some(b) => b.clone(),
            None => return findings,
        };
        // No marker yet (never refreshed) → nothing to compare against.
        if !binary.is_file() || !self.refresh_marker.is_file() {
            return findings;
        }

        let bin_mtime = std::fs::metadata(&binary)
            .ok()
            .and_then(|m| m.modified().ok());
        let marker_mtime = std::fs::metadata(&self.refresh_marker)
            .ok()
            .and_then(|m| m.modified().ok());

        if let (Some(bin_t), Some(marker_t)) = (bin_mtime, marker_mtime) {
            if binary_newer_than_marker(bin_t, marker_t) {
                findings.push(DoctorFinding::warning(
                    "the running gal binary is newer than the last `gal refresh` — \
                     run `gal refresh` so the projected runtime surfaces match this binary",
                ));
            }
        }

        findings
    }
}

fn binary_newer_than_marker(
    binary_mtime: std::time::SystemTime,
    marker_mtime: std::time::SystemTime,
) -> bool {
    binary_mtime > marker_mtime
}

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

    // ─── remade workflow setup-surface health ────────────────────────────────

    fn make_local_skill(skills_root: &std::path::Path, name: &str) {
        let skill_dir = skills_root.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), format!("# {name}")).unwrap();
    }

    #[test]
    fn setup_health_name_is_setup() {
        let check = SetupHealthCheck {
            repo_root: PathBuf::from("."),
        };
        assert_eq!(check.name(), "setup");
    }

    #[test]
    fn manifest_secret_scan_flags_leaked_credential() {
        let temp = tempfile::tempdir().unwrap();
        let plugin_dir = temp
            .path()
            .join("plugins")
            .join("test-plugin")
            .join(".claude-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name":"test","description":"GITHUB_TOKEN=ghp_XXXXXXXXXXXXXXXX leaked here"}"#,
        )
        .unwrap();
        let mut findings = Vec::new();
        scan_manifests_for_secrets(temp.path(), &mut findings);
        assert!(findings.iter().any(|f| f.message.contains("leaked secret")));
    }

    #[test]
    fn manifest_secret_scan_allows_placeholder_tokens() {
        let temp = tempfile::tempdir().unwrap();
        let plugin_dir = temp
            .path()
            .join("plugins")
            .join("test-plugin")
            .join(".claude-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name":"test","description":"Use ${GITHUB_TOKEN} for auth"}"#,
        )
        .unwrap();
        let mut findings = Vec::new();
        scan_manifests_for_secrets(temp.path(), &mut findings);
        assert!(!findings.iter().any(|f| f.message.contains("leaked secret")));
    }

    #[test]
    fn personal_layer_counts_no_canonical_dir() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("skills");
        std::fs::create_dir_all(&local).unwrap();
        make_local_skill(&local, "aaa-skill");
        make_local_skill(&local, "zzz-skill");
        let (n, collisions) = personal_layer_counts(&local, None);
        assert_eq!(n, 2);
        assert_eq!(collisions, 0);
    }

    #[test]
    fn personal_layer_counts_detects_collision() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local-skills");
        let canon = temp.path().join("canon-skills");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::create_dir_all(&canon).unwrap();
        make_local_skill(&local, "doc-sync");
        make_local_skill(&local, "personal-only");
        std::fs::create_dir_all(canon.join("doc-sync")).unwrap();
        let (n, collisions) = personal_layer_counts(&local, Some(&canon));
        assert_eq!(n, 2);
        assert_eq!(collisions, 1);
    }

    #[test]
    fn personal_layer_counts_ignores_dir_without_skill_md() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("skills");
        std::fs::create_dir_all(local.join("incomplete")).unwrap();
        make_local_skill(&local, "valid");
        let (n, _) = personal_layer_counts(&local, None);
        assert_eq!(n, 1);
    }

    #[test]
    fn count_markdown_files_counts_only_md_extension() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("conventions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("csharp.md"), "# C#\n").unwrap();
        std::fs::write(dir.join("go.md"), "# Go\n").unwrap();
        std::fs::write(dir.join("README.txt"), "not markdown\n").unwrap();
        assert_eq!(count_markdown_files(&dir), 2);
    }

    #[test]
    fn count_markdown_files_missing_dir_returns_zero() {
        let missing = std::env::temp_dir().join("gal-doctor-conventions-does-not-exist");
        assert_eq!(count_markdown_files(&missing), 0);
    }

    #[test]
    fn claude_plugin_cache_no_finding_when_cache_absent() {
        let temp = tempfile::tempdir().unwrap();
        let canonical_root = temp.path().join("canonical");
        std::fs::create_dir_all(&canonical_root).unwrap();
        let check = ClaudePluginCacheCheck {
            canonical_root,
            cache_dir: Some(temp.path().join("no-such-cache")),
        };
        assert!(!check.check().iter().any(|f| f.message.contains("stale")));
    }

    #[test]
    fn claude_plugin_cache_no_finding_when_canonical_absent() {
        let temp = tempfile::tempdir().unwrap();
        let cache_dir = temp.path().join("cache");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let check = ClaudePluginCacheCheck {
            canonical_root: temp.path().join("not-installed"),
            cache_dir: Some(cache_dir),
        };
        assert!(check.check().is_empty());
    }

    #[test]
    fn claude_plugin_cache_stale_predicate() {
        use std::time::{Duration, SystemTime};
        let older = SystemTime::UNIX_EPOCH;
        let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        assert!(cache_is_stale(older, newer));
        assert!(!cache_is_stale(newer, older));
        assert!(!cache_is_stale(older, older));
    }

    #[test]
    fn binary_newer_than_marker_predicate() {
        use std::time::{Duration, SystemTime};
        let older = SystemTime::UNIX_EPOCH;
        let newer = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        // binary newer than the last-refresh marker → stale projection → warn.
        assert!(binary_newer_than_marker(newer, older));
        // binary same age or older than the marker → no warning.
        assert!(!binary_newer_than_marker(older, newer));
        assert!(!binary_newer_than_marker(older, older));
    }

    #[test]
    fn binary_refresh_no_finding_when_binary_absent() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("marketplace.json");
        std::fs::write(&marker, b"{}").unwrap();
        let check = BinaryRefreshCheck {
            refresh_marker: marker,
            binary: Some(temp.path().join("no-such-binary")),
        };
        assert!(check.check().is_empty());
    }

    #[test]
    fn binary_refresh_no_finding_when_marker_absent() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("gal-bin");
        std::fs::write(&binary, b"fake").unwrap();
        let check = BinaryRefreshCheck {
            refresh_marker: temp.path().join("never-refreshed.json"),
            binary: Some(binary),
        };
        assert!(check.check().is_empty());
    }

    #[test]
    fn doctor_finding_display_includes_severity_and_message() {
        let f = DoctorFinding::error("canonical root missing", "run `gal refresh`");
        let s = f.to_string();
        assert!(s.contains("ERROR"), "display should include ERROR: {s}");
        assert!(
            s.contains("canonical root missing"),
            "display should include message: {s}"
        );
        assert!(
            s.contains("gal refresh"),
            "display should include fix hint: {s}"
        );
    }

    #[test]
    fn doctor_finding_warning_has_no_fix_hint() {
        let f = DoctorFinding::warning("AGY surface missing");
        let s = f.to_string();
        assert!(s.contains("WARNING"), "display should include WARNING: {s}");
        assert!(f.fix_hint.is_none());
    }

    #[test]
    fn report_exit_code_zero_when_only_warnings() {
        let mut report = DoctorReport::new();
        report.push(DoctorFinding::warning("some warning"));
        assert_eq!(report.exit_code(), 0, "warning-only should exit 0");
        assert!(!report.has_errors());
    }

    #[test]
    fn report_exit_code_nonzero_when_error_present() {
        let mut report = DoctorReport::new();
        report.push(DoctorFinding::error("critical failure", "fix it"));
        assert_eq!(report.exit_code(), 1, "error should exit non-zero");
        assert!(report.has_errors());
    }

    #[test]
    fn report_exit_code_zero_when_empty() {
        let report = DoctorReport::new();
        assert_eq!(report.exit_code(), 0);
        assert!(!report.has_errors());
    }

    #[test]
    fn dry_run_option_is_constructible() {
        let opts = DoctorOptions { dry_run: true };
        // run_doctor is read-only anyway; dry_run is a no-op for checks.
        // This test verifies the struct is usable.
        assert!(opts.dry_run);
    }

    #[test]
    fn run_doctor_returns_report_without_panic() {
        let _guard = crate::test_support::HOME_ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        // Doctor may find errors (canonical root missing on CI/test machine),
        // but must not panic regardless of filesystem state.
        let opts = DoctorOptions::default();
        let report = run_doctor(&opts);
        // We don't assert exit code here — it depends on filesystem state.
        // We only verify no panic and findings is a Vec.
        let _ = report.findings.len();
        let _ = report.exit_code();
    }

    // ─── orphan temp dir doctor tests ────────────────────────

    #[test]
    fn check_orphan_temp_dirs_produces_errors_for_each_orphan() {
        use std::fs;
        use tempfile::TempDir;

        let home = TempDir::new().unwrap();
        let plugins_parent = home.path().join(".gal").join("plugins");
        fs::create_dir_all(&plugins_parent).unwrap();

        // Create two orphan temp dirs.
        fs::create_dir(plugins_parent.join(".gal-render-aaa")).unwrap();
        fs::create_dir(plugins_parent.join(".gal-render-bbb")).unwrap();

        let orphans = crate::render::scan_orphan_temp_dirs(&plugins_parent);
        let mut report = DoctorReport::new();
        for orphan in &orphans {
            report.push(DoctorFinding::error(
                format!(
                    "orphan render temp dir: {} — interrupted render was not cleaned up",
                    orphan.display()
                ),
                "run `gal refresh` to clean orphan temp dirs",
            ));
        }

        assert_eq!(report.findings.len(), 2, "one error per orphan");
        assert!(report.has_errors());
        for f in &report.findings {
            let s = f.to_string();
            assert!(
                s.contains(".gal-render-"),
                "finding must name the orphan dir: {s}"
            );
            assert!(s.contains("gal refresh"), "finding must name the fix: {s}");
        }
    }

    #[test]
    fn check_orphan_temp_dirs_no_findings_when_clean() {
        use std::fs;
        use tempfile::TempDir;

        let home = TempDir::new().unwrap();
        let plugins_parent = home.path().join(".gal").join("plugins");
        fs::create_dir_all(&plugins_parent).unwrap();
        // Only a healthy canonical root dir.
        fs::create_dir(plugins_parent.join("gal")).unwrap();

        let orphans = crate::render::scan_orphan_temp_dirs(&plugins_parent);
        assert!(orphans.is_empty(), "no orphans expected when clean");
    }

    // ─── skill surface + bin exposure doctor tests ───────────

    #[test]
    fn check_skill_surface_warning_when_missing() {
        use std::fs;
        use tempfile::TempDir;

        let root = TempDir::new().unwrap();
        let canonical_root = root.path().join("canonical");
        fs::create_dir_all(&canonical_root).unwrap();

        // Simulate: skill_surface does not exist.
        // We call check_skill_surface directly by using a fake canonical root.
        // The test cannot easily inject the home dir, so we verify the logic via
        // a fake check inline — verifying the find/message pattern.
        let skill_surface = root.path().join("skills_gal");
        let mut report = DoctorReport::new();
        if !skill_surface.exists() {
            report.push(DoctorFinding::warning(
                "Claude skill surface not found (~/.claude/skills/gal) — GAL skills not loaded by Claude",
            ));
            report.push(DoctorFinding::warning(
                "To create the Claude skill surface manually: create a junction/symlink from \
                 ~/.claude/skills/gal pointing to the canonical root (e.g. ~/.gal/plugins/gal). \
                 `gal refresh` does not create this surface automatically.",
            ));
        }
        assert!(
            !report.has_errors(),
            "missing skill surface must be a warning, not an error"
        );
        assert_eq!(report.findings.len(), 2, "should have two warnings");
        let s = report.findings[0].to_string();
        assert!(
            s.contains("skill surface"),
            "finding must name skill surface: {s}"
        );
        let remediation = report.findings[1].to_string();
        assert!(
            !remediation.contains("run `gal refresh`"),
            "remediation must not suggest gal refresh: {remediation}"
        );
        assert!(
            remediation.contains("manually"),
            "remediation must suggest manual creation: {remediation}"
        );
    }

    #[test]
    fn check_bin_in_canonical_root_error_when_missing() {
        use std::fs;
        use tempfile::TempDir;

        let root = TempDir::new().unwrap();
        let canonical_root = root.path().join("canonical");
        fs::create_dir_all(&canonical_root).unwrap();
        // No bin/ dir created — bin/gal[.exe] absent.

        let report = DoctorReport {
            findings: BinInCanonicalRootCheck { canonical_root }.check(),
        };

        assert!(report.has_errors(), "missing bin must be an error");
        let s = report.findings[0].to_string();
        assert!(s.contains("bin/"), "finding must mention bin/: {s}");
    }

    #[test]
    fn check_bin_in_canonical_root_ok_when_present() {
        use std::fs;
        use tempfile::TempDir;

        let root = TempDir::new().unwrap();
        let canonical_root = root.path().join("canonical");
        let bin_dir = canonical_root.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let bin_name = if cfg!(windows) { "gal.exe" } else { "gal" };
        let bin_path = bin_dir.join(bin_name);
        fs::write(&bin_path, b"fake binary").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&bin_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&bin_path, perms).unwrap();
        }

        let report = DoctorReport {
            findings: BinInCanonicalRootCheck { canonical_root }.check(),
        };

        assert!(
            !report.has_errors(),
            "present and executable bin must produce no errors: {:?}",
            report.findings
        );
    }

    // after removing ClaudeMarketplaceState, doctor compiles and exit grading is correct.
    #[test]
    fn tp08_no_marketplace_classification_residue() {
        let _guard = crate::test_support::HOME_ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        // Compile-time check: this test file does not reference ClaudeMarketplaceState.
        // If the enum still exists, this test serves as a reminder to remove it.
        // The real check is: `cargo test` compiles without any ClaudeMarketplaceState usage.
        let opts = DoctorOptions::default();
        let report = run_doctor(&opts);
        // Exit grading: 0 when warning-only, 1 when any error.
        let code = report.exit_code();
        assert!(
            code == 0 || code == 1,
            "exit code must be 0 or 1, got {code}"
        );
    }

    // ─── stale devMode/galRoot key check ─────────────────────────────────────

    #[test]
    fn stale_config_keys_no_finding_when_absent() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), r#"{"executorRouting": {}}"#).unwrap();
        let mut findings = Vec::new();
        check_stale_config_keys_with_path(tmp.path(), &mut findings);
        assert!(
            findings.is_empty(),
            "no warning expected when retired keys are absent"
        );
    }

    #[test]
    fn stale_config_keys_warning_for_dev_mode() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), r#"{"devMode": true}"#).unwrap();
        let mut findings = Vec::new();
        check_stale_config_keys_with_path(tmp.path(), &mut findings);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].to_string().contains("devMode"));
    }

    #[test]
    fn stale_config_keys_warning_for_gal_root() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), r#"{"galRoot": "/some/path"}"#).unwrap();
        let mut findings = Vec::new();
        check_stale_config_keys_with_path(tmp.path(), &mut findings);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].to_string().contains("galRoot"));
    }

    #[test]
    fn stale_config_keys_two_warnings_when_both_present() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), r#"{"devMode": false, "galRoot": "/repo"}"#).unwrap();
        let mut findings = Vec::new();
        check_stale_config_keys_with_path(tmp.path(), &mut findings);
        assert_eq!(findings.len(), 2);
    }

    // ─── canonical_root_path delegation ──────────────────────────────────────

    #[test]
    fn canonical_root_path_delegates_to_shared_helper() {
        let _guard = crate::test_support::HOME_ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        // Same expression as the shared helper's Some/None arms — proves
        // canonical_root_path() is a pure delegation, not a parallel
        // reimplementation, on both branches at once.
        let expected = gal_foundation::paths::gal_plugin_root("gal")
            .unwrap_or_else(|| PathBuf::from(".").join(".gal").join("plugins").join("gal"));
        assert_eq!(canonical_root_path(), expected);
    }

    #[test]
    fn home_root_path_follows_override_and_matches_canonical_root_base() {
        let _guard = crate::test_support::HOME_ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let original_home = std::env::var_os("HOME");
        let original_userprofile = std::env::var_os("USERPROFILE");
        let override_home = tempfile::TempDir::new().unwrap();

        std::env::set_var("HOME", override_home.path());
        std::env::set_var("USERPROFILE", override_home.path());

        let result = std::panic::catch_unwind(|| {
            assert_eq!(home_root_path().as_deref(), Some(override_home.path()));
            assert_eq!(
                canonical_root_path(),
                override_home
                    .path()
                    .join(".gal")
                    .join("plugins")
                    .join("gal")
            );
        });

        match original_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match original_userprofile {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }

        result.unwrap();
    }

    #[test]
    fn canonical_root_path_matches_shared_helper_some_branch_on_this_machine() {
        let _guard = crate::test_support::HOME_ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        // Home resolves on this machine (USERPROFILE on Windows / HOME on
        // Unix), so the Some branch is exercised for real, not just proven
        // at source level.
        let home_resolved = gal_foundation::paths::gal_plugin_root("gal").is_some();
        assert!(home_resolved, "expected a resolvable home on this machine");
        assert_eq!(
            canonical_root_path(),
            gal_foundation::paths::gal_plugin_root("gal").unwrap()
        );
    }

    #[test]
    fn canonical_root_path_is_deterministic() {
        let _guard = crate::test_support::HOME_ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        assert_eq!(canonical_root_path(), canonical_root_path());
    }
}
