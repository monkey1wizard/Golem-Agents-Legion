use dispatch::usage::UsageCategory;
use pipeline::usage::reconcile::{
    reconcile, Allocation, Amount, Basis, ClockDomain, Contribution, ElapsedInterval, MetricKey,
    ObservationRecord, ReconcileError, ReconcileInput, Reconciliation, Totals, Treatment,
};
use serde::Deserialize;

const FIXTURES: &str = include_str!("fixtures/usage_accounting.json");

#[derive(Deserialize)]
struct FixtureFile {
    cases: Vec<FixtureCase>,
}

#[derive(Deserialize)]
struct FixtureCase {
    name: String,
    run_id: String,
    cutoff_unix_ms: u64,
    observations: Vec<ObservationRecord>,
    intervals: Vec<ElapsedInterval>,
}

fn reconcile_case(name: &str) -> Reconciliation {
    let file: FixtureFile = serde_json::from_str(FIXTURES).expect("fixture file is valid JSON");
    let case = file
        .cases
        .into_iter()
        .find(|case| case.name == name)
        .unwrap_or_else(|| panic!("fixture case `{name}` is missing"));
    reconcile(ReconcileInput {
        run_id: case.run_id,
        cutoff_unix_ms: case.cutoff_unix_ms,
        observations: case.observations,
        intervals: case.intervals,
    })
    .expect("fixture case reconciles")
}

fn output_row<'a>(reconciled: &'a Reconciliation, observation_id: &str) -> &'a Contribution {
    reconciled
        .contributions
        .iter()
        .find(|row| {
            row.observation_id == observation_id
                && row.metric == MetricKey::Tokens(UsageCategory::Output)
        })
        .unwrap_or_else(|| panic!("no output row for `{observation_id}`"))
}

fn task<'a>(reconciled: &'a Reconciliation, task_id: &str) -> &'a Totals {
    reconciled
        .tasks
        .iter()
        .find(|(key, _)| key.task_id == task_id)
        .map(|(_, totals)| totals)
        .unwrap_or_else(|| panic!("no task `{task_id}`"))
}

fn record(observation_id: &str, observation: serde_json::Value) -> ObservationRecord {
    serde_json::from_value(serde_json::json!({
        "observation_id": observation_id,
        "observation": observation,
    }))
    .expect("inline observation is valid")
}

#[test]
fn retry_attempts_all_count_including_the_failed_one() {
    let reconciled = reconcile_case("retry_attempts_all_count");
    let alpha = task(&reconciled, "alpha-task");

    let output = alpha.token(UsageCategory::Output);
    assert_eq!(output.total, Some(160));
    assert!(output.complete);
    assert_eq!(output.coverage.included, 2);
    assert_eq!(output.cutoff_unix_ms, 10_000_000);
    assert_eq!(
        output_row(&reconciled, "obs-attempt-1").treatment,
        Treatment::Included
    );
    assert_eq!(
        output_row(&reconciled, "obs-attempt-2").treatment,
        Treatment::Included
    );

    assert_eq!(alpha.cost.len(), 1);
    let cost = &alpha.cost[0].subtotal;
    assert_eq!(
        cost.total.as_ref().map(|amount| amount.amount.to_string()),
        Some("0.50".to_owned())
    );
    assert!(!cost.complete);
    assert_eq!(cost.coverage.unknown, 1);
}

#[test]
fn cumulative_series_adds_its_delta_once() {
    let reconciled = reconcile_case("cumulative_series_counts_once");

    let output = reconciled.run.token(UsageCategory::Output);
    assert_eq!(output.total, Some(60));

    let step = output_row(&reconciled, "obs-cumulative-2");
    assert_eq!(step.contributed, Some(Amount::Tokens(60)));
    assert_eq!(step.basis, Some(Basis::CumulativeDelta));
    assert_eq!(
        output_row(&reconciled, "obs-cumulative-1").treatment,
        Treatment::Excluded("baseline_reading".to_owned())
    );
    for turn in ["obs-turn-1", "obs-turn-2"] {
        assert_eq!(
            output_row(&reconciled, turn).treatment,
            Treatment::Excluded("alternative_basis_present".to_owned())
        );
    }
}

#[test]
fn inclusive_parent_total_does_not_add_its_child() {
    let reconciled = reconcile_case("inclusive_parent_counts_child_once");

    assert_eq!(reconciled.run.token(UsageCategory::Output).total, Some(200));
    assert_eq!(
        output_row(&reconciled, "obs-child").treatment,
        Treatment::Excluded("included_in_ancestor".to_owned())
    );
}

#[test]
fn exclusive_parent_total_adds_its_child() {
    let reconciled = reconcile_case("exclusive_parent_adds_child");

    assert_eq!(reconciled.run.token(UsageCategory::Output).total, Some(260));
    assert_eq!(
        output_row(&reconciled, "obs-child").treatment,
        Treatment::Included
    );
}

#[test]
fn parallel_agents_sum_above_their_wall_time() {
    let reconciled = reconcile_case("parallel_intervals");

    assert_eq!(reconciled.elapsed.summed_ms, Some(20_000));
    assert_eq!(reconciled.elapsed.wall_ms, Some(10_000));
    assert!(reconciled.elapsed.incomparable_remote_attempts.is_empty());
    assert_eq!(reconciled.elapsed.cutoff_unix_ms, 10_000_000);
}

#[test]
fn all_unknown_costs_stay_unavailable_and_keep_their_reasons() {
    let reconciled = reconcile_case("all_costs_unknown");
    let alpha = task(&reconciled, "alpha-task");

    assert_eq!(alpha.cost.len(), 1);
    let cost = &alpha.cost[0].subtotal;
    assert_eq!(cost.total, None);
    assert!(!cost.complete);
    assert_eq!(cost.reason.as_deref(), Some("provider reported no cost"));
    assert_eq!(cost.coverage.unknown, 2);

    let cost_rows: Vec<&Contribution> = reconciled
        .contributions
        .iter()
        .filter(|row| row.metric == MetricKey::Cost)
        .collect();
    assert_eq!(cost_rows.len(), 2);
    assert_eq!(
        cost_rows[0].treatment,
        Treatment::Unknown("provider reported no cost".to_owned())
    );
    assert_eq!(
        cost_rows[1].treatment,
        Treatment::Unknown("cost field is not a number".to_owned())
    );
    assert!(cost_rows.iter().all(|row| row.reported.is_none()));

    assert_eq!(alpha.token(UsageCategory::Output).total, Some(160));
}

#[test]
fn counter_reset_and_missing_baseline_stay_non_additive() {
    let reconciled = reconcile_case("reset_and_missing_baseline");
    let delta = task(&reconciled, "delta-task").token(UsageCategory::Output);

    assert_eq!(delta.total, Some(20));
    assert!(!delta.complete);
    assert_eq!(delta.coverage.non_additive, 2);
    assert_eq!(
        output_row(&reconciled, "obs-reset-3").contributed,
        Some(Amount::Tokens(20))
    );
    assert_eq!(
        output_row(&reconciled, "obs-reset-2").treatment,
        Treatment::NonAdditive("counter_reset".to_owned())
    );
    assert_eq!(
        output_row(&reconciled, "obs-lone").treatment,
        Treatment::NonAdditive("missing_baseline".to_owned())
    );
    assert_eq!(
        output_row(&reconciled, "obs-lone").reported,
        Some(Amount::Tokens(500))
    );
}

#[test]
fn shared_session_counts_once_and_leaves_tasks_unallocated() {
    let reconciled = reconcile_case("shared_session_unallocated");

    let output = reconciled.run.token(UsageCategory::Output);
    assert_eq!(output.total, Some(60));
    assert_eq!(output.coverage.unallocated, 2);
    assert!(reconciled.tasks.is_empty());
    assert_eq!(
        output_row(&reconciled, "obs-shared-a").allocation,
        Allocation::Unallocated("shared_session".to_owned())
    );
}

#[test]
fn remote_clock_sums_but_stays_out_of_wall_time() {
    let intervals = vec![
        ElapsedInterval {
            attempt_id: "attempt-1".to_owned(),
            start_unix_ms: 0,
            end_unix_ms: 10_000,
            clock: ClockDomain::Controller,
        },
        ElapsedInterval {
            attempt_id: "attempt-3".to_owned(),
            start_unix_ms: 2_000,
            end_unix_ms: 7_000,
            clock: ClockDomain::Remote {
                host: "host-b".to_owned(),
            },
        },
    ];
    let reconciled = reconcile(ReconcileInput {
        run_id: "run-remote".to_owned(),
        cutoff_unix_ms: 10_000_000,
        observations: Vec::new(),
        intervals,
    })
    .expect("remote intervals reconcile");

    assert_eq!(reconciled.elapsed.wall_ms, Some(10_000));
    assert_eq!(reconciled.elapsed.summed_ms, Some(15_000));
    assert_eq!(
        reconciled.elapsed.incomparable_remote_attempts,
        vec!["attempt-3".to_owned()]
    );
}

#[test]
fn rows_after_the_cutoff_are_excluded_but_kept() {
    let observation = serde_json::json!({
        "schema_version": 1,
        "identity": { "session_id": "late-session", "task_id": "late-task" },
        "tokens": {
            "output": {
                "count": {
                    "value": { "state": "reported", "value": 25 },
                    "semantics": "delta",
                    "descendants": "own",
                    "window": { "start_unix_ms": 4000, "end_unix_ms": 5000 }
                },
                "relation": { "kind": "exclusive" }
            }
        }
    });
    let reconciled = reconcile(ReconcileInput {
        run_id: "run-cutoff".to_owned(),
        cutoff_unix_ms: 4_000,
        observations: vec![record("obs-late", observation)],
        intervals: Vec::new(),
    })
    .expect("cutoff case reconciles");

    let row = output_row(&reconciled, "obs-late");
    assert_eq!(
        row.treatment,
        Treatment::Excluded("after_cutoff".to_owned())
    );
    assert_eq!(row.reported, Some(Amount::Tokens(25)));

    let output = reconciled.run.token(UsageCategory::Output);
    assert_eq!(output.total, None);
    assert_eq!(output.cutoff_unix_ms, 4_000);
    assert_eq!(output.reason.as_deref(), Some("no additive observations"));
}

#[test]
fn duplicate_observation_ids_are_rejected() {
    let observation = serde_json::json!({ "schema_version": 1 });
    let result = reconcile(ReconcileInput {
        run_id: "run-duplicate".to_owned(),
        cutoff_unix_ms: 10_000_000,
        observations: vec![
            record("obs-same", observation.clone()),
            record("obs-same", observation),
        ],
        intervals: Vec::new(),
    });

    assert_eq!(
        result.err(),
        Some(ReconcileError::DuplicateObservation("obs-same".to_owned()))
    );
}

#[test]
fn token_and_cost_use_separate_contribution_sets_when_inclusion_differs() {
    let parent = serde_json::json!({
        "schema_version": 1,
        "identity": { "plan_id": "plan-a", "task_id": "gamma-task", "session_id": "parent-session" },
        "tokens": {
            "output": {
                "count": {
                    "value": { "state": "reported", "value": 200 },
                    "semantics": "delta",
                    "descendants": "includes_descendants",
                    "window": { "start_unix_ms": 1000, "end_unix_ms": 2000 }
                },
                "relation": { "kind": "exclusive" }
            }
        },
        "cost": {
            "value": {
                "state": "reported",
                "value": {
                    "amount": "2.00",
                    "unit": { "currency": "USD" },
                    "kind": "provider_reported"
                }
            },
            "semantics": "delta",
            "descendants": "own",
            "window": { "start_unix_ms": 1000, "end_unix_ms": 2000 }
        }
    });

    let child = serde_json::json!({
        "schema_version": 1,
        "identity": {
            "plan_id": "plan-a",
            "task_id": "gamma-task",
            "session_id": "child-session",
            "parent_session_id": "parent-session"
        },
        "tokens": {
            "output": {
                "count": {
                    "value": { "state": "reported", "value": 60 },
                    "semantics": "delta",
                    "descendants": "own",
                    "window": { "start_unix_ms": 1200, "end_unix_ms": 1800 }
                },
                "relation": { "kind": "exclusive" }
            }
        },
        "cost": {
            "value": {
                "state": "reported",
                "value": {
                    "amount": "0.60",
                    "unit": { "currency": "USD" },
                    "kind": "provider_reported"
                }
            },
            "semantics": "delta",
            "descendants": "own",
            "window": { "start_unix_ms": 1200, "end_unix_ms": 1800 }
        }
    });

    let reconciled = reconcile(ReconcileInput {
        run_id: "run-diff-inclusion".to_owned(),
        cutoff_unix_ms: 10_000_000,
        observations: vec![record("obs-parent", parent), record("obs-child", child)],
        intervals: Vec::new(),
    })
    .expect("different inclusion case reconciles");

    // Tokens: parent is inclusive (200), child (60) is excluded -> total 200
    assert_eq!(reconciled.run.token(UsageCategory::Output).total, Some(200));
    assert_eq!(
        output_row(&reconciled, "obs-parent").treatment,
        Treatment::Included
    );
    assert_eq!(
        output_row(&reconciled, "obs-child").treatment,
        Treatment::Excluded("included_in_ancestor".to_owned())
    );

    // Cost: parent is exclusive/own ($2.00), child is own ($0.60) -> total $2.60
    assert_eq!(reconciled.run.cost.len(), 1);
    let cost = &reconciled.run.cost[0].subtotal;
    assert_eq!(
        cost.total.as_ref().map(|amount| amount.amount.to_string()),
        Some("2.60".to_owned())
    );

    let cost_parent = reconciled
        .contributions
        .iter()
        .find(|row| row.observation_id == "obs-parent" && row.metric == MetricKey::Cost)
        .expect("parent cost row");
    assert_eq!(cost_parent.treatment, Treatment::Included);

    let cost_child = reconciled
        .contributions
        .iter()
        .find(|row| row.observation_id == "obs-child" && row.metric == MetricKey::Cost)
        .expect("child cost row");
    assert_eq!(cost_child.treatment, Treatment::Included);
}
