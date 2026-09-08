//! Deterministic implementation-commit gate.

use super::test_first_fs;
use gal_foundation::validated_repo_path::{ValidatedRepoPath, ValidatedRepoPathMode};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct CommitInput {
    pub plan_slug: String,
    pub task_id: String,
    pub base_commit: String,
    pub production_paths: Vec<PathBuf>,
    pub test_paths: Vec<PathBuf>,
    pub correctness_receipt: PathBuf,
    pub auditor_receipt: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitEvidence {
    pub base_commit: String,
    pub new_commit: String,
    pub parent: String,
    pub range: String,
    pub dirty_set: Vec<String>,
    pub cached_hashes: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StatusEntry {
    path: String,
    status: [u8; 2],
}

fn git(repo: &Path, args: &[&str], stdin: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(repo)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git command failed to spawn: {e}"))?;
    if let Some(bytes) = stdin {
        child
            .stdin
            .take()
            .ok_or("git stdin unavailable")?
            .write_all(bytes)
            .map_err(|e| format!("cannot write git stdin: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("git command failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn git_text(repo: &Path, args: &[&str]) -> Result<String, String> {
    String::from_utf8(git(repo, args, None)?)
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("git output is not UTF-8: {e}"))
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Validated-read the receipt at `path` with a double identity recheck
/// bracketing the byte read (before and after), matching the same
/// read-then-recheck discipline `converge_check::read_validated_bytes` uses
/// for evidence artifacts elsewhere in the test-first pipeline.
fn read_receipt_text(repo: &Path, path: &Path) -> Result<String, String> {
    let binding = ValidatedRepoPath::new(repo, path, ValidatedRepoPathMode::RegularFileOrMissing)
        .map_err(|e| format!("receipt path validation failed: {e}"))?;
    if !binding.exists() {
        return Err(format!("missing required receipt: {}", path.display()));
    }
    binding
        .recheck()
        .map_err(|e| format!("receipt identity changed: {e}"))?;
    let bytes = fs::read(binding.full_path()).map_err(|e| format!("cannot read receipt: {e}"))?;
    binding
        .recheck()
        .map_err(|e| format!("receipt identity changed while reading: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("receipt is not UTF-8: {e}"))
}

fn validate_receipt(repo: &Path, path: &Path, required: &str) -> Result<(), String> {
    let text = read_receipt_text(repo, path)?;
    if !text.lines().any(|line| line.trim() == required) {
        return Err(format!(
            "receipt does not contain required pass marker: {required}"
        ));
    }
    Ok(())
}

fn validate_receipt_any(repo: &Path, path: &Path, required: &[&str]) -> Result<(), String> {
    let text = read_receipt_text(repo, path)?;
    if !text
        .lines()
        .any(|line| required.iter().any(|value| line.trim() == *value))
    {
        return Err(format!(
            "receipt does not contain a required pass marker: {required:?}"
        ));
    }
    Ok(())
}

fn parse_status(bytes: &[u8]) -> Result<Vec<StatusEntry>, String> {
    let mut entries = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if index + 3 > bytes.len() {
            return Err("truncated NUL-delimited Git status".to_string());
        }
        let status = [bytes[index], bytes[index + 1]];
        if bytes[index + 2] != b' ' {
            return Err("malformed Git status record".to_string());
        }
        index += 3;
        let end = bytes[index..]
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("unterminated Git status path")?
            + index;
        let path = String::from_utf8(bytes[index..end].to_vec())
            .map_err(|e| format!("Git path is not UTF-8: {e}"))?;
        entries.push(StatusEntry { path, status });
        index = end + 1;
        if status[0] == b'R' || status[0] == b'C' || status[1] == b'R' || status[1] == b'C' {
            let end = bytes[index..]
                .iter()
                .position(|byte| *byte == 0)
                .ok_or("unterminated Git rename target")?
                + index;
            index = end + 1;
        }
    }
    Ok(entries)
}

fn parse_cached_names(bytes: &[u8]) -> Result<BTreeSet<String>, String> {
    let mut names = BTreeSet::new();
    let records: Vec<&[u8]> = bytes.split(|byte| *byte == 0).collect();
    let mut index = 0;
    while index + 1 < records.len() {
        if records[index].is_empty() {
            index += 1;
            continue;
        }
        let path = if let Some(separator) = records[index].iter().position(|byte| *byte == b'\t') {
            &records[index][separator + 1..]
        } else {
            index += 1;
            records
                .get(index)
                .ok_or("malformed cached Git name-status record")?
        };
        names.insert(
            String::from_utf8(path.to_vec())
                .map_err(|e| format!("cached Git path is not UTF-8: {e}"))?,
        );
        index += 1;
    }
    Ok(names)
}

fn normalize_paths(paths: &[PathBuf]) -> Result<BTreeSet<String>, String> {
    let mut result = BTreeSet::new();
    for path in paths {
        if path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(format!("invalid implementation path: {}", path.display()));
        }
        let normalized = path.to_string_lossy().replace('\\', "/");
        if normalized.is_empty() || normalized == "." || normalized.contains('*') {
            return Err(format!("invalid implementation path: {normalized}"));
        }
        result.insert(normalized);
    }
    Ok(result)
}

fn validate_and_bind_paths(repo: &Path, paths: &BTreeSet<String>) -> Result<(), String> {
    for (index, path) in paths.iter().enumerate() {
        let relative = Path::new(path);
        let binding =
            ValidatedRepoPath::new(repo, relative, ValidatedRepoPathMode::RegularFileOrMissing)
                .map_err(|e| format!("path validation failed for {path}: {e}"))?;
        if !binding.exists() {
            continue;
        }
        binding
            .recheck()
            .map_err(|e| format!("path identity changed for {path}: {e}"))?;
        let candidate = relative
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(".gal-commit-{}-{index}", std::process::id()));
        let _ = fs::remove_file(repo.join(&candidate));
        let bytes =
            fs::read(binding.full_path()).map_err(|e| format!("cannot read {path}: {e}"))?;
        let mode = fs::metadata(binding.full_path())
            .map_err(|e| format!("cannot read mode for {path}: {e}"))?
            .permissions();
        test_first_fs::atomic_publish_bytes(&repo.join(&candidate), &bytes)?;
        fs::set_permissions(repo.join(&candidate), mode)
            .map_err(|e| format!("cannot preserve mode for {path}: {e}"))?;
        let candidate_binding = ValidatedRepoPath::new(
            repo,
            &candidate,
            ValidatedRepoPathMode::RegularFileOrMissing,
        )
        .map_err(|e| format!("candidate validation failed for {path}: {e}"))?;
        let replacement = binding
            .bind_replacement_candidate(candidate_binding)
            .map_err(|e| format!("replacement binding failed for {path}: {e}"))?;
        fs::remove_file(replacement.target_path())
            .map_err(|e| format!("cannot replace {path}: {e}"))?;
        fs::rename(replacement.candidate_path(), replacement.target_path())
            .map_err(|e| format!("cannot publish {path}: {e}"))?;
        replacement
            .verify()
            .map_err(|e| format!("replacement verification failed for {path}: {e}"))?;
    }
    Ok(())
}

/// Snapshots each declared path's **worktree** bytes immediately after staging.
///
/// Deliberately hashes the worktree bytes rather than the staged blob
/// (`git show :path`). A clean filter — `core.autocrlf`, or any `filter=`
/// gitattribute — rewrites content on the way into the index, so an
/// index-versus-worktree digest comparison reports a difference for every
/// filtered file even when nothing actually changed. The invariant this
/// function and `verify_cached_bytes` jointly guard is temporal: nothing
/// mutated the declared paths between `git add` and the commit. Both sides
/// therefore read the same representation through the same validated binding.
fn cached_hashes(repo: &Path, paths: &BTreeSet<String>) -> Result<Vec<(String, String)>, String> {
    let mut result = Vec::new();
    for path in paths {
        if git(repo, &["ls-files", "--stage", "--", path], None)?.is_empty() {
            result.push((path.clone(), "-".to_string()));
        } else {
            let binding = ValidatedRepoPath::new(
                repo,
                Path::new(path),
                ValidatedRepoPathMode::RegularFileOrMissing,
            )
            .map_err(|e| format!("cached path validation failed for {path}: {e}"))?;
            let bytes =
                fs::read(binding.full_path()).map_err(|e| format!("cannot read {path}: {e}"))?;
            result.push((path.clone(), digest(&bytes)));
        }
    }
    Ok(result)
}

fn verify_cached_bytes(
    repo: &Path,
    paths: &BTreeSet<String>,
    cached: &[(String, String)],
) -> Result<(), String> {
    for (path, expected) in cached {
        let binding = ValidatedRepoPath::new(
            repo,
            Path::new(path),
            ValidatedRepoPathMode::RegularFileOrMissing,
        )
        .map_err(|e| format!("cached path validation failed for {path}: {e}"))?;
        if expected == "-" {
            if binding.exists() {
                return Err(format!("deleted path reappeared before commit: {path}"));
            }
            continue;
        }
        binding
            .recheck()
            .map_err(|e| format!("cached path identity changed for {path}: {e}"))?;
        let bytes =
            fs::read(binding.full_path()).map_err(|e| format!("cannot read {path}: {e}"))?;
        binding
            .recheck()
            .map_err(|e| format!("cached path identity changed while reading {path}: {e}"))?;
        if digest(&bytes) != *expected {
            return Err(format!("cached bytes differ for {path}"));
        }
    }
    if cached.len() != paths.len() {
        return Err("cached evidence does not cover the declared path set".to_string());
    }
    Ok(())
}

/// Pass markers this gate accepts on an auditor receipt, compared as whole
/// trimmed lines.
///
/// The first entry is what `golem-auditor.agent.md` actually instructs the
/// auditor to write. Its absence is why the gate could never have accepted a
/// real auditor receipt: the other three are shapes no GAL contract emits, so
/// every whole-line comparison failed. They are kept because other producers
/// may already emit them, and one named constant is the only place this set is
/// written down.
const AUDITOR_PASS_MARKERS: &[&str] = &[
    "<!-- AUDIT_REVIEW: CLEAR -->",
    "Verdict: APPROVE",
    "verdict: pass",
    "overall: pass",
];

pub fn commit_implementation(repo: &Path, input: &CommitInput) -> Result<CommitEvidence, String> {
    validate_receipt(repo, &input.correctness_receipt, "overall: pass")?;
    validate_receipt_any(repo, &input.auditor_receipt, AUDITOR_PASS_MARKERS)?;
    let head = git_text(repo, &["rev-parse", "HEAD"])?;
    if head != input.base_commit {
        return Err(format!(
            "stale base: expected {}, found {head}",
            input.base_commit
        ));
    }
    let allowed = normalize_paths(
        &input
            .production_paths
            .iter()
            .chain(&input.test_paths)
            .cloned()
            .collect::<Vec<_>>(),
    )?;
    let status = parse_status(&git(
        repo,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        None,
    )?)?;
    if status
        .iter()
        .any(|entry| entry.status[0] != b' ' && entry.status[0] != b'?')
    {
        return Err("index is not empty at implementation-commit entry".to_string());
    }
    let dirty: BTreeSet<String> = status.iter().map(|entry| entry.path.clone()).collect();
    if dirty != allowed {
        return Err(format!(
            "dirty set differs from declared paths: {:?} != {:?}",
            dirty, allowed
        ));
    }
    validate_and_bind_paths(repo, &allowed)?;
    let mut pathspec = Vec::new();
    for path in &allowed {
        pathspec.extend_from_slice(path.as_bytes());
        pathspec.push(0);
    }
    git(
        repo,
        &["add", "--pathspec-from-file=-", "--pathspec-file-nul", "--"],
        Some(&pathspec),
    )?;
    let cached = cached_hashes(repo, &allowed)?;
    verify_cached_bytes(repo, &allowed, &cached)?;
    let cached_set = parse_cached_names(&git(
        repo,
        &["diff", "--cached", "--no-renames", "--name-status", "-z"],
        None,
    )?)?;
    if cached_set != allowed {
        return Err(format!(
            "cached set differs from declared paths: {cached_set:?} != {allowed:?}"
        ));
    }
    let parent = git_text(repo, &["rev-parse", "HEAD"])?;
    if parent != input.base_commit {
        return Err("HEAD changed during commit preparation".to_string());
    }
    let _ = git_text(
        repo,
        &[
            "commit",
            "--no-verify",
            "--no-gpg-sign",
            "-m",
            &input.message,
        ],
    )?;
    let new_commit = git_text(repo, &["rev-parse", "HEAD"])?;
    let actual_parent = git_text(repo, &["rev-parse", "HEAD^"])?;
    if actual_parent != parent {
        return Err("new commit has the wrong parent".to_string());
    }
    let range = format!("{parent}..{new_commit}");
    let range_paths = git(
        repo,
        &[
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-renames",
            "--name-only",
            "-z",
            &range,
        ],
        None,
    )?;
    let range_set: BTreeSet<String> = range_paths
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec()).map_err(|e| format!("range path is not UTF-8: {e}"))
        })
        .collect::<Result<_, _>>()?;
    if range_set != allowed {
        return Err(format!(
            "commit range differs from declared paths: {range_set:?} != {allowed:?}"
        ));
    }
    // `git diff --quiet` / `git diff --cached --quiet` only see tracked-content
    // changes — an untracked file left behind (e.g. by a post-commit hook, a
    // build artifact, or any other side effect during the commit itself) is
    // invisible to both and would pass silently. `status --porcelain` with
    // `--untracked-files=all` catches tracked AND untracked dirt in one check.
    let final_status = git(
        repo,
        &["status", "--porcelain", "--untracked-files=all"],
        None,
    )?;
    if !final_status.is_empty() {
        return Err(format!(
            "worktree or index is dirty after implementation commit: {}",
            String::from_utf8_lossy(&final_status).trim()
        ));
    }
    Ok(CommitEvidence {
        base_commit: input.base_commit.clone(),
        new_commit,
        parent,
        range,
        dirty_set: allowed.into_iter().collect(),
        cached_hashes: cached,
    })
}

fn required_arg(args: &[String], name: &str) -> Result<String, String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .ok_or_else(|| format!("missing required option {name}"))
}

fn repeated_args(args: &[String], names: &[&str]) -> Vec<PathBuf> {
    args.windows(2)
        .filter(|pair| names.contains(&pair[0].as_str()))
        .map(|pair| PathBuf::from(&pair[1]))
        .collect()
}

fn receipt_text(input: &CommitInput, evidence: &CommitEvidence) -> String {
    let dirty_set = evidence.dirty_set.join(",");
    let cached_hashes = evidence
        .cached_hashes
        .iter()
        .map(|(path, hash)| format!("{path}={hash}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "kind=test-first-commit\nplan_slug={}\ntask_id={}\nbase_commit={}\nnew_commit={}\nparent={}\nrange={}\ndirty_set={}\ncached_hashes={}\noverall: pass\n",
        input.plan_slug,
        input.task_id,
        evidence.base_commit,
        evidence.new_commit,
        evidence.parent,
        evidence.range,
        dirty_set,
        cached_hashes,
    )
}

/// Run `gal test-first-commit run ...` through the production CLI surface.
pub(crate) fn cmd_test_first_commit(args: &[String]) -> gal_engine::ExitCode {
    let result = (|| -> Result<(), String> {
        if args.get(1).map(String::as_str) != Some("run") {
            return Err(
                "usage: gal test-first-commit run --plan <slug> --task <id> \
                 --base <commit> --production <path>... [--test <path>...] \
                 --correctness-receipt <path> --auditor-receipt <path> \
                 --receipt <path> [--message <message>]"
                    .to_string(),
            );
        }
        let repo =
            std::env::current_dir().map_err(|e| format!("cannot resolve repository: {e}"))?;
        let production_paths = repeated_args(args, &["--production", "--production-path"]);
        if production_paths.is_empty() {
            return Err("missing required option --production".to_string());
        }
        let input = CommitInput {
            plan_slug: required_arg(args, "--plan")?,
            task_id: required_arg(args, "--task")?,
            base_commit: required_arg(args, "--base")?,
            production_paths,
            test_paths: repeated_args(args, &["--test", "--test-path"]),
            correctness_receipt: PathBuf::from(required_arg(args, "--correctness-receipt")?),
            auditor_receipt: PathBuf::from(required_arg(args, "--auditor-receipt")?),
            message: args
                .windows(2)
                .find(|pair| pair[0] == "--message")
                .map(|pair| pair[1].clone())
                .unwrap_or_else(|| {
                    format!(
                        "test-first: implement {}",
                        required_arg(args, "--task").unwrap_or_default()
                    )
                }),
        };
        let receipt = PathBuf::from(required_arg(args, "--receipt")?);
        // Validate the destination before the commit, not after. A rejected path
        // discovered post-commit would leave a landed commit with no receipt,
        // which is exactly the unevidenced state the gate exists to prevent.
        test_first_fs::validate_receipt_destination(&repo, &receipt)?;
        let evidence = commit_implementation(&repo, &input)?;
        test_first_fs::publish_receipt(
            &repo,
            &receipt,
            receipt_text(&input, &evidence).as_bytes(),
        )?;
        println!("receipt={}", receipt.display());
        println!("new_commit={}", evidence.new_commit);
        Ok(())
    })();

    match result {
        Ok(()) => gal_engine::ExitCode::Success,
        Err(error) => {
            eprintln!("test-first-commit: {error}");
            if args.get(1).map(String::as_str) == Some("run") {
                gal_engine::ExitCode::Error
            } else {
                gal_engine::ExitCode::Usage
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn run(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git must spawn");
        assert!(output.status.success(), "git failed: {:?}", output);
    }

    fn fixture() -> (TempDir, String) {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        run(repo, &["init", "--quiet"]);
        run(repo, &["config", "user.email", "test@example.invalid"]);
        run(repo, &["config", "user.name", "test"]);
        fs::create_dir_all(repo.join(".dev/pipeline/receipts")).unwrap();
        fs::write(repo.join(".gitignore"), ".dev/\n").unwrap();
        fs::write(repo.join("base.txt"), "base\n").unwrap();
        run(repo, &["add", "."]);
        run(
            repo,
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--no-verify",
                "--no-gpg-sign",
                "--quiet",
                "-m",
                "base",
            ],
        );
        let base = git_text(repo, &["rev-parse", "HEAD"]).unwrap();
        (temp, base)
    }

    fn input(base: &str, path: &str) -> CommitInput {
        CommitInput {
            plan_slug: "plan".to_string(),
            task_id: "task".to_string(),
            base_commit: base.to_string(),
            production_paths: vec![PathBuf::from(path)],
            test_paths: Vec::new(),
            correctness_receipt: PathBuf::from(".dev/pipeline/receipts/correctness.md"),
            auditor_receipt: PathBuf::from(".dev/pipeline/receipts/audit.md"),
            message: "implementation".to_string(),
        }
    }

    fn write_receipts(repo: &Path) {
        fs::write(
            repo.join(".dev/pipeline/receipts/correctness.md"),
            "overall: pass\n",
        )
        .unwrap();
        fs::write(
            repo.join(".dev/pipeline/receipts/audit.md"),
            "Verdict: APPROVE\n",
        )
        .unwrap();
    }

    #[test]
    fn commit_gate_accepts_literal_metacharacter_and_non_ascii_path() {
        let (temp, base) = fixture();
        let path = "odd;[x] 東京.txt";
        fs::write(temp.path().join(path), "new bytes\n").unwrap();
        write_receipts(temp.path());
        let evidence = commit_implementation(temp.path(), &input(&base, path)).unwrap();
        assert_eq!(evidence.parent, base);
        assert_eq!(evidence.dirty_set, vec![path]);
        assert!(git(temp.path(), &["status", "--porcelain"], None)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn commit_gate_rejects_missing_receipt_and_outside_dirtiness() {
        let (temp, base) = fixture();
        let path = "allowed.txt";
        fs::write(temp.path().join(path), "new\n").unwrap();
        let missing = commit_implementation(temp.path(), &input(&base, path));
        assert!(missing.is_err());
        fs::write(temp.path().join("outside.txt"), "outside\n").unwrap();
        write_receipts(temp.path());
        let outside = commit_implementation(temp.path(), &input(&base, path));
        assert!(outside.is_err());
        assert_eq!(git_text(temp.path(), &["rev-parse", "HEAD"]).unwrap(), base);
    }

    #[test]
    fn cached_name_status_parser_is_nul_delimited() {
        let paths =
            parse_cached_names(b"M\0odd;[x] \xe6\x9d\xb1\xe4\xba\xac.txt\0A\0new.txt\0").unwrap();
        assert!(paths.contains("odd;[x] 東京.txt"));
        assert!(paths.contains("new.txt"));
    }

    #[test]
    fn commit_gate_rejects_stale_base() {
        let (temp, base) = fixture();
        let path = "allowed.txt";
        fs::write(temp.path().join(path), "new\n").unwrap();
        write_receipts(temp.path());
        let stale_base = "0".repeat(40);
        let result = commit_implementation(temp.path(), &input(&stale_base, path));
        let error = result.expect_err("a stale declared base must be rejected");
        assert!(error.contains("stale base"), "{error}");
        assert_eq!(
            git_text(temp.path(), &["rev-parse", "HEAD"]).unwrap(),
            base,
            "a rejected stale-base call must never move HEAD"
        );
    }

    #[test]
    fn commit_gate_preserves_file_mode_through_atomic_republish() {
        let (temp, base) = fixture();
        let path = "mode.txt";
        let full = temp.path().join(path);
        fs::write(&full, "mode content\n").unwrap();
        let mut perms = fs::metadata(&full).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&full, perms).unwrap();
        write_receipts(temp.path());

        let evidence = commit_implementation(temp.path(), &input(&base, path)).unwrap();

        assert_eq!(evidence.dirty_set, vec![path]);
        let after = fs::metadata(&full).unwrap().permissions();
        assert!(
            after.readonly(),
            "the atomic candidate republish/replace must preserve the original file mode"
        );
        // No permission restore needed: TempDir removal deletes directory entries,
        // which is governed by the parent directory's own write permission, not
        // the leaf file's readonly bit.
    }

    #[test]
    fn commit_gate_surfaces_git_failure_without_moving_head() {
        let (temp, base) = fixture();
        let path = "allowed.txt";
        fs::write(temp.path().join(path), "new\n").unwrap();
        write_receipts(temp.path());
        // Simulate a genuine Git failure (e.g. a concurrent Git process holding the
        // repo lock) deterministically: a stale index.lock makes every subsequent
        // `git add`/`git commit` invocation fail the same way a real lock
        // contention would, without depending on environment-specific config.
        fs::write(temp.path().join(".git/index.lock"), b"").unwrap();

        let result = commit_implementation(temp.path(), &input(&base, path));

        assert!(result.is_err(), "a genuine Git failure must surface as Err");
        fs::remove_file(temp.path().join(".git/index.lock")).unwrap();
        assert_eq!(
            git_text(temp.path(), &["rev-parse", "HEAD"]).unwrap(),
            base,
            "HEAD must not move when an underlying Git operation fails"
        );
    }

    #[test]
    fn commit_gate_rejects_post_commit_dirtiness_left_by_a_hook() {
        let (temp, base) = fixture();
        let path = "allowed.txt";
        fs::write(temp.path().join(path), "new\n").unwrap();
        write_receipts(temp.path());

        // A repo-local `core.hooksPath` always overrides any machine-wide
        // `core.hooksPath` (e.g. this very repo's own naming-gate hook) for
        // this repo, so the post-commit hook below is guaranteed to be the
        // one Git actually runs, regardless of the host machine's global
        // config.
        run(temp.path(), &["config", "core.hooksPath", ".git/hooks"]);
        let hook_dir = temp.path().join(".git/hooks");
        fs::create_dir_all(&hook_dir).unwrap();
        let hook_path = hook_dir.join("post-commit");
        fs::write(&hook_path, "#!/bin/sh\necho dirty > post_dirty.txt\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&hook_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&hook_path, perms).unwrap();
        }

        let result = commit_implementation(temp.path(), &input(&base, path));

        assert!(
            temp.path().join("post_dirty.txt").exists(),
            "the post-commit hook must actually have run for this test to prove anything \
             — an untested escape hatch here would hide the real check"
        );
        let err =
            result.expect_err("an untracked file left by a post-commit hook must be rejected");
        assert!(err.contains("dirty after implementation commit"), "{err}");
    }
}
