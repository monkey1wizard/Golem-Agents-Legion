//! `gal translation-freshness` — report translation freshness for docs/i18n.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The repo-relative path that is the sole semantic authority a locale
/// terminology profile tracks freshness against (see `docs/naming.md`
/// "Semantic Authority vs. Locale Presentation Profiles").
const TERMINOLOGY_SOURCE: &str = "docs/naming.md";

/// Distinguishes the two kinds of artifact tracked under `docs/i18n/<lang>/`.
/// A terminology profile is never treated as, or checked like, a full
/// translation — see `docs/devguide.md` "Two Translation Artifact Kinds".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    /// A full translation of one of the allowlisted canonical docs.
    Translation,
    /// A locale terminology profile tracked against `docs/naming.md`.
    Terminology,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRow {
    pub doc: String,
    pub lang: String,
    pub status: String,
    pub note: String,
    pub kind: ArtifactKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslationReport {
    pub rows: Vec<TranslationRow>,
    pub current: usize,
    pub stale: usize,
    pub missing: usize,
    pub unexpected: usize,
}

pub fn run_translation_freshness(repo_root: &Path, allowlist: &[&str]) -> TranslationReport {
    let i18n_root = repo_root.join("docs").join("i18n");
    if !i18n_root.is_dir() {
        return TranslationReport::default();
    }

    let languages: Vec<String> = fs::read_dir(&i18n_root)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();

    let mut rows = Vec::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();

    for file in markdown_files(&i18n_root) {
        if file.file_name().and_then(|name| name.to_str()) == Some("guide.md") {
            continue;
        }
        let frontmatter = read_frontmatter(&file);
        let Some(source) = frontmatter.get("source").cloned() else {
            rows.push(TranslationRow {
                doc: "(unknown)".to_string(),
                lang: "?".to_string(),
                status: "stale".to_string(),
                note: "no front-matter source".to_string(),
                kind: ArtifactKind::Translation,
            });
            continue;
        };
        let lang = frontmatter.get("lang").cloned().unwrap_or_default();
        let stamp = frontmatter
            .get("source_commit")
            .cloned()
            .unwrap_or_default();
        seen.insert((source.clone(), lang.clone()));

        // A terminology profile tracks freshness against docs/naming.md and
        // is its own recognized kind — never `missing` against the doc
        // allowlist, never `unexpected` for being off that allowlist. The
        // filename must also match `terminology.<lang>.md`; any other
        // filename declaring this source is a disguised full translation
        // and falls through to the ordinary allowlist-gated path.
        let is_terminology_filename = file
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name == format!("terminology.{lang}.md"))
            .unwrap_or(false);
        let kind = if source == TERMINOLOGY_SOURCE && is_terminology_filename {
            ArtifactKind::Terminology
        } else {
            ArtifactKind::Translation
        };
        let is_allowlisted =
            kind == ArtifactKind::Terminology || allowlist.iter().any(|allowed| *allowed == source);

        let source_path = repo_root.join(&source);
        if !source_path.is_file() {
            rows.push(TranslationRow {
                doc: source,
                lang,
                status: "stale".to_string(),
                note: "source missing".to_string(),
                kind,
            });
            continue;
        }

        let actual = latest_commit(repo_root, &source);
        if is_hash(&stamp)
            && actual
                .as_ref()
                .map(|hash| hash.starts_with(&stamp) || stamp.starts_with(hash))
                .unwrap_or(false)
        {
            let status = if is_allowlisted {
                "current"
            } else {
                "unexpected"
            };
            let note = if is_allowlisted {
                String::new()
            } else {
                "full translation of a non-allowlisted doc".to_string()
            };
            rows.push(TranslationRow {
                doc: source,
                lang,
                status: status.to_string(),
                note,
                kind,
            });
        } else if !is_allowlisted {
            rows.push(TranslationRow {
                doc: source,
                lang,
                status: "unexpected".to_string(),
                note: "full translation of a non-allowlisted doc".to_string(),
                kind,
            });
        } else {
            let note = if is_hash(&stamp) {
                actual
                    .as_ref()
                    .map(|hash| format!("source advanced to {}", &hash[..7.min(hash.len())]))
                    .unwrap_or_else(|| "source missing git history".to_string())
            } else {
                format!("unstamped ({stamp})")
            };
            rows.push(TranslationRow {
                doc: source,
                lang,
                status: "stale".to_string(),
                note,
                kind,
            });
        }
    }

    let mut allowlist_missing = std::collections::BTreeSet::new();
    for src in allowlist {
        for lang in &languages {
            if !seen.contains(&(src.to_string(), lang.clone())) {
                allowlist_missing.insert((src.to_string(), lang.clone()));
                rows.push(TranslationRow {
                    doc: src.to_string(),
                    lang: lang.clone(),
                    status: "missing".to_string(),
                    note: "allowlisted, no translation".to_string(),
                    kind: ArtifactKind::Translation,
                });
            }
        }
    }

    for lang in &languages {
        let pair = (TERMINOLOGY_SOURCE.to_string(), lang.clone());
        if !seen.contains(&pair) && !allowlist_missing.contains(&pair) {
            rows.push(TranslationRow {
                doc: TERMINOLOGY_SOURCE.to_string(),
                lang: lang.clone(),
                status: "missing".to_string(),
                note: "terminology profile not yet created".to_string(),
                kind: ArtifactKind::Terminology,
            });
        }
    }

    rows.sort_by(|left, right| {
        left.doc
            .cmp(&right.doc)
            .then_with(|| left.lang.cmp(&right.lang))
    });

    let mut report = TranslationReport::default();
    for row in rows {
        match row.status.as_str() {
            "current" => report.current += 1,
            "stale" => report.stale += 1,
            "missing" => report.missing += 1,
            "unexpected" => report.unexpected += 1,
            _ => {}
        }
        report.rows.push(row);
    }
    report
}

fn markdown_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk_markdown(root, &mut files);
    files
}

fn walk_markdown(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            walk_markdown(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            files.push(path);
        }
    }
}

fn read_frontmatter(path: &Path) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    let Ok(content) = fs::read_to_string(path) else {
        return map;
    };
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return map;
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}

fn latest_commit(repo_root: &Path, source: &str) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["log", "-1", "--format=%H", "--", source])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn is_hash(value: &str) -> bool {
    let len = value.len();
    (7..=40).contains(&len) && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_i18n_tree_returns_empty_report() {
        let temp = TempDir::new().unwrap();
        let report = run_translation_freshness(temp.path(), &["README.md"]);
        assert!(report.rows.is_empty());
    }

    #[test]
    fn report_classifies_current_stale_and_missing() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        fs::create_dir_all(repo.join("docs").join("i18n").join("zh-Hant")).unwrap();
        fs::write(repo.join("README.md"), "root\n").unwrap();
        fs::create_dir_all(repo.join("docs")).unwrap();
        fs::write(repo.join("docs").join("manual.md"), "manual\n").unwrap();

        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["init", "-q"])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["config", "user.email", "test@example.com"])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["config", "user.name", "Test User"])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["config", "commit.gpgsign", "false"])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["add", "README.md", "docs/manual.md"])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["commit", "-m", "seed", "-q"])
            .status()
            .unwrap();

        let readme_hash = latest_commit(repo, "README.md").unwrap();
        fs::write(
            repo.join("docs")
                .join("i18n")
                .join("zh-Hant")
                .join("README.zh-Hant.md"),
            format!(
                "---\nsource: README.md\nlang: zh-Hant\nsource_commit: {}\n---\ntranslated\n",
                &readme_hash[..7]
            ),
        )
        .unwrap();
        fs::write(
            repo.join("docs")
                .join("i18n")
                .join("zh-Hant")
                .join("manual.zh-Hant.md"),
            "---\nsource: docs/manual.md\nlang: zh-Hant\nsource_commit: PENDING\n---\ntranslated\n",
        )
        .unwrap();

        let report =
            run_translation_freshness(repo, &["README.md", "docs/manual.md", "docs/missing.md"]);
        assert_eq!(report.current, 1);
        assert_eq!(report.stale, 1);
        // "docs/missing.md" (allowlisted, no translation) plus the
        // terminology profile, which is also absent from this fixture.
        assert_eq!(report.missing, 2);
    }

    /// Sets up a repo with a git history for `source` and returns its
    /// current commit hash, used by the fixtures below to stamp a
    /// `source_commit` that will read as `current`.
    fn init_repo_with_source(repo: &Path, source_relpath: &str, body: &str) -> String {
        let full_path = repo.join(source_relpath);
        fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        fs::write(&full_path, body).unwrap();

        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["init", "-q"])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["config", "user.email", "test@example.com"])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["config", "user.name", "Test User"])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["config", "commit.gpgsign", "false"])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["add", source_relpath])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["commit", "-m", "seed", "-q"])
            .status()
            .unwrap();

        latest_commit(repo, source_relpath).unwrap()
    }

    #[test]
    fn full_translation_of_non_allowlisted_doc_is_unexpected() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        let arch_hash = init_repo_with_source(repo, "docs/architecture.md", "architecture\n");

        fs::create_dir_all(repo.join("docs").join("i18n").join("zh-Hant")).unwrap();
        fs::write(
            repo.join("docs")
                .join("i18n")
                .join("zh-Hant")
                .join("architecture.zh-Hant.md"),
            format!(
                "---\nsource: docs/architecture.md\nlang: zh-Hant\nsource_commit: {}\n---\ntranslated\n",
                &arch_hash[..7]
            ),
        )
        .unwrap();

        let report = run_translation_freshness(repo, &["README.md", "docs/manual.md"]);
        let row = report
            .rows
            .iter()
            .find(|row| row.doc == "docs/architecture.md")
            .expect("architecture row present");
        assert_eq!(row.status, "unexpected");
        assert_eq!(row.kind, ArtifactKind::Translation);
        assert_eq!(report.unexpected, 1);
        // Not counted as current, stale, or missing.
        assert_eq!(report.current, 0);
        assert_eq!(report.stale, 0);
    }

    #[test]
    fn terminology_profile_is_tracked_current_and_stale_distinct_from_translations() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        let naming_hash = init_repo_with_source(repo, "docs/naming.md", "naming\n");

        fs::create_dir_all(repo.join("docs").join("i18n").join("zh-Hant")).unwrap();
        fs::write(
            repo.join("docs")
                .join("i18n")
                .join("zh-Hant")
                .join("terminology.zh-Hant.md"),
            format!(
                "---\nsource: docs/naming.md\nlang: zh-Hant\nsource_commit: {}\n---\nprofile\n",
                &naming_hash[..7]
            ),
        )
        .unwrap();

        let report = run_translation_freshness(repo, &["README.md", "docs/manual.md"]);
        let row = report
            .rows
            .iter()
            .find(|row| row.doc == "docs/naming.md")
            .expect("terminology row present");
        assert_eq!(row.status, "current");
        assert_eq!(row.kind, ArtifactKind::Terminology);
        // The terminology profile itself is never reported missing against
        // the doc allowlist, nor unexpected for being off it — only the two
        // allowlisted docs (README.md, docs/manual.md) are missing here.
        assert_eq!(report.missing, 2);
        assert_eq!(report.unexpected, 0);

        // Advance docs/naming.md so the stamped commit goes stale.
        fs::write(repo.join("docs").join("naming.md"), "naming v2\n").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["add", "docs/naming.md"])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["commit", "-m", "advance naming.md", "-q"])
            .status()
            .unwrap();

        let stale_report = run_translation_freshness(repo, &["README.md", "docs/manual.md"]);
        let stale_row = stale_report
            .rows
            .iter()
            .find(|row| row.doc == "docs/naming.md")
            .expect("terminology row present after advance");
        assert_eq!(stale_row.status, "stale");
        assert_eq!(stale_row.kind, ArtifactKind::Terminology);
    }

    #[test]
    fn disguised_full_translation_of_naming_md_is_unexpected() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        let naming_hash = init_repo_with_source(repo, "docs/naming.md", "naming\n");

        fs::create_dir_all(repo.join("docs").join("i18n").join("zh-Hant")).unwrap();
        fs::write(
            repo.join("docs")
                .join("i18n")
                .join("zh-Hant")
                .join("naming.zh-Hant.md"),
            format!(
                "---\nsource: docs/naming.md\nlang: zh-Hant\nsource_commit: {}\n---\ntranslated\n",
                &naming_hash[..7]
            ),
        )
        .unwrap();

        let report = run_translation_freshness(repo, &["README.md", "docs/manual.md"]);
        let row = report
            .rows
            .iter()
            .find(|row| row.doc == "docs/naming.md" && row.kind == ArtifactKind::Translation)
            .expect("disguised translation row present");
        assert_eq!(row.status, "unexpected");
        assert_eq!(row.kind, ArtifactKind::Translation);
        assert_eq!(report.unexpected, 1);
        assert_eq!(report.current, 0);
    }

    #[test]
    fn unstamped_disguised_naming_translation_is_still_unexpected() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        init_repo_with_source(repo, "docs/naming.md", "naming\n");

        fs::create_dir_all(repo.join("docs").join("i18n").join("zh-Hant")).unwrap();
        fs::write(
            repo.join("docs")
                .join("i18n")
                .join("zh-Hant")
                .join("naming.zh-Hant.md"),
            "---\nsource: docs/naming.md\nlang: zh-Hant\nsource_commit: PENDING\n---\ntranslated\n",
        )
        .unwrap();

        let report = run_translation_freshness(repo, &["README.md", "docs/manual.md"]);
        let row = report
            .rows
            .iter()
            .find(|row| row.doc == "docs/naming.md" && row.kind == ArtifactKind::Translation)
            .expect("disguised translation row present");
        assert_eq!(row.status, "unexpected");
    }

    #[test]
    fn disguised_naming_translation_with_blank_lang_is_unexpected() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        let naming_hash = init_repo_with_source(repo, "docs/naming.md", "naming\n");

        fs::create_dir_all(repo.join("docs").join("i18n").join("zh-Hant")).unwrap();
        fs::write(
            repo.join("docs")
                .join("i18n")
                .join("zh-Hant")
                .join("naming.zh-Hant.md"),
            format!(
                "---\nsource: docs/naming.md\nlang: \nsource_commit: {}\n---\ntranslated\n",
                &naming_hash[..7]
            ),
        )
        .unwrap();

        let report = run_translation_freshness(repo, &["README.md", "docs/manual.md"]);
        let row = report
            .rows
            .iter()
            .find(|row| row.doc == "docs/naming.md" && row.kind == ArtifactKind::Translation)
            .expect("disguised translation row present");
        assert_eq!(row.status, "unexpected");
    }

    #[test]
    fn terminology_backfill_does_not_duplicate_allowlist_missing_row() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        init_repo_with_source(repo, "docs/naming.md", "naming\n");

        // No artifact declares docs/naming.md at all; the locale directory
        // exists so `zh-Hant` is a known language, but is otherwise empty.
        fs::create_dir_all(repo.join("docs").join("i18n").join("zh-Hant")).unwrap();

        let report = run_translation_freshness(repo, &["README.md", "docs/naming.md"]);
        let naming_missing_rows: Vec<_> = report
            .rows
            .iter()
            .filter(|row| row.doc == "docs/naming.md" && row.lang == "zh-Hant")
            .collect();
        assert_eq!(
            naming_missing_rows.len(),
            1,
            "expected exactly one missing row for (docs/naming.md, zh-Hant), got {naming_missing_rows:?}"
        );
    }
}
