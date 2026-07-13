//! `gal commit-msg` — git commit-msg hook + message generator.
//!
//! This is the single Rust-native implementation that replaces the retired
//!
//! No-hijack rule: type, scope, subject, and bullets are derived from changed
//! file PATHS and git STATUS only — never from diff body keywords. A body that
//! happens to contain words like `fix`, `<think>`, or `git-commit-msg` must not
//! change the classification or lock the message onto a canned phrase.
//!
//! Two consumers:
//! - `gal commit-msg --print` → write a generated message to stdout (the
//!   `git-commits` skill / `git-commit-msg` command call this).
//! - `gal commit-msg <file>` → git commit-msg hook. NON-DESTRUCTIVE: an author
//!   message that is already present is preserved untouched; only a blank
//!   message is filled from the staged changes.

use std::path::Path;

// ---------------------------------------------------------------------------
// Staged-entry model + parsing (paths and status only — no diff body is read)
// ---------------------------------------------------------------------------

/// One staged change as reported by `git diff --cached --name-status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedEntry {
    /// Raw git status token, e.g. `M`, `A`, `D`, `R100`.
    pub status: String,
    /// Effective path (for renames, the destination path).
    pub path: String,
    /// First path component, lowercased (e.g. `crates`, `docs`).
    pub top: String,
    /// Lowercased extension including the dot (e.g. `.rs`, `.md`), or empty.
    pub extension: String,
}

/// Parse `git diff --cached --name-status --find-renames` output into entries.
///
/// Rename lines have three tab-separated fields (`R100\told\tnew`); the new path
/// is used. All other lines have two fields (`STATUS\tpath`).
pub fn parse_name_status(output: &str) -> Vec<StagedEntry> {
    let mut entries = Vec::new();
    for line in output.lines() {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let status = parts[0].to_string();
        let path = if status.starts_with('R') && parts.len() >= 3 {
            parts[2].to_string()
        } else {
            parts[parts.len() - 1].to_string()
        };
        let normalized = path.replace('\\', "/");
        let top = normalized
            .trim_start_matches('/')
            .split('/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let extension = match normalized
            .rsplit_once('/')
            .map_or(normalized.as_str(), |(_, f)| f)
            .rsplit_once('.')
        {
            Some((_, ext)) if !ext.is_empty() => format!(".{}", ext.to_ascii_lowercase()),
            _ => String::new(),
        };
        entries.push(StagedEntry {
            status,
            path: normalized,
            top,
            extension,
        });
    }
    entries
}

const DOC_EXTS: [&str; 4] = [".md", ".mdx", ".txt", ".rst"];

fn is_doc(entry: &StagedEntry) -> bool {
    DOC_EXTS.contains(&entry.extension.as_str())
}

fn has_rename(entries: &[StagedEntry]) -> bool {
    entries.iter().any(|e| e.status.starts_with('R'))
}

fn all_docs(entries: &[StagedEntry]) -> bool {
    !entries.is_empty() && entries.iter().all(is_doc)
}

/// True only when every non-doc (code) entry lives under a `test/`/`tests/` dir.
/// A single test file among a larger change must NOT classify the commit as test.
fn all_tests(entries: &[StagedEntry]) -> bool {
    let code: Vec<&StagedEntry> = entries.iter().filter(|e| !is_doc(e)).collect();
    if code.is_empty() {
        return false;
    }
    code.iter().all(|e| path_has_test_dir(&e.path))
}

fn path_has_test_dir(path: &str) -> bool {
    path.split('/').any(|seg| seg == "test" || seg == "tests")
}

fn is_added_feature(entry: &StagedEntry) -> bool {
    if !entry.status.starts_with('A') {
        return false;
    }
    const FEATURE_DIRS: [&str; 5] = ["commands", "agent", "skills", "workflows", "templates"];
    entry.path.starts_with("plugins/gal-core/")
        && FEATURE_DIRS
            .iter()
            .any(|d| entry.path.starts_with(&format!("plugins/gal-core/{d}/")))
        || FEATURE_DIRS.contains(&entry.top.as_str())
}

fn is_config_json(path: &str) -> bool {
    matches!(path, "opencode.json" | "mcp.json")
        || path == "plugins/gal-core/opencode.json"
        || path == "plugins/gal-core/mcp.json"
}

/// Derive the conventional-commit TYPE from paths + status only (no-hijack).
pub fn derive_type(entries: &[StagedEntry]) -> &'static str {
    if all_docs(entries) {
        return "docs";
    }
    if entries
        .iter()
        .any(|e| e.path.starts_with(".github/workflows/") || e.path.starts_with(".github/actions/"))
    {
        return "ci";
    }
    // A rename is structurally a refactor regardless of where the files live, so
    // it takes precedence over the (all-or-nothing) test classification.
    if has_rename(entries) {
        return "refactor";
    }
    if all_tests(entries) {
        return "test";
    }
    if entries.iter().any(is_added_feature) {
        return "feat";
    }
    if entries
        .iter()
        .any(|e| e.top == "scripts" || is_config_json(&e.path))
    {
        return "chore";
    }
    "refactor"
}

fn single_named_under(entries: &[StagedEntry], dir: &str) -> Option<String> {
    let mut names: Vec<String> = Vec::new();
    for e in entries {
        for prefix in [format!("plugins/gal-core/{dir}/"), format!("{dir}/")] {
            if let Some(rest) = e.path.strip_prefix(&prefix) {
                if let Some(name) = rest.split('/').next() {
                    if !name.is_empty() {
                        let lower = name.to_ascii_lowercase();
                        if !names.contains(&lower) {
                            names.push(lower);
                        }
                    }
                }
                break;
            }
        }
    }
    if names.len() == 1 {
        names.pop()
    } else {
        None
    }
}

/// Derive the SCOPE from changed file paths only (no-hijack).
pub fn derive_scope_from_entries(entries: &[StagedEntry]) -> Option<String> {
    if entries.is_empty() {
        return None;
    }
    if entries
        .iter()
        .any(|e| e.path == "plugins/gal-core/opencode.json" || e.path == "opencode.json")
    {
        return Some("opencode".to_string());
    }
    if let Some(cmd) = single_named_under(entries, "commands") {
        return Some(cmd);
    }
    if let Some(skill) = single_named_under(entries, "skills") {
        return Some(skill);
    }

    // Plurality of top-level components.
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for e in entries {
        if !e.top.is_empty() {
            *counts.entry(e.top.as_str()).or_insert(0) += 1;
        }
    }
    let best = counts.into_iter().max_by_key(|(_, c)| *c).map(|(k, _)| k)?;
    Some(match best {
        "agent" => "agents".to_string(),
        other => other.to_string(),
    })
}

/// Derive the SUBJECT from type, scope, and status only (no-hijack).
pub fn derive_subject(commit_type: &str, scope: Option<&str>, entries: &[StagedEntry]) -> String {
    if has_rename(entries) {
        if let Some(s) = scope {
            return format!("rename {s}");
        }
    }
    match (commit_type, scope) {
        ("docs", Some(s)) => format!("document {s}"),
        ("docs", None) => "document repo workflows".to_string(),
        ("feat", Some(s)) => format!("add {s} support"),
        ("feat", None) => "add repo workflow support".to_string(),
        ("fix", Some(s)) => format!("stabilize {s}"),
        ("fix", None) => "stabilize repo workflow".to_string(),
        ("refactor", Some(s)) => format!("refactor {s}"),
        ("refactor", None) => "refactor repo workflow".to_string(),
        ("test", Some(s)) => format!("cover {s}"),
        ("test", None) => "cover repo workflow".to_string(),
        ("ci", Some(s)) => format!("adjust {s}"),
        ("ci", None) => "adjust ci workflow".to_string(),
        (_, Some(s)) => format!("update {s}"),
        (_, None) => "update repo workflow".to_string(),
    }
}

/// Derive body bullets from concrete path signals only (no body-keyword filler).
pub fn derive_bullets(entries: &[StagedEntry]) -> Vec<String> {
    let mut bullets = Vec::new();
    if entries.iter().any(|e| {
        e.path
            .starts_with("plugins/gal-core/commands/git-commit-msg/")
            || e.path.starts_with("commands/git-commit-msg/")
    }) {
        bullets.push("add a source-of-truth git-commit-msg command under commands/".to_string());
    }
    if entries
        .iter()
        .any(|e| e.path == "plugins/gal-core/opencode.json" || e.path == "opencode.json")
    {
        bullets.push(
            "route the repo OpenCode git-commit-msg command through staged helper output"
                .to_string(),
        );
    }
    bullets.truncate(3);
    bullets
}

/// Build the full commit message from staged entries, or `None` if nothing is
/// staged. Bullets are only appended for broader changes (>3 files).
pub fn generate_commit_message(entries: &[StagedEntry]) -> Option<String> {
    if entries.is_empty() {
        return None;
    }
    let commit_type = derive_type(entries);
    let scope = derive_scope_from_entries(entries);
    let subject = derive_subject(commit_type, scope.as_deref(), entries);

    let header = match &scope {
        Some(s) => format!("{commit_type}({s}): {subject}"),
        None => format!("{commit_type}: {subject}"),
    };

    let unique_files: std::collections::HashSet<&str> =
        entries.iter().map(|e| e.path.as_str()).collect();
    let bullets = derive_bullets(entries);
    if unique_files.len() > 3 && !bullets.is_empty() {
        let body = bullets
            .iter()
            .map(|b| format!("- {b}"))
            .collect::<Vec<_>>()
            .join("\n");
        Some(format!("{header}\n\n{body}"))
    } else {
        Some(header)
    }
}

/// True when the commit message file holds no author content (only blank lines
/// and `#` comment lines). Such a message is safe to fill from staged changes.
pub fn message_is_blank(raw: &str) -> bool {
    raw.lines()
        .map(str::trim)
        .all(|l| l.is_empty() || l.starts_with('#'))
}

/// Result of a commit-msg run.
#[derive(Debug, PartialEq, Eq)]
pub enum CommitMsgResult {
    /// Message may have been updated and written back.
    Updated,
    /// No staged files — no-op (empty staging → no-op).
    NoOp,
}

/// Derive the conventional-commit scope from a list of staged file paths.
///
/// Rules (no-hijack — paths only, never body keywords):
/// 1. If all changed files share the same top-level component, use it.
/// 2. Common mappings: `crates/gal-engine` → `gal-engine`, `crates/gal-cli` → `gal-cli`,
///    `scripts/` → `scripts`, `docs/` → `docs`, `agent/` → `agent`,
///    `skills/` → `skills`, `commands/` → `commands`.
/// 3. If mixed top-levels, pick the plurality component.
/// 4. Returns `None` if no clear scope can be derived.
#[cfg(test)]
pub fn derive_scope(staged_files: &[&str]) -> Option<String> {
    if staged_files.is_empty() {
        return None;
    }

    let components: Vec<String> = staged_files
        .iter()
        .filter_map(|f| first_component(f))
        .collect();

    if components.is_empty() {
        return None;
    }

    // Count occurrences of each component.
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for c in &components {
        *counts.entry(c.as_str()).or_insert(0) += 1;
    }

    // Pick the most common component.
    let best = counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(k, _)| k)?;

    // Map top-level directory to scope token.
    Some(map_to_scope(best).to_string())
}

/// Extract the first path component from a file path string.
#[cfg(test)]
fn first_component(path: &str) -> Option<String> {
    // Normalise separators
    let p = path.replace('\\', "/");
    let trimmed = p.trim_start_matches('/');
    let part = trimmed.split('/').next()?;
    if part.is_empty() {
        None
    } else {
        Some(part.to_string())
    }
}

/// Map a top-level directory to a conventional-commit scope token.
#[cfg(test)]
fn map_to_scope(component: &str) -> &str {
    match component {
        // crates sub-paths are handled at the caller (first_component returns "crates").
        // We unwrap one more level for Cargo crates by reprocessing below.
        "crates" => "crates",
        "scripts" => "scripts",
        "docs" => "docs",
        "agent" => "agent",
        "skills" => "skills",
        "commands" => "commands",
        ".dev" => "state",
        "packaging" => "packaging",
        other => other,
    }
}

/// For paths under `crates/`, extract the crate name as the scope.
#[cfg(test)]
pub fn derive_scope_from_files(staged_files: &[&str]) -> Option<String> {
    if staged_files.is_empty() {
        return None;
    }

    // Special handling: if all files are under crates/<crate-name>/, use that crate.
    let crate_names: Vec<Option<String>> =
        staged_files.iter().map(|f| extract_crate_name(f)).collect();

    let all_same_crate = crate_names.iter().all(|c| c == &crate_names[0]);
    if all_same_crate {
        if let Some(Some(crate_name)) = crate_names.first() {
            return Some(crate_name.clone());
        }
    }

    // Fall back to top-level component scope.
    derive_scope(staged_files)
}

/// Extract crate name from a path like `crates/gal-engine/src/lib.rs` → `gal-engine`.
#[cfg(test)]
fn extract_crate_name(path: &str) -> Option<String> {
    let p = path.replace('\\', "/");
    let mut parts = p.trim_start_matches('/').splitn(3, '/');
    let first = parts.next()?;
    if first == "crates" {
        parts.next().map(|s| s.to_string())
    } else {
        None
    }
}

/// Process a commit message file given staged file paths.
///
/// - If `staged_files` is empty → `CommitMsgResult::NoOp` (do not touch the file).
/// - Otherwise, reads the message from `msg_path`, derives scope from file paths,
///   writes the (possibly updated) message back, and returns `CommitMsgResult::Updated`.
///
/// No-hijack guarantee: the scope is derived solely from `staged_files` paths.
/// The message body is never parsed for keywords to determine scope.
#[cfg(test)]
pub fn process_commit_msg(
    msg_path: &Path,
    staged_files: &[&str],
) -> std::io::Result<CommitMsgResult> {
    // empty staging → no-op.
    if staged_files.is_empty() {
        return Ok(CommitMsgResult::NoOp);
    }

    let current_msg = std::fs::read_to_string(msg_path)?;

    // Derive scope from file paths (no body keyword inspection — no-hijack).
    let scope = derive_scope_from_files(staged_files);

    let updated = inject_scope_prefix(&current_msg, scope.as_deref()).unwrap_or(current_msg);
    std::fs::write(msg_path, updated)?;

    Ok(CommitMsgResult::Updated)
}

fn inject_scope_prefix(message: &str, scope: Option<&str>) -> Option<String> {
    let scope = scope?;
    let first_line = message.lines().next()?;
    let trimmed = first_line.trim();
    let colon = trimmed.find(':')?;
    let prefix = &trimmed[..colon];

    if prefix.contains('(') || prefix.contains(')') || prefix.is_empty() {
        return None;
    }

    let allowed_types = [
        "feat", "fix", "refactor", "docs", "test", "chore", "perf", "ci",
    ];
    if !allowed_types.contains(&prefix) {
        return None;
    }

    let scoped_first = first_line.replacen(prefix, &format!("{prefix}({scope})"), 1);
    if let Some(rest) = message.strip_prefix(first_line) {
        Some(format!("{scoped_first}{rest}"))
    } else {
        Some(scoped_first)
    }
}

/// Non-destructive git commit-msg hook behavior.
///
/// - Empty staging → `NoOp` (do not touch the file).
/// - Author already wrote a message (non-blank) → preserve it untouched. This is
///   the fix for the old `> $1` clobber bug: the hook never overwrites a real
///   message.
/// - Author message is blank → fill it from the generated message.
pub fn fill_commit_msg_file(
    msg_path: &Path,
    entries: &[StagedEntry],
) -> std::io::Result<CommitMsgResult> {
    if entries.is_empty() {
        return Ok(CommitMsgResult::NoOp);
    }

    let current = std::fs::read_to_string(msg_path)?;
    if !message_is_blank(&current) {
        // Author wrote a message: never clobber its wording. A conventional
        // header that merely lacks a scope can still be enriched in place
        // (no-hijack holds: scope derives only from staged paths, and freeform
        // or already-scoped headers are left untouched by inject_scope_prefix).
        let scope = derive_scope_from_entries(entries);
        if let Some(updated) = inject_scope_prefix(&current, scope.as_deref()) {
            if updated != current {
                std::fs::write(msg_path, updated)?;
                return Ok(CommitMsgResult::Updated);
            }
        }
        return Ok(CommitMsgResult::NoOp);
    }

    match generate_commit_message(entries) {
        Some(generated) => {
            // Keep any trailing comment block git appended below the blank message.
            let comments: String = current
                .lines()
                .filter(|l| l.trim_start().starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n");
            let out = if comments.is_empty() {
                format!("{generated}\n")
            } else {
                format!("{generated}\n\n{comments}\n")
            };
            std::fs::write(msg_path, out)?;
            Ok(CommitMsgResult::Updated)
        }
        None => Ok(CommitMsgResult::NoOp),
    }
}

// ---------------------------------------------------------------------------
// Plan/prompt intent extraction (used by --context assembly)
// ---------------------------------------------------------------------------

/// `# Plan:` title and `## Goal` summary extracted from a plan/prompt file's
/// staged content. Empty fields mean the corresponding heading was absent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanIntent {
    pub title: String,
    pub goal: String,
}

/// Goal truncation caps: first 12 lines or ~500 chars, whichever comes first.
const GOAL_MAX_LINES: usize = 12;
const GOAL_MAX_CHARS: usize = 500;

/// Extract the `# Plan:` title and `## Goal` section from plan/prompt content.
///
/// Missing `# Plan:` or `## Goal` yields an empty field, never a panic. The
/// goal is truncated on a line boundary (first 12 lines) then a char boundary
/// (first ~500 chars), with an ellipsis appended — `.chars()` is used so a
/// truncation point can never land inside a multi-byte UTF-8 sequence.
pub fn extract_plan_intent(staged_content: &str) -> PlanIntent {
    let title = staged_content
        .lines()
        .find_map(|l| l.strip_prefix("# Plan:"))
        .map(|t| t.trim().to_string())
        .unwrap_or_default();

    let goal = extract_section_body(staged_content, "## Goal")
        .map(|body| truncate_goal(&body))
        .unwrap_or_default();

    PlanIntent { title, goal }
}

/// Return the trimmed body of a `## <heading>` section, up to the next `## `
/// heading or end of content. `None` if the heading is not present.
fn extract_section_body(content: &str, heading: &str) -> Option<String> {
    let start = content.find(heading)?;
    let after_heading = &content[start + heading.len()..];
    let body_start = after_heading
        .find('\n')
        .map(|i| i + 1)
        .unwrap_or(after_heading.len());
    let body = &after_heading[body_start..];
    let end = body.find("\n## ").unwrap_or(body.len());
    Some(body[..end].trim().to_string())
}

/// Truncate `body` to `GOAL_MAX_LINES` lines then `GOAL_MAX_CHARS` chars
/// (char-boundary safe via `.chars()`), appending an ellipsis when cut.
fn truncate_goal(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let within_lines = lines.len() <= GOAL_MAX_LINES;
    let within_chars = body.chars().count() <= GOAL_MAX_CHARS;
    if within_lines && within_chars {
        return body.to_string();
    }

    let line_capped = if within_lines {
        body.to_string()
    } else {
        lines[..GOAL_MAX_LINES].join("\n")
    };

    if line_capped.chars().count() <= GOAL_MAX_CHARS {
        return format!("{line_capped}…");
    }

    let char_capped: String = line_capped.chars().take(GOAL_MAX_CHARS).collect();
    format!("{char_capped}…")
}

// ---------------------------------------------------------------------------
// --context assembly (classification lives here — callers never classify)
// ---------------------------------------------------------------------------

/// Plan-slug verb prefixes, per the plan filename convention
/// (`conventions/task-atomicity.md` / `workflows/coding.md` plan lifecycle).
const PLAN_VERBS: [&str; 7] = ["feat", "fix", "refactor", "sec", "perf", "infra", "release"];

/// How a staged path classifies for `--context` assembly. Classification is
/// path-only (no-hijack: never inspects diff/body content) and lives solely
/// in this module — callers pass `&[StagedEntry]` and never re-derive it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StagedKind {
    /// `.dev/plans/<slug>.md` — a source plan.
    SourcePlan { slug: String },
    /// `.dev/plans/<slug>.prompt.md` — an execution prompt.
    Prompt { slug: String },
    /// `.dev/plans/<slug>.en.md` or `.dev/plans/<slug>.equiv.md` — a non-semantic
    /// EN draft, or a retired-mechanism equivalence-receipt file. The `.equiv.md`
    /// suffix has no writer since the equivalence proof now lives inline in the
    /// source plan's `gal:planning-authority` block; the branch is kept as a
    /// defensive guard so a legacy/downstream orphan `.equiv.md` still excludes
    /// cleanly instead of misclassifying as a source plan. Excluded entirely
    /// from all blocks.
    Draft,
    /// Anything else — a regular code/doc file, eligible for `CHANGES`.
    Code,
}

fn classify_staged_path(path: &str) -> StagedKind {
    let Some(rest) = path.strip_prefix(".dev/plans/") else {
        return StagedKind::Code;
    };
    if let Some(slug) = rest.strip_suffix(".prompt.md") {
        return StagedKind::Prompt {
            slug: slug.to_string(),
        };
    }
    if rest.ends_with(".en.md") || rest.ends_with(".equiv.md") {
        return StagedKind::Draft;
    }
    if let Some(slug) = rest.strip_suffix(".md") {
        return StagedKind::SourcePlan {
            slug: slug.to_string(),
        };
    }
    StagedKind::Code
}

/// The plan-type verb prefix of a slug (e.g. `feat-foo` → `feat`), or `None`
/// when the slug does not start with a known verb.
fn plan_verb(slug: &str) -> Option<&'static str> {
    PLAN_VERBS
        .iter()
        .copied()
        .find(|v| slug.starts_with(&format!("{v}-")))
}

/// Total Goal budget shared across all plans in one `--context` call. Each
/// individual Goal is already capped by `extract_plan_intent`; this further
/// clamps per-plan share when more than one plan is staged at once.
const TOTAL_GOAL_BUDGET_CHARS: usize = 1500;

fn clamp_total_goal_budget(goal: &str, plans_count: usize) -> String {
    if plans_count <= 1 || goal.is_empty() {
        return goal.to_string();
    }
    let per_plan = TOTAL_GOAL_BUDGET_CHARS / plans_count;
    if goal.chars().count() <= per_plan {
        return goal.to_string();
    }
    let clamped: String = goal.chars().take(per_plan).collect();
    format!("{clamped}…")
}

struct ClassifiedPlan<'a> {
    entry: &'a StagedEntry,
    slug: String,
}

/// Assemble the 5-block `--context` output (FILES/BASELINE/PLANS/PROMPTS/CHANGES)
/// from staged entries. Classification (source plan / prompt / draft-ignored /
/// code) is this function's sole responsibility — callers pass raw entries and
/// two IO closures and never re-derive path classification themselves.
///
/// `resolve_staged_content(path)` should return the staged (`git show :<path>`)
/// content of a plan/prompt path, or `None` on any read failure.
/// `resolve_hunk_headers(code_paths)` should return a `@@`-style hunk-header
/// summary for the given non-plan code paths.
///
/// Empty blocks are omitted; empty `entries` returns a one-line friendly note.
pub fn assemble_commit_context(
    entries: &[StagedEntry],
    resolve_staged_content: impl Fn(&str) -> Option<String>,
    resolve_hunk_headers: impl Fn(&[String]) -> String,
) -> String {
    if entries.is_empty() {
        return "No staged changes.".to_string();
    }

    let files_block = entries
        .iter()
        .map(|e| format!("{}\t{}", e.status, e.path))
        .collect::<Vec<_>>()
        .join("\n");

    let baseline = generate_commit_message(entries);

    let mut source_plans: Vec<ClassifiedPlan> = Vec::new();
    let mut prompts: Vec<ClassifiedPlan> = Vec::new();
    let mut code_paths: Vec<String> = Vec::new();

    for e in entries {
        match classify_staged_path(&e.path) {
            StagedKind::SourcePlan { slug } => source_plans.push(ClassifiedPlan { entry: e, slug }),
            StagedKind::Prompt { slug } => prompts.push(ClassifiedPlan { entry: e, slug }),
            StagedKind::Draft => {}
            StagedKind::Code => code_paths.push(e.path.clone()),
        }
    }

    let plans_block = if source_plans.is_empty() {
        None
    } else {
        let count = source_plans.len();
        let lines: Vec<String> = source_plans
            .iter()
            .map(|p| {
                let verb = plan_verb(&p.slug).unwrap_or("plan");
                // [deleted] is decided by git status only — the resolve closure
                // is never called for a deleted entry (no-hijack + it cannot
                // read staged content for a path that no longer exists).
                if p.entry.status.starts_with('D') {
                    return format!("- {} ({verb}) [deleted]", p.slug);
                }
                let intent = resolve_staged_content(&p.entry.path)
                    .map(|content| extract_plan_intent(&content))
                    .unwrap_or_default();
                let title = if intent.title.is_empty() {
                    p.slug.clone()
                } else {
                    intent.title.clone()
                };
                let goal = clamp_total_goal_budget(&intent.goal, count);
                if goal.is_empty() {
                    format!("- {} ({verb}): {title}", p.slug)
                } else {
                    format!("- {} ({verb}): {title}\n  Goal: {goal}", p.slug)
                }
            })
            .collect();
        Some(lines.join("\n"))
    };

    let prompts_block = if prompts.is_empty() {
        None
    } else {
        let slugs: Vec<&str> = prompts.iter().map(|p| p.slug.as_str()).collect();
        Some(format!("{} prompt(s): {}", slugs.len(), slugs.join(", ")))
    };

    let changes_block = if code_paths.is_empty() {
        None
    } else {
        let headers = resolve_hunk_headers(&code_paths);
        if headers.trim().is_empty() {
            None
        } else {
            Some(headers)
        }
    };

    let mut sections: Vec<String> = vec![format!("FILES:\n{files_block}")];
    if let Some(b) = baseline {
        sections.push(format!("BASELINE:\n{b}"));
    }
    if let Some(p) = plans_block {
        sections.push(format!("PLANS:\n{p}"));
    }
    if let Some(p) = prompts_block {
        sections.push(format!("PROMPTS:\n{p}"));
    }
    if let Some(c) = changes_block {
        sections.push(format!("CHANGES:\n{c}"));
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn parse(lines: &str) -> Vec<StagedEntry> {
        parse_name_status(lines)
    }

    #[test]
    fn rename_is_refactor_not_test_even_under_tests_dir() {
        // The exact failure the user hit: a crate rename whose files include a
        // tests/ path must classify as refactor, not test.
        let e = parse(
            "R100\tcrates/gal-core/src/lib.rs\tcrates/gal-engine/src/lib.rs\n\
             R098\tcrates/gal-core/tests/parity.rs\tcrates/gal-engine/tests/parity.rs",
        );
        assert_eq!(derive_type(&e), "refactor");
        let scope = derive_scope_from_entries(&e);
        assert_eq!(
            derive_subject("refactor", scope.as_deref(), &e),
            "rename crates"
        );
    }

    #[test]
    fn single_test_among_code_is_not_test_type() {
        let e = parse(
            "M\tcrates/gal-cli/src/main.rs\n\
             A\tcrates/gal-cli/tests/t.rs",
        );
        assert_eq!(derive_type(&e), "refactor");
    }

    #[test]
    fn all_tests_is_test_type() {
        let e = parse("A\tcrates/gal-engine/tests/probe.rs");
        assert_eq!(derive_type(&e), "test");
    }

    #[test]
    fn docs_body_keywords_do_not_hijack_subject() {
        // A doc whose CONTENT mentions fix/<think>/git-commit-msg must not produce
        // any canned phrase — we never read the body.
        let e = parse("M\tdocs/notes.md");
        assert_eq!(derive_type(&e), "docs");
        let scope = derive_scope_from_entries(&e);
        assert_eq!(
            derive_subject("docs", scope.as_deref(), &e),
            "document docs"
        );
        assert_eq!(
            generate_commit_message(&e).unwrap(),
            "docs(docs): document docs"
        );
    }

    #[test]
    fn script_change_is_chore_not_canned_git_commit_phrase() {
        let e = parse("M\tscripts/whatever.sh");
        assert_eq!(
            generate_commit_message(&e).unwrap(),
            "chore(scripts): update scripts"
        );
    }

    #[test]
    fn empty_staging_generates_none() {
        assert_eq!(generate_commit_message(&[]), None);
    }

    #[test]
    fn fill_preserves_existing_author_message() {
        let tmp = TempDir::new().unwrap();
        let msg_path = tmp.path().join("COMMIT_EDITMSG");
        let original = "feat(x): a real message the author wrote\n";
        std::fs::write(&msg_path, original).unwrap();

        let e = parse("M\tscripts/whatever.sh");
        let result = fill_commit_msg_file(&msg_path, &e).unwrap();
        assert_eq!(result, CommitMsgResult::NoOp);
        assert_eq!(std::fs::read_to_string(&msg_path).unwrap(), original);
    }

    #[test]
    fn fill_populates_blank_message_from_staged() {
        let tmp = TempDir::new().unwrap();
        let msg_path = tmp.path().join("COMMIT_EDITMSG");
        std::fs::write(&msg_path, "\n# Please enter the commit message\n").unwrap();

        let e = parse("M\tscripts/whatever.sh");
        let result = fill_commit_msg_file(&msg_path, &e).unwrap();
        assert_eq!(result, CommitMsgResult::Updated);
        let written = std::fs::read_to_string(&msg_path).unwrap();
        assert!(written.starts_with("chore(scripts): update scripts"));
        // Comment block preserved.
        assert!(written.contains("# Please enter the commit message"));
    }

    #[test]
    fn message_is_blank_detects_comment_only() {
        assert!(message_is_blank("\n#comment\n   \n"));
        assert!(!message_is_blank("real text"));
    }

    // body contains gemini+agy but no rename → no hijack
    #[test]
    fn scope_from_paths_not_from_body_keywords() {
        let files = vec!["docs/manual.md", "docs/devguide.md"];
        let scope = derive_scope_from_files(&files);
        // Scope should be "docs" from paths, not "antigravity" from any keyword inspection.
        assert_eq!(scope, Some("docs".to_string()));
        // The message body having "gemini" or "agy" is irrelevant — we don't look at it.
    }

    // agy body no hijack (crates files → crate scope, not antigravity)
    #[test]
    fn no_hijack_when_body_has_agy_keywords_but_files_in_crates() {
        let files = vec![
            "crates/gal-engine/src/install.rs",
            "crates/gal-engine/src/ledger.rs",
        ];
        let scope = derive_scope_from_files(&files);
        assert_eq!(scope, Some("gal-engine".to_string()));
        // Even if a commit message body said "fix agy gemini thing", scope stays gal-engine.
    }

    // empty staging → no-op
    #[test]
    fn empty_staged_files_returns_noop() {
        let tmp = TempDir::new().unwrap();
        let msg_path = tmp.path().join("COMMIT_EDITMSG");
        std::fs::write(&msg_path, "fix: something").unwrap();

        let result = process_commit_msg(&msg_path, &[]).unwrap();
        assert_eq!(result, CommitMsgResult::NoOp);

        // File should be unchanged.
        let content = std::fs::read_to_string(&msg_path).unwrap();
        assert_eq!(content, "fix: something");
    }

    #[test]
    fn derive_scope_from_docs_files() {
        let scope = derive_scope_from_files(&["docs/manual.md"]);
        assert_eq!(scope, Some("docs".to_string()));
    }

    #[test]
    fn derive_scope_from_scripts_files() {
        let scope = derive_scope_from_files(&["scripts/gal.ps1", "scripts/common.sh"]);
        assert_eq!(scope, Some("scripts".to_string()));
    }

    #[test]
    fn derive_scope_from_single_crate() {
        let scope =
            derive_scope_from_files(&["crates/gal-cli/src/main.rs", "crates/gal-cli/Cargo.toml"]);
        assert_eq!(scope, Some("gal-cli".to_string()));
    }

    #[test]
    fn derive_scope_from_mixed_crates_falls_back_to_crates() {
        let scope = derive_scope_from_files(&[
            "crates/gal-engine/src/lib.rs",
            "crates/gal-cli/src/main.rs",
        ]);
        // Mixed crates → top-level fallback returns "crates"
        assert_eq!(scope, Some("crates".to_string()));
    }

    #[test]
    fn derive_scope_none_for_empty_input() {
        assert_eq!(derive_scope(&[]), None);
        assert_eq!(derive_scope_from_files(&[]), None);
    }

    #[test]
    fn process_commit_msg_preserves_message() {
        let tmp = TempDir::new().unwrap();
        let msg_path = tmp.path().join("COMMIT_EDITMSG");
        let original = "feat(gal-engine): add ledger module\n\nFixes the tracking gap.";
        std::fs::write(&msg_path, original).unwrap();

        let files = vec!["crates/gal-engine/src/ledger.rs"];
        let result = process_commit_msg(&msg_path, &files).unwrap();

        assert_eq!(result, CommitMsgResult::Updated);
        let content = std::fs::read_to_string(&msg_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn process_commit_msg_injects_scope_when_header_lacks_one() {
        let tmp = TempDir::new().unwrap();
        let msg_path = tmp.path().join("COMMIT_EDITMSG");
        std::fs::write(
            &msg_path,
            "fix: stabilize uninstall flow\n\nBody stays untouched.\n",
        )
        .unwrap();

        let files = vec!["crates/gal-engine/src/install.rs"];
        let result = process_commit_msg(&msg_path, &files).unwrap();

        assert_eq!(result, CommitMsgResult::Updated);
        let content = std::fs::read_to_string(&msg_path).unwrap();
        assert_eq!(
            content,
            "fix(gal-engine): stabilize uninstall flow\n\nBody stays untouched.\n"
        );
    }

    #[test]
    fn process_commit_msg_keeps_freeform_message_without_conventional_prefix() {
        let tmp = TempDir::new().unwrap();
        let msg_path = tmp.path().join("COMMIT_EDITMSG");
        let original = "stabilize uninstall flow manually\n";
        std::fs::write(&msg_path, original).unwrap();

        let files = vec!["crates/gal-engine/src/install.rs"];
        let result = process_commit_msg(&msg_path, &files).unwrap();

        assert_eq!(result, CommitMsgResult::Updated);
        let content = std::fs::read_to_string(&msg_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn fill_commit_msg_file_injects_scope_into_authored_unscoped_header() {
        // The live hook path (cmd_commit_msg → fill_commit_msg_file): an author's
        // unscoped conventional header must be enriched with the path-derived scope.
        // Scope source is the same derive_scope_from_entries the generator uses, so
        // injection and blank-fill agree (top-level "scripts" bucket here).
        let tmp = TempDir::new().unwrap();
        let msg_path = tmp.path().join("COMMIT_EDITMSG");
        std::fs::write(&msg_path, "fix: stabilize uninstall flow\n").unwrap();

        let entries = parse("M\tscripts/install.sh");
        let result = fill_commit_msg_file(&msg_path, &entries).unwrap();

        assert_eq!(result, CommitMsgResult::Updated);
        let content = std::fs::read_to_string(&msg_path).unwrap();
        assert_eq!(content, "fix(scripts): stabilize uninstall flow\n");
    }

    #[test]
    fn fill_commit_msg_file_leaves_already_scoped_header_untouched() {
        let tmp = TempDir::new().unwrap();
        let msg_path = tmp.path().join("COMMIT_EDITMSG");
        let original = "fix(install): stabilize uninstall flow\n";
        std::fs::write(&msg_path, original).unwrap();

        let entries = parse("M\tcrates/gal-engine/src/install.rs");
        let result = fill_commit_msg_file(&msg_path, &entries).unwrap();

        assert_eq!(result, CommitMsgResult::NoOp);
        let content = std::fs::read_to_string(&msg_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn fill_commit_msg_file_leaves_freeform_authored_message_untouched() {
        let tmp = TempDir::new().unwrap();
        let msg_path = tmp.path().join("COMMIT_EDITMSG");
        let original = "stabilize uninstall flow manually\n";
        std::fs::write(&msg_path, original).unwrap();

        let entries = parse("M\tcrates/gal-engine/src/install.rs");
        let result = fill_commit_msg_file(&msg_path, &entries).unwrap();

        assert_eq!(result, CommitMsgResult::NoOp);
        let content = std::fs::read_to_string(&msg_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn extract_crate_name_from_path() {
        assert_eq!(
            extract_crate_name("crates/gal-engine/src/lib.rs"),
            Some("gal-engine".to_string())
        );
        assert_eq!(extract_crate_name("docs/manual.md"), None);
    }

    #[test]
    fn first_component_handles_slash_variants() {
        assert_eq!(first_component("docs/manual.md"), Some("docs".to_string()));
        assert_eq!(first_component("/docs/manual.md"), Some("docs".to_string()));
        assert_eq!(first_component(""), None);
    }

    #[test]
    fn extract_plan_intent_reads_title_and_goal() {
        let content = "# Plan: Foo Bar\n\nsome preamble\n\n## Goal\nShip the foo.\n\n## Requirements\nR1\n";
        let intent = extract_plan_intent(content);
        assert_eq!(intent.title, "Foo Bar");
        assert_eq!(intent.goal, "Ship the foo.");
    }

    #[test]
    fn extract_plan_intent_missing_sections_yields_empty_no_panic() {
        let intent = extract_plan_intent("no headings here at all");
        assert_eq!(intent.title, "");
        assert_eq!(intent.goal, "");
    }

    #[test]
    fn extract_plan_intent_truncates_long_goal_with_ellipsis() {
        let long_goal = (0..20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let content = format!("# Plan: Long\n\n## Goal\n{long_goal}\n\n## Requirements\n");
        let intent = extract_plan_intent(&content);
        assert!(intent.goal.ends_with('…'));
        assert!(intent.goal.lines().count() <= GOAL_MAX_LINES + 1); // +1 for the ellipsis-bearing last line
    }

    #[test]
    fn extract_plan_intent_zh_tw_goal_truncation_does_not_panic_on_char_boundary() {
        // A Traditional Chinese goal long enough that a naive byte-index cap at
        // GOAL_MAX_CHARS would land mid-UTF-8-sequence if bytes were sliced
        // directly instead of iterating `.chars()`.
        let zh_goal: String = "測試中文目標截斷不應該在多位元組字元中間切斷"
            .chars()
            .cycle()
            .take(600)
            .collect();
        let content = format!("# Plan: 中文計畫\n\n## Goal\n{zh_goal}\n\n## Requirements\n");
        let intent = extract_plan_intent(&content); // must not panic
        assert!(intent.goal.ends_with('…'));
        assert!(intent.goal.chars().count() <= GOAL_MAX_CHARS + 1);
    }

    fn no_headers(_paths: &[String]) -> String {
        String::new()
    }

    #[test]
    fn assemble_commit_context_empty_staging_is_friendly_note() {
        let out = assemble_commit_context(&[], |_| None, no_headers);
        assert_eq!(out, "No staged changes.");
    }

    #[test]
    fn assemble_commit_context_pure_plan_emits_plans_block() {
        let entries = parse("A\t.dev/plans/feat-foo.md");
        let out = assemble_commit_context(
            &entries,
            |_| Some("# Plan: Foo\n\n## Goal\nShip the foo.\n".to_string()),
            no_headers,
        );
        assert!(out.contains("PLANS:"));
        assert!(out.contains("feat-foo (feat): Foo"));
        assert!(out.contains("Goal: Ship the foo."));
        assert!(!out.contains("PROMPTS:"));
        assert!(!out.contains("CHANGES:"));
    }

    #[test]
    fn assemble_commit_context_plan_and_prompt_emits_prompts_count() {
        let entries = parse(
            "A\t.dev/plans/feat-foo.md\nA\t.dev/plans/feat-foo.prompt.md",
        );
        let out = assemble_commit_context(&entries, |_| None, no_headers);
        assert!(out.contains("PROMPTS:"));
        assert!(out.contains("1 prompt(s): feat-foo"));
    }

    #[test]
    fn assemble_commit_context_pure_code_emits_changes_block() {
        let entries = parse("M\tcrates/cli/src/gal/commit_msg.rs");
        let out = assemble_commit_context(&entries, |_| None, |paths| {
            assert_eq!(paths, &["crates/cli/src/gal/commit_msg.rs".to_string()]);
            "@@ -1,2 +1,2 @@".to_string()
        });
        assert!(out.contains("CHANGES:"));
        assert!(out.contains("@@ -1,2 +1,2 @@"));
        assert!(!out.contains("PLANS:"));
        assert!(!out.contains("PROMPTS:"));
    }

    #[test]
    fn assemble_commit_context_mixed_has_all_blocks_no_empties() {
        let entries = parse(
            "A\t.dev/plans/feat-foo.md\nM\tcrates/cli/src/gal/commit_msg.rs",
        );
        let out = assemble_commit_context(
            &entries,
            |_| Some("# Plan: Foo\n\n## Goal\nShip it.\n".to_string()),
            |_| "@@ -1 +1 @@".to_string(),
        );
        assert!(out.contains("FILES:"));
        assert!(out.contains("BASELINE:"));
        assert!(out.contains("PLANS:"));
        assert!(out.contains("CHANGES:"));
        assert!(!out.contains("PROMPTS:"));
    }

    #[test]
    fn assemble_commit_context_deleted_plan_labels_deleted_never_calls_closure() {
        let entries = parse("D\t.dev/plans/feat-foo.md");
        let out = assemble_commit_context(
            &entries,
            |_| panic!("resolve_staged_content must not be called for a deleted entry"),
            no_headers,
        );
        assert!(out.contains("[deleted]"));
        assert!(!out.contains("Goal:"));
    }

    #[test]
    fn assemble_commit_context_closure_none_on_non_deleted_omits_goal_not_deleted() {
        let entries = parse("A\t.dev/plans/feat-foo.md");
        let out = assemble_commit_context(&entries, |_| None, no_headers);
        assert!(out.contains("feat-foo"));
        assert!(!out.contains("[deleted]"));
        assert!(!out.contains("Goal:"));
    }

    #[test]
    fn assemble_commit_context_renamed_plan_reads_goal_from_new_path() {
        let entries = parse("R100\t.dev/plans/feat-old.md\t.dev/plans/feat-new.md");
        let out = assemble_commit_context(
            &entries,
            |path| {
                assert_eq!(path, ".dev/plans/feat-new.md");
                Some("# Plan: New\n\n## Goal\nRenamed goal.\n".to_string())
            },
            no_headers,
        );
        assert!(out.contains("feat-new"));
        assert!(out.contains("Renamed goal."));
    }

    #[test]
    fn assemble_commit_context_excludes_draft_and_receipt_from_all_blocks() {
        let entries = parse(
            "A\t.dev/plans/feat-foo.en.md\nA\t.dev/plans/feat-foo.equiv.md",
        );
        let out = assemble_commit_context(&entries, |_| None, no_headers);
        assert!(!out.contains("PLANS:"));
        assert!(!out.contains("PROMPTS:"));
        assert!(!out.contains("CHANGES:"));
    }
}
