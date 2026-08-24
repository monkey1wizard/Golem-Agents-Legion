//! Canonical MCP manifest HealthCheck.

use gal_foundation::health::{DoctorFinding, HealthCheck};

// ─────────────────────────────────────────────────────────────────────────────
// HealthCheck implementation
// ─────────────────────────────────────────────────────────────────────────────

/// Checks that the canonical `.mcp.json` gal renders into its plugin root
/// exists and is valid JSON.
///
/// This is gal's own manifest — the servers gal declares and consumes. gal
/// manages no host's live MCP config, so nothing else is inspected here.
pub struct CanonicalMcpHealthCheck {
    manifest_path: std::path::PathBuf,
}

impl CanonicalMcpHealthCheck {
    /// Construct using the standard `~/.gal/plugins/gal/.mcp.json` path.
    pub fn from_standard_path() -> Option<Self> {
        gal_foundation::paths::canonical_mcp_path().map(|p| Self { manifest_path: p })
    }

    pub fn with_path(path: std::path::PathBuf) -> Self {
        Self {
            manifest_path: path,
        }
    }
}

impl HealthCheck for CanonicalMcpHealthCheck {
    fn name(&self) -> &str {
        "canonical-mcp"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        if !self.manifest_path.exists() {
            return vec![DoctorFinding::warning(format!(
                "canonical MCP manifest not found: {} — run `gal refresh`",
                self.manifest_path.display()
            ))];
        }
        let content = match std::fs::read_to_string(&self.manifest_path) {
            Ok(c) => c,
            Err(e) => {
                return vec![DoctorFinding::error(
                    format!("canonical MCP manifest unreadable: {e}"),
                    "run `gal refresh`",
                )]
            }
        };
        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(_) => vec![],
            Err(e) => vec![DoctorFinding::error(
                format!("canonical MCP manifest invalid JSON: {e}"),
                "run `gal refresh`",
            )],
        }
    }
}
