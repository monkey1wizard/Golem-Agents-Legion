use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{tempdir, TempDir};

fn test_gal_exe() -> &'static std::path::Path {
    static TEST_EXE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    TEST_EXE
        .get_or_init(|| {
            let src = std::path::PathBuf::from(env!("CARGO_BIN_EXE_gal"));
            let needs_copy = src.components().any(|component| {
                ["worktrees", ".worktrees"].iter().any(|name| {
                    component
                        .as_os_str()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(name)
                })
            });
            if needs_copy {
                let dir = std::env::temp_dir().join(format!("gal-test-bin-{}", std::process::id()));
                let _ = std::fs::create_dir_all(&dir);
                let dst = dir.join(src.file_name().unwrap());
                let _ = std::fs::copy(&src, &dst);
                dst
            } else {
                src
            }
        })
        .as_path()
}

fn tid(n: u32) -> String {
    format!("T-{n:02}")
}

fn tpid(n: u32) -> String {
    format!("TP-{n:02}")
}

fn git(repo: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn repo() -> TempDir {
    let temp = tempdir().expect("fixture tempdir");
    git(temp.path(), &["init", "-q", "-b", "main"]);
    git(
        temp.path(),
        &["config", "user.email", "fixture@example.com"],
    );
    git(
        temp.path(),
        &["config", "user.name", "Task Grammar Fixture"],
    );
    git(temp.path(), &["config", "commit.gpgsign", "false"]);
    fs::create_dir_all(temp.path().join("docs")).unwrap();
    fs::write(temp.path().join("docs/example.md"), "fixture\n").unwrap();
    fs::write(
        temp.path().join("docs/glossary.md"),
        "# RETIRED-TERMS:BEGIN\n# RETIRED-TERMS:END\n",
    )
    .unwrap();
    temp
}

fn plan(task_line: &str, status: &str) -> String {
    let task = tid(1);
    let test = tpid(1);
    format!(
        "# Plan Prompt: Grammar Fixture\n\n## Status\n\nWorkflow: {status}\nCurrent Task: —\n\n## Tasks\n\n{task_line}\n  - **Verification Impact**: {{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false}}\n\n## Verification Map\n\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n| {task} | unit | cargo test | pass | TESTER | test output |\n\n## Test Plan\n\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n| {test} | integration | grammar | {task} |\n"
    )
}

fn write_plan(repo: &Path, name: &str, text: &str) -> PathBuf {
    let path = repo.join(".dev/plans").join(format!("{name}.prompt.md"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

fn run_gal(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new(test_gal_exe())
        .current_dir(repo)
        .args(args)
        .output()
        .expect("gal binary must start")
}

fn run_receipt(repo: &Path, command: &str, prompt: &Path, name: &str) -> String {
    let receipt = repo.join(format!("{name}.receipt.md"));
    let output = run_gal(
        repo,
        &[
            command,
            prompt.to_str().unwrap(),
            "--receipt",
            receipt.to_str().unwrap(),
        ],
    );
    assert!(
        output.status.code().is_some(),
        "{command} did not return an exit status"
    );
    fs::read_to_string(receipt).unwrap_or_default()
}

fn run_terminal_reverify(repo: &Path, prompt: &Path) -> String {
    let slug = prompt
        .file_name()
        .unwrap()
        .to_string_lossy()
        .strip_suffix(".prompt.md")
        .unwrap()
        .to_string();
    let receipt = repo
        .join(".dev/pipeline")
        .join(slug)
        .join("terminal-reverify.receipt.md");
    fs::create_dir_all(receipt.parent().unwrap()).unwrap();
    let output = Command::new(test_gal_exe())
        .current_dir(repo)
        .args(["pipeline-preflight", "--terminal-reverify"])
        .arg(prompt)
        .output()
        .expect("terminal reverify must start");
    assert!(output.status.code().is_some());
    fs::read_to_string(receipt).expect("terminal reverify receipt must be written")
}

fn check_state<'a>(receipt: &'a str, check: &str) -> &'a str {
    check_row(receipt, check).split('|').nth(2).unwrap().trim()
}

fn check_row<'a>(receipt: &'a str, check: &str) -> &'a str {
    let row = receipt
        .lines()
        .find(|line| line.starts_with(&format!("| {check} |")))
        .unwrap_or_else(|| panic!("missing {check} row in receipt:\n{receipt}"));
    row
}

fn task_forms() -> Vec<(String, bool)> {
    let id = tid(1);
    let digits = id.strip_prefix("T-").unwrap();
    vec![
        (format!("- [x] {id} work"), true),
        (format!("- [x] {id}: work"), true),
        (format!("- [x] {id} — work"), true),
        (format!("- [x] **{id}** work"), true),
        (format!("- [x] T-{} work", &digits[..1]), false),
        (format!("- [x] T-{digits}0 work"), false),
        (format!("- [x] {id}x work"), false),
        (format!("- [X] {id} work"), false),
        (format!("- [x] **{id} work"), false),
    ]
}

#[test]
fn task_line_forms_agree_across_commit_preflight_refining_and_prompt_checks() {
    let fixture = repo();
    let root = fixture.path();
    let hash = {
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "fixture"]);
        git(root, &["rev-parse", "HEAD"])
    };

    for (index, (line, accepted)) in task_forms().into_iter().enumerate() {
        let id = tid(1);
        let task_line = if accepted {
            format!("{line} *({hash})*")
        } else {
            line.clone()
        };
        let prompt_text = plan(&task_line, "DONE");
        let prompt = write_plan(root, &format!("grammar-{index}"), &prompt_text);

        let converge_receipt = root.join("converge.receipt.md");
        let converge_output = run_gal(
            root,
            &[
                "pipeline-converge-check",
                prompt.to_str().unwrap(),
                "--task",
                &id,
                "--receipt",
                converge_receipt.to_str().unwrap(),
            ],
        );
        assert!(converge_output.status.code().is_some());
        let task_commit = fs::read_to_string(converge_receipt).unwrap_or_default();
        assert_eq!(
            check_state(&task_commit, "task-commit-existence"),
            if accepted { "pass" } else { "not-run" },
            "task commit result for {line:?}:\n{task_commit}"
        );

        let state_path = root.join(".dev/state.md");
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        fs::write(
            &state_path,
            format!(
                "## Session Continuity\n\n| grammar-{index} | {} | in progress |\n",
                format!(".dev/plans/grammar-{index}.prompt.md")
            ),
        )
        .unwrap();
        let preflight = run_receipt(root, "pipeline-preflight", &prompt, "preflight");
        assert_eq!(
            check_state(&preflight, "tasks-well-formed"),
            if accepted { "pass" } else { "fail" },
            "preflight result for {line:?}:\n{preflight}"
        );
        if !accepted && !line.starts_with("- [X]") {
            assert!(
                check_row(&preflight, "tasks-well-formed").contains(&line),
                "preflight must name the malformed bullet {line:?}:\n{preflight}"
            );
        }
        let terminal = run_terminal_reverify(root, &prompt);
        assert_eq!(
            check_state(&terminal, "terminal-tasks-checked"),
            if accepted { "pass" } else { "fail" },
            "terminal result for {line:?}:\n{terminal}"
        );
        if !accepted && !line.starts_with("- [X]") {
            assert!(
                check_row(&terminal, "terminal-tasks-checked").contains(&line),
                "terminal check must name the malformed bullet {line:?}:\n{terminal}"
            );
        }

        let refining = run_receipt(root, "refining-check", &prompt, "refining");
        assert_eq!(
            check_state(&refining, "id-well-formed"),
            if accepted { "pass" } else { "fail" },
            "refining result for {line:?}:\n{refining}"
        );
        if !accepted {
            assert!(
                check_row(&refining, "id-well-formed").contains(&line),
                "refining must name the malformed bullet {line:?}:\n{refining}"
            );
        }

        let unchecked_prompt_text = prompt_text.replace("- [x]", "- [ ]");
        let unchecked_terminal_prompt = write_plan(
            root,
            &format!("unchecked-terminal-{index}"),
            &unchecked_prompt_text,
        );
        let unchecked_terminal = run_terminal_reverify(root, &unchecked_terminal_prompt);
        assert_eq!(
            check_state(&unchecked_terminal, "terminal-tasks-checked"),
            "fail",
            "terminal check must count unchecked task form {line:?} as unchecked:\n{unchecked_terminal}"
        );

        let prompt_check_prompt = write_plan(
            root,
            &format!("prompt-check-{index}"),
            &unchecked_prompt_text,
        );
        let prompt_check_receipt = root.join("prompt-check.receipt.md");
        let prompt_check_output = run_gal(
            root,
            &[
                "prompt-check",
                prompt_check_prompt.to_str().unwrap(),
                "--receipt",
                prompt_check_receipt.to_str().unwrap(),
            ],
        );
        assert!(prompt_check_output.status.code().is_some());
        let prompt_check = fs::read_to_string(prompt_check_receipt).unwrap_or_default();
        assert_eq!(
            check_state(&prompt_check, "unchecked-task-anchor"),
            if accepted { "pass" } else { "fail" },
            "prompt-check result for {line:?}:\n{prompt_check}"
        );
    }
}

#[test]
fn malformed_task_forms_fail_all_task_validators_beside_a_valid_task() {
    let fixture = repo();
    let root = fixture.path();

    for (index, (malformed, _)) in task_forms()
        .into_iter()
        .filter(|(_, accepted)| !accepted)
        .enumerate()
    {
        let valid_id = tid(1);
        let valid_line = format!("- [x] {valid_id} valid task");
        let prompt_text = plan(&format!("{valid_line}\n{malformed}"), "DONE");
        let prompt = write_plan(root, &format!("mixed-malformed-{index}"), &prompt_text);

        let state_path = root.join(".dev/state.md");
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        fs::write(
            &state_path,
            format!(
                "## Session Continuity\n\n| mixed-malformed-{index} | .dev/plans/mixed-malformed-{index}.prompt.md | in progress |\n"
            ),
        )
        .unwrap();

        let preflight = run_receipt(root, "pipeline-preflight", &prompt, "mixed-preflight");
        assert_eq!(
            check_state(&preflight, "tasks-well-formed"),
            "fail",
            "preflight must reject {malformed:?} beside a valid task:\n{preflight}"
        );
        assert!(
            check_row(&preflight, "tasks-well-formed").contains(&malformed),
            "preflight must name malformed bullet {malformed:?}:\n{preflight}"
        );

        let terminal = run_terminal_reverify(root, &prompt);
        assert_eq!(
            check_state(&terminal, "terminal-tasks-checked"),
            "fail",
            "terminal check must reject {malformed:?} beside a valid task:\n{terminal}"
        );
        assert!(
            check_row(&terminal, "terminal-tasks-checked").contains(&malformed),
            "terminal check must name malformed bullet {malformed:?}:\n{terminal}"
        );

        let refining = run_receipt(root, "refining-check", &prompt, "mixed-refining");
        assert_eq!(
            check_state(&refining, "id-well-formed"),
            "fail",
            "refining must reject {malformed:?} beside a valid task:\n{refining}"
        );
        assert!(
            check_row(&refining, "id-well-formed").contains(&malformed),
            "refining must name malformed bullet {malformed:?}:\n{refining}"
        );
    }
}

#[test]
fn malformed_task_line_does_not_extend_the_previous_task_block() {
    let fixture = repo();
    let root = fixture.path();
    let first = tid(1);
    let second = tid(2);
    let malformed = format!("T-{}", 100);
    let test = tpid(1);
    let tasks = format!(
        "- [ ] {first} update `docs/example.md`\n- [ ] {malformed} malformed `docs/i18n/fr/example.fr.md`\n- [ ] {second} update `docs/other.md`\n"
    );
    let text = format!(
        "# Plan\n\n## Test Plan\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n| {test} | integration | grammar | {first}, {second} |\n\n## Review Results\n### Engineering Review\nCLEAR\n\n<!-- ENG_REVIEW: CLEAR -->\n\n## Tasks\n{tasks}"
    );
    let prompt = write_plan(root, "block-boundary", &text);
    let refining = run_receipt(root, "refining-check", &prompt, "refining-blocks");
    assert_eq!(
        check_state(&refining, "translation-split"),
        "pass",
        "malformed task content must not be attached to the previous block:\n{refining}"
    );
}
