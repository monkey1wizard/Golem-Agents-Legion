use pipeline::report::{read_design_roster_version, EvidenceRecord, EvidenceStore};
use pipeline::usage::{Publication, StoreError};
use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
};

fn event(content: &str) -> EvidenceRecord {
    EvidenceRecord {
        schema_version: 1,
        event_id: "event-1".into(),
        operation_id: "operation-1".into(),
        run_id: "run-1".into(),
        plan_generation: None,
        task_id: None,
        attempt_id: None,
        parent_attempt_id: None,
        phase: "planning".into(),
        source_kind: "run".into(),
        source_version: "1".into(),
        recorded_at_unix_ms: 1,
        content_hash: "hash".into(),
        content: serde_json::json!(content),
    }
}

#[test]
fn evidence_is_immutable_and_separate() {
    let dir = tempfile::tempdir().unwrap();
    let store = EvidenceStore::open(dir.path()).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|value| {
            let store = store.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                store.publish(&event(value))
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Ok(Publication::Published)))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(StoreError::Conflict(_))))
            .count(),
        1
    );
    let winner = store.read("run-1", "event-1").unwrap();
    assert_eq!(store.publish(&winner).unwrap(), Publication::Replayed);
    assert_eq!(store.list_page("run-1", None, 10).unwrap().entries.len(), 1);
    assert!(!dir.path().join("gal/usage/v1").exists());
    let path = dir.path().join("gal/evidence/v1/run-1/evidence");
    fs::remove_file(path.join("event-1.manifest.json")).unwrap();
    fs::write(path.join(".other.manifest.json.tmp"), b"partial").unwrap();
    assert!(store
        .list_page("run-1", None, 10)
        .unwrap()
        .entries
        .is_empty());
    assert!(matches!(
        store.read("run-1", "event-1"),
        Err(StoreError::NotFound(_))
    ));
}

#[test]
fn roster_version_handles_absent_valid_and_malformed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir
        .path()
        .join("plugins/gal-core/conventions/governance-report.md");
    assert_eq!(
        read_design_roster_version(dir.path())
            .unwrap()
            .reason
            .as_deref(),
        Some("roster-absent")
    );
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    for (text, version, reason) in [
        ("no version", None, Some("roster-absent")),
        (
            "```\nRoster-Version: ignored\n```\nRoster-Version: 2026.1\n",
            Some("2026.1"),
            None,
        ),
        (
            "Roster-Version: 2026.1\nRoster-Version: 2026.2\n",
            None,
            Some("roster-malformed"),
        ),
        (
            "Roster-Version: bad value\n",
            None,
            Some("roster-malformed"),
        ),
    ] {
        fs::write(&path, text).unwrap();
        let result = read_design_roster_version(dir.path()).unwrap();
        assert_eq!(result.design_roster_version.as_deref(), version);
        assert_eq!(result.reason.as_deref(), reason);
    }
}
