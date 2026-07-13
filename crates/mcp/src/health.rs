//! MCP projection HealthCheck.

use gal_foundation::health::{DoctorFinding, HealthCheck};

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
        gal_foundation::paths::generated_mcp_path().map(|p| Self { projection_path: p })
    }

    pub fn with_path(path: std::path::PathBuf) -> Self {
        Self {
            projection_path: path,
        }
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
