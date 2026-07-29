//! Subprocess spawn + stdin feed + process-tree timeout + durable executor log.
//!
//! # Terminal state vocabulary
//!
//! | State                | Meaning                                              |
//! |----------------------|------------------------------------------------------|
//! | `completed`          | Exit 0 (write-back verification)      |
//! | `no-receipt`         | Exit 0 but write-back not confirmed   |
//! | `timeout`            | Process killed because `timeout_secs` elapsed        |
//! | `disconnected-partial` | Non-zero exit code                                 |
//! | `unavailable`        | Executor CLI not found in PATH                       |
//!
//! # Log file format
//!
//! `<log_dir>/<epoch_secs>-<subsec_nanos>-<attempt_seq>-<task_id>-<phase>-<executor>.log`
//!
//! The `<subsec_nanos>-<attempt_seq>` pair makes the path collision-safe. Within
//! one process the `attempt_seq` monotonic counter alone guarantees distinct paths,
//! even for two attempts in the same nanosecond. Across two concurrent `gal`
//! processes the counter does not help (each starts at 0), so `subsec_nanos` is
//! what separates them there — a same-nanosecond cross-process collision is
//! astronomically unlikely but not impossible, which is acceptable because
//! concurrent plans are isolated by worktree and rarely share a log directory.
//!
//! # Marker-before-spawn
//!
//! Before any availability check or process spawn, [`spawn_executor`] allocates the
//! unique attempt path above and durably writes a `terminal_state: started` marker
//! there. Marker-write failure prevents spawn (propagated as [`DispatchError`]).
//! On terminal completion the SAME path is rewritten with the full log content; if
//! that terminal rewrite fails, the file is left holding the `started` marker — this
//! residue is intentional evidence of an unterminated attempt, not a bug.
//!
//! Header lines (always present before stdout/stderr sections):
//! ```text
//! GAL-DISPATCH-LOG v1
//! timestamp_start: <ISO-8601>
//! timestamp_end:   <ISO-8601>
//! duration_ms:     <u64>
//! executor:        <name>
//! phase:           <implement|test|audit|verify>
//! task_id:         <T-NN>
//! git_branch:      <branch>
//! git_head:        <sha7>
//! exit_code:       <i32 or "none">
//! actual_model:    <model-id>
//! terminal_state:  <state>
//! session_id:      <uuid or "none">
//! ---STDOUT---
//! ...
//! ---STDERR---
//! ...
//! ```

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use thiserror::Error;

const COPILOT_TOKEN_VARS: &[&str] = &["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"];
const CLAUDE_TOKEN_VARS: &[&str] = &["ANTHROPIC_API_KEY", "CLAUDE_API_KEY"];
const CODEX_TOKEN_VARS: &[&str] = &["OPENAI_API_KEY", "CODEX_API_KEY"];
const AGY_TOKEN_VARS: &[&str] = &[
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GOOGLE_GENERATIVE_AI_API_KEY",
];
const OPENCODE_TOKEN_VARS: &[&str] = &["OPENCODE_API_KEY"];

/// Fixed grace period timeout cleanup waits for confirmed child exit, after
/// primary termination and again after the Windows-only `taskkill /F /T`
/// fallback. Never waited indefinitely — a `recv_timeout` bound on the
/// existing wait channel, not a blocking join.
const CLEANUP_GRACE_PERIOD: Duration = Duration::from_secs(5);

// ── Terminal state ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalState {
    /// Process exited 0; write-back verified.
    Completed,
    /// Process exited 0 but write-back not confirmed.
    NoReceipt,
    /// Killed because timeout elapsed.
    Timeout,
    /// Timed out after producing zero output before the kill. Compatibility
    /// state token only — observed output alone does not establish a cause.
    TimeoutNoOutput,
    /// Timed out after producing some output before the kill. Compatibility
    /// state token only — observed output alone does not establish a cause.
    TimeoutMidrun,
    /// Non-zero exit code.
    DisconnectedPartial,
    /// Executor CLI not found in PATH.
    Unavailable,
}

impl TerminalState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::NoReceipt => "no-receipt",
            Self::Timeout => "timeout",
            Self::TimeoutNoOutput => "timeout-no-output",
            Self::TimeoutMidrun => "timeout-midrun",
            Self::DisconnectedPartial => "disconnected-partial",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Outcome of the dedicated stdin-writer thread, reported over a one-shot
/// channel so timeout evidence can distinguish a delivery stall from an
/// executor that received the full spec but simply emitted no output.
/// `Pending` means the main path checked before the writer reported back —
/// it is read non-blockingly (`try_recv`), never waited on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StdinDelivery {
    Complete,
    Failed,
    Pending,
}

impl StdinDelivery {
    fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Failed => "failed",
            Self::Pending => "pending",
        }
    }
}

impl std::fmt::Display for TerminalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    Unauthenticated { hint: String },
    Unknown { message: String },
}

// ── Dispatch configuration ─────────────────────────────────────────────────────

/// Coupled contract-provenance value: the control-node absolute path a phase
/// contract was read from, and which resolution tier produced it
/// (`workdir`/`ancestor`/`exe-side`/`embedded`). Kept as one value so path and
/// source can never diverge as it moves from `DispatchArgs` into `SpawnConfig`
/// and on into markers/logs. Set programmatically only (never an argv flag) —
/// `None` for raw/direct dispatch, which stays byte-compatible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractProvenance {
    pub path: PathBuf,
    pub source: String,
}

/// Full configuration for one dispatch invocation.
#[derive(Debug, Clone)]
pub struct SpawnConfig {
    /// CLI executable name (e.g. "claude", "codex"). Must be findable in PATH.
    pub executor: String,
    /// Arguments passed to the executor *before* stdin spec injection.
    pub executor_args: Vec<String>,
    /// Task spec written to the executor's stdin.
    /// For adapters that embed the spec in args (e.g. copilot `-p`), this should be empty.
    pub spec: String,
    /// Working directory for the spawned process.
    pub workdir: PathBuf,
    /// Kill threshold in seconds (hard wall-clock limit).
    pub timeout_secs: u64,
    /// Task identifier (e.g. "T-NN") — used in log filename and header.
    pub task_id: String,
    /// Pipeline phase string (e.g. "implement") — used in log filename and header.
    pub phase: String,
    /// Actual model injected (for log header transparency).
    pub actual_model: String,
    /// Directory where executor logs are written.
    /// Defaults to `<workdir>/.dev/executor-logs/` when constructed via [`SpawnConfig::default_log_dir`].
    pub log_dir: PathBuf,
    /// Path of the file the executor is expected to write back to.
    ///
    /// When `Some`, terminal state is only `Completed` if this file exists and is
    /// non-empty after a successful exit (exit 0). Otherwise it is downgraded to
    /// `NoReceipt`. When `None`, receipt verification is skipped and a successful
    /// exit is always recorded as `Completed`.
    pub receipt_path: Option<PathBuf>,
    /// Optional coupled contract path+source, carried through from
    /// `DispatchArgs.contract_provenance` for pipeline-created dispatches.
    /// `None` for raw/direct dispatch — composition is unchanged in that case.
    pub contract_provenance: Option<ContractProvenance>,
}

impl SpawnConfig {
    /// Convenience: resolve the default log dir relative to `workdir`.
    pub fn default_log_dir(workdir: &Path) -> PathBuf {
        workdir.join(".dev").join("executor-logs")
    }
}

// ── Dispatch result ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct DispatchResult {
    pub terminal_state: TerminalState,
    pub log_path: PathBuf,
    /// Provider-native session/job id extracted from stdout (if any).
    pub session_id: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
}

// ── Errors ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum DispatchError {
    #[error("log directory could not be created: {0}")]
    LogDirCreate(std::io::Error),
    #[error("log file could not be written: {0}")]
    LogWrite(std::io::Error),
}

// ── Main dispatch entry point ──────────────────────────────────────────────────

/// Spawn the executor, feed the spec via stdin, enforce the timeout, and write
/// a durable executor log. Returns a `DispatchResult` — never panics.
pub fn spawn_executor(cfg: &SpawnConfig) -> Result<DispatchResult, DispatchError> {
    let start = Instant::now();
    let start_ts = utc_now_iso8601();

    // ── Allocate a collision-safe unique attempt path and durably write the
    // `started` marker BEFORE any availability check or process spawn. Marker
    // write failure prevents spawn — the `?` returns before the child is ever
    // launched. Every branch below reuses this same `log_path` so the eventual
    // terminal state is a rewrite of this attempt, not a second file.
    let log_path = build_log_path(&cfg.log_dir, &cfg.task_id, &cfg.phase, &cfg.executor);
    write_started_marker(&log_path, cfg, &start_ts)?;

    // ── Check executor availability ─────────────────────────────────────────
    if !is_available(&cfg.executor) {
        let duration_ms = start.elapsed().as_millis() as u64;
        let end_ts = utc_now_iso8601();
        let result = DispatchResult {
            terminal_state: TerminalState::Unavailable,
            log_path: log_path.clone(),
            session_id: None,
            exit_code: None,
            duration_ms,
            stdout: String::new(),
            stderr: format!("executor '{}' not found in PATH", cfg.executor),
        };
        write_log(&log_path, cfg, &start_ts, &end_ts, duration_ms, &result)?;
        return Ok(result);
    }

    // ── Spawn the child process, contained ──────────────────────────────────
    // `build_command` resolves Windows script shims (.cmd/.bat/.ps1) that
    // CreateProcess cannot launch directly (e.g. npm-installed `codex.cmd`).
    // `spawn_contained` places the child in a kill-on-close Job Object
    // (Windows) or a dedicated process group (Unix) before it ever executes.
    let mut command = build_command(&cfg.executor, &cfg.executor_args);
    command
        .current_dir(&cfg.workdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (mut child, primary_termination) = match spawn_contained(&mut command) {
        Ok(c) => c,
        Err(e) => {
            // Spawn failure (e.g. permission denied) → unavailable
            let duration_ms = start.elapsed().as_millis() as u64;
            let end_ts = utc_now_iso8601();
            let result = DispatchResult {
                terminal_state: TerminalState::Unavailable,
                log_path: log_path.clone(),
                session_id: None,
                exit_code: None,
                duration_ms,
                stdout: String::new(),
                stderr: format!("spawn failed: {e}"),
            };
            write_log(&log_path, cfg, &start_ts, &end_ts, duration_ms, &result)?;
            return Ok(result);
        }
    };

    let child_pid = child.id();
    let timeout = Duration::from_secs(cfg.timeout_secs);
    let spec_bytes = cfg.spec.len();

    // ── Drain stdout/stderr on reader threads into shared buffers ───────────
    // Started BEFORE spec delivery: a child that writes output before or
    // without ever reading stdin must never be starved of drain capacity by
    // a stdin write that hasn't happened yet.
    // `wait_with_output` would lose partial output on a timeout kill, so we read
    // the pipes incrementally. `saw_output` flips on the first byte from either
    // stream — at a timeout this records only whether any output byte was
    // observed before the kill, not a cause.
    use std::io::Read;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    let saw_output = Arc::new(AtomicBool::new(false));
    let out_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let err_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let drain = |mut pipe: Option<Box<dyn Read + Send>>,
                 buf: Arc<Mutex<Vec<u8>>>,
                 flag: Arc<AtomicBool>| {
        std::thread::spawn(move || {
            if let Some(ref mut p) = pipe {
                let mut chunk = [0u8; 4096];
                while let Ok(n) = p.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    flag.store(true, Ordering::SeqCst);
                    if let Ok(mut b) = buf.lock() {
                        b.extend_from_slice(&chunk[..n]);
                    }
                }
            }
        })
    };
    let stdout_pipe: Option<Box<dyn Read + Send>> = child
        .stdout
        .take()
        .map(|s| Box::new(s) as Box<dyn Read + Send>);
    let stderr_pipe: Option<Box<dyn Read + Send>> = child
        .stderr
        .take()
        .map(|s| Box::new(s) as Box<dyn Read + Send>);
    let h_out = drain(stdout_pipe, Arc::clone(&out_buf), Arc::clone(&saw_output));
    let h_err = drain(stderr_pipe, Arc::clone(&err_buf), Arc::clone(&saw_output));

    // ── Feed spec to stdin on a dedicated thread ─────────────────────────────
    // A large spec can fill the OS pipe buffer before a child that buffers
    // its own output first (or never reads stdin at all) drains it — a
    // synchronous `write_all` here would then block the timeout clock
    // itself. The writer runs independently and reports its own outcome over
    // a one-shot channel; nothing on the main path ever joins it before
    // timeout cleanup completes (see the timeout branch below).
    let (stdin_tx, stdin_rx) = mpsc::channel::<StdinDelivery>();
    let spec_data = cfg.spec.clone().into_bytes();
    if let Some(mut stdin) = child.stdin.take() {
        std::thread::spawn(move || {
            let result = stdin.write_all(&spec_data);
            let _ = stdin_tx.send(if result.is_ok() {
                StdinDelivery::Complete
            } else {
                StdinDelivery::Failed
            });
            // `stdin` dropped here → EOF sent to child (on success) or the
            // pipe is simply closed (on failure, e.g. the child already exited).
        });
    } else {
        let _ = stdin_tx.send(StdinDelivery::Failed);
    }

    // ── Wait with timeout using a background thread ─────────────────────────
    let (tx, rx) = mpsc::channel::<std::io::Result<std::process::ExitStatus>>();
    std::thread::spawn(move || {
        let status = child.wait();
        let _ = tx.send(status);
    });

    let collect = |buf: &Arc<Mutex<Vec<u8>>>| {
        buf.lock()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default()
    };

    let (terminal_state, exit_code, stdout_str, stderr_str) = match rx.recv_timeout(timeout) {
        Ok(Ok(status)) => {
            let _ = h_out.join();
            let _ = h_err.join();
            let code = status.code();
            let out = collect(&out_buf);
            let err = collect(&err_buf);
            let state = if status.success() {
                // verify write-back. Downgrade to NoReceipt if file absent/empty.
                match &cfg.receipt_path {
                    None => TerminalState::Completed,
                    Some(path) => {
                        if verify_receipt(path) {
                            TerminalState::Completed
                        } else {
                            TerminalState::NoReceipt
                        }
                    }
                }
            } else {
                TerminalState::DisconnectedPartial
            };
            (state, code, out, err)
        }
        Ok(Err(e)) => (
            TerminalState::DisconnectedPartial,
            None,
            collect(&out_buf),
            format!("wait error: {e}"),
        ),
        Err(_timeout) => {
            // Bounded timeout cleanup: fire primary termination (Windows Job
            // Object / Unix process-group kill from spawn-time containment),
            // then wait only
            // through fixed grace periods on the existing wait channel — never
            // indefinitely. Drains are joined only after exit is confirmed;
            // if cleanup cannot be confirmed even after the Windows-only
            // `taskkill /F /T` fallback, return `DisconnectedPartial` with
            // `cleanup-unconfirmed` and leave the drain threads unjoined
            // rather than risk blocking on a child that never truly exited.
            primary_termination.terminate(child_pid);
            let confirmed = match rx.recv_timeout(CLEANUP_GRACE_PERIOD) {
                Ok(_) => true,
                Err(_) => {
                    #[cfg(target_os = "windows")]
                    {
                        kill_process_tree(child_pid); // taskkill /F /T fallback
                        rx.recv_timeout(CLEANUP_GRACE_PERIOD).is_ok()
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        false
                    }
                }
            };
            // `primary_termination` is dropped at the end of `spawn_executor`
            // (its `Drop` impl closes the Windows Job handle exactly once,
            // on every path — see `impl Drop for PrimaryTermination`).

            // Non-blocking: the writer either already reported back (child
            // exit typically breaks its pipe promptly) or hasn't yet — either
            // way we never wait for it here, per the nonblocking-delivery
            // contract (timeout cleanup never joins/waits on the writer).
            let stdin_delivery = stdin_rx.try_recv().unwrap_or(StdinDelivery::Pending);

            if confirmed {
                let _ = h_out.join();
                let _ = h_err.join();
                let saw = saw_output.load(Ordering::SeqCst);
                // Observation-only: state what was actually seen (output
                // bytes, stdin-delivery status, spec size) and nothing else —
                // never infer a cause (e.g. an interactive prompt or a hang)
                // the evidence available here cannot actually establish.
                let output_observation = if saw {
                    "output observed before kill"
                } else {
                    "no output observed before kill"
                };
                let reason = format!(
                    "killed after {}s timeout; {output_observation}; stdin_delivery={}; spec_bytes={spec_bytes}",
                    cfg.timeout_secs,
                    stdin_delivery.as_str()
                );
                (classify_timeout(saw), None, collect(&out_buf), reason)
            } else {
                (
                    TerminalState::DisconnectedPartial,
                    None,
                    collect(&out_buf),
                    format!(
                        "cleanup-unconfirmed: process did not exit after primary termination \
                         and grace-period fallback; stdin_delivery={}; spec_bytes={spec_bytes}",
                        stdin_delivery.as_str()
                    ),
                )
            }
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    let end_ts = utc_now_iso8601();

    // Extract session id from stdout (tool-specific; adapter layer refines this)
    let session_id = extract_session_id_generic(&stdout_str);

    let result = DispatchResult {
        terminal_state,
        log_path: log_path.clone(),
        session_id,
        exit_code,
        duration_ms,
        stdout: stdout_str,
        stderr: stderr_str,
    };

    // Terminal rewrite of the SAME attempt path allocated before spawn. If this
    // write fails, the file is left holding the `started` marker written above —
    // intentional residue evidencing an unterminated attempt (never panic, never
    // delete).
    write_log(&log_path, cfg, &start_ts, &end_ts, duration_ms, &result)?;
    Ok(result)
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Classify a timeout by whether the executor produced any output before the
/// kill. `TimeoutNoOutput` (zero output) and `TimeoutMidrun` (some output)
/// are compatibility state tokens only — neither implies a cause (an
/// interactive prompt, a hang, or otherwise); the evidence available here is
/// limited to observed output bytes, not the executor's internal state.
fn classify_timeout(saw_output: bool) -> TerminalState {
    if saw_output {
        TerminalState::TimeoutMidrun
    } else {
        TerminalState::TimeoutNoOutput
    }
}

/// Verify that the receipt file exists and is non-empty.
///
/// Returns `true` only when the file exists and has at least one byte. An empty
/// file or a missing file both count as "no receipt".
pub fn verify_receipt(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() > 0)
        .unwrap_or(false)
}

/// Check if an executable is findable in PATH.
pub fn is_available(name: &str) -> bool {
    if Path::new(name).is_file() {
        return true;
    }

    // Try `where` (Windows) or `which` (Unix) as a quick check.
    #[cfg(target_os = "windows")]
    let check = Command::new("where").arg(name).output();
    #[cfg(not(target_os = "windows"))]
    let check = Command::new("which").arg(name).output();

    check.map(|o| o.status.success()).unwrap_or(false)
}

pub fn executor_readiness(executor: &str) -> Readiness {
    match executor.to_ascii_lowercase().as_str() {
        "copilot" => {
            let has_token = COPILOT_TOKEN_VARS.iter().any(|name| env_var_present(name));
            let has_local_state = copilot_local_state_exists();
            copilot_readiness_from(has_token, has_local_state)
        }
        "claude" => probe_readiness(
            "claude",
            CLAUDE_TOKEN_VARS,
            ".claude",
            "run `claude` then `/login`, or set ANTHROPIC_API_KEY for headless use",
        ),
        "codex" => {
            // Nested Codex needs more than `where codex` success: when `codex exec`
            // cannot clean up its arg0 temp shim or resolve a PATH alias it emits an
            // access-denied warning and the nested run fails, even though the CLI is
            // on PATH. Probe `codex exec --help` and classify that output.
            let has_token = CODEX_TOKEN_VARS.iter().any(|name| env_var_present(name));
            let has_local_state = home_subdir_exists(".codex");
            codex_readiness_from(has_token, has_local_state, &codex_probe_output())
        }
        "agy" => {
            let has_token = AGY_TOKEN_VARS.iter().any(|name| env_var_present(name));
            let has_agy_dir = home_subdir_exists(".agy");
            let has_gemini_antigravity_dir = home_subdir_exists(".gemini/antigravity-cli");
            agy_readiness_from(has_token, has_agy_dir, has_gemini_antigravity_dir)
        }
        "opencode" => probe_readiness(
            "opencode",
            OPENCODE_TOKEN_VARS,
            ".opencode",
            "run `opencode auth login`, or set OPENCODE_API_KEY for headless use",
        ),
        _ => Readiness::Unknown {
            message: format!(
                "executor '{executor}' has no dedicated readiness probe yet; allowing dispatch"
            ),
        },
    }
}

fn env_var_present(name: &str) -> bool {
    std::env::var(name)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn copilot_local_state_exists() -> bool {
    home_subdir_exists(".copilot")
}

fn home_subdir_exists(name: &str) -> bool {
    dirs::home_dir()
        .map(|home| home.join(name).exists())
        .unwrap_or(false)
}

/// Generic per-executor readiness probe (mirrors [`copilot_readiness_from`]):
/// a present auth token env var → `Ready`; a local state dir present but
/// headless auth unconfirmable without a dedicated status command → `Unknown`
/// (allow dispatch); neither → `Unauthenticated` with a fix hint. This gives
/// each executor a distinct fail-fast `Unauthenticated` signal before spawn
/// instead of discovering the failure as a timeout.
fn probe_readiness(executor: &str, token_vars: &[&str], state_dir: &str, hint: &str) -> Readiness {
    if token_vars.iter().any(|name| env_var_present(name)) {
        return Readiness::Ready;
    }
    if home_subdir_exists(state_dir) {
        return Readiness::Unknown {
            message: format!(
                "{executor} has local state under ~/{state_dir} but headless authentication \
                 could not be confirmed without a dedicated status command; allowing dispatch"
            ),
        };
    }
    Readiness::Unauthenticated {
        hint: hint.to_string(),
    }
}

/// `true` when a `codex exec` probe output carries an access-denied signal —
/// an arg0 temp-shim cleanup failure or a PATH-alias resolution denial. These
/// mean the nested executor will fail to launch even though `codex` is on PATH.
fn codex_probe_shows_access_denied(probe_output: &str) -> bool {
    let lower = probe_output.to_ascii_lowercase();
    const DENIAL_MARKERS: &[&str] = &[
        "access is denied",
        "permission denied",
        "operation not permitted",
        "os error 5",
    ];
    DENIAL_MARKERS.iter().any(|m| lower.contains(m))
}

/// Codex-specific readiness classifier (pure): an access-denied probe warning
/// (arg0 cleanup / PATH alias) yields a degraded readiness even when a token or
/// local state is present, because the nested executor will fail to launch.
/// Otherwise it falls back to the standard token → `Ready`, local-state →
/// `Unknown` (allow), neither → `Unauthenticated` ladder.
fn codex_readiness_from(has_token: bool, has_local_state: bool, probe_output: &str) -> Readiness {
    if codex_probe_shows_access_denied(probe_output) {
        return Readiness::Unknown {
            message: "codex nested-executor readiness DEGRADED: the `codex exec` probe reported an \
                      access-denied warning (arg0 temp-shim cleanup or PATH-alias resolution). The \
                      nested executor may fail to launch even though `codex` is on PATH. Remediation: \
                      ensure the temp directory is writable and PATH resolves the real `codex` binary \
                      (not a restricted alias), or run codex with write approval outside the sandbox"
                .to_string(),
        };
    }
    if has_token {
        return Readiness::Ready;
    }
    if has_local_state {
        return Readiness::Unknown {
            message:
                "codex has local state under ~/.codex but headless authentication could not be \
                      confirmed without a dedicated status command; allowing dispatch"
                    .to_string(),
        };
    }
    Readiness::Unauthenticated {
        hint: "run `codex login`, or set OPENAI_API_KEY for headless use".to_string(),
    }
}

/// Best-effort nested-Codex probe: run `codex exec --help` and return the
/// combined stdout+stderr. Returns an empty string when the probe cannot run —
/// classification then falls through to the token/local-state ladder.
fn codex_probe_output() -> String {
    match Command::new("codex").args(["exec", "--help"]).output() {
        Ok(out) => {
            let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&out.stderr));
            combined
        }
        Err(_) => String::new(),
    }
}

/// agy-specific readiness: recognises both `~/.agy/` (Linux/macOS) and
/// `~/.gemini/antigravity-cli/` (Windows OAuth state) as valid local auth.
/// Mirrors `copilot_readiness_from` — pure fn, injected booleans, no real fs.
fn agy_readiness_from(
    has_token: bool,
    has_agy_dir: bool,
    has_gemini_antigravity_dir: bool,
) -> Readiness {
    if has_token {
        return Readiness::Ready;
    }
    if has_agy_dir || has_gemini_antigravity_dir {
        return Readiness::Unknown {
            message: "agy has local auth state but headless authentication could not be confirmed \
                      without a dedicated status command; allowing dispatch"
                .to_string(),
        };
    }
    Readiness::Unauthenticated {
        hint: "authenticate the Antigravity/Gemini CLI (`agy auth login`), or set \
               GEMINI_API_KEY for headless use"
            .to_string(),
    }
}

fn copilot_readiness_from(has_token: bool, has_local_state: bool) -> Readiness {
    if has_token {
        return Readiness::Ready;
    }

    if has_local_state {
        return Readiness::Unknown {
            message: "copilot has local state under ~/.copilot but headless authentication could not be confirmed without a dedicated status command; allowing dispatch".to_string(),
        };
    }

    Readiness::Unauthenticated {
        hint: "run `copilot login` or set `COPILOT_GITHUB_TOKEN` / `GH_TOKEN` for headless use"
            .to_string(),
    }
}

/// Build a [`Command`] for `executor`, resolving Windows script shims that
/// `CreateProcess` cannot launch directly.
///
/// On Windows, an executor installed as a `.cmd`/`.bat`/`.ps1` shim (e.g.
/// npm's `codex.cmd`) cannot be spawned by `Command::new("codex")`, because
/// `CreateProcess` only appends `.exe` when no extension is given. This caused
/// `spawn failed: program not found` even though `where codex` succeeded.
///
/// This helper resolves the full path (preferring `.exe`, then `.cmd`/`.bat`,
/// then `.ps1`) and wraps shim scripts through their interpreter so stdin is
/// still piped through to the underlying tool. On non-Windows it is a direct
/// `Command::new(executor)`.
fn build_command(executor: &str, args: &[String]) -> Command {
    #[cfg(target_os = "windows")]
    {
        if let Some(resolved) = resolve_windows_executable(executor) {
            let lower = resolved.to_ascii_lowercase();
            if lower.ends_with(".cmd") || lower.ends_with(".bat") {
                let mut cmd = Command::new("cmd");
                cmd.arg("/C").arg(&resolved).args(args);
                return cmd;
            } else if lower.ends_with(".ps1") {
                let mut cmd = Command::new("powershell");
                cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                    .arg(&resolved)
                    .args(args);
                return cmd;
            } else {
                // .exe / .com or other directly-launchable image.
                let mut cmd = Command::new(&resolved);
                cmd.args(args);
                return cmd;
            }
        }
    }
    let mut cmd = Command::new(executor);
    cmd.args(args);
    cmd
}

/// Resolve `name` to a launchable full path on Windows via `where`, preferring
/// directly-runnable images over shell-script shims.
///
/// `where codex` may return several lines — the extensionless Bash shim, plus
/// `codex.cmd` and `codex.ps1`. We must skip the extensionless shim (Rust
/// cannot exec it) and prefer, in order: `.exe`, `.com`, `.cmd`, `.bat`, `.ps1`.
#[cfg(target_os = "windows")]
fn resolve_windows_executable(name: &str) -> Option<String> {
    let output = Command::new("where").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let candidates: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    const PRIORITY: &[&str] = &[".exe", ".com", ".cmd", ".bat", ".ps1"];
    for ext in PRIORITY {
        if let Some(found) = candidates
            .iter()
            .find(|c| c.to_ascii_lowercase().ends_with(ext))
        {
            return Some(found.clone());
        }
    }
    // Fallback: first candidate carrying any extension (skip extensionless shims).
    candidates
        .into_iter()
        .find(|c| std::path::Path::new(c).extension().is_some())
}

/// The strongest containment-based kill available for timeout cleanup, ahead
/// of [`kill_process_tree`]'s own kill (which becomes the Windows-only
/// fallback — see `spawn_executor`'s timeout branch).
enum PrimaryTermination {
    #[cfg(target_os = "windows")]
    WindowsJob(windows_containment::JobHandle),
    /// Unix (and any other target): the process-group `kill_process_tree`
    /// already targets via negative PGID is itself the primary termination —
    /// no separate handle is needed.
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    ProcessTree,
}

impl PrimaryTermination {
    /// Fire primary termination. Best-effort; the caller confirms exit via
    /// the child's own wait channel, not this call's return value.
    fn terminate(&self, pid: u32) {
        match self {
            #[cfg(target_os = "windows")]
            PrimaryTermination::WindowsJob(job) => windows_containment::terminate_job(*job),
            PrimaryTermination::ProcessTree => kill_process_tree(pid),
        }
    }
}

/// Closes the Windows Job handle exactly once, on every code path that drops
/// `PrimaryTermination` — the normal-completion path just as much as the
/// timeout-cleanup path. A manual `close()` call would be easy to forget on
/// one of `spawn_executor`'s several match arms and silently leak the handle
/// again (the exact leak this bounded-cleanup rework exists to fix); `Drop`
/// cannot be skipped.
impl Drop for PrimaryTermination {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        if let PrimaryTermination::WindowsJob(job) = self {
            windows_containment::close_job(*job);
        }
    }
}

/// Spawn `command`, contained before it ever executes: on Windows, suspended
/// and assigned to a kill-on-close Job Object, resumed only after assignment
/// succeeds; on Unix, placed in its own process group. Any containment
/// failure terminates and reaps the child before returning the error.
fn spawn_contained(
    command: &mut Command,
) -> std::io::Result<(std::process::Child, PrimaryTermination)> {
    #[cfg(target_os = "windows")]
    {
        let (child, job) = windows_containment::spawn_contained(command)?;
        Ok((child, PrimaryTermination::WindowsJob(job)))
    }
    #[cfg(unix)]
    {
        let child = unix_containment::spawn_contained(command)?;
        Ok((child, PrimaryTermination::ProcessTree))
    }
    #[cfg(not(any(target_os = "windows", unix)))]
    {
        Ok((command.spawn()?, PrimaryTermination::ProcessTree))
    }
}

/// Place each spawned executor in its own dedicated process group before
/// execution, so [`kill_process_tree`] can target the whole group via a
/// negative PGID instead of only the direct child. Deliberate session/process-
/// group breakaway by the executor itself is outside this containment's
/// guarantee — only the group formed at spawn time is covered.
#[cfg(unix)]
mod unix_containment {
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command};

    /// `process_group(0)` (stable std API since Rust 1.64) starts the child in
    /// a brand-new process group whose PGID equals the child's own PID — no
    /// unsafe FFI required, unlike the Windows Job Object path.
    pub(super) fn spawn_contained(command: &mut Command) -> std::io::Result<Child> {
        command.process_group(0).spawn()
    }
}

#[cfg(target_os = "windows")]
mod windows_containment {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use std::process::{Child, Command};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenThread, ResumeThread, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME,
    };

    /// Raw Job Object handle, threaded back to the caller so timeout cleanup
    /// can invoke [`terminate_job`] as primary termination and [`close_job`]
    /// once cleanup is fully resolved — never closed early, since
    /// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` would race-kill the child the
    /// moment the last handle closes.
    pub(super) type JobHandle = HANDLE;

    /// Spawn `command` suspended (`CREATE_SUSPENDED`), assign it to a
    /// kill-on-close Job Object, then resume its initial thread only after
    /// assignment succeeds. Any failure (Job creation, spawn, assignment,
    /// initial-thread lookup, resume) terminates and reaps the child —
    /// containment fails closed, never leaving a runnable unconfirmed child.
    /// Returns the Job handle alongside the child; the caller owns its
    /// lifetime (see [`terminate_job`], [`close_job`]).
    pub(super) fn spawn_contained(command: &mut Command) -> std::io::Result<(Child, JobHandle)> {
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        if let Err(e) = set_kill_on_close(job) {
            unsafe { CloseHandle(job) };
            return Err(e);
        }

        let mut child = match command.creation_flags(CREATE_SUSPENDED).spawn() {
            Ok(c) => c,
            Err(e) => {
                unsafe { CloseHandle(job) };
                return Err(e);
            }
        };

        let process_handle = child.as_raw_handle() as HANDLE;
        if unsafe { AssignProcessToJobObject(job, process_handle) } == 0 {
            let e = std::io::Error::last_os_error();
            terminate_and_reap(&mut child);
            unsafe { CloseHandle(job) };
            return Err(e);
        }

        if let Err(e) = resume_initial_thread(child.id()) {
            terminate_and_reap(&mut child);
            unsafe { CloseHandle(job) };
            return Err(e);
        }

        Ok((child, job))
    }

    /// Primary timeout termination: kill every process in the Job immediately
    /// (does not require closing the handle, unlike relying on kill-on-close).
    /// Best-effort — the caller confirms exit via the child's own wait channel.
    pub(super) fn terminate_job(job: JobHandle) {
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;
        unsafe {
            TerminateJobObject(job, 1);
        }
    }

    /// Close the Job handle once cleanup (confirmed or unconfirmed) is fully
    /// resolved. Safe to call exactly once, after the caller is done with it.
    pub(super) fn close_job(job: JobHandle) {
        unsafe { CloseHandle(job) };
    }

    fn set_kill_on_close(job: HANDLE) -> std::io::Result<()> {
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// Terminate and reap `child`, best-effort — used only on a containment
    /// failure path where the child must not be left running or zombied.
    fn terminate_and_reap(child: &mut Child) {
        let _ = child.kill();
        let _ = child.wait();
    }

    /// Locate `pid`'s initial (first-created) thread via a ToolHelp snapshot
    /// and resume it. A suspended-launched process has exactly one thread
    /// until it resumes, so the first (and only) `TH32CS_SNAPTHREAD` entry
    /// owned by `pid` is always the right one.
    fn resume_initial_thread(pid: u32) -> std::io::Result<()> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }

        let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut found_tid = None;
        let mut has_entry = unsafe { Thread32First(snapshot, &mut entry) } != 0;
        while has_entry {
            if entry.th32OwnerProcessID == pid {
                found_tid = Some(entry.th32ThreadID);
                break;
            }
            has_entry = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
        }
        unsafe { CloseHandle(snapshot) };

        let tid = found_tid.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no initial thread found for pid {pid}"),
            )
        })?;

        let thread_handle = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, tid) };
        if thread_handle.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let resumed = unsafe { ResumeThread(thread_handle) };
        unsafe { CloseHandle(thread_handle) };
        if resumed == u32::MAX {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

/// Kill a process tree rooted at `pid`.
fn kill_process_tree(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output();
    }
    #[cfg(unix)]
    {
        // `spawn_contained` placed this child in its own process group with
        // PGID == its PID (`process_group(0)`), so the negative PID targets
        // the whole group — every process the executor spawned, not just the
        // direct child. Deliberate group/session breakaway by the executor is
        // outside this guarantee (see `unix_containment`).
        let pgid = pid as libc::pid_t;
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
    }
    #[cfg(not(any(target_os = "windows", unix)))]
    {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
    }
}

/// Build the collision-safe log file path:
/// `<log_dir>/<epoch_secs>-<subsec_nanos>-<attempt_seq>-<task>-<phase>-<executor>.log`
///
/// Two attempts sharing the same `task_id`/`phase`/`executor` — even triggered in
/// rapid back-to-back retries — never collide: `attempt_seq` is a per-process
/// monotonic counter that always advances, independent of wall-clock resolution.
fn build_log_path(log_dir: &Path, task_id: &str, phase: &str, executor: &str) -> PathBuf {
    let ts = unique_attempt_token();
    let safe_task = sanitize_log_component(task_id);
    let safe_phase = sanitize_log_component(phase);
    let safe_executor = sanitize_log_component(executor);
    let filename = format!("{ts}-{safe_task}-{safe_phase}-{safe_executor}.log");
    log_dir.join(filename)
}

/// Generate a collision-safe unique token: epoch seconds, sub-second nanoseconds,
/// and a per-process monotonic counter (`AtomicU64`, process-lifetime, starts at 0).
/// The counter alone guarantees uniqueness within one process; across concurrent
/// processes (each counter starts at 0) `subsec_nanos` is the separator. The
/// timestamp components also keep the filename sortable and human-inspectable.
fn unique_attempt_token() -> String {
    static ATTEMPT_COUNTER: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seq = ATTEMPT_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{:010}-{:09}-{:06}", now.as_secs(), now.subsec_nanos(), seq)
}

/// Strip line breaks from a value interpolated into a log header field.
///
/// The header is a run of newline-delimited `key: value` lines terminated by the
/// `---STDOUT---` marker, and the evidence reader resolves each key from those
/// lines. A value carrying a raw `\n` injects an extra, caller-chosen header line.
/// That matters most for `session_id`, which is scraped from executor stdout and
/// emitted after `terminal_state`: an embedded newline could otherwise forge a
/// second `terminal_state` line and override the very state the evidence gate
/// exists to establish. Values stay otherwise intact (unlike
/// [`sanitize_log_component`], which is for filename components).
fn sanitize_header_value(value: &str) -> String {
    value.replace(['\n', '\r'], " ")
}

/// Render the optional ` contract=<path> contract_source=<source>` suffix shared
/// by every `Dispatch:` marker line printed from `run.rs` (successful and
/// degraded alike). Empty when `provenance` is `None`, keeping raw/direct
/// dispatch markers byte-compatible with pre-provenance output. Reuses
/// [`sanitize_header_value`] so an embedded `\n`/`\r` in the contract path or
/// source can never forge a second `--- GAL DISPATCH ---` banner or `Dispatch:`
/// line within the printed marker.
pub(crate) fn render_dispatch_marker_suffix(provenance: &Option<ContractProvenance>) -> String {
    match provenance {
        None => String::new(),
        Some(p) => format!(
            " contract={} contract_source={}",
            sanitize_header_value(&p.path.to_string_lossy()),
            sanitize_header_value(&p.source),
        ),
    }
}

/// Render the routed variant of the `Dispatch:` marker suffix: a sanitized
/// ` effort=<value>` field immediately before the existing contract provenance
/// suffix, for pipeline-created dispatches only (`provenance` is `Some`).
/// Compute once after route resolution (an executor entry was found for the
/// phase's role) and reuse it for every later success/degrade marker in that
/// run, so every marker reports the same effort the run actually attempted.
/// Falls back to `(default)` when the route sets no `effort`, matching the
/// OFFLOAD block's `EFFORT` field. Returns the plain (unrouted)
/// [`render_dispatch_marker_suffix`] output when `provenance` is `None`, which
/// keeps raw/direct dispatch and no-routing markers byte-identical: no `effort`
/// field is ever added ahead of a route being resolved. Reuses
/// [`sanitize_header_value`] so an embedded `\n`/`\r` in an (even invalid)
/// routed effort value can never forge a second `Dispatch:` marker line.
pub(crate) fn render_routed_dispatch_marker_suffix(
    effort: Option<&str>,
    provenance: &Option<ContractProvenance>,
) -> String {
    match provenance {
        None => render_dispatch_marker_suffix(provenance),
        Some(p) => {
            let effort_display = match effort {
                Some(e) if !e.is_empty() => sanitize_header_value(e),
                _ => "(default)".to_string(),
            };
            format!(
                " effort={effort_display} contract={} contract_source={}",
                sanitize_header_value(&p.path.to_string_lossy()),
                sanitize_header_value(&p.source),
            )
        }
    }
}

/// Render the optional `contract`/`contract_source` header lines shared by
/// [`write_started_marker`] and [`write_log`]. Returns an empty string when
/// `provenance` is `None`, keeping legacy (raw/direct dispatch) headers
/// byte-compatible with pre-provenance logs.
fn render_contract_header(provenance: &Option<ContractProvenance>) -> String {
    match provenance {
        None => String::new(),
        Some(p) => format!(
            "contract:        {}\ncontract_source: {}\n",
            sanitize_header_value(&p.path.to_string_lossy()),
            sanitize_header_value(&p.source),
        ),
    }
}

fn sanitize_log_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '_' | '-' => c,
            _ => '-',
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Durably write the pre-spawn `started` attempt marker at `path`, before any
/// availability check or process spawn.
///
/// Carries the same header shape as [`write_log`]'s terminal content (so a later
/// consumer parses both with one header reader) but with `terminal_state: started`,
/// no `timestamp_end`/`duration_ms`/`exit_code`/`session_id` yet, and empty
/// stdout/stderr sections. Marker-write failure (e.g. an uncreatable log
/// directory) is propagated to the caller, which must NOT proceed to spawn.
fn write_started_marker(
    path: &Path,
    cfg: &SpawnConfig,
    start_ts: &str,
) -> Result<(), DispatchError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(DispatchError::LogDirCreate)?;
    }

    let git_branch = sanitize_header_value(&git_current_branch());
    let git_head = sanitize_header_value(&git_head_short());
    let contract_header = render_contract_header(&cfg.contract_provenance);

    let content = format!(
        "GAL-DISPATCH-LOG v1\n\
         timestamp_start: {start_ts}\n\
         timestamp_end:   \n\
         duration_ms:     \n\
         executor:        {executor}\n\
         phase:           {phase}\n\
         task_id:         {task_id}\n\
         git_branch:      {git_branch}\n\
         git_head:        {git_head}\n\
         exit_code:       none\n\
         actual_model:    {model}\n\
         {contract_header}\
         terminal_state:  started\n\
         session_id:      none\n\
         ---STDOUT---\n\
         \n\
         ---STDERR---\n\
         \n",
        executor = sanitize_header_value(&cfg.executor),
        phase = sanitize_header_value(&cfg.phase),
        task_id = sanitize_header_value(&cfg.task_id),
        model = sanitize_header_value(&cfg.actual_model),
    );

    std::fs::write(path, &content).map_err(DispatchError::LogWrite)?;
    Ok(())
}

/// Write the durable executor log file.
fn write_log(
    path: &Path,
    cfg: &SpawnConfig,
    start_ts: &str,
    end_ts: &str,
    duration_ms: u64,
    result: &DispatchResult,
) -> Result<(), DispatchError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(DispatchError::LogDirCreate)?;
    }

    let git_branch = git_current_branch();
    let git_head = git_head_short();
    let exit_str = result
        .exit_code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "none".to_string());
    let session_str = sanitize_header_value(result.session_id.as_deref().unwrap_or("none"));
    let git_branch = sanitize_header_value(&git_branch);
    let git_head = sanitize_header_value(&git_head);
    let contract_header = render_contract_header(&cfg.contract_provenance);

    let content = format!(
        "GAL-DISPATCH-LOG v1\n\
         timestamp_start: {start_ts}\n\
         timestamp_end:   {end_ts}\n\
         duration_ms:     {duration_ms}\n\
         executor:        {executor}\n\
         phase:           {phase}\n\
         task_id:         {task_id}\n\
         git_branch:      {git_branch}\n\
         git_head:        {git_head}\n\
         exit_code:       {exit_str}\n\
         actual_model:    {model}\n\
         {contract_header}\
         terminal_state:  {state}\n\
         session_id:      {session_str}\n\
         ---STDOUT---\n\
         {stdout}\n\
         ---STDERR---\n\
         {stderr}\n",
        executor = sanitize_header_value(&cfg.executor),
        phase = sanitize_header_value(&cfg.phase),
        task_id = sanitize_header_value(&cfg.task_id),
        model = sanitize_header_value(&cfg.actual_model),
        state = result.terminal_state,
        stdout = result.stdout,
        stderr = result.stderr,
    );

    std::fs::write(path, &content).map_err(DispatchError::LogWrite)?;
    Ok(())
}

/// Generic session id extractor for use in the durable log header.
///
/// Tries JSON field extraction first (covers claude, opencode, copilot whose stdout
/// is a JSON object or NDJSON stream), then falls back to a UUID-shape scan.
/// Tool-specific adapters may further refine this for the `Dispatch:` marker.
fn extract_session_id_generic(stdout: &str) -> Option<String> {
    // Pass 1: Try to parse each line as JSON and look for known session id field names.
    // This covers: claude (`session_id`), opencode (`sessionID`), copilot (`result.sessionId`),
    // codex (`thread_id`) in a unified way so the log header gets the correct id.
    const JSON_FIELDS: &[&str] = &["session_id", "sessionID", "sessionId", "thread_id"];
    for line in stdout.lines() {
        let line = line.trim();
        if !line.is_empty() && line.starts_with('{') {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                for &field in JSON_FIELDS {
                    if let Some(id) = v.get(field).and_then(|f| f.as_str()) {
                        if !id.is_empty() {
                            return Some(id.to_string());
                        }
                    }
                }
                // Try one level of nesting: result.sessionId (copilot)
                if let Some(id) = v
                    .get("result")
                    .and_then(|r| r.get("sessionId"))
                    .and_then(|f| f.as_str())
                {
                    if !id.is_empty() {
                        return Some(id.to_string());
                    }
                }
            }
        }
    }

    // Pass 2: UUID-shape token scan (fallback for tools whose output is plain text).
    for word in stdout.split_whitespace() {
        let w = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
        if looks_like_uuid(w) {
            return Some(w.to_string());
        }
    }
    None
}

fn looks_like_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts[0].len() == 8
        && parts[1].len() == 4
        && parts[2].len() == 4
        && parts[3].len() == 4
        && parts[4].len() == 12
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

fn utc_now_iso8601() -> String {
    // Without chrono: use systemtime epoch seconds as a rough ISO-8601 approximation.
    // Full ISO-8601 is available via chrono (added when session ids are logged).
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("epoch+{secs}s") // placeholder until chrono is added
}

fn git_current_branch() -> String {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn git_head_short() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(all(test, target_os = "windows"))]
mod windows_containment_tests {
    use super::windows_containment::{close_job, spawn_contained};
    use std::process::{Command, Stdio};

    fn cmd(args: &[&str]) -> Command {
        let mut c = Command::new("cmd");
        c.arg("/C")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    }

    #[test]
    fn contained_child_runs_to_completion_with_correct_exit_code() {
        let (mut child, job) = spawn_contained(&mut cmd(&["exit", "0"])).unwrap();
        let status = child.wait().unwrap();
        close_job(job);
        assert!(status.success());
    }

    #[test]
    fn contained_child_preserves_nonzero_exit_code() {
        let (mut child, job) = spawn_contained(&mut cmd(&["exit", "7"])).unwrap();
        let status = child.wait().unwrap();
        close_job(job);
        assert_eq!(status.code(), Some(7));
    }

    #[test]
    fn contained_child_preserves_stdout() {
        let (mut child, job) = spawn_contained(&mut cmd(&["echo", "contained-hello"])).unwrap();
        let mut out = String::new();
        std::io::Read::read_to_string(child.stdout.as_mut().unwrap(), &mut out).unwrap();
        let status = child.wait().unwrap();
        close_job(job);
        assert!(status.success());
        assert!(out.contains("contained-hello"));
    }

    #[test]
    fn spawn_failure_on_nonexistent_executable_returns_err_and_leaves_no_child() {
        let mut c = Command::new("gal-dispatch-nonexistent-executable-xyz-12345");
        c.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        assert!(spawn_contained(&mut c).is_err());
    }
}

#[cfg(all(test, unix))]
mod unix_containment_tests {
    use super::unix_containment::spawn_contained;
    use std::process::{Command, Stdio};

    fn sh(script: &str) -> Command {
        let mut c = Command::new("sh");
        c.arg("-c")
            .arg(script)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    }

    #[test]
    fn contained_child_runs_to_completion_with_correct_exit_code() {
        let mut child = spawn_contained(&mut sh("exit 0")).unwrap();
        let status = child.wait().unwrap();
        assert!(status.success());
    }

    #[test]
    fn contained_child_preserves_nonzero_exit_code() {
        let mut child = spawn_contained(&mut sh("exit 7")).unwrap();
        let status = child.wait().unwrap();
        assert_eq!(status.code(), Some(7));
    }

    #[test]
    fn contained_child_preserves_stdout() {
        let mut child = spawn_contained(&mut sh("echo contained-hello")).unwrap();
        let mut out = String::new();
        std::io::Read::read_to_string(child.stdout.as_mut().unwrap(), &mut out).unwrap();
        let status = child.wait().unwrap();
        assert!(status.success());
        assert!(out.contains("contained-hello"));
    }

    #[test]
    fn contained_child_is_the_leader_of_its_own_process_group() {
        // process_group(0) starts a new group whose PGID equals the child's
        // own PID — verify that invariant directly via `sh -c 'echo $$'`
        // (the shell's own PID, which is also the PGID leader by construction).
        let mut child = spawn_contained(&mut sh("echo $$")).unwrap();
        let mut out = String::new();
        std::io::Read::read_to_string(child.stdout.as_mut().unwrap(), &mut out).unwrap();
        child.wait().unwrap();
        let reported_pid: u32 = out.trim().parse().expect("pid line");
        assert_eq!(reported_pid, child.id());
    }

    #[test]
    fn kill_process_tree_kills_the_whole_group_not_just_the_direct_child() {
        // Spawn a shell that backgrounds a long-running child and sleeps
        // itself; kill_process_tree targets the negative PGID, so the
        // backgrounded grandchild must die too, not just the shell.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let marker = std::env::temp_dir().join(format!(
            "gal-dispatch-pgroup-test-{}-{nanos}",
            std::process::id(),
        ));
        let script = format!(
            "(sleep 30 && touch {marker:?}) & sleep 30",
            marker = marker.display()
        );
        let mut child = spawn_contained(&mut sh(&script)).unwrap();
        let pid = child.id();
        std::thread::sleep(std::time::Duration::from_millis(200));
        super::kill_process_tree(pid);
        let _ = child.wait();
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(
            !marker.exists(),
            "backgrounded grandchild ran to completion (touched the marker) — \
             kill_process_tree did not reach the whole process group"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(
        tmp: &TempDir,
        executor: &str,
        args: Vec<&str>,
        spec: &str,
        timeout_secs: u64,
    ) -> SpawnConfig {
        SpawnConfig {
            executor: executor.to_string(),
            executor_args: args.iter().map(|s| s.to_string()).collect(),
            spec: spec.to_string(),
            workdir: tmp.path().to_path_buf(),
            timeout_secs,
            task_id: "T-TEST".to_string(),
            phase: "implement".to_string(),
            actual_model: "test-model".to_string(),
            log_dir: tmp.path().join("executor-logs"),
            receipt_path: None,
            contract_provenance: None,
        }
    }

    #[test]
    fn absolute_existing_executable_is_available() {
        let exe = std::env::current_exe().unwrap();
        assert!(is_available(exe.to_str().unwrap()));
    }

    #[test]
    fn log_path_sanitizes_absolute_executor_path() {
        let tmp = TempDir::new().unwrap();
        let path = build_log_path(
            tmp.path(),
            "T-TEST",
            "implement",
            r"C:\Program Files\Git\bin\sh.exe",
        );
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap();
        assert!(file_name.ends_with("T-TEST-implement-C--Program-Files-Git-bin-sh.exe.log"));
        assert!(!file_name.contains('\\'));
        assert_eq!(file_name.matches(':').count(), 0);
    }

    // echo mock → completed
    #[test]
    fn echo_exits_zero_gives_completed() {
        let tmp = TempDir::new().unwrap();

        #[cfg(target_os = "windows")]
        let (exe, args) = ("cmd", vec!["/C", "echo", "hello"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sh", vec!["-c", "echo hello"]);

        let cfg = test_config(&tmp, exe, args, "spec", 30);
        let result = spawn_executor(&cfg).unwrap();
        assert_eq!(
            result.terminal_state,
            TerminalState::Completed,
            "exit-0 process should give Completed before write-back check"
        );
        assert!(result.exit_code == Some(0));
    }

    // non-zero exit → disconnected-partial
    #[test]
    fn non_zero_exit_gives_disconnected_partial() {
        let tmp = TempDir::new().unwrap();

        #[cfg(target_os = "windows")]
        let (exe, args) = ("cmd", vec!["/C", "exit", "1"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sh", vec!["-c", "exit 1"]);

        let cfg = test_config(&tmp, exe, args, "spec", 30);
        let result = spawn_executor(&cfg).unwrap();
        assert_eq!(result.terminal_state, TerminalState::DisconnectedPartial);
    }

    // missing CLI → unavailable
    #[test]
    fn missing_executor_gives_unavailable() {
        let tmp = TempDir::new().unwrap();
        let cfg = test_config(&tmp, "nonexistent-executor-xyz-12345", vec![], "spec", 30);
        let result = spawn_executor(&cfg).unwrap();
        assert_eq!(result.terminal_state, TerminalState::Unavailable);
    }

    // timeout → timeout state
    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)] // Windows-first; Unix needs setsid for clean process group
    fn timeout_gives_timeout_state() {
        let tmp = TempDir::new().unwrap();

        // Sleep for 60s, but we timeout after 1s
        #[cfg(target_os = "windows")]
        let (exe, args) = ("ping", vec!["-n", "60", "127.0.0.1"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sleep", vec!["60"]);

        let cfg = test_config(&tmp, exe, args, "spec", 1);
        let result = spawn_executor(&cfg).unwrap();
        // A timeout now classifies by whether output was seen before the kill;
        // either timeout-* variant is a valid timeout outcome (ping's first
        // reply vs the 1s deadline is a race, so accept both).
        assert!(
            matches!(
                result.terminal_state,
                TerminalState::TimeoutNoOutput | TerminalState::TimeoutMidrun
            ),
            "expected a timeout-* state, got {:?}",
            result.terminal_state
        );
    }

    // Timeout cleanup must return within a bounded wall-clock ceiling — never
    // hang indefinitely joining drain threads on an unconfirmed exit.
    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)] // Windows-first; Unix needs setsid for clean process group
    fn timeout_cleanup_returns_within_bounded_wall_clock() {
        let tmp = TempDir::new().unwrap();

        #[cfg(target_os = "windows")]
        let (exe, args) = ("ping", vec!["-n", "60", "127.0.0.1"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sleep", vec!["60"]);

        let timeout_secs = 1;
        let cfg = test_config(&tmp, exe, args, "spec", timeout_secs);
        let start = std::time::Instant::now();
        let result = spawn_executor(&cfg).unwrap();
        let elapsed = start.elapsed();

        // Theoretical worst case: timeout_secs + primary grace period +
        // Windows-only fallback grace period, plus generous scheduling slack.
        let ceiling =
            Duration::from_secs(timeout_secs) + 2 * CLEANUP_GRACE_PERIOD + Duration::from_secs(5);
        assert!(
            elapsed < ceiling,
            "timeout cleanup took {elapsed:?}, expected under {ceiling:?} — \
             a hang here means cleanup is waiting unboundedly on drains/exit"
        );
        assert!(
            matches!(
                result.terminal_state,
                TerminalState::TimeoutNoOutput | TerminalState::TimeoutMidrun
            ),
            "expected a timeout-* state (confirmed-exit path), got {:?}",
            result.terminal_state
        );
    }

    // A multi-megabyte spec, delivered to a child that never reads stdin,
    // must not block the timeout clock — the stdin writer thread may sit
    // blocked on `write_all` forever (until the child is killed and its
    // stdin pipe closes), but the main dispatch path must never wait on it.
    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)] // Windows-first; Unix needs setsid for clean process group
    fn large_spec_to_child_that_never_reads_stdin_does_not_block_timeout() {
        let tmp = TempDir::new().unwrap();

        #[cfg(target_os = "windows")]
        let (exe, args) = ("ping", vec!["-n", "60", "127.0.0.1"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sleep", vec!["60"]);

        // Several MB — comfortably past any platform's default pipe buffer
        // (commonly 64 KB), so a synchronous write would have blocked.
        let big_spec = "x".repeat(8 * 1024 * 1024);
        let timeout_secs = 1;
        let cfg = test_config(&tmp, exe, args, &big_spec, timeout_secs);
        let start = std::time::Instant::now();
        let result = spawn_executor(&cfg).unwrap();
        let elapsed = start.elapsed();

        let ceiling =
            Duration::from_secs(timeout_secs) + 2 * CLEANUP_GRACE_PERIOD + Duration::from_secs(5);
        assert!(
            elapsed < ceiling,
            "timeout with an 8MB spec to a stdin-blind child took {elapsed:?}, \
             expected under {ceiling:?} — a hang here means the stdin writer \
             thread is blocking the timeout clock"
        );
        assert!(
            matches!(
                result.terminal_state,
                TerminalState::TimeoutNoOutput | TerminalState::TimeoutMidrun
            ),
            "expected a timeout-* state, got {:?}",
            result.terminal_state
        );
        assert!(
            result.stderr.contains("spec_bytes=8388608"),
            "expected spec_bytes in timeout evidence, got: {}",
            result.stderr
        );
        assert!(
            result.stderr.contains("stdin_delivery="),
            "expected stdin_delivery in timeout evidence, got: {}",
            result.stderr
        );
    }

    // Timeout evidence must state only what was observed — never an
    // unproven interactive-prompt/hang diagnosis — while keeping the stable
    // `timeout-no-output` compatibility token.
    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)] // Windows-first; Unix needs setsid for clean process group
    fn timeout_reason_is_observation_only() {
        let tmp = TempDir::new().unwrap();

        #[cfg(target_os = "windows")]
        let (exe, args) = ("ping", vec!["-n", "60", "127.0.0.1"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sleep", vec!["60"]);

        let cfg = test_config(&tmp, exe, args, "spec", 1);
        let result = spawn_executor(&cfg).unwrap();

        assert!(
            matches!(
                result.terminal_state,
                TerminalState::TimeoutNoOutput | TerminalState::TimeoutMidrun
            ),
            "expected a timeout-* state, got {:?}",
            result.terminal_state
        );
        // The stable compatibility token is still produced by `as_str()`.
        assert!(
            result.terminal_state.as_str() == "timeout-no-output"
                || result.terminal_state.as_str() == "timeout-midrun"
        );
        let lower = result.stderr.to_ascii_lowercase();
        assert!(
            !lower.contains("interactive") && !lower.contains("prompt") && !lower.contains("hung"),
            "timeout reason must be observation-only, got: {}",
            result.stderr
        );
        assert!(
            result.stderr.contains("stdin_delivery=") && result.stderr.contains("spec_bytes="),
            "expected observed stdin_delivery/spec_bytes evidence, got: {}",
            result.stderr
        );
    }

    // log header present and contains required fields
    #[test]
    fn log_header_is_complete() {
        let tmp = TempDir::new().unwrap();

        #[cfg(target_os = "windows")]
        let (exe, args) = ("cmd", vec!["/C", "echo", "hello"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sh", vec!["-c", "echo hello"]);

        let cfg = test_config(&tmp, exe, args, "my-spec", 30);
        let result = spawn_executor(&cfg).unwrap();

        let log_content = std::fs::read_to_string(&result.log_path).unwrap();
        assert!(
            log_content.contains("GAL-DISPATCH-LOG v1"),
            "missing log header"
        );
        assert!(
            log_content.contains("timestamp_start:"),
            "missing timestamp_start"
        );
        assert!(
            log_content.contains("timestamp_end:"),
            "missing timestamp_end"
        );
        assert!(log_content.contains("duration_ms:"), "missing duration_ms");
        assert!(log_content.contains("executor:"), "missing executor");
        assert!(log_content.contains("phase:"), "missing phase");
        assert!(log_content.contains("task_id:"), "missing task_id");
        assert!(log_content.contains("git_branch:"), "missing git_branch");
        assert!(log_content.contains("git_head:"), "missing git_head");
        assert!(log_content.contains("exit_code:"), "missing exit_code");
        assert!(
            log_content.contains("actual_model:"),
            "missing actual_model"
        );
        assert!(
            log_content.contains("terminal_state:"),
            "missing terminal_state"
        );
        assert!(log_content.contains("session_id:"), "missing session_id");
        assert!(
            log_content.contains("---STDOUT---"),
            "missing STDOUT section"
        );
        assert!(
            log_content.contains("---STDERR---"),
            "missing STDERR section"
        );
    }

    #[test]
    fn terminal_state_strings_are_correct() {
        assert_eq!(TerminalState::Completed.as_str(), "completed");
        assert_eq!(TerminalState::NoReceipt.as_str(), "no-receipt");
        assert_eq!(TerminalState::Timeout.as_str(), "timeout");
        assert_eq!(
            TerminalState::DisconnectedPartial.as_str(),
            "disconnected-partial"
        );
        assert_eq!(TerminalState::Unavailable.as_str(), "unavailable");
    }

    #[test]
    fn uuid_detection_works() {
        assert!(looks_like_uuid("f5c9f450-c44d-41ba-9e7e-e0bfcb19e105"));
        assert!(!looks_like_uuid("not-a-uuid"));
        assert!(!looks_like_uuid("too-short-1234"));
        assert!(!looks_like_uuid(""));
    }

    // receipt file written by executor → Completed
    #[test]
    fn receipt_present_and_nonempty_gives_completed() {
        let tmp = TempDir::new().unwrap();
        let receipt = tmp.path().join("receipt.txt");
        std::fs::write(&receipt, "written by executor").unwrap();

        #[cfg(target_os = "windows")]
        let (exe, args) = ("cmd", vec!["/C", "echo", "ok"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sh", vec!["-c", "echo ok"]);

        let mut cfg = test_config(&tmp, exe, args, "spec", 30);
        cfg.receipt_path = Some(receipt);
        let result = spawn_executor(&cfg).unwrap();
        assert_eq!(
            result.terminal_state,
            TerminalState::Completed,
            "exit-0 + non-empty receipt file should give Completed"
        );
    }

    // executor exits 0 but receipt file missing → NoReceipt
    #[test]
    fn receipt_missing_gives_no_receipt() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("nonexistent-receipt.txt");

        #[cfg(target_os = "windows")]
        let (exe, args) = ("cmd", vec!["/C", "echo", "ok"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sh", vec!["-c", "echo ok"]);

        let mut cfg = test_config(&tmp, exe, args, "spec", 30);
        cfg.receipt_path = Some(missing);
        let result = spawn_executor(&cfg).unwrap();
        assert_eq!(
            result.terminal_state,
            TerminalState::NoReceipt,
            "exit-0 but missing receipt file should give NoReceipt"
        );
    }

    // ── agy readiness ───────────────────────────────────────────────────────

    #[test]
    fn codex_readiness_flags_arg0_or_path_alias_access_denied() {
        // arg0 temp-shim cleanup denial → DEGRADED even with token + local state.
        let arg0 = codex_readiness_from(
            true,
            true,
            "warning: failed to clean up arg0 shim: Access is denied. (os error 5)",
        );
        assert!(
            matches!(&arg0, Readiness::Unknown { message } if message.contains("DEGRADED")),
            "arg0 access-denied must degrade readiness, got {arg0:?}"
        );

        // PATH-alias resolution denial → DEGRADED.
        let path_alias = codex_readiness_from(
            true,
            false,
            "error: could not resolve `codex` via PATH alias: permission denied",
        );
        assert!(
            matches!(&path_alias, Readiness::Unknown { message } if message.contains("DEGRADED")),
            "PATH-alias access-denied must degrade readiness, got {path_alias:?}"
        );

        // A clean probe with a token is Ready; with neither token nor state,
        // Unauthenticated (the access-denied heuristic must not false-positive).
        assert_eq!(
            codex_readiness_from(true, true, "Usage: codex exec [OPTIONS]"),
            Readiness::Ready
        );
        assert!(matches!(
            codex_readiness_from(false, false, "Usage: codex exec"),
            Readiness::Unauthenticated { .. }
        ));
        // Local state without token and a clean probe → Unknown (allow dispatch).
        assert!(matches!(
            codex_readiness_from(false, true, "Usage: codex exec"),
            Readiness::Unknown { .. }
        ));
    }

    #[test]
    fn agy_readiness_ready_when_token_present() {
        assert_eq!(agy_readiness_from(true, false, false), Readiness::Ready);
        assert_eq!(agy_readiness_from(true, true, false), Readiness::Ready);
        assert_eq!(agy_readiness_from(true, false, true), Readiness::Ready);
        assert_eq!(agy_readiness_from(true, true, true), Readiness::Ready);
    }

    #[test]
    fn agy_readiness_unknown_when_agy_dir_exists_without_token() {
        let r = agy_readiness_from(false, true, false);
        assert!(
            matches!(r, Readiness::Unknown { .. }),
            "~/.agy/ present should give Unknown (allow dispatch), got {r:?}"
        );
    }

    #[test]
    fn agy_readiness_unknown_when_gemini_antigravity_dir_exists_without_token() {
        let r = agy_readiness_from(false, false, true);
        assert!(
            matches!(r, Readiness::Unknown { .. }),
            "~/.gemini/antigravity-cli/ present should give Unknown (allow dispatch), got {r:?}"
        );
    }

    #[test]
    fn agy_readiness_unauthenticated_when_no_token_and_both_dirs_absent() {
        let r = agy_readiness_from(false, false, false);
        assert!(
            matches!(r, Readiness::Unauthenticated { .. }),
            "no token + no dirs should give Unauthenticated, got {r:?}"
        );
    }

    #[test]
    fn copilot_readiness_is_ready_when_token_present() {
        assert_eq!(copilot_readiness_from(true, false), Readiness::Ready);
    }

    #[test]
    fn copilot_readiness_is_unknown_when_local_state_exists_without_token() {
        assert_eq!(
            copilot_readiness_from(false, true),
            Readiness::Unknown {
                message: "copilot has local state under ~/.copilot but headless authentication could not be confirmed without a dedicated status command; allowing dispatch".to_string(),
            }
        );
    }

    #[test]
    fn copilot_readiness_is_unauthenticated_when_no_token_or_local_state() {
        assert_eq!(
            copilot_readiness_from(false, false),
            Readiness::Unauthenticated {
                hint: "run `copilot login` or set `COPILOT_GITHUB_TOKEN` / `GH_TOKEN` for headless use".to_string(),
            }
        );
    }

    #[test]
    fn unsupported_executor_defaults_to_unknown() {
        assert_eq!(
            executor_readiness("frobnicate"),
            Readiness::Unknown {
                message:
                    "executor 'frobnicate' has no dedicated readiness probe yet; allowing dispatch"
                        .to_string(),
            }
        );
    }

    #[test]
    fn classify_timeout_distinguishes_no_output_from_midrun() {
        assert_eq!(classify_timeout(false), TerminalState::TimeoutNoOutput);
        assert_eq!(classify_timeout(true), TerminalState::TimeoutMidrun);
        assert_eq!(TerminalState::TimeoutNoOutput.as_str(), "timeout-no-output");
        assert_eq!(TerminalState::TimeoutMidrun.as_str(), "timeout-midrun");
    }

    #[test]
    fn probe_readiness_unauthenticated_when_no_token_and_no_state() {
        // No env var set + a home subdir that does not exist → Unauthenticated.
        let r = probe_readiness(
            "test-exec",
            &["GAL_TEST_NO_SUCH_TOKEN_VAR_X9Z"],
            ".gal-no-such-state-dir-x9z",
            "do the thing",
        );
        assert_eq!(
            r,
            Readiness::Unauthenticated {
                hint: "do the thing".to_string()
            }
        );
    }

    #[test]
    fn codex_claude_agy_opencode_now_have_a_dedicated_probe() {
        // These four executors are routed through probe_readiness, so their
        // result is Ready / Unauthenticated / state-Unknown — NEVER the blanket
        // "no dedicated readiness probe yet" fallback. That fallback message is the
        // distinguishing marker of an unprobed executor.
        for exec in ["codex", "claude", "agy", "opencode"] {
            let r = executor_readiness(exec);
            if let Readiness::Unknown { message } = &r {
                assert!(
                    !message.contains("no dedicated readiness probe yet"),
                    "{exec} should be probed, got fallback: {message}"
                );
            }
        }
    }

    // executor exits 0 but receipt file is empty → NoReceipt (partial write)
    #[test]
    fn receipt_empty_gives_no_receipt() {
        let tmp = TempDir::new().unwrap();
        let receipt = tmp.path().join("empty.txt");
        std::fs::write(&receipt, "").unwrap(); // empty file

        #[cfg(target_os = "windows")]
        let (exe, args) = ("cmd", vec!["/C", "echo", "ok"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sh", vec!["-c", "echo ok"]);

        let mut cfg = test_config(&tmp, exe, args, "spec", 30);
        cfg.receipt_path = Some(receipt);
        let result = spawn_executor(&cfg).unwrap();
        assert_eq!(
            result.terminal_state,
            TerminalState::NoReceipt,
            "exit-0 but empty receipt file (partial write) should give NoReceipt"
        );
    }

    // no receipt_path configured → Completed (no check)
    #[test]
    fn no_receipt_path_skips_check() {
        let tmp = TempDir::new().unwrap();

        #[cfg(target_os = "windows")]
        let (exe, args) = ("cmd", vec!["/C", "echo", "ok"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sh", vec!["-c", "echo ok"]);

        let cfg = test_config(&tmp, exe, args, "spec", 30);
        assert!(cfg.receipt_path.is_none());
        let result = spawn_executor(&cfg).unwrap();
        assert_eq!(
            result.terminal_state,
            TerminalState::Completed,
            "no receipt_path → skip check → Completed"
        );
    }

    // verify_receipt unit tests
    #[test]
    fn verify_receipt_returns_true_for_nonempty_file() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("r.txt");
        std::fs::write(&f, "content").unwrap();
        assert!(verify_receipt(&f));
    }

    #[test]
    fn verify_receipt_returns_false_for_missing_file() {
        let tmp = TempDir::new().unwrap();
        assert!(!verify_receipt(&tmp.path().join("no-such-file.txt")));
    }

    #[test]
    fn verify_receipt_returns_false_for_empty_file() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("empty.txt");
        std::fs::write(&f, "").unwrap();
        assert!(!verify_receipt(&f));
    }

    // ── Session id extraction tests ─────────────────

    #[test]
    fn session_id_extracted_from_claude_json() {
        // Claude outputs a JSON object with `session_id` at root level
        let stdout = r#"{"type":"result","session_id":"f5c9f450-c44d-41ba-9e7e-e0bfcb19e105","duration_ms":902}"#;
        let id = extract_session_id_generic(stdout);
        assert_eq!(id.as_deref(), Some("f5c9f450-c44d-41ba-9e7e-e0bfcb19e105"));
    }

    #[test]
    fn session_id_extracted_from_opencode_ndjson() {
        // OpenCode streams NDJSON events; sessionID in one of the events
        let stdout = "{\"type\":\"info\"}\n{\"sessionID\":\"ses_abc123\",\"model\":\"gpt-4\"}\n";
        let id = extract_session_id_generic(stdout);
        assert_eq!(id.as_deref(), Some("ses_abc123"));
    }

    #[test]
    fn session_id_extracted_from_copilot_nested_result() {
        // Copilot has result.sessionId nested
        let stdout =
            r#"{"result":{"sessionId":"0f3e7af9-6cd2-4853-bf41-9e5a7a9b6df9","status":"ok"}}"#;
        let id = extract_session_id_generic(stdout);
        assert_eq!(id.as_deref(), Some("0f3e7af9-6cd2-4853-bf41-9e5a7a9b6df9"));
    }

    #[test]
    fn session_id_extracted_from_codex_thread_id() {
        // Codex NDJSON stream has thread_id in thread.started event
        let stdout = "{\"type\":\"thread.started\",\"thread_id\":\"019e92a3-afbb-7af1-9d42-5bd8f5b017ef\"}\n";
        let id = extract_session_id_generic(stdout);
        assert_eq!(id.as_deref(), Some("019e92a3-afbb-7af1-9d42-5bd8f5b017ef"));
    }

    #[test]
    fn session_id_falls_back_to_uuid_scan() {
        // Plain text output: UUID as a standalone whitespace-separated token (possibly with
        // leading/trailing punctuation that gets trimmed). The UUID must appear where
        // trim_matches(non-alphanumeric, non-hyphen) isolates the UUID portion.
        let stdout = "task complete session=a1b2c3d4-e5f6-7890-abcd-ef1234567890.";
        // The token "session=a1b2c3d4-e5f6-7890-abcd-ef1234567890." → trim '.' from end → still has 'session=' prefix
        // For a clean token, the UUID must be standalone or only have trimmable punctuation
        let stdout2 = "completed. {\"id\": \"unknown\"} a1b2c3d4-e5f6-7890-abcd-ef1234567890 done";
        let id = extract_session_id_generic(stdout2);
        assert_eq!(
            id.as_deref(),
            Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890"),
            "standalone UUID should be found by fallback scan"
        );
        // The first input won't find the UUID (ref= prefix can't be trimmed mid-word)
        let _ = extract_session_id_generic(stdout); // does not panic
    }

    #[test]
    fn session_id_returns_none_for_empty_stdout() {
        assert!(extract_session_id_generic("").is_none());
        assert!(extract_session_id_generic("no id here").is_none());
    }

    // ── Collision-safe, marker-before-spawn attempts ────────────────────────

    #[test]
    fn session_id_line_break_cannot_forge_a_second_terminal_state_header() {
        // `session_id` is scraped from executor stdout (model-influenced) and is
        // emitted AFTER `terminal_state`. An embedded line break would otherwise
        // append a header line of the writer's choosing, letting a failed attempt
        // present itself as `completed` to the evidence gate.
        let tmp = TempDir::new().unwrap();
        let cfg = test_config(&tmp, "some-exec", vec![], "spec", 30);
        let log_path = build_log_path(&cfg.log_dir, &cfg.task_id, &cfg.phase, &cfg.executor);
        let result = DispatchResult {
            terminal_state: TerminalState::Timeout,
            log_path: log_path.clone(),
            session_id: Some("abc\nterminal_state:  completed".to_string()),
            exit_code: Some(1),
            duration_ms: 1,
            stdout: String::new(),
            stderr: String::new(),
        };

        write_log(&log_path, &cfg, "epoch+0s", "epoch+1s", 1, &result).unwrap();
        let content = std::fs::read_to_string(&log_path).unwrap();

        let header = content.split("---STDOUT---").next().unwrap();
        // Exactly one line may carry the `terminal_state` KEY, and its value must
        // be the real one. The forged text survives only as flattened data inside
        // the `session_id` value, where no header reader can key off it.
        let states: Vec<&str> = header
            .lines()
            .filter_map(|l| l.split_once(':'))
            .filter(|(k, _)| k.trim() == "terminal_state")
            .map(|(_, v)| v.trim())
            .collect();
        assert_eq!(
            states,
            vec!["timeout"],
            "the real terminal_state must be the only one a header reader can resolve, got:\n{header}"
        );
        // The id is retained (flattened), not dropped — forensics keep the value.
        assert!(header.contains("session_id:      abc terminal_state:  completed"));
    }

    #[test]
    fn header_value_sanitizer_flattens_line_breaks_only() {
        assert_eq!(sanitize_header_value("a\nb"), "a b");
        assert_eq!(sanitize_header_value("a\r\nb"), "a  b");
        // Non-newline content, including spaces and punctuation, is untouched.
        assert_eq!(
            sanitize_header_value("claude-sonnet-4-6 (x)"),
            "claude-sonnet-4-6 (x)"
        );
    }

    #[test]
    fn rapid_identical_attempts_get_distinct_paths() {
        let tmp = TempDir::new().unwrap();
        let mut paths = std::collections::HashSet::new();
        for _ in 0..50 {
            let p = build_log_path(tmp.path(), "T-TEST", "implement", "claude");
            assert!(
                paths.insert(p),
                "rapid identical task/phase/executor attempts must get distinct paths"
            );
        }
    }

    #[test]
    fn unique_attempt_token_is_monotonically_distinct() {
        let a = unique_attempt_token();
        let b = unique_attempt_token();
        assert_ne!(a, b, "back-to-back attempt tokens must never collide");
    }

    #[test]
    fn started_marker_is_written_before_spawn_and_is_non_empty() {
        let tmp = TempDir::new().unwrap();
        let cfg = test_config(&tmp, "some-exec", vec![], "spec", 30);
        let log_path = build_log_path(&cfg.log_dir, &cfg.task_id, &cfg.phase, &cfg.executor);

        write_started_marker(&log_path, &cfg, "epoch+0s").unwrap();

        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(!content.is_empty(), "started marker must be non-empty");
        assert!(
            content.contains("terminal_state:  started"),
            "started marker must carry terminal_state: started, got: {content}"
        );
        assert!(content.contains("task_id:         T-TEST"));
        assert!(content.contains("phase:           implement"));
        assert!(
            content.contains("GAL-DISPATCH-LOG v1"),
            "started marker must use the same header format as the terminal log"
        );
    }

    #[test]
    fn marker_write_failure_prevents_spawn() {
        let tmp = TempDir::new().unwrap();
        // A file sitting where the log_dir parent needs to be a directory forces
        // `create_dir_all` (inside `write_started_marker`) to fail, so the marker
        // is never written.
        let blocking_file = tmp.path().join("blocked");
        std::fs::write(&blocking_file, "not a directory").unwrap();
        let bad_log_dir = blocking_file.join("executor-logs");

        let mut cfg = test_config(&tmp, "some-exec", vec![], "spec", 30);
        cfg.log_dir = bad_log_dir;

        let result = spawn_executor(&cfg);
        assert!(
            result.is_err(),
            "marker-write failure must prevent spawn and surface as an Err, not a DispatchResult"
        );
    }

    #[test]
    fn terminal_write_failure_leaves_started_marker_residue() {
        let tmp = TempDir::new().unwrap();
        let cfg = test_config(&tmp, "some-exec", vec![], "spec", 30);
        let log_path = build_log_path(&cfg.log_dir, &cfg.task_id, &cfg.phase, &cfg.executor);
        write_started_marker(&log_path, &cfg, "epoch+0s").unwrap();

        // Make the marker file read-only so the terminal rewrite (`write_log`) fails.
        let mut perms = std::fs::metadata(&log_path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&log_path, perms.clone()).unwrap();

        let result = DispatchResult {
            terminal_state: TerminalState::Completed,
            log_path: log_path.clone(),
            session_id: None,
            exit_code: Some(0),
            duration_ms: 1,
            stdout: String::new(),
            stderr: String::new(),
        };
        let write_result = write_log(&log_path, &cfg, "epoch+0s", "epoch+1s", 1, &result);
        assert!(
            write_result.is_err(),
            "write to a read-only marker file must fail"
        );

        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            content.contains("terminal_state:  started"),
            "terminal-write failure must leave the started marker intact, got: {content}"
        );

        // Restore write perms so the TempDir cleanup can delete the file.
        perms.set_readonly(false);
        std::fs::set_permissions(&log_path, perms).unwrap();
    }

    #[test]
    fn successful_spawn_terminal_rewrite_replaces_started_marker_in_place() {
        // End-to-end through the real spawn_executor path (not write_log called
        // directly): prove the SAME attempt path allocated pre-spawn is rewritten
        // from `started` to a terminal state on success, not left as `started` and
        // not written to a second file.
        let tmp = TempDir::new().unwrap();

        #[cfg(target_os = "windows")]
        let (exe, args) = ("cmd", vec!["/C", "echo", "ok"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sh", vec!["-c", "echo ok"]);

        let cfg = test_config(&tmp, exe, args, "spec", 30);

        let result = spawn_executor(&cfg).unwrap();

        // Exactly one attempt file must exist for this task/phase/executor — the
        // pre-spawn `started` marker and the terminal result share one path, not two.
        let entries: Vec<_> = std::fs::read_dir(&cfg.log_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(
            entries.len(),
            1,
            "marker-before-spawn + terminal rewrite must produce exactly one log file, got: {entries:?}"
        );
        assert_eq!(
            entries[0], result.log_path,
            "the single on-disk attempt file must be the terminal result's log_path"
        );

        let content = std::fs::read_to_string(&result.log_path).unwrap();
        assert!(
            !content.contains("terminal_state:  started"),
            "successful terminal rewrite must replace the started marker, got: {content}"
        );
        assert!(
            content.contains("terminal_state:  completed"),
            "successful spawn must rewrite terminal_state to completed, got: {content}"
        );
    }

    // ── Contract provenance header rendering ────────────────────────────────

    #[test]
    fn contract_provenance_none_gives_byte_compatible_legacy_header() {
        let tmp = TempDir::new().unwrap();
        let cfg = test_config(&tmp, "some-exec", vec![], "spec", 30);
        assert!(cfg.contract_provenance.is_none());

        let log_path = build_log_path(&cfg.log_dir, &cfg.task_id, &cfg.phase, &cfg.executor);
        write_started_marker(&log_path, &cfg, "epoch+0s").unwrap();
        let marker_content = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            !marker_content.contains("contract:"),
            "None provenance must produce no contract header line in the started marker, got: {marker_content}"
        );

        let result = DispatchResult {
            terminal_state: TerminalState::Completed,
            log_path: log_path.clone(),
            session_id: None,
            exit_code: Some(0),
            duration_ms: 1,
            stdout: String::new(),
            stderr: String::new(),
        };
        write_log(&log_path, &cfg, "epoch+0s", "epoch+1s", 1, &result).unwrap();
        let log_content = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            !log_content.contains("contract:"),
            "None provenance must produce no contract header line in the terminal log, got: {log_content}"
        );

        // `actual_model` must be immediately followed by `terminal_state` — no
        // blank contract lines inserted between them — matching pre-provenance logs.
        let model_idx = log_content.find("actual_model:").unwrap();
        let after_model = &log_content[model_idx..];
        let model_line_end = after_model.find('\n').unwrap();
        let next_line = &after_model[model_line_end + 1..];
        assert!(
            next_line.starts_with("terminal_state:"),
            "expected terminal_state line immediately after actual_model, got: {next_line}"
        );
    }

    #[test]
    fn contract_provenance_renders_sanitized_and_agrees_across_started_and_terminal() {
        let tmp = TempDir::new().unwrap();
        let mut cfg = test_config(&tmp, "some-exec", vec![], "spec", 30);
        cfg.contract_provenance = Some(ContractProvenance {
            path: PathBuf::from("C:\\Users\\dev\\contract\r\n.md"),
            source: "ancestor\ninjected".to_string(),
        });

        let log_path = build_log_path(&cfg.log_dir, &cfg.task_id, &cfg.phase, &cfg.executor);
        write_started_marker(&log_path, &cfg, "epoch+0s").unwrap();
        let marker_content = std::fs::read_to_string(&log_path).unwrap();

        let result = DispatchResult {
            terminal_state: TerminalState::Completed,
            log_path: log_path.clone(),
            session_id: None,
            exit_code: Some(0),
            duration_ms: 1,
            stdout: String::new(),
            stderr: String::new(),
        };
        write_log(&log_path, &cfg, "epoch+0s", "epoch+1s", 1, &result).unwrap();
        let log_content = std::fs::read_to_string(&log_path).unwrap();

        fn extract_contract_lines(content: &str) -> Vec<&str> {
            content
                .lines()
                .filter(|l| l.starts_with("contract:") || l.starts_with("contract_source:"))
                .collect()
        }
        let marker_lines = extract_contract_lines(&marker_content);
        let log_lines = extract_contract_lines(&log_content);

        assert_eq!(
            marker_lines, log_lines,
            "started marker and terminal log must render identical contract header lines"
        );
        assert_eq!(
            marker_lines.len(),
            2,
            "expected exactly two contract header lines"
        );
        assert!(
            !marker_lines[0].contains('\n') && !marker_lines[0].contains('\r'),
            "raw newlines in the contract path must be sanitized out of the header"
        );
        assert!(
            marker_lines[1] == "contract_source: ancestor injected",
            "raw newline in contract source must be flattened, got: {}",
            marker_lines[1]
        );
    }

    // ── Dispatch marker suffix rendering ────────────────────────────────────

    #[test]
    fn dispatch_marker_suffix_none_is_empty() {
        assert_eq!(
            render_dispatch_marker_suffix(&None),
            "",
            "None provenance must produce no marker suffix, keeping raw/direct `Dispatch:` lines byte-compatible"
        );
    }

    #[test]
    fn dispatch_marker_suffix_renders_path_and_source() {
        let provenance = Some(ContractProvenance {
            path: PathBuf::from("/repo/plugins/gal-core/agents/golem-implementer.agent.md"),
            source: "workdir".to_string(),
        });
        assert_eq!(
            render_dispatch_marker_suffix(&provenance),
            " contract=/repo/plugins/gal-core/agents/golem-implementer.agent.md contract_source=workdir"
        );
    }

    #[test]
    fn dispatch_marker_suffix_sanitizes_unsafe_characters_into_one_line() {
        let provenance = Some(ContractProvenance {
            path: PathBuf::from("C:\\weird\r\npath\\golem-implementer.agent.md"),
            source: "embedded\ninjected".to_string(),
        });
        let suffix = render_dispatch_marker_suffix(&provenance);

        assert!(
            !suffix.contains('\n') && !suffix.contains('\r'),
            "embedded newline/whitespace in path or source must never forge a second marker/banner line, got: {suffix:?}"
        );
        assert_eq!(
            suffix,
            " contract=C:\\weird  path\\golem-implementer.agent.md contract_source=embedded injected"
        );
    }

    // ── Routed dispatch marker suffix rendering ─────────────────────────────

    #[test]
    fn routed_marker_suffix_none_provenance_is_empty_even_with_effort() {
        assert_eq!(
            render_routed_dispatch_marker_suffix(Some("medium"), &None),
            "",
            "no provenance means no route context to attribute effort to; raw/direct dispatch markers must stay byte-compatible"
        );
    }

    #[test]
    fn routed_marker_suffix_explicit_effort_precedes_provenance() {
        let provenance = Some(ContractProvenance {
            path: PathBuf::from("/repo/plugins/gal-core/agents/golem-implementer.agent.md"),
            source: "workdir".to_string(),
        });
        assert_eq!(
            render_routed_dispatch_marker_suffix(Some("medium"), &provenance),
            " effort=medium contract=/repo/plugins/gal-core/agents/golem-implementer.agent.md contract_source=workdir"
        );
    }

    #[test]
    fn routed_marker_suffix_missing_effort_defaults() {
        let provenance = Some(ContractProvenance {
            path: PathBuf::from("/repo/plan.prompt.md"),
            source: "workdir".to_string(),
        });
        assert_eq!(
            render_routed_dispatch_marker_suffix(None, &provenance),
            " effort=(default) contract=/repo/plan.prompt.md contract_source=workdir"
        );
        assert_eq!(
            render_routed_dispatch_marker_suffix(Some(""), &provenance),
            " effort=(default) contract=/repo/plan.prompt.md contract_source=workdir",
            "an empty (but Some) effort must fall back to (default), matching the OFFLOAD block's EFFORT field"
        );
    }

    #[test]
    fn routed_marker_suffix_sanitizes_crlf_in_effort() {
        let provenance = Some(ContractProvenance {
            path: PathBuf::from("/repo/plan.prompt.md"),
            source: "workdir".to_string(),
        });
        let suffix =
            render_routed_dispatch_marker_suffix(Some("bad\r\nreason=forged"), &provenance);

        assert!(
            !suffix.contains('\n') && !suffix.contains('\r'),
            "embedded newline/CR in an (even invalid) routed effort value must never forge a second marker/banner line, got: {suffix:?}"
        );
        assert_eq!(
            suffix,
            " effort=bad  reason=forged contract=/repo/plan.prompt.md contract_source=workdir"
        );
    }

    #[test]
    fn collision_safe_path_still_sanitizes_separators() {
        let tmp = TempDir::new().unwrap();
        let path = build_log_path(
            tmp.path(),
            "T-TEST",
            "implement",
            r"C:\Program Files\Git\bin\sh.exe",
        );
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap();
        assert!(!file_name.contains('\\'), "no raw backslashes in filename");
        assert_eq!(
            file_name.matches(':').count(),
            0,
            "no raw colons in filename"
        );
        assert!(file_name.ends_with("T-TEST-implement-C--Program-Files-Git-bin-sh.exe.log"));
    }
}
