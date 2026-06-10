//! `gal translation-freshness` — report translation freshness for docs/i18n.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRow {
    pub doc: String,
    pub lang: String,
    pub status: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslationReport {
    pub rows: Vec<TranslationRow>,
    pub current: usize,
    pub stale: usize,
    pub missing: usize,
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
            });
            continue;
        };
        let lang = frontmatter.get("lang").cloned().unwrap_or_default();
        let stamp = frontmatter.get("source_commit").cloned().unwrap_or_default();
        seen.insert((source.clone(), lang.clone()));

        let source_path = repo_root.join(&source);
        if !source_path.is_file() {
            rows.push(TranslationRow {
                doc: source,
                lang,
                status: "stale".to_string(),
                note: "source missing".to_string(),
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
            rows.push(TranslationRow {
                doc: source,
                lang,
                status: "current".to_string(),
                note: String::new(),
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
            });
        }
    }

    for src in allowlist {
        for lang in &languages {
            if !seen.contains(&(src.to_string(), lang.clone())) {
                rows.push(TranslationRow {
                    doc: src.to_string(),
                    lang: lang.clone(),
                    status: "missing".to_string(),
                    note: "allowlisted, no translation".to_string(),
                });
            }
        }
    }

    rows.sort_by(|left, right| left.doc.cmp(&right.doc).then_with(|| left.lang.cmp(&right.lang)));

    let mut report = TranslationReport::default();
    for row in rows {
        match row.status.as_str() {
            "current" => report.current += 1,
            "stale" => report.stale += 1,
            "missing" => report.missing += 1,
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
    if value.is_empty() { None } else { Some(value) }
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

        Command::new("git").arg("-C").arg(repo).args(["init", "-q"]).status().unwrap();
        Command::new("git").arg("-C").arg(repo).args(["config", "user.email", "test@example.com"]).status().unwrap();
        Command::new("git").arg("-C").arg(repo).args(["config", "user.name", "Test User"]).status().unwrap();
        Command::new("git").arg("-C").arg(repo).args(["add", "README.md", "docs/manual.md"]).status().unwrap();
        Command::new("git").arg("-C").arg(repo).args(["commit", "-m", "seed", "-q"]).status().unwrap();

        let readme_hash = latest_commit(repo, "README.md").unwrap();
        fs::write(
            repo.join("docs").join("i18n").join("zh-Hant").join("README.zh-Hant.md"),
            format!("---\nsource: README.md\nlang: zh-Hant\nsource_commit: {}\n---\ntranslated\n", &readme_hash[..7]),
        ).unwrap();
        fs::write(
            repo.join("docs").join("i18n").join("zh-Hant").join("manual.zh-Hant.md"),
            "---\nsource: docs/manual.md\nlang: zh-Hant\nsource_commit: PENDING\n---\ntranslated\n",
        ).unwrap();

        let report = run_translation_freshness(repo, &["README.md", "docs/manual.md", "docs/missing.md"]);
        assert_eq!(report.current, 1);
        assert_eq!(report.stale, 1);
        assert_eq!(report.missing, 1);
    }
}