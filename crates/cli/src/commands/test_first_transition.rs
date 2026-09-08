//! `test_first_transition` — dormant crash-atomic prompt transitions.
//!
//! Owns the sole `TransitionBindingRecord` codec, parser, and producer.

use super::test_first_fs;
use fs2::FileExt;
use gal_foundation::validated_repo_path::{ValidatedRepoPath, ValidatedRepoPathMode};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Single record line in `transition.journal.tsv`.
/// Format: `state\tkind\told_digest\tnew_digest`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionBindingRecord {
    pub state: String,
    pub kind: String,
    pub old_digest: String,
    pub new_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionJournalStatus {
    Missing,
    Committed(TransitionBindingRecord),
    Prepared(TransitionBindingRecord),
    Ambiguous(String),
}

fn is_lower_hex_64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_valid_digest(s: &str, allow_dash: bool) -> bool {
    (allow_dash && s == "-") || is_lower_hex_64(s)
}

fn is_valid_kind(s: &str) -> bool {
    matches!(
        s,
        "init"
            | "legacy-bootstrap"
            | "phase-rerun"
            | "probe-defect"
            | "contract-change"
            | "implementation-defect"
    )
}

impl TransitionBindingRecord {
    /// Parse a single line from `transition.journal.tsv`.
    pub fn parse_line(line: &str) -> Result<Self, String> {
        if line.contains('\r') {
            return Err("line contains CR (carriage return)".to_string());
        }
        if line != line.trim_matches(|c| c == ' ' || c == '\t') {
            return Err("line contains leading or trailing whitespace".to_string());
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 4 {
            return Err(format!(
                "transition record requires exactly 4 fields, got {}: {line:?}",
                parts.len()
            ));
        }
        let state = parts[0];
        let kind = parts[1];
        let old_digest = parts[2];
        let new_digest = parts[3];

        if state != "committed" && state != "prepared" {
            return Err(format!("unknown transition record state '{state}'"));
        }
        if !is_valid_kind(kind) {
            return Err(format!("unknown transition record kind '{kind}'"));
        }
        if !is_valid_digest(old_digest, true) {
            return Err(format!("invalid old_digest domain '{old_digest}'"));
        }
        if !is_valid_digest(new_digest, false) {
            return Err(format!("invalid new_digest domain '{new_digest}'"));
        }

        Ok(Self {
            state: state.to_string(),
            kind: kind.to_string(),
            old_digest: old_digest.to_string(),
            new_digest: new_digest.to_string(),
        })
    }

    fn line(&self) -> String {
        format!(
            "{}	{}	{}	{}\n",
            self.state, self.kind, self.old_digest, self.new_digest
        )
    }
}

/// Read the latest transition record from `.dev/pipeline/journal/<slug>/transition.journal.tsv`.
pub fn read_latest_transition_record(repo_root: &Path, slug: &str) -> TransitionJournalStatus {
    let rel_journal_path = PathBuf::from(".dev")
        .join("pipeline")
        .join("journal")
        .join(slug)
        .join("transition.journal.tsv");

    let val_path = match ValidatedRepoPath::new(
        repo_root,
        &rel_journal_path,
        ValidatedRepoPathMode::RegularFileOrMissing,
    ) {
        Ok(vp) => vp,
        Err(e) => {
            return TransitionJournalStatus::Ambiguous(format!(
                "invalid transition journal path {}: {e}",
                rel_journal_path.display()
            ));
        }
    };

    if !val_path.exists() {
        return TransitionJournalStatus::Missing;
    }

    if let Err(e) = val_path.recheck() {
        return TransitionJournalStatus::Ambiguous(format!(
            "transition journal identity check failed {}: {e}",
            rel_journal_path.display()
        ));
    }

    let content = match fs::read_to_string(val_path.full_path()) {
        Ok(c) => c,
        Err(e) => {
            return TransitionJournalStatus::Ambiguous(format!(
                "cannot read transition journal {}: {e}",
                val_path.full_path().display()
            ))
        }
    };

    if content.contains('\r') {
        return TransitionJournalStatus::Ambiguous(
            "transition journal contains non-canonical CR bytes".to_string(),
        );
    }

    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

    if lines.is_empty() {
        return TransitionJournalStatus::Missing;
    }

    let mut records = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        match TransitionBindingRecord::parse_line(line) {
            Ok(rec) => records.push(rec),
            Err(e) => {
                return TransitionJournalStatus::Ambiguous(format!(
                    "line {}: invalid transition record: {e}",
                    idx + 1
                ))
            }
        }
    }

    if let Some(last) = records.last() {
        if last.state == "committed" {
            TransitionJournalStatus::Committed(last.clone())
        } else if last.state == "prepared" {
            TransitionJournalStatus::Prepared(last.clone())
        } else {
            TransitionJournalStatus::Ambiguous(format!("unhandled record state '{}'", last.state))
        }
    } else {
        TransitionJournalStatus::Missing
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn prompt_slug(prompt_path: &Path) -> Result<String, String> {
    let name = prompt_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "prompt path has no UTF-8 filename".to_string())?;
    name.strip_suffix(".prompt.md")
        .filter(|slug| !slug.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("prompt is not an execution prompt: {name}"))
}

fn journal_paths(
    repo_root: &Path,
    prompt_path: &Path,
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let slug = prompt_slug(prompt_path)?;
    let dir = repo_root.join(".dev/pipeline/journal").join(&slug);
    Ok((
        dir.join("transition.journal.tsv"),
        dir.join("transition.lock"),
        dir.join("transition.backup"),
    ))
}

fn ensure_prompt_binding(
    repo_root: &Path,
    prompt_path: &Path,
) -> Result<ValidatedRepoPath, String> {
    let rel = prompt_path
        .strip_prefix(repo_root)
        .map_err(|_| format!("prompt path escapes repository: {}", prompt_path.display()))?;
    ValidatedRepoPath::new(repo_root, rel, ValidatedRepoPathMode::RegularFileOrMissing)
        .map_err(|e| format!("prompt path validation failed: {e}"))
}

/// Delegates to the one real implementation, which fsyncs on Unix and, on
/// Windows, opens the directory handle and tolerates the `ERROR_ACCESS_DENIED`
/// that Win32 returns for a directory flush by design. The local copy this
/// replaces did nothing at all off Unix, so this restores the Unix durability
/// barrier and turns the Windows path into an openability check rather than
/// silence. Windows still has no directory flush available.
fn sync_parent(path: &Path) -> Result<(), String> {
    super::test_first_fs::sync_dir(path)
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

#[cfg(windows)]
fn replace_file(candidate: &Path, target: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING};
    let source: Vec<u16> = candidate.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both strings are NUL-terminated UTF-16 paths owned for this call.
    let ok = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING,
        )
    };
    if ok == 0 {
        Err(format!(
            "native atomic replacement failed: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(candidate: &Path, target: &Path) -> Result<(), String> {
    fs::rename(candidate, target).map_err(|e| format!("atomic replacement failed: {e}"))
}

/// Durably publish an internal control file (journal, lock, backup) that carries no
/// repo-identity guarantee of its own. Reuses the shared atomic-publication primitive
/// rather than re-implementing candidate-write-then-rename here.
fn write_control_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "cannot create transition directory {}: {e}",
                parent.display()
            )
        })?;
    }
    test_first_fs::atomic_publish_bytes(path, bytes)
}

/// Durably publish the prompt's own bytes through the shared consuming transitions,
/// closing the TOCTOU window between validation and the native atomic replace.
fn publish_prompt_bytes(
    repo_root: &Path,
    prompt: ValidatedRepoPath,
    new_bytes: &[u8],
) -> Result<(), String> {
    // Build the candidate's path from the prompt's validated repo-relative path, not
    // from `prompt.full_path()`: on Windows that full path is canonicalized (extended-
    // length `\\?\...`) while `repo_root` is caller-supplied and raw, so a naive
    // `strip_prefix(repo_root)` against it fails even though the candidate is well
    // inside the repository. Joining `repo_root` with the relative path keeps both the
    // filesystem write and the re-validation in the same raw path domain.
    let prompt_rel = prompt.rel_path().to_path_buf();
    let file_name = prompt_rel
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "prompt path has no UTF-8 filename".to_string())?;
    let parent_rel = prompt_rel.parent().unwrap_or_else(|| Path::new(""));
    let candidate_rel = parent_rel.join(format!(
        "{}{}",
        candidate_prefix(file_name),
        unique_suffix()
    ));
    let candidate_path = repo_root.join(&candidate_rel);
    let full_path = repo_root.join(&prompt_rel);
    let parent = full_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("prompt path has no parent: {}", full_path.display()))?;

    let publish_result = test_first_fs::atomic_publish_bytes(&candidate_path, new_bytes);
    if let Err(e) = publish_result {
        let _ = fs::remove_file(&candidate_path);
        return Err(e);
    }
    let candidate = ValidatedRepoPath::new(
        repo_root,
        &candidate_rel,
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .map_err(|e| format!("candidate validation failed: {e}"));
    let result = (|| {
        let candidate = candidate?;
        // Consume the same `prompt` binding that observed the pre-write state across
        // the actual replace: `bind_created_leaf`/`bind_replacement_candidate` require
        // the identity captured at validation time, not a freshly reconstructed one
        // taken after the file already changed underneath it.
        let existed_before = prompt.exists();
        if existed_before {
            let replacement = prompt
                .bind_replacement_candidate(candidate)
                .map_err(|e| format!("replacement binding failed: {e}"))?;
            replace_file(replacement.candidate_path(), replacement.target_path())?;
            sync_parent(&parent)?;
            replacement
                .verify()
                .map_err(|e| format!("replacement recheck failed: {e}"))?;
        } else {
            replace_file(&candidate_path, &full_path)?;
            sync_parent(&parent)?;
            prompt
                .bind_created_leaf()
                .map_err(|e| format!("creation recheck failed: {e}"))?;
        }
        if fs::read(&full_path).map_err(|e| format!("durable readback failed: {e}"))? != new_bytes {
            return Err("durable readback bytes differ".to_string());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&candidate_path);
    }
    result
}

fn journal_bytes(
    record: &TransitionBindingRecord,
    prior: Option<&TransitionBindingRecord>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Some(prior) = prior {
        bytes.extend_from_slice(prior.line().as_bytes());
    }
    bytes.extend_from_slice(record.line().as_bytes());
    bytes
}

/// Filename prefix every candidate for `file_name` is published under. One
/// definition so the writer and the orphan scanner cannot drift: the guard
/// below used to scan the journal directory, where a candidate is never
/// written, so it could not fire at all.
fn candidate_prefix(file_name: &str) -> String {
    format!(".{file_name}.candidate.")
}

/// Reject a leftover candidate for *this* prompt. Candidates are published
/// beside the prompt, so that is the directory to read. The prefix match keeps
/// the scan plan-scoped: `.dev/plans/` holds every plan's prompt, and one
/// plan's orphan must not block another plan's transition.
fn scan_for_orphan_candidate(prompt_path: &Path) -> Result<(), String> {
    let Some(file_name) = prompt_path.file_name().and_then(|n| n.to_str()) else {
        return Err("prompt path has no UTF-8 filename".to_string());
    };
    let Some(parent) = prompt_path.parent() else {
        return Err(format!(
            "prompt path has no parent: {}",
            prompt_path.display()
        ));
    };
    if !parent.exists() {
        return Ok(());
    }
    let prefix = candidate_prefix(file_name);
    for entry in
        fs::read_dir(parent).map_err(|e| format!("cannot inspect transition artifacts: {e}"))?
    {
        let entry = entry.map_err(|e| format!("cannot inspect transition artifact: {e}"))?;
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            return Err(format!(
                "ambiguous transition candidate exists: {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn recover_locked(repo_root: &Path, prompt_path: &Path) -> Result<(), String> {
    let (journal, _, backup) = journal_paths(repo_root, prompt_path)?;
    scan_for_orphan_candidate(prompt_path)?;
    match read_latest_transition_record(repo_root, &prompt_slug(prompt_path)?) {
        TransitionJournalStatus::Missing | TransitionJournalStatus::Committed(_) => {
            if backup.exists() {
                Err(format!(
                    "ambiguous transition backup exists: {}",
                    backup.display()
                ))
            } else {
                Ok(())
            }
        }
        TransitionJournalStatus::Ambiguous(reason) => {
            Err(format!("rollback-unconfirmed: {reason}"))
        }
        TransitionJournalStatus::Prepared(record) => {
            if backup.exists() {
                let backup_bytes =
                    fs::read(&backup).map_err(|e| format!("cannot read transition backup: {e}"))?;
                if digest(&backup_bytes) != record.old_digest {
                    return Err(
                        "rollback-unconfirmed: backup digest differs from old digest".to_string(),
                    );
                }
                fs::remove_file(&backup)
                    .map_err(|e| format!("cannot remove recovered transition backup: {e}"))?;
                sync_parent(backup.parent().unwrap())?;
            }
            let prompt = ensure_prompt_binding(repo_root, prompt_path)?;
            let current = if prompt.exists() {
                fs::read(prompt.full_path())
                    .map_err(|e| format!("cannot read prompt during recovery: {e}"))?
            } else {
                Vec::new()
            };
            let current_digest = if prompt.exists() {
                digest(&current)
            } else {
                "-".to_string()
            };
            if current_digest == record.new_digest {
                let committed = TransitionBindingRecord {
                    state: "committed".into(),
                    ..record
                };
                write_control_file(&journal, committed.line().as_bytes())
            } else if current_digest == record.old_digest {
                fs::remove_file(&journal)
                    .map_err(|e| format!("cannot discard prepared journal: {e}"))?;
                sync_parent(journal.parent().unwrap())
            } else {
                Err("rollback-unconfirmed: prompt matches neither prepared digest".to_string())
            }
        }
    }
}

/// Test-only fault injection: simulate a crash at a named point inside
/// `transition_prompt`, standing in for a real process kill so recovery can be
/// proven deterministically instead of by racing an actual OS signal.
fn transition_checkpoint(label: &str) -> Result<(), String> {
    let configured = std::env::var("GAL_TEST_FIRST_TRANSITION_FAIL_AT").ok();
    if configured.as_deref() == Some(label) {
        return Err(format!("injected transition interruption at {label}"));
    }
    Ok(())
}

/// Apply one exact-byte prompt transition. The expected digest is `None` only for
/// first creation, and only `init` may use that form.
pub fn transition_prompt(
    repo_root: &Path,
    prompt_path: &Path,
    new_bytes: &[u8],
    expected_old_digest: Option<&str>,
    kind: &str,
) -> Result<TransitionBindingRecord, String> {
    if !is_valid_kind(kind) || kind == "legacy-bootstrap" {
        return Err(format!("kind is not a writable prompt transition: {kind}"));
    }
    if expected_old_digest.is_none() && kind != "init" {
        return Err("absent old digest is allowed only for init".to_string());
    }
    let prompt = ensure_prompt_binding(repo_root, prompt_path)?;
    let (journal, lock_path, backup) = journal_paths(repo_root, prompt_path)?;
    fs::create_dir_all(lock_path.parent().unwrap())
        .map_err(|e| format!("cannot create transition directory: {e}"))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|e| format!("cannot open transition lock: {e}"))?;
    lock.try_lock_exclusive()
        .map_err(|e| format!("transition lock is held: {e}"))?;
    let result = (|| {
        recover_locked(repo_root, prompt_path)?;
        let current = if prompt.exists() {
            prompt
                .recheck()
                .map_err(|e| format!("prompt identity changed: {e}"))?;
            fs::read(prompt.full_path()).map_err(|e| format!("cannot read prompt: {e}"))?
        } else {
            Vec::new()
        };
        let old_digest = if prompt.exists() {
            digest(&current)
        } else {
            "-".to_string()
        };
        if expected_old_digest != Some(old_digest.as_str())
            && !(expected_old_digest.is_none() && old_digest == "-")
        {
            return Err(format!(
                "stale transition CAS: expected {:?}, actual {old_digest}",
                expected_old_digest
            ));
        }
        {
            // The marker lives above the first H2, so it sits outside every
            // region `compute_contract_region_digest` hashes. Without this
            // guard a transition could drop it and leave the digest unchanged,
            // silently demoting a journal-bound prompt to the legacy lane where
            // every marked-lane control reports pass by reporting nothing.
            // Decoded lossily on purpose: this guard applies to every kind, and
            // a strict decode here would newly reject non-UTF-8 prompts on the
            // `contract-change` and `init` paths that never required one.
            let current_lossy = String::from_utf8_lossy(&current);
            let new_lossy = String::from_utf8_lossy(new_bytes);
            if pipeline::task_spec::has_test_first_marker(&current_lossy)
                && !pipeline::task_spec::has_test_first_marker(&new_lossy)
            {
                return Err("transition would remove the `Pipeline Contract: test-first-v1` marker: a marked prompt may not be demoted to the legacy lane by a transition".to_string());
            }
        }
        if kind != "contract-change" && kind != "init" {
            let current_str =
                std::str::from_utf8(&current).map_err(|e| format!("prompt is not UTF-8: {e}"))?;
            let new_str =
                std::str::from_utf8(new_bytes).map_err(|e| format!("prompt is not UTF-8: {e}"))?;
            let old_contract_digest =
                pipeline::task_spec::compute_contract_region_digest(current_str)
                    .map_err(|e| e.to_string())?;
            let new_contract_digest = pipeline::task_spec::compute_contract_region_digest(new_str)
                .map_err(|e| e.to_string())?;
            if old_contract_digest != new_contract_digest {
                return Err(format!(
                    "contract-region changed from {old_contract_digest} to {new_contract_digest}: requires contract-change transition"
                ));
            }
        }
        let new_digest = digest(new_bytes);
        let prepared = TransitionBindingRecord {
            state: "prepared".into(),
            kind: kind.into(),
            old_digest: old_digest.clone(),
            new_digest: new_digest.clone(),
        };
        write_control_file(&journal, &journal_bytes(&prepared, None))?;
        if prompt.exists() {
            write_control_file(&backup, &current)?;
        }
        transition_checkpoint("before-prompt-write")?;
        publish_prompt_bytes(repo_root, prompt, new_bytes)?;
        transition_checkpoint("after-prompt-write")?;
        if backup.exists() {
            fs::remove_file(&backup)
                .map_err(|e| format!("cannot remove transition backup: {e}"))?;
            sync_parent(backup.parent().unwrap())?;
        }
        let committed = TransitionBindingRecord {
            state: "committed".into(),
            ..prepared
        };
        write_control_file(&journal, &journal_bytes(&committed, None))?;
        Ok(committed)
    })();
    let _ = lock.unlock();
    result
}

pub fn create_prompt(
    repo_root: &Path,
    prompt_path: &Path,
    bytes: &[u8],
) -> Result<TransitionBindingRecord, String> {
    transition_prompt(repo_root, prompt_path, bytes, None, "init")
}

pub fn refresh_prompt(
    repo_root: &Path,
    prompt_path: &Path,
    bytes: &[u8],
    old_digest: &str,
) -> Result<TransitionBindingRecord, String> {
    transition_prompt(
        repo_root,
        prompt_path,
        bytes,
        Some(old_digest),
        "phase-rerun",
    )
}

pub fn install_prompt(
    repo_root: &Path,
    prompt_path: &Path,
    bytes: &[u8],
    old_digest: &str,
) -> Result<TransitionBindingRecord, String> {
    transition_prompt(
        repo_root,
        prompt_path,
        bytes,
        Some(old_digest),
        "contract-change",
    )
}

/// Append one generation row through the same exact-byte transition producer.
pub fn append_generation(
    repo_root: &Path,
    prompt_path: &Path,
    task: &str,
    contract_digest: &str,
    reason: &str,
    old_digest: &str,
) -> Result<TransitionBindingRecord, String> {
    if !is_lower_hex_64(contract_digest) {
        return Err("invalid generation contract digest".to_string());
    }
    if !matches!(
        reason,
        "init" | "phase-rerun" | "probe-defect" | "contract-change"
    ) {
        return Err(format!("invalid generation reason: {reason}"));
    }
    let prompt = ensure_prompt_binding(repo_root, prompt_path)?;
    if !prompt.exists() {
        return Err("generation append requires an existing prompt".to_string());
    }
    let bytes = fs::read(prompt.full_path()).map_err(|e| format!("cannot read prompt: {e}"))?;
    let text = String::from_utf8(bytes).map_err(|e| format!("prompt is not UTF-8: {e}"))?;
    let heading = "### Test-First Generations";
    let mut status_start = None;
    let mut status_end = text.len();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']).trim();
        if status_start.is_none() {
            if trimmed == "## Status" {
                status_start = Some(offset + line.len());
            }
        } else if line
            .strip_prefix("##")
            .is_some_and(|rest| rest.starts_with(char::is_whitespace))
        {
            status_end = offset;
            break;
        }
        offset += line.len();
    }
    let (status_start, status_end) = match status_start {
        Some(status_start) => (status_start, status_end),
        None => (0, text.len()),
    };
    let status = &text[status_start..status_end];
    let heading_offset = status
        .split_inclusive('\n')
        .scan(0, |line_offset, line| {
            let current = *line_offset;
            *line_offset += line.len();
            Some((current, line))
        })
        .find(|(_, line)| line.trim_end_matches(['\r', '\n']).trim() == heading)
        .map(|(line_offset, _)| line_offset)
        .ok_or_else(|| "generation table is missing".to_string())?;
    let heading_pos = status_start + heading_offset;
    let table_end = text[heading_pos..status_end]
        .find("\n### ")
        .map(|offset| heading_pos + offset)
        .unwrap_or(status_end);
    let table = &text[heading_pos..table_end];
    let mut latest = 0u32;
    for line in table.lines().skip(1) {
        // A 4-column `| task | generation | digest | reason |` row splits on '|' into
        // 6 elements: an empty leading element, the 4 fields, and an empty trailing
        // element (the row starts and ends with a pipe).
        let fields: Vec<_> = line.split('|').map(str::trim).collect();
        if fields.len() == 6 && fields[1] == task {
            let generation = fields[2]
                .parse::<u32>()
                .map_err(|_| "malformed generation row".to_string())?;
            latest = latest.max(generation);
        }
    }
    if reason == "init" && latest != 0 {
        return Err("generation init already exists for task".to_string());
    }
    if reason != "init" && latest == 0 {
        return Err("generation transition has no prior generation".to_string());
    }
    let generation = latest
        .checked_add(1)
        .ok_or_else(|| "generation overflow".to_string())?;
    let row = format!("| {task} | {generation} | {contract_digest} | {reason} |\n");
    let separator = "| --- | --- | --- | --- |";
    let separator_pos = table
        .find(separator)
        .ok_or_else(|| "generation table separator is missing".to_string())?;
    let separator_end = heading_pos + separator_pos + separator.len();
    let mut insert_at = separator_end;
    let mut insertion = format!("\n{row}");
    let mut offset = separator_end;
    for line in text[separator_end..table_end].split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']).trim();
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            insert_at = offset + line.len();
            insertion = row.clone();
        }
        offset += line.len();
    }
    let mut next = text;
    next.insert_str(insert_at, &insertion);
    transition_prompt(
        repo_root,
        prompt_path,
        next.as_bytes(),
        Some(old_digest),
        reason,
    )
}

fn current_dir() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|e| format!("cannot determine repository root: {e}"))
}

/// Run an internal transition: `gal test-first-transition <operation> <prompt>`.
pub(crate) fn cmd_test_first_transition(args: &[String]) -> gal_engine::ExitCode {
    let operation = args.get(1).map(String::as_str);
    let prompt_arg = args.get(2).map(String::as_str);

    if operation == Some("legacy-bootstrap") {
        eprintln!("test-first-transition: legacy-bootstrap producer is unavailable");
        return gal_engine::ExitCode::Error;
    }

    if !matches!(
        operation,
        Some("init" | "refresh" | "install" | "generation")
    ) || prompt_arg.is_none()
    {
        eprintln!("usage: gal test-first-transition <operation> <prompt> [options]");
        return gal_engine::ExitCode::Usage;
    }

    let repo = match current_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("test-first-transition: {error}");
            return gal_engine::ExitCode::Error;
        }
    };
    let prompt = repo.join(prompt_arg.unwrap());

    // `generation` reads and rewrites the prompt's existing bytes internally
    // (via `append_generation`); it takes no external bytes file. Every other
    // writable operation requires one.
    let record = if operation == Some("generation") {
        let task = args.get(3).ok_or_else(|| "task is required".to_string());
        let contract = args
            .get(4)
            .ok_or_else(|| "contract digest is required".to_string());
        let reason = args.get(5).ok_or_else(|| "reason is required".to_string());
        let old = args
            .get(6)
            .ok_or_else(|| "old digest is required".to_string());
        task.and_then(|task| {
            contract.and_then(|contract| {
                reason.and_then(|reason| {
                    old.and_then(|old| {
                        append_generation(&repo, &prompt, task, contract, reason, old)
                    })
                })
            })
        })
    } else {
        let bytes_path = args.get(3).map(PathBuf::from);
        let bytes = match bytes_path {
            Some(path) => match fs::read(repo.join(path)) {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!("test-first-transition: cannot read transition bytes: {error}");
                    return gal_engine::ExitCode::Error;
                }
            },
            None => {
                eprintln!("test-first-transition: transition bytes are required");
                return gal_engine::ExitCode::Usage;
            }
        };
        match operation {
            Some("init") => create_prompt(&repo, &prompt, &bytes),
            Some("refresh") => args
                .get(4)
                .ok_or_else(|| "old digest is required".to_string())
                .and_then(|old| refresh_prompt(&repo, &prompt, &bytes, old)),
            Some("install") => args
                .get(4)
                .ok_or_else(|| "old digest is required".to_string())
                .and_then(|old| install_prompt(&repo, &prompt, &bytes, old)),
            _ => unreachable!(),
        }
    };
    match record {
        Ok(record) => {
            println!("kind={}", record.kind);
            println!("old_digest={}", record.old_digest);
            println!("new_digest={}", record.new_digest);
            println!("journal_state={}", record.state);
            gal_engine::ExitCode::Success
        }
        Err(error) => {
            eprintln!("test-first-transition: {error}");
            gal_engine::ExitCode::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn parse_line_valid_committed_record() {
        let line =
            "committed\tinit\t-\t0000000000000000000000000000000000000000000000000000000000000000";
        let rec = TransitionBindingRecord::parse_line(line).unwrap();
        assert_eq!(rec.state, "committed");
        assert_eq!(rec.kind, "init");
        assert_eq!(rec.old_digest, "-");
        assert_eq!(
            rec.new_digest,
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn parse_line_rejects_extra_fields() {
        let line = "committed\tinit\t-\t0000000000000000000000000000000000000000000000000000000000000000\textra";
        assert!(TransitionBindingRecord::parse_line(line).is_err());
    }

    #[test]
    fn parse_line_rejects_cr() {
        let line = "committed\tinit\t-\t0000000000000000000000000000000000000000000000000000000000000000\r";
        assert!(TransitionBindingRecord::parse_line(line).is_err());
    }

    #[test]
    fn parse_line_rejects_surrounding_whitespace() {
        let line =
            " committed\tinit\t-\t0000000000000000000000000000000000000000000000000000000000000000";
        assert!(TransitionBindingRecord::parse_line(line).is_err());
    }

    #[test]
    fn parse_line_rejects_invalid_kind() {
        let line = "committed\tunknown_kind\t-\t0000000000000000000000000000000000000000000000000000000000000000";
        assert!(TransitionBindingRecord::parse_line(line).is_err());
    }

    #[test]
    fn parse_line_rejects_non_hex_digest() {
        let line = "committed\tinit\t-\tnot_a_hex_digest";
        assert!(TransitionBindingRecord::parse_line(line).is_err());
    }

    fn tid(n: u32) -> String {
        format!("T-{n:02}")
    }

    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .expect("git must be available");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// A bare git-initialized repo with a `.dev/plans/` directory. Callers that need
    /// the prompt or source plan tracked commit it themselves.
    fn fixture(slug: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let temp = tempfile::tempdir().expect("fixture tempdir");
        let repo = temp.path().to_path_buf();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["config", "user.email", "fixture@example.com"]);
        git(&repo, &["config", "user.name", "CLI Fixture"]);
        git(&repo, &["config", "commit.gpgsign", "false"]);
        git(&repo, &["config", "core.hooksPath", ".git/hooks"]);
        fs::create_dir_all(repo.join(".dev/plans")).unwrap();
        let prompt_path = repo.join(".dev/plans").join(format!("{slug}.prompt.md"));
        (temp, repo, prompt_path)
    }

    #[test]
    fn create_prompt_succeeds_when_absent_and_records_init() {
        let (_temp, repo, prompt_path) = fixture("t23-create");
        let record = create_prompt(&repo, &prompt_path, b"hello\n").unwrap();
        assert_eq!(record.state, "committed");
        assert_eq!(record.kind, "init");
        assert_eq!(record.old_digest, "-");
        assert_eq!(record.new_digest, digest(b"hello\n"));
        assert_eq!(fs::read(&prompt_path).unwrap(), b"hello\n");
    }

    #[test]
    fn create_prompt_rejects_reinit_over_existing_prompt() {
        let (_temp, repo, prompt_path) = fixture("t23-reinit");
        create_prompt(&repo, &prompt_path, b"one\n").unwrap();
        let err = create_prompt(&repo, &prompt_path, b"two\n").unwrap_err();
        assert!(err.contains("stale transition CAS"), "{err}");
        assert_eq!(fs::read(&prompt_path).unwrap(), b"one\n");
    }

    fn valid_prompt(status: &str) -> Vec<u8> {
        format!(
            "## Goal\n\n- Goal\n\n## Requirements\n\n- Req\n\n## Tasks\n\n- Task\n\n## Test Plan\n\n- Test plan\n\n## Status\n\n{status}\n"
        )
        .into_bytes()
    }

    #[test]
    fn refresh_prompt_rejects_stale_digest_and_preserves_bytes() {
        let (_temp, repo, prompt_path) = fixture("t23-refresh-stale");
        create_prompt(&repo, &prompt_path, &valid_prompt("v1")).unwrap();
        let wrong_digest = "0000000000000000000000000000000000000000000000000000000000000000";
        let err =
            refresh_prompt(&repo, &prompt_path, &valid_prompt("v2"), wrong_digest).unwrap_err();
        assert!(err.contains("stale transition CAS"), "{err}");
        assert_eq!(fs::read(&prompt_path).unwrap(), valid_prompt("v1"));
    }

    #[test]
    fn refresh_prompt_transitions_with_correct_digest_and_exact_bytes() {
        let (_temp, repo, prompt_path) = fixture("t23-refresh-ok");
        let init = create_prompt(&repo, &prompt_path, &valid_prompt("v1")).unwrap();
        let refreshed =
            refresh_prompt(&repo, &prompt_path, &valid_prompt("v2"), &init.new_digest).unwrap();
        assert_eq!(refreshed.kind, "phase-rerun");
        assert_eq!(refreshed.old_digest, init.new_digest);
        assert_eq!(refreshed.new_digest, digest(&valid_prompt("v2")));
        assert_eq!(fs::read(&prompt_path).unwrap(), valid_prompt("v2"));
    }

    #[test]
    fn install_prompt_uses_contract_change_kind() {
        let (_temp, repo, prompt_path) = fixture("t23-install");
        let init = create_prompt(&repo, &prompt_path, b"v1\n").unwrap();
        let installed = install_prompt(&repo, &prompt_path, b"v2\n", &init.new_digest).unwrap();
        assert_eq!(installed.kind, "contract-change");
        assert_eq!(fs::read(&prompt_path).unwrap(), b"v2\n");
    }

    fn marked_prompt(status: &str) -> Vec<u8> {
        format!(
            "# Plan Prompt

Pipeline Contract: test-first-v1

## Goal

- Goal

## Requirements

- Req

## Tasks

- Task

## Test Plan

- Test plan

## Status

{status}
"
        )
        .into_bytes()
    }

    #[test]
    fn transition_may_not_strip_the_test_first_marker() {
        // The marker sits above the first H2, outside every hashed contract
        // region, so the digest guard cannot see it leave. Dropping it would
        // demote a journal-bound prompt to the legacy lane, where the marked
        // controls stop reporting instead of reporting a failure.
        let (_temp, repo, prompt_path) = fixture("t23-marker-strip");
        let marked = marked_prompt("v1");
        let init = create_prompt(&repo, &prompt_path, &marked).unwrap();

        let stripped = String::from_utf8(marked.clone()).unwrap().replace(
            "Pipeline Contract: test-first-v1

",
            "",
        );
        assert!(!pipeline::task_spec::has_test_first_marker(&stripped));

        let err =
            refresh_prompt(&repo, &prompt_path, stripped.as_bytes(), &init.new_digest).unwrap_err();
        assert!(
            err.contains("would remove the `Pipeline Contract: test-first-v1` marker"),
            "{err}"
        );
        // Rejected before any write: the prompt is still byte-identical.
        assert_eq!(fs::read(&prompt_path).unwrap(), marked);
    }

    #[test]
    fn transition_prompt_rejects_concurrent_lock_holder() {
        let (_temp, repo, prompt_path) = fixture("t23-lock");
        create_prompt(&repo, &prompt_path, &valid_prompt("v1")).unwrap();
        let (_journal, lock_path, _backup) = journal_paths(&repo, &prompt_path).unwrap();
        let held = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        held.try_lock_exclusive().unwrap();
        let err = refresh_prompt(
            &repo,
            &prompt_path,
            &valid_prompt("v2"),
            &digest(&valid_prompt("v1")),
        )
        .unwrap_err();
        assert!(err.contains("transition lock is held"), "{err}");
        let _ = held.unlock();
        // The lock holder never touched the prompt, so it is still exactly v1.
        assert_eq!(fs::read(&prompt_path).unwrap(), valid_prompt("v1"));
    }

    #[test]
    fn append_generation_records_unique_init_row_through_exact_byte_producer() {
        let (_temp, repo, prompt_path) = fixture("t23-gen-init");
        let prompt_body = "# Plan Prompt\n\n### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n";
        let init = create_prompt(&repo, &prompt_path, prompt_body.as_bytes()).unwrap();
        let contract = "a".repeat(64);
        let task = tid(1);
        let appended = append_generation(
            &repo,
            &prompt_path,
            &task,
            &contract,
            "init",
            &init.new_digest,
        )
        .unwrap();
        let expected_bytes = fs::read(&prompt_path).unwrap();
        assert_eq!(appended.new_digest, digest(&expected_bytes));
        let text = String::from_utf8(expected_bytes).unwrap();
        assert!(text.contains(&format!("| {task} | 1 | {contract} | init |")));

        // A second `init` for the same task is rejected: the row is unique per task.
        let err = append_generation(
            &repo,
            &prompt_path,
            &task,
            &contract,
            "init",
            &appended.new_digest,
        )
        .unwrap_err();
        assert!(err.contains("generation init already exists"), "{err}");
    }

    #[test]
    fn append_generation_rejects_transition_reason_with_no_prior_generation() {
        let (_temp, repo, prompt_path) = fixture("t23-gen-no-prior");
        let prompt_body = "# Plan Prompt\n\n### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n";
        let init = create_prompt(&repo, &prompt_path, prompt_body.as_bytes()).unwrap();
        let contract = "b".repeat(64);
        let err = append_generation(
            &repo,
            &prompt_path,
            &tid(2),
            &contract,
            "phase-rerun",
            &init.new_digest,
        )
        .unwrap_err();
        assert!(err.contains("no prior generation"), "{err}");
    }

    #[test]
    fn append_generation_advances_generation_number_for_transition_reasons() {
        let (_temp, repo, prompt_path) = fixture("t23-gen-advance");
        let prompt_body = "## Goal\n\n- Goal\n\n## Requirements\n\n- Req\n\n## Tasks\n\n- Task\n\n## Test Plan\n\n- Test plan\n\n## Status\n\n### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n";
        let init = create_prompt(&repo, &prompt_path, prompt_body.as_bytes()).unwrap();
        let task = tid(3);
        let contract_1 = "c".repeat(64);
        let gen1 = append_generation(
            &repo,
            &prompt_path,
            &task,
            &contract_1,
            "init",
            &init.new_digest,
        )
        .unwrap();
        let contract_2 = "d".repeat(64);
        let gen2 = append_generation(
            &repo,
            &prompt_path,
            &task,
            &contract_2,
            "phase-rerun",
            &gen1.new_digest,
        )
        .unwrap();
        let text = fs::read_to_string(&prompt_path).unwrap();
        assert!(text.contains(&format!("| {task} | 2 | {contract_2} | phase-rerun |")));
        assert_ne!(gen1.new_digest, gen2.new_digest);
    }

    #[test]
    fn recover_locked_resolves_prepared_state_matching_new_digest_as_committed() {
        let (_temp, repo, prompt_path) = fixture("t23-recover-new");
        let init = create_prompt(&repo, &prompt_path, b"v1\n").unwrap();
        let (journal, _lock, backup) = journal_paths(&repo, &prompt_path).unwrap();
        // Simulate a crash after the prompt write landed but before the journal was
        // marked committed: the prompt already holds the prepared record's new bytes.
        let prepared = TransitionBindingRecord {
            state: "prepared".into(),
            kind: "phase-rerun".into(),
            old_digest: init.new_digest.clone(),
            new_digest: digest(b"v2\n"),
        };
        fs::write(&prompt_path, b"v2\n").unwrap();
        test_first_fs::atomic_publish_bytes(&journal, prepared.line().as_bytes()).unwrap();
        assert!(!backup.exists());

        recover_locked(&repo, &prompt_path).unwrap();
        match read_latest_transition_record(&repo, "t23-recover-new") {
            TransitionJournalStatus::Committed(rec) => {
                assert_eq!(rec.new_digest, digest(b"v2\n"));
            }
            other => panic!("expected committed recovery, got {other:?}"),
        }
    }

    #[test]
    fn recover_locked_discards_prepared_state_matching_old_digest() {
        let (_temp, repo, prompt_path) = fixture("t23-recover-old");
        create_prompt(&repo, &prompt_path, b"v1\n").unwrap();
        let init_digest = digest(b"v1\n");
        let (journal, _lock, backup) = journal_paths(&repo, &prompt_path).unwrap();
        let prepared = TransitionBindingRecord {
            state: "prepared".into(),
            kind: "phase-rerun".into(),
            old_digest: init_digest,
            new_digest: digest(b"v2\n"),
        };
        // Simulate a crash before the prompt write landed: bytes are still the old value.
        test_first_fs::atomic_publish_bytes(&journal, prepared.line().as_bytes()).unwrap();
        assert!(!backup.exists());

        recover_locked(&repo, &prompt_path).unwrap();
        assert!(!journal.exists());
        assert_eq!(fs::read(&prompt_path).unwrap(), b"v1\n");
    }

    #[test]
    fn recover_locked_fails_closed_on_unknown_journal_state() {
        let (_temp, repo, prompt_path) = fixture("t23-recover-unknown");
        create_prompt(&repo, &prompt_path, b"v1\n").unwrap();
        let (journal, _lock, _backup) = journal_paths(&repo, &prompt_path).unwrap();
        let bogus =
            "disputed\tinit\t-\t0000000000000000000000000000000000000000000000000000000000000000\n";
        test_first_fs::atomic_publish_bytes(&journal, bogus.as_bytes()).unwrap();

        let err = recover_locked(&repo, &prompt_path).unwrap_err();
        assert!(err.contains("rollback-unconfirmed"), "{err}");
    }
}
