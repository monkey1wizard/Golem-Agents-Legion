---
name: plan-eng-review
description: "Engineering review of an active plan. Forces architecture, data flow, and test matrix into the open. The only required gate before /ship. Writes ## Eng Review and ## Test Plan to the active plan."
---

# /plan-eng-review

Review the active plan from an engineering perspective. Make the architecture real and the test plan explicit before any code is written.

## Role

Technical lead / engineering manager. Your job is to make the plan buildable — not to generate more ideas. Force every important architecture decision into the open now.

## When to Use

- After `/office-hours` or `/plan-ceo-review`
- This is the **only required gate** before `/ship` — do not skip it
- As part of `/autoplan`

## Step 1 — Read Plan

Read the entire active plan file (identified from `.dev/state.md`). Note: goal, scope, requirements, steps.

Also read `## Open Questions` in the execution work file (`.prompt.md`) — note which questions are still open and which may be resolved by this review.

## Step 2 — Question Architecture

For each major feature or requirement, ask: "How does this actually work?" Force answers on:

- **System boundaries** — What are the components? Where do they communicate?
- **Data flow** — Where does data enter? Where is it stored? Where is it read back? What fails at each step?
- **State machines** — For each stateful entity, enumerate all states and valid transitions
- **Error paths** — What happens when the external call fails? When the DB is unavailable? When input is invalid?
- **Trust boundaries** — What data from users or external services is trusted? What is sanitized?
- **Security sketch** — Where are the auth checks? What is the minimal attack surface?

Use `AskUserQuestion` for architecture decisions where the plan is silent and you cannot infer the right answer.

## Step 3 — Produce Architecture Artifacts

Generate these inline in the review. Even rough versions are mandatory.

**ASCII Data Flow Diagram:**

```
[User] → [API Layer] → [Service Layer] → [DB]
                            ↓
                       [Queue] → [Worker]
```

**State Machine Table** (for each stateful entity):

| State | Trigger | Next State | Side Effect |
| --- | --- | --- | --- |
| PENDING | payment_received | ACTIVE | send_welcome_email |

**Failure Mode Table:**

| Failure | User Impact | Mitigation |
| --- | --- | --- |
| DB unavailable | Cannot log in | Return 503, retry with backoff |

## Step 4 — Build Test Matrix

For each requirement, define the minimum test coverage needed:

| Requirement | Test Type | What to Verify | Priority |
| --- | --- | --- | --- |
| User can log in | Integration | Happy path + wrong password | P0 |

Test types: Unit / Integration / E2E / Manual

Priority levels:
- **P0** — Must pass before `/ship`
- **P1** — Should pass before `/ship`
- **P2** — Optional for MVP

## Step 5 — Write Back to Plan

**Append `### Eng Review` under `## Review Results`:**

```markdown
### Eng Review

**Date:** <today>

#### Architecture

<Data flow diagram, system boundaries, trust boundaries — produced in Step 3.>

#### State Machines

<Any state machine tables produced.>

#### Failure Modes

<Failure mode table produced in Step 3.>

#### Open Architecture Decisions

<List any questions asked via AskUserQuestion and the agreed answers.>

<!-- ENG_REVIEW: CLEAR -->
```

**Append a new top-level `## Test Plan` section:**

```markdown
## Test Plan

<Test matrix table from Step 4.>

**Coverage targets:** P0 tests must pass before `/ship`. P1 tests should pass. P2 tests are optional for MVP.
```

**Resolve `## Open Questions`:**

For each open question (`- [ ] OQ-NNN`) that was definitively answered during this review, update the entry to:

```markdown
- [x] OQ-NNN — <description> *(raised by: X, resolved by: plan-eng-review)*
```

Do NOT close questions that remain genuinely unanswered. `/plan-eng-review` is the only command that may mark an OQ as resolved.

**Append a new top-level `## Tasks` section:**

Derive tasks from the test matrix and remaining implementation work. Each task must be verifiable.

```markdown
## Tasks

- [ ] T-001 — <task description> (Verify: <how to confirm done>)
- [ ] T-002 — <task description> (Verify: <how to confirm done>)
```

Assign T-NNN IDs sequentially starting from T-001. No task should be orphaned — each must map to at least one row in the test matrix.

Tell the user: architecture gaps found, decisions resolved, OQs closed, that `<!-- ENG_REVIEW: CLEAR -->` is now written and `## Tasks` has been initialized. Suggested next step: begin implementation, or run `/qa` after the first commit.
