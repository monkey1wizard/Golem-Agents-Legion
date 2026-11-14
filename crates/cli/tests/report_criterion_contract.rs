use pipeline::report::{CriterionPayload, EvidenceStore};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path, process::Command};
use tempfile::TempDir;

fn task(number: u32) -> String {
    format!("T-{number:02}")
}

fn test_id(number: u32) -> String {
    format!("TP-{number:02}")
}

fn declaration(id: &str) -> Value {
    json!({"criterion_id":id,"requirements":["R1"],"gate_id":"verification","pass_condition":"值含有 | \\ 與 \" 引號","source_kind":"test","target":null,"security":null})
}

fn cell(declarations: Vec<Value>) -> String {
    serde_json::to_string(&declarations)
        .unwrap()
        .replace('|', "\\u007c")
}

fn row(number: u32, covers: &str, criteria: &str) -> String {
    format!(
        "| {} | integration | Behavior | {covers} | {criteria} |\n",
        test_id(number)
    )
}

fn plan(rows: &str, count: u32) -> String {
    let tasks = (1..=count).map(|number| format!("- [ ] {} behavior\n  - **Verification Impact**: `{{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false}}`\n  - **Files**: `product.json`\n  - **Change**: Verify behavior.\n  - **Acceptance**: Assertion holds.\n", task(number))).collect::<String>();
    let map = (1..=count)
        .map(|number| {
            format!(
                "| {} | unit | `cargo test` | pass | TESTER | output |\n",
                task(number)
            )
        })
        .collect::<String>();
    format!("# Contract\n\n## Requirements\n\n- [ ] R1 behavior\n\n## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {}\n\n## Tasks\n\n{tasks}\n## Test Plan\n\n| ID | Type | Description | Covers | Criteria |\n| --- | --- | --- | --- | --- |\n{rows}\n## Verification Map\n\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n{map}\n## Test Results\n\n## Review Results\n\n### Architecture Review\nCLEAR\n### Business Review\nNot requested.\n### Design Review\nNot requested.\n### Engineering Review\nCLEAR\n\n<!-- ENG_REVIEW: CLEAR -->\n", task(1))
}

struct Fixture {
    repo: TempDir,
    home: TempDir,
}

impl Fixture {
    fn new(text: &str) -> Self {
        let fixture = Self {
            repo: TempDir::new().unwrap(),
            home: TempDir::new().unwrap(),
        };
        fixture.git(&["init", "-q"]);
        for args in [
            ["config", "user.name", "Fixture"],
            ["config", "user.email", "fixture@example.com"],
            ["config", "commit.gpgsign", "false"],
            ["config", "core.hooksPath", ".git/hooks"],
            ["config", "core.autocrlf", "false"],
        ] {
            fixture.git(&args);
        }
        fs::write(fixture.repo.path().join(".gitignore"), ".dev/\n").unwrap();
        fs::write(fixture.repo.path().join("product.json"), "behavior\n").unwrap();
        fs::create_dir_all(fixture.repo.path().join(".dev/plans")).unwrap();
        fs::write(
            fixture.repo.path().join(".dev/state.md"),
            "| contract | .dev/plans/contract.prompt.md | active |\n",
        )
        .unwrap();
        fixture.write(text, text);
        fixture.git(&["add", "."]);
        fixture.git(&["commit", "-qm", "fixture"]);
        fixture
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(self.repo.path())
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write(&self, source: &str, prompt: &str) {
        fs::write(self.repo.path().join(".dev/plans/contract.md"), source).unwrap();
        fs::write(
            self.repo.path().join(".dev/plans/contract.prompt.md"),
            prompt,
        )
        .unwrap();
    }

    fn run(&self, command: &str, terminal: bool) -> (bool, String) {
        let source = if command == "refining-check" {
            ".dev/plans/contract.md"
        } else {
            ".dev/plans/contract.prompt.md"
        };
        let mut process = Command::new(env!("CARGO_BIN_EXE_gal"));
        process
            .current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("GAL_ENTRY_RECOVERY")
            .args([command, source]);
        if terminal {
            process.arg("--terminal-reverify");
        }
        let output = process.output().unwrap();
        let name = match command {
            "refining-check" => "refining-check.receipt.md",
            "prompt-check" => "prompt-check.receipt.md",
            _ if terminal => "terminal-reverify.receipt.md",
            _ => "preflight.receipt.md",
        };
        let receipt =
            fs::read_to_string(self.repo.path().join(".dev/pipeline/contract").join(name))
                .unwrap_or_default();
        assert!(
            !receipt.is_empty(),
            "{command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        (output.status.success(), receipt)
    }

    fn freeze(&self) -> (String, Value) {
        let (passed, receipt) = self.run("pipeline-preflight", false);
        assert!(passed, "{receipt}");
        let field = |name: &str| {
            receipt
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .unwrap()
                .to_owned()
        };
        let run = field("run_id: ");
        let digest = field("criterion_manifest_sha256: ");
        let record = EvidenceStore::open(&self.repo.path().join(".git"))
            .unwrap()
            .read(&run, &format!("criterion-manifest-{digest}"))
            .unwrap();
        assert_eq!(
            pipeline::usage::digest_bytes(&serde_json::to_vec(&record.content).unwrap()),
            digest
        );
        (run, record.content)
    }
}

fn valid_plan() -> String {
    plan(&row(1, &task(1), &cell(vec![declaration("behavior")])), 1)
}

fn check_row<'a>(receipt: &'a str, name: &str) -> &'a str {
    receipt
        .lines()
        .find(|line| line.starts_with(&format!("| {name} |")))
        .unwrap_or_else(|| panic!("missing {name}: {receipt}"))
}

#[test]
fn planning_chain_freezes_decoded_declarations_and_immutable_mappings() {
    let text = valid_plan();
    let fixture = Fixture::new(&text);
    for command in ["refining-check", "prompt-check"] {
        let (passed, receipt) = fixture.run(command, false);
        assert!(passed, "{receipt}");
    }
    let (_, manifest) = fixture.freeze();
    let criteria: BTreeMap<String, CriterionPayload> =
        serde_json::from_value(manifest["contract"]["criteria"].clone()).unwrap();
    assert_eq!(
        criteria["behavior"].declaration.pass_condition,
        declaration("behavior")["pass_condition"].as_str().unwrap()
    );
    assert_eq!(criteria["behavior"].mappings.len(), 1);
    assert_eq!(manifest["files"][task(1)], json!(["product.json"]));
}

#[test]
fn format_repair_preserves_criteria_and_table_consumers() {
    let source = valid_plan();
    let handoff = format!(
        "### Handoff Notes\n\n#### Retry Handoff — {} / IMPLEMENT\nstatus : OPEN\nProblem: Verify behavior.\nNext human step: Run verification.\n\n",
        task(1)
    );
    let prompt = source.replace("## Tasks", &(handoff + "## Tasks"));
    let fixture = Fixture::new(&source);
    fixture.write(&source, &prompt);
    fs::write(fixture.repo.path().join(".gitignore"), ".dev/pipeline/\n").unwrap();
    fixture.git(&["add", "."]);
    fixture.git(&["commit", "-qm", "repair fixture"]);
    let (passed, receipt) = fixture.run("pipeline-preflight", false);
    assert!(!passed, "{receipt}");
    assert!(
        receipt.contains("- retry-handoff-shape: format (repair_id: retry-field-shape,"),
        "{receipt}"
    );
    let attempt = receipt
        .lines()
        .find_map(|line| line.strip_prefix("gate_attempt_id: "))
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(fixture.repo.path())
        .env("HOME", fixture.home.path())
        .env("USERPROFILE", fixture.home.path())
        .args([
            "format-repair",
            "--target",
            ".dev/plans/contract.prompt.md",
            "--receipt",
            ".dev/pipeline/contract/preflight.receipt.md",
            "--repair-id",
            "retry-field-shape",
            "--gate",
            "preflight",
            "--mode",
            "ordinary",
            "--attempt",
            attempt,
            "--round",
            "0",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let repaired =
        fs::read_to_string(fixture.repo.path().join(".dev/plans/contract.prompt.md")).unwrap();
    let table = |text: &str| {
        text.split("## Test Plan\n")
            .nth(1)
            .unwrap()
            .split("## Verification Map")
            .next()
            .unwrap()
            .to_owned()
    };
    assert_eq!(table(&prompt).as_bytes(), table(&repaired).as_bytes());
    assert!(repaired.contains("- Status: OPEN"));
    let (passed, receipt) = fixture.run("pipeline-preflight", false);
    assert!(passed, "{receipt}");
}

#[test]
fn malformed_declarations_and_references_refuse_with_locations() {
    let valid = cell(vec![declaration("behavior")]);
    let mut bad_security = declaration("behavior");
    bad_security["security"] = json!({"rule_or_threat":"","pass_condition":"holds"});
    let mut bad_owner = declaration("behavior");
    bad_owner["source_kind"] = json!("owner-acceptance");
    bad_owner["target"] = json!("OA-99");
    let cases = vec![
        "[]".to_owned(),
        "[{\"criterion_id\":\"behavior\"}]".to_owned(),
        valid.replace("\"target\":null", "\"target\":null,\"target\":null"),
        valid.replace("\"security\":null", "\"security\":null,\"unknown\":true"),
        valid.replace("R1", "R99"),
        cell(vec![bad_security]),
        cell(vec![bad_owner]),
        cell(vec![declaration("behavior"), declaration("behavior")]),
        valid.replace("\\u007c", "|"),
    ];
    let fixture = Fixture::new(&valid_plan());
    for invalid in cases {
        let text = plan(&row(1, &task(1), &invalid), 1);
        fixture.write(&text, &text);
        let (passed, receipt) = fixture.run("refining-check", false);
        assert!(!passed, "{invalid}");
        let check = check_row(&receipt, "criterion-contract");
        assert!(
            check.contains("| fail |") && check.contains("line "),
            "{check}"
        );
    }
    for covers in [
        "invalid".to_owned(),
        task(99),
        format!("{}, {}", task(1), task(1)),
    ] {
        let text = plan(&row(1, &covers, &valid), 1);
        fixture.write(&text, &text);
        let (_, receipt) = fixture.run("refining-check", false);
        assert!(check_row(&receipt, "criterion-contract").contains("| fail |"));
    }
    let text = plan(
        &row(1, &task(1), &valid).replace(&test_id(1), "TP-invalid"),
        1,
    );
    fixture.write(&text, &text);
    let (_, receipt) = fixture.run("refining-check", false);
    assert!(check_row(&receipt, "criterion-contract").contains("| fail |"));
}

#[test]
fn shared_criteria_union_rows_and_tasks_and_reject_conflicts() {
    let declarations = cell(vec![declaration("behavior"), declaration("integrity")]);
    let covers = format!("{}..{}", task(1), task(2));
    let rows = row(1, &covers, &declarations) + &row(2, &covers, &declarations);
    let text = plan(&rows, 2);
    let fixture = Fixture::new(&text);
    let (_, manifest) = fixture.freeze();
    let criteria = manifest["contract"]["criteria"].as_object().unwrap();
    assert_eq!(criteria.len(), 2);
    assert_eq!(
        criteria["behavior"]["mappings"].as_array().unwrap().len(),
        4
    );
    for rows in [
        rows.clone() + &row(1, &covers, &declarations),
        row(1, &covers, &declarations)
            + &row(2, &covers, &declarations.replace("值含有", "changed")),
    ] {
        let text = plan(&rows, 2);
        fixture.write(&text, &text);
        let (_, receipt) = fixture.run("refining-check", false);
        assert!(check_row(&receipt, "criterion-contract").contains("| fail |"));
    }
}

#[test]
fn translation_checkboxes_order_and_range_expansion_preserve_hashes() {
    let declarations = cell(vec![declaration("behavior"), declaration("integrity")]);
    let source = plan(
        &row(1, &format!("{}..{}", task(1), task(2)), &declarations),
        2,
    );
    let fixture = Fixture::new(&source);
    let (_, before) = fixture.freeze();
    let prompt = source
        .replace("R1 behavior", "R1 translated requirement")
        .replace("| Behavior |", "| English description |")
        .replace("- [ ] R1", "- [x] R1")
        .replace(
            &format!("{}..{}", task(1), task(2)),
            &format!("{}, {}", task(2), task(1)),
        )
        .replace(
            &declarations,
            &cell(vec![declaration("integrity"), declaration("behavior")]),
        );
    fixture.write(&source, &prompt);
    let (passed, receipt) = fixture.run("prompt-check", false);
    assert!(passed, "{receipt}");
    let (_, after) = fixture.freeze();
    assert_eq!(before["membership_sha256"], after["membership_sha256"]);
    let payload = |manifest: &Value| {
        serde_json::from_value::<CriterionPayload>(
            manifest["contract"]["criteria"]["behavior"].clone(),
        )
        .unwrap()
    };
    assert_eq!(
        payload(&before).spec_sha256().unwrap(),
        payload(&after).spec_sha256().unwrap()
    );
}

#[test]
fn parity_refuses_modified_declarations_mappings_and_files() {
    let source = valid_plan();
    let fixture = Fixture::new(&source);
    for prompt in [
        source.replace("值含有", "translated"),
        source.replace("product.json", "other.txt"),
        source.replace("Criteria |", "Removed |"),
        source.replace(&test_id(1), &test_id(2)),
    ] {
        fixture.write(&source, &prompt);
        let (passed, receipt) = fixture.run("prompt-check", false);
        assert!(!passed);
        assert!(check_row(&receipt, "criterion-source-parity").contains("| fail |"));
    }
}

#[test]
fn fenced_examples_are_ignored_and_legacy_plans_keep_checks() {
    let text = valid_plan();
    let example = format!(
        "~~~text\n## Test Plan\n| ID | Type | Description | Covers | Criteria |\n{}~~~\n",
        row(99, &task(99), "[]")
    );
    let with_example = text.replace(
        "| ID | Type | Description | Covers | Criteria |",
        &(example + "| ID | Type | Description | Covers | Criteria |"),
    );
    let fixture = Fixture::new(&with_example);
    let (_, manifest) = fixture.freeze();
    assert_eq!(
        manifest["contract"]["criteria"]["behavior"]["mappings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let (passed, receipt) = fixture.run("refining-check", false);
    assert!(passed, "{receipt}");
    let legacy = text
        .replace(" | Criteria", "")
        .replace(
            "| --- | --- | --- | --- | --- |",
            "| --- | --- | --- | --- |",
        )
        .replace(&format!(" | {}", cell(vec![declaration("behavior")])), "");
    fixture.write(&legacy, &legacy);
    let (passed, receipt) = fixture.run("refining-check", false);
    assert!(passed, "{receipt}");
    assert!(!receipt.contains("| criterion-contract |"));
    let (passed, receipt) = fixture.run("pipeline-preflight", false);
    assert!(passed, "{receipt}");
    assert!(receipt.contains("criterion_membership: legacy-unknown"));
}

#[test]
fn terminal_reads_only_prompt_and_refuses_drift_or_missing_manifest() {
    let text = valid_plan();
    let fixture = Fixture::new(&text);
    fixture.freeze();
    let terminal = text
        .replace("Workflow: IMPLEMENT", "Workflow: DONE")
        .replace(&format!("Current Task: {}", task(1)), "Current Task: —")
        .replace(&format!("- [ ] {}", task(1)), &format!("- [x] {}", task(1)));
    fs::write(
        fixture.repo.path().join(".dev/plans/contract.prompt.md"),
        &terminal,
    )
    .unwrap();
    fs::remove_file(fixture.repo.path().join(".dev/plans/contract.md")).unwrap();
    let (passed, receipt) = fixture.run("pipeline-preflight", true);
    assert!(passed, "{receipt}");
    assert!(check_row(&receipt, "criterion-terminal-reverify").contains("| pass |"));
    fs::write(
        fixture.repo.path().join(".dev/plans/contract.prompt.md"),
        terminal.replace("值含有", "changed"),
    )
    .unwrap();
    let (_, receipt) = fixture.run("pipeline-preflight", true);
    assert!(check_row(&receipt, "criterion-terminal-reverify").contains("| fail |"));
    fs::write(
        fixture.repo.path().join(".dev/plans/contract.prompt.md"),
        &terminal,
    )
    .unwrap();
    let pin = fixture
        .repo
        .path()
        .join(".dev/pipeline/contract/preflight.receipt.md");
    let without_manifest = fs::read_to_string(&pin)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("criterion_manifest_sha256:"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(pin, without_manifest).unwrap();
    let (_, receipt) = fixture.run("pipeline-preflight", true);
    assert!(check_row(&receipt, "criterion-terminal-reverify")
        .contains("missing frozen criterion manifest"));
}

#[test]
fn terminal_refuses_unresolvable_and_tampered_manifest_digests() {
    let fixture = Fixture::new(&valid_plan());
    fixture.freeze();
    let path = fixture
        .repo
        .path()
        .join(".dev/pipeline/contract/preflight.receipt.md");
    let original = fs::read_to_string(&path).unwrap();
    let digest = original
        .lines()
        .find_map(|line| line.strip_prefix("criterion_manifest_sha256: "))
        .unwrap();
    for replacement in ["invalid".to_owned(), "0".repeat(64)] {
        fs::write(&path, original.replace(digest, &replacement)).unwrap();
        let (passed, receipt) = fixture.run("pipeline-preflight", true);
        assert!(!passed, "{receipt}");
        assert!(check_row(&receipt, "criterion-terminal-reverify").contains("| fail |"));
    }
}

#[test]
fn revisions_keep_old_manifests_and_evidence_readable() {
    let source = plan(
        &row(
            1,
            &task(1),
            &cell(vec![declaration("behavior"), declaration("integrity")]),
        ),
        1,
    );
    let fixture = Fixture::new(&source);
    let (old_run, old) = fixture.freeze();
    let store = EvidenceStore::open(&fixture.repo.path().join(".git")).unwrap();
    let content =
        json!({"criterion_id":"integrity","outcome":"fail","evidence":["observed failure"]});
    store
        .publish(&pipeline::report::EvidenceRecord {
            schema_version: 1,
            event_id: "failed-criterion".into(),
            operation_id: "verification".into(),
            run_id: old_run.clone(),
            plan_generation: None,
            task_id: Some(task(1)),
            attempt_id: None,
            parent_attempt_id: None,
            phase: "test".into(),
            source_kind: "criterion-result".into(),
            source_version: "1".into(),
            recorded_at_unix_ms: 0,
            content_hash: pipeline::usage::digest_bytes(&serde_json::to_vec(&content).unwrap()),
            content,
        })
        .unwrap();
    let revised = valid_plan();
    fixture.write(&revised, &revised);
    let (_, new) = fixture.freeze();
    assert_ne!(old["membership_sha256"], new["membership_sha256"]);
    let old_digest = pipeline::usage::digest_bytes(&serde_json::to_vec(&old).unwrap());
    let retained = EvidenceStore::open(&fixture.repo.path().join(".git"))
        .unwrap()
        .read(&old_run, &format!("criterion-manifest-{old_digest}"))
        .unwrap();
    assert_eq!(retained.content, old);
    assert_eq!(
        store.read(&old_run, "failed-criterion").unwrap().content["outcome"],
        "fail"
    );
}

#[test]
fn artifact_freeze_refuses_fallbacks_invalid_paths_and_extractor_disagreement() {
    let original = valid_plan();
    let fixture = Fixture::new(&original);
    assert!(pipeline::task_spec::extract_plan_affected_file_paths(
        &original.replace("product.json", "product.bin"),
        &task(1)
    )
    .is_empty());
    let incidental = original.replace("Verify behavior.", "Verify `unrelated.json`.");
    assert!(
        pipeline::task_spec::extract_plan_affected_file_paths(&incidental, &task(1))
            .contains(&"unrelated.json".to_owned())
    );
    for text in [
        original.replace("  - **Files**: `product.json`\n", "")
            + "\n## Files to Create or Modify\n- `product.json`\n",
        original.replace("product.json", "../outside.json"),
        original.replace("product.json", "/absolute/product.json"),
        original.replace("product.json", "folder/*.json"),
        original.replace("product.json", "new-directory/"),
        original.replace("product.json", "product.bin"),
        original.replace("Verify behavior.", "Verify `unrelated.json`."),
    ] {
        fixture.write(&text, &text);
        let (passed, receipt) = fixture.run("pipeline-preflight", false);
        assert!(!passed, "{receipt}");
        assert!(check_row(&receipt, "criterion-membership-freeze").contains("| fail |"));
        assert!(!receipt.contains("criterion_manifest_sha256:"));
    }
}

#[test]
fn templates_declare_producer_and_verbatim_carry_over() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |path: &str| fs::read_to_string(root.join(path)).unwrap();
    assert!(read("plugins/gal-core/templates/plan.md")
        .contains("| ID | Type | Description | Covers | Criteria |"));
    assert!(
        read("plugins/gal-core/commands/refining-plan/SKILL.template.md")
            .contains("formal Criteria producer")
    );
    assert!(
        read("plugins/gal-core/commands/plan-to-prompt/SKILL.template.md")
            .contains("Carry each cell verbatim")
    );
}
