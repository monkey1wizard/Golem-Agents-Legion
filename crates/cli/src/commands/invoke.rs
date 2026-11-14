use dispatch::dispatch::{spawn_generic_executor, AttemptContext, SpawnConfig};
use dispatch::executors::{get_adapter, AdapterInvocationInput, SpecDelivery};
use dispatch::routing::{
    load_routing_default, parse_one_shot_route_options, resolve_one_shot_route,
    ResolvedOneShotRoute,
};
use gal_engine::ExitCode;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_TIMEOUT_SECS: u64 = 1800;
const MAX_INPUT_BYTES: usize = 1024 * 1024;

fn emit_error(message: &str, code: ExitCode) -> ExitCode {
    let body = serde_json::to_string(&serde_json::json!({
        "schema_version": 1,
        "error": message,
        "observed_route": "unknown",
        "terminal_state": "rejected"
    }))
    .unwrap_or_else(|_| "{\"error\":\"invoke failed\"}".to_owned());
    eprintln!("{body}");
    code
}

fn take_value<'a>(args: &'a [String], index: &mut usize, option: &str) -> Result<&'a str, String> {
    *index += 1;
    let value = args
        .get(*index)
        .ok_or_else(|| format!("{option} requires a value"))?;
    *index += 1;
    Ok(value)
}

fn parse_args(
    args: &[String],
) -> Result<
    (
        Vec<String>,
        Option<InputSource>,
        PathBuf,
        Option<u64>,
        Option<String>,
        Option<PathBuf>,
    ),
    String,
> {
    let mut route_args = Vec::new();
    let mut input = None;
    let mut workdir = None;
    let mut timeout = None;
    let mut run = None;
    let mut plan = None;
    let mut positional = Vec::new();
    let mut separator = false;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        if separator {
            positional.push(args[index].clone());
            index += 1;
            continue;
        }
        match arg {
            "--" => {
                separator = true;
                index += 1;
            }
            "--executor" | "--host-subagent" | "--model" | "--effort" => {
                route_args.push(args[index].clone());
                let value = take_value(args, &mut index, arg)?;
                route_args.push(value.to_owned());
            }
            "--task" => {
                let value = take_value(args, &mut index, arg)?;
                if input.replace(InputSource::Text(value.to_owned())).is_some() {
                    return Err("supply exactly one input source".to_owned());
                }
            }
            "--task-file" => {
                let value = take_value(args, &mut index, arg)?;
                if input
                    .replace(InputSource::File(PathBuf::from(value)))
                    .is_some()
                {
                    return Err("supply exactly one input source".to_owned());
                }
            }
            "--stdin" => {
                if input.replace(InputSource::Stdin).is_some() {
                    return Err("supply exactly one input source".to_owned());
                }
                index += 1;
            }
            "--workdir" => {
                let value = take_value(args, &mut index, arg)?;
                if workdir.replace(PathBuf::from(value)).is_some() {
                    return Err("--workdir may be specified only once".to_owned());
                }
            }
            "--run" | "--plan" => {
                let value = take_value(args, &mut index, arg)?;
                if arg == "--run" {
                    if run.replace(value.to_owned()).is_some() {
                        return Err("--run may be specified only once".into());
                    }
                } else if plan.replace(PathBuf::from(value)).is_some() {
                    return Err("--plan may be specified only once".into());
                }
            }
            "--timeout" => {
                let value = take_value(args, &mut index, arg)?;
                let seconds = value
                    .parse::<u64>()
                    .ok()
                    .filter(|seconds| *seconds > 0)
                    .ok_or_else(|| "--timeout must be a positive number of seconds".to_owned())?;
                if timeout.replace(seconds).is_some() {
                    return Err("--timeout may be specified only once".to_owned());
                }
            }
            option if option.starts_with('-') => {
                return Err(format!("unknown invoke option '{option}'"));
            }
            _ => {
                return Err(
                    "input must be supplied with --task, --task-file, --stdin, or after --"
                        .to_owned(),
                )
            }
        }
    }

    if positional.len() > 1 {
        return Err("exactly one input is required after --".to_owned());
    }
    if !positional.is_empty() && input.is_some() {
        return Err("supply exactly one input source".to_owned());
    }
    if let Some(value) = positional.pop() {
        input = Some(InputSource::Text(value));
    }

    let workdir = match workdir {
        Some(path) => path,
        None => {
            std::env::current_dir().map_err(|error| format!("cannot resolve workdir: {error}"))?
        }
    };
    Ok((route_args, input, workdir, timeout, run, plan))
}

enum InputSource {
    Text(String),
    File(PathBuf),
    Stdin,
}

fn read_input(input: Option<InputSource>, caller_dir: &Path) -> Result<String, String> {
    let bytes = match input.ok_or_else(|| "exactly one input is required".to_owned())? {
        InputSource::Text(value) => value.into_bytes(),
        InputSource::File(path) => {
            let path = if path.is_absolute() {
                path
            } else {
                caller_dir.join(path)
            };
            let file = std::fs::File::open(&path)
                .map_err(|error| format!("cannot read input file '{}': {error}", path.display()))?;
            let mut bytes = Vec::new();
            file.take((MAX_INPUT_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("cannot read input file '{}': {error}", path.display()))?;
            bytes
        }
        InputSource::Stdin => {
            let mut bytes = Vec::new();
            std::io::stdin()
                .take((MAX_INPUT_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("cannot read stdin: {error}"))?;
            bytes
        }
    };
    if bytes.len() > MAX_INPUT_BYTES {
        return Err("input exceeds 1 MiB".to_owned());
    }
    if bytes.is_empty() {
        return Err("input must not be empty".to_owned());
    }
    String::from_utf8(bytes).map_err(|_| "input must be valid UTF-8".to_owned())
}

pub(crate) fn cmd_invoke(args: &[String]) -> ExitCode {
    let args = args.get(1..).unwrap_or_default();
    let caller_dir = match std::env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            return emit_error(
                &format!("cannot resolve caller directory: {error}"),
                ExitCode::Error,
            )
        }
    };
    let (route_args, input, requested_workdir, timeout_override, run, plan) = match parse_args(args)
    {
        Ok(parsed) => parsed,
        Err(error) => return emit_error(&error, ExitCode::Usage),
    };
    let (request, consumed) = match parse_one_shot_route_options(&route_args) {
        Ok(parsed) => parsed,
        Err(error) => return emit_error(&error.to_string(), ExitCode::Usage),
    };
    if consumed != route_args.len() {
        return emit_error("invalid route options", ExitCode::Usage);
    }
    if request.host_subagent.is_some() {
        return emit_error(
            "host selectors are not supported by gal invoke",
            ExitCode::Usage,
        );
    }
    if request.executor.is_none() {
        return emit_error("gal invoke requires --executor", ExitCode::Usage);
    }

    let workdir = match requested_workdir.canonicalize() {
        Ok(path) if path.is_dir() => path,
        Ok(_) => return emit_error("workdir must be a directory", ExitCode::Usage),
        Err(error) => {
            return emit_error(&format!("cannot resolve workdir: {error}"), ExitCode::Usage)
        }
    };
    let spec = match read_input(input, &caller_dir) {
        Ok(spec) => spec,
        Err(error) => return emit_error(&error, ExitCode::Usage),
    };

    let table = load_routing_default();
    let resolved = match resolve_one_shot_route(&request, &table, None) {
        Ok(ResolvedOneShotRoute::Routed(binding)) => binding,
        Ok(ResolvedOneShotRoute::Host(_)) => {
            return emit_error(
                "host selectors are not supported by gal invoke",
                ExitCode::Usage,
            )
        }
        Err(error) => return emit_error(&error.to_string(), ExitCode::Usage),
    };
    let route = resolved.route();
    let Some(adapter) = get_adapter(&route.executor) else {
        return emit_error("unsupported executor", ExitCode::Usage);
    };
    if route.effort.is_some() && !adapter.supports_effort() {
        return emit_error(
            "selected executor does not support effort selection",
            ExitCode::Usage,
        );
    }

    let timeout_secs = timeout_override
        .or(route.timeout_secs)
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    let invocation = adapter.build_invocation(&AdapterInvocationInput {
        model: &route.model,
        workdir: &workdir,
        spec: &spec,
        effort: route.effort.as_deref(),
        mcp_disable_servers: &[],
        timeout_secs: Some(timeout_secs),
    });
    let (executor_args, spec) = match invocation.delivery {
        SpecDelivery::Stdin => (invocation.args, spec.as_str()),
        SpecDelivery::CliFlag(_) => (invocation.args, ""),
    };

    let invocation_id = format!(
        "invoke-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let log_dir = workdir
        .join(".dev")
        .join("invocations")
        .join(&invocation_id);
    let config = SpawnConfig::generic(
        &route.executor,
        executor_args,
        spec,
        &workdir,
        timeout_secs,
        &route.model,
        &invocation_id,
        log_dir,
    );
    let context = AttemptContext::generic(&invocation_id, Some(resolved.digest()));
    let plan_path = plan.as_ref().map(|path| {
        if path.is_absolute() {
            path.clone()
        } else {
            caller_dir.join(path)
        }
    });
    let plan_id = plan_path
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|name| {
            name.strip_suffix(".prompt.md")
                .or_else(|| name.strip_suffix(".md"))
                .unwrap_or(name)
        })
        .unwrap_or("standalone");
    let run_id = run
        .or_else(|| {
            plan_path.as_deref().and_then(|path| {
                super::pipeline_preflight::read_pin(path)
                    .ok()
                    .map(|pin| pin.run_id)
            })
        })
        .unwrap_or_else(|| format!("standalone-{invocation_id}"));
    if let Ok(common_dir) = super::usage::git_common_dir_in(&workdir) {
        if let Err(error) = super::usage_capture::expect_attempt(
            &common_dir,
            &workdir,
            &run_id,
            plan_id,
            &invocation_id,
            "invoke",
            plan_path.as_deref(),
        ) {
            eprintln!("gal usage: expected attempt not stored for {invocation_id}: {error}");
        }
    }
    let outcome = spawn_generic_executor(&config, &context);
    let unavailable_usage = dispatch::dispatch::DispatchUsage::not_attempted();
    let usage = outcome
        .as_ref()
        .map(|outcome| &outcome.result.usage)
        .unwrap_or(&unavailable_usage);
    match super::usage::git_common_dir_in(&workdir) {
        Ok(common_dir) => {
            if let Err(error) = super::usage_capture::capture_native(
                &common_dir,
                &workdir,
                &run_id,
                plan_id,
                &invocation_id,
                &super::usage_capture::AttemptAttribution {
                    configured_model: Some(route.model.clone()),
                    effort: route.effort.clone(),
                    phase: "invoke".into(),
                    session_id: outcome
                        .as_ref()
                        .ok()
                        .and_then(|outcome| outcome.result.session_id.clone()),
                    route_source: Some("one-shot".into()),
                    one_shot_override: Some(true),
                    source_plan: plan_path.clone(),
                    ..Default::default()
                },
                usage,
            ) {
                eprintln!("gal usage: capture failed for {invocation_id}: {error}");
            }
        }
        Err(_) => eprintln!(
            "gal usage: no-common-dir; local log: {}",
            outcome
                .as_ref()
                .map(|outcome| outcome.journal_path.as_path())
                .unwrap_or(&config.log_dir)
                .display()
        ),
    }
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => return emit_error(&error.to_string(), ExitCode::Error),
    };

    let stdout = outcome.result.stdout.as_bytes();
    if let Err(error) = std::io::stdout().write_all(stdout) {
        return emit_error(
            &format!("cannot write executor output: {error}"),
            ExitCode::Error,
        );
    }
    if let Some(emergency) = outcome.emergency_stderr.as_deref() {
        let _ = std::io::stderr().write_all(emergency.as_bytes());
        let _ = std::io::stderr().write_all(b"\n");
    }
    let report = serde_json::json!({
        "schema_version": 1,
        "invocation_id": invocation_id,
        "context": "generic",
        "executor": route.executor,
        "model": route.model,
        "effort": route.effort,
        "observed_route": "unknown",
        "route_digest": resolved.digest(),
        "terminal_state": outcome.result.terminal_state.as_str(),
        "exit_code": outcome.result.exit_code,
        "duration_ms": outcome.result.duration_ms,
        "stdout_stored_bytes": outcome.stdout.stored_bytes,
        "stdout_sha256": outcome.stdout.stored_sha256,
        "stdout_discarded_bytes": outcome.stdout.discarded_bytes,
        "stderr_stored_bytes": outcome.stderr.stored_bytes,
        "stderr_sha256": outcome.stderr.stored_sha256,
        "stderr_discarded_bytes": outcome.stderr.discarded_bytes,
        "evidence_write_failed": outcome.evidence_write_failed
    });
    match serde_json::to_string(&report) {
        Ok(json) => {
            let _ = writeln!(std::io::stderr(), "{json}");
        }
        Err(error) => {
            return emit_error(
                &format!("cannot serialize invoke report: {error}"),
                ExitCode::Error,
            )
        }
    }
    if outcome.result.exit_code == Some(0)
        && outcome.result.terminal_state.as_str() == "completed"
        && !outcome.evidence_write_failed
    {
        ExitCode::Success
    } else {
        ExitCode::Error
    }
}
