use dispatch::executors::{get_adapter, UsageCapability, MAX_NATIVE_USAGE_BYTES};
use dispatch::usage::{Availability, CounterRelation, CounterSemantics};
use serde_json::Value;

const FIXTURES: &str = include_str!("fixtures/usage_native.json");

/// Checks each expected field against the serialized observation. Fields the fixture does
/// not name are ignored, so a case states only the counters it proves.
fn assert_subset(actual: &Value, expected: &Value, path: &str) {
    match expected {
        Value::Object(expected_map) => {
            for (key, expected_value) in expected_map {
                let child = actual
                    .get(key)
                    .unwrap_or_else(|| panic!("{path}.{key} is missing from {actual}"));
                assert_subset(child, expected_value, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn native_usage_fixtures_match_expected_scopes() {
    let fixtures: Value = serde_json::from_str(FIXTURES).expect("fixture file is valid JSON");
    let cases = fixtures["cases"].as_array().expect("cases is an array");
    assert!(!cases.is_empty(), "fixture file has no cases");

    for case in cases {
        let name = case["name"].as_str().expect("case name");
        let executor = case["executor"].as_str().expect("case executor");
        let stdout = case["stdout_lines"]
            .as_array()
            .expect("stdout_lines is an array")
            .iter()
            .map(|line| line.as_str().expect("stdout line is a string"))
            .collect::<Vec<_>>()
            .join("\n");
        let adapter =
            get_adapter(executor).unwrap_or_else(|| panic!("{name}: no adapter for {executor}"));

        let native = adapter.extract_native_usage(&stdout);
        match &native.capability {
            UsageCapability::Mapped { source_version } => {
                let schema = case["schema"].as_str().expect("case schema");
                let version = case["version"].as_u64().expect("case version");
                assert_eq!(
                    *source_version,
                    format!("{schema}.v{version}"),
                    "{name}: source version"
                );
                assert_eq!(case["capability"], "mapped", "{name}: capability");
            }
            UsageCapability::Unsupported { reason } => {
                println!("{name}: unsupported, {reason}");
                assert_eq!(case["capability"], "unsupported", "{name}: capability");
            }
        }

        let expected_observations = case["expect"].as_array().expect("expect is an array");
        assert_eq!(
            native.observations.len(),
            expected_observations.len(),
            "{name}: observation count"
        );
        println!("{name}: {} observation(s)", native.observations.len());
        for (index, (observation, expected)) in native
            .observations
            .iter()
            .zip(expected_observations)
            .enumerate()
        {
            let actual = serde_json::to_value(observation).expect("observation serializes");
            println!(
                "  [{index}] input={} cost={}",
                actual["tokens"]["input"]["count"]["value"], actual["cost"]["value"]
            );
            assert_subset(&actual, expected, &format!("{name}[{index}]"));
        }
    }
}

#[test]
fn oversized_output_is_explicit_and_unparsed() {
    let oversized = "{".repeat(MAX_NATIVE_USAGE_BYTES + 1);
    let native = get_adapter("codex")
        .expect("codex adapter exists")
        .extract_native_usage(&oversized);

    assert!(matches!(native.capability, UsageCapability::Mapped { .. }));
    assert_eq!(native.observations.len(), 1);
    let actual = serde_json::to_value(&native.observations[0]).expect("observation serializes");
    println!(
        "oversized input = {}",
        actual["tokens"]["input"]["count"]["value"]
    );
    assert_eq!(
        actual["tokens"]["input"]["count"]["value"]["state"],
        "unavailable"
    );
    let reason = actual["tokens"]["input"]["count"]["value"]["reason"]
        .as_str()
        .expect("reason is text");
    assert!(reason.contains("exceeds"), "reason was {reason}");
}

#[test]
fn real_shape_agy_headless_usage() {
    let adapter = get_adapter("agy").expect("agy adapter exists");

    // Case 1: Real-shape agy terminal object with matching total
    let real_output = r#"{"conversation_id":"176f1141-b606-47aa-a47b-76f2a6147623","status":"SUCCESS","response":"ok","usage":{"input_tokens":112281,"output_tokens":14350,"thinking_tokens":9131,"cache_read_tokens":2082177,"total_tokens":126631}}"#;
    let native = adapter.extract_native_usage(real_output);

    match &native.capability {
        UsageCapability::Mapped { source_version } => {
            assert_eq!(*source_version, "agy.headless.usage.v1");
        }
        UsageCapability::Unsupported { reason } => panic!("unexpected unsupported: {reason}"),
    }

    assert_eq!(native.observations.len(), 1);
    let obs = &native.observations[0];

    // Input: 112281 as additive delta counter (exclusive relation)
    match &obs.tokens.input.count.value {
        Availability::Reported { value } => assert_eq!(value.get(), 112281),
        other => panic!("expected Reported {{ value: 112281 }}, got {other:?}"),
    }
    assert_eq!(obs.tokens.input.count.semantics, CounterSemantics::Delta);
    assert_eq!(obs.tokens.input.relation, CounterRelation::Exclusive);

    // Output: 14350 as additive delta counter (exclusive relation)
    match &obs.tokens.output.count.value {
        Availability::Reported { value } => assert_eq!(value.get(), 14350),
        other => panic!("expected Reported {{ value: 14350 }}, got {other:?}"),
    }
    assert_eq!(obs.tokens.output.count.semantics, CounterSemantics::Delta);
    assert_eq!(obs.tokens.output.relation, CounterRelation::Exclusive);

    // Cached input: 2082177 reported with unknown semantics & relation
    match &obs.tokens.cached_input.count.value {
        Availability::Reported { value } => assert_eq!(value.get(), 2082177),
        other => panic!("expected Reported {{ value: 2082177 }}, got {other:?}"),
    }
    assert_eq!(
        obs.tokens.cached_input.count.semantics,
        CounterSemantics::Unknown
    );
    assert_eq!(obs.tokens.cached_input.relation, CounterRelation::Unknown);

    // Reasoning: 9131 reported with unknown semantics & relation
    match &obs.tokens.reasoning.count.value {
        Availability::Reported { value } => assert_eq!(value.get(), 9131),
        other => panic!("expected Reported {{ value: 9131 }}, got {other:?}"),
    }
    assert_eq!(
        obs.tokens.reasoning.count.semantics,
        CounterSemantics::Unknown
    );
    assert_eq!(obs.tokens.reasoning.relation, CounterRelation::Unknown);

    // Case 2: Mismatched total marks input and output semantics unknown
    let mismatched_output = r#"{"conversation_id":"176f1141-b606-47aa-a47b-76f2a6147623","status":"SUCCESS","usage":{"input_tokens":40,"output_tokens":8,"thinking_tokens":3,"cache_read_tokens":16,"total_tokens":50}}"#;
    let native_mismatched = adapter.extract_native_usage(mismatched_output);
    assert_eq!(native_mismatched.observations.len(), 1);
    let obs_mismatched = &native_mismatched.observations[0];

    assert_eq!(
        obs_mismatched.tokens.input.count.semantics,
        CounterSemantics::Unknown
    );
    assert_eq!(
        obs_mismatched.tokens.output.count.semantics,
        CounterSemantics::Unknown
    );

    // Case 3: Object without usage block stays unsupported with its reason
    let no_usage_output = r#"{"conversation_id":"176f1141-b606-47aa-a47b-76f2a6147623","status":"SUCCESS","response":"ok"}"#;
    let native_no_usage = adapter.extract_native_usage(no_usage_output);
    match &native_no_usage.capability {
        UsageCapability::Unsupported { reason } => {
            assert!(reason.contains("usage block"), "reason was: {reason}");
        }
        UsageCapability::Mapped { .. } => panic!("expected unsupported for missing usage block"),
    }
    assert!(native_no_usage.observations.is_empty());
}
