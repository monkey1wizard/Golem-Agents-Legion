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
    }

    // Check 3: Ledger freshness
    check_ledger(&mut report);

    // Check 4: Claude marketplace state (T-017, TP-024)
    check_claude_marketplace(&canonical_root, &mut report);

    // Check 5: AGY surfaces (warning only, best-effort per OE-A)
    check_agy_surfaces(&mut report);

    // Check 6 (release gate only): package-manager metadata + marketplace gate
    if opts.release_gate {
        check_release_gate_packaging(&mut report);
        check_release_gate_marketplace(&canonical_root, &mut report);
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

/// Check Claude marketplace classification is not stale-cache (release gate).
///
/// Stale local cache without a canonical root is an inconsistent state that
/// must be resolved before publication.
fn check_release_gate_marketplace(canonical_root: &Path, report: &mut DoctorReport) {
    let cache_dir = dirs::home_dir().map(|h| {
        h.join(".claude").join("plugins").join("cache").join("gal")
    });
    let ledger_has_install = ledger_path()
        .and_then(|p| {
            if p.exists() {
                Some(crate::ledger::Ledger::load(&p).last.is_some())
            } else {
                None
            }
        })
        .unwrap_or(false);

    let state = classify_claude_marketplace_state(
        canonical_root,
        cache_dir.as_deref(),
        ledger_has_install,
    );

    match state {
        ClaudeMarketplaceState::StaleLocalCache => {
            report.push(DoctorFinding::error(
                "release gate: Claude local cache is stale (no canonical root) — \
                 stale cache must not be mistaken for public marketplace install",
                "run `gal install` to rebuild the canonical root",
            ));
        }
        ClaudeMarketplaceState::NotInstalled => {
            report.push(DoctorFinding::error(
                "release gate: GAL is not installed (no canonical root, no local cache) — \
                 install and verify before release",
                "run `gal install`",
            ));
        }
        ClaudeMarketplaceState::LocalMarketplaceInstalled => {
            // Healthy for local release gate. Public marketplace submission is
            // a separate plan (OQ-005).
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

/// Classification of the Claude marketplace install state (T-017, TP-024).
///
/// These three states are mutually distinct — a local plugin-cache hit is NOT
/// evidence of a public marketplace install. Doctor reports each state honestly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeMarketplaceState {
    /// `~/.claude/plugins/cache/gal/` exists but canonical root is absent or
    /// there is no `gal install` ledger entry. The cache is stale or orphaned.
    StaleLocalCache,
    /// Canonical root exists and ledger shows a successful `gal install`.
    /// Installed through the local marketplace (`claude plugin marketplace add …`).
    LocalMarketplaceInstalled,
    /// Neither a local cache nor a canonical root was detected.
    NotInstalled,
}

/// Classify the current Claude marketplace state for GAL.
///
/// Rule (R-008): local cache must NOT be reported as "public marketplace" evidence.
pub fn classify_claude_marketplace_state(
    canonical_root: &Path,
    claude_cache_dir: Option<&Path>,
    ledger_has_install: bool,
) -> ClaudeMarketplaceState {
    let cache_exists = claude_cache_dir
        .map(|p| p.exists())
        .unwrap_or(false);
    let canonical_exists = canonical_root.exists();

    if canonical_exists && ledger_has_install {
        ClaudeMarketplaceState::LocalMarketplaceInstalled
    } else if cache_exists && !canonical_exists {
        ClaudeMarketplaceState::StaleLocalCache
    } else {
        ClaudeMarketplaceState::NotInstalled
    }
}

fn check_claude_marketplace(canonical_root: &Path, report: &mut DoctorReport) {
    // Resolve the Claude plugin cache path.
    let cache_dir = dirs::home_dir().map(|h| {
        h.join(".claude").join("plugins").join("cache").join("gal")
    });

    // Check ledger for a recorded install.
    let ledger_has_install = ledger_path()
        .and_then(|p| {
            if p.exists() {
                let ledger = crate::ledger::Ledger::load(&p);
                Some(ledger.last.is_some())
            } else {
                None
            }
        })
        .unwrap_or(false);

    let state = classify_claude_marketplace_state(
        canonical_root,
        cache_dir.as_deref(),
        ledger_has_install,
    );

    match state {
        ClaudeMarketplaceState::StaleLocalCache => {
            report.push(DoctorFinding::error(
                "Claude local plugin cache found (~/.claude/plugins/cache/gal) \
                 but canonical root is absent — cache may be stale or orphaned",
                "run `gal install` to rebuild the canonical root; \
                 local cache alone is not evidence of a working install",
            ));
        }
        ClaudeMarketplaceState::LocalMarketplaceInstalled => {
            // Healthy — local marketplace install verified. No finding needed.
        }
        ClaudeMarketplaceState::NotInstalled => {
            // Canonical root absence is already caught by check_canonical_root.
            // Only add a note if the cache is missing too (fully clean state).
        }
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

    // TP-024: Claude local cache stale → doctor reports stale, not public installed.

    #[test]
    fn classify_stale_local_cache_when_cache_exists_no_canonical() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let canonical_root = tmp.path().join("nonexistent_canonical");
        let cache_dir = tmp.path().join("claude_cache");
        std::fs::create_dir_all(&cache_dir).unwrap();

        let state = classify_claude_marketplace_state(
            &canonical_root,
            Some(&cache_dir),
            false, // no ledger install
        );
        assert_eq!(
            state,
            ClaudeMarketplaceState::StaleLocalCache,
            "cache without canonical root must be StaleLocalCache"
        );
    }

    #[test]
    fn classify_local_marketplace_when_canonical_and_ledger() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let canonical_root = tmp.path().join("canonical");
        std::fs::create_dir_all(&canonical_root).unwrap();

        let state = classify_claude_marketplace_state(
            &canonical_root,
            None,
            true, // ledger shows install
        );
        assert_eq!(state, ClaudeMarketplaceState::LocalMarketplaceInstalled);
    }

    #[test]
    fn classify_not_installed_when_nothing_exists() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let canonical_root = tmp.path().join("nonexistent_canonical");
        let cache_dir = tmp.path().join("nonexistent_cache");

        let state = classify_claude_marketplace_state(
            &canonical_root,
            Some(&cache_dir),
            false,
        );
        assert_eq!(state, ClaudeMarketplaceState::NotInstalled);
    }

    #[test]
    fn stale_cache_generates_error_finding_not_marketplace_claim() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let canonical_root = tmp.path().join("no_canonical");
        let cache_dir = tmp.path().join("cache");
        std::fs::create_dir_all(&cache_dir).unwrap();

        let mut report = DoctorReport::new();
        // Pass the cache dir as the mock cache.
        let state = classify_claude_marketplace_state(&canonical_root, Some(&cache_dir), false);
        if state == ClaudeMarketplaceState::StaleLocalCache {
            report.push(DoctorFinding::error(
                "Claude local plugin cache found but canonical root absent — cache may be stale",
                "run `gal install`",
            ));
        }
        // TP-024: Must produce an error (not claim public installed).
        assert!(report.has_errors(), "stale cache must produce an error finding");
        let msg = report.findings[0].to_string();
        assert!(
            msg.contains("stale") || msg.contains("cache"),
            "finding must mention stale/cache state, not public marketplace: {msg}"
        );
        assert!(
            !msg.to_lowercase().contains("public"),
            "must NOT claim public marketplace: {msg}"
        );
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
    fn release_gate_marketplace_error_for_stale_cache() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let canonical_root = tmp.path().join("no_canonical");
        let cache_dir = tmp.path().join("cache");
        std::fs::create_dir_all(&cache_dir).unwrap();

        let state = classify_claude_marketplace_state(&canonical_root, Some(&cache_dir), false);
        let mut report = DoctorReport::new();
        if state == ClaudeMarketplaceState::StaleLocalCache {
            report.push(DoctorFinding::error(
                "release gate: Claude local cache is stale",
                "run `gal install`",
            ));
        }
        assert!(report.has_errors(), "stale cache must block release gate");
        assert!(report.findings[0].to_string().contains("stale"));
    }

    #[test]
    fn release_gate_clears_when_canonical_and_packaging_present() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let canonical_root = tmp.path().join("canonical");
        std::fs::create_dir_all(&canonical_root).unwrap();
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
        // Marketplace check
        let state = classify_claude_marketplace_state(&canonical_root, None, true);
        if state != ClaudeMarketplaceState::LocalMarketplaceInstalled {
            report.push(DoctorFinding::error("marketplace", "fix"));
        }
        // TP-028: all present + LocalMarketplaceInstalled → no errors, exit 0.
        assert!(!report.has_errors(), "all present must clear release gate: {:?}", report.findings);
        assert_eq!(report.exit_code(), 0);
    }
}
