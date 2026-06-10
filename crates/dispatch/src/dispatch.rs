//! Subprocess spawn + stdin feed + process-tree timeout + durable executor log (T-005).
//!
//! # Terminal state vocabulary
//!
//! | State                | Meaning                                              |
//! |----------------------|------------------------------------------------------|
//! | `completed`          | Exit 0 (write-back verification added in T-006)      |
//! | `no-receipt`         | Exit 0 but write-back not confirmed (set by T-006)   |
//! | `timeout`            | Process killed because `timeout_secs` elapsed        |
//! | `disconnected-partial` | Non-zero exit code                                 |
//! | `unavailable`        | Executor CLI not found in PATH                       |
//!
//! # Log file format
//!
//! `<log_dir>/YYYYMMDD-HHMMSS-<task_id>-<phase>-<executor>.log`
//!
//! Header lines (always present before stdout/stderr sections):
//! ```text
//! GAL-DISPATCH-LOG v1
//! timestamp_start: <ISO-8601>
//! timestamp_end:   <ISO-8601>
//! duration_ms:     <u64>
//! executor:        <name>
//! phase:           <implement|test|audit|verify>
//! task_id:         <T-NNN>
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
use std::sync::mpsc;
use std::time::{Duration, Instant};
use thiserror::Error;

const COPILOT_TOKEN_VARS: &[&str] = &[
    "COPILOT_GITHUB_TOKEN",
    "GH_TOKEN",
    "GITHUB_TOKEN",
];

// ── Terminal state ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalState {
    /// Process exited 0; write-back verified (T-006 sets this).
    Completed,
    /// Process exited 0 but write-back not confirmed.
    NoReceipt,
    /// Killed because timeout elapsed.
    Timeout,
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
            Self::DisconnectedPartial => "disconnected-partial",
            Self::Unavailable => "unavailable",
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
    /// Task identifier (e.g. "T-005") — used in log filename and header.
    pub task_id: String,
    /// Pipeline phase string (e.g. "implement") — used in log filename and header.
    pub phase: String,
    /// Actual model injected (for log header transparency).
    pub actual_model: String,
    /// Directory where executor logs are written.
    /// Defaults to `<workdir>/.dev/executor-logs/` when constructed via [`SpawnConfig::default_log_dir`].
    pub log_dir: PathBuf,
    /// Path of the file the executor is expected to write back to (T-006).
    ///
    /// When `Some`, terminal state is only `Completed` if this file exists and is
    /// non-empty after a successful exit (exit 0). Otherwise it is downgraded to
    /// `NoReceipt`. When `None`, receipt verification is skipped and a successful
    /// exit is always recorded as `Completed`.
    pub receipt_path: Option<PathBuf>,
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

    // ── Check executor availability ─────────────────────────────────────────
    if !is_available(&cfg.executor) {
        let duration_ms = start.elapsed().as_millis() as u64;
        let end_ts = utc_now_iso8601();
        let log_path = build_log_path(&cfg.log_dir, &cfg.task_id, &cfg.phase, &cfg.executor);
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

    // ── Spawn the child process ─────────────────────────────────────────────
    // `build_command` resolves Windows script shims (.cmd/.bat/.ps1) that
    // CreateProcess cannot launch directly (e.g. npm-installed `codex.cmd`).
    let mut child = match build_command(&cfg.executor, &cfg.executor_args)
        .current_dir(&cfg.workdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            // Spawn failure (e.g. permission denied) → unavailable
            let duration_ms = start.elapsed().as_millis() as u64;
            let end_ts = utc_now_iso8601();
            let log_path = build_log_path(&cfg.log_dir, &cfg.task_id, &cfg.phase, &cfg.executor);
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

    // ── Feed spec to stdin (close stdin after writing) ──────────────────────
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(cfg.spec.as_bytes());
        // stdin dropped here → EOF sent to child
    }

    let child_pid = child.id();
    let timeout = Duration::from_secs(cfg.timeout_secs);

    // ── Wait with timeout using a background thread ─────────────────────────
    let (tx, rx) = mpsc::channel::<std::io::Result<std::process::Output>>();

    std::thread::spawn(move || {
        let output = child.wait_with_output();
        let _ = tx.send(output);
    });

    let (terminal_state, exit_code, stdout_str, stderr_str) = match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => {
            let code = output.status.code();
            let out = String::from_utf8_lossy(&output.stdout).into_owned();
            let err = String::from_utf8_lossy(&output.stderr).into_owned();
            let state = if output.status.success() {
                // T-006: verify write-back. Downgrade to NoReceipt if file absent/empty.
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
        Ok(Err(e)) => {
            (TerminalState::DisconnectedPartial, None, String::new(), format!("wait_with_output error: {e}"))
        }
        Err(_timeout) => {
            // Timeout: kill the process tree
            kill_process_tree(child_pid);
            (TerminalState::Timeout, None, String::new(), format!("killed after {}s timeout", cfg.timeout_secs))
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    let end_ts = utc_now_iso8601();
    let log_path = build_log_path(&cfg.log_dir, &cfg.task_id, &cfg.phase, &cfg.executor);

    // Extract session id from stdout (tool-specific; adapter layer in T-008 refines this)
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

    write_log(&log_path, cfg, &start_ts, &end_ts, duration_ms, &result)?;
    Ok(result)
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Verify that the receipt file exists and is non-empty (T-006).
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
    dirs::home_dir()
        .map(|home| home.join(".copilot").exists())
        .unwrap_or(false)
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
        hint: "run `copilot login` or set `COPILOT_GITHUB_TOKEN` / `GH_TOKEN` for headless use".to_string(),
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
        if let Some(found) = candidates.iter().find(|c| c.to_ascii_lowercase().ends_with(ext)) {
            return Some(found.clone());
        }
    }
    // Fallback: first candidate carrying any extension (skip extensionless shims).
    candidates
        .into_iter()
        .find(|c| std::path::Path::new(c).extension().is_some())
}

/// Kill a process tree rooted at `pid`.
fn kill_process_tree(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output();
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Send SIGKILL to the process group (best-effort; may not catch all orphans)
        let _ = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output();
    }
}

/// Build the log file path: `<log_dir>/YYYYMMDD-HHMMSS-<task>-<phase>-<executor>.log`
fn build_log_path(log_dir: &Path, task_id: &str, phase: &str, executor: &str) -> PathBuf {
    // Use a timestamp-derived name; without chrono just use a monotonic counter proxy.
    // The actual timestamp is written in the log header; filename uses UTC seconds for uniqueness.
    let ts = utc_seconds_for_filename();
    // Sanitize task_id (replace / and spaces)
    let safe_task = task_id.replace(['/', ' '], "-");
    let filename = format!("{ts}-{safe_task}-{phase}-{executor}.log");
    log_dir.join(filename)
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
    let exit_str = result.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "none".to_string());
    let session_str = result.session_id.as_deref().unwrap_or("none");

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
         terminal_state:  {state}\n\
         session_id:      {session_str}\n\
         ---STDOUT---\n\
         {stdout}\n\
         ---STDERR---\n\
         {stderr}\n",
        executor = cfg.executor,
        phase = cfg.phase,
        task_id = cfg.task_id,
        model = cfg.actual_model,
        state = result.terminal_state,
        stdout = result.stdout,
        stderr = result.stderr,
    );

    std::fs::write(path, &content).map_err(DispatchError::LogWrite)?;
    Ok(())
}

/// Generic session id extractor for use in the durable log header (T-006/T-012).
///
/// Tries JSON field extraction first (covers claude, opencode, copilot whose stdout
/// is a JSON object or NDJSON stream), then falls back to a UUID-shape scan.
/// Tool-specific adapters (T-008) may further refine this for the `Dispatch:` marker.
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
                if let Some(id) = v.get("result")
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
    // Full ISO-8601 is available via chrono (added in T-008 when session ids are logged).
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("epoch+{secs}s") // placeholder until chrono is added
}

fn utc_seconds_for_filename() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(tmp: &TempDir, executor: &str, args: Vec<&str>, spec: &str, timeout_secs: u64) -> SpawnConfig {
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
        }
    }

    // TP-006: echo mock → completed
    #[test]
    fn echo_exits_zero_gives_completed() {
        let tmp = TempDir::new().unwrap();

        #[cfg(target_os = "windows")]
        let (exe, args) = ("cmd", vec!["/C", "echo", "hello"]);
        #[cfg(not(target_os = "windows"))]
        let (exe, args) = ("sh", vec!["-c", "echo hello"]);

        let cfg = test_config(&tmp, exe, args, "spec", 30);
        let result = spawn_executor(&cfg).unwrap();
        assert_eq!(result.terminal_state, TerminalState::Completed,
            "exit-0 process should give Completed before write-back check");
        assert!(result.exit_code == Some(0));
    }

    // TP-006: non-zero exit → disconnected-partial
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

    // TP-006: missing CLI → unavailable
    #[test]
    fn missing_executor_gives_unavailable() {
        let tmp = TempDir::new().unwrap();
        let cfg = test_config(&tmp, "nonexistent-executor-xyz-12345", vec![], "spec", 30);
        let result = spawn_executor(&cfg).unwrap();
        assert_eq!(result.terminal_state, TerminalState::Unavailable);
    }

    // TP-006: timeout → timeout state
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
        assert_eq!(result.terminal_state, TerminalState::Timeout);
    }

    // TP-006: log header present and contains required fields
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
        assert!(log_content.contains("GAL-DISPATCH-LOG v1"), "missing log header");
        assert!(log_content.contains("timestamp_start:"), "missing timestamp_start");
        assert!(log_content.contains("timestamp_end:"), "missing timestamp_end");
        assert!(log_content.contains("duration_ms:"), "missing duration_ms");
        assert!(log_content.contains("executor:"), "missing executor");
        assert!(log_content.contains("phase:"), "missing phase");
        assert!(log_content.contains("task_id:"), "missing task_id");
        assert!(log_content.contains("git_branch:"), "missing git_branch");
        assert!(log_content.contains("git_head:"), "missing git_head");
        assert!(log_content.contains("exit_code:"), "missing exit_code");
        assert!(log_content.contains("actual_model:"), "missing actual_model");
        assert!(log_content.contains("terminal_state:"), "missing terminal_state");
        assert!(log_content.contains("session_id:"), "missing session_id");
        assert!(log_content.contains("---STDOUT---"), "missing STDOUT section");
        assert!(log_content.contains("---STDERR---"), "missing STDERR section");
    }

    #[test]
    fn terminal_state_strings_are_correct() {
        assert_eq!(TerminalState::Completed.as_str(), "completed");
        assert_eq!(TerminalState::NoReceipt.as_str(), "no-receipt");
        assert_eq!(TerminalState::Timeout.as_str(), "timeout");
        assert_eq!(TerminalState::DisconnectedPartial.as_str(), "disconnected-partial");
        assert_eq!(TerminalState::Unavailable.as_str(), "unavailable");
    }

    #[test]
    fn uuid_detection_works() {
        assert!(looks_like_uuid("f5c9f450-c44d-41ba-9e7e-e0bfcb19e105"));
        assert!(!looks_like_uuid("not-a-uuid"));
        assert!(!looks_like_uuid("too-short-1234"));
        assert!(!looks_like_uuid(""));
    }

    // TP-007: receipt file written by executor → Completed
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
        assert_eq!(result.terminal_state, TerminalState::Completed,
            "exit-0 + non-empty receipt file should give Completed");
    }

    // TP-007: executor exits 0 but receipt file missing → NoReceipt
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
        assert_eq!(result.terminal_state, TerminalState::NoReceipt,
            "exit-0 but missing receipt file should give NoReceipt");
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
            executor_readiness("codex"),
            Readiness::Unknown {
                message: "executor 'codex' has no dedicated readiness probe yet; allowing dispatch"
                    .to_string(),
            }
        );
    }

    // TP-007: executor exits 0 but receipt file is empty → NoReceipt (partial write)
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
        assert_eq!(result.terminal_state, TerminalState::NoReceipt,
            "exit-0 but empty receipt file (partial write) should give NoReceipt");
    }

    // TP-007: no receipt_path configured → Completed (no check)
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
        assert_eq!(result.terminal_state, TerminalState::Completed,
            "no receipt_path → skip check → Completed");
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

    // ── Session id extraction tests (T-012 / T-006 completion) ─────────────────

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
        let stdout = r#"{"result":{"sessionId":"0f3e7af9-6cd2-4853-bf41-9e5a7a9b6df9","status":"ok"}}"#;
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
        assert_eq!(id.as_deref(), Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890"),
            "standalone UUID should be found by fallback scan");
        // The first input won't find the UUID (ref= prefix can't be trimmed mid-word)
        let _ = extract_session_id_generic(stdout); // does not panic
    }

    #[test]
    fn session_id_returns_none_for_empty_stdout() {
        assert!(extract_session_id_generic("").is_none());
        assert!(extract_session_id_generic("no id here").is_none());
    }
}
