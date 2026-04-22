---
name: planning
description: "GAL-native planning entry. Converts a new request into a formal source plan in docs/plans without generating execution state yet."
---

# /planning

Create or replace a formal source plan for a new feature, sprint, or initiative.

## Role

Planning lead. Your job is to turn the request into a clean, human-readable source plan that captures scope, rationale, and requirements without prematurely binding execution-state details.

## When to Use

- Starting a new feature or sprint
- When `/gal whats-next` reports no active plan
- When an informal request should become a formal source plan in `docs/plans/`
- When you want a formal source plan without starting a question-heavy discovery workflow

## Step 1 — Gather Inputs

Read `.dev/project.md` if it exists.

Read the current request and any directly referenced files. Prefer a low-interruption planning pass: infer reasonable defaults, write assumptions explicitly, and record unresolved items in `## Open Questions`.

Read `graphify-out/GRAPH_REPORT.md` if it exists. Use communities and surprising connections to judge whether the request crosses module boundaries or hides coupling that should be called out in scope.

If `graphify-out/GAL_GRAPHIFY_VERSION.txt` exists and the `graphify` CLI is available, compare the stamped version to the current `graphify --version` output. When the versions differ and `GRAPH_REPORT.md` is not newer than the stamp file, treat the graphify report as stale-by-tool-version: do not rely on it for scope judgment, and note that `/graphify .` should be rerun before the next graph-aware planning or review pass. If the report is newer than the stamp file, keep treating it as advisory context.

Escalate to a direct user question only when one of these is true:

- the decision affects security or trust boundaries
- the choice is effectively irreversible once implementation begins
- the plan depends on a genuine taste decision with no repo precedent

Do not route the user to legacy planning commands from this workflow. If later planning-stage review is needed, describe it as a review lane (business, design, engineering), not as a gstack command name.
Treat `/deep-planning` as the architect-reviewed planning pass when the plan needs structural challenge before `/plan-to-prompt`.

## Step 2 — Produce Source Plan

Write or update `docs/plans/<plan-slug>.md` using `templates/plan.md`.

The source plan is the human-readable plan document for:

- goal and rationale
- scope boundaries
- requirements
- approach
- risks and open questions
- approval state

If assumptions remain unresolved, record them in `## Open Questions` with stable `OQ-NNN` IDs marked as raised by `planning`.

Do not create `.dev/plans/<plan-slug>.prompt.md` in this command.

## Step 3 — Update Repo State

Update `.dev/state.md` `## Active Plans` so the row points at `docs/plans/<plan-slug>.md` until `/plan-to-prompt` creates the execution prompt.

## Step 4 — Handoff

Tell the user:

- which source plan file was written
- the chosen `plan-slug`
- whether the plan should go through `/deep-planning` for architect review first, or can move on to `/refining-plan` to lock the implementation contract
- whether the plan should invoke a specific domain lane directly against the source plan when business or design review is needed without an architect-led `/deep-planning` pass
- whether the next concern is architecture convergence or locking the implementation contract
