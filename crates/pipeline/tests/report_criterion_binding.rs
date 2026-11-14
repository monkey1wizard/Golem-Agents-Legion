use pipeline::report::{criterion_status, CriterionMapping, CriterionRow, CriterionStatus};
use std::collections::BTreeSet;

fn mapping(task: &str) -> CriterionMapping {
    CriterionMapping {
        tp_id: plan_id("TP", 1),
        task_id: task.into(),
        gate_id: "gate".into(),
    }
}

#[test]
fn criterion_requires_every_expected_row() {
    let first = mapping(&plan_id("T", 1));
    let second = mapping(&plan_id("T", 2));
    let expected = BTreeSet::from([first.clone(), second.clone()]);
    let mut rows = vec![CriterionRow {
        mapping: first,
        status: CriterionStatus::Pass,
    }];
    assert_eq!(
        criterion_status(&expected, &rows),
        CriterionStatus::Unverified
    );
    rows.push(CriterionRow {
        mapping: second,
        status: CriterionStatus::Pass,
    });
    assert_eq!(criterion_status(&expected, &rows), CriterionStatus::Pass);
    rows[0].status = CriterionStatus::Fail;
    assert_eq!(criterion_status(&expected, &rows), CriterionStatus::Fail);
    rows[0].status = CriterionStatus::Superseded;
    assert_eq!(
        criterion_status(&expected, &rows),
        CriterionStatus::Superseded
    );
}

#[test]
fn duplicate_or_unknown_rows_do_not_pass() {
    let item = mapping(&plan_id("T", 1));
    let expected = BTreeSet::from([item.clone()]);
    let row = CriterionRow {
        mapping: item,
        status: CriterionStatus::Pass,
    };
    assert_eq!(
        criterion_status(&expected, &[row.clone(), row]),
        CriterionStatus::Unverified
    );
    assert_eq!(
        criterion_status(
            &expected,
            &[CriterionRow {
                mapping: mapping(&plan_id("T", 2)),
                status: CriterionStatus::Pass
            }]
        ),
        CriterionStatus::Unverified
    );
}

fn plan_id(prefix: &str, number: u32) -> String {
    format!("{prefix}-{number:02}")
}

#[test]
fn artifact_validity_is_sticky_and_checks_source() {
    use pipeline::report::{artifact_snapshot, ArtifactState, ArtifactValidity};
    let dir = tempfile::tempdir().unwrap();
    assert!(std::process::Command::new("git")
        .arg("init")
        .arg("-q")
        .arg(dir.path())
        .status()
        .unwrap()
        .success());
    let path = dir.path().join("artifact.txt");
    std::fs::write(&path, b"first\n").unwrap();
    let paths = BTreeSet::from(["artifact.txt".to_string(), "absent.txt".to_string()]);
    let original = artifact_snapshot(dir.path(), &paths).unwrap();
    assert_eq!(original.entries["absent.txt"], ArtifactState::Absent);
    let mut validity = ArtifactValidity::new(original.clone(), b"evidence");
    std::fs::write(dir.path().join("unrelated"), b"change").unwrap();
    assert_eq!(
        validity.check(&artifact_snapshot(dir.path(), &paths).unwrap(), b"evidence"),
        Ok(CriterionStatus::Pass)
    );
    assert_eq!(
        validity.check(&original, b"altered"),
        Err("integrity-failure")
    );
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        validity.check(&artifact_snapshot(dir.path(), &paths).unwrap(), b"evidence"),
        Ok(CriterionStatus::Superseded)
    );
    std::fs::write(&path, b"first\n").unwrap();
    assert_eq!(
        validity.check(&artifact_snapshot(dir.path(), &paths).unwrap(), b"evidence"),
        Ok(CriterionStatus::Superseded)
    );
}

fn observation(id: &str, output: u64) -> pipeline::usage::ObservationRecord {
    serde_json::from_value(serde_json::json!({
        "observation_id": id,
        "observation": {
            "schema_version": 1,
            "identity": {"plan_id":"plan", "task_id":"task", "attempt_id":id, "executor":"codex", "session_id":id},
            "tokens": {"output": {"count": {"value": {"state":"reported", "value":output}, "semantics":"delta", "descendants":"own", "window":{"start_unix_ms":1000,"end_unix_ms":2000}}, "relation":{"kind":"exclusive"}}},
            "cost":{"value":{"state":"unavailable","reason":"no cost"}}
        }
    })).unwrap()
}

fn git(repo: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn artifact_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("artifact.txt"), b"original\n").unwrap();
    dir
}

#[test]
fn shared_artifact_supersedes_both_results_without_changing_published_evidence() {
    use pipeline::report::{artifact_snapshot, ArtifactValidity, EvidenceRecord, EvidenceStore};
    use pipeline::usage::store::record_digest;

    let dir = artifact_repo();
    let paths = BTreeSet::from(["artifact.txt".to_string()]);
    let original = artifact_snapshot(dir.path(), &paths).unwrap();
    let store = EvidenceStore::open(dir.path()).unwrap();
    let first = mapping(&plan_id("T", 1));
    let second = mapping(&plan_id("T", 2));
    let record = EvidenceRecord {
        schema_version: 1,
        event_id: "first".into(),
        operation_id: "operation".into(),
        run_id: "run".into(),
        plan_generation: None,
        task_id: Some(first.task_id.clone()),
        attempt_id: None,
        parent_attempt_id: None,
        phase: "test".into(),
        source_kind: "test".into(),
        source_version: "1".into(),
        recorded_at_unix_ms: 1,
        content_hash: pipeline::usage::digest_bytes(b"pass"),
        content: serde_json::json!({"result": "pass"}),
    };
    store.publish(&record).unwrap();
    let old_entry = store.list_page("run", None, 10).unwrap().entries[0].clone();
    let digest = record_digest(&record).unwrap();
    assert_eq!(old_entry.digest, digest);
    let mut second_record = record.clone();
    second_record.event_id = "second".into();
    second_record.task_id = Some(second.task_id.clone());
    store.publish(&second_record).unwrap();
    let results_for = |mapping, record: &EvidenceRecord| {
        let source = serde_json::to_vec(record).unwrap();
        let validity = ArtifactValidity::new(original.clone(), &source);
        (mapping, validity, source)
    };
    let mut results = [
        results_for(first, &record),
        results_for(second, &second_record),
    ];

    std::fs::remove_file(dir.path().join("artifact.txt")).unwrap();
    let changed = artifact_snapshot(dir.path(), &paths).unwrap();
    for (mapping, validity, source) in &mut results {
        let status = validity.check(&changed, source).unwrap();
        assert_eq!(
            criterion_status(
                &BTreeSet::from([mapping.clone()]),
                &[CriterionRow {
                    mapping: mapping.clone(),
                    status
                }]
            ),
            CriterionStatus::Superseded
        );
    }
    let reread = store.read("run", "first").unwrap();
    assert_eq!(reread, record);
    assert_eq!(record_digest(&reread).unwrap(), digest);
    assert_eq!(
        store.list_page("run", None, 10).unwrap().entries[0],
        old_entry
    );

    std::fs::write(dir.path().join("artifact.txt"), b"original\n").unwrap();
    let restored = artifact_snapshot(dir.path(), &paths).unwrap();
    for (_, validity, source) in &mut results {
        assert_eq!(
            validity.check(&restored, source),
            Ok(CriterionStatus::Superseded)
        );
    }
}

#[test]
fn receipt_changes_outside_artifact_set_do_not_supersede() {
    use pipeline::report::{artifact_snapshot, ArtifactValidity};
    let dir = artifact_repo();
    let paths = BTreeSet::from(["artifact.txt".to_string()]);
    let mut validity =
        ArtifactValidity::new(artifact_snapshot(dir.path(), &paths).unwrap(), b"evidence");
    let receipt = dir.path().join("receipt.md");
    for content in [b"first".as_slice(), b"updated".as_slice()] {
        std::fs::write(&receipt, content).unwrap();
        assert_eq!(
            validity.check(&artifact_snapshot(dir.path(), &paths).unwrap(), b"evidence"),
            Ok(CriterionStatus::Pass)
        );
        assert!(!validity.superseded);
    }
}

#[test]
fn commit_without_artifact_change_keeps_snapshot_valid() {
    use pipeline::report::{artifact_snapshot, ArtifactValidity};
    let dir = artifact_repo();
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "--allow-empty",
            "-qm",
            "initial",
        ],
    );
    let paths = BTreeSet::from(["artifact.txt".to_string()]);
    let original = artifact_snapshot(dir.path(), &paths).unwrap();
    let mut validity = ArtifactValidity::new(original.clone(), b"evidence");
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "--allow-empty",
            "-qm",
            "next",
        ],
    );
    let current = artifact_snapshot(dir.path(), &paths).unwrap();
    assert_eq!(current, original);
    assert_eq!(
        validity.check(&current, b"evidence"),
        Ok(CriterionStatus::Pass)
    );
    assert!(!validity.superseded);
}

#[test]
fn freeze_deduplicates_all_committed_observations_and_rejects_conflicts() {
    use pipeline::usage::{SnapshotEvidence, UsageStore};
    let dir = tempfile::tempdir().unwrap();
    let store = UsageStore::open(dir.path()).unwrap();
    let publish = |id: &str, observations| {
        store
            .publish(&SnapshotEvidence {
                schema_version: 1,
                snapshot_id: id.into(),
                run_id: "run".into(),
                cutoff_unix_ms: 3000,
                observations,
                intervals: vec![],
            })
            .unwrap()
    };
    publish("one", vec![observation("shared", 1), observation("a", 2)]);
    publish("two", vec![observation("shared", 1), observation("b", 3)]);
    let frozen = pipeline::report::freeze_inputs(dir.path(), "run").unwrap();
    assert_eq!(frozen.usage_evidence_ids.len(), 2);
    assert_eq!(frozen.observations.len(), 3);
    frozen.verify(dir.path()).unwrap();
    assert!(frozen.reconcile(3000).is_ok());
    publish("three", vec![observation("shared", 9)]);
    assert!(pipeline::report::freeze_inputs(dir.path(), "run")
        .unwrap_err()
        .to_string()
        .contains("integrity-failure"));
    std::fs::write(
        dir.path().join("gal/usage/v1/run/evidence/one.record.json"),
        b"altered",
    )
    .unwrap();
    assert!(frozen
        .verify(dir.path())
        .unwrap_err()
        .to_string()
        .contains("integrity-failure"));
}
