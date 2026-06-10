//! `gal resolve-catalog` — deterministic plugin resolution and lockfile output.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogResolution {
    pub profile_name: String,
    pub resolved_plugins: Vec<Value>,
    pub errors: Vec<String>,
    pub drift_detected: bool,
    pub drift_details: Vec<String>,
    pub lockfile: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveCatalogOptions {
    pub catalog_path: PathBuf,
    pub config_path: PathBuf,
    pub lockfile_path: PathBuf,
    pub dry_run: bool,
}

pub fn run_resolve_catalog(opts: &ResolveCatalogOptions) -> Result<CatalogResolution, String> {
    let catalog = read_json_file(&opts.catalog_path)?
        .ok_or_else(|| format!("Catalog not found at: {}", opts.catalog_path.display()))?;
    let config = read_json_file(&opts.config_path)?.unwrap_or_else(|| json!({}));

    let resolution = resolve_plugin_set(&catalog, &config, &opts.lockfile_path)?;
    if !opts.dry_run {
        write_json_file(&opts.lockfile_path, &resolution.lockfile)?;
    }
    Ok(resolution)
}

fn read_json_file(path: &Path) -> Result<Option<Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|e| format!("Failed to parse {}: {e}", path.display()))
}

fn write_json_file(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
    }
    fs::write(path, serde_json::to_string_pretty(value).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

fn resolve_plugin_set(catalog: &Value, config: &Value, lockfile_path: &Path) -> Result<CatalogResolution, String> {
    let mut errors = Vec::new();
    let plugins = catalog
        .get("plugins")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "catalog.plugins missing or invalid".to_string())?;
    let profiles = catalog
        .get("profiles")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| "catalog.profiles missing or invalid".to_string())?;

    for plugin in &plugins {
        validate_catalog_entry(plugin, &mut errors);
    }

    let requested_profile = config
        .get("defaultProfile")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let profile_name = if profiles.contains_key(requested_profile) {
        requested_profile.to_string()
    } else {
        errors.push(format!(
            "Profile '{}' not found in catalog. Available: {}",
            requested_profile,
            profiles.keys().cloned().collect::<Vec<_>>().join(", ")
        ));
        "default".to_string()
    };

    let profile = profiles
        .get(&profile_name)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("catalog profile '{}' missing", profile_name))?;

    let mut resolved_ids: Vec<String> = profile
        .get("plugins")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();

    if let Some(enabled) = config.get("enabledPlugins").and_then(Value::as_array) {
        for plugin_id in enabled.iter().filter_map(Value::as_str) {
            if !resolved_ids.contains(&plugin_id.to_string()) {
                resolved_ids.push(plugin_id.to_string());
            }
        }
    }

    if let Some(disabled) = config.get("disabledPlugins").and_then(Value::as_array) {
        resolved_ids.retain(|plugin_id| !disabled.iter().filter_map(Value::as_str).any(|disabled_id| disabled_id == plugin_id));
    }

    let mut resolved_plugins = Vec::new();
    for plugin_id in &resolved_ids {
        let Some(plugin) = plugins
            .iter()
            .find(|plugin| plugin.get("pluginId").and_then(Value::as_str) == Some(plugin_id.as_str()))
        else {
            errors.push(format!("Resolved plugin '{}' not found in catalog", plugin_id));
            continue;
        };

        let resolved_by_profile = profile
            .get("plugins")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|value| value == plugin_id);
        let resolved_by_explicit = config
            .get("enabledPlugins")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|value| value == plugin_id);

        resolved_plugins.push(json!({
            "pluginId": plugin.get("pluginId").cloned().unwrap_or(Value::Null),
            "displayName": plugin.get("displayName").cloned().unwrap_or(Value::Null),
            "supportTier": plugin.get("supportTier").cloned().unwrap_or(Value::Null),
            "sourceType": plugin.get("sourceType").cloned().unwrap_or(Value::Null),
            "upstream": plugin.get("upstream").cloned().unwrap_or(Value::Null),
            "license": plugin.get("license").cloned().unwrap_or(Value::Null),
            "checksumPolicy": plugin.get("checksumPolicy").cloned().unwrap_or(Value::Null),
            "componentMap": plugin.get("componentMap").cloned().unwrap_or(Value::Null),
            "supportedProviders": plugin.get("supportedProviders").cloned().unwrap_or(Value::Null),
            "installStrategy": plugin.get("installStrategy").cloned().unwrap_or(Value::Null),
            "resolvedByProfile": resolved_by_profile,
            "resolvedByExplicit": resolved_by_explicit,
        }));
    }

    let mut drift_detected = false;
    let mut drift_details = Vec::new();
    if let Some(existing_lock) = read_json_file(lockfile_path)? {
        if let Some(existing_plugins) = existing_lock.get("resolvedPlugins").and_then(Value::as_array) {
            let existing_ids: Vec<String> = existing_plugins
                .iter()
                .filter_map(|plugin| plugin.get("pluginId").and_then(Value::as_str))
                .map(str::to_string)
                .collect();
            if existing_ids != resolved_ids {
                drift_detected = true;
                drift_details.push(format!(
                    "Resolved plugin set changed: was [{}], now [{}]",
                    existing_ids.join(", "),
                    resolved_ids.join(", ")
                ));
            }
        }
    }

    let lockfile = build_lockfile(catalog, &resolved_plugins, &profile_name)?;
    Ok(CatalogResolution {
        profile_name,
        resolved_plugins,
        errors,
        drift_detected,
        drift_details,
        lockfile,
    })
}

fn validate_catalog_entry(plugin: &Value, errors: &mut Vec<String>) {
    let plugin_id = plugin.get("pluginId").and_then(Value::as_str).unwrap_or("<unknown>");
    for field in [
        "pluginId",
        "displayName",
        "supportTier",
        "sourceType",
        "upstream",
        "license",
        "checksumPolicy",
        "componentMap",
        "supportedProviders",
        "installStrategy",
        "defaultProfiles",
        "allowAutoUpdate",
        "localOverridePolicy",
    ] {
        if plugin.get(field).is_none() {
            errors.push(format!("[{plugin_id}] Missing required field: {field}"));
        }
    }

    let source_type = plugin.get("sourceType").and_then(Value::as_str).unwrap_or_default();
    if matches!(source_type, "curated-upstream" | "mirrored" | "forked") {
        let upstream = plugin.get("upstream").and_then(Value::as_object);
        if upstream.and_then(|map| map.get("repo")).and_then(Value::as_str).is_none() {
            errors.push(format!("[{plugin_id}] Upstream repo is required for sourceType={source_type}"));
        }
        if plugin.get("license").and_then(Value::as_str).map(str::trim).unwrap_or_default().is_empty() {
            errors.push(format!("[{plugin_id}] License is required for sourceType={source_type}"));
        }
        if plugin.get("checksumPolicy").and_then(Value::as_str).map(str::trim).unwrap_or_default().is_empty() {
            errors.push(format!("[{plugin_id}] checksumPolicy is required for sourceType={source_type}"));
        }
        if upstream.and_then(|map| map.get("ref")).and_then(Value::as_str).is_none() {
            errors.push(format!("[{plugin_id}] Upstream ref is required for sourceType={source_type}"));
        }
    }

    let valid_tiers = ["official-gal", "curated-upstream", "mirrored", "forked", "local"];
    let tier = plugin.get("supportTier").and_then(Value::as_str).unwrap_or_default();
    if !valid_tiers.contains(&tier) {
        errors.push(format!(
            "[{plugin_id}] Invalid supportTier: {tier}. Must be one of: {}",
            valid_tiers.join(", ")
        ));
    }
}

fn build_lockfile(catalog: &Value, resolved_plugins: &[Value], profile_name: &str) -> Result<Value, String> {
    let catalog_json = serde_json::to_string(&catalog).map_err(|e| e.to_string())?;
    let catalog_hash = hash_string(&catalog_json);
    let generated_at = chrono::Utc::now().to_rfc3339();

    let resolved_plugins: Vec<Value> = resolved_plugins
        .iter()
        .map(|plugin| {
            json!({
                "pluginId": plugin.get("pluginId").cloned().unwrap_or(Value::Null),
                "displayName": plugin.get("displayName").cloned().unwrap_or(Value::Null),
                "supportTier": plugin.get("supportTier").cloned().unwrap_or(Value::Null),
                "sourceType": plugin.get("sourceType").cloned().unwrap_or(Value::Null),
                "resolvedSource": plugin.get("upstream").and_then(|u| u.get("repo")).cloned().unwrap_or(Value::Null),
                "resolvedRef": plugin.get("upstream").and_then(|u| u.get("ref")).cloned().unwrap_or(Value::Null),
                "resolvedVersion": Value::Null,
                "resolvedChecksum": Value::Null,
                "resolvedLicense": plugin.get("license").cloned().unwrap_or(Value::Null),
                "resolvedComponentMap": plugin.get("componentMap").cloned().unwrap_or(Value::Null),
                "selectedProviders": plugin.get("supportedProviders").cloned().unwrap_or(Value::Null),
                "installStrategy": plugin.get("installStrategy").cloned().unwrap_or(Value::Null),
                "resolvedByProfile": plugin.get("resolvedByProfile").cloned().unwrap_or(Value::Bool(false)),
                "resolvedByExplicit": plugin.get("resolvedByExplicit").cloned().unwrap_or(Value::Bool(false)),
            })
        })
        .collect();

    Ok(json!({
        "schemaVersion": 1,
        "generatedAt": generated_at,
        "resolverVersion": "gal-resolver-1.0",
        "profileName": profile_name,
        "catalogHash": catalog_hash,
        "resolvedPlugins": resolved_plugins,
        "driftDetection": {
            "lastResolvedAt": generated_at,
            "catalogHash": catalog_hash,
        }
    }))
}

fn hash_string(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{:02X}", byte)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_json(path: &Path, value: &Value) {
        fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
    }

    #[test]
    fn default_profile_resolves_only_gal_core() {
        let temp = TempDir::new().unwrap();
        let catalog_path = temp.path().join("catalog.json");
        let config_path = temp.path().join("config.json");
        let lockfile_path = temp.path().join("lock.json");
        fs::write(&catalog_path, include_str!("../../../plugins/catalog.json")).unwrap();
        write_json(&config_path, &json!({"defaultProfile": "default"}));

        let result = run_resolve_catalog(&ResolveCatalogOptions {
            catalog_path,
            config_path,
            lockfile_path,
            dry_run: true,
        }).unwrap();

        let ids: Vec<&str> = result
            .resolved_plugins
            .iter()
            .filter_map(|plugin| plugin.get("pluginId").and_then(Value::as_str))
            .collect();
        assert_eq!(ids, vec!["gal-core"]);
    }

    #[test]
    fn dart_profile_resolves_companion_plugin() {
        let temp = TempDir::new().unwrap();
        let catalog_path = temp.path().join("catalog.json");
        let config_path = temp.path().join("config.json");
        let lockfile_path = temp.path().join("lock.json");
        fs::write(&catalog_path, include_str!("../../../plugins/catalog.json")).unwrap();
        write_json(&config_path, &json!({"defaultProfile": "dart"}));

        let result = run_resolve_catalog(&ResolveCatalogOptions {
            catalog_path,
            config_path,
            lockfile_path,
            dry_run: true,
        }).unwrap();

        let ids: Vec<&str> = result
            .resolved_plugins
            .iter()
            .filter_map(|plugin| plugin.get("pluginId").and_then(Value::as_str))
            .collect();
        assert_eq!(ids, vec!["gal-core", "dart-skills"]);
    }

    #[test]
    fn full_profile_resolves_all_plugins() {
        let temp = TempDir::new().unwrap();
        let catalog_path = temp.path().join("catalog.json");
        let config_path = temp.path().join("config.json");
        let lockfile_path = temp.path().join("lock.json");
        fs::write(&catalog_path, include_str!("../../../plugins/catalog.json")).unwrap();
        write_json(&config_path, &json!({"defaultProfile": "full"}));

        let result = run_resolve_catalog(&ResolveCatalogOptions {
            catalog_path,
            config_path,
            lockfile_path,
            dry_run: true,
        }).unwrap();

        assert_eq!(result.resolved_plugins.len(), 9);
    }

    #[test]
    fn explicit_enabled_plugins_are_added() {
        let temp = TempDir::new().unwrap();
        let catalog_path = temp.path().join("catalog.json");
        let config_path = temp.path().join("config.json");
        let lockfile_path = temp.path().join("lock.json");
        fs::write(&catalog_path, include_str!("../../../plugins/catalog.json")).unwrap();
        write_json(&config_path, &json!({"enabledPlugins": ["gal-core", "rust-skills"]}));

        let result = run_resolve_catalog(&ResolveCatalogOptions {
            catalog_path,
            config_path,
            lockfile_path,
            dry_run: true,
        }).unwrap();

        let ids: Vec<&str> = result
            .resolved_plugins
            .iter()
            .filter_map(|plugin| plugin.get("pluginId").and_then(Value::as_str))
            .collect();
        assert_eq!(ids, vec!["gal-core", "rust-skills"]);
    }

    #[test]
    fn invalid_catalog_entry_emits_validation_errors() {
        let temp = TempDir::new().unwrap();
        let catalog_path = temp.path().join("catalog.json");
        let config_path = temp.path().join("config.json");
        let lockfile_path = temp.path().join("lock.json");
        let mut catalog: Value = serde_json::from_str(include_str!("../../../plugins/catalog.json")).unwrap();
        let plugins = catalog.get_mut("plugins").and_then(Value::as_array_mut).unwrap();
        plugins.push(json!({
            "pluginId": "bad-plugin",
            "displayName": "Bad Plugin",
            "supportTier": "curated-upstream",
            "sourceType": "curated-upstream",
            "upstream": { "repo": "https://github.com/example/bad" },
            "license": "",
            "checksumPolicy": "",
            "componentMap": { "skills": true },
            "supportedProviders": ["claude"],
            "installStrategy": "git-clone",
            "defaultProfiles": ["bad"],
            "allowAutoUpdate": false,
            "localOverridePolicy": "source-mode-only"
        }));
        write_json(&catalog_path, &catalog);
        write_json(&config_path, &json!({"defaultProfile": "default"}));

        let result = run_resolve_catalog(&ResolveCatalogOptions {
            catalog_path,
            config_path,
            lockfile_path,
            dry_run: true,
        }).unwrap();

        assert!(!result.errors.is_empty());
        assert!(result.errors.iter().any(|error| error.contains("License")));
        assert!(result.errors.iter().any(|error| error.contains("checksumPolicy")));
    }
}