//! Checks that the host-subagent tier rules and spawn contract are stated the same
//! way in the English configuration guide, its zh-Hant and ja translations, and the
//! pipeline command template.
//!
//! Each document must carry every language-neutral policy marker that the dispatch
//! code uses. Tier-source markers are read from `TierSource::as_str`, so a renamed
//! code value fails this test until the documents follow.

use std::path::{Path, PathBuf};

use dispatch::model_tier::TierSource;

const POLICY_DOCS: [&str; 4] = [
    "docs/configuration.md",
    "docs/i18n/zh-Hant/configuration.zh-Hant.md",
    "docs/i18n/ja/configuration.ja.md",
    "plugins/gal-core/commands/gal-pipeline/SKILL.template.md",
];

/// Task-content inputs that no longer decide the tier. None of the policy
/// documents may name them.
const RETIRED_TIER_INPUTS: [&str; 4] = [
    "extract_task_signals",
    "has_protected_path",
    "dependency_count",
    "crate_roots",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate manifest dir has a repo root two levels up")
        .to_path_buf()
}

fn read_policy_doc(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{rel}: cannot read {}: {err}", path.display()))
}

fn assert_doc_contains(rel: &str, text: &str, marker: &str) {
    assert!(
        text.contains(marker),
        "{rel} is missing host-subagent policy marker `{marker}`"
    );
}

fn recovery_action_rows(doc: &str) -> Vec<(String, bool, bool, String)> {
    let mut in_table = false;
    doc.lines()
        .filter_map(|line| {
            let cells: Vec<_> = line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            if cells.first() == Some(&"Action") {
                in_table = true;
                return None;
            }
            if !in_table || cells.len() != 4 || cells[0].starts_with("---") {
                return None;
            }
            Some((
                cells[0].replace('`', ""),
                cells[1] == "Yes",
                cells[2] == "Yes",
                cells[3].to_owned(),
            ))
        })
        .collect()
}

#[test]
fn host_subagent_policy_docs_state_code_tier_sources() {
    let code_markers = [
        TierSource::PrerequisiteTierUnknown.as_str(),
        TierSource::FinalizeReview.as_str(),
    ];
    for rel in POLICY_DOCS {
        let text = read_policy_doc(rel);
        for marker in code_markers {
            assert_doc_contains(rel, &text, marker);
        }
    }
}

#[test]
fn host_subagent_policy_docs_state_shared_tier_markers() {
    let tier_markers = [
        "model_tier_applied: no",
        "HOST_SUBAGENT_ADVISORY:",
        "--tier deep --tier-reason <reason>",
        "inherit",
        "hostSubagentModels",
    ];
    for rel in POLICY_DOCS {
        let text = read_policy_doc(rel);
        for marker in tier_markers {
            assert_doc_contains(rel, &text, marker);
        }
    }
}

#[test]
fn host_subagent_policy_docs_state_bounded_spawn_contract() {
    let spawn_markers = ["spec_path", "15"];
    for rel in POLICY_DOCS {
        let text = read_policy_doc(rel);
        for marker in spawn_markers {
            assert_doc_contains(rel, &text, marker);
        }
    }
}

#[test]
fn host_subagent_policy_docs_omit_retired_tier_inputs() {
    for rel in POLICY_DOCS {
        let text = read_policy_doc(rel);
        for retired in RETIRED_TIER_INPUTS {
            assert!(
                !text.contains(retired),
                "{rel} names retired tier input `{retired}`"
            );
        }
    }
}

#[test]
fn host_subagent_recovery_requires_owner_approval_and_stopped_writer_evidence() {
    for rel in [
        "plugins/gal-core/commands/gal-pipeline/SKILL.template.md",
        "docs/workflows.md",
    ] {
        let doc = read_policy_doc(rel);
        for marker in [
            "stable OS mutex",
            "crash releases the mutex through the OS",
            "Ownership and artifact checks fail closed",
            "Legacy `--abandon` always refuses coordinator-owned attempts",
            "never restores or republishes artifacts",
            "validates ownership of artifacts that exist",
        ] {
            assert_doc_contains(rel, &doc, marker);
        }
        let gate = "Before `--abandon`, the owner must approve that invocation";
        let table_start = doc.find("| Action | Owner approved recovery and continuation |");
        assert!(
            doc.find(gate)
                .is_some_and(|gate_at| table_start.is_some_and(|table_at| gate_at < table_at)),
            "{rel} must state the owner gate before both recovery transitions"
        );
        assert!(
            doc[..table_start.expect("recovery table is required")]
                .contains("A receipt write observed before stopped-writer confirmation remains part of the old attempt and cannot satisfy the next attempt."),
            "{rel} must keep a late write before confirmation with the old attempt"
        );
        assert!(
            doc[..table_start.expect("recovery table is required")]
                .contains("no separate approval is required for `OPEN`"),
            "{rel} must not require a separate owner approval for the next OPEN"
        );
        assert!(
            doc[..table_start.expect("recovery table is required")]
                .contains("The same owner message may approve the recovery and continuation through the next `OPEN`"),
            "{rel} must allow one owner instruction to approve recovery and continuation"
        );
        assert!(
            doc[..table_start.expect("recovery table is required")]
                .contains("Confirm again from observable evidence before the next `OPEN` that the old subagent has stopped and cannot write to the shared receipt."),
            "{rel} must require stopped-writer evidence before the next OPEN"
        );
        let pid_warning = if rel == "docs/workflows.md" {
            "An `OPEN` PID alone does not prove that the writer stopped."
        } else {
            "A PID recorded by `OPEN` does not prove the writer is dead."
        };
        assert!(
            doc[..table_start.expect("recovery table is required")].contains(pid_warning),
            "{rel} must not treat the OPEN PID as proof that the old writer stopped"
        );

        let rows = recovery_action_rows(&doc);
        for action in ["--abandon", "next OPEN"] {
            let action_rows: Vec<_> = rows.iter().filter(|row| row.0 == action).collect();
            assert_eq!(
                action_rows.len(),
                4,
                "{rel}: {action} must define all evidence cases"
            );
            for (approval, stopped, result) in [
                (false, false, "Refused"),
                (true, false, "Refused"),
                (false, true, "Refused"),
            ] {
                assert!(
                    action_rows
                        .iter()
                        .any(|row| row.1 == approval && row.2 == stopped && row.3 == result),
                    "{rel}: {action} must be refused when approval={approval} and stopped-writer evidence={stopped}"
                );
            }
            assert!(
                action_rows
                    .iter()
                    .any(|row| row.1 && row.2 && row.3.starts_with("May proceed")),
                "{rel}: {action} may proceed only with recovery approval and stopped-writer evidence"
            );
        }
    }
}
