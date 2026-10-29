use dispatch::quota::{
    AccountIdentityCheck, AccountRef, Acquisition, AuthProfileId, BillingMode, BucketId,
    EvidenceField, Freshness, HostId, ModelScope, ObservationDigest, ProviderId, QueryMetadata,
    QuotaAmount, QuotaError, QuotaIdentity, QuotaLimit, QuotaObservation, QuotaReference,
    QuotaRuntime, QuotaSnapshot, QuotaUnit, QuotaWindow, RuntimeVersion, SnapshotId,
    UnlimitedBasis, UnlimitedClaim, Utilization, UtilizationDirection, UtilizationScale,
    WindowKind, WindowName, QUOTA_SCHEMA_VERSION,
};
use dispatch::usage::{Availability, CreditUnit, ObservationSource};
use rust_decimal::Decimal;
use std::str::FromStr;

fn dec(value: &str) -> Decimal {
    Decimal::from_str(value).unwrap()
}

fn identity(runtime: QuotaRuntime) -> QuotaIdentity {
    QuotaIdentity {
        runtime,
        runtime_version: Some(RuntimeVersion::new("1.0.98").unwrap()),
        provider: ProviderId::new("github").unwrap(),
        billing: BillingMode::Subscription,
        account: Some(AccountRef::new("acct-7f3a").unwrap()),
        identity_check: AccountIdentityCheck::Verified,
        host: Some(HostId::new("local").unwrap()),
        auth_profile: Some(AuthProfileId::new("default").unwrap()),
        model_scope: None,
    }
}

fn observation(identity: QuotaIdentity) -> QuotaObservation {
    QuotaObservation {
        schema_version: QUOTA_SCHEMA_VERSION,
        identity,
        bucket: BucketId::new("premium-requests").unwrap(),
        window: Availability::Reported {
            value: QuotaWindow {
                kind: WindowKind::Monthly,
                duration_minutes: Some(43_200),
                resets_at_unix_ms: Some(1_790_000_000_000),
            },
        },
        utilization: Availability::Unavailable {
            reason: "provider sent no percentage".to_owned(),
        },
        used: Availability::Reported {
            value: QuotaAmount::new(dec("12.5"), QuotaUnit::Credit(credit())).unwrap(),
        },
        remaining: Availability::Reported {
            value: QuotaAmount::new(dec("37.5"), QuotaUnit::Credit(credit())).unwrap(),
        },
        limit: Availability::Reported {
            value: QuotaLimit::Limited {
                amount: QuotaAmount::new(dec("50"), QuotaUnit::Credit(credit())).unwrap(),
            },
        },
        observed_at_unix_ms: Some(1_781_000_000_000),
        collected_at_unix_ms: 1_781_000_005_000,
        source: ObservationSource {
            collector: Some("copilot-account-rpc".to_owned()),
            source_version: Some("1.0.98".to_owned()),
            cursor: None,
        },
        query: Some(QueryMetadata {
            method: "account.getQuota".to_owned(),
            duration_ms: 412,
        }),
        acquisition: Acquisition::Queried,
        freshness: Freshness::Current,
    }
}

fn credit() -> CreditUnit {
    CreditUnit::new("plan-credit").unwrap()
}

#[test]
fn round_trip_keeps_fractional_used_and_remaining_values() {
    let observation = observation(identity(QuotaRuntime::Copilot));
    observation.validate().unwrap();

    let json = serde_json::to_string(&observation).unwrap();
    println!("{json}");
    assert!(json.contains(r#""value":"12.5""#));
    assert!(json.contains(r#""value":"37.5""#));

    let parsed: QuotaObservation = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, observation);
    parsed.validate().unwrap();
}

#[test]
fn utilization_keeps_raw_scale_and_direction() {
    let used = Utilization::new(
        dec("0.25"),
        UtilizationScale::Fraction,
        UtilizationDirection::Used,
    )
    .unwrap();
    let remaining = Utilization::new(
        dec("75"),
        UtilizationScale::Percent,
        UtilizationDirection::Remaining,
    )
    .unwrap();

    let json = serde_json::to_string(&(&used, &remaining)).unwrap();
    println!("{json}");
    assert!(json.contains(r#""scale":"fraction""#));
    assert!(json.contains(r#""scale":"percent""#));
    assert!(json.contains(r#""direction":"remaining""#));

    let parsed: (Utilization, Utilization) = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.0.value(), dec("0.25"));
    assert_eq!(parsed.0.direction(), UtilizationDirection::Used);
    assert_eq!(parsed.1.scale(), UtilizationScale::Percent);
    assert_eq!(parsed.1.value(), dec("75"));
}

#[test]
fn explicit_unlimited_round_trips_with_provider_evidence() {
    let mut observation = observation(identity(QuotaRuntime::Copilot));
    observation.limit = Availability::Reported {
        value: QuotaLimit::Unlimited {
            claim: UnlimitedClaim {
                basis: UnlimitedBasis::ProviderFlag,
                source_field: EvidenceField::new("quotaSnapshots.premium.unlimited").unwrap(),
            },
        },
    };
    observation.validate().unwrap();

    let json = serde_json::to_string(&observation).unwrap();
    println!("{json}");
    assert!(json.contains(r#""kind":"unlimited""#));

    let parsed: QuotaObservation = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, observation);
}

#[test]
fn unlimited_claim_without_evidence_field_is_rejected() {
    assert_eq!(
        EvidenceField::new(""),
        Err(QuotaError::EmptyIdentifier {
            kind: "evidence field"
        })
    );

    let json = r#"{"kind":"unlimited","claim":{"basis":"documented_sentinel","source_field":""}}"#;
    assert!(serde_json::from_str::<QuotaLimit>(json).is_err());

    let missing = r#"{"kind":"unlimited","claim":{"basis":"provider_flag"}}"#;
    assert!(serde_json::from_str::<QuotaLimit>(missing).is_err());
}

#[test]
fn missing_window_and_unknown_time_stay_explicit() {
    let mut observation = observation(identity(QuotaRuntime::Codex));
    observation.window = Availability::Unavailable {
        reason: "passive event covered only the weekly transition".to_owned(),
    };
    observation.observed_at_unix_ms = None;
    observation.freshness = Freshness::Unknown;
    observation.validate().unwrap();

    let json = serde_json::to_string(&observation).unwrap();
    println!("{json}");
    assert!(json.contains(r#""state":"unavailable""#));
    assert!(json.contains(r#""observed_at_unix_ms":null"#));

    let parsed: QuotaObservation = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.window, observation.window);
    assert_eq!(parsed.observed_at_unix_ms, None);
    assert_eq!(parsed.freshness, Freshness::Unknown);
}

#[test]
fn current_freshness_without_source_time_is_rejected() {
    let mut observation = observation(identity(QuotaRuntime::Claude));
    observation.observed_at_unix_ms = None;
    observation.freshness = Freshness::Current;

    assert_eq!(
        observation.validate(),
        Err(QuotaError::FreshnessWithoutSourceTime)
    );
}

#[test]
fn stale_values_keep_original_time_and_failed_acquisition() {
    let mut observation = observation(identity(QuotaRuntime::Copilot));
    observation.acquisition = Acquisition::Failed {
        reason: "refresh timed out after 5000 ms".to_owned(),
    };
    observation.freshness = Freshness::Stale;
    observation.collected_at_unix_ms = 1_781_900_000_000;
    observation.validate().unwrap();

    let json = serde_json::to_string(&observation).unwrap();
    println!("{json}");
    let parsed: QuotaObservation = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.observed_at_unix_ms, Some(1_781_000_000_000));
    assert_eq!(parsed.freshness, Freshness::Stale);
    assert!(matches!(parsed.acquisition, Acquisition::Failed { .. }));
    assert!(json.contains(r#""state":"failed""#));
    assert!(json.contains(r#""freshness":"stale""#));
}

#[test]
fn utilization_outside_its_scale_is_rejected() {
    assert!(Utilization::new(
        dec("1.01"),
        UtilizationScale::Fraction,
        UtilizationDirection::Used
    )
    .is_err());
    assert!(Utilization::new(
        dec("-0.1"),
        UtilizationScale::Fraction,
        UtilizationDirection::Used
    )
    .is_err());
    assert!(Utilization::new(
        dec("100.5"),
        UtilizationScale::Percent,
        UtilizationDirection::Used
    )
    .is_err());

    let parsed = serde_json::from_str::<Utilization>(
        r#"{"value":"1.5","scale":"fraction","direction":"used"}"#,
    );
    assert!(parsed.is_err());
}

#[test]
fn utilization_without_declared_scale_is_rejected_as_ambiguous() {
    let parsed = serde_json::from_str::<Utilization>(r#"{"value":"45","direction":"used"}"#);
    assert!(parsed.is_err());
}

#[test]
fn amounts_in_different_units_are_rejected_as_ambiguous() {
    let mut observation = observation(identity(QuotaRuntime::Antigravity));
    observation.limit = Availability::Reported {
        value: QuotaLimit::Limited {
            amount: QuotaAmount::new(dec("50"), QuotaUnit::Tokens).unwrap(),
        },
    };

    assert_eq!(
        observation.validate(),
        Err(QuotaError::AmbiguousUnit { field: "limit" })
    );
}

#[test]
fn negative_amounts_are_rejected() {
    assert_eq!(
        QuotaAmount::new(dec("-1"), QuotaUnit::Requests),
        Err(QuotaError::NegativeAmount(dec("-1")))
    );
}

#[test]
fn verified_identity_without_account_is_rejected() {
    let mut identity = identity(QuotaRuntime::OpenCode);
    identity.account = None;
    let observation = observation(identity);

    assert_eq!(
        observation.validate(),
        Err(QuotaError::VerifiedWithoutAccount)
    );
}

#[test]
fn future_schema_version_is_rejected() {
    let mut observation = observation(identity(QuotaRuntime::Codex));
    observation.schema_version = QUOTA_SCHEMA_VERSION + 1;

    assert_eq!(
        observation.validate(),
        Err(QuotaError::UnsupportedSchemaVersion {
            found: QUOTA_SCHEMA_VERSION + 1,
            supported: QUOTA_SCHEMA_VERSION,
        })
    );
}

#[test]
fn legacy_quota_defaults_to_unavailable_with_reason() {
    let legacy: QuotaSnapshot = serde_json::from_str("{}").unwrap();
    assert_eq!(legacy, QuotaSnapshot::default());

    let json = serde_json::to_string(&legacy).unwrap();
    println!("{json}");
    assert!(json.contains(r#""state":"unavailable""#));
    assert!(json.contains("legacy record predates quota collection"));
    legacy.validate().unwrap();
}

#[test]
fn snapshot_reference_round_trips() {
    let snapshot = QuotaSnapshot::referenced(QuotaReference {
        snapshot_id: SnapshotId::new("qs-0001").unwrap(),
        observation_digest: ObservationDigest::new("sha256:ab12").unwrap(),
    });

    let json = serde_json::to_string(&snapshot).unwrap();
    println!("{json}");
    let parsed: QuotaSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, snapshot);
}

#[test]
fn every_runtime_and_window_identity_round_trips() {
    let runtimes = [
        QuotaRuntime::Codex,
        QuotaRuntime::Claude,
        QuotaRuntime::Copilot,
        QuotaRuntime::Antigravity,
        QuotaRuntime::OpenCode,
    ];
    for runtime in runtimes {
        let json = serde_json::to_string(&runtime).unwrap();
        let parsed: QuotaRuntime = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, runtime);
    }

    let windows = [
        WindowKind::FiveHour,
        WindowKind::Weekly,
        WindowKind::Monthly,
        WindowKind::RequestRate,
        WindowKind::Named {
            name: WindowName::new("overage-daily").unwrap(),
        },
    ];
    for window in windows {
        let json = serde_json::to_string(&window).unwrap();
        let parsed: WindowKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, window);
    }
    assert_eq!(
        WindowName::new(""),
        Err(QuotaError::EmptyIdentifier {
            kind: "window name"
        })
    );
}

#[test]
fn model_and_host_scope_stay_optional() {
    let mut identity = identity(QuotaRuntime::Claude);
    identity.model_scope = Some(ModelScope::new("claude-opus-5-5").unwrap());
    identity.host = None;
    identity.auth_profile = None;

    let json = serde_json::to_string(&identity).unwrap();
    let parsed: QuotaIdentity = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, identity);
    assert!(json.contains(r#""host":null"#));
}
