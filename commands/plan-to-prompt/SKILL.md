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

If `.dev/plans/<plan-slug>.prompt.md` already exists, read it too before refreshing.

## Step 2 — Materialize Execution Prompt

Create or update `.dev/plans/<plan-slug>.prompt.md`.

The execution prompt must:

- use `templates/plan-prompt.md` as the output schema; the template controls section names, order, and scaffold shape
- copy the stable planning content from the source plan by semantic mapping, not by mirroring the source plan's layout
- initialize mutable workflow sections such as `## Status`, `## Tasks`, `## Analyze`, `## Review Results`, `## Test Plan`, `## Test Results`, and `### Handoff Notes`
- preserve any existing mutable progress sections when the user asks for a refresh rather than a destructive reset

Important rules:

- Do not treat the source plan's heading structure as authoritative for the execution prompt. Rebuild the prompt around the canonical template every time.
- If the source plan already contains execution-style sections because of drift, import, or a previous incorrect materialization, salvage the content into the matching canonical mutable sections instead of copying the non-canonical structure forward.
- `## Status`, `## Analyze`, `## Test Results`, `### Deviations`, and `### Handoff Notes` are mutable execution-state sections. Initialize them from the template unless an existing execution prompt is being refreshed and those sections should be preserved.
- `## Open Questions` should carry forward existing `OQ-NNN` items.
- `## Review Results`, `## Test Plan`, and `## Tasks` may carry forward existing planning-stage review content when that content already exists and belongs in the canonical matching section.
- If the user explicitly asked for an output language, honor it consistently for rewritten prose. Otherwise preserve the source-plan language for copied content rather than translating it opportunistically.

This command is a materializer only. It must not decide scope, initialize engineering tasks, produce a test matrix, or write `<!-- ENG_REVIEW: CLEAR -->`.

When refreshing an existing prompt, do not wipe completed tasks, review history, or handoff notes unless the user explicitly asks for a reset.

## Step 3 — Update Repo State

Update `.dev/state.md` `## Active Plans` so the `File` column points at `.dev/plans/<plan-slug>.prompt.md`.

## Step 4 — Handoff

Tell the user:

- which execution prompt was created or updated
- whether existing mutable sections were preserved or reset
- that the next step is usually a planning-stage review lane through the configured provider or the fallback golem for that lane
