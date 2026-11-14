use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn dispatch_script(args: &[&str], home: &Path, cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(cwd)
        .args(["dispatch-script"])
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .output()
        .unwrap()
}

fn setup() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let cwd = temp.path().join("repo");
    fs::create_dir_all(home.join(".gal/config")).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    fs::write(
        home.join(".gal/config/config.json"),
        br#"{"executorRouting":{"executors":{"claude":{"model":"default-model"},"codex":{"model":"codex-model","effort":"high"}}}}"#,
    )
    .unwrap();
    (temp, home, cwd)
}

#[test]
fn consult_script_rejects_one_shot_route_flags_with_isolated_call_recovery() {
    let (_temp, home, cwd) = setup();

    let consult = dispatch_script(
        &["consult-script", "--executor", "claude", "golem-architect"],
        &home,
        &cwd,
    );
    let consult = String::from_utf8_lossy(&consult.stdout);
    assert!(consult.contains("COMMAND: error"), "{consult}");
    assert!(
        consult.contains("cannot change the model of an in-context consult"),
        "{consult}"
    );
    assert!(consult.contains("isolated role call"), "{consult}");
}

#[test]
fn dispatch_script_emits_one_shot_role_actions() {
    let (_temp, home, cwd) = setup();

    let routed = dispatch_script(
        &["analyst", "--executor", "claude", "--model", "review-model"],
        &home,
        &cwd,
    );
    let routed = String::from_utf8_lossy(&routed.stdout);
    assert!(routed.contains("COMMAND: routed"), "{routed}");
    assert!(
        routed.contains("ONE_SHOT_ROUTE: executor=claude model=review-model effort=(default)"),
        "{routed}"
    );

    let host = dispatch_script(
        &[
            "designer",
            "--host-subagent",
            "codex",
            "--model",
            "codex-model",
        ],
        &home,
        &cwd,
    );
    let host = String::from_utf8_lossy(&host.stdout);
    assert!(host.contains("COMMAND: host-native"), "{host}");
    assert!(host.contains("wait for final completion"), "{host}");
    assert!(host.contains("do not start a replacement"), "{host}");

    let formal = dispatch_script(
        &[
            "architect",
            "--review-role",
            "architect",
            "--executor",
            "claude",
        ],
        &home,
        &cwd,
    );
    let formal = String::from_utf8_lossy(&formal.stdout);
    assert!(formal.contains("COMMAND: formal-review"), "{formal}");
    assert!(formal.contains("REVIEW_ROLE: architect"), "{formal}");
    assert!(formal.contains("source snapshot"), "{formal}");

    let advisory = dispatch_script(&["architect", "--executor", "claude"], &home, &cwd);
    let advisory = String::from_utf8_lossy(&advisory.stdout);
    assert!(advisory.contains("COMMAND: advisory-direct"), "{advisory}");
    assert!(
        advisory.contains("cannot write the plan or set CLEAR"),
        "{advisory}"
    );
}

#[test]
fn dispatch_script_rejects_unsupported_or_unused_routes_and_keeps_no_override() {
    let (_temp, home, cwd) = setup();

    let unsupported = dispatch_script(
        &[
            "architect",
            "--host-subagent",
            "opencode",
            "--model",
            "model",
        ],
        &home,
        &cwd,
    );
    let unsupported = String::from_utf8_lossy(&unsupported.stdout);
    assert!(unsupported.contains("COMMAND: error"), "{unsupported}");
    assert!(
        unsupported.contains("host does not support model selection"),
        "{unsupported}"
    );

    let unused = dispatch_script(
        &[
            "architect",
            "--review-role",
            "analyst",
            "--executor",
            "claude",
        ],
        &home,
        &cwd,
    );
    let unused = String::from_utf8_lossy(&unused.stdout);
    assert!(unused.contains("COMMAND: error"), "{unused}");
    assert!(unused.contains("unused conditional route"), "{unused}");

    let unchanged = dispatch_script(&["designer", "--discuss"], &home, &cwd);
    let unchanged = String::from_utf8_lossy(&unchanged.stdout);
    assert!(unchanged.contains("MODE: direct"), "{unchanged}");
    assert!(!unchanged.contains("ONE_SHOT_ROUTE:"), "{unchanged}");
    assert!(unchanged.contains("--discuss"), "{unchanged}");
}
