use dispatch::routing::{
    load_routing, parse_one_shot_route_options, resolve_one_shot_route, HostRouteCapability,
    OneShotRouteError, OneShotRouteRequest, ResolvedOneShotRoute,
};
use sha2::{Digest, Sha256};
use std::fs;
use tempfile::TempDir;

fn routing_fixture() -> (TempDir, std::path::PathBuf, Vec<u8>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let bytes = br#"{
        "executorRouting": {
            "executors": {
                "codex": { "model": "gpt-default", "effort": "medium", "timeoutSecs": 45 },
                "claude": { "model": "claude-default" }
            },
            "combinations": {
                "private-combo": { "executor": "codex", "model": "combo-model", "effort": "high" }
            },
            "pipeline": { "CODER": { "executor": "private-combo", "model": "role-model" } }
        }
    }"#
    .to_vec();
    fs::write(&path, &bytes).unwrap();
    (dir, path, bytes)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn request(executor: &str, model: Option<&str>, effort: Option<&str>) -> OneShotRouteRequest {
    OneShotRouteRequest {
        executor: Some(executor.into()),
        host_subagent: None,
        model: model.map(str::to_owned),
        effort: effort.map(str::to_owned),
    }
}

#[test]
fn shared_route_grammar_parses_options_and_stops_at_separator() {
    let args = [
        "--executor".into(),
        "codex".into(),
        "--model".into(),
        "gpt-x".into(),
        "--effort".into(),
        "high".into(),
        "--".into(),
        "--host-subagent".into(),
        "claude".into(),
    ];
    let (parsed, next) = parse_one_shot_route_options(&args).unwrap();
    assert_eq!(parsed, request("codex", Some("gpt-x"), Some("high")));
    assert_eq!(next, 7);
    assert_eq!(&args[next..], &["--host-subagent", "claude"]);
}

#[test]
fn shared_route_grammar_has_stable_duplicate_missing_and_conflict_errors() {
    let duplicate = ["--model".into(), "a".into(), "--model".into(), "b".into()];
    assert_eq!(
        parse_one_shot_route_options(&duplicate).unwrap_err(),
        OneShotRouteError::DuplicateOption("--model")
    );
    let missing = ["--executor".into()];
    assert_eq!(
        parse_one_shot_route_options(&missing).unwrap_err(),
        OneShotRouteError::MissingValue("--executor")
    );
    let unknown = ["--combination".into(), "private-combo".into()];
    assert_eq!(
        parse_one_shot_route_options(&unknown).unwrap_err(),
        OneShotRouteError::UnknownOption("--combination".into())
    );
    let table = dispatch::routing::RoutingTable::default();
    let both = OneShotRouteRequest {
        executor: Some("codex".into()),
        host_subagent: Some("claude".into()),
        ..OneShotRouteRequest::default()
    };
    assert_eq!(
        resolve_one_shot_route(&both, &table, None).unwrap_err(),
        OneShotRouteError::ConflictingSelectors
    );
}

#[test]
fn routed_route_uses_executor_defaults_and_explicit_values_without_combinations() {
    let (_dir, path, config_bytes) = routing_fixture();
    let before_hash = sha256(&config_bytes);
    assert_eq!(
        before_hash,
        "c6aab280d474f507aea296787ffde28e967dbba5725663fadba7bdcfafe254b6"
    );
    let table = load_routing(&path);

    let defaulted = resolve_one_shot_route(&request("codex", None, None), &table, None).unwrap();
    let explicit = resolve_one_shot_route(
        &request("codex", Some("gpt-explicit"), Some("xhigh")),
        &table,
        None,
    )
    .unwrap();
    let hidden_combination =
        resolve_one_shot_route(&request("private-combo", None, None), &table, None);

    assert!(matches!(
        &defaulted,
        ResolvedOneShotRoute::Routed(binding)
            if binding.route().executor == "codex"
                && binding.route().model == "gpt-default"
                && binding.route().effort.as_deref() == Some("medium")
                && binding.route().timeout_secs == Some(45)
                && binding.route().ssh_target.is_none()
                && binding.route().remote_workdir.is_none()
    ));
    assert_eq!(
        defaulted.digest(),
        "sha256:5bb8af04f74690154751d1e35d7d8dc6a612d6afc7b210cd83549eee9d47f781"
    );
    assert!(matches!(
        &explicit,
        ResolvedOneShotRoute::Routed(binding)
            if binding.route().model == "gpt-explicit"
                && binding.route().effort.as_deref() == Some("xhigh")
                && binding.route().ssh_target.is_none()
                && binding.route().remote_workdir.is_none()
    ));
    assert_eq!(
        explicit.digest(),
        "sha256:7a6d447a50efc2a26d21abf15e2f1d618e6eae24fbdccba685b8da6bff64debd"
    );
    assert_eq!(
        hidden_combination.unwrap_err(),
        OneShotRouteError::MissingModel
    );
    assert_eq!(fs::read(&path).unwrap(), config_bytes);
    assert_eq!(sha256(&fs::read(&path).unwrap()), before_hash);
}

#[test]
fn route_digest_is_stable_and_effort_requires_an_effective_model() {
    let (_dir, path, _) = routing_fixture();
    let table = load_routing(&path);
    let first = resolve_one_shot_route(&request("claude", None, None), &table, None).unwrap();
    let second = resolve_one_shot_route(&request("claude", None, None), &table, None).unwrap();
    assert_eq!(first.digest(), second.digest());
    assert_eq!(
        serde_json::to_string(match &first {
            ResolvedOneShotRoute::Routed(binding) => binding.route(),
            ResolvedOneShotRoute::Host(_) => unreachable!(),
        })
        .unwrap(),
        r#"{"executor":"claude","model":"claude-default","ssh_target":null,"remote_workdir":null,"effort":null,"timeout_secs":null}"#
    );
    assert_eq!(
        resolve_one_shot_route(&request("unknown", None, Some("high")), &table, None).unwrap_err(),
        OneShotRouteError::EffortWithoutModel
    );
}

#[test]
fn host_route_uses_only_caller_capability_and_fails_closed() {
    let table = dispatch::routing::RoutingTable::default();
    let capability = HostRouteCapability {
        host: "claude".into(),
        model: Some("host-current".into()),
        effort: Some("medium".into()),
        supports_model_selection: true,
        supports_effort_selection: true,
    };
    let host_request = OneShotRouteRequest {
        executor: None,
        host_subagent: Some("claude".into()),
        model: Some("host-explicit".into()),
        effort: Some("high".into()),
    };
    let route = resolve_one_shot_route(&host_request, &table, Some(&capability)).unwrap();
    assert!(matches!(
        &route,
        ResolvedOneShotRoute::Host(binding)
            if binding.route().host == "claude" && binding.route().model == "host-explicit" && binding.route().effort.as_deref() == Some("high")
    ));
    assert_eq!(
        route.digest(),
        "sha256:14b6274ea64029aadfbd4375489e39ee288df3f18ddd23906a99f5d6cc1c3556"
    );

    let unsupported = HostRouteCapability {
        supports_model_selection: false,
        ..capability.clone()
    };
    assert_eq!(
        resolve_one_shot_route(&host_request, &table, Some(&unsupported)).unwrap_err(),
        OneShotRouteError::UnsupportedHostModelSelection
    );
    assert_eq!(
        resolve_one_shot_route(&host_request, &table, None).unwrap_err(),
        OneShotRouteError::MissingSelector
    );
}
