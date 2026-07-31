//! `gal doctor --executor-smoke` — durable, repeatable self-test of the headless
//! coding-agent executors (`codex`, `claude`, `copilot`, `agy`, `opencode`).
//!
//! Drives the SAME `dispatch::run::run_dispatch` code path a real pipeline-phase
//! dispatch uses (routing, readiness gate, adapter invocation, spawn, terminal-state
//! classification, receipt verification, executor-log write, session-id extraction)
//! with a pipeline-shaped minimal task spec — never a parallel spawn-only probe.

use dispatch::dispatch::{Readiness, TerminalState};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The five coding-agent CLIs this self-test covers by default.
pub const AGENT_MATRIX: &[&str] = &["codex", "claude", "copilot", "agy", "opencode"];

/// Top-level directory names that must never receive smoke-run output — durable
/// docs, generated adapters, source/plugin trees, and other tracked repo-root
/// surfaces. A `--report-dir` resolving under (or equal to) any of these, or to
/// the repo root itself, is rejected with `CONFIG_ERROR` before any write.
const REPORT_DIR_DENYLIST_DIRS: &[&str] = &[
    "docs", "plugins", "crates", ".github", ".claude", ".agents", "src",
];
const REPORT_DIR_DENYLIST_FILES: &[&str] = &[
    "README.md",
    "CLAUDE.md",
    "AGENTS.md",
    "GEMINI.md",
    "GEMINI.toml",
];

/// Error returned when a `--report-dir` target is unsafe. Smoke-side, pre-dispatch
/// — maps to `SmokeStatus::ConfigError`, never attempted a dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsafeReportDir(pub String);

impl std::fmt::Display for UnsafeReportDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "'{}' is not a safe report directory (durable docs, generated adapters, tracked \
             source, and the repo root are never used for smoke evidence)",
            self.0
        )
    }
}

/// Validate a `--report-dir` target against `repo_root` *before any write*.
/// Rejects the repo root itself and any path under a durable/tracked surface.
/// A target outside the repo root entirely (e.g. a temp dir in tests) is accepted.
pub fn validate_report_dir(repo_root: &Path, target: &Path) -> Result<(), UnsafeReportDir> {
    let resolved = if target.is_absolute() {
        target.to_path_buf()
    } else {
        repo_root.join(target)
    };
    let normalized = lexical_normalize(&resolved);
    let root_normalized = lexical_normalize(repo_root);

    if normalized == root_normalized {
        return Err(UnsafeReportDir(resolved.display().to_string()));
    }

    if let Ok(rel) = normalized.strip_prefix(&root_normalized) {
        let mut components = rel.components();
        if let Some(first) = components.next() {
            let first_str = first.as_os_str().to_string_lossy();
            if rel.components().count() == 1
                && REPORT_DIR_DENYLIST_FILES
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(&first_str))
            {
                return Err(UnsafeReportDir(resolved.display().to_string()));
            }
            if REPORT_DIR_DENYLIST_DIRS
                .iter()
                .any(|d| d.eq_ignore_ascii_case(&first_str))
            {
                return Err(UnsafeReportDir(resolved.display().to_string()));
            }
        }
    }

    Ok(())
}

/// Lexical (non-canonicalizing) path normalization: collapses `.`/`..` components
/// without touching the filesystem, so validation works even for paths that do
/// not exist yet (a fresh report-dir has no reason to exist before this check).
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Generate a UTC run id: `YYYYMMDDTHHMMSSZ`, second-resolution, filesystem-safe.
pub fn generate_run_id() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_utc_from_epoch_secs(secs)
}

/// Pure epoch-seconds -> `YYYYMMDDTHHMMSSZ` formatter (no chrono dependency).
fn format_utc_from_epoch_secs(secs: u64) -> String {
    // Civil-from-days algorithm (Howard Hinnant's public-domain days_from_civil
    // inverse) — avoids pulling in a datetime crate for a filesystem-safe run id.
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z")
}

/// The run store layout for one smoke invocation: `<report_root>/runs/<run-id>/<transport>/`.
pub struct RunStore {
    pub report_root: PathBuf,
    pub run_id: String,
    pub transport: String,
}

impl RunStore {
    pub fn new(report_root: PathBuf, run_id: String, transport: &str) -> Self {
        Self {
            report_root,
            run_id,
            transport: transport.to_string(),
        }
    }

    pub fn run_dir(&self) -> PathBuf {
        self.report_root
            .join("runs")
            .join(&self.run_id)
            .join(&self.transport)
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.run_dir().join("logs")
    }

    pub fn receipts_dir(&self) -> PathBuf {
        self.run_dir().join("receipts")
    }

    pub fn summary_json_path(&self) -> PathBuf {
        self.run_dir().join("summary.json")
    }

    pub fn summary_md_path(&self) -> PathBuf {
        self.run_dir().join("summary.md")
    }

    pub fn latest_json_path(&self) -> PathBuf {
        self.report_root.join("latest.json")
    }

    /// Create the run directory tree (`local/logs`, `local/receipts`). Idempotent.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.logs_dir())?;
        std::fs::create_dir_all(self.receipts_dir())?;
        Ok(())
    }

    /// Render + write `summary.json`, `summary.md`, and refresh `latest.json`.
    pub fn write_reports(&self, rows: &[ReportRow]) -> std::io::Result<()> {
        self.ensure_dirs()?;
        let json = render_summary_json(&self.run_id, &self.transport, rows);
        std::fs::write(self.summary_json_path(), json.as_bytes())?;
        let md = render_summary_md(&self.run_id, &self.transport, rows);
        std::fs::write(self.summary_md_path(), md.as_bytes())?;
        std::fs::write(self.latest_json_path(), json.as_bytes())?;
        Ok(())
    }
}

/// Render the machine-readable `summary.json` body.
pub fn render_summary_json(run_id: &str, transport: &str, rows: &[ReportRow]) -> String {
    let row_values: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "executor": r.executor,
                "authPreflight": r.auth_preflight,
                "finalStatus": r.final_status.as_str(),
                "logPath": r.log_path,
                "receiptPath": r.receipt_path,
                "sessionId": r.session_id,
                "nextActionHint": r.next_action_hint,
            })
        })
        .collect();
    let value = serde_json::json!({
        "runId": run_id,
        "transport": transport,
        "rows": row_values,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

/// Assemble the pipeline-shaped smoke task spec for one agent, reusing the
/// real `pipeline::task_spec::assemble_task_spec` machinery (real-path
/// fidelity — never a hand-rolled spec string). Phase is fixed to `"audit"` so
/// the mandatory-receipt write-back clause is always present regardless of
/// which real pipeline role the agent under test is routed to.
pub fn assemble_smoke_spec(executor: &str, receipt_path: &str, generated: &str) -> String {
    let prompt_body = format!(
        "## Tasks\n\n\
         - [ ] T-SMOKE — Executor smoke self-test ({executor})\n\
         \x20 - Files: `{receipt_path}`\n\
         \x20 - Change: write exactly one line `SMOKE_PASS` to the receipt file listed above \
         to confirm this headless dispatch path works end-to-end. Do not modify any other file.\n"
    );
    let input = pipeline::task_spec::TaskSpecInput {
        task_scope: "T-SMOKE",
        phase: "audit",
        prompt_path: "(executor-smoke self-test — no prompt file)",
        prompt_body: &prompt_body,
        generated,
        git_branch: "n/a",
        git_head: "n/a",
        convention_hints: None,
        receipt_path: Some(receipt_path),
        agent_contract_body: None,
    };
    pipeline::task_spec::assemble_task_spec(&input)
        .map(|spec| spec.markdown)
        .unwrap_or_else(|e| format!("# Executor smoke spec assembly failed: {e}"))
}

/// Smoke-only default model per executor, used solely to synthesize the
/// self-test's injected routing JSON. Deliberately independent of the user's
/// real `config.json#executorRouting` (the plan's real-path-fidelity
/// requirement is about the *dispatch code path*, not the model choice) — a
/// stale default here surfaces as a visible `CALL_FAILED`/log entry, never a
/// silent skip. Kept in sync manually; see `docs/manual.md`.
fn smoke_default_model(executor: &str) -> &'static str {
    match executor {
        "claude" => "claude-sonnet-4-6",
        "codex" => "gpt-5.4-mini",
        "copilot" => "auto",
        "agy" => "gemini-3.5-flash",
        // Provider `nvidia` + model `nvidia/nemotron-3-ultra-550b-a55b` (verified
        // present in the installed OpenCode CLI's `opencode models`). The prior
        // single-prefix id was stale and not a valid model.
        "opencode" => "nvidia/nvidia/nemotron-3-ultra-550b-a55b",
        _ => "auto",
    }
}

/// The synthetic routing role used for every smoke dispatch. Fixed to
/// `"AUDITOR"` (`Phase::Audit`) purely so `assemble_smoke_spec`'s mandatory
/// receipt clause is always present — unrelated to which real pipeline role
/// the agent under test is routed to in the user's own config.
const SMOKE_PHASE: &str = "audit";
const SMOKE_ROLE_KEY: &str = "AUDITOR";

/// Write a synthesized single-executor routing JSON (`executorRouting` subtree,
/// injected via `DispatchArgs.routing_path` — never the user's real
/// `config.json#executorRouting`) into `path`.
fn write_synthetic_routing(path: &Path, executor: &str) -> std::io::Result<()> {
    let model = smoke_default_model(executor);
    let value = serde_json::json!({
        "executorRouting": {
            "executors": { executor: model },
            "pipeline": { SMOKE_ROLE_KEY: { "executor": executor } },
        }
    });
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&value).unwrap())
}

/// Drive one agent's smoke attempt through the real `dispatch::run::run_dispatch`
/// code path: synthesize a single-executor routing file, assemble the
/// pipeline-shaped spec via [`assemble_smoke_spec`], and dispatch with a
/// run-scoped log dir + receipt path + bounded timeout. Real-path fidelity —
/// no parallel spawn-only probe.
pub fn smoke_one_agent(
    executor: &str,
    store: &RunStore,
    timeout_secs: u64,
    generated: &str,
) -> ReportRow {
    // Smoke-side, pre-dispatch: an `--executor` filter naming a CLI outside the
    // supported five-agent matrix is a configuration mistake, not an install/auth
    // condition — never attempt a dispatch for it.
    if !AGENT_MATRIX.contains(&executor) {
        let status = SmokeStatus::Unsupported;
        return ReportRow {
            executor: executor.to_string(),
            auth_preflight: "n/a".to_string(),
            final_status: status,
            log_path: None,
            receipt_path: None,
            session_id: None,
            next_action_hint: Some(format!(
                "'{executor}' is not one of the supported agents: {}",
                AGENT_MATRIX.join(", ")
            )),
        };
    }

    let available = dispatch::dispatch::is_available(executor);
    let readiness = dispatch::dispatch::executor_readiness(executor);
    let auth_preflight = match &readiness {
        _ if !available => "not-installed",
        Readiness::Ready => "ready",
        Readiness::Unauthenticated { .. } => "unauthenticated",
        Readiness::Unknown { .. } => "unknown",
    };

    if !available {
        let status = SmokeStatus::NotInstalled;
        return ReportRow {
            executor: executor.to_string(),
            auth_preflight: auth_preflight.to_string(),
            final_status: status,
            log_path: None,
            receipt_path: None,
            session_id: None,
            next_action_hint: next_action_hint(status, None),
        };
    }

    if let Readiness::Unauthenticated { hint } = &readiness {
        let status = SmokeStatus::NotAuthenticated;
        return ReportRow {
            executor: executor.to_string(),
            auth_preflight: auth_preflight.to_string(),
            final_status: status,
            log_path: None,
            receipt_path: None,
            session_id: None,
            next_action_hint: next_action_hint(status, Some(hint)),
        };
    }

    let run_dir = store.run_dir();
    let routing_path = run_dir.join(format!("{executor}-routing.json"));
    let receipt_path = store.receipts_dir().join(format!("{executor}.receipt.md"));
    let log_dir = store.logs_dir();

    if let Err(e) = write_synthetic_routing(&routing_path, executor) {
        let status = SmokeStatus::ConfigError;
        return ReportRow {
            executor: executor.to_string(),
            auth_preflight: auth_preflight.to_string(),
            final_status: status,
            log_path: None,
            receipt_path: None,
            session_id: None,
            next_action_hint: Some(format!("failed to write synthesized routing file: {e}")),
        };
    }

    let spec = assemble_smoke_spec(executor, &receipt_path.display().to_string(), generated);

    let args = dispatch::cli::DispatchArgs {
        phase: dispatch::stage::Phase::from_str(SMOKE_PHASE)
            .unwrap_or(dispatch::stage::Phase::Audit),
        task: "T-SMOKE".to_string(),
        workdir: store.report_root.clone(),
        timeout_secs,
        routing_path: Some(routing_path),
        receipt_path: Some(receipt_path.clone()),
        log_dir_override: Some(log_dir),
        stdout_quiet: true,
        contract_provenance: None,
    };

    let outcome = dispatch::run::run_dispatch(&args, &spec);
    let receipt_ok = dispatch::dispatch::verify_receipt(&receipt_path);
    let status = classify_status(
        available,
        &readiness,
        outcome.terminal_state.as_ref(),
        receipt_ok,
    );
    let hint = match &readiness {
        Readiness::Unauthenticated { hint } => Some(hint.as_str()),
        _ => None,
    };

    let log_path = outcome.log_path.map(|p| p.display().to_string());

    // On NO_RECEIPT, distinguish a context overflow from a plain exit-0-no-write-back
    // by scanning the preserved executor log, and surface it in the summary hint.
    let mut next_hint = next_action_hint(status, hint);
    if status == SmokeStatus::NoReceipt {
        if let Some(note) = no_receipt_cause_note(log_path.as_deref()) {
            next_hint = Some(match next_hint {
                Some(h) => format!("{h} [{note}]"),
                None => note,
            });
        }
    }

    ReportRow {
        executor: executor.to_string(),
        auth_preflight: auth_preflight.to_string(),
        final_status: status,
        log_path,
        receipt_path: Some(receipt_path.display().to_string()),
        session_id: outcome.session_id,
        next_action_hint: next_hint,
    }
}

/// Run the local-transport smoke matrix for `agents` (falls back to
/// [`AGENT_MATRIX`] when empty), writing every artifact under `store`.
pub fn run_local_smoke(
    agents: &[String],
    store: &RunStore,
    timeout_secs: u64,
    generated: &str,
) -> Vec<ReportRow> {
    let selected: Vec<String> = if agents.is_empty() {
        AGENT_MATRIX.iter().map(|s| s.to_string()).collect()
    } else {
        agents.to_vec()
    };
    selected
        .iter()
        .map(|agent| smoke_one_agent(agent, store, timeout_secs, generated))
        .collect()
}

// ── SSH transport adapter ────────────────────────────────────────────────────
//
// The SSH self-test reuses the SAME shared core (matrix, spec assembly, run store,
// schema, renderer, classification statuses) and drives the SAME real
// `dispatch::run::run_dispatch` remote branch a user-issued pipeline SSH dispatch
// uses — guard, unsupported-CliFlag rejection, receipt fetch, and terminal-state
// sync are the real dispatch code, never re-implemented here. The genuinely-new
// SSH-adapter work is: a non-interactive reachability precheck and remote
// install/auth probes evaluated ON the remote machine (the real remote branch only
// gates on `ssh` availability + the HEAD/clean guard, never remote executor state).

/// Remote authentication preflight for one executor, evaluated ON the remote
/// machine over SSH (never inferred from the local control node). No universal
/// cheap remote auth signal exists, so this is best-effort: a clear "not logged in"
/// marker → `Unauthenticated`; otherwise `Unknown` and the bounded smoke run is the
/// authoritative auth test (recorded as `AUTH_UNKNOWN` preflight).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteAuth {
    Unauthenticated,
    Unknown,
}

impl RemoteAuth {
    fn preflight_str(self) -> &'static str {
        match self {
            RemoteAuth::Unauthenticated => "unauthenticated",
            RemoteAuth::Unknown => "unknown",
        }
    }
}

/// Pure parse of a remote auth-probe's combined stdout+stderr into a [`RemoteAuth`].
/// Scans for common "not authenticated / please log in" markers; absent a clear
/// marker the result is `Unknown` (the bounded smoke run is the real auth test).
pub fn parse_remote_auth(output: &str) -> RemoteAuth {
    const UNAUTH_MARKERS: &[&str] = &[
        "not logged in",
        "not authenticated",
        "please log in",
        "please login",
        "authentication required",
        "login required",
        "not signed in",
        "no api key",
        "unauthorized",
        "run `login`",
        "run login",
    ];
    let lower = output.to_ascii_lowercase();
    if UNAUTH_MARKERS.iter().any(|m| lower.contains(m)) {
        RemoteAuth::Unauthenticated
    } else {
        RemoteAuth::Unknown
    }
}

/// How the SSH smoke adapter runs its own remote prechecks (reachability, remote
/// install probe, remote auth probe). Injectable so fixtures drive the real
/// probe+parse code with canned remote output and no network. `run_dispatch`'s
/// HEAD/clean guard and receipt-fetch use their own hardcoded `ssh` (the real
/// remote branch), exercised in live acceptance rather than through this seam.
pub struct SshProbe {
    program: String,
    base_args: Vec<String>,
}

impl SshProbe {
    /// Production probe: `ssh -o BatchMode=yes -o ConnectTimeout=<n> <target> <remote-cmd>`.
    pub fn real(connect_timeout_secs: u64) -> Self {
        Self {
            program: "ssh".to_string(),
            base_args: vec![
                "-o".to_string(),
                "BatchMode=yes".to_string(),
                "-o".to_string(),
                format!("ConnectTimeout={connect_timeout_secs}"),
            ],
        }
    }

    /// Test seam: run `<program> <base_args...> <target> <remote-cmd>` — e.g. `sh`
    /// pointed at a fake-ssh fixture script that inspects the remote command and
    /// emits canned output.
    #[cfg(test)]
    pub fn with_program(program: &str, base_args: Vec<String>) -> Self {
        Self {
            program: program.to_string(),
            base_args,
        }
    }

    fn run(&self, target: &str, remote_cmd: &str) -> std::io::Result<std::process::Output> {
        std::process::Command::new(&self.program)
            .args(&self.base_args)
            .arg(target)
            .arg(remote_cmd)
            .output()
    }

    /// Non-interactive reachability: `ssh <target> true`. Reachable iff exit 0.
    pub fn reachable(&self, target: &str) -> bool {
        self.run(target, "true")
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Remote install probe: `command -v <executor>` on the remote *non-interactive*
    /// PATH (the same shell the dispatch uses), never the local control node's PATH.
    pub fn remote_available(&self, target: &str, executor: &str) -> bool {
        let cmd = format!("command -v {}", dispatch::run::shell_quote(executor));
        self.run(target, &cmd)
            .map(|o| o.status.success() && !o.stdout.is_empty())
            .unwrap_or(false)
    }

    /// Remote auth probe: a cheap non-interactive `<executor> --version` call whose
    /// combined output is scanned for an unauthenticated marker. Best-effort — see
    /// [`RemoteAuth`]. Failure to run at all → `Unknown` (defer to the bounded smoke).
    pub fn remote_auth(&self, target: &str, executor: &str) -> RemoteAuth {
        let cmd = format!("{} --version", dispatch::run::shell_quote(executor));
        match self.run(target, &cmd) {
            Ok(o) => {
                let combined = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                parse_remote_auth(&combined)
            }
            Err(_) => RemoteAuth::Unknown,
        }
    }
}

/// Dispatch-derived evidence the SSH classifier consumes, taken from the real
/// `run_dispatch` remote branch's [`dispatch::run::DispatchOutcome`].
pub struct SshDispatchEvidence<'a> {
    pub reason: Option<&'a str>,
    pub terminal: Option<&'a TerminalState>,
    pub receipt_ok: bool,
}

/// Pure classification of one agent's SSH smoke outcome. Reachability + remote
/// install/auth are decided smoke-side before any dispatch; the remaining cases come
/// from the real remote branch via [`SshDispatchEvidence`] (`reason` + `terminal`).
/// Never claims `PASS` without a fetched, non-empty receipt.
pub fn classify_ssh_status(
    reachable: bool,
    remote_available: bool,
    remote_auth: RemoteAuth,
    dispatched: Option<&SshDispatchEvidence>,
) -> SmokeStatus {
    if !reachable {
        return SmokeStatus::SshUnreachable;
    }
    if !remote_available {
        return SmokeStatus::NotInstalled;
    }
    if remote_auth == RemoteAuth::Unauthenticated {
        return SmokeStatus::NotAuthenticated;
    }
    match dispatched {
        // ready/unknown auth but no bounded smoke attempted → preflight-only signal.
        None => SmokeStatus::AuthUnknown,
        Some(ev) => classify_ssh_dispatch(ev),
    }
}

/// Map the real remote-branch dispatch evidence to a status. Remote post-processing
/// / degrade reasons take priority over the terminal state, so an SSH-specific
/// failure (guard, unsupported CliFlag, receipt-fetch failure) is never collapsed
/// into a generic executor failure.
fn classify_ssh_dispatch(ev: &SshDispatchEvidence) -> SmokeStatus {
    match ev.reason {
        Some("remote-cliflag-unsupported") => return SmokeStatus::Unsupported,
        Some("remote-guard-failed") => return SmokeStatus::RemoteGuardFailed,
        Some("remote-receipt-fetch-failed") => return SmokeStatus::RemoteFetchFailed,
        Some("remote-missing-workdir") | Some("remote-absolute-receipt-unsupported") => {
            return SmokeStatus::ConfigError
        }
        // ssh went missing between the reachability precheck and dispatch.
        Some("executor-unavailable") => return SmokeStatus::SshUnreachable,
        _ => {}
    }
    match ev.terminal {
        Some(TerminalState::Completed) => {
            if ev.receipt_ok {
                SmokeStatus::Pass
            } else {
                SmokeStatus::NoReceipt
            }
        }
        Some(TerminalState::NoReceipt) => SmokeStatus::NoReceipt,
        Some(TerminalState::Timeout)
        | Some(TerminalState::TimeoutNoOutput)
        | Some(TerminalState::TimeoutMidrun) => SmokeStatus::Timeout,
        Some(TerminalState::DisconnectedPartial) | Some(TerminalState::Unavailable) | None => {
            SmokeStatus::CallFailed
        }
    }
}

/// Write a synthesized single-executor *remote* routing JSON (`executorRouting`
/// subtree with `sshTarget` + `remoteWorkdir`, injected via `DispatchArgs.routing_path`
/// — never the user's real `config.json#executorRouting`) into `path`.
fn write_synthetic_ssh_routing(
    path: &Path,
    executor: &str,
    ssh_target: &str,
    remote_workdir: &str,
) -> std::io::Result<()> {
    let model = smoke_default_model(executor);
    let value = serde_json::json!({
        "executorRouting": {
            "executors": { executor: model },
            "pipeline": {
                SMOKE_ROLE_KEY: {
                    "executor": executor,
                    "sshTarget": ssh_target,
                    "remoteWorkdir": remote_workdir,
                },
            },
        }
    });
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&value).unwrap())
}

/// A row that never reached a bounded dispatch (short-circuit precondition).
fn ssh_precondition_row(executor: &str, auth_preflight: &str, status: SmokeStatus) -> ReportRow {
    ReportRow {
        executor: executor.to_string(),
        auth_preflight: auth_preflight.to_string(),
        final_status: status,
        log_path: None,
        receipt_path: None,
        session_id: None,
        next_action_hint: next_action_hint(status, None),
    }
}

/// Drive one agent's SSH smoke attempt: precheck reachability/install/auth ON the
/// remote machine, then (when ready/unknown) synthesize a remote route and dispatch
/// through the real `run_dispatch` remote branch. `reachable` is hoisted once per run.
#[allow(clippy::too_many_arguments)]
pub fn ssh_one_agent(
    executor: &str,
    ssh_target: &str,
    remote_workdir: &str,
    probe: &SshProbe,
    reachable: bool,
    store: &RunStore,
    timeout_secs: u64,
    generated: &str,
) -> ReportRow {
    if !AGENT_MATRIX.contains(&executor) {
        return ReportRow {
            executor: executor.to_string(),
            auth_preflight: "n/a".to_string(),
            final_status: SmokeStatus::Unsupported,
            log_path: None,
            receipt_path: None,
            session_id: None,
            next_action_hint: Some(format!(
                "'{executor}' is not one of the supported agents: {}",
                AGENT_MATRIX.join(", ")
            )),
        };
    }

    if !reachable {
        return ssh_precondition_row(executor, "n/a", SmokeStatus::SshUnreachable);
    }

    // Remote install + auth are evaluated ON the remote machine — the real remote
    // branch never probes them (it only gates on `ssh` + the HEAD/clean guard).
    if !probe.remote_available(ssh_target, executor) {
        return ssh_precondition_row(executor, "not-installed", SmokeStatus::NotInstalled);
    }
    let auth = probe.remote_auth(ssh_target, executor);
    if auth == RemoteAuth::Unauthenticated {
        return ssh_precondition_row(
            executor,
            auth.preflight_str(),
            SmokeStatus::NotAuthenticated,
        );
    }

    let run_dir = store.run_dir();
    let routing_path = run_dir.join(format!("{executor}-ssh-routing.json"));
    let receipt_path = store.receipts_dir().join(format!("{executor}.receipt.md"));
    let log_dir = store.logs_dir();

    if let Err(e) = write_synthetic_ssh_routing(&routing_path, executor, ssh_target, remote_workdir)
    {
        return ReportRow {
            executor: executor.to_string(),
            auth_preflight: auth.preflight_str().to_string(),
            final_status: SmokeStatus::ConfigError,
            log_path: None,
            receipt_path: None,
            session_id: None,
            next_action_hint: Some(format!("failed to write synthesized SSH routing file: {e}")),
        };
    }

    let spec = assemble_smoke_spec(executor, &receipt_path.display().to_string(), generated);
    let args = dispatch::cli::DispatchArgs {
        phase: dispatch::stage::Phase::from_str(SMOKE_PHASE)
            .unwrap_or(dispatch::stage::Phase::Audit),
        task: "T-SMOKE".to_string(),
        workdir: store.report_root.clone(),
        timeout_secs,
        routing_path: Some(routing_path),
        receipt_path: Some(receipt_path.clone()),
        log_dir_override: Some(log_dir),
        stdout_quiet: true,
        contract_provenance: None,
    };

    let outcome = dispatch::run::run_dispatch(&args, &spec);
    let receipt_ok = dispatch::dispatch::verify_receipt(&receipt_path);
    let ev = SshDispatchEvidence {
        reason: outcome.reason.as_deref(),
        terminal: outcome.terminal_state.as_ref(),
        receipt_ok,
    };
    let status = classify_ssh_status(true, true, auth, Some(&ev));

    ReportRow {
        executor: executor.to_string(),
        auth_preflight: auth.preflight_str().to_string(),
        final_status: status,
        log_path: outcome.log_path.map(|p| p.display().to_string()),
        receipt_path: Some(receipt_path.display().to_string()),
        session_id: outcome.session_id,
        next_action_hint: next_action_hint(status, None),
    }
}

/// Run the SSH-transport smoke matrix for `agents` (falls back to [`AGENT_MATRIX`]
/// when empty), writing every artifact under `store` (`transport=ssh`). Reachability
/// is probed once and shared across rows — an unreachable target yields
/// `SSH_UNREACHABLE` for every requested executor with no per-agent probe.
#[allow(clippy::too_many_arguments)]
pub fn run_ssh_smoke(
    agents: &[String],
    ssh_target: &str,
    remote_workdir: &str,
    probe: &SshProbe,
    store: &RunStore,
    timeout_secs: u64,
    generated: &str,
) -> Vec<ReportRow> {
    let selected: Vec<String> = if agents.is_empty() {
        AGENT_MATRIX.iter().map(|s| s.to_string()).collect()
    } else {
        agents.to_vec()
    };
    let reachable = probe.reachable(ssh_target);
    selected
        .iter()
        .map(|agent| {
            ssh_one_agent(
                agent,
                ssh_target,
                remote_workdir,
                probe,
                reachable,
                store,
                timeout_secs,
                generated,
            )
        })
        .collect()
}

/// Render the human-readable `summary.md` / concise-table body.
pub fn render_summary_md(run_id: &str, transport: &str, rows: &[ReportRow]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Executor Smoke — {run_id} ({transport})\n\n"));
    out.push_str("| Executor | Auth Preflight | Status | Next Action |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for row in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            row.executor,
            row.auth_preflight,
            row.final_status.as_str(),
            row.next_action_hint.as_deref().unwrap_or("-"),
        ));
    }
    out
}

/// Classification for one agent's smoke-test outcome.
///
/// `CONFIG_ERROR` and `UNSUPPORTED` are smoke-side, pre-dispatch conditions
/// (unsafe `--report-dir`, an unrecognized `--executor` filter) and sit outside
/// the dispatch-derived Status Mapping below.
///
/// The last three variants are SSH-transport-only extensions. They never
/// occur on the local transport and are additive — they do not change the shape
/// or `as_str()` rendering of any local status:
/// - `SshUnreachable` — the control node cannot open a non-interactive SSH
///   session to the remote target (checked before any executor probe).
/// - `RemoteGuardFailed` — the remote workdir is missing, unsafe, dirty, or not
///   at the control node's HEAD (the real remote-dispatch pre-run guard).
/// - `RemoteFetchFailed` — the remote process may have run but fetching its
///   receipt back to the control node failed (distinct from a genuinely
///   missing/empty remote receipt).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeStatus {
    Pass,
    NotInstalled,
    NotAuthenticated,
    AuthUnknown,
    Unsupported,
    ConfigError,
    CallFailed,
    NoReceipt,
    Timeout,
    SshUnreachable,
    RemoteGuardFailed,
    RemoteFetchFailed,
}

impl SmokeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::NotInstalled => "NOT_INSTALLED",
            Self::NotAuthenticated => "NOT_AUTHENTICATED",
            Self::AuthUnknown => "AUTH_UNKNOWN",
            Self::Unsupported => "UNSUPPORTED",
            Self::ConfigError => "CONFIG_ERROR",
            Self::CallFailed => "CALL_FAILED",
            Self::NoReceipt => "NO_RECEIPT",
            Self::Timeout => "TIMEOUT",
            Self::SshUnreachable => "SSH_UNREACHABLE",
            Self::RemoteGuardFailed => "REMOTE_GUARD_FAILED",
            Self::RemoteFetchFailed => "REMOTE_FETCH_FAILED",
        }
    }
}

impl std::fmt::Display for SmokeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One agent's row in the smoke report.
#[derive(Debug, Clone)]
pub struct ReportRow {
    pub executor: String,
    /// `"ready"` | `"unauthenticated"` | `"unknown"` | `"not-installed"` — the
    /// preflight reading recorded before any smoke attempt.
    pub auth_preflight: String,
    pub final_status: SmokeStatus,
    pub log_path: Option<String>,
    pub receipt_path: Option<String>,
    pub session_id: Option<String>,
    pub next_action_hint: Option<String>,
}

/// Pure classification: maps `is_available` + readiness preflight + (optional)
/// dispatch terminal state to a `SmokeStatus`, per the Status Mapping table.
///
/// - `is_available` false -> `NOT_INSTALLED`
/// - `Readiness::Unauthenticated` -> `NOT_AUTHENTICATED` (never dispatched)
/// - `Readiness::Unknown` with no terminal state yet -> `AUTH_UNKNOWN` (preflight only)
/// - `Readiness::Unknown` (or `Ready`) with a terminal state -> the smoke outcome
///   (`TerminalState::Completed` + valid receipt -> `PASS`, `NoReceipt` -> `NO_RECEIPT`,
///   `Timeout`/`TimeoutNoOutput`/`TimeoutMidrun` -> `TIMEOUT`,
///   `DisconnectedPartial`/`Unavailable` -> `CALL_FAILED`)
pub fn classify_status(
    is_available: bool,
    readiness: &Readiness,
    terminal: Option<&TerminalState>,
    receipt_ok: bool,
) -> SmokeStatus {
    if !is_available {
        return SmokeStatus::NotInstalled;
    }
    if matches!(readiness, Readiness::Unauthenticated { .. }) {
        return SmokeStatus::NotAuthenticated;
    }

    match terminal {
        None => SmokeStatus::AuthUnknown,
        Some(TerminalState::Completed) => {
            if receipt_ok {
                SmokeStatus::Pass
            } else {
                SmokeStatus::NoReceipt
            }
        }
        Some(TerminalState::NoReceipt) => SmokeStatus::NoReceipt,
        Some(TerminalState::Timeout)
        | Some(TerminalState::TimeoutNoOutput)
        | Some(TerminalState::TimeoutMidrun) => SmokeStatus::Timeout,
        Some(TerminalState::DisconnectedPartial) | Some(TerminalState::Unavailable) => {
            SmokeStatus::CallFailed
        }
    }
}

/// Sub-cause of a `NO_RECEIPT` outcome, distinguished from the raw executor log
/// where practical so a Copilot static-context overflow is not confused with a
/// plain exit-0-no-write-back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoReceiptCause {
    /// The log carries a context-length / token-limit overflow marker — the
    /// executor's prompt was too large before it could write the receipt.
    ContextOverflow,
    /// Exit 0 but the receipt file was never written, with no overflow marker.
    ExitZeroNoWriteBack,
}

impl NoReceiptCause {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ContextOverflow => "context-overflow",
            Self::ExitZeroNoWriteBack => "exit-0-no-write-back",
        }
    }
}

/// Pure classification of a `NO_RECEIPT` sub-cause from the raw executor-log text.
/// Scans (case-insensitively) for context-length / token-limit overflow markers;
/// finding one → `ContextOverflow`, otherwise → `ExitZeroNoWriteBack` (the plain
/// exit-0 case that `NO_RECEIPT` already means). Best-effort — the raw log is always
/// preserved on disk regardless.
pub fn classify_no_receipt_cause(log_text: &str) -> NoReceiptCause {
    const OVERFLOW_MARKERS: &[&str] = &[
        "context length",
        "context window",
        "context_length_exceeded",
        "maximum context",
        "token limit",
        "too many tokens",
        "prompt is too long",
        "input is too long",
        "exceeds the maximum",
    ];
    let lower = log_text.to_ascii_lowercase();
    if OVERFLOW_MARKERS.iter().any(|m| lower.contains(m)) {
        NoReceiptCause::ContextOverflow
    } else {
        NoReceiptCause::ExitZeroNoWriteBack
    }
}

/// Read the executor log (if present) and produce a short `NO_RECEIPT` cause note
/// for the smoke summary. Returns `None` when the log path is absent/unreadable —
/// the raw log on disk stays the authoritative evidence either way.
fn no_receipt_cause_note(log_path: Option<&str>) -> Option<String> {
    let text = std::fs::read_to_string(log_path?).ok()?;
    Some(format!(
        "no-receipt cause: {}",
        classify_no_receipt_cause(&text).as_str()
    ))
}

/// Next-action hint text for a final status, reusing the same remediation
/// language `executor_readiness`'s `Unauthenticated` hint carries where applicable.
pub fn next_action_hint(status: SmokeStatus, readiness_hint: Option<&str>) -> Option<String> {
    match status {
        SmokeStatus::NotInstalled => Some("install the CLI and ensure it is on PATH".to_string()),
        SmokeStatus::NotAuthenticated => readiness_hint.map(str::to_string),
        SmokeStatus::AuthUnknown => {
            Some("readiness could not be confirmed; smoke attempted a bounded call".to_string())
        }
        SmokeStatus::CallFailed => Some("inspect the executor log for the failure".to_string()),
        SmokeStatus::NoReceipt => {
            Some("executor exited 0 but never wrote the receipt file".to_string())
        }
        SmokeStatus::Timeout => Some(
            "executor did not finish within the bounded timeout; rerun with --timeout <secs>"
                .to_string(),
        ),
        SmokeStatus::SshUnreachable => Some(
            "control node could not open a non-interactive SSH session to the target; check \
             --ssh-target, network, and key-based auth (BatchMode)"
                .to_string(),
        ),
        SmokeStatus::RemoteGuardFailed => Some(
            "remote workdir is missing, unsafe, dirty, or not at the control node's HEAD; \
             re-sync the dedicated remote checkout before retrying"
                .to_string(),
        ),
        SmokeStatus::RemoteFetchFailed => Some(
            "remote run may have executed but its receipt could not be fetched back over SSH; \
             inspect the executor log and remote receipt path"
                .to_string(),
        ),
        SmokeStatus::ConfigError | SmokeStatus::Unsupported | SmokeStatus::Pass => None,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn unauth() -> Readiness {
        Readiness::Unauthenticated {
            hint: "log in".to_string(),
        }
    }
    fn unknown() -> Readiness {
        Readiness::Unknown {
            message: "indeterminate".to_string(),
        }
    }

    #[test]
    fn not_installed_when_unavailable() {
        assert_eq!(
            classify_status(false, &Readiness::Ready, None, false),
            SmokeStatus::NotInstalled
        );
    }

    #[test]
    fn not_authenticated_never_dispatched() {
        assert_eq!(
            classify_status(true, &unauth(), None, false),
            SmokeStatus::NotAuthenticated
        );
    }

    #[test]
    fn auth_unknown_preflight_only() {
        assert_eq!(
            classify_status(true, &unknown(), None, false),
            SmokeStatus::AuthUnknown
        );
    }

    #[test]
    fn completed_with_receipt_is_pass() {
        assert_eq!(
            classify_status(
                true,
                &Readiness::Ready,
                Some(&TerminalState::Completed),
                true
            ),
            SmokeStatus::Pass
        );
    }

    #[test]
    fn completed_without_receipt_is_no_receipt() {
        assert_eq!(
            classify_status(
                true,
                &Readiness::Ready,
                Some(&TerminalState::Completed),
                false
            ),
            SmokeStatus::NoReceipt
        );
    }

    #[test]
    fn no_receipt_terminal_state_is_no_receipt() {
        assert_eq!(
            classify_status(
                true,
                &Readiness::Ready,
                Some(&TerminalState::NoReceipt),
                false
            ),
            SmokeStatus::NoReceipt
        );
    }

    #[test]
    fn all_three_timeout_variants_map_to_timeout() {
        for state in [
            TerminalState::Timeout,
            TerminalState::TimeoutNoOutput,
            TerminalState::TimeoutMidrun,
        ] {
            assert_eq!(
                classify_status(true, &Readiness::Ready, Some(&state), false),
                SmokeStatus::Timeout,
                "state {state:?} should map to TIMEOUT"
            );
        }
    }

    #[test]
    fn disconnected_partial_is_call_failed() {
        assert_eq!(
            classify_status(
                true,
                &Readiness::Ready,
                Some(&TerminalState::DisconnectedPartial),
                false
            ),
            SmokeStatus::CallFailed
        );
    }

    #[test]
    fn auth_unknown_with_terminal_state_uses_smoke_outcome() {
        // Readiness::Unknown records AUTH_UNKNOWN at preflight, but once a bounded
        // smoke attempt produced a terminal state, the final status is that outcome.
        assert_eq!(
            classify_status(true, &unknown(), Some(&TerminalState::Completed), true),
            SmokeStatus::Pass
        );
    }

    // ── run store + report-dir safety ───────────────────────────────────────

    fn sample_rows() -> Vec<ReportRow> {
        vec![ReportRow {
            executor: "codex".to_string(),
            auth_preflight: "ready".to_string(),
            final_status: SmokeStatus::Pass,
            log_path: Some("/tmp/log".to_string()),
            receipt_path: Some("/tmp/receipt".to_string()),
            session_id: Some("abc".to_string()),
            next_action_hint: None,
        }]
    }

    #[test]
    fn run_store_writes_all_artifacts_under_temp_report_root() {
        let tmp = TempDir::new().unwrap();
        let store = RunStore::new(
            tmp.path().to_path_buf(),
            "20260709T000000Z".to_string(),
            "local",
        );
        store.write_reports(&sample_rows()).unwrap();

        assert!(store.summary_json_path().exists(), "summary.json missing");
        assert!(store.summary_md_path().exists(), "summary.md missing");
        assert!(store.latest_json_path().exists(), "latest.json missing");
        assert!(store.logs_dir().is_dir(), "logs dir missing");
        assert!(store.receipts_dir().is_dir(), "receipts dir missing");

        let json = std::fs::read_to_string(store.summary_json_path()).unwrap();
        assert!(json.contains("\"codex\""));
        assert!(json.contains("PASS"));
    }

    #[test]
    fn generate_run_id_is_filesystem_safe_and_stable_format() {
        let id = generate_run_id();
        assert!(id.ends_with('Z'));
        assert!(id.contains('T'));
        assert!(!id.contains(':'), "run id must be filesystem-safe");
    }

    #[test]
    fn format_utc_known_epoch_matches_expected() {
        // 2021-01-01T00:00:00Z = 1609459200
        assert_eq!(format_utc_from_epoch_secs(1609459200), "20210101T000000Z");
    }

    // ── SSH status extensions ───────────────────────────────────────────────

    #[test]
    fn ssh_status_extensions_render_expected_strings() {
        assert_eq!(SmokeStatus::SshUnreachable.as_str(), "SSH_UNREACHABLE");
        assert_eq!(
            SmokeStatus::RemoteGuardFailed.as_str(),
            "REMOTE_GUARD_FAILED"
        );
        assert_eq!(
            SmokeStatus::RemoteFetchFailed.as_str(),
            "REMOTE_FETCH_FAILED"
        );
    }

    #[test]
    fn ssh_status_extensions_do_not_change_local_status_rendering() {
        // The additive SSH variants must not perturb any pre-existing local status string.
        assert_eq!(SmokeStatus::Pass.as_str(), "PASS");
        assert_eq!(SmokeStatus::NotInstalled.as_str(), "NOT_INSTALLED");
        assert_eq!(SmokeStatus::NotAuthenticated.as_str(), "NOT_AUTHENTICATED");
        assert_eq!(SmokeStatus::AuthUnknown.as_str(), "AUTH_UNKNOWN");
        assert_eq!(SmokeStatus::Unsupported.as_str(), "UNSUPPORTED");
        assert_eq!(SmokeStatus::ConfigError.as_str(), "CONFIG_ERROR");
        assert_eq!(SmokeStatus::CallFailed.as_str(), "CALL_FAILED");
        assert_eq!(SmokeStatus::NoReceipt.as_str(), "NO_RECEIPT");
        assert_eq!(SmokeStatus::Timeout.as_str(), "TIMEOUT");
    }

    #[test]
    fn ssh_status_extensions_carry_remediation_hints() {
        for status in [
            SmokeStatus::SshUnreachable,
            SmokeStatus::RemoteGuardFailed,
            SmokeStatus::RemoteFetchFailed,
        ] {
            assert!(
                next_action_hint(status, None).is_some(),
                "SSH status {status:?} must carry a remediation hint"
            );
        }
    }

    #[test]
    fn ssh_status_extensions_render_in_summary_json_and_md_without_reshaping() {
        // A report row carrying an SSH-only status renders through the SAME renderers as
        // local rows — proving the extensions do not change the report shape.
        let rows = vec![ReportRow {
            executor: "copilot".to_string(),
            auth_preflight: "unknown".to_string(),
            final_status: SmokeStatus::RemoteFetchFailed,
            log_path: Some("/run/logs/x.log".to_string()),
            receipt_path: Some("/run/receipts/copilot.receipt.md".to_string()),
            session_id: None,
            next_action_hint: Some("fetch failed".to_string()),
        }];
        let json = render_summary_json("20260710T000000Z", "ssh", &rows);
        assert!(json.contains("\"transport\": \"ssh\""));
        assert!(json.contains("REMOTE_FETCH_FAILED"));
        let md = render_summary_md("20260710T000000Z", "ssh", &rows);
        assert!(md.contains("(ssh)"));
        assert!(md.contains("REMOTE_FETCH_FAILED"));
    }

    #[test]
    fn run_store_ssh_transport_isolates_artifacts_under_ssh_subdir() {
        // The run store already parameterizes transport; `ssh` self-tests must land under
        // <report_root>/runs/<id>/ssh/, never under the root .dev/executor-logs/ dispatch dir.
        let tmp = TempDir::new().unwrap();
        let store = RunStore::new(
            tmp.path().to_path_buf(),
            "20260710T000000Z".to_string(),
            "ssh",
        );
        store.write_reports(&sample_rows()).unwrap();

        let run_dir = store.run_dir();
        assert!(run_dir.ends_with("runs/20260710T000000Z/ssh"));
        assert!(store.logs_dir().starts_with(&run_dir));
        assert!(store.receipts_dir().starts_with(&run_dir));
        assert!(store.summary_json_path().exists());
        // Nothing was written to a sibling root executor-log directory.
        assert!(!tmp.path().join(".dev").join("executor-logs").exists());
    }

    #[test]
    fn report_dir_under_repo_root_dotdev_is_safe() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join(".dev").join("executor-smoke");
        assert!(validate_report_dir(tmp.path(), &target).is_ok());
    }

    #[test]
    fn report_dir_equal_to_repo_root_is_unsafe() {
        let tmp = TempDir::new().unwrap();
        assert!(validate_report_dir(tmp.path(), tmp.path()).is_err());
    }

    #[test]
    fn report_dir_under_docs_is_unsafe() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("docs").join("evidence");
        assert!(validate_report_dir(tmp.path(), &target).is_err());
    }

    #[test]
    fn report_dir_under_plugins_is_unsafe() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("plugins").join("gal-core");
        assert!(validate_report_dir(tmp.path(), &target).is_err());
    }

    #[test]
    fn report_dir_equal_to_readme_is_unsafe() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("README.md");
        assert!(validate_report_dir(tmp.path(), &target).is_err());
    }

    #[test]
    fn report_dir_outside_repo_root_is_safe() {
        let repo = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        assert!(validate_report_dir(repo.path(), outside.path()).is_ok());
    }

    #[test]
    fn unsafe_report_dir_rejected_before_any_write() {
        let tmp = TempDir::new().unwrap();
        let unsafe_target = tmp.path().join("docs");
        let result = validate_report_dir(tmp.path(), &unsafe_target);
        assert!(result.is_err());
        // no write attempted by the validator itself — confirm nothing was created
        assert!(!unsafe_target.exists());
    }

    // ── smoke spec assembly + fixtures ──────────────────────────────────────

    #[test]
    fn assembled_smoke_spec_carries_mandatory_receipt_clause() {
        let spec = assemble_smoke_spec("codex", "/tmp/receipt.md", "2026-07-09T00:00:00Z");
        assert!(
            spec.contains("MANDATORY"),
            "assembled spec must carry the mandatory receipt clause"
        );
        assert!(spec.contains("/tmp/receipt.md"));
        assert!(spec.contains("T-SMOKE"));
    }

    fn fixture_path(name: &str) -> String {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/executor-smoke/"
        )
        .to_string()
            + name
    }

    // Resolve a POSIX shell for fixture scripts. On Windows `sh` is not on PATH,
    // but Git ships one; fall back to a bare `sh` elsewhere.
    fn fixture_shell() -> String {
        #[cfg(target_os = "windows")]
        {
            for path in [
                r"C:\Program Files\Git\bin\sh.exe",
                r"C:\Program Files\Git\usr\bin\sh.exe",
            ] {
                if std::path::Path::new(path).is_file() {
                    return path.to_string();
                }
            }
        }
        "sh".to_string()
    }

    #[test]
    fn pass_fixture_writes_receipt_and_exits_zero() {
        let tmp = TempDir::new().unwrap();
        let receipt = tmp.path().join("receipt.md");
        let cfg = dispatch::dispatch::SpawnConfig {
            executor: fixture_shell(),
            executor_args: vec![fixture_path("pass.sh"), receipt.display().to_string()],
            spec: "spec-body".to_string(),
            workdir: tmp.path().to_path_buf(),
            timeout_secs: 30,
            task_id: "T-SMOKE".to_string(),
            phase: "audit".to_string(),
            actual_model: "n/a".to_string(),
            log_dir: tmp.path().join("logs"),
            receipt_path: Some(receipt.clone()),
            contract_provenance: None,
        };
        let result = dispatch::dispatch::spawn_executor(&cfg).unwrap();
        assert_eq!(result.terminal_state, TerminalState::Completed);
        assert!(receipt.exists());
        assert_eq!(
            std::fs::read_to_string(&receipt).unwrap().trim(),
            "SMOKE_PASS"
        );
    }

    #[test]
    fn call_failed_fixture_exits_nonzero() {
        let tmp = TempDir::new().unwrap();
        let receipt = tmp.path().join("receipt.md");
        let cfg = dispatch::dispatch::SpawnConfig {
            executor: fixture_shell(),
            executor_args: vec![fixture_path("call-failed.sh")],
            spec: "spec-body".to_string(),
            workdir: tmp.path().to_path_buf(),
            timeout_secs: 30,
            task_id: "T-SMOKE".to_string(),
            phase: "audit".to_string(),
            actual_model: "n/a".to_string(),
            log_dir: tmp.path().join("logs"),
            receipt_path: Some(receipt),
            contract_provenance: None,
        };
        let result = dispatch::dispatch::spawn_executor(&cfg).unwrap();
        assert_eq!(result.terminal_state, TerminalState::DisconnectedPartial);
    }

    #[test]
    fn no_receipt_fixture_exits_zero_without_receipt() {
        let tmp = TempDir::new().unwrap();
        let receipt = tmp.path().join("receipt.md");
        let cfg = dispatch::dispatch::SpawnConfig {
            executor: fixture_shell(),
            executor_args: vec![fixture_path("no-receipt.sh")],
            spec: "spec-body".to_string(),
            workdir: tmp.path().to_path_buf(),
            timeout_secs: 30,
            task_id: "T-SMOKE".to_string(),
            phase: "audit".to_string(),
            actual_model: "n/a".to_string(),
            log_dir: tmp.path().join("logs"),
            receipt_path: Some(receipt.clone()),
            contract_provenance: None,
        };
        let result = dispatch::dispatch::spawn_executor(&cfg).unwrap();
        assert_eq!(result.terminal_state, TerminalState::NoReceipt);
        assert!(!receipt.exists());
    }

    // ── local transport adapter ──────────────────────────────────────────────

    #[test]
    fn synthetic_routing_round_trips_through_real_routing_loader() {
        let tmp = TempDir::new().unwrap();
        let routing_path = tmp.path().join("codex-routing.json");
        write_synthetic_routing(&routing_path, "codex").unwrap();

        let table = dispatch::routing::load_routing(&routing_path);
        let entry = table.get(SMOKE_ROLE_KEY).expect("AUDITOR entry must load");
        assert_eq!(entry.executor, "codex");
        assert_eq!(entry.model, smoke_default_model("codex"));
    }

    #[test]
    fn no_receipt_cause_distinguishes_overflow_from_plain() {
        // A log carrying an overflow marker → ContextOverflow.
        let overflow_log = "GAL-DISPATCH-LOG v1\nexit_code: 0\n---STDERR---\nError: This model's maximum context length is 128000 tokens.\n";
        assert_eq!(
            classify_no_receipt_cause(overflow_log),
            NoReceiptCause::ContextOverflow
        );
        assert_eq!(NoReceiptCause::ContextOverflow.as_str(), "context-overflow");

        // A clean exit-0 log with no overflow marker → ExitZeroNoWriteBack.
        let plain_log = "GAL-DISPATCH-LOG v1\nexit_code: 0\nterminal_state: no-receipt\n---STDOUT---\n{\"result\":\"ok\"}\n";
        assert_eq!(
            classify_no_receipt_cause(plain_log),
            NoReceiptCause::ExitZeroNoWriteBack
        );
        assert_eq!(
            NoReceiptCause::ExitZeroNoWriteBack.as_str(),
            "exit-0-no-write-back"
        );
    }

    #[test]
    fn no_receipt_cause_note_reads_log_and_missing_is_none() {
        let tmp = TempDir::new().unwrap();
        let log = tmp.path().join("run.log");
        std::fs::write(&log, "token limit reached").unwrap();
        let note = no_receipt_cause_note(Some(log.to_str().unwrap())).unwrap();
        assert!(note.contains("context-overflow"), "got {note}");
        // Missing path → None (raw log stays the authoritative evidence).
        assert!(no_receipt_cause_note(None).is_none());
        assert!(no_receipt_cause_note(Some("/no/such/log/xyz")).is_none());
    }

    #[test]
    fn smoke_default_model_opencode_is_double_prefixed_and_copilot_is_auto() {
        // OpenCode's smoke model must be the double-prefixed id present in the
        // installed CLI; Copilot Free is auto-only (no-op guard against regression).
        assert_eq!(
            smoke_default_model("opencode"),
            "nvidia/nvidia/nemotron-3-ultra-550b-a55b"
        );
        assert_eq!(smoke_default_model("copilot"), "auto");
    }

    #[test]
    fn smoke_one_agent_unrecognized_executor_short_circuits_as_unsupported() {
        let tmp = TempDir::new().unwrap();
        let store = RunStore::new(
            tmp.path().to_path_buf(),
            "20260709T000000Z".to_string(),
            "local",
        );
        let row = smoke_one_agent(
            "nonexistent-executor-xyz-12345",
            &store,
            5,
            "2026-07-09T00:00:00Z",
        );
        assert_eq!(row.final_status, SmokeStatus::Unsupported);
        assert!(row.log_path.is_none());
        assert!(row.receipt_path.is_none());
        // UNSUPPORTED short-circuits before writing the routing file or dispatching —
        // nothing should exist under the run dir at all.
        assert!(!store.run_dir().exists());
    }

    #[test]
    fn run_local_smoke_defaults_to_full_agent_matrix_when_unfiltered() {
        // Uses AGENT_MATRIX's own names only to prove *selection* (all five, in matrix
        // order); it does not assert on classification, since one or more of the real
        // CLIs may legitimately be installed on the machine running this test suite —
        // asserting per-agent status here would either be flaky or make a real call.
        let names: Vec<String> = AGENT_MATRIX.iter().map(|s| s.to_string()).collect();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&"codex".to_string()));
        assert!(names.contains(&"opencode".to_string()));
    }

    #[test]
    fn run_local_smoke_respects_explicit_executor_filter() {
        let tmp = TempDir::new().unwrap();
        let store = RunStore::new(
            tmp.path().to_path_buf(),
            "20260709T000000Z".to_string(),
            "local",
        );
        // Deliberately fictitious names — never a real installed CLI, and never one of
        // AGENT_MATRIX's five — so this exercises only the filter/selection logic and
        // short-circuits at UNSUPPORTED, never attempting a real dispatch.
        let filter = vec![
            "nonexistent-executor-filter-a".to_string(),
            "nonexistent-executor-filter-b".to_string(),
        ];
        let rows = run_local_smoke(&filter, &store, 1, "2026-07-09T00:00:00Z");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].executor, "nonexistent-executor-filter-a");
        assert_eq!(rows[1].executor, "nonexistent-executor-filter-b");
        assert!(rows
            .iter()
            .all(|r| r.final_status == SmokeStatus::Unsupported));
    }

    #[test]
    fn smoke_one_agent_recognized_but_unavailable_matrix_name_would_be_not_installed() {
        // classify_status's NOT_INSTALLED row is exercised directly (see the classification
        // tests above) since every AGENT_MATRIX name may legitimately be installed on the
        // machine running this suite — this test only proves AGENT_MATRIX membership gates
        // the UNSUPPORTED short-circuit, not that any given name is (un)available here.
        assert!(AGENT_MATRIX.contains(&"codex"));
        assert!(!AGENT_MATRIX.contains(&"nonexistent-executor-xyz-12345"));
    }

    // ── SSH transport adapter ────────────────────────────────────────────────

    fn ev<'a>(
        reason: Option<&'a str>,
        terminal: Option<&'a TerminalState>,
        receipt_ok: bool,
    ) -> SshDispatchEvidence<'a> {
        SshDispatchEvidence {
            reason,
            terminal,
            receipt_ok,
        }
    }

    #[test]
    fn parse_remote_auth_detects_unauthenticated_markers() {
        for out in [
            "Error: not logged in. Please log in with `login`.",
            "authentication required",
            "You are NOT SIGNED IN",
            "no api key found",
        ] {
            assert_eq!(
                parse_remote_auth(out),
                RemoteAuth::Unauthenticated,
                "expected Unauthenticated for: {out}"
            );
        }
    }

    #[test]
    fn parse_remote_auth_clean_version_output_is_unknown() {
        // A plain version banner carries no auth signal → defer to the bounded smoke.
        assert_eq!(parse_remote_auth("codex 1.2.3"), RemoteAuth::Unknown);
        assert_eq!(parse_remote_auth(""), RemoteAuth::Unknown);
    }

    #[test]
    fn classify_ssh_unreachable_short_circuits_before_any_probe() {
        // unreachable → SSH_UNREACHABLE regardless of downstream inputs.
        assert_eq!(
            classify_ssh_status(false, false, RemoteAuth::Unknown, None),
            SmokeStatus::SshUnreachable
        );
    }

    #[test]
    fn classify_ssh_remote_missing_executable_is_not_installed() {
        // install decided on the remote machine, not the local control node.
        assert_eq!(
            classify_ssh_status(true, false, RemoteAuth::Unknown, None),
            SmokeStatus::NotInstalled
        );
    }

    #[test]
    fn classify_ssh_confirmed_no_auth_is_not_authenticated_without_dispatch() {
        // confirmed no-auth → NOT_AUTHENTICATED, never spawns (dispatched = None).
        assert_eq!(
            classify_ssh_status(true, true, RemoteAuth::Unauthenticated, None),
            SmokeStatus::NotAuthenticated
        );
    }

    #[test]
    fn classify_ssh_auth_unknown_preflight_only_is_auth_unknown() {
        // auth-unknown + no bounded smoke attempted → AUTH_UNKNOWN preflight.
        assert_eq!(
            classify_ssh_status(true, true, RemoteAuth::Unknown, None),
            SmokeStatus::AuthUnknown
        );
    }

    #[test]
    fn classify_ssh_auth_unknown_with_dispatch_uses_smoke_outcome() {
        // once the bounded smoke ran, the final status is the remote outcome.
        let e = ev(None, Some(&TerminalState::Completed), true);
        assert_eq!(
            classify_ssh_status(true, true, RemoteAuth::Unknown, Some(&e)),
            SmokeStatus::Pass
        );
    }

    #[test]
    fn classify_ssh_remote_copilot_cliflag_is_unsupported() {
        // the real CliFlag-rejection branch surfaces remote-cliflag-unsupported.
        let e = ev(Some("remote-cliflag-unsupported"), None, false);
        assert_eq!(
            classify_ssh_status(true, true, RemoteAuth::Unknown, Some(&e)),
            SmokeStatus::Unsupported
        );
    }

    #[test]
    fn classify_ssh_remote_guard_failed_is_remote_guard_failed() {
        // guard failure surfaces as its own status, not a generic failure.
        let e = ev(Some("remote-guard-failed"), None, false);
        assert_eq!(
            classify_ssh_status(true, true, RemoteAuth::Unknown, Some(&e)),
            SmokeStatus::RemoteGuardFailed
        );
    }

    #[test]
    fn classify_ssh_success_reuses_pass_with_fetched_receipt() {
        // completed + fetched non-empty receipt → PASS.
        let e = ev(None, Some(&TerminalState::Completed), true);
        assert_eq!(
            classify_ssh_status(true, true, RemoteAuth::Unknown, Some(&e)),
            SmokeStatus::Pass
        );
    }

    #[test]
    fn classify_ssh_completed_without_receipt_is_no_receipt() {
        // completed but the fetched receipt was missing/empty → NO_RECEIPT.
        let e = ev(None, Some(&TerminalState::Completed), false);
        assert_eq!(
            classify_ssh_status(true, true, RemoteAuth::Unknown, Some(&e)),
            SmokeStatus::NoReceipt
        );
        let e2 = ev(None, Some(&TerminalState::NoReceipt), false);
        assert_eq!(
            classify_ssh_status(true, true, RemoteAuth::Unknown, Some(&e2)),
            SmokeStatus::NoReceipt
        );
    }

    #[test]
    fn classify_ssh_fetch_failure_is_remote_fetch_failed_not_no_receipt() {
        // the whole point of the run.rs reason field — fetch failure is distinct
        // from a genuinely missing receipt even though both carry TerminalState::NoReceipt.
        let e = ev(
            Some("remote-receipt-fetch-failed"),
            Some(&TerminalState::NoReceipt),
            false,
        );
        assert_eq!(
            classify_ssh_status(true, true, RemoteAuth::Unknown, Some(&e)),
            SmokeStatus::RemoteFetchFailed
        );
    }

    #[test]
    fn classify_ssh_timeout_and_call_failed_never_pass() {
        // timeout/disconnect stay non-pass.
        for state in [
            TerminalState::Timeout,
            TerminalState::TimeoutNoOutput,
            TerminalState::TimeoutMidrun,
        ] {
            let e = ev(None, Some(&state), false);
            assert_eq!(
                classify_ssh_status(true, true, RemoteAuth::Unknown, Some(&e)),
                SmokeStatus::Timeout
            );
        }
        let e = ev(None, Some(&TerminalState::DisconnectedPartial), false);
        assert_eq!(
            classify_ssh_status(true, true, RemoteAuth::Unknown, Some(&e)),
            SmokeStatus::CallFailed
        );
    }

    #[test]
    fn classify_ssh_half_config_and_absolute_receipt_reasons_are_config_error() {
        for reason in [
            "remote-missing-workdir",
            "remote-absolute-receipt-unsupported",
        ] {
            let e = ev(Some(reason), None, false);
            assert_eq!(
                classify_ssh_status(true, true, RemoteAuth::Unknown, Some(&e)),
                SmokeStatus::ConfigError
            );
        }
    }

    #[test]
    fn synthetic_ssh_routing_round_trips_as_remote_route() {
        let tmp = TempDir::new().unwrap();
        let routing_path = tmp.path().join("claude-ssh-routing.json");
        write_synthetic_ssh_routing(&routing_path, "claude", "user@host", "/home/user/gal-smoke")
            .unwrap();
        let table = dispatch::routing::load_routing(&routing_path);
        let entry = table.get(SMOKE_ROLE_KEY).expect("AUDITOR entry must load");
        assert_eq!(entry.executor, "claude");
        assert_eq!(entry.ssh_target.as_deref(), Some("user@host"));
        assert_eq!(
            entry.remote_workdir.as_deref(),
            Some("/home/user/gal-smoke")
        );
        assert!(
            entry.is_remote(),
            "synthesized route must be a remote route"
        );
        assert!(
            table.warnings.is_empty(),
            "unexpected warnings: {:?}",
            table.warnings
        );
    }

    // ── SSH fixture-driven transport tests ───────────────────────────────────
    //
    // These drive the REAL probe + parse code through the injectable `SshProbe`
    // seam against fake-ssh fixture scripts (no network). They deliberately cover
    // only the pre-dispatch short-circuit statuses (unreachable / not-installed /
    // not-authenticated), which never reach `run_dispatch` — the dispatch-derived
    // statuses (pass / no-receipt / fetch-failure / timeout) are covered by the
    // pure `classify_ssh_status` tests above, and end-to-end by live acceptance.

    fn ssh_fixture_probe(name: &str) -> SshProbe {
        SshProbe::with_program(&fixture_shell(), vec![fixture_path(&format!("ssh/{name}"))])
    }

    #[test]
    fn ssh_probe_fixtures_drive_real_reachability_install_and_auth_parse() {
        let unauth = ssh_fixture_probe("unauthenticated.sh");
        assert!(unauth.reachable("target"));
        assert!(unauth.remote_available("target", "agy"));
        assert_eq!(
            unauth.remote_auth("target", "agy"),
            RemoteAuth::Unauthenticated
        );

        let missing = ssh_fixture_probe("not-installed.sh");
        assert!(missing.reachable("target"));
        assert!(!missing.remote_available("target", "codex"));

        let down = ssh_fixture_probe("unreachable.sh");
        assert!(!down.reachable("target"));
    }

    #[test]
    fn run_ssh_smoke_unreachable_marks_every_row_ssh_unreachable() {
        let tmp = TempDir::new().unwrap();
        let store = RunStore::new(
            tmp.path().to_path_buf(),
            "20260710T000000Z".to_string(),
            "ssh",
        );
        let probe = ssh_fixture_probe("unreachable.sh");
        let rows = run_ssh_smoke(&[], "fake-target", "/remote/wd", &probe, &store, 5, "gen");
        assert_eq!(rows.len(), 5, "all five agents get a truthful row");
        assert!(rows
            .iter()
            .all(|r| r.final_status == SmokeStatus::SshUnreachable));

        store.write_reports(&rows).unwrap();
        assert!(store.run_dir().ends_with("runs/20260710T000000Z/ssh"));
        // An unreachable run never dispatches → no root executor-log pollution.
        assert!(!tmp.path().join(".dev").join("executor-logs").exists());
    }

    #[test]
    fn run_ssh_smoke_reachable_but_missing_executable_is_not_installed() {
        let tmp = TempDir::new().unwrap();
        let store = RunStore::new(
            tmp.path().to_path_buf(),
            "20260710T000000Z".to_string(),
            "ssh",
        );
        let probe = ssh_fixture_probe("not-installed.sh");
        let rows = run_ssh_smoke(
            &["codex".to_string()],
            "target",
            "/remote/wd",
            &probe,
            &store,
            5,
            "gen",
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].final_status, SmokeStatus::NotInstalled);
        assert_eq!(rows[0].auth_preflight, "not-installed");
    }

    #[test]
    fn run_ssh_smoke_reachable_installed_unauthenticated_is_not_authenticated_without_dispatch() {
        let tmp = TempDir::new().unwrap();
        let store = RunStore::new(
            tmp.path().to_path_buf(),
            "20260710T000000Z".to_string(),
            "ssh",
        );
        let probe = ssh_fixture_probe("unauthenticated.sh");
        let rows = run_ssh_smoke(
            &["agy".to_string()],
            "target",
            "/remote/wd",
            &probe,
            &store,
            5,
            "gen",
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].final_status, SmokeStatus::NotAuthenticated);
        assert_eq!(rows[0].auth_preflight, "unauthenticated");
        // Confirmed no-auth never spawns a dispatch → no log/session recorded.
        assert!(rows[0].log_path.is_none());
        assert!(rows[0].session_id.is_none());
    }

    #[test]
    fn ssh_summary_json_shares_the_local_report_schema() {
        // Report-schema parity: an SSH report has the same top-level keys and per-row
        // fields as a local report — only `transport` differs.
        let tmp = TempDir::new().unwrap();
        let store = RunStore::new(
            tmp.path().to_path_buf(),
            "20260710T000000Z".to_string(),
            "ssh",
        );
        let probe = ssh_fixture_probe("not-installed.sh");
        let rows = run_ssh_smoke(
            &["codex".to_string()],
            "target",
            "/remote/wd",
            &probe,
            &store,
            5,
            "gen",
        );
        let json = render_summary_json(&store.run_id, &store.transport, &rows);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["transport"], "ssh");
        assert!(v["runId"].is_string());
        let row = &v["rows"][0];
        for key in [
            "executor",
            "authPreflight",
            "finalStatus",
            "logPath",
            "receiptPath",
            "sessionId",
            "nextActionHint",
        ] {
            assert!(row.get(key).is_some(), "SSH report row missing key '{key}'");
        }
    }

    #[test]
    fn hang_fixture_times_out() {
        let tmp = TempDir::new().unwrap();
        let cfg = dispatch::dispatch::SpawnConfig {
            executor: fixture_shell(),
            executor_args: vec![fixture_path("hang.sh")],
            spec: "spec-body".to_string(),
            workdir: tmp.path().to_path_buf(),
            timeout_secs: 1,
            task_id: "T-SMOKE".to_string(),
            phase: "audit".to_string(),
            actual_model: "n/a".to_string(),
            log_dir: tmp.path().join("logs"),
            receipt_path: None,
            contract_provenance: None,
        };
        let result = dispatch::dispatch::spawn_executor(&cfg).unwrap();
        assert!(matches!(
            result.terminal_state,
            TerminalState::TimeoutNoOutput | TerminalState::TimeoutMidrun
        ));
    }
}
