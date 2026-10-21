//! `gal-dispatch` binary entry point.
//!
//! Usage:
//!   gal-dispatch --phase <implement|test|audit> --task <T-NN>
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
        print_help(&mut std::io::stdout());
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

fn print_help(w: &mut impl std::io::Write) {
    let _ = writeln!(w, "gal-dispatch — GAL headless executor dispatch");
    let _ = writeln!(w);
    let _ = writeln!(w, "Usage:");
    let _ = writeln!(w, "  gal-dispatch --phase <phase> --task <T-NN> [options]");
    let _ = writeln!(w, "  (Pipe the task spec to stdin)");
    let _ = writeln!(w);
    let _ = writeln!(w, "Required:");
    let _ = writeln!(w, "  --phase <implement|test|audit>  Pipeline phase");
    let _ = writeln!(
        w,
        "  --task  <T-NN>                          Task identifier"
    );
    let _ = writeln!(w);
    let _ = writeln!(w, "Options:");
    let _ = writeln!(
        w,
        "  --workdir <path>        Working directory (default: cwd)"
    );
    let _ = writeln!(
        w,
        "  --timeout <seconds>     Execution timeout (default: 300)"
    );
    let _ = writeln!(w, "  --routing <path>        Override routing JSON path");
    let _ = writeln!(
        w,
        "  --receipt <path>        File to confirm was written back"
    );
    let _ = writeln!(w, "  --version               Print version");
    let _ = writeln!(w, "  --help                  Print this help");
    let _ = writeln!(w);
    let _ = writeln!(w, "Exit codes:");
    let _ = writeln!(w, "  0  Completed with write-back confirmed");
    let _ = writeln!(
        w,
        "  1  Ran but write-back unconfirmed (no-receipt or non-zero exit)"
    );
    let _ = writeln!(
        w,
        "  2  Degraded: no routing / executor unavailable / text dispatch"
    );
    let _ = writeln!(w, " 64  Usage error");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_help_lists_phases_without_verify() {
        let mut buf = Vec::new();
        print_help(&mut buf);
        let help_text = String::from_utf8(buf).expect("help text should be valid UTF-8");
        assert!(help_text.contains("implement"));
        assert!(help_text.contains("test"));
        assert!(help_text.contains("audit"));
        assert!(!help_text.contains("verify"));
    }
}
