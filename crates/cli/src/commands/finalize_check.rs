//! `gal finalize-check <plan-or-prompt> --receipt <path>` — finalize-internal,
//! deterministic zero-trust precondition checks.
//!
//! This subcommand is the mechanization lever for `/gal finalize`'s zero-trust
//! contract: the deterministic checks run here (in the binary) and emit a machine
//! `Receipt`, so finalize trusts the receipt rather than "the model ran it".
//! It is **finalize-internal**, not a public `/gal` control-plane command.
//!
//! Skeleton scope: argument parsing, the `Receipt` model, the exit-code contract
//! (any failed check → nonzero), and a deterministic stub receipt with no checks
//! wired yet. The individual deterministic checks land in follow-up tasks.

use gal_engine::ExitCode;
use std::path::{Path, PathBuf};

/// Terminal state of a single check, mirroring the gal-pipeline terminal-state
/// vocabulary so finalize and the pipeline share one language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CheckState {
    /// Ran and passed.
    Pass,
    /// Ran and failed.
    Fail,
    /// Could not run (e.g. no authoritative command declared); never a silent pass.
    NotRun,
}

impl CheckState {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            CheckState::Pass => "pass",
            CheckState::Fail => "fail",
            CheckState::NotRun => "not-run",
        }
    }
}

/// One check's machine-captured outcome.
#[derive(Debug, Clone)]
pub(crate) struct CheckOutcome {
    /// Stable check id, e.g. `authoritative-command`, `commit-existence`.
    pub(crate) name: String,
    /// The command or operation performed (for audit), if any.
    pub(crate) command: Option<String>,
    pub(crate) state: CheckState,
    /// One-line summary or content hash that explains the outcome.
    pub(crate) summary: String,
}

/// The machine receipt finalize reads as its sole pass-basis.
#[derive(Debug, Clone, Default)]
pub(crate) struct Receipt {
    pub(crate) checks: Vec<CheckOutcome>,
}

impl Receipt {
    /// A receipt passes only when **every** check passed. An empty receipt
    /// (no checks wired yet) passes vacuously; `NotRun` never passes.
    pub(crate) fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.state == CheckState::Pass)
    }

    /// Render the receipt as deterministic markdown (no timestamps — those are
    /// stamped by the caller, keeping the binary output reproducible).
    pub(crate) fn render(&self) -> String {
        let mut out = String::from("# finalize-check receipt\n\n");
        out.push_str(&format!(
            "overall: {}\n\n",
            if self.passed() { "pass" } else { "fail" }
        ));
        out.push_str("| check | state | command | summary |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for c in &self.checks {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                c.name,
                c.state.as_str(),
                c.command.as_deref().unwrap_or("—"),
                c.summary,
            ));
        }
        out
    }

    /// Exit-code contract: any non-pass check → `Error`; all pass → `Success`.
    pub(crate) fn exit_code(&self) -> ExitCode {
        if self.passed() {
            ExitCode::Success
        } else {
            ExitCode::Error
        }
    }
}

/// Parsed `finalize-check` arguments.
struct Args {
    target: PathBuf,
    receipt: PathBuf,
}

/// Default receipt location: gitignored `.dev/pipeline/receipts/` (reuses the area
/// established by the closed `fix-pipeline-verify-role-and-receipt` plan).
fn default_receipt_path() -> PathBuf {
    Path::new(".dev")
        .join("pipeline")
        .join("receipts")
        .join("finalize-check.receipt.md")
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut target: Option<PathBuf> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--receipt" => {
                let v = it.next().ok_or("--receipt requires a path")?;
                receipt = Some(PathBuf::from(v));
            }
            s if s.starts_with("--") => return Err(format!("unknown option '{s}'")),
            s => {
                if target.is_none() {
                    target = Some(PathBuf::from(s));
                } else {
                    return Err(format!("unexpected extra argument '{s}'"));
                }
            }
        }
    }
    Ok(Args {
        target: target.ok_or("finalize-check requires a <plan-or-prompt> path")?,
        receipt: receipt.unwrap_or_else(default_receipt_path),
    })
}

/// Locate the authoritative-check command list: the `<!-- gal:authoritative-check -->`
/// marker followed by the next ```json fence. Deterministic structured parse — NOT
/// prose-parse of the Tech-Stack table. Returns `None` when marker/fence/`command`
/// is absent or the fence is not valid JSON.
fn locate_authoritative_commands(project_md: &str) -> Option<Vec<String>> {
    let after_marker = project_md.split_once("<!-- gal:authoritative-check -->")?.1;
    let fence_open = after_marker.find("```json")?;
    let after_open = &after_marker[fence_open + "```json".len()..];
    let fence_close = after_open.find("```")?;
    let json = after_open[..fence_close].trim();
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let arr = value.get("command")?.as_array()?;
    Some(
        arr.iter()
            .filter_map(|c| c.as_str().map(str::to_string))
            .collect(),
    )
}

/// Execute one command **verbatim** (whitespace-split argv, no shell interpolation).
/// Returns (passed, summary). A nonzero exit or spawn failure → not passed.
fn run_command_verbatim(command: &str) -> (bool, String) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let Some((program, rest)) = parts.split_first() else {
        return (false, "empty command".to_string());
    };
    match std::process::Command::new(program).args(rest).output() {
        Ok(out) => {
            let code = out.status.code().unwrap_or(-1);
            (out.status.success(), format!("exit {code}"))
        }
        Err(e) => (false, format!("spawn failed: {e}")),
    }
}

/// check(a): read `<repo_root>/.dev/project.md` authoritative commands, run each
/// verbatim, plus a naming-gate scan. Missing declaration → a single `NotRun`
/// outcome (never a silent pass).
fn check_authoritative(repo_root: &Path) -> Vec<CheckOutcome> {
    let project_md = repo_root.join(".dev").join("project.md");
    let commands = std::fs::read_to_string(&project_md)
        .ok()
        .and_then(|md| locate_authoritative_commands(&md));

    let mut outcomes = Vec::new();
    match commands {
        None => outcomes.push(CheckOutcome {
            name: "authoritative-command".to_string(),
            command: None,
            state: CheckState::NotRun,
            summary: "no <!-- gal:authoritative-check --> json fence in .dev/project.md"
                .to_string(),
        }),
        Some(cmds) if cmds.is_empty() => outcomes.push(CheckOutcome {
            name: "authoritative-command".to_string(),
            command: None,
            state: CheckState::NotRun,
            summary: "authoritative-check fence has an empty command array".to_string(),
        }),
        Some(cmds) => {
            for cmd in cmds {
                let (passed, summary) = run_command_verbatim(&cmd);
                outcomes.push(CheckOutcome {
                    name: "authoritative-command".to_string(),
                    command: Some(cmd),
                    state: if passed {
                        CheckState::Pass
                    } else {
                        CheckState::Fail
                    },
                    summary,
                });
            }
        }
    }

    // naming-gate scan via the library (no self-spawn).
    outcomes.push(check_naming_gate(repo_root));
    outcomes
}

/// naming-gate over the repo tree, run in-process via the gal_engine library.
fn check_naming_gate(repo_root: &Path) -> CheckOutcome {
    use crate::gal::naming_gate::{load_retired_terms, NamingGate};
    let retired = load_retired_terms(repo_root);
    match NamingGate::new(&retired) {
        Ok(gate) => {
            let hits = gate.scan_tree(repo_root);
            let total: usize = hits.iter().map(|f| f.violations.len()).sum();
            CheckOutcome {
                name: "naming-gate".to_string(),
                command: Some("gal naming-gate".to_string()),
                state: if total == 0 {
                    CheckState::Pass
                } else {
                    CheckState::Fail
                },
                summary: format!("{total} violation(s) across {} file(s)", hits.len()),
            }
        }
        Err(e) => CheckOutcome {
            name: "naming-gate".to_string(),
            command: Some("gal naming-gate".to_string()),
            state: CheckState::Fail,
            summary: format!("scanner build failed: {e}"),
        },
    }
}

/// A `[x]` task line's recorded commit short-hash, if any. Looks for the `*(hash)*`
/// commit-note convention on a completed task line.
pub(crate) fn commit_note_hash(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("- [x]") {
        return None;
    }
    let open = line.rfind("*(")?;
    let rest = &line[open + 2..];
    let close = rest.find(")*")?;
    let token = rest[..close].trim();
    // A commit hash is hex, 7..=40 chars. Reject prose like "(verify-only)".
    if (7..=40).contains(&token.len()) && token.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(token.to_string())
    } else {
        None
    }
}

/// check(b): every `[x]` task line with a recorded commit hash must resolve to a
/// real commit (`git cat-file -e <hash>^{commit}`).
fn check_commit_existence(plan_text: &str, repo_root: &Path) -> CheckOutcome {
    let hashes: Vec<String> = plan_text.lines().filter_map(commit_note_hash).collect();
    let mut missing = Vec::new();
    for h in &hashes {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .arg("cat-file")
            .arg("-e")
            .arg(format!("{h}^{{commit}}"))
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !ok {
            missing.push(h.clone());
        }
    }
    CheckOutcome {
        name: "commit-existence".to_string(),
        command: Some("git cat-file -e <hash>".to_string()),
        state: if missing.is_empty() {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        summary: if missing.is_empty() {
            format!("{} recorded commit(s) all exist", hashes.len())
        } else {
            format!("missing commit(s): {}", missing.join(", "))
        },
    }
}

/// Set of `T-NN` ids that are checked (`[x]`) in a plan text.
pub(crate) fn checked_task_ids(plan_text: &str) -> std::collections::BTreeSet<String> {
    let mut set = std::collections::BTreeSet::new();
    for line in plan_text.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("- [x] ") {
            // Strip optional Markdown bold markers (**) before matching the T-NN id.
            // Source plans may bold the task id (e.g. `**T-NN — ...`).
            if let Some(raw) = rest.split_whitespace().next() {
                let id = raw.trim_matches('*');
                if id.starts_with("T-") && id[2..].chars().all(|c| c.is_ascii_digit()) {
                    set.insert(id.to_string());
                }
            }
        }
    }
    set
}

/// check(c): source plan ↔ execution prompt must agree on which `T-NN` are `[x]`.
/// (`.dev/state.md` carries a continuity row, not per-task checkboxes, so the
/// machine-checkable checkbox surfaces are the source plan and the prompt.)
pub(crate) fn check_three_surface(
    prompt_path: &Path,
    prompt_text: &str,
    repo_root: &Path,
) -> CheckOutcome {
    let name = "three-surface-checkbox".to_string();
    // Derive source plan: .dev/plans/<slug>.prompt.md → .dev/plans/<slug>.md
    let slug = prompt_path
        .file_name()
        .and_then(|f| f.to_str())
        .and_then(|f| f.strip_suffix(".prompt.md"));
    let Some(slug) = slug else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::NotRun,
            summary: "target is not a *.prompt.md execution prompt".to_string(),
        };
    };
    let source_plan = repo_root
        .join(".dev")
        .join("plans")
        .join(format!("{slug}.md"));
    let Ok(source_text) = std::fs::read_to_string(&source_plan) else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::NotRun,
            summary: format!("paired source plan not found: {}", source_plan.display()),
        };
    };
    let prompt_set = checked_task_ids(prompt_text);
    let source_set = checked_task_ids(&source_text);
    if prompt_set == source_set {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: format!(
                "source plan and prompt agree on {} checked task(s)",
                prompt_set.len()
            ),
        }
    } else {
        let only_prompt: Vec<_> = prompt_set.difference(&source_set).cloned().collect();
        let only_source: Vec<_> = source_set.difference(&prompt_set).cloned().collect();
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!(
                "checkbox disagreement — prompt-only: [{}], source-only: [{}]",
                only_prompt.join(", "),
                only_source.join(", ")
            ),
        }
    }
}

/// Extract backtick-quoted snake_case identifiers cited in the `## Test Results`
/// section that look like Rust test fn names (heuristic: snake_case, len ≥ 3,
/// no `::`, no spaces).
fn cited_test_names(plan_text: &str) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    let section = match plan_text.split_once("## Test Results") {
        Some((_, after)) => match after.split_once("\n## ") {
            Some((sec, _)) => sec,
            None => after,
        },
        None => return names,
    };
    let mut chars = section.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '`' {
            // read until next backtick
            let rest = &section[i + 1..];
            if let Some(end) = rest.find('`') {
                let tok = &rest[..end];
                let looks_test = tok.len() >= 3
                    && !tok.contains("::")
                    && !tok.contains(char::is_whitespace)
                    && tok.starts_with(|c: char| c.is_ascii_lowercase())
                    && tok
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                    && tok.contains('_');
                if looks_test {
                    names.insert(tok.to_string());
                }
                // advance past the closing backtick
                for _ in 0..end + 1 {
                    chars.next();
                }
            }
        }
    }
    names
}

/// Classification of a plan's `## Test Plan` table by declared probe `Type`.
///
/// Gates check(e): a plan whose Test Plan declares only `manual` probes has no
/// fn-citation contract, so harvesting `## Test Results` for fn names would only
/// produce false positives (front-matter field names, YAML keys) — see
/// `check_cited_test_names`. Anything not cleanly all-`manual` is fail-closed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TestPlanKind {
    /// No `## Test Plan` section, no data rows, or a `Type`-less / unparseable table.
    NoTable,
    /// ≥1 data row and every data row's `Type` cell is exactly `manual`.
    AllManual(usize),
    /// At least one data row's `Type` is non-`manual` (incl. empty / unknown).
    HasNonManual,
}

/// Split a markdown table row into trimmed cell strings, dropping the empty
/// leading/trailing cells produced by the outer `|` delimiters.
fn split_md_row(row: &str) -> Vec<String> {
    let row = row.trim();
    let row = row.strip_prefix('|').unwrap_or(row);
    let row = row.strip_suffix('|').unwrap_or(row);
    row.split('|').map(|c| c.trim().to_string()).collect()
}

/// Scan the target plan's `## Test Plan` markdown table and classify it by the
/// declared `Type` column. Locates `Type` by header cell name (never a fixed
/// index), skips the markdown separator row (cells of only `-`/`:`), and treats
/// an empty/unknown `Type` as non-`manual` (fail-closed).
fn scan_test_plan_kind(plan_text: &str) -> TestPlanKind {
    let lines: Vec<&str> = plan_text.lines().collect();
    let Some(idx) = lines.iter().position(|l| l.trim() == "## Test Plan") else {
        return TestPlanKind::NoTable;
    };
    let start = idx + 1;
    let end = lines[start..]
        .iter()
        .position(|l| l.starts_with("## "))
        .map(|off| start + off)
        .unwrap_or(lines.len());
    let mut rows = lines[start..end]
        .iter()
        .map(|l| l.trim())
        .filter(|l| l.starts_with('|'));

    // Header row → find the `Type` column by name.
    let Some(header) = rows.next() else {
        return TestPlanKind::NoTable;
    };
    let Some(type_col) = split_md_row(header)
        .iter()
        .position(|c| c.eq_ignore_ascii_case("type"))
    else {
        return TestPlanKind::NoTable;
    };

    let mut count = 0usize;
    for row in rows {
        let cells = split_md_row(row);
        // Skip the markdown separator row (every cell only `-`/`:`).
        if cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
        {
            continue;
        }
        // Skip a fully-empty row.
        if cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        let ty = cells.get(type_col).map(String::as_str).unwrap_or("");
        if ty.eq_ignore_ascii_case("manual") {
            count += 1;
        } else {
            return TestPlanKind::HasNonManual;
        }
    }

    if count > 0 {
        TestPlanKind::AllManual(count)
    } else {
        TestPlanKind::NoTable
    }
}

/// check(e): each cited test-name identifier must exist as `fn <name>` somewhere
/// under `crates/` (catches hallucinated test names in `## Test Results`).
///
/// Plan-type-aware: when the target plan's `## Test Plan` declares only `manual`
/// probes there is no fn-citation contract, so the check vacuously passes with a
/// `skipped:` summary instead of harvesting prose. Every other case (missing /
/// mixed / unknown-type / `Type`-less table) is fail-closed — harvest runs.
fn check_cited_test_names(plan_text: &str, repo_root: &Path) -> CheckOutcome {
    if let TestPlanKind::AllManual(n) = scan_test_plan_kind(plan_text) {
        return CheckOutcome {
            name: "cited-test-existence".to_string(),
            command: Some("test-plan type scan".to_string()),
            state: CheckState::Pass,
            summary: format!(
                "skipped: all {n} test-plan probes are manual — no fn-citation contract"
            ),
        };
    }
    let cited = cited_test_names(plan_text);
    let crates_dir = repo_root.join("crates");
    let mut missing = Vec::new();
    for name in &cited {
        let needle = format!("fn {name}");
        if !grep_exists(&crates_dir, &needle) {
            missing.push(name.clone());
        }
    }
    CheckOutcome {
        name: "cited-test-existence".to_string(),
        command: Some("grep 'fn <name>' crates/".to_string()),
        state: if missing.is_empty() {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        summary: if cited.is_empty() {
            "no cited test-name identifiers found".to_string()
        } else if missing.is_empty() {
            format!("{} cited test name(s) all exist", cited.len())
        } else {
            format!("cited but missing: {}", missing.join(", "))
        },
    }
}

/// Recursively check whether any `*.rs` file under `dir` contains `needle`.
fn grep_exists(dir: &Path, needle: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            if grep_exists(&path, needle) {
                return true;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if content.contains(needle) {
                    return true;
                }
            }
        }
    }
    false
}

/// Generated adapter carriers — the doc-sync output whose idempotency we assert.
const ADAPTER_PATHS: &[&str] = &[
    "CLAUDE.md",
    "AGENTS.md",
    "GEMINI.md",
    ".github/copilot-instructions.md",
    ".agents/rules/gal.md",
];

/// Snapshot (relpath, content-hash) for each existing adapter under `repo_root`.
fn snapshot_adapters(repo_root: &Path) -> Vec<(String, u64)> {
    use std::hash::{Hash, Hasher};
    let mut snap = Vec::new();
    for rel in ADAPTER_PATHS {
        let p = repo_root.join(rel);
        if let Ok(bytes) = std::fs::read(&p) {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            bytes.hash(&mut h);
            snap.push(((*rel).to_string(), h.finish()));
        }
    }
    snap
}

/// check(d): doc-sync idempotency. Run sync, snapshot adapters; run sync again,
/// snapshot; the two snapshots must be byte-identical (sync is a fixpoint). The
/// sync action is injected so the unit test is deterministic without a real sync.
fn check_sync_idempotency(repo_root: &Path, run_sync: impl Fn()) -> CheckOutcome {
    run_sync();
    let first = snapshot_adapters(repo_root);
    run_sync();
    let second = snapshot_adapters(repo_root);
    let identical = first == second;
    CheckOutcome {
        name: "sync-idempotency".to_string(),
        command: Some("adapter render ×2 [in-process]".to_string()),
        state: if identical {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        summary: if first.is_empty() {
            "no adapter files present to compare".to_string()
        } else if identical {
            format!("{} adapter(s) byte-identical across two syncs", first.len())
        } else {
            "adapter content changed between consecutive syncs (non-idempotent)".to_string()
        },
    }
}

/// Production sync action: run the repo-adapter render **in-process** (gal tree).
///
/// S2: replaced the `gal sync` subprocess spawn with a direct in-process call to
/// gal's own render module. finalize-check no longer shells out and never touches
/// ccync — repo-adapter render is pure workflow (gal tree).
fn run_gal_sync_in_process(repo_root: &Path) {
    if let Ok(opts) = crate::gal::render::sync_options_from(repo_root.to_path_buf(), false) {
        let _ = crate::gal::render::run_sync(&opts);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// check(f) — finalize-mode detect
// ─────────────────────────────────────────────────────────────────────────────

/// Run `git -C <repo_root> <args>` and return trimmed stdout on success.
fn git_one_line(repo_root: &Path, args: &[&str]) -> String {
    std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// check(f): mode-detect — purely informational, state is always `Pass`.
/// Injectable for tests: supply the branch name directly.
pub(crate) fn check_finalize_mode_from(branch: &str) -> CheckOutcome {
    let summary = if branch == "main" || branch == "master" {
        "already-on-main".to_string()
    } else if branch.is_empty() {
        "already-on-main (detached HEAD — branch name unavailable)".to_string()
    } else {
        "worktree".to_string()
    };
    CheckOutcome {
        name: "finalize-mode".to_string(),
        command: Some("git branch --show-current".to_string()),
        state: CheckState::Pass,
        summary,
    }
}

fn check_finalize_mode(repo_root: &Path) -> CheckOutcome {
    let branch = git_one_line(repo_root, &["branch", "--show-current"]);
    check_finalize_mode_from(&branch)
}

// ─────────────────────────────────────────────────────────────────────────────
// check(g) — executor-log scan
// ─────────────────────────────────────────────────────────────────────────────

/// Parse `terminal_state:` value from a `.dev/executor-logs/*.log` header.
fn parse_terminal_state(log_content: &str) -> Option<String> {
    let header = match log_content.split_once("---STDOUT---") {
        Some((h, _)) => h,
        None => log_content,
    };
    for line in header.lines() {
        if let Some(val) = line.strip_prefix("terminal_state:") {
            return Some(val.trim().to_string());
        }
    }
    None
}

/// check(g): executor-log scan — injectable for tests.
/// `logs` is a slice of `(filename, content)` pairs from `.dev/executor-logs/`.
pub(crate) fn check_executor_logs_from(logs: &[(&str, &str)]) -> CheckOutcome {
    let name = "executor-log-scan".to_string();
    if logs.is_empty() {
        return CheckOutcome {
            name,
            command: Some(".dev/executor-logs/".to_string()),
            state: CheckState::NotRun,
            summary: "no executor log files found".to_string(),
        };
    }
    let mut non_completed: Vec<String> = Vec::new();
    for (filename, content) in logs {
        if let Some(state) = parse_terminal_state(content) {
            if state != "completed" {
                non_completed.push(format!("{filename}:{state}"));
            }
        }
    }
    if non_completed.is_empty() {
        CheckOutcome {
            name,
            command: Some(".dev/executor-logs/".to_string()),
            state: CheckState::Pass,
            summary: format!("{} executor log(s) all completed", logs.len()),
        }
    } else {
        CheckOutcome {
            name,
            command: Some(".dev/executor-logs/".to_string()),
            state: CheckState::Fail,
            summary: format!(
                "non-completed terminal state(s): {}",
                non_completed.join(", ")
            ),
        }
    }
}

fn check_executor_logs(repo_root: &Path) -> CheckOutcome {
    let logs_dir = repo_root.join(".dev").join("executor-logs");
    let mut entries: Vec<(String, String)> = Vec::new();
    if let Ok(dir) = std::fs::read_dir(&logs_dir) {
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("log") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let fname = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?")
                        .to_string();
                    entries.push((fname, content));
                }
            }
        }
    }
    let refs: Vec<(&str, &str)> = entries
        .iter()
        .map(|(n, c)| (n.as_str(), c.as_str()))
        .collect();
    check_executor_logs_from(&refs)
}

// ─────────────────────────────────────────────────────────────────────────────
// check(h) — durable-layer commit-hash gate
// ─────────────────────────────────────────────────────────────────────────────

/// check(h): durable-layer commit-hash gate.
/// Finds the last `*(hex)*` note on any `- [x]` line via `commit_note_hash`,
/// then asserts existence only — receipt summary contains no authorization language.
fn check_durable_layer_commit(plan_text: &str, repo_root: &Path) -> CheckOutcome {
    let name = "durable-layer-commit".to_string();
    let last_hash: Option<String> = plan_text.lines().filter_map(commit_note_hash).next_back();
    let Some(hash) = last_hash else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: "not yet recorded".to_string(),
        };
    };
    let ok = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{hash}^{{commit}}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    CheckOutcome {
        name,
        command: Some(format!("git cat-file -e {hash}")),
        state: if ok {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        summary: if ok {
            format!("exists: {hash}")
        } else {
            format!("missing: {hash}")
        },
    }
}

/// Run `gal finalize-check`. Parse args, run the wired deterministic checks against
/// the repo root, write the receipt, and return the exit code from the receipt's
/// pass-state.
pub(crate) fn cmd_finalize_check(args: &[String]) -> ExitCode {
    // args[0] is the "finalize-check" command token (dispatch passes full argv).
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let parsed = match parse_args(&rest) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("gal finalize-check: {e}");
            return ExitCode::Usage;
        }
    };

    if !parsed.target.exists() {
        eprintln!(
            "gal finalize-check: target not found: {}",
            parsed.target.display()
        );
        return ExitCode::Usage;
    }

    // Checks run against the repo root (where .dev/project.md lives).
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let target_text = std::fs::read_to_string(&parsed.target).unwrap_or_default();
    let mut checks = check_authoritative(&repo_root);
    checks.push(check_commit_existence(&target_text, &repo_root));
    checks.push(check_three_surface(
        &parsed.target,
        &target_text,
        &repo_root,
    ));
    checks.push(check_cited_test_names(&target_text, &repo_root));
    {
        let root = repo_root.clone();
        checks.push(check_sync_idempotency(&repo_root, || {
            run_gal_sync_in_process(&root)
        }));
    }
    checks.push(check_finalize_mode(&repo_root));
    checks.push(check_executor_logs(&repo_root));
    checks.push(check_durable_layer_commit(&target_text, &repo_root));
    let receipt = Receipt { checks };

    if let Some(parent) = parsed.receipt.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!(
                "gal finalize-check: cannot create receipt dir {}: {e}",
                parent.display()
            );
            return ExitCode::Error;
        }
    }
    if let Err(e) = std::fs::write(&parsed.receipt, receipt.render()) {
        eprintln!(
            "gal finalize-check: cannot write receipt {}: {e}",
            parsed.receipt.display()
        );
        return ExitCode::Error;
    }

    println!(
        "gal finalize-check: {} ({} check(s)) → {}",
        if receipt.passed() { "pass" } else { "fail" },
        receipt.checks.len(),
        parsed.receipt.display()
    );
    receipt.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(name: &str, state: CheckState) -> CheckOutcome {
        CheckOutcome {
            name: name.to_string(),
            command: None,
            state,
            summary: String::new(),
        }
    }

    #[test]
    fn empty_receipt_passes_vacuously() {
        let r = Receipt::default();
        assert!(r.passed());
        assert_eq!(r.exit_code(), ExitCode::Success);
    }

    #[test]
    fn all_pass_is_success() {
        let r = Receipt {
            checks: vec![
                outcome("a", CheckState::Pass),
                outcome("b", CheckState::Pass),
            ],
        };
        assert!(r.passed());
        assert_eq!(r.exit_code(), ExitCode::Success);
    }

    #[test]
    fn any_fail_is_nonzero() {
        let r = Receipt {
            checks: vec![
                outcome("a", CheckState::Pass),
                outcome("b", CheckState::Fail),
            ],
        };
        assert!(!r.passed());
        assert_eq!(r.exit_code(), ExitCode::Error);
    }

    #[test]
    fn not_run_never_passes() {
        let r = Receipt {
            checks: vec![outcome("a", CheckState::NotRun)],
        };
        assert!(!r.passed());
        assert_eq!(r.exit_code(), ExitCode::Error);
    }

    #[test]
    fn render_lists_each_check_and_overall() {
        let r = Receipt {
            checks: vec![outcome("commit-existence", CheckState::Pass)],
        };
        let md = r.render();
        assert!(md.contains("overall: pass"));
        assert!(md.contains("commit-existence"));
        assert!(md.contains("| check | state | command | summary |"));
    }

    #[test]
    fn parse_requires_target() {
        assert!(parse_args(&[]).is_err());
    }

    #[test]
    fn parse_target_and_receipt() {
        let a = parse_args(&[
            ".dev/plans/x.md".to_string(),
            "--receipt".to_string(),
            "out/r.md".to_string(),
        ])
        .unwrap();
        assert_eq!(a.target, PathBuf::from(".dev/plans/x.md"));
        assert_eq!(a.receipt, PathBuf::from("out/r.md"));
    }

    #[test]
    fn parse_defaults_receipt_under_dev_pipeline_receipts() {
        let a = parse_args(&[".dev/plans/x.md".to_string()]).unwrap();
        assert!(a.receipt.ends_with("finalize-check.receipt.md"));
        assert!(a.receipt.to_string_lossy().contains("receipts"));
    }

    #[test]
    fn parse_rejects_unknown_option() {
        assert!(parse_args(&["x".to_string(), "--frob".to_string()]).is_err());
    }

    // --- check(a) ---

    #[test]
    fn locate_finds_commands_in_fence() {
        let md = "intro\n\n<!-- gal:authoritative-check -->\n```json\n{ \"command\": [\"cargo test --workspace\", \"cargo clippy --workspace\"] }\n```\n\nmore";
        let cmds = locate_authoritative_commands(md).unwrap();
        assert_eq!(
            cmds,
            vec!["cargo test --workspace", "cargo clippy --workspace"]
        );
    }

    #[test]
    fn locate_returns_none_without_marker() {
        let md = "no marker here\n```json\n{ \"command\": [\"x\"] }\n```\n";
        assert!(locate_authoritative_commands(md).is_none());
    }

    #[test]
    fn locate_returns_none_on_invalid_json() {
        let md = "<!-- gal:authoritative-check -->\n```json\n{ not json }\n```\n";
        assert!(locate_authoritative_commands(md).is_none());
    }

    #[test]
    fn run_command_verbatim_passes_on_exit_zero() {
        // `cargo --version` exists in the test env, exits 0, and is NOT a nested
        // test run (no `cargo test` recursion).
        let (passed, _summary) = run_command_verbatim("cargo --version");
        assert!(passed);
    }

    #[test]
    fn run_command_verbatim_fails_on_missing_program() {
        let (passed, summary) = run_command_verbatim("definitely-not-a-real-command-xyz123");
        assert!(!passed);
        assert!(summary.contains("spawn failed"));
    }

    #[test]
    fn check_authoritative_not_run_when_fence_absent() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".dev")).unwrap();
        std::fs::write(
            tmp.path().join(".dev").join("project.md"),
            "# no fence here\n",
        )
        .unwrap();
        let outcomes = check_authoritative(tmp.path());
        let auth = outcomes
            .iter()
            .find(|o| o.name == "authoritative-command")
            .unwrap();
        assert_eq!(auth.state, CheckState::NotRun);
    }

    // Compose plan-task IDs at runtime — never hardcode `T-NN` literals in source
    // (naming-gate provenance rule: plan-task IDs belong only in .dev).
    fn tid(n: u32) -> String {
        format!("T-{n:02}")
    }

    fn tpid(n: u32) -> String {
        format!("TP-{n:02}")
    }

    // --- check(b): commit existence ---

    #[test]
    fn commit_note_hash_extracts_only_hex() {
        assert_eq!(
            commit_note_hash(&format!("- [x] {} — do thing. *(ff62d9c)*", tid(1))),
            Some("ff62d9c".to_string())
        );
        assert_eq!(
            commit_note_hash(&format!("- [x] {} — verify. *(verify-only)*", tid(5))),
            None
        );
        assert_eq!(
            commit_note_hash(&format!("- [ ] {} — pending *(ff62d9c)*", tid(2))),
            None
        );
    }

    #[test]
    fn check_commit_existence_flags_nonexistent_hash() {
        // repo_root "." is the GAL repo; a bogus hash cannot resolve.
        let plan = format!("- [x] {} — x. *(0000000deadbeef0000000)*\n", tid(1));
        let out = check_commit_existence(&plan, Path::new("."));
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("missing"));
    }

    // --- check(c): three-surface ---

    #[test]
    fn checked_task_ids_collects_x_only() {
        let txt = format!(
            "- [x] {} — a\n- [ ] {} — b\n- [x] {} — c *(abc)*\n",
            tid(1),
            tid(2),
            tid(3)
        );
        let ids = checked_task_ids(&txt);
        assert!(ids.contains(&tid(1)) && ids.contains(&tid(3)) && !ids.contains(&tid(2)));
    }

    #[test]
    fn three_surface_fails_on_disagreement() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".dev").join("plans")).unwrap();
        std::fs::write(
            tmp.path().join(".dev").join("plans").join("demo.md"),
            format!("- [x] {} — a\n- [ ] {} — b\n", tid(1), tid(2)),
        )
        .unwrap();
        let prompt_path = tmp.path().join("demo.prompt.md");
        // second id checked in prompt but not source → disagreement
        let prompt_text = format!("- [x] {} — a\n- [x] {} — b\n", tid(1), tid(2));
        let out = check_three_surface(&prompt_path, &prompt_text, tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains(&tid(2)));
    }

    // --- check(e): cited test-name existence ---

    #[test]
    fn cited_test_names_extracts_snake_case_in_test_results() {
        let txt = "## Test Results\n\nevidence: `parses_every_known_command` PASS; see `Receipt`.\n\n## Tasks\n";
        let names = cited_test_names(txt);
        assert!(names.contains("parses_every_known_command"));
        assert!(!names.contains("Receipt")); // not snake_case
    }

    #[test]
    fn check_cited_test_names_flags_hallucinated() {
        // A snake_case identifier that does not exist as `fn <name>` in crates/.
        let txt = "## Test Results\n\n`this_test_does_not_exist_anywhere_xyz` PASS\n";
        let out = check_cited_test_names(txt, Path::new("."));
        assert_eq!(out.state, CheckState::Fail);
        assert!(out
            .summary
            .contains("this_test_does_not_exist_anywhere_xyz"));
    }

    // --- check(e): Test Plan type scan (manual-only skip gate) ---

    // ids composed at runtime (tid/tpid) so no plan-ID literal lands in source.
    fn test_plan_row(tp: u32, ty: &str, t: u32) -> String {
        format!("| {} | {ty} | desc | {} |\n", tpid(tp), tid(t))
    }
    const TP_HEADER: &str = "## Test Plan\n\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n";

    #[test]
    fn test_plan_kind_all_manual() {
        let txt = format!(
            "{TP_HEADER}{}{}\n## Tasks\n",
            test_plan_row(1, "manual", 1),
            test_plan_row(2, "Manual", 1)
        );
        assert_eq!(scan_test_plan_kind(&txt), TestPlanKind::AllManual(2));
    }

    #[test]
    fn test_plan_kind_mixed_is_non_manual() {
        let txt = format!(
            "{TP_HEADER}{}{}\n## Tasks\n",
            test_plan_row(1, "manual", 1),
            test_plan_row(2, "integration", 1)
        );
        assert_eq!(scan_test_plan_kind(&txt), TestPlanKind::HasNonManual);
    }

    #[test]
    fn test_plan_kind_unknown_type_is_non_manual() {
        let e2e = format!("{TP_HEADER}{}", test_plan_row(1, "e2e", 1));
        assert_eq!(scan_test_plan_kind(&e2e), TestPlanKind::HasNonManual);
        let empty = format!("{TP_HEADER}{}", test_plan_row(1, "", 1));
        assert_eq!(scan_test_plan_kind(&empty), TestPlanKind::HasNonManual);
    }

    #[test]
    fn test_plan_kind_missing_section_is_no_table() {
        let txt = format!("## Tasks\n\n- [ ] {} — do\n", tid(1));
        assert_eq!(scan_test_plan_kind(&txt), TestPlanKind::NoTable);
    }

    #[test]
    fn test_plan_kind_header_without_type_column_is_no_table() {
        let txt = format!(
            "## Test Plan\n\n| ID | Kind | Description |\n| --- | --- | --- |\n| {} | manual | a |\n",
            tpid(1)
        );
        assert_eq!(scan_test_plan_kind(&txt), TestPlanKind::NoTable);
    }

    #[test]
    fn cited_test_names_skips_manual_only_plan() {
        // Docs-only plan: all-manual TP table + Test Results citing front-matter
        // field names that match the fn-name shape but have no `fn` definition.
        let txt = format!(
            "{TP_HEADER}{}\n## Test Results\n\nverified `source_commit` and `translated_at` present.\n",
            test_plan_row(1, "manual", 1)
        );
        let out = check_cited_test_names(&txt, Path::new("."));
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.starts_with("skipped:"));
        assert!(!out.summary.contains("missing"));
    }

    #[test]
    fn cited_test_names_runs_for_unit_plan_hallucinated_fails() {
        // A `unit` TP is present → not skipped → harvest runs → hallucinated fails.
        let txt = format!(
            "{TP_HEADER}{}\n## Test Results\n\n`this_fn_absolutely_does_not_exist_zzz` PASS\n",
            test_plan_row(1, "unit", 1)
        );
        let out = check_cited_test_names(&txt, Path::new("."));
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("this_fn_absolutely_does_not_exist_zzz"));
    }

    #[test]
    fn cited_test_names_runs_for_unit_plan_real_fn_passes() {
        // `unit` TP present → harvest runs; a real fn under crates/ passes.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("crates")).unwrap();
        std::fs::write(tmp.path().join("crates").join("x.rs"), "fn my_probe_fn() {}\n").unwrap();
        let txt = format!(
            "{TP_HEADER}{}\n## Test Results\n\n`my_probe_fn` PASS\n",
            test_plan_row(1, "unit", 1)
        );
        let out = check_cited_test_names(&txt, tmp.path());
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("all exist"));
    }

    // --- check(d): sync idempotency ---

    #[test]
    fn sync_idempotency_passes_when_adapters_stable() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), "stable content\n").unwrap();
        // no-op sync: adapters never change → byte-identical
        let out = check_sync_idempotency(tmp.path(), || {});
        assert_eq!(out.state, CheckState::Pass);
    }

    #[test]
    fn sync_idempotency_fails_when_adapter_changes_between_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = tmp.path().join("CLAUDE.md");
        std::fs::write(&claude, "v0\n").unwrap();
        let counter = std::cell::Cell::new(0u32);
        // non-idempotent sync: each call rewrites adapter with a new value
        let out = check_sync_idempotency(tmp.path(), || {
            let n = counter.get();
            counter.set(n + 1);
            std::fs::write(&claude, format!("v{n}\n")).unwrap();
        });
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("non-idempotent"));
    }

    #[test]
    fn sync_idempotency_no_adapters_is_benign() {
        let tmp = tempfile::tempdir().unwrap();
        let out = check_sync_idempotency(tmp.path(), || {});
        // empty snapshot both times → equal → Pass with benign summary
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("no adapter files"));
    }

    // --- check(f): finalize-mode ---

    #[test]
    fn mode_detect_feature_branch_reports_worktree() {
        let out = check_finalize_mode_from("feature-branch");
        assert_eq!(out.state, CheckState::Pass);
        assert_eq!(out.summary, "worktree");
    }

    #[test]
    fn mode_detect_main_reports_already_on_main() {
        let out = check_finalize_mode_from("main");
        assert_eq!(out.state, CheckState::Pass);
        assert_eq!(out.summary, "already-on-main");
    }

    #[test]
    fn mode_detect_state_always_pass() {
        for branch in &["main", "master", "feat-foo", "fix/bar", ""] {
            let out = check_finalize_mode_from(branch);
            assert_eq!(
                out.state,
                CheckState::Pass,
                "branch {:?} must always be Pass",
                branch
            );
        }
    }

    // --- check(g): executor-log scan ---

    #[test]
    fn executor_log_scan_empty_dir_is_not_run() {
        let out = check_executor_logs_from(&[]);
        assert_eq!(out.state, CheckState::NotRun);
    }

    #[test]
    fn executor_log_scan_all_completed_is_pass() {
        let log = "terminal_state: completed\n---STDOUT---\nsome output\n";
        let out = check_executor_logs_from(&[("t01-impl.log", log), ("t01-test.log", log)]);
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("2"));
    }

    #[test]
    fn executor_log_scan_non_completed_is_fail() {
        let good = "terminal_state: completed\n---STDOUT---\n";
        let bad = "terminal_state: timeout-midrun\n---STDOUT---\n";
        let out = check_executor_logs_from(&[("ok.log", good), ("bad.log", bad)]);
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("timeout-midrun"));
        assert!(out.summary.contains("bad.log"));
    }

    #[test]
    fn executor_log_scan_no_receipt_is_fail() {
        let bad = "terminal_state: no-receipt\n---STDOUT---\n";
        let out = check_executor_logs_from(&[("t02.log", bad)]);
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("no-receipt"));
    }

    // --- check(h): durable-layer commit ---

    #[test]
    fn durable_layer_commit_no_hash_is_pass() {
        let plan = format!("## Tasks\n\n- [ ] {} — pending\n", tid(1));
        let out = check_durable_layer_commit(&plan, Path::new("."));
        assert_eq!(out.state, CheckState::Pass);
        assert_eq!(out.summary, "not yet recorded");
    }

    #[test]
    fn durable_layer_commit_bogus_hash_is_fail() {
        let plan = format!("- [x] {} — done. *(0000000deadbeef0000000)*\n", tid(1));
        let out = check_durable_layer_commit(&plan, Path::new("."));
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.starts_with("missing:"));
    }

    #[test]
    fn durable_layer_commit_summary_has_no_delete_language() {
        let no_hash = format!("## Tasks\n\n- [ ] {}\n", tid(1));
        let bogus = format!("- [x] {} *(0000000aaabbbccddee0)*\n", tid(1));
        for plan in &[no_hash.as_str(), bogus.as_str()] {
            let out = check_durable_layer_commit(plan, Path::new("."));
            assert!(
                !out.summary.contains("delet"),
                "summary must not mention delete"
            );
            assert!(
                !out.summary.contains("authoriz"),
                "summary must not mention authorize"
            );
        }
    }
}
