# Coding Flow

The primary development workflow is control-plane-and-agent-driven, not dispatcher-state-driven.
Within Coding Flow, the primary work-file model is: source plans live in `docs/plans/`, execution prompts live in `.dev/plans/`, repo continuity lives in `.dev/state.md`, planning-stage domain reviews write back to the source plan, execution-stage specialists write detailed phase results to the `.dev/plans/<slug>.prompt.md` execution file, and `/gal pipeline` synchronizes task completion summaries across all three durable surfaces at task closeout.
Control-plane chat, `/gal status`, `/gal whats-next`, `/gal pipeline`, and execution-stage specialists all resume from the same repo-owned execution-memory substrate: `.dev/state.md` plus the active `.dev/plans/<slug>.prompt.md`.

Cross-model verification remains the default guardrail: planning critique, testing, and review should be done by different models whenever a separate capable model is available.

## Choosing Planning Depth

Not every change needs the same amount of planning. GAL uses command choice, not a named risk tier, to decide how much review to apply.

| Situation | Recommended Flow | Architect | Notes |
| --- | --- | --- | --- |
| Obvious local fix | Direct implement, optional orchestrator correctness gate, optional `golem-tester`, optional `golem-auditor` | Optional consult | Use when scope and impact are already clear |
| Scoped feature or known-cause bug | `/planning` -> `/refining-plan` -> `/plan-to-prompt` -> implement -> orchestrator correctness gate -> `golem-tester` -> `golem-auditor` -> `golem-releaser` | Optional consult | Use when the source plan is straightforward and does not need architectural challenge |
| Structural, cross-cutting, or uncertain change | `/planning` -> `/deep-planning` -> `/refining-plan` -> `/plan-to-prompt` -> implement -> orchestrator correctness gate -> `golem-tester` -> `golem-auditor` -> `golem-releaser` | Required in `/deep-planning` | Use when the plan touches shared structure, dependencies, public interfaces, or protected paths |

If implementation uncovers architectural uncertainty, stop and return to `/deep-planning` before continuing. After planning-stage changes, rerun `/refining-plan` before regenerating the execution prompt with `/plan-to-prompt`.

## Execution Lifecycle

GAL's coding flow is expressed as write-back command phases.

| Phase | Entry Signal | Owners | Main Files / Outputs |
| --- | --- | --- | --- |
| **Draft plan** | No active plan, or an existing plan needs reset | `/planning`, `/deep-planning`, `/refining-plan`, `/plan-to-prompt` | `docs/plans/<slug>.md`, `.dev/plans/<slug>.prompt.md`, `.dev/state.md` active plan row |
| **Planning reviews** | Source plan exists, buildability not yet locked | Business, design, and engineering review lanes via collaborative tools or fallback golems | `## Open Questions`, `## Tasks`, `## Review Results`, `## Test Plan` |
| **Implementation** | Tasks exist and work remains | Manual execution or `/gal pipeline` | `## Status`, `## Tasks`, `.dev/state.md` for repo continuity, `.dev/plans/<slug>.prompt.md` for mutable execution state, code changes |
| **Review-stage audits** | Implementation reached a meaningful checkpoint | orchestrator correctness gate, `golem-tester`, `golem-auditor`, conditional `golem-designer` | `## Analyze`, `## Review Results`, `## Test Results` |
| **Wrap-up or release** | Work is paused or ready to land | `/gal wrap-up`, `golem-releaser` | `### Handoff Notes`, `.dev/state.md`, active `.dev/plans/<slug>.prompt.md`, `## Release` |

The `Workflow:` field inside `## Status` is a **plan phase marker**, not a dispatcher-owned state machine. It may be useful for humans and specialist commands, but readiness is determined by the presence and contents of plan files and sections such as `## Tasks`, `## Analyze`, `## Review Results`, and `## Test Results`. When an execution prompt exists, chat-oriented control-plane actions and specialist agents must both treat that prompt as the mutable task-memory file rather than resuming from provider-local chat memory.

### File Ownership Rules

- `docs/plans/<slug>.md` is the planning-stage source plan and human-readable task checklist. Planning commands and planning review lanes may update its full content; `/gal pipeline` may update task checkboxes and commit notes after a task passes implement, the orchestrator correctness gate, test, and audit.
- `.dev/plans/<slug>.prompt.md` is the execution-stage work file. `## Status`, retry counters, handoff notes, task commit markers, `## Test Results`, `## Review Results`, `## Analyze`, and detailed task execution history belong here.
- `.dev/state.md` is the repo-level active-plan index and session-continuity surface. `/gal pipeline` updates it after each completed task so `/gal status` and `/gal whats-next` resume from the same place a human sees in the source plan.
- Execution-stage specialists write detailed phase results to the prompt. The pipeline orchestrator owns cross-file convergence between source plan, execution prompt, and `.dev/state.md`.
- If `/gal status` or `/gal whats-next` sees a live workflow phase without the expected durable markers in `.dev/plans/<slug>.prompt.md`, or sees source-plan and prompt task checkboxes disagree, treat that as missing execution write-back rather than as a cleanly completed phase.

`golem-auditor` and `golem-designer` both belong to the post-implementation audit stage when used in audit mode. The orchestrator correctness gate owns correctness, completeness, scope drift, and obvious performance using the full task context before test. `golem-auditor` owns the independent deep-performance and security audit over implemented changes; `golem-designer` audits the running UI against `DESIGN.md`.

## Planning Reviews

`/deep-planning` always activates architect review before a plan is treated as implementation-ready.

- **Architect**: always activates (mandatory) in `/deep-planning`; checks trade-offs, over-engineering, bug surface, dependency pollution, and public API risk.
- **Analyst**: auto-activates when content touches business rules, pricing, permissions, notifications, onboarding, eligibility, or other customer-visible logic.
- **Designer**: auto-activates when content touches customer-facing flows, layout, states, components, or accessibility-sensitive interactions.

Each domain lane can also be invoked directly against the source plan outside of `/deep-planning` when that specialist review is needed without an architect-led deep-planning pass.

Planning-stage security concerns still sit with architect during `/deep-planning`, especially around trust boundaries, risky interfaces, and security-sensitive design decisions. `golem-auditor` does not replace that planning review; it audits the implemented branch once code exists.

Implementation-stage `AUDITOR` and `DEBUGGER` remain separate specialists. They do not replace planning review.

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

`.dev/state.md` is a **global index + per-plan session continuity** file, not a per-task tracker.

| Responsibility | Where |
| --- | --- |
| Per-task phase marker, step progress, deviations | **Execution prompt** `## Status` |
| Human-readable task completion checklist | **Source plan** `## Tasks`, synchronized by `/gal pipeline` closeout |
| Active plans index (which branches have active plans) | **state.md** |
| Repo-level blockers, cross-plan decisions | **state.md** |
| Session continuity (one row per active plan: last session, stopped at, next step, context) | **state.md**, refreshed by `/gal pipeline` after each task |

## Context Handoff

Before switching worktrees or ending a session:

1. Compress key context into the plan's `## Status > ### Handoff Notes`
2. Update the matching `.dev/state.md` `## Session Continuity` row for that plan
3. Commit changes to the current branch

Before pausing work, switching providers, or switching machines, run `/gal wrap-up` so the active `.dev/plans/<slug>.prompt.md` and `.dev/state.md` become the authoritative handoff package. Resumption must come from those repo files, not from provider-local transcript memory.

This is manually triggered — the AI does not know when you're switching context unless you record it.

## Direct Agent Invocation

| Type | Allowed | Examples |
| --- | --- | --- |
| **Consult** | Yes — read-only or scoped advice, no implied phase transition | `/gal architect`, `/gal analyst` |
| **Utility** | Yes — independent helper | `/gal debugger`, `/gal notewriter` |
| **Pipeline** | Yes | `/gal tester`, `/gal auditor` for bounded specialist work; `/gal pipeline` remains the full chained execution path |

Examples above use the literal dispatcher-facing golem names. Consult output is advice unless the named agent's contract explicitly includes formal write-back for that specialist stage.

## Model Roles and Per-Phase Assignment

Define roles by **what they do**, not by which model they are. When you switch AI tools, update `~/.gal/config/executor-routing.json` (machine-read dispatch) and the mapping section below (human reference).

Default principle: formal cross-checks should use a different model from the one that authored the work file whenever practical.

### Roles

| Role | Purpose | Key Trait |
| --- | --- | --- |
| ARCHITECT | Adversarial plan review — trade-offs, over-engineering, bugs | Critical thinking, minimalism, direct communication |
| ANALYST | Business logic review — ROI, domain correctness, user impact | Commercial awareness, domain expertise |
| DESIGNER | Review visual design, UX flow, accessibility, and design-system consistency | Experience design judgment, user empathy |
| RESEARCHER | Investigate unknowns, synthesize findings, cross-review sources, and prepare research outputs for independent verification | Evidence gathering, source attribution, synthesis |
| CODER | Write implementation code following a plan | Code generation, refactoring |
| TESTER | Write tests from plan spec + public API only | Spec-driven, usually basic unit/integration coverage |
| AUDITOR | Audit code for deep performance, security, and other high-confidence risks | Higher-level critical eye, different perspective |
| NOTEWRITER | Obsidian writes — diary, private captures, inbox processing, and knowledge extraction | Note authoring, Guide-aware fallback, private vs. durable routing |
| LOCAL | Tasks requiring privacy or local language | Runs on-device, no data leaves machine |

### Routing Rules

1. **Planning review should be a different-model check** — the model that critiques a plan should differ from the one that drafted it whenever practical.
2. **CODER and TESTER must be different models** — independent verification
3. **AUDITOR should differ from CODER** — fresh perspective catches blind spots
4. **AUDITOR should also differ from TESTER when practical** — audit is a higher-level check than test generation
5. **DESIGNER should differ from CODER when used as a formal reviewer** — keep experience review independent from implementation
6. **RESEARCHER owns research, synthesis, and cross-review** — independent reference verification must be done by a different model
7. **NOTEWRITER** handles all Obsidian writes — it should load the user's configured Guide when available and fall back to generic mode when not
8. **LOCAL** is for privacy-sensitive data or Traditional Chinese tasks
9. When switching tools, update `~/.gal/config/executor-routing.json` (machine-read) and any personal planning notes

### Planning Review Rules

`/deep-planning` is the default architect-reviewed planning pass before prompt generation.

| Reviewer | When Included | Verdict Required? |
| --- | --- | --- |
| **ARCHITECT** | Every `/deep-planning` pass | Yes — review required before `/plan-to-prompt` |
| **ANALYST** | Business rules, pricing, permissions, customer-visible logic | Yes — when included |
| **DESIGNER** | Customer-facing flows, layout, states, component systems, accessibility-sensitive work | Yes — when included |

Implementation-stage `AUDITOR` and `DEBUGGER` remain separate specialists. They do not replace planning review.

### Typical Workflow (Single Developer)

```text
1. `/planning` or `/deep-planning` → Use a frontier-class model (interactive or async) to produce the initial plan
2. Plan reviews → Use different models for architect, design, or business critiques when practical
3. IMPLEMENT → Use a standard coding agent — follow the approved plan
4. TEST → Use a DIFFERENT model — feed it plan + public interfaces only; this is usually basic unit/integration coverage
5. AUDIT → Use a DIFFERENT model again — deep performance and security; this should be a higher-level check than TEST
6. VERIFY → Run full test suite, confirm all plan items implemented
```

Research workflow note: `/gal research` and `/gal deep-research` have their own VERIFY state for citation checking. That VERIFY pass must use a different model from the research author.

### Per-Phase Rules

- **The planning author and architect should differ when practical** — cross-check the plan before implementation
- **The planning author and designer should differ when designer is a formal reviewer** — keep design critique independent from plan authorship
- **Implementer and tester must be different models** — independent verification
- **Auditor should differ from implementer** — fresh perspective
- **Auditor should also differ from tester when practical** — audit should be higher-level than test generation
- **Auditor model tier ≥ implementer model tier** — the auditor must be at least as capable
- **Auditor model tier ≥ tester model tier** — audit should be at least as capable as test generation and usually stronger

The orchestrator correctness gate is not an independent reviewer replacement. It is an early full-context catch layer for checklist 1–13 plus obvious performance before test, while the independently dispatched auditor, tester, and verifier preserve the implementation-stage cross-check stack.

> **Executor routing**: machine-read per-role CLI assignment lives in `~/.gal/config/executor-routing.json`. See [docs/manual.md — Headless Executor Routing](docs/manual.md#headless-executor-routing) for setup. Copy `executor-routing.example.json` to customize.
