//! `gal commit-msg` — git commit-msg hook + message generator.
//!
//! This is the single Rust-native implementation that replaces the retired
//! `Get-StagedCommitMessage.ps1` / `get-staged-commit-message.sh` helpers.
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
        let extension = match normalized.rsplit_once('/').map_or(normalized.as_str(), |(_, f)| f).rsplit_once('.') {
            Some((_, ext)) if !ext.is_empty() => format!(".{}", ext.to_ascii_lowercase()),
            _ => String::new(),
        };
        entries.push(StagedEntry { status, path: normalized, top, extension });
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
        && FEATURE_DIRS.iter().any(|d| entry.path.starts_with(&format!("plugins/gal-core/{d}/")))
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
    if entries.iter().any(|e| e.path.starts_with(".github/workflows/") || e.path.starts_with(".github/actions/")) {
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
    if entries.iter().any(|e| e.top == "scripts" || is_config_json(&e.path)) {
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
    if entries.iter().any(|e| e.path == "plugins/gal-core/opencode.json" || e.path == "opencode.json") {
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
        e.path.starts_with("plugins/gal-core/commands/git-commit-msg/") || e.path.starts_with("commands/git-commit-msg/")
    }) {
        bullets.push("add a source-of-truth git-commit-msg command under commands/".to_string());
    }
    if entries.iter().any(|e| e.path == "plugins/gal-core/opencode.json" || e.path == "opencode.json") {
        bullets.push("route the repo OpenCode git-commit-msg command through staged helper output".to_string());
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

    let unique_files: std::collections::HashSet<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    let bullets = derive_bullets(entries);
    if unique_files.len() > 3 && !bullets.is_empty() {
        let body = bullets.iter().map(|b| format!("- {b}")).collect::<Vec<_>>().join("\n");
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
    /// No staged files — no-op (TP-019: empty staging → no-op).
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
pub fn derive_scope_from_files(staged_files: &[&str]) -> Option<String> {
    if staged_files.is_empty() {
        return None;
    }

    // Special handling: if all files are under crates/<crate-name>/, use that crate.
    let crate_names: Vec<Option<String>> = staged_files
        .iter()
        .map(|f| extract_crate_name(f))
        .collect();

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
pub fn process_commit_msg(
    msg_path: &Path,
    staged_files: &[&str],
) -> std::io::Result<CommitMsgResult> {
    // TP-019: empty staging → no-op.
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

    let allowed_types = ["feat", "fix", "refactor", "docs", "test", "chore", "perf", "ci"];
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
        // Preserve the author's message verbatim — never clobber.
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
        assert_eq!(derive_subject("refactor", scope.as_deref(), &e), "rename crates");
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
        assert_eq!(derive_subject("docs", scope.as_deref(), &e), "document docs");
        assert_eq!(generate_commit_message(&e).unwrap(), "docs(docs): document docs");
    }

    #[test]
    fn script_change_is_chore_not_canned_git_commit_phrase() {
        let e = parse("M\tscripts/whatever.sh");
        assert_eq!(generate_commit_message(&e).unwrap(), "chore(scripts): update scripts");
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

    // TP-019: body contains gemini+agy but no rename → no hijack
    #[test]
    fn scope_from_paths_not_from_body_keywords() {
        let files = vec!["docs/manual.md", "docs/devguide.md"];
        let scope = derive_scope_from_files(&files);
        // Scope should be "docs" from paths, not "antigravity" from any keyword inspection.
        assert_eq!(scope, Some("docs".to_string()));
        // The message body having "gemini" or "agy" is irrelevant — we don't look at it.
    }

    // TP-019: agy body no hijack (crates files → crate scope, not antigravity)
    #[test]
    fn no_hijack_when_body_has_agy_keywords_but_files_in_crates() {
        let files = vec!["crates/gal-engine/src/install.rs", "crates/gal-engine/src/ledger.rs"];
        let scope = derive_scope_from_files(&files);
        assert_eq!(scope, Some("gal-engine".to_string()));
        // Even if a commit message body said "fix agy gemini thing", scope stays gal-engine.
    }

    // TP-019: empty staging → no-op
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
        let scope = derive_scope_from_files(&[
            "crates/gal-cli/src/main.rs",
            "crates/gal-cli/Cargo.toml",
        ]);
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
        std::fs::write(&msg_path, "fix: stabilize uninstall flow\n\nBody stays untouched.\n").unwrap();

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
    fn extract_crate_name_from_path() {
        assert_eq!(
            extract_crate_name("crates/gal-engine/src/lib.rs"),
            Some("gal-engine".to_string())
        );
        assert_eq!(
            extract_crate_name("docs/manual.md"),
            None
        );
    }

    #[test]
    fn first_component_handles_slash_variants() {
        assert_eq!(first_component("docs/manual.md"), Some("docs".to_string()));
        assert_eq!(first_component("/docs/manual.md"), Some("docs".to_string()));
        assert_eq!(first_component(""), None);
    }
}
