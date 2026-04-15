---
name: plan-to-prompt
description: "Materialize .dev/plans/<plan-slug>.prompt.md from docs/plans/<plan-slug>.md using the canonical execution-prompt template."
---

# /plan-to-prompt

Create or refresh the execution prompt for a source plan.

## Role

Artifact materializer. Your job is to transform a stable source plan into the mutable execution work file that GAL and specialist commands use for stateful workflow operations.

## When to Use

- A source plan exists at `docs/plans/<plan-slug>.md`
- The active plan still points to the source plan and no execution prompt exists yet
- The source plan has changed materially and the execution prompt should be regenerated before reviews begin

## Step 1 — Read Source Plan

Read `docs/plans/<plan-slug>.md` and `templates/plan-prompt.md`.

## Step 2 — Materialize Execution Prompt

Create or update `.dev/plans/<plan-slug>.prompt.md`.

The execution prompt must:

- copy the stable planning sections from the source plan
- initialize mutable workflow sections such as `## Status`, `## Tasks`, `## Analyze`, `## Review Results`, `## Test Plan`, `## Test Results`, and `### Handoff Notes`
- preserve any existing mutable progress sections when the user asks for a refresh rather than a destructive reset

This command is a materializer only. It must not decide scope, initialize engineering tasks, produce a test matrix, or write `<!-- ENG_REVIEW: CLEAR -->`.

When refreshing an existing prompt, do not wipe completed tasks, review history, or handoff notes unless the user explicitly asks for a reset.

## Step 3 — Update Repo State

Update `.dev/state.md` `## Active Plans` so the `File` column points at `.dev/plans/<plan-slug>.prompt.md`.

## Step 4 — Handoff

Tell the user:

- which execution prompt was created or updated
- whether existing mutable sections were preserved or reset
- that the next step is usually a planning-stage review lane through the configured provider or the fallback golem for that lane