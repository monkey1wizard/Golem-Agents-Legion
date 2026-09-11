//! Dormant deterministic cleanup for one finalized test-first plan.

use super::test_first_fs;
use gal_foundation::validated_repo_path::{ValidatedRepoPath, ValidatedRepoPathMode};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const RECEIPTS_ROOT: &str = ".dev/pipeline/receipts";
const SNAPSHOTS_ROOT: &str = ".dev/pipeline/snapshots";

#[derive(Debug, Clone)]
pub struct CleanupInput {
    pub plan_slug: String,
    pub finalized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupEvidence {
    pub identities: Vec<String>,
    pub quarantine_paths: Vec<String>,
    pub removed_roots: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RootRecord {
    source: String,
    quarantine: String,
    state: String,
}

/// Delegates to the one real implementation, which fsyncs on Unix and, on
/// Windows, opens the directory handle and tolerates the `ERROR_ACCESS_DENIED`
/// that Win32 returns for a directory flush by design. The local copy this
/// replaces did nothing at all off Unix. Windows still has no directory flush
/// available, so the quarantine-then-unlink journal gains an openability check
/// there rather than a durability barrier.
fn sync_dir(path: &Path) -> Result<(), String> {
    super::test_first_fs::sync_dir(path)
}

fn validate_slug(slug: &str) -> Result<(), String> {
    if slug.is_empty()
        || slug == "."
        || slug == ".."
        || slug.contains('/')
        || slug.contains('\\')
        || slug.contains(':')
    {
        return Err(format!("invalid plan slug '{slug}'"));
    }
    Ok(())
}

fn journal_path(repo_root: &Path, slug: &str) -> PathBuf {
    repo_root
        .join(".dev/pipeline/cleanup")
        .join(slug)
        .join("cleanup.journal.tsv")
}

fn journal_relative(slug: &str) -> PathBuf {
    PathBuf::from(".dev/pipeline/cleanup")
        .join(slug)
        .join("cleanup.journal.tsv")
}

fn parse_journal(bytes: &[u8]) -> Result<Vec<RootRecord>, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|e| format!("cleanup journal is not UTF-8: {e}"))?;
    let mut records = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.contains('\r') {
            return Err("cleanup journal contains an empty or non-canonical line".to_string());
        }
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 || fields.iter().any(|field| field.is_empty()) {
            return Err(format!("malformed cleanup journal row: {line:?}"));
        }
        if !matches!(fields[2], "prepared" | "quarantined" | "removed") {
            return Err(format!("unknown cleanup journal state '{}'", fields[2]));
        }
        records.push(RootRecord {
            source: fields[0].to_string(),
            quarantine: fields[1].to_string(),
            state: fields[2].to_string(),
        });
    }
    Ok(records)
}

fn write_journal(path: &Path, records: &[RootRecord]) -> Result<(), String> {
    let mut bytes = Vec::new();
    for record in records {
        bytes.extend_from_slice(
            format!(
                "{}\t{}\t{}\n",
                record.source, record.quarantine, record.state
            )
            .as_bytes(),
        );
    }
    test_first_fs::atomic_publish_bytes(path, &bytes)?;
    sync_dir(path.parent().ok_or("cleanup journal has no parent")?)
}

fn checkpoint(label: &str) -> Result<(), String> {
    if std::env::var("GAL_TEST_FIRST_CLEANUP_FAIL_AT")
        .ok()
        .as_deref()
        == Some(label)
    {
        return Err(format!("injected cleanup interruption at {label}"));
    }
    Ok(())
}

fn is_lower_hex_64(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

fn is_valid_task_components(comp1: &str, comp2: &str) -> bool {
    if !comp1.starts_with("T-") {
        return false;
    }
    let digits = &comp1[2..];
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    if !comp2.starts_with('g') {
        return false;
    }
    let rest = &comp2[1..];
    let Some(dash_c_idx) = rest.find("-c") else {
        return false;
    };
    let g_num_str = &rest[..dash_c_idx];
    let c_hex = &rest[dash_c_idx + 2..];

    if g_num_str.is_empty() || !g_num_str.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let Ok(g_num) = g_num_str.parse::<u64>() else {
        return false;
    };
    if g_num == 0 {
        return false;
    }

    is_lower_hex_64(c_hex)
}

fn is_owned_receipt_file(rel_path: &str) -> bool {
    let parts: Vec<&str> = rel_path.split('/').collect();
    if parts.is_empty() {
        return false;
    }

    let filename = parts[parts.len() - 1];
    if filename.ends_with(".receipt.md") {
        return true;
    }

    if parts.len() == 4 && is_valid_task_components(parts[0], parts[1]) && parts[2] == "outputs" {
        if let Some(hex_part) = parts[3].strip_suffix(".bin") {
            if is_lower_hex_64(hex_part) {
                return true;
            }
        }
    }

    false
}

fn is_owned_snapshot_file(rel_path: &str) -> bool {
    let parts: Vec<&str> = rel_path.split('/').collect();
    if parts.len() < 3 {
        return false;
    }

    if !is_valid_task_components(parts[0], parts[1]) {
        return false;
    }

    match parts.len() {
        3 => matches!(
            parts[2],
            "implementation-commit.snapshot.tsv"
                | "pre-implement.snapshot.tsv"
                | "state-recording.snapshot.tsv"
                | "post-test.snapshot.tsv"
                | "post-audit.snapshot.tsv"
        ),
        4 => {
            if parts[2] == "blobs" {
                if let Some(hex_part) = parts[3].strip_suffix(".bin") {
                    return is_lower_hex_64(hex_part);
                }
            }
            if parts[2] == "restore" && parts[3] == "restore.journal.tsv" {
                return true;
            }
            false
        }
        5 => {
            if parts[2] == "restore" && parts[3] == "blobs" {
                if let Some(hex_part) = parts[4].strip_suffix(".bin") {
                    return is_lower_hex_64(hex_part);
                }
            }
            false
        }
        _ => false,
    }
}

fn is_symlink_or_reparse(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT = 0x400
        if (meta.file_attributes() & 0x400) != 0 {
            return true;
        }
    }
    false
}

fn check_git_tracked_files(repo_root: &Path, rel_root: &Path) -> Result<(), String> {
    let rel_str = rel_root.to_string_lossy().replace('\\', "/");
    let output = std::process::Command::new("git")
        .current_dir(repo_root)
        .args(["ls-files", "--", &rel_str])
        .output()
        .map_err(|e| {
            format!(
                "cleanup ownership: cannot execute git ls-files for root {}: {e}",
                rel_root.display()
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "cleanup ownership: git ls-files failed for root {}: {}",
            rel_root.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        return Err(format!(
            "cleanup ownership: root {} contains tracked files in git",
            rel_root.display()
        ));
    }
    Ok(())
}

fn validate_precleanup_root(
    repo_root: &Path,
    rel_root: &Path,
    is_receipts: bool,
) -> Result<(), String> {
    let full_root = repo_root.join(rel_root);
    let meta = match fs::symlink_metadata(&full_root) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(format!(
                "cleanup ownership: cannot stat root {}: {e}",
                rel_root.display()
            ))
        }
    };

    if is_symlink_or_reparse(&meta) {
        return Err(format!(
            "cleanup ownership: root {} is a symlink or reparse point",
            rel_root.display()
        ));
    }
    if !meta.is_dir() {
        return Err(format!(
            "cleanup ownership: root {} is not a directory",
            rel_root.display()
        ));
    }

    validate_root(repo_root, rel_root).map_err(|e| format!("cleanup ownership: {e}"))?;

    check_git_tracked_files(repo_root, rel_root)?;

    let mut stack = vec![full_root.clone()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| {
            format!(
                "cleanup ownership: cannot enumerate directory {}: {e}",
                dir.display()
            )
        })?;
        for entry_res in entries {
            let entry = entry_res.map_err(|e| {
                format!(
                    "cleanup ownership: error reading directory entry in {}: {e}",
                    dir.display()
                )
            })?;
            let entry_path = entry.path();
            let entry_meta = fs::symlink_metadata(&entry_path).map_err(|e| {
                format!(
                    "cleanup ownership: cannot stat entry {}: {e}",
                    entry_path.display()
                )
            })?;

            if is_symlink_or_reparse(&entry_meta) {
                return Err(format!(
                    "cleanup ownership: symlink or reparse point at {}",
                    entry_path.display()
                ));
            }

            if entry_meta.is_dir() {
                stack.push(entry_path);
            } else if entry_meta.is_file() {
                let rel_file = entry_path.strip_prefix(&full_root).map_err(|e| {
                    format!(
                        "cleanup ownership: path prefix strip error for {}: {e}",
                        entry_path.display()
                    )
                })?;
                let rel_file_str = rel_file.to_string_lossy().replace('\\', "/");
                let owned = if is_receipts {
                    is_owned_receipt_file(&rel_file_str)
                } else {
                    is_owned_snapshot_file(&rel_file_str)
                };
                if !owned {
                    return Err(format!(
                        "cleanup ownership: unowned file shape at {} ({})",
                        entry_path.display(),
                        rel_file_str
                    ));
                }
            } else {
                return Err(format!(
                    "cleanup ownership: non-regular file shape at {}",
                    entry_path.display()
                ));
            }
        }
    }

    Ok(())
}

fn validate_root(repo_root: &Path, relative: &Path) -> Result<ValidatedRepoPath, String> {
    let parent = relative
        .parent()
        .ok_or_else(|| format!("cleanup root has no parent: {}", relative.display()))?;
    ValidatedRepoPath::new(
        repo_root,
        relative,
        ValidatedRepoPathMode::ValidatedDirectChildDirectory {
            expected_parent: parent.to_path_buf(),
        },
    )
    .map_err(|e| {
        format!(
            "cleanup root validation failed for {}: {e}",
            relative.display()
        )
    })
}

fn unique_quarantine(
    repo_root: &Path,
    source: &ValidatedRepoPath,
) -> Result<ValidatedRepoPath, String> {
    let parent = source
        .rel_path()
        .parent()
        .ok_or("cleanup root has no parent")?;
    for attempt in 0..32_u32 {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let name = format!(".gal-cleanup-{suffix}-{}-{attempt}", std::process::id());
        let relative = parent.join(name);
        let candidate = ValidatedRepoPath::new(
            repo_root,
            &relative,
            ValidatedRepoPathMode::RegularFileOrMissing,
        )
        .map_err(|e| format!("quarantine target validation failed: {e}"))?;
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("could not allocate a unique quarantine target".to_string())
}

fn remove_tree(root: ValidatedRepoPath, evidence: &mut CleanupEvidence) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(root.full_path())
        .map_err(|e| format!("cannot enumerate {}: {e}", root.full_path().display()))?
    {
        let entry = entry.map_err(|e| format!("cannot enumerate cleanup child: {e}"))?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or("cleanup child name is not UTF-8")?
            .to_string();
        if !names.insert(name) {
            return Err("duplicate cleanup child name".to_string());
        }
    }

    for name in names {
        let child = root
            .validate_child(Path::new(&name))
            .map_err(|e| format!("cleanup child validation failed: {e}"))?;
        let child = child.into_path();
        evidence
            .identities
            .push(format!("{}", child.full_path().display()));
        if child.is_dir() {
            remove_tree(child, evidence)?;
        } else {
            let child_parent = child
                .full_path()
                .parent()
                .ok_or("cleanup child has no parent")?
                .to_path_buf();
            let removal = child
                .bind_removal()
                .map_err(|e| format!("cleanup removal binding failed: {e}"))?;
            checkpoint("before-removal")?;
            fs::remove_file(removal.unlink_path())
                .map_err(|e| format!("cannot unlink cleanup file: {e}"))?;
            removal
                .verify()
                .map_err(|e| format!("cleanup unlink verification failed: {e}"))?;
            sync_dir(&child_parent)?;
        }
    }

    let root_parent = root
        .full_path()
        .parent()
        .ok_or("cleanup root has no parent")?
        .to_path_buf();
    let removal = root
        .bind_removal()
        .map_err(|e| format!("cleanup directory removal binding failed: {e}"))?;
    checkpoint("before-removal")?;
    fs::remove_dir(removal.unlink_path())
        .map_err(|e| format!("cannot remove empty cleanup directory: {e}"))?;
    removal
        .verify()
        .map_err(|e| format!("cleanup directory removal verification failed: {e}"))?;
    sync_dir(&root_parent)?;
    Ok(())
}

fn cleanup_root(
    repo_root: &Path,
    relative: &Path,
    journal: &Path,
    records: &mut Vec<RootRecord>,
    evidence: &mut CleanupEvidence,
) -> Result<(), String> {
    let source_text = relative.to_string_lossy().replace('\\', "/");
    let existing = records
        .iter()
        .find(|record| record.source == source_text)
        .cloned();
    let (quarantine, state) = if let Some(record) = existing {
        if record.state == "removed" {
            return Ok(());
        }
        (PathBuf::from(record.quarantine), record.state)
    } else {
        let source = validate_root(repo_root, relative)?;
        let target = unique_quarantine(repo_root, &source)?;
        let quarantine = target.rel_path().to_string_lossy().replace('\\', "/");
        records.push(RootRecord {
            source: source_text.clone(),
            quarantine: quarantine.clone(),
            state: "prepared".to_string(),
        });
        write_journal(journal, records)?;
        checkpoint("after-journal")?;
        let relocation = source
            .bind_same_parent_relocation_target(target)
            .map_err(|e| format!("cleanup quarantine binding failed: {e}"))?;
        checkpoint("before-quarantine")?;
        fs::rename(relocation.source_path(), relocation.destination_path())
            .map_err(|e| format!("cannot quarantine cleanup root: {e}"))?;
        let quarantined = relocation
            .verify()
            .map_err(|e| format!("cleanup quarantine verification failed: {e}"))?;
        if let Some(record) = records
            .iter_mut()
            .find(|record| record.source == source_text)
        {
            record.state = "quarantined".to_string();
        }
        write_journal(journal, records)?;
        (
            quarantined.rel_path().to_path_buf(),
            "quarantined".to_string(),
        )
    };

    if state == "prepared" {
        let source_present = fs::symlink_metadata(repo_root.join(relative)).is_ok();
        let quarantine_present = fs::symlink_metadata(repo_root.join(&quarantine)).is_ok();
        match (source_present, quarantine_present) {
            (true, false) => {
                let source = validate_root(repo_root, relative)?;
                let target = ValidatedRepoPath::new(
                    repo_root,
                    &quarantine,
                    ValidatedRepoPathMode::RegularFileOrMissing,
                )
                .map_err(|e| format!("prepared quarantine validation failed: {e}"))?;
                let relocation = source
                    .bind_same_parent_relocation_target(target)
                    .map_err(|e| format!("prepared quarantine binding failed: {e}"))?;
                fs::rename(relocation.source_path(), relocation.destination_path())
                    .map_err(|e| format!("cannot resume cleanup quarantine: {e}"))?;
                relocation
                    .verify()
                    .map_err(|e| format!("resumed cleanup quarantine verification failed: {e}"))?;
            }
            (false, true) => {
                validate_root(repo_root, &quarantine)?;
            }
            _ => return Err("ambiguous prepared cleanup quarantine state".to_string()),
        }
        if let Some(record) = records
            .iter_mut()
            .find(|record| record.source == source_text)
        {
            record.state = "quarantined".to_string();
        }
        write_journal(journal, records)?;
    } else if state != "quarantined" {
        return Err("invalid cleanup recovery state".to_string());
    }
    let quarantined = validate_root(repo_root, &quarantine)?;
    evidence
        .quarantine_paths
        .push(quarantine.display().to_string());
    remove_tree(quarantined, evidence)?;
    if let Some(record) = records
        .iter_mut()
        .find(|record| record.source == source_text)
    {
        record.state = "removed".to_string();
    }
    write_journal(journal, records)?;
    Ok(())
}

/// Remove only the finalized plan's receipts and snapshots, with resumable recovery.
pub fn cleanup_plan(repo_root: &Path, input: &CleanupInput) -> Result<CleanupEvidence, String> {
    validate_slug(&input.plan_slug)?;
    if !input.finalized {
        return Err("cleanup requires a finalized plan".to_string());
    }
    let journal = journal_path(repo_root, &input.plan_slug);
    let journal_binding = ValidatedRepoPath::new(
        repo_root,
        &journal_relative(&input.plan_slug),
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .map_err(|e| format!("cleanup journal validation failed: {e}"))?;
    let mut records = if journal_binding.exists() {
        journal_binding
            .recheck()
            .map_err(|e| format!("cleanup journal identity changed: {e}"))?;
        parse_journal(
            &fs::read(journal_binding.full_path())
                .map_err(|e| format!("cannot read cleanup journal: {e}"))?,
        )?
    } else {
        Vec::new()
    };
    if records.iter().any(|record| {
        records
            .iter()
            .filter(|other| other.source == record.source)
            .count()
            > 1
    }) {
        return Err("ambiguous cleanup journal".to_string());
    }

    for (root_path, is_receipts) in [
        (PathBuf::from(RECEIPTS_ROOT).join(&input.plan_slug), true),
        (PathBuf::from(SNAPSHOTS_ROOT).join(&input.plan_slug), false),
    ] {
        let source_key = root_path.to_string_lossy().replace('\\', "/");
        let existing = records.iter().find(|r| r.source == source_key);
        let target_to_validate = match existing {
            Some(rec) if rec.state == "removed" => None,
            Some(rec) if rec.state == "quarantined" => Some(PathBuf::from(&rec.quarantine)),
            Some(rec) if rec.state == "prepared" => {
                if fs::symlink_metadata(repo_root.join(&root_path)).is_ok() {
                    Some(root_path.clone())
                } else if fs::symlink_metadata(repo_root.join(&rec.quarantine)).is_ok() {
                    Some(PathBuf::from(&rec.quarantine))
                } else {
                    Some(root_path.clone())
                }
            }
            _ => Some(root_path.clone()),
        };

        if let Some(target) = target_to_validate {
            validate_precleanup_root(repo_root, &target, is_receipts)?;
        }
    }

    let mut evidence = CleanupEvidence {
        identities: Vec::new(),
        quarantine_paths: Vec::new(),
        removed_roots: Vec::new(),
    };
    for root in [
        PathBuf::from(RECEIPTS_ROOT).join(&input.plan_slug),
        PathBuf::from(SNAPSHOTS_ROOT).join(&input.plan_slug),
    ] {
        let source_key = root.to_string_lossy().replace('\\', "/");
        let was_recorded = records.iter().any(|record| record.source == source_key);
        if !was_recorded && !repo_root.join(&root).exists() {
            continue;
        }
        if !was_recorded {
            validate_root(repo_root, &root)?;
        }
        cleanup_root(repo_root, &root, &journal, &mut records, &mut evidence)?;
        evidence.removed_roots.push(source_key);
    }
    if !records.is_empty() {
        write_journal(&journal, &records)?;
    }
    Ok(evidence)
}

fn required_arg(args: &[String], name: &str) -> Result<String, String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .ok_or_else(|| format!("missing required option {name}"))
}

fn receipt_text(input: &CleanupInput, evidence: &CleanupEvidence) -> String {
    format!(
        "kind=test-first-cleanup\nplan_slug={}\nfinalized={}\nremoved_roots={}\nquarantine_paths={}\nidentities={}\noverall: pass\n",
        input.plan_slug,
        input.finalized,
        evidence.removed_roots.join(","),
        evidence.quarantine_paths.join(","),
        evidence.identities.join(","),
    )
}

fn fail_receipt_text(plan_slug: &str, finalized: bool, error: &str) -> String {
    format!(
        "kind=test-first-cleanup\nplan_slug={}\nfinalized={}\nerror={}\noverall: fail\n",
        plan_slug,
        finalized,
        error.replace('\n', " "),
    )
}

/// Run `gal test-first-cleanup run ...` through the production CLI surface.
pub(crate) fn cmd_test_first_cleanup(args: &[String]) -> gal_engine::ExitCode {
    if args.get(1).map(String::as_str) != Some("run") {
        eprintln!(
            "test-first-cleanup: usage: gal test-first-cleanup run --plan <slug> --finalized --receipt <path>"
        );
        return gal_engine::ExitCode::Usage;
    }

    let plan_slug_res = required_arg(args, "--plan");
    let finalized = args.iter().any(|arg| arg == "--finalized");
    let receipt_res = required_arg(args, "--receipt");

    let (plan_slug, receipt_path) = match (plan_slug_res, receipt_res) {
        (Ok(slug), Ok(r_path)) => (slug, PathBuf::from(r_path)),
        (Err(err), _) | (_, Err(err)) => {
            eprintln!("test-first-cleanup: {err}");
            return gal_engine::ExitCode::Usage;
        }
    };

    let repo = match std::env::current_dir() {
        Ok(r) => r,
        Err(e) => {
            // With no repository root there is nothing to validate the receipt
            // destination against, so writing one would be the very unconstrained
            // write this path is meant to refuse. Report and exit instead.
            eprintln!("test-first-cleanup: cannot resolve repository: {e}");
            return gal_engine::ExitCode::Error;
        }
    };

    // Bind the destination once, before any branch writes. A bad `--receipt`
    // must fail with zero mutation, and every later branch then publishes
    // through the same validator rather than joining an operator string.
    if let Err(e) = test_first_fs::validate_receipt_destination(&repo, &receipt_path) {
        eprintln!("test-first-cleanup: {e}");
        return gal_engine::ExitCode::Error;
    }

    if !finalized {
        let error = "missing required option --finalized".to_string();
        eprintln!("test-first-cleanup: {error}");
        let fail_text = fail_receipt_text(&plan_slug, false, &error);
        let _ = test_first_fs::publish_receipt(&repo, &receipt_path, fail_text.as_bytes());
        return gal_engine::ExitCode::Error;
    }

    let input = CleanupInput {
        plan_slug: plan_slug.clone(),
        finalized,
    };

    match cleanup_plan(&repo, &input) {
        Ok(evidence) => {
            if let Err(e) = test_first_fs::publish_receipt(
                &repo,
                &receipt_path,
                receipt_text(&input, &evidence).as_bytes(),
            ) {
                eprintln!("test-first-cleanup: cannot write receipt: {e}");
                return gal_engine::ExitCode::Error;
            }
            println!("receipt={}", receipt_path.display());
            println!("removed_roots={}", evidence.removed_roots.len());
            gal_engine::ExitCode::Success
        }
        Err(error) => {
            // The original error reaches stderr first, so a later receipt-write
            // failure cannot mask what actually went wrong.
            eprintln!("test-first-cleanup: {error}");
            let fail_text = fail_receipt_text(&plan_slug, finalized, &error);
            let _ = test_first_fs::publish_receipt(&repo, &receipt_path, fail_text.as_bytes());
            gal_engine::ExitCode::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_root(temp: &TempDir, slug: &str) {
        let receipts_path = temp.path().join(RECEIPTS_ROOT).join(slug).join("task");
        fs::create_dir_all(&receipts_path).unwrap();
        fs::write(receipts_path.join("item.receipt.md"), b"x").unwrap();

        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let snapshots_path = temp
            .path()
            .join(SNAPSHOTS_ROOT)
            .join(slug)
            .join(format!("T-1/g1-c{hex}"));
        fs::create_dir_all(&snapshots_path).unwrap();
        fs::write(snapshots_path.join("pre-implement.snapshot.tsv"), b"x").unwrap();
    }

    fn setup(slug: &str) -> TempDir {
        let temp = TempDir::new().unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(temp.path())
            .status()
            .expect("git init must execute");
        assert!(status.success(), "git init must succeed");
        setup_root(&temp, slug);
        temp
    }

    // `GAL_TEST_FIRST_CLEANUP_FAIL_AT` is process-global. Every test in this module
    // — not just the ones that set it — acquires this lock first, so a parallel
    // `cargo test` thread can never observe another thread's injected checkpoint
    // value while running an unrelated, unguarded assertion.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    fn with_fail_at<T>(label: &str, body: impl FnOnce() -> T) -> T {
        std::env::set_var("GAL_TEST_FIRST_CLEANUP_FAIL_AT", label);
        let result = body();
        std::env::remove_var("GAL_TEST_FIRST_CLEANUP_FAIL_AT");
        result
    }

    #[test]
    fn finalized_cleanup_is_plan_scoped_and_reports_evidence() {
        let _guard = env_guard();
        let temp = setup("one");
        setup_root(&temp, "two");
        let evidence = cleanup_plan(
            temp.path(),
            &CleanupInput {
                plan_slug: "one".into(),
                finalized: true,
            },
        )
        .unwrap();
        assert_eq!(evidence.removed_roots.len(), 2);
        assert!(!temp.path().join(RECEIPTS_ROOT).join("one").exists());
        assert!(temp.path().join(RECEIPTS_ROOT).join("two").exists());
    }

    #[test]
    fn cleanup_requires_finalization_and_rejects_special_children() {
        let _guard = env_guard();
        let temp = setup("one");
        assert!(cleanup_plan(
            temp.path(),
            &CleanupInput {
                plan_slug: "one".into(),
                finalized: false
            }
        )
        .is_err());
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            "item",
            temp.path().join(RECEIPTS_ROOT).join("one").join("link"),
        )
        .unwrap();
        #[cfg(unix)]
        assert!(cleanup_plan(
            temp.path(),
            &CleanupInput {
                plan_slug: "one".into(),
                finalized: true
            }
        )
        .is_err());
    }

    #[test]
    fn resumed_cleanup_completes_after_injected_before_quarantine_failure() {
        let _guard = env_guard();
        let temp = setup("one");
        let input = CleanupInput {
            plan_slug: "one".into(),
            finalized: true,
        };

        let first = with_fail_at("before-quarantine", || cleanup_plan(temp.path(), &input));
        assert!(first.is_err(), "the injected failure must surface as Err");
        // The rename is injected before it runs: the source root must still be
        // present, untouched, and the journal must record the prepared row that
        // makes the resume path resolvable.
        assert!(temp.path().join(RECEIPTS_ROOT).join("one").exists());
        let journal = fs::read(journal_path(temp.path(), "one")).unwrap();
        assert!(parse_journal(&journal)
            .unwrap()
            .iter()
            .any(|record| record.state == "prepared"));

        let resumed = cleanup_plan(temp.path(), &input)
            .expect("resume must complete once the injected failure is gone");
        assert_eq!(resumed.removed_roots.len(), 2);
        assert!(!temp.path().join(RECEIPTS_ROOT).join("one").exists());
        assert!(!temp.path().join(SNAPSHOTS_ROOT).join("one").exists());
    }

    #[test]
    fn resumed_cleanup_completes_after_injected_before_removal_failure() {
        let _guard = env_guard();
        let temp = setup("one");
        let input = CleanupInput {
            plan_slug: "one".into(),
            finalized: true,
        };

        let first = with_fail_at("before-removal", || cleanup_plan(temp.path(), &input));
        assert!(first.is_err(), "the injected failure must surface as Err");
        // Quarantine (the rename) must have already completed before the injected
        // removal failure fires on the first enumerated child.
        assert!(!temp.path().join(RECEIPTS_ROOT).join("one").exists());
        let journal = fs::read(journal_path(temp.path(), "one")).unwrap();
        assert!(parse_journal(&journal)
            .unwrap()
            .iter()
            .any(|record| record.state == "quarantined"));

        let resumed = cleanup_plan(temp.path(), &input)
            .expect("resume must complete once the injected failure is gone");
        assert_eq!(resumed.removed_roots.len(), 2);
        assert!(!temp.path().join(SNAPSHOTS_ROOT).join("one").exists());
    }

    #[test]
    fn ambiguous_journal_blocks_cleanup() {
        let _guard = env_guard();
        let temp = setup("one");
        let journal = journal_path(temp.path(), "one");
        fs::create_dir_all(journal.parent().unwrap()).unwrap();
        let source = format!("{RECEIPTS_ROOT}/one");
        fs::write(
            &journal,
            format!("{source}\t.gal-cleanup-a\tprepared\n{source}\t.gal-cleanup-b\tprepared\n"),
        )
        .unwrap();
        let err = cleanup_plan(
            temp.path(),
            &CleanupInput {
                plan_slug: "one".into(),
                finalized: true,
            },
        )
        .unwrap_err();
        assert!(err.contains("ambiguous cleanup journal"), "{err}");
    }

    #[test]
    fn ambiguous_prepared_quarantine_state_blocks_cleanup() {
        let _guard = env_guard();
        let temp = setup("one");
        let journal = journal_path(temp.path(), "one");
        fs::create_dir_all(journal.parent().unwrap()).unwrap();
        let source = format!("{RECEIPTS_ROOT}/one");
        let quarantine = format!("{RECEIPTS_ROOT}/.gal-cleanup-ambiguous");
        fs::write(&journal, format!("{source}\t{quarantine}\tprepared\n")).unwrap();
        // Both the original source (from `setup`) and a stray quarantine target
        // exist: neither a clean pre-rename nor a clean post-rename state, so
        // recovery must refuse to guess which one is authoritative.
        fs::create_dir_all(temp.path().join(&quarantine)).unwrap();
        let err = cleanup_plan(
            temp.path(),
            &CleanupInput {
                plan_slug: "one".into(),
                finalized: true,
            },
        )
        .unwrap_err();
        assert!(
            err.contains("ambiguous prepared cleanup quarantine state"),
            "{err}"
        );
    }
}
