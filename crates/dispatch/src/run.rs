//! In-process dispatch flow shared by the `gal-dispatch` bin and `gal pipeline`.
//!
//! Extracted from the bin's `main` so the single `gal` binary can run a headless
//! dispatch without spawning a separate `gal-dispatch` executable (which is not in
//! the end-user release artifact). The safety gate (routing present +
//! executor available + authenticated) lives here, so both entry points share it.

use std::path::PathBuf;

use crate::cli::DispatchArgs;
use crate::dispatch::{
    executor_readiness, is_available, spawn_executor, Readiness, SpawnConfig, TerminalState,
};
use crate::executors::{self, SpecDelivery};
use crate::routing::{load_routing, load_routing_default, RouteEntry};

/// Print one dispatch banner/marker line to stdout, or to stderr when `quiet`
/// is set (so a caller capturing clean machine-readable stdout, e.g. a
/// `--json` smoke report, is never corrupted by dispatch's own banner text).
fn emit_line(quiet: bool, line: &str) {
    if quiet {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

/// Result of an in-process dispatch run.
pub struct DispatchOutcome {
    /// Process exit code: 0 completed, 1 ran/unconfirmed, 2 degraded/unavailable.
    pub exit_code: u8,
    pub terminal_state: Option<TerminalState>,
    pub session_id: Option<String>,
    pub log_path: Option<PathBuf>,
    /// Machine-readable reason token, mirroring the `reason=` field of the
    /// `Dispatch:` marker line, so an in-process caller can classify a degrade or
    /// remote post-processing outcome without parsing stdout. `None` on a clean
    /// completed run. The remote branch sets `remote-receipt-fetch-failed` when a
    /// remote receipt-fetch fails (distinct from a genuinely missing/empty remote
    /// receipt, which stays `NoReceipt` with no reason); every degrade path carries
    /// its marker reason here (e.g. `remote-guard-failed`, `remote-cliflag-unsupported`).
    pub reason: Option<String>,
}

/// Run one headless dispatch: load routing → safety gate → adapter → spawn →
/// emit the `--- GAL DISPATCH ---` marker. The spec is the full task spec text.
pub fn run_dispatch(args: &DispatchArgs, spec: &str) -> DispatchOutcome {
    let degraded = |exit_code: u8, reason: &str| DispatchOutcome {
        exit_code,
        terminal_state: None,
        session_id: None,
        log_path: None,
        reason: Some(reason.to_string()),
    };

    let routing = match &args.routing_path {
        Some(p) => load_routing(p),
        None => load_routing_default(),
    };
    for w in &routing.warnings {
        eprintln!("warning: {w}");
    }

    let role = args.phase.role();

    // ── safety gate: only offload with routing + an available executor ──
    let entry = match routing.get(role) {
        None => {
            emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
            emit_line(
                args.stdout_quiet,
                &format!(
                    "Dispatch: phase={} task={} role={} executor=none reason=no-routing",
                    args.phase.as_str(),
                    args.task,
                    role,
                ),
            );
            eprintln!("gal dispatch: no executor configured for role {role}; falling back to text dispatch");
            return degraded(2, "no-routing");
        }
        Some(e) => e,
    };

    let executor_name = &entry.executor;
    let model = &entry.model;

    // ── half-configured sshTarget/remoteWorkdir → fail loud (never silently run local) ──
    if entry.ssh_target.is_some() != entry.remote_workdir.is_some() {
        emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
        emit_line(
            args.stdout_quiet,
            &format!(
                "Dispatch: phase={} task={} role={} executor={} model={} reason=remote-missing-workdir",
                args.phase.as_str(),
                args.task,
                role,
                executor_name,
                model,
            ),
        );
        eprintln!("gal dispatch: role '{role}' has sshTarget/remoteWorkdir half-configured; both are required for remote dispatch");
        return degraded(2, "remote-missing-workdir");
    }

    // ── effort pre-dispatch check (single pinned location) ──
    // When the route sets `effort`, it must be honorable before we spawn:
    //   (a) the resolved adapter must opt in via `supports_effort()` (fail-closed
    //       default is `false`, so agy / unknown executors reject here), else
    //       `unsupported-effort`;
    //   (b) the value must be a non-empty argv-safe token `[A-Za-z0-9._-]+` (it is
    //       embedded in the Codex TOML override and every native flag), else
    //       `invalid-effort`.
    // Never silently ignore, never warn-and-continue.
    if let Some(effort) = entry.effort.as_deref() {
        let supported = executors::get_adapter(executor_name)
            .map(|a| a.supports_effort())
            .unwrap_or(false);
        if !supported {
            emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
            emit_line(
                args.stdout_quiet,
                &format!(
                    "Dispatch: phase={} task={} role={} executor={} model={} reason=unsupported-effort",
                    args.phase.as_str(),
                    args.task,
                    role,
                    executor_name,
                    model,
                ),
            );
            eprintln!("gal dispatch: role '{role}' sets effort but executor '{executor_name}' cannot honor it; remove `effort` from this route or route it to an executor that supports it");
            return degraded(2, "unsupported-effort");
        }
        if !is_valid_effort(effort) {
            emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
            emit_line(
                args.stdout_quiet,
                &format!(
                    "Dispatch: phase={} task={} role={} executor={} model={} reason=invalid-effort",
                    args.phase.as_str(),
                    args.task,
                    role,
                    executor_name,
                    model,
                ),
            );
            eprintln!("gal dispatch: role '{role}' effort value is not a valid argv-safe token (allowed: [A-Za-z0-9._-]); refusing to embed it in a native flag");
            return degraded(2, "invalid-effort");
        }
    }

    let cfg = if entry.is_remote() {
        // ── remote branch: gate on `ssh`, not the local executor (F1) ──
        if !is_available("ssh") {
            emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
            emit_line(
                args.stdout_quiet,
                &format!(
                    "Dispatch: phase={} task={} role={} executor=ssh model={} reason=executor-unavailable",
                    args.phase.as_str(),
                    args.task,
                    role,
                    model,
                ),
            );
            eprintln!("gal dispatch: 'ssh' not found in PATH; falling back to text dispatch");
            return degraded(2, "executor-unavailable");
        }

        // ── reject CliFlag delivery over ssh (F3) — copilot is local-only in v1 ──
        let delivery_is_cliflag = executors::get_adapter(executor_name)
            .map(|a| {
                let input = executors::AdapterInvocationInput {
                    model,
                    workdir: &args.workdir,
                    spec,
                    effort: None,
                    mcp_disable_servers: &[],
                };
                matches!(
                    a.build_invocation(&input).delivery,
                    SpecDelivery::CliFlag(_)
                )
            })
            .unwrap_or(false);
        if delivery_is_cliflag {
            emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
            emit_line(
                args.stdout_quiet,
                &format!(
                    "Dispatch: phase={} task={} role={} executor={} model={} reason=remote-cliflag-unsupported",
                    args.phase.as_str(),
                    args.task,
                    role,
                    executor_name,
                    model,
                ),
            );
            eprintln!("gal dispatch: executor '{executor_name}' delivers its spec via a CLI flag, which cannot be safely forwarded over ssh; remote dispatch is unsupported for this executor in v1");
            return degraded(2, "remote-cliflag-unsupported");
        }

        // ── receipt mapping (F4): reject an explicit absolute --receipt on a remote
        // route before composing — the control-node absolute path must never reach
        // the remote spec (`remote-absolute-receipt-unsupported`) ──
        if let Some(local_receipt) = &args.receipt_path {
            if derive_receipt_rel_path(local_receipt, &args.workdir).is_none() {
                emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
                emit_line(
                    args.stdout_quiet,
                    &format!(
                        "Dispatch: phase={} task={} role={} executor={} model={} reason=remote-absolute-receipt-unsupported",
                        args.phase.as_str(),
                        args.task,
                        role,
                        executor_name,
                        model,
                    ),
                );
                eprintln!("gal dispatch: an explicit absolute --receipt cannot be mapped to a workdir-relative remote path; pass a receipt under the workdir instead");
                return degraded(2, "remote-absolute-receipt-unsupported");
            }
        }

        // ── pre-run guard (F6): remote checkout must be at the control node's current
        // HEAD and clean before this run starts, so a multi-task remote pipeline never
        // silently stacks task N's leftover diff onto task N+1's fetch ──
        if let (Some(target), Some(remote_workdir)) = (&entry.ssh_target, &entry.remote_workdir) {
            let expected_head = local_git_head(&args.workdir);
            let guard_ok = match expected_head {
                Some(head) => {
                    let guard_cmd = guard_command(remote_workdir, &head);
                    let guard_argv = ssh_args(target, &guard_cmd);
                    match std::process::Command::new("ssh").args(&guard_argv).output() {
                        Ok(out) => {
                            out.status.success()
                                && String::from_utf8_lossy(&out.stdout).contains("PARITY_OK")
                        }
                        Err(_) => false,
                    }
                }
                None => false,
            };
            if !guard_ok {
                emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
                emit_line(
                    args.stdout_quiet,
                    &format!(
                        "Dispatch: phase={} task={} role={} executor={} model={} reason=remote-guard-failed",
                        args.phase.as_str(),
                        args.task,
                        role,
                        executor_name,
                        model,
                    ),
                );
                eprintln!("gal dispatch: remote checkout at '{remote_workdir}' is not at the control node's HEAD, is not clean, or the guard could not be evaluated; re-sync the remote checkout before retrying");
                return degraded(2, "remote-guard-failed");
            }
        }

        build_remote_spawn_config(entry, args, spec)
    } else {
        // ── local branch: gate on the configured executor (unchanged baseline) ──
        if !is_available(executor_name) {
            emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
            emit_line(
                args.stdout_quiet,
                &format!(
                    "Dispatch: phase={} task={} role={} executor={} model={} reason=executor-unavailable",
                    args.phase.as_str(),
                    args.task,
                    role,
                    executor_name,
                    model,
                ),
            );
            eprintln!("gal dispatch: executor '{executor_name}' not found in PATH; falling back to text dispatch");
            return degraded(2, "executor-unavailable");
        }

        match executor_readiness(executor_name) {
            Readiness::Ready => {}
            Readiness::Unauthenticated { hint } => {
                emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
                emit_line(
                    args.stdout_quiet,
                    &format!(
                        "Dispatch: phase={} task={} role={} executor={} model={} reason=executor-unauthenticated-confirmed",
                        args.phase.as_str(),
                        args.task,
                        role,
                        executor_name,
                        model,
                    ),
                );
                eprintln!("gal dispatch: executor '{executor_name}' is not authenticated for confirmed headless use; {hint}");
                return degraded(2, "executor-unauthenticated-confirmed");
            }
            Readiness::Unknown { message } => {
                eprintln!("warning: gal dispatch: {message}");
            }
        }

        // Copilot slims its host-loaded MCP servers so a headless prompt-mode run does
        // not overflow before writing the receipt. The disable list is derived from
        // Copilot's own `~/.copilot/mcp-config.json`; other executors get an empty list.
        let mcp_disable: Vec<String> = if executor_name == "copilot" {
            let (names, warn) = copilot_mcp_disable_names(copilot_mcp_config_path());
            if let Some(w) = warn {
                eprintln!("warning: gal dispatch: {w}");
            }
            names
        } else {
            Vec::new()
        };

        build_local_spawn_config(entry, args, spec, &mcp_disable)
    };

    let is_remote_dispatch = entry.is_remote();

    let mut result = match spawn_executor(&cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gal dispatch: log write failed: {e}");
            emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
            emit_line(
                args.stdout_quiet,
                &format!(
                    "Dispatch: phase={} task={} executor={} model={} reason=log-error",
                    args.phase.as_str(),
                    args.task,
                    executor_name,
                    model,
                ),
            );
            return degraded(2, "log-error");
        }
    };

    // Reason carried into the final `DispatchOutcome` for the caller to classify remote
    // post-processing outcomes that downgrade `terminal_state` after the spawn. Left `None`
    // on a clean completed run; set to `remote-receipt-fetch-failed` when the receipt-fetch
    // step below fails, so a caller can tell that apart from a genuinely missing receipt.
    let mut post_reason: Option<String> = None;

    // F4: for a remote dispatch, the receipt (if requested) was written on the *remote*
    // filesystem, never checked by `spawn_executor` (its `SpawnConfig.receipt_path` is always
    // `None` on the remote branch — see `build_remote_spawn_config`). Fetch the remote bytes
    // over ssh into the local receipt path here, then apply the same non-empty-file rule
    // `verify_receipt` uses. Only meaningful when the remote run itself completed cleanly.
    if is_remote_dispatch && result.terminal_state == TerminalState::Completed {
        if let (Some(local_receipt), Some(target), Some(remote_workdir)) =
            (&args.receipt_path, &entry.ssh_target, &entry.remote_workdir)
        {
            // Already validated as workdir-relative above; re-derive here rather than
            // threading extra state through `cfg`.
            let receipt_rel =
                derive_receipt_rel_path(local_receipt, &args.workdir).unwrap_or_default();
            let fetch_cmd = receipt_fetch_command(remote_workdir, &receipt_rel);
            let fetch_argv = ssh_args(target, &fetch_cmd);
            let fetched = std::process::Command::new("ssh").args(&fetch_argv).output();
            let fetched_ok = match fetched {
                Ok(out) if out.status.success() && !out.stdout.is_empty() => {
                    if let Some(parent) = local_receipt.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::fs::write(local_receipt, &out.stdout).is_ok()
                }
                _ => false,
            };
            if !fetched_ok {
                result.terminal_state = TerminalState::NoReceipt;
                post_reason = Some("remote-receipt-fetch-failed".to_string());
            }
        }
    }

    // File-return (F6): only for the mutating `implement` phase, and only when the remote
    // run itself completed cleanly. Fetch the remote diff as a binary-safe patch, apply it
    // to the control-node workdir, and — only on a successful local apply — clean the remote
    // checkout back to pristine HEAD. Apply failure leaves the remote intact (never cleaned)
    // so the diff can be inspected/retried; the dispatch itself fails loud in that case.
    if is_remote_dispatch
        && args.phase == crate::stage::Phase::Implement
        && result.terminal_state == TerminalState::Completed
    {
        if let (Some(target), Some(remote_workdir)) = (&entry.ssh_target, &entry.remote_workdir) {
            let fetch_argv = ssh_args(target, &diff_fetch_command(remote_workdir));
            match std::process::Command::new("ssh").args(&fetch_argv).output() {
                Ok(out) if out.status.success() => {
                    if !out.stdout.is_empty() {
                        let patch_path = args
                            .workdir
                            .join(".dev")
                            .join("pipeline")
                            .join(format!("{}-remote.patch", args.task));
                        let apply_ok = std::fs::write(&patch_path, &out.stdout).is_ok()
                            && std::process::Command::new("git")
                                .args(["apply", &patch_path.display().to_string()])
                                .current_dir(&args.workdir)
                                .output()
                                .map(|o| o.status.success())
                                .unwrap_or(false);
                        let _ = std::fs::remove_file(&patch_path);
                        if apply_ok {
                            // Only clean the remote once the local apply genuinely succeeded.
                            let cleanup_argv = ssh_args(target, &cleanup_command(remote_workdir));
                            let _ = std::process::Command::new("ssh")
                                .args(&cleanup_argv)
                                .output();
                        } else {
                            eprintln!("gal dispatch: remote diff fetched but `git apply` failed on the control node; remote checkout at '{remote_workdir}' left intact for inspection");
                            result.terminal_state = TerminalState::DisconnectedPartial;
                        }
                    }
                }
                _ => {
                    eprintln!("gal dispatch: remote diff-fetch failed; remote checkout at '{remote_workdir}' left intact");
                    result.terminal_state = TerminalState::DisconnectedPartial;
                }
            }
        }
    }

    // Remote post-processing can downgrade the final dispatch state after `spawn_executor`
    // has already written the ssh-process log. Keep executor-log evidence aligned with
    // the final state that `gal dispatch` returns.
    if is_remote_dispatch {
        sync_log_terminal_state(&result.log_path, &result.terminal_state);
    }
    // F2: agy's `extract_session_id` ignores stdout and scans the *local* `~/.agy/brain/`
    // filesystem — on a remote dispatch that would report a stale local session id, which is
    // wrong data, not merely a missing value. Remote dispatch never calls it for agy; every
    // other adapter genuinely parses `stdout`, which is legitimately forwarded over ssh.
    let session_id = if is_remote_dispatch && executor_name == "agy" {
        None
    } else if let Some(adapter) = executors::get_adapter(executor_name) {
        adapter
            .extract_session_id(&result.stdout)
            .or_else(|| result.session_id.clone())
    } else {
        result.session_id.clone()
    };
    let session_str = session_id.as_deref().unwrap_or("none");

    emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
    emit_line(
        args.stdout_quiet,
        &format!(
            "Dispatch: phase={} task={} role={} executor={} model={} state={} session_id={} log={}",
            args.phase.as_str(),
            args.task,
            role,
            executor_name,
            model,
            result.terminal_state,
            session_str,
            result.log_path.display(),
        ),
    );

    let exit_code = match result.terminal_state {
        TerminalState::Completed => 0,
        TerminalState::NoReceipt | TerminalState::DisconnectedPartial => 1,
        TerminalState::Timeout
        | TerminalState::TimeoutNoOutput
        | TerminalState::TimeoutMidrun
        | TerminalState::Unavailable => 2,
    };

    DispatchOutcome {
        exit_code,
        terminal_state: Some(result.terminal_state),
        session_id,
        log_path: Some(result.log_path),
        reason: post_reason,
    }
}

/// `true` when `effort` is a non-empty argv-safe token: every char is in
/// `[A-Za-z0-9._-]`. This is the syntactic contract enforced before an `effort`
/// value is embedded in any native flag (notably the Codex `-c` TOML override),
/// so a value can never carry a quote/whitespace/control char that would break it.
fn is_valid_effort(effort: &str) -> bool {
    !effort.is_empty()
        && effort
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// The Copilot CLI's host MCP config path: `~/.copilot/mcp-config.json`. `None`
/// when the home directory cannot be resolved.
fn copilot_mcp_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".copilot").join("mcp-config.json"))
}

/// Resolve the Copilot MCP server names to disable, from the Copilot CLI's own host
/// config (the set Copilot actually loads). Returns `(names, warning)`:
/// - path `None` or file missing → `(empty, None)` — a valid, expected state.
/// - file present but unreadable or not valid JSON → `(empty, Some(warning))` —
///   never silent.
/// - valid JSON → sorted top-level `mcpServers` keys (empty if the key is absent).
///
/// No owner-local server name is ever hardcoded; the list is entirely host-derived.
/// Names are sorted so the resulting argv is deterministic.
fn copilot_mcp_disable_names(path: Option<PathBuf>) -> (Vec<String>, Option<String>) {
    let Some(path) = path else {
        return (Vec::new(), None);
    };
    if !path.exists() {
        return (Vec::new(), None);
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            return (
                Vec::new(),
                Some(format!(
                    "copilot mcp config at {} is unreadable ({e}); no MCP servers disabled",
                    path.display()
                )),
            );
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return (
                Vec::new(),
                Some(format!(
                    "copilot mcp config at {} is not valid JSON ({e}); no MCP servers disabled",
                    path.display()
                )),
            );
        }
    };
    let mut names: Vec<String> = value
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    names.sort();
    (names, None)
}

/// Compose the local `SpawnConfig` for `entry` — the route-to-spawn seam. Pure: builds
/// executor args via the adapter and the log dir path, but performs no I/O and no spawn.
/// `mcp_disable` is the caller-resolved list of MCP server names to disable (host-derived
/// for copilot, empty for every other executor). This is the baseline byte-for-byte local
/// behavior; a route with `is_remote()` true takes a separate remote-composition path.
pub fn build_local_spawn_config(
    entry: &RouteEntry,
    args: &DispatchArgs,
    spec: &str,
    mcp_disable: &[String],
) -> SpawnConfig {
    let executor_name = &entry.executor;
    let model = &entry.model;

    let (executor_args, stdin_spec) = match executors::get_adapter(executor_name) {
        Some(adapter) => {
            let input = executors::AdapterInvocationInput {
                model,
                workdir: &args.workdir,
                spec,
                effort: entry.effort.as_deref(),
                mcp_disable_servers: mcp_disable,
            };
            let inv = adapter.build_invocation(&input);
            let stdin = match inv.delivery {
                SpecDelivery::Stdin => spec.to_string(),
                SpecDelivery::CliFlag(_) => String::new(),
            };
            (inv.args, stdin)
        }
        None => (vec![], spec.to_string()),
    };

    SpawnConfig {
        executor: executor_name.clone(),
        executor_args,
        spec: stdin_spec,
        workdir: args.workdir.clone(),
        timeout_secs: args.timeout_secs,
        task_id: args.task.clone(),
        phase: args.phase.as_str().to_string(),
        actual_model: model.clone(),
        log_dir: args
            .log_dir_override
            .clone()
            .unwrap_or_else(|| SpawnConfig::default_log_dir(&args.workdir)),
        receipt_path: args.receipt_path.clone(),
    }
}

/// Read the control-node's current git HEAD (full SHA) in `workdir`. `None` on any failure
/// (not a git repo, `git` unavailable, etc.) — the caller treats that as guard-unusable.
fn local_git_head(workdir: &std::path::Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workdir)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let head = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if head.is_empty() {
                    None
                } else {
                    Some(head)
                }
            } else {
                None
            }
        })
}

fn sync_log_terminal_state(log_path: &std::path::Path, state: &TerminalState) {
    let Ok(content) = std::fs::read_to_string(log_path) else {
        return;
    };
    let mut changed = false;
    let mut updated = String::with_capacity(content.len());
    for line in content.lines() {
        if line.starts_with("terminal_state:") {
            updated.push_str(&format!("terminal_state:  {state}"));
            changed = true;
        } else {
            updated.push_str(line);
        }
        updated.push('\n');
    }
    if changed {
        let _ = std::fs::write(log_path, updated);
    }
}
/// Compose the remote `SpawnConfig` for a route with `sshTarget` + `remoteWorkdir` set.
/// Pure: assumes the caller already rejected `SpecDelivery::CliFlag` (F3) and confirmed
/// `entry.is_remote()`. The composed process is literally `ssh`; the actual agent CLI runs
/// on the far end of the held session via `remote_command`/`ssh_args` (see below).
pub fn build_remote_spawn_config(
    entry: &RouteEntry,
    args: &DispatchArgs,
    spec: &str,
) -> SpawnConfig {
    let executor_name = &entry.executor;
    let model = &entry.model;
    let remote_workdir = entry.remote_workdir.as_deref().unwrap_or_default();
    let target = entry.ssh_target.as_deref().unwrap_or_default();

    let executor_args = match executors::get_adapter(executor_name) {
        Some(adapter) => {
            let input = executors::AdapterInvocationInput {
                model,
                workdir: &args.workdir,
                spec,
                effort: entry.effort.as_deref(),
                mcp_disable_servers: &[],
            };
            adapter.build_invocation(&input).args
        }
        None => vec![],
    };

    let remote_cmd = remote_command(remote_workdir, executor_name, &executor_args);
    let ssh_argv = ssh_args(target, &remote_cmd);

    SpawnConfig {
        executor: "ssh".to_string(),
        executor_args: ssh_argv,
        spec: spec.to_string(),
        workdir: args.workdir.clone(),
        timeout_secs: args.timeout_secs,
        task_id: args.task.clone(),
        phase: args.phase.as_str().to_string(),
        actual_model: model.clone(),
        log_dir: args
            .log_dir_override
            .clone()
            .unwrap_or_else(|| SpawnConfig::default_log_dir(&args.workdir)),
        // Receipt verification for remote dispatch happens via an explicit post-run
        // fetch-over-ssh step in `run_dispatch` (the receipt lives on the *remote*
        // filesystem, not locally) — never the local-file check `spawn_executor` runs
        // when `receipt_path` is set. See the remote branch's fetch step.
        receipt_path: None,
    }
}

// ── Remote (SSH) command composition — pure, no I/O, no network ────────────

/// POSIX single-quote escape: `'` → `'\''`. Wraps `s` in single quotes so it is
/// safe to interpolate into a remote shell command regardless of content.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r#"'\''"#))
}

/// Compose the remote command: `cd '<workdir>' && '<executor>' '<arg>' …`.
pub fn remote_command(workdir: &str, executor: &str, args: &[String]) -> String {
    let mut cmd = format!("cd {} && {}", shell_quote(workdir), shell_quote(executor));
    for a in args {
        cmd.push(' ');
        cmd.push_str(&shell_quote(a));
    }
    cmd
}

/// Compose the full `ssh` argv: `["-o", "BatchMode=yes", "<target>", "<remote_cmd>"]`.
pub fn ssh_args(target: &str, remote_cmd: &str) -> Vec<String> {
    vec![
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        target.to_string(),
        remote_cmd.to_string(),
    ]
}

/// Join a remote workdir with a workdir-relative receipt path (forward-slash, POSIX remote only).
pub fn remote_receipt_path(remote_workdir: &str, rel: &str) -> String {
    format!("{}/{}", remote_workdir.trim_end_matches('/'), rel)
}

/// Compose the remote receipt-fetch command: print the receipt file's bytes to stdout.
pub fn receipt_fetch_command(remote_workdir: &str, rel: &str) -> String {
    format!(
        "cat {}",
        shell_quote(&remote_receipt_path(remote_workdir, rel))
    )
}

/// Derive the workdir-relative receipt path from a resolved local receipt path.
/// `None` means `receipt_path` was not under `workdir` — i.e. the caller passed an
/// explicit absolute `--receipt`, which is unsupported on a remote route
/// (`remote-absolute-receipt-unsupported`; the control-node absolute path must never
/// be embedded into the remote spec).
pub fn derive_receipt_rel_path(
    receipt_path: &std::path::Path,
    workdir: &std::path::Path,
) -> Option<String> {
    receipt_path
        .strip_prefix(workdir)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
}

/// Pre-run guard: remote HEAD must equal `expected_head` and the remote tree must be clean.
/// Prints `PARITY_OK` only when both hold; any mismatch or dirty tree leaves no `PARITY_OK` line.
pub fn guard_command(remote_workdir: &str, expected_head: &str) -> String {
    format!(
        "cd {wd} && test \"$(git rev-parse HEAD)\" = {head} && test -z \"$(git status --porcelain)\" && echo PARITY_OK",
        wd = shell_quote(remote_workdir),
        head = shell_quote(expected_head),
    )
}

/// Fetch remote mutations as a binary-safe patch: stage everything, diff cached, then unstage.
/// Leaves the remote tree byte-identical to before the fetch (staging is reset, nothing committed).
pub fn diff_fetch_command(remote_workdir: &str) -> String {
    format!(
        "cd {wd} && git add -A && git diff --cached --binary && git reset -q",
        wd = shell_quote(remote_workdir),
    )
}

/// Destructive remote cleanup: hard-reset + remove untracked files/dirs. Only ever run after a
/// successful local `git apply` of the fetched diff — see `crates/dispatch/src/run.rs` remote branch.
pub fn cleanup_command(remote_workdir: &str) -> String {
    format!(
        "cd {wd} && git reset --hard -q && git clean -fdq",
        wd = shell_quote(remote_workdir),
    )
}

#[cfg(test)]
mod route_to_spawn_baseline_tests {
    use super::*;
    use crate::cli::DispatchArgs;
    use crate::stage::Phase;
    use std::path::PathBuf;

    fn local_entry() -> RouteEntry {
        RouteEntry {
            executor: "claude".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            ssh_target: None,
            remote_workdir: None,
            effort: None,
        }
    }

    fn args() -> DispatchArgs {
        DispatchArgs {
            phase: Phase::Implement,
            task: "example-task".to_string(),
            workdir: PathBuf::from("/tmp/gal-workdir"),
            timeout_secs: 300,
            routing_path: None,
            receipt_path: None,
            log_dir_override: None,
            stdout_quiet: false,
        }
    }

    #[test]
    fn local_route_spawn_config_uses_configured_executor_not_ssh() {
        let entry = local_entry();
        let cfg = build_local_spawn_config(&entry, &args(), "spec text", &[]);
        assert_eq!(cfg.executor, "claude");
        assert_ne!(cfg.executor, "ssh");
    }

    #[test]
    fn local_route_spawn_config_preserves_task_phase_model_workdir() {
        let entry = local_entry();
        let a = args();
        let cfg = build_local_spawn_config(&entry, &a, "spec text", &[]);
        assert_eq!(cfg.task_id, "example-task");
        assert_eq!(cfg.phase, "implement");
        assert_eq!(cfg.actual_model, "claude-sonnet-4-6");
        assert_eq!(cfg.workdir, PathBuf::from("/tmp/gal-workdir"));
    }

    #[test]
    fn local_route_spawn_config_delivers_spec_on_stdin_for_stdin_adapters() {
        // claude's adapter uses stdin delivery — the spec must appear verbatim on stdin,
        // not folded into executor_args.
        let entry = local_entry();
        let cfg = build_local_spawn_config(&entry, &args(), "the task spec", &[]);
        assert_eq!(cfg.spec, "the task spec");
    }

    #[test]
    fn local_route_spawn_config_sets_default_log_dir_under_workdir() {
        let entry = local_entry();
        let cfg = build_local_spawn_config(&entry, &args(), "spec", &[]);
        assert_eq!(
            cfg.log_dir,
            SpawnConfig::default_log_dir(&PathBuf::from("/tmp/gal-workdir"))
        );
    }

    #[test]
    fn local_route_spawn_config_threads_receipt_path() {
        let entry = local_entry();
        let mut a = args();
        a.receipt_path = Some(PathBuf::from(
            "/tmp/gal-workdir/.dev/pipeline/receipts/example.receipt.md",
        ));
        let cfg = build_local_spawn_config(&entry, &a, "spec", &[]);
        assert_eq!(
            cfg.receipt_path,
            Some(PathBuf::from(
                "/tmp/gal-workdir/.dev/pipeline/receipts/example.receipt.md"
            ))
        );
    }

    #[test]
    fn unset_log_dir_override_is_byte_identical_to_default() {
        let entry = local_entry();
        let a = args();
        assert!(a.log_dir_override.is_none());
        let cfg = build_local_spawn_config(&entry, &a, "spec", &[]);
        assert_eq!(
            cfg.log_dir,
            SpawnConfig::default_log_dir(&PathBuf::from("/tmp/gal-workdir")),
            "unset log_dir_override must produce the same log_dir as before this field existed"
        );
    }

    #[test]
    fn log_dir_override_is_honored_in_composed_spawn_config() {
        let entry = local_entry();
        let mut a = args();
        a.log_dir_override = Some(PathBuf::from("/tmp/gal-workdir/.dev/executor-smoke/run-1/logs"));
        let cfg = build_local_spawn_config(&entry, &a, "spec", &[]);
        assert_eq!(
            cfg.log_dir,
            PathBuf::from("/tmp/gal-workdir/.dev/executor-smoke/run-1/logs")
        );
    }
}

#[cfg(test)]
mod remote_dispatch_branch_tests {
    use super::*;
    use crate::cli::DispatchArgs;
    use crate::stage::Phase;
    use std::path::PathBuf;

    fn remote_entry(executor: &str) -> RouteEntry {
        RouteEntry {
            executor: executor.to_string(),
            model: "claude-sonnet-4-6".to_string(),
            ssh_target: Some("user@build-box".to_string()),
            remote_workdir: Some("/home/user/gal-remote".to_string()),
            effort: None,
        }
    }

    fn args() -> DispatchArgs {
        DispatchArgs {
            phase: Phase::Implement,
            task: "example-task".to_string(),
            workdir: PathBuf::from("/tmp/gal-workdir"),
            timeout_secs: 300,
            routing_path: None,
            receipt_path: None,
            log_dir_override: None,
            stdout_quiet: false,
        }
    }

    #[test]
    fn remote_spawn_config_uses_ssh_as_the_executor() {
        let entry = remote_entry("claude");
        let cfg = build_remote_spawn_config(&entry, &args(), "spec text");
        assert_eq!(cfg.executor, "ssh");
    }

    #[test]
    fn remote_spawn_config_composes_batch_mode_target_and_remote_command() {
        let entry = remote_entry("claude");
        let cfg = build_remote_spawn_config(&entry, &args(), "spec text");
        assert_eq!(cfg.executor_args[0], "-o");
        assert_eq!(cfg.executor_args[1], "BatchMode=yes");
        assert_eq!(cfg.executor_args[2], "user@build-box");
        let remote_cmd = &cfg.executor_args[3];
        assert!(remote_cmd.starts_with("cd '/home/user/gal-remote' && 'claude'"));
    }

    #[test]
    fn remote_spawn_config_delivers_spec_on_stdin() {
        let entry = remote_entry("claude");
        let cfg = build_remote_spawn_config(&entry, &args(), "the task spec");
        assert_eq!(cfg.spec, "the task spec");
    }

    #[test]
    fn remote_spawn_config_preserves_task_phase_and_model() {
        let entry = remote_entry("codex");
        let cfg = build_remote_spawn_config(&entry, &args(), "spec");
        assert_eq!(cfg.task_id, "example-task");
        assert_eq!(cfg.phase, "implement");
        assert_eq!(cfg.actual_model, "claude-sonnet-4-6");
    }

    fn write_routing(dir: &std::path::Path, json: &str) -> PathBuf {
        let p = dir.join("routing.json");
        std::fs::write(&p, json).unwrap();
        p
    }

    fn effort_args(routing: PathBuf, workdir: PathBuf) -> DispatchArgs {
        DispatchArgs {
            phase: Phase::Audit, // role AUDITOR
            task: "example-task".to_string(),
            workdir,
            timeout_secs: 5,
            routing_path: Some(routing),
            receipt_path: None,
            log_dir_override: None,
            stdout_quiet: true,
        }
    }

    #[test]
    fn copilot_mcp_disable_missing_file_is_empty_no_warning() {
        let tmp = tempfile::TempDir::new().unwrap();
        let missing = tmp.path().join("nope.json");
        let (names, warn) = copilot_mcp_disable_names(Some(missing));
        assert!(names.is_empty());
        assert!(warn.is_none(), "missing file is a valid empty state, no warning");
        // No-home path is also empty + silent.
        assert_eq!(copilot_mcp_disable_names(None).0.len(), 0);
    }

    #[test]
    fn copilot_mcp_disable_reads_sorted_keys_no_hardcoded_names() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = tmp.path().join("mcp-config.json");
        std::fs::write(
            &cfg,
            r#"{"mcpServers":{"zeta":{"command":"z"},"alpha":{"command":"a"}}}"#,
        )
        .unwrap();
        let (names, warn) = copilot_mcp_disable_names(Some(cfg));
        assert!(warn.is_none());
        // Sorted for deterministic argv; exactly the host-config keys, nothing hardcoded.
        assert_eq!(names, vec!["alpha".to_string(), "zeta".to_string()]);
    }

    #[test]
    fn copilot_mcp_disable_malformed_is_empty_with_warning() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = tmp.path().join("mcp-config.json");
        std::fs::write(&cfg, "{ not valid json ]]]").unwrap();
        let (names, warn) = copilot_mcp_disable_names(Some(cfg));
        assert!(names.is_empty());
        assert!(
            warn.is_some_and(|w| w.contains("not valid JSON")),
            "malformed config must warn, never silently fall back"
        );
    }

    #[test]
    fn copilot_mcp_disable_absent_key_is_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = tmp.path().join("mcp-config.json");
        std::fs::write(&cfg, r#"{"other":1}"#).unwrap();
        let (names, warn) = copilot_mcp_disable_names(Some(cfg));
        assert!(names.is_empty());
        assert!(warn.is_none());
    }

    #[test]
    fn effort_syntactic_contract() {
        assert!(is_valid_effort("high"));
        assert!(is_valid_effort("x-high"));
        assert!(is_valid_effort("gpt.5_mini-2"));
        assert!(!is_valid_effort("")); // empty
        assert!(!is_valid_effort("has space"));
        assert!(!is_valid_effort("quote\"d"));
        assert!(!is_valid_effort("new\nline"));
        assert!(!is_valid_effort("semi;colon"));
    }

    #[test]
    fn effort_on_unsupporting_executor_degrades_unsupported() {
        // agy keeps the fail-closed supports_effort() default → any effort is rejected
        // before spawn (even a syntactically valid value).
        let tmp = tempfile::TempDir::new().unwrap();
        let routing = write_routing(
            tmp.path(),
            r#"{"executorRouting":{"executors":{"agy":"gemini-3.5-flash"},"pipeline":{"AUDITOR":{"executor":"agy","effort":"high"}}}}"#,
        );
        let out = run_dispatch(&effort_args(routing, tmp.path().to_path_buf()), "spec");
        assert_eq!(out.reason.as_deref(), Some("unsupported-effort"));
        assert_eq!(out.exit_code, 2);
        assert!(out.log_path.is_none(), "must not spawn / write a log");
    }

    #[test]
    fn invalid_effort_on_supporting_executor_degrades_invalid() {
        // claude supports effort, but a value with a space is not argv-safe →
        // invalid-effort before spawn.
        let tmp = tempfile::TempDir::new().unwrap();
        let routing = write_routing(
            tmp.path(),
            r#"{"executorRouting":{"executors":{"claude":"claude-sonnet-4-6"},"pipeline":{"AUDITOR":{"executor":"claude","effort":"bad value"}}}}"#,
        );
        let out = run_dispatch(&effort_args(routing, tmp.path().to_path_buf()), "spec");
        assert_eq!(out.reason.as_deref(), Some("invalid-effort"));
        assert_eq!(out.exit_code, 2);
        assert!(out.log_path.is_none(), "must not spawn / write a log");
    }

    #[test]
    fn local_route_never_composes_ssh_argv() {
        // regression: a local RouteEntry (no ssh_target) must never produce an "ssh" executor.
        let entry = RouteEntry {
            executor: "claude".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            ssh_target: None,
            remote_workdir: None,
            effort: None,
        };
        let cfg = build_local_spawn_config(&entry, &args(), "spec", &[]);
        assert_ne!(cfg.executor, "ssh");
    }
}

#[cfg(test)]
mod remote_command_tests {
    use super::*;

    #[test]
    fn shell_quote_wraps_plain_string() {
        assert_eq!(shell_quote("hello"), "'hello'");
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("it's"), r#"'it'\''s'"#);
    }

    #[test]
    fn shell_quote_handles_spaces() {
        assert_eq!(shell_quote("a b"), "'a b'");
    }

    #[test]
    fn remote_command_composes_cd_and_executor_with_args() {
        let cmd = remote_command(
            "/home/user/gal-remote",
            "claude",
            &[
                "-p".to_string(),
                "--model".to_string(),
                "claude-sonnet-4-6".to_string(),
            ],
        );
        assert_eq!(
            cmd,
            "cd '/home/user/gal-remote' && 'claude' '-p' '--model' 'claude-sonnet-4-6'"
        );
    }

    #[test]
    fn remote_command_escapes_arg_with_quote() {
        let cmd = remote_command("/wd", "claude", &["it's".to_string()]);
        assert!(cmd.contains(r#"'it'\''s'"#));
    }

    #[test]
    fn ssh_args_includes_batch_mode_and_target() {
        let args = ssh_args("user@build-box", "echo hi");
        assert_eq!(
            args,
            vec!["-o", "BatchMode=yes", "user@build-box", "echo hi"]
        );
    }

    #[test]
    fn remote_receipt_path_joins_workdir_and_relative_path() {
        assert_eq!(
            remote_receipt_path(
                "/home/user/gal-remote",
                ".dev/pipeline/receipts/example-task.receipt.md"
            ),
            "/home/user/gal-remote/.dev/pipeline/receipts/example-task.receipt.md"
        );
    }

    #[test]
    fn remote_receipt_path_trims_trailing_slash_on_workdir() {
        assert_eq!(
            remote_receipt_path("/home/user/gal-remote/", "rel.md"),
            "/home/user/gal-remote/rel.md"
        );
    }

    #[test]
    fn guard_command_checks_head_and_clean_tree() {
        let cmd = guard_command("/wd", "abc123");
        assert!(cmd.contains("git rev-parse HEAD"));
        assert!(cmd.contains("'abc123'"));
        assert!(cmd.contains("git status --porcelain"));
        assert!(cmd.contains("PARITY_OK"));
    }

    #[test]
    fn receipt_fetch_command_cats_the_remote_receipt_path() {
        let cmd = receipt_fetch_command(
            "/home/user/gal-remote",
            ".dev/pipeline/receipts/example-task-test.receipt.md",
        );
        assert_eq!(
            cmd,
            "cat '/home/user/gal-remote/.dev/pipeline/receipts/example-task-test.receipt.md'"
        );
    }

    #[test]
    fn derive_receipt_rel_path_strips_workdir_prefix() {
        let workdir = PathBuf::from("/tmp/gal-workdir");
        let receipt = PathBuf::from("/tmp/gal-workdir/.dev/pipeline/receipts/example.receipt.md");
        assert_eq!(
            derive_receipt_rel_path(&receipt, &workdir),
            Some(".dev/pipeline/receipts/example.receipt.md".to_string())
        );
    }

    #[test]
    fn local_git_head_returns_a_full_sha_inside_a_git_repo() {
        // This crate lives inside the gal git repo/worktree, so `git rev-parse HEAD`
        // resolves upward from any subdirectory — no fixture repo needed.
        let cwd = std::env::current_dir().unwrap();
        if let Some(head) = local_git_head(&cwd) {
            assert_eq!(head.len(), 40, "expected a full 40-char SHA, got: {head}");
            assert!(head.chars().all(|c| c.is_ascii_hexdigit()));
        }
        // If this ever runs outside a git checkout (e.g. an extracted release tarball),
        // `local_git_head` degrades to `None` rather than panicking — nothing to assert.
    }

    #[test]
    fn derive_receipt_rel_path_none_for_explicit_absolute_outside_workdir() {
        let workdir = PathBuf::from("/tmp/gal-workdir");
        let receipt = PathBuf::from("/etc/somewhere/else.receipt.md");
        assert_eq!(derive_receipt_rel_path(&receipt, &workdir), None);
    }

    #[test]
    fn diff_fetch_command_stages_diffs_then_resets() {
        let cmd = diff_fetch_command("/wd");
        assert!(cmd.contains("git add -A"));
        assert!(cmd.contains("git diff --cached --binary"));
        assert!(cmd.contains("git reset -q"));
    }

    #[test]
    fn cleanup_command_hard_resets_and_cleans() {
        let cmd = cleanup_command("/wd");
        assert!(cmd.contains("git reset --hard -q"));
        assert!(cmd.contains("git clean -fdq"));
    }
    #[test]
    fn sync_log_terminal_state_rewrites_completed_header() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log_path = tmp.path().join("dispatch.log");
        std::fs::write(
            &log_path,
            "GAL-DISPATCH-LOG v1\nterminal_state:  completed\n---STDOUT---\n",
        )
        .unwrap();

        sync_log_terminal_state(&log_path, &TerminalState::NoReceipt);

        let updated = std::fs::read_to_string(&log_path).unwrap();
        assert!(updated.contains("terminal_state:  no-receipt"));
        assert!(!updated.contains("terminal_state:  completed"));
    }
}

#[cfg(test)]
mod outcome_reason_tests {
    //! `DispatchOutcome.reason` carries the same machine-readable token the `Dispatch:`
    //! marker prints, so an in-process caller (the SSH executor-smoke adapter) can classify
    //! a degrade without parsing stdout. These exercise deterministic, network-free degrade
    //! paths; the remote receipt-fetch-failure reason (`remote-receipt-fetch-failed`) is
    //! exercised by the live remote path, not a unit test.
    use super::*;
    use crate::cli::DispatchArgs;
    use crate::stage::Phase;
    use std::io::Write;

    fn args_with_routing(routing_path: std::path::PathBuf) -> DispatchArgs {
        DispatchArgs {
            phase: Phase::Audit,
            task: "T-SMOKE".to_string(),
            workdir: std::env::temp_dir(),
            timeout_secs: 5,
            routing_path: Some(routing_path),
            receipt_path: None,
            log_dir_override: None,
            stdout_quiet: true,
        }
    }

    #[test]
    fn no_routing_entry_reason_is_no_routing() {
        // An empty routing table → no AUDITOR entry → degrade with reason `no-routing`.
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"{ \"executorRouting\": {} }").unwrap();
        let outcome = run_dispatch(&args_with_routing(f.path().to_path_buf()), "spec");
        assert_eq!(outcome.exit_code, 2);
        assert!(outcome.terminal_state.is_none());
        assert_eq!(outcome.reason.as_deref(), Some("no-routing"));
    }

    #[test]
    fn half_configured_remote_route_reason_is_remote_missing_workdir() {
        // sshTarget without remoteWorkdir → fail loud before any ssh spawn, reason carried.
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(
            br#"{ "executorRouting": {
                "executors": { "claude": "claude-sonnet-4-6" },
                "pipeline": { "AUDITOR": { "executor": "claude", "sshTarget": "user@remote" } }
            } }"#,
        )
        .unwrap();
        let outcome = run_dispatch(&args_with_routing(f.path().to_path_buf()), "spec");
        assert_eq!(outcome.exit_code, 2);
        assert_eq!(outcome.reason.as_deref(), Some("remote-missing-workdir"));
    }
}
