use dispatch::usage::{
    Availability, CounterRelation, CounterSemantics, CreditUnit, CurrencyCode, DescendantScope,
    Metric, MoneyAmount, MoneyKind, MoneyUnit, ObservationWindow, TokenCount, UsageCategory,
    UsageError, UsageObservation, MAX_TOKEN_COUNT, USAGE_SCHEMA_VERSION,
};
use rust_decimal::Decimal;
use std::str::FromStr;

fn usd(amount: &str, kind: MoneyKind) -> MoneyAmount {
    MoneyAmount {
        amount: Decimal::from_str(amount).unwrap(),
        unit: MoneyUnit::Currency(CurrencyCode::new("USD").unwrap()),
        kind,
    }
}

#[test]
fn round_trip_preserves_known_zero_reasons_and_subsets() {
    let mut observation = UsageObservation::new();
    observation.identity.run_id = Some("run-1".to_owned());
    observation.tokens.input.count = Metric {
        value: Availability::Reported {
            value: TokenCount::new(0).unwrap(),
        },
        semantics: CounterSemantics::Delta,
        descendants: DescendantScope::Own,
        window: Some(ObservationWindow::new(1_000, 2_000).unwrap()),
    };
    observation.tokens.cached_input.count.value = Availability::Reported {
        value: TokenCount::new(40).unwrap(),
    };
    observation.tokens.cached_input.relation = CounterRelation::SubsetOf {
        category: UsageCategory::Input,
    };
    observation.tokens.output.count.value = Availability::Unsupported {
        reason: "executor reports no output counter".to_owned(),
    };
    observation.tokens.reasoning.count.value = Availability::Partial {
        value: TokenCount::new(7).unwrap(),
        reason: "final usage event missing".to_owned(),
    };
    observation.tokens.reasoning.relation = CounterRelation::SubsetOf {
        category: UsageCategory::Output,
    };
    observation.cost = Metric {
        value: Availability::Reported {
            value: usd("0.10", MoneyKind::ProviderReported),
        },
        ..Metric::default()
    };

    let json = serde_json::to_string(&observation).unwrap();
    println!("{json}");
    assert!(json.contains(r#""state":"reported","value":0"#));
    assert!(json.contains(r#""relation":{"kind":"subset_of","category":"input"}"#));

    let parsed: UsageObservation = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, observation);
    assert_eq!(
        parsed.tokens.input.count.value,
        Availability::Reported {
            value: TokenCount::new(0).unwrap()
        }
    );
    assert!(matches!(
        parsed.tokens.output.count.value,
        Availability::Unsupported { .. }
    ));
    assert!(parsed.validate().is_ok());
}

#[test]
fn legacy_record_without_usage_stays_unavailable() {
    let observation: UsageObservation =
        serde_json::from_str(r#"{"identity":{"run_id":"run-legacy"}}"#).unwrap();
    println!("legacy input = {:?}", observation.tokens.input.count.value);

    assert_eq!(observation.schema_version, 0);
    assert_eq!(observation.identity.run_id.as_deref(), Some("run-legacy"));
    assert!(matches!(
        observation.tokens.input.count.value,
        Availability::Unavailable { .. }
    ));
    assert!(matches!(
        observation.cost.value,
        Availability::Unavailable { .. }
    ));
    assert_eq!(observation.tokens.input.relation, CounterRelation::Unknown);
    assert_eq!(
        observation.tokens.input.count.semantics,
        CounterSemantics::Unknown
    );
    assert_eq!(
        observation.tokens.input.count.descendants,
        DescendantScope::Unknown
    );
    assert!(observation.validate().is_ok());
}

#[test]
fn counter_without_scope_fields_defaults_to_unknown() {
    let json = r#"{"tokens":{"input":{"count":{"value":{"state":"reported","value":5}}}}}"#;
    let observation: UsageObservation = serde_json::from_str(json).unwrap();

    assert_eq!(
        observation.tokens.input.count.value,
        Availability::Reported {
            value: TokenCount::new(5).unwrap()
        }
    );
    assert_eq!(
        observation.tokens.input.count.semantics,
        CounterSemantics::Unknown
    );
    assert_eq!(observation.tokens.input.count.window, None);
}

#[test]
fn decimal_precision_and_scale_survive_round_trip() {
    let amount = usd("0.10000000000000000001", MoneyKind::Estimated);
    let json = serde_json::to_string(&amount).unwrap();
    println!("{json}");
    assert!(json.contains(r#""amount":"0.10000000000000000001""#));

    let parsed: MoneyAmount = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, amount);

    let sum = parsed.try_add(&usd("0.2", MoneyKind::Estimated)).unwrap();
    println!("sum = {}", sum.amount);
    assert_eq!(sum.amount.to_string(), "0.30000000000000000001");

    let scaled: MoneyAmount = serde_json::from_str(
        r#"{"amount":"1.50","unit":{"currency":"USD"},"kind":"confirmed_charge"}"#,
    )
    .unwrap();
    assert_eq!(scaled.amount.to_string(), "1.50");
}

#[test]
fn negative_counter_fails_explicitly() {
    let error =
        serde_json::from_str::<Availability<TokenCount>>(r#"{"state":"reported","value":-1}"#)
            .unwrap_err();
    println!("negative counter: {error}");
    assert!(error.is_data());
}

#[test]
fn counter_above_maximum_is_rejected() {
    let too_large = MAX_TOKEN_COUNT + 1;
    assert_eq!(
        TokenCount::new(too_large),
        Err(UsageError::CounterOutOfRange { value: too_large })
    );
    assert_eq!(
        TokenCount::new(MAX_TOKEN_COUNT).unwrap().get(),
        MAX_TOKEN_COUNT
    );

    let json = format!(r#"{{"state":"reported","value":{too_large}}}"#);
    assert!(serde_json::from_str::<Availability<TokenCount>>(&json).is_err());
}

#[test]
fn token_addition_rejects_sum_above_maximum() {
    let max = TokenCount::new(MAX_TOKEN_COUNT).unwrap();
    let one = TokenCount::new(1).unwrap();
    assert_eq!(
        max.try_add(one),
        Err(UsageError::CounterOutOfRange {
            value: MAX_TOKEN_COUNT + 1
        })
    );
    assert_eq!(TokenCount::new(40).unwrap().try_add(one).unwrap().get(), 41);
}

#[test]
fn money_addition_rejects_decimal_overflow_and_mismatch() {
    let max = usd("79228162514264337593543950335", MoneyKind::ConfirmedCharge);
    assert_eq!(
        max.try_add(&usd("1", MoneyKind::ConfirmedCharge)),
        Err(UsageError::MoneyOverflow)
    );
    assert_eq!(
        max.try_add(&usd("1", MoneyKind::Estimated)),
        Err(UsageError::MoneyMismatch)
    );
}

#[test]
fn invalid_units_fail_explicitly() {
    assert_eq!(
        CurrencyCode::new("usd"),
        Err(UsageError::InvalidCurrencyCode("usd".to_owned()))
    );
    assert!(CurrencyCode::new("USDX").is_err());
    assert_eq!(CreditUnit::new(""), Err(UsageError::EmptyCreditUnit));

    let bad_currency = r#"{"amount":"1","unit":{"currency":"us"},"kind":"estimated"}"#;
    assert!(serde_json::from_str::<MoneyAmount>(bad_currency).is_err());

    let credit: MoneyAmount = serde_json::from_str(
        r#"{"amount":"12.5","unit":{"credit":"copilot-premium-request"},"kind":"provider_reported"}"#,
    )
    .unwrap();
    assert_eq!(
        credit.unit,
        MoneyUnit::Credit(CreditUnit::new("copilot-premium-request").unwrap())
    );
}

#[test]
fn inverted_window_is_rejected() {
    assert_eq!(
        ObservationWindow::new(5, 4),
        Err(UsageError::InvalidWindow { start: 5, end: 4 })
    );
    assert!(
        serde_json::from_str::<ObservationWindow>(r#"{"start_unix_ms":5,"end_unix_ms":4}"#)
            .is_err()
    );
    assert_eq!(ObservationWindow::new(4, 4).unwrap().end_unix_ms(), 4);
}

#[test]
fn newer_schema_version_is_rejected() {
    let newer = USAGE_SCHEMA_VERSION + 1;
    let observation: UsageObservation =
        serde_json::from_str(&format!(r#"{{"schema_version":{newer}}}"#)).unwrap();
    assert_eq!(
        observation.validate(),
        Err(UsageError::UnsupportedSchemaVersion {
            found: newer,
            supported: USAGE_SCHEMA_VERSION,
        })
    );
}
