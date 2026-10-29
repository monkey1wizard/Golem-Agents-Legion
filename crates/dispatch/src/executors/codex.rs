//! Codex executor adapter.
//!
//! Invocation:
//!   codex exec --json --dangerously-bypass-approvals-and-sandbox -m <m>
//! Spec: via stdin.
//! Session id: `thread_id` field in `thread.started` NDJSON event.
//! Resumable: `codex exec resume <thread_id>`
//!
//! `--json` activates NDJSON streaming output (required for session id extraction).
//!
//! Security note: `--dangerously-bypass-approvals-and-sandbox` runs the child
//! without Codex's sandbox, the same full-access trust level the other
//! executors get from their permission-bypass flags. The child previously ran
//! under `-s workspace-write`, but the Codex Windows sandbox fails its setup
//! refresh before any command starts (`helper_unknown_error: setup refresh had
//! errors`), so every sandboxed dispatch ended without a receipt. Headless
//! dispatch cannot answer an approval prompt, so approvals are bypassed too.

use super::{
    count_in, decimal_from_json, exceeds_native_bound, json_objects, scoped_observation,
    unsupported_cost, unsupported_token, Adapter, AdapterInvocation, AdapterInvocationInput,
    CounterTotal, NativeUsage, SpecDelivery,
};
use crate::quota::{
    gap_observation, missing, run_rpc_session, unix_seconds_to_ms, window_kind_for_minutes,
    Acquisition, BillingMode, BucketId, Freshness, ProbeBudget, ProbeError, ProviderId,
    QueryMetadata, QuotaIdentity, QuotaObservation, QuotaRuntime, QuotaWindow, RpcCall,
    Utilization, UtilizationDirection, UtilizationScale, WindowKind, COLLECTOR,
    QUOTA_SCHEMA_VERSION,
};
use crate::usage::{
    Availability, CounterRelation, CounterSemantics, DescendantScope, ModelRecord,
    ObservationSource, TokenMetric, TokenUsage, UsageCategory,
};
use serde_json::{json, Value};
use std::process::Command;
use std::time::Instant;

/// Source version of the per-turn usage mapping. Each turn reports its own usage.
const USAGE_SOURCE_VERSION: &str = "codex.exec.turn_usage.v1";

/// Source version of the `account/rateLimits/read` mapping, pinned to the tested CLI 0.163.0.
pub const RATE_LIMITS_SOURCE_VERSION: &str = "codex.app_server.account_rate_limits.v1";

const RATE_LIMITS_METHOD: &str = "account/rateLimits/read";

pub struct CodexAdapter;

impl Adapter for CodexAdapter {
    fn executor_name(&self) -> &str {
        "codex"
    }

    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation {
        let mut args = vec![
            "exec".to_string(),
            "--json".to_string(),
            "--dangerously-bypass-approvals-and-sandbox".to_string(),
        ];
        if !input.model.is_empty() {
            args.push("-m".to_string());
            args.push(input.model.to_string());
        }
        args.push("-c".to_string());
        args.push("project_doc_max_bytes=0".to_string());
        if let Some(effort) = input.effort {
            // Codex has no first-class effort flag; set it via a `-c` config override.
            // The value is embedded as a quoted TOML string. argv tokens are passed
            // directly (no shell), so the literal double quotes reach codex, which
            // parses `model_reasoning_effort="high"` as the TOML string `high`. The
            // dispatch layer validates `effort` against `[A-Za-z0-9._-]+` before spawn,
            // so the value can never contain a quote/whitespace that breaks the override.
            args.push("-c".to_string());
            args.push(format!("model_reasoning_effort=\"{effort}\""));
        }
        AdapterInvocation {
            args,
            delivery: SpecDelivery::Stdin,
        }
    }

    fn supports_effort(&self) -> bool {
        true
    }

    fn extract_session_id(&self, stdout: &str) -> Option<String> {
        // Codex streams NDJSON events. Look for the `thread.started` event and
        // extract `thread_id` from it.
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() || !line.starts_with('{') {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                // Event type is at root `type` or `event` field
                let is_thread_started = v.get("type").and_then(|t| t.as_str())
                    == Some("thread.started")
                    || v.get("event").and_then(|t| t.as_str()) == Some("thread.started");
                if is_thread_started {
                    if let Some(id) = v.get("thread_id").and_then(|f| f.as_str()) {
                        return Some(id.to_string());
                    }
                }
                // Also try direct `thread_id` at root (some codex versions)
                if let Some(id) = v.get("thread_id").and_then(|f| f.as_str()) {
                    return Some(id.to_string());
                }
            }
        }
        None
    }

    /// Sums `turn.completed` usage. Each turn reports its own usage, so the sums are deltas.
    fn extract_native_usage(&self, stdout: &str) -> NativeUsage {
        if exceeds_native_bound(stdout) {
            return NativeUsage::oversized(self.executor_name(), USAGE_SOURCE_VERSION);
        }
        let mut input = CounterTotal::default();
        let mut cached_input = CounterTotal::default();
        let mut output = CounterTotal::default();
        let mut reasoning = CounterTotal::default();
        let mut final_turn_failed = false;
        for event in json_objects(stdout) {
            match event.get("type").and_then(Value::as_str) {
                Some("turn.completed") => {
                    final_turn_failed = false;
                    let usage = event.get("usage");
                    input.push(count_in(usage, "input_tokens"));
                    cached_input.push(count_in(usage, "cached_input_tokens"));
                    output.push(count_in(usage, "output_tokens"));
                    reasoning.push(count_in(usage, "reasoning_output_tokens"));
                }
                Some("turn.failed") => final_turn_failed = true,
                _ => {}
            }
        }
        let status =
            final_turn_failed.then_some("final turn failed, so usage covers completed turns only");
        let tokens = TokenUsage {
            input: TokenMetric {
                count: input.finish(CounterSemantics::Delta, DescendantScope::Own, status),
                relation: CounterRelation::Exclusive,
            },
            cached_input: TokenMetric {
                count: cached_input.finish(CounterSemantics::Delta, DescendantScope::Own, status),
                relation: CounterRelation::SubsetOf {
                    category: UsageCategory::Input,
                },
            },
            output: TokenMetric {
                count: output.finish(CounterSemantics::Delta, DescendantScope::Own, status),
                relation: CounterRelation::Exclusive,
            },
            reasoning: TokenMetric {
                count: reasoning.finish(CounterSemantics::Delta, DescendantScope::Own, status),
                relation: CounterRelation::SubsetOf {
                    category: UsageCategory::Output,
                },
            },
            cache_write: unsupported_token("codex turn usage has no cache write counter"),
            other: unsupported_token("codex turn usage has no other counter"),
        };
        let observation = scoped_observation(
            self.executor_name(),
            USAGE_SOURCE_VERSION,
            tokens,
            unsupported_cost("codex reports no cost"),
            ModelRecord::default(),
        );
        NativeUsage::mapped(USAGE_SOURCE_VERSION, vec![observation])
    }

    fn extract_passive_quota(
        &self,
        _stdout: &str,
        collected_at_unix_ms: u64,
    ) -> Vec<QuotaObservation> {
        vec![gap_observation(
            codex_identity(),
            Acquisition::Unsupported {
                reason: "codex exec output carries no rate-limit record; only the app-server query reports it"
                    .to_owned(),
            },
            collected_at_unix_ms,
            RATE_LIMITS_SOURCE_VERSION,
        )]
    }

    /// Reads `account/rateLimits/read` through the app-server stdio protocol. No thread starts,
    /// so the query never sends an inference turn or a prompt.
    fn query_quota(&self, mut launch: Command, collected_at_unix_ms: u64) -> Vec<QuotaObservation> {
        launch.arg("app-server");
        let budget = ProbeBudget::start();
        let started = Instant::now();
        match run_rpc_session(&mut launch, &control_calls(), &budget) {
            Ok(results) => {
                let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                let rates = results.get(1).cloned().unwrap_or(Value::Null);
                normalize_rate_limits(&rates, codex_identity(), collected_at_unix_ms, duration_ms)
            }
            Err(error) => vec![gap_observation(
                codex_identity(),
                probe_acquisition(&error),
                collected_at_unix_ms,
                RATE_LIMITS_SOURCE_VERSION,
            )],
        }
    }
}

/// Scope of the ChatGPT-backed Codex account. The account is not checked against the attempt,
/// so the identity stays unverified.
fn codex_identity() -> QuotaIdentity {
    QuotaIdentity::unverified(
        QuotaRuntime::Codex,
        ProviderId::new("openai").expect("constant provider id is not empty"),
        BillingMode::Subscription,
    )
}

/// The protocol the query sends: `initialize`, the `initialized` notification, and one read.
/// The probe then closes stdin and terminates the child, which is the shutdown step.
pub fn control_calls() -> Vec<RpcCall> {
    vec![
        RpcCall {
            method: "initialize",
            id: Some(0),
            params: Some(json!({
                "clientInfo": {
                    "name": "gal-dispatch",
                    "title": "GAL dispatch",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            })),
        },
        RpcCall {
            method: "initialized",
            id: None,
            params: Some(json!({})),
        },
        RpcCall {
            method: RATE_LIMITS_METHOD,
            id: Some(1),
            params: None,
        },
    ]
}

fn probe_acquisition(error: &ProbeError) -> Acquisition {
    match error {
        ProbeError::Timeout => Acquisition::TimedOut {
            reason: error.to_string(),
        },
        other => Acquisition::Failed {
            reason: other.to_string(),
        },
    }
}

/// Maps an `account/rateLimits/read` result into one observation per reported window.
///
/// `rateLimitsByLimitId` is preferred. `rateLimits` is the single-bucket view. Only reported
/// windows become observations, and a missing window is never invented. Codex reports percent
/// only, so the absolute amounts stay unavailable.
pub fn normalize_rate_limits(
    result: &Value,
    identity: QuotaIdentity,
    collected_at_unix_ms: u64,
    duration_ms: u64,
) -> Vec<QuotaObservation> {
    let buckets: Vec<&Value> = match result.get("rateLimitsByLimitId") {
        Some(Value::Object(by_id)) => by_id.values().collect(),
        _ => result.get("rateLimits").into_iter().collect(),
    };
    if buckets.is_empty() {
        return vec![gap_observation(
            identity,
            Acquisition::Unsupported {
                reason: "account/rateLimits/read reported no rate-limit bucket".to_owned(),
            },
            collected_at_unix_ms,
            RATE_LIMITS_SOURCE_VERSION,
        )];
    }
    let query = QueryMetadata {
        method: RATE_LIMITS_METHOD.to_owned(),
        duration_ms,
    };
    let mut observations = Vec::new();
    for bucket in buckets {
        let bucket_id = bucket
            .get("limitId")
            .and_then(Value::as_str)
            .and_then(|id| BucketId::new(id).ok());
        let Some(bucket_id) = bucket_id else {
            observations.push(gap_observation(
                identity.clone(),
                Acquisition::Failed {
                    reason: "rate-limit bucket has no usable limitId".to_owned(),
                },
                collected_at_unix_ms,
                RATE_LIMITS_SOURCE_VERSION,
            ));
            continue;
        };
        let windows: Vec<&Value> = ["primary", "secondary"]
            .iter()
            .filter_map(|key| bucket.get(*key).filter(|window| window.is_object()))
            .collect();
        if windows.is_empty() {
            observations.push(bucket_observation(
                &identity,
                &bucket_id,
                Availability::Unavailable {
                    reason: "bucket reports no window".to_owned(),
                },
                Availability::Unavailable {
                    reason: "bucket reports no window".to_owned(),
                },
                collected_at_unix_ms,
                &query,
            ));
        }
        for window in windows {
            let (window_value, utilization) = window_fields(window);
            observations.push(bucket_observation(
                &identity,
                &bucket_id,
                window_value,
                utilization,
                collected_at_unix_ms,
                &query,
            ));
        }
    }
    observations
}

/// Reads one window object. Each field keeps its own state, so one bad field never hides another.
fn window_fields(window: &Value) -> (Availability<QuotaWindow>, Availability<Utilization>) {
    let utilization = match window.get("usedPercent") {
        None | Some(Value::Null) => Availability::Unavailable {
            reason: "usedPercent is absent".to_owned(),
        },
        Some(value) => match decimal_from_json(value).and_then(|amount| {
            Utilization::new(
                amount,
                UtilizationScale::Percent,
                UtilizationDirection::Used,
            )
            .map_err(|error| error.to_string())
        }) {
            Ok(utilization) => Availability::Reported { value: utilization },
            Err(reason) => Availability::Malformed { reason },
        },
    };
    let window_value = match window.get("windowDurationMins").and_then(Value::as_u64) {
        None => Availability::Unavailable {
            reason: "windowDurationMins is absent or not an integer".to_owned(),
        },
        Some(minutes) => match window_span(minutes) {
            Ok((minutes, kind)) => Availability::Reported {
                value: QuotaWindow {
                    kind,
                    duration_minutes: Some(minutes),
                    resets_at_unix_ms: window
                        .get("resetsAt")
                        .and_then(Value::as_u64)
                        .and_then(unix_seconds_to_ms),
                },
            },
            Err(reason) => Availability::Malformed { reason },
        },
    };
    (window_value, utilization)
}

/// Checks a reported window length and maps it to its kind. A length above `u32::MAX` is malformed.
fn window_span(minutes: u64) -> Result<(u32, WindowKind), String> {
    let minutes = u32::try_from(minutes)
        .map_err(|_| format!("windowDurationMins {minutes} is out of range"))?;
    let kind = window_kind_for_minutes(minutes).map_err(|error| error.to_string())?;
    Ok((minutes, kind))
}

fn bucket_observation(
    identity: &QuotaIdentity,
    bucket: &BucketId,
    window: Availability<QuotaWindow>,
    utilization: Availability<Utilization>,
    collected_at_unix_ms: u64,
    query: &QueryMetadata,
) -> QuotaObservation {
    const ABSOLUTE_REASON: &str = "codex reports percent only, not absolute amounts";
    QuotaObservation {
        schema_version: QUOTA_SCHEMA_VERSION,
        identity: identity.clone(),
        bucket: bucket.clone(),
        window,
        utilization,
        used: missing(ABSOLUTE_REASON),
        remaining: missing(ABSOLUTE_REASON),
        limit: missing(ABSOLUTE_REASON),
        observed_at_unix_ms: None,
        collected_at_unix_ms,
        source: ObservationSource {
            collector: Some(COLLECTOR.to_owned()),
            source_version: Some(RATE_LIMITS_SOURCE_VERSION.to_owned()),
            cursor: None,
        },
        query: Some(query.clone()),
        acquisition: Acquisition::Queried,
        freshness: Freshness::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn input<'a>(effort: Option<&'a str>) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model: "test-codex-model",
            workdir: Path::new("/wd"),
            spec: "spec",
            effort,
            mcp_disable_servers: &[],
            timeout_secs: None,
        }
    }

    fn assert_project_doc_suppressed(args: &[String]) {
        let adjacent = args
            .windows(2)
            .any(|w| w[0] == "-c" && w[1] == "project_doc_max_bytes=0");
        assert!(
            adjacent,
            "codex invocation must carry an adjacent project_doc_max_bytes=0 override"
        );
        let count = args
            .windows(2)
            .filter(|w| w[0] == "-c" && w[1] == "project_doc_max_bytes=0")
            .count();
        assert_eq!(count, 1, "project_doc_max_bytes=0 must occur exactly once");
    }

    #[test]
    fn effort_maps_to_quoted_config_override() {
        let inv = CodexAdapter.build_invocation(&input(Some("high")));
        // The `-c` flag and the quoted TOML override are two adjacent argv tokens.
        assert!(
            inv.args
                .join(" ")
                .contains(r#"-c model_reasoning_effort="high""#),
            "codex argv should contain the quoted config override, got {:?}",
            inv.args
        );
        // The value token carries literal double quotes (no shell strips them).
        assert!(inv
            .args
            .iter()
            .any(|a| a == r#"model_reasoning_effort="high""#));
        assert_project_doc_suppressed(&inv.args);
    }

    #[test]
    fn no_effort_omits_config_override() {
        let inv = CodexAdapter.build_invocation(&input(None));
        assert!(!inv
            .args
            .iter()
            .any(|a| a.starts_with("model_reasoning_effort")));
        assert_project_doc_suppressed(&inv.args);
        assert_eq!(
            inv.args.iter().filter(|a| *a == "-c").count(),
            1,
            "only one -c flag should be present when effort is omitted"
        );
    }

    #[test]
    fn codex_supports_effort() {
        assert!(CodexAdapter.supports_effort());
    }

    #[test]
    fn codex_child_bypasses_sandbox_and_selects_no_sandbox_mode() {
        let inv = CodexAdapter.build_invocation(&input(None));
        assert_eq!(
            inv.args
                .iter()
                .filter(|a| *a == "--dangerously-bypass-approvals-and-sandbox")
                .count(),
            1,
            "codex child must carry the bypass flag exactly once, got {:?}",
            inv.args
        );
        assert!(
            !inv.args.iter().any(|a| a == "-s" || a == "--sandbox"),
            "codex child must not select a sandbox mode, got {:?}",
            inv.args
        );
    }
}
