//! Real-CLI source-resolution checks with a copied executable and isolated home.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

struct Fixture {
    temp: TempDir,
    binary: PathBuf,
    blocked_home: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let binary_dir = temp.path().join("bin");
        fs::create_dir_all(&binary_dir).unwrap();
        let binary = binary_dir.join(format!("gal{}", std::env::consts::EXE_SUFFIX));
        fs::copy(env!("CARGO_BIN_EXE_gal"), &binary).unwrap();
        let blocked_home = temp.path().join("blocked-home");
        fs::write(&blocked_home, b"not a directory").unwrap();
        Self {
            temp,
            binary,
            blocked_home,
        }
    }

    fn directory(&self, name: &str) -> PathBuf {
        let path = self.temp.path().join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn prompt(&self, repo: &Path) -> PathBuf {
        let path = repo.join("plan.prompt.md");
        let task = format!("T-{:02}", 1);
        fs::write(
            &path,
            format!("# Plan\n\n## Tasks\n\n- [ ] {task} - Test.\n"),
        )
        .unwrap();
        path
    }

    fn run(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(&self.binary)
            .current_dir(cwd)
            .env("USERPROFILE", &self.blocked_home)
            .env("HOME", &self.blocked_home)
            .args(args)
            .output()
            .unwrap()
    }
}

fn source_root(path: &Path, contract: Option<&str>) {
    let agents = path.join("agents");
    fs::create_dir_all(&agents).unwrap();
    fs::create_dir_all(path.join("skills")).unwrap();
    fs::create_dir_all(path.join("commands")).unwrap();
    if let Some(body) = contract {
        fs::write(agents.join("golem-implementer.agent.md"), body).unwrap();
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn pipeline_reports_embedded_materialization_io_failure() {
    let fixture = Fixture::new();
    let repo = fixture.directory("isolated-repo");
    let prompt = fixture.prompt(&repo);
    let output = fixture.run(&repo, &["pipeline", prompt.to_str().unwrap()]);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(1), "{error}");
    assert!(
        error.contains("embedded agent-contract source materialization failed"),
        "{error}"
    );
    assert!(
        error.contains("create embedded source staging directory"),
        "{error}"
    );
    assert!(
        error.contains(&fixture.blocked_home.display().to_string()),
        "{error}"
    );
    assert!(
        !error.contains("no GAL agent-contract source root is available"),
        "{error}"
    );
    assert!(!fixture.blocked_home.join(".gal").exists());
}

#[test]
fn init_refresh_and_render_adapters_preserve_materialization_reason() {
    let fixture = Fixture::new();
    for (name, args, marker) in [
        (
            "init-repo",
            vec!["init"],
            "Could not materialize the embedded GAL source",
        ),
        (
            "refresh-repo",
            vec!["refresh"],
            "embedded source materialization failed",
        ),
        (
            "render-repo",
            vec!["render-adapters"],
            "could not materialize embedded GAL source",
        ),
    ] {
        let repo = fixture.directory(name);
        if name == "render-repo" {
            fs::create_dir_all(repo.join(".dev")).unwrap();
            fs::write(repo.join(".dev/project.md"), "# Fixture\n").unwrap();
        }
        let output = fixture.run(&repo, &args);
        let error = stderr(&output);
        assert!(!output.status.success(), "{name} unexpectedly succeeded");
        assert!(error.contains(marker), "{name}: {error}");
        assert!(
            error.contains("create embedded source staging directory"),
            "{name}: {error}"
        );
        assert!(
            error.contains(&fixture.blocked_home.display().to_string()),
            "{name}: {error}"
        );
    }
    assert!(!fixture.blocked_home.join(".gal").exists());
}

#[test]
fn pipeline_corrupt_workdir_source_never_falls_back() {
    let fixture = Fixture::new();
    let repo = fixture.directory("workdir-repo");
    source_root(&repo.join("plugins/gal-core"), None);
    source_root(fixture.binary.parent().unwrap(), Some("exe-side contract"));
    let prompt = fixture.prompt(&repo);
    let output = fixture.run(&repo, &["pipeline", prompt.to_str().unwrap()]);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(1), "{error}");
    assert!(error.contains("tier `workdir`"), "{error}");
    assert!(error.contains("golem-implementer.agent.md"), "{error}");
    assert!(!error.contains("materialization failed"), "{error}");
    assert!(!fixture.blocked_home.join(".gal").exists());
}

#[test]
fn pipeline_corrupt_ancestor_source_wins_over_exe_side() {
    let fixture = Fixture::new();
    let ancestor = fixture.directory("ancestor-repo");
    source_root(&ancestor.join("plugins/gal-core"), None);
    source_root(fixture.binary.parent().unwrap(), Some("exe-side contract"));
    let repo = ancestor.join("nested");
    fs::create_dir_all(&repo).unwrap();
    let prompt = fixture.prompt(&repo);
    let output = fixture.run(&repo, &["pipeline", prompt.to_str().unwrap()]);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(1), "{error}");
    assert!(error.contains("tier `ancestor`"), "{error}");
    assert!(!error.contains("materialization failed"), "{error}");
    assert!(!fixture.blocked_home.join(".gal").exists());
}

#[test]
fn pipeline_exe_side_source_precedes_embedded_materialization() {
    let fixture = Fixture::new();
    let repo = fixture.directory("exe-side-repo");
    source_root(fixture.binary.parent().unwrap(), None);
    let prompt = fixture.prompt(&repo);
    let output = fixture.run(&repo, &["pipeline", prompt.to_str().unwrap()]);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(1), "{error}");
    assert!(error.contains("tier `exe-side`"), "{error}");
    assert!(!error.contains("materialization failed"), "{error}");
    assert!(!fixture.blocked_home.join(".gal").exists());
}
