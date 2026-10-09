//! `gal finalize-check <plan-or-prompt> --receipt <path>` — finalize-internal,
//! deterministic zero-trust precondition checks.
//!
//! This subcommand is the mechanization lever for `/gal finalize`'s zero-trust
//! contract: the deterministic checks run here (in the binary) and emit a machine
//! `Receipt`, so finalize trusts the receipt rather than "the model ran it".
//! It is **finalize-internal**, not a public `/gal` control-plane command.
//!
//! Skeleton scope: argument parsing, the `Receipt` model, the exit-code contract
//! (any failed check → nonzero), and a deterministic stub receipt with no checks
//! wired yet. The individual deterministic checks land in follow-up tasks.

use super::converge_check::read_validated_bytes;
use super::dispatch::resolve_receipt_path;
use super::finalize_hygiene::{
    check_contract_roster_parity, check_doc_link_resolution, check_project_source_doc_existence,
    check_state_bound, is_gal_source_repo, HygieneMode,
};
use gal_engine::ExitCode;
use regex::Regex;
use sha2::Digest;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Terminal state of a single check, mirroring the gal-pipeline terminal-state
/// vocabulary so finalize and the pipeline share one language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CheckState {
    /// Ran and passed.
    Pass,
    /// Ran and failed.
    Fail,
    /// Could not run (e.g. no authoritative command declared); never a silent pass.
    NotRun,
}

impl CheckState {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            CheckState::Pass => "pass",
            CheckState::Fail => "fail",
            CheckState::NotRun => "not-run",
        }
    }
}

/// One check's machine-captured outcome.
#[derive(Debug, Clone)]
pub(crate) struct CheckOutcome {
    /// Stable check id, e.g. `authoritative-command`, `commit-existence`.
    pub(crate) name: String,
    /// The command or operation performed (for audit), if any.
    pub(crate) command: Option<String>,
    pub(crate) state: CheckState,
    /// One-line summary or content hash that explains the outcome.
    pub(crate) summary: String,
}

/// The machine receipt finalize reads as its sole pass-basis. Shared by every
/// deterministic check subcommand (`boundary-check`, `converge-check`,
/// `planning-check`, `prompt-check`, `refining-check`, `pipeline-preflight`,
/// `finalize-check`) — this struct stays check-family-agnostic. Finalize's
/// full/hygiene-only mode line is rendered separately by
/// `render_finalize_receipt`, not baked in here.
#[derive(Debug, Clone, Default)]
pub(crate) struct Receipt {
    pub(crate) checks: Vec<CheckOutcome>,
}

/// Identity recorded beside deterministic gate evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReceiptEnvelope {
    pub(crate) binding_scope: String,
    pub(crate) execution_binding_sha256: String,
    pub(crate) executable_path: String,
    pub(crate) executable_sha256: String,
    pub(crate) plan_scope: Option<String>,
    pub(crate) coordinator_path: Option<String>,
}

pub(crate) fn receipt_envelope(
    scope: Option<&str>,
    _plan_path: Option<&Path>,
) -> Result<ReceiptEnvelope, String> {
    let worktree = super::pipeline_execution::current_worktree_root().map_err(|e| e.to_string())?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let roots = vec![
        worktree.clone(),
        executable
            .parent()
            .ok_or("executable has no parent")?
            .to_path_buf(),
    ];
    let identity = super::pipeline_execution::identify(&worktree, &executable, &roots)
        .map_err(|e| e.to_string())?;
    let executable_path = super::pipeline_execution::binding_path_text(&identity.executable);
    let worktree_root = super::pipeline_execution::binding_path_text(&identity.worktree);
    let executable_sha256 = format!(
        "{:x}",
        sha2::Sha256::digest(std::fs::read(&identity.executable).map_err(|e| e.to_string())?)
    );
    let (binding_scope, plan_scope, coordinator_path) =
        if let Some(scope) = scope.filter(|s| *s != "standalone") {
            let declared = scope
                .strip_prefix("coordinator:")
                .ok_or("finalize scope must be standalone or coordinator:<plan-scope>")?;
            if declared.is_empty() || declared.contains(['/', '\\']) {
                return Err("invalid coordinator plan scope".into());
            }
            let path = identity
                .worktree
                .join(".dev/pipeline")
                .join(declared)
                .join("coordinator.json");
            let state: pipeline::coordinator::CoordinatorState = serde_json::from_slice(
                &std::fs::read(&path).map_err(|e| format!("coordinator unavailable: {e}"))?,
            )
            .map_err(|e| format!("coordinator malformed: {e}"))?;
            let binding = state
                .execution_binding
                .as_ref()
                .ok_or("coordinator execution binding missing")?;
            if state.plan_scope != declared
                || binding.binding_scope != scope
                || binding.worktree_root != worktree_root
                || binding.executable_path != executable_path
                || binding.executable_sha256 != executable_sha256
            {
                return Err("coordinator execution binding mismatch".into());
            }
            (
                scope.to_string(),
                Some(declared.to_string()),
                Some(
                    path.canonicalize()
                        .map_err(|e| e.to_string())?
                        .to_string_lossy()
                        .replace('\\', "/"),
                ),
            )
        } else {
            ("standalone".into(), None, None)
        };
    let binding = pipeline::coordinator::ExecutionBinding {
        version: pipeline::coordinator::ExecutionBinding::VERSION,
        binding_scope: binding_scope.clone(),
        worktree_root,
        executable_path: executable_path.clone(),
        executable_sha256: executable_sha256.clone(),
    };
    let execution_binding_sha256 = binding.digest().map_err(|e| e.to_string())?;
    if identity.lane == super::pipeline_execution::ExecutionLane::Source
        && !executable_path.contains("/target/gal-pipeline/bin/")
    {
        return Err("source worktree receipts require its worktree-private GAL executable".into());
    }
    Ok(ReceiptEnvelope {
        binding_scope,
        execution_binding_sha256,
        executable_path,
        executable_sha256,
        plan_scope,
        coordinator_path,
    })
}

pub(crate) fn render_receipt_envelope(envelope: &ReceiptEnvelope) -> String {
    format!(
        "binding_scope: {}\nexecution_binding_sha256: {}\nexecutable_path: {}\nexecutable_sha256: {}\n{}",
        envelope.binding_scope, envelope.execution_binding_sha256, envelope.executable_path, envelope.executable_sha256,
        match (&envelope.plan_scope, &envelope.coordinator_path) { (Some(scope), Some(path)) => format!("plan_scope: {scope}\ncoordinator_path: {path}\n"), _ => String::new() }
    )
}

pub(crate) fn bind_receipt(body: String, envelope: &ReceiptEnvelope) -> Result<String, String> {
    let bound = format!("{body}\n{}", render_receipt_envelope(envelope));
    verify_receipt_envelope(&bound, envelope)?;
    Ok(bound)
}

pub(crate) fn verify_receipt_envelope(
    text: &str,
    expected: &ReceiptEnvelope,
) -> Result<(), String> {
    let actual = parse_receipt_envelope(text)?;
    if actual.binding_scope.starts_with("coordinator:") {
        let scope = actual.binding_scope.strip_prefix("coordinator:").unwrap();
        let path = PathBuf::from(
            actual
                .coordinator_path
                .as_ref()
                .ok_or("coordinator path missing")?,
        );
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("coordinator unavailable: {e}"))?;
        if canonical.to_string_lossy().replace('\\', "/")
            != *actual.coordinator_path.as_ref().unwrap()
        {
            return Err("coordinator path is not canonical".into());
        }
        let state: pipeline::coordinator::CoordinatorState =
            serde_json::from_slice(&std::fs::read(&canonical).map_err(|e| e.to_string())?)
                .map_err(|e| format!("coordinator malformed: {e}"))?;
        let binding = state
            .execution_binding
            .as_ref()
            .ok_or("coordinator execution binding missing")?;
        if state.plan_scope != scope
            || actual.plan_scope.as_deref() != Some(scope)
            || binding.binding_scope != actual.binding_scope
            || binding.executable_path != actual.executable_path
            || binding.executable_sha256 != actual.executable_sha256
        {
            return Err("coordinator execution binding mismatch".into());
        }
    }
    let executable = PathBuf::from(&actual.executable_path);
    let bytes =
        std::fs::read(&executable).map_err(|e| format!("receipt executable unavailable: {e}"))?;
    let executable_sha256 = format!("{:x}", sha2::Sha256::digest(bytes));
    let cwd = super::pipeline_execution::current_worktree_root().map_err(|e| e.to_string())?;
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let roots = vec![
        cwd.clone(),
        current_exe
            .parent()
            .ok_or("executable has no parent")?
            .to_path_buf(),
    ];
    let identity = super::pipeline_execution::identify(&cwd, &current_exe, &roots)
        .map_err(|e| e.to_string())?;
    if super::pipeline_execution::binding_path_text(&identity.executable) != actual.executable_path
    {
        return Err("receipt executable path mismatch".into());
    }
    let binding = pipeline::coordinator::ExecutionBinding {
        version: pipeline::coordinator::ExecutionBinding::VERSION,
        binding_scope: actual.binding_scope.clone(),
        worktree_root: super::pipeline_execution::binding_path_text(&identity.worktree),
        executable_path: actual.executable_path.clone(),
        executable_sha256: executable_sha256.clone(),
    };
    let digest = binding.digest().map_err(|e| e.to_string())?;
    if executable_sha256 != actual.executable_sha256 || digest != actual.execution_binding_sha256 {
        return Err("receipt execution binding digest or executable hash mismatch".into());
    }
    if &actual == expected {
        Ok(())
    } else {
        Err("receipt execution identity mismatch".to_string())
    }
}

pub(crate) fn parse_receipt_envelope(text: &str) -> Result<ReceiptEnvelope, String> {
    let fields: Vec<_> = [
        "binding_scope",
        "execution_binding_sha256",
        "executable_path",
        "executable_sha256",
        "plan_scope",
        "coordinator_path",
    ]
    .into_iter()
    .map(|key| {
        let values: Vec<_> = text
            .lines()
            .filter_map(|line| line.strip_prefix(&format!("{key}: ")).map(str::trim))
            .collect();
        if values.len() > 1 {
            return Err(format!("duplicate receipt field {key}"));
        }
        Ok(values.first().copied())
    })
    .collect::<Result<_, _>>()?;
    let required = |index: usize, name: &str| {
        fields[index]
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("receipt missing or empty {name}"))
    };
    let binding_scope = required(0, "binding_scope")?.to_string();
    let execution_binding_sha256 = required(1, "execution_binding_sha256")?.to_string();
    let executable_path = required(2, "executable_path")?.to_string();
    let executable_sha256 = required(3, "executable_sha256")?.to_string();
    let plan_scope = fields[4].map(str::to_string);
    let coordinator_path = fields[5].map(str::to_string);
    let coordinator = binding_scope.strip_prefix("coordinator:");
    if binding_scope != "standalone" && coordinator.is_none() {
        return Err("invalid receipt binding scope".into());
    }
    if let Some(scope) = coordinator {
        if scope.is_empty()
            || plan_scope.as_deref() != Some(scope)
            || coordinator_path.as_deref().unwrap_or("").is_empty()
        {
            return Err("malformed receipt scope envelope".into());
        }
    } else if plan_scope.is_some() || coordinator_path.is_some() {
        return Err("standalone receipt has coordinator fields".into());
    }
    if execution_binding_sha256.len() != 64
        || !execution_binding_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
        || executable_sha256.len() != 64
        || !executable_sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("malformed receipt digest".into());
    }
    Ok(ReceiptEnvelope {
        binding_scope,
        execution_binding_sha256,
        executable_path,
        executable_sha256,
        plan_scope,
        coordinator_path,
    })
}

impl Receipt {
    /// A receipt passes only when **every** check passed. An empty receipt
    /// (no checks wired yet) passes vacuously; `NotRun` never passes.
    pub(crate) fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.state == CheckState::Pass)
    }

    /// Exit-code contract: any non-pass check → `Error`; all pass → `Success`.
    pub(crate) fn exit_code(&self) -> ExitCode {
        if self.passed() {
            ExitCode::Success
        } else {
            ExitCode::Error
        }
    }
}

/// Parsed `finalize-check` arguments.
struct Args {
    target: PathBuf,
    receipt: PathBuf,
    mode: HygieneMode,
    durable_commit: Option<String>,
    scope: String,
}

/// Default receipt location: gitignored `.dev/pipeline/<scope>/` (reuses the area
/// established by the closed `fix-pipeline-verify-role-and-receipt` plan). Full and
/// hygiene-only modes use distinct default filenames so neither run overwrites the
/// other's evidence.
fn default_receipt_path(target: &Path, mode: HygieneMode) -> Result<PathBuf, String> {
    let filename = match mode {
        HygieneMode::Full => "finalize-check.receipt.md",
        HygieneMode::HygieneOnly => "finalize-check.hygiene.receipt.md",
    };
    resolve_receipt_path(Some(target), None, filename)
        .map_err(|error| format!("receipt scope resolution: {error}"))
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut target: Option<PathBuf> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut mode = HygieneMode::Full;
    let mut durable_commit: Option<String> = None;
    let mut scope = "standalone".to_string();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--receipt" => {
                let v = it.next().ok_or("--receipt requires a path")?;
                receipt = Some(PathBuf::from(v));
            }
            "--hygiene-only" => {
                mode = HygieneMode::HygieneOnly;
            }
            "--durable-commit" => {
                let v = it.next().ok_or("--durable-commit requires a hash")?;
                durable_commit = Some(v.clone());
            }
            "--scope" => {
                scope = it
                    .next()
                    .ok_or("--scope requires standalone or coordinator:<plan-scope>")?
                    .clone();
            }
            s if s.starts_with("--") => return Err(format!("unknown option '{s}'")),
            s => {
                if target.is_none() {
                    target = Some(PathBuf::from(s));
                } else {
                    return Err(format!("unexpected extra argument '{s}'"));
                }
            }
        }
    }
    let target = target.ok_or("finalize-check requires a <plan-or-prompt> path")?;
    let receipt = match receipt {
        Some(path) => path,
        None => default_receipt_path(&target, mode)?,
    };
    Ok(Args {
        target,
        receipt,
        mode,
        durable_commit,
        scope,
    })
}

/// Locate the authoritative-check command list: the `<!-- gal:authoritative-check -->`
/// marker followed by the next ```json fence. Deterministic structured parse — NOT
/// prose-parse of the Tech-Stack table. Returns `None` when marker/fence/`command`
/// is absent or the fence is not valid JSON.
fn locate_authoritative_commands(project_md: &str) -> Option<Vec<String>> {
    let after_marker = project_md.split_once("<!-- gal:authoritative-check -->")?.1;
    let fence_open = after_marker.find("```json")?;
    let after_open = &after_marker[fence_open + "```json".len()..];
    let fence_close = after_open.find("```")?;
    let json = after_open[..fence_close].trim();
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let arr = value.get("command")?.as_array()?;
    Some(
        arr.iter()
            .filter_map(|c| c.as_str().map(str::to_string))
            .collect(),
    )
}

/// The authoritative checks of `project_md` as the one-line command a dispatched
/// reviewer runs once. Several declared commands join with `&&`, so the first failure
/// stops the run. A missing fence, an empty list, or a command that is empty,
/// multi-line, or holds a backtick is an error: a review never starts without a
/// usable command.
pub(crate) fn authoritative_review_command(project_md: &str) -> Result<String, String> {
    let commands = locate_authoritative_commands(project_md)
        .ok_or("no <!-- gal:authoritative-check --> json fence in .dev/project.md")?;
    if commands.is_empty() {
        return Err("authoritative-check fence has an empty command array".to_string());
    }
    for command in &commands {
        if command.trim().is_empty() || command.chars().any(char::is_control) {
            return Err("authoritative-check command is empty or spans several lines".to_string());
        }
        if command.contains('`') {
            return Err("authoritative-check command holds a backtick".to_string());
        }
    }
    Ok(commands
        .iter()
        .map(|command| command.trim())
        .collect::<Vec<_>>()
        .join(" && "))
}

const MAX_AUTHORITATIVE_STREAM_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundedCommandStream {
    text: String,
    original_bytes: usize,
    truncated: bool,
}

/// Capture one command stream without allowing command output to make the receipt
/// unbounded. The byte count is taken before truncation and UTF-8 conversion.
fn capture_command_stream(bytes: &[u8]) -> BoundedCommandStream {
    let original_bytes = bytes.len();
    let truncated = original_bytes > MAX_AUTHORITATIVE_STREAM_BYTES;
    let limit = original_bytes.min(MAX_AUTHORITATIVE_STREAM_BYTES);
    let mut end = limit;
    while end > 0 && std::str::from_utf8(&bytes[..end]).is_err() {
        end -= 1;
    }
    let text = String::from_utf8_lossy(&bytes[..end])
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('"', "\\\"")
        .replace('\r', "\\r")
        .replace('\n', "\\n");
    BoundedCommandStream {
        text,
        original_bytes,
        truncated,
    }
}

fn format_command_stream(label: &str, stream: &BoundedCommandStream) -> String {
    format!(
        "{label}_bytes={}; {label}_truncated={}; {label}=\"{}\"",
        stream.original_bytes, stream.truncated, stream.text
    )
}

/// Execute one command **verbatim** (whitespace-split argv, no shell interpolation).
/// Returns (passed, summary). A nonzero exit or spawn failure → not passed.
fn run_command_verbatim(command: &str) -> (bool, String) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let Some((program, rest)) = parts.split_first() else {
        return (
            false,
            "spawn_error=empty command; stdout_bytes=0; stdout_truncated=false; stdout=\"\"; stderr_bytes=0; stderr_truncated=false; stderr=\"\""
                .to_string(),
        );
    };
    match std::process::Command::new(program).args(rest).output() {
        Ok(out) => {
            let stdout = capture_command_stream(&out.stdout);
            let stderr = capture_command_stream(&out.stderr);
            let status = out
                .status
                .code()
                .map_or_else(|| "unknown".to_string(), |code| code.to_string());
            (
                out.status.success(),
                format!(
                    "exit_status={status}; {}; {}",
                    format_command_stream("stdout", &stdout),
                    format_command_stream("stderr", &stderr)
                ),
            )
        }
        Err(e) => (
            false,
            format!(
                "spawn_error={}; stdout_bytes=0; stdout_truncated=false; stdout=\"\"; stderr_bytes=0; stderr_truncated=false; stderr=\"\"",
                e
            ),
        ),
    }
}

/// check(a): read `<repo_root>/.dev/project.md` authoritative commands, run each
/// verbatim, plus a naming-gate scan. Missing declaration → a single `NotRun`
/// outcome (never a silent pass).
fn check_authoritative(repo_root: &Path) -> Vec<CheckOutcome> {
    let project_md = repo_root.join(".dev").join("project.md");
    let commands = std::fs::read_to_string(&project_md)
        .ok()
        .and_then(|md| locate_authoritative_commands(&md));

    let mut outcomes = Vec::new();
    match commands {
        None => outcomes.push(CheckOutcome {
            name: "authoritative-command".to_string(),
            command: None,
            state: CheckState::NotRun,
            summary: "no <!-- gal:authoritative-check --> json fence in .dev/project.md"
                .to_string(),
        }),
        Some(cmds) if cmds.is_empty() => outcomes.push(CheckOutcome {
            name: "authoritative-command".to_string(),
            command: None,
            state: CheckState::NotRun,
            summary: "authoritative-check fence has an empty command array".to_string(),
        }),
        Some(cmds) => {
            for cmd in cmds {
                let (passed, summary) = run_command_verbatim(&cmd);
                outcomes.push(CheckOutcome {
                    name: "authoritative-command".to_string(),
                    command: Some(cmd),
                    state: if passed {
                        CheckState::Pass
                    } else {
                        CheckState::Fail
                    },
                    summary,
                });
            }
        }
    }

    // naming-gate scan via the library (no self-spawn).
    outcomes.push(check_naming_gate(repo_root));
    outcomes
}

/// check(g): prove that the repository is clean using Git's machine-readable
/// status format. Any inability to obtain or decode the authoritative output
/// fails closed. Both finalize modes call this as their terminal evaluator.
/// Outcome of reading `git status --porcelain=v1 --untracked-files=all`.
/// Shared so the fail-closed ladder exists once. Callers format their own
/// summary, because the finalize and preflight receipts word theirs
/// differently and those strings are contract-visible.
pub(crate) enum WorkingTreeStatus {
    SpawnError(String),
    Undecodable {
        status: String,
        error: String,
    },
    Read {
        success: bool,
        status: String,
        dirty_entries: usize,
        output_bytes: usize,
    },
}

pub(crate) fn read_working_tree_status(repo_root: &Path) -> WorkingTreeStatus {
    let output = match std::process::Command::new("git")
        .current_dir(repo_root)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()
    {
        Ok(output) => output,
        Err(error) => return WorkingTreeStatus::SpawnError(error.to_string()),
    };
    let status = output
        .status
        .code()
        .map_or_else(|| "unknown".to_string(), |code| code.to_string());
    let stdout = match String::from_utf8(output.stdout) {
        Ok(stdout) => stdout,
        Err(error) => {
            return WorkingTreeStatus::Undecodable {
                status,
                error: error.to_string(),
            }
        }
    };
    WorkingTreeStatus::Read {
        success: output.status.success(),
        status,
        dirty_entries: stdout.lines().filter(|line| !line.is_empty()).count(),
        output_bytes: stdout.len(),
    }
}

fn check_working_tree_clean(repo_root: &Path) -> CheckOutcome {
    let command = "git status --porcelain=v1 --untracked-files=all".to_string();
    evaluate_working_tree_status(read_working_tree_status(repo_root), command)
}

/// Formats the shared status reading into this receipt's row. The reading and
/// its fail-closed ladder live in `read_working_tree_status`; only the wording
/// is local, because the receipt text is contract-visible.
fn evaluate_working_tree_status(status: WorkingTreeStatus, command: String) -> CheckOutcome {
    let name = "working-tree-clean".to_string();
    match status {
        WorkingTreeStatus::SpawnError(error) => CheckOutcome {
            name,
            command: Some(command),
            state: CheckState::Fail,
            summary: format!("spawn_error={error}; dirty_entries=unknown"),
        },
        WorkingTreeStatus::Undecodable { status, error } => CheckOutcome {
            name,
            command: Some(command),
            state: CheckState::Fail,
            summary: format!(
                "exit_status={status}; undecodable_stdout={error}; dirty_entries=unknown"
            ),
        },
        WorkingTreeStatus::Read {
            success,
            status,
            dirty_entries,
            output_bytes,
        } => CheckOutcome {
            name,
            command: Some(command),
            state: if success && dirty_entries == 0 {
                CheckState::Pass
            } else {
                CheckState::Fail
            },
            summary: format!(
                "exit_status={status}; dirty_entries={dirty_entries}; status_output_bytes={output_bytes}"
            ),
        },
    }
}

/// naming-gate over the repo tree, run in-process via the gal_engine library.
fn check_naming_gate(repo_root: &Path) -> CheckOutcome {
    use crate::gal::naming_gate::{load_retired_terms, NamingGate};
    let retired = load_retired_terms(repo_root);
    match NamingGate::new(&retired) {
        Ok(gate) => match gate.scan_tree(repo_root) {
            Ok(hits) => {
                let total: usize = hits.iter().map(|f| f.violations.len()).sum();
                CheckOutcome {
                    name: "naming-gate".to_string(),
                    command: Some("gal naming-gate".to_string()),
                    state: if total == 0 {
                        CheckState::Pass
                    } else {
                        CheckState::Fail
                    },
                    summary: format!("{total} violation(s) across {} file(s)", hits.len()),
                }
            }
            Err(e) => CheckOutcome {
                name: "naming-gate".to_string(),
                command: Some("gal naming-gate".to_string()),
                state: CheckState::Fail,
                summary: format!("scan failed: {e}"),
            },
        },
        Err(e) => CheckOutcome {
            name: "naming-gate".to_string(),
            command: Some("gal naming-gate".to_string()),
            state: CheckState::Fail,
            summary: format!("scanner build failed: {e}"),
        },
    }
}

/// A `[x]` task line's recorded commit short-hash, if any. Looks for the `*(hash)*`
/// commit-note convention on a completed task line. `converge_check`'s
/// `check_task_commit` is the production caller.
pub(crate) fn commit_note_hash(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("- [x]") {
        return None;
    }
    let open = line.rfind("*(")?;
    let rest = &line[open + 2..];
    let close = rest.find(")*")?;
    let token = rest[..close].trim();
    // A commit hash is hex, 7..=40 chars. Reject prose like "(verify-only)".
    if (7..=40).contains(&token.len()) && token.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(token.to_string())
    } else {
        None
    }
}

/// Ordered blocking task ids projected from the real `## Tasks` section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TaskCheckboxProjection {
    pub(crate) checked: Vec<String>,
    pub(crate) unchecked: Vec<String>,
}

/// Parse the blocking task checkbox surface without interpreting prose or
/// checkboxes in other sections. Malformed task lines are ignored.
pub(crate) fn task_checkbox_projection(plan_text: &str) -> TaskCheckboxProjection {
    let lines: Vec<&str> = plan_text.lines().collect();
    let Some(start) = lines.iter().position(|line| line.trim() == "## Tasks") else {
        return TaskCheckboxProjection::default();
    };
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with("## "))
        .map(|offset| start + 1 + offset)
        .unwrap_or(lines.len());

    let mut projection = TaskCheckboxProjection::default();
    for line in &lines[start + 1..end] {
        let trimmed = line.trim_start();
        let (checked, rest) = if let Some(rest) = trimmed.strip_prefix("- [x] ") {
            (true, rest)
        } else if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            (false, rest)
        } else {
            continue;
        };
        let Some(raw) = rest.split_whitespace().next() else {
            continue;
        };
        let id = raw.trim_matches('*');
        if !id.starts_with("T-") || id.len() <= 2 || !id[2..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if checked {
            projection.checked.push(id.to_string());
        } else {
            projection.unchecked.push(id.to_string());
        }
    }
    projection
}

/// Set of `T-NN` ids that are checked (`[x]`) in the Tasks section.
pub(crate) fn checked_task_ids(plan_text: &str) -> BTreeSet<String> {
    task_checkbox_projection(plan_text)
        .checked
        .into_iter()
        .collect()
}

/// check(c): source plan ↔ execution prompt must agree on which `T-NN` are `[x]`.
/// (`.dev/state.md` carries a continuity row, not per-task checkboxes, so the
/// machine-checkable checkbox surfaces are the source plan and the prompt.)
pub(crate) fn check_three_surface(
    prompt_path: &Path,
    prompt_text: &str,
    repo_root: &Path,
) -> CheckOutcome {
    let name = "three-surface-checkbox".to_string();
    // Derive source plan: .dev/plans/<slug>.prompt.md → .dev/plans/<slug>.md
    let slug = prompt_path
        .file_name()
        .and_then(|f| f.to_str())
        .and_then(|f| f.strip_suffix(".prompt.md"));
    let Some(slug) = slug else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::NotRun,
            summary: "target is not a *.prompt.md execution prompt".to_string(),
        };
    };
    let source_rel = Path::new(".dev").join("plans").join(format!("{slug}.md"));
    let source_text = match read_validated_bytes(repo_root, &source_rel) {
        Ok(Some(bytes)) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                return CheckOutcome {
                    name,
                    command: None,
                    state: CheckState::Fail,
                    summary: format!("paired source plan is not UTF-8: {}", source_rel.display()),
                };
            }
        },
        Ok(None) => {
            return CheckOutcome {
                name,
                command: None,
                state: CheckState::NotRun,
                summary: format!("paired source plan not found: {}", source_rel.display()),
            };
        }
        Err(error) => {
            return CheckOutcome {
                name,
                command: None,
                state: CheckState::Fail,
                summary: format!("paired source plan validation failed: {error}"),
            };
        }
    };
    let prompt_set = checked_task_ids(prompt_text);
    let source_set = checked_task_ids(&source_text);
    if prompt_set == source_set {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: format!(
                "source plan and prompt agree on {} checked task(s)",
                prompt_set.len()
            ),
        }
    } else {
        let only_prompt: Vec<_> = prompt_set.difference(&source_set).cloned().collect();
        let only_source: Vec<_> = source_set.difference(&prompt_set).cloned().collect();
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!(
                "checkbox disagreement — prompt-only: [{}], source-only: [{}]",
                only_prompt.join(", "),
                only_source.join(", ")
            ),
        }
    }
}

/// Split a markdown table row into trimmed cell strings, dropping the empty
/// leading/trailing cells produced by the outer `|` delimiters.
pub(crate) fn split_md_row(row: &str) -> Vec<String> {
    let row = row.trim();
    let row = row.strip_prefix('|').unwrap_or(row);
    let row = row.strip_suffix('|').unwrap_or(row);
    row.split('|').map(|c| c.trim().to_string()).collect()
}

fn citation_resolves(repo_root: &Path, name: &str) -> bool {
    let crates_dir = repo_root.join("crates");
    grep_exists(&crates_dir, &format!("fn {name}"))
        || grep_exists(&crates_dir, &format!("mod {name}"))
        || rs_file_exists(&crates_dir, name)
        || grep_word_exists(&crates_dir, name)
}

/// Recursively check whether any `*.rs` file under `dir` contains `needle`.
///
/// Skip list must stay in sync with its sibling `rs_file_exists`.
fn grep_exists(dir: &Path, needle: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            if grep_exists(&path, needle) {
                return true;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if content.contains(needle) {
                    return true;
                }
            }
        }
    }
    false
}

/// Recursively check whether any `*.rs` file under `dir` contains `needle` as
/// a whole Rust word, where word characters are ASCII letters, digits, or `_`.
///
/// Keep recursion and the `target` skip in sync with `grep_exists`.
fn grep_word_exists(dir: &Path, needle: &str) -> bool {
    grep_word_exists_below(dir, dir, needle)
}

fn grep_word_exists_below(root: &Path, dir: &Path, needle: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            if grep_word_exists_below(root, &path, needle) {
                return true;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if path == root.join("cli/src/commands/finalize_check.rs") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                if content.match_indices(needle).any(|(start, _)| {
                    let end = start + needle.len();
                    let is_word_char = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
                    content.as_bytes()[..start]
                        .last()
                        .is_none_or(|byte| !is_word_char(*byte))
                        && content.as_bytes()[end..]
                            .first()
                            .is_none_or(|byte| !is_word_char(*byte))
                }) {
                    return true;
                }
            }
        }
    }
    false
}

/// Recursively check whether any `*.rs` file under `dir` has file stem `stem`
/// (a file named `<stem>.rs`), independent of its content.
///
/// Skip list must stay in sync with its sibling `grep_exists`.
fn rs_file_exists(dir: &Path, stem: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            if rs_file_exists(&path, stem) {
                return true;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && path.file_stem().and_then(|s| s.to_str()) == Some(stem)
        {
            return true;
        }
    }
    false
}

/// Report-only classification of every rendered adapter candidate against its
/// on-disk counterpart. Drift never fails `sync-idempotency` — mutation
/// authority belongs to the adapter apply path, not to this checker.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct AdapterDriftSummary {
    pub in_sync: Vec<String>,
    pub drifted: Vec<String>,
    pub absent_on_disk: Vec<String>,
    pub extra_marker_owned: Vec<String>,
    pub filter_personalized: Vec<String>,
}

fn has_gal_config_filter(repo_root: &Path, rel_path: &str) -> bool {
    let gitattrs_path = repo_root.join(".gitattributes");
    if let Ok(content) = std::fs::read_to_string(&gitattrs_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            if let Some(pattern) = parts.next() {
                let pattern = pattern.trim_start_matches("./");
                if pattern == rel_path && parts.any(|attr| attr == "filter=gal-config") {
                    return true;
                }
            }
        }
    }
    false
}

fn analyze_adapter_drift(
    repo_root: &Path,
    rendered: &crate::gal::render::RenderedAdapters,
) -> AdapterDriftSummary {
    let mut summary = AdapterDriftSummary::default();

    for candidate in &rendered.root_candidates {
        let p = repo_root.join(&candidate.path);
        if !p.exists() {
            summary.absent_on_disk.push(candidate.path.clone());
        } else if let Ok(disk_bytes) = std::fs::read(&p) {
            let disk_str = String::from_utf8_lossy(&disk_bytes);
            let disk_norm = disk_str.replace("\r\n", "\n");
            let cand_norm = candidate.content.replace("\r\n", "\n");
            if disk_norm == cand_norm {
                summary.in_sync.push(candidate.path.clone());
            } else if has_gal_config_filter(repo_root, &candidate.path) {
                summary.filter_personalized.push(candidate.path.clone());
            } else {
                summary.drifted.push(candidate.path.clone());
            }
        } else {
            summary.absent_on_disk.push(candidate.path.clone());
        }
    }

    if let Some(layers) = &rendered.conditional_layers {
        for candidate in layers {
            let p = repo_root.join(&candidate.path);
            if !p.exists() {
                summary.absent_on_disk.push(candidate.path.clone());
            } else if let Ok(disk_bytes) = std::fs::read(&p) {
                let disk_str = String::from_utf8_lossy(&disk_bytes);
                let disk_norm = disk_str.replace("\r\n", "\n");
                let cand_norm = candidate.content.replace("\r\n", "\n");
                if disk_norm == cand_norm {
                    summary.in_sync.push(candidate.path.clone());
                } else if has_gal_config_filter(repo_root, &candidate.path) {
                    summary.filter_personalized.push(candidate.path.clone());
                } else {
                    summary.drifted.push(candidate.path.clone());
                }
            } else {
                summary.absent_on_disk.push(candidate.path.clone());
            }
        }
    } else {
        for layer_path in crate::gal::render::REPO_ADAPTER_CONDITIONAL_LAYERS {
            let p = repo_root.join(layer_path);
            if p.exists() {
                if let Ok(disk_bytes) = std::fs::read(&p) {
                    let disk_str = String::from_utf8_lossy(&disk_bytes);
                    if disk_str.contains(crate::gal::render::GAL_LAYER_OWNERSHIP_MARKER) {
                        summary.extra_marker_owned.push((*layer_path).to_string());
                    }
                }
            }
        }
    }

    summary
}

/// check(d): doc-sync candidate render determinism and drift reporting.
/// Runs the injected candidate render action twice in memory; the two candidate
/// sets must be byte-identical (deterministic). Candidate-vs-disk drift is report-only.
fn check_sync_idempotency(
    repo_root: &Path,
    render: impl Fn() -> Result<crate::gal::render::RenderedAdapters, crate::gal::render::AdapterError>,
) -> CheckOutcome {
    let first = match render() {
        Ok(c) => c,
        Err(e) => {
            return CheckOutcome {
                name: "sync-idempotency".to_string(),
                command: Some("adapter render ×2 [in-process]".to_string()),
                state: CheckState::Fail,
                summary: format!("candidate render error: {e}"),
            };
        }
    };
    let second = match render() {
        Ok(c) => c,
        Err(e) => {
            return CheckOutcome {
                name: "sync-idempotency".to_string(),
                command: Some("adapter render ×2 [in-process]".to_string()),
                state: CheckState::Fail,
                summary: format!("candidate render error: {e}"),
            };
        }
    };

    if first != second {
        return CheckOutcome {
            name: "sync-idempotency".to_string(),
            command: Some("adapter render ×2 [in-process]".to_string()),
            state: CheckState::Fail,
            summary:
                "adapter candidate content changed between consecutive renders (non-idempotent)"
                    .to_string(),
        };
    }

    let drift = analyze_adapter_drift(repo_root, &first);
    let summary = format!(
        "candidate render determinism verified: in-sync={}, drifted={}, absent-on-disk={}, extra-marker-owned={}, filter-personalized={}",
        drift.in_sync.len(),
        drift.drifted.len(),
        drift.absent_on_disk.len(),
        drift.extra_marker_owned.len(),
        drift.filter_personalized.len(),
    );

    CheckOutcome {
        name: "sync-idempotency".to_string(),
        command: Some("adapter render ×2 [in-process]".to_string()),
        state: CheckState::Pass,
        summary,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// check(f) — finalize-mode detect
// ─────────────────────────────────────────────────────────────────────────────

/// Run `git -C <repo_root> <args>` and return trimmed stdout on success.
fn git_one_line(repo_root: &Path, args: &[&str]) -> String {
    std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// check(f): mode-detect — purely informational, state is always `Pass`.
/// Injectable for tests: supply the branch name directly.
pub(crate) fn check_finalize_mode_from(branch: &str) -> CheckOutcome {
    let summary = if branch == "main" || branch == "master" {
        "already-on-main".to_string()
    } else if branch.is_empty() {
        "already-on-main (detached HEAD — branch name unavailable)".to_string()
    } else {
        "worktree".to_string()
    };
    CheckOutcome {
        name: "finalize-mode".to_string(),
        command: Some("git branch --show-current".to_string()),
        state: CheckState::Pass,
        summary,
    }
}

fn check_finalize_mode(repo_root: &Path) -> CheckOutcome {
    let branch = git_one_line(repo_root, &["branch", "--show-current"]);
    check_finalize_mode_from(&branch)
}

// ─────────────────────────────────────────────────────────────────────────────
// check(h) — durable-layer commit-hash gate
// ─────────────────────────────────────────────────────────────────────────────

/// check(h): durable-layer commit-hash gate.
/// Verifies exactly the hash the orchestrator supplies via `--durable-commit`
/// (Sequence 5 hygiene-only), asserting existence only — receipt summary
/// contains no authorization language. Absent a supplied hash, reports
/// `NotRun`; it never reports `Pass` without one.
fn check_durable_layer_commit(supplied: Option<&str>, repo_root: &Path) -> CheckOutcome {
    let name = "durable-layer-commit".to_string();
    let Some(hash) = supplied else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::NotRun,
            summary: "not supplied".to_string(),
        };
    };
    let ok = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{hash}^{{commit}}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    CheckOutcome {
        name,
        command: Some(format!("git cat-file -e {hash}")),
        state: if ok {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        summary: if ok {
            format!("exists: {hash}")
        } else {
            format!("missing: {hash}")
        },
    }
}

/// Resolve the prompt path `check_finalize_review_shape` reads, per R9: when
/// `target` already ends in `.prompt.md` it is read as-is, otherwise its
/// sibling `<stem>.prompt.md` in the same directory is read instead (Sequence
/// 5 may pass either the source plan or the execution prompt).
fn resolve_finalize_review_prompt_path(target: &Path) -> PathBuf {
    let is_prompt = target
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.ends_with(".prompt.md"))
        .unwrap_or(false);
    if is_prompt {
        return target.to_path_buf();
    }
    let stem = target.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let dir = target.parent().unwrap_or_else(|| Path::new(""));
    dir.join(format!("{stem}.prompt.md"))
}

/// Extract the body lines of a `## <heading>` (exact-match) section, bounded
/// by the next `## ` heading or end of file. Sibling of
/// `finalize_hygiene::section_body`, kept local because that one is private
/// to its own module.
fn review_section_body<'a>(lines: &'a [&'a str], heading: &str) -> Option<&'a [&'a str]> {
    let idx = lines.iter().position(|l| l.trim() == heading)?;
    let start = idx + 1;
    let end = lines[start..]
        .iter()
        .position(|l| l.starts_with("## "))
        .map(|off| start + off)
        .unwrap_or(lines.len());
    Some(&lines[start..end])
}

/// check(i): `finalize-review-shape` — hygiene-only row mechanizing R8/R9.
/// Resolves the execution prompt per R9, then checks: exactly one
/// `### Finalize Review ` heading under `## Review Results`, a first pipe
/// table whose header names `Requirement`, `L1`, `L2`, `L3`, `L4` and whose
/// data-row count matches the `## Requirements` count, every layer cell in
/// `PASS — <text>` / `FAIL — <text>` / `N/A — <text>` form, no `N/A` L1 cell,
/// no all-`N/A` row, exactly one `Review Independence:` line, and every
/// backticked Rust-identifier-shaped token (containing `_`) inside an L1 cell
/// resolving through `citation_resolves` — skipped, with a pass-summary note,
/// when the repo has no `crates/` tree.
fn check_finalize_review_shape(target: &Path, repo_root: &Path) -> CheckOutcome {
    let prompt_path = resolve_finalize_review_prompt_path(target);
    let Ok(text) = std::fs::read_to_string(&prompt_path) else {
        return CheckOutcome {
            name: FINALIZE_REVIEW_SHAPE_CHECK.to_string(),
            command: None,
            state: CheckState::Fail,
            summary: format!("resolved prompt not found: {}", prompt_path.display()),
        };
    };
    evaluate_finalize_review_text(&text, repo_root, ReviewIndependenceRule::Any)
}

/// Check id shared by the hygiene row and the dispatch-time candidate validation.
const FINALIZE_REVIEW_SHAPE_CHECK: &str = "finalize-review-shape";

/// Value the `Review Independence:` line must carry on a dispatched review.
pub(crate) const DISPATCHED_REVIEW_INDEPENDENCE: &str = "full";

/// How strictly the single `Review Independence:` line is judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReviewIndependenceRule {
    /// Hygiene mode: any value passes, because an in-process review writes
    /// `DEGRADED_SAME_RUNTIME`.
    Any,
    /// A dispatched review must carry exactly [`DISPATCHED_REVIEW_INDEPENDENCE`].
    MustBeFull,
}

/// The `finalize-review-shape` evaluator over prompt text held in memory. The
/// hygiene check reads the prompt file and calls this. The dispatcher calls it with
/// the candidate prompt before replacing the real file, so both paths accept and
/// reject the same review fixtures.
pub(crate) fn evaluate_finalize_review_text(
    text: &str,
    repo_root: &Path,
    independence: ReviewIndependenceRule,
) -> CheckOutcome {
    let name = FINALIZE_REVIEW_SHAPE_CHECK.to_string();
    let fail = |summary: String| CheckOutcome {
        name: name.clone(),
        command: None,
        state: CheckState::Fail,
        summary,
    };

    let lines: Vec<&str> = text.lines().collect();

    let Some(requirements_body) = review_section_body(&lines, "## Requirements") else {
        return fail("no ## Requirements section found".to_string());
    };
    let requirement_re = Regex::new(r"^- \[[ xX]\] R\d+ — ").unwrap();
    let requirement_count = requirements_body
        .iter()
        .filter(|l| requirement_re.is_match(l))
        .count();

    let Some(review_body) = review_section_body(&lines, "## Review Results") else {
        return fail("no ## Review Results section found".to_string());
    };

    let heading_positions: Vec<usize> = review_body
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("### Finalize Review "))
        .map(|(idx, _)| idx)
        .collect();
    match heading_positions.len() {
        0 => {
            return fail("no ### Finalize Review heading found under ## Review Results".to_string())
        }
        1 => {}
        n => {
            return fail(format!(
                "expected exactly one ### Finalize Review heading, found {n}"
            ))
        }
    }
    let heading_idx = heading_positions[0];
    let sub_start = heading_idx + 1;
    let sub_end = review_body[sub_start..]
        .iter()
        .position(|l| l.starts_with("### "))
        .map(|off| sub_start + off)
        .unwrap_or(review_body.len());
    let subsection = &review_body[sub_start..sub_end];

    let Some(header_idx) = subsection
        .iter()
        .position(|l| l.trim_start().starts_with('|'))
    else {
        return fail("no pipe table found after ### Finalize Review heading".to_string());
    };
    let header_cells = split_md_row(subsection[header_idx]);
    let required_headers = ["Requirement", "L1", "L2", "L3", "L4"];
    let mut column_index = std::collections::HashMap::new();
    for header in required_headers {
        let Some(idx) = header_cells.iter().position(|c| c == header) else {
            return fail(format!("table header missing {header} column"));
        };
        column_index.insert(header, idx);
    }

    let mut row_start = header_idx + 1;
    if let Some(sep_line) = subsection.get(row_start) {
        if is_separator_md_row(sep_line) {
            row_start += 1;
        }
    }
    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut idx = row_start;
    while let Some(line) = subsection.get(idx) {
        if !line.trim_start().starts_with('|') {
            break;
        }
        data_rows.push(split_md_row(line));
        idx += 1;
    }
    let table_end = idx;

    if data_rows.len() != requirement_count {
        return fail(format!(
            "table has {} data row(s) against {} ## Requirements entries",
            data_rows.len(),
            requirement_count
        ));
    }

    let cell_re = Regex::new(r"^(PASS|FAIL|N/A) — \S").unwrap();
    let backtick_re = Regex::new(r"`([^`]+)`").unwrap();
    let identifier_re = Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").unwrap();
    let has_crates_tree = repo_root.join("crates").is_dir();

    let mut l1_tokens: Vec<(String, String)> = Vec::new(); // (row id, token)
    for row in &data_rows {
        let req_idx = column_index["Requirement"];
        let row_id = row.get(req_idx).cloned().unwrap_or_default();
        let layer_cells: Vec<(&str, &str)> = ["L1", "L2", "L3", "L4"]
            .iter()
            .map(|layer| {
                let idx = column_index[layer];
                (*layer, row.get(idx).map(String::as_str).unwrap_or(""))
            })
            .collect();
        for (layer, cell) in &layer_cells {
            if !cell_re.is_match(cell) {
                return fail(format!(
                    "row {row_id} cell {layer} is not in PASS/FAIL/N/A — <text> form: {cell:?}"
                ));
            }
        }
        let l1_cell = layer_cells[0].1;
        if l1_cell.starts_with("N/A") {
            return fail(format!("row {row_id} L1 cell is N/A: {l1_cell:?}"));
        }
        if layer_cells.iter().all(|(_, c)| c.starts_with("N/A")) {
            return fail(format!("row {row_id} has all four layer cells N/A"));
        }
        for cap in backtick_re.captures_iter(l1_cell) {
            let token = &cap[1];
            if identifier_re.is_match(token) && token.contains('_') {
                l1_tokens.push((row_id.clone(), token.to_string()));
            }
        }
    }

    if has_crates_tree {
        for (row_id, token) in &l1_tokens {
            if !citation_resolves(repo_root, token) {
                return fail(format!(
                    "row {row_id} L1 cell cites unresolvable test name `{token}`"
                ));
            }
        }
    }

    let independence_lines: Vec<&str> = subsection[table_end..]
        .iter()
        .copied()
        .filter(|l| l.trim_start().starts_with("Review Independence:"))
        .collect();
    if independence_lines.len() != 1 {
        return fail(format!(
            "expected exactly one Review Independence: line, found {}",
            independence_lines.len()
        ));
    }
    if independence == ReviewIndependenceRule::MustBeFull {
        let value = independence_lines[0]
            .trim_start()
            .trim_start_matches("Review Independence:")
            .trim();
        if value != DISPATCHED_REVIEW_INDEPENDENCE {
            return fail(format!(
                "a dispatched review must carry `Review Independence: {DISPATCHED_REVIEW_INDEPENDENCE}`, found {value:?}"
            ));
        }
    }

    CheckOutcome {
        name,
        command: None,
        state: CheckState::Pass,
        summary: if has_crates_tree {
            format!(
                "{} requirement row(s) verified, {} L1 test citation(s) resolved",
                data_rows.len(),
                l1_tokens.len()
            )
        } else {
            format!(
                "{} requirement row(s) verified, skipped: no crates/ tree",
                data_rows.len()
            )
        },
    }
}

/// True when every cell in a split row is a markdown separator cell
/// (`-`/`:`). Sibling of `finalize_hygiene::is_separator_row`, kept local
/// because that one is private to its own module.
fn is_separator_md_row(line: &str) -> bool {
    let cells = split_md_row(line);
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
}

/// Render a `finalize-check` receipt with the exact `mode: full|hygiene-only`
/// line the finalize contract checks at each use site. This wraps the shared
/// `Receipt::render()` rather than adding a `mode` field to `Receipt` itself,
/// since that struct is shared verbatim by six other check subcommands
/// (`boundary-check`, `converge-check`, `planning-check`, `prompt-check`,
/// `refining-check`, `pipeline-preflight`) that have no such concept.
fn render_finalize_receipt(receipt: &Receipt, mode: HygieneMode) -> String {
    let mut out = String::from("# finalize-check receipt\n\n");
    out.push_str(&format!(
        "overall: {}\n",
        if receipt.passed() { "pass" } else { "fail" }
    ));
    out.push_str(&format!("mode: {}\n\n", mode.as_str()));
    out.push_str("| check | state | command | summary |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for c in &receipt.checks {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            c.name,
            c.state.as_str(),
            c.command.as_deref().unwrap_or("—"),
            c.summary,
        ));
    }
    out
}

/// Returns true if `target_rel` matches `.dev/plans/<slug>.prompt.md`.
fn is_canonical_execution_prompt(target_rel: &Path) -> bool {
    let norm = target_rel.to_string_lossy().replace('\\', "/");
    let rel_str = norm.strip_prefix("./").unwrap_or(&norm);
    if let Some(rest) = rel_str.strip_prefix(".dev/plans/") {
        if let Some(slug) = rest.strip_suffix(".prompt.md") {
            return !slug.is_empty() && !slug.contains('/');
        }
    }
    false
}

/// Run `gal finalize-check`. Parse args, run the wired deterministic checks against
/// the repo root, write the receipt, and return the exit code from the receipt's
/// pass-state.
pub(crate) fn cmd_finalize_check(args: &[String]) -> ExitCode {
    // args[0] is the "finalize-check" command token (dispatch passes full argv).
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let parsed = match parse_args(&rest) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("gal finalize-check: {e}");
            return if e.starts_with("receipt scope resolution:") {
                ExitCode::Error
            } else {
                ExitCode::Usage
            };
        }
    };

    // Checks run against the repo root (where .dev/project.md lives).
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let target_rel = parsed
        .target
        .strip_prefix(&repo_root)
        .unwrap_or(parsed.target.as_path());

    if parsed.mode == HygieneMode::Full && !is_canonical_execution_prompt(target_rel) {
        eprintln!(
            "gal finalize-check: target in full mode must be a canonical execution prompt (.dev/plans/<slug>.prompt.md): {}",
            parsed.target.display()
        );
        return ExitCode::Usage;
    }

    let _target_text = match read_validated_bytes(&repo_root, target_rel) {
        Ok(Some(bytes)) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("gal finalize-check: target is not UTF-8: {error}");
                return ExitCode::Usage;
            }
        },
        Ok(None) => {
            eprintln!(
                "gal finalize-check: target not found: {}",
                parsed.target.display()
            );
            return ExitCode::Usage;
        }
        Err(error) => {
            eprintln!("gal finalize-check: target validation failed: {error}");
            return ExitCode::Usage;
        }
    };

    let mut checks = Vec::new();
    if parsed.mode == HygieneMode::Full {
        checks.append(&mut check_authoritative(&repo_root));
        {
            let root = repo_root.clone();
            checks.push(check_sync_idempotency(&repo_root, move || {
                let opts = crate::gal::render::sync_options_from(root.clone(), false)?;
                crate::gal::render::render_candidates(&opts)
            }));
        }
        checks.push(check_finalize_mode(&repo_root));
    }

    // Repo-hygiene rows (see finalize_hygiene): universal rows run in both
    // modes; GAL-source-repo-scoped rows are emitted only when
    // `plugins/gal-core/` exists at the repo root — a downstream repo never
    // sees them at all (absent, not `NotRun`).
    checks.push(check_project_source_doc_existence(&repo_root));
    checks.push(check_state_bound(&repo_root, parsed.mode));
    if is_gal_source_repo(&repo_root) {
        checks.push(check_contract_roster_parity(&repo_root));
        checks.push(check_doc_link_resolution(&repo_root));
    }
    if parsed.mode == HygieneMode::HygieneOnly {
        checks.push(check_finalize_review_shape(&parsed.target, &repo_root));
        checks.push(check_durable_layer_commit(
            parsed.durable_commit.as_deref(),
            &repo_root,
        ));
    }
    checks.push(check_working_tree_clean(&repo_root));

    let receipt = Receipt { checks };

    if let Some(parent) = parsed.receipt.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!(
                "gal finalize-check: cannot create receipt dir {}: {e}",
                parent.display()
            );
            return ExitCode::Error;
        }
    }
    let body = match receipt_envelope(Some(&parsed.scope), Some(&parsed.target)) {
        Ok(envelope) => {
            match bind_receipt(render_finalize_receipt(&receipt, parsed.mode), &envelope) {
                Ok(body) => body,
                Err(e) => {
                    eprintln!("gal finalize-check: invalid execution binding: {e}");
                    return ExitCode::Error;
                }
            }
        }
        Err(e) => {
            eprintln!("gal finalize-check: cannot establish execution identity: {e}");
            return ExitCode::Error;
        }
    };
    if let Err(e) = std::fs::write(&parsed.receipt, body) {
        eprintln!(
            "gal finalize-check: cannot write receipt {}: {e}",
            parsed.receipt.display()
        );
        return ExitCode::Error;
    }

    println!(
        "gal finalize-check: {} ({} check(s)) → {}",
        if receipt.passed() { "pass" } else { "fail" },
        receipt.checks.len(),
        parsed.receipt.display()
    );
    receipt.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_receipt_envelope() -> ReceiptEnvelope {
        use super::super::pipeline_execution::{binding_path_text, current_worktree_root};
        let cwd = current_worktree_root().unwrap();
        let executable = std::env::current_exe().unwrap();
        let roots = vec![cwd.clone(), executable.parent().unwrap().to_path_buf()];
        let identity =
            super::super::pipeline_execution::identify(&cwd, &executable, &roots).unwrap();
        let executable_path = binding_path_text(&identity.executable);
        let executable_sha256 = format!(
            "{:x}",
            sha2::Sha256::digest(std::fs::read(&identity.executable).unwrap())
        );
        let binding = pipeline::coordinator::ExecutionBinding {
            version: pipeline::coordinator::ExecutionBinding::VERSION,
            binding_scope: "standalone".to_string(),
            worktree_root: binding_path_text(&identity.worktree),
            executable_path: executable_path.clone(),
            executable_sha256: executable_sha256.clone(),
        };
        ReceiptEnvelope {
            binding_scope: "standalone".to_string(),
            execution_binding_sha256: binding.digest().unwrap(),
            executable_path,
            executable_sha256,
            plan_scope: None,
            coordinator_path: None,
        }
    }

    #[test]
    fn receipt_envelope_rejects_real_mismatch_matrix() {
        let current = std::env::current_dir().unwrap();
        let _guard = CwdGuard::enter(&current);
        let expected = test_receipt_envelope();
        let valid = render_receipt_envelope(&expected);
        assert!(verify_receipt_envelope(&valid, &expected).is_ok());

        let mut cases = Vec::new();
        let mut changed = expected.clone();
        changed.binding_scope = "coordinator:missing-scope".to_string();
        cases.push(("scope", render_receipt_envelope(&changed)));
        let mut changed = expected.clone();
        changed.execution_binding_sha256 = "0".repeat(64);
        cases.push(("execution digest", render_receipt_envelope(&changed)));
        let mut changed = expected.clone();
        changed.executable_path.push_str("-different");
        cases.push(("executable path", render_receipt_envelope(&changed)));
        let mut changed = expected.clone();
        changed.executable_sha256 = "0".repeat(64);
        cases.push(("executable hash", render_receipt_envelope(&changed)));
        cases.push((
            "malformed envelope",
            "binding_scope: standalone\n".to_string(),
        ));

        let mut changed = expected.clone();
        changed.binding_scope = "coordinator:missing-scope".to_string();
        changed.plan_scope = Some("missing-scope".to_string());
        changed.coordinator_path = Some("/path/that/does/not/exist/coordinator.json".to_string());
        cases.push(("missing coordinator", render_receipt_envelope(&changed)));
        let mut changed = expected.clone();
        changed.binding_scope = "coordinator:missing-scope".to_string();
        changed.plan_scope = Some("wrong-scope".to_string());
        changed.coordinator_path = Some("/path/that/does/not/exist/coordinator.json".to_string());
        cases.push((
            "malformed coordinator scope",
            render_receipt_envelope(&changed),
        ));

        let rejected = cases
            .iter()
            .filter(|(_, candidate)| verify_receipt_envelope(candidate, &expected).is_err())
            .count();
        println!("gate receipt fixtures: planning/refining/prompt/finalize; mismatch variants rejected: {rejected} of {}", cases.len());
        assert_eq!(
            rejected,
            cases.len(),
            "every production verifier mismatch must fail"
        );
    }

    #[test]
    fn receipt_envelope_is_bound_to_canonical_worktree() {
        let worktree_a = TempDir::new().unwrap();
        let worktree_b = TempDir::new().unwrap();
        let _guard = CwdGuard::enter(worktree_a.path());

        let expected_a = receipt_envelope(None, None).unwrap();
        let receipt = render_receipt_envelope(&expected_a);
        verify_receipt_envelope(&receipt, &expected_a)
            .expect("receipt must verify against the worktree that created it");

        let mut expected_b = expected_a.clone();
        let worktree_b_path = worktree_b.path().canonicalize().unwrap();
        let worktree_b_root = worktree_b_path.to_string_lossy().replace('\\', "/");
        let binding = pipeline::coordinator::ExecutionBinding {
            version: pipeline::coordinator::ExecutionBinding::VERSION,
            binding_scope: expected_b.binding_scope.clone(),
            worktree_root: worktree_b_root,
            executable_path: expected_b.executable_path.clone(),
            executable_sha256: expected_b.executable_sha256.clone(),
        };
        expected_b.execution_binding_sha256 = binding.digest().unwrap();

        assert_ne!(worktree_a.path().canonicalize().unwrap(), worktree_b_path);
        assert!(
            verify_receipt_envelope(&receipt, &expected_b).is_err(),
            "a receipt from worktree A must not verify against worktree B"
        );
    }

    #[test]
    fn coordinator_receipt_verifies_and_rejects_rebind() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdGuard::enter(temp.path());
        let scope = "coordinator:binding-test";
        let worktree = std::env::current_dir().unwrap();
        let executable = std::env::current_exe().unwrap();
        let roots = vec![worktree.clone(), executable.parent().unwrap().to_path_buf()];
        let identity =
            super::super::pipeline_execution::identify(&worktree, &executable, &roots).unwrap();
        let executable_path = identity.executable.to_string_lossy().replace('\\', "/");
        let executable_sha256 = format!(
            "{:x}",
            sha2::Sha256::digest(std::fs::read(&identity.executable).unwrap())
        );
        let make_binding = |hash: String| pipeline::coordinator::ExecutionBinding {
            version: pipeline::coordinator::ExecutionBinding::VERSION,
            binding_scope: scope.to_string(),
            worktree_root: identity.worktree.to_string_lossy().replace('\\', "/"),
            executable_path: executable_path.clone(),
            executable_sha256: hash,
        };
        let coordinator_path = temp
            .path()
            .join(".dev/pipeline/binding-test/coordinator.json");
        std::fs::create_dir_all(coordinator_path.parent().unwrap()).unwrap();
        let mut state = pipeline::coordinator::CoordinatorState::new_legacy(
            "binding-test",
            "prompt",
            "T-EV",
            "implement",
        );
        state.execution_binding = Some(make_binding(executable_sha256.clone()));
        std::fs::write(&coordinator_path, serde_json::to_vec(&state).unwrap()).unwrap();

        let expected = receipt_envelope(Some(scope), None).unwrap();
        assert_eq!(expected.binding_scope, scope);
        assert_eq!(expected.plan_scope.as_deref(), Some("binding-test"));
        assert_eq!(
            expected.coordinator_path.as_deref(),
            Some(
                coordinator_path
                    .canonicalize()
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
                    .as_str()
            )
        );
        let receipt = bind_receipt("overall: pass".to_string(), &expected).unwrap();
        let parsed = parse_receipt_envelope(&receipt).unwrap();
        assert_eq!(
            parsed.execution_binding_sha256,
            expected.execution_binding_sha256
        );
        verify_receipt_envelope(&receipt, &expected).unwrap();

        state.execution_binding = Some(make_binding("f".repeat(64)));
        std::fs::write(&coordinator_path, serde_json::to_vec(&state).unwrap()).unwrap();
        assert!(
            verify_receipt_envelope(&receipt, &expected).is_err(),
            "rebound coordinator must invalidate old receipt"
        );
    }

    #[test]
    fn guarded_entry_binding_is_accepted_by_coordinator_receipt_envelope() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdGuard::enter(temp.path());
        // The guarded entry binds canonicalized paths, which carry the Windows
        // verbatim prefix; the receipt envelope must still accept them.
        let worktree = std::fs::canonicalize(temp.path()).unwrap();
        let executable = std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
        let binding = super::super::pipeline_driver::coordinator_execution_binding(
            "entry-binding",
            &worktree,
            &executable,
        )
        .unwrap();
        assert!(!binding.worktree_root.starts_with("//?/"));
        assert!(!binding.executable_path.starts_with("//?/"));
        let coordinator_path = temp
            .path()
            .join(".dev/pipeline/entry-binding/coordinator.json");
        std::fs::create_dir_all(coordinator_path.parent().unwrap()).unwrap();
        let mut state = pipeline::coordinator::CoordinatorState::new_legacy(
            "entry-binding",
            "prompt",
            "task",
            "implement",
        );
        state.execution_binding = Some(binding.clone());
        std::fs::write(&coordinator_path, serde_json::to_vec(&state).unwrap()).unwrap();

        let expected = receipt_envelope(Some("coordinator:entry-binding"), None)
            .expect("coordinator-bound envelope must accept the guarded entry binding");
        assert_eq!(expected.executable_path, binding.executable_path);
        assert_eq!(expected.execution_binding_sha256, binding.digest().unwrap());
        let receipt = bind_receipt("overall: pass".to_string(), &expected).unwrap();
        verify_receipt_envelope(&receipt, &expected).unwrap();
    }

    fn outcome(name: &str, state: CheckState) -> CheckOutcome {
        CheckOutcome {
            name: name.to_string(),
            command: None,
            state,
            summary: String::new(),
        }
    }

    #[test]
    fn working_tree_status_with_undecodable_stdout_fails_closed() {
        let result = evaluate_working_tree_status(
            WorkingTreeStatus::Undecodable {
                status: "0".to_string(),
                error: "invalid utf-8 sequence".to_string(),
            },
            "git status --porcelain=v1 --untracked-files=all".to_string(),
        );

        assert_eq!(result.state, CheckState::Fail);
        assert!(result.summary.contains("undecodable_stdout="));
        assert!(result.summary.contains("dirty_entries=unknown"));
    }

    #[test]
    fn check_naming_gate_fails_closed_outside_git_repo() {
        // No `git init`: scan_tree cannot prove a clean result and must fail
        // the check, not report a false Pass.
        let tmp = TempDir::new().unwrap();
        let result = check_naming_gate(tmp.path());
        assert_eq!(result.state, CheckState::Fail);
        assert!(
            result.summary.contains("scan failed"),
            "{:?}",
            result.summary
        );
    }

    #[test]
    fn empty_receipt_passes_vacuously() {
        let r = Receipt::default();
        assert!(r.passed());
        assert_eq!(r.exit_code(), ExitCode::Success);
    }

    #[test]
    fn all_pass_is_success() {
        let r = Receipt {
            checks: vec![
                outcome("a", CheckState::Pass),
                outcome("b", CheckState::Pass),
            ],
        };
        assert!(r.passed());
        assert_eq!(r.exit_code(), ExitCode::Success);
    }

    #[test]
    fn any_fail_is_nonzero() {
        let r = Receipt {
            checks: vec![
                outcome("a", CheckState::Pass),
                outcome("b", CheckState::Fail),
            ],
        };
        assert!(!r.passed());
        assert_eq!(r.exit_code(), ExitCode::Error);
    }

    #[test]
    fn not_run_never_passes() {
        let r = Receipt {
            checks: vec![outcome("a", CheckState::NotRun)],
        };
        assert!(!r.passed());
        assert_eq!(r.exit_code(), ExitCode::Error);
    }

    #[test]
    fn render_lists_each_check_and_overall() {
        let r = Receipt {
            checks: vec![outcome("commit-existence", CheckState::Pass)],
        };
        let md = render_finalize_receipt(&r, HygieneMode::Full);
        assert!(md.contains("overall: pass"));
        assert!(md.contains("mode: full"));
        assert!(md.contains("commit-existence"));
        assert!(md.contains("| check | state | command | summary |"));
    }

    #[test]
    fn render_finalize_receipt_hygiene_only_mode_line() {
        let r = Receipt {
            checks: vec![outcome("state-bound", CheckState::Fail)],
        };
        let md = render_finalize_receipt(&r, HygieneMode::HygieneOnly);
        assert!(md.contains("overall: fail"));
        assert!(md.contains("mode: hygiene-only"));
    }

    #[test]
    fn parse_requires_target() {
        assert!(parse_args(&[]).is_err());
    }

    #[test]
    fn parse_target_and_receipt() {
        let a = parse_args(&[
            ".dev/plans/x.md".to_string(),
            "--receipt".to_string(),
            "out/r.md".to_string(),
        ])
        .unwrap();
        assert_eq!(a.target, PathBuf::from(".dev/plans/x.md"));
        assert_eq!(a.receipt, PathBuf::from("out/r.md"));
    }

    #[test]
    fn parse_defaults_receipt_under_dev_pipeline_scope() {
        let a = parse_args(&[".dev/plans/x.md".to_string()]).unwrap();
        assert!(a.receipt.ends_with("finalize-check.receipt.md"));
        assert!(a
            .receipt
            .to_string_lossy()
            .replace('\\', "/")
            .contains(".dev/pipeline/x"));
    }

    #[test]
    fn parse_rejects_unknown_option() {
        assert!(parse_args(&["x".to_string(), "--frob".to_string()]).is_err());
    }

    #[test]
    fn parse_defaults_to_full_mode() {
        let a = parse_args(&[".dev/plans/x.md".to_string()]).unwrap();
        assert_eq!(a.mode, HygieneMode::Full);
        assert!(a.receipt.ends_with("finalize-check.receipt.md"));
    }

    #[test]
    fn parse_hygiene_only_flag_switches_mode_and_default_receipt() {
        let a = parse_args(&[".dev/plans/x.md".to_string(), "--hygiene-only".to_string()]).unwrap();
        assert_eq!(a.mode, HygieneMode::HygieneOnly);
        assert!(a.receipt.ends_with("finalize-check.hygiene.receipt.md"));
    }

    #[test]
    fn parse_hygiene_only_with_explicit_receipt_keeps_explicit_path() {
        let a = parse_args(&[
            ".dev/plans/x.md".to_string(),
            "--hygiene-only".to_string(),
            "--receipt".to_string(),
            "out/custom.md".to_string(),
        ])
        .unwrap();
        assert_eq!(a.mode, HygieneMode::HygieneOnly);
        assert_eq!(a.receipt, PathBuf::from("out/custom.md"));
    }

    #[test]
    fn parse_durable_commit_requires_a_value() {
        let result = parse_args(&[
            ".dev/plans/x.md".to_string(),
            "--durable-commit".to_string(),
        ]);
        match result {
            Err(e) => assert!(e.contains("--durable-commit") && e.contains("requires")),
            Ok(_) => panic!("expected an error, mirroring --receipt's missing-value shape"),
        }
    }

    #[test]
    fn parse_durable_commit_populates_args() {
        let a = parse_args(&[
            ".dev/plans/x.md".to_string(),
            "--durable-commit".to_string(),
            "abc123f".to_string(),
        ])
        .unwrap();
        assert_eq!(a.durable_commit, Some("abc123f".to_string()));
    }

    #[test]
    fn full_and_hygiene_only_default_receipt_paths_are_distinct() {
        let target = Path::new(".dev/plans/finalize-scope.prompt.md");
        assert_ne!(
            default_receipt_path(target, HygieneMode::Full),
            default_receipt_path(target, HygieneMode::HygieneOnly)
        );
    }

    // --- check(a) ---

    #[test]
    fn locate_finds_commands_in_fence() {
        let md = "intro\n\n<!-- gal:authoritative-check -->\n```json\n{ \"command\": [\"cargo test --workspace\", \"cargo clippy --workspace\"] }\n```\n\nmore";
        let cmds = locate_authoritative_commands(md).unwrap();
        assert_eq!(
            cmds,
            vec!["cargo test --workspace", "cargo clippy --workspace"]
        );
    }

    #[test]
    fn locate_returns_none_without_marker() {
        let md = "no marker here\n```json\n{ \"command\": [\"x\"] }\n```\n";
        assert!(locate_authoritative_commands(md).is_none());
    }

    #[test]
    fn locate_returns_none_on_invalid_json() {
        let md = "<!-- gal:authoritative-check -->\n```json\n{ not json }\n```\n";
        assert!(locate_authoritative_commands(md).is_none());
    }

    #[test]
    fn run_command_verbatim_passes_on_exit_zero() {
        // `cargo --version` exists in the test env, exits 0, and is NOT a nested
        // test run (no `cargo test` recursion).
        let (passed, _summary) = run_command_verbatim("cargo --version");
        assert!(passed);
    }

    #[test]
    fn run_command_verbatim_fails_on_missing_program() {
        let (passed, summary) = run_command_verbatim("definitely-not-a-real-command-xyz123");
        assert!(!passed);
        assert!(summary.contains("spawn_error="));
    }

    #[test]
    fn check_authoritative_not_run_when_fence_absent() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".dev")).unwrap();
        std::fs::write(
            tmp.path().join(".dev").join("project.md"),
            "# no fence here\n",
        )
        .unwrap();
        let outcomes = check_authoritative(tmp.path());
        let auth = outcomes
            .iter()
            .find(|o| o.name == "authoritative-command")
            .unwrap();
        assert_eq!(auth.state, CheckState::NotRun);
    }

    #[test]
    fn init_template_authoritative_check_fence() {
        // `run_init_repo` resolves the checkout source from the process CWD.
        // Serialize with tests that temporarily change CWD so this assertion
        // does not race with another module's fixture setup.
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = make_git_repo();
        let opts = crate::init_repo::InitRepoOptions {
            target_path: tmp.path().to_path_buf(),
            project_name: Some("test-project".to_string()),
            blank: false,
        };
        crate::init_repo::run_init_repo(&opts).unwrap();
        let project_md_path = tmp.path().join(".dev").join("project.md");
        let project_md = std::fs::read_to_string(&project_md_path).unwrap();

        let cmds = locate_authoritative_commands(&project_md)
            .expect("init template authoritative-check fence parses to one bracketed placeholder");
        assert_eq!(
            cmds.len(),
            1,
            "init template authoritative-check fence parses to one bracketed placeholder"
        );

        let cmd = &cmds[0];
        let first_token = cmd
            .split_whitespace()
            .next()
            .expect("authoritative check command must have at least one whitespace token");
        assert!(
            first_token.starts_with('['),
            "first whitespace token of authoritative check placeholder command must start with '[': {cmd}"
        );

        let outcomes = check_authoritative(tmp.path());
        let auth = outcomes
            .iter()
            .find(|o| o.name == "authoritative-command")
            .expect("authoritative-command outcome missing");

        assert_eq!(
            auth.state,
            CheckState::Fail,
            "authoritative check placeholder command must fail execution"
        );
        assert!(
            auth.summary.starts_with("spawn_error="),
            "authoritative check summary must start with spawn_error=: {}",
            auth.summary
        );
        assert_eq!(
            auth.command.as_deref(),
            Some(cmd.as_str()),
            "authoritative check outcome command must carry placeholder verbatim"
        );
    }

    // Compose plan-task IDs at runtime — never hardcode `T-NN` literals in source
    // (naming-gate provenance rule: plan-task IDs belong only in .dev).
    fn tid(n: u32) -> String {
        format!("T-{n:02}")
    }

    #[test]
    fn commit_note_hash_extracts_only_hex() {
        assert_eq!(
            commit_note_hash(&format!("- [x] {} — do thing. *(ff62d9c)*", tid(1))),
            Some("ff62d9c".to_string())
        );
        assert_eq!(
            commit_note_hash(&format!("- [x] {} — verify. *(verify-only)*", tid(5))),
            None
        );
        assert_eq!(
            commit_note_hash(&format!("- [ ] {} — pending *(ff62d9c)*", tid(2))),
            None
        );
    }

    // --- check(c): three-surface ---

    #[test]
    fn checked_task_ids_collects_x_only() {
        let txt = format!(
            "## Tasks\n\n- [x] {} — a\n- [ ] {} — b\n- [x] {} — c *(abc)*\n\n## Test Plan\n",
            tid(1),
            tid(2),
            tid(3)
        );
        let ids = checked_task_ids(&txt);
        assert!(ids.contains(&tid(1)) && ids.contains(&tid(3)) && !ids.contains(&tid(2)));
    }

    #[test]
    fn task_checkbox_projection_preserves_checked_ids_and_orders_unchecked_ids() {
        let txt = format!(
            "## Notes\n- [x] {} — out of section\n\n## Tasks\n\n- [x] {} — done\n- [ ] {} — next\n- [x] malformed\n- [ ] **{}** — later\n- [ ] T-nope — malformed\n\n## Test Plan\n- [ ] {} — out of section\n",
            tid(9),
            tid(1),
            tid(2),
            tid(3),
            tid(8),
        );
        assert_eq!(
            task_checkbox_projection(&txt),
            TaskCheckboxProjection {
                checked: vec![tid(1)],
                unchecked: vec![tid(2), tid(3)],
            }
        );
    }

    #[test]
    fn task_checkbox_projection_requires_tasks_section() {
        let txt = format!("- [x] {} — prose\n- [ ] {} — prose\n", tid(1), tid(2));
        assert_eq!(
            task_checkbox_projection(&txt),
            TaskCheckboxProjection::default()
        );
    }

    #[test]
    fn three_surface_fails_on_disagreement() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".dev").join("plans")).unwrap();
        std::fs::write(
            tmp.path().join(".dev").join("plans").join("demo.md"),
            format!("## Tasks\n\n- [x] {} — a\n- [ ] {} — b\n", tid(1), tid(2)),
        )
        .unwrap();
        let prompt_path = tmp.path().join("demo.prompt.md");
        // second id checked in prompt but not source → disagreement
        let prompt_text = format!("## Tasks\n\n- [x] {} — a\n- [x] {} — b\n", tid(1), tid(2));
        let out = check_three_surface(&prompt_path, &prompt_text, tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains(&tid(2)));
    }

    // --- check(d): sync idempotency ---

    #[test]
    fn sync_idempotency_passes_when_adapters_stable() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), "stable content\n").unwrap();
        let out = check_sync_idempotency(tmp.path(), || {
            Ok(crate::gal::render::RenderedAdapters {
                root_candidates: vec![crate::gal::render::AdapterCandidate {
                    path: "CLAUDE.md".to_string(),
                    content: "stable content\n".to_string(),
                }],
                conditional_layers: None,
            })
        });
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("in-sync=1"));
    }

    #[test]
    fn sync_idempotency_fails_when_adapter_changes_between_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let counter = std::cell::Cell::new(0u32);
        // candidate render nondeterminism: returns different candidate on second render
        let out = check_sync_idempotency(tmp.path(), || {
            let n = counter.get();
            counter.set(n + 1);
            Ok(crate::gal::render::RenderedAdapters {
                root_candidates: vec![crate::gal::render::AdapterCandidate {
                    path: "CLAUDE.md".to_string(),
                    content: format!("v{n}\n"),
                }],
                conditional_layers: None,
            })
        });
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("non-idempotent"));
    }

    #[test]
    fn sync_idempotency_no_adapters_is_benign() {
        let tmp = tempfile::tempdir().unwrap();
        // Candidate returns roots but disk is empty -> reported as absent-on-disk, check passes
        let out = check_sync_idempotency(tmp.path(), || {
            Ok(crate::gal::render::RenderedAdapters {
                root_candidates: vec![crate::gal::render::AdapterCandidate {
                    path: "CLAUDE.md".to_string(),
                    content: "rendered content\n".to_string(),
                }],
                conditional_layers: None,
            })
        });
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("absent-on-disk=1"));
    }

    #[test]
    fn sync_idempotency_fails_when_a_conditional_layer_changes_between_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let counter = std::cell::Cell::new(0u32);
        let out = check_sync_idempotency(tmp.path(), || {
            let n = counter.get();
            counter.set(n + 1);
            Ok(crate::gal::render::RenderedAdapters {
                root_candidates: vec![],
                conditional_layers: Some([
                    crate::gal::render::AdapterCandidate {
                        path: ".agents/rules/rust.md".to_string(),
                        content: format!("v{n}\n"),
                    },
                    crate::gal::render::AdapterCandidate {
                        path: ".claude/rules/rust.md".to_string(),
                        content: "stable\n".to_string(),
                    },
                ]),
            })
        });
        assert_eq!(
            out.state,
            CheckState::Fail,
            "a changed conditional layer must fail idempotency even with all roots stable, got {out:?}"
        );
        assert!(out.summary.contains("non-idempotent"));
    }

    #[test]
    fn sync_idempotency_passes_when_the_full_adapter_inventory_is_stable() {
        let tmp = tempfile::tempdir().unwrap();
        let mut root_candidates = Vec::new();
        for root in crate::gal::render::REPO_ADAPTER_ROOTS {
            let p = tmp.path().join(root);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, format!("{root} content\n")).unwrap();
            root_candidates.push(crate::gal::render::AdapterCandidate {
                path: (*root).to_string(),
                content: format!("{root} content\n"),
            });
        }
        let mut layer_candidates = Vec::new();
        for layer in crate::gal::render::REPO_ADAPTER_CONDITIONAL_LAYERS {
            let p = tmp.path().join(layer);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, format!("{layer} content\n")).unwrap();
            layer_candidates.push(crate::gal::render::AdapterCandidate {
                path: (*layer).to_string(),
                content: format!("{layer} content\n"),
            });
        }

        let out = check_sync_idempotency(tmp.path(), || {
            Ok(crate::gal::render::RenderedAdapters {
                root_candidates: root_candidates.clone(),
                conditional_layers: Some([
                    layer_candidates[0].clone(),
                    layer_candidates[1].clone(),
                ]),
            })
        });
        assert_eq!(out.state, CheckState::Pass);
        let expected = crate::gal::render::REPO_ADAPTER_ROOTS.len()
            + crate::gal::render::REPO_ADAPTER_CONDITIONAL_LAYERS.len();
        assert!(
            out.summary.contains(&format!("in-sync={expected}")),
            "expected the summary to count all {expected} present paths, got {:?}",
            out.summary
        );
    }

    #[test]
    fn sync_idempotency_fails_closed_on_candidate_render_error() {
        let tmp = tempfile::tempdir().unwrap();
        // Candidate render error must fail check_sync_idempotency closed.
        // Today check_sync_idempotency only inspects on-disk snapshots and passes when unchanged,
        // ignoring candidate render errors.
        let out = check_sync_idempotency(tmp.path(), || {
            Err(crate::gal::render::AdapterError::Message(
                "candidate render error".to_string(),
            ))
        });
        assert_eq!(
            out.state,
            CheckState::Fail,
            "sync-idempotency must fail closed on a candidate render error"
        );
    }

    #[test]
    fn sync_idempotency_fails_when_candidate_renders_differ() {
        let tmp = tempfile::tempdir().unwrap();
        // When two candidate renders differ in memory (without mutating disk),
        // check_sync_idempotency must detect candidate divergence and fail.
        // Today check_sync_idempotency compares disk snapshots, so non-mutating candidate divergence passes.
        let counter = std::cell::Cell::new(0u32);
        let out = check_sync_idempotency(tmp.path(), || {
            let n = counter.get();
            counter.set(n + 1);
            Ok(crate::gal::render::RenderedAdapters {
                root_candidates: vec![crate::gal::render::AdapterCandidate {
                    path: "CLAUDE.md".to_string(),
                    content: format!("v{n}\n"),
                }],
                conditional_layers: None,
            })
        });
        assert_eq!(
            out.state,
            CheckState::Fail,
            "sync-idempotency must fail when two candidate renders differ"
        );
    }

    #[test]
    fn sync_idempotency_filter_personalized_counts_correctly() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(".gitattributes"),
            "CLAUDE.md filter=gal-config\nAGENTS.md filter=gal-config\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), "local personalized\n").unwrap();
        std::fs::write(tmp.path().join("AGENTS.md"), "local personalized\n").unwrap();
        std::fs::write(tmp.path().join("GEMINI.md"), "unfiltered drifted\n").unwrap();

        let out = check_sync_idempotency(tmp.path(), || {
            Ok(crate::gal::render::RenderedAdapters {
                root_candidates: vec![
                    crate::gal::render::AdapterCandidate {
                        path: "CLAUDE.md".to_string(),
                        content: "rendered template\n".to_string(),
                    },
                    crate::gal::render::AdapterCandidate {
                        path: "AGENTS.md".to_string(),
                        content: "rendered template\n".to_string(),
                    },
                    crate::gal::render::AdapterCandidate {
                        path: "GEMINI.md".to_string(),
                        content: "rendered template\n".to_string(),
                    },
                ],
                conditional_layers: None,
            })
        });
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("filter-personalized=2"));
        assert!(out.summary.contains("drifted=1"));
    }

    // --- check(f): finalize-mode ---

    #[test]
    fn mode_detect_feature_branch_reports_worktree() {
        let out = check_finalize_mode_from("feature-branch");
        assert_eq!(out.state, CheckState::Pass);
        assert_eq!(out.summary, "worktree");
    }

    #[test]
    fn mode_detect_main_reports_already_on_main() {
        let out = check_finalize_mode_from("main");
        assert_eq!(out.state, CheckState::Pass);
        assert_eq!(out.summary, "already-on-main");
    }

    #[test]
    fn mode_detect_state_always_pass() {
        for branch in &["main", "master", "feat-foo", "fix/bar", ""] {
            let out = check_finalize_mode_from(branch);
            assert_eq!(
                out.state,
                CheckState::Pass,
                "branch {:?} must always be Pass",
                branch
            );
        }
    }

    /// Hermetic git repo with an identity configured (needed for a real commit).
    fn make_git_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        std::process::Command::new("git")
            .args(["init", root])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "config", "user.email", "t@t"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "config", "user.name", "t"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "config", "commit.gpgsign", "false"])
            .output()
            .unwrap();
        tmp
    }

    /// Stage and commit everything in `tmp`, returning the new commit's short hash.
    fn commit_all(tmp: &TempDir, msg: &str) -> String {
        let root = tmp.path().to_str().unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "add", "-A"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "commit", "-m", msg])
            .output()
            .unwrap();
        let out = std::process::Command::new("git")
            .args(["-C", root, "rev-parse", "--short", "HEAD"])
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    // --- check(h): durable-layer commit ---

    #[test]
    fn durable_layer_commit_reachable_hash_is_pass() {
        let repo = make_git_repo();
        std::fs::write(repo.path().join("f.txt"), "x").unwrap();
        let hash = commit_all(&repo, "seed");
        let out = check_durable_layer_commit(Some(&hash), repo.path());
        assert_eq!(out.state, CheckState::Pass);
        assert_eq!(out.summary, format!("exists: {hash}"));
    }

    #[test]
    fn durable_layer_commit_supplied_hash_wins_over_any_task_line_note() {
        // A plan carrying an `- [x]` commit note is in scope but never passed
        // to `check_durable_layer_commit` (no `plan_text` parameter exists on
        // this signature) — proves the `next_back()` scrape is deleted, not
        // merely shadowed.
        let repo = make_git_repo();
        std::fs::write(repo.path().join("f.txt"), "x").unwrap();
        let real_hash = commit_all(&repo, "seed");
        let _unrelated_plan_with_note =
            format!("- [x] {} — done. *(0000000deadbeef0000000)*\n", tid(1));
        let out = check_durable_layer_commit(Some(&real_hash), repo.path());
        assert_eq!(out.state, CheckState::Pass);
        assert_eq!(out.summary, format!("exists: {real_hash}"));
    }

    #[test]
    fn durable_layer_commit_no_hash_supplied_is_not_run() {
        let out = check_durable_layer_commit(None, Path::new("."));
        assert_eq!(out.state, CheckState::NotRun);
        assert_eq!(out.summary, "not supplied");
    }

    #[test]
    fn durable_layer_commit_bogus_hash_is_fail() {
        let out = check_durable_layer_commit(Some("0000000deadbeef0000000"), Path::new("."));
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.starts_with("missing:"));
    }

    #[test]
    fn durable_layer_commit_summary_has_no_delete_language() {
        for supplied in [None, Some("0000000aaabbbccddee0")] {
            let out = check_durable_layer_commit(supplied, Path::new("."));
            assert!(
                !out.summary.contains("delet"),
                "summary must not mention delete"
            );
            assert!(
                !out.summary.contains("authoriz"),
                "summary must not mention authorize"
            );
        }
    }

    // --- check(i): finalize-review-shape ---
    //
    // These tests are written from the spec text in the source prompt, not from any implementation.

    /// Writes `<tmp>/.dev/plans/demo.prompt.md` with a fixed two-requirement
    /// `## Requirements` block and the given `## Review Results` body glued on
    /// underneath, returning the prompt's path.
    fn write_prompt_with_review(tmp: &TempDir, review_body: &str) -> PathBuf {
        let plans_dir = tmp.path().join(".dev").join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        let text = format!(
            "## Requirements\n\n\
             - [ ] R1 — first requirement\n\
             - [x] R2 — second requirement\n\n\
             ## Review Results\n\n\
             {review_body}\n"
        );
        let path = plans_dir.join("demo.prompt.md");
        std::fs::write(&path, text).unwrap();
        path
    }

    /// Writes a one-file `crates/` tree under `tmp` declaring `fn <name>() {}`,
    /// so `citation_resolves(tmp.path(), name)` finds it via `grep_exists`'s
    /// `fn {name}` needle.
    fn write_resolvable_fn(tmp: &TempDir, name: &str) {
        let dir = tmp.path().join("crates").join("fake").join("src");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lib.rs"), format!("fn {name}() {{}}\n")).unwrap();
    }

    /// A complete, valid Finalize Review body: one row per requirement, every
    /// cell in one of the three forms with non-empty text, no L1 `N/A`, one
    /// `Review Independence:` line, and an L1 test name (`probe_one_behavior`)
    /// that `write_resolvable_fn` makes resolvable.
    const VALID_REVIEW_BODY: &str = "### Finalize Review 2026-09-07\n\n\
        | Requirement | L1 | L2 | L3 | L4 |\n\
        | --- | --- | --- | --- | --- |\n\
        | R1 | PASS — `probe_one_behavior` proves it | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
        | R2 | PASS — see hunk | N/A — removal only | PASS — wired | N/A — no new trust boundary |\n\n\
        - FINDING-001 (LOW, blocking: no): cosmetic nit.\n\n\
        Review Independence: DEGRADED_SAME_RUNTIME\n";

    #[test]
    fn finalize_review_shape_passes_valid_fixture() {
        let tmp = TempDir::new().unwrap();
        write_resolvable_fn(&tmp, "probe_one_behavior");
        let prompt = write_prompt_with_review(&tmp, VALID_REVIEW_BODY);
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(out.state, CheckState::Pass, "summary: {}", out.summary);
    }

    #[test]
    fn finalize_review_shape_rejects_a_bare_na_cell() {
        let tmp = TempDir::new().unwrap();
        write_resolvable_fn(&tmp, "probe_one_behavior");
        let body = "### Finalize Review 2026-09-07\n\n\
            | Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — `probe_one_behavior` proves it | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
            | R2 | PASS — see hunk | N/A | PASS — wired | N/A — no new trust boundary |\n\n\
            Review Independence: DEGRADED_SAME_RUNTIME\n";
        let prompt = write_prompt_with_review(&tmp, body);
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(
            out.state,
            CheckState::Fail,
            "finalize-review-shape rejects a bare N/A cell"
        );
    }

    #[test]
    fn finalize_review_shape_rejects_a_bare_pass_cell() {
        let tmp = TempDir::new().unwrap();
        write_resolvable_fn(&tmp, "probe_one_behavior");
        let body = "### Finalize Review 2026-09-07\n\n\
            | Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — `probe_one_behavior` proves it | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
            | R2 | PASS — see hunk | N/A — removal only | PASS | N/A — no new trust boundary |\n\n\
            Review Independence: DEGRADED_SAME_RUNTIME\n";
        let prompt = write_prompt_with_review(&tmp, body);
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(
            out.state,
            CheckState::Fail,
            "finalize-review-shape rejects a bare PASS cell"
        );
    }

    #[test]
    fn finalize_review_shape_rejects_l1_na_with_reason() {
        let tmp = TempDir::new().unwrap();
        write_resolvable_fn(&tmp, "probe_one_behavior");
        let body = "### Finalize Review 2026-09-07\n\n\
            | Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — `probe_one_behavior` proves it | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
            | R2 | N/A — no observable behaviour | N/A — removal only | PASS — wired | N/A — no new trust boundary |\n\n\
            Review Independence: DEGRADED_SAME_RUNTIME\n";
        let prompt = write_prompt_with_review(&tmp, body);
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(
            out.summary.contains("R2"),
            "fail summary names the offending row: {}",
            out.summary
        );
    }

    #[test]
    fn finalize_review_shape_rejects_an_all_na_row() {
        let tmp = TempDir::new().unwrap();
        write_resolvable_fn(&tmp, "probe_one_behavior");
        let body = "### Finalize Review 2026-09-07\n\n\
            | Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — `probe_one_behavior` proves it | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
            | R2 | N/A — no observable behaviour | N/A — removal only | N/A — nothing wired | N/A — no new trust boundary |\n\n\
            Review Independence: DEGRADED_SAME_RUNTIME\n";
        let prompt = write_prompt_with_review(&tmp, body);
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(
            out.summary.contains("R2"),
            "fail summary names the offending row: {}",
            out.summary
        );
    }

    #[test]
    fn finalize_review_shape_rejects_a_row_count_short_by_one() {
        let tmp = TempDir::new().unwrap();
        write_resolvable_fn(&tmp, "probe_one_behavior");
        let body = "### Finalize Review 2026-09-07\n\n\
            | Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — `probe_one_behavior` proves it | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\n\
            Review Independence: DEGRADED_SAME_RUNTIME\n";
        let prompt = write_prompt_with_review(&tmp, body);
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(
            out.state,
            CheckState::Fail,
            "table has 1 row against 2 requirements"
        );
    }

    #[test]
    fn finalize_review_shape_rejects_two_finalize_review_headings() {
        let tmp = TempDir::new().unwrap();
        write_resolvable_fn(&tmp, "probe_one_behavior");
        let body = "### Finalize Review 2026-09-07\n\n\
            | Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — `probe_one_behavior` proves it | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
            | R2 | PASS — see hunk | N/A — removal only | PASS — wired | N/A — no new trust boundary |\n\n\
            Review Independence: DEGRADED_SAME_RUNTIME\n\n\
            ### Finalize Review 2026-09-08\n\n\
            duplicate heading, re-review\n";
        let prompt = write_prompt_with_review(&tmp, body);
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(
            out.state,
            CheckState::Fail,
            "two ### Finalize Review headings must fail"
        );
    }

    #[test]
    fn finalize_review_shape_rejects_a_missing_review_independence_line() {
        let tmp = TempDir::new().unwrap();
        write_resolvable_fn(&tmp, "probe_one_behavior");
        let body = "### Finalize Review 2026-09-07\n\n\
            | Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — `probe_one_behavior` proves it | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
            | R2 | PASS — see hunk | N/A — removal only | PASS — wired | N/A — no new trust boundary |\n";
        let prompt = write_prompt_with_review(&tmp, body);
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(
            out.state,
            CheckState::Fail,
            "missing Review Independence: line must fail"
        );
    }

    #[test]
    fn finalize_review_shape_rejects_an_unresolvable_l1_test_name() {
        let tmp = TempDir::new().unwrap();
        // Deliberately do not write any `crates/` tree entry for this name.
        let body = "### Finalize Review 2026-09-07\n\n\
            | Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — `nonexistent_probe_fn` proves it | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
            | R2 | PASS — see hunk | N/A — removal only | PASS — wired | N/A — no new trust boundary |\n\n\
            Review Independence: DEGRADED_SAME_RUNTIME\n";
        let prompt = write_prompt_with_review(&tmp, body);
        std::fs::create_dir_all(tmp.path().join("crates")).unwrap();
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(
            out.state,
            CheckState::Fail,
            "unresolvable L1 test name must fail"
        );
        assert!(
            out.summary.contains("nonexistent_probe_fn"),
            "fail summary names the unresolvable token: {}",
            out.summary
        );
    }

    #[test]
    fn finalize_review_shape_rejects_a_source_plan_target_with_no_sibling_prompt() {
        let tmp = TempDir::new().unwrap();
        let plans_dir = tmp.path().join(".dev").join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        let plan_path = plans_dir.join("demo.md");
        std::fs::write(&plan_path, "## Requirements\n\n- [ ] R1 — x\n").unwrap();
        // No sibling demo.prompt.md is written.
        let out = check_finalize_review_shape(&plan_path, tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(
            out.summary.contains("demo.prompt.md"),
            "fail summary names the missing sibling prompt path: {}",
            out.summary
        );
    }

    #[test]
    fn finalize_review_shape_passes_an_l1_path_token_without_resolving_it() {
        let tmp = TempDir::new().unwrap();
        // No `probe_one_behavior` fn is written; if the path token were
        // resolved as a test name this would incorrectly fail.
        let body = "### Finalize Review 2026-09-07\n\n\
            | Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — `crates/cli/src/x.rs:12` shows the hunk | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
            | R2 | PASS — see hunk | N/A — removal only | PASS — wired | N/A — no new trust boundary |\n\n\
            Review Independence: DEGRADED_SAME_RUNTIME\n";
        let prompt = write_prompt_with_review(&tmp, body);
        std::fs::create_dir_all(tmp.path().join("crates")).unwrap();
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(out.state, CheckState::Pass, "summary: {}", out.summary);
    }

    #[test]
    fn finalize_review_shape_passes_an_l1_prose_token_without_underscore() {
        let tmp = TempDir::new().unwrap();
        // No `holistic` fn is written; a prose token without `_` must never be
        // resolved as a test name, matching or missing.
        let body = "### Finalize Review 2026-09-07\n\n\
            | Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — the review is `holistic` here | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
            | R2 | PASS — see hunk | N/A — removal only | PASS — wired | N/A — no new trust boundary |\n\n\
            Review Independence: DEGRADED_SAME_RUNTIME\n";
        let prompt = write_prompt_with_review(&tmp, body);
        std::fs::create_dir_all(tmp.path().join("crates")).unwrap();
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(out.state, CheckState::Pass, "summary: {}", out.summary);
    }

    #[test]
    fn finalize_review_shape_skips_resolution_with_no_crates_tree() {
        let tmp = TempDir::new().unwrap();
        // tmp.path() has no `crates/` directory at all. `nonexistent_probe_fn`
        // would fail resolution if it ran, but resolution must be skipped.
        let body = "### Finalize Review 2026-09-07\n\n\
            | Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — `nonexistent_probe_fn` proves it | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
            | R2 | PASS — see hunk | N/A — removal only | PASS — wired | N/A — no new trust boundary |\n\n\
            Review Independence: DEGRADED_SAME_RUNTIME\n";
        let prompt = write_prompt_with_review(&tmp, body);
        let out = check_finalize_review_shape(&prompt, tmp.path());
        assert_eq!(out.state, CheckState::Pass, "summary: {}", out.summary);
        assert!(
            out.summary.contains("skipped: no crates/ tree"),
            "pass summary must note the skipped resolution: {}",
            out.summary
        );
    }

    // --- shared candidate-text validator ---

    /// Review fixtures the hygiene checker and the in-memory validator must judge alike.
    fn parity_fixtures() -> Vec<(&'static str, String, bool)> {
        let table = "| Requirement | L1 | L2 | L3 | L4 |\n\
            | --- | --- | --- | --- | --- |\n\
            | R1 | PASS — `probe_one_behavior` proves it | PASS — file exists | PASS — wired | N/A — no new trust boundary |\n\
            | R2 | PASS — see hunk | N/A — removal only | PASS — wired | N/A — no new trust boundary |\n\n";
        vec![
            (
                "valid",
                format!("### Finalize Review 2026-09-07\n\n{table}Review Independence: full\n"),
                true,
            ),
            (
                "manual L1 evidence without a citation",
                "### Finalize Review 2026-09-07\n\n\
                 | Requirement | L1 | L2 | L3 | L4 |\n\
                 | --- | --- | --- | --- | --- |\n\
                 | R1 | PASS — reviewed by hand against the diff | PASS — file exists | PASS — wired | N/A — none |\n\
                 | R2 | PASS — see hunk | N/A — removal only | PASS — wired | N/A — none |\n\n\
                 Review Independence: full\n"
                    .to_string(),
                true,
            ),
            (
                "bare PASS cell",
                "### Finalize Review 2026-09-07\n\n\
                 | Requirement | L1 | L2 | L3 | L4 |\n\
                 | --- | --- | --- | --- | --- |\n\
                 | R1 | PASS | PASS — file exists | PASS — wired | N/A — none |\n\
                 | R2 | PASS — see hunk | N/A — removal only | PASS — wired | N/A — none |\n\n\
                 Review Independence: full\n"
                    .to_string(),
                false,
            ),
            (
                "unresolvable citation",
                format!(
                    "### Finalize Review 2026-09-07\n\n{}Review Independence: full\n",
                    table.replace("probe_one_behavior", "missing_probe_name")
                ),
                false,
            ),
            (
                "duplicate independence line",
                format!(
                    "### Finalize Review 2026-09-07\n\n{table}Review Independence: full\nReview Independence: full\n"
                ),
                false,
            ),
        ]
    }

    fn prompt_text_with_review(review_body: &str) -> String {
        format!(
            "## Requirements\n\n\
             - [ ] R1 — first requirement\n\
             - [x] R2 — second requirement\n\n\
             ## Review Results\n\n\
             {review_body}\n"
        )
    }

    #[test]
    fn candidate_text_validator_and_hygiene_checker_judge_the_same_fixtures_alike() {
        for with_crates in [true, false] {
            for (label, body, expect_pass_with_crates) in parity_fixtures() {
                let tmp = TempDir::new().unwrap();
                if with_crates {
                    write_resolvable_fn(&tmp, "probe_one_behavior");
                }
                let text = prompt_text_with_review(&body);
                let prompt = write_prompt_with_review(&tmp, &body);
                let from_file = check_finalize_review_shape(&prompt, tmp.path());
                let from_text =
                    evaluate_finalize_review_text(&text, tmp.path(), ReviewIndependenceRule::Any);
                assert_eq!(
                    (from_file.state.clone(), from_file.summary.clone()),
                    (from_text.state.clone(), from_text.summary.clone()),
                    "{label} (crates tree: {with_crates}): file and text paths must agree"
                );
                // Without a crates tree citations are not resolved, so only the
                // unresolvable-citation fixture changes its verdict.
                let expected = if label == "unresolvable citation" && !with_crates {
                    true
                } else {
                    expect_pass_with_crates
                };
                assert_eq!(
                    from_text.state == CheckState::Pass,
                    expected,
                    "{label} (crates tree: {with_crates}): {}",
                    from_text.summary
                );
            }
        }
    }

    #[test]
    fn dispatched_independence_rule_requires_the_literal_full() {
        let tmp = TempDir::new().unwrap();
        write_resolvable_fn(&tmp, "probe_one_behavior");
        let make = |value: &str| {
            prompt_text_with_review(&format!(
                "### Finalize Review 2026-09-07\n\n\
                 | Requirement | L1 | L2 | L3 | L4 |\n\
                 | --- | --- | --- | --- | --- |\n\
                 | R1 | PASS — `probe_one_behavior` proves it | PASS — ok | PASS — ok | N/A — none |\n\
                 | R2 | PASS — see hunk | N/A — none | PASS — ok | N/A — none |\n\n\
                 Review Independence: {value}\n"
            ))
        };
        let strict = |text: &str| {
            evaluate_finalize_review_text(text, tmp.path(), ReviewIndependenceRule::MustBeFull)
        };
        assert_eq!(strict(&make("full")).state, CheckState::Pass);
        for wrong in ["DEGRADED_SAME_RUNTIME", "full-ish", "FULL", ""] {
            let out = strict(&make(wrong));
            assert_eq!(out.state, CheckState::Fail, "value {wrong:?} must fail");
            assert!(out.summary.contains("full"), "{}", out.summary);
        }
        // The hygiene rule still accepts the in-process degraded value.
        let degraded = evaluate_finalize_review_text(
            &make("DEGRADED_SAME_RUNTIME"),
            tmp.path(),
            ReviewIndependenceRule::Any,
        );
        assert_eq!(degraded.state, CheckState::Pass, "{}", degraded.summary);
    }

    #[test]
    fn authoritative_review_command_joins_declared_commands_and_rejects_unusable_ones() {
        let fence =
            |json: &str| format!("intro\n<!-- gal:authoritative-check -->\n```json\n{json}\n```\n");
        assert_eq!(
            authoritative_review_command(&fence(
                r#"{"command":["cargo test","cargo fmt --check"]}"#
            ))
            .unwrap(),
            "cargo test && cargo fmt --check"
        );
        assert!(authoritative_review_command("no fence here").is_err());
        assert!(authoritative_review_command(&fence(r#"{"command":[]}"#)).is_err());
        assert!(authoritative_review_command(&fence(r#"{"command":["  "]}"#)).is_err());
        assert!(authoritative_review_command(&fence(r#"{"command":["echo `x`"]}"#)).is_err());
        assert!(authoritative_review_command(&fence(r#"{"command":["a\nb"]}"#)).is_err());
    }

    // --- hygiene-only row inventory ---

    /// Holds the binary-wide `ENV_GUARD` for as long as the process CWD is
    /// switched, and restores the original CWD on drop — including when an
    /// assertion fails partway through — before releasing the lock.
    ///
    /// `cargo test` runs one binary's tests as threads inside a single
    /// process, and `cmd_finalize_check` resolves its repo root from the
    /// process CWD. Without the shared lock, a CWD-mutating test in another
    /// module restores its own directory mid-run, so this test's receipt lands
    /// outside its `TempDir` and every row is computed against the wrong root.
    struct CwdGuard {
        original: PathBuf,
        _lock: std::sync::MutexGuard<'static, ()>,
    }
    impl CwdGuard {
        fn enter(dir: &Path) -> Self {
            // A panicking test poisons the lock; the CWD invariant is restored
            // by that test's own Drop, so the poison carries no state to honour.
            let lock = crate::commands::ENV_GUARD
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(dir).unwrap();
            Self {
                original,
                _lock: lock,
            }
        }
    }
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    #[test]
    fn hygiene_only_row_inventory_adds_finalize_review_shape() {
        let tmp = TempDir::new().unwrap();
        let plans_dir = tmp.path().join(".dev").join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        std::fs::write(
            plans_dir.join("demo.prompt.md"),
            "## Requirements\n\n- [ ] R1 — x\n\n## Review Results\n\nNot started.\n",
        )
        .unwrap();

        let _guard = CwdGuard::enter(tmp.path());

        let args = vec![
            "finalize-check".to_string(),
            ".dev/plans/demo.prompt.md".to_string(),
            "--hygiene-only".to_string(),
            "--receipt".to_string(),
            "out/finalize.hygiene.receipt.md".to_string(),
        ];
        let _ = cmd_finalize_check(&args);

        let receipt_text =
            std::fs::read_to_string(tmp.path().join("out/finalize.hygiene.receipt.md")).unwrap();
        let names: Vec<String> = receipt_text
            .lines()
            .filter(|l| {
                l.starts_with("| ") && !l.starts_with("| check ") && !l.starts_with("| --- ")
            })
            .map(|l| split_md_row(l)[0].clone())
            .collect();

        assert_eq!(
            names,
            vec![
                "project-source-doc-existence".to_string(),
                "state-bound".to_string(),
                "finalize-review-shape".to_string(),
                "durable-layer-commit".to_string(),
                "working-tree-clean".to_string(),
            ],
            "hygiene-only row inventory must be today's rows plus finalize-review-shape and nothing else, in receipt text:\n{receipt_text}"
        );
    }

    // --- full-mode row inventory ---

    /// Minimal fixture: a canonical execution prompt plus the `.dev/project.md`
    /// and `.dev/state.md` full-mode row emission reads. `with_gal_core`
    /// controls whether `plugins/gal-core/` exists, which gates the
    /// gal-source-repo-scoped rows (`contract-roster-parity`,
    /// `doc-link-resolution`).
    fn write_full_mode_fixture(tmp: &TempDir, with_gal_core: bool) {
        let plans_dir = tmp.path().join(".dev").join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        std::fs::write(
            plans_dir.join("demo.prompt.md"),
            "## Requirements\n\n- [ ] R1 — x\n\n## Review Results\n\nNot started.\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join(".dev").join("project.md"), "# Project\n").unwrap();
        std::fs::write(tmp.path().join(".dev").join("state.md"), "# State\n").unwrap();
        if with_gal_core {
            std::fs::create_dir_all(tmp.path().join("plugins").join("gal-core")).unwrap();
        }
    }

    /// Runs `cmd_finalize_check` in full mode against `tmp` (as CWD) and
    /// returns the receipt's ordered `name` column.
    fn full_mode_row_names(tmp: &TempDir) -> Vec<String> {
        let _guard = CwdGuard::enter(tmp.path());

        let args = vec![
            "finalize-check".to_string(),
            ".dev/plans/demo.prompt.md".to_string(),
            "--receipt".to_string(),
            "out/finalize.receipt.md".to_string(),
        ];
        let _ = cmd_finalize_check(&args);

        let receipt_text =
            std::fs::read_to_string(tmp.path().join("out/finalize.receipt.md")).unwrap();
        receipt_text
            .lines()
            .filter(|l| {
                l.starts_with("| ") && !l.starts_with("| check ") && !l.starts_with("| --- ")
            })
            .map(|l| split_md_row(l)[0].clone())
            .collect()
    }

    /// Locks the full-mode row inventory to the repo-level rows only (R2):
    /// with `plugins/gal-core/` present, the nine rows from
    /// `check_authoritative` (`authoritative-command`, `naming-gate`),
    /// `check_sync_idempotency`, `check_finalize_mode`, and the four
    /// repo-hygiene rows plus `working-tree-clean`; without it, the same list
    /// minus the two gal-source-repo-scoped rows. Full mode carries no
    /// per-task rows — those checks were orphaned dead code and were removed.
    #[test]
    fn full_mode_row_inventory_matches_repo_shape() {
        let with_core = TempDir::new().unwrap();
        write_full_mode_fixture(&with_core, true);
        let names_with_core = full_mode_row_names(&with_core);
        assert_eq!(
            names_with_core,
            vec![
                "authoritative-command".to_string(),
                "naming-gate".to_string(),
                "sync-idempotency".to_string(),
                "finalize-mode".to_string(),
                "project-source-doc-existence".to_string(),
                "state-bound".to_string(),
                "contract-roster-parity".to_string(),
                "doc-link-resolution".to_string(),
                "working-tree-clean".to_string(),
            ],
            "full mode emits exactly the repo-level rows"
        );

        let without_core = TempDir::new().unwrap();
        write_full_mode_fixture(&without_core, false);
        let names_without_core = full_mode_row_names(&without_core);
        assert_eq!(
            names_without_core,
            vec![
                "authoritative-command".to_string(),
                "naming-gate".to_string(),
                "sync-idempotency".to_string(),
                "finalize-mode".to_string(),
                "project-source-doc-existence".to_string(),
                "state-bound".to_string(),
                "working-tree-clean".to_string(),
            ],
            "full mode emits exactly the repo-level rows"
        );
    }
}
