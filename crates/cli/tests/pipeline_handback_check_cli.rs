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

fn test_gal_exe() -> &'static Path {
    static TEST_EXE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    TEST_EXE.get_or_init(|| {
        let src = PathBuf::from(env!("CARGO_BIN_EXE_gal"));
        let needs_copy = src.components().any(|c| {
            ["worktrees", ".worktrees"]
                .iter()
                .any(|name| c.as_os_str().to_string_lossy().eq_ignore_ascii_case(name))
        });
        if needs_copy {
            let tmp_bin_dir =
                std::env::temp_dir().join(format!("gal-test-bin-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&tmp_bin_dir);
            let dst = tmp_bin_dir.join(src.file_name().unwrap());
            let _ = std::fs::copy(&src, &dst);
            dst
        } else {
            src
        }
    })
}

fn run(repo: &Path, prompt: &Path, receipt: &Path, extra: &[String]) -> (i32, String) {
    let output = Command::new(test_gal_exe())
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

fn run_with_home(
    repo: &Path,
    prompt: &Path,
    receipt: &Path,
    extra: &[String],
    home: &Path,
) -> (i32, String) {
    let output = Command::new(test_gal_exe())
        .current_dir(repo)
        .env("HOME", home)
        .env("USERPROFILE", home)
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
fn effective_routed_attempt_ignores_superseded_mode_and_requires_valid_witness() {
    let fixture = repo();
    let task = task(1);
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "effective-routed-attempt",
        &prompt(
            &format!("- [ ] {task} implement",),
            "—",
            "Verification Independence: DEGRADED_HOST_SUBAGENT",
        ),
    );
    let scope = fixture
        .path()
        .join(".dev/pipeline/effective-routed-attempt");
    let home = fixture.path().join("test-home");
    let config_dir = home.join(".gal/config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.json"),
        r#"{"executorRouting":{"executors":{"codex":"fixture-model"},"pipeline":{"CODER":{"executor":"codex"}}}}"#,
    )
    .unwrap();
    let task_dir = scope.join(&task);
    std::fs::create_dir_all(&task_dir).unwrap();
    std::fs::write(
        scope.join("preflight.receipt.md"),
        "host: none\ndispatch_mode: routed\nrun_id: fixture-run\n",
    )
    .unwrap();
    let log = |ordinal: u32, executor: &str, terminal: &str, contract: &str| {
        std::fs::write(
            task_dir.join(format!("{ordinal:06}-000001-000001-{task}-implement.log")),
            format!("GAL-DISPATCH-LOG v1\nevidence_contract: {contract}\nexecutor: {executor}\n{}task_id: {task}\nphase: implement\ntimestamp_start: 2026-10-15T00:00:0{ordinal}Z\nterminal_state: {terminal}\nattempt_id: attempt-{ordinal}\nsession_id: session-{ordinal}\nactual_model: model-{ordinal}\nactual_effort: high\ntier: standard\ntier_source: standard\nmodel_tier_applied: yes\n---STDOUT---\n", if executor == "host-subagent" { "host: codex\n" } else { "" }),
        )
        .unwrap();
    };
    log(1, "host-subagent", "completed", "v1");
    log(2, "codex", "completed", "v1");
    log(3, "codex", "no-writeback", "v1");

    let receipt = fixture.path().join("effective-routed-attempt.md");
    let original_logs = [
        task_dir.join(format!("{:06}-000001-000001-{task}-implement.log", 1)),
        task_dir.join(format!("{:06}-000001-000001-{task}-implement.log", 2)),
        task_dir.join(format!("{:06}-000001-000001-{task}-implement.log", 3)),
    ]
    .map(|path| std::fs::read(&path).unwrap());
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);

    assert!(
        output.contains("decision: continue"),
        "receipt missing: {output}"
    );
    assert!(
        !output.contains("host_dispatch_evidence: fail"),
        "valid routed witness and effective record should pass: {output}"
    );
    assert_eq!(
        original_logs,
        [
            task_dir.join(format!("{:06}-000001-000001-{task}-implement.log", 1)),
            task_dir.join(format!("{:06}-000001-000001-{task}-implement.log", 2)),
            task_dir.join(format!("{:06}-000001-000001-{task}-implement.log", 3)),
        ]
        .map(|path| std::fs::read(path).unwrap()),
        "handback command must not mutate retained attempt logs"
    );

    for route_config in [
        Some(
            r#"{"executorRouting":{"executors":{"codex":"fixture-model"},"pipeline":{"CODER":{"executor":"different-model"}}}}"#,
        ),
        None,
        Some("not-json"),
    ] {
        match route_config {
            Some(config) => std::fs::write(config_dir.join("config.json"), config).unwrap(),
            None => std::fs::remove_file(config_dir.join("config.json")).unwrap(),
        }
        let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
        assert!(
            !output.contains("host_dispatch_evidence: fail"),
            "routed evidence must not depend on live routing: {output}"
        );
        assert!(
            !output.contains("host_dispatch_evidence_issue:"),
            "successful routed evidence must not emit issue lines: {output}"
        );
        assert!(output.contains("decision: continue"), "{output}");
    }
    std::fs::write(
        config_dir.join("config.json"),
        r#"{"executorRouting":{"executors":{"codex":"fixture-model"},"pipeline":{"CODER":{"executor":"codex"}}}}"#,
    )
    .unwrap();

    log(3, "codex", "no-writeback", "v2");
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(
        output.contains("host_dispatch_evidence: fail"),
        "effective incomplete-v2 record must fail: {output}"
    );
    assert!(output.contains(&format!(
        "host_dispatch_evidence_issue: task={task} phase=implement reason=not-completed"
    )));

    log(3, "codex", "no-writeback", "v1");
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(
        !output.contains("host_dispatch_evidence: fail"),
        "a valid retry after failed effective evidence must pass: {output}"
    );

    log(2, "codex", "started", "v1");
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(
        output.contains("host_dispatch_evidence: fail"),
        "a noncompleted earlier attempt must not witness routed no-writeback: {output}"
    );
    assert!(output.contains(&format!(
        "host_dispatch_evidence_issue: task={task} phase=implement reason=no-writeback-without-witness"
    )));

    log(2, "codex", "completed", "v2");
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(
        output.contains("host_dispatch_evidence: fail"),
        "an incomplete-v2 earlier attempt must not witness routed no-writeback: {output}"
    );

    log(2, "codex", "completed", "v1");
    std::fs::remove_file(scope.join("preflight.receipt.md")).unwrap();
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(
        output.contains("host_dispatch_evidence: fail"),
        "retained host evidence without a pin must fail: {output}"
    );

    // A routed pin that names no host cannot own host-subagent attempts.
    std::fs::write(
        scope.join("preflight.receipt.md"),
        "host: none\ndispatch_mode: routed\nrun_id: fixture-run\n",
    )
    .unwrap();
    log(1, "host-subagent", "completed", "v1");
    log(2, "host-subagent", "completed", "v1");
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(
        output.contains("decision: continue"),
        "receipt missing: {output}"
    );
    assert!(
        output.contains("host_dispatch_evidence: fail"),
        "an incompatible witness must fail closed: {output}"
    );
    let checked_text = prompt(
        &checked(&task, "0000000000000000000000000000000000000000"),
        "—",
        "Verification Independence: DEGRADED_HOST_SUBAGENT",
    );
    std::fs::write(&prompt_path, checked_text).unwrap();
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(
        output.contains("host_dispatch_evidence: fail"),
        "checked-task receipt must reject invalid routed evidence: {output}"
    );

    std::fs::write(
        scope.join("preflight.receipt.md"),
        "dispatch_mode: invalid\n",
    )
    .unwrap();
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(
        output.contains("host_dispatch_evidence: fail"),
        "an invalid preflight pin must fail closed: {output}"
    );

    std::fs::write(
        scope.join("preflight.receipt.md"),
        "host: none\ndispatch_mode: routed\nrun_id: fixture-run\n",
    )
    .unwrap();
    std::fs::write(
        task_dir.join(format!("000004-000001-000001-{task}-implement.log")),
        "not a dispatch log",
    )
    .unwrap();
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(
        output.contains("host_dispatch_evidence: fail"),
        "malformed retained history must fail closed: {output}"
    );
    assert!(output.contains(&format!(
        "host_dispatch_evidence_issue: task={task} phase=implement reason=malformed-log"
    )));

    // A readable log with a routed-mode contradiction is attributable to the
    // task and phase even though its evidence is rejected.
    std::fs::write(
        task_dir.join(format!("000004-000001-000001-{task}-implement.log")),
        format!(
            "GAL-DISPATCH-LOG v1\nevidence_contract: v1\nexecutor: codex\nhost: codex\ntask_id: {task}\nphase: implement\ntimestamp_start: 2026-10-15T00:00:04Z\nterminal_state: completed\nattempt_id: attempt-4\nsession_id: session-4\nactual_model: fixture-model\nactual_effort: high\ntier: standard\ntier_source: standard\nmodel_tier_applied: yes\n---STDOUT---\n"
        ),
    )
    .unwrap();
    std::fs::write(
        task_dir.join(format!("000004-000001-000001-{task}-test.log")),
        "not a dispatch log",
    )
    .unwrap();
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(output.contains("host_dispatch_evidence: fail"), "{output}");
    assert!(output.contains(&format!(
        "host_dispatch_evidence_issue: task={task} phase=implement reason=mode-mismatch"
    )));
    let implement_issue = output
        .find(&format!(
            "host_dispatch_evidence_issue: task={task} phase=implement reason=mode-mismatch"
        ))
        .unwrap();
    let test_issue = output
        .find(&format!(
            "host_dispatch_evidence_issue: task={task} phase=test reason=malformed-log"
        ))
        .unwrap();
    assert!(
        implement_issue < test_issue,
        "issue lines are not sorted: {output}"
    );

    std::fs::write(
        scope.join("preflight.receipt.md"),
        "host: codex\ndispatch_mode: host-subagent\nrun_id: fixture-run\n",
    )
    .unwrap();
    std::fs::write(
        task_dir.join(format!("000005-000001-000001-{task}-implement.log")),
        format!(
            "GAL-DISPATCH-LOG v1\nevidence_contract: v1\nexecutor: another-executor\ntask_id: {task}\nphase: implement\ntimestamp_start: 2026-10-15T00:00:05Z\nterminal_state: completed\nattempt_id: attempt-5\nsession_id: session-5\nactual_model: fixture-model\nactual_effort: high\ntier: standard\ntier_source: standard\nmodel_tier_applied: yes\n---STDOUT---\n"
        ),
    )
    .unwrap();
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(output.contains("host_dispatch_evidence: fail"), "{output}");
    assert!(output.contains(&format!(
        "host_dispatch_evidence_issue: task={task} phase=implement reason=executor-mismatch"
    )));

    std::fs::remove_dir_all(&scope).unwrap();
    std::fs::write(&scope, "not a directory").unwrap();
    let (_, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert!(output.contains("host_dispatch_evidence: fail"), "{output}");
    assert!(output.contains(&format!(
        "host_dispatch_evidence_issue: task=unknown phase=unknown reason=unreadable-log-directory"
    )));
}

#[test]
fn invalid_routed_evidence_keeps_all_checked_goal_on_handback_path() {
    let fixture = repo();
    let head = initial_commit(fixture.path());
    let task = task(1);
    let checked_tasks = checked(&task, &head);
    let plan_name = "checked-invalid-routed-evidence";
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        plan_name,
        &prompt(
            &checked_tasks,
            "—",
            "Verification Independence: DEGRADED_HOST_SUBAGENT",
        ),
    );
    let scope = fixture.path().join(".dev/pipeline").join(plan_name);
    let task_dir = scope.join(&task);
    let home = fixture.path().join("test-home");
    let config_dir = home.join(".gal/config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.json"),
        r#"{"executorRouting":{"executors":{"codex":"fixture-model"},"pipeline":{"CODER":{"executor":"codex"}}}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(&task_dir).unwrap();
    std::fs::write(
        scope.join("preflight.receipt.md"),
        "host: none\ndispatch_mode: routed\nrun_id: fixture-run\n",
    )
    .unwrap();
    std::fs::write(
        task_dir.join(format!("000001-000001-000001-{task}-implement.log")),
        format!(
            "GAL-DISPATCH-LOG v1\nevidence_contract: v1\nexecutor: host-subagent\nhost: codex\ntask_id: {task}\nphase: implement\ntimestamp_start: 2026-10-15T00:00:01Z\nterminal_state: completed\nattempt_id: attempt-1\nsession_id: session-1\nactual_model: fixture-model\nactual_effort: high\ntier: standard\ntier_source: standard\nmodel_tier_applied: yes\n---STDOUT---\n"
        ),
    )
    .unwrap();
    let goal_path = scope.join("goal-verification.receipt.md");
    let canonical_prompt = std::fs::canonicalize(&prompt_path).unwrap();
    std::fs::write(
        &goal_path,
        goal(&canonical_prompt, "", &head, &task, &sha256(&prompt_path)),
    )
    .unwrap();

    let receipt = fixture.path().join("checked-invalid-routed-evidence.md");
    let (code, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);

    assert_eq!(code, 1, "invalid host evidence must fail closed: {output}");
    assert!(output.contains("host_dispatch_evidence: fail"), "{output}");
    assert!(output.contains("decision: continue"), "{output}");
    assert!(!output.contains("decision: goal-verified"), "{output}");
    assert!(
        output.contains("continue_action: run-goal-backward-verification"),
        "checked tasks must retain the handback action: {output}"
    );
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
        let output = Command::new(test_gal_exe())
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
    let executable = std::fs::canonicalize(test_gal_exe()).unwrap();
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
        let verification = Command::new(test_gal_exe())
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

    let retry =
        format!("Test Retry Count: 3\n\n#### Retry Handoff — {t1} / IMPLEMENT\nStatus: OPEN\n");
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "retry",
        &prompt(&checked(&t1, &head), &t1, &retry),
    );
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 0);
    assert!(output.contains("decision: retry-ceiling"));

    let dashed_retry =
        format!("Test Retry Count: 3\n\n#### Retry Handoff — {t1} / IMPLEMENT\n- Status: OPEN\n");
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "retry-dashed-open",
        &prompt(&checked(&t1, &head), &t1, &dashed_retry),
    );
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 0);
    assert!(output.contains("decision: retry-ceiling"));

    let t2 = task(2);
    for (name, current, status_fields, handoff, expected) in [
        (
            "review-count",
            &t1,
            "Review Retry Count: 3\n",
            "#### Retry Handoff — {t1} / IMPLEMENT\nStatus: OPEN\n",
            true,
        ),
        (
            "test-below",
            &t1,
            "Test Retry Count: 2\n",
            "#### Retry Handoff — {t1} / AUDIT\nStatus: OPEN\n",
            false,
        ),
        (
            "review-below",
            &t1,
            "Review Retry Count: 2\n",
            "#### Retry Handoff — {t1} / IMPLEMENT\nStatus: OPEN\n",
            false,
        ),
        (
            "test-counter-review-handoff",
            &t1,
            "Test Retry Count: 3\n",
            "#### Retry Handoff — {t1} / TEST\nStatus: OPEN\n",
            true,
        ),
        (
            "review-counter-test-handoff",
            &t1,
            "Review Retry Count: 3\n",
            "#### Retry Handoff — {t1} / AUDIT\nStatus: OPEN\n",
            true,
        ),
        (
            "task-mismatch",
            &t2,
            "Test Retry Count: 3\n",
            "#### Retry Handoff — {t1} / TEST\nStatus: OPEN\n",
            false,
        ),
        (
            "handoff-task-mismatch",
            &t1,
            "Test Retry Count: 3\n",
            "#### Retry Handoff — {t2} / TEST\nStatus: OPEN\n",
            false,
        ),
        (
            "implement-phase",
            &t1,
            "Test Retry Count: 3\n",
            "#### Retry Handoff — {t1} / IMPLEMENT\nStatus: OPEN\n",
            true,
        ),
        (
            "resolved",
            &t1,
            "Test Retry Count: 3\n",
            "#### Retry Handoff — {t1} / TEST\nStatus: RESOLVED\n",
            false,
        ),
        (
            "resolved-dashed",
            &t1,
            "Test Retry Count: 3\n",
            "#### Retry Handoff — {t1} / TEST\n- Status: RESOLVED\n",
            false,
        ),
        (
            "resolved-then-peer-section-open",
            &t1,
            "Test Retry Count: 3\n",
            "#### Retry Handoff — {t1} / TEST\nStatus: RESOLVED\n\n### Review Results\nStatus: OPEN\n",
            false,
        ),
        (
            "resolved-then-enclosing-section-open",
            &t1,
            "Test Retry Count: 3\n",
            "#### Retry Handoff — {t1} / TEST\nStatus: RESOLVED\n\n## Debug Log\n- Status: OPEN\n",
            false,
        ),
        (
            "open-under-deeper-heading",
            &t1,
            "Test Retry Count: 3\n",
            "#### Retry Handoff — {t1} / TEST\n##### Detail\nStatus: OPEN\n",
            true,
        ),
        (
            "generic-counter",
            &t1,
            "Retry Count: 3\n",
            "#### Retry Handoff — {t1} / TEST\nStatus: OPEN\n",
            false,
        ),
        (
            "security-counter",
            &t1,
            "Security Retry Count: 3\n",
            "#### Retry Handoff — {t1} / TEST\nStatus: OPEN\n",
            false,
        ),
    ] {
        let handoff = handoff.replace("{t1}", &t1).replace("{t2}", &t2);
        let (_, case_path) = write_prompt(
            fixture.path(),
            name,
            &prompt(
                &checked(&t1, &head),
                current,
                &format!("{status_fields}\n{handoff}"),
            ),
        );
        let (_, case_output) = run(fixture.path(), &case_path, &receipt, &[]);
        let decision = if expected {
            "retry-ceiling"
        } else {
            "continue"
        };
        assert!(
            case_output.contains(&format!("decision: {decision}")),
            "case {name}: {case_output}"
        );
    }

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
fn codex_stop_handback_accepts_structured_security_pass_evidence() {
    use pipeline::coordinator::CoordinatorState;

    let fixture = repo();
    let head = initial_commit(fixture.path());
    let task_id = task(1);
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "structured-security-pass",
        &prompt(&checked(&task_id, &head), "—", ""),
    );
    let coordinator = fixture
        .path()
        .join(".dev/pipeline/structured-security-pass/coordinator.json");
    std::fs::create_dir_all(coordinator.parent().unwrap()).unwrap();
    let mut state =
        CoordinatorState::new_legacy("structured-security-pass", "prompt", &task_id, "audit");
    state
        .guarded_entry(execution_binding(
            fixture.path(),
            "structured-security-pass",
        ))
        .unwrap();
    state
        .activate_codex_stop_v1("activation-digest".into())
        .unwrap();
    state.last_verified_evidence = Some(format!(
        "Pipeline Evidence: {{\"schema_version\":1,\"task_id\":\"{task_id}\",\"repair_round\":0,\"attempt_id\":\"audit-attempt\",\"implement_fix_attempt_id\":null,\"prompt_sha256\":\"{}\",\"head_sha\":\"{}\",\"producer\":\"AUDITOR\",\"executor_terminal_state\":\"completed\",\"security_gate\":\"pass\",\"requires_out_of_allowlist_fix\":false,\"gates\":[{{\"gate_id\":\"fixture-gate\",\"premise_id\":\"fixture-premise\",\"premise_text\":\"fixture premise\",\"outcome\":\"pass\",\"findings\":[]}}]}}",
        "a".repeat(64),
        "b".repeat(40)
    ));
    std::fs::write(&coordinator, serde_json::to_vec(&state).unwrap()).unwrap();

    let receipt = fixture.path().join("structured-security-pass-receipt.md");
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);

    assert_eq!(
        code, 0,
        "structured security pass should allow handback: {output}"
    );
    assert!(output.contains("decision: continue"), "{output}");
    assert!(!output.contains("security-protected-path"), "{output}");
    assert!(receipt.exists());
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
fn unconsumed_boundary_widening_receipt_stays_on_disk_and_coordinator_is_untouched() {
    use pipeline::coordinator::{CheckpointKind, CheckpointReceipt, CoordinatorState};

    let fixture = repo();
    let head = initial_commit(fixture.path());
    let t1 = task(1);
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        "boundary-widening-receipt",
        &prompt(&checked(&t1, &head), "—", ""),
    );
    let pipeline_dir = fixture
        .path()
        .join(".dev/pipeline/boundary-widening-receipt");
    std::fs::create_dir_all(&pipeline_dir).unwrap();
    let coordinator = pipeline_dir.join("coordinator.json");
    let mut state =
        CoordinatorState::new_legacy("boundary-widening-receipt", "prompt-hash", &t1, "implement");
    state
        .guarded_entry(execution_binding(
            fixture.path(),
            "boundary-widening-receipt",
        ))
        .unwrap();
    let binding_digest = state.execution_binding.as_ref().unwrap().digest().unwrap();
    state.activate_codex_stop_v1("activation".into()).unwrap();
    let revision = state
        .checkpoint(
            "checkpoint-boundary".into(),
            CheckpointKind::BoundaryWidening,
            Some(head.clone()),
        )
        .unwrap();
    std::fs::write(&coordinator, serde_json::to_vec(&state).unwrap()).unwrap();
    let coordinator_before = sha256(&coordinator);
    let receipt_path = pipeline_dir.join("orchestrator-receipt.json");
    let receipt = CheckpointReceipt {
        receipt_id: "receipt-boundary".into(),
        checkpoint_id: "checkpoint-boundary".into(),
        revision,
        prompt_sha256: "prompt-hash".into(),
        execution_binding_sha256: binding_digest,
        commit: Some(head.clone()),
        accepted: true,
    };
    std::fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    let receipt_before = sha256(&receipt_path);
    let output_receipt = fixture.path().join("boundary-widening-receipt.md");

    let (_code, _output) = run(fixture.path(), &prompt_path, &output_receipt, &[]);

    assert_eq!(
        sha256(&coordinator),
        coordinator_before,
        "non-GoalBackward receipt must not write the coordinator"
    );
    assert!(
        receipt_path.exists(),
        "unconsumed receipt must stay on disk"
    );
    assert_eq!(sha256(&receipt_path), receipt_before);
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

fn write_phase_attempt(
    task_dir: &Path,
    task: &str,
    phase: &str,
    ordinal: u32,
    executor: &str,
    host: &str,
    terminal: &str,
) {
    std::fs::create_dir_all(task_dir).unwrap();
    let host_line = if host.is_empty() {
        String::new()
    } else {
        format!("host: {host}\n")
    };
    std::fs::write(
        task_dir.join(format!("{ordinal:06}-000001-000001-{task}-{phase}.log")),
        format!("GAL-DISPATCH-LOG v1\nevidence_contract: v1\nexecutor: {executor}\n{host_line}task_id: {task}\nphase: {phase}\ntimestamp_start: 2026-10-15T00:00:0{ordinal}Z\nterminal_state: {terminal}\nattempt_id: attempt-{ordinal}\nsession_id: session-{ordinal}\nactual_model: model-{ordinal}\nactual_effort: high\ntier: standard\ntier_source: standard\nmodel_tier_applied: yes\n---STDOUT---\n"),
    )
    .unwrap();
}

fn write_attempt(
    task_dir: &Path,
    task: &str,
    ordinal: u32,
    executor: &str,
    host: &str,
    terminal: &str,
) {
    write_phase_attempt(
        task_dir,
        task,
        "implement",
        ordinal,
        executor,
        host,
        terminal,
    );
}

fn write_pin(scope: &Path, host: &str, mode: &str) {
    std::fs::write(
        scope.join("preflight.receipt.md"),
        format!("host: {host}\ndispatch_mode: {mode}\nrun_id: fixture-run\n"),
    )
    .unwrap();
}

const HOST_MARKER: &str = "Verification Independence: DEGRADED_HOST_SUBAGENT";
const FINALIZE_REVIEW_SCOPE: &str = "finalize-review";

fn issue_lines(receipt: &str) -> Vec<&str> {
    receipt
        .lines()
        .filter(|line| line.starts_with("host_dispatch_evidence_issue: "))
        .collect()
}

fn issue_line(task: &str, phase: &str, reason: &str) -> String {
    format!("host_dispatch_evidence_issue: task={task} phase={phase} reason={reason}")
}

/// A fully checked plan with a bound goal receipt, so only dispatch evidence
/// can keep it from `goal-verified`. Returns the prompt path and plan scope.
fn bound_goal_fixture(fixture: &TempDir, name: &str, extra: &str) -> (PathBuf, PathBuf) {
    let head = initial_commit(fixture.path());
    let (_, prompt_path) = write_prompt(
        fixture.path(),
        name,
        &prompt(&checked(&task(1), &head), "—", extra),
    );
    let scope = fixture.path().join(".dev/pipeline").join(name);
    std::fs::create_dir_all(&scope).unwrap();
    let canonical_prompt = std::fs::canonicalize(&prompt_path).unwrap();
    std::fs::write(
        scope.join("goal-verification.receipt.md"),
        goal(
            &canonical_prompt,
            "",
            &head,
            &task(1),
            &sha256(&prompt_path),
        ),
    )
    .unwrap();
    (prompt_path, scope)
}

#[test]
fn bound_goal_with_routed_history_survives_live_route_changes() {
    let fixture = repo();
    let task = task(1);
    let (prompt_path, scope) = bound_goal_fixture(&fixture, "routed-goal", "");
    let home = fixture.path().join("test-home");
    let config_dir = home.join(".gal/config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        scope.join("preflight.receipt.md"),
        "host: codex\ndispatch_mode: routed\nrun_id: fixture-run\n",
    )
    .unwrap();
    let task_dir = scope.join(&task);
    write_attempt(&task_dir, &task, 1, "codex", "", "completed");
    write_attempt(&task_dir, &task, 2, "codex", "", "no-writeback");
    let receipt = fixture.path().join("routed-goal-result.md");

    for route_config in [
        Some(
            r#"{"executorRouting":{"executors":{"codex":"fixture-model"},"pipeline":{"CODER":{"executor":"codex"}}}}"#,
        ),
        Some(
            r#"{"executorRouting":{"executors":{"codex":"fixture-model"},"pipeline":{"CODER":{"executor":"different-model"}}}}"#,
        ),
        None,
        Some("not-json"),
    ] {
        match route_config {
            Some(config) => std::fs::write(config_dir.join("config.json"), config).unwrap(),
            None => {
                let _ = std::fs::remove_file(config_dir.join("config.json"));
            }
        }
        let (code, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
        assert_eq!(code, 0, "bound routed goal must be verified: {output}");
        assert!(output.contains("decision: goal-verified"), "{output}");
        assert!(!output.contains("host_dispatch_evidence"), "{output}");
    }

    // Without an earlier completed witness the same routed history must fail.
    std::fs::remove_file(task_dir.join(format!("000001-000001-000001-{task}-implement.log")))
        .unwrap();
    let (code, output) = run_with_home(fixture.path(), &prompt_path, &receipt, &[], &home);
    assert_eq!(code, 1, "{output}");
    assert!(!output.contains("decision: goal-verified"), "{output}");
    assert!(output.contains(&format!(
        "host_dispatch_evidence_issue: task={task} phase=implement reason=no-writeback-without-witness"
    )));
}

#[test]
fn bound_goal_with_matching_host_pin_is_verified() {
    for host in ["codex", "claude"] {
        let fixture = repo();
        let task = task(1);
        let (prompt_path, scope) = bound_goal_fixture(
            &fixture,
            "host-goal",
            "Verification Independence: DEGRADED_HOST_SUBAGENT",
        );
        std::fs::write(
            scope.join("preflight.receipt.md"),
            format!("host: {host}\ndispatch_mode: host-subagent\nrun_id: fixture-run\n"),
        )
        .unwrap();
        let task_dir = scope.join(&task);
        write_attempt(&task_dir, &task, 1, "host-subagent", host, "completed");
        let receipt = fixture.path().join("host-goal-result.md");
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, 0, "matching {host} pin must verify: {output}");
        assert!(output.contains("decision: goal-verified"), "{output}");
        assert!(!output.contains("host_dispatch_evidence"), "{output}");

        // A different host than the pin must fail with executor-mismatch.
        let other = if host == "codex" { "claude" } else { "codex" };
        write_attempt(&task_dir, &task, 1, "host-subagent", other, "completed");
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, 1, "{output}");
        assert!(!output.contains("decision: goal-verified"), "{output}");
        assert!(output.contains(&format!(
            "host_dispatch_evidence_issue: task={task} phase=implement reason=executor-mismatch"
        )));
    }
}

#[test]
fn malformed_pin_never_authorizes_host_evidence() {
    let fixture = repo();
    let task = task(1);
    let (prompt_path, scope) = bound_goal_fixture(
        &fixture,
        "malformed-pin",
        "Verification Independence: DEGRADED_HOST_SUBAGENT",
    );
    write_pin(&scope, "codex", "host-subagent extra");
    write_attempt(
        &scope.join(&task),
        &task,
        1,
        "host-subagent",
        "codex",
        "completed",
    );
    let receipt = fixture.path().join("malformed-pin-result.md");
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "malformed pin must fail closed: {output}");
    assert!(!output.contains("decision: goal-verified"), "{output}");
}

#[test]
fn unreadable_task_directory_is_attributed_to_its_task() {
    let fixture = repo();
    let task = task(1);
    let (prompt_path, scope) = bound_goal_fixture(&fixture, "unreadable-task", "");
    std::fs::write(
        scope.join("preflight.receipt.md"),
        "host: codex\ndispatch_mode: routed\nrun_id: fixture-run\n",
    )
    .unwrap();
    let task_dir = scope.join(&task);
    std::fs::create_dir_all(&task_dir).unwrap();
    // A regular file where the host attempt directory belongs cannot be read as one.
    std::fs::write(task_dir.join("host-attempts"), "not a directory").unwrap();
    let receipt = fixture.path().join("unreadable-task-result.md");
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "{output}");
    assert!(!output.contains("decision: goal-verified"), "{output}");
    assert!(output.contains(&format!(
        "host_dispatch_evidence_issue: task={task} phase=unknown reason=unreadable-log-directory"
    )));
    assert!(!output.contains("task=unknown"), "{output}");
}

#[test]
fn missing_independence_marker_is_attributed_to_the_host_attempt() {
    let fixture = repo();
    let task = task(1);
    let (prompt_path, scope) = bound_goal_fixture(&fixture, "host-marker", "");
    std::fs::write(
        scope.join("preflight.receipt.md"),
        "host: codex\ndispatch_mode: host-subagent\nrun_id: fixture-run\n",
    )
    .unwrap();
    write_attempt(
        &scope.join(&task),
        &task,
        1,
        "host-subagent",
        "codex",
        "completed",
    );
    let receipt = fixture.path().join("host-marker-result.md");
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "{output}");
    assert!(!output.contains("decision: goal-verified"), "{output}");
    assert!(output.contains(&format!(
        "host_dispatch_evidence_issue: task={task} phase=implement reason=mode-mismatch"
    )));
    assert!(!output.contains("task=unknown"), "{output}");
}

fn drop_executor_line(task_dir: &Path, ordinal: u32) {
    let prefix = format!("{ordinal:06}-");
    let path = std::fs::read_dir(task_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix))
        })
        .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let kept: String = text
        .lines()
        .filter(|line| !line.starts_with("executor:"))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_ne!(text, kept, "fixture must have had an executor line");
    std::fs::write(&path, kept).unwrap();
}

#[test]
fn routed_attempt_without_executor_header_is_a_malformed_log() {
    for pinned in [true, false] {
        let fixture = repo();
        let task = task(1);
        let (prompt_path, scope) = bound_goal_fixture(&fixture, "routed-no-executor", "");
        if pinned {
            std::fs::write(
                scope.join("preflight.receipt.md"),
                "host: codex\ndispatch_mode: routed\nrun_id: fixture-run\n",
            )
            .unwrap();
        }
        let task_dir = scope.join(&task);
        write_attempt(&task_dir, &task, 1, "codex", "", "completed");
        drop_executor_line(&task_dir, 1);
        let receipt = fixture.path().join("routed-no-executor-result.md");
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, 1, "pinned={pinned}: {output}");
        assert!(!output.contains("decision: goal-verified"), "{output}");
        assert!(output.contains("host_dispatch_evidence: fail"), "{output}");
        assert!(
            output.contains(&format!(
                "host_dispatch_evidence_issue: task={task} phase=implement reason=malformed-log"
            )),
            "pinned={pinned}: {output}"
        );
    }
}

#[test]
fn malformed_log_phase_comes_from_every_dispatch_phase_name() {
    for (phase, suffix) in [
        ("finalize-review", "-codex"),
        ("finalize-review", ""),
        ("investigate", "-codex"),
        ("investigate", ""),
        ("implement", ""),
    ] {
        let fixture = repo();
        let task = task(1);
        let (prompt_path, scope) = bound_goal_fixture(&fixture, "malformed-phase", "");
        let task_dir = scope.join(&task);
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(
            task_dir.join(format!("000001-000001-000001-{task}-{phase}{suffix}.log")),
            "not a real log at all",
        )
        .unwrap();
        let receipt = fixture.path().join("malformed-phase-result.md");
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, 1, "{phase}{suffix}: {output}");
        assert!(
            output.contains(&format!(
                "host_dispatch_evidence_issue: task={task} phase={phase} reason=malformed-log"
            )),
            "{phase}{suffix}: {output}"
        );
    }
}

#[test]
fn routed_attempt_without_executor_header_is_not_a_no_writeback_witness() {
    for pinned in [true, false] {
        let fixture = repo();
        let task = task(1);
        let (prompt_path, scope) = bound_goal_fixture(&fixture, "routed-no-witness", "");
        if pinned {
            std::fs::write(
                scope.join("preflight.receipt.md"),
                "host: codex\ndispatch_mode: routed\nrun_id: fixture-run\n",
            )
            .unwrap();
        }
        let task_dir = scope.join(&task);
        write_attempt(&task_dir, &task, 1, "codex", "", "completed");
        write_attempt(&task_dir, &task, 2, "codex", "", "no-writeback");
        drop_executor_line(&task_dir, 1);
        let receipt = fixture.path().join("routed-no-witness-result.md");
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, 1, "pinned={pinned}: {output}");
        assert!(!output.contains("decision: goal-verified"), "{output}");
        assert!(output.contains("host_dispatch_evidence: fail"), "{output}");
    }
}

#[test]
fn host_attempt_is_not_a_routed_no_writeback_witness() {
    let task = task(1);

    // Case A: a routed pin that names a host admits a host attempt as evidence,
    // but that attempt must not witness a later routed no-writeback.
    let fixture = repo();
    let (prompt_path, scope) = bound_goal_fixture(&fixture, "host-witness-routed", HOST_MARKER);
    write_pin(&scope, "codex", "routed");
    let task_dir = scope.join(&task);
    write_attempt(&task_dir, &task, 1, "host-subagent", "codex", "completed");
    write_attempt(&task_dir, &task, 2, "codex", "", "no-writeback");
    let receipt = fixture.path().join("host-witness-routed-result.md");
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "{output}");
    assert!(!output.contains("decision: goal-verified"), "{output}");
    assert!(output.contains("host_dispatch_evidence: fail"), "{output}");
    assert_eq!(
        issue_lines(&output),
        vec![issue_line(
            &task,
            "implement",
            "no-writeback-without-witness"
        )],
        "{output}"
    );

    // Case B: an earlier completed routed attempt still witnesses.
    write_attempt(&task_dir, &task, 1, "codex", "", "completed");
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_goal_verified(&output, code, "routed witness under a routed pin");

    // Case C: under a host-subagent pin, a routed finalize-review no-writeback
    // whose only earlier completed attempt is a host attempt has no witness.
    let review = FINALIZE_REVIEW_SCOPE;
    let fixture = repo();
    let (prompt_path, scope) = bound_goal_fixture(&fixture, "host-witness-review", HOST_MARKER);
    write_pin(&scope, "codex", "host-subagent");
    write_attempt(
        &scope.join(&task),
        &task,
        1,
        "host-subagent",
        "codex",
        "completed",
    );
    let review_dir = scope.join(review);
    write_phase_attempt(
        &review_dir,
        review,
        review,
        1,
        "host-subagent",
        "codex",
        "completed",
    );
    write_phase_attempt(&review_dir, review, review, 2, "codex", "", "no-writeback");
    let receipt = fixture.path().join("host-witness-review-result.md");
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "{output}");
    assert!(!output.contains("decision: goal-verified"), "{output}");
    assert_eq!(
        issue_lines(&output),
        vec![issue_line(review, review, "no-writeback-without-witness")],
        "{output}"
    );
}

fn assert_goal_verified(output: &str, code: i32, context: &str) {
    assert_eq!(code, 0, "{context}: {output}");
    assert!(
        output.contains("decision: goal-verified"),
        "{context}: {output}"
    );
    assert!(
        !output.contains("host_dispatch_evidence"),
        "{context}: {output}"
    );
}

#[test]
fn finalize_review_attempt_is_valid_under_any_pin() {
    let task = task(1);
    let review = FINALIZE_REVIEW_SCOPE;
    // Dispatch keeps a present auditor route for finalize-review even under a
    // host-subagent pin, so a routed review attempt is valid under every pin.
    for (mode, review_executor, review_host) in [
        ("host-subagent", "codex", ""),
        ("host-subagent", "host-subagent", "codex"),
        ("routed", "codex", ""),
    ] {
        let fixture = repo();
        let (prompt_path, scope) = bound_goal_fixture(&fixture, "finalize-review-pin", HOST_MARKER);
        write_pin(&scope, "codex", mode);
        let (executor, host) = if mode == "host-subagent" {
            ("host-subagent", "codex")
        } else {
            ("codex", "")
        };
        write_attempt(&scope.join(&task), &task, 1, executor, host, "completed");
        write_phase_attempt(
            &scope.join(review),
            review,
            review,
            1,
            review_executor,
            review_host,
            "completed",
        );
        let receipt = fixture.path().join("finalize-review-pin-result.md");
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_goal_verified(
            &output,
            code,
            &format!("{mode} pin with {review_executor} review"),
        );
    }

    // A routed review attempt that ended in no-writeback needs an earlier valid
    // completed routed witness, also under a host-subagent pin.
    let fixture = repo();
    let (prompt_path, scope) = bound_goal_fixture(&fixture, "finalize-review-witness", HOST_MARKER);
    write_pin(&scope, "codex", "host-subagent");
    write_attempt(
        &scope.join(&task),
        &task,
        1,
        "host-subagent",
        "codex",
        "completed",
    );
    let review_dir = scope.join(review);
    write_phase_attempt(&review_dir, review, review, 1, "codex", "", "completed");
    write_phase_attempt(&review_dir, review, review, 2, "codex", "", "no-writeback");
    let receipt = fixture.path().join("finalize-review-witness-result.md");
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_goal_verified(&output, code, "routed review witness under a host pin");
    std::fs::remove_file(review_dir.join(format!("000001-000001-000001-{review}-{review}.log")))
        .unwrap();
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "{output}");
    assert_eq!(
        issue_lines(&output),
        vec![issue_line(review, review, "no-writeback-without-witness")],
        "{output}"
    );

    // A review attempt that contradicts both shapes is still rejected.
    let fixture = repo();
    let (prompt_path, scope) = bound_goal_fixture(&fixture, "finalize-review-reject", HOST_MARKER);
    write_pin(&scope, "codex", "host-subagent");
    write_attempt(
        &scope.join(&task),
        &task,
        1,
        "host-subagent",
        "codex",
        "completed",
    );
    write_phase_attempt(
        &scope.join(review),
        review,
        review,
        1,
        "codex",
        "codex",
        "completed",
    );
    let receipt = fixture.path().join("finalize-review-reject-result.md");
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "{output}");
    assert!(!output.contains("decision: goal-verified"), "{output}");
    assert!(
        issue_lines(&output).contains(&issue_line(review, review, "executor-mismatch").as_str()),
        "{output}"
    );
}

#[test]
fn routed_pin_with_host_accepts_only_a_matching_independent_host_attempt() {
    let task = task(1);
    // (pin host, attempt host, marker present, terminal state, expected reason)
    let cases = [
        ("codex", "codex", true, "completed", None),
        ("claude", "claude", true, "completed", None),
        ("codex", "claude", true, "completed", Some("mode-mismatch")),
        ("codex", "codex", false, "completed", Some("mode-mismatch")),
        ("none", "codex", true, "completed", Some("mode-mismatch")),
        ("codex", "codex", true, "started", Some("not-completed")),
    ];
    for (pin_host, attempt_host, marker, terminal, expected) in cases {
        let fixture = repo();
        let (prompt_path, scope) = bound_goal_fixture(
            &fixture,
            "routed-pin-host-attempt",
            if marker { HOST_MARKER } else { "" },
        );
        write_pin(&scope, pin_host, "routed");
        write_attempt(
            &scope.join(&task),
            &task,
            1,
            "host-subagent",
            attempt_host,
            terminal,
        );
        let receipt = fixture.path().join("routed-pin-host-attempt-result.md");
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        let context = format!("pin={pin_host} host={attempt_host} marker={marker} {terminal}");
        match expected {
            None => assert_goal_verified(&output, code, &context),
            Some(reason) => {
                assert_eq!(code, 1, "{context}: {output}");
                assert!(
                    !output.contains("decision: goal-verified"),
                    "{context}: {output}"
                );
                assert_eq!(
                    issue_lines(&output),
                    vec![issue_line(&task, "implement", reason)],
                    "{context}: {output}"
                );
            }
        }
    }
}

#[test]
fn issue_lines_keep_full_task_phase_reason_order_across_tasks() {
    let fixture = repo();
    let (t1, t2) = (task(1), task(2));
    let (prompt_path, scope) = bound_goal_fixture(&fixture, "issue-order", "");
    write_pin(&scope, "codex", "routed");
    let first = scope.join(&t1);
    // A routed executor carrying a host header contradicts the routed pin and
    // is not a completed host attempt.
    write_attempt(&first, &t1, 1, "codex", "codex", "started");
    // A lone no-writeback result has no witness, and a second file is malformed.
    write_phase_attempt(&first, &t1, "test", 1, "codex", "", "no-writeback");
    std::fs::write(
        first.join(format!("000009-000001-000001-{t1}-test.log")),
        "not a dispatch log",
    )
    .unwrap();
    let second = scope.join(&t2);
    // A retained host attempt needs the independence marker, and the effective
    // attempt has no executor header.
    write_attempt(&second, &t2, 1, "host-subagent", "codex", "completed");
    write_attempt(&second, &t2, 2, "codex", "", "completed");
    drop_executor_line(&second, 2);
    write_phase_attempt(&second, &t2, "audit", 3, "codex", "", "started");

    let receipt = fixture.path().join("issue-order-result.md");
    let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
    assert_eq!(code, 1, "{output}");
    assert!(output.contains("host_dispatch_evidence: fail"), "{output}");
    assert_eq!(
        issue_lines(&output),
        vec![
            issue_line(&t1, "implement", "mode-mismatch"),
            issue_line(&t1, "implement", "not-completed"),
            issue_line(&t1, "test", "malformed-log"),
            issue_line(&t1, "test", "no-writeback-without-witness"),
            issue_line(&t2, "audit", "not-completed"),
            issue_line(&t2, "implement", "malformed-log"),
            issue_line(&t2, "implement", "mode-mismatch"),
        ],
        "{output}"
    );
}

#[test]
fn attempt_without_executor_header_reports_only_a_malformed_log() {
    let task = task(1);
    for mode in ["routed", "host-subagent"] {
        let fixture = repo();
        let (prompt_path, scope) = bound_goal_fixture(&fixture, "no-executor-single", HOST_MARKER);
        write_pin(&scope, "codex", mode);
        let task_dir = scope.join(&task);
        write_attempt(&task_dir, &task, 1, "codex", "", "completed");
        drop_executor_line(&task_dir, 1);
        let receipt = fixture.path().join("no-executor-single-result.md");
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, 1, "{mode}: {output}");
        assert_eq!(
            issue_lines(&output),
            vec![issue_line(&task, "implement", "malformed-log")],
            "{mode}: {output}"
        );
    }
}

#[test]
fn unrecognized_phase_name_is_attributed_when_the_scan_read_it() {
    let task = task(1);
    // A phase name the scan read is attributed. A byte that cannot stand in one
    // issue token is percent-encoded instead of falling back to `unknown`.
    for (phase, attributed) in [("scaffold", "scaffold"), ("two words", "two%20words")] {
        let fixture = repo();
        let (prompt_path, scope) = bound_goal_fixture(&fixture, "unrecognized-phase", "");
        write_pin(&scope, "codex", "routed");
        write_phase_attempt(
            &scope.join(&task),
            &task,
            phase,
            1,
            "codex",
            "",
            "completed",
        );
        let receipt = fixture.path().join("unrecognized-phase-result.md");
        let (code, output) = run(fixture.path(), &prompt_path, &receipt, &[]);
        assert_eq!(code, 1, "{phase}: {output}");
        assert_eq!(
            issue_lines(&output),
            vec![issue_line(&task, attributed, "mode-mismatch")],
            "{phase}: {output}"
        );
    }
}

/// Splits a markdown span into sentences so a probe can judge each claim.
fn doc_sentences(text: &str) -> Vec<String> {
    text.replace('\n', " ")
        .split(". ")
        .map(|sentence| sentence.trim().to_string())
        .collect()
}

#[test]
fn workflow_doc_publishes_the_handback_evidence_contract() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/workflows.md");
    let doc = std::fs::read_to_string(&path).expect("docs/workflows.md");
    for literal in [
        "executor-mismatch",
        "mode-mismatch",
        "not-completed",
        "no-writeback-without-witness",
        "malformed-log",
        "unreadable-log-directory",
        "host_dispatch_evidence_issue: task=",
        "unknown",
        "Test Retry Count",
        "Review Retry Count",
        "`## Status`",
    ] {
        assert!(doc.contains(literal), "missing literal: {literal}");
    }
    let evidence = doc
        .find("#### Handback Dispatch Evidence")
        .expect("evidence heading");
    let retry = doc
        .find("#### Retry Ceiling Evidence")
        .expect("retry heading");
    let next = doc
        .find("### Task Execution Cycle and Roles")
        .expect("next section heading");
    for opening in [
        "Retry rounds require",
        "The pipeline halts when",
        "To recover from a legacy interruption",
    ] {
        let at = doc.find(opening).expect(opening);
        assert!(
            at < evidence,
            "{opening} belongs to the pipeline section, before the new subsections"
        );
    }
    assert!(evidence < retry && retry < next, "subsection order");
    assert!(
        doc[evidence..next].contains("finalize-review"),
        "the routed finalize-review rule is documented"
    );
    assert!(
        doc[evidence..next].contains("pinned host"),
        "the host attempt under a routed pin rule is documented"
    );
    assert!(
        doc[evidence..next].contains("never serves as a routed witness"),
        "the routed witness rule is documented"
    );
    for sentence in doc_sentences(&doc[evidence..next]) {
        if sentence.contains("Security Retry Count") {
            assert!(
                sentence.contains(" not "),
                "Security Retry Count is not a supported source: {sentence}"
            );
        }
        let lower = sentence.to_ascii_lowercase();
        if lower.contains("live") {
            assert!(
                ["without", " not ", "never"]
                    .iter()
                    .any(|negation| lower.contains(negation)),
                "no claim that evidence is resolved against the live route: {sentence}"
            );
        }
    }
}
