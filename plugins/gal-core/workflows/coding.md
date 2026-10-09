# Coding Flow

## Writing quality

Apply the neutral [writing-quality convention](../conventions/writing-quality.md)
to main replies, progress prose, specialist handbacks, and durable documents.
Checker evidence supplements semantic review. Preserve `PROSE_AUDIT: required`,
ownership, not-run, STOP semantics, machine headings, and receipt placement.
Advisory checker unavailability permits self-review with truthful status.
Required artifact checks fail closed when unavailable or when blocking findings
remain. Do not create checker-generated progress recursion or dispatch an
auditor per message.

## Named Workflow Obedience (workflow-level invariant)

When any GAL named workflow is invoked — `$gal-pipeline`, `$gal-finalize`, `$gal-status`, `$deep-planning`, `$refining-plan`, `$plan-to-prompt`, or equivalent `/gal ...` form — or when the user expresses repo-work intent (implement, plan, finalize, review, 實作, 規劃, 跑 pipeline, finalize), the active runtime MUST load and execute the corresponding `SKILL.md` before taking any action. Generic autonomous coding, batch edits, or summary responses that bypass the named workflow are a `named workflow obedience failure`. The always-on rule is rendered into `AGENTS.md` by `crates/cli/src/gal/render.rs`; this section is the workflow-contract pointer only — do not restate the rule body here.

The primary development workflow is control-plane-and-agent-driven, not dispatcher-state-driven.
Within Coding Flow, the primary work-file model is: source plans live in `.dev/plans/`, execution prompts live in `.dev/plans/` (suffix-distinguished), repo continuity lives in `.dev/state.md`, planning-stage domain reviews write back to the source plan, execution-stage specialists deliver phase results via receipts (placed into `.dev/plans/<slug>.prompt.md` by the control node for in-scope dispatches), and `/gal pipeline` synchronizes task completion summaries across all three durable surfaces at task closeout.
Control-plane chat, `/gal status`, `/gal whats-next`, `/gal pipeline`, and execution-stage specialists all resume from the same repo-owned execution-memory substrate: `.dev/state.md` plus the active `.dev/plans/<slug>.prompt.md`.

Cross-model verification remains the default guardrail: planning critique, testing, and review should be done by different models whenever a separate capable model is available.

## Choosing Planning Depth

Not every change needs the same amount of planning. GAL uses command choice, not a named risk tier, to decide how much review to apply.

| Situation | Recommended Flow | Architect | Notes |
| --- | --- | --- | --- |
| Obvious local fix | Direct implement, optional orchestrator task check, optional `golem-tester`, optional `golem-auditor` | Optional consult | Use when scope and impact are already clear |
| Scoped feature or known-cause bug | `/planning` -> `/refining-plan` -> `/plan-to-prompt` -> implement -> orchestrator task check -> `golem-tester` -> `golem-auditor` -> `/gal finalize` | Optional consult | Use when the source plan is straightforward and does not need architectural challenge |
| Structural, cross-cutting, or uncertain change | `/planning` -> `/deep-planning` -> `/refining-plan` -> `/plan-to-prompt` -> implement -> orchestrator task check -> `golem-tester` -> `golem-auditor` -> `/gal finalize` | Required in `/deep-planning` | Use when the plan touches shared structure, dependencies, public interfaces, or protected paths |

If implementation uncovers architectural uncertainty, stop and return to `/deep-planning` before continuing. After planning-stage changes, rerun `/refining-plan` before regenerating the execution prompt with `/plan-to-prompt`.

## Execution Lifecycle

GAL's coding flow is expressed as write-back command phases.

| Phase | Entry Signal | Owners | Main Files / Outputs |
| --- | --- | --- | --- |
| **Draft plan** | No active plan, or an existing plan needs reset | `/planning`, `/deep-planning`, `/refining-plan`, `/plan-to-prompt` | `.dev/plans/<slug>.md`, `.dev/plans/<slug>.prompt.md`, `.dev/state.md` active plan row |
| **Planning reviews** | Source plan exists, buildability not yet locked | Business, design, and engineering review lanes via collaborative tools or fallback golems | `## Open Questions`, `## Tasks`, `## Review Results`, `## Test Plan` |
| **Implementation** | Tasks exist and work remains | Manual execution or `/gal pipeline` | `## Status`, `## Tasks`, `.dev/state.md` for repo continuity, `.dev/plans/<slug>.prompt.md` for mutable execution state, code changes |
| **Review-stage audits** | Implementation reached a meaningful checkpoint | orchestrator task check, `golem-tester`, `golem-auditor`, conditional `golem-designer` | `## Analyze`, `## Review Results`, `## Test Results` |
| **Wrap-up or landing** | Work is paused or ready to land | `/gal wrap-up`, `/gal finalize` | `### Handoff Notes`, `.dev/state.md`, active `.dev/plans/<slug>.prompt.md` |

The `Workflow:` field inside `## Status` is a **plan phase marker**, not a dispatcher-owned state machine. Readiness comes from the plan files and sections such as `## Tasks`, `## Analyze`, `## Review Results`, and `## Test Results`. Chat-oriented control-plane actions and specialist agents must both treat that prompt as the mutable task-memory file rather than resuming from provider-local chat memory.

### Pipeline Continuation Profiles

The default `legacy_interactive` profile preserves the existing v1 invocation, receipt and marker formats, process exit behavior, projection layout, and host-owned continuation. A trusted Codex capability may select the versioned `codex_stop_v1` profile. Do not infer the profile from an executor name, model, environment guess, or stale marker. The profile and guarded action contract are defined in the [`/gal-pipeline` command](../commands/gal-pipeline/SKILL.template.md#continuation-contract).

In `codex_stop_v1`, each CLI segment owns mechanical transitions until it reaches a typed action, terminal outcome, human-authority blocker, or unrecoverable evidence failure. The ORCHESTRATOR retains task-quality, boundary-widening, convergence, and goal-backward judgments. The host continues the same thread and starts each next segment from the action returned by the current gate. This does not transfer loop or phase-dispatch authority to the coordinator.

### Guarded Segment Lifecycle

Follow this order for every guarded entry or resume:

1. Run a fresh `gal pipeline-preflight` entry latch and require its isolated receipt to report `overall: pass`. On resume, run the latch again before accepting a checkpoint receipt or starting a provider. This entry-latch rule is defined here.
2. Recover any pending in-root projection journal. Apply idempotent recovery only when target hashes match the recorded transaction. Stop on a third-hash conflict before classifying evidence or advancing coordinator state. This journal recovery rule is defined here.
3. Validate the current typed action against its coordinator revision, commit, task, phase, attempt, and prompt binding. Reject stale, replayed, or incomplete evidence without redispatching or consuming semantic test and audit retries.
4. Start or resume exactly the segment named by that action. Keep task-quality, boundary, convergence, and final goal-backward checkpoints in the same ORCHESTRATOR thread. The coordinator persists each checkpoint. The ORCHESTRATOR supplies a fresh checkpoint-bound receipt and invokes the typed resume action.
5. Before downstream gates classify a v2 attempt, require consistent provider-session identity and provenance across the dispatch outcome, terminal attempt log, and evidence. This workflow defines that provenance rule. Require the plan, task, phase, attempt, receipt, prompt/spec, and tested commit/diff bindings defined by the [`/gal-pipeline` command](../commands/gal-pipeline/SKILL.template.md#continuation-contract). Preserve v1 evidence and its public grammar under `legacy_interactive`.
6. Project verified progress through the journal. Complete cross-file convergence only after the applicable phase gates pass.

The coordinator and gate commands own typed action, evidence, and journal mechanics. The ORCHESTRATOR owns semantic decisions. Review-stage duties remain independent: task check, test, audit, and any required design review retain their assigned owners and cross-model independence rules.

### Staged Write-Back Ownership

For dispatched execution phases, write-back ownership is split between executor delivery and control-node placement. Reuse the gate and receipt definitions in the [`/gal-pipeline` command](../commands/gal-pipeline/SKILL.template.md). This section defines ownership only:

- **Gate Scope**: Binary-owned semantic write-back applies to in-scope dispatched `audit` and `test` phase combinations. Out-of-scope combinations and direct execution retain existing receipt-existence and direct placement rules.
- **Receipt Production (Executor Delivery)**: For in-scope dispatches, the specialist executor (`golem-auditor` or `golem-tester`) owns writing the complete task-scoped Markdown subsection (`### [T-NN] YYYY-MM-DD`) into the current-run receipt file (`<task>-<phase>.receipt.md`). The heading must be the first line; metadata or any other preamble before it is invalid. Dispatched in-scope executors must deliver via receipt and must not edit the execution prompt directly. Executor terminal state `completed` represents receipt delivery only, not semantic placement.
- **Semantic Placement (Control-Node Placement)**: The control node (`gal` binary dispatch runner) owns validating receipt payload structure and performing deterministic placement (semantic commit) into the execution prompt (`.dev/plans/<slug>.prompt.md`). It appends the task-scoped subsection under `## Review Results` for audit or `## Test Results` for test. The generated phase instruction names the receipt path and destination section.
- **Skip-Path & In-Conversation Write-Back**: In-conversation specialist runs, non-dispatched executions, and skip-path write-backs (e.g. skipped phases) remain ORCHESTRATOR-owned.
- **Cross-File Convergence**: The pipeline ORCHESTRATOR owns convergence between the source plan, execution prompt, and `.dev/state.md` after all phase gates pass. Guarded projection uses the in-root journal and recovery order above. Legacy projection layout remains unchanged.

## Pipeline Guard Mechanization

Continuation authority is a single evidence path:

```text
coding workflow contract
        │ defines vocabulary and precedence
        ▼
gal-pipeline skill
        │ invokes the checker at convergence and before voluntary final output
        ▼
pipeline-handback-check receipt
        │ sole authority for continuation, handback, retry, stop, or finalization
        ▼
one literal continue_action, or an authorized terminal response
```

The workflow, the `gal-pipeline` skill, and `pipeline-handback-check` MUST use the same closed vocabulary. The receipt tuple is exactly `(decision, reason, voluntary_response_authorized, continue_action)`:

- `decision`: `continue`, `goal-verified`, `human-required`, `retry-ceiling`, or `stop-at`.
- `reason`: `none`, `security-protected-path`, `goal-gaps-blocked`, `head-drift`, `boundary-scope-decision`, or `convergence-human-repair`.
- `voluntary_response_authorized`: a literal boolean; it is true only for `goal-verified`, `human-required`, `retry-ceiling`, and a reached, re-proven `stop-at`.
- `continue_action`: exactly one closed control-plane action under `decision: continue`; it is the literal `none` under every terminal decision. The forms are `/gal pipeline <prompt> from <task> [stop-at <target>]`, `run-goal-backward-verification`, `repair-stop-at-target <target>`, or `repair-stop-at-convergence <target>`. A narrative status, model judgment, runtime cutoff, interruption marker, or unchanged checker self-loop is never an action authority.

| decision | authorized | action |
| --- | --- | --- |
| `continue` | false | Execute the one closed `continue_action`. |
| `goal-verified` | true | Verify the receipt in this invocation, then enter `/gal finalize` Step 1 (path B) after the Owner Acceptance check. Return `LANDED` only with finalize evidence, else the exact stop and the `/gal finalize` resume action. A read-only terminal-reverify never chains into finalize. |
| `human-required` | true | Stop and return the receipt-backed handback. |
| `retry-ceiling` | true | Stop and return the receipt-backed handback. |
| `stop-at` | true | Stop and return the receipt-backed decision. |

Typed human-required producers map to the only reasons they may emit:

| Producer | Reason | Phase | Required handback decision |
| --- | --- | --- | --- |
| `AUDITOR` | `security-protected-path` | `AUDIT` | `human-required` |
| `VERIFY` | `goal-gaps-blocked` | `VERIFY` | `human-required` |
| `PIPELINE` | `head-drift` | `CONVERGE` | `human-required` |
| `BOUNDARY` | `boundary-scope-decision` | `BOUNDARY` | `human-required` |
| `CONVERGE` | `convergence-human-repair` | `CONVERGE` | `human-required` |

Each `human-required` receipt is bound to exactly one `OPEN` block under `### Handoff Notes` headed `#### Human Handback — <reason>`; use the field format (including the `What to check`, `Expected result`, and `Pass/fail rule` checklist fields, all required non-empty) and single-OPEN rules in [conventions/handoff-notes.md](../conventions/handoff-notes.md). The checker ignores historical `RESOLVED` blocks when enforcing the single-OPEN rule. When the issue clears, mark that block `RESOLVED` rather than creating a second live block.

No field of a handback receipt may name an action inside the orchestrator's action space in permission grammar. `continue` is the sole live row by design, and its `continue_action` field is the only executable value; check any future vocabulary addition against this rule.

All plan-scoped evidence uses one topology. The canonical execution-prompt path is the scope root; plan receipts live under `.dev/pipeline/<plan-slug>/`, task logs live under `.dev/pipeline/<plan-slug>/<task>/`, and `resolve_receipt_path(Some(prompt), <filename>)` derives the receipt path from that root. The handback receipt binds prompt path/hash, current Git HEAD, checked/unchecked task projection, prompt-side current-task cursor, and evidence path. `Current Task` is a canonical prompt-side gate and must be cleared before final authorization; it is not duplicated in the goal record. `Workflow` is advisory prompt metadata and is not a goal-record field or binding. The closed goal-record schema is matching `prompt_path`, `prompt_sha256`, `head`, `checked_tasks`, `verdict: VERIFIED`, at least one continuous non-empty `must_have_1..N` sequence starting at 1 with no gaps or duplicates, and at least one non-empty `command`. Generic `evidence` is advisory and never counts as a must-have. Chat prose and foreign, stale, malformed, incomplete, or unbound records cannot satisfy the binding. This is a cooperative control-plane protocol: the receipt is ORCHESTRATOR-emitted durable evidence for freshness and state consistency, not cryptographic provenance against another writer with equal repository authority.

Classification precedence is fixed: reached and re-proven `stop-at`; retry ceiling; typed producer plus one valid handback; incomplete or invalid state as repair `continue`; pending valid future `stop-at` as next-task `continue` with the target preserved; all tasks checked without a fresh bound goal record as `run-goal-backward-verification`; fresh checked completion with a bound `VERIFIED` goal record as `goal-verified`. An unreached `stop-at` target does not suppress retry or human-required checks. On `continue`, execute its one literal `continue_action` in the same invocation. `run-goal-backward-verification` is consumed by the ORCHESTRATOR in-process and is not a public CLI subcommand; the checker only validates freshness/state consistency and never performs semantic verification. On an authorized terminal decision, return only the receipt-backed terminal response.

Interruption is recovery-only. A runtime cutoff writes or refreshes an `Interrupted Phase` marker and a rerunnable resume marker, but it cannot authorize final output, schedule a wake-up, or replace a missing receipt. Working-hours policy is an execution scheduling exemption, not continuation authority: `/gal pipeline` remains governed by its explicit exemption, while every decision still requires the fresh checker receipt. Terra is an acceptance baseline for behavior and tests; it is not a routing policy and does not grant authority to any executor or model.

### File Ownership Rules

- `.dev/plans/<slug>.md` is the planning-stage source plan and human-readable task checklist. Planning commands and planning review lanes may update its full content; `/gal pipeline` may update task checkboxes and commit notes after a task passes implement, the orchestrator task check, test, and audit.
- `.dev/plans/<slug>.prompt.md` is the execution-stage work file. `## Status`, retry counters, handoff notes, task commit markers, `## Test Results`, `## Review Results`, `## Analyze`, and detailed task execution history belong here.
- `.dev/state.md` is the repo-level active-plan index and session-continuity surface. `/gal pipeline` updates it after each completed task so `/gal status` and `/gal whats-next` resume from the same place a human sees in the source plan.
- For in-scope dispatches, specialist executors write detailed phase results to current-run receipts, and the control node validates and places them into the execution prompt. Pipeline-bound test and audit receipts must begin on the first line with `### [T-NN] YYYY-MM-DD`; metadata or any other preamble before that heading is invalid. For out-of-scope, non-dispatched, or in-conversation runs, prompt updates and cross-file convergence between source plan, execution prompt, and `.dev/state.md` are owned by the pipeline orchestrator.
- If `/gal status` or `/gal whats-next` sees a live workflow phase without the expected durable markers in `.dev/plans/<slug>.prompt.md`, or sees source-plan and prompt task checkboxes disagree, treat that as missing execution write-back rather than as a cleanly completed phase.

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

1. **refining** drafts `## Tasks` / `## Test Plan` / `### Engineering Review`, applying the Atomicity Rubric (a)–(e) and the quantitative split triggers (see `commands/refining-plan`). Tasks are behavior-sized, and every test point is executed by an agent. Checks only a human can perform go to `## Owner Acceptance`, and environment checks go to `## Preconditions`.
2. **Definition-of-Ready dual lens** (Three-Amigos; **review lens ≠ refiner**, cross-model when practical):
   - **architect lens** — structural atomicity, blast radius, dependencies, rename+move bundling, feasibility → APPROVE / REVISE.
   - **tester lens** — every task has a minimal, reproducible, observable acceptance probe (the test **contract**, spec-layer not code; this is rubric (e)'s oracle); tighten `TP-NN`, name the evidence shape → APPROVE / REVISE.
   - **analyst lens (conditional)** — value lens when business rules / permissions / pricing are touched.
3. **STAGE 3.6 convergence (dual-owner)** — **STEWARD** converges documentation structure (`.dev/plans/` doc well-formed / naming / no orphan-or-duplicate / figure sync); **ORCHESTRATOR** converges `.dev` execution state (`state.md ↔ .dev/plans` agreement, lifecycle markers).

**Stop-line (jidoka):** any REVISE, or non-convergence, returns to refining or `/deep-planning` — defects are fixed at the source, not waved through.

**Exit condition for readiness review (conjunction — all required):** `<!-- ENG_REVIEW: CLEAR -->` ∧ dual-lens APPROVE ∧ STAGE 3.6 converged (docs + `.dev`).

**Exit condition for `/plan-to-prompt`:** readiness-review exit condition ∧ `## Approval > Human approval = [approved]`.

**Depth-scaling:** an obvious local fix may collapse the loop to "rubric self-check + light convergence". Structural / multi-file / protected-path changes require the architect + tester dual sign-off. A single execution plan must stay **≤ 99 blocking tasks** — over that, split the plan (see `commands/refining-plan`).

**Depth-scaled diagrams:** structural / multi-file / Protected-Path plans include a `## Diagrams` section (text flowchart or architecture diagram) that aids comprehension. Obvious local-fix plans may omit it. The STAGE 3.6 STEWARD gate checks that any diagram is in sync with the finalized `## Tasks` scope.

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
  /gal pipeline ─► task check → implement → tester → auditor → commit → … → orchestrator goal-verify
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

Session safety is agent-internal discipline, not a public command family.

- `golem-debugger` owns freeze-style scope control during investigations
- destructive shell operations require explicit caution and user clarity
- session safety has no public slash commands or workflow steps

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
| **architect** | Directly callable | `MODE: direct`; isolated default or in-context via standalone command with `discuss` argument |
| **analyst** | Directly callable | `MODE: direct`; auto-activates in `/deep-planning` for business-rule content; dual-mode |
| **designer** | Directly callable | `MODE: direct`; auto-activates in `/deep-planning` for UX/UI content; dual-mode |
| **releaser** | Directly callable | `MODE: direct`; planning-stage release-flow designer; isolated default or in-context via standalone command with `discuss` argument; emits design advice, does not execute |
| **debugger** | Directly callable | `MODE: direct`; isolated only — no `discuss` support; always directly callable |
| **steward** | Directly callable | `MODE: direct`; isolated only — no `discuss` support; callable via `/gal steward` |
| **implementer** | **Orchestrated-only** | Only via `/gal pipeline` (pipeline-phase context); bare call → `COMMAND: error` |
| **tester** | **Orchestrated-only** | Only via `/gal pipeline` (pipeline-phase context); bare call → `COMMAND: error` |
| **auditor** | **Orchestrated-only** | Via `/gal pipeline` (task audit) or `/gal finalize` (whole-branch audit); bare call → `COMMAND: error` |
| **researcher** | **Orchestrated-only** | Only via `/gal research` or `/gal deep-research`; bare call → `COMMAND: error` |

### Consult Dual-Mode

Four of the six specialist roles (architect, analyst, designer, releaser) have standalone commands that accept `discuss` and can run in two conversation modes. Debugger has no standalone command, and steward's standalone command is isolated-only:

| Mode | Trigger | What happens | Response label |
| --- | --- | --- | --- |
| **Isolated** (default) | `/gal <role>` | Native subagent runs role in isolation; only verdict/summary returns to main context | `[<role> · isolated]` |
| **In-context** | Standalone role command with `discuss` argument | The standalone role command dispatches through `gal consult-script <role>` and loads role instructions into the current conversation. | `[<role> · in-context]` |

**Hot-join**: the isolated verdict is already in the transcript, so in-context mode continues from there without re-running the role from scratch.

### Invocation Table

| Type | Allowed | Examples |
| --- | --- | --- |
| **Directly callable — isolated** | Yes — read-only advice, no phase transition | `/gal architect`, `/gal analyst`, `/gal designer`, `/gal releaser`, `/gal debugger`, `/gal steward` |
| **Directly callable — in-context (discuss)** | Yes, four roles only — loads activation-core into main context | Claude Code: `/<role> discuss`; plugin mode: `/gal:<role> discuss`; Codex: `$<role> discuss` |
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
2. **CODER and TESTER should differ when practical** — dispatch isolation provides independent verification, while a different model strengthens it
3. **AUDITOR should differ from CODER** — fresh perspective catches blind spots
4. **AUDITOR should also differ from TESTER when practical** — audit is a higher-level check than test generation
5. **DESIGNER should differ from CODER when used as a formal review lane** — keep experience review independent from implementation
6. **RESEARCHER's reference verification should differ from the research author when practical** — the three parallel isolated workers already provide independent verification, while a different model strengthens it further
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
4. TEST → Prefer a different model when practical (isolated dispatch provides baseline independence) — feed it plan + public interfaces only; this is usually basic unit/integration coverage
5. AUDIT → Prefer a different model again when practical — deep performance and security; this should be a higher-level check than TEST
6. VERIFY → Run full test suite, confirm all plan items implemented
```

Research workflow note: `/gal research` and `/gal deep-research` have their own VERIFY state for citation checking. Research independence rests on RESEARCH, SYNTHESIZE, and CROSS-REVIEW each running across three parallel isolated workers, blind to one another. Using a different model from the research author for the VERIFY pass strengthens that isolation further, but is not the only source of independent verification.

### Per-Phase Rules

- **The planning author and architect should differ when practical** — cross-check the plan before implementation
- **The planning author and designer should differ when designer is a formal review lane** — keep design critique independent from plan authorship
- **Implementer and tester should differ when practical** — dispatch isolation provides baseline independence, while a different model strengthens verification
- **Auditor should differ from implementer** — fresh perspective
- **Auditor should also differ from tester when practical** — audit should be higher-level than test generation
- **Auditor model tier ≥ implementer model tier** — the auditor must be at least as capable
- **Auditor model tier ≥ tester model tier** — audit should be at least as capable as test generation and usually stronger

### Checking-Role Triangle

Checking responsibilities form a triangle, with documentation lifecycle support:

- **ORCHESTRATOR** — task check, dispatch, return-versus-plan reconciliation, **end-of-run goal-backward verification**, and plan lifecycle (ABSORBED / delete). Goal-backward verification always runs in-process; it is never dispatched.
- **TESTER** — isolated subagent that runs the task's planned tests against the committed implementation; it does not see the CODER or AUDITOR return.
- **AUDITOR** — isolated subagent that independently audits **within a single task** (deep performance + security); it does not see the CODER or TESTER return and does not own goal, cross-task, or lifecycle decisions.
- **STEWARD** — documentation structure: end-of-run knowledge extraction → `docs/`, doc drift, structural hygiene.

Every dispatch spec is assembled from the plan task and the phase's own agent contract, never from another role's return, handoff, or receipt.

```text
══════════════════════════════════════════════════════════════════════
  ORCHESTRATOR              TESTER                   AUDITOR
  task check + reconcile    isolated task tests       isolated single-task audit
  + goal-backward +         (does not see coder      (deep perf + security,
  lifecycle                 or auditor return)       does not see other returns)
══════════════════════════════════════════════════════════════════════
  STEWARD: documentation structure (knowledge extraction → docs/,
  doc drift, structural hygiene)

per task T-NN:
  ORCHESTRATOR ── 2b task check → dispatch implement → CODER
       │ reconcile CODER return against the PLAN
       ▼
  ORCHESTRATOR ── 2d implementation commit + dispatch test → TESTER
       │ reconcile TESTER return against the PLAN
       ▼
  TESTER ── 2e task tests (isolated subagent)
       ▼
  AUDITOR ── 2f single-task deep audit (isolated subagent)
       │ BLOCKING/security/protected → fix or STOP
       ▼
  ORCHESTRATOR ── reconcile audit return + 2g converge   ↺ next task
       │ (all tasks done)
       ▼
  ORCHESTRATOR ── end-of-run goal-backward verification
       │   does the whole result actually achieve the plan GOAL? (task done ≠ goal met)
       │   VERIFIED → continue │ GAPS_FOUND / BLOCKED → STOP
       ▼
  ORCHESTRATOR ── lifecycle: mark ABSORBED + delete plan files (post-finalize)
```

**Verify-independence policy.** End-of-run goal-backward verification is **ORCHESTRATOR-owned and always in-process**. TESTER and AUDITOR remain isolated subagents; each receives only the plan task and its own agent contract, not another role's return, handoff, or receipt.

The checklist's "naming conventions" item is governed by the project-wide naming authority — `conventions/naming.md` (full term registry in `docs/glossary.md`): reserved words (bare `agent` = golem agent, `model` = LLM), qualified overloaded terms, no generic bucket names, and provenance discipline (plan-task IDs and migration narration only under `.dev/**`). The naming gate enforces provenance + retired terms at pre-commit and pipeline closeout.

> **Executor routing**: machine-read per-role CLI assignment lives in `config.json#executorRouting`. See [docs/configuration.md — Executor Routing](../../../docs/configuration.md#executor-routing-executorrouting) for the `executorRouting` subtree shape to paste into `config.json`.

## Plan Finalization (`/gal finalize`)

Normal `/gal pipeline` lands a verified plan with no second routine approval. After end-of-run goal-backward verification (VERIFIED) yields a fresh `goal-verified` receipt, the orchestrator runs `/gal finalize` as the last node (path B). Standalone `/gal finalize` stays the explicit entry and the recovery command (path A). Human acceptance is separate from landing authority: before substantive finalize work, outstanding `## Owner Acceptance` rows are shown verbatim and finalize stops with `Interrupted Phase — finalize / ACCEPTANCE`. No section, or no outstanding row, needs no approval. Explicit owner acceptance or waiver of every outstanding row, recorded in `#### Owner Acceptance Evidence`, permits continuation. Pipeline startup is never acceptance evidence. Before the first task, the pipeline stops when a `## Preconditions` check does not match its expected result. `/gal finalize` is the gated, interruptible/resumable **completion command** that takes a verified plan the rest of the way: one top-down four-layer review, dispatched once to the AUDITOR route (in-process only when that route is genuinely absent) → doc-sync → (on a worktree/feature branch) merge-to-main + teardown → bounded `.dev/state.md` write + a distinct post-write hygiene-only receipt → lifecycle close → last-good tag. This is the canonical flowchart; `docs/workflows.md` and `README.md` link here rather than redrawing it.

```text
/gal pipeline ──► all tasks [x] + ORCHESTRATOR goal-backward VERIFIED + fresh verified handback receipt  (terminal-reverify stops here, never chains)
      │ path B (same invocation)                 path A: owner's own finalize request ──┐
      ▼                                                                                   ▼
/gal finalize  entry condition: pipeline-handback-check receipt carries decision: goal-verified ∧ voluntary_response_authorized: true (two fields only, never head-compared; a stored receipt, status line, or prior final response never starts finalize)
      ▼
Owner Acceptance check (## Owner Acceptance + #### Owner Acceptance Evidence; before any substantive work)
  no table / no rows / every row accepted or waived → continue, no approval prompt
  any row outstanding → show rows verbatim, `Interrupted Phase — finalize / ACCEPTANCE`, STOP, resume with `/gal finalize`
      ▼
  full-mode `gal finalize-check` receipt — 9 repo-level rows in a GAL source repo, 7 downstream, none per task:
    authoritative-command · naming-gate · sync-idempotency · finalize-mode · project-source-doc-existence ·
    state-bound · contract-roster-parity · doc-link-resolution · working-tree-clean
      │ miss → HARD-BLOCK (no action): fix the named repo-level row, then re-run the gate
      ▼
  1. Sequence 1 — top-down zero-trust review (whole branch, once): `gal pipeline <prompt> --phase finalize-review`, executor from the AUDITOR route, `Review Independence: full`
       reason=no-routing (route genuinely absent) → in-process, `Review Independence: DEGRADED_SAME_RUNTIME`; any other failure → STOP + `Interrupted Phase — finalize / REVIEW`, no fallback
       four layers per requirement: L1 Truths (diff hunk + test name, or hunk + manual-probe output when every Test Plan row is Type manual)
         · L2 Files (path + non-stub proof) · L3 Wiring (cross-task seams, doc-vs-code, scope drift) · L4 Trust boundaries (one STRIDE row per new input/write/exec/path/call)
       write-back: `### Finalize Review <date>`, a `Requirement × L1 | L2 | L3 | L4` table, findings with severity + blocking: yes|no, one `Review Independence:` line
         cell grammar: each cell is `PASS — <evidence>` | `FAIL — <evidence>` | `N/A — <reason>`, text after the separator never empty, L1 never `N/A`, no row all-`N/A`
       ORCHESTRATOR is NOT a review owner (it owns entry, dispatch, doc-sync, Git, lifecycle, and final close)
       any empty cell → reviewer failure: re-run Sequence 1, no implementer dispatched
       REVISE → fix (implementer) → re-review clean → commit on branch
      ▼
  2. doc-sync (in-process, orchestrator under the STEWARD contract; semantic document review first): STEWARD → durable layer (README.md + docs/ excl. plans/+research/); then re-index .dev/project.md; re-run full-mode `gal finalize-check`, whose read-only `sync-idempotency` row renders both adapters twice in memory (byte-identical ×2); drifted>0 → `gal render-adapters`, then re-check
      ▼
  3. recheck accepted targets (material change → renewed acceptance of the affected rows, STOP) → (worktree/branch) merge into main — plain `git merge` (no squash / ff-only / rebase); conflict → STOP + handoff, except an unmerged path set of exactly {.dev/state.md}: `gal state-merge` resolves deterministically (exit 0 → continue; `STATE_MERGE: unresolved` → STOP, state proven intact; `STATE_MERGE: rollback-unconfirmed` → STOP, possible mutation named); an unmerged path set of exactly {.dev/project.md} or {.dev/project.md, .dev/state.md}: take theirs for .dev/project.md, re-run STEWARD's reindex, resolve .dev/state.md via `gal state-merge` when present, then continue (reindex failure or budget overrun → STOP); every other conflict shape keeps the unconditional STOP
      ▼
  4. (worktree/branch) teardown — plain git; already-on-main: the accepted-target recheck runs before teardown/deletion (`git worktree remove` + `git branch -d`); cross-runtime
      ▼
  5a. bounded state write — remove exact-predicate legacy comments from .dev/state.md; upsert + trim ## Recent Close-outs to <=2 rows (Landing = durable-layer commit); upsert every `blocking: no` finding from `### Finalize Review <date>` into ## Follow-ups, newest first, trimmed to <=5 rows
      ▼
  5b. post-write hygiene-only receipt (`gal finalize-check --hygiene-only`, distinct default path) — universal rows `project-source-doc-existence`, `state-bound`, `durable-layer-commit`, `finalize-review-shape`, plus `contract-roster-parity`/`doc-link-resolution` when applicable, ending with `working-tree-clean`; every applicable row must pass
      │ fail → STOP: keep plan files, repair the named drift (a `finalize-review-shape` miss re-runs Sequence 1, never the code), re-run
      ▼
  5c. lifecycle close — ORCHESTRATOR: ABSORBED + delete plan files [gated on durable-layer commit AND hygiene-only receipt pass]
      ▼
  6. `git tag -f gal-last-good <landing-commit>` — advance the known-good marker; non-fatal if it fails
```

**Provider-Memory Harvest at finalize.** Step 2 (doc-sync) is pointer-only: STEWARD may consider provider-memory candidates already approved in the plan's `### Handoff Notes` (see Context Handoff above), but finalize performs no provider acquisition of its own. Approval is never itself promotion evidence — promotion still requires the same verified-root-cause-plus-credible-recurrence gate as any other task-to-project promotion (`conventions/token-budget.md`); a gate miss omits the candidate without blocking finalize.

### Zero-Trust Gates (mechanized — finalize trusts evidence, not self-reports)

finalize re-establishes facts rather than trusting recorded claims. Six gates carry this, mirrored in the `gal-finalize` SKILL:

1. **Entry condition = a bound start, plus two fields off the pipeline's handback receipt, then the Owner Acceptance check and the repo-level row inventory — never the prompt's recorded `VERIFIED`, and never the receipt alone.** Path A is the owner's own finalize request. Path B is normal pipeline completion with a receipt produced and verified in that same invocation. The receipt half of Step 1 reads only `decision: goal-verified` and `voluntary_response_authorized: true` from `.dev/pipeline/<plan-slug>/pipeline-handback-check.receipt.md`, and never compares its `head` to current HEAD; that receipt half is necessary but not sufficient, because finalize is never started from a stored receipt, status line, or prior pipeline final response alone. The deterministic `gal finalize-check` full-mode receipt then re-runs the repo's authoritative checks (declared in `.dev/project.md` `<!-- gal:authoritative-check -->`) and the row inventory: `authoritative-command`, `naming-gate`, `sync-idempotency`, `finalize-mode`, `project-source-doc-existence`, `state-bound`, `contract-roster-parity`, `doc-link-resolution`, `working-tree-clean` — nine rows in a GAL source repo, seven downstream (no `plugins/gal-core/`), none per task. Any failing/not-run row → HARD-BLOCK with the real command output. Run every gate through the guarded-entry executable and verify each stored pass receipt with `doctor --verify-receipt <path> --expect-scope <scope>` before consuming it. Follow [self-bootstrap](../conventions/self-bootstrap.md) for immutable worktree generations, coordinator binding, and package-manager ownership.
2. **No recorded verdict is a review input.** Sequence 1's forbidden inputs are every receipt under `.dev/pipeline/<plan-slug>/`, every verdict line in `## Test Results` and `## Review Results`, `## Status`, and the verdict lines of `### Handoff Notes` — the Step 1 entry-signal receipt is read before Sequence 1 begins and is not itself an input to the review. The reviewer derives every must-have itself from the plan's `## Goal`, `## Requirements`, `## Success Criteria`, `## Test Plan`, the branch diff, the working tree, and one live authoritative-command run. `finalize-review-shape` mechanizes the resulting write-back's shape — row count, cell grammar, the `Review Independence:` line, and L1 test-name resolution — at the post-write hygiene-only receipt, not at Sequence 1 itself.
3. **doc-sync is mandatory, never skipped for "drift risk".** The read-only `sync-idempotency` row in `gal finalize-check` renders both adapters twice in memory and asserts determinism without writing. doc-sync lands durable knowledge, then re-indexes `.dev/project.md`, then re-runs full-mode `gal finalize-check` so the row reflects the reindexed content. When that row reports `drifted>0`, `gal render-adapters` regenerates the on-disk adapters before a fresh re-check. A non-deterministic render, or a render rejected on the size budget → STOP.
4. **Independence by input rule, with a named degradation.** Sequence 1 runs in an independent executor chosen by the AUDITOR route and records `Review Independence: full`. Fresh context claims neither a different model nor filesystem isolation, so independence still comes from never reading a forbidden input (gate 2). Only a genuinely absent AUDITOR route (`reason=no-routing`) permits the in-process review, which records `Review Independence: DEGRADED_SAME_RUNTIME`. Unreadable or malformed config, a malformed selected route, an unavailable executor, launch failure, timeout, incomplete evidence, or an invalid payload stops finalize with no fallback. The full four-layer table is required either way, and a dispatched independent reviewer is never claimed when none ran.
5. **Evidence-aligned report.** Every `--- FINALIZE COMPLETE ---` line maps to a proving evidence pointer; no-evidence lines may only read `skipped`/`DEGRADED`/`not-run`, never `done`/`CLEAR`/`LANDED`.
6. **Post-write hygiene-only receipt closes the full time-of-check window.** The Step 1 entry-signal check runs before Sequence 1's review, doc-sync, and merge — drift any of those introduce (including drift from the merge itself) would otherwise reach plan-file deletion unchecked. A second, distinct `gal finalize-check --hygiene-only` receipt runs after the bounded `.dev/state.md` write and before deletion, naming `project-source-doc-existence`, `state-bound`, `durable-layer-commit`, and `finalize-review-shape` as universal rows (plus `contract-roster-parity`/`doc-link-resolution` when applicable, ending with `working-tree-clean`); any applicable row failing it is a STOP that keeps the plan files, never a silent pass-through, and a `finalize-review-shape` miss re-runs Sequence 1, never the code.

### Finalize Failure Recovery and Terminal Reverify

A failed full-mode finalize gate is diagnostic only. It authorizes no mutation, evidence deletion, prompt rewrite, commit, merge, lifecycle close, or relabeling of the failure as ordinary unfinished work. Read the failing rows, classify the failure class, and take exactly the corresponding recovery route:

| Failure class | Recovery route |
| --- | --- |
| Authoritative command/tool failure | Create or resume a separate remediation plan; remediation owns the mutation and must produce a committed clean state. |
| Results-structure failure or unresolved citation failure | Create or select a bounded evidence-remediation plan; repair only the named evidence contract, then run a fresh gate. |
| Stale goal binding with otherwise clean, passing state | Enter `terminal-reverify`; do not send the plan back through ordinary implementation. |
| Genuinely unchecked work with a non-`DONE` workflow | Resume the ordinary `/gal pipeline` route. |
| `DONE` with unchecked or contradictory state | Treat as terminal corruption; stop and create the required human handback. Never relabel it as ordinary work. |

The canonical recovery state machine is:

```text
failed full gate
      │ finalize records diagnostics only; no action on the failed gate
      ▼
classified failure class
      │ authoritative/evaluation ───────────────► remediation plan
      │ results structure/citations ────────────► evidence-remediation plan
      │ stale goal binding, clean passing state ─► terminal-reverify
      │ unchecked non-DONE work ─────────────────► ordinary pipeline
      │ DONE + contradictory state ──────────────► terminal corruption handback
      ▼
committed clean state
      ▼
terminal-reverify (closed, read-only precondition lane)
      ▼
fresh orchestrator goal-backward verification and fresh handback binding
      ▼
rerun full-mode finalize
```

The authority split is binding: **finalize is diagnostic-only**; **remediation owns mutation**; **the ORCHESTRATOR owns semantic verification** and must write a fresh bound goal record; and **pipeline-handback-check owns binding** of prompt bytes, Git HEAD, checked-task projection, cursor state, and goal evidence. No role may substitute narrative status for the receipt owned by the next authority.

`terminal-reverify` is available only for a clean, committed state whose prompt is `DONE`, all tasks are checked, the current-task cursor is cleared, and no retry, interruption, or human handoff remains open. It authorizes only the ORCHESTRATOR's in-process goal-backward verification followed by a fresh handback check, rendering prompt path, prompt SHA-256, `HEAD`, mode, and overall verdict deterministically into a bindable terminal receipt. It must not dispatch implementation, testing, auditing, security work, or remediation; mutate the prompt or plan; create a commit; or perform finalize lifecycle actions. After the fresh handback is authorized, rerun full-mode finalize so its diagnostics observe the repaired and rebound state. Terminal-reverify never chains into finalize, so that rerun is an explicit `/gal finalize` (path A). Never restart the ordinary pipeline on a `DONE` prompt.

### Top-Down Review Stage (cross-task, whole-branch)

Per-task audit passing does **not** prove the combined branch is coherent. finalize closes that gap with a single top-down, requirement-by-requirement review over the whole branch diff, run once, in an independent executor selected by the AUDITOR route through `gal pipeline <prompt> --phase finalize-review` (in-process under `DEGRADED_SAME_RUNTIME` only when that route is absent) — **no new agent and no delegation to `golem-architect`**:

- **L1 Truths** — the requirement is an observable behaviour: one diff hunk plus one test name reaching it through its production caller, of the `## Test Plan` row's declared Type or higher; when every covering row is Type `manual`, one diff hunk plus the live output of that manual probe instead.
- **L2 Files** — what must exist for L1 exists and is not a stub: a path plus the non-stub proof.
- **L3 Wiring** — what must be connected is connected and reachable: cross-task seams, documentation-versus-code checks, `.dev/project.md` and adapter freshness, and scope drift against `## Requirements`.
- **L4 Trust boundaries** — every new input source, file write, command execution, path derivation and external call the branch introduces, each its own row with one STRIDE judgment.

Top-down review ≠ goal-backward verification: the latter is ORCHESTRATOR-owned and is finalize's **precondition**, not part of its review stage.

### Lifecycle-Delegation Boundary

finalize holds **zero** lifecycle authority — it only sequences existing owners:

- **ORCHESTRATOR** owns the goal-backward verify (precondition), Git operations (commit, merge, teardown, tag), and lifecycle close: ABSORBED marking + plan-file deletion + `.dev/state.md` close-out. The independent reviewer never commits, merges, tags, deletes plans, or edits the prompt.
- **STEWARD** contract owns knowledge extraction → durable layer (`README.md` + `docs/`) and provides the **durable-layer commit hash** that gates plan-file deletion. The orchestrator runs it in-process, with semantic document review first. Mechanical checks do not prove document correctness.

finalize itself deletes nothing, extracts nothing, and re-verifies nothing.

### finalize ↔ wrap-up Boundary

- **`/gal wrap-up`** = non-destructive session **pause**, callable anytime; it compresses handoff notes and updates session continuity so any session/machine can resume. It does not land or delete anything.
- **`/gal finalize`** = one-shot, gated, **destructive completion landing** (merge + plan-file deletion) for a plan that is actually done.

They reverse-prompt each other (wrap-up in a completed-plan state offers finalize) but never overlap: pausing is wrap-up, landing is finalize.
