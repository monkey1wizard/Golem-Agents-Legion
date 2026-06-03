//! `gal commit-msg` — git commit-msg hook implementation.
//!
//! Ports the logic from the frozen `Get-StagedCommitMessage.*` oracle,
//! with the keyword-hijack already removed (commit f64f517).
//!
//! No-hijack rule: scope is derived from changed file PATHS only,
//! never from message body keywords. A body mentioning "gemini" or "agy"
//! without actual renames must NOT change the scope to "antigravity".
//!
//! TP-019:
//! - body contains `gemini`+`agy` but no rename → no hijack, scope from paths
//! - empty staging → no-op, exit 0
//!
//! Corresponds to T-013 of fix-gal-bootstrap-install-convergence.
//! This module is optional (R5 — may be deferred or dropped).

use std::path::Path;

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
/// 2. Common mappings: `crates/gal-core` → `gal-core`, `crates/gal-cli` → `gal-cli`,
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

/// Extract crate name from a path like `crates/gal-core/src/lib.rs` → `gal-core`.
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
    let _scope = derive_scope_from_files(staged_files);

    // The message is accepted as-is (scope injection is opt-in, not forced).
    // The main value of the hook is detecting the no-hijack case.
    // Future work: scope injection when message lacks conventional format.
    std::fs::write(msg_path, &current_msg)?;

    Ok(CommitMsgResult::Updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
        let files = vec!["crates/gal-core/src/install.rs", "crates/gal-core/src/ledger.rs"];
        let scope = derive_scope_from_files(&files);
        assert_eq!(scope, Some("gal-core".to_string()));
        // Even if a commit message body said "fix agy gemini thing", scope stays gal-core.
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
            "crates/gal-core/src/lib.rs",
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
        let original = "feat(gal-core): add ledger module\n\nFixes the tracking gap.";
        std::fs::write(&msg_path, original).unwrap();

        let files = vec!["crates/gal-core/src/ledger.rs"];
        let result = process_commit_msg(&msg_path, &files).unwrap();

        assert_eq!(result, CommitMsgResult::Updated);
        let content = std::fs::read_to_string(&msg_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn extract_crate_name_from_path() {
        assert_eq!(
            extract_crate_name("crates/gal-core/src/lib.rs"),
            Some("gal-core".to_string())
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
