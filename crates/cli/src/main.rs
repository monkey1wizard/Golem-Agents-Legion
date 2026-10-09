//! `gal` CLI entry point.
//!
//! Entry switch: the Rust binary is the single entry for the gal workflow
//! command surface. See `gal_engine::CommandKind::ALL` for the full,
//! authoritative subcommand list.

mod commands;
mod dispatch_script;
mod gal;
mod init_repo;

use commands::boundary_check::cmd_boundary_check;
use commands::converge_check::cmd_pipeline_converge_check;
use commands::dispatch::{
    cmd_consult_script, cmd_dispatch, cmd_dispatch_script, cmd_pipeline, cmd_pipeline_log,
    cmd_research,
};
use commands::doctor::cmd_doctor;
use commands::finalize_check::cmd_finalize_check;
use commands::lifecycle::cmd_update;
use commands::marketplace_snapshot::cmd_marketplace_snapshot;
use commands::pipeline_clean::cmd_pipeline_clean;
use commands::pipeline_handback_check::cmd_pipeline_handback_check;
use commands::pipeline_host_hook::cmd_pipeline_host_hook;
use commands::pipeline_preflight::cmd_pipeline_preflight;
use commands::planning_authority::cmd_planning_stamp;
use commands::planning_check::cmd_planning_check;
use commands::prompt_check::cmd_prompt_check;
use commands::refining_check::cmd_refining_check;
use commands::refresh::cmd_refresh;
use commands::release::cmd_release;
use commands::release_notes::cmd_release_notes;
use commands::restore::cmd_restore;
use commands::state_merge::cmd_state_merge;
use commands::system::{
    cmd_commit_msg, cmd_filter_transform, cmd_init, cmd_naming_gate, cmd_render_adapters,
    cmd_translation_freshness,
};
use gal_engine::{classify_args, Action, CommandKind, ExitCode};
use std::process::ExitCode as ProcessExitCode;

const TRUST_BEARING_COMMANDS: [CommandKind; 10] = [
    CommandKind::Pipeline,
    CommandKind::PipelinePreflight,
    CommandKind::BoundaryCheck,
    CommandKind::PipelineConvergeCheck,
    CommandKind::PipelineHandbackCheck,
    CommandKind::PlanningCheck,
    CommandKind::RefiningCheck,
    CommandKind::PromptCheck,
    CommandKind::FinalizeCheck,
    CommandKind::NamingGate,
];

fn trusted_handoff(args: &[String]) -> Option<i32> {
    let Action::NotWired(kind) = classify_args(args) else {
        return None;
    };
    if !TRUST_BEARING_COMMANDS.contains(&kind) {
        return None;
    }
    let start = match std::env::current_dir().and_then(std::fs::canonicalize) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("gal: cannot resolve worktree: {error}");
            return Some(1);
        }
    };
    // Downstream repositories keep using their installed executable. Identify
    // the source checkout by the CLI package identity in its manifest, from
    // any subdirectory of it.
    let worktree = commands::pipeline_execution::resolve_worktree_root(&start);
    if !commands::pipeline_execution::is_gal_source_checkout(&worktree) {
        return None;
    }

    let current = match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("gal: cannot resolve running executable: {error}");
            return Some(1);
        }
    };
    let private_root = worktree.join("target/gal-pipeline/bin");
    if current.starts_with(&private_root) {
        if let Err(error) = validate_private_executable(&current, |path| std::fs::read(path)) {
            eprintln!("gal: {error}");
            return Some(1);
        }
        return None;
    }

    let manifest = worktree.join("Cargo.toml");
    let child = match commands::pipeline_execution::build_private_generation(&worktree, &manifest) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("gal: private executable unavailable: {error}");
            return Some(1);
        }
    };
    let child_hash = match std::fs::read(&child) {
        Ok(bytes) => {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(bytes))
        }
        Err(error) => {
            eprintln!(
                "gal: cannot validate bound executable {}: {error}",
                child.display()
            );
            return Some(1);
        }
    };
    if child
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        != Some(child_hash.as_str())
    {
        eprintln!("gal: bound executable path/hash mismatch");
        return Some(1);
    }
    let output = match std::process::Command::new(&child).args(args).output() {
        Ok(output) => output,
        Err(error) => {
            eprintln!(
                "gal: cannot launch bound executable {}: {error}",
                child.display()
            );
            return Some(1);
        }
    };
    let status = output.status.code().unwrap_or(1);
    eprintln!(
        "gal handoff: action={} child_path={} child_sha256={} exit_status={status}",
        args.first().map(String::as_str).unwrap_or_default(),
        child.display(),
        child_hash
    );
    use std::io::Write;
    let _ = std::io::stdout().write_all(&output.stdout);
    let _ = std::io::stderr().write_all(&output.stderr);
    Some(status)
}

fn validate_private_executable(
    current: &std::path::Path,
    read: impl FnOnce(&std::path::Path) -> std::io::Result<Vec<u8>>,
) -> Result<(), String> {
    let expected_hash = current
        .parent()
        .and_then(std::path::Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| "bound private executable path has no valid hash directory".to_string())?;
    let bytes = read(current).map_err(|error| {
        format!(
            "cannot read bound private executable {}: {error}",
            current.display()
        )
    })?;
    use sha2::{Digest, Sha256};
    let observed = format!("{:x}", Sha256::digest(bytes));
    if expected_hash != observed {
        return Err("bound private executable hash mismatch".into());
    }
    Ok(())
}

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
    println!("  init [--blank]                    Initialize GAL in the current repository");
    println!("  render-adapters                   Re-render AGENTS.md from .dev/project.md");
    println!("  translation-freshness            Report docs/i18n translation freshness");
}

fn run(args: &[String]) -> ExitCode {
    if !cfg!(test) {
        if let Some(code) = trusted_handoff(args) {
            return match code {
                0 => ExitCode::Success,
                64 => ExitCode::Usage,
                _ => ExitCode::Error,
            };
        }
    }
    match classify_args(args) {
        Action::Version => {
            // GAL_GIT_STAMP is baked by build.rs; empty on a packaged build with no git.
            let stamp = env!("GAL_GIT_STAMP");
            if stamp.is_empty() {
                println!("gal {}", env!("CARGO_PKG_VERSION"));
            } else {
                println!("gal {} ({})", env!("CARGO_PKG_VERSION"), stamp);
            }
            ExitCode::Success
        }
        Action::Help => {
            print_help();
            ExitCode::Success
        }
        Action::MissingCommand => {
            // Bare `gal` (no args) prints help and exits 0. winget validation
            // bare-executes the installed exe (both Packages\… and WinGet\Links\
            // paths) and records the exit code; a non-zero here is treated as an
            // install failure. An unknown subcommand still exits Usage(64) below.
            print_help();
            ExitCode::Success
        }
        // wired update
        Action::NotWired(CommandKind::Update) => cmd_update(args),
        Action::NotWired(CommandKind::Dispatch) => cmd_dispatch(args),
        Action::NotWired(CommandKind::Pipeline) => cmd_pipeline(args),
        // wired doctor
        Action::NotWired(CommandKind::Doctor) => cmd_doctor(args),
        // wired commit-msg (R5, optional)
        Action::NotWired(CommandKind::CommitMsg) => cmd_commit_msg(args),
        // wired git clean/smudge filters
        Action::NotWired(CommandKind::Clean) => cmd_filter_transform(args, false),
        Action::NotWired(CommandKind::Smudge) => cmd_filter_transform(args, true),
        // wired init
        Action::NotWired(CommandKind::Init) => cmd_init(args),
        Action::NotWired(CommandKind::RenderAdapters) => cmd_render_adapters(args),
        // wired translation freshness
        Action::NotWired(CommandKind::TranslationFreshness) => cmd_translation_freshness(),
        // wired chat-control-plane dispatch.
        Action::NotWired(CommandKind::DispatchScript) => cmd_dispatch_script(args),
        Action::NotWired(CommandKind::ConsultScript) => cmd_consult_script(args),
        // naming-gate: blocking scan for plan-task ID provenance + retired terms.
        Action::NotWired(CommandKind::NamingGate) => cmd_naming_gate(args),
        // finalize-check: finalize-internal deterministic zero-trust precondition checks.
        Action::NotWired(CommandKind::FinalizeCheck) => cmd_finalize_check(args),
        // pipeline-log: pipeline-internal loop-log append (in-process append path).
        Action::NotWired(CommandKind::PipelineLog) => cmd_pipeline_log(args),
        // pipeline-clean: remove one closed plan's runtime directory.
        Action::NotWired(CommandKind::PipelineClean) => cmd_pipeline_clean(args),
        // release: generate release artifacts (checksums + manifest + winget/homebrew).
        Action::NotWired(CommandKind::Release) => cmd_release(args),
        // release-notes: draft a deterministic CHANGELOG section from a commit range.
        Action::NotWired(CommandKind::ReleaseNotes) => cmd_release_notes(args),
        // refresh: idempotent machine-level rebuild of canonical root + marketplace + runtime projections.
        Action::NotWired(CommandKind::Refresh) => cmd_refresh(args),
        // pipeline-converge-check: 2g closeout receipt (three-surface + commit + cursor).
        Action::NotWired(CommandKind::PipelineConvergeCheck) => cmd_pipeline_converge_check(args),
        // pipeline-handback-check: pipeline continuation handback authority.
        Action::NotWired(CommandKind::PipelineHandbackCheck) => cmd_pipeline_handback_check(args),
        // boundary-check: 2c pre-commit allowlist (diff+untracked vs ## Affected Files).
        Action::NotWired(CommandKind::BoundaryCheck) => cmd_boundary_check(args),
        // pipeline-preflight: Step 1 prereq + resume cursor + wrong-plan guard.
        Action::NotWired(CommandKind::PipelinePreflight) => cmd_pipeline_preflight(args),
        // planning-check: planning/deep-planning handoff gate.
        Action::NotWired(CommandKind::PlanningCheck) => cmd_planning_check(args),
        // prompt-check: compressed prompt anchor validation gate.
        Action::NotWired(CommandKind::PromptCheck) => cmd_prompt_check(args),
        // refining-check: refined source-plan structure gate.
        Action::NotWired(CommandKind::RefiningCheck) => cmd_refining_check(args),
        // planning-stamp: deterministic hash/receipt write-side (internal).
        Action::NotWired(CommandKind::PlanningStamp) => cmd_planning_stamp(args),
        // restore: revert GAL source repo to last gal-last-good marker.
        Action::NotWired(CommandKind::Restore) => cmd_restore(args),
        Action::NotWired(CommandKind::MarketplaceSnapshot) => cmd_marketplace_snapshot(args),
        // state-merge: finalize-internal deterministic `.dev/state.md` conflict resolver.
        Action::NotWired(CommandKind::StateMerge) => cmd_state_merge(args),
        Action::NotWired(CommandKind::PipelineHostHook) => cmd_pipeline_host_hook(&args[1..]),
        // research: dedicated research dispatch lane entry point
        // (`gal research <slug> --worker <1|2>`).
        Action::NotWired(CommandKind::Research) => cmd_research(args),
        Action::UnknownCommand(cmd) => {
            // Keep compatibility with Codex manifests rendered before the
            // canonical `gal pipeline-host-hook <event>` command was adopted.
            if cmd == "hook" {
                return cmd_pipeline_host_hook(&args[1..]);
            }
            eprintln!("gal: unknown command '{cmd}'. Run `gal --help`.");
            ExitCode::Usage
        }
    }
}

fn main() -> ProcessExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = trusted_handoff(&args) {
        return ProcessExitCode::from(code.clamp(0, 255) as u8);
    }
    run(&args).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::doctor::RoutedExecutorHealthCheck;
    use gal_foundation::health::{HealthCheck, Severity};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn version_flag_succeeds() {
        assert_eq!(run(&["--version".to_string()]), ExitCode::Success);
    }

    #[test]
    fn help_flag_succeeds() {
        assert_eq!(run(&["--help".to_string()]), ExitCode::Success);
    }

    #[test]
    fn no_args_prints_help_and_exits_success() {
        // Bare `gal` must exit 0 (winget sandbox bare-executes the binary).
        assert_eq!(run(&[]), ExitCode::Success);
    }

    #[test]
    fn unknown_command_is_usage() {
        assert_eq!(run(&["frobnicate".to_string()]), ExitCode::Usage);
    }

    // update/sync/dispatch are wired (return Success or Error, not NotWired)
    #[test]
    fn update_is_wired_not_not_wired() {
        let result = run(&["update".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "update must be wired");
    }

    #[test]
    fn dispatch_is_wired_not_not_wired() {
        let result = run(&["dispatch".to_string(), "--help".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "dispatch must be wired");
    }

    #[test]
    fn pipeline_without_task_spec_is_usage_not_not_wired() {
        let result = run(&["pipeline".to_string()]);
        assert_eq!(
            result,
            ExitCode::Usage,
            "pipeline without a task spec should be a usage error"
        );
    }

    #[test]
    fn xmachine_is_retired_unknown_command() {
        // xmachine was retired (replaced by the SSH dispatch lane) — `gal xmachine`
        // is now simply an unrecognized command, not a special-cased usage error.
        let result = run(&["xmachine".to_string()]);
        assert_eq!(
            result,
            ExitCode::Usage,
            "xmachine should be classified as an unknown command"
        );
    }

    // doctor is wired
    #[test]
    fn doctor_is_wired_not_not_wired() {
        let result = run(&["doctor".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "doctor must be wired");
    }

    #[test]
    fn doctor_dry_run_is_wired() {
        let result = run(&["doctor".to_string(), "--dry-run".to_string()]);
        assert_ne!(result, ExitCode::NotWired, "doctor --dry-run must be wired");
    }

    #[test]
    fn doctor_command_remains_wired_after_domain_health_aggregation() {
        let result = run(&["doctor".to_string()]);
        assert_ne!(
            result,
            ExitCode::NotWired,
            "doctor must stay wired after aggregation"
        );
    }

    #[test]
    fn routed_executor_healthcheck_warns_when_routing_file_missing() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("missing-routing.json");

        let findings = RoutedExecutorHealthCheck::with_path(missing).check();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0]
            .message
            .contains("skipping routed-executor readiness check"));
    }

    #[test]
    fn routed_executor_healthcheck_warns_for_indeterminate_executor() {
        let tmp = TempDir::new().unwrap();
        let routing = tmp.path().join("config.json");
        fs::write(
            &routing,
            "{\n  \"executorRouting\": {\n    \"executors\": { \"codex\": \"test-codex-model\" },\n    \"pipeline\": { \"CODER\": { \"executor\": \"codex\" } }\n  }\n}\n",
        )
        .unwrap();

        // Inject the probes so the result does not depend on the host's PATH,
        // file permissions or `codex` authentication state.
        let findings = RoutedExecutorHealthCheck::with_path(routing).check_with(
            &|_| dispatch::availability::Availability::Available {
                path: std::path::PathBuf::from("codex"),
                evidence: Vec::new(),
            },
            &|_| dispatch::dispatch::Readiness::Unknown {
                message: "authentication could not be confirmed".to_string(),
            },
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0]
            .message
            .contains("routed executor 'codex' readiness indeterminate"));
    }

    #[test]
    fn routed_executor_healthcheck_reports_denied_launch_path_without_indeterminate_wording() {
        let tmp = TempDir::new().unwrap();
        let routing = tmp.path().join("config.json");
        fs::write(
            &routing,
            "{\n  \"executorRouting\": {\n    \"executors\": { \"codex\": \"test-codex-model\" },\n    \"pipeline\": { \"CODER\": { \"executor\": \"codex\" } }\n  }\n}\n",
        )
        .unwrap();

        // A sandbox that cannot read a PATH entry yields `Denied`, which has its
        // own message and must not be reported as a missing executor.
        let findings = RoutedExecutorHealthCheck::with_path(routing).check_with(
            &|_| dispatch::availability::Availability::Denied {
                evidence: Vec::new(),
            },
            &|_| dispatch::dispatch::Readiness::Ready,
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0]
            .message
            .contains("routed executor 'codex' launch path access was denied"));
        assert!(!findings[0].message.contains("readiness indeterminate"));
        assert!(!findings[0].message.contains("not found"));
    }

    // commit-msg is wired
    #[test]
    fn commit_msg_without_arg_returns_usage() {
        let result = run(&["commit-msg".to_string()]);
        assert_eq!(
            result,
            ExitCode::Usage,
            "commit-msg with no file arg should be Usage"
        );
    }

    #[test]
    fn dispatch_script_is_wired() {
        // dispatch-script is now wired — it emits a dispatch block and exits 0,
        // never NotWired. (Bare invocation → unknown-intent error block, still exit 0.)
        let result = run(&["dispatch-script".to_string()]);
        assert_eq!(
            result,
            ExitCode::Success,
            "dispatch-script must be wired and exit 0"
        );
    }

    #[test]
    fn dispatch_script_direct_golem_emits_role_block() {
        // A directly callable role with no pipeline context → direct block (run via the
        // entry point, exercising the real FS readers against this repo's state).
        // Emits ROLE: golem-architect and MODE: direct (see dispatch_script::model::golem_mode).
        let result = run(&["dispatch-script".to_string(), "golem-architect".to_string()]);
        assert_eq!(result, ExitCode::Success);
    }

    #[test]
    fn exit_code_values_are_correct() {
        assert_eq!(ExitCode::Success as u8, 0);
        assert_eq!(ExitCode::Error as u8, 1);
        assert_eq!(ExitCode::NotWired as u8, 2);
        assert_eq!(ExitCode::Usage as u8, 64);
    }

    #[test]
    fn private_executable_read_failure_is_rejected() {
        let path = std::path::Path::new("target/gal-pipeline/bin/hash/gal");
        let result = validate_private_executable(path, |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "denied",
            ))
        });
        assert!(result
            .unwrap_err()
            .contains("cannot read bound private executable"));
    }

    #[test]
    fn invalid_private_hash_directory_is_rejected() {
        #[cfg(windows)]
        let path = {
            use std::os::windows::ffi::OsStringExt;
            let mut path = std::ffi::OsString::from("target/gal-pipeline/bin/");
            path.push(std::ffi::OsString::from_wide(&[0xD800]));
            path.push("/gal");
            std::path::PathBuf::from(path)
        };
        #[cfg(unix)]
        let path = {
            use std::os::unix::ffi::OsStringExt;
            let mut path = std::ffi::OsString::from("target/gal-pipeline/bin/");
            path.push(std::ffi::OsString::from_vec(b"invalid-\xff".to_vec()));
            path.push("/gal");
            std::path::PathBuf::from(path)
        };
        #[cfg(not(any(windows, unix)))]
        let path = std::path::PathBuf::from("target/gal-pipeline/bin/invalid/gal");
        let result = validate_private_executable(&path, |_| Ok(Vec::new()));
        assert!(result.unwrap_err().contains("no valid hash directory"));
    }
}
