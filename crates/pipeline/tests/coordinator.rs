use dispatch::host_attempt_state::HostTerminalState;
use pipeline::coordinator::ResumeHostSubagent;
use pipeline::coordinator::{
    ActiveAttempt, AttemptOwnership, CheckpointKind, CheckpointReceipt, ContinuationProfile,
    CoordinatorError, CoordinatorState, ExecutionBinding, NextAction, OwnerObservation, RetryState,
};
use pipeline::usage::binding::{RunBindingOutcome, RunIdentity, RUN_MANIFEST_VERSION};

fn host_checkpoint() -> (CoordinatorState, ResumeHostSubagent) {
    let mut state = CoordinatorState::new_legacy("plan", "prompt", "task", "implement");
    state.activate_codex_stop_v1("activation".into()).unwrap();
    let execution_binding_sha256 = bind(&mut state);
    state.pinned_host = Some("codex".into());
    state.pinned_mode = Some("host-subagent".into());
    state.claim_attempt("host-attempt".into()).unwrap();
    state.bind_process(1234, Some("opener".into())).unwrap();
    state
        .checkpoint_host_subagent(
            "host-checkpoint".into(),
            "run".into(),
            Some("commit".into()),
        )
        .unwrap();
    let request = ResumeHostSubagent {
        checkpoint_id: "host-checkpoint".into(),
        expected_revision: state.revision,
        run_id: "run".into(),
        task_id: "task".into(),
        phase: "implement".into(),
        attempt_id: "host-attempt".into(),
        session_ref: "codex:child-session".into(),
        prompt_sha256: "prompt".into(),
        execution_binding_sha256,
        commit: Some("commit".into()),
    };
    (state, request)
}

#[test]
fn coordinator_host_subagent_waits_after_opener_exit() {
    let (mut state, _) = host_checkpoint();
    let before = state.clone();
    for verdict in [
        AttemptOwnership::Dead,
        AttemptOwnership::Unknown,
        AttemptOwnership::Live,
    ] {
        assert_eq!(
            state.recover_owner(OwnerObservation {
                verdict,
                observed_pid: None,
                observed_birth: None,
            }),
            Ok(AttemptOwnership::NotApplicable)
        );
        assert_eq!(state, before);
    }
    assert!(state.active_attempt.is_some());
    assert!(matches!(
        state.next_action,
        NextAction::AwaitOrchestrator { .. }
    ));
}

#[test]
fn coordinator_host_subagent_rejects_mismatched_resume_bindings() {
    let (state, request) = host_checkpoint();
    let original = serde_json::to_value(&request).unwrap();
    for field in [
        "checkpoint_id",
        "run_id",
        "task_id",
        "phase",
        "attempt_id",
        "session_ref",
        "prompt_sha256",
        "execution_binding_sha256",
        "commit",
    ] {
        let mut altered = original.clone();
        altered[field] = serde_json::json!("wrong");
        assert!(
            state
                .resume_host_subagent(serde_json::from_value(altered).unwrap())
                .is_err(),
            "{field}"
        );
    }
    for revision in [request.expected_revision - 1, request.expected_revision + 1] {
        let mut altered = request.clone();
        altered.expected_revision = revision;
        assert!(state.resume_host_subagent(altered).is_err());
    }
    for field in [
        "task_id",
        "phase",
        "prompt_sha256",
        "pinned_host",
        "pinned_mode",
    ] {
        let mut altered = serde_json::to_value(&state).unwrap();
        altered[field] = serde_json::json!("wrong");
        let altered: CoordinatorState = serde_json::from_value(altered).unwrap();
        assert!(
            altered.resume_host_subagent(request.clone()).is_err(),
            "{field}"
        );
    }
    let mut altered = state.clone();
    altered
        .execution_binding
        .as_mut()
        .unwrap()
        .executable_sha256 = "changed".into();
    assert!(altered.resume_host_subagent(request).is_err());
}

#[test]
fn coordinator_host_subagent_rejects_missing_or_stale_checkpoint_identity() {
    let (state, request) = host_checkpoint();
    let identity = state.host_subagent.as_ref().unwrap();
    let checkpoint = state.checkpoint.as_ref().unwrap();
    assert_eq!(identity.checkpoint_id, checkpoint.checkpoint_id);
    assert_eq!(identity.checkpoint_revision, checkpoint.revision);
    let serialized = serde_json::to_value(&state).unwrap();
    assert!(serialized["checkpoint"].get("host_subagent").is_none());

    let mut missing = serialized.clone();
    missing.as_object_mut().unwrap().remove("host_subagent");
    let missing: CoordinatorState = serde_json::from_value(missing).unwrap();
    assert_eq!(
        missing.resume_host_subagent(request.clone()).unwrap_err(),
        CoordinatorError::CheckpointMismatch
    );

    // Replacing a checkpoint cannot reuse the previous host identity, even if
    // the replacement reuses its checkpoint ID.
    for reuse_id in [false, true] {
        let mut replaced = state.clone();
        replaced.revision += 1;
        let checkpoint = replaced.checkpoint.as_mut().unwrap();
        checkpoint.revision = replaced.revision;
        if !reuse_id {
            checkpoint.checkpoint_id = "replacement".into();
        }
        let mut request = request.clone();
        request.checkpoint_id = checkpoint.checkpoint_id.clone();
        request.expected_revision = replaced.revision;
        replaced.next_action = NextAction::AwaitOrchestrator {
            checkpoint_id: request.checkpoint_id.clone(),
        };
        assert_eq!(
            replaced.resume_host_subagent(request).unwrap_err(),
            CoordinatorError::CheckpointMismatch
        );
    }
}

#[test]
fn coordinator_host_subagent_rejects_generic_consumption_and_finish() {
    let (mut state, request) = host_checkpoint();
    let before = state.clone();
    assert!(state
        .finish_attempt(&request.attempt_id, Some("ordinary receipt".into()))
        .is_err());
    assert!(state
        .consume_checkpoint_receipt(CheckpointReceipt {
            receipt_id: "generic".into(),
            checkpoint_id: request.checkpoint_id,
            revision: request.expected_revision,
            prompt_sha256: request.prompt_sha256,
            execution_binding_sha256: request.execution_binding_sha256,
            commit: request.commit,
            accepted: true,
        })
        .is_err());
    assert!(state
        .checkpoint("replacement".into(), CheckpointKind::TaskQuality, None)
        .is_err());
    assert_eq!(state, before);
    state.record_retry("ordinary retry".into());
    assert!(matches!(state.next_action, NextAction::Blocked { .. }));
    assert!(state.active_attempt.is_some());
}

#[test]
fn coordinator_host_subagent_consumes_completed_digest_once() {
    let (mut state, request) = host_checkpoint();
    let digest = "a".repeat(64);
    let custody = state.resume_host_subagent(request.clone()).unwrap();
    assert!(state
        .consume_host_subagent_checkpoint(custody, &digest)
        .is_err());
    let custody = state.resume_host_subagent(request.clone()).unwrap();
    let custody = state
        .attach_host_subagent_terminal(custody, HostTerminalState::Completed, digest.clone())
        .unwrap();
    assert!(state.active_attempt.is_some());
    assert!(state
        .checkpoint
        .as_ref()
        .unwrap()
        .consumed_receipt_id
        .is_none());
    state
        .consume_host_subagent_checkpoint(custody, &digest)
        .unwrap();
    assert!(state.active_attempt.is_none());
    assert!(matches!(state.next_action, NextAction::ResumeTask { .. }));
    assert_eq!(state.consumed_checkpoint_receipts.len(), 1);
    assert!(state
        .last_verified_evidence
        .as_ref()
        .unwrap()
        .contains(&digest));
    assert_eq!(
        state.resume_host_subagent(request).unwrap_err(),
        CoordinatorError::ReceiptReplay
    );
}

#[test]
fn coordinator_host_subagent_failed_close_blocks_advancement() {
    for terminal in [
        HostTerminalState::HostSessionInvalid,
        HostTerminalState::NoWriteback,
        HostTerminalState::LeaseOwnerMismatch,
        HostTerminalState::LeaseMissing,
        HostTerminalState::IdentityMismatch,
        HostTerminalState::TerminalReplay,
        HostTerminalState::PlacementFailed,
        HostTerminalState::ModelTierMismatch,
    ] {
        let (mut state, request) = host_checkpoint();
        let custody = state.resume_host_subagent(request.clone()).unwrap();
        assert!(state
            .attach_host_subagent_terminal(custody, terminal, "a".repeat(64))
            .is_err());
        assert!(matches!(state.next_action, NextAction::Blocked { .. }));
        assert_eq!(state.retry.last_failure.as_deref(), Some(terminal.as_str()));
        assert!(state.active_attempt.is_some());
        assert!(state
            .checkpoint
            .as_ref()
            .unwrap()
            .consumed_receipt_id
            .is_none());
        assert!(state.consumed_checkpoint_receipts.is_empty());
        let mut request = request;
        request.expected_revision = state.revision;
        assert!(state.resume_host_subagent(request).is_err());
        let before = state.clone();
        state
            .recover_owner(OwnerObservation {
                verdict: AttemptOwnership::Dead,
                observed_pid: None,
                observed_birth: None,
            })
            .unwrap();
        assert_eq!(state, before);
    }
}

#[test]
fn coordinator_host_subagent_exact_terminal_recovery() {
    let (mut state, mut request) = host_checkpoint();
    let digest = "b".repeat(64);
    // A terminal publication before coordinator persistence is attached after reload.
    let mut store = MemoryCoordinatorStore::new();
    store.save(&state).unwrap();
    state = store.reload().unwrap();
    let custody = state.resume_host_subagent(request.clone()).unwrap();
    state
        .attach_host_subagent_terminal(custody, HostTerminalState::Completed, digest.clone())
        .unwrap();
    // An interruption after attachment recovers the same session and digest.
    store.save(&state).unwrap();
    state = store.reload().unwrap();
    assert!(state.resume_host_subagent(request.clone()).is_err());
    request.expected_revision = state.revision;
    let before = state.clone();
    let custody = state.resume_host_subagent(request.clone()).unwrap();
    assert!(state
        .consume_host_subagent_checkpoint(custody, &"c".repeat(64))
        .is_err());
    let custody = state.resume_host_subagent(request.clone()).unwrap();
    assert!(state
        .attach_host_subagent_terminal(custody, HostTerminalState::Completed, "c".repeat(64))
        .is_err());
    assert_eq!(state, before);
    let mut wrong_session = request.clone();
    wrong_session.session_ref = "codex:another-child".into();
    assert!(state.resume_host_subagent(wrong_session).is_err());
    let custody = state.resume_host_subagent(request.clone()).unwrap();
    let custody = state
        .attach_host_subagent_terminal(custody, HostTerminalState::Completed, digest.clone())
        .unwrap();
    state
        .consume_host_subagent_checkpoint(custody, &digest)
        .unwrap();
    store.save(&state).unwrap();
    let reloaded = store.reload().unwrap();
    assert_eq!(
        reloaded.resume_host_subagent(request).unwrap_err(),
        CoordinatorError::ReceiptReplay
    );
}

#[test]
fn coordinator_host_subagent_rejects_stale_custody_and_invalid_digest() {
    let (mut state, request) = host_checkpoint();
    let before = state.clone();
    assert!(state.bind_process(9999, None).is_err());
    assert_eq!(state, before);
    for digest in [String::new(), "a".repeat(63), "z".repeat(64)] {
        let custody = state.resume_host_subagent(request.clone()).unwrap();
        assert!(state
            .attach_host_subagent_terminal(custody, HostTerminalState::Completed, digest)
            .is_err());
        assert_eq!(state, before);
    }
    let stale = state.resume_host_subagent(request.clone()).unwrap();
    let current = state.resume_host_subagent(request).unwrap();
    let digest = "a".repeat(64);
    let current = state
        .attach_host_subagent_terminal(current, HostTerminalState::Completed, digest.clone())
        .unwrap();
    let attached = state.clone();
    assert!(state
        .consume_host_subagent_checkpoint(stale, &digest)
        .is_err());
    assert_eq!(state, attached);
    state
        .consume_host_subagent_checkpoint(current, &digest)
        .unwrap();
}

#[test]
fn coordinator_host_subagent_legacy_state_defaults_and_open_requirements() {
    let state = CoordinatorState::new_legacy("plan", "prompt", "task", "implement");
    let mut json = serde_json::to_value(&state).unwrap();
    json.as_object_mut().unwrap().remove("pinned_host");
    json.as_object_mut().unwrap().remove("pinned_mode");
    json.as_object_mut().unwrap().remove("host_subagent");
    let mut loaded: CoordinatorState = serde_json::from_value(json).unwrap();
    assert_eq!(state, loaded);
    assert!(loaded
        .checkpoint_host_subagent("checkpoint".into(), "run".into(), None)
        .is_err());
    let (mut state, _) = host_checkpoint();
    state.checkpoint = None;
    state.pinned_mode = Some("routed".into());
    assert!(state
        .checkpoint_host_subagent("checkpoint".into(), "run".into(), None)
        .is_err());
    state.pinned_mode = Some("host-subagent".into());
    assert!(state
        .checkpoint("checkpoint".into(), CheckpointKind::HostSubagent, None)
        .is_err());
}

fn bind(state: &mut CoordinatorState) -> String {
    state
        .guarded_entry(ExecutionBinding {
            version: ExecutionBinding::VERSION,
            binding_scope: "source-worktree".into(),
            worktree_root: "C:/work/tree".into(),
            executable_path: "C:/work/tree/target/gal-pipeline/bin/abc/gal.exe".into(),
            executable_sha256: "abc123".into(),
        })
        .expect("guarded entry binding")
}

/// In-memory persistence double simulating atomic coordinator state persistence.
struct MemoryCoordinatorStore {
    data: Option<Vec<u8>>,
}

impl MemoryCoordinatorStore {
    fn new() -> Self {
        Self { data: None }
    }

    fn save(&mut self, state: &CoordinatorState) -> Result<usize, String> {
        let serialized =
            serde_json::to_vec_pretty(state).map_err(|e| format!("serialization error: {e}"))?;
        let len = serialized.len();
        self.data = Some(serialized);
        Ok(len)
    }

    fn reload(&self) -> Result<CoordinatorState, String> {
        let bytes = self
            .data
            .as_ref()
            .ok_or_else(|| "no persisted data in store".to_string())?;
        serde_json::from_slice(bytes).map_err(|e| format!("deserialization error: {e}"))
    }
}

#[test]
fn test_in_memory_persistence_double_serialization_reload_equivalence() {
    let mut store = MemoryCoordinatorStore::new();

    // 1. Initial legacy state (rev 0)
    let mut state = CoordinatorState::new_legacy(
        "fix-pipeline-orchestration-continuity",
        "sha256-prompt-abc1234567890",
        "T-EV",
        "implement",
    );
    assert_eq!(state.revision, 0);
    assert_eq!(state.profile, ContinuationProfile::LegacyInteractive);

    let bytes_len = store.save(&state).expect("save legacy state");
    assert!(bytes_len > 0);
    let reloaded = store.reload().expect("reload legacy state");
    assert_eq!(state, reloaded);

    // 2. Activate CodexStopV1 (rev 1)
    state
        .activate_codex_stop_v1("activation-digest-v1-token".into())
        .expect("activate codex stop v1");
    let binding_digest = bind(&mut state);
    assert_eq!(state.revision, 2);
    assert_eq!(state.profile, ContinuationProfile::CodexStopV1);
    assert_eq!(
        state.activation_digest,
        Some("activation-digest-v1-token".into())
    );

    store.save(&state).expect("save activated state");
    let reloaded = store.reload().expect("reload activated state");
    assert_eq!(state, reloaded);

    // 3. Claim attempt (rev 3)
    let rev = state
        .claim_attempt("attempt-001".into())
        .expect("claim attempt");
    assert_eq!(rev, 3);
    assert_eq!(state.revision, 3);
    assert_eq!(
        state.next_action,
        NextAction::ResumeTask {
            task_id: "T-EV".into(),
            phase: "implement".into(),
        }
    );

    store.save(&state).expect("save claimed state");
    let reloaded = store.reload().expect("reload claimed state");
    assert_eq!(state, reloaded);

    // 4. Bind process (rev 4)
    let rev = state
        .bind_process(4242, Some("2026-09-29T00:00:00Z".into()))
        .expect("bind process");
    assert_eq!(rev, 4);
    assert_eq!(state.revision, 4);
    assert_eq!(
        state.active_attempt,
        Some(ActiveAttempt {
            attempt_id: "attempt-001".into(),
            task_id: "T-EV".into(),
            phase: "implement".into(),
            process_id: Some(4242),
            process_birth: Some("2026-09-29T00:00:00Z".into()),
            dispatch_claimed: true,
        })
    );

    store.save(&state).expect("save bound process state");
    let reloaded = store.reload().expect("reload bound process state");
    assert_eq!(state, reloaded);

    // 5. Finish attempt (rev 5)
    let rev = state
        .finish_attempt("attempt-001", Some("evidence-receipt-001".into()))
        .expect("finish attempt");
    assert_eq!(rev, 5);
    assert!(state.active_attempt.is_none());
    assert_eq!(
        state.last_verified_evidence,
        Some("evidence-receipt-001".into())
    );

    store.save(&state).expect("save finished attempt state");
    let reloaded = store.reload().expect("reload finished attempt state");
    assert_eq!(state, reloaded);

    // 6. Checkpoint creation (rev 6)
    let rev = state
        .checkpoint(
            "chk-tp05-task-quality".into(),
            CheckpointKind::TaskQuality,
            Some("d3a60ae4abf86f8950449fedfa82ac7bd4372c6e".into()),
        )
        .expect("create checkpoint");
    assert_eq!(rev, 6);
    assert_eq!(
        state.checkpoint.as_ref().unwrap().execution_binding_sha256,
        binding_digest
    );
    assert_eq!(
        state.next_action,
        NextAction::AwaitOrchestrator {
            checkpoint_id: "chk-tp05-task-quality".into(),
        }
    );

    store.save(&state).expect("save checkpoint state");
    let reloaded = store.reload().expect("reload checkpoint state");
    assert_eq!(state, reloaded);

    // 7. Checkpoint receipt consumption (rev 7)
    let receipt = CheckpointReceipt {
        receipt_id: "rcpt-tp05-001".into(),
        checkpoint_id: "chk-tp05-task-quality".into(),
        revision: 6,
        prompt_sha256: "sha256-prompt-abc1234567890".into(),
        execution_binding_sha256: binding_digest,
        commit: Some("d3a60ae4abf86f8950449fedfa82ac7bd4372c6e".into()),
        accepted: true,
    };
    let rev = state
        .consume_checkpoint_receipt(receipt)
        .expect("consume checkpoint receipt");
    assert_eq!(rev, 7);
    assert_eq!(
        state.consumed_checkpoint_receipts,
        vec!["rcpt-tp05-001".to_string()]
    );
    assert_eq!(
        state.next_action,
        NextAction::ResumeTask {
            task_id: "T-EV".into(),
            phase: "implement".into(),
        }
    );

    store.save(&state).expect("save consumed receipt state");
    let reloaded = store.reload().expect("reload consumed receipt state");
    assert_eq!(state, reloaded);
}

#[test]
fn test_owner_classification_live_dead_unknown() {
    let mut state = CoordinatorState::new_legacy("plan-1", "sha256-1", "T-EV", "implement");
    state
        .activate_codex_stop_v1("token".into())
        .expect("activate");
    bind(&mut state);
    state.claim_attempt("att-1".into()).expect("claim");
    state
        .bind_process(1234, Some("birth-token-xyz".into()))
        .expect("bind");

    // Case 1: Live owner matches PID and birth identity
    let live_obs = OwnerObservation {
        verdict: AttemptOwnership::Live,
        observed_pid: Some(1234),
        observed_birth: Some("birth-token-xyz".into()),
    };
    let verdict = state
        .recover_owner(live_obs)
        .expect("recover live matching owner");
    assert_eq!(verdict, AttemptOwnership::Live);
    assert_eq!(
        state.next_action,
        NextAction::ResumeTask {
            task_id: "T-EV".into(),
            phase: "implement".into(),
        }
    );

    // Case 2: Observed Live but PID mismatch -> Unknown (PID reuse or different process)
    let pid_mismatch_obs = OwnerObservation {
        verdict: AttemptOwnership::Live,
        observed_pid: Some(5678),
        observed_birth: Some("birth-token-xyz".into()),
    };
    let verdict = state
        .recover_owner(pid_mismatch_obs)
        .expect("recover pid mismatch");
    assert_eq!(verdict, AttemptOwnership::Unknown);
    assert_eq!(
        state.next_action,
        NextAction::Blocked {
            reason: "attempt-owner-unknown".into(),
        }
    );

    // Case 3: Observed Live but birth mismatch -> Unknown
    let birth_mismatch_obs = OwnerObservation {
        verdict: AttemptOwnership::Live,
        observed_pid: Some(1234),
        observed_birth: Some("different-birth-epoch".into()),
    };
    let verdict = state
        .recover_owner(birth_mismatch_obs)
        .expect("recover birth mismatch");
    assert_eq!(verdict, AttemptOwnership::Unknown);
    assert_eq!(
        state.next_action,
        NextAction::Blocked {
            reason: "attempt-owner-unknown".into(),
        }
    );

    // Case 4: Dead owner -> RecoverAttempt
    let dead_obs = OwnerObservation {
        verdict: AttemptOwnership::Dead,
        observed_pid: None,
        observed_birth: None,
    };
    let verdict = state.recover_owner(dead_obs).expect("recover dead owner");
    assert_eq!(verdict, AttemptOwnership::Dead);
    assert_eq!(
        state.next_action,
        NextAction::RecoverAttempt {
            attempt_id: "att-1".into(),
        }
    );

    // Case 5: Direct Unknown verdict -> Blocked without killing or redispatching
    let unknown_obs = OwnerObservation {
        verdict: AttemptOwnership::Unknown,
        observed_pid: None,
        observed_birth: None,
    };
    let verdict = state
        .recover_owner(unknown_obs)
        .expect("recover unknown owner");
    assert_eq!(verdict, AttemptOwnership::Unknown);
    assert_eq!(
        state.next_action,
        NextAction::Blocked {
            reason: "attempt-owner-unknown".into(),
        }
    );

    // Case 6: NotApplicable returns InvalidTransition
    let na_obs = OwnerObservation {
        verdict: AttemptOwnership::NotApplicable,
        observed_pid: None,
        observed_birth: None,
    };
    let err = state
        .recover_owner(na_obs)
        .expect_err("not applicable should error");
    assert_eq!(
        err,
        CoordinatorError::InvalidTransition("owner observation is not applicable")
    );
}

#[test]
fn test_no_duplicate_dispatch() {
    let mut state = CoordinatorState::new_legacy("plan-1", "sha256-1", "T-EV", "implement");
    state
        .activate_codex_stop_v1("token".into())
        .expect("activate");
    bind(&mut state);

    // First claim succeeds
    state.claim_attempt("att-1".into()).expect("first claim");

    // Second claim without finishing the first must fail with DispatchAlreadyClaimed
    let err = state
        .claim_attempt("att-2".into())
        .expect_err("second claim must fail");
    assert_eq!(err, CoordinatorError::DispatchAlreadyClaimed);

    // Check after persistence double round-trip: still prevents duplicate dispatch
    let mut store = MemoryCoordinatorStore::new();
    store.save(&state).expect("save state");
    let mut reloaded = store.reload().expect("reload state");

    let err2 = reloaded
        .claim_attempt("att-3".into())
        .expect_err("claim on reloaded active attempt must fail");
    assert_eq!(err2, CoordinatorError::DispatchAlreadyClaimed);

    // Finishing att-1 clears the active attempt and permits the next claim
    reloaded
        .finish_attempt("att-1", Some("receipt-1".into()))
        .expect("finish att-1");
    reloaded
        .claim_attempt("att-4".into())
        .expect("claim after finish must succeed");
}

#[test]
fn test_checkpoint_replay_rejection_and_binding_validation() {
    let mut state = CoordinatorState::new_legacy("plan-1", "sha256-prompt-val", "T-EV", "audit");

    // Checkpoint requires CodexStopV1 profile
    let err = state
        .checkpoint("chk-1".into(), CheckpointKind::BoundaryWidening, None)
        .expect_err("checkpoint in legacy mode must fail");
    assert_eq!(
        err,
        CoordinatorError::InvalidTransition("semantic checkpoint requires CodexStopV1")
    );

    state
        .activate_codex_stop_v1("token".into())
        .expect("activate");
    bind(&mut state);
    let chk_rev = state
        .checkpoint(
            "chk-1".into(),
            CheckpointKind::BoundaryWidening,
            Some("commit-aaa".into()),
        )
        .expect("checkpoint in codex stop v1");
    assert_eq!(chk_rev, 3);

    // 1. Checkpoint ID mismatch
    let err = state
        .consume_checkpoint_receipt(CheckpointReceipt {
            receipt_id: "rcpt-1".into(),
            checkpoint_id: "chk-wrong".into(),
            revision: chk_rev,
            prompt_sha256: "sha256-prompt-val".into(),
            execution_binding_sha256: state.execution_binding.as_ref().unwrap().digest().unwrap(),
            commit: Some("commit-aaa".into()),
            accepted: true,
        })
        .expect_err("wrong checkpoint id");
    assert_eq!(err, CoordinatorError::CheckpointMismatch);

    // 2. Revision mismatch
    let err = state
        .consume_checkpoint_receipt(CheckpointReceipt {
            receipt_id: "rcpt-1".into(),
            checkpoint_id: "chk-1".into(),
            revision: chk_rev + 1,
            prompt_sha256: "sha256-prompt-val".into(),
            execution_binding_sha256: state.execution_binding.as_ref().unwrap().digest().unwrap(),
            commit: Some("commit-aaa".into()),
            accepted: true,
        })
        .expect_err("wrong revision");
    assert_eq!(err, CoordinatorError::CheckpointMismatch);

    // 3. Prompt SHA mismatch
    let err = state
        .consume_checkpoint_receipt(CheckpointReceipt {
            receipt_id: "rcpt-1".into(),
            checkpoint_id: "chk-1".into(),
            revision: chk_rev,
            prompt_sha256: "sha256-wrong".into(),
            execution_binding_sha256: state.execution_binding.as_ref().unwrap().digest().unwrap(),
            commit: Some("commit-aaa".into()),
            accepted: true,
        })
        .expect_err("wrong prompt sha");
    assert_eq!(err, CoordinatorError::ReceiptBindingMismatch);

    // 4. Commit mismatch
    let err = state
        .consume_checkpoint_receipt(CheckpointReceipt {
            receipt_id: "rcpt-1".into(),
            checkpoint_id: "chk-1".into(),
            revision: chk_rev,
            prompt_sha256: "sha256-prompt-val".into(),
            execution_binding_sha256: state.execution_binding.as_ref().unwrap().digest().unwrap(),
            commit: Some("commit-wrong".into()),
            accepted: true,
        })
        .expect_err("wrong commit");
    assert_eq!(err, CoordinatorError::ReceiptBindingMismatch);

    // 5. Successful receipt consumption
    let valid_receipt = CheckpointReceipt {
        receipt_id: "rcpt-1".into(),
        checkpoint_id: "chk-1".into(),
        revision: chk_rev,
        prompt_sha256: "sha256-prompt-val".into(),
        execution_binding_sha256: state.execution_binding.as_ref().unwrap().digest().unwrap(),
        commit: Some("commit-aaa".into()),
        accepted: true,
    };
    state
        .consume_checkpoint_receipt(valid_receipt.clone())
        .expect("consume valid receipt");

    // 6. Replay identical receipt ID
    let err = state
        .consume_checkpoint_receipt(valid_receipt)
        .expect_err("replay receipt id");
    assert_eq!(err, CoordinatorError::ReceiptReplay);

    // 7. Attempt new receipt ID for already consumed checkpoint
    let different_receipt = CheckpointReceipt {
        receipt_id: "rcpt-2".into(),
        checkpoint_id: "chk-1".into(),
        revision: chk_rev,
        prompt_sha256: "sha256-prompt-val".into(),
        execution_binding_sha256: state.execution_binding.as_ref().unwrap().digest().unwrap(),
        commit: Some("commit-aaa".into()),
        accepted: true,
    };
    let err = state
        .consume_checkpoint_receipt(different_receipt)
        .expect_err("replay on consumed checkpoint");
    assert_eq!(err, CoordinatorError::ReceiptReplay);
}

#[test]
fn test_bounded_large_input_status() {
    // Simulate a 160 KB prompt hash / text and a 500 KB log failure
    let large_prompt = "p".repeat(160 * 1024);
    let large_log = "l".repeat(500 * 1024);
    let large_task_id = "T-".to_string() + &"9".repeat(10 * 1024);
    let large_phase = "phase-".to_string() + &"x".repeat(5 * 1024);

    let mut state = CoordinatorState::new_legacy(
        "plan-scope-large",
        large_prompt,
        large_task_id.clone(),
        large_phase.clone(),
    );
    state
        .activate_codex_stop_v1("tok".into())
        .expect("activate");

    // 1. Initial StartTask status with huge task/phase
    let status = state.bounded_status();
    assert!(status.len() <= 512, "status length {} > 512", status.len());
    assert!(status.contains("profile=CodexStopV1"));

    // 2. Blocked status with 500 KB failure reason
    state.next_action = NextAction::Blocked {
        reason: large_log.clone(),
    };
    let status = state.bounded_status();
    assert!(status.len() <= 512, "status length {} > 512", status.len());
    assert!(status.contains("blocked:"));

    // 3. AwaitOrchestrator status with large checkpoint id
    state.next_action = NextAction::AwaitOrchestrator {
        checkpoint_id: "chk-".to_string() + &"c".repeat(10 * 1024),
    };
    let status = state.bounded_status();
    assert!(status.len() <= 512, "status length {} > 512", status.len());
    assert!(status.contains("await:"));

    // 4. RecoverAttempt status with large attempt id
    state.next_action = NextAction::RecoverAttempt {
        attempt_id: "att-".to_string() + &"a".repeat(10 * 1024),
    };
    let status = state.bounded_status();
    assert!(status.len() <= 512, "status length {} > 512", status.len());
    assert!(status.contains("recover:"));

    // 5. RetryTask status
    state.next_action = NextAction::RetryTask {
        task_id: large_task_id,
        phase: large_phase,
        attempt: 3,
    };
    let status = state.bounded_status();
    assert!(status.len() <= 512, "status length {} > 512", status.len());
    assert!(status.contains("retry:"));
}

#[test]
fn test_retry_state_and_orchestrator_rejection() {
    let mut state = CoordinatorState::new_legacy("plan-1", "prompt-sha", "T-EV", "test");
    state
        .activate_codex_stop_v1("token".into())
        .expect("activate");
    state.retry = RetryState {
        attempt: 0,
        max_attempts: 2,
        last_failure: None,
    };

    // First retry increment
    let rev1 = state.record_retry("first failure".into());
    assert_eq!(state.retry.attempt, 1);
    assert_eq!(
        state.next_action,
        NextAction::RetryTask {
            task_id: "T-EV".into(),
            phase: "test".into(),
            attempt: 1,
        }
    );
    assert_eq!(rev1, state.revision);

    // Second retry reaches max_attempts (2) -> Blocked
    let rev2 = state.record_retry("second failure".into());
    assert_eq!(state.retry.attempt, 2);
    assert_eq!(
        state.next_action,
        NextAction::Blocked {
            reason: "retry-limit-exhausted".into(),
        }
    );
    assert_eq!(rev2, state.revision);

    // Orchestrator checkpoint rejection triggers retry
    let mut state2 = CoordinatorState::new_legacy("plan-2", "prompt-sha", "T-EV", "test");
    state2
        .activate_codex_stop_v1("token".into())
        .expect("activate");
    bind(&mut state2);
    let chk_rev = state2
        .checkpoint(
            "chk-rejected".into(),
            CheckpointKind::GoalBackward,
            Some("commit-1".into()),
        )
        .expect("chk");

    let rejection_receipt = CheckpointReceipt {
        receipt_id: "rcpt-rej-1".into(),
        checkpoint_id: "chk-rejected".into(),
        revision: chk_rev,
        prompt_sha256: "prompt-sha".into(),
        execution_binding_sha256: state2.execution_binding.as_ref().unwrap().digest().unwrap(),
        commit: Some("commit-1".into()),
        accepted: false,
    };

    state2
        .consume_checkpoint_receipt(rejection_receipt)
        .expect("consume rejection receipt");
    assert_eq!(
        state2.retry.last_failure,
        Some("orchestrator-rejected-checkpoint".into())
    );
    assert_eq!(
        state2.next_action,
        NextAction::RetryTask {
            task_id: "T-EV".into(),
            phase: "test".into(),
            attempt: 1,
        }
    );
}

#[test]
fn test_binding_digest_migration_and_safe_rebind() {
    let mut state = CoordinatorState::new_legacy("plan", "prompt", "T-1", "implement");
    state.schema_version = 1;
    state.migrate_schema().expect("migrate legacy schema");
    assert_eq!(state.schema_version, CoordinatorState::SCHEMA_VERSION);
    let digest = bind(&mut state);
    assert_eq!(
        digest,
        "f2fb1db205ea28f8aa37fa0b16dd5a116d4d69f36272d9c2dfd399af67cb0f5f"
    );
    let binding_json = serde_json::to_string(state.execution_binding.as_ref().unwrap()).unwrap();
    assert_eq!(
        binding_json,
        r#"{"version":1,"binding_scope":"source-worktree","worktree_root":"C:/work/tree","executable_path":"C:/work/tree/target/gal-pipeline/bin/abc/gal.exe","executable_sha256":"abc123"}"#
    );

    state.claim_attempt("active".into()).unwrap();
    let changed = ExecutionBinding {
        executable_sha256: "def456".into(),
        ..state.execution_binding.clone().unwrap()
    };
    assert_eq!(
        state.guarded_entry(changed.clone()),
        Err(CoordinatorError::UnsafeRebind)
    );
    state.finish_attempt("active", None).unwrap();
    let changed_digest = state.guarded_entry(changed).expect("safe segment rebind");
    assert_ne!(digest, changed_digest);
}

#[test]
fn test_active_state_without_binding_fails_closed() {
    let mut state = CoordinatorState::new_legacy("plan", "prompt", "T-1", "implement");
    state
        .activate_codex_stop_v1("token".into())
        .expect("activate");
    bind(&mut state);
    state.claim_attempt("active".into()).expect("claim");

    let mut serialized = serde_json::to_value(&state).expect("serialize active state");
    serialized
        .as_object_mut()
        .expect("state object")
        .remove("execution_binding");
    let mut unbound: CoordinatorState =
        serde_json::from_value(serialized).expect("load legacy active state");

    let attempted_binding = ExecutionBinding {
        version: ExecutionBinding::VERSION,
        binding_scope: "source-worktree".into(),
        worktree_root: "C:/work/tree".into(),
        executable_path: "C:/work/tree/target/gal-pipeline/bin/new/gal.exe".into(),
        executable_sha256: "def456".into(),
    };
    assert!(unbound.guarded_entry(attempted_binding).is_err());
    assert!(unbound.execution_binding.is_none());
    assert!(unbound.active_attempt.is_some());
}

#[test]
fn test_malformed_binding_is_rejected_at_guarded_entry() {
    let mut state = CoordinatorState::new_legacy("plan", "prompt", "T-1", "implement");
    let mut malformed = ExecutionBinding {
        version: u32::MAX,
        binding_scope: "source-worktree".into(),
        worktree_root: "C:/work/tree".into(),
        executable_path: "C:/work/tree/target/gal-pipeline/bin/abc/gal.exe".into(),
        executable_sha256: "abc123".into(),
    };
    assert!(state.guarded_entry(malformed.clone()).is_err());
    malformed.version = ExecutionBinding::VERSION;
    let digest = state
        .guarded_entry(malformed)
        .expect("initialize a new safe segment");
    assert_eq!(
        state.execution_binding.as_ref().unwrap().digest().unwrap(),
        digest
    );
}

fn accounting_identity(run_id: &str) -> RunIdentity {
    RunIdentity {
        version: RUN_MANIFEST_VERSION,
        run_id: run_id.into(),
        canonical_worktree: "C:/work/tree".into(),
        plan_scope: "plan".into(),
        host_session: Some("codex:session-a".into()),
        host_window: None,
    }
}

#[test]
fn legacy_state_loads_without_accounting_run_or_invented_usage() {
    let state = CoordinatorState::new_legacy("plan", "prompt", "T-1", "implement");
    let mut legacy = serde_json::to_value(&state).unwrap();
    legacy.as_object_mut().unwrap().remove("accounting_run");
    let loaded: CoordinatorState = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(loaded.accounting_run, None);
    assert_eq!(loaded.accounting_run_id(), None);
    // Unset state is not written back with a new key, so legacy bytes stay equal.
    assert_eq!(serde_json::to_value(&loaded).unwrap(), legacy);
}

#[test]
fn accounting_run_is_allocated_once_and_reused_on_resume() {
    let mut state = CoordinatorState::new_legacy("plan", "prompt", "T-1", "implement");
    let before = state.revision;
    assert_eq!(
        state.bind_accounting_run(accounting_identity("run-a")),
        Ok(RunBindingOutcome::Allocated)
    );
    assert_eq!(state.revision, before + 1);
    assert_eq!(state.accounting_run_id(), Some("run-a"));
    state.claim_attempt("attempt-1".into()).unwrap();
    state.finish_attempt("attempt-1", None).unwrap();

    // A resume reloads the persisted state and presents the same identity.
    let mut resumed: CoordinatorState =
        serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
    let snapshot = resumed.clone();
    let outcome_reuse = resumed.bind_accounting_run(accounting_identity("run-a"));
    assert_eq!(outcome_reuse, Ok(RunBindingOutcome::Reused));
    assert_eq!(resumed, snapshot);
    assert_eq!(resumed.accounting_run_id(), Some("run-a"));
    println!(
        "probe coordinator continuity: allocation=Allocated reuse=Reused run_id={:?}",
        resumed.accounting_run_id()
    );
}

#[test]
fn accounting_run_rejects_conflicting_identity_without_mutation() {
    let mut state = CoordinatorState::new_legacy("plan", "prompt", "T-1", "implement");
    state
        .bind_accounting_run(accounting_identity("run-a"))
        .unwrap();
    let before = state.clone();
    let mutations: [fn(&mut RunIdentity); 4] = [
        |identity| identity.run_id = "run-b".into(),
        |identity| identity.canonical_worktree = "C:/other/tree".into(),
        |identity| identity.plan_scope = "other-plan".into(),
        |identity| identity.host_session = None,
    ];
    for mutate in mutations {
        let mut conflicting = accounting_identity("run-a");
        mutate(&mut conflicting);
        assert_eq!(
            state.bind_accounting_run(conflicting),
            Err(CoordinatorError::AccountingRunConflict)
        );
        assert_eq!(state, before);
    }
}

#[test]
fn accounting_run_rejects_invalid_identity_before_recording_it() {
    let mut state = CoordinatorState::new_legacy("plan", "prompt", "T-1", "implement");
    let before = state.clone();
    assert_eq!(
        state.bind_accounting_run(accounting_identity("")),
        Err(CoordinatorError::InvalidTransition(
            "accounting run identity is invalid"
        ))
    );
    assert_eq!(state, before);
    assert_eq!(state.accounting_run_id(), None);
}
