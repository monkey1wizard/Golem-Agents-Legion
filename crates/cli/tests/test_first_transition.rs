//! Real-binary coverage for the internal prompt transition registration.
//!
//! Exercises the compiled `gal` binary end to end: argv parsing, exit codes, and
//! stdout/receipt output, not just the in-process producer functions (those have
//! their own unit tests in `commands::test_first_transition`).

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("git must be available");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_with_global_config(repo: &Path, args: &[&str], global_config: &Path) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", global_config)
        .output()
        .expect("git must be available");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn tid(n: u32) -> String {
    format!("T-{n:02}")
}

fn source_text(digest: &str) -> String {
    format!("# Source\n\n- Human approval: [approved]\n\n<!-- gal:approved-prompt-bytes sha256={digest} -->\n")
}

fn valid_prompt(status: &str) -> Vec<u8> {
    format!(
        "## Goal\n\n- Goal\n\n## Requirements\n\n- Req\n\n## Tasks\n\n- Task\n\n## Test Plan\n\n- Test plan\n\n## Status\n\n{status}\n"
    )
    .into_bytes()
}

/// A git-initialized repo with a paired `.dev/plans/sample.{prompt.md,md}` fixture,
/// both committed. `Fixture::source` may be freely mutated by callers before
/// invoking the binary to build dirty/staged/unapproved/mismatched variants.
struct Fixture {
    _temp: TempDir,
    repo: PathBuf,
    prompt: PathBuf,
    source: PathBuf,
    prompt_bytes: Vec<u8>,
}

fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().to_path_buf();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "fixture@example.com"]);
    git(&repo, &["config", "user.name", "CLI Fixture"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    git(&repo, &["config", "core.hooksPath", ".git/hooks-fixture"]);
    fs::create_dir_all(repo.join(".git/hooks-fixture")).unwrap();
    fs::create_dir_all(repo.join(".dev/plans")).unwrap();
    let prompt_bytes = b"# Legacy prompt\n\nNo marker.\n".to_vec();
    let prompt = repo.join(".dev/plans/sample.prompt.md");
    fs::write(&prompt, &prompt_bytes).unwrap();
    let digest = sha256_hex(&prompt_bytes);
    let source = repo.join(".dev/plans/sample.md");
    fs::write(&source, source_text(&digest)).unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "fixture"]);
    Fixture {
        _temp: temp,
        repo,
        prompt,
        source,
        prompt_bytes,
    }
}

#[test]
fn fixture_commit_ignores_hostile_global_git_config() {
    let fx = fixture();
    let hostile_hooks = fx._temp.path().join("hostile-hooks");
    fs::create_dir_all(&hostile_hooks).unwrap();
    fs::write(hostile_hooks.join("pre-commit"), b"#!/bin/sh\nexit 1\n").unwrap();

    let hostile_global = fx._temp.path().join("hostile.gitconfig");
    let hostile_hooks_path = hostile_hooks.to_string_lossy().replace('\\', "/");
    fs::write(
        &hostile_global,
        format!("[commit]\n\tgpgsign = true\n[core]\n\thooksPath = {hostile_hooks_path}\n"),
    )
    .unwrap();
    let global_before = fs::read(&hostile_global).unwrap();
    let local_config = fx.repo.join(".git/config");
    let local_before = fs::read(&local_config).unwrap();

    fs::write(fx.repo.join("hostile-config-probe.txt"), b"fixture\n").unwrap();
    git(&fx.repo, &["add", "."]);
    git_with_global_config(
        &fx.repo,
        &["commit", "-q", "-m", "hostile config probe"],
        &hostile_global,
    );

    assert_eq!(fs::read(&hostile_global).unwrap(), global_before);
    assert_eq!(fs::read(&local_config).unwrap(), local_before);
}

fn bootstrap(repo: &Path, receipt: Option<&Path>) -> (bool, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gal"));
    command.current_dir(repo).args([
        "test-first-transition",
        "legacy-bootstrap",
        ".dev/plans/sample.prompt.md",
    ]);
    if let Some(receipt) = receipt {
        command.arg("--receipt").arg(receipt);
    }
    let output = command.output().unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn transition(repo: &Path, args: &[&str]) -> (bool, String, String) {
    let mut full = vec!["test-first-transition"];
    full.extend_from_slice(args);
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .args(&full)
        .output()
        .unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn write_bytes_file(repo: &Path, name: &str, bytes: &[u8]) -> String {
    fs::write(repo.join(name), bytes).unwrap();
    name.to_string()
}

fn journal_path(repo: &Path) -> PathBuf {
    repo.join(".dev/pipeline/journal/sample/transition.journal.tsv")
}

#[test]
fn real_binary_no_journal_boundary_rejects_stale_cas() {
    let fx = fixture();
    // The prompt exists on disk (from the fixture) but was never transitioned, so
    // there is no journal at all yet. A `refresh` against a wrong assumed baseline
    // must still be rejected by the CAS check, not treated as an absent-prompt init.
    let bytes_file = write_bytes_file(&fx.repo, "new.bytes", b"v2\n");
    let wrong_digest = "f".repeat(64);
    let (ok, _out, err) = transition(
        &fx.repo,
        &[
            "refresh",
            ".dev/plans/sample.prompt.md",
            &bytes_file,
            &wrong_digest,
        ],
    );
    assert!(!ok, "{err}");
    assert!(err.contains("stale transition CAS"), "{err}");
    assert_eq!(fs::read(&fx.prompt).unwrap(), fx.prompt_bytes);
}
#[test]
fn marked_prompt_refresh_composes_status_edit_and_binds_digest() {
    let fx = fixture();
    let task = tid(23);
    let marked = format!(
        "Pipeline Contract: test-first-v1\n\n# Prompt\n\n## Goal\n\n- Goal\n\n## Requirements\n\n- Req\n\n## Tasks\n\n- [ ] {task} — transition composition coverage\n\n## Test Plan\n\n- Test plan\n\n## Status\n\nCurrent Task: —\n"
    )
    .into_bytes();
    let marked_digest = sha256_hex(&marked);
    fs::write(&fx.prompt, &marked).unwrap();
    fs::write(&fx.source, source_text(&marked_digest)).unwrap();
    git(&fx.repo, &["add", "."]);
    git(
        &fx.repo,
        &["commit", "-q", "-m", "marked transition fixture"],
    );

    let composed = String::from_utf8(marked.clone())
        .unwrap()
        .replace("Current Task: —", &format!("Current Task: {task}"))
        .into_bytes();
    let composed_file = write_bytes_file(&fx.repo, "composed-status.bytes", &composed);
    let (ok, out, err) = transition(
        &fx.repo,
        &[
            "refresh",
            ".dev/plans/sample.prompt.md",
            &composed_file,
            &marked_digest,
        ],
    );
    assert!(ok, "stdout: {out}\nstderr: {err}");
    let journal = fs::read_to_string(journal_path(&fx.repo)).unwrap();
    assert!(journal.contains("\tphase-rerun\t"), "{journal}");
    assert_eq!(fs::read(&fx.prompt).unwrap(), composed);

    let receipt_path = fx.repo.join("refresh-boundary.receipt.md");
    let _boundary = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(&fx.repo)
        .args([
            "boundary-check",
            ".dev/plans/sample.prompt.md",
            "--task",
            &task,
            "--receipt",
            "refresh-boundary.receipt.md",
        ])
        .output()
        .unwrap();
    let receipt = fs::read_to_string(&receipt_path).unwrap();
    assert!(receipt.contains("| digest-binding | pass |"), "{receipt}");

    let negative = fixture();
    fs::write(&negative.prompt, &marked).unwrap();
    fs::write(&negative.source, source_text(&marked_digest)).unwrap();
    git(&negative.repo, &["add", "."]);
    git(
        &negative.repo,
        &["commit", "-q", "-m", "marked direct-write fixture"],
    );
    fs::write(&negative.prompt, &composed).unwrap();
    let direct_write_boundary = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(&negative.repo)
        .args([
            "boundary-check",
            ".dev/plans/sample.prompt.md",
            "--task",
            &task,
            "--receipt",
            "direct-write-boundary.receipt.md",
        ])
        .output()
        .unwrap();
    let direct_write_receipt =
        fs::read_to_string(negative.repo.join("direct-write-boundary.receipt.md")).unwrap();
    assert!(
        direct_write_receipt.contains("| digest-binding | fail |"),
        "{direct_write_receipt}"
    );
    assert!(!direct_write_boundary.status.success());

    let negative_composed_file =
        write_bytes_file(&negative.repo, "composed-status.bytes", &composed);
    let before_stale = fs::read(&negative.prompt).unwrap();
    let (ok, _out, err) = transition(
        &negative.repo,
        &[
            "refresh",
            ".dev/plans/sample.prompt.md",
            &negative_composed_file,
            &marked_digest,
        ],
    );
    assert!(!ok, "{err}");
    assert!(err.contains("stale transition CAS"), "{err}");
    assert_eq!(fs::read(&negative.prompt).unwrap(), before_stale);
}

#[test]
fn real_binary_init_refresh_install_generation_round_trip_with_native_replacement() {
    let fx = fixture();
    // `init` requires an absent prompt; start from a fresh slug.
    let bytes_v1 = write_bytes_file(&fx.repo, "v1.bytes", &valid_prompt("v1"));
    let (ok, out, err) = transition(
        &fx.repo,
        &["init", ".dev/plans/roundtrip.prompt.md", &bytes_v1],
    );
    assert!(ok, "{err}");
    assert!(out.contains("kind=init"));
    let prompt = fx.repo.join(".dev/plans/roundtrip.prompt.md");
    assert_eq!(fs::read(&prompt).unwrap(), valid_prompt("v1"));
    let d1 = sha256_hex(&valid_prompt("v1"));

    let bytes_v2 = write_bytes_file(&fx.repo, "v2.bytes", &valid_prompt("v2"));
    let (ok, _out, err) = transition(
        &fx.repo,
        &["refresh", ".dev/plans/roundtrip.prompt.md", &bytes_v2, &d1],
    );
    assert!(ok, "{err}");
    // Native atomic replacement landed the new bytes exactly, with no partial write
    // or leftover candidate file beside the target.
    assert_eq!(fs::read(&prompt).unwrap(), valid_prompt("v2"));
    let d2 = sha256_hex(&valid_prompt("v2"));

    let bytes_v3 = write_bytes_file(&fx.repo, "v3.bytes", &valid_prompt("v3"));
    let (ok, _out, err) = transition(
        &fx.repo,
        &["install", ".dev/plans/roundtrip.prompt.md", &bytes_v3, &d2],
    );
    assert!(ok, "{err}");
    assert_eq!(fs::read(&prompt).unwrap(), valid_prompt("v3"));
    let d3 = sha256_hex(&valid_prompt("v3"));

    // `generation` rewrites the prompt's own bytes in place; it needs a generation
    // table already present, so seed one via `install` first.
    let table = "## Goal\n\n- Goal\n\n## Requirements\n\n- Req\n\n## Tasks\n\n- Task\n\n## Test Plan\n\n- Test plan\n\n## Status\n\n### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n";
    let bytes_table = write_bytes_file(&fx.repo, "table.bytes", table.as_bytes());
    let (ok, _out, err) = transition(
        &fx.repo,
        &[
            "install",
            ".dev/plans/roundtrip.prompt.md",
            &bytes_table,
            &d3,
        ],
    );
    assert!(ok, "{err}");
    let d4 = sha256_hex(table.as_bytes());

    let contract = "a".repeat(64);
    let task = tid(24);
    let (ok, out, err) = transition(
        &fx.repo,
        &[
            "generation",
            ".dev/plans/roundtrip.prompt.md",
            &task,
            &contract,
            "init",
            &d4,
        ],
    );
    assert!(ok, "{err}");
    assert!(out.contains("kind=init"));
    let final_text = fs::read_to_string(&prompt).unwrap();
    assert!(final_text.contains(&format!("| {task} | 1 | {contract} | init |")));
}

#[test]
fn real_binary_recovers_from_injected_crash_before_prompt_write() {
    let fx = fixture();
    let bytes_v1 = write_bytes_file(&fx.repo, "v1.bytes", &valid_prompt("v1"));
    let (ok, _out, err) = transition(
        &fx.repo,
        &["init", ".dev/plans/crash1.prompt.md", &bytes_v1],
    );
    assert!(ok, "{err}");
    let prompt = fx.repo.join(".dev/plans/crash1.prompt.md");
    let d1 = sha256_hex(&valid_prompt("v1"));

    let bytes_v2 = write_bytes_file(&fx.repo, "crash1-v2.bytes", &valid_prompt("v2"));
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(&fx.repo)
        .args([
            "test-first-transition",
            "refresh",
            ".dev/plans/crash1.prompt.md",
            &bytes_v2,
            &d1,
        ])
        .env("GAL_TEST_FIRST_TRANSITION_FAIL_AT", "before-prompt-write")
        .output()
        .unwrap();
    assert!(!output.status.success());
    // The crash landed before the prompt's own bytes were touched: still v1.
    assert_eq!(fs::read(&prompt).unwrap(), valid_prompt("v1"));

    // A clean retry recovers (discards the stale `prepared` record) and completes.
    let (ok, _out, err) = transition(
        &fx.repo,
        &["refresh", ".dev/plans/crash1.prompt.md", &bytes_v2, &d1],
    );
    assert!(ok, "{err}");
    assert_eq!(fs::read(&prompt).unwrap(), valid_prompt("v2"));
}

#[test]
fn real_binary_recovers_from_injected_crash_after_prompt_write() {
    let fx = fixture();
    let bytes_v1 = write_bytes_file(&fx.repo, "v1.bytes", &valid_prompt("v1"));
    let (ok, _out, err) = transition(
        &fx.repo,
        &["init", ".dev/plans/crash2.prompt.md", &bytes_v1],
    );
    assert!(ok, "{err}");
    let prompt = fx.repo.join(".dev/plans/crash2.prompt.md");
    let d1 = sha256_hex(&valid_prompt("v1"));

    let bytes_v2 = write_bytes_file(&fx.repo, "crash2-v2.bytes", &valid_prompt("v2"));
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(&fx.repo)
        .args([
            "test-first-transition",
            "refresh",
            ".dev/plans/crash2.prompt.md",
            &bytes_v2,
            &d1,
        ])
        .env("GAL_TEST_FIRST_TRANSITION_FAIL_AT", "after-prompt-write")
        .output()
        .unwrap();
    assert!(!output.status.success());
    // The crash landed after the native replace committed the new bytes to disk,
    // even though the journal was not yet finalized as `committed`.
    assert_eq!(fs::read(&prompt).unwrap(), valid_prompt("v2"));

    // The next transition (any transition, not just a retry of the same one) rolls
    // the journal forward to `committed` without re-mutating the prompt, then
    // proceeds normally from the now-current digest.
    let d2 = sha256_hex(&valid_prompt("v2"));
    let bytes_v3 = write_bytes_file(&fx.repo, "crash2-v3.bytes", &valid_prompt("v3"));
    let (ok, _out, err) = transition(
        &fx.repo,
        &["refresh", ".dev/plans/crash2.prompt.md", &bytes_v3, &d2],
    );
    assert!(ok, "{err}");
    assert_eq!(fs::read(&prompt).unwrap(), valid_prompt("v3"));
}

#[test]
fn real_binary_generation_anchors_on_status_section_heading_ignoring_prose_mention() {
    let fx = fixture();
    let table = "# Prompt\n\nDiscussion about ### Test-First Generations in prose before status.\n\n## Status\n\n### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n";
    let bytes_table = write_bytes_file(&fx.repo, "table_prose.bytes", table.as_bytes());
    let bytes_v1 = write_bytes_file(&fx.repo, "v1.bytes", b"v1\n");
    let (ok, _out, err) = transition(&fx.repo, &["init", ".dev/plans/prose.prompt.md", &bytes_v1]);
    assert!(ok, "{err}");
    let prompt = fx.repo.join(".dev/plans/prose.prompt.md");
    let d1 = sha256_hex(b"v1\n");

    let (ok, _out, err) = transition(
        &fx.repo,
        &["install", ".dev/plans/prose.prompt.md", &bytes_table, &d1],
    );
    assert!(ok, "{err}");
    let d2 = sha256_hex(table.as_bytes());

    let contract = "b".repeat(64);
    let task = tid(26);
    let (ok, out, err) = transition(
        &fx.repo,
        &[
            "generation",
            ".dev/plans/prose.prompt.md",
            &task,
            &contract,
            "init",
            &d2,
        ],
    );
    assert!(ok, "stdout: {out}\nstderr: {err}");
    let final_text = fs::read_to_string(&prompt).unwrap();
    assert!(final_text.contains(&format!("| {task} | 1 | {contract} | init |")));
}

/// EF-01 round trip: two generation rows written through the real
/// producer (the compiled `gal test-first-transition generation` subcommand,
/// which calls `append_generation`) and read back through the real evaluator
/// (`pipeline::test_first_evidence::evaluate`, the same function
/// `converge_check.rs` calls). Asserts the desired end state — the evaluator
/// resolves the current generation to `2`, the position of the newest row in
/// an oldest-first ledger. `append_generation` currently inserts each new row
/// immediately after the table separator instead, so the ledger ends up
/// newest-first and the evaluator's position check rejects it with
/// "generation gap or reorder" before it ever resolves a generation. This
/// assertion is expected red until the production fix lands; no receipts
/// are supplied, so `validate_receipts` and overall Pass/Fail are out of
/// scope for this probe — only `evaluation.generation` is asserted.
#[test]
fn real_producer_generation_rows_read_back_by_real_evaluator_reorder() {
    let fx = fixture();
    let marker_table = "Pipeline Contract: test-first-v1\n\n# Prompt\n\n## Goal\n\n- Goal\n\n## Requirements\n\n- Req\n\n## Tasks\n\n- Task\n\n## Test Plan\n\n- Test plan\n\n## Status\n\n### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n";
    let bytes_v1 = write_bytes_file(&fx.repo, "gen-round-v1.bytes", &valid_prompt("v1"));
    let (ok, _out, err) = transition(
        &fx.repo,
        &["init", ".dev/plans/gen-round.prompt.md", &bytes_v1],
    );
    assert!(ok, "{err}");
    let prompt = fx.repo.join(".dev/plans/gen-round.prompt.md");
    let d1 = sha256_hex(&valid_prompt("v1"));

    let bytes_table = write_bytes_file(&fx.repo, "gen-round-table.bytes", marker_table.as_bytes());
    let (ok, _out, err) = transition(
        &fx.repo,
        &[
            "install",
            ".dev/plans/gen-round.prompt.md",
            &bytes_table,
            &d1,
        ],
    );
    assert!(ok, "{err}");
    let d2 = sha256_hex(marker_table.as_bytes());

    // A minimal `Required` contract. Both generation rows are written with its
    // digest, so the evaluator's own digest-mismatch check never fires — the
    // only failure this probe proves is the position/reorder check.
    let contract = pipeline::task_spec::TaskContract {
        plan_slug: "gen-round".to_string(),
        task_id: tid(29),
        applicability: pipeline::task_spec::Applicability::Required,
        seam: "crates/cli/src/commands/test_first_transition.rs".to_string(),
        expected_failures: vec!["EF-01".to_string()],
        production_paths: vec!["crates/cli/src/commands/test_first_transition.rs".to_string()],
        test_paths: vec!["crates/cli/tests/test_first_transition.rs".to_string()],
        scaffold: "not-required".to_string(),
        not_applicable_rationale: String::new(),
        non_red_probe: String::new(),
        tp_rows: vec![],
    };
    let digest = contract.compute_digest();

    let (ok, out, err) = transition(
        &fx.repo,
        &[
            "generation",
            ".dev/plans/gen-round.prompt.md",
            &contract.task_id,
            &digest,
            "init",
            &d2,
        ],
    );
    assert!(ok, "stdout: {out}\nstderr: {err}");
    let d3 = sha256_hex(&fs::read(&prompt).unwrap());

    let (ok, out, err) = transition(
        &fx.repo,
        &[
            "generation",
            ".dev/plans/gen-round.prompt.md",
            &contract.task_id,
            &digest,
            "phase-rerun",
            &d3,
        ],
    );
    assert!(ok, "stdout: {out}\nstderr: {err}");

    let final_text = fs::read_to_string(&prompt).unwrap();
    let evaluation =
        pipeline::test_first_evidence::evaluate(&pipeline::test_first_evidence::EvaluationInput {
            plan_slug: &contract.plan_slug,
            task_id: &contract.task_id,
            prompt_body: &final_text,
            contract: Some(&contract),
            receipts: &[],
            output_envelopes: &Default::default(),
            disputes: &[],
            retry_count: 0,
            phases: &[],
        });

    // Desired end state: the evaluator resolves the newest of two rows as
    // generation 2. Currently false — `validate_generation` rejects the
    // newest-first ledger before it resolves anything, so `generation` stays
    // `None` and the mismatch message below carries the locked EF-01 text.
    assert_eq!(evaluation.generation, Some(2), "{:?}", evaluation.errors);
}

/// EF-01 round trip for R22: a ledger whose first row carries a task
/// contract's old digest, followed by a `contract-change` row carrying the
/// corrected contract's new digest, written through the real producer (the
/// compiled `gal test-first-transition generation` subcommand, which calls
/// `append_generation`) and read back through the real evaluator
/// (`pipeline::test_first_evidence::evaluate`, the same function
/// `converge_check.rs` calls). Desired end state: the current contract binds
/// only to the *last* row, so the corrected contract's digest matches the
/// newest row and the ledger is accepted. Currently false —
/// `validate_generation` compares every row against the current contract
/// digest, so the first row's old digest fails even though its own
/// `contract-change` successor correctly declares the switch. The declared
/// recovery reason is therefore unreachable: correcting a task's contract
/// after its first generation row can never pass.
#[test]
fn real_producer_generation_contract_change_reason_unreachable() {
    let fx = fixture();
    let marker_table = "Pipeline Contract: test-first-v1\n\n# Prompt\n\n## Status\n\n### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n";
    let bytes_v1 = write_bytes_file(&fx.repo, "gen-cc-v1.bytes", b"v1\n");
    let (ok, _out, err) = transition(
        &fx.repo,
        &["init", ".dev/plans/gen-cc.prompt.md", &bytes_v1],
    );
    assert!(ok, "{err}");
    let prompt = fx.repo.join(".dev/plans/gen-cc.prompt.md");
    let d1 = sha256_hex(b"v1\n");

    let bytes_table = write_bytes_file(&fx.repo, "gen-cc-table.bytes", marker_table.as_bytes());
    let (ok, _out, err) = transition(
        &fx.repo,
        &["install", ".dev/plans/gen-cc.prompt.md", &bytes_table, &d1],
    );
    assert!(ok, "{err}");
    let d2 = sha256_hex(marker_table.as_bytes());

    let task = tid(31);
    let contract_old = pipeline::task_spec::TaskContract {
        plan_slug: "gen-cc".to_string(),
        task_id: task.clone(),
        applicability: pipeline::task_spec::Applicability::Required,
        seam: "crates/pipeline/src/test_first_evidence.rs".to_string(),
        expected_failures: vec!["EF-01".to_string()],
        production_paths: vec!["crates/pipeline/src/test_first_evidence.rs".to_string()],
        test_paths: vec!["crates/cli/tests/test_first_transition.rs".to_string()],
        scaffold: "not-required".to_string(),
        not_applicable_rationale: String::new(),
        non_red_probe: String::new(),
        tp_rows: vec![],
    };
    // The corrected contract: only `expected_failures` differs, mirroring the
    // real matcher correction this defect blocks. That single field
    // change is enough to change `compute_digest`'s output.
    let mut contract_new = contract_old.clone();
    contract_new.expected_failures = vec!["EF-02".to_string()];

    let digest_old = contract_old.compute_digest();
    let digest_new = contract_new.compute_digest();
    assert_ne!(digest_old, digest_new, "fixture contracts must differ");

    let (ok, out, err) = transition(
        &fx.repo,
        &[
            "generation",
            ".dev/plans/gen-cc.prompt.md",
            &task,
            &digest_old,
            "init",
            &d2,
        ],
    );
    assert!(ok, "stdout: {out}\nstderr: {err}");
    let d3 = sha256_hex(&fs::read(&prompt).unwrap());

    let (ok, out, err) = transition(
        &fx.repo,
        &[
            "generation",
            ".dev/plans/gen-cc.prompt.md",
            &task,
            &digest_new,
            "contract-change",
            &d3,
        ],
    );
    assert!(ok, "stdout: {out}\nstderr: {err}");

    let final_text = fs::read_to_string(&prompt).unwrap();
    let evaluation =
        pipeline::test_first_evidence::evaluate(&pipeline::test_first_evidence::EvaluationInput {
            plan_slug: "gen-cc",
            task_id: &task,
            prompt_body: &final_text,
            contract: Some(&contract_new),
            receipts: &[],
            output_envelopes: &Default::default(),
            disputes: &[],
            retry_count: 0,
            phases: &[],
        });

    // Desired end state: the corrected contract binds to the last row only,
    // so the ledger is accepted and generation resolves to 2. Currently
    // false — every row is checked against `contract_new`'s digest, so the
    // first (`init`, old-digest) row fails validation before a generation is
    // ever resolved, `evaluation.generation` stays `None`, and the assertion
    // failure below carries the locked EF-01 text via `evaluation.errors`.
    assert_eq!(evaluation.generation, Some(2), "{:?}", evaluation.errors);
}

/// Acceptance probe for contract-region invariance.
///
/// Under `test-first-transition refresh` (`phase-rerun`), mutations to the contract
/// region (e.g. `## Goal` or `## Requirements`) must be rejected prior to
/// journal, backup, or prompt writes, requiring an explicit `install` (`contract-change`)
/// transition instead.
///
/// Acceptance:
/// 1. Unauthorized contract-region edits under `refresh` fail with output indicating `contract-change` is required, leaving prompt and journal byte-identical (hashes match).
/// 2. Explicit `install` (`contract-change`) permits contract-region edits and succeeds.
/// 3. Legal `refresh` (`phase-rerun`) with edits outside the contract region (e.g. `## Status`) succeeds.
#[test]
fn contract_region_invariance_rejects_unauthorized_phase_rerun_contract_mutation() {
    let fx = fixture();

    let task = tid(10);
    let initial_prompt = format!(
        "\
# Task Spec: {task} (test)

## Goal

- {task} — Initial task goal

## Requirements

Initial requirement.

## Tasks

- [ ] {task} — Enforce contract-region invariance

## Test Plan

- Focused real-command test

## Status

- Status: in-progress
"
    );

    let bytes_v1 = write_bytes_file(&fx.repo, "cr_v1.bytes", initial_prompt.as_bytes());
    let (ok, _out, err) = transition(
        &fx.repo,
        &["init", ".dev/plans/contract_inv.prompt.md", &bytes_v1],
    );
    assert!(ok, "{err}");

    let prompt = fx.repo.join(".dev/plans/contract_inv.prompt.md");
    let d1 = sha256_hex(initial_prompt.as_bytes());
    let journal = fx
        .repo
        .join(".dev/pipeline/journal/contract_inv/transition.journal.tsv");

    let prompt_bytes_before = fs::read(&prompt).unwrap();
    let journal_bytes_before = if journal.exists() {
        fs::read(&journal).unwrap()
    } else {
        Vec::new()
    };
    let prompt_hash_before = sha256_hex(&prompt_bytes_before);
    let journal_hash_before = sha256_hex(&journal_bytes_before);

    // 1. Unauthorized contract-region mutation (modifying `## Goal`) attempted via `refresh` (`phase-rerun`).
    let mutated_contract_prompt = format!(
        "\
# Task Spec: {task} (test)

## Goal

- {task} — Mutated task goal without contract-change

## Requirements

Initial requirement.

## Tasks

- [ ] {task} — Enforce contract-region invariance

## Test Plan

- Focused real-command test

## Status

- Status: in-progress
"
    );
    let bytes_mutated = write_bytes_file(
        &fx.repo,
        "cr_mutated.bytes",
        mutated_contract_prompt.as_bytes(),
    );

    let (ok, out, err) = transition(
        &fx.repo,
        &[
            "refresh",
            ".dev/plans/contract_inv.prompt.md",
            &bytes_mutated,
            &d1,
        ],
    );

    // Assert that refresh failed and error contains "contract-change", and files are byte-identical.
    assert!(
        !ok && (err.contains("contract-change") || out.contains("contract-change")),
        "expected phase-rerun with contract mutation to fail requiring contract-change, got ok={ok}, stdout={out}, stderr={err}"
    );

    let prompt_bytes_after = fs::read(&prompt).unwrap();
    let journal_bytes_after = if journal.exists() {
        fs::read(&journal).unwrap()
    } else {
        Vec::new()
    };
    let prompt_hash_after = sha256_hex(&prompt_bytes_after);
    let journal_hash_after = sha256_hex(&journal_bytes_after);

    assert_eq!(
        prompt_hash_before, prompt_hash_after,
        "prompt bytes must remain byte-identical after rejected refresh"
    );
    assert_eq!(
        journal_hash_before, journal_hash_after,
        "journal bytes must remain byte-identical after rejected refresh"
    );

    // 2. Explicit `install` (`contract-change`) permits contract-region edits and succeeds.
    let (ok_install, out_install, err_install) = transition(
        &fx.repo,
        &[
            "install",
            ".dev/plans/contract_inv.prompt.md",
            &bytes_mutated,
            &d1,
        ],
    );
    assert!(
        ok_install,
        "explicit contract-change install must succeed, got stdout={out_install}, stderr={err_install}"
    );
    assert_eq!(
        fs::read(&prompt).unwrap(),
        mutated_contract_prompt.as_bytes()
    );
    let d2 = sha256_hex(mutated_contract_prompt.as_bytes());

    // 3. Legal `refresh` (`phase-rerun`) with edits outside the contract region (e.g. `## Status`) succeeds.
    let status_edited_prompt = format!(
        "\
# Task Spec: {task} (test)

## Goal

- {task} — Mutated task goal without contract-change

## Requirements

Initial requirement.

## Tasks

- [ ] {task} — Enforce contract-region invariance

## Test Plan

- Focused real-command test

## Status

- Status: completed
"
    );
    let bytes_status_edited = write_bytes_file(
        &fx.repo,
        "cr_status_edited.bytes",
        status_edited_prompt.as_bytes(),
    );
    let (ok_refresh, out_refresh, err_refresh) = transition(
        &fx.repo,
        &[
            "refresh",
            ".dev/plans/contract_inv.prompt.md",
            &bytes_status_edited,
            &d2,
        ],
    );
    assert!(
        ok_refresh,
        "legal phase-rerun status edit must succeed, got stdout={out_refresh}, stderr={err_refresh}"
    );
    assert_eq!(fs::read(&prompt).unwrap(), status_edited_prompt.as_bytes());
}

#[test]
fn contract_region_invariance_rejects_prompt_with_missing_required_section() {
    let fx = fixture();

    let task = tid(10);
    let incomplete_prompt = format!(
        "\
# Task Spec: {task} (test)

## Goal

- {task} — Initial task goal

## Requirements

Initial requirement.

## Status

- Status: in-progress
"
    );

    let bytes_v1 = write_bytes_file(
        &fx.repo,
        "missing_sec_v1.bytes",
        incomplete_prompt.as_bytes(),
    );
    let (ok, _out, err) = transition(
        &fx.repo,
        &["init", ".dev/plans/missing_sec.prompt.md", &bytes_v1],
    );
    assert!(ok, "{err}");

    let d1 = sha256_hex(incomplete_prompt.as_bytes());
    let (ok, out, err) = transition(
        &fx.repo,
        &[
            "refresh",
            ".dev/plans/missing_sec.prompt.md",
            &bytes_v1,
            &d1,
        ],
    );

    assert!(
        !ok,
        "expected phase-rerun on prompt with missing contract section to fail, got ok={ok}, stdout={out}, stderr={err}"
    );
}

#[test]
fn legacy_bootstrap_producer_is_unavailable_and_writes_no_journal() {
    let fx = fixture();
    let receipt = fx.repo.join("transition.receipt");
    let (ok, out, err) = bootstrap(&fx.repo, Some(&receipt));
    let combined = format!("{out}\n{err}");
    assert!(
        !ok,
        "legacy-bootstrap producer must be unavailable and rejected, combined output: {combined}"
    );
    assert!(
        combined.contains("legacy-bootstrap"),
        "rejection message must reference legacy-bootstrap, combined output: {combined}"
    );
    let journal = journal_path(&fx.repo);
    assert!(
        !journal.exists(),
        "legacy-bootstrap must write no journal file, found: {}",
        journal.display()
    );
}

#[test]
fn historical_legacy_bootstrap_journal_rows_remain_readable() {
    let fx = fixture();
    let task = tid(23);
    let marked = format!(
        "Pipeline Contract: test-first-v1\n\n# Prompt\n\n## Goal\n\n- Goal\n\n## Requirements\n\n- Req\n\n## Tasks\n\n- [ ] {task} — historical reader compatibility\n\n## Files to Create or Modify\n\n- `sample.txt`\n\n## Test Plan\n\n- Test plan\n\n## Status\n\nCurrent Task: —\n"
    )
    .into_bytes();
    let marked_digest = sha256_hex(&marked);
    fs::write(&fx.prompt, &marked).unwrap();
    fs::write(&fx.source, source_text(&marked_digest)).unwrap();
    git(&fx.repo, &["add", "."]);
    git(
        &fx.repo,
        &[
            "commit",
            "-q",
            "-m",
            "historical legacy-bootstrap journal fixture",
        ],
    );

    let journal = journal_path(&fx.repo);
    fs::create_dir_all(journal.parent().unwrap()).unwrap();
    fs::write(
        &journal,
        format!("committed\tlegacy-bootstrap\t-\t{marked_digest}\n"),
    )
    .unwrap();
    git(&fx.repo, &["add", "."]);
    git(&fx.repo, &["commit", "-q", "-m", "commit journal"]);

    let receipt_path = fx.repo.join("historical-boundary.receipt.md");
    let boundary = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(&fx.repo)
        .args([
            "boundary-check",
            ".dev/plans/sample.prompt.md",
            "--task",
            &task,
            "--receipt",
            "historical-boundary.receipt.md",
        ])
        .output()
        .unwrap();

    let receipt = fs::read_to_string(&receipt_path).unwrap_or_default();
    assert!(
        boundary.status.success(),
        "boundary-check must succeed reading historical legacy-bootstrap journal row, stderr: {}",
        String::from_utf8_lossy(&boundary.stderr)
    );
    assert!(
        receipt.contains("| digest-binding | pass |"),
        "historical legacy-bootstrap journal row must pass digest binding, receipt:\n{receipt}"
    );
}

/// The orphan-candidate guard must actually fire. It used to read the journal
/// directory, where a candidate is never published, so a leftover candidate
/// beside the prompt was invisible and the next transition proceeded over it.
#[test]
fn orphan_candidate_beside_the_prompt_blocks_the_next_transition() {
    let fx = fixture();
    let bytes_v1 = write_bytes_file(&fx.repo, "v1.bytes", &valid_prompt("v1"));
    let (ok, _out, err) = transition(
        &fx.repo,
        &["init", ".dev/plans/orphan.prompt.md", &bytes_v1],
    );
    assert!(ok, "{err}");
    let prompt = fx.repo.join(".dev/plans/orphan.prompt.md");
    let d1 = sha256_hex(&valid_prompt("v1"));

    let orphan = fx
        .repo
        .join(".dev/plans/.orphan.prompt.md.candidate.stale-1");
    fs::write(&orphan, b"partial\n").unwrap();

    let bytes_v2 = write_bytes_file(&fx.repo, "v2.bytes", &valid_prompt("v2"));
    let (ok, _out, err) = transition(
        &fx.repo,
        &["refresh", ".dev/plans/orphan.prompt.md", &bytes_v2, &d1],
    );
    assert!(!ok, "orphan candidate must block the transition");
    assert!(err.contains("ambiguous transition candidate"), "{err}");
    assert_eq!(
        fs::read(&prompt).unwrap(),
        valid_prompt("v1"),
        "a blocked transition must not touch the prompt"
    );
}

/// The scan is plan-scoped by filename prefix. `.dev/plans/` holds every plan's
/// prompt, so one plan's orphan must not block another plan's transition. A
/// candidate under the journal directory is not a candidate at all, because
/// nothing ever writes one there.
#[test]
fn orphan_candidate_for_another_prompt_does_not_block() {
    let fx = fixture();
    let bytes_v1 = write_bytes_file(&fx.repo, "v1.bytes", &valid_prompt("v1"));
    let (ok, _out, err) = transition(
        &fx.repo,
        &["init", ".dev/plans/scoped.prompt.md", &bytes_v1],
    );
    assert!(ok, "{err}");
    let d1 = sha256_hex(&valid_prompt("v1"));

    fs::write(
        fx.repo
            .join(".dev/plans/.other.prompt.md.candidate.stale-1"),
        b"partial\n",
    )
    .unwrap();
    fs::create_dir_all(fx.repo.join(".dev/pipeline/journal/scoped")).unwrap();
    fs::write(
        fx.repo
            .join(".dev/pipeline/journal/scoped/.scoped.prompt.md.candidate.stale-1"),
        b"partial\n",
    )
    .unwrap();

    let bytes_v2 = write_bytes_file(&fx.repo, "v2.bytes", &valid_prompt("v2"));
    let (ok, _out, err) = transition(
        &fx.repo,
        &["refresh", ".dev/plans/scoped.prompt.md", &bytes_v2, &d1],
    );
    assert!(ok, "{err}");
    assert_eq!(
        fs::read(fx.repo.join(".dev/plans/scoped.prompt.md")).unwrap(),
        valid_prompt("v2")
    );
}
