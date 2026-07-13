//! Shared planning-language authority helpers.
//!
//! Single source for the localized-source metadata block schema + parser, the
//! deterministic rendered-source hash, and the equivalence-receipt reader.
//! `planning-check`, `prompt-check`, and the `gal planning-stamp` write-side
//! all consume these helpers so the three never drift into private copies.
//!
//! The metadata block is an HTML comment embedded in a localized (non-English)
//! source plan. It records only relative repo paths, a language code, and hex
//! hashes — never secrets, config values, or absolute machine paths.
//!
//! Canonical block shape:
//!
//! ```text
//! <!-- gal:planning-authority
//! semantic-draft: .dev/plans/<slug>.en.md
//! planLanguage: zh-TW
//! draft-hash: <sha256-hex>
//! rendered-source-hash: <sha256-hex>
//! prompt-hash: none
//! equivalence-verdict: pending
//! -->
//! ```
//!
//! `prompt-hash` / `equivalence-verdict` carry the equivalence proof inline —
//! there is no separate `.equiv.md` receipt file. Before `/plan-to-prompt` has
//! run, both fields are non-empty placeholders (`none` / `pending`); after the
//! prompt is generated and the equivalence gate passes, `gal planning-stamp
//! --equivalence <prompt>` overwrites them with the live prompt hash and
//! `EQUIVALENT`.

// Helpers are wired incrementally across this plan; the parser lands before its
// planning-check consumer and the write-side lands before its subcommand. Allow
// dead_code so every intermediate per-task commit stays clippy-clean.
#![allow(dead_code)]

use gal_engine::ExitCode;
use sha2::{Digest, Sha256};
use std::io;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Marker line opening the canonical localized-source metadata block.
pub(crate) const META_BLOCK_MARKER: &str = "gal:planning-authority";

/// The six canonical metadata field keys, in canonical order.
pub(crate) const META_FIELDS: [&str; 6] = [
    "semantic-draft",
    "planLanguage",
    "draft-hash",
    "rendered-source-hash",
    "prompt-hash",
    "equivalence-verdict",
];

/// Non-empty placeholder values for `prompt-hash` / `equivalence-verdict`
/// before `/plan-to-prompt` has generated the English execution prompt.
pub(crate) const PENDING_PROMPT_HASH: &str = "none";
pub(crate) const PENDING_EQUIVALENCE_VERDICT: &str = "pending";

/// Parsed localized-source metadata block. Every field is a relative repo path,
/// a language code, a hex hash, or a verdict literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalizedMeta {
    /// Relative path to the EN semantic draft (`.dev/plans/<slug>.en.md`).
    pub semantic_draft: String,
    /// planLanguage code recorded at render time (e.g. `zh-TW`).
    pub plan_language: String,
    /// SHA-256 hex of the EN draft body at last render/reconcile.
    pub draft_hash: String,
    /// SHA-256 hex of the localized body (metadata block excluded) at last render.
    pub rendered_source_hash: String,
    /// SHA-256 hex of the English execution prompt body, stamped by
    /// `gal planning-stamp --equivalence` after `/plan-to-prompt`'s equivalence
    /// gate passes. `"none"` before the prompt exists.
    pub prompt_hash: String,
    /// Equivalence verdict — `"pending"` before the prompt exists, `EQUIVALENT`
    /// once the equivalence gate has passed.
    pub equivalence_verdict: String,
}

/// Parse the canonical metadata block out of a localized source plan.
///
/// Returns `None` when the block marker is absent or any of the six fields is
/// missing/empty — a partial or stale (pre-migration five-field) block is
/// never a valid `LocalizedMeta`.
pub(crate) fn parse_localized_meta(text: &str) -> Option<LocalizedMeta> {
    let block = extract_meta_block(text)?;
    Some(LocalizedMeta {
        semantic_draft: block_field(&block, "semantic-draft")?,
        plan_language: block_field(&block, "planLanguage")?,
        draft_hash: block_field(&block, "draft-hash")?,
        rendered_source_hash: block_field(&block, "rendered-source-hash")?,
        prompt_hash: block_field(&block, "prompt-hash")?,
        equivalence_verdict: block_field(&block, "equivalence-verdict")?,
    })
}

/// Compute the deterministic rendered-source hash of a localized source body.
///
/// The hash domain EXCLUDES the metadata block whole (so it never references its
/// own hash fields — no perpetual-stale self-reference) and is normalized for
/// cross-machine stability: LF line endings (covers Windows CRLF), trailing
/// whitespace stripped per line, and Unicode NFC. Returns lowercase SHA-256 hex.
pub(crate) fn rendered_source_hash(body: &str) -> String {
    let stripped = strip_meta_block(body);
    let normalized = normalize_for_hash(&stripped);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex_lower(&hasher.finalize())
}

/// Excise the whole `<!-- gal:planning-authority ... -->` block from a body,
/// leaving any other HTML comments untouched. Bodies without the block are
/// returned unchanged.
fn strip_meta_block(body: &str) -> String {
    let Some(marker) = body.find(META_BLOCK_MARKER) else {
        return body.to_string();
    };
    let open = body[..marker].rfind("<!--").unwrap_or(marker);
    let Some(rel_close) = body[marker..].find("-->") else {
        return body.to_string();
    };
    let close = marker + rel_close + "-->".len();
    let mut result = String::with_capacity(body.len());
    result.push_str(&body[..open]);
    result.push_str(&body[close..]);
    result
}

/// LF line endings + per-line trailing-whitespace strip + Unicode NFC.
fn normalize_for_hash(text: &str) -> String {
    let lf = text.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed: Vec<&str> = lf.lines().map(str::trim_end).collect();
    trimmed.join("\n").nfc().collect()
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Stamp a localized source plan's metadata `draft-hash` and
/// `rendered-source-hash` fields in place (deterministic write; narrative prose
/// untouched). The rendered-source hash excludes the metadata block, so writing
/// the hash fields does not change the value it stamps — re-computing after a
/// stamp matches (no perpetual stale).
pub(crate) fn stamp_metadata_hash(localized_path: &Path) -> io::Result<()> {
    let localized = std::fs::read_to_string(localized_path)?;
    let Some(meta) = parse_localized_meta(&localized) else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "no planning-authority metadata block"));
    };
    let source_hash = rendered_source_hash(&localized);
    let Some(draft_body) = resolve_relative_from(localized_path, &meta.semantic_draft) else {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("EN draft not found: {}", meta.semantic_draft)));
    };
    let draft_hash = rendered_source_hash(&draft_body);
    let updated = set_meta_field(&localized, "draft-hash", &draft_hash);
    let updated = set_meta_field(&updated, "rendered-source-hash", &source_hash);
    std::fs::write(localized_path, updated)
}

/// Stamp a localized source plan's inline `prompt-hash` + `equivalence-verdict`
/// fields in place, replacing the pre-prompt placeholders. Deterministic write —
/// the command runs this only after the planning-stage command's equivalence
/// gate already passed, so the recorded verdict is always `EQUIVALENT`.
///
/// `prompt_path` is `.dev/plans/<slug>.prompt.md`; the paired source plan
/// (`.dev/plans/<slug>.md`) is derived from it and must already carry a
/// six-field `gal:planning-authority` block (from an earlier default-mode
/// stamp) or this returns an error.
pub(crate) fn stamp_equivalence(prompt_path: &Path) -> io::Result<()> {
    let prompt_body = std::fs::read_to_string(prompt_path)?;
    let Some(source_path) = paired_source_plan(prompt_path) else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("cannot derive source plan from {}", prompt_path.display())));
    };
    let localized = std::fs::read_to_string(&source_path)?;
    if parse_localized_meta(&localized).is_none() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "source plan has no six-field planning-authority metadata block"));
    }
    let prompt_hash = rendered_source_hash(&prompt_body);
    let updated = set_meta_field(&localized, "prompt-hash", &prompt_hash);
    let updated = set_meta_field(&updated, "equivalence-verdict", "EQUIVALENT");
    std::fs::write(&source_path, updated)
}

/// Read a repo-relative path by trying it against each ancestor of `anchor`
/// until one resolves — robust for both a real repo root and a temp fixture.
pub(crate) fn resolve_relative_from(anchor: &Path, rel: &str) -> Option<String> {
    for ancestor in anchor.ancestors() {
        if let Ok(body) = std::fs::read_to_string(ancestor.join(rel)) {
            return Some(body);
        }
    }
    None
}

/// Replace the value of a `key:` field inside the metadata block only, keeping
/// the line's original indentation. Lines outside the block are untouched.
fn set_meta_field(text: &str, key: &str, value: &str) -> String {
    let mut out = String::with_capacity(text.len() + value.len());
    let mut in_meta = false;
    for line in text.lines() {
        if line.contains(META_BLOCK_MARKER) {
            in_meta = true;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_meta && line.contains("-->") {
            in_meta = false;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_meta {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix(key) {
                if rest.trim_start().starts_with(':') {
                    let indent = &line[..line.len() - trimmed.len()];
                    out.push_str(indent);
                    out.push_str(key);
                    out.push_str(": ");
                    out.push_str(value);
                    out.push('\n');
                    continue;
                }
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Return the raw text between the block marker and its closing `-->`.
fn extract_meta_block(text: &str) -> Option<String> {
    let start = text.find(META_BLOCK_MARKER)?;
    let after = &text[start + META_BLOCK_MARKER.len()..];
    let end = after.find("-->")?;
    Some(after[..end].to_string())
}

/// Read a single `key: value` field from an already-extracted block body.
///
/// A field matches only when the key is followed (after trimming) by `:`, so a
/// longer key such as `rendered-source-hash` never satisfies a lookup for the
/// shorter `draft-hash`.
fn block_field(block: &str, key: &str) -> Option<String> {
    for line in block.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(key) {
            if let Some(value) = rest.trim_start().strip_prefix(':') {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

/// Derive the paired source plan path from an execution prompt path:
/// `.dev/plans/<slug>.prompt.md` -> `.dev/plans/<slug>.md`.
fn paired_source_plan(prompt_path: &Path) -> Option<PathBuf> {
    let name = prompt_path.file_name()?.to_str()?;
    let slug = name.strip_suffix(".prompt.md")?;
    Some(prompt_path.with_file_name(format!("{slug}.md")))
}

/// `gal planning-stamp` — internal deterministic hash write-side.
///
/// - `gal planning-stamp <localized-source>` → stamp the localized metadata hashes.
/// - `gal planning-stamp --equivalence <prompt>` → derive the paired source
///   plan and stamp its inline `prompt-hash` + `equivalence-verdict` fields.
///   No `--receipt` argument — there is no separate receipt file.
pub(crate) fn cmd_planning_stamp(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let mut equivalence_prompt: Option<PathBuf> = None;
    let mut positional: Option<PathBuf> = None;
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--equivalence" => match it.next() {
                Some(v) => equivalence_prompt = Some(PathBuf::from(v)),
                None => {
                    eprintln!("gal planning-stamp: --equivalence requires a prompt path");
                    return ExitCode::Usage;
                }
            },
            s if s.starts_with("--") => {
                eprintln!("gal planning-stamp: unknown option '{s}'");
                return ExitCode::Usage;
            }
            s => positional = Some(PathBuf::from(s)),
        }
    }

    if let Some(prompt_path) = equivalence_prompt {
        if !prompt_path.exists() {
            eprintln!("gal planning-stamp: prompt not found: {}", prompt_path.display());
            return ExitCode::Usage;
        }
        match stamp_equivalence(&prompt_path) {
            Ok(()) => {
                println!("gal planning-stamp: stamped equivalence verdict for {}", prompt_path.display());
                ExitCode::Success
            }
            Err(err) => {
                eprintln!("gal planning-stamp: cannot stamp equivalence for {}: {err}", prompt_path.display());
                ExitCode::Error
            }
        }
    } else {
        let Some(localized) = positional else {
            eprintln!("gal planning-stamp requires a <localized-source> path (or --equivalence)");
            return ExitCode::Usage;
        };
        match stamp_metadata_hash(&localized) {
            Ok(()) => {
                println!("gal planning-stamp: stamped metadata hashes in {}", localized.display());
                ExitCode::Success
            }
            Err(err) => {
                eprintln!("gal planning-stamp: cannot stamp {}: {err}", localized.display());
                ExitCode::Error
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_block() -> &'static str {
        "# Plan: 範例\n\n\
<!-- gal:planning-authority\n\
semantic-draft: .dev/plans/example.en.md\n\
planLanguage: zh-TW\n\
draft-hash: aaaa1111\n\
rendered-source-hash: bbbb2222\n\
prompt-hash: cccc3333\n\
equivalence-verdict: EQUIVALENT\n\
-->\n\n\
## Goal\n目標\n"
    }

    #[test]
    fn parse_localized_meta_reads_all_fields() {
        let meta = parse_localized_meta(sample_block()).expect("block should parse");
        assert_eq!(meta.semantic_draft, ".dev/plans/example.en.md");
        assert_eq!(meta.plan_language, "zh-TW");
        assert_eq!(meta.draft_hash, "aaaa1111");
        assert_eq!(meta.rendered_source_hash, "bbbb2222");
        assert_eq!(meta.prompt_hash, "cccc3333");
        assert_eq!(meta.equivalence_verdict, "EQUIVALENT");
    }

    #[test]
    fn pending_placeholders_parse_as_valid_non_empty_fields() {
        let block = sample_block()
            .replace("prompt-hash: cccc3333", &format!("prompt-hash: {PENDING_PROMPT_HASH}"))
            .replace("equivalence-verdict: EQUIVALENT", &format!("equivalence-verdict: {PENDING_EQUIVALENCE_VERDICT}"));
        let meta = parse_localized_meta(&block).expect("pending placeholders still parse");
        assert_eq!(meta.prompt_hash, PENDING_PROMPT_HASH);
        assert_eq!(meta.equivalence_verdict, PENDING_EQUIVALENCE_VERDICT);
    }

    #[test]
    fn no_block_returns_none() {
        assert!(parse_localized_meta("# Plan\n\n## Goal\n目標\n").is_none());
    }

    #[test]
    fn partial_block_returns_none() {
        let partial = "<!-- gal:planning-authority\nsemantic-draft: x.en.md\nplanLanguage: zh-TW\n-->\n";
        assert!(parse_localized_meta(partial).is_none());
    }

    #[test]
    fn crlf_and_lf_same_content_same_hash() {
        let lf = "## Goal\n目標一行\n\n## Approach\n做法\n";
        let crlf = "## Goal\r\n目標一行\r\n\r\n## Approach\r\n做法\r\n";
        assert_eq!(rendered_source_hash(lf), rendered_source_hash(crlf));
    }

    #[test]
    fn metadata_field_change_keeps_body_hash() {
        let with_a = sample_block();
        // Same body, only the metadata hash fields differ → body hash unchanged.
        let with_b = with_a
            .replace("draft-hash: aaaa1111", "draft-hash: 9999ffff")
            .replace("rendered-source-hash: bbbb2222", "rendered-source-hash: 7777eeee");
        assert_ne!(with_a, with_b.as_str());
        assert_eq!(rendered_source_hash(with_a), rendered_source_hash(&with_b));
    }

    #[test]
    fn prose_change_changes_hash() {
        let base = sample_block();
        let edited = base.replace("目標", "新目標");
        assert_ne!(rendered_source_hash(base), rendered_source_hash(&edited));
    }

    #[test]
    fn trailing_whitespace_does_not_change_hash() {
        let clean = "## Goal\n目標\n";
        let trailing = "## Goal   \n目標\t\n";
        assert_eq!(rendered_source_hash(clean), rendered_source_hash(trailing));
    }

    fn stamp_fixture(dir: &TempDir) -> (std::path::PathBuf, &'static str) {
        std::fs::create_dir_all(dir.path().join(".dev/plans")).unwrap();
        let draft_body = "# EN draft\n\n## Goal\nGoal text\n";
        std::fs::write(dir.path().join(".dev/plans/x.en.md"), draft_body).unwrap();
        let localized_path = dir.path().join(".dev/plans/x.md");
        let localized = "# Plan\n\n## Goal\n目標\n\n<!-- gal:planning-authority\nsemantic-draft: .dev/plans/x.en.md\nplanLanguage: zh-TW\ndraft-hash: PLACEHOLDER\nrendered-source-hash: PLACEHOLDER\nprompt-hash: none\nequivalence-verdict: pending\n-->\n";
        std::fs::write(&localized_path, localized).unwrap();
        (localized_path, draft_body)
    }

    #[test]
    fn stamp_sets_draft_hash() {
        let dir = TempDir::new().unwrap();
        let (localized_path, draft_body) = stamp_fixture(&dir);
        stamp_metadata_hash(&localized_path).unwrap();
        let stamped = std::fs::read_to_string(&localized_path).unwrap();
        let meta = parse_localized_meta(&stamped).unwrap();
        assert_eq!(meta.draft_hash, rendered_source_hash(draft_body));
        assert_ne!(meta.draft_hash, "PLACEHOLDER");
    }

    #[test]
    fn stamp_updates_metadata_rendered_hash_to_match() {
        let dir = TempDir::new().unwrap();
        let (localized_path, _) = stamp_fixture(&dir);
        stamp_metadata_hash(&localized_path).unwrap();
        let stamped = std::fs::read_to_string(&localized_path).unwrap();
        let meta = parse_localized_meta(&stamped).unwrap();
        // Self-reference safety: re-computing the hash of the stamped file matches
        // the value just written (metadata block is excluded from the hash domain).
        assert_eq!(meta.rendered_source_hash, rendered_source_hash(&stamped));
    }

    #[test]
    fn planning_stamp_command_parses() {
        // command token is recognized
        assert_eq!(gal_engine::CommandKind::parse("planning-stamp"), Some(gal_engine::CommandKind::PlanningStamp));
        // default mode stamps the localized metadata hashes
        let dir = TempDir::new().unwrap();
        let (localized_path, _) = stamp_fixture(&dir);
        let args = vec!["planning-stamp".to_string(), localized_path.to_string_lossy().into_owned()];
        assert_eq!(cmd_planning_stamp(&args), ExitCode::Success);
        let stamped = std::fs::read_to_string(&localized_path).unwrap();
        let meta = parse_localized_meta(&stamped).unwrap();
        assert_ne!(meta.rendered_source_hash, "PLACEHOLDER");
    }

    #[test]
    fn planning_stamp_equivalence_requires_existing_prompt() {
        let args = vec![
            "planning-stamp".to_string(),
            "--equivalence".to_string(),
            "does-not-exist.prompt.md".to_string(),
        ];
        assert_eq!(cmd_planning_stamp(&args), ExitCode::Usage);
    }

    fn equivalence_fixture(dir: &TempDir) -> (std::path::PathBuf, std::path::PathBuf, String) {
        std::fs::create_dir_all(dir.path().join(".dev/plans")).unwrap();
        let localized_path = dir.path().join(".dev/plans/x.md");
        let localized = "# Plan\n目標\n\n<!-- gal:planning-authority\nsemantic-draft: .dev/plans/x.en.md\nplanLanguage: zh-TW\ndraft-hash: aaaa\nrendered-source-hash: bbbb\nprompt-hash: none\nequivalence-verdict: pending\n-->\n";
        std::fs::write(&localized_path, localized).unwrap();
        let prompt_path = dir.path().join(".dev/plans/x.prompt.md");
        let prompt_body = "# Prompt\nGoal\n".to_string();
        std::fs::write(&prompt_path, &prompt_body).unwrap();
        (localized_path, prompt_path, prompt_body)
    }

    #[test]
    fn stamp_equivalence_writes_prompt_hash_and_verdict_into_source_plan() {
        let dir = TempDir::new().unwrap();
        let (localized_path, prompt_path, prompt_body) = equivalence_fixture(&dir);
        stamp_equivalence(&prompt_path).unwrap();
        let stamped = std::fs::read_to_string(&localized_path).unwrap();
        let meta = parse_localized_meta(&stamped).unwrap();
        assert_eq!(meta.prompt_hash, rendered_source_hash(&prompt_body));
        assert_eq!(meta.equivalence_verdict, "EQUIVALENT");
        // No sibling receipt file is created.
        assert!(!dir.path().join(".dev/plans/x.equiv.md").exists());
    }

    #[test]
    fn planning_stamp_equivalence_command_stamps_source_plan() {
        let dir = TempDir::new().unwrap();
        let (localized_path, prompt_path, _) = equivalence_fixture(&dir);
        let args = vec![
            "planning-stamp".to_string(),
            "--equivalence".to_string(),
            prompt_path.to_string_lossy().into_owned(),
        ];
        assert_eq!(cmd_planning_stamp(&args), ExitCode::Success);
        let stamped = std::fs::read_to_string(&localized_path).unwrap();
        let meta = parse_localized_meta(&stamped).unwrap();
        assert_eq!(meta.equivalence_verdict, "EQUIVALENT");
    }

    #[test]
    fn draft_hash_lookup_not_satisfied_by_rendered_source_hash() {
        // A block missing `draft-hash` but carrying `rendered-source-hash` must
        // not let the longer key satisfy the shorter lookup.
        let block = "<!-- gal:planning-authority\n\
semantic-draft: x.en.md\n\
planLanguage: zh-TW\n\
rendered-source-hash: bbbb2222\n\
prompt-hash: none\n\
equivalence-verdict: pending\n\
-->\n";
        assert!(parse_localized_meta(block).is_none());
    }

    #[test]
    fn stale_five_field_block_returns_none() {
        // A pre-migration five-field block (still carrying the retired
        // `equivalence-receipt` pointer, missing the new two fields) must not
        // silently parse — this is exactly the live-migration target.
        let block = "<!-- gal:planning-authority\n\
semantic-draft: x.en.md\n\
planLanguage: zh-TW\n\
draft-hash: aaaa\n\
rendered-source-hash: bbbb\n\
equivalence-receipt: x.equiv.md\n\
-->\n";
        assert!(parse_localized_meta(block).is_none());
    }
}
