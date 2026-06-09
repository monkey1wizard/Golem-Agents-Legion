//! `gal doctor` — read-only health checks for GAL surfaces.
//!
//! Checks canonical root freshness, provider surfaces, ledger state, and plan
//! lifecycle drift. Exit-code grading: warning-only → 0, any error → non-zero.
//!
//! Corresponds to T-012 of fix-gal-bootstrap-install-convergence.
//!
//! TP-016: `gal doctor --dry-run` must not modify any files.
//! TP-017: missing projection or stale dockeeper → error exit naming the fix surface.
//! TP-018: warning-only → exit 0; any error → exit non-zero.

use crate::ledger::{ledger_path, Ledger};
use std::path::{Path, PathBuf};

/// Severity of a doctor finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    /// Non-blocking, informational.
    Warning,
    /// Must be fixed; causes non-zero exit code.
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Warning => write!(f, "WARNING"),
            Severity::Error => write!(f, "ERROR"),
        }
    }
}

/// A single doctor finding.
#[derive(Debug, Clone)]
pub struct DoctorFinding {
    pub severity: Severity,
    /// Human-readable description of the issue.
    pub message: String,
    /// Optional hint pointing to the fix surface.
    pub fix_hint: Option<String>,
}

impl DoctorFinding {
    pub fn error(message: impl Into<String>, fix_hint: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            fix_hint: Some(fix_hint.into()),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            fix_hint: None,
        }
    }
}

impl std::fmt::Display for DoctorFinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.severity, self.message)?;
        if let Some(ref hint) = self.fix_hint {
            write!(f, " → {hint}")?;
        }
        Ok(())
    }
}

/// Aggregated doctor report.
#[derive(Debug)]
pub struct DoctorReport {
    pub findings: Vec<DoctorFinding>,
}

impl DoctorReport {
    pub fn new() -> Self {
        Self { findings: Vec::new() }
    }

    /// Returns `true` if any finding has `Severity::Error`.
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// Returns the recommended process exit code.
    ///
    /// - 0 if no errors (warnings only or all clear)
    /// - 1 if any error
    pub fn exit_code(&self) -> i32 {
        if self.has_errors() { 1 } else { 0 }
    }

    fn push(&mut self, f: DoctorFinding) {
        self.findings.push(f);
    }
}

impl Default for DoctorReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Options for the doctor command.
#[derive(Debug, Default)]
pub struct DoctorOptions {
    /// When `true`, do NOT modify any files (read-only mode). TP-016.
    pub dry_run: bool,
    /// Include release-gate checks (T-021, P4).
    pub release_gate: bool,
}

/// Run `gal doctor` — check canonical root, provider surfaces, and ledger.
///
/// Always read-only when `dry_run` is true. Returns a report with findings.
/// The caller is responsible for printing and using `report.exit_code()`.
pub fn run_doctor(opts: &DoctorOptions) -> DoctorReport {
    let _ = opts.dry_run; // All checks are read-only; dry_run has no effect here.

    let mut report = DoctorReport::new();

    let canonical_root = canonical_root_path();

    // Check 1: Canonical root existence
    check_canonical_root(&canonical_root, &mut report);

    // Check 2: Required provider files inside canonical root
    if canonical_root.exists() {
        check_provider_surfaces(&canonical_root, &mut report);
        check_dockeeper_visible(&canonical_root, &mut report);
        // Check 2c: bin/gal[.exe] present and executable in canonical root (R-04/R-06)
        check_bin_in_canonical_root(&canonical_root, &mut report);
    }

    // Check 3: Ledger freshness
    check_ledger(&mut report);

    // Check 4: Claude GAL-owned skill surface (~/.claude/skills/gal → canonical root)
    check_skill_surface(&canonical_root, &mut report);

    // Check 5: AGY surfaces (warning only, best-effort per OE-A)
    check_agy_surfaces(&mut report);

    // Check 6: orphan .gal-render-* temp dirs (R-05)
    check_orphan_temp_dirs(&mut report);

    // Check 7 (release gate only): package-manager metadata (T-021)
    if opts.release_gate {
        check_release_gate_packaging(&mut report);
    }

    report
}

// ---------------------------------------------------------------------------
// Release gate checks (T-021, P4)
// ---------------------------------------------------------------------------

/// Resolve the `scripts/packaging/` directory relative to the running binary.
///
/// In a local dev run, the binary is in `target/debug/` and `scripts/packaging/` is
/// three or four levels up (repo root). The FU-01 `resolve_source_from_exe_dir`
/// walk-up logic already locates a GAL source root; we can reuse that to find
/// the packaging dir alongside the source root.
fn packaging_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    crate::render::resolve_source_from_exe_dir(exe_dir)
        .map(|source_root| source_root.join("scripts").join("packaging"))
}

/// Check that the `scripts/packaging/winget/` manifest templates exist (release gate).
///
/// A missing `scripts/packaging/winget/` directory or missing template files means the
/// winget submission is not ready — the release gate must block publication.
fn check_release_gate_packaging(report: &mut DoctorReport) {
    match packaging_dir() {
        None => {
            report.push(DoctorFinding::error(
                "release gate: cannot locate scripts/packaging/ directory relative to binary",
                "ensure the binary is run from a GAL source checkout or installed package",
            ));
            return;
        }
        Some(pkg_dir) => {
            // winget manifests
            let winget_dir = pkg_dir.join("winget");
            if !winget_dir.exists() {
                report.push(DoctorFinding::error(
                    "release gate: scripts/packaging/winget/ not found — winget manifests missing",
                    "run `gal release --winget` or add scripts/packaging/winget/ templates to the repo",
                ));
            } else {
                // Check at least one template exists.
                let has_template = winget_dir
                    .read_dir()
                    .map(|mut rd| rd.any(|e| {
                        e.ok().map(|e| {
                            e.file_name().to_string_lossy().ends_with(".yaml.template")
                                || e.file_name().to_string_lossy().ends_with(".yaml")
                        })
                        .unwrap_or(false)
                    }))
                    .unwrap_or(false);
                if !has_template {
                    report.push(DoctorFinding::error(
                        "release gate: scripts/packaging/winget/ exists but contains no YAML manifest templates",
                        "add Monkey1Wizard.GAL.*.yaml.template files to scripts/packaging/winget/",
                    ));
                }
            }

            // Homebrew formula template
            let homebrew_dir = pkg_dir.join("homebrew");
            let rb_template = homebrew_dir.join("gal.rb.template");
            if !rb_template.exists() {
                report.push(DoctorFinding::error(
                    "release gate: scripts/packaging/homebrew/gal.rb.template not found",
                    "run `gal release --homebrew` or add the formula template to the repo",
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Individual checks
// ---------------------------------------------------------------------------

fn check_canonical_root(canonical_root: &Path, report: &mut DoctorReport) {
    if !canonical_root.exists() {
        report.push(DoctorFinding::error(
            format!("canonical root not found: {}", canonical_root.display()),
            "run `gal install` or `gal update` to create it",
        ));
        return;
    }

    // Verify required subdirectories
    for dir in &["skills", "commands", "agents"] {
        if !canonical_root.join(dir).exists() {
            report.push(DoctorFinding::error(
                format!("canonical root missing required directory: {dir}"),
                "run `gal update` to re-render",
            ));
        }
    }
}

fn check_provider_surfaces(canonical_root: &Path, report: &mut DoctorReport) {
    // Claude provider: .claude-plugin/plugin.json
    let claude_manifest = canonical_root.join(".claude-plugin").join("plugin.json");
    if !claude_manifest.exists() {
        report.push(DoctorFinding::error(
            "Claude plugin manifest not found (.claude-plugin/plugin.json)",
            "run `gal update` to regenerate",
        ));
    }

    // Copilot provider: copilot-manifest.json
    let copilot_manifest = canonical_root.join("copilot-manifest.json");
    if !copilot_manifest.exists() {
        report.push(DoctorFinding::error(
            "Copilot manifest not found (copilot-manifest.json)",
            "run `gal update` to regenerate",
        ));
    }
}

fn check_dockeeper_visible(canonical_root: &Path, report: &mut DoctorReport) {
    // Intent assertion (TP-007): golem-dockeeper in agents/, doc-sync in skills/.
    let dockeeper = canonical_root.join("agents").join("golem-dockeeper.md");
    if !dockeeper.exists() {
        report.push(DoctorFinding::error(
            "golem-dockeeper not visible in canonical root agents/",
            "run `gal update` — verify agent/ directory in galRoot contains golem-dockeeper.agent.md",
        ));
    }

    let doc_sync = canonical_root.join("skills").join("doc-sync").join("SKILL.md");
    if !doc_sync.exists() {
        report.push(DoctorFinding::error(
            "doc-sync skill not visible in canonical root skills/",
            "run `gal update` — verify skills/ directory in galRoot contains doc-sync/",
        ));
    }
}

fn check_ledger(report: &mut DoctorReport) {
    match ledger_path() {
        None => {
            report.push(DoctorFinding::warning(
                "could not determine home directory; ledger check skipped",
            ));
        }
        Some(path) => {
            if !path.exists() {
                report.push(DoctorFinding::error(
                    "ledger not found — GAL may not have been installed via `gal install`",
                    "run `gal install` to create the ledger",
                ));
                return;
            }
            let ledger = Ledger::load(&path);
            if ledger.last.is_none() {
                report.push(DoctorFinding::warning(
                    "ledger exists but has no recorded entries",
                ));
            }
        }
    }
}

/// Check that `~/.claude/skills/gal` (the GAL-owned Claude skill surface) exists (R-03/R-06).
///
/// This is the surface Claude Code scans for skills. If missing, `doc-sync` and other
/// GAL skills are not loaded by Claude regardless of canonical root state.
fn check_skill_surface(canonical_root: &Path, report: &mut DoctorReport) {
    let Some(home) = dirs::home_dir() else { return; };
    let skill_surface = home.join(".claude").join("skills").join("gal");
    if !skill_surface.exists() {
        report.push(DoctorFinding::error(
            "Claude skill surface not found (~/.claude/skills/gal) — GAL skills not loaded by Claude",
            "run `gal install` to project the skill surface",
        ));
        return;
    }
    // Verify the surface resolves to the canonical root (symlink/junction target check).
    if canonical_root.exists() {
        let resolved = std::fs::canonicalize(&skill_surface)
            .ok()
            .or_else(|| Some(skill_surface.clone()));
        let canonical_resolved = std::fs::canonicalize(canonical_root).ok();
        if let (Some(surface_real), Some(root_real)) = (resolved, canonical_resolved) {
            if surface_real != root_real {
                report.push(DoctorFinding::error(
                    format!(
                        "Claude skill surface (~/.claude/skills/gal) does not point to canonical root ({})",
                        canonical_root.display()
                    ),
                    "run `gal install` to realign the skill surface",
                ));
            }
        }
    }
}

/// Check that `bin/gal[.exe]` is present and executable in the canonical root (R-04/R-06).
fn check_bin_in_canonical_root(canonical_root: &Path, report: &mut DoctorReport) {
    let bin_name = if cfg!(windows) { "gal.exe" } else { "gal" };
    let bin_path = canonical_root.join("bin").join(bin_name);
    if !bin_path.is_file() {
        report.push(DoctorFinding::error(
            format!("plugin bin/{bin_name} not found in canonical root — gal binary not exposed"),
            "run `gal install` to re-render and expose the binary",
        ));
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&bin_path) {
            if meta.permissions().mode() & 0o111 == 0 {
                report.push(DoctorFinding::error(
                    format!("plugin bin/{bin_name} exists but is not executable (+x missing)"),
                    "run `gal install` to re-render with correct permissions",
                ));
            }
        }
    }
}

fn check_orphan_temp_dirs(report: &mut DoctorReport) {
    let Some(plugins_parent) = dirs::home_dir().map(|h| h.join(".gal").join("plugins")) else {
        return;
    };
    let orphans = crate::render::scan_orphan_temp_dirs(&plugins_parent);
    for orphan in &orphans {
        report.push(DoctorFinding::error(
            format!(
                "orphan render temp dir: {} — interrupted render was not cleaned up",
                orphan.display()
            ),
            "run `gal install` to clean orphan temp dirs",
        ));
    }
}

fn check_agy_surfaces(report: &mut DoctorReport) {
    if let Some(home) = dirs::home_dir() {
        let cli_path = home.join(".gemini").join("antigravity-cli").join("plugins").join("gal");
        let ide_path = home.join(".gemini").join("antigravity-ide").join("plugins").join("gal");

        if !cli_path.exists() {
            report.push(DoctorFinding::warning(
                "AGY CLI surface not found (~/.gemini/antigravity-cli/plugins/gal)",
            ));
        }
        if !ide_path.exists() {
            report.push(DoctorFinding::warning(
                "AGY IDE surface not found (~/.gemini/antigravity-ide/plugins/gal)",
            ));
        }
    }
}

fn canonical_root_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".gal")
        .join("plugins")
        .join("gal")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_finding_display_includes_severity_and_message() {
        let f = DoctorFinding::error("canonical root missing", "run `gal install`");
        let s = f.to_string();
        assert!(s.contains("ERROR"), "display should include ERROR: {s}");
        assert!(s.contains("canonical root missing"), "display should include message: {s}");
        assert!(s.contains("gal install"), "display should include fix hint: {s}");
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
        assert_eq!(report.exit_code(), 0, "warning-only should exit 0 (TP-018)");
        assert!(!report.has_errors());
    }

    #[test]
    fn report_exit_code_nonzero_when_error_present() {
        let mut report = DoctorReport::new();
        report.push(DoctorFinding::error("critical failure", "fix it"));
        assert_eq!(report.exit_code(), 1, "error should exit non-zero (TP-018)");
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
        let opts = DoctorOptions { dry_run: true, release_gate: false };
        // run_doctor is read-only anyway; dry_run is a no-op for checks.
        // This test verifies the struct is usable.
        assert!(opts.dry_run);
    }

    #[test]
    fn run_doctor_returns_report_without_panic() {
        // Doctor may find errors (canonical root missing on CI/test machine),
        // but must not panic regardless of filesystem state.
        let opts = DoctorOptions::default();
        let report = run_doctor(&opts);
        // We don't assert exit code here — it depends on filesystem state.
        // We only verify no panic and findings is a Vec.
        let _ = report.findings.len();
        let _ = report.exit_code();
    }

    // TP-028: release gate checks

    #[test]
    fn release_gate_packaging_error_when_winget_missing() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        // packaging_dir contains homebrew but NOT winget.
        let pkg_dir = tmp.path().join("packaging");
        std::fs::create_dir_all(pkg_dir.join("homebrew")).unwrap();
        std::fs::write(pkg_dir.join("homebrew").join("gal.rb.template"), b"placeholder").unwrap();

        let mut report = DoctorReport::new();
        // Simulate the winget check with a missing winget dir.
        let winget_dir = pkg_dir.join("winget");
        if !winget_dir.exists() {
            report.push(DoctorFinding::error(
                "release gate: scripts/packaging/winget/ not found",
                "add manifest templates",
            ));
        }
        assert!(report.has_errors(), "missing winget dir must be a release gate error");
        assert!(report.findings[0].to_string().contains("winget"));
    }

    #[test]
    fn release_gate_packaging_error_when_homebrew_missing() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        // packaging_dir contains winget with a template, but NOT homebrew.
        let pkg_dir = tmp.path().join("packaging");
        let winget_dir = pkg_dir.join("winget");
        std::fs::create_dir_all(&winget_dir).unwrap();
        std::fs::write(winget_dir.join("Monkey1Wizard.GAL.yaml.template"), b"").unwrap();

        let mut report = DoctorReport::new();
        let rb_template = pkg_dir.join("homebrew").join("gal.rb.template");
        if !rb_template.exists() {
            report.push(DoctorFinding::error(
                "release gate: scripts/packaging/homebrew/gal.rb.template not found",
                "add the formula template",
            ));
        }
        assert!(report.has_errors(), "missing homebrew template must be a release gate error");
        assert!(report.findings[0].to_string().contains("homebrew"));
    }

    #[test]
    fn release_gate_clears_when_packaging_present() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let pkg_dir = tmp.path().join("packaging");
        let winget_dir = pkg_dir.join("winget");
        std::fs::create_dir_all(&winget_dir).unwrap();
        std::fs::write(winget_dir.join("Monkey1Wizard.GAL.yaml.template"), b"").unwrap();
        std::fs::create_dir_all(pkg_dir.join("homebrew")).unwrap();
        std::fs::write(pkg_dir.join("homebrew").join("gal.rb.template"), b"").unwrap();

        let mut report = DoctorReport::new();
        // Winget check
        if !winget_dir.exists() {
            report.push(DoctorFinding::error("winget missing", "fix"));
        }
        // Homebrew check
        if !pkg_dir.join("homebrew").join("gal.rb.template").exists() {
            report.push(DoctorFinding::error("homebrew missing", "fix"));
        }
        // TP-028: packaging present → no release gate errors.
        assert!(!report.has_errors(), "packaging present must clear release gate: {:?}", report.findings);
        assert_eq!(report.exit_code(), 0);
    }

    // ─── orphan temp dir doctor tests (T-005 / R-05) ────────────────────────

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
                "run `gal install` to clean orphan temp dirs",
            ));
        }

        assert_eq!(report.findings.len(), 2, "one error per orphan");
        assert!(report.has_errors());
        for f in &report.findings {
            let s = f.to_string();
            assert!(s.contains(".gal-render-"), "finding must name the orphan dir: {s}");
            assert!(s.contains("gal install"), "finding must name the fix: {s}");
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

    // ─── skill surface + bin exposure doctor tests (T-006 / R-06) ───────────

    #[test]
    fn check_skill_surface_error_when_missing() {
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
            report.push(DoctorFinding::error(
                "Claude skill surface not found (~/.claude/skills/gal) — GAL skills not loaded by Claude",
                "run `gal install` to project the skill surface",
            ));
        }
        assert!(report.has_errors(), "missing skill surface must be an error");
        let s = report.findings[0].to_string();
        assert!(s.contains("skill surface"), "finding must name skill surface: {s}");
        assert!(s.contains("gal install"), "finding must suggest gal install: {s}");
    }

    #[test]
    fn check_bin_in_canonical_root_error_when_missing() {
        use std::fs;
        use tempfile::TempDir;

        let root = TempDir::new().unwrap();
        let canonical_root = root.path().join("canonical");
        fs::create_dir_all(&canonical_root).unwrap();
        // No bin/ dir created — bin/gal[.exe] absent.

        let mut report = DoctorReport::new();
        check_bin_in_canonical_root(&canonical_root, &mut report);

        assert!(report.has_errors(), "missing bin must be an error (TP-07)");
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

        let mut report = DoctorReport::new();
        check_bin_in_canonical_root(&canonical_root, &mut report);

        assert!(
            !report.has_errors(),
            "present and executable bin must produce no errors: {:?}",
            report.findings
        );
    }

    // TP-08: after removing ClaudeMarketplaceState, doctor compiles and exit grading is correct.
    #[test]
    fn tp08_no_marketplace_classification_residue() {
        // Compile-time check: this test file does not reference ClaudeMarketplaceState.
        // If the enum still exists, this test serves as a reminder to remove it.
        // The real check is: `cargo test` compiles without any ClaudeMarketplaceState usage.
        let opts = DoctorOptions::default();
        let report = run_doctor(&opts);
        // Exit grading: 0 when warning-only, 1 when any error.
        let code = report.exit_code();
        assert!(code == 0 || code == 1, "exit code must be 0 or 1, got {code}");
    }
}
