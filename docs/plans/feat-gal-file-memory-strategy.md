# Plan: GAL Unified File Memory Strategy

## Goal

GAL has one authoritative file-system memory model that works across sessions, projects, providers, chat interactions, and implementation agents. Copilot, Antigravity CLI, Codex, Claude Code, and future runtimes must all recover and update the same repo-owned Markdown memory without relying on provider-local chat history, external memory frameworks, vector stores, databases, or hand-edited generated adapters.

## Requirements

- [ ] Cross-session memory must be recoverable from repo files alone: `.dev/state.md`, `.dev/plans/<slug>.prompt.md`, and source plan handoff context must let a new session resume without prior chat history.
- [ ] Cross-provider memory must use file handoff, not provider memory: Provider A writes repo state through GAL workflow files, then Provider B reads the same cold-start stack and continues from the same facts.
- [ ] Cross-project memory must be explicit and bounded: project facts stay in each target repo, while reusable GAL methodology is promoted only through GAL source docs, conventions, workflows, templates, commands, and agents.
- [ ] Chat interactions and implementation agents must use the same memory substrate: normal AI chat, `/gal status`, `/gal whats-next`, `/gal pipeline`, and golem specialists all read the same cold-start priority and write back only to owned GAL files.
- [ ] This plan must treat GAL as already Remi-like at the storage layer: repo-owned Markdown remains the default memory substrate, and the work focuses on stronger operations, tiering, and consolidation rather than replacing storage.
- [ ] The strategy must add a lightweight memory-operations contract for the core actions GAL actually needs: retrieve, encode, summarize, promote, and prune.
- [ ] The strategy must add a low-overhead learning loop so recurring errors become durable lessons: mistakes found during implementation, review, test, or debugging must be eligible for promotion from task memory to project memory, then to shared GAL methodology when evidence shows they are reusable.
- [ ] The strategy must improve accuracy over time without background daemons or heavy indexing: lesson consolidation must be event-triggered and file-based.
- [ ] GAL memory must remain local, file-system based, versionable, reviewable, and aligned with the existing Document-driven / No Database architecture.
- [ ] The strategy must explicitly reject Mem0 mode and Mem0-like middleware patterns for core GAL state: no dual-store vector/graph stack, no memory middleware service, no database-backed semantic layer, and no hidden retrieval tier unless a future architect-reviewed plan changes the architecture.
- [ ] The strategy must explicitly reject Zep, MemGPT-style external frameworks, provider-local memory APIs, long-running memory daemons, vector stores, and databases as core GAL state unless a future architect-reviewed plan changes the architecture.
- [ ] Generated runtime adapters must remain derived outputs from source documents and setup scripts; they may carry the memory contract to providers but must not become the source of truth.
- [ ] Privacy boundaries must be explicit: repo memory stores repo-relevant context only, never secrets, private journal material, machine-local paths, or personal memory that belongs in local config or Obsidian.

## Approach

### Memory Model

This plan treats memory as a file ownership contract, not as a new service.

| Memory scope | Authoritative location | Written by | Read by | Boundary behavior |
| --- | --- | --- | --- | --- |
| Shared GAL methodology memory | `conventions/`, `workflows/`, `commands/`, `agent/`, `templates/`, durable docs | Reviewed source changes | Generated adapters, command skills, chat agents, golem specialists | Cross-project by design; these are reusable GAL rules, not project facts |
| Project durable memory | `.dev/project.md`, selected long-lived docs | Planning, verification, release, explicit documentation updates | All providers and agents during cold start | Cross-session and cross-provider inside one repo; cross-project only after deliberate extraction into shared GAL methodology |
| Project session memory | `.dev/state.md` | `/gal wrap-up`, planning commands, control-plane state updates | `/gal status`, `/gal whats-next`, chat agents, golem specialists | Keeps active plan, blockers, and next step resumable across sessions and providers |
| Task execution memory | `.dev/plans/<slug>.prompt.md` `## Status`, `### Handoff Notes`, tasks, analyze, test, review, debug sections | `/plan-to-prompt`, `/gal wrap-up`, implementer, tester, reviewer, debugger, verifier | Chat and implementation agents | Primary cross-session work memory for an active task |
| Source planning memory | `docs/plans/<slug>.md` | `/planning`, `/deep-planning`, `/refining-plan`, planning review lanes | `/plan-to-prompt`, reviewers, humans | Reviewable source plan; not the mutable execution state after prompt generation |
| Private or machine-local memory | `config.local.env`, `mcp.local.json`, `xmachine.config.json`, user Obsidian vault | User or local-only agents | Only workflows explicitly routed to local/private context | Never authoritative GAL project state; never required for another provider to resume the repo |
| Generated adapter memory surface | `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, runtime command outputs | Sync and setup scripts only | Providers at startup | Carries source memory rules into runtimes; must not be edited as source |

### Reference Positioning

This plan uses the YouTube comparison as a selection filter, not as a requirement to reproduce every framework.

- **Remi baseline accepted**: GAL already behaves most like Remi because its durable memory is repo-owned Markdown, transparent to the user, git-reviewable, and file-addressable.
- **Mem0 mode rejected**: GAL should not add middleware-style memory orchestration, dual-store vector plus graph persistence, or a separate memory service just to imitate Mem0.
- **Core additions accepted**: Keep only the parts that directly improve accuracy at low cost: explicit memory tiering, explicit memory operations, and event-triggered lesson consolidation.
- **Improvement target**: Keep the Remi-like storage layer, then add clearer memory operations, tiering, handoff rules, and lesson consolidation so accuracy improves without materially increasing system load.
- **Performance rule**: Prefer event-triggered file updates and bounded reads over background indexing, heavy retrieval infrastructure, or always-on memory workers.

### Core Learning Loop

The minimum viable learning system for GAL is not a new framework. It is a promotion pipeline over existing files.

1. **Retrieve**: At task start, provider switch, review, test, or debug entry, read the minimum relevant repo memory from the cold-start stack.
2. **Encode**: When a new fact, blocker, decision, or failure is confirmed, write it first to the active execution memory file.
3. **Summarize**: At wrap-up or checkpoint boundaries, compress the active session into `### Handoff Notes` and `.dev/state.md`.
4. **Promote**: When a lesson is verified and likely to recur, move it upward from task memory to project memory, then to shared GAL methodology only if it generalizes across repos.
5. **Prune**: Remove or replace stale, disproven, or superseded lessons so file memory stays trustworthy.

This loop is event-triggered. It must not require a background service, vector index, or always-on worker.

### Promotion And Pruning Gates

Promotion and pruning must be rule-driven, not intuitive.

- **Task -> project promotion gate**: promote only when a lesson has a verified root cause and is likely to recur inside the current repo. Acceptable evidence includes a confirmed debugger finding, a reviewer finding, a failing test with diagnosis, a verifier result, or a repeated manual correction to the same repo workflow.
- **Project -> shared GAL promotion gate**: promote only when the lesson changes GAL methodology rather than one repo's local practice. The lesson must affect a GAL convention, workflow, template, command, agent, or other reusable source contract, and must be reusable across repos rather than tied to one project's implementation details.
- **No guess promotion rule**: hypotheses, temporary workarounds, and unverified explanations must remain in task memory and must not be promoted into durable project memory or shared GAL methodology.
- **Prune gate**: remove or replace a durable lesson when later evidence disproves it, a newer rule supersedes it, the referenced source surface no longer exists, or the lesson has become stale because the owning workflow changed.
- **Promotion cost rule**: promotion is manual, event-triggered, and bounded to existing repo files. No background agent, daemon, semantic index, or automatic bulk promotion is allowed.

### Step 1: Make the memory contract authoritative

- **Files**: `conventions/token-budget.md`
- **What**: Add a File-System Memory Contract that defines each memory scope, owner file, writer, reader, promotion path, and the five core operations: retrieve, encode, summarize, promote, and prune. State that chat agents and implementation agents use the same cold-start stack and that provider-local chat memory is advisory at most.
- **Verify**: A reader can answer where task memory, session memory, project memory, shared GAL methodology memory, private memory, and generated adapter output belong, plus when each core memory operation should run.

### Step 2: Encode cross-session and cross-provider handoff

- **Files**: `workflows/coding.md`, `commands/gal-wrap-up/SKILL.template.md`
- **What**: Clarify that `/gal wrap-up` is the required handoff path before pausing, switching providers, or switching machines. Fix wrap-up wording so source-plan rows prefer `.dev/plans/<slug>.prompt.md` when it exists, because task execution memory lives under `.dev/plans/`, not `docs/plans/`.
- **Verify**: Provider A can stop after writing `### Handoff Notes` and `.dev/state.md`; Provider B can resume by reading repo files only.

### Step 3: Make chat and implementation agents consume the same memory

- **Files**: `commands/plan-to-prompt/SKILL.template.md`, `templates/plan-prompt.md`, `agent/agents.md`
- **What**: Clarify that the execution prompt is the shared mutable work memory for both conversational control-plane actions and golem implementation/review/test/debug agents. Do not create a separate chat memory lane.
- **Verify**: `/gal status`, `/gal whats-next`, `/gal pipeline`, and golem specialists all point at the same active execution prompt and do not depend on provider chat transcript memory.

### Step 4: Define bounded cross-project memory

- **Files**: `.dev/project.md`, `templates/project.md`, `docs/devguide.md` *(follow-on only if the primary contract leaves ambiguity after Steps 1-3 and 5)*
- **What**: Document that each target repo owns its own project memory, while cross-project reuse happens only through deliberate promotion into shared GAL methodology files such as conventions, workflows, templates, commands, agents, and durable docs. Defer these edits unless the primary contract files do not make the boundary clear enough.
- **Verify**: A new project initialized by GAL can explain the same memory stack, but facts from Project A do not silently appear in Project B unless promoted to shared GAL source files.

### Step 5: Add lesson-consolidation rules

- **Files**: `conventions/token-budget.md`, `workflows/coding.md`, `docs/devguide.md`
- **What**: Define the minimum learning loop that reduces repeat mistakes. Specify which findings stay in task memory, which must be promoted into project memory, and which can be promoted into shared GAL methodology. Keep promotion gated by evidence and recurrence, not intuition.
- **Verify**: The docs explain how GAL should get more accurate over time without inventing a new service or writing unverified guesses into durable memory.

### Step 6: Add operator privacy and local-memory guidance

- **Files**: `docs/personalization.md`
- **What**: Explain the boundary between repo memory, machine-local settings, Obsidian/private notes, and provider-local memory. Make clear that secrets, diary content, personal notes, and local paths stay outside tracked GAL memory unless explicitly routed to a private local destination.
- **Verify**: The docs say where private memory belongs and warn against storing private or secret material in tracked GAL files.

### Step 7: Propagate through generated runtime surfaces

- **Files**: `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, generated command skills
- **What**: After source files change, run the existing sync/setup path and inspect generated diffs as validation output only. Do not manually edit generated adapters or baked command files.
- **Verify**: Each supported provider receives the same memory rules from generated source-derived adapters, and generated output contains no hand-authored divergence.

## Files to Create or Modify

### Phase 1: Required authored changes

- `conventions/token-budget.md` - authoritative File-System Memory Contract, cold-start stack, memory ownership table, no-external-memory rule, and lesson-promotion/pruning gates.
- `workflows/coding.md` - cross-session, cross-provider, and chat/agent handoff semantics.
- `commands/gal-wrap-up/SKILL.template.md` - wrap-up behavior for provider switching and correct `.dev/plans/<slug>.prompt.md` execution-memory resolution.
- `commands/plan-to-prompt/SKILL.template.md` - execution prompt generation wording that names `.dev/plans/<slug>.prompt.md` as shared mutable memory for chat and agents.
- `templates/plan-prompt.md` - template wording for the shared mutable execution memory file.

### Phase 2: Conditional authored changes

- `.dev/project.md` - add a compact verified fact only if the Phase 1 contract does not make cross-project boundaries clear enough in practice.
- `templates/project.md` - propagate the minimum cold-start wording only if new repos cannot infer the contract from the primary source files.
- `docs/devguide.md` - add a short maintainer pointer only if contributors need an explicit navigation aid to the authoritative contract.
- `docs/personalization.md` - add operator-facing privacy and machine-local memory boundaries only if the current user-facing guidance leaves ambiguity after the source-contract pass.
- `agent/agents.md` - update only if agent-index wording still conflicts with the primary memory contract after Phase 1.

### Generated Validation Outputs

- `.github/copilot-instructions.md` - generated adapter output after sync only; validate regenerated content but do not edit directly.
- `AGENTS.md` - generated adapter output after sync only; validate regenerated content but do not edit directly.
- `CLAUDE.md` - generated adapter output after sync only; validate regenerated content but do not edit directly.
- `GEMINI.md` - generated adapter output after sync only; validate regenerated content but do not edit directly.
- `commands/*/SKILL.md` - generated command outputs after setup/sync only; validate regenerated content but do not edit directly.

## Test Cases

- [ ] Cross-session resume: after `/gal wrap-up`, a fresh session can recover active plan, last completed work, next step, blockers, and task status from `.dev/state.md` plus `.dev/plans/<slug>.prompt.md` without reading prior chat.
- [ ] Cross-provider handoff: one provider writes handoff notes and state, then a different provider can run `/gal status` or `/gal whats-next` and report the same current position from repo files.
- [ ] Cross-project boundary: a second initialized repo inherits the memory contract from GAL templates and adapters, but does not receive project-specific facts from the first repo unless those facts were deliberately promoted into shared GAL source docs.
- [ ] Chat/agent parity: a normal chat answer, `/gal pipeline`, and a golem specialist all identify the same active plan and task state from the cold-start stack.
- [ ] Remi-baseline review: the implementation strengthens file-based memory operations and tiering without introducing Mem0-style middleware, vector/graph dual storage, or a separate memory service.
- [ ] Learning-loop review: a confirmed repeated mistake can be traced from task memory into project memory or shared GAL methodology through an explicit promotion rule.
- [ ] Operation-contract review: the docs define when GAL should retrieve, encode, summarize, promote, and prune memory.
- [ ] Promotion-gate review: the docs define what evidence is sufficient for task-to-project promotion, project-to-shared promotion, and pruning.
- [ ] Generated adapter validation: regenerated provider files contain the same memory contract derived from source files, with no direct manual adapter edits.
- [ ] Negative external-memory check: no implementation task introduces Mem0, Zep, MemGPT-style frameworks, provider-local memory APIs, databases, vector stores, or long-running memory daemons as core GAL state.
- [ ] Privacy review: docs warn that secrets, private diaries, personal notes, and machine-local paths do not belong in tracked GAL memory.

## Success Criteria

- [ ] GAL has a clear memory topology that explicitly covers cross-session, cross-provider, cross-project, chat, and implementation-agent behavior.
- [ ] Every memory scope has one owner location, one write path, and one read path.
- [ ] GAL is explicitly positioned as a Remi-like file-memory system with stronger operational rules, not as a Mem0-style memory middleware stack.
- [ ] GAL has a lightweight learning loop that makes repeated mistakes less likely over time through verified lesson promotion and pruning.
- [ ] GAL defines the minimum memory operations needed for accuracy without adding a new service layer.
- [ ] GAL defines explicit promotion and prune gates so durable memory changes are evidence-based rather than intuition-based.
- [ ] Provider switching works through `/gal wrap-up` plus Cold Start Priority, not through provider-local chat memory.
- [ ] Cross-project memory is bounded: reusable methodology is shared through GAL source files and templates; project-specific facts remain in the owning repo.
- [ ] Chat interactions and golem implementation agents use the same repo-owned memory stack and active execution prompt.
- [ ] No core GAL state depends on external services, databases, vector stores, or runtime-specific memory APIs.
- [ ] Generated adapters and command outputs are refreshed from source and show consistent runtime guidance.

## Risks

- Cross-project memory is easy to misunderstand as automatic fact sharing between repos. The implementation must state that only shared methodology crosses projects by default.
- If Mem0-like ideas are copied in piecemeal form, the repo can accidentally recreate a hidden middleware layer while claiming to stay file-based. The implementation must keep storage and retrieval transparent and file-addressable.
- If lesson promotion is too loose, GAL can turn temporary guesses into durable rules and become less accurate over time. The implementation must require evidence before promotion and allow pruning of stale lessons.
- Duplicating the memory contract across docs can create drift. Keep normative wording in `conventions/token-budget.md` and use short pointers elsewhere.
- Chat/agent parity can fail if command prompts or agent docs point at different state files. The implementation must make `.dev/state.md` plus `.dev/plans/<slug>.prompt.md` the common active-work substrate.
- Generated adapters can drift if edited by hand. Treat adapter changes as sync validation output only.
- Privacy boundaries can weaken if repo memory is described too broadly. The plan must keep secrets, private notes, diary content, and local paths out of tracked GAL memory.
- This plan touches protected paths (`conventions/`, `workflows/`, `templates/`, command templates, and agent docs), so it requires architect-reviewed planning before implementation.

## References

- YouTube analysis source: [Agent记忆框架怎么选?5大Agent Memory项目工程级横向对比](https://youtu.be/BVwpVRpbph4?si=uu4hyF3RKVslH2Iv)
- GitHub repository entry point: [README.md](https://github.com/monkey1wizard/Golem-Agents-Legion/blob/main/README.md)
- GitHub memory-loading and handoff contract: [conventions/token-budget.md](https://github.com/monkey1wizard/Golem-Agents-Legion/blob/main/conventions/token-budget.md)
- GitHub execution workflow contract: [workflows/coding.md](https://github.com/monkey1wizard/Golem-Agents-Legion/blob/main/workflows/coding.md)
- GitHub user guidance surface: [docs/personalization.md](https://github.com/monkey1wizard/Golem-Agents-Legion/blob/main/docs/personalization.md)

## Open Questions

None.

<!-- Format: - [ ] OQ-NNN - description *(raised by: command)* -->
<!-- Resolved: - [x] OQ-NNN - description *(raised by: command, resolved by: engineering-review-lane)* -->

- [x] OQ-001 - Cross-project memory means shared GAL methodology and templates cross projects by default; project-specific facts remain in the owning repo unless deliberately promoted into shared GAL source files. *(raised by: replanning, resolved by: source-plan rewrite)*
- [x] OQ-002 - Chat interactions and implementation agents must use the same memory substrate: `.dev/project.md`, `.dev/state.md`, and `.dev/plans/<slug>.prompt.md` after prompt generation. *(raised by: replanning, resolved by: source-plan rewrite)*
- [x] OQ-003 - External memory systems are out of scope for core GAL state; if this file-based model cannot satisfy the main goal, the plan should stop rather than optimize peripheral documentation. *(raised by: replanning, resolved by: source-plan rewrite)*
- [x] OQ-004 - The YouTube comparison should be used to reinforce GAL's existing Remi-like direction, not to introduce a Mem0-style middleware layer. *(raised by: user follow-up, resolved by: source-plan rewrite)*
- [x] OQ-005 - The required additions from the remaining comparison methods are the minimum useful pieces only: cross-session continuity, tiered memory use, explicit operations, and a low-overhead learning loop that reduces repeated mistakes. *(raised by: user follow-up, resolved by: source-plan rewrite)*

## Approval

- Human approval: [clear]
- Architect review: [clear]
- Additional domain review: [not triggered]

## Review Results

### Architecture Review

#### Verdict: APPROVE

This rewrite makes the actual memory architecture explicit. The plan now treats cross-session, cross-provider, cross-project, chat, and implementation-agent parity as the central requirement instead of a side effect of documentation cleanup.

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| Use repo-owned Markdown as the authoritative memory substrate | Portable across providers, machines, sessions, and git history | Requires disciplined write-back and concise handoff notes | OK |
| Treat GAL as already Remi-like and improve operations instead of replacing storage | Preserves the existing low-load architecture and transparent files | Requires explicit memory-operation rules to gain accuracy without a new service | OK |
| Add a lightweight learning loop through promotion and pruning | Lets GAL improve from repeated mistakes over time | Requires evidence discipline so guesses do not become durable rules | OK |
| Bound cross-project memory to shared methodology, not automatic fact sharing | Prevents private or repo-specific context leakage | Users must deliberately promote reusable knowledge into GAL source docs | OK |
| Make `.dev/plans/<slug>.prompt.md` the shared mutable task memory | Gives chat and golem agents one active-work source | Requires prompt generation before implementation can rely on execution state | OK |
| Reject Mem0 mode and other external memory frameworks for core state | Preserves no-database architecture and runtime portability | Less automatic semantic recall across unrelated projects | OK |
| Keep generated adapters as derived carriers only | Maintains one source of truth | Requires sync validation after source edits | OK |

#### Over-engineering Flags

- **OE-01** Mem0 mode would add a second memory architecture on top of a repo that already behaves like Remi at the storage layer. Keep the file substrate and improve operations instead.
- **OE-02** A cross-project fact database would blur repo boundaries and risk leaking project-specific context. Share methodology through GAL source docs and templates instead.
- **OE-03** Repeating the full memory table in every doc would create drift. Put the normative table in `conventions/token-budget.md`; link or summarize elsewhere.
- **OE-04** A proactive learning loop should stay event-triggered and small. Do not turn it into a background agent fleet or always-on summarizer.

#### Bug Surface

- **BUG-01** Medium: If `/gal wrap-up` resolves source-plan rows to `docs/plans/<slug>.prompt.md`, it will miss the real execution prompt under `.dev/plans/`. Fix the command template while implementing Step 2.
- **BUG-02** Medium: If chat-oriented commands and golem agents name different memory files, provider handoff will produce inconsistent state. Fix by naming `.dev/state.md` plus `.dev/plans/<slug>.prompt.md` as the common active memory substrate.
- **BUG-03** Low: If cross-project memory wording is vague, users may expect automatic recall between repos. Fix by explicitly separating shared methodology from project facts.
- **BUG-04** Medium: If Mem0-like metadata or retrieval layers are added without a hard boundary, GAL can drift into hidden non-file memory behavior that users cannot inspect or review. Fix by keeping all authoritative state file-addressable and explicitly rejecting middleware storage tiers.
- **BUG-05** Medium: If every failure is promoted immediately, GAL will accumulate noisy or contradictory lessons. Fix by requiring a verified root cause or repeat signal before promotion.

#### Performance Concerns

None. The plan adds Markdown contract clarity and adapter regeneration only; it does not add runtime data loading or new background services.

#### Missing from Plan

None after this rewrite. The main goal is now testable through cross-session, cross-provider, cross-project, and chat/agent parity scenarios.

#### Recommended Changes

1. Implement the memory ownership table first in `conventions/token-budget.md`; all other files should point back to it.
2. State explicitly that GAL is keeping a Remi-like file substrate and is not implementing Mem0 mode.
3. Add the minimum memory-operations contract: retrieve, encode, summarize, promote, and prune.
4. Add explicit lesson-promotion rules so repeated mistakes reduce over time without introducing a new runtime service.
5. Fix `/gal wrap-up` execution prompt resolution and provider-switch wording in the same implementation pass.
6. Update `plan-to-prompt` and the execution prompt template so chat and agents share one mutable task memory file.
7. Keep cross-project memory deliberately bounded to shared GAL methodology and templates unless a later architect-reviewed plan introduces a stronger mechanism.

#### What's Good (keep these)

- The plan preserves GAL's document-driven and no-database architecture.
- The plan makes the failure condition clear: if the file-based model cannot serve cross-session, cross-provider, cross-project, chat, and agent memory, peripheral optimization is not useful.
- The plan keeps generated adapters as carriers of source truth rather than new sources of truth.

#### Scope Refinement

- **Phase 1 only**: land the authoritative contract and shared active-memory path first in `conventions/token-budget.md`, `workflows/coding.md`, `commands/gal-wrap-up/SKILL.template.md`, `commands/plan-to-prompt/SKILL.template.md`, and `templates/plan-prompt.md`.
- **Phase 2 only if needed**: touch `.dev/project.md`, `templates/project.md`, `docs/devguide.md`, `docs/personalization.md`, or `agent/agents.md` only when the Phase 1 source-of-truth pass leaves a demonstrated ambiguity.
- **Generated outputs are validation only**: regenerated adapters and command outputs are inspection targets after sync, not authored implementation surfaces.

### Business Review

Not triggered. The plan does not change business rules, pricing, permissions, notifications, onboarding, or eligibility logic.

### Design Review

Not triggered. The plan does not change customer-facing flows, layout, components, or accessibility-sensitive UI states.

### Engineering Review

#### Verdict: CLEAR

The plan is sound and ready for implementation. It defines the required architecture boundary, explicitly rejects external memory middleware for core GAL state, names the primary source-of-truth files for the required Phase 1 pass, and constrains follow-on Phase 2 edits behind demonstrated ambiguity rather than bundling them into the required implementation.

Engineering rationale:

- The required work is tightly anchored to a small set of authoritative source files, which keeps the change reviewable even though it touches protected paths.
- The plan defines both positive behavior and negative constraints, so implementation can improve the file-based memory model without drifting into a second hidden memory architecture.
- The handoff path, shared execution-memory substrate, and generated-output propagation requirements are explicit and testable.
- Conditional follow-on edits are appropriately deferred, which prevents scope creep and reduces documentation drift.

No engineering blocker requires a return to `/deep-planning`. Human approval remains pending under `## Approval`, but that is a governance state, not an engineering defect in the source plan.

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | doc review | Confirm `conventions/token-budget.md` defines memory scopes, owner files, writers, readers, cold-start behavior, retrieve / encode / summarize / promote / prune operations, promotion gates, prune gate, and the explicit rejection of external memory middleware for core GAL state. | T-001 |
| TP-002 | doc review | Confirm `workflows/coding.md` names `.dev/state.md` plus `.dev/plans/<slug>.prompt.md` as the common resume substrate for control-plane chat and specialist agents, and makes `/gal wrap-up` the handoff path for provider or machine switching. | T-002 |
| TP-003 | doc review | Confirm `commands/gal-wrap-up/SKILL.template.md` resolves active execution files under `.dev/plans` and no longer points to `docs/plans/<slug>.prompt.md`; verify provider-switch and machine-switch handoff language is file-based. | T-003 |
| TP-004 | doc review | Confirm `commands/plan-to-prompt/SKILL.template.md` defines the execution prompt as the shared mutable work file and preserves execution-owned sections on refresh without inventing a separate chat-memory lane. | T-004 |
| TP-005 | doc review | Confirm `templates/plan-prompt.md` matches the shared-memory contract and clearly separates stable planning content from mutable execution state. | T-005 |
| TP-006 | integration | Run the existing sync/setup regeneration path and inspect generated diffs to verify provider adapters and generated command outputs carry the same memory contract with no manual edits. | T-006 |
| TP-007 | manual (conditional) | If `docs/personalization.md` is updated, confirm privacy guidance keeps secrets, diary content, personal notes, and local machine paths out of tracked GAL memory. | T-007 |
| TP-008 | manual (conditional) | If `docs/devguide.md` is updated, confirm it only points contributors to the authoritative contract and does not duplicate normative rules. | T-008 |
| TP-009 | manual (conditional) | If `templates/project.md` is updated, confirm new repos inherit the bounded cross-project memory wording without copying project-specific facts. | T-009 |
| TP-010 | manual (conditional) | If `.dev/project.md` is updated, confirm it adds only a compact repo summary fact about the cross-project boundary. | T-010 |
| TP-011 | manual (conditional) | If `agent/agents.md` is updated, confirm its wording aligns with the same shared execution-memory substrate used by workflow and command docs. | T-011 |

## Tasks

- [ ] T-001 — Update `conventions/token-budget.md` with the authoritative file-system memory contract: memory scopes, owner files, writers, readers, cold-start usage, retrieve / encode / summarize / promote / prune operations, promotion gates, prune gate, and the explicit no-external-memory rule for core GAL state.
  Verify: the document alone lets a reader identify the owner location and promotion path for task, session, project, shared methodology, private, and generated memory surfaces.
- [ ] T-002 — Update `workflows/coding.md` to encode cross-session and cross-provider handoff semantics, require file-based resume through `.dev/state.md` plus `.dev/plans/<slug>.prompt.md`, and state that control-plane chat plus specialist agents share the same execution-memory substrate.
  Verify: the workflow text makes `/gal wrap-up` the handoff path and names the same repo-owned files for chat and specialist resumption.
- [ ] T-003 — Update `commands/gal-wrap-up/SKILL.template.md` so active-plan resolution prefers `.dev/plans/<slug>.prompt.md`, not `docs/plans/<slug>.prompt.md`, and so wrap-up guidance explicitly covers pausing, provider switching, and machine switching through repo files only.
  Verify: the template resolves the execution prompt under `.dev/plans` and its handoff language is provider-agnostic and file-based.
- [ ] T-004 — Update `commands/plan-to-prompt/SKILL.template.md` so the execution prompt is defined as the shared mutable work file for control-plane chat and specialist write-back flows, seeded from the refined source plan without inventing a separate chat-memory lane.
  Verify: the command text points to `.dev/plans/<slug>.prompt.md` as the common mutable execution file and preserves execution-owned sections on refresh.
- [ ] T-005 — Update `templates/plan-prompt.md` so the scaffold wording matches the shared-memory contract and clearly identifies which sections are stable planning content versus mutable execution-owned state.
  Verify: the template header and section guidance match the source-contract wording used by plan-to-prompt and the workflow docs.
- [ ] T-006 — Regenerate derived runtime and command outputs through the existing sync/setup path and inspect the generated diffs only as validation output.
  Verify: generated adapters and baked command files reflect the updated memory contract with no hand-authored divergence.
- [ ] T-007 — (optional, deferred) Update `docs/personalization.md` only if Phase 1 still leaves privacy or machine-local memory boundaries ambiguous.
  Verify: the doc explicitly routes secrets, diary content, personal notes, and local machine paths away from tracked GAL memory.
- [ ] T-008 — (optional, deferred) Update `docs/devguide.md` only if contributors still need an explicit pointer to the authoritative memory contract after Phase 1.
  Verify: the doc adds a short navigation pointer without duplicating the normative contract.
- [ ] T-009 — (optional, deferred) Update `templates/project.md` only if newly initialized repos cannot infer the cross-project memory boundary from the primary contract files.
  Verify: the template adds only the minimum cold-start wording needed to keep project facts local and methodology shared.
- [ ] T-010 — (optional, deferred) Update `.dev/project.md` only if this repo still needs a compact verified fact about cross-project memory boundaries after Phase 1.
  Verify: the repo summary adds a short fact without restating the full contract.
- [ ] T-011 — (optional, deferred) Update `agent/agents.md` only if agent-index wording still conflicts with the shared execution-memory contract after Phase 1.
  Verify: the agent guidance points at the same repo-owned memory stack as the workflow and command templates.
