//! Dispatch-family commands: dispatch / pipeline / dispatch-script.

use dispatch::dispatch::ContractProvenance;
use gal_engine::ExitCode;
use sha2::{Digest, Sha256};
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

/// `gal pipeline --phase` accepts only the four implementation-lane phases
/// (`scaffold`, `implement`, `test`, `audit`). `investigate` is deliberately excluded —
/// research work has its own dedicated `gal research` command, not a pipeline phase —
/// so it (and any other unrecognized value) is rejected here rather than falling
/// through to a dispatch.
fn validate_pipeline_phase(phase: &str) -> Result<(), String> {
    match phase.to_lowercase().as_str() {
        "scaffold" | "implement" | "test" | "audit" => Ok(()),
        "investigate" => Err(
            "phase 'investigate' is not a `gal pipeline` phase; use `gal research` instead"
                .to_string(),
        ),
        other => Err(format!(
            "unknown phase '{other}'; expected one of scaffold, implement, test, audit \
             (for research, use `gal research`)"
        )),
    }
}

pub(crate) fn cmd_dispatch(args: &[String]) -> ExitCode {
    // In-process via the dispatch library — self-contained, no separate
    // `gal-dispatch` executable (which is not in the end-user release artifact).
    // The task spec is read from stdin, matching the gal-dispatch contract.
    let flags: Vec<String> = args.iter().skip(1).cloned().collect();
    if flags.iter().any(|a| a == "--help" || a == "-h") {
        println!("gal dispatch --phase <scaffold|implement|test|audit|investigate> --task <T-NN> [--workdir <p>] [--timeout <s>] [--routing <p>] [--receipt <p>]");
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

    let receipt = resolve_receipt_path(
        Some(&research_path),
        format!("investigate-{worker}.receipt.md"),
    )
    .to_string_lossy()
    .replace('\\', "/");

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
        log_dir_override: resolve_log_dir_override(&workdir, Some(&research_path)),
        worker: Some(worker),
        ..Default::default()
    };
    if let Some(dir) = &dispatch_args.log_dir_override {
        let _ = std::fs::create_dir_all(dir);
    }

    let outcome = dispatch::run::run_dispatch(&dispatch_args, &spec);

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
    if args.len() < 2 {
        eprintln!("gal pipeline: missing plan / prompt / task-spec path");
        eprintln!("usage: gal pipeline <PromptPath|SourcePlanPath|TaskSpecPath> [--phase <phase>] [--task <T-NN>] [--receipt <path>] [--fix]");
        return ExitCode::Usage;
    }

    let raw_input = PathBuf::from(&args[1]);

    let mut phase = String::from("implement");
    let mut task_override: Option<String> = None;
    let mut receipt_path: Option<String> = None;
    let mut workdir_flag: Option<String> = None;
    let mut fix_mode = false;

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
            other => {
                eprintln!("gal pipeline: unknown argument '{other}'");
                return ExitCode::Usage;
            }
        }
        index += 1;
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
    let input_kind = resolve_pipeline_input(&input_path);
    if let Err(message) = validate_fix_mode(fix_mode, &phase, input_kind) {
        eprintln!("gal pipeline: {message}");
        return ExitCode::Usage;
    }

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
    ) = match input_kind {
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
            let receipt = effective_receipt_path(
                &phase,
                &task_id,
                receipt_path.as_deref(),
                scope_source.as_deref(),
            );
            (spec, task_id, receipt, None, None, Vec::new(), None)
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

    if fix_mode {
        let Some(plan_source) = plan_log_source.as_deref() else {
            eprintln!("gal pipeline: --fix requires a plan-scoped prompt or source plan");
            return ExitCode::Usage;
        };
        let Some(slug) = plan_slug_from_path(plan_source) else {
            eprintln!(
                "gal pipeline: cannot derive replay storage for '{}'",
                plan_source.display()
            );
            return ExitCode::Error;
        };
        let Some(authority) = fix_authority.as_ref() else {
            eprintln!("gal pipeline: fix-mode stable authority inputs were not assembled");
            return ExitCode::Error;
        };
        let fingerprint = stable_fix_authority_fingerprint(authority);
        let replay_path = workdir
            .join(".dev")
            .join("pipeline")
            .join("replay")
            .join(sanitize_plan_slug(&slug))
            .join(format!("{task_id}-{phase}.sha256"));
        match persist_fix_fingerprint(&replay_path, &fingerprint) {
            Ok(true) => {
                eprintln!("gal pipeline: fix-mode replay refused for task {task_id} phase {phase}: stable authority fingerprint is unchanged");
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
    args.log_dir_override = resolve_log_dir_override(&workdir, receipt_log_scope_source.as_deref());
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

    let before_affected = fix_mode.then(|| fingerprint_affected_files(&workdir, &affected_paths));
    let outcome = dispatch::run::run_dispatch(&args, &spec);
    if before_affected.is_some()
        && outcome
            .terminal_state
            .as_ref()
            .is_some_and(|state| state.as_str() == "completed")
        && before_affected == Some(fingerprint_affected_files(&workdir, &affected_paths))
    {
        let now = gal_foundation::time::now_timestamp();
        let run = now.get(0..10).unwrap_or("run").to_string();
        let event = pipeline::loop_log::LoopEvent {
            time: now,
            task: task_id.clone(),
            phase: args.phase.as_str().to_string(),
            role: args.phase.role().to_string(),
            kind: "retry".to_string(),
            level: pipeline::loop_log::LoopLevel::Error,
            msg: "fix-round-no-change: affected implementation files were unchanged".to_string(),
            log_ptr: outcome
                .log_path
                .as_ref()
                .map(|path| path.display().to_string()),
        };
        let dir = workdir.join(".dev").join("pipeline").join("loop-log");
        let _ = pipeline::loop_log::append_event(&dir, &run, &event);
        eprintln!("gal pipeline: fix-round-no-change for task {task_id}: affected implementation files were unchanged");
        return ExitCode::Error;
    }

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

    // A third party changing the execution prompt during a dispatch is invisible
    // to the worktree containment check, which only fires on sibling worktrees.
    // Detect it here and keep the pre-dispatch bytes, so a silent loss becomes a
    // named, recoverable event. Deliberately outside the success guard below: a
    // failed dispatch can leave the file mutated too.
    if let (Some(snap), Some(prompt_path)) = (&prompt_snapshot, plan_log_source.as_deref()) {
        if let Ok(after_bytes) = std::fs::read(prompt_path) {
            if let Some(msg) = prompt_mutation_msg(snap, &after_bytes) {
                let now = gal_foundation::time::now_timestamp();
                let run = now.get(0..10).unwrap_or("run").to_string();
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
                let dir = workdir.join(".dev").join("pipeline").join("loop-log");
                let _ = pipeline::loop_log::append_event(&dir, &run, &event);
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
            let prompt_bytes = match std::fs::read(prompt_path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    let msg = format!(
                        "failed to read execution prompt '{}': {e}",
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
            let prompt_str = std::str::from_utf8(&prompt_bytes).unwrap_or("");
            let is_test_first = pipeline::task_spec::has_test_first_marker(prompt_str);
            if pipeline::task_spec::is_writeback_in_scope(args.phase.as_str(), is_test_first) {
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
                let receipt_bytes = match std::fs::read(&receipt_file) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        let msg = format!(
                            "failed to read receipt file '{}': {e}",
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
                    }
                };
                let rendered = match pipeline::task_spec::render_phase_writeback(
                    &prompt_bytes,
                    &receipt_bytes,
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

                if is_test_first {
                    let old_digest = format!("{:x}", Sha256::digest(&prompt_bytes));
                    if let Err(e) = crate::commands::test_first_transition::transition_prompt(
                        &workdir,
                        prompt_path,
                        &rendered,
                        Some(&old_digest),
                        "phase-rerun",
                    ) {
                        let msg = format!("transition_prompt failed: {e}");
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
                } else if let Err(e) = std::fs::write(prompt_path, &rendered) {
                    let msg = format!(
                        "failed to write prompt file '{}': {e}",
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

    match outcome.exit_code {
        0 => ExitCode::Success,
        1 => ExitCode::Error,
        2 => ExitCode::NotWired,
        _ => ExitCode::Error,
    }
}

/// The execution prompt as it stood immediately before `run_dispatch`: its
/// digest, plus the backup copy holding those exact bytes. A snapshot failure
/// must never block the dispatch it observes, so every fallible step degrades
/// to `None` rather than propagating.
struct PromptSnapshot {
    digest: String,
    backup: PathBuf,
}

/// Capture the prompt's pre-dispatch bytes under the gitignored
/// `.dev/pipeline/prewrite/<slug>/` so a change made during the dispatch stays
/// recoverable. `None` when the prompt cannot be read or the copy cannot be
/// written — detection is best-effort and never fails the run.
fn snapshot_prompt(
    workdir: &Path,
    prompt_path: &Path,
    task_id: &str,
    phase: &str,
) -> Option<PromptSnapshot> {
    let bytes = std::fs::read(prompt_path).ok()?;
    let slug = pipeline::task_spec::extract_plan_slug(&prompt_path.display().to_string());
    let dir = workdir
        .join(".dev")
        .join("pipeline")
        .join("prewrite")
        .join(slug);
    std::fs::create_dir_all(&dir).ok()?;
    let backup = dir.join(format!("{task_id}-{phase}.prompt.bak"));
    std::fs::write(&backup, &bytes).ok()?;
    Some(PromptSnapshot {
        digest: format!("{:x}", Sha256::digest(&bytes)),
        backup,
    })
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

fn log_semantic_failure(
    workdir: &Path,
    task_id: &str,
    phase: &str,
    role: &str,
    err_msg: &str,
    log_ptr: Option<&PathBuf>,
) {
    let now = gal_foundation::time::now_timestamp();
    let run = now.get(0..10).unwrap_or("run").to_string();
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
    let dir = workdir.join(".dev").join("pipeline").join("loop-log");
    let _ = pipeline::loop_log::append_event(&dir, &run, &event);
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

/// Resolve a receipt path beneath the same plan scope used for executor logs.
/// Plan and prompt inputs share a sanitized scope key; raw/direct inputs retain the
/// legacy unscoped receipt location because no plan identity is available.
pub(crate) fn resolve_receipt_path(
    plan_source: Option<&Path>,
    receipt_name: impl AsRef<Path>,
) -> PathBuf {
    let receipt_root = PathBuf::from(".dev/pipeline/receipts");
    match plan_source.and_then(plan_slug_from_path) {
        Some(slug) => receipt_root
            .join(sanitize_plan_slug(&slug))
            .join(receipt_name),
        None => receipt_root.join(receipt_name),
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
    let receipt = effective_receipt_path(phase, &task_id, explicit_receipt, Some(prompt_path));
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
) -> Option<String> {
    if let Some(e) = explicit {
        return Some(e.to_string());
    }
    let phase_lc = phase.to_lowercase();
    match phase_lc.as_str() {
        "test" | "audit" | "implement" => Some(
            resolve_receipt_path(plan_source, format!("{task_id}-{phase_lc}.receipt.md"))
                .to_string_lossy()
                .replace('\\', "/"),
        ),
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

        let receipt = effective_receipt_path("test", &tid(3), None, None);
        assert_eq!(
            receipt,
            Some(format!(".dev/pipeline/receipts/{}-test.receipt.md", tid(3)))
        );
        assert_eq!(
            resolve_receipt_path(None, format!("{}-test.receipt.md", tid(3))),
            PathBuf::from(format!(".dev/pipeline/receipts/{}-test.receipt.md", tid(3)))
        );
    }

    #[test]
    fn effective_receipt_path_defaults() {
        let t = tid(8);
        // test/audit/implement get a deterministic default receipt.
        assert_eq!(
            effective_receipt_path("test", &t, None, None),
            Some(format!(".dev/pipeline/receipts/{t}-test.receipt.md"))
        );
        assert_eq!(
            effective_receipt_path("audit", &t, None, None),
            Some(format!(".dev/pipeline/receipts/{t}-audit.receipt.md"))
        );
        assert_eq!(
            effective_receipt_path("implement", &t, None, None),
            Some(format!(".dev/pipeline/receipts/{t}-implement.receipt.md"))
        );
        // Mixed-case phase resolves to lowercased filename.
        assert_eq!(
            effective_receipt_path("Implement", &t, None, None),
            Some(format!(".dev/pipeline/receipts/{t}-implement.receipt.md"))
        );
        // Other phases default to no receipt.
        assert_eq!(effective_receipt_path("unknown", &t, None, None), None);
        // explicit --receipt always wins.
        assert_eq!(
            effective_receipt_path("test", &t, Some("custom/r.md"), None),
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
        let expected = format!(".dev/pipeline/receipts/x/{}-test.receipt.md", tid(8));
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
    fn resolve_receipt_path_scopes_plan_inputs_and_preserves_raw_fallback() {
        let receipt = "task-test.receipt.md";
        assert_eq!(
            resolve_receipt_path(Some(Path::new(".dev/plans/fix-one.prompt.md")), receipt),
            PathBuf::from(".dev/pipeline/receipts/fix-one").join(receipt)
        );
        assert_eq!(
            resolve_receipt_path(Some(Path::new(".dev/plans/fix-two.md")), receipt),
            PathBuf::from(".dev/pipeline/receipts/fix-two").join(receipt)
        );
        assert_eq!(
            resolve_receipt_path(Some(Path::new(".dev/plans/fix-one.md")), receipt),
            resolve_receipt_path(Some(Path::new(".dev/plans/fix-one.prompt.md")), receipt)
        );
        assert_eq!(
            resolve_receipt_path(Some(Path::new(".dev/plans/....md")), receipt),
            PathBuf::from(".dev/pipeline/receipts/unknown-plan").join(receipt)
        );
        assert_eq!(
            resolve_receipt_path(None, receipt),
            PathBuf::from(".dev/pipeline/receipts").join(receipt)
        );
    }

    #[test]
    fn research_scope_resolves_receipt_and_log_directories() {
        // A `.dev/research/<slug>.md` dispatch source must scope its executor
        // log directory under `.dev/executor-logs/<slug>/` — the SAME mechanism
        // (`plan_slug_from_path` + `resolve_log_dir_override`) already used for
        // `.dev/plans/` Prompt/SourcePlan inputs — instead of the unscoped default.
        //
        // `resolve_pipeline_input` classifies `.dev/research/<slug>.md` as `RawSpec`
        // (see `resolve_pipeline_input_classifies_by_location_and_suffix`), and
        // `cmd_pipeline`'s `RawSpec` arm currently hardcodes `plan_log_source = None`
        // unconditionally for every `RawSpec` input, so today a research-sourced
        // dispatch never reaches `resolve_log_dir_override` with its own path and
        // keeps the unscoped `.dev/executor-logs/` location. This probe drives the
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
            .join("executor-logs")
            .join("find-flaky-tests");
        assert!(
            scoped_log_dir.is_dir(),
            "research scope resolves receipt and log directories: expected scoped \
             executor-log dir '{}' to exist after dispatching a `.dev/research/` \
             source, found only: {:?}",
            scoped_log_dir.display(),
            std::fs::read_dir(workdir.path().join(".dev").join("executor-logs"))
                .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
                .unwrap_or_else(|_| Vec::<PathBuf>::new())
        );
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
        let path = tmp
            .path()
            .join("replay")
            .join(format!("{}-implement.sha256", tid(3)));
        assert!(!persist_fix_fingerprint(&path, "authority-a").unwrap());
        assert!(path.is_file());
        assert!(persist_fix_fingerprint(&path, "authority-a").unwrap());
        assert!(!persist_fix_fingerprint(&path, "authority-b").unwrap());
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
    fn resolve_contract_scaffold_phase_selects_implementer_contract_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("root");
        write_source_root(&root, "scaffold", "IMPLEMENTER SCAFFOLD BODY");
        let got =
            resolve_contract_from_candidates(&[cand("workdir", Some(&root))], "scaffold").unwrap();
        assert!(got.path.ends_with("golem-implementer.agent.md"));
        assert_eq!(got.body, "IMPLEMENTER SCAFFOLD BODY");
    }

    #[test]
    fn build_spec_from_prompt_scaffold_inlines_implementer_contract() {
        let tmp = tempfile::TempDir::new().unwrap();
        let prompt = tmp.path().join("x.prompt.md");
        std::fs::write(
            &prompt,
            format!("## Tasks\n\n- [ ] {t} — Touch `crates/a.rs`.\n", t = tid(1)),
        )
        .unwrap();
        let body = "# Golem Implementer\n\nImplementer contract inline for scaffold phase.\n";
        let (spec, task_id, receipt, _affected, _authority) = build_spec_from_prompt(
            &prompt,
            tmp.path(),
            "scaffold",
            Some(&tid(1)),
            None,
            Some(body),
            false,
        )
        .unwrap();
        assert_eq!(task_id, tid(1));
        assert!(receipt.is_none());
        assert!(
            spec.contains(&format!("# Task Spec: {} (scaffold)", tid(1))),
            "spec title must include scaffold phase:\n{spec}"
        );
        assert!(
            spec.contains("Implementer contract inline for scaffold phase."),
            "implementer contract body must be inlined into scaffold spec:\n{spec}"
        );
        assert!(
            !spec.contains("Follow the instructions in:"),
            "no control-node contract path reference should remain in spec:\n{spec}"
        );
        assert!(verify_agent_contract(&spec, tmp.path()).is_ok());
    }

    #[test]
    fn scaffold_dispatch_spec_and_receipt_plan_scoping() {
        let tmp = tempfile::TempDir::new().unwrap();
        let prompt = tmp.path().join("feature-a.prompt.md");
        std::fs::write(
            &prompt,
            format!("## Tasks\n\n- [ ] {t} — Touch `crates/a.rs`.\n", t = tid(2)),
        )
        .unwrap();
        let body = "# Golem Implementer\n\nContract body.\n";
        let explicit_receipt = format!(
            ".dev/pipeline/receipts/feature-a/{}-scaffold.receipt.md",
            tid(2)
        );
        let (spec, task_id, receipt, _affected, _authority) = build_spec_from_prompt(
            &prompt,
            tmp.path(),
            "scaffold",
            Some(&tid(2)),
            Some(&explicit_receipt),
            Some(body),
            false,
        )
        .unwrap();
        assert_eq!(task_id, tid(2));
        assert_eq!(receipt.as_deref(), Some(explicit_receipt.as_str()));
        assert!(
            spec.contains(&format!("# Task Spec: {} (scaffold)", tid(2))),
            "spec title must include scaffold phase:\n{spec}"
        );
        let resolved_log = resolve_log_dir_override(tmp.path(), Some(&prompt));
        assert_eq!(
            resolved_log,
            Some(tmp.path().join(".dev/executor-logs/feature-a"))
        );
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
            "scaffold",
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
                phase: "scaffold".to_string(),
                executor: executor.to_string(),
                executor_args: vec![],
                spec: spec_base.clone(),
                timeout_secs: 10,
                actual_model: "test-model".to_string(),
                log_dir: tmp.path().join(".dev/executor-logs"),
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

        // 4. Marked prompt audit via transition_prompt & boundary_check
        let marked_prompt_path = tmp.path().join("marked.prompt.md");
        let marked_prompt = format!(
            "# Plan\n\nPipeline Contract: test-first-v1\n\n## Goal\nGoal.\n\n## Requirements\nReqs.\n\n## Tasks\n- [ ] {} — Goal (`crates/pipeline/src/task_spec.rs`, `crates/cli/src/commands/dispatch.rs`)\n\n## Test Plan\nPlan.\n\n## Test Results\n\n## Review Results\n",
            tid(1)
        );

        crate::commands::test_first_transition::transition_prompt(
            tmp.path(),
            &marked_prompt_path,
            marked_prompt.as_bytes(),
            None,
            "init",
        )
        .unwrap();

        let prompt_bytes = std::fs::read(&marked_prompt_path).unwrap();
        let old_digest = format!("{:x}", Sha256::digest(&prompt_bytes));
        let rendered = pipeline::task_spec::render_phase_writeback(
            &prompt_bytes,
            audit_receipt.as_bytes(),
            "audit",
            &tid(1),
        )
        .unwrap();

        let rec = crate::commands::test_first_transition::transition_prompt(
            tmp.path(),
            &marked_prompt_path,
            &rendered,
            Some(&old_digest),
            "phase-rerun",
        )
        .unwrap();
        assert_eq!(rec.kind, "phase-rerun");

        let eval = crate::commands::boundary_check::evaluate_marked_digest(
            &std::fs::read_to_string(&marked_prompt_path).unwrap(),
            tmp.path(),
            &marked_prompt_path,
        );
        assert!(eval.is_pass(), "marked digest binding must pass: {eval:?}");

        // 5. Marked test bypasses write-back gate predicate
        let is_test_first = pipeline::task_spec::has_test_first_marker(
            &std::fs::read_to_string(&marked_prompt_path).unwrap(),
        );
        assert!(!pipeline::task_spec::is_writeback_in_scope(
            "test",
            is_test_first
        ));
        assert!(pipeline::task_spec::is_writeback_in_scope(
            "audit",
            is_test_first
        ));

        // 6. Implement and scaffold bypass write-back gate predicate
        assert!(!pipeline::task_spec::is_writeback_in_scope(
            "implement",
            is_test_first
        ));
        assert!(!pipeline::task_spec::is_writeback_in_scope(
            "scaffold",
            is_test_first
        ));

        // 7. Malformed payload failure leaves prompt unchanged
        let malformed_receipt = format!("### [{}] 2026-08-26\n\nverdict: PASS\n", tid(2));
        let prompt_bytes_before = std::fs::read(&marked_prompt_path).unwrap();
        assert!(pipeline::task_spec::render_phase_writeback(
            &prompt_bytes_before,
            malformed_receipt.as_bytes(),
            "audit",
            &tid(1)
        )
        .is_err());
        let prompt_bytes_after = std::fs::read(&marked_prompt_path).unwrap();
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

        // 10. Stale old digest rejection in transition_prompt leaves prompt unchanged
        let prompt_before_stale = std::fs::read_to_string(&marked_prompt_path).unwrap();
        let stale_res = crate::commands::test_first_transition::transition_prompt(
            tmp.path(),
            &marked_prompt_path,
            &rendered,
            Some("staledigest12345"),
            "phase-rerun",
        );
        assert!(
            stale_res.is_err(),
            "stale digest must fail transition_prompt"
        );
        let prompt_after_stale = std::fs::read_to_string(&marked_prompt_path).unwrap();
        assert_eq!(prompt_before_stale, prompt_after_stale);

        // 11. log_semantic_failure writes structured loop log entry
        let test_log_dir = tmp.path().join(".dev/pipeline/loop-log");
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
        let log_content = std::fs::read_to_string(entries[0].as_ref().unwrap().path()).unwrap();
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
            .join(".dev/pipeline/receipts/t10-probe-slug/investigate-2.receipt.md");
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
