use dispatch::usage::{MoneyUnit, UsageObservation};
use pipeline::usage::pricing::{
    price_snapshot, Estimate, PricedSnapshot, PricingError, PricingInput, RateSet,
};
use pipeline::usage::{digest_bytes, ObservationDigest, UsageSnapshot, RECORD_SCHEMA_VERSION};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const FIXTURES: &str = include_str!("fixtures/usage_rates.json");

#[derive(Deserialize)]
struct FixtureFile {
    rate_sets: BTreeMap<String, Value>,
    observations: BTreeMap<String, Value>,
}

fn fixture() -> FixtureFile {
    serde_json::from_str(FIXTURES).expect("fixture file is valid JSON")
}

fn rate_set(name: &str) -> RateSet {
    let file = fixture();
    let value = file
        .rate_sets
        .get(name)
        .unwrap_or_else(|| panic!("rate set `{name}` is missing"));
    let bytes = serde_json::to_vec(value).expect("rate set encodes");
    RateSet::parse(&bytes).expect("fixture rate set parses")
}

fn base_observation() -> Value {
    let mut file = fixture();
    file.observations
        .remove("cache_subset")
        .expect("cache_subset fixture exists")
}

fn observation(value: Value) -> UsageObservation {
    serde_json::from_value(value).expect("fixture observation deserializes")
}

fn with_cost(cost: Value) -> Value {
    let mut value = base_observation();
    value["cost"] = cost;
    value
}

fn input(observation_id: &str, observation: UsageObservation) -> PricingInput {
    PricingInput {
        observation_id: observation_id.to_owned(),
        provider: "codex".to_owned(),
        billing_mode: "standard".to_owned(),
        observation,
    }
}

fn factual(observation_ids: &[&str]) -> UsageSnapshot {
    UsageSnapshot {
        schema_version: RECORD_SCHEMA_VERSION,
        snapshot_id: "snapshot-1".to_owned(),
        run_id: "run-1".to_owned(),
        reconciliation_version: 1,
        rate_set_digest: None,
        observations: observation_ids
            .iter()
            .map(|id| ObservationDigest {
                observation_id: (*id).to_owned(),
                digest: digest_bytes(id.as_bytes()),
            })
            .collect(),
    }
}

fn price_one(
    observation_id: &str,
    observation: UsageObservation,
    rates: &RateSet,
) -> PricedSnapshot {
    price_snapshot(
        &factual(&[observation_id]),
        &[input(observation_id, observation)],
        rates,
    )
    .expect("pricing succeeds")
}

fn dec(text: &str) -> Decimal {
    text.parse().expect("decimal literal")
}

fn amount_of(estimate: &Estimate) -> Decimal {
    match estimate {
        Estimate::Known { amount } => amount.amount,
        Estimate::Unknown { reason } => panic!("expected a known estimate, got unknown: {reason}"),
    }
}

fn unit_of(estimate: &Estimate) -> String {
    match estimate {
        Estimate::Known { amount } => match &amount.unit {
            MoneyUnit::Currency(code) => code.as_str().to_owned(),
            MoneyUnit::Credit(name) => name.as_str().to_owned(),
        },
        Estimate::Unknown { reason } => panic!("expected a known estimate, got unknown: {reason}"),
    }
}

fn reason_of(estimate: &Estimate) -> &str {
    match estimate {
        Estimate::Unknown { reason } => reason,
        Estimate::Known { .. } => panic!("expected an unknown estimate"),
    }
}

#[test]
fn exact_cost_counts_cached_tokens_once() {
    let rates = rate_set("base");
    let snapshot = price_snapshot(
        &factual(&["obs-1", "obs-2"]),
        &[
            input("obs-1", observation(base_observation())),
            input("obs-2", observation(base_observation())),
        ],
        &rates,
    )
    .expect("pricing succeeds");

    // 600 uncached input at 2.5, 400 cached at 0.25, and 200 output at 10, each per million.
    let per_observation = &snapshot.observations[0].rate_estimate;
    assert_eq!(amount_of(per_observation), dec("0.0036"));
    assert_eq!(unit_of(per_observation), "USD");

    let total = snapshot.rate_total();
    assert_eq!(amount_of(&total), dec("0.0072"));
    println!(
        "probe usage_rates base: observation rate estimate {} {}, two-observation total {} {}",
        amount_of(per_observation),
        unit_of(per_observation),
        amount_of(&total),
        unit_of(&total)
    );
}

#[test]
fn changed_rates_append_a_new_snapshot_and_keep_the_old_one() {
    let base = rate_set("base");
    let changed = rate_set("changed_input");
    let original = price_one("obs-1", observation(base_observation()), &base);
    let repriced = price_one("obs-1", observation(base_observation()), &changed);

    assert_eq!(
        amount_of(&repriced.observations[0].rate_estimate),
        dec("0.0039")
    );
    assert_ne!(original.snapshot_id, repriced.snapshot_id);
    assert_ne!(original.rate_set_digest, repriced.rate_set_digest);
    assert_eq!(original.factual_snapshot_id, repriced.factual_snapshot_id);

    let replayed = price_one("obs-1", observation(base_observation()), &base);
    assert_eq!(replayed, original);
    assert_eq!(
        amount_of(&original.observations[0].rate_estimate),
        dec("0.0036")
    );
}

#[test]
fn missing_rate_is_unknown_while_provider_cost_stays_separate() {
    let rates = rate_set("missing_output");
    let observation = observation(with_cost(json!({
        "value": {
            "state": "reported",
            "value": { "amount": "0.0040", "unit": { "currency": "USD" }, "kind": "provider_reported" }
        },
        "semantics": "delta",
        "descendants": "own"
    })));
    let snapshot = price_one("obs-1", observation, &rates);
    let row = &snapshot.observations[0];

    assert!(reason_of(&row.rate_estimate).contains("no rate for output"));
    assert_eq!(amount_of(&row.provider_estimate), dec("0.0040"));
    assert!(matches!(snapshot.rate_total(), Estimate::Unknown { .. }));
}

#[test]
fn duplicate_rate_keys_are_unknown() {
    let rates = rate_set("duplicate_input");
    let snapshot = price_one("obs-1", observation(base_observation()), &rates);

    assert!(reason_of(&snapshot.observations[0].rate_estimate)
        .contains("duplicate or conflicting rates for input"));
}

#[test]
fn mixed_money_units_stay_unknown() {
    let rates = rate_set("mixed_units");
    let snapshot = price_one("obs-1", observation(base_observation()), &rates);

    assert_eq!(
        reason_of(&snapshot.observations[0].rate_estimate),
        "rates use mixed money units"
    );
}

#[test]
fn incompatible_request_unit_cannot_price_tokens() {
    let rates = rate_set("per_request_output");
    let snapshot = price_one("obs-1", observation(base_observation()), &rates);

    assert!(reason_of(&snapshot.observations[0].rate_estimate).contains("per request"));
}

#[test]
fn credits_and_confirmed_charges_stay_separate() {
    let rates = rate_set("base");
    let credit = observation(with_cost(json!({
        "value": {
            "state": "reported",
            "value": { "amount": "12", "unit": { "credit": "plan-credit" }, "kind": "provider_reported" }
        },
        "semantics": "delta",
        "descendants": "own"
    })));
    let credit_row = &price_one("obs-credit", credit, &rates).observations[0];

    assert_eq!(amount_of(&credit_row.credits), dec("12"));
    assert_eq!(unit_of(&credit_row.credits), "plan-credit");
    assert!(reason_of(&credit_row.provider_estimate).contains("no provider-reported currency"));
    assert_eq!(amount_of(&credit_row.rate_estimate), dec("0.0036"));

    let matched = observation(with_cost(json!({
        "value": {
            "state": "reported",
            "value": { "amount": "0.0036", "unit": { "currency": "USD" }, "kind": "confirmed_charge" }
        },
        "semantics": "delta",
        "descendants": "own"
    })));
    let matched_row = &price_one("obs-matched", matched, &rates).observations[0];
    assert_eq!(amount_of(&matched_row.confirmed_charge), dec("0.0036"));
    assert_eq!(unit_of(&matched_row.confirmed_charge), "USD");

    let scoped_out = observation(with_cost(json!({
        "value": {
            "state": "reported",
            "value": { "amount": "0.0036", "unit": { "currency": "USD" }, "kind": "confirmed_charge" }
        },
        "semantics": "delta",
        "descendants": "includes_descendants"
    })));
    let scoped_row = &price_one("obs-scoped", scoped_out, &rates).observations[0];
    assert!(reason_of(&scoped_row.confirmed_charge).contains("does not match"));
}

#[test]
fn unrecorded_observed_model_is_unknown() {
    let rates = rate_set("base");
    let mut value = base_observation();
    value["models"] = json!({});
    let snapshot = price_one("obs-1", observation(value), &rates);

    assert_eq!(
        reason_of(&snapshot.observations[0].rate_estimate),
        "observed model is not recorded"
    );
}

#[test]
fn cache_subset_larger_than_its_parent_is_unknown() {
    let rates = rate_set("base");
    let mut value = base_observation();
    value["tokens"]["cached_input"]["count"]["value"]["value"] = json!(1200);
    let snapshot = price_one("obs-1", observation(value), &rates);

    assert!(reason_of(&snapshot.observations[0].rate_estimate).contains("exceeds its parent"));
}

#[test]
fn rate_set_parsing_is_strict_and_digest_covers_exact_bytes() {
    let compact = br#"{"schema_version":1,"rate_set_id":"r","rates":[]}"#;
    let parsed = RateSet::parse(compact).expect("minimal rate set parses");
    assert_eq!(parsed.digest(), digest_bytes(compact));
    assert_eq!(parsed.rate_set_id(), "r");

    let spaced = br#"{ "schema_version":1,"rate_set_id":"r","rates":[] }"#;
    let respaced = RateSet::parse(spaced).expect("respaced rate set parses");
    assert_ne!(respaced.digest(), parsed.digest());

    let unknown_field = br#"{"schema_version":1,"rate_set_id":"r","rates":[],"extra":true}"#;
    assert!(matches!(
        RateSet::parse(unknown_field),
        Err(PricingError::InvalidRateSet(_))
    ));

    let future = br#"{"schema_version":2,"rate_set_id":"r","rates":[]}"#;
    assert_eq!(
        RateSet::parse(future),
        Err(PricingError::UnsupportedRateSetVersion {
            found: 2,
            supported: 1
        })
    );

    let numeric_price = br#"{"schema_version":1,"rate_set_id":"r","rates":[{"provider":"p","model":"m","billing_mode":"standard","category":"input","unit":"per_token","price":2.5,"money_unit":{"currency":"USD"}}]}"#;
    assert!(matches!(
        RateSet::parse(numeric_price),
        Err(PricingError::InvalidRateSet(_))
    ));

    let negative_price = br#"{"schema_version":1,"rate_set_id":"r","rates":[{"provider":"p","model":"m","billing_mode":"standard","category":"input","unit":"per_token","price":"-1","money_unit":{"currency":"USD"}}]}"#;
    assert!(matches!(
        RateSet::parse(negative_price),
        Err(PricingError::InvalidRateSet(_))
    ));
}

#[test]
fn observations_must_match_the_factual_snapshot() {
    let rates = rate_set("base");
    let result = price_snapshot(
        &factual(&["obs-1", "obs-2"]),
        &[input("obs-1", observation(base_observation()))],
        &rates,
    );

    assert_eq!(result, Err(PricingError::ObservationMismatch));
}
