//! Real-binary coverage for the `TestFirstEvaluation` integration in
//! `gal pipeline-converge-check`. Focuses on the two checks this integration
//! adds (`boundary-evidence`, `test-first-evaluation`) rather than re-proving
//! the pure evaluator logic its own dedicated test suite already covers
//! exhaustively, or the pre-existing bookkeeping checks (three-surface,
//! task-commit, cursor) that are out of scope here.

use pipeline::task_spec::{encode_base64url_unpadded, extract_plan_slug};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, iter};
use tempfile::TempDir;

fn tid(n: u32) -> String {
    format!("T-{n:02}")
}

fn run_gal(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap()
}

fn write_attempt_log(dir: &Path, task: &str, phase: &str, terminal_state: &str, seq: u32) {
    fs::create_dir_all(dir).unwrap();
    let filename = format!(
        "{:010}-000000001-{seq:06}-{task}-{phase}-claude.log",
        1700000000 + seq as u64
    );
    let content = format!(
        "GAL-DISPATCH-LOG v1\n\
         timestamp_start: 2026-08-21T00:00:00Z\n\
         timestamp_end:   2026-08-21T00:00:01Z\n\
         duration_ms:     1\n\
         executor:        claude\n\
         phase:           {phase}\n\
         task_id:         {task}\n\
         git_branch:      main\n\
         git_head:        abc1234\n\
         exit_code:       0\n\
         actual_model:    m\n\
         terminal_state:  {terminal_state}\n\
         session_id:      none\n\
         ---STDOUT---\n\
         \n\
         ---STDERR---\n\
         \n"
    );
    fs::write(dir.join(filename), content).unwrap();
}

/// Locked task-contract block matching the exact `/refining-plan` grammar
/// `parse_task_contract` expects, for a single-EF acceptance-only task.
fn contract_block(task: &str, ef_line: &str) -> String {
    format!(
        "- [x] {task} — placeholder.\n  \
         - Test-first: required\n  \
         - Seam: `crates/cli/src/commands/converge_check.rs`\n  \
         - Expected failures:\n    \
         - {ef_line}\n  \
         - Production Paths: `crates/cli/src/commands/converge_check.rs`\n  \
         - Test Paths: `crates/cli/tests/test_first_converge.rs`\n  \
         - Scaffold: not-required\n"
    )
}

fn ef_line(matcher: &[u8]) -> String {
    format!(
        "EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64={} — locked",
        encode_base64url_unpadded(matcher)
    )
}

/// Builds a git-initialized repo with a prompt at `.dev/plans/<slug>.prompt.md`.
/// `extra_status` is inserted inside `## Status` (generation ledger, Deviations
/// table with any dispute rows); `tasks_body` is the `## Tasks` section content.
fn write_prompt(
    slug: &str,
    marked: bool,
    extra_status: &str,
    tasks_body: &str,
) -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().to_path_buf();
    Command::new("git")
        .current_dir(&repo)
        .args(["init", "-q", "-b", "main"])
        .output()
        .unwrap();
    let marker = if marked {
        "\n\nPipeline Contract: test-first-v1"
    } else {
        ""
    };
    let text = format!(
        "# Plan Prompt: {slug}{marker}\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n{extra_status}\n## Tasks\n\n{tasks_body}\n"
    );
    let prompt_path = repo.join(".dev/plans").join(format!("{slug}.prompt.md"));
    fs::create_dir_all(prompt_path.parent().unwrap()).unwrap();
    fs::write(&prompt_path, text).unwrap();
    (temp, prompt_path)
}

fn write_passing_boundary_receipt(repo: &Path, slug: &str, task: &str) {
    let path = repo
        .join(".dev/pipeline/receipts")
        .join(slug)
        .join(format!("{task}-boundary-check.receipt.md"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "# boundary-check receipt\n\noverall: pass\n").unwrap();
}

fn extract_receipt_line<'a>(receipt: &'a str, check_name: &str) -> Option<&'a str> {
    receipt
        .lines()
        .find(|line| line.starts_with(&format!("| {check_name} |")))
}

fn converge_check(repo: &Path, prompt: &Path, task: &str) -> (std::process::Output, String) {
    let receipt_path = repo.join("converge.receipt.md");
    let output = run_gal(
        repo,
        &[
            "pipeline-converge-check",
            prompt.to_str().unwrap(),
            "--task",
            task,
            "--receipt",
            receipt_path.to_str().unwrap(),
        ],
    );
    let receipt = fs::read_to_string(&receipt_path).unwrap_or_default();
    (output, receipt)
}

fn run_real_dispatch(repo: &Path, slug: &str, task: &str) -> std::process::Output {
    let home = repo.join("fake-home");
    let bin = repo.join("fake-bin");
    fs::create_dir_all(home.join(".gal/config")).unwrap();
    fs::create_dir_all(&bin).unwrap();
    let routing = home.join(".gal/config/config.json");
    fs::write(
        &routing,
        r#"{"executorRouting":{"pipeline":{"CODER":{"executor":"cmd","model":"test"}}}}"#,
    )
    .unwrap();
    fs::write(
        bin.join("cmd.cmd"),
        "@echo off\r\necho changed > dispatch-touched.txt\r\nexit /b 0\r\n",
    )
    .unwrap();
    fs::write(
        bin.join("cmd.bat"),
        "@echo off\r\necho changed > dispatch-touched.txt\r\nexit /b 0\r\n",
    )
    .unwrap();
    fs::write(
        bin.join("cmd"),
        "#!/bin/sh\necho changed > dispatch-touched.txt\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let p = bin.join("cmd");
        let mut perms = fs::metadata(&p).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&p, perms).unwrap();
    }

    let path = env::var_os("PATH").unwrap_or_default();
    let path = env::join_paths(iter::once(bin).chain(env::split_paths(&path))).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_gal"));
    command
        .current_dir(repo)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("PATH", path)
        .args([
            "dispatch",
            "--phase",
            "implement",
            "--task",
            task,
            "--routing",
            routing.to_str().unwrap(),
        ])
        .stdin(Stdio::piped());
    let mut child = command.spawn().unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"echo changed > dispatch-touched.txt\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let source_dir = repo.join(".dev/executor-logs");
    let target_dir = source_dir.join(slug);
    fs::create_dir_all(&target_dir).unwrap();
    for entry in fs::read_dir(&source_dir).unwrap().filter_map(Result::ok) {
        if entry.path().extension().is_some_and(|ext| ext == "log") {
            fs::rename(entry.path(), target_dir.join(entry.file_name())).unwrap();
        }
    }
    output
}

#[test]
fn markerless_task_skips_test_first_checks_entirely() {
    let task = tid(1);
    let tasks_body = format!("- [x] {task} — legacy placeholder.\n");
    let (temp, prompt) = write_prompt("markerless-plan", false, "", &tasks_body);
    let (_output, receipt) = converge_check(temp.path(), &prompt, &task);
    assert!(
        extract_receipt_line(&receipt, "boundary-evidence").is_none(),
        "{receipt}"
    );
    assert!(
        extract_receipt_line(&receipt, "test-first-evaluation").is_none(),
        "{receipt}"
    );
}

/// `scaffold` is a first-class dispatch phase for a marked task, so the gate
/// must emit a `phase-scaffold` row for it. The per-phase loop used to iterate
/// a local three-phase list regardless of contract, so an unfinished scaffold
/// dispatch was invisible to the gate.
#[test]
fn marked_task_emits_a_scaffold_phase_row() {
    let task = tid(9);
    let ef = ef_line(b"needle");
    let tasks_body = contract_block(&task, &ef);
    let slug = "marked-plan-scaffold";
    let (temp, prompt) = write_prompt(slug, true, "", &tasks_body);
    // An unterminated scaffold dispatch. The gate can only fail closed on it if
    // it asks about the scaffold phase in the first place.
    let log_dir = temp.path().join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "scaffold", "started", 1);

    let (_output, receipt) = converge_check(temp.path(), &prompt, &task);
    let line = extract_receipt_line(&receipt, "phase-scaffold")
        .unwrap_or_else(|| panic!("marked task must emit a phase-scaffold row:\n{receipt}"));
    assert!(
        line.contains("| fail |"),
        "an unterminated scaffold dispatch must fail closed: {line}"
    );
}

/// The same loop must stay exactly three phases for a legacy plan.
#[test]
fn markerless_task_emits_no_scaffold_phase_row() {
    let task = tid(10);
    let tasks_body = format!("- [x] {task} — legacy placeholder.\n");
    let (temp, prompt) = write_prompt("markerless-plan-scaffold", false, "", &tasks_body);
    let (_output, receipt) = converge_check(temp.path(), &prompt, &task);
    assert!(
        extract_receipt_line(&receipt, "phase-scaffold").is_none(),
        "{receipt}"
    );
    for phase in ["implement", "test", "audit"] {
        assert!(
            extract_receipt_line(&receipt, &format!("phase-{phase}")).is_some(),
            "legacy plan must keep its {phase} row:\n{receipt}"
        );
    }
}

#[test]
fn marked_task_missing_boundary_receipt_fails_boundary_evidence() {
    let task = tid(2);
    let ef = ef_line(b"needle");
    let tasks_body = contract_block(&task, &ef);
    let (temp, prompt) = write_prompt("marked-plan-2", true, "", &tasks_body);
    let (_output, receipt) = converge_check(temp.path(), &prompt, &task);
    let line = extract_receipt_line(&receipt, "boundary-evidence").unwrap();
    assert!(line.contains("| fail |"), "{line}");
    assert!(line.contains("missing boundary receipt"), "{line}");
}

#[test]
fn marked_task_missing_probe_receipts_fails_evaluation() {
    let task = tid(3);
    let ef = ef_line(b"needle");
    let tasks_body = contract_block(&task, &ef);
    let slug = "marked-plan-3";
    let (temp, prompt) = write_prompt(slug, true, "", &tasks_body);
    write_passing_boundary_receipt(temp.path(), slug, &task);
    let (_output, receipt) = converge_check(temp.path(), &prompt, &task);
    let line = extract_receipt_line(&receipt, "test-first-evaluation").unwrap();
    assert!(line.contains("| fail |"), "{line}");
    assert!(
        line.contains("verdict:Fail") || line.contains("verdict=Fail") || line.contains("Fail"),
        "{line}"
    );
}

/// Full integration path: a real red probe and a real green probe (generated
/// through the actual `gal test-first-probe run` binary, not hand-serialized),
/// a satisfied generation ledger, matching contract digest, and completed
/// implement/test/audit attempt logs. Proves the whole wiring converges to
/// Pass when every piece of real evidence lines up.
#[test]
fn marked_task_with_real_matching_red_and_green_probes_and_no_disputes_passes() {
    let task = tid(4);
    let matcher = b"needle";
    let ef = ef_line(matcher);
    let tasks_body = contract_block(&task, &ef);
    let slug = "marked-plan-4";

    // Compute the contract digest the same way the evaluator will: parse the
    // locked task block with the same parser, then hash it.
    let (temp, prompt) = write_prompt(slug, true, "", &tasks_body);
    let repo = temp.path().to_path_buf();
    let prompt_text = fs::read_to_string(&prompt).unwrap();
    let plan_slug = extract_plan_slug(&prompt.to_string_lossy());
    let contract =
        pipeline::task_spec::parse_task_contract(&prompt_text, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();

    // Now rewrite the prompt with the generation row present (self-consistent
    // digest, computed from the exact same locked contract text above).
    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let (_temp2, prompt2) = write_prompt(slug, true, &extra_status, &tasks_body);
    // write_prompt made a fresh temp dir; instead overwrite the ORIGINAL repo's
    // prompt in place so the receipts/log paths below stay consistent.
    fs::write(&prompt, fs::read_to_string(&prompt2).unwrap()).unwrap();

    write_passing_boundary_receipt(&repo, slug, &task);
    let log_dir = repo.join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "implement", "completed", 1);
    write_attempt_log(&log_dir, &task, "test", "completed", 2);
    write_attempt_log(&log_dir, &task, "audit", "completed", 3);

    // Identity (R16) is (id, selector, argv_b64, env_digest, expected_failure_ref,
    // timeout_ms) — red and green must share the exact same argv, matching the
    // real workflow of re-running the identical probe command after the fix
    // lands. The script branches on a marker file's existence, so the SAME argv
    // produces genuinely different output/exit-code (and therefore a distinct
    // output blob, avoiding a collision on the content-addressed publish path)
    // between the two invocations, exactly as it would across a real red/green
    // rerun where only the implementation under test changed.
    let script = "test -f fixed.marker && exit 0 || { printf needle; exit 1; }";
    let argv = encode_argv(&["sh", "-c", script]);

    // Real red probe: marker absent, so it exits nonzero with the locked matcher.
    let red = run_gal(
        &repo,
        &[
            "test-first-probe",
            "run",
            &plan_slug,
            &task,
            "1",
            &digest,
            "test",
            "red",
            "P-01",
            "acceptance",
            &argv,
            "2000",
            "--expected-failure",
            &ef,
        ],
    );
    assert!(
        red.status.success(),
        "{}",
        String::from_utf8_lossy(&red.stderr)
    );

    // Real green probe: identical argv, but the marker now exists, so the
    // same script takes the exit-0 branch instead.
    fs::write(repo.join("fixed.marker"), b"").unwrap();
    let green = run_gal(
        &repo,
        &[
            "test-first-probe",
            "run",
            &plan_slug,
            &task,
            "1",
            &digest,
            "green-rerun",
            "green",
            "P-01",
            "acceptance",
            &argv,
            "2000",
            "--expected-failure",
            &ef,
        ],
    );
    assert!(
        green.status.success(),
        "{}",
        String::from_utf8_lossy(&green.stderr)
    );

    let (_output, receipt) = converge_check(&repo, &prompt, &task);
    let line = extract_receipt_line(&receipt, "test-first-evaluation").unwrap();
    assert!(line.contains("| pass |"), "{receipt}");
}

#[test]
fn marked_task_with_open_dispute_fails_evaluation_and_reports_it_open() {
    let task = tid(5);
    let matcher = b"needle";
    let ef = ef_line(matcher);
    let tasks_body = contract_block(&task, &ef);
    let slug = "marked-plan-5";

    let (temp, prompt) = write_prompt(slug, true, "", &tasks_body);
    let repo = temp.path().to_path_buf();
    let prompt_text = fs::read_to_string(&prompt).unwrap();
    let plan_slug = extract_plan_slug(&prompt.to_string_lossy());
    let contract =
        pipeline::task_spec::parse_task_contract(&prompt_text, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();

    let refs = encode_base64url_unpadded(b"[]");
    let dispute_payload = format!(
        "dispute_id={task}-D01;generation=1;contract_digest={digest};lock_refs_b64={refs};evidence_refs_b64={refs};basis=behavior-wrong;classification=implementation-defect;status=open;retry_before=0;retry_after=1"
    );
    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n| 2026-08-21 | {task} | Implementation defect found. | {dispute_payload} |\n"
    );
    let (_temp2, prompt2) = write_prompt(slug, true, &extra_status, &tasks_body);
    fs::write(&prompt, fs::read_to_string(&prompt2).unwrap()).unwrap();

    write_passing_boundary_receipt(&repo, slug, &task);
    let log_dir = repo.join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "implement", "completed", 1);
    write_attempt_log(&log_dir, &task, "test", "completed", 2);
    write_attempt_log(&log_dir, &task, "audit", "completed", 3);

    let (_output, receipt) = converge_check(&repo, &prompt, &task);
    let line = extract_receipt_line(&receipt, "test-first-evaluation").unwrap();
    assert!(line.contains("| fail |"), "{receipt}");
    assert!(
        line.contains(&format!("{task}-D01")),
        "expected the open dispute id to surface in open_disputes: {line}"
    );
}

#[test]
fn marked_task_with_stale_contract_digest_fails_evaluation() {
    let task = tid(6);
    let matcher = b"needle";
    let ef = ef_line(matcher);
    let tasks_body = contract_block(&task, &ef);
    let slug = "marked-plan-6";

    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | 0000000000000000000000000000000000000000000000000000000000000000 | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let (temp, prompt) = write_prompt(slug, true, &extra_status, &tasks_body);
    let repo = temp.path().to_path_buf();

    write_passing_boundary_receipt(&repo, slug, &task);
    let log_dir = repo.join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "implement", "completed", 1);
    write_attempt_log(&log_dir, &task, "test", "completed", 2);
    write_attempt_log(&log_dir, &task, "audit", "completed", 3);

    let (_output, receipt) = converge_check(&repo, &prompt, &task);
    let line = extract_receipt_line(&receipt, "test-first-evaluation").unwrap();
    assert!(line.contains("| fail |"), "{receipt}");
}

#[test]
fn marked_task_with_missing_output_blob_fails_evaluation() {
    let task = tid(7);
    let matcher = b"needle";
    let ef = ef_line(matcher);
    let tasks_body = contract_block(&task, &ef);
    let slug = "marked-plan-7";

    let (temp, prompt) = write_prompt(slug, true, "", &tasks_body);
    let repo = temp.path().to_path_buf();
    let prompt_text = fs::read_to_string(&prompt).unwrap();
    let plan_slug = extract_plan_slug(&prompt.to_string_lossy());
    let contract =
        pipeline::task_spec::parse_task_contract(&prompt_text, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();

    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let (_temp2, prompt2) = write_prompt(slug, true, &extra_status, &tasks_body);
    fs::write(&prompt, fs::read_to_string(&prompt2).unwrap()).unwrap();

    write_passing_boundary_receipt(&repo, slug, &task);
    let log_dir = repo.join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "implement", "completed", 1);
    write_attempt_log(&log_dir, &task, "test", "completed", 2);
    write_attempt_log(&log_dir, &task, "audit", "completed", 3);

    let argv_b64 = encode_argv(&["sh", "-c", "exit 1"]);
    let record_json = format!(
        "{{\"id\":\"P-01\",\"selector\":\"acceptance\",\"argv_b64\":\"{argv_b64}\",\"env_digest\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"expected_failure_ref\":\"EF-01\",\"observed\":\"fail\",\"failure_class\":\"assertion\",\"termination\":\"exit:1\",\"timeout_ms\":1000,\"output_ref\":\"outputs/0000000000000000000000000000000000000000000000000000000000000000.bin\",\"output_digest\":\"0000000000000000000000000000000000000000000000000000000000000000\"}}"
    );
    let rec_b64 = encode_base64url_unpadded(record_json.as_bytes());

    let receipt_dir = repo
        .join(".dev/pipeline/receipts")
        .join(slug)
        .join(&task)
        .join(format!("g1-c{digest}"));
    fs::create_dir_all(&receipt_dir).unwrap();
    let receipt_content = format!(
        "plan={slug}\ntask={task}\ngeneration=1\ncontract_digest={digest}\nphase=test\nexpectation=red\nrunner_contract=test-first-probe-v1\nprobe_set_digest=17035ef9f34455d23d72b9ea60d3a0513ab90f5ef01f43214c735ea6ae950098\nverdict=fail\n\nprobe_record_b64={rec_b64}\n"
    );
    fs::write(receipt_dir.join("probe-red.receipt.md"), receipt_content).unwrap();

    let (_output, receipt) = converge_check(&repo, &prompt, &task);
    let line = extract_receipt_line(&receipt, "test-first-evaluation").unwrap();
    assert!(line.contains("| fail |"), "{receipt}");
    assert!(line.contains("missing runner output"), "{line}");
}

fn encode_argv(args: &[&str]) -> String {
    pipeline::task_spec::probe_evidence::encode_argv_b64(
        &args.iter().map(|a| (*a).to_string()).collect::<Vec<_>>(),
    )
    .unwrap()
}

/// A marked not-applicable task with complete `test`, `implement`, and `audit`
/// attempt-log evidence, plus a real `("test", "pass")` probe receipt for its
/// `Non-red probe:` field, must evaluate to Pass with those three phases
/// completed. Until the required phase list depends on applicability
/// (R18), this same evidence evaluates as a failure whose output contains
/// `phase evidence has gaps`, because `green-rerun` and `scaffold` — phases
/// a not-applicable task never produces — stay in the required list.
#[test]
fn marked_not_applicable_task_with_complete_evidence_passes_evaluation() {
    let task = tid(8);
    let tasks_body = format!(
        "- [x] {task} — Documentation update task.\n  \
         - Test-first: not-applicable — Documentation only change with no executable behavior\n  \
         - Non-red probe: `cargo test --doc`\n"
    );
    let slug = "marked-plan-8";

    let (temp, prompt) = write_prompt(slug, true, "", &tasks_body);
    let repo = temp.path().to_path_buf();
    let prompt_text = fs::read_to_string(&prompt).unwrap();
    let plan_slug = extract_plan_slug(&prompt.to_string_lossy());
    let contract =
        pipeline::task_spec::parse_task_contract(&prompt_text, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();

    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let (_temp2, prompt2) = write_prompt(slug, true, &extra_status, &tasks_body);
    fs::write(&prompt, fs::read_to_string(&prompt2).unwrap()).unwrap();

    write_passing_boundary_receipt(&repo, slug, &task);
    let log_dir = repo.join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "test", "completed", 1);
    write_attempt_log(&log_dir, &task, "implement", "completed", 2);
    write_attempt_log(&log_dir, &task, "audit", "completed", 3);

    // Real non-red probe through the actual `test-first-probe run` binary: a
    // trivially exiting script produces the exact `("test", "pass")` receipt
    // shape the not-applicable contract's `Non-red probe:` field describes,
    // not a hand-serialized stand-in.
    let argv = encode_argv(&["sh", "-c", "exit 0"]);
    let probe = run_gal(
        &repo,
        &[
            "test-first-probe",
            "run",
            &plan_slug,
            &task,
            "1",
            &digest,
            "test",
            "pass",
            "P-01",
            "non-red",
            &argv,
            "2000",
        ],
    );
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );

    let (_output, receipt) = converge_check(&repo, &prompt, &task);
    let line = extract_receipt_line(&receipt, "test-first-evaluation").unwrap();
    assert!(line.contains("| pass |"), "{receipt}");
    assert!(
        line.contains("completed_phases=[\"test\", \"implement\", \"audit\"]"),
        "{line}"
    );
}

#[test]
fn marked_task_completes_the_test_phase_from_a_probe_receipt_without_an_attempt_log() {
    // Regression: the test phase does not always dispatch. Under the documented
    // dispatch-necessity skip path, a task whose covering Test Plan rows are all
    // `grep`/`manual`/`documentation` has ORCHESTRATOR run the check in-process,
    // so no TESTER attempt log is ever written. Phase completion used to be read
    // from attempt logs alone, so such a task reported `implement` and `audit` as
    // out of order and could never converge — even with a valid canonical probe
    // receipt present. This fixture omits exactly the `test` attempt log and
    // keeps everything else identical to the fully-dispatched case above.
    let task = tid(11);
    let tasks_body = format!(
        "- [x] {task} — Documentation update task.\n  \
         - Test-first: not-applicable — Documentation only change with no executable behavior\n  \
         - Non-red probe: `rg -n \"pattern\" docs/`\n"
    );
    let slug = "marked-plan-11";

    let (temp, prompt) = write_prompt(slug, true, "", &tasks_body);
    let repo = temp.path().to_path_buf();
    let prompt_text = fs::read_to_string(&prompt).unwrap();
    let plan_slug = extract_plan_slug(&prompt.to_string_lossy());
    let contract =
        pipeline::task_spec::parse_task_contract(&prompt_text, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();

    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let (_temp2, prompt2) = write_prompt(slug, true, &extra_status, &tasks_body);
    fs::write(&prompt, fs::read_to_string(&prompt2).unwrap()).unwrap();

    write_passing_boundary_receipt(&repo, slug, &task);
    let log_dir = repo.join(".dev/executor-logs").join(slug);
    // Deliberately no `test` attempt log — that is the skip path being covered.
    write_attempt_log(&log_dir, &task, "implement", "completed", 1);
    write_attempt_log(&log_dir, &task, "audit", "completed", 2);

    let argv = encode_argv(&["sh", "-c", "exit 0"]);
    let probe = run_gal(
        &repo,
        &[
            "test-first-probe",
            "run",
            &plan_slug,
            &task,
            "1",
            &digest,
            "test",
            "pass",
            "P-01",
            "non-red",
            &argv,
            "2000",
        ],
    );
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );

    let (_output, receipt) = converge_check(&repo, &prompt, &task);
    let line = extract_receipt_line(&receipt, "test-first-evaluation").unwrap();
    assert!(line.contains("| pass |"), "{receipt}");
    assert!(
        line.contains("completed_phases=[\"test\", \"implement\", \"audit\"]"),
        "{line}"
    );
}

#[test]
fn marked_not_applicable_task_reads_attempt_log_from_real_dispatch() {
    let task = tid(9);
    let tasks_body = format!(
        "- [x] {task} — Documentation update task.\n  \
         - Test-first: not-applicable — Documentation only change with no executable behavior\n  \
         - Non-red probe: `cargo test --doc`\n"
    );
    let slug = "marked-plan-9";
    let (temp, prompt) = write_prompt(slug, true, "", &tasks_body);
    let repo = temp.path();
    let prompt_text = fs::read_to_string(&prompt).unwrap();
    let plan_slug = extract_plan_slug(&prompt.to_string_lossy());
    let contract =
        pipeline::task_spec::parse_task_contract(&prompt_text, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();
    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let (_temp2, prompt2) = write_prompt(slug, true, &extra_status, &tasks_body);
    fs::write(&prompt, fs::read_to_string(&prompt2).unwrap()).unwrap();
    write_passing_boundary_receipt(repo, slug, &task);

    let dispatch = run_real_dispatch(repo, slug, &task);
    assert!(
        dispatch.status.success(),
        "{}",
        String::from_utf8_lossy(&dispatch.stderr)
    );
    let log_dir = repo.join(".dev/executor-logs").join(slug);
    assert!(
        fs::read_dir(&log_dir)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.path().extension().is_some_and(|ext| ext == "log")),
        "real dispatch must produce an attempt log"
    );
    write_attempt_log(&log_dir, &task, "test", "completed", 2);
    write_attempt_log(&log_dir, &task, "audit", "completed", 3);

    let argv = encode_argv(&["sh", "-c", "exit 0"]);
    let probe = run_gal(
        repo,
        &[
            "test-first-probe",
            "run",
            &plan_slug,
            &task,
            "1",
            &digest,
            "test",
            "pass",
            "P-01",
            "non-red",
            &argv,
            "2000",
        ],
    );
    assert!(probe.status.success());

    let (_output, receipt) = converge_check(repo, &prompt, &task);
    let line = extract_receipt_line(&receipt, "test-first-evaluation").unwrap();
    assert!(line.contains("| pass |"), "{receipt}");
    assert!(
        line.contains("completed_phases=[\"test\", \"implement\", \"audit\"]"),
        "{line}"
    );
}

#[test]
fn marked_not_applicable_task_accepts_no_writeback_implement_phase() {
    let task = tid(12);
    let tasks_body = format!(
        "- [x] {task} — Documentation update task.\n  \
         - Test-first: not-applicable — Documentation only change with no executable behavior\n  \
         - Non-red probe: `cargo test --doc`\n"
    );
    let slug = "marked-plan-12";

    let (temp, prompt) = write_prompt(slug, true, "", &tasks_body);
    let repo = temp.path().to_path_buf();
    let prompt_text = fs::read_to_string(&prompt).unwrap();
    let plan_slug = extract_plan_slug(&prompt.to_string_lossy());
    let contract =
        pipeline::task_spec::parse_task_contract(&prompt_text, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();

    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let (_temp2, prompt2) = write_prompt(slug, true, &extra_status, &tasks_body);
    fs::write(&prompt, fs::read_to_string(&prompt2).unwrap()).unwrap();

    write_passing_boundary_receipt(&repo, slug, &task);
    let log_dir = repo.join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "test", "completed", 1);
    write_attempt_log(&log_dir, &task, "implement", "no-writeback", 2);
    write_attempt_log(&log_dir, &task, "audit", "completed", 3);

    let argv = encode_argv(&["sh", "-c", "exit 0"]);
    let probe = run_gal(
        &repo,
        &[
            "test-first-probe",
            "run",
            &plan_slug,
            &task,
            "1",
            &digest,
            "test",
            "pass",
            "P-01",
            "non-red",
            &argv,
            "2000",
        ],
    );
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );

    let (_output, receipt) = converge_check(&repo, &prompt, &task);
    let phase_impl_line = extract_receipt_line(&receipt, "phase-implement")
        .unwrap_or_else(|| panic!("marked task must emit a phase-implement row:\n{receipt}"));
    assert!(
        phase_impl_line.contains("| pass |"),
        "classify_phase must accept no-writeback for a not-applicable implement phase: {phase_impl_line}"
    );
    let line = extract_receipt_line(&receipt, "test-first-evaluation").unwrap();
    assert!(
        line.contains("completed_phases=[\"test\", \"implement\", \"audit\"]"),
        "not-applicable implement phase must accept a no-writeback attempt: {line}"
    );
    assert!(
        line.contains("| pass |"),
        "not-applicable implement phase must accept a no-writeback attempt: {receipt}"
    );
}

#[test]
fn marked_required_task_rejects_no_writeback_implement_phase() {
    let task = tid(13);
    let ef = ef_line(b"needle");
    let tasks_body = contract_block(&task, &ef);
    let slug = "marked-plan-13";

    let (temp, prompt) = write_prompt(slug, true, "", &tasks_body);
    let repo = temp.path().to_path_buf();
    let prompt_text = fs::read_to_string(&prompt).unwrap();
    let plan_slug = extract_plan_slug(&prompt.to_string_lossy());
    let contract =
        pipeline::task_spec::parse_task_contract(&prompt_text, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();

    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let (_temp2, prompt2) = write_prompt(slug, true, &extra_status, &tasks_body);
    fs::write(&prompt, fs::read_to_string(&prompt2).unwrap()).unwrap();

    write_passing_boundary_receipt(&repo, slug, &task);
    let log_dir = repo.join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "test", "completed", 1);
    write_attempt_log(&log_dir, &task, "implement", "no-writeback", 2);
    write_attempt_log(&log_dir, &task, "audit", "completed", 3);

    let (_output, receipt) = converge_check(&repo, &prompt, &task);
    let line = extract_receipt_line(&receipt, "test-first-evaluation").unwrap();
    assert!(line.contains("| fail |"), "{receipt}");
    assert!(
        !line.contains("\"implement\""),
        "required contract must reject no-writeback implement attempt: {line}"
    );
}

#[test]
fn marked_not_applicable_task_rejects_no_writeback_audit_phase() {
    let task = tid(14);
    let tasks_body = format!(
        "- [x] {task} — Documentation update task.\n  \
         - Test-first: not-applicable — Documentation only change with no executable behavior\n  \
         - Non-red probe: `cargo test --doc`\n"
    );
    let slug = "marked-plan-14";

    let (temp, prompt) = write_prompt(slug, true, "", &tasks_body);
    let repo = temp.path().to_path_buf();
    let prompt_text = fs::read_to_string(&prompt).unwrap();
    let plan_slug = extract_plan_slug(&prompt.to_string_lossy());
    let contract =
        pipeline::task_spec::parse_task_contract(&prompt_text, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();

    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let (_temp2, prompt2) = write_prompt(slug, true, &extra_status, &tasks_body);
    fs::write(&prompt, fs::read_to_string(&prompt2).unwrap()).unwrap();

    write_passing_boundary_receipt(&repo, slug, &task);
    let log_dir = repo.join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "test", "completed", 1);
    write_attempt_log(&log_dir, &task, "implement", "completed", 2);
    write_attempt_log(&log_dir, &task, "audit", "no-writeback", 3);

    let argv = encode_argv(&["sh", "-c", "exit 0"]);
    let probe = run_gal(
        &repo,
        &[
            "test-first-probe",
            "run",
            &plan_slug,
            &task,
            "1",
            &digest,
            "test",
            "pass",
            "P-01",
            "non-red",
            &argv,
            "2000",
        ],
    );
    assert!(probe.status.success());

    let (_output, receipt) = converge_check(&repo, &prompt, &task);
    let line = extract_receipt_line(&receipt, "test-first-evaluation").unwrap();
    assert!(line.contains("| fail |"), "{receipt}");
    assert!(
        !line.contains("\"audit\""),
        "audit phase must reject no-writeback attempt: {line}"
    );
}
