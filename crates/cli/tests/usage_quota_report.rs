//! Account capacity in `gal usage report` and the assessment write path, through the real binary.
//!
//! Quota and usage records are seeded with the production capture helpers. Then `gal` runs in a
//! repository as a user runs it. Text and JSON must show the same scoped quota states and the same
//! cost total, and neither a report nor a rejected assessment may change a stored byte.

#[path = "../src/commands/usage_capture.rs"]
#[allow(dead_code)]
mod usage_capture;

use dispatch::dispatch::DispatchQuota;
use dispatch::quota::{
    missing, AccountIdentityCheck, AccountRef, Acquisition, BillingMode, BucketId, Freshness,
    ProviderId, QueryMetadata, QuotaIdentity, QuotaObservation, QuotaRuntime, QuotaWindow,
    RuntimeVersion, WindowKind, COLLECTOR, QUOTA_SCHEMA_VERSION,
};
use dispatch::usage::{Availability, ObservationSource};
use pipeline::usage::binding::{RunIdentity, RUN_MANIFEST_VERSION};
use pipeline::usage::UsageStore;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;
use usage_capture::AttemptUsage;

const RUN: &str = "run-capacity";
const PLAIN_RUN: &str = "run-plain";
const NOW: u64 = 1_792_000_000_000;
const OBSERVED_AT: u64 = 1_800_000_100_000;
const COLLECTED_AT: u64 = 1_800_000_200_000;
const WEEKLY: &str = "codex-weekly";
const FIVE_HOUR: &str = "codex-five-hour";
const NO_FIVE_HOUR_WINDOW: &str = "provider sent no five-hour window";
const REFRESH_FAILED: &str = "refresh failed with exit code 1";
const NO_QUOTA: &str = "no quota revision is recorded for this run";

/// Exit code of a usage error, as `gal` reports it.
const USAGE_EXIT: i32 = 64;

fn gal(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("gal runs")
}

fn repository() -> TempDir {
    let root = TempDir::new().expect("temp repo");
    let status = Command::new("git")
        .args(["init", "-q"])
        .current_dir(root.path())
        .status()
        .expect("git runs");
    assert!(status.success(), "git init succeeds");
    root
}

fn store_at(root: &Path) -> UsageStore {
    UsageStore::open(&root.join(".git")).expect("store opens under .git")
}

fn run_identity(run_id: &str) -> RunIdentity {
    RunIdentity {
        version: RUN_MANIFEST_VERSION,
        run_id: run_id.to_owned(),
        canonical_worktree: "C:/work/golem".to_owned(),
        plan_scope: "feat-execution-usage-accounting".to_owned(),
        host_session: None,
        host_window: None,
    }
}

fn verified_identity() -> QuotaIdentity {
    QuotaIdentity {
        runtime: QuotaRuntime::Codex,
        runtime_version: Some(RuntimeVersion::new("0.163.0").expect("version is valid")),
        provider: ProviderId::new("openai").expect("provider is valid"),
        billing: BillingMode::Subscription,
        account: Some(AccountRef::new("acct-shared").expect("account is valid")),
        identity_check: AccountIdentityCheck::Verified,
        host: None,
        auth_profile: None,
        model_scope: None,
    }
}

fn amount(value: &str, unit: &str) -> Value {
    json!({ "value": value, "unit": unit })
}

/// A weekly read that carries the account's full state: window, utilization, amounts, and limit.
fn weekly(
    acquisition: Acquisition,
    observed_at: Option<u64>,
    query: Option<QueryMetadata>,
) -> QuotaObservation {
    QuotaObservation {
        schema_version: QUOTA_SCHEMA_VERSION,
        identity: verified_identity(),
        bucket: BucketId::new(WEEKLY).expect("bucket is valid"),
        window: Availability::Reported {
            value: QuotaWindow {
                kind: WindowKind::Weekly,
                duration_minutes: Some(10_080),
                resets_at_unix_ms: Some(1_800_600_000_000),
            },
        },
        utilization: Availability::Reported {
            value: serde_json::from_value(json!({
                "value": "0.42",
                "scale": "fraction",
                "direction": "used"
            }))
            .expect("utilization is valid"),
        },
        used: Availability::Reported {
            value: serde_json::from_value(amount("58", "requests")).expect("used is valid"),
        },
        remaining: Availability::Reported {
            value: serde_json::from_value(amount("42", "requests")).expect("remaining is valid"),
        },
        limit: Availability::Reported {
            value: serde_json::from_value(json!({
                "kind": "limited",
                "amount": amount("100", "requests")
            }))
            .expect("limit is valid"),
        },
        observed_at_unix_ms: observed_at,
        collected_at_unix_ms: COLLECTED_AT,
        source: ObservationSource {
            collector: Some(COLLECTOR.to_owned()),
            source_version: Some("0.163.0".to_owned()),
            cursor: None,
        },
        query,
        acquisition,
        freshness: if observed_at.is_some() {
            Freshness::Current
        } else {
            Freshness::Unknown
        },
    }
}

fn control_query() -> QueryMetadata {
    QueryMetadata {
        method: "account/rateLimits/read".to_owned(),
        duration_ms: 42,
    }
}

/// A launched return that reads passively, then runs its own control query.
fn weekly_reads() -> DispatchQuota {
    DispatchQuota::Collected {
        observations: vec![
            weekly(Acquisition::Passive, Some(OBSERVED_AT), None),
            weekly(
                Acquisition::Queried,
                Some(COLLECTED_AT),
                Some(control_query()),
            ),
        ],
    }
}

/// A five-hour read whose refresh failed. Its window is missing and no source time is known.
fn five_hour_refresh_failed() -> QuotaObservation {
    QuotaObservation {
        schema_version: QUOTA_SCHEMA_VERSION,
        identity: verified_identity(),
        bucket: BucketId::new(FIVE_HOUR).expect("bucket is valid"),
        window: Availability::Unavailable {
            reason: NO_FIVE_HOUR_WINDOW.to_owned(),
        },
        utilization: missing(REFRESH_FAILED),
        used: missing(REFRESH_FAILED),
        remaining: missing(REFRESH_FAILED),
        limit: missing(REFRESH_FAILED),
        observed_at_unix_ms: None,
        collected_at_unix_ms: COLLECTED_AT,
        source: ObservationSource {
            collector: Some(COLLECTOR.to_owned()),
            source_version: None,
            cursor: None,
        },
        query: None,
        acquisition: Acquisition::Failed {
            reason: REFRESH_FAILED.to_owned(),
        },
        freshness: Freshness::Unknown,
    }
}

fn output_usage(attempt_id: &str, output: u64) -> AttemptUsage {
    let observation = serde_json::from_value(json!({
        "tokens": {
            "output": {
                "count": {
                    "value": { "state": "reported", "value": output },
                    "semantics": "delta",
                    "descendants": "own",
                    "window": { "start_unix_ms": 1000, "end_unix_ms": 2000 }
                },
                "relation": { "kind": "exclusive" }
            }
        }
    }))
    .expect("output observation is valid");
    AttemptUsage {
        attempt_id: attempt_id.to_owned(),
        observations: vec![observation],
        elapsed_ms: Some(5_000),
    }
}

/// Run with 100 output tokens, two attempts that share one weekly scope, and one failed five-hour read.
fn seeded_repository() -> TempDir {
    let root = repository();
    let store = store_at(root.path());
    let identity = run_identity(RUN);
    usage_capture::capture_attempt(&store, RUN, &output_usage("attempt-a", 100), NOW)
        .expect("usage captures");
    usage_capture::capture_quota(&store, &identity, Some("attempt-a"), &weekly_reads());
    usage_capture::capture_quota(&store, &identity, Some("attempt-b"), &weekly_reads());
    usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-c"),
        &DispatchQuota::Collected {
            observations: vec![five_hour_refresh_failed()],
        },
    );
    drop(store);
    root
}

fn report(root: &Path, run: &str, json: bool) -> Output {
    let mut args = vec!["usage", "report", "--run", run];
    if json {
        args.push("--json");
    }
    gal(root, &args)
}

fn json_report(root: &Path, run: &str) -> Value {
    let output = report(root, run, true);
    assert!(output.status.success(), "report succeeds");
    serde_json::from_slice(&output.stdout).expect("report is one JSON document")
}

fn text_report(root: &Path, run: &str) -> String {
    let output = report(root, run, false);
    assert!(output.status.success(), "report succeeds");
    String::from_utf8(output.stdout).expect("report is UTF-8")
}

fn scope<'a>(account: &'a Value, bucket: &str) -> &'a Value {
    account["scopes"]
        .as_array()
        .expect("scopes is a list")
        .iter()
        .find(|scope| scope["bucket"] == bucket)
        .unwrap_or_else(|| panic!("no scope for bucket {bucket}"))
}

/// Every file under the Git common directory, with its bytes. A byte change anywhere shows here.
fn store_state(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(dir: &Path, state: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("directory reads") {
            let path = entry.expect("entry reads").path();
            if path.is_dir() {
                collect(&path, state);
            } else {
                state.insert(path.clone(), fs::read(&path).expect("file reads"));
            }
        }
    }
    let mut state = BTreeMap::new();
    collect(&root.join(".git"), &mut state);
    state
}

#[test]
fn text_and_json_show_the_same_scoped_states_and_one_cost_total() {
    let root = seeded_repository();
    let document = json_report(root.path(), RUN);
    let text = text_report(root.path(), RUN);

    let account = &document["account"];
    assert_eq!(account["contributes_to_cost"], false);
    assert_eq!(account["revision_id"], "quota-r3");
    assert_eq!(
        account["revision_ids"],
        json!(["quota-r1", "quota-r2", "quota-r3"])
    );
    assert_eq!(account["reason"], Value::Null);

    // The shared weekly scope appears once, and each attempt keeps its own reference revision.
    let weekly = scope(account, WEEKLY);
    assert_eq!(weekly["attempts"][0]["attempt_id"], "attempt-a");
    assert_eq!(weekly["attempts"][0]["reference"]["state"], "reported");
    assert_eq!(
        weekly["attempts"][0]["reference"]["revision_id"],
        "quota-r1"
    );
    assert_eq!(weekly["attempts"][1]["attempt_id"], "attempt-b");
    assert_eq!(
        weekly["attempts"][1]["reference"]["revision_id"],
        "quota-r2"
    );
    assert_eq!(weekly["window"]["state"], "reported");
    assert_eq!(weekly["utilization"]["value"]["direction"], "used");
    assert_eq!(weekly["used"]["value"]["value"], "58");
    assert_eq!(weekly["observed_at_unix_ms"], COLLECTED_AT);
    assert_eq!(weekly["collected_at_unix_ms"], COLLECTED_AT);
    assert_eq!(weekly["acquisition"]["state"], "queried");
    assert_eq!(weekly["freshness"], "current");
    assert_eq!(weekly["control_query"]["method"], "account/rateLimits/read");

    // The missing window keeps its reason, and the failed refresh keeps its own acquisition state.
    let five_hour = scope(account, FIVE_HOUR);
    assert_eq!(five_hour["window"]["state"], "unavailable");
    assert_eq!(five_hour["window"]["reason"], NO_FIVE_HOUR_WINDOW);
    assert_eq!(five_hour["acquisition"]["state"], "failed");
    assert_eq!(five_hour["acquisition"]["reason"], REFRESH_FAILED);
    assert_eq!(
        five_hour["attempts"][0]["reference"]["state"],
        "unavailable"
    );
    assert_eq!(
        five_hour["attempts"][0]["reference"]["reason"],
        REFRESH_FAILED
    );

    let cost = document["factual"]["totals"]
        .as_array()
        .expect("totals is a list")
        .iter()
        .find(|total| total["scope"] == "run" && total["basis"] == "tokens.output")
        .expect("output total exists");
    assert_eq!(cost["total"], 100);

    // Text shows the same states, the same single weekly entry, and the same cost total.
    assert!(text.contains("account capacity (shared quota, not a cost contribution):"));
    assert!(text.contains("revision: quota-r3, chain: quota-r1, quota-r2, quota-r3"));
    assert_eq!(text.matches("bucket: codex-weekly").count(), 1, "{text}");
    assert!(text.contains(
        "attempt-a observation=attempt-a-quota-0001 reference=reported revision=quota-r1"
    ));
    assert!(text.contains(
        "attempt-b observation=attempt-b-quota-0001 reference=reported revision=quota-r2"
    ));
    assert!(text.contains(&format!(
        "window: unavailable, reason: {NO_FIVE_HOUR_WINDOW}"
    )));
    assert!(text.contains(&format!(
        "acquisition: failed ({REFRESH_FAILED}), freshness: unknown"
    )));
    assert!(
        text.contains("observed_at_unix_ms: 1800000200000, collected_at_unix_ms: 1800000200000")
    );
    assert!(text.contains("observed_at_unix_ms: not recorded, collected_at_unix_ms: 1800000200000"));
    assert!(text.contains(&format!(
        "attempt-c observation=attempt-c-quota-0000 reference=unavailable revision=none digest=none reason={REFRESH_FAILED}"
    )));
    assert!(text.contains("run tokens.output: total=100"), "{text}");
}

#[test]
fn historical_reports_write_no_byte_and_keep_every_quota_revision() {
    let root = seeded_repository();
    let before = store_state(root.path());

    let text = text_report(root.path(), RUN);
    let document = json_report(root.path(), RUN);

    assert_eq!(
        store_state(root.path()),
        before,
        "reports change no stored byte"
    );
    assert_eq!(
        document["account"]["revision_ids"].as_array().map(Vec::len),
        Some(3)
    );
    assert!(text.contains("chain: quota-r1, quota-r2, quota-r3"));
}

#[test]
fn assessment_input_with_quota_fields_fails_and_changes_no_factual_byte() {
    let root = seeded_repository();
    let snapshot_id = json_report(root.path(), RUN)["snapshot"]["snapshot_id"]
        .as_str()
        .expect("snapshot id is a string")
        .to_owned();
    let before = store_state(root.path());
    let input_path = root.path().join("judgment.json");

    for field in [
        "quota",
        "quota_snapshot",
        "account_capacity",
        "quota_references",
    ] {
        let mut judgment = json!({
            "useful_work_rule": "one reviewed task",
            "author": "reviewer",
            "session_id": "session-a",
            "model": "model-a",
            "assignments": [],
            "residuals": [],
        });
        judgment[field] = json!({ "used": 58 });
        fs::write(&input_path, judgment.to_string()).expect("input writes");

        let output = gal(
            root.path(),
            &[
                "usage",
                "assess",
                "--run",
                RUN,
                "--snapshot",
                &snapshot_id,
                "--input",
                input_path.to_str().expect("path is UTF-8"),
            ],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(USAGE_EXIT), "{stderr}");
        assert!(stderr.contains(field), "{stderr}");
        assert_eq!(
            store_state(root.path()),
            before,
            "a rejected quota write changes no stored byte"
        );
    }
}

#[test]
fn run_without_quota_reports_the_missing_revision_in_text_and_json() {
    let root = repository();
    let store = store_at(root.path());
    usage_capture::capture_attempt(&store, PLAIN_RUN, &output_usage("attempt-p", 7), NOW)
        .expect("usage captures");
    drop(store);

    let document = json_report(root.path(), PLAIN_RUN);
    assert_eq!(document["account"]["revision_id"], Value::Null);
    assert_eq!(document["account"]["revision_ids"], json!([]));
    assert_eq!(document["account"]["reason"], NO_QUOTA);
    assert_eq!(document["account"]["scopes"], json!([]));

    let text = text_report(root.path(), PLAIN_RUN);
    assert!(
        text.contains(&format!("  unavailable: {NO_QUOTA}")),
        "{text}"
    );
    assert!(text.contains("run tokens.output: total=7"), "{text}");
}
