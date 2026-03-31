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
| `docs/plans/*.prompt.md` | Temporary per-task execution memory |

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

That is why plan files include `## Status`, `## Test Results`, `## Review Results`, and optional `## Debug Log` sections.

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
