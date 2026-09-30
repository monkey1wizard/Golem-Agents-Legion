//! `gal pipeline-clean` — remove one closed plan runtime directory.

use super::dispatch::resolve_scope;
use gal_engine::ExitCode;
use gal_foundation::validated_repo_path::{ValidatedRepoPath, ValidatedRepoPathMode};
use std::path::{Path, PathBuf};
use std::process::Command;

fn repository_top() -> Result<PathBuf, String> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot determine working directory: {e}"))?;
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("cannot run git rev-parse --show-toplevel: {e}"))?;
    if !output.status.success() {
        return Err("git rev-parse --show-toplevel failed".into());
    }
    let top = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if top.is_empty() {
        return Err("git rev-parse --show-toplevel returned an empty path".into());
    }
    Ok(PathBuf::from(top))
}

fn refuse(err: &mut dyn std::io::Write, path: &Path, reason: impl std::fmt::Display) -> ExitCode {
    let _ = writeln!(err, "refused {}: {reason}", path.display());
    ExitCode::Error
}

pub(crate) fn cmd_pipeline_clean(args: &[String]) -> ExitCode {
    pipeline_clean_impl(args, &mut std::io::stdout(), &mut std::io::stderr())
}

fn pipeline_clean_impl(
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    if args.len() != 2 {
        let _ = writeln!(err, "usage: gal pipeline-clean <plan-or-prompt-path>");
        return ExitCode::Usage;
    }

    let source = Path::new(&args[1]);
    let top = match repository_top() {
        Ok(top) => top,
        Err(reason) => {
            let _ = writeln!(err, "gal pipeline-clean: {reason}");
            return ExitCode::Error;
        }
    };

    let scope = match resolve_scope(source) {
        Ok(scope) => scope,
        Err(reason) => {
            let target = top.join(".dev").join("pipeline").join(
                source
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown-plan"),
            );
            return refuse(err, &target, reason);
        }
    };
    let target = top.join(".dev").join("pipeline").join(&scope);

    for active in [
        top.join(".dev").join("plans").join(format!("{scope}.md")),
        top.join(".dev")
            .join("plans")
            .join(format!("{scope}.prompt.md")),
        top.join(".dev")
            .join("research")
            .join(format!("{scope}.md")),
    ] {
        if active.exists() {
            return refuse(
                err,
                &target,
                format!("active scope file exists: {}", active.display()),
            );
        }
    }
    if scope == "unknown-plan" {
        return refuse(err, &target, "reserved scope 'unknown-plan'");
    }

    if !target.exists() {
        let _ = writeln!(out, "already-absent {}", target.display());
        return ExitCode::Success;
    }

    let binding = match ValidatedRepoPath::new(
        &top,
        Path::new(&format!(".dev/pipeline/{scope}")),
        ValidatedRepoPathMode::ValidatedDirectChildDirectory {
            expected_parent: ".dev/pipeline".into(),
        },
    ) {
        Ok(binding) => binding,
        Err(error) => return refuse(err, &target, error),
    };
    if let Err(error) = binding.recheck() {
        return refuse(err, &target, error);
    }
    if let Err(error) = std::fs::remove_dir_all(binding.full_path()) {
        return refuse(err, &target, error);
    }
    let _ = writeln!(out, "removed {}", target.display());
    ExitCode::Success
}

#[cfg(test)]
mod tests {
    use super::*;
    use gal_foundation::platform::{create_dir_link, remove_dir_link};
    use std::fs;
    use tempfile::TempDir;

    struct CwdGuard {
        original: PathBuf,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl CwdGuard {
        fn enter(dir: &Path) -> Self {
            let lock = crate::commands::ENV_GUARD
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let original = std::env::current_dir().expect("get current dir");
            std::env::set_current_dir(dir).expect("set current dir");
            Self {
                original,
                _lock: lock,
            }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    fn init_test_repo(tmp: &TempDir) -> PathBuf {
        let r = tmp.path().to_path_buf();
        let status = Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(&r)
            .status()
            .or_else(|_| Command::new("git").args(["init"]).current_dir(&r).status())
            .expect("git init");
        assert!(status.success(), "git init must succeed");
        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&r)
            .status();
        let _ = Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&r)
            .status();
        r
    }

    fn setup_sibling_scope(repo: &Path) -> (PathBuf, Vec<u8>) {
        let sibling_dir = repo.join(".dev").join("pipeline").join("sibling-scope");
        fs::create_dir_all(&sibling_dir).expect("create sibling dir");
        let sibling_file = sibling_dir.join("sibling.txt");
        let sibling_content = b"sibling invariant payload\n".to_vec();
        fs::write(&sibling_file, &sibling_content).expect("write sibling file");
        (sibling_file, sibling_content)
    }

    fn assert_sibling_unchanged(sibling_file: &Path, expected: &[u8]) {
        assert!(sibling_file.exists(), "sibling file must exist");
        let actual = fs::read(sibling_file).expect("read sibling file");
        assert_eq!(actual, expected, "sibling scope must be byte-identical");
    }

    #[test]
    fn tp_09_missing_argument_is_usage() {
        assert_eq!(
            cmd_pipeline_clean(&["pipeline-clean".into()]),
            ExitCode::Usage
        );
    }

    #[test]
    fn tp_09_extra_argument_is_usage() {
        assert_eq!(
            cmd_pipeline_clean(&["pipeline-clean".into(), "a.md".into(), "extra".into()]),
            ExitCode::Usage
        );
    }

    #[test]
    fn tp_09_removed() {
        let tmp = TempDir::new().unwrap();
        let repo = init_test_repo(&tmp);
        let (sib_file, sib_data) = setup_sibling_scope(&repo);

        let target_dir = repo.join(".dev").join("pipeline").join("sample-plan");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(target_dir.join("log.txt"), b"runtime data").unwrap();

        let _guard = CwdGuard::enter(&repo);
        let exit = cmd_pipeline_clean(&[
            "pipeline-clean".into(),
            ".dev/plans/sample-plan.prompt.md".into(),
        ]);
        assert_eq!(exit, ExitCode::Success);
        assert!(!target_dir.exists(), "target scope must be removed");
        assert_sibling_unchanged(&sib_file, &sib_data);
    }

    #[test]
    fn tp_09_already_absent() {
        let tmp = TempDir::new().unwrap();
        let repo = init_test_repo(&tmp);
        let (sib_file, sib_data) = setup_sibling_scope(&repo);

        let target_dir = repo.join(".dev").join("pipeline").join("sample-plan");
        assert!(!target_dir.exists());

        let _guard = CwdGuard::enter(&repo);
        let exit = cmd_pipeline_clean(&[
            "pipeline-clean".into(),
            ".dev/plans/sample-plan.prompt.md".into(),
        ]);
        assert_eq!(exit, ExitCode::Success);
        assert!(!target_dir.exists());
        assert_sibling_unchanged(&sib_file, &sib_data);
    }

    #[test]
    fn tp_09_refused_while_plan_prompt_or_research_file_exists() {
        let tmp = TempDir::new().unwrap();
        let repo = init_test_repo(&tmp);
        let (sib_file, sib_data) = setup_sibling_scope(&repo);

        let target_dir = repo.join(".dev").join("pipeline").join("sample-plan");
        fs::create_dir_all(&target_dir).unwrap();

        let plans_dir = repo.join(".dev").join("plans");
        let research_dir = repo.join(".dev").join("research");
        fs::create_dir_all(&plans_dir).unwrap();
        fs::create_dir_all(&research_dir).unwrap();

        let _guard = CwdGuard::enter(&repo);

        // Branch: plan file exists
        let plan_file = plans_dir.join("sample-plan.md");
        fs::write(&plan_file, "# Plan").unwrap();
        let exit = cmd_pipeline_clean(&[
            "pipeline-clean".into(),
            ".dev/plans/sample-plan.prompt.md".into(),
        ]);
        assert_eq!(exit, ExitCode::Error);
        assert!(target_dir.exists());
        assert_sibling_unchanged(&sib_file, &sib_data);
        fs::remove_file(&plan_file).unwrap();

        // Branch: prompt file exists
        let prompt_file = plans_dir.join("sample-plan.prompt.md");
        fs::write(&prompt_file, "# Prompt").unwrap();
        let exit = cmd_pipeline_clean(&[
            "pipeline-clean".into(),
            ".dev/plans/sample-plan.prompt.md".into(),
        ]);
        assert_eq!(exit, ExitCode::Error);
        assert!(target_dir.exists());
        assert_sibling_unchanged(&sib_file, &sib_data);
        fs::remove_file(&prompt_file).unwrap();

        // Branch: research file exists
        let research_file = research_dir.join("sample-plan.md");
        fs::write(&research_file, "# Research").unwrap();
        let exit = cmd_pipeline_clean(&[
            "pipeline-clean".into(),
            ".dev/plans/sample-plan.prompt.md".into(),
        ]);
        assert_eq!(exit, ExitCode::Error);
        assert!(target_dir.exists());
        assert_sibling_unchanged(&sib_file, &sib_data);
        fs::remove_file(&research_file).unwrap();
    }

    #[test]
    fn tp_09_refused_for_unknown_plan_20260915_and_locks() {
        let tmp = TempDir::new().unwrap();
        let repo = init_test_repo(&tmp);
        let (sib_file, sib_data) = setup_sibling_scope(&repo);

        let _guard = CwdGuard::enter(&repo);

        // unknown-plan
        let exit =
            cmd_pipeline_clean(&["pipeline-clean".into(), ".dev/plans/unknown-plan.md".into()]);
        assert_eq!(exit, ExitCode::Error);
        assert_sibling_unchanged(&sib_file, &sib_data);

        // 20260915 (8-digit date root)
        let exit = cmd_pipeline_clean(&["pipeline-clean".into(), ".dev/plans/20260915.md".into()]);
        assert_eq!(exit, ExitCode::Error);
        assert_sibling_unchanged(&sib_file, &sib_data);

        // .locks
        let exit = cmd_pipeline_clean(&["pipeline-clean".into(), ".dev/plans/.locks.md".into()]);
        assert_eq!(exit, ExitCode::Error);
        assert_sibling_unchanged(&sib_file, &sib_data);
    }

    #[test]
    fn tp_09_refused_for_symlink_component_junction() {
        let tmp = TempDir::new().unwrap();
        let repo = init_test_repo(&tmp);
        let (sib_file, sib_data) = setup_sibling_scope(&repo);

        let real_target = tmp.path().join("real_target_dir");
        fs::create_dir_all(&real_target).unwrap();
        fs::write(real_target.join("precious.txt"), b"do not delete").unwrap();

        let link_target = repo.join(".dev").join("pipeline").join("sym-scope");
        fs::create_dir_all(repo.join(".dev").join("pipeline")).unwrap();
        create_dir_link(&real_target, &link_target).expect("create junction or symlink");

        let _guard = CwdGuard::enter(&repo);
        let exit = cmd_pipeline_clean(&[
            "pipeline-clean".into(),
            ".dev/plans/sym-scope.prompt.md".into(),
        ]);
        assert_eq!(exit, ExitCode::Error);
        assert!(real_target.exists(), "real target dir must not be deleted");
        assert!(
            real_target.join("precious.txt").exists(),
            "target content preserved"
        );
        assert_sibling_unchanged(&sib_file, &sib_data);

        let _ = remove_dir_link(&link_target);
    }

    #[test]
    fn tp_09_subdirectory_run_prints_absolute_path_under_repo_top() {
        let tmp = TempDir::new().unwrap();
        let repo = init_test_repo(&tmp);
        let (sib_file, sib_data) = setup_sibling_scope(&repo);

        let subdir = repo.join("crates").join("some_sub");
        fs::create_dir_all(&subdir).unwrap();

        let target_dir = repo.join(".dev").join("pipeline").join("sub-scope");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(target_dir.join("file.txt"), b"hello").unwrap();

        let _guard = CwdGuard::enter(&subdir);

        let mut out = Vec::new();
        let mut err = Vec::new();
        let exit = pipeline_clean_impl(
            &[
                "pipeline-clean".into(),
                ".dev/plans/sub-scope.prompt.md".into(),
            ],
            &mut out,
            &mut err,
        );
        assert_eq!(exit, ExitCode::Success);
        let out_str = String::from_utf8(out).unwrap();
        assert!(out_str.starts_with("removed "));
        let printed_path_str = out_str.strip_prefix("removed ").unwrap().trim();
        let printed_path = PathBuf::from(printed_path_str);
        assert!(
            printed_path.is_absolute(),
            "printed path must be absolute: {printed_path_str}"
        );
        assert_eq!(
            printed_path.to_string_lossy().replace('\\', "/"),
            repo.join(".dev")
                .join("pipeline")
                .join("sub-scope")
                .to_string_lossy()
                .replace('\\', "/"),
            "printed path must match repository top level path"
        );
        assert!(!target_dir.exists());
        assert_sibling_unchanged(&sib_file, &sib_data);
    }
}
