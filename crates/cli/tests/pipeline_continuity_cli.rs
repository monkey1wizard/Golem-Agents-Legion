use pipeline::coordinator::{
    AttemptOwnership, CheckpointKind, CheckpointReceipt, ContinuationProfile, CoordinatorState,
    ExecutionBinding, NextAction, OwnerObservation, SemanticCheckpoint,
};
use pipeline::projection_journal::{Boundary, JournalError, Projection};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Barrier},
    thread,
};
use tempfile::TempDir;

fn task(number: u32) -> String {
    format!("T-{number:02}")
}

fn run(root: &Path, prompt: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(root)
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn guarded_status_is_read_only_and_continuations_require_unambiguous_arguments() {
    let repo = TempDir::new().unwrap();
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("continuity.prompt.md");
    let first = task(1);
    fs::write(&prompt, format!("## Tasks\n- [ ] {first}\n")).unwrap();
    let root = repo.path().join(".dev/pipeline/continuity");
    fs::create_dir_all(&root).unwrap();
    let mut state = CoordinatorState::new_legacy("continuity", "hash", &first, "implement");
    state.profile = ContinuationProfile::CodexStopV1;
    let coordinator = root.join("coordinator.json");
    fs::write(&coordinator, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
    fs::write(root.join("marker"), b"unchanged").unwrap();
    let before: Vec<_> = walkdir(&root)
        .into_iter()
        .map(|(name, bytes)| (name, hash(&bytes)))
        .collect();

    for (action, expected) in [
        ("--status", "profile=CodexStopV1"),
        ("--submit-checkpoint-receipt", "checkpoint receipt missing"),
    ] {
        let output = run(repo.path(), &prompt, &[action]);
        if action == "--status" {
            assert_eq!(output.status.code(), Some(0));
            let status = String::from_utf8_lossy(&output.stdout);
            assert!(status.contains(expected));
            assert!(status.len() <= 512);
            println!("status-length-bytes={}", status.trim_end().len());
        } else {
            assert_ne!(output.status.code(), Some(64));
            assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
        }
    }
    let after: Vec<_> = walkdir(&root)
        .into_iter()
        .map(|(name, bytes)| (name, hash(&bytes)))
        .collect();
    println!("status-file-hashes-before={before:?}");
    println!("status-file-hashes-after={after:?}");
    assert_eq!(before, after, "status must not mutate plan-root files");

    for extra in [
        vec!["--bogus"],
        vec!["--status", "--resume"],
        vec!["--resume"],
        vec!["--resume", "--task", first.as_str()],
    ] {
        let output = run(repo.path(), &prompt, &extra);
        assert_eq!(output.status.code(), Some(64), "args: {extra:?}");
    }
    let output = run(
        repo.path(),
        &prompt,
        &["--resume", "--task", first.as_str(), "--phase", "test"],
    );
    assert_ne!(
        output.status.code(),
        Some(0),
        "resume must fail closed when a fresh entry latch cannot pass"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("resume entry latch did not pass"));
}

fn bind_fixture(state: &mut CoordinatorState, root: &Path) -> String {
    let executable = fs::canonicalize(env!("CARGO_BIN_EXE_gal")).unwrap();
    // Same path form as the guarded entry writer: no Windows verbatim prefix.
    let binding_path_text = |path: &Path| {
        let text = path.to_string_lossy();
        text.strip_prefix(r"\\?\")
            .unwrap_or(&text)
            .replace('\\', "/")
    };
    let binding = ExecutionBinding {
        version: ExecutionBinding::VERSION,
        binding_scope: format!("coordinator:{}", state.plan_scope),
        worktree_root: binding_path_text(&fs::canonicalize(root).unwrap()),
        executable_path: binding_path_text(&executable),
        executable_sha256: hash(&fs::read(executable).unwrap()),
    };
    let digest = binding.digest().unwrap();
    state.guarded_entry(binding).unwrap();
    digest
}

#[test]
fn safe_rebind_barriers_and_worktree_bindings_preserve_prior_evidence() {
    let repo = TempDir::new().unwrap();
    let source_a = repo.path().join("source-a");
    let source_b = repo.path().join("source-b");
    let downstream = repo.path().join("downstream");
    for path in [&source_a, &source_b, &downstream] {
        fs::create_dir_all(path).unwrap();
    }
    let old_executable = fs::canonicalize(env!("CARGO_BIN_EXE_gal")).unwrap();
    let make_binding = |worktree: &Path, marker: &str| ExecutionBinding {
        version: ExecutionBinding::VERSION,
        binding_scope: format!("coordinator:{marker}"),
        worktree_root: fs::canonicalize(worktree)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/"),
        executable_path: old_executable.to_string_lossy().replace('\\', "/"),
        executable_sha256: hash(&fs::read(&old_executable).unwrap()),
    };

    let downstream_files = [
        ("executable", b"paused-downstream-executable".to_vec()),
        ("coordinator", b"paused-downstream-coordinator".to_vec()),
        ("checkpoint", b"paused-downstream-checkpoint".to_vec()),
        ("receipt", b"paused-downstream-receipt".to_vec()),
        ("shared-sentinel", b"shared-root-sentinel".to_vec()),
    ];
    for (name, bytes) in &downstream_files {
        fs::write(downstream.join(name), bytes).unwrap();
    }
    let downstream_before: Vec<_> = downstream_files
        .iter()
        .map(|(name, _)| fs::read(downstream.join(name)).unwrap())
        .collect();

    let rejected_states = [
        "active-attempt",
        "unconsumed-checkpoint",
        "unrecovered-projection-journal",
    ];
    for barrier in rejected_states {
        let mut state = CoordinatorState::new_legacy("continuity", "prompt", "task", "implement");
        state.profile = ContinuationProfile::CodexStopV1;
        state
            .guarded_entry(make_binding(&source_a, "source-a"))
            .unwrap();
        match barrier {
            "active-attempt" => {
                state.claim_attempt("attempt-a".into()).unwrap();
            }
            "unconsumed-checkpoint" => {
                state
                    .checkpoint("checkpoint-a".into(), CheckpointKind::TaskQuality, None)
                    .unwrap();
            }
            _ => state.pending_projection_id = Some("projection-a".into()),
        }
        let bytes_before = serde_json::to_vec(&state).unwrap();
        assert!(state
            .guarded_entry(make_binding(&source_b, "source-b"))
            .is_err());
        assert_eq!(
            serde_json::to_vec(&state).unwrap(),
            bytes_before,
            "rejected {barrier} rebind mutated state"
        );
        println!("rebind-barrier={barrier}; result=rejected; generation=unchanged");
    }

    let mut state = CoordinatorState::new_legacy("continuity", "prompt", "task", "implement");
    state.profile = ContinuationProfile::CodexStopV1;
    let old_digest = state
        .guarded_entry(make_binding(&source_a, "source-a"))
        .unwrap();
    let old_revision = state.revision;
    let old_generation = serde_json::to_vec(&state).unwrap();
    let old_generation_path = repo.path().join("generation-1.json");
    fs::write(&old_generation_path, &old_generation).unwrap();
    let new_digest = state
        .guarded_entry(make_binding(&source_b, "source-b"))
        .unwrap();
    assert_ne!(old_digest, new_digest);
    assert_eq!(state.revision, old_revision + 1);
    assert_eq!(fs::read(&old_generation_path).unwrap(), old_generation);
    assert_eq!(
        state.execution_binding.as_ref().unwrap().worktree_root,
        make_binding(&source_b, "source-b").worktree_root
    );

    let barrier = Arc::new(Barrier::new(3));
    let start = |binding: ExecutionBinding, marker: &'static str, barrier: Arc<Barrier>| {
        thread::spawn(move || {
            let mut state =
                CoordinatorState::new_legacy("continuity", "prompt", "task", "implement");
            state.profile = ContinuationProfile::CodexStopV1;
            println!("barrier-timeline={marker}:ready");
            barrier.wait();
            println!("barrier-timeline={marker}:entry-start");
            let generation = state.guarded_entry(binding).unwrap();
            println!(
                "barrier-timeline={marker}:entry-bound;revision={}",
                state.revision
            );
            (generation, state)
        })
    };
    let a = start(
        make_binding(&source_a, "source-a"),
        "source-a",
        barrier.clone(),
    );
    let b = start(
        make_binding(&source_b, "source-b"),
        "source-b",
        barrier.clone(),
    );
    println!("barrier-timeline=orchestrator:release");
    barrier.wait();
    let (generation_a, concurrent_a) = a.join().unwrap();
    let (generation_b, concurrent_b) = b.join().unwrap();
    assert_ne!(generation_a, generation_b);
    assert_ne!(
        concurrent_a.execution_binding,
        concurrent_b.execution_binding
    );
    let downstream_after: Vec<_> = downstream_files
        .iter()
        .map(|(name, _)| fs::read(downstream.join(name)).unwrap())
        .collect();
    assert_eq!(
        downstream_before, downstream_after,
        "source worktree binds changed paused downstream/shared bytes"
    );
    println!(
        "rebind-generation={old_revision}->{}; old-generation-bytes=unchanged",
        state.revision
    );
    println!("source-worktrees=2; paused-downstream=unchanged; executable/coordinator/checkpoint/receipt/shared-sentinel=byte-identical");
}

#[test]
fn handback_emitted_action_strings_round_trip_through_parser() {
    let repo = TempDir::new().unwrap();
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("continuity.prompt.md");
    let first = task(1);
    fs::write(&prompt, format!("## Tasks\n- [ ] {first}\n")).unwrap();
    let root = repo.path().join(".dev/pipeline/continuity");
    fs::create_dir_all(&root).unwrap();
    let mut state = CoordinatorState::new_legacy("continuity", "hash", &first, "implement");
    state.profile = ContinuationProfile::CodexStopV1;
    let coordinator = root.join("coordinator.json");
    fs::write(&coordinator, serde_json::to_vec_pretty(&state).unwrap()).unwrap();

    // In the guarded profile, pipeline-handback-check writes the JSON form of the coordinator's next action.
    let start_action =
        format!(r#"{{"kind":"start_task","binding":{{"task_id":"{first}","phase":"implement"}}}}"#);
    let resume_action =
        format!(r#"{{"kind":"resume_task","binding":{{"task_id":"{first}","phase":"test"}}}}"#);
    let retry_action = format!(
        r#"{{"kind":"retry_task","binding":{{"task_id":"{first}","phase":"fix","attempt":2}}}}"#
    );
    let await_action = r#"{"kind":"await_orchestrator","binding":{"checkpoint_id":"chk-01"}}"#;
    let complete_action = r#"{"kind":"complete","binding":{}}"#;
    let blocked_action = r#"{"kind":"blocked","binding":{"reason":"held"}}"#;
    let recover_action = r#"{"kind":"recover_attempt","binding":{"attempt_id":"a-1"}}"#;
    let action_cases: Vec<(&str, Vec<&str>)> = vec![
        ("start_task => resume", vec![start_action.as_str()]),
        ("resume_task => resume", vec![resume_action.as_str()]),
        ("retry_task => resume", vec![retry_action.as_str()]),
        ("await_orchestrator => submit receipt", vec![await_action]),
        ("complete => status", vec![complete_action]),
        ("blocked => status", vec![blocked_action]),
    ];

    for (mapping, action_args) in action_cases {
        let output = run(repo.path(), &prompt, &action_args);
        println!(
            "parsed-action={mapping}; exit-code={:?}",
            output.status.code()
        );
        assert_ne!(
            output.status.code(),
            Some(64),
            "action string '{action_args:?}' must parse into a continuation action instead of exiting with usage error 64: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let output = run(repo.path(), &prompt, &[recover_action]);
    println!(
        "parsed-action=recover_attempt => rejected; exit-code={:?}",
        output.status.code()
    );
    assert_eq!(output.status.code(), Some(64));
    let missing_phase = format!(r#"{{"kind":"start_task","binding":{{"task_id":"{first}"}}}}"#);
    for invalid in [
        r#"{"kind":"unknown","binding":{}}"#,
        missing_phase.as_str(),
        r#"{"kind":"resume_task","binding":{"phase":"test"}}"#,
    ] {
        let output = run(repo.path(), &prompt, &[invalid]);
        println!(
            "parsed-action=invalid => rejected; exit-code={:?}",
            output.status.code()
        );
        assert_eq!(output.status.code(), Some(64), "action: {invalid}");
    }
    let output = run(repo.path(), &prompt, &[resume_action.as_str(), "--status"]);
    println!(
        "parsed-action=ambiguous => rejected; exit-code={:?}",
        output.status.code()
    );
    assert_eq!(
        output.status.code(),
        Some(64),
        "mixed actions must be rejected"
    );
}

fn init_git_repo(path: &Path) {
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "test"],
        vec!["config", "commit.gpgsign", "false"],
    ] {
        assert!(Command::new("git")
            .current_dir(path)
            .args(args)
            .status()
            .unwrap()
            .success());
    }
}

#[test]
fn checkpoint_receipt_submission_and_resume_through_entry_latch() {
    let repo = TempDir::new().unwrap();
    init_git_repo(repo.path());

    // Task-quality convention required for receipt validation
    let conv_dir = repo.path().join("plugins/gal-core/conventions");
    fs::create_dir_all(&conv_dir).unwrap();
    fs::write(
        conv_dir.join("task-quality.md"),
        "# Task Quality\n\nCanonical questions for checking whether a task is sufficiently specified\n",
    )
    .unwrap();

    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("continuity.prompt.md");
    let task_id = task(1);
    let probe_id = task_id.replacen('T', "TP", 1);
    let prompt_content = format!(
        "# Plan Prompt: Continuity\n\n\
        ## Files to Create or Modify\n\n\
        - `crates/pipeline/src/task_spec.rs`\n\n\
        ## Tasks\n\n\
        - [ ] {task_id} — Task goal description.\n  \
          - Targets: `crates/pipeline/src/task_spec.rs`\n  \
          - Change: Implement checkpoint receipt submission.\n  \
          - Acceptance: Verifies all checkpoints.\n\n\
        ## Test Plan\n\n\
        | ID | Type | Description | Covers |\n\
        | --- | --- | --- | --- |\n\
        | {probe_id} | integration | Continuity probe | {task_id} |\n\n\
        ## Status\n\n\
        Workflow: IMPLEMENT\n"
    );
    fs::write(&prompt, &prompt_content).unwrap();

    let state_md = repo.path().join(".dev/state.md");
    fs::write(
        &state_md,
        "# State\n\n| continuity | .dev/plans/continuity.prompt.md |\n",
    )
    .unwrap();

    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "init"])
        .status()
        .unwrap()
        .success());

    let head_out = Command::new("git")
        .current_dir(repo.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(head_out.status.success());
    let head = String::from_utf8_lossy(&head_out.stdout).trim().to_string();
    let prompt_hash = hash(&fs::read(&prompt).unwrap());

    let root = repo.path().join(".dev/pipeline/continuity");
    fs::create_dir_all(&root).unwrap();

    let mut state =
        CoordinatorState::new_legacy("continuity", prompt_hash.clone(), &task_id, "implement");
    state.profile = ContinuationProfile::CodexStopV1;
    let binding_digest = bind_fixture(&mut state, repo.path());
    state.revision = 1;
    state.checkpoint = Some(SemanticCheckpoint {
        checkpoint_id: "chk-01".into(),
        revision: 1,
        task_id: task_id.clone(),
        phase: "implement".into(),
        prompt_sha256: prompt_hash.clone(),
        execution_binding_sha256: binding_digest.clone(),
        commit: Some(head.clone()),
        kind: CheckpointKind::TaskQuality,
        consumed_receipt_id: None,
    });
    state.next_action = NextAction::AwaitOrchestrator {
        checkpoint_id: "chk-01".into(),
    };
    let coordinator_path = root.join("coordinator.json");
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let authority_files = [coordinator_path.clone(), prompt.clone(), state_md.clone()];
    let get_authority_hashes = || -> Vec<(PathBuf, String)> {
        authority_files
            .iter()
            .map(|p| (p.clone(), hash(&fs::read(p).unwrap())))
            .collect()
    };
    let base_hashes = get_authority_hashes();

    let receipt_path = root.join("orchestrator-receipt.json");

    // 1. Malformed receipt rejection -> authority hashes unchanged
    fs::write(&receipt_path, b"not json at all {[[[").unwrap();
    let out = run(repo.path(), &prompt, &["--submit-checkpoint-receipt"]);
    assert_ne!(out.status.code(), Some(0));
    assert_eq!(
        get_authority_hashes(),
        base_hashes,
        "malformed receipt must not alter authority hashes"
    );
    fs::remove_file(&receipt_path).unwrap();

    // 2. Wrong-revision rejection -> authority hashes unchanged
    let wrong_rev_receipt = CheckpointReceipt {
        receipt_id: "rcpt-wrong-rev".into(),
        checkpoint_id: "chk-01".into(),
        revision: 99,
        prompt_sha256: prompt_hash.clone(),
        execution_binding_sha256: binding_digest.clone(),
        commit: Some(head.clone()),
        accepted: true,
    };
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&wrong_rev_receipt).unwrap(),
    )
    .unwrap();
    let out = run(repo.path(), &prompt, &["--submit-checkpoint-receipt"]);
    assert_ne!(out.status.code(), Some(0));
    assert_eq!(
        get_authority_hashes(),
        base_hashes,
        "wrong-revision receipt must not alter authority hashes"
    );
    fs::remove_file(&receipt_path).unwrap();

    // 3. Wrong-commit rejection -> authority hashes unchanged
    let wrong_commit_receipt = CheckpointReceipt {
        receipt_id: "rcpt-wrong-commit".into(),
        checkpoint_id: "chk-01".into(),
        revision: 1,
        prompt_sha256: prompt_hash.clone(),
        execution_binding_sha256: binding_digest.clone(),
        commit: Some("0000000000000000000000000000000000000000".into()),
        accepted: true,
    };
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&wrong_commit_receipt).unwrap(),
    )
    .unwrap();
    let out = run(repo.path(), &prompt, &["--submit-checkpoint-receipt"]);
    assert_ne!(out.status.code(), Some(0));
    assert_eq!(
        get_authority_hashes(),
        base_hashes,
        "wrong-commit receipt must not alter authority hashes"
    );
    fs::remove_file(&receipt_path).unwrap();

    // 3b. Task-quality convention missing/invalid rejection -> authority hashes unchanged
    let conv_path = conv_dir.join("task-quality.md");
    let conv_backup = conv_dir.join("task-quality.md.bak");
    fs::rename(&conv_path, &conv_backup).unwrap();
    let valid_receipt_tmp = CheckpointReceipt {
        receipt_id: "rcpt-tq-fail".into(),
        checkpoint_id: "chk-01".into(),
        revision: 1,
        prompt_sha256: prompt_hash.clone(),
        execution_binding_sha256: binding_digest.clone(),
        commit: Some(head.clone()),
        accepted: true,
    };
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&valid_receipt_tmp).unwrap(),
    )
    .unwrap();
    let out = run(repo.path(), &prompt, &["--submit-checkpoint-receipt"]);
    assert_ne!(out.status.code(), Some(0));
    assert_eq!(
        get_authority_hashes(),
        base_hashes,
        "task-quality validation failure must not alter authority hashes"
    );
    fs::remove_file(&receipt_path).unwrap();
    fs::rename(&conv_backup, &conv_path).unwrap();

    // 4. Pending projection journal recovery before receipt handling
    let t1 = repo.path().join("proj_t1.txt");
    let t2 = repo.path().join("proj_t2.txt");
    let t3 = repo.path().join("proj_t3.txt");
    fs::write(&t1, b"old-1").unwrap();
    fs::write(&t2, b"old-2").unwrap();
    fs::write(&t3, b"old-3").unwrap();

    let journal_path = root.join("projection-journal.json");
    let _ = pipeline::projection_journal::apply_with_hook(
        &journal_path,
        [
            Projection {
                path: t1.clone(),
                bytes: b"recovered-1".to_vec(),
            },
            Projection {
                path: t2.clone(),
                bytes: b"recovered-2".to_vec(),
            },
            Projection {
                path: t3.clone(),
                bytes: b"recovered-3".to_vec(),
            },
        ],
        |boundary| {
            if boundary == Boundary::TargetReplaced(0) {
                Err(JournalError::Interrupted(boundary))
            } else {
                Ok(())
            }
        },
    );
    assert!(journal_path.exists());
    let j_uncommitted: serde_json::Value =
        serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
    assert_eq!(j_uncommitted["committed"], false);

    // 5. Valid checkpoint receipt single consumption
    let valid_receipt = CheckpointReceipt {
        receipt_id: "rcpt-01".into(),
        checkpoint_id: "chk-01".into(),
        revision: 1,
        prompt_sha256: prompt_hash.clone(),
        execution_binding_sha256: binding_digest.clone(),
        commit: Some(head.clone()),
        accepted: true,
    };
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&valid_receipt).unwrap(),
    )
    .unwrap();

    let out = run(repo.path(), &prompt, &["--submit-checkpoint-receipt"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "valid receipt submission must succeed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Verify pending journal was recovered during receipt handling
    let j_recovered: serde_json::Value =
        serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
    assert_eq!(j_recovered["committed"], true);
    assert_eq!(fs::read(&t1).unwrap(), b"recovered-1");
    assert_eq!(fs::read(&t2).unwrap(), b"recovered-2");
    assert_eq!(fs::read(&t3).unwrap(), b"recovered-3");

    // Verify single consumption: receipt file removed, coordinator revision updated
    assert!(
        !receipt_path.exists(),
        "orchestrator-receipt.json must be removed upon consumption"
    );
    let updated_state: CoordinatorState =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert_eq!(updated_state.revision, 2);
    assert_eq!(
        updated_state.consumed_checkpoint_receipts,
        vec!["rcpt-01".to_string()]
    );
    assert_eq!(
        updated_state.last_verified_evidence.as_deref(),
        Some("checkpoint:chk-01;receipt:rcpt-01")
    );
    assert_eq!(
        updated_state.next_action,
        NextAction::ResumeTask {
            task_id: task_id.clone(),
            phase: "implement".into(),
        }
    );

    // 6. Replay rejection -> authority hashes unchanged
    let hashes_after_consumption = get_authority_hashes();
    fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&valid_receipt).unwrap(),
    )
    .unwrap();
    let out = run(repo.path(), &prompt, &["--submit-checkpoint-receipt"]);
    assert_ne!(out.status.code(), Some(0));
    assert_eq!(
        get_authority_hashes(),
        hashes_after_consumption,
        "replayed receipt must not alter authority hashes"
    );
    fs::remove_file(&receipt_path).unwrap();

    // 7. Resume through entry latch - fail closed when latch fails
    // Set .dev/state.md to point to an mismatched plan so wrong-plan-guard fails
    fs::write(
        &state_md,
        "# State\n\n| mismatched | .dev/plans/other.prompt.md |\n",
    )
    .unwrap();
    let hashes_before_failing_resume = get_authority_hashes();
    let out = run(
        repo.path(),
        &prompt,
        &["--resume", "--task", &task_id, "--phase", "implement"],
    );
    assert_ne!(out.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("resume entry latch did not pass"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        get_authority_hashes(),
        hashes_before_failing_resume,
        "failing resume must not alter authority hashes"
    );

    // Verify failing resume wrote a preflight receipt recording overall: fail
    let preflight_receipts_after_fail: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("resume-preflight-")
        })
        .collect();
    assert_eq!(
        preflight_receipts_after_fail.len(),
        1,
        "exactly one preflight receipt after failed resume"
    );
    let fail_preflight_text = fs::read_to_string(preflight_receipts_after_fail[0].path()).unwrap();
    assert!(
        fail_preflight_text.contains("overall: fail"),
        "failing resume preflight receipt must show overall: fail"
    );

    // 8. Resume through entry latch - success when preflight passes
    // Restore matching state.md
    fs::write(
        &state_md,
        "# State\n\n| continuity | .dev/plans/continuity.prompt.md |\n",
    )
    .unwrap();
    let out = run(
        repo.path(),
        &prompt,
        &["--resume", "--task", &task_id, "--phase", "fix"],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "passing resume must succeed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let resumed_state: CoordinatorState =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert_eq!(resumed_state.revision, 3);
    assert_eq!(resumed_state.task_id, task_id);
    assert_eq!(resumed_state.phase, "fix");
    assert_eq!(
        resumed_state.next_action,
        NextAction::ResumeTask {
            task_id: task_id.clone(),
            phase: "fix".into(),
        }
    );

    // Verify fresh preflight receipt was written for passing resume
    let preflight_receipts_after_pass: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("resume-preflight-")
        })
        .collect();
    assert_eq!(
        preflight_receipts_after_pass.len(),
        2,
        "must have both failed and fresh passing preflight receipts"
    );
    let has_passing_preflight = preflight_receipts_after_pass.iter().any(|r| {
        fs::read_to_string(r.path())
            .unwrap()
            .contains("overall: pass")
    });
    assert!(
        has_passing_preflight,
        "a fresh preflight receipt must show overall: pass"
    );

    // 9. Provider-start count is zero across all operations
    let attempt_files: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("attempt-") || name.ends_with(".log")
        })
        .collect();
    assert_eq!(
        attempt_files.len(),
        0,
        "provider-start count must be zero: found {:?}",
        attempt_files
    );
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn walkdir(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walkdir(&path));
        } else {
            out.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                fs::read(path).unwrap(),
            ));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn reconcile_active_attempt_by_exact_process_identity_transitions() {
    let repo = TempDir::new().unwrap();
    init_git_repo(repo.path());

    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("continuity.prompt.md");
    let task_id = task(1);
    let probe_id = task_id.replacen('T', "TP", 1);
    let prompt_content = format!(
        "# Plan Prompt: Continuity\n\n\
        ## Files to Create or Modify\n\n\
        - `crates/cli/src/commands/pipeline_driver.rs`\n\n\
        ## Tasks\n\n\
        - [ ] {task_id} — Task goal description.\n  \
          - Targets: `crates/cli/src/commands/pipeline_driver.rs`\n  \
          - Change: Implement attempt reconciliation.\n  \
          - Acceptance: Deterministic transitions.\n\n\
        ## Test Plan\n\n\
        | ID | Type | Description | Covers |\n\
        | --- | --- | --- | --- |\n\
        | {probe_id} | integration | Continuity probe | {task_id} |\n\n\
        ## Status\n\n\
        Workflow: IMPLEMENT\n"
    );
    fs::write(&prompt, &prompt_content).unwrap();

    let state_md = repo.path().join(".dev/state.md");
    fs::write(
        &state_md,
        "# State\n\n| continuity | .dev/plans/continuity.prompt.md |\n",
    )
    .unwrap();

    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "init"])
        .status()
        .unwrap()
        .success());

    let prompt_hash = hash(&fs::read(&prompt).unwrap());
    let root = repo.path().join(".dev/pipeline/continuity");
    fs::create_dir_all(&root).unwrap();

    // Fixed process-identity fixture
    let fixed_attempt_id = "attempt-t38-fixed-fixture";
    let fixed_pid: u32 = 4242;
    let fixed_birth = "2026-09-29T10:00:00Z".to_string();

    // 1. Pure coordinator transition verification with fixed process-identity fixture
    // Tests: delayed (Live wait), conflict (Unknown on mismatch), restart (reloaded active attempt prevents duplicate dispatch, then finishes)
    {
        let mut pure_coord =
            CoordinatorState::new_legacy("continuity", &prompt_hash, &task_id, "implement");
        bind_fixture(&mut pure_coord, &std::env::current_dir().unwrap());
        pure_coord
            .activate_codex_stop_v1("fixed-activation-token".into())
            .unwrap();
        pure_coord.claim_attempt(fixed_attempt_id.into()).unwrap();
        pure_coord
            .bind_process(fixed_pid, Some(fixed_birth.clone()))
            .unwrap();

        // Conflict transition: observed live with mismatched birth identity -> Unknown / Blocked
        let conflict_birth_obs = OwnerObservation {
            verdict: AttemptOwnership::Live,
            observed_pid: Some(fixed_pid),
            observed_birth: Some("mismatched-birth-token".into()),
        };
        let verdict = pure_coord.recover_owner(conflict_birth_obs).unwrap();
        assert_eq!(verdict, AttemptOwnership::Unknown);
        assert_eq!(
            pure_coord.next_action,
            NextAction::Blocked {
                reason: "attempt-owner-unknown".into()
            }
        );

        // Delayed transition: observed live with matching pid and birth -> Live / ResumeTask
        let mut live_coord =
            CoordinatorState::new_legacy("continuity", &prompt_hash, &task_id, "implement");
        bind_fixture(&mut live_coord, &std::env::current_dir().unwrap());
        live_coord
            .activate_codex_stop_v1("fixed-activation-token".into())
            .unwrap();
        live_coord.claim_attempt(fixed_attempt_id.into()).unwrap();
        live_coord
            .bind_process(fixed_pid, Some(fixed_birth.clone()))
            .unwrap();

        let live_obs = OwnerObservation {
            verdict: AttemptOwnership::Live,
            observed_pid: Some(fixed_pid),
            observed_birth: Some(fixed_birth.clone()),
        };
        let verdict = live_coord.recover_owner(live_obs).unwrap();
        assert_eq!(verdict, AttemptOwnership::Live);
        assert_eq!(
            live_coord.next_action,
            NextAction::ResumeTask {
                task_id: task_id.clone(),
                phase: "implement".into(),
            }
        );

        // Restart / Duplicate dispatch prevention: active attempt cannot claim a second attempt
        let err = live_coord
            .claim_attempt("attempt-t38-second".into())
            .unwrap_err();
        assert_eq!(
            err,
            pipeline::coordinator::CoordinatorError::DispatchAlreadyClaimed
        );
    }

    // 2. CLI Unknown-owner path (conflict transition: recorded pid with no terminal log)
    // Setup initial coordinator state and extra files to verify zero mutations
    let coordinator_path = root.join("coordinator.json");
    let cursor_path = root.join("task-cursor.json");
    let progress_path = root.join("progress.json");
    let mock_attempt_path = root.join("mock-attempt.txt");

    fs::write(
        &cursor_path,
        format!("{{\"current_task\":\"{}\"}}", task(1)),
    )
    .unwrap();
    fs::write(&progress_path, b"{\"status\":\"pending\"}").unwrap();
    fs::write(&mock_attempt_path, b"mock-attempt-content").unwrap();

    let mut state = CoordinatorState::new_legacy("continuity", &prompt_hash, &task_id, "implement");
    state.profile = ContinuationProfile::CodexStopV1;
    bind_fixture(&mut state, &root);
    state.revision = 1;
    state.claim_attempt(fixed_attempt_id.into()).unwrap();
    state
        .bind_process(fixed_pid, Some(fixed_birth.clone()))
        .unwrap();
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let files_before_unknown = walkdir(&root);
    let out = run(repo.path(), &prompt, &[]);
    assert_ne!(
        out.status.code(),
        Some(0),
        "unknown-owner path must fail closed without kill or reclaim"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("attempt-owner-unknown"),
        "stderr must contain attempt-owner-unknown: {stderr}"
    );

    let files_after_unknown = walkdir(&root);
    assert_eq!(
        files_before_unknown, files_after_unknown,
        "cursor, attempt, progress and coordinator files must be unchanged on unknown-owner path"
    );

    // Verify provider-start count is zero
    let attempt_logs_unknown: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".log"))
        .collect();
    assert_eq!(
        attempt_logs_unknown.len(),
        0,
        "provider-start count must be zero on unknown-owner path"
    );

    // 2b. CLI Unknown-owner path with no PID (direct unknown)
    let mut state_no_pid =
        CoordinatorState::new_legacy("continuity", &prompt_hash, &task_id, "implement");
    state_no_pid.profile = ContinuationProfile::CodexStopV1;
    bind_fixture(&mut state_no_pid, &root);
    state_no_pid.revision = 1;
    state_no_pid.claim_attempt(fixed_attempt_id.into()).unwrap();
    // active_attempt has process_id: None
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state_no_pid).unwrap(),
    )
    .unwrap();

    let files_before_no_pid = walkdir(&root);
    let out_no_pid = run(repo.path(), &prompt, &[]);
    assert_ne!(out_no_pid.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out_no_pid.stderr)
        .contains("attempt-owner-unknown: refusing kill, reclaim, or redispatch"));
    let files_after_no_pid = walkdir(&root);
    assert_eq!(
        files_before_no_pid, files_after_no_pid,
        "files must remain unchanged when attempt has no PID"
    );

    // 3. CLI Attach transition (dead attempt with terminal log)
    // Add an unreadable non-UTF-8 binary file to ensure find_terminal_attempt_log skips it cleanly
    let binary_unreadable_path = root.join("corrupt-attempt.bin");
    fs::write(&binary_unreadable_path, [0xFF, 0xFE, 0xFD, 0x80]).unwrap();

    // Reset coordinator to active attempt with fixed_pid
    state.revision = 2;
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    // Write terminal attempt log proving completion
    let terminal_log_path = root.join(format!("{fixed_attempt_id}.log"));
    let terminal_log_content = format!(
        "attempt_id:       {fixed_attempt_id}\n\
        task_id:          {task_id}\n\
        phase:            implement\n\
        terminal_state:   completed\n"
    );
    fs::write(&terminal_log_path, terminal_log_content.as_bytes()).unwrap();

    let out_attach = run(repo.path(), &prompt, &[]);
    assert_eq!(
        out_attach.status.code(),
        Some(0),
        "attach transition must succeed: stderr={}",
        String::from_utf8_lossy(&out_attach.stderr)
    );
    let attach_stdout = String::from_utf8_lossy(&out_attach.stdout);
    assert!(
        attach_stdout.contains(&format!(
            "attempt={fixed_attempt_id} ownership=dead action=attached log="
        )),
        "stdout must indicate attached action: {attach_stdout}"
    );

    // Check coordinator state was updated: active_attempt cleared, evidence recorded, revision incremented
    let attached_state: CoordinatorState =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert!(
        attached_state.active_attempt.is_none(),
        "active_attempt must be cleared after attach transition"
    );
    assert!(
        attached_state
            .last_verified_evidence
            .as_ref()
            .is_some_and(|e| e.starts_with("attempt-log:")),
        "last_verified_evidence must record the terminal attempt log"
    );
    assert!(
        attached_state.revision > state.revision,
        "revision must be incremented atomically"
    );

    // Provider-start count is zero on attach path (no new attempt logs were created)
    let attempt_logs_attach: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.ends_with(".log") && name != format!("{fixed_attempt_id}.log")
        })
        .collect();
    assert_eq!(
        attempt_logs_attach.len(),
        0,
        "provider-start count must be zero on attach path"
    );

    // 4. Restart transition: re-entering after attach
    // Coordinator now has active_attempt = None; running status reports clean state without restarting provider
    let out_status = run(repo.path(), &prompt, &["--status"]);
    assert_eq!(out_status.status.code(), Some(0));
    let status_str = String::from_utf8_lossy(&out_status.stdout);
    assert!(status_str.contains("profile=CodexStopV1"));

    // 5. Delayed (Live wait) CLI transition if on Linux or platform where process_birth_identity returns an identity
    if let Some(live_birth) = dispatch::dispatch::process_birth_identity(std::process::id()) {
        let mut live_cli_state =
            CoordinatorState::new_legacy("continuity", &prompt_hash, &task_id, "implement");
        live_cli_state.profile = ContinuationProfile::CodexStopV1;
        bind_fixture(&mut live_cli_state, &root);
        live_cli_state.revision = 10;
        live_cli_state
            .claim_attempt("attempt-t38-live".into())
            .unwrap();
        live_cli_state
            .bind_process(std::process::id(), Some(live_birth))
            .unwrap();
        fs::write(
            &coordinator_path,
            serde_json::to_vec_pretty(&live_cli_state).unwrap(),
        )
        .unwrap();

        let live_out = run(repo.path(), &prompt, &[]);
        assert_eq!(
            live_out.status.code(),
            Some(0),
            "live wait transition must exit 0: stderr={}",
            String::from_utf8_lossy(&live_out.stderr)
        );
        let live_stdout = String::from_utf8_lossy(&live_out.stdout);
        assert!(
            live_stdout.contains("ownership=live action=wait"),
            "stdout must indicate live wait action: {live_stdout}"
        );
        let live_saved: CoordinatorState =
            serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
        assert_eq!(live_saved.revision, 11);
    }
}

fn ensure_fixture_executor() -> &'static Path {
    static FIXTURE_DIR: std::sync::OnceLock<TempDir> = std::sync::OnceLock::new();
    FIXTURE_DIR
        .get_or_init(|| {
            let tmp = TempDir::new().expect("create fixture temp dir");
            let src_path = tmp.path().join("mock_executor.rs");
            let src = r####"
use std::io::Read;
fn main() {
    let mut stdin_content = String::new();
    let _ = std::io::stdin().read_to_string(&mut stdin_content);
    for line in stdin_content.lines() {
        if let Some(pos) = line.find("receipt file: ") {
            let mut path = line[pos + "receipt file: ".len()..].trim();
            if path.ends_with('.') {
                path = &path[..path.len() - 1];
            }
            if !path.is_empty() {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let content = if path.contains("-test.receipt.md") {
                    if let Ok(c) = std::env::var("MOCK_TEST_RECEIPT") {
                        c
                    } else {
                        format!("### [T-{:02}] 2026-09-29\nVerdict: PASS\nEvidence: test passed\n", 1)
                    }
                } else if path.contains("-audit.receipt.md") {
                    if let Ok(c) = std::env::var("MOCK_AUDIT_RECEIPT") {
                        c
                    } else {
                        format!("### [T-{:02}] 2026-09-29\nVerdict: APPROVE\n<!-- AUDIT_REVIEW: CLEAR -->\nEvidence: audit passed\n", 1)
                    }
                } else {
                    format!("### [T-{:02}] 2026-09-29\noverall: pass\n", 1)
                };
                let _ = std::fs::write(path, content);
            }
        }
    }
    if let Ok(receipt_path) = std::env::var("MOCK_RECEIPT_PATH") {
        if let Some(parent) = std::path::Path::new(&receipt_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&receipt_path, format!("### [T-{:02}] 2026-09-29\noverall: pass\n", 1));
    }
    if let Ok(touch_path) = std::env::var("MOCK_TOUCH_FILE") {
        if let Some(parent) = std::path::Path::new(&touch_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&touch_path, b"unauthorized change");
    } else {
        // In the normal passing path, perform valid in-target edit
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("crates/pipeline/src/task_spec.rs")
            .and_then(|mut f| std::io::Write::write_all(&mut f, b"// implement writeback\n"));
    }
    println!("{{\"conversation_id\":\"176f1141-b606-47aa-a47b-76f2a6147623\",\"status\":\"SUCCESS\"}}");
}
"####;
            std::fs::write(&src_path, src).expect("write mock executor source");
            #[cfg(target_os = "windows")]
            let exe_name = "agy.exe";
            #[cfg(not(target_os = "windows"))]
            let exe_name = "agy";
            let primary_exe = tmp.path().join(exe_name);
            let status = Command::new("rustc")
                .arg(&src_path)
                .arg("-o")
                .arg(&primary_exe)
                .status()
                .expect("compile mock executor with rustc");
            assert!(status.success(), "rustc compilation must succeed");
            tmp
        })
        .path()
}

fn run_with_env(
    root: &Path,
    prompt: &Path,
    extra: &[&str],
    envs: &[(&str, &std::ffi::OsStr)],
) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_gal"));
    cmd.current_dir(root)
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .args(extra);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

#[test]
fn guarded_implement_segment_drives_in_process_dispatch_and_boundary_check() {
    // 1. Pure coordinator transition verification: claim -> bind -> finish in that order
    let task_id = task(1);
    let prompt_hash = "mock-prompt-hash".to_string();
    let mut pure_coord =
        CoordinatorState::new_legacy("continuity", &prompt_hash, &task_id, "implement");
    bind_fixture(&mut pure_coord, &std::env::current_dir().unwrap());
    pure_coord.activate_codex_stop_v1("token".into()).unwrap();
    assert_eq!(pure_coord.revision, 2);

    // Out of order: cannot bind or finish before claim
    assert_eq!(
        pure_coord.bind_process(1234, None).unwrap_err(),
        pipeline::coordinator::CoordinatorError::AttemptMissing
    );
    assert_eq!(
        pure_coord.finish_attempt("attempt-1", None).unwrap_err(),
        pipeline::coordinator::CoordinatorError::AttemptMissing
    );

    // 1a. claim_attempt
    let rev_claim = pure_coord.claim_attempt("attempt-1".into()).unwrap();
    assert_eq!(rev_claim, 3);
    assert_eq!(pure_coord.revision, 3);
    assert!(pure_coord
        .active_attempt
        .as_ref()
        .is_some_and(|a| a.dispatch_claimed));

    // Cannot claim a second attempt while active attempt exists
    assert_eq!(
        pure_coord.claim_attempt("attempt-2".into()).unwrap_err(),
        pipeline::coordinator::CoordinatorError::DispatchAlreadyClaimed
    );

    // 1b. bind_process
    let rev_bind = pure_coord
        .bind_process(1234, Some("birth-token".into()))
        .unwrap();
    assert_eq!(rev_bind, 4);
    assert_eq!(pure_coord.revision, 4);
    assert_eq!(
        pure_coord.active_attempt.as_ref().unwrap().process_id,
        Some(1234)
    );

    // 1c. finish_attempt
    assert_eq!(
        pure_coord
            .finish_attempt("wrong-attempt", None)
            .unwrap_err(),
        pipeline::coordinator::CoordinatorError::InvalidTransition("attempt identity mismatch")
    );
    let rev_finish = pure_coord
        .finish_attempt(
            "attempt-1",
            Some("implement-dispatch:Success;boundary:pass".into()),
        )
        .unwrap();
    assert_eq!(rev_finish, 5);
    assert_eq!(pure_coord.revision, 5);
    assert!(pure_coord.active_attempt.is_none());
    assert_eq!(
        pure_coord.last_verified_evidence.as_deref(),
        Some("implement-dispatch:Success;boundary:pass")
    );

    // 2. Setup repo and mock executor environment
    let repo = TempDir::new().unwrap();
    init_git_repo(repo.path());

    // gitignore .dev/pipeline/ so git status is clean
    fs::write(repo.path().join(".gitignore"), ".dev/pipeline/\n").unwrap();

    // Conventions
    let conv_dir = repo.path().join("plugins/gal-core/conventions");
    fs::create_dir_all(&conv_dir).unwrap();
    fs::write(
        conv_dir.join("task-quality.md"),
        "# Task Quality\n\nCanonical questions for checking whether a task is sufficiently specified\n",
    )
    .unwrap();

    // Agents contract
    let agents_dir = repo.path().join("plugins/gal-core/agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join("golem-implementer.agent.md"),
        "---\nname: golem-implementer\n---\n",
    )
    .unwrap();

    // Target file declared in prompt
    let target_dir = repo.path().join("crates/pipeline/src");
    fs::create_dir_all(&target_dir).unwrap();
    let target_file = target_dir.join("task_spec.rs");
    fs::write(&target_file, b"// original content\n").unwrap();

    // Execution prompt and state.md
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("continuity.prompt.md");
    let probe_id = task_id.replacen('T', "TP", 1);
    let prompt_content = format!(
        "# Plan Prompt: Continuity\n\n\
        ## Files to Create or Modify\n\n\
        - `crates/pipeline/src/task_spec.rs`\n\n\
        ## Tasks\n\n\
        - [ ] {task_id} — Task goal description.\n  \
          - Targets: `crates/pipeline/src/task_spec.rs`\n  \
          - Change: Implement guarded implement segment.\n  \
          - Acceptance: Verifies all transitions.\n\n\
        ## Test Plan\n\n\
        | ID | Type | Description | Covers |\n\
        | --- | --- | --- | --- |\n\
        | {probe_id} | integration | Continuity probe | {task_id} |\n\n\
        ## Status\n\n\
        Workflow: IMPLEMENT\n"
    );
    fs::write(&prompt, &prompt_content).unwrap();

    let state_md = repo.path().join(".dev/state.md");
    fs::write(
        &state_md,
        "# State\n\n| continuity | .dev/plans/continuity.prompt.md |\n",
    )
    .unwrap();

    // Commit baseline in git
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "init"])
        .status()
        .unwrap()
        .success());

    // Fixture executor and mock routing home
    let fixture_dir = ensure_fixture_executor();
    let user_home = TempDir::new().unwrap();
    let cfg_dir = user_home.path().join(".gal/config");
    fs::create_dir_all(&cfg_dir).unwrap();
    let routing_json = r#"{
        "executorRouting": {
            "executors": { "agy": "gemini-2.5-pro" },
            "pipeline": {
                "CODER": { "executor": "agy" }
            }
        }
    }"#;
    fs::write(cfg_dir.join("config.json"), routing_json).unwrap();

    let mut paths = vec![fixture_dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let new_path = std::env::join_paths(paths).unwrap();

    let home_str = user_home.path().as_os_str();
    let key_str = std::ffi::OsStr::new("mock-gemini-key");

    let base_envs: [(&str, &std::ffi::OsStr); 4] = [
        ("USERPROFILE", home_str),
        ("HOME", home_str),
        ("PATH", &new_path),
        ("GEMINI_API_KEY", key_str),
    ];

    let root = repo.path().join(".dev/pipeline/continuity");
    fs::create_dir_all(&root).unwrap();
    let coordinator_path = root.join("coordinator.json");
    let prompt_hash = hash(&fs::read(&prompt).unwrap());

    let task_log_dir = root.join(&task_id);
    let count_attempt_logs = || -> usize {
        if !task_log_dir.exists() {
            return 0;
        }
        fs::read_dir(&task_log_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".log"))
            .count()
    };

    // 3. Zero provider starts while active attempt exists
    let mut state = CoordinatorState::new_legacy("continuity", &prompt_hash, &task_id, "implement");
    state.profile = ContinuationProfile::CodexStopV1;
    bind_fixture(&mut state, &root);
    state.revision = 1;
    state
        .claim_attempt("active-existing-attempt".into())
        .unwrap();
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    assert_eq!(count_attempt_logs(), 0);

    let out_blocked = run_with_env(
        repo.path(),
        &prompt,
        &["--resume", "--task", &task_id, "--phase", "implement"],
        &base_envs,
    );
    assert_ne!(out_blocked.status.code(), Some(0));
    let blocked_err = String::from_utf8_lossy(&out_blocked.stderr);
    assert!(
        blocked_err.contains("active attempt exists; refusing provider start"),
        "stderr: {blocked_err}"
    );
    assert_eq!(
        count_attempt_logs(),
        0,
        "zero provider starts while active attempt exists"
    );

    // 4. Advance to test phase on passing boundary check
    state.active_attempt = None;
    state.revision = 1;
    state.next_action = NextAction::ResumeTask {
        task_id: task_id.clone(),
        phase: "implement".into(),
    };
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let out_pass = run_with_env(
        repo.path(),
        &prompt,
        &["--resume", "--task", &task_id, "--phase", "implement"],
        &base_envs,
    );
    assert_eq!(
        out_pass.status.code(),
        Some(0),
        "passing implement segment must succeed: stderr={}",
        String::from_utf8_lossy(&out_pass.stderr)
    );

    // Exactly one provider start for the implement segment
    assert_eq!(
        count_attempt_logs(),
        1,
        "exactly one provider start for implement segment"
    );

    // Executor log terminal state completed
    let log_file = fs::read_dir(&task_log_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().ends_with(".log"))
        .unwrap();
    let log_content = fs::read_to_string(log_file.path()).unwrap();
    assert!(
        log_content.contains("terminal_state:  completed"),
        "executor log must show completed: {log_content}"
    );
    assert!(
        log_content.contains("phase:           implement"),
        "executor log must show phase implement: {log_content}"
    );

    // Coordinator advanced to test phase
    let passed_state: CoordinatorState =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert_eq!(
        passed_state.phase, "test",
        "passing boundary check must advance phase to test"
    );
    assert_eq!(
        passed_state.next_action,
        NextAction::StartTask {
            task_id: task_id.clone(),
            phase: "test".into(),
        },
        "next action must be start test task"
    );
    assert!(
        passed_state.active_attempt.is_none(),
        "active attempt must be cleared after finish"
    );
    assert!(
        passed_state
            .last_verified_evidence
            .as_ref()
            .is_some_and(|e| e.contains("boundary:pass")),
        "last_verified_evidence must record boundary:pass: {:?}",
        passed_state.last_verified_evidence
    );
    // Revision progression: 1 (initial) -> 2 (resume latch) -> 3 (claim) -> 4 (bind) -> 5 (finish)
    assert_eq!(passed_state.revision, 5);

    // 5. Failing boundary check raises BoundaryWidening checkpoint with no Target change
    let unauthorized_path = repo.path().join("crates/pipeline/src/outside_target.txt");
    let touch_str = unauthorized_path.as_os_str();
    let mut fail_envs = base_envs.to_vec();
    fail_envs.push(("MOCK_TOUCH_FILE", touch_str));

    // Snapshot prompt before run to verify no Target change
    let prompt_before = fs::read_to_string(&prompt).unwrap();

    state.phase = "implement".into();
    state.revision = 10;
    state.active_attempt = None;
    state.next_action = NextAction::ResumeTask {
        task_id: task_id.clone(),
        phase: "implement".into(),
    };
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let logs_before_fail = count_attempt_logs();

    let out_fail = run_with_env(
        repo.path(),
        &prompt,
        &["--resume", "--task", &task_id, "--phase", "implement"],
        &fail_envs,
    );
    assert_eq!(
        out_fail.status.code(),
        Some(0),
        "failing boundary check stops at checkpoint and exits 0: stderr={}",
        String::from_utf8_lossy(&out_fail.stderr)
    );

    // Exactly one provider start occurred
    assert_eq!(count_attempt_logs(), logs_before_fail + 1);

    let failed_state: CoordinatorState =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert_eq!(
        failed_state.phase, "implement",
        "phase remains implement on boundary failure"
    );
    assert!(failed_state.active_attempt.is_none());
    assert!(
        matches!(
            failed_state.checkpoint.as_ref().map(|c| c.kind),
            Some(CheckpointKind::BoundaryWidening)
        ),
        "checkpoint kind must be BoundaryWidening: {:?}",
        failed_state.checkpoint
    );
    let chk_id = failed_state
        .checkpoint
        .as_ref()
        .unwrap()
        .checkpoint_id
        .clone();
    assert_eq!(
        failed_state.next_action,
        NextAction::AwaitOrchestrator {
            checkpoint_id: chk_id
        }
    );
    assert!(
        failed_state
            .last_verified_evidence
            .as_ref()
            .is_some_and(|e| e.contains("boundary:fail")),
        "last_verified_evidence must record boundary:fail: {:?}",
        failed_state.last_verified_evidence
    );

    // Verify driver never widened Targets itself
    let prompt_after = fs::read_to_string(&prompt).unwrap();
    assert_eq!(
        prompt_before, prompt_after,
        "driver must never change Targets or prompt on boundary widening failure"
    );
}

#[test]
fn guarded_test_and_audit_segments_drive_retry_ceiling_and_convergence() {
    let task_id = task(1);
    let repo = TempDir::new().unwrap();
    init_git_repo(repo.path());

    // gitignore .dev/pipeline/ so git status is clean
    fs::write(repo.path().join(".gitignore"), ".dev/pipeline/\n").unwrap();

    // Conventions
    let conv_dir = repo.path().join("plugins/gal-core/conventions");
    fs::create_dir_all(&conv_dir).unwrap();
    fs::write(
        conv_dir.join("task-quality.md"),
        "# Task Quality\n\nCanonical questions for checking whether a task is sufficiently specified\n",
    )
    .unwrap();

    // Agents contracts for all phases
    let agents_dir = repo.path().join("plugins/gal-core/agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join("golem-implementer.agent.md"),
        "---\nname: golem-implementer\n---\n",
    )
    .unwrap();
    fs::write(
        agents_dir.join("golem-tester.agent.md"),
        "---\nname: golem-tester\n---\n",
    )
    .unwrap();
    fs::write(
        agents_dir.join("golem-auditor.agent.md"),
        "---\nname: golem-auditor\n---\n",
    )
    .unwrap();

    // Target file declared in prompt
    let target_dir = repo.path().join("crates/pipeline/src");
    fs::create_dir_all(&target_dir).unwrap();
    let target_file = target_dir.join("task_spec.rs");
    fs::write(&target_file, b"// original content\n").unwrap();

    let state_md = repo.path().join(".dev/state.md");
    fs::create_dir_all(state_md.parent().unwrap()).unwrap();
    fs::write(
        &state_md,
        "# State\n\n| continuity | .dev/plans/continuity.prompt.md |\n",
    )
    .unwrap();

    // Commit baseline in git
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "init"])
        .status()
        .unwrap()
        .success());

    let head_out = Command::new("git")
        .current_dir(repo.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(head_out.status.success());
    let head = String::from_utf8_lossy(&head_out.stdout).trim().to_string();

    // Execution prompt
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("continuity.prompt.md");
    let probe_id = task_id.replacen('T', "TP", 1);
    let prompt_content = format!(
        "# Plan Prompt: Continuity\n\n\
        ## Files to Create or Modify\n\n\
        - `crates/pipeline/src/task_spec.rs`\n\n\
        ## Tasks\n\n\
        - [ ] {task_id} — Task goal description.\n  \
          - Targets: `crates/pipeline/src/task_spec.rs`\n  \
          - Change: Implement test and audit segments.\n  \
          - Acceptance: Verifies all transitions.\n\n\
        ## Status\n\n\
        Workflow: IMPLEMENT\n\
        Task Final Commit: {head}\n\n\
        ## Test Results\n\n\
        ## Review Results\n\n\
        ## Test Plan\n\n\
        | ID | Type | Description | Covers |\n\
        | --- | --- | --- | --- |\n\
        | {probe_id} | integration | Continuity probe | {task_id} |\n"
    );
    fs::write(&prompt, &prompt_content).unwrap();

    // Fixture executor and mock routing home
    let fixture_dir = ensure_fixture_executor();
    let user_home = TempDir::new().unwrap();
    let cfg_dir = user_home.path().join(".gal/config");
    fs::create_dir_all(&cfg_dir).unwrap();
    let routing_json = r#"{
        "executorRouting": {
            "executors": { "agy": "gemini-2.5-pro" },
            "pipeline": {
                "TESTER": { "executor": "agy" },
                "AUDITOR": { "executor": "agy" },
                "CODER": { "executor": "agy" }
            }
        }
    }"#;
    fs::write(cfg_dir.join("config.json"), routing_json).unwrap();

    let mut paths = vec![fixture_dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let new_path = std::env::join_paths(paths).unwrap();

    let home_str = user_home.path().as_os_str();
    let key_str = std::ffi::OsStr::new("mock-gemini-key");

    let base_envs: [(&str, &std::ffi::OsStr); 4] = [
        ("USERPROFILE", home_str),
        ("HOME", home_str),
        ("PATH", &new_path),
        ("GEMINI_API_KEY", key_str),
    ];

    let root = repo.path().join(".dev/pipeline/continuity");
    fs::create_dir_all(&root).unwrap();
    let coordinator_path = root.join("coordinator.json");
    let prompt_hash = hash(&fs::read(&prompt).unwrap());

    let task_log_dir = root.join(&task_id);
    let count_attempt_logs = || -> usize {
        if !task_log_dir.exists() {
            return 0;
        }
        fs::read_dir(&task_log_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".log"))
            .count()
    };

    // 1. Happy path: test passes, then audit passes -> advances to converge
    let mut state = CoordinatorState::new_legacy("continuity", &prompt_hash, &task_id, "test");
    state.profile = ContinuationProfile::CodexStopV1;
    bind_fixture(&mut state, &root);
    state.revision = 1;
    state.next_action = NextAction::ResumeTask {
        task_id: task_id.clone(),
        phase: "test".into(),
    };
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let out_happy = run_with_env(
        repo.path(),
        &prompt,
        &["--resume", "--task", &task_id, "--phase", "test"],
        &base_envs,
    );
    assert_eq!(
        out_happy.status.code(),
        Some(0),
        "happy path test + audit must succeed: stderr={}",
        String::from_utf8_lossy(&out_happy.stderr)
    );

    // Exactly 2 starts: test then audit, separate dispatches
    assert_eq!(
        count_attempt_logs(),
        2,
        "test and audit must be separate dispatches (2 starts)"
    );

    let logs: Vec<String> = fs::read_dir(&task_log_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".log"))
        .map(|e| fs::read_to_string(e.path()).unwrap())
        .collect();

    assert!(
        logs.iter().any(
            |l| l.contains("phase:           test") && l.contains("terminal_state:  completed")
        ),
        "test phase log must show completed"
    );
    assert!(
        logs.iter()
            .any(|l| l.contains("phase:           audit")
                && l.contains("terminal_state:  completed")),
        "audit phase log must show completed"
    );

    let happy_state: CoordinatorState =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert_eq!(
        happy_state.phase, "converge",
        "passing audit sets phase to converge"
    );
    assert_eq!(
        happy_state.next_action,
        NextAction::StartTask {
            task_id: task_id.clone(),
            phase: "converge".into(),
        },
        "passing audit sets next action to convergence"
    );
    assert!(
        happy_state
            .last_verified_evidence
            .as_deref()
            .is_some_and(|e| e.contains("audit-verdict:pass")),
        "evidence must record audit-verdict:pass: {:?}",
        happy_state.last_verified_evidence
    );

    // 2. Failing test records retry and returns implement fix action; audit does not run
    let test_fail_receipt =
        format!("### [{task_id}] 2026-09-29\n\nVerdict: FAIL\nEvidence: test assertion failed\n");
    let test_fail_str = std::ffi::OsStr::new(&test_fail_receipt);
    let mut test_fail_envs = base_envs.to_vec();
    test_fail_envs.push(("MOCK_TEST_RECEIPT", test_fail_str));

    state.phase = "test".into();
    state.revision = 10;
    state.retry.attempt = 0;
    state.retry.max_attempts = 3;
    state.active_attempt = None;
    state.next_action = NextAction::ResumeTask {
        task_id: task_id.clone(),
        phase: "test".into(),
    };
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let logs_before_test_fail = count_attempt_logs();

    let out_test_fail = run_with_env(
        repo.path(),
        &prompt,
        &["--resume", "--task", &task_id, "--phase", "test"],
        &test_fail_envs,
    );
    assert_eq!(
        out_test_fail.status.code(),
        Some(0),
        "failing test records retry and exits 0"
    );

    // Only test ran, audit was skipped -> exactly 1 new provider start
    assert_eq!(
        count_attempt_logs(),
        logs_before_test_fail + 1,
        "failing test must not run audit (exactly 1 provider start)"
    );

    let test_fail_state: CoordinatorState =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert_eq!(
        test_fail_state.retry.attempt, 1,
        "one retry recorded for failing test"
    );
    assert_eq!(
        test_fail_state.next_action,
        NextAction::RetryTask {
            task_id: task_id.clone(),
            phase: "fix".into(),
            attempt: 1,
        },
        "failing test returns implement fix action"
    );

    // 3. Blocking audit result records retry and returns implement fix action
    let test_pass_receipt =
        format!("### [{task_id}] 2026-09-29\n\nVerdict: PASS\nEvidence: test passed\n");
    let test_pass_str = std::ffi::OsStr::new(&test_pass_receipt);
    let audit_block_receipt = format!("### [{task_id}] 2026-09-29\n\nVerdict: REJECT\n<!-- AUDIT_REVIEW: FINDINGS-OPEN -->\nEvidence: blocking audit finding\n");
    let audit_block_str = std::ffi::OsStr::new(&audit_block_receipt);
    let mut audit_block_envs = base_envs.to_vec();
    audit_block_envs.push(("MOCK_TEST_RECEIPT", test_pass_str));
    audit_block_envs.push(("MOCK_AUDIT_RECEIPT", audit_block_str));

    state.phase = "test".into();
    state.revision = 20;
    state.retry.attempt = 1;
    state.retry.max_attempts = 3;
    state.active_attempt = None;
    state.next_action = NextAction::ResumeTask {
        task_id: task_id.clone(),
        phase: "test".into(),
    };
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let logs_before_audit_block = count_attempt_logs();

    let out_audit_block = run_with_env(
        repo.path(),
        &prompt,
        &["--resume", "--task", &task_id, "--phase", "test"],
        &audit_block_envs,
    );
    assert_eq!(
        out_audit_block.status.code(),
        Some(0),
        "blocking audit records retry and exits 0"
    );

    // Both test and audit ran -> 2 provider starts
    assert_eq!(
        count_attempt_logs(),
        logs_before_audit_block + 2,
        "test and audit both ran before blocking audit stopped (2 provider starts)"
    );

    let audit_block_state: CoordinatorState =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert_eq!(
        audit_block_state.retry.attempt, 2,
        "one retry recorded for blocking audit"
    );
    assert_eq!(
        audit_block_state.next_action,
        NextAction::RetryTask {
            task_id: task_id.clone(),
            phase: "fix".into(),
            attempt: 2,
        },
        "blocking audit returns implement fix action"
    );

    // 4. Protected Path / security audit finding stops at once with a blocked action
    let audit_prot_receipt = format!("### [{task_id}] 2026-09-29\n\nVerdict: REJECT\n<!-- AUDIT_REVIEW: FINDINGS-OPEN -->\nEvidence: Protected Path finding in plugins/gal-core/conventions/\n");
    let audit_prot_str = std::ffi::OsStr::new(&audit_prot_receipt);
    let mut audit_prot_envs = base_envs.to_vec();
    audit_prot_envs.push(("MOCK_TEST_RECEIPT", test_pass_str));
    audit_prot_envs.push(("MOCK_AUDIT_RECEIPT", audit_prot_str));

    state.phase = "test".into();
    state.revision = 30;
    state.retry.attempt = 0;
    state.retry.max_attempts = 3;
    state.active_attempt = None;
    state.next_action = NextAction::ResumeTask {
        task_id: task_id.clone(),
        phase: "test".into(),
    };
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let out_audit_prot = run_with_env(
        repo.path(),
        &prompt,
        &["--resume", "--task", &task_id, "--phase", "test"],
        &audit_prot_envs,
    );
    assert_eq!(
        out_audit_prot.status.code(),
        Some(0),
        "protected path finding stops at once and exits 0"
    );

    let audit_prot_state: CoordinatorState =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert_eq!(
        audit_prot_state.next_action,
        NextAction::Blocked {
            reason: "security-or-protected-path-finding".into(),
        },
        "Protected Path finding must stop at once with blocked action"
    );
    assert_eq!(
        audit_prot_state.retry.attempt, 0,
        "security/protected-path finding does not consume retry attempt"
    );

    // 5. Blocked action with zero further starts at the ceiling
    state.phase = "test".into();
    state.revision = 40;
    state.retry.attempt = 2; // Next failure hits max_attempts = 3
    state.retry.max_attempts = 3;
    state.active_attempt = None;
    state.next_action = NextAction::ResumeTask {
        task_id: task_id.clone(),
        phase: "test".into(),
    };
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let logs_before_ceiling = count_attempt_logs();

    let out_reach_ceiling = run_with_env(
        repo.path(),
        &prompt,
        &["--resume", "--task", &task_id, "--phase", "test"],
        &test_fail_envs,
    );
    assert_eq!(out_reach_ceiling.status.code(), Some(0));

    assert_eq!(count_attempt_logs(), logs_before_ceiling + 1);

    let ceiling_state: CoordinatorState =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert_eq!(
        ceiling_state.retry.attempt, 3,
        "attempt count reaches ceiling (3)"
    );
    assert_eq!(
        ceiling_state.next_action,
        NextAction::Blocked {
            reason: "retry-limit-exhausted".into(),
        },
        "next action becomes blocked at retry ceiling"
    );

    // Attempting further resume at the ceiling must start zero providers
    let logs_at_ceiling = count_attempt_logs();
    let out_blocked_ceiling = run_with_env(
        repo.path(),
        &prompt,
        &["--resume", "--task", &task_id, "--phase", "test"],
        &test_fail_envs,
    );
    assert_ne!(
        out_blocked_ceiling.status.code(),
        Some(0),
        "running at retry ceiling must fail closed"
    );
    assert!(
        String::from_utf8_lossy(&out_blocked_ceiling.stderr).contains("retry limit exhausted"),
        "stderr must report retry limit exhausted: {}",
        String::from_utf8_lossy(&out_blocked_ceiling.stderr)
    );
    assert_eq!(
        count_attempt_logs(),
        logs_at_ceiling,
        "zero further provider starts at retry ceiling"
    );
}

#[test]
fn guarded_task_convergence_and_goal_backward_pause() {
    let task_id = task(1);
    let repo = TempDir::new().unwrap();
    init_git_repo(repo.path());

    // gitignore .dev/pipeline/ so git status is clean
    fs::write(repo.path().join(".gitignore"), ".dev/pipeline/\n").unwrap();

    // Conventions
    let conv_dir = repo.path().join("plugins/gal-core/conventions");
    fs::create_dir_all(&conv_dir).unwrap();
    fs::write(
        conv_dir.join("task-quality.md"),
        "# Task Quality\n\nCanonical questions for checking whether a task is sufficiently specified\n",
    )
    .unwrap();

    // Agents contracts for all phases
    let agents_dir = repo.path().join("plugins/gal-core/agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join("golem-implementer.agent.md"),
        "---\nname: golem-implementer\n---\n",
    )
    .unwrap();
    fs::write(
        agents_dir.join("golem-tester.agent.md"),
        "---\nname: golem-tester\n---\n",
    )
    .unwrap();
    fs::write(
        agents_dir.join("golem-auditor.agent.md"),
        "---\nname: golem-auditor\n---\n",
    )
    .unwrap();

    // Target file declared in prompt
    let target_dir = repo.path().join("crates/pipeline/src");
    fs::create_dir_all(&target_dir).unwrap();
    let target_file = target_dir.join("task_spec.rs");
    fs::write(&target_file, b"// original content\n").unwrap();

    let state_md = repo.path().join(".dev/state.md");
    fs::create_dir_all(state_md.parent().unwrap()).unwrap();
    fs::write(
        &state_md,
        "# State\n\n| continuity | .dev/plans/continuity.prompt.md |\n",
    )
    .unwrap();

    // Source plan
    let plan_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&plan_dir).unwrap();
    let source_plan = plan_dir.join("continuity.md");
    fs::write(
        &source_plan,
        format!("# Plan: Continuity\n\n## Tasks\n\n- [ ] {task_id} — Task goal description.\n"),
    )
    .unwrap();

    // Execution prompt
    let prompt = plan_dir.join("continuity.prompt.md");
    let probe_id = task_id.replacen('T', "TP", 1);
    let prompt_content = format!(
        "# Plan Prompt: Continuity\n\n\
        ## Files to Create or Modify\n\n\
        - `crates/pipeline/src/task_spec.rs`\n\n\
        ## Tasks\n\n\
        - [ ] {task_id} — Task goal description.\n  \
          - Targets: `crates/pipeline/src/task_spec.rs`\n  \
          - Change: Implement convergence.\n  \
          - Acceptance: Verifies all transitions.\n\n\
        ## Status\n\n\
        Workflow: IMPLEMENT\n\
        Current Task: {task_id}\n\
        Task Final Commit: —\n\n\
        ## Test Results\n\n\
        ## Review Results\n\n\
        ## Test Plan\n\n\
        | ID | Type | Description | Covers |\n\
        | --- | --- | --- | --- |\n\
        | {probe_id} | integration | Continuity probe | {task_id} |\n"
    );
    fs::write(&prompt, &prompt_content).unwrap();

    // Commit baseline in git
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "init"])
        .status()
        .unwrap()
        .success());

    let head_out = Command::new("git")
        .current_dir(repo.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(head_out.status.success());
    let head = String::from_utf8_lossy(&head_out.stdout).trim().to_string();

    // Update Task Final Commit to head
    let prompt_content = prompt_content.replace(
        "Task Final Commit: —",
        &format!("Task Final Commit: {head}"),
    );
    fs::write(&prompt, &prompt_content).unwrap();

    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", ".dev/plans/continuity.prompt.md"])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "update task final commit"])
        .status()
        .unwrap()
        .success());

    let fixture_dir = ensure_fixture_executor();
    let user_home = TempDir::new().unwrap();
    let cfg_dir = user_home.path().join(".gal/config");
    fs::create_dir_all(&cfg_dir).unwrap();
    let routing_json = r#"{
        "executorRouting": {
            "executors": { "agy": "gemini-2.5-pro" },
            "pipeline": {
                "TESTER": { "executor": "agy" },
                "AUDITOR": { "executor": "agy" },
                "CODER": { "executor": "agy" }
            }
        }
    }"#;
    fs::write(cfg_dir.join("config.json"), routing_json).unwrap();

    let mut paths = vec![fixture_dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let new_path = std::env::join_paths(paths).unwrap();

    let home_str = user_home.path().as_os_str();
    let key_str = std::ffi::OsStr::new("mock-gemini-key");

    let base_envs: [(&str, &std::ffi::OsStr); 4] = [
        ("USERPROFILE", home_str),
        ("HOME", home_str),
        ("PATH", &new_path),
        ("GEMINI_API_KEY", key_str),
    ];

    let root = repo.path().join(".dev/pipeline/continuity");
    fs::create_dir_all(&root).unwrap();
    let coordinator_path = root.join("coordinator.json");
    let prompt_hash = hash(&fs::read(&prompt).unwrap());

    let task_log_dir = root.join(&task_id);
    let count_attempt_logs = || -> usize {
        if !task_log_dir.exists() {
            return 0;
        }
        fs::read_dir(&task_log_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".log"))
            .count()
    };

    let mut state = CoordinatorState::new_legacy("continuity", &prompt_hash, &task_id, "converge");
    state.profile = ContinuationProfile::CodexStopV1;
    bind_fixture(&mut state, &root);
    state.revision = 1;
    state.next_action = NextAction::StartTask {
        task_id: task_id.clone(),
        phase: "converge".into(),
    };
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let prompt_rel = PathBuf::from(".dev/plans/continuity.prompt.md");

    let get_surface_hashes = || -> Vec<(PathBuf, String)> {
        vec![
            (prompt.clone(), hash(&fs::read(&prompt).unwrap())),
            (source_plan.clone(), hash(&fs::read(&source_plan).unwrap())),
            (state_md.clone(), hash(&fs::read(&state_md).unwrap())),
        ]
    };

    // 1. Gate 1 failure: converge_check fails (task is unchecked, cursor not cleared)
    // Verify: stops on first failing gate, Gate 2 does not run, surfaces remain unchanged
    let surfaces_before_gate1_fail = get_surface_hashes();
    let converge_receipt = root.join(format!("{task_id}-converge.receipt.md"));
    let boundary_receipt = root.join(format!("{task_id}-state-boundary.receipt.md"));

    let out_gate1_fail = run_with_env(
        repo.path(),
        &prompt_rel,
        &["--resume", "--task", &task_id, "--phase", "converge"],
        &base_envs,
    );
    assert_ne!(
        out_gate1_fail.status.code(),
        Some(0),
        "converge segment must fail closed when converge_check fails"
    );
    assert!(
        converge_receipt.exists(),
        "converge_check receipt must be written"
    );
    let converge_content = fs::read_to_string(&converge_receipt).unwrap();
    assert!(
        converge_content.contains("overall: fail"),
        "converge_check receipt must show fail: {converge_content}"
    );
    assert!(
        !boundary_receipt.exists(),
        "boundary_check must not run when converge_check fails (gate order stop on first failure)"
    );

    let surfaces_after_gate1_fail = get_surface_hashes();
    assert_eq!(
        surfaces_before_gate1_fail, surfaces_after_gate1_fail,
        "surfaces must remain unchanged when converge_check fails"
    );
    assert_eq!(
        count_attempt_logs(),
        0,
        "zero provider starts during convergence gate 1 failure"
    );
    assert!(
        !root.join("task-cursor.json").exists(),
        "no unauthorized cursor write on gate 1 failure"
    );
    assert!(
        !root.join("progress.json").exists(),
        "no unauthorized progress write on gate 1 failure"
    );

    // 2. Gate 2 failure: converge_check passes, boundary_check --boundary-kind state-recording fails
    // Update prompt and source plan so converge_check bookkeeping checks pass
    let prompt_checked = prompt_content
        .replace(
            &format!("- [ ] {task_id} — Task goal description."),
            &format!("- [x] {task_id} — Task goal description. *({head})*"),
        )
        .replace(&format!("Current Task: {task_id}"), "Current Task: —");
    fs::write(&prompt, &prompt_checked).unwrap();

    let source_checked = format!(
        "# Plan: Continuity\n\n## Tasks\n\n- [x] {task_id} — Task goal description. *({head})*\n"
    );
    fs::write(&source_plan, &source_checked).unwrap();

    // Commit changes to git so head matches
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", ".dev/plans/"])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "check task and clear cursor"])
        .status()
        .unwrap()
        .success());

    let new_head_out = Command::new("git")
        .current_dir(repo.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let new_head = String::from_utf8_lossy(&new_head_out.stdout)
        .trim()
        .to_string();

    // Update commit hash note to new_head in both files
    let prompt_checked = prompt_checked.replace(&head, &new_head);
    fs::write(&prompt, &prompt_checked).unwrap();
    let source_checked = source_checked.replace(&head, &new_head);
    fs::write(&source_plan, &source_checked).unwrap();

    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-a", "-m", "update commit note"])
        .status()
        .unwrap()
        .success());

    // Introduce an uncommitted file outside the state-recording boundary
    let outside_file = repo.path().join("crates/pipeline/src/outside.txt");
    fs::write(
        &outside_file,
        b"unauthorized change outside state-recording boundary",
    )
    .unwrap();

    let surfaces_before_gate2_fail = get_surface_hashes();

    // Reset coordinator for gate 2 test
    let updated_prompt_hash = hash(&fs::read(&prompt).unwrap());
    state.prompt_sha256 = updated_prompt_hash.clone();
    state.revision = 10;
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let out_gate2_fail = run_with_env(
        repo.path(),
        &prompt_rel,
        &["--resume", "--task", &task_id, "--phase", "converge"],
        &base_envs,
    );
    assert_ne!(
        out_gate2_fail.status.code(),
        Some(0),
        "converge segment must fail closed when boundary_check fails"
    );
    assert!(
        converge_receipt.exists(),
        "converge_check receipt must exist"
    );
    let converge_pass_content = fs::read_to_string(&converge_receipt).unwrap();
    assert!(
        converge_pass_content.contains("overall: pass"),
        "converge_check must pass: {converge_pass_content}"
    );
    assert!(
        boundary_receipt.exists(),
        "boundary_check receipt must exist (ran second)"
    );
    let boundary_fail_content = fs::read_to_string(&boundary_receipt).unwrap();
    assert!(
        boundary_fail_content.contains("overall: fail"),
        "boundary_check must fail due to outside file: {boundary_fail_content}"
    );

    let surfaces_after_gate2_fail = get_surface_hashes();
    assert_eq!(
        surfaces_before_gate2_fail, surfaces_after_gate2_fail,
        "surfaces must remain unchanged when boundary_check fails"
    );
    assert_eq!(
        count_attempt_logs(),
        0,
        "zero provider starts during convergence gate 2 failure"
    );
    assert!(
        !root.join("task-cursor.json").exists(),
        "no unauthorized cursor write on gate 2 failure"
    );
    assert!(
        !root.join("progress.json").exists(),
        "no unauthorized progress write on gate 2 failure"
    );

    // Clean up outside file so git working copy is clean for state-recording
    fs::remove_file(&outside_file).unwrap();

    // 3. Passing convergence: Gate 1 and Gate 2 both pass
    // When no unchecked task remains, raise GoalBackward checkpoint and pause with AwaitOrchestrator
    state.revision = 20;
    state.prompt_sha256 = hash(&fs::read(&prompt).unwrap());
    fs::write(
        &coordinator_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let surfaces_before_pass = get_surface_hashes();

    let out_pass = run_with_env(
        repo.path(),
        &prompt_rel,
        &["--resume", "--task", &task_id, "--phase", "converge"],
        &base_envs,
    );
    assert_eq!(
        out_pass.status.code(),
        Some(0),
        "passing convergence must exit 0: stderr={}",
        String::from_utf8_lossy(&out_pass.stderr)
    );

    // Both receipts must show pass
    let converge_final = fs::read_to_string(&converge_receipt).unwrap();
    assert!(
        converge_final.contains("overall: pass"),
        "final converge_check must pass: {converge_final}"
    );
    let boundary_final = fs::read_to_string(&boundary_receipt).unwrap();
    assert!(
        boundary_final.contains("overall: pass"),
        "final boundary_check must pass: {boundary_final}"
    );

    // Projection hashes after passing convergence
    let surfaces_after_pass = get_surface_hashes();
    println!("surfaces-before-pass: {surfaces_before_pass:?}");
    println!("surfaces-after-pass: {surfaces_after_pass:?}");
    for (path, h) in &surfaces_after_pass {
        println!("projection-hash: path={} hash={h}", path.display());
    }

    // Coordinator state verification
    let passed_state: CoordinatorState =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert_eq!(
        passed_state.last_verified_evidence.as_deref(),
        Some(format!("converge-check:pass;boundary-state-recording:pass;task:{task_id}").as_str()),
        "evidence must record converge-check and boundary-state-recording pass: {:?}",
        passed_state.last_verified_evidence
    );

    // When no unchecked task remains, GoalBackward checkpoint is raised
    assert!(
        passed_state.checkpoint.is_some(),
        "checkpoint must be raised when no unchecked task remains"
    );
    let chk = passed_state.checkpoint.as_ref().unwrap();
    assert_eq!(
        chk.kind,
        CheckpointKind::GoalBackward,
        "checkpoint kind must be GoalBackward"
    );
    assert_eq!(
        chk.checkpoint_id,
        format!("goal-backward-{task_id}"),
        "checkpoint_id must be goal-backward-{task_id}"
    );
    assert_eq!(
        passed_state.next_action,
        NextAction::AwaitOrchestrator {
            checkpoint_id: format!("goal-backward-{task_id}")
        },
        "next action must be await orchestrator with goal-backward checkpoint"
    );

    // Provider-start count remains 0 throughout convergence
    assert_eq!(
        count_attempt_logs(),
        0,
        "zero provider starts during convergence"
    );
    assert!(
        !root.join("task-cursor.json").exists(),
        "no unauthorized cursor write"
    );
    assert!(
        !root.join("progress.json").exists(),
        "no unauthorized progress write"
    );
}
