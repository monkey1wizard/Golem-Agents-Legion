use pipeline::report::{EvidenceRecord, EvidenceStore};

#[test]
fn receipt_verifier_calls_are_confined_to_collection_and_self_verification() {
    fn test_module_end(source: &str, start: usize) -> usize {
        let bytes = source.as_bytes();
        let mut i = start + source[start..].find('{').unwrap();
        let mut depth = 0usize;
        while i < bytes.len() {
            if bytes[i..].starts_with(b"//") {
                i += bytes[i..]
                    .iter()
                    .position(|b| *b == b'\n')
                    .unwrap_or(bytes.len() - i);
            } else if bytes[i..].starts_with(b"/*") {
                i += 2;
                let mut comments = 1;
                while i < bytes.len() && comments > 0 {
                    if bytes[i..].starts_with(b"/*") {
                        comments += 1;
                        i += 2;
                    } else if bytes[i..].starts_with(b"*/") {
                        comments -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
            } else if bytes[i] == b'r' && bytes.get(i + 1).is_some_and(|b| *b == b'"' || *b == b'#')
            {
                let mut quote = i + 1;
                while bytes.get(quote) == Some(&b'#') {
                    quote += 1;
                }
                if bytes.get(quote) != Some(&b'"') {
                    i += 1;
                    continue;
                }
                let hashes = quote - i - 1;
                i = quote + 1;
                while i < bytes.len() {
                    if bytes[i] == b'"'
                        && bytes
                            .get(i + 1..i + 1 + hashes)
                            .is_some_and(|tail| tail.iter().all(|b| *b == b'#'))
                    {
                        i += 1 + hashes;
                        break;
                    }
                    i += 1;
                }
            } else if bytes[i] == b'"'
                || (bytes[i] == b'\''
                    && (bytes.get(i + 2) == Some(&b'\'')
                        || (bytes.get(i + 1) == Some(&b'\\') && bytes.get(i + 3) == Some(&b'\''))))
            {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else if bytes[i] == quote {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
            } else {
                if bytes[i] == b'{' {
                    depth += 1;
                }
                if bytes[i] == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1;
                    }
                }
                i += 1;
            }
        }
        panic!("unterminated test module")
    }
    fn scan(dir: &std::path::Path, needle: &str) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                scan(&path, needle);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let source = std::fs::read_to_string(&path)
                    .unwrap()
                    .replace("\r\n", "\n");
                let test_module = source
                    .find("#[cfg(test)]\nmod tests {")
                    .map(|start| (start, test_module_end(&source, start)));
                for (offset, _) in source.match_indices(needle) {
                    if test_module.is_some_and(|(start, end)| (start..end).contains(&offset)) {
                        continue;
                    }
                    let prefix = &source[..offset];
                    let function = prefix
                        .rsplit_once("fn ")
                        .map(|(_, name)| name)
                        .unwrap_or("");
                    let function = function
                        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                        .next()
                        .unwrap_or("");
                    let file = path.file_name().unwrap().to_string_lossy();
                    let declaration =
                        prefix.trim_end().ends_with("fn") && file == "finalize_check.rs";
                    let self_check = file == "finalize_check.rs" && function == "bind_receipt";
                    let collector = file == "report_capture.rs" && function == "verify";
                    assert!(
                        declaration || self_check || collector,
                        "receipt verifier bypass in {} near byte {offset}",
                        path.display()
                    );
                }
            }
        }
    }
    let needle = ["verify_receipt", "_envelope"].concat();
    scan(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &needle,
    );
    let bypass = tempfile::tempdir().unwrap();
    std::fs::write(
        bypass.path().join("bypass.rs"),
        format!("fn bypass() {{ {needle}(); }}"),
    )
    .unwrap();
    assert!(std::panic::catch_unwind(|| scan(bypass.path(), &needle)).is_err());
    std::fs::write(bypass.path().join("bypass.rs"), format!("#[cfg(test)]\nmod tests {{ fn case() {{ {needle}(); }} }}\nfn bypass() {{ {needle}(); }}")).unwrap();
    assert!(std::panic::catch_unwind(|| scan(bypass.path(), &needle)).is_err());
}
use serde_json::json;
use std::{
    fs,
    process::{Command, Output},
};

struct Fixture {
    repo: tempfile::TempDir,
}
impl Fixture {
    fn pipeline_prompt(&self) -> String {
        let prompt = ".dev/plans/capture.prompt.md";
        fs::create_dir_all(self.repo.path().join(".dev/plans")).unwrap();
        let task = format!("{}-{:02}", "T", 1);
        fs::write(self.repo.path().join(prompt), format!(
            "# Capture\n\n## Tasks\n\n- [ ] {task} capture\n  - **Verification Impact**: {{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false}}\n  - **Files**: `src/lib.rs`\n\n## Verification Map\n\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n| {task} | unit | cargo test | pass | TESTER | test output |\n\n## Test Plan\n\n| ID | Description |\n| --- | --- |\n| capture | capture |\n\n## Status\n\nWorkflow: IMPLEMENT\n"
        )).unwrap();
        fs::write(
            self.repo.path().join(".dev/state.md"),
            format!("| capture | {prompt} |\n"),
        )
        .unwrap();
        prompt.into()
    }

    fn evidence(&self) -> Vec<EvidenceRecord> {
        let store = self.store();
        let mut records = Vec::new();
        for directory in fs::read_dir(self.repo.path().join(".git/gal/evidence/v1")).unwrap() {
            let directory = directory.unwrap();
            if !directory.file_type().unwrap().is_dir() {
                continue;
            }
            let run = directory.file_name().to_string_lossy().into_owned();
            let mut after = None;
            loop {
                let page = store.list_page(&run, after.as_deref(), 256).unwrap();
                for item in page.entries {
                    records.push(store.read(&run, &item.record_id).unwrap());
                }
                after = page.next_after;
                if after.is_none() {
                    break;
                }
            }
        }
        records
    }

    fn new() -> Self {
        let repo = tempfile::tempdir().unwrap();
        assert!(Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());
        Self { repo }
    }
    fn call(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .args(args)
            .current_dir(self.repo.path())
            .output()
            .unwrap()
    }
    fn begin(&self, invocation: &str) -> Output {
        self.call(&[
            "usage",
            "begin",
            "--phase",
            "planning",
            "--invocation",
            invocation,
            "--worktree",
            self.repo.path().to_str().unwrap(),
            "--host",
            "unknown",
        ])
    }
    fn store(&self) -> EvidenceStore {
        EvidenceStore::open(&self.repo.path().join(".git")).unwrap()
    }
}

#[test]
fn preflight_reuses_run_and_operation_after_refusal_overwrites_receipt() {
    let fixture = Fixture::new();
    let prompt = fixture.pipeline_prompt();
    let first = fixture.call(&["pipeline-preflight", &prompt]);
    check(&first);
    let operation = field(&first, "operation_id");
    let receipt = fixture
        .repo
        .path()
        .join(".dev/pipeline/capture/preflight.receipt.md");
    let initial = fs::read_to_string(&receipt).unwrap();
    let run = initial
        .lines()
        .find_map(|line| line.strip_prefix("run_id: "))
        .unwrap()
        .to_owned();
    let existing = fixture.store().read(&run, "run").unwrap();
    assert_eq!(
        existing.content["design_roster_version"],
        serde_json::Value::Null
    );
    assert_eq!(existing.content["design_roster_reason"], "roster-absent");
    let mut cost = existing.clone();
    cost.event_id = "existing-attempt-cost".into();
    cost.source_kind = "attempt".into();
    cost.content = json!({"cost":"1.25","membership":"frozen","attempt_id":"previous"});
    cost.content_hash = pipeline::usage::digest_bytes(&serde_json::to_vec(&cost.content).unwrap());
    fixture.store().publish(&cost).unwrap();
    fs::write(fixture.repo.path().join(&prompt), "# Invalid prompt\n").unwrap();
    for _ in 0..2 {
        let refusal = fixture.call(&["pipeline-preflight", &prompt]);
        assert!(!refusal.status.success());
        assert_eq!(field(&refusal, "operation_id"), operation);
        assert!(fs::read_to_string(&receipt)
            .unwrap()
            .contains(&format!("run_id: {run}")));
    }
    assert_eq!(
        fixture.store().read(&run, "existing-attempt-cost").unwrap(),
        cost
    );
    assert_eq!(fixture.store().read(&run, "run").unwrap(), existing);
    let refusals: Vec<_> = fixture
        .evidence()
        .into_iter()
        .filter(|record| {
            record.source_kind == "transition" && record.content["detail"]["outcome"] == "refused"
        })
        .collect();
    assert_eq!(refusals.len(), 2);
    assert!(refusals.iter().all(|record| record.run_id == run));
}

#[test]
fn first_preflight_refusal_uses_entry_and_conflicting_run_stays_unknown() {
    let fixture = Fixture::new();
    let prompt = fixture.pipeline_prompt();
    fs::write(fixture.repo.path().join(&prompt), "# Invalid\n").unwrap();
    assert!(!fixture
        .call(&["pipeline-preflight", &prompt])
        .status
        .success());
    let records = fixture.evidence();
    assert!(!records.iter().any(|record| record.source_kind == "run"));
    let transition = records
        .iter()
        .find(|record| record.source_kind == "transition")
        .unwrap();
    assert!(transition.run_id.starts_with("entry-"));
    assert_eq!(transition.content["run_association"], "no-bound-run");
    for invariant in transition.content["invariants"]
        .as_object()
        .unwrap()
        .values()
    {
        assert_eq!(
            invariant,
            &json!({"status":"not-evaluated","reason":"no-coordinator"})
        );
    }
    fixture.pipeline_prompt();
    check(&fixture.call(&["pipeline-preflight", &prompt]));
    let receipt = fixture
        .repo
        .path()
        .join(".dev/pipeline/capture/preflight.receipt.md");
    fs::write(&receipt, "run_id: conflicting-run\n").unwrap();
    let before = fixture
        .evidence()
        .iter()
        .filter(|record| record.source_kind == "run")
        .count();
    let result = fixture.call(&["pipeline-preflight", &prompt]);
    check(&result);
    assert!(!String::from_utf8_lossy(&result.stdout).contains("operation_id:"));
    assert!(fs::read_to_string(&receipt)
        .unwrap()
        .contains("run-association-unknown"));
    let after = fixture.evidence();
    assert_eq!(
        after
            .iter()
            .filter(|record| record.source_kind == "run")
            .count(),
        before
    );
    assert!(after.iter().any(|record| record.source_kind == "transition"
        && record.content["run_association"] == "run-association-unknown"));
}

#[test]
fn handback_observation_does_not_terminate_pipeline_and_usage_end_does() {
    let fixture = Fixture::new();
    let prompt = fixture.pipeline_prompt();
    let preflight = fixture.call(&["pipeline-preflight", &prompt]);
    check(&preflight);
    let operation = field(&preflight, "operation_id");
    fixture.call(&["pipeline-handback-check", &prompt]);
    let entry = format!("entry-{}", operation.strip_prefix("op-").unwrap());
    assert!(fixture
        .store()
        .read(&entry, &format!("{operation}-end"))
        .is_err());
    assert!(fixture
        .evidence()
        .iter()
        .any(|record| record.source_kind == "transition"
            && record.content["request"]["command"] == "pipeline-handback-check"));
    let end = fixture.call(&[
        "usage",
        "end",
        "--operation",
        &operation,
        "--status",
        "stopped",
        "--reason",
        "stop-at",
    ]);
    check(&end);
    assert_eq!(field(&end, "terminal_operation_id"), operation);
    assert_eq!(
        fixture
            .store()
            .read(&entry, &format!("{operation}-end"))
            .unwrap()
            .content["status"],
        "stopped"
    );
    let next = fixture.call(&["pipeline-preflight", &prompt]);
    check(&next);
    assert_ne!(field(&next, "operation_id"), operation);
}

#[test]
fn fresh_guarded_refusal_leaves_no_pipeline_or_operation_records() {
    for existing_scope in [None, Some("other"), Some("capture")] {
        let fixture = Fixture::new();
        let prompt = fixture.pipeline_prompt();
        let plugin = tempfile::tempdir().unwrap();
        if let Some(scope) = existing_scope {
            fs::create_dir_all(fixture.repo.path().join(".dev/pipeline").join(scope)).unwrap();
        }
        let refusal = Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(fixture.repo.path())
            .env("PLUGIN_DATA", plugin.path())
            .args(["pipeline", "--require-codex-stop-v1", &prompt])
            .output()
            .unwrap();
        assert!(!refusal.status.success());
        assert!(String::from_utf8_lossy(&refusal.stderr).contains("host-continuation-not-ready"));
        assert_eq!(
            fixture.repo.path().join(".dev/pipeline").exists(),
            existing_scope.is_some()
        );
        assert_eq!(
            fixture.repo.path().join(".dev/pipeline/capture").exists(),
            existing_scope == Some("capture")
        );
        assert!(!fixture.repo.path().join(".git/gal/evidence/v1").exists());
        assert_eq!(fs::read_dir(plugin.path()).unwrap().count(), 0);
    }
}

#[test]
fn driver_early_refusal_retains_run_and_terminates_its_operation() {
    let fixture = Fixture::new();
    let prompt = fixture.pipeline_prompt();
    let preflight = fixture.call(&["pipeline-preflight", &prompt]);
    check(&preflight);
    let operation = field(&preflight, "operation_id");
    let refusal = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(fixture.repo.path())
        .env_remove("PLUGIN_DATA")
        .args(["pipeline", "--require-codex-stop-v1", &prompt])
        .output()
        .unwrap();
    assert!(!refusal.status.success());
    assert_eq!(field(&refusal, "terminal_operation_id"), operation);
    let records = fixture.evidence();
    let transition = records
        .iter()
        .find(|record| {
            record.source_kind == "transition" && record.content["request"]["command"] == "pipeline"
        })
        .unwrap();
    assert_eq!(transition.content["run_association"], "bound");
    assert_eq!(transition.content["detail"]["outcome"], "refused");
    assert!(transition.content["before"].is_null());
    assert!(transition.content["after"].is_null());
    assert_eq!(
        transition.content["invariants"].as_object().unwrap().len(),
        5
    );
}

#[test]
fn host_hook_refusal_records_transition_without_changing_output() {
    use std::io::Write;
    use std::process::Stdio;
    let fixture = Fixture::new();
    let prompt = fixture.pipeline_prompt();
    check(&fixture.call(&["pipeline-preflight", &prompt]));
    let plugin = tempfile::tempdir().unwrap();
    let mut process = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(fixture.repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .args(["pipeline-host-hook", "pre-tool-use"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let event = json!({"cwd":fixture.repo.path(),"prompt":format!("gal pipeline {prompt}"),"session_id":"session","turn_id":"turn"});
    process
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&event).unwrap().as_bytes())
        .unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        json!({"continue":false,"decision":"deny","reason":"missing-generation"})
    );
    let records = fixture.evidence();
    let transition = records
        .iter()
        .find(|record| {
            record.source_kind == "transition"
                && record.content["request"]["command"] == "pipeline-host-hook"
        })
        .unwrap();
    assert_eq!(transition.content["detail"]["decision"], "deny");
    for invariant in transition.content["invariants"]
        .as_object()
        .unwrap()
        .values()
    {
        assert_eq!(invariant["status"], "not-evaluated");
        assert_eq!(invariant["reason"], "no-coordinator");
    }
}

#[test]
fn guarded_entry_records_allowed_transition_and_matching_terminal_operation() {
    let fixture = Fixture::new();
    let prompt = fixture.pipeline_prompt();
    for args in [
        vec!["config", "user.name", "test"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "commit.gpgsign", "false"],
        vec!["add", "."],
        vec!["commit", "-q", "-m", "fixture"],
    ] {
        assert!(Command::new("git")
            .current_dir(fixture.repo.path())
            .args(args)
            .status()
            .unwrap()
            .success());
    }
    let plugin = tempfile::tempdir().unwrap();
    let worktree = fs::canonicalize(fixture.repo.path()).unwrap();
    let hash = pipeline::usage::digest_bytes(&fs::read(fixture.repo.path().join(&prompt)).unwrap());
    let executable_hash =
        pipeline::usage::digest_bytes(&fs::read(env!("CARGO_BIN_EXE_gal")).unwrap());
    let expires = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 300;
    let binding = json!({"session_id":"session","turn_id":"turn","worktree":worktree,"prompt_hash":hash,"generation":1,"nonce":"nonce","handler_digest":"handler"});
    fs::write(plugin.path().join("ready-grant.json"), json!({"state":"ReadyGrant","expires":expires,"presented_turn_id":"turn","binding":binding}).to_string()).unwrap();
    let scope = fixture.repo.path().join(".dev/pipeline/capture");
    fs::create_dir_all(&scope).unwrap();
    fs::write(scope.join("codex-hook-grant.json"), json!({"session_id":"session","turn_id":"turn","worktree":worktree,"prompt_hash":hash,"generation":1,"nonce_digest":pipeline::usage::digest_bytes(b"nonce"),"handler_digest":"handler","expires":expires}).to_string()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(fixture.repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .env("GAL_PINNED_EXECUTABLE_SHA256", executable_hash)
        .args(["pipeline", "--require-codex-stop-v1", &prompt])
        .output()
        .unwrap();
    check(&output);
    let operation = field(&output, "operation_id");
    assert_eq!(field(&output, "terminal_operation_id"), operation);
    let records = fixture.evidence();
    let transition = records
        .iter()
        .find(|record| {
            record.source_kind == "transition" && record.content["request"]["command"] == "pipeline"
        })
        .unwrap();
    assert_eq!(transition.content["after_revision"], 2);
    assert_eq!(
        transition.content["invariants"]["inv-allowed-edge"]["status"],
        "pass"
    );
    assert_eq!(
        transition.content["invariants"]["inv-authorization-binding"]["status"],
        "pass"
    );
    assert_eq!(transition.content["run_association"], "bound");
    let entry = format!("entry-{}", operation.strip_prefix("op-").unwrap());
    assert_eq!(
        fixture
            .store()
            .read(&entry, &format!("{operation}-end"))
            .unwrap()
            .content["status"],
        "stopped"
    );
    let status = fixture.call(&["pipeline", "--require-codex-stop-v1", &prompt, "--status"]);
    check(&status);
    assert!(!String::from_utf8_lossy(&status.stdout).contains("terminal_operation_id:"));
    let refusal = fixture.call(&["pipeline", "--require-codex-stop-v1", &prompt]);
    assert!(!refusal.status.success());
    let records = fixture.evidence();
    let refused = records
        .iter()
        .find(|record| {
            record.source_kind == "transition"
                && record.content["request"]["command"] == "pipeline"
                && record.content["detail"]["outcome"] == "refused"
        })
        .unwrap();
    assert_eq!(refused.content["before_revision"], 2);
    assert_eq!(refused.content["after_revision"], 2);
    assert_eq!(
        refused.content["invariants"]["inv-allowed-edge"]["status"],
        "fail"
    );
}

#[test]
fn unavailable_store_and_coordinator_conflict_never_allocate_another_run() {
    let fixture = Fixture::new();
    let prompt = fixture.pipeline_prompt();
    check(&fixture.call(&["pipeline-preflight", &prompt]));
    let path = fixture
        .repo
        .path()
        .join(".dev/pipeline/capture/coordinator.json");
    fs::write(
        &path,
        json!({"accounting_run":{"run_id":"other-run"}}).to_string(),
    )
    .unwrap();
    let before = fixture
        .evidence()
        .iter()
        .filter(|record| record.source_kind == "run")
        .count();
    check(&fixture.call(&["pipeline-preflight", &prompt]));
    let records = fixture.evidence();
    assert_eq!(
        records
            .iter()
            .filter(|record| record.source_kind == "run")
            .count(),
        before
    );
    assert!(records
        .iter()
        .any(|record| record.source_kind == "transition"
            && record.content["run_association"] == "run-association-unknown"));

    let unavailable = Fixture::new();
    let prompt = unavailable.pipeline_prompt();
    fs::create_dir_all(unavailable.repo.path().join(".git/gal/evidence")).unwrap();
    fs::write(
        unavailable.repo.path().join(".git/gal/evidence/v1"),
        "blocked",
    )
    .unwrap();
    let output = unavailable.call(&["pipeline-preflight", &prompt]);
    check(&output);
    let receipt = fs::read_to_string(
        unavailable
            .repo
            .path()
            .join(".dev/pipeline/capture/preflight.receipt.md"),
    )
    .unwrap();
    assert!(receipt.contains("run_association: run-association-unknown"));
    assert!(!receipt.contains("run_id:"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("operation_id:"));
}

#[test]
fn pipeline_run_roster_records_version_and_malformed_reason() {
    for (roster, version, reason) in [
        ("Roster-Version: roster.2\n", Some("roster.2"), None),
        (
            "Roster-Version: a\nRoster-Version: b\n",
            None,
            Some("roster-malformed"),
        ),
    ] {
        let fixture = Fixture::new();
        let prompt = fixture.pipeline_prompt();
        let conventions = fixture.repo.path().join("plugins/gal-core/conventions");
        fs::create_dir_all(&conventions).unwrap();
        fs::write(conventions.join("governance-report.md"), roster).unwrap();
        check(&fixture.call(&["pipeline-preflight", &prompt]));
        let records = fixture.evidence();
        let run = records
            .iter()
            .find(|record| record.source_kind == "run")
            .unwrap();
        assert_eq!(run.content["design_roster_version"], json!(version));
        assert_eq!(run.content["design_roster_reason"], json!(reason));
        let operation = records
            .iter()
            .find(|record| record.source_kind == "operation-begin" && record.phase == "pipeline")
            .unwrap();
        assert_eq!(operation.content["design_roster_version"], json!(version));
    }
}

#[test]
fn gate_rounds_survive_receipt_overwrite_and_write_failure() {
    let fixture = Fixture::new();
    fs::write(fixture.repo.path().join("plan.md"), "# Plan\n").unwrap();
    fixture.call(&["planning-check", "plan.md", "--receipt", "receipt.md"]);
    let first = fs::read_to_string(fixture.repo.path().join("receipt.md")).unwrap();
    fixture.call(&["planning-check", "plan.md", "--receipt", "receipt.md"]);
    fs::write(fixture.repo.path().join("blocker"), "file").unwrap();
    fixture.call(&[
        "planning-check",
        "plan.md",
        "--receipt",
        "blocker/receipt.md",
    ]);

    let store = fixture.store();
    let mut records = Vec::new();
    for directory in fs::read_dir(fixture.repo.path().join(".git/gal/evidence/v1")).unwrap() {
        let run = directory
            .unwrap()
            .file_name()
            .to_string_lossy()
            .into_owned();
        if run.starts_with("entry-") {
            for item in store.list_page(&run, None, 256).unwrap().entries {
                records.push(store.read(&run, &item.record_id).unwrap());
            }
        }
    }
    let gates: Vec<_> = records
        .iter()
        .filter(|r| r.source_kind == "gate-result")
        .collect();
    assert_eq!(gates.len(), 3);
    assert!(gates.iter().any(|r| r.content["receipt_text"] == first));
    for gate in gates {
        let raw = gate.content["receipt_text"].as_str().unwrap();
        assert_eq!(
            gate.content["receipt_sha256"],
            pipeline::usage::digest_bytes(raw.as_bytes())
        );
        assert_eq!(gate.content["gate"], "planning");
        assert!(!gate.content["results"].as_array().unwrap().is_empty());
    }
    assert!(records
        .iter()
        .any(|r| r.source_kind == "receipt-write" && r.content["status"] == "fail"));
}

#[test]
fn malformed_receipt_consumptions_leave_expected_and_terminal_evidence() {
    let fixture = Fixture::new();
    fs::write(fixture.repo.path().join("receipt.md"), "malformed\n").unwrap();
    let doctor = fixture.call(&[
        "doctor",
        "--verify-receipt",
        "receipt.md",
        "--expect-scope",
        "standalone",
    ]);
    assert!(!doctor.status.success());
    let repair = fixture.call(&[
        "format-repair",
        "--target",
        "doc.md",
        "--receipt",
        "receipt.md",
        "--repair-id",
        "bullet-shape",
        "--gate",
        "preflight",
        "--mode",
        "ordinary",
        "--attempt",
        "attempt",
        "--round",
        "0",
    ]);
    assert!(!repair.status.success());
    let store = fixture.store();
    let root = fixture.repo.path().join(".git/gal/evidence/v1");
    let mut records = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let run = entry.file_name().to_string_lossy().into_owned();
        if !run.starts_with("entry-") {
            continue;
        }
        for item in store.list_page(&run, None, 256).unwrap().entries {
            records.push(store.read(&run, &item.record_id).unwrap());
        }
    }
    let expected = records
        .iter()
        .filter(|r| r.source_kind == "verification-expected")
        .count();
    let outcomes: Vec<_> = records
        .iter()
        .filter(|r| r.source_kind == "verification")
        .collect();
    assert_eq!(expected, 2);
    assert_eq!(outcomes.len(), 2);
    for outcome in outcomes {
        assert_eq!(outcome.content["parse_result"]["status"], "fail");
        assert_eq!(outcome.content["decision"], "reject");
        assert_eq!(outcome.content["consumption_eligible"], false);
        assert!(outcome.content["receipt_sha256"].is_string());
        if outcome.content["expected"]["gate"] == "preflight" {
            assert_eq!(
                outcome.content["freshness"]["repair_context"]["gate"]["expected"],
                "preflight"
            );
            assert_eq!(
                outcome.content["freshness"]["repair_context"]["gate"]["observed"],
                json!([])
            );
        }
    }
}

#[test]
fn repair_context_mismatch_records_expected_and_observed_values() {
    let fixture = Fixture::new();
    fs::write(
        fixture.repo.path().join("receipt.md"),
        "gate: older\nmode: ordinary\ngate_attempt_id: previous\nrepair_round: 1\n",
    )
    .unwrap();
    let output = fixture.call(&[
        "format-repair",
        "--target",
        "doc.md",
        "--receipt",
        "receipt.md",
        "--repair-id",
        "bullet-shape",
        "--gate",
        "preflight",
        "--mode",
        "ordinary",
        "--attempt",
        "attempt",
        "--round",
        "0",
    ]);
    assert!(!output.status.success());
    let store = fixture.store();
    let root = fixture.repo.path().join(".git/gal/evidence/v1");
    let outcomes: Vec<_> = fs::read_dir(root)
        .unwrap()
        .flat_map(|entry| {
            let run = entry.unwrap().file_name().to_string_lossy().into_owned();
            if !run.starts_with("entry-") {
                return Vec::new();
            }
            store
                .list_page(&run, None, 256)
                .unwrap()
                .entries
                .into_iter()
                .map(|item| store.read(&run, &item.record_id).unwrap())
                .filter(|r| r.source_kind == "verification")
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(outcomes.len(), 1);
    let content = &outcomes[0].content;
    assert_eq!(
        content["freshness"]["repair_context"]["gate"],
        json!({"expected":"preflight", "observed":["older"]})
    );
    assert_eq!(
        content["freshness"]["repair_context"]["gate_attempt_id"],
        json!({"expected":"attempt", "observed":["previous"]})
    );
    assert_eq!(
        content["freshness"]["repair_context"]["repair_round"],
        json!({"expected":0, "observed":["1"]})
    );
    assert_eq!(content["decision"], "reject");
    assert_eq!(content["consumption_eligible"], false);
}
#[test]
fn receipt_binding_observations_cover_valid_and_changed_executable_hashes() {
    for changed in [false, true] {
        let fixture = Fixture::new();
        let receipt_path = fixture.repo.path().join("receipt.md");
        fs::write(&receipt_path, "malformed\n").unwrap();
        fixture.call(&[
            "doctor",
            "--verify-receipt",
            "receipt.md",
            "--expect-scope",
            "standalone",
        ]);
        let records = || {
            let store = fixture.store();
            fs::read_dir(fixture.repo.path().join(".git/gal/evidence/v1"))
                .unwrap()
                .flat_map(|entry| {
                    let run = entry.unwrap().file_name().to_string_lossy().into_owned();
                    if !run.starts_with("entry-") {
                        return Vec::new();
                    }
                    store
                        .list_page(&run, None, 256)
                        .unwrap()
                        .entries
                        .into_iter()
                        .map(|item| store.read(&run, &item.record_id).unwrap())
                        .filter(|record| record.source_kind == "verification")
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        let expected = records()[0].content["expected_envelope"].clone();
        let executable_hash = expected["executable_sha256"].as_str().unwrap();
        let observed_hash = if changed {
            "0".repeat(64)
        } else {
            executable_hash.to_owned()
        };
        let receipt = format!(
            "gate: preflight\nmode: ordinary\ngate_attempt_id: attempt\nrepair_round: 0\nbinding_scope: standalone\nexecution_binding_sha256: {}\nexecutable_path: {}\nexecutable_sha256: {}\n",
            expected["execution_binding_sha256"].as_str().unwrap(),
            expected["executable_path"].as_str().unwrap(), observed_hash
        );
        fs::write(&receipt_path, receipt).unwrap();
        let doctor = fixture.call(&[
            "doctor",
            "--verify-receipt",
            "receipt.md",
            "--expect-scope",
            "standalone",
        ]);
        assert_eq!(doctor.status.success(), !changed);
        let repair = fixture.call(&[
            "format-repair",
            "--target",
            "doc.md",
            "--receipt",
            "receipt.md",
            "--repair-id",
            "bullet-shape",
            "--gate",
            "preflight",
            "--mode",
            "ordinary",
            "--attempt",
            "attempt",
            "--round",
            "0",
        ]);
        assert!(!repair.status.success());
        let outcomes = records();
        assert_eq!(outcomes.len(), 3);
        assert_eq!(
            outcomes
                .iter()
                .filter(|record| record.content["parse_result"]["status"] == "pass")
                .count(),
            2
        );
        for outcome in outcomes
            .iter()
            .filter(|record| record.content["parse_result"]["status"] == "pass")
        {
            let content = &outcome.content;
            assert_eq!(content["expected_envelope"], expected);
            assert_eq!(content["actual"]["executable_sha256"], observed_hash);
            assert_eq!(
                content["freshness"]["envelope"]["status"],
                if changed { "fail" } else { "pass" }
            );
            assert_eq!(
                content["decision"],
                if !changed && content["expected"]["binding_scope"] == "standalone" {
                    "accept"
                } else {
                    "reject"
                }
            );
            assert_eq!(
                content["consumption_eligible"],
                !changed && content["expected"]["binding_scope"] == "standalone"
            );
        }
    }
}

fn field(output: &Output, field: &str) -> String {
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{field}: ")))
        .unwrap_or_else(|| panic!("missing {field}: {text}"))
        .to_owned()
}
fn check(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn receipt_consumers_record_binding_and_freshness_cases() {
    for consumer in ["doctor", "format-repair"] {
        for case in [
            "stale",
            "prompt-drift",
            "worktree-drift",
            "bootstrap-mismatch",
            "malformed",
            "non-pass",
            "valid",
        ] {
            let fixture = Fixture::new();
            let records = || {
                let store = fixture.store();
                fs::read_dir(fixture.repo.path().join(".git/gal/evidence/v1"))
                    .unwrap()
                    .flat_map(|entry| {
                        let run = entry.unwrap().file_name().to_string_lossy().into_owned();
                        if !run.starts_with("entry-") {
                            return Vec::new();
                        }
                        store
                            .list_page(&run, None, 256)
                            .unwrap()
                            .entries
                            .into_iter()
                            .map(|item| store.read(&run, &item.record_id).unwrap())
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            };
            fs::write(fixture.repo.path().join("receipt.md"), "malformed\n").unwrap();
            let bootstrap = fixture.call(&[
                "doctor",
                "--verify-receipt",
                "receipt.md",
                "--expect-scope",
                "standalone",
            ]);
            assert!(!bootstrap.status.success());
            let initial = records();
            let expected = initial
                .iter()
                .find(|record| record.source_kind == "verification")
                .unwrap()
                .content["expected_envelope"]
                .clone();
            let original = b"\xef\xbb\xbf## Tasks\n- [ ] example task\n";
            fs::write(fixture.repo.path().join("doc.prompt.md"), original).unwrap();
            fs::write(fixture.repo.path().join("prompt.md"), "current prompt\n").unwrap();
            for args in [
                vec!["config", "user.name", "Receipt Probe"],
                vec!["config", "user.email", "receipt@example.invalid"],
                vec!["add", "doc.prompt.md"],
                vec!["commit", "-qm", "fixture"],
            ] {
                assert!(Command::new("git")
                    .args(args)
                    .current_dir(fixture.repo.path())
                    .output()
                    .unwrap()
                    .status
                    .success());
            }
            let current_root = fixture
                .repo
                .path()
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .trim_start_matches("\\\\?\\")
                .replace('\\', "/");
            let observed_root = if case == "worktree-drift" {
                format!("{current_root}/other")
            } else {
                current_root.clone()
            };
            let binding = pipeline::coordinator::ExecutionBinding {
                version: pipeline::coordinator::ExecutionBinding::VERSION,
                binding_scope: "standalone".into(),
                worktree_root: observed_root.clone(),
                executable_path: expected["executable_path"].as_str().unwrap().into(),
                executable_sha256: expected["executable_sha256"].as_str().unwrap().into(),
            };
            let observed_binding = if case == "bootstrap-mismatch" {
                "0".repeat(64)
            } else {
                binding.digest().unwrap()
            };
            let mut receipt = format!(
                "overall: {}\ngate: prompt\nmode: default\ngate_attempt_id: attempt\nrepair_round: 0\nrepair_target: doc.prompt.md sha256:{}\n- tasks-heading: format (repair_id: leading-bom, target: doc.prompt.md)\nbinding_scope: standalone\nexecution_binding_sha256: {}\nexecutable_path: {}\nexecutable_sha256: {}\n",
                if case == "non-pass" { "fail" } else { "pass" }, pipeline::usage::digest_bytes(original), observed_binding,
                expected["executable_path"].as_str().unwrap(), expected["executable_sha256"].as_str().unwrap());
            if case == "worktree-drift" {
                receipt.push_str(&format!("worktree_root: {observed_root}\n"));
            }
            if case == "stale" {
                receipt.push_str("expires_at: 1\n");
            }
            if case == "prompt-drift" {
                receipt.push_str(&format!(
                    "prompt_path: prompt.md\nprompt_sha256: {}\n",
                    "0".repeat(64)
                ));
            }
            if case == "malformed" {
                receipt = format!(
                    "malformed\nexpires_at: 1\nprompt_path: prompt.md\nprompt_sha256: {}\n",
                    "0".repeat(64)
                );
            }
            fs::write(fixture.repo.path().join("receipt.md"), &receipt).unwrap();
            let output = if consumer == "doctor" {
                fixture.call(&[
                    "doctor",
                    "--verify-receipt",
                    "receipt.md",
                    "--expect-scope",
                    "standalone",
                ])
            } else {
                fixture.call(&[
                    "format-repair",
                    "--target",
                    "doc.prompt.md",
                    "--receipt",
                    "receipt.md",
                    "--repair-id",
                    "leading-bom",
                    "--gate",
                    "prompt",
                    "--mode",
                    "default",
                    "--attempt",
                    "attempt",
                    "--round",
                    "0",
                ])
            };
            let accepted = !matches!(case, "worktree-drift" | "bootstrap-mismatch" | "malformed");
            assert_eq!(
                output.status.success(),
                accepted,
                "{consumer} {case}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let all = records();
            let outcome = all
                .iter()
                .find(|record| {
                    record.source_kind == "verification"
                        && !initial.iter().any(|old| old.event_id == record.event_id)
                })
                .unwrap();
            let content = &outcome.content;
            assert_eq!(
                content["decision"],
                if accepted { "accept" } else { "reject" },
                "{consumer} {case}"
            );
            assert_eq!(content["consumption_eligible"], accepted);
            assert_eq!(content["expected_envelope"], expected);
            assert_eq!(
                content["parse_result"]["status"],
                if case == "malformed" { "fail" } else { "pass" }
            );
            assert_eq!(
                content["receipt_sha256"],
                pipeline::usage::digest_bytes(receipt.as_bytes())
            );
            let conditions = &content["freshness"]["conditions"];
            for key in ["executable_sha256", "execution_binding_sha256"] {
                assert_eq!(conditions[key]["expected"], expected[key]);
                if case == "malformed" {
                    assert_eq!(conditions[key]["status"], "unknown");
                    assert!(conditions[key]["observed"].is_null());
                } else {
                    assert_eq!(
                        conditions[key]["observed"],
                        if key == "execution_binding_sha256" {
                            json!(observed_binding)
                        } else {
                            expected[key].clone()
                        }
                    );
                    assert_eq!(
                        conditions[key]["status"],
                        if key == "execution_binding_sha256"
                            && matches!(case, "worktree-drift" | "bootstrap-mismatch")
                        {
                            "fail"
                        } else {
                            "pass"
                        }
                    );
                }
            }
            assert_eq!(conditions["worktree_root"]["expected"], current_root);
            assert_eq!(
                conditions["worktree_root"]["status"],
                if case == "worktree-drift" {
                    "fail"
                } else {
                    "unknown"
                }
            );
            if case == "worktree-drift" {
                assert_eq!(conditions["worktree_root"]["observed"], observed_root);
            }
            assert_eq!(
                conditions["prompt_sha256"]["status"],
                if case == "prompt-drift" {
                    "fail"
                } else {
                    "unknown"
                }
            );
            if case == "prompt-drift" {
                assert_eq!(
                    conditions["prompt_sha256"]["expected"],
                    pipeline::usage::digest_bytes(b"current prompt\n")
                );
                assert_eq!(conditions["prompt_sha256"]["observed"], "0".repeat(64));
            } else if case == "malformed" {
                assert_eq!(
                    conditions["prompt_sha256"]["reason"],
                    "receipt-envelope-unavailable"
                );
            } else {
                assert_eq!(
                    conditions["prompt_sha256"]["reason"],
                    "receipt-has-no-prompt-hash"
                );
            }
            assert_eq!(
                conditions["expiration"]["status"],
                if case == "stale" { "fail" } else { "unknown" }
            );
            if case == "stale" {
                assert_eq!(
                    conditions["expiration"]["observed"]["expires_at_unix_seconds"],
                    1
                );
            } else if case == "malformed" {
                assert_eq!(
                    conditions["expiration"]["reason"],
                    "receipt-envelope-unavailable"
                );
            } else {
                assert_eq!(
                    conditions["expiration"]["reason"],
                    "receipt-has-no-expiration"
                );
            }
            assert_eq!(
                conditions["receipt_verdict"]["status"],
                match case {
                    "malformed" => "unknown",
                    "non-pass" => "fail",
                    _ => "pass",
                }
            );
            let expected_event = all
                .iter()
                .find(|record| {
                    record.operation_id == outcome.operation_id
                        && record.source_kind == "verification-expected"
                })
                .unwrap();
            assert_eq!(expected_event.content["consumer"], consumer);
        }
    }
}

#[test]
fn begin_replays_and_end_preserves_plan_and_rejects_conflicts() {
    let fixture = Fixture::new();
    let first = fixture.begin("stable");
    check(&first);
    let op = field(&first, "operation_id");
    let second = fixture.begin("stable");
    check(&second);
    assert_eq!(op, field(&second, "operation_id"));
    let run = format!("entry-{}", &op[3..]);
    let begin = fixture.store().read(&run, &op).unwrap();
    assert_eq!(begin.content["binding"]["reason"], "session-absent");
    let plan = fixture.repo.path().join("plan.md");
    fs::write(&plan, b"plan content").unwrap();
    let args = [
        "usage",
        "end",
        "--operation",
        &op,
        "--status",
        "completed",
        "--plan",
        plan.to_str().unwrap(),
    ];
    let end = fixture.call(&args);
    check(&end);
    assert_eq!(field(&end, "terminal_operation_id"), op);
    check(&fixture.call(&args));
    let record = fixture.store().read(&run, &format!("{op}-end")).unwrap();
    assert_eq!(
        record.content["plan_sha256"],
        pipeline::usage::digest_bytes(b"plan content")
    );
    assert!(!fixture
        .call(&[
            "usage",
            "end",
            "--operation",
            &op,
            "--status",
            "stopped",
            "--reason",
            "stop-at"
        ])
        .status
        .success());
}

#[test]
fn reasons_are_validated_before_storage() {
    let fixture = Fixture::new();
    let op = field(&fixture.begin("reasons"), "operation_id");
    for extra in [
        vec!["--status", "stopped"],
        vec!["--status", "stopped", "--reason", "unknown"],
        vec!["--status", "completed", "--reason", "stop-at"],
    ] {
        let mut args = vec!["usage", "end", "--operation", &op];
        args.extend(extra);
        assert_eq!(fixture.call(&args).status.code(), Some(64));
    }
    check(&fixture.call(&[
        "usage",
        "end",
        "--operation",
        &op,
        "--status",
        "stopped",
        "--reason",
        "stop-at",
    ]));
}

#[test]
fn finalize_reuses_unfinished_operation_and_starts_another_after_end() {
    let fixture = Fixture::new();
    let worktree = fixture.repo.path().to_str().unwrap();
    let args = [
        "usage",
        "begin",
        "--phase",
        "finalize",
        "--run",
        "run-example",
        "--worktree",
        worktree,
        "--host",
        "codex",
    ];
    let first = fixture.call(&args);
    check(&first);
    let op = field(&first, "operation_id");
    let second = fixture.call(&args);
    check(&second);
    assert_eq!(field(&second, "operation_id"), op);
    assert_eq!(
        fixture
            .store()
            .read("run-example", &format!("finalize-{op}"))
            .unwrap()
            .operation_id,
        op
    );
    check(&fixture.call(&["usage", "end", "--operation", &op, "--status", "completed"]));
    let next = fixture.call(&args);
    check(&next);
    assert_ne!(field(&next, "operation_id"), op);
}

#[test]
fn failed_begin_leaves_a_gap_that_is_imported() {
    let fixture = Fixture::new();
    let gal = fixture.repo.path().join(".git/gal");
    fs::create_dir_all(&gal).unwrap();
    let obstruction = gal.join("evidence");
    fs::write(&obstruction, b"blocked").unwrap();
    assert!(!fixture.begin("gap").status.success());
    let gap = fixture.repo.path().join(".dev/pipeline/usage-gap");
    assert_eq!(fs::read_dir(&gap).unwrap().count(), 1);
    fs::remove_file(obstruction).unwrap();
    let output = fixture.begin("gap");
    check(&output);
    let op = field(&output, "operation_id");
    let run = format!("entry-{}", &op[3..]);
    let page = fixture.store().list_page(&run, None, 256).unwrap();
    assert!(page
        .entries
        .iter()
        .any(|entry| entry.record_id.starts_with("gap-")));
    assert_eq!(fs::read_dir(gap).unwrap().count(), 0);
}

fn seed(
    store: &EvidenceStore,
    hex: &str,
    pid: Option<u32>,
    birth: Option<String>,
) -> (String, String) {
    let op = format!("op-{hex}");
    let run = format!("entry-{hex}");
    let record = EvidenceRecord {
        schema_version: 1,
        event_id: op.clone(),
        operation_id: op.clone(),
        run_id: run.clone(),
        plan_generation: None,
        task_id: None,
        attempt_id: None,
        parent_attempt_id: None,
        phase: "pipeline".into(),
        source_kind: "operation-begin".into(),
        source_version: "1".into(),
        recorded_at_unix_ms: 1,
        content_hash: "fixture".into(),
        content: json!({"owner_pid":pid,"owner_birth":birth,"sequence":1}),
    };
    store.publish(&record).unwrap();
    (run, op)
}

#[test]
fn ownership_keeps_live_and_unknown_operations() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let (unknown_run, unknown) = seed(&store, "0000000000000001", None, None);
    let (live_run, live) = seed(
        &store,
        "0000000000000002",
        Some(std::process::id()),
        dispatch::dispatch::process_birth_identity(std::process::id()),
    );
    check(&fixture.begin("inspect"));
    assert!(store
        .read(&unknown_run, &format!("{unknown}-interrupted"))
        .is_err());
    assert!(store
        .read(&live_run, &format!("{live}-interrupted"))
        .is_err());
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[test]
fn dead_owner_is_recorded_as_interrupted() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let (run, op) = seed(
        &store,
        "0000000000000003",
        Some(999_999),
        Some("gone".into()),
    );
    check(&fixture.begin("inspect-dead"));
    assert_eq!(
        store
            .read(&run, &format!("{op}-interrupted"))
            .unwrap()
            .content["status"],
        "interrupted"
    );
}

#[test]
fn all_stop_reasons_are_accepted_and_other_combinations_are_rejected() {
    let fixture = Fixture::new();
    let pipeline = [
        "design-defect",
        "retry-ceiling",
        "stop-at",
        "preflight-rejected",
        "security-protected-path",
        "goal-gaps-blocked",
        "head-drift",
        "boundary-scope-decision",
        "convergence-human-repair",
        "task-quality-unanswered",
    ];
    let finalize = [
        "ACCEPTANCE",
        "REVIEW",
        "RELEASE",
        "MERGE",
        "TEARDOWN",
        "CLOSE",
    ];
    assert_eq!(pipeline.len(), 10);
    assert_eq!(finalize.len(), 6);
    for reason in pipeline.into_iter().chain(finalize) {
        let begin = fixture.begin(reason);
        check(&begin);
        let op = field(&begin, "operation_id");
        check(&fixture.call(&[
            "usage",
            "end",
            "--operation",
            &op,
            "--status",
            "stopped",
            "--reason",
            reason,
        ]));
    }
    let begin = fixture.begin("invalid-reasons");
    check(&begin);
    let op = field(&begin, "operation_id");
    for reason in ["continue", "goal-verified", "human-required", "out-of-list"] {
        assert_eq!(
            fixture
                .call(&[
                    "usage",
                    "end",
                    "--operation",
                    &op,
                    "--status",
                    "stopped",
                    "--reason",
                    reason
                ])
                .status
                .code(),
            Some(64),
            "{reason}"
        );
    }
    assert_eq!(
        fixture
            .call(&["usage", "end", "--operation", &op, "--status", "stopped"])
            .status
            .code(),
        Some(64)
    );
    assert_eq!(
        fixture
            .call(&[
                "usage",
                "end",
                "--operation",
                &op,
                "--status",
                "completed",
                "--reason",
                "stop-at"
            ])
            .status
            .code(),
        Some(64)
    );
    let run = format!("entry-{}", &op[3..]);
    assert!(fixture.store().read(&run, &format!("{op}-end")).is_err());
}

#[test]
fn unsupported_host_session_is_unavailable() {
    let fixture = Fixture::new();
    let output = fixture.call(&[
        "usage",
        "begin",
        "--phase",
        "planning",
        "--invocation",
        "unsupported",
        "--worktree",
        fixture.repo.path().to_str().unwrap(),
        "--host",
        "unknown",
        "--host-session",
        "codex:example",
    ]);
    check(&output);
    let binding: serde_json::Value = serde_json::from_str(&field(&output, "binding")).unwrap();
    assert_eq!(
        binding,
        json!({"status":"unavailable", "reason":"host-unsupported"})
    );
}

#[test]
fn mismatched_host_session_is_unavailable() {
    let fixture = Fixture::new();
    let output = fixture.call(&[
        "usage",
        "begin",
        "--phase",
        "planning",
        "--invocation",
        "mismatch",
        "--worktree",
        fixture.repo.path().to_str().unwrap(),
        "--host",
        "codex",
        "--host-session",
        "claude:example",
    ]);
    check(&output);
    let binding: serde_json::Value = serde_json::from_str(&field(&output, "binding")).unwrap();
    assert_eq!(
        binding,
        json!({"status":"unavailable", "reason":"session-mismatch"})
    );
}

#[test]
fn lost_begin_response_recovers_original_operation() {
    let fixture = Fixture::new();
    check(&fixture.begin("lost-response"));
    let entries: Vec<_> = fs::read_dir(fixture.repo.path().join(".git/gal/evidence/v1"))
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("entry-"))
        .collect();
    assert_eq!(entries.len(), 1);
    let entry = entries[0].file_name().to_string_lossy().into_owned();
    let original = format!("op-{}", &entry[6..]);
    let stored = fixture.store().read(&entry, &original).unwrap();
    assert_eq!(stored.content["invocation_id"], "lost-response");
    let recovered = fixture.begin("lost-response");
    check(&recovered);
    assert_eq!(field(&recovered, "operation_id"), original);
    assert_eq!(fixture.store().read(&entry, &original).unwrap(), stored);
}

#[test]
fn missing_owner_birth_and_skill_operation_remain_unfinished() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let (run, op) = seed(&store, "0000000000000004", Some(std::process::id()), None);
    let skill = fixture.begin("skill-operation");
    check(&skill);
    let skill_op = field(&skill, "operation_id");
    let skill_run = format!("entry-{}", &skill_op[3..]);
    let record = store.read(&skill_run, &skill_op).unwrap();
    assert!(record.content["owner_pid"].is_null());
    assert!(record.content["owner_birth"].is_null());
    check(&fixture.begin("inspect-unknown"));
    assert!(store.read(&run, &format!("{op}-interrupted")).is_err());
    assert!(store
        .read(&skill_run, &format!("{skill_op}-interrupted"))
        .is_err());
}
