//! OpenCode executor adapter.
//!
//! Invocation (verified against installed OpenCode 1.17.18):
//!   opencode run --format json --auto --agent build -m <provider/model>
//! Spec: via stdin.
//! Session id: `sessionID` field in NDJSON stream events.
//! Resumable: `opencode run --session <sessionID>`
//!
//! `--auto` ("auto-approve permissions that are not explicitly denied (dangerous!)")
//! is the current permission-bypass flag required for headless write-back — without it
//! OpenCode auto-rejects the file `write`/`edit` tool call in non-interactive mode,
//! yielding `no-receipt`. It replaces the removed `--dangerously-skip-permissions`,
//! which is no longer a valid flag in current OpenCode releases.
//!
//! `--agent build` selects the write-capable agent (the default/`plan` agent is
//! read-only, which would silently produce no file changes).
//!
//! `effort` maps to `--variant` ("model variant (provider-specific reasoning effort,
//! e.g., high, max, minimal)").

use super::{
    count_in, exceeds_native_bound, extract_json_field, json_objects, scoped_observation,
    unsupported_cost, unsupported_token, Adapter, AdapterInvocation, AdapterInvocationInput,
    CounterTotal, NativeUsage, SpecDelivery,
};
use crate::quota::{
    gap_observation, Acquisition, BillingMode, ProviderId, QuotaIdentity, QuotaObservation,
    QuotaRuntime,
};
use crate::usage::{
    CounterRelation, CounterSemantics, DescendantScope, ModelRecord, TokenMetric, TokenUsage,
    UsageCategory,
};
use serde_json::Value;
use std::process::Command;

/// Source version recorded on the OpenCode quota gap. OpenCode is not on the current PATH.
const QUOTA_SOURCE_VERSION: &str = "opencode.session_stats.v1";

/// Source version of the `step_finish` mapping. Each step reports its own tokens.
const USAGE_SOURCE_VERSION: &str = "opencode.run.step_finish.v1";

pub struct OpenCodeAdapter;

impl Adapter for OpenCodeAdapter {
    fn executor_name(&self) -> &str {
        "opencode"
    }

    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation {
        let mut args = vec![
            "run".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--auto".to_string(),
            "--agent".to_string(),
            "build".to_string(),
        ];
        if !input.model.is_empty() {
            args.push("-m".to_string());
            args.push(input.model.to_string());
        }
        if let Some(effort) = input.effort {
            args.push("--variant".to_string());
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
        // OpenCode streams NDJSON events; `sessionID` appears in early events.
        extract_json_field(stdout, "sessionID")
    }

    /// Sums the tokens of each `step_finish` event. A trailing `error` event marks the
    /// sums partial, because the failed step may not have reported usage.
    fn extract_native_usage(&self, stdout: &str) -> NativeUsage {
        if exceeds_native_bound(stdout) {
            return NativeUsage::oversized(self.executor_name(), USAGE_SOURCE_VERSION);
        }
        let events = json_objects(stdout);
        let final_event_failed = events
            .last()
            .and_then(|event| event.get("type"))
            .and_then(Value::as_str)
            == Some("error");
        let mut input = CounterTotal::default();
        let mut output = CounterTotal::default();
        let mut reasoning = CounterTotal::default();
        let mut cache_read = CounterTotal::default();
        let mut cache_write = CounterTotal::default();
        for event in &events {
            if event.get("type").and_then(Value::as_str) != Some("step_finish") {
                continue;
            }
            let Some(tokens) = event.pointer("/part/tokens") else {
                continue;
            };
            input.push(count_in(Some(tokens), "input"));
            output.push(count_in(Some(tokens), "output"));
            reasoning.push(count_in(Some(tokens), "reasoning"));
            cache_read.push(count_in(tokens.get("cache"), "read"));
            cache_write.push(count_in(tokens.get("cache"), "write"));
        }
        let status = final_event_failed
            .then_some("final event reported an error, so usage covers completed steps only");
        let tokens = TokenUsage {
            input: TokenMetric {
                count: input.finish(CounterSemantics::Delta, DescendantScope::Own, status),
                relation: CounterRelation::Exclusive,
            },
            output: TokenMetric {
                count: output.finish(CounterSemantics::Delta, DescendantScope::Own, status),
                relation: CounterRelation::Exclusive,
            },
            cached_input: TokenMetric {
                count: cache_read.finish(CounterSemantics::Delta, DescendantScope::Own, status),
                relation: CounterRelation::Exclusive,
            },
            cache_write: TokenMetric {
                count: cache_write.finish(CounterSemantics::Delta, DescendantScope::Own, status),
                relation: CounterRelation::Exclusive,
            },
            reasoning: TokenMetric {
                count: reasoning.finish(CounterSemantics::Delta, DescendantScope::Own, status),
                relation: CounterRelation::SubsetOf {
                    category: UsageCategory::Output,
                },
            },
            other: unsupported_token("opencode step_finish has no other counter"),
        };
        let observation = scoped_observation(
            self.executor_name(),
            USAGE_SOURCE_VERSION,
            tokens,
            unsupported_cost("opencode cost currency is not declared by this schema"),
            ModelRecord::default(),
        );
        NativeUsage::mapped(USAGE_SOURCE_VERSION, vec![observation])
    }

    /// `opencode stats` reports token and cost use. It is not remaining quota, so the passive
    /// source is a gap that names the provider scope.
    fn extract_passive_quota(
        &self,
        _stdout: &str,
        collected_at_unix_ms: u64,
    ) -> Vec<QuotaObservation> {
        vec![quota_gap(
            "no provider-independent quota source; session statistics are token and cost use, not remaining quota",
            collected_at_unix_ms,
        )]
    }

    /// No verified machine quota read exists for the inspected OpenCode surface, and quota
    /// depends on the selected provider. The launch command is dropped unused.
    fn query_quota(&self, _launch: Command, collected_at_unix_ms: u64) -> Vec<QuotaObservation> {
        vec![quota_gap(
            "no verified read-only quota control for OpenCode; quota depends on the selected provider and billing mode",
            collected_at_unix_ms,
        )]
    }
}

fn quota_gap(reason: &str, collected_at_unix_ms: u64) -> QuotaObservation {
    gap_observation(
        QuotaIdentity::unverified(
            QuotaRuntime::OpenCode,
            ProviderId::new("opencode").expect("constant provider id is not empty"),
            BillingMode::Unknown,
        ),
        Acquisition::Unsupported {
            reason: reason.to_owned(),
        },
        collected_at_unix_ms,
        QUOTA_SOURCE_VERSION,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn input<'a>(effort: Option<&'a str>) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model: "nvidia/nvidia/nemotron-3-ultra-550b-a55b",
            workdir: Path::new("/wd"),
            spec: "spec",
            effort,
            mcp_disable_servers: &[],
            timeout_secs: None,
        }
    }

    #[test]
    fn argv_uses_auto_agent_build_and_omits_removed_flag() {
        let inv = OpenCodeAdapter.build_invocation(&input(None));
        let joined = inv.args.join(" ");
        assert!(
            joined.contains("run --format json --auto --agent build -m nvidia/nvidia/nemotron-3-ultra-550b-a55b"),
            "opencode argv should be run --format json --auto --agent build -m <model>, got {:?}",
            inv.args
        );
        assert!(
            !inv.args
                .iter()
                .any(|a| a == "--dangerously-skip-permissions"),
            "removed flag must not appear, got {:?}",
            inv.args
        );
    }

    #[test]
    fn effort_maps_to_variant_flag() {
        let inv = OpenCodeAdapter.build_invocation(&input(Some("max")));
        assert!(
            inv.args.join(" ").contains("--variant max"),
            "opencode argv should contain `--variant max`, got {:?}",
            inv.args
        );
    }

    #[test]
    fn opencode_supports_effort() {
        assert!(OpenCodeAdapter.supports_effort());
    }
}
