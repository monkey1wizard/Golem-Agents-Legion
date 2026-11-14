use super::pipeline_preflight::{bind_orchestrator, OrchestratorRecord};
use fs2::FileExt;
use pipeline::coordinator::AttemptOwnership;
use pipeline::{
    report::{read_design_roster_version, EvidenceRecord, EvidenceStore},
    usage::{digest_bytes, StoreError},
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub(crate) struct CaptureError(pub(crate) String);
impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl From<StoreError> for CaptureError {
    fn from(e: StoreError) -> Self {
        Self(e.to_string())
    }
}
impl From<std::io::Error> for CaptureError {
    fn from(e: std::io::Error) -> Self {
        Self(e.to_string())
    }
}

pub(crate) struct BeginInput {
    pub phase: String,
    pub invocation_id: String,
    pub run_id: Option<String>,
    pub worktree: PathBuf,
    pub host: String,
    pub host_session: Option<String>,
    pub owner_pid: Option<u32>,
}
pub(crate) struct BeginOutcome {
    pub operation_id: String,
    pub binding: Value,
}

pub(crate) struct ReceiptObservation {
    common_dir: Option<PathBuf>,
    operation_id: Option<String>,
    expected: Value,
    expected_envelope: Value,
    actual: Value,
    actual_context: Value,
    parse_result: Value,
    receipt_sha256: Option<String>,
    receipt_text: Option<String>,
    verification: Value,
    freshness: Value,
}

fn receipt_field<'a>(text: &'a str, field: &str) -> Result<Option<&'a str>, String> {
    let prefix = format!("{field}: ");
    let mut values = text.lines().filter_map(|line| line.strip_prefix(&prefix));
    let value = values.next().map(str::trim);
    if values.next().is_some() {
        return Err(format!("duplicate receipt field {field}"));
    }
    Ok(value)
}

fn compare_receipt_value(expected: Value, observed: Value, reason: &str) -> Value {
    if expected.is_null() || observed.is_null() {
        json!({"status":"unknown", "expected":expected, "observed":observed, "reason":reason})
    } else {
        json!({"status":if expected == observed { "pass" } else { "fail" },
            "expected":expected, "observed":observed})
    }
}

fn receipt_freshness(text: &str, expected: &Value, actual: &Value) -> Value {
    let field = |name| receipt_field(text, name).ok().flatten().map(str::to_owned);
    let worktree_root = super::pipeline_execution::current_worktree_root()
        .ok()
        .and_then(|path| path.canonicalize().ok());
    let worktree = worktree_root
        .as_ref()
        .map(|path| super::pipeline_execution::binding_path_text(path));
    let observed_worktree = field("worktree_root").or_else(|| {
        let coordinator = actual["coordinator_path"].as_str()?;
        let state: pipeline::coordinator::CoordinatorState =
            serde_json::from_slice(&fs::read(coordinator).ok()?).ok()?;
        Some(state.execution_binding?.worktree_root)
    });
    let prompt_hash = field("prompt_sha256");
    let prompt_path = prompt_hash.as_ref().and_then(|_| field("prompt_path"));
    let current_prompt_hash = prompt_path
        .as_ref()
        .and_then(|path| Path::new(path).canonicalize().ok())
        .filter(|path| {
            worktree_root
                .as_ref()
                .is_some_and(|root| path.starts_with(root))
        })
        .and_then(|path| fs::read(path).ok())
        .map(|bytes| digest_bytes(&bytes));
    let prompt_comparison = if actual.is_null() && prompt_hash.is_some() {
        json!({"status":"unknown", "expected":current_prompt_hash, "observed":prompt_hash,
            "reason":"receipt-envelope-unavailable"})
    } else {
        compare_receipt_value(
            json!(current_prompt_hash),
            json!(prompt_hash),
            if prompt_hash.is_none() {
                "receipt-has-no-prompt-hash"
            } else {
                "current-prompt-unavailable"
            },
        )
    };
    let expires_at = field("expires_at");
    let checked_at = now();
    let expiration = match expires_at
        .as_deref()
        .and_then(|value| value.parse::<u64>().ok())
    {
        Some(deadline) if !actual.is_null() => {
            json!({"status":if checked_at <= deadline { "pass" } else { "fail" },
            "expected":{"checked_at_unix_seconds":checked_at}, "observed":{"expires_at_unix_seconds":deadline}})
        }
        Some(deadline) => {
            json!({"status":"unknown", "expected":{"checked_at_unix_seconds":checked_at},
            "observed":{"expires_at_unix_seconds":deadline}, "reason":"receipt-envelope-unavailable"})
        }
        None => json!({"status":"unknown", "expected":null, "observed":expires_at,
            "reason":if expires_at.is_some() { "unparseable-expiration" } else { "receipt-has-no-expiration" }}),
    };
    let verdict = field("overall");
    let mut conditions = json!({
        "worktree_root":compare_receipt_value(json!(worktree), json!(observed_worktree), "receipt-worktree-unavailable"),
        "executable_sha256":compare_receipt_value(expected["executable_sha256"].clone(), actual["executable_sha256"].clone(), "receipt-envelope-unavailable"),
        "execution_binding_sha256":compare_receipt_value(expected["execution_binding_sha256"].clone(), actual["execution_binding_sha256"].clone(), "receipt-envelope-unavailable"),
        "prompt_sha256":prompt_comparison,
        "expiration":expiration,
        "receipt_verdict":compare_receipt_value(json!("pass"), json!(verdict), "receipt-has-no-verdict")
    });
    for (field, condition) in [
        ("worktree_root", "worktree_root"),
        ("prompt_sha256", "prompt_sha256"),
        ("prompt_path", "prompt_sha256"),
        ("expires_at", "expiration"),
        ("overall", "receipt_verdict"),
    ] {
        if let Err(error) = receipt_field(text, field) {
            conditions[condition]["status"] = json!("unknown");
            conditions[condition]["reason"] = json!("duplicate-receipt-field");
            conditions[condition]["error"] = json!(error);
            conditions[condition]["observed_fields"][field] = json!(text
                .lines()
                .filter_map(|line| line.strip_prefix(&format!("{field}: ")))
                .collect::<Vec<_>>());
        }
    }
    conditions
}

fn envelope_value(envelope: &super::finalize_check::ReceiptEnvelope) -> Value {
    json!({"binding_scope":envelope.binding_scope,
        "execution_binding_sha256":envelope.execution_binding_sha256,
        "executable_path":envelope.executable_path,
        "executable_sha256":envelope.executable_sha256,
        "plan_scope":envelope.plan_scope,
        "coordinator_path":envelope.coordinator_path})
}

impl ReceiptObservation {
    pub(crate) fn start(consumer: &str, path: &Path, expected: Value) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let worktree = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let invocation_id = format!(
            "{}-{}-{}",
            std::process::id(),
            now(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let common_dir = super::usage::git_common_dir_in(&worktree);
        let started = common_dir.as_ref().map_err(Clone::clone).and_then(|dir| {
            begin(
                dir,
                BeginInput {
                    phase: "receipt-verification".into(),
                    invocation_id,
                    run_id: None,
                    worktree: worktree.clone(),
                    host: "gal".into(),
                    host_session: None,
                    owner_pid: Some(std::process::id()),
                },
            )
            .map_err(|e| e.to_string())
        });
        let operation_id = match started {
            Ok(outcome) => {
                let entry =
                    entry_id(&outcome.operation_id).expect("begin returned invalid operation id");
                let store =
                    EvidenceStore::open(common_dir.as_ref().unwrap()).map_err(|e| e.to_string());
                let result = store.and_then(|store| {
                    store
                        .publish(&event(
                            &entry,
                            &outcome.operation_id,
                            &format!("{}-expected", outcome.operation_id),
                            "receipt-verification",
                            "verification-expected",
                            json!({"producer":"receipt-verification", "consumer": consumer,
                        "validation_method":"receipt-envelope-and-consumer-checks",
                        "receipt_path": path, "expected": expected}),
                        ))
                        .map_err(|e| e.to_string())
                });
                match result {
                    Ok(_) => Some(outcome.operation_id),
                    Err(error) => {
                        eprintln!("receipt observation failed: {error}");
                        None
                    }
                }
            }
            Err(error) => {
                eprintln!("receipt observation failed: {error}");
                None
            }
        };
        Self {
            common_dir: common_dir.ok(),
            operation_id,
            expected,
            expected_envelope: Value::Null,
            actual: Value::Null,
            actual_context: Value::Null,
            parse_result: json!({"status":"not-run"}),
            receipt_sha256: None,
            receipt_text: None,
            verification: json!({"status":"not-run"}),
            freshness: Value::Null,
        }
    }

    pub(crate) fn receipt(&mut self, text: &str) {
        self.receipt_sha256 = Some(digest_bytes(text.as_bytes()));
        self.receipt_text = Some(text.to_owned());
        let scope = self.expected["binding_scope"]
            .as_str()
            .or_else(|| receipt_field(text, "binding_scope").ok().flatten())
            .unwrap_or("standalone");
        if let Ok(expected) = super::finalize_check::receipt_envelope(Some(scope), None) {
            self.expected_envelope = envelope_value(&expected);
        }
        self.actual_context = json!(["gate", "mode", "gate_attempt_id", "repair_round"]
            .into_iter()
            .map(|field| (
                field.to_owned(),
                json!(text
                    .lines()
                    .filter_map(|line| {
                        line.strip_prefix(field)
                            .and_then(|value| value.strip_prefix(": "))
                    })
                    .collect::<Vec<_>>())
            ))
            .collect::<serde_json::Map<_, _>>());
        match super::finalize_check::parse_receipt_envelope(text) {
            Ok(actual) => {
                self.actual = envelope_value(&actual);
                self.parse_result = json!({"status":"pass"});
            }
            Err(error) => self.parse_result = json!({"status":"fail", "error":error}),
        }
        self.freshness = receipt_freshness(text, &self.expected_envelope, &self.actual);
    }

    pub(crate) fn verify(
        &mut self,
        text: &str,
        expected: &super::finalize_check::ReceiptEnvelope,
    ) -> Result<(), String> {
        self.expected_envelope = envelope_value(expected);
        self.freshness = receipt_freshness(text, &self.expected_envelope, &self.actual);
        let result = super::finalize_check::verify_receipt_envelope(text, expected);
        self.verification = match &result {
            Ok(()) => json!({"status":"pass"}),
            Err(error) => json!({"status":"fail", "error":error}),
        };
        result
    }

    pub(crate) fn finish(&self, result: &Result<(), String>) {
        let (Some(dir), Some(op)) = (&self.common_dir, &self.operation_id) else {
            return;
        };
        let eligibility = if result.is_ok() {
            json!(true)
        } else if self.receipt_text.is_some() {
            json!(false)
        } else {
            Value::Null
        };
        let outcome = (|| -> Result<(), CaptureError> {
            let entry = entry_id(op)?;
            let store = EvidenceStore::open(dir)?;
            let read_result = if self.receipt_text.is_some() {
                json!({"status":"pass"})
            } else {
                json!({"status":"fail","error":result.as_ref().err()})
            };
            let context = [
                ("gate", "gate"),
                ("mode", "mode"),
                ("attempt", "gate_attempt_id"),
                ("round", "repair_round"),
            ].into_iter().map(|(expected, actual)| {
                (actual.to_owned(), json!({"expected":self.expected[expected],"observed":self.actual_context[actual]}))
            }).collect::<serde_json::Map<_, _>>();
            let content = json!({"expected":self.expected,"expected_envelope":self.expected_envelope,"actual":self.actual,"actual_context":self.actual_context,
                "receipt_sha256":self.receipt_sha256,"receipt_text":self.receipt_text,"read_result":read_result,"parse_result":self.parse_result,
                "freshness":{"envelope":self.verification,"repair_context":context,"conditions":self.freshness},
                "envelope_verification":self.verification,"run_association":"unknown","consumption_eligible":eligibility,
                "decision":if result.is_ok() { "accept" } else { "reject" },
                "error":result.as_ref().err()});
            store.publish(&event(
                &entry,
                op,
                &format!("{op}-verification"),
                "receipt-verification",
                "verification",
                content,
            ))?;
            end(dir, op, "completed", None, None)?;
            Ok(())
        })();
        if let Err(error) = outcome {
            eprintln!("receipt observation failed: {error}");
        }
    }
}

pub(crate) fn write_gate_receipt<F, E>(
    gate: &str,
    mode: &str,
    path: &Path,
    text: &str,
    write: F,
) -> Result<(), E>
where
    F: FnOnce() -> Result<(), E>,
    E: std::fmt::Display,
{
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let worktree = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let capture = (|| -> Result<(PathBuf, String), CaptureError> {
        let common = super::usage::git_common_dir_in(&worktree).map_err(CaptureError)?;
        let invocation_id = format!(
            "{}-{}-{}",
            std::process::id(),
            now(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let operation = begin(
            &common,
            BeginInput {
                phase: "gate".into(),
                invocation_id,
                run_id: None,
                worktree,
                host: "gal".into(),
                host_session: None,
                owner_pid: Some(std::process::id()),
            },
        )?
        .operation_id;
        let entry = entry_id(&operation)?;
        let rows: Vec<Value> = text.lines().skip_while(|line| *line != "| check | state | command | summary |").skip(2).take_while(|line| line.starts_with("| ")).map(|line| {
            let fields: Vec<_> = line.trim_matches('|').split('|').map(str::trim).collect();
            json!({"check": fields.first(), "state": fields.get(1), "command": fields.get(2), "summary": fields.get(3)})
        }).collect();
        let content = json!({"gate": gate, "mode": mode, "command": std::env::args().collect::<Vec<_>>(), "receipt_path": path, "receipt_text": text, "receipt_sha256": digest_bytes(text.as_bytes()), "results": rows});
        EvidenceStore::open(&common)?.publish(&event(
            &entry,
            &operation,
            &format!("{operation}-gate"),
            "gate",
            "gate-result",
            content,
        ))?;
        Ok((common, operation))
    })();
    if let Err(error) = &capture {
        eprintln!("gate evidence capture failed: {error}");
    }
    let result = write();
    if let Ok((common, operation)) = capture {
        let recorded = (|| -> Result<(), CaptureError> {
            let entry = entry_id(&operation)?;
            let content = json!({"status": if result.is_ok() { "pass" } else { "fail" }, "error": result.as_ref().err().map(ToString::to_string)});
            EvidenceStore::open(&common)?.publish(&event(
                &entry,
                &operation,
                &format!("{operation}-write"),
                "gate",
                "receipt-write",
                content,
            ))?;
            end(&common, &operation, "completed", None, None)
        })();
        if let Err(error) = recorded {
            eprintln!("gate evidence capture failed: {error}");
        }
    }
    result
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
fn event(run: &str, op: &str, id: &str, phase: &str, kind: &str, content: Value) -> EvidenceRecord {
    EvidenceRecord {
        schema_version: 1,
        event_id: id.into(),
        operation_id: op.into(),
        run_id: run.into(),
        plan_generation: None,
        task_id: None,
        attempt_id: None,
        parent_attempt_id: None,
        phase: phase.into(),
        source_kind: kind.into(),
        source_version: "1".into(),
        recorded_at_unix_ms: now(),
        content_hash: digest_bytes(&serde_json::to_vec(&content).unwrap_or_default()),
        content,
    }
}
fn entries(store: &EvidenceStore, run: &str) -> Result<Vec<EvidenceRecord>, CaptureError> {
    let mut result = Vec::new();
    let mut after = None;
    loop {
        let page = store.list_page(run, after.as_deref(), 256)?;
        for item in page.entries {
            result.push(store.read(run, &item.record_id)?);
        }
        after = page.next_after;
        if after.is_none() {
            break;
        }
    }
    Ok(result)
}

fn correction_json(bytes: &[u8]) -> Result<Value, CaptureError> {
    let value = serde_json::from_slice(bytes).map_err(|e| CaptureError(e.to_string()))?;
    let mut objects: Vec<BTreeSet<String>> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => objects.push(BTreeSet::new()),
            b'}' => {
                objects.pop();
            }
            b'"' => {
                let start = index;
                index += 1;
                while index < bytes.len() && bytes[index] != b'"' {
                    if bytes[index] == b'\\' {
                        index += 1;
                    }
                    index += 1;
                }
                let mut next = index + 1;
                while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                    next += 1;
                }
                if bytes.get(next) == Some(&b':') {
                    let key: String = serde_json::from_slice(&bytes[start..=index])
                        .map_err(|e| CaptureError(e.to_string()))?;
                    if objects
                        .last_mut()
                        .is_some_and(|keys| !keys.insert(key.clone()))
                    {
                        return Err(CaptureError(format!("duplicate key `{key}`")));
                    }
                }
            }
            _ => {}
        }
        index += 1;
    }
    Ok(value)
}

fn correction_fields(value: &Value, keys: &[&str]) -> Result<(), CaptureError> {
    let fields = value
        .as_object()
        .ok_or_else(|| CaptureError("expected an object".into()))?;
    if fields.len() != keys.len() || !keys.iter().all(|key| fields.contains_key(*key)) {
        return Err(CaptureError(format!(
            "expected only these required keys: {}",
            keys.join(", ")
        )));
    }
    Ok(())
}

fn correction_text<'a>(value: &'a Value, field: &str) -> Result<&'a str, CaptureError> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| CaptureError(format!("{field} must be a non-empty string")))
}

fn correction_store_id(value: &Value) -> Result<&str, CaptureError> {
    let id = correction_text(value, "identifier")?;
    let stem = id.split('.').next().unwrap_or(id).to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"));
    if id.len() > 128
        || !id.starts_with(|c: char| c.is_ascii_alphanumeric())
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        || id.contains("..")
        || device
    {
        return Err(CaptureError(format!("invalid store identifier `{id}`")));
    }
    Ok(id)
}

fn correction_digest(value: &Value) -> Result<&str, CaptureError> {
    let digest = correction_text(value, "sha256")?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(CaptureError(
            "sha256 must contain 64 lowercase hexadecimal digits".into(),
        ));
    }
    Ok(digest)
}

fn correction_refs(value: &Value) -> Result<&[Value], CaptureError> {
    let refs = value
        .as_array()
        .ok_or_else(|| CaptureError("evidence_refs must be an array".into()))?;
    for reference in refs {
        correction_fields(reference, &["evidence_id", "sha256"])?;
        correction_text(&reference["evidence_id"], "evidence_id")?;
        correction_digest(&reference["sha256"])?;
    }
    Ok(refs)
}

fn correction_effective_time(value: &Value) -> Result<Option<u64>, CaptureError> {
    if value.is_null() {
        return Ok(None);
    }
    if let Some(milliseconds) = value.as_u64() {
        return Ok(Some(milliseconds));
    }
    let invalid = || {
        CaptureError(
            "rule.effective_at must be Unix milliseconds, an RFC 3339 timestamp, or null".into(),
        )
    };
    let pattern = regex::Regex::new(r"^([0-9]{4})-([0-9]{2})-([0-9]{2})[Tt]([0-9]{2}):([0-9]{2}):([0-9]{2})(?:\.([0-9]{1,9}))?([Zz]|[+-][0-9]{2}:[0-9]{2})$")
        .map_err(|e| CaptureError(e.to_string()))?;
    let parts = pattern
        .captures(value.as_str().ok_or_else(invalid)?)
        .ok_or_else(invalid)?;
    let number = |index| {
        parts
            .get(index)
            .ok_or_else(invalid)?
            .as_str()
            .parse::<i64>()
            .map_err(|_| invalid())
    };
    let (year, month, day) = (number(1)?, number(2)?, number(3)?);
    let (hour, minute, second) = (number(4)?, number(5)?, number(6)?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return Err(invalid()),
    };
    if year < 1970 || !(1..=month_days).contains(&day) || hour > 23 || minute > 59 || second > 59 {
        return Err(invalid());
    }
    let zone = parts.get(8).ok_or_else(invalid)?.as_str();
    let offset = if zone.eq_ignore_ascii_case("z") {
        0
    } else {
        let hours = zone[1..3].parse::<i64>().map_err(|_| invalid())?;
        let minutes = zone[4..6].parse::<i64>().map_err(|_| invalid())?;
        if hours > 23 || minutes > 59 {
            return Err(invalid());
        }
        (hours * 3600 + minutes * 60) * if zone.starts_with('-') { -1 } else { 1 }
    };
    let year = year - i64::from(month <= 2);
    let era = year / 400;
    let era_year = year - era * 400;
    let march_month = month + if month > 2 { -3 } else { 9 };
    let days = era * 146097 + era_year * 365 + era_year / 4 - era_year / 100
        + (153 * march_month + 2) / 5
        + day
        - 1
        - 719468;
    let fraction = parts
        .get(7)
        .map(|part| format!("{:0<3}", part.as_str()))
        .map(|fraction| {
            fraction[..3]
                .parse::<i64>()
                .map(|milliseconds| {
                    milliseconds + i64::from(fraction[3..].bytes().any(|digit| digit != b'0'))
                })
                .map_err(|_| invalid())
        })
        .transpose()?
        .unwrap_or(0);
    let milliseconds =
        (days * 86400 + hour * 3600 + minute * 60 + second - offset) * 1000 + fraction;
    u64::try_from(milliseconds).map(Some).map_err(|_| invalid())
}

pub(crate) fn correction_input(bytes: &[u8]) -> Result<Value, CaptureError> {
    let input = correction_json(bytes)?;
    correction_fields(
        &input,
        &[
            "schema_version",
            "correction_id",
            "revision",
            "incident_ids",
            "mechanism",
            "rule",
            "verification",
            "register_sha256",
        ],
    )?;
    if input["schema_version"].as_u64() != Some(1)
        || !input["revision"].as_u64().is_some_and(|n| n > 0)
    {
        return Err(CaptureError(
            "schema_version must be 1 and revision must be positive".into(),
        ));
    }
    let id = correction_store_id(&input["correction_id"])?;
    correction_store_id(&json!(format!("correction-{id}-r{}", input["revision"])))?;
    let incidents = input["incident_ids"]
        .as_array()
        .ok_or_else(|| CaptureError("incident_ids must be an array".into()))?;
    let mut unique = BTreeSet::new();
    for incident in incidents {
        if !unique.insert(correction_store_id(incident)?) {
            return Err(CaptureError("incident_ids must be unique".into()));
        }
    }
    let mechanism = &input["mechanism"];
    if !mechanism.is_null() {
        correction_fields(
            mechanism,
            &["mechanism_id", "path", "level", "reason_for_level"],
        )?;
        correction_store_id(&mechanism["mechanism_id"])?;
        for field in ["path", "reason_for_level"] {
            correction_text(&mechanism[field], field)?;
        }
        if !matches!(
            mechanism["level"].as_str(),
            Some("architecture" | "type" | "lint-ci" | "behavior-test" | "judgment")
        ) {
            return Err(CaptureError("invalid mechanism.level".into()));
        }
    }
    let rule = &input["rule"];
    correction_fields(
        rule,
        &["rule_id", "version", "effective_at", "evidence_refs"],
    )?;
    correction_text(&rule["rule_id"], "rule_id")?;
    if !rule["version"].is_null() && !rule["version"].as_u64().is_some_and(|n| n > 0) {
        correction_text(&rule["version"], "rule.version")?;
    }
    correction_effective_time(&rule["effective_at"])?;
    correction_refs(&rule["evidence_refs"])?;
    let verification = &input["verification"];
    correction_fields(verification, &["status", "evidence_refs"])?;
    if !matches!(
        verification["status"].as_str(),
        Some("pass" | "fail" | "not-run" | "unknown")
    ) {
        return Err(CaptureError("invalid verification.status".into()));
    }
    let refs = correction_refs(&verification["evidence_refs"])?;
    if matches!(verification["status"].as_str(), Some("pass" | "fail")) && refs.is_empty() {
        return Err(CaptureError(
            "pass and fail verification require evidence".into(),
        ));
    }
    correction_digest(&input["register_sha256"])?;
    Ok(input)
}

fn correction_records(
    common: &Path,
    store: &EvidenceStore,
) -> Result<Vec<EvidenceRecord>, CaptureError> {
    let mut records = Vec::new();
    for directory in fs::read_dir(common.join("gal/evidence/v1"))? {
        let directory = directory?;
        if directory.file_type()?.is_dir() {
            let scope = directory.file_name().to_string_lossy().into_owned();
            records.extend(entries(store, &scope)?);
        }
    }
    Ok(records)
}

fn failed_operation(record: &EvidenceRecord) -> bool {
    matches!(
        record.source_kind.as_str(),
        "operation" | "operation-end" | "operation-terminal" | "operation-interrupted"
    ) && (matches!(
        record.content["status"].as_str(),
        Some("fail" | "failed" | "error" | "interrupted")
    ) || record.content["exit_code"]
        .as_i64()
        .is_some_and(|code| code != 0))
}

fn correction_incident(
    record: &EvidenceRecord,
    records: &[EvidenceRecord],
) -> Result<String, CaptureError> {
    let mut current = record;
    let mut visited = BTreeSet::new();
    let mut first_failure = record;
    loop {
        if let Some(id) = current.content["incident_id"].as_str() {
            return Ok(correction_store_id(&json!(id))?.to_owned());
        }
        if failed_operation(current) {
            first_failure = current;
        }
        let Some(parent) = current.parent_attempt_id.as_deref() else {
            break;
        };
        if !visited.insert(parent) {
            return Err(CaptureError("cyclic incident attempt chain".into()));
        }
        let parent_record = records
            .iter()
            .filter(|candidate| {
                candidate.operation_id == record.operation_id
                    && candidate.attempt_id.as_deref() == Some(parent)
            })
            .min_by_key(|candidate| {
                (
                    !failed_operation(candidate),
                    candidate.recorded_at_unix_ms,
                    &candidate.event_id,
                )
            });
        let Some(parent_record) = parent_record else {
            break;
        };
        current = parent_record;
    }
    Ok(first_failure.event_id.clone())
}

fn correction_associations(
    input: &Value,
    records: &[EvidenceRecord],
) -> Result<Value, CaptureError> {
    let mut identities = BTreeMap::new();
    for record in records.iter().filter(|record| failed_operation(record)) {
        identities.insert(
            record.event_id.clone(),
            correction_incident(record, records)?,
        );
    }
    let sources: BTreeSet<String> = input["incident_ids"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|id| identities.get(id).cloned().unwrap_or_else(|| id.to_owned()))
        .collect();
    let rule = &input["rule"];
    let effective_at = correction_effective_time(&rule["effective_at"])?;
    let known_rule = !rule["version"].is_null() && effective_at.is_some();
    let mut incident_times: BTreeMap<&str, u64> = BTreeMap::new();
    for record in records.iter().filter(|record| failed_operation(record)) {
        let incident = identities[&record.event_id].as_str();
        incident_times
            .entry(incident)
            .and_modify(|time| *time = (*time).min(record.recorded_at_unix_ms))
            .or_insert(record.recorded_at_unix_ms);
    }
    let mut associations: BTreeMap<String, &str> = BTreeMap::new();
    for record in records.iter().filter(|record| failed_operation(record)) {
        let incident = &identities[&record.event_id];
        if sources.contains(incident) {
            continue;
        }
        let matches_rule = record.content["rule_id"] == rule["rule_id"]
            || record.content["evidence_refs"]
                .as_array()
                .is_some_and(|refs| {
                    refs.iter()
                        .any(|reference| reference["rule_id"] == rule["rule_id"])
                });
        if !matches_rule {
            continue;
        }
        let status = if !known_rule {
            "unknown"
        } else if incident_times[incident.as_str()] >= effective_at.unwrap_or(u64::MAX) {
            "recurrence"
        } else {
            continue;
        };
        associations.insert(incident.clone(), status);
    }
    let count = associations
        .values()
        .filter(|status| **status == "recurrence")
        .count();
    Ok(
        json!({"source_incident_ids":sources,"source_association":if sources.is_empty() {"unknown"} else {"known"},
        "incidents":associations.into_iter().map(|(id,status)| json!({"incident_id":id,"status":status})).collect::<Vec<_>>(),
        "recurrence_count":count,"recurrence_denominator":"unavailable"}),
    )
}

pub(crate) fn criterion_result_input(bytes: &[u8]) -> Result<Value, CaptureError> {
    let input = correction_json(bytes)?;
    let mut required = vec![
        "schema_version",
        "run_id",
        "prompt_path",
        "task_id",
        "tp_id",
        "gate_id",
        "source_kind",
        "binding",
        "outcome",
        "evidence_refs",
        "source_operation_id",
        "source_evidence_sha256",
        "observed_head_sha",
        "goal_spec_sha256",
        "owner_acceptance_id",
        "owner_target_sha256",
    ];
    if input.get("target_reason").is_some() {
        required.push("target_reason");
        correction_text(&input["target_reason"], "target_reason")?;
    }
    correction_fields(&input, &required)?;
    if input["schema_version"] != 1 {
        return Err(CaptureError(
            "criterion result schema_version must be 1".into(),
        ));
    }
    for key in [
        "run_id",
        "task_id",
        "tp_id",
        "gate_id",
        "source_operation_id",
    ] {
        correction_store_id(&input[key])?;
    }
    correction_text(&input["prompt_path"], "prompt_path")?;
    let binding: pipeline::report::CriterionBinding =
        serde_json::from_value(input["binding"].clone())
            .map_err(|error| CaptureError(error.to_string()))?;
    correction_store_id(&json!(binding.criterion_id))?;
    for hash in [
        &binding.spec_sha256,
        &binding.membership_sha256,
        &binding.artifact_set_sha256,
        &binding.artifact_snapshot_sha256,
    ] {
        correction_digest(&json!(hash))?;
    }
    let source = correction_text(&input["source_kind"], "source_kind")?;
    let valid = match source {
        "orchestrator-check" | "goal-verification" => matches!(
            input["outcome"].as_str(),
            Some("pass" | "fail" | "not-run" | "unverified" | "unknown")
        ),
        "owner-acceptance" => matches!(
            input["outcome"].as_str(),
            Some("accepted" | "waived" | "pending")
        ),
        _ => false,
    };
    if !valid {
        return Err(CaptureError("invalid non-TEST source or outcome".into()));
    }
    correction_digest(&input["source_evidence_sha256"])?;
    let head = correction_text(&input["observed_head_sha"], "observed_head_sha")?;
    if !matches!(head.len(), 40 | 64) || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(CaptureError("invalid observed_head_sha".into()));
    }
    for key in ["goal_spec_sha256", "owner_target_sha256"] {
        if !input[key].is_null() {
            correction_digest(&input[key])?;
        }
    }
    if source == "owner-acceptance" {
        correction_text(&input["owner_acceptance_id"], "owner_acceptance_id")?;
        if !input["goal_spec_sha256"].is_null() {
            return Err(CaptureError(
                "owner result has an inapplicable goal digest".into(),
            ));
        }
    } else if !input["owner_acceptance_id"].is_null() || !input["owner_target_sha256"].is_null() {
        return Err(CaptureError("non-owner result has owner fields".into()));
    }
    if source == "orchestrator-check" && !input["goal_spec_sha256"].is_null() {
        return Err(CaptureError(
            "orchestrator check has an inapplicable goal digest".into(),
        ));
    }
    if ((source == "goal-verification" && input["goal_spec_sha256"].is_null())
        || (source == "owner-acceptance" && input["owner_target_sha256"].is_null()))
        && input.get("target_reason").is_none()
    {
        return Err(CaptureError(
            "unobservable target requires target_reason".into(),
        ));
    }
    correction_refs(&input["evidence_refs"])?;
    Ok(input)
}

fn criterion_section(text: &str, heading: &str) -> Option<String> {
    let mut lines = text.lines().skip_while(|line| line.trim() != heading);
    lines.next()?;
    Some(
        lines
            .take_while(|line| !line.starts_with("## "))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned(),
    )
}

fn criterion_owner_evidence(text: &str, owner_id: &str) -> Option<BTreeMap<String, String>> {
    let section = text.split_once("#### Owner Acceptance Evidence")?.1;
    let mut table = section
        .lines()
        .take_while(|line| !line.starts_with('#'))
        .filter(|line| line.trim().starts_with('|'));
    let headers = super::finalize_check::split_md_row(table.next()?);
    let id_column = headers.iter().position(|cell| cell == "ID")?;
    let mut rows = table
        .map(super::finalize_check::split_md_row)
        .filter(|cells| cells.get(id_column).is_some_and(|id| id == owner_id));
    let row = rows.next()?;
    if rows.next().is_some() || headers.len() != row.len() {
        return None;
    }
    Some(headers.into_iter().zip(row).collect())
}

pub(crate) fn criterion_target_digest(
    repo: &Path,
    prompt: &Path,
    source: &str,
    criterion: &str,
    owner_id: Option<&str>,
) -> Option<String> {
    let text = fs::read_to_string(prompt).ok()?;
    let contract = super::test_plan_table::parse_criterion_contract(&text).ok()??;
    let payload = contract.criteria.get(criterion)?;
    match source {
        "goal-verification" => {
            let source_path = prompt.with_file_name(
                prompt
                    .file_name()?
                    .to_str()?
                    .strip_suffix(".prompt.md")?
                    .to_owned()
                    + ".md",
            );
            let source_text = fs::read_to_string(source_path).ok()?;
            let goal = criterion_section(&source_text, "## Goal")?;
            let success = criterion_section(&source_text, "## Success Criteria")?;
            let goals = contract
                .criteria
                .iter()
                .filter(|(_, payload)| {
                    payload.declaration.source_kind
                        == pipeline::report::CriterionSource::GoalVerification
                })
                .map(|(id, payload)| Some((id.clone(), payload.spec_sha256().ok()?)))
                .collect::<Option<BTreeMap<_, _>>>()?;
            Some(digest_bytes(
                &serde_json::to_vec(
                    &json!({"goal":goal,"success_criteria":success,"criteria":goals}),
                )
                .ok()?,
            ))
        }
        "owner-acceptance" => {
            let owner_id = owner_id?;
            if payload.declaration.target.as_deref() != Some(owner_id) {
                return None;
            }
            let owner_section = criterion_section(&text, "## Owner Acceptance")?;
            let mut table = owner_section
                .lines()
                .filter(|line| line.trim().starts_with('|'));
            let headers = super::finalize_check::split_md_row(table.next()?);
            let id_column = headers.iter().position(|cell| cell == "ID")?;
            let rows = table
                .map(super::finalize_check::split_md_row)
                .filter(|cells| cells.get(id_column).is_some_and(|id| id == owner_id))
                .collect::<Vec<_>>();
            if rows.len() != 1 {
                return None;
            }
            let fields = ["What to check", "Expected result", "Pass/fail rule"]
                .into_iter()
                .map(|name| {
                    Some((
                        name,
                        rows[0]
                            .get(headers.iter().position(|cell| cell == name)?)?
                            .clone(),
                    ))
                })
                .collect::<Option<BTreeMap<_, _>>>()?;
            let evidence = criterion_owner_evidence(&text, owner_id)?;
            let target = evidence.get("Target binding")?;
            if target.trim().is_empty() {
                return None;
            }
            let files = contract.files(&text);
            let paths = payload
                .mappings
                .iter()
                .flat_map(|mapping| files[&mapping.task_id].iter().cloned())
                .collect();
            let snapshot = pipeline::report::artifact_snapshot(repo, &paths).ok()?;
            Some(digest_bytes(&serde_json::to_vec(&json!({"fields":fields,"target":target,"artifact_snapshot_sha256":snapshot.digest()})).ok()?))
        }
        _ => None,
    }
}

pub(crate) fn criterion_operation_sequence(common: &Path, record: &EvidenceRecord) -> Option<u64> {
    let store = EvidenceStore::open(common).ok()?;
    let operation = record.content["source_operation_id"]
        .as_str()
        .unwrap_or(&record.operation_id);
    let begin = store
        .read(&entry_id(operation).ok()?, operation)
        .ok()
        .or_else(|| {
            entries(&store, &record.run_id)
                .ok()?
                .into_iter()
                .find(|candidate| {
                    candidate.source_kind == "operation-begin"
                        && candidate.operation_id == operation
                })
        })?;
    (begin.source_kind == "operation-begin").then_some(())?;
    begin.content["sequence"]
        .as_u64()
        .filter(|sequence| *sequence > 0)
}

pub(crate) fn criterion_result_validity(
    common: &Path,
    repo: &Path,
    prompt: &Path,
    record: &EvidenceRecord,
    row: &Value,
) -> pipeline::report::CriterionStatus {
    use pipeline::report::CriterionStatus;
    let source = row["source_kind"].as_str().unwrap_or(&record.source_kind);
    if source == "test" {
        return CriterionStatus::Pass;
    }
    let input = &record.content;
    if input["source_kind"].as_str() != Some(source) || record.source_kind != source {
        return CriterionStatus::Unverified;
    }
    if input["status"] == "superseded" {
        return CriterionStatus::Superseded;
    }
    let Some(binding) =
        serde_json::from_value::<pipeline::report::CriterionBinding>(input["binding"].clone()).ok()
    else {
        return CriterionStatus::Unverified;
    };
    let Some(text) = fs::read_to_string(prompt).ok() else {
        return CriterionStatus::Unverified;
    };
    let Some(contract) = super::test_plan_table::parse_criterion_contract(&text)
        .ok()
        .flatten()
    else {
        return CriterionStatus::Unverified;
    };
    let Some(payload) = contract.criteria.get(&binding.criterion_id) else {
        return CriterionStatus::Superseded;
    };
    let files = contract.files(&text);
    let paths: BTreeSet<_> = payload
        .mappings
        .iter()
        .flat_map(|mapping| files[&mapping.task_id].iter().cloned())
        .collect();
    if payload.spec_sha256().ok().as_deref() != Some(&binding.spec_sha256)
        || serde_json::to_vec(&paths)
            .ok()
            .map(|bytes| digest_bytes(&bytes))
            .as_deref()
            != Some(&binding.artifact_set_sha256)
    {
        return CriterionStatus::Superseded;
    }
    let Some(sequence) = criterion_operation_sequence(common, record) else {
        return CriterionStatus::Unverified;
    };
    if input["operation_sequence"].as_u64() != Some(sequence) {
        return CriterionStatus::Unverified;
    }
    let Some(references) = input["evidence_refs"]
        .as_array()
        .filter(|refs| !refs.is_empty())
    else {
        return CriterionStatus::Unverified;
    };
    let Ok(store) = EvidenceStore::open(common) else {
        return CriterionStatus::Unverified;
    };
    let mut source_found = false;
    for reference in references {
        let Some(id) = reference["evidence_id"].as_str() else {
            return CriterionStatus::Unverified;
        };
        let Ok(evidence) = store.read(&record.run_id, id) else {
            return CriterionStatus::Unverified;
        };
        let Ok(hash) = pipeline::usage::store::record_digest(&evidence) else {
            return CriterionStatus::Unverified;
        };
        if reference["sha256"].as_str() != Some(&hash) {
            return CriterionStatus::Unverified;
        }
        source_found |= input["source_evidence_sha256"].as_str() == Some(&hash)
            && evidence.operation_id == record.operation_id;
    }
    if !source_found {
        return CriterionStatus::Unverified;
    }
    if source == "owner-acceptance" && row["outcome"] == "accepted" {
        let evidence = input["owner_acceptance_id"]
            .as_str()
            .and_then(|id| criterion_owner_evidence(&text, id));
        if evidence.as_ref().is_none_or(|row| {
            row.get("Outcome").map(String::as_str) != Some("accepted")
                || row
                    .get("Owner evidence")
                    .is_none_or(|text| text.trim().is_empty())
        }) {
            return CriterionStatus::Unverified;
        }
    }
    let field = match source {
        "goal-verification" => "goal_spec_sha256",
        "owner-acceptance" => "owner_target_sha256",
        "orchestrator-check" => return CriterionStatus::Pass,
        _ => return CriterionStatus::Unverified,
    };
    let Some(expected) = input[field].as_str() else {
        return CriterionStatus::Unverified;
    };
    match criterion_target_digest(
        repo,
        prompt,
        source,
        row["criterion_id"].as_str().unwrap_or_default(),
        input["owner_acceptance_id"].as_str(),
    ) {
        Some(current) if current == expected => CriterionStatus::Pass,
        Some(_) => CriterionStatus::Superseded,
        None => CriterionStatus::Unverified,
    }
}

pub(crate) fn register_criterion_result(
    common: &Path,
    repo: &Path,
    input: Value,
) -> Result<Value, CaptureError> {
    let store = EvidenceStore::open(common)?;
    let _lock = lock(common)?;
    refuse_closed_worktree(common, repo, None)?;
    let prompt = repo
        .join(input["prompt_path"].as_str().unwrap_or_default())
        .canonicalize()?;
    if !prompt.starts_with(repo.canonicalize()?) {
        return Err(CaptureError(
            "criterion prompt is outside the worktree".into(),
        ));
    }
    let run = correction_text(&input["run_id"], "run_id")?;
    let receipt = repo.join(
        super::dispatch::resolve_receipt_path(Some(&prompt), None, "preflight.receipt.md")
            .map_err(CaptureError)?,
    );
    if resolve_run(common, &receipt) != RunAssociation::Bound(run.to_owned()) {
        return Err(CaptureError("criterion run binding mismatch".into()));
    }
    let task = correction_text(&input["task_id"], "task_id")?;
    let membership = super::dispatch::load_criterion_membership(repo, &prompt, task)
        .map_err(CaptureError)?
        .ok_or_else(|| CaptureError("missing frozen criterion membership".into()))?;
    let binding: pipeline::report::CriterionBinding =
        serde_json::from_value(input["binding"].clone())
            .map_err(|error| CaptureError(error.to_string()))?;
    if !membership.bindings.contains(&binding) {
        return Err(CaptureError("criterion binding mismatch".into()));
    }
    let source = correction_text(&input["source_kind"], "source_kind")?;
    let text = fs::read_to_string(&prompt)?;
    let contract = super::test_plan_table::parse_criterion_contract(&text)
        .map_err(CaptureError)?
        .ok_or_else(|| CaptureError("missing criterion declarations".into()))?;
    let payload = contract
        .criteria
        .get(&binding.criterion_id)
        .ok_or_else(|| CaptureError("unknown criterion".into()))?;
    if serde_json::to_value(&payload.declaration.source_kind)
        .map_err(|error| CaptureError(error.to_string()))?
        != input["source_kind"]
        || payload
            .spec_sha256()
            .map_err(|error| CaptureError(error.to_string()))?
            != binding.spec_sha256
        || !payload.mappings.iter().any(|row| {
            row.task_id == task && input["tp_id"] == row.tp_id && input["gate_id"] == row.gate_id
        })
        || (source == "owner-acceptance"
            && input["owner_acceptance_id"].as_str() != payload.declaration.target.as_deref())
        || (source == "orchestrator-check"
            && payload
                .test_plans
                .iter()
                .any(|row| !matches!(row.test_type.as_str(), "grep" | "manual" | "documentation")))
    {
        return Err(CaptureError("criterion source or mapping mismatch".into()));
    }
    if !membership.mappings.iter().any(|row| {
        row.criterion_id == binding.criterion_id
            && row.task_id == task
            && input["tp_id"] == row.tp_id
            && input["gate_id"] == row.gate_id
            && row
                .source_kind
                .as_ref()
                .is_none_or(|kind| *kind == payload.declaration.source_kind)
    }) {
        return Err(CaptureError("frozen criterion mapping mismatch".into()));
    }
    let files = contract.files(&text);
    let paths: BTreeSet<_> = payload
        .mappings
        .iter()
        .flat_map(|row| files[&row.task_id].iter().cloned())
        .collect();
    let snapshot = pipeline::report::artifact_snapshot(repo, &paths)?;
    if digest_bytes(&serde_json::to_vec(&paths).map_err(|error| CaptureError(error.to_string()))?)
        != binding.artifact_set_sha256
        || snapshot.digest() != binding.artifact_snapshot_sha256
    {
        return Err(CaptureError("criterion artifact binding mismatch".into()));
    }
    let operation = correction_text(&input["source_operation_id"], "source_operation_id")?;
    let probe = event(run, operation, "probe", source, source, input.clone());
    let sequence = criterion_operation_sequence(common, &probe)
        .ok_or_else(|| CaptureError("source operation sequence unavailable".into()))?;
    let records = entries(&store, run)?;
    let result_sequence = records
        .iter()
        .filter(|record| record.operation_id == operation)
        .filter_map(|record| record.content["result_sequence"].as_u64())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| CaptureError("criterion sequence exhausted".into()))?;
    let mut raw = Vec::new();
    let mut source_found = false;
    for reference in correction_refs(&input["evidence_refs"])? {
        let id = correction_text(&reference["evidence_id"], "evidence_id")?;
        let evidence = store.read(run, id)?;
        let hash = pipeline::usage::store::record_digest(&evidence)?;
        if reference["sha256"] != hash {
            return Err(CaptureError("criterion evidence integrity-failure".into()));
        }
        source_found |=
            input["source_evidence_sha256"] == hash && evidence.operation_id == operation;
        raw.extend(serde_json::to_vec(&evidence).map_err(|error| CaptureError(error.to_string()))?);
    }
    if !source_found {
        return Err(CaptureError(
            "source evidence is not bound to the source operation".into(),
        ));
    }
    let binding_hash = digest_bytes(
        &serde_json::to_vec(&input["binding"]).map_err(|error| CaptureError(error.to_string()))?,
    );
    let mut content = input.clone();
    content["operation_sequence"] = json!(sequence);
    content["result_sequence"] = json!(result_sequence);
    let row = json!({"task_id":task,"tp_id":input["tp_id"],"gate_id":input["gate_id"],
        "criterion_id":binding.criterion_id,"source_kind":source,"outcome":input["outcome"],
        "binding_sha256":binding_hash,"evidence":input["evidence_refs"].as_array().into_iter().flatten()
            .filter_map(|reference| reference["evidence_id"].as_str()).collect::<Vec<_>>(),
        "artifact_validity":pipeline::report::ArtifactValidity::new(snapshot, &raw)});
    let id = format!("criterion-{operation}-{result_sequence}");
    let probe = event(run, operation, &id, source, source, content.clone());
    let validity = criterion_result_validity(common, repo, &prompt, &probe, &row);
    let status = if (source == "goal-verification" && input["goal_spec_sha256"].is_null())
        || (source == "owner-acceptance" && input["owner_target_sha256"].is_null())
    {
        "unknown"
    } else {
        match validity {
            pipeline::report::CriterionStatus::Pass => match input["outcome"].as_str() {
                Some("pass" | "accepted") => "pass",
                Some("fail") => "fail",
                _ => "unverified",
            },
            pipeline::report::CriterionStatus::Superseded => "superseded",
            _ => "unknown",
        }
    };
    content["status"] = json!(status);
    content["raw_bytes"] = json!(raw);
    content["criterion_results"] = json!([row]);
    let mut record = event(run, operation, &id, source, source, content);
    record.task_id = Some(task.into());
    let mut frozen = event(
        run,
        operation,
        &format!("membership-{operation}-{result_sequence}"),
        source,
        "criterion-membership",
        membership.value(),
    );
    frozen.task_id = Some(task.into());
    store.publish(&frozen)?;
    store.publish(&record)?;
    Ok(
        json!({"record_id":id,"operation_id":operation,"operation_sequence":sequence,"result_sequence":result_sequence,"status":status}),
    )
}

pub(crate) fn register_correction(
    common: &Path,
    worktree: &Path,
    input: Value,
) -> Result<Value, CaptureError> {
    let register = fs::read(worktree.join(".dev/mistake-register.md"))?;
    if digest_bytes(&register) != input["register_sha256"].as_str().unwrap_or_default() {
        return Err(CaptureError("register digest mismatch".into()));
    }
    let register_text = String::from_utf8(register).map_err(|e| CaptureError(e.to_string()))?;
    let id = format!(
        "correction-{}-r{}",
        input["correction_id"].as_str().unwrap_or_default(),
        input["revision"]
    );
    let operation = begin(
        common,
        BeginInput {
            phase: "correction".into(),
            invocation_id: id.clone(),
            run_id: None,
            worktree: worktree.to_path_buf(),
            host: "gal".into(),
            host_session: None,
            owner_pid: Some(std::process::id()),
        },
    )?
    .operation_id;
    let store = EvidenceStore::open(common)?;
    let result = (|| -> Result<Value, CaptureError> {
        let _lock = lock(common)?;
        let records = correction_records(common, &store)?;
        for reference in correction_refs(&input["rule"]["evidence_refs"])?
            .iter()
            .chain(correction_refs(&input["verification"]["evidence_refs"])?)
        {
            let evidence_id = correction_text(&reference["evidence_id"], "evidence_id")?;
            let expected = correction_digest(&reference["sha256"])?;
            let path = worktree.join(evidence_id);
            let digest = if path.is_file() {
                digest_bytes(&fs::read(path)?)
            } else {
                let matches: Vec<_> = records
                    .iter()
                    .filter(|record| record.event_id == evidence_id)
                    .collect();
                if matches.is_empty() {
                    return Err(CaptureError(format!(
                        "unreadable evidence reference `{evidence_id}`"
                    )));
                }
                let mut digests = BTreeSet::new();
                for record in matches {
                    digests.insert(pipeline::usage::store::record_digest(record)?);
                }
                if digests.len() != 1 {
                    return Err(CaptureError(format!(
                        "ambiguous evidence reference `{evidence_id}`"
                    )));
                }
                digests
                    .into_iter()
                    .next()
                    .ok_or_else(|| CaptureError("missing evidence digest".into()))?
            };
            if digest != expected {
                return Err(CaptureError(format!(
                    "evidence reference digest mismatch `{evidence_id}`"
                )));
            }
        }
        let associations = correction_associations(&input, &records)?;
        let mut content = input.clone();
        content["run_id"] = Value::Null;
        content["task_id"] = Value::Null;
        content["register_snapshot"] = json!({"path":".dev/mistake-register.md", "sha256":input["register_sha256"], "text":register_text});
        let existing: Vec<_> = records
            .iter()
            .filter(|record| record.event_id == id)
            .collect();
        if existing
            .iter()
            .any(|record| record.source_kind != "correction" || record.content != content)
        {
            return Err(CaptureError(format!("correction conflict for `{id}`")));
        }
        let scope = entry_id(&operation)?;
        let stored_operation = if let Some(record) = existing.first() {
            record.operation_id.clone()
        } else {
            store.publish(&event(
                &scope,
                &operation,
                &id,
                "correction",
                "correction",
                content,
            ))?;
            operation.clone()
        };
        Ok(
            json!({"correction_id":input["correction_id"],"revision":input["revision"],"record_id":id,
            "operation_id":stored_operation,"run_id":null,"task_id":null,"association":associations}),
        )
    })();
    end(common, &operation, "completed", None, None)?;
    result
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RunAssociation {
    Bound(String),
    None,
    Unknown,
}

pub(crate) struct PipelineObservation {
    common: PathBuf,
    pub run: RunAssociation,
    operation: String,
    receipt: Option<PathBuf>,
    coordinator: Option<PathBuf>,
    before: Value,
    worktree: PathBuf,
    changes_before: Value,
    request: Value,
    invariants: serde_json::Map<String, Value>,
    started: std::time::Instant,
}

fn coordinator_snapshot(path: &Path) -> Value {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(state) => json!({"status":"present", "state":state}),
            Err(error) => json!({"status":"malformed", "error":error.to_string()}),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({"status":"absent"}),
        Err(error) => json!({"status":"unavailable", "error":error.to_string()}),
    }
}

fn working_tree_changes(repo: &Path) -> Value {
    match std::process::Command::new("git")
        .current_dir(repo)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()
    {
        Ok(output) if output.status.success() => match String::from_utf8(output.stdout) {
            Ok(text) => json!({"status":"observed", "porcelain":text}),
            Err(error) => json!({"status":"unavailable", "error":error.to_string()}),
        },
        Ok(output) => {
            json!({"status":"unavailable", "exit_code":output.status.code(),"stderr":String::from_utf8_lossy(&output.stderr)})
        }
        Err(error) => json!({"status":"unavailable", "error":error.to_string()}),
    }
}

impl PipelineObservation {
    pub(crate) fn start(repo: &Path, prompt: &Path, request: Value) -> Option<Self> {
        Self::start_inner(repo, Some(prompt), request)
    }

    pub(crate) fn unscoped(repo: &Path, request: Value) -> Option<Self> {
        Self::start_inner(repo, None, request)
    }

    fn start_inner(repo: &Path, prompt: Option<&Path>, request: Value) -> Option<Self> {
        let result = (|| -> Result<Self, CaptureError> {
            let common = super::usage::git_common_dir_in(repo).map_err(CaptureError)?;
            let receipt = prompt
                .map(|prompt| {
                    super::dispatch::resolve_receipt_path(
                        Some(prompt),
                        None,
                        "preflight.receipt.md",
                    )
                    .map(|path| repo.join(path))
                    .map_err(CaptureError)
                })
                .transpose()?;
            if let Some(receipt) = &receipt {
                fs::create_dir_all(
                    receipt
                        .parent()
                        .ok_or_else(|| CaptureError("missing pipeline scope".into()))?,
                )?;
            }
            let run = receipt
                .as_ref()
                .map(|receipt| resolve_run(&common, receipt))
                .unwrap_or(RunAssociation::None);
            let coordinator = receipt
                .as_ref()
                .map(|receipt| receipt.with_file_name("coordinator.json"));
            let before = coordinator
                .as_deref()
                .map(coordinator_snapshot)
                .unwrap_or_else(|| json!({"status":"absent"}));
            let operation = begin(
                &common,
                BeginInput {
                    phase: "pipeline-observation".into(),
                    invocation_id: format!(
                        "{}-{}",
                        std::process::id(),
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos()
                    ),
                    run_id: None,
                    worktree: repo.to_path_buf(),
                    host: "gal".into(),
                    host_session: None,
                    owner_pid: Some(std::process::id()),
                },
            )?
            .operation_id;
            let observation = Self {
                common,
                run,
                operation,
                receipt,
                coordinator,
                before,
                worktree: repo.to_path_buf(),
                changes_before: working_tree_changes(repo),
                request,
                invariants: serde_json::Map::new(),
                started: std::time::Instant::now(),
            };
            observation.publish(
                "transition-expected",
                json!({"request":observation.request,"before":observation.before}),
            )?;
            Ok(observation)
        })();
        match result {
            Ok(observation) => Some(observation),
            Err(error) => {
                eprintln!("pipeline evidence capture failed: {error}");
                None
            }
        }
    }

    fn publish(&self, kind: &str, content: Value) -> Result<(), CaptureError> {
        let run = match &self.run {
            RunAssociation::Bound(run) => run.clone(),
            _ => entry_id(&self.operation)?,
        };
        EvidenceStore::open(&self.common)?.publish(&event(
            &run,
            &self.operation,
            &format!("{}-{kind}", self.operation),
            "pipeline",
            kind,
            content,
        ))?;
        Ok(())
    }

    pub(crate) fn invariant(&mut self, id: &str, passed: bool) {
        if self
            .invariants
            .get(id)
            .is_some_and(|result| result["status"] == "fail")
        {
            return;
        }
        self.invariants.insert(
            id.into(),
            json!({"status":if passed { "pass" } else { "fail" }}),
        );
    }

    pub(crate) fn association(&self) -> RunAssociation {
        match &self.run {
            RunAssociation::Bound(run) => RunAssociation::Bound(run.clone()),
            RunAssociation::None => RunAssociation::None,
            RunAssociation::Unknown => RunAssociation::Unknown,
        }
    }

    pub(crate) fn terminate_pipeline(&mut self, completed: bool, reason: &str) {
        let result = (|| -> Result<(), CaptureError> {
            let Some(receipt) = self.receipt.as_ref() else {
                return Ok(());
            };
            let resolved = resolve_run(&self.common, receipt);
            if !matches!(
                (&self.run, &resolved),
                (RunAssociation::None, RunAssociation::Unknown)
            ) {
                self.run = resolved;
            }
            let RunAssociation::Bound(run) = &self.run else {
                return Ok(());
            };
            let Some(operation) = pipeline_operation(&self.common, receipt, run)? else {
                return Ok(());
            };
            end(
                &self.common,
                &operation,
                if completed { "completed" } else { "stopped" },
                None,
                (!completed).then_some(reason),
            )?;
            println!("terminal_operation_id: {operation}");
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("pipeline evidence capture failed: {error}");
        }
    }

    pub(crate) fn pipeline_operation(
        &mut self,
        run: &str,
        repo: &Path,
        host: Option<&str>,
        session: Option<&str>,
    ) -> Result<String, CaptureError> {
        let operation = begin(
            &self.common,
            BeginInput {
                phase: "pipeline".into(),
                invocation_id: self.operation.clone(),
                run_id: Some(run.into()),
                worktree: repo.to_path_buf(),
                host: host.unwrap_or("gal").into(),
                host_session: session.map(str::to_owned),
                owner_pid: None,
            },
        )?
        .operation_id;
        let receipt = self
            .receipt
            .as_ref()
            .ok_or_else(|| CaptureError("pipeline scope unavailable".into()))?;
        associate_run(&self.common, receipt, run, &operation)?;
        self.run = RunAssociation::Bound(run.into());
        Ok(operation)
    }

    pub(crate) fn finish(mut self, exit: gal_engine::ExitCode, detail: Value) {
        let result = (|| -> Result<(), CaptureError> {
            if let RunAssociation::Bound(run) = &self.run {
                let store = EvidenceStore::open(&self.common)?;
                let entry = entry_id(&self.operation)?;
                for id in [
                    &self.operation,
                    &format!("{}-transition-expected", self.operation),
                ] {
                    match store.read(&entry, id) {
                        Ok(mut record) => {
                            record.run_id = run.clone();
                            store.publish(&record)?;
                        }
                        Err(StoreError::NotFound(_)) => {}
                        Err(error) => return Err(error.into()),
                    }
                }
            }
            let after = self
                .coordinator
                .as_deref()
                .map(coordinator_snapshot)
                .unwrap_or_else(|| json!({"status":"absent"}));
            let coordinated = [&self.before, &after]
                .iter()
                .any(|snapshot| snapshot["state"]["profile"] == "codex_stop_v1");
            for id in [
                "inv-authorization-binding",
                "inv-allowed-edge",
                "inv-completed-evidence",
                "inv-single-custody",
                "inv-terminal-quiescence",
            ] {
                if !coordinated || !self.invariants.contains_key(id) {
                    self.invariants.insert(id.into(), json!({"status":"not-evaluated", "reason":if coordinated { "boundary-check-not-run" } else { "no-coordinator" }}));
                }
            }
            let association = match &self.run {
                RunAssociation::Bound(_) => "bound",
                RunAssociation::None => "no-bound-run",
                RunAssociation::Unknown => "run-association-unknown",
            };
            let required_receipts = self
                .receipt
                .as_ref()
                .map(|receipt| {
                    vec![
                        receipt.clone(),
                        receipt.with_file_name("orchestrator-receipt.json"),
                        receipt.with_file_name("pipeline-handback-check.receipt.md"),
                    ]
                })
                .unwrap_or_default();
            self.publish("transition", json!({"request":self.request,"before":self.before["state"],"after":after["state"],
                "before_read":self.before,"after_read":after,"before_revision":self.before["state"]["revision"],"after_revision":after["state"]["revision"],
                "task_cursor":{"before":self.before["state"]["task_id"],"after":after["state"]["task_id"]},
                "required_receipts":required_receipts,
                "allowed_files":self.request.get("allowed_files").cloned().unwrap_or_else(|| json!([])),"actual_change_summary":{"coordinator_changed":self.before != after,"before":self.changes_before,"after":working_tree_changes(&self.worktree)},
                "invariants":self.invariants,"run_association":association,"exit_code":exit as i32,"duration_ms":self.started.elapsed().as_millis(),"detail":detail}))?;
            end(&self.common, &self.operation, "completed", None, None)
        })();
        if let Err(error) = result {
            eprintln!("pipeline evidence capture failed: {error}");
        }
    }
}

pub(crate) fn pipeline_operation(
    common: &Path,
    receipt: &Path,
    run: &str,
) -> Result<Option<String>, CaptureError> {
    let store = EvidenceStore::open(common)?;
    let mut active = Vec::new();
    for record in entries(&store, run)? {
        if record.source_kind == "operation-begin" && record.phase == "pipeline" {
            let entry = entry_id(&record.operation_id)?;
            match store.read(&entry, &format!("{}-end", record.operation_id)) {
                Ok(_) => {}
                Err(StoreError::NotFound(_)) => active.push(record.operation_id),
                Err(error) => return Err(error.into()),
            }
        }
    }
    active.sort();
    active.dedup();
    if active.len() > 1 {
        return Err(CaptureError(format!(
            "multiple pipeline operations for {}",
            receipt.display()
        )));
    }
    Ok(active.pop())
}

fn scope_entry(receipt: &Path) -> String {
    let scope = receipt.parent().unwrap_or(Path::new("."));
    let scope = fs::canonicalize(scope).unwrap_or_else(|_| scope.to_path_buf());
    format!(
        "entry-scope-{}",
        &digest_bytes(scope.to_string_lossy().as_bytes())[..24]
    )
}

pub(crate) fn resolve_run(common: &Path, receipt: &Path) -> RunAssociation {
    fn resolve(common: &Path, receipt: &Path) -> Result<RunAssociation, CaptureError> {
        let store = EvidenceStore::open(common)?;
        let associated = match store.read(&scope_entry(receipt), "run-association") {
            Ok(record) => Some(
                record.content["run_id"]
                    .as_str()
                    .ok_or_else(|| CaptureError("malformed association".into()))?
                    .to_owned(),
            ),
            Err(StoreError::NotFound(_)) => None,
            Err(error) => return Err(error.into()),
        };
        let text = match fs::read_to_string(receipt) {
            Ok(text) => Some(text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let receipt_run = text
            .as_deref()
            .map(|text| receipt_field(text, "run_id"))
            .transpose()
            .map_err(CaptureError)?
            .flatten();
        if associated
            .as_deref()
            .into_iter()
            .chain(receipt_run)
            .any(|id| {
                id.is_empty()
                    || !id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            })
        {
            return Ok(RunAssociation::Unknown);
        }
        let coordinator = receipt
            .parent()
            .unwrap_or(Path::new("."))
            .join("coordinator.json");
        let coordinator_run = match fs::read(coordinator) {
            Ok(bytes) => {
                let value: Value = serde_json::from_slice(&bytes)
                    .map_err(|error| CaptureError(error.to_string()))?;
                Some(
                    value["accounting_run"]["run_id"]
                        .as_str()
                        .ok_or_else(|| CaptureError("missing coordinator run".into()))?
                        .to_owned(),
                )
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let run = associated.as_deref().or(receipt_run);
        if let Some(run) = run {
            if receipt_run.is_some_and(|other| other != run)
                || coordinator_run.as_deref().is_some_and(|other| other != run)
            {
                return Ok(RunAssociation::Unknown);
            }
            Ok(RunAssociation::Bound(run.to_owned()))
        } else if coordinator_run.is_some() {
            Ok(RunAssociation::Unknown)
        } else {
            Ok(RunAssociation::None)
        }
    }
    resolve(common, receipt).unwrap_or(RunAssociation::Unknown)
}

pub(crate) fn associate_run(
    common: &Path,
    receipt: &Path,
    run: &str,
    operation: &str,
) -> Result<(), CaptureError> {
    let store = EvidenceStore::open(common)?;
    let entry = scope_entry(receipt);
    match store.read(&entry, "run-association") {
        Ok(record) if record.content["run_id"].as_str() == Some(run) => return Ok(()),
        Ok(_) => return Err(CaptureError("conflicting run association".into())),
        Err(StoreError::NotFound(_)) => {}
        Err(error) => return Err(error.into()),
    }
    store.publish(&event(
        &entry,
        operation,
        "run-association",
        "pipeline",
        "run-association",
        json!({"run_id":run}),
    ))?;
    Ok(())
}

fn lock(common_dir: &Path) -> Result<fs::File, CaptureError> {
    fs::create_dir_all(common_dir.join("gal/evidence/v1"))?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(common_dir.join("gal/evidence/v1/.operation-lock"))?;
    file.lock_exclusive()?;
    Ok(file)
}
fn ownership(record: &EvidenceRecord) -> AttemptOwnership {
    let Some(pid) = record.content["owner_pid"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
    else {
        return AttemptOwnership::Unknown;
    };
    let observed = dispatch::dispatch::process_birth_identity(pid);
    if observed.as_deref().is_some()
        && observed.as_deref() == record.content["owner_birth"].as_str()
    {
        return AttemptOwnership::Live;
    }
    if observed.is_some() {
        return AttemptOwnership::Unknown;
    }
    #[cfg(target_os = "linux")]
    if Path::new("/proc").join(pid.to_string()).exists() {
        return AttemptOwnership::Unknown;
    }
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output();
        return match output {
            Ok(output) if output.status.success() => {
                let text = String::from_utf8_lossy(&output.stdout);
                if text.lines().any(|line| {
                    line.split(',')
                        .nth(1)
                        .is_some_and(|v| v.trim_matches('"') == pid.to_string())
                }) {
                    AttemptOwnership::Unknown
                } else {
                    AttemptOwnership::Dead
                }
            }
            _ => AttemptOwnership::Unknown,
        };
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    return AttemptOwnership::Unknown;
    #[cfg(target_os = "linux")]
    AttemptOwnership::Dead
}
fn inspect_unfinished(store: &EvidenceStore, common_dir: &Path) -> Result<u64, CaptureError> {
    let mut sequence = 0;
    for entry in fs::read_dir(common_dir.join("gal/evidence/v1"))? {
        let entry = entry?;
        let run = entry.file_name().to_string_lossy().into_owned();
        if !run.starts_with("entry-") || !entry.file_type()?.is_dir() {
            continue;
        }
        let records = entries(store, &run)?;
        for begin in records
            .iter()
            .filter(|r| r.source_kind == "operation-begin")
        {
            sequence = sequence.max(begin.content["sequence"].as_u64().unwrap_or(0));
            let end = format!("{}-end", begin.operation_id);
            let interrupted = format!("{}-interrupted", begin.operation_id);
            if records
                .iter()
                .any(|r| r.event_id == end || r.event_id == interrupted)
            {
                continue;
            }
            if ownership(begin) == AttemptOwnership::Dead {
                let content = json!({"status":"interrupted","ownership":"dead"});
                store.publish(&event(
                    &run,
                    &begin.operation_id,
                    &interrupted,
                    &begin.phase,
                    "operation-interrupted",
                    content,
                ))?;
            }
        }
    }
    Ok(sequence)
}
fn entry_id(op: &str) -> Result<String, CaptureError> {
    let hex = op
        .strip_prefix("op-")
        .filter(|x| {
            x.len() == 16
                && x.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
        .ok_or_else(|| CaptureError("invalid operation id".into()))?;
    Ok(format!("entry-{hex}"))
}
fn binding(input: &BeginInput) -> Value {
    let reason = if input.host_session.is_none() {
        Some("session-absent")
    } else if !["codex", "claude"].contains(&input.host.as_str()) {
        Some("host-unsupported")
    } else {
        match bind_orchestrator(
            Some(&input.host),
            input.host_session.as_deref(),
            &input.worktree,
            "",
            0,
        ) {
            OrchestratorRecord::Bound(_) => None,
            _ => Some("session-mismatch"),
        }
    };
    match reason {
        Some(reason) => json!({"status":"unavailable", "reason":reason}),
        None => json!({"status":"verified", "reason":null}),
    }
}
fn marker(input: &BeginInput, message: &str) {
    let dir = input.worktree.join(".dev/pipeline/usage-gap");
    if fs::create_dir_all(&dir).is_ok() {
        let key = digest_bytes(input.invocation_id.as_bytes());
        let _ = fs::write(
            dir.join(format!("{}.json", &key[..16])),
            json!({"phase":input.phase,"invocation_id":input.invocation_id,"reason":message})
                .to_string(),
        );
    }
}
fn import_markers(
    store: &EvidenceStore,
    run: &str,
    op: &str,
    input: &BeginInput,
) -> Result<(), CaptureError> {
    let dir = input.worktree.join(".dev/pipeline/usage-gap");
    let files = match fs::read_dir(dir) {
        Ok(files) => files,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    for file in files {
        let path = file?.path();
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path)?;
        let content: Value =
            serde_json::from_slice(&bytes).map_err(|e| CaptureError(e.to_string()))?;
        let id = format!("gap-{}", &digest_bytes(&bytes)[..16]);
        let record = event(run, op, &id, &input.phase, "capture-gap", content);
        match store.read(run, &id) {
            Ok(_) => {}
            Err(StoreError::NotFound(_)) => {
                store.publish(&record)?;
            }
            Err(e) => return Err(e.into()),
        };
        fs::remove_file(path)?;
    }
    Ok(())
}

pub(crate) fn begin(common_dir: &Path, input: BeginInput) -> Result<BeginOutcome, CaptureError> {
    let result = begin_inner(common_dir, &input);
    if let Err(e) = &result {
        if !e.0.starts_with("Closed:") {
            marker(&input, &e.0);
        }
    }
    result
}
fn begin_inner(common_dir: &Path, input: &BeginInput) -> Result<BeginOutcome, CaptureError> {
    let store = EvidenceStore::open(common_dir)?;
    let _lock = lock(common_dir)?;
    refuse_closed_worktree(common_dir, &input.worktree, None)?;
    let sequence = inspect_unfinished(&store, common_dir)?
        .checked_add(1)
        .ok_or_else(|| CaptureError("operation sequence exhausted".into()))?;
    if let Some(run) = input
        .run_id
        .as_ref()
        .filter(|_| matches!(input.phase.as_str(), "pipeline" | "finalize"))
    {
        for directory in fs::read_dir(common_dir.join("gal/evidence/v1"))? {
            let directory = directory?;
            let entry = directory.file_name().to_string_lossy().into_owned();
            if !entry.starts_with("entry-") || !directory.file_type()?.is_dir() {
                continue;
            }
            for record in entries(&store, &entry)? {
                if record.source_kind != "operation-begin"
                    || record.phase != input.phase
                    || record.content["run_id"].as_str() != Some(run)
                {
                    continue;
                }
                match store.read(&entry, &format!("{}-end", record.operation_id)) {
                    Ok(_) => continue,
                    Err(StoreError::NotFound(_)) => {}
                    Err(e) => return Err(e.into()),
                }
                let mut linked = record.clone();
                linked.run_id = run.clone();
                linked.event_id = format!("{}-{}", input.phase, record.operation_id);
                store.publish(&linked)?;
                import_markers(&store, &entry, &record.operation_id, input)?;
                return Ok(BeginOutcome {
                    operation_id: record.operation_id,
                    binding: record.content["binding"].clone(),
                });
            }
        }
        for record in entries(&store, run)? {
            if record.source_kind == "operation-begin" && record.phase == input.phase {
                let entry = entry_id(&record.operation_id)?;
                match store.read(&entry, &format!("{}-end", record.operation_id)) {
                    Ok(_) => {}
                    Err(StoreError::NotFound(_)) => {
                        import_markers(&store, &entry, &record.operation_id, input)?;
                        return Ok(BeginOutcome {
                            operation_id: record.operation_id,
                            binding: record.content["binding"].clone(),
                        });
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
    let tuple = json!([
        1,
        input.phase,
        input.worktree.to_string_lossy(),
        input.host,
        input.host_session,
        input.invocation_id
    ]);
    let digest =
        digest_bytes(&serde_json::to_vec(&tuple).map_err(|e| CaptureError(e.to_string()))?);
    let op = format!("op-{}", &digest[..16]);
    let entry = entry_id(&op)?;
    match store.read(&entry, &op) {
        Ok(record) => {
            if let Some(run) = &input.run_id {
                let mut linked = record.clone();
                linked.run_id = run.clone();
                linked.event_id = format!("{}-{op}", input.phase);
                store.publish(&linked)?;
            }
            import_markers(&store, &entry, &op, input)?;
            return Ok(BeginOutcome {
                operation_id: op,
                binding: record.content["binding"].clone(),
            });
        }
        Err(StoreError::NotFound(_)) => {}
        Err(e) => return Err(e.into()),
    }
    let binding = binding(input);
    let roster = read_design_roster_version(&input.worktree)?;
    let content = json!({"phase":input.phase,"invocation_id":input.invocation_id,"worktree":input.worktree,"host":input.host,"host_session":input.host_session,"binding":binding,"run_id":input.run_id,"owner_pid":input.owner_pid,"owner_birth":input.owner_pid.and_then(dispatch::dispatch::process_birth_identity),"sequence":sequence,"design_roster_version":roster.design_roster_version,"design_roster_reason":roster.reason});
    let record = event(&entry, &op, &op, &input.phase, "operation-begin", content);
    store.publish(&record)?;
    if let Some(run) = &input.run_id {
        let mut linked = record.clone();
        linked.run_id = run.clone();
        linked.event_id = format!("{}-{op}", input.phase);
        store.publish(&linked)?;
    }
    import_markers(&store, &entry, &op, input)?;
    Ok(BeginOutcome {
        operation_id: op,
        binding,
    })
}

pub(crate) struct ArchiveRequest {
    pub target: Option<PathBuf>,
    pub run: String,
    pub worktree: PathBuf,
    pub boundary: String,
    pub proof: Option<String>,
    pub source_set: Option<String>,
    pub prior_proof: Option<String>,
}

#[derive(PartialEq, Eq)]
struct ArchivedSource {
    path: PathBuf,
    original_sha256: Option<String>,
    original_length: usize,
    content: Option<String>,
    archived_sha256: Option<String>,
}

struct ArchiveProof {
    schema_version: u32,
    run_id: String,
    source_worktree: PathBuf,
    boundary: String,
    plan: PathBuf,
    scope: String,
    source_set_id: String,
    terminal_operation_id: String,
    prior_proof: Option<String>,
    redaction_rule_version: String,
    redaction_scope: String,
    closure_markers: BTreeMap<PathBuf, String>,
    archive_set: Vec<ArchivedSource>,
    to_delete_set: BTreeSet<PathBuf>,
    collector_versions: Value,
}

impl ArchiveProof {
    fn to_value(&self) -> Value {
        let sources: Vec<_> = self
            .archive_set
            .iter()
            .map(|source| {
                json!({
                    "path":source.path,"original_sha256":source.original_sha256,
                    "original_length":source.original_length,"content":source.content,
                    "archived_sha256":source.archived_sha256
                })
            })
            .collect();
        json!({"schema_version":self.schema_version,"run_id":self.run_id,
            "source_worktree":self.source_worktree,"boundary":self.boundary,"plan":self.plan,
            "scope":self.scope,"source_set_id":self.source_set_id,"terminal_operation_id":self.terminal_operation_id,
            "prior_proof":self.prior_proof,"redaction_rule_version":self.redaction_rule_version,
            "redaction_scope":self.redaction_scope,"closure_markers":self.closure_markers,
            "archive_set":sources,"to_delete_set":self.to_delete_set,"collector_versions":self.collector_versions})
    }

    fn from_value(value: Value) -> Result<Self, CaptureError> {
        let string = |key| {
            value[key]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| archive_error(format!("missing proof field {key}")))
        };
        let paths = |key| {
            serde_json::from_value::<PathBuf>(value[key].clone())
                .map_err(|error| archive_error(error.to_string()))
        };
        let sources = value["archive_set"]
            .as_array()
            .ok_or_else(|| archive_error("missing archive set"))?;
        let mut archive_set = Vec::new();
        for source in sources {
            let optional = |key| match &source[key] {
                Value::Null => Ok(None),
                Value::String(value) => Ok(Some(value.clone())),
                _ => Err(archive_error("malformed archive source")),
            };
            archive_set.push(ArchivedSource {
                path: serde_json::from_value(source["path"].clone())
                    .map_err(|error| archive_error(error.to_string()))?,
                original_sha256: optional("original_sha256")?,
                original_length: source["original_length"]
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| archive_error("invalid archive length"))?,
                content: optional("content")?,
                archived_sha256: optional("archived_sha256")?,
            });
        }
        Ok(Self {
            schema_version: value["schema_version"]
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| archive_error("invalid archive schema"))?,
            run_id: string("run_id")?,
            source_worktree: paths("source_worktree")?,
            boundary: string("boundary")?,
            plan: paths("plan")?,
            scope: string("scope")?,
            source_set_id: string("source_set_id")?,
            terminal_operation_id: string("terminal_operation_id")?,
            prior_proof: value["prior_proof"].as_str().map(str::to_owned),
            redaction_rule_version: string("redaction_rule_version")?,
            redaction_scope: string("redaction_scope")?,
            closure_markers: serde_json::from_value(value["closure_markers"].clone())
                .map_err(|error| archive_error(error.to_string()))?,
            archive_set,
            to_delete_set: serde_json::from_value(value["to_delete_set"].clone())
                .map_err(|error| archive_error(error.to_string()))?,
            collector_versions: value["collector_versions"].clone(),
        })
    }
}

fn archive_error(message: impl Into<String>) -> CaptureError {
    CaptureError(message.into())
}

fn archive_json(value: &Value) -> Result<Vec<u8>, CaptureError> {
    serde_json::to_vec(value).map_err(|error| archive_error(error.to_string()))
}

fn archive_root(common: &Path) -> PathBuf {
    common.join("gal/archive/v1")
}

fn check_archive_path(path: &Path) -> Result<(), CaptureError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(archive_error("archive source contains parent traversal"));
        }
        current.push(component);
        if matches!(component, std::path::Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                #[cfg(windows)]
                let linked = {
                    use std::os::windows::fs::MetadataExt;
                    metadata.file_attributes() & 0x400 != 0
                };
                #[cfg(not(windows))]
                let linked = metadata.file_type().is_symlink();
                if linked {
                    return Err(archive_error(format!(
                        "archive path contains a link: {}",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn archive_write(path: &Path, bytes: &[u8]) -> Result<(), CaptureError> {
    use std::io::Write;
    check_archive_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| archive_error("archive path has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn archive_replace(path: &Path, bytes: &[u8]) -> Result<(), CaptureError> {
    check_archive_path(path)?;
    let temporary = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    archive_write(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

pub(crate) fn refuse_closed_worktree(
    common: &Path,
    worktree: &Path,
    plan_scope: Option<&str>,
) -> Result<(), CaptureError> {
    let root = worktree.canonicalize()?;
    let directory = archive_root(&common.canonicalize()?).join("closures");
    check_archive_path(&directory)?;
    let files = match fs::read_dir(directory) {
        Ok(files) => files,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for file in files {
        let path = file?.path();
        check_archive_path(&path)?;
        let value: Value = serde_json::from_slice(&fs::read(path)?)
            .map_err(|error| archive_error(error.to_string()))?;
        if value["source_worktree"] == json!(root)
            && plan_scope.is_none_or(|scope| {
                value["scope"]
                    .as_str()
                    .is_none_or(|archived| archived.is_empty() || archived == scope)
            })
        {
            return Err(archive_error("Closed: source worktree is archived"));
        }
    }
    Ok(())
}

fn archive_context(request: &ArchiveRequest) -> Result<(PathBuf, PathBuf), CaptureError> {
    correction_store_id(&json!(request.run))?;
    check_archive_path(&request.worktree)?;
    let root = request.worktree.canonicalize()?;
    let top = std::process::Command::new("git")
        .current_dir(&root)
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !top.status.success()
        || PathBuf::from(String::from_utf8_lossy(&top.stdout).trim()).canonicalize()? != root
    {
        return Err(archive_error("source worktree must be a Git worktree root"));
    }
    let common = super::usage::git_common_dir_in(&root)
        .map_err(archive_error)?
        .canonicalize()?;
    let caller = std::env::current_dir()?;
    if super::usage::git_common_dir_in(&caller)
        .map_err(archive_error)?
        .canonicalize()?
        != common
    {
        return Err(archive_error(
            "source worktree belongs to a different repository",
        ));
    }
    check_archive_path(&archive_root(&common))?;
    Ok((common, root))
}

fn archive_files(
    path: &Path,
    files: &mut BTreeSet<PathBuf>,
    skip_store_locks: bool,
) -> Result<(), CaptureError> {
    check_archive_path(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            files.insert(path.to_path_buf());
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    if metadata.is_dir() {
        for file in fs::read_dir(path)? {
            let file = file?;
            if !skip_store_locks || file.file_name() != ".lock" {
                archive_files(&file.path(), files, skip_store_locks)?;
            }
        }
    } else if metadata.is_file() {
        files.insert(path.to_path_buf());
    } else {
        return Err(archive_error(format!(
            "unsupported archive source: {}",
            path.display()
        )));
    }
    Ok(())
}

fn cleaned_host_artifacts(
    path: &Path,
    bytes: &[u8],
    root: &Path,
    run: &str,
    scope: &str,
) -> Result<Option<[PathBuf; 2]>, CaptureError> {
    use dispatch::host_attempt_state::{HostAttemptState, HostTerminalState};

    let scope_dir = root.join(".dev/pipeline").join(scope);
    let Some(parts) = path.strip_prefix(&scope_dir).ok().map(Path::components) else {
        return Ok(None);
    };
    let parts: Vec<_> = parts.map(|part| part.as_os_str()).collect();
    let [task, attempts, filename] = parts.as_slice() else {
        return Ok(None);
    };
    let Some(attempt) = filename
        .to_str()
        .and_then(|name| name.strip_suffix(".json"))
    else {
        return Ok(None);
    };
    if *attempts != "host-attempts" {
        return Ok(None);
    }

    let state = HostAttemptState::decode_immutable(bytes)
        .map_err(|error| archive_error(format!("invalid host attempt manifest: {error}")))?;
    let task_dir = scope_dir.join(task);
    let attempt_dir = task_dir.join(attempts);
    check_archive_path(&state.worktree)?;
    check_archive_path(&state.prompt)?;
    if state.attempt_id != attempt
        || state.task.as_str() != *task
        || state.run_id != run
        || state.mode != "host-subagent"
        || !matches!(state.phase.as_str(), "implement" | "test" | "audit")
        || state.worktree.canonicalize()? != root
        || state.prompt.canonicalize()?
            != root.join(".dev/plans").join(format!("{scope}.prompt.md"))
    {
        return Err(archive_error("host attempt archive identity mismatch"));
    }

    let spec = attempt_dir.join(format!("{attempt}.spec.md"));
    let snapshot = task_dir.join(format!("{}-{}.prompt.bak", state.task, state.phase));
    let log = attempt_dir.join(format!("{attempt}.log.md"));
    for (actual, expected) in [
        (&state.spec_path, &spec),
        (&state.snapshot_path, &snapshot),
        (&state.log_path, &log),
    ] {
        check_archive_path(actual)?;
        if !actual.is_absolute()
            || actual.file_name() != expected.file_name()
            || actual.parent().map(Path::canonicalize).transpose()?
                != expected.parent().map(Path::to_path_buf)
        {
            return Err(archive_error("host attempt archive artifact path mismatch"));
        }
    }

    let claim = path.with_extension("close-claim");
    check_archive_path(&claim)?;
    if claim.try_exists()? {
        return Err(archive_error("host attempt still holds a close claim"));
    }
    let log_text = fs::read_to_string(log)?;
    let mut lines = log_text.lines();
    if lines.next() != Some("GAL-DISPATCH-LOG v1") {
        return Err(archive_error("invalid host attempt archive log"));
    }
    let mut header = BTreeMap::new();
    let mut header_ended = false;
    for line in lines {
        if line == "---STDOUT---" {
            header_ended = true;
            break;
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| archive_error("invalid host attempt archive log header"))?;
        if header.insert(key, value.trim()).is_some() {
            return Err(archive_error("duplicate host attempt archive log field"));
        }
    }
    for (key, expected) in [
        ("attempt_id", state.attempt_id.as_str()),
        ("executor", "host-subagent"),
        ("host", state.host.as_str()),
        ("task_id", state.task.as_str()),
        ("phase", state.phase.as_str()),
        ("timestamp_start", state.started_at.as_str()),
    ] {
        if header.get(key).copied() != Some(expected) {
            return Err(archive_error("host attempt archive log identity mismatch"));
        }
    }

    if !header_ended {
        return Err(archive_error(
            "unterminated host attempt archive log header",
        ));
    }
    let terminal = header
        .get("terminal_state")
        .and_then(|state| HostTerminalState::parse(state));
    Ok(matches!(
        terminal,
        Some(HostTerminalState::Completed | HostTerminalState::Abandoned)
    )
    .then_some([snapshot, spec]))
}

fn registered_archive_paths(
    value: &Value,
    root: &Path,
    files: &mut BTreeSet<PathBuf>,
    cleaned: Option<&[PathBuf; 2]>,
) -> Result<(), CaptureError> {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if matches!(
                    key.as_str(),
                    "receipt_path"
                        | "spec_path"
                        | "stdout_path"
                        | "stderr_path"
                        | "output_path"
                        | "manifest_path"
                        | "task_spec_path"
                        | "source_path"
                        | "log_path"
                        | "log_ptr"
                        | "snapshot_path"
                        | "binding_path"
                ) {
                    if let Some(path) = value.as_str().filter(|path| !path.is_empty()) {
                        let path = root.join(path);
                        check_archive_path(&path)?;
                        let embedded_receipt = key == "receipt_path"
                            && fields
                                .get("receipt_text")
                                .and_then(Value::as_str)
                                .is_some_and(|text| {
                                    fields.get("receipt_sha256").and_then(Value::as_str)
                                        == Some(digest_bytes(text.as_bytes()).as_str())
                                });
                        let cleaned_path = match (key.as_str(), cleaned) {
                            ("snapshot_path", Some([snapshot, _])) => Some(snapshot),
                            ("spec_path", Some([_, spec])) => Some(spec),
                            _ => None,
                        };
                        if let Some(cleaned_path) = cleaned_path {
                            match fs::symlink_metadata(&path) {
                                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                                    files.insert(cleaned_path.clone());
                                    continue;
                                }
                                Err(error) => return Err(error.into()),
                                Ok(_) => {}
                            }
                        }
                        if !path.is_file() && !embedded_receipt {
                            return Err(archive_error(format!(
                                "uncovered registered source: {}",
                                path.display()
                            )));
                        }
                        let hash_key = key.strip_suffix("_path").map(|key| format!("{key}_sha256"));
                        if let Some(expected) = hash_key
                            .as_ref()
                            .and_then(|key| fields.get(key))
                            .and_then(Value::as_str)
                        {
                            if !embedded_receipt && digest_bytes(&fs::read(&path)?) != expected {
                                return Err(archive_error("registered source digest mismatch"));
                            }
                        }
                        files.insert(if path.exists() {
                            path.canonicalize()?
                        } else {
                            path
                        });
                    }
                } else if matches!(key.as_str(), "required_receipts" | "registered_sources") {
                    if let Some(paths) = value.as_array() {
                        for path in paths {
                            if let Some(path) = path.as_str() {
                                let path = root.join(path);
                                check_archive_path(&path)?;
                                if !path.is_file() {
                                    return Err(archive_error(format!(
                                        "uncovered registered source: {}",
                                        path.display()
                                    )));
                                }
                                files.insert(path.canonicalize()?);
                            }
                        }
                    }
                }
                registered_archive_paths(value, root, files, None)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                registered_archive_paths(value, root, files, None)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn unbound_archive_operation_matches_scope(
    records: &[EvidenceRecord],
    operation_id: &str,
    root: &Path,
    scope: &str,
) -> bool {
    let pipeline_root = root.join(dispatch::dispatch::PIPELINE_ROOT);
    let mut has_foreign_receipt = false;
    for record in records.iter().filter(|record| {
        record.operation_id == operation_id
            && matches!(
                record.source_kind.as_str(),
                "gate-result" | "verification-expected"
            )
    }) {
        let Some(path) = record.content["receipt_path"].as_str() else {
            continue;
        };
        let path = Path::new(path);
        if path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            continue;
        }
        let path = root.join(path);
        let Some(base) = path.ancestors().find(|ancestor| {
            ancestor.ends_with(dispatch::dispatch::PIPELINE_ROOT)
                && ancestor.canonicalize().ok().as_deref() == Some(&pipeline_root)
        }) else {
            continue;
        };
        let Ok(relative) = path.strip_prefix(base) else {
            continue;
        };
        let mut components = relative.components();
        if let (Some(std::path::Component::Normal(receipt_scope)), Some(_)) =
            (components.next(), components.next())
        {
            if receipt_scope == scope {
                return true;
            }
            has_foreign_receipt = true;
        }
    }
    !has_foreign_receipt
}
fn archive_evidence_entries(
    common: &Path,
    root: &Path,
    run: &str,
    scope: &str,
) -> Result<BTreeSet<String>, CaptureError> {
    let store = EvidenceStore::open(common)?;
    let mut selected = BTreeSet::from([
        run.to_owned(),
        scope_entry(
            &root
                .join(".dev/pipeline")
                .join(scope)
                .join("preflight.receipt.md"),
        ),
    ]);
    for record in entries(&store, run)? {
        if let Ok(entry) = entry_id(&record.operation_id) {
            selected.insert(entry);
        }
    }
    for directory in fs::read_dir(common.join("gal/evidence/v1"))? {
        let directory = directory?;
        let name = directory.file_name().to_string_lossy().into_owned();
        if !name.starts_with("entry-") || !directory.file_type()?.is_dir() {
            continue;
        }
        check_archive_path(&directory.path())?;
        let records = entries(&store, &name)?;
        for record in &records {
            if record.source_kind != "operation-begin" {
                continue;
            }
            let operation_run = &record.content["run_id"];
            if operation_run.as_str() == Some(run)
                || (operation_run.is_null()
                    && record.content["worktree"].as_str().is_some_and(|path| {
                        Path::new(path).canonicalize().ok().as_deref() == Some(root)
                    })
                    && unbound_archive_operation_matches_scope(
                        &records,
                        &record.operation_id,
                        root,
                        scope,
                    ))
            {
                selected.insert(name.clone());
            }
        }
    }
    Ok(selected)
}

fn archive_writers_stopped(
    common: &Path,
    selected: &BTreeSet<String>,
    root: &Path,
    scope: &str,
) -> Result<(), CaptureError> {
    let store = EvidenceStore::open(common)?;
    for entry in selected {
        let records = entries(&store, entry)?;
        for record in records
            .iter()
            .filter(|record| record.source_kind == "operation-begin")
        {
            if records.iter().any(|other| {
                other.operation_id == record.operation_id
                    && matches!(
                        other.source_kind.as_str(),
                        "operation-end" | "operation-interrupted"
                    )
            }) {
                continue;
            }
            let operation_entry = entry_id(&record.operation_id)?;
            if store
                .read(&operation_entry, &format!("{}-end", record.operation_id))
                .is_ok()
                || store
                    .read(
                        &operation_entry,
                        &format!("{}-interrupted", record.operation_id),
                    )
                    .is_ok()
            {
                continue;
            }
            if record.phase == "finalize" && record.content["owner_pid"].is_null() {
                continue;
            }
            return Err(archive_error(format!(
                "active writer: {}",
                record.operation_id
            )));
        }
    }
    let leases = root
        .join(dispatch::dispatch::LEASE_ROOT)
        .join("prompt-writers");
    check_archive_path(&leases)?;
    if leases.exists() {
        for file in fs::read_dir(leases)? {
            let path = file?.path();
            check_archive_path(&path)?;
            let text = fs::read_to_string(path)?;
            for line in text.lines().filter_map(|line| line.strip_prefix("prompt=")) {
                let prompt = Path::new(line);
                if super::dispatch::resolve_scope(prompt).ok().as_deref() == Some(scope) {
                    return Err(archive_error("active prompt writer lease"));
                }
            }
        }
    }
    let coordinator = root
        .join(".dev/pipeline")
        .join(scope)
        .join("coordinator.json");
    if coordinator.exists() {
        check_archive_path(&coordinator)?;
        let state: Value = serde_json::from_slice(&fs::read(coordinator)?)
            .map_err(|error| archive_error(error.to_string()))?;
        if state
            .get("active_attempt")
            .is_some_and(|value| !value.is_null())
            || state
                .get("active_attempt_id")
                .is_some_and(|value| !value.is_null())
        {
            return Err(archive_error("active coordinator writer"));
        }
    }
    Ok(())
}

fn archive_primary_files(
    common: &Path,
    root: &Path,
    run: &str,
    scope: &str,
    selected: &BTreeSet<String>,
) -> Result<BTreeSet<PathBuf>, CaptureError> {
    let mut files = BTreeSet::new();
    for path in [
        format!(".dev/plans/{scope}.md"),
        format!(".dev/plans/{scope}.prompt.md"),
        ".dev/mistake-register.md".into(),
        ".dev/pipeline/usage-gap".into(),
        format!(".dev/pipeline/{scope}"),
    ] {
        archive_files(&root.join(path), &mut files, false)?;
    }
    archive_files(&common.join("gal/usage/v1").join(run), &mut files, true)?;
    for entry in selected {
        archive_files(
            &common.join("gal/evidence/v1").join(entry),
            &mut files,
            true,
        )?;
    }
    Ok(files)
}

fn archive_source_files(
    common: &Path,
    root: &Path,
    run: &str,
    scope: &str,
    selected: &BTreeSet<String>,
) -> Result<BTreeSet<PathBuf>, CaptureError> {
    let mut files = archive_primary_files(common, root, run, scope, selected)?;
    let mut inspected = BTreeSet::new();
    loop {
        let pending: Vec<_> = files.difference(&inspected).cloned().collect();
        if pending.is_empty() {
            break;
        }
        for path in pending {
            inspected.insert(path.clone());
            let store_manifest = path.to_string_lossy().ends_with(".manifest.json")
                && (path.starts_with(common.join("gal/evidence/v1"))
                    || path.starts_with(common.join("gal/usage/v1")));
            if let Ok(bytes) = fs::read(&path) {
                if let Ok(mut value) = serde_json::from_slice::<Value>(&bytes) {
                    if store_manifest {
                        let id = value["record_id"]
                            .as_str()
                            .ok_or_else(|| archive_error("malformed source manifest"))?;
                        correction_store_id(&json!(id))?;
                        let record_path = path.with_file_name(format!("{id}.record.json"));
                        check_archive_path(&record_path)?;
                        if value["digest"].as_str()
                            != Some(digest_bytes(&fs::read(&record_path)?).as_str())
                        {
                            return Err(archive_error("source manifest digest mismatch"));
                        }
                    }
                    if let Some(cleaned) = cleaned_host_artifacts(&path, &bytes, root, run, scope)?
                    {
                        let state = std::mem::take(&mut value["state"]);
                        registered_archive_paths(&state, root, &mut files, Some(&cleaned))?;
                    }
                    registered_archive_paths(&value, root, &mut files, None)?;
                } else if store_manifest {
                    return Err(archive_error("malformed source manifest"));
                }
            }
        }
    }
    if files
        .iter()
        .any(|path| path.starts_with(archive_root(common)))
    {
        return Err(archive_error("archive proof cannot include itself"));
    }
    Ok(files)
}

fn archive_redact(text: &str) -> String {
    let mut text = text.to_owned();
    let mut secrets: Vec<_> = std::env::vars()
        .filter(|(key, value)| {
            let key = key.to_ascii_uppercase();
            !value.is_empty()
                && (key.ends_with("_TOKEN")
                    || key.ends_with("_SECRET")
                    || key.ends_with("_PASSWORD")
                    || key.ends_with("_API_KEY")
                    || key == "GAL_ARCHIVE_KNOWN_SECRET")
        })
        .map(|(_, value)| value)
        .collect();
    secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
    for secret in secrets {
        text = text.replace(&secret, "<redacted>");
    }
    let headers =
        regex::Regex::new(r"(?im)(authorization\s*[:=]\s*|proxy-authorization\s*[:=]\s*)[^\r\n]+")
            .expect("constant redaction regex");
    text = headers.replace_all(&text, "${1}<redacted>").into_owned();
    let credentials = regex::Regex::new(r#"(?i)((?:[a-z0-9_]*[_-])?(?:api[_-]?key|token|secret|password|passwd)[\"']?\s*[:=]\s*[\"']?)[^\s\"',;}]+"#).expect("constant redaction regex");
    text = credentials
        .replace_all(&text, "${1}<redacted>")
        .into_owned();
    let flags = regex::Regex::new(
        r#"(?i)(--(?:api[-_]?key|token|secret|password|authorization)(?:=|\s+)[\"']?)[^\s\"']+"#,
    )
    .expect("constant redaction regex");
    text = flags.replace_all(&text, "${1}<redacted>").into_owned();
    let tokens = regex::Regex::new(r"\b(?:sk-[A-Za-z0-9_-]+|gh[pousr]_[A-Za-z0-9_]+)\b")
        .expect("constant redaction regex");
    tokens.replace_all(&text, "<redacted>").into_owned()
}

fn redact_archive_value(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                let key = key.to_ascii_lowercase().replace('-', "_");
                if matches!(
                    key.as_str(),
                    "authorization"
                        | "proxy_authorization"
                        | "token"
                        | "secret"
                        | "password"
                        | "passwd"
                        | "api_key"
                        | "apikey"
                ) || key.ends_with("_api_key")
                    || key.ends_with("_secret")
                    || key.ends_with("_password")
                    || key.ends_with("_token")
                {
                    *value = json!("<redacted>");
                } else {
                    redact_archive_value(value);
                }
            }
        }
        Value::Array(values) => {
            let bytes: Option<Vec<u8>> = values
                .iter()
                .map(|value| value.as_u64().and_then(|value| u8::try_from(value).ok()))
                .collect();
            if let Some(text) = bytes.and_then(|bytes| String::from_utf8(bytes).ok()) {
                let redacted = archive_redact(&text);
                if redacted != text {
                    *values = redacted.bytes().map(|byte| json!(byte)).collect();
                    return;
                }
            }
            let mut secret_argument = false;
            for value in values {
                if secret_argument {
                    *value = json!("<redacted>");
                    secret_argument = false;
                    continue;
                }
                secret_argument = value.as_str().is_some_and(|arg| {
                    matches!(
                        arg.to_ascii_lowercase().as_str(),
                        "--api-key"
                            | "--api_key"
                            | "--token"
                            | "--secret"
                            | "--password"
                            | "--authorization"
                    )
                });
                redact_archive_value(value);
            }
        }
        Value::String(text) => *text = archive_redact(text),
        _ => {}
    }
}

fn read_archive_sources(files: &BTreeSet<PathBuf>) -> Result<Vec<ArchivedSource>, CaptureError> {
    files
        .iter()
        .map(|path| {
            check_archive_path(path)?;
            match fs::read(path) {
                Ok(bytes) => {
                    let text = String::from_utf8(bytes.clone()).map_err(|_| {
                        archive_error(format!("non-text archive source: {}", path.display()))
                    })?;
                    let content = if let Ok(mut value) = serde_json::from_str::<Value>(&text) {
                        redact_archive_value(&mut value);
                        String::from_utf8(archive_json(&value)?)
                            .map_err(|error| archive_error(error.to_string()))?
                    } else {
                        archive_redact(&text)
                    };
                    Ok(ArchivedSource {
                        path: path.clone(),
                        original_sha256: Some(digest_bytes(&bytes)),
                        original_length: bytes.len(),
                        archived_sha256: Some(digest_bytes(content.as_bytes())),
                        content: Some(content),
                    })
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ArchivedSource {
                    path: path.clone(),
                    original_sha256: None,
                    original_length: 0,
                    content: None,
                    archived_sha256: None,
                }),
                Err(error) => Err(error.into()),
            }
        })
        .collect()
}

fn source_set_digest(sources: &[ArchivedSource]) -> Result<String, CaptureError> {
    let identities: Vec<_> = sources
        .iter()
        .map(|source| {
            (
                &source.path,
                &source.original_sha256,
                source.original_length,
            )
        })
        .collect();
    Ok(digest_bytes(&archive_json(&json!(identities))?))
}

fn close_evidence_entry(common: &Path, entry: &str) -> Result<PathBuf, CaptureError> {
    correction_store_id(&json!(entry))?;
    let directory = common.join("gal/evidence/v1").join(entry);
    check_archive_path(&directory)?;
    fs::create_dir_all(&directory)?;
    check_archive_path(&directory.join(".lock"))?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(".lock"))?;
    file.try_lock_exclusive()
        .map_err(|_| archive_error("active evidence writer"))?;
    let marker = directory.join(".closed");
    check_archive_path(&marker)?;
    if !marker.exists() {
        archive_write(&marker, b"closed\n")?;
    }
    Ok(marker)
}

fn archive_writer_lock(directory: &Path) -> Result<Option<fs::File>, CaptureError> {
    check_archive_path(directory)?;
    if !directory.exists() {
        return Ok(None);
    }
    let path = directory.join(".lock");
    check_archive_path(&path)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.try_lock_exclusive()
        .map_err(|_| archive_error("active source writer"))?;
    Ok(Some(file))
}

fn archive_operation_lock(common: &Path) -> Result<fs::File, CaptureError> {
    let path = common.join("gal/evidence/v1/.operation-lock");
    check_archive_path(&path)?;
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| archive_error("missing operation lock parent"))?,
    )?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    file.try_lock_exclusive()
        .map_err(|_| archive_error("active operation writer"))?;
    Ok(file)
}

fn read_archive_proof(common: &Path, id: &str) -> Result<ArchiveProof, CaptureError> {
    correction_digest(&json!(id))?;
    let path = archive_root(common)
        .join("proofs")
        .join(format!("{id}.json"));
    check_archive_path(&path)?;
    let bytes = fs::read(path)?;
    if digest_bytes(&bytes) != id {
        return Err(archive_error("archive proof digest mismatch"));
    }
    let proof = ArchiveProof::from_value(
        serde_json::from_slice(&bytes).map_err(|error| archive_error(error.to_string()))?,
    )?;
    if proof.schema_version != 1 || source_set_digest(&proof.archive_set)? != proof.source_set_id {
        return Err(archive_error("archive manifest mismatch"));
    }
    for source in &proof.archive_set {
        if source
            .content
            .as_ref()
            .map(|content| digest_bytes(content.as_bytes()))
            != source.archived_sha256
        {
            return Err(archive_error("unreadable archive content"));
        }
    }
    Ok(proof)
}

pub(crate) fn archive_sources(request: &ArchiveRequest) -> Result<Value, CaptureError> {
    let (common, root) = archive_context(request)?;
    let target = root.join(
        request
            .target
            .as_ref()
            .ok_or_else(|| archive_error("missing archive target"))?,
    );
    check_archive_path(&target)?;
    let target = target.canonicalize()?;
    if target.parent() != Some(root.join(".dev/plans").as_path()) {
        return Err(archive_error("archive target must be a plan or prompt"));
    }
    let scope = super::dispatch::resolve_scope(&target).map_err(archive_error)?;
    if target.file_name().is_none_or(|name| {
        name != format!("{scope}.md").as_str() && name != format!("{scope}.prompt.md").as_str()
    }) {
        return Err(archive_error(
            "archive target filename does not match its scope",
        ));
    }
    let receipt = root
        .join(".dev/pipeline")
        .join(&scope)
        .join("preflight.receipt.md");
    let _lock = archive_operation_lock(&common)?;
    if resolve_run(&common, &receipt) != RunAssociation::Bound(request.run.clone()) {
        return Err(archive_error("run association mismatch or unknown"));
    }
    if let Some(id) = &request.prior_proof {
        let prior = read_archive_proof(&common, id)?;
        if request.boundary != "pipeline-clean"
            || prior.run_id != request.run
            || prior.boundary != "worktree-remove"
        {
            return Err(archive_error("prior proof boundary or run mismatch"));
        }
        let verified = archive_root(&common)
            .join("verified")
            .join(format!("{id}.json"));
        check_archive_path(&verified)?;
        if !verified.is_file() {
            return Err(archive_error("prior proof has not been verified"));
        }
    }
    let selected = archive_evidence_entries(&common, &root, &request.run, &scope)?;
    archive_writers_stopped(&common, &selected, &root, &scope)?;
    drop(archive_writer_lock(
        &common.join("gal/usage/v1").join(&request.run),
    )?);
    let mut markers = BTreeSet::new();
    for entry in &selected {
        markers.insert(close_evidence_entry(&common, entry)?);
    }
    pipeline::usage::UsageStore::open(&common)?.close_run(&request.run)?;
    markers.insert(
        common
            .join("gal/usage/v1")
            .join(&request.run)
            .join(".closed"),
    );
    let closure = archive_root(&common).join("closures").join(format!(
        "{}.json",
        digest_bytes(&archive_json(&json!([root, request.run, scope]))?)
    ));
    archive_replace(
        &closure,
        &archive_json(
            &json!({"source_worktree":root,"run_id":request.run,"scope":scope,"generation":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos().to_string()}),
        )?,
    )?;
    markers.insert(closure);
    let closure_markers = markers
        .into_iter()
        .map(|path| Ok((path.clone(), digest_bytes(&fs::read(path)?))))
        .collect::<Result<BTreeMap<_, _>, CaptureError>>()?;
    let files = archive_source_files(&common, &root, &request.run, &scope, &selected)?;
    let sources = read_archive_sources(&files)?;
    let source_set_id = source_set_digest(&sources)?;
    let to_delete_set = files
        .iter()
        .filter(|path| match request.boundary.as_str() {
            "worktree-remove" => path.starts_with(&root) && !path.starts_with(&common),
            _ => {
                path.starts_with(root.join(".dev/pipeline").join(&scope))
                    || **path == root.join(".dev/plans").join(format!("{scope}.md"))
                    || **path == root.join(".dev/plans").join(format!("{scope}.prompt.md"))
            }
        })
        .cloned()
        .collect();
    let terminal_operation_id = format!(
        "archive-{}",
        &digest_bytes(&archive_json(&json!([
            source_set_id,
            request.boundary,
            now()
        ]))?)[..24]
    );
    let git_version = std::process::Command::new("git")
        .arg("--version")
        .output()?;
    if !git_version.status.success() {
        return Err(archive_error("Git version unavailable"));
    }
    let proof = ArchiveProof { schema_version: 1, run_id: request.run.clone(), source_worktree: root.clone(), boundary: request.boundary.clone(), plan: target, scope, source_set_id: source_set_id.clone(), terminal_operation_id, prior_proof: request.prior_proof.clone(), redaction_rule_version: "archive-redaction-v1".into(), redaction_scope: "all source contents; credentials, authorization headers, known secret environment values; original SHA-256 retained".into(), closure_markers, archive_set: sources, to_delete_set, collector_versions: json!({"gal":env!("CARGO_PKG_VERSION"),"git":String::from_utf8_lossy(&git_version.stdout).trim()}) };
    let bytes = archive_json(&proof.to_value())?;
    let id = digest_bytes(&bytes);
    let path = archive_root(&common)
        .join("proofs")
        .join(format!("{id}.json"));
    archive_write(&path, &bytes)?;
    read_archive_proof(&common, &id)?;
    if source_set_digest(&read_archive_sources(&archive_source_files(
        &common,
        &root,
        &request.run,
        &proof.scope,
        &selected,
    )?)?)?
        != source_set_id
    {
        return Err(archive_error("source changed during archive"));
    }
    archive_write(
        &archive_root(&common)
            .join("closeout")
            .join(format!("{id}.json")),
        &archive_json(
            &json!({"archive_proof_id":id,"source_set_id":source_set_id,"run_id":request.run,"source_worktree":root,"boundary":request.boundary,"terminal_operation_id":proof.terminal_operation_id,"status":"awaiting-verification"}),
        )?,
    )?;
    Ok(json!({"archive_proof_id":id,"source_set_id":source_set_id}))
}

pub(crate) fn verify_archive(request: &ArchiveRequest) -> Result<Value, CaptureError> {
    let (common, root) = archive_context(request)?;
    let _lock = archive_operation_lock(&common)?;
    let id = request
        .proof
        .as_deref()
        .ok_or_else(|| archive_error("missing archive proof"))?;
    let proof = read_archive_proof(&common, id)?;
    if proof.run_id != request.run
        || proof.source_worktree != root
        || proof.boundary != request.boundary
        || request.source_set.as_deref() != Some(&proof.source_set_id)
    {
        return Err(archive_error(
            "archive run, worktree, boundary, or source set mismatch",
        ));
    }
    let revoked = archive_root(&common)
        .join("revoked")
        .join(format!("{id}.json"));
    check_archive_path(&revoked)?;
    if revoked.exists() {
        return Err(archive_error(
            "archive proof revoked; re-archive before deletion",
        ));
    }
    let closeout_path = archive_root(&common)
        .join("closeout")
        .join(format!("{id}.json"));
    check_archive_path(&closeout_path)?;
    let closeout: Value = serde_json::from_slice(&fs::read(closeout_path)?)
        .map_err(|error| archive_error(error.to_string()))?;
    if closeout["archive_proof_id"] != id || closeout["source_set_id"] != proof.source_set_id {
        return Err(archive_error("archive closeout record mismatch"));
    }
    let validation = (|| -> Result<(), CaptureError> {
        for (path, digest) in &proof.closure_markers {
            check_archive_path(path)?;
            if digest_bytes(&fs::read(path)?) != *digest {
                return Err(archive_error("source reopened: archive closure revoked"));
            }
        }
        let receipt = root
            .join(".dev/pipeline")
            .join(&proof.scope)
            .join("preflight.receipt.md");
        if resolve_run(&common, &receipt) != RunAssociation::Bound(request.run.clone()) {
            return Err(archive_error("run association changed"));
        }
        let selected = archive_evidence_entries(&common, &root, &request.run, &proof.scope)?;
        let mut writer_locks = Vec::new();
        writer_locks.push(archive_writer_lock(
            &common.join("gal/usage/v1").join(&request.run),
        )?);
        for entry in &selected {
            writer_locks.push(archive_writer_lock(
                &common.join("gal/evidence/v1").join(entry),
            )?);
        }
        archive_writers_stopped(&common, &selected, &root, &proof.scope)?;
        let current = read_archive_sources(&archive_source_files(
            &common,
            &root,
            &request.run,
            &proof.scope,
            &selected,
        )?)?;
        if source_set_digest(&current)? != proof.source_set_id {
            return Err(archive_error("archive source set or digest changed"));
        }
        Ok(())
    })();
    if let Err(error) = validation {
        archive_write(
            &revoked,
            &archive_json(
                &json!({"archive_proof_id":id,"reason":archive_redact(&error.to_string())}),
            )?,
        )?;
        return Err(error);
    }
    let output = json!({"archive_proof_id":id,"source_set_id":proof.source_set_id,"terminal_operation_id":proof.terminal_operation_id,"status":"verified","boundary":proof.boundary,"run_id":proof.run_id,"source_worktree":proof.source_worktree});
    let path = archive_root(&common)
        .join("verified")
        .join(format!("{id}.json"));
    if !path.exists() {
        archive_write(&path, &archive_json(&output)?)?;
    }
    Ok(output)
}
pub(crate) struct CleanupOutcome {
    pub operation_id: Option<String>,
    pub result: Result<&'static str, CaptureError>,
}

fn validate_cleanup_archive(
    common: &Path,
    root: &Path,
    scope: &str,
    run: &str,
    id: &str,
    proof: &ArchiveProof,
) -> Result<Vec<Option<fs::File>>, CaptureError> {
    if proof.run_id != run
        || proof.source_worktree != root
        || proof.scope != scope
        || proof.boundary != "pipeline-clean"
    {
        return Err(archive_error(
            "main archive run, worktree, scope, or boundary mismatch",
        ));
    }
    let revoked = archive_root(common)
        .join("revoked")
        .join(format!("{id}.json"));
    check_archive_path(&revoked)?;
    if revoked.exists() {
        return Err(archive_error(
            "archive proof revoked; re-archive before deletion",
        ));
    }
    let verified = archive_root(common)
        .join("verified")
        .join(format!("{id}.json"));
    check_archive_path(&verified)?;
    let verified: Value = serde_json::from_slice(&fs::read(verified)?)
        .map_err(|error| archive_error(error.to_string()))?;
    if verified
        != json!({"archive_proof_id":id,"source_set_id":proof.source_set_id,
        "terminal_operation_id":proof.terminal_operation_id,"status":"verified",
        "boundary":proof.boundary,"run_id":proof.run_id,"source_worktree":proof.source_worktree})
    {
        return Err(archive_error("main archive has no matching verification"));
    }
    for (path, digest) in &proof.closure_markers {
        check_archive_path(path)?;
        if digest_bytes(&fs::read(path)?) != *digest {
            return Err(archive_error("source reopened: archive closure revoked"));
        }
    }
    let receipt = root
        .join(".dev/pipeline")
        .join(scope)
        .join("preflight.receipt.md");
    if resolve_run(common, &receipt) != RunAssociation::Bound(run.to_owned()) {
        return Err(archive_error("run association changed or unknown"));
    }
    let selected = archive_evidence_entries(common, root, run, scope)?;
    let mut writer_locks = vec![archive_writer_lock(&common.join("gal/usage/v1").join(run))?];
    for entry in &selected {
        writer_locks.push(archive_writer_lock(
            &common.join("gal/evidence/v1").join(entry),
        )?);
    }
    archive_writers_stopped(common, &selected, root, scope)?;
    let plans = BTreeSet::from([
        root.join(".dev/plans").join(format!("{scope}.md")),
        root.join(".dev/plans").join(format!("{scope}.prompt.md")),
    ]);
    let runtime = root.join(".dev/pipeline").join(scope);
    let expected: BTreeSet<_> = proof
        .archive_set
        .iter()
        .map(|source| source.path.clone())
        .collect();
    let mut current = archive_primary_files(common, root, run, scope, &selected)?;
    current.extend(expected.iter().cloned());
    if current != expected {
        return Err(archive_error("unarchived files in cleanup source set"));
    }
    if !plans.contains(&proof.plan)
        || !proof
            .archive_set
            .iter()
            .any(|source| source.path == proof.plan && source.content.is_some())
        || proof.to_delete_set
            != expected
                .iter()
                .filter(|path| path.starts_with(&runtime) || plans.contains(*path))
                .cloned()
                .collect()
    {
        return Err(archive_error(
            "deleted plan or cleanup set is not fully archived",
        ));
    }
    for source in &proof.archive_set {
        check_archive_path(&source.path)?;
        let bytes = match fs::read(&source.path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if bytes.is_none() && plans.contains(&source.path) {
            continue;
        }
        if bytes.as_ref().map(|bytes| digest_bytes(bytes)) != source.original_sha256
            || bytes.as_ref().map_or(0, Vec::len) != source.original_length
        {
            return Err(archive_error("archive source set or digest changed"));
        }
    }
    Ok(writer_locks)
}

pub(crate) fn consume_cleanup_archive<F>(
    common: &Path,
    root: &Path,
    scope: &str,
    run: &str,
    id: &str,
    cleanup: F,
) -> CleanupOutcome
where
    F: FnOnce() -> Result<&'static str, String>,
{
    let mut operation_id = None;
    let result = (|| -> Result<&'static str, CaptureError> {
        let common = common.canonicalize()?;
        let root = root.canonicalize()?;
        let operation = format!(
            "cleanup-{}",
            &digest_bytes(&archive_json(&json!([
                root,
                scope,
                run,
                id,
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
                    .to_string()
            ]))?)[..24]
        );
        let path = archive_root(&common)
            .join("operations")
            .join(format!("{operation}.json"));
        let mut record = json!({"schema_version":1,"operation_id":operation,
            "terminal_operation_id":operation,"source_kind":"operation-begin","phase":"closeout",
            "archive_proof_id":id,"run_id":run,"source_worktree":root,"scope":scope,
            "boundary":"pipeline-clean","status":"started"});
        archive_write(&path, &archive_json(&record)?)?;
        operation_id = Some(operation.clone());
        let result = (|| -> Result<&'static str, CaptureError> {
            let _lock = archive_operation_lock(&common)?;
            let mut closeout = None;
            let result = (|| -> Result<&'static str, CaptureError> {
                let proof = read_archive_proof(&common, id)?;
                if proof.run_id == run
                    && proof.source_worktree == root
                    && proof.scope == scope
                    && proof.boundary == "pipeline-clean"
                {
                    let closeout_path = archive_root(&common)
                        .join("closeout")
                        .join(format!("{id}.json"));
                    check_archive_path(&closeout_path)?;
                    let mut value: Value = serde_json::from_slice(&fs::read(&closeout_path)?)
                        .map_err(|error| archive_error(error.to_string()))?;
                    if value["archive_proof_id"] != id
                        || value["source_set_id"] != proof.source_set_id
                    {
                        return Err(archive_error("archive closeout record mismatch"));
                    }
                    value["terminal_operation_id"] = json!(operation);
                    value["status"] = json!("cleanup-started");
                    archive_replace(&closeout_path, &archive_json(&value)?)?;
                    closeout = Some((closeout_path, value));
                }
                let _writer_locks = match validate_cleanup_archive(
                    &common, &root, scope, run, id, &proof,
                ) {
                    Ok(locks) => locks,
                    Err(error) => {
                        if closeout.is_some() {
                            let revoked = archive_root(&common)
                                .join("revoked")
                                .join(format!("{id}.json"));
                            check_archive_path(&revoked)?;
                            if !revoked.exists() {
                                archive_write(
                                    &revoked,
                                    &archive_json(
                                        &json!({"archive_proof_id":id,"reason":archive_redact(&error.to_string())}),
                                    )?,
                                )?;
                            }
                        }
                        return Err(error);
                    }
                };
                cleanup().map_err(archive_error)
            })();
            record["source_kind"] = json!("operation-end");
            record["status"] = json!(if result.is_ok() {
                "completed"
            } else {
                "refused"
            });
            record["cleanup_result"] = json!(match &result {
                Ok(value) => *value,
                Err(_) => "refused",
            });
            record["reason"] = result
                .as_ref()
                .err()
                .map(|error| json!(archive_redact(&error.to_string())))
                .unwrap_or(Value::Null);
            archive_replace(&path, &archive_json(&record)?)?;
            if let Some((path, mut value)) = closeout.take() {
                value["terminal_operation_id"] = json!(operation);
                value["status"] = record["status"].clone();
                value["cleanup_result"] = record["cleanup_result"].clone();
                value["reason"] = record["reason"].clone();
                archive_replace(&path, &archive_json(&value)?)?;
            }
            result
        })();
        if let Some(error) = result
            .as_ref()
            .err()
            .filter(|_| record["status"] == "started")
        {
            record["source_kind"] = json!("operation-end");
            record["status"] = json!("refused");
            record["cleanup_result"] = json!("refused");
            record["reason"] = json!(archive_redact(&error.to_string()));
            archive_replace(&path, &archive_json(&record)?)?;
        }
        result
    })();
    CleanupOutcome {
        operation_id,
        result,
    }
}

pub(crate) fn end(
    common_dir: &Path,
    operation_id: &str,
    status: &str,
    plan: Option<&Path>,
    reason: Option<&str>,
) -> Result<(), CaptureError> {
    if !matches!(status, "completed" | "stopped")
        || (status == "completed" && reason.is_some())
        || (status == "stopped" && !reason.is_some_and(super::usage::is_stop_reason))
    {
        return Err(CaptureError("invalid status or reason".into()));
    }
    let entry = entry_id(operation_id)?;
    let store = EvidenceStore::open(common_dir)?;
    let _lock = lock(common_dir)?;
    let begin = store.read(&entry, operation_id)?;
    let (plan_path, plan_digest) = match plan {
        Some(path) => (
            Some(path.to_string_lossy().into_owned()),
            Some(digest_bytes(&fs::read(path)?)),
        ),
        None => (None, None),
    };
    let content =
        json!({"status":status,"reason":reason,"plan":plan_path,"plan_sha256":plan_digest});
    let id = format!("{operation_id}-end");
    match store.read(&entry, &id) {
        Ok(existing) if existing.content == content => return Ok(()),
        Ok(_) => {
            return Err(CaptureError(
                "operation already ended with different content".into(),
            ));
        }
        Err(StoreError::NotFound(_)) => {}
        Err(e) => return Err(e.into()),
    }
    store.publish(&event(
        &entry,
        operation_id,
        &id,
        &begin.phase,
        "operation-end",
        content,
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalize_archive_guard_distinguishes_plans_in_the_same_worktree() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap();
        let closures = archive_root(&root).join("closures");
        fs::create_dir_all(&closures).unwrap();
        let marker = closures.join("archived.json");
        fs::write(
            &marker,
            json!({"source_worktree": root, "scope": "previous-plan"}).to_string(),
        )
        .unwrap();

        assert!(refuse_closed_worktree(&root, &root, Some("next-plan")).is_ok());
        assert!(refuse_closed_worktree(&root, &root, Some("previous-plan")).is_err());
        assert!(refuse_closed_worktree(&root, &root, None).is_err());

        fs::write(&marker, json!({"source_worktree": root}).to_string()).unwrap();
        assert!(refuse_closed_worktree(&root, &root, Some("next-plan")).is_err());
    }

    #[test]
    fn run_association_recovers_and_rejects_conflicts() {
        let root = tempfile::tempdir().unwrap();
        let scope = root.path().join("scope");
        fs::create_dir_all(&scope).unwrap();
        let receipt = scope.join("preflight.receipt.md");
        fs::write(&receipt, "overall: fail\n").unwrap();
        assert_eq!(resolve_run(root.path(), &receipt), RunAssociation::None);
        associate_run(root.path(), &receipt, "run-one", "op-one").unwrap();
        assert_eq!(
            resolve_run(root.path(), &receipt),
            RunAssociation::Bound("run-one".into())
        );
        fs::write(&receipt, "run_id: run-two\n").unwrap();
        assert_eq!(resolve_run(root.path(), &receipt), RunAssociation::Unknown);
        fs::write(&receipt, "run_id: run-one\n").unwrap();
        fs::write(
            scope.join("coordinator.json"),
            r#"{"accounting_run":{"run_id":"run-one"}}"#,
        )
        .unwrap();
        assert_eq!(
            resolve_run(root.path(), &receipt),
            RunAssociation::Bound("run-one".into())
        );
        fs::write(
            scope.join("coordinator.json"),
            r#"{"accounting_run":{"run_id":"run-two"}}"#,
        )
        .unwrap();
        assert_eq!(resolve_run(root.path(), &receipt), RunAssociation::Unknown);
    }

    #[test]
    fn receipt_only_run_is_recovered_and_coordinator_only_is_unknown() {
        let root = tempfile::tempdir().unwrap();
        let scope = root.path().join("scope");
        fs::create_dir_all(&scope).unwrap();
        let receipt = scope.join("preflight.receipt.md");
        fs::write(&receipt, "run_id: run-one\n").unwrap();
        assert_eq!(
            resolve_run(root.path(), &receipt),
            RunAssociation::Bound("run-one".into())
        );
        fs::write(&receipt, "overall: fail\n").unwrap();
        fs::write(
            scope.join("coordinator.json"),
            r#"{"accounting_run":{"run_id":"run-one"}}"#,
        )
        .unwrap();
        assert_eq!(resolve_run(root.path(), &receipt), RunAssociation::Unknown);
    }

    #[test]
    fn receipt_freshness_preserves_unknown_for_unparseable_fields() {
        for (text, actual, reason) in [
            ("", json!({}), "receipt-has-no-expiration"),
            ("expires_at: invalid\n", json!({}), "unparseable-expiration"),
            (
                "expires_at: 1\nexpires_at: 2\n",
                json!({}),
                "duplicate-receipt-field",
            ),
            (
                "expires_at: 1\n",
                Value::Null,
                "receipt-envelope-unavailable",
            ),
        ] {
            let facts = receipt_freshness(text, &Value::Null, &actual);
            assert_eq!(facts["expiration"]["status"], "unknown");
            assert_eq!(facts["expiration"]["reason"], reason);
        }
        let facts = receipt_freshness(
            "overall: pass\noverall: fail\nprompt_sha256: a\nprompt_sha256: b\n",
            &Value::Null,
            &json!({}),
        );
        for condition in ["prompt_sha256", "receipt_verdict"] {
            assert_eq!(facts[condition]["status"], "unknown");
            assert_eq!(facts[condition]["reason"], "duplicate-receipt-field");
        }
        assert_eq!(
            facts["prompt_sha256"]["observed_fields"]["prompt_sha256"],
            json!(["a", "b"])
        );
    }
}
