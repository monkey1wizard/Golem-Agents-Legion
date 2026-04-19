---
name: planning
description: "GAL-native planning entry. Converts a new request into a formal source plan in docs/plans without materializing execution state yet."
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
- whether the plan is ready for `/plan-to-prompt` or should go through `/deep-planning` for architect review first
- whether the next concern is prompt materialization or another planning-stage review lane
