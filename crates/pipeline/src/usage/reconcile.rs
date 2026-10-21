//! Deterministic reconciliation of usage observations into subtotals.
//!
//! Every observation contributes one row per metric, even when the row adds
//! nothing to a total. A row is counted only when its value is known and adds
//! safely. Every other row keeps the reason it was left out. Each subtotal
//! reports its coverage and the cutoff it was computed at, so a partial total
//! never reads as complete.

use crate::usage::{digest_bytes, ObservationDigest, UsageSnapshot, RECORD_SCHEMA_VERSION};
use dispatch::usage::{
    Availability, CounterRelation, CounterSemantics, DescendantScope, Metric, MoneyAmount,
    MoneyKind, MoneyUnit, ObservationWindow, TokenCount, TokenMetric, TokenUsage, UsageCategory,
    UsageObservation,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use thiserror::Error;

/// Version of these reconciliation rules. Snapshots record it.
pub const RECONCILIATION_VERSION: u32 = 1;

/// Token categories in the order their subtotals are reported.
pub const TOKEN_CATEGORIES: [UsageCategory; 6] = [
    UsageCategory::Input,
    UsageCategory::Output,
    UsageCategory::CachedInput,
    UsageCategory::CacheWrite,
    UsageCategory::Reasoning,
    UsageCategory::Other,
];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReconcileError {
    #[error("observation `{0}` appears more than once")]
    DuplicateObservation(String),
    #[error("interval for attempt `{attempt_id}` is invalid: {reason}")]
    InvalidInterval { attempt_id: String, reason: String },
    #[error("a subtotal exceeds the supported range")]
    TotalOverflow,
    #[error("money subtotal failed: {0}")]
    Money(String),
    #[error("observation cannot be encoded for its digest: {0}")]
    Encode(String),
}

/// One observation with the identifier that snapshots and assessments cite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationRecord {
    pub observation_id: String,
    pub observation: UsageObservation,
}

/// Clock that stamped an elapsed interval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClockDomain {
    /// The controller clock. Its intervals can be aligned for wall time.
    Controller,
    /// A remote clock. Its durations are summed, but its instants cannot be aligned.
    Remote { host: String },
}

/// Controller-observed execution interval of one attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElapsedInterval {
    pub attempt_id: String,
    pub start_unix_ms: u64,
    pub end_unix_ms: u64,
    pub clock: ClockDomain,
}

/// Everything one reconciliation reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileInput {
    pub run_id: String,
    pub cutoff_unix_ms: u64,
    pub observations: Vec<ObservationRecord>,
    pub intervals: Vec<ElapsedInterval>,
}

/// Metric that a contribution row belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKey {
    Tokens(UsageCategory),
    Cost,
}

impl MetricKey {
    fn rank(self) -> usize {
        match self {
            Self::Tokens(category) => category_index(category),
            Self::Cost => TOKEN_CATEGORIES.len(),
        }
    }
}

/// Quantity that a row reports. Token counts and money amounts both fit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Amount {
    Tokens(u64),
    Money(MoneyAmount),
}

/// How a cumulative reading or a delta was turned into a contributed quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    Delta,
    CumulativeDelta,
}

/// Whether a row enters a total, and why it does not when it does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Treatment {
    /// The row enters the total.
    Included,
    /// The row is covered by another row or by a rule, so it is not added again.
    Excluded(String),
    /// The value is known, but it cannot be added safely.
    NonAdditive(String),
    /// The source reported no known value. The reason is the source's reason.
    Unknown(String),
}

impl Treatment {
    fn gap_reason(&self) -> Option<&str> {
        match self {
            Self::Unknown(reason) | Self::NonAdditive(reason) => Some(reason.as_str()),
            Self::Included | Self::Excluded(_) => None,
        }
    }
}

/// Task that an allocated row belongs to. Task identifiers repeat across plans.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskKey {
    pub plan_id: Option<String>,
    pub task_id: String,
}

/// Where a row is counted. Rows on a shared session stay unallocated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Allocation {
    Task(TaskKey),
    Unallocated(String),
}

/// One observation metric with its treatment and its quantities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contribution {
    pub observation_id: String,
    pub metric: MetricKey,
    /// Raw reported value. `None` when the source reported no known value.
    pub reported: Option<Amount>,
    /// Quantity this row adds to a total. `None` unless the row is included.
    pub contributed: Option<Amount>,
    pub basis: Option<Basis>,
    pub treatment: Treatment,
    pub allocation: Allocation,
}

/// Counts of rows behind one subtotal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Coverage {
    pub included: usize,
    pub excluded: usize,
    pub non_additive: usize,
    pub unknown: usize,
    pub unallocated: usize,
}

impl Coverage {
    fn of(rows: &[&Contribution]) -> Self {
        let mut coverage = Self::default();
        for row in rows {
            match row.treatment {
                Treatment::Included => coverage.included += 1,
                Treatment::Excluded(_) => coverage.excluded += 1,
                Treatment::NonAdditive(_) => coverage.non_additive += 1,
                Treatment::Unknown(_) => coverage.unknown += 1,
            }
            if matches!(row.allocation, Allocation::Unallocated(_)) {
                coverage.unallocated += 1;
            }
        }
        coverage
    }
}

/// Total of one scope, with the coverage and cutoff it was computed under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subtotal<V> {
    /// `None` when no additive row exists. It is never zero for absent data.
    pub total: Option<V>,
    /// `true` only when the total exists and no row was unknown or non-additive.
    pub complete: bool,
    /// The first reason a row is unknown or non-additive, or why no total exists.
    pub reason: Option<String>,
    pub coverage: Coverage,
    pub cutoff_unix_ms: u64,
}

impl<V> Subtotal<V> {
    fn new(total: Option<V>, rows: &[&Contribution], cutoff_unix_ms: u64) -> Self {
        let coverage = Coverage::of(rows);
        let gap = rows
            .iter()
            .find_map(|row| row.treatment.gap_reason().map(str::to_owned));
        let reason = gap.or_else(|| {
            total
                .is_none()
                .then(|| "no additive observations".to_owned())
        });
        let complete = total.is_some() && coverage.unknown == 0 && coverage.non_additive == 0;
        Self {
            total,
            complete,
            reason,
            coverage,
            cutoff_unix_ms,
        }
    }
}

/// Money subtotal for one unit and kind. Rows of other units never mix into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostSubtotal {
    /// `None` when no row reported a known amount.
    pub unit: Option<MoneyUnit>,
    pub kind: Option<MoneyKind>,
    pub subtotal: Subtotal<MoneyAmount>,
}

/// Subtotals of one scope for every token category and every money basis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Totals {
    /// Token subtotals in the order of [`TOKEN_CATEGORIES`].
    pub tokens: Vec<Subtotal<u64>>,
    pub cost: Vec<CostSubtotal>,
}

impl Totals {
    pub fn token(&self, category: UsageCategory) -> &Subtotal<u64> {
        &self.tokens[category_index(category)]
    }
}

/// Union and sum of elapsed time for the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElapsedSummary {
    pub cutoff_unix_ms: u64,
    pub intervals_in_scope: usize,
    pub intervals_after_cutoff: usize,
    /// Union of controller-clock intervals. `None` when none is in scope.
    pub wall_ms: Option<u64>,
    /// Sum of every in-scope interval, including remote clocks. `None` when none is in scope.
    pub summed_ms: Option<u64>,
    /// Attempts whose remote clock cannot be aligned with the controller clock.
    pub incomparable_remote_attempts: Vec<String>,
}

/// Full result of one reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciliation {
    pub reconciliation_version: u32,
    pub run_id: String,
    pub cutoff_unix_ms: u64,
    /// Digest of every observation, sorted by identifier.
    pub manifest: Vec<ObservationDigest>,
    /// Every row, sorted by observation identifier and metric.
    pub contributions: Vec<Contribution>,
    pub run: Totals,
    pub tasks: BTreeMap<TaskKey, Totals>,
    pub elapsed: ElapsedSummary,
}

impl Reconciliation {
    /// Builds the factual snapshot record that names this reconciliation's observations.
    pub fn snapshot(&self, snapshot_id: &str, rate_set_digest: Option<String>) -> UsageSnapshot {
        UsageSnapshot {
            schema_version: RECORD_SCHEMA_VERSION,
            snapshot_id: snapshot_id.to_owned(),
            run_id: self.run_id.clone(),
            reconciliation_version: self.reconciliation_version,
            rate_set_digest,
            observations: self.manifest.clone(),
        }
    }
}

/// Reconciles observations and intervals into subtotals.
pub fn reconcile(input: ReconcileInput) -> Result<Reconciliation, ReconcileError> {
    let ReconcileInput {
        run_id,
        cutoff_unix_ms,
        mut observations,
        intervals,
    } = input;
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    if let Some(pair) = observations
        .windows(2)
        .find(|pair| pair[0].observation_id == pair[1].observation_id)
    {
        return Err(ReconcileError::DuplicateObservation(
            pair[0].observation_id.clone(),
        ));
    }

    let sessions = SessionIndex::new(&observations);
    let allocations: Vec<Allocation> = observations
        .iter()
        .map(|record| allocation_of(record, &sessions))
        .collect();

    let mut contributions = Vec::new();
    for category in TOKEN_CATEGORIES {
        let readings = token_readings(&observations, &sessions, category, cutoff_unix_ms);
        contributions.extend(resolve_readings(
            MetricKey::Tokens(category),
            &readings,
            &observations,
            &allocations,
        ));
    }
    let readings = cost_readings(&observations, &sessions, cutoff_unix_ms);
    contributions.extend(resolve_readings(
        MetricKey::Cost,
        &readings,
        &observations,
        &allocations,
    ));
    contributions.sort_by(|left, right| {
        (left.observation_id.as_str(), left.metric.rank())
            .cmp(&(right.observation_id.as_str(), right.metric.rank()))
    });

    let every_row: Vec<&Contribution> = contributions.iter().collect();
    let run = totals(&every_row, cutoff_unix_ms)?;

    let mut rows_by_task: BTreeMap<TaskKey, Vec<&Contribution>> = BTreeMap::new();
    for contribution in &contributions {
        if let Allocation::Task(key) = &contribution.allocation {
            rows_by_task
                .entry(key.clone())
                .or_default()
                .push(contribution);
        }
    }
    let mut tasks = BTreeMap::new();
    for (key, rows) in &rows_by_task {
        tasks.insert(key.clone(), totals(rows, cutoff_unix_ms)?);
    }

    let manifest = observations
        .iter()
        .map(|record| {
            let bytes = serde_json::to_vec(&record.observation)
                .map_err(|error| ReconcileError::Encode(error.to_string()))?;
            Ok(ObservationDigest {
                observation_id: record.observation_id.clone(),
                digest: digest_bytes(&bytes),
            })
        })
        .collect::<Result<Vec<_>, ReconcileError>>()?;

    Ok(Reconciliation {
        reconciliation_version: RECONCILIATION_VERSION,
        run_id,
        cutoff_unix_ms,
        manifest,
        contributions,
        run,
        tasks,
        elapsed: elapsed_summary(&intervals, cutoff_unix_ms)?,
    })
}

/// Session identity lookup. Built once so no step scans every observation.
struct SessionIndex<'a> {
    observations: HashMap<&'a str, Vec<usize>>,
    plans: HashMap<&'a str, BTreeSet<&'a str>>,
}

impl<'a> SessionIndex<'a> {
    fn new(records: &'a [ObservationRecord]) -> Self {
        let mut index = Self {
            observations: HashMap::new(),
            plans: HashMap::new(),
        };
        for (position, record) in records.iter().enumerate() {
            let identity = &record.observation.identity;
            if let Some(session) = identity.session_id.as_deref() {
                index
                    .observations
                    .entry(session)
                    .or_default()
                    .push(position);
                if let Some(plan) = identity.plan_id.as_deref() {
                    index.plans.entry(session).or_default().insert(plan);
                }
            }
        }
        index
    }

    /// A session that spans two plans cannot be assigned to one task.
    fn is_shared(&self, session: &str) -> bool {
        self.plans.get(session).is_some_and(|plans| plans.len() > 1)
    }
}

fn allocation_of(record: &ObservationRecord, sessions: &SessionIndex) -> Allocation {
    let identity = &record.observation.identity;
    if identity
        .session_id
        .as_deref()
        .is_some_and(|session| sessions.is_shared(session))
    {
        return Allocation::Unallocated("shared_session".to_owned());
    }
    match &identity.task_id {
        Some(task_id) => Allocation::Task(TaskKey {
            plan_id: identity.plan_id.clone(),
            task_id: task_id.clone(),
        }),
        None => Allocation::Unallocated("no_task".to_owned()),
    }
}

fn session_of(records: &[ObservationRecord], index: usize) -> Option<&str> {
    records[index].observation.identity.session_id.as_deref()
}

/// Finds the treatment an ancestor session's descendant scope imposes on a child.
///
/// An ancestor that includes its descendants already counts this child, so the child
/// is excluded. An ancestor with an unknown scope makes the child non-additive.
fn ancestor_overlap(
    records: &[ObservationRecord],
    sessions: &SessionIndex,
    index: usize,
    scope_of: impl Fn(&UsageObservation) -> DescendantScope,
) -> Option<Treatment> {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut saw_unknown = false;
    let mut next = records[index]
        .observation
        .identity
        .parent_session_id
        .as_deref();
    while let Some(parent) = next.take() {
        if !visited.insert(parent) {
            break;
        }
        let Some(ancestors) = sessions.observations.get(parent) else {
            break;
        };
        for &ancestor in ancestors {
            match scope_of(&records[ancestor].observation) {
                DescendantScope::IncludesDescendants => {
                    return Some(Treatment::Excluded("included_in_ancestor".to_owned()));
                }
                DescendantScope::Unknown => saw_unknown = true,
                DescendantScope::Own => {}
            }
            if next.is_none() {
                next = records[ancestor]
                    .observation
                    .identity
                    .parent_session_id
                    .as_deref();
            }
        }
    }
    saw_unknown.then(|| Treatment::NonAdditive("descendant_scope_unknown".to_owned()))
}

/// Quantity behavior that the shared series logic needs.
trait Quantity: Clone {
    /// Returns `self - base`. Returns `None` when `self` is below `base`, which marks a reset.
    fn delta_from(&self, base: &Self) -> Option<Self>;
    fn to_amount(&self) -> Amount;
}

impl Quantity for TokenCount {
    fn delta_from(&self, base: &Self) -> Option<Self> {
        let delta = self.get().checked_sub(base.get())?;
        TokenCount::new(delta).ok()
    }

    fn to_amount(&self) -> Amount {
        Amount::Tokens(self.get())
    }
}

impl Quantity for MoneyAmount {
    fn delta_from(&self, base: &Self) -> Option<Self> {
        if self.unit != base.unit || self.kind != base.kind || self.amount < base.amount {
            return None;
        }
        Some(MoneyAmount {
            amount: self.amount.checked_sub(base.amount)?,
            unit: self.unit.clone(),
            kind: self.kind,
        })
    }

    fn to_amount(&self) -> Amount {
        Amount::Money(self.clone())
    }
}

/// One metric value of one observation, before its treatment is decided.
struct Reading<Q> {
    index: usize,
    end_unix_ms: u64,
    reported: Option<Q>,
    semantics: CounterSemantics,
    /// Series key within a session. Money readings split by unit and kind.
    group: String,
    /// Treatment fixed before series logic runs. `None` leaves the reading to that logic.
    fixed: Option<Treatment>,
}

/// Decision for one reading after series logic has run.
struct Decision<Q> {
    treatment: Treatment,
    contributed: Option<Q>,
    basis: Option<Basis>,
}

impl<Q> Decision<Q> {
    fn without_value(treatment: Treatment) -> Self {
        Self {
            treatment,
            contributed: None,
            basis: None,
        }
    }

    fn included(value: Q, basis: Basis) -> Self {
        Self {
            treatment: Treatment::Included,
            contributed: Some(value),
            basis: Some(basis),
        }
    }
}

fn read_metric<Q: Quantity>(
    index: usize,
    metric: &Metric<Q>,
    structural: Option<Treatment>,
    cutoff_unix_ms: u64,
) -> Reading<Q> {
    let (reported, gap) = match &metric.value {
        Availability::Reported { value } => (Some(value.clone()), None),
        Availability::Partial { value, reason } => (Some(value.clone()), Some(reason.clone())),
        Availability::Unsupported { reason }
        | Availability::Unavailable { reason }
        | Availability::Malformed { reason } => (None, Some(reason.clone())),
    };
    let end_unix_ms = metric.window.map_or(0, |window| window.end_unix_ms());
    let fixed = if reported.is_none() {
        Some(Treatment::Unknown(gap.clone().unwrap_or_default()))
    } else if structural.is_some() {
        structural
    } else if end_unix_ms > cutoff_unix_ms {
        Some(Treatment::Excluded("after_cutoff".to_owned()))
    } else if metric.semantics == CounterSemantics::Unknown {
        Some(Treatment::NonAdditive("unknown_semantics".to_owned()))
    } else {
        gap.map(Treatment::NonAdditive)
    };
    Reading {
        index,
        end_unix_ms,
        reported,
        semantics: metric.semantics,
        group: String::new(),
        fixed,
    }
}

fn token_metric(usage: &TokenUsage, category: UsageCategory) -> &TokenMetric {
    match category {
        UsageCategory::Input => &usage.input,
        UsageCategory::Output => &usage.output,
        UsageCategory::CachedInput => &usage.cached_input,
        UsageCategory::CacheWrite => &usage.cache_write,
        UsageCategory::Reasoning => &usage.reasoning,
        UsageCategory::Other => &usage.other,
    }
}

fn category_index(category: UsageCategory) -> usize {
    match category {
        UsageCategory::Input => 0,
        UsageCategory::Output => 1,
        UsageCategory::CachedInput => 2,
        UsageCategory::CacheWrite => 3,
        UsageCategory::Reasoning => 4,
        UsageCategory::Other => 5,
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

fn token_readings(
    records: &[ObservationRecord],
    sessions: &SessionIndex,
    category: UsageCategory,
    cutoff_unix_ms: u64,
) -> Vec<Reading<TokenCount>> {
    records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            let token = token_metric(&record.observation.tokens, category);
            let relation = match &token.relation {
                CounterRelation::Exclusive => None,
                CounterRelation::SubsetOf { category: parent } => Some(Treatment::Excluded(
                    format!("subset_of_{}", category_name(*parent)),
                )),
                CounterRelation::Unknown => {
                    Some(Treatment::NonAdditive("unknown_relation".to_owned()))
                }
            };
            let structural = relation.or_else(|| {
                ancestor_overlap(records, sessions, index, |observation| {
                    token_metric(&observation.tokens, category)
                        .count
                        .descendants
                })
            });
            read_metric(index, &token.count, structural, cutoff_unix_ms)
        })
        .collect()
}

fn cost_readings(
    records: &[ObservationRecord],
    sessions: &SessionIndex,
    cutoff_unix_ms: u64,
) -> Vec<Reading<MoneyAmount>> {
    records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            let structural = ancestor_overlap(records, sessions, index, |observation| {
                observation.cost.descendants
            });
            let mut reading =
                read_metric(index, &record.observation.cost, structural, cutoff_unix_ms);
            reading.group = reading.reported.as_ref().map(money_key).unwrap_or_default();
            reading
        })
        .collect()
}

fn money_key(amount: &MoneyAmount) -> String {
    format!("{:?}|{:?}", amount.unit, amount.kind)
}

/// Resolves one metric's readings into rows.
///
/// A cumulative series counts each step between consecutive readings once. Its first
/// reading is a baseline. A series with no later reading has no baseline, so it is
/// non-additive. A delta reading is excluded when a usable cumulative series covers the
/// same session and group, because adding both would count the same usage twice.
fn resolve_readings<Q: Quantity>(
    metric: MetricKey,
    readings: &[Reading<Q>],
    records: &[ObservationRecord],
    allocations: &[Allocation],
) -> Vec<Contribution> {
    let mut decisions: Vec<Option<Decision<Q>>> = readings
        .iter()
        .map(|reading| reading.fixed.clone().map(Decision::without_value))
        .collect();

    let mut series: BTreeMap<(&str, &str), Vec<usize>> = BTreeMap::new();
    for (position, reading) in readings.iter().enumerate() {
        if decisions[position].is_some()
            || reading.semantics != CounterSemantics::CumulativeSnapshot
        {
            continue;
        }
        match session_of(records, reading.index) {
            Some(session) => series
                .entry((session, reading.group.as_str()))
                .or_default()
                .push(position),
            None => {
                decisions[position] = Some(Decision::without_value(Treatment::NonAdditive(
                    "missing_baseline".to_owned(),
                )))
            }
        }
    }

    let mut usable: HashSet<(&str, &str)> = HashSet::new();
    for (key, mut positions) in series {
        positions
            .sort_by_key(|&position| (readings[position].end_unix_ms, readings[position].index));
        if positions.len() < 2 {
            for position in positions {
                decisions[position] = Some(Decision::without_value(Treatment::NonAdditive(
                    "missing_baseline".to_owned(),
                )));
            }
            continue;
        }
        usable.insert(key);
        let mut baseline: Option<&Q> = None;
        for position in positions {
            let Some(value) = readings[position].reported.as_ref() else {
                continue;
            };
            decisions[position] = Some(match baseline {
                None => Decision::without_value(Treatment::Excluded("baseline_reading".to_owned())),
                Some(base) => match value.delta_from(base) {
                    Some(delta) => Decision::included(delta, Basis::CumulativeDelta),
                    None => {
                        Decision::without_value(Treatment::NonAdditive("counter_reset".to_owned()))
                    }
                },
            });
            baseline = Some(value);
        }
    }

    for (position, reading) in readings.iter().enumerate() {
        if decisions[position].is_some() || reading.semantics != CounterSemantics::Delta {
            continue;
        }
        let shadowed = session_of(records, reading.index)
            .is_some_and(|session| usable.contains(&(session, reading.group.as_str())));
        decisions[position] = Some(match &reading.reported {
            _ if shadowed => {
                Decision::without_value(Treatment::Excluded("alternative_basis_present".to_owned()))
            }
            Some(value) => Decision::included(value.clone(), Basis::Delta),
            None => Decision::without_value(Treatment::NonAdditive("value_missing".to_owned())),
        });
    }

    readings
        .iter()
        .zip(decisions)
        .map(|(reading, decision)| {
            let decision = decision.unwrap_or_else(|| {
                Decision::without_value(Treatment::NonAdditive("unresolved_basis".to_owned()))
            });
            Contribution {
                observation_id: records[reading.index].observation_id.clone(),
                metric,
                reported: reading.reported.as_ref().map(Quantity::to_amount),
                contributed: decision.contributed.as_ref().map(Quantity::to_amount),
                basis: decision.basis,
                treatment: decision.treatment,
                allocation: allocations[reading.index].clone(),
            }
        })
        .collect()
}

/// Builds the subtotals of one scope. Every category and money basis gets a subtotal.
fn totals(rows: &[&Contribution], cutoff_unix_ms: u64) -> Result<Totals, ReconcileError> {
    let mut tokens = Vec::with_capacity(TOKEN_CATEGORIES.len());
    for category in TOKEN_CATEGORIES {
        let scope: Vec<&Contribution> = rows
            .iter()
            .copied()
            .filter(|row| row.metric == MetricKey::Tokens(category))
            .collect();
        tokens.push(token_subtotal(&scope, cutoff_unix_ms)?);
    }
    let cost_rows: Vec<&Contribution> = rows
        .iter()
        .copied()
        .filter(|row| row.metric == MetricKey::Cost)
        .collect();
    Ok(Totals {
        tokens,
        cost: cost_subtotals(&cost_rows, cutoff_unix_ms)?,
    })
}

fn token_subtotal(
    rows: &[&Contribution],
    cutoff_unix_ms: u64,
) -> Result<Subtotal<u64>, ReconcileError> {
    let mut total: Option<u64> = None;
    for row in rows {
        if let (Treatment::Included, Some(Amount::Tokens(value))) =
            (&row.treatment, &row.contributed)
        {
            let sum = total
                .unwrap_or(0)
                .checked_add(*value)
                .ok_or(ReconcileError::TotalOverflow)?;
            total = Some(sum);
        }
    }
    Ok(Subtotal::new(total, rows, cutoff_unix_ms))
}

/// Money subtotals per unit and kind.
///
/// A row whose unit is unknown belongs to every group, so it keeps each group incomplete.
fn cost_subtotals(
    rows: &[&Contribution],
    cutoff_unix_ms: u64,
) -> Result<Vec<CostSubtotal>, ReconcileError> {
    let mut groups: BTreeMap<String, (MoneyUnit, MoneyKind)> = BTreeMap::new();
    for row in rows {
        if let Some(Amount::Money(amount)) = &row.reported {
            groups
                .entry(money_key(amount))
                .or_insert_with(|| (amount.unit.clone(), amount.kind));
        }
    }
    if groups.is_empty() {
        return Ok(vec![CostSubtotal {
            unit: None,
            kind: None,
            subtotal: Subtotal::new(None, rows, cutoff_unix_ms),
        }]);
    }

    let mut subtotals = Vec::with_capacity(groups.len());
    for (key, (unit, kind)) in groups {
        let scope: Vec<&Contribution> = rows
            .iter()
            .copied()
            .filter(|row| match &row.reported {
                Some(Amount::Money(amount)) => money_key(amount) == key,
                _ => true,
            })
            .collect();
        let mut total: Option<MoneyAmount> = None;
        for row in &scope {
            if let (Treatment::Included, Some(Amount::Money(amount))) =
                (&row.treatment, &row.contributed)
            {
                total = Some(match total {
                    None => amount.clone(),
                    Some(sum) => sum
                        .try_add(amount)
                        .map_err(|error| ReconcileError::Money(error.to_string()))?,
                });
            }
        }
        subtotals.push(CostSubtotal {
            unit: Some(unit),
            kind: Some(kind),
            subtotal: Subtotal::new(total, &scope, cutoff_unix_ms),
        });
    }
    Ok(subtotals)
}

/// Sums every in-scope interval and takes the controller-clock union for wall time.
///
/// Remote intervals count toward the sum, but they stay out of the wall union. Their
/// attempts are listed as incomparable instead.
fn elapsed_summary(
    intervals: &[ElapsedInterval],
    cutoff_unix_ms: u64,
) -> Result<ElapsedSummary, ReconcileError> {
    let mut ordered: Vec<&ElapsedInterval> = intervals.iter().collect();
    ordered.sort_by(|left, right| {
        (
            left.attempt_id.as_str(),
            left.start_unix_ms,
            left.end_unix_ms,
        )
            .cmp(&(
                right.attempt_id.as_str(),
                right.start_unix_ms,
                right.end_unix_ms,
            ))
    });

    let mut in_scope = Vec::new();
    let mut after_cutoff = 0;
    for interval in ordered {
        let window = ObservationWindow::new(interval.start_unix_ms, interval.end_unix_ms).map_err(
            |error| ReconcileError::InvalidInterval {
                attempt_id: interval.attempt_id.clone(),
                reason: error.to_string(),
            },
        )?;
        if window.end_unix_ms() > cutoff_unix_ms {
            after_cutoff += 1;
        } else {
            in_scope.push((interval, window));
        }
    }

    let mut summed: Option<u64> = None;
    for (_, window) in &in_scope {
        let length = window.end_unix_ms() - window.start_unix_ms();
        summed = Some(
            summed
                .unwrap_or(0)
                .checked_add(length)
                .ok_or(ReconcileError::TotalOverflow)?,
        );
    }

    let mut controller_spans: Vec<(u64, u64)> = in_scope
        .iter()
        .filter(|(interval, _)| interval.clock == ClockDomain::Controller)
        .map(|(_, window)| (window.start_unix_ms(), window.end_unix_ms()))
        .collect();
    let incomparable_remote_attempts = in_scope
        .iter()
        .filter(|(interval, _)| matches!(interval.clock, ClockDomain::Remote { .. }))
        .map(|(interval, _)| interval.attempt_id.clone())
        .collect();

    Ok(ElapsedSummary {
        cutoff_unix_ms,
        intervals_in_scope: in_scope.len(),
        intervals_after_cutoff: after_cutoff,
        wall_ms: union_length(&mut controller_spans),
        summed_ms: summed,
        incomparable_remote_attempts,
    })
}

/// Total length covered by at least one span. `None` when there are no spans.
fn union_length(spans: &mut [(u64, u64)]) -> Option<u64> {
    spans.sort_unstable();
    let (&first, rest) = spans.split_first()?;
    let mut current = first;
    let mut covered = 0;
    for &(start, end) in rest {
        if start <= current.1 {
            current.1 = current.1.max(end);
        } else {
            covered += current.1 - current.0;
            current = (start, end);
        }
    }
    Some(covered + current.1 - current.0)
}
