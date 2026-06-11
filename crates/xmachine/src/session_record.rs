//! session_record (T-009, R-06): remote-run session records.
//!
//! Remote execution must be traceable: host, transport, SSH/zellij session id,
//! times, and result path. Per the architect decision (ARCH-X2) and the GAL
//! file-memory contract, these records **extend dispatch's `.dev/executor-logs/`**
//! — they do not introduce a second store. The directory is exactly the one
//! `dispatch::spawn_executor` writes its `.log` files to; the session record sits
//! beside them as a `.session.json` sidecar.

use std::io;
use std::path::{Path, PathBuf};

use dispatch::dispatch::SpawnConfig;
use serde::{Deserialize, Serialize};

/// One remote-run session record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub task_id: String,
    pub phase: String,
    /// Transport that ran the work: `"ssh"`, `"zellij"`, `"local"`.
    pub transport: String,
    pub work_node: String,
    /// `user@host` SSH destination.
    pub host: String,
    /// SSH/zellij native session id, when one is captured (resume handle).
    #[serde(default)]
    pub session_id: Option<String>,
    pub dispatched_at_utc: String,
    #[serde(default)]
    pub finished_at_utc: Option<String>,
    /// Where the collected results landed locally.
    #[serde(default)]
    pub result_path: Option<String>,
    pub terminal_state: String,
}

/// The executor-logs directory for a workdir — the *same* dir
/// `dispatch::spawn_executor` uses (`<workdir>/.dev/executor-logs/`). Reused so
/// session records do not create a second store.
pub fn executor_logs_dir(workdir: &Path) -> PathBuf {
    SpawnConfig::default_log_dir(workdir)
}

/// Path of a session record sidecar within the executor-logs dir. Mirrors the
/// dispatch log filename shape (`<stamp>-<task>-<phase>-…`), with `.session.json`
/// and the transport in place of the executor.
pub fn session_record_path(
    log_dir: &Path,
    stamp: &str,
    task_id: &str,
    phase: &str,
    transport: &str,
) -> PathBuf {
    log_dir.join(format!("{stamp}-{task_id}-{phase}-{transport}.session.json"))
}

/// Serialize a record to pretty JSON.
pub fn to_json(record: &SessionRecord) -> String {
    serde_json::to_string_pretty(record).expect("SessionRecord is always serializable")
}

/// Write a session record into the executor-logs dir (creating it if needed) and
/// return the path written. No other store is touched.
pub fn write_session_record(
    log_dir: &Path,
    stamp: &str,
    record: &SessionRecord,
) -> io::Result<PathBuf> {
    std::fs::create_dir_all(log_dir)?;
    let path = session_record_path(log_dir, stamp, &record.task_id, &record.phase, &record.transport);
    std::fs::write(&path, to_json(record))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample() -> SessionRecord {
        SessionRecord {
            task_id: "T-013".into(),
            phase: "implement".into(),
            transport: "ssh".into(),
            work_node: "mac-mini".into(),
            host: "alice@mac-mini".into(),
            session_id: Some("zellij:pipeline-20260611-abc123".into()),
            dispatched_at_utc: "2026-06-11T10:00:00Z".into(),
            finished_at_utc: Some("2026-06-11T10:05:00Z".into()),
            result_path: Some(".tmp/gal-results/20260611-abc123".into()),
            terminal_state: "completed".into(),
        }
    }

    #[test]
    fn record_dir_is_the_dispatch_executor_logs_dir() {
        let workdir = Path::new("/work/repo");
        // Must equal dispatch's own log dir — no second store.
        assert_eq!(
            executor_logs_dir(workdir),
            SpawnConfig::default_log_dir(workdir)
        );
        assert!(executor_logs_dir(workdir).ends_with("executor-logs"));
    }

    #[test]
    fn record_path_sits_beside_dispatch_logs() {
        let dir = Path::new("/work/repo/.dev/executor-logs");
        let p = session_record_path(dir, "20260611-100000", "T-013", "implement", "ssh");
        assert_eq!(
            p,
            Path::new("/work/repo/.dev/executor-logs/20260611-100000-T-013-implement-ssh.session.json")
        );
    }

    #[test]
    fn json_carries_host_transport_session_and_result() {
        let json = to_json(&sample());
        assert!(json.contains("\"host\": \"alice@mac-mini\""));
        assert!(json.contains("\"transport\": \"ssh\""));
        assert!(json.contains("\"sessionId\": \"zellij:pipeline-20260611-abc123\""));
        assert!(json.contains("\"resultPath\""));
        assert!(json.contains("\"workNode\": \"mac-mini\""));
    }

    #[test]
    fn write_then_reparse_round_trips_into_executor_logs() {
        let tmp = TempDir::new().unwrap();
        let log_dir = executor_logs_dir(tmp.path());
        let rec = sample();
        let path = write_session_record(&log_dir, "20260611-100000", &rec).unwrap();

        assert!(path.exists());
        assert!(path.starts_with(&log_dir));
        let back: SessionRecord =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back, rec);
    }

    #[test]
    fn optional_fields_default_to_none() {
        let json = r#"{
            "taskId":"t","phase":"test","transport":"local","workNode":"n",
            "host":"u@h","dispatchedAtUtc":"now","terminalState":"completed"
        }"#;
        let rec: SessionRecord = serde_json::from_str(json).unwrap();
        assert_eq!(rec.session_id, None);
        assert_eq!(rec.finished_at_utc, None);
        assert_eq!(rec.result_path, None);
    }
}
