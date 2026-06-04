//! `gal-dispatch` binary entry point (T-004).
//!
//! Usage: gal-dispatch --phase <implement|test|review|verify> --task <T-NNN>
//!                     [--workdir <path>] [--timeout <seconds>] [--routing <path>]

use gal_dispatch::cli::parse_args;
use gal_dispatch::routing::{load_routing, load_routing_default};
use std::process::ExitCode;

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();

    // Quick help/version gate
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
            return ExitCode::from(64); // EX_USAGE
        }
    };

    // Load routing table
    let routing = match &dispatch_args.routing_path {
        Some(p) => load_routing(p),
        None => load_routing_default(),
    };

    for w in &routing.warnings {
        eprintln!("warning: {w}");
    }

    // Resolve role → route entry
    let role = dispatch_args.phase.role();
    let route = routing.get(role);

    match route {
        None => {
            // No routing configured → text dispatch degradation (T-007)
            println!("--- GAL DISPATCH ---");
            println!("Dispatch: phase={} task={} route=none executor=text-dispatch",
                dispatch_args.phase.as_str(), dispatch_args.task);
            eprintln!("gal-dispatch: no executor configured for role {role}; falling back to text dispatch");
            ExitCode::from(2) // signal to caller: degraded
        }
        Some(entry) => {
            // Phase 1 stub: executor dispatch wired in T-005 onwards.
            // For now: print resolved route so the caller can see routing works.
            println!("--- GAL DISPATCH ---");
            println!("Dispatch: phase={} task={} role={role} executor={} model={}",
                dispatch_args.phase.as_str(),
                dispatch_args.task,
                entry.executor,
                entry.model,
            );
            eprintln!(
                "gal-dispatch: executor dispatch not yet wired (T-005); route resolved to {}/{}",
                entry.executor, entry.model
            );
            ExitCode::from(2) // not yet dispatching; stub exit
        }
    }
}

fn print_help() {
    println!("gal-dispatch — GAL headless executor dispatch");
    println!();
    println!("Usage:");
    println!("  gal-dispatch --phase <phase> --task <T-NNN> [options]");
    println!();
    println!("Required:");
    println!("  --phase <implement|test|review|verify>   Pipeline phase");
    println!("  --task  <T-NNN>                          Task identifier");
    println!();
    println!("Options:");
    println!("  --workdir <path>        Working directory (default: cwd)");
    println!("  --timeout <seconds>     Execution timeout (default: 300)");
    println!("  --routing <path>        Override routing JSON path");
    println!("  --version               Print version");
    println!("  --help                  Print this help");
}
