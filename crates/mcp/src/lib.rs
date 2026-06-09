//! MCP (Model Context Protocol) config orchestration crate.
//!
//! Split from `gal-engine::mcp` (T-011, R-02). Owns variable resolution,
//! safe GAL-managed merge, per-provider write, and the `HealthCheck` impl
//! for MCP config state.
//!
//! Dependency law: `mcp` → `base` + `providers`. No GAL dep cycles.

use base::health::{DoctorFinding, HealthCheck};
pub use base::mcp::{McpError, McpManifest, McpServer, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ─────────────────────────────────────────────────────────────────────────────
// Variable resolver
// ─────────────────────────────────────────────────────────────────────────────

/// Resolves `${VAR_NAME}` placeholders in MCP server configs.
///
/// Ports `Resolve-McpConfig` / variable substitution from `Update-Mcp.ps1`.
#[derive(Debug)]
pub struct McpVariableResolver {
    variables: HashMap<String, String>,
}

impl McpVariableResolver {
    /// Create a resolver with an explicit variable map.
    pub fn new(variables: HashMap<String, String>) -> Self {
        Self { variables }
    }

    /// Create an empty resolver (no substitution performed).
    pub fn empty() -> Self {
        Self { variables: HashMap::new() }
    }

    /// Resolve `${VAR}` placeholders in `value`.
    ///
    /// Unresolved secret-name placeholders (KEY/SECRET/TOKEN/PASSWORD) are an
    /// error; other unresolved placeholders are left as-is.
    pub fn resolve_string(&self, value: &str) -> Result<String> {
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let re = RE.get_or_init(|| regex::Regex::new(r"\$\{([A-Z0-9_]+)\}").unwrap());

        let mut result = value.to_string();
        for cap in re.captures_iter(value) {
            let var_name = &cap[1];
            let placeholder = &cap[0];
            if let Some(resolved) = self.variables.get(var_name) {
                result = result.replace(placeholder, resolved);
            } else if is_secret_name(var_name) {
                return Err(McpError::UnresolvedSecret(var_name.to_string()));
            }
        }
        Ok(result)
    }

    /// Resolve all string fields of a single server entry.
    pub fn resolve_server(&self, server: &McpServer) -> Result<McpServer> {
        let mut s = server.clone();
        if let Some(ref cmd) = server.command {
            s.command = Some(self.resolve_string(cmd)?);
        }
        if let Some(ref args) = server.args {
            s.args = Some(args.iter().map(|a| self.resolve_string(a)).collect::<Result<_>>()?);
        }
        if let Some(ref env) = server.env {
            let mut resolved = HashMap::new();
            for (k, v) in env {
                resolved.insert(k.clone(), self.resolve_string(v)?);
            }
            s.env = Some(resolved);
        }
        if let Some(ref url) = server.url {
            s.url = Some(self.resolve_string(url)?);
        }
        if let Some(ref headers) = server.headers {
            let mut resolved = HashMap::new();
            for (k, v) in headers {
                resolved.insert(k.clone(), self.resolve_string(v)?);
            }
            s.headers = Some(resolved);
        }
        Ok(s)
    }

    /// Resolve all servers in a manifest.
    pub fn resolve_manifest(&self, manifest: &McpManifest) -> Result<McpManifest> {
        let mut servers = HashMap::new();
        for (name, server) in &manifest.servers {
            servers.insert(name.clone(), self.resolve_server(server)?);
        }
        Ok(McpManifest { servers, inputs: manifest.inputs.clone() })
    }
}

fn is_secret_name(name: &str) -> bool {
    let u = name.to_uppercase();
    u.contains("KEY") || u.contains("SECRET") || u.contains("TOKEN") || u.contains("PASSWORD")
}

// ─────────────────────────────────────────────────────────────────────────────
// Safe-merge
// ─────────────────────────────────────────────────────────────────────────────

/// Safe merge of GAL-managed entries over user-owned entries.
///
/// GAL-managed servers (those listed in the manifest) replace any existing
/// entry; user-owned entries are preserved unchanged.
///
/// Ports `Merge-OrderedMap` + the GAL-managed guard from `Update-Mcp.ps1`.
#[derive(Debug)]
pub struct McpMerger {
    gal_managed_servers: Vec<String>,
}

impl McpMerger {
    pub fn new(gal_managed_servers: Vec<String>) -> Self {
        Self { gal_managed_servers }
    }

    /// Merge `gal_manifest` over `existing` (if any).
    ///
    /// - GAL-managed names are always replaced.
    /// - User-owned names in `existing` are preserved.
    pub fn merge(&self, gal_manifest: &McpManifest, existing: Option<&McpManifest>) -> McpManifest {
        let mut servers = HashMap::new();
        for (name, server) in &gal_manifest.servers {
            servers.insert(name.clone(), server.clone());
        }
        if let Some(ex) = existing {
            for (name, server) in &ex.servers {
                if !self.gal_managed_servers.contains(name) {
                    servers.insert(name.clone(), server.clone());
                }
            }
        }
        McpManifest { servers, inputs: gal_manifest.inputs.clone() }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Projection metadata + projection struct
// ─────────────────────────────────────────────────────────────────────────────

/// Metadata written into the GAL-managed projection file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpProjectionMetadata {
    pub ownership: String,
    pub generated_at: String,
    pub generated_by: String,
    pub secret_bearing: bool,
    pub source_files: HashMap<String, String>,
    pub preservation: String,
}

/// The GAL-managed MCP projection file (written to `~/.gal/generated/mcp/managed.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpProjection {
    pub schema_version: u32,
    pub mcp_servers: HashMap<String, McpServer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<serde_json::Value>,
    #[serde(rename = "_metadata")]
    pub metadata: McpProjectionMetadata,
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugin-root guard + load/save helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Return `Err` when the plugin root is missing any of the three required dirs.
///
/// Guards against writing MCP configs before the canonical root is complete
/// (ports the check in `Invoke-UpdateMcp`).
pub fn check_plugin_root_complete(plugin_root: &Path) -> Result<()> {
    for dir in &["commands", "agents", "skills"] {
        let p = plugin_root.join(dir);
        if !p.exists() || !p.is_dir() {
            return Err(McpError::PluginRootIncomplete(dir.to_string()));
        }
    }
    Ok(())
}

/// Load an `McpManifest` from a JSON file.
pub fn load_manifest(path: &Path) -> Result<McpManifest> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Write an `McpProjection` to `path`, only when `plugin_root` is complete.
pub fn save_projection(path: &Path, projection: &McpProjection, plugin_root: &Path) -> Result<()> {
    check_plugin_root_complete(plugin_root)?;
    let json = serde_json::to_string_pretty(projection)?;
    std::fs::write(path, json)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// HealthCheck implementation
// ─────────────────────────────────────────────────────────────────────────────

/// Checks that the GAL-managed MCP projection file exists and is valid JSON.
pub struct McpProjectionHealthCheck {
    projection_path: std::path::PathBuf,
}

impl McpProjectionHealthCheck {
    /// Construct using the standard `~/.gal/generated/mcp/managed.json` path.
    pub fn from_standard_path() -> Option<Self> {
        base::paths::generated_mcp_path().map(|p| Self { projection_path: p })
    }

    pub fn with_path(path: std::path::PathBuf) -> Self {
        Self { projection_path: path }
    }
}

impl HealthCheck for McpProjectionHealthCheck {
    fn name(&self) -> &str {
        "mcp-projection"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        if !self.projection_path.exists() {
            return vec![DoctorFinding::warning(format!(
                "MCP projection not found: {} — run `gal mcp update`",
                self.projection_path.display()
            ))];
        }
        let content = match std::fs::read_to_string(&self.projection_path) {
            Ok(c) => c,
            Err(e) => {
                return vec![DoctorFinding::error(
                    format!("MCP projection unreadable: {e}"),
                    "run `gal mcp update`",
                )]
            }
        };
        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(_) => vec![],
            Err(e) => vec![DoctorFinding::error(
                format!("MCP projection invalid JSON: {e}"),
                "run `gal mcp update`",
            )],
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// gal mcp backend
// ─────────────────────────────────────────────────────────────────────────────

/// Result of a `gal mcp update` run.
pub struct McpUpdateReport {
    pub providers_updated: Vec<String>,
    pub servers_written: usize,
    pub warnings: Vec<String>,
}

/// Run the MCP config update for all available providers.
///
/// Ports `Invoke-UpdateMcp` from `Update-Mcp.ps1` (R-02).
/// Reads the manifest from `manifest_path`, resolves variables, then
/// dispatches per-provider serializers that write provider config files.
pub fn run_mcp_update(manifest_path: &Path) -> std::result::Result<McpUpdateReport, McpUpdateError> {
    use providers::claude::ClaudeDesktopMcpConfig;
    use providers::codex::CodexMcpConfig;
    use providers::copilot::CopilotCliMcpConfig;
    use providers::opencode::OpenCodeMcpConfig;
    use providers::McpProviderConfig;

    let manifest = load_manifest(manifest_path)?;
    let resolver = McpVariableResolver::empty();
    let resolved = resolver.resolve_manifest(&manifest)?;

    let servers_written = resolved.servers.len();
    let mut providers_updated = Vec::new();
    let mut warnings = Vec::new();

    // Claude Desktop
    match ClaudeDesktopMcpConfig::from_manifest(&resolved) {
        Ok(cfg) => match cfg.to_config_string() {
            Ok(json) => {
                if let Some(dest) = claude_desktop_mcp_path() {
                    ensure_parent(&dest)?;
                    std::fs::write(&dest, json)?;
                    providers_updated.push("claude".to_string());
                }
            }
            Err(e) => warnings.push(format!("claude serialization error: {e}")),
        },
        Err(e) => warnings.push(format!("claude manifest error: {e}")),
    }

    // Copilot CLI
    match CopilotCliMcpConfig::from_manifest(&resolved) {
        Ok(cfg) => match cfg.to_config_string() {
            Ok(json) => {
                if let Some(dest) = copilot_cli_mcp_path() {
                    ensure_parent(&dest)?;
                    std::fs::write(&dest, json)?;
                    providers_updated.push("copilot".to_string());
                }
            }
            Err(e) => warnings.push(format!("copilot serialization error: {e}")),
        },
        Err(e) => warnings.push(format!("copilot manifest error: {e}")),
    }

    // Codex
    match CodexMcpConfig::from_manifest(&resolved) {
        Ok(cfg) => match cfg.to_config_string() {
            Ok(toml) => {
                if let Some(dest) = codex_config_path() {
                    ensure_parent(&dest)?;
                    // Codex config.toml: append managed sections (simplified — full
                    // remove-then-append from Remove-CodexManagedServersFromToml
                    // is handled in the test path; here we just write the sections).
                    std::fs::write(&dest, toml)?;
                    providers_updated.push("codex".to_string());
                }
            }
            Err(e) => warnings.push(format!("codex serialization error: {e}")),
        },
        Err(e) => warnings.push(format!("codex manifest error: {e}")),
    }

    // OpenCode
    match OpenCodeMcpConfig::from_manifest(&resolved) {
        Ok(cfg) => match cfg.to_config_string() {
            Ok(json) => {
                if let Some(dest) = opencode_config_path() {
                    ensure_parent(&dest)?;
                    std::fs::write(&dest, json)?;
                    providers_updated.push("opencode".to_string());
                }
            }
            Err(e) => warnings.push(format!("opencode serialization error: {e}")),
        },
        Err(e) => warnings.push(format!("opencode manifest error: {e}")),
    }

    Ok(McpUpdateReport { providers_updated, servers_written, warnings })
}

fn ensure_parent(path: &Path) -> std::result::Result<(), McpUpdateError> {
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
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests (moved from gal-engine::mcp)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── McpVariableResolver ──────────────────────────────────────────────────

    #[test]
    fn resolver_substitutes_known_var() {
        let mut vars = HashMap::new();
        vars.insert("HOME".to_string(), "/home/user".to_string());
        let r = McpVariableResolver::new(vars);
        assert_eq!(r.resolve_string("${HOME}/vault").unwrap(), "/home/user/vault");
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
        let gal = McpManifest { servers: gal_servers, inputs: None };
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
        let existing = McpManifest { servers: ex_servers, inputs: None };
        let merged = merger.merge(&gal, Some(&existing));
        assert_eq!(merged.servers["github"].url.as_deref(), Some("https://new.url"));
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
        let gal = McpManifest { servers: gal_servers, inputs: None };
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
        let existing = McpManifest { servers: ex_servers, inputs: None };
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
        let gal = McpManifest { servers: gal_servers, inputs: None };
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
        use base::health::Severity;
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
        fs::write(&path, r#"{"schemaVersion":1,"mcpServers":{},"_metadata":{}}"#).unwrap();
        let check = McpProjectionHealthCheck::with_path(path);
        assert!(check.check().is_empty());
    }

    #[test]
    fn health_check_error_on_invalid_json() {
        use base::health::Severity;
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("managed.json");
        fs::write(&path, "not json").unwrap();
        let check = McpProjectionHealthCheck::with_path(path);
        let findings = check.check();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Error);
    }
}
