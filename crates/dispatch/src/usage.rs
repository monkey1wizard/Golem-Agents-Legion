//! Normalized usage observations.
//!
//! An observation is one usage record reported by a provider or a host,
//! mapped to a shape shared by every executor. Each counter carries its own
//! availability, so an unknown value never reads as zero. Counters also carry
//! their semantics, which mark a value as a delta or a cumulative snapshot.
//! They carry their descendant scope and their subset relation too, so a
//! subset is never added twice.
//!
//! The schema does not depend on pipeline storage. It only validates what it
//! deserializes, and a caller persists the serialized form.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Schema version written by this build. A record without a version is legacy.
pub const USAGE_SCHEMA_VERSION: u32 = 1;

/// Largest counter value. It stays within the range JSON integers hold exactly.
pub const MAX_TOKEN_COUNT: u64 = i64::MAX as u64;

const NOT_RECORDED: &str = "not recorded";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UsageError {
    #[error("counter {value} exceeds the supported maximum")]
    CounterOutOfRange { value: u64 },
    #[error("currency code `{0}` is not three ASCII uppercase letters")]
    InvalidCurrencyCode(String),
    #[error("credit unit name must not be empty")]
    EmptyCreditUnit,
    #[error("observation window ends at {end} before it starts at {start}")]
    InvalidWindow { start: u64, end: u64 },
    #[error("schema version {found} is newer than supported version {supported}")]
    UnsupportedSchemaVersion { found: u32, supported: u32 },
    #[error("money addition needs the same unit and kind on both sides")]
    MoneyMismatch,
    #[error("money addition overflows the decimal range")]
    MoneyOverflow,
}

/// State of one reported value. `Reported` with a zero value is a known zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Availability<T> {
    Reported { value: T },
    Partial { value: T, reason: String },
    Unsupported { reason: String },
    Unavailable { reason: String },
    Malformed { reason: String },
}

impl<T> Default for Availability<T> {
    fn default() -> Self {
        Self::Unavailable {
            reason: NOT_RECORDED.to_owned(),
        }
    }
}

/// Non-negative token count. Construction rejects values above [`MAX_TOKEN_COUNT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct TokenCount(u64);

impl TokenCount {
    pub fn new(value: u64) -> Result<Self, UsageError> {
        if value > MAX_TOKEN_COUNT {
            return Err(UsageError::CounterOutOfRange { value });
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }

    /// Adds two counts. Both operands are at most `MAX_TOKEN_COUNT`, so the sum
    /// cannot wrap a `u64`. The range check rejects sums above the maximum.
    pub fn try_add(self, other: Self) -> Result<Self, UsageError> {
        Self::new(self.0 + other.0)
    }
}

impl TryFrom<u64> for TokenCount {
    type Error = UsageError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<TokenCount> for u64 {
    fn from(count: TokenCount) -> Self {
        count.0
    }
}

/// Whether an observation is an increment or a running total.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterSemantics {
    Delta,
    CumulativeSnapshot,
    #[default]
    Unknown,
}

/// Whether a value includes the usage of child agents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DescendantScope {
    Own,
    IncludesDescendants,
    #[default]
    Unknown,
}

/// Usage category of a token counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageCategory {
    Input,
    Output,
    CachedInput,
    CacheWrite,
    Reasoning,
    Other,
}

/// How a counter relates to the other counters in the same record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CounterRelation {
    Exclusive,
    /// The counter is already included in `category`, so totals must skip it.
    SubsetOf {
        category: UsageCategory,
    },
    #[default]
    Unknown,
}

/// Half-open time interval in Unix milliseconds. `start` is never after `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ObservationWindowFields", into = "ObservationWindowFields")]
pub struct ObservationWindow {
    start_unix_ms: u64,
    end_unix_ms: u64,
}

impl ObservationWindow {
    pub fn new(start_unix_ms: u64, end_unix_ms: u64) -> Result<Self, UsageError> {
        if end_unix_ms < start_unix_ms {
            return Err(UsageError::InvalidWindow {
                start: start_unix_ms,
                end: end_unix_ms,
            });
        }
        Ok(Self {
            start_unix_ms,
            end_unix_ms,
        })
    }

    pub fn start_unix_ms(&self) -> u64 {
        self.start_unix_ms
    }

    pub fn end_unix_ms(&self) -> u64 {
        self.end_unix_ms
    }
}

#[derive(Serialize, Deserialize)]
struct ObservationWindowFields {
    start_unix_ms: u64,
    end_unix_ms: u64,
}

impl TryFrom<ObservationWindowFields> for ObservationWindow {
    type Error = UsageError;

    fn try_from(fields: ObservationWindowFields) -> Result<Self, Self::Error> {
        Self::new(fields.start_unix_ms, fields.end_unix_ms)
    }
}

impl From<ObservationWindow> for ObservationWindowFields {
    fn from(window: ObservationWindow) -> Self {
        Self {
            start_unix_ms: window.start_unix_ms,
            end_unix_ms: window.end_unix_ms,
        }
    }
}

/// One measured value with the scope needed to combine it safely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Metric<T> {
    pub value: Availability<T>,
    pub semantics: CounterSemantics,
    pub descendants: DescendantScope,
    pub window: Option<ObservationWindow>,
}

impl<T> Default for Metric<T> {
    fn default() -> Self {
        Self {
            value: Availability::default(),
            semantics: CounterSemantics::default(),
            descendants: DescendantScope::default(),
            window: None,
        }
    }
}

/// A token counter with its relation to the other counters in the record.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TokenMetric {
    pub count: Metric<TokenCount>,
    pub relation: CounterRelation,
}

/// Token usage categories of one observation. Each field defaults to unavailable.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TokenUsage {
    pub input: TokenMetric,
    pub output: TokenMetric,
    pub cached_input: TokenMetric,
    pub cache_write: TokenMetric,
    pub reasoning: TokenMetric,
    pub other: TokenMetric,
}

/// What a monetary amount represents. Only `ConfirmedCharge` is a billed amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoneyKind {
    ProviderReported,
    Estimated,
    ConfirmedCharge,
}

/// ISO 4217 style code: exactly three ASCII uppercase letters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CurrencyCode(String);

impl CurrencyCode {
    pub fn new(code: &str) -> Result<Self, UsageError> {
        if code.len() == 3 && code.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Ok(Self(code.to_owned()));
        }
        Err(UsageError::InvalidCurrencyCode(code.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CurrencyCode {
    type Error = UsageError;

    fn try_from(code: String) -> Result<Self, Self::Error> {
        Self::new(&code)
    }
}

impl From<CurrencyCode> for String {
    fn from(code: CurrencyCode) -> Self {
        code.0
    }
}

/// Provider credit or plan unit. Credits are never converted to currency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CreditUnit(String);

impl CreditUnit {
    pub fn new(name: &str) -> Result<Self, UsageError> {
        if name.is_empty() {
            return Err(UsageError::EmptyCreditUnit);
        }
        Ok(Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CreditUnit {
    type Error = UsageError;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::new(&name)
    }
}

impl From<CreditUnit> for String {
    fn from(unit: CreditUnit) -> Self {
        unit.0
    }
}

/// Unit of a monetary amount. Currency and credits never mix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoneyUnit {
    Currency(CurrencyCode),
    Credit(CreditUnit),
}

/// Exact decimal amount with its unit and kind. Arithmetic never uses floats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoneyAmount {
    #[serde(with = "rust_decimal::serde::str")]
    pub amount: Decimal,
    pub unit: MoneyUnit,
    pub kind: MoneyKind,
}

impl MoneyAmount {
    pub fn try_add(&self, other: &Self) -> Result<Self, UsageError> {
        if self.unit != other.unit || self.kind != other.kind {
            return Err(UsageError::MoneyMismatch);
        }
        let amount = self
            .amount
            .checked_add(other.amount)
            .ok_or(UsageError::MoneyOverflow)?;
        Ok(Self {
            amount,
            unit: self.unit.clone(),
            kind: self.kind,
        })
    }
}

/// Identity of the work that produced an observation. `None` means not captured.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservationIdentity {
    pub plan_id: Option<String>,
    pub run_id: Option<String>,
    pub task_id: Option<String>,
    pub phase: Option<String>,
    pub attempt_id: Option<String>,
    pub executor: Option<String>,
    pub session_id: Option<String>,
    pub parent_session_id: Option<String>,
}

/// Position of an observation in its source, used to skip repeats on resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCursor {
    pub epoch: String,
    pub position: String,
}

/// Where an observation came from. Every field is optional for legacy data.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservationSource {
    pub collector: Option<String>,
    pub source_version: Option<String>,
    pub cursor: Option<SourceCursor>,
}

/// Models in use. The configured model stays when the observed model is unknown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelRecord {
    pub observed: Option<String>,
    pub configured: Option<String>,
}

/// One normalized usage observation.
///
/// Records without `schema_version` are legacy records. They deserialize with
/// every counter unavailable, so a legacy record never reads as zero usage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UsageObservation {
    pub schema_version: u32,
    pub identity: ObservationIdentity,
    pub source: ObservationSource,
    pub models: ModelRecord,
    pub tokens: TokenUsage,
    pub cost: Metric<MoneyAmount>,
}

impl UsageObservation {
    /// Creates an empty observation at the current schema version.
    pub fn new() -> Self {
        Self {
            schema_version: USAGE_SCHEMA_VERSION,
            ..Self::default()
        }
    }

    /// Rejects records written by a newer schema than this build reads.
    pub fn validate(&self) -> Result<(), UsageError> {
        if self.schema_version > USAGE_SCHEMA_VERSION {
            return Err(UsageError::UnsupportedSchemaVersion {
                found: self.schema_version,
                supported: USAGE_SCHEMA_VERSION,
            });
        }
        Ok(())
    }
}
