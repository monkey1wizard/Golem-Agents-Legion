//! Repo-adapter render — gal's own copy (gal tree / workflow).
//!
//! Generates the 5 repo-local adapter documents (`.github/copilot-instructions.md`,
//! `GEMINI.md`, `CLAUDE.md`, `AGENTS.md`, `.agents/rules/gal.md`) from `.dev/project.md`
//! + selected conventions + `workflows/coding.md`.
//!
//! This is the **repo-adapter render** (workflow), distinct from the
//! **canonical-root render** (machine bake). It was de-hybridized out of
//! `projection` in S2 so the gal product owns its own `gal sync` render with
//! self-owned report/error types and no `use projection::`. The shared markdown
//! read helpers are copied per side (decoupling > DRY); each side owns its own.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::gal::skill_discovery::{self, DiscoveredSkill};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectionWarning {
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectionReport {
    pub written_files: Vec<PathBuf>,
    pub removed_paths: Vec<PathBuf>,
    pub created_links: Vec<PathBuf>,
    pub warnings: Vec<ProjectionWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOptions {
    pub repo_root: PathBuf,
    pub source_root: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("{0}")]
    Message(String),
    #[error("{0}")]
    Io(#[from] io::Error),
}

pub fn sync_options_from(repo_root: PathBuf, dry_run: bool) -> Result<SyncOptions, AdapterError> {
    let source_root = resolve_source_root()?;
    Ok(SyncOptions {
        repo_root,
        source_root,
        dry_run,
    })
}

/// Resolve the GAL source root for repo-adapter render.
///
/// Resolution chain: (a) cwd-walk bounded at `.git` repo edge, (b) exe-side
/// packaged/FHS layout, (c) clear error.
fn resolve_source_root() -> Result<PathBuf, AdapterError> {
    let cwd = std::env::current_dir().map_err(|e| {
        AdapterError::Message(format!("could not determine current directory: {e}"))
    })?;
    let exe = std::env::current_exe()
        .map_err(|e| AdapterError::Message(format!("could not resolve current executable: {e}")))?;
    // Canonicalize so a symlinked entry point (winget's WinGet\Links\gal.exe)
    // resolves to the real install dir before the exe-side walk.
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    let exe_dir = exe.parent().unwrap_or(exe.as_path());
    // (a) cwd-walk / (b) exe-side, then (c) the build-time embedded payload
    // (Cargo bare-binary channel — materialized to ~/.gal/embedded-src, shared
    // with `gal refresh` / template resolution).
    resolve_source_root_in(&cwd, exe_dir)
        .or_else(gal_engine::render::materialize_embedded_source)
        .ok_or_else(|| {
            AdapterError::Message(
                "could not resolve GAL source root: run from inside a GAL repo checkout, \
                 or reinstall via a packaging channel (Cargo/Homebrew/WinGet)"
                    .into(),
            )
        })
}

/// Pure helper: resolve the GAL source root given a cwd and exe directory.
///
/// Walk cwd ancestors up to and including the first `.git`-containing ancestor
/// (repo boundary), then fall through to exe-side ancestors (packaged/FHS).
/// The `.git` boundary prevents matching a parent repository when GAL is a sub-repo.
pub fn resolve_source_root_in(cwd: &Path, exe_dir: &Path) -> Option<PathBuf> {
    // cwd-walk: bounded at repo .git edge
    for anc in cwd.ancestors() {
        if looks_like_gal_core(anc) {
            return Some(anc.to_path_buf());
        }
        let plugin = anc.join("plugins").join("gal-core");
        if looks_like_gal_core(&plugin) {
            return Some(plugin);
        }
        // Stop after checking the `.git`-containing ancestor (repo root)
        if anc.join(".git").exists() {
            break;
        }
    }
    // exe-side fallback: packaged binary layout or FHS (e.g. share/gal/plugins/gal-core)
    for anc in exe_dir.ancestors() {
        if looks_like_gal_core(anc) {
            return Some(anc.to_path_buf());
        }
        let plugin = anc.join("plugins").join("gal-core");
        if looks_like_gal_core(&plugin) {
            return Some(plugin);
        }
    }
    None
}

fn looks_like_gal_core(path: &Path) -> bool {
    ["commands", "agents", "skills", "conventions", "workflows"]
        .iter()
        .all(|name| path.join(name).is_dir())
}

/// Hard cap on `.dev/project.md`'s normalized-LF UTF-8 byte size.
///
/// Single source of truth: `plugins/gal-core/conventions/token-budget.md`
/// § Bounded Current-Topic Index (`.dev/project.md`). Keep this constant in
/// sync with that document if the budget ever changes.
const PROJECT_MD_MAX_BYTES: usize = 30_720;

/// Exact ownership marker embedded in every GAL-generated conditional layer.
///
/// Collision preflight uses this to distinguish GAL-owned files (safe to
/// update or remove) from user-owned files (hard collision — no mutation).
pub const GAL_LAYER_OWNERSHIP_MARKER: &str = "<!-- GAL-generated: gal init -->";

/// Fixed repo-relative paths of the five repo-local root adapters.
///
/// Stable ascending-sorted order; no duplicates. This is the single path
/// authority shared by rendering, `gal init` reporting, and finalize
/// idempotency. Do not duplicate this list elsewhere.
pub const REPO_ADAPTER_ROOTS: &[&str] = &[
    ".agents/rules/gal.md",
    ".github/copilot-instructions.md",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
];

/// Fixed repo-relative paths of the two verified conditional-layer adapters.
///
/// Present only when the Rust convention is selected. Same authority contract
/// as `REPO_ADAPTER_ROOTS`.
pub const REPO_ADAPTER_CONDITIONAL_LAYERS: &[&str] = &[
    ".claude/rules/gal-rust.md",
    ".github/instructions/gal-rust.instructions.md",
];

/// The `.dev/project.md` H2 sections every repo-adapter root must render, in
/// canonical order.
///
/// Order here is the order they appear in a rendered root, independent of the
/// order they happen to appear in the source document.
pub const REQUIRED_PROJECT_SECTIONS: &[&str] = &[
    "What This Is",
    "Tech Stack",
    "Architecture",
    "Constraints",
    "Response Style",
    "Freshness",
    "Project Language",
    "Protected Paths",
];

/// A required `.dev/project.md` section: its canonical heading and its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSection {
    pub heading: &'static str,
    pub body: String,
}

/// Split markdown into `(h2_heading, body)` pairs.
///
/// Fenced code blocks are tracked so a `## ` line inside a fence is body text,
/// not a heading — `.dev/project.md` embeds a fenced block under `Tech Stack`,
/// and mistaking its contents for structure would silently truncate a section.
fn split_h2_sections(content: &str) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    let mut in_fence = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            if let Some(heading) = trimmed.strip_prefix("## ") {
                if let Some((name, body)) = current.take() {
                    sections.push((name, body.join("\n")));
                }
                current = Some((heading.trim().to_string(), Vec::new()));
                continue;
            }
        }
        if let Some((_, body)) = current.as_mut() {
            body.push(line);
        }
    }
    if let Some((name, body)) = current.take() {
        sections.push((name, body.join("\n")));
    }
    sections
}

/// Extract every required `.dev/project.md` section in canonical order.
///
/// Fail-closed: a heading that is missing, or that appears more than once, is
/// an error naming the source path and the offending heading. Both cases mean
/// the renderer cannot tell which content belongs in a root, so it must refuse
/// rather than guess — a silently dropped section would strip required project
/// context out of every adapter. Standard library only.
fn extract_required_project_sections(
    project_path: &Path,
    content: &str,
) -> Result<Vec<ProjectSection>, AdapterError> {
    let sections = split_h2_sections(content);
    let display = project_path.display();

    let mut extracted = Vec::with_capacity(REQUIRED_PROJECT_SECTIONS.len());
    for required in REQUIRED_PROJECT_SECTIONS {
        let mut matches = sections.iter().filter(|(name, _)| name == required);
        let Some((_, body)) = matches.next() else {
            return Err(AdapterError::Message(format!(
                "{display} is missing the required H2 section `## {required}`. Every repo-adapter root renders this section, so it must be present exactly once."
            )));
        };
        if matches.next().is_some() {
            let count = sections.iter().filter(|(name, _)| name == required).count();
            return Err(AdapterError::Message(format!(
                "{display} declares the required H2 section `## {required}` {count} times. It must appear exactly once — the renderer cannot choose between duplicates."
            )));
        }
        extracted.push(ProjectSection {
            heading: required,
            body: body.trim().to_string(),
        });
    }
    Ok(extracted)
}

/// The five repo-adapter roots' rendering identity — preamble wording, native
/// Rust-layer status, and skill-index inclusion all key off this rather than
/// off the raw path string. One-to-one with `REPO_ADAPTER_ROOTS` in declared
/// order; `slim_runtime_for_root` is the single place that pairs them up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlimRuntime {
    Antigravity,
    Copilot,
    Codex,
    Claude,
    Gemini,
}

impl SlimRuntime {
    fn title(self) -> &'static str {
        match self {
            SlimRuntime::Antigravity => "GAL Workspace Rules",
            SlimRuntime::Copilot => "Copilot Instructions",
            SlimRuntime::Codex => "GAL Agent Instructions",
            SlimRuntime::Claude => "CLAUDE Context",
            SlimRuntime::Gemini => "GEMINI Context",
        }
    }

    fn generated_marker(self) -> &'static str {
        match self {
            SlimRuntime::Antigravity => {
                "> Generated by `/gal init` for tools reading `.agents/rules/gal.md`. Do not edit manually."
            }
            SlimRuntime::Copilot => "> Generated by `/gal init` for GitHub Copilot. Do not edit manually.",
            SlimRuntime::Codex => {
                "> Generated by `/gal init` for Codex CLI, Copilot CLI, Antigravity workspace rules, remaining Google CLI bridges, and opencode. Do not edit manually."
            }
            SlimRuntime::Claude => "> Generated by `/gal init` for Claude Code. Do not edit manually.",
            SlimRuntime::Gemini => {
                "> Generated by `/gal init` for Antigravity CLI and remaining Google CLI surfaces. Do not edit manually."
            }
        }
    }

    /// `true` for the two runtimes with a verified native conditional-loading
    /// mechanism (Claude `paths`, Copilot `applyTo`; rendered separately by
    /// the conditional-layer renderer). Those roots rely on the native layer
    /// instead of an explicit root-level Rust
    /// trigger line.
    fn has_native_rust_layer(self) -> bool {
        matches!(self, SlimRuntime::Claude | SlimRuntime::Copilot)
    }

    /// `true` for roots that discover repo-local skills by name (Codex,
    /// Claude, Gemini). Copilot and the Antigravity workspace-rules root omit
    /// the index — Copilot discovers GAL commands from the global runtime
    /// install, and the rules file stays a compact routing surface.
    fn include_skill_index(self) -> bool {
        matches!(
            self,
            SlimRuntime::Codex | SlimRuntime::Claude | SlimRuntime::Gemini
        )
    }
}

/// Pair a `REPO_ADAPTER_ROOTS` path with its rendering identity. `None` for
/// any path outside the fixed inventory — keeps the two lists from silently
/// drifting apart.
pub(crate) fn slim_runtime_for_root(root_path: &str) -> Option<SlimRuntime> {
    match root_path {
        ".agents/rules/gal.md" => Some(SlimRuntime::Antigravity),
        ".github/copilot-instructions.md" => Some(SlimRuntime::Copilot),
        "AGENTS.md" => Some(SlimRuntime::Codex),
        "CLAUDE.md" => Some(SlimRuntime::Claude),
        "GEMINI.md" => Some(SlimRuntime::Gemini),
        _ => None,
    }
}

/// Render one slim repo-adapter root: runtime-appropriate generated marker,
/// Named Workflow Obedience preamble, critical receipt/preflight stop rules,
/// working-hours stop rule, compact cold-start order, the required project
/// sections, the repo-skill index where applicable, canonical source
/// pointers, and — for runtimes without a verified native conditional-layer
/// mechanism — an explicit Rust trigger when the Rust convention is selected.
///
/// Deliberately does not accept full convention or workflow document bodies:
/// large source content is represented by a canonical file pointer, never
/// concatenated in. That is what makes "no full body embed" true by
/// construction rather than by a fragile string check.
///
/// `personal_convention_labels` and `detected_skills_block` carry the two
/// machine-local discovery sources as routing only: personal conventions
/// become pointer lines to `~/.gal/local/conventions/`, and the detected
/// language skills block is already reference-only (name + origin + a
/// load-first instruction, never body content).
pub(crate) fn render_slim_root_document(
    runtime: SlimRuntime,
    sections: &[ProjectSection],
    skill_names: &[String],
    rust_selected: bool,
    personal_convention_labels: &[String],
    detected_skills_block: &str,
) -> String {
    let mut lines = Vec::new();
    lines.push(runtime.generated_marker().to_string());
    lines.push(String::new());
    lines.push(format!("# {}", runtime.title()));
    lines.push(String::new());

    lines.push("## Named Workflow Obedience".to_string());
    lines.push(String::new());
    lines.push(
        "**Binding.** When the user invokes any GAL named workflow — `$gal-pipeline`, \
         `$gal-finalize`, `$gal-status`, `$deep-planning`, `$refining-plan`, `$plan-to-prompt` \
         — or expresses repo-work intent (implement, plan, finalize, review, 實作, 規劃, 跑 \
         pipeline, finalize), your first action MUST be to load and execute the corresponding \
         `SKILL.md` before taking any action. Generic autonomous coding, batch edits, or a \
         summary response that bypasses the named workflow is a `named workflow obedience \
         failure`."
            .to_string(),
    );
    lines.push(String::new());

    lines.push("## Critical Stop Rules".to_string());
    lines.push(String::new());
    lines.push(
        "The GAL critical runtime pack — binding stop rules for headless and interactive runs \
         alike:"
            .to_string(),
    );
    lines.push(String::new());
    lines.push(
        "- `$gal-pipeline`: the first implementation edit is gated on `gal pipeline-preflight \
         <prompt> --receipt <path>` returning `pass` — make no edit before that."
            .to_string(),
    );
    lines.push(
        "- When more than one plan is active, require an explicit `'#file:<prompt>'` path. \
         Never auto-select the first active-plan row."
            .to_string(),
    );
    lines.push(
        "- If a receipt/log write fails under sandbox denial, STOP and rerun the exact `gal` \
         command after approval — do not fall back to role-playing pipeline phases in chat."
            .to_string(),
    );
    lines.push(
        "- The goal-backward verify pass is orchestrator-owned and always runs in-process. It \
         is never a dispatch phase."
            .to_string(),
    );
    lines.push(String::new());

    lines.push("## Working Hours".to_string());
    lines.push(String::new());
    lines.push(
        "Interactive/chat work and direct golem calls honor Wrap-up Time and Hard Stop when \
         enabled. `/gal pipeline` execution itself is exempt. Read \
         `plugins/gal-core/conventions/working-hours.md` for the current rule before assuming \
         either state."
            .to_string(),
    );
    lines.push(String::new());

    lines.push("## Cold Start Order".to_string());
    lines.push(String::new());
    lines.push(
        "1. `.dev/project.md` 2. `.dev/state.md` 3. the active plan's `*.prompt.md`, resolved \
         via the `.dev/state.md` Active Plans row — never a hardcoded transient filename 4. \
         `docs/manual.md` / `plugins/gal-core/workflows/coding.md` only when the current task \
         needs them."
            .to_string(),
    );
    lines.push(String::new());

    if runtime.include_skill_index() && !skill_names.is_empty() {
        lines.push("## Repo Skills".to_string());
        lines.push(String::new());
        lines.push(
            "The following repo-local skills are available by name. Read the corresponding \
             `skills/<name>/SKILL.md` file when full instructions are needed."
                .to_string(),
        );
        lines.push(String::new());
        for skill in skill_names {
            lines.push(format!("- `{skill}` - `skills/{skill}/SKILL.md`"));
        }
        lines.push(String::new());
    }

    lines.push("## Project Context".to_string());
    lines.push(String::new());
    for section in sections {
        lines.push(format!("### {}", section.heading));
        lines.push(String::new());
        lines.push(section.body.clone());
        lines.push(String::new());
    }

    if !detected_skills_block.is_empty() {
        lines.push(detected_skills_block.trim_end().to_string());
        lines.push(String::new());
    }

    lines.push("## Canonical Sources".to_string());
    lines.push(String::new());
    lines.push("- Full project index: `.dev/project.md`".to_string());
    lines.push("- Session state: `.dev/state.md`".to_string());
    lines.push(
        "- Active plan execution prompt: resolve via the `.dev/state.md` Active Plans row"
            .to_string(),
    );
    lines.push("- Operator manual: `docs/manual.md`".to_string());
    lines.push("- Coding workflow contract: `plugins/gal-core/workflows/coding.md`".to_string());
    lines.push(
        "- Conventions: `plugins/gal-core/conventions/` — read the relevant file before acting. \
         Its content is not duplicated here."
            .to_string(),
    );
    for label in personal_convention_labels {
        let stem = label
            .strip_prefix("personal-conventions/")
            .unwrap_or(label.as_str());
        lines.push(format!(
            "- Personal convention (machine-local): `~/.gal/local/conventions/{stem}` — read \
             before related work. Its content is never embedded here."
        ));
    }
    lines.push(String::new());

    if rust_selected && !runtime.has_native_rust_layer() {
        lines.push("## Rust Convention Trigger".to_string());
        lines.push(String::new());
        lines.push(
            "Before any Rust/Cargo work in this repo, read `plugins/gal-core/conventions/rust.md` first."
                .to_string(),
        );
        lines.push(String::new());
    }

    format!("{}\n", lines.join("\n").trim_end())
}

/// Render the two verified conditional Rust layers — Claude `paths` rules and
/// Copilot `applyTo` instructions — when the Rust convention is selected.
/// `None` when it is not: neither layer renders, matching the Runtime Routing
/// Contract's "otherwise render neither" rule.
///
/// `rust_convention_body` is the canonical `plugins/gal-core/conventions/rust.md`
/// content, embedded once beneath each layer's native frontmatter. This is the
/// one place a full convention body is intentionally duplicated — into files
/// whose native conditional-loading mechanism makes them situational rather
/// than always-on, unlike the five slim roots.
pub(crate) fn render_conditional_rust_layers(
    rust_selected: bool,
    rust_convention_body: &str,
) -> Option<[AdapterCandidate; 2]> {
    if !rust_selected {
        return None;
    }
    let body = rust_convention_body.trim();
    let claude = AdapterCandidate {
        path: ".claude/rules/gal-rust.md".to_string(),
        content: format!(
            "---\npaths:\n  - \"**/*.rs\"\n  - \"**/Cargo.toml\"\n  - \"**/Cargo.lock\"\n---\n\n{GAL_LAYER_OWNERSHIP_MARKER}\n\n{body}\n"
        ),
    };
    let copilot = AdapterCandidate {
        path: ".github/instructions/gal-rust.instructions.md".to_string(),
        content: format!(
            "---\napplyTo: \"**/*.rs,**/Cargo.toml,**/Cargo.lock\"\n---\n\n{GAL_LAYER_OWNERSHIP_MARKER}\n\n{body}\n"
        ),
    };
    Some([claude, copilot])
}

/// Classification of one fixed conditional-layer path's on-disk state,
/// computed before any adapter mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerOwnership {
    /// Path does not exist yet — safe to create.
    Absent,
    /// Path exists and carries the exact GAL ownership marker — safe to
    /// update or remove.
    GalOwned,
    /// Path exists without the exact GAL ownership marker — a user-owned
    /// file. Hard collision.
    Collision,
}

/// Classify a fixed layer path's current on-disk state against the exact GAL
/// ownership marker. Read-only — never mutates.
pub(crate) fn classify_layer_ownership(path: &Path) -> Result<LayerOwnership, AdapterError> {
    if !path.is_file() {
        return Ok(LayerOwnership::Absent);
    }
    let content = fs::read_to_string(path)?;
    if content.contains(GAL_LAYER_OWNERSHIP_MARKER) {
        Ok(LayerOwnership::GalOwned)
    } else {
        Ok(LayerOwnership::Collision)
    }
}

/// The action to take for one fixed conditional-layer path, computed by
/// `preflight_conditional_layers`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LayerAction {
    /// Neither rendered nor present on disk — nothing to do.
    Noop,
    /// Safe to create or update this rendered candidate.
    Write(AdapterCandidate),
    /// The Rust convention is no longer selected but a GAL-owned stale layer
    /// exists — remove it.
    RemoveStale(PathBuf),
}

/// Classify both fixed conditional-layer paths and compute their actions
/// before any write. A collision on **either** path aborts the whole call —
/// hard error, no action list returned, so a caller can never partially apply
/// one layer while the other collided. Mirrors the no-partial-output contract
/// already established by `validate_project_md_size` and
/// `validate_root_adapter_budgets`.
pub(crate) fn preflight_conditional_layers(
    repo_root: &Path,
    rendered: Option<&[AdapterCandidate; 2]>,
) -> Result<Vec<LayerAction>, AdapterError> {
    let mut actions = Vec::with_capacity(REPO_ADAPTER_CONDITIONAL_LAYERS.len());
    for layer_path in REPO_ADAPTER_CONDITIONAL_LAYERS {
        let full_path = repo_root.join(layer_path);
        let ownership = classify_layer_ownership(&full_path)?;
        if ownership == LayerOwnership::Collision {
            return Err(AdapterError::Message(format!(
                "{layer_path} exists without the GAL ownership marker — refusing to overwrite a user-owned file. No adapter files were written."
            )));
        }
        let candidate = rendered.and_then(|pair| pair.iter().find(|c| &c.path == layer_path));
        let action = match (candidate, ownership) {
            (Some(candidate), _) => LayerAction::Write(candidate.clone()),
            (None, LayerOwnership::GalOwned) => LayerAction::RemoveStale(full_path),
            (None, LayerOwnership::Absent) => LayerAction::Noop,
            (None, LayerOwnership::Collision) => unreachable!("collision returns Err above"),
        };
        actions.push(action);
    }
    Ok(actions)
}

/// Apply the actions computed by `preflight_conditional_layers`. `dry_run`
/// computes the same report without touching the filesystem, mirroring
/// `write_text`'s existing dry-run contract: the report always reflects the
/// intended action, real I/O only happens when `!dry_run`.
pub(crate) fn apply_layer_actions(
    repo_root: &Path,
    actions: &[LayerAction],
    dry_run: bool,
    report: &mut ProjectionReport,
) -> Result<(), AdapterError> {
    for action in actions {
        match action {
            LayerAction::Noop => {}
            LayerAction::Write(candidate) => {
                let path = repo_root.join(&candidate.path);
                write_text_with_report(&path, &candidate.content, dry_run, report)?;
            }
            LayerAction::RemoveStale(path) => {
                if !dry_run {
                    fs::remove_file(path)?;
                }
                report.removed_paths.push(path.clone());
            }
        }
    }
    Ok(())
}

/// The observable outcome for one repo-adapter path after preflight + write.
/// Mutually exclusive by construction: a `BTreeMap<String, AdapterOutcome>`
/// key can carry only one value, and iteration order is deterministic
/// (sorted by path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdapterOutcome {
    /// Created (did not exist) or updated (existed with different content).
    Written,
    /// Existed with byte-identical content — no write attempted.
    Unchanged,
    /// A GAL-owned conditional layer removed because it is no longer selected.
    Removed,
}

/// Preflight every candidate — root budgets (`validate_root_adapter_budgets`)
/// and conditional-layer ownership (`preflight_conditional_layers`) — then
/// write via the existing `write_text_with_report` helper and
/// `apply_layer_actions`, partitioning every path into exactly one
/// `AdapterOutcome`. A root budget violation or a layer collision aborts the
/// whole call before any write — the two preflights already guarantee this
/// individually; this function only sequences them ahead of the writes and
/// does not weaken either guarantee.
pub(crate) fn render_and_apply_repo_adapters(
    repo_root: &Path,
    root_candidates: &[AdapterCandidate],
    conditional_layers: Option<&[AdapterCandidate; 2]>,
    dry_run: bool,
) -> Result<(ProjectionReport, BTreeMap<String, AdapterOutcome>), AdapterError> {
    validate_root_adapter_budgets(root_candidates)?;
    let layer_actions = preflight_conditional_layers(repo_root, conditional_layers)?;

    let mut report = ProjectionReport::default();
    for candidate in root_candidates {
        let path = repo_root.join(&candidate.path);
        write_text_with_report(&path, &candidate.content, dry_run, &mut report)?;
    }
    apply_layer_actions(repo_root, &layer_actions, dry_run, &mut report)?;

    let written_set: std::collections::HashSet<&PathBuf> = report.written_files.iter().collect();
    let mut outcomes = BTreeMap::new();
    for candidate in root_candidates {
        let path = repo_root.join(&candidate.path);
        let outcome = if written_set.contains(&path) {
            AdapterOutcome::Written
        } else {
            AdapterOutcome::Unchanged
        };
        outcomes.insert(candidate.path.clone(), outcome);
    }
    for (layer_path, action) in REPO_ADAPTER_CONDITIONAL_LAYERS
        .iter()
        .zip(layer_actions.iter())
    {
        match action {
            LayerAction::Noop => {}
            LayerAction::Write(_) => {
                let path = repo_root.join(layer_path);
                let outcome = if written_set.contains(&path) {
                    AdapterOutcome::Written
                } else {
                    AdapterOutcome::Unchanged
                };
                outcomes.insert(layer_path.to_string(), outcome);
            }
            LayerAction::RemoveStale(_) => {
                outcomes.insert(layer_path.to_string(), AdapterOutcome::Removed);
            }
        }
    }

    Ok((report, outcomes))
}

/// Validate `.dev/project.md`'s size against the bounded-index budget before
/// any adapter write happens. Normalizes CRLF to LF first (matching
/// `token-budget.md`'s normalization rule) so the count is platform-stable.
/// Called before any `write_text_with_report` in `run_sync`, so a rejection
/// here happens before any file is touched — no partial output.
fn validate_project_md_size(content: &str) -> Result<(), AdapterError> {
    let normalized = content.replace("\r\n", "\n");
    let size = normalized.len();
    if size > PROJECT_MD_MAX_BYTES {
        return Err(AdapterError::Message(format!(
            ".dev/project.md is {size} bytes (normalized LF UTF-8), exceeding the {PROJECT_MD_MAX_BYTES}-byte bounded-index budget (see plugins/gal-core/conventions/token-budget.md § Bounded Current-Topic Index). Compress the Verified Facts topics before re-running."
        )));
    }
    Ok(())
}

/// Hard cap on a fully rendered repo-adapter root's normalized-LF UTF-8 byte
/// size. Single source of truth for the Budget Contract's cap value —
/// 32,768 passes, 32,769 fails.
pub(crate) const ROOT_ADAPTER_MAX_BYTES: usize = 32_768;

/// One rendered adapter candidate awaiting the budget/collision preflight —
/// its repo-relative path paired with its not-yet-normalized content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdapterCandidate {
    pub path: String,
    pub content: String,
}

/// Validate every candidate's normalized-LF UTF-8 size against
/// `ROOT_ADAPTER_MAX_BYTES` before any write happens. The whole batch is
/// rejected on the first over-budget candidate — the caller is expected to
/// validate every candidate before attempting any write, so a single
/// oversized root leaves every existing adapter file untouched. Mirrors
/// `validate_project_md_size`'s normalization and no-partial-output
/// precedent for `.dev/project.md`.
pub(crate) fn validate_root_adapter_budgets(
    candidates: &[AdapterCandidate],
) -> Result<(), AdapterError> {
    for candidate in candidates {
        let normalized = candidate.content.replace("\r\n", "\n");
        let size = normalized.len();
        if size > ROOT_ADAPTER_MAX_BYTES {
            return Err(AdapterError::Message(format!(
                "{} is {size} bytes (normalized LF UTF-8), exceeding the {ROOT_ADAPTER_MAX_BYTES}-byte root-adapter budget (see plugins/gal-core/conventions/token-budget.md). No adapter files were written.",
                candidate.path
            )));
        }
    }
    Ok(())
}

pub fn run_sync(opts: &SyncOptions) -> Result<ProjectionReport, AdapterError> {
    let project_path = opts.repo_root.join(".dev").join("project.md");
    if !project_path.is_file() {
        return Err(AdapterError::Message(format!(
            "missing required file: {}",
            project_path.display()
        )));
    }

    let project_content = read_markdown_required(&project_path)?;
    validate_project_md_size(&project_content)?;
    let sections = extract_required_project_sections(&project_path, &project_content)?;
    let convention_names = project_convention_file_names(&project_path)?;
    let rust_selected = convention_names.iter().any(|name| name == "rust.md");
    // The canonical Rust body feeds only the two verified conditional layers;
    // no root embeds it. Missing source file → no layers, same skip-if-absent
    // semantics `read_selected_markdown` always had.
    let rust_body = if rust_selected {
        read_selected_markdown(
            &opts.source_root.join("conventions"),
            &["rust.md".to_string()],
        )?
        .into_iter()
        .next()
        .map(|(_, content)| content)
    } else {
        None
    };
    let personal_convention_labels: Vec<String> = personal_convention_docs(&project_content)?
        .into_iter()
        .map(|(label, _)| label)
        .collect();
    let detected_skills_block = detected_language_skills_block(&project_content);
    let skill_names = list_named_children(&opts.source_root.join("skills"))?;

    let mut root_candidates = Vec::with_capacity(REPO_ADAPTER_ROOTS.len());
    for root in REPO_ADAPTER_ROOTS {
        let Some(runtime) = slim_runtime_for_root(root) else {
            return Err(AdapterError::Message(format!(
                "{root} has no rendering identity — REPO_ADAPTER_ROOTS and slim_runtime_for_root have drifted apart"
            )));
        };
        root_candidates.push(AdapterCandidate {
            path: (*root).to_string(),
            content: render_slim_root_document(
                runtime,
                &sections,
                &skill_names,
                rust_selected,
                &personal_convention_labels,
                &detected_skills_block,
            ),
        });
    }

    let conditional_layers = rust_body
        .as_deref()
        .and_then(|body| render_conditional_rust_layers(rust_selected, body));

    let (report, _outcomes) = render_and_apply_repo_adapters(
        &opts.repo_root,
        &root_candidates,
        conditional_layers.as_ref(),
        opts.dry_run,
    )?;
    Ok(report)
}

// ── markdown read/render helpers — gal's own copy (management side keeps its own) ──

fn list_named_children(root: &Path) -> Result<Vec<String>, AdapterError> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    Ok(names)
}

fn read_markdown_required(path: &Path) -> Result<String, AdapterError> {
    fs::read_to_string(path).map_err(AdapterError::from)
}

fn read_selected_markdown(
    root: &Path,
    file_names: &[String],
) -> Result<Vec<(String, String)>, AdapterError> {
    let mut docs = Vec::new();
    for name in file_names {
        let path = root.join(name);
        if path.is_file() {
            docs.push((
                format!("conventions/{name}"),
                read_markdown_required(&path)?,
            ));
        }
    }
    Ok(docs)
}

/// Tokenize a line into lowercase word-boundary tokens. Token characters are
/// alphanumerics plus `#`, `.`, and `+` (so `c#`, `.net`, and `c++` survive as
/// single tokens); every other character is a separator. Shared by the core
/// selector and the personal-conventions / detected-skills matchers so all
/// three consumers use identical word-boundary semantics.
pub(crate) fn language_tokens(line: &str) -> Vec<String> {
    line.to_ascii_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '#' || c == '.' || c == '+'))
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

/// Extract the Language-line word-boundary tokens from `.dev/project.md` content.
/// Empty when there is no `| Language |` row.
fn project_language_tokens(project_content: &str) -> Vec<String> {
    project_content
        .lines()
        .find(|line| line.trim_start().starts_with("| Language |"))
        .map(language_tokens)
        .unwrap_or_default()
}

fn project_convention_file_names(project_path: &Path) -> Result<Vec<String>, AdapterError> {
    let project = read_markdown_required(project_path)?;
    let tokens = project_language_tokens(&project);
    let mut names = vec![
        "conventions.md".to_string(),
        "token-budget.md".to_string(),
        "working-hours.md".to_string(),
    ];
    if tokens.iter().any(|token| token == "rust") {
        names.push("rust.md".to_string());
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// `true` when `.dev/project.md` carries a "Personal Conventions = off" Tech
/// Stack row (case-insensitive substring match on the row text).
fn personal_conventions_off(project_content: &str) -> bool {
    project_content.lines().any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("personal conventions") && lower.contains("off")
    })
}

/// Match a personal convention file's stem against the Language-line tokens.
/// Recognized language stems (with aliases) match only their own language;
/// any other stem is treated as a generic/universal convention and always
/// matches, regardless of Language line.
fn personal_stem_matches(stem: &str, tokens: &[String]) -> bool {
    const LANGUAGE_ALIASES: &[(&str, &[&str])] = &[
        ("csharp", &["csharp", "c#", ".net"]),
        ("typescript", &["typescript", "javascript"]),
        ("go", &["go", "golang"]),
    ];
    let stem_lower = stem.to_ascii_lowercase();
    for (canonical, aliases) in LANGUAGE_ALIASES {
        if stem_lower == *canonical {
            return aliases
                .iter()
                .any(|alias| tokens.iter().any(|t| t == alias));
        }
    }
    true
}

/// Source 2: scan `~/.gal/local/conventions/` for personal convention files
/// that match the project's Language line (or are generic/universal), and
/// return them labeled distinctly from source-1 (gal-core) conventions.
/// Empty when the "Personal Conventions = off" row is present, the personal
/// conventions root cannot be resolved, or the directory does not exist —
/// each of those cases keeps rendering byte-identical to the no-personal-layer
/// baseline.
fn personal_convention_docs(project_content: &str) -> Result<Vec<(String, String)>, AdapterError> {
    if personal_conventions_off(project_content) {
        return Ok(Vec::new());
    }
    let Some(root) = gal_foundation::paths::gal_local_conventions_root() else {
        return Ok(Vec::new());
    };
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let tokens = project_language_tokens(project_content);
    let mut entries: Vec<PathBuf> = fs::read_dir(&root)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
        .collect();
    entries.sort();

    let mut docs = Vec::new();
    for path in entries {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if personal_stem_matches(stem, &tokens) {
            let content = read_markdown_required(&path)?;
            docs.push((format!("personal-conventions/{stem}.md"), content));
        }
    }
    Ok(docs)
}

/// Names failing the safe charset (alphanumerics, `-`, `_`) or exceeding the
/// bounded length are rejected before ever reaching a rendered reference
/// block — an untrusted third-party skill name must never carry markdown,
/// control characters, or unbounded length into an always-on instruction
/// surface.
fn safe_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `true` when the skill's name or origin path carries a token overlapping
/// the project's Language-line tokens. Description text is display-only and
/// is never matched (a false-positive generator for short tokens like `go`).
fn skill_matches_language(skill: &DiscoveredSkill, language_tokens_wanted: &[String]) -> bool {
    let mut skill_tokens = language_tokens(&skill.name);
    skill_tokens.extend(language_tokens(&skill.origin.to_string_lossy()));
    skill_tokens
        .iter()
        .any(|token| language_tokens_wanted.iter().any(|lt| lt == token))
}

/// Source 3: scan installed agent-plugin skills for ones matching the
/// project's Language line, and render a **reference-only** block (skill
/// name + origin + a standing load-first instruction). Never copies skill
/// content. Empty when the "Personal Conventions = off" row is present or
/// there is no Language line or no match — each case keeps rendering
/// byte-identical to the no-detection baseline.
fn detected_language_skills_block(project_content: &str) -> String {
    if personal_conventions_off(project_content) {
        return String::new();
    }
    let tokens = project_language_tokens(project_content);
    if tokens.is_empty() {
        return String::new();
    }

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(cache_root) = gal_foundation::paths::claude_plugins_cache_root() {
        roots.extend(skill_discovery::claude_plugin_skill_roots(&cache_root));
    }
    for root in [
        gal_foundation::paths::shared_agents_skills_root(),
        gal_foundation::paths::copilot_skills_root(),
        gal_foundation::paths::agy_skills_root(),
    ]
    .into_iter()
    .flatten()
    {
        roots.push(root);
    }

    let skills = skill_discovery::discover_skills(&roots);
    let mut hits: Vec<&DiscoveredSkill> = skills
        .iter()
        .filter(|s| safe_skill_name(&s.name) && skill_matches_language(s, &tokens))
        .collect();
    if hits.is_empty() {
        return String::new();
    }
    hits.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.origin.cmp(&b.origin)));

    let mut lines = vec![
        "## Detected Language Skills".to_string(),
        String::new(),
        "The following installed agent-plugin skills matched this project's Language line. \
         Load the skill file first before writing related code. GAL only detects and \
         references these skills — it never copies or manages their content."
            .to_string(),
        String::new(),
    ];
    for skill in hits {
        lines.push(format!(
            "- `{}` — origin: `{}` — load this skill first for related work.",
            skill.name,
            skill.origin.display()
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn write_text_with_report(
    path: &Path,
    content: &str,
    dry_run: bool,
    report: &mut ProjectionReport,
) -> Result<(), AdapterError> {
    if write_text(path, content, dry_run)? {
        report.written_files.push(path.to_path_buf());
    }
    Ok(())
}

fn write_text(path: &Path, content: &str, dry_run: bool) -> Result<bool, AdapterError> {
    let normalized = format!("{}\n", content.trim_end());
    if fs::read_to_string(path)
        .ok()
        .map(|existing| existing == normalized)
        .unwrap_or(false)
    {
        return Ok(false);
    }
    if !dry_run {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, normalized.as_bytes())?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_file(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// A minimal `.dev/project.md` carrying all eight required H2 sections.
    /// `run_sync` fail-closes on a missing or duplicate required section, so
    /// every fixture that reaches the renderer must be section-complete.
    fn full_project_md(language_row: &str) -> String {
        format!(
            "# Project\n\n\
             ## What This Is\n\nFixture project.\n\n\
             ## Tech Stack\n\n| Layer | Technology |\n| --- | --- |\n{language_row}\n\n\
             ## Architecture\n\nSingle crate.\n\n\
             ## Constraints\n\n- none\n\n\
             ## Response Style\n\n- concise\n\n\
             ## Freshness\n\n- none\n\n\
             ## Project Language\n\n- `PROJECT_LANGUAGE`: `en`\n\n\
             ## Protected Paths\n\n- none\n"
        )
    }

    fn write_project(temp: &TempDir, language_line: Option<&str>) -> PathBuf {
        let repo_root = temp.path().join("repo");
        let body = match language_line {
            Some(line) => format!("# Project\n\n{line}\n"),
            None => "# Project\n\nno language table here\n".to_string(),
        };
        let path = repo_root.join(".dev").join("project.md");
        write_file(&path, &body);
        path
    }

    #[test]
    fn selector_drops_removed_language_names() {
        let temp = TempDir::new().unwrap();
        let project_path = write_project(&temp, Some("| Language | C# |"));
        let names = project_convention_file_names(&project_path).unwrap();
        assert!(
            !names
                .iter()
                .any(|n| n == "csharp.md" || n == "go.md" || n == "typescript.md"),
            "selector must never select removed names, got {names:?}"
        );
        assert_eq!(
            names,
            vec!["conventions.md", "token-budget.md", "working-hours.md"]
        );
    }

    #[test]
    fn selector_word_boundary_rejects_django_and_mongodb() {
        let temp = TempDir::new().unwrap();
        let project_path = write_project(&temp, Some("| Language | Python (Django, MongoDB) |"));
        let names = project_convention_file_names(&project_path).unwrap();
        assert!(
            !names.contains(&"go.md".to_string()),
            "substring match on 'go' inside Django/MongoDB must not select go.md, got {names:?}"
        );
    }

    #[test]
    fn selector_rust_token_selects_rust_md() {
        let temp = TempDir::new().unwrap();
        let project_path = write_project(&temp, Some("| Language | Rust |"));
        let names = project_convention_file_names(&project_path).unwrap();
        assert_eq!(
            names,
            vec![
                "conventions.md",
                "rust.md",
                "token-budget.md",
                "working-hours.md"
            ]
        );
    }

    fn set_home(tmp: &Path) {
        #[cfg(windows)]
        std::env::set_var("USERPROFILE", tmp);
        #[cfg(not(windows))]
        std::env::set_var("HOME", tmp);
    }

    #[test]
    fn personal_source2_injects_matching_file_with_distinct_label() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let home = TempDir::new().unwrap();
        set_home(home.path());

        let personal_root = home.path().join(".gal").join("local").join("conventions");
        write_file(
            &personal_root.join("go.md"),
            "# Personal Go style\n\nUse table-driven tests.\n",
        );

        let repo_temp = TempDir::new().unwrap();
        let project_path = write_project(&repo_temp, Some("| Language | Golang |"));
        let project_content = fs::read_to_string(&project_path).unwrap();

        let docs = personal_convention_docs(&project_content).unwrap();
        assert_eq!(
            docs.len(),
            1,
            "expected exactly one personal doc, got {docs:?}"
        );
        assert_eq!(docs[0].0, "personal-conventions/go.md");
        assert!(docs[0].1.contains("Personal Go style"));
    }

    #[test]
    fn personal_source2_off_row_skips_whole_pass() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let home = TempDir::new().unwrap();
        set_home(home.path());

        let personal_root = home.path().join(".gal").join("local").join("conventions");
        write_file(&personal_root.join("go.md"), "# Personal Go style\n");

        let repo_temp = TempDir::new().unwrap();
        let repo_root = repo_temp.path().join("repo");
        write_file(
            &repo_root.join(".dev").join("project.md"),
            "# Project\n\n| Language | Golang |\n| Personal Conventions | OFF |\n",
        );
        let project_content =
            fs::read_to_string(repo_root.join(".dev").join("project.md")).unwrap();

        let docs = personal_convention_docs(&project_content).unwrap();
        assert!(
            docs.is_empty(),
            "off row must skip the whole personal-conventions pass, got {docs:?}"
        );
    }

    #[test]
    fn personal_source2_absent_dir_is_byte_identical() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let home = TempDir::new().unwrap();
        set_home(home.path());
        // No ~/.gal/local/conventions/ created at all.

        let repo_temp = TempDir::new().unwrap();
        let project_path = write_project(&repo_temp, Some("| Language | Golang |"));
        let project_content = fs::read_to_string(&project_path).unwrap();

        let docs = personal_convention_docs(&project_content).unwrap();
        assert!(
            docs.is_empty(),
            "absent personal conventions dir must yield zero docs, got {docs:?}"
        );
    }

    #[test]
    fn detected_skills_block_injects_matching_skill_no_content() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let home = TempDir::new().unwrap();
        set_home(home.path());

        let shared_root = home.path().join(".agents").join("skills");
        write_file(
            &shared_root.join("flutter-style").join("SKILL.md"),
            "---\nname: flutter-style\ndescription: Official Flutter style guide\n---\nSECRET SKILL BODY should never appear in the adapter.\n",
        );

        let repo_temp = TempDir::new().unwrap();
        let project_path = write_project(&repo_temp, Some("| Language | Dart (Flutter) |"));
        let project_content = fs::read_to_string(&project_path).unwrap();

        let block = detected_language_skills_block(&project_content);
        assert!(
            block.contains("## Detected Language Skills"),
            "block must render, got: {block}"
        );
        assert!(block.contains("flutter-style"), "must name the skill");
        assert!(
            block.contains(
                &shared_root
                    .join("flutter-style")
                    .to_string_lossy()
                    .to_string()
            ),
            "must reference the origin path"
        );
        assert!(
            !block.contains("SECRET SKILL BODY"),
            "must never copy skill content into the block"
        );
    }

    #[test]
    fn detected_skills_block_off_row_yields_no_block() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let home = TempDir::new().unwrap();
        set_home(home.path());

        let shared_root = home.path().join(".agents").join("skills");
        write_file(
            &shared_root.join("flutter-style").join("SKILL.md"),
            "---\nname: flutter-style\ndescription: Official Flutter style guide\n---\nBody\n",
        );

        let repo_temp = TempDir::new().unwrap();
        let repo_root = repo_temp.path().join("repo");
        write_file(
            &repo_root.join(".dev").join("project.md"),
            "# Project\n\n| Language | Dart (Flutter) |\n| Personal Conventions | off |\n",
        );
        let project_content =
            fs::read_to_string(repo_root.join(".dev").join("project.md")).unwrap();

        let block = detected_language_skills_block(&project_content);
        assert!(
            block.is_empty(),
            "off row must yield zero block, got: {block}"
        );
    }

    #[test]
    fn detected_skills_block_no_match_is_byte_identical() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let home = TempDir::new().unwrap();
        set_home(home.path());

        let shared_root = home.path().join(".agents").join("skills");
        write_file(
            &shared_root.join("flutter-style").join("SKILL.md"),
            "---\nname: flutter-style\ndescription: Official Flutter style guide\n---\nBody\n",
        );

        let repo_temp = TempDir::new().unwrap();
        let project_path = write_project(&repo_temp, Some("| Language | Rust |"));
        let project_content = fs::read_to_string(&project_path).unwrap();

        let block = detected_language_skills_block(&project_content);
        assert!(
            block.is_empty(),
            "no matching skill must yield zero block, got: {block}"
        );
    }

    #[test]
    fn detected_skills_description_only_go_is_not_matched() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let home = TempDir::new().unwrap();
        set_home(home.path());

        let shared_root = home.path().join(".agents").join("skills");
        write_file(
            &shared_root.join("build-helper").join("SKILL.md"),
            "---\nname: build-helper\ndescription: helps you go build things faster\n---\nBody\n",
        );

        let repo_temp = TempDir::new().unwrap();
        let project_path = write_project(&repo_temp, Some("| Language | Go |"));
        let project_content = fs::read_to_string(&project_path).unwrap();

        let block = detected_language_skills_block(&project_content);
        assert!(
            block.is_empty(),
            "a 'go' token appearing only in description prose must not match, got: {block}"
        );
    }

    #[test]
    fn detected_skills_unsafe_name_is_skipped() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let home = TempDir::new().unwrap();
        set_home(home.path());

        let shared_root = home.path().join(".agents").join("skills");
        // Frontmatter name contains spaces and markdown — fails the safe charset.
        write_file(
            &shared_root.join("go-helper").join("SKILL.md"),
            "---\nname: go helper *unsafe*\ndescription: Go helper\n---\nBody\n",
        );

        let repo_temp = TempDir::new().unwrap();
        let project_path = write_project(&repo_temp, Some("| Language | Go |"));
        let project_content = fs::read_to_string(&project_path).unwrap();

        let block = detected_language_skills_block(&project_content);
        assert!(
            block.is_empty(),
            "unsafe-charset skill name must be skipped, got: {block}"
        );
    }

    #[test]
    fn selector_no_language_fallback_excludes_rust_md() {
        let temp = TempDir::new().unwrap();
        let project_path = write_project(&temp, None);
        let names = project_convention_file_names(&project_path).unwrap();
        assert_eq!(
            names,
            vec!["conventions.md", "token-budget.md", "working-hours.md"]
        );
        assert!(!names.contains(&"rust.md".to_string()));
    }

    #[test]
    fn agents_md_contains_obedience_marker_within_byte_limit() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().join("repo");
        let source_root = temp.path().join("source");

        write_file(
            &repo_root.join(".dev").join("project.md"),
            &full_project_md("| Language | Rust |"),
        );
        write_file(
            &source_root.join("conventions").join("rust.md"),
            "# Rust conventions\n",
        );
        write_file(
            &source_root.join("workflows").join("coding.md"),
            "# Coding flow\n",
        );

        run_sync(&SyncOptions {
            repo_root: repo_root.clone(),
            source_root,
            dry_run: false,
        })
        .unwrap();

        let agents_path = repo_root.join("AGENTS.md");
        let content = fs::read_to_string(&agents_path).unwrap();

        // Marker must be present
        const MARKER: &str = "Named Workflow Obedience";
        assert!(
            content.contains(MARKER),
            "AGENTS.md missing Named Workflow Obedience marker"
        );

        // Byte offset must be < 32768 (official Codex project_doc_max_bytes default)
        let offset = content.find(MARKER).unwrap();
        assert!(
            offset < 32768,
            "Named Workflow Obedience marker at byte offset {offset} >= 32768"
        );

        // Ideally within the first 4096 bytes (first screen)
        assert!(
            offset < 4096,
            "Named Workflow Obedience marker at byte offset {offset} >= 4096 (prefer top-of-doc)"
        );
    }

    #[test]
    fn agents_md_contains_critical_runtime_pack_markers_within_budget() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().join("repo");
        let source_root = temp.path().join("source");

        write_file(
            &repo_root.join(".dev").join("project.md"),
            &full_project_md("| Language | Rust |"),
        );
        write_file(
            &source_root.join("conventions").join("rust.md"),
            "# Rust conventions\n",
        );
        write_file(
            &source_root.join("workflows").join("coding.md"),
            "# Coding flow\n",
        );

        run_sync(&SyncOptions {
            repo_root: repo_root.clone(),
            source_root,
            dry_run: false,
        })
        .unwrap();

        let content = fs::read_to_string(repo_root.join("AGENTS.md")).unwrap();

        // The Codex critical runtime pack must carry every load-bearing marker,
        // and each must land inside the documented Codex project_doc_max_bytes
        // budget (32768) so a partial project-doc read still sees the contract.
        const PACK_MARKERS: &[&str] = &[
            "critical runtime pack",
            "pipeline-preflight",
            "#file:",
            "role-playing pipeline phases",
            "goal-backward verify",
        ];
        for marker in PACK_MARKERS {
            let offset = content
                .find(marker)
                .unwrap_or_else(|| panic!("AGENTS.md missing critical-pack marker: {marker}"));
            assert!(
                offset < 32768,
                "critical-pack marker {marker:?} at byte offset {offset} >= 32768"
            );
        }
    }

    #[test]
    fn validate_project_md_size_accepts_exactly_max() {
        let content = "a".repeat(PROJECT_MD_MAX_BYTES);
        assert_eq!(content.len(), PROJECT_MD_MAX_BYTES);
        assert!(validate_project_md_size(&content).is_ok());
    }

    #[test]
    fn validate_project_md_size_rejects_max_plus_one() {
        let content = "a".repeat(PROJECT_MD_MAX_BYTES + 1);
        let err = validate_project_md_size(&content).unwrap_err();
        assert!(err.to_string().contains("bounded-index budget"));
    }

    #[test]
    fn validate_project_md_size_normalizes_crlf_before_counting() {
        // 30719 'a' bytes + one CRLF line ending = 30721 raw bytes (over cap),
        // but normalizing CRLF -> LF drops 1 byte, landing exactly at the cap.
        let content = format!("{}\r\n", "a".repeat(PROJECT_MD_MAX_BYTES - 1));
        assert_eq!(content.len(), PROJECT_MD_MAX_BYTES + 1);
        assert!(
            validate_project_md_size(&content).is_ok(),
            "CRLF-normalized size should land at exactly the cap and pass"
        );
    }

    #[test]
    fn root_adapter_budget() {
        // Exactly at cap: pass.
        let ok = vec![AdapterCandidate {
            path: "CLAUDE.md".to_string(),
            content: "a".repeat(ROOT_ADAPTER_MAX_BYTES),
        }];
        assert!(validate_root_adapter_budgets(&ok).is_ok());

        // One byte over: fail, naming path/measured/cap.
        let over = vec![AdapterCandidate {
            path: "AGENTS.md".to_string(),
            content: "a".repeat(ROOT_ADAPTER_MAX_BYTES + 1),
        }];
        let err = validate_root_adapter_budgets(&over).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("AGENTS.md"), "must name the path, got {msg:?}");
        assert!(
            msg.contains(&(ROOT_ADAPTER_MAX_BYTES + 1).to_string()),
            "must name measured bytes, got {msg:?}"
        );
        assert!(
            msg.contains(&ROOT_ADAPTER_MAX_BYTES.to_string()),
            "must name the cap, got {msg:?}"
        );

        // CRLF normalization mirrors validate_project_md_size: one CRLF line
        // ending over cap normalizes down to exactly the cap and passes.
        let crlf_content = format!("{}\r\n", "a".repeat(ROOT_ADAPTER_MAX_BYTES - 1));
        assert_eq!(crlf_content.len(), ROOT_ADAPTER_MAX_BYTES + 1);
        let crlf = vec![AdapterCandidate {
            path: "GEMINI.md".to_string(),
            content: crlf_content,
        }];
        assert!(
            validate_root_adapter_budgets(&crlf).is_ok(),
            "CRLF-normalized size should land at exactly the cap and pass"
        );

        // No partial output: every pre-existing adapter file stays
        // byte-identical when any one candidate in the batch is over budget.
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().join("repo");
        let existing_files = [
            (".agents/rules/gal.md", "existing rules content\n"),
            (
                ".github/copilot-instructions.md",
                "existing copilot content\n",
            ),
            ("AGENTS.md", "existing agents content\n"),
            ("CLAUDE.md", "existing claude content\n"),
            ("GEMINI.md", "existing gemini content\n"),
        ];
        for (rel, content) in existing_files {
            write_file(&repo_root.join(rel), content);
        }
        let before: Vec<(PathBuf, String)> = existing_files
            .iter()
            .map(|(rel, _)| {
                let p = repo_root.join(rel);
                let content = fs::read_to_string(&p).unwrap();
                (p, content)
            })
            .collect();

        let batch = vec![
            AdapterCandidate {
                path: ".agents/rules/gal.md".to_string(),
                content: "new small".to_string(),
            },
            AdapterCandidate {
                path: ".github/copilot-instructions.md".to_string(),
                content: "new small".to_string(),
            },
            AdapterCandidate {
                path: "AGENTS.md".to_string(),
                content: "a".repeat(ROOT_ADAPTER_MAX_BYTES + 1),
            },
            AdapterCandidate {
                path: "CLAUDE.md".to_string(),
                content: "new small".to_string(),
            },
            AdapterCandidate {
                path: "GEMINI.md".to_string(),
                content: "new small".to_string(),
            },
        ];
        assert!(
            validate_root_adapter_budgets(&batch).is_err(),
            "a batch with any over-budget candidate must fail as a whole"
        );

        for (path, expected) in before {
            let actual = fs::read_to_string(&path).unwrap();
            assert_eq!(
                actual,
                expected,
                "{} must be byte-identical after a rejected budget batch",
                path.display()
            );
        }
    }

    #[test]
    fn conditional_rust_layers() {
        let body = "# Rust Conventions\n\nCANONICAL_RUST_BODY_MARKER\n";

        // Rust selected: both layers render.
        let layers = render_conditional_rust_layers(true, body)
            .expect("Rust convention selected must render both layers");

        let claude = layers
            .iter()
            .find(|c| c.path == ".claude/rules/gal-rust.md")
            .expect("Claude layer must be present");
        assert!(
            claude.content.starts_with("---\n"),
            "Claude layer must open with YAML frontmatter, got {:?}",
            claude.content
        );
        let claude_frontmatter_end = claude
            .content
            .match_indices("---\n")
            .nth(1)
            .expect("Claude layer frontmatter must be closed")
            .0;
        let claude_frontmatter = &claude.content[..claude_frontmatter_end];
        assert!(
            claude_frontmatter.contains("paths:"),
            "Claude layer must carry `paths` frontmatter, got {claude_frontmatter:?}"
        );
        assert!(
            claude.content.contains(GAL_LAYER_OWNERSHIP_MARKER),
            "Claude layer must carry the GAL ownership marker"
        );
        assert!(
            claude.content.contains("CANONICAL_RUST_BODY_MARKER"),
            "Claude layer must carry the canonical Rust convention body"
        );

        let copilot = layers
            .iter()
            .find(|c| c.path == ".github/instructions/gal-rust.instructions.md")
            .expect("Copilot layer must be present");
        assert!(
            copilot.content.starts_with("---\n"),
            "Copilot layer must open with YAML frontmatter, got {:?}",
            copilot.content
        );
        let copilot_frontmatter_end = copilot
            .content
            .match_indices("---\n")
            .nth(1)
            .expect("Copilot layer frontmatter must be closed")
            .0;
        let copilot_frontmatter = &copilot.content[..copilot_frontmatter_end];
        assert!(
            copilot_frontmatter.contains("applyTo:"),
            "Copilot layer must carry `applyTo` frontmatter, got {copilot_frontmatter:?}"
        );
        assert!(
            copilot.content.contains(GAL_LAYER_OWNERSHIP_MARKER),
            "Copilot layer must carry the GAL ownership marker"
        );
        assert!(
            copilot.content.contains("CANONICAL_RUST_BODY_MARKER"),
            "Copilot layer must carry the canonical Rust convention body"
        );

        // Not selected: neither layer renders.
        assert!(
            render_conditional_rust_layers(false, body).is_none(),
            "a non-Rust project must render neither conditional layer"
        );

        // Paths match the fixed conditional-layer inventory exactly.
        let mut paths: Vec<&str> = layers.iter().map(|c| c.path.as_str()).collect();
        paths.sort_unstable();
        let mut sorted_inventory = REPO_ADAPTER_CONDITIONAL_LAYERS.to_vec();
        sorted_inventory.sort_unstable();
        assert_eq!(
            paths, sorted_inventory,
            "conditional layer paths must match REPO_ADAPTER_CONDITIONAL_LAYERS exactly"
        );
    }

    #[test]
    fn layer_ownership() {
        // Absent: safe to create.
        {
            let temp = TempDir::new().unwrap();
            let repo_root = temp.path().join("repo");
            fs::create_dir_all(&repo_root).unwrap();
            let rendered = render_conditional_rust_layers(true, "# Rust\n\nBODY\n").unwrap();
            let actions = preflight_conditional_layers(&repo_root, Some(&rendered)).unwrap();
            assert!(
                actions.iter().all(|a| matches!(a, LayerAction::Write(_))),
                "an absent path must resolve to Write, got {actions:?}"
            );

            let mut report = ProjectionReport::default();
            apply_layer_actions(&repo_root, &actions, false, &mut report).unwrap();
            for layer_path in REPO_ADAPTER_CONDITIONAL_LAYERS {
                assert!(
                    repo_root.join(layer_path).is_file(),
                    "{layer_path} must be created"
                );
            }
            assert_eq!(report.written_files.len(), 2);
        }

        // GAL-owned: safe to update.
        {
            let temp = TempDir::new().unwrap();
            let repo_root = temp.path().join("repo");
            for layer_path in REPO_ADAPTER_CONDITIONAL_LAYERS {
                write_file(
                    &repo_root.join(layer_path),
                    &format!("---\nold: true\n---\n\n{GAL_LAYER_OWNERSHIP_MARKER}\n\nOLD BODY\n"),
                );
            }
            let rendered = render_conditional_rust_layers(true, "# Rust\n\nNEW BODY\n").unwrap();
            let actions = preflight_conditional_layers(&repo_root, Some(&rendered)).unwrap();
            assert!(
                actions.iter().all(|a| matches!(a, LayerAction::Write(_))),
                "a GAL-owned path with a new candidate must resolve to Write, got {actions:?}"
            );
            let mut report = ProjectionReport::default();
            apply_layer_actions(&repo_root, &actions, false, &mut report).unwrap();
            for layer_path in REPO_ADAPTER_CONDITIONAL_LAYERS {
                let content = fs::read_to_string(repo_root.join(layer_path)).unwrap();
                assert!(
                    content.contains("NEW BODY"),
                    "{layer_path} must be updated to new content, got {content:?}"
                );
            }
        }

        // GAL-owned + Rust deselected: safe to remove.
        {
            let temp = TempDir::new().unwrap();
            let repo_root = temp.path().join("repo");
            for layer_path in REPO_ADAPTER_CONDITIONAL_LAYERS {
                write_file(
                    &repo_root.join(layer_path),
                    &format!("---\nold: true\n---\n\n{GAL_LAYER_OWNERSHIP_MARKER}\n\nOLD BODY\n"),
                );
            }
            let actions = preflight_conditional_layers(&repo_root, None).unwrap();
            assert!(
                actions
                    .iter()
                    .all(|a| matches!(a, LayerAction::RemoveStale(_))),
                "a GAL-owned path with no candidate must resolve to RemoveStale, got {actions:?}"
            );
            let mut report = ProjectionReport::default();
            apply_layer_actions(&repo_root, &actions, false, &mut report).unwrap();
            for layer_path in REPO_ADAPTER_CONDITIONAL_LAYERS {
                assert!(
                    !repo_root.join(layer_path).exists(),
                    "{layer_path} must be removed when stale"
                );
            }
            assert_eq!(report.removed_paths.len(), 2);
        }

        // Collision on the first path: unmarked file preserved, hard error,
        // every other root/layer file stays byte-identical.
        {
            let temp = TempDir::new().unwrap();
            let repo_root = temp.path().join("repo");
            let unmarked = "# My own Claude rules\n\nDo not touch this.\n";
            write_file(&repo_root.join(".claude/rules/gal-rust.md"), unmarked);
            let existing_roots = [
                ("CLAUDE.md", "existing claude root\n"),
                ("AGENTS.md", "existing agents root\n"),
            ];
            for (rel, content) in existing_roots {
                write_file(&repo_root.join(rel), content);
            }
            let rendered = render_conditional_rust_layers(true, "# Rust\n\nBODY\n").unwrap();
            let err = preflight_conditional_layers(&repo_root, Some(&rendered)).unwrap_err();
            assert!(
                err.to_string().contains(".claude/rules/gal-rust.md"),
                "collision error must name the path, got {err}"
            );

            assert_eq!(
                fs::read_to_string(repo_root.join(".claude/rules/gal-rust.md")).unwrap(),
                unmarked,
                "unmarked collision file must be preserved exactly"
            );
            assert!(
                !repo_root
                    .join(".github/instructions/gal-rust.instructions.md")
                    .exists(),
                "the second layer must not be created when the first layer collides"
            );
            for (rel, expected) in existing_roots {
                assert_eq!(
                    fs::read_to_string(repo_root.join(rel)).unwrap(),
                    expected,
                    "{rel} must stay byte-identical when a layer collision aborts the batch"
                );
            }
        }

        // Collision on the second path still aborts before the first path's
        // safe action is ever computed as a returned Vec.
        {
            let temp = TempDir::new().unwrap();
            let repo_root = temp.path().join("repo");
            fs::create_dir_all(&repo_root).unwrap();
            let unmarked = "# My own Copilot instructions\n";
            write_file(
                &repo_root.join(".github/instructions/gal-rust.instructions.md"),
                unmarked,
            );
            let rendered = render_conditional_rust_layers(true, "# Rust\n\nBODY\n").unwrap();
            assert!(preflight_conditional_layers(&repo_root, Some(&rendered)).is_err());
            assert!(
                !repo_root.join(".claude/rules/gal-rust.md").exists(),
                "the first (non-colliding) layer must not be created when the second layer collides"
            );
            assert_eq!(
                fs::read_to_string(repo_root.join(".github/instructions/gal-rust.instructions.md"))
                    .unwrap(),
                unmarked,
                "unmarked collision file must be preserved exactly"
            );
        }

        // Dry-run: no filesystem mutation at all, report still populated.
        {
            let temp = TempDir::new().unwrap();
            let repo_root = temp.path().join("repo");
            fs::create_dir_all(&repo_root).unwrap();
            let rendered = render_conditional_rust_layers(true, "# Rust\n\nBODY\n").unwrap();
            let actions = preflight_conditional_layers(&repo_root, Some(&rendered)).unwrap();
            let mut report = ProjectionReport::default();
            apply_layer_actions(&repo_root, &actions, true, &mut report).unwrap();
            for layer_path in REPO_ADAPTER_CONDITIONAL_LAYERS {
                assert!(
                    !repo_root.join(layer_path).exists(),
                    "{layer_path} must not exist on disk after a dry-run"
                );
            }
            assert_eq!(
                report.written_files.len(),
                2,
                "dry-run report must still reflect the intended write"
            );
        }
    }

    fn full_root_candidates() -> Vec<AdapterCandidate> {
        REPO_ADAPTER_ROOTS
            .iter()
            .map(|p| AdapterCandidate {
                path: p.to_string(),
                content: format!("{p} content\n"),
            })
            .collect()
    }

    #[test]
    fn projection_report() {
        // Create + unchanged + update, all five roots in one call.
        {
            let temp = TempDir::new().unwrap();
            let repo_root = temp.path().join("repo");
            fs::create_dir_all(&repo_root).unwrap();
            // CLAUDE.md's pre-existing content must match what
            // full_root_candidates() will generate for it (`"CLAUDE.md content\n"`)
            // so it lands in the Unchanged bucket; GEMINI.md's pre-existing
            // content deliberately differs so it lands in Written (update).
            write_file(&repo_root.join("CLAUDE.md"), "CLAUDE.md content\n");
            write_file(&repo_root.join("GEMINI.md"), "old gemini content\n");

            let root_candidates = full_root_candidates();

            let (report, outcomes) =
                render_and_apply_repo_adapters(&repo_root, &root_candidates, None, false).unwrap();

            assert_eq!(
                outcomes.get(".agents/rules/gal.md"),
                Some(&AdapterOutcome::Written)
            );
            assert_eq!(
                outcomes.get(".github/copilot-instructions.md"),
                Some(&AdapterOutcome::Written)
            );
            assert_eq!(outcomes.get("AGENTS.md"), Some(&AdapterOutcome::Written));
            assert_eq!(outcomes.get("CLAUDE.md"), Some(&AdapterOutcome::Unchanged));
            assert_eq!(outcomes.get("GEMINI.md"), Some(&AdapterOutcome::Written));

            assert!(!report.written_files.contains(&repo_root.join("CLAUDE.md")));
            assert!(report.written_files.contains(&repo_root.join("GEMINI.md")));
            assert_eq!(
                outcomes.len(),
                5,
                "every root must land in exactly one bucket"
            );
        }

        // Stale removal via conditional layers, alongside unrelated roots.
        {
            let temp = TempDir::new().unwrap();
            let repo_root = temp.path().join("repo");
            for layer_path in REPO_ADAPTER_CONDITIONAL_LAYERS {
                write_file(
                    &repo_root.join(layer_path),
                    &format!("{GAL_LAYER_OWNERSHIP_MARKER}\n\nold\n"),
                );
            }
            let root_candidates = full_root_candidates();

            let (report, outcomes) =
                render_and_apply_repo_adapters(&repo_root, &root_candidates, None, false).unwrap();
            for layer_path in REPO_ADAPTER_CONDITIONAL_LAYERS {
                assert_eq!(outcomes.get(*layer_path), Some(&AdapterOutcome::Removed));
                assert!(!repo_root.join(layer_path).exists());
            }
            assert_eq!(report.removed_paths.len(), 2);
            assert_eq!(
                outcomes.len(),
                7,
                "five roots plus two removed layers must all be present"
            );
        }

        // Dry-run: outcomes computed, no filesystem mutation.
        {
            let temp = TempDir::new().unwrap();
            let repo_root = temp.path().join("repo");
            fs::create_dir_all(&repo_root).unwrap();
            let root_candidates = full_root_candidates();

            let (_, outcomes) =
                render_and_apply_repo_adapters(&repo_root, &root_candidates, None, true).unwrap();
            assert_eq!(outcomes.len(), 5);
            assert!(outcomes.values().all(|o| *o == AdapterOutcome::Written));
            for root_path in REPO_ADAPTER_ROOTS {
                assert!(
                    !repo_root.join(root_path).exists(),
                    "{root_path} must not exist after dry-run"
                );
            }
        }

        // Collision: whole call errors, no root is created.
        {
            let temp = TempDir::new().unwrap();
            let repo_root = temp.path().join("repo");
            write_file(&repo_root.join(".claude/rules/gal-rust.md"), "user owned\n");
            let root_candidates = full_root_candidates();
            let layers = render_conditional_rust_layers(true, "# Rust\n\nBODY\n").unwrap();

            let result =
                render_and_apply_repo_adapters(&repo_root, &root_candidates, Some(&layers), false);
            assert!(result.is_err());
            for root_path in REPO_ADAPTER_ROOTS {
                assert!(
                    !repo_root.join(root_path).exists(),
                    "{root_path} must not be created when a layer collision aborts the whole call"
                );
            }
        }

        // Root budget violation: whole call errors before any write.
        {
            let temp = TempDir::new().unwrap();
            let repo_root = temp.path().join("repo");
            fs::create_dir_all(&repo_root).unwrap();
            let mut root_candidates = full_root_candidates();
            root_candidates[0].content = "a".repeat(ROOT_ADAPTER_MAX_BYTES + 1);

            let result = render_and_apply_repo_adapters(&repo_root, &root_candidates, None, false);
            assert!(result.is_err());
            for root_path in REPO_ADAPTER_ROOTS {
                assert!(
                    !repo_root.join(root_path).exists(),
                    "no root may be created when a budget violation aborts the whole call"
                );
            }
        }
    }

    #[test]
    fn run_sync_rejects_oversized_project_md_with_no_partial_write() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().join("repo");
        let source_root = temp.path().join("source");

        let oversized = format!("# Project\n\n{}\n", "a".repeat(PROJECT_MD_MAX_BYTES + 1));
        write_file(&repo_root.join(".dev").join("project.md"), &oversized);
        write_file(
            &source_root.join("workflows").join("coding.md"),
            "# Coding flow\n",
        );

        let result = run_sync(&SyncOptions {
            repo_root: repo_root.clone(),
            source_root,
            dry_run: false,
        });

        assert!(result.is_err(), "oversized project.md must reject the sync");

        // No partial output: none of the five adapter targets were written.
        let targets = [
            repo_root.join(".github").join("copilot-instructions.md"),
            repo_root.join("GEMINI.md"),
            repo_root.join("CLAUDE.md"),
            repo_root.join("AGENTS.md"),
            repo_root.join(".agents").join("rules").join("gal.md"),
        ];
        for target in targets {
            assert!(
                !target.exists(),
                "{} must not exist after a rejected sync (no partial output)",
                target.display()
            );
        }
    }

    #[test]
    fn adapters_name_only_existing_commands() {
        // Generated adapters must not reference removed commands (gal setup / gal sync).
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().join("repo");
        let source_root = temp.path().join("source");

        write_file(
            &repo_root.join(".dev").join("project.md"),
            &full_project_md("| Language | Rust |"),
        );
        write_file(
            &source_root.join("conventions").join("rust.md"),
            "# Rust conventions\n",
        );
        write_file(
            &source_root.join("workflows").join("coding.md"),
            "# Coding flow\n",
        );

        run_sync(&SyncOptions {
            repo_root: repo_root.clone(),
            source_root,
            dry_run: false,
        })
        .unwrap();

        let claude = fs::read_to_string(repo_root.join("CLAUDE.md")).unwrap();
        let rules =
            fs::read_to_string(repo_root.join(".agents").join("rules").join("gal.md")).unwrap();
        for (name, content) in [("CLAUDE.md", &claude), ("gal.md", &rules)] {
            assert!(
                !content.contains("gal setup"),
                "{name} must not reference removed `gal setup`"
            );
            assert!(
                !content.contains("gal sync"),
                "{name} must not reference removed `gal sync`"
            );
        }
        assert!(
            claude.contains("gal init"),
            "CLAUDE.md must name the real regenerate command `gal init`"
        );
        assert!(
            rules.contains("gal init"),
            "workspace rules must name the real regenerate command `gal init`"
        );
    }

    #[test]
    fn run_sync_writes_workspace_rules_from_project_sources() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().join("repo");
        let source_root = temp.path().join("source");

        write_file(
            &repo_root.join(".dev").join("project.md"),
            &full_project_md("| Language | Rust |"),
        );
        write_file(
            &source_root.join("conventions").join("rust.md"),
            "# Rust conventions\n",
        );
        write_file(
            &source_root.join("workflows").join("coding.md"),
            "# Coding flow\n",
        );

        let report = run_sync(&SyncOptions {
            repo_root: repo_root.clone(),
            source_root,
            dry_run: false,
        })
        .unwrap();

        let rules_path = repo_root.join(".agents").join("rules").join("gal.md");
        let rendered = fs::read_to_string(&rules_path).unwrap();
        assert!(rendered.contains("# GAL Workspace Rules"));
        // Slim contract: required project sections are embedded, source
        // bodies are routed by pointer instead of concatenated in.
        assert!(rendered.contains("### What This Is"));
        assert!(rendered.contains("plugins/gal-core/workflows/coding.md"));
        assert!(
            !rendered.contains("# Rust conventions"),
            "the Rust convention body must not be embedded in a root"
        );
        assert!(report.written_files.contains(&rules_path));

        // Rust selected → both verified conditional layers render, carrying
        // the canonical body under the GAL ownership marker.
        let claude_layer =
            fs::read_to_string(repo_root.join(".claude").join("rules").join("gal-rust.md"))
                .unwrap();
        assert!(claude_layer.contains(GAL_LAYER_OWNERSHIP_MARKER));
        assert!(claude_layer.contains("# Rust conventions"));
        let copilot_layer = fs::read_to_string(
            repo_root
                .join(".github")
                .join("instructions")
                .join("gal-rust.instructions.md"),
        )
        .unwrap();
        assert!(copilot_layer.contains("applyTo:"));

        // all 5 adapters written
        assert!(repo_root.join("CLAUDE.md").is_file());
        assert!(repo_root.join("AGENTS.md").is_file());
        assert!(repo_root.join("GEMINI.md").is_file());
        assert!(repo_root
            .join(".github")
            .join("copilot-instructions.md")
            .is_file());
    }

    // ── resolve_source_root_in tests ─────────────────────────────────────

    fn make_gal_core(base: &Path) -> PathBuf {
        for d in &["commands", "agents", "skills", "conventions", "workflows"] {
            fs::create_dir_all(base.join(d)).unwrap();
        }
        base.to_path_buf()
    }

    #[test]
    fn resolve_source_root_in_finds_plugins_gal_core_under_cwd_ancestor() {
        let temp = TempDir::new().unwrap();
        // repo_root/plugins/gal-core is the source
        let gal_core = temp.path().join("plugins").join("gal-core");
        make_gal_core(&gal_core);
        // Mark as repo root with .git
        fs::create_dir(temp.path().join(".git")).unwrap();
        // cwd is a sub-directory
        let cwd = temp.path().join("crates").join("cli");
        fs::create_dir_all(&cwd).unwrap();

        let result = resolve_source_root_in(&cwd, Path::new("/nonexistent/exe/dir"));
        assert_eq!(result, Some(gal_core));
    }

    #[test]
    fn resolve_source_root_in_stops_at_git_boundary_no_parent_leak() {
        let temp = TempDir::new().unwrap();
        // parent repo has plugins/gal-core
        let parent_gal_core = temp.path().join("plugins").join("gal-core");
        make_gal_core(&parent_gal_core);
        fs::create_dir(temp.path().join(".git")).unwrap();
        // child repo is a nested git repo with no gal-core
        let child_repo = temp.path().join("nested");
        fs::create_dir_all(&child_repo).unwrap();
        fs::create_dir(child_repo.join(".git")).unwrap();
        let cwd = child_repo.join("src");
        fs::create_dir_all(&cwd).unwrap();

        // Should NOT find parent's gal-core (boundary is child's .git)
        let result = resolve_source_root_in(&cwd, Path::new("/nonexistent/exe/dir"));
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_source_root_in_falls_back_to_exe_dir() {
        let temp = TempDir::new().unwrap();
        // exe-side has flat gal-core layout
        let exe_source = temp.path().join("gal-core");
        make_gal_core(&exe_source);
        // cwd has no gal-core and no .git
        let cwd = temp.path().join("some_other_dir");
        fs::create_dir_all(&cwd).unwrap();

        let result = resolve_source_root_in(&cwd, &exe_source.join("bin"));
        // exe_source itself should be found when walking ancestors of exe_source/bin
        assert_eq!(result, Some(exe_source));
    }

    /// A `.dev/project.md` body carrying all eight required H2 sections, with a
    /// fenced block under Tech Stack whose contents must not be read as structure.
    fn canonical_project_md() -> String {
        let mut out = String::from("# Project\n\n| Language | Rust |\n\n");
        for heading in REQUIRED_PROJECT_SECTIONS {
            out.push_str(&format!("## {heading}\n\nbody of {heading}\n\n"));
            if *heading == "Tech Stack" {
                out.push_str("```json\n{ \"note\": \"## Not A Heading\" }\n```\n\n");
            }
        }
        out
    }

    #[test]
    fn required_project_sections() {
        let path = Path::new(".dev/project.md");

        // Canonical set: every required section extracted, in canonical order.
        let content = canonical_project_md();
        let sections = extract_required_project_sections(path, &content)
            .expect("canonical project.md must extract cleanly");
        let headings: Vec<&str> = sections.iter().map(|s| s.heading).collect();
        assert_eq!(
            headings,
            REQUIRED_PROJECT_SECTIONS.to_vec(),
            "sections must come back in canonical order"
        );
        assert!(
            sections[0].body.contains("body of What This Is"),
            "each section must carry its own body, got {:?}",
            sections[0].body
        );

        // A `## ` line inside a fenced block is body text, not a section break.
        let tech = sections
            .iter()
            .find(|s| s.heading == "Tech Stack")
            .expect("Tech Stack must be extracted");
        assert!(
            tech.body.contains("## Not A Heading"),
            "a fenced `## ` line must stay inside the section body, got {:?}",
            tech.body
        );

        // Missing heading: named failure.
        let missing = content.replace("## Protected Paths", "## Something Else");
        let err = extract_required_project_sections(path, &missing)
            .expect_err("a missing required section must fail closed");
        let msg = err.to_string();
        assert!(
            msg.contains("Protected Paths") && msg.contains("missing"),
            "missing-section error must name the heading, got {msg:?}"
        );
        assert!(
            msg.contains(".dev/project.md") || msg.contains(".dev\\project.md"),
            "missing-section error must name the source path, got {msg:?}"
        );

        // Duplicate heading: named failure.
        let duplicated = format!("{content}\n## Constraints\n\na second one\n");
        let err = extract_required_project_sections(path, &duplicated)
            .expect_err("a duplicated required section must fail closed");
        let msg = err.to_string();
        assert!(
            msg.contains("Constraints") && msg.contains('2'),
            "duplicate-section error must name the heading and its count, got {msg:?}"
        );
    }

    #[test]
    fn slim_root() {
        let sections: Vec<ProjectSection> = REQUIRED_PROJECT_SECTIONS
            .iter()
            .map(|heading| ProjectSection {
                heading,
                body: format!("body of {heading}"),
            })
            .collect();
        let skill_names = vec!["result-pattern".to_string()];

        // Ground-truth distinctive markers pulled from the real full bodies —
        // these headings only exist if the whole document were concatenated in,
        // never as part of the slim canonical-pointer rendering.
        let conventions_body = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("plugins")
                .join("gal-core")
                .join("conventions")
                .join("conventions.md"),
        )
        .unwrap();
        let coding_body = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("plugins")
                .join("gal-core")
                .join("workflows")
                .join("coding.md"),
        )
        .unwrap();
        assert!(conventions_body.contains("## Active Conventions"));
        assert!(coding_body.contains("## Choosing Planning Depth"));

        const SAFETY_COLD_START_PROJECT_MARKERS: &[&str] = &[
            "Named Workflow Obedience",
            "pipeline-preflight",
            "working-hours.md",
            "Cold Start",
        ];

        for runtime in [
            SlimRuntime::Antigravity,
            SlimRuntime::Copilot,
            SlimRuntime::Codex,
            SlimRuntime::Claude,
            SlimRuntime::Gemini,
        ] {
            let content =
                render_slim_root_document(runtime, &sections, &skill_names, true, &[], "");

            for marker in SAFETY_COLD_START_PROJECT_MARKERS {
                assert!(
                    content.contains(marker),
                    "{runtime:?} root missing required marker {marker:?}"
                );
            }
            for heading in REQUIRED_PROJECT_SECTIONS {
                assert!(
                    content.contains(&format!("### {heading}")),
                    "{runtime:?} root missing required project section heading {heading:?}"
                );
            }

            // Rust trigger: present for pointer-based runtimes, absent for the
            // two runtimes with a verified native conditional-layer mechanism.
            let has_trigger = content.contains("plugins/gal-core/conventions/rust.md");
            assert_eq!(
                has_trigger,
                !runtime.has_native_rust_layer(),
                "{runtime:?} root Rust-trigger presence mismatch (has_native_rust_layer={})",
                runtime.has_native_rust_layer()
            );

            // No full convention/workflow bodies: the real full-body markers
            // must never leak into a slim root.
            assert!(
                !content.contains("## Active Conventions"),
                "{runtime:?} root must not embed the full conventions.md body"
            );
            assert!(
                !content.contains("## Choosing Planning Depth"),
                "{runtime:?} root must not embed the full coding.md body"
            );
        }

        // Rust trigger absent entirely when the Rust convention is not selected.
        let no_rust_content =
            render_slim_root_document(SlimRuntime::Codex, &sections, &skill_names, false, &[], "");
        assert!(
            !no_rust_content.contains("plugins/gal-core/conventions/rust.md"),
            "Rust trigger must be absent when the Rust convention is not selected"
        );

        // Skill index: included for Codex/Claude/Gemini, omitted for Copilot
        // and the Antigravity workspace-rules root.
        let claude_content =
            render_slim_root_document(SlimRuntime::Claude, &sections, &skill_names, true, &[], "");
        assert!(
            claude_content.contains("## Repo Skills") && claude_content.contains("result-pattern")
        );
        let copilot_content =
            render_slim_root_document(SlimRuntime::Copilot, &sections, &skill_names, true, &[], "");
        assert!(!copilot_content.contains("## Repo Skills"));
        let antigravity_content = render_slim_root_document(
            SlimRuntime::Antigravity,
            &sections,
            &skill_names,
            true,
            &[],
            "",
        );
        assert!(!antigravity_content.contains("## Repo Skills"));

        // Machine-local discovery stays routing-only: a personal convention
        // renders as a pointer line (never its body), and the detected-skills
        // block (already reference-only) is carried through verbatim.
        let routed = render_slim_root_document(
            SlimRuntime::Claude,
            &sections,
            &skill_names,
            true,
            &["personal-conventions/rust.md".to_string()],
            "## Detected Language Skills\n\n- `sample-skill` — origin: `sample/origin` — load this skill first for related work.\n",
        );
        assert!(routed.contains("`~/.gal/local/conventions/rust.md`"));
        assert!(routed.contains("## Detected Language Skills"));
        assert!(routed.contains("sample-skill"));

        // slim_runtime_for_root parity with the fixed inventory: every root path
        // resolves, and it resolves to the runtime this test exercised for it.
        assert_eq!(
            slim_runtime_for_root(".agents/rules/gal.md"),
            Some(SlimRuntime::Antigravity)
        );
        assert_eq!(
            slim_runtime_for_root(".github/copilot-instructions.md"),
            Some(SlimRuntime::Copilot)
        );
        assert_eq!(slim_runtime_for_root("AGENTS.md"), Some(SlimRuntime::Codex));
        assert_eq!(
            slim_runtime_for_root("CLAUDE.md"),
            Some(SlimRuntime::Claude)
        );
        assert_eq!(
            slim_runtime_for_root("GEMINI.md"),
            Some(SlimRuntime::Gemini)
        );
        assert_eq!(slim_runtime_for_root("gal-engine/whatever"), None);
    }

    #[test]
    fn repo_adapter_inventory() {
        assert_eq!(
            REPO_ADAPTER_ROOTS.len(),
            5,
            "expected exactly 5 root adapter paths, got {:?}",
            REPO_ADAPTER_ROOTS
        );
        assert_eq!(
            REPO_ADAPTER_CONDITIONAL_LAYERS.len(),
            2,
            "expected exactly 2 conditional layer paths, got {:?}",
            REPO_ADAPTER_CONDITIONAL_LAYERS
        );
        // Ownership marker must be a non-empty HTML comment for collision preflight.
        assert!(
            GAL_LAYER_OWNERSHIP_MARKER.starts_with("<!--")
                && GAL_LAYER_OWNERSHIP_MARKER.ends_with("-->"),
            "GAL_LAYER_OWNERSHIP_MARKER must be a well-formed HTML comment"
        );

        // No machine-projection paths in the repo-adapter inventory.
        for path in REPO_ADAPTER_ROOTS
            .iter()
            .chain(REPO_ADAPTER_CONDITIONAL_LAYERS.iter())
        {
            assert!(
                !path.contains("gal-engine"),
                "inventory path {path:?} must not reference gal-engine"
            );
            assert!(
                !path.contains("projection"),
                "inventory path {path:?} must not reference projection"
            );
        }

        // No duplicates across the full inventory.
        let all: Vec<&str> = REPO_ADAPTER_ROOTS
            .iter()
            .chain(REPO_ADAPTER_CONDITIONAL_LAYERS.iter())
            .copied()
            .collect();
        let mut sorted_all = all.clone();
        sorted_all.sort_unstable();
        sorted_all.dedup();
        assert_eq!(
            all.len(),
            sorted_all.len(),
            "inventory must contain no duplicate paths"
        );

        // Roots are in stable ascending-sort order.
        let mut sorted_roots: Vec<&str> = REPO_ADAPTER_ROOTS.to_vec();
        sorted_roots.sort_unstable();
        assert_eq!(
            REPO_ADAPTER_ROOTS.to_vec(),
            sorted_roots,
            "REPO_ADAPTER_ROOTS must be in ascending sort order"
        );

        // Conditional layers are in stable ascending-sort order.
        let mut sorted_layers: Vec<&str> = REPO_ADAPTER_CONDITIONAL_LAYERS.to_vec();
        sorted_layers.sort_unstable();
        assert_eq!(
            REPO_ADAPTER_CONDITIONAL_LAYERS.to_vec(),
            sorted_layers,
            "REPO_ADAPTER_CONDITIONAL_LAYERS must be in ascending sort order"
        );
    }
}
