# Coding Flow

The primary development workflow — a state machine governing how code changes move from idea to done.
Every AI agent follows this workflow. Tool-agnostic: works with Copilot, Gemini CLI, Claude Code, or any future tool.

Cross-model verification is the default guardrail: planning, testing, and review should be done by different models whenever a separate capable model is available.

## Risk Weight

Not every change needs the same process. **Risk weight** determines which states are active.

| Weight | When | States | Plan? | Architect | Designer | Analyst | Reviewer |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Trivial** | Typo fix, obvious bug, single-file edit | IMPLEMENT → TEST? → VERIFY? | No | No (consult OK) | No | No | No |
| **Standard** | Small feature, known-cause bug fix, 2–3 files | PLAN → IMPLEMENT → TEST → REVIEW → VERIFY | Lightweight | Lite (default on) | No | No | Mandatory (lite) |
| **Strategic** | New feature, arch change, high-risk, cross-cutting | PLAN → DISCUSS → APPROVE → IMPLEMENT → TEST → REVIEW → VERIFY | Full | Full (mandatory) | Full (mandatory) | Conditional | Full |

**Upgrade rule**: Any change can escalate to Strategic mid-flight if complexity exceeds expectations. Stop, create/upgrade the plan, engage architect-full.

Risk weight is about how many guardrails the task needs, not about whether the task is morally "important." Trivial minimizes ceremony; Standard adds cheap structural protection; Strategic buys explicit review gates when a wrong move would be expensive.

## State Machine

### Strategic (Full)

```text
IDLE → PLAN → DISCUSS → APPROVE → IMPLEMENT → TEST → REVIEW → VERIFY → DONE
                                       ↑                                  │
                                       └──────── (verify failed) ─────────┘
```

### Standard

```text
IDLE → PLAN → IMPLEMENT → TEST → REVIEW(lite) → VERIFY → DONE
                 ↑                                  │
                 └──────── (verify failed) ──────────┘
```

### Trivial

```text
IDLE → IMPLEMENT → DONE
```

Trivial may optionally run TEST and VERIFY, but they are not required.

## State Definitions

### IDLE

No active work. Waiting for a new task.

### PLAN

- **Entry**: New feature or complex change requiring a plan (Standard/Strategic).
- **Actions**:
  - Read `.dev/project.md` for architecture context.
  - Read `.dev/state.md` for current position and active plans.
  - Read relevant `docs/` if project.md points to them.
  - Create source plan doc `docs/plans/<type>-<slug>.md` with: Goal, Risk Weight, Review Pack, Requirements, Approach, Files, Test Cases, Risks, Success Criteria.
  - Create paired execution work file `docs/plans/<type>-<slug>.prompt.md` with the same planning content plus empty section scaffolds: `## Open Questions`, `## Tasks`, `## Analyze`, `## Status`.
- For Strategic weight, the adversarial plan review is handled by `architect-full` and `designer`, both using different models from the planner.
- **Specialist**: planner
- **Exit**: Plan file created with all sections filled.
- **Checkpoint**: HUMAN — review plan before proceeding.

### DISCUSS (Strategic only)

- **Entry**: Plan exists, needs adversarial review before implementation by a different model.
- **Actions**:
  - **Review Pack** reviews the plan (see Review Pack section below) with `architect-full` and `designer` as separate models.
  - Resolve items in Risks / Open Questions.
  - Refine approach based on feedback.
- **Specialists**: architect-full (mandatory) + designer (mandatory) + analyst (conditional) + others per task
- **Verdicts**: Each reviewer issues APPROVE / REVISE / REJECT
  - All required reviewers APPROVE → proceed to human approval
  - Any REVISE → planner updates plan, re-review
  - Any REJECT → plan needs fundamental rethinking
- **Exit**: All required reviewers issue APPROVE, all open questions resolved.
- **Checkpoint**: HUMAN — confirm questions addressed.

### APPROVE (Strategic only)

- **Entry**: Plan is complete and reviewed.
- **Actions**: Human reads the plan and decides go / no-go.
- **Exit**: Explicit human approval.
- **Checkpoint**: HUMAN — **mandatory gate, never skip**.

### IMPLEMENT

- **Entry**: Approved plan (Standard/Strategic) or direct task (Trivial).
- **Actions**:
  - Write code following the plan. Check off items as completed.
  - Reference the plan file in commit messages.
  - Update plan's `## Status` section with progress (not state.md).
  - Observe the Scope Fence for Trivial/Standard work — see below.
- **Specialist**: implementer
- **Exit**: All implementation steps in the plan are checked.
- **Checkpoint**: NONE — agent works autonomously within the approved plan.

### TEST

- **Entry**: Implementation complete.
- **Actions**:
  - Write tests based on the plan's Test Cases and public API only.
  - Prefer basic unit/integration coverage first; keep deeper judgment for REVIEW.
  - **NEVER read implementation code** (independent verification).
  - Run tests and fix failures.
  - Write summary to plan's `## Test Results` section.
- **Specialist**: tester — **must be a different model from implementer**.
- **Exit**: All test cases from the plan pass.
- **Checkpoint**: NONE — report results to human.

### REVIEW

- **Entry**: All tests pass.
- **Actions (Strategic — full)**:
  - Different model reviews for: bugs, OWASP Top 10, architecture, conventions.
  - REVIEW is a higher-level check than TEST; it should not be used as a substitute for test generation.
  - Report findings with severity: BLOCKING / WARNING / INFO.
  - Write findings to plan's `## Review Results` section.
- **Actions (Standard — lite)**:
  - Correctness + architecture only. Skip full OWASP scan.
  - Still mandatory — catches Scope Fence violations the implementer missed.
- **Specialist**: reviewer — **should be a different model from implementer**.
- **Exit**: No blocking issues, or blocking issues fixed.
- **Checkpoint**: HUMAN — if blocking issues found.

### VERIFY

- **Entry**: Review passed.
- **Actions**:
  - Run the full test suite (not just new tests).
  - Confirm no regressions.
  - Verify every plan item is implemented (goal-backward: truths → artifacts → wiring).
  - Confirm related knowledge has been extracted to `docs/`.
  - Mark plan status as ABSORBED.
  - Delete the plan file.
- **Specialist**: verifier
- **Exit**: All green, plan absorbed and deleted.
- **Checkpoint**: HUMAN — final sign-off before merge.

### DONE

- **Entry**: Human signs off.
- **Actions**:
  - Update `.dev/state.md` session continuity.
  - Clean up: close related issues, remove worktree if used.
- **Exit**: Back to IDLE.

## Review Pack (Strategic)

Strategic weight uses a fixed core review pack plus conditional specialists.
Current default policy includes `designer` in every Strategic review, even for technical tasks.

### Always included

- **Architect-full**: trade-off analysis, over-engineering, bug surface, dependency pollution, public API risk.
- **Designer**: visual direction, UX flow, accessibility, interaction clarity, and design-system consistency. If the task has no meaningful UI surface, return a no-impact verdict instead of inventing issues.

### Conditionally included

- **Analyst**: Only when the task involves business rules, pricing/billing, permission/policy, notification behavior, onboarding/funnel, eligibility/approval, or any change to customer-visible outcomes.
- **Reviewer**: When implementation is large or touches security-sensitive code.
- **Debugger**: When the task involves complex integration or known fragile areas.

### Entry to IMPLEMENT

The condition is: **all required reviewers in the pack APPROVE**. If analyst is not in the pack, analyst approval is not needed.

### When analyst is NOT needed (Strategic examples)

- Pure technical refactoring
- Infrastructure / build / CI changes
- Performance optimization
- Dependency upgrades
- Bug fixes where business semantics don't change

Strategic weight does not automatically imply analyst involvement. Strategic means technical or delivery risk; analyst is added only when the change also carries business or customer-facing meaning.

## Architect Modes

| Mode | Weight | Scope |
| --- | --- | --- |
| **Lite** | Standard (default on) | Structure risk only: cross-layer, DI/interface/public API, protected paths, obvious over-engineering |
| **Full** | Strategic (mandatory) | Complete trade-off review: all 6 dimensions (architecture fit, complexity budget, trade-offs, bug surface, performance, security) |
| **Consult** | Any | Human-initiated, no formal verdict. Ask architect for advice without entering DISCUSS. |

This split exists because architect review is most useful when it is frequent enough to catch drift but not so heavy that trivial work pays a Strategic-weight tax.

## Scope Fence (Trivial/Standard)

Trivial and Standard weight changes lack full architect review. To prevent accidental architecture damage, a dual-layer defence applies.

### Layer 1 — Implementer Scope Fence (Prevention)

The implementer's instruction set includes a **Trivial/Standard prohibited operations list**. Triggering any item requires stopping and requesting a Strategic upgrade:

- Create or delete project files (.csproj, .sln, package.json, etc.)
- Add or remove package dependencies
- Move files across architecture layers
- Create new interfaces or abstract base classes
- Modify DI registrations or service composition
- Change public API signatures used by 2+ consumers
- Introduce new design patterns
- Modify shared/core/base classes used by 3+ consumers

### Layer 2 — Standard Mandatory Reviewer (Detection)

Standard weight's reviewer is mandatory (not optional), but checks only two dimensions: **correctness + architecture**. This catches Scope Fence violations the implementer missed, without the overhead of a full OWASP scan.

## Protected Paths

Each repo's `.dev/project.md` contains a `## Protected Paths` section listing architecture-critical files. The implementer touching any protected path during Trivial/Standard work **automatically triggers a Strategic upgrade**.

## Plan Lifecycle: Transient Task Memory

Plans are temporary work files, not permanent records. `docs/plans/` is a staging area.

### Lifecycle

1. **Planner creates plan** → `docs/plans/<type>-<slug>.md` (source plan doc) + `docs/plans/<type>-<slug>.prompt.md` (execution work file, initialized with empty `## Open Questions`, `## Tasks`, `## Analyze`, `## Status` scaffolds)
2. **Plan self-tracks status** → `## Status` section carries workflow state, step, deviations, decisions
3. **Implementer updates plan status** during execution (not state.md)
4. **Tester writes results** → plan `## Test Results`
5. **Reviewer writes findings** → plan `## Review Results`
6. **Verifier confirms goal** → extracts knowledge to `docs/`, marks plan ABSORBED
7. **Plan is deleted** → task memory returns to zero, no orphaned state

### Plan Filename Convention

```text
docs/plans/<type>-<slug>.prompt.md
```

Type prefixes: `feat-`, `fix-`, `refactor-`, `sec-`, `perf-`, `infra-`

### Safety Rules

- **Never delete a plan before VERIFY → DONE** — plan is the spec; mid-deletion breaks implementer and verifier.
- **Plan is pipeline, docs/ is sink** — valuable knowledge flows from plan → docs/ during VERIFY. Plan itself is disposable after extraction.

## State File Role

`.dev/state.md` is a **global index + session continuity** file, not a per-task tracker.

| Responsibility | Where |
| --- | --- |
| Per-task workflow state, step progress, deviations | **Plan file** `## Status` |
| Active plans index (which branches have active plans) | **state.md** |
| Repo-level blockers, cross-plan decisions | **state.md** |
| Session continuity (last session, stopped at, next step) | **state.md** |

## Context Handoff

Before switching worktrees or ending a session:

1. Compress key context into plan's `## Status > ### Handoff Notes`
2. Update `state.md` Session Continuity section
3. Commit changes to current branch

This is manually triggered — the AI doesn't know when you're switching context.

## Direct Agent Invocation

| Type | Allowed | Examples |
| --- | --- | --- |
| **Consult** | Yes — read-only advice, no state change | `/gal [ask architect]`, `/gal [ask analyst]` |
| **Utility** | Yes — independent of workflow state | `/gal [run debugger]`, `/gal [run scribe]` |
| **Workflow-bound** | Yes — consult only unless already bound by current state | `/gal [golem-tester]`, `/gal [golem-reviewer]` |

Consult output is advice, not a formal APPROVE/REVIEW verdict. Formal verdicts come from the DISCUSS/REVIEW states only.
Explicitly naming a workflow specialist never overrides the workflow gates; only the dispatcher can activate it in `bound` mode for the current state.

## Per-Phase Model Assignment

Model assignment is defined in [model-roles.md](../model-roles.md).

Key rules:

- **Planner and architect must be different models** — cross-check the plan before implementation
- **Planner and designer should be different models when designer is a formal reviewer** — keep design critique independent from plan authorship
- **Implementer and tester must be different models** — independent verification
- **Reviewer should differ from implementer** — fresh perspective
- **Reviewer should also differ from tester when practical** — review should be higher-level than test generation
- **Reviewer model tier ≥ implementer model tier** — the reviewer must be at least as capable
- **Reviewer model tier ≥ tester model tier** — review should be at least as capable as test generation and usually stronger
