//! `gal-dispatch` binary entry point.
//!
//! Usage:
//!   gal-dispatch --phase <implement|test|audit> --task <T-NNN>
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

use dispatch::cli::parse_args;
use dispatch::run::run_dispatch;

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

    // Run the shared in-process dispatch flow (safety gate + spawn + marker).
    let outcome = run_dispatch(&dispatch_args, &spec);
    ExitCode::from(outcome.exit_code)
}

fn print_help() {
    println!("gal-dispatch — GAL headless executor dispatch");
    println!();
    println!("Usage:");
    println!("  gal-dispatch --phase <phase> --task <T-NNN> [options]");
    println!("  (Pipe the task spec to stdin)");
    println!();
    println!("Required:");
    println!("  --phase <implement|test|audit>          Pipeline phase");
    println!("  --task  <T-NNN>                          Task identifier");
    println!();
    println!("Options:");
    println!("  --workdir <path>        Working directory (default: cwd)");
    println!("  --timeout <seconds>     Execution timeout (default: 300)");
    println!("  --routing <path>        Override routing JSON path");
    println!("  --receipt <path>        File to verify was written back");
    println!("  --version               Print version");
    println!("  --help                  Print this help");
    println!();
    println!("Exit codes:");
    println!("  0  Completed with write-back verified");
    println!("  1  Ran but write-back unconfirmed (no-receipt or non-zero exit)");
    println!("  2  Degraded: no routing / executor unavailable / text dispatch");
    println!(" 64  Usage error");
}
