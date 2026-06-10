//! AGY legacy pre-cleanup (ported from Setup-Machine.{ps1,sh}).
//!
//! Removes GAL-managed Antigravity legacy surfaces before the install
//! orchestration runs, and strips only `gal` / `gal-*` keys from the global
//! AGY `mcp_config.json` while preserving all other content (no jq
//! dependency; architect C-8).
//!
//! Every removal passes the path-safety guard: deletion is refused for
//! empty, relative, root, or home-equal paths, and for any path that does
//! not live under the user home (architect C-8 — the classic empty-env-var
//! catastrophic-delete failure).

use serde_json::{Map, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The three legacy AGY paths Setup-Machine removes, with their labels.
pub fn agy_legacy_paths(user_home: &Path) -> Vec<(PathBuf, &'static str)> {
    let antigravity_root = user_home.join(".gemini").join("antigravity-cli");
    vec![
        (antigravity_root.join("skills"), "legacy AGY skills directory"),
        (antigravity_root.join("gal"), "legacy AGY GAL_ROOT symlink"),
        (
            antigravity_root.join("plugins").join("gal"),
            "existing AGY plugin install",
        ),
    ]
}

/// Global AGY MCP config file.
pub fn agy_mcp_file(user_home: &Path) -> PathBuf {
    user_home
        .join(".gemini")
        .join("antigravity-cli")
        .join("mcp_config.json")
}

/// Path-safety guard (C-8): only allow deletion of an absolute path that is
/// strictly inside the user home (never the home itself, never a root).
pub fn is_safe_to_remove(path: &Path, user_home: &Path) -> bool {
    if path.as_os_str().is_empty() || user_home.as_os_str().is_empty() {
        return false;
    }
    if !path.is_absolute() || !user_home.is_absolute() {
        return false;
    }
    // A filesystem root has no parent.
    if path.parent().is_none() {
        return false;
    }
    if path == user_home {
        return false;
    }
    path.starts_with(user_home)
}

/// Strip GAL-managed keys (`gal`, `gal-*`) from an `mcpServers` map,
/// preserving every other key and all non-`mcpServers` content.
/// Returns the removed key names (empty when nothing matched).
pub fn strip_gal_mcp_entries(config: &mut Map<String, Value>) -> Vec<String> {
    let Some(Value::Object(servers)) = config.get_mut("mcpServers") else {
        return Vec::new();
    };
    let removed: Vec<String> = servers
        .keys()
        .filter(|k| *k == "gal" || k.starts_with("gal-"))
        .cloned()
        .collect();
    for key in &removed {
        servers.remove(key);
    }
    removed
}

/// Run the AGY legacy pre-cleanup. Honors `dry_run` (zero writes, C-7).
pub fn run_agy_cleanup(
    user_home: &Path,
    dry_run: bool,
    out: &mut dyn Write,
) -> std::io::Result<()> {
    for (path, label) in agy_legacy_paths(user_home) {
        if path.symlink_metadata().is_err() {
            continue;
        }
        if !is_safe_to_remove(&path, user_home) {
            let _ = writeln!(
                out,
                "  [WARN] Refusing to remove {label}: unsafe path {}",
                path.display()
            );
            continue;
        }
        if dry_run {
            let _ = writeln!(out, "  [DRY RUN] Would remove {label}: {}", path.display());
        } else {
            remove_path(&path)?;
            let _ = writeln!(out, "  [REMOVED] {label}: {}", path.display());
        }
    }

    let mcp_file = agy_mcp_file(user_home);
    if mcp_file.is_file() {
        match base::json_util::read_json_map(&mcp_file) {
            Ok(mut config) => {
                let removed = {
                    // Probe without mutating first so dry-run stays read-only.
                    let probe: Vec<String> = config
                        .get("mcpServers")
                        .and_then(Value::as_object)
                        .map(|servers| {
                            servers
                                .keys()
                                .filter(|k| *k == "gal" || k.starts_with("gal-"))
                                .cloned()
                                .collect()
                        })
                        .unwrap_or_default();
                    probe
                };
                if !removed.is_empty() {
                    if dry_run {
                        let _ = writeln!(
                            out,
                            "  [DRY RUN] Would remove GAL-managed MCP entries from: {} ({})",
                            mcp_file.display(),
                            removed.join(", ")
                        );
                    } else {
                        strip_gal_mcp_entries(&mut config);
                        base::json_util::write_json_map(&mcp_file, &config)?;
                        let _ = writeln!(
                            out,
                            "  [REMOVED] GAL-managed MCP entries from: {} ({})",
                            mcp_file.display(),
                            removed.join(", ")
                        );
                    }
                }
            }
            Err(_) => {
                let _ = writeln!(
                    out,
                    "  [WARN] Could not parse {} for legacy cleanup — skipping",
                    mcp_file.display()
                );
            }
        }
    }
    Ok(())
}

/// Remove a path of any kind (dir tree, file, or link).
fn remove_path(path: &Path) -> std::io::Result<()> {
    let meta = path.symlink_metadata()?;
    if meta.is_dir() && !base::platform::is_symlink_or_junction(path) {
        fs::remove_dir_all(path)
    } else if meta.is_dir() {
        base::platform::remove_dir_link(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── path-safety guard (C-8) ──────────────────────────────────────────────

    #[test]
    fn guard_refuses_empty_root_home_and_outside_paths() {
        let home = if cfg!(windows) {
            PathBuf::from(r"C:\Users\someone")
        } else {
            PathBuf::from("/home/someone")
        };
        let root = if cfg!(windows) {
            PathBuf::from(r"C:\")
        } else {
            PathBuf::from("/")
        };

        assert!(!is_safe_to_remove(Path::new(""), &home), "empty path");
        assert!(!is_safe_to_remove(&root, &home), "filesystem root");
        assert!(!is_safe_to_remove(&home, &home), "home itself");
        assert!(
            !is_safe_to_remove(Path::new("relative/path"), &home),
            "relative path"
        );
        let outside = if cfg!(windows) {
            PathBuf::from(r"C:\Users\other\.gemini")
        } else {
            PathBuf::from("/home/other/.gemini")
        };
        assert!(!is_safe_to_remove(&outside, &home), "outside home");
        // Empty home refuses everything (empty-env-var failure mode).
        assert!(!is_safe_to_remove(&home.join(".gemini"), Path::new("")));
    }

    #[test]
    fn guard_allows_paths_inside_home() {
        let home = if cfg!(windows) {
            PathBuf::from(r"C:\Users\someone")
        } else {
            PathBuf::from("/home/someone")
        };
        for (path, _label) in agy_legacy_paths(&home) {
            assert!(is_safe_to_remove(&path, &home), "{}", path.display());
        }
    }

    // ── JSON strip (C-8) ─────────────────────────────────────────────────────

    #[test]
    fn strip_removes_only_gal_keys_and_preserves_everything_else() {
        let value = json!({
            "mcpServers": {
                "gal": {"command": "x"},
                "gal-doc-sync": {"command": "y"},
                "galaxy": {"command": "keep-me"},
                "user-server": {"command": "keep"}
            },
            "otherTopLevel": {"keep": true}
        });
        let mut map = value.as_object().unwrap().clone();
        let removed = strip_gal_mcp_entries(&mut map);
        assert_eq!(removed, vec!["gal".to_string(), "gal-doc-sync".to_string()]);
        let servers = map.get("mcpServers").unwrap().as_object().unwrap();
        assert!(servers.contains_key("galaxy"), "prefix-similar key preserved");
        assert!(servers.contains_key("user-server"));
        assert!(map.contains_key("otherTopLevel"));
    }

    #[test]
    fn strip_handles_missing_mcp_servers_key() {
        let mut map = json!({"unrelated": 1}).as_object().unwrap().clone();
        assert!(strip_gal_mcp_entries(&mut map).is_empty());
        assert!(map.contains_key("unrelated"));
    }

    // ── cleanup behavior (hermetic temp home) ────────────────────────────────

    #[test]
    fn cleanup_removes_legacy_paths_and_strips_json() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        let agy_root = home.join(".gemini").join("antigravity-cli");
        fs::create_dir_all(agy_root.join("skills")).unwrap();
        fs::create_dir_all(agy_root.join("plugins").join("gal")).unwrap();
        fs::write(
            agy_mcp_file(home),
            serde_json::to_string_pretty(&json!({
                "mcpServers": {"gal-doc-sync": {}, "mine": {"keep": true}},
                "telemetry": false
            }))
            .unwrap(),
        )
        .unwrap();

        let mut out = Vec::new();
        run_agy_cleanup(home, false, &mut out).unwrap();

        assert!(!agy_root.join("skills").exists());
        assert!(!agy_root.join("plugins").join("gal").exists());
        let after = base::json_util::read_json_map(&agy_mcp_file(home)).unwrap();
        let servers = after.get("mcpServers").unwrap().as_object().unwrap();
        assert!(!servers.contains_key("gal-doc-sync"));
        assert!(servers.contains_key("mine"));
        assert_eq!(after.get("telemetry"), Some(&Value::Bool(false)));
    }

    #[test]
    fn cleanup_dry_run_writes_nothing(){
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        let agy_root = home.join(".gemini").join("antigravity-cli");
        fs::create_dir_all(agy_root.join("skills")).unwrap();
        let mcp_raw = serde_json::to_string_pretty(&json!({
            "mcpServers": {"gal": {}}
        }))
        .unwrap();
        fs::write(agy_mcp_file(home), &mcp_raw).unwrap();

        let mut out = Vec::new();
        run_agy_cleanup(home, true, &mut out).unwrap();
        let printed = String::from_utf8(out).unwrap();

        assert!(agy_root.join("skills").exists(), "dry-run must not delete");
        assert_eq!(
            fs::read_to_string(agy_mcp_file(home)).unwrap(),
            mcp_raw,
            "dry-run must not rewrite mcp_config.json"
        );
        assert!(printed.contains("[DRY RUN] Would remove legacy AGY skills directory"));
        assert!(printed.contains("[DRY RUN] Would remove GAL-managed MCP entries"));
    }
}
