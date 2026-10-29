//! GitHub Copilot CLI executor adapter.
//!
//! Spike-confirmed invocation:
//!   copilot -C <workdir> --model <m> --allow-all --output-format json -p <spec>
//! Spec: passed via `-p` CLI flag (NOT stdin).
//! Session id: `result.sessionId` nested field in JSON output.
//! Resumable: `copilot --resume=<sessionId>`
//!
//! Security note: `--allow-all` grants the secondary CLI full filesystem, tool,
//! and URL access. Enable only in a trusted local environment.

use super::{
    count_in, exceeds_native_bound, extract_nested_json_field, json_objects, provider_cost,
    scoped_observation, single_counter, uniform_tokens, unsupported_cost, unsupported_token,
    Adapter, AdapterInvocation, AdapterInvocationInput, NativeUsage, SpecDelivery,
};
use crate::quota::{
    gap_observation, Acquisition, BillingMode, ProviderId, QuotaIdentity, QuotaObservation,
    QuotaRuntime,
};
use crate::usage::{
    Availability, CounterRelation, CounterSemantics, CreditUnit, DescendantScope, ModelRecord,
    MoneyUnit, TokenCount, TokenMetric, TokenUsage, UsageCategory,
};
use serde_json::Value;
use std::process::Command;

/// Source version recorded on the Copilot quota gap, pinned to the tested CLI 1.0.98.
const QUOTA_SOURCE_VERSION: &str = "copilot.account.get_quota.v1";

/// Source version of the usage block mapping.
const USAGE_SOURCE_VERSION: &str = "copilot.usage.v1";

/// Usage block the output must declare before any counter is read.
const DECLARED_USAGE_SCHEMA: &str = "copilot.usage";
const DECLARED_USAGE_VERSION: u64 = 1;

pub struct CopilotAdapter;

impl Adapter for CopilotAdapter {
    fn executor_name(&self) -> &str {
        "copilot"
    }

    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation {
        let mut args = vec![
            "-C".to_string(),
            input.workdir.to_string_lossy().into_owned(),
            "--allow-all".to_string(),
            "--output-format".to_string(),
            "json".to_string(),
            // Context slimming (adapter-owned, always on): Copilot Free's headless
            // prompt-mode fails when it loads the large generated custom-instructions
            // context and built-in MCP servers, overflowing before it can write the
            // receipt. The task spec + its referenced agent-contract file are
            // self-sufficient (dispatch verifies the contract path before dispatch),
            // so the custom instructions are not needed. Default tools are kept — the
            // receipt write-back needs the file `write` tool, and `--available-tools
            // write` was empirically over-restrictive (no real write-back).
            "--no-custom-instructions".to_string(),
            "--disable-builtin-mcps".to_string(),
        ];
        if !input.model.is_empty() {
            args.push("--model".to_string());
            args.push(input.model.to_string());
        }
        if let Some(effort) = input.effort {
            args.push("--reasoning-effort".to_string());
            args.push(effort.to_string());
        }
        // Disable each host-configured MCP server the dispatch layer resolved
        // (from `~/.copilot/mcp-config.json`), one flag per name.
        for name in input.mcp_disable_servers {
            args.push("--disable-mcp-server".to_string());
            args.push(name.clone());
        }
        // Spec is passed as the `-p` argument — copilot does not read from stdin.
        args.push("-p".to_string());
        args.push(input.spec.to_string());
        // stdin is empty because spec is in args
        AdapterInvocation {
            args,
            delivery: SpecDelivery::CliFlag("-p".to_string()),
        }
    }

    fn supports_effort(&self) -> bool {
        true
    }

    fn extract_session_id(&self, stdout: &str) -> Option<String> {
        // Copilot JSON output has `result.sessionId` (nested).
        extract_nested_json_field(stdout, "result", "sessionId").or_else(|| {
            // Some versions emit sessionId at root level too
            super::extract_json_field(stdout, "sessionId")
        })
    }

    /// Maps the declared `usage` block. Tokens and credits form one observation. The legacy
    /// premium request count forms a second one, because it uses a different unit.
    fn extract_native_usage(&self, stdout: &str) -> NativeUsage {
        if exceeds_native_bound(stdout) {
            return NativeUsage::oversized(self.executor_name(), USAGE_SOURCE_VERSION);
        }
        let Some(usage) = json_objects(stdout)
            .into_iter()
            .rev()
            .find_map(|object| object.get("usage").cloned())
        else {
            return NativeUsage::unsupported("copilot output carries no usage block");
        };
        if usage.get("schema").and_then(Value::as_str) != Some(DECLARED_USAGE_SCHEMA)
            || usage.get("version").and_then(Value::as_u64) != Some(DECLARED_USAGE_VERSION)
        {
            return NativeUsage::unsupported("copilot usage block declares an unrecognized schema");
        }

        let token_block = usage.get("tokens");
        // A token block without input or output is incomplete, so none of its counters is trusted.
        let complete = token_block
            .is_some_and(|block| block.get("input").is_some() && block.get("output").is_some());
        let token = |key: &str| -> Availability<TokenCount> {
            match token_block {
                None => Availability::Unavailable {
                    reason: "token statistics are absent".to_owned(),
                },
                Some(_) if !complete => Availability::Malformed {
                    reason: "token statistics are incomplete".to_owned(),
                },
                Some(_) => count_in(token_block, key),
            }
        };
        let delta = |key: &str| {
            single_counter(
                token(key),
                CounterSemantics::Delta,
                DescendantScope::Own,
                None,
            )
        };
        let tokens = TokenUsage {
            input: TokenMetric {
                count: delta("input"),
                relation: CounterRelation::Exclusive,
            },
            output: TokenMetric {
                count: delta("output"),
                relation: CounterRelation::Exclusive,
            },
            cached_input: TokenMetric {
                count: delta("cache_read"),
                relation: CounterRelation::SubsetOf {
                    category: UsageCategory::Input,
                },
            },
            reasoning: TokenMetric {
                count: delta("reasoning"),
                relation: CounterRelation::SubsetOf {
                    category: UsageCategory::Output,
                },
            },
            cache_write: unsupported_token("copilot token statistics have no cache write counter"),
            other: unsupported_token("copilot token statistics have no other counter"),
        };

        let credit = |key: &str, unit: &str| match CreditUnit::new(unit) {
            Ok(unit) => provider_cost(usage.get(key), MoneyUnit::Credit(unit), key),
            Err(error) => unsupported_cost(&error.to_string()),
        };
        let mut observations = vec![scoped_observation(
            self.executor_name(),
            USAGE_SOURCE_VERSION,
            tokens,
            credit("credits", "copilot-credit"),
            ModelRecord::default(),
        )];
        if usage.get("premium_requests").is_some() {
            observations.push(scoped_observation(
                self.executor_name(),
                USAGE_SOURCE_VERSION,
                uniform_tokens(&Availability::Unsupported {
                    reason: "premium request record carries no token counters".to_owned(),
                }),
                credit("premium_requests", "copilot-premium-request"),
                ModelRecord::default(),
            ));
        }
        NativeUsage::mapped(USAGE_SOURCE_VERSION, observations)
    }

    /// Copilot output carries no quota snapshot in the usage block, so the passive source is a gap.
    fn extract_passive_quota(
        &self,
        _stdout: &str,
        collected_at_unix_ms: u64,
    ) -> Vec<QuotaObservation> {
        vec![quota_gap(
            "copilot output carries no quota snapshot; the usage block reports tokens and credits",
            collected_at_unix_ms,
        )]
    }

    /// The SDK's `account.getQuota` is the documented source. Its stdio entry point and connect
    /// handshake are not verified for CLI 1.0.98, so no control process starts. The launch
    /// command is dropped unused.
    fn query_quota(&self, _launch: Command, collected_at_unix_ms: u64) -> Vec<QuotaObservation> {
        vec![quota_gap(
            "copilot account.getQuota: stdio entry point and connect handshake are unverified for CLI 1.0.98, so no control query was sent",
            collected_at_unix_ms,
        )]
    }
}

fn quota_gap(reason: &str, collected_at_unix_ms: u64) -> QuotaObservation {
    gap_observation(
        QuotaIdentity::unverified(
            QuotaRuntime::Copilot,
            ProviderId::new("github").expect("constant provider id is not empty"),
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

    fn input<'a>(effort: Option<&'a str>, disable: &'a [String]) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model: "auto",
            workdir: Path::new("/wd"),
            spec: "spec",
            effort,
            mcp_disable_servers: disable,
            timeout_secs: None,
        }
    }

    #[test]
    fn effort_maps_to_reasoning_effort_flag() {
        let inv = CopilotAdapter.build_invocation(&input(Some("high"), &[]));
        assert!(
            inv.args.join(" ").contains("--reasoning-effort high"),
            "copilot argv should contain `--reasoning-effort high`, got {:?}",
            inv.args
        );
    }

    #[test]
    fn copilot_supports_effort() {
        assert!(CopilotAdapter.supports_effort());
    }

    #[test]
    fn context_slimming_flags_always_present_default_tools_kept() {
        let inv = CopilotAdapter.build_invocation(&input(None, &[]));
        let joined = inv.args.join(" ");
        assert!(
            joined.contains("--no-custom-instructions"),
            "got {:?}",
            inv.args
        );
        assert!(
            joined.contains("--disable-builtin-mcps"),
            "got {:?}",
            inv.args
        );
        // Base flags preserved.
        assert!(joined.contains("--allow-all"));
        assert!(joined.contains("--output-format json"));
        assert!(inv.args.iter().any(|a| a == "-C"));
        assert!(inv.args.iter().any(|a| a == "-p"));
        // Default tool surface kept — no --available-tools restriction.
        assert!(
            !inv.args.iter().any(|a| a == "--available-tools"),
            "got {:?}",
            inv.args
        );
    }

    #[test]
    fn one_disable_mcp_server_flag_per_entry() {
        let disable = vec!["srv-a".to_string(), "srv-b".to_string()];
        let inv = CopilotAdapter.build_invocation(&input(None, &disable));
        let count = inv
            .args
            .iter()
            .filter(|a| *a == "--disable-mcp-server")
            .count();
        assert_eq!(
            count, 2,
            "one --disable-mcp-server per name, got {:?}",
            inv.args
        );
        assert!(inv.args.iter().any(|a| a == "srv-a"));
        assert!(inv.args.iter().any(|a| a == "srv-b"));
    }
}
