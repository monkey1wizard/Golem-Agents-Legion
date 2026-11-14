use pipeline::{
    report::{EvidenceRecord, EvidenceStore},
    usage::digest_bytes,
};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

struct Fixture {
    repo: tempfile::TempDir,
    external: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let repo = tempfile::tempdir().unwrap();
        let output = Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(output.status.success());
        fs::create_dir_all(repo.path().join(".dev/plans")).unwrap();
        fs::create_dir_all(repo.path().join(".dev/pipeline/archive")).unwrap();
        for name in ["archive.md", "archive.prompt.md"] {
            fs::write(
                repo.path().join(".dev/plans").join(name),
                "# Archive\nrequirements and criteria\n",
            )
            .unwrap();
        }
        fs::write(repo.path().join(".dev/mistake-register.md"), "# Mistakes\n").unwrap();
        fs::write(
            repo.path()
                .join(".dev/pipeline/archive/preflight.receipt.md"),
            "run_id: archive-run\n",
        )
        .unwrap();
        Self {
            repo,
            external: tempfile::tempdir().unwrap(),
        }
    }

    fn call(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .args(args)
            .current_dir(self.repo.path())
            .output()
            .unwrap()
    }

    fn archive(&self, boundary: &str) -> Output {
        self.call(&[
            "finalize-check",
            ".dev/plans/archive.prompt.md",
            "--archive",
            "--run",
            "archive-run",
            "--source-worktree",
            self.repo.path().to_str().unwrap(),
            "--boundary",
            boundary,
        ])
    }

    fn verify(&self, proof: &str, set: &str, run: &str, boundary: &str) -> Output {
        self.call(&[
            "finalize-check",
            "--verify-archive",
            "--archive-proof",
            proof,
            "--source-set",
            set,
            "--run",
            run,
            "--source-worktree",
            self.repo.path().to_str().unwrap(),
            "--boundary",
            boundary,
        ])
    }

    fn proof_path(&self, id: &str) -> PathBuf {
        self.repo
            .path()
            .join(".git/gal/archive/v1/proofs")
            .join(format!("{id}.json"))
    }

    fn proof(&self, id: &str) -> Value {
        serde_json::from_slice(&fs::read(self.proof_path(id)).unwrap()).unwrap()
    }

    fn publish(&self, id: &str, content: Value) {
        EvidenceStore::open(&self.repo.path().join(".git"))
            .unwrap()
            .publish(&EvidenceRecord {
                schema_version: 1,
                event_id: id.into(),
                operation_id: "op-0123456789abcdef".into(),
                run_id: "archive-run".into(),
                plan_generation: None,
                task_id: None,
                attempt_id: None,
                parent_attempt_id: None,
                phase: "test".into(),
                source_kind: "source-registration".into(),
                source_version: "1".into(),
                recorded_at_unix_ms: 1,
                content_hash: digest_bytes(&serde_json::to_vec(&content).unwrap()),
                content,
            })
            .unwrap();
    }

    fn remove_plans(&self) {
        for name in ["archive.md", "archive.prompt.md"] {
            fs::remove_file(self.repo.path().join(".dev/plans").join(name)).unwrap();
        }
    }

    fn main_proof(&self) -> String {
        let output = self.archive("pipeline-clean");
        let id = field(&output, "archive_proof_id");
        field(
            &self.verify(
                &id,
                &field(&output, "source_set_id"),
                "archive-run",
                "pipeline-clean",
            ),
            "terminal_operation_id",
        );
        id
    }

    fn clean(&self, id: &str) -> Output {
        self.call(&[
            "pipeline-clean",
            ".dev/plans/archive.prompt.md",
            "--run",
            "archive-run",
            "--archive-proof",
            id,
        ])
    }

    fn terminal(&self, output: &Output, status: &str) -> Value {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let operation = stdout
            .lines()
            .find_map(|line| line.strip_prefix("terminal_operation_id: "))
            .unwrap();
        let value: Value = serde_json::from_slice(
            &fs::read(
                self.repo
                    .path()
                    .join(".git/gal/archive/v1/operations")
                    .join(format!("{operation}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(value["operation_id"], operation);
        assert_eq!(value["terminal_operation_id"], operation);
        assert_eq!(value["source_kind"], "operation-end");
        assert_eq!(value["status"], status);
        value
    }
}

fn field(output: &Output, key: &str) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}: ")))
        .unwrap()
        .to_owned()
}

fn refused(output: Output) {
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("refused"));
}

#[test]
fn cleanup_without_proof_refuses_bound_run_and_preserves_scope() {
    let fixture = Fixture::new();
    for name in ["archive.md", "archive.prompt.md"] {
        fs::remove_file(fixture.repo.path().join(".dev/plans").join(name)).unwrap();
    }
    refused(fixture.call(&["pipeline-clean", ".dev/plans/archive.prompt.md"]));
    assert!(fixture
        .repo
        .path()
        .join(".dev/pipeline/archive/preflight.receipt.md")
        .exists());
}

#[test]
fn cleanup_without_bound_run_reports_archive_absence() {
    let fixture = Fixture::new();
    for name in ["archive.md", "archive.prompt.md"] {
        fs::remove_file(fixture.repo.path().join(".dev/plans").join(name)).unwrap();
    }
    fs::remove_file(
        fixture
            .repo
            .path()
            .join(".dev/pipeline/archive/preflight.receipt.md"),
    )
    .unwrap();
    let output = fixture.call(&["pipeline-clean", ".dev/plans/archive.prompt.md"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("archive: none (no-bound-run)"));
    assert!(!fixture.repo.path().join(".dev/pipeline/archive").exists());
}

#[test]
fn archive_verifies_without_report_and_retains_closeout_identity() {
    for boundary in ["worktree-remove", "pipeline-clean"] {
        let fixture = Fixture::new();
        let external = fixture.external.path().join("diagnostic.txt");
        fs::write(&external, "failure diagnostic\n").unwrap();
        fixture.publish("external-output", json!({"output_path":external}));
        let output = fixture.archive(boundary);
        let id = field(&output, "archive_proof_id");
        let set = field(&output, "source_set_id");
        let proof = fixture.proof(&id);
        assert_eq!(proof["source_set_id"], set);
        assert!(proof["archive_set"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["content"] == "failure diagnostic\n"));
        assert!(!proof["to_delete_set"]
            .as_array()
            .unwrap()
            .contains(&json!(external)));
        let verified = fixture.verify(&id, &set, "archive-run", boundary);
        assert_eq!(
            field(&verified, "terminal_operation_id"),
            proof["terminal_operation_id"].as_str().unwrap()
        );
        assert!(fixture
            .repo
            .path()
            .join(".git/gal/archive/v1/closeout")
            .join(format!("{id}.json"))
            .is_file());
        assert!(fixture
            .repo
            .path()
            .join(".git/gal/archive/v1/verified")
            .join(format!("{id}.json"))
            .is_file());
        assert!(!fixture
            .repo
            .path()
            .join(".dev/pipeline/archive/finalize-check.receipt.md")
            .exists());
        assert!(!fixture
            .repo
            .path()
            .join(".dev/pipeline/archive/finalize-check.hygiene.receipt.md")
            .exists());
        assert!(fixture.repo.path().exists());
    }
}

#[test]
fn archive_rejects_parent_components_in_source_worktree() {
    let fixture = Fixture::new();
    let worktree = format!("{}/.dev/..", fixture.repo.path().display());
    let output = fixture.call(&[
        "finalize-check",
        ".dev/plans/archive.prompt.md",
        "--archive",
        "--run",
        "archive-run",
        "--source-worktree",
        &worktree,
        "--boundary",
        "worktree-remove",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("archive source contains parent traversal"));
    assert!(fixture
        .repo
        .path()
        .join(".dev/plans/archive.prompt.md")
        .is_file());
    assert!(!fixture
        .repo
        .path()
        .join(".git/gal/archive/v1/proofs")
        .exists());
}

#[test]
fn verification_refuses_wrong_identity_changed_sources_and_corrupt_archive() {
    for change in [
        "run", "set", "boundary", "digest", "added", "deleted", "corrupt", "reopened",
    ] {
        let fixture = Fixture::new();
        let output = fixture.archive("worktree-remove");
        let id = field(&output, "archive_proof_id");
        let set = field(&output, "source_set_id");
        match change {
            "digest" => fs::write(
                fixture.repo.path().join(".dev/mistake-register.md"),
                "changed",
            )
            .unwrap(),
            "added" => fs::write(
                fixture
                    .repo
                    .path()
                    .join(".dev/pipeline/archive/new-output.txt"),
                "new",
            )
            .unwrap(),
            "deleted" => {
                fs::remove_file(fixture.repo.path().join(".dev/plans/archive.md")).unwrap()
            }
            "corrupt" => fs::write(fixture.proof_path(&id), "{}").unwrap(),
            "reopened" => {
                fs::remove_file(
                    fixture
                        .repo
                        .path()
                        .join(".git/gal/usage/v1/archive-run/.closed"),
                )
                .unwrap();
            }
            _ => {}
        }
        refused(fixture.verify(
            &id,
            if change == "set" { "wrong" } else { &set },
            if change == "run" {
                "wrong-run"
            } else {
                "archive-run"
            },
            if change == "boundary" {
                "pipeline-clean"
            } else {
                "worktree-remove"
            },
        ));
        assert!(fixture.repo.path().exists());
    }
}

#[test]
fn verification_refuses_another_worktree() {
    let fixture = Fixture::new();
    let output = fixture.archive("worktree-remove");
    let id = field(&output, "archive_proof_id");
    let set = field(&output, "source_set_id");
    let other = Fixture::new();
    fs::create_dir_all(other.repo.path().join(".git/gal/archive/v1/proofs")).unwrap();
    fs::copy(fixture.proof_path(&id), other.proof_path(&id)).unwrap();
    refused(other.verify(&id, &set, "archive-run", "worktree-remove"));
    assert!(fixture.repo.path().exists());
}

#[test]
fn active_writer_uncovered_source_and_archive_write_failure_refuse() {
    for change in ["active", "missing", "digest", "write-failure"] {
        let fixture = Fixture::new();
        match change {
            "active" => {
                fs::write(fixture.repo.path().join(".dev/pipeline/archive/coordinator.json"), r#"{"accounting_run":{"run_id":"archive-run"},"active_attempt":{"attempt_id":"busy"}}"#).unwrap();
            }
            "missing" => fixture.publish(
                "missing",
                json!({"output_path":fixture.external.path().join("missing.txt")}),
            ),
            "digest" => {
                let path = fixture.external.path().join("spec.md");
                fs::write(&path, "changed").unwrap();
                fixture.publish(
                    "mismatch",
                    json!({"spec_path":path,"spec_sha256":digest_bytes(b"original")}),
                );
            }
            "write-failure" => {
                fs::create_dir_all(fixture.repo.path().join(".git/gal/archive/v1")).unwrap();
                fs::write(
                    fixture.repo.path().join(".git/gal/archive/v1/proofs"),
                    "blocked",
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        refused(fixture.archive("worktree-remove"));
        assert!(fixture
            .repo
            .path()
            .join(".dev/plans/archive.prompt.md")
            .is_file());
    }
}

fn host_archive_attempt(
    fixture: &Fixture,
    attempt: &str,
    terminal: &str,
) -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
    use dispatch::host_attempt_state::HostAttemptState;

    let root = fixture.repo.path().canonicalize().unwrap();
    let task = root.join(".dev/pipeline/archive/task-a");
    let attempts = task.join("host-attempts");
    fs::create_dir_all(&attempts).unwrap();
    let snapshot = task.join("task-a-test.prompt.bak");
    let spec = attempts.join(format!("{attempt}.spec.md"));
    let log = attempts.join(format!("{attempt}.log.md"));
    let receipt = task.join("task-a-test.receipt.md");
    let manifest = attempts.join(format!("{attempt}.json"));
    let state: HostAttemptState = serde_json::from_value(json!({
        "attempt_id": attempt,
        "worktree": root,
        "prompt": root.join(".dev/plans/archive.prompt.md"),
        "task": "task-a",
        "phase": "test",
        "profile": "legacy_interactive",
        "run_id": "archive-run",
        "host": "codex",
        "mode": "host-subagent",
        "spec_path": spec,
        "spec_sha256": digest_bytes(b"spec"),
        "receipt_path": receipt,
        "log_path": log,
        "snapshot_path": snapshot,
        "snapshot_sha256": digest_bytes(attempt.as_bytes()),
        "started_at": "2026-11-13T10:00:00.000Z",
        "model": "test-model",
    }))
    .unwrap();
    fs::write(&manifest, state.encode_immutable().unwrap()).unwrap();
    fs::write(
        &log,
        format!(
            "GAL-DISPATCH-LOG v1\ntimestamp_start: {}\nattempt_id: {attempt}\nexecutor: host-subagent\nhost: codex\nphase: test\ntask_id: task-a\nterminal_state: {terminal}\n---STDOUT---\n\n---STDERR---\n",
            state.started_at
        ),
    )
    .unwrap();
    fs::write(&receipt, "task receipt").unwrap();
    (manifest, log, snapshot, spec, receipt)
}

#[test]
fn main_archive_excludes_unbound_gate_evidence_for_another_plan() {
    let fixture = Fixture::new();
    let root = fixture.repo.path();
    let store = EvidenceStore::open(&root.join(".git")).unwrap();
    let operation = "op-1234567890abcdef";
    let entry = "entry-1234567890abcdef";
    for (event_id, kind, content) in [
        (
            operation,
            "operation-begin",
            json!({"run_id":null,"worktree":root}),
        ),
        (
            "foreign-expectation",
            "verification-expected",
            json!({"receipt_path":".dev/pipeline/closed-plan/finalize-check.hygiene.receipt.md"}),
        ),
        (
            "foreign-end",
            "operation-end",
            json!({"status":"completed"}),
        ),
    ] {
        store
            .publish(&EvidenceRecord {
                schema_version: 1,
                event_id: event_id.into(),
                operation_id: operation.into(),
                run_id: entry.into(),
                plan_generation: None,
                task_id: None,
                attempt_id: None,
                parent_attempt_id: None,
                phase: "receipt-verification".into(),
                source_kind: kind.into(),
                source_version: "1".into(),
                recorded_at_unix_ms: 1,
                content_hash: digest_bytes(&serde_json::to_vec(&content).unwrap()),
                content,
            })
            .unwrap();
    }
    let output = fixture.archive("pipeline-clean");
    let id = field(&output, "archive_proof_id");
    let set = field(&output, "source_set_id");
    field(
        &fixture.verify(&id, &set, "archive-run", "pipeline-clean"),
        "terminal_operation_id",
    );
    assert!(!fixture.proof(&id).to_string().contains(entry));
    assert!(store.read(entry, "foreign-expectation").is_ok());
}
#[test]
fn archive_verifies_cleaned_host_attempts_with_shared_snapshot_path() {
    let fixture = Fixture::new();
    let completed = host_archive_attempt(&fixture, "host_1_1", "completed");
    let abandoned = host_archive_attempt(&fixture, "host_1_2", "abandoned");
    let output = fixture.archive("worktree-remove");
    let id = field(&output, "archive_proof_id");
    let set = field(&output, "source_set_id");
    field(
        &fixture.verify(&id, &set, "archive-run", "worktree-remove"),
        "terminal_operation_id",
    );
    let proof = fixture.proof(&id);
    let sources = proof["archive_set"].as_array().unwrap();
    for path in [
        &completed.0,
        &completed.1,
        &abandoned.0,
        &abandoned.1,
        &completed.4,
    ] {
        assert!(sources
            .iter()
            .any(|entry| entry["path"] == json!(path) && entry["original_sha256"].is_string()));
    }
    for path in [&completed.2, &completed.3, &abandoned.3] {
        assert!(sources
            .iter()
            .any(|entry| entry["path"] == json!(path) && entry["original_sha256"].is_null()));
        assert!(!path.exists());
    }
}

#[test]
fn archive_refuses_unproven_host_cleanup_and_still_checks_other_sources() {
    use dispatch::host_attempt_state::HostAttemptState;

    for case in [
        "started",
        "duplicate-status",
        "unbound-log",
        "missing-log",
        "claim",
        "wrong-run",
        "wrong-path",
        "bad-manifest",
        "missing-receipt",
        "present-corrupt-spec",
        "registered-missing-spec",
    ] {
        let fixture = Fixture::new();
        let (manifest, log, snapshot, spec, receipt) = host_archive_attempt(
            &fixture,
            "host_1_1",
            if case == "started" {
                "started"
            } else {
                "completed"
            },
        );
        match case {
            "started" => {}
            "duplicate-status" | "unbound-log" => {
                let contents = fs::read_to_string(&log).unwrap();
                let replacement = if case == "duplicate-status" {
                    contents.replace(
                        "terminal_state: completed",
                        "terminal_state: completed\nterminal_state: started",
                    )
                } else {
                    contents.replace("attempt_id: host_1_1", "attempt_id: host_1_2")
                };
                fs::write(log, replacement).unwrap();
            }
            "missing-log" => fs::remove_file(log).unwrap(),
            "claim" => fs::create_dir(manifest.with_extension("close-claim")).unwrap(),
            "wrong-run" | "wrong-path" => {
                let mut state = HostAttemptState::load(&manifest, "host_1_1").unwrap();
                if case == "wrong-run" {
                    state.run_id = "other-run".into();
                } else {
                    state.snapshot_path = snapshot.with_file_name("other.prompt.bak");
                }
                fs::write(manifest, state.encode_immutable().unwrap()).unwrap();
            }
            "bad-manifest" => {
                let mut envelope: Value =
                    serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
                envelope["sha256"] = json!(digest_bytes(b"wrong"));
                fs::write(manifest, serde_json::to_vec(&envelope).unwrap()).unwrap();
            }
            "missing-receipt" => fs::remove_file(receipt).unwrap(),
            "present-corrupt-spec" => fs::write(spec, "changed spec").unwrap(),
            "registered-missing-spec" => fixture.publish("missing-spec", json!({"spec_path":spec})),
            _ => unreachable!(),
        }
        let result = fixture.archive("worktree-remove");
        assert!(!result.status.success(), "case {case} should refuse");
    }
}

#[test]
fn archived_sources_redact_credentials_headers_and_known_values() {
    let fixture = Fixture::new();
    let raw = "OPENAI_API_KEY=sk-private-credential\nAuthorization: Bearer short-header-value\npassword=private-password\nknown plain-secret-value elsewhere\n";
    fs::write(
        fixture
            .repo
            .path()
            .join(".dev/pipeline/archive/task-spec.md"),
        raw,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .args([
            "finalize-check",
            ".dev/plans/archive.prompt.md",
            "--archive",
            "--run",
            "archive-run",
            "--source-worktree",
            fixture.repo.path().to_str().unwrap(),
            "--boundary",
            "worktree-remove",
        ])
        .env("GAL_ARCHIVE_KNOWN_SECRET", "plain-secret-value")
        .current_dir(fixture.repo.path())
        .output()
        .unwrap();
    let id = field(&output, "archive_proof_id");
    let proof_text = fs::read_to_string(fixture.proof_path(&id)).unwrap();
    for secret in [
        "sk-private-credential",
        "short-header-value",
        "private-password",
        "plain-secret-value",
    ] {
        assert!(!proof_text.contains(secret));
    }
    let proof = fixture.proof(&id);
    assert_eq!(proof["redaction_rule_version"], "archive-redaction-v1");
    let source = proof["archive_set"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["path"].as_str().unwrap().ends_with("task-spec.md"))
        .unwrap();
    assert_eq!(source["original_sha256"], digest_bytes(raw.as_bytes()));
    assert_eq!(source["original_length"], raw.len());
    assert!(source["content"].as_str().unwrap().contains("<redacted>"));
    assert!(fixture
        .verify(
            &id,
            &field(&output, "source_set_id"),
            "archive-run",
            "worktree-remove"
        )
        .status
        .success());
}

#[test]
fn closed_sources_refuse_usage_operations_and_evidence_publication() {
    let fixture = Fixture::new();
    let output = fixture.archive("worktree-remove");
    field(&output, "archive_proof_id");
    let result = fixture.call(&[
        "usage",
        "begin",
        "--phase",
        "planning",
        "--invocation",
        "closed",
        "--worktree",
        fixture.repo.path().to_str().unwrap(),
        "--host",
        "unknown",
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Closed"));
    let result = fixture.call(&["finalize-check", ".dev/plans/archive.prompt.md"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Closed"));
    assert!(!fixture
        .repo
        .path()
        .join(".dev/pipeline/archive/finalize-check.receipt.md")
        .exists());
    let record = EvidenceRecord {
        schema_version: 1,
        event_id: "late".into(),
        operation_id: "op-0123456789abcdef".into(),
        run_id: "archive-run".into(),
        plan_generation: None,
        task_id: None,
        attempt_id: None,
        parent_attempt_id: None,
        phase: "test".into(),
        source_kind: "test".into(),
        source_version: "1".into(),
        recorded_at_unix_ms: 1,
        content_hash: digest_bytes(b"null"),
        content: Value::Null,
    };
    assert!(matches!(
        EvidenceStore::open(&fixture.repo.path().join(".git"))
            .unwrap()
            .publish(&record),
        Err(pipeline::usage::StoreError::Closed(_))
    ));
    assert!(fixture
        .verify(
            &field(&output, "archive_proof_id"),
            &field(&output, "source_set_id"),
            "archive-run",
            "worktree-remove"
        )
        .status
        .success());
}

#[test]
fn rearchiving_replaces_the_closure_generation_and_revokes_old_proofs() {
    let fixture = Fixture::new();
    let first = fixture.archive("worktree-remove");
    let first_id = field(&first, "archive_proof_id");
    let first_set = field(&first, "source_set_id");
    assert!(fixture
        .verify(&first_id, &first_set, "archive-run", "worktree-remove")
        .status
        .success());
    fs::remove_file(
        fixture
            .repo
            .path()
            .join(".git/gal/usage/v1/archive-run/.closed"),
    )
    .unwrap();
    refused(fixture.verify(&first_id, &first_set, "archive-run", "worktree-remove"));
    let second = fixture.archive("worktree-remove");
    let second_id = field(&second, "archive_proof_id");
    assert_ne!(first_id, second_id);
    refused(fixture.verify(&first_id, &first_set, "archive-run", "worktree-remove"));
    assert!(fixture
        .verify(
            &second_id,
            &field(&second, "source_set_id"),
            "archive-run",
            "worktree-remove"
        )
        .status
        .success());
}

#[test]
fn active_store_lock_and_prompt_lease_refuse_archiving() {
    use fs2::FileExt;
    let fixture = Fixture::new();
    let directory = fixture.repo.path().join(".git/gal/usage/v1/archive-run");
    fs::create_dir_all(&directory).unwrap();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join(".lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    refused(fixture.archive("worktree-remove"));
    drop(lock);
    let operation = fixture
        .repo
        .path()
        .join(".git/gal/evidence/v1/.operation-lock");
    fs::create_dir_all(operation.parent().unwrap()).unwrap();
    let operation_lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(operation)
        .unwrap();
    operation_lock.lock_exclusive().unwrap();
    refused(fixture.archive("worktree-remove"));
    drop(operation_lock);
    let directory = fixture
        .repo
        .path()
        .join(".dev/pipeline/.locks/prompt-writers");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("writer.lock"),
        format!(
            "owner_token=writer\nprompt={}\n",
            fixture
                .repo
                .path()
                .join(".dev/plans/archive.prompt.md")
                .display()
        ),
    )
    .unwrap();
    refused(fixture.archive("worktree-remove"));
}

#[test]
fn verification_refuses_writers_that_appear_after_archiving() {
    use fs2::FileExt;

    for writer in ["usage", "evidence", "operation", "coordinator", "prompt"] {
        let fixture = Fixture::new();
        let output = fixture.archive("worktree-remove");
        let id = field(&output, "archive_proof_id");
        let set = field(&output, "source_set_id");
        let lock_path = match writer {
            "usage" => Some(".git/gal/usage/v1/archive-run/.lock"),
            "evidence" => Some(".git/gal/evidence/v1/archive-run/.lock"),
            "operation" => Some(".git/gal/evidence/v1/.operation-lock"),
            _ => None,
        };
        let _writer_lock = lock_path.map(|path| {
            let file = fs::OpenOptions::new()
                .write(true)
                .open(fixture.repo.path().join(path))
                .unwrap();
            file.lock_exclusive().unwrap();
            file
        });
        match writer {
            "coordinator" => {
                fs::write(
                    fixture.repo.path().join(".dev/pipeline/archive/coordinator.json"),
                    r#"{"accounting_run":{"run_id":"archive-run"},"active_attempt":{"attempt_id":"busy"}}"#,
                )
                .unwrap();
            }
            "prompt" => {
                let directory = fixture
                    .repo
                    .path()
                    .join(".dev/pipeline/.locks/prompt-writers");
                fs::create_dir_all(&directory).unwrap();
                fs::write(
                    directory.join("writer.lock"),
                    format!(
                        "owner_token=writer\nprompt={}\n",
                        fixture
                            .repo
                            .path()
                            .join(".dev/plans/archive.prompt.md")
                            .display()
                    ),
                )
                .unwrap();
            }
            _ => {}
        }
        refused(fixture.verify(&id, &set, "archive-run", "worktree-remove"));
        assert!(fixture
            .repo
            .path()
            .join(".dev/plans/archive.prompt.md")
            .is_file());
        assert!(!fixture
            .repo
            .path()
            .join(".git/gal/archive/v1/verified")
            .join(format!("{id}.json"))
            .exists());
    }
}

#[test]
fn runtime_attempt_manifest_is_distinct_from_store_manifest() {
    let fixture = Fixture::new();
    let spec = fixture
        .repo
        .path()
        .join(".dev/pipeline/archive/input.spec.md");
    fs::write(&spec, "input task spec").unwrap();
    fs::write(fixture.repo.path().join(".dev/pipeline/archive/attempt.manifest.json"), json!({"attempt_id":"attempt-one","spec_path":spec,"spec_sha256":digest_bytes(b"input task spec")}).to_string()).unwrap();
    let content = json!({"plan_id":"archive"});
    EvidenceStore::open(&fixture.repo.path().join(".git"))
        .unwrap()
        .publish(&EvidenceRecord {
            schema_version: 1,
            event_id: "run".into(),
            operation_id: "archive-run".into(),
            run_id: "archive-run".into(),
            plan_generation: None,
            task_id: None,
            attempt_id: None,
            parent_attempt_id: None,
            phase: "run".into(),
            source_kind: "run".into(),
            source_version: "1".into(),
            recorded_at_unix_ms: 1,
            content_hash: digest_bytes(&serde_json::to_vec(&content).unwrap()),
            content,
        })
        .unwrap();
    let output = fixture.archive("worktree-remove");
    assert!(fixture
        .verify(
            &field(&output, "archive_proof_id"),
            &field(&output, "source_set_id"),
            "archive-run",
            "worktree-remove"
        )
        .status
        .success());
}

#[test]
fn json_sources_keep_structure_and_old_receipt_versions_with_redaction() {
    let fixture = Fixture::new();
    let receipt = fixture
        .repo
        .path()
        .join(".dev/pipeline/archive/finalize-check.receipt.md");
    fs::write(&receipt, "new receipt").unwrap();
    fixture.publish("old-receipt", json!({"receipt_path":receipt,"receipt_text":"original receipt", "receipt_sha256":digest_bytes(b"original receipt"),"headers":{"Authorization":"Bearer old-header"},"raw_bytes":b"password=byte-secret".to_vec(),"command":["tool","--api-key","argument-secret"],"diagnostics":{"exit_code":1,"stderr":"original diagnostic"}}));
    let output = fixture.archive("worktree-remove");
    let proof = fixture.proof(&field(&output, "archive_proof_id"));
    let source = proof["archive_set"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| {
            source["path"]
                .as_str()
                .unwrap()
                .ends_with("old-receipt.record.json")
        })
        .unwrap();
    let archived: Value = serde_json::from_str(source["content"].as_str().unwrap()).unwrap();
    assert_eq!(archived["content"]["receipt_text"], "original receipt");
    assert_eq!(
        archived["content"]["headers"]["Authorization"],
        "<redacted>"
    );
    assert_eq!(
        archived["content"]["diagnostics"]["stderr"],
        "original diagnostic"
    );
    assert_eq!(archived["content"]["diagnostics"]["exit_code"], 1);
    let raw: Vec<u8> = serde_json::from_value(archived["content"]["raw_bytes"].clone()).unwrap();
    assert_eq!(String::from_utf8(raw).unwrap(), "password=<redacted>");
    assert_eq!(archived["content"]["command"][2], "<redacted>");
}

#[test]
fn main_cleanup_consumes_verified_proof_and_keeps_terminal_closeout() {
    let fixture = Fixture::new();
    let id = fixture.main_proof();
    fixture.remove_plans();
    let output = fixture.clean(&id);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let terminal = fixture.terminal(&output, "completed");
    assert_eq!(terminal["cleanup_result"], "removed");
    let closeout: Value = serde_json::from_slice(
        &fs::read(
            fixture
                .repo
                .path()
                .join(".git/gal/archive/v1/closeout")
                .join(format!("{id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(closeout["cleanup_result"], "removed");
    assert_eq!(closeout["terminal_operation_id"], terminal["operation_id"]);
    assert!(!fixture.repo.path().join(".dev/pipeline/archive").exists());
    assert_eq!(fixture.proof(&id)["prior_proof"], Value::Null);
    assert!(fixture.proof(&id)["archive_set"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["path"]
            .as_str()
            .unwrap()
            .ends_with("archive.prompt.md")
            && source["content"]
                .as_str()
                .unwrap()
                .contains("requirements and criteria")));
}

#[test]
fn cleanup_refusals_terminate_operations_and_keep_runtime_sources() {
    use fs2::FileExt;
    for change in [
        "added",
        "runtime-lock",
        "written",
        "missing",
        "unverified",
        "old-boundary",
        "usage-lock",
        "evidence-lock",
        "operation-lock",
        "coordinator",
        "prompt",
        "reopened",
        "active-plan",
        "active-research",
        "other-run",
        "corrupt",
        "closeout-write",
    ] {
        let fixture = Fixture::new();
        let archived = fixture.archive(if change == "old-boundary" {
            "worktree-remove"
        } else {
            "pipeline-clean"
        });
        let id = field(&archived, "archive_proof_id");
        if change != "unverified" {
            field(
                &fixture.verify(
                    &id,
                    &field(&archived, "source_set_id"),
                    "archive-run",
                    if change == "old-boundary" {
                        "worktree-remove"
                    } else {
                        "pipeline-clean"
                    },
                ),
                "terminal_operation_id",
            );
        }
        fixture.remove_plans();
        let runtime = fixture.repo.path().join(".dev/pipeline/archive");
        let lock_path = match change {
            "usage-lock" => Some(".git/gal/usage/v1/archive-run/.lock"),
            "evidence-lock" => Some(".git/gal/evidence/v1/archive-run/.lock"),
            "operation-lock" => Some(".git/gal/evidence/v1/.operation-lock"),
            _ => None,
        };
        let _writer = lock_path.map(|path| {
            let file = fs::OpenOptions::new()
                .write(true)
                .open(fixture.repo.path().join(path))
                .unwrap();
            file.lock_exclusive().unwrap();
            file
        });
        match change {
            "added" => fs::write(runtime.join("unarchived.txt"), "new source").unwrap(),
            "runtime-lock" => fs::write(runtime.join(".lock"), "unarchived lock").unwrap(),
            "written" => fs::write(fixture.repo.path().join(".dev/mistake-register.md"), "changed").unwrap(),
            "missing" => fs::remove_file(runtime.join("preflight.receipt.md")).unwrap(),
            "coordinator" => fs::write(runtime.join("coordinator.json"), r#"{"accounting_run":{"run_id":"archive-run"},"active_attempt":{"attempt_id":"busy"}}"#).unwrap(),
            "prompt" => {
                let directory = fixture.repo.path().join(".dev/pipeline/.locks/prompt-writers");
                fs::create_dir_all(&directory).unwrap();
                fs::write(directory.join("writer.lock"), format!("prompt={}\n", fixture.repo.path().join(".dev/plans/archive.prompt.md").display())).unwrap();
            }
            "reopened" => fs::remove_file(fixture.repo.path().join(".git/gal/usage/v1/archive-run/.closed")).unwrap(),
            "active-plan" => fs::write(fixture.repo.path().join(".dev/plans/archive.md"), "# Active").unwrap(),
            "active-research" => {
                fs::create_dir_all(fixture.repo.path().join(".dev/research")).unwrap();
                fs::write(fixture.repo.path().join(".dev/research/archive.md"), "# Active").unwrap();
            }
            "other-run" => fs::write(runtime.join("preflight.receipt.md"), "run_id: another-run\n").unwrap(),
            "corrupt" => fs::write(fixture.proof_path(&id), "{}").unwrap(),
            "closeout-write" => {
                let path = fixture.repo.path().join(".git/gal/archive/v1/closeout").join(format!("{id}.json"));
                fs::remove_file(&path).unwrap();
                fs::create_dir(path).unwrap();
            }
            _ => {}
        }
        let output = fixture.clean(&id);
        let terminal = fixture.terminal(&output, "refused");
        assert_eq!(terminal["cleanup_result"], "refused", "{change}");
        refused(output);
        assert!(runtime.is_dir(), "{change}");
        if change == "added" {
            let closeout: Value = serde_json::from_slice(
                &fs::read(
                    fixture
                        .repo
                        .path()
                        .join(".git/gal/archive/v1/closeout")
                        .join(format!("{id}.json")),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(closeout["cleanup_result"], "refused");
            assert_eq!(closeout["terminal_operation_id"], terminal["operation_id"]);
        }
    }
}

#[test]
fn cleanup_requires_both_flags_and_refuses_unreadable_associations() {
    let fixture = Fixture::new();
    fixture.remove_plans();
    for args in [
        vec!["--run", "archive-run"],
        vec!["--archive-proof", "proof"],
    ] {
        let mut command = vec!["pipeline-clean", ".dev/plans/archive.prompt.md"];
        command.extend(args);
        let output = fixture.call(&command);
        assert_eq!(output.status.code(), Some(64));
    }
    fs::create_dir_all(fixture.repo.path().join(".git/gal/evidence")).unwrap();
    fs::write(
        fixture.repo.path().join(".git/gal/evidence/v1"),
        "unreadable store",
    )
    .unwrap();
    refused(fixture.call(&["pipeline-clean", ".dev/plans/archive.prompt.md"]));
    assert!(fixture.repo.path().join(".dev/pipeline/archive").is_dir());
    assert!(!fixture
        .repo
        .path()
        .join(".git/gal/archive/v1/operations")
        .exists());
}

#[test]
fn main_archive_write_failure_preserves_plan_and_runtime_sources() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.repo.path().join(".git/gal/archive/v1")).unwrap();
    fs::write(
        fixture.repo.path().join(".git/gal/archive/v1/proofs"),
        "blocked",
    )
    .unwrap();
    refused(fixture.archive("pipeline-clean"));
    let output = fixture.clean(&"0".repeat(64));
    fixture.terminal(&output, "refused");
    refused(output);
    assert!(fixture
        .repo
        .path()
        .join(".dev/plans/archive.prompt.md")
        .is_file());
    assert!(fixture.repo.path().join(".dev/plans/archive.md").is_file());
    assert!(fixture
        .repo
        .path()
        .join(".dev/pipeline/archive/preflight.receipt.md")
        .is_file());
}

#[test]
fn worktree_then_main_cleanup_preserves_evidence_and_usage_identities() {
    use pipeline::usage::{ObservationRecord, SnapshotEvidence, UsageStore};
    let fixture = Fixture::new();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(fixture.repo.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "--allow-empty",
        "-qm",
        "initial",
    ]);
    let worktree = fixture.external.path().join("worktree");
    git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "archive-feature",
        worktree.to_str().unwrap(),
    ]);
    fs::create_dir_all(worktree.join(".dev/plans")).unwrap();
    fs::create_dir_all(worktree.join(".dev/pipeline/archive")).unwrap();
    for path in [
        ".dev/plans/archive.md",
        ".dev/plans/archive.prompt.md",
        ".dev/pipeline/archive/preflight.receipt.md",
        ".dev/mistake-register.md",
    ] {
        fs::copy(fixture.repo.path().join(path), worktree.join(path)).unwrap();
    }
    fixture.publish(
        "preserved-event",
        json!({"diagnostic":"failure and correction evidence"}),
    );
    let usage = UsageStore::open(&fixture.repo.path().join(".git")).unwrap();
    let observation = ObservationRecord {
        observation_id: "observation-original".into(),
        observation: serde_json::from_value(json!({})).unwrap(),
    };
    let evidence = SnapshotEvidence {
        schema_version: 1,
        snapshot_id: "snapshot-original".into(),
        run_id: "archive-run".into(),
        cutoff_unix_ms: 1,
        observations: vec![observation],
        intervals: vec![],
    };
    usage.publish(&evidence).unwrap();
    let before = usage
        .list_page::<SnapshotEvidence>("archive-run", None, 10)
        .unwrap()
        .entries;
    let event_before = EvidenceStore::open(&fixture.repo.path().join(".git"))
        .unwrap()
        .read("archive-run", "preserved-event")
        .unwrap();
    let call = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .args(args)
            .current_dir(&worktree)
            .output()
            .unwrap()
    };
    let archived = call(&[
        "finalize-check",
        ".dev/plans/archive.prompt.md",
        "--archive",
        "--run",
        "archive-run",
        "--source-worktree",
        worktree.to_str().unwrap(),
        "--boundary",
        "worktree-remove",
    ]);
    let prior = field(&archived, "archive_proof_id");
    field(
        &call(&[
            "finalize-check",
            "--verify-archive",
            "--archive-proof",
            &prior,
            "--run",
            "archive-run",
            "--source-worktree",
            worktree.to_str().unwrap(),
            "--source-set",
            &field(&archived, "source_set_id"),
            "--boundary",
            "worktree-remove",
        ]),
        "terminal_operation_id",
    );
    git(&["worktree", "remove", "--force", worktree.to_str().unwrap()]);
    assert!(!worktree.exists());
    let archived = fixture.call(&[
        "finalize-check",
        ".dev/plans/archive.prompt.md",
        "--archive",
        "--run",
        "archive-run",
        "--source-worktree",
        fixture.repo.path().to_str().unwrap(),
        "--boundary",
        "pipeline-clean",
        "--prior-proof",
        &prior,
    ]);
    let id = field(&archived, "archive_proof_id");
    field(
        &fixture.verify(
            &id,
            &field(&archived, "source_set_id"),
            "archive-run",
            "pipeline-clean",
        ),
        "terminal_operation_id",
    );
    assert_eq!(fixture.proof(&id)["prior_proof"], prior);
    fixture.remove_plans();
    let cleaned = fixture.clean(&id);
    field(&cleaned, "terminal_operation_id");
    fixture.terminal(&cleaned, "completed");
    assert_eq!(
        usage
            .list_page::<SnapshotEvidence>("archive-run", None, 10)
            .unwrap()
            .entries,
        before
    );
    assert_eq!(
        usage
            .read::<SnapshotEvidence>("archive-run", "snapshot-original")
            .unwrap(),
        evidence
    );
    let event_after = EvidenceStore::open(&fixture.repo.path().join(".git"))
        .unwrap()
        .read("archive-run", "preserved-event")
        .unwrap();
    assert_eq!(event_after.event_id, event_before.event_id);
    assert_eq!(event_after.content_hash, event_before.content_hash);
    assert!(fixture.proof(&prior)["archive_set"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["content"]
            .as_str()
            .is_some_and(|content| content.contains("preserved-event"))));
}

#[test]
fn cleanup_proof_refuses_a_linked_runtime_directory() {
    use gal_foundation::platform::{create_dir_link, remove_dir_link};
    let fixture = Fixture::new();
    let id = fixture.main_proof();
    fixture.remove_plans();
    let runtime = fixture
        .repo
        .path()
        .join(".dev")
        .join("pipeline")
        .join("archive");
    let saved = fixture.external.path().join("saved-runtime");
    fs::rename(&runtime, &saved).unwrap();
    create_dir_link(&saved, &runtime).unwrap();
    let output = fixture.clean(&id);
    fixture.terminal(&output, "refused");
    refused(output);
    assert!(saved.join("preflight.receipt.md").is_file());
    remove_dir_link(&runtime).unwrap();
}

#[test]
fn cleanup_accepts_proof_flags_in_either_order() {
    let fixture = Fixture::new();
    let id = fixture.main_proof();
    fixture.remove_plans();
    let output = fixture.call(&[
        "pipeline-clean",
        ".dev/plans/archive.prompt.md",
        "--archive-proof",
        &id,
        "--run",
        "archive-run",
    ]);
    field(&output, "terminal_operation_id");
    fixture.terminal(&output, "completed");
}

#[test]
fn refused_cleanup_revokes_changed_sources_even_if_original_bytes_are_restored() {
    let fixture = Fixture::new();
    let id = fixture.main_proof();
    fixture.remove_plans();
    let path = fixture.repo.path().join(".dev/mistake-register.md");
    let original = fs::read(&path).unwrap();
    fs::write(&path, "changed source").unwrap();
    refused(fixture.clean(&id));
    fs::write(path, original).unwrap();
    let output = fixture.clean(&id);
    fixture.terminal(&output, "refused");
    assert!(String::from_utf8_lossy(&output.stderr).contains("archive proof revoked"));
    refused(output);
    assert!(fixture.repo.path().join(".dev/pipeline/archive").is_dir());
}

#[cfg(windows)]
#[test]
fn filesystem_cleanup_failure_is_recorded_and_keeps_the_locked_source() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let id = fixture.main_proof();
    fixture.remove_plans();
    let receipt = fixture
        .repo
        .path()
        .join(".dev/pipeline/archive/preflight.receipt.md");
    let _reader = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&receipt)
        .unwrap();
    let output = fixture.clean(&id);
    fixture.terminal(&output, "refused");
    refused(output);
    assert!(receipt.is_file());
    let closeout: Value = serde_json::from_slice(
        &fs::read(
            fixture
                .repo
                .path()
                .join(".git/gal/archive/v1/closeout")
                .join(format!("{id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(closeout["cleanup_result"], "refused");
}

#[test]
fn finalize_contract_and_workflow_locales_cover_both_deletion_boundaries() {
    let contract =
        include_str!("../../../plugins/gal-core/commands/gal-finalize/SKILL.template.md");
    let main = contract
        .split_once("### Sequence 5")
        .map(|(_, main)| main)
        .unwrap()
        .split("### Sequence 6")
        .next()
        .unwrap();
    let archive = main.find("--archive --run <run-id>").unwrap();
    let verify = main
        .find("--verify-archive --archive-proof <main-proof-id>")
        .unwrap();
    let cleanup = main.find("pipeline-clean .dev/plans/<slug>.prompt.md --run <run-id> --archive-proof <main-proof-id>").unwrap();
    assert!(archive < verify && verify < cleanup);
    assert!(main.contains("--prior-proof <worktree-proof-id>"));
    assert!(main.contains("no `run_id:` line"));
    assert!(main.contains("two-argument `gal pipeline-clean .dev/plans/<slug>.prompt.md`"));
    assert!(main.contains("manual cleanup to the owner"));
    for doc in [
        include_str!("../../../docs/workflows.md"),
        include_str!("../../../docs/i18n/zh-Hant/workflows.zh-Hant.md"),
        include_str!("../../../docs/i18n/ja/workflows.ja.md"),
    ] {
        for required in [
            "--boundary worktree-remove",
            "--boundary pipeline-clean",
            "--verify-archive",
            "--archive-proof <main-proof-id>",
            "gal usage correction --input <json-path>",
            "unavailable",
            "terminal_operation_id:",
            "run_id:",
        ] {
            assert!(doc.contains(required), "missing {required}");
        }
    }
}
