//! `gal release-notes` — draft a deterministic CHANGELOG section from a commit range.
//!
//! Internal binary subcommand, NOT a public `/gal` slash command. Peer of `release`.
//!
//! Usage:
//!   gal release-notes [<from>..<to>]
//!
//! No argument: `<from>` is the numerically highest HEAD-reachable exact stable
//! `vMAJOR.MINOR.PATCH` tag, `<to>` is `HEAD`; no stable tag fails prescriptively.
//! Explicit form: exactly one non-empty `<from>..<to>` argument; `...` separators,
//! option-like tokens, and extra arguments are rejected before any git call. Both
//! endpoints are resolved to commit hashes before the range is constructed.
//!
//! Commits are read via `git log --no-merges` as NUL-framed hash/subject/body
//! triples; a merge commit itself never appears, but every commit it landed still
//! does, since `--no-merges` only removes merge commits from the output, not from
//! history traversal. Any git spawn failure, nonzero exit status, invalid UTF-8, or
//! truncated record framing aborts before any output is written.

use std::io::Write;
use std::path::Path;
use std::process::Command;

use gal_engine::ExitCode;

use crate::gal::naming_gate::{NamingGate, ViolationKind};

/// Output of a single git invocation, normalized away from `std::process::Output`
/// so fakes can be constructed in tests without fabricating a platform `ExitStatus`.
pub(crate) struct GitOutput {
    pub(crate) success: bool,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

/// Abstraction over git subprocess invocation, injectable so failure modes
/// (spawn error, nonzero status, invalid UTF-8, truncated framing) are testable
/// without depending on a real git binary misbehaving.
pub(crate) trait GitRunner {
    fn run(&self, repo_dir: &Path, args: &[&str]) -> Result<GitOutput, String>;
}

pub(crate) struct SystemGit;

impl GitRunner for SystemGit {
    fn run(&self, repo_dir: &Path, args: &[&str]) -> Result<GitOutput, String> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(args)
            .output()
            .map_err(|e| format!("git spawn failed: {e}"))?;
        Ok(GitOutput {
            success: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

/// One surviving commit: full hash, subject, and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommitRecord {
    pub(crate) hash: String,
    pub(crate) subject: String,
    pub(crate) body: String,
}

/// Parsed argv shape for `gal release-notes [<from>..<to>]`.
#[derive(Debug, PartialEq, Eq)]
enum RangeRequest<'a> {
    Explicit(&'a str, &'a str),
    Default,
}

/// Parse the argument slice (index 0 is the subcommand token itself).
fn parse_args(args: &[String]) -> Result<RangeRequest<'_>, String> {
    let rest = &args[1..];
    match rest.len() {
        0 => Ok(RangeRequest::Default),
        1 => {
            let raw = rest[0].as_str();
            if raw.starts_with('-') {
                return Err(format!("unrecognized option '{raw}'"));
            }
            if raw.contains("...") {
                return Err(format!("range must use '..' not '...': '{raw}'"));
            }
            let (from, to) = raw
                .split_once("..")
                .ok_or_else(|| format!("expected exactly one '<from>..<to>' range, got '{raw}'"))?;
            if from.is_empty() || to.is_empty() {
                return Err(format!("both range endpoints are required in '{raw}'"));
            }
            if to.contains("..") {
                return Err(format!("range must contain exactly one '..': '{raw}'"));
            }
            Ok(RangeRequest::Explicit(from, to))
        }
        _ => Err("unexpected extra arguments after the range".to_string()),
    }
}

/// Resolve `rev` to a full commit hash, failing closed on any git error.
fn resolve_hash(repo_dir: &Path, git: &dyn GitRunner, rev: &str) -> Result<String, String> {
    let out = git.run(
        repo_dir,
        &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
    )?;
    if !out.success {
        return Err(format!(
            "cannot resolve '{rev}' to a commit: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8(out.stdout)
        .map_err(|e| format!("invalid UTF-8 resolving '{rev}': {e}"))?;
    let hash = text.trim().to_string();
    if hash.is_empty() {
        return Err(format!("cannot resolve '{rev}' to a commit"));
    }
    Ok(hash)
}

/// Parse a tag as an exact stable `vMAJOR.MINOR.PATCH` (no prerelease/build suffix).
fn parse_stable_semver(tag: &str) -> Option<(u64, u64, u64)> {
    let rest = tag.strip_prefix('v')?;
    let mut parts = rest.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// The numerically highest HEAD-reachable exact stable `vMAJOR.MINOR.PATCH` tag.
fn highest_stable_tag(repo_dir: &Path, git: &dyn GitRunner) -> Result<String, String> {
    let out = git.run(repo_dir, &["tag", "--merged", "HEAD"])?;
    if !out.success {
        return Err(format!(
            "failed to list HEAD-reachable tags: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text =
        String::from_utf8(out.stdout).map_err(|e| format!("invalid UTF-8 listing tags: {e}"))?;

    let mut best: Option<((u64, u64, u64), String)> = None;
    for line in text.lines() {
        let tag = line.trim();
        if let Some(version) = parse_stable_semver(tag) {
            let replace = match &best {
                Some((best_version, _)) => version > *best_version,
                None => true,
            };
            if replace {
                best = Some((version, tag.to_string()));
            }
        }
    }
    best.map(|(_, tag)| tag)
        .ok_or_else(|| "no HEAD-reachable stable vMAJOR.MINOR.PATCH tag found".to_string())
}

/// Resolve the request to a `(from_hash, to_hash)` pair.
fn resolve_range(
    request: RangeRequest<'_>,
    repo_dir: &Path,
    git: &dyn GitRunner,
) -> Result<(String, String), String> {
    match request {
        RangeRequest::Explicit(from, to) => {
            let from_hash = resolve_hash(repo_dir, git, from)?;
            let to_hash = resolve_hash(repo_dir, git, to)?;
            Ok((from_hash, to_hash))
        }
        RangeRequest::Default => {
            let tag = highest_stable_tag(repo_dir, git)?;
            let from_hash = resolve_hash(repo_dir, git, &tag)?;
            let to_hash = resolve_hash(repo_dir, git, "HEAD")?;
            Ok((from_hash, to_hash))
        }
    }
}

/// Split raw `git log` stdout (NUL-framed hash/subject/body triples, each field
/// followed by a NUL) into complete records. Any incomplete trailing group is a
/// truncated-framing failure.
///
/// `git log --format=...` always appends its own `\n` after each entry in
/// addition to our `%x00` terminator, so every entry boundary in the raw stream
/// looks like `...\0\n<next-hash>`. That `\n` is not part of any field — strip it
/// before splitting on NUL, or it silently prefixes the next record's hash.
fn parse_records(stdout: &[u8]) -> Result<Vec<CommitRecord>, String> {
    let text = String::from_utf8(stdout.to_vec())
        .map_err(|e| format!("invalid UTF-8 in git log output: {e}"))?;
    let normalized = text.replace("\u{0}\n", "\u{0}");
    let mut fields: Vec<&str> = normalized.split('\u{0}').collect();
    match fields.last() {
        Some(&"") => {
            fields.pop();
        }
        _ => return Err("truncated git log record framing".to_string()),
    }
    if !fields.len().is_multiple_of(3) {
        return Err("truncated git log record framing".to_string());
    }
    Ok(fields
        .chunks(3)
        .map(|chunk| CommitRecord {
            hash: chunk[0].to_string(),
            subject: chunk[1].to_string(),
            body: chunk[2].to_string(),
        })
        .collect())
}

/// Read commits in `from..to`, excluding merge commits from the output.
fn read_commits(
    repo_dir: &Path,
    git: &dyn GitRunner,
    from: &str,
    to: &str,
) -> Result<Vec<CommitRecord>, String> {
    let range = format!("{from}..{to}");
    let out = git.run(
        repo_dir,
        &["log", "--no-merges", "--format=%H%x00%s%x00%b%x00", &range],
    )?;
    if !out.success {
        return Err(format!(
            "git log failed for range '{range}': {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    parse_records(&out.stdout)
}

/// The CHANGELOG section a surviving commit is grouped under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Added,
    Changed,
    Fixed,
    Other,
}

impl Section {
    fn heading(self) -> &'static str {
        match self {
            Section::Added => "### Added",
            Section::Changed => "### Changed",
            Section::Fixed => "### Fixed",
            Section::Other => "### Other",
        }
    }
}

/// True when the subject carries a `!` right before its `: description` split
/// (with or without a scope), or the body carries a `BREAKING CHANGE:` or
/// `BREAKING-CHANGE:` trailer.
fn is_breaking(subject: &str, body: &str) -> bool {
    if let Some((head, _)) = subject.split_once(": ") {
        if head.ends_with('!') {
            return true;
        }
    }
    body.contains("BREAKING CHANGE:") || body.contains("BREAKING-CHANGE:")
}

/// Classify one survivor. Breaking markers take precedence over the type
/// mapping; an unrecognized (or breaking-free `!`-less) type falls into
/// `Other` rather than disappearing.
fn classify(record: &CommitRecord) -> Section {
    if is_breaking(&record.subject, &record.body) {
        return Section::Changed;
    }
    match parse_conventional(&record.subject) {
        Some(("feat", _, _)) => Section::Added,
        Some(("fix", _, _)) => Section::Fixed,
        _ => Section::Other,
    }
}

/// Group survivors into the four sections, preserving git-log order within
/// each group. Every survivor appears in exactly one section.
fn group_by_section(records: Vec<CommitRecord>) -> [(Section, Vec<CommitRecord>); 4] {
    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut fixed = Vec::new();
    let mut other = Vec::new();
    for record in records {
        match classify(&record) {
            Section::Added => added.push(record),
            Section::Changed => changed.push(record),
            Section::Fixed => fixed.push(record),
            Section::Other => other.push(record),
        }
    }
    [
        (Section::Added, added),
        (Section::Changed, changed),
        (Section::Fixed, fixed),
        (Section::Other, other),
    ]
}

/// True when `subject` contains a plan-task-ID naming violation, per the
/// naming authority's `NamingGate`. Reuses the single existing scanner and
/// regex rather than introducing a second task-ID pattern.
fn has_plan_task_id(gate: &NamingGate, subject: &str) -> bool {
    gate.scan_content(subject)
        .iter()
        .any(|v| v.kind == ViolationKind::PlanTaskId)
}

fn print_records(
    out: &mut dyn Write,
    sections: &[(Section, Vec<CommitRecord>)],
    gate: &NamingGate,
) -> std::io::Result<()> {
    for (section, records) in sections {
        if records.is_empty() {
            continue;
        }
        writeln!(out, "{}", section.heading())?;
        for record in records {
            let short = &record.hash[..record.hash.len().min(7)];
            let sentinel = format!("[[GAL-RELEASE-DRAFT:{short}]]");
            // [REWRITE] wins when both apply: a plan-task-ID subject in the
            // residual Other group still needs rewriting before naming-gate,
            // not merely a review nudge.
            let flag = if has_plan_task_id(gate, &record.subject) {
                " [REWRITE]"
            } else if matches!(section, Section::Other) {
                " [REVIEW]"
            } else {
                ""
            };
            let callout = match section {
                Section::Changed => " [BREAKING]",
                Section::Added | Section::Fixed | Section::Other => "",
            };
            writeln!(out, "{sentinel} {short} {}{callout}{flag}", record.subject)?;
        }
    }
    Ok(())
}

/// Frozen process-noise allowlist: exact `(type, scope, subject-prefix)` tuples.
/// A commit is dropped only when all three match completely — the same scope
/// with any other subject prefix survives into `### Other`, so a future real
/// pipeline or finalize change can never silently disappear.
const NOISE_ALLOWLIST: &[(&str, &str, &str)] = &[
    ("chore", "pipeline", "converge"),
    ("chore", "pipeline", "goal-backward verify"),
    ("chore", "finalize", "land"),
    ("chore", "finalize", "close out"),
];

/// Split a Conventional Commit subject into `(type, scope, description)`.
/// A subject with no `type(scope): description` or `type: description` shape
/// yields `None` and is never treated as noise.
fn parse_conventional(subject: &str) -> Option<(&str, Option<&str>, &str)> {
    let (head, description) = subject.split_once(": ")?;
    let head = head.strip_suffix('!').unwrap_or(head);
    match head.find('(') {
        Some(open) => {
            let close = head.rfind(')')?;
            if close <= open || close != head.len() - 1 {
                return None;
            }
            Some((&head[..open], Some(&head[open + 1..close]), description))
        }
        None => Some((head, None, description)),
    }
}

/// True when `subject` is a complete match against `NOISE_ALLOWLIST`: same
/// Conventional Commit type, same scope, and the description starts with the
/// allowlisted prefix.
fn is_noise(subject: &str) -> bool {
    let Some((kind, scope, description)) = parse_conventional(subject) else {
        return false;
    };
    let Some(scope) = scope else {
        return false;
    };
    NOISE_ALLOWLIST
        .iter()
        .any(|(t, s, prefix)| *t == kind && *s == scope && description.starts_with(prefix))
}

/// Drop complete `NOISE_ALLOWLIST` matches, returning the survivors in
/// git-log order plus the number of dropped commits.
fn filter_noise(records: Vec<CommitRecord>) -> (Vec<CommitRecord>, usize) {
    let mut dropped = 0usize;
    let survivors = records
        .into_iter()
        .filter(|r| {
            if is_noise(&r.subject) {
                dropped += 1;
                false
            } else {
                true
            }
        })
        .collect();
    (survivors, dropped)
}

fn run_release_notes(
    args: &[String],
    repo_dir: &Path,
    git: &dyn GitRunner,
    out: &mut dyn Write,
) -> ExitCode {
    let request = match parse_args(args) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gal release-notes: {e}");
            eprintln!("Usage: gal release-notes [<from>..<to>]");
            return ExitCode::Usage;
        }
    };

    let (from, to) = match resolve_range(request, repo_dir, git) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("gal release-notes: {e}");
            return ExitCode::Error;
        }
    };

    let records = match read_commits(repo_dir, git, &from, &to) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gal release-notes: {e}");
            return ExitCode::Error;
        }
    };

    let (survivors, dropped) = filter_noise(records);
    let sections = group_by_section(survivors);

    let gate = match NamingGate::new(&[]) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("gal release-notes: failed to build naming gate: {e}");
            return ExitCode::Error;
        }
    };

    if let Err(e) = print_records(out, &sections, &gate) {
        eprintln!("gal release-notes: failed to write output: {e}");
        return ExitCode::Error;
    }
    if let Err(e) = writeln!(out, "Dropped {dropped} process-noise commit(s).") {
        eprintln!("gal release-notes: failed to write output: {e}");
        return ExitCode::Error;
    }

    ExitCode::Success
}

pub(crate) fn cmd_release_notes(args: &[String]) -> ExitCode {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    run_release_notes(args, Path::new("."), &SystemGit, &mut handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── parse_args: pure, no git required ───────────────────────────────────

    #[test]
    fn parse_args_no_range_is_default() {
        let args = vec!["release-notes".to_string()];
        assert_eq!(parse_args(&args).unwrap(), RangeRequest::Default);
    }

    #[test]
    fn parse_args_explicit_range() {
        let args = vec!["release-notes".to_string(), "v0.1.1..v0.1.2".to_string()];
        assert_eq!(
            parse_args(&args).unwrap(),
            RangeRequest::Explicit("v0.1.1", "v0.1.2")
        );
    }

    #[test]
    fn parse_args_rejects_three_dots() {
        let args = vec!["release-notes".to_string(), "v0.1.1...v0.1.2".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_args_rejects_empty_from() {
        let args = vec!["release-notes".to_string(), "..v0.1.2".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_args_rejects_empty_to() {
        let args = vec!["release-notes".to_string(), "v0.1.1..".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_args_rejects_option_like_token() {
        let args = vec!["release-notes".to_string(), "--bogus".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parse_args_rejects_extra_arguments() {
        let args = vec![
            "release-notes".to_string(),
            "v0.1.1..v0.1.2".to_string(),
            "extra".to_string(),
        ];
        assert!(parse_args(&args).is_err());
    }

    // ── parse_stable_semver ──────────────────────────────────────────────────

    #[test]
    fn parse_stable_semver_accepts_exact_form() {
        assert_eq!(parse_stable_semver("v0.1.2"), Some((0, 1, 2)));
    }

    #[test]
    fn parse_stable_semver_rejects_prerelease() {
        assert_eq!(parse_stable_semver("v0.1.2-rc1"), None);
    }

    #[test]
    fn parse_stable_semver_rejects_non_semver() {
        assert_eq!(parse_stable_semver("latest"), None);
        assert_eq!(parse_stable_semver("v1"), None);
    }

    // ── parse_records: NUL-framed triples ────────────────────────────────────

    #[test]
    fn parse_records_reads_complete_triples() {
        let raw = b"aaa\0feat: one\0body one\0bbb\0fix: two\0\0";
        let records = parse_records(raw).unwrap();
        assert_eq!(
            records,
            vec![
                CommitRecord {
                    hash: "aaa".into(),
                    subject: "feat: one".into(),
                    body: "body one".into(),
                },
                CommitRecord {
                    hash: "bbb".into(),
                    subject: "fix: two".into(),
                    body: "".into(),
                },
            ]
        );
    }

    #[test]
    fn parse_records_empty_output_is_zero_records() {
        assert_eq!(parse_records(b"").unwrap(), Vec::new());
    }

    #[test]
    fn parse_records_truncated_triple_fails() {
        let raw = b"aaa\0feat: one\0";
        assert!(parse_records(raw).is_err());
    }

    #[test]
    fn parse_records_invalid_utf8_fails() {
        let raw = &[0xff, 0xfe, 0x00];
        assert!(parse_records(raw).is_err());
    }

    // ── injectable GitRunner: spawn/status/UTF-8/framing failures ───────────

    enum ScriptedFailure {
        SpawnFail,
        NonzeroStatus,
        InvalidUtf8,
        TruncatedFraming,
    }

    struct ScriptedGit(ScriptedFailure);

    impl GitRunner for ScriptedGit {
        fn run(&self, _repo_dir: &Path, args: &[&str]) -> Result<GitOutput, String> {
            // Endpoint resolution (rev-parse) always succeeds so the failure is
            // isolated to the `git log` call under test.
            if args[0] == "rev-parse" {
                return Ok(GitOutput {
                    success: true,
                    stdout: b"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n".to_vec(),
                    stderr: Vec::new(),
                });
            }
            assert_eq!(args[0], "log");
            match self.0 {
                ScriptedFailure::SpawnFail => Err("git spawn failed: boom".to_string()),
                ScriptedFailure::NonzeroStatus => Ok(GitOutput {
                    success: false,
                    stdout: Vec::new(),
                    stderr: b"fatal: bad range".to_vec(),
                }),
                ScriptedFailure::InvalidUtf8 => Ok(GitOutput {
                    success: true,
                    stdout: vec![0xff, 0xfe, 0x00],
                    stderr: Vec::new(),
                }),
                ScriptedFailure::TruncatedFraming => Ok(GitOutput {
                    success: true,
                    stdout: b"abc123\0subject only\0".to_vec(),
                    stderr: Vec::new(),
                }),
            }
        }
    }

    fn assert_range_call_fails_closed(failure: ScriptedFailure) {
        let git = ScriptedGit(failure);
        let args = vec!["release-notes".to_string(), "aaa..bbb".to_string()];
        let mut out: Vec<u8> = Vec::new();
        let code = run_release_notes(&args, Path::new("."), &git, &mut out);
        assert_eq!(code, ExitCode::Error);
        assert!(out.is_empty(), "no partial draft on failure");
    }

    #[test]
    fn git_log_spawn_failure_fails_closed_with_empty_stdout() {
        assert_range_call_fails_closed(ScriptedFailure::SpawnFail);
    }

    #[test]
    fn git_log_nonzero_status_fails_closed_with_empty_stdout() {
        assert_range_call_fails_closed(ScriptedFailure::NonzeroStatus);
    }

    #[test]
    fn git_log_invalid_utf8_fails_closed_with_empty_stdout() {
        assert_range_call_fails_closed(ScriptedFailure::InvalidUtf8);
    }

    #[test]
    fn git_log_truncated_framing_fails_closed_with_empty_stdout() {
        assert_range_call_fails_closed(ScriptedFailure::TruncatedFraming);
    }

    // ── filter_noise: frozen allowlist tuples ────────────────────────────────

    fn record(hash: &str, subject: &str) -> CommitRecord {
        CommitRecord {
            hash: hash.to_string(),
            subject: subject.to_string(),
            body: String::new(),
        }
    }

    #[test]
    fn filter_noise_drops_only_complete_allowlist_matches() {
        let records = vec![
            record("aaa", "chore(pipeline): converge task state"),
            record("bbb", "chore(pipeline): goal-backward verify VERIFIED"),
            record("ccc", "chore(finalize): land some-slug (ABSORBED)"),
            record("ddd", "chore(finalize): close out task, all work complete"),
            record("eee", "chore(pipeline): widen the release gate"),
            record("fff", "feat(cli): add release-notes command"),
        ];

        let (survivors, dropped) = filter_noise(records);

        assert_eq!(
            survivors,
            vec![
                record("eee", "chore(pipeline): widen the release gate"),
                record("fff", "feat(cli): add release-notes command"),
            ]
        );
        assert_eq!(dropped, 4);
    }

    #[test]
    fn filter_noise_footer_discloses_dropped_count() {
        let git = ScriptedNoiseGit;
        let args = vec!["release-notes".to_string(), "aaa..bbb".to_string()];
        let mut out: Vec<u8> = Vec::new();
        let code = run_release_notes(&args, Path::new("."), &git, &mut out);
        assert_eq!(code, ExitCode::Success);
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("feat(cli): add release-notes command"));
        assert!(!printed.contains("chore(pipeline): converge"));
        assert!(printed.contains("Dropped 1 process-noise commit(s)."));
    }

    struct ScriptedNoiseGit;

    impl GitRunner for ScriptedNoiseGit {
        fn run(&self, _repo_dir: &Path, args: &[&str]) -> Result<GitOutput, String> {
            if args[0] == "rev-parse" {
                return Ok(GitOutput {
                    success: true,
                    stdout: b"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n".to_vec(),
                    stderr: Vec::new(),
                });
            }
            assert_eq!(args[0], "log");
            Ok(GitOutput {
                success: true,
                stdout: b"aaa\0chore(pipeline): converge task state\0\0bbb\0feat(cli): add release-notes command\0\0"
                    .to_vec(),
                stderr: Vec::new(),
            })
        }
    }

    // ── group_by_section: type mapping, breaking precedence, order ─────────

    #[test]
    fn group_by_section_maps_feat_fix_breaking_and_unknown() {
        let records = vec![
            record("aaa", "feat(cli): add a thing"),
            record("bbb", "fix(cli): correct a thing"),
            record("ccc", "feat(cli)!: replace the config format"),
            record("ddd", "docs: update the readme"),
        ];

        let sections = group_by_section(records);

        assert_eq!(sections[0].0, Section::Added);
        assert_eq!(sections[0].1, vec![record("aaa", "feat(cli): add a thing")]);

        assert_eq!(sections[1].0, Section::Changed);
        assert_eq!(
            sections[1].1,
            vec![record("ccc", "feat(cli)!: replace the config format")]
        );

        assert_eq!(sections[2].0, Section::Fixed);
        assert_eq!(
            sections[2].1,
            vec![record("bbb", "fix(cli): correct a thing")]
        );

        assert_eq!(sections[3].0, Section::Other);
        assert_eq!(
            sections[3].1,
            vec![record("ddd", "docs: update the readme")]
        );
    }

    #[test]
    fn group_by_section_body_breaking_trailer_takes_precedence_over_fix() {
        let mut fix_with_trailer = record("eee", "fix(cli): correct a thing");
        fix_with_trailer.body = "BREAKING CHANGE: callers must update".to_string();
        let records = vec![fix_with_trailer];

        let sections = group_by_section(records);

        assert_eq!(sections[1].0, Section::Changed);
        assert_eq!(sections[1].1.len(), 1);
        assert_eq!(sections[1].1[0].hash, "eee");
        // Not double-counted into Fixed.
        assert!(sections[2].1.is_empty());
    }

    #[test]
    fn group_by_section_hyphenated_breaking_trailer_takes_precedence_over_feat() {
        let mut feat_with_trailer = record("fff", "feat(cli): add a thing");
        feat_with_trailer.body = "BREAKING-CHANGE: callers must update".to_string();
        let records = vec![feat_with_trailer];

        let sections = group_by_section(records);

        assert_eq!(sections[1].0, Section::Changed);
        assert_eq!(sections[1].1.len(), 1);
        assert_eq!(sections[1].1[0].hash, "fff");
        // Not double-counted into Added.
        assert!(sections[0].1.is_empty());
    }

    #[test]
    fn group_by_section_breaking_marker_without_scope() {
        let records = vec![record("ggg", "feat!: replace the config format")];

        let sections = group_by_section(records);

        assert_eq!(sections[1].0, Section::Changed);
        assert_eq!(
            sections[1].1,
            vec![record("ggg", "feat!: replace the config format")]
        );
        assert!(sections[0].1.is_empty());
    }

    #[test]
    fn group_by_section_unknown_type_with_breaking_marker_still_changed() {
        let records = vec![record("hhh", "docs!: rewrite the manual")];

        let sections = group_by_section(records);

        assert_eq!(sections[1].0, Section::Changed);
        assert_eq!(
            sections[1].1,
            vec![record("hhh", "docs!: rewrite the manual")]
        );
        // Not swallowed into Other despite being an unrecognized type.
        assert!(sections[3].1.is_empty());
    }

    #[test]
    fn print_records_emits_headings_in_order_with_callouts_and_no_duplicates() {
        let records = vec![
            record("aaa", "feat(cli): add a thing"),
            record("bbb", "fix(cli): correct a thing"),
            record("ccc", "feat(cli)!: replace the config format"),
            record("ddd", "docs: update the readme"),
        ];
        let sections = group_by_section(records);
        let gate = NamingGate::new(&[]).unwrap();

        let mut out: Vec<u8> = Vec::new();
        print_records(&mut out, &sections, &gate).unwrap();
        let printed = String::from_utf8(out).unwrap();

        let added_pos = printed.find("### Added").unwrap();
        let changed_pos = printed.find("### Changed").unwrap();
        let fixed_pos = printed.find("### Fixed").unwrap();
        let other_pos = printed.find("### Other").unwrap();
        assert!(added_pos < changed_pos);
        assert!(changed_pos < fixed_pos);
        assert!(fixed_pos < other_pos);

        assert!(printed.contains("feat(cli): add a thing"));
        assert!(printed.contains("feat(cli)!: replace the config format [BREAKING]"));
        assert!(printed.contains("fix(cli): correct a thing"));
        assert!(printed.contains("docs: update the readme [REVIEW]"));

        // Every survivor appears exactly once across the whole draft.
        assert_eq!(
            printed.matches("feat(cli): add a thing").count(),
            1,
            "expected each survivor exactly once"
        );
    }

    // ── sentinel + naming-gate flagging ─────────────────────────────────────

    #[test]
    fn print_records_prefixes_every_item_with_the_draft_sentinel() {
        let records = vec![
            record("aaaaaaa111", "feat(cli): add a thing"),
            record("bbbbbbb222", "docs: update the readme"),
        ];
        let sections = group_by_section(records);
        let gate = NamingGate::new(&[]).unwrap();

        let mut out: Vec<u8> = Vec::new();
        print_records(&mut out, &sections, &gate).unwrap();
        let printed = String::from_utf8(out).unwrap();

        assert!(printed.contains("[[GAL-RELEASE-DRAFT:aaaaaaa]]"));
        assert!(printed.contains("[[GAL-RELEASE-DRAFT:bbbbbbb]]"));
    }

    #[test]
    fn print_records_flags_plan_task_id_subject_with_rewrite() {
        let plan_id_subject = format!("feat(cli): finish the release notes task ({}-05)", "T");
        let records = vec![record("ccc0000000", &plan_id_subject)];
        let sections = group_by_section(records);
        let gate = NamingGate::new(&[]).unwrap();

        let mut out: Vec<u8> = Vec::new();
        print_records(&mut out, &sections, &gate).unwrap();
        let printed = String::from_utf8(out).unwrap();

        assert!(printed.contains("[[GAL-RELEASE-DRAFT:ccc0000]]"));
        assert!(printed.contains("[REWRITE]"));
    }

    #[test]
    fn print_records_other_gets_review_without_task_id() {
        let records = vec![record("ddd0000000", "docs: update the readme")];
        let sections = group_by_section(records);
        let gate = NamingGate::new(&[]).unwrap();

        let mut out: Vec<u8> = Vec::new();
        print_records(&mut out, &sections, &gate).unwrap();
        let printed = String::from_utf8(out).unwrap();

        assert!(printed.contains("[REVIEW]"));
        assert!(!printed.contains("[REWRITE]"));
    }

    #[test]
    fn print_records_rewrite_wins_over_review_on_other_task_id_item() {
        let plan_id_subject = format!("docs: rewrite section ({}-06)", "T");
        let records = vec![record("eee0000000", &plan_id_subject)];
        let sections = group_by_section(records);
        let gate = NamingGate::new(&[]).unwrap();

        let mut out: Vec<u8> = Vec::new();
        print_records(&mut out, &sections, &gate).unwrap();
        let printed = String::from_utf8(out).unwrap();

        assert!(printed.contains("[REWRITE]"));
        assert!(!printed.contains("[REVIEW]"));
    }

    #[test]
    fn print_records_clean_added_item_has_only_sentinel_no_second_marker() {
        let records = vec![record("fff0000000", "feat(cli): add a thing")];
        let sections = group_by_section(records);
        let gate = NamingGate::new(&[]).unwrap();

        let mut out: Vec<u8> = Vec::new();
        print_records(&mut out, &sections, &gate).unwrap();
        let printed = String::from_utf8(out).unwrap();

        assert!(printed.contains("[[GAL-RELEASE-DRAFT:fff0000]]"));
        assert!(!printed.contains("[REWRITE]"));
        assert!(!printed.contains("[REVIEW]"));
        assert!(!printed.contains("[BREAKING]"));
    }

    // ── temp-repo tests against the real git binary ──────────────────────────

    fn git_available() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    fn init_repo() -> TempDir {
        let temp = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .current_dir(temp.path())
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test User"]);
        // Override any machine-global core.hooksPath so a commit-msg hook from
        // this repo (or elsewhere) never rewrites the throwaway test commits.
        run(&["config", "core.hooksPath", ".git/hooks"]);
        temp
    }

    fn commit(repo: &Path, file: &str, message: &str) -> String {
        std::fs::write(repo.join(file), message).unwrap();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .current_dir(repo)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["add", file]);
        run(&["commit", "-q", "-m", message]);
        let out = Command::new("git")
            .current_dir(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn tag(repo: &Path, name: &str) {
        let status = Command::new("git")
            .current_dir(repo)
            .args(["tag", name])
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn explicit_range_resolves_endpoints_and_reads_commits() {
        if !git_available() {
            return;
        }
        let temp = init_repo();
        let repo = temp.path();
        commit(repo, "a.txt", "feat: first");
        let second = commit(repo, "b.txt", "feat: second");

        let args = vec!["release-notes".to_string(), format!("HEAD~1..{second}")];
        let mut out: Vec<u8> = Vec::new();
        let code = run_release_notes(&args, repo, &SystemGit, &mut out);
        assert_eq!(code, ExitCode::Success);
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("feat: second"));
        assert!(!printed.contains("feat: first"));
    }

    #[test]
    fn default_range_selects_highest_stable_semver_tag() {
        if !git_available() {
            return;
        }
        let temp = init_repo();
        let repo = temp.path();
        commit(repo, "a.txt", "feat: v011");
        tag(repo, "v0.1.1");
        commit(repo, "b.txt", "feat: v012");
        tag(repo, "v0.1.2");
        commit(repo, "c.txt", "feat: prerelease");
        tag(repo, "v0.1.3-rc1");
        commit(repo, "d.txt", "feat: nonsemver");
        tag(repo, "latest");
        commit(repo, "e.txt", "feat: after v012");

        let args = vec!["release-notes".to_string()];
        let mut out: Vec<u8> = Vec::new();
        let code = run_release_notes(&args, repo, &SystemGit, &mut out);
        assert_eq!(code, ExitCode::Success);
        let printed = String::from_utf8(out).unwrap();
        // Range is v0.1.2..HEAD: only commits after v0.1.2 are present.
        assert!(printed.contains("feat: prerelease"));
        assert!(printed.contains("feat: nonsemver"));
        assert!(printed.contains("feat: after v012"));
        assert!(!printed.contains("feat: v011"));
        assert!(!printed.contains("feat: v012"));
    }

    #[test]
    fn no_stable_tag_fails_prescriptively() {
        if !git_available() {
            return;
        }
        let temp = init_repo();
        let repo = temp.path();
        commit(repo, "a.txt", "feat: only commit");

        let args = vec!["release-notes".to_string()];
        let mut out: Vec<u8> = Vec::new();
        let code = run_release_notes(&args, repo, &SystemGit, &mut out);
        assert_eq!(code, ExitCode::Error);
        assert!(out.is_empty());
    }

    #[test]
    fn merge_commit_is_excluded_but_its_task_commit_survives() {
        if !git_available() {
            return;
        }
        let temp = init_repo();
        let repo = temp.path();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .current_dir(repo)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };

        let base = commit(repo, "a.txt", "feat: base");
        run(&["checkout", "-q", "-b", "side"]);
        commit(repo, "b.txt", "feat: side task");
        run(&["checkout", "-q", "main"]);
        run(&["merge", "--no-ff", "-q", "-m", "merge: land side", "side"]);
        let head = Command::new("git")
            .current_dir(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        let head = String::from_utf8(head.stdout).unwrap().trim().to_string();

        let args = vec!["release-notes".to_string(), format!("{base}..{head}")];
        let mut out: Vec<u8> = Vec::new();
        let code = run_release_notes(&args, repo, &SystemGit, &mut out);
        assert_eq!(code, ExitCode::Success);
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("feat: side task"));
        assert!(!printed.contains("merge: land side"));
    }
}
