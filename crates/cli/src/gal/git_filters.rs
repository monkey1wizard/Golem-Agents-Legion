//! `gal clean` / `gal smudge` — Git filter content transforms.

use gal_foundation::config::{machine_path_value, MACHINE_PATH_REGISTRY};
use regex::Regex;
use std::collections::BTreeMap;
use std::path::Path;

/// Returns a compiled regex that matches secret-like values in content.
///
/// Pattern: `(API_KEY|TOKEN|SECRET|PASSWORD|PAT)[:=] <non-whitespace-non-angle>+`
/// Covers common credential key suffixes including PAT (personal access token).
/// Shared between the git clean filter and `gal doctor` manifest scanning.
pub fn secret_re() -> Regex {
    Regex::new(r"(API_KEY|TOKEN|SECRET|PASSWORD|PAT)[[:space:]]*[:=][[:space:]]*[^<[:space:]]+")
        .unwrap()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterResult {
    pub output: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum FilterError {
    #[error("{0}")]
    Message(String),
}

pub fn run_clean(content: &str, _repo_root: &Path) -> Result<FilterResult, FilterError> {
    let primary_json = gal_foundation::paths::machine_config_path();
    run_clean_from_config(content, primary_json.as_deref())
}

pub fn run_smudge(content: &str, _repo_root: &Path) -> Result<FilterResult, FilterError> {
    let primary_json = gal_foundation::paths::machine_config_path();
    run_smudge_from_config(content, primary_json.as_deref())
}

/// Resolve filter values from `config.json` (the sole machine-config source).
pub fn run_clean_from_config(
    content: &str,
    primary_json: Option<&Path>,
) -> Result<FilterResult, FilterError> {
    let values = build_filter_values(primary_json);
    run_clean_inner(content, &values, primary_json)
}

/// Resolve filter values from `config.json` (the sole machine-config source).
pub fn run_smudge_from_config(
    content: &str,
    primary_json: Option<&Path>,
) -> Result<FilterResult, FilterError> {
    let values = build_filter_values(primary_json);
    run_smudge_inner(content, &values)
}

fn run_clean_inner(
    content: &str,
    values: &BTreeMap<String, String>,
    primary_label: Option<&Path>,
) -> Result<FilterResult, FilterError> {
    if values.is_empty() {
        let path_re = Regex::new(r"([A-Za-z]:[\\/]|/Users/|/home/|/Volumes/|\\\\)").unwrap();
        let secret_re = secret_re();
        if path_re.is_match(content) || secret_re.is_match(content) {
            let primary = primary_label
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            return Err(FilterError::Message(format!(
                "gal-clean: ERROR — no config source found at {primary}. Refusing to clean content that appears to contain machine-local paths or secret-like values."
            )));
        }

        return Ok(FilterResult {
            output: content.to_string(),
            warnings: vec!["gal-clean: WARN — no config source found; passing through placeholder-only content unchanged.".to_string()],
        });
    }

    let mut output = content.to_string();
    for (key, value) in sorted_values_desc(values) {
        let placeholder = format!("<{key}>");
        output = replace_at_non_identifier_boundaries(&output, &value, &placeholder);
    }

    Ok(FilterResult {
        output,
        warnings: Vec::new(),
    })
}

fn run_smudge_inner(
    content: &str,
    values: &BTreeMap<String, String>,
) -> Result<FilterResult, FilterError> {
    if values.is_empty() {
        return Ok(FilterResult {
            output: content.to_string(),
            warnings: Vec::new(),
        });
    }

    let mut output = content.to_string();
    for (key, value) in sorted_values_desc(values) {
        let placeholder = format!("<{key}>");
        output = replace_at_non_identifier_boundaries(&output, &placeholder, &value);
    }

    Ok(FilterResult {
        output,
        warnings: Vec::new(),
    })
}

/// Build filter values from `config.json` via `MACHINE_PATH_REGISTRY`.
fn build_filter_values(primary_json: Option<&Path>) -> BTreeMap<String, String> {
    primary_json
        .filter(|p| p.is_file())
        .map(read_filter_values_from_json)
        .unwrap_or_default()
}

/// Read machine path values from a `config.json` via `MACHINE_PATH_REGISTRY`.
/// Does NOT include `config.json#secrets` — those are credential placeholders, not path values.
fn read_filter_values_from_json(path: &Path) -> BTreeMap<String, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let Ok(root) = serde_json::from_str::<serde_json::Value>(&text) else {
        return BTreeMap::new();
    };
    MACHINE_PATH_REGISTRY
        .iter()
        .filter_map(|(config_path, snake_key)| {
            let value = machine_path_value(&root, config_path)?;
            if value.is_empty() {
                return None;
            }
            Some((snake_key.to_string(), value))
        })
        .collect()
}

fn sorted_values_desc(values: &BTreeMap<String, String>) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = values
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    pairs.sort_by(|left, right| {
        right
            .1
            .len()
            .cmp(&left.1.len())
            .then_with(|| left.0.cmp(&right.0))
    });
    pairs
}

fn replace_at_non_identifier_boundaries(content: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return content.to_string();
    }

    let mut output = String::with_capacity(content.len());
    let mut index = 0;

    while index < content.len() {
        if content[index..].starts_with(needle) {
            let end = index + needle.len();
            let prev = content[..index].chars().next_back();
            let next = content[end..].chars().next();

            if is_non_identifier_boundary(prev) && is_non_identifier_boundary(next) {
                output.push_str(replacement);
                index = end;
                continue;
            }
        }

        let ch = content[index..]
            .chars()
            .next()
            .expect("index always points to a valid char boundary");
        output.push(ch);
        index += ch.len_utf8();
    }

    output
}

fn is_non_identifier_boundary(ch: Option<char>) -> bool {
    ch.is_none_or(|value| !value.is_ascii_alphanumeric() && value != '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    // ── config.json filter API (run_clean_from_config / run_smudge_from_config) ─

    fn write_cfg(dir: &Path, body: &str) -> PathBuf {
        let cfg = dir.join("config.json");
        fs::write(&cfg, body).unwrap();
        cfg
    }

    #[test]
    fn smudge_resolves_registry_keys() {
        let temp = tempdir().unwrap();
        let cfg = write_cfg(temp.path(), r#"{"galSkills":"/home/test/skills"}"#);
        let result = run_smudge_from_config(
            "s=<GAL_SKILLS>
",
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(
            result.output,
            "s=/home/test/skills
"
        );
    }

    #[test]
    fn clean_replaces_values_with_placeholders() {
        let temp = tempdir().unwrap();
        let cfg = write_cfg(temp.path(), r#"{"galSkills":"/home/test/skills"}"#);
        let result = run_clean_from_config(
            "path=/home/test/skills/bin
",
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(
            result.output,
            "path=<GAL_SKILLS>/bin
"
        );
    }

    #[test]
    fn clean_replaces_longest_value_first() {
        let temp = tempdir().unwrap();
        let cfg = write_cfg(temp.path(), r#"{"galSkills":"/data/skills"}"#);
        let mut unordered = BTreeMap::new();
        unordered.insert("GAL_SKILLS_BIN".to_string(), "/data/skills/bin".to_string());
        unordered.insert("GAL_SKILLS".to_string(), "/data/skills".to_string());

        assert_eq!(
            sorted_values_desc(&unordered),
            vec![
                ("GAL_SKILLS_BIN".to_string(), "/data/skills/bin".to_string()),
                ("GAL_SKILLS".to_string(), "/data/skills".to_string()),
            ]
        );

        let result = run_clean_inner(
            "a /data/skills/bin b /data/skills c
",
            &unordered,
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(
            result.output,
            "a <GAL_SKILLS_BIN> b <GAL_SKILLS> c
"
        );
    }

    #[test]
    fn clean_replaces_at_non_identifier_boundaries() {
        let temp = tempdir().unwrap();
        let cfg = write_cfg(temp.path(), r#"{"galSkills":"/skills"}"#);
        let result = run_clean_from_config(
            "Use /skills/guide
",
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(
            result.output,
            "Use <GAL_SKILLS>/guide
"
        );
    }

    #[test]
    fn clean_does_not_replace_inside_identifier_boundaries() {
        let temp = tempdir().unwrap();
        let cfg = write_cfg(temp.path(), r#"{"galSkills":"repo"}"#);
        let result = run_clean_from_config(
            "repository repo
",
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(
            result.output,
            "repository <GAL_SKILLS>
"
        );
    }

    #[test]
    fn clean_and_smudge_round_trip() {
        let temp = tempdir().unwrap();
        let cfg = write_cfg(temp.path(), r#"{"galSkills":"/vault"}"#);
        let cleaned = run_clean_from_config(
            "Path=/vault
",
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(
            cleaned.output,
            "Path=<GAL_SKILLS>
"
        );
        assert!(!cleaned.output.contains("/vault"));
        let smudged = run_smudge_from_config(&cleaned.output, Some(&cfg)).unwrap();
        assert_eq!(
            smudged.output,
            "Path=/vault
"
        );
    }

    #[test]
    fn clean_does_not_replace_retired_keys() {
        let temp = tempdir().unwrap();
        let cfg = write_cfg(
            temp.path(),
            r#"{"obsidian":{"vault":"/vault"},"research":{"localSearchProject":"/search"},"tempDir":"/tmp/work","galSkills":"/skills"}"#,
        );
        let result = run_clean_from_config(
            "vault=/vault search=/search temp=/tmp/work skills=/skills
",
            Some(&cfg),
        )
        .unwrap();
        assert_eq!(
            result.output,
            "vault=/vault search=/search temp=/tmp/work skills=<GAL_SKILLS>
"
        );
    }

    #[test]
    fn secrets_in_config_json_not_used_as_path_values() {
        let temp = tempdir().unwrap();
        let cfg = write_cfg(
            temp.path(),
            r#"{"galSkills":"/vault","secrets":{"CONTEXT7_API_KEY":"sk-secret-abc"}}"#,
        );
        let result = run_smudge_from_config(
            "vault=<GAL_SKILLS> key=<CONTEXT7_API_KEY>
",
            Some(&cfg),
        )
        .unwrap();
        // GAL_SKILLS resolved; CONTEXT7_API_KEY placeholder unchanged (not a path value).
        assert_eq!(
            result.output,
            "vault=/vault key=<CONTEXT7_API_KEY>
"
        );
    }

    #[test]
    fn clean_refuses_suspicious_content_without_config() {
        // No config source (config.json absent) → refuse to clean
        // content that looks like it contains machine paths / secrets.
        let err = run_clean_from_config("secret=abc123\nPath=C:/Users/me\n", None).unwrap_err();
        assert!(err.to_string().contains("Refusing to clean content"));
    }

    #[test]
    fn clean_passthrough_without_config_for_placeholder_only_content() {
        let result = run_clean_from_config("<GAL_SKILLS>\n", None).unwrap();
        assert_eq!(result.output, "<GAL_SKILLS>\n");
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn smudge_leaves_placeholders_as_is_without_config() {
        let temp = tempdir().unwrap();
        let absent_cfg = temp.path().join("config.json");
        let result = run_smudge_from_config(
            "<GAL_SKILLS>
",
            Some(&absent_cfg),
        )
        .unwrap();
        assert_eq!(
            result.output,
            "<GAL_SKILLS>
"
        );
    }
}
