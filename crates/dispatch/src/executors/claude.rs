//! Claude Code executor adapter.
//!
//! Spike-confirmed invocation:
//!   claude -p --model <m> --output-format json --dangerously-skip-permissions
//! On the tested CLI version, dispatch rewrites the output pair to
//! `--output-format stream-json --verbose` so passive quota events reach the output.
//! Spec: via stdin.
//! Session id: JSON root field `session_id`.
//! Resumable: `claude --resume <session_id>`
//!
//! Security note: `--dangerously-skip-permissions` grants the secondary CLI full
//! filesystem and terminal access. Enable only in a trusted local environment.

use super::{
    count_in, decimal_from_json, exceeds_native_bound, extract_json_field, json_objects,
    provider_cost, scoped_observation, single_counter, unsupported_cost, unsupported_token,
    Adapter, AdapterInvocation, AdapterInvocationInput, CounterTotal, NativeUsage, SpecDelivery,
};
use crate::quota::{
    gap_observation, missing, Acquisition, BillingMode, BucketId, Freshness, ProviderId,
    QuotaIdentity, QuotaObservation, QuotaRuntime, QuotaWindow, Utilization, UtilizationDirection,
    UtilizationScale, WindowKind, WindowName, COLLECTOR, QUOTA_SCHEMA_VERSION,
};
use crate::usage::{
    Availability, CounterRelation, CounterSemantics, CurrencyCode, DescendantScope, ModelRecord,
    MoneyUnit, ObservationSource, TokenMetric, TokenUsage,
};
use serde_json::Value;

/// Source version of the `result` record mapping.
const USAGE_SOURCE_VERSION: &str = "claude.result.usage.v1";

/// Source version of the passive `rate_limit_event` mapping, pinned to the tested CLI 2.1.301.
pub const RATE_LIMIT_SOURCE_VERSION: &str = "claude.rate_limit_event.v1";

/// The only CLI version whose stream output is tested to carry `rate_limit_event` records.
/// Other versions keep the json argv, so their terminal output does not change.
pub const STREAM_JSON_CLI_VERSION: &str = "2.1.301";

pub struct ClaudeAdapter;

/// Rewrites the json output pair in `args` to `stream-json --verbose`. Returns `false` and
/// leaves `args` unchanged when the json pair is absent.
pub fn enable_stream_json(args: &mut Vec<String>) -> bool {
    let Some(position) = args
        .windows(2)
        .position(|pair| pair[0] == "--output-format" && pair[1] == "json")
    else {
        return false;
    };
    args[position + 1] = "stream-json".to_string();
    args.insert(position + 2, "--verbose".to_string());
    true
}

/// Whether the output of `claude --version` names the tested stream-json CLI version.
pub fn supports_stream_json(version_output: &str) -> bool {
    version_output.split_whitespace().next() == Some(STREAM_JSON_CLI_VERSION)
}

impl Adapter for ClaudeAdapter {
    fn executor_name(&self) -> &str {
        "claude"
    }

    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "json".to_string(),
            "--dangerously-skip-permissions".to_string(),
            "--setting-sources".to_string(),
            "user".to_string(),
        ];
        if !input.model.is_empty() {
            args.push("--model".to_string());
            args.push(input.model.to_string());
        }
        if let Some(effort) = input.effort {
            args.push("--effort".to_string());
            args.push(effort.to_string());
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
        // Claude outputs a JSON object with `session_id` at root level.
        extract_json_field(stdout, "session_id")
    }

    /// Maps the `result` record into two observations with different scopes.
    ///
    /// The top-level `usage` counts this run's own work. `modelUsage` and `total_cost_usd`
    /// count the run including its child agents, so they stay in a separate observation.
    fn extract_native_usage(&self, stdout: &str) -> NativeUsage {
        if exceeds_native_bound(stdout) {
            return NativeUsage::oversized(self.executor_name(), USAGE_SOURCE_VERSION);
        }
        let result = json_objects(stdout)
            .into_iter()
            .rev()
            .find(|object| object.get("type").and_then(Value::as_str) == Some("result"))
            .unwrap_or(Value::Null);
        let status = match (
            result.get("is_error").and_then(Value::as_bool),
            result.get("subtype").and_then(Value::as_str),
        ) {
            (Some(true), subtype) => Some(format!(
                "result reported an error ({})",
                subtype.unwrap_or("no subtype")
            )),
            _ => None,
        };
        let status = status.as_deref();

        let own_usage = result.get("usage");
        let own_counter = |key: &str| TokenMetric {
            count: single_counter(
                count_in(own_usage, key),
                CounterSemantics::Delta,
                DescendantScope::Own,
                status,
            ),
            relation: CounterRelation::Exclusive,
        };
        let own_tokens = TokenUsage {
            input: own_counter("input_tokens"),
            output: own_counter("output_tokens"),
            cached_input: own_counter("cache_read_input_tokens"),
            cache_write: own_counter("cache_creation_input_tokens"),
            reasoning: unsupported_token("claude usage has no reasoning counter"),
            other: unsupported_token("claude usage has no other counter"),
        };
        let own = scoped_observation(
            self.executor_name(),
            USAGE_SOURCE_VERSION,
            own_tokens,
            unsupported_cost("claude reports cost on the run-scoped observation"),
            ModelRecord::default(),
        );

        let model_usage = result.get("modelUsage").and_then(Value::as_object);
        let mut input = CounterTotal::default();
        let mut output = CounterTotal::default();
        let mut cache_read = CounterTotal::default();
        let mut cache_creation = CounterTotal::default();
        if let Some(models) = model_usage {
            for model in models.values() {
                input.push(count_in(Some(model), "inputTokens"));
                output.push(count_in(Some(model), "outputTokens"));
                cache_read.push(count_in(Some(model), "cacheReadInputTokens"));
                cache_creation.push(count_in(Some(model), "cacheCreationInputTokens"));
            }
        }
        let scope = DescendantScope::IncludesDescendants;
        let run_tokens = TokenUsage {
            input: TokenMetric {
                count: input.finish(CounterSemantics::Unknown, scope, status),
                relation: CounterRelation::Exclusive,
            },
            output: TokenMetric {
                count: output.finish(CounterSemantics::Unknown, scope, status),
                relation: CounterRelation::Exclusive,
            },
            cached_input: TokenMetric {
                count: cache_read.finish(CounterSemantics::Unknown, scope, status),
                relation: CounterRelation::Exclusive,
            },
            cache_write: TokenMetric {
                count: cache_creation.finish(CounterSemantics::Unknown, scope, status),
                relation: CounterRelation::Exclusive,
            },
            reasoning: unsupported_token("claude modelUsage has no reasoning counter"),
            other: unsupported_token("claude modelUsage has no other counter"),
        };
        let models = ModelRecord {
            observed: model_usage
                .filter(|models| models.len() == 1)
                .and_then(|models| models.keys().next().cloned()),
            ..ModelRecord::default()
        };
        let cost = match CurrencyCode::new("USD") {
            Ok(code) => provider_cost(
                result.get("total_cost_usd"),
                MoneyUnit::Currency(code),
                "total_cost_usd",
            ),
            Err(error) => unsupported_cost(&error.to_string()),
        };
        let run = scoped_observation(
            self.executor_name(),
            USAGE_SOURCE_VERSION,
            run_tokens,
            cost,
            models,
        );
        NativeUsage::mapped(USAGE_SOURCE_VERSION, vec![own, run])
    }

    /// Reads passive `rate_limit_event` records that the output already carries. Without any
    /// such record, the result is an explicit gap. No control query exists for this CLI.
    fn extract_passive_quota(
        &self,
        stdout: &str,
        collected_at_unix_ms: u64,
    ) -> Vec<QuotaObservation> {
        let identity = claude_identity();
        let events: Vec<Value> = json_objects(stdout)
            .into_iter()
            .filter(|object| object.get("type").and_then(Value::as_str) == Some("rate_limit_event"))
            .collect();
        if events.is_empty() {
            return vec![gap_observation(
                identity,
                Acquisition::Unsupported {
                    reason: "output carries no rate_limit_event".to_owned(),
                },
                collected_at_unix_ms,
                RATE_LIMIT_SOURCE_VERSION,
            )];
        }
        events
            .iter()
            .map(|event| {
                rate_limit_observation(
                    &identity,
                    event.get("rate_limit_info"),
                    collected_at_unix_ms,
                )
            })
            .collect()
    }
}

/// Scope of the Claude Code account. The plan type is not visible in the event, so billing stays unknown.
fn claude_identity() -> QuotaIdentity {
    QuotaIdentity::unverified(
        QuotaRuntime::Claude,
        ProviderId::new("anthropic").expect("constant provider id is not empty"),
        BillingMode::Unknown,
    )
}

/// Maps one `rate_limit_info` object. The utilization stays a fraction. The reset time stays
/// unknown, because the timestamp unit is not verified, and the event carries no source time.
fn rate_limit_observation(
    identity: &QuotaIdentity,
    info: Option<&Value>,
    collected_at_unix_ms: u64,
) -> QuotaObservation {
    let field = |key: &str| {
        info.and_then(|info| info.get(key))
            .filter(|value| !value.is_null())
    };
    let window = match field("rateLimitType").and_then(Value::as_str) {
        None => Availability::Unavailable {
            reason: "rateLimitType is absent".to_owned(),
        },
        Some(kind) => match claude_window_kind(kind) {
            Ok(kind) => Availability::Reported {
                value: QuotaWindow {
                    kind,
                    duration_minutes: None,
                    resets_at_unix_ms: None,
                },
            },
            Err(reason) => Availability::Malformed { reason },
        },
    };
    let utilization = match field("utilization") {
        None => Availability::Unavailable {
            reason: "utilization is absent".to_owned(),
        },
        Some(value) => match decimal_from_json(value).and_then(|amount| {
            Utilization::new(
                amount,
                UtilizationScale::Fraction,
                UtilizationDirection::Used,
            )
            .map_err(|error| error.to_string())
        }) {
            Ok(utilization) => Availability::Reported { value: utilization },
            Err(reason) => Availability::Malformed { reason },
        },
    };
    const ABSOLUTE_REASON: &str = "claude rate_limit_event reports utilization only";
    QuotaObservation {
        schema_version: QUOTA_SCHEMA_VERSION,
        identity: identity.clone(),
        bucket: BucketId::new("claude-rate-limit").expect("constant bucket id is not empty"),
        window,
        utilization,
        used: missing(ABSOLUTE_REASON),
        remaining: missing(ABSOLUTE_REASON),
        limit: missing(ABSOLUTE_REASON),
        observed_at_unix_ms: None,
        collected_at_unix_ms,
        source: ObservationSource {
            collector: Some(COLLECTOR.to_owned()),
            source_version: Some(RATE_LIMIT_SOURCE_VERSION.to_owned()),
            cursor: None,
        },
        query: None,
        acquisition: Acquisition::Passive,
        freshness: Freshness::Unknown,
    }
}

/// Maps the documented window names. Other names keep their own identity instead of a fixed pair.
fn claude_window_kind(kind: &str) -> Result<WindowKind, String> {
    match kind {
        "five_hour" => Ok(WindowKind::FiveHour),
        "seven_day" => Ok(WindowKind::Weekly),
        other => WindowName::new(other)
            .map(|name| WindowKind::Named { name })
            .map_err(|error| error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::AdapterInvocationInput;
    use super::*;
    use std::path::Path;

    fn input<'a>(effort: Option<&'a str>) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model: "claude-sonnet-4-6",
            workdir: Path::new("/wd"),
            spec: "spec",
            effort,
            mcp_disable_servers: &[],
            timeout_secs: None,
        }
    }

    fn assert_setting_sources_suppressed(args: &[String]) {
        let adjacent = args
            .windows(2)
            .any(|w| w[0] == "--setting-sources" && w[1] == "user");
        assert!(
            adjacent,
            "claude invocation must carry an adjacent --setting-sources user pair, got {:?}",
            args
        );
        let count = args
            .windows(2)
            .filter(|w| w[0] == "--setting-sources" && w[1] == "user")
            .count();
        assert_eq!(count, 1, "--setting-sources user must occur exactly once");
    }

    #[test]
    fn effort_maps_to_effort_flag() {
        let inv = ClaudeAdapter.build_invocation(&input(Some("xhigh")));
        let joined = inv.args.join(" ");
        assert!(
            joined.contains("--effort xhigh"),
            "claude argv should contain `--effort xhigh`, got {:?}",
            inv.args
        );
        assert_setting_sources_suppressed(&inv.args);
    }

    #[test]
    fn no_effort_omits_flag() {
        let inv = ClaudeAdapter.build_invocation(&input(None));
        assert!(!inv.args.iter().any(|a| a == "--effort"));
        assert_setting_sources_suppressed(&inv.args);
    }

    #[test]
    fn claude_supports_effort() {
        assert!(ClaudeAdapter.supports_effort());
    }

    #[test]
    fn stream_json_rewrites_only_the_output_pair() {
        let mut args = ClaudeAdapter.build_invocation(&input(None)).args;
        let before = args.clone();
        assert!(enable_stream_json(&mut args));
        assert_eq!(
            args,
            [
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--dangerously-skip-permissions",
                "--setting-sources",
                "user",
                "--model",
                "claude-sonnet-4-6",
            ]
        );
        assert_eq!(before.len() + 1, args.len());
        assert_setting_sources_suppressed(&args);
    }

    #[test]
    fn stream_json_leaves_argv_without_the_json_pair_unchanged() {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "text".to_string(),
        ];
        let before = args.clone();
        assert!(!enable_stream_json(&mut args));
        assert_eq!(args, before);
    }

    #[test]
    fn stream_json_gate_accepts_only_the_tested_version() {
        assert!(supports_stream_json("2.1.301 (Claude Code)\n"));
        assert!(!supports_stream_json("2.1.300 (Claude Code)\n"));
        assert!(!supports_stream_json("2.1.3010 (Claude Code)\n"));
        assert!(!supports_stream_json(""));
        assert!(!supports_stream_json("{\"type\":\"result\"}"));
    }
}
