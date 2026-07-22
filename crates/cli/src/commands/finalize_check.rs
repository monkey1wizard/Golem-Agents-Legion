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

use super::converge_check::{
    check_cursor_cleared, check_task_commit, check_three_surface_and_task, classify_phase,
    scan_plan_log_dir, PhaseOutcome, PhaseVerdict, REQUIRED_PHASES,
};
use super::dispatch::resolve_log_dir_override;
use super::finalize_hygiene::{
    check_contract_roster_parity, check_doc_link_resolution, check_project_source_doc_existence,
    check_state_bound, is_gal_source_repo, HygieneMode,
};
use gal_engine::ExitCode;
use std::collections::BTreeMap;
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

/// The machine receipt finalize reads as its sole pass-basis. Shared by every
/// deterministic check subcommand (`boundary-check`, `converge-check`,
/// `planning-check`, `prompt-check`, `refining-check`, `pipeline-preflight`,
/// `finalize-check`) — this struct stays check-family-agnostic. Finalize's
/// full/hygiene-only mode line is rendered separately by
/// `render_finalize_receipt`, not baked in here.
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
    mode: HygieneMode,
}

/// Default receipt location: gitignored `.dev/pipeline/receipts/` (reuses the area
/// established by the closed `fix-pipeline-verify-role-and-receipt` plan). Full and
/// hygiene-only modes use distinct default filenames so neither run overwrites the
/// other's evidence.
fn default_receipt_path(mode: HygieneMode) -> PathBuf {
    let filename = match mode {
        HygieneMode::Full => "finalize-check.receipt.md",
        HygieneMode::HygieneOnly => "finalize-check.hygiene.receipt.md",
    };
    Path::new(".dev")
        .join("pipeline")
        .join("receipts")
        .join(filename)
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut target: Option<PathBuf> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut mode = HygieneMode::Full;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--receipt" => {
                let v = it.next().ok_or("--receipt requires a path")?;
                receipt = Some(PathBuf::from(v));
            }
            "--hygiene-only" => {
                mode = HygieneMode::HygieneOnly;
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
        receipt: receipt.unwrap_or_else(|| default_receipt_path(mode)),
        mode,
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
        Ok(gate) => match gate.scan_tree(repo_root) {
            Ok(hits) => {
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
                summary: format!("scan failed: {e}"),
            },
        },
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

/// Extract backtick-quoted snake_case identifiers cited in the real `## Test
/// Results` section that look like Rust test fn names (heuristic: snake_case,
/// len ≥ 3, no `::`, no spaces).
///
/// Line-anchored, mirroring `scan_test_plan_kind`: the window opens at the
/// first line whose trimmed content is exactly `## Test Results` and closes
/// at the next line starting `## ` (else EOF). An inline or backticked
/// mention of the heading elsewhere in the plan text never opens the window;
/// no real heading returns the empty set.
fn cited_test_names(plan_text: &str) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    let lines: Vec<&str> = plan_text.lines().collect();
    let Some(idx) = lines.iter().position(|l| l.trim() == "## Test Results") else {
        return names;
    };
    let start = idx + 1;
    let end = lines[start..]
        .iter()
        .position(|l| l.starts_with("## "))
        .map(|off| start + off)
        .unwrap_or(lines.len());
    let section = lines[start..end].join("\n");
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
pub(crate) fn split_md_row(row: &str) -> Vec<String> {
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

/// check(e): each cited Rust identifier must resolve as `fn <name>`, `mod <name>`,
/// or a source file `<name>.rs` somewhere under `crates/` (catches hallucinated
/// identifiers in `## Test Results`).
///
/// Plan-type-aware: when the target plan's `## Test Plan` declares only `manual`
/// probes there is no cited-identifier contract, so the check vacuously passes
/// with a `skipped:` summary instead of harvesting prose. Every other case
/// (missing / mixed / unknown-type / `Type`-less table) is fail-closed — harvest
/// runs.
///
/// Resolution ladder (short-circuit on first hit):
/// 1. `fn <name>` — function definition (overwhelmingly common; costs one walk)
/// 2. `mod <name>` — module declaration (covers directory modules with no
///    `<name>.rs`, e.g. `dispatch_script`)
/// 3. `<name>.rs` file — covers integration-test crates with no `mod` declaration
///
/// `grep_exists` and `rs_file_exists` share the same recursion and `target` skip;
/// their skip lists must stay in sync.
fn check_cited_test_names(plan_text: &str, repo_root: &Path) -> CheckOutcome {
    if let TestPlanKind::AllManual(n) = scan_test_plan_kind(plan_text) {
        return CheckOutcome {
            name: "cited-test-existence".to_string(),
            command: Some(
                "resolve cited Rust identifiers as fn/mod/file under crates/".to_string(),
            ),
            state: CheckState::Pass,
            summary: format!(
                "skipped: all {n} test-plan probes are manual — no automated cited-test contract"
            ),
        };
    }
    let cited = cited_test_names(plan_text);
    let crates_dir = repo_root.join("crates");
    let mut missing = Vec::new();
    for name in &cited {
        let resolved = grep_exists(&crates_dir, &format!("fn {name}"))
            || grep_exists(&crates_dir, &format!("mod {name}"))
            || rs_file_exists(&crates_dir, name);
        if !resolved {
            missing.push(name.clone());
        }
    }
    CheckOutcome {
        name: "cited-test-existence".to_string(),
        command: Some("resolve cited Rust identifiers as fn/mod/file under crates/".to_string()),
        state: if missing.is_empty() {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        summary: if cited.is_empty() {
            "no cited Rust identifiers found".to_string()
        } else if missing.is_empty() {
            format!("{} cited Rust identifier(s) all resolve", cited.len())
        } else {
            format!("cited but missing: {}", missing.join(", "))
        },
    }
}

/// Recursively check whether any `*.rs` file under `dir` contains `needle`.
///
/// Skip list must stay in sync with its sibling `rs_file_exists`.
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

/// Recursively check whether any `*.rs` file under `dir` has file stem `stem`
/// (a file named `<stem>.rs`), independent of its content.
///
/// Skip list must stay in sync with its sibling `grep_exists`.
fn rs_file_exists(dir: &Path, stem: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            if rs_file_exists(&path, stem) {
                return true;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && path.file_stem().and_then(|s| s.to_str()) == Some(stem)
        {
            return true;
        }
    }
    false
}

/// Generated adapter carriers — the doc-sync output whose idempotency we
/// assert. Shared with the renderer's own fixed inventory rather than a
/// locally duplicated list: the five roots plus the two conditional Rust
/// layers. `snapshot_adapters` already skips any path that doesn't exist on
/// disk, so an absent conditional layer (Rust convention not selected)
/// contributes nothing to either snapshot and never causes a spurious
/// idempotency failure — "benign" by the same read-if-present behavior that
/// already handled a missing root.
fn adapter_paths() -> Vec<&'static str> {
    crate::gal::render::REPO_ADAPTER_ROOTS
        .iter()
        .chain(crate::gal::render::REPO_ADAPTER_CONDITIONAL_LAYERS.iter())
        .copied()
        .collect()
}

/// Snapshot (relpath, content-hash) for each existing adapter under `repo_root`.
fn snapshot_adapters(repo_root: &Path) -> Vec<(String, u64)> {
    use std::hash::{Hash, Hasher};
    let mut snap = Vec::new();
    for rel in adapter_paths() {
        let p = repo_root.join(rel);
        if let Ok(bytes) = std::fs::read(&p) {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            bytes.hash(&mut h);
            snap.push((rel.to_string(), h.finish()));
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
/// gal's own render module. finalize-check no longer shells out; repo-adapter
/// render is pure workflow (gal tree) with no management-tree dependency.
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

/// check(g): executor-log scan — v2 bound per-checked-task evidence consumption.
///
/// Replaces the old whole-root verdict (which scanned every log under
/// `.dev/executor-logs/` regardless of plan/task ownership and failed the whole
/// receipt if ANY one was non-completed). The check is now scoped to exactly the
/// target's own `[x]` checked tasks, and for each one reuses the SAME classification
/// engine `gal pipeline-converge-check` uses (`converge_check::scan_plan_log_dir` /
/// `classify_phase` / `REQUIRED_PHASES`), rather than a second implementation.
///
/// For each checked task: the three v1 bookkeeping checks (three-surface+task,
/// task-commit, cursor-cleared) must ALL pass to establish "bound convergence" —
/// this is the same gate that allows a non-completed latest attempt to classify as
/// `recovered-in-conversation` rather than fail. Once convergence is established,
/// each required phase (implement/test/audit) is classified into exactly one of the
/// three R2 outcome literals; an unterminated `started` marker, a malformed header,
/// or an unreadable plan-scoped directory fails that task closed. Zero checked tasks
/// is `NotRun` (nothing to verify yet). Any single checked task's failure fails the
/// whole check(g), naming the task and the reason; a passing overall check(g) counts
/// phases by outcome across every checked task, preserving `recovered-in-conversation`
/// visibility rather than folding it into `dispatch-offload`.
pub(crate) fn check_executor_logs(
    repo_root: &Path,
    target_path: &Path,
    target_text: &str,
) -> CheckOutcome {
    let name = "executor-log-scan".to_string();
    let checked = checked_task_ids(target_text);
    if checked.is_empty() {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::NotRun,
            summary: "no checked ([x]) tasks to verify".to_string(),
        };
    }

    let Some(log_dir) = resolve_log_dir_override(repo_root, Some(target_path)) else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!(
                "could not derive the R1 plan-scoped log directory from the target path for {} checked task(s)",
                checked.len()
            ),
        };
    };

    let mut failures: Vec<String> = Vec::new();
    let mut task_summaries: Vec<String> = Vec::new();
    let mut outcome_counts: BTreeMap<&'static str, usize> = BTreeMap::new();

    for task in &checked {
        // Bound convergence: the same three v1 checks `gal pipeline-converge-check`
        // requires. All three must Pass — a missing/malformed commit note, a
        // checkbox disagreement, or an uncleared cursor means this task's evidence
        // is not bound and cannot be trusted, regardless of dispatch-phase content.
        let surface = check_three_surface_and_task(task, target_path, target_text, repo_root);
        let commit = check_task_commit(task, target_text, repo_root);
        let cursor = check_cursor_cleared(task, target_text);
        let convergence_passed = surface.state == CheckState::Pass
            && commit.state == CheckState::Pass
            && cursor.state == CheckState::Pass;
        if !convergence_passed {
            failures.push(format!(
                "{task}: bound convergence not established — three-surface: {}, task-commit: {}, cursor-cleared: {}",
                surface.summary, commit.summary, cursor.summary
            ));
            continue;
        }

        let scan = scan_plan_log_dir(&log_dir, task);
        if let Some(reason) = &scan.directory_unreadable {
            failures.push(format!(
                "{task}: plan-scoped log directory unreadable — {reason}"
            ));
            continue;
        }
        if !scan.malformed_files.is_empty() {
            failures.push(format!(
                "{task}: malformed attempt log(s), header unparseable: {}",
                scan.malformed_files.join(", ")
            ));
            continue;
        }

        let mut phase_outcomes: Vec<(&str, PhaseOutcome)> = Vec::new();
        let mut task_failed = false;
        for phase in REQUIRED_PHASES {
            let attempts = scan
                .attempts_by_phase
                .get(phase)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            match classify_phase(attempts, convergence_passed) {
                PhaseVerdict::Outcome(outcome) => phase_outcomes.push((phase, outcome)),
                PhaseVerdict::FailClosed(reason) => {
                    failures.push(format!("{task}: phase '{phase}' — {reason}"));
                    task_failed = true;
                    break;
                }
            }
        }
        if task_failed {
            continue;
        }

        let detail = phase_outcomes
            .iter()
            .map(|(phase, outcome)| {
                *outcome_counts.entry(outcome.as_str()).or_insert(0) += 1;
                format!("{phase}={}", outcome.as_str())
            })
            .collect::<Vec<_>>()
            .join(", ");
        task_summaries.push(format!("{task}[{detail}]"));
    }

    let total_phases: usize = outcome_counts.values().sum();
    if failures.is_empty() {
        CheckOutcome {
            name,
            command: Some("scan_plan_log_dir(.dev/executor-logs/<plan-slug>/)".to_string()),
            state: CheckState::Pass,
            summary: format!(
                "{} checked task(s), {} phase(s) — dispatch-offload: {}, in-conversation: {}, recovered-in-conversation: {}; {}",
                checked.len(),
                total_phases,
                outcome_counts.get("dispatch-offload").copied().unwrap_or(0),
                outcome_counts.get("in-conversation").copied().unwrap_or(0),
                outcome_counts
                    .get("recovered-in-conversation")
                    .copied()
                    .unwrap_or(0),
                task_summaries.join("; "),
            ),
        }
    } else {
        CheckOutcome {
            name,
            command: Some("scan_plan_log_dir(.dev/executor-logs/<plan-slug>/)".to_string()),
            state: CheckState::Fail,
            summary: format!(
                "{} of {} checked task(s) failed bound-evidence validation — {}",
                failures.len(),
                checked.len(),
                failures.join(" | "),
            ),
        }
    }
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

/// Render a `finalize-check` receipt with the exact `mode: full|hygiene-only`
/// line the finalize contract checks at each use site. This wraps the shared
/// `Receipt::render()` rather than adding a `mode` field to `Receipt` itself,
/// since that struct is shared verbatim by six other check subcommands
/// (`boundary-check`, `converge-check`, `planning-check`, `prompt-check`,
/// `refining-check`, `pipeline-preflight`) that have no such concept.
fn render_finalize_receipt(receipt: &Receipt, mode: HygieneMode) -> String {
    let mut out = String::from("# finalize-check receipt\n\n");
    out.push_str(&format!(
        "overall: {}\n",
        if receipt.passed() { "pass" } else { "fail" }
    ));
    out.push_str(&format!("mode: {}\n\n", mode.as_str()));
    out.push_str("| check | state | command | summary |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for c in &receipt.checks {
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

    let mut checks = Vec::new();
    if parsed.mode == HygieneMode::Full {
        checks.append(&mut check_authoritative(&repo_root));
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
        checks.push(check_executor_logs(
            &repo_root,
            &parsed.target,
            &target_text,
        ));
        checks.push(check_durable_layer_commit(&target_text, &repo_root));
    }

    // Repo-hygiene rows (see finalize_hygiene): universal rows run in both
    // modes; GAL-source-repo-scoped rows are emitted only when
    // `plugins/gal-core/` exists at the repo root — a downstream repo never
    // sees them at all (absent, not `NotRun`).
    checks.push(check_project_source_doc_existence(&repo_root));
    checks.push(check_state_bound(&repo_root, parsed.mode));
    if is_gal_source_repo(&repo_root) {
        checks.push(check_contract_roster_parity(&repo_root));
        checks.push(check_doc_link_resolution(&repo_root));
    }

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
    if let Err(e) = std::fs::write(
        &parsed.receipt,
        render_finalize_receipt(&receipt, parsed.mode),
    ) {
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
    use tempfile::TempDir;

    fn outcome(name: &str, state: CheckState) -> CheckOutcome {
        CheckOutcome {
            name: name.to_string(),
            command: None,
            state,
            summary: String::new(),
        }
    }

    #[test]
    fn check_naming_gate_fails_closed_outside_git_repo() {
        // No `git init`: scan_tree cannot prove a clean result and must fail
        // the check, not report a false Pass.
        let tmp = TempDir::new().unwrap();
        let result = check_naming_gate(tmp.path());
        assert_eq!(result.state, CheckState::Fail);
        assert!(
            result.summary.contains("scan failed"),
            "{:?}",
            result.summary
        );
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
        let md = render_finalize_receipt(&r, HygieneMode::Full);
        assert!(md.contains("overall: pass"));
        assert!(md.contains("mode: full"));
        assert!(md.contains("commit-existence"));
        assert!(md.contains("| check | state | command | summary |"));
    }

    #[test]
    fn render_finalize_receipt_hygiene_only_mode_line() {
        let r = Receipt {
            checks: vec![outcome("state-bound", CheckState::Fail)],
        };
        let md = render_finalize_receipt(&r, HygieneMode::HygieneOnly);
        assert!(md.contains("overall: fail"));
        assert!(md.contains("mode: hygiene-only"));
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

    #[test]
    fn parse_defaults_to_full_mode() {
        let a = parse_args(&[".dev/plans/x.md".to_string()]).unwrap();
        assert_eq!(a.mode, HygieneMode::Full);
        assert!(a.receipt.ends_with("finalize-check.receipt.md"));
    }

    #[test]
    fn parse_hygiene_only_flag_switches_mode_and_default_receipt() {
        let a = parse_args(&[".dev/plans/x.md".to_string(), "--hygiene-only".to_string()]).unwrap();
        assert_eq!(a.mode, HygieneMode::HygieneOnly);
        assert!(a.receipt.ends_with("finalize-check.hygiene.receipt.md"));
    }

    #[test]
    fn parse_hygiene_only_with_explicit_receipt_keeps_explicit_path() {
        let a = parse_args(&[
            ".dev/plans/x.md".to_string(),
            "--hygiene-only".to_string(),
            "--receipt".to_string(),
            "out/custom.md".to_string(),
        ])
        .unwrap();
        assert_eq!(a.mode, HygieneMode::HygieneOnly);
        assert_eq!(a.receipt, PathBuf::from("out/custom.md"));
    }

    #[test]
    fn full_and_hygiene_only_default_receipt_paths_are_distinct() {
        assert_ne!(
            default_receipt_path(HygieneMode::Full),
            default_receipt_path(HygieneMode::HygieneOnly)
        );
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
    const TP_HEADER: &str =
        "## Test Plan\n\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n";

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
        assert!(out
            .summary
            .contains("this_fn_absolutely_does_not_exist_zzz"));
    }

    #[test]
    fn cited_test_names_runs_for_unit_plan_real_fn_passes() {
        // `unit` TP present → harvest runs; a real fn under crates/ passes.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("crates")).unwrap();
        std::fs::write(
            tmp.path().join("crates").join("x.rs"),
            "fn my_probe_fn() {}\n",
        )
        .unwrap();
        let txt = format!(
            "{TP_HEADER}{}\n## Test Results\n\n`my_probe_fn` PASS\n",
            test_plan_row(1, "unit", 1)
        );
        let out = check_cited_test_names(&txt, tmp.path());
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("all resolve"));
    }

    #[test]
    fn cited_test_names_runs_for_unit_plan_mod_rung_passes() {
        // `unit` TP present → harvest runs; a name resolving only via `mod <name>`
        // (a directory module with no matching file stem) passes at the mod rung.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("crates")).unwrap();
        std::fs::write(
            tmp.path().join("crates").join("lib.rs"),
            "mod probe_module_only;\n",
        )
        .unwrap();
        let txt = format!(
            "{TP_HEADER}{}\n## Test Results\n\n`probe_module_only` PASS\n",
            test_plan_row(1, "unit", 1)
        );
        let out = check_cited_test_names(&txt, tmp.path());
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("all resolve"));
    }

    #[test]
    fn cited_test_names_runs_for_unit_plan_file_stem_rung_passes() {
        // `unit` TP present → harvest runs; a name resolving only via a matching
        // `<name>.rs` file stem (no `fn`/`mod` declaration) passes at the file-stem
        // rung.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("crates")).unwrap();
        std::fs::write(
            tmp.path().join("crates").join("probe_file_only.rs"),
            "// no fn or mod declarations for this identifier\n",
        )
        .unwrap();
        let txt = format!(
            "{TP_HEADER}{}\n## Test Results\n\n`probe_file_only` PASS\n",
            test_plan_row(1, "unit", 1)
        );
        let out = check_cited_test_names(&txt, tmp.path());
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("all resolve"));
    }

    // --- check(e): line-anchored window (P22 hijack regression) ---

    #[test]
    fn cited_test_names_ignores_inline_decoy_heading_before_real_section() {
        // Handoff-Notes-shaped prose inline-mentions the heading literal before
        // the real section opens. The line-anchored window must not open there:
        // a decoy-only backtick token must never be harvested, while a token
        // planted in the real section still is.
        let txt = "## Handoff Notes\n\n\
            earlier note mentions `## Test Results` inline and cites \
            `decoy_only_token_never_harvested`.\n\n\
            ## Test Results\n\n\
            `this_test_does_not_exist_anywhere_xyz` PASS\n";
        let names = cited_test_names(txt);
        assert!(names.contains("this_test_does_not_exist_anywhere_xyz"));
        assert!(!names.contains("decoy_only_token_never_harvested"));
    }

    #[test]
    fn check_cited_test_names_hijack_decoy_fabricated_token_fails_by_name() {
        // Same decoy shape at the check(e) level: the fabricated token planted
        // in the real section still fails and is named; the decoy-only token
        // is excluded from the summary.
        let txt = format!(
            "## Handoff Notes\n\n\
            earlier note mentions `## Test Results` inline and cites \
            `decoy_only_token_never_harvested`.\n\n\
            {TP_HEADER}{}\n## Test Results\n\n\
            `this_fn_absolutely_does_not_exist_zzz` PASS\n",
            test_plan_row(1, "unit", 1)
        );
        let out = check_cited_test_names(&txt, Path::new("."));
        assert_eq!(out.state, CheckState::Fail);
        assert!(out
            .summary
            .contains("this_fn_absolutely_does_not_exist_zzz"));
        assert!(!out.summary.contains("decoy_only_token_never_harvested"));
    }

    #[test]
    fn check_cited_test_names_hijack_decoy_honest_real_citation_passes() {
        // Same decoy shape, but the real section only cites a real fn — proves
        // the window still finds and resolves an honest citation past a decoy.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("crates")).unwrap();
        std::fs::write(
            tmp.path().join("crates").join("x.rs"),
            "fn my_probe_fn() {}\n",
        )
        .unwrap();
        let txt = format!(
            "## Handoff Notes\n\n\
            earlier note mentions `## Test Results` inline and cites \
            `decoy_only_token_never_harvested`.\n\n\
            {TP_HEADER}{}\n## Test Results\n\n\
            `my_probe_fn` PASS\n",
            test_plan_row(1, "unit", 1)
        );
        let out = check_cited_test_names(&txt, tmp.path());
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("all resolve"));
        assert!(!out.summary.contains("decoy_only_token_never_harvested"));
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

    #[test]
    fn sync_idempotency_fails_when_a_conditional_layer_changes_between_runs() {
        let tmp = tempfile::tempdir().unwrap();
        // All 5 roots present and stable — only the shared conditional layer
        // pulled from crate::gal::render::REPO_ADAPTER_CONDITIONAL_LAYERS
        // changes between the two syncs.
        for root in crate::gal::render::REPO_ADAPTER_ROOTS {
            let p = tmp.path().join(root);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, "stable root content\n").unwrap();
        }
        let layer = tmp
            .path()
            .join(crate::gal::render::REPO_ADAPTER_CONDITIONAL_LAYERS[0]);
        std::fs::create_dir_all(layer.parent().unwrap()).unwrap();
        std::fs::write(&layer, "v0\n").unwrap();

        let counter = std::cell::Cell::new(0u32);
        let out = check_sync_idempotency(tmp.path(), || {
            let n = counter.get();
            counter.set(n + 1);
            std::fs::write(&layer, format!("v{n}\n")).unwrap();
        });
        assert_eq!(
            out.state,
            CheckState::Fail,
            "a changed conditional layer must fail idempotency even with all roots stable, got {out:?}"
        );
        assert!(out.summary.contains("non-idempotent"));
    }

    #[test]
    fn sync_idempotency_passes_when_the_full_seven_path_inventory_is_stable() {
        let tmp = tempfile::tempdir().unwrap();
        for root in crate::gal::render::REPO_ADAPTER_ROOTS {
            let p = tmp.path().join(root);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, format!("{root} content\n")).unwrap();
        }
        for layer in crate::gal::render::REPO_ADAPTER_CONDITIONAL_LAYERS {
            let p = tmp.path().join(layer);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, format!("{layer} content\n")).unwrap();
        }

        let out = check_sync_idempotency(tmp.path(), || {});
        assert_eq!(out.state, CheckState::Pass);
        assert!(
            out.summary.contains("7 adapter"),
            "expected the summary to count all seven present paths, got {:?}",
            out.summary
        );
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

    // --- check(g): executor-log scan (v2, per-checked-task bound evidence) ---

    /// Hermetic git repo with an identity configured (needed for a real commit).
    fn make_git_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        std::process::Command::new("git")
            .args(["init", root])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "config", "user.email", "t@t"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "config", "user.name", "t"])
            .output()
            .unwrap();
        tmp
    }

    /// Stage and commit everything in `tmp`, returning the new commit's short hash.
    fn commit_all(tmp: &TempDir, msg: &str) -> String {
        let root = tmp.path().to_str().unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "add", "-A"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "commit", "-m", msg])
            .output()
            .unwrap();
        let out = std::process::Command::new("git")
            .args(["-C", root, "rev-parse", "--short", "HEAD"])
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    /// Write one well-formed `GAL-DISPATCH-LOG v1` attempt log — mirrors the exact
    /// header shape `dispatch::dispatch::write_log` produces (duplicated here as a
    /// small test-only fixture, same pattern `converge_check.rs`'s own tests use).
    fn write_attempt_log(
        dir: &Path,
        filename: &str,
        task_id: &str,
        phase: &str,
        terminal_state: &str,
        timestamp_start: &str,
    ) {
        std::fs::create_dir_all(dir).unwrap();
        let content = format!(
            "GAL-DISPATCH-LOG v1\n\
             timestamp_start: {timestamp_start}\n\
             timestamp_end:   {timestamp_start}\n\
             duration_ms:     1\n\
             executor:        claude\n\
             phase:           {phase}\n\
             task_id:         {task_id}\n\
             git_branch:      main\n\
             git_head:        abc1234\n\
             exit_code:       0\n\
             actual_model:    m\n\
             terminal_state:  {terminal_state}\n\
             session_id:      none\n\
             ---STDOUT---\n\
             \n\
             ---STDERR---\n\
             \n"
        );
        std::fs::write(dir.join(filename), content).unwrap();
    }

    fn attempt_filename(secs: u64, seq: u32, task: &str, phase: &str) -> String {
        format!("{secs:010}-000000001-{seq:06}-{task}-{phase}-claude.log")
    }

    /// Write a paired source plan + execution prompt (identical checked-task
    /// content, cursor cleared) under `<tmp>/.dev/plans/<slug>.{md,prompt.md}`.
    /// `tasks` is `(task_id, optional *(hash)* commit note)`.
    fn write_plan_and_prompt(tmp: &TempDir, slug: &str, tasks: &[(&str, Option<&str>)]) -> PathBuf {
        let plans_dir = tmp.path().join(".dev").join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        let mut body = String::from("## Status\n\nCurrent Task: —\n\n## Tasks\n\n");
        for (task, hash) in tasks {
            match hash {
                Some(h) => body.push_str(&format!("- [x] {task} — done. *({h})*\n")),
                None => body.push_str(&format!("- [x] {task} — done.\n")),
            }
        }
        std::fs::write(plans_dir.join(format!("{slug}.md")), &body).unwrap();
        let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
        std::fs::write(&prompt_path, &body).unwrap();
        prompt_path
    }

    // (a) every checked task has complete dispatch-offload evidence → overall Pass,
    // phase counts surfaced in the summary.
    #[test]
    fn executor_log_scan_all_checked_tasks_pass() {
        let tmp = make_git_repo();
        std::fs::write(tmp.path().join("README.md"), "x").unwrap();
        let hash = commit_all(&tmp, "init");
        let task = tid(1);
        let prompt_path = write_plan_and_prompt(&tmp, "demo-plan", &[(&task, Some(&hash))]);
        let log_dir = resolve_log_dir_override(tmp.path(), Some(prompt_path.as_path())).unwrap();
        for phase in ["implement", "test", "audit"] {
            write_attempt_log(
                &log_dir,
                &attempt_filename(1, 0, &task, phase),
                &task,
                phase,
                "completed",
                "2026-07-16T00:00:00Z",
            );
        }
        let prompt_text = std::fs::read_to_string(&prompt_path).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &prompt_text);
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("dispatch-offload: 3"));
        assert!(out.summary.contains(&task));
    }

    // (b) a checked task with no evidence at all (no commit note, so bound
    // convergence can never be established) → overall Fail, naming that task.
    #[test]
    fn executor_log_scan_task_with_no_evidence_fails_naming_task() {
        let tmp = make_git_repo();
        let task = tid(2);
        let prompt_path = write_plan_and_prompt(&tmp, "demo-plan", &[(&task, None)]);
        let prompt_text = std::fs::read_to_string(&prompt_path).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &prompt_text);
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains(&task));
    }

    // (c) a checked task with a malformed attempt-log header in its plan-scoped
    // directory → overall Fail, naming that task and the malformed evidence.
    #[test]
    fn executor_log_scan_malformed_header_fails_naming_task() {
        let tmp = make_git_repo();
        std::fs::write(tmp.path().join("README.md"), "x").unwrap();
        let hash = commit_all(&tmp, "init");
        let task = tid(3);
        let prompt_path = write_plan_and_prompt(&tmp, "demo-plan", &[(&task, Some(&hash))]);
        let log_dir = resolve_log_dir_override(tmp.path(), Some(prompt_path.as_path())).unwrap();
        std::fs::create_dir_all(&log_dir).unwrap();
        std::fs::write(log_dir.join("garbled.log"), "not a real log at all").unwrap();
        let prompt_text = std::fs::read_to_string(&prompt_path).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &prompt_text);
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains(&task));
        assert!(out.summary.contains("malformed"));
    }

    // (d) zero checked tasks → NotRun (nothing to verify yet), never a silent pass.
    #[test]
    fn executor_log_scan_zero_checked_tasks_is_not_run() {
        let tmp = make_git_repo();
        let task = tid(4);
        let prompt_path = tmp.path().join("demo.prompt.md");
        let text = format!("## Tasks\n\n- [ ] {task} — pending\n");
        std::fs::write(&prompt_path, &text).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &text);
        assert_eq!(out.state, CheckState::NotRun);
    }

    // (e) a mix of one passing and one failing checked task → overall Fail, naming
    // ONLY the failing task (the passing task's own detail is not a failure entry).
    #[test]
    fn executor_log_scan_mixed_tasks_fails_naming_only_the_failing_task() {
        let tmp = make_git_repo();
        std::fs::write(tmp.path().join("README.md"), "x").unwrap();
        let hash = commit_all(&tmp, "init");
        let good = tid(5);
        let bad = tid(6);
        let prompt_path =
            write_plan_and_prompt(&tmp, "demo-plan", &[(&good, Some(&hash)), (&bad, None)]);
        let log_dir = resolve_log_dir_override(tmp.path(), Some(prompt_path.as_path())).unwrap();
        for phase in ["implement", "test", "audit"] {
            write_attempt_log(
                &log_dir,
                &attempt_filename(1, 0, &good, phase),
                &good,
                phase,
                "completed",
                "2026-07-16T00:00:00Z",
            );
        }
        let prompt_text = std::fs::read_to_string(&prompt_path).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &prompt_text);
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains(&bad));
        assert!(
            !out.summary.contains(&format!("{good}:")),
            "the passing task must not appear as a failure entry: {}",
            out.summary
        );
    }

    // (f) recovered-in-conversation must be counted separately from
    // dispatch-offload in check(g)'s own aggregate summary, not folded into it.
    // converge_check's classify_phase already proves the R2 literal is produced;
    // this proves finalize_check's aggregation preserves that distinction.
    #[test]
    fn executor_log_scan_recovered_in_conversation_counted_separately_from_dispatch_offload() {
        let tmp = make_git_repo();
        std::fs::write(tmp.path().join("README.md"), "x").unwrap();
        let hash = commit_all(&tmp, "init");
        let task = tid(7);
        let prompt_path = write_plan_and_prompt(&tmp, "demo-plan", &[(&task, Some(&hash))]);
        let log_dir = resolve_log_dir_override(tmp.path(), Some(prompt_path.as_path())).unwrap();
        // implement + audit: completed (dispatch-offload). test: non-completed latest
        // attempt, but bound convergence passes → recovered-in-conversation, not a fail.
        write_attempt_log(
            &log_dir,
            &attempt_filename(1, 0, &task, "implement"),
            &task,
            "implement",
            "completed",
            "2026-07-16T00:00:00Z",
        );
        write_attempt_log(
            &log_dir,
            &attempt_filename(2, 0, &task, "test"),
            &task,
            "test",
            "timeout",
            "2026-07-16T00:00:01Z",
        );
        write_attempt_log(
            &log_dir,
            &attempt_filename(3, 0, &task, "audit"),
            &task,
            "audit",
            "completed",
            "2026-07-16T00:00:02Z",
        );
        let prompt_text = std::fs::read_to_string(&prompt_path).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &prompt_text);
        assert_eq!(out.state, CheckState::Pass);
        assert!(
            out.summary.contains("dispatch-offload: 2"),
            "expected exactly 2 dispatch-offload phases: {}",
            out.summary
        );
        assert!(
            out.summary.contains("recovered-in-conversation: 1"),
            "expected the timeout phase to surface as recovered-in-conversation, not folded \
             into dispatch-offload: {}",
            out.summary
        );
        assert!(
            !out.summary.contains("dispatch-offload: 3"),
            "recovered-in-conversation must not be silently counted as dispatch-offload: {}",
            out.summary
        );
    }

    // (g) two checked tasks share the same plan-scoped log directory (same plan,
    // sibling tasks). Task B has zero attempt logs of its own; task A's logs sit in
    // the same directory. Task B must classify every phase as in-conversation (no
    // matching evidence), never accidentally credited with task A's completed
    // evidence by directory proximity alone — the exact task_id header match in
    // `scan_plan_log_dir` is what `finalize_check`'s check(g) relies on.
    #[test]
    fn executor_log_scan_sibling_task_in_same_directory_does_not_leak_evidence() {
        let tmp = make_git_repo();
        std::fs::write(tmp.path().join("README.md"), "x").unwrap();
        let hash = commit_all(&tmp, "init");
        let task_a = tid(8);
        let task_b = tid(9);
        let prompt_path = write_plan_and_prompt(
            &tmp,
            "demo-plan",
            &[(&task_a, Some(&hash)), (&task_b, Some(&hash))],
        );
        let log_dir = resolve_log_dir_override(tmp.path(), Some(prompt_path.as_path())).unwrap();
        // Only task_a gets real evidence; task_b's directory is shared but empty of
        // its own task_id.
        for phase in ["implement", "test", "audit"] {
            write_attempt_log(
                &log_dir,
                &attempt_filename(1, 0, &task_a, phase),
                &task_a,
                phase,
                "completed",
                "2026-07-16T00:00:00Z",
            );
        }
        let prompt_text = std::fs::read_to_string(&prompt_path).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &prompt_text);
        // task_b has no attempts of its own → every required phase is legitimately
        // in-conversation, which is a passing (not failing) outcome — so the overall
        // check still passes, but task_b must show as in-conversation, not as having
        // inherited task_a's dispatch-offload evidence.
        assert_eq!(out.state, CheckState::Pass);
        assert!(
            out.summary
                .contains(&format!("{task_b}[implement=in-conversation")),
            "task_b must not inherit task_a's evidence from the shared directory: {}",
            out.summary
        );
        assert!(
            out.summary.contains("dispatch-offload: 3"),
            "task_a's 3 real phases must still be counted: {}",
            out.summary
        );
        assert!(
            out.summary.contains("in-conversation: 3"),
            "task_b's 3 phases must classify as in-conversation, not dispatch-offload: {}",
            out.summary
        );
    }

    // --- hermetic end-to-end fixtures for check(g) (R8/R9) ---
    //
    // Each scenario below builds its own complete `TempDir` git repo, a real
    // execution-prompt-shaped text with `[x]` tasks and `*(hash)*` commit notes, and
    // (where relevant) a real plan-scoped `.dev/executor-logs/<slug>/` directory
    // populated with real `.log` files in the exact `GAL-DISPATCH-LOG v1` format,
    // then calls the real `check_executor_logs` entry point directly — not an
    // internal helper. These are deliberately DIFFERENT scenarios from the five
    // above (a)-(g): those exercise all-completed / no-commit-note / malformed
    // header / zero-checked / mixed-tasks shapes; these exercise the R8/R9
    // recovery-and-isolation matrix (all-in-conversation, missing-receipt,
    // foreign-plan-same-task-id, mixed-mode recovery, and unrecovered timeout).

    // (1) all-in-conversation pass: a checked task with a valid commit note, cursor
    // cleared, three-surface agreement — and NO attempt log files at all (the
    // plan-scoped directory is never created). Every required phase legitimately
    // classifies `in-conversation`, and the check still Passes.
    #[test]
    fn end_to_end_all_in_conversation_is_pass() {
        let tmp = make_git_repo();
        std::fs::write(tmp.path().join("README.md"), "x").unwrap();
        let hash = commit_all(&tmp, "init");
        let task = tid(1);
        let prompt_path =
            write_plan_and_prompt(&tmp, "all-in-conversation-plan", &[(&task, Some(&hash))]);
        // Deliberately do NOT create the plan-scoped log directory at all.
        let prompt_text = std::fs::read_to_string(&prompt_path).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &prompt_text);
        assert_eq!(out.state, CheckState::Pass);
        assert!(out.summary.contains("in-conversation: 3"));
        assert!(out.summary.contains("dispatch-offload: 0"));
        assert!(out.summary.contains("recovered-in-conversation: 0"));
    }

    // (2) missing-commit-note fail: the bound-convergence prerequisite itself is
    // missing — a checked `[x]` task whose commit note is absent, so bound
    // convergence can never be established regardless of any attempt evidence.
    // (check(g) re-establishes convergence in-process; it reads no receipt file.)
    #[test]
    fn end_to_end_missing_commit_note_is_fail() {
        let tmp = make_git_repo();
        let task = tid(1);
        let prompt_path = write_plan_and_prompt(&tmp, "missing-commit-note-plan", &[(&task, None)]);
        let prompt_text = std::fs::read_to_string(&prompt_path).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &prompt_text);
        assert_eq!(out.state, CheckState::Fail);
        assert!(
            out.summary.contains(&task),
            "failure summary must name the specific task: {}",
            out.summary
        );
    }

    // (2b) missing-commit-note fail even with a complete, otherwise-perfect
    // attempt-log trail: unlike (2) above, every required phase has real
    // `terminal_state: completed` evidence sitting in the plan-scoped directory —
    // proving the bound-convergence gate is checked BEFORE and independently of
    // any scan content, so real dispatch evidence can never substitute for a
    // missing commit note. Without this fixture, (2)'s "no commit note" case is
    // indistinguishable from "nothing to scan happened to also be missing";
    // this closes that ambiguity by giving the scan something real to find and
    // showing it still gets rejected.
    #[test]
    fn end_to_end_missing_commit_note_fails_even_with_complete_dispatch_evidence() {
        let tmp = make_git_repo();
        std::fs::write(tmp.path().join("README.md"), "x").unwrap();
        commit_all(&tmp, "init");
        let task = tid(1);
        let prompt_path = write_plan_and_prompt(
            &tmp,
            "missing-commit-note-with-evidence-plan",
            &[(&task, None)],
        );
        let log_dir = resolve_log_dir_override(tmp.path(), Some(prompt_path.as_path())).unwrap();
        for phase in ["implement", "test", "audit"] {
            write_attempt_log(
                &log_dir,
                &attempt_filename(1, 0, &task, phase),
                &task,
                phase,
                "completed",
                "2026-07-16T00:00:00Z",
            );
        }
        let prompt_text = std::fs::read_to_string(&prompt_path).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &prompt_text);
        assert_eq!(out.state, CheckState::Fail);
        assert!(
            out.summary.contains(&task),
            "failure summary must name the specific task: {}",
            out.summary
        );
        assert!(
            out.summary.contains("bound convergence not established"),
            "the failure must trace to the bound-convergence gate, not to an absence \
             of dispatch evidence — real completed logs exist but must never be \
             scanned or counted while convergence is not established: {}",
            out.summary
        );
        assert!(
            !out.summary.contains("dispatch-offload: 3"),
            "complete attempt evidence must never be credited toward a pass when \
             bound convergence itself is not established: {}",
            out.summary
        );
    }

    // (3) foreign same-task-id timeout is irrelevant (R9): a SEPARATE plan-scoped
    // directory (different plan slug, same repo) carries a `timeout` attempt log
    // for the SAME task id as "this" plan's checked task. "This" plan has zero
    // attempt logs of its own — finalizing it must still Pass as in-conversation,
    // proving `resolve_log_dir_override`'s per-plan scoping keeps the foreign
    // plan's stale timeout from ever being scanned.
    #[test]
    fn end_to_end_foreign_plan_same_task_id_timeout_is_irrelevant() {
        let tmp = make_git_repo();
        std::fs::write(tmp.path().join("README.md"), "x").unwrap();
        let hash = commit_all(&tmp, "init");
        let task = tid(1);
        let this_prompt = write_plan_and_prompt(&tmp, "this-plan", &[(&task, Some(&hash))]);
        let foreign_prompt = write_plan_and_prompt(&tmp, "foreign-plan", &[(&task, Some(&hash))]);
        let foreign_log_dir =
            resolve_log_dir_override(tmp.path(), Some(foreign_prompt.as_path())).unwrap();
        write_attempt_log(
            &foreign_log_dir,
            &attempt_filename(1, 0, &task, "implement"),
            &task,
            "implement",
            "timeout",
            "2026-07-16T00:00:00Z",
        );

        let this_text = std::fs::read_to_string(&this_prompt).unwrap();
        let out = check_executor_logs(tmp.path(), &this_prompt, &this_text);
        assert_eq!(out.state, CheckState::Pass);
        assert!(
            out.summary.contains("in-conversation: 3"),
            "this plan's own zero-evidence phases must classify in-conversation, \
             unaffected by the foreign plan's timeout: {}",
            out.summary
        );
        assert!(
            out.summary.contains("dispatch-offload: 0")
                && out.summary.contains("recovered-in-conversation: 0"),
            "the foreign plan's timeout must not surface as this plan's evidence: {}",
            out.summary
        );
    }

    // (4) mixed-mode recovered timeout pass with recovery: a checked task with
    // valid bound convergence where implement is dispatch-offload (completed),
    // test's latest attempt is a non-completed `timeout` that legally classifies
    // `recovered-in-conversation` because convergence holds, and audit has no
    // attempt at all (in-conversation). Overall Pass, and the passing summary must
    // surface the recovered outcome visibly, not fold it into dispatch-offload.
    #[test]
    fn end_to_end_mixed_mode_recovered_timeout_is_pass_with_recovery_surfaced() {
        let tmp = make_git_repo();
        std::fs::write(tmp.path().join("README.md"), "x").unwrap();
        let hash = commit_all(&tmp, "init");
        let task = tid(1);
        let prompt_path = write_plan_and_prompt(&tmp, "mixed-mode-plan", &[(&task, Some(&hash))]);
        let log_dir = resolve_log_dir_override(tmp.path(), Some(prompt_path.as_path())).unwrap();
        write_attempt_log(
            &log_dir,
            &attempt_filename(1, 0, &task, "implement"),
            &task,
            "implement",
            "completed",
            "2026-07-16T00:00:00Z",
        );
        write_attempt_log(
            &log_dir,
            &attempt_filename(2, 0, &task, "test"),
            &task,
            "test",
            "timeout",
            "2026-07-16T00:00:01Z",
        );
        // audit: no attempt log at all.
        let prompt_text = std::fs::read_to_string(&prompt_path).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &prompt_text);
        assert_eq!(out.state, CheckState::Pass);
        assert!(
            out.summary
                .contains(&format!("{task}[implement=dispatch-offload")),
            "implement must classify dispatch-offload: {}",
            out.summary
        );
        assert!(
            out.summary.contains("test=recovered-in-conversation"),
            "test's timeout must legally recover, visibly, not silently pass as something else: {}",
            out.summary
        );
        assert!(
            out.summary.contains("audit=in-conversation"),
            "audit's zero-evidence phase must classify in-conversation: {}",
            out.summary
        );
        assert!(out.summary.contains("dispatch-offload: 1"));
        assert!(out.summary.contains("recovered-in-conversation: 1"));
        assert!(out.summary.contains("in-conversation: 1"));
    }

    // (5) same timeout shape without passing convergence fails: identical
    // non-completed-latest-attempt evidence as (4), but bound convergence does NOT
    // hold this time (cursor still points at this task) — proving a non-completed
    // attempt is never forgiven unconditionally, only when convergence genuinely
    // holds.
    #[test]
    fn end_to_end_same_timeout_without_convergence_is_fail() {
        let tmp = make_git_repo();
        std::fs::write(tmp.path().join("README.md"), "x").unwrap();
        let hash = commit_all(&tmp, "init");
        let task = tid(1);
        let plans_dir = tmp.path().join(".dev").join("plans");
        std::fs::create_dir_all(&plans_dir).unwrap();
        // Cursor still points at this task on both surfaces — convergence cannot
        // be established even though the commit note and checkbox are otherwise
        // valid.
        let body = format!(
            "## Status\n\nCurrent Task: {task}\n\n## Tasks\n\n- [x] {task} — done. *({hash})*\n"
        );
        std::fs::write(plans_dir.join("unrecovered-plan.md"), &body).unwrap();
        let prompt_path = plans_dir.join("unrecovered-plan.prompt.md");
        std::fs::write(&prompt_path, &body).unwrap();

        let log_dir = resolve_log_dir_override(tmp.path(), Some(prompt_path.as_path())).unwrap();
        write_attempt_log(
            &log_dir,
            &attempt_filename(1, 0, &task, "implement"),
            &task,
            "implement",
            "timeout",
            "2026-07-16T00:00:00Z",
        );

        let prompt_text = std::fs::read_to_string(&prompt_path).unwrap();
        let out = check_executor_logs(tmp.path(), &prompt_path, &prompt_text);
        assert_eq!(out.state, CheckState::Fail);
        assert!(
            out.summary.contains(&task),
            "failure summary must name the specific task: {}",
            out.summary
        );
        assert!(
            out.summary.contains("cursor not cleared"),
            "failure must trace to the broken convergence prerequisite, not an \
             unconditional forgiveness of the non-completed attempt: {}",
            out.summary
        );
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
