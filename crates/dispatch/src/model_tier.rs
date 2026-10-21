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
    fn raise(self) -> Self {
        match self {
            Self::Light => Self::Standard,
            Self::Standard | Self::Deep => Self::Deep,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierSource {
    Documentation,
    Standard,
    DeepTrigger,
    Retry,
    Prerequisite,
    PrerequisiteTierUnknown,
    Override,
    FinalizeReview,
}

impl TierSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Documentation => "documentation",
            Self::Standard => "standard",
            Self::DeepTrigger => "deep-trigger",
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
    pub files: Vec<String>,
    pub crate_roots: Vec<String>,
    pub has_protected_path: bool,
    pub dependency_count: usize,
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

fn is_documentation(path: &str) -> bool {
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(ext.as_str(), "md" | "mdx" | "txt" | "rst" | "adoc")
}

fn accepted_tier(evidence: Option<&PrerequisiteEvidence>) -> Result<(ModelTier, bool), TierError> {
    let e = evidence
        .filter(|e| e.accepted)
        .ok_or(TierError::MissingPrerequisite)?;
    Ok(match e.tier {
        Some(tier) => (tier, false),
        None if e.mode == "routed" || e.mode == "text" => (ModelTier::Deep, true),
        None if e.mode == "host-subagent" => return Err(TierError::MissingPrerequisite),
        None => return Err(TierError::MissingPrerequisite),
    })
}

pub fn compute_tier(signals: &TaskSignals) -> Result<TierDecision, TierError> {
    use ModelTier::*;
    let (mut tier, mut source) = match signals.phase {
        Phase::FinalizeReview => (Deep, TierSource::FinalizeReview),
        Phase::Implement => {
            let docs =
                !signals.files.is_empty() && signals.files.iter().all(|f| is_documentation(f));
            if docs && signals.files.len() <= 2 && !signals.has_protected_path {
                (Light, TierSource::Documentation)
            } else if signals.has_protected_path
                || signals.crate_roots.len() >= 2
                || signals.files.len() >= 4
                || signals.dependency_count >= 3
            {
                (Deep, TierSource::DeepTrigger)
            } else {
                (Standard, TierSource::Standard)
            }
        }
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
    if signals.prior_failed_attempts > 0 && tier != Deep {
        tier = (0..signals.prior_failed_attempts).fold(tier, |t, _| t.raise());
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
            files: vec!["src/lib.rs".into()],
            crate_roots: vec!["dispatch".into()],
            has_protected_path: false,
            dependency_count: 0,
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
    fn model_tier_implement_classifies_positive_documentation() {
        let mut s = signals(Phase::Implement);
        s.files = vec!["README.md".into()];
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Light);
    }
    #[test]
    fn model_tier_implement_deep_triggers_and_retry_cap() {
        let mut s = signals(Phase::Implement);
        s.files = vec!["a.rs".into(), "b.rs".into(), "c.rs".into(), "d.rs".into()];
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Deep);
        s.files.truncate(1);
        s.prior_failed_attempts = 9;
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Deep);
    }
    #[test]
    fn model_tier_test_escalates_from_accepted_prerequisite_and_audit_uses_unknown_mixed_mode_deep()
    {
        let mut s = signals(Phase::Test);
        s.implement = evidence(Some(ModelTier::Deep), "host-subagent");
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Standard);
        s.implement = evidence(None, "routed");
        let d = compute_tier(&s).unwrap();
        assert_eq!(d.tier, ModelTier::Standard);
        assert_eq!(d.source, TierSource::PrerequisiteTierUnknown);
        s.phase = Phase::Audit;
        s.test = evidence(None, "text");
        let d = compute_tier(&s).unwrap();
        assert_eq!(d.tier, ModelTier::Deep);
        assert_eq!(d.source, TierSource::PrerequisiteTierUnknown);
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
            Some(ModelTier::Light),
            Some("docs only"),
        )
        .unwrap();
        assert_eq!(d.source, TierSource::Override);
        assert_eq!(d.override_reason.as_deref(), Some("docs only"));
        let d = compute_tier(&s).unwrap();
        assert!(matches!(
            apply_override(d, Phase::Audit, Some(ModelTier::Light), Some("reason")),
            Err(TierError::InvalidOverride { .. })
        ));
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
    fn model_tier_protected_markdown_unknown_extension_and_empty_files_are_not_light() {
        let mut s = signals(Phase::Implement);
        s.files = vec!["a.md".into(), "b.md".into()];
        s.has_protected_path = true;
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Deep);
        s.has_protected_path = false;
        s.files = vec!["Makefile".into()];
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Standard);
        s.files = vec!["build.sh".into()];
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Standard);
        s.files.clear();
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Standard);
    }

    #[test]
    fn model_tier_prior_failure_raises_light_to_standard_and_tester_escalates() {
        let mut s = signals(Phase::Implement);
        s.files = vec!["README.md".into()];
        s.prior_failed_attempts = 1;
        assert_eq!(compute_tier(&s).unwrap().tier, ModelTier::Standard);
        let mut t = signals(Phase::Test);
        t.implement = evidence(Some(ModelTier::Light), "host-subagent");
        t.prior_failed_attempts = 1;
        assert_eq!(compute_tier(&t).unwrap().tier, ModelTier::Standard);
        t.implement = evidence(Some(ModelTier::Deep), "host-subagent");
        t.prior_failed_attempts = 3;
        assert_eq!(compute_tier(&t).unwrap().tier, ModelTier::Deep);
    }

    #[test]
    fn model_tier_host_subagent_evidence_without_tier_fails() {
        let mut s = signals(Phase::Test);
        s.implement = evidence(None, "host-subagent");
        assert_eq!(compute_tier(&s), Err(TierError::MissingPrerequisite));
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
                        source: TierSource::Standard,
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
                    source: TierSource::DeepTrigger,
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
