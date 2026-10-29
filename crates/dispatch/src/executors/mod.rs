//! Executor adapters — one per supported headless CLI tool.
//!
//! Each adapter knows:
//! - How to build the CLI invocation (args + spec delivery method)
//! - How to extract the provider-native session/job id from stdout
//!
//! Invocation flags are locked by the Phase 0 spike results:
//! | Tool     | Flags                                            | Session id field               |
//! |----------|--------------------------------------------------|-------------------------------|
//! | claude   | `-p --model <m> --output-format json --dangerously-skip-permissions` | `session_id` (JSON root)      |
//! | codex    | `exec --json --dangerously-bypass-approvals-and-sandbox -m <m>` | `thread_id` in `thread.started` event |
//! | opencode | `run --format json --dangerously-skip-permissions -m <provider/model>` | `sessionID` in stream events |
//! | copilot  | `-C <workdir> --model <m> --allow-all --output-format json -p <spec>` | `result.sessionId` (JSON root) |
//! | agy      | (stdin spec)                                     | newest `~/.agy/brain/<uuid>/`  |

use std::path::Path;
use std::process::Command;
use std::str::FromStr;

use crate::quota::QuotaObservation;
use crate::usage::{
    Availability, CounterRelation, CounterSemantics, DescendantScope, Metric, ModelRecord,
    MoneyAmount, MoneyKind, MoneyUnit, ObservationIdentity, ObservationSource, TokenCount,
    TokenMetric, TokenUsage, UsageObservation,
};
use rust_decimal::Decimal;
use serde_json::Value;

pub mod agy;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod opencode;

/// How the task spec is delivered to the executor.
pub enum SpecDelivery {
    /// Spec is written to the process stdin.
    Stdin,
    /// Spec is embedded as the value of a named CLI flag (e.g. copilot `-p`).
    CliFlag(String),
}

/// The resolved invocation produced by an adapter.
pub struct AdapterInvocation {
    /// CLI arguments to pass to the executor.
    pub args: Vec<String>,
    /// Spec delivery method. Determines whether `spec` goes to stdin or a CLI flag.
    pub delivery: SpecDelivery,
}

/// Provider session evidence parsed from output owned by one executor attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEvidence {
    pub session_id: String,
    pub status: String,
}

/// Errors that make provider session output unsafe to publish as success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvidenceError {
    MissingTerminalObject,
    DuplicateTerminalObject,
    MalformedJson,
    MissingField(&'static str),
    InvalidField(&'static str),
    MismatchedSession,
    Truncated,
    Oversized,
}

/// The single input struct every adapter's `build_invocation` receives.
///
/// Carries the existing model/workdir/spec plus three cross-cutting optional fields:
/// - `effort`: reasoning-intensity hint mapped to each executor's native flag
///   (only adapters that opt in via [`Adapter::supports_effort`] honor it).
/// - `mcp_disable_servers`: MCP server names an adapter should disable (currently
///   only Copilot consumes this via `--disable-mcp-server`).
/// - `timeout_secs`: the dispatch timeout budget, for executors whose own CLI applies
///   a shorter internal default (currently only agy, via `--print-timeout`).
///
/// Fields are borrowed — the struct is constructed fresh at the dispatch call site
/// and lives only for the `build_invocation` call, so no ownership transfer is needed.
/// When `effort` is `None`, `mcp_disable_servers` is empty, and `timeout_secs` is
/// `None`, every adapter produces byte-identical argv to the pre-struct behavior.
pub struct AdapterInvocationInput<'a> {
    /// Model identifier passed to the executor. Empty string means "omit the model flag".
    pub model: &'a str,
    /// Working directory of the dispatched process.
    pub workdir: &'a Path,
    /// Task spec content (delivered via stdin or a CLI flag per [`SpecDelivery`]).
    pub spec: &'a str,
    /// Optional reasoning-intensity hint. Adapters that return `true` from
    /// [`Adapter::supports_effort`] map this to their native flag; others ignore it
    /// (the dispatch layer rejects an `effort` set on an unsupporting executor before spawn).
    pub effort: Option<&'a str>,
    /// MCP server names to disable for this invocation. Empty for every executor
    /// except Copilot, which appends one `--disable-mcp-server <name>` per entry.
    pub mcp_disable_servers: &'a [String],
    /// The effective dispatch timeout in seconds, when the caller has one. An executor
    /// whose CLI enforces its own shorter internal wait must raise that wait to this
    /// budget, otherwise the executor kills itself long before the dispatch timeout is
    /// reached. Only agy consumes this today (`--print-timeout`, whose own default is
    /// `5m0s`). `None` means the caller supplied no budget, and every adapter then omits
    /// the flag and keeps its baseline argv.
    pub timeout_secs: Option<u64>,
}

/// Adapter trait implemented by each tool's module.
pub trait Adapter: Send + Sync {
    fn executor_name(&self) -> &str;

    /// Build the invocation from the shared [`AdapterInvocationInput`].
    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation;

    /// Whether this executor can honor a route-entry `effort` value.
    ///
    /// Fail-closed: the trait default is `false`, so a newly added adapter that
    /// forgets to opt in can never silently accept-and-drop an `effort` value —
    /// the dispatch layer rejects the route with `unsupported-effort` instead.
    /// Adapters that map `effort` to a native flag override this to `true`.
    fn supports_effort(&self) -> bool {
        false
    }

    /// Extract the provider-native session/job id from the executor's stdout.
    ///
    /// Returns `None` when the id cannot be extracted (e.g. not present in output).
    /// For tools whose session id is stored on the filesystem (agy), this scans
    /// the appropriate directory.
    fn extract_session_id(&self, stdout: &str) -> Option<String>;

    /// Build a v2 invocation. Adapters without v2 output keep their legacy argv.
    fn build_v2_invocation(
        &self,
        input: &AdapterInvocationInput,
        _resumed_session_id: Option<&str>,
    ) -> AdapterInvocation {
        self.build_invocation(input)
    }

    /// Parse attempt-owned output into typed provider session evidence.
    fn parse_session_evidence(
        &self,
        stdout: &str,
        _resumed_session_id: Option<&str>,
    ) -> Result<Option<SessionEvidence>, SessionEvidenceError> {
        Ok(self
            .extract_session_id(stdout)
            .map(|session_id| SessionEvidence {
                session_id,
                status: "unknown".to_string(),
            }))
    }

    /// Extract usage from the executor's native output, one observation per reported scope.
    ///
    /// The default reports the executor as unsupported. An adapter that does not override
    /// it therefore never reports zero or complete usage.
    fn extract_native_usage(&self, _stdout: &str) -> NativeUsage {
        NativeUsage::unsupported("executor reports no native usage record")
    }

    /// Quota observations from native output that this attempt already produced.
    ///
    /// The default reports no source. An adapter that returns an empty list therefore
    /// adds no quota evidence, and an adapter without a source should return a gap.
    fn extract_passive_quota(
        &self,
        _stdout: &str,
        _collected_at_unix_ms: u64,
    ) -> Vec<QuotaObservation> {
        Vec::new()
    }

    /// Quota observations from a read-only control query.
    ///
    /// `launch` already names the executable that the attempt resolved. The query
    /// adds its own protocol arguments and inherits the same environment, so the
    /// probe reads the same account as the attempt. The default has no verified
    /// control query and sends nothing.
    fn query_quota(&self, _launch: Command, _collected_at_unix_ms: u64) -> Vec<QuotaObservation> {
        Vec::new()
    }
}

/// Look up the adapter for a given executor name. Returns `None` for unknown executors.
pub fn get_adapter(executor: &str) -> Option<Box<dyn Adapter>> {
    match executor {
        "claude" => Some(Box::new(claude::ClaudeAdapter)),
        "codex" => Some(Box::new(codex::CodexAdapter)),
        "opencode" => Some(Box::new(opencode::OpenCodeAdapter)),
        "copilot" => Some(Box::new(copilot::CopilotAdapter)),
        "agy" => Some(Box::new(agy::AgyAdapter)),
        _ => None,
    }
}

// ── Native usage extraction ───────────────────────────────────────────────────

/// Largest native output parsed for usage. Larger output yields an explicit oversized status.
pub const MAX_NATIVE_USAGE_BYTES: usize = 16 * 1024 * 1024;

/// Collector label recorded on every observation mapped from native output.
const NATIVE_COLLECTOR: &str = "native-output";

/// Reason stored when no event in a record carried a counter.
const NO_EVENT_REASON: &str = "no event reported this counter";

/// Whether an executor's native output can yield usage that this build maps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageCapability {
    /// The output matches a schema this build maps. Each counter still carries its own state.
    Mapped { source_version: &'static str },
    /// The output carries no usage that this build maps. The reason says why.
    Unsupported { reason: String },
}

/// Usage read from one executor attempt's native output.
///
/// `observations` holds one entry per reported scope, such as the run itself and the run
/// including its child agents. It is empty when the capability is unsupported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeUsage {
    pub capability: UsageCapability,
    pub observations: Vec<UsageObservation>,
}

impl NativeUsage {
    /// The output carries no usage that this build maps.
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            capability: UsageCapability::Unsupported {
                reason: reason.into(),
            },
            observations: Vec::new(),
        }
    }

    /// The output matches a mapped schema. The observations may hold unavailable counters.
    pub(crate) fn mapped(
        source_version: &'static str,
        observations: Vec<UsageObservation>,
    ) -> Self {
        Self {
            capability: UsageCapability::Mapped { source_version },
            observations,
        }
    }

    /// Output above [`MAX_NATIVE_USAGE_BYTES`] is not parsed, so every counter stays unavailable.
    pub(crate) fn oversized(executor: &str, source_version: &'static str) -> Self {
        let reason = format!("native output exceeds {MAX_NATIVE_USAGE_BYTES} bytes");
        let observation = scoped_observation(
            executor,
            source_version,
            uniform_tokens(&Availability::Unavailable {
                reason: reason.clone(),
            }),
            Metric {
                value: Availability::Unavailable { reason },
                ..Metric::default()
            },
            ModelRecord::default(),
        );
        Self::mapped(source_version, vec![observation])
    }
}

/// Whether `stdout` is too large to parse for usage.
pub(crate) fn exceeds_native_bound(stdout: &str) -> bool {
    stdout.len() > MAX_NATIVE_USAGE_BYTES
}

/// Parses the JSON objects in `stdout`, read as one document or as NDJSON lines.
pub(crate) fn json_objects(stdout: &str) -> Vec<Value> {
    if let Ok(document @ Value::Object(_)) = serde_json::from_str::<Value>(stdout.trim()) {
        return vec![document];
    }
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('{'))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

/// Reads one counter from `container`. An absent or null field is unavailable, never zero.
pub(crate) fn count_in(container: Option<&Value>, key: &str) -> Availability<TokenCount> {
    match container.and_then(|object| object.get(key)) {
        None | Some(Value::Null) => Availability::Unavailable {
            reason: format!("{key} is absent"),
        },
        Some(value) => match value.as_u64().map(TokenCount::new) {
            Some(Ok(count)) => Availability::Reported { value: count },
            _ => Availability::Malformed {
                reason: format!("{key} is not a non-negative integer within range"),
            },
        },
    }
}

/// Reads a non-negative JSON number as an exact decimal. Strings and negative values fail.
pub(crate) fn decimal_from_json(value: &Value) -> Result<Decimal, String> {
    let Value::Number(number) = value else {
        return Err("amount is not a JSON number".to_owned());
    };
    let text = number.to_string();
    let amount = Decimal::from_str(&text)
        .or_else(|_| Decimal::from_scientific(&text))
        .map_err(|_| format!("amount {text} is not a decimal"))?;
    if amount.is_sign_negative() {
        return Err(format!("amount {text} is negative"));
    }
    Ok(amount)
}

/// Builds a provider-reported amount from an optional JSON number in `unit`.
pub(crate) fn provider_cost(
    raw: Option<&Value>,
    unit: MoneyUnit,
    label: &str,
) -> Metric<MoneyAmount> {
    let value = match raw {
        None | Some(Value::Null) => Availability::Unavailable {
            reason: format!("{label} is absent"),
        },
        Some(value) => match decimal_from_json(value) {
            Ok(amount) => Availability::Reported {
                value: MoneyAmount {
                    amount,
                    unit,
                    kind: MoneyKind::ProviderReported,
                },
            },
            Err(reason) => Availability::Malformed { reason },
        },
    };
    Metric {
        value,
        ..Metric::default()
    }
}

/// A cost the executor does not report, with the reason it is absent.
pub(crate) fn unsupported_cost(reason: &str) -> Metric<MoneyAmount> {
    Metric {
        value: Availability::Unsupported {
            reason: reason.to_owned(),
        },
        ..Metric::default()
    }
}

/// A token counter the executor's record has no field for.
pub(crate) fn unsupported_token(reason: &str) -> TokenMetric {
    TokenMetric {
        count: Metric {
            value: Availability::Unsupported {
                reason: reason.to_owned(),
            },
            ..Metric::default()
        },
        relation: CounterRelation::Unknown,
    }
}

/// Every token counter set to one state, for records that cannot be read at all.
pub(crate) fn uniform_tokens(state: &Availability<TokenCount>) -> TokenUsage {
    let counter = || TokenMetric {
        count: Metric {
            value: state.clone(),
            ..Metric::default()
        },
        relation: CounterRelation::Unknown,
    };
    TokenUsage {
        input: counter(),
        output: counter(),
        cached_input: counter(),
        cache_write: counter(),
        reasoning: counter(),
        other: counter(),
    }
}

/// Reads one counter from one source field, with its scope and an optional status.
pub(crate) fn single_counter(
    counter: Availability<TokenCount>,
    semantics: CounterSemantics,
    descendants: DescendantScope,
    status: Option<&str>,
) -> Metric<TokenCount> {
    let mut total = CounterTotal::default();
    total.push(counter);
    total.finish(semantics, descendants, status)
}

/// One observation for one scope, with the executor and source version recorded.
pub(crate) fn scoped_observation(
    executor: &str,
    source_version: &str,
    tokens: TokenUsage,
    cost: Metric<MoneyAmount>,
    models: ModelRecord,
) -> UsageObservation {
    UsageObservation {
        identity: ObservationIdentity {
            executor: Some(executor.to_owned()),
            ..ObservationIdentity::default()
        },
        source: ObservationSource {
            collector: Some(NATIVE_COLLECTOR.to_owned()),
            source_version: Some(source_version.to_owned()),
            cursor: None,
        },
        models,
        tokens,
        cost,
        ..UsageObservation::new()
    }
}

/// Sums one counter across the events of one record and keeps every gap visible.
///
/// A missing event, a non-numeric value, or an overflow never adds zero to the sum.
/// Any such gap turns a known sum into a partial value that says how many events
/// were omitted.
#[derive(Default)]
pub(crate) struct CounterTotal {
    sum: Option<TokenCount>,
    reported: usize,
    events: usize,
    gap: Option<Availability<TokenCount>>,
    malformed: Option<String>,
}

impl CounterTotal {
    pub(crate) fn push(&mut self, counter: Availability<TokenCount>) {
        self.events += 1;
        match counter {
            Availability::Reported { value } => {
                self.reported += 1;
                let total = match self.sum {
                    None => Ok(value),
                    Some(sum) => sum.try_add(value),
                };
                match total {
                    Ok(total) => self.sum = Some(total),
                    Err(error) => self.malformed = Some(error.to_string()),
                }
            }
            Availability::Malformed { reason } => self.malformed = Some(reason),
            other => {
                self.gap.get_or_insert(other);
            }
        }
    }

    /// Produces the counter for this record. A status or a gap makes a known sum partial.
    pub(crate) fn finish(
        self,
        semantics: CounterSemantics,
        descendants: DescendantScope,
        status: Option<&str>,
    ) -> Metric<TokenCount> {
        let value = if let Some(reason) = self.malformed {
            Availability::Malformed { reason }
        } else if let Some(sum) = self.sum {
            let mut reasons = Vec::new();
            let omitted = self.events - self.reported;
            if omitted > 0 {
                reasons.push(format!(
                    "{omitted} of {} events omitted this counter",
                    self.events
                ));
            }
            if let Some(status) = status {
                reasons.push(status.to_owned());
            }
            if reasons.is_empty() {
                Availability::Reported { value: sum }
            } else {
                Availability::Partial {
                    value: sum,
                    reason: reasons.join(" and "),
                }
            }
        } else {
            self.gap.unwrap_or(Availability::Unavailable {
                reason: NO_EVENT_REASON.to_owned(),
            })
        };
        Metric {
            value,
            semantics,
            descendants,
            window: None,
        }
    }
}

// ── Shared JSON extraction helpers ────────────────────────────────────────────

/// Extract a top-level string field from JSON (handles streaming NDJSON lines too).
pub(crate) fn extract_json_field(output: &str, field: &str) -> Option<String> {
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(s) = v.get(field).and_then(|f| f.as_str()) {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Extract a nested field `parent.child` from JSON lines.
pub(crate) fn extract_nested_json_field(output: &str, parent: &str, child: &str) -> Option<String> {
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(s) = v
                .get(parent)
                .and_then(|p| p.get(child))
                .and_then(|f| f.as_str())
            {
                return Some(s.to_string());
            }
        }
    }
    None
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a baseline input: no effort, no MCP disable list, no timeout budget — the
    /// state that must reproduce the pre-struct-migration argv byte-for-byte.
    fn baseline<'a>(
        model: &'a str,
        workdir: &'a Path,
        spec: &'a str,
    ) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model,
            workdir,
            spec,
            effort: None,
            mcp_disable_servers: &[],
            timeout_secs: None,
        }
    }

    /// Every adapter's `build_invocation` argv is byte-identical to the
    /// spike-locked baseline when `effort` is absent and the disable list is empty.
    #[test]
    fn build_invocation_argv_byte_identical_baseline() {
        let wd = Path::new("/wd");
        let inp = baseline("m", wd, "the-spec");

        assert_eq!(
            claude::ClaudeAdapter.build_invocation(&inp).args,
            vec![
                "-p",
                "--output-format",
                "json",
                "--dangerously-skip-permissions",
                "--setting-sources",
                "user",
                "--model",
                "m"
            ]
        );
        // Codex's baseline always carries `-c project_doc_max_bytes=0` (suppresses
        // the repo-level project doc so project instructions do not override task specs);
        // it is not gated on effort.
        assert_eq!(
            codex::CodexAdapter.build_invocation(&inp).args,
            vec![
                "exec",
                "--json",
                "--dangerously-bypass-approvals-and-sandbox",
                "-m",
                "m",
                "-c",
                "project_doc_max_bytes=0",
            ]
        );
        // OpenCode's baseline argv intentionally uses `--auto --agent build`
        // (verified against installed OpenCode 1.17.18); the older
        // `--dangerously-skip-permissions` flag was removed from the CLI. This is a
        // deliberate flag change, not a migration regression.
        assert_eq!(
            opencode::OpenCodeAdapter.build_invocation(&inp).args,
            vec!["run", "--format", "json", "--auto", "--agent", "build", "-m", "m"]
        );
        // Copilot's baseline always carries the adapter-owned context-slimming flags
        // `--no-custom-instructions` + `--disable-builtin-mcps` (a deliberate change so
        // Copilot Free headless does not overflow before writing the receipt); they are
        // not gated on effort/disable-list.
        assert_eq!(
            copilot::CopilotAdapter.build_invocation(&inp).args,
            vec![
                "-C",
                &*wd.to_string_lossy(),
                "--allow-all",
                "--output-format",
                "json",
                "--no-custom-instructions",
                "--disable-builtin-mcps",
                "--model",
                "m",
                "-p",
                "the-spec",
            ]
        );
        assert_eq!(
            agy::AgyAdapter.build_invocation(&inp).args,
            vec![
                "--dangerously-skip-permissions",
                "--add-dir",
                "/wd",
                "--model",
                "m"
            ]
        );
    }

    /// Empty model omits the `--model`/`-m` flag pair (baseline behavior preserved).
    #[test]
    fn build_invocation_empty_model_omits_model_flag() {
        let wd = Path::new("/wd");
        let inp = baseline("", wd, "spec");
        assert_eq!(
            claude::ClaudeAdapter.build_invocation(&inp).args,
            vec![
                "-p",
                "--output-format",
                "json",
                "--dangerously-skip-permissions",
                "--setting-sources",
                "user",
            ]
        );
        assert!(!codex::CodexAdapter
            .build_invocation(&inp)
            .args
            .iter()
            .any(|a| a == "-m"));
    }

    /// Fail-closed capability default: an adapter that does not override
    /// `supports_effort` reports `false`.
    #[test]
    fn supports_effort_defaults_false() {
        struct MinimalAdapter;
        impl Adapter for MinimalAdapter {
            fn executor_name(&self) -> &str {
                "minimal"
            }
            fn build_invocation(&self, _input: &AdapterInvocationInput) -> AdapterInvocation {
                AdapterInvocation {
                    args: vec![],
                    delivery: SpecDelivery::Stdin,
                }
            }
            fn extract_session_id(&self, _stdout: &str) -> Option<String> {
                None
            }
        }

        assert!(!MinimalAdapter.supports_effort());
    }

    /// Native usage defaults to an explicit unsupported capability with no observations.
    #[test]
    fn native_usage_defaults_to_unsupported_without_observations() {
        struct SilentAdapter;
        impl Adapter for SilentAdapter {
            fn executor_name(&self) -> &str {
                "silent"
            }
            fn build_invocation(&self, _input: &AdapterInvocationInput) -> AdapterInvocation {
                AdapterInvocation {
                    args: vec![],
                    delivery: SpecDelivery::Stdin,
                }
            }
            fn extract_session_id(&self, _stdout: &str) -> Option<String> {
                None
            }
        }

        let native = SilentAdapter.extract_native_usage(r#"{"usage":{"input_tokens":5}}"#);
        assert!(native.observations.is_empty());
        assert!(matches!(
            native.capability,
            UsageCapability::Unsupported { .. }
        ));
    }

    /// A sum keeps a known total and marks omitted events instead of counting them as zero.
    #[test]
    fn counter_total_marks_omitted_events_partial() {
        let mut total = CounterTotal::default();
        total.push(Availability::Reported {
            value: TokenCount::new(7).unwrap(),
        });
        total.push(Availability::Unavailable {
            reason: "input_tokens is absent".to_owned(),
        });
        let metric = total.finish(
            CounterSemantics::Delta,
            DescendantScope::Own,
            Some("final turn failed"),
        );
        assert_eq!(
            metric.value,
            Availability::Partial {
                value: TokenCount::new(7).unwrap(),
                reason: "1 of 2 events omitted this counter and final turn failed".to_owned(),
            }
        );
    }

    /// A non-numeric counter is malformed and never becomes zero.
    #[test]
    fn count_in_rejects_malformed_and_keeps_absence_distinct() {
        let object = serde_json::json!({ "input_tokens": "12", "output_tokens": 9 });
        assert!(matches!(
            count_in(Some(&object), "input_tokens"),
            Availability::Malformed { .. }
        ));
        assert_eq!(
            count_in(Some(&object), "output_tokens"),
            Availability::Reported {
                value: TokenCount::new(9).unwrap()
            }
        );
        assert!(matches!(
            count_in(Some(&object), "cached_input_tokens"),
            Availability::Unavailable { .. }
        ));
    }
}
