//! Real-binary coverage for the legacy `pipeline-converge-check` lane.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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

fn write_prompt(slug: &str, tasks_body: &str) -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().to_path_buf();
    Command::new("git")
        .current_dir(&repo)
        .args(["init", "-q", "-b", "main"])
        .output()
        .unwrap();
    let text = format!(
        "# Plan Prompt: {slug}\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n## Tasks\n\n{tasks_body}\n"
    );
    let prompt_path = repo.join(".dev/plans").join(format!("{slug}.prompt.md"));
    fs::create_dir_all(prompt_path.parent().unwrap()).unwrap();
    fs::write(&prompt_path, text).unwrap();
    (temp, prompt_path)
}

fn extract_receipt_line<'a>(receipt: &'a str, check_name: &str) -> Option<&'a str> {
    receipt
        .lines()
        .find(|line| line.starts_with(&format!("| {check_name} |")))
}

fn converge_check(repo: &Path, prompt: &Path, task: &str) -> String {
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
    assert!(
        output.status.success() || output.status.code().is_some(),
        "converge check did not return a process status"
    );
    fs::read_to_string(receipt_path).unwrap_or_default()
}

#[test]
fn legacy_task_omits_test_first_only_checks() {
    let task = tid(1);
    let tasks_body = format!("- [x] {task} — legacy placeholder.\n");
    let (temp, prompt) = write_prompt("legacy-plan", &tasks_body);
    let receipt = converge_check(temp.path(), &prompt, &task);

    assert!(
        extract_receipt_line(&receipt, "boundary-evidence").is_none(),
        "{receipt}"
    );
    assert!(
        extract_receipt_line(&receipt, "test-first-evaluation").is_none(),
        "{receipt}"
    );
}

#[test]
fn legacy_task_emits_only_legacy_phase_rows() {
    let task = tid(2);
    let tasks_body = format!("- [x] {task} — legacy placeholder.\n");
    let (temp, prompt) = write_prompt("legacy-plan-phases", &tasks_body);
    let receipt = converge_check(temp.path(), &prompt, &task);

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
fn incomplete_v2_evidence_is_typed_and_does_not_consume_retries() {
    let task = tid(3);
    let tasks_body = format!("- [x] {task} — legacy placeholder.\n");
    let (temp, prompt) = write_prompt("incomplete-v2", &tasks_body);
    let log_dir = temp.path().join(".dev/pipeline/incomplete-v2").join(&task);
    fs::create_dir_all(&log_dir).unwrap();
    let log = format!(
        "GAL-DISPATCH-LOG v1\nevidence_contract: v2\ntimestamp_start: now\nattempt_id: a1\nphase: test\ntask_id: {task}\nterminal_state: completed\nsession_id: provider-session\nprompt_sha256: prompt\nspec_sha256: spec\ncommit: commit\n---STDOUT---\n\n---STDERR---\n"
    );
    fs::write(
        log_dir.join(format!(
            "0000000001-000000001-000001-{task}-test-claude.log"
        )),
        log,
    )
    .unwrap();

    let before = fs::read_to_string(&prompt).unwrap();
    let receipt = converge_check(temp.path(), &prompt, &task);
    let after = fs::read_to_string(&prompt).unwrap();

    assert!(
        receipt.contains("dispatch-evidence-incomplete"),
        "{receipt}"
    );
    assert_eq!(before, after, "convergence must not consume retry state");
}

use sha2::{Digest, Sha256};

fn sha256_hex(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn setup_git_repo(slug: &str, task: &str) -> (TempDir, PathBuf, String) {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().to_path_buf();
    Command::new("git")
        .current_dir(&repo)
        .args(["init", "-q", "-b", "main"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(&repo)
        .args(["config", "user.email", "tester@example.com"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(&repo)
        .args(["config", "user.name", "Tester"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(&repo)
        .args(["config", "commit.gpgsign", "false"])
        .output()
        .unwrap();

    let empty_commit = Command::new("git")
        .current_dir(&repo)
        .args(["commit", "--allow-empty", "-q", "-m", "init"])
        .output()
        .unwrap();
    assert!(empty_commit.status.success());
    let head = String::from_utf8_lossy(
        &Command::new("git")
            .current_dir(&repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .trim()
    .to_string();

    let tasks_body = format!("- [x] {task} — task completed *({head})*\n");
    let text = format!(
        "# Plan Prompt: {slug}\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n## Tasks\n\n{tasks_body}\n"
    );
    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    let source_path = plans_dir.join(format!("{slug}.md"));
    fs::write(&prompt_path, &text).unwrap();
    fs::write(&source_path, &text).unwrap();
    let receipt_dir = repo.join(".dev/pipeline").join(slug).join(task);
    fs::create_dir_all(&receipt_dir).unwrap();
    for phase in ["implement", "test", "audit"] {
        fs::write(
            receipt_dir.join(format!("{task}-{phase}.receipt.md")),
            format!("phase receipt: {phase}\n"),
        )
        .unwrap();
    }

    (temp, prompt_path, head)
}

fn valid_v2_headers(
    prompt_path: &Path,
    prompt_text: &str,
    task: &str,
    phase: &str,
    attempt_id: &str,
    commit: &str,
) -> Vec<(&'static str, String)> {
    let canonical = prompt_path
        .canonicalize()
        .unwrap_or_else(|_| prompt_path.to_path_buf())
        .to_string_lossy()
        .to_string();
    let repo = prompt_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let scope = prompt_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .strip_suffix(".prompt.md")
        .unwrap()
        .to_string();
    let receipt = repo
        .join(".dev/pipeline")
        .join(&scope)
        .join(task)
        .join(format!("{task}-{phase}.receipt.md"));
    let receipt_sha256 = sha256_hex(&fs::read(receipt).unwrap());
    let timestamp_start = "epoch+1790000000s";
    let receipt_name = format!(".dev/pipeline/{scope}/{task}/{task}-{phase}.receipt.md");
    let spec = pipeline::task_spec::assemble_task_spec(&pipeline::task_spec::TaskSpecInput {
        task_scope: task,
        phase,
        prompt_path: &format!(".dev/plans/{scope}.prompt.md"),
        prompt_body: prompt_text,
        generated: timestamp_start,
        git_branch: "main",
        git_head: &commit[..7],
        convention_hints: None,
        receipt_path: Some(&receipt_name),
        agent_contract_body: None,
        fix_mode: false,
    })
    .unwrap();
    vec![
        ("evidence_contract", "v2".to_string()),
        ("plan_scope", canonical),
        ("task_id", task.to_string()),
        ("phase", phase.to_string()),
        ("attempt_id", attempt_id.to_string()),
        (
            "session_id",
            "176f1141-b606-47aa-a47b-76f2a6147623".to_string(),
        ),
        ("prompt_sha256", sha256_hex(prompt_text.as_bytes())),
        ("spec_sha256", sha256_hex(spec.markdown.as_bytes())),
        ("commit", commit.to_string()),
        ("diff_sha256", sha256_hex(&[])),
        ("receipt_sha256", receipt_sha256),
        ("terminal_state", "completed".to_string()),
        ("timestamp_start", timestamp_start.to_string()),
    ]
}

fn format_log(headers: &[(&str, String)]) -> String {
    let mut s = String::from("GAL-DISPATCH-LOG v1\n");
    for (k, v) in headers {
        s.push_str(&format!("{k}: {v}\n"));
    }
    s.push_str("---STDOUT---\n\n---STDERR---\n");
    s
}

#[test]
fn valid_v2_evidence_repeated_validation_preserves_verdict_and_receipt() {
    let task = tid(4);
    let (temp, prompt, head) = setup_git_repo("valid-v2", &task);
    let prompt_text = fs::read_to_string(&prompt).unwrap();

    let log_dir = temp.path().join(".dev/pipeline/valid-v2").join(&task);
    fs::create_dir_all(&log_dir).unwrap();

    for (phase, attempt_id) in [
        ("implement", "0000000001-000000001-000001"),
        ("test", "0000000001-000000001-000002"),
        ("audit", "0000000001-000000001-000003"),
    ] {
        let headers = valid_v2_headers(&prompt, &prompt_text, &task, phase, attempt_id, &head);
        let log_content = format_log(&headers);
        fs::write(
            log_dir.join(format!("{attempt_id}-{task}-{phase}-agy.log")),
            log_content,
        )
        .unwrap();
    }

    let before_prompt = fs::read_to_string(&prompt).unwrap();

    // First validation
    let receipt_1 = converge_check(temp.path(), &prompt, &task);
    assert!(
        receipt_1.contains("overall: pass"),
        "First run must pass:\n{receipt_1}"
    );
    assert!(receipt_1.contains("phase-implement"), "{receipt_1}");
    assert!(receipt_1.contains("phase-test"), "{receipt_1}");
    assert!(receipt_1.contains("phase-audit"), "{receipt_1}");
    assert!(
        !receipt_1.contains("dispatch-evidence-incomplete"),
        "{receipt_1}"
    );

    // Second validation (repeated check)
    let receipt_2 = converge_check(temp.path(), &prompt, &task);
    assert_eq!(
        receipt_1, receipt_2,
        "Repeated validation must return identical receipt"
    );

    let after_prompt = fs::read_to_string(&prompt).unwrap();
    assert_eq!(
        before_prompt, after_prompt,
        "Repeated convergence check must not consume retry counts or mutate prompt"
    );

    // All log files still exist
    assert!(log_dir
        .join(format!("0000000001-000000001-000002-{task}-test-agy.log"))
        .exists());
}

#[test]
fn evidence_only_v2_implement_no_writeback_is_accepted() {
    let task = tid(40);
    let (temp, prompt, head) = setup_git_repo("evidence-only", &task);
    let text = format!(
        "# Plan Prompt: evidence-only\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n## Tasks\n\n- [x] {task} — evidence-only validation *({head})*\n  - Targets: `.dev/pipeline/evidence-only/acceptance.receipt.md`\n  - Evidence: Evidence-only task has no product writeback.\n"
    );
    fs::write(&prompt, &text).unwrap();
    fs::write(prompt.parent().unwrap().join("evidence-only.md"), &text).unwrap();

    let log_dir = temp.path().join(".dev/pipeline/evidence-only").join(&task);
    fs::create_dir_all(&log_dir).unwrap();
    let attempt_id = "0000000001-000000001-000001";
    let mut headers = valid_v2_headers(&prompt, &text, &task, "implement", attempt_id, &head);
    for (key, value) in headers.iter_mut() {
        if *key == "terminal_state" {
            *value = "no-writeback".to_string();
        }
    }
    fs::write(
        log_dir.join(format!("{attempt_id}-{task}-implement-agy.log")),
        format_log(&headers),
    )
    .unwrap();

    let receipt = converge_check(temp.path(), &prompt, &task);
    let implement = extract_receipt_line(&receipt, "phase-implement").unwrap();
    assert!(
        implement.contains("pass") && implement.contains("dispatch-offload"),
        "qualifying evidence-only no-writeback must be accepted:\n{receipt}"
    );
}

#[test]
fn ordinary_v2_implement_no_writeback_remains_fail_closed() {
    let task = tid(41);
    let (temp, prompt, head) = setup_git_repo("ordinary-no-writeback", &task);
    let text = format!(
        "# Plan Prompt: ordinary-no-writeback\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n## Tasks\n\n- [x] {task} — ordinary implementation *({head})*\n  - Targets: `crates/cli/src/example.rs`\n"
    );
    fs::write(&prompt, &text).unwrap();
    fs::write(
        prompt.parent().unwrap().join("ordinary-no-writeback.md"),
        &text,
    )
    .unwrap();

    let log_dir = temp
        .path()
        .join(".dev/pipeline/ordinary-no-writeback")
        .join(&task);
    fs::create_dir_all(&log_dir).unwrap();
    let attempt_id = "0000000001-000000001-000001";
    let mut headers = valid_v2_headers(&prompt, &text, &task, "implement", attempt_id, &head);
    for (key, value) in headers.iter_mut() {
        if *key == "terminal_state" {
            *value = "no-writeback".to_string();
        }
    }
    fs::write(
        log_dir.join(format!("{attempt_id}-{task}-implement-agy.log")),
        format_log(&headers),
    )
    .unwrap();

    let before = fs::read_to_string(&prompt).unwrap();
    let receipt = converge_check(temp.path(), &prompt, &task);
    let implement = extract_receipt_line(&receipt, "phase-implement").unwrap();
    assert!(
        implement.contains("fail") && !implement.contains("dispatch-offload"),
        "ordinary zero-write implement attempts must fail closed:\n{receipt}"
    );
    assert_eq!(
        before,
        fs::read_to_string(&prompt).unwrap(),
        "ordinary zero-write rejection must not consume semantic retries"
    );
}

#[test]
fn evidence_only_targets_ignore_unrelated_backticked_command_paths() {
    let task = tid(44);
    let (temp, prompt, head) = setup_git_repo("evidence-only-target-field", &task);
    let text = format!(
        "# Plan Prompt: evidence-only-target-field\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n## Tasks\n\n- [x] {task} — validate evidence with `crates/not-a-target.rs` and `cargo test --manifest-path crates/not-a-target/Cargo.toml` *({head})*\n  - Targets: `.dev/pipeline/evidence-only-target-field/acceptance.receipt.md`\n  - Evidence: This task has no product writeback. The command path `crates/also-not-a-target.rs` is unrelated to writeback.\n"
    );
    fs::write(&prompt, &text).unwrap();
    fs::write(
        prompt
            .parent()
            .unwrap()
            .join("evidence-only-target-field.md"),
        &text,
    )
    .unwrap();

    let log_dir = temp
        .path()
        .join(".dev/pipeline/evidence-only-target-field")
        .join(&task);
    fs::create_dir_all(&log_dir).unwrap();
    let attempt_id = "0000000001-000000001-000001";
    let mut headers = valid_v2_headers(&prompt, &text, &task, "implement", attempt_id, &head);
    for (key, value) in headers.iter_mut() {
        if *key == "terminal_state" {
            *value = "no-writeback".to_string();
        }
    }
    fs::write(
        log_dir.join(format!("{attempt_id}-{task}-implement-agy.log")),
        format_log(&headers),
    )
    .unwrap();

    let receipt = converge_check(temp.path(), &prompt, &task);
    let implement = extract_receipt_line(&receipt, "phase-implement").unwrap();
    assert!(
        implement.contains("pass") && implement.contains("dispatch-offload"),
        "only Targets: paths should control evidence-only eligibility:\n{receipt}"
    );
}

#[test]
fn evidence_only_v1_implement_no_writeback_requires_current_session_and_receipt() {
    let task = tid(42);
    let (temp, prompt, head) = setup_git_repo("evidence-only-v1", &task);
    let text = format!(
        "# Plan Prompt: evidence-only-v1\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n## Tasks\n\n- [x] {task} — evidence-only validation *({head})*\n  - Targets: `.dev/pipeline/evidence-only-v1/acceptance.receipt.md`\n  - Evidence: This task has no product writeback.\n"
    );
    fs::write(&prompt, &text).unwrap();
    fs::write(prompt.parent().unwrap().join("evidence-only-v1.md"), &text).unwrap();

    let log_dir = temp
        .path()
        .join(".dev/pipeline/evidence-only-v1")
        .join(&task);
    fs::create_dir_all(&log_dir).unwrap();
    let attempt_id = "0000000001-000000001-000001";
    let log = format!(
        "GAL-DISPATCH-LOG v1\n\
         timestamp_start: epoch+1790000000s\n\
         attempt_id: {attempt_id}\n\
         phase: implement\n\
         task_id: {task}\n\
         session_id: provider-session-42\n\
         terminal_state: no-writeback\n\
         ---STDOUT---\n\n\
         ---STDERR---\n"
    );
    fs::write(
        log_dir.join(format!("{attempt_id}-{task}-implement-claude.log")),
        log,
    )
    .unwrap();

    let receipt = converge_check(temp.path(), &prompt, &task);
    let implement = extract_receipt_line(&receipt, "phase-implement").unwrap();
    assert!(
        implement.contains("pass") && implement.contains("dispatch-offload"),
        "qualifying v1 evidence-only no-writeback must be accepted:\n{receipt}"
    );
}

#[test]
fn evidence_only_v1_implement_no_writeback_rejects_missing_session_or_receipt() {
    let task = tid(43);
    let (temp, prompt, head) = setup_git_repo("evidence-only-v1-reject", &task);
    let text = format!(
        "# Plan Prompt: evidence-only-v1-reject\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n## Tasks\n\n- [x] {task} — evidence-only validation *({head})*\n  - Targets: `.dev/pipeline/evidence-only-v1-reject/acceptance.receipt.md`\n  - Evidence: This task has no product writeback.\n"
    );
    fs::write(&prompt, &text).unwrap();
    fs::write(
        prompt.parent().unwrap().join("evidence-only-v1-reject.md"),
        &text,
    )
    .unwrap();
    let log_dir = temp
        .path()
        .join(".dev/pipeline/evidence-only-v1-reject")
        .join(&task);
    fs::create_dir_all(&log_dir).unwrap();
    let attempt_id = "0000000001-000000001-000001";
    let log = format!(
        "GAL-DISPATCH-LOG v1\n\
         timestamp_start: epoch+1790000000s\n\
         attempt_id: {attempt_id}\n\
         phase: implement\n\
         task_id: {task}\n\
         session_id: none\n\
         terminal_state: no-writeback\n\
         ---STDOUT---\n\n\
         ---STDERR---\n"
    );
    fs::write(
        log_dir.join(format!("{attempt_id}-{task}-implement-claude.log")),
        log,
    )
    .unwrap();
    fs::write(
        temp.path()
            .join(".dev/pipeline/evidence-only-v1-reject")
            .join(&task)
            .join(format!("{task}-implement.receipt.md")),
        "",
    )
    .unwrap();

    let receipt = converge_check(temp.path(), &prompt, &task);
    let implement = extract_receipt_line(&receipt, "phase-implement").unwrap();
    assert!(
        implement.contains("fail") && implement.contains("no-writeback"),
        "incomplete v1 evidence-only no-writeback must fail closed:\n{receipt}"
    );
}

#[test]
fn stale_and_mismatched_v2_bindings_fail_closed_with_typed_token() {
    let task = tid(5);
    let (temp, prompt, head) = setup_git_repo("matrix-v2", &task);
    let prompt_text = fs::read_to_string(&prompt).unwrap();

    let foreign_prompt = temp.path().join(".dev/plans/foreign.prompt.md");
    fs::write(&foreign_prompt, "foreign").unwrap();
    let foreign_canonical = foreign_prompt
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let attempt_id = "0000000001-000000001-000001";

    let foreign = foreign_canonical.clone();
    type HeaderMutator = Box<dyn Fn(&mut Vec<(&'static str, String)>)>;
    let test_cases: Vec<(&str, HeaderMutator)> = vec![
        (
            "plan_scope_mismatch",
            Box::new(move |headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "plan_scope" {
                        *v = foreign.clone();
                    }
                }
            }),
        ),
        (
            "task_id_mismatch",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "task_id" {
                        *v = format!("T-{}", 99);
                    }
                }
            }),
        ),
        (
            "phase_mismatch",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "phase" {
                        *v = "audit".to_string();
                    }
                }
            }),
        ),
        (
            "attempt_id_mismatch",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "attempt_id" {
                        *v = "0000000001-000000001-999999".to_string();
                    }
                }
            }),
        ),
        (
            "session_id_none",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "session_id" {
                        *v = "none".to_string();
                    }
                }
            }),
        ),
        (
            "session_id_unknown",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "session_id" {
                        *v = "unknown".to_string();
                    }
                }
            }),
        ),
        (
            "session_id_null",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "session_id" {
                        *v = "null".to_string();
                    }
                }
            }),
        ),
        (
            "session_id_n_a",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "session_id" {
                        *v = "n/a".to_string();
                    }
                }
            }),
        ),
        (
            "session_id_na",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "session_id" {
                        *v = "na".to_string();
                    }
                }
            }),
        ),
        (
            "session_id_placeholder",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "session_id" {
                        *v = "placeholder".to_string();
                    }
                }
            }),
        ),
        (
            "session_id_angle_none",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "session_id" {
                        *v = "<none>".to_string();
                    }
                }
            }),
        ),
        (
            "session_id_zero_uuid",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "session_id" {
                        *v = "00000000-0000-0000-0000-000000000000".to_string();
                    }
                }
            }),
        ),
        (
            "prompt_sha256_stale",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "prompt_sha256" {
                        *v = "0".repeat(64);
                    }
                }
            }),
        ),
        (
            "spec_sha256_not_hex",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "spec_sha256" {
                        *v = "not-a-valid-sha256-hex-digest".to_string();
                    }
                }
            }),
        ),
        (
            "spec_sha256_none",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "spec_sha256" {
                        *v = "none".to_string();
                    }
                }
            }),
        ),
        (
            "commit_not_hex",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "commit" {
                        *v = "not-a-valid-commit-hash".to_string();
                    }
                }
            }),
        ),
        (
            "diff_sha256_short",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "diff_sha256" {
                        *v = "abc".to_string();
                    }
                }
            }),
        ),
        (
            "receipt_sha256_invalid_hex",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "receipt_sha256" {
                        *v = "z".repeat(64);
                    }
                }
            }),
        ),
        (
            "evidence_contract_invalid",
            Box::new(|headers| {
                for (k, v) in headers.iter_mut() {
                    if *k == "evidence_contract" {
                        *v = "v3".to_string();
                    }
                }
            }),
        ),
        (
            "missing_receipt_sha256_header",
            Box::new(|headers| {
                headers.retain(|(k, _)| *k != "receipt_sha256");
            }),
        ),
    ];

    let log_dir = temp.path().join(".dev/pipeline/matrix-v2").join(&task);
    fs::create_dir_all(&log_dir).unwrap();
    let log_file = log_dir.join(format!("{attempt_id}-{task}-test-agy.log"));

    for (case_name, mutate) in test_cases {
        let mut headers = valid_v2_headers(&prompt, &prompt_text, &task, "test", attempt_id, &head);
        mutate(&mut headers);
        fs::write(&log_file, format_log(&headers)).unwrap();

        let before = fs::read_to_string(&prompt).unwrap();
        let receipt = converge_check(temp.path(), &prompt, &task);
        let after = fs::read_to_string(&prompt).unwrap();

        assert!(
            receipt.contains("dispatch-evidence-incomplete"),
            "Case '{case_name}' must fail closed with dispatch-evidence-incomplete, got:\n{receipt}"
        );
        assert_eq!(
            before, after,
            "Case '{case_name}' must not mutate prompt or consume retries"
        );
    }
}

#[test]
fn current_repository_binding_mismatches_fail_closed() {
    let task = tid(8);
    let (temp, prompt, head) = setup_git_repo("current-bindings", &task);
    let prompt_text = fs::read_to_string(&prompt).unwrap();
    let log_dir = temp
        .path()
        .join(".dev/pipeline/current-bindings")
        .join(&task);
    fs::create_dir_all(&log_dir).unwrap();
    let attempt_id = "0000000001-000000001-000001";

    for key in ["commit", "diff_sha256", "receipt_sha256"] {
        let mut headers = valid_v2_headers(&prompt, &prompt_text, &task, "test", attempt_id, &head);
        for (header, value) in &mut headers {
            if *header == key {
                *value = "d".repeat(value.len());
            }
        }
        fs::write(
            log_dir.join(format!("{attempt_id}-{task}-test-agy.log")),
            format_log(&headers),
        )
        .unwrap();

        let receipt = converge_check(temp.path(), &prompt, &task);
        assert!(
            receipt.contains("dispatch-evidence-incomplete"),
            "{key} mismatch must fail closed:\n{receipt}"
        );
    }
}

#[test]
fn implement_evidence_survives_final_commit_and_rejects_each_stale_binding() {
    let task = tid(9);
    let (temp, prompt, base_head) = setup_git_repo("implement-lifecycle", &task);
    let text = format!(
        "# Plan Prompt: implement-lifecycle\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n## Tasks\n\n- [x] {task} — evidence-only lifecycle *({base_head})*\n  - Targets: `.dev/pipeline/implement-lifecycle/{task}/acceptance.receipt.md`\n  - Evidence: This task has no product writeback.\n"
    );
    fs::write(&prompt, &text).unwrap();
    fs::write(
        prompt.parent().unwrap().join("implement-lifecycle.md"),
        &text,
    )
    .unwrap();

    let log_dir = temp
        .path()
        .join(".dev/pipeline/implement-lifecycle")
        .join(&task);
    fs::create_dir_all(&log_dir).unwrap();
    let attempt_id = "0000000001-000000001-000001";
    let valid = valid_v2_headers(&prompt, &text, &task, "implement", attempt_id, &base_head);
    let log_path = log_dir.join(format!("{attempt_id}-{task}-implement-agy.log"));
    fs::write(&log_path, format_log(&valid)).unwrap();

    // The implement evidence was captured at the base commit. The orchestrator
    // then advances HEAD to the task's final commit without changing the prompt
    // or the working-tree diff.
    let final_commit = Command::new("git")
        .current_dir(temp.path())
        .args(["commit", "--allow-empty", "-q", "-m", "task-final"])
        .output()
        .unwrap();
    assert!(final_commit.status.success());

    let receipt = converge_check(temp.path(), &prompt, &task);
    let implement = extract_receipt_line(&receipt, "phase-implement").unwrap();
    assert!(
        implement.contains("pass") && implement.contains("dispatch-offload"),
        "valid pre-commit evidence must pass after the final commit advances HEAD:\n{receipt}"
    );

    let prompt_before_mutations = fs::read_to_string(&prompt).unwrap();
    for key in ["spec_sha256", "commit", "diff_sha256", "receipt_sha256"] {
        let mut mutated = valid.clone();
        for (header, value) in &mut mutated {
            if *header == key {
                *value = "d".repeat(value.len());
            }
        }
        fs::write(&log_path, format_log(&mutated)).unwrap();
        let receipt = converge_check(temp.path(), &prompt, &task);
        assert!(
            receipt.contains("dispatch-evidence-incomplete"),
            "{key} mutation must fail closed after the final commit lifecycle:\n{receipt}"
        );
        assert_eq!(
            prompt_before_mutations,
            fs::read_to_string(&prompt).unwrap(),
            "{key} mutation must not consume semantic retries"
        );
    }
}

#[test]
fn older_completed_evidence_does_not_mask_latest_incomplete_v2() {
    let task = tid(6);
    let (temp, prompt, head) = setup_git_repo("older-completed", &task);
    let prompt_text = fs::read_to_string(&prompt).unwrap();

    let log_dir = temp
        .path()
        .join(".dev/pipeline/older-completed")
        .join(&task);
    fs::create_dir_all(&log_dir).unwrap();

    // Attempt 1: Valid completed v2
    let attempt_1 = "0000000001-000000001-000001";
    let headers_1 = valid_v2_headers(&prompt, &prompt_text, &task, "test", attempt_1, &head);
    fs::write(
        log_dir.join(format!("{attempt_1}-{task}-test-agy.log")),
        format_log(&headers_1),
    )
    .unwrap();

    // Attempt 2: Later timestamp, but stale prompt binding
    let attempt_2 = "0000000001-000000001-000002";
    let mut headers_2 = valid_v2_headers(&prompt, &prompt_text, &task, "test", attempt_2, &head);
    for (k, v) in headers_2.iter_mut() {
        if *k == "prompt_sha256" {
            *v = "0".repeat(64);
        }
    }
    fs::write(
        log_dir.join(format!("{attempt_2}-{task}-test-agy.log")),
        format_log(&headers_2),
    )
    .unwrap();

    let before = fs::read_to_string(&prompt).unwrap();
    let receipt = converge_check(temp.path(), &prompt, &task);
    let after = fs::read_to_string(&prompt).unwrap();

    assert!(
        receipt.contains("dispatch-evidence-incomplete"),
        "Latest incomplete attempt must fail closed even if older attempt was completed:\n{receipt}"
    );
    assert_eq!(before, after, "Must not consume retry counters");
}

#[test]
fn v1_completed_and_accepted_no_writeback_retain_verdicts() {
    let task = tid(7);
    let (temp, prompt, _head) = setup_git_repo("v1-verdicts", &task);

    let log_dir = temp.path().join(".dev/pipeline/v1-verdicts").join(&task);
    fs::create_dir_all(&log_dir).unwrap();

    // V1 attempt with completed state (no v2 headers required)
    let v1_log = format!(
        "GAL-DISPATCH-LOG v1\n\
         timestamp_start: epoch+1790000000s\n\
         attempt_id: 0000000001-000000001-000001\n\
         phase: test\n\
         task_id: {task}\n\
         terminal_state: completed\n\
         ---STDOUT---\n\n\
         ---STDERR---\n"
    );
    fs::write(
        log_dir.join(format!(
            "0000000001-000000001-000001-{task}-test-claude.log"
        )),
        v1_log,
    )
    .unwrap();

    let receipt = converge_check(temp.path(), &prompt, &task);
    let test_line =
        extract_receipt_line(&receipt, "phase-test").expect("phase-test row must be present");
    assert!(
        test_line.contains("pass") && test_line.contains("dispatch-offload"),
        "v1 completed attempt must classify as dispatch-offload:\n{receipt}"
    );

    // Now test accepted no-writeback history:
    // Attempt 1 was completed (above), attempt 2 is no-writeback
    let v1_nw_log = format!(
        "GAL-DISPATCH-LOG v1\n\
         timestamp_start: epoch+1790000100s\n\
         attempt_id: 0000000001-000000001-000002\n\
         phase: test\n\
         task_id: {task}\n\
         terminal_state: no-writeback\n\
         ---STDOUT---\n\n\
         ---STDERR---\n"
    );
    fs::write(
        log_dir.join(format!(
            "0000000001-000000001-000002-{task}-test-claude.log"
        )),
        v1_nw_log,
    )
    .unwrap();

    let receipt_nw = converge_check(temp.path(), &prompt, &task);
    let test_nw_line =
        extract_receipt_line(&receipt_nw, "phase-test").expect("phase-test row must be present");
    assert!(
        test_nw_line.contains("pass") && test_nw_line.contains("dispatch-offload"),
        "v1 accepted no-writeback with prior completed must classify as dispatch-offload:\n{receipt_nw}"
    );
}
