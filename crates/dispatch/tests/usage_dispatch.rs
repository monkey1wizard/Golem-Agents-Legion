//! Usage capture on dispatch terminal paths, driven by fake executors that print fixture output.
//!
//! Every case runs in one test because each case rewrites `PATH` to find its own fake executor.

use std::fs;
use std::path::Path;

use dispatch::dispatch::{
    dedupe_usage_copies, spawn_executor, DispatchResult, SpawnConfig, UsageCollection,
};
use dispatch::executors::{NativeUsage, UsageCapability};
use dispatch::usage::{Availability, TokenMetric, TokenUsage};
use serde_json::Value;
use tempfile::TempDir;

const FIXTURE: &str = include_str!("fixtures/usage_processes.json");

/// Writes a fake executor named `executor` that prints `emit` lines, then holds for `hold_secs`.
fn write_fake_executor(bin_dir: &Path, executor: &str, emit: &[String], hold_secs: u64) {
    #[cfg(target_os = "windows")]
    {
        let mut script = String::from("@echo off\r\n");
        for line in emit {
            script.push_str(&format!("echo {line}\r\n"));
        }
        if hold_secs > 0 {
            script.push_str(&format!("ping -n {} 127.0.0.1 >nul\r\n", hold_secs + 1));
        }
        script.push_str("exit /b 0\r\n");
        fs::write(bin_dir.join(format!("{executor}.cmd")), script).unwrap();
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut script = String::from("#!/bin/sh\n");
        for line in emit {
            script.push_str(&format!("printf '%s\\n' '{line}'\n"));
        }
        if hold_secs > 0 {
            script.push_str(&format!("sleep {hold_secs}\n"));
        }
        let path = bin_dir.join(executor);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn write_fake_executor_writing_receipt(
    bin_dir: &Path,
    executor: &str,
    emit: &[String],
    receipt_path: &Path,
    receipt_content: &str,
) {
    #[cfg(target_os = "windows")]
    {
        let mut script = String::from("@echo off\r\n");
        for line in emit {
            script.push_str(&format!("echo {line}\r\n"));
        }
        script.push_str(&format!(
            "echo {receipt_content}>\"{}\"\r\n",
            receipt_path.display()
        ));
        script.push_str("exit /b 0\r\n");
        fs::write(bin_dir.join(format!("{executor}.cmd")), script).unwrap();
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut script = String::from("#!/bin/sh\n");
        for line in emit {
            script.push_str(&format!("printf '%s\\n' '{line}'\n"));
        }
        script.push_str(&format!(
            "printf '%s\\n' '{receipt_content}' > '{}'\n",
            receipt_path.display()
        ));
        let path = bin_dir.join(executor);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn test_config(tmp: &TempDir, executor: &str, timeout_secs: u64) -> SpawnConfig {
    SpawnConfig {
        executor: executor.to_string(),
        executor_args: Vec::new(),
        spec: "usage probe".to_string(),
        workdir: tmp.path().to_path_buf(),
        timeout_secs,
        task_id: "T-TEST".to_string(),
        phase: "test".to_string(),
        actual_model: "test-model".to_string(),
        log_dir: tmp
            .path()
            .join(".dev/pipeline")
            .join(gal_foundation::time::utc_date_compact())
            .join("test-direct"),
        receipt_path: None,
        contract_provenance: None,
    }
}

/// The reported value of one counter, or `None` when it is absent, unsupported, or malformed.
fn observed<F>(result: &DispatchResult, pick: F) -> Option<u64>
where
    F: for<'a> Fn(&'a TokenUsage) -> &'a TokenMetric,
{
    let observation = result.usage.native.observations.first()?;
    reported_value(pick(&observation.tokens))
}

fn assert_case(case: &Value, result: &DispatchResult) {
    let name = case["name"].as_str().unwrap();
    let expect = &case["expect"];

    assert_eq!(
        result.terminal_state.as_str(),
        expect["terminal_state"].as_str().unwrap(),
        "{name}: terminal state"
    );
    assert_eq!(
        result.exit_code.map(i64::from),
        expect["exit_code"].as_i64(),
        "{name}: exit code"
    );

    let capability = match &result.usage.native.capability {
        UsageCapability::Mapped { .. } => "mapped",
        UsageCapability::Unsupported { .. } => "unsupported",
    };
    assert_eq!(
        capability,
        expect["capability"].as_str().unwrap(),
        "{name}: capability"
    );

    let collection = match &result.usage.collection {
        UsageCollection::Complete => "complete",
        UsageCollection::Partial { reason } => {
            assert!(
                !reason.is_empty(),
                "{name}: partial reason must be recorded"
            );
            "partial"
        }
        UsageCollection::NotAttempted => "not-attempted",
    };
    assert_eq!(
        collection,
        expect["collection"].as_str().unwrap(),
        "{name}: collection status"
    );

    let counters: [(&str, fn(&TokenUsage) -> &TokenMetric); 4] = [
        ("input_tokens", |tokens| &tokens.input),
        ("output_tokens", |tokens| &tokens.output),
        ("cached_input_tokens", |tokens| &tokens.cached_input),
        ("reasoning_tokens", |tokens| &tokens.reasoning),
    ];
    for (key, pick) in counters {
        if let Some(expected) = expect[key].as_u64() {
            assert_eq!(observed(result, pick), Some(expected), "{name}: {key}");
        }
    }

    let log = fs::read_to_string(&result.log_path).unwrap();
    let has_extension = log.contains("usage_extension:  v1\n");
    // A launched attempt always writes the extension, even when its executor has no usage
    // capability. Only an attempt that never launched writes none.
    let launched = !matches!(result.usage.collection, UsageCollection::NotAttempted);
    assert_eq!(
        has_extension, launched,
        "{name}: usage extension in executor log"
    );
    if has_extension {
        assert!(
            log.contains(&format!("usage_collection: {collection}")),
            "{name}: log records the collection status"
        );
    }
    if launched && capability == "unsupported" {
        assert_unsupported_observation(name, &log, case["executor"].as_str().unwrap());
    }

    let observation = result.usage.native.observations.first();
    eprintln!(
        "usage_dispatch case={name} terminal={} exit={:?} capability={capability} collection={collection} \
         input={:?} output={:?} cached_input={:?} reasoning={:?} log_extension={has_extension}",
        result.terminal_state.as_str(),
        result.exit_code,
        observation.and_then(|o| reported_value(&o.tokens.input)),
        observation.and_then(|o| reported_value(&o.tokens.output)),
        observation.and_then(|o| reported_value(&o.tokens.cached_input)),
        observation.and_then(|o| reported_value(&o.tokens.reasoning)),
    );
}

/// Checks the single logged observation of an unsupported attempt: the executor is named, the
/// source is `unsupported`, and every token metric and the cost are unsupported with one reason.
fn assert_unsupported_observation(name: &str, log: &str, executor: &str) {
    let line = log
        .lines()
        .find_map(|line| line.strip_prefix("usage_observations: "))
        .unwrap_or_else(|| panic!("{name}: log has no usage_observations line"));
    let observations: Value = serde_json::from_str(line).unwrap();
    let observations = observations.as_array().unwrap();
    assert_eq!(observations.len(), 1, "{name}: one logged observation");

    let observation = &observations[0];
    assert_eq!(
        observation["identity"]["executor"].as_str(),
        Some(executor),
        "{name}: logged executor"
    );
    assert_eq!(
        observation["source"]["source_version"].as_str(),
        Some("unsupported"),
        "{name}: logged source"
    );

    let metrics = [
        &observation["cost"]["value"],
        &observation["tokens"]["input"]["count"]["value"],
        &observation["tokens"]["output"]["count"]["value"],
        &observation["tokens"]["cached_input"]["count"]["value"],
        &observation["tokens"]["cache_write"]["count"]["value"],
        &observation["tokens"]["reasoning"]["count"]["value"],
        &observation["tokens"]["other"]["count"]["value"],
    ];
    let reason = metrics[0]["reason"].as_str().unwrap();
    assert!(
        !reason.is_empty(),
        "{name}: unsupported reason must be recorded"
    );
    for metric in metrics {
        assert_eq!(
            metric["state"].as_str(),
            Some("unsupported"),
            "{name}: metric state"
        );
        assert_eq!(
            metric["reason"].as_str(),
            Some(reason),
            "{name}: metric reason"
        );
    }
}

/// Replays the attempt as an SSH return would, then checks that the two copies count once.
fn assert_replay(case: &Value, cfg: &SpawnConfig, first: &DispatchResult) {
    let name = case["name"].as_str().unwrap();

    let replay = spawn_executor(cfg).unwrap();
    assert_eq!(
        replay.usage.evidence_id, first.usage.evidence_id,
        "{name}: replay evidence id"
    );
    assert_eq!(
        replay.usage.native, first.usage.native,
        "{name}: replay native usage"
    );

    let mut conflicting = first.usage.clone();
    conflicting.native = NativeUsage::unsupported("altered copy");
    assert!(
        dedupe_usage_copies(vec![first.usage.clone(), conflicting]).is_err(),
        "{name}: copies that disagree must be rejected"
    );

    let copies = dedupe_usage_copies(vec![first.usage.clone(), replay.usage.clone()]).unwrap();
    assert_eq!(
        copies.len(),
        case["expect"]["copies_counted"].as_u64().unwrap() as usize,
        "{name}: copies counted"
    );
}

fn reported_value(counter: &TokenMetric) -> Option<u64> {
    match &counter.count.value {
        Availability::Reported { value } => Some(value.get()),
        _ => None,
    }
}

static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn usage_is_captured_on_every_terminal_case() {
    let _lock = PATH_LOCK.lock().unwrap();
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let original_path = std::env::var_os("PATH").unwrap_or_default();

    for case in fixture["cases"].as_array().unwrap() {
        let executor = case["executor"].as_str().unwrap();
        let emit: Vec<String> = case["emit"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| line.as_str().unwrap().to_string())
            .collect();
        let hold_secs = case["hold_secs"].as_u64().unwrap();
        let timeout_secs = case["timeout_secs"].as_u64().unwrap();

        let tmp = TempDir::new().unwrap();
        let bin_dir = tmp.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        write_fake_executor(&bin_dir, executor, &emit, hold_secs);

        let mut paths = vec![bin_dir];
        paths.extend(std::env::split_paths(&original_path));
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());

        let cfg = test_config(&tmp, executor, timeout_secs);
        let result = spawn_executor(&cfg).unwrap();
        assert_case(case, &result);
        if case["replay"].as_bool() == Some(true) {
            assert_replay(case, &cfg, &result);
        }
    }
    std::env::set_var("PATH", &original_path);
}

#[test]
fn executor_without_usage_adapter_writes_unsupported_gap() {
    let _lock = PATH_LOCK.lock().unwrap();
    let original_path = std::env::var_os("PATH").unwrap_or_default();

    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let executor = "fake_unmapped_executor";
    assert!(
        dispatch::executors::get_adapter(executor).is_none(),
        "executor must have no adapter"
    );

    let receipt_path = tmp.path().join(".dev/pipeline/test.receipt.md");
    let receipt_content = "receipt-content-unmapped";
    fs::create_dir_all(receipt_path.parent().unwrap()).unwrap();

    write_fake_executor_writing_receipt(
        &bin_dir,
        executor,
        &["unmapped executor launched and completed".to_string()],
        &receipt_path,
        receipt_content,
    );

    let mut paths = vec![bin_dir];
    paths.extend(std::env::split_paths(&original_path));
    std::env::set_var("PATH", std::env::join_paths(paths).unwrap());

    let mut cfg = test_config(&tmp, executor, 10);
    cfg.receipt_path = Some(receipt_path.clone());

    let result = spawn_executor(&cfg).unwrap();
    std::env::set_var("PATH", &original_path);

    // 1. Terminal state is completed and exit code is 0
    assert_eq!(result.terminal_state.as_str(), "completed");
    assert_eq!(result.exit_code, Some(0));

    // 2. Receipt bytes are unchanged
    let receipt_after = fs::read_to_string(&receipt_path).unwrap();
    assert_eq!(
        receipt_after.trim(),
        receipt_content,
        "receipt bytes must be unchanged"
    );

    // 3. Capability is unsupported carrying the adapter gap reason
    match &result.usage.native.capability {
        UsageCapability::Unsupported { reason } => {
            assert_eq!(reason, "executor has no native usage adapter");
        }
        UsageCapability::Mapped { .. } => panic!("expected unsupported capability"),
    }
    assert_eq!(result.usage.collection, UsageCollection::Complete);
    assert!(
        result.usage.elapsed.is_some(),
        "elapsed duration must be recorded"
    );

    // 4. Log file carries versioned extension with single unsupported observation and elapsed time
    let log = fs::read_to_string(&result.log_path).unwrap();
    assert!(log.contains("usage_extension:  v1\n"));
    assert!(log.contains("usage_source:     unsupported\n"));
    assert!(log.contains("usage_collection: complete\n"));
    assert!(log.contains("usage_elapsed_ms: "));
    assert!(!log.contains("usage_elapsed_ms: none\n"));

    assert_unsupported_observation("adapterless executor", &log, executor);

    // 5. Ensure that no numeric zero was written
    let obs_line = log
        .lines()
        .find(|l| l.starts_with("usage_observations: "))
        .expect("log has usage_observations");
    assert!(!obs_line.contains(":0,") && !obs_line.contains(":0}"));
}
