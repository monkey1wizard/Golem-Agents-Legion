//! Dispatch-family commands: dispatch / pipeline / dispatch-script.

use gal_engine::ExitCode;
use std::path::{Path, PathBuf};

/// Classification of a `gal pipeline <path>` argument. Pure, no I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PipelineInput {
    /// An execution prompt (`*.prompt.md`) — read its body, materialize the spec, dispatch.
    Prompt,
    /// A source plan (`.dev/plans/<slug>.md`) — switch to the paired execution prompt.
    SourcePlan,
    /// An already-materialized task spec (or any other/unknown input) — fed as-is.
    RawSpec,
}

/// Classify a `gal pipeline` path argument by its conventional location/suffix.
/// Deterministic and pure: `*.prompt.md` → Prompt; a `.dev/plans/…*.md` → SourcePlan;
/// a `.dev/generated/task-specs/…` path or any other/unknown input → RawSpec (the
/// back-compatible default, so a bare materialized spec keeps working unchanged).
pub(crate) fn resolve_pipeline_input(path: &Path) -> PipelineInput {
    let s = path.to_string_lossy().replace('\\', "/");
    if s.ends_with(".prompt.md") {
        PipelineInput::Prompt
    } else if s.ends_with(".en.md") || s.ends_with(".equiv.md") {
        // Planning-language EN draft / retired equivalence-receipt file — never a
        // source plan, even though it lives in .dev/plans/ and ends in .md.
        // `.equiv.md` has no writer (proof lives inline in the source plan's
        // planning-authority block); kept as a defensive guard for a legacy or
        // downstream orphan file.
        PipelineInput::RawSpec
    } else if s.contains(".dev/plans/") && s.ends_with(".md") {
        PipelineInput::SourcePlan
    } else {
        PipelineInput::RawSpec
    }
}

pub(crate) fn cmd_dispatch(args: &[String]) -> ExitCode {
    // In-process via the dispatch library — self-contained, no separate
    // `gal-dispatch` executable (which is not in the end-user release artifact).
    // The task spec is read from stdin, matching the gal-dispatch contract.
    let flags: Vec<String> = args.iter().skip(1).cloned().collect();
    if flags.iter().any(|a| a == "--help" || a == "-h") {
        println!("gal dispatch --phase <implement|test|audit> --task <T-NNN> [--workdir <p>] [--timeout <s>] [--routing <p>] [--receipt <p>]");
        println!("(pipe the task spec to stdin)");
        return ExitCode::Success;
    }
    let args = match dispatch::cli::parse_args(&flags) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("gal dispatch: {e}");
            return ExitCode::Usage;
        }
    };
    let mut spec = String::new();
    use std::io::Read as _;
    if let Err(e) = std::io::stdin().read_to_string(&mut spec) {
        eprintln!("gal dispatch: failed to read spec from stdin: {e}");
        return ExitCode::Usage;
    }
    let outcome = dispatch::run::run_dispatch(&args, &spec);
    match outcome.exit_code {
        0 => ExitCode::Success,
        1 => ExitCode::Error,
        2 => ExitCode::NotWired,
        _ => ExitCode::Error,
    }
}

pub(crate) fn cmd_pipeline(args: &[String]) -> ExitCode {
    if args.len() < 2 {
        eprintln!("gal pipeline: missing plan / prompt / task-spec path");
        eprintln!("usage: gal pipeline <PromptPath|SourcePlanPath|TaskSpecPath> [--phase <phase>] [--task <T-NN>] [--receipt <path>]");
        return ExitCode::Usage;
    }

    let raw_input = PathBuf::from(&args[1]);

    let mut phase = String::from("implement");
    let mut task_override: Option<String> = None;
    let mut receipt_path: Option<String> = None;
    let mut workdir_flag: Option<String> = None;

    let mut index = 2usize;
    while index < args.len() {
        match args[index].as_str() {
            "--phase" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("gal pipeline: --phase requires a value");
                    return ExitCode::Usage;
                };
                phase = value.clone();
            }
            "--task" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("gal pipeline: --task requires a value");
                    return ExitCode::Usage;
                };
                task_override = Some(value.clone());
            }
            "--receipt" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("gal pipeline: --receipt requires a value");
                    return ExitCode::Usage;
                };
                receipt_path = Some(value.clone());
            }
            "--workdir" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("gal pipeline: --workdir requires a value");
                    return ExitCode::Usage;
                };
                workdir_flag = Some(value.clone());
            }
            other => {
                eprintln!("gal pipeline: unknown argument '{other}'");
                return ExitCode::Usage;
            }
        }
        index += 1;
    }

    // Working directory the dispatch operates in: the explicit --workdir (as emitted by
    // the OFFLOAD ACTION), else the current dir. A relative input path resolves against it
    // so the orchestrator can run `gal pipeline` from anywhere.
    let workdir = workdir_flag
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let input_path = if raw_input.is_absolute() {
        raw_input
    } else {
        workdir.join(&raw_input)
    };

    // Resolve the input kind, producing the dispatch spec markdown + the task id.
    // Prompt / SourcePlan inputs are materialized to a per-(task,phase) spec internally;
    // a RawSpec (or any other input) is fed as-is for back-compat. `plan_log_source` carries
    // the prompt/source-plan path used to derive the plan-scoped log directory (R1) — `None`
    // for RawSpec, so raw/direct dispatch keeps its default (unscoped) log location.
    let (spec, task_id, effective_receipt, plan_log_source) = match resolve_pipeline_input(
        &input_path,
    ) {
        PipelineInput::Prompt => {
            if !input_path.exists() {
                eprintln!("gal pipeline: prompt not found: {}", input_path.display());
                return ExitCode::Error;
            }
            match build_spec_from_prompt(
                &input_path,
                &phase,
                task_override.as_deref(),
                receipt_path.as_deref(),
            ) {
                Ok((spec, task_id, receipt)) => (spec, task_id, receipt, Some(input_path.clone())),
                Err(e) => {
                    eprintln!("gal pipeline: {e}");
                    return ExitCode::Error;
                }
            }
        }
        PipelineInput::SourcePlan => {
            // Never dispatch against a source plan directly — switch to its paired
            // execution prompt, or hard-error pointing at the planning commands.
            let Some(prompt) = paired_prompt_path(&input_path) else {
                eprintln!(
                    "gal pipeline: cannot derive a prompt path from '{}'",
                    input_path.display()
                );
                return ExitCode::Error;
            };
            if !prompt.exists() {
                eprintln!(
                    "gal pipeline: source plan '{}' has no paired execution prompt at '{}'.",
                    input_path.display(),
                    prompt.display()
                );
                eprintln!(
                    "  run /refining-plan to lock the contract, then /plan-to-prompt to generate the execution prompt, then re-run gal pipeline against the prompt."
                );
                return ExitCode::Error;
            }
            match build_spec_from_prompt(
                &prompt,
                &phase,
                task_override.as_deref(),
                receipt_path.as_deref(),
            ) {
                Ok((spec, task_id, receipt)) => (spec, task_id, receipt, Some(prompt)),
                Err(e) => {
                    eprintln!("gal pipeline: {e}");
                    return ExitCode::Error;
                }
            }
        }
        PipelineInput::RawSpec => {
            if !input_path.exists() {
                eprintln!(
                    "gal pipeline: task spec not found: {}",
                    input_path.display()
                );
                return ExitCode::Error;
            }
            let spec = match std::fs::read_to_string(&input_path) {
                Ok(value) => value,
                Err(e) => {
                    eprintln!(
                        "gal pipeline: failed to read task spec '{}': {e}",
                        input_path.display()
                    );
                    return ExitCode::Error;
                }
            };
            let task_id = task_override.unwrap_or_else(|| {
                input_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|stem| stem.split('-').take(2).collect::<Vec<_>>().join("-"))
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "T-unknown".to_string())
            });
            let receipt = effective_receipt_path(&phase, &task_id, receipt_path.as_deref());
            (spec, task_id, receipt, None)
        }
    };

    // Agent-contract self-sufficiency guard: a spec's `## Agent Contract`
    // section points the executor at an external contract file. Copilot's
    // `--no-custom-instructions` slimming removes the generated rules context, so
    // the spec + that referenced contract are the whole contract. Verify the
    // referenced path exists and is readable under the workdir before dispatch —
    // never dispatch a spec chasing a dangling contract reference.
    if let Err(msg) = verify_agent_contract(&spec, &workdir) {
        eprintln!("gal pipeline: {msg}");
        return ExitCode::Error;
    }

    // Run the dispatch in-process via the dispatch library — the single `gal`
    // binary is self-contained and does not depend on a separate `gal-dispatch`
    // executable (which is not in the end-user release artifact). The safety
    // gate lives in `dispatch::run::run_dispatch`.
    let mut raw_args = vec![
        "--phase".to_string(),
        phase,
        "--task".to_string(),
        task_id,
        "--workdir".to_string(),
        workdir.display().to_string(),
    ];
    if let Some(receipt) = effective_receipt {
        // Resolve a relative receipt against workdir so verification matches where the
        // executor (which runs with cwd = workdir) writes it, regardless of gal's own cwd.
        let receipt_resolved = {
            let p = PathBuf::from(&receipt);
            if p.is_absolute() {
                p
            } else {
                workdir.join(&p)
            }
        };
        raw_args.push("--receipt".to_string());
        raw_args.push(receipt_resolved.display().to_string());
    }

    let mut args = match dispatch::cli::parse_args(&raw_args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("gal pipeline: {e}");
            return ExitCode::Usage;
        }
    };

    // R1: a Prompt/SourcePlan pipeline input derives a plan slug and reuses the
    // existing (programmatic-only) `DispatchArgs.log_dir_override` to scope executor
    // logs under that plan. Raw/direct dispatch (`plan_log_source` is `None`) keeps
    // the default unscoped log location unchanged. No new flag.
    args.log_dir_override = resolve_log_dir_override(&workdir, plan_log_source.as_deref());

    let outcome = dispatch::run::run_dispatch(&args, &spec);

    // Loop-log the dispatch outcome: one structured event per terminal
    // state, written under the workdir's gitignored loop-log dir. This lives in
    // cli (not dispatch) because the crate DAG forbids `dispatch -> pipeline`.
    if let Some(state) = &outcome.terminal_state {
        let state_str = state.as_str();
        let (kind, level) = pipeline::loop_log::classify_dispatch(state_str);
        let now = gal_foundation::time::now_timestamp();
        let run = now.get(0..10).unwrap_or("run").to_string();
        let event = pipeline::loop_log::LoopEvent {
            time: now.clone(),
            task: args.task.clone(),
            phase: args.phase.as_str().to_string(),
            role: args.phase.role().to_string(),
            kind: kind.to_string(),
            level,
            msg: format!("dispatch {state_str} (exit {})", outcome.exit_code),
            log_ptr: outcome.log_path.as_ref().map(|p| p.display().to_string()),
        };
        let dir = workdir.join(".dev").join("pipeline").join("loop-log");
        let _ = pipeline::loop_log::append_event(&dir, &run, &event); // best-effort forensics
    }

    match outcome.exit_code {
        0 => ExitCode::Success,
        1 => ExitCode::Error,
        2 => ExitCode::NotWired,
        _ => ExitCode::Error,
    }
}

/// `gal pipeline-log append --task <t> --phase <p> --kind <k> --level <l> --msg <m>`
/// — pipeline-internal: append one structured loop-log event. This is the
/// in-process append path the orchestrator uses for `gal pipeline` degraded
/// phases (the Rust dispatch path appends directly via `cmd_pipeline`). Writes to
/// `<cwd>/.dev/pipeline/loop-log/<date>.ndjson`. Not a public `/gal` command.
pub fn cmd_pipeline_log(args: &[String]) -> ExitCode {
    // args[0] is the "pipeline-log" subcommand token; the action follows.
    if args.get(1).map(String::as_str) != Some("append") {
        eprintln!("gal pipeline-log: usage: gal pipeline-log append --task <t> --phase <p> --kind <k> --level <l> --msg <m>");
        return ExitCode::Usage;
    }
    let mut task = String::new();
    let mut phase = String::new();
    let mut role = String::new();
    let mut kind = String::new();
    let mut level = String::from("error");
    let mut msg = String::new();
    let mut log_ptr: Option<String> = None;
    let mut i = 2usize;
    while i < args.len() {
        let val = args.get(i + 1).cloned();
        match args[i].as_str() {
            "--task" => task = val.unwrap_or_default(),
            "--phase" => phase = val.unwrap_or_default(),
            "--role" => role = val.unwrap_or_default(),
            "--kind" => kind = val.unwrap_or_default(),
            "--level" => level = val.unwrap_or_default(),
            "--msg" => msg = val.unwrap_or_default(),
            "--log-ptr" => log_ptr = val,
            other => {
                eprintln!("gal pipeline-log: unknown argument '{other}'");
                return ExitCode::Usage;
            }
        }
        i += 2;
    }
    let level = match level.to_ascii_lowercase().as_str() {
        "info" => pipeline::loop_log::LoopLevel::Info,
        "warning" | "warn" => pipeline::loop_log::LoopLevel::Warning,
        _ => pipeline::loop_log::LoopLevel::Error,
    };
    let now = gal_foundation::time::now_timestamp();
    let run = now.get(0..10).unwrap_or("run").to_string();
    let event = pipeline::loop_log::LoopEvent {
        time: now,
        task,
        phase,
        role,
        kind,
        level,
        msg,
        log_ptr,
    };
    let dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".dev")
        .join("pipeline")
        .join("loop-log");
    match pipeline::loop_log::append_event(&dir, &run, &event) {
        Ok(_) => ExitCode::Success,
        Err(e) => {
            eprintln!("gal pipeline-log: append failed: {e}");
            ExitCode::Error
        }
    }
}

/// Derive the paired execution-prompt path for a `.dev/plans/<slug>.md` source plan:
/// strip `.md` suffix and append `.prompt.md`. Pure string transform so it works for
/// relative or absolute inputs and on both path separators.
fn paired_prompt_path(source_plan: &Path) -> Option<PathBuf> {
    let s = source_plan.to_string_lossy().replace('\\', "/");
    let stem = s.strip_suffix(".md")?;
    Some(PathBuf::from(format!("{stem}.prompt.md")))
}

/// Derive the plan slug (R1) from a Prompt/SourcePlan path: the filename with a
/// trailing `.prompt.md` or `.md` suffix stripped. Pure string transform, no I/O.
/// Returns `None` for a filename that has no usable stem (defensive; the caller
/// then leaves `log_dir_override` unset and dispatch falls back to the default
/// unscoped log location).
fn plan_slug_from_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let stem = name
        .strip_suffix(".prompt.md")
        .or_else(|| name.strip_suffix(".md"))?;
    if stem.is_empty() {
        None
    } else {
        Some(stem.to_string())
    }
}

/// Sanitize a plan slug into a filesystem-safe directory component: alphanumerics,
/// `.`, `_`, `-` pass through; everything else becomes `-`. Mirrors the executor-log
/// filename sanitizer in `dispatch::dispatch` so a plan-scoped log directory can never
/// escape `.dev/executor-logs/` via path separators or other unsafe characters.
fn sanitize_plan_slug(slug: &str) -> String {
    let sanitized: String = slug
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '_' | '-' => c,
            _ => '-',
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    // A slug that is entirely dots (e.g. "." or "..") is a directory-traversal
    // component once joined onto the log-dir path — refuse it rather than let it
    // escape the intended `.dev/executor-logs/<slug>/` confinement.
    if trimmed.is_empty() || trimmed.chars().all(|c| c == '.') {
        "unknown-plan".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Resolve the R1 plan-scoped log-dir override for `gal pipeline`: `Some(dir)` when
/// the pipeline input was a Prompt/SourcePlan (derives a plan slug and scopes logs
/// under `<workdir>/.dev/executor-logs/<slug>/`); `None` for raw/direct dispatch
/// (`plan_log_source` is `None`), which leaves `DispatchArgs.log_dir_override` unset
/// so `dispatch::run` falls back to `SpawnConfig::default_log_dir` — raw/direct
/// dispatch location/interface stays unchanged.
///
/// `pub(crate)` so `converge_check`'s v2 receipt can resolve the SAME plan-scoped
/// directory this module scopes dispatch logs under, without duplicating the
/// slug-derivation/sanitization logic in a second place that could drift.
pub(crate) fn resolve_log_dir_override(
    workdir: &Path,
    plan_log_source: Option<&Path>,
) -> Option<PathBuf> {
    let source_path = plan_log_source?;
    let slug = plan_slug_from_path(source_path)?;
    Some(
        workdir
            .join(".dev")
            .join("executor-logs")
            .join(sanitize_plan_slug(&slug)),
    )
}

/// Read an execution prompt and assemble the per-(task,phase) task spec, scoped to the
/// single task id (explicit `--task` wins, else the first `T-NN` task bullet in the
/// prompt). Returns `(spec_markdown, task_id)`. The spec is scoped to one task — the
/// goal block for `task_id` only, not the whole prompt.
fn build_spec_from_prompt(
    prompt_path: &Path,
    phase: &str,
    task_override: Option<&str>,
    explicit_receipt: Option<&str>,
) -> Result<(String, String, Option<String>), String> {
    let prompt_body = std::fs::read_to_string(prompt_path)
        .map_err(|e| format!("failed to read prompt '{}': {e}", prompt_path.display()))?;
    let task_id = match task_override {
        Some(t) => t.to_string(),
        None => first_task_id(&prompt_body).ok_or_else(|| {
            format!(
                "no --task given and no task bullet found in '{}'",
                prompt_path.display()
            )
        })?,
    };
    let prompt_path_disp = prompt_path.to_string_lossy().replace('\\', "/");
    let now = gal_foundation::time::now_timestamp();
    let branch = git_current_branch();
    let head = git_head_short();
    // Resolve the effective receipt (explicit override, else the deterministic
    // default for test/audit) and embed it into the spec so the executor is told
    // to write the verifiable receipt the pipeline checks.
    let receipt = effective_receipt_path(phase, &task_id, explicit_receipt);
    let input = pipeline::task_spec::TaskSpecInput {
        task_scope: &task_id,
        phase,
        prompt_path: &prompt_path_disp,
        prompt_body: &prompt_body,
        generated: &now,
        git_branch: &branch,
        git_head: &head,
        convention_hints: None,
        receipt_path: receipt.as_deref(),
    };
    let spec = pipeline::task_spec::assemble_task_spec(&input)
        .map_err(|e| format!("task-spec assembly failed for {task_id}: {e}"))?;
    Ok((spec.markdown, task_id, receipt))
}

/// Resolve the effective receipt path for a (phase, task): an explicit `--receipt`
/// always wins; otherwise test/audit get a deterministic per-(task,phase) default
/// (`.dev/pipeline/receipts/<task>-<phase>.receipt.md`, workdir-relative) so the
/// executor writes a verifiable receipt and `verify_receipt` confirms the phase ran.
/// implement (and any other phase) defaults to no receipt — the orchestrator verifies
/// those via the code-file write-back instead.
fn effective_receipt_path(phase: &str, task_id: &str, explicit: Option<&str>) -> Option<String> {
    if let Some(e) = explicit {
        return Some(e.to_string());
    }
    match phase.to_lowercase().as_str() {
        "test" | "audit" => Some(format!(
            ".dev/pipeline/receipts/{task_id}-{phase}.receipt.md"
        )),
        _ => None,
    }
}

/// Extract the agent-contract path a spec's `## Agent Contract` section references.
/// The section is emitted by `pipeline::task_spec` as
/// `Follow the instructions in: `<path>``. Returns the backtick-quoted path, or
/// `None` when the spec carries no such reference (e.g. a raw spec).
fn extract_agent_contract_ref(spec: &str) -> Option<String> {
    for line in spec.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Follow the instructions in:") {
            let rest = rest.trim();
            // Path is wrapped in backticks: `<path>`.
            let inner = rest.strip_prefix('`').unwrap_or(rest);
            let inner = inner.strip_suffix('`').unwrap_or(inner);
            let path = inner.trim();
            if !path.is_empty() {
                return Some(path.to_string());
            }
        }
    }
    None
}

/// Verify the spec's `## Agent Contract` referenced file exists and is readable
/// under `workdir` before dispatch. `Ok(())` when there is no reference (raw spec)
/// or the referenced file reads successfully; `Err(message)` when a referenced
/// contract is missing or unreadable — the caller fails loud and does not dispatch.
fn verify_agent_contract(spec: &str, workdir: &Path) -> Result<(), String> {
    let Some(rel) = extract_agent_contract_ref(spec) else {
        return Ok(());
    };
    let contract_path = {
        let p = PathBuf::from(&rel);
        if p.is_absolute() {
            p
        } else {
            workdir.join(&p)
        }
    };
    std::fs::read_to_string(&contract_path).map(|_| ()).map_err(|e| {
        format!(
            "the spec's `## Agent Contract` references '{}', but it does not exist or is not readable ({e}); refusing to dispatch a spec pointing at a missing contract",
            contract_path.display()
        )
    })
}

/// First `T-<digits>` task id from a `- [ ]` / `- [x]` checklist bullet in the prompt.
fn first_task_id(prompt_body: &str) -> Option<String> {
    for line in prompt_body.lines() {
        let t = line.trim_start();
        if t.starts_with("- [ ]") || t.starts_with("- [x]") || t.starts_with("- [X]") {
            if let Some(id) = find_task_token(t) {
                return Some(id);
            }
        }
    }
    None
}

/// Scan for a `T-<digits>` token (the task-id format), composed at runtime — never a
/// literal in source. Returns the first match.
fn find_task_token(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b'T' && bytes[i + 1] == b'-' && bytes[i + 2].is_ascii_digit() {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            return Some(s[i..j].to_string());
        }
        i += 1;
    }
    None
}

fn git_current_branch() -> String {
    git_capture(&["rev-parse", "--abbrev-ref", "HEAD"])
}

fn git_head_short() -> String {
    git_capture(&["rev-parse", "--short", "HEAD"])
}

/// Capture trimmed stdout of a `git` invocation, falling back to `"unknown"` (the spec's
/// git fields are informational; `run_dispatch` re-derives the authoritative values for
/// the executor log).
fn git_capture(args: &[&str]) -> String {
    std::process::Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// The stderr warning lines `cmd_dispatch_script` emits before the dispatch block.
///
/// Routing diagnostics (retired flat-shape entries, wrong-group / unknown roles,
/// malformed entries) were previously swallowed here — `build_dispatch_block` decides
/// OFFLOAD but never surfaced `routing.warnings`, so a retired flat config degraded
/// silently in-process. This seam makes the emission testable: `cmd_dispatch_script`
/// reads the fixed default config path (not in-process-injectable), so the visibility
/// contract is proven by feeding a `RoutingTable` parsed from a temp config here.
fn dispatch_script_warning_lines(routing: &dispatch::routing::RoutingTable) -> Vec<String> {
    routing
        .warnings
        .iter()
        .map(|w| format!("warning: {w}"))
        .collect()
}

/// `gal dispatch-script <intent> [tokens…]` — the chat-control-plane dispatch block
/// emitter ported from the `scripts/gal.ps1`/`gal.sh` `dispatch` branch
/// (refactor-gal-dispatch-script-rust-port). Reads repo state and executor routing,
/// routes to the right block via `crate::dispatch_script::build_dispatch_block`,
/// and prints it. Always exits 0; the block itself carries any `COMMAND: error`.
pub(crate) fn cmd_dispatch_script(args: &[String]) -> ExitCode {
    let intent = args.get(1).map(String::as_str).unwrap_or("");
    let tokens: Vec<String> = args.iter().skip(2).cloned().collect();

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let repo_root = crate::dispatch_script::get_repo_context_root(&cwd);
    let repo_display = crate::dispatch_script::forward_slash(&repo_root);
    let state = crate::dispatch_script::read_state_context(&repo_root);
    let routing = dispatch::routing::load_routing_default();

    // Surface routing diagnostics to the user before the dispatch block — a retired
    // flat config must warn by name, never degrade silently.
    for line in dispatch_script_warning_lines(&routing) {
        eprintln!("{line}");
    }

    let block = crate::dispatch_script::build_dispatch_block(
        intent,
        &tokens,
        &routing,
        &state,
        &repo_display,
    );
    println!("{}", block.render());
    ExitCode::Success
}

#[cfg(test)]
mod tests {
    use super::*;

    // Task ids are composed at runtime, never written as digit literals in source
    // (a stale `T-<digits>` literal is just dead data; the naming gate enforces this).
    fn tid(n: u32) -> String {
        format!("T-{:03}", n)
    }

    // ── dispatch-script surfaces routing diagnostics (never swallowed) ──

    #[test]
    fn dispatch_script_surfaces_retirement_warning_for_flat_config() {
        // A retired flat top-level role → load_routing emits a retirement warning →
        // the exact line dispatch-script prints to stderr names the key + is visible.
        let tmp = tempfile::TempDir::new().unwrap();
        let flat = tmp.path().join("flat.json");
        std::fs::write(
            &flat,
            r#"{"executorRouting":{"CODER":{"executor":"claude","model":"m"}}}"#,
        )
        .unwrap();
        let routing = dispatch::routing::load_routing(&flat);
        let lines = dispatch_script_warning_lines(&routing);
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("warning:") && l.contains("retired") && l.contains("CODER")),
            "flat config must surface a retirement warning line: {lines:?}"
        );
    }

    #[test]
    fn dispatch_script_no_warning_for_migrated_nested_config() {
        let tmp = tempfile::TempDir::new().unwrap();
        let nested = tmp.path().join("nested.json");
        std::fs::write(
            &nested,
            r#"{"executorRouting":{"pipeline":{"CODER":{"executor":"claude","model":"m"}}}}"#,
        )
        .unwrap();
        let routing = dispatch::routing::load_routing(&nested);
        let lines = dispatch_script_warning_lines(&routing);
        assert!(
            lines.is_empty(),
            "a correctly-migrated nested config must produce no warning lines: {lines:?}"
        );
    }

    #[test]
    fn build_spec_from_prompt_scopes_to_single_task() {
        let tmp = tempfile::TempDir::new().unwrap();
        let prompt = tmp.path().join("x.prompt.md");
        let body = format!(
            "# Plan Prompt\n\n## Tasks\n\n- [ ] {t1} — First task touches `crates/a.rs`.\n- [ ] {t2} — Second task touches `crates/b.rs`.\n\n## Status\n",
            t1 = tid(1),
            t2 = tid(2),
        );
        std::fs::write(&prompt, &body).unwrap();

        // Explicit --task scopes the materialized spec to that one task's goal only.
        let (spec, task_id, _r) =
            build_spec_from_prompt(&prompt, "implement", Some(&tid(2)), None).unwrap();
        assert_eq!(task_id, tid(2));
        assert!(
            spec.contains("Second task"),
            "spec must carry the scoped task goal:\n{spec}"
        );
        assert!(
            !spec.contains("First task"),
            "spec must NOT leak other tasks (single-task scoping):\n{spec}"
        );

        // No --task → auto-detect the first task bullet.
        let (_spec2, auto, _r2) = build_spec_from_prompt(&prompt, "implement", None, None).unwrap();
        assert_eq!(auto, tid(1));
    }

    #[test]
    fn effective_receipt_defaults_for_test_and_audit_only() {
        let t = tid(8);
        // test/audit get a deterministic default receipt.
        assert_eq!(
            effective_receipt_path("test", &t, None),
            Some(format!(".dev/pipeline/receipts/{t}-test.receipt.md"))
        );
        assert_eq!(
            effective_receipt_path("audit", &t, None),
            Some(format!(".dev/pipeline/receipts/{t}-audit.receipt.md"))
        );
        // implement (and other phases) default to no receipt.
        assert_eq!(effective_receipt_path("implement", &t, None), None);
        // explicit --receipt always wins.
        assert_eq!(
            effective_receipt_path("test", &t, Some("custom/r.md")),
            Some("custom/r.md".to_string())
        );
    }

    #[test]
    fn build_spec_from_prompt_embeds_default_receipt_for_test() {
        let tmp = tempfile::TempDir::new().unwrap();
        let prompt = tmp.path().join("x.prompt.md");
        let body = format!(
            "## Tasks\n\n- [ ] {t} — Touches `crates/a.rs`.\n",
            t = tid(8)
        );
        std::fs::write(&prompt, &body).unwrap();
        let (spec, task_id, receipt) =
            build_spec_from_prompt(&prompt, "test", Some(&tid(8)), None).unwrap();
        let expected = format!(".dev/pipeline/receipts/{}-test.receipt.md", tid(8));
        assert_eq!(receipt.as_deref(), Some(expected.as_str()));
        assert!(
            spec.contains(&expected),
            "spec must instruct writing the receipt:\n{spec}"
        );
        assert_eq!(task_id, tid(8));
    }

    #[test]
    fn paired_prompt_path_appends_prompt_suffix() {
        assert_eq!(
            paired_prompt_path(Path::new(".dev/plans/foo.md")).unwrap(),
            PathBuf::from(".dev/plans/foo.prompt.md")
        );
        assert_eq!(
            paired_prompt_path(Path::new("C:/repo/.dev/plans/bar.md")).unwrap(),
            PathBuf::from("C:/repo/.dev/plans/bar.prompt.md")
        );
    }

    // ── Plan-slug derivation for the R1 log-dir override ────────────────────

    #[test]
    fn plan_slug_from_path_strips_prompt_suffix() {
        assert_eq!(
            plan_slug_from_path(Path::new(".dev/plans/fix-foo.prompt.md")).as_deref(),
            Some("fix-foo")
        );
        assert_eq!(
            plan_slug_from_path(Path::new("C:/repo/.dev/plans/feat-bar.prompt.md")).as_deref(),
            Some("feat-bar")
        );
    }

    #[test]
    fn plan_slug_from_path_strips_plain_md_suffix_for_source_plan() {
        assert_eq!(
            plan_slug_from_path(Path::new(".dev/plans/fix-foo.md")).as_deref(),
            Some("fix-foo")
        );
    }

    #[test]
    fn plan_slug_from_path_none_for_dotfile_or_no_stem() {
        // A bare ".md" (empty stem after stripping) yields None — defensive guard.
        assert_eq!(plan_slug_from_path(Path::new(".dev/plans/.md")), None);
    }

    #[test]
    fn sanitize_plan_slug_passes_through_safe_characters() {
        assert_eq!(
            sanitize_plan_slug("fix-executor-log-scan-inprocess-mode"),
            "fix-executor-log-scan-inprocess-mode"
        );
    }

    #[test]
    fn sanitize_plan_slug_replaces_unsafe_characters_and_trims_dashes() {
        // `.` passes through (mirrors dispatch::dispatch's sanitizer); `/` becomes `-`.
        assert_eq!(sanitize_plan_slug("../../etc/passwd"), "..-..-etc-passwd");
        assert_eq!(sanitize_plan_slug("weird slug!"), "weird-slug");
    }

    #[test]
    fn sanitize_plan_slug_falls_back_when_fully_stripped() {
        assert_eq!(sanitize_plan_slug("///"), "unknown-plan");
        assert_eq!(sanitize_plan_slug(""), "unknown-plan");
    }

    #[test]
    fn sanitize_plan_slug_rejects_dot_only_traversal_components() {
        // A slug that reduces to "." or ".." must not survive sanitization — once
        // joined onto the log-dir path it would resolve to the current or parent
        // directory, escaping the intended `.dev/executor-logs/<slug>/` confinement.
        assert_eq!(sanitize_plan_slug(".."), "unknown-plan");
        assert_eq!(sanitize_plan_slug("."), "unknown-plan");
        assert_eq!(sanitize_plan_slug("..."), "unknown-plan");
    }

    #[test]
    fn resolve_log_dir_override_scopes_under_plan_slug_for_prompt_or_source_plan() {
        let workdir = PathBuf::from("/repo");
        let prompt = Path::new(".dev/plans/fix-foo.prompt.md");
        let source_plan = Path::new(".dev/plans/fix-foo.md");
        let expected = PathBuf::from("/repo/.dev/executor-logs/fix-foo");
        assert_eq!(
            resolve_log_dir_override(&workdir, Some(prompt)),
            Some(expected.clone())
        );
        assert_eq!(
            resolve_log_dir_override(&workdir, Some(source_plan)),
            Some(expected)
        );
    }

    #[test]
    fn resolve_log_dir_override_none_for_raw_direct_dispatch() {
        // No plan_log_source (RawSpec / raw-direct dispatch) → None → the caller
        // leaves DispatchArgs.log_dir_override unset → default unscoped location.
        let workdir = PathBuf::from("/repo");
        assert_eq!(resolve_log_dir_override(&workdir, None), None);
    }

    #[test]
    fn build_spec_from_prompt_plan_slug_round_trip_matches_prompt_stem() {
        // Prove the two-step derivation used by cmd_pipeline (input_path/prompt →
        // plan_slug_from_path → sanitize_plan_slug) is stable and collision-free
        // across the Prompt vs SourcePlan input shapes for the same plan.
        let prompt_path = Path::new(".dev/plans/fix-executor-log-scan-inprocess-mode.prompt.md");
        let source_plan_path = Path::new(".dev/plans/fix-executor-log-scan-inprocess-mode.md");
        let slug_from_prompt = plan_slug_from_path(prompt_path).unwrap();
        let slug_from_source = plan_slug_from_path(source_plan_path).unwrap();
        assert_eq!(slug_from_prompt, slug_from_source);
        assert_eq!(
            sanitize_plan_slug(&slug_from_prompt),
            "fix-executor-log-scan-inprocess-mode"
        );
    }

    #[test]
    fn resolve_pipeline_input_classifies_by_location_and_suffix() {
        // Execution prompt → Prompt (prompt-preferred, regardless of directory).
        assert_eq!(
            resolve_pipeline_input(Path::new("a.prompt.md")),
            PipelineInput::Prompt
        );
        assert_eq!(
            resolve_pipeline_input(Path::new(".dev/plans/x.prompt.md")),
            PipelineInput::Prompt
        );
        // Source plan under .dev/plans → SourcePlan (both path separators).
        assert_eq!(
            resolve_pipeline_input(Path::new(".dev/plans/foo.md")),
            PipelineInput::SourcePlan
        );
        assert_eq!(
            resolve_pipeline_input(Path::new(".dev\\plans\\foo.md")),
            PipelineInput::SourcePlan
        );
        // Materialized spec → RawSpec (fed as-is, back-compat).
        assert_eq!(
            resolve_pipeline_input(Path::new(".dev/generated/task-specs/T-NNN-implement.md")),
            PipelineInput::RawSpec
        );
        // Planning-language EN draft / equivalence receipt → RawSpec, never SourcePlan,
        // even though both live in .dev/plans/ and end in .md.
        assert_eq!(
            resolve_pipeline_input(Path::new(".dev/plans/foo.en.md")),
            PipelineInput::RawSpec
        );
        assert_eq!(
            resolve_pipeline_input(Path::new(".dev/plans/foo.equiv.md")),
            PipelineInput::RawSpec
        );
        // Any other/unknown .md defaults to RawSpec (deterministic fallback).
        assert_eq!(
            resolve_pipeline_input(Path::new(".dev/research/x.md")),
            PipelineInput::RawSpec
        );
    }

    #[test]
    fn extract_agent_contract_ref_parses_backtick_path() {
        let spec = "## Agent Contract\n\nFollow the instructions in: `plugins/gal-core/agents/golem-auditor.agent.md`\n\n## Next\n";
        assert_eq!(
            extract_agent_contract_ref(spec).as_deref(),
            Some("plugins/gal-core/agents/golem-auditor.agent.md")
        );
        // No reference → None (a raw spec passes the guard unconditionally).
        assert!(extract_agent_contract_ref("## Goal\n\ndo the thing\n").is_none());
    }

    #[test]
    fn verify_agent_contract_fails_on_missing_passes_on_readable() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Referenced contract does not exist → Err (fail loud, no dispatch).
        let spec_missing =
            "## Agent Contract\n\nFollow the instructions in: `agents/does-not-exist.md`\n";
        assert!(verify_agent_contract(spec_missing, tmp.path()).is_err());

        // Create the contract → readable → Ok (proceeds).
        let rel = "agents/golem-auditor.agent.md";
        let contract = tmp.path().join(rel);
        std::fs::create_dir_all(contract.parent().unwrap()).unwrap();
        std::fs::write(&contract, "be a good auditor").unwrap();
        let spec_ok = format!("## Agent Contract\n\nFollow the instructions in: `{rel}`\n");
        assert!(verify_agent_contract(&spec_ok, tmp.path()).is_ok());

        // A spec with no Agent Contract reference always passes.
        assert!(verify_agent_contract("## Goal\n\nx\n", tmp.path()).is_ok());
    }

    /// Cross-module visibility — prove pub(crate) exposes the 3 shared primitives
    /// from finalize_check to sibling command modules (compile + call test).
    #[test]
    fn finalize_check_primitives_are_pub_crate_visible() {
        // Call all 3 primitives to prove pub(crate) cross-module accessibility.
        // Behavior is fully covered by finalize_check's own unit tests; here we
        // only need the compiler + linker to accept these calls from a sibling module.
        let _ = crate::commands::finalize_check::commit_note_hash("not a task line");
        let _ = crate::commands::finalize_check::checked_task_ids("empty fixture");
        let _ = crate::commands::finalize_check::check_three_surface(
            std::path::Path::new("not-a-prompt.txt"),
            "",
            std::path::Path::new("."),
        );
    }
}
