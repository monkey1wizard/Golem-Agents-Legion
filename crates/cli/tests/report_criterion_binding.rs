use pipeline::report::{
    artifact_snapshot, criterion_status, freeze_inputs, CriterionBinding, CriterionMapping,
    CriterionRow, CriterionStatus, EvidenceRecord, EvidenceStore,
};
use pipeline::usage::digest_bytes;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::OnceLock,
};
use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn executor() -> &'static Path {
    static EXECUTOR: OnceLock<TempDir> = OnceLock::new();
    EXECUTOR.get_or_init(|| {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("receipt_executor.rs");
        fs::write(&source, r####"
use std::{io::{self, Read}, fs};
fn main() {
    let mut input = String::new(); io::stdin().read_to_string(&mut input).unwrap();
    let task = input.lines().find_map(|line|line.strip_prefix("# Task Spec: ")).unwrap().split_whitespace().next().unwrap();
    let mode = std::env::var("RECEIPT_MODE").unwrap_or_default();
    let receipt = input.lines().find_map(|line|line.split_once("receipt file: ").map(|(_,path)|path.trim_end_matches('.'))).unwrap();
    let sample = input.lines().find_map(|line|line.split_once("`Pipeline Evidence: ").map(|(_,tail)|tail.split('`').next().unwrap())).unwrap();
    let mut rows = Vec::new();
    for line in input.lines().filter(|line|line.trim().starts_with("- tp_id=")) {
        let field = |name: &str| line.split_whitespace().find_map(|part|part.strip_prefix(name)).unwrap();
        let tp = field("tp_id="); let criterion = field("criterion_id="); let binding = field("binding_sha256=");
        let outcome = if mode == "fail" && rows.is_empty() { "fail" } else if mode == "not-run" && rows.is_empty() { "not-run" } else { "pass" };
        rows.push(format!("{{\"tp_id\":\"{tp}\",\"criterion_id\":\"{criterion}\",\"outcome\":\"{outcome}\",\"binding_sha256\":\"{binding}\",\"evidence\":[\"observed assertion\"]}}"));
    }
    if mode == "missing" { rows.pop(); }
    if mode == "duplicate" { rows.push(rows[0].clone()); }
    if mode == "unknown" { rows[0] = rows[0].replacen("\"tp_id\":\"", "\"tp_id\":\"unknown-", 1); }
    if mode == "binding" || mode == "shrink" { rows[0] = rows[0].replacen("\"binding_sha256\":\"", "\"binding_sha256\":\"changed", 1); }
    if mode == "extra" { rows[0] = rows[0].replacen('{', "{\"extra\":true,", 1); }
    if mode == "source" { rows[0] = rows[0].replacen('{', "{\"source_kind\":\"unknown\",", 1); }
    if mode == "empty" { rows[0] = rows[0].replace("[\"observed assertion\"]", "[]"); }
    let gate = format!("{{\"gate_id\":\"verification\",\"premise_id\":\"behavior\",\"premise_text\":\"observed behavior\",\"outcome\":\"pass\",\"findings\":[],\"criterion_results\":[{}]}}", rows.join(","));
    let evidence = if mode == "historical" { sample.replace("\"schema_version\":2", "\"schema_version\":1").replace("<id>", "verification") }
        else if mode == "old-format" { String::new() }
        else if sample.contains("\"schema_version\":2") { format!("{},\"gates\":[{gate}]}}", sample.split(",\"gates\":").next().unwrap()) }
        else { sample.replace("<id>", "verification") };
    let verdict = if mode == "verdict-fail" { "FAIL" } else { "PASS" };
    let mut content = format!("### [{task}] 2026-11-12\nVerdict: {verdict}\nRed-proof base failure: expected failure observed\nEvidence: observed assertions\n");
    if input.contains("producer=`AUDITOR`") { content.push_str("<!-- AUDIT_REVIEW: CLEAR -->\n"); }
    if !evidence.is_empty() { content.push_str(&format!("Pipeline Evidence: {evidence}\n")); }
    if let Some(parent)=std::path::Path::new(receipt).parent(){fs::create_dir_all(parent).unwrap();}
    fs::write(receipt, content).unwrap();
    if mode == "artifact-change" { fs::write("product.txt", "changed during verification\n").unwrap(); }
    println!("{{\"conversation_id\":\"176f1141-b606-47aa-a47b-76f2a6147623\",\"status\":\"SUCCESS\"}}");
}
"####).unwrap();
        let output = Command::new("rustc").arg(&source).arg("-o").arg(dir.path().join(if cfg!(windows) { "agy.exe" } else { "agy" })).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        dir
    }).path()
}

struct Fixture {
    repo: TempDir,
    home: TempDir,
    prompt: PathBuf,
    task: String,
    bindings: Vec<CriterionBinding>,
    mappings: Vec<Value>,
}

impl Fixture {
    fn new(two_criteria: bool) -> Self {
        let repo = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        git(repo.path(), &["init", "-q"]);
        for args in [
            ["config", "user.name", "Fixture"],
            ["config", "user.email", "fixture@example.com"],
            ["config", "commit.gpgsign", "false"],
            ["config", "core.hooksPath", ".git/hooks"],
        ] {
            git(repo.path(), &args);
        }
        fs::write(repo.path().join(".gitignore"), ".dev/\n").unwrap();
        fs::write(repo.path().join("product.txt"), "original\n").unwrap();
        let agents = repo.path().join("plugins/gal-core/agents");
        fs::create_dir_all(&agents).unwrap();
        for role in ["tester", "auditor"] {
            fs::write(
                agents.join(format!("golem-{role}.agent.md")),
                format!("---\nname: golem-{role}\n---\n"),
            )
            .unwrap();
        }
        git(repo.path(), &["add", "."]);
        git(repo.path(), &["commit", "-qm", "fixture"]);
        let head = git(repo.path(), &["rev-parse", "HEAD"]);
        let task = format!("T-{:02}", 1);
        let criteria = if two_criteria {
            vec!["behavior", "integrity"]
        } else {
            vec!["behavior"]
        };
        let mut mappings = Vec::new();
        let mut table = String::new();
        for number in 1..=if two_criteria { 1 } else { 2 } {
            let tp = format!("TP-{number:02}");
            let declarations = criteria.iter().map(|criterion| {
                mappings.push(json!({"tp_id":tp,"criterion_id":criterion,"task_id":task,"gate_id":"verification"}));
                json!({"criterion_id":criterion,"requirements":["R1"],"gate_id":"verification","pass_condition":"assertion holds","source_kind":"test","target":null,"security":null})
            }).collect::<Vec<_>>();
            table.push_str(&format!(
                "| {tp} | integration | Behavior | {task} | {} |\n",
                json!(declarations)
            ));
        }
        let plans = repo.path().join(".dev/plans");
        fs::create_dir_all(&plans).unwrap();
        let prompt = plans.join("criterion.prompt.md");
        fs::write(&prompt, format!("# Criterion\n\n## Tasks\n\n- [ ] {task} behavior\n  - **Files**: `product.txt`\n  - **Change**: Verify behavior.\n  - **Acceptance**: Assertion holds.\n\n## Status\n\nCurrent Task: {task}\nTask Final Commit: {head}\n\n## Test Results\n\n## Review Results\n\n## Test Plan\n\n| ID | Type | Description | Covers | Criteria |\n| --- | --- | --- | --- | --- |\n{table}")).unwrap();
        let scope = repo.path().join(".dev/pipeline/criterion");
        fs::create_dir_all(&scope).unwrap();
        fs::write(
            scope.join("preflight.receipt.md"),
            "host: none\ndispatch_mode: routed\nrun_id: criterion-run\n",
        )
        .unwrap();
        let paths = BTreeSet::from(["product.txt".to_owned()]);
        let bindings = criteria
            .iter()
            .map(|criterion| CriterionBinding {
                criterion_id: (*criterion).into(),
                spec_sha256: digest_bytes(criterion.as_bytes()),
                membership_sha256: digest_bytes(b"membership"),
                artifact_set_sha256: digest_bytes(&serde_json::to_vec(&paths).unwrap()),
                artifact_snapshot_sha256: artifact_snapshot(repo.path(), &paths).unwrap().digest(),
            })
            .collect::<Vec<_>>();
        let content = json!({"bindings":bindings,"mappings":mappings});
        EvidenceStore::open(&repo.path().join(".git"))
            .unwrap()
            .publish(&EvidenceRecord {
                schema_version: 1,
                event_id: format!("membership-{task}"),
                operation_id: "freeze".into(),
                run_id: "criterion-run".into(),
                plan_generation: None,
                task_id: Some(task.clone()),
                attempt_id: None,
                parent_attempt_id: None,
                phase: "preflight".into(),
                source_kind: "criterion-membership".into(),
                source_version: "1".into(),
                recorded_at_unix_ms: 0,
                content_hash: digest_bytes(&serde_json::to_vec(&content).unwrap()),
                content,
            })
            .unwrap();
        let frozen = freeze_inputs(&repo.path().join(".git"), "criterion-run").unwrap();
        frozen.verify(&repo.path().join(".git")).unwrap();
        assert_eq!(frozen.evidence_ids.len(), 1);
        let config = home.path().join(".gal/config");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("config.json"), r#"{"executorRouting":{"executors":{"agy":"fixture-model"},"pipeline":{"TESTER":{"executor":"agy"},"AUDITOR":{"executor":"agy"}}}}"#).unwrap();
        Self {
            repo,
            home,
            prompt,
            task,
            bindings,
            mappings,
        }
    }

    fn run(&self, mode: &str, phase: &str) -> Output {
        let mut paths = vec![executor().to_owned()];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("GEMINI_API_KEY", "fixture-key")
            .env("RECEIPT_MODE", mode)
            .arg("pipeline")
            .arg(&self.prompt)
            .args(["--task", &self.task, "--phase", phase])
            .output()
            .unwrap()
    }

    fn phases(&self) -> Vec<EvidenceRecord> {
        let store = EvidenceStore::open(&self.repo.path().join(".git")).unwrap();
        store
            .list_page("criterion-run", None, 256)
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| store.read("criterion-run", &entry.record_id).unwrap())
            .filter(|record| record.source_kind == "phase")
            .collect()
    }

    fn status(&self, record: &EvidenceRecord, criterion: &str) -> CriterionStatus {
        let expected = self
            .mappings
            .iter()
            .filter(|row| row["criterion_id"] == criterion)
            .map(|row| CriterionMapping {
                tp_id: row["tp_id"].as_str().unwrap().into(),
                task_id: self.task.clone(),
                gate_id: "verification".into(),
            })
            .collect();
        let rows = record.content["criterion_results"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["criterion_id"] == criterion)
            .map(|row| CriterionRow {
                mapping: CriterionMapping {
                    tp_id: row["tp_id"].as_str().unwrap().into(),
                    task_id: self.task.clone(),
                    gate_id: "verification".into(),
                },
                status: serde_json::from_value(row["status"].clone()).unwrap(),
            })
            .collect::<Vec<_>>();
        criterion_status(&expected, &rows)
    }

    fn consumer(&self, command: &str) -> (Option<i32>, String) {
        let receipt = self.repo.path().join(".dev/pipeline/consumer.md");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_gal"));
        cmd.current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .arg(command)
            .arg(&self.prompt)
            .arg("--receipt")
            .arg(&receipt);
        if command == "pipeline-converge-check" {
            cmd.args(["--task", &self.task]);
        }
        let output = cmd.output().unwrap();
        assert!(
            receipt.exists(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        (output.status.code(), fs::read_to_string(receipt).unwrap())
    }
}

#[test]
fn every_test_plan_row_contributes_to_the_shared_criterion() {
    for (mode, expected) in [
        ("pass", CriterionStatus::Pass),
        ("fail", CriterionStatus::Fail),
        ("not-run", CriterionStatus::Unverified),
    ] {
        let fixture = Fixture::new(false);
        let output = fixture.run(mode, "test");
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let phases = fixture.phases();
        assert_eq!(phases.len(), 1);
        assert_eq!(fixture.status(&phases[0], "behavior"), expected);
        assert_eq!(
            phases[0].content["criterion_results"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
}

#[test]
fn malformed_combinations_and_changed_bindings_are_rejected_and_retained() {
    for mode in [
        "missing",
        "duplicate",
        "unknown",
        "binding",
        "extra",
        "empty",
        "source",
        "shrink",
    ] {
        let fixture = Fixture::new(false);
        let output = fixture.run(mode, "test");
        assert!(!output.status.success(), "{mode} was accepted");
        let phases = fixture.phases();
        assert_eq!(
            phases.len(),
            1,
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(phases[0].content["validation_error"].is_string(), "{mode}");
        assert_eq!(
            fixture.status(&phases[0], "behavior"),
            CriterionStatus::Unverified
        );
        assert!(!phases[0].content["raw_content"]
            .as_str()
            .unwrap()
            .is_empty());
    }
}

#[test]
fn independent_criteria_keep_first_results_and_supersede_after_artifact_changes() {
    let fixture = Fixture::new(true);
    let output = fixture.run("fail", "test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let original = fixture.phases();
    assert_eq!(
        fixture.status(&original[0], "behavior"),
        CriterionStatus::Fail
    );
    assert_eq!(
        fixture.status(&original[0], "integrity"),
        CriterionStatus::Pass
    );
    fs::write(fixture.repo.path().join("product.txt"), "changed\n").unwrap();
    for command in ["pipeline-converge-check", "pipeline-handback-check"] {
        let (_, receipt) = fixture.consumer(command);
        assert!(receipt.contains("\"status\":\"superseded\""), "{receipt}");
    }
    assert_eq!(original, fixture.phases());
    fs::write(fixture.repo.path().join("product.txt"), "original\n").unwrap();
    let (_, receipt) = fixture.consumer("pipeline-converge-check");
    assert!(receipt.contains("\"status\":\"superseded\""));
}

#[test]
fn artifact_changes_during_verification_cannot_yield_a_valid_pass() {
    let fixture = Fixture::new(false);
    let output = fixture.run("artifact-change", "test");
    assert!(!output.status.success());
    let phases = fixture.phases();
    assert_eq!(phases.len(), 1);
    for phase in phases {
        assert!(phase.content["raw_content"]
            .as_str()
            .unwrap()
            .contains("Pipeline Evidence:"));
        assert_ne!(fixture.status(&phase, "behavior"), CriterionStatus::Pass);
    }
    let (_, receipt) = fixture.consumer("pipeline-converge-check");
    assert!(!receipt.contains("\"status\":\"pass\""));
}

#[test]
fn frozen_artifact_sets_cannot_shrink_and_non_test_mappings_are_not_test_obligations() {
    let fixture = Fixture::new(true);
    let text = fs::read_to_string(&fixture.prompt).unwrap();
    let text = text
        .lines()
        .map(|line| {
            if !line.contains("\"criterion_id\":\"integrity\"") {
                return line.to_owned();
            }
            let mut cells = line
                .trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect::<Vec<_>>();
            let mut declarations: Value = serde_json::from_str(&cells[4]).unwrap();
            for declaration in declarations
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .filter(|declaration| declaration["criterion_id"] == "integrity")
            {
                declaration["source_kind"] = json!("owner-acceptance");
                declaration["target"] = json!(format!("OA-{:02}", 1));
            }
            cells[4] = declarations.to_string();
            format!("| {} |", cells.join(" | "))
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&fixture.prompt, &text).unwrap();
    let output = fixture.run("pass", "test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let phases = fixture.phases();
    assert_eq!(
        fixture.status(&phases[0], "behavior"),
        CriterionStatus::Pass
    );
    assert_eq!(
        fixture.status(&phases[0], "integrity"),
        CriterionStatus::Unverified
    );
    fs::write(
        &fixture.prompt,
        text.replace("`product.txt`", "`smaller.txt`"),
    )
    .unwrap();
    let output = fixture.run("pass", "test");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("artifact set does not match frozen binding"));
    assert_eq!(fixture.phases(), phases);
}

#[test]
fn missing_and_empty_membership_preserve_version_one_dispatch() {
    for run in ["no-membership", "empty-membership"] {
        let fixture = Fixture::new(false);
        fs::write(
            fixture
                .repo
                .path()
                .join(".dev/pipeline/criterion/preflight.receipt.md"),
            format!("host: none\ndispatch_mode: routed\nrun_id: {run}\n"),
        )
        .unwrap();
        let store = EvidenceStore::open(&fixture.repo.path().join(".git")).unwrap();
        if run == "empty-membership" {
            let content = json!({"bindings":[],"mappings":[]});
            store
                .publish(&EvidenceRecord {
                    schema_version: 1,
                    event_id: format!("membership-{}", fixture.task),
                    operation_id: "freeze".into(),
                    run_id: run.into(),
                    plan_generation: None,
                    task_id: Some(fixture.task.clone()),
                    attempt_id: None,
                    parent_attempt_id: None,
                    phase: "preflight".into(),
                    source_kind: "criterion-membership".into(),
                    source_version: "1".into(),
                    recorded_at_unix_ms: 0,
                    content_hash: digest_bytes(&serde_json::to_vec(&content).unwrap()),
                    content,
                })
                .unwrap();
        }
        freeze_inputs(&fixture.repo.path().join(".git"), run)
            .unwrap()
            .verify(&fixture.repo.path().join(".git"))
            .unwrap();
        for phase in ["test", "audit"] {
            let output = fixture.run("pass", phase);
            assert!(
                output.status.success(),
                "{run}/{phase}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let phases = store
            .list_page(run, None, 256)
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| store.read(run, &entry.record_id).unwrap())
            .filter(|record| record.source_kind == "phase")
            .collect::<Vec<_>>();
        assert_eq!(phases.len(), 2);
        assert!(phases.iter().all(|record| record.source_version == "1"
            && record.content["criterion_results"] == json!([])));
    }
}

#[test]
fn criterion_facts_do_not_change_consumer_decisions_and_legacy_receipts_cannot_pass_criteria() {
    let fixture = Fixture::new(false);
    let baseline = [
        fixture.consumer("pipeline-converge-check"),
        fixture.consumer("pipeline-handback-check"),
    ];
    let output = fixture.run("fail", "test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for (command, before) in ["pipeline-converge-check", "pipeline-handback-check"]
        .into_iter()
        .zip(baseline)
    {
        let after = fixture.consumer(command);
        assert_eq!(before.0, after.0);
        let authority = |text: String| {
            text.lines()
                .filter(|line| {
                    line.starts_with("overall:")
                        || line.starts_with("decision:")
                        || line.starts_with("voluntary_response_authorized:")
                })
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        assert_eq!(authority(before.1), authority(after.1));
    }
    let output = fixture.run("historical", "test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let phases = fixture.phases();
    let historical = phases
        .iter()
        .find(|record| record.source_version == "1")
        .unwrap();
    assert_eq!(
        fixture.status(historical, "behavior"),
        CriterionStatus::Unverified
    );
    assert_eq!(fixture.bindings.len(), 1);
}

struct NonTestFixture {
    repo: TempDir,
    prompt: PathBuf,
    source: String,
    binding: CriterionBinding,
    payload: pipeline::report::CriterionPayload,
    operation: String,
    evidence_hash: String,
}

impl NonTestFixture {
    fn new(source: &str, test_type: &str) -> Self {
        Self::with_membership(source, test_type, true)
    }

    fn with_membership(source: &str, test_type: &str, legacy_membership: bool) -> Self {
        use pipeline::report::{
            CriterionDeclaration, CriterionPayload, CriterionSource, CriterionTestPlan,
        };
        let repo = TempDir::new().unwrap();
        git(repo.path(), &["init", "-q"]);
        git(repo.path(), &["config", "user.name", "Fixture"]);
        git(
            repo.path(),
            &["config", "user.email", "fixture@example.com"],
        );
        git(repo.path(), &["config", "commit.gpgsign", "false"]);
        git(repo.path(), &["config", "core.hooksPath", ".git/hooks"]);
        fs::write(repo.path().join("product.txt"), "original\n").unwrap();
        git(repo.path(), &["add", "."]);
        git(repo.path(), &["commit", "-qm", "fixture"]);
        let task = format!("T-{:02}", 1);
        let tp = format!("TP-{:02}", 1);
        let owner = format!("OA-{:02}", 1);
        let declaration = CriterionDeclaration {
            criterion_id: "behavior".into(),
            requirements: BTreeSet::from(["R1".into()]),
            gate_id: "verification".into(),
            pass_condition: "assertion holds".into(),
            source_kind: serde_json::from_value(json!(source)).unwrap(),
            target: match source {
                "owner-acceptance" => Some(owner.clone()),
                "goal-verification" => Some("goal".into()),
                _ => None,
            },
            security: None,
        };
        assert_ne!(declaration.source_kind, CriterionSource::Test);
        let payload = CriterionPayload {
            schema_version: 1,
            declaration: declaration.clone(),
            test_plans: BTreeSet::from([CriterionTestPlan {
                id: tp.clone(),
                test_type: test_type.into(),
                covers: BTreeSet::from([task.clone()]),
            }]),
            mappings: BTreeSet::from([CriterionMapping {
                task_id: task.clone(),
                tp_id: tp.clone(),
                gate_id: "verification".into(),
            }]),
        };
        let plans = repo.path().join(".dev/plans");
        fs::create_dir_all(&plans).unwrap();
        let prompt = plans.join("registration.prompt.md");
        fs::write(plans.join("registration.md"), "# Source\n\n## Goal\n\nObservable behavior.\n\n## Success Criteria\n\n- Assertion holds.\n").unwrap();
        fs::write(&prompt, format!("# Registration\n\n## Requirements\n\n- [ ] R1 behavior\n\n## Tasks\n\n- [ ] {task} behavior\n  - **Files**: `product.txt`\n  - **Change**: Verify behavior.\n  - **Acceptance**: Assertion holds.\n\n## Status\n\nCurrent Task: {task}\n\n### Handoff Notes\n\n#### Owner Acceptance Evidence\n\n| ID | Outcome | Owner evidence | Target binding |\n| --- | --- | --- | --- |\n| {owner} | accepted | Owner confirmed the observed output. | behavior product version one |\n\n## Owner Acceptance\n\n| ID | What to check | Expected result | Pass/fail rule |\n| --- | --- | --- | --- |\n| {owner} | Inspect behavior | Assertion holds | All checks pass |\n\n## Test Plan\n\n| ID | Type | Description | Covers | Criteria |\n| --- | --- | --- | --- | --- |\n| {tp} | {test_type} | Behavior | {task} | {} |\n", json!([declaration]))).unwrap();
        let scope = repo.path().join(".dev/pipeline/registration");
        fs::create_dir_all(&scope).unwrap();
        fs::write(
            scope.join("preflight.receipt.md"),
            "host: none\ndispatch_mode: routed\nrun_id: registration-run\n",
        )
        .unwrap();
        let paths = BTreeSet::from(["product.txt".to_owned()]);
        let binding = CriterionBinding {
            criterion_id: "behavior".into(),
            spec_sha256: payload.spec_sha256().unwrap(),
            membership_sha256: digest_bytes(b"membership"),
            artifact_set_sha256: digest_bytes(&serde_json::to_vec(&paths).unwrap()),
            artifact_snapshot_sha256: artifact_snapshot(repo.path(), &paths).unwrap().digest(),
        };
        let mut fixture = Self {
            repo,
            prompt,
            source: source.into(),
            binding,
            payload,
            operation: String::new(),
            evidence_hash: String::new(),
        };
        if legacy_membership {
            fixture.publish(&format!("membership-{task}"), "freeze", "criterion-membership", json!({"bindings":[fixture.binding],"mappings":[{"tp_id":tp,"criterion_id":"behavior","task_id":task,"gate_id":"verification","source_kind":source}]}));
        }
        let output = fixture.cli(&[
            "usage",
            "begin",
            "--phase",
            "finalize",
            "--run",
            "registration-run",
            "--worktree",
            fixture.repo.path().to_str().unwrap(),
            "--host",
            "fixture",
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fixture.operation = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix("operation_id: "))
            .unwrap()
            .into();
        fixture.evidence_hash = fixture.publish("source-evidence", &fixture.operation, "command-output", json!({"command":"inspect product.txt","output":"assertion holds","owner_evidence":"Owner confirmed the observed output."}));
        fixture
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(self.repo.path())
            .args(args)
            .output()
            .unwrap()
    }

    fn publish(&self, id: &str, operation: &str, kind: &str, content: Value) -> String {
        let record = EvidenceRecord {
            schema_version: 1,
            event_id: id.into(),
            operation_id: operation.into(),
            run_id: "registration-run".into(),
            plan_generation: None,
            task_id: (kind == "criterion-membership").then(|| format!("T-{:02}", 1)),
            attempt_id: None,
            parent_attempt_id: None,
            phase: "verification".into(),
            source_kind: kind.into(),
            source_version: "1".into(),
            recorded_at_unix_ms: 0,
            content_hash: digest_bytes(&serde_json::to_vec(&content).unwrap()),
            content,
        };
        EvidenceStore::open(&self.repo.path().join(".git"))
            .unwrap()
            .publish(&record)
            .unwrap();
        pipeline::usage::store::record_digest(&record).unwrap()
    }

    fn target(&self) -> Value {
        match self.source.as_str() {
            "goal-verification" => json!(digest_bytes(&serde_json::to_vec(&json!({"goal":"Observable behavior.","success_criteria":"- Assertion holds.","criteria":{"behavior":self.payload.spec_sha256().unwrap()}})).unwrap())),
            "owner-acceptance" => json!(digest_bytes(&serde_json::to_vec(&json!({"fields":{"What to check":"Inspect behavior","Expected result":"Assertion holds","Pass/fail rule":"All checks pass"},"target":"behavior product version one","artifact_snapshot_sha256":self.binding.artifact_snapshot_sha256})).unwrap())),
            _ => Value::Null,
        }
    }

    fn input(&self, outcome: &str) -> Value {
        json!({"schema_version":1,"run_id":"registration-run","prompt_path":self.prompt,
            "task_id":format!("T-{:02}",1),"tp_id":format!("TP-{:02}",1),"gate_id":"verification",
            "source_kind":self.source,"binding":self.binding,"outcome":outcome,
            "evidence_refs":[{"evidence_id":"source-evidence","sha256":self.evidence_hash}],
            "source_operation_id":self.operation,"source_evidence_sha256":self.evidence_hash,
            "observed_head_sha":git(self.repo.path(), &["rev-parse","HEAD"]),
            "goal_spec_sha256":if self.source == "goal-verification" {self.target()} else {Value::Null},
            "owner_acceptance_id":if self.source == "owner-acceptance" {json!(format!("OA-{:02}",1))} else {Value::Null},
            "owner_target_sha256":if self.source == "owner-acceptance" {self.target()} else {Value::Null}})
    }

    fn register(&self, input: &Value) -> Output {
        let path = self.repo.path().join("result.json");
        fs::write(&path, serde_json::to_vec(input).unwrap()).unwrap();
        self.cli(&[
            "usage",
            "criterion-result",
            "--input",
            path.to_str().unwrap(),
        ])
    }

    fn records(&self) -> Vec<EvidenceRecord> {
        let store = EvidenceStore::open(&self.repo.path().join(".git")).unwrap();
        store
            .list_page("registration-run", None, 256)
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| store.read("registration-run", &entry.record_id).unwrap())
            .collect()
    }

    fn handback(&self) -> String {
        let receipt = self.repo.path().join("handback.md");
        self.cli(&[
            "pipeline-handback-check",
            self.prompt.to_str().unwrap(),
            "--receipt",
            receipt.to_str().unwrap(),
        ]);
        fs::read_to_string(receipt).unwrap()
    }

    fn status(&self) -> String {
        let receipt = self.handback();
        let evidence = receipt
            .lines()
            .find_map(|line| line.strip_prefix("Criterion Evidence: "))
            .unwrap();
        let evidence: Value = serde_json::from_str(evidence).unwrap();
        evidence["criteria"][0]["status"].as_str().unwrap().into()
    }
}

#[test]
fn non_test_sources_round_trip_and_invalid_registration_leaves_records_unchanged() {
    for (source, outcome) in [
        ("orchestrator-check", "pass"),
        ("goal-verification", "pass"),
        ("owner-acceptance", "accepted"),
    ] {
        let fixture = NonTestFixture::new(source, "manual");
        let input = fixture.input(outcome);
        let output = fixture.register(&input);
        assert!(
            output.status.success(),
            "{source}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["status"], "pass", "{source}: {result}");
        assert_eq!(fixture.status(), "pass");
        let records = fixture.records();
        let stored = records
            .iter()
            .find(|record| record.event_id == result["record_id"])
            .unwrap();
        for (key, value) in input.as_object().unwrap() {
            assert_eq!(&stored.content[key], value, "{key}");
        }
        for invalid in [
            "source", "missing", "binding", "mapping", "hash", "extra", "target",
        ] {
            let mut input = input.clone();
            match invalid {
                "source" => input["source_kind"] = json!("test"),
                "missing" => {
                    input.as_object_mut().unwrap().remove("observed_head_sha");
                }
                "binding" => input["binding"]["membership_sha256"] = json!(digest_bytes(b"wrong")),
                "mapping" => input["gate_id"] = json!("wrong"),
                "hash" => input["evidence_refs"][0]["sha256"] = json!(digest_bytes(b"wrong")),
                "extra" => input["extra"] = json!(true),
                _ => input["owner_acceptance_id"] = json!(format!("OA-{:02}", 9)),
            }
            let output = fixture.register(&input);
            assert_eq!(
                output.status.code(),
                Some(64),
                "{invalid}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(fixture.records(), records, "{invalid}");
        }
    }
}

#[test]
fn orchestrator_only_rows_require_evidence_and_do_not_create_test_attempts() {
    for test_type in ["grep", "manual", "documentation"] {
        let fixture = NonTestFixture::new("orchestrator-check", test_type);
        assert_eq!(fixture.status(), "unverified");
        let mut input = fixture.input("pass");
        input["evidence_refs"] = json!([]);
        assert_eq!(fixture.register(&input).status.code(), Some(64));
        assert_eq!(fixture.status(), "unverified");
        assert!(fixture.register(&fixture.input("pass")).status.success());
        assert_eq!(fixture.status(), "pass");
        assert!(fixture
            .records()
            .iter()
            .all(|record| record.attempt_id.is_none()
                && record.source_kind != "test"
                && record.source_kind != "phase"));
    }
}

#[test]
fn owner_acceptance_waivers_pending_and_changed_targets_never_pass() {
    let fixture = NonTestFixture::new("owner-acceptance", "manual");
    assert!(fixture
        .register(&fixture.input("accepted"))
        .status
        .success());
    assert_eq!(fixture.status(), "pass");
    for outcome in ["waived", "pending"] {
        assert!(fixture.register(&fixture.input(outcome)).status.success());
        assert_eq!(fixture.status(), "unverified");
    }
    assert!(fixture
        .register(&fixture.input("accepted"))
        .status
        .success());
    assert_eq!(fixture.status(), "pass");
    let original = fs::read_to_string(&fixture.prompt).unwrap();
    fs::write(
        &fixture.prompt,
        original.replace("Inspect behavior", "Inspect revised behavior"),
    )
    .unwrap();
    assert_eq!(fixture.status(), "superseded");
    fs::write(&fixture.prompt, &original).unwrap();
    assert_eq!(fixture.status(), "superseded");
}

#[test]
fn goal_spec_changes_invalidate_results_without_granting_handback_authority() {
    let fixture = NonTestFixture::new("goal-verification", "manual");
    let authority = |receipt: &str| {
        receipt
            .lines()
            .filter(|line| {
                line.starts_with("decision:")
                    || line.starts_with("voluntary_response_authorized:")
                    || line.starts_with("continue_action:")
            })
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let baseline = authority(&fixture.handback());
    let output = fixture.register(&fixture.input("pass"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fixture.status(), "pass");
    assert_eq!(authority(&fixture.handback()), baseline);
    let source = fixture.prompt.with_file_name("registration.md");
    let original = fs::read_to_string(&source).unwrap();
    fs::write(
        &source,
        original.replace("Observable behavior.", "Changed goal."),
    )
    .unwrap();
    assert_eq!(fixture.status(), "superseded");
    fs::write(&source, &original).unwrap();
    assert_eq!(fixture.status(), "superseded");
}

#[test]
fn unobservable_targets_require_a_reason_and_remain_unknown() {
    for (source, field, outcome) in [
        ("goal-verification", "goal_spec_sha256", "pass"),
        ("owner-acceptance", "owner_target_sha256", "accepted"),
    ] {
        let fixture = NonTestFixture::new(source, "manual");
        let mut input = fixture.input(outcome);
        input[field] = Value::Null;
        let before = fixture.records();
        assert_eq!(fixture.register(&input).status.code(), Some(64));
        assert_eq!(fixture.records(), before);
        input["target_reason"] = json!("target unavailable");
        let output = fixture.register(&input);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap()["status"],
            "unknown"
        );
        assert_eq!(fixture.status(), "unverified");
    }
}

#[test]
fn skill_templates_register_each_non_test_source_at_its_producer() {
    let pipeline =
        include_str!("../../../plugins/gal-core/commands/gal-pipeline/SKILL.template.md");
    let finalize =
        include_str!("../../../plugins/gal-core/commands/gal-finalize/SKILL.template.md");
    let command = "gal usage criterion-result --input";
    assert!(pipeline.split("### 2e").next().unwrap().contains(command));
    assert!(pipeline
        .split("## Step 3 — Orchestrator Goal-Backward Verification")
        .nth(1)
        .unwrap()
        .contains(command));
    assert!(finalize
        .split("**Owner Acceptance check")
        .nth(1)
        .unwrap()
        .contains(command));
}

#[test]
fn operation_sequence_wins_over_late_registration_from_an_earlier_operation() {
    let fixture = NonTestFixture::new("orchestrator-check", "manual");
    let old_input = fixture.input("fail");
    assert!(fixture.register(&old_input).status.success());
    let ended = fixture.cli(&[
        "usage",
        "end",
        "--operation",
        &fixture.operation,
        "--status",
        "completed",
    ]);
    assert!(
        ended.status.success(),
        "{}",
        String::from_utf8_lossy(&ended.stderr)
    );
    let output = fixture.cli(&[
        "usage",
        "begin",
        "--phase",
        "finalize",
        "--run",
        "registration-run",
        "--worktree",
        fixture.repo.path().to_str().unwrap(),
        "--host",
        "fixture",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let operation = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("operation_id: "))
        .unwrap()
        .to_owned();
    assert_ne!(operation, fixture.operation);
    let hash = fixture.publish(
        "new-source-evidence",
        &operation,
        "command-output",
        json!({"command":"inspect product.txt","output":"assertion holds"}),
    );
    let mut new_input = fixture.input("pass");
    new_input["source_operation_id"] = json!(operation);
    new_input["source_evidence_sha256"] = json!(hash);
    new_input["evidence_refs"] = json!([{"evidence_id":"new-source-evidence","sha256":hash}]);
    assert!(fixture.register(&new_input).status.success());
    assert_eq!(fixture.status(), "pass");
    assert!(fixture.register(&old_input).status.success());
    assert_eq!(fixture.status(), "pass");
    new_input["outcome"] = json!("fail");
    assert!(fixture.register(&new_input).status.success());
    assert_eq!(fixture.status(), "fail");
    new_input["outcome"] = json!("pass");
    assert!(fixture.register(&new_input).status.success());
    assert_eq!(fixture.status(), "pass");
}

#[test]
fn changed_common_bindings_and_mismatched_target_digests_cannot_pass() {
    for source in [
        "orchestrator-check",
        "goal-verification",
        "owner-acceptance",
    ] {
        let fixture = NonTestFixture::new(source, "manual");
        let mut input = fixture.input(if source == "owner-acceptance" {
            "accepted"
        } else {
            "pass"
        });
        if source != "orchestrator-check" {
            let field = if source == "owner-acceptance" {
                "owner_target_sha256"
            } else {
                "goal_spec_sha256"
            };
            input[field] = json!(digest_bytes(b"wrong target"));
            let output = fixture.register(&input);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap()["status"],
                "superseded"
            );
            assert_eq!(fixture.status(), "superseded");
        }
        assert!(fixture
            .register(&fixture.input(if source == "owner-acceptance" {
                "accepted"
            } else {
                "pass"
            }))
            .status
            .success());
        assert_eq!(fixture.status(), "pass");
        let original = fs::read_to_string(&fixture.prompt).unwrap();
        fs::write(
            &fixture.prompt,
            original.replace("assertion holds", "different assertion"),
        )
        .unwrap();
        assert_eq!(fixture.status(), "superseded");
    }
}

#[test]
fn accepted_results_require_owner_evidence_and_ignore_unrelated_head_changes() {
    let fixture = NonTestFixture::new("owner-acceptance", "manual");
    assert!(fixture
        .register(&fixture.input("accepted"))
        .status
        .success());
    assert_eq!(fixture.status(), "pass");
    fs::write(fixture.repo.path().join("unrelated.txt"), "metadata\n").unwrap();
    git(fixture.repo.path(), &["add", "unrelated.txt"]);
    git(
        fixture.repo.path(),
        &["commit", "-qm", "unrelated metadata"],
    );
    assert_eq!(fixture.status(), "pass");
    let text = fs::read_to_string(&fixture.prompt).unwrap();
    fs::write(&fixture.prompt, text.replace("| accepted |", "| waived |")).unwrap();
    assert_eq!(fixture.status(), "unverified");
    fs::write(
        &fixture.prompt,
        text.replace("Owner confirmed the observed output.", ""),
    )
    .unwrap();
    assert_eq!(fixture.status(), "unverified");
}

#[test]
fn unknown_or_conflicting_operation_order_never_selects_a_pass() {
    for known_order in [false, true] {
        let fixture = NonTestFixture::new("orchestrator-check", "manual");
        assert!(fixture.register(&fixture.input("pass")).status.success());
        assert_eq!(fixture.status(), "pass");
        let original = fixture
            .records()
            .into_iter()
            .find(|record| record.source_kind == "orchestrator-check")
            .unwrap();
        let mut content = original.content.clone();
        if !known_order {
            content.as_object_mut().unwrap().remove("result_sequence");
        }
        fixture.publish(
            "conflicting-result",
            &fixture.operation,
            "orchestrator-check",
            content,
        );
        assert_eq!(fixture.status(), "unverified");
        assert!(fixture.register(&fixture.input("pass")).status.success());
        assert_eq!(
            fixture.status(),
            if known_order { "pass" } else { "unverified" }
        );
    }
}

#[test]
fn manifest_bound_registration_retains_membership_for_the_consumer() {
    let mut fixture = NonTestFixture::with_membership("owner-acceptance", "manual", false);
    let contract = json!({"schema_version":1,"criteria":{"behavior":fixture.payload}});
    fixture.binding.membership_sha256 = digest_bytes(&serde_json::to_vec(&contract).unwrap());
    let source = fixture.prompt.with_file_name("registration.md");
    let content = json!({"schema_version":1,"contract":contract,"membership_sha256":fixture.binding.membership_sha256,
        "files":{format!("T-{:02}",1):["product.txt"]},"source_path":source,
        "source_sha256":digest_bytes(&fs::read(&source).unwrap())});
    let manifest = digest_bytes(&serde_json::to_vec(&content).unwrap());
    fixture.publish(
        &format!("criterion-manifest-{manifest}"),
        "freeze",
        "criterion-manifest",
        content,
    );
    fs::write(fixture.repo.path().join(".dev/pipeline/registration/preflight.receipt.md"),
        format!("host: none\ndispatch_mode: routed\nrun_id: registration-run\ncriterion_manifest_sha256: {manifest}\n")).unwrap();
    let output = fixture.register(&fixture.input("accepted"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fixture.status(), "pass");
    fs::write(fixture.repo.path().join("product.txt"), "revised\n").unwrap();
    assert_eq!(fixture.status(), "superseded");
    fixture.binding.artifact_snapshot_sha256 = artifact_snapshot(
        fixture.repo.path(),
        &BTreeSet::from(["product.txt".to_owned()]),
    )
    .unwrap()
    .digest();
    let target = digest_bytes(&serde_json::to_vec(&json!({"fields":{"What to check":"Inspect behavior","Expected result":"Assertion holds","Pass/fail rule":"All checks pass"},"target":"behavior product version one","artifact_snapshot_sha256":fixture.binding.artifact_snapshot_sha256})).unwrap());
    let mut input = fixture.input("accepted");
    input["owner_target_sha256"] = json!(target);
    let output = fixture.register(&input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["status"],
        "pass"
    );
    assert!(fixture.handback().contains("\"status\":\"pass\""));
}
