//! Path location resolution for GAL state directories.
//!
//! All functions return `Option<PathBuf>` — `None` when home is unavailable.
//! No file I/O; callers decide whether to create directories.
//! Cross-platform: `USERPROFILE` on Windows, `HOME` on Unix (matches
//! `Get-GalUserHome` / `get_gal_user_home` in `Common.{ps1,sh}`).

use std::path::PathBuf;

// ── home ─────────────────────────────────────────────────────────────────────

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// The effective user home directory used by GAL path resolution.
pub fn user_home() -> Option<PathBuf> {
    home_dir()
}

/// The GAL home directory (`~/.gal`).
pub fn gal_home() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".gal"))
}

// ── config paths ─────────────────────────────────────────────────────────────

/// `~/.gal/config/config.json`
pub fn machine_config_path() -> Option<PathBuf> {
    gal_home().map(|h| h.join("config").join("config.json"))
}

/// `~/.gal/config/executor-routing.json`
///
/// Ports the canonical path in `Read-ExecutorRouting` from Common.ps1.
pub fn executor_routing_path() -> Option<PathBuf> {
    gal_home().map(|h| h.join("config").join("executor-routing.json"))
}

// ── state paths ───────────────────────────────────────────────────────────────

/// `~/.gal/install-state.json`
///
/// Ports `InstallStateFile` from `New-SetupContext` in Common.ps1.
pub fn install_state_path() -> Option<PathBuf> {
    gal_home().map(|h| h.join("install-state.json"))
}

// ── plugin paths ──────────────────────────────────────────────────────────────

/// `~/.gal/plugins`
///
/// Ports `Get-GalPluginsRoot` from Common.ps1.
pub fn gal_plugins_root() -> Option<PathBuf> {
    gal_home().map(|h| h.join("plugins"))
}

/// `~/.gal/plugins/<plugin_id>`
///
/// Ports `Get-GalPluginRoot` from Common.ps1.
pub fn gal_plugin_root(plugin_id: &str) -> Option<PathBuf> {
    gal_plugins_root().map(|r| r.join(plugin_id))
}

// ── data / cache paths ────────────────────────────────────────────────────────

/// `~/.gal/data`
///
/// Ports `Get-GalDataRoot` from Common.ps1.
pub fn gal_data_root() -> Option<PathBuf> {
    gal_home().map(|h| h.join("data"))
}

/// `~/.gal/data/<plugin_id>`
///
/// Ports `Get-GalPluginDataRoot` from Common.ps1.
pub fn gal_plugin_data_root(plugin_id: &str) -> Option<PathBuf> {
    gal_data_root().map(|r| r.join(plugin_id))
}

/// `~/.gal/cache`
///
/// Ports `Get-GalCacheRoot` from Common.ps1.
pub fn gal_cache_root() -> Option<PathBuf> {
    gal_home().map(|h| h.join("cache"))
}

// ── provider / active paths ───────────────────────────────────────────────────

/// `~/.gal/active/<provider>`
///
/// Ports `Get-GalActiveProviderTarget` from Common.ps1.
pub fn gal_active_provider_path(provider: &str) -> Option<PathBuf> {
    gal_home().map(|h| h.join("active").join(provider))
}

// ── generated paths ───────────────────────────────────────────────────────────

/// `~/.gal/generated/mcp/managed.json`
///
/// Ports `GalGeneratedMcpFile` from `New-SetupContext` in Common.ps1.
pub fn generated_mcp_path() -> Option<PathBuf> {
    gal_home().map(|h| h.join("generated").join("mcp").join("managed.json"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gal_home_ends_with_dot_gal() {
        if let Some(h) = gal_home() {
            assert_eq!(h.file_name().and_then(|n| n.to_str()), Some(".gal"));
        }
    }

    #[test]
    fn machine_config_path_ends_with_config_json() {
        if let Some(p) = machine_config_path() {
            let s = p.to_string_lossy();
            assert!(s.contains(".gal"), "must be under .gal: {s}");
            assert!(s.ends_with("config.json"), "must end with config.json: {s}");
        }
    }

    #[test]
    fn executor_routing_path_ends_with_executor_routing_json() {
        if let Some(p) = executor_routing_path() {
            let s = p.to_string_lossy();
            assert!(
                s.ends_with("executor-routing.json"),
                "must end with executor-routing.json: {s}"
            );
        }
    }

    #[test]
    fn install_state_path_ends_with_install_state_json() {
        if let Some(p) = install_state_path() {
            let s = p.to_string_lossy();
            assert!(
                s.ends_with("install-state.json"),
                "must end with install-state.json: {s}"
            );
        }
    }

    #[test]
    fn gal_plugins_root_leaf_is_plugins() {
        if let Some(p) = gal_plugins_root() {
            assert_eq!(p.file_name().and_then(|n| n.to_str()), Some("plugins"));
        }
    }

    #[test]
    fn gal_plugin_root_appends_plugin_id() {
        if let Some(p) = gal_plugin_root("gal-core") {
            let s = p.to_string_lossy();
            assert!(
                s.ends_with("gal-core") || s.ends_with("gal-core\\"),
                "must end with plugin id: {s}"
            );
        }
    }

    #[test]
    fn gal_data_root_leaf_is_data() {
        if let Some(p) = gal_data_root() {
            assert_eq!(p.file_name().and_then(|n| n.to_str()), Some("data"));
        }
    }

    #[test]
    fn gal_plugin_data_root_contains_data_and_plugin_id() {
        if let Some(p) = gal_plugin_data_root("gal-core") {
            let s = p.to_string_lossy();
            assert!(s.contains("data"), "must contain 'data': {s}");
            assert!(
                s.ends_with("gal-core") || s.ends_with("gal-core\\"),
                "must end with plugin id: {s}"
            );
        }
    }

    #[test]
    fn gal_cache_root_leaf_is_cache() {
        if let Some(p) = gal_cache_root() {
            assert_eq!(p.file_name().and_then(|n| n.to_str()), Some("cache"));
        }
    }

    #[test]
    fn gal_active_provider_path_appends_provider() {
        if let Some(p) = gal_active_provider_path("claude") {
            assert_eq!(p.file_name().and_then(|n| n.to_str()), Some("claude"));
        }
    }

    #[test]
    fn generated_mcp_path_ends_with_managed_json() {
        if let Some(p) = generated_mcp_path() {
            let s = p.to_string_lossy();
            assert!(s.contains("generated"), "must contain 'generated': {s}");
            assert!(s.ends_with("managed.json"), "must end with managed.json: {s}");
        }
    }

    #[test]
    fn all_paths_descend_from_gal_home() {
        if let Some(home) = gal_home() {
            for (label, path) in [
                ("plugins_root", gal_plugins_root()),
                ("data_root", gal_data_root()),
                ("cache_root", gal_cache_root()),
                ("install_state", install_state_path()),
                ("executor_routing", executor_routing_path()),
                ("machine_config", machine_config_path()),
                ("generated_mcp", generated_mcp_path()),
            ] {
                if let Some(p) = path {
                    assert!(
                        p.starts_with(&home),
                        "{label} ({}) must be under gal_home ({})",
                        p.display(),
                        home.display()
                    );
                }
            }
        }
    }
}
