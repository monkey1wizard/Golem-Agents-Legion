# Coding Flow

The primary development workflow is command-driven, not dispatcher-state-driven.
Every AI agent follows the same work-file model: source plans live in `docs/plans/`, execution prompts live in `.dev/plans/`, repo continuity lives in `.dev/state.md`, and specialist commands write back to the active execution prompt.

Cross-model verification remains the default guardrail: planning critique, testing, and review should be done by different models whenever a separate capable model is available.

## Choosing Planning Depth

Not every change needs the same amount of planning. GAL uses command choice, not a named risk tier, to decide how much review to apply.

| Situation | Recommended Flow | Architect | Notes |
| --- | --- | --- | --- |
| Obvious local fix | Direct implement, optional `/review`, optional `/qa` | Optional consult | Use when scope and impact are already clear |
| Scoped feature or known-cause bug | `/planning` → `/plan-to-prompt` → implement → `/review` → conditional `/design-review` or `/cso` → `/qa` → `/ship` | Optional consult | Use when the source plan is straightforward and does not need architectural challenge |
| Structural, cross-cutting, or uncertain change | `/planning` → `/deep-planning` → `/plan-to-prompt` → implement → `/review` → conditional `/design-review` or `/cso` → `/qa` → `/ship` | Required in `/deep-planning` | Use when the plan touches shared structure, dependencies, public interfaces, or protected paths |

If implementation uncovers architectural uncertainty, stop and return to `/deep-planning` before continuing.

## Execution Lifecycle

GAL's coding flow is expressed as write-back command phases.

| Phase | Entry Signal | Commands | Main Files / Outputs |
| --- | --- | --- | --- |
| **Draft plan** | No active plan, or an existing plan needs reset | `/planning`, `/deep-planning`, `/plan-to-prompt` | `docs/plans/<slug>.md`, `.dev/plans/<slug>.prompt.md`, `.dev/state.md` active plan row |
| **Planning reviews** | Execution prompt exists, buildability not yet locked | Business, design, and engineering review lanes via provider or fallback golems | `## Open Questions`, `## Tasks`, `## Review Results`, `## Test Plan` |
| **Implementation** | Tasks exist and work remains | Manual execution or `/gal pipeline` | `## Status`, `## Tasks`, code changes |
| **Review-stage audits** | Implementation reached a meaningful checkpoint | `/review`, conditional `/design-review`, conditional `/cso`, `/qa`, `/qa-only` | `## Analyze`, `## Review Results`, `## Test Results` |
| **Wrap-up or ship** | Work is paused or ready to land | `/gal wrap-up`, `/ship`, `/land-and-deploy` | `### Handoff Notes`, `.dev/state.md`, `## Ship`, `## Deploy` |

The `Workflow:` field inside `## Status` is a **plan phase marker**, not a dispatcher-owned state machine. It may be useful for humans and specialist commands, but readiness is determined by the presence and contents of plan files and sections such as `## Tasks`, `## Analyze`, `## Review Results`, and `## Test Results`.

`/review` and `/design-review` both belong to the post-implementation review stage. `/review` audits correctness, completeness, and scope drift in the code changes; `/design-review` audits the running UI against `DESIGN.md`; `/cso` is the security audit for branches that touch auth, data handling, input handling, or public API surface.

## Planning Reviews

`/deep-planning` includes an architect review by default before a plan is treated as implementation-ready.

- **Architect**: default reviewer for `/deep-planning`; checks trade-offs, over-engineering, bug surface, dependency pollution, and public API risk.
- **Analyst**: add when the plan changes business rules, pricing, permissions, notifications, onboarding, eligibility, or other customer-visible logic.
- **Designer**: add when the plan changes customer-facing flows, layout, states, components, or accessibility-sensitive interactions.

Implementation-stage `REVIEWER` and `DEBUGGER` remain separate specialists. They do not replace planning review.

## Architectural Escalation Fence

Implementation must stop and return to `/deep-planning` if the work requires any of the following structural changes:

- Create or delete project files (.csproj, .sln, package.json, etc.)
- Add or remove package dependencies
- Move files across architecture layers
- Create new interfaces or abstract base classes
- Modify DI registrations or service composition
- Change public API signatures used by 2+ consumers
- Introduce new design patterns
- Modify shared/core/base classes used by 3+ consumers

These changes need an architect-reviewed plan before implementation continues.

## Session Safety Mode

`/guard` is a high-risk-only safety mode. Use it when the work touches production systems, live data, shared risky config, or any task where you want `/careful` plus a strict edit boundary from `/freeze` in one command.

For normal feature work, `/careful` is the lighter default. `/guard` should not be treated as a universal readiness step.

## Protected Paths

Each repo's `.dev/project.md` contains a `## Protected Paths` section listing architecture-critical files. Touching any protected path requires a return to `/deep-planning` before implementation continues.

## Plan Lifecycle: Transient Task Memory

Plans are temporary work files, not permanent records. `docs/plans/` is a staging area.

### Lifecycle

1. **`/planning` creates the source plan** → `docs/plans/<type>-<slug>.md`
2. **`/deep-planning` refines the source plan when needed** → keeps scope and rationale review-ready
3. **`/plan-to-prompt` creates the execution prompt** → `.dev/plans/<type>-<slug>.prompt.md`
4. **The execution prompt self-tracks progress** → `## Status` carries phase markers, step, deviations, and decisions
5. **Implementation updates progress** during execution (not `.dev/state.md`)
6. **Testing and review write results** → execution prompt `## Test Results`, `## Review Results`, `## Analyze`
7. **Verification confirms the goal** → extracts knowledge to `docs/`, marks the plan ready for closure
8. **Plan is deleted after lifecycle closure** → task memory returns to zero, no orphaned state

### Plan Filename Convention

```text
docs/plans/<type>-<slug>.md
.dev/plans/<type>-<slug>.prompt.md
```

Type prefixes: `feat-`, `fix-`, `refactor-`, `sec-`, `perf-`, `infra-`

### Safety Rules

- **Never delete a plan before verification and handoff are complete** — the plan is the spec; early deletion breaks review, QA, and resumption.
- **Plan is pipeline, docs/ is sink** — valuable knowledge flows from the plan into long-lived docs. The plan itself is disposable only after that extraction.

## State File Role

`.dev/state.md` is a **global index + session continuity** file, not a per-task tracker.

| Responsibility | Where |
| --- | --- |
| Per-task phase marker, step progress, deviations | **Plan file** `## Status` |
| Active plans index (which branches have active plans) | **state.md** |
| Repo-level blockers, cross-plan decisions | **state.md** |
| Session continuity (last session, stopped at, next step) | **state.md** |

## Context Handoff

Before switching worktrees or ending a session:

1. Compress key context into the plan's `## Status > ### Handoff Notes`
2. Update `.dev/state.md` Session Continuity section
3. Commit changes to the current branch

This is manually triggered — the AI does not know when you're switching context unless you record it.

## Direct Agent Invocation

| Type | Allowed | Examples |
| --- | --- | --- |
| **Consult** | Yes — read-only or scoped advice, no implied phase transition | `/gal [ask architect]`, `/gal [ask analyst]` |
| **Utility** | Yes — independent helper | `/gal [run debugger]`, `/gal [run scribe]` |
| **Pipeline** | Direct invocation is consult-only | `/gal [golem-tester]`, `/gal [golem-reviewer]` for scoped advice; full execution authority comes from `/gal pipeline` |

Consult output is advice, not a formal APPROVE or REVIEW verdict. Formal outcomes come from the specialist commands and the files and sections they update.

## Per-Phase Model Assignment

Model assignment is defined in [model-roles.md](../model-roles.md).

Key rules:

- **The planning author and architect should differ when practical** — cross-check the plan before implementation
- **The planning author and designer should differ when designer is a formal reviewer** — keep design critique independent from plan authorship
- **Implementer and tester must be different models** — independent verification
- **Reviewer should differ from implementer** — fresh perspective
- **Reviewer should also differ from tester when practical** — review should be higher-level than test generation
- **Reviewer model tier ≥ implementer model tier** — the reviewer must be at least as capable
- **Reviewer model tier ≥ tester model tier** — review should be at least as capable as test generation and usually stronger
