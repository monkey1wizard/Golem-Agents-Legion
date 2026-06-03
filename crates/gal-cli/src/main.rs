//! `gal` CLI entry point — Rust-native install/update/doctor/uninstall/commit-msg.
//!
//! Entry switch (T-011, BUG-B): the Rust binary is now the single entry for
//! install, update, uninstall, doctor, and commit-msg. The frozen PS/Bash scripts
//! remain as oracle only and are NOT invoked by this entry.
//!
//! T-011: install/update/uninstall wired to Rust-native gal_core::install.
//! T-012: doctor wired to Rust-native gal_core::doctor.
//! T-013: commit-msg wired to Rust-native gal_core::commit_msg (R5, optional).

use gal_core::{classify_args, Action, CommandKind, ExitCode};
use std::path::Path;
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
    println!();
    println!("Options:");
    println!("  doctor --dry-run                  Read-only health check (no filesystem changes)");
    println!("  doctor --release-gate             Include package-manager and marketplace checks");
    println!("  release --dry-run                 Local artifact dry-run (checksums + manifest)");
    println!("  release --version <tag>           Override version tag (default: Cargo.toml)");
    println!("  release --output-dir <path>       Output directory (default: release-artifacts/)");
}

/// Run `gal install` — Rust-native install flow (T-011).
fn cmd_install() -> ExitCode {
    use gal_core::{config::GalConfig, install::run_install};

    let config = GalConfig::load();
    match run_install(&config) {
        Ok(report) => {
            println!("GAL installed to {}", report.canonical_root.display());
            println!("Mode: {}", report.mode);
            println!("Providers: {}", report.providers.join(", "));
            for w in &report.warnings {
                eprintln!("warning: {w}");
            }
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal install: {e}");
            ExitCode::Error
        }
    }
}

/// Run `gal update` — re-render and re-project all surfaces (T-011).
fn cmd_update() -> ExitCode {
    use gal_core::{config::GalConfig, install::run_update};

    let config = GalConfig::load();
    match run_update(&config) {
        Ok(report) => {
            println!("GAL updated at {}", report.canonical_root.display());
            println!("Mode: {}", report.mode);
            println!("Providers: {}", report.providers.join(", "));
            for w in &report.warnings {
                eprintln!("warning: {w}");
            }
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal update: {e}");
            ExitCode::Error
        }
    }
}

/// Run `gal uninstall` — remove canonical root and surfaces (T-011).
fn cmd_uninstall() -> ExitCode {
    use gal_core::install::run_uninstall;

    match run_uninstall() {
        Ok(()) => {
            println!("GAL uninstalled.");
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal uninstall: {e}");
            ExitCode::Error
        }
    }
}

/// Run `gal doctor [--dry-run] [--release-gate]` — read-only health checks (T-012).
fn cmd_doctor(args: &[String]) -> ExitCode {
    use gal_core::doctor::{run_doctor, DoctorOptions};

    let dry_run = args.iter().any(|a| a == "--dry-run");
    let release_gate = args.iter().any(|a| a == "--release-gate");

    let opts = DoctorOptions { dry_run, release_gate };
    let report = run_doctor(&opts);

    if report.findings.is_empty() {
        println!("gal doctor: all checks passed.");
    } else {
        for f in &report.findings {
            println!("{f}");
        }
    }

    if report.has_errors() {
        ExitCode::Error
    } else {
        ExitCode::Success
    }
}

/// Run `gal release [--dry-run] [--version <tag>] [--output-dir <dir>]` (T-014).
///
/// Produces `checksums.txt` and `artifact-manifest.json` in the output directory.
/// Cosign signing is CI-only (OIDC); locally a placeholder is written instead.
fn cmd_release(args: &[String]) -> ExitCode {
    use gal_core::release::{run_release, AssetSpec, ReleaseOptions};
    use std::path::PathBuf;

    // Parse flags: --version <tag>, --output-dir <path>, --dry-run (implied always)
    let mut version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let mut output_dir = PathBuf::from("release-artifacts");
    let mut extra_assets: Vec<PathBuf> = Vec::new();

    let mut i = 1usize; // skip "release"
    while i < args.len() {
        match args[i].as_str() {
            "--version" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    version = v.clone();
                } else {
                    eprintln!("gal release: --version requires a value");
                    return ExitCode::Usage;
                }
            }
            "--output-dir" => {
                i += 1;
                if let Some(d) = args.get(i) {
                    output_dir = PathBuf::from(d);
                } else {
                    eprintln!("gal release: --output-dir requires a value");
                    return ExitCode::Usage;
                }
            }
            "--asset" => {
                // Accept --asset <path> for CI invocation with pre-built binaries.
                i += 1;
                if let Some(p) = args.get(i) {
                    extra_assets.push(PathBuf::from(p));
                } else {
                    eprintln!("gal release: --asset requires a path");
                    return ExitCode::Usage;
                }
            }
            "--dry-run" => {} // always dry-run locally; flag is accepted but implied
            unknown => {
                eprintln!("gal release: unknown option '{unknown}'");
                return ExitCode::Usage;
            }
        }
        i += 1;
    }

    // Build asset specs from explicit --asset paths (CI) or empty (local dev dry-run).
    let assets: Vec<AssetSpec> = extra_assets
        .into_iter()
        .map(|path| {
            // Infer platform/arch/kind from the canonical filename convention:
            // gal-<version>-<platform>-<arch>[.exe|.zip|.tar.gz]
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let (platform, architecture, kind) = parse_asset_name(&name, &version);
            AssetSpec {
                name,
                platform,
                architecture,
                kind,
                contains: vec![],
                path,
            }
        })
        .collect();

    match run_release(ReleaseOptions { version: version.clone(), output_dir: output_dir.clone(), assets }) {
        Ok(result) => {
            println!("gal release: artifacts written to {}", output_dir.display());
            println!("  checksums.txt:        {}", result.checksums_path.display());
            println!("  artifact-manifest.json: {}", result.manifest_path.display());
            if result.cosign_skipped {
                println!("  cosign: SKIPPED (no OIDC locally) — placeholder written");
                println!("  To sign, run this in CI with OIDC token available.");
            }
            println!("  version: {}", result.manifest.version);
            println!("  assets:  {}", result.manifest.assets.len());
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal release: {e}");
            ExitCode::Error
        }
    }
}

/// Infer (platform, architecture, ArtifactKind) from a canonical asset filename.
/// Falls back to ("unknown", "unknown", Binary) when the name does not match the
/// convention — never panics.
fn parse_asset_name(name: &str, _version: &str) -> (String, String, gal_core::release::ArtifactKind) {
    use gal_core::release::ArtifactKind;

    let kind = if name.ends_with(".zip") || name.ends_with(".tar.gz") {
        ArtifactKind::Archive
    } else {
        ArtifactKind::Binary
    };

    // Strip known suffixes to expose the platform-arch portion.
    let stem = name
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".zip")
        .trim_end_matches(".exe");

    // Expected: gal-<version>-<platform>-<arch>
    let parts: Vec<&str> = stem.split('-').collect();
    if parts.len() >= 2 {
        let arch = parts[parts.len() - 1].to_string();
        let platform = parts[parts.len() - 2].to_string();
        (platform, arch, kind)
    } else {
        ("unknown".to_string(), "unknown".to_string(), kind)
    }
}

/// Run `gal commit-msg <msg-file>` — git commit-msg hook (T-013, R5 optional).
fn cmd_commit_msg(args: &[String]) -> ExitCode {
    use gal_core::commit_msg::{process_commit_msg, CommitMsgResult};

    // The first argument after "commit-msg" is the message file path.
    let msg_path_str = match args.get(1) {
        Some(p) => p.clone(),
        None => {
            eprintln!("gal commit-msg: missing message file argument");
            eprintln!("usage: gal commit-msg <path-to-commit-message-file>");
            return ExitCode::Usage;
        }
    };

    let msg_path = Path::new(&msg_path_str);
    if !msg_path.exists() {
        eprintln!("gal commit-msg: message file not found: {msg_path_str}");
        return ExitCode::Error;
    }

    // Get staged files from git.
    let staged_files = get_staged_files();
    let staged_refs: Vec<&str> = staged_files.iter().map(|s| s.as_str()).collect();

    match process_commit_msg(msg_path, &staged_refs) {
        Ok(CommitMsgResult::NoOp) => {
            // Empty staging — no-op (TP-019).
            ExitCode::Success
        }
        Ok(CommitMsgResult::Updated) => ExitCode::Success,
        Err(e) => {
            eprintln!("gal commit-msg: {e}");
            ExitCode::Error
        }
    }
}

/// Retrieve staged file paths from git (no-op list on error).
fn get_staged_files() -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(["diff", "--staged", "--name-only"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        }
        _ => Vec::new(), // git unavailable or not a repo → treat as empty staging
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
        // T-011: wired install/update/uninstall
        Action::NotWired(CommandKind::Install) => cmd_install(),
        Action::NotWired(CommandKind::Update) => cmd_update(),
        Action::NotWired(CommandKind::Uninstall) => cmd_uninstall(),
        // T-012: wired doctor
        Action::NotWired(CommandKind::Doctor) => cmd_doctor(args),
        // T-013: wired commit-msg (R5, optional)
        Action::NotWired(CommandKind::CommitMsg) => cmd_commit_msg(args),
        // T-014: wired release artifact generation
        Action::NotWired(CommandKind::Release) => cmd_release(args),
        // Not yet wired (dispatch-script, etc.)
        Action::NotWired(cmd) => {
            eprintln!("gal {}: not wired", cmd.as_str());
            ExitCode::NotWired
        }
        Action::UnknownCommand(cmd) => {
            eprintln!("gal: unknown command '{cmd}'. Run `gal --help`.");
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
    fn no_args_is_usage() {
        assert_eq!(run(&[]), ExitCode::Usage);
    }

    #[test]
    fn unknown_command_is_usage() {
        assert_eq!(run(&["frobnicate".to_string()]), ExitCode::Usage);
    }

    // T-011: install/update/uninstall are now wired (return Success or Error, not NotWired)
    #[test]
    fn install_is_wired_not_not_wired() {
        let result = run(&["install".to_string()]);
        assert_ne!(
            result,
            ExitCode::NotWired,
            "install must be wired (T-011) — should return Success or Error, not NotWired"
        );
    }

    #[test]
    fn update_is_wired_not_not_wired() {
        let result = run(&["update".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "update must be wired (T-011)");
    }

    #[test]
    fn uninstall_is_wired_not_not_wired() {
        let result = run(&["uninstall".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "uninstall must be wired (T-011)");
    }

    // T-012: doctor is wired
    #[test]
    fn doctor_is_wired_not_not_wired() {
        let result = run(&["doctor".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "doctor must be wired (T-012)");
    }

    #[test]
    fn doctor_dry_run_is_wired() {
        let result = run(&["doctor".to_string(), "--dry-run".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "doctor --dry-run must be wired (T-012)");
    }

    // T-013: commit-msg is wired
    #[test]
    fn commit_msg_without_arg_returns_usage() {
        let result = run(&["commit-msg".to_string()]);
        assert_eq!(result, ExitCode::Usage, "commit-msg with no file arg should be Usage");
    }

    #[test]
    fn dispatch_script_is_still_not_wired() {
        assert_eq!(
            run(&["dispatch-script".to_string()]),
            ExitCode::NotWired,
            "dispatch-script should remain not wired"
        );
    }

    #[test]
    fn exit_code_values_are_correct() {
        assert_eq!(ExitCode::Success as u8, 0);
        assert_eq!(ExitCode::Error as u8, 1);
        assert_eq!(ExitCode::NotWired as u8, 2);
        assert_eq!(ExitCode::Usage as u8, 64);
    }
}
