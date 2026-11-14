use pipeline::coordinator::{
    check_transition_invariants, verify_bounded_model, AttemptOwnership, CheckpointKind,
    CheckpointReceipt, CoordinatorError, CoordinatorState, ExecutionBinding, ModelEvent,
    ModelOutcome, NextAction, OwnerObservation,
};
use std::time::Duration;

fn state() -> CoordinatorState {
    let mut state = CoordinatorState::new_legacy("plan", "prompt", "task", "implement");
    state.activate_codex_stop_v1("activation".into()).unwrap();
    state
        .guarded_entry(ExecutionBinding {
            version: ExecutionBinding::VERSION,
            binding_scope: "task".into(),
            worktree_root: "worktree".into(),
            executable_path: "codex".into(),
            executable_sha256: "digest".into(),
        })
        .unwrap();
    state
}

#[test]
fn bounded_exploration() {
    let report = verify_bounded_model().unwrap_or_else(|v| panic!("{v}"));
    println!("attempt depth 4: {:?}; checkpoints depth 4: {:?}; host depth 4: {:?}; witnesses: {:?}; elapsed: {:?}", report.attempt_counts, report.checkpoint_counts, report.host_counts, report.witnesses, report.elapsed);
    assert_eq!(report.attempt_counts, [11, 121, 1331, 14641]);
    assert_eq!(report.checkpoint_counts, [1, 5, 29, 197]);
    assert_eq!(report.host_counts, [6, 36, 216, 1296]);
    for witness in [
        "claim-same",
        "claim-different",
        "reclaim",
        "ownership-Live-Live",
        "ownership-Live-Unknown",
        "ownership-Dead-Dead",
        "ownership-Unknown-Unknown",
        "ownership-NotApplicable-NotApplicable",
        "receipt-true",
        "receipt-false",
        "receipt-replay",
        "stale-receipt",
        "host-receipt-rejected",
        "host-close",
        "host-close-replay",
    ] {
        assert!(report.witnesses.contains(witness), "missing {witness}");
    }
    assert!(report.elapsed <= Duration::from_secs(10));
}

#[test]
fn synthetic_violations_have_replayable_properties() {
    let mut before = state();
    before.claim_attempt("A".into()).unwrap();
    let event = ModelEvent::Claim("B".into());
    let violation = check_transition_invariants(
        &before,
        &event,
        &ModelOutcome::Revision(Ok(before.revision + 1)),
        &before,
    )
    .unwrap_err();
    assert_eq!(violation.property, "P1");
    assert!(!violation.steps.is_empty());
    assert!(!violation.expected.is_empty());
    assert!(!violation.actual.is_empty());
    let event = ModelEvent::Observe(OwnerObservation {
        verdict: AttemptOwnership::Unknown,
        observed_pid: None,
        observed_birth: None,
    });
    let mut after = before.clone();
    after.next_action = NextAction::RecoverAttempt {
        attempt_id: "A".into(),
    };
    let violation = check_transition_invariants(
        &before,
        &event,
        &ModelOutcome::Ownership(Ok(AttemptOwnership::Unknown)),
        &after,
    )
    .unwrap_err();
    assert_eq!(violation.property, "P2");
    assert!(!violation.steps.is_empty());
    before
        .checkpoint("checkpoint".into(), CheckpointKind::TaskQuality, None)
        .unwrap();
    let c = before.checkpoint.as_ref().unwrap();
    let receipt = CheckpointReceipt {
        receipt_id: "receipt".into(),
        checkpoint_id: c.checkpoint_id.clone(),
        revision: c.revision,
        prompt_sha256: c.prompt_sha256.clone(),
        execution_binding_sha256: c.execution_binding_sha256.clone(),
        commit: c.commit.clone(),
        accepted: true,
    };
    before.consume_checkpoint_receipt(receipt.clone()).unwrap();
    let event = ModelEvent::Receipt(receipt);
    let violation = check_transition_invariants(
        &before,
        &event,
        &ModelOutcome::Revision(Ok(before.revision + 1)),
        &before,
    )
    .unwrap_err();
    assert_eq!(violation.property, "P3");
    assert!(!violation.steps.is_empty());
    assert!(check_transition_invariants(
        &before,
        &event,
        &ModelOutcome::Revision(Err(CoordinatorError::ReceiptReplay)),
        &before
    )
    .is_ok());
}

#[test]
fn five_choice_controls_detect_missing_and_substituted_events() {
    use pipeline::coordinator::check_checkpoint_event_choices;
    let mut state = state();
    state
        .checkpoint("checkpoint".into(), CheckpointKind::TaskQuality, None)
        .unwrap();
    let publication = state.checkpoint.clone().unwrap();
    let mut choices = vec![ModelEvent::Publish];
    for id in ["receipt-A", "receipt-B"] {
        for accepted in [true, false] {
            choices.push(ModelEvent::Receipt(CheckpointReceipt {
                receipt_id: id.into(),
                checkpoint_id: publication.checkpoint_id.clone(),
                revision: publication.revision,
                prompt_sha256: publication.prompt_sha256.clone(),
                execution_binding_sha256: publication.execution_binding_sha256.clone(),
                commit: publication.commit.clone(),
                accepted,
            }));
        }
    }
    assert!(check_checkpoint_event_choices(
        std::slice::from_ref(&publication),
        &choices
    ));
    let mut missing = choices.clone();
    missing.pop();
    assert!(!check_checkpoint_event_choices(
        std::slice::from_ref(&publication),
        &missing
    ));
    let mut substituted = choices.clone();
    substituted[4] = substituted[3].clone();
    assert!(!check_checkpoint_event_choices(
        &[publication],
        &substituted
    ));
}
