//! Naming gate: scans durable surfaces for naming-authority violations.
//!
//! The authority is `docs/naming.md`. This module enforces two of its rules
//! mechanically:
//!
//! * **Provenance discipline** — plan-task IDs (`T-NN`, `R-NN`, `TP-NN`,
//!   `FU-NN`, `BUG-N`) must not appear in durable artifacts. They are allowed
//!   only under `.dev/**`.
//! * **Retired terms** — phrases listed in the `RETIRED-TERMS` block of
//!   `docs/naming.md` (e.g. `MCP provider`) must not appear in durable artifacts.
//!
//! A *durable surface* is every tracked file except `.dev/**`. The two naming
//! **teaching surfaces** — `docs/naming.md` and `conventions/naming.md` — are
//! additionally excluded, because they legitimately name retired terms (and
//! quote plan-ID shapes) in order to define them.
//!
//! The plan-ID regex matches only *numeric* IDs (`[0-9]{2,3}`), so rule-text
//! placeholders such as `T-NN` or `BUG-X` never trip the gate.

use regex::Regex;
use std::fs;
use std::path::Path;

/// File extensions the tree walker scans (durable text surfaces).
const SCAN_EXTENSIONS: &[&str] = &["rs", "md", "toml", "json", "yml", "yaml"];
/// Directories the tree walker never descends into: VCS internals, build
/// output, dependencies, gitignored scratch (`.tmp`, `graphify-out`), and
/// execution memory (`.dev`, also excluded by [`NamingGate::is_excluded`]).
/// `graphify-out` holds the graphify tool's generated graph, whose node
/// descriptions self-referentially quote retired terms while describing this
/// very gate — scanning it is a guaranteed false hit that regeneration repeats.
const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", ".tmp", ".dev", "graphify-out"];

/// Which authority rule a [`Violation`] breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
    /// A plan-task ID (`T-NN`, `R-NN`, `TP-NN`, `FU-NN`, `BUG-N`) in a durable surface.
    PlanTaskId,
    /// A retired term from the `RETIRED-TERMS` block in a durable surface.
    RetiredTerm,
}

/// One naming-authority violation found by the gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// 1-based line number within the scanned content.
    pub line: usize,
    /// Which rule was broken.
    pub kind: ViolationKind,
    /// The exact text that matched.
    pub matched: String,
}

/// Compiled scanner. Build once with [`NamingGate::new`], reuse across files.
pub struct NamingGate {
    plan_id: Regex,
    bug_id: Regex,
    retired: Vec<(String, Regex)>,
}

impl NamingGate {
    /// Build a gate from the retired-term list (see [`parse_retired_terms`]).
    ///
    /// The two static regexes use longest-first alternation (`TP`/`FU` before
    /// `T`) so `TP-001` matches as a single `TP` ID.
    pub fn new(retired_terms: &[String]) -> Result<Self, regex::Error> {
        let plan_id = Regex::new(r"\b(?:TP|FU|T|R)-[0-9]{2,3}\b")?;
        let bug_id = Regex::new(r"\bBUG-[0-9]+\b")?;
        let mut retired = Vec::with_capacity(retired_terms.len());
        for term in retired_terms {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(term));
            retired.push((term.clone(), Regex::new(&pattern)?));
        }
        Ok(Self {
            plan_id,
            bug_id,
            retired,
        })
    }

    /// True when `rel_path` is exempt from scanning.
    ///
    /// `rel_path` is interpreted repo-relative; both `/` and `\` separators are
    /// accepted. Exempt:
    ///
    /// * anything under `.dev/` (provenance home — plans, research, execution prompts);
    /// * the naming **definition sites**, which necessarily name the forbidden
    ///   tokens in order to define or detect them: `docs/naming.md` (authority),
    ///   its translations (`docs/i18n/<lang>/naming.<lang>.md`),
    ///   `conventions/naming.md` (agent digest), and `naming_gate.rs` (this
    ///   module — its regexes and tests embed sample IDs and retired terms);
    /// * derived carriers that cannot be hand-fixed: `*.private.md`, baked
    ///   `commands/<name>/SKILL.md`, and the generated repo adapters
    ///   (`CLAUDE.md`/`AGENTS.md`/`GEMINI.md`/`.github/copilot-instructions.md`/
    ///   `.agents/rules/gal.md`), which are regenerated from the excluded
    ///   `.dev/project.md`.
    pub fn is_excluded(rel_path: &str) -> bool {
        let norm = rel_path.replace('\\', "/");
        let trimmed = norm.trim_start_matches("./");
        // A naming-authority translation: docs/i18n/<lang>/naming.<lang>.md.
        let is_naming_translation = trimmed.contains("/i18n/")
            && trimmed
                .rsplit('/')
                .next()
                .map(|name| name.starts_with("naming."))
                .unwrap_or(false);
        // Gitignored, non-durable surfaces the tree walk would otherwise reach:
        //  * `*.private.md` — machine-local private notes (never a shared surface);
        //  * baked `commands/<name>/SKILL.md` — a derived product; its source is the
        //    sibling `SKILL.template.md` (which IS scanned), so gating the derived
        //    copy would double-flag and break on rebake drift.
        let is_private_note = trimmed.ends_with(".private.md");
        let is_baked_command_skill =
            trimmed.contains("/commands/") && trimmed.ends_with("/SKILL.md");
        //  * generated repo adapters (CLAUDE.md / AGENTS.md / GEMINI.md /
        //    .github/copilot-instructions.md / .agents/rules/gal.md) — derived
        //    carriers regenerated by `gal sync`/`gal init` from `.dev/project.md`
        //    (itself excluded and legitimately recording plan-task IDs in its
        //    Verified Facts). They cannot be hand-fixed, so gating them would be
        //    unenforceable; the source `.dev/` surface is the right place to scan.
        // Anchored to the repo-root paths `gal sync`/`gal init` actually writes —
        // not a bare basename match, so a hand-authored source doc that happens to
        // be named e.g. `AGENTS.md` in a subdirectory is still scanned.
        let is_generated_adapter = matches!(
            trimmed,
            "CLAUDE.md"
                | "AGENTS.md"
                | "GEMINI.md"
                | ".github/copilot-instructions.md"
                | ".agents/rules/gal.md"
        );
        trimmed == ".dev"
            || trimmed.starts_with(".dev/")
            || trimmed.contains("/.dev/")
            || trimmed.ends_with("docs/naming.md")
            || trimmed == "docs/naming.md"
            || trimmed.ends_with("conventions/naming.md")
            || trimmed.ends_with("naming_gate.rs")
            || is_naming_translation
            || is_private_note
            || is_baked_command_skill
            || is_generated_adapter
    }

    /// Scan raw `content`, returning every violation in line order.
    ///
    /// Pure: no path filtering. Use [`scan_file`](Self::scan_file) to honor
    /// exclusions.
    pub fn scan_content(&self, content: &str) -> Vec<Violation> {
        let mut out = Vec::new();
        for (idx, line) in content.lines().enumerate() {
            let line_no = idx + 1;
            // Collect this line's matches with their column so the report is
            // ordered by textual position, regardless of which regex found them.
            let mut hits: Vec<(usize, ViolationKind, String)> = Vec::new();
            for m in self.plan_id.find_iter(line) {
                hits.push((m.start(), ViolationKind::PlanTaskId, m.as_str().to_string()));
            }
            for m in self.bug_id.find_iter(line) {
                hits.push((m.start(), ViolationKind::PlanTaskId, m.as_str().to_string()));
            }
            for (_term, re) in &self.retired {
                for m in re.find_iter(line) {
                    hits.push((
                        m.start(),
                        ViolationKind::RetiredTerm,
                        m.as_str().to_string(),
                    ));
                }
            }
            hits.sort_by_key(|(start, _, _)| *start);
            for (_, kind, matched) in hits {
                out.push(Violation {
                    line: line_no,
                    kind,
                    matched,
                });
            }
        }
        out
    }

    /// Scan `content` for `rel_path`, returning no violations when the path is
    /// excluded.
    pub fn scan_file(&self, rel_path: &str, content: &str) -> Vec<Violation> {
        if Self::is_excluded(rel_path) {
            return Vec::new();
        }
        self.scan_content(content)
    }

    /// Walk `root`, scanning durable text files (see [`SCAN_EXTENSIONS`]),
    /// skipping [`SKIP_DIRS`] and excluded paths. Returns only files with hits,
    /// each path repo-relative with `/` separators. Unreadable files and
    /// non-UTF-8 content are silently skipped.
    pub fn scan_tree(&self, root: &Path) -> Vec<FileViolations> {
        let mut out = Vec::new();
        self.scan_dir(root, root, &mut out);
        out
    }

    fn scan_dir(&self, root: &Path, dir: &Path, out: &mut Vec<FileViolations>) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if path.is_dir() {
                // Skip never-descend dirs and nested git repo/worktree roots.
                // A subdirectory holding a `.git` entry is a separate checkout
                // (e.g. `.claude/worktrees/<branch>/`) — its generated adapters
                // legitimately carry plan-task IDs derived from `.dev/project.md`
                // and are not part of THIS repo's durable tracked tree.
                if !SKIP_DIRS.contains(&name.as_ref()) && !is_nested_git_root(&path) {
                    self.scan_dir(root, &path, out);
                }
            } else if Self::has_scan_ext(&path) {
                let rel = path.strip_prefix(root).unwrap_or(path.as_path());
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if Self::is_excluded(&rel_str) {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(&path) {
                    let violations = self.scan_content(&content);
                    if !violations.is_empty() {
                        out.push(FileViolations {
                            path: rel_str,
                            violations,
                        });
                    }
                }
            }
        }
    }

    fn has_scan_ext(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| SCAN_EXTENSIONS.contains(&e))
            .unwrap_or(false)
    }
}

/// True when `dir` is a nested git repo/worktree root — it contains a `.git`
/// entry (a gitlink file for a linked worktree, or a `.git` directory).
///
/// Such a directory is a separate checkout and must not be walked as part of
/// this repo's durable-surface scan. Only subdirectories reached during descent
/// are tested; the scan root itself is never passed here, so the top-level
/// repo's own `.git` is handled by [`SKIP_DIRS`] instead.
fn is_nested_git_root(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// A scanned file and the violations found in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileViolations {
    /// Repo-relative path with `/` separators.
    pub path: String,
    /// Violations in this file, in line/column order.
    pub violations: Vec<Violation>,
}

/// Load retired terms from `<root>/docs/naming.md`. Empty if the file is absent.
pub fn load_retired_terms(root: &Path) -> Vec<String> {
    match fs::read_to_string(root.join("docs").join("naming.md")) {
        Ok(content) => parse_retired_terms(&content),
        Err(_) => Vec::new(),
    }
}

/// Extract retired phrases from the `RETIRED-TERMS` block of `docs/naming.md`.
///
/// Reads only the lines between the `# RETIRED-TERMS:BEGIN` and
/// `# RETIRED-TERMS:END` fences; comment lines (`#`) and blanks are ignored.
/// A missing block yields an empty list.
pub fn parse_retired_terms(naming_md: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut inside = false;
    for line in naming_md.lines() {
        let trimmed = line.trim();
        if trimmed == "# RETIRED-TERMS:BEGIN" {
            inside = true;
            continue;
        }
        if trimmed == "# RETIRED-TERMS:END" {
            break;
        }
        if inside && !trimmed.is_empty() && !trimmed.starts_with('#') {
            terms.push(trimmed.to_string());
        }
    }
    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> NamingGate {
        NamingGate::new(&["MCP provider".to_string()]).expect("gate compiles")
    }

    #[test]
    fn parses_retired_terms_block() {
        let md = "intro\n\
                  ```text\n\
                  # RETIRED-TERMS v1 — header comment\n\
                  # RETIRED-TERMS:BEGIN\n\
                  MCP provider\n\
                  # RETIRED-TERMS:END\n\
                  ```\n\
                  trailing\n";
        assert_eq!(parse_retired_terms(md), vec!["MCP provider".to_string()]);
    }

    #[test]
    fn missing_block_yields_empty() {
        assert!(parse_retired_terms("no fences here\nMCP provider\n").is_empty());
    }

    #[test]
    fn skip_dirs_excludes_graphify_out() {
        // graphify-out is gitignored generated scratch whose graph.json
        // self-referentially quotes retired terms; the walker must never
        // descend into it (peer of .tmp).
        assert!(SKIP_DIRS.contains(&"graphify-out"));
    }

    #[test]
    fn flags_plan_ids_and_retired_terms() {
        let content = "fix per T-001 and R-12\n\
                       see the MCP provider config\n\
                       tracked as BUG-7 / TP-014\n";
        let v = gate().scan_content(content);
        let ids: Vec<&str> = v
            .iter()
            .filter(|x| x.kind == ViolationKind::PlanTaskId)
            .map(|x| x.matched.as_str())
            .collect();
        assert_eq!(ids, vec!["T-001", "R-12", "BUG-7", "TP-014"]);
        let retired: Vec<&str> = v
            .iter()
            .filter(|x| x.kind == ViolationKind::RetiredTerm)
            .map(|x| x.matched.as_str())
            .collect();
        assert_eq!(retired, vec!["MCP provider"]);
        // line numbers are 1-based and correct.
        assert_eq!(v[0].line, 1);
        assert_eq!(
            v.iter()
                .find(|x| x.kind == ViolationKind::RetiredTerm)
                .unwrap()
                .line,
            2
        );
    }

    #[test]
    fn retired_term_is_case_insensitive() {
        let v = gate().scan_content("the mcp PROVIDER role");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::RetiredTerm);
    }

    #[test]
    fn tp_matches_as_single_id() {
        let v = gate().scan_content("TP-001");
        let ids: Vec<&str> = v.iter().map(|x| x.matched.as_str()).collect();
        assert_eq!(ids, vec!["TP-001"]);
    }

    #[test]
    fn false_positive_corpus_is_clean() {
        let corpus = "a T-shirt and an x-ray, UTF-8 encoded, RFC 430, vitamin B-12\n\
                      placeholders T-NN R-NN TP-NN FU-NN BUG-X stay clean\n\
                      DEBUG-12 is not a bug id, year T-2024 is too long\n\
                      the MCP host consumes an MCP server\n";
        assert!(
            gate().scan_content(corpus).is_empty(),
            "unexpected violations: {:?}",
            gate().scan_content(corpus)
        );
    }

    #[test]
    fn excluded_paths_are_skipped() {
        let content = "leaked T-001 and MCP provider";
        assert!(gate().scan_file(".dev/plans/foo.md", content).is_empty());
        assert!(gate().scan_file(".dev/state.md", content).is_empty());
        assert!(gate()
            .scan_file(".dev/plans/x.prompt.md", content)
            .is_empty());
        assert!(gate().scan_file("docs/naming.md", content).is_empty());
        assert!(gate()
            .scan_file("plugins/gal-core/conventions/naming.md", content)
            .is_empty());
        // the gate's own source is a definition site (it embeds sample IDs/terms).
        assert!(gate()
            .scan_file("crates/gal-engine/src/naming_gate.rs", content)
            .is_empty());
        // naming-authority translations are definition sites too.
        assert!(gate()
            .scan_file("docs/i18n/zh-Hant/naming.zh-Hant.md", content)
            .is_empty());
        // backslash separators (Windows) are normalized.
        assert!(gate().scan_file(".dev\\plans\\foo.md", content).is_empty());
        // machine-local private notes (gitignored) are not durable surfaces.
        assert!(gate()
            .scan_file("git-publish-strategy.private.md", content)
            .is_empty());
        // baked command skills are derived; the source SKILL.template.md is scanned.
        assert!(gate()
            .scan_file("plugins/gal-core/commands/gal/SKILL.md", content)
            .is_empty());
        // generated repo adapters are derived carriers (regenerated from .dev/project.md,
        // which legitimately records plan-task IDs) — not hand-fixable, so excluded.
        for adapter in [
            "CLAUDE.md",
            "AGENTS.md",
            "GEMINI.md",
            ".github/copilot-instructions.md",
            ".agents/rules/gal.md",
        ] {
            assert!(
                gate().scan_file(adapter, content).is_empty(),
                "generated adapter should be excluded: {adapter}"
            );
        }
        // but a real source doc with the same leak is still scanned.
        assert!(!gate().scan_file("docs/devguide.md", content).is_empty());
        // adapter exclusion is root-anchored: a same-named source doc in a subdir is NOT excluded.
        assert!(!gate().scan_file("docs/AGENTS.md", content).is_empty());
    }

    #[test]
    fn command_skill_template_and_skills_dir_stay_scanned() {
        let content = "leaked T-001 here";
        // The SKILL.template.md source IS a durable surface.
        assert_eq!(
            gate()
                .scan_file("plugins/gal-core/commands/gal/SKILL.template.md", content)
                .len(),
            1
        );
        // skills/<name>/SKILL.md are source contracts, not baked — still scanned.
        assert_eq!(
            gate()
                .scan_file("plugins/gal-core/skills/doc-sync/SKILL.md", content)
                .len(),
            1
        );
    }

    #[test]
    fn durable_surface_is_scanned() {
        let content = "leaked T-001 here";
        let v = gate().scan_file("crates/base/src/lib.rs", content);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, ViolationKind::PlanTaskId);
    }

    #[test]
    fn scan_tree_reports_only_durable_hits() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let write = |rel: &str, body: &str| {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        };
        write("crates/base/src/lib.rs", "// see T-001\n");
        write(".dev/plans/x.md", "T-002 here\n"); // excluded dir (.dev/)
        write("docs/naming.md", "MCP provider definition\n"); // excluded definition site
        write("target/gen.rs", "T-003 build artifact\n"); // skipped dir
        write("README.md", "clean doc, no ids\n"); // durable, clean
        write("notes.txt", "T-004 but wrong ext\n"); // ext not scanned

        let hits = gate().scan_tree(root);
        assert_eq!(hits.len(), 1, "unexpected: {hits:?}");
        assert_eq!(hits[0].path, "crates/base/src/lib.rs");
        assert_eq!(hits[0].violations[0].matched, "T-001");
    }

    #[test]
    fn scan_tree_skips_nested_git_worktree() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let write = |rel: &str, body: &str| {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        };
        // A real durable surface at the top level — must be flagged.
        write("crates/base/src/lib.rs", "// see T-001\n");
        // A nested git worktree: marked by a `.git` gitlink file at its root.
        // Its generated adapter carries a plan-task ID derived from project.md.
        write(
            ".claude/worktrees/feat-x/.git",
            "gitdir: /repo/.git/worktrees/feat-x\n",
        );
        write(".claude/worktrees/feat-x/CLAUDE.md", "task T-042 here\n");
        write(
            ".claude/worktrees/feat-x/crates/base/src/lib.rs",
            "// T-099\n",
        );

        let hits = gate().scan_tree(root);
        assert_eq!(
            hits.len(),
            1,
            "nested worktree must be skipped; only the top-level file should be flagged: {hits:?}"
        );
        assert_eq!(hits[0].path, "crates/base/src/lib.rs");
    }
}
