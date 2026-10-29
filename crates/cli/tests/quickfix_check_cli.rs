//! `cargo test -p gal-cli --test quickfix_check_cli`
//!
//! Real-binary coverage for `gal quickfix-check`: scope limits, Protected Path and
//! dependency-file refusal, check-command execution against the committed
//! `.dev/project.md`, the empty-change `not-run` result, whitespace-insensitive
//! line counting, and the `quickfix` configuration key.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const PASSING_COMMAND: &str = "git --version";

struct Fixture {
    repo: TempDir,
    home: TempDir,
}

impl Fixture {
    fn receipt_path(&self) -> PathBuf {
        self.home.path().join("quickfix.receipt.md")
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.repo.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn write_config(&self, json: &str) {
        let dir = self.home.path().join(".gal").join("config");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.json"), json).unwrap();
    }

    fn run(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .args(["quickfix-check", "--receipt"])
            .arg(self.receipt_path())
            .output()
            .unwrap()
    }

    fn receipt(&self) -> String {
        fs::read_to_string(self.receipt_path()).unwrap()
    }
}

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn project_md(command: &str) -> String {
    format!(
        "## Tech Stack\n<!-- gal:authoritative-check -->\n```json\n{{\n  \"command\": [\"{command}\"]\n}}\n```\n\n## Protected Paths\n\n- `plugins/gal-core/conventions/` — workflow semantics\n"
    )
}

fn numbered_lines(count: usize) -> String {
    (1..=count).map(|n| format!("line {n}\n")).collect()
}

fn fixture_with_command(command: &str) -> Fixture {
    let fixture = Fixture {
        repo: tempfile::tempdir().unwrap(),
        home: tempfile::tempdir().unwrap(),
    };
    let repo = fixture.repo.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.email", "t@t"]);
    git(repo, &["config", "user.name", "t"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
    fixture.write(".dev/project.md", &project_md(command));
    fixture.write("wide.txt", &numbered_lines(80));
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        fixture.write(name, &numbered_lines(30));
    }
    fixture.write("plugins/gal-core/conventions/rule.md", "one\ntwo\n");
    fixture.write("Cargo.lock", "# lock\n");
    fixture.write("web/package-lock.json", "{}\n");
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "fixture"]);
    fixture
}

fn fixture() -> Fixture {
    fixture_with_command(PASSING_COMMAND)
}

fn append_lines(fixture: &Fixture, name: &str, count: usize) {
    let path = fixture.repo.path().join(name);
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str(&numbered_lines(count));
    fs::write(path, text).unwrap();
}

#[test]
fn small_diff_with_passing_check_passes() {
    let fixture = fixture();
    append_lines(&fixture, "a.txt", 10);
    append_lines(&fixture, "b.txt", 10);

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(output.status.success(), "{receipt}");
    assert!(receipt.contains("overall: pass"), "{receipt}");
    assert!(receipt.contains("files: 2"), "{receipt}");
    assert!(receipt.contains("lines: 20"), "{receipt}");
    assert!(receipt.contains(PASSING_COMMAND), "{receipt}");
}

#[test]
fn file_count_over_limit_fails_with_scope_reason() {
    let fixture = fixture();
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        append_lines(&fixture, name, 1);
    }

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(!output.status.success(), "{receipt}");
    assert!(receipt.contains("overall: fail"), "{receipt}");
    assert!(
        receipt.contains("scope exceeded: 4 file(s) changed, limit 3"),
        "{receipt}"
    );
}

#[test]
fn deleted_and_untracked_files_count_toward_the_file_limit() {
    let fixture = fixture();
    fs::remove_file(fixture.repo.path().join("a.txt")).unwrap();
    append_lines(&fixture, "b.txt", 1);
    fixture.write("new-one.txt", "x\n");
    fixture.write("new-two.txt", "y\n");

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(!output.status.success(), "{receipt}");
    assert!(receipt.contains("files: 4"), "{receipt}");
    assert!(receipt.contains("scope exceeded"), "{receipt}");
}

#[test]
fn configured_limit_overrides_the_default() {
    let fixture = fixture();
    fixture.write_config(r#"{"quickfix":{"maxFiles":4}}"#);
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        append_lines(&fixture, name, 1);
    }

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(output.status.success(), "{receipt}");
    assert!(receipt.contains("overall: pass"), "{receipt}");
    assert!(receipt.contains("maxFiles=4"), "{receipt}");
}

#[test]
fn line_count_over_limit_fails() {
    let fixture = fixture();
    append_lines(&fixture, "a.txt", 51);

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(!output.status.success(), "{receipt}");
    assert!(
        receipt.contains("scope exceeded: 51 line(s) changed, limit 50"),
        "{receipt}"
    );
}

#[test]
fn one_line_under_a_protected_path_fails() {
    let fixture = fixture();
    append_lines(&fixture, "plugins/gal-core/conventions/rule.md", 1);

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(!output.status.success(), "{receipt}");
    assert!(receipt.contains("overall: fail"), "{receipt}");
    assert!(
        receipt.contains("Protected Path(s) touched: plugins/gal-core/conventions/rule.md"),
        "{receipt}"
    );
}

#[test]
fn lock_file_change_fails_as_dependency_change() {
    let fixture = fixture();
    fixture.write("Cargo.lock", "# lock\n# changed\n");

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(!output.status.success(), "{receipt}");
    assert!(
        receipt.contains("dependency file(s) touched: Cargo.lock"),
        "{receipt}"
    );
}

#[test]
fn nested_package_lock_fails_as_dependency_change() {
    let fixture = fixture();
    fixture.write("web/package-lock.json", "{\"changed\": true}\n");

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(!output.status.success(), "{receipt}");
    assert!(
        receipt.contains("dependency file(s) touched: web/package-lock.json"),
        "{receipt}"
    );
}

#[test]
fn failing_check_command_fails_and_is_named() {
    let failing = "git rev-parse --verify refs/heads/no-such-branch";
    let fixture = fixture_with_command(failing);
    append_lines(&fixture, "a.txt", 1);

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(!output.status.success(), "{receipt}");
    assert!(receipt.contains("overall: fail"), "{receipt}");
    assert!(
        receipt.contains(&format!("command failed: {failing}")),
        "{receipt}"
    );
}

#[test]
fn empty_diff_is_not_run_never_pass() {
    let fixture = fixture();

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(!output.status.success(), "{receipt}");
    assert!(receipt.contains("overall: not-run"), "{receipt}");
    assert!(!receipt.contains("overall: pass"), "{receipt}");
}

#[test]
fn working_tree_edit_of_the_command_block_does_not_change_the_commands() {
    let fixture = fixture();
    fixture.write(".dev/project.md", &project_md("git bogus-subcommand-zzz"));

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(output.status.success(), "{receipt}");
    assert!(receipt.contains("overall: pass"), "{receipt}");
    assert!(receipt.contains(PASSING_COMMAND), "{receipt}");
    assert!(!receipt.contains("bogus-subcommand-zzz"), "{receipt}");
}

#[test]
fn whitespace_only_edits_do_not_count_as_lines() {
    let fixture = fixture();
    let indented: String = (1..=80).map(|n| format!("    line {n}\n")).collect();
    fixture.write("wide.txt", &indented);
    append_lines(&fixture, "a.txt", 10);

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(output.status.success(), "{receipt}");
    assert!(receipt.contains("overall: pass"), "{receipt}");
    assert!(receipt.contains("files: 2"), "{receipt}");
    assert!(receipt.contains("lines: 10"), "{receipt}");
}

#[test]
fn untracked_file_lines_are_counted() {
    let fixture = fixture();
    fixture.write("fresh.txt", &numbered_lines(60));

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(!output.status.success(), "{receipt}");
    assert!(
        receipt.contains("scope exceeded: 60 line(s) changed, limit 50"),
        "{receipt}"
    );
}

#[test]
fn binary_file_counts_as_one_file_and_zero_lines() {
    let fixture = fixture();
    fs::write(fixture.repo.path().join("blob.bin"), [0u8, 1, 2, 3, 0, 255]).unwrap();
    git(fixture.repo.path(), &["add", "blob.bin"]);
    git(fixture.repo.path(), &["commit", "-q", "-m", "binary"]);
    fs::write(
        fixture.repo.path().join("blob.bin"),
        [0u8, 9, 9, 0, 0, 7, 7],
    )
    .unwrap();

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(output.status.success(), "{receipt}");
    assert!(receipt.contains("files: 1"), "{receipt}");
    assert!(receipt.contains("lines: 0"), "{receipt}");
}

#[test]
fn malformed_quickfix_key_is_a_parse_error_failure() {
    let fixture = fixture();
    fixture.write_config(r#"{"quickfix":{"maxFiles":"three"}}"#);
    append_lines(&fixture, "a.txt", 1);

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(!output.status.success(), "{receipt}");
    assert!(receipt.contains("overall: fail"), "{receipt}");
    assert!(receipt.contains("quickfix-config"), "{receipt}");
    assert!(
        receipt.contains("quickfix.maxFiles must be a non-negative integer"),
        "{receipt}"
    );
}

#[test]
fn committed_project_without_protected_paths_heading_is_not_run() {
    let fixture = fixture();
    let without_heading = format!(
        "## Tech Stack\n<!-- gal:authoritative-check -->\n```json\n{{\n  \"command\": [\"{PASSING_COMMAND}\"]\n}}\n```\n"
    );
    fixture.write(".dev/project.md", &without_heading);
    git(fixture.repo.path(), &["add", "-A"]);
    git(fixture.repo.path(), &["commit", "-q", "-m", "drop heading"]);
    append_lines(&fixture, "a.txt", 1);

    let output = fixture.run();
    let receipt = fixture.receipt();

    assert!(!output.status.success(), "{receipt}");
    assert!(receipt.contains("overall: not-run"), "{receipt}");
    assert!(!receipt.contains("overall: pass"), "{receipt}");
    assert!(
        receipt.contains("| protected-paths | not-run |"),
        "{receipt}"
    );
}
