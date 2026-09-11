//! Dispatch-script data model: dispatch block primitive, golem/pipeline
//! parsing, state context, and the filesystem layer. No block construction.

use std::path::{Path, PathBuf};

/// All golem names recognized by `Resolve-Golem` (gal.ps1 L707-710), with or without
/// the `golem-` prefix.
const KNOWN_GOLEMS: &[&str] = &[
    "golem-architect",
    "golem-analyst",
    "golem-implementer",
    "golem-tester",
    "golem-auditor",
    "golem-debugger",
    "golem-designer",
    "golem-researcher",
    "golem-releaser",
    "golem-steward",
];

/// Pipeline golems → MODE `bound` when a pipeline phase context is present (gal.ps1 L940).
const PIPELINE_GOLEMS: &[&str] = &["golem-implementer", "golem-tester", "golem-auditor"];

/// Orchestrated-only golems: bare `/gal <role>` without orchestration context → unknown-intent.
/// These remain in `KNOWN_GOLEMS` so the pipeline resolver can still dispatch them.
/// Researcher's orchestration path (research subcommands) bypasses `golem_mode` entirely.
const ORCHESTRATED_ONLY_GOLEMS: &[&str] = &[
    "golem-implementer",
    "golem-tester",
    "golem-auditor",
    "golem-researcher",
];

/// Roles that support `/gal discuss` (consult dual-mode: isolated → in-context upgrade).
/// All other roles fail the discuss gate with a role-specific recovery `ACTION`.
const DISCUSS_CAPABLE_GOLEMS: &[&str] = &[
    "golem-architect",
    "golem-analyst",
    "golem-designer",
    "golem-releaser",
];

/// The standard `ON_COMPLETE` for golem dispatch blocks (gal.ps1 L963).
pub(crate) const REPORT_TO_USER: &str = "Report result to user.";

/// Resolve a golem name with or without the `golem-` prefix, returning the canonical
/// full name or `None` if unknown. Port of `Resolve-Golem` (gal.ps1 L706-715).
pub fn resolve_golem(name: &str) -> Option<String> {
    let full = if name.starts_with("golem-") {
        name.to_string()
    } else {
        format!("golem-{name}")
    };
    if KNOWN_GOLEMS.contains(&full.as_str()) {
        Some(full)
    } else {
        None
    }
}

/// Dispatch mode for a resolved golem given the active orchestration context.
///
/// Returns:
/// - `"direct"` — directly callable roles (architect, analyst, designer, releaser, debugger,
///   steward); the caller selects the role under its own `*.agent.md` contract intersected
///   with host-granted tools. No tools granted, no sandbox policy changed.
/// - `"bound"` — orchestrated-only golem in a valid orchestration context (pipeline phase or
///   finalize branch-audit for auditor). For the auditor finalize context, the emitted output
///   is a non-dispatch signal produced by `crates/cli/src/dispatch_script/build.rs`, not a
///   pipeline-phase dispatch.
/// - `"orchestrated-only"` — orchestrated-only golem with **no** orchestration context;
///   the caller must emit an unknown-intent sentinel (gate: bare `/gal <role>` rejected).
pub fn golem_mode(
    golem: &str,
    pipeline_requested: bool,
    finalize_branch_audit: bool,
) -> &'static str {
    // `finalize_branch_audit` is a user-reachable token, so it must NOT broadly relax the
    // orchestrated-only gate. The finalize branch-audit context legitimately covers ONLY the
    // auditor. Granting it to implementer/tester/researcher would let
    // `/gal implementer --finalize-branch-audit` bypass the gate without real pipeline orchestration.
    let auditor_finalize_ctx = finalize_branch_audit && golem == "golem-auditor";
    if (pipeline_requested && PIPELINE_GOLEMS.contains(&golem)) || auditor_finalize_ctx {
        "bound"
    } else if ORCHESTRATED_ONLY_GOLEMS.contains(&golem) {
        "orchestrated-only"
    } else {
        "direct"
    }
}

/// True when this golem supports `/gal discuss` (the four consult-dual-mode roles only).
pub fn is_discuss_capable(golem: &str) -> bool {
    DISCUSS_CAPABLE_GOLEMS.contains(&golem)
}

/// Role-specific recovery `ACTION` for a rejected `/gal discuss <role>` call.
/// - Directly callable but discuss-unsupported roles (debugger, steward): bare `/gal <role>`.
/// - Orchestrated-only roles: their owning workflow command(s).
pub fn discuss_recovery_action(golem: &str) -> String {
    let short = golem.strip_prefix("golem-").unwrap_or(golem);
    match golem {
        "golem-debugger" | "golem-steward" => {
            format!("'/gal discuss {short}' is not supported. Invoke it directly: /gal {short}")
        }
        "golem-implementer" | "golem-tester" => {
            format!("'/gal discuss {short}' is not supported. {short} is orchestrated-only — use /gal pipeline to dispatch it.")
        }
        "golem-auditor" => {
            "'/gal discuss auditor' is not supported. auditor is orchestrated-only — use /gal pipeline (task audit) or /gal finalize (branch audit) to dispatch it.".to_string()
        }
        "golem-researcher" => {
            "'/gal discuss researcher' is not supported. researcher is orchestrated-only — use /gal research or /gal deep-research instead.".to_string()
        }
        _ => format!("'/gal discuss {short}' is not supported. Use a bare /gal {short} call or the appropriate workflow command."),
    }
}

/// Parse a `/gal discuss <role>` entry. Returns `(canonical_golem, is_in_context)` where
/// `is_in_context` is true when the `discuss` keyword is in the tokens at position 0, false
/// for a bare role invocation (isolated consult, the default).
///
/// Accepted:
/// - `intent="discuss"`, `tokens=["architect", ...]` → `(golem-architect, true)`
/// - `intent="discuss"`, `tokens=["golem-architect", ...]` → `(golem-architect, true)`
/// - `intent="architect"`, any tokens → `(golem-architect, false)` (bare, isolated)
///
/// Returns `None` when neither form resolves to a known golem.
pub fn parse_discuss_context(intent: &str, tokens: &[String]) -> Option<(String, bool)> {
    if intent == "discuss" {
        let role = tokens.first().map(|s| s.as_str()).unwrap_or("");
        let golem = resolve_golem(role)?;
        Some((golem, true))
    } else {
        let golem = resolve_golem(intent)?;
        Some((golem, false))
    }
}

/// A dispatch block: an ordered list of `(KEY, value)` pairs rendered between the
/// `--- GAL DISPATCH ---` / `--- END DISPATCH ---` markers. Empty values are dropped
/// (parity with `Write-Dispatch` L407 skipping `$null`/empty).
#[derive(Debug, Default, Clone)]
pub struct DispatchBlock {
    fields: Vec<(String, String)>,
}

impl DispatchBlock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a field. Empty values are ignored (shell parity).
    pub fn push(&mut self, key: &str, value: impl Into<String>) -> &mut Self {
        let value = value.into();
        if !value.is_empty() {
            self.fields.push((key.to_string(), value));
        }
        self
    }

    /// Render the block with fields sorted by key (the deterministic order — D-001).
    pub fn render(&self) -> String {
        let mut sorted = self.fields.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let mut out = String::from("--- GAL DISPATCH ---\n");
        for (k, v) in &sorted {
            out.push_str(&format!("{k}: {v}\n"));
        }
        out.push_str("--- END DISPATCH ---");
        out
    }
}

/// Workflow-state classification of the repo (port of `Get-StateContext`, gal.ps1
/// L355-395). Only `Active` carries plan-derived metadata; `Idle` is the common case
/// when `.dev/state.md` lists active plans as a bullet list rather than the
/// header-table form `Get-ActivePlanPath` requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateContext {
    pub workflow_state: Option<String>,
    pub active_plan_path: Option<String>,
}

impl StateContext {
    /// The idle context: initialized repo, no active plan resolved (gal.ps1 L369-376).
    pub fn idle() -> Self {
        Self {
            workflow_state: Some("IDLE".to_string()),
            active_plan_path: None,
        }
    }
}

/// Strip one layer of surrounding markdown backticks (port of `Unwrap-MarkdownCode`).
fn unwrap_markdown_code(value: &str) -> String {
    let t = value.trim();
    if t.len() >= 2 && t.starts_with('`') && t.ends_with('`') {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

/// Resolve the active execution-plan path from `.dev/state.md` content. Port of
/// `Get-ActivePlanPath` (gal.ps1 L117-157): only `|`-table rows AFTER a header row
/// containing `Plan` and (`Workflow State` or `Plan Phase`) are considered; a cell
/// matching `*.prompt.md`/`*.md` is returned. A bullet list (current repo form) yields
/// `None` → idle. Returns the unwrapped cell text (FS-resolution is the shell's
/// `Resolve-PlanPath` final fallback `return $absolutePath`).
pub fn active_plan_path_from_state(state_md: &str) -> Option<String> {
    let mut in_active = false;
    let mut header_seen = false;
    for line in state_md.lines() {
        if line.trim_start().starts_with("## Active Plans") {
            in_active = true;
            continue;
        }
        if in_active && line.starts_with("## ") {
            break;
        }
        if !in_active {
            continue;
        }
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        if trimmed
            .trim_start_matches('|')
            .trim_start()
            .starts_with("---")
        {
            continue;
        }
        let cells: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect();
        if cells.len() < 2 {
            continue;
        }
        if !header_seen {
            let has_plan = cells.iter().any(|c| c == "Plan");
            let has_state = cells
                .iter()
                .any(|c| c == "Workflow State" || c == "Plan Phase");
            if has_plan && has_state {
                header_seen = true;
            }
            continue;
        }
        for cell in &cells {
            let candidate = unwrap_markdown_code(cell);
            // Planning-language EN draft / retired equivalence-receipt file are never
            // an active plan, even though both end in .md. `.equiv.md` has no writer
            // (proof now lives inline in the source plan) — kept as a defensive guard.
            if candidate.ends_with(".en.md") || candidate.ends_with(".equiv.md") {
                continue;
            }
            if candidate.ends_with(".prompt.md") || candidate.ends_with(".md") {
                return Some(candidate);
            }
        }
    }
    None
}
/// Parsed pipeline-phase dispatch context (port of `Get-PipelineDispatchContext`,
/// gal.ps1 L514-653). `error` is `Some` on a malformed `--pipeline-phase`/`--task-scope`
/// or an unsupported phase; the caller emits an error block.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PipelineContext {
    pub requested: bool,
    pub phase: Option<String>,
    pub task_scope: Option<String>,
    pub from: Option<String>,
    pub stop_at: Option<String>,
    pub fix_mode: bool,
    /// The explicit plan/prompt path from a `#file:<path>` (or bare `*.md`/`*.prompt.md`)
    /// token, `#file:` prefix stripped. Threaded into the OFFLOAD ACTION so the dispatch
    /// targets the plan/prompt directly (gal pipeline materializes the spec internally).
    pub plan_path: Option<String>,
    pub remaining: Vec<String>,
    pub error: Option<String>,
}

// `verify` is intentionally NOT a valid dispatch phase: goal-backward verification
// is ORCHESTRATOR-owned and always runs in-process (never dispatched). Emitting an
// OFFLOAD whose downstream `gal pipeline --phase verify` cannot exist would be a
// guaranteed failure, so the phase is rejected here at parse time.
const VALID_PHASES: &[&str] = &["implement", "test", "audit", "scaffold"];

fn is_explicit_plan_token(token: &str) -> bool {
    let t = token.strip_prefix("#file:").unwrap_or(token).trim();
    if t.ends_with(".en.md") || t.ends_with(".equiv.md") {
        // Planning-language EN draft / retired equivalence-receipt file — never a
        // plan token, even though both end in .md. Defensive guard (see model.rs
        // header comment above for the writer-removal rationale).
        return false;
    }
    t.ends_with(".prompt.md") || t.ends_with(".md")
}

/// Parse pipeline context from the dispatch tokens.
pub fn parse_pipeline_context(tokens: &[String]) -> PipelineContext {
    let mut ctx = PipelineContext::default();
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i].as_str();
        if token.is_empty() {
            i += 1;
            continue;
        }
        match token {
            "--pipeline-phase" => {
                ctx.requested = true;
                match tokens.get(i + 1).filter(|v| !v.trim().is_empty()) {
                    Some(v) => ctx.phase = Some(v.trim().to_ascii_lowercase()),
                    None => {
                        return PipelineContext {
                            error: Some("Missing phase after --pipeline-phase. Expected one of: implement, test, audit, scaffold.".to_string()),
                            ..Default::default()
                        };
                    }
                }
                i += 2;
                continue;
            }
            "--task-scope" => {
                match tokens.get(i + 1).filter(|v| !v.trim().is_empty()) {
                    Some(v) => ctx.task_scope = Some(v.trim().to_string()),
                    None => {
                        return PipelineContext {
                            error: Some("Missing task reference after --task-scope.".to_string()),
                            ..Default::default()
                        };
                    }
                }
                i += 2;
                continue;
            }
            "--fix-mode" => {
                ctx.fix_mode = true;
            }
            "from" => match tokens.get(i + 1).filter(|v| !v.trim().is_empty()) {
                Some(v) => {
                    ctx.from = Some(v.trim().to_string());
                    i += 2;
                    continue;
                }
                None => {
                    return PipelineContext {
                        error: Some("Missing task reference after from.".to_string()),
                        ..Default::default()
                    };
                }
            },
            "stop-at" => match tokens.get(i + 1).filter(|v| !v.trim().is_empty()) {
                Some(v) => {
                    ctx.stop_at = Some(v.trim().to_string());
                    i += 2;
                    continue;
                }
                None => {
                    return PipelineContext {
                        error: Some("Missing task reference after stop-at.".to_string()),
                        ..Default::default()
                    };
                }
            },
            other => {
                if is_explicit_plan_token(other) {
                    if ctx.plan_path.is_none() {
                        ctx.plan_path = Some(
                            other
                                .strip_prefix("#file:")
                                .unwrap_or(other)
                                .trim()
                                .to_string(),
                        );
                    }
                } else {
                    ctx.remaining.push(other.to_string());
                }
            }
        }
        i += 1;
    }
    if ctx.requested {
        match ctx.phase.as_deref() {
            None => {
                return PipelineContext {
                    error: Some(
                        "Pipeline-bound dispatch requires --pipeline-phase <implement|test|audit|scaffold>."
                            .to_string(),
                    ),
                    ..Default::default()
                };
            }
            Some(p) if !VALID_PHASES.contains(&p) => {
                return PipelineContext {
                    error: Some(format!("Unsupported pipeline phase '{p}'. Expected one of: implement, test, audit, scaffold. (verify is orchestrator-owned and always in-process — never a dispatch phase.)")),
                    ..Default::default()
                };
            }
            _ => {}
        }
    }
    ctx
}
/// Walk up from `start` to the nearest ancestor containing `.dev/state.md`; fall back
/// to `start` (port of `Get-RepoContextRoot`, gal.ps1 L46-62).
pub fn get_repo_context_root(start: &Path) -> PathBuf {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".dev").join("state.md").exists() {
            return current;
        }
        match current.parent() {
            Some(parent) if parent != current => current = parent.to_path_buf(),
            _ => return start.to_path_buf(),
        }
    }
}

/// Forward-slash display form of a path (the platform-consistent block form, D-001).
pub fn forward_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Read the repo `StateContext` from `<repo>/.dev/state.md` (port of `Get-StateContext`
/// for the initialized cases; uninitialized → idle-with-no-state).
pub fn read_state_context(repo_root: &Path) -> StateContext {
    let state_path = repo_root.join(".dev").join("state.md");
    match std::fs::read_to_string(&state_path) {
        Ok(content) => match active_plan_path_from_state(&content) {
            Some(active) => StateContext {
                workflow_state: None,
                active_plan_path: Some(active),
            },
            None => StateContext::idle(),
        },
        Err(_) => StateContext {
            workflow_state: None,
            active_plan_path: None,
        },
    }
}
