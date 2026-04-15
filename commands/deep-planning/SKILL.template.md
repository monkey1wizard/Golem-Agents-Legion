---
name: deep-planning
description: "Refines any planning-stage text artifact into a formal source plan in docs/plans."
---

# /deep-planning

Refine planning-stage material into a canonical source plan.

## Role

Planning refiner. Your job is to take rough planning artifacts and converge them into a formal, scoped, review-ready source plan.

## When to Use

- A source plan exists but needs restructuring, splitting, or convergence
- The input is a gstack plan, research memo, architecture draft, or other planning-stage text artifact
- The user wants another pass before prompt materialization or planning-stage review lanes

## Step 1 — Read Planning Inputs

Read every planning-stage document the user identifies. This may include:

- `docs/plans/*.md`
- `docs/research/*.md`
- design notes
- external or migrated planning documents

Prefer convergence over interrogation. Ask a focused question only when the plan cannot be made review-ready without resolving a security, irreversible scope, or taste decision.

## Step 2 — Converge To One Source Plan

Write or update `docs/plans/<plan-slug>.md` using `templates/plan.md`.

This command may:

- narrow an over-scoped plan
- merge supporting material into one plan
- clarify architecture or task boundaries
- rewrite requirements so later reviews can operate on stable semantics
- prepare the plan for business, design, or engineering review lanes without naming a specific provider command

Do not create or mutate `.dev/plans/<plan-slug>.prompt.md` here.

## Step 3 — Update Repo State

If `.dev/state.md` tracks the active plan, keep it pointed at the source plan until `/plan-to-prompt` runs.

## Step 4 — Handoff

Tell the user:

- what changed in the source plan
- whether the plan is now review-ready
- whether the next action is `/plan-to-prompt`, another deep-planning pass, or a review lane through the configured provider or fallback golem