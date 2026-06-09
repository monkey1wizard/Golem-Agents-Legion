//! Claude provider: MCP config serializer + skill surface projection.
//!
//! # MCP config (ClaudeDesktopMcpConfig)
//! Converts portable `.mcp.json` manifest to Claude Desktop's config format.
//! On Windows, wraps env vars into a PowerShell wrapper for stdio servers.
//!
//! # Skill surface (ClaudeSkillProjection)
//! Creates and maintains `~/.claude/skills/gal` → canonical root.
//! This is the GAL-owned persistent projection surface loaded by Claude Code
//! for skills (e.g. `doc-sync`). Never touches `~/.claude/plugins/gal` (oracle
//! legacy, removed on every refresh).

use base::mcp::{McpManifest, McpServer, Result};
use crate::{has_unresolved_secrets, McpProviderConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

/// Claude Desktop server entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeDesktopServerEntry {
    pub command: String,
    pub args: Vec<String>,
}

/// Claude Desktop MCP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopMcpConfig {
    pub mcp_servers: HashMap<String, ClaudeDesktopServerEntry>,
}

impl McpProviderConfig for ClaudeDesktopMcpConfig {
    fn from_manifest(manifest: &McpManifest) -> Result<Self> {
        let mut mcp_servers = HashMap::new();

        for (server_name, server_config) in &manifest.servers {
            // Skip HTTP servers (no command)
            if server_config.command.is_none() {
                continue;
            }

            // Skip servers with type=http explicitly
            if let Some(ref server_type) = server_config.server_type {
                if server_type == "http" {
                    continue;
                }
            }

            // Skip servers with unresolved secrets
            if has_unresolved_secrets(server_config) {
                continue;
            }

            // Convert to Claude Desktop format
            let entry = convert_to_claude_entry(server_name, server_config);
            mcp_servers.insert(server_name.clone(), entry);
        }

        Ok(Self { mcp_servers })
    }

    fn to_json_pretty(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

/// Convert MCP server config to Claude Desktop entry
///
/// On Windows, wraps env vars into PowerShell wrapper
fn convert_to_claude_entry(
    _server_name: &str,
    server_config: &McpServer,
) -> ClaudeDesktopServerEntry {
    let command = server_config.command.as_ref().unwrap().clone();
    let args = server_config
        .args.clone()
        .unwrap_or_default();

    // Check if we need PowerShell wrapper (Windows only, and env vars present)
    if cfg!(target_os = "windows") {
        if let Some(ref env) = server_config.env {
            if !env.is_empty() {
                return wrap_with_powershell(&command, &args, env);
            }
        }
    }

    ClaudeDesktopServerEntry { command, args }
}

/// Wrap command with PowerShell to set environment variables
///
/// Generates:
/// ```powershell
/// powershell -NoProfile -Command "$env:VAR='value'; & 'command' 'arg1' 'arg2'"
/// ```
fn wrap_with_powershell(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
) -> ClaudeDesktopServerEntry {
    let mut script_parts = Vec::new();

    // Add env var assignments
    for (name, value) in env {
        let escaped_value = value.replace('\'', "''");
        script_parts.push(format!("$env:{} = '{}'", name, escaped_value));
    }

    // Build command invocation
    let mut command_parts = vec!["&".to_string()];
    command_parts.push(format!("'{}'", command.replace('\'', "''")));
    for arg in args {
        command_parts.push(format!("'{}'", arg.replace('\'', "''")));
    }
    script_parts.push(command_parts.join(" "));

    ClaudeDesktopServerEntry {
        command: "powershell".to_string(),
        args: vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            script_parts.join("; "),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_simple_server() {
        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("node".to_string()),
                args: Some(vec!["index.js".to_string()]),
                env: None,
                url: None,
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();

        assert_eq!(config.mcp_servers.len(), 1);
        assert!(config.mcp_servers.contains_key("test-server"));

        let entry = &config.mcp_servers["test-server"];
        assert_eq!(entry.command, "node");
        assert_eq!(entry.args, vec!["index.js"]);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_wrap_with_env_on_windows() {
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "secret123".to_string());

        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("node".to_string()),
                args: Some(vec!["index.js".to_string()]),
                env: Some(env),
                url: None,
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
        let entry = &config.mcp_servers["test-server"];

        assert_eq!(entry.command, "powershell");
        assert_eq!(entry.args[0], "-NoProfile");
        assert_eq!(entry.args[1], "-Command");
        assert!(entry.args[2].contains("$env:API_KEY = 'secret123'"));
        assert!(entry.args[2].contains("& 'node' 'index.js'"));
    }

    #[test]
    fn test_skip_http_server() {
        let mut servers = HashMap::new();
        servers.insert(
            "http-server".to_string(),
            McpServer {
                server_type: Some("http".to_string()),
                command: None,
                args: None,
                env: None,
                url: Some("http://example.com".to_string()),
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
        assert_eq!(config.mcp_servers.len(), 0);
    }

    #[test]
    fn test_skip_unresolved_secret() {
        let mut servers = HashMap::new();
        servers.insert(
            "secret-server".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("${API_KEY}".to_string()),
                args: None,
                env: None,
                url: None,
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
        assert_eq!(config.mcp_servers.len(), 0);
    }
}

// ─── Claude skill surface projection ────────────────────────────────────────

/// Error type for Claude skill surface projection operations.
#[derive(Debug, Error)]
pub enum ClaudeSkillError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("home directory not found")]
    NoHome,
    #[error("skills directory creation failed: {0}")]
    SkillsDirCreation(String),
    #[error("skill surface link operation failed: {0}")]
    LinkOp(String),
}

/// Manages `~/.claude/skills/gal` → canonical root (T-003 / R-03).
///
/// On Windows: NTFS directory junction (`mklink /J`).
/// On Unix: symlink (`std::os::unix::fs::symlink`).
///
/// Invariants:
/// - Never reads or writes `~/.claude/plugins/gal` (legacy, oracle removes it).
/// - `apply()` is idempotent: removes any existing link before recreating.
/// - `remove()` removes only the link, never the canonical root target.
pub struct ClaudeSkillProjection {
    /// Rendered canonical plugin root (`~/.gal/plugins/gal`).
    pub canonical_root: PathBuf,
    /// Skill surface link (`~/.claude/skills/gal`).
    pub skill_surface: PathBuf,
}

impl ClaudeSkillProjection {
    /// Construct using the real home directory.
    pub fn new(canonical_root: PathBuf) -> std::result::Result<Self, ClaudeSkillError> {
        let home = dirs::home_dir().ok_or(ClaudeSkillError::NoHome)?;
        let skill_surface = home.join(".claude").join("skills").join("gal");
        Ok(Self {
            canonical_root,
            skill_surface,
        })
    }

    /// Create or update `~/.claude/skills/gal` → `canonical_root`.
    pub fn apply(&self) -> std::result::Result<(), ClaudeSkillError> {
        // Ensure ~/.claude/skills/ exists.
        let skills_dir = self.skill_surface.parent().ok_or_else(|| {
            ClaudeSkillError::SkillsDirCreation("invalid skill surface path".to_string())
        })?;
        fs::create_dir_all(skills_dir).map_err(|e| {
            ClaudeSkillError::SkillsDirCreation(format!("{}: {e}", skills_dir.display()))
        })?;

        // Remove any existing link at the surface path.
        self.remove_link()?;

        // Create new link.
        self.create_link()
    }

    /// Return `true` if the skill surface exists (link resolves).
    pub fn verify_aligned(&self) -> bool {
        self.skill_surface.exists()
    }

    /// Remove `~/.claude/skills/gal` (link only, never the canonical root).
    pub fn remove(&self) -> std::result::Result<(), ClaudeSkillError> {
        self.remove_link()
    }

    fn remove_link(&self) -> std::result::Result<(), ClaudeSkillError> {
        // Nothing to remove.
        if !self.skill_surface.exists()
            && !base::platform::is_symlink_or_junction(&self.skill_surface)
        {
            return Ok(());
        }

        // Only remove when the surface is the link/junction we own — never delete
        // a real directory. Windows guards on is_dir (junction), Unix on is_symlink.
        #[cfg(windows)]
        let should_remove = self.skill_surface.is_dir();
        #[cfg(not(windows))]
        let should_remove = self.skill_surface.is_symlink();

        if should_remove {
            base::platform::remove_dir_link(&self.skill_surface)
                .map_err(|e| ClaudeSkillError::LinkOp(format!("link removal failed: {e}")))?;
        }

        Ok(())
    }

    fn create_link(&self) -> std::result::Result<(), ClaudeSkillError> {
        base::platform::create_dir_link(&self.canonical_root, &self.skill_surface)
            .map_err(|e| ClaudeSkillError::LinkOp(format!("link creation failed: {e}")))
    }
}

#[cfg(test)]
mod skill_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_claude_skill_projection_new_paths() {
        let temp = TempDir::new().unwrap();
        let canonical = temp.path().join("canonical");
        fs::create_dir_all(&canonical).unwrap();

        let proj = ClaudeSkillProjection {
            canonical_root: canonical.clone(),
            skill_surface: temp.path().join(".claude").join("skills").join("gal"),
        };

        assert!(proj.canonical_root.ends_with("canonical"));
        assert!(proj.skill_surface.to_string_lossy().contains("skills"));
        assert!(proj.skill_surface.to_string_lossy().ends_with("gal"));
    }

    #[test]
    fn test_apply_creates_skill_surface() {
        let temp = TempDir::new().unwrap();
        let canonical = temp.path().join("canonical");
        fs::create_dir_all(&canonical).unwrap();

        let skill_surface = temp.path().join(".claude").join("skills").join("gal");

        let proj = ClaudeSkillProjection {
            canonical_root: canonical.clone(),
            skill_surface: skill_surface.clone(),
        };

        proj.apply().unwrap();

        // Surface must exist (link resolves to canonical which exists).
        assert!(skill_surface.exists(), "skill surface must exist after apply");
    }

    #[test]
    fn test_apply_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let canonical = temp.path().join("canonical");
        fs::create_dir_all(&canonical).unwrap();

        let skill_surface = temp.path().join(".claude").join("skills").join("gal");

        let proj = ClaudeSkillProjection {
            canonical_root: canonical.clone(),
            skill_surface: skill_surface.clone(),
        };

        proj.apply().unwrap();
        // Second apply must not error.
        proj.apply().unwrap();

        assert!(skill_surface.exists());
    }

    #[test]
    fn test_verify_aligned_false_when_absent() {
        let temp = TempDir::new().unwrap();
        let proj = ClaudeSkillProjection {
            canonical_root: temp.path().join("canonical"),
            skill_surface: temp.path().join(".claude").join("skills").join("gal"),
        };
        assert!(!proj.verify_aligned());
    }

    #[test]
    fn test_remove_after_apply() {
        let temp = TempDir::new().unwrap();
        let canonical = temp.path().join("canonical");
        fs::create_dir_all(&canonical).unwrap();

        let skill_surface = temp.path().join(".claude").join("skills").join("gal");

        let proj = ClaudeSkillProjection {
            canonical_root: canonical.clone(),
            skill_surface: skill_surface.clone(),
        };

        proj.apply().unwrap();
        assert!(skill_surface.exists());

        proj.remove().unwrap();
        // After remove, the surface link is gone but canonical root is untouched.
        assert!(!skill_surface.exists(), "skill surface must be gone after remove");
        assert!(canonical.exists(), "canonical root must survive remove");
    }

    #[test]
    fn test_remove_when_absent_is_noop() {
        let temp = TempDir::new().unwrap();
        let proj = ClaudeSkillProjection {
            canonical_root: temp.path().join("canonical"),
            skill_surface: temp.path().join(".claude").join("skills").join("gal"),
        };
        // Must not error when nothing exists.
        proj.remove().unwrap();
    }

    #[test]
    fn test_never_touches_legacy_plugins_path() {
        let temp = TempDir::new().unwrap();
        let canonical = temp.path().join("canonical");
        fs::create_dir_all(&canonical).unwrap();

        let skill_surface = temp.path().join(".claude").join("skills").join("gal");
        let legacy_plugins = temp.path().join(".claude").join("plugins").join("gal");

        let proj = ClaudeSkillProjection {
            canonical_root: canonical,
            skill_surface,
        };

        proj.apply().unwrap();
        // Legacy path must never be created.
        assert!(
            !legacy_plugins.exists(),
            "legacy ~/.claude/plugins/gal must never be touched"
        );
    }
}
