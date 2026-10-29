use pipeline::usage::binding::{
    bind_session_intervals, AttemptLink, BindingError, BoundInterval, IntervalBinding, RunIdentity,
    RunManifest, SessionClaim, RUN_MANIFEST_VERSION,
};

fn identity(run_id: &str) -> RunIdentity {
    RunIdentity {
        version: RUN_MANIFEST_VERSION,
        run_id: run_id.into(),
        canonical_worktree: "C:/work/tree".into(),
        plan_scope: "plan".into(),
        host_session: Some("codex:session-a".into()),
        host_window: None,
    }
}

fn claim(run_id: &str, session: &str, start_ms: u64, end_ms: u64) -> SessionClaim {
    SessionClaim {
        run_id: run_id.into(),
        session: session.into(),
        start_ms,
        end_ms,
    }
}

fn segment(
    run_id: &str,
    session: &str,
    start_ms: u64,
    end_ms: u64,
    binding: IntervalBinding,
) -> BoundInterval {
    BoundInterval {
        run_id: run_id.into(),
        session: session.into(),
        start_ms,
        end_ms,
        binding,
    }
}

fn ambiguous(run_ids: &[&str]) -> IntervalBinding {
    IntervalBinding::Ambiguous {
        competing_run_ids: run_ids.iter().map(|id| id.to_string()).collect(),
    }
}

#[test]
fn run_identity_digest_is_stable_and_separates_fields() {
    let base = identity("run-a");
    assert_eq!(base.digest().unwrap(), identity("run-a").digest().unwrap());

    let mut windowed = base.clone();
    windowed.host_window = Some("pane-1".into());
    assert_ne!(base.digest().unwrap(), windowed.digest().unwrap());

    let mut unbound = base.clone();
    unbound.host_session = None;
    assert_ne!(base.digest().unwrap(), unbound.digest().unwrap());

    // Length prefixes keep the boundary between adjacent fields significant.
    let mut split_a = identity("run-a");
    split_a.canonical_worktree = "ab".into();
    split_a.plan_scope = "c".into();
    let mut split_b = identity("run-a");
    split_b.canonical_worktree = "a".into();
    split_b.plan_scope = "bc".into();
    assert_ne!(split_a.digest().unwrap(), split_b.digest().unwrap());
}

#[test]
fn run_identity_rejects_empty_fields_bad_sessions_and_versions() {
    let mut empty = identity("run-a");
    empty.run_id = " ".into();
    assert_eq!(empty.validate(), Err(BindingError::EmptyField("run_id")));

    let mut window = identity("run-a");
    window.host_window = Some(" ".into());
    assert_eq!(
        window.validate(),
        Err(BindingError::EmptyField("host_window"))
    );

    let mut foreign = identity("run-a");
    foreign.host_session = Some("gemini:session-a".into());
    assert_eq!(
        foreign.validate(),
        Err(BindingError::InvalidHostSession("gemini:session-a".into()))
    );

    let mut version = identity("run-a");
    version.version = RUN_MANIFEST_VERSION + 1;
    assert_eq!(
        version.digest(),
        Err(BindingError::UnsupportedVersion(RUN_MANIFEST_VERSION + 1))
    );
}

#[test]
fn manifest_digest_separates_initial_retry_and_resume_links() {
    let initial = RunManifest::standalone(identity("run-a"), "attempt-2");
    let retry = RunManifest {
        identity: identity("run-a"),
        attempt_id: "attempt-2".into(),
        link: AttemptLink::Retry {
            previous_attempt_id: "attempt-1".into(),
        },
    };
    let resume = RunManifest {
        identity: identity("run-a"),
        attempt_id: "attempt-2".into(),
        link: AttemptLink::Resume {
            previous_attempt_id: "attempt-1".into(),
        },
    };
    let initial_digest = initial.digest().unwrap();
    let retry_digest = retry.digest().unwrap();
    let resume_digest = resume.digest().unwrap();
    assert_ne!(initial_digest, retry_digest);
    assert_ne!(initial_digest, resume_digest);
    assert_ne!(retry_digest, resume_digest);
}

#[test]
fn manifest_rejects_self_links_and_empty_attempts() {
    let self_link = RunManifest {
        identity: identity("run-a"),
        attempt_id: "attempt-1".into(),
        link: AttemptLink::Resume {
            previous_attempt_id: "attempt-1".into(),
        },
    };
    assert_eq!(self_link.validate(), Err(BindingError::SelfLink));

    let empty = RunManifest::standalone(identity("run-a"), " ");
    assert_eq!(
        empty.validate(),
        Err(BindingError::EmptyField("attempt_id"))
    );
}

#[test]
fn standalone_manifest_round_trips_as_an_initial_link() {
    let manifest = RunManifest::standalone(identity("run-a"), "attempt-1");
    let json = serde_json::to_value(&manifest).unwrap();
    assert_eq!(json["link"]["kind"], "initial");
    let loaded: RunManifest = serde_json::from_value(json).unwrap();
    assert_eq!(loaded, manifest);
}

#[test]
fn disjoint_intervals_on_a_shared_session_stay_exclusive() {
    let bound = bind_session_intervals(&[
        claim("run-a", "codex:s", 0, 10),
        claim("run-b", "codex:s", 10, 20),
    ])
    .unwrap();
    assert_eq!(
        bound,
        vec![
            segment("run-a", "codex:s", 0, 10, IntervalBinding::Exclusive),
            segment("run-b", "codex:s", 10, 20, IntervalBinding::Exclusive),
        ]
    );
}

#[test]
fn overlapping_plans_split_into_exclusive_and_ambiguous_segments() {
    let bound = bind_session_intervals(&[
        claim("run-a", "codex:s", 0, 10),
        claim("run-b", "codex:s", 5, 15),
    ])
    .unwrap();
    // The shared window [5, 10) is ambiguous for both runs. Neither run gets
    // it as an exclusive segment, so it cannot be counted twice.
    println!("probe shared session binding: segments={:?}", bound);
    assert_eq!(
        bound,
        vec![
            segment("run-a", "codex:s", 0, 5, IntervalBinding::Exclusive),
            segment("run-a", "codex:s", 5, 10, ambiguous(&["run-b"])),
            segment("run-b", "codex:s", 5, 10, ambiguous(&["run-a"])),
            segment("run-b", "codex:s", 10, 15, IntervalBinding::Exclusive),
        ]
    );
}

#[test]
fn ambiguous_segments_list_each_competing_run_once_in_order() {
    let bound = bind_session_intervals(&[
        claim("run-a", "codex:s", 0, 10),
        claim("run-c", "codex:s", 0, 10),
        claim("run-b", "codex:s", 0, 10),
    ])
    .unwrap();
    assert_eq!(bound.len(), 3);
    assert_eq!(bound[0].binding, ambiguous(&["run-b", "run-c"]));
    assert_eq!(bound[1].binding, ambiguous(&["run-a", "run-b"]));
    assert_eq!(bound[2].binding, ambiguous(&["run-a", "run-c"]));
}

#[test]
fn same_run_overlap_and_other_sessions_are_not_ambiguous() {
    let same_run = bind_session_intervals(&[
        claim("run-a", "codex:s", 0, 10),
        claim("run-a", "codex:s", 5, 15),
    ])
    .unwrap();
    assert_eq!(
        same_run,
        vec![
            segment("run-a", "codex:s", 0, 10, IntervalBinding::Exclusive),
            segment("run-a", "codex:s", 5, 15, IntervalBinding::Exclusive),
        ]
    );

    let other_session = bind_session_intervals(&[
        claim("run-a", "codex:s1", 0, 10),
        claim("run-b", "codex:s2", 0, 10),
    ])
    .unwrap();
    assert_eq!(
        other_session,
        vec![
            segment("run-a", "codex:s1", 0, 10, IntervalBinding::Exclusive),
            segment("run-b", "codex:s2", 0, 10, IntervalBinding::Exclusive),
        ]
    );
}

#[test]
fn inverted_or_empty_session_intervals_are_rejected() {
    assert_eq!(
        bind_session_intervals(&[claim("run-a", "codex:s", 10, 10)]),
        Err(BindingError::InvertedInterval("codex:s".into()))
    );
    assert_eq!(
        bind_session_intervals(&[claim("run-a", "codex:s", 20, 10)]),
        Err(BindingError::InvertedInterval("codex:s".into()))
    );
}
