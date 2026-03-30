---
name: autoplan
description: "Automated review pipeline. Chains /plan-ceo-review → /plan-design-review → /plan-eng-review with auto-decisions, surfacing only genuine taste decisions at a final approval gate."
---

# /autoplan

Run all three plan-stage reviews in sequence. Resolve obvious decisions automatically. Present only genuine taste decisions at a final approval gate.

## Role

Review autopilot. Your job is to run the full review pipeline with minimal interruption, using the 6 auto-decision principles below.

## When to Use

- After `/office-hours` creates a plan file
- When you want the full CEO + Design + Eng pipeline without running each manually
- Standard pre-implementation gate when the plan is new and no strong design opinion is needed up front

## Auto-Decision Principles

When a decision is required during any review phase, resolve it automatically using these principles in priority order:

1. **Prefer completeness** — If uncertain whether to include something, include it
2. **Match existing patterns** — If the codebase has an established pattern, follow it
3. **Choose reversible options** — If two approaches are roughly equal, pick the one easier to change later
4. **Prefer prior user choices** — If the user made a similar decision in this session or plan history, use the same choice
5. **Defer ambiguous scope** — If a requirement's value is unclear, defer it to a follow-on plan
6. **Escalate security decisions** — Any decision touching auth, data handling, or trust boundaries must be surfaced to the user immediately — not auto-resolved

## Step 1 — Read Plan

Read the active plan file (identified from `.dev/state.md`) and `.dev/project.md`.

## Step 2 — CEO Review (Auto Mode)

Follow the `/plan-ceo-review` procedure with auto-decisions enabled:

- Apply the auto-decision principles to all scope choices
- Record any choice where competing approaches were within 20% of each other as a **taste decision** for the final gate
- Write `### CEO Review` to the plan per the `/plan-ceo-review` contract

## Step 3 — Design Review (Auto Mode)

Follow the `/plan-design-review` procedure with auto-decisions enabled:

- Apply the auto-decision principles to all design choices
- Record any genuine design taste decisions (aesthetic direction, interaction pattern, component choice) for the final gate
- Write `### Design Review` to the plan per the `/plan-design-review` contract

## Step 4 — Eng Review (Auto Mode)

Follow the `/plan-eng-review` procedure with auto-decisions enabled:

- Apply the auto-decision principles to all architecture choices
- **Exception:** Do NOT auto-resolve any security-related architecture decision — escalate to the user immediately
- Write `### Eng Review` and `## Test Plan` to the plan per the `/plan-eng-review` contract

## Step 5 — Final Approval Gate

Present all collected taste decisions as a single approval gate:

```
--- AUTOPLAN REVIEW COMPLETE ---

Auto-resolved: N decisions (listed below)
Taste decisions for your input: M items

[For each taste decision:]
Decision: <what was deferred>
Options: A) ... vs B) ...
Recommendation: A — because <reason>
Accept? [yes / no / explain]
```

After the user resolves all taste decisions: update the plan file with the final choices.

## Step 6 — Confirm Completion

Tell the user:

- Which three review sections were written to the plan
- Which taste decisions were resolved and how
- That `<!-- ENG_REVIEW: CLEAR -->` has been written
- Suggested next step: begin implementation, or `/review` after the first implementation commit
