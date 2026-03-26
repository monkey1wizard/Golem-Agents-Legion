# Coding Flow

The primary development workflow — a state machine governing how code changes move from idea to done.
Every AI agent follows this workflow. Tool-agnostic: works with Copilot, Gemini CLI, Claude Code, or any future tool.

## Tier System

Not every change needs the same process. Tier determines which states are active.

| Tier | When | States | Plan? | Architect | Analyst | Reviewer |
| --- | --- | --- | --- | --- | --- | --- |
| T0 (Trivial) | Typo fix, obvious bug, single-file edit | IMPLEMENT → TEST? → VERIFY? | No | No (consult OK) | No | No |
| T1 (Standard) | Small feature, known-cause bug fix, 2-3 files | PLAN → IMPLEMENT → TEST → REVIEW → VERIFY | Lightweight | Lite (default on) | No | Mandatory (lite) |
| T2 (Strategic) | New feature, arch change, high-risk, cross-cutting | PLAN → DISCUSS → APPROVE → IMPLEMENT → TEST → REVIEW → VERIFY | Full | Full (mandatory) | Conditional | Full |

**Upgrade rule**: Any tier can upgrade to T2 mid-flight if complexity exceeds expectations. Stop, create/upgrade the plan, engage architect-full.

## State Machine

### T2 (Full)

```text
IDLE → PLAN → DISCUSS → APPROVE → IMPLEMENT → TEST → REVIEW → VERIFY → DONE
                                       ↑                                  │
                                       └──────── (verify failed) ─────────┘
```

### T1 (Standard)

```text
IDLE → PLAN → IMPLEMENT → TEST → REVIEW(lite) → VERIFY → DONE
                 ↑                                  │
                 └──────── (verify failed) ──────────┘
```

### T0 (Trivial)

```text
IDLE → IMPLEMENT → DONE
```

T0 may optionally run TEST and VERIFY, but they are not required.

## State Definitions

### IDLE

No active work. Waiting for a new task.

### PLAN

- **Entry**: New feature or complex change identified (T1/T2).
- **Actions**:
  - Read `.dev/project.md` for architecture context.
  - Read `.dev/state.md` for current position and active plans.
  - Read relevant `docs/` if project.md points to them.
  - Create `docs/plans/<type>-<slug>.prompt.md` using the plan template.
  - Fill in: Goal, Tier, Review Pack, Requirements, Approach, Files, Test Cases, Risks, Success Criteria.
- **Golem**: planner
- **Exit**: Plan file created with all sections filled.
- **Checkpoint**: HUMAN — review plan before proceeding.

### DISCUSS (T2 only)

- **Entry**: Plan exists, needs adversarial review before implementation.
- **Actions**:
  - **Review Pack** reviews the plan (see Review Pack section below).
  - Resolve items in Risks / Open Questions.
  - Refine approach based on feedback.
- **Golems**: architect-full (mandatory) + analyst (conditional) + others per task
- **Verdicts**: Each reviewer issues APPROVE / REVISE / REJECT
  - All required reviewers APPROVE → proceed to human approval
  - Any REVISE → planner updates plan, re-review
  - Any REJECT → plan needs fundamental rethinking
- **Exit**: All required reviewers issue APPROVE, all open questions resolved.
- **Checkpoint**: HUMAN — confirm questions addressed.

### APPROVE (T2 only)

- **Entry**: Plan is complete and reviewed.
- **Actions**: Human reads the plan and decides go / no-go.
- **Exit**: Explicit human approval.
- **Checkpoint**: HUMAN — **mandatory gate, never skip**.

### IMPLEMENT

- **Entry**: Approved plan (T1/T2) or direct task (T0).
- **Actions**:
  - Write code following the plan. Check off items as completed.
  - Reference the plan file in commit messages.
  - Update plan's `## Status` section with progress (not state.md).
  - Observe the Scope Fence (T0/T1) — see below.
- **Golem**: implementer
- **Exit**: All implementation steps in the plan are checked.
- **Checkpoint**: NONE — agent works autonomously within the approved plan.

### TEST

- **Entry**: Implementation complete.
- **Actions**:
  - Write tests based on the plan's Test Cases and public API only.
  - **NEVER read implementation code** (independent verification).
  - Run tests and fix failures.
  - Write summary to plan's `## Test Results` section.
- **Golem**: tester — **must be a different model from implementer**.
- **Exit**: All test cases from the plan pass.
- **Checkpoint**: NONE — report results to human.

### REVIEW

- **Entry**: All tests pass.
- **Actions (T2 — full)**:
  - Different model reviews for: bugs, OWASP Top 10, architecture, conventions.
  - Report findings with severity: BLOCKING / WARNING / INFO.
  - Write findings to plan's `## Review Results` section.
- **Actions (T1 — lite)**:
  - Correctness + architecture only. Skip full OWASP scan.
  - Still mandatory — catches Scope Fence violations the implementer missed.
- **Golem**: reviewer — **should be a different model from implementer**.
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
- **Golem**: verifier
- **Exit**: All green, plan absorbed and deleted.
- **Checkpoint**: HUMAN — final sign-off before merge.

### DONE

- **Entry**: Human signs off.
- **Actions**:
  - Update `.dev/state.md` session continuity.
  - Clean up: close related issues, remove worktree if used.
- **Exit**: Back to IDLE.

## Review Pack (T2)

T2 no longer hard-codes "architect + analyst." Instead, the review pack is composed per task.

### Always included

- **Architect-full**: trade-off analysis, over-engineering, bug surface, dependency pollution, public API risk.

### Conditionally included

- **Analyst**: Only when the task involves business rules, pricing/billing, permission/policy, notification behavior, onboarding/funnel, eligibility/approval, or any change to customer-visible outcomes.
- **Reviewer**: When implementation is large or touches security-sensitive code.
- **Debugger**: When the task involves complex integration or known fragile areas.

### Entry to IMPLEMENT

The condition is: **all required reviewers in the pack APPROVE**. If analyst is not in the pack, analyst approval is not needed.

### When analyst is NOT needed (T2 examples)

- Pure technical refactoring
- Infrastructure / build / CI changes
- Performance optimization
- Dependency upgrades
- Bug fixes where business semantics don't change

## Architect Modes

| Mode | Tier | Scope |
| --- | --- | --- |
| **Lite** | T1 (default on) | Structure risk only: cross-layer, DI/interface/public API, protected paths, obvious over-engineering |
| **Full** | T2 (mandatory) | Complete trade-off review: all 6 dimensions (architecture fit, complexity budget, trade-offs, bug surface, performance, security) |
| **Consult** | Any | Human-initiated, no formal verdict. Ask architect for advice without entering DISCUSS. |

## Scope Fence (T0/T1)

T0/T1 lack full architect review. To prevent accidental architecture damage, a dual-layer defence applies.

### Layer 1 — Implementer Scope Fence (Prevention)

The implementer's instruction set includes a **T0/T1 prohibited operations list**. Triggering any item requires stopping and requesting T2 upgrade:

- Create or delete project files (.csproj, .sln, package.json, etc.)
- Add or remove package dependencies
- Move files across architecture layers
- Create new interfaces or abstract base classes
- Modify DI registrations or service composition
- Change public API signatures used by 2+ consumers
- Introduce new design patterns
- Modify shared/core/base classes used by 3+ consumers

### Layer 2 — T1 Mandatory Reviewer (Detection)

T1's reviewer is mandatory (not optional), but checks only two dimensions: **correctness + architecture**. This catches Scope Fence violations the implementer missed, without the overhead of a full OWASP scan.

## Protected Paths

Each repo's `.dev/project.md` contains a `## Protected Paths` section listing architecture-critical files. The implementer touching any protected path during T0/T1 work **automatically triggers T2 upgrade**.

## Plan Lifecycle: Transient Task Memory

Plans are temporary work files, not permanent records. `docs/plans/` is a staging area.

### Lifecycle

1. **Planner creates plan** → `docs/plans/<type>-<slug>.prompt.md`
2. **Plan self-tracks status** → `## Status` section carries workflow state, step, deviations, decisions
3. **Implementer updates plan status** during execution (not state.md)
4. **Tester writes results** → plan `## Test Results`
5. **Reviewer writes findings** → plan `## Review Results`
6. **Verifier confirms goal** → extracts knowledge to `docs/`, marks plan ABSORBED
7. **Plan is deleted** → task memory returns to zero, no orphaned state

### Plan Filename Convention

```
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

## `gal pause`: Context Handoff

Before switching worktrees or ending a session:

1. Compress key context into plan's `## Status > ### Handoff Notes`
2. Update `state.md` Session Continuity section
3. Commit changes to current branch

This is manually triggered — the AI doesn't know when you're switching context.

## Direct Agent Invocation

| Type | Allowed | Examples |
| --- | --- | --- |
| **Consult** | Yes — read-only advice, no state change | `gal ask architect`, `gal ask analyst` |
| **Utility** | Yes — independent of workflow state | `gal run debugger`, `gal run scribe` |
| **Workflow-bound** | No — state transitions via `gal next` only | implementer, tester, reviewer, verifier |

Consult output is advice, not a formal APPROVE/REVIEW verdict. Formal verdicts come from the DISCUSS/REVIEW states only.

## Per-Phase Model Assignment

Model assignment is defined in [model-roles.md](../model-roles.md).

Key rules:

- **Implementer and tester must be different models** — independent verification
- **Reviewer should differ from implementer** — fresh perspective
- **Reviewer model tier ≥ implementer model tier** — the reviewer must be at least as capable
