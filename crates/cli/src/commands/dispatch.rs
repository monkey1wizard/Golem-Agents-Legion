//! Dispatch-family commands: dispatch / pipeline / dispatch-script.

use dispatch::dispatch::ContractProvenance;
use gal_engine::ExitCode;
use std::path::{Path, PathBuf};

/// A phase agent contract resolved from the first available authoritative source
/// tier: the contract bytes (read once), the control-node absolute path they were
/// read from, and the tier label (`workdir`/`ancestor`/`exe-side`/`embedded`).
/// The bytes are inlined into the task spec so one spec serves local and SSH; the
/// path + tier become the dispatch provenance recorded in markers and logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedContract {
    pub body: String,
    pub path: PathBuf,
    pub source: String,
}

/// One ordered contract-source candidate: the tier label and the resolved source
/// root, or `None` when that tier produced no root (unavailable). Highest priority
/// first. Passed to [`resolve_contract_from_candidates`], the pure tier-selection +
/// read + error layer, so it is unit-testable without touching the real resolvers
/// or `current_exe()`.
#[derive(Debug, Clone)]
pub(crate) struct ContractCandidate {
    pub tier: &'static str,
    pub root: Option<PathBuf>,
}

/// Failure resolving an authoritative contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContractResolveError {
    /// A higher-priority root was recognized, but its phase contract is missing,
    /// non-UTF-8, or unreadable. R2: fail loud, never fall through to a lower tier.
    Corrupt {
        tier: &'static str,
        path: PathBuf,
        cause: String,
    },
    /// No source root was available in any tier. R5: list every tier's outcome and
    /// both recovery routes.
    AllMiss { tiers: Vec<(&'static str, String)> },
}

impl ContractResolveError {
    /// Render the guided, operator-facing error message (prefixed by the caller
    /// with `gal pipeline: `). Both variants keep exit code 1.
    pub(crate) fn message(&self) -> String {
        match self {
            ContractResolveError::Corrupt { tier, path, cause } => format!(
                "authoritative agent-contract source (tier `{tier}`) is corrupt: cannot read '{}' ({cause}). \
                 Refusing to silently fall back to a lower-priority source — the winning source root must be repaired. \
                 Fix that source, or reinstall `gal` via its packaging channel, or run from a GAL checkout.",
                path.display()
            ),
            ContractResolveError::AllMiss { tiers } => {
                let mut lines = String::from(
                    "no GAL agent-contract source root is available. Tiers tried (highest priority first):\n",
                );
                for (tier, outcome) in tiers {
                    lines.push_str(&format!("  - {tier}: {outcome}\n"));
                }
                lines.push_str(
                    "Recover by either reinstalling `gal` through its packaging channel \
                     (so the embedded source materializes), or running from a GAL source checkout \
                     (so the workdir/ancestor tier resolves).",
                );
                lines
            }
        }
    }
}

/// Pure tier selection + contract read. Walks `candidates` highest-priority first;
/// the first candidate with a resolved root wins. Reads that root's
/// `agents/<golem-*.agent.md>` (the basename from [`pipeline::task_spec::agent_contract_rel`]).
/// A winning root whose contract is missing/non-UTF-8/unreadable is a hard
/// [`ContractResolveError::Corrupt`] — never a fall-through (R2). All candidates
/// unavailable → [`ContractResolveError::AllMiss`] (R5). No filesystem walking or
/// GAL source-layout knowledge lives here — only in the production wrapper.
pub(crate) fn resolve_contract_from_candidates(
    candidates: &[ContractCandidate],
    phase: &str,
) -> Result<ResolvedContract, ContractResolveError> {
    let rel = pipeline::task_spec::agent_contract_rel(phase);
    let contract_file = Path::new(rel)
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(rel));
    let mut tier_outcomes: Vec<(&'static str, String)> = Vec::new();
    for cand in candidates {
        match &cand.root {
            None => tier_outcomes.push((cand.tier, "unavailable (no source root)".to_string())),
            Some(root) => {
                let contract_path = root.join("agents").join(&contract_file);
                match std::fs::read_to_string(&contract_path) {
                    Ok(body) => {
                        return Ok(ResolvedContract {
                            body,
                            path: contract_path,
                            source: cand.tier.to_string(),
                        });
                    }
                    Err(e) => {
                        return Err(ContractResolveError::Corrupt {
                            tier: cand.tier,
                            path: contract_path,
                            cause: e.to_string(),
                        });
                    }
                }
            }
        }
    }
    Err(ContractResolveError::AllMiss {
        tiers: tier_outcomes,
    })
}

/// Build the ordered **higher-priority** (`workdir`/`ancestor`/`exe-side`) contract-source
/// candidates for a `(workdir, current-exe)` pair, classifying the cwd hit as `workdir`
/// (the root is the workdir itself or its direct `plugins/gal-core`) or `ancestor` (a
/// higher level). Canonicalizes both inputs (R1) so a relative `--workdir` and a
/// symlinked exe resolve to absolute roots. Pure given its inputs; the production
/// caller supplies the real `current_exe()`.
///
/// Deliberately excludes the `embedded` tier: materializing it is I/O (and, under
/// concurrent `gal` processes, a shared-state write) that must happen only when every
/// higher tier has already missed — see [`resolve_authoritative_contract`], which
/// appends the `embedded` candidate lazily, exactly once, only on that fallback path.
pub(crate) fn build_contract_candidates(
    workdir: &Path,
    current_exe: &Path,
) -> Vec<ContractCandidate> {
    use gal_engine::render::{resolve_source_from_cwd, resolve_source_from_exe_dir};
    let workdir_abs = std::fs::canonicalize(workdir).unwrap_or_else(|_| workdir.to_path_buf());

    // Tiers 1+2 share one resolver (`resolve_source_from_cwd` returns a single root):
    // classify its hit as `workdir` (the workdir itself or its direct `plugins/gal-core`)
    // or `ancestor` (a higher level), and leave the other of the two `None`. Emitting
    // both slots (rather than collapsing to one) means an all-miss error can list every
    // tier (R5) even though the cwd walk produces at most one root.
    let (mut workdir_root, mut ancestor_root) = (None, None);
    if let Some(root) = resolve_source_from_cwd(&workdir_abs) {
        let direct_gal_core = workdir_abs.join("plugins").join("gal-core");
        if root == workdir_abs || root == direct_gal_core {
            workdir_root = Some(root);
        } else {
            ancestor_root = Some(root);
        }
    }

    // Tier 3: exe-side (canonicalize the running executable first, R1).
    let exe_abs = std::fs::canonicalize(current_exe).unwrap_or_else(|_| current_exe.to_path_buf());
    let exe_root = exe_abs.parent().and_then(resolve_source_from_exe_dir);

    vec![
        ContractCandidate {
            tier: "workdir",
            root: workdir_root,
        },
        ContractCandidate {
            tier: "ancestor",
            root: ancestor_root,
        },
        ContractCandidate {
            tier: "exe-side",
            root: exe_root,
        },
    ]
}

/// Resolve and read the authoritative phase contract for a prompt/source-plan
/// pipeline run: build the higher-priority tier candidates from `workdir` + the
/// running executable, try them first, and only materialize + append the `embedded`
/// candidate — exactly once — when every higher tier missed. A `workdir`/`ancestor`/
/// `exe-side` hit therefore never calls `materialize_embedded_source()` at all; a
/// `Corrupt` result from the higher-priority attempt is returned immediately (R2:
/// never fall through a recognized-but-broken root), preserving the same all-miss
/// tier diagnostics (R5) as before, now computed lazily.
pub(crate) fn resolve_authoritative_contract(
    workdir: &Path,
    phase: &str,
) -> Result<ResolvedContract, ContractResolveError> {
    let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("gal"));
    let mut candidates = build_contract_candidates(workdir, &current_exe);
    match resolve_contract_from_candidates(&candidates, phase) {
        Err(ContractResolveError::AllMiss { .. }) => {
            use gal_engine::render::materialize_embedded_source;
            candidates.push(ContractCandidate {
                tier: "embedded",
                root: materialize_embedded_source(),
            });
            resolve_contract_from_candidates(&candidates, phase)
        }
        other => other,
    }
}

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
        println!("gal dispatch --phase <implement|test|audit> --task <T-NN> [--workdir <p>] [--timeout <s>] [--routing <p>] [--receipt <p>]");
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
    // For prompt/source-plan inputs, resolve and read the authoritative phase
    // contract before spec assembly, inline its bytes into the spec (so one spec
    // serves local + SSH — no control-node path is an executor read dependency), and
    // carry its path+tier as dispatch provenance. A corrupt winning source or a
    // total miss fails loud here, before any spawn. Raw/direct specs neither resolve
    // nor invent provenance (R8) — `provenance` stays `None`.
    let (spec, task_id, effective_receipt, plan_log_source, provenance) =
        match resolve_pipeline_input(&input_path) {
            PipelineInput::Prompt => {
                if !input_path.exists() {
                    eprintln!("gal pipeline: prompt not found: {}", input_path.display());
                    return ExitCode::Error;
                }
                let contract = match resolve_authoritative_contract(&workdir, &phase) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("gal pipeline: {}", e.message());
                        return ExitCode::Error;
                    }
                };
                match build_spec_from_prompt(
                    &input_path,
                    &phase,
                    task_override.as_deref(),
                    receipt_path.as_deref(),
                    Some(&contract.body),
                ) {
                    Ok((spec, task_id, receipt)) => (
                        spec,
                        task_id,
                        receipt,
                        Some(input_path.clone()),
                        Some(ContractProvenance {
                            path: contract.path,
                            source: contract.source,
                        }),
                    ),
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
                let contract = match resolve_authoritative_contract(&workdir, &phase) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("gal pipeline: {}", e.message());
                        return ExitCode::Error;
                    }
                };
                match build_spec_from_prompt(
                    &prompt,
                    &phase,
                    task_override.as_deref(),
                    receipt_path.as_deref(),
                    Some(&contract.body),
                ) {
                    Ok((spec, task_id, receipt)) => (
                        spec,
                        task_id,
                        receipt,
                        Some(prompt),
                        Some(ContractProvenance {
                            path: contract.path,
                            source: contract.source,
                        }),
                    ),
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
                (spec, task_id, receipt, None, None)
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

    // R4: carry the resolved contract provenance (path + tier) into dispatch so
    // markers and executor-log headers record which source served the contract.
    // `None` for raw/direct dispatch keeps its byte-compatible marker/log shape (R8).
    args.contract_provenance = provenance;

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
    agent_contract_body: Option<&str>,
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
        agent_contract_body,
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
            build_spec_from_prompt(&prompt, "implement", Some(&tid(2)), None, None).unwrap();
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
        let (_spec2, auto, _r2) =
            build_spec_from_prompt(&prompt, "implement", None, None, None).unwrap();
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
            build_spec_from_prompt(&prompt, "test", Some(&tid(8)), None, None).unwrap();
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
            resolve_pipeline_input(Path::new(".dev/generated/task-specs/T-NN-implement.md")),
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

    // ── authoritative contract resolution / read / inline ──────────────

    /// Create a source root under `dir` (the three marker dirs) and write the
    /// phase contract for `phase` with `body`. Returns `dir`.
    fn write_source_root(dir: &Path, phase: &str, body: &str) {
        for sub in ["skills", "agents", "commands"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        let rel = pipeline::task_spec::agent_contract_rel(phase);
        let file = Path::new(rel).file_name().unwrap();
        std::fs::write(dir.join("agents").join(file), body).unwrap();
    }

    fn cand(tier: &'static str, root: Option<&Path>) -> ContractCandidate {
        ContractCandidate {
            tier,
            root: root.map(|p| p.to_path_buf()),
        }
    }

    #[test]
    fn resolve_contract_reads_first_available_tier_bytes_exactly() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("root");
        let body = "# Golem Implementer\n\nExact bytes, do not truncate.\n";
        write_source_root(&root, "implement", body);
        let cands = vec![cand("workdir", Some(&root))];
        let got = resolve_contract_from_candidates(&cands, "implement").unwrap();
        assert_eq!(got.source, "workdir");
        assert_eq!(got.body, body, "contract bytes must be read verbatim");
        assert_eq!(
            got.path,
            root.join("agents").join("golem-implementer.agent.md")
        );
    }

    #[test]
    fn resolve_contract_precedence_workdir_over_all_lower_tiers() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (w, a, e, b) = ("w", "a", "e", "b");
        for (name, marker) in [("w", w), ("a", a), ("e", e), ("b", b)] {
            write_source_root(&tmp.path().join(name), "implement", marker);
        }
        let cands = vec![
            cand("workdir", Some(&tmp.path().join("w"))),
            cand("ancestor", Some(&tmp.path().join("a"))),
            cand("exe-side", Some(&tmp.path().join("e"))),
            cand("embedded", Some(&tmp.path().join("b"))),
        ];
        let got = resolve_contract_from_candidates(&cands, "implement").unwrap();
        assert_eq!(got.source, "workdir");
        assert_eq!(got.body, w);
    }

    #[test]
    fn resolve_contract_precedence_ancestor_then_exe_then_embedded() {
        let tmp = tempfile::TempDir::new().unwrap();
        for name in ["a", "e", "b"] {
            write_source_root(&tmp.path().join(name), "implement", name);
        }
        // ancestor beats exe + embedded
        let cands = vec![
            cand("workdir", None),
            cand("ancestor", Some(&tmp.path().join("a"))),
            cand("exe-side", Some(&tmp.path().join("e"))),
            cand("embedded", Some(&tmp.path().join("b"))),
        ];
        assert_eq!(
            resolve_contract_from_candidates(&cands, "implement")
                .unwrap()
                .source,
            "ancestor"
        );
        // exe beats embedded
        let cands = vec![
            cand("workdir", None),
            cand("ancestor", None),
            cand("exe-side", Some(&tmp.path().join("e"))),
            cand("embedded", Some(&tmp.path().join("b"))),
        ];
        assert_eq!(
            resolve_contract_from_candidates(&cands, "implement")
                .unwrap()
                .source,
            "exe-side"
        );
    }

    #[test]
    fn resolve_contract_corrupt_winning_root_never_falls_through() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Winning (workdir) root is recognized but MISSING its phase contract.
        let bad = tmp.path().join("bad");
        for sub in ["skills", "agents", "commands"] {
            std::fs::create_dir_all(bad.join(sub)).unwrap();
        }
        // A perfectly good lower-tier root that must NOT be used.
        let good = tmp.path().join("good");
        write_source_root(&good, "implement", "GOOD BODY");
        let cands = vec![cand("workdir", Some(&bad)), cand("embedded", Some(&good))];
        match resolve_contract_from_candidates(&cands, "implement") {
            Err(ContractResolveError::Corrupt { tier, .. }) => assert_eq!(tier, "workdir"),
            other => panic!("expected Corrupt on the winning root, got {other:?}"),
        }
    }

    #[test]
    fn resolve_contract_all_unavailable_lists_four_tiers_and_both_recoveries() {
        let cands = vec![
            cand("workdir", None),
            cand("ancestor", None),
            cand("exe-side", None),
            cand("embedded", None),
        ];
        let err = resolve_contract_from_candidates(&cands, "implement").unwrap_err();
        assert!(matches!(err, ContractResolveError::AllMiss { .. }));
        let msg = err.message();
        for tier in ["workdir", "ancestor", "exe-side", "embedded"] {
            assert!(
                msg.contains(tier),
                "all-miss must list tier `{tier}`: {msg}"
            );
        }
        assert!(
            msg.contains("reinstall"),
            "must name the reinstall route: {msg}"
        );
        assert!(
            msg.contains("checkout"),
            "must name the checkout route: {msg}"
        );
    }

    #[test]
    fn resolve_contract_phase_selects_the_right_contract_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("root");
        write_source_root(&root, "audit", "AUDITOR BODY");
        let got =
            resolve_contract_from_candidates(&[cand("workdir", Some(&root))], "audit").unwrap();
        assert!(got.path.ends_with("golem-auditor.agent.md"));
        assert_eq!(got.body, "AUDITOR BODY");
    }

    #[test]
    fn build_candidates_classifies_flat_workdir_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_source_root(tmp.path(), "implement", "X");
        let fake_exe = tmp.path().join("nonexistent-gal.exe");
        let cands = build_contract_candidates(tmp.path(), &fake_exe);
        assert_eq!(cands[0].tier, "workdir");
        let want = std::fs::canonicalize(tmp.path()).unwrap();
        assert_eq!(cands[0].root.as_ref().unwrap(), &want);
        assert!(
            cands[1].root.is_none(),
            "ancestor slot empty when workdir wins"
        );
    }

    #[test]
    fn build_candidates_classifies_plugins_gal_core_as_workdir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gal_core = tmp.path().join("plugins").join("gal-core");
        write_source_root(&gal_core, "implement", "X");
        let fake_exe = tmp.path().join("nonexistent-gal.exe");
        let cands = build_contract_candidates(tmp.path(), &fake_exe);
        assert_eq!(cands[0].tier, "workdir");
        let want = std::fs::canonicalize(&gal_core).unwrap();
        assert_eq!(cands[0].root.as_ref().unwrap(), &want);
    }

    #[test]
    fn build_candidates_classifies_ancestor_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        // parent is a flat source root; workdir is a subdir with no root of its own.
        write_source_root(tmp.path(), "implement", "X");
        let sub = tmp.path().join("workdir-sub");
        std::fs::create_dir_all(&sub).unwrap();
        let fake_exe = tmp.path().join("nonexistent-gal.exe");
        let cands = build_contract_candidates(&sub, &fake_exe);
        assert!(cands[0].root.is_none(), "workdir slot empty");
        assert_eq!(cands[1].tier, "ancestor");
        let want = std::fs::canonicalize(tmp.path()).unwrap();
        assert_eq!(cands[1].root.as_ref().unwrap(), &want);
    }

    #[test]
    fn build_candidates_canonicalizes_relative_workdir_segments() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_source_root(tmp.path(), "implement", "X");
        let sub = tmp.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        // A non-canonical path with a `..` segment that resolves back to the root.
        let noncanon = sub.join("..");
        let fake_exe = tmp.path().join("nonexistent-gal.exe");
        let cands = build_contract_candidates(&noncanon, &fake_exe);
        assert_eq!(cands[0].tier, "workdir");
        let want = std::fs::canonicalize(tmp.path()).unwrap();
        assert_eq!(
            cands[0].root.as_ref().unwrap(),
            &want,
            "`..` segment must canonicalize before resolution"
        );
    }

    #[test]
    fn build_contract_candidates_excludes_embedded_tier() {
        // The eager tier-4 (embedded) computation must not happen at all in
        // build_contract_candidates — it is now lazy, appended only by
        // resolve_authoritative_contract on the all-higher-tier-miss path.
        let tmp = tempfile::TempDir::new().unwrap();
        let fake_exe = tmp.path().join("nonexistent-gal.exe");
        let cands = build_contract_candidates(tmp.path(), &fake_exe);
        assert_eq!(cands.len(), 3, "expected exactly workdir/ancestor/exe-side");
        assert!(
            cands.iter().all(|c| c.tier != "embedded"),
            "embedded tier must not be present in the eager candidate list"
        );
    }

    #[test]
    fn workdir_hit_never_reaches_embedded_fallback() {
        // A recognized workdir root must short-circuit before embedded
        // materialization is ever attempted — proven by never calling it:
        // build_contract_candidates (which resolve_authoritative_contract uses
        // for the higher-priority attempt) carries no embedded candidate at all.
        let tmp = tempfile::TempDir::new().unwrap();
        write_source_root(tmp.path(), "implement", "X");
        let got = resolve_authoritative_contract(tmp.path(), "implement").unwrap();
        assert_eq!(got.source, "workdir");
    }

    #[test]
    fn all_higher_tier_miss_falls_back_to_embedded_exactly_once() {
        // Shared process-global env guard — serializes every env-mutating test
        // in this binary (matches the pattern in commands::refresh's tests).
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        #[cfg(windows)]
        let prev = std::env::var_os("USERPROFILE");
        #[cfg(not(windows))]
        let prev = std::env::var_os("HOME");
        #[cfg(windows)]
        std::env::set_var("USERPROFILE", tmp.path());
        #[cfg(not(windows))]
        std::env::set_var("HOME", tmp.path());

        // Neither workdir nor ancestor resolves to anything real; exe-side
        // resolves against the real test-binary path, which also has no
        // sibling gal-core source tree under target/debug.
        let bogus_workdir = tmp.path().join("no-such-workdir");
        let result = resolve_authoritative_contract(&bogus_workdir, "implement");

        #[cfg(windows)]
        match prev {
            Some(v) => std::env::set_var("USERPROFILE", v),
            None => std::env::remove_var("USERPROFILE"),
        }
        #[cfg(not(windows))]
        match prev {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }

        // This build's embedded payload is populated (a real git checkout, per
        // is_populated()'s own contract), so the lazy embedded tier succeeds —
        // proving the all-higher-tier-miss path reaches and uses it.
        if gal_engine::embedded::is_populated() {
            let got = result.expect("embedded tier should resolve when populated");
            assert_eq!(got.source, "embedded");
        } else {
            let err = result.expect_err("expected AllMiss when embedded is also unavailable");
            let msg = err.message();
            assert!(
                msg.contains("embedded"),
                "AllMiss diagnostics must still list the embedded tier, got: {msg}"
            );
        }
    }

    #[test]
    fn partial_root_missing_agents_dir_is_not_recognized() {
        // A `plugins/gal-core` with skills/ + commands/ but NO agents/ must not be
        // recognized as a source root; resolution continues past it (R2 fail-loud
        // fires only after a root is recognized). Pins the boundary from cli's side.
        let tmp = tempfile::TempDir::new().unwrap();
        let gal_core = tmp.path().join("plugins").join("gal-core");
        std::fs::create_dir_all(gal_core.join("skills")).unwrap();
        std::fs::create_dir_all(gal_core.join("commands")).unwrap();
        // deliberately no agents/
        let resolved = gal_engine::render::resolve_source_from_cwd(tmp.path());
        assert_ne!(
            resolved.as_deref(),
            Some(gal_core.as_path()),
            "a gal-core missing agents/ must not be selected as a source root"
        );
    }

    #[test]
    fn resolve_against_real_repo_returns_workdir_tier_with_exact_bytes() {
        // R7: run inside the GAL checkout → workdir tier wins and the delivered
        // bytes equal the on-disk phase contract exactly. Uses the real repo files
        // (CARGO_MANIFEST_DIR is crates/cli → repo root is two levels up).
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let contract_file = repo_root
            .join("plugins")
            .join("gal-core")
            .join("agents")
            .join("golem-implementer.agent.md");
        if !contract_file.exists() {
            return; // not in a full checkout (e.g. packaged-only) — skip
        }
        let got = resolve_authoritative_contract(&repo_root, "implement").unwrap();
        assert_eq!(got.source, "workdir");
        let on_disk = std::fs::read_to_string(&contract_file).unwrap();
        assert_eq!(got.body, on_disk, "workdir tier must deliver exact bytes");
    }

    #[test]
    fn build_spec_inlines_contract_and_drops_control_node_reference() {
        // The generated spec is self-contained (transport-independent): the contract
        // bytes appear inline and no `Follow the instructions in:` control-node path
        // remains, so `extract_agent_contract_ref` is None and the same spec serves
        // local + SSH. Covers exact-inline-bytes + SSH parity at the wiring level.
        let tmp = tempfile::TempDir::new().unwrap();
        let prompt = tmp.path().join("x.prompt.md");
        std::fs::write(
            &prompt,
            format!("## Tasks\n\n- [ ] {t} — Touch `crates/a.rs`.\n", t = tid(1)),
        )
        .unwrap();
        let body = "# Golem Implementer\n\nWhole contract inline.\n";
        let (spec, _task, _r) =
            build_spec_from_prompt(&prompt, "implement", Some(&tid(1)), None, Some(body)).unwrap();
        assert!(
            spec.contains("Whole contract inline."),
            "body must be inlined"
        );
        assert!(
            !spec.contains("Follow the instructions in:"),
            "no control-node contract path may remain in a generated spec"
        );
        assert!(
            extract_agent_contract_ref(&spec).is_none(),
            "self-contained spec has no external contract reference"
        );
        // verify_agent_contract is a no-op for the self-contained spec (transport-safe).
        assert!(verify_agent_contract(&spec, tmp.path()).is_ok());
    }
}
