//! MCP (Model Context Protocol) config orchestration crate.
//!
//! Owns variable resolution, safe GAL-managed merge, per-host write, and the
//! `HealthCheck` impl for MCP config state.
//!
//! Dependency law: `mcp` → `base`. No GAL dep cycles. MCP host config
//! serializers live in the `serializers` submodule.

pub mod serializers;

mod health;
mod merge;
mod projection;
mod resolver;

pub use gal_foundation::mcp::{McpError, McpManifest, McpServer, Result};
pub use health::*;
pub use merge::*;
pub use projection::*;
pub use resolver::*;

use std::path::Path;

// gal mcp backend
// ─────────────────────────────────────────────────────────────────────────────

/// Result of a `gal mcp update` run.
pub struct McpUpdateReport {
    pub hosts_updated: Vec<String>,
    pub servers_written: usize,
    pub warnings: Vec<String>,
}

/// Run the MCP config update for all available providers.
///
/// Ports `Invoke-UpdateMcp` from `Update-Mcp.ps1`. Reads the manifest,
/// resolves `${VAR}` placeholders from `config.json` plus the process environment,
/// then writes each host config **non-destructively** (preserving user-owned entries
/// and, for Codex, non-MCP TOML content).
pub fn run_mcp_update(
    manifest_path: &Path,
) -> std::result::Result<McpUpdateReport, McpUpdateError> {
    use crate::serializers::claude_mcp::ClaudeDesktopMcpConfig;
    use crate::serializers::codex::CodexMcpConfig;
    use crate::serializers::copilot::CopilotCliMcpConfig;
    use crate::serializers::opencode::OpenCodeMcpConfig;
    use crate::serializers::McpHostConfig;

    let manifest = load_manifest(manifest_path)?;

    let resolver = McpVariableResolver::new(build_variable_map());
    let resolved = resolver.resolve_manifest(&manifest)?;

    let servers_written = resolved.servers.len();
    let mut hosts_updated = Vec::new();
    let mut warnings = Vec::new();

    // Claude Desktop (mcpServers) — non-destructive overlay.
    match ClaudeDesktopMcpConfig::from_manifest(&resolved) {
        Ok(cfg) => match serde_json::to_value(&cfg) {
            Ok(val) => {
                if let Some(dest) = claude_desktop_mcp_path() {
                    match write_json_provider_merged(&dest, "mcpServers", &val) {
                        Ok(_) => hosts_updated.push("claude".to_string()),
                        Err(e) => warnings.push(format!("claude write error: {e}")),
                    }
                }
            }
            Err(e) => warnings.push(format!("claude serialization error: {e}")),
        },
        Err(e) => warnings.push(format!("claude manifest error: {e}")),
    }

    // Copilot CLI (mcpServers) — non-destructive overlay.
    match CopilotCliMcpConfig::from_manifest(&resolved) {
        Ok(cfg) => match serde_json::to_value(&cfg) {
            Ok(val) => {
                if let Some(dest) = copilot_cli_mcp_path() {
                    match write_json_provider_merged(&dest, "mcpServers", &val) {
                        Ok(_) => hosts_updated.push("copilot".to_string()),
                        Err(e) => warnings.push(format!("copilot write error: {e}")),
                    }
                }
            }
            Err(e) => warnings.push(format!("copilot serialization error: {e}")),
        },
        Err(e) => warnings.push(format!("copilot manifest error: {e}")),
    }

    // OpenCode (mcp) — non-destructive overlay, preserves other top-level keys.
    match OpenCodeMcpConfig::from_manifest(&resolved) {
        Ok(cfg) => match serde_json::to_value(&cfg) {
            Ok(val) => {
                if let Some(dest) = opencode_config_path() {
                    match write_json_provider_merged(&dest, "mcp", &val) {
                        Ok(_) => hosts_updated.push("opencode".to_string()),
                        Err(e) => warnings.push(format!("opencode write error: {e}")),
                    }
                }
            }
            Err(e) => warnings.push(format!("opencode serialization error: {e}")),
        },
        Err(e) => warnings.push(format!("opencode manifest error: {e}")),
    }

    // Codex (TOML) — remove managed sections then append, preserving non-MCP.
    match CodexMcpConfig::from_manifest(&resolved) {
        Ok(cfg) => match cfg.to_config_string() {
            Ok(sections) => {
                if let Some(dest) = codex_config_path() {
                    let managed_names: std::collections::HashSet<String> =
                        resolved.servers.keys().cloned().collect();
                    match write_codex_merged(&dest, &managed_names, &sections) {
                        Ok(()) => hosts_updated.push("codex".to_string()),
                        Err(e) => warnings.push(format!("codex write error: {e}")),
                    }
                }
            }
            Err(e) => warnings.push(format!("codex serialization error: {e}")),
        },
        Err(e) => warnings.push(format!("codex manifest error: {e}")),
    }

    Ok(McpUpdateReport {
        hosts_updated,
        servers_written,
        warnings,
    })
}

pub(crate) fn ensure_parent(path: &Path) -> std::result::Result<(), McpUpdateError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn claude_desktop_mcp_path() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|d| {
            std::path::PathBuf::from(d)
                .join("Claude")
                .join("claude_desktop_config.json")
        })
    }
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("Claude")
                .join("claude_desktop_config.json")
        })
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        dirs::config_dir().map(|c| c.join("Claude").join("claude_desktop_config.json"))
    }
}

fn copilot_cli_mcp_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".copilot").join("mcp-config.json"))
}

fn codex_config_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex").join("config.toml"))
}

fn opencode_config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|c| c.join("opencode").join("opencode.json"))
}

/// Errors from the `gal mcp` backend that are not covered by `McpError`.
#[derive(Debug, thiserror::Error)]
pub enum McpUpdateError {
    #[error("manifest error: {0}")]
    Manifest(#[from] McpError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests (moved from gal-engine::mcp)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use gal_foundation::health::HealthCheck;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    // ── McpVariableResolver ──────────────────────────────────────────────────

    #[test]
    fn resolver_substitutes_known_var() {
        let mut vars = HashMap::new();
        vars.insert("HOME".to_string(), "/home/user".to_string());
        let r = McpVariableResolver::new(vars);
        assert_eq!(
            r.resolve_string("${HOME}/vault").unwrap(),
            "/home/user/vault"
        );
    }

    #[test]
    fn resolver_substitutes_multiple_vars() {
        let mut vars = HashMap::new();
        vars.insert("HOME".to_string(), "/home/user".to_string());
        vars.insert("PROJECT".to_string(), "myproject".to_string());
        let r = McpVariableResolver::new(vars);
        assert_eq!(
            r.resolve_string("${HOME}/projects/${PROJECT}").unwrap(),
            "/home/user/projects/myproject"
        );
    }

    #[test]
    fn resolver_rejects_unresolved_secret() {
        let r = McpVariableResolver::empty();
        assert!(matches!(
            r.resolve_string("${API_KEY}"),
            Err(McpError::UnresolvedSecret(_))
        ));
    }

    #[test]
    fn resolver_leaves_unresolved_non_secret_as_is() {
        let r = McpVariableResolver::empty();
        assert_eq!(r.resolve_string("${SOME_VAR}").unwrap(), "${SOME_VAR}");
    }

    // ── McpMerger ────────────────────────────────────────────────────────────

    #[test]
    fn merger_replaces_gal_managed() {
        let merger = McpMerger::new(vec!["github".to_string()]);
        let mut gal_servers = HashMap::new();
        gal_servers.insert(
            "github".to_string(),
            McpServer {
                server_type: None,
                url: Some("https://new.url".to_string()),
                command: None,
                args: None,
                env: None,
                headers: None,
            },
        );
        let gal = McpManifest {
            servers: gal_servers,
            inputs: None,
        };
        let mut ex_servers = HashMap::new();
        ex_servers.insert(
            "github".to_string(),
            McpServer {
                server_type: None,
                url: Some("https://old.url".to_string()),
                command: None,
                args: None,
                env: None,
                headers: None,
            },
        );
        let existing = McpManifest {
            servers: ex_servers,
            inputs: None,
        };
        let merged = merger.merge(&gal, Some(&existing));
        assert_eq!(
            merged.servers["github"].url.as_deref(),
            Some("https://new.url")
        );
    }

    #[test]
    fn merger_preserves_user_owned() {
        let merger = McpMerger::new(vec!["github".to_string()]);
        let mut gal_servers = HashMap::new();
        gal_servers.insert(
            "github".to_string(),
            McpServer {
                server_type: None,
                url: Some("https://new.url".to_string()),
                command: None,
                args: None,
                env: None,
                headers: None,
            },
        );
        let gal = McpManifest {
            servers: gal_servers,
            inputs: None,
        };
        let mut ex_servers = HashMap::new();
        ex_servers.insert(
            "my-tool".to_string(),
            McpServer {
                server_type: None,
                command: Some("my-tool".to_string()),
                args: None,
                env: None,
                url: None,
                headers: None,
            },
        );
        let existing = McpManifest {
            servers: ex_servers,
            inputs: None,
        };
        let merged = merger.merge(&gal, Some(&existing));
        assert_eq!(merged.servers.len(), 2);
        assert!(merged.servers.contains_key("my-tool"));
    }

    #[test]
    fn merger_idempotent() {
        let merger = McpMerger::new(vec!["github".to_string()]);
        let mut gal_servers = HashMap::new();
        gal_servers.insert(
            "github".to_string(),
            McpServer {
                server_type: None,
                url: Some("https://url".to_string()),
                command: None,
                args: None,
                env: None,
                headers: None,
            },
        );
        let gal = McpManifest {
            servers: gal_servers,
            inputs: None,
        };
        let m1 = merger.merge(&gal, None);
        let m2 = merger.merge(&gal, Some(&m1));
        assert_eq!(m1.servers.len(), m2.servers.len());
    }

    // ── check_plugin_root_complete ───────────────────────────────────────────

    #[test]
    fn plugin_root_missing_commands_fails() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("agents")).unwrap();
        fs::create_dir(tmp.path().join("skills")).unwrap();
        assert!(matches!(
            check_plugin_root_complete(tmp.path()),
            Err(McpError::PluginRootIncomplete(_))
        ));
    }

    #[test]
    fn plugin_root_complete_ok() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("commands")).unwrap();
        fs::create_dir(tmp.path().join("agents")).unwrap();
        fs::create_dir(tmp.path().join("skills")).unwrap();
        assert!(check_plugin_root_complete(tmp.path()).is_ok());
    }

    #[test]
    fn save_projection_fails_on_incomplete_root() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("agents")).unwrap();
        fs::create_dir(tmp.path().join("skills")).unwrap();
        let out = tmp.path().join("out.json");
        let proj = McpProjection {
            schema_version: 1,
            mcp_servers: HashMap::new(),
            inputs: None,
            metadata: McpProjectionMetadata {
                ownership: "gal-managed".into(),
                generated_at: "2026-06-09T00:00:00Z".into(),
                generated_by: "test".into(),
                secret_bearing: false,
                source_files: HashMap::new(),
                preservation: "test".into(),
            },
        };
        assert!(save_projection(&out, &proj, tmp.path()).is_err());
        assert!(!out.exists());
    }

    #[test]
    fn save_projection_ok_on_complete_root() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("commands")).unwrap();
        fs::create_dir(tmp.path().join("agents")).unwrap();
        fs::create_dir(tmp.path().join("skills")).unwrap();
        let out = tmp.path().join("out.json");
        let proj = McpProjection {
            schema_version: 1,
            mcp_servers: HashMap::new(),
            inputs: None,
            metadata: McpProjectionMetadata {
                ownership: "gal-managed".into(),
                generated_at: "2026-06-09T00:00:00Z".into(),
                generated_by: "test".into(),
                secret_bearing: false,
                source_files: HashMap::new(),
                preservation: "test".into(),
            },
        };
        assert!(save_projection(&out, &proj, tmp.path()).is_ok());
        assert!(out.exists());
    }

    // ── McpProjectionHealthCheck ─────────────────────────────────────────────

    #[test]
    fn health_check_warns_when_projection_missing() {
        use gal_foundation::health::Severity;
        let tmp = TempDir::new().unwrap();
        let check = McpProjectionHealthCheck::with_path(tmp.path().join("nonexistent.json"));
        let findings = check.check();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
    }

    #[test]
    fn health_check_ok_when_valid_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("managed.json");
        fs::write(
            &path,
            r#"{"schemaVersion":1,"mcpServers":{},"_metadata":{}}"#,
        )
        .unwrap();
        let check = McpProjectionHealthCheck::with_path(path);
        assert!(check.check().is_empty());
    }

    #[test]
    fn health_check_error_on_invalid_json() {
        use gal_foundation::health::Severity;
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("managed.json");
        fs::write(&path, "not json").unwrap();
        let check = McpProjectionHealthCheck::with_path(path);
        let findings = check.check();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Error);
    }

    // ── run_mcp_update orchestration: variable resolution ─────────────────────

    #[test]
    fn build_variable_map_resolves_secret_from_env() {
        // Defect-1 regression: an empty resolver hard-failed on secret-named
        // placeholders. The map must pick up a secret from the environment so
        // resolve_manifest succeeds instead of returning UnresolvedSecret.
        let key = "GAL_TEST_MCP_API_KEY";
        std::env::set_var(key, "sekret");
        let map = build_variable_map_from(None);
        let r = McpVariableResolver::new(map);
        assert_eq!(
            r.resolve_string("${GAL_TEST_MCP_API_KEY}").unwrap(),
            "sekret"
        );
        std::env::remove_var(key);
    }

    #[test]
    fn build_variable_map_reads_config_json_path_via_registry() {
        let tmp = TempDir::new().unwrap();
        let cfg = tmp.path().join("config.json");
        fs::write(&cfg, r#"{"galSkills":"/home/test/.gal/skills"}"#).unwrap();

        let map = build_variable_map_from(Some(&cfg));
        assert_eq!(
            map.get("GAL_SKILLS").map(String::as_str),
            Some("/home/test/.gal/skills")
        );
    }

    #[test]
    fn build_variable_map_reads_secrets_verbatim() {
        let tmp = TempDir::new().unwrap();
        let cfg = tmp.path().join("config.json");
        fs::write(&cfg, r#"{"secrets":{"CONTEXT7_API_KEY":"ctx-123"}}"#).unwrap();

        let map = build_variable_map_from(Some(&cfg));
        assert_eq!(
            map.get("CONTEXT7_API_KEY").map(String::as_str),
            Some("ctx-123")
        );
    }

    #[test]
    fn build_variable_map_reads_config_json_only() {
        let tmp = TempDir::new().unwrap();
        let cfg = tmp.path().join("config.json");
        fs::write(&cfg, r#"{"galSkills":"/Users/test/.gal/skills"}"#).unwrap();

        let map = build_variable_map_from(Some(&cfg));
        assert_eq!(
            map.get("GAL_SKILLS").map(String::as_str),
            Some("/Users/test/.gal/skills")
        );
    }

    // ── write_json_provider_merged (non-destructive) ──────────────────────────

    #[test]
    fn json_merge_preserves_user_entries_and_other_keys() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("claude_desktop_config.json");
        // Pre-existing config: a user MCP server + an unrelated top-level key.
        fs::write(
            &dest,
            r#"{"mcpServers":{"user-server":{"command":"mine"}},"theme":"dark"}"#,
        )
        .unwrap();

        let generated = serde_json::json!({
            "mcpServers": { "gal-managed": { "command": "npx", "args": [] } }
        });
        let n = write_json_provider_merged(&dest, "mcpServers", &generated).unwrap();
        assert_eq!(n, 1);

        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&dest).unwrap()).unwrap();
        // User server preserved.
        assert_eq!(written["mcpServers"]["user-server"]["command"], "mine");
        // Managed server added.
        assert_eq!(written["mcpServers"]["gal-managed"]["command"], "npx");
        // Unrelated top-level key preserved.
        assert_eq!(written["theme"], "dark");
    }

    #[test]
    fn json_merge_creates_file_when_absent() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("sub").join("mcp-config.json");
        let generated = serde_json::json!({ "mcp": { "s": { "type": "local" } } });
        let n = write_json_provider_merged(&dest, "mcp", &generated).unwrap();
        assert_eq!(n, 1);
        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&dest).unwrap()).unwrap();
        assert_eq!(written["mcp"]["s"]["type"], "local");
    }

    #[test]
    fn json_merge_refuses_to_overwrite_non_object() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("bad.json");
        fs::write(&dest, "[1,2,3]").unwrap();
        let generated = serde_json::json!({ "mcpServers": { "x": {} } });
        let err = write_json_provider_merged(&dest, "mcpServers", &generated);
        assert!(err.is_err(), "must not clobber a non-object config");
        // Original content untouched.
        assert_eq!(fs::read_to_string(&dest).unwrap(), "[1,2,3]");
    }

    // ── Codex TOML section parsing + non-destructive merge ────────────────────

    #[test]
    fn codex_table_server_name_parses_bare_and_quoted() {
        assert_eq!(
            codex_table_server_name("[mcp_servers.memory]").as_deref(),
            Some("memory")
        );
        assert_eq!(
            codex_table_server_name("[mcp_servers.memory.env]").as_deref(),
            Some("memory")
        );
        assert_eq!(
            codex_table_server_name(r#"[mcp_servers."upstash/context7"]"#).as_deref(),
            Some("upstash/context7")
        );
        assert_eq!(codex_table_server_name("[model]"), None);
        assert_eq!(codex_table_server_name("command = \"npx\""), None);
    }

    #[test]
    fn codex_remove_preserves_non_mcp_and_user_servers() {
        let raw = "[model]\nname = \"gpt\"\n\n[mcp_servers.gal_managed]\ncommand = \"npx\"\n\n[mcp_servers.user_kept]\ncommand = \"mine\"\n";
        let managed: std::collections::HashSet<String> =
            ["gal_managed".to_string()].into_iter().collect();
        let out = remove_codex_managed_sections(raw, &managed);
        assert!(out.contains("[model]"), "non-MCP [model] must survive");
        assert!(out.contains("name = \"gpt\""));
        assert!(
            out.contains("[mcp_servers.user_kept]"),
            "user MCP server must survive"
        );
        assert!(
            !out.contains("gal_managed"),
            "managed section must be removed"
        );
    }

    #[test]
    fn codex_write_merge_appends_and_preserves() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("config.toml");
        fs::write(&dest, "[model]\nname = \"gpt\"\n").unwrap();
        let managed: std::collections::HashSet<String> =
            ["memory".to_string()].into_iter().collect();
        let sections = "[mcp_servers.memory]\r\ncommand = \"npx\"\r\n";
        write_codex_merged(&dest, &managed, sections).unwrap();
        let written = fs::read_to_string(&dest).unwrap();
        assert!(written.contains("[model]"), "non-MCP content preserved");
        assert!(
            written.contains("[mcp_servers.memory]"),
            "managed section appended"
        );
    }

    #[test]
    fn run_mcp_update_does_not_treat_agy_as_an_mcp_provider() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let appdata = tmp.path().join("appdata");
        let xdg = tmp.path().join("xdg-config");
        let gal_home = home.join(".gal");
        let manifest_path = gal_home.join("generated").join("mcp").join("managed.json");

        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::create_dir_all(home.join(".copilot")).unwrap();
        fs::create_dir_all(appdata.join("Claude")).unwrap();
        fs::create_dir_all(xdg.join("opencode")).unwrap();
        fs::create_dir_all(gal_home.join("config")).unwrap();

        fs::write(
            &manifest_path,
            r#"{
  "servers": {
    "memory": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@mcp/server-memory"]
    }
  }
}"#,
        )
        .unwrap();

        #[cfg(windows)]
        unsafe {
            std::env::set_var("USERPROFILE", &home);
            std::env::set_var("HOME", &home);
            std::env::set_var("APPDATA", &appdata);
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        #[cfg(not(windows))]
        unsafe {
            std::env::set_var("HOME", &home);
            std::env::set_var("XDG_CONFIG_HOME", &xdg);
            std::env::remove_var("APPDATA");
        }

        let report = run_mcp_update(&manifest_path).unwrap();

        assert!(report.hosts_updated.contains(&"codex".to_string()));
        assert!(!report.hosts_updated.contains(&"agy".to_string()));
        assert!(report.warnings.iter().all(|w| !w.contains("agy")));
    }
}
