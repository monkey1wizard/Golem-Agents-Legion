use pipeline::usage::{
    digest_bytes, ObservationDigest, Publication, RecordKind, StoreError, UsageAssessment,
    UsageSnapshot, UsageStore, RECORD_SCHEMA_VERSION,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Barrier},
    thread,
};

fn snapshot(run_id: &str, snapshot_id: &str, rate: Option<&str>) -> UsageSnapshot {
    UsageSnapshot {
        schema_version: RECORD_SCHEMA_VERSION,
        snapshot_id: snapshot_id.to_owned(),
        run_id: run_id.to_owned(),
        reconciliation_version: 1,
        rate_set_digest: rate.map(str::to_owned),
        observations: vec![ObservationDigest {
            observation_id: "obs-1".to_owned(),
            digest: "a".repeat(64),
        }],
    }
}

fn assessment(run_id: &str, assessment_id: &str, snapshot_digest: &str) -> UsageAssessment {
    UsageAssessment {
        schema_version: RECORD_SCHEMA_VERSION,
        assessment_id: assessment_id.to_owned(),
        run_id: run_id.to_owned(),
        snapshot_id: "snap-1".to_owned(),
        snapshot_digest: snapshot_digest.to_owned(),
        classification_rule: "direct-outcome".to_owned(),
        recorded_at_unix_ms: 1_800_000_000_000,
    }
}

fn kind_dir(common: &Path, run_id: &str, kind: RecordKind) -> PathBuf {
    common
        .join("gal")
        .join("usage")
        .join("v1")
        .join(run_id)
        .join(kind.dir_name())
}

fn record_bytes(record: &UsageSnapshot) -> Vec<u8> {
    serde_json::to_vec(record).expect("snapshot serializes")
}

fn open(common: &Path) -> UsageStore {
    UsageStore::open(common).expect("store opens under an existing common dir")
}

#[test]
fn published_snapshot_and_assessment_round_trip() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());

    let snap = snapshot("run-1", "snap-1", Some("rates-v1"));
    assert_eq!(store.publish(&snap).unwrap(), Publication::Published);
    let read_back: UsageSnapshot = store.read("run-1", "snap-1").unwrap();
    assert_eq!(read_back, snap);

    let digest = digest_bytes(&record_bytes(&snap));
    let judged = assessment("run-1", "assess-1", &digest);
    assert_eq!(store.publish(&judged).unwrap(), Publication::Published);
    let read_judgment: UsageAssessment = store.read("run-1", "assess-1").unwrap();
    assert_eq!(read_judgment, judged);
}

#[test]
fn identical_replay_is_idempotent() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let snap = snapshot("run-1", "snap-1", None);

    assert_eq!(store.publish(&snap).unwrap(), Publication::Published);
    assert_eq!(store.publish(&snap).unwrap(), Publication::Replayed);

    let page = store
        .list_page::<UsageSnapshot>("run-1", None, 10)
        .expect("page lists");
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.next_after, None);
}

#[test]
fn conflicting_replay_fails_and_keeps_the_original_bytes() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let original = snapshot("run-1", "snap-1", Some("rates-v1"));
    store.publish(&original).unwrap();

    let conflicting = snapshot("run-1", "snap-1", Some("rates-v2"));
    let error = store.publish(&conflicting).unwrap_err();
    assert!(matches!(error, StoreError::Conflict(ref id) if id == "snap-1"));

    let stored: UsageSnapshot = store.read("run-1", "snap-1").unwrap();
    assert_eq!(stored, original);
}

#[test]
fn concurrent_identical_publishers_produce_one_record() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let barrier = Arc::new(Barrier::new(8));

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let store = store.clone();
            let barrier = Arc::clone(&barrier);
            let snap = snapshot("run-1", "snap-1", Some("rates-v1"));
            thread::spawn(move || {
                barrier.wait();
                store.publish(&snap)
            })
        })
        .collect();
    let outcomes: Vec<Publication> = handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .expect("thread completes")
                .expect("publish succeeds")
        })
        .collect();

    let published = outcomes
        .iter()
        .filter(|outcome| **outcome == Publication::Published)
        .count();
    let replayed = outcomes
        .iter()
        .filter(|outcome| **outcome == Publication::Replayed)
        .count();
    assert_eq!((published, replayed), (1, 7));

    let mut files = fs::read_dir(kind_dir(common.path(), "run-1", RecordKind::Snapshot))
        .expect("kind dir exists")
        .map(|entry| {
            entry
                .expect("entry reads")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(files, ["snap-1.manifest.json", "snap-1.record.json"]);
}

#[test]
fn concurrent_conflicting_publishers_admit_exactly_one() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let barrier = Arc::new(Barrier::new(2));

    let handles: Vec<_> = ["rates-v1", "rates-v2"]
        .into_iter()
        .map(|rate| {
            let store = store.clone();
            let barrier = Arc::clone(&barrier);
            let snap = snapshot("run-1", "snap-1", Some(rate));
            thread::spawn(move || {
                barrier.wait();
                store.publish(&snap)
            })
        })
        .collect();
    let outcomes: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread completes"))
        .collect();

    assert_eq!(outcomes.iter().filter(|o| o.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| matches!(o, Err(StoreError::Conflict(_))))
            .count(),
        1
    );
}

#[test]
fn unpublished_record_is_invisible_until_its_manifest_lands() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let snap = snapshot("run-1", "snap-1", None);
    store.publish(&snap).unwrap();

    // Simulate a crash after the record rename but before the manifest rename.
    let dir = kind_dir(common.path(), "run-1", RecordKind::Snapshot);
    fs::remove_file(dir.join("snap-1.manifest.json")).unwrap();

    let page = store.list_page::<UsageSnapshot>("run-1", None, 10).unwrap();
    assert!(page.entries.is_empty());
    assert!(matches!(
        store.read::<UsageSnapshot>("run-1", "snap-1"),
        Err(StoreError::NotFound(_))
    ));

    // An identical replay completes the publication without a second record.
    assert_eq!(store.publish(&snap).unwrap(), Publication::Replayed);
    let page = store.list_page::<UsageSnapshot>("run-1", None, 10).unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(
        store.read::<UsageSnapshot>("run-1", "snap-1").unwrap(),
        snap
    );
}

#[test]
fn unpublished_record_with_different_bytes_still_conflicts() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    store.publish(&snapshot("run-1", "snap-1", None)).unwrap();
    let dir = kind_dir(common.path(), "run-1", RecordKind::Snapshot);
    fs::remove_file(dir.join("snap-1.manifest.json")).unwrap();

    let error = store
        .publish(&snapshot("run-1", "snap-1", Some("rates-v2")))
        .unwrap_err();
    assert!(matches!(error, StoreError::Conflict(_)));
}

#[test]
fn stale_temporary_file_from_an_interrupted_write_is_not_listed() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let dir = kind_dir(common.path(), "run-1", RecordKind::Snapshot);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(".snap-9.record.json.tmp"), b"{\"schema_").unwrap();
    fs::write(dir.join(".snap-9.manifest.json.tmp"), b"{").unwrap();

    let page = store.list_page::<UsageSnapshot>("run-1", None, 10).unwrap();
    assert!(page.entries.is_empty());
    assert!(matches!(
        store.read::<UsageSnapshot>("run-1", "snap-9"),
        Err(StoreError::NotFound(_))
    ));

    // The leftover temporary file does not block a later publication.
    assert_eq!(
        store.publish(&snapshot("run-1", "snap-9", None)).unwrap(),
        Publication::Published
    );
}

#[test]
fn read_rejects_a_record_that_no_longer_matches_its_digest() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    store.publish(&snapshot("run-1", "snap-1", None)).unwrap();

    let dir = kind_dir(common.path(), "run-1", RecordKind::Snapshot);
    let tampered = snapshot("run-1", "snap-1", Some("rates-tampered"));
    fs::write(dir.join("snap-1.record.json"), record_bytes(&tampered)).unwrap();

    let error = store.read::<UsageSnapshot>("run-1", "snap-1").unwrap_err();
    assert!(matches!(error, StoreError::DigestMismatch(ref id) if id == "snap-1"));
}

#[test]
fn pages_cover_every_record_exactly_once() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    for index in 0..5 {
        store
            .publish(&snapshot("run-1", &format!("snap-{index}"), None))
            .unwrap();
    }

    let mut seen = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let page = store
            .list_page::<UsageSnapshot>("run-1", after.as_deref(), 2)
            .unwrap();
        assert!(page.entries.len() <= 2, "page exceeds its limit");
        seen.extend(page.entries.into_iter().map(|entry| entry.record_id));
        match page.next_after {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    assert_eq!(seen, ["snap-0", "snap-1", "snap-2", "snap-3", "snap-4"]);
}

#[test]
fn page_limit_is_bounded() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());

    assert!(matches!(
        store.list_page::<UsageSnapshot>("run-1", None, 0),
        Err(StoreError::InvalidPageLimit(0))
    ));
    assert!(matches!(
        store.list_page::<UsageSnapshot>("run-1", None, 257),
        Err(StoreError::InvalidPageLimit(257))
    ));
}

#[test]
fn unsafe_identifiers_are_rejected_before_any_path_is_built() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());

    for bad in [
        "",
        "../escape",
        "a/b",
        "a\\b",
        ".hidden",
        "CON",
        "lpt3",
        "x..y",
    ] {
        let error = store
            .publish(&snapshot("run-1", bad, None))
            .expect_err("unsafe id must fail");
        assert!(
            matches!(error, StoreError::InvalidId(_)),
            "id {bad:?} gave {error:?}"
        );
    }
    let error = store
        .publish(&snapshot("../run", "snap-1", None))
        .expect_err("unsafe run id must fail");
    assert!(matches!(error, StoreError::InvalidId(_)));
    assert!(!common.path().join("gal").exists());
}

#[test]
fn run_directory_that_escapes_the_common_dir_is_rejected() {
    let common = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("tempdir");
    let usage_root = common.path().join("gal").join("usage").join("v1");
    fs::create_dir_all(&usage_root).unwrap();
    link_dir(outside.path(), &usage_root.join("run-x"));
    let store = open(common.path());

    let error = store
        .publish(&snapshot("run-x", "snap-1", None))
        .expect_err("escaping run dir must fail");
    assert!(
        matches!(error, StoreError::SymlinkEscape(_)),
        "got {error:?}"
    );
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(windows)]
fn link_dir(target: &Path, link: &Path) {
    // A junction needs no special privilege, unlike a directory symlink.
    let status = Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .status()
        .expect("cmd runs");
    assert!(status.success(), "mklink /J failed");
}

#[cfg(not(windows))]
fn link_dir(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("symlink created");
}
