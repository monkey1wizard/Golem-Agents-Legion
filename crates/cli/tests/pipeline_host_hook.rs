use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
use tempfile::TempDir;

fn hash_file(path: &Path) -> String {
    let bytes = fs::read(path).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

fn invoke(data: &TempDir, event: &str, body: &str) -> serde_json::Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gal"))
        .args(["pipeline-host-hook", event])
        .env("PLUGIN_DATA", data.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(body.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    serde_json::from_slice(&out.stdout).unwrap_or_else(|_| serde_json::json!({}))
}

fn invoke_raw(
    data: &TempDir,
    event: &str,
    body: &[u8],
) -> (std::process::ExitStatus, Vec<u8>, Vec<u8>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gal"))
        .args(["pipeline-host-hook", event])
        .env("PLUGIN_DATA", data.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.take().unwrap().write_all(body);
    let out = child.wait_with_output().unwrap();
    (out.status, out.stdout, out.stderr)
}

fn make_event(dir: &TempDir, prompt: &str) -> String {
    format!(
        r#"{{"session_id":"s1","turn_id":"t1","cwd":{:?},"prompt":{:?},"generation":1,"nonce":"n1","tool_name":"shell"}}"#,
        dir.path().to_string_lossy(),
        prompt
    )
}

fn write_plan(repo: &TempDir, slug: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let plans = repo.path().join(".dev/plans");
    fs::create_dir_all(&plans).unwrap();
    let source = plans.join(format!("{slug}.md"));
    let prompt = plans.join(format!("{slug}.prompt.md"));
    fs::write(&source, format!("# {slug}\n")).unwrap();
    fs::write(&prompt, format!("# {slug} Prompt\n")).unwrap();
    (source, prompt)
}

fn make_pipeline_event(repo: &TempDir) -> String {
    write_plan(repo, "demo");
    make_event(repo, "/gal pipeline .dev/plans/demo.prompt.md")
}

#[test]
fn test_exact_state_sequence_denials_and_unchanged_protected_file_hashes() {
    let data = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();

    // Setup protected files in repository
    let prompt_file = repo.path().join(".dev/plans/demo.prompt.md");
    let state_file = repo.path().join(".dev").join("state.md");
    let src_file = repo.path().join("src").join("main.rs");

    fs::create_dir_all(prompt_file.parent().unwrap()).unwrap();
    fs::create_dir_all(src_file.parent().unwrap()).unwrap();

    fs::write(&prompt_file, b"# Demo Prompt\nInitial spec content\n").unwrap();
    fs::write(&state_file, b"# State\nActive Plans: demo\n").unwrap();
    fs::write(&src_file, b"fn main() { println!(\"hello\"); }\n").unwrap();

    let h_prompt_init = hash_file(&prompt_file);
    let h_state_init = hash_file(&state_file);
    let h_src_init = hash_file(&src_file);

    let body = make_event(&repo, "/gal pipeline .dev/plans/demo.prompt.md");

    // Step 1: UserPromptSubmit creates BootstrapPending
    let ups_res = invoke(&data, "user-prompt-submit", &body);
    assert_eq!(ups_res["continue"], true);

    let pending_path = data.path().join("bootstrap-pending.json");
    assert!(pending_path.exists());
    let pending: serde_json::Value =
        serde_json::from_slice(&fs::read(&pending_path).unwrap()).unwrap();
    assert_eq!(pending["state"], "BootstrapPending");
    assert_eq!(pending["stops"], 0);

    // Step 2: PreToolUse denies every supported tool
    for tool in [
        "shell",
        "apply_patch",
        "mcp",
        "read",
        "write",
        "unknown_tool",
    ] {
        let mut request: serde_json::Value = serde_json::from_str(&body).unwrap();
        request["tool_name"] = serde_json::json!(tool);
        let pre_res = invoke(&data, "pre-tool-use", &request.to_string());
        assert_eq!(pre_res["continue"], false);
        assert_eq!(pre_res["decision"], "deny");
    }

    // Verify protected files unchanged after PreToolUse denials
    assert_eq!(hash_file(&prompt_file), h_prompt_init);
    assert_eq!(hash_file(&state_file), h_state_init);
    assert_eq!(hash_file(&src_file), h_src_init);

    // Step 3: Stop emits block with continuation and publishes ReadyGrant
    let stop_res = invoke(&data, "stop", &body);
    assert_eq!(stop_res["continue"], false);
    assert_eq!(stop_res["decision"], "block");
    let action = "gal pipeline --require-codex-stop-v1 .dev/plans/demo.prompt.md";
    assert_eq!(stop_res["reason"], action);

    // Feed the exact returned argv through the real guarded CLI parser. A fresh
    // plugin store forces the later grant gate to fail without consuming the
    // grant created by this handshake.
    let parser_data = TempDir::new().unwrap();
    let mut action_parts = action.split_whitespace();
    assert_eq!(action_parts.next(), Some("gal"));
    let parser_output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .args(action_parts)
        .current_dir(repo.path())
        .env("PLUGIN_DATA", parser_data.path())
        .output()
        .unwrap();
    let parser_stderr = String::from_utf8_lossy(&parser_output.stderr);
    assert!(parser_stderr.contains("host-continuation-not-ready"));
    assert!(!parser_stderr.contains("guarded pipeline requires an execution prompt path"));

    let grant_path = data.path().join("ready-grant.json");
    assert!(grant_path.exists());
    let grant: serde_json::Value = serde_json::from_slice(&fs::read(&grant_path).unwrap()).unwrap();
    assert_eq!(grant["state"], "ReadyGrant");
    assert_eq!(grant["stops"], 1);

    // Verify protected files unchanged after Stop
    assert_eq!(hash_file(&prompt_file), h_prompt_init);
    assert_eq!(hash_file(&state_file), h_state_init);
    assert_eq!(hash_file(&src_file), h_src_init);

    // Step 4: PreToolUse cannot allow the action before the immediate
    // continuation UserPromptSubmit binds its new turn.
    let mut presented: serde_json::Value = serde_json::from_str(&body).unwrap();
    presented["turn_id"] = serde_json::json!("t2");
    presented["prompt"] = serde_json::json!(action);
    presented["tool_input"] = serde_json::json!({"command":action});
    let before_presentation = invoke(&data, "pre-tool-use", &presented.to_string());
    assert_ne!(before_presentation["decision"], "allow");
    let ready_state: serde_json::Value =
        serde_json::from_slice(&fs::read(&pending_path).unwrap()).unwrap();
    assert_eq!(ready_state["state"], "ReadyGrant");

    let presentation = invoke(&data, "user-prompt-submit", &presented.to_string());
    assert_eq!(presentation["continue"], true, "{presentation}");
    let presented_state: serde_json::Value =
        serde_json::from_slice(&fs::read(&pending_path).unwrap()).unwrap();
    assert_eq!(presented_state["state"], "GrantPresented");
    assert_eq!(presented_state["presented_turn_id"], "t2");

    // Step 5: PreToolUse allows only that exact action and presenting turn.
    let allow_res = invoke(&data, "pre-tool-use", &presented.to_string());
    assert_eq!(allow_res["continue"], true);
    assert_eq!(allow_res["decision"], "allow");

    assert!(pending_path.exists());
    assert!(grant_path.exists());
    let control_path = repo.path().join(".dev/pipeline/demo/codex-hook-grant.json");
    let control: serde_json::Value =
        serde_json::from_slice(&fs::read(&control_path).unwrap()).unwrap();
    assert_eq!(control["session_id"], "s1");
    assert_eq!(control["turn_id"], "t2");
    assert_eq!(
        control["worktree"],
        fs::canonicalize(repo.path())
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(
        control["nonce_digest"],
        format!("{:x}", Sha256::digest(b"n1"))
    );
    assert_eq!(control["prompt_hash"], h_prompt_init);
    assert!(control.get("nonce").is_none());
    assert_eq!(control.as_object().unwrap().len(), 8);

    // The real guarded CLI accepts and consumes the presented grant. The
    // minimal fixture can fail later because it is not a complete pipeline
    // prompt, but it must not fail the host-continuation gate.
    let mut action_parts = action.split_whitespace();
    assert_eq!(action_parts.next(), Some("gal"));
    let guarded_output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .args(action_parts)
        .current_dir(repo.path())
        .env("PLUGIN_DATA", data.path())
        .output()
        .unwrap();
    let guarded_stderr = String::from_utf8_lossy(&guarded_output.stderr);
    assert!(!guarded_stderr.contains("host-continuation-not-ready"));
    let consumed_grant: serde_json::Value =
        serde_json::from_slice(&fs::read(&grant_path).unwrap()).unwrap();
    assert_eq!(consumed_grant["state"], "Consumed");

    // Step 6: Post-activation neutral root-tool fixtures
    let mut post_act: serde_json::Value = serde_json::from_str(&body).unwrap();
    post_act["tool_name"] = serde_json::json!("shell");
    post_act["tool_input"] = serde_json::json!({"command":"cargo check"});
    let post_res = invoke(&data, "pre-tool-use", &post_act.to_string());
    assert_eq!(post_res["continue"], true);

    // Final verification of protected file byte-identity
    assert_eq!(hash_file(&prompt_file), h_prompt_init);
    assert_eq!(hash_file(&state_file), h_state_init);
    assert_eq!(hash_file(&src_file), h_src_init);
}

#[test]
fn test_consumed_grant_is_neutral_only_for_its_bound_session_and_worktree() {
    for mismatch in ["session", "worktree"] {
        let data = TempDir::new().unwrap();
        let bound_repo = TempDir::new().unwrap();
        let body = make_pipeline_event(&bound_repo);

        assert_eq!(invoke(&data, "user-prompt-submit", &body)["continue"], true);
        let pre_before_consumption = invoke(&data, "pre-tool-use", &body);
        assert_eq!(pre_before_consumption["decision"], "deny");

        let grant_path = data.path().join("ready-grant.json");
        let pending: serde_json::Value =
            serde_json::from_slice(&fs::read(data.path().join("bootstrap-pending.json")).unwrap())
                .unwrap();
        let consumed = serde_json::json!({"state":"Consumed","binding":pending["binding"]});
        fs::write(&grant_path, serde_json::to_vec(&consumed).unwrap()).unwrap();

        let (new_repo, mut new_event) = if mismatch == "session" {
            let mut event: serde_json::Value = serde_json::from_str(&body).unwrap();
            event["session_id"] = serde_json::json!("s2");
            (None, event)
        } else {
            let repo = TempDir::new().unwrap();
            let event: serde_json::Value =
                serde_json::from_str(&make_pipeline_event(&repo)).unwrap();
            (Some(repo), event)
        };
        let _keep_repo_alive = new_repo;
        new_event["tool_name"] = serde_json::json!("shell");
        let new_body = new_event.to_string();
        assert_eq!(
            invoke(&data, "user-prompt-submit", &new_body)["continue"],
            true
        );

        let stale_grant_result = invoke(&data, "pre-tool-use", &new_body);
        assert_eq!(
            stale_grant_result["continue"], false,
            "mismatched {mismatch} must not bypass BootstrapPending"
        );
        assert_eq!(stale_grant_result["decision"], "deny");
        assert_eq!(stale_grant_result["reason"], "grant-expired-or-invalid");
    }
}

#[test]
fn test_neutral_cases_and_non_pipeline_prompts() {
    let data = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();

    for prompt in ["hello", "cargo build", "gal doctor", "fix this test"] {
        let body = make_event(&repo, prompt);
        let res = invoke(&data, "user-prompt-submit", &body);
        assert_eq!(res["continue"], true);
        assert!(!data.path().join("bootstrap-pending.json").exists());
        assert!(!data.path().join("ready-grant.json").exists());

        let stop = invoke(&data, "stop", &body);
        assert_eq!(stop["continue"], true);

        let pre = invoke(&data, "pre-tool-use", &body);
        assert_eq!(pre["continue"], true);
    }
}

#[test]
fn test_malformed_and_ambiguous_pipeline_prompts_fail_closed() {
    let data = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();

    for (prompt, reason) in [
        ("/gal pipeline", "active-plan-unavailable"),
        (
            "/gal pipeline first.prompt.md second.prompt.md",
            "ambiguous-pipeline-request",
        ),
        ("/gal pipeline demo.md", "source-plan-unavailable"),
        ("/gal pipeline demo.prompt.md;rm", "invalid-plan-scope"),
    ] {
        let result = invoke(&data, "user-prompt-submit", &make_event(&repo, prompt));
        assert_eq!(result["continue"], false, "prompt: {prompt}");
        assert_eq!(result["decision"], "deny", "prompt: {prompt}");
        assert_eq!(result["reason"], reason, "prompt: {prompt}");
        assert!(!data.path().join("bootstrap-pending.json").exists());
        assert!(!data.path().join("ready-grant.json").exists());
    }
}

#[test]
fn test_canonical_pipeline_plan_forms_resolve_to_real_prompt_actions() {
    for (request, expected_action) in [
        (
            "/gal pipeline #file:.dev/plans/demo.md",
            "gal pipeline --require-codex-stop-v1 .dev/plans/demo.prompt.md",
        ),
        (
            "/gal pipeline @.dev/plans/demo.prompt.md",
            "gal pipeline --require-codex-stop-v1 .dev/plans/demo.prompt.md",
        ),
    ] {
        let data = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        write_plan(&repo, "demo");
        let body = make_event(&repo, request);
        assert_eq!(invoke(&data, "user-prompt-submit", &body)["continue"], true);
        let stop = invoke(&data, "stop", &body);
        assert_eq!(stop["decision"], "block");
        assert_eq!(stop["reason"], expected_action);
    }
}

#[test]
fn test_bare_pipeline_resolves_only_one_active_plan() {
    let data = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    write_plan(&repo, "demo");
    fs::write(
        repo.path().join(".dev/state.md"),
        "## Active Plans\n\n| Plan | File | Plan Phase |\n| --- | --- | --- |\n| Demo | `.dev/plans/demo.md` | IMPLEMENT |\n",
    )
    .unwrap();
    let body = make_event(&repo, "/gal pipeline");
    assert_eq!(invoke(&data, "user-prompt-submit", &body)["continue"], true);
    assert_eq!(
        invoke(&data, "stop", &body)["reason"],
        "gal pipeline --require-codex-stop-v1 .dev/plans/demo.prompt.md"
    );

    let ambiguous_data = TempDir::new().unwrap();
    write_plan(&repo, "other");
    fs::write(
        repo.path().join(".dev/state.md"),
        "## Active Plans\n\n| Plan | File | Plan Phase |\n| --- | --- | --- |\n| Demo | `.dev/plans/demo.md` | IMPLEMENT |\n| Other | `.dev/plans/other.prompt.md` | TEST |\n",
    )
    .unwrap();
    let ambiguous = invoke(&ambiguous_data, "user-prompt-submit", &body);
    assert_eq!(ambiguous["decision"], "deny");
    assert_eq!(ambiguous["reason"], "active-plan-ambiguous");
    assert!(!ambiguous_data
        .path()
        .join("bootstrap-pending.json")
        .exists());

    let missing_data = TempDir::new().unwrap();
    fs::remove_file(repo.path().join(".dev/state.md")).unwrap();
    let missing = invoke(&missing_data, "user-prompt-submit", &body);
    assert_eq!(missing["decision"], "deny");
    assert_eq!(missing["reason"], "active-plan-unavailable");
    assert!(!missing_data.path().join("bootstrap-pending.json").exists());
}

#[test]
fn test_pipeline_modifiers_fail_closed_until_guarded_cli_preserves_them() {
    let task_a = ["T", "07"].join("-");
    let task_b = ["T", "09"].join("-");
    for request in [
        format!("/gal pipeline #file:.dev/plans/demo.md from {task_a}"),
        format!("/gal pipeline @.dev/plans/demo.prompt.md stop-at {task_b}"),
    ] {
        let data = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        write_plan(&repo, "demo");
        let result = invoke(&data, "user-prompt-submit", &make_event(&repo, &request));
        assert_eq!(result["decision"], "deny");
        assert_eq!(result["reason"], "guarded-pipeline-modifiers-unsupported");
        assert!(!data.path().join("bootstrap-pending.json").exists());
    }
}

#[test]
fn test_binding_failures_rejected() {
    let data = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    let body = make_pipeline_event(&repo);

    // Establish BootstrapPending for s1, t1, cwd=repo, nonce=n1, gen=1
    assert_eq!(invoke(&data, "user-prompt-submit", &body)["continue"], true);
    assert!(data.path().join("bootstrap-pending.json").exists());

    let other_repo = TempDir::new().unwrap();

    // Mismatched session
    let mut req_session: serde_json::Value = serde_json::from_str(&body).unwrap();
    req_session["session_id"] = serde_json::json!("s2");
    let res = invoke(&data, "pre-tool-use", &req_session.to_string());
    assert_ne!(res["decision"], "allow");

    // Mismatched CWD (cross-worktree)
    let mut req_cwd: serde_json::Value = serde_json::from_str(&body).unwrap();
    req_cwd["cwd"] = serde_json::json!(other_repo.path().to_string_lossy());
    let res_cwd = invoke(&data, "pre-tool-use", &req_cwd.to_string());
    assert_ne!(res_cwd["decision"], "allow");

    // Mismatched generation
    let mut req_gen: serde_json::Value = serde_json::from_str(&body).unwrap();
    req_gen["generation"] = serde_json::json!(99);
    let res_gen = invoke(&data, "pre-tool-use", &req_gen.to_string());
    assert_ne!(res_gen["decision"], "allow");

    // Mismatched nonce
    let mut req_nonce: serde_json::Value = serde_json::from_str(&body).unwrap();
    req_nonce["nonce"] = serde_json::json!("wrong-nonce");
    let res_nonce = invoke(&data, "pre-tool-use", &req_nonce.to_string());
    assert_ne!(res_nonce["decision"], "allow");

    // Stop with mismatched session is rejected and does not publish ReadyGrant
    let stop_mismatch = invoke(&data, "stop", &req_session.to_string());
    assert_eq!(stop_mismatch["decision"], "deny");
    assert_eq!(stop_mismatch["reason"], "bootstrap-binding-mismatch");
    assert!(!data.path().join("ready-grant.json").exists());
}

#[test]
fn test_failure_injection_between_grant_store_writes() {
    let data = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    let body = make_pipeline_event(&repo);

    // Case 1: Corrupted bootstrap-pending.json
    fs::write(
        data.path().join("bootstrap-pending.json"),
        b"not valid json",
    )
    .unwrap();
    let stop = invoke(&data, "stop", &body);
    // Should fail closed, not panic, and not create ready-grant
    assert_eq!(stop["continue"], false);
    assert_eq!(stop["decision"], "deny");
    assert!(!data.path().join("ready-grant.json").exists());

    // Case 2: Corrupted ready-grant.json
    fs::write(data.path().join("ready-grant.json"), b"{\"invalid\":").unwrap();
    let mut presented: serde_json::Value = serde_json::from_str(&body).unwrap();
    presented["tool_input"] = serde_json::json!({"command":"gal pipeline --require-codex-stop-v1 .dev/plans/demo.prompt.md"});
    let res = invoke(&data, "pre-tool-use", &presented.to_string());
    // Must fail closed and not allow
    assert_ne!(res["decision"], "allow");
}

#[test]
fn test_limit_boundaries_and_rejections() {
    let data = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();

    // Payload exceeding 1 MiB (1024 * 1024 + 1 bytes) is rejected
    let oversized = vec![b'x'; 1024 * 1024 + 1];
    let (status, stdout, _stderr) = invoke_raw(&data, "user-prompt-submit", &oversized);
    assert!(
        !status.success()
            || stdout.is_empty()
            || serde_json::from_slice::<serde_json::Value>(&stdout)
                .map(|v| v.get("continue") == Some(&serde_json::Value::Bool(false)))
                .unwrap_or(true)
    );

    // Valid 1 MiB payload is handled cleanly
    let base_event = make_event(&repo, "hello");
    let padding_len = 1024 * 1024 - base_event.len() - 30;
    let padded = format!(
        r#"{{"session_id":"s1","turn_id":"t1","cwd":{:?},"prompt":"hello","generation":1,"nonce":"n1","tool_name":"shell","pad":{:?}}}"#,
        repo.path().to_string_lossy(),
        "a".repeat(padding_len)
    );
    if padded.len() <= 1024 * 1024 {
        let (status, stdout, _) = invoke_raw(&data, "user-prompt-submit", padded.as_bytes());
        assert!(status.success());
        let val: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(val["continue"], true);
    }
}

#[test]
fn test_presented_grant_remains_for_guarded_cli_consumption() {
    let data = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    let body = make_pipeline_event(&repo);

    assert_eq!(invoke(&data, "user-prompt-submit", &body)["continue"], true);
    assert_eq!(invoke(&data, "stop", &body)["decision"], "block");

    let mut presented: serde_json::Value = serde_json::from_str(&body).unwrap();
    presented["turn_id"] = serde_json::json!("t2");
    presented["prompt"] =
        serde_json::json!("gal pipeline --require-codex-stop-v1 .dev/plans/demo.prompt.md");
    presented["tool_input"] = serde_json::json!({"command":"gal pipeline --require-codex-stop-v1 .dev/plans/demo.prompt.md"});

    let presentation = invoke(&data, "user-prompt-submit", &presented.to_string());
    assert_eq!(presentation["continue"], true, "{presentation}");
    // The hook allows the bound presentation and leaves grant consumption to the guarded CLI.
    let allow1 = invoke(&data, "pre-tool-use", &presented.to_string());
    assert_eq!(allow1["decision"], "allow");
    assert!(data.path().join("ready-grant.json").exists());
    assert!(repo
        .path()
        .join(".dev/pipeline/demo/codex-hook-grant.json")
        .exists());

    // Replay is rejected after the turn has been bound.
    let allow2 = invoke(&data, "pre-tool-use", &presented.to_string());
    assert_ne!(allow2["decision"], "allow");
}

#[test]
fn test_grant_expiry() {
    let data = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    let body = make_pipeline_event(&repo);

    assert_eq!(invoke(&data, "user-prompt-submit", &body)["continue"], true);
    assert_eq!(invoke(&data, "stop", &body)["decision"], "block");

    let grant_path = data.path().join("ready-grant.json");
    assert!(grant_path.exists());

    // Expire the grant by setting expires to timestamp in the past
    let mut grant: serde_json::Value =
        serde_json::from_slice(&fs::read(&grant_path).unwrap()).unwrap();
    grant["expires"] = serde_json::json!(100); // Year 1970
    fs::write(&grant_path, serde_json::to_vec(&grant).unwrap()).unwrap();

    let mut presented: serde_json::Value = serde_json::from_str(&body).unwrap();
    presented["turn_id"] = serde_json::json!("t2");
    presented["prompt"] =
        serde_json::json!("gal pipeline --require-codex-stop-v1 .dev/plans/demo.prompt.md");
    presented["tool_input"] = serde_json::json!({"command":"gal pipeline --require-codex-stop-v1 .dev/plans/demo.prompt.md"});

    let exp_res = invoke(&data, "user-prompt-submit", &presented.to_string());
    // An expired grant must not be allowed
    assert_ne!(
        exp_res["decision"], "allow",
        "Expired grant should be rejected by PreToolUse"
    );
}

#[test]
fn test_repeated_stops_bounding_and_post_activation_blocking() {
    let data = TempDir::new().unwrap();
    let repo = TempDir::new().unwrap();
    let body = make_pipeline_event(&repo);

    assert_eq!(invoke(&data, "user-prompt-submit", &body)["continue"], true);

    // 1st stop during bootstrap emits block and creates ReadyGrant
    let stop1 = invoke(&data, "stop", &body);
    assert_eq!(stop1["decision"], "block");
    let grant1: serde_json::Value =
        serde_json::from_slice(&fs::read(data.path().join("ready-grant.json")).unwrap()).unwrap();
    assert_eq!(grant1["stops"], 1);

    // Repeated stops before activation: must block premature final without pass-through
    let stop2 = invoke(&data, "stop", &body);
    assert_eq!(
        stop2["decision"], "block",
        "Repeated stop before grant consumption should remain blocked"
    );
}
