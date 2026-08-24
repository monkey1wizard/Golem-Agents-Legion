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

// ── local / personal paths ────────────────────────────────────────────────

/// `~/.gal/local`
///
/// Root of the machine-local personal content home. GAL reads this directory
/// as a render source and never writes or deletes content here.
pub fn gal_local_root() -> Option<PathBuf> {
    gal_home().map(|h| h.join("local"))
}

/// `~/.gal/local/skills`
///
/// Personal skill directory scanned by canonical-root render when the
/// personal layer is enabled (`personalLayer.enabled` in config.json).
pub fn gal_local_skills_root() -> Option<PathBuf> {
    gal_home().map(|h| h.join("local").join("skills"))
}

/// `~/.gal/local/mcp.json`
///
/// Personal MCP server manifest merged into the canonical `.mcp.json`
/// during render (core-wins collision policy).
pub fn gal_local_mcp_path() -> Option<PathBuf> {
    gal_home().map(|h| h.join("local").join("mcp.json"))
}

/// `~/.gal/local/conventions`
///
/// Personal coding-style convention files (`<lang>.md`), scanned by the
/// repo-adapter render selector as source 2 alongside gal-core conventions.
pub fn gal_local_conventions_root() -> Option<PathBuf> {
    gal_home().map(|h| h.join("local").join("conventions"))
}

// ── state paths (explicit) ────────────────────────────────────────────────────

/// `~/.gal/state`
pub fn gal_state_root() -> Option<PathBuf> {
    gal_home().map(|h| h.join("state"))
}

/// `~/.gal/state/plugins.lock.json`
///
/// The canonical catalog lockfile written by `gal resolve-catalog` and consumed
/// read-only by the install/projection layers.
pub fn plugins_lock_path() -> Option<PathBuf> {
    gal_home().map(|h| h.join("state").join("plugins.lock.json"))
}

/// `~/.gal/state/gal-self.json`
///
/// Registry for GAL's own managed artifacts (empty-shell foundation; not yet
/// read or written by any production path).
pub fn gal_self_registry_path() -> Option<PathBuf> {
    gal_home().map(|h| h.join("state").join("gal-self.json"))
}

// ── agent state paths ─────────────────────────────────────────────────────────

/// `~/.claude/plugins/installed_plugins.json`
///
/// Claude installed plugin state (v2 schema: `{plugins:{"name@mkt":[...]}}`).
pub fn claude_installed_plugins_path() -> Option<PathBuf> {
    home_dir().map(|h| {
        h.join(".claude")
            .join("plugins")
            .join("installed_plugins.json")
    })
}

/// `~/.claude/plugins/cache/gal`
///
/// Claude's version-keyed cache for GAL's local directory-source plugin
/// (`cache/gal/<plugin>/<version>/`). Claude Code manages this cache; GAL never
/// writes it. `gal doctor` flags it when it is older than the canonical root and
/// advises `gal refresh` plus a Claude restart so the stale copy is replaced.
pub fn claude_plugins_cache_dir() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".claude").join("plugins").join("cache").join("gal"))
}

/// `~/.claude.json`
///
/// Claude top-level config (contains `mcpServers` entries).
pub fn claude_config_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".claude.json"))
}

/// `~/.codex/config.toml`
///
/// Codex config file; GAL reads the MCP section only.
pub fn codex_config_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".codex").join("config.toml"))
}

/// `~/.claude/plugins/cache`
///
/// Root of Claude's installed-plugin cache (all plugins, not just GAL's own —
/// contrast `claude_plugins_cache_dir()`, which is scoped to `.../cache/gal`).
/// Scanned read-only by the installed-skill detector for official
/// third-party plugin skills (e.g. an installed Flutter/Dart plugin).
pub fn claude_plugins_cache_root() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".claude").join("plugins").join("cache"))
}

/// `~/.agents/skills`
///
/// Shared skill root read by Codex, OpenCode, and Copilot (the common
/// `~/.agents/skills` namespace). Scanned read-only by the installed-skill
/// detector.
pub fn shared_agents_skills_root() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".agents").join("skills"))
}

/// `~/.copilot/skills`
///
/// Copilot CLI's own skill root. Scanned read-only by the installed-skill
/// detector.
pub fn copilot_skills_root() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".copilot").join("skills"))
}

/// `~/.gemini/antigravity-cli/skills`
///
/// Antigravity CLI (agy)'s skill root. Scanned read-only by the
/// installed-skill detector.
pub fn agy_skills_root() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".gemini").join("antigravity-cli").join("skills"))
}

// ── generated paths ───────────────────────────────────────────────────────────

/// `~/.gal/plugins/gal/.mcp.json` — the canonical MCP manifest written by
/// `gal refresh` into the plugin root.
///
/// This is the only MCP file gal produces. gal declares and consumes its own
/// MCP servers and manages no host's live MCP config.
pub fn canonical_mcp_path() -> Option<PathBuf> {
    gal_plugin_root("gal").map(|r| r.join(".mcp.json"))
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
    fn claude_installed_plugins_path_under_claude_plugins() {
        if let Some(p) = claude_installed_plugins_path() {
            let s = p.to_string_lossy();
            assert!(s.contains(".claude"), "must be under .claude: {s}");
            assert!(
                s.ends_with("installed_plugins.json"),
                "must end with installed_plugins.json: {s}"
            );
        }
    }

    #[test]
    fn claude_plugins_cache_dir_under_claude_plugins_cache() {
        if let Some(p) = claude_plugins_cache_dir() {
            let s = p.to_string_lossy().replace('\\', "/");
            assert!(s.contains(".claude"), "must be under .claude: {s}");
            assert!(
                s.ends_with(".claude/plugins/cache/gal"),
                "must end with .claude/plugins/cache/gal: {s}"
            );
        }
    }

    #[test]
    fn claude_config_path_ends_with_claude_json() {
        if let Some(p) = claude_config_path() {
            let s = p.to_string_lossy();
            assert!(
                s.ends_with(".claude.json"),
                "must end with .claude.json: {s}"
            );
        }
    }

    #[test]
    fn codex_config_path_under_codex_dir() {
        if let Some(p) = codex_config_path() {
            let s = p.to_string_lossy();
            assert!(s.contains(".codex"), "must be under .codex: {s}");
            assert!(s.ends_with("config.toml"), "must end with config.toml: {s}");
        }
    }

    #[test]
    fn all_agent_state_paths_under_home() {
        if let Some(home) = home_dir() {
            for (label, path) in [
                ("claude_installed_plugins", claude_installed_plugins_path()),
                ("claude_config", claude_config_path()),
                ("codex_config", codex_config_path()),
            ] {
                if let Some(p) = path {
                    assert!(
                        p.starts_with(&home),
                        "{label} ({}) must be under home ({})",
                        p.display(),
                        home.display()
                    );
                }
            }
        }
    }

    #[test]
    fn canonical_mcp_path_sits_in_the_gal_plugin_root() {
        if let Some(p) = canonical_mcp_path() {
            let s = p.to_string_lossy().replace('\\', "/");
            assert!(
                s.ends_with("plugins/gal/.mcp.json"),
                "must end with plugins/gal/.mcp.json: {s}"
            );
        }
    }

    #[test]
    fn gal_local_root_leaf_is_local() {
        if let Some(p) = gal_local_root() {
            assert_eq!(p.file_name().and_then(|n| n.to_str()), Some("local"));
        }
    }

    #[test]
    fn gal_local_skills_root_ends_with_skills_under_local() {
        if let Some(p) = gal_local_skills_root() {
            let s = p.to_string_lossy();
            assert!(s.contains("local"), "must contain 'local': {s}");
            assert!(
                s.ends_with("skills") || s.ends_with("skills\\"),
                "must end with 'skills': {s}"
            );
        }
    }

    #[test]
    fn gal_local_mcp_path_ends_with_mcp_json_under_local() {
        if let Some(p) = gal_local_mcp_path() {
            let s = p.to_string_lossy();
            assert!(s.contains("local"), "must be under local: {s}");
            assert!(s.ends_with("mcp.json"), "must end with mcp.json: {s}");
        }
    }

    #[test]
    fn gal_local_conventions_root_ends_with_conventions_under_local() {
        if let Some(p) = gal_local_conventions_root() {
            let s = p.to_string_lossy();
            assert!(s.contains("local"), "must be under local: {s}");
            assert!(
                s.ends_with("conventions") || s.ends_with("conventions\\"),
                "must end with 'conventions': {s}"
            );
        }
    }

    #[test]
    fn claude_plugins_cache_root_ends_with_cache_not_gal() {
        if let Some(p) = claude_plugins_cache_root() {
            let s = p.to_string_lossy().replace('\\', "/");
            assert!(s.contains(".claude"), "must be under .claude: {s}");
            assert!(
                s.ends_with(".claude/plugins/cache"),
                "must end with .claude/plugins/cache (not the gal-scoped subdir): {s}"
            );
        }
    }

    #[test]
    fn shared_agents_skills_root_ends_with_agents_skills() {
        if let Some(p) = shared_agents_skills_root() {
            let s = p.to_string_lossy().replace('\\', "/");
            assert!(
                s.ends_with(".agents/skills"),
                "must end with .agents/skills: {s}"
            );
        }
    }

    #[test]
    fn copilot_skills_root_ends_with_copilot_skills() {
        if let Some(p) = copilot_skills_root() {
            let s = p.to_string_lossy().replace('\\', "/");
            assert!(
                s.ends_with(".copilot/skills"),
                "must end with .copilot/skills: {s}"
            );
        }
    }

    #[test]
    fn agy_skills_root_ends_with_antigravity_cli_skills() {
        if let Some(p) = agy_skills_root() {
            let s = p.to_string_lossy().replace('\\', "/");
            assert!(
                s.ends_with(".gemini/antigravity-cli/skills"),
                "must end with .gemini/antigravity-cli/skills: {s}"
            );
        }
    }

    #[test]
    fn all_new_skill_roots_under_home() {
        if let Some(home) = home_dir() {
            for (label, path) in [
                ("claude_plugins_cache_root", claude_plugins_cache_root()),
                ("shared_agents_skills_root", shared_agents_skills_root()),
                ("copilot_skills_root", copilot_skills_root()),
                ("agy_skills_root", agy_skills_root()),
                ("gal_local_conventions_root", gal_local_conventions_root()),
            ] {
                if let Some(p) = path {
                    assert!(
                        p.starts_with(&home),
                        "{label} ({}) must be under home ({})",
                        p.display(),
                        home.display()
                    );
                }
            }
        }
    }

    #[test]
    fn gal_state_root_leaf_is_state() {
        if let Some(p) = gal_state_root() {
            assert_eq!(p.file_name().and_then(|n| n.to_str()), Some("state"));
        }
    }

    #[test]
    fn plugins_lock_path_ends_with_plugins_lock_json() {
        if let Some(p) = plugins_lock_path() {
            let s = p.to_string_lossy();
            assert!(s.contains("state"), "must be under state: {s}");
            assert!(
                s.ends_with("plugins.lock.json"),
                "must end with plugins.lock.json: {s}"
            );
        }
    }

    #[test]
    fn gal_self_registry_path_ends_with_gal_self_json_under_state() {
        if let Some(p) = gal_self_registry_path() {
            let s = p.to_string_lossy();
            assert!(s.contains("state"), "must be under state: {s}");
            assert!(
                s.ends_with("gal-self.json"),
                "must end with gal-self.json: {s}"
            );
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
                ("machine_config", machine_config_path()),
                ("canonical_mcp", canonical_mcp_path()),
                ("local_root", gal_local_root()),
                ("local_skills_root", gal_local_skills_root()),
                ("local_mcp_path", gal_local_mcp_path()),
                ("gal_state_root", gal_state_root()),
                ("plugins_lock_path", plugins_lock_path()),
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
