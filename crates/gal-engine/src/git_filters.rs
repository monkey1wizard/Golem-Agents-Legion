//! `gal clean` / `gal smudge` — Git filter content transforms.

use base::env_config::read_key_value_env;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FILTER_KEYS: &[&str] = &[
    "OBSIDIAN_VAULT",
    "OBSIDIAN_VAULT_NAME",
    "OBSIDIAN_GUIDE_PATH",
    "OBSIDIAN_PRIVATE_RESEARCH_DIR",
    "OBSIDIAN_DIARY_DIR",
    "OBSIDIAN_SCRATCH_DIR",
    "OBSIDIAN_ARCHIVE_DIR",
    "LOCAL_SEARCH_PROJECT",
    "GAL_SKILLS",
    "TEMP_DIR",
    "MCP_FILESYSTEM_PATHS",
];

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

pub fn run_clean(content: &str, repo_root: &Path) -> Result<FilterResult, FilterError> {
    let primary = base::paths::gal_home().map(|h| h.join("config").join("config.local.env"));
    let legacy = repo_root.join("config.local.env");
    run_clean_with_paths(content, primary.as_deref(), Some(&legacy))
}

pub fn run_smudge(content: &str, repo_root: &Path) -> Result<FilterResult, FilterError> {
    let primary = base::paths::gal_home().map(|h| h.join("config").join("config.local.env"));
    let legacy = repo_root.join("config.local.env");
    run_smudge_with_paths(content, primary.as_deref(), Some(&legacy))
}

pub fn run_clean_with_paths(
    content: &str,
    primary_config: Option<&Path>,
    legacy_config: Option<&Path>,
) -> Result<FilterResult, FilterError> {
    let config = select_config(primary_config, legacy_config);
    let values = config
        .as_ref()
        .map(|path| read_filter_values(path.as_path()))
        .unwrap_or_default();

    if values.is_empty() {
        let path_re = Regex::new(r"([A-Za-z]:[\\/]|/Users/|/home/|/Volumes/|\\\\)").unwrap();
        let secret_re = Regex::new(r"(API_KEY|TOKEN|SECRET|PASSWORD)[[:space:]]*[:=][[:space:]]*[^<[:space:]]+").unwrap();
        if path_re.is_match(content) || secret_re.is_match(content) {
            let primary = primary_config
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            let legacy = legacy_config
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            return Err(FilterError::Message(format!(
                "gal-clean: ERROR — no config source found at {primary} or legacy fallback {legacy}. Refusing to clean content that appears to contain machine-local paths or secret-like values."
            )));
        }

        return Ok(FilterResult {
            output: content.to_string(),
            warnings: vec!["gal-clean: WARN — no config source found; passing through placeholder-only content unchanged.".to_string()],
        });
    }

    let mut output = content.to_string();
    for (key, value) in sorted_values_desc(&values) {
        let placeholder = format!("<{key}>");
        output = output.replace(&value, &placeholder);
    }

    Ok(FilterResult {
        output,
        warnings: Vec::new(),
    })
}

pub fn run_smudge_with_paths(
    content: &str,
    primary_config: Option<&Path>,
    legacy_config: Option<&Path>,
) -> Result<FilterResult, FilterError> {
    let config = select_config(primary_config, legacy_config);
    let values = config
        .as_ref()
        .map(|path| read_filter_values(path.as_path()))
        .unwrap_or_default();

    if values.is_empty() {
        return Ok(FilterResult {
            output: content.to_string(),
            warnings: Vec::new(),
        });
    }

    let mut output = content.to_string();
    for (key, value) in sorted_values_desc(&values) {
        let placeholder = format!("<{key}>");
        output = output.replace(&placeholder, &value);
    }

    Ok(FilterResult {
        output,
        warnings: Vec::new(),
    })
}

fn select_config(primary_config: Option<&Path>, legacy_config: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = primary_config.filter(|p| p.is_file()) {
        return Some(path.to_path_buf());
    }
    legacy_config.filter(|p| p.is_file()).map(PathBuf::from)
}

fn read_filter_values(path: &Path) -> BTreeMap<String, String> {
    read_key_value_env(path)
        .into_iter()
        .filter(|(key, value)| FILTER_KEYS.contains(&key.as_str()) && !value.is_empty())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn smudge_replaces_placeholders_from_primary_config() {
        let temp = tempdir().unwrap();
        let primary = temp.path().join("config.local.env");
        fs::write(&primary, "OBSIDIAN_VAULT=/vault\nTEMP_DIR=/tmp/work\n").unwrap();

        let result = run_smudge_with_paths(
            "Path=<OBSIDIAN_VAULT>; Temp=<TEMP_DIR>\n",
            Some(&primary),
            None,
        )
        .unwrap();

        assert_eq!(result.output, "Path=/vault; Temp=/tmp/work\n");
    }

    #[test]
    fn clean_replaces_values_with_placeholders_longest_first() {
        let temp = tempdir().unwrap();
        let primary = temp.path().join("config.local.env");
        fs::write(
            &primary,
            "OBSIDIAN_VAULT=/vault\nOBSIDIAN_GUIDE_PATH=/vault/guide.md\n",
        )
        .unwrap();

        let result = run_clean_with_paths(
            "Use /vault/guide.md inside /vault\n",
            Some(&primary),
            None,
        )
        .unwrap();

        assert_eq!(
            result.output,
            "Use <OBSIDIAN_GUIDE_PATH> inside <OBSIDIAN_VAULT>\n"
        );
    }

    #[test]
    fn smudge_falls_back_to_legacy_repo_config() {
        let temp = tempdir().unwrap();
        let legacy = temp.path().join("config.local.env");
        fs::write(&legacy, "LOCAL_SEARCH_PROJECT=C:/Code/Proj\n").unwrap();

        let result = run_smudge_with_paths("<LOCAL_SEARCH_PROJECT>\n", None, Some(&legacy)).unwrap();
        assert_eq!(result.output, "C:/Code/Proj\n");
    }

    #[test]
    fn clean_refuses_suspicious_content_without_config() {
        let err = run_clean_with_paths("secret=abc123\nPath=C:/Users/me\n", None, None).unwrap_err();
        assert!(err.to_string().contains("Refusing to clean content"));
    }

    #[test]
    fn clean_passthrough_without_config_for_placeholder_only_content() {
        let result = run_clean_with_paths("<OBSIDIAN_VAULT>\n", None, None).unwrap();
        assert_eq!(result.output, "<OBSIDIAN_VAULT>\n");
        assert_eq!(result.warnings.len(), 1);
    }
}