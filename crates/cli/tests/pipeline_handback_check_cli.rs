//! Black-box fixtures for the continuation authority command.

use pipeline::coordinator::ExecutionBinding;
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

#[test]
fn legacy_boundary_and_converge_receipts_bind_standalone_runtime() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let task = task(1);
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "legacy-runtime-identity",
        &format!(
            "{}\n## Files to Create or Modify\n- `src/lib.rs`\n",
            prompt(&checked(&task, &head), "—", "")
        ),
    );
    git(
        fixture.path(),
        &["add", ".dev/plans/legacy-runtime-identity.prompt.md"],
    );
    std::fs::write(
        fixture.path().join(".dev/plans/legacy-runtime-identity.md"),
        format!("# fixture\n\n## Tasks\n{}\n", checked(&task, &head)),
    )
    .unwrap();
    git(
        fixture.path(),
        &["add", ".dev/plans/legacy-runtime-identity.md"],
    );
    git(fixture.path(), &["commit", "-q", "-m", "fixture prompt"]);
    for command in ["boundary-check", "pipeline-converge-check"] {
        let receipt = fixture.path().join(format!("{command}.receipt.md"));
        let output = Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(fixture.path())
            .args([
                command,
                prompt_path.to_str().unwrap(),
                "--task",
                &task,
                "--receipt",
                receipt.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        let text = std::fs::read_to_string(&receipt).unwrap_or_default();
        assert!(
            output.status.success(),
            "{command} exit={:?} stderr={} stdout={} receipt={text}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            text.contains("binding_scope: standalone"),
            "{command}: {text}"
        );
        assert!(
            text.contains("executable_path:") && text.contains("executable_sha256:"),
            "{command}: {text}"
        );
    }
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

/// Binding path form written by the guarded entry: no Windows verbatim
/// prefix, `/` separators.
fn binding_path_text(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\")
        .unwrap_or(&text)
        .replace('\\', "/")
}

fn execution_binding(repo: &Path, scope: &str) -> ExecutionBinding {
    let executable = std::fs::canonicalize(env!("CARGO_BIN_EXE_gal")).unwrap();
    ExecutionBinding {
        version: ExecutionBinding::VERSION,
        binding_scope: format!("coordinator:{scope}"),
        worktree_root: binding_path_text(&std::fs::canonicalize(repo).unwrap()),
        executable_path: binding_path_text(&executable),
        executable_sha256: sha256(&executable),
    }
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
        .join(".dev/pipeline/goal-predicate-matrix/goal-verification.receipt.md");
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
        "continue_action: /gal pipeline <prompt> from {t2} stop-at {t2}"
    )));
    assert!(!receipt_text.contains("continue_action: gal.exe pipeline-handback-check"));
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
        .join(".dev/pipeline/goal/goal-verification.receipt.md");
    std::fs::create_dir_all(goal_path.parent().unwrap()).unwrap();

    let checked_tasks = t1.clone();
    let canonical_prompt = std::fs::canonicalize(&prompt_path).unwrap();
    let foreign_goal = fixture
        .path()
        .join(".dev/pipeline/foreign/goal-verification.receipt.md");
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
    assert!(output.contains("continue_action: run-goal-backward-verification"));

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
    assert!(output.contains("continue_action: run-goal-backward-verification"));

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
    assert!(output.contains("decision: goal-verified"));
    assert!(output.contains("voluntary_response_authorized: true"));
    assert!(output.contains("continue_action: none"));
    assert!(!output.contains("await"));

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
    assert!(output.contains("continue_action: run-goal-backward-verification"));

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
        assert!(output.contains("continue_action: run-goal-backward-verification"));
    }

    let changed = prompt(&checked(&t1, &head), "—", "changed prompt");
    std::fs::write(&prompt_path, changed).unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1);
    assert!(output.contains("decision: continue"));
    assert!(output.contains("continue_action: run-goal-backward-verification"));
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
            "#### Human Handback — {reason}\nStatus: OPEN\nReason: {reason}\nTask: {t1}\nPhase: {phase}\nProducer: {producer}\nProducer state: blocked\nNext human step: await the required decision\nWhat to check: the blocked task output\nExpected result: the output matches the task goal\nPass/fail rule: pass when every goal item is met\nGit HEAD: {head}\n{drift}"
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
        let verification = Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(fixture.path())
            .args([
                "doctor",
                "--verify-receipt",
                receipt.to_str().unwrap(),
                "--expect-scope",
                "standalone",
            ])
            .output()
            .unwrap();
        assert!(
            verification.status.success(),
            "receipt verification failed for {reason}: {}",
            String::from_utf8_lossy(&verification.stderr)
        );
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

    let full = format!(
        "#### Human Handback — goal-gaps-blocked\nStatus: OPEN\nReason: goal-gaps-blocked\nTask: {t1}\nPhase: VERIFY\nProducer: VERIFY\nProducer state: blocked\nNext human step: await the required decision\nWhat to check: the blocked task output\nExpected result: the output matches the task goal\nPass/fail rule: pass when every goal item is met\nGit HEAD: {head}\n"
    );
    for (name, block, expected_code) in [
        (
            "missing-rule",
            full.replace("Pass/fail rule: pass when every goal item is met\n", ""),
            1,
        ),
        (
            "empty-check",
            full.replace("What to check: the blocked task output", "What to check:"),
            1,
        ),
        (
            "late-head",
            full.replace("Git HEAD:", "Note: a\nNote: b\nNote: c\nGit HEAD:"),
            0,
        ),
    ] {
        let (_, prompt_path) = write_prompt(
            fixture.path(),
            name,
            &prompt(&checked(&t1, &head), "—", &block),
        );
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, expected_code, "case {name}");
        let decision = if expected_code == 0 {
            "human-required"
        } else {
            "continue"
        };
        assert!(
            output.contains(&format!("decision: {decision}")),
            "case {name}"
        );
    }

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
    let goal_path = fixture
        .path()
        .join(".dev/pipeline/markerless-terminal-binding-test/goal-verification.receipt.md");
    std::fs::create_dir_all(goal_path.parent().unwrap()).unwrap();
    let canonical_prompt = std::fs::canonicalize(&prompt_path).unwrap();

    let goal_text = goal(&canonical_prompt, "", &head, &t1, &sha256(&prompt_path));
    std::fs::write(&goal_path, &goal_text).unwrap();

    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(
        code, 0,
        "markerless prompt must retain explicit no-binding semantics: {output}"
    );
    assert!(output.contains("decision: goal-verified"));
    assert!(output.contains("goal_binding: pass"));
}

#[test]
fn codex_stop_classification_succeeds_without_terminal_authorization() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "guarded-classification",
        &prompt(&checked(&t1, &head), "—", ""),
    );
    use pipeline::coordinator::{CheckpointKind, CoordinatorState};
    let coordinator = fixture
        .path()
        .join(".dev/pipeline/guarded-classification/coordinator.json");
    std::fs::create_dir_all(coordinator.parent().unwrap()).unwrap();
    let mut state =
        CoordinatorState::new_legacy("guarded-classification", "prompt", &t1, "implement");
    state
        .guarded_entry(execution_binding(fixture.path(), "guarded-classification"))
        .unwrap();
    state
        .activate_codex_stop_v1("activation-digest".into())
        .unwrap();
    state
        .checkpoint(
            "pending-checkpoint".into(),
            CheckpointKind::GoalBackward,
            Some(head.clone()),
        )
        .unwrap();
    std::fs::write(&coordinator, serde_json::to_vec(&state).unwrap()).unwrap();
    let receipt = fixture.path().join("guarded-receipt.md");
    let coordinator_before = sha256(&coordinator);

    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);

    assert_eq!(code, 0, "classification receipt should succeed: {output}");
    assert!(output.contains("decision: continue"));
    assert!(output.contains("voluntary_response_authorized: false"));
    assert!(output.contains("coordinator_revision: 3"));
    assert!(output.contains(r#"typed_action: {"kind":"await_orchestrator","binding":{"checkpoint_id":"pending-checkpoint"}}"#));
    assert!(output.contains("continue_action: {\"kind\":"));
    assert!(receipt.exists());
    for field in [
        "binding_scope: coordinator:guarded-classification",
        "execution_binding_sha256:",
        "executable_path:",
        "executable_sha256:",
        "plan_scope: guarded-classification",
        "coordinator_path:",
    ] {
        assert!(
            output.contains(field),
            "coordinator-bound handback receipt must include {field}: {output}"
        );
    }
    println!("valid-fixture coordinator_before={coordinator_before} coordinator_after={} checkpoint_state=embedded-in-coordinator receipt_before=absent receipt_after={}", sha256(&coordinator), sha256(&receipt));
}

#[test]
fn guarded_identity_mismatch_and_malformed_coordinator_fail_before_mutation() {
    use pipeline::coordinator::{CheckpointKind, CoordinatorState};

    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "identity-mismatch-fixture",
        &prompt(&checked(&t1, &head), "—", ""),
    );
    let pipeline_dir = fixture
        .path()
        .join(".dev/pipeline/identity-mismatch-fixture");
    std::fs::create_dir_all(&pipeline_dir).unwrap();
    let coordinator = pipeline_dir.join("coordinator.json");
    let checkpoint_receipt = pipeline_dir.join("orchestrator-receipt.json");
    let mut state =
        CoordinatorState::new_legacy("identity-mismatch-fixture", "prompt-hash", &t1, "implement");
    state
        .guarded_entry(execution_binding(
            fixture.path(),
            "identity-mismatch-fixture",
        ))
        .unwrap();
    state
        .activate_codex_stop_v1("activation-digest".into())
        .unwrap();
    state
        .checkpoint(
            "checkpoint".into(),
            CheckpointKind::GoalBackward,
            Some(head),
        )
        .unwrap();
    let receipt = fixture.path().join("identity-mismatch.md");
    std::fs::write(&coordinator, serde_json::to_vec(&state).unwrap()).unwrap();
    std::fs::write(&checkpoint_receipt, b"checkpoint receipt sentinel").unwrap();
    let coordinator_before = sha256(&coordinator);

    let binding = state.execution_binding.as_mut().unwrap();
    binding.executable_sha256 = "0".repeat(64);
    std::fs::write(&coordinator, serde_json::to_vec(&state).unwrap()).unwrap();
    let mismatched_coordinator = sha256(&coordinator);
    let checkpoint_before = sha256(&checkpoint_receipt);
    let (code, _) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "mismatched executable digest must fail closed");
    assert_eq!(sha256(&coordinator), mismatched_coordinator);
    assert_eq!(sha256(&checkpoint_receipt), checkpoint_before);
    assert!(!receipt.exists());
    println!("identity-fixture coordinator_before={mismatched_coordinator} coordinator_after={} checkpoint_before={checkpoint_before} checkpoint_after={} receipt_before=absent receipt_after=absent mismatch_total=1", sha256(&coordinator), sha256(&checkpoint_receipt));

    std::fs::write(&coordinator, b"not-json").unwrap();
    let malformed_before = sha256(&coordinator);
    let checkpoint_before = sha256(&checkpoint_receipt);
    let (code, _) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "malformed coordinator must fail closed");
    assert_eq!(sha256(&coordinator), malformed_before);
    assert_eq!(sha256(&checkpoint_receipt), checkpoint_before);
    assert!(!receipt.exists());
    println!("malformed-fixture coordinator_before={malformed_before} coordinator_after={} checkpoint_before={checkpoint_before} checkpoint_after={} receipt_before=absent receipt_after=absent mismatch_total=1", sha256(&coordinator), sha256(&checkpoint_receipt));
    assert_ne!(coordinator_before, mismatched_coordinator);
}

#[test]
fn command_consumes_checkpoint_receipt_once_and_binds_revision_and_commit() {
    use pipeline::coordinator::{CheckpointKind, CheckpointReceipt, CoordinatorState};

    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "command-receipt-binding",
        &prompt(&checked(&t1, &head), "—", ""),
    );
    let pipeline_dir = fixture.path().join(".dev/pipeline/command-receipt-binding");
    std::fs::create_dir_all(&pipeline_dir).unwrap();
    let coordinator = pipeline_dir.join("coordinator.json");
    let mut state =
        CoordinatorState::new_legacy("command-receipt-binding", "prompt-hash", &t1, "implement");
    state
        .guarded_entry(execution_binding(fixture.path(), "command-receipt-binding"))
        .unwrap();
    let binding_digest = state.execution_binding.as_ref().unwrap().digest().unwrap();
    state.activate_codex_stop_v1("activation".into()).unwrap();
    let revision = state
        .checkpoint(
            "checkpoint-1".into(),
            CheckpointKind::GoalBackward,
            Some(head.clone()),
        )
        .unwrap();
    std::fs::write(&coordinator, serde_json::to_vec(&state).unwrap()).unwrap();
    let receipt_path = pipeline_dir.join("orchestrator-receipt.json");
    let receipt = CheckpointReceipt {
        receipt_id: "receipt-1".into(),
        checkpoint_id: "checkpoint-1".into(),
        revision,
        prompt_sha256: "prompt-hash".into(),
        execution_binding_sha256: binding_digest,
        commit: Some(head.clone()),
        accepted: true,
    };
    let output_receipt = fixture.path().join("command-receipt.md");

    std::fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &output_receipt, &[]);
    assert_eq!(code, 0, "valid receipt rejected: {output}");
    let consumed: CoordinatorState =
        serde_json::from_slice(&std::fs::read(&coordinator).unwrap()).unwrap();
    assert_eq!(consumed.revision, revision + 1);
    assert_eq!(consumed.consumed_checkpoint_receipts, ["receipt-1"]);
    assert!(!receipt_path.exists());

    std::fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    let (code, _) = run(fixture.path(), &prompt_path, &output_receipt, &[]);
    assert_eq!(code, 1, "replayed receipt must fail through command");

    let mut wrong_revision = consumed.clone();
    wrong_revision
        .checkpoint
        .as_mut()
        .unwrap()
        .consumed_receipt_id = None;
    wrong_revision.consumed_checkpoint_receipts.clear();
    std::fs::write(&coordinator, serde_json::to_vec(&wrong_revision).unwrap()).unwrap();
    let mut bad = receipt.clone();
    bad.receipt_id = "receipt-wrong-revision".into();
    bad.revision += 9;
    std::fs::write(&receipt_path, serde_json::to_vec(&bad).unwrap()).unwrap();
    let (code, _) = run(fixture.path(), &prompt_path, &output_receipt, &[]);
    assert_eq!(code, 1, "wrong revision must fail through command");

    let mut bad_commit = receipt;
    bad_commit.receipt_id = "receipt-wrong-commit".into();
    bad_commit.commit = Some("0".repeat(40));
    std::fs::write(&receipt_path, serde_json::to_vec(&bad_commit).unwrap()).unwrap();
    let (code, _) = run(fixture.path(), &prompt_path, &output_receipt, &[]);
    assert_eq!(code, 1, "wrong commit must fail through command");
}

#[test]
fn typed_action_serialization_round_trips_every_action_variant() {
    use pipeline::coordinator::NextAction;

    let actions = vec![
        NextAction::StartTask {
            task_id: task(1),
            phase: "implement".into(),
        },
        NextAction::ResumeTask {
            task_id: task(1),
            phase: "test".into(),
        },
        NextAction::AwaitOrchestrator {
            checkpoint_id: "chk-01".into(),
        },
        NextAction::RecoverAttempt {
            attempt_id: "attempt-01".into(),
        },
        NextAction::RetryTask {
            task_id: task(1),
            phase: "test".into(),
            attempt: 2,
        },
        NextAction::Complete,
        NextAction::Blocked {
            reason: "manual-intervention-required".into(),
        },
    ];

    for action in actions {
        let serialized = serde_json::to_string(&action).expect("serialize action");
        let deserialized: NextAction =
            serde_json::from_str(&serialized).expect("deserialize action");
        assert_eq!(action, deserialized, "round-trip mismatch for {serialized}");
    }
}

#[test]
fn checkpoint_receipt_replay_rejection_and_wrong_revision() {
    use pipeline::coordinator::{
        CheckpointKind, CheckpointReceipt, CoordinatorError, CoordinatorState, NextAction,
    };

    let mut state = CoordinatorState::new_legacy("plan-scope", "prompt-sha", task(1), "implement");
    state
        .guarded_entry(execution_binding(Path::new("."), "plan-scope"))
        .unwrap();
    state
        .activate_codex_stop_v1("activation-digest-001".into())
        .expect("activate");
    let binding_digest = state.execution_binding.as_ref().unwrap().digest().unwrap();

    let rev = state
        .checkpoint(
            "chk-tp11-test".into(),
            CheckpointKind::GoalBackward,
            Some("commit-hash-001".into()),
        )
        .expect("create checkpoint");

    assert_eq!(
        state.next_action,
        NextAction::AwaitOrchestrator {
            checkpoint_id: "chk-tp11-test".into()
        }
    );

    // Wrong revision failure
    let wrong_rev_receipt = CheckpointReceipt {
        receipt_id: "rcpt-001".into(),
        checkpoint_id: "chk-tp11-test".into(),
        revision: rev + 10,
        prompt_sha256: "prompt-sha".into(),
        execution_binding_sha256: binding_digest.clone(),
        commit: Some("commit-hash-001".into()),
        accepted: true,
    };
    assert_eq!(
        state
            .consume_checkpoint_receipt(wrong_rev_receipt)
            .unwrap_err(),
        CoordinatorError::CheckpointMismatch
    );

    // Valid receipt consumed
    let valid_receipt = CheckpointReceipt {
        receipt_id: "rcpt-001".into(),
        checkpoint_id: "chk-tp11-test".into(),
        revision: rev,
        prompt_sha256: "prompt-sha".into(),
        execution_binding_sha256: binding_digest.clone(),
        commit: Some("commit-hash-001".into()),
        accepted: true,
    };
    let new_rev = state
        .consume_checkpoint_receipt(valid_receipt.clone())
        .expect("consume valid receipt");
    assert!(new_rev > rev);
    assert_eq!(
        state.consumed_checkpoint_receipts,
        vec!["rcpt-001".to_string()]
    );

    // Replay rejection (same receipt ID already consumed)
    assert_eq!(
        state.consume_checkpoint_receipt(valid_receipt).unwrap_err(),
        CoordinatorError::ReceiptReplay
    );

    // Checkpoint already consumed: new receipt for same checkpoint rejected
    let second_receipt = CheckpointReceipt {
        receipt_id: "rcpt-002".into(),
        checkpoint_id: "chk-tp11-test".into(),
        revision: rev,
        prompt_sha256: "prompt-sha".into(),
        execution_binding_sha256: binding_digest,
        commit: Some("commit-hash-001".into()),
        accepted: true,
    };
    assert_eq!(
        state
            .consume_checkpoint_receipt(second_receipt)
            .unwrap_err(),
        CoordinatorError::ReceiptReplay
    );
}

#[test]
fn journal_conflict_fails_closed_before_classification() {
    use pipeline::projection_journal::{apply_with_hook, Boundary, JournalError, Projection};

    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let plan_name = "journal-conflict-scope";
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        plan_name,
        &prompt(&checked(&t1, &head), "—", ""),
    );
    let pipeline_dir = fixture.path().join(".dev/pipeline").join(plan_name);
    std::fs::create_dir_all(&pipeline_dir).unwrap();

    let coordinator = pipeline_dir.join("coordinator.json");
    std::fs::write(&coordinator, r#"{"profile":"codex_stop_v1","revision":1}"#).unwrap();

    // Prepare 3 projections
    let target1 = pipeline_dir.join("p1.md");
    let target2 = pipeline_dir.join("p2.md");
    let target3 = pipeline_dir.join("p3.md");
    std::fs::write(&target1, b"initial 1").unwrap();
    std::fs::write(&target2, b"initial 2").unwrap();
    std::fs::write(&target3, b"initial 3").unwrap();

    let journal_path = pipeline_dir.join("projection-journal.json");
    let projections = [
        Projection {
            path: target1.clone(),
            bytes: b"new 1".to_vec(),
        },
        Projection {
            path: target2.clone(),
            bytes: b"new 2".to_vec(),
        },
        Projection {
            path: target3.clone(),
            bytes: b"new 3".to_vec(),
        },
    ];

    // Interrupt after journal is prepared (uncommitted)
    let _ = apply_with_hook(&journal_path, projections, |boundary| {
        if boundary == Boundary::JournalPrepared {
            Err(JournalError::Interrupted(boundary))
        } else {
            Ok(())
        }
    });

    // Induce conflict by mutating a target to a third hash
    std::fs::write(&target1, b"conflicting third-party write").unwrap();

    let receipt = fixture.path().join("conflict-receipt.md");
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);

    assert_eq!(
        code, 1,
        "journal conflict must fail closed with exit code 1: {output}"
    );
    assert!(
        !receipt.exists(),
        "receipt must not be written when journal recovery fails"
    );
}
