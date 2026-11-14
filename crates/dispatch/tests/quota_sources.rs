//! Quota evidence from each runtime, replayed from versioned fixtures.
//!
//! The fixtures keep each native scope and each missing-data state. The control transcript
//! checks the frames that the Codex probe sends. The test sends nothing and starts no process.

use dispatch::executors::{codex, get_adapter};
use dispatch::quota::{
    Acquisition, BillingMode, BucketId, Freshness, ProviderId, QuotaIdentity, QuotaObservation,
    QuotaRuntime, UtilizationDirection, UtilizationScale, WindowKind,
};
use dispatch::usage::Availability;
use serde_json::Value;
use std::process::Command;

const COLLECTED_AT_MS: u64 = 1_781_000_005_000;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/quota_sources.json")).unwrap()
}

fn codex_identity() -> QuotaIdentity {
    QuotaIdentity::unverified(
        QuotaRuntime::Codex,
        ProviderId::new("openai").unwrap(),
        BillingMode::Subscription,
    )
}

fn only(observations: &[QuotaObservation]) -> &QuotaObservation {
    assert_eq!(observations.len(), 1, "expected one observation");
    &observations[0]
}

fn is_unsupported(observation: &QuotaObservation) -> bool {
    matches!(observation.acquisition, Acquisition::Unsupported { .. })
}

#[test]
fn codex_weekly_only_bucket_keeps_its_window_and_invents_no_five_hour_window() {
    let fixture = fixture();
    let result = &fixture["codex"]["rate_limits_weekly_only"];
    let observations = codex::normalize_rate_limits(result, codex_identity(), COLLECTED_AT_MS, 42);
    let observation = only(&observations);

    assert_eq!(observation.bucket, BucketId::new("codex").unwrap());
    assert_eq!(observation.acquisition, Acquisition::Queried);
    assert_eq!(observation.freshness, Freshness::Unknown);
    assert_eq!(observation.observed_at_unix_ms, None);
    let Availability::Reported { value: window } = &observation.window else {
        panic!(
            "weekly window must be reported, got {:?}",
            observation.window
        );
    };
    assert_eq!(window.kind, WindowKind::Weekly);
    assert_eq!(window.duration_minutes, Some(10_080));
    assert_eq!(window.resets_at_unix_ms, Some(1_790_000_000_000));
    let Availability::Reported { value: utilization } = &observation.utilization else {
        panic!(
            "percent must be reported, got {:?}",
            observation.utilization
        );
    };
    assert_eq!(utilization.value().to_string(), "37");
    assert_eq!(utilization.scale(), UtilizationScale::Percent);
    assert_eq!(utilization.direction(), UtilizationDirection::Used);
    assert!(matches!(observation.used, Availability::Unsupported { .. }));
    assert!(observations
        .iter()
        .all(|observation| !matches!(&observation.window, Availability::Reported { value } if value.kind == WindowKind::FiveHour)));
}

#[test]
fn codex_bucket_without_window_reports_the_missing_window() {
    let fixture = fixture();
    let result = &fixture["codex"]["rate_limits_without_window"];
    let observations = codex::normalize_rate_limits(result, codex_identity(), COLLECTED_AT_MS, 42);
    let observation = only(&observations);

    assert!(matches!(
        observation.window,
        Availability::Unavailable { .. }
    ));
    assert!(matches!(
        observation.utilization,
        Availability::Unavailable { .. }
    ));
}

#[test]
fn codex_control_transcript_is_initialize_read_and_shutdown_only() {
    let fixture = fixture();
    let expected = fixture["codex"]["control_transcript"]["frames"]
        .as_array()
        .unwrap();
    let calls = codex::control_calls();

    assert_eq!(calls.len(), expected.len());
    for (call, frame) in calls.iter().zip(expected) {
        assert_eq!(call.method, frame["method"].as_str().unwrap());
        assert_eq!(call.id, frame["id"].as_u64());
    }
    let forbidden = [
        "prompt", "turn", "thread", "login", "purchase", "config", "reset", "send", "usage",
        "stats",
    ];
    for call in &calls {
        for word in forbidden {
            assert!(
                !call.method.contains(word),
                "control transcript must not contain {word}: {}",
                call.method
            );
        }
    }
}

#[test]
fn claude_passive_rate_limit_event_keeps_fraction_and_unknown_reset() {
    let fixture = fixture();
    let stdout = fixture["claude"]["passive_stdout"].as_str().unwrap();
    let adapter = get_adapter("claude").unwrap();
    let observations = adapter.extract_passive_quota(stdout, COLLECTED_AT_MS);
    let observation = only(&observations);

    assert_eq!(observation.acquisition, Acquisition::Passive);
    let Availability::Reported { value: window } = &observation.window else {
        panic!(
            "five-hour window must be reported, got {:?}",
            observation.window
        );
    };
    assert_eq!(window.kind, WindowKind::FiveHour);
    assert_eq!(window.resets_at_unix_ms, None);
    let Availability::Reported { value: utilization } = &observation.utilization else {
        panic!(
            "fraction must be reported, got {:?}",
            observation.utilization
        );
    };
    assert_eq!(utilization.value().to_string(), "0.42");
    assert_eq!(utilization.scale(), UtilizationScale::Fraction);
    assert_eq!(utilization.direction(), UtilizationDirection::Used);
}

#[test]
fn claude_output_without_event_is_an_explicit_gap() {
    let fixture = fixture();
    let stdout = fixture["claude"]["passive_stdout_without_event"]
        .as_str()
        .unwrap();
    let adapter = get_adapter("claude").unwrap();
    let observations = adapter.extract_passive_quota(stdout, COLLECTED_AT_MS);

    assert!(is_unsupported(only(&observations)));
}

#[test]
fn antigravity_remaining_fraction_keeps_its_direction_and_unknown_reset() {
    let fixture = fixture();
    let statusline = fixture["antigravity"]["statusline"].to_string();
    let adapter = get_adapter("agy").unwrap();
    let observations = adapter.extract_passive_quota(&statusline, COLLECTED_AT_MS);
    let observation = only(&observations);

    assert_eq!(observation.bucket, BucketId::new("gemini-weekly").unwrap());
    assert_eq!(observation.identity.billing, BillingMode::Unknown);
    let Availability::Reported { value: utilization } = &observation.utilization else {
        panic!(
            "remaining fraction must be reported, got {:?}",
            observation.utilization
        );
    };
    assert_eq!(utilization.value().to_string(), "0.9378");
    assert_eq!(utilization.scale(), UtilizationScale::Fraction);
    assert_eq!(utilization.direction(), UtilizationDirection::Remaining);
    assert!(matches!(
        observation.window,
        Availability::Unsupported { .. }
    ));
}

#[test]
fn copilot_quota_without_a_verified_transport_is_an_explicit_gap() {
    let fixture = fixture();
    let adapter = get_adapter("copilot").unwrap();
    let passive = adapter.extract_passive_quota(
        fixture["copilot"]["passive_stdout"].as_str().unwrap(),
        COLLECTED_AT_MS,
    );
    let queried = adapter.query_quota(Command::new("copilot-not-launched"), COLLECTED_AT_MS);

    assert!(is_unsupported(only(&passive)));
    let observation = only(&queried);
    assert!(is_unsupported(observation));
    assert_eq!(observation.identity.runtime, QuotaRuntime::Copilot);
    assert_eq!(observation.freshness, Freshness::Unknown);
}

#[test]
fn opencode_has_no_quota_source_and_names_its_provider_scope() {
    let fixture = fixture();
    let adapter = get_adapter("opencode").unwrap();
    let passive = adapter.extract_passive_quota(
        fixture["opencode"]["passive_stdout"].as_str().unwrap(),
        COLLECTED_AT_MS,
    );
    let queried = adapter.query_quota(Command::new("opencode-not-launched"), COLLECTED_AT_MS);

    assert!(is_unsupported(only(&passive)));
    let observation = only(&queried);
    assert!(is_unsupported(observation));
    assert_eq!(observation.identity.runtime, QuotaRuntime::OpenCode);
    assert_eq!(observation.identity.provider.as_str(), "opencode");
}
