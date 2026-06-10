//! `gal-dispatch` binary entry point (T-004, T-007).
//!
//! Usage:
//!   gal-dispatch --phase <implement|test|review|verify> --task <T-NNN>
//!               [--workdir <path>] [--timeout <seconds>] [--routing <path>]
//!               [--receipt <path>]
//!
//! The task spec is read from stdin before the executor is spawned.
//!
//! Exit codes:
//!   0  — dispatch completed and write-back verified (terminal state: completed)
//!   1  — dispatch ran but write-back unconfirmed (terminal state: no-receipt or disconnected-partial)
//!   2  — degraded: no routing / executor unavailable / bin text dispatch (terminal state: unavailable or text-dispatch)
//!  64  — usage error (bad CLI args)
//!
//! Stdout always includes a `--- GAL DISPATCH ---` header and a `Dispatch:` marker line.
//! The marker line lets the pipeline orchestrator record the session id and receipt status.

use std::io::Read as _;
use std::process::ExitCode;

use dispatch::adapters::{self, SpecDelivery};
use dispatch::cli::parse_args;
use dispatch::dispatch::{executor_readiness, is_available, spawn_executor, Readiness, SpawnConfig, TerminalState};
use dispatch::routing::{load_routing, load_routing_default};

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();

    if raw_args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if raw_args.iter().any(|a| a == "--version" || a == "-V") {
        println!("gal-dispatch {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    // Parse CLI args
    let dispatch_args = match parse_args(&raw_args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("gal-dispatch: {e}");
            eprintln!("Run `gal-dispatch --help` for usage.");
            return ExitCode::from(64);
        }
    };

    // Read task spec from stdin (before spawning anything)
    let mut spec = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut spec) {
        eprintln!("gal-dispatch: failed to read spec from stdin: {e}");
        return ExitCode::from(64);
    }

    // Load routing table
    let routing = match &dispatch_args.routing_path {
        Some(p) => load_routing(p),
        None => load_routing_default(),
    };
    for w in &routing.warnings {
        eprintln!("warning: {w}");
    }

    let role = dispatch_args.phase.role();
    let route = routing.get(role);

    // ── T-007: Default safety gate ─────────────────────────────────────────────
    // Only offload when: routing has an entry AND the executor CLI is in PATH.
    // If either condition fails, degrade to text dispatch output.

    let entry = match route {
        None => {
            // No routing configured for this role → text dispatch
            println!("--- GAL DISPATCH ---");
            println!(
                "Dispatch: phase={} task={} role={} executor=none reason=no-routing",
                dispatch_args.phase.as_str(),
                dispatch_args.task,
                role,
            );
            eprintln!("gal-dispatch: no executor configured for role {role}; falling back to text dispatch");
            return ExitCode::from(2);
        }
        Some(e) => e,
    };

    let executor_name = &entry.executor;
    let model = &entry.model;

    // Safety gate: executor must be available in PATH
    if !is_available(executor_name) {
        println!("--- GAL DISPATCH ---");
        println!(
            "Dispatch: phase={} task={} role={} executor={} model={} reason=executor-unavailable",
            dispatch_args.phase.as_str(),
            dispatch_args.task,
            role,
            executor_name,
            model,
        );
        eprintln!("gal-dispatch: executor '{executor_name}' not found in PATH; falling back to text dispatch");
        return ExitCode::from(2);
    }

    match executor_readiness(executor_name) {
        Readiness::Ready => {}
        Readiness::Unauthenticated { hint } => {
            println!("--- GAL DISPATCH ---");
            println!(
                "Dispatch: phase={} task={} role={} executor={} model={} reason=executor-unauthenticated-confirmed",
                dispatch_args.phase.as_str(),
                dispatch_args.task,
                role,
                executor_name,
                model,
            );
            eprintln!(
                "gal-dispatch: executor '{executor_name}' is not authenticated for confirmed headless use; {hint}"
            );
            return ExitCode::from(2);
        }
        Readiness::Unknown { message } => {
            eprintln!("warning: gal-dispatch: {message}");
        }
    }

    // ── Build invocation via adapter ───────────────────────────────────────────

    // Build SpawnConfig. If no adapter is registered for the executor, fall back
    // to treating it as a generic stdin-based tool.
    let (executor_args, stdin_spec) = match adapters::get_adapter(executor_name) {
        Some(adapter) => {
            let inv = adapter.build_invocation(model, &dispatch_args.workdir, &spec);
            let stdin = match inv.delivery {
                SpecDelivery::Stdin => spec.clone(),
                SpecDelivery::CliFlag(_) => String::new(), // spec already in args
            };
            (inv.args, stdin)
        }
        None => {
            // Unknown executor: pass spec via stdin with no extra args
            (vec![], spec.clone())
        }
    };

    let log_dir = SpawnConfig::default_log_dir(&dispatch_args.workdir);

    let cfg = SpawnConfig {
        executor: executor_name.clone(),
        executor_args,
        spec: stdin_spec,
        workdir: dispatch_args.workdir.clone(),
        timeout_secs: dispatch_args.timeout_secs,
        task_id: dispatch_args.task.clone(),
        phase: dispatch_args.phase.as_str().to_string(),
        actual_model: model.clone(),
        log_dir,
        receipt_path: dispatch_args.receipt_path.clone(),
    };

    // ── Dispatch ───────────────────────────────────────────────────────────────

    let result = match spawn_executor(&cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gal-dispatch: log write failed: {e}");
            // Still attempt to output a Dispatch: marker
            println!("--- GAL DISPATCH ---");
            println!(
                "Dispatch: phase={} task={} executor={} model={} reason=log-error",
                dispatch_args.phase.as_str(),
                dispatch_args.task,
                executor_name,
                model,
            );
            return ExitCode::from(2);
        }
    };

    // Refine session id using tool-specific adapter extraction
    let session_id = if let Some(adapter) = adapters::get_adapter(executor_name) {
        adapter.extract_session_id(&result.stdout)
            .or(result.session_id.clone())
    } else {
        result.session_id.clone()
    };
    let session_str = session_id.as_deref().unwrap_or("none");

    // ── Output Dispatch: marker ────────────────────────────────────────────────

    println!("--- GAL DISPATCH ---");
    println!(
        "Dispatch: phase={} task={} role={} executor={} model={} state={} session_id={} log={}",
        dispatch_args.phase.as_str(),
        dispatch_args.task,
        role,
        executor_name,
        model,
        result.terminal_state,
        session_str,
        result.log_path.display(),
    );

    // ── Exit code mapping ──────────────────────────────────────────────────────
    match result.terminal_state {
        TerminalState::Completed => ExitCode::SUCCESS,
        TerminalState::NoReceipt | TerminalState::DisconnectedPartial => ExitCode::from(1),
        TerminalState::Timeout | TerminalState::Unavailable => ExitCode::from(2),
    }
}

fn print_help() {
    println!("gal-dispatch — GAL headless executor dispatch");
    println!();
    println!("Usage:");
    println!("  gal-dispatch --phase <phase> --task <T-NNN> [options]");
    println!("  (Pipe the task spec to stdin)");
    println!();
    println!("Required:");
    println!("  --phase <implement|test|review|verify>   Pipeline phase");
    println!("  --task  <T-NNN>                          Task identifier");
    println!();
    println!("Options:");
    println!("  --workdir <path>        Working directory (default: cwd)");
    println!("  --timeout <seconds>     Execution timeout (default: 300)");
    println!("  --routing <path>        Override routing JSON path");
    println!("  --receipt <path>        File to verify was written back (T-006)");
    println!("  --version               Print version");
    println!("  --help                  Print this help");
    println!();
    println!("Exit codes:");
    println!("  0  Completed with write-back verified");
    println!("  1  Ran but write-back unconfirmed (no-receipt or non-zero exit)");
    println!("  2  Degraded: no routing / executor unavailable / text dispatch");
    println!(" 64  Usage error");
}
