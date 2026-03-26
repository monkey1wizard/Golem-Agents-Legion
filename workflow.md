# Development Workflow

A state machine defining how work flows from idea to done.
Every AI agent (Copilot, Gemini CLI, Claude Code, OpenClaw) should follow this workflow.

## State Machine

```text
IDLE → PLAN → DISCUSS → APPROVE → IMPLEMENT → TEST → CROSS_REVIEW → VERIFY → DONE
                                       ↑                                  │
                                       └──────── (verify failed) ─────────┘
```

## Decision Gate: Do I Need a Plan?

```text
Change requested →
  ├─ Touches 1 file with obvious fix?
  │     └─ NO plan needed → implement directly
  ├─ Touches 2-3 files in the same layer?
  │     └─ NO plan needed (unless cross-cutting)
  └─ Any of these?
      • New feature (any size)
      • Crosses 2+ architecture layers
      • Requires new interfaces or DI registrations
      • Complex bug fix (root cause unclear)
      • Large refactor (renaming across codebase)
          └─ YES → start at PLAN state
```

If no plan is needed, skip directly to IMPLEMENT.

## State Definitions

### IDLE

- No active work. Waiting for a new task.

### PLAN

- **Entry**: New feature or complex change identified.
- **Actions**:
  - Read the project's `.dev/project.md` for architecture context.
  - Create `docs/plans/<type>-<name>.prompt.md` using the project's plan template.
  - Fill in: Goal, Requirements, Approach, Files to Modify, Test Cases, Risks.
- **Model Role**: PLANNER
- **Exit**: Plan file created with all sections filled.
- **Checkpoint**: HUMAN — review plan before proceeding.

### DISCUSS

- **Entry**: Plan exists but has open questions or ambiguities.
- **Actions**:
  - **Architect reviews the plan**: trade-off analysis, over-engineering check, bug surface scan.
  - **Analyst reviews the plan**: business logic correctness, user impact, revenue risk, market fit.
  - Resolve items in "Risks / Open Questions" section.
  - Clarify requirements with the human.
  - Refine approach based on architect + analyst feedback and discussion.
- **Model Role**: ARCHITECT (technical review) + ANALYST (business review) + PLANNER (revision if needed)
- **Architect Verdicts**: APPROVE / REVISE / REJECT (technical)
- **Analyst Verdicts**: APPROVE / REVISE / REJECT (business)
  - Both must APPROVE → proceed to human approval
  - Either issues REVISE → planner updates plan, reviewer re-reviews
  - Either issues REJECT → plan needs fundamental rethinking
- **Exit**: Both architect and analyst issue APPROVE, all open questions resolved.
- **Checkpoint**: HUMAN — confirm all questions addressed.

### APPROVE

- **Entry**: Plan is complete and reviewed.
- **Actions**: Human reads the plan and decides go / no-go.
- **Exit**: Explicit human approval (e.g., "approved" or "go").
- **Checkpoint**: HUMAN — **mandatory gate, never skip**.

### IMPLEMENT

- **Entry**: Approved plan.
- **Actions**:
  - Write code following the plan. Check off items as completed.
  - Reference the plan file in commit messages.
  - Update `.dev/state.md` with progress.
- **Model Role**: CODER
- **Exit**: All implementation checkboxes in the plan are checked.
- **Checkpoint**: NONE — agent works autonomously within the approved plan.

### TEST

- **Entry**: Implementation complete.
- **Actions**:
  - Write tests based on the plan's Test Cases section and public API.
  - **Do NOT read the implementation** when writing tests (independent verification).
  - Run tests and fix failures.
- **Model Role**: TESTER — **must be a different model from CODER**.
- **Exit**: All test cases from the plan pass.
- **Checkpoint**: NONE — but report test results to human.

### CROSS_REVIEW

- **Entry**: All tests pass.
- **Actions**:
  - Different model reviews the implementation for:
    - Bugs and logic errors
    - Security vulnerabilities (OWASP Top 10)
    - Code style and conventions (from `.dev/project.md`)
    - Architecture violations
  - Report findings with severity (blocking / warning / info).
- **Model Role**: REVIEWER — **must be a different model from CODER**.
- **Exit**: No blocking issues, or blocking issues fixed.
- **Checkpoint**: HUMAN — if blocking issues are found.

### VERIFY

- **Entry**: Cross-review passed.
- **Actions**:
  - Run the full test suite (not just new tests).
  - Confirm no regressions.
  - Verify every item in the plan is implemented.
  - Update `.dev/state.md` to reflect completion.
- **Model Role**: Any.
- **Exit**: All green.
- **Checkpoint**: HUMAN — final sign-off before merge.

### DONE

- **Entry**: Human signs off.
- **Actions**:
  - Update `.dev/state.md`: move active work to history.
  - Clean up: remove worktree if used, close related issues.
- **Exit**: Back to IDLE.

## Quick Mode (No Plan)

For simple changes that skip the plan:

```text
IDLE → IMPLEMENT → TEST → VERIFY → DONE
```

Quick mode still requires TEST and VERIFY — never skip those.

## State File: `.dev/state.md`

Every project should maintain a `.dev/state.md` that tracks:

```markdown
# Current State

## Active Work

- Plan: `docs/plans/feat-basic-mode.prompt.md`
- Phase: IMPLEMENT
- Branch: `feat/basic-mode`
- CODER model: Copilot (Claude Sonnet 4.6)
- TESTER model: Gemini CLI

## Last Session

- Date: 2026-03-24
- What was done: Implemented OrderViewModel changes, started Workspace.razor
- Next step: Finish Workspace.razor, then move to TEST phase

## Decisions Log

- 2026-03-19: Use EnsureCreatedAsync instead of Migration
- 2026-03-19: AmountReceived >= Total to enable Submit
```

Any AI agent starting a new session should read `.dev/state.md` first
to understand where the previous session left off.

## Per-Phase Model Assignment

Which model does what is defined in [model-roles.md](model-roles.md).
The key rule: **CODER and TESTER must be different models.**

This ensures independent verification — the model writing tests hasn't seen
the implementation's internal structure and can only test based on the plan spec
and public API.
