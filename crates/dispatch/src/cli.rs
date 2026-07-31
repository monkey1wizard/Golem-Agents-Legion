//! CLI argument parsing for `gal-dispatch`.
//!
//! Accepted flags:
//!   --phase   <implement|test|audit>          (required)
//!   --task    <T-NN>                           (required)
//!   --workdir <path>                            (optional; defaults to cwd)
//!   --timeout <seconds>                         (optional; defaults to 300)

use crate::dispatch::ContractProvenance;
use crate::stage::{Phase, PhaseParseError};
use std::path::PathBuf;
use thiserror::Error;

/// Parsed CLI options for a `gal-dispatch` invocation.
#[derive(Debug, Clone)]
pub struct DispatchArgs {
    pub phase: Phase,
    pub task: String,
    pub workdir: PathBuf,
    pub timeout_secs: u64,
    /// Routing file override (optional; defaults to `~/.gal/config/config.json#executorRouting`).
    pub routing_path: Option<PathBuf>,
    /// Expected receipt file path for write-back verification.
    ///
    /// When set, the dispatcher verifies the file exists and is non-empty after the executor
    /// exits 0. Missing or empty file → terminal state `no-receipt`.
    pub receipt_path: Option<PathBuf>,
    /// Optional override for the executor-log directory (defaults to
    /// `<workdir>/.dev/executor-logs/` via [`crate::dispatch::SpawnConfig::default_log_dir`]
    /// when unset). Lets a caller (e.g. the executor-smoke self-test) run-scope its logs
    /// instead of polluting the root executor-log directory. `None` is byte-identical to
    /// today's behavior.
    pub log_dir_override: Option<PathBuf>,
    /// When `true`, the `--- GAL DISPATCH ---` / `Dispatch:` banner lines print to stderr
    /// instead of stdout, so a caller capturing clean machine-readable stdout (e.g. a
    /// `--json` smoke report) is not corrupted by dispatch's own banner text. `false`
    /// (the default) is byte-identical to today's behavior.
    pub stdout_quiet: bool,
    /// Coupled contract path+source for pipeline-created dispatches (resolved
    /// and read by cli, see `crates/cli/src/commands/dispatch.rs`). Not exposed
    /// as a CLI flag — argv parsing always leaves this `None`; callers such as
    /// the pipeline command set it programmatically. `None` is byte-identical
    /// to today's raw/direct behavior.
    pub contract_provenance: Option<ContractProvenance>,
}

impl Default for DispatchArgs {
    fn default() -> Self {
        Self {
            phase: Phase::Implement,
            task: String::new(),
            workdir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            timeout_secs: 300,
            routing_path: None,
            receipt_path: None,
            log_dir_override: None,
            stdout_quiet: false,
            contract_provenance: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum CliParseError {
    #[error("missing required flag --phase")]
    MissingPhase,
    #[error("missing required flag --task")]
    MissingTask,
    #[error("invalid phase: {0}")]
    BadPhase(#[from] PhaseParseError),
    #[error("flag {0} requires a value")]
    MissingValue(&'static str),
    #[error("invalid timeout value '{0}': must be a positive integer")]
    BadTimeout(String),
    #[error("unknown flag '{0}'")]
    UnknownFlag(String),
}

/// Parse `gal-dispatch` CLI arguments from a slice (not including argv[0]).
pub fn parse_args(args: &[String]) -> Result<DispatchArgs, CliParseError> {
    let mut phase: Option<Phase> = None;
    let mut task: Option<String> = None;
    let mut workdir: Option<PathBuf> = None;
    let mut timeout_secs: u64 = 300;
    let mut routing_path: Option<PathBuf> = None;
    let mut receipt_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--phase" => {
                i += 1;
                let v = args.get(i).ok_or(CliParseError::MissingValue("--phase"))?;
                phase = Some(Phase::from_str(v)?);
            }
            "--task" => {
                i += 1;
                let v = args.get(i).ok_or(CliParseError::MissingValue("--task"))?;
                task = Some(v.clone());
            }
            "--workdir" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or(CliParseError::MissingValue("--workdir"))?;
                workdir = Some(PathBuf::from(v));
            }
            "--timeout" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or(CliParseError::MissingValue("--timeout"))?;
                timeout_secs = v
                    .parse::<u64>()
                    .map_err(|_| CliParseError::BadTimeout(v.clone()))?;
                if timeout_secs == 0 {
                    return Err(CliParseError::BadTimeout(v.clone()));
                }
            }
            "--routing" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or(CliParseError::MissingValue("--routing"))?;
                routing_path = Some(PathBuf::from(v));
            }
            "--receipt" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or(CliParseError::MissingValue("--receipt"))?;
                receipt_path = Some(PathBuf::from(v));
            }
            other if other.starts_with('-') => {
                return Err(CliParseError::UnknownFlag(other.to_string()));
            }
            _ => {} // positional args ignored for now
        }
        i += 1;
    }

    Ok(DispatchArgs {
        phase: phase.ok_or(CliParseError::MissingPhase)?,
        task: task.ok_or(CliParseError::MissingTask)?,
        workdir: workdir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        timeout_secs,
        routing_path,
        receipt_path,
        // Not exposed as CLI flags (no caller needs them from argv today); set
        // programmatically by callers such as the executor-smoke self-test and
        // the pipeline command's contract-provenance resolution.
        log_dir_override: None,
        stdout_quiet: false,
        contract_provenance: None,
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::Phase;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_minimal_required_flags() {
        let a = parse_args(&args(&["--phase", "implement", "--task", "T-NN"])).unwrap();
        assert_eq!(a.phase, Phase::Implement);
        assert_eq!(a.task, "T-NN");
        assert_eq!(a.timeout_secs, 300);
    }

    #[test]
    fn parses_all_flags() {
        let a = parse_args(&args(&[
            "--phase",
            "test",
            "--task",
            "T-NN",
            "--workdir",
            "/tmp/work",
            "--timeout",
            "120",
        ]))
        .unwrap();
        assert_eq!(a.phase, Phase::Test);
        assert_eq!(a.task, "T-NN");
        assert_eq!(a.workdir, PathBuf::from("/tmp/work"));
        assert_eq!(a.timeout_secs, 120);
    }

    #[test]
    fn missing_phase_is_error() {
        let r = parse_args(&args(&["--task", "T-NN"]));
        assert!(matches!(r, Err(CliParseError::MissingPhase)));
    }

    #[test]
    fn missing_task_is_error() {
        let r = parse_args(&args(&["--phase", "audit"]));
        assert!(matches!(r, Err(CliParseError::MissingTask)));
    }

    #[test]
    fn bad_phase_is_error() {
        let r = parse_args(&args(&["--phase", "deploy", "--task", "T-NN"]));
        assert!(matches!(r, Err(CliParseError::BadPhase(_))));
    }

    #[test]
    fn zero_timeout_is_error() {
        let r = parse_args(&args(&[
            "--phase",
            "implement",
            "--task",
            "T-NN",
            "--timeout",
            "0",
        ]));
        assert!(matches!(r, Err(CliParseError::BadTimeout(_))));
    }

    #[test]
    fn non_numeric_timeout_is_error() {
        let r = parse_args(&args(&[
            "--phase",
            "implement",
            "--task",
            "T-NN",
            "--timeout",
            "forever",
        ]));
        assert!(matches!(r, Err(CliParseError::BadTimeout(_))));
    }

    #[test]
    fn unknown_flag_is_error() {
        let r = parse_args(&args(&[
            "--phase",
            "implement",
            "--task",
            "T-NN",
            "--frobnicate",
        ]));
        assert!(matches!(r, Err(CliParseError::UnknownFlag(_))));
    }

    #[test]
    fn flag_missing_value_is_error() {
        let r = parse_args(&args(&[
            "--phase",
            "implement",
            "--task",
            "T-NN",
            "--workdir",
        ]));
        assert!(matches!(r, Err(CliParseError::MissingValue(_))));
    }

    #[test]
    fn routing_path_override_parsed() {
        let a = parse_args(&args(&[
            "--phase",
            "audit",
            "--task",
            "T-NN",
            "--routing",
            "/custom/config.json",
        ]))
        .unwrap();
        assert_eq!(a.routing_path, Some(PathBuf::from("/custom/config.json")));
    }

    #[test]
    fn receipt_path_parsed() {
        let a = parse_args(&args(&[
            "--phase",
            "implement",
            "--task",
            "T-NN",
            "--receipt",
            "/tmp/receipt.txt",
        ]))
        .unwrap();
        assert_eq!(a.receipt_path, Some(PathBuf::from("/tmp/receipt.txt")));
    }

    #[test]
    fn receipt_path_absent_gives_none() {
        let a = parse_args(&args(&["--phase", "implement", "--task", "T-NN"])).unwrap();
        assert!(a.receipt_path.is_none());
    }
}
