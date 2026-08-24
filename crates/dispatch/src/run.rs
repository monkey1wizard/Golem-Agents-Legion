//! In-process dispatch flow shared by the `gal-dispatch` bin and `gal pipeline`.
//!
//! Extracted from the bin's `main` so the single `gal` binary can run a headless
//! dispatch without spawning a separate `gal-dispatch` executable (which is not in
//! the end-user release artifact). The safety gate (routing present +
//! executor available + authenticated) lives here, so both entry points share it.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::cli::DispatchArgs;
use crate::dispatch::{
    executor_readiness, is_available, prepare_receipt_path, render_dispatch_marker_suffix,
    render_routed_dispatch_marker_suffix, spawn_executor, DispatchError, PreparedReceipt,
    Readiness, SpawnConfig, TerminalState, RECEIPT_LEASE_CLEANUP_MARKER,
};

const REMOTE_RECEIPT_FRESHNESS_EXIT: i32 = 73;
const REMOTE_RECEIPT_FRESHNESS_MARKER: &str = "GAL_REMOTE_RECEIPT_FRESHNESS_FAILED";
static REMOTE_RECEIPT_LEASE_SEQ: AtomicU64 = AtomicU64::new(0);
const REMOTE_RECEIPT_POST_TIMEOUT: Duration = Duration::from_secs(30);
const REMOTE_RECEIPT_OUTPUT_LIMIT: usize = 1024 * 1024;
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

enum BoundedCommandOutput {
    Completed {
        success: bool,
        stdout: Vec<u8>,
        stdout_overflowed: bool,
        stdout_read_failed: bool,
    },
    TimedOut {
        cleanup_confirmed: bool,
    },
    Failed {
        cleanup_confirmed: bool,
    },
    SpawnFailed,
}

#[derive(Default)]
struct CappedDrain {
    bytes: Vec<u8>,
    overflowed: bool,
    read_failed: bool,
}

fn drain_capped<R: std::io::Read>(mut reader: R, limit: usize) -> CappedDrain {
    let mut drained = CappedDrain {
        bytes: Vec::with_capacity(limit.min(8192)),
        overflowed: false,
        read_failed: false,
    };
    let mut chunk = [0_u8; 8192];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Err(_) => {
                drained.read_failed = true;
                break;
            }
            Ok(read) => {
                let retained = read.min(limit.saturating_sub(drained.bytes.len()));
                drained.bytes.extend_from_slice(&chunk[..retained]);
                drained.overflowed |= retained < read;
            }
        }
    }
    drained
}

fn append_post_reason(slot: &mut Option<String>, reason: &str) {
    match slot {
        Some(existing) if existing.split('+').any(|item| item == reason) => {}
        Some(existing) => {
            existing.push('+');
            existing.push_str(reason);
        }
        None => *slot = Some(reason.to_string()),
    }
}

/// Capture a bounded auxiliary command without risking pipe-buffer deadlock.
/// Reader threads drain concurrently; on timeout the local process is killed.
fn bounded_command_output(
    mut command: std::process::Command,
    timeout: Duration,
) -> BoundedCommandOutput {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let Ok(mut child) = command.spawn() else {
        return BoundedCommandOutput::SpawnFailed;
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_reader = std::thread::spawn(move || {
        stdout
            .map(|pipe| drain_capped(pipe, REMOTE_RECEIPT_OUTPUT_LIMIT))
            .unwrap_or_default()
    });
    let stderr_reader = std::thread::spawn(move || {
        stderr
            .map(|pipe| drain_capped(pipe, REMOTE_RECEIPT_OUTPUT_LIMIT))
            .unwrap_or_default()
    });

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_reader.join().unwrap_or_default();
                let _ = stderr_reader.join();
                return BoundedCommandOutput::Completed {
                    success: status.success(),
                    stdout: stdout.bytes,
                    stdout_overflowed: stdout.overflowed,
                    stdout_read_failed: stdout.read_failed,
                };
            }
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let cleanup_confirmed = child.kill().is_ok()
                    && wait_for_bounded_child_exit(&mut child, Duration::from_secs(1));
                if cleanup_confirmed {
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                }
                return BoundedCommandOutput::TimedOut { cleanup_confirmed };
            }
            Err(_) => {
                let cleanup_confirmed = child.kill().is_ok()
                    && wait_for_bounded_child_exit(&mut child, Duration::from_secs(1));
                if cleanup_confirmed {
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                }
                return BoundedCommandOutput::Failed { cleanup_confirmed };
            }
        }
    }
}

fn wait_for_bounded_child_exit(child: &mut std::process::Child, timeout: Duration) -> bool {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) | Err(_) => return false,
        }
    }
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

    // Pre-route-resolution suffix: provenance only, no `effort` field (no route is
    // known yet). Used only by the `no-routing` marker below, which by definition
    // never resolves an entry. Shadowed with the routed variant immediately after
    // route resolution — see the comment there — so every later marker in this run
    // carries the same routed effort and can never drift from
    // `SpawnConfig.contract_provenance`, which is propagated from this same `args` value.
    let marker_suffix = render_dispatch_marker_suffix(&args.contract_provenance);

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
                    "Dispatch: phase={} task={} role={} executor=none reason=no-routing{marker_suffix}",
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

    // Computed once, right after route resolution, and reused for every later
    // success/degrade marker below so every marker in this run reports the same
    // routed effort — see `render_routed_dispatch_marker_suffix`.
    let marker_suffix =
        render_routed_dispatch_marker_suffix(entry.effort.as_deref(), &args.contract_provenance);

    let executor_name = &entry.executor;
    let model = &entry.model;

    // ── half-configured sshTarget/remoteWorkdir → fail loud (never silently run local) ──
    if entry.ssh_target.is_some() != entry.remote_workdir.is_some() {
        emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
        emit_line(
            args.stdout_quiet,
            &format!(
                "Dispatch: phase={} task={} role={} executor={} model={} reason=remote-missing-workdir{marker_suffix}",
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
                    "Dispatch: phase={} task={} role={} executor={} model={} reason=unsupported-effort{marker_suffix}",
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
                    "Dispatch: phase={} task={} role={} executor={} model={} reason=invalid-effort{marker_suffix}",
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

    // Remote dispatch prepares the control-node destination here and retains
    // its lease until the remote bytes have been fetched and written.
    let mut remote_local_receipt: Option<PreparedReceipt> = None;
    let mut remote_receipt_lease: Option<RemoteReceiptLease> = None;
    let cfg = if entry.is_remote() {
        // ── remote branch: gate on `ssh`, not the local executor (F1) ──
        if !is_available("ssh") {
            emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
            emit_line(
                args.stdout_quiet,
                &format!(
                    "Dispatch: phase={} task={} role={} executor=ssh model={} reason=executor-unavailable{marker_suffix}",
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
                    "Dispatch: phase={} task={} role={} executor={} model={} reason=remote-cliflag-unsupported{marker_suffix}",
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
                        "Dispatch: phase={} task={} role={} executor={} model={} reason=remote-absolute-receipt-unsupported{marker_suffix}",
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
            remote_local_receipt = match prepare_receipt_path(&args.workdir, local_receipt) {
                Ok(prepared) => Some(prepared),
                Err(error) => {
                    emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
                    emit_line(
                        args.stdout_quiet,
                        &format!(
                            "Dispatch: phase={} task={} role={} executor={} model={} reason=receipt-preparation-failed{marker_suffix}",
                            args.phase.as_str(),
                            args.task,
                            role,
                            executor_name,
                            model,
                        ),
                    );
                    eprintln!("gal dispatch: {error}");
                    return degraded(2, "receipt-preparation-failed");
                }
            };
            remote_receipt_lease =
                derive_receipt_rel_path(local_receipt, &args.workdir).map(RemoteReceiptLease::new);
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
                let cleanup_error = remote_local_receipt
                    .as_mut()
                    .and_then(|prepared| prepared.release_checked().err());
                let reason = if cleanup_error.is_some() {
                    "remote-guard-failed+receipt-lease-cleanup-failed"
                } else {
                    "remote-guard-failed"
                };
                emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
                emit_line(
                    args.stdout_quiet,
                    &format!(
                        "Dispatch: phase={} task={} role={} executor={} model={} reason={reason}{marker_suffix}",
                        args.phase.as_str(),
                        args.task,
                        role,
                        executor_name,
                        model,
                    ),
                );
                if let Some(error) = cleanup_error {
                    eprintln!("gal dispatch: {error}");
                } else {
                    eprintln!("gal dispatch: remote checkout at '{remote_workdir}' is not at the control node's HEAD, is not clean, or the guard could not be evaluated; re-sync the remote checkout before retrying");
                }
                return degraded(2, reason);
            }
        }

        build_remote_spawn_config_with_lease(entry, args, spec, remote_receipt_lease.as_ref())
    } else {
        // ── local branch: gate on the configured executor (unchanged baseline) ──
        if !is_available(executor_name) {
            emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
            emit_line(
                args.stdout_quiet,
                &format!(
                    "Dispatch: phase={} task={} role={} executor={} model={} reason=executor-unavailable{marker_suffix}",
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
                        "Dispatch: phase={} task={} role={} executor={} model={} reason=executor-unauthenticated-confirmed{marker_suffix}",
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
            let cleanup_error = remote_local_receipt
                .as_mut()
                .and_then(|prepared| prepared.release_checked().err());
            let primary_reason = if matches!(
                e,
                DispatchError::ReceiptPrepare(_) | DispatchError::ReceiptPrepareCleanup(_)
            ) {
                "receipt-preparation-failed"
            } else {
                "log-error"
            };
            let cleanup_failed = cleanup_error.is_some()
                || matches!(
                    e,
                    DispatchError::ReceiptCleanup(_) | DispatchError::ReceiptPrepareCleanup(_)
                );
            let reason = if cleanup_failed {
                format!("{primary_reason}+receipt-lease-cleanup-failed")
            } else {
                primary_reason.to_string()
            };
            eprintln!("gal dispatch: {e}");
            if let Some(error) = cleanup_error {
                eprintln!("gal dispatch: {error}");
            }
            emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
            emit_line(
                args.stdout_quiet,
                &format!(
                    "Dispatch: phase={} task={} executor={} model={} reason={reason}{marker_suffix}",
                    args.phase.as_str(),
                    args.task,
                    executor_name,
                    model,
                ),
            );
            return degraded(2, &reason);
        }
    };

    // Reason carried into the final `DispatchOutcome` for the caller to classify remote
    // post-processing outcomes that downgrade `terminal_state` after the spawn. Left `None`
    // on a clean completed run; set to `remote-receipt-fetch-failed` when the receipt-fetch
    // step below fails, so a caller can tell that apart from a genuinely missing receipt.
    let mut post_reason: Option<String> = None;
    if result.stderr.contains(RECEIPT_LEASE_CLEANUP_MARKER) {
        append_post_reason(&mut post_reason, "receipt-lease-cleanup-failed");
    }
    let remote_freshness_failed = is_remote_dispatch
        && result.exit_code == Some(REMOTE_RECEIPT_FRESHNESS_EXIT)
        && result.stderr.contains(REMOTE_RECEIPT_FRESHNESS_MARKER);
    if remote_freshness_failed {
        append_post_reason(&mut post_reason, "remote-receipt-freshness-failed");
    }
    // F4: for a remote dispatch, the receipt (if requested) was written on the *remote*
    // filesystem, never checked by `spawn_executor` (its `SpawnConfig.receipt_path` is always
    // `None` on the remote branch — see `build_remote_spawn_config`). Fetch the remote bytes
    // over ssh into the local receipt path here, then apply the same non-empty-file rule
    // `verify_receipt` uses. Only meaningful when the remote run itself completed cleanly.
    if is_remote_dispatch && result.terminal_state == TerminalState::Completed {
        if let (Some(prepared), Some(configured_receipt), Some(target), Some(remote_workdir)) = (
            &remote_local_receipt,
            &args.receipt_path,
            &entry.ssh_target,
            &entry.remote_workdir,
        ) {
            // Already validated as workdir-relative above; re-derive here rather than
            // threading extra state through `cfg`.
            let receipt_rel =
                derive_receipt_rel_path(configured_receipt, &args.workdir).unwrap_or_default();
            let fetch_cmd = receipt_fetch_command(remote_workdir, &receipt_rel);
            let fetch_argv = ssh_args(target, &fetch_cmd);
            let mut fetch_process = std::process::Command::new("ssh");
            fetch_process.args(&fetch_argv);
            let fetched = bounded_command_output(fetch_process, REMOTE_RECEIPT_POST_TIMEOUT);
            let fetched_ok = match fetched {
                BoundedCommandOutput::Completed {
                    success: true,
                    stdout,
                    stdout_overflowed: false,
                    stdout_read_failed: false,
                } if !stdout.is_empty() => prepared.write_fetched(&stdout).is_ok(),
                BoundedCommandOutput::Completed {
                    stdout_overflowed: true,
                    ..
                } => {
                    append_post_reason(&mut post_reason, "remote-receipt-fetch-too-large");
                    false
                }
                BoundedCommandOutput::Completed {
                    stdout_read_failed: true,
                    ..
                } => {
                    append_post_reason(&mut post_reason, "remote-receipt-fetch-read-failed");
                    false
                }
                BoundedCommandOutput::TimedOut {
                    cleanup_confirmed: true,
                } => {
                    append_post_reason(&mut post_reason, "remote-receipt-fetch-timeout");
                    false
                }
                BoundedCommandOutput::TimedOut {
                    cleanup_confirmed: false,
                } => {
                    append_post_reason(
                        &mut post_reason,
                        "remote-receipt-fetch-timeout-unconfirmed",
                    );
                    false
                }
                BoundedCommandOutput::Failed {
                    cleanup_confirmed: false,
                } => {
                    append_post_reason(&mut post_reason, "remote-receipt-fetch-failed-unconfirmed");
                    false
                }
                _ => false,
            };
            if !fetched_ok {
                result.terminal_state = TerminalState::NoReceipt;
                if !matches!(
                    post_reason.as_deref(),
                    Some(
                        "remote-receipt-fetch-timeout"
                            | "remote-receipt-fetch-timeout-unconfirmed"
                            | "remote-receipt-fetch-failed-unconfirmed"
                            | "remote-receipt-fetch-read-failed"
                            | "remote-receipt-fetch-too-large"
                    )
                ) {
                    append_post_reason(&mut post_reason, "remote-receipt-fetch-failed");
                }
            }
        }
    }

    // The remote lease intentionally outlives the executor SSH process: only
    // release it after the separate receipt-fetch round trip has completed.
    // Owner-token comparison prevents a failed contender from removing the
    // active dispatch's lock. A crash leaves a stale lock and therefore fails
    // later attempts closed until an operator inspects/removes it.
    if is_remote_dispatch && !remote_freshness_failed && result.exit_code.is_some() {
        if let (Some(lease), Some(target), Some(remote_workdir)) = (
            &remote_receipt_lease,
            &entry.ssh_target,
            &entry.remote_workdir,
        ) {
            let cleanup_argv = ssh_args(target, &lease.cleanup_command(remote_workdir));
            let mut cleanup_process = std::process::Command::new("ssh");
            cleanup_process.args(&cleanup_argv);
            let cleanup_reason =
                match bounded_command_output(cleanup_process, REMOTE_RECEIPT_POST_TIMEOUT) {
                    BoundedCommandOutput::Completed { success: true, .. } => None,
                    BoundedCommandOutput::TimedOut {
                        cleanup_confirmed: true,
                    } => Some("remote-receipt-lease-cleanup-timeout"),
                    BoundedCommandOutput::TimedOut {
                        cleanup_confirmed: false,
                    } => Some("remote-receipt-lease-cleanup-timeout-unconfirmed"),
                    BoundedCommandOutput::Failed {
                        cleanup_confirmed: false,
                    } => Some("remote-receipt-lease-cleanup-failed-unconfirmed"),
                    _ => Some("remote-receipt-lease-cleanup-failed"),
                };
            if let Some(reason) = cleanup_reason {
                result.terminal_state = TerminalState::DisconnectedPartial;
                append_post_reason(&mut post_reason, reason);
            }
        }
    }

    if let Some(prepared) = remote_local_receipt.as_mut() {
        if let Err(error) = prepared.release_checked() {
            result.terminal_state = TerminalState::DisconnectedPartial;
            let evidence = format!("{RECEIPT_LEASE_CLEANUP_MARKER}: {error}");
            if !result.stderr.is_empty() {
                result.stderr.push('\n');
            }
            result.stderr.push_str(&evidence);
            eprintln!("gal dispatch: {evidence}");
            append_post_reason(&mut post_reason, "receipt-lease-cleanup-failed");
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
        sync_log_postprocessing(&result.log_path, &result);
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
    let reason_suffix = post_reason
        .as_deref()
        .map(|reason| format!(" reason={reason}"))
        .unwrap_or_default();

    emit_line(args.stdout_quiet, "--- GAL DISPATCH ---");
    emit_line(
        args.stdout_quiet,
        &format!(
            "Dispatch: phase={} task={} role={} executor={} model={} state={} session_id={} log={}{}{marker_suffix}",
            args.phase.as_str(),
            args.task,
            role,
            executor_name,
            model,
            result.terminal_state,
            session_str,
            result.log_path.display(),
            reason_suffix,
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
/// for copilot, empty for every other executor). `entry.timeout_secs`, when set, overrides
/// `args.timeout_secs` (the incoming CLI/dispatch-script default); absent, the incoming
/// value passes through unchanged. A route with `is_remote()` true takes a separate
/// remote-composition path.
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
        timeout_secs: entry.timeout_secs.unwrap_or(args.timeout_secs),
        task_id: args.task.clone(),
        phase: args.phase.as_str().to_string(),
        actual_model: model.clone(),
        log_dir: args
            .log_dir_override
            .clone()
            .unwrap_or_else(|| SpawnConfig::default_log_dir(&args.workdir)),
        receipt_path: args.receipt_path.clone(),
        contract_provenance: args.contract_provenance.clone(),
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

fn sync_log_postprocessing(log_path: &std::path::Path, result: &crate::dispatch::DispatchResult) {
    let Ok(content) = std::fs::read_to_string(log_path) else {
        return;
    };
    let mut changed = false;
    let mut updated = String::with_capacity(content.len());
    for line in content.lines() {
        if line.starts_with("terminal_state:") {
            updated.push_str(&format!("terminal_state:  {}", result.terminal_state));
            changed = true;
        } else if line == "---STDERR---" {
            updated.push_str(line);
            updated.push('\n');
            updated.push_str(&result.stderr);
            if !result.stderr.ends_with('\n') {
                updated.push('\n');
            }
            changed = true;
            break;
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
/// `entry.timeout_secs`, when set, overrides `args.timeout_secs` the same way the local
/// path does.
pub fn build_remote_spawn_config(
    entry: &RouteEntry,
    args: &DispatchArgs,
    spec: &str,
) -> SpawnConfig {
    let lease = args
        .receipt_path
        .as_deref()
        .and_then(|path| derive_receipt_rel_path(path, &args.workdir))
        .map(RemoteReceiptLease::new);
    build_remote_spawn_config_with_lease(entry, args, spec, lease.as_ref())
}

fn build_remote_spawn_config_with_lease(
    entry: &RouteEntry,
    args: &DispatchArgs,
    spec: &str,
    receipt_lease: Option<&RemoteReceiptLease>,
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

    let remote_cmd = remote_command_with_receipt_lease(
        remote_workdir,
        executor_name,
        &executor_args,
        receipt_lease,
    );
    let ssh_argv = ssh_args(target, &remote_cmd);

    SpawnConfig {
        executor: "ssh".to_string(),
        executor_args: ssh_argv,
        spec: spec.to_string(),
        workdir: args.workdir.clone(),
        timeout_secs: entry.timeout_secs.unwrap_or(args.timeout_secs),
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
        contract_provenance: args.contract_provenance.clone(),
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

/// Compose the remote executor command with a same-shell receipt freshness
/// precondition. Keeping invalidation and spawn in one remote shell prevents a
/// second SSH round trip from reopening a stale-receipt window.
pub fn remote_command_with_receipt_freshness(
    workdir: &str,
    executor: &str,
    args: &[String],
    receipt_rel: Option<&str>,
) -> String {
    let lease = receipt_rel.map(RemoteReceiptLease::new);
    remote_command_with_receipt_lease(workdir, executor, args, lease.as_ref())
}

#[derive(Debug)]
struct RemoteReceiptLease {
    rel: String,
    lock_rel: String,
    owner_token: String,
}

impl RemoteReceiptLease {
    fn new(rel: impl Into<String>) -> Self {
        let rel = rel.into();
        let lock_rel = format!(
            ".dev/pipeline/receipts/.locks/{:016x}.lockdir",
            remote_receipt_lock_hash(&rel)
        );
        let sequence = REMOTE_RECEIPT_LEASE_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let owner_token = format!("{:x}-{nanos:x}-{sequence:x}", std::process::id());
        Self {
            rel,
            lock_rel,
            owner_token,
        }
    }

    fn owner_rel(&self) -> String {
        format!("{}/owner", self.lock_rel)
    }

    fn cleanup_command(&self, workdir: &str) -> String {
        let owner_rel = self.owner_rel();
        format!(
            "cd {wd} && test \"$(cat {owner} 2>/dev/null)\" = {token} && rm -f -- {owner} && rmdir -- {lock}",
            wd = shell_quote(workdir),
            owner = shell_quote(&owner_rel),
            token = shell_quote(&self.owner_token),
            lock = shell_quote(&self.lock_rel),
        )
    }
}

fn remote_command_with_receipt_lease(
    workdir: &str,
    executor: &str,
    args: &[String],
    lease: Option<&RemoteReceiptLease>,
) -> String {
    let executor_cmd = remote_command(workdir, executor, args);
    let Some(lease) = lease else {
        return executor_cmd;
    };
    let fail_evidence = format!("{REMOTE_RECEIPT_FRESHNESS_MARKER} lock={}", lease.lock_rel);
    let fail = format!(
        "printf '%s\\n' {} >&2; exit {REMOTE_RECEIPT_FRESHNESS_EXIT}",
        shell_quote(&fail_evidence)
    );
    let Some(parts) = safe_remote_relative_components(&lease.rel) else {
        return fail;
    };
    if parts.starts_with(&[".dev", "pipeline", "receipts", ".locks"]) {
        return fail;
    }

    let quoted_rel = shell_quote(&lease.rel);
    let mut receipt_checks = remote_link_checks(&parts);
    if parts.starts_with(&[".dev", "pipeline", "receipts"]) && parts.len() > 3 {
        receipt_checks.push(format!("rm -f -- {quoted_rel}"));
    } else {
        receipt_checks.push(format!("test ! -e {quoted_rel}"));
    }
    let freshness = receipt_checks.join(" && ");

    let lock_root = ".dev/pipeline/receipts/.locks";
    let lock_parts = [".dev", "pipeline", "receipts", ".locks"];
    let mut lease_checks = remote_link_checks(&lock_parts);
    lease_checks.insert(3, format!("mkdir -p -- {}", shell_quote(lock_root)));
    let lease_precondition = lease_checks.join(" && ");
    let owner_rel = lease.owner_rel();
    let abandon = format!(
        "rm -f -- {owner}; rmdir -- {lock}",
        owner = shell_quote(&owner_rel),
        lock = shell_quote(&lease.lock_rel),
    );
    let acquire = format!(
        "if mkdir -- {lock}; then printf '%s\\n' {token} > {owner} || {{ {abandon}; {fail}; }}; else {fail}; fi",
        lock = shell_quote(&lease.lock_rel),
        token = shell_quote(&lease.owner_token),
        owner = shell_quote(&owner_rel),
    );

    let executor_tail = executor_cmd
        .strip_prefix(&format!("cd {} && ", shell_quote(workdir)))
        .unwrap_or(&executor_cmd);
    format!(
        "cd {wd} && {{ {lease_precondition} || {{ {fail}; }}; }} && {acquire} && {{ {freshness} || {{ {abandon}; {fail}; }}; }} && {executor_tail}",
        wd = shell_quote(workdir),
    )
}

fn remote_link_checks(parts: &[&str]) -> Vec<String> {
    let mut checks = Vec::new();
    let mut prefix = String::new();
    for part in parts {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
        checks.push(format!("test ! -L {}", shell_quote(&prefix)));
    }
    checks
}

fn remote_receipt_lock_hash(rel: &str) -> u64 {
    rel.as_bytes()
        .iter()
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

fn safe_remote_relative_components(rel: &str) -> Option<Vec<&str>> {
    let parts: Vec<_> = rel.split('/').collect();
    if parts.is_empty()
        || parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return None;
    }
    Some(parts)
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
    let Some(parts) = safe_remote_relative_components(rel) else {
        return "false".to_string();
    };
    let mut checks = remote_link_checks(&parts);
    checks.push(format!("test -f {}", shell_quote(rel)));
    checks.push(format!("cat -- {}", shell_quote(rel)));
    format!(
        "cd {} && {}",
        shell_quote(remote_workdir),
        checks.join(" && ")
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
    let rel = match receipt_path.strip_prefix(workdir) {
        Ok(rel) => rel,
        Err(_) if receipt_path.is_absolute() || receipt_path.to_string_lossy().starts_with('/') => {
            return None;
        }
        Err(_) => receipt_path,
    };
    Some(rel.to_string_lossy().replace('\\', "/"))
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
            timeout_secs: None,
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
            contract_provenance: None,
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
        a.log_dir_override = Some(PathBuf::from(
            "/tmp/gal-workdir/.dev/executor-smoke/run-1/logs",
        ));
        let cfg = build_local_spawn_config(&entry, &a, "spec", &[]);
        assert_eq!(
            cfg.log_dir,
            PathBuf::from("/tmp/gal-workdir/.dev/executor-smoke/run-1/logs")
        );
    }

    #[test]
    fn contract_provenance_reaches_local_and_remote_spawn_config_identically() {
        use crate::dispatch::ContractProvenance;

        let provenance = ContractProvenance {
            path: PathBuf::from(
                "/tmp/gal-workdir/plugins/gal-core/agents/golem-implementer.agent.md",
            ),
            source: "workdir".to_string(),
        };

        let mut a = args();
        a.contract_provenance = Some(provenance.clone());

        let local_entry = local_entry();
        let local_cfg = build_local_spawn_config(&local_entry, &a, "spec", &[]);
        assert_eq!(local_cfg.contract_provenance, Some(provenance.clone()));

        let remote_entry = RouteEntry {
            executor: "claude".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            ssh_target: Some("user@build-box".to_string()),
            remote_workdir: Some("/home/user/gal-remote".to_string()),
            effort: None,
            timeout_secs: None,
        };
        let remote_cfg = build_remote_spawn_config(&remote_entry, &a, "spec");
        assert_eq!(remote_cfg.contract_provenance, Some(provenance));
    }

    #[test]
    fn local_spawn_config_preserves_incoming_timeout_when_route_has_none() {
        let entry = local_entry();
        assert_eq!(entry.timeout_secs, None);
        let cfg = build_local_spawn_config(&entry, &args(), "spec", &[]);
        assert_eq!(cfg.timeout_secs, 300);
    }

    #[test]
    fn local_spawn_config_uses_route_timeout_override_when_set() {
        let mut entry = local_entry();
        entry.timeout_secs = Some(900);
        let cfg = build_local_spawn_config(&entry, &args(), "spec", &[]);
        assert_eq!(cfg.timeout_secs, 900);
    }

    #[test]
    fn remote_spawn_config_uses_route_timeout_override_when_set() {
        let mut entry = RouteEntry {
            executor: "claude".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            ssh_target: Some("user@build-box".to_string()),
            remote_workdir: Some("/home/user/gal-remote".to_string()),
            effort: None,
            timeout_secs: None,
        };
        let cfg = build_remote_spawn_config(&entry, &args(), "spec");
        assert_eq!(cfg.timeout_secs, 300);

        entry.timeout_secs = Some(120);
        let cfg = build_remote_spawn_config(&entry, &args(), "spec");
        assert_eq!(cfg.timeout_secs, 120);
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
            timeout_secs: None,
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
            contract_provenance: None,
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
    fn remote_spawn_config_places_receipt_freshness_before_executor() {
        let entry = remote_entry("claude");
        let mut dispatch_args = args();
        dispatch_args.receipt_path = Some(PathBuf::from(
            "/tmp/gal-workdir/.dev/pipeline/receipts/plan/current-task-test.receipt.md",
        ));
        let cfg = build_remote_spawn_config(&entry, &dispatch_args, "spec text");
        let remote_cmd = &cfg.executor_args[3];
        let invalidation = remote_cmd.find("rm -f --").unwrap();
        let executor = remote_cmd.find("'claude'").unwrap();
        assert!(invalidation < executor);
        assert!(remote_cmd.contains(REMOTE_RECEIPT_FRESHNESS_MARKER));
        assert!(remote_cmd.contains("mkdir -- '.dev/pipeline/receipts/.locks/"));
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
            contract_provenance: None,
        }
    }

    #[test]
    fn copilot_mcp_disable_missing_file_is_empty_no_warning() {
        let tmp = tempfile::TempDir::new().unwrap();
        let missing = tmp.path().join("nope.json");
        let (names, warn) = copilot_mcp_disable_names(Some(missing));
        assert!(names.is_empty());
        assert!(
            warn.is_none(),
            "missing file is a valid empty state, no warning"
        );
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
            timeout_secs: None,
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
    fn managed_remote_receipt_is_invalidated_before_executor() {
        let cmd = remote_command_with_receipt_freshness(
            "/wd",
            "claude",
            &["--model".to_string(), "sonnet".to_string()],
            Some(".dev/pipeline/receipts/plan/current-task-test.receipt.md"),
        );
        let symlink_check = cmd.find("test ! -L '.dev/pipeline/receipts/plan'").unwrap();
        let invalidation = cmd
            .find("rm -f -- '.dev/pipeline/receipts/plan/current-task-test.receipt.md'")
            .unwrap();
        let executor = cmd.find("'claude' '--model' 'sonnet'").unwrap();
        assert!(symlink_check < invalidation && invalidation < executor);
        assert!(cmd.contains(REMOTE_RECEIPT_FRESHNESS_MARKER));
    }

    #[test]
    fn external_remote_receipt_requires_nonexistence_before_executor() {
        let cmd = remote_command_with_receipt_freshness(
            "/wd",
            "claude",
            &[],
            Some("evidence/custom.receipt.md"),
        );
        let guard = cmd.find("test ! -e 'evidence/custom.receipt.md'").unwrap();
        let executor = cmd.find("'claude'").unwrap();
        assert!(guard < executor);
        assert!(!cmd.contains("rm -f -- 'evidence/custom.receipt.md'"));
        assert!(cmd.contains("test ! -L 'evidence'"));
        assert!(cmd.contains("test ! -L 'evidence/custom.receipt.md'"));
    }

    #[test]
    fn unsafe_remote_receipt_path_fails_before_executor_composition() {
        let cmd = remote_command_with_receipt_freshness(
            "/wd",
            "claude",
            &[],
            Some(".dev/pipeline/receipts/../../outside.receipt.md"),
        );
        assert!(cmd.contains(REMOTE_RECEIPT_FRESHNESS_MARKER));
        assert!(cmd.contains("exit 73"));
        assert!(!cmd.contains("claude"));
    }

    #[test]
    fn remote_receipt_lease_is_atomic_and_persists_past_executor() {
        let lease = RemoteReceiptLease::new(".dev/pipeline/receipts/plan/task.md");
        let cmd = remote_command_with_receipt_lease("/wd", "claude", &[], Some(&lease));
        let acquire = cmd.find(&format!("mkdir -- '{}'", lease.lock_rel)).unwrap();
        let invalidate = cmd
            .find("rm -f -- '.dev/pipeline/receipts/plan/task.md'")
            .unwrap();
        let executor = cmd.find("'claude'").unwrap();
        assert!(acquire < invalidate && invalidate < executor);
        assert!(!cmd.contains("trap "));
        assert!(cmd.contains(&format!("lock={}", lease.lock_rel)));
        let cleanup = lease.cleanup_command("/wd");
        assert!(cleanup.contains(&lease.owner_token));
        assert!(cleanup.contains(&lease.lock_rel));
    }

    #[test]
    fn executor_exit_73_is_preserved_exactly() {
        let cmd = remote_command_with_receipt_freshness(
            "/wd",
            "claude",
            &[],
            Some(".dev/pipeline/receipts/plan/task.md"),
        );
        assert!(cmd.ends_with("'claude'"));
        assert!(!cmd.contains("exit 74"));
    }

    #[test]
    fn absent_receipt_keeps_remote_command_byte_compatible() {
        let baseline = remote_command("/wd", "claude", &["arg".to_string()]);
        assert_eq!(
            remote_command_with_receipt_freshness("/wd", "claude", &["arg".to_string()], None),
            baseline
        );
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
    fn receipt_fetch_command_rechecks_links_and_regular_file_before_cat() {
        let cmd = receipt_fetch_command(
            "/home/user/gal-remote",
            ".dev/pipeline/receipts/example-task-test.receipt.md",
        );
        let parent_guard = cmd.find("test ! -L '.dev/pipeline/receipts'").unwrap();
        let final_guard = cmd
            .find("test ! -L '.dev/pipeline/receipts/example-task-test.receipt.md'")
            .unwrap();
        let regular = cmd
            .find("test -f '.dev/pipeline/receipts/example-task-test.receipt.md'")
            .unwrap();
        let cat = cmd
            .find("cat -- '.dev/pipeline/receipts/example-task-test.receipt.md'")
            .unwrap();
        assert!(cmd.starts_with("cd '/home/user/gal-remote'"));
        assert!(parent_guard < final_guard && final_guard < regular && regular < cat);
    }

    #[test]
    fn bounded_command_output_completes_and_times_out() {
        #[cfg(target_os = "windows")]
        let success = {
            let mut command = std::process::Command::new("cmd");
            command.args(["/C", "echo ok"]);
            command
        };
        #[cfg(not(target_os = "windows"))]
        let success = {
            let mut command = std::process::Command::new("sh");
            command.args(["-c", "printf ok"]);
            command
        };
        assert!(matches!(
            bounded_command_output(success, Duration::from_secs(2)),
            BoundedCommandOutput::Completed { success: true, .. }
        ));

        #[cfg(target_os = "windows")]
        let timeout = {
            let mut command = std::process::Command::new("cmd");
            command.args(["/C", "ping -n 3 127.0.0.1 >NUL"]);
            command
        };
        #[cfg(not(target_os = "windows"))]
        let timeout = {
            let mut command = std::process::Command::new("sh");
            command.args(["-c", "sleep 2"]);
            command
        };
        assert!(matches!(
            bounded_command_output(timeout, Duration::from_millis(20)),
            BoundedCommandOutput::TimedOut { .. }
        ));
    }

    #[test]
    fn capped_drain_continues_reading_but_bounds_retained_bytes() {
        let below = drain_capped(std::io::Cursor::new(vec![7_u8; 8]), 16);
        assert_eq!(below.bytes.len(), 8);
        assert!(!below.overflowed);

        let above = drain_capped(std::io::Cursor::new(vec![9_u8; 32]), 16);
        assert_eq!(above.bytes.len(), 16);
        assert!(above.overflowed);
        assert!(!above.read_failed);
    }

    #[test]
    fn capped_drain_distinguishes_read_error_from_clean_eof() {
        struct FailingReader(bool);
        impl std::io::Read for FailingReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if self.0 {
                    return Err(std::io::Error::other("synthetic read failure"));
                }
                self.0 = true;
                buffer[..4].copy_from_slice(b"part");
                Ok(4)
            }
        }

        let partial = drain_capped(FailingReader(false), 16);
        assert_eq!(partial.bytes, b"part");
        assert!(partial.read_failed);
    }

    #[test]
    fn post_reasons_preserve_primary_and_cleanup_failures() {
        let mut reason = None;
        append_post_reason(&mut reason, "remote-receipt-fetch-failed");
        append_post_reason(&mut reason, "remote-receipt-lease-cleanup-failed");
        assert_eq!(
            reason.as_deref(),
            Some("remote-receipt-fetch-failed+remote-receipt-lease-cleanup-failed")
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
    fn derive_receipt_rel_path_preserves_relative_receipt() {
        let workdir = PathBuf::from("/tmp/gal-workdir");
        let receipt = PathBuf::from(".dev/pipeline/receipts/example.receipt.md");
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
    fn preexisting_external_local_receipt_is_detected_without_touching_it() {
        let tmp = tempfile::TempDir::new().unwrap();
        let managed = tmp.path().join(".dev/pipeline/receipts/managed.md");
        let external = tmp.path().join("evidence/external.md");
        std::fs::create_dir_all(managed.parent().unwrap()).unwrap();
        std::fs::create_dir_all(external.parent().unwrap()).unwrap();
        std::fs::write(&managed, "managed").unwrap();
        std::fs::write(&external, "external").unwrap();
        let prepared = prepare_receipt_path(tmp.path(), &managed).unwrap();
        assert!(!managed.exists());
        drop(prepared);
        assert!(prepare_receipt_path(tmp.path(), &external).is_err());
        assert_eq!(std::fs::read_to_string(external).unwrap(), "external");
    }

    #[test]
    fn relative_remote_local_receipt_is_bound_to_workdir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let configured = PathBuf::from(".dev/pipeline/receipts/relative.md");
        let prepared = prepare_receipt_path(tmp.path(), &configured).unwrap();
        assert_eq!(prepared.path(), tmp.path().join(configured));
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
            "GAL-DISPATCH-LOG v1\nterminal_state:  completed\n---STDOUT---\n---STDERR---\nold stderr\n",
        )
        .unwrap();

        let result = crate::dispatch::DispatchResult {
            terminal_state: TerminalState::NoReceipt,
            log_path: log_path.clone(),
            session_id: None,
            exit_code: Some(0),
            duration_ms: 0,
            stdout: String::new(),
            stderr: "fresh stderr evidence".to_string(),
        };
        sync_log_postprocessing(&log_path, &result);

        let updated = std::fs::read_to_string(&log_path).unwrap();
        assert!(updated.contains("terminal_state:  no-receipt"));
        assert!(updated.contains("fresh stderr evidence"));
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
            contract_provenance: None,
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
    fn invalid_effort_with_provenance_still_degrades_cleanly() {
        // Route resolves (AUDITOR → claude) with an invalid `effort` token *and* a
        // `contract_provenance`, so the routed marker suffix (effort + provenance)
        // is built and used on this degrade path without panicking.
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(
            br#"{ "executorRouting": {
                "executors": { "claude": "claude-sonnet-4-6" },
                "pipeline": { "AUDITOR": { "executor": "claude", "effort": "not valid!" } }
            } }"#,
        )
        .unwrap();
        let mut args = args_with_routing(f.path().to_path_buf());
        args.contract_provenance = Some(crate::dispatch::ContractProvenance {
            path: std::path::PathBuf::from("/repo/plan.prompt.md"),
            source: "workdir".to_string(),
        });
        let outcome = run_dispatch(&args, "spec");
        assert_eq!(outcome.exit_code, 2);
        assert_eq!(outcome.reason.as_deref(), Some("invalid-effort"));
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
