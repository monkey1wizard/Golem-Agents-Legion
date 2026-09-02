use pipeline::task_spec::{
    assemble_task_spec,
    compute_contract_region_digest, // EF-09 contract-region probe
    decode_base64url_unpadded,
    encode_base64url_unpadded,
    extract_generation_ledger,
    extract_latest_generation,
    extract_phase_affected_file_paths,
    extract_phase_affected_files,
    extract_plan_slug,
    has_test_first_marker,
    list,
    lp,
    normalize_canonical_text,
    parse_task_contract,
    test_first_receipt_rel,
    test_first_snapshot_rel,
    validate_repo_path,
    Applicability,
    TaskContractError,
    TaskSpecInput,
};

// Compose plan-task IDs at runtime — never hardcode `T-NN` literals in source
// (naming-gate provenance rule: plan-task IDs belong only in .dev).
fn tid(n: u32) -> String {
    format!("T-{n:02}")
}

fn tpid(n: u32) -> String {
    format!("TP-{n:02}")
}

fn sample_prompt_required(task_id: &str) -> String {
    format!(
        r#"# Plan Prompt: Test-First Pipeline

Pipeline Contract: test-first-v1

## Tasks

- [ ] {task_id} — Parse contracts and canonical digests in `crates/pipeline`.
  - Test-first: required
  - Seam: `pipeline::task_spec::parse_task_contract`
  - Expected failures:
    - EF-01;class=error-code;term=exit:1;stream=stderr;matcher_b64=cGFyc2VyIGVycm9y — Parse error
    - EF-02;class=assertion;term=nonzero;stream=combined;matcher_b64=YXNzZXJ0aW9uIGZhaWxlZA — Assertion failure
  - Production Paths:
    - `crates/pipeline/src/task_spec.rs`
    - `crates/pipeline/Cargo.toml`
  - Test Paths:
    - `crates/pipeline/tests/task_spec.rs`
  - Scaffold: required

## Test Plan

| ID | Type | Description | Tasks |
| {tp_id} | unit | Task-spec golden vectors prove exact digest | {task_id} |
"#,
        tp_id = tpid(14)
    )
}

fn sample_prompt_not_applicable(task_id: &str) -> String {
    format!(
        r#"# Plan Prompt: Test-First Pipeline

Pipeline Contract: test-first-v1

## Tasks

- [ ] {task_id} — Documentation update task.
  - Test-first: not-applicable — Documentation only change with no executable behavior
  - Non-red probe: `cargo test --doc`

## Test Plan

| ID | Type | Description | Tasks |
| {tp_id} | doc | Verify docs compile | {task_id} |
"#,
        tp_id = tpid(99)
    )
}

#[test]
fn test_has_test_first_marker() {
    let marked = sample_prompt_required(&tid(14));
    assert!(has_test_first_marker(&marked));

    let markerless = format!(
        r#"# Plan Prompt: Legacy Pipeline

## Tasks

- [ ] {} — Legacy task.
"#,
        tid(1)
    );
    assert!(!has_test_first_marker(&markerless));

    // Marker inside code fence must be ignored
    let fence_marked = format!(
        r#"# Plan Prompt: Fake Marker

```text
Pipeline Contract: test-first-v1
```

## Tasks

- [ ] {} — Task.
"#,
        tid(1)
    );
    assert!(!has_test_first_marker(&fence_marked));
}

#[test]
fn test_lp_and_list_framing() {
    let raw = b"hello";
    let framed = lp(raw);
    assert_eq!(framed.len(), 8 + 5);
    assert_eq!(&framed[0..8], &5u64.to_be_bytes());
    assert_eq!(&framed[8..], b"hello");

    let items = vec![b"abc".to_vec(), b"de".to_vec()];
    let lst = list(&items);
    assert_eq!(&lst[0..8], &2u64.to_be_bytes());
    assert_eq!(&lst[8..], &[lp(b"abc"), lp(b"de")].concat());
}

#[test]
fn test_base64url_unpadded_codec() {
    let input = b"test-first-argv-v1\x00\x00\x00\x00\x00\x00\x00\x01";
    let encoded = encode_base64url_unpadded(input);
    assert!(!encoded.contains('='));
    assert!(!encoded.contains('+'));
    assert!(!encoded.contains('/'));

    let decoded = decode_base64url_unpadded(&encoded).expect("must decode cleanly");
    assert_eq!(decoded, input);

    // Standard base64 with padding must be rejected by unpadded decoder
    let padded = format!("{encoded}=");
    assert!(decode_base64url_unpadded(&padded).is_err());
}

#[test]
fn test_validate_repo_path() {
    assert_eq!(
        validate_repo_path("crates/pipeline/src/task_spec.rs").unwrap(),
        "crates/pipeline/src/task_spec.rs"
    );

    assert!(validate_repo_path("").is_err());
    assert!(validate_repo_path("/absolute/path.rs").is_err());
    assert!(validate_repo_path("C:/absolute/path.rs").is_err());
    assert!(validate_repo_path("crates\\pipeline\\src\\task_spec.rs").is_err());
    assert!(validate_repo_path("crates/../pipeline/src/task_spec.rs").is_err());
    assert!(validate_repo_path("crates/./task_spec.rs").is_err());
    assert!(validate_repo_path("crates/pipeline/*").is_err());
    assert!(validate_repo_path("crates/pipeline/dir/").is_err());
}

#[test]
fn test_parse_valid_required_contract() {
    let task_id = tid(14);
    let prompt = sample_prompt_required(&task_id);
    let contract = parse_task_contract(&prompt, "feat-test-first-pipeline", &task_id)
        .expect("should parse valid required contract");

    assert_eq!(contract.plan_slug, "feat-test-first-pipeline");
    assert_eq!(contract.task_id, task_id);
    assert_eq!(contract.applicability, Applicability::Required);
    assert_eq!(contract.seam, "`pipeline::task_spec::parse_task_contract`");
    assert_eq!(contract.expected_failures.len(), 2);
    assert_eq!(
        contract.production_paths,
        vec![
            "crates/pipeline/src/task_spec.rs",
            "crates/pipeline/Cargo.toml"
        ]
    );
    assert_eq!(
        contract.test_paths,
        vec!["crates/pipeline/tests/task_spec.rs"]
    );
    assert_eq!(contract.scaffold, "required");
    assert_eq!(contract.tp_rows.len(), 1);

    let digest = contract.compute_digest();
    assert_eq!(digest.len(), 64);
    assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_parse_valid_not_applicable_contract() {
    let task_id = tid(99);
    let prompt = sample_prompt_not_applicable(&task_id);
    let contract = parse_task_contract(&prompt, "feat-test-first-pipeline", &task_id)
        .expect("should parse valid not-applicable contract");

    assert_eq!(contract.applicability, Applicability::NotApplicable);
    assert_eq!(
        contract.not_applicable_rationale,
        "Documentation only change with no executable behavior"
    );
    assert_eq!(contract.non_red_probe, "`cargo test --doc`");
    assert!(contract.seam.is_empty());
    assert!(contract.expected_failures.is_empty());
    assert_eq!(contract.tp_rows.len(), 1);

    let digest = contract.compute_digest();
    assert_eq!(digest.len(), 64);
}

#[test]
fn test_missing_required_fields_returns_error() {
    let task_id = tid(14);
    let prompt_missing_seam = format!(
        r#"# Plan Prompt: Test-First Pipeline

Pipeline Contract: test-first-v1

## Tasks

- [ ] {task_id} — Missing seam task.
  - Test-first: required
  - Production Paths:
    - `crates/pipeline/src/task_spec.rs`
  - Test Paths:
    - `crates/pipeline/tests/task_spec.rs`
  - Scaffold: required
"#
    );

    let err = parse_task_contract(&prompt_missing_seam, "slug", &task_id).unwrap_err();
    assert!(matches!(err, TaskContractError::MissingField(_, "Seam")));
}

#[test]
fn test_same_file_production_and_test_paths_parse_successfully() {
    let task_id = tid(14);
    let prompt_overlap = format!(
        r#"# Plan Prompt: Test-First Pipeline

Pipeline Contract: test-first-v1

## Tasks

- [ ] {task_id} — Overlapping paths task.
  - Test-first: required
  - Seam: `some::seam`
  - Expected failures: EF-01;class=error-code;term=exit:1;stream=stderr;matcher_b64=eHg — err
  - Production Paths:
    - `crates/pipeline/src/task_spec.rs`
  - Test Paths:
    - `crates/pipeline/src/task_spec.rs`
  - Scaffold: required
"#
    );

    let contract = parse_task_contract(&prompt_overlap, "slug", &task_id)
        .expect("same-file production and test paths parse successfully");
    assert_eq!(
        contract.production_paths,
        vec!["crates/pipeline/src/task_spec.rs"]
    );
    assert_eq!(
        contract.test_paths,
        vec!["crates/pipeline/src/task_spec.rs"]
    );
}

#[test]
fn test_markerless_defaults_to_not_applicable() {
    let task_id = tid(14);
    let markerless_prompt = format!(
        r#"# Plan Prompt: Legacy

## Tasks

- [ ] {task_id} — Legacy task.
  - `crates/pipeline/src/task_spec.rs`
"#
    );

    let contract = parse_task_contract(&markerless_prompt, "slug", &task_id).unwrap();
    assert_eq!(contract.applicability, Applicability::NotApplicable);
}

#[test]
fn test_lfcrlf_normalization_yields_identical_digest() {
    let task_id = tid(14);
    let prompt_lf = sample_prompt_required(&task_id);
    let prompt_crlf = prompt_lf.replace('\n', "\r\n");

    let contract_lf = parse_task_contract(&prompt_lf, "slug", &task_id).unwrap();
    let contract_crlf = parse_task_contract(&prompt_crlf, "slug", &task_id).unwrap();

    assert_eq!(
        contract_lf.canonical_bytes(),
        contract_crlf.canonical_bytes()
    );
    assert_eq!(contract_lf.compute_digest(), contract_crlf.compute_digest());
}

#[test]
fn test_nfc_normalization_yields_identical_digest() {
    let nfc_seam = "seam_\u{00E9}";
    let nfd_seam = "seam_e\u{0301}";

    let task_id = tid(14);
    let mut c1 = parse_task_contract(&sample_prompt_required(&task_id), "slug", &task_id).unwrap();
    let mut c2 = c1.clone();

    c1.seam = nfc_seam.to_string();
    c2.seam = nfd_seam.to_string();

    assert_eq!(
        normalize_canonical_text(&c1.seam),
        normalize_canonical_text(&c2.seam)
    );
    assert_eq!(c1.canonical_bytes(), c2.canonical_bytes());
    assert_eq!(c1.compute_digest(), c2.compute_digest());
}

#[test]
fn test_path_order_neutrality() {
    let task_id = tid(14);
    let mut c1 = parse_task_contract(&sample_prompt_required(&task_id), "slug", &task_id).unwrap();
    let c2 = {
        let mut c = c1.clone();
        c.production_paths = vec!["b.rs".to_string(), "a.rs".to_string()];
        c
    };
    c1.production_paths = vec!["a.rs".to_string(), "b.rs".to_string()];

    assert_eq!(c1.canonical_bytes(), c2.canonical_bytes());
    assert_eq!(c1.compute_digest(), c2.compute_digest());
}

#[test]
fn test_expected_failure_and_tp_order_sensitivity() {
    let task_id = tid(14);
    let c1 = parse_task_contract(&sample_prompt_required(&task_id), "slug", &task_id).unwrap();

    let mut c1 = c1;
    let mut c2 = c1.clone();
    c1.expected_failures = vec!["EF-01".to_string(), "EF-02".to_string()];
    c2.expected_failures = vec!["EF-02".to_string(), "EF-01".to_string()];

    assert_ne!(c1.canonical_bytes(), c2.canonical_bytes());
    assert_ne!(c1.compute_digest(), c2.compute_digest());

    let mut c3 = c1.clone();
    let mut c4 = c1.clone();
    let row_a = format!("| {} | a |", tpid(1));
    let row_b = format!("| {} | b |", tpid(2));
    c3.tp_rows = vec![row_a.clone(), row_b.clone()];
    c4.tp_rows = vec![row_b, row_a];

    assert_ne!(c3.compute_digest(), c4.compute_digest());
}

#[test]
fn test_one_component_mutation_vectors() {
    let task_id = tid(14);
    let base = parse_task_contract(&sample_prompt_required(&task_id), "slug", &task_id).unwrap();
    let base_digest = base.compute_digest();

    let mut m = base.clone();
    m.plan_slug.push('x');
    assert_ne!(m.compute_digest(), base_digest);

    let mut m = base.clone();
    m.task_id.push('x');
    assert_ne!(m.compute_digest(), base_digest);

    let mut m = base.clone();
    m.applicability = Applicability::NotApplicable;
    assert_ne!(m.compute_digest(), base_digest);

    let mut m = base.clone();
    m.seam.push('x');
    assert_ne!(m.compute_digest(), base_digest);

    let mut m = base.clone();
    m.expected_failures[0].push('x');
    assert_ne!(m.compute_digest(), base_digest);

    let mut m = base.clone();
    m.production_paths[0].push('x');
    assert_ne!(m.compute_digest(), base_digest);

    let mut m = base.clone();
    m.test_paths[0].push('x');
    assert_ne!(m.compute_digest(), base_digest);

    let mut m = base.clone();
    m.scaffold = "not-required".to_string();
    assert_ne!(m.compute_digest(), base_digest);

    let mut m = base.clone();
    m.not_applicable_rationale.push('x');
    assert_ne!(m.compute_digest(), base_digest);

    let mut m = base.clone();
    m.non_red_probe.push('x');
    assert_ne!(m.compute_digest(), base_digest);

    let mut m = base.clone();
    m.tp_rows[0].push('x');
    assert_ne!(m.compute_digest(), base_digest);
}

#[test]
fn golden_contract_digest_is_locked() {
    let task_id = tid(14);
    let prompt = sample_prompt_required(&task_id);
    let contract = parse_task_contract(&prompt, "feat-test-first-pipeline", &task_id).unwrap();
    assert_eq!(
        contract.compute_digest(),
        "7a5a429aa90c311312da6c44a6e71adb59aa962f7b7fe8c0938c39ab61c6dfa2"
    );
    assert_eq!(contract.canonical_bytes().len(), 613);
}

#[test]
fn test_extract_plan_slug_parsing() {
    assert_eq!(
        extract_plan_slug(".dev/plans/feat-test-first-pipeline.prompt.md"),
        "feat-test-first-pipeline"
    );
    assert_eq!(
        extract_plan_slug("C:/Code/Golem-Agents-Legion/.dev/plans/my-feature.md"),
        "my-feature"
    );
    assert_eq!(extract_plan_slug("bare-slug"), "bare-slug");
}

#[test]
fn test_test_first_path_helpers() {
    let receipt = test_first_receipt_rel("slug", &tid(16), 2, "abc123digest", "red");
    assert_eq!(
        receipt,
        format!(
            ".dev/pipeline/receipts/slug/{}/g2-cabc123digest/probe-red.receipt.md",
            tid(16)
        )
    );

    let snapshot = test_first_snapshot_rel("slug", &tid(16), 1, "abc123digest", "pre-implement");
    assert_eq!(
        snapshot,
        format!(
            ".dev/pipeline/snapshots/slug/{}/g1-cabc123digest/pre-implement.snapshot.tsv",
            tid(16)
        )
    );
}

#[test]
fn test_extract_generation_ledger_and_latest_generation() {
    let task_id = tid(16);
    let other_id = tid(99);
    let absent_id = tid(0);
    let prompt_body = format!(
        r#"# Plan Prompt

Pipeline Contract: test-first-v1

## Status

### Test-First Generations

| Task | Generation | Contract Digest | Reason |
| {task_id} | 1 | digest1 | init |
| {task_id} | 2 | digest2 | probe-defect |
| {other_id} | 1 | digest99 | init |
"#
    );

    let ledger = extract_generation_ledger(&prompt_body);
    assert_eq!(ledger.len(), 3);
    assert_eq!(ledger[0].task_id, task_id);
    assert_eq!(ledger[0].generation, 1);
    assert_eq!(ledger[1].task_id, task_id);
    assert_eq!(ledger[1].generation, 2);

    assert_eq!(extract_latest_generation(&prompt_body, &task_id), Some(2));
    assert_eq!(extract_latest_generation(&prompt_body, &other_id), Some(1));
    assert_eq!(extract_latest_generation(&prompt_body, &absent_id), None);
}

#[test]
fn test_phase_allowlists_scaffold_test_implement_audit() {
    let task_id = tid(16);
    let prompt = sample_prompt_required(&task_id);
    let slug = "feat-test-first-pipeline";

    let scaffold_paths = extract_phase_affected_file_paths(&prompt, slug, &task_id, "scaffold");
    assert_eq!(
        scaffold_paths,
        vec![
            "crates/pipeline/src/task_spec.rs",
            "crates/pipeline/Cargo.toml"
        ]
    );

    let implement_paths = extract_phase_affected_file_paths(&prompt, slug, &task_id, "implement");
    assert_eq!(
        implement_paths,
        vec![
            "crates/pipeline/src/task_spec.rs",
            "crates/pipeline/Cargo.toml"
        ]
    );

    let test_paths = extract_phase_affected_file_paths(&prompt, slug, &task_id, "test");
    assert_eq!(test_paths, vec!["crates/pipeline/tests/task_spec.rs"]);

    let audit_paths = extract_phase_affected_file_paths(&prompt, slug, &task_id, "audit");
    assert_eq!(
        audit_paths,
        vec![
            "crates/pipeline/src/task_spec.rs",
            "crates/pipeline/Cargo.toml",
            "crates/pipeline/tests/task_spec.rs"
        ]
    );

    let scaffold_files = extract_phase_affected_files(&prompt, slug, &task_id, "scaffold");
    assert_eq!(scaffold_files.len(), 2);

    let test_files = extract_phase_affected_files(&prompt, slug, &task_id, "test");
    assert_eq!(test_files.len(), 1);

    let audit_files = extract_phase_affected_files(&prompt, slug, &task_id, "audit");
    assert_eq!(audit_files.len(), 3);
}

#[test]
fn test_assemble_task_spec_test_first_rendering() {
    let task_id = tid(16);
    let mut prompt_body = sample_prompt_required(&task_id);
    prompt_body.push_str(&format!(
        "\n## Status\n\n### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| {task_id} | 3 | digest3 | phase-rerun |\n"
    ));

    let input = TaskSpecInput {
        task_scope: &task_id,
        phase: "test",
        prompt_path: ".dev/plans/feat-test-first-pipeline.prompt.md",
        prompt_body: &prompt_body,
        generated: "2026-08-19 12:00",
        git_branch: "main",
        git_head: "26649bc3",
        convention_hints: None,
        receipt_path: None,
        agent_contract_body: None,
        fix_mode: false,
    };

    let spec = assemble_task_spec(&input).expect("spec assembly should succeed");

    assert!(spec.markdown.contains("Pipeline Contract: test-first-v1"));
    assert!(spec.markdown.contains("Generation: 3"));
    assert!(spec.markdown.contains("Contract digest:"));
    assert!(spec.markdown.contains("### Test-First Contract Details"));
    assert!(spec
        .markdown
        .contains("TESTER must author acceptance probes in Test Paths"));
    let affected_section = spec
        .markdown
        .split_once("## Affected Files")
        .unwrap()
        .1
        .split_once("## Write-Back")
        .unwrap()
        .0;
    assert!(affected_section.contains("crates/pipeline/tests/task_spec.rs"));
    assert!(!affected_section.contains("crates/pipeline/src/task_spec.rs"));
    assert!(spec.markdown.contains("probe-red.receipt.md"));
}

#[test]
fn test_assemble_task_spec_legacy_unchanged() {
    let task_id = tid(16);
    let legacy_prompt = format!(
        r#"# Plan Prompt: Legacy

## Files to Create or Modify

- [MODIFY] `crates/pipeline/src/task_spec.rs`: port task spec.

## Tasks

- [ ] {task_id} — Port task spec.
  - touch `crates/pipeline/src/task_spec.rs`
"#
    );

    let input = TaskSpecInput {
        task_scope: &task_id,
        phase: "implement",
        prompt_path: ".dev/plans/legacy-plan.prompt.md",
        prompt_body: &legacy_prompt,
        generated: "2026-08-19 12:00",
        git_branch: "main",
        git_head: "26649bc3",
        convention_hints: None,
        receipt_path: None,
        agent_contract_body: None,
        fix_mode: false,
    };

    let spec = assemble_task_spec(&input).expect("spec assembly should succeed");

    assert!(!spec.markdown.contains("Pipeline Contract: test-first-v1"));
    assert!(!spec.markdown.contains("Generation:"));
    assert!(!spec.markdown.contains("Test-First Contract Details"));
    assert!(spec.markdown.contains("crates/pipeline/src/task_spec.rs"));
}

#[test]
fn test_malformed_marked_contract_returns_empty_allowlist() {
    let task_id = tid(16);
    let slug = "feat-test-first-pipeline";

    let malformed_prompt = format!(
        r#"# Plan Prompt: Test-First Pipeline

Pipeline Contract: test-first-v1

## Files to Create or Modify

- [MODIFY] `crates/pipeline/src/task_spec.rs`: legacy roster path.
- [MODIFY] `crates/pipeline/src/lib.rs`: legacy roster path.

## Tasks

- [ ] {task_id} — Malformed contract task.
  - Test-first: required
  - Production Paths:
    - `crates/pipeline/src/task_spec.rs`
"#
    );

    assert!(parse_task_contract(&malformed_prompt, slug, &task_id).is_err());

    let file_paths =
        extract_phase_affected_file_paths(&malformed_prompt, slug, &task_id, "implement");
    assert!(
        file_paths.is_empty(),
        "malformed marked contract must yield empty file paths allowlist, got {:?}",
        file_paths
    );

    let files = extract_phase_affected_files(&malformed_prompt, slug, &task_id, "implement");
    assert!(
        files.is_empty(),
        "malformed marked contract must yield empty files allowlist, got {:?}",
        files
    );

    let na_prompt_with_files = format!(
        r#"# Plan Prompt: Test-First Pipeline

Pipeline Contract: test-first-v1

## Files to Create or Modify

- [MODIFY] `crates/pipeline/src/task_spec.rs`: doc updates.

## Tasks

- [ ] {task_id} — Documentation update task.
  - Test-first: not-applicable — Documentation only change with no executable behavior
  - Non-red probe: `cargo test --doc`
"#
    );

    let contract_na = parse_task_contract(&na_prompt_with_files, slug, &task_id).unwrap();
    assert_eq!(contract_na.applicability, Applicability::NotApplicable);

    let na_file_paths =
        extract_phase_affected_file_paths(&na_prompt_with_files, slug, &task_id, "implement");
    assert_eq!(na_file_paths, vec!["crates/pipeline/src/task_spec.rs"]);
}

fn sample_contract_region_prompt() -> String {
    format!(
        r#"# Plan Prompt: Test Contract Region Digest

## Goal

Repair terminal deadlock for completed plans.

## Requirements

- [ ] R1 — Full-mode finalize-check must use clean evaluator.
- [ ] R2 — Shared goal-binding evaluator returns structured results.

## Tasks

- [ ] {task_1} — Task one description.
  - Test-first: required
  - Seam: `some::seam`
  - Expected failures: EF-01 — description
  - Production Paths:
    - `crates/pipeline/src/task_spec.rs`
  - Test Paths:
    - `crates/pipeline/tests/task_spec.rs`
  - Scaffold: not-required

## Test Plan

| ID | Type | Description | Tasks |
| {tp_1} | unit | Test one | {task_1} |

## Status

Workflow: TEST
Step: 2 of 7
Last activity: 2026-08-25 — {task_1} started

### Handoff Notes

- Handoff note line 1.

## Test Results

### [{task_1}] 2026-08-25
Total: 1 | Passed: 1 | Failed: 0
"#,
        task_1 = tid(1),
        tp_1 = tpid(1)
    )
}

#[test]
fn test_marked_contract_region_digest_mutation_matrix() {
    // EF-09 probe: verify contract-region digest helper and mutation matrix
    let base_prompt = sample_contract_region_prompt();
    let base_digest = compute_contract_region_digest(&base_prompt)
        .expect("valid contract-region prompt must compute digest");

    assert_eq!(base_digest.len(), 64);
    assert!(base_digest.chars().all(|c| c.is_ascii_hexdigit()));

    // 1. Mutate Goal -> digest MUST change
    let goal_mutated = base_prompt.replace(
        "Repair terminal deadlock",
        "Repair terminal deadlock quickly",
    );
    let goal_digest = compute_contract_region_digest(&goal_mutated).unwrap();
    assert_ne!(
        goal_digest, base_digest,
        "contract-region digest must change when Goal is mutated"
    );

    // 2. Mutate Requirements -> digest MUST change
    let req_mutated = base_prompt.replace("- [ ] R1 — Full-mode", "- [ ] R1 — Modified full-mode");
    let req_digest = compute_contract_region_digest(&req_mutated).unwrap();
    assert_ne!(
        req_digest, base_digest,
        "contract-region digest must change when Requirements is mutated"
    );

    // 3. Mutate Tasks -> digest MUST change
    let tasks_mutated =
        base_prompt.replace("Task one description", "Task one modified description");
    let tasks_digest = compute_contract_region_digest(&tasks_mutated).unwrap();
    assert_ne!(
        tasks_digest, base_digest,
        "contract-region digest must change when Tasks is mutated"
    );

    // 4. Mutate Test Plan -> digest MUST change
    let tp_mutated = base_prompt.replace("Test one", "Test one modified");
    let tp_digest = compute_contract_region_digest(&tp_mutated).unwrap();
    assert_ne!(
        tp_digest, base_digest,
        "contract-region digest must change when Test Plan is mutated"
    );

    // 5. Mutate Status -> digest MUST NOT change
    let status_mutated = base_prompt.replace("Workflow: TEST", "Workflow: IMPLEMENT");
    let status_digest = compute_contract_region_digest(&status_mutated).unwrap();
    assert_eq!(
        status_digest, base_digest,
        "contract-region digest must NOT change when Status is mutated"
    );

    // 6. Mutate Handoff Notes -> digest MUST NOT change
    let handoff_mutated =
        base_prompt.replace("Handoff note line 1.", "Handoff note line 1 modified.");
    let handoff_digest = compute_contract_region_digest(&handoff_mutated).unwrap();
    assert_eq!(
        handoff_digest, base_digest,
        "contract-region digest must NOT change when Handoff Notes is mutated"
    );

    // 7. Mutate Test Results -> digest MUST NOT change
    let results_mutated = base_prompt.replace("Total: 1 | Passed: 1", "Total: 2 | Passed: 2");
    let results_digest = compute_contract_region_digest(&results_mutated).unwrap();
    assert_eq!(
        results_digest, base_digest,
        "contract-region digest must NOT change when Test Results is mutated"
    );
}

#[test]
fn test_marked_contract_region_digest_topology_validation() {
    // EF-09 probe: contract-region topology validation rejects missing/duplicate sections
    let base = sample_contract_region_prompt();

    // Missing Goal
    let missing_goal = base.replace(
        "## Goal\n\nRepair terminal deadlock for completed plans.",
        "",
    );
    assert!(
        compute_contract_region_digest(&missing_goal).is_err(),
        "missing Goal must fail topology validation"
    );

    // Missing Requirements
    let missing_req = base.split("## Requirements").next().unwrap().to_string()
        + "## Tasks"
        + base.split("## Tasks").nth(1).unwrap();
    assert!(
        compute_contract_region_digest(&missing_req).is_err(),
        "missing Requirements must fail topology validation"
    );

    // Missing Tasks
    let missing_tasks = base.split("## Tasks").next().unwrap().to_string()
        + "## Test Plan"
        + base.split("## Test Plan").nth(1).unwrap();
    assert!(
        compute_contract_region_digest(&missing_tasks).is_err(),
        "missing Tasks must fail topology validation"
    );

    // Missing Test Plan
    let missing_tp = base.split("## Test Plan").next().unwrap().to_string()
        + "## Status"
        + base.split("## Status").nth(1).unwrap();
    assert!(
        compute_contract_region_digest(&missing_tp).is_err(),
        "missing Test Plan must fail topology validation"
    );

    // Duplicate Goal
    let duplicate_goal = base.replace("## Goal", "## Goal\n\nDuplicate Goal text\n\n## Goal");
    assert!(
        compute_contract_region_digest(&duplicate_goal).is_err(),
        "duplicate Goal section must fail topology validation"
    );
}

#[test]
fn test_marked_contract_region_digest_normalization() {
    // EF-09 probe: contract-region normalization (CRLF vs LF)
    let prompt_lf = sample_contract_region_prompt();
    let prompt_crlf = prompt_lf.replace('\n', "\r\n");

    let digest_lf = compute_contract_region_digest(&prompt_lf).unwrap();
    let digest_crlf = compute_contract_region_digest(&prompt_crlf).unwrap();
    assert_eq!(
        digest_lf, digest_crlf,
        "CRLF and LF must produce identical contract-region digest"
    );
}
