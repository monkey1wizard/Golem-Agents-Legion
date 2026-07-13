//! MCP variable resolver (${VAR} substitution) + the variable-map builders.

use gal_foundation::mcp::{McpError, McpManifest, McpServer, Result};
use std::collections::HashMap;
use std::path::Path;

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
        Self {
            variables: HashMap::new(),
        }
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
            s.args = Some(
                args.iter()
                    .map(|a| self.resolve_string(a))
                    .collect::<Result<_>>()?,
            );
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
        Ok(McpManifest {
            servers,
            inputs: manifest.inputs.clone(),
        })
    }
}

fn is_secret_name(name: &str) -> bool {
    let u = name.to_uppercase();
    u.contains("KEY") || u.contains("SECRET") || u.contains("TOKEN") || u.contains("PASSWORD")
}
/// Build the MCP variable map used to resolve `${VAR}` placeholders.
///
/// Resolution layers, lowest priority first (later layers overlay earlier):
/// 1. process environment;
/// 2. `~/.gal/config/config.json` — path values via `MACHINE_PATH_REGISTRY` +
///    `config.json#secrets` verbatim (the sole machine-config source).
pub(crate) fn build_variable_map() -> HashMap<String, String> {
    let machine_cfg = gal_foundation::paths::machine_config_path();
    build_variable_map_from(machine_cfg.as_deref())
}

/// Testable core of [`build_variable_map`] with an explicit config.json path.
///
/// `machine_cfg_path` — JSON `config.json` (the sole machine-config source).
pub(crate) fn build_variable_map_from(machine_cfg_path: Option<&Path>) -> HashMap<String, String> {
    // Layer 1: process environment.
    let mut map: HashMap<String, String> = std::env::vars().collect();

    // Layer 2: config.json — wins over the environment for overlapping keys.
    // Empty string values are treated as "not set". If the file is unreadable or
    // malformed, a warning is emitted and the environment values stand.
    if let Some(cfg_path) = machine_cfg_path {
        match std::fs::read_to_string(cfg_path) {
            Err(e) => eprintln!("gal-mcp: WARN — cannot read {}: {e}", cfg_path.display()),
            Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                Err(e) => eprintln!(
                    "gal-mcp: WARN — malformed config.json at {}: {e}",
                    cfg_path.display()
                ),
                Ok(root) => {
                    // Path values via MACHINE_PATH_REGISTRY (nested dot-path).
                    for (config_path, snake_key) in gal_foundation::config::MACHINE_PATH_REGISTRY {
                        if let Some(value) =
                            gal_foundation::config::machine_path_value(&root, config_path)
                        {
                            if !value.trim().is_empty() {
                                map.insert((*snake_key).to_string(), value);
                            }
                        }
                    }
                    // Credential placeholders from config.json#secrets verbatim.
                    if let Some(secrets) = root.get("secrets").and_then(|v| v.as_object()) {
                        for (k, v) in secrets {
                            if let Some(s) = v.as_str() {
                                if !s.trim().is_empty() {
                                    map.insert(k.clone(), s.to_string());
                                }
                            }
                        }
                    }
                }
            },
        }
    }

    map
}
