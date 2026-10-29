//! Host subagent model-tier policy and local model map resolution.

use crate::stage::Phase;
use serde_json::Value;
use std::{fs, path::Path};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ModelTier {
    Light,
    Standard,
    Deep,
}

impl ModelTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Standard => "standard",
            Self::Deep => "deep",
        }
    }
    fn lower(self) -> Self {
        match self {
            Self::Deep => Self::Standard,
            Self::Standard | Self::Light => Self::Light,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierSource {
    Default,
    Retry,
    Prerequisite,
    PrerequisiteTierUnknown,
    Override,
    FinalizeReview,
}

impl TierSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Retry => "retry",
            Self::Prerequisite => "prerequisite",
            Self::PrerequisiteTierUnknown => "prerequisite-tier-unknown",
            Self::Override => "override",
            Self::FinalizeReview => "finalize-review",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrerequisiteEvidence {
    pub accepted: bool,
    pub mode: String,
    pub tier: Option<ModelTier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSignals {
    pub phase: Phase,
    pub prior_failed_attempts: usize,
    pub implement: Option<PrerequisiteEvidence>,
    pub test: Option<PrerequisiteEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TierDecision {
    pub tier: ModelTier,
    pub source: TierSource,
    pub override_reason: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TierError {
    #[error("audit requires accepted implement and test execution evidence")]
    MissingPrerequisite,
    #[error("a tier override requires a non-empty reason")]
    MissingOverrideReason,
    #[error("override from {computed} to {requested} is not permitted for {phase}")]
    InvalidOverride {
        computed: &'static str,
        requested: &'static str,
        phase: &'static str,
    },
    #[error("investigate does not have a host-subagent tier policy")]
    UnsupportedPhase,
}

fn accepted_tier(evidence: Option<&PrerequisiteEvidence>) -> Result<(ModelTier, bool), TierError> {
    let e = evidence
        .filter(|e| e.accepted)
        .ok_or(TierError::MissingPrerequisite)?;
    Ok(match e.tier {
        Some(tier) => (tier, false),
        None if e.mode == "routed" || e.mode == "text" => (ModelTier::Standard, true),
        None => return Err(TierError::MissingPrerequisite),
    })
}

pub fn compute_tier(signals: &TaskSignals) -> Result<TierDecision, TierError> {
    use ModelTier::*;
    let (mut tier, mut source) = match signals.phase {
        Phase::FinalizeReview => (Standard, TierSource::FinalizeReview),
        Phase::Implement => (Light, TierSource::Default),
        Phase::Test => {
            let (base, unknown) = accepted_tier(signals.implement.as_ref())?;
            (
                base.lower(),
                if unknown {
                    TierSource::PrerequisiteTierUnknown
                } else {
                    TierSource::Prerequisite
                },
            )
        }
        Phase::Audit => {
            let (imp, imp_unknown) = accepted_tier(signals.implement.as_ref())?;
            let (test, test_unknown) = accepted_tier(signals.test.as_ref())?;
            (
                Standard.max(imp).max(test),
                if imp_unknown || test_unknown {
                    TierSource::PrerequisiteTierUnknown
                } else {
                    TierSource::Prerequisite
                },
            )
        }
        Phase::Investigate => return Err(TierError::UnsupportedPhase),
    };
    if signals.prior_failed_attempts > 0 && tier == Light {
        tier = Standard;
        source = TierSource::Retry;
    }
    Ok(TierDecision {
        tier,
        source,
        override_reason: None,
    })
}

pub fn apply_override(
    decision: TierDecision,
    phase: Phase,
    requested: Option<ModelTier>,
    reason: Option<&str>,
) -> Result<TierDecision, TierError> {
    let Some(requested) = requested else {
        return Ok(decision);
    };
    let reason = reason
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .ok_or(TierError::MissingOverrideReason)?;
    let permitted = requested >= decision.tier
        || (matches!(phase, Phase::Implement | Phase::Test) && requested == decision.tier.lower());
    if !permitted {
        return Err(TierError::InvalidOverride {
            computed: decision.tier.as_str(),
            requested: requested.as_str(),
            phase: phase.as_str(),
        });
    }
    Ok(TierDecision {
        tier: requested,
        source: TierSource::Override,
        override_reason: Some(reason.to_owned()),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEffort {
    pub model: String,
    pub effort: Option<String>,
    pub inherited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostModelMap {
    codex: [ModelEffort; 3],
    claude: [ModelEffort; 3],
}

#[derive(Debug, Error)]
pub enum HostModelMapError {
    #[error("could not read host model configuration: {0}")]
    Io(#[from] std::io::Error),
    #[error("host model configuration is malformed JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("hostSubagentModels must be an object")]
    InvalidRoot,
    #[error("invalid {host} model configuration for tier {tier}: {detail}")]
    InvalidEntry {
        host: String,
        tier: String,
        detail: String,
    },
}

fn defaults(host: &str) -> [ModelEffort; 3] {
    if host == "codex" {
        [
            ModelEffort {
                model: "gpt-6-luna".into(),
                effort: Some("medium".into()),
                inherited: false,
            },
            ModelEffort {
                model: "gpt-6.1-sol".into(),
                effort: Some("medium".into()),
                inherited: false,
            },
            ModelEffort {
                model: "gpt-6-astra".into(),
                effort: Some("medium".into()),
                inherited: false,
            },
        ]
    } else {
        [
            ModelEffort {
                model: "haiku".into(),
                effort: None,
                inherited: false,
            },
            ModelEffort {
                model: "sonnet".into(),
                effort: None,
                inherited: false,
            },
            ModelEffort {
                model: "opus".into(),
                effort: None,
                inherited: false,
            },
        ]
    }
}

impl HostModelMap {
    pub fn load_default() -> Result<Self, HostModelMapError> {
        let path = crate::routing::default_routing_path().ok_or_else(|| {
            HostModelMapError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "cannot determine GAL config path",
            ))
        })?;
        Self::load(path)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, HostModelMapError> {
        let path = path.as_ref();
        let value: Value = match fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Null,
            Err(e) => return Err(e.into()),
        };
        let mut map = Self {
            codex: defaults("codex"),
            claude: defaults("claude"),
        };
        if value.is_null() {
            return Ok(map);
        }
        if !value.is_object() {
            return Err(HostModelMapError::InvalidRoot);
        }
        let Some(entries) = value.get("hostSubagentModels") else {
            return Ok(map);
        };
        let hosts = entries.as_object().ok_or(HostModelMapError::InvalidRoot)?;
        for host in ["codex", "claude"] {
            let Some(tiers) = hosts.get(host) else {
                continue;
            };
            let tiers = tiers
                .as_object()
                .ok_or_else(|| invalid(host, "*", "host value must be an object"))?;
            for (idx, tier) in ["light", "standard", "deep"].iter().enumerate() {
                let Some(raw) = tiers.get(*tier) else {
                    continue;
                };
                let entry = parse_entry(host, tier, raw)?;
                if host == "codex" {
                    map.codex[idx] = entry;
                } else {
                    map.claude[idx] = entry;
                }
            }
            for tier in tiers.keys() {
                if !["light", "standard", "deep"].contains(&tier.as_str()) {
                    return Err(invalid(host, tier, "unknown tier"));
                }
            }
        }
        for host in hosts.keys() {
            if !["codex", "claude"].contains(&host.as_str()) {
                return Err(invalid(host, "*", "unknown host"));
            }
        }
        Ok(map)
    }

    pub fn resolve(&self, host: &str, tier: ModelTier) -> Result<ModelEffort, HostModelMapError> {
        let entries = match host {
            "codex" => &self.codex,
            "claude" => &self.claude,
            _ => return Err(invalid(host, tier.as_str(), "unknown host")),
        };
        Ok(entries[tier as usize].clone())
    }
}

fn invalid(host: &str, tier: &str, detail: &str) -> HostModelMapError {
    HostModelMapError::InvalidEntry {
        host: host.into(),
        tier: tier.into(),
        detail: detail.into(),
    }
}

fn parse_entry(host: &str, tier: &str, raw: &Value) -> Result<ModelEffort, HostModelMapError> {
    let o = raw
        .as_object()
        .ok_or_else(|| invalid(host, tier, "entry must be an object"))?;
    let model = o
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(host, tier, "model must be a string"))?;
    if model == "inherit" {
        if o.get("effort").is_some() {
            return Err(invalid(host, tier, "inherit cannot specify effort"));
        }
        return Ok(ModelEffort {
            model: model.into(),
            effort: None,
            inherited: true,
        });
    }
    let effort = o
        .get("effort")
        .map(|v| {
            v.as_str()
                .ok_or_else(|| invalid(host, tier, "effort must be a string"))
        })
        .transpose()?
        .map(str::to_owned);
    let valid = if host == "codex" {
        let supported = ["medium", "high", "xhigh"];
        let compatible = match model {
            "gpt-6-luna" => matches!(effort.as_deref(), Some("medium" | "high" | "xhigh")),
            "gpt-6.1-sol" => matches!(effort.as_deref(), Some("medium" | "high" | "xhigh")),
            "gpt-5.6-sol" => matches!(effort.as_deref(), Some("medium" | "high" | "xhigh")),
            "gpt-6-astra" => matches!(effort.as_deref(), Some("medium" | "high" | "xhigh")),
            _ => false,
        };
        compatible
            && effort
                .as_deref()
                .map(|e| supported.contains(&e))
                .unwrap_or(false)
    } else {
        ["haiku", "sonnet", "opus", "fable"].contains(&model)
            && effort
                .as_deref()
                .map(|e| ["low", "medium", "high", "xhigh", "max"].contains(&e))
                .unwrap_or(true)
    };
    if !valid {
        return Err(invalid(host, tier, "unsupported model/effort combination"));
    }
    Ok(ModelEffort {
        model: model.into(),
        effort,
        inherited: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn signals(phase: Phase) -> TaskSignals {
        TaskSignals {
            phase,
            prior_failed_attempts: 0,
            implement: None,
            test: None,
        }
    }
    fn evidence(tier: Option<ModelTier>, mode: &str) -> Option<PrerequisiteEvidence> {
        Some(PrerequisiteEvidence {
            accepted: true,
            mode: mode.into(),
            tier,
        })
    }

    #[test]
    fn model_tier_implement_is_light_with_default_source() {
        let d = compute_tier(&signals(Phase::Implement)).unwrap();
        assert_eq!(d.tier, ModelTier::Light);
        assert_eq!(d.source, TierSource::Default);
    }
    #[test]
    fn model_tier_test_after_implement_light_stays_light() {
        let mut s = signals(Phase::Test);
        s.implement = evidence(Some(ModelTier::Light), "host-subagent");
        let d = compute_tier(&s).unwrap();
        assert_eq!(d.tier, ModelTier::Light);
        assert_eq!(d.source, TierSource::Prerequisite);
    }
    #[test]
    fn model_tier_test_after_implement_standard_lowers_to_light() {
        let mut s = signals(Phase::Test);
        s.implement = evidence(Some(ModelTier::Standard), "host-subagent");
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Light);
    }
    #[test]
    fn model_tier_test_after_implement_deep_lowers_to_standard() {
        let mut s = signals(Phase::Test);
        s.implement = evidence(Some(ModelTier::Deep), "host-subagent");
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Standard);
    }
    #[test]
    fn model_tier_tierless_routed_and_text_implement_evidence_is_standard_for_test_and_audit() {
        for mode in ["routed", "text"] {
            let mut s = signals(Phase::Test);
            s.implement = evidence(None, mode);
            let d = compute_tier(&s).unwrap();
            assert_eq!(d.tier, ModelTier::Light, "test after {mode}");
            assert_eq!(d.source, TierSource::PrerequisiteTierUnknown);
            s.phase = Phase::Audit;
            s.test = evidence(Some(ModelTier::Light), "host-subagent");
            let d = compute_tier(&s).unwrap();
            assert_eq!(d.tier, ModelTier::Standard, "audit after {mode}");
            assert_eq!(d.source, TierSource::PrerequisiteTierUnknown);
        }
    }
    #[test]
    fn model_tier_tierless_host_subagent_evidence_is_missing_prerequisite() {
        let mut s = signals(Phase::Test);
        s.implement = evidence(None, "host-subagent");
        assert_eq!(compute_tier(&s), Err(TierError::MissingPrerequisite));
        s.phase = Phase::Audit;
        s.test = evidence(Some(ModelTier::Light), "host-subagent");
        assert_eq!(compute_tier(&s), Err(TierError::MissingPrerequisite));
    }
    #[test]
    fn model_tier_audit_light_prerequisites_floor_at_standard() {
        let mut s = signals(Phase::Audit);
        s.implement = evidence(Some(ModelTier::Light), "host-subagent");
        s.test = evidence(Some(ModelTier::Light), "host-subagent");
        let d = compute_tier(&s).unwrap();
        assert_eq!(d.tier, ModelTier::Standard);
        assert_eq!(d.source, TierSource::Prerequisite);
    }
    #[test]
    fn model_tier_audit_deep_prerequisite_stays_deep() {
        let mut s = signals(Phase::Audit);
        s.implement = evidence(Some(ModelTier::Light), "host-subagent");
        s.test = evidence(Some(ModelTier::Deep), "host-subagent");
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Deep);
    }
    #[test]
    fn model_tier_finalize_review_is_standard() {
        let d = compute_tier(&signals(Phase::FinalizeReview)).unwrap();
        assert_eq!(d.tier, ModelTier::Standard);
        assert_eq!(d.source, TierSource::FinalizeReview);
    }
    #[test]
    fn model_tier_investigate_is_unsupported() {
        assert_eq!(
            compute_tier(&signals(Phase::Investigate)),
            Err(TierError::UnsupportedPhase)
        );
    }
    #[test]
    fn model_tier_audit_requires_both_and_uses_maximum() {
        let mut s = signals(Phase::Audit);
        s.implement = evidence(Some(ModelTier::Light), "host-subagent");
        s.test = evidence(Some(ModelTier::Deep), "host-subagent");
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Deep);
    }
    #[test]
    fn model_tier_override_records_reason_and_limits_downgrade() {
        let s = signals(Phase::Implement);
        let d = compute_tier(&s).unwrap();
        let d = apply_override(
            d,
            Phase::Implement,
            Some(ModelTier::Deep),
            Some("cross-cutting change"),
        )
        .unwrap();
        assert_eq!(d.tier, ModelTier::Deep);
        assert_eq!(d.source, TierSource::Override);
        assert_eq!(d.override_reason.as_deref(), Some("cross-cutting change"));
        let audit = TierDecision {
            tier: ModelTier::Standard,
            source: TierSource::Prerequisite,
            override_reason: None,
        };
        assert!(matches!(
            apply_override(audit, Phase::Audit, Some(ModelTier::Light), Some("reason")),
            Err(TierError::InvalidOverride { .. })
        ));
    }
    #[test]
    fn model_tier_reasoned_deep_override_is_accepted_in_every_phase() {
        let cases = [
            (Phase::Implement, light_decision()),
            (Phase::Test, light_decision()),
            (Phase::Audit, standard_decision()),
            (Phase::FinalizeReview, standard_decision()),
        ];
        for (phase, decision) in cases {
            let d = apply_override(decision, phase, Some(ModelTier::Deep), Some("reason")).unwrap();
            assert_eq!(d.tier, ModelTier::Deep);
            assert_eq!(d.source, TierSource::Override);
        }
    }
    #[test]
    fn model_tier_override_without_reason_is_rejected() {
        let decision = compute_tier(&signals(Phase::Implement)).unwrap();
        assert_eq!(
            apply_override(
                decision.clone(),
                Phase::Implement,
                Some(ModelTier::Deep),
                None
            ),
            Err(TierError::MissingOverrideReason)
        );
        assert_eq!(
            apply_override(
                decision,
                Phase::Implement,
                Some(ModelTier::Deep),
                Some("  ")
            ),
            Err(TierError::MissingOverrideReason)
        );
    }
    fn light_decision() -> TierDecision {
        TierDecision {
            tier: ModelTier::Light,
            source: TierSource::Default,
            override_reason: None,
        }
    }
    fn standard_decision() -> TierDecision {
        TierDecision {
            tier: ModelTier::Standard,
            source: TierSource::Prerequisite,
            override_reason: None,
        }
    }
    #[test]
    fn host_model_map_defaults_and_explicit_overrides() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        let map = HostModelMap::load(&path).unwrap();
        assert_eq!(
            map.resolve("codex", ModelTier::Light).unwrap().model,
            "gpt-6-luna"
        );
        std::fs::write(&path,r#"{"hostSubagentModels":{"codex":{"deep":{"model":"gpt-6.1-sol","effort":"high"}},"claude":{"deep":{"model":"fable"}}}}"#).unwrap();
        let map = HostModelMap::load(&path).unwrap();
        assert_eq!(
            map.resolve("codex", ModelTier::Deep)
                .unwrap()
                .effort
                .as_deref(),
            Some("high")
        );
        assert_eq!(
            map.resolve("claude", ModelTier::Deep).unwrap().model,
            "fable"
        );
    }
    #[test]
    fn host_model_map_accepts_gpt_5_6_sol_with_supported_effort_at_every_tier() {
        for tier in ["light", "standard", "deep"] {
            for effort in ["medium", "high", "xhigh"] {
                let entry = serde_json::json!({"model": "gpt-5.6-sol", "effort": effort});
                let parsed = parse_entry("codex", tier, &entry).unwrap();
                assert_eq!(parsed.model, "gpt-5.6-sol");
                assert_eq!(parsed.effort.as_deref(), Some(effort));
                assert!(!parsed.inherited);
            }
            for entry in [
                serde_json::json!({"model": "gpt-5.6-sol"}),
                serde_json::json!({"model": "gpt-5.6-sol", "effort": "low"}),
                serde_json::json!({"model": "gpt-5.6-sol", "effort": "extreme"}),
            ] {
                assert!(parse_entry("codex", tier, &entry).is_err());
            }
        }
    }

    #[test]
    fn host_model_map_rejects_malformed_and_invalid_entries_and_records_inherit() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "{").unwrap();
        assert!(HostModelMap::load(&path).is_err());
        std::fs::write(
            &path,
            r#"{"hostSubagentModels":{"codex":{"light":{"model":"gpt-6-luna","effort":"extreme"}}}}"#,
        )
        .unwrap();
        assert!(HostModelMap::load(&path).is_err());
        std::fs::write(
            &path,
            r#"{"hostSubagentModels":{"codex":{"light":{"model":"gpt-6-luna","effort":"high"}}}}"#,
        )
        .unwrap();
        assert_eq!(
            HostModelMap::load(&path)
                .unwrap()
                .resolve("codex", ModelTier::Light)
                .unwrap()
                .effort
                .as_deref(),
            Some("high")
        );
        std::fs::write(
            &path,
            r#"{"hostSubagentModels":{"codex":{"light":{"model":"inherit"}}}}"#,
        )
        .unwrap();
        assert!(
            HostModelMap::load(&path)
                .unwrap()
                .resolve("codex", ModelTier::Light)
                .unwrap()
                .inherited
        );
    }

    #[test]
    fn model_tier_one_prior_failure_raises_light_to_standard_with_retry_source() {
        let mut s = signals(Phase::Implement);
        s.prior_failed_attempts = 1;
        let d = compute_tier(&s).unwrap();
        assert_eq!(d.tier, ModelTier::Standard);
        assert_eq!(d.source, TierSource::Retry);
    }
    #[test]
    fn model_tier_many_prior_failures_never_raise_above_standard() {
        let mut s = signals(Phase::Implement);
        s.prior_failed_attempts = 9;
        let d = compute_tier(&s).unwrap();
        assert_eq!(d.tier, ModelTier::Standard);
        assert_eq!(d.source, TierSource::Retry);
    }
    #[test]
    fn model_tier_prior_failure_raises_lowered_test_light_to_standard() {
        let mut t = signals(Phase::Test);
        t.implement = evidence(Some(ModelTier::Standard), "host-subagent");
        t.prior_failed_attempts = 3;
        let d = compute_tier(&t).unwrap();
        assert_eq!(d.tier, ModelTier::Standard);
        assert_eq!(d.source, TierSource::Retry);
    }
    #[test]
    fn model_tier_deep_prerequisite_stays_deep_under_prior_failures() {
        let mut a = signals(Phase::Audit);
        a.implement = evidence(Some(ModelTier::Deep), "host-subagent");
        a.test = evidence(Some(ModelTier::Light), "host-subagent");
        a.prior_failed_attempts = 3;
        let d = compute_tier(&a).unwrap();
        assert_eq!(d.tier, ModelTier::Deep);
        assert_eq!(d.source, TierSource::Prerequisite);
    }

    #[test]
    fn model_tier_tierless_routed_or_text_test_evidence_raises_to_standard_on_retry() {
        for mode in ["routed", "text"] {
            let mut t = signals(Phase::Test);
            t.implement = evidence(None, mode);
            t.prior_failed_attempts = 1;
            let d = compute_tier(&t).unwrap();
            assert_eq!(d.tier, ModelTier::Standard, "test after {mode} with retry");
            assert_eq!(d.source, TierSource::Retry);
        }
    }
    #[test]
    fn model_tier_audit_standard_floor_keeps_prerequisite_source_under_failures() {
        let mut a = signals(Phase::Audit);
        a.implement = evidence(Some(ModelTier::Light), "host-subagent");
        a.test = evidence(Some(ModelTier::Light), "host-subagent");
        a.prior_failed_attempts = 2;
        let d = compute_tier(&a).unwrap();
        assert_eq!(d.tier, ModelTier::Standard);
        assert_eq!(d.source, TierSource::Prerequisite);
    }
    #[test]
    fn model_tier_standard_test_tier_is_not_raised_to_deep_by_failures() {
        let mut t = signals(Phase::Test);
        t.implement = evidence(Some(ModelTier::Deep), "host-subagent");
        t.prior_failed_attempts = 5;
        let d = compute_tier(&t).unwrap();
        assert_eq!(d.tier, ModelTier::Standard);
        assert_eq!(d.source, TierSource::Prerequisite);
    }

    #[test]
    fn model_tier_audit_and_finalize_review_reject_lowering_overrides() {
        for (phase, computed) in [
            (Phase::Audit, ModelTier::Standard),
            (Phase::FinalizeReview, ModelTier::Deep),
        ] {
            assert!(matches!(
                apply_override(
                    TierDecision {
                        tier: computed,
                        source: TierSource::Prerequisite,
                        override_reason: None
                    },
                    phase,
                    Some(ModelTier::Light),
                    Some("reason")
                ),
                Err(TierError::InvalidOverride { .. })
            ));
        }
        assert!(matches!(
            apply_override(
                TierDecision {
                    tier: ModelTier::Deep,
                    source: TierSource::Prerequisite,
                    override_reason: None
                },
                Phase::Implement,
                Some(ModelTier::Light),
                Some("reason")
            ),
            Err(TierError::InvalidOverride { .. })
        ));
    }

    #[test]
    fn model_tier_test_override_lowers_at_most_one_step() {
        let standard = TierDecision {
            tier: ModelTier::Standard,
            source: TierSource::Prerequisite,
            override_reason: None,
        };
        let d = apply_override(
            standard,
            Phase::Test,
            Some(ModelTier::Light),
            Some("reason"),
        );
        assert_eq!(d.unwrap().tier, ModelTier::Light);
        let deep = TierDecision {
            tier: ModelTier::Deep,
            source: TierSource::Prerequisite,
            override_reason: None,
        };
        assert!(matches!(
            apply_override(deep, Phase::Test, Some(ModelTier::Light), Some("reason")),
            Err(TierError::InvalidOverride { .. })
        ));
    }

    #[test]
    fn host_model_map_all_defaults_are_documented() {
        let dir = TempDir::new().unwrap();
        let map = HostModelMap::load(dir.path().join("missing.json")).unwrap();
        for (tier, model) in [
            (ModelTier::Light, "gpt-6-luna"),
            (ModelTier::Standard, "gpt-6.1-sol"),
            (ModelTier::Deep, "gpt-6-astra"),
        ] {
            let entry = map.resolve("codex", tier).unwrap();
            assert_eq!(entry.model, model);
            assert_eq!(entry.effort.as_deref(), Some("medium"));
        }
        for (tier, model) in [
            (ModelTier::Light, "haiku"),
            (ModelTier::Standard, "sonnet"),
            (ModelTier::Deep, "opus"),
        ] {
            let entry = map.resolve("claude", tier).unwrap();
            assert_eq!(entry.model, model);
            assert_eq!(entry.effort, None);
        }
    }

    #[test]
    fn host_model_map_rejects_unknown_host_and_tier_keys() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"hostSubagentModels":{"other":{}}}"#).unwrap();
        assert!(HostModelMap::load(&path).is_err());
        std::fs::write(&path, r#"{"hostSubagentModels":{"codex":{"extra":{"model":"gpt-6-luna","effort":"medium"}}}}"#).unwrap();
        assert!(HostModelMap::load(&path).is_err());
    }

    #[test]
    fn host_model_map_allows_claude_effort_and_rejects_unknown_effort() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"hostSubagentModels":{"claude":{"deep":{"model":"fable","effort":"max"}}}}"#,
        )
        .unwrap();
        assert_eq!(
            HostModelMap::load(&path)
                .unwrap()
                .resolve("claude", ModelTier::Deep)
                .unwrap()
                .effort
                .as_deref(),
            Some("max")
        );
        std::fs::write(
            &path,
            r#"{"hostSubagentModels":{"claude":{"deep":{"model":"opus","effort":"extreme"}}}}"#,
        )
        .unwrap();
        assert!(HostModelMap::load(&path).is_err());
    }
}
