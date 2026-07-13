//! Repo-adapter render — gal's own copy (gal tree / workflow).
//!
//! Generates the 5 repo-local adapter documents (`.github/copilot-instructions.md`,
//! `GEMINI.md`, `CLAUDE.md`, `AGENTS.md`, `.agents/rules/gal.md`) from `.dev/project.md`
//! + selected conventions + `workflows/coding.md`.
//!
//! This is the **repo-adapter render** (workflow), distinct from ccync's
//! **canonical-root render** (machine bake). It was de-hybridized out of
//! `projection` in S2 so the gal product owns its own `gal sync` render with
//! self-owned report/error types and no `use projection::`. The shared markdown
//! read helpers are copied per side (decoupling > DRY); ccync keeps its own.

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
    let convention_names = project_convention_file_names(&project_path)?;
    let mut conventions =
        read_selected_markdown(&opts.source_root.join("conventions"), &convention_names)?;
    conventions.extend(personal_convention_docs(&project_content)?);
    let detected_skills_block = detected_language_skills_block(&project_content);
    let workflow = read_markdown_required(&opts.source_root.join("workflows").join("coding.md"))?;
    let skill_names = list_named_children(&opts.source_root.join("skills"))?;

    let copilot_content = render_adapter_document(
        "Copilot Instructions",
        &[
            "This is the repo-local adapter for GitHub Copilot.",
            "Skills in this repo's `skills/` directory are listed by name only — Copilot discovers GAL commands from the global runtime install.",
            "This generator never modifies `~/.copilot/skills/` or any other machine-level configuration.",
        ],
        &project_content,
        &conventions,
        &detected_skills_block,
        &workflow,
        &skill_names,
        false,
    );
    let gemini_content = render_adapter_document(
        "GEMINI Context",
        &[
            "This is the repo-local Google CLI adapter consumed by Antigravity CLI and retained under the `GEMINI.md` filename for Google-runtime compatibility.",
            "Repo-local skills are indexed below by name so Antigravity CLI and remaining Google CLI surfaces can discover them without duplicating every skill body in the adapter.",
            "This generator does not mutate `~/.gemini/gal-context.md` or other machine-level Google CLI configuration.",
        ],
        &project_content,
        &conventions,
        &detected_skills_block,
        &workflow,
        &skill_names,
        true,
    );
    let claude_content = render_adapter_document(
        "CLAUDE Context",
        &[
            "This is the repo-local adapter for Claude Code.",
            "Repo-local skills are indexed below by name so Claude Code can find the right skill file without duplicating every skill body in the adapter.",
            "This generator only writes repo-local adapters such as `CLAUDE.md`; regenerate them with `gal init`.",
        ],
        &project_content,
        &conventions,
        &detected_skills_block,
        &workflow,
        &skill_names,
        true,
    );
    let agents_content = render_adapter_document(
        "GAL Agent Instructions",
        &[
            "**Named Workflow Obedience (binding):** When the user invokes any GAL named workflow \
             — `$gal-pipeline`, `$gal finalize`, `$gal-status`, `$deep-planning`, \
             `$refining-plan`, `$plan-to-prompt` — OR expresses intent to implement, plan, \
             review, or finalize work in this repo (中文意圖：「實作」「跑 pipeline」「規劃」\
             「refine plan」「finalize」「審查」/ EN intent: \"implement\", \"run the pipeline\", \
             \"plan this\", \"finalize\", \"review\") — your **first action** MUST be to load \
             and execute the corresponding `~/.agents/skills/<name>/SKILL.md` and its \
             precondition/receipt gate. Do NOT switch to generic autonomous coding, batch edits, \
             or a summary response. Violating this rule is a `workflow contract violation` / \
             `named workflow obedience failure`.",
            "**GAL critical runtime pack (headless/Codex):** For `$gal-pipeline`, the \
             first implementation edit is gated on `gal pipeline-preflight <prompt> \
             --receipt <path>` returning `pass` — make no edit before that. When more than \
             one plan is active, require an explicit PowerShell-quoted `'#file:<prompt>'` \
             path and never auto-select the first plan row. If a Rust gate, receipt, or log \
             write fails under sandbox denial, STOP and rerun the exact `gal` command with \
             approval — do NOT fall back to role-playing pipeline phases in chat. Dispatch \
             phases only via `gal dispatch-script ... '#file:<prompt>'`; goal-backward verify \
             stays in-process (never a dispatch phase).",
            "This is the shared cross-CLI contract generated from repo sources for Copilot CLI, Codex CLI, Antigravity workspace rules, remaining Google CLI bridges, and Claude Code CLI.",
            "Repo-local skills are indexed below by name so runtimes can discover the right skill file without duplicating every skill body in this shared adapter.",
            "This file is generated by `/gal init`. Do not edit manually.",
        ],
        &project_content,
        &conventions,
        &detected_skills_block,
        &workflow,
        &skill_names,
        true,
    );
    let workspace_rules_content = render_adapter_document(
        "GAL Workspace Rules",
        &[
            "This is the repo-local workspace rule file for tools that read `.agents/rules/gal.md`.",
            "It is generated from `.dev/project.md`, the selected conventions, and `workflows/coding.md`.",
            "Regenerate it with `gal init` after changing the project summary or shared workflow sources.",
        ],
        &project_content,
        &conventions,
        &detected_skills_block,
        &workflow,
        &[],
        false,
    );

    let mut report = ProjectionReport::default();
    write_text_with_report(
        &opts
            .repo_root
            .join(".github")
            .join("copilot-instructions.md"),
        &copilot_content,
        opts.dry_run,
        &mut report,
    )?;
    write_text_with_report(
        &opts.repo_root.join("GEMINI.md"),
        &gemini_content,
        opts.dry_run,
        &mut report,
    )?;
    write_text_with_report(
        &opts.repo_root.join("CLAUDE.md"),
        &claude_content,
        opts.dry_run,
        &mut report,
    )?;
    write_text_with_report(
        &opts.repo_root.join("AGENTS.md"),
        &agents_content,
        opts.dry_run,
        &mut report,
    )?;
    write_text_with_report(
        &opts.repo_root.join(".agents").join("rules").join("gal.md"),
        &workspace_rules_content,
        opts.dry_run,
        &mut report,
    )?;
    Ok(report)
}

// ── markdown read/render helpers — gal's own copy (ccync keeps its own in support.rs) ──

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
            return aliases.iter().any(|alias| tokens.iter().any(|t| t == alias));
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

#[allow(clippy::too_many_arguments)] // private render helper; each param is a distinct content slice, not a natural struct
fn render_adapter_document(
    title: &str,
    preamble: &[&str],
    project_content: &str,
    conventions: &[(String, String)],
    detected_skills_block: &str,
    workflow_content: &str,
    skill_names: &[String],
    include_skill_index: bool,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("# {title}"));
    lines.push(String::new());
    lines.push("> Generated by `/gal init`. Do not edit manually.".to_string());
    lines.push(
        "> Update `.dev/project.md` or the GAL source docs, then rerun `/gal init`.".to_string(),
    );
    lines.push(String::new());
    lines.push("## Adapter Rules".to_string());
    lines.push(String::new());
    for entry in preamble {
        lines.push(format!("- {entry}"));
    }
    lines.push(String::new());
    if include_skill_index && !skill_names.is_empty() {
        lines.push("## Repo Skills".to_string());
        lines.push(String::new());
        lines.push(
            "The following repo-local skills are available by name. Read the corresponding `skills/<name>/SKILL.md` file when full instructions are needed."
                .to_string(),
        );
        lines.push(String::new());
        for skill in skill_names {
            lines.push(format!("- `{skill}` - `skills/{skill}/SKILL.md`"));
        }
        lines.push(String::new());
    }
    append_source_block(&mut lines, ".dev/project.md", project_content);
    for (label, content) in conventions {
        append_source_block(&mut lines, label, content);
    }
    if !detected_skills_block.is_empty() {
        lines.push(detected_skills_block.trim_end().to_string());
        lines.push(String::new());
    }
    append_source_block(&mut lines, "workflows/coding.md", workflow_content);
    format!("{}\n", lines.join("\n").trim_end())
}

fn append_source_block(lines: &mut Vec<String>, label: &str, content: &str) {
    lines.push(format!("<!-- Source: {label} -->"));
    lines.push(content.trim().to_string());
    lines.push(String::new());
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
            !names.iter().any(|n| n == "csharp.md" || n == "go.md" || n == "typescript.md"),
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
        let _guard = crate::commands::ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
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
        assert_eq!(docs.len(), 1, "expected exactly one personal doc, got {docs:?}");
        assert_eq!(docs[0].0, "personal-conventions/go.md");
        assert!(docs[0].1.contains("Personal Go style"));
    }

    #[test]
    fn personal_source2_off_row_skips_whole_pass() {
        let _guard = crate::commands::ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
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
        let _guard = crate::commands::ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
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
        let _guard = crate::commands::ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
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
            block.contains(&shared_root.join("flutter-style").to_string_lossy().to_string()),
            "must reference the origin path"
        );
        assert!(
            !block.contains("SECRET SKILL BODY"),
            "must never copy skill content into the block"
        );
    }

    #[test]
    fn detected_skills_block_off_row_yields_no_block() {
        let _guard = crate::commands::ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
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
        assert!(block.is_empty(), "off row must yield zero block, got: {block}");
    }

    #[test]
    fn detected_skills_block_no_match_is_byte_identical() {
        let _guard = crate::commands::ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
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
        let _guard = crate::commands::ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
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
        let _guard = crate::commands::ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
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
            "# Project\n\n| Language | Rust |\n",
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
            "# Project\n\n| Language | Rust |\n",
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
            let offset = content.find(marker).unwrap_or_else(|| {
                panic!("AGENTS.md missing critical-pack marker: {marker}")
            });
            assert!(
                offset < 32768,
                "critical-pack marker {marker:?} at byte offset {offset} >= 32768"
            );
        }
    }

    #[test]
    fn render_adapter_document_orders_repo_skills_before_project_content_when_included() {
        let content = render_adapter_document(
            "Test Adapter",
            &["preamble line"],
            "PROJECT_CONTENT_MARKER",
            &[],
            "",
            "WORKFLOW_CONTENT_MARKER",
            &["some-skill".to_string()],
            true,
        );
        let skills_pos = content
            .find("## Repo Skills")
            .expect("Repo Skills section must be present when include_skill_index is true");
        let project_pos = content
            .find("PROJECT_CONTENT_MARKER")
            .expect("project content must still be present");
        assert!(
            skills_pos < project_pos,
            "Repo Skills (byte {skills_pos}) must precede project content (byte {project_pos})"
        );
    }

    #[test]
    fn render_adapter_document_omits_repo_skills_when_index_disabled() {
        let content = render_adapter_document(
            "Test Adapter",
            &["preamble line"],
            "PROJECT_CONTENT_MARKER",
            &[],
            "",
            "WORKFLOW_CONTENT_MARKER",
            &["some-skill".to_string()],
            false,
        );
        assert!(
            !content.contains("## Repo Skills"),
            "Copilot/rules carriers (include_skill_index=false) must omit Repo Skills entirely"
        );
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
            "# Project\n\n| Language | Rust |\n",
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
        let rules = fs::read_to_string(repo_root.join(".agents").join("rules").join("gal.md"))
            .unwrap();
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
            "# Project\n\n| Language | Rust |\n",
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
        assert!(rendered.contains("<!-- Source: .dev/project.md -->"));
        assert!(rendered.contains("<!-- Source: conventions/rust.md -->"));
        assert!(rendered.contains("<!-- Source: workflows/coding.md -->"));
        assert!(report.written_files.contains(&rules_path));

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
}
