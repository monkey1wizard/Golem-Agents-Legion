//! MCP (Model Context Protocol) health crate.
//!
//! Owns the `HealthCheck` for gal's own canonical MCP manifest.
//!
//! Product boundary: gal declares and consumes its own MCP servers. It manages
//! no host's live MCP config, so nothing here writes into another tool's
//! configuration file, and nothing here inspects one.
//!
//! Dependency law: `mcp` → `base`. No GAL dep cycles.

mod health;

pub use health::*;

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use gal_foundation::health::HealthCheck;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn health_check_warns_when_manifest_missing() {
        use gal_foundation::health::Severity;
        let tmp = TempDir::new().unwrap();
        let check = CanonicalMcpHealthCheck::with_path(tmp.path().join("nonexistent.json"));
        let findings = check.check();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
    }

    #[test]
    fn health_check_ok_when_valid_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".mcp.json");
        fs::write(&path, r#"{"mcpServers":{}}"#).unwrap();
        let check = CanonicalMcpHealthCheck::with_path(path);
        assert!(check.check().is_empty());
    }

    #[test]
    fn health_check_error_on_invalid_json() {
        use gal_foundation::health::Severity;
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".mcp.json");
        fs::write(&path, "not json").unwrap();
        let check = CanonicalMcpHealthCheck::with_path(path);
        let findings = check.check();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Error);
    }
}
