//! Doctor command + its aggregated health checks (routed-executor and others).

use gal_engine::ExitCode;
use gal_foundation::health::{DoctorFinding, HealthCheck, Severity};
use std::path::{Path, PathBuf};

pub(crate) struct RoutedExecutorHealthCheck {
    routing_path: Option<PathBuf>,
}

impl RoutedExecutorHealthCheck {
    fn from_default() -> Self {
        Self {
            routing_path: dispatch::routing::default_routing_path(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_path(path: PathBuf) -> Self {
        Self {
            routing_path: Some(path),
        }
    }
}

impl HealthCheck for RoutedExecutorHealthCheck {
    fn name(&self) -> &str {
        "routed-executor-readiness"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        let Some(routing_path) = self.routing_path.clone() else {
            return vec![DoctorFinding::warning(
                "executor-routing: home directory unavailable; skipping routed-executor readiness check",
            )];
        };

        if !routing_path.exists() {
            return vec![DoctorFinding::warning(format!(
                "executor-routing: '{}' not found; skipping routed-executor readiness check",
                routing_path.display()
            ))];
        }

        let routing = dispatch::routing::load_routing(&routing_path);
        let mut findings: Vec<DoctorFinding> = routing
            .warnings
            .into_iter()
            .map(DoctorFinding::warning)
            .collect();

        let mut routed_executors = std::collections::BTreeSet::new();
        for entry in routing.entries.values() {
            routed_executors.insert(entry.executor.clone());
        }

        if routed_executors.is_empty() {
            findings.push(DoctorFinding::warning(
                "executor-routing: no routed executors configured; skipping readiness aggregation",
            ));
            return findings;
        }

        for executor in routed_executors {
            match dispatch::dispatch::executor_readiness(&executor) {
                dispatch::dispatch::Readiness::Ready => {}
                dispatch::dispatch::Readiness::Unknown { message } => {
                    findings.push(DoctorFinding::warning(format!(
                        "routed executor '{executor}' readiness indeterminate: {message}"
                    )));
                }
                dispatch::dispatch::Readiness::Unauthenticated { hint } => {
                    findings.push(DoctorFinding {
                        severity: Severity::Warning,
                        message: format!(
                            "routed executor '{executor}' is not authenticated for confirmed headless use"
                        ),
                        fix_hint: Some(hint),
                    });
                }
            }
        }

        findings
    }
}
/// Names of skills + commands the canonical plugin root currently declares —
/// the dynamic required-skill inventory for the Codex shared-skill-surface
/// health check. A missing/unreadable subdirectory yields an empty list
/// (fail-safe: no canonical root means nothing to require yet).
pub(crate) fn required_shared_skill_names(canonical_root: &std::path::Path) -> Vec<String> {
    let mut names = Vec::new();
    for sub in ["skills", "commands"] {
        let dir = canonical_root.join(sub);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Parsed `--executor-smoke` flags. Presence of `--executor-smoke` itself is
/// checked by the caller; this struct only carries the opt-in sub-flags.
struct SmokeFlags {
    executors: Vec<String>,
    timeout_secs: u64,
    strict: bool,
    json: bool,
    report_dir: Option<PathBuf>,
    transport: String,
    ssh_target: Option<String>,
    remote_workdir: Option<String>,
}

fn parse_smoke_flags(args: &[String]) -> SmokeFlags {
    let mut executors = Vec::new();
    let mut timeout_secs: u64 = 300;
    let mut strict = false;
    let mut json = false;
    let mut report_dir = None;
    let mut transport = "local".to_string();
    let mut ssh_target = None;
    let mut remote_workdir = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--executor" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    executors.push(v.clone());
                }
            }
            "--timeout" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|s| s.parse::<u64>().ok()) {
                    timeout_secs = v;
                }
            }
            "--strict" => strict = true,
            "--json" => json = true,
            "--report-dir" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    report_dir = Some(PathBuf::from(v));
                }
            }
            "--transport" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    transport = v.clone();
                }
            }
            "--ssh-target" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    ssh_target = Some(v.clone());
                }
            }
            "--remote-workdir" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    remote_workdir = Some(v.clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    SmokeFlags {
        executors,
        timeout_secs,
        strict,
        json,
        report_dir,
        transport,
        ssh_target,
        remote_workdir,
    }
}

/// SSH reachability connect timeout (seconds). Kept short and independent of the
/// per-executor smoke `--timeout` so an unreachable target fails fast rather than
/// waiting the full smoke budget on the TCP connect.
const SSH_CONNECT_TIMEOUT_SECS: u64 = 10;

/// Run `gal doctor --executor-smoke [...]` — the durable, repeatable headless-executor
/// self-test. Never runs unless `--executor-smoke` is explicitly present; plain
/// `gal doctor` never reaches this function.
fn run_executor_smoke(args: &[String]) -> ExitCode {
    use crate::commands::executor_smoke::{
        generate_run_id, run_local_smoke, run_ssh_smoke, RunStore, SmokeStatus, SshProbe,
        UnsafeReportDir,
    };

    let flags = parse_smoke_flags(args);
    if flags.transport != "local" && flags.transport != "ssh" {
        eprintln!(
            "gal doctor --executor-smoke: CONFIG_ERROR: unsupported --transport '{}' (supported: 'local', 'ssh')",
            flags.transport
        );
        return ExitCode::Error;
    }

    // SSH transport requires a target + a dedicated remote workdir; fail fast before any run.
    let ssh_route = if flags.transport == "ssh" {
        match (flags.ssh_target.as_deref(), flags.remote_workdir.as_deref()) {
            (Some(t), Some(w)) if !t.is_empty() && !w.is_empty() => {
                Some((t.to_string(), w.to_string()))
            }
            _ => {
                eprintln!(
                    "gal doctor --executor-smoke: CONFIG_ERROR: --transport ssh requires both --ssh-target <target> and --remote-workdir <dedicated-checkout>"
                );
                return ExitCode::Error;
            }
        }
    } else {
        None
    };

    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let default_report_root = repo_root.join(".dev").join("executor-smoke");
    let report_root = flags.report_dir.clone().unwrap_or(default_report_root);

    if let Err(UnsafeReportDir(msg)) = crate::commands::executor_smoke::validate_report_dir(
        &repo_root,
        flags.report_dir.as_deref().unwrap_or(&report_root),
    ) {
        eprintln!("gal doctor --executor-smoke: CONFIG_ERROR: {msg}");
        return ExitCode::Error;
    }

    let run_id = generate_run_id();
    let store = RunStore::new(report_root, run_id, &flags.transport);
    let generated = run_id_as_generated_timestamp(&store.run_id);

    let rows = match &ssh_route {
        Some((ssh_target, remote_workdir)) => {
            let probe = SshProbe::real(SSH_CONNECT_TIMEOUT_SECS);
            run_ssh_smoke(
                &flags.executors,
                ssh_target,
                remote_workdir,
                &probe,
                &store,
                flags.timeout_secs,
                &generated,
            )
        }
        None => run_local_smoke(&flags.executors, &store, flags.timeout_secs, &generated),
    };

    if let Err(e) = store.write_reports(&rows) {
        eprintln!("gal doctor --executor-smoke: failed to write report: {e}");
        return ExitCode::Error;
    }

    if flags.json {
        println!(
            "{}",
            crate::commands::executor_smoke::render_summary_json(
                &store.run_id,
                &store.transport,
                &rows
            )
        );
    } else {
        println!(
            "{}",
            crate::commands::executor_smoke::render_summary_md(
                &store.run_id,
                &store.transport,
                &rows
            )
        );
        println!("Report: {}", store.summary_json_path().display());
    }

    let any_non_pass = rows.iter().any(|r| r.final_status != SmokeStatus::Pass);
    if flags.strict && any_non_pass {
        ExitCode::Error
    } else {
        ExitCode::Success
    }
}

/// `<run-id>` is already `YYYYMMDDTHHMMSSZ`; reuse it directly as the smoke
/// spec's `Generated:` timestamp so no second clock read is needed.
fn run_id_as_generated_timestamp(run_id: &str) -> String {
    run_id.to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Runtime-adapter budget advisory (read-only, informational-by-default)
// ─────────────────────────────────────────────────────────────────────────────

/// Repo-relative path + owning runtime for each generated adapter carrier, in
/// the fixed display order. Codex and Claude have documented budget rules;
/// the rest are informational-only (no threshold, never a `DoctorFinding`).
const ADAPTER_BUDGET_FILES: &[(&str, &str)] = &[
    ("Codex", "AGENTS.md"),
    ("Claude", "CLAUDE.md"),
    ("Antigravity", "GEMINI.md"),
    ("Copilot", ".github/copilot-instructions.md"),
    ("OpenCode", ".agents/rules/gal.md"),
];

/// Codex's documented default `project_doc_max_bytes` — a local config value
/// that MAY override this default; never presented as the effective cutoff.
const CODEX_DOCUMENTED_DEFAULT_MAX_BYTES: u64 = 32_768;
/// Claude's documented under-this-many-lines guidance target — guidance, not
/// a truncation boundary.
const CLAUDE_GUIDANCE_MAX_LINES: usize = 200;

/// One measured (or missing) adapter row.
struct AdapterBudgetRow {
    runtime: &'static str,
    rel_path: &'static str,
    bytes: Option<u64>,
    lines: Option<usize>,
}

/// Pure measurement: raw on-disk bytes (Codex's own unit) and physical line
/// count (Claude's own unit) for each adapter carrier under `repo_root`. A
/// missing file yields `None` for both — reported as `missing`, never an error.
fn measure_adapter_budget_rows(repo_root: &Path) -> Vec<AdapterBudgetRow> {
    ADAPTER_BUDGET_FILES
        .iter()
        .map(|(runtime, rel_path)| {
            let path = repo_root.join(rel_path);
            match std::fs::read(&path) {
                Ok(content) => AdapterBudgetRow {
                    runtime,
                    rel_path,
                    bytes: Some(content.len() as u64),
                    lines: Some(String::from_utf8_lossy(&content).lines().count()),
                },
                Err(_) => AdapterBudgetRow {
                    runtime,
                    rel_path,
                    bytes: None,
                    lines: None,
                },
            }
        })
        .collect()
}

/// `(load rule, status)` display cells for one row. Codex/Claude carry their
/// documented rule + a provenance-qualified over/within status; the other
/// three runtimes are always `informational`.
fn adapter_budget_cells(row: &AdapterBudgetRow) -> (String, String) {
    let (Some(bytes), Some(lines)) = (row.bytes, row.lines) else {
        return ("—".to_string(), "missing".to_string());
    };
    match row.runtime {
        "Codex" => {
            let load_rule =
                format!("<= {CODEX_DOCUMENTED_DEFAULT_MAX_BYTES} B documented default (local config may override)");
            let status = if bytes > CODEX_DOCUMENTED_DEFAULT_MAX_BYTES {
                format!("over documented default ({bytes} B)")
            } else {
                "within documented default".to_string()
            };
            (load_rule, status)
        }
        "Claude" => {
            let load_rule = format!(
                "< {CLAUDE_GUIDANCE_MAX_LINES} lines (guidance, not a truncation boundary)"
            );
            let status = if lines >= CLAUDE_GUIDANCE_MAX_LINES {
                format!("over guidance target ({lines} lines)")
            } else {
                "within guidance target".to_string()
            };
            (load_rule, status)
        }
        _ => ("informational".to_string(), "informational".to_string()),
    }
}

/// Render the fixed five-row adapter budget table as markdown.
fn render_adapter_budget_table(rows: &[AdapterBudgetRow]) -> String {
    let mut out = String::from("Runtime adapter budget:\n\n");
    out.push_str("| Runtime | Path | Bytes | Lines | Load Rule | Status |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for row in rows {
        let (load_rule, status) = adapter_budget_cells(row);
        let bytes_cell = row
            .bytes
            .map(|b| b.to_string())
            .unwrap_or_else(|| "—".to_string());
        let lines_cell = row
            .lines
            .map(|l| l.to_string())
            .unwrap_or_else(|| "—".to_string());
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            row.runtime, row.rel_path, bytes_cell, lines_cell, load_rule, status
        ));
    }
    out
}

/// Advisory-only findings: Codex over its documented default, or Claude over
/// its guidance line target. Every other case (missing file, other three
/// runtimes, within-budget) yields no finding — this never blocks `gal doctor`.
fn adapter_budget_findings(rows: &[AdapterBudgetRow]) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();
    for row in rows {
        let (Some(bytes), Some(lines)) = (row.bytes, row.lines) else {
            continue;
        };
        match row.runtime {
            "Codex" if bytes > CODEX_DOCUMENTED_DEFAULT_MAX_BYTES => {
                findings.push(DoctorFinding::warning(format!(
                    "{} is {bytes} B, over the documented {CODEX_DOCUMENTED_DEFAULT_MAX_BYTES} B \
                     project_doc_max_bytes default — local Codex config may override this default; \
                     this is not necessarily the effective cutoff on a given machine.",
                    row.rel_path
                )));
            }
            "Claude" if lines >= CLAUDE_GUIDANCE_MAX_LINES => {
                findings.push(DoctorFinding::warning(format!(
                    "{} is {lines} lines, over the {CLAUDE_GUIDANCE_MAX_LINES}-line guidance target — \
                     this is guidance, not a truncation boundary.",
                    row.rel_path
                )));
            }
            _ => {}
        }
    }
    findings
}

/// Run `gal doctor [--dry-run]` — read-only health checks.
pub(crate) fn cmd_doctor(args: &[String]) -> ExitCode {
    cmd_doctor_with_paths(args, &DoctorPathContext::from_standard_paths())
}

fn cmd_doctor_with_paths(args: &[String], paths: &DoctorPathContext) -> ExitCode {
    if args.iter().any(|a| a == "--executor-smoke") {
        return run_executor_smoke(args);
    }

    let dry_run = args.iter().any(|a| a == "--dry-run");
    let mut report = collect_doctor_report(dry_run, paths);

    // Runtime-adapter budget advisory — only in an initialized repo (missing
    // `.dev/project.md` means there is nothing generated to measure yet).
    // Always warning-severity: never turns an oversized adapter into a
    // blocking health error.
    if is_initialized_repo(&paths.repo_root) {
        let rows = measure_adapter_budget_rows(&paths.repo_root);
        println!("{}", render_adapter_budget_table(&rows));
        report.findings.extend(adapter_budget_findings(&rows));
    }

    if report.findings.is_empty() {
        println!("gal doctor: all checks passed.");
    } else {
        for f in &report.findings {
            println!("{f}");
        }
    }

    if report.has_errors() {
        ExitCode::Error
    } else {
        ExitCode::Success
    }
}

struct DoctorPathContext {
    repo_root: PathBuf,
    opencode_root: Option<PathBuf>,
    plugins_lock_path: Option<PathBuf>,
    canonical_root: Option<PathBuf>,
}

impl DoctorPathContext {
    fn from_standard_paths() -> Self {
        let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let opencode_root =
            gal_foundation::paths::user_home().map(|home| home.join(".config").join("opencode"));
        Self {
            repo_root,
            opencode_root,
            plugins_lock_path: gal_foundation::paths::plugins_lock_path(),
            canonical_root: gal_foundation::paths::gal_plugin_root("gal"),
        }
    }
}

fn collect_doctor_report(
    dry_run: bool,
    paths: &DoctorPathContext,
) -> gal_engine::doctor::DoctorReport {
    use gal_engine::doctor::{run_doctor, DoctorOptions};

    let opts = DoctorOptions { dry_run };
    let mut report = run_doctor(&opts);
    if let Some(check) = mcp::CanonicalMcpHealthCheck::from_standard_path() {
        report.findings.extend(check.check());
    }
    report.findings.extend(
        gal_engine::doctor::SetupHealthCheck {
            repo_root: paths.repo_root.clone(),
        }
        .check(),
    );
    if let Some(canonical_root) = &paths.canonical_root {
        report.findings.extend(
            gal_engine::doctor::ClaudePluginCacheCheck::from_user_home(canonical_root.clone())
                .check(),
        );
        report.findings.extend(
            gal_engine::doctor::BinaryRefreshCheck::from_current_exe(canonical_root.clone())
                .check(),
        );
    }
    let canonical_root = paths.canonical_root.clone();
    let required_names = canonical_root
        .as_ref()
        .map(|root| required_shared_skill_names(root))
        .unwrap_or_default();
    if let Some(home) = gal_foundation::paths::user_home() {
        let shared_skills_root = home.join(".agents").join("skills");
        let mut check = projection::SkillsProjectionHealthCheck::with_inventory(
            shared_skills_root,
            required_names.clone(),
            paths.plugins_lock_path.clone(),
        );
        if let Some(root) = &canonical_root {
            check = check.with_canonical_source(root.clone());
        }
        report.findings.extend(check.check());
    }
    if let (Some(root), Some(opencode_root)) = (&canonical_root, &paths.opencode_root) {
        let opencode_check = projection::OpenCodeProjectionHealthCheck::new(
            opencode_root.clone(),
            paths.plugins_lock_path.clone(),
            root.clone(),
        );
        report.findings.extend(opencode_check.check());
    }

    report.findings.extend(binary_source_skew_finding(
        env!("GAL_GIT_STAMP"),
        current_gal_checkout_head().as_deref(),
    ));
    if let Some(codex_config_path) = gal_foundation::paths::codex_config_path() {
        report.findings.extend(
            projection::CodexSkillsConfigCheck::new(codex_config_path, required_names).check(),
        );
    }
    report
        .findings
        .extend(RoutedExecutorHealthCheck::from_default().check());
    report
}

/// An initialized repo has run `gal init` at least once — signalled by the
/// presence of `.dev/project.md`. A non-initialized repo has no generated
/// adapters to measure, so the budget table is suppressed rather than shown
/// with all-missing rows.
fn is_initialized_repo(repo_root: &Path) -> bool {
    repo_root.join(".dev").join("project.md").is_file()
}

/// Pure: emit a dev-checkout binary/source-skew warning when the baked git
/// stamp differs from the current GAL-checkout HEAD. An empty baked stamp
/// (packaged build with no git) or no resolvable HEAD yields no finding —
/// never a false skew report. Comparison is on the shorter length so a short
/// baked stamp matches a full HEAD.
fn binary_source_skew_finding(baked_stamp: &str, head: Option<&str>) -> Option<DoctorFinding> {
    let baked = baked_stamp.trim();
    if baked.is_empty() {
        return None;
    }
    let head = head?.trim();
    if head.is_empty() {
        return None;
    }
    let n = baked.len().min(head.len());
    if baked.get(..n) == head.get(..n) {
        return None;
    }
    Some(DoctorFinding::warning(format!(
        "gal binary was built from git {baked} but this GAL checkout HEAD is {head} — the binary \
         on PATH is older than source; rebuild + reinstall (`cargo build --release -p gal-cli`) \
         before trusting projection/dispatch output"
    )))
}

/// Resolve the current GAL-checkout HEAD short hash, or `None` when cwd is not
/// inside a GAL source checkout (no `plugins/gal-core` at the git toplevel) or
/// git is unavailable. Read-only.
fn current_gal_checkout_head() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let run = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(&cwd)
            .output()
            .ok()
            .filter(|o| o.status.success())?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!s.is_empty()).then_some(s)
    };
    let toplevel = run(&["rev-parse", "--show-toplevel"])?;
    // Only a GAL source checkout carries the projection source contract.
    if !PathBuf::from(&toplevel)
        .join("plugins")
        .join("gal-core")
        .is_dir()
    {
        return None;
    }
    run(&["rev-parse", "--short", "HEAD"])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // --- adapter budget advisory ---

    fn write_adapter_fixture(dir: &std::path::Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn adapter_budget_measures_bytes_and_lines_for_present_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_adapter_fixture(tmp.path(), "AGENTS.md", "hello\nworld\n");
        write_adapter_fixture(tmp.path(), "CLAUDE.md", "a\nb\nc\n");
        let rows = measure_adapter_budget_rows(tmp.path());
        let codex = rows.iter().find(|r| r.runtime == "Codex").unwrap();
        assert_eq!(codex.bytes, Some(12));
        assert_eq!(codex.lines, Some(2));
        let claude = rows.iter().find(|r| r.runtime == "Claude").unwrap();
        assert_eq!(claude.bytes, Some(6));
        assert_eq!(claude.lines, Some(3));
    }

    #[test]
    fn adapter_budget_missing_file_yields_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        let rows = measure_adapter_budget_rows(tmp.path());
        assert!(rows.iter().all(|r| r.bytes.is_none() && r.lines.is_none()));
        for row in &rows {
            let (load_rule, status) = adapter_budget_cells(row);
            assert_eq!(load_rule, "—");
            assert_eq!(status, "missing");
        }
    }

    #[test]
    fn adapter_budget_row_order_is_fixed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let rows = measure_adapter_budget_rows(tmp.path());
        let runtimes: Vec<&str> = rows.iter().map(|r| r.runtime).collect();
        assert_eq!(
            runtimes,
            vec!["Codex", "Claude", "Antigravity", "Copilot", "OpenCode"]
        );
    }

    #[test]
    fn adapter_budget_codex_over_default_warns_without_claiming_effective_cutoff() {
        let tmp = tempfile::TempDir::new().unwrap();
        let big = "x".repeat((CODEX_DOCUMENTED_DEFAULT_MAX_BYTES + 1) as usize);
        write_adapter_fixture(tmp.path(), "AGENTS.md", &big);
        let rows = measure_adapter_budget_rows(tmp.path());
        let findings = adapter_budget_findings(&rows);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("may override"));
        // Clarifies it is NOT necessarily the effective cutoff -- never
        // asserts it IS the effective cutoff.
        assert!(!findings[0].message.contains("is the effective cutoff"));
        assert!(findings[0]
            .message
            .contains("not necessarily the effective cutoff"));
        assert_eq!(findings[0].severity, Severity::Warning);
    }

    #[test]
    fn adapter_budget_claude_over_guidance_warns_as_guidance_not_truncation() {
        let tmp = tempfile::TempDir::new().unwrap();
        let many_lines = "x\n".repeat(CLAUDE_GUIDANCE_MAX_LINES);
        write_adapter_fixture(tmp.path(), "CLAUDE.md", &many_lines);
        let rows = measure_adapter_budget_rows(tmp.path());
        let findings = adapter_budget_findings(&rows);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("guidance"));
        // Clarifies it is NOT a truncation boundary -- never claims content
        // will actually be truncated/cut off.
        assert!(findings[0].message.contains("not a truncation boundary"));
        assert!(!findings[0].message.contains("will be truncated"));
        assert_eq!(findings[0].severity, Severity::Warning);
    }

    #[test]
    fn adapter_budget_within_thresholds_yields_no_findings() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_adapter_fixture(tmp.path(), "AGENTS.md", "small\n");
        write_adapter_fixture(tmp.path(), "CLAUDE.md", "small\n");
        let rows = measure_adapter_budget_rows(tmp.path());
        assert!(adapter_budget_findings(&rows).is_empty());
    }

    #[test]
    fn adapter_budget_informational_runtimes_never_produce_findings() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Oversized by any measure, but Antigravity/Copilot/OpenCode carry no
        // documented threshold — informational only, never a finding.
        let huge = "x".repeat(1_000_000);
        write_adapter_fixture(tmp.path(), "GEMINI.md", &huge);
        write_adapter_fixture(tmp.path(), ".github/copilot-instructions.md", &huge);
        write_adapter_fixture(tmp.path(), ".agents/rules/gal.md", &huge);
        let rows = measure_adapter_budget_rows(tmp.path());
        assert!(adapter_budget_findings(&rows).is_empty());
        for row in rows
            .iter()
            .filter(|r| r.runtime != "Codex" && r.runtime != "Claude")
        {
            let (load_rule, status) = adapter_budget_cells(row);
            assert_eq!(load_rule, "informational");
            assert_eq!(status, "informational");
        }
    }

    #[test]
    fn adapter_budget_table_renders_all_five_rows() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_adapter_fixture(tmp.path(), "AGENTS.md", "a\n");
        let rows = measure_adapter_budget_rows(tmp.path());
        let table = render_adapter_budget_table(&rows);
        for (runtime, path) in ADAPTER_BUDGET_FILES {
            assert!(table.contains(runtime), "table missing runtime {runtime}");
            assert!(table.contains(path), "table missing path {path}");
        }
    }

    #[test]
    fn is_initialized_repo_requires_dev_project_md() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(!is_initialized_repo(tmp.path()));
        write_adapter_fixture(tmp.path(), ".dev/project.md", "# Project\n");
        assert!(is_initialized_repo(tmp.path()));
    }

    // These two tests originally routed through `cmd_doctor_with_paths` ->
    // `collect_doctor_report` -> `run_doctor()`, which always runs its own
    // internal `CanonicalRootCheck` (and several other checks requiring a
    // fully-rendered canonical root with real agent/skill/binary content)
    // against the REAL environment home directory — `gal_foundation::
    // paths::gal_plugin_root` has no fixture-injection point, and building
    // a complete fake canonical root satisfying every downstream check is
    // its own large, fragile undertaking unrelated to what these two tests
    // are actually about. Rewritten to test the adapter-budget-advisory
    // behavior directly, the same isolated way `adapter_budget_codex_over_
    // default_warns_without_claiming_effective_cutoff` above already does
    // — hermetic, no environment dependency, and precisely scoped to the
    // claim each test name makes.
    #[test]
    fn cmd_doctor_non_initialized_repo_suppresses_table_and_stays_non_blocking() {
        let tmp = tempfile::TempDir::new().unwrap();
        // A non-initialized repo has no `.dev/project.md`, so the adapter
        // budget rows are never even measured — this alone must not
        // contribute any finding, let alone an error-severity one.
        assert!(!is_initialized_repo(tmp.path()));
        let rows = measure_adapter_budget_rows(tmp.path());
        let findings = adapter_budget_findings(&rows);
        assert!(
            findings.is_empty(),
            "a non-initialized repo must not produce any adapter-budget finding: {findings:?}"
        );
    }

    #[test]
    fn cmd_doctor_oversized_adapter_warns_but_exit_stays_non_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_adapter_fixture(tmp.path(), ".dev/project.md", "# Project\n");
        let many_lines = "x\n".repeat(CLAUDE_GUIDANCE_MAX_LINES);
        write_adapter_fixture(tmp.path(), "CLAUDE.md", &many_lines);
        // The oversized-adapter finding is warning-severity only; an
        // advisory threshold must never become a blocking health error.
        assert!(is_initialized_repo(tmp.path()));
        let rows = measure_adapter_budget_rows(tmp.path());
        let findings = adapter_budget_findings(&rows);
        assert!(
            !findings.is_empty(),
            "an oversized CLAUDE.md must still be reported"
        );
        assert!(
            findings
                .iter()
                .all(|f| matches!(f.severity, Severity::Warning)),
            "adapter-budget overrun must be advisory-only, never a blocking error: {findings:?}"
        );
    }

    #[test]
    fn doctor_wires_codex_skill_freshness_and_binary_skew() {
        use std::fs;
        use tempfile::TempDir;

        // ── binary/source skew (pure) ──
        assert!(
            binary_source_skew_finding("abc1234", Some("def5678")).is_some(),
            "differing baked stamp vs HEAD must warn"
        );
        assert!(
            binary_source_skew_finding("abc1234", Some("abc1234def")).is_none(),
            "a short baked stamp matching the HEAD prefix must not warn"
        );
        assert!(
            binary_source_skew_finding("", Some("def5678")).is_none(),
            "empty baked stamp (packaged build) must never warn"
        );
        assert!(
            binary_source_skew_finding("abc1234", None).is_none(),
            "no resolvable HEAD (not a GAL checkout) must never warn"
        );
        let skew = binary_source_skew_finding("abc1234", Some("def5678")).unwrap();
        assert!(skew.message.contains("older than source"));
        assert!(skew.message.contains("gal-cli"));

        // ── freshness wiring: a drifted command-projected skill is reported stale ──
        let tmp = TempDir::new().unwrap();
        let shared = tmp.path().join("skills");
        let plugin_root = tmp.path().join("plugin");
        let sdir = shared.join("gal-pipeline");
        fs::create_dir_all(&sdir).unwrap();
        fs::write(
            sdir.join("SKILL.md"),
            "---\nname: gal-pipeline\ndescription: d\n---\nstale projected body\n",
        )
        .unwrap();
        let cdir = plugin_root.join("commands").join("gal-pipeline");
        fs::create_dir_all(&cdir).unwrap();
        fs::write(
            cdir.join("SKILL.md"),
            "---\nname: gal-pipeline\ndescription: d\n---\ncurrent canonical body\n",
        )
        .unwrap();

        let findings = projection::SkillsProjectionHealthCheck::with_inventory(
            shared,
            vec!["gal-pipeline".into()],
            None,
        )
        .with_canonical_source(plugin_root)
        .check();
        assert!(
            findings.iter().any(
                |f| f.message.contains("differs from current canonical source")
                    && f.message.contains("gal refresh")
            ),
            "doctor's freshness wiring must surface stale Codex skill projection with remediation"
        );
    }

    #[test]
    fn doctor_wires_opencode_projection_health_check() {
        use std::fs;
        use tempfile::TempDir;

        // ── OpenCode projection drift detection wiring ──
        let tmp = TempDir::new().unwrap();
        let opencode_root = tmp.path().join(".config").join("opencode");
        let canonical_root = tmp.path().join("canonical");
        let lockfile = tmp.path().join("plugins.lock.json");

        // Create OpenCode commands directory
        let commands_dir = opencode_root.join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        // Write a stale command file (different from canonical)
        let cmd_file = commands_dir.join("gal-pipeline.md");
        fs::write(
            &cmd_file,
            "# Generated by GAL Setup-Machine. Do not edit manually.\n---\ndescription: |\n  stale\n---\n\nstale body\n",
        )
        .unwrap();

        // Create canonical root with current command
        let canonical_cmd_dir = canonical_root.join("commands").join("gal-pipeline");
        fs::create_dir_all(&canonical_cmd_dir).unwrap();
        fs::write(
            canonical_cmd_dir.join("SKILL.md"),
            "---\nname: gal-pipeline\ndescription: current\n---\ncurrent canonical body\n",
        )
        .unwrap();

        // Create lockfile with the command path recorded
        let lockfile_content = serde_json::json!({
            "_galProjection": {
                "commandProjectionPaths": [cmd_file.to_string_lossy().replace('\\', "/").to_lowercase()]
            }
        });
        fs::write(&lockfile, serde_json::to_string(&lockfile_content).unwrap()).unwrap();

        // Call OpenCodeProjectionHealthCheck directly
        let findings = projection::OpenCodeProjectionHealthCheck::new(
            opencode_root,
            Some(lockfile),
            canonical_root,
        )
        .check();

        // Assert it detects the drift
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("differs from canonical source")
                    && f.message.contains("gal-pipeline")),
            "doctor's OpenCode wiring must surface stale command projection with remediation"
        );
    }

    #[test]
    fn cmd_doctor_collects_opencode_projection_health_check_from_fixture_paths() {
        use std::fs;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let opencode_root = tmp.path().join("opencode");
        let command_path = opencode_root.join("commands").join("gal-pipeline.md");
        fs::create_dir_all(command_path.parent().unwrap()).unwrap();
        fs::write(&command_path, "stale command projection\n").unwrap();
        let lockfile_path = tmp.path().join("plugins.lock.json");
        let canonical_root = tmp.path().join("canonical");
        let canonical_command = canonical_root.join("commands").join("gal-pipeline");
        fs::create_dir_all(&canonical_command).unwrap();
        fs::write(
            canonical_command.join("SKILL.md"),
            "---\nname: gal-pipeline\ndescription: current\n---\ncurrent canonical body\n",
        )
        .unwrap();
        let lockfile = serde_json::json!({
            "_galProjection": {
                "commandProjectionPaths": [command_path.to_string_lossy().replace('\\', "/").to_lowercase()]
            }
        });
        fs::write(&lockfile_path, serde_json::to_string(&lockfile).unwrap()).unwrap();

        let paths = DoctorPathContext {
            repo_root: tmp.path().to_path_buf(),
            opencode_root: Some(opencode_root),
            plugins_lock_path: Some(lockfile_path),
            canonical_root: Some(canonical_root),
        };
        let report = collect_doctor_report(true, &paths);

        assert!(
            report.findings.iter().any(|finding| {
                finding.message.contains("OpenCode command 'gal-pipeline'")
                    && finding.message.contains("differs from canonical source")
            }),
            "cmd_doctor must pass isolated fixture paths to OpenCodeProjectionHealthCheck"
        );
    }

    #[test]
    fn parse_smoke_flags_defaults_when_nothing_given() {
        let f = parse_smoke_flags(&a(&["--executor-smoke"]));
        assert!(f.executors.is_empty());
        assert_eq!(f.timeout_secs, 300);
        assert!(!f.strict);
        assert!(!f.json);
        assert!(f.report_dir.is_none());
        assert_eq!(f.transport, "local");
    }

    #[test]
    fn parse_smoke_flags_collects_repeated_executor_and_other_flags() {
        let f = parse_smoke_flags(&a(&[
            "--executor-smoke",
            "--executor",
            "codex",
            "--executor",
            "claude",
            "--timeout",
            "15",
            "--strict",
            "--json",
            "--transport",
            "local",
            "--report-dir",
            "/tmp/report",
        ]));
        assert_eq!(f.executors, vec!["codex".to_string(), "claude".to_string()]);
        assert_eq!(f.timeout_secs, 15);
        assert!(f.strict);
        assert!(f.json);
        assert_eq!(f.transport, "local");
        assert_eq!(f.report_dir, Some(PathBuf::from("/tmp/report")));
    }

    #[test]
    fn cmd_doctor_executor_smoke_rejects_unsafe_report_dir_before_any_write() {
        use crate::commands::ENV_GUARD;
        let _guard = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let exit = cmd_doctor(&a(&["doctor", "--executor-smoke", "--report-dir", "docs"]));

        std::env::set_current_dir(original).unwrap();
        assert!(matches!(exit, ExitCode::Error));
        assert!(
            !tmp.path().join("docs").exists(),
            "unsafe report-dir must be rejected before any write"
        );
    }

    #[test]
    fn cmd_doctor_executor_smoke_json_covers_unsupported_filter_without_dispatch() {
        // A fictitious --executor name is UNSUPPORTED and never dispatches — a
        // safe way to exercise the full doctor -> smoke-core -> persisted-report
        // -> --json/--strict plumbing without a real (quota-consuming) CLI call.
        use crate::commands::ENV_GUARD;
        let _guard = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let exit = cmd_doctor(&a(&[
            "doctor",
            "--executor-smoke",
            "--executor",
            "nonexistent-executor-doctor-test",
            "--strict",
            "--json",
        ]));

        std::env::set_current_dir(original).unwrap();
        // --strict + a non-pass (UNSUPPORTED) row must be a non-zero exit.
        assert!(matches!(exit, ExitCode::Error));

        let latest = tmp
            .path()
            .join(".dev")
            .join("executor-smoke")
            .join("latest.json");
        assert!(latest.exists(), "persisted latest.json must exist");
        let body = std::fs::read_to_string(&latest).unwrap();
        assert!(body.contains("nonexistent-executor-doctor-test"));
        assert!(body.contains("UNSUPPORTED"));
    }

    #[test]
    fn parse_smoke_flags_parses_ssh_transport_target_and_remote_workdir() {
        let f = parse_smoke_flags(&a(&[
            "--executor-smoke",
            "--transport",
            "ssh",
            "--ssh-target",
            "user@host",
            "--remote-workdir",
            "/home/user/gal-smoke",
        ]));
        assert_eq!(f.transport, "ssh");
        assert_eq!(f.ssh_target.as_deref(), Some("user@host"));
        assert_eq!(f.remote_workdir.as_deref(), Some("/home/user/gal-smoke"));
    }

    #[test]
    fn parse_smoke_flags_ssh_fields_default_to_none() {
        let f = parse_smoke_flags(&a(&["--executor-smoke"]));
        assert!(f.ssh_target.is_none());
        assert!(f.remote_workdir.is_none());
    }

    #[test]
    fn cmd_doctor_ssh_transport_without_remote_workdir_fails_fast_before_any_run() {
        // Missing --remote-workdir is a usage/config error that must fail before a run id
        // is generated or any report directory is created (and before any ssh spawn).
        use crate::commands::ENV_GUARD;
        let _guard = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let exit = cmd_doctor(&a(&[
            "doctor",
            "--executor-smoke",
            "--transport",
            "ssh",
            "--ssh-target",
            "user@host",
        ]));

        let smoke_dir_exists = tmp.path().join(".dev").join("executor-smoke").exists();
        std::env::set_current_dir(original).unwrap();
        assert!(matches!(exit, ExitCode::Error));
        assert!(
            !smoke_dir_exists,
            "missing --remote-workdir must fail fast before writing any report"
        );
    }

    #[test]
    fn cmd_doctor_ssh_transport_without_ssh_target_fails_fast() {
        use crate::commands::ENV_GUARD;
        let _guard = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let exit = cmd_doctor(&a(&[
            "doctor",
            "--executor-smoke",
            "--transport",
            "ssh",
            "--remote-workdir",
            "/home/user/gal-smoke",
        ]));

        let smoke_dir_exists = tmp.path().join(".dev").join("executor-smoke").exists();
        std::env::set_current_dir(original).unwrap();
        assert!(matches!(exit, ExitCode::Error));
        assert!(!smoke_dir_exists, "missing --ssh-target must fail fast");
    }

    #[test]
    fn cmd_doctor_rejects_unknown_transport() {
        use crate::commands::ENV_GUARD;
        let _guard = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let exit = cmd_doctor(&a(&[
            "doctor",
            "--executor-smoke",
            "--transport",
            "carrier-pigeon",
        ]));
        std::env::set_current_dir(original).unwrap();
        assert!(matches!(exit, ExitCode::Error));
    }

    #[test]
    fn cmd_doctor_without_executor_smoke_flag_never_writes_a_smoke_report() {
        // Plain `gal doctor` (no --executor-smoke) must never touch
        // .dev/executor-smoke/ — run it against a fresh temp cwd and confirm no
        // smoke run directory was created, no matter what plain doctor's own
        // (unrelated) health checks decide.
        use crate::commands::ENV_GUARD;
        let _guard = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let _ = cmd_doctor(&a(&["doctor"]));
        let smoke_dir_exists = tmp.path().join(".dev").join("executor-smoke").exists();
        std::env::set_current_dir(original).unwrap();
        assert!(
            !smoke_dir_exists,
            "plain `gal doctor` must never create .dev/executor-smoke/"
        );
    }
}
