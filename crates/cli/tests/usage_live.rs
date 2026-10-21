//! Bounded real-source acceptance for usage collection.
//!
//! The harness runs only when the operator names an explicit run and binding file
//! through `GAL_USAGE_LIVE_RUN` and `GAL_USAGE_LIVE_BINDING`. It never discovers
//! sessions, never signs in to a provider, and never sends a prompt. Without both
//! variables it prints `NotRun`. It is ignored by default, so it runs only with
//! `--ignored`, through the live acceptance rows of the plan test matrix.

use std::path::Path;
use std::process::{Command, Output};

const RUN_VARIABLE: &str = "GAL_USAGE_LIVE_RUN";
const BINDING_VARIABLE: &str = "GAL_USAGE_LIVE_BINDING";

fn gal(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .args(args)
        .output()
        .expect("gal runs")
}

#[test]
#[ignore = "live: needs an explicit run and binding file; run through the live acceptance rows of the plan test matrix"]
fn collection_reconciles_an_explicit_binding() {
    let (Some(run), Some(binding)) = (
        std::env::var_os(RUN_VARIABLE),
        std::env::var_os(BINDING_VARIABLE),
    ) else {
        println!(
            "NotRun: set {RUN_VARIABLE} and {BINDING_VARIABLE} to an explicit run and binding file"
        );
        return;
    };
    let run = run.to_string_lossy().into_owned();
    let binding = Path::new(&binding).display().to_string();

    let collect = gal(&["usage", "collect", "--run", &run, "--binding", &binding]);
    let collect_text = String::from_utf8_lossy(&collect.stdout);
    println!("{collect_text}");
    assert!(
        collect.status.success(),
        "collection succeeds: {}",
        String::from_utf8_lossy(&collect.stderr)
    );

    let report = gal(&["usage", "report", "--run", &run]);
    let report_text = String::from_utf8_lossy(&report.stdout);
    println!("{report_text}");
    assert!(
        report.status.success(),
        "report succeeds after collection: {}",
        String::from_utf8_lossy(&report.stderr)
    );
    assert!(
        !report_text.trim().is_empty(),
        "report lists the collected run"
    );
}

#[test]
#[ignore = "live: quota lane. Runs only through the live quota acceptance row of the plan test matrix with an explicitly bound existing session"]
fn quota_lane_records_native_source_evidence() {
    let (Some(run), Some(binding)) = (
        std::env::var_os(RUN_VARIABLE),
        std::env::var_os(BINDING_VARIABLE),
    ) else {
        println!(
            "NotRun: set {RUN_VARIABLE} and {BINDING_VARIABLE} to an explicit run and binding file"
        );
        return;
    };
    let run = run.to_string_lossy().into_owned();
    let binding = Path::new(&binding).display().to_string();

    let collect = gal(&["usage", "collect", "--run", &run, "--binding", &binding]);
    let collect_text = String::from_utf8_lossy(&collect.stdout).into_owned();
    println!("{collect_text}");
    assert!(
        collect.status.success(),
        "collection succeeds: {}",
        String::from_utf8_lossy(&collect.stderr)
    );

    let quota: Vec<&str> = collect_text
        .lines()
        .skip_while(|line| !line.starts_with("quota: "))
        .take(2)
        .collect();
    let Some(state) = quota.first() else {
        panic!("collection output names no quota line");
    };
    if !state.starts_with("quota: reported,") {
        println!("NotRun: no verified native quota source for this binding. {state}");
        return;
    }
    let evidence = quota.get(1).copied().unwrap_or_default();
    for field in [
        "runtime version ",
        "identity Verified",
        "observed ",
        "collected ",
        "acquisition ",
        "freshness ",
        "control query ",
    ] {
        assert!(
            evidence.contains(field),
            "quota evidence names `{field}`: {evidence}"
        );
    }
}
