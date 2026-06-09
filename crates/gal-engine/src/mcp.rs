//! MCP (Model Context Protocol) configuration management
//!
//! This module implements provider-agnostic MCP manifest processing:
//! - Variable/placeholder resolution
//! - Safe merge of GAL-managed and user-owned entries
//! - Secret validation guards
//! - Integrity checks before writing config files
//!
//! Corresponds to T-008 of the bootstrap convergence plan.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// MCP shared data-model types (McpError, Result, McpServer, McpManifest) now live
// in `base::mcp` to break the providers→mcp cycle (R-00/T-003, architect BUG-01).
// Re-exported so `crate::mcp::{McpManifest, McpServer, Result, McpError}` and
// `gal_engine::mcp::*` keep resolving for the generation logic and the providers.
pub use base::mcp::{McpError, McpManifest, McpServer, Result};

/// Variable resolver for MCP configuration
///
/// Resolves ${VAR_NAME} placeholders using provided variable map
#[derive(Debug)]
pub struct McpVariableResolver {
    variables: HashMap<String, String>,
}

impl McpVariableResolver {
    /// Create a new resolver with the given variables
    pub fn new(variables: HashMap<String, String>) -> Self {
        Self { variables }
    }

    /// Create a resolver from GAL machine config + local env
    ///
    /// Loads variables from:
    /// - config.local.env (local environment)
    /// - ~/.gal/config/config.json (machine config)
    pub fn from_config(_config_root: &Path) -> Result<Self> {
        // TODO: Load from config.local.env and machine config
        // For now, return empty resolver
        Ok(Self {
            variables: HashMap::new(),
        })
    }

    /// Resolve a string containing ${VAR_NAME} placeholders
    ///
    /// Returns the resolved string, or original if no placeholders found.
    /// Unresolved secrets (containing "KEY" or "SECRET" in name) cause error.
    pub fn resolve_string(&self, value: &str) -> Result<String> {
        let mut result = value.to_string();
        let placeholder_regex = regex::Regex::new(r"\$\{([A-Z0-9_]+)\}").unwrap();

        for cap in placeholder_regex.captures_iter(value) {
            let var_name = &cap[1];
            let placeholder = &cap[0];

            if let Some(resolved) = self.variables.get(var_name) {
                result = result.replace(placeholder, resolved);
            } else {
                // Check if this is a secret placeholder
                if self.is_secret_variable(var_name) {
                    return Err(McpError::UnresolvedSecret(var_name.to_string()));
                }
                // Non-secret unresolved placeholders are left as-is
            }
        }

        Ok(result)
    }

    /// Resolve a server configuration
    ///
    /// Applies variable resolution to all string fields in the server config
    pub fn resolve_server(&self, server: &McpServer) -> Result<McpServer> {
        let mut resolved = server.clone();

        // Resolve command
        if let Some(ref cmd) = server.command {
            resolved.command = Some(self.resolve_string(cmd)?);
        }

        // Resolve args
        if let Some(ref args) = server.args {
            let mut resolved_args = Vec::new();
            for arg in args {
                resolved_args.push(self.resolve_string(arg)?);
            }
            resolved.args = Some(resolved_args);
        }

        // Resolve env
        if let Some(ref env) = server.env {
            let mut resolved_env = HashMap::new();
            for (key, value) in env {
                resolved_env.insert(key.clone(), self.resolve_string(value)?);
            }
            resolved.env = Some(resolved_env);
        }

        // Resolve url
        if let Some(ref url) = server.url {
            resolved.url = Some(self.resolve_string(url)?);
        }

        // Resolve headers
        if let Some(ref headers) = server.headers {
            let mut resolved_headers = HashMap::new();
            for (key, value) in headers {
                resolved_headers.insert(key.clone(), self.resolve_string(value)?);
            }
            resolved.headers = Some(resolved_headers);
        }

        Ok(resolved)
    }

    /// Resolve entire manifest
    pub fn resolve_manifest(&self, manifest: &McpManifest) -> Result<McpManifest> {
        let mut resolved = McpManifest {
            servers: HashMap::new(),
            inputs: manifest.inputs.clone(),
        };

        for (name, server) in &manifest.servers {
            resolved.servers.insert(name.clone(), self.resolve_server(server)?);
        }

        Ok(resolved)
    }

    /// Check if a variable name indicates a secret
    fn is_secret_variable(&self, name: &str) -> bool {
        let upper = name.to_uppercase();
        upper.contains("KEY") || upper.contains("SECRET") || upper.contains("TOKEN") || upper.contains("PASSWORD")
    }
}

/// MCP configuration merger
///
/// Implements safe merge of GAL-managed and user-owned MCP entries.
/// Only overwrites GAL-managed servers; preserves user entries.
#[derive(Debug)]
pub struct McpMerger {
    gal_managed_servers: Vec<String>,
}

impl McpMerger {
    /// Create a new merger
    ///
    /// `gal_managed_servers` is the list of server names that GAL owns
    pub fn new(gal_managed_servers: Vec<String>) -> Self {
        Self {
            gal_managed_servers,
        }
    }

    /// Merge GAL manifest with existing user config
    ///
    /// - GAL-managed entries are replaced with new values
    /// - User-owned entries are preserved
    /// - Result is idempotent (running twice produces same output)
    pub fn merge(
        &self,
        gal_manifest: &McpManifest,
        existing_config: Option<&McpManifest>,
    ) -> McpManifest {
        let mut merged = McpManifest {
            servers: HashMap::new(),
            inputs: gal_manifest.inputs.clone(),
        };

        // Add all GAL-managed servers from the new manifest
        for (name, server) in &gal_manifest.servers {
            merged.servers.insert(name.clone(), server.clone());
        }

        // Preserve user-owned servers from existing config
        if let Some(existing) = existing_config {
            for (name, server) in &existing.servers {
                if !self.gal_managed_servers.contains(name) {
                    merged.servers.insert(name.clone(), server.clone());
                }
            }
        }

        merged
    }
}

/// MCP projection metadata
///
/// Tracks ownership and source information for runtime config
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

/// MCP projection for runtime config
///
/// This is written to provider-specific MCP config locations
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

/// Check if plugin root is complete
///
/// Verifies that the canonical plugin root contains required directories:
/// - commands/
/// - agents/
/// - skills/
pub fn check_plugin_root_complete(plugin_root: &Path) -> Result<()> {
    let required = ["commands", "agents", "skills"];

    for dir_name in &required {
        let dir_path = plugin_root.join(dir_name);
        if !dir_path.exists() || !dir_path.is_dir() {
            return Err(McpError::PluginRootIncomplete(dir_name.to_string()));
        }
    }

    Ok(())
}

/// Load MCP manifest from file
pub fn load_manifest(path: &Path) -> Result<McpManifest> {
    let content = std::fs::read_to_string(path)?;
    let manifest = serde_json::from_str(&content)?;
    Ok(manifest)
}

/// Save MCP projection to file
///
/// Only writes if plugin root is complete (prevents half-bare config)
pub fn save_projection(path: &Path, projection: &McpProjection, plugin_root: &Path) -> Result<()> {
    // Verify plugin root completeness before writing
    check_plugin_root_complete(plugin_root)?;

    let json = serde_json::to_string_pretty(projection)?;
    std::fs::write(path, json)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_variable_resolver_simple() {
        let mut vars = HashMap::new();
        vars.insert("HOME".to_string(), "/home/user".to_string());
        let resolver = McpVariableResolver::new(vars);

        let result = resolver.resolve_string("${HOME}/vault").unwrap();
        assert_eq!(result, "/home/user/vault");
    }

    #[test]
    fn test_variable_resolver_multiple() {
        let mut vars = HashMap::new();
        vars.insert("HOME".to_string(), "/home/user".to_string());
        vars.insert("PROJECT".to_string(), "myproject".to_string());
        let resolver = McpVariableResolver::new(vars);

        let result = resolver
            .resolve_string("${HOME}/projects/${PROJECT}")
            .unwrap();
        assert_eq!(result, "/home/user/projects/myproject");
    }

    #[test]
    fn test_variable_resolver_unresolved_secret_fails() {
        let resolver = McpVariableResolver::new(HashMap::new());

        let result = resolver.resolve_string("${API_KEY}");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::UnresolvedSecret(_)));
    }

    #[test]
    fn test_variable_resolver_unresolved_non_secret_passes() {
        let resolver = McpVariableResolver::new(HashMap::new());

        let result = resolver.resolve_string("${SOME_VAR}").unwrap();
        assert_eq!(result, "${SOME_VAR}"); // Left as-is
    }

    #[test]
    fn test_merger_replaces_gal_managed() {
        let gal_servers = vec!["github".to_string()];
        let merger = McpMerger::new(gal_servers);

        let mut gal_manifest_servers = HashMap::new();
        gal_manifest_servers.insert(
            "github".to_string(),
            McpServer {
                server_type: Some("http".to_string()),
                url: Some("https://api.githubcopilot.com/mcp/".to_string()),
                command: None,
                args: None,
                env: None,
                headers: None,
            },
        );
        let gal_manifest = McpManifest {
            servers: gal_manifest_servers,
            inputs: None,
        };

        let mut existing_servers = HashMap::new();
        existing_servers.insert(
            "github".to_string(),
            McpServer {
                server_type: Some("http".to_string()),
                url: Some("https://old.url.com".to_string()),
                command: None,
                args: None,
                env: None,
                headers: None,
            },
        );
        let existing = McpManifest {
            servers: existing_servers,
            inputs: None,
        };

        let merged = merger.merge(&gal_manifest, Some(&existing));

        assert_eq!(merged.servers.len(), 1);
        assert_eq!(
            merged.servers.get("github").unwrap().url.as_ref().unwrap(),
            "https://api.githubcopilot.com/mcp/"
        );
    }

    #[test]
    fn test_merger_preserves_user_owned() {
        let gal_servers = vec!["github".to_string()];
        let merger = McpMerger::new(gal_servers);

        let mut gal_manifest_servers = HashMap::new();
        gal_manifest_servers.insert(
            "github".to_string(),
            McpServer {
                server_type: Some("http".to_string()),
                url: Some("https://api.githubcopilot.com/mcp/".to_string()),
                command: None,
                args: None,
                env: None,
                headers: None,
            },
        );
        let gal_manifest = McpManifest {
            servers: gal_manifest_servers,
            inputs: None,
        };

        let mut existing_servers = HashMap::new();
        existing_servers.insert(
            "github".to_string(),
            McpServer {
                server_type: Some("http".to_string()),
                url: Some("https://old.url.com".to_string()),
                command: None,
                args: None,
                env: None,
                headers: None,
            },
        );
        existing_servers.insert(
            "my-custom-server".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("my-tool".to_string()),
                args: Some(vec!["--flag".to_string()]),
                env: None,
                url: None,
                headers: None,
            },
        );
        let existing = McpManifest {
            servers: existing_servers,
            inputs: None,
        };

        let merged = merger.merge(&gal_manifest, Some(&existing));

        assert_eq!(merged.servers.len(), 2);
        assert!(merged.servers.contains_key("github"));
        assert!(merged.servers.contains_key("my-custom-server"));

        // GAL-managed updated
        assert_eq!(
            merged.servers.get("github").unwrap().url.as_ref().unwrap(),
            "https://api.githubcopilot.com/mcp/"
        );

        // User-owned preserved
        assert_eq!(
            merged
                .servers
                .get("my-custom-server")
                .unwrap()
                .command
                .as_ref()
                .unwrap(),
            "my-tool"
        );
    }

    #[test]
    fn test_merger_idempotent() {
        let gal_servers = vec!["github".to_string()];
        let merger = McpMerger::new(gal_servers);

        let mut gal_manifest_servers = HashMap::new();
        gal_manifest_servers.insert(
            "github".to_string(),
            McpServer {
                server_type: Some("http".to_string()),
                url: Some("https://api.githubcopilot.com/mcp/".to_string()),
                command: None,
                args: None,
                env: None,
                headers: None,
            },
        );
        let gal_manifest = McpManifest {
            servers: gal_manifest_servers,
            inputs: None,
        };

        // First merge
        let merged1 = merger.merge(&gal_manifest, None);

        // Second merge with result of first
        let merged2 = merger.merge(&gal_manifest, Some(&merged1));

        // Should be identical
        assert_eq!(merged1.servers.len(), merged2.servers.len());
        assert_eq!(
            merged1
                .servers
                .get("github")
                .unwrap()
                .url
                .as_ref()
                .unwrap(),
            merged2
                .servers
                .get("github")
                .unwrap()
                .url
                .as_ref()
                .unwrap()
        );
    }

    // TP-011: Integration tests for safe-merge and incomplete plugin root guard

    #[test]
    fn test_plugin_root_incomplete_missing_commands() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_root = temp_dir.path();

        // Create only agents/ and skills/, missing commands/
        fs::create_dir(plugin_root.join("agents")).unwrap();
        fs::create_dir(plugin_root.join("skills")).unwrap();

        let result = check_plugin_root_complete(plugin_root);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            McpError::PluginRootIncomplete(_)
        ));
    }

    #[test]
    fn test_plugin_root_incomplete_missing_agents() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_root = temp_dir.path();

        // Create only commands/ and skills/, missing agents/
        fs::create_dir(plugin_root.join("commands")).unwrap();
        fs::create_dir(plugin_root.join("skills")).unwrap();

        let result = check_plugin_root_complete(plugin_root);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_root_complete() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_root = temp_dir.path();

        // Create all required directories
        fs::create_dir(plugin_root.join("commands")).unwrap();
        fs::create_dir(plugin_root.join("agents")).unwrap();
        fs::create_dir(plugin_root.join("skills")).unwrap();

        let result = check_plugin_root_complete(plugin_root);
        assert!(result.is_ok());
    }

    #[test]
    fn test_save_projection_fails_on_incomplete_plugin_root() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_root = temp_dir.path();
        let output_path = temp_dir.path().join("mcp_config.json");

        // Create incomplete plugin root (missing commands/)
        fs::create_dir(plugin_root.join("agents")).unwrap();
        fs::create_dir(plugin_root.join("skills")).unwrap();

        let projection = McpProjection {
            schema_version: 1,
            mcp_servers: HashMap::new(),
            inputs: None,
            metadata: McpProjectionMetadata {
                ownership: "gal-managed".to_string(),
                generated_at: "2024-01-01T00:00:00Z".to_string(),
                generated_by: "test".to_string(),
                secret_bearing: false,
                source_files: HashMap::new(),
                preservation: "test".to_string(),
            },
        };

        let result = save_projection(&output_path, &projection, plugin_root);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            McpError::PluginRootIncomplete(_)
        ));

        // Verify no file was written
        assert!(!output_path.exists());
    }

    #[test]
    fn test_save_projection_succeeds_on_complete_plugin_root() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_root = temp_dir.path();
        let output_path = temp_dir.path().join("mcp_config.json");

        // Create complete plugin root
        fs::create_dir(plugin_root.join("commands")).unwrap();
        fs::create_dir(plugin_root.join("agents")).unwrap();
        fs::create_dir(plugin_root.join("skills")).unwrap();

        let mut servers = HashMap::new();
        servers.insert(
            "github".to_string(),
            McpServer {
                server_type: Some("http".to_string()),
                url: Some("https://api.githubcopilot.com/mcp/".to_string()),
                command: None,
                args: None,
                env: None,
                headers: None,
            },
        );

        let projection = McpProjection {
            schema_version: 1,
            mcp_servers: servers,
            inputs: None,
            metadata: McpProjectionMetadata {
                ownership: "gal-managed".to_string(),
                generated_at: "2024-01-01T00:00:00Z".to_string(),
                generated_by: "test".to_string(),
                secret_bearing: false,
                source_files: HashMap::new(),
                preservation: "test".to_string(),
            },
        };

        let result = save_projection(&output_path, &projection, plugin_root);
        assert!(result.is_ok());

        // Verify file was written and contains valid JSON
        assert!(output_path.exists());
        let content = fs::read_to_string(&output_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["schemaVersion"], 1);
        assert!(parsed["mcpServers"]["github"].is_object());
    }
}
