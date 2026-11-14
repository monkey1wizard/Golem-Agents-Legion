use pipeline::{
    report::{EvidenceRecord, EvidenceStore},
    usage::digest_bytes,
};
use serde_json::{json, Value};
use std::{
    fs,
    process::{Command, Output},
};

struct Fixture {
    repo: tempfile::TempDir,
    input: Value,
}

impl Fixture {
    fn new() -> Self {
        let repo = tempfile::tempdir().unwrap();
        assert!(Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());
        fs::create_dir_all(repo.path().join(".dev")).unwrap();
        let register = "# Mistake Register\n\nExisting unstructured entry remains readable.\n";
        fs::write(repo.path().join(".dev/mistake-register.md"), register).unwrap();
        fs::write(repo.path().join("proof.txt"), "verified rule\n").unwrap();
        let reference =
            json!({"evidence_id":"proof.txt","sha256":digest_bytes(b"verified rule\n")});
        let input = json!({"schema_version":1,"correction_id":"safer-rule","revision":1,
            "incident_ids":["source-failure"],
            "mechanism":{"mechanism_id":"rule-check","path":"src/check.rs","level":"behavior-test","reason_for_level":"Behavior depends on runtime input."},
            "rule":{"rule_id":"protect-source","version":"1","effective_at":1000,"evidence_refs":[reference.clone()]},
            "verification":{"status":"pass","evidence_refs":[reference]},
            "register_sha256":digest_bytes(register.as_bytes())});
        Self { repo, input }
    }

    fn call(&self, text: &str) -> Output {
        fs::write(self.repo.path().join("correction.json"), text).unwrap();
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .args(["usage", "correction", "--input", "correction.json"])
            .current_dir(self.repo.path())
            .output()
            .unwrap()
    }

    fn register(&self) -> Value {
        let output = self.call(&self.input.to_string());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn store(&self) -> EvidenceStore {
        EvidenceStore::open(&self.repo.path().join(".git")).unwrap()
    }

    fn corrections(&self) -> Vec<EvidenceRecord> {
        let mut records = Vec::new();
        for directory in fs::read_dir(self.repo.path().join(".git/gal/evidence/v1")).unwrap() {
            let directory = directory.unwrap();
            if !directory.file_type().unwrap().is_dir() {
                continue;
            }
            let scope = directory.file_name().to_string_lossy().into_owned();
            let mut cursor = None;
            loop {
                let page = self
                    .store()
                    .list_page(&scope, cursor.as_deref(), 256)
                    .unwrap();
                for entry in page.entries {
                    let record = self.store().read(&scope, &entry.record_id).unwrap();
                    if record.source_kind == "correction" {
                        records.push(record);
                    }
                }
                cursor = page.next_after;
                if cursor.is_none() {
                    break;
                }
            }
        }
        records
    }

    fn failure(
        &self,
        id: &str,
        operation: &str,
        attempt: Option<&str>,
        parent: Option<&str>,
        time: u64,
        content: Value,
    ) -> EvidenceRecord {
        let record = EvidenceRecord {
            schema_version: 1,
            event_id: id.into(),
            operation_id: operation.into(),
            run_id: "fixture-run".into(),
            plan_generation: None,
            task_id: None,
            attempt_id: attempt.map(str::to_owned),
            parent_attempt_id: parent.map(str::to_owned),
            phase: "pipeline".into(),
            source_kind: "operation-end".into(),
            source_version: "1".into(),
            recorded_at_unix_ms: time,
            content_hash: digest_bytes(&serde_json::to_vec(&content).unwrap()),
            content,
        };
        self.store().publish(&record).unwrap();
        record
    }
}

fn rejected(output: Output, expected: &str) {
    assert!(!output.status.success(), "unexpected success");
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains(expected), "expected {expected}: {error}");
}

#[test]
fn correction_is_stored_in_its_entry_scope_with_register_snapshot() {
    let fixture = Fixture::new();
    let register_path = fixture.repo.path().join(".dev/mistake-register.md");
    let register = fs::read(&register_path).unwrap();
    let first = fixture.register();
    let second = fixture.register();
    assert_eq!(first, second);
    let records = fixture.corrections();
    assert_eq!(records.len(), 1);
    let correction = &records[0];
    assert_eq!(correction.event_id, "correction-safer-rule-r1");
    assert_eq!(
        correction.run_id,
        format!(
            "entry-{}",
            correction.operation_id.strip_prefix("op-").unwrap()
        )
    );
    assert_eq!(correction.content["run_id"], Value::Null);
    assert_eq!(correction.content["task_id"], Value::Null);
    assert_eq!(
        correction.content["correction_id"],
        fixture.input["correction_id"]
    );
    assert_eq!(
        correction.content["register_snapshot"]["text"],
        String::from_utf8(register.clone()).unwrap()
    );
    assert_eq!(fs::read(register_path).unwrap(), register);
    let begin = fixture
        .store()
        .read(&correction.run_id, &correction.operation_id)
        .unwrap();
    assert_eq!(begin.source_kind, "operation-begin");
    assert_eq!(begin.content["run_id"], Value::Null);
    assert_eq!(begin.task_id, None);
}

#[test]
fn correction_rejects_conflicts_and_keeps_revisions() {
    let mut fixture = Fixture::new();
    fixture.register();
    let original = fixture.corrections()[0].clone();
    fixture.input["mechanism"]["reason_for_level"] = json!("Different reason.");
    rejected(fixture.call(&fixture.input.to_string()), "conflict");
    assert_eq!(fixture.corrections(), vec![original]);
    fixture.input["revision"] = json!(2);
    fixture.register();
    assert_eq!(fixture.corrections().len(), 2);
}

#[test]
fn correction_refuses_invalid_schema_values_and_closed_object_keys() {
    let fixture = Fixture::new();
    for (pointer, value, error) in [
        ("/schema_version", json!(2), "schema_version"),
        ("/revision", json!(0), "revision"),
        ("/revision", json!(1.5), "revision"),
        ("/correction_id", json!("../escape"), "identifier"),
        ("/correction_id", json!("CON"), "identifier"),
        ("/correction_id", json!("x".repeat(128)), "identifier"),
        ("/incident_ids", json!(["same", "same"]), "unique"),
        ("/incident_ids", json!(null), "array"),
        ("/mechanism/level", json!("reminder"), "mechanism.level"),
        ("/mechanism/path", json!(""), "path"),
        ("/rule/rule_id", json!("  "), "rule_id"),
        ("/rule/version", json!(false), "rule.version"),
        ("/rule/effective_at", json!(-1), "effective_at"),
        (
            "/verification/status",
            json!("passed"),
            "verification.status",
        ),
        ("/verification/evidence_refs", json!([]), "require evidence"),
        ("/rule/evidence_refs/0/sha256", json!("bad"), "sha256"),
    ] {
        let mut input = fixture.input.clone();
        *input.pointer_mut(pointer).unwrap() = value;
        rejected(fixture.call(&input.to_string()), error);
    }
    for pointer in [
        "",
        "/mechanism",
        "/rule",
        "/verification",
        "/rule/evidence_refs/0",
    ] {
        let mut input = fixture.input.clone();
        input
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), json!(true));
        rejected(fixture.call(&input.to_string()), "required keys");
    }
    for field in fixture.input.as_object().unwrap().keys() {
        let mut input = fixture.input.clone();
        input.as_object_mut().unwrap().remove(field);
        rejected(fixture.call(&input.to_string()), "required keys");
    }
}

#[test]
fn correction_refuses_duplicate_keys_at_every_object_depth() {
    let fixture = Fixture::new();
    let text = fixture.input.to_string();
    for (original, duplicated) in [
        ("\"revision\":1", "\"revision\":1,\"revision\":1"),
        (
            "\"rule_id\":\"protect-source\"",
            "\"rule_id\":\"protect-source\",\"rule\\u005fid\":\"protect-source\"",
        ),
        (
            "\"status\":\"pass\"",
            "\"status\":\"pass\",\"status\":\"pass\"",
        ),
        (
            "\"evidence_id\":\"proof.txt\"",
            "\"evidence_id\":\"proof.txt\",\"evidence_id\":\"proof.txt\"",
        ),
        (
            "\"level\":\"behavior-test\"",
            "\"level\":\"behavior-test\",\"level\":\"behavior-test\"",
        ),
    ] {
        assert!(text.contains(original));
        rejected(
            fixture.call(&text.replace(original, duplicated)),
            "duplicate key",
        );
    }
}

#[test]
fn correction_refuses_changed_register_and_unreadable_or_changed_references() {
    for (pointer, value, error) in [
        (
            "/register_sha256",
            json!("0".repeat(64)),
            "register digest mismatch",
        ),
        (
            "/rule/evidence_refs/0/sha256",
            json!("0".repeat(64)),
            "reference digest mismatch",
        ),
        (
            "/verification/evidence_refs/0/sha256",
            json!("0".repeat(64)),
            "reference digest mismatch",
        ),
        (
            "/rule/evidence_refs/0/evidence_id",
            json!("missing.txt"),
            "unreadable evidence reference",
        ),
        (
            "/verification/evidence_refs/0/evidence_id",
            json!("missing.txt"),
            "unreadable evidence reference",
        ),
    ] {
        let fixture = Fixture::new();
        let mut input = fixture.input.clone();
        *input.pointer_mut(pointer).unwrap() = value;
        rejected(fixture.call(&input.to_string()), error);
        let root = fixture.repo.path().join(".git/gal/evidence/v1");
        if root.exists() {
            assert!(fixture.corrections().is_empty());
        }
    }
}

#[test]
fn later_failures_associate_once_across_retries_and_translation_copies() {
    let fixture = Fixture::new();
    fixture.failure(
        "source-failure",
        "source-op",
        Some("source-attempt"),
        None,
        1500,
        json!({"status":"failed","rule_id":"protect-source"}),
    );
    assert_eq!(fixture.register()["association"]["recurrence_count"], 0);
    fixture.failure(
        "later-failure",
        "later-op",
        Some("first-attempt"),
        None,
        2000,
        json!({"status":"failed","rule_id":"protect-source","error":"same error"}),
    );
    fixture.failure(
        "retry-failure",
        "later-op",
        Some("second-attempt"),
        Some("first-attempt"),
        3000,
        json!({"status":"failed","rule_id":"protect-source"}),
    );
    fixture.failure(
        "source-retry",
        "source-op",
        Some("source-retry-attempt"),
        Some("source-attempt"),
        3000,
        json!({"status":"failed","rule_id":"protect-source"}),
    );
    fixture.failure(
        "unrelated",
        "unrelated-op",
        None,
        None,
        2000,
        json!({"status":"failed","error":"same error"}),
    );
    fixture.failure(
        "other-rule",
        "other-op",
        None,
        None,
        2000,
        json!({"status":"failed","rule_id":"another-rule"}),
    );
    fixture.failure(
        "success",
        "success-op",
        None,
        None,
        2000,
        json!({"status":"completed","rule_id":"protect-source"}),
    );
    fixture.failure("referenced-rule", "reference-op", None, None, 1000,
        json!({"status":"failed","incident_id":"explicit-incident","evidence_refs":[{"rule_id":"protect-source"}]}));
    let result = fixture.register();
    assert_eq!(result["association"]["recurrence_count"], 2);
    assert_eq!(
        result["association"]["incidents"],
        json!([
            {"incident_id":"explicit-incident","status":"recurrence"},
            {"incident_id":"later-failure","status":"recurrence"}
        ])
    );
    for translation in ["zh-TW", "ja-JP"] {
        fs::write(
            fixture
                .repo
                .path()
                .join(format!("register.{translation}.md")),
            translation,
        )
        .unwrap();
        let output = fixture.call(&serde_json::to_string_pretty(&fixture.input).unwrap());
        assert!(output.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            result
        );
    }
    assert_eq!(fixture.corrections().len(), 1);
}

#[test]
fn source_aliases_and_pre_effect_incidents_stay_out_of_recurrence() {
    let mut fixture = Fixture::new();
    fixture.failure(
        "early-failure",
        "early-op",
        Some("early-first"),
        None,
        999,
        json!({"status":"failed","rule_id":"protect-source"}),
    );
    fixture.failure(
        "early-retry",
        "early-op",
        Some("early-second"),
        Some("early-first"),
        2000,
        json!({"status":"failed","rule_id":"protect-source"}),
    );
    fixture.failure(
        "source-failure",
        "source-op",
        Some("source-first"),
        None,
        2000,
        json!({"status":"failed","incident_id":"source-incident","rule_id":"protect-source"}),
    );
    fixture.failure(
        "source-retry",
        "source-op",
        Some("source-second"),
        Some("source-first"),
        3000,
        json!({"status":"failed","rule_id":"protect-source"}),
    );
    fixture.input["incident_ids"] = json!(["source-retry", "source-failure", "source-incident"]);
    let result = fixture.register();
    assert_eq!(
        result["association"]["source_incident_ids"],
        json!(["source-incident"])
    );
    assert_eq!(result["association"]["recurrence_count"], 0);
    assert_eq!(
        result["association"]["recurrence_denominator"],
        "unavailable"
    );
}

#[test]
fn unknown_rule_metadata_keeps_association_unknown() {
    for field in ["version", "effective_at"] {
        let mut fixture = Fixture::new();
        fixture.input["rule"][field] = Value::Null;
        fixture.input["incident_ids"] = json!([]);
        fixture.input["mechanism"] = Value::Null;
        fixture.input["verification"] = json!({"status":"not-run","evidence_refs":[]});
        fixture.failure(
            "named-failure",
            "named-op",
            None,
            None,
            2000,
            json!({"status":"failed","rule_id":"protect-source"}),
        );
        fixture.failure(
            "unnamed-failure",
            "unnamed-op",
            None,
            None,
            2000,
            json!({"status":"failed"}),
        );
        let result = fixture.register();
        assert_eq!(result["association"]["source_association"], "unknown");
        assert_eq!(
            result["association"]["incidents"],
            json!([{"incident_id":"named-failure","status":"unknown"}])
        );
        assert_eq!(result["association"]["recurrence_count"], 0);
        assert_eq!(
            result["association"]["recurrence_denominator"],
            "unavailable"
        );
    }
}

#[test]
fn attempt_names_do_not_link_different_operations_and_explicit_ids_dedupe() {
    let fixture = Fixture::new();
    for (id, operation, attempt, parent, content) in [
        (
            "first-failure",
            "first-op",
            "first",
            None,
            json!({"status":"failed","rule_id":"protect-source"}),
        ),
        (
            "other-failure",
            "other-op",
            "second",
            Some("first"),
            json!({"status":"failed","rule_id":"protect-source"}),
        ),
        (
            "explicit-first",
            "explicit-op",
            "explicit-one",
            None,
            json!({"status":"failed","incident_id":"shared-incident","rule_id":"protect-source"}),
        ),
        (
            "explicit-second",
            "explicit-op",
            "explicit-two",
            None,
            json!({"status":"failed","incident_id":"shared-incident","rule_id":"protect-source"}),
        ),
    ] {
        fixture.failure(id, operation, Some(attempt), parent, 2000, content);
    }
    assert_eq!(fixture.register()["association"]["recurrence_count"], 3);
}

#[test]
fn published_evidence_references_are_digest_checked() {
    let mut fixture = Fixture::new();
    let record = fixture.failure(
        "source-failure",
        "source-op",
        None,
        None,
        900,
        json!({"status":"failed"}),
    );
    let reference = json!({"evidence_id":record.event_id,"sha256":pipeline::usage::store::record_digest(&record).unwrap()});
    fixture.input["rule"]["evidence_refs"] = json!([reference.clone()]);
    fixture.input["verification"]["evidence_refs"] = json!([reference]);
    fixture.register();
    fixture.input["verification"]["evidence_refs"][0]["sha256"] = json!("0".repeat(64));
    rejected(
        fixture.call(&fixture.input.to_string()),
        "reference digest mismatch",
    );
}

#[test]
fn effective_time_accepts_utc_and_offset_timestamps_at_the_inclusive_boundary() {
    for timestamp in [
        "1970-01-01T00:00:01Z",
        "1970-01-01T01:00:01+01:00",
        "1970-01-01T00:00:01.000000000z",
        "1970-01-01T00:00:00.999Z",
        "1970-01-01T00:00:00.999000001Z",
    ] {
        let mut fixture = Fixture::new();
        let boundary = if timestamp.ends_with(".999Z") {
            999
        } else {
            1000
        };
        fixture.input["rule"]["effective_at"] = json!(timestamp);
        fixture.failure(
            "before-effect",
            "before-op",
            None,
            None,
            boundary - 1,
            json!({"status":"failed","rule_id":"protect-source"}),
        );
        fixture.failure(
            "at-effect",
            "at-op",
            None,
            None,
            boundary,
            json!({"status":"failed","rule_id":"protect-source"}),
        );
        assert_eq!(
            fixture.register()["association"]["incidents"],
            json!([{"incident_id":"at-effect","status":"recurrence"}])
        );
    }
    let fixture = Fixture::new();
    for timestamp in [
        "invalid",
        "2026-02-29T00:00:00Z",
        "2026-01-01T24:00:00Z",
        "2026-01-01T00:00:00+25:00",
    ] {
        let mut input = fixture.input.clone();
        input["rule"]["effective_at"] = json!(timestamp);
        rejected(fixture.call(&input.to_string()), "effective_at");
    }
}

#[test]
fn correction_uses_repository_register_when_invoked_from_a_subdirectory() {
    let fixture = Fixture::new();
    let first = fixture.register();
    let subdirectory = fixture.repo.path().join("nested");
    fs::create_dir(&subdirectory).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .args(["usage", "correction", "--input", "../correction.json"])
        .current_dir(subdirectory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        first
    );
    assert_eq!(fixture.corrections().len(), 1);
}

#[test]
fn correction_rejects_missing_extra_and_duplicate_options() {
    let fixture = Fixture::new();
    for args in [
        vec!["usage", "correction"],
        vec!["usage", "correction", "--input"],
        vec!["usage", "correction", "--run", "unneeded-run"],
        vec![
            "usage",
            "correction",
            "--input",
            "correction.json",
            "--input",
            "correction.json",
        ],
        vec![
            "usage",
            "correction",
            "--input",
            "correction.json",
            "--run",
            "unneeded-run",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_gal"))
            .args(args)
            .current_dir(fixture.repo.path())
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(gal_engine::ExitCode::Usage as i32)
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
    }
}
