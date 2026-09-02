//! Real-binary compatibility coverage for a pre-R17 markerless prompt.

use pipeline::test_first_evidence::{
    ContractMode, EvaluationInput, EvaluationVerdict, PhaseEvidence,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{tempdir, TempDir};

const PROMPT_FIXTURE: &[u8] = include_bytes!("fixtures/test-first/markerless-bootstrap.prompt.md");
const PLAN_FIXTURE: &[u8] = include_bytes!("fixtures/test-first/markerless-bootstrap.plan.md");

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn task() -> String {
    format!("T-{}", 42)
}

struct Fixture {
    _temp: TempDir,
    _repo: PathBuf,
    prompt: PathBuf,
}

fn fixture() -> Fixture {
    let temp = tempdir().unwrap();
    let repo = temp.path().to_path_buf();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "fixture@example.com"]);
    git(&repo, &["config", "user.name", "CLI Fixture"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);

    let plans = repo.join(".dev/plans");
    fs::create_dir_all(&plans).unwrap();
    fs::write(
        repo.join(".dev/project.md"),
        "# Project\n\n## Source Documents\n\n| Document | Path |\n| --- | --- |\n| sample | doc.md |\n\n<!-- gal:authoritative-check -->\n```json\n{ \"command\": [\"echo ok\"] }\n```\n",
    )
    .unwrap();
    fs::write(repo.join("doc.md"), "# Doc\n").unwrap();
    // Mirror a real GAL repository: pipeline receipts are machine-local
    // evidence, not tracked content. Without this the goal-verification
    // receipt written below would leave the tree dirty and fail the
    // `working-tree-clean` row that full-mode finalize now emits.
    fs::write(repo.join(".gitignore"), ".dev/pipeline/\n").unwrap();
    let prompt = plans.join("markerless-bootstrap.prompt.md");
    let plan = plans.join("markerless-bootstrap.md");

    // The tracked fixture bytes carry the placeholder token `T-NN` instead of
    // a digit-shaped task ID, because a literal `T-<digits>` string in a
    // tracked file outside `.dev/**` trips `gal naming-gate` (see
    // docs/naming.md's plan-task-ID provenance rule). `pipeline-preflight`'s
    // tasks-well-formed check and the evidence evaluator both require a real
    // digit-shaped task ID in the prompt body, so the placeholder is composed
    // into a real ID here, at test run time, in an untracked temp repo —
    // never as a literal in source. The paired plan's approved-prompt-bytes
    // digest is recomputed against the substituted prompt bytes so the
    // governance chain stays exact-byte-correct for the fixture actually
    // written to disk.
    let real_task = task();
    let placeholder_prompt_bytes = PROMPT_FIXTURE;
    let placeholder_digest = digest(placeholder_prompt_bytes);
    let prompt_bytes = String::from_utf8(placeholder_prompt_bytes.to_vec())
        .unwrap()
        .replace("T-NN", &real_task)
        .into_bytes();
    let real_digest = digest(&prompt_bytes);
    let plan_text = String::from_utf8(PLAN_FIXTURE.to_vec())
        .unwrap()
        .replace("T-NN", &real_task)
        .replace(&placeholder_digest, &real_digest);
    fs::write(&prompt, &prompt_bytes).unwrap();
    fs::write(&plan, plan_text.as_bytes()).unwrap();
    fs::write(
        repo.join(".dev/state.md"),
        "## Session Continuity\n\n| markerless-bootstrap | .dev/plans/markerless-bootstrap.prompt.md | in progress |\n",
    )
    .unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "markerless fixture"]);

    Fixture {
        _temp: temp,
        _repo: repo,
        prompt,
    }
}

/// Write the goal-verification receipt the shared goal-binding evaluator
/// reads, bound to the repository's current state. `prompt_rel` must be the
/// exact string later passed to the binary, because the evaluator compares
/// the recorded path against its own argument rather than a canonical form.

#[test]
fn legacy_evaluator_rejects_test_first_only_evidence_and_accepts_legacy_phases() {
    let fx = fixture();
    let prompt = String::from_utf8(fs::read(&fx.prompt).unwrap()).unwrap();
    let phases = [
        PhaseEvidence {
            phase: "implement".into(),
            completed: true,
        },
        PhaseEvidence {
            phase: "test".into(),
            completed: true,
        },
        PhaseEvidence {
            phase: "audit".into(),
            completed: true,
        },
        PhaseEvidence {
            phase: "scaffold".into(),
            completed: true,
        },
        PhaseEvidence {
            phase: "green-rerun".into(),
            completed: true,
        },
    ];
    let evaluation = pipeline::test_first_evidence::evaluate(&EvaluationInput {
        plan_slug: "markerless-bootstrap",
        task_id: &task(),
        prompt_body: &prompt,
        contract: None,
        receipts: &[],
        output_envelopes: &Default::default(),
        disputes: &[],
        retry_count: 0,
        phases: &phases[..3],
    });
    assert_eq!(evaluation.mode, ContractMode::Legacy);
    assert_eq!(evaluation.verdict, EvaluationVerdict::Pass);

    let test_first_only = pipeline::test_first_evidence::evaluate(&EvaluationInput {
        plan_slug: "markerless-bootstrap",
        task_id: &task(),
        prompt_body: &prompt,
        contract: None,
        receipts: &[],
        output_envelopes: &Default::default(),
        disputes: &[],
        retry_count: 0,
        phases: &phases[3..],
    });
    assert_eq!(test_first_only.mode, ContractMode::Legacy);
    assert_eq!(test_first_only.verdict, EvaluationVerdict::Fail);
}
