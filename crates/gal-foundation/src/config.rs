//! Configuration parsing and management for GAL.
//!
//! Rust owns config truth. The config file is `~/.gal/config/config.json`.
//! Missing file or keys produce explicit defaults; no panics.

use serde_json::Value;

// ── Machine path registry ─────────────────────────────────────────────────────

/// Maps retained machine-local path keys in `config.json` to UPPER_SNAKE
/// placeholder names.
///
/// The dot-path side is the authoritative `config.json` key; the UPPER_SNAKE
/// side is the placeholder name used in git filters and projections.
///
/// Only `galSkills` remains: it is the lone machine-local absolute path still
/// worth path substitution after the obsidian/research/tempDir retirement.
/// Credential keys (`*_API_KEY`, `*_TOKEN`, etc.) live in `config.json#secrets`
/// and are never path-substituted.
pub const MACHINE_PATH_REGISTRY: &[(&str, &str)] = &[("galSkills", "GAL_SKILLS")];

/// Look up a machine path value from a `config.json` root `serde_json::Value`.
///
/// `config_path` is a dot-separated key path (e.g. `"galSkills"`).
/// Returns the string value when present, `None` when absent or not a string.
pub fn machine_path_value(config: &Value, config_path: &str) -> Option<String> {
    let mut current = config;
    for segment in config_path.split('.') {
        current = current.get(segment)?;
    }
    current.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── MACHINE_PATH_REGISTRY + machine_path_value ───────────────────────────

    #[test]
    fn registry_contains_only_gal_skills() {
        assert_eq!(MACHINE_PATH_REGISTRY, &[("galSkills", "GAL_SKILLS")]);
    }

    #[test]
    fn registry_excludes_retired_keys() {
        let keys: Vec<&str> = MACHINE_PATH_REGISTRY.iter().map(|(cp, _)| *cp).collect();
        assert!(!keys.iter().any(|key| key.starts_with("obsidian.")));
        assert!(!keys.contains(&"research.localSearchProject"));
        assert!(!keys.contains(&"research.privateResearchDir"));
        assert!(!keys.contains(&"tempDir"));
    }

    #[test]
    fn registry_excludes_credential_keys() {
        for (_, snake) in MACHINE_PATH_REGISTRY {
            assert!(
                !snake.ends_with("_API_KEY") && !snake.ends_with("_TOKEN"),
                "credential key found in registry: {snake}"
            );
        }
    }

    #[test]
    fn registry_snake_for_gal_skills() {
        let found = MACHINE_PATH_REGISTRY
            .iter()
            .find(|(cp, _)| *cp == "galSkills");
        assert_eq!(found.map(|(_, s)| *s), Some("GAL_SKILLS"));
    }

    #[test]
    fn machine_path_value_flat_key() {
        let config = serde_json::json!({ "galSkills": "/home/user/.gal/skills" });
        assert_eq!(
            machine_path_value(&config, "galSkills"),
            Some("/home/user/.gal/skills".to_owned())
        );
    }

    #[test]
    fn machine_path_value_gal_skills() {
        let config = serde_json::json!({
            "galSkills": "/Users/user/.gal/skills"
        });
        assert_eq!(
            machine_path_value(&config, "galSkills"),
            Some("/Users/user/.gal/skills".to_owned())
        );
    }

    #[test]
    fn machine_path_value_missing_key_returns_none() {
        let config = serde_json::json!({});
        assert_eq!(machine_path_value(&config, "galSkills"), None);
    }

    #[test]
    fn machine_path_value_non_string_returns_none() {
        let config = serde_json::json!({ "galSkills": 42 });
        assert_eq!(machine_path_value(&config, "galSkills"), None);
    }
}
