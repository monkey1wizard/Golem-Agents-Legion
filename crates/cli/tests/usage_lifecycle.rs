//! Accounting survives pipeline cleanup and reports without injected rebuild functions.
//!
//! The durable usage store lives under the Git common directory, so the real
//! `gal pipeline-clean` must remove only the transient scope and leave the store intact.

#[path = "../src/commands/usage_capture.rs"]
#[allow(dead_code)]
mod usage_capture;

use pipeline::usage::UsageStore;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;
use usage_capture::AttemptUsage;

const RUN: &str = "run-lifecycle";
const SCOPE: &str = "feat-lifecycle";
const NOW: u64 = 1_792_000_000_000;

fn gal(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("gal runs")
}

fn git_init(root: &Path) {
    let status = Command::new("git")
        .args(["init", "-q"])
        .current_dir(root)
        .status()
        .expect("git runs");
    assert!(status.success(), "git init succeeds");
}

fn output_attempt(attempt_id: &str, output: u64) -> AttemptUsage {
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

#[test]
fn failed_and_resumed_attempts_survive_pipeline_clean_and_report_total_160() {
    let root = TempDir::new().expect("temp repo");
    git_init(root.path());
    let transient = root
        .path()
        .join(".dev")
        .join("pipeline")
        .join(SCOPE)
        .join("task-a");
    fs::create_dir_all(&transient).expect("transient scope exists");
    fs::write(
        transient.join("task-a-implement.log"),
        "transient executor output\n",
    )
    .expect("transient file writes");

    let store = UsageStore::open(&root.path().join(".git")).expect("store opens under .git");
    usage_capture::capture_attempt(&store, RUN, &output_attempt("attempt-failed", 100), NOW)
        .expect("failed attempt captures");
    usage_capture::capture_attempt(
        &store,
        RUN,
        &output_attempt("attempt-resumed", 60),
        NOW + 60_000,
    )
    .expect("resumed attempt captures");
    drop(store);

    let clean = gal(
        root.path(),
        &["pipeline-clean", &format!(".dev/plans/{SCOPE}.prompt.md")],
    );
    assert!(
        clean.status.success(),
        "pipeline-clean succeeds: {}",
        String::from_utf8_lossy(&clean.stderr)
    );
    assert!(!transient.exists(), "the transient scope is removed");

    let report = gal(root.path(), &["usage", "report", "--run", RUN]);
    let text = String::from_utf8_lossy(&report.stdout);
    assert!(report.status.success(), "report succeeds: {text}");
    assert!(
        text.contains("160"),
        "the factual total survives cleanup: {text}"
    );
    assert!(
        !text.contains("snapshot manifests"),
        "no injected rebuild is needed: {text}"
    );

    let json_report = gal(root.path(), &["usage", "report", "--run", RUN, "--json"]);
    assert!(json_report.status.success());
    let document: serde_json::Value =
        serde_json::from_slice(&json_report.stdout).expect("report is one JSON document");
    assert_eq!(document["run_id"], RUN);
}

#[test]
fn worktree_removal_and_cleanup_preserve_store_and_both_assessment_versions() {
    let root = TempDir::new().expect("temp repo");
    git_init(root.path());
    let _ = Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(root.path())
        .status();
    let _ = Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(root.path())
        .status();
    let commit = Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(root.path())
        .status()
        .expect("git commit succeeds");
    assert!(commit.success());

    let transient = root
        .path()
        .join(".dev")
        .join("pipeline")
        .join(SCOPE)
        .join("task-a");
    fs::create_dir_all(&transient).expect("transient scope exists");
    fs::write(
        transient.join("task-a-implement.log"),
        "transient executor output\n",
    )
    .expect("transient file writes");

    let store = UsageStore::open(&root.path().join(".git")).expect("store opens under .git");
    usage_capture::capture_attempt(&store, RUN, &output_attempt("attempt-failed", 100), NOW)
        .expect("failed attempt captures");
    let resumed = usage_capture::capture_attempt(
        &store,
        RUN,
        &output_attempt("attempt-resumed", 60),
        NOW + 60_000,
    )
    .expect("resumed attempt captures");
    let snapshot_id = resumed
        .snapshot_id
        .expect("resumed attempt publishes a snapshot");

    let base_input = json!({
        "useful_work_rule": "accepted only when the kept change reached the final diff",
        "author": "tester",
        "session_id": "session-1",
        "model": "test-model",
        "assignments": [
            {
                "contribution_id": "attempt-resumed-0000/output",
                "outcome": "accepted",
                "reason": "kept change reached final diff",
                "evidence": ["evidence-1"]
            },
            {
                "contribution_id": "attempt-failed-0000/output",
                "outcome": "unused",
                "reason": "attempt failed and was replaced",
                "evidence": ["evidence-1"]
            }
        ],
        "residuals": []
    });
    let base_path = root.path().join("base_assessment.json");
    fs::write(&base_path, serde_json::to_vec(&base_input).unwrap()).unwrap();
    let assess1 = gal(
        root.path(),
        &[
            "usage",
            "assess",
            "--run",
            RUN,
            "--snapshot",
            &snapshot_id,
            "--input",
            base_path.to_str().unwrap(),
        ],
    );
    assert!(
        assess1.status.success(),
        "assess 1 succeeds: {}",
        String::from_utf8_lossy(&assess1.stderr)
    );
    let out1 = String::from_utf8_lossy(&assess1.stdout);
    let assess1_id = out1
        .lines()
        .find(|l| l.starts_with("assessment:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap()
        .to_owned();

    let rev_input = json!({
        "useful_work_rule": "revised rule",
        "author": "tester",
        "session_id": "session-1",
        "model": "test-model",
        "supersedes": assess1_id,
        "assignments": [
            {
                "contribution_id": "attempt-resumed-0000/output",
                "outcome": "support",
                "reason": "informed the fix",
                "evidence": ["evidence-2"]
            },
            {
                "contribution_id": "attempt-failed-0000/output",
                "outcome": "unused",
                "reason": "attempt failed and was replaced",
                "evidence": ["evidence-2"]
            }
        ],
        "residuals": []
    });
    let rev_path = root.path().join("rev_assessment.json");
    fs::write(&rev_path, serde_json::to_vec(&rev_input).unwrap()).unwrap();
    let assess2 = gal(
        root.path(),
        &[
            "usage",
            "assess",
            "--run",
            RUN,
            "--snapshot",
            &snapshot_id,
            "--input",
            rev_path.to_str().unwrap(),
        ],
    );
    assert!(
        assess2.status.success(),
        "assess 2 succeeds: {}",
        String::from_utf8_lossy(&assess2.stderr)
    );
    let out2 = String::from_utf8_lossy(&assess2.stdout);
    let assess2_id = out2
        .lines()
        .find(|l| l.starts_with("assessment:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap()
        .to_owned();

    let snap_path = root
        .path()
        .join(".git")
        .join("gal")
        .join("usage")
        .join("v1")
        .join(RUN)
        .join("snapshots")
        .join(format!("{snapshot_id}.record.json"));
    let assess1_path = root
        .path()
        .join(".git")
        .join("gal")
        .join("usage")
        .join("v1")
        .join(RUN)
        .join("assessments")
        .join(format!("{assess1_id}.record.json"));
    let assess2_path = root
        .path()
        .join(".git")
        .join("gal")
        .join("usage")
        .join("v1")
        .join(RUN)
        .join("assessments")
        .join(format!("{assess2_id}.record.json"));

    let snap_bytes_before = fs::read(&snap_path).expect("snapshot record exists");
    let assess1_bytes_before = fs::read(&assess1_path).expect("assessment 1 record exists");
    let assess2_bytes_before = fs::read(&assess2_path).expect("assessment 2 record exists");

    let wt_dir = TempDir::new().expect("temp worktree");
    let wt_path = wt_dir.path().to_str().unwrap();
    let wt_add = Command::new("git")
        .args(["worktree", "add", wt_path, "-b", "branch-wt"])
        .current_dir(root.path())
        .output()
        .expect("git worktree add");
    assert!(
        wt_add.status.success(),
        "worktree add succeeds: {}",
        String::from_utf8_lossy(&wt_add.stderr)
    );

    let wt_report = gal(wt_dir.path(), &["usage", "report", "--run", RUN]);
    assert!(
        wt_report.status.success(),
        "report inside linked worktree succeeds: {}",
        String::from_utf8_lossy(&wt_report.stderr)
    );
    assert!(String::from_utf8_lossy(&wt_report.stdout).contains("160"));

    let wt_remove = Command::new("git")
        .args(["worktree", "remove", "--force", wt_path])
        .current_dir(root.path())
        .output()
        .expect("git worktree remove");
    assert!(
        wt_remove.status.success(),
        "worktree remove succeeds: {}",
        String::from_utf8_lossy(&wt_remove.stderr)
    );

    let clean = gal(
        root.path(),
        &["pipeline-clean", &format!(".dev/plans/{SCOPE}.prompt.md")],
    );
    assert!(clean.status.success());
    assert!(!transient.exists());

    let rep_default = gal(root.path(), &["usage", "report", "--run", RUN]);
    assert!(rep_default.status.success());
    let rep_def_out = String::from_utf8_lossy(&rep_default.stdout);
    assert!(rep_def_out.contains("160"));
    assert!(rep_def_out.contains(&assess2_id));

    let rep_a1 = gal(
        root.path(),
        &["usage", "report", "--run", RUN, "--assessment", &assess1_id],
    );
    assert!(rep_a1.status.success());
    let rep_a1_out = String::from_utf8_lossy(&rep_a1.stdout);
    assert!(rep_a1_out.contains(&assess1_id));

    let snap_bytes_after = fs::read(&snap_path).expect("snapshot record exists");
    let assess1_bytes_after = fs::read(&assess1_path).expect("assessment 1 record exists");
    let assess2_bytes_after = fs::read(&assess2_path).expect("assessment 2 record exists");
    assert_eq!(snap_bytes_before, snap_bytes_after);
    assert_eq!(assess1_bytes_before, assess1_bytes_after);
    assert_eq!(assess2_bytes_before, assess2_bytes_after);
}

#[test]
fn failed_rebuild_marks_report_as_integrity_failure_and_never_shows_zero() {
    let root = TempDir::new().expect("temp repo");
    git_init(root.path());

    let store = UsageStore::open(&root.path().join(".git")).expect("store opens under .git");
    let _failed =
        usage_capture::capture_attempt(&store, RUN, &output_attempt("attempt-failed", 100), NOW)
            .expect("failed attempt captures");
    let resumed = usage_capture::capture_attempt(
        &store,
        RUN,
        &output_attempt("attempt-resumed", 60),
        NOW + 60_000,
    )
    .expect("resumed attempt captures");
    let snapshot_id = resumed
        .snapshot_id
        .expect("resumed attempt publishes a snapshot");
    drop(store);

    let report_before = gal(root.path(), &["usage", "report", "--run", RUN]);
    assert!(report_before.status.success());
    assert!(String::from_utf8_lossy(&report_before.stdout).contains("160"));

    let evidence_record = root
        .path()
        .join(".git")
        .join("gal")
        .join("usage")
        .join("v1")
        .join(RUN)
        .join("evidence")
        .join(format!("{snapshot_id}.record.json"));
    fs::write(&evidence_record, b"{\"tampered\": true}").expect("evidence tampered");

    let report_after = gal(root.path(), &["usage", "report", "--run", RUN]);
    let stdout = String::from_utf8_lossy(&report_after.stdout);
    assert!(
        stdout.contains("unavailable"),
        "tampered evidence makes factuals unavailable without panic: {stdout}"
    );
    assert!(
        !stdout.contains("total: 0") && !stdout.contains("0 tokens"),
        "the failed rebuild never shows zero: {stdout}"
    );

    let snap_record = root
        .path()
        .join(".git")
        .join("gal")
        .join("usage")
        .join("v1")
        .join(RUN)
        .join("snapshots")
        .join(format!("{snapshot_id}.record.json"));
    fs::write(&snap_record, b"{}").expect("corrupt snapshot");

    let report_corrupt = gal(root.path(), &["usage", "report", "--run", RUN]);
    assert!(
        !report_corrupt.status.success(),
        "a corrupt snapshot is an integrity failure"
    );
    let corrupt_stdout = String::from_utf8_lossy(&report_corrupt.stdout);
    let corrupt_stderr = String::from_utf8_lossy(&report_corrupt.stderr);
    assert!(
        corrupt_stderr.contains("gal usage:"),
        "integrity error is printed on stderr: {corrupt_stderr}"
    );
    assert!(
        !corrupt_stdout.contains("total: 0") && !corrupt_stdout.contains("0 tokens"),
        "the corrupt integrity failure never shows zero: {corrupt_stdout}"
    );
}
