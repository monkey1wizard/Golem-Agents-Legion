# Coding Flow

## Named Workflow Obedience (workflow-level invariant)

When any GAL named workflow is invoked — `$gal-pipeline`, `$gal-finalize`, `$gal-status`, `$deep-planning`, `$refining-plan`, `$plan-to-prompt`, or equivalent `/gal ...` form — or when the user expresses repo-work intent (implement, plan, finalize, review, 實作, 規劃, 跑 pipeline, finalize), the active runtime MUST load and execute the corresponding `SKILL.md` before taking any action. Generic autonomous coding, batch edits, or summary responses that bypass the named workflow are a `named workflow obedience failure`. The always-on rule is rendered into `AGENTS.md` by `crates/cli/src/gal/render.rs`; this section is the workflow-contract pointer only — do not restate the rule body here.

The primary development workflow is control-plane-and-agent-driven, not dispatcher-state-driven.
Within Coding Flow, the primary work-file model is: source plans live in `.dev/plans/`, execution prompts live in `.dev/plans/` (suffix-distinguished), repo continuity lives in `.dev/state.md`, planning-stage domain reviews write back to the source plan, execution-stage specialists write detailed phase results to the `.dev/plans/<slug>.prompt.md` execution file, and `/gal pipeline` synchronizes task completion summaries across all three durable surfaces at task closeout.
Control-plane chat, `/gal status`, `/gal whats-next`, `/gal pipeline`, and execution-stage specialists all resume from the same repo-owned execution-memory substrate: `.dev/state.md` plus the active `.dev/plans/<slug>.prompt.md`.

Cross-model verification remains the default guardrail: planning critique, testing, and review should be done by different models whenever a separate capable model is available.

## Choosing Planning Depth

Not every change needs the same amount of planning. GAL uses command choice, not a named risk tier, to decide how much review to apply.

| Situation | Recommended Flow | Architect | Notes |
| --- | --- | --- | --- |
| Obvious local fix | Direct implement, optional orchestrator correctness gate, optional `golem-tester`, optional `golem-auditor` | Optional consult | Use when scope and impact are already clear |
| Scoped feature or known-cause bug | `/planning` -> `/refining-plan` -> `/plan-to-prompt` -> implement -> orchestrator correctness gate -> `golem-tester` -> `golem-auditor` -> `/gal finalize` | Optional consult | Use when the source plan is straightforward and does not need architectural challenge |
| Structural, cross-cutting, or uncertain change | `/planning` -> `/deep-planning` -> `/refining-plan` -> `/plan-to-prompt` -> implement -> orchestrator correctness gate -> `golem-tester` -> `golem-auditor` -> `/gal finalize` | Required in `/deep-planning` | Use when the plan touches shared structure, dependencies, public interfaces, or protected paths |

If implementation uncovers architectural uncertainty, stop and return to `/deep-planning` before continuing. After planning-stage changes, rerun `/refining-plan` before regenerating the execution prompt with `/plan-to-prompt`.

## Execution Lifecycle

GAL's coding flow is expressed as write-back command phases.

| Phase | Entry Signal | Owners | Main Files / Outputs |
| --- | --- | --- | --- |
| **Draft plan** | No active plan, or an existing plan needs reset | `/planning`, `/deep-planning`, `/refining-plan`, `/plan-to-prompt` | `.dev/plans/<slug>.md`, `.dev/plans/<slug>.prompt.md`, `.dev/state.md` active plan row |
| **Planning reviews** | Source plan exists, buildability not yet locked | Business, design, and engineering review lanes via collaborative tools or fallback golems | `## Open Questions`, `## Tasks`, `## Review Results`, `## Test Plan` |
| **Implementation** | Tasks exist and work remains | Manual execution or `/gal pipeline` | `## Status`, `## Tasks`, `.dev/state.md` for repo continuity, `.dev/plans/<slug>.prompt.md` for mutable execution state, code changes |
| **Review-stage audits** | Implementation reached a meaningful checkpoint | orchestrator correctness gate, `golem-tester`, `golem-auditor`, conditional `golem-designer` | `## Analyze`, `## Review Results`, `## Test Results` |
| **Wrap-up or landing** | Work is paused or ready to land | `/gal wrap-up`, `/gal finalize` | `### Handoff Notes`, `.dev/state.md`, active `.dev/plans/<slug>.prompt.md` |

The `Workflow:` field inside `## Status` is a **plan phase marker**, not a dispatcher-owned state machine. It may be useful for humans and specialist commands, but readiness is determined by the presence and contents of plan files and sections such as `## Tasks`, `## Analyze`, `## Review Results`, and `## Test Results`. When an execution prompt exists, chat-oriented control-plane actions and specialist agents must both treat that prompt as the mutable task-memory file rather than resuming from provider-local chat memory.

### File Ownership Rules

- `.dev/plans/<slug>.md` is the planning-stage source plan and human-readable task checklist. Planning commands and planning review lanes may update its full content; `/gal pipeline` may update task checkboxes and commit notes after a task passes implement, the orchestrator correctness gate, test, and audit.
- `.dev/plans/<slug>.prompt.md` is the execution-stage work file. `## Status`, retry counters, handoff notes, task commit markers, `## Test Results`, `## Review Results`, `## Analyze`, and detailed task execution history belong here.
- `.dev/state.md` is the repo-level active-plan index and session-continuity surface. `/gal pipeline` updates it after each completed task so `/gal status` and `/gal whats-next` resume from the same place a human sees in the source plan.
- Execution-stage specialists write detailed phase results to the prompt. The pipeline orchestrator owns cross-file convergence between source plan, execution prompt, and `.dev/state.md`.
- If `/gal status` or `/gal whats-next` sees a live workflow phase without the expected durable markers in `.dev/plans/<slug>.prompt.md`, or sees source-plan and prompt task checkboxes disagree, treat that as missing execution write-back rather than as a cleanly completed phase.

`golem-auditor` and `golem-designer` both belong to the post-implementation audit stage when used in audit mode. The orchestrator correctness gate owns correctness, completeness, scope drift, and obvious performance using the full task context before test. `golem-auditor` owns the independent deep-performance and security audit over implemented changes; `golem-designer` audits the running UI against `DESIGN.md`.

## Planning Reviews

`/deep-planning` always activates architect review before a plan is treated as implementation-ready.

**Planning Order Principle (single source — all three stages).** Planning progressively converges toward feasibility; it does not brainstorm and stop. Every stage follows **understand → diverge → converge**, and **never converge-first**:

1. **Understand** — read the relevant code first, depth proportional to the change (ref [`conventions/minimalism.md`](../conventions/minimalism.md) → "Understanding Precedes Minimization"). Docs are advisory; running code is decisive. Capture web/official-doc sources as URLs for later deep reading, don't inline-trace them here.
2. **Diverge** — enumerate the candidate approaches (list-only, cheap). Do not prune during enumeration.
3. **Converge** — apply the minimalism ladder ([`conventions/minimalism.md`](../conventions/minimalism.md)) to cut to the minimum correct implementation; reuse what already exists first.

This is **depth-scaled**: an obvious local fix collapses the loop to a quick self-check; a genuinely open/structural design space earns the full loop via `/deep-planning`. The invariant is "never converge-first" and "depth-scaled", **not** "every stage must diverge". The three planning stages (`/planning`, `/deep-planning`, `/refining-plan`) reference this principle **pointer-only**; they do not restate it.

**Minimalism runs at the converge step** (not before drafting): during the converge step above — and at every adversarial review gate — apply the minimalism ladder (ref: [`conventions/minimalism.md`](../conventions/minimalism.md)) to each proposed mechanism, abstraction, and file. Elements a rung eliminates (already-in-codebase reuse / std-lib / native-platform / installed-dep / one-line covers them) are cut before the plan is finalized. See `conventions/minimalism.md`.

**`/deep-planning` order contract — expand FIRST, converge AFTER:** the deep-planning instance of the Planning Order Principle. Phase A (expand/diverge) surfaces the full design space without pruning; Phase B (converge) applies the minimalism ladder challenge then adversarial reviews to cut back to the minimum correct implementation. Placing minimalism and adversarial reviews after expansion ensures they act on the full design surface. See `commands/deep-planning/SKILL.template.md`.

**Pointer-only hygiene:** planning commands (`/planning`, `/deep-planning`, `/refining-plan`) reference canonical convention homes (`conventions/`, `workflows/coding.md`) and do not restate rule bodies inline. When a command's SKILL body says "see X", reading X is authoritative; the SKILL body is the pointer, not the rule.

- **Architect**: always activates (mandatory) in `/deep-planning`; checks trade-offs, over-engineering, bug surface, dependency pollution, and public API risk. Applies the Degradation & Residue dimension; see `agents/golem-architect.agent.md`.
- **Steward (Step 3e)**: always activates in `/deep-planning` as a doc-structure adversarial review of the **plan document itself** (naming, structure, brand-residue, diagram sync). Scope is plan-doc only — knowledge extraction is a pipeline-closeout concern.
- **Analyst**: auto-activates when content touches business rules, pricing, permissions, notifications, onboarding, eligibility, or other customer-visible logic.
- **Designer**: auto-activates when content touches customer-facing flows, layout, states, components, or accessibility-sensitive interactions.

Each domain lane can also be invoked directly against the source plan outside of `/deep-planning` when that specialist review is needed without an architect-led deep-planning pass.

Planning-stage security concerns still sit with architect during `/deep-planning`, especially around trust boundaries, risky interfaces, and security-sensitive design decisions. `golem-auditor` does not replace that planning review; it audits the implemented branch once code exists.

Implementation-stage `AUDITOR` and `DEBUGGER` remain separate specialists. They do not replace planning review.

## Planning-Language Authority

When `planLanguage != en`, planning runs under a three-layer authority model so a non-English plan keeps both maximum technical accuracy and maximum language correctness:

1. **EN semantic draft** (`.dev/plans/<slug>.en.md`) — the sole planning-stage semantic authority. Exists only for non-English `planLanguage`. It is **not** a source plan: never indexed in `.dev/state.md` Active Plans, never a `/gal pipeline` or `/gal finalize` input, never an orphan/duplicate source-plan finding.
2. **Localized source plan** (`.dev/plans/<slug>.md`) — the human-facing surface AND the GAL-tool-visible source / human-approval / review-marker surface. Rendered from the EN draft; carries a planning-authority metadata block.
3. **English execution prompt** (`.dev/plans/<slug>.prompt.md`) — the post-prompt execution authority (English-only, unchanged).

**planLanguage routing.** Normalize `planLanguage` (trim + lowercase); if it has the `en` prefix, the plan is English — `.dev/plans/<slug>.md` is itself the EN source plan and **no draft is created** (single-file fast path, zero reconcile cost). Otherwise the planning stage maintains the EN-draft + localized-render pair.

**Machine-anchor allowlist (keep, no rename).** In a non-English source plan, section headings, file paths, commands, code spans, task/test IDs (`T-NN` / `TP-NN`), review-verdict literals, and metadata keys stay in **English and MUST NOT be renamed** — `prompt-check` and `finalize-check` depend on them. Only the narrative prose follows `planLanguage`. This is a preservation rule, not a translation target.

**Reconcile on hand-edit.** A user may hand-edit the localized source plan, but before the next planning-stage command (`/deep-planning`, `/refining-plan`, `/plan-to-prompt`) continues, a rendered-source-hash mismatch forces a read-only reconcile preflight: the divergence is merged back into the EN draft (the single semantic authority), the localized source is re-rendered, and the metadata hashes are re-stamped by the internal `gal planning-stamp`. After `/plan-to-prompt` passes, the EN draft is deleted; the equivalence proof lives on as the source plan's inline `prompt-hash` / `equivalence-verdict` metadata fields, stamped in place by `gal planning-stamp --equivalence <prompt>` — there is no separate `.equiv.md` file. The deterministic gates (`planning-check`, `prompt-check`) enforce hash freshness, machine-anchor parity, language correctness, and the inline equivalence verdict; the semantic merge itself is the planning-stage command's (model's) job.

## STAGE 3.5 — Definition-of-Ready Gate (REFINE-LOCK Loop)

After `/deep-planning` and the OQ-completion gate, refining is **not** three linear hand-offs — it is one **REFINE-LOCK loop** that iterates to convergence before `/plan-to-prompt` may run. The loop has three internal gates; REVISE or non-convergence sends it back round:

1. **refining** drafts `## Tasks` / `## Test Plan` / `### Engineering Review`, applying the Atomicity Rubric (a)–(e) and the quantitative split triggers (see `commands/refining-plan`).
2. **Definition-of-Ready dual lens** (Three-Amigos; **review lens ≠ refiner**, cross-model when practical):
   - **architect lens** — structural atomicity, blast radius, dependencies, rename+move bundling, feasibility → APPROVE / REVISE.
   - **tester lens** — every task has a minimal, reproducible, observable acceptance probe (the test **contract**, spec-layer not code; this is rubric (e)'s oracle); tighten `TP-NN`, name the evidence shape → APPROVE / REVISE.
   - **analyst lens (conditional)** — value lens when business rules / permissions / pricing are touched.
3. **STAGE 3.6 convergence (dual-owner)** — **STEWARD** converges documentation structure (`.dev/plans/` doc well-formed / naming / no orphan-or-duplicate / figure sync); **ORCHESTRATOR** converges `.dev` execution state (`state.md ↔ .dev/plans` agreement, lifecycle markers).

**Stop-line (jidoka):** any REVISE, or non-convergence, returns to refining or `/deep-planning` — defects are fixed at the source, not waved through.

**Exit condition for readiness review (conjunction — all required):** `<!-- ENG_REVIEW: CLEAR -->` ∧ dual-lens APPROVE ∧ STAGE 3.6 converged (docs + `.dev`).

**Exit condition for `/plan-to-prompt`:** readiness-review exit condition ∧ `## Approval > Human approval = [clear|approved]`.

**Depth-scaling:** an obvious local fix may collapse the loop to "rubric self-check + light convergence". Structural / multi-file / protected-path changes require the architect + tester dual sign-off. A single execution plan must stay **≤ 99 blocking tasks** — over that, split the plan (see `commands/refining-plan`).

**Depth-scaled diagrams:** structural / multi-file / Protected-Path plans include a `## Diagrams` section (ASCII flowchart or architecture diagram) that aids comprehension. Obvious local-fix plans may omit it. The STAGE 3.6 STEWARD gate checks that any diagram is in sync with the finalized `## Tasks` scope.

The canonical atomicity anti-patterns and the atomicity principle are the single source of truth in [`conventions/task-atomicity.md`](../conventions/task-atomicity.md); the operational rubric and quantitative split triggers live in `commands/refining-plan`.

OQ closure is **class-gated** — single source of truth in [`conventions/open-questions.md`](../conventions/open-questions.md): **H** (human-only authority) / **A** (architect-role, recorded rationale) / **F** (false OQ). `/refining-plan` has zero closure authority (gate-check only); the architect role closes A/F via `/deep-planning` or a direct `/gal architect` write-back review; **doubt → H**.

```text
PLANNING WORKFLOW
  /planning ─► .dev/plans/<slug>.md            (STAGE 1; steward light doc-naming check)
      │  (structural / protected-path → required)
      ▼
  /deep-planning ─► architect (always) · designer/analyst (conditional)   (STAGE 2)
      │  (architect CLEAR)
      ▼
  OQ-completion gate ── all OQ closed per class authority (H=human · A/F=architect role, recorded)
      │  (OQ = 0)        → fold into body, remove entries (no `[x]` breadcrumbs); see conventions/open-questions.md
      ▼
  ╔═ STAGE 3  REFINE-LOCK LOOP — ONE loop, iterate to convergence (NOT 3 linear steps) ═══╗
  ║  (1) refining drafts ## Tasks / ## Test Plan / ENG Review  (rubric (a)–(e) + triggers)║
  ║        ▼                                                                              ║
  ║  (2) Definition-of-Ready dual lens (review lens ≠ refiner):                              ║
  ║        architect = structural atomicity/blast radius   → APPROVE/REVISE               ║
  ║        tester    = minimal observable probe (contract, not code) → APPROVE/REVISE     ║
  ║        analyst (conditional) = value lens                                             ║
  ║      REVISE ─► back to (1) or stop-line to /deep-planning (jidoka)                    ║
  ║        ▼ APPROVE                                                                      ║
  ║  (3) STAGE 3.6 convergence (dual-owner):                                              ║
  ║        STEWARD = documentation structure (.dev/plans well-formed/naming/figure-sync)  ║
  ║        ORCHESTRATOR = .dev execution state (state.md ↔ .dev/plans, lifecycle)         ║
  ║      not converged ─► fix → back to (1)/(2)                                           ║
  ║        ▼                                                                              ║
  ║  EXIT (readiness review): ENG_REVIEW CLEAR ∧ dual-lens APPROVE ∧ converged(docs + .dev)║
  ╚═══════════════════════════════════════════════════════════════════════════════════════╝
      │  + human approval recorded in ## Approval
      ▼
  /plan-to-prompt ─► .dev/plans/<slug>.prompt.md (DRAFT)   (STAGE 4)
      ▼
  /gal pipeline ─► implement → correctness gate → tester → auditor → commit → … → orchestrator goal-verify
```

This is **one** REFINE-LOCK loop: (1) refining, (2) dual-lens DoR, (3) STAGE 3.6 convergence are internal gates of the same phase, not three independent command stages. REVISE / non-convergence loops back; the exit is the conjunction above.

## Optional Capability Preflight

Before planning, review, or specialist lanes attempt to use an optional capability, resolve its state through the five-state preflight in [optional-capabilities.md](../conventions/optional-capabilities.md).

| Workflow phase | Capabilities that may apply | Degrade behavior |
| --- | --- | --- |
| `/planning` and `/deep-planning` | structural-retrieval for structural context | Continue with native planning and fallback golems |
| review-stage audit | structural-retrieval for cross-community coupling checks | Continue with standard diff-based review |
| research workflows | OpenCLI for structured external retrieval | Fall back to MCP retrieval or browser tools |

Do not treat a missing optional capability as a workflow error. Do not prompt for install or initialization unless the user explicitly asked for the capability-specific setup.

## Architectural Escalation Fence

Implementation must stop and obtain a **recorded architect review** before continuing if the work requires any of the following structural changes:

- Create or delete project files (.csproj, .sln, package.json, etc.)
- Add or remove package dependencies
- Move files across architecture layers
- Create new interfaces or abstract base classes
- Modify DI registrations or service composition
- Change public API signatures used by 2+ consumers
- Introduce new design patterns
- Modify shared/core/base classes used by 3+ consumers

These changes need an architect-reviewed plan before implementation continues. **The gate binds to the architect review itself, not to a specific command.** The standard path to that review is `/deep-planning`, but a direct `/gal architect` write-back recording an `APPROVE` verdict in the plan's `## Review Results > ### Architecture Review` (marked `<!-- ARCH_REVIEW: CLEAR -->`) satisfies it equally. A plan whose architect verdict is `Pending` or absent has **not** cleared the gate no matter how many informal consults were absorbed; conversely a plan that already carries a CLEAR architect verdict does **not** need `/deep-planning` re-run.

## Session Safety Mode

Session safety is now agent-internal discipline, not a public command family.

- `golem-debugger` owns freeze-style scope control during investigations
- destructive shell operations still require explicit caution and user clarity
- do not reintroduce session-safety slash commands as public workflow steps

## Documentation Structure (golem-steward)

`golem-steward` is the first-class agent that owns **documentation structure** (callable via `/gal steward`, roster-visible). Its charter is documentation structure only: the NDJSON structure map, code→doc drift, end-of-run knowledge extraction → `docs/`, `.dev/plans/` structural hygiene, and figure/flowchart sync.

**Three activation points (all documentation-structure framed):**

1. **planning open (adversarial)** — new plan **document** naming compliance; `.dev/plans/` has no orphan or duplicate plan documents; plan prose free of brand/name residue.
2. **refining end (adversarial)** — source-plan document is well-formed, `.dev/plans/` naming is compliant, `## Diagrams` in sync with finalized `## Tasks`, figures/flowcharts are in sync. (`.dev/state.md` ↔ `.dev/plans` three-surface convergence is ORCHESTRATOR's.)
3. **pipeline closeout** — code→doc drift, structure-map update, and end-of-run knowledge extraction → durable layer (`README.md` + `docs/`); then re-index `.dev/project.md`.

**Charter boundary (no double-ownership):**

- **STEWARD = documentation structure**: structure map, doc drift, knowledge extraction → `docs/`, `.dev/plans/` structural hygiene, figure sync.
- **ORCHESTRATOR = execution state + lifecycle**: `.dev/state.md` ↔ `.dev/plans` three-surface convergence, task closeout, and plan lifecycle (ABSORBED / delete). Steward does NOT converge `.dev` execution state.

## Protected Paths

Each repo's `.dev/project.md` contains a `## Protected Paths` section listing architecture-critical files. Touching any protected path requires a **recorded architect review** before implementation continues — an `APPROVE` verdict in the plan's `## Review Results > ### Architecture Review`, marked `<!-- ARCH_REVIEW: CLEAR -->`. `/deep-planning` is the standard path to that review; a direct `/gal architect` write-back is equally valid. **The gate is the architect verdict, not the `/deep-planning` command** — a plan that already carries a CLEAR architect verdict passes without re-running `/deep-planning`, and a plan whose architect review is still `Pending` is not cleared by informal consults alone.

## Plan Lifecycle: Transient Task Memory

Plans are temporary work files, not permanent records. `.dev/plans/` is a staging area.

### Lifecycle

1. **`/planning` creates the source plan** → `.dev/plans/<type>-<slug>.md`
2. **`/deep-planning` refines the source plan when needed** → keeps scope and rationale review-ready
3. **`/refining-plan` writes the implementation contract into the source plan** → `## Tasks`, `## Test Plan`, `## Review Results > ### Engineering Review`
4. **`/plan-to-prompt` creates or refreshes the execution prompt from that reviewed, human-approved source plan** → `.dev/plans/<type>-<slug>.prompt.md`
5. **The execution prompt self-tracks progress** → `## Status` carries phase markers, step, deviations, and decisions
6. **Implementation updates prompt progress** during execution while the task is in flight
7. **Testing and review write results** → execution prompt `## Test Results`, `## Review Results`, `## Analyze`
8. **Pipeline task closeout converges state** → source plan task checkbox and commit note, execution prompt status/task summary, and `.dev/state.md` session continuity all agree before the next task starts
9. **Verification confirms the goal** → extracts knowledge to `docs/`, marks the plan ready for closure
10. **Plan is deleted after lifecycle closure** → task memory returns to zero, no orphaned state

### Plan Filename Convention

```text
.dev/plans/<type>-<slug>.md
.dev/plans/<type>-<slug>.prompt.md
```

Type prefixes: `feat-`, `fix-`, `refactor-`, `sec-`, `perf-`, `infra-`, `release-`

### Safety Rules

- **Never delete a plan before verification and handoff are complete** — the plan is the spec; early deletion breaks review, QA, and resumption.
- **Plan is pipeline, docs/ is sink** — valuable knowledge flows from the plan into long-lived docs. The plan itself is disposable only after that extraction.

### Release-Plan Type (`release-<slug>`)

A `release-` plan is a first-class plan type, not a phase of `finalize` or `pipeline`.

**Lifecycle:**
1. **`/gal releaser`** (consult, isolated) — reads project context, researches APIs/CICD tools, designs a release/devops flow, emits design advice. Does not write files. Does not execute.
2. **`/planning release-<slug>`** — materializes the design advice into a `release-<slug>.md` source plan with `## Tasks`.
3. **Normal pipeline** (`/gal pipeline`) — `golem-implementer` / `golem-tester` / `golem-auditor` execute the plan atomically.
4. **`/gal finalize`** — lands and closes the release plan like any other plan.

**Boundaries:**
- `golem-releaser` is **consult-only** (tools: `read`/`search`/`web`). It never edits files, never commits, and never executes commands.
- `release-` is a plan type — not a `Phase` enum variant, not a pipeline phase, and not a step inside `/gal finalize`. Finalize's Sequence 2 is doc-sync only.
- Release execution (version bump, artifact build, publish) is standard implementer work driven by the `release-` plan's `## Tasks`.

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

**Provider-Memory Harvest inlet.** This wrap-up-time handoff is the **sole** intake boundary for provider-memory harvest — see `conventions/token-budget.md` → Provider-Memory Harvest for the canonical opt-in, redaction, and approval contract. No other command or lifecycle stage acquires provider memory.

## Direct Agent Invocation

### Role Invocability Matrix

Two user-facing classes: **Directly callable** and **Orchestrated-only**.

| Role | Class | Notes |
| --- | --- | --- |
| **architect** | Directly callable | `MODE: direct`; isolated default or in-context via `/gal discuss architect` |
| **analyst** | Directly callable | `MODE: direct`; auto-activates in `/deep-planning` for business-rule content; dual-mode |
| **designer** | Directly callable | `MODE: direct`; auto-activates in `/deep-planning` for UX/UI content; dual-mode |
| **releaser** | Directly callable | `MODE: direct`; planning-stage release-flow designer; isolated default or in-context via `/gal discuss releaser`; emits design advice, does not execute |
| **debugger** | Directly callable | `MODE: direct`; isolated only — no `discuss` support; always directly callable |
| **steward** | Directly callable | `MODE: direct`; isolated only — no `discuss` support; callable via `/gal steward` |
| **implementer** | **Orchestrated-only** | Only via `/gal pipeline` (pipeline-phase context); bare call → `COMMAND: error` |
| **tester** | **Orchestrated-only** | Only via `/gal pipeline` (pipeline-phase context); bare call → `COMMAND: error` |
| **auditor** | **Orchestrated-only** | Via `/gal pipeline` (task audit) or `/gal finalize` (whole-branch audit); bare call → `COMMAND: error` |
| **researcher** | **Orchestrated-only** | Only via `/gal research` or `/gal deep-research`; bare call → `COMMAND: error` |

### Consult Dual-Mode

Four of the six directly callable roles (architect, analyst, designer, releaser) additionally support `discuss` and can run in two conversation modes; debugger and steward are directly callable but isolated-only (no `discuss`):

| Mode | Trigger | What happens | Response label |
| --- | --- | --- | --- |
| **Isolated** (default) | `/gal <role>` | Native subagent runs role in isolation; only verdict/summary returns to main context | `[<role> · isolated]` |
| **In-context** | `/gal discuss <role>` | Activation-core loads into main conversation; hot-joins from prior isolated verdict in transcript; continues multi-turn until topic changes | `[<role> · in-context]` |

**Hot-join**: the isolated verdict is already in the transcript, so in-context mode continues from there without re-running the role from scratch.

### Invocation Table

| Type | Allowed | Examples |
| --- | --- | --- |
| **Directly callable — isolated** | Yes — read-only advice, no phase transition | `/gal architect`, `/gal analyst`, `/gal designer`, `/gal releaser`, `/gal debugger`, `/gal steward` |
| **Directly callable — in-context (discuss)** | Yes, four roles only — loads activation-core into main context | `/gal discuss architect`, `/gal discuss analyst`, `/gal discuss designer`, `/gal discuss releaser` |
| **Orchestrated-only** | No — bare call → `COMMAND: error` | `implementer`, `tester`, `auditor`, `researcher` (only reachable via pipeline / finalize / research commands) |

Direct-call output is advice unless the named agent's contract explicitly includes formal write-back for that specialist stage.

## Model Roles and Per-Phase Assignment

Define roles by **what they do**, not by which model they are. When you switch AI tools, update `config.json#executorRouting` (machine-read dispatch) and the mapping section below (human reference).

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

### Routing Rules

1. **Planning review should be a different-model check** — the model that critiques a plan should differ from the one that drafted it whenever practical.
2. **CODER and TESTER must be different models** — independent verification
3. **AUDITOR should differ from CODER** — fresh perspective catches blind spots
4. **AUDITOR should also differ from TESTER when practical** — audit is a higher-level check than test generation
5. **DESIGNER should differ from CODER when used as a formal review lane** — keep experience review independent from implementation
6. **RESEARCHER owns research, synthesis, and cross-review** — independent reference verification must be done by a different model
7. When switching tools, update `config.json#executorRouting` (machine-read) and any personal planning notes

### Planning Review Rules

`/deep-planning` is the default architect-reviewed planning pass before prompt generation.

| Review Lane | When Included | Verdict Required? |
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
- **The planning author and designer should differ when designer is a formal review lane** — keep design critique independent from plan authorship
- **Implementer and tester must be different models** — independent verification
- **Auditor should differ from implementer** — fresh perspective
- **Auditor should also differ from tester when practical** — audit should be higher-level than test generation
- **Auditor model tier ≥ implementer model tier** — the auditor must be at least as capable
- **Auditor model tier ≥ tester model tier** — audit should be at least as capable as test generation and usually stronger

The orchestrator correctness gate is not a substitute for the independently dispatched auditor and tester. It is an early full-context catch layer for checklist 1–13 plus obvious performance before test, while the independently dispatched auditor and tester preserve the implementation-stage cross-check stack and the orchestrator owns end-of-run goal-backward verification.

### Checking-Role Triangle

Checking responsibilities form a triangle:

- **ORCHESTRATOR** — dispatch + per-task correctness gate + **end-of-run goal-backward verification** + plan lifecycle (ABSORBED / delete). The orchestrator owns verification and **always runs it in-process** — verify is never dispatched out (there is no `verify` dispatch phase and no `VERIFY` routing key in `config.json#executorRouting` or `crates/dispatch`). Independent cross-verification is provided by AUDITOR, not by a separate verify dispatch.
- **AUDITOR** — independent deep audit **within a single task** (deep performance + security); not goal/cross-task/lifecycle. Standalone branch-audit mode unchanged.
- **STEWARD** — documentation structure: end-of-run knowledge extraction → `docs/`, doc drift, structural hygiene.

```text
══════════════════════════════════════════════════════════════════════
 ORCHESTRATOR              AUDITOR                   STEWARD
 dispatch + verify         single-task deep audit    documentation structure
 returns (per-task         (deep perf + security,    (knowledge extraction →
 correctness + end-of-run  ≠coder; NOT goal/         docs/, doc drift,
 goal-backward) + lifecycle cross-task/lifecycle)    structural hygiene)
══════════════════════════════════════════════════════════════════════

per task T-NN:
  ORCHESTRATOR ── dispatch implement → CODER
       │ collect
       ▼
  ORCHESTRATOR ── check returns (single-task correctness gate)
       │ not-ok → back to CODER
       │ ok → dispatch test → TESTER → dispatch audit → AUDITOR
       ▼
  AUDITOR ── single-task deep audit (perf + OWASP/STRIDE ≥8); BLOCKING/security/protected → fix or STOP
       ▼
  ORCHESTRATOR ── commit + three-surface convergence   ↺ next task
       │ (all tasks done)
       ▼
  ORCHESTRATOR ── end-of-run goal-backward verification
       │   does the whole result actually achieve the plan GOAL? (task done ≠ goal met)
       │   VERIFIED → continue │ GAPS_FOUND / BLOCKED → STOP
       ▼
  STEWARD ── knowledge extraction → docs/ (documentation structure)
       ▼
  ORCHESTRATOR ── lifecycle: mark ABSORBED + delete plan files (post-finalize)
```

**Verify-independence policy.** End-of-run goal-backward verification is **ORCHESTRATOR-owned and always in-process** — it is never dispatched to another model. The cross-model independence guardrail is provided by **AUDITOR** (the independent ≠coder deep audit within each task), not by a separate verify dispatch. There is no `verify` dispatch phase, no `VERIFY` routing key, and no `DEGRADED_SAME_RUNTIME` verify marker.

The checklist's "naming conventions" item is governed by the project-wide naming authority — `conventions/naming.md` (full term registry in `docs/naming.md`): reserved words (bare `agent` = golem agent, `model` = LLM), qualified overloaded terms, no generic bucket names, and provenance discipline (plan-task IDs and migration narration only under `.dev/**`). The naming gate enforces provenance + retired terms at pre-commit and pipeline closeout.

> **Executor routing**: machine-read per-role CLI assignment lives in `config.json#executorRouting`. See [docs/manual.md — Headless Executor Routing](../../../docs/manual.md#headless-executor-routing) for the `executorRouting` subtree shape to paste into `config.json`.

## Plan Finalization (`/gal finalize`)

`/gal pipeline` ends at end-of-run goal-backward verification (VERIFIED) and reports READY TO FINALIZE — it does **not** land or close the plan; landing is `/gal finalize`'s job. `/gal finalize` is the gated, interruptible/resumable **completion command** that takes a verified plan the rest of the way: whole-branch holistic review → doc-sync → (on a worktree/feature branch) merge-to-main + teardown → bounded `.dev/state.md` write + a distinct post-write hygiene-only receipt → lifecycle close → last-good tag. This is the canonical flowchart; `docs/manual.md` and `README.md` link here rather than redrawing it.

```text
/gal pipeline ──► all tasks [x] + ORCHESTRATOR goal-backward VERIFIED  (READY TO FINALIZE)
      │
      ▼
/gal finalize
  precondition gate (full-mode `gal finalize-check` receipt): all task [x] ∧ goal-backward VERIFIED ∧ clean tree ∧ detect worktree/branch vs main
      │ miss → HARD-BLOCK + redirect (no action):
      │   · both plan & prompt [ ] = incomplete      → /gal pipeline
      │   · plan [ ] but prompt [x] = write-back gap  → ORCHESTRATOR three-surface converge → re-gate
      │   · dirty tree → commit first  ·  just pausing → /gal wrap-up
      ▼
  1. holistic review (whole-branch, cross-task)
       architect lens = correctness / arch fit / conventions / scope-drift   (review lens ≠ dispatcher)
       auditor standalone branch-audit = deep perf + OWASP/STRIDE
       ORCHESTRATOR is NOT a review owner (it owns precondition verify + final close only)
       REVISE → fix (implementer) → re-review clean → commit on branch
      ▼
  2. doc-sync: STEWARD → durable layer (README.md + docs/ excl. plans/+research/); then re-index .dev/project.md; adapter render runs in-process via gal finalize-check (already byte-identical ×2)
      ▼
  3. (worktree/branch) merge into main — plain `git merge` (no squash / ff-only / rebase); conflict → STOP + handoff
      ▼
  4. (worktree/branch) teardown — plain git (`git worktree remove` + `git branch -d`); cross-runtime
      ▼
  5a. bounded state write — remove exact-predicate legacy comments from .dev/state.md; upsert + trim ## Recent Close-outs to <=2 rows (Landing = durable-layer commit)
      ▼
  5b. post-write hygiene-only receipt (`gal finalize-check --hygiene-only`, distinct default path) — every applicable row must pass
      │ fail → STOP: keep plan files, repair the named drift, re-run
      ▼
  5c. lifecycle close — ORCHESTRATOR: ABSORBED + delete plan files [gated on BOTH the durable-layer commit hash from STEWARD AND the 5b hygiene-only pass]
      ▼
  6. `git tag -f gal-last-good <landing-commit>` — advance the known-good marker; non-fatal if it fails
```

**Provider-Memory Harvest at finalize.** Step 2 (doc-sync) is pointer-only: STEWARD may consider provider-memory candidates already approved in the plan's `### Handoff Notes` (see Context Handoff above), but finalize performs no provider acquisition of its own. Approval is never itself promotion evidence — promotion still requires the same verified-root-cause-plus-credible-recurrence gate as any other task-to-project promotion (`conventions/token-budget.md`); a gate miss omits the candidate without blocking finalize.

### Zero-Trust Gates (mechanized — finalize trusts evidence, not self-reports)

finalize re-establishes facts rather than trusting recorded claims. Six gates carry this, mirrored in the `gal-finalize` SKILL:

1. **Precondition = `gal finalize-check` receipt, not the prompt's `VERIFIED`.** The recorded VERIFIED verdict is an unverified claim; the deterministic binary re-runs the repo's authoritative checks (declared in `.dev/project.md` `<!-- gal:authoritative-check -->`), commit-existence, three-surface checkbox agreement, cited-test existence, and adapter-render idempotency (in-process ×2). Any failing/not-run check → HARD-BLOCK with the real command output. When the plan touched `crates/`, rebuild+install `gal` before checking (self-bootstrap) so the receipt reflects the work being finalized.
2. **Pipeline-integrity before holistic review.** An `integration` oracle silently satisfied by a `unit` test → GAP+STOP; `no-receipt`/`timeout`/exit-0-alone are never PASS; cited test names must exist (binary receipt is the authority).
3. **doc-sync is mandatory, never skipped for "drift risk".** The adapter render runs in-process via `gal finalize-check` (idempotency already asserted); doc-sync lands durable knowledge then re-indexes `.dev/project.md`; a non-idempotent render → STOP.
4. **Holistic review independence — honest when separable, marked when not.** Non-separable model → `Review Independence: DEGRADED_SAME_RUNTIME` + mandatory full `git diff` per-file judgement evidence; an unread-diff stamp is a gate failure; never claim a dispatched independent reviewer when none ran.
5. **Evidence-aligned report.** Every `--- FINALIZE COMPLETE ---` line maps to a proving evidence pointer; no-evidence lines may only read `skipped`/`DEGRADED`/`not-run`, never `done`/`CLEAR`/`LANDED`.
6. **Post-write hygiene-only receipt closes the full time-of-check window.** The Step 1 precondition receipt runs before holistic review, doc-sync, and merge — drift any of those introduce (including drift from the merge itself) would otherwise reach plan-file deletion unchecked. A second, distinct `gal finalize-check --hygiene-only` receipt runs after the bounded `.dev/state.md` write and before deletion; any applicable row failing it is a STOP that keeps the plan files, never a silent pass-through.

### Holistic Review Stage (cross-task, whole-branch)

Per-task audit passing does **not** prove the combined branch is coherent. finalize closes that gap by orchestrating a whole-branch review over the full diff, delegating existing owners — **no new agent**:

- **correctness / architecture fit / conventions / scope-drift** → `golem-architect` lens (a small non-planning-stage close-out extension of its charter). The review owner must be independent of the dispatcher, so the **ORCHESTRATOR does not self-review**.
- **deep performance / security** → `golem-auditor` standalone branch-audit mode (unchanged).

Holistic review ≠ goal-backward verification: the latter is ORCHESTRATOR-owned and is finalize's **precondition**, not part of its review stage.

### Lifecycle-Delegation Boundary

finalize holds **zero** lifecycle authority — it only sequences existing owners:

- **ORCHESTRATOR** owns the goal-backward verify (precondition) and lifecycle close: ABSORBED marking + plan-file deletion + `.dev/state.md` close-out.
- **STEWARD** owns knowledge extraction → durable layer (`README.md` + `docs/`) and provides the **durable-layer commit hash** that gates plan-file deletion.

finalize itself deletes nothing, extracts nothing, and re-verifies nothing.

### finalize ↔ wrap-up Boundary

- **`/gal wrap-up`** = non-destructive session **pause**, callable anytime; it compresses handoff notes and updates session continuity so any session/machine can resume. It does not land or delete anything.
- **`/gal finalize`** = one-shot, gated, **destructive completion landing** (merge + plan-file deletion) for a plan that is actually done.

They reverse-prompt each other (wrap-up in a completed-plan state offers finalize) but never overlap: pausing is wrap-up, landing is finalize.
