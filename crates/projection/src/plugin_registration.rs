//! Read-only observation of plugin registration evidence for supported runtimes.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationState {
    Registered,
    NotRegistered,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationObservation {
    pub state: RegistrationState,
    pub source: PathBuf,
    /// Stable reason code. `None` means the observation is unambiguous.
    pub reason: Option<&'static str>,
}

impl RegistrationObservation {
    fn known(state: RegistrationState, source: PathBuf) -> Self {
        Self {
            state,
            source,
            reason: None,
        }
    }

    fn unknown(source: PathBuf, reason: &'static str) -> Self {
        Self {
            state: RegistrationState::Unknown,
            source,
            reason: Some(reason),
        }
    }
}

/// Immutable evidence captured for a single projection refresh.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RegistrationSnapshot {
    observations: BTreeMap<String, RegistrationObservation>,
}

impl RegistrationSnapshot {
    pub fn observe(home: &Path) -> Self {
        let observations = [
            ("claude", observe_claude(home)),
            ("codex", observe_codex(home)),
            ("copilot", observe_copilot(home)),
            ("agy", observe_antigravity(home)),
        ]
        .into_iter()
        .map(|(runtime, observation)| (runtime.to_string(), observation))
        .collect();
        Self { observations }
    }

    pub fn get(&self, runtime: &str) -> Option<&RegistrationObservation> {
        self.observations.get(runtime)
    }
}

fn read_json(path: &Path) -> Result<Value, RegistrationObservation> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|_| RegistrationObservation::unknown(path.to_path_buf(), "malformed-json")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(
            RegistrationObservation::known(RegistrationState::NotRegistered, path.to_path_buf()),
        ),
        Err(_) => Err(RegistrationObservation::unknown(
            path.to_path_buf(),
            "read-error",
        )),
    }
}

fn named_gal(name: &str) -> bool {
    name == "gal" || name.starts_with("gal@")
}

fn observe_claude(home: &Path) -> RegistrationObservation {
    let manifest = home.join(".claude/skills/gal/.claude-plugin/plugin.json");
    let installed = home.join(".claude/plugins/installed_plugins.json");
    let mut uncertain = None;
    match read_json(&manifest) {
        Ok(value) => {
            if value.get("name").and_then(Value::as_str) == Some("gal") {
                let plugin_dir = manifest
                    .parent()
                    .and_then(Path::parent)
                    .unwrap_or(Path::new(""));
                if plugin_dir.is_dir() {
                    return RegistrationObservation::known(RegistrationState::Registered, manifest);
                }
                uncertain = Some(RegistrationObservation::unknown(
                    manifest.clone(),
                    "inaccessible-plugin-directory",
                ));
            } else {
                uncertain = Some(RegistrationObservation::unknown(
                    manifest.clone(),
                    "invalid-manifest-identity",
                ));
            }
        }
        Err(obs) if obs.state == RegistrationState::Unknown => uncertain = Some(obs),
        Err(_) => {}
    }
    match read_json(&installed) {
        Ok(value) => {
            let plugins = value.get("plugins");
            let entries = plugins.and_then(Value::as_object);
            if plugins.is_some() && entries.is_none() {
                return RegistrationObservation::unknown(installed, "invalid-installation-record");
            }
            if let Some(entries) = entries {
                for (name, records) in entries {
                    if !named_gal(name) {
                        continue;
                    }
                    let Some(records) = records.as_array() else {
                        return RegistrationObservation::unknown(
                            installed,
                            "invalid-installation-record",
                        );
                    };
                    if records.is_empty() {
                        return RegistrationObservation::unknown(
                            installed,
                            "empty-installation-record",
                        );
                    }
                    let mut record_uncertain = None;
                    for record in records {
                        let Some(path) = record.get("installPath").and_then(Value::as_str) else {
                            record_uncertain.get_or_insert_with(|| {
                                RegistrationObservation::unknown(
                                    installed.clone(),
                                    "invalid-installation-record",
                                )
                            });
                            continue;
                        };
                        let target = PathBuf::from(path);
                        if target.is_dir() {
                            let target_manifest = target.join(".claude-plugin/plugin.json");
                            if let Ok(content) = fs::read_to_string(&target_manifest) {
                                if serde_json::from_str::<Value>(&content)
                                    .ok()
                                    .and_then(|v| {
                                        v.get("name").and_then(Value::as_str).map(str::to_owned)
                                    })
                                    .as_deref()
                                    == Some("gal")
                                {
                                    return RegistrationObservation::known(
                                        RegistrationState::Registered,
                                        installed,
                                    );
                                }
                            }
                            record_uncertain.get_or_insert_with(|| {
                                RegistrationObservation::unknown(
                                    installed.clone(),
                                    "unverifiable-installation-target",
                                )
                            });
                            continue;
                        }
                        record_uncertain.get_or_insert_with(|| {
                            RegistrationObservation::unknown(
                                installed.clone(),
                                "inaccessible-installation-target",
                            )
                        });
                    }
                    if let Some(obs) = record_uncertain {
                        return obs;
                    }
                }
            }
        }
        Err(obs) if obs.state == RegistrationState::Unknown => uncertain = Some(obs),
        Err(_) => {}
    }
    uncertain.unwrap_or_else(|| {
        RegistrationObservation::known(RegistrationState::NotRegistered, installed)
    })
}

fn observe_codex(home: &Path) -> RegistrationObservation {
    let path = home.join(".codex/config.toml");
    match fs::read_to_string(&path) {
        Ok(text) => match text.parse::<toml::Value>() {
            Ok(value) => {
                let plugins = value.get("plugins");
                let Some(plugins) = plugins else {
                    return RegistrationObservation::known(RegistrationState::NotRegistered, path);
                };
                let Some(table) = plugins.as_table() else {
                    return RegistrationObservation::unknown(path, "invalid-plugins-type");
                };
                for (_, config) in table.iter().filter(|(name, _)| named_gal(name)) {
                    let Some(config) = config.as_table() else {
                        return RegistrationObservation::unknown(path, "invalid-plugin-entry");
                    };
                    match config.get("enabled") {
                        None => {
                            return RegistrationObservation::known(
                                RegistrationState::Registered,
                                path,
                            )
                        }
                        Some(toml::Value::Boolean(true)) => {
                            return RegistrationObservation::known(
                                RegistrationState::Registered,
                                path,
                            )
                        }
                        Some(toml::Value::Boolean(false)) => {}
                        Some(_) => {
                            return RegistrationObservation::unknown(path, "invalid-enabled-type")
                        }
                    }
                }
                RegistrationObservation::known(RegistrationState::NotRegistered, path)
            }
            Err(_) => RegistrationObservation::unknown(path, "malformed-toml"),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            RegistrationObservation::known(RegistrationState::NotRegistered, path)
        }
        Err(_) => RegistrationObservation::unknown(path, "read-error"),
    }
}

fn observe_copilot(home: &Path) -> RegistrationObservation {
    let path = home.join(".copilot/settings.json");
    match read_json(&path) {
        Ok(value) => {
            let Some(plugins) = value.get("enabledPlugins") else {
                return RegistrationObservation::known(RegistrationState::NotRegistered, path);
            };
            let Some(plugins) = plugins.as_object() else {
                return RegistrationObservation::unknown(path, "invalid-enabled-plugins-type");
            };
            let mut uncertain = false;
            for (_, enabled) in plugins.iter().filter(|(name, _)| named_gal(name)) {
                match enabled.as_bool() {
                    Some(true) => {
                        return RegistrationObservation::known(RegistrationState::Registered, path)
                    }
                    Some(false) => {}
                    None => uncertain = true,
                }
            }
            if uncertain {
                RegistrationObservation::unknown(path, "invalid-plugin-enabled-type")
            } else {
                RegistrationObservation::known(RegistrationState::NotRegistered, path)
            }
        }
        Err(observation) => observation,
    }
}

fn observe_antigravity(home: &Path) -> RegistrationObservation {
    let path = home
        .join(".gemini")
        .join("antigravity-cli")
        .join("plugins")
        .join("gal");
    match fs::symlink_metadata(&path) {
        Ok(_) => match fs::canonicalize(&path) {
            Ok(directory) => {
                let manifest = directory.join(".claude-plugin/plugin.json");
                match read_json(&manifest) {
                    Ok(value) if value.get("name").and_then(Value::as_str) == Some("gal") => {
                        RegistrationObservation::known(RegistrationState::Registered, path)
                    }
                    Ok(_) => {
                        RegistrationObservation::unknown(manifest, "invalid-manifest-identity")
                    }
                    Err(observation) if observation.state == RegistrationState::Unknown => {
                        observation
                    }
                    Err(_) => RegistrationObservation::unknown(manifest, "missing-plugin-manifest"),
                }
            }
            Err(_) => RegistrationObservation::unknown(path, "unresolvable-plugin-directory"),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            RegistrationObservation::known(RegistrationState::NotRegistered, path)
        }
        Err(_) => RegistrationObservation::unknown(path, "unresolvable-plugin-directory"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_sources_are_not_registered() {
        let home = tempdir().unwrap();
        for runtime in ["claude", "codex", "copilot", "agy"] {
            assert_eq!(
                RegistrationSnapshot::observe(home.path())
                    .get(runtime)
                    .unwrap()
                    .state,
                RegistrationState::NotRegistered
            );
        }
    }

    #[test]
    fn malformed_sources_are_unknown_with_diagnostics() {
        let home = tempdir().unwrap();
        for (path, content) in [
            (".codex/config.toml", "[plugins"),
            (".copilot/settings.json", "{"),
            (".claude/plugins/installed_plugins.json", "{"),
        ] {
            let full = home.path().join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, content).unwrap();
        }
        let snapshot = RegistrationSnapshot::observe(home.path());
        for runtime in ["claude", "codex", "copilot"] {
            let observation = snapshot.get(runtime).unwrap();
            assert_eq!(observation.state, RegistrationState::Unknown);
            assert!(observation.reason.is_some());
        }
    }

    #[test]
    fn enabled_codex_and_copilot_entries_are_registered() {
        let home = tempdir().unwrap();
        let codex = home.path().join(".codex/config.toml");
        fs::create_dir_all(codex.parent().unwrap()).unwrap();
        fs::write(codex, "[plugins.gal]\nenabled = true\n").unwrap();
        let copilot = home.path().join(".copilot/settings.json");
        fs::create_dir_all(copilot.parent().unwrap()).unwrap();
        fs::write(copilot, r#"{"enabledPlugins":{"gal":true}}"#).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("codex").unwrap().state,
            RegistrationState::Registered
        );
        assert_eq!(
            snapshot.get("copilot").unwrap().state,
            RegistrationState::Registered
        );
    }

    #[test]
    fn claude_and_antigravity_require_gal_manifest_identity() {
        let home = tempdir().unwrap();
        let claude_manifest = home
            .path()
            .join(".claude/skills/gal/.claude-plugin/plugin.json");
        fs::create_dir_all(claude_manifest.parent().unwrap()).unwrap();
        fs::write(&claude_manifest, r#"{"name":"gal"}"#).unwrap();

        let agy_plugin = home.path().join(".gemini/antigravity-cli/plugins/gal");
        let agy_manifest = agy_plugin.join(".claude-plugin/plugin.json");
        fs::create_dir_all(agy_manifest.parent().unwrap()).unwrap();
        fs::write(&agy_manifest, r#"{"name":"gal"}"#).unwrap();

        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("claude").unwrap().state,
            RegistrationState::Registered
        );
        assert_eq!(
            snapshot.get("agy").unwrap().state,
            RegistrationState::Registered
        );
    }

    #[test]
    fn antigravity_invalid_manifest_is_unknown_with_source() {
        let home = tempdir().unwrap();
        let manifest = home
            .path()
            .join(".gemini/antigravity-cli/plugins/gal/.claude-plugin/plugin.json");
        fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        fs::write(&manifest, r#"{"name":"other"}"#).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let observation = snapshot.get("agy").unwrap();
        assert_eq!(observation.state, RegistrationState::Unknown);
        assert_eq!(observation.source, fs::canonicalize(manifest).unwrap());
        assert_eq!(observation.reason, Some("invalid-manifest-identity"));
    }

    #[test]
    fn codex_detailed_registration_matrix() {
        let home = tempdir().unwrap();
        let codex = home.path().join(".codex/config.toml");
        fs::create_dir_all(codex.parent().unwrap()).unwrap();

        // 1. Absent enabled defaults to true -> Registered
        fs::write(&codex, "[plugins.gal]\n").unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("codex").unwrap().state,
            RegistrationState::Registered
        );

        // 2. gal@version is recognized -> Registered
        fs::write(&codex, "[plugins.\"gal@1.2.0\"]\nenabled = true\n").unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("codex").unwrap().state,
            RegistrationState::Registered
        );

        // 3. Explicitly disabled -> NotRegistered
        fs::write(&codex, "[plugins.gal]\nenabled = false\n").unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("codex").unwrap().state,
            RegistrationState::NotRegistered
        );

        // 4. Other plugins only -> NotRegistered
        fs::write(&codex, "[plugins.other]\nenabled = true\n").unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("codex").unwrap().state,
            RegistrationState::NotRegistered
        );

        // 5. Non-boolean enabled -> Unknown (invalid-enabled-type)
        fs::write(&codex, "[plugins.gal]\nenabled = \"true\"\n").unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("codex").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("invalid-enabled-type"));

        // 6. plugins is not a table -> Unknown (invalid-plugins-type)
        fs::write(&codex, "plugins = \"gal\"\n").unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("codex").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("invalid-plugins-type"));

        // 7. plugins.gal is not a table -> Unknown (invalid-plugin-entry)
        fs::write(&codex, "[plugins]\ngal = true\n").unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("codex").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("invalid-plugin-entry"));
    }

    #[test]
    fn copilot_detailed_registration_matrix() {
        let home = tempdir().unwrap();
        let copilot = home.path().join(".copilot/settings.json");
        fs::create_dir_all(copilot.parent().unwrap()).unwrap();

        // 1. gal@version is recognized -> Registered
        fs::write(&copilot, r#"{"enabledPlugins":{"gal@0.2.0":true}}"#).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("copilot").unwrap().state,
            RegistrationState::Registered
        );

        // 2. Explicitly disabled -> NotRegistered
        fs::write(&copilot, r#"{"enabledPlugins":{"gal":false}}"#).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("copilot").unwrap().state,
            RegistrationState::NotRegistered
        );

        // 3. Other plugins only -> NotRegistered
        fs::write(&copilot, r#"{"enabledPlugins":{"other":true}}"#).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("copilot").unwrap().state,
            RegistrationState::NotRegistered
        );

        // 4. Non-boolean enabled -> Unknown (invalid-plugin-enabled-type)
        fs::write(&copilot, r#"{"enabledPlugins":{"gal":"true"}}"#).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("copilot").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("invalid-plugin-enabled-type"));

        // 5. enabledPlugins is not an object -> Unknown (invalid-enabled-plugins-type)
        fs::write(&copilot, r#"{"enabledPlugins":["gal"]}"#).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("copilot").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("invalid-enabled-plugins-type"));
    }

    #[test]
    fn claude_detailed_registration_matrix() {
        let home = tempdir().unwrap();
        let installed = home.path().join(".claude/plugins/installed_plugins.json");
        fs::create_dir_all(installed.parent().unwrap()).unwrap();

        // 1. Valid installed plugin with target -> Registered
        let target_dir = home.path().join("external-gal-plugin");
        fs::create_dir_all(target_dir.join(".claude-plugin")).unwrap();
        fs::write(
            target_dir.join(".claude-plugin/plugin.json"),
            r#"{"name":"gal"}"#,
        )
        .unwrap();
        let target_str = target_dir.to_str().unwrap().replace('\\', "\\\\");
        fs::write(
            &installed,
            format!(
                r#"{{"plugins":{{"gal":[{{"installPath":"{}"}}]}}}}"#,
                target_str
            ),
        )
        .unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("claude").unwrap().state,
            RegistrationState::Registered
        );

        // 2. Empty installation record -> Unknown (empty-installation-record)
        fs::write(&installed, r#"{"plugins":{"gal":[]}}"#).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("claude").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("empty-installation-record"));

        // 3. Invalid installation record (not an array) -> Unknown (invalid-installation-record)
        fs::write(&installed, r#"{"plugins":{"gal":"not-an-array"}}"#).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("claude").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("invalid-installation-record"));

        // 4. Record without installPath -> Unknown (invalid-installation-record)
        fs::write(&installed, r#"{"plugins":{"gal":[{}]}}"#).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("claude").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("invalid-installation-record"));

        // 5. Inaccessible installation target -> Unknown (inaccessible-installation-target)
        fs::write(
            &installed,
            r#"{"plugins":{"gal":[{"installPath":"/nonexistent/path/for/test"}]}}"#,
        )
        .unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("claude").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("inaccessible-installation-target"));

        // 6. Unverifiable installation target (manifest missing or invalid) -> Unknown (unverifiable-installation-target)
        let empty_target = home.path().join("empty-target");
        fs::create_dir_all(&empty_target).unwrap();
        let empty_target_str = empty_target.to_str().unwrap().replace('\\', "\\\\");
        fs::write(
            &installed,
            format!(
                r#"{{"plugins":{{"gal":[{{"installPath":"{}"}}]}}}}"#,
                empty_target_str
            ),
        )
        .unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("claude").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("unverifiable-installation-target"));
    }

    #[test]
    fn claude_independent_source_precedence() {
        let home = tempdir().unwrap();
        let dir_manifest = home
            .path()
            .join(".claude/skills/gal/.claude-plugin/plugin.json");
        fs::create_dir_all(dir_manifest.parent().unwrap()).unwrap();
        let installed = home.path().join(".claude/plugins/installed_plugins.json");
        fs::create_dir_all(installed.parent().unwrap()).unwrap();

        // 1. Valid dir manifest + malformed installed -> Registered (positive wins over uncertain)
        fs::write(&dir_manifest, r#"{"name":"gal"}"#).unwrap();
        fs::write(&installed, "malformed").unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("claude").unwrap().state,
            RegistrationState::Registered
        );

        // 2. Valid dir manifest + missing installed -> Registered (positive wins over absent)
        fs::remove_file(&installed).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("claude").unwrap().state,
            RegistrationState::Registered
        );

        // 3. Malformed dir manifest + missing installed -> Unknown (uncertain wins over absent)
        fs::write(&dir_manifest, "malformed").unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("claude").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("malformed-json"));

        // 4. Missing dir manifest + malformed installed -> Unknown (uncertain wins over absent)
        fs::remove_file(&dir_manifest).unwrap();
        fs::write(&installed, "malformed").unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("claude").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("malformed-json"));
    }

    #[test]
    fn antigravity_detailed_registration_matrix() {
        let home = tempdir().unwrap();
        let agy_plugin = home.path().join(".gemini/antigravity-cli/plugins/gal");

        // 1. Missing -> NotRegistered
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("agy").unwrap().state,
            RegistrationState::NotRegistered
        );

        // 2. Directory exists without manifest -> Unknown (missing-plugin-manifest)
        fs::create_dir_all(&agy_plugin).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("agy").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("missing-plugin-manifest"));

        // 3. Directory exists with malformed manifest -> Unknown (malformed-json)
        let manifest = agy_plugin.join(".claude-plugin/plugin.json");
        fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        fs::write(&manifest, "malformed").unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("agy").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("malformed-json"));

        // 4. Directory exists with valid manifest -> Registered
        fs::write(&manifest, r#"{"name":"gal"}"#).unwrap();
        let snapshot = RegistrationSnapshot::observe(home.path());
        assert_eq!(
            snapshot.get("agy").unwrap().state,
            RegistrationState::Registered
        );
    }

    #[test]
    fn antigravity_broken_link_is_unknown() {
        let home = tempdir().unwrap();
        let agy_plugin = home
            .path()
            .join(".gemini")
            .join("antigravity-cli")
            .join("plugins")
            .join("gal");
        fs::create_dir_all(agy_plugin.parent().unwrap()).unwrap();

        // Create a target and a junction/symlink to it, then remove the target
        let target = home.path().join("transient-target");
        fs::create_dir_all(&target).unwrap();

        gal_foundation::platform::create_dir_link(&target, &agy_plugin).unwrap();

        // Delete target to make it a broken link
        fs::remove_dir_all(&target).unwrap();

        let snapshot = RegistrationSnapshot::observe(home.path());
        let obs = snapshot.get("agy").unwrap();
        assert_eq!(obs.state, RegistrationState::Unknown);
        assert_eq!(obs.reason, Some("unresolvable-plugin-directory"));
    }
}
