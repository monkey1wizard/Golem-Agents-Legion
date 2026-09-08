//! Black-box fixtures for the continuation authority command.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{tempdir, TempDir};

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("git must be available");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn repo() -> TempDir {
    let temp = tempdir().expect("fixture tempdir");
    git(temp.path(), &["init", "-q", "-b", "main"]);
    git(
        temp.path(),
        &["config", "user.email", "fixture@example.com"],
    );
    git(temp.path(), &["config", "user.name", "CLI Fixture"]);
    git(temp.path(), &["config", "commit.gpgsign", "false"]);
    git(temp.path(), &["config", "core.hooksPath", ".git/hooks"]);
    temp
}

fn task(number: u32) -> String {
    format!("T-{number:02}")
}

fn initial_commit(repo: &Path) -> String {
    git(repo, &["commit", "--allow-empty", "-q", "-m", "fixture"]);
    git(repo, &["rev-parse", "HEAD"])
}

fn prompt(tasks: &str, current: &str, extra: &str) -> String {
    format!("# fixture\n\n## Tasks\n{tasks}\n\n## Status\nCurrent Task: {current}\n\n{extra}\n")
}

fn write_prompt(repo: &Path, name: &str, text: &str) -> (PathBuf, PathBuf) {
    let plans = repo.join(".dev").join("plans");
    std::fs::create_dir_all(&plans).unwrap();
    let prompt_path = plans.join(format!("{name}.prompt.md"));
    let source_path = plans.join(format!("{name}.md"));
    std::fs::write(&prompt_path, text).unwrap();
    (source_path, prompt_path)
}

fn run(repo: &Path, prompt: &Path, receipt: &Path, extra: &[String]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .arg("pipeline-handback-check")
        .arg(prompt)
        .arg("--receipt")
        .arg(receipt)
        .args(extra)
        .output()
        .expect("failed to spawn gal binary");
    let code = output
        .status
        .code()
        .expect("process was not terminated by signal");
    let receipt_text = std::fs::read_to_string(receipt).unwrap_or_default();
    assert!(
        output.stderr.is_empty() || code != 0,
        "unexpected stderr on success: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (code, receipt_text)
}

fn checked(task: &str, head: &str) -> String {
    format!("- [x] {task} — complete *({head})*")
}

fn goal(prompt: &Path, text: &str, head: &str, checked_tasks: &str, sha: &str) -> String {
    format!(
        "prompt_path: {}\nprompt_sha256: {sha}\nhead: {head}\nverdict: VERIFIED\nchecked_tasks: {checked_tasks}\nmust_have_1: cargo test passed\ncommand: cargo test\n{text}",
        prompt.display()
    )
}

fn sha256(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

fn failed_goal_predicates(receipt: &str) -> Vec<&str> {
    receipt
        .lines()
        .find_map(|line| line.strip_prefix("goal_binding_failed_predicates: "))
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|predicate| !predicate.is_empty() && *predicate != "none")
        .collect()
}

#[test]
fn goal_binding_reports_the_exact_independently_failed_predicate() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let receipt = fixture.path().join("goal-predicate-matrix.md");
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "goal-predicate-matrix",
        &prompt(&checked(&t1, &head), "—", ""),
    );
    let goal_path = fixture
        .path()
        .join(".dev/pipeline/receipts/goal-predicate-matrix/goal-verification.receipt.md");
    std::fs::create_dir_all(goal_path.parent().unwrap()).unwrap();
    let canonical_prompt = std::fs::canonicalize(&prompt_path).unwrap();
    let valid_goal = goal(&canonical_prompt, "", &head, &t1, &sha256(&prompt_path));

    let corruptions = [
        (
            "prompt-path",
            valid_goal.replacen(
                &format!("prompt_path: {}", canonical_prompt.display()),
                &format!(
                    "prompt_path: {}",
                    fixture.path().join("foreign.prompt.md").display()
                ),
                1,
            ),
        ),
        (
            "prompt-hash",
            valid_goal.replacen(
                &format!("prompt_sha256: {}", sha256(&prompt_path)),
                &format!("prompt_sha256: {}", "0".repeat(64)),
                1,
            ),
        ),
        (
            "head",
            valid_goal.replacen(
                &format!("head: {head}"),
                &format!("head: {}", "0".repeat(40)),
                1,
            ),
        ),
        (
            "checked-task-projection",
            valid_goal.replacen(&format!("checked_tasks: {t1}"), "checked_tasks: foreign", 1),
        ),
        (
            "must-haves",
            valid_goal.replacen("must_have_1: cargo test passed\n", "", 1),
        ),
        (
            "commands",
            valid_goal.replacen("command: cargo test\n", "", 1),
        ),
        (
            "verdict",
            valid_goal.replacen("verdict: VERIFIED", "verdict: GAPS_FOUND", 1),
        ),
    ];

    for (expected, corrupted_goal) in corruptions {
        std::fs::write(&goal_path, corrupted_goal).unwrap();
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, 1, "corrupt {expected} predicate unexpectedly passed");
        assert_eq!(
            failed_goal_predicates(&output),
            vec![expected],
            "corrupt fixture must report only the {expected} predicate; receipt:\n{output}"
        );
        assert!(
            output.contains("goal_binding: fail"),
            "corrupt {expected} fixture must report aggregate failure; receipt:\n{output}"
        );
    }

    let uncleared_prompt = prompt(&checked(&t1, &head), &t1, "");
    std::fs::write(&prompt_path, uncleared_prompt).unwrap();
    std::fs::write(
        &goal_path,
        goal(&canonical_prompt, "", &head, &t1, &sha256(&prompt_path)),
    )
    .unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "uncleared cursor unexpectedly passed");
    assert_eq!(
        failed_goal_predicates(&output),
        vec!["cursor"],
        "uncleared cursor must report only the cursor predicate; receipt:\n{output}"
    );
    assert!(output.contains("goal_binding: fail"));

    std::fs::write(&prompt_path, prompt(&checked(&t1, &head), "—", "")).unwrap();
    let valid_goal = goal(&canonical_prompt, "", &head, &t1, &sha256(&prompt_path));
    std::fs::write(&goal_path, valid_goal).unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 0, "valid goal fixture failed: {output}");
    assert!(output.contains("goal_binding: pass"));
    assert!(failed_goal_predicates(&output).is_empty());
}

#[test]
fn continuation_is_process_restart_safe_and_preserves_bindings() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let t2 = task(2);
    let text = prompt(
        &format!("{}\n- [ ] {t2} — pending", checked(&t1, &head)),
        &t2,
        "",
    );
    let (_, prompt_path) = write_prompt(fixture.path(), "continuation", &text);
    let receipt = fixture.path().join("explicit").join("first.md");

    for _ in 0..2 {
        let (code, receipt_text) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, 1);
        assert!(receipt_text.contains("decision: continue"));
        assert!(receipt_text.contains(&format!("unchecked_tasks: {t2}")));
        assert!(receipt_text.contains("current_task: ")); // active cursor is bound, not invented
        assert!(receipt_text.contains("evidence_path:"));
    }

    let (code, receipt_text) = run(
        fixture.path(),
        &prompt_path,
        &receipt,
        &["--stop-at".into(), t2.clone()],
    );
    assert_eq!(code, 1);
    assert!(receipt_text.contains(&format!(
        "next_action: gal.exe pipeline <prompt> from {t2} stop-at {t2}"
    )));
    assert!(!receipt_text.contains("next_action: gal.exe pipeline-handback-check"));
}

#[test]
fn source_and_prompt_inputs_share_sanitized_scope_and_collision_isolated() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let text = prompt(&checked(&t1, &head), "—", "");
    let (source, prompt_path) = write_prompt(fixture.path(), "fix weird.plan", &text);
    std::fs::write(&source, &text).unwrap();
    let source_receipt = fixture.path().join("receipts").join("source.md");
    let prompt_receipt = fixture.path().join("receipts").join("prompt.md");

    let (source_code, source_output) = run(fixture.path(), &source, &source_receipt, &[]);
    let (prompt_code, prompt_output) = run(fixture.path(), &prompt_path, &prompt_receipt, &[]);
    assert_eq!(source_code, 1);
    assert_eq!(prompt_code, 1);
    assert!(source_output.contains("decision: continue"));
    assert!(prompt_output.contains("decision: continue"));
    let source_scope = source_output
        .lines()
        .find(|line| line.starts_with("evidence_path:"))
        .unwrap();
    let prompt_scope = prompt_output
        .lines()
        .find(|line| line.starts_with("evidence_path:"))
        .unwrap();
    assert_eq!(source_scope, prompt_scope);
    assert!(source_scope.contains("fix-weird.plan"));
}

#[test]
fn only_fresh_bound_goal_authorizes_finalization() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let text = prompt(&checked(&t1, &head), "—", "");
    let (_, prompt_path) = write_prompt(fixture.path(), "goal", &text);
    let receipt = fixture.path().join("goal-result.md");
    let goal_path = fixture
        .path()
        .join(".dev/pipeline/receipts/goal/goal-verification.receipt.md");
    std::fs::create_dir_all(goal_path.parent().unwrap()).unwrap();

    let checked_tasks = t1.clone();
    let canonical_prompt = std::fs::canonicalize(&prompt_path).unwrap();
    let foreign_goal = fixture
        .path()
        .join(".dev/pipeline/receipts/foreign/goal-verification.receipt.md");
    std::fs::create_dir_all(foreign_goal.parent().unwrap()).unwrap();
    std::fs::write(
        &foreign_goal,
        goal(
            &canonical_prompt,
            "",
            &head,
            &checked_tasks,
            &sha256(&prompt_path),
        ),
    )
    .unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1);
    assert!(output.contains("decision: continue"));
    assert!(output.contains("next_action: run-goal-backward-verification"));

    std::fs::write(
        &goal_path,
        goal(
            &canonical_prompt,
            "",
            &"0".repeat(40),
            &checked_tasks,
            &sha256(&prompt_path),
        ),
    )
    .unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1);
    assert!(output.contains("decision: continue"));
    assert!(output.contains("next_action: run-goal-backward-verification"));

    let valid = goal(
        &canonical_prompt,
        "",
        &head,
        &checked_tasks,
        &sha256(&prompt_path),
    );
    std::fs::write(&goal_path, valid).unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 0, "unexpected bound-goal result: {output}");
    assert!(output.contains("decision: ready-to-finalize"));
    assert!(output.contains("final_authorized: true"));

    let generic_only = goal(
        &canonical_prompt,
        "",
        &head,
        &checked_tasks,
        &sha256(&prompt_path),
    )
    .replace(
        "must_have_1: cargo test passed\n",
        "evidence: advisory only\n",
    );
    std::fs::write(&goal_path, generic_only).unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1);
    assert!(output.contains("decision: continue"));
    assert!(output.contains("next_action: run-goal-backward-verification"));

    for malformed in [
        "must_have_2: gap\n",
        "must_have_1: first\nmust_have_1: duplicate\n",
        "must_have_x: malformed\n",
    ] {
        let invalid = goal(
            &canonical_prompt,
            malformed,
            &head,
            &checked_tasks,
            &sha256(&prompt_path),
        )
        .replace("must_have_1: cargo test passed\n", "");
        std::fs::write(&goal_path, invalid).unwrap();
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, 1, "malformed must-have authorized: {malformed}");
        assert!(output.contains("next_action: run-goal-backward-verification"));
    }

    let changed = prompt(&checked(&t1, &head), "—", "changed prompt");
    std::fs::write(&prompt_path, changed).unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1);
    assert!(output.contains("decision: continue"));
    assert!(output.contains("next_action: run-goal-backward-verification"));
}

#[test]
fn human_producers_retry_ceiling_and_invalid_stop_at_fail_closed() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let receipt = fixture.path().join("human.md");
    let cases = [
        ("security-protected-path", "AUDIT", "AUDITOR"),
        ("goal-gaps-blocked", "VERIFY", "VERIFY"),
        ("head-drift", "CONVERGE", "PIPELINE"),
        ("boundary-scope-decision", "BOUNDARY", "BOUNDARY"),
        ("convergence-human-repair", "CONVERGE", "CONVERGE"),
    ];
    for (reason, phase, producer) in cases {
        let drift = if reason == "head-drift" {
            format!("Baseline HEAD: {}\nObserved HEAD: {head}\n", "0".repeat(40))
        } else {
            String::new()
        };
        let handback = format!(
            "#### Human Handback — {reason}\nStatus: OPEN\nReason: {reason}\nTask: {t1}\nPhase: {phase}\nProducer: {producer}\nProducer state: blocked\nNext human step: await the required decision\nGit HEAD: {head}\n{drift}"
        );
        let (_, prompt_path) = write_prompt(
            fixture.path(),
            "human",
            &prompt(&checked(&t1, &head), "—", &handback),
        );
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, 0, "producer {producer}");
        assert!(output.contains("decision: human-required"));
        assert!(output.contains(&format!("reason: {reason}")));
    }

    let invalid = format!(
        "#### Human Handback — goal-gaps-blocked\nStatus: OPEN\nReason: goal-gaps-blocked\nTask: {t1}\nPhase: VERIFY\nProducer state: blocked\nNext human step: await the required decision\nGit HEAD: "
    );
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "invalid-human",
        &prompt(&checked(&t1, &head), "—", &invalid),
    );
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1);
    assert!(output.contains("decision: continue"));

    let retry = format!("#### Retry Handoff — {t1} / TEST\nStatus: OPEN\nTest Retry Count: 3\n");
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "retry",
        &prompt(&checked(&t1, &head), "—", &retry),
    );
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 0);
    assert!(output.contains("decision: retry-ceiling"));

    let (code, output) = run(
        fixture.path(),
        &prompt_path,
        &receipt,
        &["--stop-at".into(), task(99)],
    );
    assert_eq!(code, 0);
    assert!(output.contains("decision: retry-ceiling"));
}

#[test]
fn marked_terminal_recovery_verifies_terminal_receipt_sha256_binding() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let receipt = fixture.path().join("terminal-binding-test.md");
    let marked_prompt_text = format!(
        "Pipeline Contract: test-first-v1\n\n{}",
        prompt(&checked(&t1, &head), "—", "")
    );
    let (_, prompt_path) =
        write_prompt(fixture.path(), "terminal-binding-test", &marked_prompt_text);
    let goal_path = fixture
        .path()
        .join(".dev/pipeline/receipts/terminal-binding-test/goal-verification.receipt.md");
    std::fs::create_dir_all(goal_path.parent().unwrap()).unwrap();
    let canonical_prompt = std::fs::canonicalize(&prompt_path).unwrap();

    // Write a goal verification receipt that lacks terminal-receipt-sha256
    let goal_text = goal(&canonical_prompt, "", &head, &t1, &sha256(&prompt_path));
    std::fs::write(&goal_path, &goal_text).unwrap();

    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(
        code, 1,
        "marked prompt with missing terminal-receipt-sha256 must fail goal verification"
    );
    assert!(
        output.contains("terminal-receipt-sha256")
            || failed_goal_predicates(&output).contains(&"terminal-binding"),
        "pre-change goal receipts do not bind terminal-receipt-sha256; receipt:\n{output}"
    );
}

#[test]
fn markerless_prompt_goal_verification_retains_explicit_no_binding_semantics() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let receipt = fixture.path().join("markerless-terminal-binding-test.md");
    let markerless_prompt_text = prompt(&checked(&t1, &head), "—", "");
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "markerless-terminal-binding-test",
        &markerless_prompt_text,
    );
    let goal_path = fixture.path().join(
        ".dev/pipeline/receipts/markerless-terminal-binding-test/goal-verification.receipt.md",
    );
    std::fs::create_dir_all(goal_path.parent().unwrap()).unwrap();
    let canonical_prompt = std::fs::canonicalize(&prompt_path).unwrap();

    let goal_text = goal(&canonical_prompt, "", &head, &t1, &sha256(&prompt_path));
    std::fs::write(&goal_path, &goal_text).unwrap();

    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(
        code, 0,
        "markerless prompt must retain explicit no-binding semantics: {output}"
    );
    assert!(output.contains("decision: ready-to-finalize"));
    assert!(output.contains("goal_binding: pass"));
}

#[test]
fn marked_terminal_recovery_accepts_valid_bound_terminal_receipt() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let receipt = fixture.path().join("marked-valid-terminal-test.md");
    let marked_prompt_text = format!(
        "Pipeline Contract: test-first-v1\n\n{}",
        prompt(&checked(&t1, &head), "—", "")
    );
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "marked-valid-terminal-test",
        &marked_prompt_text,
    );
    let receipt_dir = fixture
        .path()
        .join(".dev/pipeline/receipts/marked-valid-terminal-test");
    std::fs::create_dir_all(&receipt_dir).unwrap();
    let canonical_prompt = std::fs::canonicalize(&prompt_path).unwrap();

    let terminal_path = receipt_dir.join("terminal-reverify.receipt.md");
    let prompt_sha = sha256(&prompt_path);
    let terminal_text = format!(
        "mode: terminal-reverify\noverall: pass\nprompt_path: {}\nprompt_sha256: {prompt_sha}\nhead: {head}\n",
        canonical_prompt.display()
    );
    std::fs::write(&terminal_path, &terminal_text).unwrap();
    let term_sha = sha256(&terminal_path);

    let goal_path = receipt_dir.join("goal-verification.receipt.md");
    let goal_text = format!(
        "{}\nterminal_receipt_sha256: {term_sha}\n",
        goal(&canonical_prompt, "", &head, &t1, &prompt_sha)
    );
    std::fs::write(&goal_path, &goal_text).unwrap();

    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(
        code, 0,
        "valid bound terminal receipt must allow ready-to-finalize: {output}"
    );
    assert!(output.contains("decision: ready-to-finalize"));
    assert!(output.contains("goal_binding: pass"));
    assert!(
        !failed_goal_predicates(&output).contains(&"terminal-binding"),
        "terminal-binding should pass: {output}"
    );
}

#[test]
fn marked_terminal_recovery_rejects_tampered_stale_deleted_and_substituted_receipts() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let receipt = fixture.path().join("marked-negative-terminal-test.md");
    let marked_prompt_text = format!(
        "Pipeline Contract: test-first-v1\n\n{}",
        prompt(&checked(&t1, &head), "—", "")
    );
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "marked-negative-terminal-test",
        &marked_prompt_text,
    );
    let receipt_dir = fixture
        .path()
        .join(".dev/pipeline/receipts/marked-negative-terminal-test");
    std::fs::create_dir_all(&receipt_dir).unwrap();
    let canonical_prompt = std::fs::canonicalize(&prompt_path).unwrap();
    let prompt_sha = sha256(&prompt_path);

    let terminal_path = receipt_dir.join("terminal-reverify.receipt.md");
    let goal_path = receipt_dir.join("goal-verification.receipt.md");

    // Case 1: Deleted-after terminal receipt
    let valid_terminal_text = format!(
        "mode: terminal-reverify\noverall: pass\nprompt_path: {}\nprompt_sha256: {prompt_sha}\nhead: {head}\n",
        canonical_prompt.display()
    );
    std::fs::write(&terminal_path, &valid_terminal_text).unwrap();
    let term_sha = sha256(&terminal_path);
    let goal_text = format!(
        "{}\nterminal_receipt_sha256: {term_sha}\n",
        goal(&canonical_prompt, "", &head, &t1, &prompt_sha)
    );
    std::fs::write(&goal_path, &goal_text).unwrap();
    std::fs::remove_file(&terminal_path).unwrap();

    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "deleted terminal receipt must fail");
    assert!(failed_goal_predicates(&output).contains(&"terminal-binding"));

    // Case 2: Stale / Tampered terminal receipt (SHA mismatch)
    let tampered_terminal_text = format!("{valid_terminal_text}\n# tampered\n");
    std::fs::write(&terminal_path, &tampered_terminal_text).unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "tampered terminal receipt must fail");
    assert!(failed_goal_predicates(&output).contains(&"terminal-binding"));

    // Case 3: Substituted receipt with overall: fail
    let failing_terminal_text = format!(
        "mode: terminal-reverify\noverall: fail\nprompt_path: {}\nprompt_sha256: {prompt_sha}\nhead: {head}\n",
        canonical_prompt.display()
    );
    std::fs::write(&terminal_path, &failing_terminal_text).unwrap();
    let fail_term_sha = sha256(&terminal_path);
    let goal_text = format!(
        "{}\nterminal_receipt_sha256: {fail_term_sha}\n",
        goal(&canonical_prompt, "", &head, &t1, &prompt_sha)
    );
    std::fs::write(&goal_path, &goal_text).unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "overall: fail terminal receipt must fail");
    assert!(failed_goal_predicates(&output).contains(&"terminal-binding"));

    // Case 4: Fail closed when prompt_path mismatches and goal record has no prompt_path
    let foreign_prompt_terminal = format!(
        "mode: terminal-reverify\noverall: pass\nprompt_path: foreign/path.prompt.md\nprompt_sha256: {prompt_sha}\nhead: {head}\n"
    );
    std::fs::write(&terminal_path, &foreign_prompt_terminal).unwrap();
    let foreign_term_sha = sha256(&terminal_path);
    let goal_no_path = format!(
        "prompt_sha256: {prompt_sha}\nhead: {head}\nverdict: VERIFIED\nchecked_tasks: {t1}\nmust_have_1: pass\ncommand: test\nterminal_receipt_sha256: {foreign_term_sha}\n"
    );
    std::fs::write(&goal_path, &goal_no_path).unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(
        code, 1,
        "mismatched prompt_path with missing goal prompt_path must fail closed"
    );
    assert!(failed_goal_predicates(&output).contains(&"terminal-binding"));
}
