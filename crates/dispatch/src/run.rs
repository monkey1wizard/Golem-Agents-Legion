//! In-process dispatch flow shared by the `gal-dispatch` bin and `gal pipeline`.
//!
//! Extracted from the bin's `main` so the single `gal` binary can run a headless
//! dispatch without spawning a separate `gal-dispatch` executable (which is not in
//! the end-user release artifact). The T-007 safety gate (routing present +
//! executor available + authenticated) lives here, so both entry points share it.

use std::path::PathBuf;

use crate::adapters::{self, SpecDelivery};
use crate::cli::DispatchArgs;
use crate::dispatch::{
    executor_readiness, is_available, spawn_executor, Readiness, SpawnConfig, TerminalState,
};
use crate::routing::{load_routing, load_routing_default};

/// Result of an in-process dispatch run.
pub struct DispatchOutcome {
    /// Process exit code: 0 completed, 1 ran/unconfirmed, 2 degraded/unavailable.
    pub exit_code: u8,
    pub terminal_state: Option<TerminalState>,
    pub session_id: Option<String>,
    pub log_path: Option<PathBuf>,
}

/// Run one headless dispatch: load routing → safety gate → adapter → spawn →
/// emit the `--- GAL DISPATCH ---` marker. The spec is the full task spec text.
pub fn run_dispatch(args: &DispatchArgs, spec: &str) -> DispatchOutcome {
    let degraded = |exit_code: u8| DispatchOutcome {
        exit_code,
        terminal_state: None,
        session_id: None,
        log_path: None,
    };

    let routing = match &args.routing_path {
        Some(p) => load_routing(p),
        None => load_routing_default(),
    };
    for w in &routing.warnings {
        eprintln!("warning: {w}");
    }

    let role = args.phase.role();

    // ── T-007 safety gate: only offload with routing + an available executor ──
    let entry = match routing.get(role) {
        None => {
            println!("--- GAL DISPATCH ---");
            println!(
                "Dispatch: phase={} task={} role={} executor=none reason=no-routing",
                args.phase.as_str(),
                args.task,
                role,
            );
            if role == "AUDITOR" {
                eprintln!("gal dispatch: no executor configured for role {role}; the `REVIEWER` key was renamed `AUDITOR`, update ~/.gal/config/executor-routing.json");
            } else {
                eprintln!("gal dispatch: no executor configured for role {role}; falling back to text dispatch");
            }
            return degraded(2);
        }
        Some(e) => e,
    };

    let executor_name = &entry.executor;
    let model = &entry.model;

    if !is_available(executor_name) {
        println!("--- GAL DISPATCH ---");
        println!(
            "Dispatch: phase={} task={} role={} executor={} model={} reason=executor-unavailable",
            args.phase.as_str(),
            args.task,
            role,
            executor_name,
            model,
        );
        eprintln!("gal dispatch: executor '{executor_name}' not found in PATH; falling back to text dispatch");
        return degraded(2);
    }

    match executor_readiness(executor_name) {
        Readiness::Ready => {}
        Readiness::Unauthenticated { hint } => {
            println!("--- GAL DISPATCH ---");
            println!(
                "Dispatch: phase={} task={} role={} executor={} model={} reason=executor-unauthenticated-confirmed",
                args.phase.as_str(),
                args.task,
                role,
                executor_name,
                model,
            );
            eprintln!("gal dispatch: executor '{executor_name}' is not authenticated for confirmed headless use; {hint}");
            return degraded(2);
        }
        Readiness::Unknown { message } => {
            eprintln!("warning: gal dispatch: {message}");
        }
    }

    // ── Build invocation via adapter ──
    let (executor_args, stdin_spec) = match adapters::get_adapter(executor_name) {
        Some(adapter) => {
            let inv = adapter.build_invocation(model, &args.workdir, spec);
            let stdin = match inv.delivery {
                SpecDelivery::Stdin => spec.to_string(),
                SpecDelivery::CliFlag(_) => String::new(),
            };
            (inv.args, stdin)
        }
        None => (vec![], spec.to_string()),
    };

    let cfg = SpawnConfig {
        executor: executor_name.clone(),
        executor_args,
        spec: stdin_spec,
        workdir: args.workdir.clone(),
        timeout_secs: args.timeout_secs,
        task_id: args.task.clone(),
        phase: args.phase.as_str().to_string(),
        actual_model: model.clone(),
        log_dir: SpawnConfig::default_log_dir(&args.workdir),
        receipt_path: args.receipt_path.clone(),
    };

    let result = match spawn_executor(&cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gal dispatch: log write failed: {e}");
            println!("--- GAL DISPATCH ---");
            println!(
                "Dispatch: phase={} task={} executor={} model={} reason=log-error",
                args.phase.as_str(),
                args.task,
                executor_name,
                model,
            );
            return degraded(2);
        }
    };

    let session_id = if let Some(adapter) = adapters::get_adapter(executor_name) {
        adapter
            .extract_session_id(&result.stdout)
            .or_else(|| result.session_id.clone())
    } else {
        result.session_id.clone()
    };
    let session_str = session_id.as_deref().unwrap_or("none");

    println!("--- GAL DISPATCH ---");
    println!(
        "Dispatch: phase={} task={} role={} executor={} model={} state={} session_id={} log={}",
        args.phase.as_str(),
        args.task,
        role,
        executor_name,
        model,
        result.terminal_state,
        session_str,
        result.log_path.display(),
    );

    let exit_code = match result.terminal_state {
        TerminalState::Completed => 0,
        TerminalState::NoReceipt | TerminalState::DisconnectedPartial => 1,
        TerminalState::Timeout | TerminalState::Unavailable => 2,
    };

    DispatchOutcome {
        exit_code,
        terminal_state: Some(result.terminal_state),
        session_id,
        log_path: Some(result.log_path),
    }
}
