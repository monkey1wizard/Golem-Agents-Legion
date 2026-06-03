//! `gal` CLI entry point (Phase 1 skeleton).
//!
//! Owns argument parsing, help/version, and exit-code classification. It does
//! not parse machine config and does not yet execute install/update/doctor —
//! known subcommands are reported as `not wired` until they are delegated to the
//! legacy scripts in a later phase (T-009).

use gal_core::{classify_args, Action, CommandKind, ExitCode};
use std::process::ExitCode as ProcessExitCode;

fn print_help() {
    println!("gal — GAL bootstrap CLI");
    println!();
    println!("Usage: gal <command> [options]");
    println!("       gal --version");
    println!("       gal --help");
    println!();
    println!("Commands:");
    for cmd in CommandKind::ALL {
        println!("  {}", cmd.as_str());
    }
}

fn run(args: &[String]) -> ExitCode {
    match classify_args(args) {
        Action::Version => {
            println!("gal {}", env!("CARGO_PKG_VERSION"));
            ExitCode::Success
        }
        Action::Help => {
            print_help();
            ExitCode::Success
        }
        Action::MissingCommand => {
            print_help();
            ExitCode::Usage
        }
        Action::NotWired(cmd) => {
            eprintln!(
                "gal {}: not wired yet (Phase 1 skeleton). This subcommand will \
                 delegate to the legacy scripts in a later phase.",
                cmd.as_str()
            );
            ExitCode::NotWired
        }
        Action::UnknownCommand(cmd) => {
            eprintln!("gal: unknown command '{}'. Run `gal --help`.", cmd);
            ExitCode::Usage
        }
    }
}

fn main() -> ProcessExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_flag_succeeds() {
        assert_eq!(run(&["--version".to_string()]), ExitCode::Success);
    }

    #[test]
    fn help_flag_succeeds() {
        assert_eq!(run(&["--help".to_string()]), ExitCode::Success);
    }

    #[test]
    fn known_subcommand_is_not_wired() {
        assert_eq!(run(&["doctor".to_string()]), ExitCode::NotWired);
        assert_eq!(run(&["uninstall".to_string()]), ExitCode::NotWired);
    }

    #[test]
    fn unknown_subcommand_is_usage_error() {
        assert_eq!(run(&["frobnicate".to_string()]), ExitCode::Usage);
    }

    #[test]
    fn no_args_is_usage_error() {
        assert_eq!(run(&[]), ExitCode::Usage);
    }
}
