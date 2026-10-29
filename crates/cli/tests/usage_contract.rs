//! Parity probes for the shared usage accounting contract.
//!
//! `plugins/gal-core/conventions/usage-accounting.md` is the one place that states the rules.
//! These probes read it, the production sources it describes, and every command or workflow
//! source that links to it. They check that the required rules are present, that the numbers,
//! command strings, runtime rows, and store path match the code, that every link resolves, and
//! that no usage or assessment wording permits a factual rewrite or a goal-verification dispatch.

use std::fs;
use std::path::{Path, PathBuf};

const CONTRACT: &str = "plugins/gal-core/conventions/usage-accounting.md";

const LINKING_SOURCES: [&str; 4] = [
    "plugins/gal-core/workflows/coding.md",
    "plugins/gal-core/commands/gal/SKILL.template.md",
    "plugins/gal-core/commands/gal-pipeline/SKILL.template.md",
    "plugins/gal-core/commands/gal-finalize/SKILL.template.md",
];

const REQUIRED_RULES: [&str; 25] = [
    "## Automatic Capture",
    "## Explicit Session Binding",
    "## Final-Turn Cutoff",
    "## Failed and Resumed Attempts",
    "## Factual Records and Assessments",
    "## Rate Bases",
    "## Account Quota",
    "### Runtime Rows",
    "### Identity Proof",
    "### Native Windows and Units",
    "### Bounded Control Reads",
    "### Freshness and Failed Refresh",
    "### Immutable Return References",
    "### Query Charge Uncertainty",
    "## Report Commands",
    "## Cleanup and Finalize",
    "## Goal Verification and Receipts",
    "## Capability Gaps",
    "Publication never replaces an existing record",
    "Identical content replays without change",
    "usage error code, 64",
    "Quota never adds to a task cost",
    "Goal verification stays in the orchestrator process",
    "A failed refresh keeps the previous values",
    "The whole probe has a deadline of five seconds",
];

const RUNTIME_ROWS: [(&str, &str); 5] = [
    ("Codex", "| Codex CLI |"),
    ("Claude", "| Claude Code |"),
    ("Copilot", "| GitHub Copilot CLI |"),
    ("Antigravity", "| Antigravity CLI |"),
    ("OpenCode", "| OpenCode |"),
];

const WRITE_VERBS: [&str; 4] = ["rewrite", "overwrite", "replace", "delete"];
const DISPATCH_PHRASES: [&str; 3] = ["dispatch goal", "dispatches goal", "dispatch the goal"];
const NEGATIONS: [&str; 5] = ["never", "not ", "no ", "cannot", "must not"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

/// Returns true when a line names a forbidden write or dispatch with no negation.
fn permits_forbidden_action(line: &str) -> bool {
    let line = line.to_lowercase();
    let forbidden = WRITE_VERBS
        .iter()
        .chain(DISPATCH_PHRASES.iter())
        .any(|phrase| line.contains(phrase));
    let negated = NEGATIONS.iter().any(|negation| line.contains(negation));
    forbidden && !negated
}

/// Lines in a document that concern usage, assessments, capture, or quota.
fn usage_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| {
            let line = line.to_lowercase();
            ["usage", "assess", "capture", "quota"]
                .iter()
                .any(|word| line.contains(word))
        })
        .collect()
}

/// Destinations of every Markdown link whose target names the contract.
fn contract_link_targets(text: &str) -> Vec<&str> {
    text.split("](")
        .skip(1)
        .filter_map(|rest| rest.split(')').next())
        .filter(|target| {
            target
                .split('#')
                .next()
                .unwrap_or("")
                .ends_with("usage-accounting.md")
        })
        .collect()
}

/// Command lines inside the contract's fenced blocks.
fn command_lines(contract: &str) -> Vec<&str> {
    let mut in_fence = false;
    let mut commands = Vec::new();
    for line in contract.lines() {
        if line.starts_with("```") {
            in_fence = !in_fence;
        } else if in_fence && line.starts_with("gal usage ") {
            commands.push(line);
        }
    }
    commands
}

#[test]
fn contract_states_every_required_rule() {
    let contract = read(CONTRACT);
    for rule in REQUIRED_RULES {
        assert!(contract.contains(rule), "contract is missing `{rule}`");
    }
}

#[test]
fn contract_limits_match_the_production_probe() {
    let quota = read("crates/dispatch/src/quota.rs");
    let contract = read(CONTRACT);
    assert!(
        quota.contains("pub const PROBE_DEADLINE: Duration = Duration::from_secs(5);"),
        "production probe deadline is no longer five seconds"
    );
    assert!(
        quota.contains("pub const PROBE_OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;"),
        "production probe output limit is no longer one MiB"
    );
    assert!(contract.contains("five seconds"));
    assert!(contract.contains("one MiB"));
    println!(
        "probe limits: deadline 5 s, output limit {} bytes, both match crates/dispatch/src/quota.rs",
        1024 * 1024
    );
}

#[test]
fn contract_commands_and_store_path_match_the_cli() {
    let usage = read("crates/cli/src/commands/usage.rs");
    let store = read("crates/pipeline/src/usage/store.rs");
    let contract = read(CONTRACT);

    assert!(
        store.contains(r#"["gal", "usage", "v1"]"#),
        "production usage store is no longer under gal/usage/v1"
    );
    assert!(contract.contains("gal/usage/v1/"));

    let commands = command_lines(&contract);
    assert_eq!(
        commands.len(),
        3,
        "expected the collect, assess, and report command lines"
    );
    for command in &commands {
        assert!(
            usage.contains(&format!("usage: {command}")),
            "production usage help does not match the contract command `{command}`"
        );
    }
    println!(
        "usage help matches {} contract commands: {commands:?}",
        commands.len()
    );
}

#[test]
fn contract_lists_the_five_quota_runtimes_in_production_order() {
    let quota = read("crates/dispatch/src/quota.rs");
    let contract = read(CONTRACT);

    let body = quota
        .split("pub enum QuotaRuntime {")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("QuotaRuntime enum is present");
    let variants: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|line| line.ends_with(','))
        .collect();
    assert_eq!(variants.len(), RUNTIME_ROWS.len(), "runtime count changed");

    for ((variant, row), production) in RUNTIME_ROWS.iter().zip(&variants) {
        assert_eq!(*production, format!("{variant},"), "runtime order changed");
        assert!(
            contract.contains(row),
            "contract has no row for {variant}: {row}"
        );
    }
    println!("runtime rows match production order: {variants:?}");
}

#[test]
fn every_linking_source_resolves_to_the_one_contract() {
    let contract_path = fs::canonicalize(repo_root().join(CONTRACT)).expect("contract exists");
    for source in LINKING_SOURCES {
        let text = read(source);
        let targets = contract_link_targets(&text);
        assert!(
            !targets.is_empty(),
            "{source} does not link the usage contract"
        );

        let source_dir = repo_root()
            .join(source)
            .parent()
            .expect("source has a directory")
            .to_path_buf();
        for target in targets {
            let path = target.split('#').next().unwrap_or(target);
            let resolved = fs::canonicalize(source_dir.join(path))
                .unwrap_or_else(|e| panic!("{source} links `{target}` that does not resolve: {e}"));
            assert_eq!(
                resolved, contract_path,
                "{source} links another copy of the rules"
            );
        }
        println!("{source}: links the one contract");
    }
}

#[test]
fn usage_wording_never_permits_a_factual_rewrite_or_goal_dispatch() {
    assert!(permits_forbidden_action(
        "An assessment may rewrite the factual table."
    ));
    assert!(permits_forbidden_action(
        "Usage capture may dispatch goal verification."
    ));
    assert!(!permits_forbidden_action(
        "An assessment never rewrites a factual snapshot."
    ));
    assert!(!permits_forbidden_action(
        "Usage capture never dispatches goal verification."
    ));

    let mut documents = vec![(CONTRACT, read(CONTRACT))];
    documents.extend(LINKING_SOURCES.iter().map(|source| (*source, read(source))));

    for (document, text) in documents {
        let lines = usage_lines(&text);
        assert!(
            !lines.is_empty(),
            "{document} has no usage accounting wording to check"
        );
        for line in lines {
            assert!(
                !permits_forbidden_action(line),
                "{document} permits a forbidden action: {line}"
            );
        }
    }
}

#[test]
fn contract_prose_avoids_semicolons() {
    let contract = read(CONTRACT);
    let mut in_fence = false;
    for (number, line) in contract.lines().enumerate() {
        if line.starts_with("```") {
            in_fence = !in_fence;
        } else if !in_fence {
            assert!(
                !line.contains(';'),
                "contract line {} contains a semicolon",
                number + 1
            );
        }
    }
}
