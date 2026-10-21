//! Dispatch-family commands: dispatch / pipeline / dispatch-script.

use dispatch::dispatch::{AttemptContext, ContractProvenance, DispatchQuota, PromptWriterLease};
use gal_engine::ExitCode;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[cfg(test)]
thread_local! {
    static PIPELINE_AFTER_DISPATCH_TEST_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

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
    /// Embedded source materialization failed after all higher-priority tiers
    /// missed. This is distinct from an absent embedded payload.
    Materialization { path: PathBuf, cause: String },
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
            ContractResolveError::Materialization { path, cause } => format!(
                "embedded agent-contract source materialization failed at '{}' ({cause}). \
                 Fix the GAL home or reinstall `gal` via its packaging channel.",
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
#[cfg(test)]
pub(crate) fn resolve_contract_from_candidates(
    candidates: &[ContractCandidate],
    phase: &str,
) -> Result<ResolvedContract, ContractResolveError> {
    resolve_source_file_from_candidates(candidates, &agent_contract_source_rel(phase))
}

/// Path of a phase's agent contract inside a source root: `agents/<basename>`.
fn agent_contract_source_rel(phase: &str) -> PathBuf {
    let rel = pipeline::task_spec::agent_contract_rel(phase);
    let contract_file = Path::new(rel)
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(rel));
    Path::new("agents").join(contract_file)
}

/// The canonical finalize command body inside a source root. It is the command
/// template, never an agent contract, so a missing file never falls back to one.
const FINALIZE_COMMAND_REL: &str = "commands/gal-finalize/SKILL.template.md";

/// Tier selection + read for one file at `rel` under the winning source root. This
/// is the shared core of [`resolve_contract_from_candidates`] and the finalize
/// command lookup, so both keep the same precedence and fail-loud corruption rules.
fn resolve_source_file_from_candidates(
    candidates: &[ContractCandidate],
    rel: &Path,
) -> Result<ResolvedContract, ContractResolveError> {
    let mut tier_outcomes: Vec<(&'static str, String)> = Vec::new();
    for cand in candidates {
        match &cand.root {
            None => tier_outcomes.push((cand.tier, "unavailable (no source root)".to_string())),
            Some(root) => {
                let contract_path = root.join(rel);
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
    resolve_authoritative_source_file(workdir, &agent_contract_source_rel(phase))
}

/// Resolve the canonical finalize command body by the same source precedence as
/// [`resolve_authoritative_contract`]: higher tiers first, a corrupt winning root is
/// an error, and the embedded tier is materialized only when every higher tier missed.
pub(crate) fn resolve_authoritative_finalize_command(
    workdir: &Path,
) -> Result<ResolvedContract, ContractResolveError> {
    resolve_authoritative_source_file(workdir, Path::new(FINALIZE_COMMAND_REL))
}

fn resolve_authoritative_source_file(
    workdir: &Path,
    rel: &Path,
) -> Result<ResolvedContract, ContractResolveError> {
    let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("gal"));
    let mut candidates = build_contract_candidates(workdir, &current_exe);
    match resolve_source_file_from_candidates(&candidates, rel) {
        Err(ContractResolveError::AllMiss { .. }) => {
            use gal_engine::render::materialize_embedded_source;
            let embedded_root = match materialize_embedded_source() {
                Ok(root) => root,
                Err(error) => {
                    return Err(ContractResolveError::Materialization {
                        path: gal_foundation::paths::gal_home()
                            .unwrap_or_else(|| PathBuf::from("~/.gal")),
                        cause: error.to_string(),
                    });
                }
            };
            candidates.push(ContractCandidate {
                tier: "embedded",
                root: embedded_root,
            });
            resolve_source_file_from_candidates(&candidates, rel)
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

/// A `.dev/research/<slug>.md` dispatch source (RawSpec-classified — see
/// `resolve_pipeline_input`) still needs the same plan-scoped receipt/executor-log
/// directory as a `.dev/plans/` Prompt/SourcePlan input, keyed off `<slug>` via the
/// existing `plan_slug_from_path` + `sanitize_plan_slug` derivation. Detected
/// separately from `resolve_pipeline_input` because RawSpec must keep dispatching the
/// file's raw bytes as-is (unlike Prompt/SourcePlan, which materialize a spec) — only
/// the scope-source path threaded through changes.
fn research_scope_source(path: &Path) -> bool {
    let s = path.to_string_lossy().replace('\\', "/");
    s.contains(".dev/research/") && s.ends_with(".md")
}

fn validate_fix_mode(
    fix_mode: bool,
    phase: &str,
    input_kind: PipelineInput,
) -> Result<(), &'static str> {
    if fix_mode && (phase != "implement" || input_kind == PipelineInput::RawSpec) {
        return Err("--fix requires a Prompt or SourcePlan input with --phase implement");
    }
    Ok(())
}

/// `gal pipeline --phase` accepts the three implementation-lane phases (`implement`,
/// `test`, `audit`) and the plan-level `finalize-review`. `investigate` is
/// deliberately excluded — research work has its own dedicated `gal research`
/// command, not a pipeline phase — so it (and any other unrecognized value) is
/// rejected here rather than falling through to a dispatch.
fn validate_pipeline_phase(phase: &str) -> Result<(), String> {
    match phase.to_lowercase().as_str() {
        "implement" | "test" | "audit" | FINALIZE_REVIEW_PHASE => Ok(()),
        "investigate" => Err(
            "phase 'investigate' is not a `gal pipeline` phase; use `gal research` instead"
                .to_string(),
        ),
        other => Err(format!(
            "unknown phase '{other}'; expected one of implement, test, audit, finalize-review \
             (for research, use `gal research`)"
        )),
    }
}

/// The plan-level review phase. It has no task, so its scope token and receipt
/// directory both use this fixed name and never a task id.
const FINALIZE_REVIEW_PHASE: &str = "finalize-review";

/// True when the raw argv selects the finalize-review phase. Used before argument
/// parsing so the guarded task-driver entry can be refused for this phase.
fn requests_finalize_review(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "--phase" && pair[1].eq_ignore_ascii_case(FINALIZE_REVIEW_PHASE))
}

pub(crate) fn cmd_dispatch(args: &[String]) -> ExitCode {
    // In-process via the dispatch library — self-contained, no separate
    // `gal-dispatch` executable (which is not in the end-user release artifact).
    // The task spec is read from stdin, matching the gal-dispatch contract.
    let flags: Vec<String> = args.iter().skip(1).cloned().collect();
    if flags.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "gal dispatch --phase <implement|test|audit|investigate> --task <T-NN> [--workdir <p>] [--timeout <s>] [--routing <p>] [--receipt <p>]"
        );
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
    if !matches!(
        args.phase.as_str(),
        "implement" | "test" | "audit" | "investigate"
    ) {
        eprintln!(
            "gal dispatch: unknown phase '{}'; expected one of implement, test, audit",
            args.phase.as_str()
        );
        return ExitCode::Usage;
    }
    let mut spec = String::new();
    use std::io::Read as _;
    if let Err(e) = std::io::stdin().read_to_string(&mut spec) {
        eprintln!("gal dispatch: failed to read spec from stdin: {e}");
        return ExitCode::Usage;
    }
    let attempt_token = new_attempt_token();
    let attempt_context = AttemptContext {
        attempt_token: Some(attempt_token.clone()),
        ..AttemptContext::default()
    };
    let outcome = dispatch::run::run_dispatch_with_context(&args, &spec, Some(&attempt_context));
    capture_attempt_usage(
        &args.workdir,
        None,
        &attempt_token,
        outcome.log_path.as_deref(),
        Some(&outcome.quota),
    );
    match outcome.exit_code {
        0 => ExitCode::Success,
        1 => ExitCode::Error,
        2 => ExitCode::NotWired,
        _ => ExitCode::Error,
    }
}

/// Ensures a research receipt at `receipt_path` carries the mandatory `mode` and
/// `winning_rule` fields after dispatch, merging into whatever a spawned executor
/// wrote (or writing a fresh minimal receipt if the file is absent) rather than
/// clobbering existing content. Idempotent: a no-op when both fields already appear.
/// Exists because `run_dispatch`'s own freshness precondition deletes a pre-written
/// skeleton receipt before spawning a routed, available executor, so a pre-dispatch
/// write alone cannot guarantee these fields land in the final file.
fn ensure_research_receipt_mandatory_fields(
    receipt_path: &str,
    task_id: &str,
    slug: &str,
    mode: &str,
    winning_rule: &str,
) {
    let final_content = std::fs::read_to_string(receipt_path).unwrap_or_default();
    // Line-anchored, not a bare substring scan: an executor's free-form output
    // could otherwise contain an unrelated line ending in e.g. "..._mode:" and
    // falsely satisfy a substring check, skipping the mandatory-field append.
    let has_field = |field: &str| {
        final_content
            .lines()
            .any(|line| line.trim_start().starts_with(field))
    };
    if has_field("mode:") && has_field("winning_rule:") {
        return;
    }
    let mandatory_fields = format!("\nmode: {mode}\nwinning_rule: {winning_rule}\n");
    let merged = if final_content.is_empty() {
        format!("# Research Dispatch Receipt\n\ntask: {task_id}\nslug: {slug}\n{mandatory_fields}")
    } else {
        format!("{final_content}{mandatory_fields}")
    };
    if let Some(parent) = PathBuf::from(receipt_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(receipt_path, merged);
}

/// `gal research <slug> --worker <1|2>` — the dedicated research dispatch lane
/// entry point (`research` is already a registered `CommandKind`; this wires it).
/// Builds a minimal `investigate` task spec via the existing
/// `pipeline::task_spec::assemble_task_spec` machinery (executor-smoke
/// precedent, `crates/cli/src/commands/executor_smoke.rs:220`) and dispatches
/// it in-process via the existing `dispatch::run::run_dispatch` internals,
/// scoping its receipt/executor-log directory under the `.dev/research/<slug>.md`
/// scope already wired for `gal pipeline` (`resolve_receipt_path` /
/// `resolve_log_dir_override`). The dispatched task id is `worker-<n>`.
///
/// Worker `#0` is always a native subagent, never dispatched — rejected here
/// with that explanation rather than silently falling through to a lookup.
/// Any worker other than `1`/`2` is rejected as out of range.
pub(crate) fn cmd_research(args: &[String]) -> ExitCode {
    let flags: Vec<String> = args.iter().skip(1).cloned().collect();
    if flags.iter().any(|a| a == "--help" || a == "-h") {
        println!("gal research <slug> --worker <1|2>");
        return ExitCode::Success;
    }

    let Some(slug) = flags.first().filter(|s| !s.starts_with('-')) else {
        eprintln!("gal research: missing <slug>");
        eprintln!("usage: gal research <slug> --worker <1|2>");
        return ExitCode::Usage;
    };
    let slug = slug.clone();

    let mut worker: Option<u32> = None;
    let mut index = 1usize;
    while index < flags.len() {
        match flags[index].as_str() {
            "--worker" => {
                index += 1;
                let Some(value) = flags.get(index) else {
                    eprintln!("gal research: --worker requires a value");
                    return ExitCode::Usage;
                };
                worker = match value.parse::<u32>() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        eprintln!(
                            "gal research: invalid --worker value '{value}'; expected 1 or 2"
                        );
                        return ExitCode::Usage;
                    }
                };
            }
            other => {
                eprintln!("gal research: unknown argument '{other}'");
                return ExitCode::Usage;
            }
        }
        index += 1;
    }

    let Some(worker) = worker else {
        eprintln!("gal research: --worker is required (1 or 2)");
        return ExitCode::Usage;
    };
    if worker == 0 {
        eprintln!(
            "gal research: --worker 0 cannot be dispatched; worker #0 is always a native \
             subagent, never dispatched"
        );
        return ExitCode::Usage;
    }
    if worker > 2 {
        eprintln!("gal research: --worker {worker} out of range; expected 1 or 2");
        return ExitCode::Usage;
    }

    let workdir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let research_path = PathBuf::from(".dev")
        .join("research")
        .join(format!("{slug}.md"));
    let research_path_disp = research_path.to_string_lossy().replace('\\', "/");
    let task_id = format!("worker-{worker}");

    let receipt = match resolve_receipt_path(
        Some(&research_path),
        Some(&format!("worker-{worker}")),
        format!("investigate-{worker}.receipt.md"),
    ) {
        Ok(path) => path.to_string_lossy().replace('\\', "/"),
        Err(error) => {
            eprintln!("gal research: {error}");
            return ExitCode::Error;
        }
    };

    // Resolve the winning routing rule (`resolve_worker_route`) up front so
    // the receipt can name it, and derive the execution mode from whether that
    // rule produced a dispatch target: `routed` when it did, `subagent` when the
    // resolution fell back to a native subagent. Mode is mandatory on this
    // receipt so a subagent run is never mistaken for a routed run.
    let routing_table = dispatch::routing::load_routing_default();
    let worker_route = dispatch::routing::resolve_worker_route(worker, &routing_table);
    let mode = if worker_route.route.is_some() {
        "routed"
    } else {
        "subagent"
    };

    // Pre-write the receipt skeleton before attempting to spawn anything: a full
    // dispatch spawns a real executor CLI, whose own completion write-back is not
    // observable in every environment (no executor available), so the mandatory
    // `mode` and `winning_rule` fields must land deterministically here rather
    // than depend on the executor's own receipt write.
    if let Some(parent) = PathBuf::from(&receipt).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let receipt_skeleton = format!(
        "# Research Dispatch Receipt\n\n\
         task: {task_id}\n\
         slug: {slug}\n\
         mode: {mode}\n\
         winning_rule: {winning_rule}\n\
         status: dispatched\n",
        winning_rule = worker_route.winning_rule,
    );
    let _ = std::fs::write(&receipt, receipt_skeleton);

    let now = gal_foundation::time::now_timestamp();
    let branch = git_current_branch();
    let head = git_head_short();
    let prompt_body = format!(
        "## Tasks\n\n\
         - [ ] {task_id} — Research: {slug} (dispatched worker {worker})\n\
         \x20 - Change: investigate `{slug}` and write findings to the research brief.\n"
    );
    let input = pipeline::task_spec::TaskSpecInput {
        task_scope: &task_id,
        phase: "investigate",
        prompt_path: &research_path_disp,
        prompt_body: &prompt_body,
        generated: &now,
        git_branch: &branch,
        git_head: &head,
        convention_hints: None,
        receipt_path: Some(&receipt),
        agent_contract_body: None,
        fix_mode: false,
    };
    let spec = match pipeline::task_spec::assemble_task_spec(&input) {
        Ok(spec) => spec.markdown,
        Err(e) => {
            eprintln!("gal research: task-spec assembly failed: {e}");
            return ExitCode::Error;
        }
    };

    let dispatch_args = dispatch::cli::DispatchArgs {
        phase: dispatch::stage::Phase::Investigate,
        task: task_id.clone(),
        workdir: workdir.clone(),
        receipt_path: Some(PathBuf::from(&receipt)),
        log_dir_override: match resolve_log_dir_override(
            &workdir,
            Some(&research_path),
            Some(&task_id),
        ) {
            Ok(path) => path,
            Err(error) => {
                eprintln!("gal research: {error}");
                return ExitCode::Error;
            }
        },
        worker: Some(worker),
        ..Default::default()
    };
    if let Some(dir) = &dispatch_args.log_dir_override {
        let _ = std::fs::create_dir_all(dir);
    }

    let attempt_token = new_attempt_token();
    let attempt_context = AttemptContext {
        attempt_token: Some(attempt_token.clone()),
        ..AttemptContext::default()
    };
    let outcome =
        dispatch::run::run_dispatch_with_context(&dispatch_args, &spec, Some(&attempt_context));
    capture_attempt_usage(
        &workdir,
        Some(&research_path),
        &attempt_token,
        outcome.log_path.as_deref(),
        Some(&outcome.quota),
    );

    // The pre-dispatch skeleton write above is not the final answer for a routed,
    // available executor: `run_dispatch`'s own freshness precondition deletes any
    // pre-existing file at this managed receipt path before spawning, so the
    // executor's own completion write-back is what actually lands. That write-back
    // is not required to carry `mode`/`winning_rule` (it is free-form executor
    // output), so re-apply both mandatory fields here, after dispatch, merging into
    // whatever the executor wrote rather than clobbering it. This covers every
    // outcome: no-receipt (file absent — write the skeleton fresh), executor wrote
    // its own content without these fields (append them), or a native-subagent /
    // pre-spawn-degrade run that never disturbed the skeleton (fields already
    // present, appending a duplicate second copy is harmless — both readers this
    // repository has treat the field as present, not as unique-per-file).
    ensure_research_receipt_mandatory_fields(
        &receipt,
        &task_id,
        &slug,
        mode,
        worker_route.winning_rule,
    );

    match outcome.exit_code {
        0 => ExitCode::Success,
        1 => ExitCode::Error,
        2 => ExitCode::NotWired,
        _ => ExitCode::Error,
    }
}

pub(crate) fn cmd_pipeline(args: &[String]) -> ExitCode {
    if args.iter().any(|arg| arg == "--require-codex-stop-v1") {
        if requests_finalize_review(args) {
            eprintln!(
                "gal pipeline: --phase finalize-review does not run under the guarded task driver; use the low-level phase path"
            );
            return ExitCode::Usage;
        }
        return super::pipeline_driver::cmd_guarded_pipeline_entry(args);
    }
    if args.len() < 2 {
        eprintln!("gal pipeline: missing plan / prompt / task-spec path");
        eprintln!(
            "usage: gal pipeline <PromptPath|SourcePlanPath|TaskSpecPath> [--phase <phase>] [--task <T-NN>] [--receipt <path>] [--fix]"
        );
        return ExitCode::Usage;
    }

    let raw_input = PathBuf::from(&args[1]);

    let mut phase = String::from("implement");
    let mut task_override: Option<String> = None;
    let mut receipt_path: Option<String> = None;
    let mut workdir_flag: Option<String> = None;
    let mut fix_mode = false;
    let mut host_dispatch = false;
    let mut close_attempt: Option<String> = None;
    let mut host_session: Option<String> = None;
    let mut tier_override: Option<dispatch::model_tier::ModelTier> = None;
    let mut tier_reason: Option<String> = None;

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
            "--fix" => fix_mode = true,
            "--dispatch" => {
                index += 1;
                if args.get(index).is_none_or(|value| value != "host-subagent") {
                    eprintln!("gal pipeline: --dispatch requires host-subagent");
                    return ExitCode::Usage;
                }
                host_dispatch = true;
            }
            "--close" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("gal pipeline: --close requires an attempt id");
                    return ExitCode::Usage;
                };
                close_attempt = Some(value.clone());
            }
            "--session" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("gal pipeline: --session requires a value");
                    return ExitCode::Usage;
                };
                host_session = Some(value.clone());
            }
            "--tier" => {
                index += 1;
                tier_override = match args.get(index).map(String::as_str) {
                    Some("light") => Some(dispatch::model_tier::ModelTier::Light),
                    Some("standard") => Some(dispatch::model_tier::ModelTier::Standard),
                    Some("deep") => Some(dispatch::model_tier::ModelTier::Deep),
                    _ => {
                        eprintln!("gal pipeline: --tier requires light, standard, or deep");
                        return ExitCode::Usage;
                    }
                };
            }
            "--tier-reason" => {
                index += 1;
                let Some(value) = args.get(index).filter(|v| !v.trim().is_empty()) else {
                    eprintln!("gal pipeline: --tier-reason requires a non-empty value");
                    return ExitCode::Usage;
                };
                tier_reason = Some(value.clone());
            }
            other => {
                eprintln!("gal pipeline: unknown argument '{other}'");
                return ExitCode::Usage;
            }
        }
        index += 1;
    }

    if (close_attempt.is_some() != host_session.is_some())
        || (close_attempt.is_some()
            && (host_dispatch || tier_override.is_some() || tier_reason.is_some()))
        || (tier_override.is_some() != tier_reason.is_some())
    {
        eprintln!(
            "gal pipeline: --close and --session must be paired; tier options apply only to open"
        );
        return ExitCode::Usage;
    }
    if (tier_override.is_some() || tier_reason.is_some()) && !host_dispatch {
        eprintln!("gal pipeline: tier options require --dispatch host-subagent");
        return ExitCode::Usage;
    }

    // `gal pipeline` dispatches only the four implementation-lane phases. `investigate`
    // is now a parseable `Phase` so it must not silently fall through to the
    // implementer contract (`agent_contract_rel`'s `_` branch) — reject it here with a
    // usage error naming the dedicated `gal research` command instead.
    if let Err(message) = validate_pipeline_phase(&phase) {
        eprintln!("gal pipeline: {message}");
        return ExitCode::Usage;
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
    let workdir = if host_dispatch || close_attempt.is_some() {
        match std::fs::canonicalize(&workdir) {
            Ok(path) => PathBuf::from(super::pipeline_execution::binding_path_text(&path)),
            Err(error) => {
                eprintln!("gal pipeline: host worktree unavailable: {error}");
                return ExitCode::Error;
            }
        }
    } else {
        workdir
    };
    let input_path = if host_dispatch || close_attempt.is_some() {
        match std::fs::canonicalize(&input_path) {
            Ok(path) => PathBuf::from(super::pipeline_execution::binding_path_text(&path)),
            Err(error) => {
                eprintln!("gal pipeline: host prompt unavailable: {error}");
                return ExitCode::Error;
            }
        }
    } else {
        input_path
    };
    let input_kind = resolve_pipeline_input(&input_path);
    // The plan-level review has no task, so it branches before any task inference.
    if phase.eq_ignore_ascii_case(FINALIZE_REVIEW_PHASE) {
        return run_finalize_review(&FinalizeReviewRequest {
            input_path: &input_path,
            input_kind,
            workdir: &workdir,
            task_override: task_override.as_deref(),
            receipt_override: receipt_path.as_deref(),
            fix_mode,
            host_dispatch,
            close_attempt: close_attempt.as_deref(),
            host_session: host_session.as_deref(),
        });
    }
    if let Err(message) = validate_fix_mode(fix_mode, &phase, input_kind) {
        eprintln!("gal pipeline: {message}");
        return ExitCode::Usage;
    }
    let mut prompt_lease = PipelinePromptLease(None);
    let attempt_token = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );

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
    let (
        spec,
        task_id,
        effective_receipt,
        plan_log_source,
        provenance,
        affected_paths,
        fix_authority,
        raw_date,
    ) = match input_kind {
        PipelineInput::Prompt => {
            if !input_path.exists() {
                eprintln!("gal pipeline: prompt not found: {}", input_path.display());
                return ExitCode::Error;
            }
            let lease_task = match task_override.clone() {
                Some(task) => task,
                None => match std::fs::read_to_string(&input_path)
                    .ok()
                    .and_then(|body| first_task_id(&body))
                {
                    Some(task) => task,
                    None => {
                        eprintln!("gal pipeline: no task id found in prompt");
                        return ExitCode::Error;
                    }
                },
            };
            let lease_receipt = match effective_receipt_path(
                &phase,
                &lease_task,
                receipt_path.as_deref(),
                Some(&input_path),
                "",
            ) {
                Ok(path) => path.map(|path| {
                    let path = PathBuf::from(path);
                    if path.is_absolute() {
                        path
                    } else {
                        workdir.join(path)
                    }
                }),
                Err(error) => {
                    eprintln!("gal pipeline: {error}");
                    return ExitCode::Error;
                }
            };
            // A host attempt holds the prompt writer lease from open until close, and close
            // reattaches it, so neither step may acquire a second lease here.
            if !host_dispatch && close_attempt.is_none() {
                prompt_lease.0 = match dispatch::dispatch::acquire_prompt_writer_lease(
                    &workdir,
                    &input_path,
                    lease_receipt.as_deref(),
                    &attempt_token,
                ) {
                    Ok(lease) => Some(lease),
                    Err(error) => {
                        eprintln!("gal pipeline: {error}");
                        return ExitCode::Error;
                    }
                };
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
                &workdir,
                &phase,
                task_override.as_deref(),
                receipt_path.as_deref(),
                Some(&contract.body),
                fix_mode,
            ) {
                Ok((spec, task_id, receipt, affected, authority)) => (
                    spec,
                    task_id,
                    receipt,
                    Some(input_path.clone()),
                    Some(ContractProvenance {
                        path: contract.path,
                        source: contract.source,
                    }),
                    affected,
                    authority,
                    None,
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
            let lease_task = match task_override.clone() {
                Some(task) => task,
                None => match std::fs::read_to_string(&prompt)
                    .ok()
                    .and_then(|body| first_task_id(&body))
                {
                    Some(task) => task,
                    None => {
                        eprintln!("gal pipeline: no task id found in prompt");
                        return ExitCode::Error;
                    }
                },
            };
            let lease_receipt = match effective_receipt_path(
                &phase,
                &lease_task,
                receipt_path.as_deref(),
                Some(&prompt),
                "",
            ) {
                Ok(path) => path.map(|path| {
                    let path = PathBuf::from(path);
                    if path.is_absolute() {
                        path
                    } else {
                        workdir.join(path)
                    }
                }),
                Err(error) => {
                    eprintln!("gal pipeline: {error}");
                    return ExitCode::Error;
                }
            };
            if !host_dispatch && close_attempt.is_none() {
                prompt_lease.0 = match dispatch::dispatch::acquire_prompt_writer_lease(
                    &workdir,
                    &prompt,
                    lease_receipt.as_deref(),
                    &attempt_token,
                ) {
                    Ok(lease) => Some(lease),
                    Err(error) => {
                        eprintln!("gal pipeline: {error}");
                        return ExitCode::Error;
                    }
                };
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
                &workdir,
                &phase,
                task_override.as_deref(),
                receipt_path.as_deref(),
                Some(&contract.body),
                fix_mode,
            ) {
                Ok((spec, task_id, receipt, affected, authority)) => (
                    spec,
                    task_id,
                    receipt,
                    Some(prompt),
                    Some(ContractProvenance {
                        path: contract.path,
                        source: contract.source,
                    }),
                    affected,
                    authority,
                    None,
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
            // A `.dev/research/<slug>.md` source scopes its receipt and executor-log
            // directory the same way a `.dev/plans/` Prompt/SourcePlan input does;
            // every other RawSpec input (materialized task specs, etc.)
            // keeps the legacy unscoped location untouched. This is fed into
            // `effective_receipt_path` here and into `receipt_log_scope_source`
            // below (for `resolve_log_dir_override`) ONLY — `plan_log_source` stays
            // `None` for every RawSpec input, exactly as before, because that same
            // variable also gates prompt-snapshot/mutation-detection, phase-writeback
            // file mutation, and `--fix` replay-fingerprint gating: none of those
            // apply to a research note, which is not a plan/prompt file.
            let scope_source = research_scope_source(&input_path).then(|| input_path.clone());
            let raw_date = scope_source
                .is_none()
                .then(gal_foundation::time::utc_date_compact);
            let receipt = match effective_receipt_path(
                &phase,
                &task_id,
                receipt_path.as_deref(),
                scope_source.as_deref(),
                raw_date.as_deref().unwrap_or(""),
            ) {
                Ok(path) => path,
                Err(error) => {
                    eprintln!("gal pipeline: {error}");
                    return ExitCode::Error;
                }
            };
            (
                spec,
                task_id,
                receipt,
                None,
                None,
                Vec::new(),
                None,
                raw_date,
            )
        }
    };

    if input_kind != PipelineInput::RawSpec && matches!(phase.as_str(), "test" | "audit") {
        let status_body = plan_log_source
            .as_deref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .unwrap_or_else(|| spec.clone());
        if let Err(message) = validate_task_final_commit(&status_body, &workdir) {
            eprintln!("gal pipeline: {message}");
            return ExitCode::Error;
        }
    }

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

    if fix_mode {
        let Some(plan_source) = plan_log_source.as_deref() else {
            eprintln!("gal pipeline: --fix requires a plan-scoped prompt or source plan");
            return ExitCode::Usage;
        };
        let Some(authority) = fix_authority.as_ref() else {
            eprintln!("gal pipeline: fix-mode stable authority inputs were not assembled");
            return ExitCode::Error;
        };
        let fingerprint = stable_fix_authority_fingerprint(authority);
        let replay_path = match resolve_receipt_path(
            Some(plan_source),
            Some(&task_id),
            format!("{task_id}-{phase}.sha256"),
        ) {
            Ok(path) => workdir.join(path),
            Err(error) => {
                eprintln!("gal pipeline: {error}");
                return ExitCode::Error;
            }
        };
        match persist_fix_fingerprint(&replay_path, &fingerprint) {
            Ok(true) => {
                eprintln!(
                    "gal pipeline: fix-mode replay refused for task {task_id} phase {phase}: stable authority fingerprint is unchanged"
                );
                return ExitCode::Error;
            }
            Ok(false) => {}
            Err(error) => {
                eprintln!(
                    "gal pipeline: failed to persist replay fingerprint '{}': {error}",
                    replay_path.display()
                );
                return ExitCode::Error;
            }
        }
    }

    // Run the dispatch in-process via the dispatch library — the single `gal`
    // binary is self-contained and does not depend on a separate `gal-dispatch`
    // executable (which is not in the end-user release artifact). The safety
    // gate lives in `dispatch::run::run_dispatch`.
    let mut raw_args = vec![
        "--phase".to_string(),
        phase,
        "--task".to_string(),
        task_id.clone(),
        "--workdir".to_string(),
        workdir.display().to_string(),
    ];
    if let Some(ref receipt) = effective_receipt {
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
    //
    // A `.dev/research/<slug>.md` RawSpec input scopes its executor-log directory
    // the same way, but through this dedicated variable rather than `plan_log_source`
    // itself — `plan_log_source` also gates prompt-snapshot/mutation-detection,
    // phase-writeback file mutation, and `--fix` replay-fingerprint gating below,
    // none of which apply to a research note.
    let receipt_log_scope_source =
        if matches!(input_kind, PipelineInput::RawSpec) && research_scope_source(&input_path) {
            Some(input_path.clone())
        } else {
            plan_log_source.clone()
        };
    args.log_dir_override = match raw_date.as_deref() {
        Some(date) => Some(
            workdir
                .join(".dev")
                .join("pipeline")
                .join(date)
                .join("test-direct"),
        ),
        None => match resolve_log_dir_override(
            &workdir,
            receipt_log_scope_source.as_deref(),
            Some(&task_id),
        ) {
            Ok(path) => path,
            Err(error) => {
                eprintln!("gal pipeline: {error}");
                return ExitCode::Error;
            }
        },
    };
    // Ensure the scoped executor-log directory exists as soon as it is resolved,
    // not only once a dispatch attempt reaches `write_started_marker`: a dispatch
    // that returns before ever spawning (e.g. `executor-unavailable`) still needs
    // the scope's directory to exist for callers (tests, tooling) that key off its
    // presence. Best-effort — a create failure here is surfaced later by the
    // dispatch itself when it tries to write into the same directory.
    if let Some(dir) = &args.log_dir_override {
        let _ = std::fs::create_dir_all(dir);
    }

    // R4: carry the resolved contract provenance (path + tier) into dispatch so
    // markers and executor-log headers record which source served the contract.
    // `None` for raw/direct dispatch keeps its byte-compatible marker/log shape (R8).
    args.contract_provenance = provenance;
    let prompt_snapshot = plan_log_source
        .as_deref()
        .and_then(|p| snapshot_prompt(&workdir, p, &task_id, args.phase.as_str()));

    let prompt_path = plan_log_source.as_deref();
    if host_dispatch || close_attempt.is_some() {
        let Some(prompt_path) = prompt_path else {
            eprintln!("gal pipeline: host dispatch requires a plan-scoped prompt");
            return ExitCode::Usage;
        };
        let pin = match read_preflight_pin(&workdir, prompt_path) {
            Ok(pin) => pin,
            Err(_) => {
                eprintln!(
                    "gal pipeline: host dispatch requires a valid canonical preflight receipt"
                );
                return ExitCode::Usage;
            }
        };
        let host = match pin.host.as_deref() {
            Some(host) => host,
            None => {
                eprintln!("gal pipeline: preflight receipt does not pin a host");
                return ExitCode::Usage;
            }
        };
        let scope = match resolve_scope(prompt_path) {
            Ok(scope) => scope,
            Err(error) => {
                eprintln!("gal pipeline: {error}");
                return ExitCode::Error;
            }
        };
        let task_scope_dir = workdir.join(".dev/pipeline").join(&scope).join(&task_id);
        if let Some(attempt_id) = close_attempt.as_deref() {
            let coordinator = workdir
                .join(".dev/pipeline")
                .join(&scope)
                .join("coordinator.json");
            if coordinator.exists() {
                eprintln!(
                    "gal pipeline: legacy close cannot consume coordinator-owned host attempts"
                );
                return ExitCode::Error;
            }
            let mut placement = |bytes: &[u8]| -> Result<(), String> {
                // Only test and audit place a receipt in the prompt. Implement delivers its
                // result as the worktree change, matching the guarded driver close.
                if !pipeline::task_spec::is_writeback_in_scope(args.phase.as_str()) {
                    return Ok(());
                }
                let snapshot = prompt_snapshot
                    .as_ref()
                    .ok_or("execution prompt snapshot unavailable")?;
                let rendered = pipeline::task_spec::render_phase_writeback(
                    &snapshot.bytes,
                    bytes,
                    args.phase.as_str(),
                    &task_id,
                )
                .map_err(|e| e.to_string())?;
                atomic_prompt_write(prompt_path, &rendered).map_err(|e| e.to_string())
            };
            if prompt_snapshot.is_none() {
                eprintln!("gal pipeline: close requires the persisted prompt snapshot");
                return ExitCode::Error;
            }
            return close_legacy_host_attempt(
                &workdir,
                &task_scope_dir,
                &task_id,
                args.phase.as_str(),
                host,
                host_session.as_deref().unwrap_or(""),
                attempt_id,
                &mut placement,
            );
        }
        if !host_dispatch
            || pin.dispatch_mode != super::pipeline_preflight::DispatchMode::HostSubagent
        {
            eprintln!(
                "gal pipeline: host open requires --dispatch host-subagent and a host-subagent preflight pin"
            );
            return ExitCode::Usage;
        }
        let Some(snapshot) = prompt_snapshot.as_ref() else {
            eprintln!("gal pipeline: mandatory prompt backup snapshot could not be created");
            return ExitCode::Error;
        };
        if let Err(code) = prepare_legacy_host_open(
            &mut args,
            &workdir,
            &task_scope_dir,
            prompt_path,
            &task_id,
            host,
            pin.run_id,
            &attempt_token,
            snapshot,
            &spec,
            effective_receipt.as_deref(),
            tier_override,
            tier_reason,
        ) {
            return code;
        }
    }

    let attempt_context = AttemptContext {
        attempt_token: Some(attempt_token),
        canonical_prompt: plan_log_source.clone(),
        ..AttemptContext::default()
    };
    let fix_origin = fix_authority
        .as_ref()
        .and_then(|authority| handoff_origin_phase(&authority.handoff_heading))
        .unwrap_or_default();
    let before_affected = fix_mode.then(|| fingerprint_affected_files(&workdir, &affected_paths));
    let outcome = dispatch::run::run_dispatch_with_context(&args, &spec, Some(&attempt_context));
    capture_pipeline_usage(
        &workdir,
        plan_log_source.as_deref(),
        attempt_context.attempt_token.as_deref().unwrap_or_default(),
        outcome.log_path.as_deref(),
        Some(&outcome.quota),
    );
    if host_dispatch && outcome.exit_code != 0 {
        eprintln!(
            "gal pipeline: host open failed: {}",
            outcome.reason.as_deref().unwrap_or("unknown")
        );
    }
    if host_dispatch && outcome.exit_code == 0 && outcome.terminal_state.is_none() {
        let Some(options) = args.host_attempt.as_ref() else {
            return ExitCode::Error;
        };
        let Some(manifest) = options.state.as_ref() else {
            return ExitCode::Error;
        };
        if manifest.profile == "codex_stop_v1" {
            if let Err(error) = super::pipeline_driver::publish_host_checkpoint(manifest) {
                eprintln!("gal pipeline: host checkpoint publication failed: {error}");
                return ExitCode::Error;
            }
        } else {
            println!(
                "CLOSE: gal pipeline {} --task {} --phase {} --close {} --session <session_ref>",
                manifest.prompt.display(),
                manifest.task,
                manifest.phase,
                manifest.attempt_id
            );
        }
        return ExitCode::Success;
    }
    #[cfg(test)]
    PIPELINE_AFTER_DISPATCH_TEST_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
    if !matches!(
        outcome.evidence.cleanup,
        dispatch::dispatch::CleanupDisposition::Confirmed
    ) {
        if let Some(lease) = prompt_lease.0.as_mut() {
            lease.retain();
        }
    }
    if let Some(before) = before_affected.as_deref() {
        let after = fingerprint_affected_files(&workdir, &affected_paths);
        let terminal = outcome.terminal_state.as_ref().map(|state| state.as_str());
        let guard_error = match fix_round_guard(fix_origin, terminal, before, &after) {
            FixRoundGuard::Pass => None,
            FixRoundGuard::FixRoundNoChange => Some((
                "fix-round-no-change: affected implementation files were unchanged".to_string(),
                format!(
                    "gal pipeline: fix-round-no-change for task {task_id}: affected implementation files were unchanged"
                ),
            )),
            FixRoundGuard::BoundaryRepairTouchedScope => Some((
                "boundary-repair-touched-scope: IMPLEMENT boundary repair changed allowlisted files"
                    .to_string(),
                format!(
                    "gal pipeline: boundary-repair-touched-scope for task {task_id}: IMPLEMENT boundary repair changed allowlisted files"
                ),
            )),
        };
        if let Some((msg, stderr_line)) = guard_error {
            let now = gal_foundation::time::now_timestamp();
            let event = pipeline::loop_log::LoopEvent {
                time: now,
                task: task_id.clone(),
                phase: args.phase.as_str().to_string(),
                role: args.phase.role().to_string(),
                kind: "retry".to_string(),
                level: pipeline::loop_log::LoopLevel::Error,
                msg,
                log_ptr: outcome
                    .log_path
                    .as_ref()
                    .map(|path| path.display().to_string()),
            };
            let dir = workdir
                .join(".dev")
                .join("pipeline")
                .join(gal_foundation::time::utc_date_compact());
            let _ = pipeline::loop_log::append_event(&dir, "loop-log", &event);
            eprintln!("{stderr_line}");
            return ExitCode::Error;
        }
    }

    // Loop-log the dispatch outcome: one structured event per terminal
    // state, written under the workdir's gitignored loop-log dir. This lives in
    // cli (not dispatch) because the crate DAG forbids `dispatch -> pipeline`.
    if let Some(state) = &outcome.terminal_state {
        let state_str = state.as_str();
        let (kind, level) = pipeline::loop_log::classify_dispatch(state_str);
        let now = gal_foundation::time::now_timestamp();
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
        let dir = workdir
            .join(".dev")
            .join("pipeline")
            .join(gal_foundation::time::utc_date_compact());
        let _ = pipeline::loop_log::append_event(&dir, "loop-log", &event); // best-effort forensics
    }

    // A third party changing the execution prompt during a dispatch is invisible
    // to the worktree containment check, which only fires on sibling worktrees.
    // Detect it here and keep the pre-dispatch bytes, so a silent loss becomes a
    // named, recoverable event. Deliberately outside the success guard below: a
    // failed dispatch can leave the file mutated too.
    if let (Some(snap), Some(prompt_path)) = (&prompt_snapshot, plan_log_source.as_deref()) {
        if let Ok(after_bytes) = std::fs::read(prompt_path) {
            if let Some(msg) = prompt_mutation_msg(snap, &after_bytes) {
                let now = gal_foundation::time::now_timestamp();
                let event = pipeline::loop_log::LoopEvent {
                    time: now,
                    task: task_id.clone(),
                    phase: args.phase.as_str().to_string(),
                    role: args.phase.role().to_string(),
                    kind: "prompt-mutated".to_string(),
                    level: pipeline::loop_log::LoopLevel::Error,
                    msg: msg.clone(),
                    log_ptr: outcome.log_path.as_ref().map(|p| p.display().to_string()),
                };
                let dir = workdir
                    .join(".dev")
                    .join("pipeline")
                    .join(gal_foundation::time::utc_date_compact());
                let _ = pipeline::loop_log::append_event(&dir, "loop-log", &event);
                eprintln!("gal pipeline: task {task_id}: {msg}");
            }
        }
    }

    if outcome.exit_code == 0
        && outcome
            .terminal_state
            .as_ref()
            .is_some_and(|state| state.as_str() == "completed")
    {
        if let Some(prompt_path) = plan_log_source.as_deref() {
            let prompt_bytes = match prompt_snapshot.as_ref() {
                Some(snapshot) => snapshot.bytes.clone(),
                None => {
                    let msg = format!(
                        "execution prompt snapshot is unavailable for '{}'",
                        prompt_path.display()
                    );
                    log_semantic_failure(
                        &workdir,
                        &task_id,
                        args.phase.as_str(),
                        args.phase.role(),
                        &msg,
                        outcome.log_path.as_ref(),
                    );
                    eprintln!(
                        "gal pipeline: phase-writeback semantic failure for task {task_id}: {msg}"
                    );
                    return ExitCode::Error;
                }
            };
            if pipeline::task_spec::is_writeback_in_scope(args.phase.as_str()) {
                let receipt_resolved = effective_receipt.as_ref().map(|receipt| {
                    let p = PathBuf::from(receipt);
                    if p.is_absolute() {
                        p
                    } else {
                        workdir.join(&p)
                    }
                });
                let Some(receipt_file) = receipt_resolved else {
                    let msg = "no receipt path resolved for in-scope phase".to_string();
                    log_semantic_failure(
                        &workdir,
                        &task_id,
                        args.phase.as_str(),
                        args.phase.role(),
                        &msg,
                        outcome.log_path.as_ref(),
                    );
                    eprintln!(
                        "gal pipeline: phase-writeback semantic failure for task {task_id}: {msg}"
                    );
                    return ExitCode::Error;
                };
                let Some(receipt_snapshot) = outcome.evidence.receipt_snapshot.as_ref() else {
                    let msg = format!(
                        "dispatch did not produce an integrity-checked receipt snapshot for '{}'",
                        receipt_file.display()
                    );
                    log_semantic_failure(
                        &workdir,
                        &task_id,
                        args.phase.as_str(),
                        args.phase.role(),
                        &msg,
                        outcome.log_path.as_ref(),
                    );
                    eprintln!(
                        "gal pipeline: phase-writeback semantic failure for task {task_id}: {msg}"
                    );
                    return ExitCode::Error;
                };
                let receipt_bytes = match attempt_receipt_bytes(receipt_snapshot, &receipt_file) {
                    Ok(bytes) => bytes,
                    Err(msg) => {
                        // A receipt changed after dispatch captured its evidence. Keep
                        // the prompt lease so a later attempt cannot turn stale evidence
                        // into a successful writeback.
                        if let Some(lease) = prompt_lease.0.as_mut() {
                            lease.retain();
                        }
                        log_semantic_failure(
                            &workdir,
                            &task_id,
                            args.phase.as_str(),
                            args.phase.role(),
                            &msg,
                            outcome.log_path.as_ref(),
                        );
                        eprintln!(
                            "gal pipeline: phase-writeback semantic failure for task {task_id}: {msg}"
                        );
                        return ExitCode::Error;
                    }
                };
                let rendered = match pipeline::task_spec::render_phase_writeback(
                    &prompt_bytes,
                    receipt_bytes,
                    args.phase.as_str(),
                    &task_id,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        let msg = format!("rendering writeback payload failed: {e}");
                        log_semantic_failure(
                            &workdir,
                            &task_id,
                            args.phase.as_str(),
                            args.phase.role(),
                            &msg,
                            outcome.log_path.as_ref(),
                        );
                        eprintln!(
                            "gal pipeline: phase-writeback semantic failure for task {task_id}: {msg}"
                        );
                        return ExitCode::Error;
                    }
                };

                if let Err(e) = atomic_prompt_write(prompt_path, &rendered) {
                    let msg = format!(
                        "failed to atomically write prompt file '{}': {e}",
                        prompt_path.display()
                    );
                    log_semantic_failure(
                        &workdir,
                        &task_id,
                        args.phase.as_str(),
                        args.phase.role(),
                        &msg,
                        outcome.log_path.as_ref(),
                    );
                    eprintln!(
                        "gal pipeline: phase-writeback semantic failure for task {task_id}: {msg}"
                    );
                    return ExitCode::Error;
                }
            }
        }
    }

    if let Some(lease) = prompt_lease.0.as_mut() {
        if let Err(error) = lease.release_confirmed() {
            eprintln!("gal pipeline: prompt lease cleanup remains unconfirmed: {error}");
            return ExitCode::Error;
        }
    }

    match outcome.exit_code {
        0 => ExitCode::Success,
        1 => ExitCode::Error,
        2 => ExitCode::NotWired,
        _ => ExitCode::Error,
    }
}

pub(crate) fn host_head(worktree: &Path) -> Result<String, String> {
    let head = git_capture_in(worktree, &["rev-parse", "HEAD"]);
    if head == "unknown" {
        return Err("host HEAD unavailable".into());
    }
    Ok(head)
}

fn accepted_pipeline_evidence(
    task_scope: &Path,
    run_id: &str,
    task: &str,
    phase: dispatch::stage::Phase,
) -> Result<
    (
        Option<dispatch::model_tier::PrerequisiteEvidence>,
        Option<dispatch::model_tier::PrerequisiteEvidence>,
        usize,
    ),
    String,
> {
    #[derive(Clone)]
    struct Attempt {
        start: u64,
        phase: String,
        task: String,
        terminal: String,
        mode: String,
        tier: Option<dispatch::model_tier::ModelTier>,
    }
    let run_start = run_id
        .split('-')
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let mut attempts = Vec::new();
    // Routed attempt logs sit in the task scope. Host attempt logs sit beside their
    // manifests under `host-attempts/`, so both directories hold accepted evidence.
    let mut log_paths = Vec::new();
    for dir in [task_scope.to_path_buf(), task_scope.join("host-attempts")] {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && dir != task_scope => {
                continue
            }
            Err(error) => return Err(error.to_string()),
        };
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_file()
            {
                log_paths.push(entry.path());
            }
        }
    }
    for path in log_paths {
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        if body.lines().next() != Some("GAL-DISPATCH-LOG v1") {
            continue;
        }
        let field = |key: &str| {
            body.lines().find_map(|line| {
                line.split_once(':')
                    .filter(|(name, _)| name.trim() == key)
                    .map(|(_, value)| value.trim().to_string())
            })
        };
        let Some(start) = field("timestamp_start")
            .and_then(|value| dispatch::host_session::epoch_seconds(&value).ok())
        else {
            continue;
        };
        let parsed = Attempt {
            start,
            phase: field("phase").unwrap_or_default(),
            task: field("task_id").unwrap_or_default(),
            terminal: field("terminal_state").unwrap_or_default(),
            mode: field("executor").unwrap_or_else(|| "text".into()),
            tier: field("tier").and_then(|v| match v.as_str() {
                "light" => Some(dispatch::model_tier::ModelTier::Light),
                "standard" => Some(dispatch::model_tier::ModelTier::Standard),
                "deep" => Some(dispatch::model_tier::ModelTier::Deep),
                _ => None,
            }),
        };
        if parsed.start >= run_start && parsed.task == task {
            attempts.push(parsed);
        }
    }
    let evidence = |wanted: &str,
                    after: Option<u64>|
     -> Result<
        Option<(u64, dispatch::model_tier::PrerequisiteEvidence)>,
        String,
    > {
        let mut matching = attempts
            .iter()
            .filter(|a| {
                a.phase == wanted && a.terminal == "completed" && after.is_none_or(|t| a.start > t)
            })
            .collect::<Vec<_>>();
        let Some(latest) = matching.iter().map(|a| a.start).max() else {
            return Ok(None);
        };
        matching.retain(|a| a.start == latest);
        if matching.len() != 1 {
            return Err(format!("ambiguous latest {wanted} attempts"));
        }
        let a = matching[0];
        Ok(Some((
            a.start,
            dispatch::model_tier::PrerequisiteEvidence {
                accepted: true,
                mode: if a.mode == "host-subagent" {
                    a.mode.clone()
                } else {
                    "routed".into()
                },
                tier: a.tier,
            },
        )))
    };
    let needs_impl = matches!(
        phase,
        dispatch::stage::Phase::Test | dispatch::stage::Phase::Audit
    );
    let needs_test = matches!(phase, dispatch::stage::Phase::Audit);
    let implement = evidence("implement", None)?;
    let test = evidence("test", implement.as_ref().map(|(start, _)| *start))?;
    if needs_impl && implement.is_none() || needs_test && test.is_none() {
        return Err("required accepted prerequisite is missing".into());
    }
    let failures = attempts
        .iter()
        .filter(|a| a.phase == phase.as_str() && a.terminal != "completed")
        .count();
    Ok((implement.map(|(_, e)| e), test.map(|(_, e)| e), failures))
}

#[allow(clippy::too_many_arguments)]
fn close_legacy_host_attempt(
    workdir: &Path,
    task_scope_dir: &Path,
    task: &str,
    phase: &str,
    host: &str,
    session_reference: &str,
    attempt_id: &str,
    placement: &mut dyn FnMut(&[u8]) -> Result<(), String>,
) -> ExitCode {
    let manifest_path = match dispatch::host_attempt_state::HostAttemptState::manifest_path_for(
        task_scope_dir,
        attempt_id,
    ) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("gal pipeline: host close failed: {error}");
            return ExitCode::Error;
        }
    };
    let manifest =
        match dispatch::host_attempt_state::HostAttemptState::load(&manifest_path, attempt_id) {
            Ok(manifest) => manifest,
            Err(error) => {
                eprintln!("gal pipeline: host close failed: {error}");
                return ExitCode::Error;
            }
        };
    if manifest.profile != "legacy_interactive"
        || manifest.task != task
        || manifest.phase != phase
        || manifest.host != host
    {
        eprintln!(
            "gal pipeline: host close manifest does not match this legacy task, phase, and host"
        );
        return ExitCode::Usage;
    }
    let session_home = gal_foundation::paths::user_home();
    let result = dispatch::run::close_host_attempt(
        workdir,
        task_scope_dir,
        task,
        attempt_id,
        task,
        phase,
        host,
        session_reference,
        &match session_home {
            Some(home) => home,
            None => {
                eprintln!("gal pipeline: cannot resolve the user home directory for host close");
                return ExitCode::Usage;
            }
        },
        projection::ORCHESTRATED_ONLY_GOLEM_NAMES,
        placement,
        &dispatch::run::HostAttemptCloseHooks::default(),
    );
    match result {
        Ok(_) => {
            capture_attempt_usage(
                workdir,
                Some(manifest.prompt.as_path()),
                attempt_id,
                Some(manifest.log_path.as_path()),
                None,
            );
            capture_host_child_usage(
                workdir,
                manifest.prompt.as_path(),
                attempt_id,
                &manifest.task,
                &manifest.phase,
                &manifest.started_at,
                session_reference,
            );
            ExitCode::Success
        }
        Err(error) => {
            eprintln!("gal pipeline: host close failed: {error}");
            if error == dispatch::run::HostAttemptCloseError::Session {
                eprintln!(
                    "gal usage: usage not collected for attempt {attempt_id}: the host session was not verified, so the attempt is left to explicit collection"
                );
            }
            ExitCode::Error
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_legacy_host_open(
    args: &mut dispatch::cli::DispatchArgs,
    workdir: &Path,
    task_scope_dir: &Path,
    prompt_path: &Path,
    task_id: &str,
    host: &str,
    run_id: String,
    attempt_token: &str,
    snapshot: &PromptSnapshot,
    spec: &str,
    effective_receipt: Option<&str>,
    tier_override: Option<dispatch::model_tier::ModelTier>,
    tier_reason: Option<String>,
) -> Result<(), ExitCode> {
    let prompt_text = String::from_utf8_lossy(&snapshot.bytes);
    let project_text = std::fs::read_to_string(workdir.join(".dev/project.md")).unwrap_or_default();
    let mut signals = if args.phase.as_str() == FINALIZE_REVIEW_PHASE {
        dispatch::model_tier::TaskSignals {
            phase: args.phase,
            files: Vec::new(),
            crate_roots: Vec::new(),
            has_protected_path: false,
            dependency_count: 0,
            prior_failed_attempts: 0,
            implement: None,
            test: None,
        }
    } else {
        match pipeline::task_signals::extract_task_signals(
            &prompt_text,
            task_id,
            &project_text,
            args.phase,
        ) {
            Ok(signals) => signals,
            Err(error) => {
                eprintln!("gal pipeline: cannot derive host model signals: {error}");
                return Err(ExitCode::Usage);
            }
        }
    };
    match accepted_pipeline_evidence(task_scope_dir, &run_id, task_id, args.phase) {
        Ok((implement, test, failures)) => {
            signals.implement = implement;
            signals.test = test;
            signals.prior_failed_attempts = failures;
        }
        Err(error) => {
            eprintln!("gal pipeline: cannot select accepted model prerequisites: {error}");
            return Err(ExitCode::Usage);
        }
    }
    let task_scope = task_scope_dir.to_path_buf();
    // The attempt ID doubles as the native subagent name. Codex accepts only lowercase
    // letters, digits, and underscores there, so the ID uses no other character.
    let attempt_id = format!(
        "host_{}",
        attempt_token
            .to_ascii_lowercase()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
    );
    let mut state = dispatch::host_attempt_state::HostAttemptState {
        attempt_id: attempt_id.clone(),
        worktree: workdir.to_path_buf(),
        prompt: prompt_path.to_path_buf(),
        task: task_id.to_string(),
        phase: args.phase.as_str().to_string(),
        profile: "legacy_interactive".into(),
        run_id,
        host: host.to_string(),
        mode: "host-subagent".into(),
        spec_path: task_scope
            .join("host-attempts")
            .join(format!("{attempt_id}.spec.md")),
        spec_sha256: format!("{:x}", Sha256::digest(spec.as_bytes())),
        receipt_path: effective_receipt.map(|p| {
            let p = PathBuf::from(p);
            if p.is_absolute() {
                p
            } else {
                workdir.join(p)
            }
        }),
        log_path: task_scope
            .join("host-attempts")
            .join(format!("{attempt_id}.log.md")),
        snapshot_path: snapshot.backup.clone(),
        // Host session verification requires millisecond precision.
        started_at: gal_foundation::time::now_timestamp().replace('Z', ".000Z"),
        model: String::new(),
        effort: None,
        tier: None,
        tier_source: None,
        override_reason: None,
        policy_applied: false,
        prerequisite_refs: Vec::new(),
        failure_history: Vec::new(),
        observed_model: None,
        observed_effort: None,
        prompt_sha256: Some(snapshot.digest.clone()),
        receipt_sha256: None,
        log_sha256: None,
        snapshot_sha256: Some(snapshot.digest.clone()),
        worktree_status_sha256: None,
        worktree_diff_sha256: None,
    };
    let coordinator_path = task_scope_dir.parent().unwrap().join("coordinator.json");
    if coordinator_path.is_file() {
        let bytes = std::fs::read(&coordinator_path).map_err(|_| ExitCode::Error)?;
        let coordinator: pipeline::coordinator::CoordinatorState =
            serde_json::from_slice(&bytes).map_err(|_| ExitCode::Error)?;
        if coordinator.profile != pipeline::coordinator::ContinuationProfile::CodexStopV1
            || coordinator.pinned_mode.as_deref() != Some("host-subagent")
            || coordinator.pinned_host.as_deref() != Some(host)
            || coordinator.task_id != task_id
            || coordinator.phase != args.phase.as_str()
            || coordinator.has_pending_host_checkpoint()
        {
            eprintln!("gal pipeline: guarded host open custody mismatch: profile={:?}, mode={:?}, host={:?}, task={}, phase={}",
                coordinator.profile, coordinator.pinned_mode, coordinator.pinned_host,
                coordinator.task_id, coordinator.phase);
            return Err(ExitCode::Error);
        }
        let active = coordinator.active_attempt.as_ref().ok_or_else(|| {
            eprintln!("gal pipeline: guarded host open has no active attempt");
            ExitCode::Error
        })?;
        state.attempt_id = active.attempt_id.clone();
        state.profile = "codex_stop_v1".into();
        state.spec_path = task_scope
            .join("host-attempts")
            .join(format!("{}.spec.md", state.attempt_id));
        state.log_path = task_scope
            .join("host-attempts")
            .join(format!("{}.log.md", state.attempt_id));
    }
    args.pinned_mode = Some(dispatch::stage::DispatchMode::HostSubagent);
    args.pinned_host = Some(host.to_string());
    args.host_attempt = Some(dispatch::cli::HostAttemptOpenOptions {
        state: Some(state),
        prompt_snapshot: Some(snapshot.bytes.clone()),
        task_scope_dir: Some(task_scope),
        tier_override,
        override_reason: tier_reason,
        task_signals: Some(signals),
        ..Default::default()
    });
    Ok(())
}

/// Captures the usage and the account quota of one pipeline attempt into its run's usage store.
///
/// The run identity comes from the preflight pin. When no pin is readable, the capture is
/// skipped and the reason is printed, so no conversation or run is chosen automatically.
/// The quota of the return is stored for the attempt, and its reference or reason is printed.
/// A capture failure is printed on stderr and never changes the attempt outcome.
fn capture_pipeline_usage(
    workdir: &Path,
    prompt_path: Option<&Path>,
    attempt_id: &str,
    log_path: Option<&Path>,
    quota: Option<&DispatchQuota>,
) {
    let Some(prompt_path) = prompt_path else {
        return;
    };
    let Some(plan_id) = plan_slug_from_path(prompt_path) else {
        eprintln!(
            "gal pipeline: usage not captured for attempt {attempt_id}: no plan id in the prompt path"
        );
        return;
    };
    let run_id = match read_preflight_pin(workdir, prompt_path) {
        Ok(pin) => pin.run_id,
        Err(_) => {
            eprintln!(
                "gal pipeline: usage not captured for attempt {attempt_id}: no preflight run identity"
            );
            return;
        }
    };
    capture_run_usage(workdir, &run_id, &plan_id, attempt_id, log_path, quota);
}

/// Plan scope of an attempt that has no preflight pin. No plan is named.
const STANDALONE_PLAN_SCOPE: &str = "standalone";

/// Captures the usage and quota of one attempt on a path that may have no preflight pin.
///
/// A preflight pin for the prompt names the run, and the attempt is captured as a pipeline attempt.
/// Without a pin, which includes a `None` prompt, the attempt is captured under its standalone run.
/// That run id is printed on stderr so `gal usage report --run` can read the rows. A capture failure
/// is printed and never changes the caller's exit code or receipts.
pub(crate) fn capture_attempt_usage(
    workdir: &Path,
    prompt_path: Option<&Path>,
    attempt_id: &str,
    log_path: Option<&Path>,
    quota: Option<&DispatchQuota>,
) {
    let pinned = prompt_path.is_some_and(|prompt| read_preflight_pin(workdir, prompt).is_ok());
    if pinned {
        capture_pipeline_usage(workdir, prompt_path, attempt_id, log_path, quota);
        return;
    }
    let run_id = format!("standalone-{attempt_id}");
    eprintln!("gal usage: standalone run {run_id} for attempt {attempt_id}");
    capture_run_usage(
        workdir,
        &run_id,
        STANDALONE_PLAN_SCOPE,
        attempt_id,
        log_path,
        quota,
    );
}

/// Stores the bounded usage of one verified host child session under the attempt's run.
///
/// The run comes from the prompt's preflight pin, or from the standalone run of the attempt when
/// no pin is readable. The caller passes only a close that verified its session. The stored run
/// and count are printed on stderr. A failure is printed and never changes the close outcome.
pub(crate) fn capture_host_child_usage(
    workdir: &Path,
    prompt_path: &Path,
    attempt_id: &str,
    task_id: &str,
    phase: &str,
    started_at: &str,
    session_reference: &str,
) {
    let (run_id, plan_scope) = match read_preflight_pin(workdir, prompt_path) {
        Ok(pin) => match plan_slug_from_path(prompt_path) {
            Some(plan_id) => (pin.run_id, plan_id),
            None => {
                eprintln!(
                    "gal usage: usage not collected for attempt {attempt_id}: no plan id in the prompt path"
                );
                return;
            }
        },
        Err(_) => (
            format!("standalone-{attempt_id}"),
            STANDALONE_PLAN_SCOPE.to_owned(),
        ),
    };
    let common_dir = match super::usage::git_common_dir_in(workdir) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("gal usage: usage not collected for attempt {attempt_id}: {message}");
            return;
        }
    };
    let canonical_worktree = match canonical_worktree_text(workdir) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("gal usage: usage not collected for attempt {attempt_id}: {error}");
            return;
        }
    };
    let Some(home) = gal_foundation::paths::user_home() else {
        eprintln!("gal usage: usage not collected for attempt {attempt_id}: the home directory is unknown");
        return;
    };
    let session = super::usage::ChildSession {
        run_id: &run_id,
        plan_scope: &plan_scope,
        canonical_worktree: &canonical_worktree,
        attempt_id,
        task_id,
        phase,
        started_at,
        session_reference,
        home: &home,
    };
    match super::usage::collect_child_attempt(&common_dir, &session) {
        Ok(added) => eprintln!(
            "gal usage: host session usage for attempt {attempt_id} stored under run {run_id}: {added} new observation(s)"
        ),
        Err(message) => {
            eprintln!("gal usage: usage not collected for attempt {attempt_id}: {message}");
        }
    }
}

/// A token that names one dispatch attempt. It is unique within a process run.
pub(crate) fn new_attempt_token() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

/// Captures the log of one attempt, and then its quota, into the usage store of `run_id`.
///
/// A `None` quota belongs to an attempt that read no account, so only the log is captured. A
/// failure is printed and never returned.
fn capture_run_usage(
    workdir: &Path,
    run_id: &str,
    plan_id: &str,
    attempt_id: &str,
    log_path: Option<&Path>,
    quota: Option<&DispatchQuota>,
) {
    let common_dir = match super::usage::git_common_dir_in(workdir) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("gal usage: usage not captured for attempt {attempt_id}: {message}");
            return;
        }
    };
    if let Some(log_path) = log_path {
        if let Err(error) =
            super::usage_capture::capture_log(&common_dir, run_id, plan_id, attempt_id, log_path)
        {
            eprintln!("gal usage: usage capture failed for attempt {attempt_id}: {error}");
        }
    }
    if let Some(quota) = quota {
        capture_attempt_quota(workdir, &common_dir, run_id, plan_id, attempt_id, quota);
    }
}

/// Stores the quota of one pipeline return and prints its reference or its reason.
///
/// A missing worktree or store prints the reason and returns. The attempt outcome is unchanged
/// in every case.
fn capture_attempt_quota(
    workdir: &Path,
    common_dir: &Path,
    run_id: &str,
    plan_id: &str,
    attempt_id: &str,
    quota: &DispatchQuota,
) {
    let canonical_worktree = match canonical_worktree_text(workdir) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("gal usage: quota not recorded for attempt {attempt_id}: {error}");
            return;
        }
    };
    let identity =
        super::usage_capture::pipeline_run_identity(run_id, &canonical_worktree, plan_id);
    let store = match pipeline::usage::UsageStore::open(common_dir) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("gal usage: quota not recorded for attempt {attempt_id}: {error}");
            return;
        }
    };
    let snapshot = super::usage_capture::capture_quota(&store, &identity, Some(attempt_id), quota);
    eprintln!(
        "gal usage: account quota for attempt {attempt_id}: {}",
        super::usage_capture::quota_text(&snapshot, None)
    );
}

/// The canonical worktree root as text, without the Windows verbatim prefix.
fn canonical_worktree_text(workdir: &Path) -> Result<String, std::io::Error> {
    let resolved = std::fs::canonicalize(workdir)?
        .to_string_lossy()
        .into_owned();
    Ok(resolved
        .strip_prefix(r"\\?\")
        .unwrap_or(&resolved)
        .to_owned())
}

fn read_preflight_pin(
    workdir: &Path,
    prompt_path: &Path,
) -> Result<super::pipeline_preflight::PreflightPin, super::pipeline_preflight::PinError> {
    // read_pin resolves the canonical receipt relative to the process cwd. Keep that
    // fast path for the usual invocation and resolve against --workdir when the caller
    // dispatches from another directory.
    if std::env::current_dir().ok().as_deref() == Some(workdir) {
        return super::pipeline_preflight::read_pin(prompt_path);
    }
    let relative = resolve_receipt_path(Some(prompt_path), None, "preflight.receipt.md")
        .map_err(|_| super::pipeline_preflight::PinError::Unparseable)?;
    let receipt = if relative.is_absolute() {
        relative
    } else {
        workdir.join(relative)
    };
    let text = std::fs::read_to_string(receipt).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            super::pipeline_preflight::PinError::Missing
        } else {
            super::pipeline_preflight::PinError::Unparseable
        }
    })?;
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(&format!("{name}: ")))
    };
    let host = match field("host") {
        Some("none") => None,
        Some(host @ ("claude" | "codex")) => Some(host.to_string()),
        _ => return Err(super::pipeline_preflight::PinError::Unparseable),
    };
    let dispatch_mode = match field("dispatch_mode") {
        Some("routed") => super::pipeline_preflight::DispatchMode::Routed,
        Some("host-subagent") => super::pipeline_preflight::DispatchMode::HostSubagent,
        _ => return Err(super::pipeline_preflight::PinError::Unparseable),
    };
    let run_id = field("run_id")
        .filter(|value| {
            !value.is_empty()
                && value
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
        .ok_or(super::pipeline_preflight::PinError::Unparseable)?
        .to_string();
    if dispatch_mode == super::pipeline_preflight::DispatchMode::HostSubagent && host.is_none() {
        return Err(super::pipeline_preflight::PinError::Unparseable);
    }
    Ok(super::pipeline_preflight::PreflightPin {
        host,
        dispatch_mode,
        run_id,
    })
}

/// Inputs of one plan-level review dispatch, as parsed from `gal pipeline`.
struct FinalizeReviewRequest<'a> {
    input_path: &'a Path,
    input_kind: PipelineInput,
    workdir: &'a Path,
    task_override: Option<&'a str>,
    receipt_override: Option<&'a str>,
    fix_mode: bool,
    host_dispatch: bool,
    close_attempt: Option<&'a str>,
    host_session: Option<&'a str>,
}

/// The two files a plan-level review reads and writes, resolved from either input form.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanPair {
    prompt: PathBuf,
    source: PathBuf,
}

/// Resolve the paired execution prompt and source plan for a prompt or source-plan
/// input. Both files must exist: the review reads the source plan and writes the prompt.
fn resolve_plan_pair(input_path: &Path, kind: PipelineInput) -> Result<PlanPair, String> {
    let (prompt, source) = match kind {
        PipelineInput::Prompt => {
            let text = input_path.to_string_lossy().replace('\\', "/");
            let stem = text
                .strip_suffix(".prompt.md")
                .ok_or_else(|| format!("'{}' is not an execution prompt", input_path.display()))?;
            (
                input_path.to_path_buf(),
                PathBuf::from(format!("{stem}.md")),
            )
        }
        PipelineInput::SourcePlan => {
            let prompt = paired_prompt_path(input_path).ok_or_else(|| {
                format!(
                    "cannot derive a prompt path from '{}'",
                    input_path.display()
                )
            })?;
            (prompt, input_path.to_path_buf())
        }
        PipelineInput::RawSpec => {
            return Err("finalize-review takes an execution prompt or its paired source plan, not a raw spec".to_string());
        }
    };
    for (label, path) in [("execution prompt", &prompt), ("source plan", &source)] {
        if !path.is_file() {
            return Err(format!(
                "finalize-review needs the paired {label}, but '{}' does not exist",
                path.display()
            ));
        }
    }
    Ok(PlanPair { prompt, source })
}

/// Verify an existing coordinator execution binding without writing the coordinator.
/// No coordinator, or one that carries no binding, leaves nothing to verify. A
/// coordinator that cannot be read, names another plan, or binds another worktree or
/// executable stops the review before any executor starts. A review never rebinds.
fn verify_existing_coordinator_binding(workdir: &Path, scope: &str) -> Result<(), String> {
    let path = workdir
        .join(".dev")
        .join("pipeline")
        .join(scope)
        .join("coordinator.json");
    if !path.exists() {
        return Ok(());
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("coordinator unavailable: {e}"))?;
    let state: pipeline::coordinator::CoordinatorState =
        serde_json::from_slice(&bytes).map_err(|e| format!("coordinator malformed: {e}"))?;
    let Some(bound) = state.execution_binding.as_ref() else {
        return Ok(());
    };
    if state.plan_scope != scope {
        return Err(format!(
            "coordinator names plan scope '{}', not '{scope}'",
            state.plan_scope
        ));
    }
    let worktree = std::fs::canonicalize(super::pipeline_execution::resolve_worktree_root(workdir))
        .map_err(|e| format!("worktree unavailable: {e}"))?;
    let executable = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|e| format!("executable resolution failed: {e}"))?;
    let expected =
        super::pipeline_driver::coordinator_execution_binding(scope, &worktree, &executable)?;
    if *bound != expected {
        return Err("coordinator execution binding mismatch".to_string());
    }
    Ok(())
}

/// `git -C <workdir> <args>` stdout, trimmed, or `unknown`. The spec's git fields are
/// informational, but they must describe the workdir under review, not the process cwd.
fn git_capture_in(workdir: &Path, args: &[&str]) -> String {
    std::process::Command::new("git")
        .arg("-C")
        .arg(workdir)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Assemble the fresh-context review spec. Returns the spec markdown and the finalize
/// command source it was cut from. Every missing or malformed input is an error here,
/// before any lease or executor exists.
fn build_finalize_review_spec(
    pair: &PlanPair,
    workdir: &Path,
    scope: &str,
    receipt_rel: &str,
) -> Result<(String, ResolvedContract), String> {
    let plan_body = std::fs::read_to_string(&pair.source).map_err(|e| {
        format!(
            "failed to read source plan '{}': {e}",
            pair.source.display()
        )
    })?;
    let sources = pipeline::task_spec::extract_finalize_review_sources(&plan_body)
        .map_err(|e| format!("source plan '{}': {e}", pair.source.display()))?;
    let command = resolve_authoritative_finalize_command(workdir).map_err(|e| e.message())?;
    let project_md_path = workdir.join(".dev").join("project.md");
    let project_md = std::fs::read_to_string(&project_md_path).map_err(|e| {
        format!(
            "failed to read '{}' for the authoritative review command: {e}",
            project_md_path.display()
        )
    })?;
    let authoritative =
        super::finalize_check::authoritative_review_command(&project_md).map_err(|e| {
            format!(
                "authoritative review command from '{}': {e}",
                project_md_path.display()
            )
        })?;
    let source_rel = prompt_identity_relative_to_workdir(&pair.source, workdir)?;
    let slug = plan_slug_from_path(&pair.prompt).unwrap_or_else(|| scope.to_string());
    let generated = gal_foundation::time::now_timestamp();
    let branch = git_capture_in(workdir, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let head = git_capture_in(workdir, &["rev-parse", "--short", "HEAD"]);
    let contract_source = format!("{} ({} tier)", command.path.display(), command.source);
    let spec = pipeline::task_spec::assemble_finalize_review_spec(
        &pipeline::task_spec::FinalizeReviewSpecInput {
            plan_slug: &slug,
            source_plan_path: &source_rel,
            generated: &generated,
            git_branch: &branch,
            git_head: &head,
            sources: &sources,
            contract_body: &command.body,
            contract_source: &contract_source,
            authoritative_command: &authoritative,
            receipt_path: receipt_rel,
        },
    )
    .map_err(|e| format!("finalize-review spec assembly failed: {e}"))?;
    Ok((spec.markdown, command))
}

/// Place a delivered review into the prompt text. The candidate is rendered in memory,
/// judged by the same evaluator the hygiene check uses with the dispatched
/// independence rule, and returned only when it passes. The prompt file is untouched.
fn render_validated_finalize_review(
    prompt_bytes: &[u8],
    receipt_bytes: &[u8],
    repo_root: &Path,
    independence: super::finalize_check::ReviewIndependenceRule,
) -> Result<Vec<u8>, String> {
    let rendered =
        pipeline::task_spec::render_finalize_review_writeback(prompt_bytes, receipt_bytes)
            .map_err(|e| format!("rendering finalize review failed: {e}"))?;
    let candidate = std::str::from_utf8(&rendered)
        .map_err(|e| format!("rendered prompt is not valid UTF-8: {e}"))?;
    let verdict =
        super::finalize_check::evaluate_finalize_review_text(candidate, repo_root, independence);
    if verdict.state != super::finalize_check::CheckState::Pass {
        return Err(format!(
            "finalize review failed {}: {}",
            verdict.name, verdict.summary
        ));
    }
    Ok(rendered)
}

/// `gal pipeline <prompt|source-plan> --phase finalize-review`: dispatch the
/// plan-level Sequence 1 review to the AUDITOR route and place its receipt under
/// `## Review Results`. Exit code `NotWired` with `reason=no-routing` is the only allowed
/// degrade signal. Every other stop exits with an error or usage code, so an invalid
/// route is never mistaken for an absent one.
fn run_finalize_review(req: &FinalizeReviewRequest<'_>) -> ExitCode {
    let phase = FINALIZE_REVIEW_PHASE;
    if req.task_override.is_some() {
        eprintln!("gal pipeline: --phase finalize-review is plan-level and takes no --task");
        return ExitCode::Usage;
    }
    if req.fix_mode {
        eprintln!("gal pipeline: --fix does not apply to --phase finalize-review");
        return ExitCode::Usage;
    }
    if req.receipt_override.is_some() {
        eprintln!(
            "gal pipeline: --phase finalize-review writes its designated receipt only and takes no --receipt"
        );
        return ExitCode::Usage;
    }
    if req.input_kind == PipelineInput::RawSpec {
        eprintln!(
            "gal pipeline: --phase finalize-review takes an execution prompt or its paired source plan, not a raw spec"
        );
        return ExitCode::Usage;
    }
    let workdir = req.workdir;
    let pair = match resolve_plan_pair(req.input_path, req.input_kind) {
        Ok(pair) => pair,
        Err(message) => {
            eprintln!("gal pipeline: {message}");
            return ExitCode::Error;
        }
    };
    let scope = match resolve_scope(&pair.prompt) {
        Ok(scope) => scope,
        Err(message) => {
            eprintln!("gal pipeline: {message}");
            return ExitCode::Error;
        }
    };
    if let Err(message) = verify_existing_coordinator_binding(workdir, &scope) {
        eprintln!("gal pipeline: finalize-review stops before launch: {message}");
        return ExitCode::Error;
    }
    let receipt_rel = match resolve_receipt_path(
        Some(&pair.prompt),
        Some(phase),
        "finalize-review.receipt.md",
    ) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("gal pipeline: {message}");
            return ExitCode::Error;
        }
    };
    let receipt_text = receipt_rel.to_string_lossy().replace('\\', "/");
    let receipt_file = workdir.join(&receipt_rel);
    let (spec, command) = match build_finalize_review_spec(&pair, workdir, &scope, &receipt_text) {
        Ok(built) => built,
        Err(message) => {
            eprintln!("gal pipeline: {message}");
            return ExitCode::Error;
        }
    };

    let attempt_token = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let selected_route = dispatch::routing::load_selected_route_default("AUDITOR");
    if matches!(selected_route, dispatch::routing::SelectedRoute::Absent) {
        let pin = read_preflight_pin(workdir, &pair.prompt)
            .ok()
            .filter(|pin| {
                pin.dispatch_mode == super::pipeline_preflight::DispatchMode::HostSubagent
                    && pin.host.is_some()
            });
        let Some(pin) = pin else {
            eprintln!("gal pipeline: finalize-review reason=no-routing: the AUDITOR route is absent, so Sequence 1 may run in-process as DEGRADED_SAME_RUNTIME");
            return ExitCode::NotWired;
        };
        let host = pin.host.as_deref().expect("filtered host pin");
        let task_scope_dir = workdir.join(".dev/pipeline").join(&scope).join(phase);
        if let Some(attempt_id) = req.close_attempt {
            if workdir
                .join(".dev/pipeline")
                .join(&scope)
                .join("coordinator.json")
                .exists()
            {
                eprintln!(
                    "gal pipeline: legacy close cannot consume coordinator-owned host attempts"
                );
                return ExitCode::Error;
            }
            let mut placement = |bytes: &[u8]| -> Result<(), String> {
                let path = dispatch::host_attempt_state::HostAttemptState::manifest_path_for(
                    &task_scope_dir,
                    attempt_id,
                )
                .map_err(|error| error.to_string())?;
                let state = dispatch::host_attempt_state::HostAttemptState::load(&path, attempt_id)
                    .map_err(|error| error.to_string())?;
                let snapshot =
                    std::fs::read(&state.snapshot_path).map_err(|error| error.to_string())?;
                if Some(format!("{:x}", Sha256::digest(&snapshot))) != state.snapshot_sha256 {
                    return Err("persisted prompt snapshot digest mismatch".into());
                }
                let rendered = render_validated_finalize_review(
                    &snapshot,
                    bytes,
                    workdir,
                    super::finalize_check::ReviewIndependenceRule::MustBeHostSubagent,
                )?;
                atomic_prompt_write(&pair.prompt, &rendered).map_err(|error| error.to_string())
            };
            return close_legacy_host_attempt(
                workdir,
                &task_scope_dir,
                phase,
                phase,
                host,
                req.host_session.unwrap_or(""),
                attempt_id,
                &mut placement,
            );
        }
        if !req.host_dispatch {
            eprintln!("gal pipeline: host open requires --dispatch host-subagent and a host-subagent preflight pin");
            return ExitCode::Usage;
        }
        let Some(snapshot) = snapshot_prompt(workdir, &pair.prompt, phase, phase) else {
            eprintln!("gal pipeline: mandatory prompt backup snapshot could not be created");
            return ExitCode::Error;
        };
        let raw_args = vec![
            "--phase".into(),
            phase.into(),
            "--task".into(),
            phase.into(),
            "--workdir".into(),
            workdir.display().to_string(),
            "--receipt".into(),
            receipt_file.display().to_string(),
        ];
        let mut args = match dispatch::cli::parse_args(&raw_args) {
            Ok(args) => args,
            Err(error) => {
                eprintln!("gal pipeline: {error}");
                return ExitCode::Usage;
            }
        };
        if let Err(code) = prepare_legacy_host_open(
            &mut args,
            workdir,
            &task_scope_dir,
            &pair.prompt,
            phase,
            host,
            pin.run_id,
            &attempt_token,
            &snapshot,
            &spec,
            receipt_file.to_str(),
            None,
            None,
        ) {
            return code;
        }
        let outcome = dispatch::run::run_dispatch_with_context(&args, &spec, None);
        capture_attempt_usage(
            workdir,
            Some(&pair.prompt),
            &attempt_token,
            outcome.log_path.as_deref(),
            Some(&outcome.quota),
        );
        if outcome.exit_code == 0 {
            return ExitCode::Success;
        }
        eprintln!(
            "gal pipeline: host open failed: {}",
            outcome.reason.as_deref().unwrap_or("unknown")
        );
        return ExitCode::Error;
    }
    let mut prompt_lease = PipelinePromptLease(None);
    prompt_lease.0 = match dispatch::dispatch::acquire_prompt_writer_lease(
        workdir,
        &pair.prompt,
        Some(&receipt_file),
        &attempt_token,
    ) {
        Ok(lease) => Some(lease),
        Err(error) => {
            eprintln!("gal pipeline: {error}");
            return ExitCode::Error;
        }
    };
    let Some(prompt_snapshot) = snapshot_prompt(workdir, &pair.prompt, phase, phase) else {
        eprintln!(
            "gal pipeline: execution prompt snapshot is unavailable for '{}'; refusing to start a review that cannot be placed",
            pair.prompt.display()
        );
        return ExitCode::Error;
    };

    let raw_args = vec![
        "--phase".to_string(),
        phase.to_string(),
        "--task".to_string(),
        phase.to_string(),
        "--workdir".to_string(),
        workdir.display().to_string(),
        "--receipt".to_string(),
        receipt_file.display().to_string(),
    ];
    let mut args = match dispatch::cli::parse_args(&raw_args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("gal pipeline: {e}");
            return ExitCode::Usage;
        }
    };
    args.log_dir_override = match resolve_log_dir_override(workdir, Some(&pair.prompt), Some(phase))
    {
        Ok(path) => path,
        Err(error) => {
            eprintln!("gal pipeline: {error}");
            return ExitCode::Error;
        }
    };
    if let Some(dir) = &args.log_dir_override {
        let _ = std::fs::create_dir_all(dir);
    }
    args.contract_provenance = Some(ContractProvenance {
        path: command.path,
        source: command.source,
    });
    let attempt_context = AttemptContext {
        attempt_token: Some(attempt_token.clone()),
        canonical_prompt: Some(pair.prompt.clone()),
        ..AttemptContext::default()
    };

    let outcome = dispatch::run::run_dispatch_with_context(&args, &spec, Some(&attempt_context));
    capture_pipeline_usage(
        workdir,
        Some(&pair.prompt),
        &attempt_token,
        outcome.log_path.as_deref(),
        Some(&outcome.quota),
    );
    #[cfg(test)]
    PIPELINE_AFTER_DISPATCH_TEST_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
    // A run that never started a provider left nothing to clean up, so its lease is
    // released on return. Any other run without confirmed cleanup keeps the lease.
    let provider_never_started = outcome.evidence.provider_started == Some(false);
    if !provider_never_started
        && !matches!(
            outcome.evidence.cleanup,
            dispatch::dispatch::CleanupDisposition::Confirmed
        )
    {
        if let Some(lease) = prompt_lease.0.as_mut() {
            lease.retain();
        }
    }

    let log_dispatch_event = |kind: &str, level: pipeline::loop_log::LoopLevel, msg: String| {
        let event = pipeline::loop_log::LoopEvent {
            time: gal_foundation::time::now_timestamp(),
            task: phase.to_string(),
            phase: phase.to_string(),
            role: args.phase.role().to_string(),
            kind: kind.to_string(),
            level,
            msg,
            log_ptr: outcome.log_path.as_ref().map(|p| p.display().to_string()),
        };
        let dir = workdir
            .join(".dev")
            .join("pipeline")
            .join(gal_foundation::time::utc_date_compact());
        let _ = pipeline::loop_log::append_event(&dir, "loop-log", &event);
    };
    if let Some(state) = &outcome.terminal_state {
        let state_str = state.as_str();
        let (kind, level) = pipeline::loop_log::classify_dispatch(state_str);
        log_dispatch_event(
            kind,
            level,
            format!("dispatch {state_str} (exit {})", outcome.exit_code),
        );
    }
    let fail = |msg: String| -> ExitCode {
        log_semantic_failure(
            workdir,
            phase,
            phase,
            args.phase.role(),
            &msg,
            outcome.log_path.as_ref(),
        );
        eprintln!("gal pipeline: finalize-review stops: {msg}");
        ExitCode::Error
    };

    // Only an absent route may degrade. Every other degrade reason, including an
    // invalid route and an unavailable executor, stops the review.
    if outcome.exit_code == 2 {
        let reason = outcome.reason.as_deref().unwrap_or("unknown");
        if reason == "no-routing" {
            eprintln!(
                "gal pipeline: finalize-review reason=no-routing: the AUDITOR route is absent, so Sequence 1 may run in-process as DEGRADED_SAME_RUNTIME"
            );
            return ExitCode::NotWired;
        }
        return fail(format!(
            "dispatch did not start the review (reason={reason}); there is no same-runtime fallback"
        ));
    }
    let completed = outcome
        .terminal_state
        .as_ref()
        .is_some_and(|state| state.as_str() == "completed");
    if outcome.exit_code != 0 || !completed {
        let state = outcome
            .terminal_state
            .as_ref()
            .map_or("none", |state| state.as_str());
        return fail(format!(
            "dispatch ended in state '{state}' with exit {}",
            outcome.exit_code
        ));
    }

    // A completed attempt must carry its own evidence. Anything missing blocks
    // writeback, because a prompt edit made without it cannot be told from a forgery.
    if !matches!(
        outcome.evidence.cleanup,
        dispatch::dispatch::CleanupDisposition::Confirmed
    ) {
        return fail("attempt cleanup was not confirmed".to_string());
    }
    if outcome.evidence.attempt_id.as_deref() != Some(attempt_token.as_str()) {
        return fail("attempt identity evidence does not match this dispatch".to_string());
    }
    if outcome
        .evidence
        .provider_session
        .as_ref()
        .is_none_or(|session| session.session_id.trim().is_empty())
    {
        return fail("provider session evidence is missing".to_string());
    }
    let expected_log_dir = args
        .log_dir_override
        .as_ref()
        .and_then(|dir| std::fs::canonicalize(dir).ok());
    let actual_log_dir = outcome
        .log_path
        .as_ref()
        .and_then(|path| path.parent())
        .and_then(|dir| std::fs::canonicalize(dir).ok());
    if expected_log_dir.is_none() || expected_log_dir != actual_log_dir {
        return fail("attempt log is missing or outside the finalize-review scope".to_string());
    }

    let prompt_after = match std::fs::read(&pair.prompt) {
        Ok(bytes) => bytes,
        Err(e) => return fail(format!("execution prompt cannot be re-read: {e}")),
    };
    if let Some(msg) = prompt_mutation_msg(&prompt_snapshot, &prompt_after) {
        log_dispatch_event(
            "prompt-mutated",
            pipeline::loop_log::LoopLevel::Error,
            msg.clone(),
        );
        return fail(msg);
    }
    let Some(receipt_snapshot) = outcome.evidence.receipt_snapshot.as_ref() else {
        return fail(format!(
            "dispatch did not produce an integrity-checked receipt snapshot for '{}'",
            receipt_file.display()
        ));
    };
    let receipt_bytes = match attempt_receipt_bytes(receipt_snapshot, &receipt_file) {
        Ok(bytes) => bytes,
        Err(msg) => {
            // Keep the prompt lease so a later attempt cannot turn stale evidence
            // into a successful placement.
            if let Some(lease) = prompt_lease.0.as_mut() {
                lease.retain();
            }
            return fail(msg);
        }
    };
    let rendered = match render_validated_finalize_review(
        &prompt_snapshot.bytes,
        receipt_bytes,
        workdir,
        super::finalize_check::ReviewIndependenceRule::MustBeFull,
    ) {
        Ok(rendered) => rendered,
        Err(msg) => return fail(msg),
    };
    if let Err(e) = atomic_prompt_write(&pair.prompt, &rendered) {
        return fail(format!(
            "failed to atomically write prompt file '{}': {e}",
            pair.prompt.display()
        ));
    }

    if let Some(lease) = prompt_lease.0.as_mut() {
        if let Err(error) = lease.release_confirmed() {
            eprintln!("gal pipeline: prompt lease cleanup remains unconfirmed: {error}");
            return ExitCode::Error;
        }
    }
    println!(
        "gal pipeline: finalize-review placed '{}' in '{}' (independence: {}); finalize interprets the verdict",
        receipt_text,
        pair.prompt.display(),
        super::finalize_check::DISPATCHED_REVIEW_INDEPENDENCE
    );
    ExitCode::Success
}

/// The execution prompt as it stood immediately before `run_dispatch`: its
/// digest, plus the backup copy holding those exact bytes. A snapshot failure
/// must never block the dispatch it observes, so every fallible step degrades
/// to `None` rather than propagating.
struct PromptSnapshot {
    digest: String,
    backup: PathBuf,
    bytes: Vec<u8>,
}

/// Capture the prompt's pre-dispatch bytes under the gitignored
/// `.dev/pipeline/<slug>/<task>/` so a change made during the dispatch stays
/// recoverable. `None` when the prompt cannot be read or the copy cannot be
/// written — detection is best-effort and never fails the run.
fn snapshot_prompt(
    workdir: &Path,
    prompt_path: &Path,
    task_id: &str,
    phase: &str,
) -> Option<PromptSnapshot> {
    let bytes = std::fs::read(prompt_path).ok()?;
    let backup = workdir.join(
        resolve_receipt_path(
            Some(prompt_path),
            Some(task_id),
            format!("{task_id}-{phase}.prompt.bak"),
        )
        .ok()?,
    );
    if let Some(parent) = backup.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(&backup, &bytes).ok()?;
    Some(PromptSnapshot {
        digest: format!("{:x}", Sha256::digest(&bytes)),
        backup,
        bytes,
    })
}

struct PipelinePromptLease(Option<PromptWriterLease>);

impl Drop for PipelinePromptLease {
    fn drop(&mut self) {
        if let Some(lease) = self.0.as_mut() {
            if let Err(error) = lease.release_confirmed() {
                eprintln!("gal pipeline: prompt lease cleanup remains unconfirmed: {error}");
            }
        }
    }
}

/// Resolve the exact receipt bytes captured and integrity-checked by dispatch.
/// The path comparison is retained as an on-disk identity check, while the
/// returned payload always comes from the immutable attempt evidence.
fn attempt_receipt_bytes<'a>(
    snapshot: &'a dispatch::dispatch::ReceiptSnapshot,
    resolved_path: &Path,
) -> Result<&'a [u8], String> {
    let paths_match = match (
        std::fs::canonicalize(&snapshot.path),
        std::fs::canonicalize(resolved_path),
    ) {
        (Ok(snapshot), Ok(resolved)) => snapshot == resolved,
        _ => false,
    };
    if !paths_match {
        return Err(format!(
            "dispatch receipt snapshot path '{}' does not match resolved receipt '{}'",
            snapshot.path.display(),
            resolved_path.display()
        ));
    }
    let on_disk = std::fs::read(resolved_path).map_err(|error| {
        format!(
            "dispatch receipt '{}' cannot be revalidated before writeback: {error}",
            resolved_path.display()
        )
    })?;
    let on_disk_sha256 = format!("{:x}", Sha256::digest(&on_disk));
    if on_disk != snapshot.bytes || on_disk_sha256 != snapshot.sha256 {
        return Err(format!(
            "dispatch receipt '{}' changed after attempt evidence was captured",
            resolved_path.display()
        ));
    }
    Ok(&snapshot.bytes)
}

/// `Some(msg)` when the prompt changed between the snapshot and `after_bytes`.
/// Pure, so the mutation case is testable without running a dispatch.
fn prompt_mutation_msg(snap: &PromptSnapshot, after_bytes: &[u8]) -> Option<String> {
    let after = format!("{:x}", Sha256::digest(after_bytes));
    (after != snap.digest).then(|| {
        format!(
            "execution prompt changed during dispatch: {} -> {after}; pre-dispatch copy: {}",
            snap.digest,
            snap.backup.display()
        )
    })
}

fn atomic_prompt_write(prompt_path: &Path, rendered: &[u8]) -> std::io::Result<()> {
    let mut temp_name = prompt_path.as_os_str().to_os_string();
    temp_name.push(".tmp");
    let temp_path = PathBuf::from(temp_name);

    if let Err(error) = std::fs::write(&temp_path, rendered) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }

    if let Err(error) = std::fs::rename(&temp_path, prompt_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }

    Ok(())
}

fn log_semantic_failure(
    workdir: &Path,
    task_id: &str,
    phase: &str,
    role: &str,
    err_msg: &str,
    log_ptr: Option<&PathBuf>,
) {
    let now = gal_foundation::time::now_timestamp();
    let event = pipeline::loop_log::LoopEvent {
        time: now,
        task: task_id.to_string(),
        phase: phase.to_string(),
        role: role.to_string(),
        kind: "failure".to_string(),
        level: pipeline::loop_log::LoopLevel::Error,
        msg: format!("phase-writeback semantic failure: {err_msg}"),
        log_ptr: log_ptr.map(|p| p.display().to_string()),
    };
    let dir = workdir
        .join(".dev")
        .join("pipeline")
        .join(gal_foundation::time::utc_date_compact());
    let _ = pipeline::loop_log::append_event(&dir, "loop-log", &event);
}

/// `gal pipeline-log append --task <t> --phase <p> --kind <k> --level <l> --msg <m>`
/// — pipeline-internal: append one structured loop-log event. This is the
/// in-process append path the orchestrator uses for `gal pipeline` degraded
/// phases (the Rust dispatch path appends directly via `cmd_pipeline`). Writes to
/// `<cwd>/.dev/pipeline/<yyyymmdd>/loop-log.ndjson`. Not a public `/gal` command.
pub fn cmd_pipeline_log(args: &[String]) -> ExitCode {
    // args[0] is the "pipeline-log" subcommand token; the action follows.
    if args.get(1).map(String::as_str) != Some("append") {
        eprintln!(
            "gal pipeline-log: usage: gal pipeline-log append --task <t> --phase <p> --kind <k> --level <l> --msg <m>"
        );
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
        .join(gal_foundation::time::utc_date_compact());
    match pipeline::loop_log::append_event(&dir, "loop-log", &event) {
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
        .or_else(|| name.strip_suffix(".en.md"))
        .or_else(|| name.strip_suffix(".md"))?;
    if stem.is_empty() {
        None
    } else {
        Some(stem.to_string())
    }
}

pub(crate) fn resolve_scope(path: &Path) -> Result<String, String> {
    let slug = plan_slug_from_path(path)
        .ok_or_else(|| format!("cannot derive a plan scope from '{}'", path.display()))?;
    let scope = sanitize_plan_slug(&slug);
    if scope.starts_with('.') || (scope.len() == 8 && scope.chars().all(|c| c.is_ascii_digit())) {
        return Err(format!("refused plan scope '{scope}'"));
    }
    Ok(scope)
}

pub(crate) fn validate_task_component(task: &str) -> Result<(), String> {
    if task.is_empty() || task.chars().all(|c| c == '.') {
        return Err(format!("refused task component '{task}'"));
    }
    if !task
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(format!("refused task component '{task}'"));
    }
    Ok(())
}

/// Sanitize a plan slug into a filesystem-safe directory component: alphanumerics,
/// `.`, `_`, `-` pass through; everything else becomes `-`. Mirrors the executor-log
/// filename sanitizer in `dispatch::dispatch` so a plan-scoped log directory can never
/// escape `.dev/pipeline/` via path separators or other unsafe characters.
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
    // escape the intended `.dev/pipeline/<scope>/` confinement.
    if trimmed.is_empty() || trimmed.chars().all(|c| c == '.') {
        "unknown-plan".to_string()
    } else {
        trimmed.to_string()
    }
}

fn stable_fix_authority_fingerprint(inputs: &pipeline::task_spec::FixAuthorityInputs) -> String {
    let mut hasher = Sha256::new();
    for field in [
        inputs.task_id.as_bytes(),
        inputs.executor_phase.as_bytes(),
        inputs.handoff_heading.as_bytes(),
        inputs.task_goal.as_bytes(),
        inputs.problem.as_bytes(),
        inputs.next_human_step.as_bytes(),
        inputs.agent_contract.as_bytes(),
    ] {
        hasher.update(field.len().to_le_bytes());
        hasher.update(field);
    }
    hasher.update(inputs.affected_files.len().to_le_bytes());
    for path in &inputs.affected_files {
        hasher.update(path.len().to_le_bytes());
        hasher.update(path.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

/// Persist a fix authority fingerprint and report whether it is an exact replay.
/// Reading the previous file on every call makes the guard process-independent.
fn persist_fix_fingerprint(path: &Path, fingerprint: &str) -> std::io::Result<bool> {
    if std::fs::read_to_string(path)
        .ok()
        .is_some_and(|previous| previous.trim() == fingerprint)
    {
        return Ok(true);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{fingerprint}\n"))?;
    Ok(false)
}

/// Origin phase of a fix round, read from the ` / <PHASE>` suffix of its
/// `#### Retry Handoff — <task> / <PHASE>` heading.
fn handoff_origin_phase(heading: &str) -> Option<&str> {
    heading
        .trim()
        .rsplit_once(" / ")
        .map(|(_, phase)| phase.trim())
}

/// Verdict of the fix-round guard after a fix dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixRoundGuard {
    /// No guard error applies.
    Pass,
    /// A `TEST` or `AUDIT` fix round left its allowlisted files unchanged.
    FixRoundNoChange,
    /// An `IMPLEMENT` boundary repair changed allowlisted files.
    BoundaryRepairTouchedScope,
}

/// Decide the fix-round guard verdict. Only a `completed` dispatch is judged.
/// An `IMPLEMENT` origin is inverted: it fails when allowlisted files changed,
/// because a boundary repair that touched scope must not be committed silently.
/// Any other origin fails when allowlisted files stayed unchanged.
fn fix_round_guard(
    origin: &str,
    terminal_state: Option<&str>,
    before: &str,
    after: &str,
) -> FixRoundGuard {
    if terminal_state != Some("completed") {
        return FixRoundGuard::Pass;
    }
    let unchanged = before == after;
    match (origin == "IMPLEMENT", unchanged) {
        (true, false) => FixRoundGuard::BoundaryRepairTouchedScope,
        (true, true) => FixRoundGuard::Pass,
        (false, true) => FixRoundGuard::FixRoundNoChange,
        (false, false) => FixRoundGuard::Pass,
    }
}

fn fingerprint_affected_files(workdir: &Path, paths: &[String]) -> String {
    let mut hasher = Sha256::new();
    for path in paths {
        hasher.update(path.as_bytes());
        match std::fs::read(workdir.join(path)) {
            Ok(bytes) => {
                hasher.update([1]);
                hasher.update(bytes);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => hasher.update([0]),
            Err(error) => {
                hasher.update([2]);
                hasher.update(error.to_string().as_bytes());
            }
        }
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

/// Resolve the R1 plan/task-scoped log directory for `gal pipeline`: `Some(dir)` when
/// the pipeline input has a plan source, under `<workdir>/.dev/pipeline/<scope>/<task>/`.
/// With no task, return the scope root for scope-level evidence. `None` for raw/direct dispatch
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
    task: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    let Some(source_path) = plan_log_source else {
        return Ok(None);
    };
    let scope_root = workdir
        .join(".dev")
        .join("pipeline")
        .join(resolve_scope(source_path)?);
    match task {
        Some(task) => {
            validate_task_component(task)?;
            Ok(Some(scope_root.join(task)))
        }
        None => Ok(Some(scope_root)),
    }
}

/// Resolve a receipt path beneath the shared pipeline root.
/// Plan and prompt inputs share a sanitized scope key; raw/direct inputs (no plan
/// source) return `Err`, since a raw receipt path is built only by
/// `effective_receipt_path`, which has the date root a raw dispatch needs.
pub(crate) fn resolve_receipt_path(
    plan_source: Option<&Path>,
    task: Option<&str>,
    receipt_name: impl AsRef<Path>,
) -> Result<PathBuf, String> {
    let pipeline_root = PathBuf::from(dispatch::dispatch::PIPELINE_ROOT);
    let scoped_root = match plan_source {
        Some(source) => pipeline_root.join(resolve_scope(source)?),
        None => {
            return Err("a raw receipt path is built only by effective_receipt_path".to_string());
        }
    };
    match task {
        Some(task) => {
            validate_task_component(task)?;
            Ok(scoped_root.join(task).join(receipt_name.as_ref()))
        }
        None => Ok(scoped_root.join(receipt_name.as_ref())),
    }
}

/// Read an execution prompt and assemble the per-(task,phase) task spec, scoped to the
/// single task id (explicit `--task` wins, else the first `T-NN` task bullet in the
/// prompt). Returns `(spec_markdown, task_id)`. The spec is scoped to one task — the
/// goal block for `task_id` only, not the whole prompt.
type BuiltPromptSpec = (
    String,
    String,
    Option<String>,
    Vec<String>,
    Option<pipeline::task_spec::FixAuthorityInputs>,
);

fn build_spec_from_prompt(
    prompt_path: &Path,
    workdir: &Path,
    phase: &str,
    task_override: Option<&str>,
    explicit_receipt: Option<&str>,
    agent_contract_body: Option<&str>,
    fix_mode: bool,
) -> Result<BuiltPromptSpec, String> {
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
    let prompt_path_disp = prompt_identity_relative_to_workdir(prompt_path, workdir)?;
    let now = gal_foundation::time::now_timestamp();
    let branch = git_current_branch();
    let head = git_head_short();
    // Resolve the effective receipt (explicit override, else the deterministic
    // default for test/audit/implement) and embed it into the spec so the executor
    // is told to write the verifiable receipt the pipeline checks.
    let receipt = effective_receipt_path(phase, &task_id, explicit_receipt, Some(prompt_path), "")?;
    let fix_authority = if fix_mode {
        let contract = agent_contract_body.ok_or_else(|| {
            format!(
                "fix-mode task-spec assembly for prompt '{}' has no resolved agent contract",
                prompt_path.display()
            )
        })?;
        Some(
            pipeline::task_spec::extract_fix_authority_inputs(
                &prompt_body,
                &task_id,
                phase,
                contract,
            )
            .map_err(|error| {
                format!(
                    "task-spec assembly failed for prompt '{}', task {task_id}: {error}",
                    prompt_path.display()
                )
            })?,
        )
    } else {
        None
    };
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
        fix_mode,
    };
    let spec = pipeline::task_spec::assemble_task_spec(&input).map_err(|e| {
        format!(
            "task-spec assembly failed for prompt '{}', task {task_id}: {e}",
            prompt_path.display()
        )
    })?;
    let affected = pipeline::task_spec::extract_affected_file_paths(&prompt_body, &task_id);
    Ok((spec.markdown, task_id, receipt, affected, fix_authority))
}

/// Return the executor-visible prompt identity relative to the dispatch workdir.
/// Control-node paths remain absolute at the call sites that read receipts and logs.
fn prompt_identity_relative_to_workdir(
    prompt_path: &Path,
    workdir: &Path,
) -> Result<String, String> {
    let workdir = std::fs::canonicalize(workdir).map_err(|e| {
        format!(
            "failed to resolve dispatch workdir '{}': {e}",
            workdir.display()
        )
    })?;
    let prompt = std::fs::canonicalize(prompt_path).map_err(|e| {
        format!(
            "failed to resolve authoritative prompt '{}': {e}",
            prompt_path.display()
        )
    })?;
    let relative = prompt.strip_prefix(&workdir).map_err(|_| {
        format!(
            "authoritative prompt '{}' is outside dispatch workdir '{}'",
            prompt_path.display(),
            workdir.display()
        )
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

/// Resolve the effective receipt path for a (phase, task): an explicit `--receipt`
/// always wins; otherwise test/audit/implement get a deterministic per-(task,phase) default
/// beneath their plan scope so the executor writes a verifiable receipt and
/// `verify_receipt` confirms the phase ran (liveness signal).
/// For implement, this dispatch receipt records liveness, while the orchestrator's
/// 2f correctness gate receipt remains the authoritative correctness verdict.
/// Any other phase defaults to no receipt (`None`).
fn effective_receipt_path(
    phase: &str,
    task_id: &str,
    explicit: Option<&str>,
    plan_source: Option<&Path>,
    date: &str,
) -> Result<Option<String>, String> {
    if let Some(e) = explicit {
        return Ok(Some(e.to_string()));
    }
    let phase_lc = phase.to_lowercase();
    match phase_lc.as_str() {
        "test" | "audit" | "implement" => Ok(Some(
            match plan_source {
                Some(source) => resolve_receipt_path(
                    Some(source),
                    Some(task_id),
                    format!("{task_id}-{phase_lc}.receipt.md"),
                )?,
                None => PathBuf::from(dispatch::dispatch::PIPELINE_ROOT)
                    .join(date)
                    .join("test-direct")
                    .join(format!("{task_id}-{phase_lc}.receipt.md")),
            }
            .to_string_lossy()
            .replace('\\', "/"),
        )),
        _ => Ok(None),
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

/// Require test and audit dispatches to run against the implementation commit recorded
/// in the prompt's `## Status` section. Implement dispatches intentionally skip this gate.
fn validate_task_final_commit(status_body: &str, workdir: &Path) -> Result<(), String> {
    let status = status_body
        .split_once("## Status")
        .map(|(_, section)| section)
        .unwrap_or("");
    let recorded = status.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Task Final Commit:")
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "—")
            .map(str::to_string)
    });
    let Some(recorded) = recorded else {
        return Err("Task Final Commit is missing (expected a commit hash, not —)".to_string());
    };

    let object_exists = std::process::Command::new("git")
        .current_dir(workdir)
        .args(["cat-file", "-e", recorded.as_str()])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if !object_exists {
        return Err(format!(
            "Task Final Commit '{recorded}' is not a valid git object"
        ));
    }

    let head = std::process::Command::new("git")
        .current_dir(workdir)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "cannot determine git rev-parse HEAD for Task Final Commit".to_string())?;
    if recorded != head {
        return Err(format!(
            "Task Final Commit drift: recorded '{recorded}', HEAD is '{head}'"
        ));
    }
    Ok(())
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

/// `gal consult-script <role> [args...]` selects the in-context consult path.
pub(crate) fn cmd_consult_script(args: &[String]) -> ExitCode {
    let mut dispatch_args = vec!["dispatch-script".to_string(), "consult-script".to_string()];
    dispatch_args.extend(args.iter().skip(1).cloned());
    cmd_dispatch_script(&dispatch_args)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Task ids are composed at runtime, never written as digit literals in source
    // (a stale `T-<digits>` literal is just dead data; the naming gate enforces this).
    fn tid(n: u32) -> String {
        format!("T-{:03}", n)
    }

    #[test]
    fn pipeline_host_flags_require_close_session_pairing() {
        let base = ["pipeline", "unused.md", "--phase", "implement"];
        let mut close_only: Vec<String> = base.iter().map(|s| s.to_string()).collect();
        close_only.extend(["--close".to_string(), "attempt".to_string()]);
        assert_eq!(cmd_pipeline(&close_only), ExitCode::Usage);

        let mut session_only: Vec<String> = base.iter().map(|s| s.to_string()).collect();
        session_only.extend(["--session".to_string(), "session".to_string()]);
        assert_eq!(cmd_pipeline(&session_only), ExitCode::Usage);
    }

    #[test]
    fn pipeline_host_flags_reject_tier_options_on_close_and_missing_reason() {
        let base = ["pipeline", "unused.md", "--phase", "implement"];
        let mut close_with_tier: Vec<String> = base.iter().map(|s| s.to_string()).collect();
        close_with_tier.extend([
            "--close".to_string(),
            "attempt".to_string(),
            "--session".to_string(),
            "session".to_string(),
            "--tier".to_string(),
            "deep".to_string(),
            "--tier-reason".to_string(),
            "reason".to_string(),
        ]);
        assert_eq!(cmd_pipeline(&close_with_tier), ExitCode::Usage);

        let mut missing_reason: Vec<String> = base.iter().map(|s| s.to_string()).collect();
        missing_reason.extend([
            "--dispatch".to_string(),
            "host-subagent".to_string(),
            "--tier".to_string(),
            "deep".to_string(),
        ]);
        assert_eq!(cmd_pipeline(&missing_reason), ExitCode::Usage);
    }

    mod pipeline_host_flag_cases {
        use super::*;
        use std::process::Command;
        use tempfile::TempDir;

        const PLAN: &str = "host-plan";

        struct Fixture {
            _lock: std::sync::MutexGuard<'static, ()>,
            root: TempDir,
            prompt: PathBuf,
            saved_home: Option<std::ffi::OsString>,
            saved_userprofile: Option<std::ffi::OsString>,
        }

        impl Fixture {
            fn new(pin: Option<(&str, &str)>) -> Self {
                let lock = crate::commands::ENV_GUARD
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let root = TempDir::new().unwrap();
                let home = root.path().join("home");
                std::fs::create_dir_all(&home).unwrap();
                let saved_home = std::env::var_os("HOME");
                let saved_userprofile = std::env::var_os("USERPROFILE");
                std::env::set_var("HOME", &home);
                std::env::set_var("USERPROFILE", &home);
                let prompt = root.path().join(format!(".dev/plans/{PLAN}.prompt.md"));
                std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
                std::fs::write(
                    &prompt,
                    format!(
                        "# Plan\n\n## Tasks\n\n- [ ] {} — Implement host dispatch.\n",
                        tid(1)
                    ),
                )
                .unwrap();
                if let Some((host, mode)) = pin {
                    let receipt = root
                        .path()
                        .join(format!(".dev/pipeline/{PLAN}/preflight.receipt.md"));
                    std::fs::create_dir_all(receipt.parent().unwrap()).unwrap();
                    std::fs::write(
                        receipt,
                        format!("# pipeline-preflight receipt\nrun_id: run-1\nhost: {host}\ndispatch_mode: {mode}\n"),
                    )
                    .unwrap();
                }
                let git = |args: &[&str]| {
                    let output = Command::new("git")
                        .current_dir(root.path())
                        .args(args)
                        .output()
                        .unwrap();
                    assert!(
                        output.status.success(),
                        "git {args:?}: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                };
                git(&["init", "-q"]);
                git(&["config", "user.email", "test@example.invalid"]);
                git(&["config", "user.name", "test"]);
                git(&["config", "commit.gpgsign", "false"]);
                git(&["add", "."]);
                git(&["commit", "-q", "-m", "fixture"]);
                Self {
                    _lock: lock,
                    root,
                    prompt,
                    saved_home,
                    saved_userprofile,
                }
            }

            fn run(&self, extra: &[&str]) -> ExitCode {
                let mut args = vec![
                    "pipeline".to_string(),
                    self.prompt.display().to_string(),
                    "--phase".to_string(),
                    "implement".to_string(),
                    "--workdir".to_string(),
                    self.root.path().display().to_string(),
                ];
                args.extend(extra.iter().map(|value| value.to_string()));
                cmd_pipeline(&args)
            }

            fn task_scope(&self, task: &str) -> PathBuf {
                self.root
                    .path()
                    .join(format!(".dev/pipeline/{PLAN}/{task}"))
            }

            fn state(
                &self,
                attempt: &str,
                task: &str,
                phase: &str,
                host: &str,
                profile: &str,
            ) -> dispatch::host_attempt_state::HostAttemptState {
                dispatch::host_attempt_state::HostAttemptState {
                    attempt_id: attempt.into(),
                    worktree: self.root.path().to_path_buf(),
                    prompt: self.prompt.clone(),
                    task: task.into(),
                    phase: phase.into(),
                    profile: profile.into(),
                    run_id: "run-1".into(),
                    host: host.into(),
                    mode: "host-subagent".into(),
                    spec_path: self.root.path().join("spec.md"),
                    spec_sha256: "0".repeat(64),
                    receipt_path: Some(self.root.path().join("receipt.md")),
                    log_path: self.root.path().join("attempt.log"),
                    snapshot_path: self.root.path().join("snapshot.bak"),
                    started_at: "now".into(),
                    model: String::new(),
                    effort: None,
                    tier: None,
                    tier_source: None,
                    override_reason: None,
                    policy_applied: false,
                    prerequisite_refs: vec![],
                    failure_history: vec![],
                    observed_model: None,
                    observed_effort: None,
                    prompt_sha256: None,
                    receipt_sha256: None,
                    log_sha256: None,
                    snapshot_sha256: None,
                    worktree_status_sha256: None,
                    worktree_diff_sha256: None,
                }
            }
        }

        impl Drop for Fixture {
            fn drop(&mut self) {
                match self.saved_home.take() {
                    Some(value) => std::env::set_var("HOME", value),
                    None => std::env::remove_var("HOME"),
                }
                match self.saved_userprofile.take() {
                    Some(value) => std::env::set_var("USERPROFILE", value),
                    None => std::env::remove_var("USERPROFILE"),
                }
            }
        }

        #[test]
        fn legacy_host_open_started_at_accepts_session_epoch_conversion() {
            let f = Fixture::new(Some(("codex", "host-subagent")));
            assert_eq!(f.run(&["--dispatch", "host-subagent"]), ExitCode::Success);
            let manifests = f.task_scope(&tid(1)).join("host-attempts");
            let manifest_path = std::fs::read_dir(manifests)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| path.extension().is_some_and(|ext| ext == "json"))
                .expect("host open must publish an attempt manifest");
            let attempt_id = manifest_path.file_stem().unwrap().to_str().unwrap();
            let manifest =
                dispatch::host_attempt_state::HostAttemptState::load(&manifest_path, attempt_id)
                    .unwrap();
            assert_eq!(manifest.phase, "implement");
            // Codex rejects a spawn_agent task_name with any other character.
            assert!(
                manifest
                    .attempt_id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                "attempt ID must be a valid native subagent name: {}",
                manifest.attempt_id
            );
            assert_eq!(manifest.started_at.len(), 24);
            assert!(
                dispatch::host_session::epoch_seconds(&manifest.started_at).is_ok(),
                "host session verification must accept started_at: {}",
                manifest.started_at
            );
        }

        #[test]
        fn legacy_host_cli_close_completes_after_cli_open() {
            let f = Fixture::new(Some(("claude", "host-subagent")));
            assert_eq!(f.run(&["--dispatch", "host-subagent"]), ExitCode::Success);
            let manifests = f.task_scope(&tid(1)).join("host-attempts");
            let manifest_path = std::fs::read_dir(manifests)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| path.extension().is_some_and(|ext| ext == "json"))
                .expect("host open must publish an attempt manifest");
            let attempt_id = manifest_path.file_stem().unwrap().to_str().unwrap();
            let state =
                dispatch::host_attempt_state::HostAttemptState::load(&manifest_path, attempt_id)
                    .unwrap();

            let sessions = f
                .root
                .path()
                .join("home/.claude/projects/demo/session/subagents");
            std::fs::create_dir_all(&sessions).unwrap();
            let user = serde_json::json!({"type":"user", "cwd":f.root.path(),
                "timestamp":state.started_at, "message":{"content":state.attempt_id}});
            let assistant = serde_json::json!({"type":"assistant", "message":{
                "model":state.model, "content":[{"type":"tool_use", "name":"Write"}]}});
            std::fs::write(
                sessions.join("agent-implementer.jsonl"),
                format!("{user}\n{assistant}\n"),
            )
            .unwrap();
            std::fs::write(
                sessions.join("agent-implementer.meta.json"),
                r#"{"agentType":"general-purpose"}"#,
            )
            .unwrap();
            std::fs::write(f.root.path().join("change.md"), "changed\n").unwrap();
            let receipt = state
                .receipt_path
                .clone()
                .expect("open records a receipt path");
            std::fs::create_dir_all(receipt.parent().unwrap()).unwrap();
            std::fs::write(&receipt, "PASS: implemented\n").unwrap();

            // Close runs through the same CLI entry as open. Open still holds the prompt
            // writer lease, so close must reattach it rather than acquire a new one.
            assert_eq!(
                f.run(&[
                    "--close",
                    &state.attempt_id,
                    "--session",
                    "claude:implementer"
                ]),
                ExitCode::Success
            );
            assert!(std::fs::read_to_string(&state.log_path)
                .unwrap()
                .contains("terminal_state:  completed"));

            // The completed host implement log lives under `host-attempts/`. A host test
            // open must accept it as the implement prerequisite. The orchestrator commits
            // the implement change and records it before the test phase opens.
            let git = |args: &[&str]| {
                let output = Command::new("git")
                    .current_dir(f.root.path())
                    .args(args)
                    .output()
                    .unwrap();
                assert!(output.status.success(), "git {args:?}");
                String::from_utf8(output.stdout).unwrap()
            };
            git(&["add", "change.md"]);
            git(&["commit", "-q", "-m", "implement"]);
            let head = git(&["rev-parse", "HEAD"]);
            let mut prompt = std::fs::read_to_string(&f.prompt).unwrap();
            prompt.push_str(&format!(
                "\n## Status\n\nTask Final Commit: {}\n",
                head.trim()
            ));
            std::fs::write(&f.prompt, prompt).unwrap();
            let test_open = cmd_pipeline(&[
                "pipeline".to_string(),
                f.prompt.display().to_string(),
                "--phase".to_string(),
                "test".to_string(),
                "--workdir".to_string(),
                f.root.path().display().to_string(),
                "--dispatch".to_string(),
                "host-subagent".to_string(),
            ]);
            assert_eq!(test_open, ExitCode::Success);
            let test_manifest = std::fs::read_dir(f.task_scope(&tid(1)).join("host-attempts"))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
                .map(|path| {
                    let id = path.file_stem().unwrap().to_str().unwrap().to_string();
                    dispatch::host_attempt_state::HostAttemptState::load(&path, &id).unwrap()
                })
                .find(|manifest| manifest.phase == "test")
                .expect("host test open must publish a test manifest");
            assert_eq!(test_manifest.tier.as_deref(), Some("light"));
        }

        #[test]
        fn pipeline_host_flags_open_requires_canonical_preflight_receipt() {
            let f = Fixture::new(None);
            assert_eq!(f.run(&["--dispatch", "host-subagent"]), ExitCode::Usage);
        }

        #[test]
        fn pipeline_host_flags_open_rejects_routed_pin() {
            let f = Fixture::new(Some(("codex", "routed")));
            assert_eq!(f.run(&["--dispatch", "host-subagent"]), ExitCode::Usage);
        }

        #[test]
        fn pipeline_host_flags_open_requires_prompt_snapshot() {
            let f = Fixture::new(Some(("codex", "host-subagent")));
            let backup = f
                .task_scope(&tid(1))
                .join(format!("{0}-implement.prompt.bak", tid(1)));
            std::fs::create_dir_all(&backup).unwrap();
            assert_eq!(f.run(&["--dispatch", "host-subagent"]), ExitCode::Error);
        }

        #[test]
        fn pipeline_host_flags_open_persists_custom_receipt_for_close() {
            let f = Fixture::new(Some(("codex", "host-subagent")));
            let receipt = f.root.path().join("custom/receipt.md");
            let expected = receipt.clone();
            assert_eq!(
                f.run(&[
                    "--dispatch",
                    "host-subagent",
                    "--receipt",
                    receipt.to_str().unwrap()
                ]),
                ExitCode::Success
            );
            let manifests = f.task_scope(&tid(1)).join("host-attempts");
            let manifest = std::fs::read_dir(manifests)
                .unwrap()
                .flatten()
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
                .map(|entry| {
                    dispatch::host_attempt_state::HostAttemptState::load(
                        &entry.path(),
                        entry
                            .file_name()
                            .to_str()
                            .unwrap()
                            .trim_end_matches(".json"),
                    )
                    .unwrap()
                })
                .next()
                .unwrap();
            assert_eq!(manifest.receipt_path, Some(expected));
            // The close command deliberately omits --receipt. It reaches the manifest-backed
            // close path and rejects only because no host session evidence was supplied.
            assert_eq!(
                f.run(&["--close", &manifest.attempt_id, "--session", "session"]),
                ExitCode::Error
            );
        }

        #[test]
        fn pipeline_host_flags_close_rejects_task_phase_and_host_mismatch_without_log_write() {
            for (override_args, pin_host, state_task, state_phase, state_host) in [
                (
                    vec!["--task", "task-b"],
                    "codex",
                    "task-a",
                    "implement",
                    "codex",
                ),
                (
                    vec!["--phase", "test"],
                    "codex",
                    "task-a",
                    "implement",
                    "codex",
                ),
                (vec![], "codex", "task-a", "implement", "claude"),
            ] {
                let f = Fixture::new(Some((pin_host, "host-subagent")));
                let mut state = f.state(
                    "attempt-one",
                    state_task,
                    state_phase,
                    state_host,
                    "legacy_interactive",
                );
                let log = f.root.path().join("attempt.log");
                std::fs::write(&log, b"unchanged log\n").unwrap();
                state.log_path = log.clone();
                state.write_once_at(&f.task_scope(state_task)).unwrap();
                let before = std::fs::read(&log).unwrap();
                let mut extra = override_args;
                extra.extend(["--close", "attempt-one", "--session", "session"]);
                assert_ne!(f.run(&extra), ExitCode::Success);
                assert_eq!(std::fs::read(log).unwrap(), before);
            }
        }

        #[test]
        fn pipeline_host_flags_close_rejects_nonlegacy_profile_without_log_write() {
            let f = Fixture::new(Some(("codex", "host-subagent")));
            let mut state = f.state("attempt-one", &tid(1), "implement", "codex", "coordinator");
            let log = f.root.path().join("attempt.log");
            std::fs::write(&log, b"unchanged log\n").unwrap();
            state.log_path = log.clone();
            state.write_once_at(&f.task_scope(&tid(1))).unwrap();
            let before = std::fs::read(&log).unwrap();
            assert_eq!(
                f.run(&["--close", "attempt-one", "--session", "session"]),
                ExitCode::Usage
            );
            assert_eq!(std::fs::read(log).unwrap(), before);
        }

        #[test]
        fn pipeline_host_flags_each_open_uses_unique_spec_and_log_paths() {
            let f = Fixture::new(Some(("codex", "host-subagent")));
            let first_receipt = f.root.path().join("receipts/first.md");
            let second_receipt = f.root.path().join("receipts/second.md");
            assert_eq!(
                f.run(&[
                    "--dispatch",
                    "host-subagent",
                    "--receipt",
                    first_receipt.to_str().unwrap()
                ]),
                ExitCode::Success
            );
            // Model the first attempt reaching its terminal cleanup before opening
            // another attempt for the same task. Its immutable manifest remains.
            let lock_root = f.root.path().join(".dev/pipeline/.locks");
            if lock_root.exists() {
                for entry in std::fs::read_dir(&lock_root).unwrap().flatten() {
                    if entry.path().is_dir() {
                        for lock in std::fs::read_dir(entry.path()).unwrap().flatten() {
                            let _ = std::fs::remove_file(lock.path());
                        }
                    } else {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
            assert_eq!(
                f.run(&[
                    "--dispatch",
                    "host-subagent",
                    "--receipt",
                    second_receipt.to_str().unwrap()
                ]),
                ExitCode::Success
            );
            let manifests = f.task_scope(&tid(1)).join("host-attempts");
            let mut specs = Vec::new();
            let mut logs = Vec::new();
            for entry in std::fs::read_dir(manifests).unwrap().flatten() {
                if entry.path().extension().is_some_and(|ext| ext == "json") {
                    let attempt = entry
                        .file_name()
                        .to_string_lossy()
                        .trim_end_matches(".json")
                        .to_string();
                    let state = dispatch::host_attempt_state::HostAttemptState::load(
                        &entry.path(),
                        &attempt,
                    )
                    .unwrap();
                    specs.push(state.spec_path);
                    logs.push(state.log_path);
                }
            }
            assert_eq!(specs.len(), 2);
            assert_ne!(specs[0], specs[1]);
            assert_ne!(logs[0], logs[1]);
        }

        #[test]
        fn pipeline_tier_flags_deep_override_is_recorded() {
            let f = Fixture::new(Some(("codex", "host-subagent")));
            assert_eq!(
                f.run(&[
                    "--dispatch",
                    "host-subagent",
                    "--tier",
                    "deep",
                    "--tier-reason",
                    "complex implementation",
                ]),
                ExitCode::Success
            );
            let manifests = f.task_scope(&tid(1)).join("host-attempts");
            let state = std::fs::read_dir(manifests)
                .unwrap()
                .flatten()
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
                .map(|entry| {
                    dispatch::host_attempt_state::HostAttemptState::load(
                        &entry.path(),
                        entry
                            .file_name()
                            .to_str()
                            .unwrap()
                            .trim_end_matches(".json"),
                    )
                    .unwrap()
                })
                .next()
                .unwrap();
            assert_eq!(state.tier.as_deref(), Some("deep"));
            assert_eq!(
                state.override_reason.as_deref(),
                Some("complex implementation")
            );
        }
    }

    mod pipeline_tier_evidence_cases {
        use super::*;

        fn signals(phase: dispatch::stage::Phase) -> dispatch::model_tier::TaskSignals {
            dispatch::model_tier::TaskSignals {
                phase,
                files: Vec::new(),
                crate_roots: Vec::new(),
                has_protected_path: false,
                dependency_count: 0,
                prior_failed_attempts: 0,
                implement: None,
                test: None,
            }
        }

        fn log(
            dir: &Path,
            file: &str,
            timestamp: &str,
            phase: &str,
            terminal: &str,
            executor: &str,
            tier: &str,
        ) {
            std::fs::write(
                dir.join(file),
                format!(
                    "GAL-DISPATCH-LOG v1\nattempt_id: fixture-{timestamp}\ntimestamp_start: {timestamp}\ntimestamp_end: {timestamp}\nexecutor: {executor}\nphase: {phase}\ntask_id: task-a\nterminal_state: {terminal}\ntier: {tier}\n"
                ),
            )
            .unwrap();
        }

        #[test]
        fn pipeline_tier_flags_accepts_current_iso_logs_and_preserves_mode_and_tier() {
            let tmp = tempfile::TempDir::new().unwrap();
            log(
                tmp.path(),
                "1791527211-task-a-implement-host-subagent.log.md",
                "2026-10-09T06:26:50.274Z",
                "implement",
                "completed",
                "host-subagent",
                "standard",
            );
            let (implement, test, failures) = accepted_pipeline_evidence(
                tmp.path(),
                "1791527210-run",
                "task-a",
                dispatch::stage::Phase::Test,
            )
            .unwrap();
            assert_eq!(test, None);
            assert_eq!(failures, 0);
            let implement = implement.unwrap();
            assert_eq!(implement.mode, "host-subagent");
            assert_eq!(
                implement.tier,
                Some(dispatch::model_tier::ModelTier::Standard)
            );
        }

        #[test]
        fn pipeline_tier_flags_ignores_non_attempt_files_in_task_scope() {
            let tmp = tempfile::TempDir::new().unwrap();
            log(
                tmp.path(),
                "1791527211-task-a-implement-agy.log",
                "epoch+1791527211s",
                "implement",
                "completed",
                "agy",
                "deep",
            );
            std::fs::write(
                tmp.path().join("preflight.receipt.md"),
                "run_id: 1791527210-run\ntask_id: task-a\nphase: implement\nterminal_state: completed\nexecutor: agy\ntier: light\n",
            )
            .unwrap();
            std::fs::write(
                tmp.path().join("task-a-implement.prompt.bak"),
                "run_id: 1791527210-run\ntask_id: task-a\nphase: implement\nterminal_state: completed\nexecutor: agy\ntier: light\n",
            )
            .unwrap();

            let (implement, _, failures) = accepted_pipeline_evidence(
                tmp.path(),
                "1791527210-run",
                "task-a",
                dispatch::stage::Phase::Test,
            )
            .unwrap();
            assert_eq!(failures, 0);
            assert_eq!(
                implement.unwrap().tier,
                Some(dispatch::model_tier::ModelTier::Deep)
            );
        }

        #[test]
        fn pipeline_tier_flags_ignores_old_logs_and_counts_current_failures() {
            let tmp = tempfile::TempDir::new().unwrap();
            log(
                tmp.path(),
                "100-task-a-implement-agy.log",
                "epoch+100s",
                "implement",
                "completed",
                "agy",
                "deep",
            );
            log(
                tmp.path(),
                "1791527211-task-a-implement-agy.log",
                "epoch+1791527211s",
                "implement",
                "completed",
                "agy",
                "light",
            );
            log(
                tmp.path(),
                "1791527212-task-a-test-agy.log",
                "epoch+1791527212s",
                "test",
                "failed",
                "agy",
                "light",
            );
            let (implement, _, failures) = accepted_pipeline_evidence(
                tmp.path(),
                "1791527210-run",
                "task-a",
                dispatch::stage::Phase::Test,
            )
            .unwrap();
            assert_eq!(
                implement.unwrap().tier,
                Some(dispatch::model_tier::ModelTier::Light)
            );
            assert_eq!(failures, 1);
        }

        #[test]
        fn pipeline_tier_flags_rejects_ambiguous_latest_implement_logs() {
            let tmp = tempfile::TempDir::new().unwrap();
            log(
                tmp.path(),
                "1791527211-task-a-implement-agy.log",
                "epoch+1791527211s",
                "implement",
                "completed",
                "agy",
                "light",
            );
            log(
                tmp.path(),
                "1791527211-task-a-implement-host-subagent.log.md",
                "2026-10-09T06:26:51.000Z",
                "implement",
                "completed",
                "host-subagent",
                "standard",
            );
            let error = accepted_pipeline_evidence(
                tmp.path(),
                "1791527210-run",
                "task-a",
                dispatch::stage::Phase::Test,
            )
            .unwrap_err();
            assert!(error.contains("ambiguous latest implement"), "{error}");
        }

        #[test]
        fn pipeline_tier_flags_markdown_implement_selects_light() {
            let mut task_signals = signals(dispatch::stage::Phase::Implement);
            task_signals.files = vec!["README.md".into()];
            let decision = dispatch::model_tier::compute_tier(&task_signals).unwrap();
            assert_eq!(decision.tier, dispatch::model_tier::ModelTier::Light);
        }

        #[test]
        fn pipeline_tier_flags_host_standard_implement_selects_light_for_test() {
            let tmp = tempfile::TempDir::new().unwrap();
            log(
                tmp.path(),
                "1791527211-task-a-implement-host-subagent.log.md",
                "epoch+1791527211s",
                "implement",
                "completed",
                "host-subagent",
                "standard",
            );
            let (implement, _, _) = accepted_pipeline_evidence(
                tmp.path(),
                "1791527210-run",
                "task-a",
                dispatch::stage::Phase::Test,
            )
            .unwrap();
            let mut task_signals = signals(dispatch::stage::Phase::Test);
            task_signals.implement = implement;
            assert_eq!(
                dispatch::model_tier::compute_tier(&task_signals)
                    .unwrap()
                    .tier,
                dispatch::model_tier::ModelTier::Light
            );
        }

        #[test]
        fn pipeline_tier_flags_tierless_routed_implement_selects_standard_for_test() {
            let tmp = tempfile::TempDir::new().unwrap();
            log(
                tmp.path(),
                "1791527211-task-a-implement-agy.log",
                "epoch+1791527211s",
                "implement",
                "completed",
                "agy",
                "none",
            );
            let (implement, _, _) = accepted_pipeline_evidence(
                tmp.path(),
                "1791527210-run",
                "task-a",
                dispatch::stage::Phase::Test,
            )
            .unwrap();
            let mut task_signals = signals(dispatch::stage::Phase::Test);
            task_signals.implement = implement;
            assert_eq!(
                dispatch::model_tier::compute_tier(&task_signals)
                    .unwrap()
                    .tier,
                dispatch::model_tier::ModelTier::Standard
            );
        }

        #[test]
        fn pipeline_tier_flags_stale_test_log_cannot_satisfy_audit() {
            let tmp = tempfile::TempDir::new().unwrap();
            log(
                tmp.path(),
                "1791527211-task-a-implement-agy.log",
                "epoch+1791527211s",
                "implement",
                "completed",
                "agy",
                "standard",
            );
            log(
                tmp.path(),
                "100-task-a-test-agy.log",
                "epoch+100s",
                "test",
                "completed",
                "agy",
                "light",
            );
            assert!(accepted_pipeline_evidence(
                tmp.path(),
                "1791527210-run",
                "task-a",
                dispatch::stage::Phase::Audit,
            )
            .is_err());
        }

        #[test]
        fn pipeline_tier_flags_current_failure_raises_computed_tier_once() {
            let tmp = tempfile::TempDir::new().unwrap();
            log(
                tmp.path(),
                "1791527211-task-a-implement-agy.log",
                "epoch+1791527211s",
                "implement",
                "completed",
                "agy",
                "standard",
            );
            log(
                tmp.path(),
                "1791527212-task-a-test-agy.log",
                "epoch+1791527212s",
                "test",
                "failed",
                "agy",
                "light",
            );
            let (implement, _, failures) = accepted_pipeline_evidence(
                tmp.path(),
                "1791527210-run",
                "task-a",
                dispatch::stage::Phase::Test,
            )
            .unwrap();
            let mut task_signals = signals(dispatch::stage::Phase::Test);
            task_signals.implement = implement;
            task_signals.prior_failed_attempts = failures;
            let decision = dispatch::model_tier::compute_tier(&task_signals).unwrap();
            assert_eq!(decision.tier, dispatch::model_tier::ModelTier::Standard);
            assert_eq!(decision.source, dispatch::model_tier::TierSource::Retry);
        }
    }

    mod task_final_commit {
        use super::*;

        #[test]
        fn rejects_missing_commit() {
            let error =
                validate_task_final_commit("## Status\n\nTask Final Commit: —\n", Path::new("."))
                    .unwrap_err();

            assert!(error.contains("Task Final Commit is missing"), "{error}");
        }

        #[test]
        fn rejects_commit_that_drifts_from_head() {
            let tmp = tempfile::TempDir::new().unwrap();
            let git = |args: &[&str]| {
                let output = std::process::Command::new("git")
                    .current_dir(tmp.path())
                    .args(args)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "git {args:?}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            };

            git(&["init"]);
            git(&["config", "user.email", "test@example.invalid"]);
            git(&["config", "user.name", "test"]);
            git(&["config", "commit.gpgsign", "false"]);
            std::fs::write(tmp.path().join("file"), "one").unwrap();
            git(&["add", "file"]);
            git(&["commit", "-m", "one"]);
            let recorded = git(&["rev-parse", "HEAD"]);
            std::fs::write(tmp.path().join("file"), "two").unwrap();
            git(&["add", "file"]);
            git(&["commit", "-m", "two"]);

            let status = format!("## Status\n\nTask Final Commit: {recorded}\n");
            let error = validate_task_final_commit(&status, tmp.path()).unwrap_err();

            assert!(error.contains(&recorded), "{error}");
            assert!(error.contains("HEAD is"), "{error}");
        }
    }

    mod atomic_prompt_write {
        use super::*;

        #[test]
        fn writes_rendered_bytes_and_removes_temp_file() {
            let tmp = tempfile::TempDir::new().unwrap();
            let prompt = tmp.path().join("plan.prompt.md");
            let rendered = b"rendered prompt bytes\n\0";

            atomic_prompt_write(&prompt, rendered).unwrap();

            assert_eq!(std::fs::read(&prompt).unwrap(), rendered);
            assert!(!prompt.with_extension("md.tmp").exists());
        }

        #[test]
        fn rename_failure_keeps_destination_and_removes_temp_file() {
            let tmp = tempfile::TempDir::new().unwrap();
            let prompt = tmp.path().join("plan.prompt.md");
            std::fs::create_dir(&prompt).unwrap();
            let original = prompt.join("original");
            std::fs::write(&original, b"untouched").unwrap();

            let error = atomic_prompt_write(&prompt, b"replacement").unwrap_err();

            assert!(!error.to_string().is_empty());
            assert_eq!(std::fs::read(&original).unwrap(), b"untouched");
            assert!(!prompt.with_extension("md.tmp").exists());
        }
    }

    // ── pre-dispatch prompt snapshot detects third-party mutation ──

    #[test]
    fn prompt_snapshot_is_silent_when_the_prompt_is_untouched() {
        let tmp = tempfile::TempDir::new().unwrap();
        let workdir = tmp.path();
        let prompt = workdir.join("feat-x.prompt.md");
        let original = b"# Plan\n\n## Status\n\nedited\n";
        std::fs::write(&prompt, original).unwrap();

        let snap = snapshot_prompt(workdir, &prompt, &tid(14), "test").unwrap();
        let after = std::fs::read(&prompt).unwrap();

        assert_eq!(prompt_mutation_msg(&snap, &after), None);
        assert_eq!(std::fs::read(&snap.backup).unwrap(), original);
        assert_eq!(
            snap.backup,
            workdir
                .join(".dev/pipeline/feat-x")
                .join(tid(14))
                .join(format!("{}-test.prompt.bak", tid(14)))
        );
    }

    #[test]
    fn prompt_snapshot_names_the_change_and_keeps_the_lost_bytes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let workdir = tmp.path();
        let prompt = workdir.join("feat-x.prompt.md");
        let task = tid(14);
        let original =
            format!("# Plan\n\n## Status\n\nedited\n\n## Review Results\n\n### [{task}]\n")
                .into_bytes();
        std::fs::write(&prompt, &original).unwrap();

        let snap = snapshot_prompt(workdir, &prompt, &task, "test").unwrap();
        // A third party reverts the file, dropping the Status edit and the
        // task-scoped Review Results subsection written by an earlier phase.
        std::fs::write(&prompt, b"# Plan\n\n## Status\n\n## Review Results\n").unwrap();
        let after = std::fs::read(&prompt).unwrap();

        let msg = prompt_mutation_msg(&snap, &after).expect("mutation must be reported");
        assert!(
            msg.contains(&snap.digest),
            "message names the before digest: {msg}"
        );
        assert!(
            msg.contains(&snap.backup.display().to_string()),
            "message names the recovery copy: {msg}"
        );
        // The bytes the dispatch would otherwise have lost are still on disk.
        assert_eq!(std::fs::read(&snap.backup).unwrap(), original);
    }

    #[test]
    fn tp05_prompt_snapshot_uses_the_plan_task_folder() {
        let tmp = tempfile::TempDir::new().unwrap();
        let prompt = tmp.path().join("fixture.prompt.md");
        std::fs::write(&prompt, b"prompt bytes").unwrap();

        let task = tid(3);
        let snap = snapshot_prompt(tmp.path(), &prompt, &task, "implement").unwrap();

        assert_eq!(
            snap.backup,
            tmp.path()
                .join(".dev/pipeline/fixture")
                .join(&task)
                .join(format!("{task}-implement.prompt.bak"))
        );
        assert_eq!(std::fs::read(&snap.backup).unwrap(), b"prompt bytes");
    }

    #[test]
    fn tp05_date_named_plan_rejects_prompt_snapshot_without_creating_state() {
        let tmp = tempfile::TempDir::new().unwrap();
        let prompt = tmp.path().join("20260915.md");
        std::fs::write(&prompt, b"prompt bytes").unwrap();

        assert!(snapshot_prompt(tmp.path(), &prompt, &tid(3), "implement").is_none());
        assert!(!tmp.path().join(".dev/pipeline").exists());
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
        let (spec, task_id, _r, _affected, _authority) = build_spec_from_prompt(
            &prompt,
            tmp.path(),
            "implement",
            Some(&tid(2)),
            None,
            None,
            false,
        )
        .unwrap();
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
        let (_spec2, auto, _r2, _affected2, _authority2) =
            build_spec_from_prompt(&prompt, tmp.path(), "implement", None, None, None, false)
                .unwrap();
        assert_eq!(auto, tid(1));
    }

    #[test]
    fn prompt_identity_is_workdir_relative() {
        let tmp = tempfile::TempDir::new().unwrap();
        let workdir = tmp.path().join("checkout");
        let prompt = workdir.join(".dev").join("plans").join("feature.prompt.md");
        std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        std::fs::write(&prompt, "prompt").unwrap();

        assert_eq!(
            prompt_identity_relative_to_workdir(&prompt, &workdir).unwrap(),
            ".dev/plans/feature.prompt.md"
        );
        assert!(!prompt_identity_relative_to_workdir(&prompt, &workdir)
            .unwrap()
            .contains('\\'));

        let outside = tmp.path().join("outside.prompt.md");
        std::fs::write(&outside, "prompt").unwrap();
        let error = prompt_identity_relative_to_workdir(&outside, &workdir).unwrap_err();
        assert!(error.contains("outside dispatch workdir"));
    }

    #[test]
    fn source_plan_paired_prompt_uses_the_same_relative_identity() {
        let tmp = tempfile::TempDir::new().unwrap();
        let workdir = tmp.path().join("checkout");
        let source_plan = workdir.join(".dev").join("plans").join("feature.md");
        let prompt = paired_prompt_path(&source_plan).unwrap();
        std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        let body = format!("## Tasks\n\n- [ ] {} - Paired task.\n", tid(3));
        std::fs::write(&source_plan, "source plan").unwrap();
        std::fs::write(&prompt, &body).unwrap();

        assert_eq!(
            resolve_pipeline_input(&source_plan),
            PipelineInput::SourcePlan
        );
        let (source_spec, ..) = build_spec_from_prompt(
            &prompt,
            &workdir,
            "implement",
            Some(&tid(3)),
            None,
            None,
            false,
        )
        .unwrap();
        let (prompt_spec, ..) = build_spec_from_prompt(
            &prompt,
            &workdir,
            "implement",
            Some(&tid(3)),
            None,
            None,
            false,
        )
        .unwrap();

        assert_eq!(
            source_spec
                .lines()
                .find(|line| line.starts_with("Source prompt: ")),
            prompt_spec
                .lines()
                .find(|line| line.starts_with("Source prompt: "))
        );
        assert!(source_spec.contains("Source prompt: .dev/plans/feature.prompt.md"));
    }

    #[test]
    fn raw_spec_route_keeps_legacy_receipt_behavior() {
        let raw_path = PathBuf::from(format!(".dev/generated/task-specs/{}-implement.md", tid(3)));
        assert_eq!(resolve_pipeline_input(&raw_path), PipelineInput::RawSpec);

        let receipt = effective_receipt_path("test", &tid(3), None, None, "20260915").unwrap();
        assert_eq!(
            receipt,
            Some(format!(
                ".dev/pipeline/20260915/test-direct/{}-test.receipt.md",
                tid(3)
            ))
        );
        assert_eq!(
            resolve_receipt_path(None, None, format!("{}-test.receipt.md", tid(3))).unwrap_err(),
            "a raw receipt path is built only by effective_receipt_path"
        );
    }

    #[test]
    fn effective_receipt_path_defaults() {
        let t = tid(8);
        // test/audit/implement get a deterministic default receipt.
        assert_eq!(
            effective_receipt_path("test", &t, None, None, "20260915").unwrap(),
            Some(format!(
                ".dev/pipeline/20260915/test-direct/{t}-test.receipt.md"
            ))
        );
        assert_eq!(
            effective_receipt_path("audit", &t, None, None, "20260915").unwrap(),
            Some(format!(
                ".dev/pipeline/20260915/test-direct/{t}-audit.receipt.md"
            ))
        );
        assert_eq!(
            effective_receipt_path("implement", &t, None, None, "20260915").unwrap(),
            Some(format!(
                ".dev/pipeline/20260915/test-direct/{t}-implement.receipt.md"
            ))
        );
        // Mixed-case phase resolves to lowercased filename.
        assert_eq!(
            effective_receipt_path("Implement", &t, None, None, "20260915").unwrap(),
            Some(format!(
                ".dev/pipeline/20260915/test-direct/{t}-implement.receipt.md"
            ))
        );
        // Other phases default to no receipt.
        assert_eq!(
            effective_receipt_path("unknown", &t, None, None, "20260915").unwrap(),
            None
        );
        // explicit --receipt always wins.
        assert_eq!(
            effective_receipt_path("test", &t, Some("custom/r.md"), None, "20260915").unwrap(),
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
        let (spec, task_id, receipt, _affected, _authority) = build_spec_from_prompt(
            &prompt,
            tmp.path(),
            "test",
            Some(&tid(8)),
            None,
            None,
            false,
        )
        .unwrap();
        let expected = format!(".dev/pipeline/x/{}/{}-test.receipt.md", tid(8), tid(8));
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
        // directory, escaping the intended `.dev/pipeline/<scope>/` confinement.
        assert_eq!(sanitize_plan_slug(".."), "unknown-plan");
        assert_eq!(sanitize_plan_slug("."), "unknown-plan");
        assert_eq!(sanitize_plan_slug("..."), "unknown-plan");
    }

    #[test]
    fn resolve_log_dir_override_scopes_under_plan_slug_for_prompt_or_source_plan() {
        let workdir = PathBuf::from("/repo");
        let prompt = Path::new(".dev/plans/fix-foo.prompt.md");
        let source_plan = Path::new(".dev/plans/fix-foo.md");
        let task = tid(1);
        let expected = PathBuf::from(format!("/repo/.dev/pipeline/fix-foo/{task}"));
        assert_eq!(
            resolve_log_dir_override(&workdir, Some(prompt), Some(&task)),
            Ok(Some(expected.clone()))
        );
        assert_eq!(
            resolve_log_dir_override(&workdir, Some(source_plan), Some(&task)),
            Ok(Some(expected))
        );
    }

    #[test]
    fn resolve_log_dir_override_none_for_raw_direct_dispatch() {
        // No plan_log_source (RawSpec / raw-direct dispatch) → None → the caller
        // leaves DispatchArgs.log_dir_override unset → default unscoped location.
        let workdir = PathBuf::from("/repo");
        assert_eq!(resolve_log_dir_override(&workdir, None, None), Ok(None));
    }

    #[test]
    fn tp_04_resolve_log_dir_override_uses_scope_root_without_task() {
        let workdir = PathBuf::from("/repo");
        assert_eq!(
            resolve_log_dir_override(
                &workdir,
                Some(Path::new(".dev/plans/fix-foo.prompt.md")),
                None,
            ),
            Ok(Some(PathBuf::from("/repo/.dev/pipeline/fix-foo")))
        );
    }

    #[test]
    fn resolve_receipt_path_scopes_plan_inputs_and_preserves_raw_fallback() {
        let receipt = "task-test.receipt.md";
        assert_eq!(
            resolve_receipt_path(
                Some(Path::new(".dev/plans/fix-one.prompt.md")),
                None,
                receipt
            )
            .unwrap(),
            PathBuf::from(".dev/pipeline/fix-one").join(receipt)
        );
        assert_eq!(
            resolve_receipt_path(Some(Path::new(".dev/plans/fix-two.md")), None, receipt).unwrap(),
            PathBuf::from(".dev/pipeline/fix-two").join(receipt)
        );
        assert_eq!(
            resolve_receipt_path(Some(Path::new(".dev/plans/fix-one.md")), None, receipt).unwrap(),
            resolve_receipt_path(
                Some(Path::new(".dev/plans/fix-one.prompt.md")),
                None,
                receipt
            )
            .unwrap()
        );
        assert_eq!(
            resolve_receipt_path(Some(Path::new(".dev/plans/....md")), None, receipt).unwrap(),
            PathBuf::from(".dev/pipeline/unknown-plan").join(receipt)
        );
        assert_eq!(
            resolve_receipt_path(None, None, receipt).unwrap_err(),
            "a raw receipt path is built only by effective_receipt_path"
        );
    }

    #[test]
    fn resolve_receipt_path_scopes_task_and_rejects_invalid_components() {
        let path = resolve_receipt_path(
            Some(Path::new(".dev/plans/fix-one.en.md")),
            Some("worker-01"),
            "check.receipt.md",
        )
        .unwrap();
        assert_eq!(
            path,
            PathBuf::from(".dev/pipeline/fix-one/worker-01/check.receipt.md")
        );
        for task in ["", ".", "..", "T/01", "T 01"] {
            assert!(resolve_receipt_path(
                Some(Path::new(".dev/plans/fix-one.md")),
                Some(task),
                "check.receipt.md"
            )
            .is_err());
        }
    }

    #[test]
    fn tp_03_receipt_layout_plan_task_and_research_scopes() {
        let plan_receipt = resolve_receipt_path(
            Some(Path::new(".dev/plans/test-plan.prompt.md")),
            None,
            "planning-check.receipt.md",
        )
        .unwrap();
        assert_eq!(
            plan_receipt,
            PathBuf::from(".dev/pipeline/test-plan/planning-check.receipt.md")
        );

        let task = tid(3);
        let task_receipt = resolve_receipt_path(
            Some(Path::new(".dev/plans/test-plan.prompt.md")),
            Some(&task),
            format!("{task}-implement.receipt.md"),
        )
        .unwrap();
        assert_eq!(
            task_receipt,
            PathBuf::from(format!(
                ".dev/pipeline/test-plan/{task}/{task}-implement.receipt.md"
            ))
        );

        let research_receipt = resolve_receipt_path(
            Some(Path::new(".dev/research/test-plan.md")),
            Some("worker-2"),
            "investigate-2.receipt.md",
        )
        .unwrap();
        assert_eq!(
            research_receipt,
            PathBuf::from(".dev/pipeline/test-plan/worker-2/investigate-2.receipt.md")
        );
    }

    #[test]
    fn resolve_scope_rejects_hidden_and_eight_digit_scopes() {
        assert!(resolve_scope(Path::new(".dev/plans/.hidden.md"))
            .unwrap_err()
            .contains(".hidden"));
        assert!(resolve_scope(Path::new(".dev/plans/12345678.md"))
            .unwrap_err()
            .contains("12345678"));
        assert_eq!(
            resolve_scope(Path::new(".dev/plans/example.en.md")).unwrap(),
            "example"
        );
    }

    #[test]
    fn research_scope_resolves_receipt_and_log_directories() {
        // A `.dev/research/<slug>.md` dispatch source must scope its executor
        // task log directory under `.dev/pipeline/<scope>/<task>/` — the SAME mechanism
        // (`plan_slug_from_path` + `resolve_log_dir_override`) already used for
        // `.dev/plans/` Prompt/SourcePlan inputs — instead of the unscoped default.
        //
        // `resolve_pipeline_input` classifies `.dev/research/<slug>.md` as `RawSpec`
        // (see `resolve_pipeline_input_classifies_by_location_and_suffix`), and
        // `cmd_pipeline`'s `RawSpec` arm currently hardcodes `plan_log_source = None`
        // unconditionally for every `RawSpec` input, so today a research-sourced
        // dispatch never reaches `resolve_log_dir_override` with its own path and
        // keeps the unscoped default location. This probe drives the
        // real entry point (`cmd_pipeline`), not the scoping helpers directly, since
        // `resolve_log_dir_override(workdir, Some(path))` is already directory-agnostic
        // (filename-only derivation) and would pass today even though the call site
        // never supplies that `Some(path)` for a research source.
        //
        // Shared process-global env guard (HOME/USERPROFILE) — same pattern as
        // `all_higher_tier_miss_falls_back_to_embedded_exactly_once`.
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());

        let home = tempfile::TempDir::new().unwrap();
        let config_dir = home.path().join(".gal").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        // A minimal routing table: TESTER → a name no test machine has on PATH, so
        // `is_available` returns `false` fast (no real subprocess spawn, no hang) —
        // the executor-log directory is created (`write_started_marker`) before that
        // availability check runs.
        std::fs::write(
            config_dir.join("config.json"),
            r#"{"executorRouting":{"executors":{},"pipeline":{"TESTER":{"executor":"gal-test-first-fixture-no-such-binary"}}}}"#,
        )
        .unwrap();

        #[cfg(windows)]
        let prev = std::env::var_os("USERPROFILE");
        #[cfg(not(windows))]
        let prev = std::env::var_os("HOME");
        #[cfg(windows)]
        std::env::set_var("USERPROFILE", home.path());
        #[cfg(not(windows))]
        std::env::set_var("HOME", home.path());

        let workdir = tempfile::TempDir::new().unwrap();
        let research_dir = workdir.path().join(".dev").join("research");
        std::fs::create_dir_all(&research_dir).unwrap();
        let research_source = research_dir.join("find-flaky-tests.md");
        std::fs::write(
            &research_source,
            "## Task Goal\n\n- [ ] STUB-02 — placeholder\n",
        )
        .unwrap();

        let args = vec![
            "pipeline".to_string(),
            research_source.to_str().unwrap().to_string(),
            "--phase".to_string(),
            "test".to_string(),
            "--task".to_string(),
            "STUB-02".to_string(),
            "--workdir".to_string(),
            workdir.path().to_str().unwrap().to_string(),
        ];
        let _exit = cmd_pipeline(&args);

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

        let scoped_log_dir = workdir
            .path()
            .join(".dev")
            .join("pipeline")
            .join("find-flaky-tests")
            .join("STUB-02");
        assert!(
            scoped_log_dir.is_dir(),
            "research scope resolves receipt and log directories: expected scoped \
             executor-log dir '{}' to exist after dispatching a `.dev/research/` \
             source, found only: {:?}",
            scoped_log_dir.display(),
            std::fs::read_dir(workdir.path().join(".dev").join("pipeline"))
                .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
                .unwrap_or_else(|_| Vec::<PathBuf>::new())
        );
    }

    #[test]
    fn tp_06_raw_dispatch_receipt_and_log_dir_share_date_root() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());

        let home = tempfile::TempDir::new().unwrap();
        let config_dir = home.path().join(".gal").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.json"),
            r#"{"executorRouting":{"executors":{},"pipeline":{"TESTER":{"executor":"gal-test-first-fixture-no-such-binary"}}}}"#,
        )
        .unwrap();

        #[cfg(windows)]
        let prev = std::env::var_os("USERPROFILE");
        #[cfg(not(windows))]
        let prev = std::env::var_os("HOME");
        #[cfg(windows)]
        std::env::set_var("USERPROFILE", home.path());
        #[cfg(not(windows))]
        std::env::set_var("HOME", home.path());

        let workdir = tempfile::TempDir::new().unwrap();
        let specs_dir = workdir.path().join(".dev").join("task-specs");
        std::fs::create_dir_all(&specs_dir).unwrap();
        let task = tid(6);
        let spec_file = specs_dir.join(format!("{task}-test.md"));
        let date = gal_foundation::time::utc_date_compact();
        let receipt_file = workdir
            .path()
            .join(".dev")
            .join("pipeline")
            .join(&date)
            .join("test-direct")
            .join(format!("{task}-test.receipt.md"));

        std::fs::write(
            &spec_file,
            format!(
                "## Task Goal\n\n- [ ] {task} — raw dispatch test\n\n## Write-Back Instruction\n\nWrite to {}\n",
                receipt_file.display()
            ),
        )
        .unwrap();

        let args = vec![
            "pipeline".to_string(),
            spec_file.to_str().unwrap().to_string(),
            "--phase".to_string(),
            "test".to_string(),
            "--task".to_string(),
            task.clone(),
            "--workdir".to_string(),
            workdir.path().to_str().unwrap().to_string(),
        ];
        let _exit = cmd_pipeline(&args);

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

        let expected_raw_dir = workdir
            .path()
            .join(".dev")
            .join("pipeline")
            .join(&date)
            .join("test-direct");

        assert!(
            expected_raw_dir.is_dir(),
            "raw dispatch creates log dir under date root: expected '{}' to exist, found: {:?}",
            expected_raw_dir.display(),
            std::fs::read_dir(workdir.path().join(".dev").join("pipeline"))
                .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
                .unwrap_or_else(|_| Vec::<PathBuf>::new())
        );

        let raw_receipt = effective_receipt_path("test", &task, None, None, &date).unwrap();
        assert_eq!(
            raw_receipt,
            Some(format!(
                ".dev/pipeline/{date}/test-direct/{task}-test.receipt.md"
            ))
        );

        assert_eq!(
            resolve_receipt_path(None, None, format!("{task}-test.receipt.md")).unwrap_err(),
            "a raw receipt path is built only by effective_receipt_path"
        );

        log_semantic_failure(
            workdir.path(),
            &task,
            "test",
            "tester",
            "test-semantic-failure",
            None,
        );

        let loop_log_file = workdir
            .path()
            .join(".dev")
            .join("pipeline")
            .join(&date)
            .join("loop-log.ndjson");
        assert!(
            loop_log_file.is_file(),
            "loop-log should be written under date root: expected '{}' to exist",
            loop_log_file.display()
        );
        let loop_log_content = std::fs::read_to_string(&loop_log_file).unwrap();
        assert!(loop_log_content.contains(&task));
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
    fn materialization_failure_message_keeps_path_and_cause() {
        let error = ContractResolveError::Materialization {
            path: PathBuf::from("C:/test/.gal"),
            cause: "create embedded source staging directory 'C:/test/.gal/embedded-src.staging-1': access denied".into(),
        };
        let message = error.message();
        assert!(message.contains("embedded agent-contract source materialization failed"));
        assert!(message.contains("C:/test/.gal"));
        assert!(message.contains("embedded-src.staging-1"));
        assert!(message.contains("access denied"));
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
        assert!(
            contract_file.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test reads the real implementer contract",
            contract_file.display()
        );
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
        let (spec, _task, _r, _affected, _authority) = build_spec_from_prompt(
            &prompt,
            tmp.path(),
            "implement",
            Some(&tid(1)),
            None,
            Some(body),
            false,
        )
        .unwrap();
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

    #[test]
    fn fix_mode_requires_implement_prompt_or_source_plan() {
        assert!(validate_fix_mode(true, "implement", PipelineInput::Prompt).is_ok());
        assert!(validate_fix_mode(true, "implement", PipelineInput::SourcePlan).is_ok());
        assert!(validate_fix_mode(true, "test", PipelineInput::Prompt).is_err());
        assert!(validate_fix_mode(true, "implement", PipelineInput::RawSpec).is_err());
        assert!(validate_fix_mode(false, "test", PipelineInput::RawSpec).is_ok());
    }

    #[test]
    fn fix_authority_fingerprint_uses_only_r4_fields() {
        let base = pipeline::task_spec::FixAuthorityInputs {
            task_id: tid(3),
            executor_phase: "implement".to_string(),
            handoff_heading: format!("#### Retry Handoff — {} / TEST", tid(3)),
            task_goal: "Repair the retry guard.".to_string(),
            affected_files: vec!["crates/cli/src/commands/dispatch.rs".to_string()],
            problem: "The retry replayed identical authority.".to_string(),
            next_human_step: "Hash only stable inputs.".to_string(),
            agent_contract: "contract bytes".to_string(),
        };
        let expected = stable_fix_authority_fingerprint(&base);
        for changed in [
            pipeline::task_spec::FixAuthorityInputs {
                task_goal: "A substantive task change.".to_string(),
                ..base.clone()
            },
            pipeline::task_spec::FixAuthorityInputs {
                problem: "A substantive handoff change.".to_string(),
                ..base.clone()
            },
            pipeline::task_spec::FixAuthorityInputs {
                agent_contract: "changed contract bytes".to_string(),
                ..base.clone()
            },
        ] {
            assert_ne!(expected, stable_fix_authority_fingerprint(&changed));
        }
    }

    #[test]
    fn fix_replay_fingerprint_persists_between_calls() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join(
            resolve_receipt_path(
                Some(Path::new(".dev/plans/fixture.prompt.md")),
                Some(&tid(3)),
                format!("{}-implement.sha256", tid(3)),
            )
            .unwrap(),
        );
        assert!(!persist_fix_fingerprint(&path, "authority-a").unwrap());
        assert!(path.is_file());
        assert!(persist_fix_fingerprint(&path, "authority-a").unwrap());
        assert!(!persist_fix_fingerprint(&path, "authority-b").unwrap());
    }

    #[test]
    fn tp05_replay_fingerprint_uses_the_plan_task_folder() {
        let task = tid(3);
        let path = resolve_receipt_path(
            Some(Path::new(".dev/plans/fixture.prompt.md")),
            Some(&task),
            format!("{task}-implement.sha256"),
        )
        .unwrap();

        assert_eq!(
            path,
            PathBuf::from(format!(
                ".dev/pipeline/fixture/{task}/{task}-implement.sha256"
            ))
        );
    }

    #[test]
    fn tp05_date_named_plan_rejects_replay_path_without_creating_state() {
        let task = tid(3);
        let path = resolve_receipt_path(
            Some(Path::new(".dev/plans/20260915.md")),
            Some(&task),
            format!("{task}-implement.sha256"),
        );

        assert!(path.is_err());
    }

    #[test]
    fn affected_file_fingerprint_ignores_writeback_and_detects_byte_changes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let affected = "crates/example.rs";
        let affected_path = tmp.path().join(affected);
        std::fs::create_dir_all(affected_path.parent().unwrap()).unwrap();
        std::fs::write(&affected_path, "before").unwrap();
        let paths = vec![affected.to_string()];
        let before = fingerprint_affected_files(tmp.path(), &paths);

        std::fs::create_dir_all(tmp.path().join(".dev/pipeline")).unwrap();
        std::fs::write(tmp.path().join("plan.prompt.md"), "write-back only").unwrap();
        std::fs::write(tmp.path().join(".dev/pipeline/receipt.md"), "pass").unwrap();
        assert_eq!(before, fingerprint_affected_files(tmp.path(), &paths));

        std::fs::write(&affected_path, "after").unwrap();
        let after = fingerprint_affected_files(tmp.path(), &paths);
        assert_ne!(before, after, "a real affected-file edit must be detected");

        std::fs::write(&affected_path, "dirty bytes changed again").unwrap();
        assert_ne!(after, fingerprint_affected_files(tmp.path(), &paths));
    }

    #[test]
    fn tp05_fix_round_guard_is_origin_aware() {
        let same = "fingerprint-a";
        let changed = "fingerprint-b";
        // IMPLEMENT origin: unchanged allowlisted files pass, changed files are a boundary repair error.
        assert_eq!(
            fix_round_guard("IMPLEMENT", Some("completed"), same, same),
            FixRoundGuard::Pass
        );
        assert_eq!(
            fix_round_guard("IMPLEMENT", Some("completed"), same, changed),
            FixRoundGuard::BoundaryRepairTouchedScope
        );
        // TEST origin: unchanged allowlisted files keep the no-change error.
        assert_eq!(
            fix_round_guard("TEST", Some("completed"), same, same),
            FixRoundGuard::FixRoundNoChange
        );
        assert_eq!(
            fix_round_guard("TEST", Some("completed"), same, changed),
            FixRoundGuard::Pass
        );
        assert_eq!(
            fix_round_guard("AUDIT", Some("completed"), same, same),
            FixRoundGuard::FixRoundNoChange
        );
        // A dispatch that did not complete is never judged by this guard.
        assert_eq!(
            fix_round_guard("IMPLEMENT", Some("failed"), same, changed),
            FixRoundGuard::Pass
        );
        assert_eq!(
            fix_round_guard("TEST", None, same, same),
            FixRoundGuard::Pass
        );
    }

    #[test]
    fn tp05_handoff_origin_is_read_from_heading_suffix() {
        let task = tid(4);
        assert_eq!(
            handoff_origin_phase(&format!("#### Retry Handoff — {task} / IMPLEMENT")),
            Some("IMPLEMENT")
        );
        assert_eq!(
            handoff_origin_phase(&format!("#### Retry Handoff — {task} / TEST")),
            Some("TEST")
        );
        assert_eq!(handoff_origin_phase("no suffix here"), None);
    }

    #[test]
    fn executor_spec_parity_across_five_executors() {
        let tmp = tempfile::TempDir::new().unwrap();
        let prompt = tmp.path().join("parity.prompt.md");
        std::fs::write(
            &prompt,
            format!("## Tasks\n\n- [ ] {t} — Touch `crates/a.rs`.\n", t = tid(5)),
        )
        .unwrap();
        let contract_body = "# Golem Implementer\n\nContract body for parity check.\n";
        let (spec_base, _task_id, _receipt, _affected, _authority) = build_spec_from_prompt(
            &prompt,
            tmp.path(),
            "implement",
            Some(&tid(5)),
            None,
            Some(contract_body),
            false,
        )
        .unwrap();

        let executors = ["claude", "copilot", "gemini", "codex", "opencode"];
        for executor in executors {
            let spawn_cfg = dispatch::dispatch::SpawnConfig {
                workdir: tmp.path().to_path_buf(),
                task_id: tid(5),
                phase: "implement".to_string(),
                executor: executor.to_string(),
                executor_args: vec![],
                spec: spec_base.clone(),
                timeout_secs: 10,
                actual_model: "test-model".to_string(),
                log_dir: tmp.path().join(".dev/pipeline/plan-fixture"),
                receipt_path: None,
                contract_provenance: None,
            };
            assert_eq!(
                spawn_cfg.spec, spec_base,
                "Executor {executor} must receive the identical assembled task spec"
            );
        }
    }

    #[test]
    fn pipeline_phase_writeback_cases() {
        let tmp = tempfile::TempDir::new().unwrap();
        let prompt_path = tmp.path().join("test.prompt.md");
        let initial_prompt = format!(
            "# Plan\n\n## Tasks\n- [ ] {} — Goal\n\n## Test Results\n\n## Review Results\n",
            tid(1)
        );
        std::fs::write(&prompt_path, initial_prompt).unwrap();

        // 1. Markerless audit success
        let audit_receipt_path = tmp.path().join(format!("{}-audit.receipt.md", tid(1)));
        let audit_receipt = format!(
            "### [{}] 2026-08-26\n\n**Date:** 2026-08-26\n**Findings:** 0 total\n\n<!-- AUDIT_REVIEW: CLEAR -->\n",
            tid(1)
        );
        std::fs::write(&audit_receipt_path, &audit_receipt).unwrap();

        let prompt_bytes = std::fs::read(&prompt_path).unwrap();
        let receipt_bytes = std::fs::read(&audit_receipt_path).unwrap();
        let rendered = pipeline::task_spec::render_phase_writeback(
            &prompt_bytes,
            &receipt_bytes,
            "audit",
            &tid(1),
        )
        .unwrap();
        std::fs::write(&prompt_path, &rendered).unwrap();

        let updated_prompt = std::fs::read_to_string(&prompt_path).unwrap();
        assert!(
            updated_prompt.contains(&format!("## Review Results\n\n### [{}] 2026-08-26", tid(1)))
        );
        assert!(updated_prompt.contains("<!-- AUDIT_REVIEW: CLEAR -->"));

        // 2. Markerless test success
        let test_receipt_path = tmp.path().join(format!("{}-test.receipt.md", tid(1)));
        let test_receipt = format!(
            "### [{}] 2026-08-26\n\nverdict: PASS\nevidence: test passed 1/1\n",
            tid(1)
        );
        std::fs::write(&test_receipt_path, &test_receipt).unwrap();

        let prompt_bytes = std::fs::read(&prompt_path).unwrap();
        let receipt_bytes = std::fs::read(&test_receipt_path).unwrap();
        let rendered = pipeline::task_spec::render_phase_writeback(
            &prompt_bytes,
            &receipt_bytes,
            "test",
            &tid(1),
        )
        .unwrap();
        std::fs::write(&prompt_path, &rendered).unwrap();

        let updated_prompt = std::fs::read_to_string(&prompt_path).unwrap();
        assert!(updated_prompt.contains(&format!("## Test Results\n\n### [{}] 2026-08-26", tid(1))));

        // 3. Retry append preserves prior history
        let retry_receipt = format!(
            "### [{}] 2026-08-27\n\nverdict: PASS\nevidence: retry test passed 2/2\n",
            tid(1)
        );
        let prompt_bytes = std::fs::read(&prompt_path).unwrap();
        let rendered = pipeline::task_spec::render_phase_writeback(
            &prompt_bytes,
            retry_receipt.as_bytes(),
            "test",
            &tid(1),
        )
        .unwrap();
        std::fs::write(&prompt_path, &rendered).unwrap();

        let updated_prompt = std::fs::read_to_string(&prompt_path).unwrap();
        assert!(updated_prompt.contains(&format!("### [{}] 2026-08-26", tid(1))));
        assert!(updated_prompt.contains(&format!("### [{}] 2026-08-27", tid(1))));

        // 6. Implement bypasses the write-back gate predicate
        assert!(!pipeline::task_spec::is_writeback_in_scope("implement"));

        // 7. Malformed payload failure leaves prompt unchanged
        let malformed_receipt = format!("### [{}] 2026-08-26\n\nverdict: PASS\n", tid(2));
        let prompt_bytes_before = std::fs::read(&prompt_path).unwrap();
        assert!(pipeline::task_spec::render_phase_writeback(
            &prompt_bytes_before,
            malformed_receipt.as_bytes(),
            "audit",
            &tid(1)
        )
        .is_err());
        let prompt_bytes_after = std::fs::read(&prompt_path).unwrap();
        assert_eq!(prompt_bytes_before, prompt_bytes_after);

        // 8. Missing destination H2 section failure leaves prompt unchanged
        let missing_h2_prompt = "# Plan\n\n## Tasks\n";
        let missing_h2_path = tmp.path().join("missing_h2.prompt.md");
        std::fs::write(&missing_h2_path, missing_h2_prompt).unwrap();
        let prompt_bytes_before = std::fs::read(&missing_h2_path).unwrap();
        assert!(matches!(
            pipeline::task_spec::render_phase_writeback(
                &prompt_bytes_before,
                audit_receipt.as_bytes(),
                "audit",
                &tid(1)
            ),
            Err(pipeline::PipelineError::MissingSection(_))
        ));
        let prompt_bytes_after = std::fs::read(&missing_h2_path).unwrap();
        assert_eq!(prompt_bytes_before, prompt_bytes_after);

        // 9. Duplicate destination H2 section failure leaves prompt unchanged
        let dup_h2_prompt = "# Plan\n\n## Review Results\n\n## Review Results\n";
        let dup_h2_path = tmp.path().join("dup_h2.prompt.md");
        std::fs::write(&dup_h2_path, dup_h2_prompt).unwrap();
        let prompt_bytes_before = std::fs::read(&dup_h2_path).unwrap();
        assert!(matches!(
            pipeline::task_spec::render_phase_writeback(
                &prompt_bytes_before,
                audit_receipt.as_bytes(),
                "audit",
                &tid(1)
            ),
            Err(pipeline::PipelineError::DuplicateSection(_))
        ));
        let prompt_bytes_after = std::fs::read(&dup_h2_path).unwrap();
        assert_eq!(prompt_bytes_before, prompt_bytes_after);

        // 11. log_semantic_failure writes structured loop log entry
        let test_log_dir = tmp
            .path()
            .join(".dev/pipeline")
            .join(gal_foundation::time::utc_date_compact());
        log_semantic_failure(
            tmp.path(),
            &tid(1),
            "audit",
            "auditor",
            "receipt-read-failure simulation",
            None,
        );
        assert!(test_log_dir.exists());
        let entries: Vec<_> = std::fs::read_dir(&test_log_dir).unwrap().collect();
        assert!(!entries.is_empty());
        let log_file_path = entries[0].as_ref().unwrap().path();
        assert_eq!(
            log_file_path.file_name().and_then(|s| s.to_str()),
            Some("loop-log.ndjson")
        );
        let log_content = std::fs::read_to_string(&log_file_path).unwrap();
        assert!(log_content
            .contains("phase-writeback semantic failure: receipt-read-failure simulation"));
        assert!(log_content.contains(&tid(1)));
    }

    // ── `gal pipeline --phase investigate` must be rejected with a
    // usage error, not silently dispatched now that `investigate` is a parseable
    // `Phase`. Locked seam: the phase validation in `cmd_pipeline` — no such
    // validation exists yet, so this probe is expected red.
    //
    // Called in-process (`cmd_pipeline` is `pub(crate)`, not a separate binary
    // invocation): `gal-cli` is a bin-only crate with no lib target, so there is no
    // `CARGO_BIN_EXE_gal` available to a unit test compiled as part of that same
    // bin's own test harness, and the crate carries no stdio-capture dependency to
    // assert on `cmd_pipeline`'s `eprintln!` text without touching the frozen
    // production seam or Cargo.toml (out of this task's allowlist). The exit-code
    // check below is the observable half of the acceptance criteria reachable from
    // here; the "naming `gal research`" wording is verified by re-running this
    // probe by hand against the implementer's change (see receipt).
    #[test]
    fn gal_pipeline_rejects_investigate_naming_gal_research() {
        let tmp = tempfile::TempDir::new().unwrap();
        let workdir = tmp.path();
        // A minimal RawSpec task-spec file (no `## Agent Contract` reference, so
        // `verify_agent_contract` is a no-op and does not mask the phase check).
        let spec_path = workdir.join(".dev/generated/task-specs/STUB-01-investigate.md");
        std::fs::create_dir_all(spec_path.parent().unwrap()).unwrap();
        std::fs::write(&spec_path, "## Task Goal\n\n- [ ] STUB-01 — placeholder\n").unwrap();

        let args = vec![
            "pipeline".to_string(),
            spec_path.to_str().unwrap().to_string(),
            "--phase".to_string(),
            "investigate".to_string(),
            "--task".to_string(),
            "STUB-01".to_string(),
        ];
        let exit = cmd_pipeline(&args);

        assert_eq!(
            exit,
            ExitCode::Usage,
            "gal pipeline rejects investigate naming gal research"
        );
    }

    #[test]
    fn compatibility_pipeline_contract_generated_dispatch_argv_round_trips_parser() {
        let task = tid(7);
        let receipt = format!("C:/workspace/.dev/pipeline/plan/{task}/{task}-test.receipt.md");
        let args = vec![
            "--phase".to_string(),
            "test".to_string(),
            "--task".to_string(),
            task.clone(),
            "--workdir".to_string(),
            "C:/workspace".to_string(),
            "--receipt".to_string(),
            receipt.clone(),
        ];
        let parsed = dispatch::cli::parse_args(&args)
            .expect("cmd_pipeline generated argv must be accepted by the real parser");
        assert_eq!(parsed.phase.as_str(), "test");
        assert_eq!(parsed.task, task);
        assert_eq!(parsed.workdir, PathBuf::from("C:/workspace"));
        assert_eq!(parsed.receipt_path.as_deref(), Some(Path::new(&receipt)));
    }

    #[test]
    fn compatibility_pipeline_contract_rejects_from_before_dispatch_and_keeps_defaults() {
        assert_eq!(validate_pipeline_phase("implement"), Ok(()));
        assert_eq!(validate_pipeline_phase("test"), Ok(()));
        assert_eq!(validate_pipeline_phase("audit"), Ok(()));

        let sandbox = tempfile::TempDir::new().unwrap();
        let spec = sandbox.path().join("task.md");
        std::fs::write(&spec, "## Task Goal\n\nFixture.\n").unwrap();
        let exit = cmd_pipeline(&[
            "pipeline".into(),
            spec.display().to_string(),
            "from".into(),
            tid(7),
        ]);
        assert_eq!(
            exit,
            ExitCode::Usage,
            "stale continuation must fail at argument parsing"
        );
        assert!(
            !sandbox.path().join(".dev/pipeline").exists(),
            "rejected continuation must not start a dispatch or create its log scope"
        );
    }

    #[test]
    fn compatibility_pipeline_contract_missing_audit_blocks_pass_without_optional_phases() {
        let phases = crate::commands::converge_check::dispatch_phases_for(
            Path::new("fixture.prompt.md"),
            "",
            &tid(7),
        );
        assert_eq!(phases, ["implement", "test", "audit"]);
        assert!(!phases
            .iter()
            .any(|phase| matches!(*phase, "human" | "live")));

        let audit = crate::commands::converge_check::classify_phase(&[], true, false);
        assert!(matches!(
            audit,
            crate::commands::converge_check::PhaseVerdict::Outcome(
                crate::commands::converge_check::PhaseOutcome::InConversation
            )
        ));
        assert!(
            !matches!(
                audit,
                crate::commands::converge_check::PhaseVerdict::Outcome(
                    crate::commands::converge_check::PhaseOutcome::DispatchOffload
                        | crate::commands::converge_check::PhaseOutcome::RecoveredInConversation
                )
            ),
            "missing required audit evidence cannot satisfy convergence PASS"
        );
    }

    #[cfg(windows)]
    #[test]
    fn compatibility_pipeline_contract_rejects_audit_before_conflicting_executor_spawn() {
        use std::process::Command;
        use std::time::{Duration, Instant};

        struct EnvironmentRestore(
            Option<std::ffi::OsString>,
            Option<std::ffi::OsString>,
            Option<std::ffi::OsString>,
        );
        impl Drop for EnvironmentRestore {
            fn drop(&mut self) {
                for (name, value) in [
                    ("HOME", self.0.take()),
                    ("USERPROFILE", self.1.take()),
                    ("PATH", self.2.take()),
                ] {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }

        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let sandbox = tempfile::TempDir::new().unwrap();
        let home = sandbox.path().join("home");
        let workdir = sandbox.path().join("work");
        let config_dir = home.join(".gal/config");
        let plans_dir = workdir.join(".dev/plans");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&plans_dir).unwrap();

        let task = tid(7);
        let prompt = plans_dir.join("phase-conflict.prompt.md");
        let prompt_body = format!(
            "## Tasks\n\n- [ ] {task} — phase conflict fixture\n\n## Test Results\n\n## Status\n\nTask Final Commit: PLACEHOLDER\n"
        );
        std::fs::write(&prompt, &prompt_body).unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(&workdir)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };
        git(&["init"]);
        git(&["config", "user.email", "fixture@example.invalid"]);
        git(&["config", "user.name", "Fixture"]);
        let hooks = sandbox.path().join("empty-hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        git(&["config", "core.hooksPath", hooks.to_str().unwrap()]);
        git(&["config", "commit.gpgsign", "false"]);
        let initial_prompt = prompt_body.replace("PLACEHOLDER", "pending");
        std::fs::write(&prompt, &initial_prompt).unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "fixture"]);
        let head = git(&["rev-parse", "HEAD"]);
        std::fs::write(&prompt, initial_prompt.replace("pending", &head)).unwrap();

        let launch_count = sandbox.path().join("launch-count.txt");
        let started = sandbox.path().join("test-started.signal");
        let release = sandbox.path().join("release-test.signal");
        let receipt = workdir
            .join(".dev/pipeline/phase-conflict")
            .join(&task)
            .join(format!("{task}-test.receipt.md"));
        let executor = sandbox.path().join("gal-pipeline-phase-conflict.cmd");
        std::fs::write(
            &executor,
            format!(
                "@echo off\r\necho launch>>\"{}\"\r\necho started>\"{}\"\r\n:wait\r\nif exist \"{}\" goto complete\r\nping -n 2 127.0.0.1 >nul\r\ngoto wait\r\n:complete\r\n(\r\necho ### [{task}] 2026-09-25\r\necho.\r\necho verdict: PASS\r\necho evidence: synchronized fake executor completed\r\n) > \"{}\"\r\nexit /b 0\r\n",
                launch_count.display(),
                started.display(),
                release.display(),
                receipt.display(),
            ),
        )
        .unwrap();
        std::fs::write(
            config_dir.join("config.json"),
            r#"{"executorRouting":{"executors":{},"pipeline":{"TESTER":{"executor":"gal-pipeline-phase-conflict.cmd"},"AUDITOR":{"executor":"gal-pipeline-phase-conflict.cmd"}}}}"#,
        )
        .unwrap();

        let _restore = EnvironmentRestore(
            std::env::var_os("HOME"),
            std::env::var_os("USERPROFILE"),
            std::env::var_os("PATH"),
        );
        std::env::set_var("HOME", &home);
        std::env::set_var("USERPROFILE", &home);
        let mut path = executor.parent().unwrap().as_os_str().to_os_string();
        path.push(";");
        path.push(std::env::var_os("PATH").unwrap_or_default());
        std::env::set_var("PATH", path);

        let test_prompt = prompt.clone();
        let test_workdir = workdir.clone();
        let test_task = task.clone();
        let test_thread = std::thread::spawn(move || {
            cmd_pipeline(&[
                "pipeline".into(),
                test_prompt.display().to_string(),
                "--phase".into(),
                "test".into(),
                "--task".into(),
                test_task,
                "--workdir".into(),
                test_workdir.display().to_string(),
            ])
        });

        let deadline = Instant::now() + Duration::from_secs(20);
        while !started.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(started.exists(), "TEST fake executor must signal its start");

        let audit_exit = cmd_pipeline(&[
            "pipeline".into(),
            prompt.display().to_string(),
            "--phase".into(),
            "audit".into(),
            "--task".into(),
            task,
            "--workdir".into(),
            workdir.display().to_string(),
        ]);
        assert_eq!(
            audit_exit,
            ExitCode::Error,
            "conflicting AUDIT must be rejected"
        );
        let launches = std::fs::read_to_string(&launch_count).unwrap();
        assert_eq!(
            launches.lines().count(),
            1,
            "AUDIT must be rejected before its executor starts"
        );

        std::fs::write(&release, "release").unwrap();
        let test_exit = test_thread.join().expect("TEST dispatch thread must join");
        assert_eq!(
            test_exit,
            ExitCode::Success,
            "TEST fake executor must complete"
        );
        assert_eq!(
            std::fs::read_to_string(&launch_count)
                .unwrap()
                .lines()
                .count(),
            1
        );
    }

    #[test]
    fn compatibility_pipeline_contract_prompt_lease_serializes_same_prompt_and_allows_independent_prompt(
    ) {
        let workdir = tempfile::TempDir::new().unwrap();
        let first = workdir.path().join("first.prompt.md");
        let second = workdir.path().join("second.prompt.md");
        std::fs::write(&first, "first").unwrap();
        std::fs::write(&second, "second").unwrap();

        let first_lease = dispatch::dispatch::acquire_prompt_writer_lease(
            workdir.path(),
            &first,
            Some(&workdir.path().join("first.receipt.md")),
            "attempt-one",
        )
        .unwrap();
        assert!(
            dispatch::dispatch::acquire_prompt_writer_lease(
                workdir.path(),
                &first,
                Some(&workdir.path().join("other.receipt.md")),
                "attempt-conflict",
            )
            .is_err(),
            "same prompt must reject a competing transaction even with a different receipt"
        );
        let independent = dispatch::dispatch::acquire_prompt_writer_lease(
            workdir.path(),
            &second,
            Some(&workdir.path().join("second.receipt.md")),
            "attempt-two",
        )
        .expect("independent prompts can proceed concurrently");
        let lease_root = workdir
            .path()
            .join(dispatch::dispatch::LEASE_ROOT)
            .join("prompt-writers");
        assert!(
            lease_root.is_dir(),
            "lease state must stay under the workdir"
        );
        assert!(!workdir
            .path()
            .join("home")
            .join(dispatch::dispatch::LEASE_ROOT)
            .exists());

        let mut first_lease = first_lease;
        let mut independent = independent;
        first_lease.release_confirmed().unwrap();
        independent.release_confirmed().unwrap();
    }

    #[test]
    fn compatibility_pipeline_contract_unknown_cleanup_retains_prompt_lease() {
        let workdir = tempfile::TempDir::new().unwrap();
        let prompt = workdir.path().join("task.prompt.md");
        std::fs::write(&prompt, "prompt").unwrap();
        let mut lease = dispatch::dispatch::acquire_prompt_writer_lease(
            workdir.path(),
            &prompt,
            Some(&workdir.path().join("task.receipt.md")),
            "attempt-unknown",
        )
        .unwrap();
        let cleanup = dispatch::dispatch::CleanupDisposition::Unknown;
        if !matches!(cleanup, dispatch::dispatch::CleanupDisposition::Confirmed) {
            lease.retain();
        }
        drop(lease);
        assert!(
            dispatch::dispatch::acquire_prompt_writer_lease(
                workdir.path(),
                &prompt,
                None,
                "attempt-must-not-pass",
            )
            .is_err(),
            "unconfirmed owned cleanup must keep the writer lock"
        );
    }

    #[test]
    fn compatibility_pipeline_contract_writeback_uses_attempt_receipt_snapshot() {
        let workdir = tempfile::TempDir::new().unwrap();
        let receipt = workdir.path().join("receipt.md");
        let attempt_bytes = b"attempt one evidence".to_vec();
        std::fs::write(&receipt, &attempt_bytes).unwrap();
        let snapshot = dispatch::dispatch::ReceiptSnapshot {
            path: receipt.clone(),
            sha256: format!("{:x}", Sha256::digest(&attempt_bytes)),
            bytes: attempt_bytes.clone(),
        };
        std::fs::write(&receipt, b"later attempt evidence").unwrap();

        assert!(
            attempt_receipt_bytes(&snapshot, &receipt).is_err(),
            "overwritten receipt must reject writeback from stale attempt evidence"
        );
        assert_eq!(std::fs::read(&receipt).unwrap(), b"later attempt evidence");
    }

    #[cfg(windows)]
    #[test]
    fn compatibility_pipeline_contract_rejects_receipt_overwrite_before_writeback() {
        use std::process::Command;

        struct EnvironmentRestore(
            Option<std::ffi::OsString>,
            Option<std::ffi::OsString>,
            Option<std::ffi::OsString>,
        );
        impl Drop for EnvironmentRestore {
            fn drop(&mut self) {
                for (name, value) in [
                    ("HOME", self.0.take()),
                    ("USERPROFILE", self.1.take()),
                    ("PATH", self.2.take()),
                ] {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }

        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let sandbox = tempfile::TempDir::new().unwrap();
        let home = sandbox.path().join("home");
        let workdir = sandbox.path().join("work");
        let config_dir = home.join(".gal/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(workdir.join(".dev/plans")).unwrap();

        let prompt = workdir.join(".dev/plans/receipt-overwrite.prompt.md");
        let task = tid(7);
        let initial_prompt = format!(
            "## Tasks\n\n- [ ] {task} — receipt overwrite test\n\n## Test Results\n\n## Status\n\nTask Final Commit: PLACEHOLDER\n"
        );
        std::fs::write(&prompt, &initial_prompt).unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(&workdir)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };
        git(&["init"]);
        git(&["config", "user.email", "fixture@example.invalid"]);
        git(&["config", "user.name", "Fixture"]);
        let hooks = sandbox.path().join("empty-hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        git(&["config", "core.hooksPath", hooks.to_str().unwrap()]);
        git(&["config", "commit.gpgsign", "false"]);
        let placeholder_prompt = initial_prompt.replace("PLACEHOLDER", "pending");
        std::fs::write(&prompt, &placeholder_prompt).unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "fixture"]);
        let head = git(&["rev-parse", "HEAD"]);
        let initial_prompt = placeholder_prompt.replace("pending", &head);
        std::fs::write(&prompt, &initial_prompt).unwrap();

        let receipt = workdir
            .join(".dev/pipeline/receipt-overwrite")
            .join(&task)
            .join(format!("{task}-test.receipt.md"));
        let initial_receipt = format!(
            "### [{task}] 2026-09-25\r\n\r\nverdict: PASS\r\nevidence: fake executor completed\r\n"
        );
        let overwritten_receipt = format!(
            "### [{task}] 2026-09-25\n\nverdict: PASS\nevidence: replaced after dispatch\n"
        );
        let executor = sandbox.path().join("gal-pipeline-receipt-fixture.cmd");
        std::fs::create_dir_all(receipt.parent().unwrap()).unwrap();
        std::fs::write(
            &executor,
            format!(
                "@echo off\r\n(\r\necho ### [{task}] 2026-09-25\r\necho.\r\necho verdict: PASS\r\necho evidence: fake executor completed\r\n) > \"{}\"\r\nexit /b 0\r\n",
                receipt.display()
            ),
        ).unwrap();
        std::fs::write(
            config_dir.join("config.json"),
            r#"{"executorRouting":{"executors":{},"pipeline":{"TESTER":{"executor":"gal-pipeline-receipt-fixture.cmd"}}}}"#,
        ).unwrap();

        let old_home = std::env::var_os("HOME");
        let old_userprofile = std::env::var_os("USERPROFILE");
        let old_path = std::env::var_os("PATH");
        let _restore = EnvironmentRestore(old_home, old_userprofile, old_path);
        std::env::set_var("HOME", &home);
        std::env::set_var("USERPROFILE", &home);
        let mut path = executor.parent().unwrap().as_os_str().to_os_string();
        path.push(";");
        path.push(std::env::var_os("PATH").unwrap_or_default());
        std::env::set_var("PATH", path);

        let hook_receipt = receipt.clone();
        let expected_initial = initial_receipt.clone();
        let overwritten = overwritten_receipt.clone();
        PIPELINE_AFTER_DISPATCH_TEST_HOOK.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move || {
                assert_eq!(
                    std::fs::read_to_string(&hook_receipt).unwrap(),
                    expected_initial
                );
                std::fs::write(&hook_receipt, overwritten).unwrap();
            }));
        });
        let args = vec![
            "pipeline".to_string(),
            prompt.display().to_string(),
            "--phase".to_string(),
            "test".to_string(),
            "--task".to_string(),
            task.clone(),
            "--receipt".to_string(),
            receipt.display().to_string(),
            "--workdir".to_string(),
            workdir.display().to_string(),
        ];
        let exit = cmd_pipeline(&args);

        assert_eq!(
            exit,
            ExitCode::Error,
            "receipt overwrite must refuse writeback"
        );
        assert_eq!(std::fs::read(&prompt).unwrap(), initial_prompt.as_bytes());
        assert_eq!(
            std::fs::read_to_string(&receipt).unwrap(),
            overwritten_receipt
        );
        assert!(
            dispatch::dispatch::acquire_prompt_writer_lease(
                &workdir,
                &prompt,
                Some(&receipt),
                "attempt-after-overwrite",
            )
            .is_err(),
            "unverified receipt evidence must retain prompt ownership"
        );
    }

    #[cfg(unix)]
    #[test]
    fn compatibility_pipeline_contract_silent_timeout_kills_owned_tree_and_preserves_unrelated_process(
    ) {
        use std::os::unix::fs::PermissionsExt;
        use std::process::{Command, Stdio};

        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let sandbox = tempfile::TempDir::new().unwrap();
        let config_dir = sandbox.path().join(".gal/config");
        let bin_dir = sandbox.path().join("bin");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&bin_dir).unwrap();

        let child_pid_file = sandbox.path().join("owned-child.pid");
        let started_file = sandbox.path().join("executor-started");
        let executor = bin_dir.join("gal-pipeline-silent-fixture");
        std::fs::write(
            &executor,
            format!(
                "#!/bin/sh\n: > '{}'\nsleep 30 &\necho $! > '{}'\nwait\n",
                started_file.display(),
                child_pid_file.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&executor).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executor, permissions).unwrap();
        std::fs::write(
            config_dir.join("config.json"),
            r#"{"executorRouting":{"executors":{},"pipeline":{"CODER":{"executor":"gal-pipeline-silent-fixture","timeoutSecs":1}}}}"#,
        )
        .unwrap();

        let old_home = std::env::var_os("HOME");
        let old_path = std::env::var_os("PATH");
        std::env::set_var("HOME", sandbox.path());
        let mut search_path = bin_dir.as_os_str().to_os_string();
        search_path.push(":");
        search_path.push(old_path.as_deref().unwrap_or_default());
        std::env::set_var("PATH", search_path);

        let workdir = sandbox.path().join("work");
        let prompt = workdir.join(".dev/plans/silent-timeout.prompt.md");
        std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        let task = tid(7);
        std::fs::write(
            &prompt,
            format!("## Tasks\n\n- [ ] {task} — timeout fixture\n"),
        )
        .unwrap();

        let mut unrelated = Command::new("sh")
            .args(["-c", "sleep 30"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let args = vec![
            "pipeline".to_string(),
            prompt.display().to_string(),
            "--phase".to_string(),
            "implement".to_string(),
            "--task".to_string(),
            task,
            "--workdir".to_string(),
            workdir.display().to_string(),
        ];
        let started = std::time::Instant::now();
        let exit = cmd_pipeline(&args);
        let elapsed = started.elapsed();

        #[cfg(windows)]
        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        #[cfg(not(windows))]
        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match old_path {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }

        assert_eq!(
            exit,
            ExitCode::NotWired,
            "silent fake executor must time out"
        );
        assert!(
            started_file.is_file(),
            "executor must reach the synchronized start"
        );
        assert!(
            child_pid_file.is_file(),
            "executor must start an owned child"
        );
        assert!(elapsed < std::time::Duration::from_secs(12));
        assert!(
            unrelated.try_wait().unwrap().is_none(),
            "dispatch cleanup must not terminate an unrelated process"
        );
        let child_pid = std::fs::read_to_string(&child_pid_file)
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap();
        let owned_alive = Command::new("kill")
            .args(["-0", &child_pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        assert!(
            !owned_alive,
            "owned descendant must exit within cleanup bound"
        );
        let _ = unrelated.kill();
        let _ = unrelated.wait();
    }

    #[cfg(windows)]
    #[test]
    fn compatibility_pipeline_contract_windows_external_cutoff_kills_owned_tree_and_retains_evidence(
    ) {
        use std::process::{Command, Stdio};

        const CHILD_MARKER: &str = "GAL_PIPELINE_CUTOFF_CHILD";
        const CHILD_WORKDIR: &str = "GAL_PIPELINE_CUTOFF_WORKDIR";
        const CHILD_PROMPT: &str = "GAL_PIPELINE_CUTOFF_PROMPT";

        if std::env::var("GAL_TEST_MODE").as_deref() == Ok("descendant") {
            std::fs::write(
                std::env::var_os("GAL_TEST_CHILD_PID").unwrap(),
                std::process::id().to_string(),
            )
            .unwrap();
            std::thread::sleep(std::time::Duration::from_secs(60));
            return;
        }
        if std::env::var("GAL_TEST_MODE").as_deref() == Ok("unrelated") {
            std::thread::sleep(std::time::Duration::from_secs(60));
            return;
        }
        if std::env::var("GAL_TEST_MODE").as_deref() == Ok("executor") {
            for (key, value) in [
                ("GAL_TEST_STARTED", "started"),
                ("GAL_TEST_DIRTY", "dirty"),
                ("GAL_TEST_UNTRACKED", "untracked"),
            ] {
                std::fs::write(std::env::var_os(key).unwrap(), value).unwrap();
            }
            std::fs::write(
                std::env::var_os("GAL_TEST_EXECUTOR_PID").unwrap(),
                std::process::id().to_string(),
            )
            .unwrap();
            let mut descendant = Command::new(std::env::var_os("GAL_TEST_EXE").unwrap())
                .args(["--exact", "commands::dispatch::tests::compatibility_pipeline_contract_windows_external_cutoff_kills_owned_tree_and_retains_evidence", "--nocapture"])
                .env("GAL_TEST_MODE", "descendant")
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !PathBuf::from(std::env::var_os("GAL_TEST_CHILD_PID").unwrap()).is_file()
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            assert!(
                PathBuf::from(std::env::var_os("GAL_TEST_CHILD_PID").unwrap()).is_file(),
                "descendant must report its PID"
            );
            let _ = descendant.wait();
            return;
        }

        // The child uses this exact test as a disposable harness. The parent
        // terminates the harness after the fake executor signals that it started.
        if std::env::var_os(CHILD_MARKER).is_some() {
            let workdir = PathBuf::from(std::env::var_os(CHILD_WORKDIR).unwrap());
            let prompt = PathBuf::from(std::env::var_os(CHILD_PROMPT).unwrap());
            let fixture_root = workdir.parent().unwrap();
            std::env::set_var("GAL_TEST_EXE", std::env::current_exe().unwrap());
            std::env::set_var("GAL_TEST_MODE", "executor");
            std::env::set_var(
                "GAL_TEST_STARTED",
                fixture_root.join("executor-started.signal"),
            );
            std::env::set_var(
                "GAL_TEST_EXECUTOR_PID",
                fixture_root.join("executor-script.pid"),
            );
            std::env::set_var(
                "GAL_TEST_CHILD_PID",
                fixture_root.join("owned-descendant.pid"),
            );
            std::env::set_var("GAL_TEST_DIRTY", workdir.join("dirty-output.txt"));
            std::env::set_var("GAL_TEST_UNTRACKED", workdir.join("untracked-output.txt"));
            let task = tid(7);
            let _ = cmd_pipeline(&[
                "pipeline".to_string(),
                prompt.display().to_string(),
                "--phase".to_string(),
                "implement".to_string(),
                "--task".to_string(),
                task,
                "--workdir".to_string(),
                workdir.display().to_string(),
            ]);
            panic!("cutoff harness returned before the parent terminated it");
        }

        fn process_exists(pid: u32) -> bool {
            Command::new("tasklist.exe")
                .args(["/FI", &format!("PID eq {pid}"), "/NH"])
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
                .unwrap_or(false)
        }

        let sandbox = tempfile::TempDir::new().unwrap();
        let home = sandbox.path().join("home");
        let config_dir = home.join(".gal/config");
        let bin_dir = sandbox.path().join("bin");
        let workdir = sandbox.path().join("work");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&bin_dir).unwrap();

        let task = tid(7);
        let prompt = workdir.join(".dev/plans/external-cutoff.prompt.md");
        std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        std::fs::write(
            &prompt,
            format!("## Tasks\n\n- [ ] {task} — external cutoff fixture\n"),
        )
        .unwrap();
        let started = sandbox.path().join("executor-started.signal");
        let executor_pid_file = sandbox.path().join("executor-script.pid");
        let child_pid_file = sandbox.path().join("owned-descendant.pid");
        let dirty = workdir.join("dirty-output.txt");
        let untracked = workdir.join("untracked-output.txt");
        let executor = bin_dir.join("gal-pipeline-cutoff-fixture.cmd");
        std::fs::write(
            &executor,
            "@echo off\r\n\"%GAL_TEST_EXE%\" --exact commands::dispatch::tests::compatibility_pipeline_contract_windows_external_cutoff_kills_owned_tree_and_retains_evidence --nocapture\r\n",
        )
        .unwrap();
        std::fs::write(
            config_dir.join("config.json"),
            r#"{"executorRouting":{"executors":{},"pipeline":{"CODER":{"executor":"gal-pipeline-cutoff-fixture.cmd","timeoutSecs":120}}}}"#,
        )
        .unwrap();

        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut search_path = bin_dir.as_os_str().to_os_string();
        search_path.push(";");
        search_path.push(std::env::var_os("PATH").unwrap_or_default());
        let mut harness = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "commands::dispatch::tests::compatibility_pipeline_contract_windows_external_cutoff_kills_owned_tree_and_retains_evidence",
                "--nocapture",
            ])
            .env(CHILD_MARKER, "child")
            .env(CHILD_WORKDIR, &workdir)
            .env(CHILD_PROMPT, &prompt)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("PATH", search_path)
            .env("PSModuleAnalysisCachePath", sandbox.path().join("module-analysis.cache"))
            .env("GAL_TEST_EXE", std::env::current_exe().unwrap())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        let mut unrelated = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "commands::dispatch::tests::compatibility_pipeline_contract_windows_external_cutoff_kills_owned_tree_and_retains_evidence", "--nocapture"])
            .env("GAL_TEST_MODE", "unrelated")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        let start_deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while (!started.is_file() || !child_pid_file.is_file())
            && std::time::Instant::now() < start_deadline
        {
            assert!(
                harness.try_wait().unwrap().is_none(),
                "child harness exited before executor start"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(
            started.is_file(),
            "fake executor must signal its synchronized start"
        );
        assert!(
            child_pid_file.is_file(),
            "fake executor must start its descendant"
        );
        let child_pid = std::fs::read_to_string(&child_pid_file)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        let executor_pid = std::fs::read_to_string(&executor_pid_file)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();

        harness.kill().unwrap();
        let _ = harness.wait();
        let cleanup_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while (process_exists(child_pid) || process_exists(executor_pid))
            && std::time::Instant::now() < cleanup_deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(
            !process_exists(child_pid),
            "owned descendant must exit after harness cutoff"
        );
        assert!(
            !process_exists(executor_pid),
            "fake executor process must exit after harness cutoff"
        );
        assert!(
            unrelated.try_wait().unwrap().is_none(),
            "unrelated process must survive cutoff"
        );
        assert_eq!(std::fs::read_to_string(&dirty).unwrap(), "dirty");
        assert_eq!(std::fs::read_to_string(&untracked).unwrap(), "untracked");
        assert!(
            started.is_file(),
            "executor start signal must remain for recovery evidence"
        );
        assert!(
            workdir
                .join(dispatch::dispatch::LEASE_ROOT)
                .join("prompt-writers")
                .is_dir(),
            "cutoff must retain prompt lease evidence under the workdir"
        );
        assert!(
            std::fs::read_dir(
                workdir
                    .join(dispatch::dispatch::LEASE_ROOT)
                    .join("prompt-writers")
            )
            .unwrap()
            .flatten()
            .any(|entry| entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "lock")),
            "cutoff must retain the prompt lock file"
        );
        let logs = workdir.join(".dev/pipeline");
        fn has_started_log(path: &Path) -> bool {
            let Ok(entries) = std::fs::read_dir(path) else {
                return false;
            };
            entries.flatten().any(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    has_started_log(&path)
                } else {
                    std::fs::read_to_string(path)
                        .is_ok_and(|text| text.contains("terminal_state:  started"))
                }
            })
        }
        assert!(
            has_started_log(&logs),
            "started attempt log must remain on disk"
        );
        let _ = unrelated.kill();
        let _ = unrelated.wait();
    }

    #[cfg(windows)]
    #[test]
    fn compatibility_pipeline_contract_windows_silent_timeout_kills_owned_tree_and_preserves_unrelated_process(
    ) {
        use std::process::{Command, Stdio};

        if std::env::var("GAL_TEST_MODE").as_deref() == Ok("descendant") {
            std::fs::write(
                std::env::var_os("GAL_TEST_CHILD_PID").unwrap(),
                std::process::id().to_string(),
            )
            .unwrap();
            std::thread::sleep(std::time::Duration::from_secs(60));
            return;
        }
        if std::env::var("GAL_TEST_MODE").as_deref() == Ok("unrelated") {
            std::thread::sleep(std::time::Duration::from_secs(60));
            return;
        }
        if std::env::var("GAL_TEST_MODE").as_deref() == Ok("executor") {
            for (key, value) in [
                ("GAL_TEST_STARTED", "started"),
                ("GAL_TEST_DIRTY", "dirty"),
                ("GAL_TEST_UNTRACKED", "untracked"),
            ] {
                std::fs::write(std::env::var_os(key).unwrap(), value).unwrap();
            }
            std::fs::write(
                std::env::var_os("GAL_TEST_EXECUTOR_PID").unwrap(),
                std::process::id().to_string(),
            )
            .unwrap();
            let mut descendant = Command::new(std::env::var_os("GAL_TEST_EXE").unwrap())
                .args(["--exact", "commands::dispatch::tests::compatibility_pipeline_contract_windows_silent_timeout_kills_owned_tree_and_preserves_unrelated_process", "--nocapture"])
                .env("GAL_TEST_MODE", "descendant")
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !PathBuf::from(std::env::var_os("GAL_TEST_CHILD_PID").unwrap()).is_file()
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            assert!(
                PathBuf::from(std::env::var_os("GAL_TEST_CHILD_PID").unwrap()).is_file(),
                "descendant must report its PID"
            );
            let _ = descendant.wait();
            return;
        }

        struct EnvironmentRestore(
            Option<std::ffi::OsString>,
            Option<std::ffi::OsString>,
            Option<std::ffi::OsString>,
            Option<std::ffi::OsString>,
        );
        impl Drop for EnvironmentRestore {
            fn drop(&mut self) {
                match self.0.take() {
                    Some(value) => std::env::set_var("HOME", value),
                    None => std::env::remove_var("HOME"),
                }
                match self.1.take() {
                    Some(value) => std::env::set_var("PATH", value),
                    None => std::env::remove_var("PATH"),
                }
                match self.2.take() {
                    Some(value) => std::env::set_var("USERPROFILE", value),
                    None => std::env::remove_var("USERPROFILE"),
                }
                match self.3.take() {
                    Some(value) => std::env::set_var("PSModuleAnalysisCachePath", value),
                    None => std::env::remove_var("PSModuleAnalysisCachePath"),
                }
            }
        }

        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let sandbox = tempfile::TempDir::new().unwrap();
        let config_dir = sandbox.path().join(".gal/config");
        let bin_dir = sandbox.path().join("bin");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&bin_dir).unwrap();

        let child_pid_file = sandbox.path().join("owned-child.pid");
        let started_file = sandbox.path().join("executor-started");
        let executor_pid_file = sandbox.path().join("executor.pid");
        let dirty_output = sandbox.path().join("work/dirty-output.txt");
        let untracked_output = sandbox.path().join("work/untracked-output.txt");
        let executor = bin_dir.join("gal-pipeline-silent-fixture.cmd");
        let script = "@echo off\r\n\"%GAL_TEST_EXE%\" --exact commands::dispatch::tests::compatibility_pipeline_contract_windows_silent_timeout_kills_owned_tree_and_preserves_unrelated_process --nocapture\r\n";
        std::fs::write(&executor, script).unwrap();
        std::fs::write(
            config_dir.join("config.json"),
            r#"{"executorRouting":{"executors":{},"pipeline":{"CODER":{"executor":"gal-pipeline-silent-fixture.cmd","timeoutSecs":2}}}}"#,
        )
        .unwrap();

        let old_home = std::env::var_os("HOME");
        let old_path = std::env::var_os("PATH");
        let old_userprofile = std::env::var_os("USERPROFILE");
        let old_module_cache = std::env::var_os("PSModuleAnalysisCachePath");
        let _restore = EnvironmentRestore(old_home, old_path, old_userprofile, old_module_cache);
        std::env::set_var("HOME", sandbox.path());
        std::env::set_var("USERPROFILE", sandbox.path());
        std::env::set_var(
            "PSModuleAnalysisCachePath",
            sandbox.path().join("module-analysis.cache"),
        );
        let mut search_path = bin_dir.as_os_str().to_os_string();
        search_path.push(";");
        search_path.push(std::env::var_os("PATH").unwrap_or_default());
        std::env::set_var("PATH", search_path);
        let old_fixture_env: Vec<_> = [
            "GAL_TEST_EXE",
            "GAL_TEST_MODE",
            "GAL_TEST_STARTED",
            "GAL_TEST_EXECUTOR_PID",
            "GAL_TEST_CHILD_PID",
            "GAL_TEST_DIRTY",
            "GAL_TEST_UNTRACKED",
        ]
        .iter()
        .map(|key| (*key, std::env::var_os(key)))
        .collect();
        struct FixtureEnvRestore(Vec<(&'static str, Option<std::ffi::OsString>)>);
        impl Drop for FixtureEnvRestore {
            fn drop(&mut self) {
                for (key, value) in self.0.drain(..) {
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
        let _fixture_env_restore = FixtureEnvRestore(old_fixture_env);
        std::env::set_var("GAL_TEST_EXE", std::env::current_exe().unwrap());
        std::env::set_var("GAL_TEST_MODE", "executor");
        std::env::set_var("GAL_TEST_STARTED", &started_file);
        std::env::set_var("GAL_TEST_EXECUTOR_PID", &executor_pid_file);
        std::env::set_var("GAL_TEST_CHILD_PID", &child_pid_file);
        std::env::set_var("GAL_TEST_DIRTY", &dirty_output);
        std::env::set_var("GAL_TEST_UNTRACKED", &untracked_output);

        let workdir = sandbox.path().join("work");
        let prompt = workdir.join(".dev/plans/silent-timeout.prompt.md");
        std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        let task = tid(7);
        std::fs::write(
            &prompt,
            format!("## Tasks\n\n- [ ] {task} — timeout fixture\n"),
        )
        .unwrap();

        let mut unrelated = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "commands::dispatch::tests::compatibility_pipeline_contract_windows_silent_timeout_kills_owned_tree_and_preserves_unrelated_process", "--nocapture"])
            .env("GAL_TEST_MODE", "unrelated")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let args = vec![
            "pipeline".to_string(),
            prompt.display().to_string(),
            "--phase".to_string(),
            "implement".to_string(),
            "--task".to_string(),
            task,
            "--workdir".to_string(),
            workdir.display().to_string(),
        ];
        let started = std::time::Instant::now();
        let exit = cmd_pipeline(&args);
        let elapsed = started.elapsed();

        assert_eq!(
            exit,
            ExitCode::NotWired,
            "silent fake executor must time out"
        );
        assert!(
            started_file.is_file(),
            "executor reached its synchronized start"
        );
        assert_eq!(std::fs::read_to_string(&dirty_output).unwrap(), "dirty");
        assert_eq!(
            std::fs::read_to_string(&untracked_output).unwrap(),
            "untracked"
        );
        assert!(
            workdir
                .join(".dev/pipeline")
                .join(prompt.file_stem().unwrap())
                .exists()
                || workdir.join(".dev/pipeline").exists(),
            "timeout must retain the workdir recovery area"
        );
        assert!(
            child_pid_file.is_file(),
            "executor recorded its owned child"
        );
        assert!(elapsed < std::time::Duration::from_secs(15));
        assert!(
            unrelated.try_wait().unwrap().is_none(),
            "unrelated process survived cleanup"
        );
        let child_pid = std::fs::read_to_string(&child_pid_file)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        let executor_pid = std::fs::read_to_string(&executor_pid_file)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        let process_exists = |pid: u32| {
            Command::new("tasklist.exe")
                .args(["/FI", &format!("PID eq {pid}"), "/NH"])
                .output()
                .is_ok_and(|output| {
                    String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
                })
        };
        let cleanup_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while (process_exists(child_pid) || process_exists(executor_pid))
            && std::time::Instant::now() < cleanup_deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(
            !process_exists(child_pid),
            "owned descendant must exit within cleanup bound"
        );
        assert!(
            !process_exists(executor_pid),
            "executor process must exit within cleanup bound"
        );
        let _ = unrelated.kill();
        let _ = unrelated.wait();
    }

    // ── `gal research <slug> --worker 0` must be rejected, explaining that `#0`
    // is always a native subagent, never dispatched. Locked seam: the argument
    // parsing and entry point of `gal research <slug> --worker <1|2>` — no such
    // subcommand is wired into `crate::run` yet (`CommandKind::Research` has no
    // `Action::NotWired` arm), so this probe is expected red: the entry point
    // does not exist to reach a worker-0 check at all.
    //
    // Called in-process via `crate::run` (`gal-cli` is a bin-only crate with no
    // lib target, so there is no `CARGO_BIN_EXE_gal` available to a unit test
    // compiled as part of that same bin's own test harness, and the crate
    // carries no stdio-capture dependency to assert on printed wording without
    // touching the frozen production seam or Cargo.toml, out of this task's
    // allowlist). The exit-code check below is the observable half of the
    // acceptance criteria reachable from here; the "`#0` is always a native
    // subagent" wording is verified by re-running this probe by hand against
    // the implementer's change (see receipt).
    #[test]
    fn gal_research_subcommand_rejects_worker_0() {
        let args = vec![
            "research".to_string(),
            "some-slug".to_string(),
            "--worker".to_string(),
            "0".to_string(),
        ];
        let exit = crate::run(&args);

        assert_eq!(
            exit,
            ExitCode::Usage,
            "gal research subcommand rejects --worker 0"
        );
    }

    // ── a routed research receipt is named `investigate-<n>.receipt.md`
    // under the research scope, and carries two mandatory fields: execution mode,
    // and the winning resolution rule (`resolve_worker_route`'s `winning_rule`).
    // Mode is mandatory so a subagent run is never mistaken for a routed run.
    // Locked seam: the research receipt writer and its mandatory field set in
    // `cmd_research` — no such writer, naming, or field set exists yet, so this
    // probe is expected red.
    //
    // A full dispatch would spawn a real executor CLI, unavailable in this
    // sandbox, so the executor's own completion content is not observable here.
    // What `cmd_research` must do deterministically, before ever attempting to
    // spawn anything, is pre-write a scoped receipt skeleton under the new name
    // carrying both mandatory fields — that skeleton is the observable half of
    // the acceptance criteria reachable from here.
    #[test]
    fn gal_research_receipt_names_mode_and_winning_rule() {
        let tmp = tempfile::TempDir::new().unwrap();
        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let args = vec![
            "research".to_string(),
            "t10-probe-slug".to_string(),
            "--worker".to_string(),
            "2".to_string(),
        ];
        let _ = cmd_research(&args);

        std::env::set_current_dir(&original_cwd).unwrap();

        let receipt_path = tmp
            .path()
            .join(".dev/pipeline/t10-probe-slug/worker-2/investigate-2.receipt.md");
        let content = std::fs::read_to_string(&receipt_path).unwrap_or_default();
        assert!(
            receipt_path.exists() && content.contains("mode") && content.contains("winning_rule"),
            "routed research receipt names mode and winning rule"
        );
    }

    #[test]
    fn research_receipt_regains_mandatory_fields_after_executor_overwrite() {
        // Reproduces the case `run_dispatch`'s own freshness precondition creates:
        // the pre-dispatch skeleton is gone and a spawned executor has written its
        // own free-form content with no `mode`/`winning_rule`. The post-dispatch
        // merge must restore both fields without erasing the executor's own text.
        let tmp = tempfile::TempDir::new().unwrap();
        let receipt_path = tmp.path().join("investigate-2.receipt.md");
        std::fs::write(&receipt_path, "executor findings: nothing notable\n").unwrap();

        ensure_research_receipt_mandatory_fields(
            receipt_path.to_str().unwrap(),
            "worker-2",
            "some-slug",
            "routed",
            "explicit-research",
        );

        let content = std::fs::read_to_string(&receipt_path).unwrap();
        assert!(
            content.contains("executor findings: nothing notable"),
            "executor's own content must survive the merge, got: {content}"
        );
        assert!(content.contains("mode: routed"));
        assert!(content.contains("winning_rule: explicit-research"));
    }

    #[test]
    fn research_receipt_is_written_fresh_when_dispatch_leaves_no_file() {
        // Reproduces a `no-receipt` / pre-spawn-degrade outcome: the file is
        // entirely absent after `run_dispatch` returns. The merge must still land
        // both mandatory fields.
        let tmp = tempfile::TempDir::new().unwrap();
        let receipt_path = tmp.path().join("investigate-1.receipt.md");
        assert!(!receipt_path.exists());

        ensure_research_receipt_mandatory_fields(
            receipt_path.to_str().unwrap(),
            "worker-1",
            "some-slug",
            "subagent",
            "native-subagent",
        );

        let content = std::fs::read_to_string(&receipt_path).unwrap();
        assert!(content.contains("mode: subagent"));
        assert!(content.contains("winning_rule: native-subagent"));
    }
}

#[cfg(test)]
mod finalize_review_tests {
    //! Production-path fixtures for `gal pipeline --phase finalize-review`. A compiled
    //! fake `agy` executor stands in for the AUDITOR route: it records its stdin and
    //! every launch, then writes the review receipt the test chose. No network, no
    //! real account.
    use super::*;
    use std::ffi::OsString;
    use std::process::Command;
    use std::sync::OnceLock;
    use tempfile::TempDir;

    const PLAN: &str = "demo-plan";

    // Task ids are composed at runtime, never written as literals.
    fn tid(n: u32) -> String {
        format!("T-{:03}", n)
    }

    static DOUBLE_DIR: OnceLock<TempDir> = OnceLock::new();

    fn mock_double_dir() -> &'static Path {
        DOUBLE_DIR
            .get_or_init(|| {
                let tmp = TempDir::new().expect("double dir");
                let src_path = tmp.path().join("mock_double.rs");
                let src = r##"
use std::io::{Read, Write};
fn main() {
    if let Ok(path) = std::env::var("MOCK_LAUNCH_FILE") {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "launch");
        }
    }
    if let Ok(path) = std::env::var("MOCK_CAPTURE_STDIN") {
        let mut input = String::new();
        let _ = std::io::stdin().read_to_string(&mut input);
        let _ = std::fs::write(path, input);
    }
    if let Ok(path) = std::env::var("MOCK_RECEIPT_WRITE_PATH") {
        let body = std::env::var("MOCK_RECEIPT_BODY").unwrap_or_default();
        let _ = std::fs::write(path, body);
    }
    if let Ok(file) = std::env::var("MOCK_PAYLOAD_FILE") {
        if let Ok(content) = std::fs::read(&file) {
            let _ = std::io::stdout().write_all(&content);
            return;
        }
    }
    println!("{{\"conversation_id\":\"5b0f7c1e-3a64-4c1b-9d52-0e8a1f6b7c24\",\"status\":\"SUCCESS\"}}");
}
"##;
                std::fs::write(&src_path, src).unwrap();
                let exe = if cfg!(windows) { "agy.exe" } else { "agy" };
                let status = Command::new("rustc")
                    .arg(&src_path)
                    .arg("-o")
                    .arg(tmp.path().join(exe))
                    .status()
                    .expect("compile the fake executor");
                assert!(status.success(), "rustc must build the fake executor");
                tmp
            })
            .path()
    }

    const ENV_NAMES: [&str; 9] = [
        "HOME",
        "USERPROFILE",
        "PATH",
        "GEMINI_API_KEY",
        "MOCK_LAUNCH_FILE",
        "MOCK_CAPTURE_STDIN",
        "MOCK_RECEIPT_WRITE_PATH",
        "MOCK_RECEIPT_BODY",
        "MOCK_PAYLOAD_FILE",
    ];

    struct ReviewFixture {
        _guard: std::sync::MutexGuard<'static, ()>,
        sandbox: TempDir,
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    impl Drop for ReviewFixture {
        fn drop(&mut self) {
            for (name, value) in self.saved.drain(..) {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    fn git_in(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// The marked reviewer fragment sits between orchestrator-only text on both sides.
    fn finalize_command_body() -> String {
        format!(
            "ORCHESTRATOR-DISPATCH-STEP lives outside the bounds.\n\n{}\n\n\
             REVIEWER-FRAGMENT-SENTINEL\n\n**Permitted inputs.** The plan Goal.\n\n\
             {}\n\nORCHESTRATOR-REPAIR-STEP merges and tags.\n",
            pipeline::task_spec::FINALIZE_REVIEW_CONTRACT_START,
            pipeline::task_spec::FINALIZE_REVIEW_CONTRACT_END
        )
    }

    fn source_plan_text() -> String {
        "# Plan: Demo\n\n\
         ## Approval\n\nSOURCE-APPROVAL-SENTINEL\n\n\
         ## Goal\n\nSOURCE-GOAL-SENTINEL land the demo.\n\n\
         ## Requirements\n\n- [ ] R1 — first requirement\n- [ ] R2 — second requirement\n\n\
         ## Success Criteria\n\n- It lands.\n\n\
         ## Test Plan\n\n| ID | Type |\n| --- | --- |\n| PROBE-A | unit |\n\n\
         ## Review Results\n\nSOURCE-REVIEW-SENTINEL\n"
            .to_string()
    }

    fn prompt_text() -> String {
        let task = tid(1);
        format!(
            "# Plan Prompt: Demo\n\n\
             ## Goal\n\nPROMPT-GOAL-SENTINEL\n\n\
             ## Requirements\n\n- [ ] R1 — first requirement\n- [ ] R2 — second requirement\n\n\
             ## Status\n\nWorkflow: DONE\nPROMPT-STATUS-SENTINEL\n\n\
             ### Handoff Notes\n\nPROMPT-HANDOFF-SENTINEL\n\n\
             ## Tasks\n\n- [x] {task} — finished work\n\n\
             ## Test Results\n\nPROMPT-TEST-RESULT-SENTINEL\n\n\
             ## Review Results\n\n### Architecture Review\n\nPROMPT-ARCH-REVIEW-SENTINEL\n"
        )
    }

    const ROW: &str = "| R2 | PASS — reviewed against the diff | PASS — file exists | PASS — wired | N/A — no trust boundary |\n";

    /// A complete review for the two fixture requirements.
    fn review_body(note: &str, independence: &str) -> String {
        format!(
            "### Finalize Review 2026-10-08\n\n\
             | Requirement | L1 | L2 | L3 | L4 |\n\
             | --- | --- | --- | --- | --- |\n\
             | R1 | PASS — reviewed against the diff | PASS — file exists | PASS — wired | N/A — no trust boundary |\n\
             {ROW}\n\
             Findings: {note}\n\n\
             Review Independence: {independence}\n"
        )
    }

    impl ReviewFixture {
        fn new() -> Self {
            let guard = crate::commands::ENV_GUARD
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let saved: Vec<(&'static str, Option<OsString>)> = ENV_NAMES
                .iter()
                .map(|name| (*name, std::env::var_os(name)))
                .collect();
            let original_path = saved
                .iter()
                .find(|(name, _)| *name == "PATH")
                .and_then(|(_, value)| value.clone())
                .unwrap_or_default();
            let fixture = Self {
                _guard: guard,
                sandbox: TempDir::new().unwrap(),
                saved,
            };
            let home = fixture.home();
            std::fs::create_dir_all(home.join(".gal/config")).unwrap();
            std::env::set_var("HOME", &home);
            std::env::set_var("USERPROFILE", &home);
            std::env::set_var("GEMINI_API_KEY", "mock-gemini-key");
            let mut paths = vec![mock_double_dir().to_path_buf()];
            paths.extend(std::env::split_paths(&original_path));
            std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
            for name in [
                "MOCK_RECEIPT_WRITE_PATH",
                "MOCK_RECEIPT_BODY",
                "MOCK_PAYLOAD_FILE",
            ] {
                std::env::remove_var(name);
            }
            std::env::set_var("MOCK_LAUNCH_FILE", fixture.launches());
            std::env::set_var("MOCK_CAPTURE_STDIN", fixture.captured_spec());

            let work = fixture.workdir();
            std::fs::create_dir_all(work.join(".dev/plans")).unwrap();
            for dir in ["skills", "agents"] {
                std::fs::create_dir_all(work.join("plugins/gal-core").join(dir)).unwrap();
            }
            let command_dir = work.join("plugins/gal-core/commands/gal-finalize");
            std::fs::create_dir_all(&command_dir).unwrap();
            std::fs::write(
                command_dir.join("SKILL.template.md"),
                finalize_command_body(),
            )
            .unwrap();
            std::fs::write(
                work.join(".dev/project.md"),
                "# Project\n\n<!-- gal:authoritative-check -->\n```json\n{\"command\":[\"cargo test --workspace\"]}\n```\n",
            )
            .unwrap();
            std::fs::write(fixture.source(), source_plan_text()).unwrap();
            std::fs::write(fixture.prompt(), prompt_text()).unwrap();
            git_in(&work, &["init", "-q"]);
            git_in(&work, &["config", "user.email", "fixture@example.invalid"]);
            git_in(&work, &["config", "user.name", "Fixture"]);
            git_in(&work, &["config", "commit.gpgsign", "false"]);
            let hooks = fixture.sandbox.path().join("empty-hooks");
            std::fs::create_dir_all(&hooks).unwrap();
            git_in(
                &work,
                &["config", "core.hooksPath", hooks.to_str().unwrap()],
            );
            git_in(&work, &["add", "."]);
            git_in(&work, &["commit", "-q", "-m", "fixture"]);
            fixture
        }

        fn home(&self) -> PathBuf {
            self.sandbox.path().join("home")
        }
        fn workdir(&self) -> PathBuf {
            self.sandbox.path().join("work")
        }
        fn prompt(&self) -> PathBuf {
            self.workdir().join(format!(".dev/plans/{PLAN}.prompt.md"))
        }
        fn source(&self) -> PathBuf {
            self.workdir().join(format!(".dev/plans/{PLAN}.md"))
        }
        fn coordinator(&self) -> PathBuf {
            self.workdir()
                .join(format!(".dev/pipeline/{PLAN}/coordinator.json"))
        }
        fn receipt(&self) -> PathBuf {
            self.workdir().join(format!(
                ".dev/pipeline/{PLAN}/finalize-review/finalize-review.receipt.md"
            ))
        }
        fn launches(&self) -> PathBuf {
            self.sandbox.path().join("launches.txt")
        }
        fn captured_spec(&self) -> PathBuf {
            self.sandbox.path().join("captured-spec.md")
        }
        fn launch_count(&self) -> usize {
            std::fs::read_to_string(self.launches())
                .map(|text| text.lines().count())
                .unwrap_or(0)
        }

        fn set_routing(&self, json: &str) {
            std::fs::write(self.home().join(".gal/config/config.json"), json).unwrap();
        }
        fn route_auditor_to_fake(&self) {
            self.set_routing(
                r#"{"executorRouting":{"executors":{"agy":"gemini-2.5-pro"},"pipeline":{"AUDITOR":{"executor":"agy"}}}}"#,
            );
        }
        fn executor_writes(&self, body: &str) {
            std::env::set_var("MOCK_RECEIPT_WRITE_PATH", self.receipt());
            std::env::set_var("MOCK_RECEIPT_BODY", body);
        }

        fn run_with(&self, input: &Path, extra: &[&str]) -> ExitCode {
            let mut args = vec![
                "pipeline".to_string(),
                input.display().to_string(),
                "--phase".to_string(),
                "finalize-review".to_string(),
                "--workdir".to_string(),
                self.workdir().display().to_string(),
            ];
            args.extend(extra.iter().map(|s| s.to_string()));
            cmd_pipeline(&args)
        }
        fn run(&self) -> ExitCode {
            self.run_with(&self.prompt(), &[])
        }

        fn prompt_bytes(&self) -> Vec<u8> {
            std::fs::read(self.prompt()).unwrap()
        }
        fn prompt_string(&self) -> String {
            String::from_utf8(self.prompt_bytes()).unwrap()
        }
    }

    impl ReviewFixture {
        fn pin_host(&self) {
            let path = self
                .workdir()
                .join(format!(".dev/pipeline/{PLAN}/preflight.receipt.md"));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(
                path,
                "host: claude\ndispatch_mode: host-subagent\nrun_id: run-1\n",
            )
            .unwrap();
        }

        fn open_host(&self) -> dispatch::host_attempt_state::HostAttemptState {
            self.pin_host();
            let before = self.prompt_bytes();
            assert_eq!(
                self.run_with(&self.prompt(), &["--dispatch", "host-subagent"]),
                ExitCode::Success
            );
            assert_eq!(self.prompt_bytes(), before);
            assert_eq!(self.launch_count(), 0);
            let dir = self.workdir().join(format!(
                ".dev/pipeline/{PLAN}/finalize-review/host-attempts"
            ));
            let path = std::fs::read_dir(dir)
                .unwrap()
                .flatten()
                .map(|entry| entry.path())
                .find(|path| path.extension().is_some_and(|ext| ext == "json"))
                .unwrap();
            let id = path.file_stem().unwrap().to_str().unwrap();
            dispatch::host_attempt_state::HostAttemptState::load(&path, id).unwrap()
        }

        fn host_session(&self, state: &dispatch::host_attempt_state::HostAttemptState) -> String {
            let dir = self.home().join(".claude/projects/demo/session/subagents");
            std::fs::create_dir_all(&dir).unwrap();
            let user = serde_json::json!({"type":"user", "cwd":self.workdir(),
                "timestamp":state.started_at, "message":{"content":state.attempt_id}});
            let assistant = serde_json::json!({"type":"assistant", "message":{
                "model":state.model, "content":[{"type":"tool_use", "name":"Read"}]}});
            std::fs::write(
                dir.join("agent-reviewer.jsonl"),
                format!("{user}\n{assistant}\n"),
            )
            .unwrap();
            std::fs::write(
                dir.join("agent-reviewer.meta.json"),
                r#"{"agentType":"general-purpose"}"#,
            )
            .unwrap();
            let reference =
                dispatch::host_session::HostSessionRef::parse("claude:reviewer").unwrap();
            dispatch::host_session::verify(
                &reference,
                &self.workdir(),
                &self.home(),
                &state.started_at,
                &state.attempt_id,
                FINALIZE_REVIEW_PHASE,
                projection::ORCHESTRATED_ONLY_GOLEM_NAMES,
            )
            .expect("valid host session fixture");
            "claude:reviewer".into()
        }

        fn close_host(
            &self,
            state: &dispatch::host_attempt_state::HostAttemptState,
            independence: &str,
        ) -> ExitCode {
            let session = self.host_session(state);
            std::fs::write(self.receipt(), review_body("host review", independence)).unwrap();
            self.run_with(
                &self.prompt(),
                &["--close", &state.attempt_id, "--session", &session],
            )
        }
    }

    #[test]
    fn finalize_review_host_open_preserves_prompt() {
        let f = ReviewFixture::new();
        let state = f.open_host();
        assert_eq!(state.phase, FINALIZE_REVIEW_PHASE);
        assert_eq!(state.task, FINALIZE_REVIEW_PHASE);
        assert_eq!(state.tier.as_deref(), Some("deep"));
        assert!(std::fs::read_to_string(state.log_path)
            .unwrap()
            .contains("terminal_state:  started"));
    }

    #[test]
    fn finalize_review_host_close_places_review_and_completes() {
        let f = ReviewFixture::new();
        let state = f.open_host();
        assert_eq!(
            f.close_host(&state, "DEGRADED_HOST_SUBAGENT"),
            ExitCode::Success
        );
        assert_eq!(block_count(&f.prompt_string()), 1);
        assert!(f
            .prompt_string()
            .contains("Review Independence: DEGRADED_HOST_SUBAGENT"));
        let log = std::fs::read_to_string(state.log_path).unwrap();
        assert!(log.contains("terminal_state:  completed"), "{log}");
    }

    fn reject_host_independence(independence: &str) {
        let f = ReviewFixture::new();
        let state = f.open_host();
        let before = f.prompt_bytes();
        assert_eq!(f.close_host(&state, independence), ExitCode::Error);
        assert_eq!(f.prompt_bytes(), before);
        let log = std::fs::read_to_string(state.log_path).unwrap();
        assert!(!log.contains("terminal_state:  completed"), "{log}");
        assert!(log.contains("terminal_state:  started"), "{log}");
    }

    #[test]
    fn finalize_review_host_close_rejects_full() {
        reject_host_independence("full");
    }

    #[test]
    fn finalize_review_host_close_rejects_same_runtime() {
        reject_host_independence("DEGRADED_SAME_RUNTIME");
    }

    #[test]
    fn finalize_review_host_pin_keeps_auditor_route_full() {
        let f = ReviewFixture::new();
        f.pin_host();
        f.route_auditor_to_fake();
        f.executor_writes(&review_body("routed", "full"));
        assert_eq!(
            f.run_with(&f.prompt(), &["--dispatch", "host-subagent"]),
            ExitCode::Success
        );
        assert_eq!(f.launch_count(), 1);
        assert!(f.prompt_string().contains("Review Independence: full"));
        let before = f.prompt_bytes();
        f.executor_writes(&review_body("wrong independence", "DEGRADED_HOST_SUBAGENT"));
        assert_eq!(f.run(), ExitCode::Error);
        assert_eq!(f.launch_count(), 2);
        assert_eq!(f.prompt_bytes(), before);
    }

    #[test]
    fn finalize_review_host_absent_route_and_pin_stays_not_wired() {
        let f = ReviewFixture::new();
        let before = f.prompt_bytes();
        assert_eq!(f.run(), ExitCode::NotWired);
        assert_eq!(f.prompt_bytes(), before);
        assert_eq!(f.launch_count(), 0);
    }

    fn block_count(prompt: &str) -> usize {
        prompt.matches("### Finalize Review ").count()
    }

    #[test]
    fn both_input_forms_place_one_scoped_review_and_leave_everything_else() {
        let f = ReviewFixture::new();
        f.route_auditor_to_fake();
        let prompt_before = f.prompt_string();
        let source_before = std::fs::read(f.source()).unwrap();

        f.executor_writes(&review_body("first pass", "full"));
        assert_eq!(f.run(), ExitCode::Success);
        let first = f.prompt_string();
        assert_eq!(block_count(&first), 1);
        assert!(first.starts_with(prompt_before.trim_end()), "{first}");
        assert!(first.contains("Findings: first pass"));
        assert!(first.contains("PROMPT-ARCH-REVIEW-SENTINEL"));
        assert_eq!(std::fs::read(f.source()).unwrap(), source_before);
        assert!(
            !f.coordinator().exists(),
            "a review never creates a coordinator"
        );
        assert_eq!(f.launch_count(), 1);

        // The scoped attempt log records a completed attempt.
        let log_dir = f
            .workdir()
            .join(format!(".dev/pipeline/{PLAN}/finalize-review"));
        let completed = std::fs::read_dir(&log_dir)
            .unwrap()
            .flatten()
            .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
            .any(|text| text.contains("terminal_state:  completed"));
        assert!(
            completed,
            "no completed attempt log under {}",
            log_dir.display()
        );

        // The paired source plan form re-reviews and replaces only the managed block.
        f.executor_writes(&review_body("second pass", "full"));
        assert_eq!(f.run_with(&f.source(), &[]), ExitCode::Success);
        let second = f.prompt_string();
        assert_eq!(block_count(&second), 1);
        assert!(second.contains("Findings: second pass"));
        assert!(!second.contains("Findings: first pass"));
        assert!(second.starts_with(prompt_before.trim_end()), "{second}");
        assert!(second.contains("PROMPT-ARCH-REVIEW-SENTINEL"));
        assert_eq!(std::fs::read(f.source()).unwrap(), source_before);
        assert_eq!(f.launch_count(), 2);
    }

    #[test]
    fn executor_input_is_the_bounded_fragment_and_permitted_sections_only() {
        let f = ReviewFixture::new();
        f.route_auditor_to_fake();
        f.executor_writes(&review_body("none", "full"));
        assert_eq!(f.run(), ExitCode::Success);

        let spec = std::fs::read_to_string(f.captured_spec()).unwrap();
        for needed in [
            "REVIEWER-FRAGMENT-SENTINEL",
            "SOURCE-GOAL-SENTINEL",
            "R1 — first requirement",
            "It lands.",
            "| PROBE-A | unit |",
            "cargo test --workspace",
            "finalize-review.receipt.md",
        ] {
            assert!(spec.contains(needed), "spec lacks {needed}");
        }
        for forbidden in [
            "PROMPT-GOAL-SENTINEL",
            "PROMPT-STATUS-SENTINEL",
            "PROMPT-HANDOFF-SENTINEL",
            "PROMPT-TEST-RESULT-SENTINEL",
            "PROMPT-ARCH-REVIEW-SENTINEL",
            "SOURCE-APPROVAL-SENTINEL",
            "SOURCE-REVIEW-SENTINEL",
            "ORCHESTRATOR-DISPATCH-STEP",
            "ORCHESTRATOR-REPAIR-STEP",
            "Affected Files",
            "Task Goal",
            "golem-auditor",
        ] {
            assert!(!spec.contains(forbidden), "spec leaks {forbidden}");
        }
    }

    #[test]
    fn unsupported_arguments_and_missing_pairs_stop_before_launch() {
        let f = ReviewFixture::new();
        f.route_auditor_to_fake();
        f.executor_writes(&review_body("none", "full"));
        let prompt_before = f.prompt_bytes();
        let task = tid(1);
        let raw_spec = f.workdir().join("raw-task-spec.md");
        std::fs::write(&raw_spec, "## Task Goal\n\nRaw.\n").unwrap();

        assert_eq!(f.run_with(&f.prompt(), &["--task", &task]), ExitCode::Usage);
        assert_eq!(f.run_with(&f.prompt(), &["--fix"]), ExitCode::Usage);
        assert_eq!(
            f.run_with(&f.prompt(), &["--receipt", "elsewhere.md"]),
            ExitCode::Usage
        );
        assert_eq!(f.run_with(&raw_spec, &[]), ExitCode::Usage);
        assert_eq!(
            f.run_with(&f.prompt(), &["--require-codex-stop-v1"]),
            ExitCode::Usage
        );

        // A prompt with no source plan, and a source plan with no prompt.
        let moved_source = f.workdir().join("moved-source.md");
        std::fs::rename(f.source(), &moved_source).unwrap();
        assert_eq!(f.run(), ExitCode::Error);
        std::fs::rename(&moved_source, f.source()).unwrap();
        let moved_prompt = f.workdir().join("moved.prompt.md");
        std::fs::rename(f.prompt(), &moved_prompt).unwrap();
        assert_eq!(f.run_with(&f.source(), &[]), ExitCode::Error);
        std::fs::rename(&moved_prompt, f.prompt()).unwrap();

        assert_eq!(f.launch_count(), 0, "no executor may start");
        assert_eq!(f.prompt_bytes(), prompt_before);
        assert!(!f.coordinator().exists());
    }

    #[test]
    fn phase_validation_accepts_the_new_phase_and_keeps_the_old_ones() {
        assert_eq!(validate_pipeline_phase("finalize-review"), Ok(()));
        assert_eq!(validate_pipeline_phase("Finalize-Review"), Ok(()));
        for phase in ["implement", "test", "audit"] {
            assert_eq!(validate_pipeline_phase(phase), Ok(()));
        }
        assert!(validate_pipeline_phase("investigate").is_err());
        let message = validate_pipeline_phase("review").unwrap_err();
        assert!(message.contains("finalize-review"), "{message}");
        let argv = |phase: &str| -> Vec<String> {
            ["pipeline", "p.prompt.md", "--phase", phase]
                .iter()
                .map(|s| s.to_string())
                .collect()
        };
        assert!(requests_finalize_review(&argv("FINALIZE-REVIEW")));
        assert!(!requests_finalize_review(&argv("audit")));
    }

    #[test]
    fn coordinator_binding_is_verified_never_rewritten() {
        let f = ReviewFixture::new();
        f.route_auditor_to_fake();
        f.executor_writes(&review_body("none", "full"));
        let prompt_before = f.prompt_bytes();

        let worktree = std::fs::canonicalize(
            super::super::pipeline_execution::resolve_worktree_root(&f.workdir()),
        )
        .unwrap();
        let executable = std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
        let good = super::super::pipeline_driver::coordinator_execution_binding(
            PLAN,
            &worktree,
            &executable,
        )
        .unwrap();
        let write_coordinator = |binding: pipeline::coordinator::ExecutionBinding| -> Vec<u8> {
            let mut state = pipeline::coordinator::CoordinatorState::new_legacy(
                PLAN,
                "prompt-hash",
                tid(1),
                "implement",
            );
            state.execution_binding = Some(binding);
            let bytes = serde_json::to_vec_pretty(&state).unwrap();
            std::fs::create_dir_all(f.coordinator().parent().unwrap()).unwrap();
            std::fs::write(f.coordinator(), &bytes).unwrap();
            bytes
        };

        // A binding for another executable stops the review before launch.
        let mut wrong = good.clone();
        wrong.executable_sha256 = "0".repeat(64);
        let wrong_bytes = write_coordinator(wrong);
        assert_eq!(f.run(), ExitCode::Error);
        assert_eq!(f.launch_count(), 0);
        assert_eq!(f.prompt_bytes(), prompt_before);
        assert_eq!(std::fs::read(f.coordinator()).unwrap(), wrong_bytes);

        // A malformed coordinator also stops it.
        std::fs::write(f.coordinator(), b"{ not json").unwrap();
        assert_eq!(f.run(), ExitCode::Error);
        assert_eq!(f.launch_count(), 0);

        // The matching binding lets the review run and the coordinator stays byte-identical.
        let good_bytes = write_coordinator(good);
        assert_eq!(f.run(), ExitCode::Success);
        assert_eq!(f.launch_count(), 1);
        assert_eq!(std::fs::read(f.coordinator()).unwrap(), good_bytes);
    }

    #[test]
    fn invalid_receipts_never_reach_the_prompt() {
        let f = ReviewFixture::new();
        f.route_auditor_to_fake();
        std::fs::create_dir_all(f.workdir().join("crates/fake/src")).unwrap();
        std::fs::write(
            f.workdir().join("crates/fake/src/lib.rs"),
            "fn real_probe() {}\n",
        )
        .unwrap();
        let prompt_before = f.prompt_bytes();
        let good = review_body("none", "full");
        let cases: Vec<(&str, String)> = vec![
            ("empty receipt", String::new()),
            ("no heading first", format!("Preface\n\n{good}")),
            (
                "wrong independence value",
                review_body("none", "DEGRADED_SAME_RUNTIME"),
            ),
            ("blank independence value", review_body("none", "")),
            (
                "duplicate independence line",
                format!("{good}Review Independence: full\n"),
            ),
            (
                "missing independence line",
                good.replace("Review Independence: full\n", ""),
            ),
            (
                "malformed cell",
                good.replacen("PASS — reviewed against the diff", "PASS", 1),
            ),
            ("short table", good.replace(ROW, "")),
            (
                "unresolved citation",
                good.replacen(
                    "PASS — reviewed against the diff",
                    "PASS — `missing_probe_name` proves it",
                    1,
                ),
            ),
            (
                "task heading",
                good.replace(
                    "### Finalize Review 2026-10-08",
                    &format!("### [{}] 2026-10-08", tid(1)),
                ),
            ),
            (
                "heading escape",
                format!("{good}\n## Tasks\n\n- [ ] injected\n"),
            ),
        ];
        let expected_launches = cases.len();
        for (label, body) in cases {
            f.executor_writes(&body);
            assert_ne!(f.run(), ExitCode::Success, "{label} must not succeed");
            assert_eq!(
                f.prompt_bytes(),
                prompt_before,
                "{label} changed the prompt"
            );
        }
        assert_eq!(f.launch_count(), expected_launches);

        // A resolvable citation in the same shape is accepted.
        f.executor_writes(&good.replacen(
            "PASS — reviewed against the diff",
            "PASS — `real_probe` proves it",
            1,
        ));
        assert_eq!(f.run(), ExitCode::Success);
        assert_eq!(block_count(&f.prompt_string()), 1);
    }

    #[test]
    fn a_stale_receipt_and_missing_session_evidence_never_place_a_review() {
        let f = ReviewFixture::new();
        f.route_auditor_to_fake();
        let prompt_before = f.prompt_bytes();

        // A receipt left by an earlier attempt is removed before launch, so an
        // executor that writes nothing cannot inherit it.
        std::fs::create_dir_all(f.receipt().parent().unwrap()).unwrap();
        std::fs::write(f.receipt(), review_body("stale", "full")).unwrap();
        assert_ne!(f.run(), ExitCode::Success);
        assert_eq!(f.prompt_bytes(), prompt_before);

        // A completed-looking run with no provider session object is not a review.
        f.executor_writes(&review_body("none", "full"));
        let payload = f.sandbox.path().join("payload.txt");
        std::fs::write(&payload, "plain text with no session object\n").unwrap();
        std::env::set_var("MOCK_PAYLOAD_FILE", &payload);
        assert_ne!(f.run(), ExitCode::Success);
        assert_eq!(f.prompt_bytes(), prompt_before);
    }

    #[test]
    fn a_receipt_changed_after_dispatch_is_refused() {
        let f = ReviewFixture::new();
        f.route_auditor_to_fake();
        f.executor_writes(&review_body("original", "full"));
        let prompt_before = f.prompt_bytes();
        let receipt = f.receipt();
        let tampered = review_body("tampered", "full");
        PIPELINE_AFTER_DISPATCH_TEST_HOOK.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move || {
                std::fs::write(&receipt, tampered).unwrap();
            }));
        });
        assert_eq!(f.run(), ExitCode::Error);
        assert_eq!(f.prompt_bytes(), prompt_before);
        assert!(
            dispatch::dispatch::acquire_prompt_writer_lease(
                &f.workdir(),
                &f.prompt(),
                None,
                "attempt-after-tamper"
            )
            .is_err(),
            "unverified receipt evidence must keep prompt ownership"
        );
    }

    #[test]
    fn a_failing_review_is_delivered_without_counting_as_approval() {
        let f = ReviewFixture::new();
        f.route_auditor_to_fake();
        let failing = review_body("BLOCKING: R1 is not met", "full").replacen(
            "R1 | PASS — reviewed",
            "R1 | FAIL — not met, reviewed",
            1,
        );
        f.executor_writes(&failing);
        assert_eq!(f.run(), ExitCode::Success);
        let placed = f.prompt_string();
        assert_eq!(block_count(&placed), 1);
        assert!(placed.contains("FAIL — not met"));
        assert!(placed.contains("BLOCKING: R1 is not met"));
        let (head, _) = placed.split_once("### Finalize Review ").unwrap();
        assert!(prompt_text().starts_with(head.trim_end()));
    }

    #[test]
    fn only_an_absent_route_may_degrade() {
        let f = ReviewFixture::new();
        let prompt_before = f.prompt_bytes();

        // No config file at all, and a config with no AUDITOR entry, are absent routes.
        assert_eq!(f.run(), ExitCode::NotWired);
        f.set_routing(
            r#"{"executorRouting":{"executors":{"agy":"m"},"pipeline":{"CODER":{"executor":"agy"}}}}"#,
        );
        assert_eq!(f.run(), ExitCode::NotWired);

        // Present but unusable routes stop the review. None may look like an absent route.
        for (label, json) in [
            ("invalid JSON", "{ not json"),
            (
                "pipeline not an object",
                r#"{"executorRouting":{"pipeline":5}}"#,
            ),
            (
                "selected entry malformed",
                r#"{"executorRouting":{"executors":{},"pipeline":{"AUDITOR":5}}}"#,
            ),
            (
                "executor missing from PATH",
                r#"{"executorRouting":{"executors":{},"pipeline":{"AUDITOR":{"executor":"gal-no-such-review-executor"}}}}"#,
            ),
        ] {
            f.set_routing(json);
            assert_eq!(f.run(), ExitCode::Error, "{label} must stop, not degrade");
        }
        assert_eq!(f.launch_count(), 0);
        assert_eq!(f.prompt_bytes(), prompt_before);
    }

    #[test]
    fn a_review_that_writes_no_receipt_stops_without_fallback() {
        let f = ReviewFixture::new();
        f.set_routing(
            r#"{"executorRouting":{"executors":{"agy":"gemini-2.5-pro"},"pipeline":{"AUDITOR":{"executor":"agy","timeoutSecs":1}}}}"#,
        );
        let prompt_before = f.prompt_bytes();
        // The executor runs but writes no receipt, so there is nothing to place.
        assert_eq!(f.run(), ExitCode::Error);
        assert_eq!(f.prompt_bytes(), prompt_before);
        assert_eq!(f.launch_count(), 1);
    }
}
