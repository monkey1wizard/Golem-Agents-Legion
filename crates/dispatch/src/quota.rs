//! Scoped account quota observations.
//!
//! A quota observation describes shared account or provider capacity at one
//! point in time. It never describes the consumption of one task. The schema
//! has no contribution or money conversion, so quota cannot enter task costs.
//!
//! Each value keeps its raw scale and direction. A missing value stays missing,
//! so it never reads as zero or as unlimited. Acquisition and freshness are
//! separate states. A failed refresh can keep older values, and the record
//! names the failure.
//!
//! The schema does not depend on pipeline storage. It only validates what it
//! deserializes, and a caller persists the serialized form.

use crate::dispatch::spawn_contained;
use crate::usage::{Availability, CreditUnit, ObservationSource};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};
use thiserror::Error;

/// Schema version written by this build. A record without a version is legacy.
pub const QUOTA_SCHEMA_VERSION: u32 = 1;

const LEGACY_REASON: &str = "legacy record predates quota collection";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QuotaError {
    #[error("schema version {found} is newer than supported version {supported}")]
    UnsupportedSchemaVersion { found: u32, supported: u32 },
    #[error("{kind} must not be empty")]
    EmptyIdentifier { kind: &'static str },
    #[error("utilization {value} is outside the {scale} range")]
    UtilizationOutOfRange { value: Decimal, scale: &'static str },
    #[error("quota amount {0} must not be negative")]
    NegativeAmount(Decimal),
    #[error("{field} uses a different unit from the other amounts")]
    AmbiguousUnit { field: &'static str },
    #[error("a current observation needs a provider observation time")]
    FreshnessWithoutSourceTime,
    #[error("a verified account identity needs an account reference")]
    VerifiedWithoutAccount,
}

/// Defines a validated, non-empty identifier newtype.
macro_rules! identifier {
    ($name:ident, $kind:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn new(value: &str) -> Result<Self, QuotaError> {
                if value.is_empty() {
                    return Err(QuotaError::EmptyIdentifier { kind: $kind });
                }
                Ok(Self(value.to_owned()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = QuotaError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(&value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

identifier!(AccountRef, "account reference");
identifier!(ProviderId, "provider");
identifier!(BucketId, "bucket");
identifier!(RuntimeVersion, "runtime version");
identifier!(HostId, "host");
identifier!(AuthProfileId, "auth profile");
identifier!(ModelScope, "model scope");
identifier!(WindowName, "window name");
identifier!(EvidenceField, "evidence field");
identifier!(SnapshotId, "snapshot id");
identifier!(ObservationDigest, "observation digest");

/// Native runtime that reported the quota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaRuntime {
    Codex,
    Claude,
    Copilot,
    Antigravity,
    OpenCode,
}

/// Billing mode of the account that owns the quota. Each mode is a separate capacity pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingMode {
    Subscription,
    EnterpriseProject,
    ApiKey,
    Unknown,
}

/// Whether the account behind the observation matches the attempt that asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountIdentityCheck {
    Verified,
    Unverified,
    Mismatch,
}

/// Account, provider, and billing scope of one observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotaIdentity {
    pub runtime: QuotaRuntime,
    pub runtime_version: Option<RuntimeVersion>,
    pub provider: ProviderId,
    pub billing: BillingMode,
    pub account: Option<AccountRef>,
    pub identity_check: AccountIdentityCheck,
    pub host: Option<HostId>,
    pub auth_profile: Option<AuthProfileId>,
    pub model_scope: Option<ModelScope>,
}

/// Raw scale of a utilization value. Each scale has its own range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UtilizationScale {
    /// The value lies in 0..=1.
    Fraction,
    /// The value lies in 0..=100.
    Percent,
}

impl UtilizationScale {
    fn max(self) -> Decimal {
        match self {
            Self::Fraction => Decimal::ONE,
            Self::Percent => Decimal::ONE_HUNDRED,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Fraction => "fraction",
            Self::Percent => "percent",
        }
    }
}

/// What the raw value counts. Used and remaining values are never converted into each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UtilizationDirection {
    Used,
    Remaining,
}

/// Utilization kept in its raw scale and direction. Construction rejects values outside the scale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UtilizationFields", into = "UtilizationFields")]
pub struct Utilization {
    value: Decimal,
    scale: UtilizationScale,
    direction: UtilizationDirection,
}

impl Utilization {
    pub fn new(
        value: Decimal,
        scale: UtilizationScale,
        direction: UtilizationDirection,
    ) -> Result<Self, QuotaError> {
        if value < Decimal::ZERO || value > scale.max() {
            return Err(QuotaError::UtilizationOutOfRange {
                value,
                scale: scale.name(),
            });
        }
        Ok(Self {
            value,
            scale,
            direction,
        })
    }

    pub fn value(&self) -> Decimal {
        self.value
    }

    pub fn scale(&self) -> UtilizationScale {
        self.scale
    }

    pub fn direction(&self) -> UtilizationDirection {
        self.direction
    }
}

#[derive(Serialize, Deserialize)]
struct UtilizationFields {
    #[serde(with = "rust_decimal::serde::str")]
    value: Decimal,
    scale: UtilizationScale,
    direction: UtilizationDirection,
}

impl TryFrom<UtilizationFields> for Utilization {
    type Error = QuotaError;

    fn try_from(fields: UtilizationFields) -> Result<Self, Self::Error> {
        Self::new(fields.value, fields.scale, fields.direction)
    }
}

impl From<Utilization> for UtilizationFields {
    fn from(utilization: Utilization) -> Self {
        Self {
            value: utilization.value,
            scale: utilization.scale,
            direction: utilization.direction,
        }
    }
}

/// Unit of an absolute quota amount. Quota units are separate from money.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaUnit {
    Requests,
    Tokens,
    Credit(CreditUnit),
}

/// Non-negative absolute quota amount with its unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "QuotaAmountFields", into = "QuotaAmountFields")]
pub struct QuotaAmount {
    value: Decimal,
    unit: QuotaUnit,
}

impl QuotaAmount {
    pub fn new(value: Decimal, unit: QuotaUnit) -> Result<Self, QuotaError> {
        if value < Decimal::ZERO {
            return Err(QuotaError::NegativeAmount(value));
        }
        Ok(Self { value, unit })
    }

    pub fn value(&self) -> Decimal {
        self.value
    }

    pub fn unit(&self) -> &QuotaUnit {
        &self.unit
    }
}

#[derive(Serialize, Deserialize)]
struct QuotaAmountFields {
    #[serde(with = "rust_decimal::serde::str")]
    value: Decimal,
    unit: QuotaUnit,
}

impl TryFrom<QuotaAmountFields> for QuotaAmount {
    type Error = QuotaError;

    fn try_from(fields: QuotaAmountFields) -> Result<Self, Self::Error> {
        Self::new(fields.value, fields.unit)
    }
}

impl From<QuotaAmount> for QuotaAmountFields {
    fn from(amount: QuotaAmount) -> Self {
        Self {
            value: amount.value,
            unit: amount.unit,
        }
    }
}

/// Native window that a limit applies to. Provider-specific windows keep their own name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    FiveHour,
    Weekly,
    Monthly,
    RequestRate,
    Named { name: WindowName },
}

/// Window metadata. A missing duration or reset time stays `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotaWindow {
    pub kind: WindowKind,
    pub duration_minutes: Option<u32>,
    pub resets_at_unix_ms: Option<u64>,
}

/// Why a provider says a value is unlimited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlimitedBasis {
    /// The provider sent an explicit unlimited or overage flag.
    ProviderFlag,
    /// The provider documents a sentinel value for unlimited.
    DocumentedSentinel,
}

/// Explicit provider evidence for an unlimited value. Absence never counts as evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlimitedClaim {
    pub basis: UnlimitedBasis,
    pub source_field: EvidenceField,
}

/// Limit of a window. Unlimited requires an `UnlimitedClaim`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuotaLimit {
    Limited { amount: QuotaAmount },
    Unlimited { claim: UnlimitedClaim },
}

/// Whether a value is current, stale, or of unknown age.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Current,
    Stale,
    Unknown,
}

/// How the observation was acquired. It is independent of freshness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Acquisition {
    /// A native event carried the value. Its source time is kept.
    Passive,
    /// A read-only control query returned the value.
    Queried,
    /// A refresh failed. Any values present are older observations.
    Failed { reason: String },
    /// A read-only control query reached its deadline.
    TimedOut { reason: String },
    /// No supported source exists for this scope.
    Unsupported { reason: String },
}

/// Metadata of the read-only query that produced the observation. It is not a billing record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryMetadata {
    pub method: String,
    pub duration_ms: u64,
}

/// One versioned quota observation for one bucket and window.
///
/// Each value is an `Availability`, so an absent window or amount carries a reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotaObservation {
    pub schema_version: u32,
    pub identity: QuotaIdentity,
    pub bucket: BucketId,
    #[serde(default)]
    pub window: Availability<QuotaWindow>,
    #[serde(default)]
    pub utilization: Availability<Utilization>,
    #[serde(default)]
    pub used: Availability<QuotaAmount>,
    #[serde(default)]
    pub remaining: Availability<QuotaAmount>,
    #[serde(default)]
    pub limit: Availability<QuotaLimit>,
    /// Provider observation time in Unix milliseconds. `None` means unknown.
    pub observed_at_unix_ms: Option<u64>,
    pub collected_at_unix_ms: u64,
    pub source: ObservationSource,
    pub query: Option<QueryMetadata>,
    pub acquisition: Acquisition,
    pub freshness: Freshness,
}

impl QuotaObservation {
    /// Rejects records written by a newer schema, and cross-field states that cannot hold.
    pub fn validate(&self) -> Result<(), QuotaError> {
        check_schema_version(self.schema_version)?;
        if self.freshness == Freshness::Current && self.observed_at_unix_ms.is_none() {
            return Err(QuotaError::FreshnessWithoutSourceTime);
        }
        if self.identity.identity_check == AccountIdentityCheck::Verified
            && self.identity.account.is_none()
        {
            return Err(QuotaError::VerifiedWithoutAccount);
        }
        check_same_unit(&[
            ("used", reported(&self.used).map(QuotaAmount::unit)),
            (
                "remaining",
                reported(&self.remaining).map(QuotaAmount::unit),
            ),
            ("limit", limited_amount(&self.limit).map(QuotaAmount::unit)),
        ])
    }
}

/// Reference from a return envelope to one immutable quota snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotaReference {
    pub snapshot_id: SnapshotId,
    pub observation_digest: ObservationDigest,
}

/// Quota reference carried by a return envelope. Absent legacy quota stays unavailable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuotaSnapshot {
    pub schema_version: u32,
    pub reference: Availability<QuotaReference>,
}

impl QuotaSnapshot {
    pub fn referenced(reference: QuotaReference) -> Self {
        Self {
            schema_version: QUOTA_SCHEMA_VERSION,
            reference: Availability::Reported { value: reference },
        }
    }

    pub fn unavailable(reason: &str) -> Self {
        Self {
            schema_version: QUOTA_SCHEMA_VERSION,
            reference: Availability::Unavailable {
                reason: reason.to_owned(),
            },
        }
    }

    /// Rejects references written by a newer schema than this build reads.
    pub fn validate(&self) -> Result<(), QuotaError> {
        check_schema_version(self.schema_version)
    }
}

impl Default for QuotaSnapshot {
    fn default() -> Self {
        Self::unavailable(LEGACY_REASON)
    }
}

fn check_schema_version(found: u32) -> Result<(), QuotaError> {
    if found > QUOTA_SCHEMA_VERSION {
        return Err(QuotaError::UnsupportedSchemaVersion {
            found,
            supported: QUOTA_SCHEMA_VERSION,
        });
    }
    Ok(())
}

fn reported<T>(availability: &Availability<T>) -> Option<&T> {
    match availability {
        Availability::Reported { value } | Availability::Partial { value, .. } => Some(value),
        _ => None,
    }
}

fn limited_amount(limit: &Availability<QuotaLimit>) -> Option<&QuotaAmount> {
    match reported(limit)? {
        QuotaLimit::Limited { amount } => Some(amount),
        QuotaLimit::Unlimited { .. } => None,
    }
}

/// Every amount that has a unit must share one unit. Mixed units are ambiguous.
fn check_same_unit(units: &[(&'static str, Option<&QuotaUnit>)]) -> Result<(), QuotaError> {
    let mut present = units
        .iter()
        .filter_map(|&(field, unit)| unit.map(|unit| (field, unit)));
    let Some((_, first)) = present.next() else {
        return Ok(());
    };
    for (field, unit) in present {
        if unit != first {
            return Err(QuotaError::AmbiguousUnit { field });
        }
    }
    Ok(())
}

// ── Control probe ─────────────────────────────────────────────────────────────

/// Deadline for one probe. The clock starts before the launch, so startup is charged to the probe.
pub const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// Largest number of bytes one probe reads from its server, across all frames.
pub const PROBE_OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;

/// Collector label on every observation this module creates.
pub const COLLECTOR: &str = "gal-dispatch-quota";

const ACCOUNT_BUCKET: &str = "account";

/// Why a probe ended without a usable answer.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProbeError {
    #[error("probe launch failed: {0}")]
    Launch(String),
    #[error("probe deadline of five seconds expired")]
    Timeout,
    #[error("probe output exceeded one MiB")]
    OutputLimit,
    #[error("probe protocol failed: {0}")]
    Protocol(String),
    #[error("{method} returned an error: {message}")]
    Rpc { method: String, message: String },
}

/// One deadline and one output budget shared by the whole probe, including handshakes and retries.
pub struct ProbeBudget {
    started: Instant,
    deadline: Duration,
}

impl ProbeBudget {
    /// Starts the budget. Call before the launch so startup counts against the deadline.
    pub fn start() -> Self {
        Self::with_deadline(PROBE_DEADLINE)
    }

    fn with_deadline(deadline: Duration) -> Self {
        Self {
            started: Instant::now(),
            deadline,
        }
    }

    /// Time left before the deadline. Returns `Timeout` once the deadline has passed.
    pub fn remaining(&self) -> Result<Duration, ProbeError> {
        self.deadline
            .checked_sub(self.started.elapsed())
            .filter(|left| !left.is_zero())
            .ok_or(ProbeError::Timeout)
    }
}

/// One JSON-RPC request, or a notification when `id` is `None`. A notification gets no reply.
pub struct RpcCall {
    pub method: &'static str,
    pub id: Option<u64>,
    pub params: Option<Value>,
}

/// Reads newline-delimited frames from a server on a worker thread, and stops at the output limit.
///
/// The reader is wrapped in `Take`, so one unterminated line cannot grow past the limit either.
fn read_frames<R: Read + Send + 'static>(
    reader: R,
    limit: usize,
) -> Receiver<Result<String, ProbeError>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader.take(limit as u64 + 1));
        let mut total = 0usize;
        let mut line = String::new();
        loop {
            line.clear();
            let frame = match reader.read_line(&mut line) {
                Ok(0) if total > limit => Err(ProbeError::OutputLimit),
                Ok(0) => Err(ProbeError::Protocol("server closed stdout".to_owned())),
                Ok(read) => {
                    total += read;
                    if total > limit {
                        Err(ProbeError::OutputLimit)
                    } else {
                        Ok(line.clone())
                    }
                }
                Err(error) => Err(ProbeError::Protocol(error.to_string())),
            };
            let stop = frame.is_err();
            if tx.send(frame).is_err() || stop {
                break;
            }
        }
    });
    rx
}

/// Waits for the reply with `id`. Notifications and replies to other ids are skipped.
fn await_response(
    frames: &Receiver<Result<String, ProbeError>>,
    budget: &ProbeBudget,
    method: &str,
    id: u64,
) -> Result<Value, ProbeError> {
    loop {
        let wait = budget.remaining()?;
        let frame = match frames.recv_timeout(wait) {
            Ok(frame) => frame?,
            Err(RecvTimeoutError::Timeout) => return Err(ProbeError::Timeout),
            Err(RecvTimeoutError::Disconnected) => {
                return Err(ProbeError::Protocol("frame reader stopped".to_owned()))
            }
        };
        let Ok(message) = serde_json::from_str::<Value>(frame.trim()) else {
            continue;
        };
        if message.get("id") != Some(&json!(id)) {
            continue;
        }
        if let Some(error) = message.get("error") {
            return Err(ProbeError::Rpc {
                method: method.to_owned(),
                message: error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("no message")
                    .to_owned(),
            });
        }
        return Ok(message.get("result").cloned().unwrap_or(Value::Null));
    }
}

fn exchange(
    child: &mut Child,
    calls: &[RpcCall],
    budget: &ProbeBudget,
) -> Result<Vec<Value>, ProbeError> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| ProbeError::Launch("server stdin is unavailable".to_owned()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ProbeError::Launch("server stdout is unavailable".to_owned()))?;
    let frames = read_frames(stdout, PROBE_OUTPUT_LIMIT_BYTES);
    let mut results = Vec::new();
    for call in calls {
        let mut frame = json!({ "method": call.method });
        if let Some(id) = call.id {
            frame["id"] = json!(id);
        }
        if let Some(params) = &call.params {
            frame["params"] = params.clone();
        }
        writeln!(stdin, "{frame}")
            .and_then(|()| stdin.flush())
            .map_err(|error| ProbeError::Protocol(format!("write to server failed: {error}")))?;
        if let Some(id) = call.id {
            results.push(await_response(&frames, budget, call.method, id)?);
        }
    }
    Ok(results)
}

/// Runs one bounded control session and returns each reply in call order.
///
/// The child is contained at spawn. Stdin closes when the exchange ends. The child is then
/// terminated and reaped on success, timeout, and protocol failure alike. Only this
/// GAL-started child is affected.
pub fn run_rpc_session(
    command: &mut Command,
    calls: &[RpcCall],
    budget: &ProbeBudget,
) -> Result<Vec<Value>, ProbeError> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let (mut child, termination) =
        spawn_contained(command).map_err(|error| ProbeError::Launch(error.to_string()))?;
    let outcome = exchange(&mut child, calls, budget);
    termination.terminate(child.id());
    let _ = child.kill();
    let _ = child.wait();
    outcome
}

// ── Observation builders ──────────────────────────────────────────────────────

impl QuotaIdentity {
    /// Scope for a source whose account has not been checked against the attempt.
    pub fn unverified(runtime: QuotaRuntime, provider: ProviderId, billing: BillingMode) -> Self {
        Self {
            runtime,
            runtime_version: None,
            provider,
            billing,
            account: None,
            identity_check: AccountIdentityCheck::Unverified,
            host: None,
            auth_profile: None,
            model_scope: None,
        }
    }
}

/// Maps a reported window length to its kind. An absent length never becomes a known window.
pub fn window_kind_for_minutes(minutes: u32) -> Result<WindowKind, QuotaError> {
    Ok(match minutes {
        300 => WindowKind::FiveHour,
        10_080 => WindowKind::Weekly,
        43_200 => WindowKind::Monthly,
        other => WindowKind::Named {
            name: WindowName::new(&format!("{other}-minute window"))?,
        },
    })
}

/// Converts a provider Unix time in seconds to milliseconds. Overflow yields `None`, never a wrapped value.
pub fn unix_seconds_to_ms(seconds: u64) -> Option<u64> {
    seconds.checked_mul(1_000)
}

/// Observation for one scope that records a missing, failed, or unsupported source.
///
/// Every value carries the same reason. None of them reads as zero or unlimited.
pub fn gap_observation(
    identity: QuotaIdentity,
    acquisition: Acquisition,
    collected_at_unix_ms: u64,
    source_version: &str,
) -> QuotaObservation {
    let reason = match &acquisition {
        Acquisition::Failed { reason }
        | Acquisition::TimedOut { reason }
        | Acquisition::Unsupported { reason } => reason.clone(),
        Acquisition::Passive | Acquisition::Queried => "no quota value was reported".to_owned(),
    };
    QuotaObservation {
        schema_version: QUOTA_SCHEMA_VERSION,
        identity,
        bucket: BucketId::new(ACCOUNT_BUCKET).expect("constant bucket id is not empty"),
        window: missing(&reason),
        utilization: missing(&reason),
        used: missing(&reason),
        remaining: missing(&reason),
        limit: missing(&reason),
        observed_at_unix_ms: None,
        collected_at_unix_ms,
        source: ObservationSource {
            collector: Some(COLLECTOR.to_owned()),
            source_version: Some(source_version.to_owned()),
            cursor: None,
        },
        query: None,
        acquisition,
        freshness: Freshness::Unknown,
    }
}

/// Value that is not available for a stated reason. The type is inferred from the field it fills.
pub fn missing<T>(reason: &str) -> Availability<T> {
    Availability::Unsupported {
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod probe_tests {
    use super::*;

    #[test]
    fn budget_reports_timeout_after_deadline() {
        let budget = ProbeBudget::with_deadline(Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(budget.remaining(), Err(ProbeError::Timeout));
    }

    #[test]
    fn budget_has_time_left_before_deadline() {
        let budget = ProbeBudget::start();
        assert!(budget.remaining().is_ok());
    }

    #[test]
    fn frame_reader_stops_at_output_limit_without_newline() {
        let flood = vec![b'a'; PROBE_OUTPUT_LIMIT_BYTES + 1024];
        let frames = read_frames(std::io::Cursor::new(flood), PROBE_OUTPUT_LIMIT_BYTES);
        assert_eq!(frames.recv().unwrap(), Err(ProbeError::OutputLimit));
    }

    #[test]
    fn frame_reader_returns_complete_lines_within_limit() {
        let frames = read_frames(std::io::Cursor::new(b"{\"id\":0}\n".to_vec()), 1024);
        assert_eq!(frames.recv().unwrap(), Ok("{\"id\":0}\n".to_owned()));
        assert!(matches!(
            frames.recv().unwrap(),
            Err(ProbeError::Protocol(_))
        ));
    }

    #[test]
    fn seconds_convert_to_milliseconds_and_reject_overflow() {
        assert_eq!(unix_seconds_to_ms(1_790_000_000), Some(1_790_000_000_000));
        assert_eq!(unix_seconds_to_ms(u64::MAX), None);
    }

    #[test]
    fn window_length_maps_only_reported_durations() {
        assert_eq!(window_kind_for_minutes(10_080).unwrap(), WindowKind::Weekly);
        assert!(matches!(
            window_kind_for_minutes(90).unwrap(),
            WindowKind::Named { .. }
        ));
    }

    /// A child that never answers. The probe must end at its deadline and reap the child.
    #[test]
    fn silent_server_hits_deadline_and_returns() {
        let mut command = silent_server();
        let budget = ProbeBudget::with_deadline(Duration::from_millis(500));
        let calls = [RpcCall {
            method: "initialize",
            id: Some(0),
            params: None,
        }];
        let started = Instant::now();
        let outcome = run_rpc_session(&mut command, &calls, &budget);
        assert_eq!(outcome, Err(ProbeError::Timeout));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[cfg(windows)]
    fn silent_server() -> Command {
        let mut command = Command::new("cmd");
        command.args(["/C", "ping -n 30 127.0.0.1 >nul"]);
        command
    }

    #[cfg(unix)]
    fn silent_server() -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30"]);
        command
    }
}
