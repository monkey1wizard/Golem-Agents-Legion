//! config (T-005 support): parse `xmachine.config.json` into work-node targets.
//!
//! The config is machine-local (user-owned, gitignored). GAL only reads it to
//! resolve a node alias to its SSH target + remote repo paths; it never writes or
//! provisions it. Shape (see `plugins/gal-core/templates/xmachine.config.example.json`):
//!
//! ```json
//! { "nodes": { "mac-mini": { "target": "user@host", "repoPath": "...",
//!     "runtimeRepoPath": "...", "repoMappings": { ... } } } }
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::ssh::{ExecutionMode, RemoteTarget};

/// One work node from `xmachine.config.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfig {
    /// `user@host` SSH destination.
    pub target: String,
    /// Remote path to the project repo clone (enables `repo` worktree mode).
    #[serde(default)]
    pub repo_path: Option<String>,
    /// Remote path to the GAL runtime checkout (where remote `gal` / scripts live).
    #[serde(default)]
    pub runtime_repo_path: Option<String>,
    /// Optional secondary repo mappings (left opaque; not needed for dispatch).
    #[serde(default)]
    pub repo_mappings: BTreeMap<String, serde_json::Value>,
}

/// Parsed `xmachine.config.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct XmachineConfig {
    #[serde(default)]
    pub nodes: BTreeMap<String, NodeConfig>,
}

/// Errors resolving the xmachine config / a node.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("xmachine config not found at {0}")]
    NotFound(String),
    #[error("xmachine config is not valid JSON: {0}")]
    Parse(String),
    #[error("work node '{0}' is not defined in xmachine.config.json")]
    UnknownNode(String),
    #[error("work node '{0}' has a blank target")]
    BlankTarget(String),
}

impl XmachineConfig {
    /// Parse a config body.
    pub fn parse(body: &str) -> Result<Self, ConfigError> {
        serde_json::from_str(body).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Read + parse the config from a path.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.display().to_string()));
        }
        let body =
            std::fs::read_to_string(path).map_err(|e| ConfigError::Parse(e.to_string()))?;
        Self::parse(&body)
    }

    /// Resolve a node alias to its config, erroring with a clear message if absent
    /// or if its target is blank.
    pub fn node(&self, alias: &str) -> Result<&NodeConfig, ConfigError> {
        let node = self
            .nodes
            .get(alias)
            .ok_or_else(|| ConfigError::UnknownNode(alias.to_string()))?;
        if node.target.trim().is_empty() {
            return Err(ConfigError::BlankTarget(alias.to_string()));
        }
        Ok(node)
    }
}

impl NodeConfig {
    /// The SSH target split into `RemoteTarget` (`user@host`). Falls back to an
    /// empty user when no `@` is present (host-only target).
    pub fn remote_target(&self) -> RemoteTarget {
        match self.target.split_once('@') {
            Some((user, host)) => RemoteTarget::new(user, host),
            None => RemoteTarget::new("", &self.target),
        }
    }

    /// Execution mode for this node: `repo` worktree when a `repoPath` is set,
    /// otherwise a throwaway `execute` workspace.
    pub fn execution_mode(&self) -> ExecutionMode {
        match &self.repo_path {
            Some(p) if !p.trim().is_empty() => ExecutionMode::Repo { repo_path: p.clone() },
            _ => ExecutionMode::Execute,
        }
    }
}

/// Default config locations, in resolution order: repo-local `xmachine.config.json`,
/// then `~/.gal/xmachine.config.json`. Returns the first that exists, or the
/// repo-local path as the canonical "expected" location when none exist.
pub fn resolve_config_path(repo_root: &Path, home: Option<&Path>) -> PathBuf {
    let repo_local = repo_root.join("xmachine.config.json");
    if repo_local.exists() {
        return repo_local;
    }
    if let Some(h) = home {
        let user_local = h.join(".gal").join("xmachine.config.json");
        if user_local.exists() {
            return user_local;
        }
    }
    repo_local
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "nodes": {
        "mac-mini": {
          "target": "alice@alice-mac-mini.local",
          "repoPath": "/Users/alice/Golem-Agents-Legion",
          "runtimeRepoPath": "/Users/alice/Golem-Agents-Legion"
        },
        "win11-pc": {
          "target": "alice@win11-pc",
          "repoPath": "%USERPROFILE%\\Golem-Agents-Legion"
        },
        "scratch": { "target": "bob@scratch" }
      }
    }"#;

    #[test]
    fn parses_nodes() {
        let cfg = XmachineConfig::parse(SAMPLE).unwrap();
        assert_eq!(cfg.nodes.len(), 3);
        let mac = cfg.node("mac-mini").unwrap();
        assert_eq!(mac.target, "alice@alice-mac-mini.local");
        assert_eq!(mac.repo_path.as_deref(), Some("/Users/alice/Golem-Agents-Legion"));
    }

    #[test]
    fn unknown_node_errors() {
        let cfg = XmachineConfig::parse(SAMPLE).unwrap();
        assert_eq!(
            cfg.node("ghost").unwrap_err(),
            ConfigError::UnknownNode("ghost".to_string())
        );
    }

    #[test]
    fn remote_target_splits_user_host() {
        let cfg = XmachineConfig::parse(SAMPLE).unwrap();
        let t = cfg.node("mac-mini").unwrap().remote_target();
        assert_eq!(t.user, "alice");
        assert_eq!(t.host, "alice-mac-mini.local");
        assert_eq!(t.ssh_target(), "alice@alice-mac-mini.local");
    }

    #[test]
    fn repo_path_selects_repo_mode_else_execute() {
        let cfg = XmachineConfig::parse(SAMPLE).unwrap();
        assert_eq!(
            cfg.node("mac-mini").unwrap().execution_mode(),
            ExecutionMode::Repo { repo_path: "/Users/alice/Golem-Agents-Legion".into() }
        );
        // 'scratch' has no repoPath → execute workspace.
        assert_eq!(cfg.node("scratch").unwrap().execution_mode(), ExecutionMode::Execute);
    }

    #[test]
    fn blank_target_errors() {
        let cfg = XmachineConfig::parse(r#"{"nodes":{"n":{"target":"  "}}}"#).unwrap();
        assert_eq!(cfg.node("n").unwrap_err(), ConfigError::BlankTarget("n".to_string()));
    }

    #[test]
    fn invalid_json_errors() {
        assert!(matches!(XmachineConfig::parse("{ not json"), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn empty_config_has_no_nodes() {
        let cfg = XmachineConfig::parse("{}").unwrap();
        assert!(cfg.nodes.is_empty());
    }
}
