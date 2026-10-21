//! `gal usage report` reads one run's stored accounting records and never writes.
//! `gal usage collect` appends the observations that a bound native source adds to
//! the run, as a new snapshot. `gal usage assess` appends one judgment on an existing
//! snapshot. Neither command rewrites a record.
//!
//! The report reads published records only. It never collects native evidence,
//! never reprices, and never rewrites a record. Factual rows exist only when a
//! reconciliation rebuilds the selected snapshot exactly. The usage store keeps
//! snapshot manifests, not observation bodies, so a report without a rebuild
//! says that its rows and totals are unavailable.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use dispatch::dispatch::DispatchQuota;
use dispatch::host_session::HostSessionRef;
use dispatch::host_usage::{
    collect as collect_host_usage, HostRoots, HostUsageCursor, HostUsageError, HostUsagePage,
    HostUsageRole, PageStatus, UsageWindow,
};
use dispatch::quota::{
    AccountIdentityCheck, Acquisition, BillingMode, Freshness, QuotaAmount, QuotaIdentity,
    QuotaLimit, QuotaObservation, QuotaRuntime, QuotaSnapshot, QuotaUnit, QuotaWindow,
    UnlimitedBasis, Utilization, UtilizationDirection, UtilizationScale, WindowKind,
};
use dispatch::usage::{
    Availability, MoneyAmount, MoneyKind, MoneyUnit, ObservationIdentity, UsageCategory,
    UsageObservation,
};
use gal_engine::ExitCode;
use pipeline::usage::assessment::{
    assess, publish_judgment, Assessment, AssessmentError, AssessmentRecord, Assignment,
    BasisAssessment, BasisKey, Completeness, Outcome, ResidualClaim,
};
use pipeline::usage::binding::{RunIdentity, RUN_MANIFEST_VERSION};
use pipeline::usage::pricing::{price_snapshot, Estimate, PricingInput, RateSet};
use pipeline::usage::quota::{
    return_reference, QuotaObservationRecord, QuotaRecordError, QuotaRevision,
};
use pipeline::usage::reconcile::{
    Allocation, Amount, Basis, Contribution, Coverage, ElapsedSummary, MetricKey,
    ObservationRecord, Reconciliation, Subtotal, TaskKey, Totals, Treatment,
    RECONCILIATION_VERSION, TOKEN_CATEGORIES,
};
use pipeline::usage::store::{record_digest, Publication, StoreError, UsageStore};
use pipeline::usage::{
    digest_bytes, ObservationDigest, StoredRecord, UsageSnapshot, RECORD_SCHEMA_VERSION,
};
use serde_json::{json, Value};

use super::pipeline_preflight::{self, OrchestratorRecord};
use super::usage_capture::{self, CaptureError};

/// Version of the `--json` document layout.
const REPORT_SCHEMA_VERSION: u32 = 1;

const USAGE: &str = "usage: gal usage report --run <run-id> [--snapshot <snapshot-id>] [--assessment <assessment-id>] [--json]";

const UNAVAILABLE: &str = "the usage store keeps snapshot manifests, not observation bodies, so factual rows and totals cannot be rebuilt";

/// Parsed `gal usage report` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    pub(crate) run_id: String,
    pub(crate) snapshot_id: Option<String>,
    pub(crate) assessment_id: Option<String>,
    pub(crate) json: bool,
}

/// Failure that ends the report before any output.
#[derive(Debug, PartialEq, Eq)]
enum ReportError {
    /// Invalid selection or input. Exits with the usage code.
    Usage(String),
    /// Storage or integrity failure. Exits with the error code.
    Integrity(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Complete,
    Partial,
    Unavailable,
    Conflict,
}

/// Quantity of one row or subtotal. Token counts and money amounts keep their own shape.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Quantity {
    Tokens(u64),
    Money {
        amount: String,
        unit: String,
        kind: &'static str,
    },
}

impl Quantity {
    fn of_amount(amount: &Amount) -> Self {
        match amount {
            Amount::Tokens(count) => Self::Tokens(*count),
            Amount::Money(money) => Self::of_money(money),
        }
    }

    fn of_money(money: &MoneyAmount) -> Self {
        Self::Money {
            amount: money.amount.to_string(),
            unit: unit_name(&money.unit).to_owned(),
            kind: kind_name(&money.kind),
        }
    }

    fn render(&self) -> String {
        match self {
            Self::Tokens(count) => count.to_string(),
            Self::Money { amount, unit, kind } => format!("{amount} {unit} {kind}"),
        }
    }
}

struct Document {
    schema_version: u32,
    status: Status,
    status_reason: Option<String>,
    run_id: String,
    /// Orchestrator binding or its unavailable reason. `None` when the run has no preflight receipt.
    orchestrator: Option<OrchestratorView>,
    snapshot: SnapshotView,
    observations: Vec<ObservationDigest>,
    factual: Factual,
    account: AccountView,
    assessment: Option<AssessmentView>,
    history: History,
}

struct SnapshotView {
    snapshot_id: String,
    record_digest: String,
    reconciliation_version: u32,
    rate_set_digest: Option<String>,
}

struct History {
    snapshot_ids: Vec<String>,
    assessment_ids: Vec<String>,
    stale_assessment_ids: Vec<String>,
}

enum Factual {
    Rebuilt {
        cutoff_unix_ms: u64,
        elapsed: ElapsedView,
        rows: Vec<RowView>,
        totals: Vec<TotalView>,
        tasks: Vec<TaskView>,
    },
    Unavailable {
        reason: String,
    },
}

struct ElapsedView {
    intervals_in_scope: usize,
    intervals_after_cutoff: usize,
    wall_ms: Option<u64>,
    summed_ms: Option<u64>,
    incomparable_remote_attempts: Vec<String>,
}

struct RowView {
    observation_id: String,
    metric: String,
    reported: Option<Quantity>,
    contributed: Option<Quantity>,
    basis: Option<&'static str>,
    treatment: &'static str,
    reason: Option<String>,
    allocation: AllocationView,
}

enum AllocationView {
    Task {
        plan_id: Option<String>,
        task_id: String,
    },
    Unallocated {
        reason: String,
    },
}

struct TaskView {
    plan_id: Option<String>,
    task_id: String,
    totals: Vec<TotalView>,
}

struct TotalView {
    scope: &'static str,
    basis: String,
    total: Option<Quantity>,
    complete: bool,
    reason: Option<String>,
    coverage: CoverageView,
    cutoff_unix_ms: u64,
}

struct CoverageView {
    included: usize,
    excluded: usize,
    non_additive: usize,
    unknown: usize,
    unallocated: usize,
}

struct AssessmentView {
    assessment_id: String,
    snapshot_id: String,
    author: String,
    session_id: String,
    model: String,
    recorded_at_unix_ms: u64,
    supersedes: Option<String>,
    useful_work_rule: String,
    assignments: Vec<AssignmentView>,
    residuals: Vec<ResidualView>,
    bases: Option<Vec<BasisView>>,
    bases_reason: Option<String>,
    unproven_residuals: Vec<UnprovenView>,
}

struct AssignmentView {
    contribution_id: String,
    outcome: Outcome,
    reason: String,
    evidence: Vec<String>,
}

struct ResidualView {
    parent_contribution_id: String,
    covered_contribution_ids: Vec<String>,
    outcome: Outcome,
    reason: String,
    evidence: Vec<String>,
}

struct BasisView {
    basis: String,
    total: Quantity,
    accepted: Quantity,
    support: Quantity,
    unused: Quantity,
    unresolved: Quantity,
    completeness: &'static str,
    accepted_ratio: Option<RatioView>,
    ratio_reason: Option<String>,
}

struct RatioView {
    value: String,
    completeness: &'static str,
}

struct UnprovenView {
    parent_contribution_id: String,
    reason: String,
}

/// Account capacity of the run's latest quota revision. Quota is shared account state, so it
/// is shown beside the cost rows and never adds to them.
struct AccountView {
    revision_id: Option<String>,
    revision_ids: Vec<String>,
    reason: Option<String>,
    scopes: Vec<ScopeView>,
}

/// One account scope and bucket. Every attempt that reads it is listed under this one entry.
struct ScopeView {
    scope: Vec<(&'static str, Option<String>)>,
    bucket: String,
    window: FieldView,
    utilization: FieldView,
    used: FieldView,
    remaining: FieldView,
    limit: FieldView,
    observed_at_unix_ms: Option<u64>,
    collected_at_unix_ms: u64,
    acquisition: AcquisitionView,
    freshness: &'static str,
    query: Option<QueryView>,
    attempts: Vec<LinkedAttempt>,
}

/// One quota value. A missing value keeps the state and the reason that explain it.
struct FieldView {
    state: &'static str,
    value: Option<(Value, String)>,
    reason: Option<String>,
}

struct AcquisitionView {
    state: &'static str,
    reason: Option<String>,
}

struct QueryView {
    method: String,
    duration_ms: u64,
}

/// One attempt's link to the scope it read, with its own return reference.
struct LinkedAttempt {
    attempt_id: String,
    observation_id: String,
    reference: ReferenceView,
}

struct ReferenceView {
    state: &'static str,
    revision_id: Option<String>,
    observation_digest: Option<String>,
    reason: Option<String>,
}

/// Observations of one scope and bucket. The representative is the latest one collected.
struct ScopeGroup {
    order: (u64, String),
    observation: QuotaObservation,
    attempts: Vec<LinkedAttempt>,
}

const NO_QUOTA: &str = "no quota revision is recorded for this run";

/// Handler for `gal usage ...`. Reads the store under the repository's Git common directory.
pub(crate) fn cmd_usage(args: &[String]) -> ExitCode {
    if args.get(1).map(String::as_str) == Some("collect") {
        return cmd_collect(args);
    }
    if args.get(1).map(String::as_str) == Some("assess") {
        return cmd_assess(args);
    }
    let request = match parse_request(args) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("gal usage: {message}");
            return ExitCode::Usage;
        }
    };
    let common_dir = match git_common_dir() {
        Ok(path) => path,
        Err(message) => {
            eprintln!("gal usage: {message}");
            return ExitCode::Error;
        }
    };
    let store = match UsageStore::open(&common_dir) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("gal usage: {error}");
            return ExitCode::Error;
        }
    };
    let rebuild = store_rebuild(&store, &request.run_id);
    execute(
        &store,
        &request,
        &rebuild,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

/// Rebuilds a snapshot from its stored evidence on the production report path. A snapshot
/// that cannot be rebuilt yields `None`, so the report shows no total for it.
fn store_rebuild<'a>(
    store: &'a UsageStore,
    run_id: &'a str,
) -> impl Fn(&UsageSnapshot) -> Option<Reconciliation> + 'a {
    move |snapshot| {
        store
            .rebuild_reconciliation(run_id, &snapshot.snapshot_id)
            .ok()
    }
}

/// Parses `usage report` and its options. `args[0]` is the command name.
pub(crate) fn parse_request(args: &[String]) -> Result<Request, String> {
    if args.get(1).map(String::as_str) != Some("report") {
        return Err(USAGE.to_owned());
    }
    let mut run_id: Option<String> = None;
    let mut snapshot_id: Option<String> = None;
    let mut assessment_id: Option<String> = None;
    let mut json = false;
    let mut rest = args[2..].iter();
    while let Some(flag) = rest.next() {
        let slot = match flag.as_str() {
            "--json" => {
                json = true;
                continue;
            }
            "--run" => &mut run_id,
            "--snapshot" => &mut snapshot_id,
            "--assessment" => &mut assessment_id,
            other => return Err(format!("unknown option `{other}`. {USAGE}")),
        };
        let value = rest
            .next()
            .ok_or_else(|| format!("`{flag}` needs a value. {USAGE}"))?;
        if slot.replace(value.clone()).is_some() {
            return Err(format!("`{flag}` is given more than once"));
        }
    }
    let run_id = run_id.ok_or_else(|| format!("`--run` is required. {USAGE}"))?;
    Ok(Request {
        run_id,
        snapshot_id,
        assessment_id,
        json,
    })
}

/// Builds the report and writes it. Text or one JSON document goes to `out`,
/// and every diagnostic goes to `err`.
fn execute(
    store: &UsageStore,
    request: &Request,
    rebuild: &dyn Fn(&UsageSnapshot) -> Option<Reconciliation>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitCode {
    let orchestrator = orchestrator_view(&request.run_id);
    let mut document = match build_document(store, request, rebuild) {
        Ok(document) => document,
        Err(error) => {
            // The binding line stays visible when the run has no snapshot to report.
            if let Some(view) = orchestrator.as_ref().filter(|_| !request.json) {
                let _ = writeln!(out, "{}", view.text());
            }
            let (message, code) = match error {
                ReportError::Usage(message) => (message, ExitCode::Usage),
                ReportError::Integrity(message) => (message, ExitCode::Error),
            };
            let _ = writeln!(err, "gal usage: {message}");
            return code;
        }
    };
    document.orchestrator = orchestrator;
    let rendered = if request.json {
        match serde_json::to_string_pretty(&document_value(&document)) {
            Ok(text) => format!("{text}\n"),
            Err(error) => {
                let _ = writeln!(err, "gal usage: cannot encode the report: {error}");
                return ExitCode::Error;
            }
        }
    } else {
        render_text(&document)
    };
    if out.write_all(rendered.as_bytes()).is_err() {
        return ExitCode::Error;
    }
    ExitCode::Success
}

fn build_document(
    store: &UsageStore,
    request: &Request,
    rebuild: &dyn Fn(&UsageSnapshot) -> Option<Reconciliation>,
) -> Result<Document, ReportError> {
    let run = request.run_id.as_str();
    let snapshot_ids = record_ids::<UsageSnapshot>(store, run)?;
    let snapshot_id = match &request.snapshot_id {
        Some(id) => id.clone(),
        None => latest_snapshot_id(store, run, &snapshot_ids)?,
    };
    let snapshot: UsageSnapshot = store.read(run, &snapshot_id).map_err(store_error)?;
    let digest = record_digest(&snapshot).map_err(store_error)?;

    let assessment_ids = record_ids::<AssessmentRecord>(store, run)?;
    let mut judgments = Vec::with_capacity(assessment_ids.len());
    for id in &assessment_ids {
        judgments.push(
            store
                .read::<AssessmentRecord>(run, id)
                .map_err(store_error)?,
        );
    }
    let stale_assessment_ids: Vec<String> = judgments
        .iter()
        .filter(|judgment| {
            judgment.snapshot_id == snapshot.snapshot_id && judgment.snapshot_digest != digest
        })
        .map(|judgment| judgment.assessment_id.clone())
        .collect();
    let selected = select_assessment(
        &judgments,
        &snapshot,
        &digest,
        request.assessment_id.as_deref(),
    )?;

    let rebuilt = rebuild(&snapshot);
    if let Some(reconciled) = &rebuilt {
        let rebuilt_snapshot =
            reconciled.snapshot(&snapshot.snapshot_id, snapshot.rate_set_digest.clone());
        if rebuilt_snapshot != snapshot {
            return Err(ReportError::Integrity(format!(
                "the rebuilt reconciliation does not reproduce snapshot `{}`",
                snapshot.snapshot_id
            )));
        }
    }

    let derived = match (&rebuilt, selected) {
        (Some(reconciled), Some(judgment)) => {
            Some(assess(judgment, reconciled, &snapshot).map_err(|error| error.to_string()))
        }
        _ => None,
    };
    let assessment = selected.map(|judgment| assessment_view(judgment, derived.as_ref()));
    let conflict = match &derived {
        Some(Err(message)) => Some(message.clone()),
        _ => None,
    };

    let (status, status_reason) = match (&rebuilt, conflict) {
        (_, Some(message)) => (Status::Conflict, Some(message)),
        (None, None) => (Status::Unavailable, Some(UNAVAILABLE.to_owned())),
        (Some(reconciled), None) if is_complete(reconciled) => (Status::Complete, None),
        (Some(_), None) => (Status::Partial, None),
    };
    let factual = match &rebuilt {
        Some(reconciled) => factual_view(reconciled),
        None => Factual::Unavailable {
            reason: UNAVAILABLE.to_owned(),
        },
    };

    let account = account_view(store, run)?;
    Ok(Document {
        schema_version: REPORT_SCHEMA_VERSION,
        status,
        status_reason,
        orchestrator: None,
        run_id: request.run_id.clone(),
        snapshot: SnapshotView {
            snapshot_id: snapshot.snapshot_id.clone(),
            record_digest: digest,
            reconciliation_version: snapshot.reconciliation_version,
            rate_set_digest: snapshot.rate_set_digest.clone(),
        },
        observations: snapshot.observations.clone(),
        factual,
        account,
        assessment,
        history: History {
            snapshot_ids,
            assessment_ids,
            stale_assessment_ids,
        },
    })
}

/// Picks the snapshot a report shows by default. Each capture lists every observation of the
/// run, so the snapshot with the most observations is the latest one. Ties go to the larger
/// identifier, which keeps the choice deterministic.
fn latest_snapshot_id(
    store: &UsageStore,
    run_id: &str,
    snapshot_ids: &[String],
) -> Result<String, ReportError> {
    let mut best: Option<(usize, &String)> = None;
    for id in snapshot_ids {
        let snapshot: UsageSnapshot = store.read(run_id, id).map_err(store_error)?;
        let key = (snapshot.observations.len(), id);
        if best.is_none_or(|known| key > known) {
            best = Some(key);
        }
    }
    best.map(|(_, id)| id.clone())
        .ok_or_else(|| ReportError::Usage(format!("run `{run_id}` has no published snapshot")))
}

/// Picks the assessment to show. An explicit request must judge this snapshot's exact bytes.
/// Without a request, the newest matching assessment is shown and stale ones are skipped.
fn select_assessment<'a>(
    judgments: &'a [AssessmentRecord],
    snapshot: &UsageSnapshot,
    digest: &str,
    requested: Option<&str>,
) -> Result<Option<&'a AssessmentRecord>, ReportError> {
    if let Some(id) = requested {
        let judgment = judgments
            .iter()
            .find(|judgment| judgment.assessment_id == id)
            .ok_or_else(|| ReportError::Usage(format!("no assessment `{id}` in this run")))?;
        if judgment.snapshot_id != snapshot.snapshot_id {
            return Err(ReportError::Usage(format!(
                "assessment `{id}` judges snapshot `{}`, not `{}`",
                judgment.snapshot_id, snapshot.snapshot_id
            )));
        }
        if judgment.snapshot_digest != digest {
            return Err(ReportError::Usage(format!(
                "assessment `{id}` is stale: it does not judge the current bytes of snapshot `{}`",
                snapshot.snapshot_id
            )));
        }
        return Ok(Some(judgment));
    }
    Ok(judgments
        .iter()
        .filter(|judgment| {
            judgment.snapshot_id == snapshot.snapshot_id && judgment.snapshot_digest == digest
        })
        .max_by(|left, right| {
            left.recorded_at_unix_ms
                .cmp(&right.recorded_at_unix_ms)
                .then_with(|| left.assessment_id.cmp(&right.assessment_id))
        }))
}

fn is_complete(reconciled: &Reconciliation) -> bool {
    reconciled
        .contributions
        .iter()
        .all(|row| matches!(row.treatment, Treatment::Included | Treatment::Excluded(_)))
        && totals_views("run", &reconciled.run, reconciled.cutoff_unix_ms)
            .iter()
            .all(|view| view.complete)
}

fn factual_view(reconciled: &Reconciliation) -> Factual {
    let cutoff = reconciled.cutoff_unix_ms;
    let elapsed: &ElapsedSummary = &reconciled.elapsed;
    Factual::Rebuilt {
        cutoff_unix_ms: cutoff,
        elapsed: ElapsedView {
            intervals_in_scope: elapsed.intervals_in_scope,
            intervals_after_cutoff: elapsed.intervals_after_cutoff,
            wall_ms: elapsed.wall_ms,
            summed_ms: elapsed.summed_ms,
            incomparable_remote_attempts: elapsed.incomparable_remote_attempts.clone(),
        },
        rows: reconciled.contributions.iter().map(row_view).collect(),
        totals: totals_views("run", &reconciled.run, cutoff),
        tasks: reconciled
            .tasks
            .iter()
            .map(|(key, totals)| task_view(key, totals, cutoff))
            .collect(),
    }
}

/// Reads the latest quota revision of a run and groups its attempts by account scope and bucket.
/// The report reads stored revisions only and never queries the account.
fn account_view(store: &UsageStore, run_id: &str) -> Result<AccountView, ReportError> {
    let mut revisions = Vec::new();
    for id in record_ids::<QuotaRevision>(store, run_id)? {
        revisions.push(
            store
                .read::<QuotaRevision>(run_id, &id)
                .map_err(store_error)?,
        );
    }
    revisions.sort_by_key(|revision| revision.revision);
    let revision_ids: Vec<String> = revisions
        .iter()
        .map(|revision| revision.revision_id.clone())
        .collect();
    let Some(latest) = revisions.last() else {
        return Ok(AccountView {
            revision_id: None,
            revision_ids,
            reason: Some(NO_QUOTA.to_owned()),
            scopes: Vec::new(),
        });
    };

    let mut groups: BTreeMap<(String, String), ScopeGroup> = BTreeMap::new();
    for entry in &latest.entries {
        // An attempt's reference names the revision that first listed it. Later revisions only
        // extend the chain, so the reference never moves.
        let listing = revisions
            .iter()
            .find(|revision| {
                revision
                    .entries
                    .iter()
                    .any(|known| known.attempt_id == entry.attempt_id)
            })
            .unwrap_or(latest);
        let reference = return_reference(store, listing, &entry.attempt_id).map_err(quota_error)?;
        let record: QuotaObservationRecord = store
            .read(run_id, &entry.observation_id)
            .map_err(store_error)?;
        let identity = serde_json::to_string(&record.observation.identity)
            .map_err(|error| ReportError::Integrity(error.to_string()))?;
        let key = (identity, record.observation.bucket.as_str().to_owned());
        let order = (
            record.observation.collected_at_unix_ms,
            record.observation_id.clone(),
        );
        let link = LinkedAttempt {
            attempt_id: entry.attempt_id.clone(),
            observation_id: entry.observation_id.clone(),
            reference: reference_view(&reference),
        };
        match groups.entry(key) {
            Entry::Vacant(slot) => {
                slot.insert(ScopeGroup {
                    order,
                    observation: record.observation,
                    attempts: vec![link],
                });
            }
            Entry::Occupied(mut slot) => {
                let group = slot.get_mut();
                if order > group.order {
                    group.order = order;
                    group.observation = record.observation;
                }
                group.attempts.push(link);
            }
        }
    }

    Ok(AccountView {
        revision_id: Some(latest.revision_id.clone()),
        revision_ids,
        reason: None,
        scopes: groups.into_values().map(scope_view).collect(),
    })
}

fn quota_error(error: QuotaRecordError) -> ReportError {
    match error {
        QuotaRecordError::Store(error) => store_error(error),
        other => ReportError::Integrity(other.to_string()),
    }
}

fn scope_view(mut group: ScopeGroup) -> ScopeView {
    let observation = group.observation;
    group
        .attempts
        .sort_by(|left, right| left.attempt_id.cmp(&right.attempt_id));
    ScopeView {
        scope: scope_pairs(&observation.identity),
        bucket: observation.bucket.as_str().to_owned(),
        window: field(&observation.window, window_value),
        utilization: field(&observation.utilization, utilization_value),
        used: field(&observation.used, amount_value),
        remaining: field(&observation.remaining, amount_value),
        limit: field(&observation.limit, limit_value),
        observed_at_unix_ms: observation.observed_at_unix_ms,
        collected_at_unix_ms: observation.collected_at_unix_ms,
        acquisition: acquisition_view(&observation.acquisition),
        freshness: freshness_name(&observation.freshness),
        query: observation.query.as_ref().map(|query| QueryView {
            method: query.method.clone(),
            duration_ms: query.duration_ms,
        }),
        attempts: group.attempts,
    }
}

fn scope_pairs(identity: &QuotaIdentity) -> Vec<(&'static str, Option<String>)> {
    vec![
        ("runtime", Some(runtime_name(&identity.runtime).to_owned())),
        (
            "runtime_version",
            identity
                .runtime_version
                .as_ref()
                .map(|version| version.as_str().to_owned()),
        ),
        ("provider", Some(identity.provider.as_str().to_owned())),
        ("billing", Some(billing_name(&identity.billing).to_owned())),
        (
            "account",
            identity
                .account
                .as_ref()
                .map(|account| account.as_str().to_owned()),
        ),
        (
            "identity_check",
            Some(identity_check_name(&identity.identity_check).to_owned()),
        ),
        (
            "host",
            identity.host.as_ref().map(|host| host.as_str().to_owned()),
        ),
        (
            "auth_profile",
            identity
                .auth_profile
                .as_ref()
                .map(|profile| profile.as_str().to_owned()),
        ),
        (
            "model_scope",
            identity
                .model_scope
                .as_ref()
                .map(|scope| scope.as_str().to_owned()),
        ),
    ]
}

/// Keeps the state of one value. A value that is present carries its JSON form and its text.
fn field<T>(availability: &Availability<T>, describe: fn(&T) -> (Value, String)) -> FieldView {
    let (state, value, reason) = match availability {
        Availability::Reported { value } => ("reported", Some(describe(value)), None),
        Availability::Partial { value, reason } => {
            ("partial", Some(describe(value)), Some(reason.clone()))
        }
        Availability::Unsupported { reason } => ("unsupported", None, Some(reason.clone())),
        Availability::Unavailable { reason } => ("unavailable", None, Some(reason.clone())),
        Availability::Malformed { reason } => ("malformed", None, Some(reason.clone())),
    };
    FieldView {
        state,
        value,
        reason,
    }
}

fn window_value(window: &QuotaWindow) -> (Value, String) {
    let kind = window_name(&window.kind);
    let duration = window.duration_minutes.map(u64::from);
    (
        json!({
            "kind": kind,
            "duration_minutes": duration,
            "resets_at_unix_ms": window.resets_at_unix_ms,
        }),
        format!(
            "{kind} duration_minutes={} resets_at_unix_ms={}",
            optional(duration),
            optional(window.resets_at_unix_ms)
        ),
    )
}

fn utilization_value(utilization: &Utilization) -> (Value, String) {
    let value = utilization.value().to_string();
    let scale = scale_name(utilization.scale());
    let direction = direction_name(utilization.direction());
    (
        json!({
            "value": value,
            "scale": scale,
            "direction": direction,
        }),
        format!("{value} {scale} {direction}"),
    )
}

fn amount_value(amount: &QuotaAmount) -> (Value, String) {
    let value = amount.value().to_string();
    let unit = quota_unit_name(amount.unit());
    (
        json!({
            "value": value,
            "unit": unit,
        }),
        format!("{value} {unit}"),
    )
}

fn limit_value(limit: &QuotaLimit) -> (Value, String) {
    match limit {
        QuotaLimit::Limited { amount } => {
            let (value, text) = amount_value(amount);
            (
                json!({ "kind": "limited", "amount": value }),
                format!("limit {text}"),
            )
        }
        QuotaLimit::Unlimited { claim } => {
            let basis = unlimited_name(claim.basis);
            let source = claim.source_field.as_str();
            (
                json!({ "kind": "unlimited", "basis": basis, "source_field": source }),
                format!("unlimited basis={basis} source_field={source}"),
            )
        }
    }
}

fn acquisition_view(acquisition: &Acquisition) -> AcquisitionView {
    let (state, reason) = match acquisition {
        Acquisition::Passive => ("passive", None),
        Acquisition::Queried => ("queried", None),
        Acquisition::Failed { reason } => ("failed", Some(reason.clone())),
        Acquisition::TimedOut { reason } => ("timed_out", Some(reason.clone())),
        Acquisition::Unsupported { reason } => ("unsupported", Some(reason.clone())),
    };
    AcquisitionView { state, reason }
}

fn reference_view(snapshot: &QuotaSnapshot) -> ReferenceView {
    let (state, value, reason) = match &snapshot.reference {
        Availability::Reported { value } => ("reported", Some(value), None),
        Availability::Partial { value, reason } => ("partial", Some(value), Some(reason.clone())),
        Availability::Unsupported { reason } => ("unsupported", None, Some(reason.clone())),
        Availability::Unavailable { reason } => ("unavailable", None, Some(reason.clone())),
        Availability::Malformed { reason } => ("malformed", None, Some(reason.clone())),
    };
    ReferenceView {
        state,
        revision_id: value.map(|reference| reference.snapshot_id.as_str().to_owned()),
        observation_digest: value.map(|reference| reference.observation_digest.as_str().to_owned()),
        reason,
    }
}

/// Encodes the account section. Its cost flag is fixed: quota never adds to a cost total.
fn account_value(account: &AccountView) -> Value {
    json!({
        "contributes_to_cost": false,
        "revision_id": account.revision_id,
        "revision_ids": account.revision_ids,
        "reason": account.reason,
        "scopes": account.scopes.iter().map(scope_value).collect::<Vec<Value>>(),
    })
}

fn scope_value(scope: &ScopeView) -> Value {
    let scope_fields: serde_json::Map<String, Value> = scope
        .scope
        .iter()
        .map(|(name, value)| ((*name).to_owned(), json!(value)))
        .collect();
    json!({
        "scope": scope_fields,
        "bucket": scope.bucket,
        "window": field_value(&scope.window),
        "utilization": field_value(&scope.utilization),
        "used": field_value(&scope.used),
        "remaining": field_value(&scope.remaining),
        "limit": field_value(&scope.limit),
        "observed_at_unix_ms": scope.observed_at_unix_ms,
        "collected_at_unix_ms": scope.collected_at_unix_ms,
        "acquisition": {
            "state": scope.acquisition.state,
            "reason": scope.acquisition.reason,
        },
        "freshness": scope.freshness,
        "control_query": scope.query.as_ref().map(|query| json!({
            "method": query.method,
            "duration_ms": query.duration_ms,
        })),
        "attempts": scope
            .attempts
            .iter()
            .map(|attempt| json!({
                "attempt_id": attempt.attempt_id,
                "observation_id": attempt.observation_id,
                "reference": {
                    "state": attempt.reference.state,
                    "revision_id": attempt.reference.revision_id,
                    "observation_digest": attempt.reference.observation_digest,
                    "reason": attempt.reference.reason,
                },
            }))
            .collect::<Vec<Value>>(),
    })
}

fn field_value(field: &FieldView) -> Value {
    json!({
        "state": field.state,
        "value": field.value.as_ref().map(|(value, _)| value),
        "reason": field.reason,
    })
}

fn row_view(row: &Contribution) -> RowView {
    let (treatment, reason) = match &row.treatment {
        Treatment::Included => ("included", None),
        Treatment::Excluded(reason) => ("excluded", Some(reason.clone())),
        Treatment::NonAdditive(reason) => ("non_additive", Some(reason.clone())),
        Treatment::Unknown(reason) => ("unknown", Some(reason.clone())),
    };
    RowView {
        observation_id: row.observation_id.clone(),
        metric: metric_name(&row.metric),
        reported: row.reported.as_ref().map(Quantity::of_amount),
        contributed: row.contributed.as_ref().map(Quantity::of_amount),
        basis: row.basis.map(|basis| match basis {
            Basis::Delta => "delta",
            Basis::CumulativeDelta => "cumulative_delta",
        }),
        treatment,
        reason,
        allocation: match &row.allocation {
            Allocation::Task(key) => AllocationView::Task {
                plan_id: key.plan_id.clone(),
                task_id: key.task_id.clone(),
            },
            Allocation::Unallocated(reason) => AllocationView::Unallocated {
                reason: reason.clone(),
            },
        },
    }
}

fn task_view(key: &TaskKey, totals: &Totals, cutoff: u64) -> TaskView {
    TaskView {
        plan_id: key.plan_id.clone(),
        task_id: key.task_id.clone(),
        totals: totals_views("task", totals, cutoff),
    }
}

/// Token and money subtotals of one scope. A scope with no money rows still
/// reports an unavailable cost total, so the total is never silently absent.
fn totals_views(scope: &'static str, totals: &Totals, cutoff: u64) -> Vec<TotalView> {
    let mut views: Vec<TotalView> = totals
        .tokens
        .iter()
        .zip(TOKEN_CATEGORIES.iter())
        .map(|(subtotal, category)| {
            total_view(
                scope,
                format!("tokens.{}", category_name(category)),
                subtotal,
                |count| Quantity::Tokens(*count),
            )
        })
        .collect();
    if totals.cost.is_empty() {
        views.push(TotalView {
            scope,
            basis: "cost".to_owned(),
            total: None,
            complete: false,
            reason: Some("no cost observations in scope".to_owned()),
            coverage: coverage_view(&Coverage::default()),
            cutoff_unix_ms: cutoff,
        });
    }
    for cost in &totals.cost {
        views.push(total_view(
            scope,
            cost_basis(cost.unit.as_ref(), cost.kind.as_ref()),
            &cost.subtotal,
            Quantity::of_money,
        ));
    }
    views
}

fn total_view<V>(
    scope: &'static str,
    basis: String,
    subtotal: &Subtotal<V>,
    quantity: impl Fn(&V) -> Quantity,
) -> TotalView {
    TotalView {
        scope,
        basis,
        total: subtotal.total.as_ref().map(quantity),
        complete: subtotal.complete,
        reason: subtotal.reason.clone(),
        coverage: coverage_view(&subtotal.coverage),
        cutoff_unix_ms: subtotal.cutoff_unix_ms,
    }
}

fn coverage_view(coverage: &Coverage) -> CoverageView {
    CoverageView {
        included: coverage.included,
        excluded: coverage.excluded,
        non_additive: coverage.non_additive,
        unknown: coverage.unknown,
        unallocated: coverage.unallocated,
    }
}

fn assessment_view(
    judgment: &AssessmentRecord,
    derived: Option<&Result<Assessment, String>>,
) -> AssessmentView {
    let (bases, bases_reason, unproven_residuals) = match derived {
        None => (None, Some(UNAVAILABLE.to_owned()), Vec::new()),
        Some(Ok(assessment)) => (
            Some(assessment.bases.iter().map(basis_view).collect()),
            None,
            assessment
                .unproven_residuals
                .iter()
                .map(|residual| UnprovenView {
                    parent_contribution_id: residual.parent_contribution_id.clone(),
                    reason: residual.reason.clone(),
                })
                .collect(),
        ),
        Some(Err(message)) => (None, Some(message.clone()), Vec::new()),
    };
    AssessmentView {
        assessment_id: judgment.assessment_id.clone(),
        snapshot_id: judgment.snapshot_id.clone(),
        author: judgment.author.clone(),
        session_id: judgment.session_id.clone(),
        model: judgment.model.clone(),
        recorded_at_unix_ms: judgment.recorded_at_unix_ms,
        supersedes: judgment.supersedes.clone(),
        useful_work_rule: judgment.useful_work_rule.clone(),
        assignments: judgment
            .assignments
            .iter()
            .map(|assignment| AssignmentView {
                contribution_id: assignment.contribution_id.clone(),
                outcome: assignment.outcome,
                reason: assignment.reason.clone(),
                evidence: assignment.evidence.clone(),
            })
            .collect(),
        residuals: judgment
            .residuals
            .iter()
            .map(|residual| ResidualView {
                parent_contribution_id: residual.parent_contribution_id.clone(),
                covered_contribution_ids: residual.covered_contribution_ids.clone(),
                outcome: residual.outcome,
                reason: residual.reason.clone(),
                evidence: residual.evidence.clone(),
            })
            .collect(),
        bases,
        bases_reason,
        unproven_residuals,
    }
}

fn basis_view(basis: &BasisAssessment) -> BasisView {
    let ratio_reason = basis
        .accepted_ratio
        .is_none()
        .then(|| "the total is zero, so the accepted share is unavailable".to_owned());
    BasisView {
        basis: basis_name(&basis.basis),
        total: Quantity::of_amount(&basis.total),
        accepted: Quantity::of_amount(&basis.accepted),
        support: Quantity::of_amount(&basis.support),
        unused: Quantity::of_amount(&basis.unused),
        unresolved: Quantity::of_amount(&basis.unresolved),
        completeness: completeness_name(basis.completeness),
        accepted_ratio: basis.accepted_ratio.as_ref().map(|ratio| RatioView {
            value: ratio.value.to_string(),
            completeness: completeness_name(ratio.completeness),
        }),
        ratio_reason,
    }
}

/// Encodes the report as one JSON document. Each missing value is null beside its reason.
fn document_value(document: &Document) -> Value {
    json!({
        "schema_version": document.schema_version,
        "status": status_name(document.status),
        "status_reason": document.status_reason,
        "run_id": document.run_id,
        "orchestrator": document.orchestrator.as_ref().map(OrchestratorView::json),
        "snapshot": {
            "snapshot_id": document.snapshot.snapshot_id,
            "record_digest": document.snapshot.record_digest,
            "reconciliation_version": document.snapshot.reconciliation_version,
            "rate_set_digest": document.snapshot.rate_set_digest,
        },
        "observations": document
            .observations
            .iter()
            .map(|observation| json!({
                "observation_id": observation.observation_id,
                "digest": observation.digest,
            }))
            .collect::<Vec<Value>>(),
        "factual": factual_value(&document.factual),
        "account": account_value(&document.account),
        "assessment": document.assessment.as_ref().map(assessment_value),
        "history": {
            "snapshot_ids": document.history.snapshot_ids,
            "assessment_ids": document.history.assessment_ids,
            "stale_assessment_ids": document.history.stale_assessment_ids,
        },
    })
}

fn factual_value(factual: &Factual) -> Value {
    match factual {
        Factual::Unavailable { reason } => json!({
            "state": "unavailable",
            "reason": reason,
        }),
        Factual::Rebuilt {
            cutoff_unix_ms,
            elapsed,
            rows,
            totals,
            tasks,
        } => json!({
            "state": "rebuilt",
            "cutoff_unix_ms": cutoff_unix_ms,
            "elapsed": {
                "intervals_in_scope": elapsed.intervals_in_scope,
                "intervals_after_cutoff": elapsed.intervals_after_cutoff,
                "wall_ms": elapsed.wall_ms,
                "summed_ms": elapsed.summed_ms,
                "incomparable_remote_attempts": elapsed.incomparable_remote_attempts,
            },
            "rows": rows.iter().map(row_value).collect::<Vec<Value>>(),
            "totals": totals.iter().map(total_value).collect::<Vec<Value>>(),
            "tasks": tasks
                .iter()
                .map(|task| json!({
                    "plan_id": task.plan_id,
                    "task_id": task.task_id,
                    "totals": task.totals.iter().map(total_value).collect::<Vec<Value>>(),
                }))
                .collect::<Vec<Value>>(),
        }),
    }
}

fn row_value(row: &RowView) -> Value {
    let allocation = match &row.allocation {
        AllocationView::Task { plan_id, task_id } => json!({
            "kind": "task",
            "plan_id": plan_id,
            "task_id": task_id,
        }),
        AllocationView::Unallocated { reason } => json!({
            "kind": "unallocated",
            "reason": reason,
        }),
    };
    json!({
        "observation_id": row.observation_id,
        "metric": row.metric,
        "reported": row.reported.as_ref().map(quantity_value),
        "contributed": row.contributed.as_ref().map(quantity_value),
        "basis": row.basis,
        "treatment": row.treatment,
        "reason": row.reason,
        "allocation": allocation,
    })
}

fn total_value(total: &TotalView) -> Value {
    json!({
        "scope": total.scope,
        "basis": total.basis,
        "total": total.total.as_ref().map(quantity_value),
        "complete": total.complete,
        "reason": total.reason,
        "coverage": {
            "included": total.coverage.included,
            "excluded": total.coverage.excluded,
            "non_additive": total.coverage.non_additive,
            "unknown": total.coverage.unknown,
            "unallocated": total.coverage.unallocated,
        },
        "cutoff_unix_ms": total.cutoff_unix_ms,
    })
}

fn assessment_value(assessment: &AssessmentView) -> Value {
    json!({
        "assessment_id": assessment.assessment_id,
        "snapshot_id": assessment.snapshot_id,
        "author": assessment.author,
        "session_id": assessment.session_id,
        "model": assessment.model,
        "recorded_at_unix_ms": assessment.recorded_at_unix_ms,
        "supersedes": assessment.supersedes,
        "useful_work_rule": assessment.useful_work_rule,
        "assignments": assessment
            .assignments
            .iter()
            .map(|assignment| json!({
                "contribution_id": assignment.contribution_id,
                "outcome": outcome_name(assignment.outcome),
                "reason": assignment.reason,
                "evidence": assignment.evidence,
            }))
            .collect::<Vec<Value>>(),
        "residuals": assessment
            .residuals
            .iter()
            .map(|residual| json!({
                "parent_contribution_id": residual.parent_contribution_id,
                "covered_contribution_ids": residual.covered_contribution_ids,
                "outcome": outcome_name(residual.outcome),
                "reason": residual.reason,
                "evidence": residual.evidence,
            }))
            .collect::<Vec<Value>>(),
        "bases": assessment
            .bases
            .as_ref()
            .map(|bases| bases.iter().map(basis_value).collect::<Vec<Value>>()),
        "bases_reason": assessment.bases_reason,
        "unproven_residuals": assessment
            .unproven_residuals
            .iter()
            .map(|residual| json!({
                "parent_contribution_id": residual.parent_contribution_id,
                "reason": residual.reason,
            }))
            .collect::<Vec<Value>>(),
    })
}

fn basis_value(basis: &BasisView) -> Value {
    json!({
        "basis": basis.basis,
        "total": quantity_value(&basis.total),
        "accepted": quantity_value(&basis.accepted),
        "support": quantity_value(&basis.support),
        "unused": quantity_value(&basis.unused),
        "unresolved": quantity_value(&basis.unresolved),
        "completeness": basis.completeness,
        "accepted_ratio": basis.accepted_ratio.as_ref().map(|ratio| json!({
            "value": ratio.value,
            "completeness": ratio.completeness,
        })),
        "ratio_reason": basis.ratio_reason,
    })
}

fn quantity_value(quantity: &Quantity) -> Value {
    match quantity {
        Quantity::Tokens(count) => json!(count),
        Quantity::Money { amount, unit, kind } => json!({
            "amount": amount,
            "unit": unit,
            "kind": kind,
        }),
    }
}

/// Orchestrator binding of a run, or the reason it has none, as the report shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum OrchestratorView {
    Bound {
        host_session: String,
        window_started_at: String,
    },
    Unavailable {
        reason: String,
    },
}

impl OrchestratorView {
    fn text(&self) -> String {
        match self {
            Self::Bound {
                host_session,
                window_started_at,
            } => format!("orchestrator: bound to {host_session} from {window_started_at}"),
            Self::Unavailable { reason } => format!("orchestrator: unavailable, because {reason}"),
        }
    }

    fn json(&self) -> Value {
        match self {
            Self::Bound {
                host_session,
                window_started_at,
            } => json!({
                "state": "bound",
                "host_session": host_session,
                "window_started_at": window_started_at,
            }),
            Self::Unavailable { reason } => json!({
                "state": "unavailable",
                "reason": reason,
            }),
        }
    }
}

/// Reads the run's orchestrator binding from its preflight receipt. A run without a receipt has
/// no view, and an unreadable receipt is reported as unavailable rather than hidden.
fn orchestrator_view(run_id: &str) -> Option<OrchestratorView> {
    match pipeline_preflight::read_orchestrator_record(run_id) {
        Ok(None) => None,
        Ok(Some(OrchestratorRecord::Bound(recorded))) => Some(OrchestratorView::Bound {
            host_session: recorded.host_session,
            window_started_at: recorded.window_started_at,
        }),
        Ok(Some(OrchestratorRecord::Unavailable { reason })) => {
            Some(OrchestratorView::Unavailable { reason })
        }
        Err(error) => Some(OrchestratorView::Unavailable {
            reason: format!("the preflight receipt is unreadable: {error}"),
        }),
    }
}

fn render_text(document: &Document) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "gal usage report");
    let _ = writeln!(text, "run: {}", document.run_id);
    if let Some(view) = &document.orchestrator {
        let _ = writeln!(text, "{}", view.text());
    }
    let _ = writeln!(text, "status: {}", status_name(document.status));
    if let Some(reason) = &document.status_reason {
        let _ = writeln!(text, "  reason: {reason}");
    }
    let snapshot = &document.snapshot;
    let _ = writeln!(text, "snapshot: {}", snapshot.snapshot_id);
    let _ = writeln!(text, "  record digest: {}", snapshot.record_digest);
    let _ = writeln!(
        text,
        "  reconciliation version: {}",
        snapshot.reconciliation_version
    );
    let _ = writeln!(
        text,
        "  rate set digest: {}",
        snapshot.rate_set_digest.as_deref().unwrap_or("unpriced")
    );
    let _ = writeln!(
        text,
        "snapshots in run: {}",
        listed(&document.history.snapshot_ids)
    );
    let _ = writeln!(
        text,
        "assessments in run: {}",
        listed(&document.history.assessment_ids)
    );
    let _ = writeln!(
        text,
        "stale assessments not applied: {}",
        listed(&document.history.stale_assessment_ids)
    );

    let _ = writeln!(text);
    let _ = writeln!(text, "observations ({}):", document.observations.len());
    if document.observations.is_empty() {
        let _ = writeln!(text, "  none");
    }
    for observation in &document.observations {
        let _ = writeln!(
            text,
            "  {} digest {}",
            observation.observation_id, observation.digest
        );
    }

    let _ = writeln!(text);
    let _ = writeln!(text, "factual rows:");
    match &document.factual {
        Factual::Unavailable { reason } => {
            let _ = writeln!(text, "  unavailable: {reason}");
        }
        Factual::Rebuilt {
            cutoff_unix_ms,
            elapsed,
            rows,
            totals,
            tasks,
        } => {
            let _ = writeln!(text, "  cutoff_unix_ms: {cutoff_unix_ms}");
            let _ = writeln!(
                text,
                "  intervals in scope: {}, after cutoff: {}, wall_ms: {}, summed_ms: {}",
                elapsed.intervals_in_scope,
                elapsed.intervals_after_cutoff,
                optional(elapsed.wall_ms),
                optional(elapsed.summed_ms),
            );
            let _ = writeln!(
                text,
                "  incomparable remote attempts: {}",
                listed(&elapsed.incomparable_remote_attempts)
            );
            let _ = writeln!(text, "  rows ({}):", rows.len());
            for row in rows {
                let _ = writeln!(
                    text,
                    "    {} {} treatment={} reported={} contributed={} basis={} allocation={} reason={}",
                    row.observation_id,
                    row.metric,
                    row.treatment,
                    quantity_text(row.reported.as_ref()),
                    quantity_text(row.contributed.as_ref()),
                    row.basis.unwrap_or("none"),
                    allocation_text(&row.allocation),
                    row.reason.as_deref().unwrap_or("none"),
                );
            }
            let _ = writeln!(text, "  totals (run):");
            for total in totals {
                let _ = writeln!(text, "    {}", total_text(total));
            }
            for task in tasks {
                let plan = task.plan_id.as_deref().unwrap_or("-");
                let _ = writeln!(text, "  totals (task {plan}/{}):", task.task_id);
                for total in &task.totals {
                    let _ = writeln!(text, "    {}", total_text(total));
                }
            }
        }
    }

    let _ = writeln!(text);
    render_account(&mut text, &document.account);

    let _ = writeln!(text);
    let _ = writeln!(text, "assessment:");
    match &document.assessment {
        None => {
            let _ = writeln!(text, "  none for this snapshot");
        }
        Some(assessment) => render_assessment(&mut text, assessment),
    }
    text
}

fn render_assessment(text: &mut String, assessment: &AssessmentView) {
    let _ = writeln!(text, "  id: {}", assessment.assessment_id);
    let _ = writeln!(text, "  snapshot: {}", assessment.snapshot_id);
    let _ = writeln!(
        text,
        "  author: {}, session: {}, model: {}",
        assessment.author, assessment.session_id, assessment.model
    );
    let _ = writeln!(
        text,
        "  recorded_at_unix_ms: {}, supersedes: {}",
        assessment.recorded_at_unix_ms,
        assessment.supersedes.as_deref().unwrap_or("none")
    );
    let _ = writeln!(text, "  useful work rule: {}", assessment.useful_work_rule);
    let _ = writeln!(text, "  assignments ({}):", assessment.assignments.len());
    for assignment in &assessment.assignments {
        let _ = writeln!(
            text,
            "    {} outcome={} reason={}",
            assignment.contribution_id,
            outcome_name(assignment.outcome),
            assignment.reason
        );
    }
    let _ = writeln!(text, "  residuals ({}):", assessment.residuals.len());
    for residual in &assessment.residuals {
        let _ = writeln!(
            text,
            "    parent {} covers {} outcome={} reason={}",
            residual.parent_contribution_id,
            listed(&residual.covered_contribution_ids),
            outcome_name(residual.outcome),
            residual.reason
        );
    }
    match (&assessment.bases, &assessment.bases_reason) {
        (Some(bases), _) => {
            let _ = writeln!(text, "  bases ({}):", bases.len());
            for basis in bases {
                let _ = writeln!(
                    text,
                    "    {} total={} accepted={} support={} unused={} unresolved={} completeness={} accepted_share={}",
                    basis.basis,
                    basis.total.render(),
                    basis.accepted.render(),
                    basis.support.render(),
                    basis.unused.render(),
                    basis.unresolved.render(),
                    basis.completeness,
                    match (&basis.accepted_ratio, &basis.ratio_reason) {
                        (Some(ratio), _) => format!("{} ({})", ratio.value, ratio.completeness),
                        (None, Some(reason)) => format!("unavailable: {reason}"),
                        (None, None) => "unavailable".to_owned(),
                    },
                );
            }
        }
        (None, reason) => {
            let _ = writeln!(
                text,
                "  bases: unavailable: {}",
                reason.as_deref().unwrap_or("not derived")
            );
        }
    }
    let _ = writeln!(
        text,
        "  unproven residuals ({}):",
        assessment.unproven_residuals.len()
    );
    for residual in &assessment.unproven_residuals {
        let _ = writeln!(
            text,
            "    {}: {}",
            residual.parent_contribution_id, residual.reason
        );
    }
}

fn render_account(text: &mut String, account: &AccountView) {
    let _ = writeln!(
        text,
        "account capacity (shared quota, not a cost contribution):"
    );
    let Some(revision) = &account.revision_id else {
        let _ = writeln!(
            text,
            "  unavailable: {}",
            account.reason.as_deref().unwrap_or("not recorded")
        );
        return;
    };
    let _ = writeln!(
        text,
        "  revision: {revision}, chain: {}",
        listed(&account.revision_ids)
    );
    let _ = writeln!(text, "  scopes ({}):", account.scopes.len());
    for scope in &account.scopes {
        let _ = writeln!(text, "    scope: {}", scope_text(&scope.scope));
        let _ = writeln!(text, "      bucket: {}", scope.bucket);
        let _ = writeln!(text, "      window: {}", field_text(&scope.window));
        let _ = writeln!(
            text,
            "      utilization: {}",
            field_text(&scope.utilization)
        );
        let _ = writeln!(text, "      used: {}", field_text(&scope.used));
        let _ = writeln!(text, "      remaining: {}", field_text(&scope.remaining));
        let _ = writeln!(text, "      limit: {}", field_text(&scope.limit));
        let _ = writeln!(
            text,
            "      observed_at_unix_ms: {}, collected_at_unix_ms: {}",
            scope
                .observed_at_unix_ms
                .map_or_else(|| "not recorded".to_owned(), |ms| ms.to_string()),
            scope.collected_at_unix_ms
        );
        let _ = writeln!(
            text,
            "      acquisition: {}, freshness: {}",
            acquisition_text(&scope.acquisition),
            scope.freshness
        );
        let _ = writeln!(
            text,
            "      control query: {}",
            scope.query.as_ref().map_or_else(
                || "none".to_owned(),
                |query| format!("{} in {} ms", query.method, query.duration_ms)
            )
        );
        let _ = writeln!(text, "      linked attempts ({}):", scope.attempts.len());
        for attempt in &scope.attempts {
            let _ = writeln!(
                text,
                "        {} observation={} reference={} revision={} digest={} reason={}",
                attempt.attempt_id,
                attempt.observation_id,
                attempt.reference.state,
                attempt.reference.revision_id.as_deref().unwrap_or("none"),
                attempt
                    .reference
                    .observation_digest
                    .as_deref()
                    .unwrap_or("none"),
                attempt.reference.reason.as_deref().unwrap_or("none"),
            );
        }
    }
}

fn field_text(field: &FieldView) -> String {
    match (&field.value, &field.reason) {
        (Some((_, text)), None) => format!("{} {text}", field.state),
        (Some((_, text)), Some(reason)) => format!("{} {text}, reason: {reason}", field.state),
        (None, reason) => format!(
            "{}, reason: {}",
            field.state,
            reason.as_deref().unwrap_or("not recorded")
        ),
    }
}

fn acquisition_text(acquisition: &AcquisitionView) -> String {
    match &acquisition.reason {
        Some(reason) => format!("{} ({reason})", acquisition.state),
        None => acquisition.state.to_owned(),
    }
}

fn scope_text(scope: &[(&'static str, Option<String>)]) -> String {
    scope
        .iter()
        .map(|(name, value)| format!("{name}={}", value.as_deref().unwrap_or("none")))
        .collect::<Vec<String>>()
        .join(" ")
}

fn runtime_name(runtime: &QuotaRuntime) -> &'static str {
    match runtime {
        QuotaRuntime::Codex => "codex",
        QuotaRuntime::Claude => "claude",
        QuotaRuntime::Copilot => "copilot",
        QuotaRuntime::Antigravity => "antigravity",
        QuotaRuntime::OpenCode => "open_code",
    }
}

fn billing_name(billing: &BillingMode) -> &'static str {
    match billing {
        BillingMode::Subscription => "subscription",
        BillingMode::EnterpriseProject => "enterprise_project",
        BillingMode::ApiKey => "api_key",
        BillingMode::Unknown => "unknown",
    }
}

fn identity_check_name(check: &AccountIdentityCheck) -> &'static str {
    match check {
        AccountIdentityCheck::Verified => "verified",
        AccountIdentityCheck::Unverified => "unverified",
        AccountIdentityCheck::Mismatch => "mismatch",
    }
}

fn freshness_name(freshness: &Freshness) -> &'static str {
    match freshness {
        Freshness::Current => "current",
        Freshness::Stale => "stale",
        Freshness::Unknown => "unknown",
    }
}

fn window_name(kind: &WindowKind) -> String {
    match kind {
        WindowKind::FiveHour => "five_hour".to_owned(),
        WindowKind::Weekly => "weekly".to_owned(),
        WindowKind::Monthly => "monthly".to_owned(),
        WindowKind::RequestRate => "request_rate".to_owned(),
        WindowKind::Named { name } => format!("named:{}", name.as_str()),
    }
}

fn scale_name(scale: UtilizationScale) -> &'static str {
    match scale {
        UtilizationScale::Fraction => "fraction",
        UtilizationScale::Percent => "percent",
    }
}

fn direction_name(direction: UtilizationDirection) -> &'static str {
    match direction {
        UtilizationDirection::Used => "used",
        UtilizationDirection::Remaining => "remaining",
    }
}

fn quota_unit_name(unit: &QuotaUnit) -> String {
    match unit {
        QuotaUnit::Requests => "requests".to_owned(),
        QuotaUnit::Tokens => "tokens".to_owned(),
        QuotaUnit::Credit(credit) => format!("credit:{}", credit.as_str()),
    }
}

fn unlimited_name(basis: UnlimitedBasis) -> &'static str {
    match basis {
        UnlimitedBasis::ProviderFlag => "provider_flag",
        UnlimitedBasis::DocumentedSentinel => "documented_sentinel",
    }
}

fn total_text(total: &TotalView) -> String {
    format!(
        "{} {}: total={} complete={} reason={} coverage(included={}, excluded={}, non_additive={}, unknown={}, unallocated={}) cutoff_unix_ms={}",
        total.scope,
        total.basis,
        total
            .total
            .as_ref()
            .map_or_else(|| "unavailable".to_owned(), Quantity::render),
        if total.complete { "yes" } else { "no" },
        total.reason.as_deref().unwrap_or("none"),
        total.coverage.included,
        total.coverage.excluded,
        total.coverage.non_additive,
        total.coverage.unknown,
        total.coverage.unallocated,
        total.cutoff_unix_ms,
    )
}

fn allocation_text(allocation: &AllocationView) -> String {
    match allocation {
        AllocationView::Task { plan_id, task_id } => match plan_id {
            Some(plan) => format!("task {plan}/{task_id}"),
            None => format!("task {task_id}"),
        },
        AllocationView::Unallocated { reason } => format!("unallocated ({reason})"),
    }
}

fn quantity_text(quantity: Option<&Quantity>) -> String {
    quantity.map_or_else(|| "none".to_owned(), Quantity::render)
}

fn optional(value: Option<u64>) -> String {
    value.map_or_else(|| "unavailable".to_owned(), |value| value.to_string())
}

fn listed(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(", ")
    }
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Complete => "complete",
        Status::Partial => "partial",
        Status::Unavailable => "unavailable",
        Status::Conflict => "conflict",
    }
}

fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Accepted => "accepted",
        Outcome::Support => "support",
        Outcome::Unused => "unused",
    }
}

fn completeness_name(completeness: Completeness) -> &'static str {
    match completeness {
        Completeness::Complete => "complete",
        Completeness::KnownSubtotal => "known_subtotal",
    }
}

fn category_name(category: &UsageCategory) -> &'static str {
    match category {
        UsageCategory::Input => "input",
        UsageCategory::Output => "output",
        UsageCategory::CachedInput => "cached_input",
        UsageCategory::CacheWrite => "cache_write",
        UsageCategory::Reasoning => "reasoning",
        UsageCategory::Other => "other",
    }
}

fn kind_name(kind: &MoneyKind) -> &'static str {
    match kind {
        MoneyKind::ProviderReported => "provider_reported",
        MoneyKind::Estimated => "estimated",
        MoneyKind::ConfirmedCharge => "confirmed_charge",
    }
}

fn unit_name(unit: &MoneyUnit) -> &str {
    match unit {
        MoneyUnit::Currency(code) => code.as_str(),
        MoneyUnit::Credit(unit) => unit.as_str(),
    }
}

fn metric_name(metric: &MetricKey) -> String {
    match metric {
        MetricKey::Tokens(category) => format!("tokens.{}", category_name(category)),
        MetricKey::Cost => "cost".to_owned(),
    }
}

fn basis_name(basis: &BasisKey) -> String {
    match basis {
        BasisKey::Tokens(category) => format!("tokens.{}", category_name(category)),
        BasisKey::Money { unit, kind } => {
            format!("cost {} {}", unit_name(unit), kind_name(kind))
        }
    }
}

fn cost_basis(unit: Option<&MoneyUnit>, kind: Option<&MoneyKind>) -> String {
    match (unit, kind) {
        (Some(unit), Some(kind)) => format!("cost {} {}", unit_name(unit), kind_name(kind)),
        _ => "cost (unit or kind unknown)".to_owned(),
    }
}

fn record_ids<R: StoredRecord>(
    store: &UsageStore,
    run_id: &str,
) -> Result<Vec<String>, ReportError> {
    usage_capture::record_ids::<R>(store, run_id).map_err(store_error)
}

/// Maps a capture or shared publication failure. Store failures keep their store mapping.
fn capture_error(error: CaptureError) -> ReportError {
    match error {
        CaptureError::Store(error) => store_error(error),
        other => ReportError::Integrity(other.to_string()),
    }
}

/// Maps store failures to exit classes. Bad identifiers and missing records are
/// invalid selections. Anything else means the store cannot be trusted.
fn store_error(error: StoreError) -> ReportError {
    match error {
        StoreError::InvalidId(id) => {
            ReportError::Usage(format!("`{id}` is not a valid identifier"))
        }
        StoreError::NotFound(id) => {
            ReportError::Usage(format!("no published record `{id}` in this run"))
        }
        other => ReportError::Integrity(other.to_string()),
    }
}

/// Resolves the Git common directory that holds the usage store.
fn git_common_dir() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("cannot resolve the working directory: {error}"))?;
    git_common_dir_in(&cwd)
}

/// Resolves the Git common directory of the repository that contains `workdir`.
pub(crate) fn git_common_dir_in(workdir: &Path) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(workdir)
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err("not inside a git repository".to_owned());
    }
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(workdir.join(path))
}

/// Most native pages one collection reads. A source that keeps growing stops here,
/// and the output names the cursor that resumes it.
const COLLECT_MAX_PAGES: usize = 64;

const COLLECT_USAGE: &str =
    "usage: gal usage collect --run <run-id> --binding <path> [--rates <path>]";

/// Billing mode of an orchestrator binding that preflight records. Preflight does not know the
/// billing mode, so a rate set prices this binding only when it names `unspecified`.
const ORCHESTRATOR_BILLING_MODE: &str = "unspecified";

/// Top-level fields a binding may carry. Anything else, including counters, is rejected.
const BINDING_FIELDS: [&str; 6] = [
    "run",
    "host_session",
    "billing_mode",
    "role",
    "window",
    "cursor",
];

/// Parsed `gal usage collect` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectRequest {
    pub(crate) run_id: String,
    /// Binding file. When absent, collection reads the binding that the run's preflight recorded.
    pub(crate) binding_path: Option<PathBuf>,
    pub(crate) rates_path: Option<PathBuf>,
}

/// Strict binding of one run to one native source. It carries identity, source,
/// and window only, never submitted counters.
#[derive(Debug)]
struct Binding {
    /// Digest of the run identity. Observations of this binding carry its prefix.
    identity_digest: String,
    reference: HostSessionRef,
    role: HostUsageRole,
    window: UsageWindow,
    cursor: Option<HostUsageCursor>,
    billing_mode: String,
    workdir: String,
    /// Run identity the binding names. Quota the source reads binds to it.
    identity: RunIdentity,
    /// Host manifest task and phase of a child binding. They fill the null task and phase
    /// of each child observation. A binding file names neither.
    task_id: Option<String>,
    phase: Option<String>,
}

/// One observation read from the source, with the identifier and digest a snapshot names.
struct SourceObservation {
    observation_id: String,
    digest: String,
    observation: UsageObservation,
}

/// Everything one bounded collection read from the native source.
struct SourceRead {
    observations: Vec<SourceObservation>,
    pages: usize,
    partial: bool,
    final_turn_pending: bool,
    /// Cursor after the last page. A binding that names it resumes the read.
    cursor: Option<HostUsageCursor>,
    /// Quota of the last page read. A source that names no account keeps its scope unknown.
    quota: DispatchQuota,
}

/// Outcome of one collection, rendered as text.
struct CollectionReport {
    run_id: String,
    source: String,
    role: String,
    read: SourceRead,
    new_observations: usize,
    run_observations: usize,
    /// Snapshot identifier and whether this call published it or replayed it.
    snapshot: Option<(String, &'static str)>,
    rate_digest: Option<String>,
    /// `supplied`, `reused`, or `none recorded`.
    rate_source: &'static str,
    priced: Vec<(String, Estimate)>,
    /// Quota reference or the reason none was stored, rendered for the report.
    quota: String,
}

/// Parses `usage collect` and its options. `args[0]` is the command name.
pub(crate) fn parse_collect_request(args: &[String]) -> Result<CollectRequest, String> {
    if args.get(1).map(String::as_str) != Some("collect") {
        return Err(COLLECT_USAGE.to_owned());
    }
    let mut run_id: Option<String> = None;
    let mut binding_path: Option<String> = None;
    let mut rates_path: Option<String> = None;
    let mut rest = args[2..].iter();
    while let Some(flag) = rest.next() {
        let slot = match flag.as_str() {
            "--run" => &mut run_id,
            "--binding" => &mut binding_path,
            "--rates" => &mut rates_path,
            other => return Err(format!("unknown option `{other}`. {COLLECT_USAGE}")),
        };
        let value = rest
            .next()
            .ok_or_else(|| format!("`{flag}` needs a value. {COLLECT_USAGE}"))?;
        if slot.replace(value.clone()).is_some() {
            return Err(format!("`{flag}` is given more than once"));
        }
    }
    Ok(CollectRequest {
        run_id: run_id.ok_or_else(|| format!("`--run` is required. {COLLECT_USAGE}"))?,
        binding_path: binding_path.map(PathBuf::from),
        rates_path: rates_path.map(PathBuf::from),
    })
}

/// Handler for `gal usage collect`. Reads the bound native source and appends what is new.
fn cmd_collect(args: &[String]) -> ExitCode {
    let request = match parse_collect_request(args) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("gal usage: {message}");
            return ExitCode::Usage;
        }
    };
    let binding = match read_binding(&request) {
        Ok(binding) => binding,
        Err(message) => {
            eprintln!("gal usage: {message}");
            return ExitCode::Usage;
        }
    };
    let rates = match request.rates_path.as_deref().map(read_rates).transpose() {
        Ok(rates) => rates,
        Err(message) => {
            eprintln!("gal usage: {message}");
            return ExitCode::Usage;
        }
    };
    let common_dir = match git_common_dir() {
        Ok(path) => path,
        Err(message) => {
            eprintln!("gal usage: {message}");
            return ExitCode::Error;
        }
    };
    let store = match UsageStore::open(&common_dir) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("gal usage: {error}");
            return ExitCode::Error;
        }
    };
    execute_collect(
        &store,
        &request.run_id,
        &binding,
        rates.as_ref(),
        &read_native_page,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

fn read_binding(request: &CollectRequest) -> Result<Binding, String> {
    let Some(binding_path) = &request.binding_path else {
        return recorded_binding(&request.run_id);
    };
    let bytes = fs::read(binding_path)
        .map_err(|error| format!("cannot read binding `{}`: {error}", binding_path.display()))?;
    parse_binding(&request.run_id, &bytes)
}

/// Builds the orchestrator binding that the run's preflight receipt recorded. It goes through
/// the same validation as a binding file, so the recorded session is checked again here.
fn recorded_binding(run_id: &str) -> Result<Binding, String> {
    let record = pipeline_preflight::read_orchestrator_record(run_id)?.ok_or_else(|| {
        format!("run `{run_id}` has no preflight receipt, so no orchestrator binding is recorded. Pass `--binding`")
    })?;
    match record {
        OrchestratorRecord::Bound(recorded) => {
            let value = json!({
                "run": {
                    "version": RUN_MANIFEST_VERSION,
                    "run_id": run_id,
                    "canonical_worktree": recorded.canonical_worktree,
                    "plan_scope": recorded.plan_scope,
                    "host_session": recorded.host_session,
                    "host_window": null,
                },
                "host_session": recorded.host_session,
                "billing_mode": ORCHESTRATOR_BILLING_MODE,
                "role": { "kind": "orchestrator" },
                "window": { "started_at": recorded.window_started_at },
            });
            parse_binding(run_id, value.to_string().as_bytes())
        }
        OrchestratorRecord::Unavailable { reason } => Err(format!(
            "run `{run_id}` has no bound orchestrator, because {reason}. Pass `--binding` to collect from a binding file"
        )),
    }
}

fn read_rates(path: &Path) -> Result<RateSet, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read rates `{}`: {error}", path.display()))?;
    RateSet::parse(&bytes)
        .map_err(|error| format!("rate set `{}` is rejected: {error}", path.display()))
}

/// Validates a binding against the requested run. Its identity must name that run and
/// the same host session as the binding itself, so a rebind is refused before any read.
fn parse_binding(run_id: &str, bytes: &[u8]) -> Result<Binding, String> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| format!("binding is not JSON: {error}"))?;
    let fields = value.as_object().ok_or("binding must be a JSON object")?;
    if let Some(key) = fields
        .keys()
        .find(|key| !BINDING_FIELDS.contains(&key.as_str()))
    {
        return Err(format!(
            "binding field `{key}` is not allowed. A binding names the run, source, and window only"
        ));
    }
    let run: RunIdentity = serde_json::from_value(required(fields, "run")?.clone())
        .map_err(|error| format!("binding run identity is invalid: {error}"))?;
    if run.run_id != run_id {
        return Err(format!(
            "binding names run `{}`, not `{run_id}`",
            run.run_id
        ));
    }
    let identity_digest = run
        .digest()
        .map_err(|error| format!("binding run identity is invalid: {error}"))?;
    let host_session = string_field(fields, "host_session")?;
    if run.host_session.as_deref() != Some(host_session.as_str()) {
        return Err(format!(
            "rebind conflict: binding host session `{host_session}` differs from its run identity"
        ));
    }
    let reference = HostSessionRef::parse(&host_session)
        .map_err(|error| format!("binding host session `{host_session}` is invalid: {error}"))?;
    let cursor = match fields.get("cursor") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            serde_json::from_value::<HostUsageCursor>(value.clone())
                .map_err(|error| format!("binding cursor is invalid: {error}"))?,
        ),
    };
    Ok(Binding {
        identity_digest,
        reference,
        role: parse_role(required(fields, "role")?)?,
        window: parse_window(required(fields, "window")?)?,
        cursor,
        billing_mode: string_field(fields, "billing_mode")?,
        workdir: run.canonical_worktree.clone(),
        identity: run,
        task_id: None,
        phase: None,
    })
}

fn parse_role(value: &Value) -> Result<HostUsageRole, String> {
    let fields = value.as_object().ok_or("binding role must be an object")?;
    match fields.get("kind").and_then(Value::as_str) {
        Some("orchestrator") if fields.len() == 1 => Ok(HostUsageRole::Orchestrator),
        Some("child") if fields.len() == 2 => Ok(HostUsageRole::Child {
            attempt_id: string_field(fields, "attempt_id")?,
        }),
        _ => Err(
            "binding role must be {\"kind\":\"orchestrator\"} or {\"kind\":\"child\",\"attempt_id\":...}"
                .to_owned(),
        ),
    }
}

fn parse_window(value: &Value) -> Result<UsageWindow, String> {
    let fields = value
        .as_object()
        .ok_or("binding window must be an object")?;
    if let Some(key) = fields
        .keys()
        .find(|key| !matches!(key.as_str(), "started_at" | "ended_at"))
    {
        return Err(format!("binding window field `{key}` is not allowed"));
    }
    let started = string_field(fields, "started_at")?;
    let window = match fields.get("ended_at") {
        None | Some(Value::Null) => UsageWindow::starting_at(&started),
        Some(_) => UsageWindow::closed(&started, &string_field(fields, "ended_at")?),
    };
    window.map_err(|error| format!("binding window is invalid: {error}"))
}

fn required<'a>(
    fields: &'a serde_json::Map<String, Value>,
    name: &str,
) -> Result<&'a Value, String> {
    fields
        .get(name)
        .ok_or_else(|| format!("binding field `{name}` is required"))
}

fn string_field(fields: &serde_json::Map<String, Value>, name: &str) -> Result<String, String> {
    required(fields, name)?
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("binding field `{name}` must be a non-empty string"))
}

/// Reads one native page of the bound session, through the same identity checks as dispatch.
fn read_native_page(
    binding: &Binding,
    cursor: Option<&HostUsageCursor>,
) -> Result<HostUsagePage, HostUsageError> {
    let home = dirs::home_dir()
        .ok_or_else(|| HostUsageError::Io("the home directory is unknown".to_owned()))?;
    read_native_page_in(&home, binding, cursor)
}

/// Reads one native page of the bound session under `home`.
fn read_native_page_in(
    home: &Path,
    binding: &Binding,
    cursor: Option<&HostUsageCursor>,
) -> Result<HostUsagePage, HostUsageError> {
    collect_host_usage(
        &binding.reference,
        &binding.role,
        Path::new(&binding.workdir),
        &HostRoots::from_env(home),
        &binding.window,
        cursor,
    )
}

/// Builds the collection and writes it. Text goes to `out`, and every diagnostic goes to `err`.
fn execute_collect(
    store: &UsageStore,
    run_id: &str,
    binding: &Binding,
    rates: Option<&RateSet>,
    read_page: &dyn Fn(&Binding, Option<&HostUsageCursor>) -> Result<HostUsagePage, HostUsageError>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitCode {
    let report = match collect_run(store, run_id, binding, rates, read_page) {
        Ok(report) => report,
        Err(ReportError::Usage(message)) => {
            let _ = writeln!(err, "gal usage: {message}");
            return ExitCode::Usage;
        }
        Err(ReportError::Integrity(message)) => {
            let _ = writeln!(err, "gal usage: {message}");
            return ExitCode::Error;
        }
    };
    if out
        .write_all(render_collection(&report).as_bytes())
        .is_err()
    {
        return ExitCode::Error;
    }
    ExitCode::Success
}

/// Reads the source, merges the result with what the run already holds, and publishes
/// a snapshot only when the merged set is not empty. Nothing is published when a read fails.
fn collect_run(
    store: &UsageStore,
    run_id: &str,
    binding: &Binding,
    rates: Option<&RateSet>,
    read_page: &dyn Fn(&Binding, Option<&HostUsageCursor>) -> Result<HostUsagePage, HostUsageError>,
) -> Result<CollectionReport, ReportError> {
    let read = read_source(binding, read_page)?;
    let evidence = usage_capture::read_run_evidence(store, run_id).map_err(capture_error)?;
    // Only observations that a host binding wrote are compared. An automatic attempt capture
    // keeps its own identifier and never blocks an orchestrator collection on the same run.
    if let Some((id, _)) = evidence.observations.iter().find(|(id, record)| {
        is_binding_observation_id(id) && !same_run_identity(&record.observation.identity, binding)
    }) {
        return Err(ReportError::Usage(format!(
            "run `{run_id}` already holds observation `{id}` from another binding, and a run cannot be rebound"
        )));
    }

    let mut merged = evidence.observations;
    let mut new_observations = 0;
    for observation in &read.observations {
        let record = ObservationRecord {
            observation_id: observation.observation_id.clone(),
            observation: fill_host_identity(binding, &observation.observation),
        };
        match merged.entry(observation.observation_id.clone()) {
            Entry::Vacant(slot) => {
                slot.insert(record);
                new_observations += 1;
            }
            Entry::Occupied(known) => {
                // A row published before the fill holds the unfilled read, so either form matches.
                let unfilled = ObservationRecord {
                    observation_id: observation.observation_id.clone(),
                    observation: observation.observation.clone(),
                };
                if known.get() != &record && known.get() != &unfilled {
                    return Err(ReportError::Integrity(format!(
                        "observation `{}` changed after it was published",
                        observation.observation_id
                    )));
                }
            }
        }
    }
    let run_observations = merged.len();

    let rate_digests = evidence.rate_set_digests;
    let (rate_digest, rate_source) = match (rates, rate_digests.len()) {
        (Some(rates), _) => (Some(rates.digest().to_owned()), "supplied"),
        (None, 0) => (None, "none recorded"),
        (None, 1) => (rate_digests.iter().next().cloned(), "reused"),
        (None, _) => {
            return Err(ReportError::Usage(format!(
                "run `{run_id}` holds several rate sets, so pass --rates to choose one"
            )))
        }
    };

    let snapshot = if merged.is_empty() {
        None
    } else {
        let now = usage_capture::unix_ms_now();
        // A replay keeps the cutoff it was first published with, so identical input adds no bytes.
        let cutoff_unix_ms = match (new_observations, evidence.cutoff_unix_ms) {
            (0, Some(known)) => known,
            (_, Some(known)) => known.max(now),
            (_, None) => now,
        };
        let (snapshot_id, publication) = usage_capture::publish_snapshot(
            store,
            run_id,
            cutoff_unix_ms,
            merged.into_values().collect(),
            evidence.intervals,
            rate_digest.clone(),
        )
        .map_err(capture_error)?;
        let state = match publication {
            Publication::Published => "published",
            Publication::Replayed => "replayed",
        };
        Some((snapshot_id, state))
    };
    let priced = match rates {
        Some(rates) => price_observations(binding, rates, &read.observations)?,
        None => Vec::new(),
    };
    let quota = collect_quota_text(store, binding, &read);

    Ok(CollectionReport {
        run_id: run_id.to_owned(),
        source: format!(
            "{}:{}",
            binding.reference.host.as_str(),
            binding.reference.reference
        ),
        role: role_name(&binding.role),
        read,
        new_observations,
        run_observations,
        snapshot,
        rate_digest,
        rate_source,
        priced,
        quota,
    })
}

/// Billing mode of a child binding that a host close builds. A close carries no rate set, so
/// the binding prices nothing.
const CHILD_BILLING_MODE: &str = "unspecified";

/// One verified host child session and the run it belongs to.
pub(crate) struct ChildSession<'a> {
    pub(crate) run_id: &'a str,
    pub(crate) plan_scope: &'a str,
    pub(crate) canonical_worktree: &'a str,
    pub(crate) attempt_id: &'a str,
    /// Task and phase of the host manifest attempt. They fill the child observations that name none.
    pub(crate) task_id: &'a str,
    pub(crate) phase: &'a str,
    /// Start of the host manifest attempt. The window opens here.
    pub(crate) started_at: &'a str,
    pub(crate) session_reference: &'a str,
    /// Home that the close verified the session under. The collector reads the same home.
    pub(crate) home: &'a Path,
}

/// Collects the bounded usage of one verified host child session into its run, using the same
/// collector as `gal usage collect`. The binding names the host manifest attempt id. Returns the
/// number of observations that this collection added.
pub(crate) fn collect_child_attempt(
    common_dir: &Path,
    session: &ChildSession<'_>,
) -> Result<usize, String> {
    let reference = HostSessionRef::parse(session.session_reference).map_err(|error| {
        format!(
            "host session `{}` is invalid: {error}",
            session.session_reference
        )
    })?;
    let identity = RunIdentity {
        version: RUN_MANIFEST_VERSION,
        run_id: session.run_id.to_owned(),
        canonical_worktree: session.canonical_worktree.to_owned(),
        plan_scope: session.plan_scope.to_owned(),
        host_session: Some(session.session_reference.to_owned()),
        host_window: None,
    };
    let binding = Binding {
        identity_digest: identity.digest().map_err(|error| error.to_string())?,
        reference,
        role: HostUsageRole::Child {
            attempt_id: session.attempt_id.to_owned(),
        },
        window: UsageWindow::starting_at(session.started_at)
            .map_err(|error| format!("window is invalid: {error}"))?,
        cursor: None,
        billing_mode: CHILD_BILLING_MODE.to_owned(),
        workdir: session.canonical_worktree.to_owned(),
        identity,
        task_id: Some(session.task_id.to_owned()),
        phase: Some(session.phase.to_owned()),
    };
    let store = UsageStore::open(common_dir).map_err(|error| error.to_string())?;
    let read_page = |binding: &Binding, cursor: Option<&HostUsageCursor>| {
        read_native_page_in(session.home, binding, cursor)
    };
    let report = collect_run(&store, session.run_id, &binding, None, &read_page).map_err(
        |error| match error {
            ReportError::Usage(message) | ReportError::Integrity(message) => message,
        },
    )?;
    Ok(report.new_observations)
}

/// Stores the quota of the source read against the child attempt it belongs to. The
/// orchestrator has no attempt, so its quota stays unavailable with the reason.
fn collect_quota_text(store: &UsageStore, binding: &Binding, read: &SourceRead) -> String {
    let attempt_id = match &binding.role {
        HostUsageRole::Child { attempt_id } => Some(attempt_id.as_str()),
        HostUsageRole::Orchestrator => None,
    };
    let snapshot = usage_capture::capture_quota(store, &binding.identity, attempt_id, &read.quota);
    let observation = match &read.quota {
        DispatchQuota::Collected { observations } => observations.last(),
        DispatchQuota::NotAttempted | DispatchQuota::UnknownScope { .. } => None,
    };
    usage_capture::quota_text(&snapshot, observation)
}

/// Reads pages from the binding's cursor until the source is complete or the page budget ends.
fn read_source(
    binding: &Binding,
    read_page: &dyn Fn(&Binding, Option<&HostUsageCursor>) -> Result<HostUsagePage, HostUsageError>,
) -> Result<SourceRead, ReportError> {
    let mut read = SourceRead {
        observations: Vec::new(),
        pages: 0,
        partial: false,
        final_turn_pending: false,
        cursor: binding.cursor.clone(),
        quota: DispatchQuota::UnknownScope {
            reason: "the source returned no page, so no account was read".to_owned(),
        },
    };
    loop {
        if read.pages == COLLECT_MAX_PAGES {
            read.partial = true;
            return Ok(read);
        }
        let page = read_page(binding, read.cursor.as_ref())
            .map_err(|error| ReportError::Integrity(format!("source unavailable: {error}")))?;
        read.pages += 1;
        for observation in page.observations {
            read.observations.push(SourceObservation {
                observation_id: observation_id(&binding.identity_digest, &page.cursor),
                digest: observation_digest(&observation)?,
                observation,
            });
        }
        read.final_turn_pending = page.final_turn_pending;
        let complete = page.status == PageStatus::Complete;
        read.quota = page.quota;
        read.cursor = Some(page.cursor);
        if complete {
            return Ok(read);
        }
    }
}

/// Prices this collection's observations. The store keeps no observation bodies or rate
/// content, so these estimates exist only for the current call.
fn price_observations(
    binding: &Binding,
    rates: &RateSet,
    observations: &[SourceObservation],
) -> Result<Vec<(String, Estimate)>, ReportError> {
    let factual = UsageSnapshot {
        schema_version: RECORD_SCHEMA_VERSION,
        snapshot_id: String::new(),
        run_id: String::new(),
        reconciliation_version: RECONCILIATION_VERSION,
        rate_set_digest: None,
        observations: observations
            .iter()
            .map(|observation| ObservationDigest {
                observation_id: observation.observation_id.clone(),
                digest: observation.digest.clone(),
            })
            .collect(),
    };
    let inputs: Vec<PricingInput> = observations
        .iter()
        .map(|observation| PricingInput {
            observation_id: observation.observation_id.clone(),
            provider: binding.reference.host.as_str().to_owned(),
            billing_mode: binding.billing_mode.clone(),
            observation: observation.observation.clone(),
        })
        .collect();
    let priced = price_snapshot(&factual, &inputs, rates).map_err(|error| {
        ReportError::Integrity(format!("rates cannot price the observations: {error}"))
    })?;
    Ok(priced
        .observations
        .into_iter()
        .map(|observation| (observation.observation_id, observation.rate_estimate))
        .collect())
}

/// Identifier of an observation. The binding prefix ties it to one binding, and the
/// digest ties it to the cursor where it ends, so a replayed read names the same observation.
fn observation_id(identity_digest: &str, cursor: &HostUsageCursor) -> String {
    let end = digest_bytes(format!("{}\n{}", cursor.epoch, cursor.offset).as_bytes());
    format!("{}{}", observation_prefix(identity_digest), &end[..32])
}

fn observation_prefix(identity_digest: &str) -> String {
    format!("{}-", &identity_digest[..16])
}

/// Whether an identifier came from a host binding. It is a 16-character identity prefix, a dash,
/// and a 32-character cursor digest, all hexadecimal. An automatic attempt capture names
/// `<attempt>-<index>`, and its attempt identifier never has this shape.
fn is_binding_observation_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    bytes.len() == 49
        && bytes[16] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 16 || byte.is_ascii_hexdigit())
}

/// Whether a stored binding observation belongs to the run identity of `binding`. The run id
/// and plan scope make that identity, and the host session does not, so an orchestrator row and
/// a child row of one run both match. A row without a run or plan stays unjudged and matches.
fn same_run_identity(identity: &ObservationIdentity, binding: &Binding) -> bool {
    let run_matches = match &identity.run_id {
        Some(run_id) => run_id == &binding.identity.run_id,
        None => true,
    };
    let plan_matches = match &identity.plan_id {
        Some(plan_id) => plan_id == &binding.identity.plan_scope,
        None => true,
    };
    run_matches && plan_matches
}

/// Fills the null run, plan, and child task and phase of one host observation. A value the
/// provider reported stays as it is. Only a child binding fills the task and phase.
fn fill_host_identity(binding: &Binding, observation: &UsageObservation) -> UsageObservation {
    let mut filled = observation.clone();
    let identity = &mut filled.identity;
    fill_unset(&mut identity.run_id, &binding.identity.run_id);
    fill_unset(&mut identity.plan_id, &binding.identity.plan_scope);
    if let HostUsageRole::Child { .. } = binding.role {
        if let Some(task_id) = &binding.task_id {
            fill_unset(&mut identity.task_id, task_id);
        }
        if let Some(phase) = &binding.phase {
            fill_unset(&mut identity.phase, phase);
        }
    }
    filled
}

fn fill_unset(field: &mut Option<String>, value: &str) {
    if field.is_none() {
        *field = Some(value.to_owned());
    }
}

fn observation_digest(observation: &UsageObservation) -> Result<String, ReportError> {
    serde_json::to_vec(observation)
        .map(|bytes| digest_bytes(&bytes))
        .map_err(|error| ReportError::Integrity(format!("observation cannot be encoded: {error}")))
}

fn role_name(role: &HostUsageRole) -> String {
    match role {
        HostUsageRole::Orchestrator => "orchestrator".to_owned(),
        HostUsageRole::Child { attempt_id } => format!("child attempt {attempt_id}"),
    }
}

fn render_collection(report: &CollectionReport) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "gal usage collect");
    let _ = writeln!(text, "run: {}", report.run_id);
    let _ = writeln!(text, "source: {} ({})", report.source, report.role);
    let _ = writeln!(text, "pages read: {}", report.read.pages);
    let _ = writeln!(
        text,
        "observations read: {}, new to run: {}, in run: {}",
        report.read.observations.len(),
        report.new_observations,
        report.run_observations
    );
    match &report.snapshot {
        Some((id, state)) => {
            let _ = writeln!(text, "snapshot: {id} ({state})");
        }
        None => {
            let _ = writeln!(
                text,
                "snapshot: none. The bound window holds no usage row, which is not zero usage"
            );
        }
    }
    match &report.rate_digest {
        Some(digest) => {
            let _ = writeln!(text, "rates: {digest} ({})", report.rate_source);
        }
        None => {
            let _ = writeln!(text, "rates: none recorded, so costs are unpriced");
        }
    }
    if report.rate_source == "supplied" {
        let _ = writeln!(text, "rate estimates (this collection only):");
        for (id, estimate) in &report.priced {
            let _ = writeln!(text, "  {id} rate estimate: {}", estimate_text(estimate));
        }
    } else if report.rate_digest.is_some() {
        let _ = writeln!(text, "costs are not repriced. Pass --rates to price them");
    }
    let _ = writeln!(text, "run totals: unavailable, because {UNAVAILABLE}");
    let _ = writeln!(text, "quota: {}", report.quota);
    if report.read.partial {
        let _ = writeln!(
            text,
            "status: partial. The page budget of {COLLECT_MAX_PAGES} ended the read, so resume from the cursor below"
        );
    } else {
        let _ = writeln!(text, "status: complete");
    }
    if report.read.final_turn_pending {
        let _ = writeln!(
            text,
            "final turn: not recorded yet. A later collection from the cursor below adds it"
        );
    }
    if let Some(cursor) = &report.read.cursor {
        if let Ok(json) = serde_json::to_string(cursor) {
            let _ = writeln!(text, "resume cursor (binding `cursor` field): {json}");
        }
    }
    text
}

fn estimate_text(estimate: &Estimate) -> String {
    match estimate {
        Estimate::Known { amount } => Quantity::of_money(amount).render(),
        Estimate::Unknown { reason } => format!("unknown ({reason})"),
    }
}

/// Judgment fields an input may declare. Factual fields, identities the handler assigns,
/// and anything unknown are rejected before any write.
const ASSESS_FIELDS: [&str; 7] = [
    "useful_work_rule",
    "author",
    "session_id",
    "model",
    "supersedes",
    "assignments",
    "residuals",
];

const ASSESS_USAGE: &str =
    "usage: gal usage assess --run <run-id> --snapshot <snapshot-id> --input <path>";

/// Parsed `gal usage assess` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssessRequest {
    pub(crate) run_id: String,
    pub(crate) snapshot_id: String,
    pub(crate) input_path: PathBuf,
}

/// Judgment fields that the input declares. The handler adds the identity, the snapshot
/// digest, and the time.
#[derive(Debug)]
struct AssessmentInput {
    useful_work_rule: String,
    author: String,
    session_id: String,
    model: String,
    supersedes: Option<String>,
    assignments: Vec<Assignment>,
    residuals: Vec<ResidualClaim>,
}

/// Outcome of one judgment that was checked and published.
struct AssessmentWritten {
    run_id: String,
    assessment_id: String,
    snapshot_id: String,
    snapshot_digest: String,
    supersedes: Option<String>,
    publication: Publication,
}

/// Parses `usage assess` and its options. `args[0]` is the command name.
pub(crate) fn parse_assess_request(args: &[String]) -> Result<AssessRequest, String> {
    if args.get(1).map(String::as_str) != Some("assess") {
        return Err(ASSESS_USAGE.to_owned());
    }
    let mut run_id: Option<String> = None;
    let mut snapshot_id: Option<String> = None;
    let mut input_path: Option<String> = None;
    let mut rest = args[2..].iter();
    while let Some(flag) = rest.next() {
        let slot = match flag.as_str() {
            "--run" => &mut run_id,
            "--snapshot" => &mut snapshot_id,
            "--input" => &mut input_path,
            other => return Err(format!("unknown option `{other}`. {ASSESS_USAGE}")),
        };
        let value = rest
            .next()
            .ok_or_else(|| format!("`{flag}` needs a value. {ASSESS_USAGE}"))?;
        if slot.replace(value.clone()).is_some() {
            return Err(format!("`{flag}` is given more than once"));
        }
    }
    Ok(AssessRequest {
        run_id: run_id.ok_or_else(|| format!("`--run` is required. {ASSESS_USAGE}"))?,
        snapshot_id: snapshot_id
            .ok_or_else(|| format!("`--snapshot` is required. {ASSESS_USAGE}"))?,
        input_path: PathBuf::from(
            input_path.ok_or_else(|| format!("`--input` is required. {ASSESS_USAGE}"))?,
        ),
    })
}

/// Handler for `gal usage assess`. Appends one judgment on an existing snapshot.
fn cmd_assess(args: &[String]) -> ExitCode {
    let request = match parse_assess_request(args) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("gal usage: {message}");
            return ExitCode::Usage;
        }
    };
    let input = match fs::read(&request.input_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!(
                "gal usage: cannot read input `{}`: {error}",
                request.input_path.display()
            );
            return ExitCode::Usage;
        }
    };
    let common_dir = match git_common_dir() {
        Ok(path) => path,
        Err(message) => {
            eprintln!("gal usage: {message}");
            return ExitCode::Error;
        }
    };
    let store = match UsageStore::open(&common_dir) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("gal usage: {error}");
            return ExitCode::Error;
        }
    };
    let rebuild = store_rebuild(&store, &request.run_id);
    execute_assess(
        &store,
        &request,
        &input,
        &rebuild,
        usage_capture::unix_ms_now(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

/// Checks and publishes one judgment. Text goes to `out`, and every diagnostic goes to `err`.
fn execute_assess(
    store: &UsageStore,
    request: &AssessRequest,
    input: &[u8],
    rebuild: &dyn Fn(&UsageSnapshot) -> Option<Reconciliation>,
    recorded_at_unix_ms: u64,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitCode {
    let written = match write_assessment(store, request, input, rebuild, recorded_at_unix_ms) {
        Ok(written) => written,
        Err(ReportError::Usage(message)) => {
            let _ = writeln!(err, "gal usage: {message}");
            return ExitCode::Usage;
        }
        Err(ReportError::Integrity(message)) => {
            let _ = writeln!(err, "gal usage: {message}");
            return ExitCode::Error;
        }
    };
    if out
        .write_all(render_assessment_written(&written).as_bytes())
        .is_err()
    {
        return ExitCode::Error;
    }
    ExitCode::Success
}

/// Checks the input against the rebuilt snapshot, then publishes the judgment.
/// Nothing is written unless every check passes.
fn write_assessment(
    store: &UsageStore,
    request: &AssessRequest,
    input_bytes: &[u8],
    rebuild: &dyn Fn(&UsageSnapshot) -> Option<Reconciliation>,
    recorded_at_unix_ms: u64,
) -> Result<AssessmentWritten, ReportError> {
    let input = parse_assessment_input(input_bytes).map_err(ReportError::Usage)?;
    let run = request.run_id.as_str();
    let snapshot: UsageSnapshot = store.read(run, &request.snapshot_id).map_err(store_error)?;
    let digest = record_digest(&snapshot).map_err(store_error)?;
    let reconciled = rebuild(&snapshot).ok_or_else(|| {
        ReportError::Integrity(format!(
            "snapshot `{}` cannot be rebuilt, so the assessment cannot be checked and nothing was written. {UNAVAILABLE}",
            snapshot.snapshot_id
        ))
    })?;
    let rebuilt_snapshot =
        reconciled.snapshot(&snapshot.snapshot_id, snapshot.rate_set_digest.clone());
    if rebuilt_snapshot != snapshot {
        return Err(ReportError::Integrity(format!(
            "the rebuilt reconciliation does not reproduce snapshot `{}`",
            snapshot.snapshot_id
        )));
    }

    let judgments = read_judgments(store, run)?;
    let heads = revision_heads(&judgments, &snapshot.snapshot_id, &digest);
    match &input.supersedes {
        Some(prior) if !heads.contains(prior) => {
            return Err(ReportError::Usage(format!(
                "`{prior}` is not a current assessment of snapshot `{}`",
                snapshot.snapshot_id
            )));
        }
        None if !heads.is_empty() => {
            return Err(ReportError::Usage(format!(
                "snapshot `{}` already has assessment {}. A revision must name the one it supersedes",
                snapshot.snapshot_id,
                listed(&heads)
            )));
        }
        _ => {}
    }

    let record = AssessmentRecord {
        schema_version: RECORD_SCHEMA_VERSION,
        assessment_id: judgment_identity(&digest, recorded_at_unix_ms, input_bytes),
        run_id: run.to_owned(),
        snapshot_id: snapshot.snapshot_id.clone(),
        snapshot_digest: digest.clone(),
        useful_work_rule: input.useful_work_rule,
        author: input.author,
        session_id: input.session_id,
        model: input.model,
        recorded_at_unix_ms,
        supersedes: input.supersedes.clone(),
        assignments: input.assignments,
        residuals: input.residuals,
    };
    assess(&record, &reconciled, &snapshot).map_err(assessment_error)?;
    let publication = publish_judgment(store, &record).map_err(assessment_error)?;
    Ok(AssessmentWritten {
        run_id: record.run_id,
        assessment_id: record.assessment_id,
        snapshot_id: record.snapshot_id,
        snapshot_digest: digest,
        supersedes: input.supersedes,
        publication,
    })
}

/// Validates the judgment input. Only the declared judgment fields are accepted, so no
/// factual value can enter a write.
fn parse_assessment_input(bytes: &[u8]) -> Result<AssessmentInput, String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("assessment input is not JSON: {error}"))?;
    let fields = value
        .as_object()
        .ok_or("assessment input must be a JSON object")?;
    if let Some(key) = fields
        .keys()
        .find(|key| !ASSESS_FIELDS.contains(&key.as_str()))
    {
        return Err(format!(
            "assessment input field `{key}` is not allowed. A judgment names its author, rule, session, model, supersedes link, assignments, and residuals only"
        ));
    }
    let supersedes = match fields.get("supersedes") {
        None | Some(Value::Null) => None,
        Some(_) => Some(judgment_text(fields, "supersedes")?),
    };
    let assignments = fields
        .get("assignments")
        .cloned()
        .ok_or("assessment field `assignments` is required")?;
    let residuals = fields
        .get("residuals")
        .cloned()
        .ok_or("assessment field `residuals` is required")?;
    Ok(AssessmentInput {
        useful_work_rule: judgment_text(fields, "useful_work_rule")?,
        author: judgment_text(fields, "author")?,
        session_id: judgment_text(fields, "session_id")?,
        model: judgment_text(fields, "model")?,
        supersedes,
        assignments: serde_json::from_value::<Vec<Assignment>>(assignments)
            .map_err(|error| format!("assessment assignments are invalid: {error}"))?,
        residuals: serde_json::from_value::<Vec<ResidualClaim>>(residuals)
            .map_err(|error| format!("assessment residuals are invalid: {error}"))?,
    })
}

fn judgment_text(fields: &serde_json::Map<String, Value>, name: &str) -> Result<String, String> {
    fields
        .get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("assessment field `{name}` is missing or empty"))
}

/// Records the judgments of one run and reads each one from the store.
fn read_judgments(store: &UsageStore, run_id: &str) -> Result<Vec<AssessmentRecord>, ReportError> {
    let mut judgments = Vec::new();
    for id in record_ids::<AssessmentRecord>(store, run_id)? {
        judgments.push(
            store
                .read::<AssessmentRecord>(run_id, &id)
                .map_err(store_error)?,
        );
    }
    Ok(judgments)
}

/// Current judgments of one snapshot that no other current judgment supersedes.
/// A judgment on an older snapshot digest is stale and does not count.
fn revision_heads(judgments: &[AssessmentRecord], snapshot_id: &str, digest: &str) -> Vec<String> {
    let current: Vec<&AssessmentRecord> = judgments
        .iter()
        .filter(|judgment| {
            judgment.snapshot_id == snapshot_id && judgment.snapshot_digest == digest
        })
        .collect();
    let superseded: BTreeSet<&str> = current
        .iter()
        .filter_map(|judgment| judgment.supersedes.as_deref())
        .collect();
    current
        .iter()
        .filter(|judgment| !superseded.contains(judgment.assessment_id.as_str()))
        .map(|judgment| judgment.assessment_id.clone())
        .collect()
}

/// Identifier of one judgment. It digests the snapshot it judges, the time, and the input,
/// so a reassessment gets a new identifier, and an identical resubmission replays.
fn judgment_identity(snapshot_digest: &str, recorded_at_unix_ms: u64, input: &[u8]) -> String {
    let basis = format!(
        "{snapshot_digest}\n{recorded_at_unix_ms}\n{}",
        digest_bytes(input)
    );
    format!("assess-{}", &digest_bytes(basis.as_bytes())[..32])
}

/// Maps judgment failures to exit classes. A rejected judgment is a usage error. A snapshot
/// or store that disagrees with itself means the store cannot be trusted.
fn assessment_error(error: AssessmentError) -> ReportError {
    let integrity = matches!(
        error,
        AssessmentError::SnapshotMismatch(_)
            | AssessmentError::SnapshotDigestMismatch
            | AssessmentError::Conservation(_)
            | AssessmentError::Encode(_)
            | AssessmentError::Store(_)
    );
    let message = error.to_string();
    if integrity {
        ReportError::Integrity(message)
    } else {
        ReportError::Usage(message)
    }
}

fn render_assessment_written(written: &AssessmentWritten) -> String {
    let state = match written.publication {
        Publication::Published => "published",
        Publication::Replayed => "replayed",
    };
    let mut text = String::new();
    let _ = writeln!(text, "gal usage assess");
    let _ = writeln!(text, "run: {}", written.run_id);
    let _ = writeln!(text, "assessment: {} ({state})", written.assessment_id);
    let _ = writeln!(text, "snapshot: {}", written.snapshot_id);
    let _ = writeln!(text, "  record digest: {}", written.snapshot_digest);
    let _ = writeln!(
        text,
        "supersedes: {}",
        written.supersedes.as_deref().unwrap_or("none")
    );
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use pipeline::usage::assessment::publish_judgment;
    use pipeline::usage::digest_bytes;
    use pipeline::usage::reconcile::{reconcile, ReconcileInput};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use std::time::SystemTime;
    use tempfile::TempDir;

    use dispatch::dispatch::DispatchQuota;
    use dispatch::usage::{
        Availability, CounterRelation, CounterSemantics, DescendantScope, Metric, TokenCount,
        TokenMetric, TokenUsage,
    };
    use pipeline::usage::binding::RUN_MANIFEST_VERSION;

    const FIXTURES: &str = include_str!("../../../pipeline/tests/fixtures/usage_accounting.json");

    /// Finds one named entry of the shared accounting fixture.
    fn fixture_entry(list: &str, name: &str) -> Value {
        let file: Value = serde_json::from_str(FIXTURES).expect("fixture file is valid JSON");
        file[list]
            .as_array()
            .expect("fixture list exists")
            .iter()
            .find(|entry| entry["name"] == name)
            .cloned()
            .unwrap_or_else(|| panic!("fixture entry `{name}` is missing"))
    }

    fn reconcile_fixture(name: &str) -> Reconciliation {
        let case = fixture_entry("cases", name);
        reconcile(ReconcileInput {
            run_id: case["run_id"]
                .as_str()
                .expect("case names its run")
                .to_owned(),
            cutoff_unix_ms: case["cutoff_unix_ms"].as_u64().expect("case has a cutoff"),
            observations: serde_json::from_value(case["observations"].clone())
                .expect("observations decode"),
            intervals: serde_json::from_value(case["intervals"].clone()).expect("intervals decode"),
        })
        .expect("fixture case reconciles")
    }

    fn judgment(name: &str) -> AssessmentRecord {
        let case = fixture_entry("assessment_cases", name);
        serde_json::from_value(case["judgment"].clone()).expect("judgment decodes")
    }

    /// Opens a store under a fresh directory and publishes one snapshot.
    /// A judgment, when given, is bound to that snapshot's exact bytes.
    fn publish(
        reconciled: &Reconciliation,
        snapshot_id: &str,
        judgment: Option<AssessmentRecord>,
    ) -> (TempDir, UsageStore) {
        let directory = TempDir::new().expect("temp dir");
        let store = UsageStore::open(directory.path()).expect("store opens");
        let snapshot = reconciled.snapshot(snapshot_id, None);
        store.publish(&snapshot).expect("snapshot publishes");
        if let Some(mut judgment) = judgment {
            judgment.snapshot_digest = record_digest(&snapshot).expect("digest");
            publish_judgment(&store, &judgment).expect("judgment publishes");
        }
        (directory, store)
    }

    fn request(run: &str, snapshot: Option<&str>, assessment: Option<&str>, json: bool) -> Request {
        Request {
            run_id: run.to_owned(),
            snapshot_id: snapshot.map(str::to_owned),
            assessment_id: assessment.map(str::to_owned),
            json,
        }
    }

    fn execute_captured(
        store: &UsageStore,
        request: &Request,
        rebuild: &dyn Fn(&UsageSnapshot) -> Option<Reconciliation>,
    ) -> (ExitCode, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(store, request, rebuild, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).expect("stdout is UTF-8"),
            String::from_utf8(err).expect("stderr is UTF-8"),
        )
    }

    fn parse_json(text: &str) -> Value {
        serde_json::from_str(text).expect("stdout is exactly one JSON document")
    }

    /// Digest and modification time of every file under `root`, for read-only checks.
    fn store_state(root: &Path) -> BTreeMap<PathBuf, (String, SystemTime)> {
        let mut state = BTreeMap::new();
        collect_state(root, &mut state);
        state
    }

    fn collect_state(dir: &Path, state: &mut BTreeMap<PathBuf, (String, SystemTime)>) {
        for entry in fs::read_dir(dir).expect("store directory reads") {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                collect_state(&path, state);
            } else {
                let bytes = fs::read(&path).expect("record reads");
                let modified = fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .expect("record has a modification time");
                state.insert(path, (digest_bytes(&bytes), modified));
            }
        }
    }

    #[test]
    fn usage_report_parses_options_and_rejects_bad_arguments() {
        let args =
            |list: &[&str]| -> Vec<String> { list.iter().map(|item| item.to_string()).collect() };

        assert_eq!(
            parse_request(&args(&["usage", "report", "--run", "run-a", "--json"])),
            Ok(request("run-a", None, None, true))
        );
        assert_eq!(
            parse_request(&args(&[
                "usage",
                "report",
                "--run",
                "run-a",
                "--snapshot",
                "snap-a",
                "--assessment",
                "assess-a",
            ])),
            Ok(request("run-a", Some("snap-a"), Some("assess-a"), false))
        );
        assert!(parse_request(&args(&["usage", "report"]))
            .unwrap_err()
            .contains("--run"));
        assert!(parse_request(&args(&["usage", "report", "--run", "run-a", "--bogus"])).is_err());
        assert!(parse_request(&args(&["usage", "report", "--run"])).is_err());
        assert!(parse_request(&args(&["usage", "report", "--run", "a", "--run", "b"])).is_err());
        assert!(parse_request(&args(&["usage", "collect", "--run", "run-a"])).is_err());
    }

    #[test]
    fn usage_report_invalid_selection_exits_usage() {
        let reconciled = reconcile_fixture("inclusive_parent_counts_child_once");
        let run_id = reconciled.run_id.clone();
        let (_directory, store) = publish(&reconciled, "snap-inclusive", None);
        let rebuild = |_: &UsageSnapshot| None;

        let (code, out, err) = execute_captured(
            &store,
            &request("run/../escape", None, None, false),
            &rebuild,
        );
        assert_eq!(
            code,
            ExitCode::Usage,
            "an invalid run identifier is a usage error"
        );
        assert!(out.is_empty());
        assert!(err.contains("not a valid identifier"));

        let (code, _, err) = execute_captured(
            &store,
            &request(&run_id, Some("snap-missing"), None, false),
            &rebuild,
        );
        assert_eq!(code, ExitCode::Usage);
        assert!(err.contains("snap-missing"));
    }

    #[test]
    fn usage_report_default_selects_latest_snapshot_and_matching_assessment() {
        let reconciled = reconcile_fixture("inclusive_parent_counts_child_once");
        let run_id = reconciled.run_id.clone();
        let (_directory, store) = publish(
            &reconciled,
            "snap-inclusive",
            Some(judgment("inclusive_residual_is_proven")),
        );
        let rebuild = |snapshot: &UsageSnapshot| {
            (snapshot.snapshot_id == "snap-inclusive")
                .then(|| reconcile_fixture("inclusive_parent_counts_child_once"))
        };

        let (code, out, err) =
            execute_captured(&store, &request(&run_id, None, None, true), &rebuild);
        assert_eq!(code, ExitCode::Success);
        assert!(err.is_empty(), "a successful report writes no diagnostics");
        let document = parse_json(&out);
        assert_eq!(document["snapshot"]["snapshot_id"], "snap-inclusive");
        assert_eq!(
            document["assessment"]["assessment_id"],
            "assess-inclusive-1"
        );

        // A newer snapshot becomes the default. The judgment names the older
        // snapshot, so it must not be applied to the newer totals.
        store
            .publish(&reconciled.snapshot("snap-inclusive-newer", Some("rates-v2".to_owned())))
            .expect("newer snapshot publishes");
        let (code, out, _) =
            execute_captured(&store, &request(&run_id, None, None, true), &rebuild);
        assert_eq!(code, ExitCode::Success);
        let document = parse_json(&out);
        assert_eq!(document["snapshot"]["snapshot_id"], "snap-inclusive-newer");
        assert!(document["assessment"].is_null());
        assert_eq!(document["status"], "unavailable");
        assert_eq!(
            document["history"]["snapshot_ids"].as_array().map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn usage_report_rejects_stale_assessment_pairing() {
        let reconciled = reconcile_fixture("inclusive_parent_counts_child_once");
        let run_id = reconciled.run_id.clone();
        let (_directory, store) = publish(&reconciled, "snap-inclusive", None);
        store
            .publish(&reconciled.snapshot("snap-other", None))
            .expect("other snapshot publishes");

        // The store accepts the record without the digest check, so this
        // judgment names the snapshot but not its exact bytes.
        let mut stale = judgment("inclusive_residual_is_proven");
        stale.snapshot_digest = "0".repeat(64);
        store.publish(&stale).expect("stale judgment publishes");
        let rebuild = |_: &UsageSnapshot| None;

        let (code, out, err) = execute_captured(
            &store,
            &request(
                &run_id,
                Some("snap-inclusive"),
                Some("assess-inclusive-1"),
                false,
            ),
            &rebuild,
        );
        assert_eq!(code, ExitCode::Usage);
        assert!(out.is_empty());
        assert!(
            err.contains("stale"),
            "diagnostic names the stale pairing: {err}"
        );

        let (code, _, err) = execute_captured(
            &store,
            &request(
                &run_id,
                Some("snap-other"),
                Some("assess-inclusive-1"),
                false,
            ),
            &rebuild,
        );
        assert_eq!(code, ExitCode::Usage);
        assert!(err.contains("judges snapshot"));

        // Default selection never applies the stale judgment and lists it instead.
        let (code, out, _) = execute_captured(
            &store,
            &request(&run_id, Some("snap-inclusive"), None, true),
            &rebuild,
        );
        assert_eq!(code, ExitCode::Success);
        let document = parse_json(&out);
        assert!(document["assessment"].is_null());
        assert_eq!(
            document["history"]["stale_assessment_ids"][0],
            "assess-inclusive-1"
        );
    }

    #[test]
    fn usage_report_text_and_json_keep_the_same_complete_rows() {
        let reconciled = reconcile_fixture("inclusive_parent_counts_child_once");
        let run_id = reconciled.run_id.clone();
        let (_directory, store) = publish(
            &reconciled,
            "snap-inclusive",
            Some(judgment("inclusive_residual_is_proven")),
        );
        let rebuild = |snapshot: &UsageSnapshot| {
            (snapshot.snapshot_id == "snap-inclusive")
                .then(|| reconcile_fixture("inclusive_parent_counts_child_once"))
        };

        let (code, text, err) =
            execute_captured(&store, &request(&run_id, None, None, false), &rebuild);
        assert_eq!(code, ExitCode::Success);
        assert!(err.is_empty());
        let (_, json_out, _) =
            execute_captured(&store, &request(&run_id, None, None, true), &rebuild);

        let document = parse_json(&json_out);
        let rows = document["factual"]["rows"]
            .as_array()
            .expect("JSON carries rows");
        assert_eq!(rows.len(), reconciled.contributions.len());
        for row in &reconciled.contributions {
            assert!(
                text.contains(&row.observation_id),
                "text omits observation {}",
                row.observation_id
            );
        }
        for row in rows {
            let observation = row["observation_id"]
                .as_str()
                .expect("row names its observation");
            assert!(
                text.contains(observation),
                "JSON row {observation} is missing from text"
            );
        }
        assert!(document["assessment"]["bases"].is_array());
        assert!(text.contains("assessment:\n  id: assess-inclusive-1"));
    }

    #[test]
    fn usage_report_all_costs_unknown_keeps_null_reasons_and_unavailable_total() {
        let reconciled = reconcile_fixture("all_costs_unknown");
        let run_id = reconciled.run_id.clone();
        let (_directory, store) = publish(&reconciled, "snap-all-costs-unknown", None);
        let rebuild = |snapshot: &UsageSnapshot| {
            (snapshot.snapshot_id == "snap-all-costs-unknown")
                .then(|| reconcile_fixture("all_costs_unknown"))
        };

        let (code, json_out, _) =
            execute_captured(&store, &request(&run_id, None, None, true), &rebuild);
        assert_eq!(code, ExitCode::Success);
        let document = parse_json(&json_out);

        let rows = document["factual"]["rows"].as_array().expect("rows");
        let cost_rows: Vec<&Value> = rows.iter().filter(|row| row["metric"] == "cost").collect();
        assert!(!cost_rows.is_empty(), "the fixture has cost rows");
        for row in cost_rows {
            assert_eq!(row["treatment"], "unknown");
            assert!(row["reason"].is_string(), "unknown row keeps its reason");
            assert!(row["contributed"].is_null());
        }

        let cost_totals: Vec<&Value> = document["factual"]["totals"]
            .as_array()
            .expect("totals")
            .iter()
            .filter(|total| {
                total["basis"]
                    .as_str()
                    .is_some_and(|basis| basis.starts_with("cost"))
            })
            .collect();
        assert!(!cost_totals.is_empty());
        for total in cost_totals {
            assert!(
                total["total"].is_null(),
                "an unknown cost total is null, never zero"
            );
            assert!(total["reason"].is_string());
        }

        let (_, text, _) = execute_captured(&store, &request(&run_id, None, None, false), &rebuild);
        assert!(text.contains("total=unavailable"));
    }

    #[test]
    fn usage_report_historical_read_leaves_store_bytes_unchanged() {
        let reconciled = reconcile_fixture("inclusive_parent_counts_child_once");
        let run_id = reconciled.run_id.clone();
        let (directory, store) = publish(
            &reconciled,
            "snap-inclusive",
            Some(judgment("inclusive_residual_is_proven")),
        );
        // Newer rate input is already published, so a reprice would show up here.
        store
            .publish(&reconciled.snapshot("snap-inclusive-newer", Some("rates-v2".to_owned())))
            .expect("newer snapshot publishes");
        let before = store_state(directory.path());
        let rebuild = |snapshot: &UsageSnapshot| {
            (snapshot.snapshot_id == "snap-inclusive")
                .then(|| reconcile_fixture("inclusive_parent_counts_child_once"))
        };

        for historical in [
            request(
                &run_id,
                Some("snap-inclusive"),
                Some("assess-inclusive-1"),
                true,
            ),
            request(&run_id, Some("snap-inclusive"), None, false),
            request(&run_id, None, None, true),
        ] {
            let (code, _, _) = execute_captured(&store, &historical, &rebuild);
            assert_eq!(code, ExitCode::Success);
        }

        assert_eq!(
            store_state(directory.path()),
            before,
            "historical reads change no digest or timestamp"
        );
    }

    #[test]
    fn usage_report_corrupt_snapshot_is_an_integrity_error() {
        let reconciled = reconcile_fixture("inclusive_parent_counts_child_once");
        let run_id = reconciled.run_id.clone();
        let (directory, store) = publish(&reconciled, "snap-inclusive", None);
        let record = store_state(directory.path())
            .into_keys()
            .find(|path| {
                path.file_name().and_then(|name| name.to_str())
                    == Some("snap-inclusive.record.json")
            })
            .expect("snapshot record file exists");
        fs::write(&record, b"{}").expect("record is overwritten");

        let (code, out, err) = execute_captured(
            &store,
            &request(&run_id, Some("snap-inclusive"), None, true),
            &|_: &UsageSnapshot| None,
        );
        assert_eq!(
            code,
            ExitCode::Error,
            "storage and integrity failures exit with the error code"
        );
        assert!(out.is_empty());
        assert!(!err.is_empty());
    }

    const BINDING: &str = r#"{
        "run": {
            "version": 1,
            "run_id": "run-a",
            "canonical_worktree": "C:/work/repo",
            "plan_scope": "plan-a",
            "host_session": "codex:sess-a",
            "host_window": "window-a"
        },
        "host_session": "codex:sess-a",
        "billing_mode": "standard",
        "role": {"kind": "orchestrator"},
        "window": {"started_at": "2026-10-20T00:00:00.000Z"}
    }"#;

    const RATES: &str = r#"{"schema_version": 1, "rate_set_id": "rates-a", "rates": [
        {"provider": "codex", "model": "gpt-x", "billing_mode": "standard", "category": "input",
         "unit": "per_million_tokens", "price": "2.5", "money_unit": {"currency": "USD"}}
    ]}"#;

    const RATES_OTHER: &str = r#"{"schema_version": 1, "rate_set_id": "rates-b", "rates": [
        {"provider": "codex", "model": "gpt-x", "billing_mode": "standard", "category": "input",
         "unit": "per_million_tokens", "price": "3", "money_unit": {"currency": "USD"}}
    ]}"#;

    fn collect_binding(host_window: &str) -> Binding {
        let identity = RunIdentity {
            version: RUN_MANIFEST_VERSION,
            run_id: "run-a".to_owned(),
            canonical_worktree: "C:/work/repo".to_owned(),
            plan_scope: "plan-a".to_owned(),
            host_session: Some("codex:sess-a".to_owned()),
            host_window: Some(host_window.to_owned()),
        };
        Binding {
            identity_digest: identity.digest().expect("identity digests"),
            reference: HostSessionRef::parse("codex:sess-a").expect("reference parses"),
            role: HostUsageRole::Orchestrator,
            window: UsageWindow::starting_at("2026-10-20T00:00:00.000Z").expect("window opens"),
            cursor: None,
            billing_mode: "standard".to_owned(),
            workdir: identity.canonical_worktree.clone(),
            identity,
            task_id: None,
            phase: None,
        }
    }

    /// One delta observation of `input` tokens. Every other category is a recorded zero,
    /// because pricing treats an unrecorded category as unknown.
    /// One output-token observation with a reported count, as an executor log records it.
    fn output_observation(output: u64) -> UsageObservation {
        serde_json::from_value(json!({
            "tokens": {
                "output": {
                    "count": {
                        "value": { "state": "reported", "value": output },
                        "semantics": "delta",
                        "descendants": "own",
                        "window": { "start_unix_ms": 1000, "end_unix_ms": 2000 }
                    },
                    "relation": { "kind": "exclusive" }
                }
            }
        }))
        .expect("output observation is valid")
    }

    /// A failed attempt of 100 tokens and its resumed attempt of 60 tokens, captured through
    /// the shared helper, must show as one run total of 160 with no injected rebuild.
    #[test]
    fn usage_report_production_rebuild_lists_total_after_failed_and_resumed_attempts() {
        let (_directory, store) = empty_store();
        let run = "run-capture";
        let now = 1_792_000_000_000;
        let failed = usage_capture::AttemptUsage {
            attempt_id: "attempt-failed".to_owned(),
            observations: vec![output_observation(100)],
            elapsed_ms: Some(5_000),
        };
        let resumed = usage_capture::AttemptUsage {
            attempt_id: "attempt-resumed".to_owned(),
            observations: vec![output_observation(60)],
            elapsed_ms: Some(3_000),
        };
        usage_capture::capture_attempt(&store, run, &failed, now).expect("failed attempt captures");
        usage_capture::capture_attempt(&store, run, &resumed, now + 60_000)
            .expect("resumed attempt captures");

        let rebuild = store_rebuild(&store, run);
        let (code, text, err) =
            execute_captured(&store, &request(run, None, None, false), &rebuild);
        assert_eq!(code, ExitCode::Success, "{err}");
        assert!(err.is_empty());
        assert!(text.contains("160"), "run total is 160: {text}");
        assert!(!text.contains(UNAVAILABLE), "rebuild succeeded: {text}");
    }

    fn metered(input: u64) -> UsageObservation {
        let counter = |count: u64| TokenMetric {
            count: Metric {
                value: Availability::Reported {
                    value: TokenCount::new(count).expect("count fits"),
                },
                semantics: CounterSemantics::Delta,
                descendants: DescendantScope::Own,
                window: None,
            },
            relation: CounterRelation::Exclusive,
        };
        let mut observation = UsageObservation::new();
        observation.tokens = TokenUsage {
            input: counter(input),
            output: counter(0),
            cached_input: counter(0),
            cache_write: counter(0),
            reasoning: counter(0),
            other: counter(0),
        };
        observation.models.observed = Some("gpt-x".to_owned());
        observation
    }

    fn page(
        offset: u64,
        observations: Vec<UsageObservation>,
        status: PageStatus,
        final_turn_pending: bool,
    ) -> HostUsagePage {
        HostUsagePage {
            observations,
            cursor: HostUsageCursor {
                epoch: "codex:sess-a".to_owned(),
                offset,
                through_unix_ms: 1_792_000_000_000,
                totals: None,
                activity_after_usage: final_turn_pending,
            },
            status,
            final_turn_pending,
            quota: DispatchQuota::UnknownScope {
                reason: "fixture".to_owned(),
            },
        }
    }

    fn empty_store() -> (TempDir, UsageStore) {
        let directory = TempDir::new().expect("temp dir");
        let store = UsageStore::open(directory.path()).expect("store opens");
        (directory, store)
    }

    fn collect_captured(
        store: &UsageStore,
        binding: &Binding,
        rates: Option<&RateSet>,
        read_page: &dyn Fn(
            &Binding,
            Option<&HostUsageCursor>,
        ) -> Result<HostUsagePage, HostUsageError>,
    ) -> (ExitCode, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_collect(
            store, "run-a", binding, rates, read_page, &mut out, &mut err,
        );
        (
            code,
            String::from_utf8(out).expect("stdout is UTF-8"),
            String::from_utf8(err).expect("stderr is UTF-8"),
        )
    }

    /// Every file that was present before must keep its digest and timestamp.
    fn assert_kept(
        before: &BTreeMap<PathBuf, (String, SystemTime)>,
        after: &BTreeMap<PathBuf, (String, SystemTime)>,
    ) {
        for (path, state) in before {
            assert_eq!(after.get(path), Some(state), "{} changed", path.display());
        }
    }

    #[test]
    fn usage_collect_parses_options_and_takes_an_optional_binding() {
        let args =
            |list: &[&str]| -> Vec<String> { list.iter().map(|item| item.to_string()).collect() };

        assert_eq!(
            parse_collect_request(&args(&[
                "usage",
                "collect",
                "--run",
                "run-a",
                "--binding",
                "binding.json",
            ])),
            Ok(CollectRequest {
                run_id: "run-a".to_owned(),
                binding_path: Some(PathBuf::from("binding.json")),
                rates_path: None,
            })
        );
        // Without a binding file, collection reads the binding that the run's preflight recorded.
        assert_eq!(
            parse_collect_request(&args(&["usage", "collect", "--run", "run-a"])),
            Ok(CollectRequest {
                run_id: "run-a".to_owned(),
                binding_path: None,
                rates_path: None,
            })
        );
        assert!(
            parse_collect_request(&args(&["usage", "collect", "--binding", "b.json"]))
                .unwrap_err()
                .contains("--run")
        );
        assert!(parse_collect_request(&args(&[
            "usage",
            "collect",
            "--run",
            "a",
            "--run",
            "b",
            "--binding",
            "x",
        ]))
        .is_err());
        assert!(parse_collect_request(&args(&["usage", "report", "--run", "run-a"])).is_err());
    }

    #[test]
    fn usage_collect_binding_rejects_assessments_and_widened_fields() {
        assert!(parse_binding("run-a", BINDING.as_bytes()).is_ok());

        let assessment = br#"{"assessment_id": "assess-a", "run_id": "run-a", "snapshot_id": "snap-a", "assignments": []}"#;
        assert!(parse_binding("run-a", assessment)
            .unwrap_err()
            .contains("not allowed"));

        let counters = BINDING.replacen('{', "{\"input_tokens\": 5,", 1);
        assert!(parse_binding("run-a", counters.as_bytes())
            .unwrap_err()
            .contains("input_tokens"));

        assert!(parse_binding("run-b", BINDING.as_bytes())
            .unwrap_err()
            .contains("not `run-b`"));

        // The first `host_session` key belongs to the run identity. Changing only it
        // leaves the binding's own session as sess-a, so the rebind is refused.
        let rebound = BINDING.replacen(
            "\"host_session\": \"codex:sess-a\",",
            "\"host_session\": \"codex:sess-b\",",
            1,
        );
        assert!(parse_binding("run-a", rebound.as_bytes())
            .unwrap_err()
            .contains("rebind conflict"));
    }

    #[test]
    fn usage_collect_same_cursor_adds_no_duplicate_cost() {
        let (directory, store) = empty_store();
        let binding = collect_binding("window-a");
        let reader =
            |_: &Binding, _: Option<&HostUsageCursor>| -> Result<HostUsagePage, HostUsageError> {
                Ok(page(
                    120,
                    vec![metered(1_000_000)],
                    PageStatus::Complete,
                    false,
                ))
            };

        let (code, out, err) = collect_captured(&store, &binding, None, &reader);
        assert_eq!(code, ExitCode::Success);
        assert!(err.is_empty());
        assert!(
            out.contains("(published)"),
            "first collection publishes: {out}"
        );
        let first = store_state(directory.path());

        let (code, out, _) = collect_captured(&store, &binding, None, &reader);
        assert_eq!(code, ExitCode::Success);
        assert!(out.contains("(replayed)"), "same cursor replays: {out}");
        assert!(out.contains("observations read: 1, new to run: 0, in run: 1"));
        assert_eq!(
            store_state(directory.path()),
            first,
            "replay writes no bytes"
        );
        assert_eq!(
            record_ids::<UsageSnapshot>(&store, "run-a")
                .expect("ids list")
                .len(),
            1
        );
    }

    #[test]
    fn usage_collect_delayed_event_appends_preserved_snapshot() {
        let (directory, store) = empty_store();
        let binding = collect_binding("window-a");
        // The first read ends with a final turn that the source has not recorded yet.
        let early =
            |_: &Binding, _: Option<&HostUsageCursor>| -> Result<HostUsagePage, HostUsageError> {
                Ok(page(
                    120,
                    vec![metered(1_000_000)],
                    PageStatus::Complete,
                    true,
                ))
            };
        let (code, _, _) = collect_captured(&store, &binding, None, &early);
        assert_eq!(code, ExitCode::Success);
        let before = store_state(directory.path());

        // The later read keeps the first observation and adds the delayed one.
        let later = |_: &Binding,
                     cursor: Option<&HostUsageCursor>|
         -> Result<HostUsagePage, HostUsageError> {
            Ok(match cursor {
                None => page(120, vec![metered(1_000_000)], PageStatus::BudgetStop, false),
                Some(_) => page(240, vec![metered(2_000_000)], PageStatus::Complete, false),
            })
        };
        let (code, out, _) = collect_captured(&store, &binding, None, &later);
        assert_eq!(code, ExitCode::Success);
        assert!(
            out.contains("new to run: 1, in run: 2"),
            "delayed event is new: {out}"
        );
        assert!(out.contains("(published)"));

        let after = store_state(directory.path());
        assert_kept(&before, &after);
        assert_eq!(
            record_ids::<UsageSnapshot>(&store, "run-a")
                .expect("ids list")
                .len(),
            2,
            "the earlier snapshot stays beside the new one"
        );
    }

    #[test]
    fn usage_collect_new_rate_appends_and_ambiguous_reuse_needs_rates() {
        let (directory, store) = empty_store();
        let binding = collect_binding("window-a");
        let reader =
            |_: &Binding, _: Option<&HostUsageCursor>| -> Result<HostUsagePage, HostUsageError> {
                Ok(page(
                    120,
                    vec![metered(1_000_000)],
                    PageStatus::Complete,
                    false,
                ))
            };
        let (code, _, _) = collect_captured(&store, &binding, None, &reader);
        assert_eq!(code, ExitCode::Success);
        let unpriced = store_state(directory.path());

        let rates = RateSet::parse(RATES.as_bytes()).expect("rates parse");
        let (code, out, _) = collect_captured(&store, &binding, Some(&rates), &reader);
        assert_eq!(code, ExitCode::Success);
        assert!(
            out.contains("(published)"),
            "a new rate set is a new snapshot: {out}"
        );
        assert!(out.contains(rates.digest()));
        assert_kept(&unpriced, &store_state(directory.path()));

        // Reusing the rate set without an override picks the one digest the run holds.
        let (code, out, _) = collect_captured(&store, &binding, None, &reader);
        assert_eq!(code, ExitCode::Success);
        assert!(
            out.contains(&format!("rates: {} (reused)", rates.digest())),
            "{out}"
        );

        // Two different rate sets leave the reuse ambiguous, so the caller must choose.
        let other = RateSet::parse(RATES_OTHER.as_bytes()).expect("rates parse");
        let (code, _, _) = collect_captured(&store, &binding, Some(&other), &reader);
        assert_eq!(code, ExitCode::Success);
        let (code, out, err) = collect_captured(&store, &binding, None, &reader);
        assert_eq!(code, ExitCode::Usage);
        assert!(out.is_empty());
        assert!(err.contains("--rates"), "{err}");
    }

    #[test]
    fn usage_collect_prices_new_observations_with_supplied_rates() {
        let (_directory, store) = empty_store();
        let binding = collect_binding("window-a");
        let reader =
            |_: &Binding, _: Option<&HostUsageCursor>| -> Result<HostUsagePage, HostUsageError> {
                Ok(page(
                    120,
                    vec![metered(1_000_000)],
                    PageStatus::Complete,
                    false,
                ))
            };
        let rates = RateSet::parse(RATES.as_bytes()).expect("rates parse");

        let (code, out, _) = collect_captured(&store, &binding, Some(&rates), &reader);
        assert_eq!(code, ExitCode::Success);
        // One million input tokens at 2.5 USD per million tokens.
        assert!(out.contains("rate estimate: 2.5"), "{out}");
        assert!(out.contains("USD"), "{out}");
        assert!(out.contains("run totals: unavailable"));
    }

    #[test]
    fn usage_collect_missing_source_publishes_nothing() {
        let (directory, store) = empty_store();
        let binding = collect_binding("window-a");
        let before = store_state(directory.path());
        let reader =
            |_: &Binding, _: Option<&HostUsageCursor>| -> Result<HostUsagePage, HostUsageError> {
                Err(HostUsageError::MalformedRecord)
            };

        let (code, out, err) = collect_captured(&store, &binding, None, &reader);
        assert_eq!(code, ExitCode::Error);
        assert!(out.is_empty());
        assert!(err.contains("source unavailable"), "{err}");
        assert_eq!(store_state(directory.path()), before);
    }

    #[test]
    fn usage_collect_rebind_conflict_is_usage_and_keeps_store() {
        let (directory, store) = empty_store();
        let first = collect_binding("window-a");
        let reader =
            |_: &Binding, _: Option<&HostUsageCursor>| -> Result<HostUsagePage, HostUsageError> {
                Ok(page(
                    120,
                    vec![metered(1_000_000)],
                    PageStatus::Complete,
                    false,
                ))
            };
        let (code, _, _) = collect_captured(&store, &first, None, &reader);
        assert_eq!(code, ExitCode::Success);
        let before = store_state(directory.path());

        // A different plan scope is a different run identity, so it cannot add to this run.
        let mut other = collect_binding("window-a");
        other.identity.plan_scope = "plan-b".to_owned();
        other.identity_digest = other.identity.digest().expect("identity digests");
        let (code, out, err) = collect_captured(&store, &other, None, &reader);
        assert_eq!(code, ExitCode::Usage);
        assert!(out.is_empty());
        assert!(err.contains("rebound"), "{err}");
        assert_eq!(store_state(directory.path()), before);
    }

    /// Child binding of the same run as `collect_binding`, for the host session of one attempt.
    fn child_binding(attempt_id: &str, task_id: &str, phase: &str) -> Binding {
        let identity = RunIdentity {
            version: RUN_MANIFEST_VERSION,
            run_id: "run-a".to_owned(),
            canonical_worktree: "C:/work/repo".to_owned(),
            plan_scope: "plan-a".to_owned(),
            host_session: Some("codex:sess-child".to_owned()),
            host_window: None,
        };
        Binding {
            identity_digest: identity.digest().expect("identity digests"),
            reference: HostSessionRef::parse("codex:sess-child").expect("reference parses"),
            role: HostUsageRole::Child {
                attempt_id: attempt_id.to_owned(),
            },
            window: UsageWindow::starting_at("2026-10-20T00:00:00.000Z").expect("window opens"),
            cursor: None,
            billing_mode: "unspecified".to_owned(),
            workdir: identity.canonical_worktree.clone(),
            identity,
            task_id: Some(task_id.to_owned()),
            phase: Some(phase.to_owned()),
        }
    }

    /// An orchestrator collection and a child close of one run store their rows side by side.
    /// The child rows take the attempt's task and phase, and a row another plan cannot join.
    #[test]
    fn usage_collect_orchestrator_and_child_share_one_run() {
        let (_directory, store) = empty_store();
        let reader =
            |_: &Binding, _: Option<&HostUsageCursor>| -> Result<HostUsagePage, HostUsageError> {
                Ok(page(
                    120,
                    vec![metered(1_000_000)],
                    PageStatus::Complete,
                    false,
                ))
            };
        let orchestrator = collect_binding("window-a");
        let (code, _, _) = collect_captured(&store, &orchestrator, None, &reader);
        assert_eq!(code, ExitCode::Success);

        let child = child_binding("attempt-child", "task-a", "implement");
        let (code, _, err) = collect_captured(&store, &child, None, &reader);
        assert_eq!(code, ExitCode::Success, "{err}");

        let evidence = usage_capture::read_run_evidence(&store, "run-a").expect("evidence reads");
        assert_eq!(evidence.observations.len(), 2, "one row per binding");
        let row_of = |binding: &Binding| {
            let prefix = observation_prefix(&binding.identity_digest);
            evidence
                .observations
                .iter()
                .find(|(id, _)| id.starts_with(&prefix))
                .map(|(_, record)| record.observation.identity.clone())
                .expect("the binding wrote a row")
        };
        let orchestrator_row = row_of(&orchestrator);
        assert_eq!(
            orchestrator_row.task_id, None,
            "orchestrator rows keep no task"
        );
        assert_eq!(orchestrator_row.plan_id.as_deref(), Some("plan-a"));
        let child_row = row_of(&child);
        assert_eq!(child_row.task_id.as_deref(), Some("task-a"));
        assert_eq!(child_row.phase.as_deref(), Some("implement"));
        assert_eq!(child_row.plan_id.as_deref(), Some("plan-a"));
    }

    /// A child binding whose plan scope differs is refused, and the store keeps its rows.
    #[test]
    fn usage_collect_child_from_another_plan_scope_is_refused() {
        let (directory, store) = empty_store();
        let reader =
            |_: &Binding, _: Option<&HostUsageCursor>| -> Result<HostUsagePage, HostUsageError> {
                Ok(page(
                    120,
                    vec![metered(1_000_000)],
                    PageStatus::Complete,
                    false,
                ))
            };
        let orchestrator = collect_binding("window-a");
        let (code, _, _) = collect_captured(&store, &orchestrator, None, &reader);
        assert_eq!(code, ExitCode::Success);
        let before = store_state(directory.path());

        let mut child = child_binding("attempt-child", "task-a", "implement");
        child.identity.plan_scope = "plan-b".to_owned();
        child.identity_digest = child.identity.digest().expect("identity digests");
        let (code, out, err) = collect_captured(&store, &child, None, &reader);
        assert_eq!(code, ExitCode::Usage);
        assert!(out.is_empty());
        assert!(err.contains("rebound"), "{err}");
        assert_eq!(store_state(directory.path()), before);
    }

    #[test]
    fn usage_collect_budget_stop_reports_partial_cursor() {
        let (_directory, store) = empty_store();
        let binding = collect_binding("window-a");
        let reader = |_: &Binding,
                      cursor: Option<&HostUsageCursor>|
         -> Result<HostUsagePage, HostUsageError> {
            let next = cursor.map_or(120, |cursor| cursor.offset + 10);
            Ok(page(next, vec![metered(1)], PageStatus::BudgetStop, false))
        };

        let (code, out, _) = collect_captured(&store, &binding, None, &reader);
        assert_eq!(
            code,
            ExitCode::Success,
            "a partial read still exits with success"
        );
        assert!(
            out.contains(&format!("pages read: {COLLECT_MAX_PAGES}")),
            "{out}"
        );
        assert!(out.contains("status: partial"), "{out}");
        assert!(
            out.contains("resume cursor (binding `cursor` field):"),
            "{out}"
        );
        assert!(
            out.contains(&format!("observations read: {COLLECT_MAX_PAGES}")),
            "{out}"
        );
    }

    #[test]
    fn usage_collect_without_usage_rows_publishes_nothing() {
        let (directory, store) = empty_store();
        let binding = collect_binding("window-a");
        let reader =
            |_: &Binding, _: Option<&HostUsageCursor>| -> Result<HostUsagePage, HostUsageError> {
                Ok(page(120, Vec::new(), PageStatus::Complete, false))
            };

        let (code, out, _) = collect_captured(&store, &binding, None, &reader);
        assert_eq!(code, ExitCode::Success);
        assert!(out.contains("snapshot: none"), "{out}");
        assert!(store_state(directory.path()).is_empty());
    }

    #[test]
    fn usage_collect_rejects_invalid_rate_file() {
        let directory = TempDir::new().expect("temp dir");
        let path = directory.path().join("rates.json");
        fs::write(&path, b"{}").expect("rate file writes");

        let error = read_rates(&path).expect_err("an empty object is not a rate set");
        assert!(error.contains("is rejected"), "{error}");
    }

    #[test]
    fn usage_collect_records_quota_reference_for_child_attempt_and_reason_for_orchestrator() {
        use dispatch::quota::{
            missing, AccountIdentityCheck, AccountRef, Acquisition, BillingMode, BucketId,
            Freshness, ProviderId, QueryMetadata, QuotaIdentity, QuotaObservation, QuotaRuntime,
            QuotaWindow, RuntimeVersion, WindowKind, COLLECTOR, QUOTA_SCHEMA_VERSION,
        };
        use dispatch::usage::ObservationSource;
        use pipeline::usage::quota::QuotaRevision;

        let (directory, store) = empty_store();
        let child_binding = Binding {
            identity_digest: "identity-digest-child-12345678".to_owned(),
            reference: HostSessionRef::parse("codex:sess-a").expect("reference parses"),
            role: HostUsageRole::Child {
                attempt_id: "attempt-child".to_owned(),
            },
            window: UsageWindow::starting_at("2026-10-20T00:00:00.000Z").expect("window opens"),
            cursor: None,
            billing_mode: "standard".to_owned(),
            workdir: "C:/work/repo".to_owned(),
            identity: RunIdentity {
                version: RUN_MANIFEST_VERSION,
                run_id: "run-a".to_owned(),
                canonical_worktree: "C:/work/repo".to_owned(),
                plan_scope: "plan-a".to_owned(),
                host_session: Some("codex:sess-a".to_owned()),
                host_window: Some("window-child".to_owned()),
            },
            task_id: None,
            phase: None,
        };

        let quota_obs = QuotaObservation {
            schema_version: QUOTA_SCHEMA_VERSION,
            identity: QuotaIdentity {
                runtime: QuotaRuntime::Codex,
                runtime_version: Some(RuntimeVersion::new("0.163.0").expect("version is valid")),
                provider: ProviderId::new("openai").expect("provider is valid"),
                billing: BillingMode::Subscription,
                account: Some(AccountRef::new("acct-shared").expect("account is valid")),
                identity_check: AccountIdentityCheck::Verified,
                host: None,
                auth_profile: None,
                model_scope: None,
            },
            bucket: BucketId::new("codex-weekly").expect("bucket is valid"),
            window: Availability::Reported {
                value: QuotaWindow {
                    kind: WindowKind::Weekly,
                    duration_minutes: Some(10_080),
                    resets_at_unix_ms: Some(1_800_600_000_000),
                },
            },
            utilization: missing("no utilization"),
            used: missing("no amount"),
            remaining: missing("no amount"),
            limit: missing("no amount"),
            observed_at_unix_ms: Some(1_800_000_100_000),
            collected_at_unix_ms: 1_800_000_200_000,
            source: ObservationSource {
                collector: Some(COLLECTOR.to_owned()),
                source_version: Some("0.163.0".to_owned()),
                cursor: None,
            },
            query: Some(QueryMetadata {
                method: "account/rateLimits/read".to_owned(),
                duration_ms: 42,
            }),
            acquisition: Acquisition::Queried,
            freshness: Freshness::Current,
        };

        let quota = DispatchQuota::Collected {
            observations: vec![quota_obs],
        };

        let reader =
            |_: &Binding, _: Option<&HostUsageCursor>| -> Result<HostUsagePage, HostUsageError> {
                let mut p = page(120, vec![metered(1_000_000)], PageStatus::Complete, false);
                p.quota = quota.clone();
                Ok(p)
            };

        let (code, out, err) = collect_captured(&store, &child_binding, None, &reader);
        assert_eq!(code, ExitCode::Success, "{err}");
        assert!(
            out.contains("quota: reported, revision quota-r1, observation"),
            "{out}"
        );
        assert!(out.contains("runtime version 0.163.0"), "{out}");
        assert!(
            out.contains("control query account/rateLimits/read in 42 ms"),
            "{out}"
        );

        let revisions = record_ids::<QuotaRevision>(&store, "run-a").expect("revisions list");
        assert_eq!(revisions.len(), 1);

        // Replay of child collection with same cursor leaves revision intact and produces identical reference
        let (code, out_replay, err_replay) =
            collect_captured(&store, &child_binding, None, &reader);
        assert_eq!(code, ExitCode::Success, "{err_replay}");
        assert!(
            out_replay.contains("quota: reported, revision quota-r1, observation"),
            "{out_replay}"
        );
        assert_eq!(
            record_ids::<QuotaRevision>(&store, "run-a")
                .expect("revisions list")
                .len(),
            1
        );

        // Orchestrator binding reports that orchestrator has no attempt
        let (_orch_dir, orch_store) = empty_store();
        let orchestrator_binding = collect_binding("window-orch");
        let (code_orch, out_orch, err_orch) =
            collect_captured(&orch_store, &orchestrator_binding, None, &reader);
        assert_eq!(code_orch, ExitCode::Success, "{err_orch}");
        assert!(
            out_orch.contains(
                "quota: unavailable. orchestrator usage has no attempt to attach quota to"
            ),
            "{out_orch}"
        );
        assert!(
            record_ids::<QuotaRevision>(&orch_store, "run-a")
                .expect("revisions list")
                .is_empty(),
            "orchestrator writes no quota revision"
        );
        drop(directory);
    }

    fn assess_request(run: &str, snapshot: &str) -> AssessRequest {
        AssessRequest {
            run_id: run.to_owned(),
            snapshot_id: snapshot.to_owned(),
            input_path: PathBuf::from("judgment.json"),
        }
    }

    /// Declared judgment fields of one fixture judgment, as an editable input document.
    fn assessment_input_value(name: &str) -> Value {
        let judgment = fixture_entry("assessment_cases", name)["judgment"].clone();
        let fields: serde_json::Map<String, Value> = ASSESS_FIELDS
            .iter()
            .filter_map(|field| {
                judgment
                    .get(*field)
                    .map(|value| (field.to_string(), value.clone()))
            })
            .collect();
        Value::Object(fields)
    }

    fn input_bytes(value: &Value) -> Vec<u8> {
        serde_json::to_vec(value).expect("input encodes")
    }

    fn rebuild_inclusive(snapshot: &UsageSnapshot) -> Option<Reconciliation> {
        (snapshot.snapshot_id == "snap-inclusive")
            .then(|| reconcile_fixture("inclusive_parent_counts_child_once"))
    }

    fn execute_assess_captured(
        store: &UsageStore,
        request: &AssessRequest,
        input: &[u8],
        recorded_at_unix_ms: u64,
    ) -> (ExitCode, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_assess(
            store,
            request,
            input,
            &rebuild_inclusive,
            recorded_at_unix_ms,
            &mut out,
            &mut err,
        );
        (
            code,
            String::from_utf8(out).expect("stdout is UTF-8"),
            String::from_utf8(err).expect("stderr is UTF-8"),
        )
    }

    #[test]
    fn usage_assess_parses_options_and_requires_each_flag() {
        let args =
            |list: &[&str]| -> Vec<String> { list.iter().map(|item| item.to_string()).collect() };

        assert_eq!(
            parse_assess_request(&args(&[
                "usage",
                "assess",
                "--run",
                "run-a",
                "--snapshot",
                "snap-a",
                "--input",
                "judgment.json",
            ])),
            Ok(assess_request("run-a", "snap-a"))
        );
        assert!(parse_assess_request(&args(&[
            "usage",
            "assess",
            "--snapshot",
            "snap-a",
            "--input",
            "judgment.json",
        ]))
        .unwrap_err()
        .contains("--run"));
        assert!(parse_assess_request(&args(&[
            "usage", "assess", "--run", "run-a", "--input", "j.json",
        ]))
        .unwrap_err()
        .contains("--snapshot"));
        assert!(parse_assess_request(&args(&[
            "usage",
            "assess",
            "--run",
            "run-a",
            "--snapshot",
            "snap-a",
        ]))
        .unwrap_err()
        .contains("--input"));
        assert!(parse_assess_request(&args(&[
            "usage",
            "assess",
            "--run",
            "run-a",
            "--snapshot",
            "snap-a",
            "--input",
            "j.json",
            "--json",
        ]))
        .is_err());
        assert!(parse_assess_request(&args(&["usage", "report", "--run", "run-a"])).is_err());
    }

    #[test]
    fn usage_assess_input_rejects_factual_fields_and_missing_metadata() {
        let base = assessment_input_value("inclusive_residual_is_proven");
        assert!(parse_assessment_input(&input_bytes(&base)).is_ok());

        for factual in [
            "tokens",
            "price",
            "rate_set_digest",
            "totals",
            "receipts",
            "observations",
            "assessment_id",
            "schema_version",
            "snapshot_digest",
            "recorded_at_unix_ms",
        ] {
            let mut input = base.clone();
            input[factual] = json!({"input": 5});
            let error = parse_assessment_input(&input_bytes(&input)).unwrap_err();
            assert!(error.contains(factual), "{error}");
        }

        let mut nested = base.clone();
        nested["assignments"][0]["tokens"] = json!(5);
        assert!(parse_assessment_input(&input_bytes(&nested))
            .unwrap_err()
            .contains("tokens"));

        let mut anonymous = base.clone();
        anonymous["author"] = json!("  ");
        assert!(parse_assessment_input(&input_bytes(&anonymous))
            .unwrap_err()
            .contains("`author`"));

        let mut unruled = base.clone();
        unruled
            .as_object_mut()
            .expect("input is an object")
            .remove("useful_work_rule");
        assert!(parse_assessment_input(&input_bytes(&unruled))
            .unwrap_err()
            .contains("useful_work_rule"));
    }

    #[test]
    fn usage_assess_publishes_judgment_and_keeps_snapshot_bytes() {
        let reconciled = reconcile_fixture("inclusive_parent_counts_child_once");
        let run_id = reconciled.run_id.clone();
        let (directory, store) = publish(&reconciled, "snap-inclusive", None);
        let before = store_state(directory.path());
        let input = input_bytes(&assessment_input_value("inclusive_residual_is_proven"));

        let (code, out, err) = execute_assess_captured(
            &store,
            &assess_request(&run_id, "snap-inclusive"),
            &input,
            20_000_000,
        );
        assert_eq!(code, ExitCode::Success, "{err}");
        assert!(err.is_empty());
        assert!(out.contains("(published)"), "{out}");
        assert!(out.contains("snapshot: snap-inclusive"), "{out}");

        let ids = record_ids::<AssessmentRecord>(&store, &run_id).expect("ids list");
        assert_eq!(ids.len(), 1);
        assert!(ids[0].starts_with("assess-"));
        assert!(out.contains(&ids[0]), "output names the identity: {out}");
        let stored: AssessmentRecord = store.read(&run_id, &ids[0]).expect("judgment reads");
        let snapshot: UsageSnapshot = store
            .read(&run_id, "snap-inclusive")
            .expect("snapshot reads");
        assert_eq!(
            stored.snapshot_digest,
            record_digest(&snapshot).expect("digest")
        );
        assert_eq!(stored.supersedes, None);
        assert_eq!(stored.recorded_at_unix_ms, 20_000_000);
        assert_kept(&before, &store_state(directory.path()));
    }

    #[test]
    fn usage_assess_revision_names_supersedes_and_keeps_both_versions() {
        let reconciled = reconcile_fixture("inclusive_parent_counts_child_once");
        let run_id = reconciled.run_id.clone();
        let (directory, store) = publish(&reconciled, "snap-inclusive", None);
        let base = assessment_input_value("inclusive_residual_is_proven");
        let request = assess_request(&run_id, "snap-inclusive");

        let (code, _, err) =
            execute_assess_captured(&store, &request, &input_bytes(&base), 20_000_000);
        assert_eq!(code, ExitCode::Success, "{err}");
        let first_id =
            record_ids::<AssessmentRecord>(&store, &run_id).expect("ids list")[0].clone();
        let before = store_state(directory.path());

        // The snapshot already has a judgment, so a new one must name the one it revises.
        let (code, out, err) =
            execute_assess_captured(&store, &request, &input_bytes(&base), 20_000_001);
        assert_eq!(code, ExitCode::Usage);
        assert!(out.is_empty());
        assert!(err.contains("supersedes"), "{err}");
        assert_eq!(store_state(directory.path()), before);

        let mut unknown = base.clone();
        unknown["supersedes"] = json!("assess-missing");
        let (code, _, err) =
            execute_assess_captured(&store, &request, &input_bytes(&unknown), 20_000_001);
        assert_eq!(code, ExitCode::Usage);
        assert!(err.contains("not a current assessment"), "{err}");
        assert_eq!(store_state(directory.path()), before);

        let mut revision = base.clone();
        revision["supersedes"] = json!(first_id);
        revision["useful_work_rule"] =
            json!("accepted only when the kept change reached the final diff");
        let (code, out, err) =
            execute_assess_captured(&store, &request, &input_bytes(&revision), 20_000_001);
        assert_eq!(code, ExitCode::Success, "{err}");
        assert!(out.contains("(published)"), "{out}");
        assert!(out.contains(&format!("supersedes: {first_id}")), "{out}");

        let ids = record_ids::<AssessmentRecord>(&store, &run_id).expect("ids list");
        assert_eq!(ids.len(), 2, "both versions stay readable");
        let revised: AssessmentRecord = ids
            .iter()
            .filter(|id| **id != first_id)
            .find_map(|id| store.read::<AssessmentRecord>(&run_id, id).ok())
            .expect("revision reads");
        assert_eq!(revised.supersedes.as_deref(), Some(first_id.as_str()));
        let original: AssessmentRecord = store.read(&run_id, &first_id).expect("original reads");
        assert_eq!(original.supersedes, None);
        assert_ne!(original.useful_work_rule, revised.useful_work_rule);
        assert_kept(&before, &store_state(directory.path()));
    }

    #[test]
    fn usage_assess_rejects_changed_factuals_and_unknown_rows_without_writing() {
        let reconciled = reconcile_fixture("inclusive_parent_counts_child_once");
        let run_id = reconciled.run_id.clone();
        let (directory, store) = publish(&reconciled, "snap-inclusive", None);
        let before = store_state(directory.path());
        let base = assessment_input_value("inclusive_residual_is_proven");
        let request = assess_request(&run_id, "snap-inclusive");

        let mut unknown_row = base.clone();
        unknown_row["assignments"][0]["contribution_id"] = json!("obs-missing/output");
        let mut evidence_free = base.clone();
        evidence_free["assignments"][0]["evidence"] = json!([]);
        let mut changed_tokens = base.clone();
        changed_tokens["tokens"] = json!({"input": 9_000_000});
        let mut changed_rates = base.clone();
        changed_rates["rate_set_digest"] = json!("rates-forged");
        let mut changed_totals = base.clone();
        changed_totals["totals"] = json!({"cost": "0"});
        let mut changed_receipt = base.clone();
        changed_receipt["receipts"] = json!(["forged"]);

        for (label, input, expected) in [
            ("unknown row", unknown_row, "obs-missing/output"),
            ("missing evidence", evidence_free, "evidence"),
            ("token change", changed_tokens, "tokens"),
            ("rate change", changed_rates, "rate_set_digest"),
            ("total change", changed_totals, "totals"),
            ("receipt change", changed_receipt, "receipts"),
        ] {
            let (code, out, err) =
                execute_assess_captured(&store, &request, &input_bytes(&input), 20_000_000);
            assert_eq!(code, ExitCode::Usage, "{label} is a usage error");
            assert!(out.is_empty(), "{label} prints no identity");
            assert!(err.contains(expected), "{label}: {err}");
            assert_eq!(
                store_state(directory.path()),
                before,
                "{label} leaves the store byte-identical"
            );
        }
    }

    #[test]
    fn usage_assess_corrupt_or_unrebuildable_snapshot_writes_nothing() {
        let reconciled = reconcile_fixture("inclusive_parent_counts_child_once");
        let run_id = reconciled.run_id.clone();
        let (directory, store) = publish(&reconciled, "snap-inclusive", None);
        let input = input_bytes(&assessment_input_value("inclusive_residual_is_proven"));
        let request = assess_request(&run_id, "snap-inclusive");

        // Without a rebuild the judgment cannot be checked, so nothing is written.
        let before = store_state(directory.path());
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_assess(
            &store,
            &request,
            &input,
            &|_: &UsageSnapshot| None,
            20_000_000,
            &mut out,
            &mut err,
        );
        assert_eq!(code, ExitCode::Error);
        assert!(out.is_empty());
        assert!(String::from_utf8_lossy(&err).contains("cannot be rebuilt"));
        assert_eq!(store_state(directory.path()), before);

        // A corrupt snapshot record is an integrity failure, and the store stays as it is.
        let record = store_state(directory.path())
            .into_keys()
            .find(|path| {
                path.file_name().and_then(|name| name.to_str())
                    == Some("snap-inclusive.record.json")
            })
            .expect("snapshot record file exists");
        fs::write(&record, b"{}").expect("record is overwritten");
        let corrupt = store_state(directory.path());
        let (code, out, _) = execute_assess_captured(&store, &request, &input, 20_000_000);
        assert_eq!(code, ExitCode::Error);
        assert!(out.is_empty());
        assert_eq!(store_state(directory.path()), corrupt);
        assert!(record_ids::<AssessmentRecord>(&store, &run_id)
            .expect("ids list")
            .is_empty());
    }
}
