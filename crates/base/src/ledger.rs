//! GAL install ledger — records install/update/uninstall operations.
//!
//! Written to `~/.gal/ledger.json` after each operation.
//! Used by `gal doctor` to verify freshness and surfaces.
//!
//! Corresponds to T-011 of fix-gal-bootstrap-install-convergence.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// A single install/update/uninstall operation record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LedgerEntry {
    /// "install" | "update" | "uninstall"
    pub operation: String,
    /// Path to canonical plugin root at time of operation.
    pub canonical_root: PathBuf,
    /// ISO 8601 UTC timestamp.
    pub timestamp: String,
    /// Providers successfully projected: "claude", "copilot", "agy".
    pub providers: Vec<String>,
    /// Mode in use: "normal" | "dev".
    pub mode: String,
    /// Non-fatal warnings observed during the operation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// The on-disk ledger format.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Ledger {
    /// Most recent entry; `None` if never installed.
    pub last: Option<LedgerEntry>,
    /// Full history, oldest first.
    pub history: Vec<LedgerEntry>,
}

impl Ledger {
    /// Load ledger from disk. Returns empty ledger on missing file or parse error.
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Record a new entry and update `last`.
    pub fn record(&mut self, entry: LedgerEntry) {
        self.last = Some(entry.clone());
        self.history.push(entry);
    }

    /// Save ledger to disk. Creates parent directories if needed.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        fs::write(path, content)
    }
}

/// Canonical path for the GAL ledger file (`~/.gal/ledger.json`).
pub fn ledger_path() -> Option<PathBuf> {
    crate::paths::user_home().map(|h| h.join(".gal").join("ledger.json"))
}

/// Current UTC timestamp as ISO 8601 string.
pub fn now_timestamp() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn ledger_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("ledger.json");

        let mut ledger = Ledger::default();
        assert!(ledger.last.is_none());
        assert!(ledger.history.is_empty());

        let entry = LedgerEntry {
            operation: "install".to_string(),
            canonical_root: PathBuf::from("/home/user/.gal/plugins/gal"),
            timestamp: "2026-06-03T10:00:00Z".to_string(),
            providers: vec!["claude".to_string(), "copilot".to_string()],
            mode: "normal".to_string(),
            warnings: vec![],
        };

        ledger.record(entry.clone());
        ledger.save(&path).unwrap();

        let loaded = Ledger::load(&path);
        assert_eq!(loaded.last.as_ref().unwrap().operation, "install");
        assert_eq!(loaded.history.len(), 1);
        assert_eq!(loaded.last.unwrap().providers, entry.providers);
    }

    #[test]
    fn missing_ledger_returns_default() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.json");
        let ledger = Ledger::load(&path);
        assert!(ledger.last.is_none());
        assert!(ledger.history.is_empty());
    }

    #[test]
    fn ledger_records_multiple_entries() {
        let mut ledger = Ledger::default();
        for op in &["install", "update", "update"] {
            ledger.record(LedgerEntry {
                operation: op.to_string(),
                canonical_root: PathBuf::from("/home/.gal/plugins/gal"),
                timestamp: "2026-06-03T10:00:00Z".to_string(),
                providers: vec!["claude".to_string()],
                mode: "normal".to_string(),
                warnings: vec![],
            });
        }
        assert_eq!(ledger.history.len(), 3);
        assert_eq!(ledger.last.unwrap().operation, "update");
    }

    #[test]
    fn now_timestamp_is_iso8601() {
        let ts = now_timestamp();
        assert!(ts.ends_with('Z'), "timestamp should end with Z: {ts}");
        assert!(ts.contains('T'), "timestamp should contain T: {ts}");
        assert_eq!(ts.len(), 20, "expected YYYY-MM-DDTHH:MM:SSZ length 20");
    }

    #[test]
    fn invalid_json_returns_default() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.json");
        fs::write(&path, "{ not valid json }").unwrap();
        let ledger = Ledger::load(&path);
        assert!(ledger.last.is_none());
    }
}
