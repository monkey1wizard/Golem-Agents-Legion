# Contributing to gal

## Start Here

gal operates as a Rust workspace alongside document-driven workflow contracts. This page serves as the contributor entry point. Follow this reading order:

1. **This page** — gates, commit rules, and the workflow skeleton.
2. [`docs/architecture.md`](docs/architecture.md) — **what the system is and why**: crate DAG, repository/runtime topology, codebase ownership map, decision records.
3. [`docs/devguide.md`](docs/devguide.md) — **how to change it**: the dev inner loop, change procedures, and machine operations.

Responsibility boundaries for every document are specified in [README → Documentation](README.md#documentation). Note the audience split: [`docs/architecture.md`](docs/architecture.md) is diagram-first and readable by anyone, while [`docs/devguide.md`](docs/devguide.md) is developer-only procedure.

## Branching & Commits

- Work on a branch off `main` and open a PR. Commit directly to `main` for solo planning docs following the established repo pattern.
- Use Conventional Commits with a scope. Examples include `feat(gal-engine): ...`, `refactor(projection): ...`, `docs(...): ...`.
- Keep each commit a single logical change allowing a clean rollback.
- The pre-commit hook runs three blocking gates: the naming gate, the `.dev` commit policy, and a personal-path leak scan. Keep `gal` on `PATH` ensuring the hook can run.

## Build & Test

```text
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
gal naming-gate
gal render-adapters # regenerate repo adapters — must be idempotent (no diff on re-run)
```

All five targets must pass cleanly before a change lands.

> **Edited the source but the agents still run the old version?** Building and testing does **not** deploy to the live coding agents. After a `plugins/gal-core/` change run `gal refresh --source ./plugins/gal-core`. After a `crates/` change rebuild and copy the binary onto `PATH` first. Then run `gal refresh --source ./plugins/gal-core`. Claude requires a restart. Full inner-loop steps and dev-machine caveats are detailed at [`docs/devguide.md — Dev Inner Loop`](docs/devguide.md#dev-inner-loop-edit--see-it-live).

### Testing Conventions

- Unit tests reside in a `mod tests` block at the bottom of the source file.
- Integration tests reside in `crates/<crate>/tests/`. Avoid `install`, `setup`, `update`, or `patch` within a `crates/gal-engine/tests/*.rs` filename. These trigger Windows spawn error 740 unless the crate ships an `asInvoker` manifest.

## The GAL Workflow

gal utilizes its own workflow. Refer to [`coding.md`](plugins/gal-core/workflows/coding.md) for the full workflow contract. For non-trivial changes:

1. `/gal planning` (or `/deep-planning` for structural/protected-path changes) → a source plan in `.dev/plans/`.
2. `/refining-plan` → `## Tasks` + `## Test Plan` + engineering review.
3. `/plan-to-prompt` → an execution prompt in `.dev/plans/`.
4. `/gal pipeline` → the checking triangle drives each task: implement → orchestrator correctness gate → tester → auditor, including cross-model verification when available.
5. `/gal finalize` → whole-branch holistic review, land, and close.

**Protected paths** (`plugins/gal-core/` contracts, `crates/projection/`, `crates/gal-engine/src/render/`) require an architect-reviewed plan before implementation. The full list resides in `.dev/project.md` under `## Protected Paths`. The review gate resides in [`coding.md`](plugins/gal-core/workflows/coding.md).

## Naming

[`docs/naming.md`](docs/naming.md) serves as the enforced naming authority. Plan-task IDs and retired terms belong only under `.dev/plans/**` and `.dev/**`. The naming gate blocks them elsewhere.
