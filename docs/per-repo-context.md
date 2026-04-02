# Per-Repo Context And Working Memory

This document explains how GAL expects target project repos to store working context and temporary task memory.

## Adopt Existing Docs First

`gal init` should default to adopting an existing repo, not treating it as blank.

That means initialization should:

1. scan README and docs that already exist
2. inspect the codebase and major config files
3. write a compressed project summary into `.dev/project.md`
4. create `.dev/state.md` as a repo-level index and continuity file

The target is not to rewrite the project's architecture docs. The target is to give agents a compact entry point into the repo.

## Canonical Per-Repo Files

| File | Role |
| --- | --- |
| `.dev/project.md` | High-density project summary and index into canonical project docs |
| `.dev/state.md` | Repo-wide active plan index, blockers, cross-plan decisions, session continuity |
| `docs/plans/<plan-slug>.md` | Human-readable source plan doc (scope, rationale, requirements, steps) |
| `docs/plans/<plan-slug>.prompt.md` | AI execution work file — mutable checklist, execution state, write-back target |

## Artifact Taxonomy

GAL artifacts fall into five layers. Each layer has a different lifecycle and naming contract.

### Layer 1 — Repo-Level Durable State

Stored in `.dev/` and at repo root.

| Artifact | Path | Lifecycle |
| --- | --- | --- |
| Project summary | `.dev/project.md` | Persistent — updated by `/gal init` |
| Repo state index | `.dev/state.md` | Persistent — updated by all workflow transitions |
| Design governance | `DESIGN.md` | Persistent — updated by `/design-consultation` |
| Operational notes | `CLAUDE.md` | Persistent — updated by `/setup-deploy` |

### Layer 2 — Source Plan Docs

Human-readable plan files that provide scope, rationale, and requirements. Created by `/office-hours` and updated by review specialists.

| Naming contract | Example |
| --- | --- |
| `docs/plans/<plan-slug>.md` | `docs/plans/auth-refresh.md` |

`plan-slug` is an en-US descriptive kebab-case identifier. It becomes the shared key for all derived artifacts.

### Layer 3 — AI Execution Work Files

`.prompt.md` is the AI execution file format. It contains per-task mutable checklist, execution steps, write-back targets, and mutable status. It is not a plan document — it is the working memory the agent operates against.

| Naming contract | Example |
| --- | --- |
| `docs/plans/<plan-slug>.prompt.md` | `docs/plans/auth-refresh.prompt.md` |

Section headings, instructions, checklist items, and execution notes in a `.prompt.md` file must be written in en-US. Non-English fragments are only permitted for literal product copy or quoted source text.

### Layer 4 — Plan-Bound Design Artifacts

Design assets scoped to a single plan. Created by `/design-shotgun` and `/design-html`.

| Artifact | Path |
| --- | --- |
| Approved mockup spec | `docs/designs/<plan-slug>/variant-approved.json` |
| Approved mockup image | `docs/designs/<plan-slug>/variant-approved.png` |
| Finalized HTML handoff | `docs/designs/<plan-slug>/handoff-final.html` |

`DESIGN.md` is **not** plan-bound. It is the repo-level design governance document. `docs/designs/<plan-slug>/` holds plan-scoped design assets only.

### Layer 5 — Durable Reports and Evidence

Report files produced by specialist commands. Named to make the plan, date, and round identifiable from the filename alone.

| Report type | Naming contract | Example |
| --- | --- | --- |
| QA report | `docs/qa-reports/YYYYMMDD-<plan-slug>.md` | `docs/qa-reports/20260401-auth-refresh.md` |
| QA report (audit only) | `docs/qa-reports/YYYYMMDD-<plan-slug>-report-only.md` | `docs/qa-reports/20260401-auth-refresh-report-only.md` |
| Design audit report | `docs/design-reports/YYYYMMDD-<plan-slug>-rNN.md` | `docs/design-reports/20260401-auth-refresh-r01.md` |
| Benchmark baseline | `docs/benchmarks/YYYYMMDD-HHmmss-<url-slug>.json` | `docs/benchmarks/20260401-143022-localhost-3000.json` |
| Canary baseline | `docs/benchmarks/canary-YYYYMMDD-HHmmss-<url-slug>.json` | `docs/benchmarks/canary-20260401-143500-myapp-com.json` |
| Research note | `docs/research/YYYYMMDD-<plan-slug>-<topic>.md` | `docs/research/20260401-auth-refresh-token-expiry.md` |
| Retro snapshot | `docs/retros/YYYYMMDD.json` | `docs/retros/20260401.json` |

**Screenshot naming:**

- Plan-bound: `docs/screenshots/<plan-slug>-NNN[-suffix].png` where `NNN` starts at `001`. Use `suffix` only when the evidence workflow requires a semantic marker (`before`, `after`, `finding-001`).
- Ad-hoc (no active plan): `docs/screenshots/<slug>-YYYYMMDD-HHmmss.png`. If no stable slug exists, use a pure timestamp.

**Benchmark vs canary:** Benchmark baselines and canary baselines use different file prefixes and are never mixed for comparison. `/benchmark` compares against the most recent `YYYYMMDD-HHmmss-<url-slug>.json` with the same `url-slug`. `/canary` compares against the most recent `canary-YYYYMMDD-HHmmss-<url-slug>.json`.

### Remote Task Artifacts (Ephemeral — Not Canonical)

Artifacts produced by the GAL remote execution plane are **not** part of the canonical
artifact taxonomy above. They are stored exclusively in `$env:TEMP\gal-worker\{taskId}\`
on the worker machine and are never committed to the repo.

| Artifact | Worker Location | Lifecycle |
| --- | --- | --- |
| `status.json` | `$TEMP\gal-worker\{taskId}\` | Ephemeral — deleted after retrieval |
| `summary.md` | `$TEMP\gal-worker\{taskId}\` | Ephemeral — deleted after retrieval |
| `worker.log` | `$TEMP\gal-worker\{taskId}\` | Ephemeral — deleted after retrieval |
| `result.patch` | Retrieved to `gal-results\{taskId}\` on Main PC | Reviewed and optionally applied; not committed |

`result.patch` may contain changes to canonical-path output files (e.g., `docs/research/`).
Those changes become canonical only after Main PC reviews and applies the patch.

The `.dev/state.md` and `docs/plans/<plan-slug>.prompt.md` are **never** updated by the
remote worker directly. State convergence always happens on Main PC in the primary feature
worktree.

## Skill Activation

Skills are activated by chat intent and runtime routing — not by a static allowlist in `.dev/project.md`.

When you invoke a specialist command (e.g. `/review`, `/design-consultation`, `/qa`), the agent loads the corresponding `SKILL.md` for that command automatically. No configuration in `project.md` is required.

`.dev/project.md` may still include a `## Active Skills` section as an optional hint for documentation purposes, but GAL no longer reads it for routing decisions and does not fail when it is absent.

The legacy `gal sync` command and the static skill allowlist it enforced are no longer part of the public workflow surface.

## Why Plans Own Per-Task Status

GAL keeps task progress in the plan file instead of in `.dev/state.md`.

This design solves two problems:

- the task spec and the task state stay together
- worktree switching does not require reconstructing task state from multiple places

That is why execution work files include `## Status`, `## Open Questions`, `## Tasks`, `## Analyze`, `## Test Results`, `## Review Results`, and optional debug sections.

## Cross-Worktree Rationale

Different worktrees are treated as separate workspaces by tools such as VS Code.
Chat history and session memory do not reliably follow you across them.

Plan-local status is the practical answer:

- the active branch carries its own task memory
- switching worktrees does not require replaying a long chat
- deleting the plan at the end also deletes obsolete task state

This is why branch-scoped state files were rejected. They create more drift and naming overhead than they solve.

## What `.dev/state.md` Still Does

`.dev/state.md` is still important, but it should not be overloaded.

It tracks:

- active plans
- repo-wide blockers
- cross-plan decisions
- session continuity such as last activity and next step

It should not become a duplicate of every plan's detailed execution log.

## `/gal wrap-up` And Handoff Notes

Before switching worktrees or ending a work session, run `/gal wrap-up`. It will:

1. compress key task context into the plan's `### Handoff Notes`
2. update `.dev/state.md` session continuity
3. prompt you to commit those changes if the task needs a clean handoff point

This keeps the repo, not the chat session, as the durable memory surface.

## Lifecycle Rule

Plans are temporary.

The intended lifecycle is:

1. create a plan
2. execute against it
3. write tests and review findings back into it
4. extract durable knowledge into permanent docs
5. delete the plan during verification

If the plan is still the only place a useful insight exists, verification is not finished yet.

## Related Sources

- [workflows/coding.md](../workflows/coding.md)
- [templates/project.md](../templates/project.md)
- [templates/state.md](../templates/state.md)
- [templates/plan.md](../templates/plan.md)
