//! Bounded telemetry collection for host orchestrator and child sessions.
//!
//! A collection reads one host session inside an explicit window. Codex
//! reports cumulative totals per session, so the usage of a window is the
//! latest in-window total minus the total before the window's first turn. That
//! baseline comes from the first in-window row, and every later row must
//! continue it exactly. Claude reports usage per response, so each response is
//! a delta, and a response split across rows counts once.
//!
//! A page stops at a byte or record budget and keeps its cursor at a complete
//! record boundary. A trailing Claude response that may still grow, and a
//! Codex turn whose usage row has not arrived, stay out of the observation
//! until a later page sees them. Hosts without a verified source produce an
//! unsupported entry, never guessed or zero usage.

use crate::dispatch::DispatchQuota;
use crate::executors::{
    count_in, scoped_observation, unsupported_cost, unsupported_token, CounterTotal,
};
use crate::host_session::{
    self, contains_token, epoch_seconds, HostKind, HostSessionError, HostSessionRef,
};
use crate::usage::{
    Availability, CounterRelation, CounterSemantics, DescendantScope, Metric, ModelRecord,
    ObservationWindow, SourceCursor, TokenCount, TokenMetric, TokenUsage, UsageCategory,
    UsageError, UsageObservation,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Largest number of bytes one collection page reads.
pub const PAGE_MAX_BYTES: u64 = 16 * 1024 * 1024;
/// Largest number of records one collection page reads.
pub const PAGE_MAX_RECORDS: usize = 10_000;
/// Records read to confirm a session's identity before its usage rows.
const HEADER_MAX_RECORDS: usize = 256;

const CODEX_SOURCE_VERSION: &str = "codex.rollout.token_count.v1";
const CLAUDE_SOURCE_VERSION: &str = "claude.transcript.assistant_usage.v1";
const UNVERIFIED_SOURCE_VERSION: &str = "host.unverified.v1";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HostUsageError {
    #[error(transparent)]
    Session(#[from] HostSessionError),
    #[error(transparent)]
    Usage(#[from] UsageError),
    #[error("host transcript resolves outside its root")]
    PathEscape,
    #[error("host session identity does not match the bound reference")]
    IdentityMismatch,
    #[error("cumulative totals have no baseline to subtract from")]
    MissingBaseline,
    #[error("cumulative totals do not continue the previous total")]
    ResetAssumption,
    #[error("host transcript record is malformed")]
    MalformedRecord,
    #[error("host transcript I/O failed: {0}")]
    Io(String),
}

/// Time bounds of the rows one collection may read. Host timestamps have second resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageWindow {
    start: u64,
    end: Option<u64>,
}

impl UsageWindow {
    /// Opens a window at `started_at` and leaves its end open.
    pub fn starting_at(started_at: &str) -> Result<Self, HostUsageError> {
        Ok(Self {
            start: epoch_seconds(started_at)?,
            end: None,
        })
    }

    /// Bounds a window to `[started_at, ended_at)`.
    pub fn closed(started_at: &str, ended_at: &str) -> Result<Self, HostUsageError> {
        let start = epoch_seconds(started_at)?;
        let end = epoch_seconds(ended_at)?;
        if end < start {
            return Err(UsageError::InvalidWindow { start, end }.into());
        }
        Ok(Self {
            start,
            end: Some(end),
        })
    }

    fn contains(&self, seconds: u64) -> bool {
        seconds >= self.start && self.end.is_none_or(|end| seconds < end)
    }
}

/// Directories that hold host session records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRoots {
    pub home: PathBuf,
    pub codex_root: PathBuf,
}

impl HostRoots {
    /// Uses `CODEX_HOME` when it is set, as the session verifier does.
    pub fn from_env(home: &Path) -> Self {
        let codex_root = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"));
        Self {
            home: home.to_path_buf(),
            codex_root,
        }
    }
}

/// Which host session a collection binds to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostUsageRole {
    /// The top-level session. Its reference is its own session ID.
    Orchestrator,
    /// A subagent bound to one host attempt by the attempt token in its task message.
    Child { attempt_id: String },
}

/// Cumulative counters of a Codex session, as one `token_count` row reports them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexTotals {
    pub input: u64,
    pub cached_input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_write: u64,
    pub total: u64,
}

impl CodexTotals {
    fn read(value: &Value) -> Result<Self, HostUsageError> {
        let field = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_u64)
                .ok_or(HostUsageError::MalformedRecord)
        };
        Ok(Self {
            input: field("input_tokens")?,
            cached_input: field("cached_input_tokens")?,
            output: field("output_tokens")?,
            reasoning: field("reasoning_output_tokens")?,
            cache_write: field("cache_write_input_tokens")?,
            total: field("total_tokens")?,
        })
    }

    fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            input: self.input.checked_add(other.input)?,
            cached_input: self.cached_input.checked_add(other.cached_input)?,
            output: self.output.checked_add(other.output)?,
            reasoning: self.reasoning.checked_add(other.reasoning)?,
            cache_write: self.cache_write.checked_add(other.cache_write)?,
            total: self.total.checked_add(other.total)?,
        })
    }

    fn checked_sub(self, other: Self) -> Option<Self> {
        Some(Self {
            input: self.input.checked_sub(other.input)?,
            cached_input: self.cached_input.checked_sub(other.cached_input)?,
            output: self.output.checked_sub(other.output)?,
            reasoning: self.reasoning.checked_sub(other.reasoning)?,
            cache_write: self.cache_write.checked_sub(other.cache_write)?,
            total: self.total.checked_sub(other.total)?,
        })
    }
}

/// Resume point of a bounded collection. Store it with the page that returned it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostUsageCursor {
    /// Identifies the session the offset belongs to.
    pub epoch: String,
    /// Byte offset of the first record the next page reads.
    pub offset: u64,
    /// End of the last counted interval, in Unix milliseconds.
    pub through_unix_ms: u64,
    /// Cumulative Codex totals already counted. `None` for Claude and before the first usage row.
    pub totals: Option<CodexTotals>,
    /// Activity was seen after the last usage row, so the final turn may be incomplete.
    pub activity_after_usage: bool,
}

impl HostUsageCursor {
    /// The cursor in the shared usage schema.
    pub fn source_cursor(&self) -> SourceCursor {
        SourceCursor {
            epoch: self.epoch.clone(),
            position: self.offset.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageStatus {
    /// The page read every record available when it ran.
    Complete,
    /// The page stopped at its byte or record budget. The next page resumes from the cursor.
    BudgetStop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostUsagePage {
    /// Zero or one observation. An empty list means no usage row was in the window, which is not zero usage.
    pub observations: Vec<UsageObservation>,
    pub cursor: HostUsageCursor,
    pub status: PageStatus,
    /// Activity or a response follows the last usage row, so final usage may still arrive.
    pub final_turn_pending: bool,
    /// Quota for the host child. A transcript proves no account identity for the attempt, so this
    /// scope is always unknown and never inherits the controller account's quota.
    pub quota: DispatchQuota,
}

/// Collects one bounded page of usage from a bound host session.
pub fn collect(
    reference: &HostSessionRef,
    role: &HostUsageRole,
    workdir: &Path,
    roots: &HostRoots,
    window: &UsageWindow,
    cursor: Option<&HostUsageCursor>,
) -> Result<HostUsagePage, HostUsageError> {
    if cursor.is_some_and(|cursor| cursor.epoch != epoch_key(reference)) {
        return Err(HostUsageError::IdentityMismatch);
    }
    match reference.host {
        HostKind::Codex => collect_codex(reference, role, workdir, roots, window, cursor),
        HostKind::Claude => collect_claude(reference, role, workdir, roots, window, cursor),
    }
}

/// Scope of a host child quota. The transcript names no account that is verified against the
/// attempt, so the scope stays unknown.
fn host_child_quota() -> DispatchQuota {
    DispatchQuota::UnknownScope {
        reason: "host child transcript carries no verified account identity".to_owned(),
    }
}

/// Capability entry for a host without a verified telemetry source. Every counter is unsupported.
pub fn unverified_host_observation(host: &str, reason: &str) -> UsageObservation {
    let token = || unsupported_token(reason);
    let tokens = TokenUsage {
        input: token(),
        output: token(),
        cached_input: token(),
        cache_write: token(),
        reasoning: token(),
        other: token(),
    };
    scoped_observation(
        host,
        UNVERIFIED_SOURCE_VERSION,
        tokens,
        unsupported_cost(reason),
        ModelRecord::default(),
    )
}

fn collect_codex(
    reference: &HostSessionRef,
    role: &HostUsageRole,
    workdir: &Path,
    roots: &HostRoots,
    window: &UsageWindow,
    cursor: Option<&HostUsageCursor>,
) -> Result<HostUsagePage, HostUsageError> {
    let path = locate_codex_transcript(reference, workdir, roots)?;
    codex_header(&path, reference, role, workdir)?;
    let resume = Resume::new(cursor, window);
    let mut lines = Lines::open(&path, resume.offset)?;
    let mut prev = resume.totals;
    let mut base = resume.totals;
    let mut latest = None;
    let mut through = resume.through_unix_ms;
    let mut pending = resume.activity_after_usage;
    let mut model = None;
    let mut status = PageStatus::Complete;
    loop {
        let line = match lines.next_line()? {
            Next::Line(line) => line,
            Next::Eof => break,
            Next::Budget => {
                status = PageStatus::BudgetStop;
                break;
            }
        };
        let Some(row) = parse_row(&line.text)? else {
            continue;
        };
        let Some(seconds) = timestamp_seconds(&row)? else {
            continue;
        };
        if !window.contains(seconds) {
            continue;
        }
        if row_type(&row) == Some("turn_context") {
            if let Some(name) = row.pointer("/payload/model").and_then(Value::as_str) {
                model = Some(name.to_owned());
            }
        } else if is_token_count(&row) {
            let Some(info) = row.pointer("/payload/info").filter(|info| !info.is_null()) else {
                continue;
            };
            let total = CodexTotals::read(info.get("total_token_usage").unwrap_or(&Value::Null))?;
            let last = info
                .get("last_token_usage")
                .filter(|value| !value.is_null())
                .map(CodexTotals::read)
                .transpose()?
                .ok_or(HostUsageError::MissingBaseline)?;
            let before = match prev {
                Some(previous) => previous,
                None => total
                    .checked_sub(last)
                    .ok_or(HostUsageError::ResetAssumption)?,
            };
            if before.checked_add(last) != Some(total) {
                return Err(HostUsageError::ResetAssumption);
            }
            base.get_or_insert(before);
            prev = Some(total);
            latest = Some(total);
            through = through.max(seconds * 1000);
            pending = false;
        } else if is_activity(&row) {
            pending = true;
        }
    }

    let mut observations = Vec::new();
    if let (Some(latest), Some(base)) = (latest, base) {
        let span = ObservationWindow::new(resume.through_unix_ms, through)?;
        let delta = latest
            .checked_sub(base)
            .ok_or(HostUsageError::ResetAssumption)?;
        let tokens = TokenUsage {
            input: counted(delta.input, span, CounterRelation::Exclusive)?,
            cached_input: counted(
                delta.cached_input,
                span,
                CounterRelation::SubsetOf {
                    category: UsageCategory::Input,
                },
            )?,
            output: counted(delta.output, span, CounterRelation::Exclusive)?,
            reasoning: counted(
                delta.reasoning,
                span,
                CounterRelation::SubsetOf {
                    category: UsageCategory::Output,
                },
            )?,
            // The rollout reports this counter, but its scope is not proven, so it is not additive.
            cache_write: counted(delta.cache_write, span, CounterRelation::Unknown)?,
            other: unsupported_token("codex rollout has no other counter"),
        };
        let observation = scoped_observation(
            reference.host.as_str(),
            CODEX_SOURCE_VERSION,
            tokens,
            unsupported_cost("codex rollout reports no cost"),
            ModelRecord {
                observed: model,
                configured: None,
            },
        );
        observations.push(identify(observation, reference, role, lines.offset));
    }
    Ok(HostUsagePage {
        observations,
        cursor: HostUsageCursor {
            epoch: epoch_key(reference),
            offset: lines.offset,
            through_unix_ms: through,
            totals: prev,
            activity_after_usage: pending,
        },
        status,
        final_turn_pending: pending,
        quota: host_child_quota(),
    })
}

fn collect_claude(
    reference: &HostSessionRef,
    role: &HostUsageRole,
    workdir: &Path,
    roots: &HostRoots,
    window: &UsageWindow,
    cursor: Option<&HostUsageCursor>,
) -> Result<HostUsagePage, HostUsageError> {
    let path = locate_claude_transcript(reference, role, workdir, roots)?;
    claude_header(&path, role, workdir)?;
    let resume = Resume::new(cursor, window);
    let mut lines = Lines::open(&path, resume.offset)?;
    let mut counters = ClaudeCounters::default();
    let mut committed = 0usize;
    let mut model = None;
    let mut through = resume.through_unix_ms;
    let mut open: Option<ClaudeResponse> = None;
    let mut status = PageStatus::Complete;
    loop {
        let line = match lines.next_line()? {
            Next::Line(line) => line,
            Next::Eof => break,
            Next::Budget => {
                status = PageStatus::BudgetStop;
                break;
            }
        };
        let Some(row) = parse_row(&line.text)? else {
            continue;
        };
        if row_type(&row) != Some("assistant") {
            continue;
        }
        let Some(seconds) = timestamp_seconds(&row)? else {
            continue;
        };
        if !window.contains(seconds) {
            continue;
        }
        let message = row.get("message").ok_or(HostUsageError::MalformedRecord)?;
        let Some(usage) = message.get("usage").filter(|usage| usage.is_object()) else {
            continue;
        };
        let id = message
            .get("id")
            .and_then(Value::as_str)
            .ok_or(HostUsageError::MalformedRecord)?;
        // Streamed content blocks repeat the same usage under one message ID.
        if open.as_ref().is_some_and(|response| response.id == id) {
            continue;
        }
        let response = ClaudeResponse {
            id: id.to_owned(),
            start: line.start,
            seconds,
            usage: usage.clone(),
            model: message
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        if let Some(done) = open.replace(response) {
            counters.add(&done.usage);
            committed += 1;
            through = through.max(done.seconds * 1000);
            model = model.or(done.model);
        }
    }

    // The trailing response may still gain rows, so the next page starts at its first row.
    let pending = open.is_some();
    let offset = open
        .as_ref()
        .map_or(lines.offset, |response| response.start);
    let mut observations = Vec::new();
    if committed > 0 {
        let span = ObservationWindow::new(resume.through_unix_ms, through)?;
        let tokens = TokenUsage {
            input: claude_counter(counters.input, span, CounterRelation::Exclusive),
            output: claude_counter(counters.output, span, CounterRelation::Exclusive),
            cached_input: claude_counter(counters.cache_read, span, CounterRelation::Exclusive),
            cache_write: claude_counter(counters.cache_creation, span, CounterRelation::Exclusive),
            reasoning: unsupported_token("claude transcript has no reasoning counter"),
            other: unsupported_token("claude transcript has no other counter"),
        };
        let observation = scoped_observation(
            reference.host.as_str(),
            CLAUDE_SOURCE_VERSION,
            tokens,
            unsupported_cost("claude transcript reports no cost"),
            ModelRecord {
                observed: model,
                configured: None,
            },
        );
        observations.push(identify(observation, reference, role, offset));
    }
    Ok(HostUsagePage {
        observations,
        cursor: HostUsageCursor {
            epoch: epoch_key(reference),
            offset,
            through_unix_ms: through,
            totals: None,
            activity_after_usage: false,
        },
        status,
        final_turn_pending: pending,
        quota: host_child_quota(),
    })
}

/// Counter sums of one Claude transcript page. Each response adds its usage once.
#[derive(Default)]
struct ClaudeCounters {
    input: CounterTotal,
    output: CounterTotal,
    cache_read: CounterTotal,
    cache_creation: CounterTotal,
}

impl ClaudeCounters {
    fn add(&mut self, usage: &Value) {
        self.input.push(count_in(Some(usage), "input_tokens"));
        self.output.push(count_in(Some(usage), "output_tokens"));
        self.cache_read
            .push(count_in(Some(usage), "cache_read_input_tokens"));
        self.cache_creation
            .push(count_in(Some(usage), "cache_creation_input_tokens"));
    }
}

/// One assistant response that may still gain content rows.
struct ClaudeResponse {
    id: String,
    start: u64,
    seconds: u64,
    usage: Value,
    model: Option<String>,
}

/// Where a page starts and what earlier pages already counted.
struct Resume {
    offset: u64,
    through_unix_ms: u64,
    totals: Option<CodexTotals>,
    activity_after_usage: bool,
}

impl Resume {
    fn new(cursor: Option<&HostUsageCursor>, window: &UsageWindow) -> Self {
        match cursor {
            Some(cursor) => Self {
                offset: cursor.offset,
                through_unix_ms: cursor.through_unix_ms,
                totals: cursor.totals,
                activity_after_usage: cursor.activity_after_usage,
            },
            None => Self {
                offset: 0,
                through_unix_ms: window.start * 1000,
                totals: None,
                activity_after_usage: false,
            },
        }
    }
}

/// Reads complete records from a byte offset, stopping at the page budget.
struct Lines {
    reader: BufReader<File>,
    offset: u64,
    bytes: u64,
    records: usize,
}

struct Line {
    start: u64,
    text: String,
}

enum Next {
    Line(Line),
    Eof,
    Budget,
}

impl Lines {
    fn open(path: &Path, offset: u64) -> Result<Self, HostUsageError> {
        let mut file = File::open(path).map_err(io_error)?;
        file.seek(SeekFrom::Start(offset)).map_err(io_error)?;
        Ok(Self {
            reader: BufReader::new(file),
            offset,
            bytes: 0,
            records: 0,
        })
    }

    /// Reads the next complete record. A fragment without a newline waits for a later page.
    fn next_line(&mut self) -> Result<Next, HostUsageError> {
        if self.records >= PAGE_MAX_RECORDS {
            return Ok(Next::Budget);
        }
        // Reading stops one byte past the budget, so an oversized record is never held whole.
        let remaining = PAGE_MAX_BYTES - self.bytes;
        let mut text = String::new();
        let read = (&mut self.reader)
            .take(remaining + 1)
            .read_line(&mut text)
            .map_err(io_error)? as u64;
        if !text.ends_with('\n') {
            return Ok(if read > remaining {
                Next::Budget
            } else {
                Next::Eof
            });
        }
        if self.bytes + read > PAGE_MAX_BYTES {
            return Ok(Next::Budget);
        }
        let start = self.offset;
        self.offset += read;
        self.bytes += read;
        self.records += 1;
        Ok(Next::Line(Line { start, text }))
    }
}

fn io_error(error: std::io::Error) -> HostUsageError {
    HostUsageError::Io(error.to_string())
}

fn epoch_key(reference: &HostSessionRef) -> String {
    format!("{}:{}", reference.host.as_str(), reference.reference)
}

fn locate_codex_transcript(
    reference: &HostSessionRef,
    workdir: &Path,
    roots: &HostRoots,
) -> Result<PathBuf, HostUsageError> {
    // The window bounds the rows, so the rollout search does not filter by day.
    let path =
        host_session::locate_codex(&roots.codex_root, workdir, &reference.reference, "epoch+0s")?;
    ensure_under(&path, &roots.codex_root)?;
    Ok(path)
}

fn locate_claude_transcript(
    reference: &HostSessionRef,
    role: &HostUsageRole,
    workdir: &Path,
    roots: &HostRoots,
) -> Result<PathBuf, HostUsageError> {
    let path = match role {
        HostUsageRole::Orchestrator => {
            host_session::locate_claude_session(&roots.home, workdir, &reference.reference)?
        }
        HostUsageRole::Child { .. } => {
            host_session::locate_claude(&roots.home, workdir, &reference.reference)?.0
        }
    };
    ensure_under(&path, &roots.home.join(".claude/projects"))?;
    Ok(path)
}

fn ensure_under(path: &Path, root: &Path) -> Result<(), HostUsageError> {
    if host_session::canonical(path).starts_with(host_session::canonical(root)) {
        Ok(())
    } else {
        Err(HostUsageError::PathEscape)
    }
}

/// Checks the Codex session header: its identity, its working directory, and for a child its attempt token.
fn codex_header(
    path: &Path,
    reference: &HostSessionRef,
    role: &HostUsageRole,
    workdir: &Path,
) -> Result<(), HostUsageError> {
    let mut lines = Lines::open(path, 0)?;
    let mut meta = None;
    let mut attempt_bound = false;
    for _ in 0..HEADER_MAX_RECORDS {
        let Next::Line(line) = lines.next_line()? else {
            break;
        };
        let Some(row) = parse_row(&line.text)? else {
            continue;
        };
        if row_type(&row) == Some("session_meta") && meta.is_none() {
            meta = Some(row);
        } else if is_token_count(&row) {
            break;
        } else if let HostUsageRole::Child { attempt_id } = role {
            attempt_bound |= is_task_text(&row)
                && codex_texts(&row)
                    .iter()
                    .any(|text| contains_token(text, attempt_id));
        }
    }
    let meta = meta.ok_or(HostUsageError::MalformedRecord)?;
    let payload = meta.get("payload").ok_or(HostUsageError::MalformedRecord)?;
    let cwd = payload
        .get("cwd")
        .and_then(Value::as_str)
        .ok_or(HostUsageError::MalformedRecord)?;
    if host_session::canonical(Path::new(cwd)) != host_session::canonical(workdir) {
        return Err(HostSessionError::CwdMismatch.into());
    }
    match role {
        HostUsageRole::Orchestrator => {
            if payload.get("id").and_then(Value::as_str) != Some(reference.reference.as_str()) {
                return Err(HostUsageError::IdentityMismatch);
            }
        }
        HostUsageRole::Child { .. } => {
            if payload.get("thread_source").and_then(Value::as_str) != Some("subagent") {
                return Err(HostUsageError::IdentityMismatch);
            }
            if !attempt_bound {
                return Err(HostSessionError::AttemptIdNotInUserMessage.into());
            }
        }
    }
    Ok(())
}

/// Checks the Claude transcript header: its working directory, and for a child its attempt token.
fn claude_header(path: &Path, role: &HostUsageRole, workdir: &Path) -> Result<(), HostUsageError> {
    let mut lines = Lines::open(path, 0)?;
    let mut cwd = None;
    let mut attempt_bound = false;
    for _ in 0..HEADER_MAX_RECORDS {
        let Next::Line(line) = lines.next_line()? else {
            break;
        };
        let Some(row) = parse_row(&line.text)? else {
            continue;
        };
        if cwd.is_none() {
            cwd = row.get("cwd").and_then(Value::as_str).map(str::to_owned);
        }
        if row_type(&row) == Some("assistant") {
            break;
        }
        if let HostUsageRole::Child { attempt_id } = role {
            attempt_bound |= row_type(&row) == Some("user")
                && claude_user_texts(&row)
                    .iter()
                    .any(|text| contains_token(text, attempt_id));
        }
    }
    let cwd = cwd.ok_or(HostUsageError::MalformedRecord)?;
    if host_session::canonical(Path::new(&cwd)) != host_session::canonical(workdir) {
        return Err(HostSessionError::CwdMismatch.into());
    }
    if matches!(role, HostUsageRole::Child { .. }) && !attempt_bound {
        return Err(HostSessionError::AttemptIdNotInUserMessage.into());
    }
    Ok(())
}

fn identify(
    mut observation: UsageObservation,
    reference: &HostSessionRef,
    role: &HostUsageRole,
    position: u64,
) -> UsageObservation {
    observation.identity.session_id = Some(reference.reference.clone());
    observation.identity.attempt_id = match role {
        HostUsageRole::Orchestrator => None,
        HostUsageRole::Child { attempt_id } => Some(attempt_id.clone()),
    };
    observation.source.cursor = Some(SourceCursor {
        epoch: epoch_key(reference),
        position: position.to_string(),
    });
    observation
}

fn counted(
    value: u64,
    span: ObservationWindow,
    relation: CounterRelation,
) -> Result<TokenMetric, HostUsageError> {
    Ok(TokenMetric {
        count: Metric {
            value: Availability::Reported {
                value: TokenCount::new(value)?,
            },
            semantics: CounterSemantics::Delta,
            descendants: DescendantScope::Own,
            window: Some(span),
        },
        relation,
    })
}

fn claude_counter(
    total: CounterTotal,
    span: ObservationWindow,
    relation: CounterRelation,
) -> TokenMetric {
    let metric = total.finish(CounterSemantics::Delta, DescendantScope::Own, None);
    TokenMetric {
        count: Metric {
            window: Some(span),
            ..metric
        },
        relation,
    }
}

fn parse_row(text: &str) -> Result<Option<Value>, HostUsageError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed)
        .map(Some)
        .map_err(|_| HostUsageError::MalformedRecord)
}

fn timestamp_seconds(row: &Value) -> Result<Option<u64>, HostUsageError> {
    Ok(row
        .get("timestamp")
        .and_then(Value::as_str)
        .map(epoch_seconds)
        .transpose()?)
}

fn row_type(row: &Value) -> Option<&str> {
    row.get("type").and_then(Value::as_str)
}

fn is_token_count(row: &Value) -> bool {
    row_type(row) == Some("event_msg")
        && row.pointer("/payload/type").and_then(Value::as_str) == Some("token_count")
}

fn is_user_message(row: &Value) -> bool {
    row_type(row) == Some("response_item")
        && row.pointer("/payload/type").and_then(Value::as_str) == Some("message")
        && row.pointer("/payload/role").and_then(Value::as_str) == Some("user")
}

/// A Codex row that carries the task text a subagent receives.
fn is_task_text(row: &Value) -> bool {
    is_user_message(row)
        || (row_type(row) == Some("response_item")
            && row.pointer("/payload/type").and_then(Value::as_str) == Some("agent_message"))
}

/// Any response or tool row. Its presence after the last usage row means the turn is not final.
fn is_activity(row: &Value) -> bool {
    row_type(row) == Some("response_item") && !is_user_message(row)
}

fn codex_texts(row: &Value) -> Vec<&str> {
    row.pointer("/payload/content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|block| {
            matches!(
                block.get("type").and_then(Value::as_str),
                Some("input_text" | "text")
            )
        })
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect()
}

fn claude_user_texts(row: &Value) -> Vec<&str> {
    match row.pointer("/message/content") {
        Some(Value::String(text)) => vec![text.as_str()],
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect(),
        _ => Vec::new(),
    }
}
