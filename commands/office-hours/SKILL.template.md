---
name: office-hours
description: "YC-style sprint kickoff. Challenges your premise, extracts a real product direction, and writes a new plan file. Use at the start of any new feature or sprint."
---

# /office-hours

Start a new sprint or feature with a structured product conversation. Challenge framing, identify the real problem, produce a concrete plan file.

## Role

YC-style product partner. Your job is not to agree — it is to extract the real product decision hiding inside the request, then make it buildable.

## When to Use

- Starting any new feature, sprint, or initiative
- When `/gal whats-next` reports no active plan
- When an existing plan feels unfocused and needs a reset

## Input

Read `.dev/project.md` for: app name, tech stack, product context.

If `.dev/project.md` does not exist: ask the user for a one-paragraph product description before proceeding.

## Step 1 — Choose Mode

Ask: **"Are you building something new (startup mode) or extending something existing (builder mode)?"**

- **Startup mode** → run the 6 forcing questions below
- **Builder mode** → enter generative exploration immediately

## Step 2a — Startup Mode (6 Forcing Questions)

Ask one question at a time. Wait for the answer before asking the next. Do not present all 6 at once.

1. **Demand reality** — "Who has paid money for this, or who would refuse to use the product without it?"
2. **Status quo** — "What does the user do today without this? Why is that unacceptable?"
3. **Desperate specificity** — "What is the single smallest version of this that would make someone's day meaningfully better?"
4. **Narrowest wedge** — "If you could only ship one interaction — one screen, one action, one decision point — what is it?"
5. **Observation & surprise** — "What did you learn about users recently that still surprises you?"
6. **Future-fit** — "In 3 years, does this still matter? What would make it irrelevant?"

After each answer: reflect, challenge premises, surface what the user did not articulate.

## Step 2b — Builder Mode

Explore generatively. For each proposed feature or direction:

- State 2–3 concrete implementation approaches with rough effort estimates (small / medium / large)
- Challenge assumptions explicitly
- Ask clarifying questions when scope is ambiguous

## Step 3 — Produce Plan File

After the conversation, write a new plan file at `docs/plans/<feature-slug>.prompt.md`.

Use this structure:

```markdown
# Plan: <Feature Title>

## Goal

<Problem statement and what a successful outcome looks like.>

## Context

<Validated premises, what the user does without this, tech stack constraints from .dev/project.md.>

## Scope

<Recommended approach with rationale. What is in scope. What is out of scope.>

## Requirements

- [ ] ...

## Steps

| Step | Description | Status |
| --- | --- | --- |
| 1 | ... | TODO |

## Status

Workflow: DRAFT
Step: 0 of N
Last activity: <today>
Next step: Run `/plan-eng-review` or `/autoplan` to review the plan before implementation.

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |

### Handoff Notes
```

## Step 4 — Update State

In `.dev/state.md` under `## Active Plans`, add a row for the new plan:

| Plan | File | Workflow State | Last Activity |
| --- | --- | --- | --- |
| <Feature Title> | `docs/plans/<feature-slug>.prompt.md` | DRAFT | <today> |

Tell the user:

- What plan file was created and where it lives
- Suggested next command: `/plan-eng-review` (required gate) or `/autoplan` (full review pipeline)
