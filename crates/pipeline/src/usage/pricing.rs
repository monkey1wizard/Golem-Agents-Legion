//! Priced snapshots from versioned owner-supplied rate sets.
//!
//! A rate set is identified by the digest of its exact bytes. Each observation
//! is priced for one provider, observed model, and billing mode. Every estimate
//! is either known or unknown, and an unknown estimate keeps its reason, so a
//! missing rate never reads as zero. Pricing only reads its inputs, so a changed
//! rate set yields a new snapshot and leaves the earlier snapshot as it was.

use crate::usage::reconcile::TOKEN_CATEGORIES;
use crate::usage::{digest_bytes, UsageSnapshot};
use dispatch::usage::{
    Availability, CounterRelation, CounterSemantics, DescendantScope, Metric, MoneyAmount,
    MoneyKind, MoneyUnit, TokenCount, TokenMetric, TokenUsage, UsageCategory, UsageObservation,
};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Version of these pricing rules. Priced snapshots record it.
pub const PRICING_VERSION: u32 = 1;

/// Schema version that a rate set file must declare.
pub const RATE_SET_SCHEMA_VERSION: u32 = 1;

/// Decimal places kept on each priced component. Components round half away from zero.
pub const COST_SCALE: u32 = 10;

const TOKENS_PER_MILLION: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PricingError {
    #[error("rate set is invalid: {0}")]
    InvalidRateSet(String),
    #[error("rate set schema version {found} is not supported, expected {supported}")]
    UnsupportedRateSetVersion { found: u32, supported: u32 },
    #[error("priced observations do not match the observations of the factual snapshot")]
    ObservationMismatch,
}

/// Unit that a price applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateUnit {
    PerToken,
    PerMillionTokens,
    /// A request price. It cannot price token counters.
    PerRequest,
}

/// One price for one provider, model, billing mode, and token category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateEntry {
    pub provider: String,
    pub model: String,
    pub billing_mode: String,
    pub category: UsageCategory,
    pub unit: RateUnit,
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    pub money_unit: MoneyUnit,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RateSetFile {
    schema_version: u32,
    rate_set_id: String,
    rates: Vec<RateEntry>,
}

/// Strictly parsed rate set with the digest of the bytes it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateSet {
    rate_set_id: String,
    digest: String,
    rates: Vec<RateEntry>,
}

impl RateSet {
    /// Parses one owner-supplied rate set. The digest covers the exact input bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, PricingError> {
        let file: RateSetFile = serde_json::from_slice(bytes)
            .map_err(|error| PricingError::InvalidRateSet(error.to_string()))?;
        if file.schema_version != RATE_SET_SCHEMA_VERSION {
            return Err(PricingError::UnsupportedRateSetVersion {
                found: file.schema_version,
                supported: RATE_SET_SCHEMA_VERSION,
            });
        }
        if file.rate_set_id.trim().is_empty() {
            return Err(PricingError::InvalidRateSet(
                "rate set id must not be empty".to_owned(),
            ));
        }
        if let Some(rate) = file.rates.iter().find(|rate| rate.price < Decimal::ZERO) {
            return Err(PricingError::InvalidRateSet(format!(
                "price for `{}` is negative",
                rate.model
            )));
        }
        Ok(Self {
            rate_set_id: file.rate_set_id,
            digest: digest_bytes(bytes),
            rates: file.rates,
        })
    }

    pub fn rate_set_id(&self) -> &str {
        &self.rate_set_id
    }

    /// Lowercase hex SHA-256 of the exact bytes this rate set was parsed from.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Finds the entry for one key. Two or more matches make the key ambiguous.
    fn find(&self, key: &PriceKey<'_>, category: UsageCategory) -> Lookup<'_> {
        let mut matches = self.rates.iter().filter(|rate| {
            rate.category == category
                && rate.provider == key.provider
                && rate.model == key.model
                && rate.billing_mode == key.billing_mode
        });
        match (matches.next(), matches.next()) {
            (None, _) => Lookup::Missing,
            (Some(rate), None) => Lookup::Found(rate),
            (Some(_), Some(_)) => Lookup::Ambiguous,
        }
    }
}

struct PriceKey<'a> {
    provider: &'a str,
    model: &'a str,
    billing_mode: &'a str,
}

enum Lookup<'a> {
    Found(&'a RateEntry),
    Missing,
    Ambiguous,
}

/// Money estimate that is either known or unknown with its reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Estimate {
    Known { amount: MoneyAmount },
    Unknown { reason: String },
}

impl Estimate {
    fn unknown(reason: impl Into<String>) -> Self {
        Self::Unknown {
            reason: reason.into(),
        }
    }
}

/// One observation with the caller-supplied basis that selects its rates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricingInput {
    pub observation_id: String,
    pub provider: String,
    pub billing_mode: String,
    pub observation: UsageObservation,
}

/// Estimates for one observation. Each basis stays separate from the others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricedObservation {
    pub observation_id: String,
    /// Estimate computed from the rate set. Cache subsets are billed once.
    pub rate_estimate: Estimate,
    /// Provider-reported cost in a currency unit.
    pub provider_estimate: Estimate,
    /// Provider-reported plan credits. Credits never convert to currency.
    pub credits: Estimate,
    /// Confirmed charge whose scope matches the token counters.
    pub confirmed_charge: Estimate,
}

/// Priced view of one factual snapshot under one rate set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricedSnapshot {
    pub pricing_version: u32,
    /// Digest of the pricing version, the factual snapshot, and the rate set digest.
    pub snapshot_id: String,
    pub factual_snapshot_id: String,
    pub rate_set_id: String,
    pub rate_set_digest: String,
    /// Sorted by observation identifier.
    pub observations: Vec<PricedObservation>,
}

impl PricedSnapshot {
    /// Sum of the rate estimates. One unknown observation makes the total unknown.
    pub fn rate_total(&self) -> Estimate {
        let mut parts = Vec::new();
        for observation in &self.observations {
            match &observation.rate_estimate {
                Estimate::Known { amount } => parts.push((amount.unit.clone(), amount.amount)),
                Estimate::Unknown { reason } => return Estimate::unknown(reason.clone()),
            }
        }
        sum_parts(parts)
    }
}

/// Prices every observation of one factual snapshot under one rate set.
///
/// The inputs must name exactly the observations of the factual snapshot.
pub fn price_snapshot(
    factual: &UsageSnapshot,
    inputs: &[PricingInput],
    rates: &RateSet,
) -> Result<PricedSnapshot, PricingError> {
    let mut expected: Vec<&str> = factual
        .observations
        .iter()
        .map(|digest| digest.observation_id.as_str())
        .collect();
    let mut given: Vec<&str> = inputs
        .iter()
        .map(|input| input.observation_id.as_str())
        .collect();
    expected.sort_unstable();
    given.sort_unstable();
    if expected != given {
        return Err(PricingError::ObservationMismatch);
    }

    let mut observations: Vec<PricedObservation> = inputs
        .iter()
        .map(|input| price_observation(input, rates))
        .collect();
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));

    Ok(PricedSnapshot {
        pricing_version: PRICING_VERSION,
        snapshot_id: digest_bytes(
            format!(
                "{PRICING_VERSION}\n{}\n{}",
                factual.snapshot_id, rates.digest
            )
            .as_bytes(),
        ),
        factual_snapshot_id: factual.snapshot_id.clone(),
        rate_set_id: rates.rate_set_id.clone(),
        rate_set_digest: rates.digest.clone(),
        observations,
    })
}

fn price_observation(input: &PricingInput, rates: &RateSet) -> PricedObservation {
    let (provider_estimate, credits, confirmed_charge) = cost_buckets(&input.observation);
    PricedObservation {
        observation_id: input.observation_id.clone(),
        rate_estimate: rate_estimate(input, rates),
        provider_estimate,
        credits,
        confirmed_charge,
    }
}

fn rate_estimate(input: &PricingInput, rates: &RateSet) -> Estimate {
    let Some(model) = input.observation.models.observed.as_deref() else {
        return Estimate::unknown("observed model is not recorded");
    };
    let key = PriceKey {
        provider: &input.provider,
        model,
        billing_mode: &input.billing_mode,
    };
    let mut parts = Vec::new();
    for (category, billable) in TOKEN_CATEGORIES
        .iter()
        .zip(billable_tokens(&input.observation.tokens))
    {
        let count = match billable {
            Ok(count) => count,
            Err(reason) => return Estimate::unknown(reason),
        };
        match price_category(rates, &key, *category, count) {
            Ok(Some(part)) => parts.push(part),
            Ok(None) => {}
            Err(reason) => return Estimate::unknown(reason),
        }
    }
    sum_parts(parts)
}

/// Price of one billable quantity. Zero needs no rate, so it never adds a gap.
fn price_category(
    rates: &RateSet,
    key: &PriceKey<'_>,
    category: UsageCategory,
    count: u64,
) -> Result<Option<(MoneyUnit, Decimal)>, String> {
    if count == 0 {
        return Ok(None);
    }
    let name = category_name(category);
    let rate = match rates.find(key, category) {
        Lookup::Found(rate) => rate,
        Lookup::Missing => {
            return Err(format!(
                "no rate for {name} on {} {} {}",
                key.provider, key.model, key.billing_mode
            ))
        }
        Lookup::Ambiguous => {
            return Err(format!(
                "duplicate or conflicting rates for {name} on {} {} {}",
                key.provider, key.model, key.billing_mode
            ))
        }
    };
    let product = Decimal::from(count)
        .checked_mul(rate.price)
        .ok_or_else(|| "rate arithmetic overflowed".to_owned())?;
    let exact = match rate.unit {
        RateUnit::PerToken => product,
        RateUnit::PerMillionTokens => product
            .checked_div(Decimal::from(TOKENS_PER_MILLION))
            .ok_or_else(|| "rate arithmetic overflowed".to_owned())?,
        RateUnit::PerRequest => {
            return Err(format!(
                "rate for {name} is per request and cannot price tokens"
            ))
        }
    };
    Ok(Some((
        rate.money_unit.clone(),
        exact.round_dp_with_strategy(COST_SCALE, RoundingStrategy::MidpointAwayFromZero),
    )))
}

/// Adds priced parts that share one money unit. Mixed units stay unknown.
fn sum_parts(parts: Vec<(MoneyUnit, Decimal)>) -> Estimate {
    let Some(unit) = parts.first().map(|(unit, _)| unit.clone()) else {
        return Estimate::unknown("no billable tokens were priced");
    };
    if parts.iter().any(|(other, _)| *other != unit) {
        return Estimate::unknown("rates use mixed money units");
    }
    let Some(total) = parts
        .iter()
        .try_fold(Decimal::ZERO, |sum, (_, amount)| sum.checked_add(*amount))
    else {
        return Estimate::unknown("rate total overflowed");
    };
    Estimate::Known {
        amount: MoneyAmount {
            amount: total,
            unit,
            kind: MoneyKind::Estimated,
        },
    }
}

/// Billable quantity per token category, in [`TOKEN_CATEGORIES`] order.
///
/// A cache subset is billed at its own rate and removed from its parent counter
/// once. Without that removal, the same tokens would be billed twice.
fn billable_tokens(tokens: &TokenUsage) -> Vec<Billable> {
    let counts: Vec<Billable> = TOKEN_CATEGORIES
        .iter()
        .map(|&category| read_count(&metric_of(tokens, category).count))
        .collect();
    let mut billable = counts.clone();
    for (index, &category) in TOKEN_CATEGORIES.iter().enumerate() {
        match (&metric_of(tokens, category).relation, &counts[index]) {
            (CounterRelation::Exclusive, _) => {}
            (CounterRelation::Unknown, Ok(count)) if *count > 0 => {
                billable[index] = Err("counter relation is not recorded".to_owned());
            }
            (CounterRelation::Unknown, _) => {}
            (CounterRelation::SubsetOf { category: parent }, subset) => {
                let Some(parent_index) = TOKEN_CATEGORIES
                    .iter()
                    .position(|candidate| candidate == parent)
                else {
                    billable[index] = Err("cache subset names an unknown parent".to_owned());
                    continue;
                };
                if parent_index == index {
                    billable[index] = Err("a counter cannot be a subset of itself".to_owned());
                    continue;
                }
                let reduced = match (&billable[parent_index], subset) {
                    (Ok(base), Ok(part)) => base
                        .checked_sub(*part)
                        .ok_or_else(|| "cached subset exceeds its parent counter".to_owned()),
                    (Err(reason), _) | (Ok(_), Err(reason)) => Err(reason.clone()),
                };
                billable[parent_index] = reduced;
            }
        }
    }
    billable
}

type Billable = Result<u64, String>;

fn read_count(metric: &Metric<TokenCount>) -> Billable {
    match &metric.value {
        Availability::Reported { value } if metric.semantics == CounterSemantics::Delta => {
            Ok(value.get())
        }
        Availability::Reported { .. } => Err("counter is not a per-observation delta".to_owned()),
        Availability::Partial { reason, .. }
        | Availability::Unsupported { reason }
        | Availability::Unavailable { reason }
        | Availability::Malformed { reason } => Err(reason.clone()),
    }
}

/// Splits the observation cost into provider, credit, and confirmed-charge estimates.
fn cost_buckets(observation: &UsageObservation) -> (Estimate, Estimate, Estimate) {
    let amount = match &observation.cost.value {
        Availability::Reported { value } => value,
        Availability::Partial { reason, .. }
        | Availability::Unsupported { reason }
        | Availability::Unavailable { reason }
        | Availability::Malformed { reason } => {
            return (
                Estimate::unknown(reason.as_str()),
                Estimate::unknown(reason.as_str()),
                Estimate::unknown(reason.as_str()),
            )
        }
    };
    let provider_reported = amount.kind == MoneyKind::ProviderReported;
    let is_currency = matches!(amount.unit, MoneyUnit::Currency(_));
    let is_credit = matches!(amount.unit, MoneyUnit::Credit(_));
    let scope_matches = observation.cost.descendants != DescendantScope::Unknown
        && token_scope(&observation.tokens) == Some(observation.cost.descendants);

    let provider = if provider_reported && is_currency {
        known(amount)
    } else {
        Estimate::unknown("no provider-reported currency cost")
    };
    let credits = if provider_reported && is_credit {
        known(amount)
    } else {
        Estimate::unknown("no provider-reported credit cost")
    };
    let confirmed = match amount.kind {
        MoneyKind::ConfirmedCharge if scope_matches => known(amount),
        MoneyKind::ConfirmedCharge => {
            Estimate::unknown("confirmed charge scope does not match the token counters")
        }
        MoneyKind::ProviderReported | MoneyKind::Estimated => {
            Estimate::unknown("no confirmed charge recorded")
        }
    };
    (provider, credits, confirmed)
}

fn known(amount: &MoneyAmount) -> Estimate {
    Estimate::Known {
        amount: amount.clone(),
    }
}

/// Descendant scope shared by every token counter, or `None` when they differ.
fn token_scope(tokens: &TokenUsage) -> Option<DescendantScope> {
    let mut scopes = TOKEN_CATEGORIES
        .iter()
        .map(|&category| metric_of(tokens, category).count.descendants);
    let first = scopes.next()?;
    scopes.all(|scope| scope == first).then_some(first)
}

fn metric_of(tokens: &TokenUsage, category: UsageCategory) -> &TokenMetric {
    match category {
        UsageCategory::Input => &tokens.input,
        UsageCategory::Output => &tokens.output,
        UsageCategory::CachedInput => &tokens.cached_input,
        UsageCategory::CacheWrite => &tokens.cache_write,
        UsageCategory::Reasoning => &tokens.reasoning,
        UsageCategory::Other => &tokens.other,
    }
}

fn category_name(category: UsageCategory) -> &'static str {
    match category {
        UsageCategory::Input => "input",
        UsageCategory::Output => "output",
        UsageCategory::CachedInput => "cached_input",
        UsageCategory::CacheWrite => "cache_write",
        UsageCategory::Reasoning => "reasoning",
        UsageCategory::Other => "other",
    }
}
