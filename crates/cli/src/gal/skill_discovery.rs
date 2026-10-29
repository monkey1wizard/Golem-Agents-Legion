//! Installed-skill discovery scanner (gal tree).
//!
//! Enumerates `SKILL.md` files one level under each injected skill root
//! (Claude plugin cache, shared agents skills, Copilot skills, agy skills)
//! and parses each file's YAML frontmatter `name`/`description`. Feeds
//! source 3 (Detected Language Skills) in the repo-adapter render. Read-only:
//! this scanner never writes to any skill root.

use std::fs;
use std::path::{Path, PathBuf};

/// A skill discovered under one of the scanned roots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSkill {
    pub name: String,
    pub origin: PathBuf,
    pub description: String,
}

/// Scan every root for `<skill-dir>/SKILL.md` (one level deep) and return
/// parsed entries sorted by name, then origin. Missing or unreadable roots,
/// and malformed entries (no parseable frontmatter `name`), are skipped
/// without panicking.
pub fn discover_skills(roots: &[PathBuf]) -> Vec<DiscoveredSkill> {
    let mut skills = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(skill) = read_skill_entry(&path) {
                skills.push(skill);
            }
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.origin.cmp(&b.origin)));
    skills
}

/// Expand the Claude plugin cache root (`~/.claude/plugins/cache`) into one
/// `skills/` root per installed plugin version
/// (`<cache_root>/<plugin>/<version>/skills`) — Claude's on-disk plugin cache
/// layout, not the flat one-level shape of the other roots. Scans **every**
/// installed plugin, not just GAL's own (`cache/gal`). Missing or unreadable
/// directories are skipped without panicking.
pub fn claude_plugin_skill_roots(cache_root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let Ok(plugin_entries) = fs::read_dir(cache_root) else {
        return roots;
    };
    for plugin_entry in plugin_entries.filter_map(|e| e.ok()) {
        let plugin_path = plugin_entry.path();
        if !plugin_path.is_dir() {
            continue;
        }
        let Ok(version_entries) = fs::read_dir(&plugin_path) else {
            continue;
        };
        for version_entry in version_entries.filter_map(|e| e.ok()) {
            let version_path = version_entry.path();
            if version_path.is_dir() {
                roots.push(version_path.join("skills"));
            }
        }
    }
    roots
}

fn read_skill_entry(skill_dir: &Path) -> Option<DiscoveredSkill> {
    let skill_md = skill_dir.join("SKILL.md");
    let content = fs::read_to_string(&skill_md).ok()?;
    let (name, description) = parse_frontmatter(&content)?;
    Some(DiscoveredSkill {
        name,
        origin: skill_dir.to_path_buf(),
        description,
    })
}

/// Parse a minimal subset of YAML frontmatter (`---` fenced) for `name:` and
/// `description:` scalar values. Returns `None` when there is no frontmatter
/// fence or no non-empty `name` value.
fn parse_frontmatter(content: &str) -> Option<(String, String)> {
    let trimmed = content.trim_start();
    let rest = trimmed.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let frontmatter = &rest[..end];

    let mut name = None;
    let mut description = String::new();
    for line in frontmatter.lines() {
        if let Some(value) = line.strip_prefix("name:") {
            name = Some(unquote(value.trim()));
        } else if let Some(value) = line.strip_prefix("description:") {
            description = unquote(value.trim());
        }
    }

    let name = name?;
    if name.is_empty() {
        return None;
    }
    Some((name, description))
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_file(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn discovers_full_sorted_list_from_two_roots() {
        let root_a = TempDir::new().unwrap();
        let root_b = TempDir::new().unwrap();

        write_file(
            &root_a.path().join("zeta-skill").join("SKILL.md"),
            "---\nname: zeta-skill\ndescription: Zeta helper\n---\nBody\n",
        );
        write_file(
            &root_b.path().join("alpha-skill").join("SKILL.md"),
            "---\nname: alpha-skill\ndescription: Alpha helper\n---\nBody\n",
        );

        let skills = discover_skills(&[root_a.path().to_path_buf(), root_b.path().to_path_buf()]);
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].name, "alpha-skill");
        assert_eq!(skills[0].description, "Alpha helper");
        assert_eq!(skills[1].name, "zeta-skill");
    }

    #[test]
    fn missing_and_unreadable_roots_skipped_without_panic() {
        let missing_root = std::env::temp_dir().join("gal-skill-discovery-does-not-exist");
        let skills = discover_skills(&[missing_root]);
        assert!(skills.is_empty());
    }

    #[test]
    fn malformed_entries_skipped_without_panic() {
        let root = TempDir::new().unwrap();
        // No frontmatter fence at all.
        write_file(
            &root.path().join("broken-skill").join("SKILL.md"),
            "# Not frontmatter\n\nJust prose.\n",
        );
        // Frontmatter fence present but no name key.
        write_file(
            &root.path().join("nameless-skill").join("SKILL.md"),
            "---\ndescription: No name here\n---\nBody\n",
        );
        // A dir with no SKILL.md at all.
        write_file(&root.path().join("not-a-skill").join("README.md"), "hi\n");

        let skills = discover_skills(&[root.path().to_path_buf()]);
        assert!(
            skills.is_empty(),
            "malformed/missing entries must be skipped, got {skills:?}"
        );
    }

    #[test]
    fn empty_roots_list_returns_empty() {
        let skills = discover_skills(&[]);
        assert!(skills.is_empty());
    }

    #[test]
    fn claude_cache_scans_non_gal_plugins_not_only_cache_gal() {
        let cache_root = TempDir::new().unwrap();

        // GAL's own plugin.
        write_file(
            &cache_root
                .path()
                .join("gal")
                .join("1.0.0")
                .join("skills")
                .join("doc-sync")
                .join("SKILL.md"),
            "---\nname: doc-sync\ndescription: GAL doc sync\n---\nBody\n",
        );
        // A non-GAL, officially-installed plugin (e.g. a Flutter/Dart plugin).
        write_file(
            &cache_root
                .path()
                .join("flutter-official")
                .join("2.3.0")
                .join("skills")
                .join("flutter-style")
                .join("SKILL.md"),
            "---\nname: flutter-style\ndescription: Official Flutter style guide\n---\nBody\n",
        );

        let roots = claude_plugin_skill_roots(cache_root.path());
        assert_eq!(
            roots.len(),
            2,
            "must expand into one skills/ root per plugin version, got {roots:?}"
        );

        let skills = discover_skills(&roots);
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"flutter-style"),
            "scan must reach a non-GAL plugin, not only cache/gal, got {names:?}"
        );
        assert!(names.contains(&"doc-sync"));
    }
}
