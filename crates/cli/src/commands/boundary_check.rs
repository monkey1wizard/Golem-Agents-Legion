//! `gal boundary-check` — pipeline-internal 2c pre-commit allowlist check.
//!
//! Resolves the task's affected-files allowlist via the shared
//! `pipeline::task_spec` parser (the same set the dispatch spec shows the
//! executor: task-block backtick paths, falling back to
//! `## Files to Create or Modify`) and compares it against `git diff
//! --name-only` plus untracked files from `git status --porcelain`.
//!
//! ## Critical contract (architect blocking requirement)
//!
//! When the task names no affected files at all → `CheckState::NotRun` (exit
//! non-zero). This is an INVARIANT of the binary's judgment body, NOT just a
//! Verify note. Degrading to "no allowlist → allow everything" would silently
//! bypass the entire boundary gate for any task that forgets to declare its
//! allowlist.
//!
//! Not a public `/gal` slash command (peer of `finalize-check`/`converge-check`).

use super::dispatch::resolve_receipt_path;
use super::finalize_check::{CheckOutcome, CheckState, Receipt};
use super::test_first_fs;
use super::test_first_transition::{self, TransitionJournalStatus};
use gal_engine::ExitCode;
use gal_foundation::validated_repo_path::{ValidatedRepoPath, ValidatedRepoPathMode};
use pipeline::task_spec::{
    extract_plan_slug, has_test_first_marker, parse_task_contract, Applicability,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundaryKind {
    ImplementationCommit,
    StateRecording,
    PostTest,
    PostAudit,
}

impl BoundaryKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::ImplementationCommit => "implementation-commit",
            Self::StateRecording => "state-recording",
            Self::PostTest => "post-test",
            Self::PostAudit => "post-audit",
        }
    }

    pub(crate) fn parse(s: &str) -> Result<Self, String> {
        match s {
            "implementation-commit" => Ok(Self::ImplementationCommit),
            "state-recording" => Ok(Self::StateRecording),
            "post-test" => Ok(Self::PostTest),
            "post-audit" => Ok(Self::PostAudit),
            _ => Err(format!("unknown boundary kind '{s}'")),
        }
    }
}

struct Args {
    prompt: PathBuf,
    task: String,
    receipt: PathBuf,
    boundary_kind: BoundaryKind,
    snapshot_phase: Option<String>,
    generation: Option<u32>,
    capture: bool,
    restore: bool,
}

fn default_receipt_path(prompt: &Path, task: &str) -> PathBuf {
    resolve_receipt_path(Some(prompt), format!("{task}-boundary-check.receipt.md"))
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut prompt: Option<PathBuf> = None;
    let mut task: Option<String> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut boundary_kind = BoundaryKind::ImplementationCommit;
    let mut snapshot_phase = None;
    let mut generation = None;
    let mut capture = false;
    let mut restore = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--task" => {
                let v = it.next().ok_or("--task requires a task id")?;
                task = Some(v.clone());
            }
            "--receipt" => {
                let v = it.next().ok_or("--receipt requires a path")?;
                receipt = Some(PathBuf::from(v));
            }
            "--boundary-kind" => {
                let v = it.next().ok_or("--boundary-kind requires a value")?;
                boundary_kind = BoundaryKind::parse(v)?;
            }
            "--phase" | "--snapshot-phase" => {
                snapshot_phase = Some(it.next().ok_or("--phase requires a value")?.clone());
            }
            "--generation" => {
                let v = it.next().ok_or("--generation requires a number")?;
                generation = Some(
                    v.parse()
                        .map_err(|_| format!("invalid generation number '{v}'"))?,
                );
            }
            "--capture" => capture = true,
            "--restore" => restore = true,
            s if s.starts_with("--") => return Err(format!("unknown option '{s}'")),
            s => {
                if prompt.is_none() {
                    prompt = Some(PathBuf::from(s));
                } else {
                    return Err(format!("unexpected extra argument '{s}'"));
                }
            }
        }
    }
    let task = task.ok_or("--task <id> is required")?;
    let prompt = prompt.ok_or("boundary-check requires a <prompt> path")?;
    let receipt = receipt.unwrap_or_else(|| default_receipt_path(&prompt, &task));
    Ok(Args {
        prompt,
        task,
        receipt,
        boundary_kind,
        snapshot_phase,
        generation,
        capture,
        restore,
    })
}

#[derive(Debug, Clone)]
struct RestoreMember {
    entry: test_first_fs::SnapshotEntry,
    bytes: Option<Vec<u8>>,
}

fn restore_paths(
    contract: &pipeline::task_spec::TaskContract,
    phase: &str,
) -> Result<Vec<String>, String> {
    let mut paths = contract.production_paths.clone();
    if phase != "implementation-commit" && phase != "pre-implement" {
        paths.extend(contract.test_paths.clone());
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn restore_snapshot_members(
    repo_root: &Path,
    slug: &str,
    task: &str,
    generation: u32,
    digest: &str,
    phase: &str,
    paths: &[String],
) -> Result<Vec<RestoreMember>, String> {
    let root = PathBuf::from(".dev")
        .join("pipeline")
        .join("snapshots")
        .join(slug)
        .join(task)
        .join(format!("g{generation}-c{digest}"));
    let manifest_rel = root.join(format!("{phase}.snapshot.tsv"));
    let manifest_binding = ValidatedRepoPath::new(
        repo_root,
        &manifest_rel,
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .map_err(|e| format!("manifest validation failed: {e}"))?;
    if !manifest_binding.exists() {
        return Err("snapshot manifest is missing".to_string());
    }
    manifest_binding
        .recheck()
        .map_err(|e| format!("manifest identity changed: {e}"))?;
    let manifest_text = String::from_utf8(
        std::fs::read(manifest_binding.full_path())
            .map_err(|e| format!("cannot read snapshot manifest: {e}"))?,
    )
    .map_err(|_| "snapshot manifest is not UTF-8".to_string())?;
    manifest_binding
        .recheck()
        .map_err(|e| format!("manifest identity changed while reading: {e}"))?;
    let entries = test_first_fs::parse_snapshot_tsv(&manifest_text)?;
    let expected: HashSet<String> = paths.iter().map(|p| p.replace('\\', "/")).collect();
    if entries.len() != expected.len()
        || entries.iter().any(|entry| !expected.contains(&entry.path))
    {
        return Err("snapshot manifest path set does not match restore allowlist".to_string());
    }
    let mut members = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.state == "missing" {
            if entry.kind != "-" || entry.mode != "-" || entry.sha256_or_dash != "-" {
                return Err(format!("invalid missing snapshot row for '{}'", entry.path));
            }
            members.push(RestoreMember { entry, bytes: None });
            continue;
        }
        if entry.state != "present"
            || entry.kind != "file"
            || entry.sha256_or_dash.len() != 64
            || !entry
                .sha256_or_dash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(format!("invalid present snapshot row for '{}'", entry.path));
        }
        let blob_rel = root
            .join("blobs")
            .join(format!("{}.bin", entry.sha256_or_dash));
        let blob_binding = ValidatedRepoPath::new(
            repo_root,
            &blob_rel,
            ValidatedRepoPathMode::RegularFileOrMissing,
        )
        .map_err(|e| format!("blob validation failed for '{}': {e}", entry.path))?;
        if !blob_binding.exists() {
            return Err(format!("snapshot blob is missing for '{}'", entry.path));
        }
        blob_binding
            .recheck()
            .map_err(|e| format!("blob identity changed for '{}': {e}", entry.path))?;
        let bytes = std::fs::read(blob_binding.full_path())
            .map_err(|e| format!("cannot read snapshot blob for '{}': {e}", entry.path))?;
        blob_binding
            .recheck()
            .map_err(|e| format!("blob identity changed while reading '{}': {e}", entry.path))?;
        if format!("{:x}", Sha256::digest(&bytes)) != entry.sha256_or_dash {
            return Err(format!(
                "snapshot blob digest mismatch for '{}'",
                entry.path
            ));
        }
        if entry.mode.is_empty() || entry.mode == "-" {
            return Err(format!("invalid file mode for '{}'", entry.path));
        }
        members.push(RestoreMember {
            entry,
            bytes: Some(bytes),
        });
    }
    Ok(members)
}

fn current_member(repo_root: &Path, path: &str) -> Result<RestoreMember, String> {
    let binding = ValidatedRepoPath::new(
        repo_root,
        Path::new(path),
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .map_err(|e| format!("current member validation failed for '{path}': {e}"))?;
    if !binding.exists() {
        return Ok(RestoreMember {
            entry: test_first_fs::SnapshotEntry {
                state: "missing".into(),
                kind: "-".into(),
                mode: "-".into(),
                sha256_or_dash: "-".into(),
                path: path.into(),
            },
            bytes: None,
        });
    }
    binding
        .recheck()
        .map_err(|e| format!("current member identity changed for '{path}': {e}"))?;
    let bytes = std::fs::read(binding.full_path())
        .map_err(|e| format!("cannot read current member '{path}': {e}"))?;
    binding
        .recheck()
        .map_err(|e| format!("current member identity changed while reading '{path}': {e}"))?;
    let metadata = std::fs::symlink_metadata(binding.full_path())
        .map_err(|e| format!("cannot read current member metadata '{path}': {e}"))?;
    Ok(RestoreMember {
        entry: test_first_fs::SnapshotEntry {
            state: "present".into(),
            kind: "file".into(),
            mode: restore_mode_string(&metadata),
            sha256_or_dash: format!("{:x}", Sha256::digest(&bytes)),
            path: path.into(),
        },
        bytes: Some(bytes),
    })
}

fn restore_mode_string(metadata: &std::fs::Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        format!("{:o}", metadata.permissions().mode())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        "100644".to_string()
    }
}

fn restore_file_mode(path: &Path, mode: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let value = u32::from_str_radix(mode, 8)
            .map_err(|_| format!("invalid file mode '{mode}' for '{}'", path.display()))?;
        let mut permissions = std::fs::metadata(path)
            .map_err(|e| format!("cannot read mode for '{}': {e}", path.display()))?
            .permissions();
        permissions.set_mode(value);
        std::fs::set_permissions(path, permissions)
            .map_err(|e| format!("cannot set mode for '{}': {e}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

fn restore_checkpoint(label: &str, index: usize) -> Result<(), String> {
    let configured = std::env::var("GAL_TEST_FIRST_RESTORE_FAIL_AT")
        .or_else(|_| std::env::var("GAL_TEST_FIRST_RESTORE_INTERRUPT_AT"))
        .ok();
    if configured
        .as_deref()
        .is_some_and(|value| value == label || value == index.to_string())
    {
        return Err(format!("injected restore interruption at {label}"));
    }
    Ok(())
}

fn restore_matches(repo_root: &Path, members: &[RestoreMember]) -> Result<(), String> {
    for member in members {
        let current = current_member(repo_root, &member.entry.path)?;
        let current_mode = current.entry.mode.clone();
        match (&member.bytes, current.bytes) {
            (None, None) => {}
            (Some(expected), Some(actual))
                if expected == &actual && current_mode == member.entry.mode => {}
            _ => {
                return Err(format!(
                    "restored baseline mismatch for '{}'",
                    member.entry.path
                ))
            }
        }
    }
    Ok(())
}

fn apply_member_internal(
    repo_root: &Path,
    member: &RestoreMember,
    index: usize,
    inject_fault: bool,
) -> Result<(), String> {
    let path = Path::new(&member.entry.path);
    let current =
        ValidatedRepoPath::new(repo_root, path, ValidatedRepoPathMode::RegularFileOrMissing)
            .map_err(|e| format!("apply validation failed for '{}': {e}", member.entry.path))?;
    if inject_fault {
        restore_checkpoint("before-apply", index)?;
    }
    match &member.bytes {
        None => {
            if current.exists() {
                let removal = current.bind_removal().map_err(|e| {
                    format!("remove binding failed for '{}': {e}", member.entry.path)
                })?;
                std::fs::remove_file(removal.unlink_path())
                    .map_err(|e| format!("cannot remove '{}': {e}", member.entry.path))?;
                if inject_fault {
                    restore_checkpoint("after-delete", index)?;
                }
                removal.verify().map_err(|e| {
                    format!("remove recheck failed for '{}': {e}", member.entry.path)
                })?;
            }
        }
        Some(bytes) => {
            let parent = path
                .parent()
                .ok_or_else(|| "restore member has no parent".to_string())?;
            let candidate = parent.join(format!(".gal-restore-{}-{index}", std::process::id()));
            let _ = std::fs::remove_file(repo_root.join(&candidate));
            test_first_fs::atomic_publish_bytes(&repo_root.join(&candidate), bytes)?;
            let candidate_binding = ValidatedRepoPath::new(
                repo_root,
                &candidate,
                ValidatedRepoPathMode::RegularFileOrMissing,
            )
            .map_err(|e| {
                format!(
                    "candidate validation failed for '{}': {e}",
                    member.entry.path
                )
            })?;
            if current.exists() {
                let replacement = current
                    .bind_replacement_candidate(candidate_binding)
                    .map_err(|e| {
                        format!(
                            "replacement binding failed for '{}': {e}",
                            member.entry.path
                        )
                    })?;
                std::fs::remove_file(replacement.target_path())
                    .map_err(|e| format!("cannot remove old '{}': {e}", member.entry.path))?;
                if inject_fault {
                    restore_checkpoint("after-delete", index)?;
                }
                std::fs::rename(replacement.candidate_path(), replacement.target_path())
                    .map_err(|e| format!("cannot replace '{}': {e}", member.entry.path))?;
                if inject_fault {
                    restore_checkpoint("after-replace", index)?;
                }
                replacement.verify().map_err(|e| {
                    format!(
                        "replacement recheck failed for '{}': {e}",
                        member.entry.path
                    )
                })?;
                restore_file_mode(repo_root.join(path).as_path(), &member.entry.mode)?;
            } else {
                std::fs::rename(repo_root.join(&candidate), repo_root.join(path))
                    .map_err(|e| format!("cannot create '{}': {e}", member.entry.path))?;
                if inject_fault {
                    restore_checkpoint("after-create", index)?;
                }
                current.bind_created_leaf().map_err(|e| {
                    format!("creation recheck failed for '{}': {e}", member.entry.path)
                })?;
                restore_file_mode(repo_root.join(path).as_path(), &member.entry.mode)?;
            }
        }
    }
    if inject_fault {
        restore_checkpoint("after-recheck", index)?;
    }
    Ok(())
}

fn apply_member(repo_root: &Path, member: &RestoreMember, index: usize) -> Result<(), String> {
    apply_member_internal(repo_root, member, index, true)
}

fn write_restore_journal(
    repo_root: &Path,
    snapshot_root: &Path,
    current: &[RestoreMember],
) -> Result<(), String> {
    let restore_root = snapshot_root.join("restore");
    std::fs::create_dir_all(repo_root.join(&restore_root).join("blobs"))
        .map_err(|e| format!("cannot create restore journal directory: {e}"))?;
    let mut entries = Vec::with_capacity(current.len());
    for member in current {
        let mut entry = member.entry.clone();
        if let Some(bytes) = &member.bytes {
            entry.sha256_or_dash = format!("{:x}", Sha256::digest(bytes));
            test_first_fs::atomic_publish_bytes(
                &repo_root
                    .join(&restore_root)
                    .join("blobs")
                    .join(format!("{}.bin", entry.sha256_or_dash)),
                bytes,
            )?;
        }
        entries.push(entry);
    }
    test_first_fs::atomic_publish_bytes(
        &repo_root.join(&restore_root).join("restore.journal.tsv"),
        test_first_fs::format_snapshot_tsv(&entries).as_bytes(),
    )
}

fn write_restore_receipt(path: &Path, verdict: &str, summary: &str) -> Result<(), String> {
    if !matches!(verdict, "restored" | "rolled-back" | "rollback-unconfirmed") {
        return Err(format!("invalid restore verdict '{verdict}'"));
    }
    let content = format!("verdict={verdict}\nsummary={summary}\n");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create receipt directory: {e}"))?;
    }
    test_first_fs::atomic_publish_bytes(path, content.as_bytes())
}

fn restore_snapshot(
    repo_root: &Path,
    receipt: &Path,
    prompt: &Path,
    task: &str,
    generation: u32,
    phase: &str,
) -> ExitCode {
    let fail = |verdict: &str, message: String, receipt: &Path| {
        let _ = write_restore_receipt(receipt, verdict, &message);
        eprintln!("gal boundary-check: restore {verdict}: {message}");
        if verdict == "restored" {
            ExitCode::Success
        } else {
            ExitCode::Error
        }
    };
    let prompt_text = match std::fs::read_to_string(prompt) {
        Ok(text) => text,
        Err(e) => {
            return fail(
                "rollback-unconfirmed",
                format!("cannot read prompt: {e}"),
                receipt,
            )
        }
    };
    if !has_test_first_marker(&prompt_text) {
        return fail(
            "rolled-back",
            "markerless prompt cannot restore snapshots".into(),
            receipt,
        );
    }
    let slug = derive_plan_slug(prompt);
    let contract = match parse_task_contract(&prompt_text, &slug, task) {
        Ok(contract) if contract.applicability == Applicability::Required => contract,
        Ok(_) => {
            return fail(
                "rolled-back",
                "not-applicable task cannot restore snapshots".into(),
                receipt,
            )
        }
        Err(e) => return fail("rolled-back", format!("task contract: {e}"), receipt),
    };
    let paths = match restore_paths(&contract, phase) {
        Ok(paths) => paths,
        Err(e) => return fail("rolled-back", e, receipt),
    };
    let digest = contract.compute_digest();
    let target = match restore_snapshot_members(
        repo_root, &slug, task, generation, &digest, phase, &paths,
    ) {
        Ok(members) => members,
        Err(e) => return fail("rolled-back", e, receipt),
    };
    let current = match paths
        .iter()
        .map(|path| current_member(repo_root, path))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(current) => current,
        Err(e) => return fail("rollback-unconfirmed", e, receipt),
    };
    let snapshot_root = PathBuf::from(".dev")
        .join("pipeline")
        .join("snapshots")
        .join(&slug)
        .join(task)
        .join(format!("g{generation}-c{digest}"));
    if let Err(e) = write_restore_journal(repo_root, &snapshot_root, &current) {
        return fail("rolled-back", e, receipt);
    }

    let mut applied = Vec::new();
    for (index, member) in target.iter().enumerate() {
        if let Err(error) = apply_member(repo_root, member, index) {
            let mut rollback_error = None;
            for (rollback_index, previous) in current[..=index].iter().rev().enumerate() {
                if let Err(e) = apply_member_internal(repo_root, previous, rollback_index, false) {
                    rollback_error = Some(e);
                    break;
                }
            }
            if let Some(rollback_error) = rollback_error {
                return fail(
                    "rollback-unconfirmed",
                    format!("apply failed: {error}; rollback failed: {rollback_error}"),
                    receipt,
                );
            }
            return fail("rolled-back", format!("apply failed: {error}"), receipt);
        }
        applied.push(current[applied.len()].clone());
    }
    if let Err(e) = restore_matches(repo_root, &target) {
        let mut rollback_error = None;
        for (index, previous) in applied.iter().rev().enumerate() {
            if let Err(error) = apply_member_internal(repo_root, previous, index, false) {
                rollback_error = Some(error);
                break;
            }
        }
        return if let Some(rollback_error) = rollback_error {
            fail(
                "rollback-unconfirmed",
                format!("baseline verification failed: {e}; rollback failed: {rollback_error}"),
                receipt,
            )
        } else {
            fail(
                "rolled-back",
                format!("baseline verification failed: {e}"),
                receipt,
            )
        };
    }
    fail(
        "restored",
        format!("restored {} member(s)", target.len()),
        receipt,
    )
}

/// Explicit orchestrator entry point. Verification never calls this function.
pub(crate) fn capture_snapshot_for_boundary(
    prompt_path: &Path,
    task: &str,
    generation: u32,
    phase: BoundaryKind,
) -> Result<PathBuf, String> {
    let prompt_text =
        std::fs::read_to_string(prompt_path).map_err(|e| format!("cannot read prompt: {e}"))?;
    if !has_test_first_marker(&prompt_text) {
        return Err("markerless prompt cannot capture snapshots".to_string());
    }
    let contract = parse_task_contract(
        &prompt_text,
        &extract_plan_slug(&prompt_path.to_string_lossy()),
        task,
    )
    .map_err(|e| format!("task contract: {e}"))?;
    if contract.applicability != Applicability::Required {
        return Err("not-applicable task cannot capture snapshots".to_string());
    }
    let phase_name = phase.as_str();
    capture_snapshot_for_phase(prompt_path, task, generation, phase_name, &contract)
}

fn capture_snapshot_for_phase(
    prompt_path: &Path,
    task: &str,
    generation: u32,
    phase_name: &str,
    contract: &pipeline::task_spec::TaskContract,
) -> Result<PathBuf, String> {
    let repo_root =
        std::env::current_dir().map_err(|e| format!("cannot resolve repo root: {e}"))?;
    let slug = derive_plan_slug(prompt_path);
    let mut paths = contract.production_paths.clone();
    if phase_name != "implementation-commit" && phase_name != "pre-implement" {
        paths.extend(contract.test_paths.clone());
        paths.sort();
        paths.dedup();
    }
    test_first_fs::capture_snapshot_manifest_and_blobs(
        &repo_root,
        &slug,
        task,
        generation,
        &contract.compute_digest(),
        phase_name,
        &paths,
    )
}

/// Resolve the nominated task's affected-files allowlist as a set of bare path
/// tokens, reusing the shared `pipeline::task_spec` parser.
fn parse_affected_files(task: &str, prompt_text: &str) -> Option<HashSet<String>> {
    let paths = pipeline::task_spec::extract_affected_file_paths(prompt_text, task);
    if paths.is_empty() {
        None
    } else {
        Some(paths.into_iter().collect())
    }
}

/// Get changed files from `git diff --name-only` (staged + unstaged) and
/// untracked files from `git status --porcelain` in `repo_root`.
fn changed_files(repo_root: &Path) -> Result<Vec<String>, String> {
    let mut files = std::collections::BTreeSet::new();

    let diff_out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("diff")
        .arg("--name-only")
        .arg("HEAD")
        .output()
        .map_err(|e| format!("failed to execute git diff: {e}"))?;

    if !diff_out.status.success() {
        return Err(format!(
            "git diff failed with exit code {:?}: {}",
            diff_out.status.code(),
            String::from_utf8_lossy(&diff_out.stderr).trim()
        ));
    }

    for line in String::from_utf8_lossy(&diff_out.stdout).lines() {
        let p = line.trim();
        if !p.is_empty() {
            files.insert(p.to_string());
        }
    }

    let status_out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("status")
        .arg("--porcelain")
        .arg("--untracked-files=all")
        .output()
        .map_err(|e| format!("failed to execute git status: {e}"))?;

    if !status_out.status.success() {
        return Err(format!(
            "git status failed with exit code {:?}: {}",
            status_out.status.code(),
            String::from_utf8_lossy(&status_out.stderr).trim()
        ));
    }

    for line in String::from_utf8_lossy(&status_out.stdout).lines() {
        let rest = if line.len() > 3 {
            line[3..].trim()
        } else {
            continue;
        };
        let path = if let Some(arrow) = rest.find(" -> ") {
            rest[arrow + 4..].trim()
        } else {
            rest
        };
        if !path.is_empty() {
            files.insert(path.to_string());
        }
    }

    Ok(files.into_iter().collect())
}

/// Compare actual changed files against the allowlist.
fn check_boundary_with_options(
    task: &str,
    prompt_text: &str,
    repo_root: &Path,
    prompt_path: &Path,
    boundary_kind: BoundaryKind,
) -> CheckOutcome {
    let name = "boundary-allowlist".to_string();
    let slug = derive_plan_slug(prompt_path);

    let allowlist = match parse_affected_files(task, prompt_text) {
        None => {
            return CheckOutcome {
                name,
                command: None,
                state: CheckState::NotRun,
                summary: format!(
                    "{task}: no affected files named (no task-block backtick paths, no `## Files to Create or Modify` entries) — refusing to allow-all"
                ),
            };
        }
        Some(set) => set,
    };

    let changed = match changed_files(repo_root) {
        Ok(c) => c,
        Err(err) => {
            return CheckOutcome {
                name,
                command: Some("git diff --name-only HEAD + git status --porcelain".to_string()),
                state: CheckState::Fail,
                summary: format!("git uncertainty: {err}"),
            };
        }
    };

    if changed.is_empty() {
        return CheckOutcome {
            name,
            command: Some("git diff --name-only HEAD + git status --porcelain".to_string()),
            state: CheckState::Pass,
            summary: "no changed files — nothing to check".to_string(),
        };
    }

    let mut violations: Vec<String> = changed
        .iter()
        .filter(|f| {
            !is_workflow_state_path_for_kind(&slug, f, boundary_kind)
                && !allowlist_contains(&allowlist, f)
        })
        .cloned()
        .collect();
    violations.sort();

    if violations.is_empty() {
        CheckOutcome {
            name,
            command: Some("git diff --name-only HEAD + git status --porcelain".to_string()),
            state: CheckState::Pass,
            summary: format!(
                "all {} changed file(s) are within the declared allowlist",
                changed.len()
            ),
        }
    } else {
        CheckOutcome {
            name,
            command: Some("git diff --name-only HEAD + git status --porcelain".to_string()),
            state: CheckState::Fail,
            summary: format!("out-of-allowlist: {}", violations.join(", ")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MarkedDigestResult {
    Match {
        current_prompt_digest: String,
        matched_committed_journal_digest: String,
    },
    DigestMismatch {
        current_prompt_digest: String,
        matched_committed_journal_digest: String,
    },
    Prepared {
        current_prompt_digest: String,
        prepared_digest: String,
    },
    Missing {
        current_prompt_digest: String,
    },
    Ambiguous {
        current_prompt_digest: String,
        error: String,
    },
    Unbound {
        current_prompt_digest: String,
    },
}

#[allow(dead_code)]
impl MarkedDigestResult {
    /// Whether this result is an affirmative digest-binding pass. `Unbound` is
    /// deliberately excluded: an unmarked prompt has no binding to verify, so
    /// answering "yes" here would let marker removal read as an affirmative
    /// pass rather than a skipped check.
    pub(crate) fn is_pass(&self) -> bool {
        matches!(self, Self::Match { .. })
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Match { .. } => "match",
            Self::DigestMismatch { .. } => "digest-mismatch",
            Self::Prepared { .. } => "prepared",
            Self::Missing { .. } => "missing",
            Self::Ambiguous { .. } => "ambiguous",
            Self::Unbound { .. } => "unbound",
        }
    }

    pub(crate) fn current_prompt_digest(&self) -> &str {
        match self {
            Self::Match {
                current_prompt_digest,
                ..
            }
            | Self::DigestMismatch {
                current_prompt_digest,
                ..
            }
            | Self::Prepared {
                current_prompt_digest,
                ..
            }
            | Self::Missing {
                current_prompt_digest,
            }
            | Self::Ambiguous {
                current_prompt_digest,
                ..
            }
            | Self::Unbound {
                current_prompt_digest,
            } => current_prompt_digest,
        }
    }

    pub(crate) fn matched_committed_journal_digest(&self) -> &str {
        match self {
            Self::Match {
                matched_committed_journal_digest,
                ..
            }
            | Self::DigestMismatch {
                matched_committed_journal_digest,
                ..
            } => matched_committed_journal_digest,
            _ => "-",
        }
    }
}

/// Reusable marked-prompt digest evaluator over prompt path and repository journal state.
pub(crate) fn evaluate_marked_digest(
    prompt_text: &str,
    repo_root: &Path,
    prompt_path: &Path,
) -> MarkedDigestResult {
    let slug = derive_plan_slug(prompt_path);
    evaluate_marked_digest_for_slug(prompt_text, repo_root, &slug)
}

/// Reusable marked-prompt digest evaluator over plan slug and repository journal state.
pub(crate) fn evaluate_marked_digest_for_slug(
    prompt_text: &str,
    repo_root: &Path,
    slug: &str,
) -> MarkedDigestResult {
    let prompt_bytes = prompt_text.as_bytes();
    let current_prompt_digest = format!("{:x}", Sha256::digest(prompt_bytes));

    if !has_test_first_marker(prompt_text) {
        return MarkedDigestResult::Unbound {
            current_prompt_digest,
        };
    }

    let status = test_first_transition::read_latest_transition_record(repo_root, slug);
    match status {
        TransitionJournalStatus::Committed(rec) => {
            let matched_digest = rec.new_digest.clone();
            if current_prompt_digest == matched_digest {
                MarkedDigestResult::Match {
                    current_prompt_digest,
                    matched_committed_journal_digest: matched_digest,
                }
            } else {
                MarkedDigestResult::DigestMismatch {
                    current_prompt_digest,
                    matched_committed_journal_digest: matched_digest,
                }
            }
        }
        TransitionJournalStatus::Prepared(rec) => MarkedDigestResult::Prepared {
            current_prompt_digest,
            prepared_digest: rec.new_digest.clone(),
        },
        TransitionJournalStatus::Missing => MarkedDigestResult::Missing {
            current_prompt_digest,
        },
        TransitionJournalStatus::Ambiguous(error) => MarkedDigestResult::Ambiguous {
            current_prompt_digest,
            error,
        },
    }
}

struct DigestBindingResult {
    outcome: CheckOutcome,
    marker_state: String,
    current_prompt_digest: String,
    matched_committed_journal_digest: String,
    evaluation: MarkedDigestResult,
}

struct GitIntegrityResult {
    outcome: CheckOutcome,
    base_head: String,
}

fn check_digest_binding(
    prompt_text: &str,
    repo_root: &Path,
    prompt_path: &Path,
) -> DigestBindingResult {
    let name = "digest-binding".to_string();
    let evaluation = evaluate_marked_digest(prompt_text, repo_root, prompt_path);

    let (outcome, marker_state, current_prompt_digest, matched_committed_journal_digest) =
        match &evaluation {
            MarkedDigestResult::Match {
                current_prompt_digest,
                matched_committed_journal_digest,
            } => (
                CheckOutcome {
                    name,
                    command: None,
                    state: CheckState::Pass,
                    summary: format!(
                        "marked prompt SHA-256 {current_prompt_digest} matches committed transition record"
                    ),
                },
                "marked".to_string(),
                current_prompt_digest.clone(),
                matched_committed_journal_digest.clone(),
            ),
            MarkedDigestResult::DigestMismatch {
                current_prompt_digest,
                matched_committed_journal_digest,
            } => (
                CheckOutcome {
                    name,
                    command: None,
                    state: CheckState::Fail,
                    summary: format!(
                        "digest mismatch: marked prompt SHA-256 ({current_prompt_digest}) does not match committed transition record new_digest ({matched_committed_journal_digest})"
                    ),
                },
                "marked".to_string(),
                current_prompt_digest.clone(),
                matched_committed_journal_digest.clone(),
            ),
            MarkedDigestResult::Prepared {
                current_prompt_digest,
                prepared_digest,
            } => (
                CheckOutcome {
                    name,
                    command: None,
                    state: CheckState::Fail,
                    summary: format!("prepared transition record ({prepared_digest}) is non-pass"),
                },
                "marked".to_string(),
                current_prompt_digest.clone(),
                "-".to_string(),
            ),
            MarkedDigestResult::Missing {
                current_prompt_digest,
            } => (
                CheckOutcome {
                    name,
                    command: None,
                    state: CheckState::Fail,
                    summary: "marked prompt has no committed transition journal record".to_string(),
                },
                "marked".to_string(),
                current_prompt_digest.clone(),
                "-".to_string(),
            ),
            MarkedDigestResult::Ambiguous {
                current_prompt_digest,
                error,
            } => (
                CheckOutcome {
                    name,
                    command: None,
                    state: CheckState::Fail,
                    summary: format!("ambiguous transition journal record: {error}"),
                },
                "marked".to_string(),
                current_prompt_digest.clone(),
                "-".to_string(),
            ),
            MarkedDigestResult::Unbound {
                current_prompt_digest,
            } => (
                CheckOutcome {
                    name,
                    command: None,
                    state: CheckState::Pass,
                    summary: "markerless prompt: inherited verdict unchanged; no binding occurs and no journal is created".to_string(),
                },
                "markerless".to_string(),
                current_prompt_digest.clone(),
                "-".to_string(),
            ),
        };

    DigestBindingResult {
        outcome,
        marker_state,
        current_prompt_digest,
        matched_committed_journal_digest,
        evaluation,
    }
}

/// Fail on Git uncertainty — the repository must resolve a current `HEAD`.
///
/// This is deliberately *not* a working-tree-cleanliness check: boundary-check
/// is normally invoked precisely when in-allowlist changes exist (to compare
/// them against the allowlist), so a literal `git status --porcelain`-is-empty
/// requirement would fail every legitimate call. "Enforce clean entry" at task
/// start is the orchestrator's own pre-dispatch discipline (2b/2c HEAD-Drift
/// Discipline, `Task Base Commit` recorded before implementing), not a
/// re-verification this command performs — same ownership split the R19
/// HEAD/index-drift fixtures document for the commit gate.
fn check_git_integrity(repo_root: &Path) -> GitIntegrityResult {
    let name = "git-integrity".to_string();
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("HEAD")
        .output();

    match out {
        Ok(o) if o.status.success() => {
            let head = String::from_utf8_lossy(&o.stdout).trim().to_string();
            GitIntegrityResult {
                outcome: CheckOutcome {
                    name,
                    command: Some("git rev-parse HEAD".to_string()),
                    state: CheckState::Pass,
                    summary: format!("git repository accessible, HEAD resolves to {head}"),
                },
                base_head: head,
            }
        }
        _ => GitIntegrityResult {
            outcome: CheckOutcome {
                name,
                command: Some("git rev-parse HEAD".to_string()),
                state: CheckState::Fail,
                summary: "git uncertainty: HEAD does not resolve or repository check failed"
                    .to_string(),
            },
            base_head: "-".to_string(),
        },
    }
}

/// Derive the running plan's slug from its execution prompt path.
fn derive_plan_slug(prompt_path: &Path) -> String {
    let name = prompt_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if let Some(stripped) = name.strip_suffix(".prompt.md") {
        stripped.to_string()
    } else if let Some(stripped) = name.strip_suffix(".md") {
        stripped.to_string()
    } else {
        name.to_string()
    }
}

/// Check if file is an exempt workflow state surface for the given `BoundaryKind`.
fn is_workflow_state_path_for_kind(slug: &str, file: &str, kind: BoundaryKind) -> bool {
    let f = strip_leading_dot_slash(file).replace('\\', "/");
    if f == format!(".dev/plans/{slug}.md") || f == format!(".dev/plans/{slug}.prompt.md") {
        return true;
    }
    if f == format!(".dev/pipeline/journal/{slug}/transition.journal.tsv") {
        return true;
    }
    if kind == BoundaryKind::StateRecording && f == ".dev/state.md" {
        return true;
    }
    false
}

fn strip_leading_dot_slash(p: &str) -> &str {
    p.strip_prefix("./").unwrap_or(p)
}

fn allowlist_contains(allowlist: &HashSet<String>, file: &str) -> bool {
    let file = strip_leading_dot_slash(file);
    if allowlist.iter().any(|e| strip_leading_dot_slash(e) == file) {
        return true;
    }
    let file_name = file.rsplit('/').next().unwrap_or(file);
    for entry in allowlist {
        let entry = strip_leading_dot_slash(entry);
        if !entry.contains('/') && !entry.contains('\\') && entry == file_name {
            return true;
        }
        let prefix = entry.trim_end_matches('/');
        if file.starts_with(&format!("{prefix}/")) {
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn render_receipt(
    receipt: &Receipt,
    task: &str,
    boundary_kind: BoundaryKind,
    marker_state: &str,
    current_prompt_digest: &str,
    matched_committed_journal_digest: &str,
    base_head: &str,
    slug: &str,
    digest_result: &str,
) -> String {
    let mut out = "# boundary-check receipt\n\n".to_string();
    out.push_str(&format!("task: {task}\n"));
    out.push_str(&format!("boundary_kind: {}\n", boundary_kind.as_str()));
    out.push_str(&format!("marker_state: {marker_state}\n"));
    out.push_str(&format!("digest_result: {digest_result}\n"));
    out.push_str(&format!("current_prompt_digest: {current_prompt_digest}\n"));
    out.push_str(&format!(
        "matched_committed_journal_digest: {matched_committed_journal_digest}\n"
    ));
    out.push_str(&format!("base_head: {base_head}\n"));
    out.push_str(&format!(
        "exempt_paths: .dev/state.md ({}), .dev/plans/{slug}.md (accepted), .dev/plans/{slug}.prompt.md (accepted)\n\n",
        if boundary_kind == BoundaryKind::StateRecording {
            "accepted"
        } else {
            "rejected"
        }
    ));

    let overall = if receipt.checks.iter().any(|c| c.state == CheckState::NotRun) {
        "not-run"
    } else if receipt.passed() {
        "pass"
    } else {
        "fail"
    };

    out.push_str(&format!("overall: {overall}\n\n"));
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

/// Run `gal boundary-check`.
pub(crate) fn cmd_boundary_check(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let parsed = match parse_args(&rest) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("gal boundary-check: {e}");
            return ExitCode::Usage;
        }
    };

    if !parsed.prompt.exists() {
        eprintln!(
            "gal boundary-check: prompt not found: {}",
            parsed.prompt.display()
        );
        return ExitCode::Usage;
    }

    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let prompt_text = std::fs::read_to_string(&parsed.prompt).unwrap_or_default();
    let slug = derive_plan_slug(&parsed.prompt);

    if parsed.restore {
        if parsed.capture {
            eprintln!("gal boundary-check: --restore and --capture are mutually exclusive");
            return ExitCode::Usage;
        }
        let phase = match parsed.snapshot_phase.as_deref() {
            Some(phase) => phase,
            None => {
                eprintln!("gal boundary-check: --restore requires --phase");
                return ExitCode::Usage;
            }
        };
        let generation = match parsed.generation {
            Some(generation) => generation,
            None => {
                eprintln!("gal boundary-check: --restore requires --generation");
                return ExitCode::Usage;
            }
        };
        if !matches!(
            phase,
            "implementation-commit"
                | "pre-implement"
                | "state-recording"
                | "post-test"
                | "post-audit"
        ) {
            eprintln!("gal boundary-check: unknown snapshot phase '{phase}'");
            return ExitCode::Usage;
        }
        return restore_snapshot(
            &repo_root,
            &parsed.receipt,
            &parsed.prompt,
            &parsed.task,
            generation,
            phase,
        );
    }

    if parsed.capture {
        let phase = parsed
            .snapshot_phase
            .as_deref()
            .ok_or_else(|| "--capture requires --phase".to_string());
        let generation = parsed
            .generation
            .ok_or_else(|| "--capture requires --generation".to_string());
        match (phase, generation) {
            (Ok(phase), Ok(generation)) => {
                let prompt_text = match std::fs::read_to_string(&parsed.prompt) {
                    Ok(text) => text,
                    Err(e) => {
                        eprintln!("gal boundary-check: cannot read prompt: {e}");
                        return ExitCode::Error;
                    }
                };
                if !has_test_first_marker(&prompt_text) {
                    eprintln!("gal boundary-check: markerless prompt cannot capture snapshots");
                    return ExitCode::Error;
                }
                let plan_slug = extract_plan_slug(&parsed.prompt.to_string_lossy());
                if let Err(e) = test_first_fs::validate_artifact_identity(
                    &plan_slug,
                    &parsed.task,
                    generation,
                    "0000000000000000000000000000000000000000000000000000000000000000",
                ) {
                    eprintln!("gal boundary-check: snapshot capture failed: {e}");
                    return ExitCode::Error;
                }
                if !matches!(
                    phase,
                    "implementation-commit"
                        | "pre-implement"
                        | "state-recording"
                        | "post-test"
                        | "post-audit"
                ) {
                    eprintln!("gal boundary-check: snapshot capture failed: invalid artifact identity: unknown snapshot phase '{phase}'");
                    return ExitCode::Usage;
                }
                let contract = match parse_task_contract(&prompt_text, &plan_slug, &parsed.task) {
                    Ok(contract) if contract.applicability == Applicability::Required => contract,
                    Ok(_) => {
                        eprintln!(
                            "gal boundary-check: not-applicable task cannot capture snapshots"
                        );
                        return ExitCode::Error;
                    }
                    Err(e) => {
                        eprintln!("gal boundary-check: task contract: {e}");
                        return ExitCode::Error;
                    }
                };
                let captured = if phase == "pre-implement" {
                    capture_snapshot_for_phase(
                        &parsed.prompt,
                        &parsed.task,
                        generation,
                        phase,
                        &contract,
                    )
                } else {
                    match BoundaryKind::parse(phase) {
                        Ok(kind) => capture_snapshot_for_boundary(
                            &parsed.prompt,
                            &parsed.task,
                            generation,
                            kind,
                        ),
                        Err(e) => Err(e),
                    }
                };
                match captured {
                    Ok(path) => {
                        println!("gal boundary-check: captured snapshot {}", path.display());
                        return ExitCode::Success;
                    }
                    Err(e) => {
                        eprintln!("gal boundary-check: snapshot capture failed: {e}");
                        return ExitCode::Error;
                    }
                }
            }
            (Err(e), _) | (_, Err(e)) => {
                eprintln!("gal boundary-check: {e}");
                return ExitCode::Usage;
            }
        }
    }

    let allowlist_outcome = check_boundary_with_options(
        &parsed.task,
        &prompt_text,
        &repo_root,
        &parsed.prompt,
        parsed.boundary_kind,
    );

    let digest_binding = check_digest_binding(&prompt_text, &repo_root, &parsed.prompt);

    let git_integrity = check_git_integrity(&repo_root);

    let snapshot_verify = match (
        parsed.snapshot_phase.as_deref(),
        parsed.generation,
        has_test_first_marker(&prompt_text),
    ) {
        (Some(phase), Some(generation), true) => {
            let plan_slug = extract_plan_slug(&parsed.prompt.to_string_lossy());
            match parse_task_contract(&prompt_text, &plan_slug, &parsed.task) {
                Ok(contract) if contract.applicability == Applicability::Required => {
                    let mut paths = contract.production_paths.clone();
                    if phase != BoundaryKind::ImplementationCommit.as_str()
                        && phase != "pre-implement"
                    {
                        paths.extend(contract.test_paths.clone());
                        paths.sort();
                        paths.dedup();
                    }
                    match test_first_fs::verify_snapshot_manifest_and_blobs(
                        &repo_root,
                        &slug,
                        &parsed.task,
                        generation,
                        &contract.compute_digest(),
                        phase,
                        &paths,
                    ) {
                        Ok(_) => CheckOutcome {
                            name: "snapshot-verify".into(),
                            command: None,
                            state: CheckState::Pass,
                            summary: format!(
                                "verified generation {generation}, contract {}, phase {phase}",
                                contract.compute_digest()
                            ),
                        },
                        Err(e) => CheckOutcome {
                            name: "snapshot-verify".into(),
                            command: None,
                            state: CheckState::Fail,
                            summary: e,
                        },
                    }
                }
                Ok(_) => CheckOutcome {
                    name: "snapshot-verify".into(),
                    command: None,
                    state: CheckState::NotRun,
                    summary: "not-applicable task has no snapshot evidence".into(),
                },
                Err(e) => CheckOutcome {
                    name: "snapshot-verify".into(),
                    command: None,
                    state: CheckState::Fail,
                    summary: format!("task contract: {e}"),
                },
            }
        }
        _ => CheckOutcome {
            name: "snapshot-verify".into(),
            command: None,
            state: CheckState::Pass,
            summary: "snapshot verification not requested".into(),
        },
    };

    let checks = vec![
        allowlist_outcome,
        digest_binding.outcome,
        git_integrity.outcome,
        snapshot_verify,
    ];
    let receipt = Receipt { checks };

    if let Some(parent) = parsed.receipt.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!(
                "gal boundary-check: cannot create receipt dir {}: {e}",
                parent.display()
            );
            return ExitCode::Error;
        }
    }

    let content = render_receipt(
        &receipt,
        &parsed.task,
        parsed.boundary_kind,
        &digest_binding.marker_state,
        &digest_binding.current_prompt_digest,
        &digest_binding.matched_committed_journal_digest,
        &git_integrity.base_head,
        &slug,
        digest_binding.evaluation.as_str(),
    );
    if let Err(e) = std::fs::write(&parsed.receipt, &content) {
        eprintln!(
            "gal boundary-check: cannot write receipt {}: {e}",
            parsed.receipt.display()
        );
        return ExitCode::Error;
    }

    println!(
        "gal boundary-check: {} ({} check(s)) → {}",
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

    #[test]
    fn default_receipt_path_is_plan_scoped_and_explicit_override_wins() {
        let prompt = Path::new(".dev/plans/boundary-scope.prompt.md");
        let task = format!("T-{:02}", 3usize);
        assert_eq!(
            default_receipt_path(prompt, &task),
            resolve_receipt_path(Some(prompt), format!("{task}-boundary-check.receipt.md"))
        );
        let parsed = parse_args(&[
            prompt.display().to_string(),
            "--task".to_string(),
            task,
            "--receipt".to_string(),
            "custom/receipt.md".to_string(),
        ])
        .unwrap();
        assert_eq!(parsed.receipt, PathBuf::from("custom/receipt.md"));
    }

    fn template_prompt(task: &str) -> String {
        format!(
            "## Files to Create or Modify\n\n- `crates/cli/src/commands/boundary_check.rs`\n\n## Tasks\n\n- [ ] {task} — Do the work in `crates/cli/src/commands/boundary_check.rs`. Verify: green.\n"
        )
    }

    #[test]
    fn workflow_state_surfaces_are_exempt() {
        let slug = "some-plan";
        assert!(is_workflow_state_path_for_kind(
            slug,
            ".dev/state.md",
            BoundaryKind::StateRecording
        ));
        assert!(is_workflow_state_path_for_kind(
            slug,
            ".dev/plans/some-plan.md",
            BoundaryKind::StateRecording
        ));
        assert!(is_workflow_state_path_for_kind(
            slug,
            ".dev/plans/some-plan.prompt.md",
            BoundaryKind::StateRecording
        ));
        assert!(is_workflow_state_path_for_kind(
            slug,
            ".dev/pipeline/journal/some-plan/transition.journal.tsv",
            BoundaryKind::StateRecording
        ));
        assert!(is_workflow_state_path_for_kind(
            slug,
            ".dev/pipeline/journal/some-plan/transition.journal.tsv",
            BoundaryKind::PostTest
        ));
        assert!(is_workflow_state_path_for_kind(
            slug,
            ".dev/pipeline/journal/some-plan/transition.journal.tsv",
            BoundaryKind::PostAudit
        ));
        assert!(
            is_workflow_state_path_for_kind(slug, "./.dev/state.md", BoundaryKind::StateRecording),
            "leading ./"
        );
        assert!(
            is_workflow_state_path_for_kind(
                slug,
                ".dev\\plans\\some-plan.prompt.md",
                BoundaryKind::StateRecording
            ),
            "windows separators"
        );
        assert!(
            is_workflow_state_path_for_kind(
                slug,
                ".dev\\pipeline\\journal\\some-plan\\transition.journal.tsv",
                BoundaryKind::StateRecording
            ),
            "windows separators in journal path"
        );
    }

    #[test]
    fn state_md_not_exempt_for_implementation_commit_boundary() {
        let slug = "some-plan";
        assert!(!is_workflow_state_path_for_kind(
            slug,
            ".dev/state.md",
            BoundaryKind::ImplementationCommit
        ));
        assert!(is_workflow_state_path_for_kind(
            slug,
            ".dev/plans/some-plan.prompt.md",
            BoundaryKind::ImplementationCommit
        ));
    }

    #[test]
    fn exemption_does_not_cover_unrelated_paths() {
        let slug = "some-plan";
        assert!(!is_workflow_state_path_for_kind(
            slug,
            ".dev/project.md",
            BoundaryKind::StateRecording
        ));
        assert!(!is_workflow_state_path_for_kind(
            slug,
            ".dev/research/note.md",
            BoundaryKind::StateRecording
        ));
        assert!(!is_workflow_state_path_for_kind(
            slug,
            "crates/cli/src/main.rs",
            BoundaryKind::StateRecording
        ));
        assert!(!is_workflow_state_path_for_kind(
            slug,
            "docs/manual.md",
            BoundaryKind::StateRecording
        ));
        assert!(
            !is_workflow_state_path_for_kind(
                slug,
                "plugins/gal-core/agents/golem-releaser.agent.md",
                BoundaryKind::StateRecording
            ),
            "a Protected Path must never be silently exempt"
        );
        assert!(
            !is_workflow_state_path_for_kind(
                slug,
                "evil/.dev/plans/x.md",
                BoundaryKind::StateRecording
            ),
            "must anchor at the repo root, not match a nested lookalike"
        );
        assert!(
            !is_workflow_state_path_for_kind(
                slug,
                ".dev/plans/other-plan.md",
                BoundaryKind::StateRecording
            ),
            "a sibling plan's files must not be exempt"
        );
        assert!(
            !is_workflow_state_path_for_kind(
                slug,
                ".dev/pipeline/journal/other-plan/transition.journal.tsv",
                BoundaryKind::StateRecording
            ),
            "another slug's journal must not be exempt"
        );
        assert!(
            is_workflow_state_path_for_kind(
                slug,
                ".dev/pipeline/journal/some-plan/transition.journal.tsv",
                BoundaryKind::ImplementationCommit
            ),
            "journal is exempt at implementation-commit boundary"
        );
        assert!(
            !is_workflow_state_path_for_kind(
                slug,
                ".dev/pipeline/locks/test.lock",
                BoundaryKind::StateRecording
            ),
            "locks are not exempt"
        );
        assert!(
            !is_workflow_state_path_for_kind(
                slug,
                ".dev/pipeline/backups/test.bak",
                BoundaryKind::StateRecording
            ),
            "backups are not exempt"
        );
        assert!(
            !is_workflow_state_path_for_kind(
                slug,
                ".dev/pipeline/receipts/test.receipt.md",
                BoundaryKind::StateRecording
            ),
            "receipts are not exempt"
        );
        assert!(
            !is_workflow_state_path_for_kind(
                slug,
                ".dev/pipeline/snapshots/test.snap",
                BoundaryKind::StateRecording
            ),
            "snapshots are not exempt"
        );
        assert!(
            !is_workflow_state_path_for_kind(
                slug,
                ".dev/pipeline/cleanup/test.log",
                BoundaryKind::StateRecording
            ),
            "cleanup state is not exempt"
        );
        assert!(
            !is_workflow_state_path_for_kind(
                slug,
                ".dev/pipeline/quarantine/test.entry",
                BoundaryKind::StateRecording
            ),
            "quarantine paths are not exempt"
        );
    }

    #[test]
    fn derive_plan_slug_strips_prompt_md_before_md() {
        assert_eq!(
            derive_plan_slug(Path::new(".dev/plans/some-plan.prompt.md")),
            "some-plan"
        );
        assert_eq!(
            derive_plan_slug(Path::new(".dev/plans/some-plan.md")),
            "some-plan"
        );
    }

    #[test]
    fn missing_affected_files_section_is_not_run() {
        let task = format!("T-{}", 99usize);
        let prompt = "## Tasks\n\n- [ ] do something\n";
        let outcome = check_boundary_with_options(
            &task,
            prompt,
            std::path::Path::new("."),
            std::path::Path::new("plan.prompt.md"),
            BoundaryKind::StateRecording,
        );
        assert_eq!(
            outcome.state,
            CheckState::NotRun,
            "must be NotRun, not Pass"
        );
    }

    #[test]
    fn empty_affected_files_section_is_not_run() {
        let task = format!("T-{}", 99usize);
        let prompt = "## Affected Files\n\n## Tasks\n\n";
        let outcome = check_boundary_with_options(
            &task,
            prompt,
            std::path::Path::new("."),
            std::path::Path::new("plan.prompt.md"),
            BoundaryKind::StateRecording,
        );
        assert_eq!(
            outcome.state,
            CheckState::NotRun,
            "empty section must be NotRun"
        );
    }

    #[test]
    fn template_shape_prompt_resolves_nonempty_allowlist() {
        let task = format!("T-{}", 42usize);
        let set = parse_affected_files(&task, &template_prompt(&task))
            .expect("template-shape prompt must resolve a non-empty allowlist");
        assert!(allowlist_contains(
            &set,
            "crates/cli/src/commands/boundary_check.rs"
        ));
    }

    #[test]
    fn template_shape_prompt_rejects_out_of_allowlist_file() {
        let task = format!("T-{}", 42usize);
        let set =
            parse_affected_files(&task, &template_prompt(&task)).expect("non-empty allowlist");
        assert!(!allowlist_contains(&set, "docs/manual.md"));
    }

    #[test]
    fn template_task_with_no_paths_is_not_run() {
        let task = format!("T-{}", 42usize);
        let prompt = format!("## Tasks\n\n- [ ] {task} — A task naming no paths.\n");
        let outcome = check_boundary_with_options(
            &task,
            &prompt,
            std::path::Path::new("."),
            std::path::Path::new("plan.prompt.md"),
            BoundaryKind::StateRecording,
        );
        assert_eq!(
            outcome.state,
            CheckState::NotRun,
            "empty allowlist must be NotRun"
        );
    }

    #[test]
    fn changed_files_lists_individual_paths_inside_a_new_untracked_directory() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        let run_git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(repo_root)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        run_git(&["init", "-q"]);
        run_git(&["config", "user.email", "test@example.com"]);
        run_git(&["config", "user.name", "test"]);
        run_git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(repo_root.join("README.md"), "hi\n").unwrap();
        run_git(&["add", "README.md"]);
        run_git(&["commit", "-q", "-m", "init"]);

        std::fs::create_dir_all(repo_root.join("newdir")).unwrap();
        std::fs::write(repo_root.join("newdir").join("file.md"), "content\n").unwrap();

        let files = changed_files(repo_root).unwrap();
        assert!(
            files.contains(&"newdir/file.md".to_string()),
            "expected the individual file path, got {files:?}"
        );
        assert!(
            !files.iter().any(|f| f == "newdir/"),
            "must not collapse into the bare directory entry, got {files:?}"
        );
    }

    #[test]
    fn allowlist_prefix_covers_subtree() {
        let mut set = HashSet::new();
        set.insert("crates/cli/src/".to_string());
        assert!(allowlist_contains(&set, "crates/cli/src/main.rs"));
        assert!(!allowlist_contains(&set, "docs/manual.md"));
    }

    #[test]
    fn allowlist_root_level_dot_slash_entry_matches_bare_diff_path() {
        let mut set = HashSet::new();
        set.insert("./CLAUDE.md".to_string());
        assert!(allowlist_contains(&set, "CLAUDE.md"));
        assert!(!allowlist_contains(&set, "AGENTS.md"));
    }

    #[test]
    fn allowlist_basename_covers_matching_file_name() {
        let mut set = HashSet::new();
        set.insert("boundary_check.rs".to_string());
        assert!(allowlist_contains(
            &set,
            "crates/cli/src/commands/boundary_check.rs"
        ));
        assert!(!allowlist_contains(
            &set,
            "crates/cli/src/commands/converge_check.rs"
        ));
    }

    fn init_temp_git_repo(repo_root: &Path) {
        let run_git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(repo_root)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        run_git(&["init", "-q"]);
        run_git(&["config", "user.email", "test@example.com"]);
        run_git(&["config", "user.name", "test"]);
        run_git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(repo_root.join("README.md"), "hi\n").unwrap();
        run_git(&["add", "README.md"]);
        run_git(&["commit", "-q", "-m", "init"]);
    }

    #[test]
    fn changed_path_under_other_plan_is_violation() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        init_temp_git_repo(repo_root);

        let slug = format!("plan-{}", 3usize);
        let other_slug = format!("plan-{}", 4usize);
        let task = format!("T-{}", 3usize);

        std::fs::create_dir_all(repo_root.join(".dev/plans")).unwrap();
        std::fs::write(
            repo_root
                .join(".dev/plans")
                .join(format!("{other_slug}.md")),
            "other plan\n",
        )
        .unwrap();

        let prompt_text = template_prompt(&task);
        let prompt_path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));
        let outcome = check_boundary_with_options(
            &task,
            &prompt_text,
            repo_root,
            &prompt_path,
            BoundaryKind::StateRecording,
        );
        assert_eq!(outcome.state, CheckState::Fail, "{}", outcome.summary);
        assert!(
            outcome
                .summary
                .contains(&format!(".dev/plans/{other_slug}.md")),
            "expected sibling plan path in violation summary, got {}",
            outcome.summary
        );
    }

    #[test]
    fn active_plan_files_and_state_are_exempt() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        init_temp_git_repo(repo_root);

        let slug = format!("plan-{}", 7usize);
        let task = format!("T-{}", 7usize);

        std::fs::create_dir_all(repo_root.join(".dev/plans")).unwrap();
        std::fs::write(
            repo_root.join(".dev/plans").join(format!("{slug}.md")),
            "plan\n",
        )
        .unwrap();
        std::fs::write(
            repo_root
                .join(".dev/plans")
                .join(format!("{slug}.prompt.md")),
            "prompt\n",
        )
        .unwrap();
        std::fs::write(repo_root.join(".dev/state.md"), "state\n").unwrap();

        let prompt_text = template_prompt(&task);
        let prompt_path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));
        let outcome = check_boundary_with_options(
            &task,
            &prompt_text,
            repo_root,
            &prompt_path,
            BoundaryKind::StateRecording,
        );
        assert_eq!(outcome.state, CheckState::Pass, "{}", outcome.summary);
    }

    #[test]
    fn active_plan_en_md_is_not_exempt() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        init_temp_git_repo(repo_root);

        let slug = format!("plan-{}", 8usize);
        let task = format!("T-{}", 8usize);

        std::fs::create_dir_all(repo_root.join(".dev/plans")).unwrap();
        std::fs::write(
            repo_root.join(".dev/plans").join(format!("{slug}.en.md")),
            "translation\n",
        )
        .unwrap();

        let prompt_text = template_prompt(&task);
        let prompt_path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));
        let outcome = check_boundary_with_options(
            &task,
            &prompt_text,
            repo_root,
            &prompt_path,
            BoundaryKind::StateRecording,
        );
        assert_eq!(outcome.state, CheckState::Fail, "{}", outcome.summary);
        assert!(
            outcome.summary.contains(&format!("{slug}.en.md")),
            "expected the .en.md translation in violation summary, got {}",
            outcome.summary
        );
    }

    #[test]
    fn derive_plan_slug_prompt_md_equals_slug_not_slug_prompt() {
        let slug = format!("plan-{}", 11usize);
        let path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));
        let derived = derive_plan_slug(&path);
        assert_eq!(derived, slug);
        assert_ne!(derived, format!("{slug}.prompt"));
    }

    #[test]
    fn cmd_entry_returns_usage_for_missing_prompt() {
        let task = format!("T-{}", 99usize);
        let result = cmd_boundary_check(&[
            "boundary-check".to_string(),
            "nonexistent.prompt.md".to_string(),
            "--task".to_string(),
            task,
        ]);
        assert_eq!(result, ExitCode::Usage);
    }

    #[test]
    fn evaluate_marked_digest_matrix_states() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        let slug = "test-plan";
        let prompt_path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));

        let markerless_text = "## Tasks\n- [ ] do something\n";
        let eval_unbound = evaluate_marked_digest(markerless_text, repo_root, &prompt_path);
        assert_eq!(eval_unbound.as_str(), "unbound");
        // An unmarked prompt has nothing to bind, so this is a skipped check
        // rather than an affirmative pass.
        assert!(!eval_unbound.is_pass());

        let marked_text = "Pipeline Contract: test-first-v1\n\n## Tasks\n- [ ] do something\n";
        let expected_digest = format!("{:x}", Sha256::digest(marked_text.as_bytes()));

        let eval_missing = evaluate_marked_digest(marked_text, repo_root, &prompt_path);
        assert_eq!(eval_missing.as_str(), "missing");
        assert!(!eval_missing.is_pass());
        assert_eq!(eval_missing.current_prompt_digest(), expected_digest);

        let journal_dir = repo_root.join(".dev/pipeline/journal").join(slug);
        std::fs::create_dir_all(&journal_dir).unwrap();
        let journal_file = journal_dir.join("transition.journal.tsv");

        std::fs::write(
            &journal_file,
            format!("committed\tinit\t-\t{expected_digest}\n"),
        )
        .unwrap();
        let eval_match = evaluate_marked_digest(marked_text, repo_root, &prompt_path);
        assert_eq!(eval_match.as_str(), "match");
        assert!(eval_match.is_pass());
        assert_eq!(
            eval_match.matched_committed_journal_digest(),
            expected_digest
        );

        std::fs::write(
            &journal_file,
            "committed\tinit\t-\t0000000000000000000000000000000000000000000000000000000000000000\n",
        )
        .unwrap();
        let eval_mismatch = evaluate_marked_digest(marked_text, repo_root, &prompt_path);
        assert_eq!(eval_mismatch.as_str(), "digest-mismatch");
        assert!(!eval_mismatch.is_pass());

        std::fs::write(
            &journal_file,
            format!("prepared\tinit\t-\t{expected_digest}\n"),
        )
        .unwrap();
        let eval_prepared = evaluate_marked_digest(marked_text, repo_root, &prompt_path);
        assert_eq!(eval_prepared.as_str(), "prepared");
        assert!(!eval_prepared.is_pass());
    }

    #[test]
    fn capture_snapshot_for_phase_and_verify_accepts_same_file_task() {
        let slug = "same-file-snapshot-characterization";
        let task = format!("T-{}", 77usize);
        let prompt_path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));
        let shared_file = "src/commands/boundary_check.rs";
        let prompt_text = format!(
            "# Plan Prompt: {slug}\n\nPipeline Contract: test-first-v1\n\n## Tasks\n\n- [ ] {task} — Same file task.\n  - Test-first: required\n  - Seam: `{shared_file}`\n  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64=dGVzdA`\n  - Production Paths: `{shared_file}`\n  - Test Paths: `{shared_file}`\n  - Scaffold: not-required\n\n## Files to Create or Modify\n\n- `{shared_file}`\n"
        );

        let contract = parse_task_contract(&prompt_text, slug, &task)
            .expect("same-file task contract must parse successfully");
        assert_eq!(contract.production_paths, vec![shared_file.to_string()]);
        assert_eq!(contract.test_paths, vec![shared_file.to_string()]);

        let generation = 1u32;
        let phase_name = "post-test";

        let manifest_path =
            capture_snapshot_for_phase(&prompt_path, &task, generation, phase_name, &contract)
                .expect("capture_snapshot_for_phase must succeed for same-file task");

        let repo_root = std::env::current_dir().expect("must resolve repo root");
        let cleanup_snapshot_dir = repo_root.join(".dev/pipeline/snapshots").join(slug);

        let manifest_content = std::fs::read_to_string(&manifest_path)
            .expect("captured snapshot manifest must exist on disk");
        let entries = test_first_fs::parse_snapshot_tsv(&manifest_content)
            .expect("captured manifest must parse as valid snapshot tsv");

        // The sort() + dedup() in capture_snapshot_for_phase collapses the duplicate path into exactly 1 entry.
        assert_eq!(
            entries.len(),
            1,
            "captured snapshot path set must have exactly one entry after deduplication"
        );
        assert_eq!(entries[0].path, shared_file);
        assert_eq!(entries[0].state, "present");

        let mut expected_paths = contract.production_paths.clone();
        expected_paths.extend(contract.test_paths.clone());
        expected_paths.sort();
        expected_paths.dedup();
        assert_eq!(expected_paths.len(), 1);

        let verify_result = test_first_fs::verify_snapshot_manifest_and_blobs(
            &repo_root,
            slug,
            &task,
            generation,
            &contract.compute_digest(),
            phase_name,
            &expected_paths,
        );

        let _ = std::fs::remove_dir_all(&cleanup_snapshot_dir);

        assert!(
            verify_result.is_ok(),
            "verify_snapshot_manifest_and_blobs must accept same-file manifest: {:?}",
            verify_result.err()
        );
    }
}
