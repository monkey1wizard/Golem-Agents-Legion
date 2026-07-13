# Contributing to gal

gal is a Rust workspace plus a set of document-driven workflow contracts. This
guide covers how to make changes to gal itself. For a deeper maintainer
reference, see [`devguide.md`](devguide.md).

## Repository shape

- `crates/` — the `gal` Rust binary (7 crates): `gal-foundation` (config/paths/
  mode/ledger/runtime/secret), `dispatch` → `pipeline` (workflow execution,
  incl. the SSH remote-execution lane inside `dispatch`), `projection` + `mcp`
  (cross-agent projection engine), `gal-engine` (render / CLI types / gal-self
  install / workflow doctor / translation), and `cli` (the command surface).
  See `docs/architecture.md` for the dependency DAG.
- `plugins/gal-core/` — the workflow contracts: `commands/`, `agents/`,
  `skills/`, `conventions/`, `workflows/`, `templates/`. These are the source of
  truth for the `/gal` workflow; the agent adapters are generated from them.
- `docs/` — user and maintainer documentation.
- `.dev/` — repo-owned working state (`project.md`, `state.md`, `plans/`).

Full annotated tree: [`devguide.md — Codebase & Runtime Structure`](devguide.md#codebase--runtime-structure). Crate dependency DAG: [`architecture.md`](architecture.md).

## Branching and commits

- Work on a branch off `main`; open a PR (or, for solo planning docs, commit to
  `main` per the established repo pattern).
- Use Conventional Commits with a scope, e.g. `feat(gal-engine): ...`,
  `refactor(projection): ...`, `docs(...): ...`.
- Keep each commit a single logical change with a clean rollback.
- The pre-commit hook runs three blocking gates: the naming gate, the `.dev`
  commit policy, and a personal-path leak scan. Keep `gal` on `PATH` so the
  hook can run.

## Build and test

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
gal naming-gate
gal init            # regenerate repo adapters; must be idempotent (no diff on re-run)
```

All five must be clean before a change lands.

> **Edited the source but the agents still run the old version?** Building and testing does **not** deploy to the live coding agents. After a `plugins/gal-core/` change run `gal refresh --source ./plugins/gal-core`; after a `crates/` change rebuild and copy the binary onto `PATH` first, then `gal refresh --source ./plugins/gal-core` (Claude also needs a restart). Full inner-loop steps and the dev-machine caveats: [`devguide.md — Dev Inner Loop`](devguide.md#dev-inner-loop-edit--see-it-live).

### Testing conventions

- Unit tests live in a `mod tests` at the bottom of the source file.
- Integration tests live in `crates/<crate>/tests/`. Avoid `install`, `setup`,
  `update`, or `patch` in a `crates/gal-engine/tests/*.rs` filename (Windows
  spawn error 740 unless the crate ships an `asInvoker` manifest).

## The workflow itself

gal is built using its own workflow. See [`coding.md`](../plugins/gal-core/workflows/coding.md) for the full workflow contract. For non-trivial changes:

1. `/gal planning` (or `/deep-planning` for structural/protected-path changes)
   → a source plan in `.dev/plans/`.
2. `/refining-plan` → `## Tasks` + `## Test Plan` + engineering review.
3. `/plan-to-prompt` → an execution prompt in `.dev/plans/`.
4. `/gal pipeline` → the checking triangle drives each task: implement →
   orchestrator correctness gate → tester → auditor, with cross-model
   verification when available.
5. `/gal finalize` → whole-branch holistic review, land, and close.

### Protected paths

See `.dev/project.md` `## Protected Paths` and [`coding.md`](../plugins/gal-core/workflows/coding.md) for the full list and architect-review gate.

## Naming

`docs/naming.md` is the enforced naming authority. Plan-task IDs and retired
terms belong only under `.dev/plans/**` and `.dev/**`; the naming gate blocks
them elsewhere.
