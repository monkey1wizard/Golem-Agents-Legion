//! Environment file and configuration-value utilities.
//!
//! Ports `Read-KeyValueEnvFile`, `Get-ConfiguredValue`, and `Split-ConfigList`
//! from `scripts/common/Common.{ps1,sh}`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Parse a `KEY=VALUE` env file into a sorted map.
///
/// Lines starting with `#` are skipped (comments). Lines without `=` are
/// skipped. Keys and values are whitespace-trimmed. Entries with an empty key
/// or empty value after trimming are skipped. When the file does not exist the
/// function returns an empty map without error.
///
/// Ports `Read-KeyValueEnvFile` from Common.ps1.
pub fn read_key_value_env(path: &Path) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return values,
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || !trimmed.contains('=') {
            continue;
        }
        // Split on first `=` only so values may contain `=` (e.g. URLs).
        let (key, value) = trimmed.split_once('=').unwrap();
        let key = key.trim().to_string();
        let value = value.trim().to_string();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        values.insert(key, value);
    }

    values
}

/// Look up a value by name — file map first, then `std::env::var`.
///
/// Returns `None` when the name is not found or the resolved value is
/// whitespace-only. Ports `Get-ConfiguredValue` from Common.ps1.
pub fn get_configured_value(values: &BTreeMap<String, String>, name: &str) -> Option<String> {
    if let Some(v) = values.get(name) {
        let v = v.trim();
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    let env_val = std::env::var(name).ok()?;
    let env_val = env_val.trim().to_string();
    if env_val.is_empty() {
        None
    } else {
        Some(env_val)
    }
}

/// Split a comma-separated config list into trimmed non-empty items.
///
/// Ports `Split-ConfigList` from Common.ps1.
pub fn split_config_list(value: &str) -> Vec<String> {
    if value.trim().is_empty() {
        return vec![];
    }
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use tempfile::NamedTempFile;

    // ── read_key_value_env ────────────────────────────────────────────────────

    #[test]
    fn missing_file_returns_empty_map() {
        let m = read_key_value_env(Path::new("/nonexistent/t010-env.cfg"));
        assert!(m.is_empty());
    }

    #[test]
    fn comment_lines_are_skipped() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "# this is a comment").unwrap();
        writeln!(f, "KEY=VALUE").unwrap();
        let m = read_key_value_env(f.path());
        assert_eq!(m.len(), 1);
        assert_eq!(m["KEY"], "VALUE");
    }

    #[test]
    fn lines_without_equals_are_skipped() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "NOEQUALS").unwrap();
        writeln!(f, "KEY=val").unwrap();
        let m = read_key_value_env(f.path());
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn key_and_value_are_trimmed() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "  KEY  =  val  ").unwrap();
        let m = read_key_value_env(f.path());
        assert_eq!(m["KEY"], "val");
    }

    #[test]
    fn value_may_contain_equals_sign() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "URL=https://example.com?a=1&b=2").unwrap();
        let m = read_key_value_env(f.path());
        assert_eq!(m["URL"], "https://example.com?a=1&b=2");
    }

    #[test]
    fn empty_value_is_skipped() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "EMPTY=").unwrap();
        writeln!(f, "KEY=val").unwrap();
        let m = read_key_value_env(f.path());
        assert!(!m.contains_key("EMPTY"));
        assert!(m.contains_key("KEY"));
    }

    #[test]
    fn multiple_entries_parsed_correctly() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "A=1").unwrap();
        writeln!(f, "B=2").unwrap();
        writeln!(f, "C=3").unwrap();
        let m = read_key_value_env(f.path());
        assert_eq!(m.len(), 3);
        assert_eq!(m["A"], "1");
        assert_eq!(m["B"], "2");
        assert_eq!(m["C"], "3");
    }

    // ── get_configured_value ──────────────────────────────────────────────────

    #[test]
    fn returns_map_value_when_present() {
        let mut m = BTreeMap::new();
        m.insert("MY_KEY".to_string(), "from_map".to_string());
        assert_eq!(
            get_configured_value(&m, "MY_KEY").as_deref(),
            Some("from_map")
        );
    }

    #[test]
    fn returns_none_for_key_not_in_map_or_env() {
        // Use a key that is very unlikely to be set in any test environment.
        let key = "GAL_T010_UNLIKELY_TEST_VAR_X9Z";
        std::env::remove_var(key);
        let m: BTreeMap<String, String> = BTreeMap::new();
        assert_eq!(get_configured_value(&m, key), None);
    }

    #[test]
    fn returns_none_for_whitespace_only_map_value_and_missing_env() {
        let key = "GAL_T010_BLANK_VALUE_TEST";
        std::env::remove_var(key);
        let mut m = BTreeMap::new();
        m.insert(key.to_string(), "   ".to_string());
        assert_eq!(get_configured_value(&m, key), None);
    }

    // ── split_config_list ─────────────────────────────────────────────────────

    #[test]
    fn empty_string_returns_empty_vec() {
        assert!(split_config_list("").is_empty());
    }

    #[test]
    fn whitespace_only_returns_empty_vec() {
        assert!(split_config_list("   ").is_empty());
    }

    #[test]
    fn single_item_returns_one_element() {
        assert_eq!(split_config_list("copilot"), vec!["copilot"]);
    }

    #[test]
    fn multiple_items_split_on_comma() {
        let result = split_config_list("copilot, claude, codex");
        assert_eq!(result, vec!["copilot", "claude", "codex"]);
    }

    #[test]
    fn items_are_trimmed() {
        let result = split_config_list("  a  ,  b  ,  c  ");
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn empty_segments_are_skipped() {
        let result = split_config_list("a,,b");
        assert_eq!(result, vec!["a", "b"]);
    }
}
