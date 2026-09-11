//! The pure test-first evidence evaluator.
//!
//! This module deliberately consumes already-read values.  Filesystem identity,
//! snapshot, and transition checks belong to their respective owners; this
//! module only validates the canonical values and their relationships.

use std::collections::{BTreeMap, BTreeSet};

use crate::task_spec::probe_evidence::{
    verify_output_envelope, FailureClass, ProbeReceipt, ProbeRecord, Termination,
};
use crate::task_spec::{
    decode_base64url_unpadded, extract_generation_ledger, has_test_first_marker, Applicability,
    TaskContract,
};
use thiserror::Error;

pub const REQUIRED_PHASES: &[&str] = &["scaffold", "test", "implement", "green-rerun", "audit"];
pub const NOT_APPLICABLE_PHASES: &[&str] = &["test", "implement", "audit"];
pub const LEGACY_PHASES: &[&str] = &["implement", "test", "audit"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractMode {
    Legacy,
    TestFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationVerdict {
    Pass,
    NotRun,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TestFirstEvidenceError {
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedFailure {
    pub reference: String,
    pub class: FailureClass,
    pub termination: ExpectedTermination,
    pub stream: ExpectedStream,
    pub matcher: String,
    pub prose: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedTermination {
    Nonzero,
    Exit(i32),
    Signal(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedStream {
    Stdout,
    Stderr,
    Combined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisputeRecord {
    pub dispute_id: String,
    pub generation: u32,
    pub contract_digest: String,
    pub basis: String,
    pub classification: String,
    pub status: String,
    pub retry_before: u32,
    pub retry_after: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseEvidence {
    pub phase: String,
    pub completed: bool,
}

#[derive(Debug, Clone)]
pub struct EvaluationInput<'a> {
    pub plan_slug: &'a str,
    pub task_id: &'a str,
    pub prompt_body: &'a str,
    pub contract: Option<&'a TaskContract>,
    pub receipts: &'a [ProbeReceipt],
    pub output_envelopes: &'a BTreeMap<String, Vec<u8>>,
    pub disputes: &'a [String],
    pub retry_count: u32,
    pub phases: &'a [PhaseEvidence],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestFirstEvaluation {
    pub mode: ContractMode,
    pub verdict: EvaluationVerdict,
    pub plan_slug: String,
    pub task_id: String,
    pub generation: Option<u32>,
    pub contract_digest: Option<String>,
    pub probe_set_digest: Option<String>,
    pub completed_phases: Vec<String>,
    pub open_disputes: Vec<String>,
    pub retry_count: u32,
    pub errors: Vec<String>,
}

impl TestFirstEvaluation {
    pub fn is_pass(&self) -> bool {
        self.verdict == EvaluationVerdict::Pass
    }

    pub fn evaluate(input: &EvaluationInput<'_>) -> Self {
        let mode = if has_test_first_marker(input.prompt_body) {
            ContractMode::TestFirst
        } else {
            ContractMode::Legacy
        };
        let mut errors = Vec::new();
        let mut generation = None;
        let mut contract_digest = None;
        let mut probe_set_digest = None;

        if mode == ContractMode::TestFirst {
            match validate_generation(input.prompt_body, input.task_id, input.contract) {
                Ok((g, digest)) => {
                    generation = Some(g);
                    contract_digest = Some(digest);
                }
                Err(error) => errors.push(error.to_string()),
            }
            if input.contract.is_none() {
                errors.push("test-first task has no contract".to_string());
            }
        }

        let required = match mode {
            ContractMode::Legacy => LEGACY_PHASES,
            ContractMode::TestFirst => {
                match input.contract.map(|contract| contract.applicability) {
                    Some(Applicability::NotApplicable) => NOT_APPLICABLE_PHASES,
                    _ => REQUIRED_PHASES,
                }
            }
        };
        let completed_phases = validate_phase_order(input.phases, required, &mut errors);

        let open_disputes = validate_dispute_history(
            input.disputes,
            generation,
            contract_digest.as_deref(),
            input.retry_count,
            &mut errors,
        );

        if mode == ContractMode::TestFirst && errors.is_empty() {
            match validate_receipts(
                input,
                generation.unwrap(),
                contract_digest.as_deref().unwrap(),
            ) {
                Ok(digest) => probe_set_digest = digest,
                Err(error) => errors.push(error.to_string()),
            }
        }

        let verdict = if !errors.is_empty() {
            EvaluationVerdict::Fail
        } else if input.phases.iter().all(|p| !p.completed) {
            EvaluationVerdict::NotRun
        } else {
            EvaluationVerdict::Pass
        };

        Self {
            mode,
            verdict,
            plan_slug: input.plan_slug.to_string(),
            task_id: input.task_id.to_string(),
            generation,
            contract_digest,
            probe_set_digest,
            completed_phases,
            open_disputes,
            retry_count: input.retry_count,
            errors,
        }
    }
}

pub fn evaluate(input: &EvaluationInput<'_>) -> TestFirstEvaluation {
    TestFirstEvaluation::evaluate(input)
}

pub fn parse_expected_failure(line: &str) -> Result<ExpectedFailure, TestFirstEvidenceError> {
    let (head, prose) = line
        .split_once(" — ")
        .ok_or_else(|| invalid("expected failure must contain ' — '"))?;
    let head = head
        .strip_prefix('`')
        .and_then(|h| h.strip_suffix('`'))
        .unwrap_or(head);
    let fields: Vec<&str> = head.split(';').collect();
    if fields.len() != 5 || !fields[0].starts_with("EF-") {
        return Err(invalid("malformed expected failure header"));
    }
    let class = fields[1]
        .strip_prefix("class=")
        .ok_or_else(|| invalid("missing expected failure class"))?;
    let class = match class {
        "assertion" => FailureClass::Assertion,
        "error-code" => FailureClass::ErrorCode,
        "status" => FailureClass::Status,
        "exception" => FailureClass::Exception,
        "diagnostic" => FailureClass::Diagnostic,
        _ => return Err(invalid("unknown expected failure class")),
    };
    let termination = parse_expected_termination(
        fields[2]
            .strip_prefix("term=")
            .ok_or_else(|| invalid("missing expected failure termination"))?,
    )?;
    let stream = match fields[3]
        .strip_prefix("stream=")
        .ok_or_else(|| invalid("missing expected failure stream"))?
    {
        "stdout" => ExpectedStream::Stdout,
        "stderr" => ExpectedStream::Stderr,
        "combined" => ExpectedStream::Combined,
        _ => return Err(invalid("unknown expected failure stream")),
    };
    let matcher_b64 = fields[4]
        .strip_prefix("matcher_b64=")
        .ok_or_else(|| invalid("missing expected failure matcher"))?;
    let matcher = String::from_utf8(
        decode_base64url_unpadded(matcher_b64)
            .map_err(|e| invalid(&format!("invalid matcher base64: {e}")))?,
    )
    .map_err(|e| invalid(&format!("matcher is not UTF-8: {e}")))?;
    Ok(ExpectedFailure {
        reference: fields[0].to_string(),
        class,
        termination,
        stream,
        matcher,
        prose: prose.to_string(),
    })
}

pub fn validate_expected_failure(
    record: &ProbeRecord,
    expected: &ExpectedFailure,
    envelope: &[u8],
) -> Result<(), TestFirstEvidenceError> {
    if record.expected_failure_ref.as_deref() != Some(expected.reference.as_str()) {
        return Err(invalid(
            "record expected_failure_ref does not match expected failure",
        ));
    }
    if record.failure_class != Some(expected.class) {
        return Err(invalid(
            "record failure class does not match expected failure",
        ));
    }
    let term_ok = match (&expected.termination, &record.termination) {
        (ExpectedTermination::Nonzero, Termination::Exit(code)) => *code != 0,
        (ExpectedTermination::Exit(expected), Termination::Exit(actual)) => expected == actual,
        (ExpectedTermination::Signal(expected), Termination::Signal(actual)) => expected == actual,
        _ => false,
    };
    if !term_ok {
        return Err(invalid(
            "record termination does not match expected failure",
        ));
    }
    verify_output_envelope(envelope, &record.output_digest).map_err(|e| invalid(&e.to_string()))?;
    let (stdout, stderr) = split_output(envelope)?;
    let haystack = match expected.stream {
        ExpectedStream::Stdout => stdout,
        ExpectedStream::Stderr => stderr,
        ExpectedStream::Combined => [stdout.as_slice(), b"\n", stderr.as_slice()].concat(),
    };
    if !String::from_utf8_lossy(&haystack).contains(&expected.matcher) {
        return Err(invalid("expected failure matcher did not match output"));
    }
    Ok(())
}

pub fn compare_receipt_identities(
    red: &ProbeReceipt,
    green: &ProbeReceipt,
) -> Result<(), TestFirstEvidenceError> {
    if red.records.len() != green.records.len() {
        return Err(invalid("red and green record counts differ"));
    }
    let mut left = red
        .records
        .iter()
        .map(comparison_identity)
        .collect::<Result<Vec<_>, _>>()?;
    let mut right = green
        .records
        .iter()
        .map(comparison_identity)
        .collect::<Result<Vec<_>, _>>()?;
    left.sort();
    right.sort();
    if left != right {
        return Err(invalid("red and green probe identities differ"));
    }
    Ok(())
}

pub fn validate_phase_order(
    phases: &[PhaseEvidence],
    required: &[&str],
    errors: &mut Vec<String>,
) -> Vec<String> {
    let mut completed = Vec::new();
    // An empty phase list means the task has not been dispatched at all yet —
    // that is `NotRun`, not a gap. Only a non-empty-but-incomplete list (some
    // phase evidence exists, but not the full required sequence) is a genuine
    // gap: without this distinction `NotRun` can never be reached, because
    // every zero-evidence input would already carry a "gaps" error by the time
    // the caller checks whether any phase completed.
    if phases.is_empty() {
        return completed;
    }
    let mut next = 0;
    for phase in phases {
        if !required.contains(&phase.phase.as_str()) {
            errors.push(format!("unknown phase '{}',", phase.phase));
            continue;
        }
        if phase.completed {
            if required.get(next).copied() != Some(phase.phase.as_str()) {
                errors.push(format!("phase '{}' is out of order", phase.phase));
            } else {
                completed.push(phase.phase.clone());
                next += 1;
            }
        }
    }
    if next != required.len() {
        errors.push("phase evidence has gaps".to_string());
    }
    completed
}

pub fn validate_dispute_history(
    lines: &[String],
    generation: Option<u32>,
    digest: Option<&str>,
    retry: u32,
    errors: &mut Vec<String>,
) -> Vec<String> {
    let mut latest = BTreeMap::new();
    // "Monotonic dispute IDs" (R7) governs the order in which NEW ids are first
    // assigned, not the order of every line: the same id legitimately reappears
    // later to update status (open -> resolved), and "the latest row for an ID
    // controls status" only works if repeats are allowed. Track the newest
    // first-seen id separately from `latest`'s per-id history.
    let mut newest_first_seen_id: Option<String> = None;
    for line in lines {
        match parse_dispute(line) {
            Ok(record) => {
                if !latest.contains_key(&record.dispute_id) {
                    if let Some(previous) = &newest_first_seen_id {
                        if record.dispute_id <= *previous {
                            errors.push(format!(
                                "dispute IDs are not strictly increasing: {}",
                                record.dispute_id
                            ));
                        }
                    }
                    newest_first_seen_id = Some(record.dispute_id.clone());
                }
                if generation != Some(record.generation)
                    || digest != Some(record.contract_digest.as_str())
                {
                    errors.push(format!("stale dispute {}", record.dispute_id));
                }
                latest.insert(record.dispute_id.clone(), record);
            }
            Err(error) => errors.push(error.to_string()),
        }
    }
    let open = latest
        .values()
        .filter(|r| r.status == "open")
        .map(|r| r.dispute_id.clone())
        .collect::<Vec<_>>();
    for record in latest.values() {
        if record.status == "open"
            && record.retry_after != retry + 1
            && record.classification == "implementation-defect"
        {
            errors.push(format!(
                "invalid retry transition for {}",
                record.dispute_id
            ));
        }
    }
    open
}

fn validate_generation(
    prompt: &str,
    task_id: &str,
    contract: Option<&TaskContract>,
) -> Result<(u32, String), TestFirstEvidenceError> {
    let contract = contract.ok_or_else(|| invalid("missing task contract"))?;
    if contract.applicability != Applicability::Required
        && contract.applicability != Applicability::NotApplicable
    {
        return Err(invalid("invalid task applicability"));
    }
    let rows = extract_generation_ledger(prompt)
        .into_iter()
        .filter(|r| r.task_id == task_id)
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Err(invalid("missing generation ledger row"));
    }
    let contract_digest = contract.compute_digest();
    let mut seen = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        if !seen.insert(row.generation) {
            return Err(invalid("duplicate generation"));
        }
        if row.generation != seen.len() as u32 {
            return Err(invalid("generation gap or reorder"));
        }
        if row.reason != "init"
            && !matches!(
                row.reason.as_str(),
                "phase-rerun" | "probe-defect" | "contract-change"
            )
        {
            return Err(invalid("invalid generation reason"));
        }
        if index > 0
            && row.contract_digest != rows[index - 1].contract_digest
            && row.reason != "contract-change"
        {
            return Err(invalid("generation contract digest mismatch"));
        }
    }
    if rows.last().unwrap().contract_digest != contract_digest {
        return Err(invalid("generation contract digest mismatch"));
    }
    Ok((rows.last().unwrap().generation, contract_digest))
}

fn validate_receipts(
    input: &EvaluationInput<'_>,
    generation: u32,
    digest: &str,
) -> Result<Option<String>, TestFirstEvidenceError> {
    if input.receipts.is_empty() {
        return Err(invalid("missing probe receipt"));
    }
    let mut set_digest = None;
    let mut comparison_set: Option<Vec<Vec<u8>>> = None;
    let mut red = None;
    let mut green = None;
    for receipt in input.receipts {
        if receipt.plan != input.plan_slug
            || receipt.task != input.task_id
            || receipt.generation != generation
            || receipt.contract_digest != digest
        {
            return Err(invalid("stale or cross-plan receipt"));
        }
        let expected = match (receipt.phase.as_str(), receipt.expectation.as_str()) {
            ("test", "red") => {
                red = Some(receipt);
                "red"
            }
            ("green-rerun", "green") => {
                green = Some(receipt);
                "green"
            }
            ("test", "pass") => "pass",
            _ => return Err(invalid("invalid receipt phase/expectation")),
        };
        // The runner computes `verdict` honestly from its own records (`pass`
        // only when every record observed `pass`). A correctly red receipt's
        // records are all expected to observe `fail`, so its own verdict is
        // legitimately `fail`, not `pass` — only green/pass receipts, where
        // every record is expected to observe `pass`, should require it.
        let expected_verdict = if expected == "red" { "fail" } else { "pass" };
        if receipt.verdict != expected_verdict {
            return Err(invalid(
                "probe receipt verdict does not match its own expectation",
            ));
        }
        if receipt
            .records
            .iter()
            .any(|r| r.observed == crate::task_spec::probe_evidence::ObservedResult::NotRun)
        {
            return Err(invalid("NotRun is not evidence"));
        }
        for record in &receipt.records {
            let envelope = input
                .output_envelopes
                .get(&record.output_ref)
                .ok_or_else(|| invalid("missing output envelope"))?;
            verify_output_envelope(envelope, &record.output_digest)
                .map_err(|e| invalid(&e.to_string()))?;
            if expected == "red"
                && record.observed != crate::task_spec::probe_evidence::ObservedResult::Fail
            {
                return Err(invalid("red receipt contains non-failing probe"));
            }
            if expected != "red"
                && record.observed != crate::task_spec::probe_evidence::ObservedResult::Pass
            {
                return Err(invalid("green/pass receipt contains failing probe"));
            }
        }
        // Compare the env-excluded projection, not `probe_set_digest`. The
        // digest folds in each probe's `env_digest`, and `test-first-v1` has the
        // red receipt authored by a dispatched TESTER while the green rerun is
        // authored by the in-process ORCHESTRATOR — two process trees that never
        // share an ambient environment, so digest equality is unsatisfiable by
        // construction. Each receipt still self-verifies its own digest at parse
        // time under the unchanged formula.
        let mut projection = receipt
            .records
            .iter()
            .map(comparison_identity)
            .collect::<Result<Vec<_>, _>>()?;
        projection.sort();
        if let Some(previous) = &comparison_set {
            if previous != &projection {
                return Err(invalid("receipt probe identities differ"));
            }
        } else {
            comparison_set = Some(projection);
        }
        if set_digest.is_none() {
            set_digest = Some(receipt.probe_set_digest.clone());
        }
    }
    if let (Some(red), Some(green)) = (red, green) {
        compare_receipt_identities(red, green)?;
    }
    if let Some(contract) = input.contract {
        let expected = contract
            .expected_failures
            .iter()
            .map(|line| parse_expected_failure(line))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|failure| (failure.reference.clone(), failure))
            .collect::<BTreeMap<_, _>>();
        if let Some(red) = red {
            for record in &red.records {
                let failure = expected
                    .get(record.expected_failure_ref.as_deref().unwrap_or(""))
                    .ok_or_else(|| invalid("red record has no locked expected failure"))?;
                let envelope = input
                    .output_envelopes
                    .get(&record.output_ref)
                    .ok_or_else(|| invalid("missing red output envelope"))?;
                validate_expected_failure(record, failure, envelope)?;
            }
        }
    }
    Ok(set_digest)
}

fn parse_dispute(line: &str) -> Result<DisputeRecord, TestFirstEvidenceError> {
    let fields = line.split(';').collect::<Vec<_>>();
    if fields.len() != 10 {
        return Err(invalid("malformed dispute payload"));
    }
    let prefixes = [
        "dispute_id=",
        "generation=",
        "contract_digest=",
        "lock_refs_b64=",
        "evidence_refs_b64=",
        "basis=",
        "classification=",
        "status=",
        "retry_before=",
        "retry_after=",
    ];
    let values = prefixes
        .iter()
        .zip(fields.iter())
        .map(|(prefix, field)| {
            field
                .strip_prefix(prefix)
                .ok_or_else(|| invalid("dispute fields are not canonical"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let generation = values[1]
        .parse()
        .map_err(|_| invalid("invalid dispute generation"))?;
    let retry_before = values[8]
        .parse()
        .map_err(|_| invalid("invalid retry_before"))?;
    let retry_after = values[9]
        .parse()
        .map_err(|_| invalid("invalid retry_after"))?;
    let classification = values[6].to_string();
    let basis = values[5].to_string();
    let status = values[7].to_string();
    if !matches!(
        classification.as_str(),
        "probe-defect" | "implementation-defect" | "contract-ambiguous"
    ) || !matches!(status.as_str(), "open" | "resolved")
    {
        return Err(invalid("invalid dispute domain"));
    }
    let valid_mapping = (classification == "contract-ambiguous" && basis.starts_with("contract-"))
        || (classification == "probe-defect" && basis.starts_with("probe-"))
        || (classification == "implementation-defect"
            && matches!(basis.as_str(), "behavior-missing" | "behavior-wrong"));
    if !valid_mapping {
        return Err(invalid("invalid dispute basis/classification mapping"));
    }
    if values[0].len() < 8 || !values[0].starts_with("T-") || !values[0].contains("-D") {
        return Err(invalid("invalid dispute ID"));
    }
    if values[2].len() != 64
        || !values[2]
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(invalid("invalid dispute contract digest"));
    }
    for value in [&values[3], &values[4]] {
        decode_base64url_unpadded(value)
            .map_err(|_| invalid("invalid dispute reference encoding"))?;
    }
    Ok(DisputeRecord {
        dispute_id: values[0].to_string(),
        generation,
        contract_digest: values[2].to_string(),
        basis,
        classification,
        status,
        retry_before,
        retry_after,
    })
}

fn split_output(envelope: &[u8]) -> Result<(Vec<u8>, Vec<u8>), TestFirstEvidenceError> {
    if envelope.len() < 16 {
        return Err(invalid("output envelope is too short"));
    }
    let stdout_len = u64::from_be_bytes(envelope[..8].try_into().unwrap()) as usize;
    let stderr_start = 8usize
        .checked_add(stdout_len)
        .ok_or_else(|| invalid("output envelope overflow"))?;
    let stderr_len_end = stderr_start
        .checked_add(8)
        .ok_or_else(|| invalid("output envelope overflow"))?;
    if stderr_len_end > envelope.len() {
        return Err(invalid("output envelope is truncated"));
    }
    let stderr_len =
        u64::from_be_bytes(envelope[stderr_start..stderr_len_end].try_into().unwrap()) as usize;
    if stderr_len_end.checked_add(stderr_len) != Some(envelope.len()) {
        return Err(invalid("output envelope length mismatch"));
    }
    Ok((
        envelope[8..stderr_start].to_vec(),
        envelope[stderr_len_end..].to_vec(),
    ))
}

/// Env-excluded identity used for every cross-receipt comparison. Compares id,
/// selector, argv, expected-failure ref, and timeout — the fields the contract
/// actually requires to agree between the red and green receipts.
fn comparison_identity(record: &ProbeRecord) -> Result<Vec<u8>, TestFirstEvidenceError> {
    record
        .compute_comparison_bytes()
        .map_err(|e| invalid(&e.to_string()))
}
fn parse_expected_termination(value: &str) -> Result<ExpectedTermination, TestFirstEvidenceError> {
    if value == "nonzero" {
        return Ok(ExpectedTermination::Nonzero);
    }
    if let Some(v) = value.strip_prefix("exit:") {
        return Ok(ExpectedTermination::Exit(
            v.parse().map_err(|_| invalid("invalid expected exit"))?,
        ));
    }
    if let Some(v) = value.strip_prefix("signal:") {
        return Ok(ExpectedTermination::Signal(
            v.parse().map_err(|_| invalid("invalid expected signal"))?,
        ));
    }
    Err(invalid("invalid expected termination"))
}
fn invalid(message: &str) -> TestFirstEvidenceError {
    TestFirstEvidenceError::Invalid(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_expected_failure_tolerates_a_wrapping_backtick_pair() {
        let unwrapped =
            "EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64=YQ — matcher text";
        let wrapped =
            "`EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64=YQ` — matcher text";
        let from_unwrapped = parse_expected_failure(unwrapped).expect("unwrapped header parses");
        let from_wrapped = parse_expected_failure(wrapped).expect("wrapped header parses");
        assert_eq!(from_unwrapped, from_wrapped);
    }

    #[test]
    fn parse_expected_failure_unwrapped_header_unchanged() {
        let line = "EF-02;class=status;term=exit:1;stream=stderr;matcher_b64=Yg — status matcher";
        let failure = parse_expected_failure(line).expect("unwrapped header still parses");
        assert_eq!(failure.reference, "EF-02");
    }

    #[test]
    fn parse_expected_failure_rejects_a_stray_single_backtick() {
        // Only strip when BOTH ends carry exactly one backtick -- a lone leading
        // backtick with no matching trailing one must still fail closed, not be
        // silently stripped.
        let line =
            "`EF-03;class=assertion;term=nonzero;stream=stdout;matcher_b64=Yw== — stray backtick";
        assert!(parse_expected_failure(line).is_err());
    }

    fn test_task_id(n: u32) -> String {
        format!("T-{n:02}")
    }

    fn test_tp_id(n: u32) -> String {
        format!("TP-{n:02}")
    }

    #[test]
    fn parse_expected_failure_wrapped_header_round_trip_is_byte_identical() {
        use crate::task_spec::{assemble_task_spec, parse_task_contract, TaskSpecInput};

        let task = test_task_id(1);
        let tp = test_tp_id(1);
        let raw_ef =
            "`EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64=YQ` — matcher text";
        let prompt = format!(
            "# Plan Prompt: Test\n\nPipeline Contract: test-first-v1\n\n## Tasks\n\n- [ ] {task} — Sample task.\n  - Test-first: required\n  - Seam: `seam`\n  - Expected failures:\n    - {raw_ef}\n  - Production Paths:\n    - `crates/pipeline/src/test_first_evidence.rs`\n  - Test Paths:\n    - `crates/pipeline/tests/test_first_evidence.rs`\n  - Scaffold: not-required\n\n## Test Plan\n\n| ID | Type | Description | Tasks |\n| {tp} | unit | desc | {task} |\n"
        );
        let contract = parse_task_contract(&prompt, "test-plan", &task).expect("contract parses");
        assert_eq!(contract.expected_failures[0], raw_ef);
        let initial_ef = parse_expected_failure(&contract.expected_failures[0])
            .expect("parsed expected failure");

        let input = TaskSpecInput {
            prompt_path: ".dev/plans/test-plan.prompt.md",
            prompt_body: &prompt,
            task_scope: &task,
            phase: "implement",
            git_head: "0123456789abcdef0123456789abcdef01234567",
            git_branch: "main",
            generated: "2026-08-30T00:00:00Z",
            convention_hints: None,
            receipt_path: None,
            agent_contract_body: None,
            fix_mode: false,
        };
        let assembled = assemble_task_spec(&input).expect("assemble task spec");
        let expected_line = format!("  - {raw_ef}\n");
        assert!(assembled.markdown.contains(&expected_line));

        let written_ef_line = assembled
            .markdown
            .lines()
            .find(|l| l.contains("matcher_b64="))
            .expect("written line exists")
            .trim_start_matches(['-', ' ']);
        assert_eq!(written_ef_line, raw_ef);

        let reparsed_ef =
            parse_expected_failure(written_ef_line).expect("reparsed expected failure");
        assert_eq!(initial_ef, reparsed_ef);
    }

    #[test]
    fn parse_expected_failure_wrapped_header_in_multiline_continuation_block() {
        use crate::task_spec::parse_task_contract;

        let task = test_task_id(2);
        let tp = test_tp_id(1);
        let prompt = format!(
            "# Plan Prompt: Test\n\nPipeline Contract: test-first-v1\n\n## Tasks\n\n- [ ] {task} — Multi-line EF task.\n  - Test-first: required\n  - Seam: `seam`\n  - Expected failures:\n    - `EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64=YQ` — first failure\n    - `EF-02;class=status;term=exit:1;stream=stderr;matcher_b64=Yg` — second failure\n  - Production Paths:\n    - `crates/pipeline/src/test_first_evidence.rs`\n  - Test Paths:\n    - `crates/pipeline/tests/test_first_evidence.rs`\n  - Scaffold: not-required\n\n## Test Plan\n\n| ID | Type | Description | Tasks |\n| {tp} | unit | desc | {task} |\n"
        );
        let contract = parse_task_contract(&prompt, "test-plan", &task).expect("contract parses");
        assert_eq!(contract.expected_failures.len(), 2);
        let ef1 = parse_expected_failure(&contract.expected_failures[0]).expect("ef1 parses");
        let ef2 = parse_expected_failure(&contract.expected_failures[1]).expect("ef2 parses");

        assert_eq!(ef1.reference, "EF-01");
        assert_eq!(ef1.class, FailureClass::Assertion);
        assert_eq!(ef1.termination, ExpectedTermination::Nonzero);
        assert_eq!(ef1.stream, ExpectedStream::Stdout);
        assert_eq!(ef1.matcher, "a");
        assert_eq!(ef1.prose, "first failure");

        assert_eq!(ef2.reference, "EF-02");
        assert_eq!(ef2.class, FailureClass::Status);
        assert_eq!(ef2.termination, ExpectedTermination::Exit(1));
        assert_eq!(ef2.stream, ExpectedStream::Stderr);
        assert_eq!(ef2.matcher, "b");
        assert_eq!(ef2.prose, "second failure");
    }
}
