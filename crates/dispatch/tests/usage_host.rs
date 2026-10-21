use dispatch::dispatch::DispatchQuota;
use dispatch::host_session::{HostKind, HostSessionError, HostSessionRef};
use dispatch::host_usage::{
    collect, unverified_host_observation, HostRoots, HostUsageCursor, HostUsageError,
    HostUsagePage, HostUsageRole, PageStatus, UsageWindow,
};
use dispatch::usage::{Availability, TokenMetric, UsageObservation};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const FIXTURES: &str = include_str!("fixtures/usage_host.json");
const CODEX_CHILD_ATTEMPT: &str = "attempt-7f30";
const CLAUDE_CHILD_ATTEMPT: &str = "attempt-claude-1";
const WINDOW_START: &str = "2026-10-20T09:00:00.000Z";
const WINDOW_END: &str = "2026-10-20T10:00:00.000Z";

/// A home directory, a working directory, and the roots that point into them.
struct Sandbox {
    _dir: TempDir,
    work: PathBuf,
    roots: HostRoots,
}

impl Sandbox {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let home = dir.path().join("home");
        let work = dir.path().join("work");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&work).unwrap();
        let roots = HostRoots {
            codex_root: home.join(".codex"),
            home,
        };
        Self {
            _dir: dir,
            work,
            roots,
        }
    }

    fn cwd(&self) -> String {
        self.work.to_string_lossy().into_owned()
    }

    /// A rollout whose file name ends with `-<name_id>.jsonl`, as the locator expects.
    fn codex_rollout(&self, name_id: &str) -> PathBuf {
        let dir = self.roots.codex_root.join("sessions/2026/10/20");
        fs::create_dir_all(&dir).unwrap();
        dir.join(format!("rollout-2026-10-20T09-00-00-{name_id}.jsonl"))
    }

    fn claude_child(&self) -> PathBuf {
        let dir = self
            .roots
            .home
            .join(".claude/projects/hash/instance/subagents");
        fs::create_dir_all(&dir).unwrap();
        dir.join("agent-a1.jsonl")
    }

    fn claude_main(&self, session: &str) -> PathBuf {
        let dir = self.roots.home.join(".claude/projects/hash");
        fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{session}.jsonl"))
    }
}

fn fill_cwd(value: &mut Value, cwd: &str) {
    match value {
        Value::String(text) if text.as_str() == "{CWD}" => *text = cwd.to_owned(),
        Value::Array(items) => items.iter_mut().for_each(|item| fill_cwd(item, cwd)),
        Value::Object(map) => map.values_mut().for_each(|item| fill_cwd(item, cwd)),
        _ => {}
    }
}

/// Rows of one named fixture case, with the working directory filled in.
fn fixture_rows(case: &str, cwd: &str) -> Vec<Value> {
    let fixtures: Value = serde_json::from_str(FIXTURES).expect("fixture file is valid JSON");
    let mut rows = fixtures[case]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("fixture {case} has no rows"))
        .clone();
    for row in &mut rows {
        fill_cwd(row, cwd);
    }
    rows
}

fn write_rows(path: &Path, rows: &[Value]) {
    let text: String = rows.iter().map(|row| format!("{row}\n")).collect();
    fs::write(path, text).unwrap();
}

fn append_rows(path: &Path, rows: &[Value]) {
    let mut file = OpenOptions::new().append(true).open(path).unwrap();
    for row in rows {
        writeln!(file, "{row}").unwrap();
    }
}

fn codex_child_ref() -> HostSessionRef {
    HostSessionRef::parse("codex:rollout-child").unwrap()
}

fn child(attempt: &str) -> HostUsageRole {
    HostUsageRole::Child {
        attempt_id: attempt.to_owned(),
    }
}

fn window() -> UsageWindow {
    UsageWindow::closed(WINDOW_START, WINDOW_END).unwrap()
}

fn counter(metric: &TokenMetric) -> u64 {
    match &metric.count.value {
        Availability::Reported { value } => value.get(),
        other => panic!("expected a reported counter, got {other:?}"),
    }
}

fn only(page: &HostUsagePage) -> &UsageObservation {
    assert_eq!(page.observations.len(), 1, "expected one observation");
    &page.observations[0]
}

fn assert_unsupported(metric: &TokenMetric, name: &str) {
    assert!(
        matches!(metric.count.value, Availability::Unsupported { .. }),
        "{name} must be unsupported, got {:?}",
        metric.count.value
    );
}

#[test]
fn codex_child_window_counts_only_bound_turns() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    write_rows(
        &sandbox.codex_rollout("rollout-child"),
        &fixture_rows("codex_child", &cwd),
    );

    let page = collect(
        &codex_child_ref(),
        &child(CODEX_CHILD_ATTEMPT),
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    )
    .expect("bound child collects");
    let observation = only(&page);
    let tokens = &observation.tokens;
    println!(
        "codex child: input={} cached={} output={} reasoning={} cache_write={} pending={} status={:?}",
        counter(&tokens.input),
        counter(&tokens.cached_input),
        counter(&tokens.output),
        counter(&tokens.reasoning),
        counter(&tokens.cache_write),
        page.final_turn_pending,
        page.status
    );

    // Foreign work before the window (input 1000) and later work (input 9000) must not count.
    assert_eq!(counter(&tokens.input), 1000);
    assert_eq!(counter(&tokens.cached_input), 600);
    assert_eq!(counter(&tokens.output), 150);
    assert_eq!(counter(&tokens.reasoning), 30);
    assert_eq!(counter(&tokens.cache_write), 0);
    assert_eq!(observation.models.observed.as_deref(), Some("gpt-6-astra"));
    assert_eq!(page.status, PageStatus::Complete);
    assert!(
        page.final_turn_pending,
        "activity follows the last usage row"
    );
    // The child transcript names no verified account, so its quota is an explicit unknown scope.
    assert!(
        matches!(page.quota, DispatchQuota::UnknownScope { .. }),
        "host child quota must not inherit the controller account: {:?}",
        page.quota
    );
    assert_eq!(
        observation.identity.attempt_id.as_deref(),
        Some(CODEX_CHILD_ATTEMPT)
    );
    let serialized = serde_json::to_string(observation).unwrap();
    assert!(
        !serialized.contains("SECRET-CONVERSATION-TEXT"),
        "conversation text must not be copied"
    );
}

#[test]
fn codex_pending_final_turn_becomes_a_later_observation() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    let path = sandbox.codex_rollout("rollout-child");
    write_rows(&path, &fixture_rows("codex_child_pending", &cwd));
    let role = child(CODEX_CHILD_ATTEMPT);

    let first = collect(
        &codex_child_ref(),
        &role,
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    )
    .expect("first collection");
    let first_tokens = &only(&first).tokens;
    println!(
        "first collection: input={} cached={} output={} pending={}",
        counter(&first_tokens.input),
        counter(&first_tokens.cached_input),
        counter(&first_tokens.output),
        first.final_turn_pending
    );
    assert_eq!(counter(&first_tokens.input), 1000);
    assert!(first.final_turn_pending);

    append_rows(&path, &fixture_rows("codex_turn_appended", &cwd));
    let second = collect(
        &codex_child_ref(),
        &role,
        &sandbox.work,
        &sandbox.roots,
        &window(),
        Some(&first.cursor),
    )
    .expect("later collection resumes from the cursor");
    let second_tokens = &only(&second).tokens;
    println!(
        "later collection: input={} cached={} output={} reasoning={} pending={}",
        counter(&second_tokens.input),
        counter(&second_tokens.cached_input),
        counter(&second_tokens.output),
        counter(&second_tokens.reasoning),
        second.final_turn_pending
    );
    assert_eq!(counter(&second_tokens.input), 500);
    assert_eq!(counter(&second_tokens.cached_input), 100);
    assert_eq!(counter(&second_tokens.output), 50);
    assert_eq!(counter(&second_tokens.reasoning), 10);
    assert!(!second.final_turn_pending);
    assert!(second.cursor.offset > first.cursor.offset);
}

#[test]
fn codex_orchestrator_reads_its_own_rollout_without_attempt_token() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    write_rows(
        &sandbox.codex_rollout("rollout-child"),
        &fixture_rows("codex_child", &cwd),
    );
    let reference = HostSessionRef::parse("codex:rollout-child").unwrap();

    let page = collect(
        &reference,
        &HostUsageRole::Orchestrator,
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    )
    .expect("orchestrator collects");
    assert_eq!(counter(&only(&page).tokens.input), 1000);
    assert_eq!(only(&page).identity.attempt_id, None);
}

#[test]
fn codex_identity_and_lineage_mismatches_are_rejected() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    write_rows(
        &sandbox.codex_rollout("rollout-child"),
        &fixture_rows("codex_child", &cwd),
    );

    let wrong_attempt = collect(
        &codex_child_ref(),
        &child("attempt-other"),
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    );
    assert_eq!(
        wrong_attempt,
        Err(HostUsageError::Session(
            HostSessionError::AttemptIdNotInUserMessage
        ))
    );

    // A rollout whose file name claims `thread-x` but whose session ID is `rollout-child`.
    let other = Sandbox::new();
    let other_cwd = other.cwd();
    write_rows(
        &other.codex_rollout("thread-x"),
        &fixture_rows("codex_child", &other_cwd),
    );
    let orchestrator_mismatch = collect(
        &HostSessionRef::parse("codex:thread-x").unwrap(),
        &HostUsageRole::Orchestrator,
        &other.work,
        &other.roots,
        &window(),
        None,
    );
    assert_eq!(orchestrator_mismatch, Err(HostUsageError::IdentityMismatch));

    let stale_cursor = HostUsageCursor {
        epoch: "codex:other".to_owned(),
        offset: 0,
        through_unix_ms: 0,
        totals: None,
        activity_after_usage: false,
    };
    let cursor_mismatch = collect(
        &codex_child_ref(),
        &child(CODEX_CHILD_ATTEMPT),
        &sandbox.work,
        &sandbox.roots,
        &window(),
        Some(&stale_cursor),
    );
    assert_eq!(cursor_mismatch, Err(HostUsageError::IdentityMismatch));

    let elsewhere = sandbox.roots.home.join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    let cwd_mismatch = collect(
        &codex_child_ref(),
        &child(CODEX_CHILD_ATTEMPT),
        &elsewhere,
        &sandbox.roots,
        &window(),
        None,
    );
    assert_eq!(
        cwd_mismatch,
        Err(HostUsageError::Session(HostSessionError::CwdMismatch))
    );
}

#[test]
fn codex_missing_baseline_is_rejected() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    let mut rows = fixture_rows("codex_child_pending", &cwd);
    // Row 4 is the first in-window usage row. Without its last-turn counters the baseline is unknown.
    rows[4]["payload"]["info"]
        .as_object_mut()
        .unwrap()
        .remove("last_token_usage");
    write_rows(&sandbox.codex_rollout("rollout-child"), &rows);

    let result = collect(
        &codex_child_ref(),
        &child(CODEX_CHILD_ATTEMPT),
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    );
    assert_eq!(result, Err(HostUsageError::MissingBaseline));
}

#[test]
fn codex_reset_assumption_is_rejected() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    let mut rows = fixture_rows("codex_child_pending", &cwd);
    // Row 6 should total 2150. A value of 2000 means the session counters reset or changed.
    rows[6]["payload"]["info"]["total_token_usage"]["total_tokens"] = json!(2000);
    write_rows(&sandbox.codex_rollout("rollout-child"), &rows);

    let result = collect(
        &codex_child_ref(),
        &child(CODEX_CHILD_ATTEMPT),
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    );
    assert_eq!(result, Err(HostUsageError::ResetAssumption));
}

#[test]
fn codex_budget_stop_is_partial_and_the_next_page_resumes() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    let mut rows = fixture_rows("codex_child_pending", &cwd);
    // Keep the header and usage rows, and fill the middle with activity so the first page exhausts its record budget.
    let usage_rows = rows.split_off(2);
    rows.extend((0..10_005).map(|_| {
        json!({"timestamp": "2026-10-20T09:00:30.000Z", "type": "response_item", "payload": {"type": "function_call_output", "output": "ok"}})
    }));
    rows.extend(usage_rows);
    let path = sandbox.codex_rollout("rollout-child");
    write_rows(&path, &rows);
    let role = child(CODEX_CHILD_ATTEMPT);

    let first = collect(
        &codex_child_ref(),
        &role,
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    )
    .expect("first page");
    println!(
        "budget page 1: status={:?} observations={} offset={} of {}",
        first.status,
        first.observations.len(),
        first.cursor.offset,
        fs::metadata(&path).unwrap().len()
    );
    assert_eq!(first.status, PageStatus::BudgetStop);
    assert!(
        first.observations.is_empty(),
        "no usage row fits the first page"
    );
    assert!(first.cursor.offset < fs::metadata(&path).unwrap().len());

    let second = collect(
        &codex_child_ref(),
        &role,
        &sandbox.work,
        &sandbox.roots,
        &window(),
        Some(&first.cursor),
    )
    .expect("second page");
    assert_eq!(second.status, PageStatus::Complete);
    assert_eq!(counter(&only(&second).tokens.input), 1000);
    assert_eq!(counter(&only(&second).tokens.cached_input), 600);
}

#[test]
fn codex_byte_budget_stops_before_a_record_that_exceeds_it() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    let mut rows = fixture_rows("codex_child_pending", &cwd);
    let padding = "x".repeat(9 * 1024 * 1024);
    let usage_rows = rows.split_off(2);
    for _ in 0..2 {
        rows.push(json!({"timestamp": "2026-10-20T09:00:30.000Z", "type": "response_item", "payload": {"type": "function_call_output", "output": padding}}));
    }
    rows.extend(usage_rows);
    let path = sandbox.codex_rollout("rollout-child");
    write_rows(&path, &rows);

    let page = collect(
        &codex_child_ref(),
        &child(CODEX_CHILD_ATTEMPT),
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    )
    .expect("page stops at the byte budget");
    println!(
        "byte budget: status={:?} offset={} file={}",
        page.status,
        page.cursor.offset,
        fs::metadata(&path).unwrap().len()
    );
    assert_eq!(page.status, PageStatus::BudgetStop);
    assert!(page.cursor.offset > 9 * 1024 * 1024);
    assert!(page.cursor.offset < 10 * 1024 * 1024 + 4096);
}

#[test]
fn zero_turn_is_reported_zero_and_absent_usage_is_empty() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    let zero = json!({"timestamp": "2026-10-20T09:00:10.000Z", "type": "event_msg", "payload": {"type": "token_count", "info": {
        "total_token_usage": {"input_tokens": 0, "cached_input_tokens": 0, "output_tokens": 0, "reasoning_output_tokens": 0, "cache_write_input_tokens": 0, "total_tokens": 0},
        "last_token_usage": {"input_tokens": 0, "cached_input_tokens": 0, "output_tokens": 0, "reasoning_output_tokens": 0, "cache_write_input_tokens": 0, "total_tokens": 0}}}});
    let mut rows = fixture_rows("codex_child", &cwd);
    rows.truncate(2);
    rows.push(zero);
    write_rows(&sandbox.codex_rollout("rollout-child"), &rows);
    let page = collect(
        &codex_child_ref(),
        &child(CODEX_CHILD_ATTEMPT),
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    )
    .expect("zero turn collects");
    println!("zero turn: input={}", counter(&only(&page).tokens.input));
    assert_eq!(counter(&only(&page).tokens.input), 0);

    let quiet = Sandbox::new();
    let quiet_cwd = quiet.cwd();
    let mut quiet_rows = fixture_rows("codex_child", &quiet_cwd);
    quiet_rows.truncate(2);
    write_rows(&quiet.codex_rollout("rollout-child"), &quiet_rows);
    let empty = collect(
        &codex_child_ref(),
        &child(CODEX_CHILD_ATTEMPT),
        &quiet.work,
        &quiet.roots,
        &window(),
        None,
    )
    .expect("session without usage collects");
    println!("no usage rows: observations={}", empty.observations.len());
    assert!(empty.observations.is_empty());
    assert!(!empty.final_turn_pending);
}

#[test]
fn claude_child_counts_each_response_once_and_holds_back_the_trailing_one() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    let path = sandbox.claude_child();
    write_rows(&path, &fixture_rows("claude_child", &cwd));
    let reference = HostSessionRef::parse("claude:a1").unwrap();
    let role = child(CLAUDE_CHILD_ATTEMPT);

    let first = collect(
        &reference,
        &role,
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    )
    .expect("first collection");
    let tokens = &only(&first).tokens;
    println!(
        "claude first: input={} output={} cache_read={} cache_creation={} pending={} model={:?}",
        counter(&tokens.input),
        counter(&tokens.output),
        counter(&tokens.cached_input),
        counter(&tokens.cache_write),
        first.final_turn_pending,
        only(&first).models.observed
    );
    // m1 is counted once although two rows carry it. m0 is before the window, and m3 is trailing.
    assert_eq!(counter(&tokens.input), 8);
    assert_eq!(counter(&tokens.output), 30);
    assert_eq!(counter(&tokens.cached_input), 100);
    assert_eq!(counter(&tokens.cache_write), 50);
    assert_eq!(
        only(&first).models.observed.as_deref(),
        Some("claude-opus-5")
    );
    assert!(first.final_turn_pending);

    append_rows(&path, &fixture_rows("claude_child_appended", &cwd));
    let second = collect(
        &reference,
        &role,
        &sandbox.work,
        &sandbox.roots,
        &window(),
        Some(&first.cursor),
    )
    .expect("later collection");
    let second_tokens = &only(&second).tokens;
    println!(
        "claude later: input={} output={} pending={}",
        counter(&second_tokens.input),
        counter(&second_tokens.output),
        second.final_turn_pending
    );
    assert_eq!(counter(&second_tokens.input), 7);
    assert_eq!(counter(&second_tokens.output), 1);
    assert!(second.final_turn_pending);
}

#[test]
fn claude_child_rejects_a_missing_attempt_token() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    write_rows(&sandbox.claude_child(), &fixture_rows("claude_child", &cwd));
    let result = collect(
        &HostSessionRef::parse("claude:a1").unwrap(),
        &child("attempt-other"),
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    );
    assert_eq!(
        result,
        Err(HostUsageError::Session(
            HostSessionError::AttemptIdNotInUserMessage
        ))
    );
}

#[test]
fn claude_orchestrator_reads_the_main_transcript() {
    let sandbox = Sandbox::new();
    let cwd = sandbox.cwd();
    write_rows(
        &sandbox.claude_main("sess-1"),
        &fixture_rows("claude_main", &cwd),
    );
    let page = collect(
        &HostSessionRef::parse("claude:sess-1").unwrap(),
        &HostUsageRole::Orchestrator,
        &sandbox.work,
        &sandbox.roots,
        &window(),
        None,
    )
    .expect("orchestrator transcript collects");
    let tokens = &only(&page).tokens;
    println!(
        "claude orchestrator: input={} output={} pending={}",
        counter(&tokens.input),
        counter(&tokens.output),
        page.final_turn_pending
    );
    assert_eq!(counter(&tokens.input), 4);
    assert_eq!(counter(&tokens.output), 4);
    assert!(page.final_turn_pending);
}

#[test]
fn references_that_could_escape_the_record_root_are_rejected_before_lookup() {
    let sandbox = Sandbox::new();
    let cases = [
        (HostKind::Claude, "../x", HostSessionError::InvalidReference),
        (HostKind::Claude, "a/b", HostSessionError::InvalidReference),
        (
            HostKind::Codex,
            "../x",
            HostSessionError::InvalidCodexReference,
        ),
    ];
    for (host, reference, expected) in cases {
        let session = HostSessionRef {
            host,
            reference: reference.to_owned(),
        };
        assert_eq!(
            collect(
                &session,
                &child(CODEX_CHILD_ATTEMPT),
                &sandbox.work,
                &sandbox.roots,
                &window(),
                None,
            ),
            Err(HostUsageError::Session(expected)),
            "{reference:?} must be rejected"
        );
    }
}

#[test]
fn unverified_host_reports_unsupported_counters_not_zero() {
    let observation = unverified_host_observation("opencode", "no verified host telemetry source");
    let tokens = &observation.tokens;
    println!("opencode: input={:?}", tokens.input.count.value);
    for (metric, name) in [
        (&tokens.input, "input"),
        (&tokens.output, "output"),
        (&tokens.cached_input, "cached_input"),
        (&tokens.cache_write, "cache_write"),
        (&tokens.reasoning, "reasoning"),
        (&tokens.other, "other"),
    ] {
        assert_unsupported(metric, name);
    }
    assert!(matches!(
        observation.cost.value,
        Availability::Unsupported { .. }
    ));
    assert_eq!(observation.identity.executor.as_deref(), Some("opencode"));
}
