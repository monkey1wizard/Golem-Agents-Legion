# Plan Prompt: GAL Unified File Memory Strategy

<!--
Generated from docs/plans/feat-gal-file-memory-strategy.md.
Output path: .dev/plans/feat-gal-file-memory-strategy.prompt.md
This is the mutable execution work file consumed by /gal status, /gal whats-next, /gal pipeline, and specialist write-back flows.
-->

## Goal

GAL has one authoritative file-system memory model that works across sessions, projects, providers, chat interactions, and implementation agents. Copilot, Gemini CLI, Codex, Claude Code, and future runtimes must all recover and update the same repo-owned Markdown memory without relying on provider-local chat history, external memory frameworks, vector stores, databases, or hand-edited generated adapters.

## Requirements

- [ ] Cross-session memory must be recoverable from repo files alone: `.dev/state.md`, `.dev/plans/<slug>.prompt.md`, and source plan handoff context must let a new session resume without prior chat history.
- [ ] Cross-provider memory must use file handoff, not provider memory: Provider A writes repo state through GAL workflow files, then Provider B reads the same cold-start stack and continues from the same facts.
- [ ] Cross-project memory must be explicit and bounded: project facts stay in each target repo, while reusable GAL methodology is promoted only through GAL source docs, conventions, workflows, templates, commands, and agents.
- [ ] Chat interactions and implementation agents must use the same memory substrate: normal AI chat, `/gal status`, `/gal whats-next`, `/gal pipeline`, and golem specialists all read the same cold-start priority and write back only to owned GAL files.
- [ ] The strategy must treat GAL as already Remi-like at the storage layer: repo-owned Markdown remains the default memory substrate, and the work focuses on stronger operations, tiering, and consolidation rather than replacing storage.
- [ ] The strategy must add a lightweight memory-operations contract for the core actions GAL actually needs: retrieve, encode, summarize, promote, and prune.
- [ ] The strategy must add a low-overhead learning loop so recurring errors become durable lessons: mistakes found during implementation, review, test, or debugging must be eligible for promotion from task memory to project memory, then to shared GAL methodology when evidence shows they are reusable.
- [ ] The strategy must improve accuracy over time without background daemons or heavy indexing: lesson consolidation must be event-triggered and file-based.
- [ ] GAL memory must remain local, file-system based, versionable, reviewable, and aligned with the existing Document-driven / No Database architecture.
- [ ] The strategy must explicitly reject Mem0 mode and Mem0-like middleware patterns for core GAL state: no dual-store vector/graph stack, no memory middleware service, no database-backed semantic layer, and no hidden retrieval tier unless a future architect-reviewed plan changes the architecture.
- [ ] The strategy must explicitly reject Zep, MemGPT-style external frameworks, provider-local memory APIs, long-running memory daemons, vector stores, and databases as core GAL state unless a future architect-reviewed plan changes the architecture.
- [ ] Generated runtime adapters must remain derived outputs from source documents and setup scripts; they may carry the memory contract to providers but must not become the source of truth.
- [ ] Privacy boundaries must be explicit: repo memory stores repo-relevant context only, never secrets, private journal material, machine-local paths, or personal memory that belongs in local config or Obsidian.

## Approach

This plan treats memory as a file ownership contract, not as a new service.

### Memory Ownership Table

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

- Remi baseline accepted: GAL already behaves most like Remi because its durable memory is repo-owned Markdown, transparent to the user, git-reviewable, and file-addressable.
- Mem0 mode rejected: GAL must not add middleware-style memory orchestration, dual-store vector plus graph persistence, or a separate memory service.
- Core additions accepted: Explicit memory tiering, explicit memory operations, and event-triggered lesson consolidation only.
- Performance rule: Prefer event-triggered file updates and bounded reads over background indexing or always-on memory workers.

### Core Learning Loop

1. Retrieve: At task start, provider switch, review, test, or debug entry, read the minimum relevant repo memory from the cold-start stack.
2. Encode: When a new fact, blocker, decision, or failure is confirmed, write it first to the active execution memory file.
3. Summarize: At wrap-up or checkpoint boundaries, compress the active session into `### Handoff Notes` and `.dev/state.md`.
4. Promote: When a lesson is verified and likely to recur, move it upward from task memory to project memory, then to shared GAL methodology only if it generalizes across repos.
5. Prune: Remove or replace stale, disproven, or superseded lessons so file memory stays trustworthy.

### Promotion and Pruning Gates

- Task to project: promote only when a lesson has a verified root cause and is likely to recur inside the current repo. Evidence: confirmed debugger/reviewer/tester finding, or repeated manual correction.
- Project to shared GAL: promote only when a lesson changes GAL methodology applicable across repos, not just one project's local practice.
- No guess promotion: hypotheses and unverified explanations must remain in task memory.
- Prune gate: remove when later evidence disproves, a newer rule supersedes, or the owning surface no longer exists.
- Promotion cost rule: manual, event-triggered, bounded to existing repo files. No background agents, daemons, or bulk promotion.

### Implementation Steps

- Step 1: `conventions/token-budget.md` — authoritative File-System Memory Contract including all six memory scopes, five operations, promotion/pruning gates, and the explicit no-external-memory rule.
- Step 2: `workflows/coding.md` and `commands/gal-wrap-up/SKILL.template.md` — cross-session/provider handoff semantics; fix wrap-up resolution to prefer `.dev/plans/<slug>.prompt.md`.
- Step 3: `commands/plan-to-prompt/SKILL.template.md` and `templates/plan-prompt.md` — define execution prompt as shared mutable work file for chat and specialist agents.
- Step 4 (conditional): `.dev/project.md`, `templates/project.md`, `docs/devguide.md` — only if Phase 1 leaves cross-project boundary ambiguous.
- Step 5: `conventions/token-budget.md` and `workflows/coding.md` — lesson-consolidation rules.
- Step 6 (conditional): `docs/personalization.md` — privacy and machine-local memory boundary guidance.
- Step 7: Regenerate derived runtime surfaces via existing sync/setup path; inspect diffs only.

## Files to Create or Modify

### Phase 1 — Required

- `conventions/token-budget.md` — authoritative File-System Memory Contract, memory ownership table, five operations, promotion/pruning gates, no-external-memory rule.
- `workflows/coding.md` — cross-session, cross-provider, chat/agent handoff semantics.
- `commands/gal-wrap-up/SKILL.template.md` — fix active-plan resolution to `.dev/plans/<slug>.prompt.md`; add provider-switch and machine-switch handoff guidance.
- `commands/plan-to-prompt/SKILL.template.md` — name `.dev/plans/<slug>.prompt.md` as shared mutable memory for chat and agents.
- `templates/plan-prompt.md` — match updated shared-memory contract wording.

### Phase 2 — Conditional (only if Phase 1 leaves demonstrated ambiguity)

- `.dev/project.md` — compact cross-project boundary fact.
- `templates/project.md` — minimum cold-start wording for new repos.
- `docs/devguide.md` — contributor pointer to the authoritative contract.
- `docs/personalization.md` — operator privacy and machine-local memory boundary guidance.
- `agent/agents.md` — align with shared execution-memory substrate if still conflicting after Phase 1.

### Generated Validation Outputs (inspect after sync; do not edit directly)

- `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `commands/*/SKILL.md`

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
- If Mem0-like ideas are copied in piecemeal form, the repo can accidentally recreate a hidden middleware layer while claiming to stay file-based.
- If lesson promotion is too loose, GAL can turn temporary guesses into durable rules and become less accurate over time.
- Duplicating the memory contract across docs can create drift. Keep normative wording in `conventions/token-budget.md` and use short pointers elsewhere.
- Chat/agent parity can fail if command prompts or agent docs point at different state files.
- Generated adapters can drift if edited by hand.
- Privacy boundaries can weaken if repo memory is described too broadly.
- This plan touches protected paths (`conventions/`, `workflows/`, `templates/`, command templates, and agent docs).

## Open Questions

All open questions resolved in source plan.

<!-- Resolved: - [x] OQ-001 — Cross-project memory means shared GAL methodology and templates cross projects by default; project-specific facts remain in the owning repo unless deliberately promoted into shared GAL source files. *(raised by: replanning, resolved by: source-plan rewrite)* -->
<!-- Resolved: - [x] OQ-002 — Chat interactions and implementation agents must use the same memory substrate: `.dev/project.md`, `.dev/state.md`, and `.dev/plans/<slug>.prompt.md` after prompt generation. *(raised by: replanning, resolved by: source-plan rewrite)* -->
<!-- Resolved: - [x] OQ-003 — External memory systems are out of scope for core GAL state. *(raised by: replanning, resolved by: source-plan rewrite)* -->
<!-- Resolved: - [x] OQ-004 — The YouTube comparison reinforces the existing Remi-like direction; Mem0-style middleware is rejected. *(raised by: user follow-up, resolved by: source-plan rewrite)* -->
<!-- Resolved: - [x] OQ-005 — Required additions: cross-session continuity, tiered memory, explicit operations, low-overhead learning loop only. *(raised by: user follow-up, resolved by: source-plan rewrite)* -->

## Approval

- Human approval: [clear]
- Architect review: [clear]
- Additional domain review: [not triggered]

---

## Status

Workflow: REVIEW
Step: 6 of 11
Last activity: 2026-05-18 — T-006 complete (commit: e775d44ca5aab735838c02ba72f8881c31fb2d3f)
Next step: Optional Phase 2 tasks remain deferred; if no ambiguity remains, move to verification or handoff.
Current Task: —
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |

### Handoff Notes

Architect review APPROVE. Engineering review CLEAR. Human approval recorded on 2026-05-18 through explicit user go-ahead to run the pipeline. Source plan `## Tasks`, `## Test Plan`, and `### Engineering Review` were generated and written back on 2026-05-18. Begin at T-001.

## Tasks

- [x] T-001 — Update `conventions/token-budget.md` with the authoritative file-system memory contract: memory scopes, owner files, writers, readers, cold-start usage, retrieve / encode / summarize / promote / prune operations, promotion gates, prune gate, and the explicit no-external-memory rule for core GAL state.
  Verify: the document alone lets a reader identify the owner location and promotion path for task, session, project, shared methodology, private, and generated memory surfaces.
- [x] T-002 — Update `workflows/coding.md` to encode cross-session and cross-provider handoff semantics, require file-based resume through `.dev/state.md` plus `.dev/plans/<slug>.prompt.md`, and state that control-plane chat plus specialist agents share the same execution-memory substrate.
  Verify: the workflow text makes `/gal wrap-up` the handoff path and names the same repo-owned files for chat and specialist resumption.
- [x] T-003 — Update `commands/gal-wrap-up/SKILL.template.md` so active-plan resolution prefers `.dev/plans/<slug>.prompt.md`, not `docs/plans/<slug>.prompt.md`, and so wrap-up guidance explicitly covers pausing, provider switching, and machine switching through repo files only.
  Verify: the template resolves the execution prompt under `.dev/plans` and its handoff language is provider-agnostic and file-based.
- [x] T-004 — Update `commands/plan-to-prompt/SKILL.template.md` so the execution prompt is defined as the shared mutable work file for control-plane chat and specialist write-back flows, seeded from the refined source plan without inventing a separate chat-memory lane.
  Verify: the command text points to `.dev/plans/<slug>.prompt.md` as the common mutable execution file and preserves execution-owned sections on refresh.
- [x] T-005 — Update `templates/plan-prompt.md` so the scaffold wording matches the shared-memory contract and clearly identifies which sections are stable planning content versus mutable execution-owned state.
- [x] T-006 — Regenerate derived runtime and command outputs through the existing sync/setup path and inspect the generated diffs only as validation output.
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

## Analyze

[Written by golem-reviewer — verdict: CLEAR | DRIFT-OPEN | NOT-RUN]

<!-- Sole writer: golem-reviewer. golem-releaser, /gal status, /gal whats-next consume verdict only — they do not recalculate drift. -->

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

## Test Results

### [T-001] 2026-05-18

PASS

- TP-001 verified in `conventions/token-budget.md`.
- Defines memory scopes with authoritative files, primary writers, primary readers, and promotion paths.
- Defines cold-start priority and cold-start usage rules.
- Defines retrieve, encode, summarize, promote, and prune operations.
- Defines task-to-project and project-to-shared promotion gates, no-guess promotion, prune gate, and promotion cost rule.
- Explicitly rejects provider-local chat history as authoritative and rejects Mem0-style middleware, provider memory APIs, vector stores, databases, graphs, and long-running memory daemons for core GAL state.

### [T-002] 2026-05-18

PASS

- TP-002 verified in `workflows/coding.md`.
- Names `.dev/state.md` plus the active `.dev/plans/<slug>.prompt.md` as the shared resume substrate for control-plane chat and specialist agents.
- States `/gal wrap-up` is the required handoff path before pausing work or switching providers or machines, with resumption from repo files rather than provider-local transcript memory.

### [T-003] 2026-05-18

PASS

- TP-003 verified in `commands/gal-wrap-up/SKILL.template.md`.
- Active execution-file resolution prefers `.dev/plans/<slug>.prompt.md`; the stale `docs/plans/<slug>.prompt.md` target is not present.
- Handoff language is file-based for provider and machine switches: resume from the active prompt plus `.dev/state.md`, not provider-local transcript memory.

### [T-004] 2026-05-18

PASS

- TP-004 verified in `commands/plan-to-prompt/SKILL.template.md`.
- Defines `.dev/plans/<slug>.prompt.md` as the single shared mutable execution work file for control-plane chat, GAL commands, and specialist write-back flows.
- Refresh rules preserve execution-owned sections and explicitly reject inventing a separate chat-memory lane.

### [T-006] 2026-05-18

PASS

- TP-006 verified against `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `commands/gal-wrap-up/SKILL.md`, and `commands/plan-to-prompt/SKILL.md`.
- Regenerated adapter files carry the updated source-contract wording for file-owned memory, `.dev/plans/<slug>.prompt.md` task execution memory, shared execution-memory substrate, and wrap-up handoff through repo files.
- `commands/gal-wrap-up/SKILL.md` matches the template on `.dev/plans/<slug>.prompt.md` resolution and provider/machine handoff guidance; `commands/plan-to-prompt/SKILL.md` matches the template on shared mutable execution-work-file and refresh-preservation wording.
- `get_changed_files` reported no staged or unstaged changes, so there is no visible evidence of hand-edited divergence in the regenerated outputs.

### [T-005] 2026-05-18

PASS

- TP-005 verified in `templates/plan-prompt.md`.
- The scaffold identifies the execution prompt as the shared mutable execution work file for control-plane chat and specialist write-back flows.
- The divider notes explicitly separate stable planning content above from mutable execution-owned state below and state the refresh/preserve rule for each side.

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

#### Bug Surface

- BUG-01 Medium: If `/gal wrap-up` resolves source-plan rows to `docs/plans/<slug>.prompt.md`, it will miss the real execution prompt under `.dev/plans/`. Fix in T-003.
- BUG-02 Medium: If chat-oriented commands and golem agents name different memory files, provider handoff will produce inconsistent state. Fix in T-002 and T-004.
- BUG-03 Low: If cross-project memory wording is vague, users may expect automatic recall between repos. Fix in T-001.
- BUG-04 Medium: If Mem0-like metadata or retrieval layers are added without a hard boundary, GAL can drift into hidden non-file memory behavior. Fix in T-001.
- BUG-05 Medium: If every failure is promoted immediately, GAL will accumulate noisy or contradictory lessons. Fix in T-001.

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

### [T-001] 2026-05-18

Reviewed: 2026-05-18
Commit range: d78d547774979f0835ad5ccfccb967b510b75b48..226ec8d334c39e5a376fdb2299b8f438b9a9d41b
Verdict: APPROVE

Reasoning: `conventions/token-budget.md` satisfies T-001 and TP-001. The added File-System Memory Contract names the owner locations, primary writers, primary readers, cold-start usage, retrieve / encode / summarize / promote / prune operations, promotion and prune gates, and the explicit no-external-memory rule for core GAL state. The change stays within T-001 scope and does not conflict with the surrounding token-budget contract.

#### Summary

- Blocking: 0 (resolved: 0, open: 0)
- Warning: 0
- Info: 0

### [T-003] 2026-05-18

Reviewed: 2026-05-18
Commit range: 2738ed17362249c8ddab16f5239fce67b3299df8..f02f1c3a865243bc9a3b842f1c7bb50087ee0cb6
Verdict: APPROVE

Reasoning: `commands/gal-wrap-up/SKILL.template.md` satisfies T-003 and TP-003. Active-plan resolution now prefers `.dev/plans/<slug>.prompt.md` when the active row points at `docs/plans/<slug>.md`, and the wrap-up guidance explicitly treats `.dev/state.md` plus the active `.dev/plans/<slug>.prompt.md` as the required handoff package before pausing or switching providers or machines. This aligns with the existing workflow contract and removes the stale `docs/plans/<slug>.prompt.md` execution-path guidance.

#### Summary

- Blocking: 0 (resolved: 0, open: 0)
- Warning: 0
- Info: 0

### [T-002] 2026-05-18

Reviewed: 2026-05-18
Commit range: 226ec8d334c39e5a376fdb2299b8f438b9a9d41b..2738ed17362249c8ddab16f5239fce67b3299df8
Verdict: APPROVE

Reasoning: `workflows/coding.md` satisfies T-002 and TP-002. The change explicitly ties control-plane chat, `/gal status`, `/gal whats-next`, `/gal pipeline`, and execution-stage specialists to the same repo-owned execution-memory substrate of `.dev/state.md` plus the active `.dev/plans/<slug>.prompt.md`; it also makes `/gal wrap-up` the required handoff path before pausing work or switching providers or machines. The edit stays within the T-002 scope and does not contradict the existing workflow contract.

#### Summary

- Blocking: 0 (resolved: 0, open: 0)
- Warning: 0
- Info: 0

### [T-004] 2026-05-18

Reviewed: 2026-05-18
Commit range: f02f1c3a865243bc9a3b842f1c7bb50087ee0cb6..1abb829cb9749c5ea6587f5b5f6897946a2c4814
Verdict: APPROVE

Reasoning: `commands/plan-to-prompt/SKILL.template.md` satisfies T-004 and TP-004. The command now defines `.dev/plans/<slug>.prompt.md` as the shared mutable execution work file for control-plane chat, GAL commands, and specialist write-back flows; it explicitly rejects creating a separate chat-memory lane or second execution-state file; and its refresh rules preserve execution-owned sections as the authoritative mutable state while only backfilling missing placeholders when that does not overwrite existing execution history. This aligns with the existing workflow contract in `workflows/coding.md` and the task-execution memory contract in `conventions/token-budget.md`.

#### Summary

- Blocking: 0 (resolved: 0, open: 0)
- Warning: 0
- Info: 0

### [T-005] 2026-05-18

Reviewed: 2026-05-18
Commit range: 1abb829cb9749c5ea6587f5b5f6897946a2c4814..18d3bc6994c272f837c8cc6ddf77aa451b285705
Verdict: APPROVE

Reasoning: `templates/plan-prompt.md` satisfies T-005 and TP-005. The scaffold header now matches the shared-memory contract by defining the prompt as the shared mutable execution work file for control-plane chat, GAL commands, and specialist write-back flows. The added divider notes clearly separate stable planning content above from mutable execution-owned state below, and the mutable-state note preserves refresh semantics without inventing a separate chat-memory lane or conflicting with the updated `commands/plan-to-prompt/SKILL.template.md` contract.

#### Summary

- Blocking: 0 (resolved: 0, open: 0)
- Warning: 0
- Info: 0

### [T-006] 2026-05-18

Reviewed: 2026-05-18
Commit range: 18d3bc6be02440f86f717845f715627ad559a864..e775d44698f1ac5a25f9d11188059e17234922df
Verdict: APPROVE

Reasoning: The regenerated outputs satisfy T-006 and TP-006. `commands/gal-wrap-up/SKILL.md` matches the updated template on `.dev/plans/<slug>.prompt.md` resolution and file-based provider or machine handoff, and `commands/plan-to-prompt/SKILL.md` matches the updated template on the shared mutable execution-memory surface plus refresh preservation of execution-owned state. The regenerated adapter files carry the same execution-memory substrate and wrap-up handoff rules consistently across `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md`, with no blocking source-to-generated divergence in the reviewed scope.

#### Summary

- Blocking: 0 (resolved: 0, open: 0)
- Warning: 0
- Info: 0

## Debug Log

[Written by debugger specialist if debugging occurs during this plan]
