---
name: plan-design-review
description: "Senior designer audit of an active plan before implementation. Rates 7 UX dimensions 0–10, fixes obvious gaps directly, and asks about genuine design choices. Run before implementation starts."
---

# /plan-design-review

Review the active plan from a design perspective before implementation begins.

## Role

Senior designer. Your job is to make sure the plan accounts for all UX states, user journeys, and design decisions the engineers will encounter — before they encounter them.

## When to Use

- After `/office-hours` or `/plan-eng-review`
- Before implementation starts
- As part of `/autoplan`

## Step 1 — Read Plan

Read the active plan file (identified from `.dev/state.md`). Also read `DESIGN.md` at the repo root if it exists.

Also read `## Open Questions` in the execution work file (`.prompt.md`) — note any existing design-related questions that are still open.

## Step 2 — Seven-Pass Audit

Rate each dimension 0–10. For dimensions scoring below 7: fix the plan directly if the fix is obvious. Use `AskUserQuestion` if the fix requires a genuine design choice.

### Pass 1: Information Architecture (0–10)

- Is the information hierarchy clear?
- Are related things grouped together?
- Is the navigation model implicit or explicit in the requirements?

### Pass 2: Interaction State Coverage (0–10)

For each feature in the plan, check that **all 5 states** are accounted for:

| State | Description |
| --- | --- |
| Empty state | Nothing to show yet |
| Loading state | Data is being fetched |
| Error state | Something went wrong |
| Partial state | Some data, not all ready |
| Populated state | Full content rendered |

Minimum target: 4 features × 5 states = 20 interaction states. Flag any missing ones explicitly.

### Pass 3: User Journey (0–10)

- Is there a clear path from entry point to goal completion?
- Are there dead ends or ambiguous transitions?
- Are error recovery paths modeled?

### Pass 4: AI Slop Risk (0–10)

- Does the plan rely on generic AI-generated copy or layout?
- Are the interactions specific to this product, or could they apply to any app?
- Flag any requirements that will default to AI-pattern implementations without intentional design

### Pass 5: Design System Alignment (0–10)

Check against `DESIGN.md` if it exists:

- Are requested components consistent with the established design system?
- Are color, typography, and spacing references consistent?
- If no `DESIGN.md` exists: note this as a risk and recommend running `/design-consultation` before implementation

### Pass 6: Responsive / Accessibility (0–10)

- Are mobile breakpoints or responsive requirements specified?
- Is accessibility (keyboard navigation, screen reader support, color contrast) mentioned?
- Flag missing requirements

### Pass 7: Unresolved Design Decisions (0–10)

- Enumerate open design decisions that engineers will encounter but the plan does not answer
- Check `## Open Questions` for any design-related items already listed — confirm they are still unresolved or note if they were addressed
- For each unresolved item: make a recommended default decision, or surface it as `AskUserQuestion`
- Items that remain unresolved after this pass will be written to `## Open Questions` in Step 3

## Step 3 — Write Back to Plan

In the active plan file (`.prompt.md`), make two updates:

**1. Append under `## Review Results`:**

```markdown
### Design Review

**Date:** <today>

| Dimension | Score | Notes |
| --- | --- | --- |
| Information Architecture | N/10 | ... |
| Interaction State Coverage | N/10 | ... |
| User Journey | N/10 | ... |
| AI Slop Risk | N/10 | ... |
| Design System Alignment | N/10 | ... |
| Responsive / Accessibility | N/10 | ... |
| Unresolved Decisions | N/10 | ... |

#### Changes Made to Plan

<List what was added or clarified directly in the plan.>

#### Open Design Decisions

<List what was surfaced but not yet resolved, with recommended defaults — also written to ## Open Questions below.>

<!-- DESIGN_REVIEW: CLEAR -->
```

If any dimension scores below 5 and the issue was not resolved in the plan: write `<!-- DESIGN_REVIEW: NEEDS-WORK -->` instead.

**2. Update `## Open Questions`:**

For each design decision that was NOT resolved during this review, append to the `## Open Questions` section:

```markdown
- [ ] OQ-NNN — <description> *(raised by: plan-design-review)*
```

Do NOT close existing OQ items — only `/plan-eng-review` may mark an OQ as resolved.

Tell the user: dimension scores, what was fixed, what remains open. Suggested next step: `/plan-eng-review` (if not yet done), or `/autoplan` to run all three reviews.
