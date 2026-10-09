use pipeline::projection_journal::{apply_with_hook, recover, Boundary, JournalError, Projection};
use std::{fs, path::Path};
use tempfile::tempdir;

fn projections(root: &Path) -> [Projection; 3] {
    ["plan.md", "prompt.md", "state.md"].map(|name| Projection {
        path: root.join(name),
        bytes: format!("new {name}").into_bytes(),
    })
}

fn seed(root: &Path) {
    for name in ["plan.md", "prompt.md", "state.md"] {
        fs::write(root.join(name), format!("old {name}")).unwrap();
    }
}

fn assert_new(root: &Path) {
    for name in ["plan.md", "prompt.md", "state.md"] {
        assert_eq!(
            fs::read(root.join(name)).unwrap(),
            format!("new {name}").as_bytes()
        );
    }
}

#[test]
fn every_boundary_recovers_idempotently() {
    // Journal, three ordered lock, three replacement, and commit boundaries.
    let points = [
        Boundary::BeforeJournalPrepared,
        Boundary::JournalPrepared,
        Boundary::BeforeLock(0),
        Boundary::LockAcquired(0),
        Boundary::BeforeLock(1),
        Boundary::LockAcquired(1),
        Boundary::BeforeLock(2),
        Boundary::LockAcquired(2),
        Boundary::BeforeTargetReplaced(0),
        Boundary::TargetReplaced(0),
        Boundary::BeforeTargetReplaced(1),
        Boundary::TargetReplaced(1),
        Boundary::BeforeTargetReplaced(2),
        Boundary::TargetReplaced(2),
        Boundary::BeforeCommit,
        Boundary::Committed,
    ];
    for point in points {
        let dir = tempdir().unwrap();
        seed(dir.path());
        let journal = dir.path().join("journal.json");
        let err = apply_with_hook(&journal, projections(dir.path()), |at| {
            if at == point {
                Err(JournalError::Interrupted(at))
            } else {
                Ok(())
            }
        });
        assert!(err.is_err(), "boundary {point:?} did not interrupt");
        if point == Boundary::BeforeJournalPrepared {
            assert!(!journal.exists());
            for name in ["plan.md", "prompt.md", "state.md"] {
                assert_eq!(
                    fs::read(dir.path().join(name)).unwrap(),
                    format!("old {name}").as_bytes()
                );
            }
            continue;
        }
        recover(&journal).unwrap();
        recover(&journal).unwrap();
        assert_new(dir.path());
    }
}

#[test]
fn third_hash_is_preserved_and_recovery_keeps_conflict_journal() {
    let dir = tempdir().unwrap();
    seed(dir.path());
    let journal = dir.path().join("journal.json");
    let err = apply_with_hook(&journal, projections(dir.path()), |at| {
        if at == Boundary::TargetReplaced(0) {
            Err(JournalError::Interrupted(at))
        } else {
            Ok(())
        }
    });
    assert!(err.is_err());
    let conflicting = dir.path().join("state.md");
    fs::write(&conflicting, b"outside edit").unwrap();
    assert!(matches!(recover(&journal), Err(JournalError::ThirdHash(path)) if path == conflicting));
    assert_eq!(fs::read(&conflicting).unwrap(), b"outside edit");
    assert!(journal.exists());
}

#[test]
fn interrupted_partial_replacement_is_detectable_and_recovers_idempotently() {
    let dir = tempdir().unwrap();
    seed(dir.path());
    let journal = dir.path().join("journal.json");

    // Interrupt after replacing the first target (index 0).
    let err = apply_with_hook(&journal, projections(dir.path()), |at| {
        if at == Boundary::TargetReplaced(0) {
            Err(JournalError::Interrupted(at))
        } else {
            Ok(())
        }
    });
    assert!(matches!(
        err,
        Err(JournalError::Interrupted(Boundary::TargetReplaced(0)))
    ));

    // Partial state is detectable: target 0 is updated, targets 1 and 2 remain old.
    assert_eq!(
        fs::read(dir.path().join("plan.md")).unwrap(),
        b"new plan.md"
    );
    assert_eq!(
        fs::read(dir.path().join("prompt.md")).unwrap(),
        b"old prompt.md"
    );
    assert_eq!(
        fs::read(dir.path().join("state.md")).unwrap(),
        b"old state.md"
    );

    // Journal exists and is not yet committed.
    let journal_raw = fs::read_to_string(&journal).unwrap();
    let val: serde_json::Value = serde_json::from_str(&journal_raw).unwrap();
    assert_eq!(val["committed"], false);

    // Recovery completes the remaining targets.
    recover(&journal).unwrap();
    assert_new(dir.path());

    // Journal is now committed.
    let val_after: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&journal).unwrap()).unwrap();
    assert_eq!(val_after["committed"], true);

    // Second recovery is idempotent and preserves the updated state.
    recover(&journal).unwrap();
    assert_new(dir.path());
}

#[test]
fn locks_are_acquired_in_sorted_order_and_cleaned_up() {
    let dir = tempdir().unwrap();
    seed(dir.path());
    let journal = dir.path().join("journal.json");

    // Pass projections in reverse order to ensure internal sort order is enforced.
    let projs = [
        Projection {
            path: dir.path().join("state.md"),
            bytes: b"new state.md".to_vec(),
        },
        Projection {
            path: dir.path().join("prompt.md"),
            bytes: b"new prompt.md".to_vec(),
        },
        Projection {
            path: dir.path().join("plan.md"),
            bytes: b"new plan.md".to_vec(),
        },
    ];

    let mut lock_sequence = Vec::new();
    apply_with_hook(&journal, projs, |at| {
        if let Boundary::BeforeLock(idx) = at {
            lock_sequence.push(idx);
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(lock_sequence, vec![0, 1, 2]);
    assert_new(dir.path());

    // Projection locks should be cleaned up after completion.
    for name in ["plan.md", "prompt.md", "state.md"] {
        let mut lock_name = dir.path().join(name).into_os_string();
        lock_name.push(".projection-lock");
        assert!(
            !Path::new(&lock_name).exists(),
            "lock file was not released"
        );
    }
}

#[test]
fn duplicate_target_paths_are_rejected() {
    let dir = tempdir().unwrap();
    seed(dir.path());
    let journal = dir.path().join("journal.json");

    let duplicate_projs = [
        Projection {
            path: dir.path().join("plan.md"),
            bytes: b"new plan.md".to_vec(),
        },
        Projection {
            path: dir.path().join("plan.md"),
            bytes: b"duplicate plan.md".to_vec(),
        },
        Projection {
            path: dir.path().join("state.md"),
            bytes: b"new state.md".to_vec(),
        },
    ];

    let res = pipeline::projection_journal::apply(&journal, duplicate_projs);
    assert!(matches!(res, Err(JournalError::InvalidTargets)));
    assert!(!journal.exists());
}
