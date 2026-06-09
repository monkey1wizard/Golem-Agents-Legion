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

/// Build the MCP variable map used to resolve `${VAR}` placeholders.
///
/// Resolution layers, lowest priority first (later layers overlay earlier):
/// 1. process environment — faithful to `Get-ConfiguredValue`'s env fallback;
/// 2. `~/.gal/config/config.local.env` (`Read-KeyValueEnvFile`);
/// 3. machine config (`~/.gal/config/config.json`) mapped keys.
///
/// Ports the common-case behaviour of `Get-McpVariableMap` from `Update-Mcp.ps1`.
/// The `MCP_FILESYSTEM_PATHS` RepoRoot-parent derivation is intentionally
/// deferred (it depends on install context, not MCP state) — see plan R-02.
fn build_variable_map() -> HashMap<String, String> {
    let env_file = base::paths::gal_home().map(|h| h.join("config").join("config.local.env"));
    let machine_cfg = base::paths::machine_config_path();
    build_variable_map_from(env_file.as_deref(), machine_cfg.as_deref())
}

/// Testable core of [`build_variable_map`] with explicit source paths.
fn build_variable_map_from(
    env_file: Option<&Path>,
    machine_cfg_path: Option<&Path>,
) -> HashMap<String, String> {
    // Layer 1: process environment.
    let mut map: HashMap<String, String> = std::env::vars().collect();

    // Layer 2: config.local.env.
    if let Some(env_file) = env_file {
        for (k, v) in base::env_config::read_key_value_env(env_file) {
            map.insert(k, v);
        }
    }

    // Layer 3: machine config (camelCase config key → UPPER_SNAKE placeholder).
    if let Some(cfg_path) = machine_cfg_path {
        if let Ok(content) = std::fs::read_to_string(cfg_path) {
            if let Ok(serde_json::Value::Object(obj)) =
                serde_json::from_str::<serde_json::Value>(&content)
            {
                const MACHINE_MAP: &[(&str, &str)] = &[
                    ("OBSIDIAN_VAULT", "obsidianVault"),
                    ("OBSIDIAN_VAULT_NAME", "obsidianVaultName"),
                    ("OBSIDIAN_GUIDE_PATH", "obsidianGuidePath"),
                    ("OBSIDIAN_GUIDE_MODE", "obsidianGuideMode"),
                    ("CONTEXT7_API_KEY", "context7ApiKey"),
                    ("TEMP_DIR", "tempDir"),
                    ("LOCAL_SEARCH_PROJECT", "localSearchProject"),
                ];
                for (placeholder, cfg_key) in MACHINE_MAP {
                    if let Some(s) = obj.get(*cfg_key).and_then(|v| v.as_str()) {
                        if !s.trim().is_empty() {
                            map.insert((*placeholder).to_string(), s.to_string());
                        }
                    }
                }
                if let Some(v) = obj.get("mcpFilesystemPaths") {
                    let joined = match v {
                        serde_json::Value::Array(a) => a
                            .iter()
                            .filter_map(|x| x.as_str())
                            .collect::<Vec<_>>()
                            .join(","),
                        serde_json::Value::String(s) => s.clone(),
                        _ => String::new(),
                    };
                    if !joined.trim().is_empty() {
                        map.insert("MCP_FILESYSTEM_PATHS".to_string(), joined);
                    }
                }
            }
        }
    }

    map
}

/// Overlay `~/.gal/config/mcp.local.json` server entries onto `manifest`.
///
/// Local entries add to or override managed entries by name (local wins),
/// matching `Merge-OrderedMap $manifest $localManifest` in `Update-Mcp.ps1`.
fn merge_local_overrides(manifest: McpManifest) -> McpManifest {
    let local_path = base::paths::gal_home().map(|h| h.join("config").join("mcp.local.json"));
    match local_path {
        Some(p) => apply_local_overrides(manifest, &p),
        None => manifest,
    }
}

/// Testable core of [`merge_local_overrides`] with an explicit override path.
fn apply_local_overrides(mut manifest: McpManifest, local_path: &Path) -> McpManifest {
    if let Ok(local) = load_manifest(local_path) {
        for (name, server) in local.servers {
            manifest.servers.insert(name, server);
        }
    }
    manifest
}

/// Non-destructively overlay GAL-managed entries into a JSON provider config.
///
/// Preserves user-owned server entries and any unrelated top-level keys.
/// `servers_key` is the provider's server-map field (`mcpServers` / `mcp`).
/// `generated` is the freshly serialized provider config (`{servers_key: {…}}`).
/// Returns the number of managed entries written. An existing file that is not
/// a JSON object is treated as an error rather than being overwritten.
fn write_json_provider_merged(
    dest: &Path,
    servers_key: &str,
    generated: &serde_json::Value,
) -> std::result::Result<usize, McpUpdateError> {
    let managed = generated
        .get(servers_key)
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let mut root = if dest.exists() {
        let content = std::fs::read_to_string(dest)?;
        if content.trim().is_empty() {
            serde_json::Value::Object(serde_json::Map::new())
        } else {
            match serde_json::from_str::<serde_json::Value>(&content)? {
                v @ serde_json::Value::Object(_) => v,
                _ => {
                    return Err(McpUpdateError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("{} is not a JSON object — refusing to overwrite", dest.display()),
                    )))
                }
            }
        }
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    let obj = root.as_object_mut().expect("root is an object");
    let servers = obj
        .entry(servers_key.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if !servers.is_object() {
        *servers = serde_json::Value::Object(serde_json::Map::new());
    }
    let servers_obj = servers.as_object_mut().expect("servers slot is an object");

    let count = managed.len();
    for (name, entry) in managed {
        servers_obj.insert(name, entry);
    }

    ensure_parent(dest)?;
    std::fs::write(dest, serde_json::to_string_pretty(&root)?)?;
    Ok(count)
}

/// Extract the server name from a Codex TOML table header line.
///
/// Returns `Some(name)` for `[mcp_servers.NAME]` / `[mcp_servers.NAME.env]`
/// (bare or quoted), else `None`. Ports `Get-CodexTableServerName`.
fn codex_table_server_name(line: &str) -> Option<String> {
    let t = line.trim();
    let inner = t.strip_prefix('[')?.strip_suffix(']')?;
    let remainder = inner.strip_prefix("mcp_servers.")?;
    if remainder.is_empty() {
        return None;
    }
    if let Some(rest) = remainder.strip_prefix('"') {
        let mut out = String::new();
        let mut prev = '"';
        for c in rest.chars() {
            if c == '"' && prev != '\\' {
                return Some(out.replace("\\\"", "\"").replace("\\\\", "\\"));
            }
            out.push(c);
            prev = c;
        }
        return None;
    }
    Some(remainder.split('.').next().unwrap_or(remainder).to_string())
}

/// True for a single-bracket TOML table header `[name]` (not `[[array]]`).
fn is_toml_table_header(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('[') && !t.starts_with("[[") && t.ends_with(']') && t.len() > 2
}

/// Remove managed `[mcp_servers.NAME]` sections (and their sub-tables) for the
/// given names, preserving all other content (user MCP entries + non-MCP config).
///
/// Ports `Remove-CodexManagedServersFromToml`.
fn remove_codex_managed_sections(
    raw: &str,
    managed: &std::collections::HashSet<String>,
) -> String {
    if raw.trim().is_empty() {
        return String::new();
    }
    let mut out: Vec<&str> = Vec::new();
    let mut skip = false;
    for line in raw.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some(name) = codex_table_server_name(line) {
            skip = managed.contains(&name);
        } else if is_toml_table_header(line) {
            skip = false;
        }
        if !skip {
            out.push(line);
        }
    }
    out.join("\r\n").trim_end_matches(['\r', '\n']).to_string()
}

/// Write Codex `config.toml` by removing managed sections then appending fresh
/// ones, preserving non-MCP content. `sections` is the codex serializer output
/// (already `\r\n`-joined with a trailing newline).
fn write_codex_merged(
    dest: &Path,
    managed_names: &std::collections::HashSet<String>,
    sections: &str,
) -> std::result::Result<(), McpUpdateError> {
    let existing = if dest.exists() {
        std::fs::read_to_string(dest)?
    } else {
        String::new()
    };
    let remaining = remove_codex_managed_sections(&existing, managed_names);

    let mut new_raw = remaining;
    if !new_raw.trim().is_empty() {
        new_raw = new_raw.trim_end_matches(['\r', '\n']).to_string();
        new_raw.push_str("\r\n\r\n");
    }
    new_raw.push_str(sections);
    if !new_raw.ends_with("\r\n") {
        new_raw.push_str("\r\n");
    }

    ensure_parent(dest)?;
    std::fs::write(dest, new_raw)?;
    Ok(())
}

/// Run the MCP config update for all available providers.
///
/// Ports `Invoke-UpdateMcp` from `Update-Mcp.ps1` (R-02). Reads the manifest,
/// merges `mcp.local.json` overrides, resolves `${VAR}` placeholders from the
/// environment + `config.local.env` + machine config, then writes each
/// provider config **non-destructively** (preserving user-owned entries and,
/// for Codex, non-MCP TOML content).
///
/// Deferred from full PS parity (advanced cleanup, not data-loss/blocking):
/// legacy-alias removal, deprecated-key cleanup, Codex bridge-profile key
/// remapping, and the previous-projection delta guard. See plan R-02.
pub fn run_mcp_update(manifest_path: &Path) -> std::result::Result<McpUpdateReport, McpUpdateError> {
    use providers::claude::ClaudeDesktopMcpConfig;
    use providers::codex::CodexMcpConfig;
    use providers::copilot::CopilotCliMcpConfig;
    use providers::opencode::OpenCodeMcpConfig;
    use providers::McpProviderConfig;

    let manifest = load_manifest(manifest_path)?;
    let manifest = merge_local_overrides(manifest);

    let resolver = McpVariableResolver::new(build_variable_map());
    let resolved = resolver.resolve_manifest(&manifest)?;

    let servers_written = resolved.servers.len();
    let mut providers_updated = Vec::new();
    let mut warnings = Vec::new();

    // Claude Desktop (mcpServers) — non-destructive overlay.
    match ClaudeDesktopMcpConfig::from_manifest(&resolved) {
        Ok(cfg) => match serde_json::to_value(&cfg) {
            Ok(val) => {
                if let Some(dest) = claude_desktop_mcp_path() {
                    match write_json_provider_merged(&dest, "mcpServers", &val) {
                        Ok(_) => providers_updated.push("claude".to_string()),
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
                        Ok(_) => providers_updated.push("copilot".to_string()),
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
                        Ok(_) => providers_updated.push("opencode".to_string()),
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
                        Ok(()) => providers_updated.push("codex".to_string()),
                        Err(e) => warnings.push(format!("codex write error: {e}")),
                    }
                }
            }
            Err(e) => warnings.push(format!("codex serialization error: {e}")),
        },
        Err(e) => warnings.push(format!("codex manifest error: {e}")),
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
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
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

    // ── run_mcp_update orchestration: variable resolution ─────────────────────

    #[test]
    fn build_variable_map_resolves_secret_from_env() {
        // Defect-1 regression: an empty resolver hard-failed on secret-named
        // placeholders. The map must pick up a secret from the environment so
        // resolve_manifest succeeds instead of returning UnresolvedSecret.
        let key = "GAL_TEST_MCP_API_KEY";
        std::env::set_var(key, "sekret");
        let map = build_variable_map_from(None, None);
        let r = McpVariableResolver::new(map);
        assert_eq!(r.resolve_string("${GAL_TEST_MCP_API_KEY}").unwrap(), "sekret");
        std::env::remove_var(key);
    }

    #[test]
    fn build_variable_map_reads_env_file_and_machine_config() {
        let tmp = TempDir::new().unwrap();
        let env_file = tmp.path().join("config.local.env");
        fs::write(&env_file, "MY_TOKEN=from_file\n").unwrap();
        let cfg = tmp.path().join("config.json");
        fs::write(&cfg, r#"{"context7ApiKey":"ctx-123"}"#).unwrap();

        let map = build_variable_map_from(Some(&env_file), Some(&cfg));
        assert_eq!(map.get("MY_TOKEN").map(String::as_str), Some("from_file"));
        assert_eq!(map.get("CONTEXT7_API_KEY").map(String::as_str), Some("ctx-123"));
    }

    // ── apply_local_overrides ─────────────────────────────────────────────────

    #[test]
    fn apply_local_overrides_adds_and_overrides_servers() {
        let mut servers = HashMap::new();
        servers.insert(
            "github".to_string(),
            McpServer {
                server_type: Some("http".to_string()),
                command: None,
                args: None,
                env: None,
                url: Some("https://managed".to_string()),
                headers: None,
            },
        );
        let manifest = McpManifest { servers, inputs: None };

        let tmp = TempDir::new().unwrap();
        let local = tmp.path().join("mcp.local.json");
        // Override `github` url and add a new local-only `mylocal` server.
        fs::write(
            &local,
            r#"{"servers":{"github":{"type":"http","url":"https://local"},"mylocal":{"command":"node"}}}"#,
        )
        .unwrap();

        let merged = apply_local_overrides(manifest, &local);
        assert_eq!(merged.servers.len(), 2);
        assert_eq!(merged.servers["github"].url.as_deref(), Some("https://local"));
        assert_eq!(merged.servers["mylocal"].command.as_deref(), Some("node"));
    }

    #[test]
    fn apply_local_overrides_noop_when_file_absent() {
        let mut servers = HashMap::new();
        servers.insert(
            "a".to_string(),
            McpServer {
                server_type: None,
                command: Some("x".to_string()),
                args: None,
                env: None,
                url: None,
                headers: None,
            },
        );
        let manifest = McpManifest { servers, inputs: None };
        let merged = apply_local_overrides(manifest, Path::new("/nonexistent/mcp.local.json"));
        assert_eq!(merged.servers.len(), 1);
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
        assert_eq!(codex_table_server_name("[mcp_servers.memory]").as_deref(), Some("memory"));
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
        assert!(out.contains("[mcp_servers.user_kept]"), "user MCP server must survive");
        assert!(!out.contains("gal_managed"), "managed section must be removed");
    }

    #[test]
    fn codex_write_merge_appends_and_preserves() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("config.toml");
        fs::write(&dest, "[model]\nname = \"gpt\"\n").unwrap();
        let managed: std::collections::HashSet<String> = ["memory".to_string()].into_iter().collect();
        let sections = "[mcp_servers.memory]\r\ncommand = \"npx\"\r\n";
        write_codex_merged(&dest, &managed, sections).unwrap();
        let written = fs::read_to_string(&dest).unwrap();
        assert!(written.contains("[model]"), "non-MCP content preserved");
        assert!(written.contains("[mcp_servers.memory]"), "managed section appended");
    }
}
