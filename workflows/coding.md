# Coding Flow

The primary development workflow is control-plane-and-agent-driven, not dispatcher-state-driven.
Within Coding Flow, the primary work-file model is: source plans live in `docs/plans/`, execution prompts live in `.dev/plans/`, repo continuity lives in `.dev/state.md`, planning-stage domain reviews write back to the source plan, execution-stage specialists write detailed phase results to the `.dev/plans/<slug>.prompt.md` execution file, and `/gal pipeline` synchronizes task completion summaries across all three durable surfaces at task closeout.
Control-plane chat, `/gal status`, `/gal whats-next`, `/gal pipeline`, and execution-stage specialists all resume from the same repo-owned execution-memory substrate: `.dev/state.md` plus the active `.dev/plans/<slug>.prompt.md`.

Cross-model verification remains the default guardrail: planning critique, testing, and review should be done by different models whenever a separate capable model is available.

## Choosing Planning Depth

Not every change needs the same amount of planning. GAL uses command choice, not a named risk tier, to decide how much review to apply.

| Situation | Recommended Flow | Architect | Notes |
| --- | --- | --- | --- |
| Obvious local fix | Direct implement, optional `golem-reviewer`, optional `golem-tester` | Optional consult | Use when scope and impact are already clear |
| Scoped feature or known-cause bug | `/planning` -> `/refining-plan` -> `/plan-to-prompt` -> implement -> `golem-reviewer` -> conditional `golem-designer` or `golem-security` -> `golem-tester` -> `golem-releaser` | Optional consult | Use when the source plan is straightforward and does not need architectural challenge |
| Structural, cross-cutting, or uncertain change | `/planning` -> `/deep-planning` -> `/refining-plan` -> `/plan-to-prompt` -> implement -> `golem-reviewer` -> conditional `golem-designer` or `golem-security` -> `golem-tester` -> `golem-releaser` | Required in `/deep-planning` | Use when the plan touches shared structure, dependencies, public interfaces, or protected paths |

If implementation uncovers architectural uncertainty, stop and return to `/deep-planning` before continuing. After planning-stage changes, rerun `/refining-plan` before regenerating the execution prompt with `/plan-to-prompt`.

## Execution Lifecycle

GAL's coding flow is expressed as write-back command phases.

| Phase | Entry Signal | Owners | Main Files / Outputs |
| --- | --- | --- | --- |
| **Draft plan** | No active plan, or an existing plan needs reset | `/planning`, `/deep-planning`, `/refining-plan`, `/plan-to-prompt` | `docs/plans/<slug>.md`, `.dev/plans/<slug>.prompt.md`, `.dev/state.md` active plan row |
| **Planning reviews** | Source plan exists, buildability not yet locked | Business, design, and engineering review lanes via collaborative tools or fallback golems | `## Open Questions`, `## Tasks`, `## Review Results`, `## Test Plan` |
| **Implementation** | Tasks exist and work remains | Manual execution or `/gal pipeline` | `## Status`, `## Tasks`, `.dev/state.md` for repo continuity, `.dev/plans/<slug>.prompt.md` for mutable execution state, code changes |
| **Review-stage audits** | Implementation reached a meaningful checkpoint | `golem-reviewer`, conditional `golem-designer`, conditional `golem-security`, `golem-tester` | `## Analyze`, `## Review Results`, `## Test Results` |
| **Wrap-up or release** | Work is paused or ready to land | `/gal wrap-up`, `golem-releaser` | `### Handoff Notes`, `.dev/state.md`, active `.dev/plans/<slug>.prompt.md`, `## Release` |

The `Workflow:` field inside `## Status` is a **plan phase marker**, not a dispatcher-owned state machine. It may be useful for humans and specialist commands, but readiness is determined by the presence and contents of plan files and sections such as `## Tasks`, `## Analyze`, `## Review Results`, and `## Test Results`. When an execution prompt exists, chat-oriented control-plane actions and specialist agents must both treat that prompt as the mutable task-memory file rather than resuming from provider-local chat memory.

### File Ownership Rules

- `docs/plans/<slug>.md` is the planning-stage source plan and human-readable task checklist. Planning commands and planning review lanes may update its full content; `/gal pipeline` may update task checkboxes and commit notes after a task passes implement, test, review, and any required security gate.
- `.dev/plans/<slug>.prompt.md` is the execution-stage work file. `## Status`, retry counters, handoff notes, task commit markers, `## Test Results`, `## Review Results`, `## Analyze`, and detailed task execution history belong here.
- `.dev/state.md` is the repo-level active-plan index and session-continuity surface. `/gal pipeline` updates it after each completed task so `/gal status` and `/gal whats-next` resume from the same place a human sees in the source plan.
- Execution-stage specialists write detailed phase results to the prompt. The pipeline orchestrator owns cross-file convergence between source plan, execution prompt, and `.dev/state.md`.
- If `/gal status` or `/gal whats-next` sees a live workflow phase without the expected durable markers in `.dev/plans/<slug>.prompt.md`, or sees source-plan and prompt task checkboxes disagree, treat that as missing execution write-back rather than as a cleanly completed phase.

`golem-reviewer` and `golem-designer` both belong to the post-implementation review stage when used in audit mode. `golem-reviewer` audits correctness, completeness, and scope drift in the code changes; `golem-designer` audits the running UI against `DESIGN.md`; `golem-security` is a code-review-level security audit over implemented changes for branches that touch auth, data handling, input handling, or public API surface.

## Planning Reviews

`/deep-planning` always activates architect review before a plan is treated as implementation-ready.

- **Architect**: always activates (mandatory) in `/deep-planning`; checks trade-offs, over-engineering, bug surface, dependency pollution, and public API risk.
- **Analyst**: auto-activates when content touches business rules, pricing, permissions, notifications, onboarding, eligibility, or other customer-visible logic.
- **Designer**: auto-activates when content touches customer-facing flows, layout, states, components, or accessibility-sensitive interactions.

Each domain lane can also be invoked directly against the source plan outside of `/deep-planning` when that specialist review is needed without an architect-led deep-planning pass.

Planning-stage security concerns still sit with architect during `/deep-planning`, especially around trust boundaries, risky interfaces, and security-sensitive design decisions. `golem-security` does not replace that planning review; it audits the implemented branch once code exists.

Implementation-stage `REVIEWER` and `DEBUGGER` remain separate specialists. They do not replace planning review.

## Collaborative Tool Preflight

Before planning, review, or specialist lanes attempt to use a collaborative tool, resolve the tool state through [docs/collaborative-tools/checking-contract.md](../docs/collaborative-tools/checking-contract.md).

| Workflow phase | Tools that may apply | Degrade behavior |
| --- | --- | --- |
| `/planning` and `/deep-planning` | graphify for structural context, gstack for optional review lanes | Continue with native planning and fallback golems |
| review-stage audit | graphify for cross-community coupling checks | Continue with standard diff-based review |
| research workflows | OpenCLI for structured external retrieval | Fall back to MCP retrieval or browser tools |

Do not treat a missing collaborative tool as a workflow error. Do not prompt for install or initialization unless the user explicitly asked for the tool-specific capability.

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

Session safety is now agent-internal discipline, not a public command family.

- `golem-debugger` owns freeze-style scope control during investigations
- destructive shell operations still require explicit caution and user clarity
- do not reintroduce session-safety slash commands as public workflow steps

## Protected Paths

Each repo's `.dev/project.md` contains a `## Protected Paths` section listing architecture-critical files. Touching any protected path requires a return to `/deep-planning` before implementation continues.

## Plan Lifecycle: Transient Task Memory

Plans are temporary work files, not permanent records. `docs/plans/` is a staging area.

### Lifecycle

1. **`/planning` creates the source plan** → `docs/plans/<type>-<slug>.md`
2. **`/deep-planning` refines the source plan when needed** → keeps scope and rationale review-ready
3. **`/refining-plan` writes the implementation contract into the source plan** → `## Tasks`, `## Test Plan`, `## Review Results > ### Engineering Review`
4. **`/plan-to-prompt` creates or refreshes the execution prompt from that reviewed source plan** → `.dev/plans/<type>-<slug>.prompt.md`
5. **The execution prompt self-tracks progress** → `## Status` carries phase markers, step, deviations, and decisions
6. **Implementation updates prompt progress** during execution while the task is in flight
7. **Testing and review write results** → execution prompt `## Test Results`, `## Review Results`, `## Analyze`
8. **Pipeline task closeout converges state** → source plan task checkbox and commit note, execution prompt status/task summary, and `.dev/state.md` session continuity all agree before the next task starts
9. **Verification confirms the goal** → extracts knowledge to `docs/`, marks the plan ready for closure
10. **Plan is deleted after lifecycle closure** → task memory returns to zero, no orphaned state

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
| Per-task phase marker, step progress, deviations | **Execution prompt** `## Status` |
| Human-readable task completion checklist | **Source plan** `## Tasks`, synchronized by `/gal pipeline` closeout |
| Active plans index (which branches have active plans) | **state.md** |
| Repo-level blockers, cross-plan decisions | **state.md** |
| Session continuity (last session, stopped at, next step) | **state.md**, refreshed by `/gal pipeline` after each task |

## Context Handoff

Before switching worktrees or ending a session:

1. Compress key context into the plan's `## Status > ### Handoff Notes`
2. Update `.dev/state.md` Session Continuity section
3. Commit changes to the current branch

Before pausing work, switching providers, or switching machines, run `/gal wrap-up` so the active `.dev/plans/<slug>.prompt.md` and `.dev/state.md` become the authoritative handoff package. Resumption must come from those repo files, not from provider-local transcript memory.

This is manually triggered — the AI does not know when you're switching context unless you record it.

## Direct Agent Invocation

| Type | Allowed | Examples |
| --- | --- | --- |
| **Consult** | Yes — read-only or scoped advice, no implied phase transition | `/gal architect`, `/gal analyst` |
| **Utility** | Yes — independent helper | `/gal debugger`, `/gal notewriter` |
| **Pipeline** | Yes | `/gal tester`, `/gal reviewer` for bounded specialist work; `/gal pipeline` remains the full chained execution path |

Examples above use the literal dispatcher-facing golem names. Consult output is advice unless the named agent's contract explicitly includes formal write-back for that specialist stage.

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
