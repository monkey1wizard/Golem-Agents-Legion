//! Useful-work judgments over one factual snapshot.
//!
//! A judgment classifies counted rows of one reconciliation as accepted,
//! support, or unused. Amounts that no judgment classifies stay unresolved.
//! A judgment never changes the snapshot it judges. A later judgment supersedes
//! an earlier one only on the same snapshot, so every version stays readable
//! and no earlier decision is rebased on new evidence.
//!
//! A residual claim splits a parent row that includes its descendants into the
//! covered child rows and the parent's own remainder. The split counts only when
//! the reconciled amounts prove it: each covered child is excluded as included
//! in an ancestor, and the children fit inside the parent. The snapshot does not
//! carry session lineage, so the proof relies on the reconciler's ancestor
//! exclusion rather than on a separate lineage check.

use crate::usage::reconcile::{
    Amount, Contribution, MetricKey, Reconciliation, Treatment, TOKEN_CATEGORIES,
};
use crate::usage::store::{Publication, StoreError, UsageStore};
use crate::usage::{digest_bytes, RecordKind, StoredRecord, UsageSnapshot, RECORD_SCHEMA_VERSION};
use dispatch::usage::{MoneyAmount, MoneyKind, MoneyUnit, UsageCategory};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Exclusion reason that the reconciler gives a child row whose ancestor includes its descendants.
const INCLUDED_IN_ANCESTOR: &str = "included_in_ancestor";

/// Decimal places kept on a ratio.
const RATIO_SCALE: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AssessmentError {
    #[error("assessment field `{0}` is missing or empty")]
    MissingField(&'static str),
    #[error("assessment schema version {found} is not supported, expected {supported}")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("assessment does not match its snapshot: {0}")]
    SnapshotMismatch(String),
    #[error("assessment snapshot digest does not match the snapshot")]
    SnapshotDigestMismatch,
    #[error("assessment names row `{0}`, which the reconciliation does not contain")]
    UnknownRow(String),
    #[error("row `{0}` is judged more than once")]
    DuplicateAssignment(String),
    #[error("row `{0}` has no known value and cannot be judged")]
    UnassignableRow(String),
    #[error("row `{0}` is excluded from every total and no residual claim covers it")]
    NotCounted(String),
    #[error("row `{0}` is named by more than one judgment or claim")]
    Overlap(String),
    #[error("assessment `{assessment}` supersedes `{prior}`, which is not on the same snapshot")]
    SupersessionMismatch { assessment: String, prior: String },
    #[error("assessment does not conserve the total of basis {0}")]
    Conservation(String),
    #[error("assessment record could not be encoded: {0}")]
    Encode(String),
    #[error("usage store failed: {0}")]
    Store(String),
}

impl From<StoreError> for AssessmentError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

/// Classification of a counted amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Work that the run kept.
    Accepted,
    /// Work that informed accepted work without being part of it.
    Support,
    /// Work that was produced and not used.
    Unused,
}

/// Classification of one counted row. The row is named by `<observation_id>/<metric>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub contribution_id: String,
    pub outcome: Outcome,
    pub reason: String,
    pub evidence: Vec<String>,
}

/// Claim that a parent row's total is its covered child rows plus a residual.
///
/// The residual takes the outcome. The covered rows keep their own assignments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResidualClaim {
    pub parent_contribution_id: String,
    pub covered_contribution_ids: Vec<String>,
    pub outcome: Outcome,
    pub reason: String,
    pub evidence: Vec<String>,
}

/// One judgment about one snapshot. The same type is the input and the stored record.
///
/// It carries no factual fields. Totals come only from the reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssessmentRecord {
    pub schema_version: u32,
    pub assessment_id: String,
    pub run_id: String,
    pub snapshot_id: String,
    /// Digest of the snapshot record bytes this judgment judges.
    pub snapshot_digest: String,
    /// Stated rule that decides what counts as useful work.
    pub useful_work_rule: String,
    pub author: String,
    pub session_id: String,
    pub model: String,
    pub recorded_at_unix_ms: u64,
    /// Earlier judgment on the same snapshot that this one supersedes.
    pub supersedes: Option<String>,
    pub assignments: Vec<Assignment>,
    pub residuals: Vec<ResidualClaim>,
}

impl StoredRecord for AssessmentRecord {
    const KIND: RecordKind = RecordKind::Assessment;

    fn schema_version(&self) -> u32 {
        self.schema_version
    }

    fn run_id(&self) -> &str {
        &self.run_id
    }

    fn record_id(&self) -> &str {
        &self.assessment_id
    }
}

/// Measure that totals are kept in. Tokens and each money unit and kind are separate bases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasisKey {
    Tokens(UsageCategory),
    Money { unit: MoneyUnit, kind: MoneyKind },
}

/// Whether a basis total covers every row of its scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completeness {
    /// Every row of the basis is known and additive.
    Complete,
    /// The total sums only the known rows. Some rows are unknown or non-additive.
    KnownSubtotal,
}

/// Share of a basis total, computed on the same basis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ratio {
    pub value: Decimal,
    pub completeness: Completeness,
}

/// Derived totals of one basis. Accepted, support, unused, and unresolved sum to `total`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasisAssessment {
    pub basis: BasisKey,
    pub total: Amount,
    pub accepted: Amount,
    pub support: Amount,
    pub unused: Amount,
    pub unresolved: Amount,
    /// Accepted share of the total. `None` when the total is zero.
    pub accepted_ratio: Option<Ratio>,
    pub completeness: Completeness,
}

/// Residual claim that the reconciled amounts do not prove. Its parent stays unresolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnprovenResidual {
    pub parent_contribution_id: String,
    pub reason: String,
}

/// Derived result of one judgment. It is recomputed and never stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    pub assessment_id: String,
    pub snapshot_id: String,
    pub bases: Vec<BasisAssessment>,
    pub unproven_residuals: Vec<UnprovenResidual>,
}

/// Derives the totals of one judgment from its reconciliation and its snapshot.
pub fn assess(
    judgment: &AssessmentRecord,
    reconciled: &Reconciliation,
    snapshot: &UsageSnapshot,
) -> Result<Assessment, AssessmentError> {
    check_fields(judgment)?;
    check_snapshot(judgment, reconciled, snapshot)?;

    let rows = index_rows(reconciled);
    let claims = resolve_claims(judgment, &rows)?;
    let parents: BTreeSet<String> = claims
        .iter()
        .map(|claim| claim.claim.parent_contribution_id.clone())
        .collect();
    let covered: BTreeSet<String> = claims
        .iter()
        .flat_map(|claim| claim.claim.covered_contribution_ids.iter().cloned())
        .collect();
    let assigned = index_assignments(judgment, &rows, &parents, &covered)?;

    let mut leaves = Vec::new();
    let mut split_parents = BTreeSet::new();
    let mut unproven_residuals = Vec::new();
    for claim in &claims {
        match prove_split(claim.parent, &claim.children) {
            Ok(split) => {
                split_parents.insert(claim.claim.parent_contribution_id.clone());
                for (id, amount) in claim
                    .claim
                    .covered_contribution_ids
                    .iter()
                    .zip(split.covered)
                {
                    leaves.push(Leaf {
                        basis: split.basis.clone(),
                        amount,
                        outcome: assigned.get(id).map(|assignment| assignment.outcome),
                    });
                }
                leaves.push(Leaf {
                    basis: split.basis,
                    amount: split.residual,
                    outcome: Some(claim.claim.outcome),
                });
            }
            Err(reason) => unproven_residuals.push(UnprovenResidual {
                parent_contribution_id: claim.claim.parent_contribution_id.clone(),
                reason,
            }),
        }
    }
    for row in &reconciled.contributions {
        let Some(amount) = row.contributed.as_ref() else {
            continue;
        };
        let id = row_id(row);
        if split_parents.contains(&id) {
            continue;
        }
        leaves.push(Leaf {
            basis: basis_of(row.metric, amount)
                .ok_or_else(|| AssessmentError::Conservation(id.clone()))?,
            amount: amount.clone(),
            outcome: assigned.get(&id).map(|assignment| assignment.outcome),
        });
    }

    let mut tallies = run_tallies(reconciled);
    for leaf in leaves {
        let tally = tallies
            .iter_mut()
            .find(|tally| tally.basis == leaf.basis)
            .ok_or_else(|| AssessmentError::Conservation(format!("{:?}", leaf.basis)))?;
        let slot = match leaf.outcome {
            Some(Outcome::Accepted) => &mut tally.accepted,
            Some(Outcome::Support) => &mut tally.support,
            Some(Outcome::Unused) => &mut tally.unused,
            None => &mut tally.unresolved,
        };
        let next = add(slot, &leaf.amount)
            .ok_or(AssessmentError::Conservation(format!("{:?}", leaf.basis)))?;
        *slot = next;
    }
    let bases = tallies
        .into_iter()
        .map(Tally::finish)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Assessment {
        assessment_id: judgment.assessment_id.clone(),
        snapshot_id: judgment.snapshot_id.clone(),
        bases,
        unproven_residuals,
    })
}

/// Publishes a judgment to the store. The judged snapshot must already be published.
///
/// A judgment that supersedes another must name a judgment on the same snapshot.
pub fn publish_judgment(
    store: &UsageStore,
    judgment: &AssessmentRecord,
) -> Result<Publication, AssessmentError> {
    let snapshot: UsageSnapshot = store.read(&judgment.run_id, &judgment.snapshot_id)?;
    if digest_record(&snapshot)? != judgment.snapshot_digest {
        return Err(AssessmentError::SnapshotDigestMismatch);
    }
    if let Some(prior_id) = &judgment.supersedes {
        let prior: AssessmentRecord = store.read(&judgment.run_id, prior_id)?;
        if prior.snapshot_id != judgment.snapshot_id {
            return Err(AssessmentError::SupersessionMismatch {
                assessment: judgment.assessment_id.clone(),
                prior: prior_id.clone(),
            });
        }
    }
    Ok(store.publish(judgment)?)
}

fn check_fields(judgment: &AssessmentRecord) -> Result<(), AssessmentError> {
    if judgment.schema_version != RECORD_SCHEMA_VERSION {
        return Err(AssessmentError::UnsupportedSchema {
            found: judgment.schema_version,
            supported: RECORD_SCHEMA_VERSION,
        });
    }
    let required = [
        ("assessment_id", &judgment.assessment_id),
        ("run_id", &judgment.run_id),
        ("snapshot_id", &judgment.snapshot_id),
        ("snapshot_digest", &judgment.snapshot_digest),
        ("useful_work_rule", &judgment.useful_work_rule),
        ("author", &judgment.author),
        ("session_id", &judgment.session_id),
        ("model", &judgment.model),
    ];
    for (field, value) in required {
        require(field, value)?;
    }
    if let Some(prior) = &judgment.supersedes {
        require("supersedes", prior)?;
    }
    for assignment in &judgment.assignments {
        require("reason", &assignment.reason)?;
        require_evidence(&assignment.evidence)?;
    }
    for claim in &judgment.residuals {
        require("reason", &claim.reason)?;
        require_evidence(&claim.evidence)?;
    }
    Ok(())
}

fn require(field: &'static str, value: &str) -> Result<(), AssessmentError> {
    if value.trim().is_empty() {
        return Err(AssessmentError::MissingField(field));
    }
    Ok(())
}

fn require_evidence(evidence: &[String]) -> Result<(), AssessmentError> {
    if evidence.is_empty() {
        return Err(AssessmentError::MissingField("evidence"));
    }
    for entry in evidence {
        require("evidence", entry)?;
    }
    Ok(())
}

fn check_snapshot(
    judgment: &AssessmentRecord,
    reconciled: &Reconciliation,
    snapshot: &UsageSnapshot,
) -> Result<(), AssessmentError> {
    if snapshot.snapshot_id != judgment.snapshot_id
        || snapshot.run_id != judgment.run_id
        || reconciled.run_id != judgment.run_id
    {
        return Err(AssessmentError::SnapshotMismatch(
            "snapshot or run identifiers differ".to_owned(),
        ));
    }
    if snapshot.reconciliation_version != reconciled.reconciliation_version {
        return Err(AssessmentError::SnapshotMismatch(
            "reconciliation versions differ".to_owned(),
        ));
    }
    if snapshot.observations != reconciled.manifest {
        return Err(AssessmentError::SnapshotMismatch(
            "observation digests differ".to_owned(),
        ));
    }
    if digest_record(snapshot)? != judgment.snapshot_digest {
        return Err(AssessmentError::SnapshotDigestMismatch);
    }
    Ok(())
}

/// Residual claim with its parent and covered rows found in the reconciliation.
struct ResolvedClaim<'j, 'r> {
    claim: &'j ResidualClaim,
    parent: &'r Contribution,
    children: Vec<&'r Contribution>,
}

/// Finds each claimed row, and rejects a row that a second claim or assignment would also split.
fn resolve_claims<'j, 'r>(
    judgment: &'j AssessmentRecord,
    rows: &BTreeMap<String, &'r Contribution>,
) -> Result<Vec<ResolvedClaim<'j, 'r>>, AssessmentError> {
    let mut seen = BTreeSet::new();
    let mut resolved = Vec::with_capacity(judgment.residuals.len());
    for claim in &judgment.residuals {
        let parent = lookup(rows, &claim.parent_contribution_id)?;
        claim_once(&mut seen, &claim.parent_contribution_id)?;
        let mut children = Vec::with_capacity(claim.covered_contribution_ids.len());
        for id in &claim.covered_contribution_ids {
            children.push(lookup(rows, id)?);
            claim_once(&mut seen, id)?;
        }
        resolved.push(ResolvedClaim {
            claim,
            parent,
            children,
        });
    }
    Ok(resolved)
}

fn claim_once(seen: &mut BTreeSet<String>, id: &str) -> Result<(), AssessmentError> {
    if !seen.insert(id.to_owned()) {
        return Err(AssessmentError::Overlap(id.to_owned()));
    }
    Ok(())
}

/// Checks each assignment against the reconciliation and the residual claims.
fn index_assignments<'j>(
    judgment: &'j AssessmentRecord,
    rows: &BTreeMap<String, &Contribution>,
    parents: &BTreeSet<String>,
    covered: &BTreeSet<String>,
) -> Result<BTreeMap<String, &'j Assignment>, AssessmentError> {
    let mut assigned = BTreeMap::new();
    for assignment in &judgment.assignments {
        let id = assignment.contribution_id.as_str();
        let row = lookup(rows, id)?;
        if parents.contains(id) {
            return Err(AssessmentError::Overlap(id.to_owned()));
        }
        if assigned.insert(id.to_owned(), assignment).is_some() {
            return Err(AssessmentError::DuplicateAssignment(id.to_owned()));
        }
        if row.contributed.is_none() && row.reported.is_none() {
            return Err(AssessmentError::UnassignableRow(id.to_owned()));
        }
        if row.contributed.is_none() && !covered.contains(id) {
            return Err(AssessmentError::NotCounted(id.to_owned()));
        }
    }
    Ok(assigned)
}

/// Parts of a parent total that the reconciled amounts prove.
struct Split {
    basis: BasisKey,
    covered: Vec<Amount>,
    residual: Amount,
}

/// Proves a residual split from reconciled amounts. The error is the reason the split is unproven.
fn prove_split(parent: &Contribution, children: &[&Contribution]) -> Result<Split, String> {
    let whole = parent
        .contributed
        .as_ref()
        .ok_or("parent row has no included value")?;
    let basis = basis_of(parent.metric, whole).ok_or("parent value does not match its metric")?;
    let mut covered = Vec::with_capacity(children.len());
    let mut covered_sum = zero(&basis);
    for child in children {
        let id = row_id(child);
        if !matches!(&child.treatment, Treatment::Excluded(reason) if reason.as_str() == INCLUDED_IN_ANCESTOR)
        {
            return Err(format!("`{id}` is not excluded as included in an ancestor"));
        }
        let value = child
            .reported
            .as_ref()
            .ok_or_else(|| format!("`{id}` has no reported value"))?;
        if basis_of(child.metric, value).as_ref() != Some(&basis) {
            return Err(format!("`{id}` is in another basis than its parent"));
        }
        covered_sum = add(&covered_sum, value).ok_or("covered values do not add up")?;
        covered.push(value.clone());
    }
    let residual = sub(whole, &covered_sum).ok_or("covered values exceed the parent total")?;
    Ok(Split {
        basis,
        covered,
        residual,
    })
}

/// One classified amount of one basis. `None` outcome means unresolved.
struct Leaf {
    basis: BasisKey,
    amount: Amount,
    outcome: Option<Outcome>,
}

/// Running totals of one basis, seeded from the reconciled total.
struct Tally {
    basis: BasisKey,
    total: Amount,
    completeness: Completeness,
    accepted: Amount,
    support: Amount,
    unused: Amount,
    unresolved: Amount,
}

impl Tally {
    fn new(basis: BasisKey, total: Amount, complete: bool) -> Self {
        let empty = zero(&basis);
        Self {
            basis,
            total,
            completeness: if complete {
                Completeness::Complete
            } else {
                Completeness::KnownSubtotal
            },
            accepted: empty.clone(),
            support: empty.clone(),
            unused: empty.clone(),
            unresolved: empty,
        }
    }

    fn finish(self) -> Result<BasisAssessment, AssessmentError> {
        let parts = [
            &self.accepted,
            &self.support,
            &self.unused,
            &self.unresolved,
        ];
        let sum = parts
            .into_iter()
            .try_fold(zero(&self.basis), |sum, part| add(&sum, part))
            .ok_or_else(|| AssessmentError::Conservation(format!("{:?}", self.basis)))?;
        if sum != self.total {
            return Err(AssessmentError::Conservation(format!("{:?}", self.basis)));
        }
        let accepted_ratio = ratio(&self.accepted, &self.total, self.completeness);
        Ok(BasisAssessment {
            basis: self.basis,
            total: self.total,
            accepted: self.accepted,
            support: self.support,
            unused: self.unused,
            unresolved: self.unresolved,
            accepted_ratio,
            completeness: self.completeness,
        })
    }
}

/// One tally per basis that has a reconciled total. A basis without a total never appears.
fn run_tallies(reconciled: &Reconciliation) -> Vec<Tally> {
    let mut tallies = Vec::new();
    for category in TOKEN_CATEGORIES {
        let subtotal = reconciled.run.token(category);
        if let Some(total) = subtotal.total {
            tallies.push(Tally::new(
                BasisKey::Tokens(category),
                Amount::Tokens(total),
                subtotal.complete,
            ));
        }
    }
    for cost in &reconciled.run.cost {
        if let Some(total) = &cost.subtotal.total {
            tallies.push(Tally::new(
                BasisKey::Money {
                    unit: total.unit.clone(),
                    kind: total.kind,
                },
                Amount::Money(total.clone()),
                cost.subtotal.complete,
            ));
        }
    }
    tallies
}

fn ratio(accepted: &Amount, total: &Amount, completeness: Completeness) -> Option<Ratio> {
    let denominator = as_decimal(total);
    if denominator <= Decimal::ZERO {
        return None;
    }
    let value = as_decimal(accepted)
        .checked_div(denominator)?
        .round_dp(RATIO_SCALE);
    Some(Ratio {
        value,
        completeness,
    })
}

fn index_rows(reconciled: &Reconciliation) -> BTreeMap<String, &Contribution> {
    reconciled
        .contributions
        .iter()
        .map(|row| (row_id(row), row))
        .collect()
}

fn lookup<'r>(
    rows: &BTreeMap<String, &'r Contribution>,
    id: &str,
) -> Result<&'r Contribution, AssessmentError> {
    rows.get(id)
        .copied()
        .ok_or_else(|| AssessmentError::UnknownRow(id.to_owned()))
}

/// Identifier of one reconciled row. Judgments name rows by this form.
fn row_id(row: &Contribution) -> String {
    format!("{}/{}", row.observation_id, metric_label(row.metric))
}

fn metric_label(metric: MetricKey) -> &'static str {
    match metric {
        MetricKey::Tokens(UsageCategory::Input) => "input",
        MetricKey::Tokens(UsageCategory::Output) => "output",
        MetricKey::Tokens(UsageCategory::CachedInput) => "cached_input",
        MetricKey::Tokens(UsageCategory::CacheWrite) => "cache_write",
        MetricKey::Tokens(UsageCategory::Reasoning) => "reasoning",
        MetricKey::Tokens(UsageCategory::Other) => "other",
        MetricKey::Cost => "cost",
    }
}

fn basis_of(metric: MetricKey, amount: &Amount) -> Option<BasisKey> {
    match (metric, amount) {
        (MetricKey::Tokens(category), Amount::Tokens(_)) => Some(BasisKey::Tokens(category)),
        (MetricKey::Cost, Amount::Money(money)) => Some(BasisKey::Money {
            unit: money.unit.clone(),
            kind: money.kind,
        }),
        _ => None,
    }
}

fn zero(basis: &BasisKey) -> Amount {
    match basis {
        BasisKey::Tokens(_) => Amount::Tokens(0),
        BasisKey::Money { unit, kind } => Amount::Money(MoneyAmount {
            amount: Decimal::ZERO,
            unit: unit.clone(),
            kind: *kind,
        }),
    }
}

/// Sum within one basis. `None` on overflow or a basis mismatch.
fn add(left: &Amount, right: &Amount) -> Option<Amount> {
    match (left, right) {
        (Amount::Tokens(a), Amount::Tokens(b)) => a.checked_add(*b).map(Amount::Tokens),
        (Amount::Money(a), Amount::Money(b)) if a.unit == b.unit && a.kind == b.kind => {
            Some(Amount::Money(MoneyAmount {
                amount: a.amount.checked_add(b.amount)?,
                unit: a.unit.clone(),
                kind: a.kind,
            }))
        }
        _ => None,
    }
}

/// Difference within one basis. `None` when the result would be negative or the bases differ.
fn sub(left: &Amount, right: &Amount) -> Option<Amount> {
    match (left, right) {
        (Amount::Tokens(a), Amount::Tokens(b)) => a.checked_sub(*b).map(Amount::Tokens),
        (Amount::Money(a), Amount::Money(b)) if a.unit == b.unit && a.kind == b.kind => {
            let amount = a.amount.checked_sub(b.amount)?;
            (amount >= Decimal::ZERO).then(|| {
                Amount::Money(MoneyAmount {
                    amount,
                    unit: a.unit.clone(),
                    kind: a.kind,
                })
            })
        }
        _ => None,
    }
}

fn as_decimal(amount: &Amount) -> Decimal {
    match amount {
        Amount::Tokens(value) => Decimal::from(*value),
        Amount::Money(money) => money.amount,
    }
}

fn digest_record<R: Serialize>(record: &R) -> Result<String, AssessmentError> {
    serde_json::to_vec(record)
        .map(|bytes| digest_bytes(&bytes))
        .map_err(|error| AssessmentError::Encode(error.to_string()))
}
