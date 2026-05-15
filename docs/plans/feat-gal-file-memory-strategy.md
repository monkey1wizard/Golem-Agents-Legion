# Plan: GAL File-Based Memory Strategy

## Goal

GAL's memory model is documented and operationalized as repo-owned Markdown state, so Copilot, Gemini CLI, Codex, Claude Code, and other runtimes inherit the same context without external memory frameworks, databases, provider-local memory APIs, or hand-edited generated adapters.

## Requirements

- [ ] GAL memory must remain local, file-system based, versionable, and aligned with the existing Document-driven / No Database architecture.
- [ ] The strategy must explicitly reject Mem0, Zep, MemGPT-style external memory frameworks as core GAL state unless a future architect-reviewed plan changes the architecture.
- [ ] The plan must define stable ownership for transient, session, and durable memory across `.dev/plans/`, `.dev/state.md`, `.dev/project.md`, `conventions/`, and long-lived docs.
- [ ] Provider switching must be documented as a file handoff process: `/gal wrap-up` writes reusable context, then the next provider follows Cold Start Priority.
- [ ] Generated runtime adapters must stay derived from source documents and scripts; implementation must not hand-edit generated adapter outputs as the source of truth.
- [ ] The strategy must include privacy guidance: repo memory stores repo-relevant context only, never secrets or private journal material.

## Approach

### Step 1: Turn the external references into a repo contract

- **Files**: `.dev/project.md`
- **What**: Use the referenced GitHub and YouTube source material below as the evidence base, then index the accepted strategy in project context as a verified architectural fact: GAL memory is repo-owned Markdown, not an external memory service.
- **Verify**: `.dev/project.md` names the memory strategy without duplicating the full research note.

### Step 2: Make memory tiers explicit in the token discipline convention

- **Files**: `conventions/token-budget.md`
- **What**: Add a concise File-System Memory Contract covering transient working memory, session handoff memory, durable project memory, and generated adapter outputs. Tie each tier to the existing Cold Start Priority and Knowledge Flow Direction sections.
- **Verify**: The convention states where each class of memory belongs and says external memory frameworks are out of scope for core GAL state.

### Step 3: Align workflow handoff rules with cross-provider memory

- **Files**: `workflows/coding.md`, `commands/gal-wrap-up/SKILL.template.md`
- **What**: Clarify that `/gal wrap-up` is the required provider-switch handoff path and that provider-local chat memory is not authoritative GAL state. Keep the change narrowly scoped to workflow semantics.
- **Verify**: The coding workflow and wrap-up command describe provider handoff through plan `### Handoff Notes` and `.dev/state.md`.

### Step 4: Add brief operator guidance without creating a second spec

- **Files**: `docs/personalization.md`, `docs/devguide.md`
- **What**: Put operator guidance in `docs/personalization.md`, then add only a short directional pointer in `docs/devguide.md` so maintainers can find the source contract without duplicating it. Keep normative memory wording in `conventions/token-budget.md`.
- **Verify**: Users can find the guidance quickly, and `docs/devguide.md` stays navigational rather than becoming a second memory specification.

### Step 5: Propagate the minimum cold-start wording to templates

- **Files**: `templates/project.md`
- **What**: Only if the finalized source contract reveals a real bootstrap gap for newly initialized repos, add the smallest possible wording to `templates/project.md`. Do not change `templates/state.md` in this plan unless implementation finds that the existing session continuity scaffold is actually insufficient.
- **Verify**: New repos inherit the memory contract at cold start without expanding the state template or introducing a second spec.

### Step 6: Regenerate adapters as a validation pass

- **Files**: `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`
- **What**: After source files are updated, run the existing adapter sync and inspect the generated diffs as output validation only. Do not treat generated files as authored scope or edit them directly.
- **Verify**: Generated adapters reflect the source contract consistently across runtimes after sync.

## Files to Create or Modify

- `.dev/project.md` - compact verified fact and source index entry for the accepted strategy.
- `conventions/token-budget.md` - authoritative memory tier and context discipline contract.
- `workflows/coding.md` - provider-switch and handoff semantics.
- `commands/gal-wrap-up/SKILL.template.md` - wrap-up behavior wording for cross-provider handoff.
- `docs/personalization.md` - operator-facing boundary between repo memory and machine-local or personal memory.
- `docs/devguide.md` - short maintainer pointer to the normative memory contract.
- `templates/project.md` - optional minimal cold-start propagation only if the finalized contract reveals a real bootstrap gap.
- `.github/copilot-instructions.md` - generated adapter output after sync only; do not edit directly.
- `AGENTS.md` - generated adapter output after sync only; do not edit directly.
- `CLAUDE.md` - generated adapter output after sync only; do not edit directly.
- `GEMINI.md` - generated adapter output after sync only; do not edit directly.

## Test Cases

- [ ] Documentation review: no plan step introduces a database, vector store, remote memory API, or provider-specific memory dependency as core GAL state.
- [ ] Cold-start review: an agent can identify the correct memory read order from `.dev/project.md`, `.dev/state.md`, and the active plan file.
- [ ] Provider-switch review: the docs explain how Provider A writes handoff context and Provider B resumes from repo files.
- [ ] Adapter sync review: generated adapters contain the updated memory contract only after source files are changed and sync is run, with no direct manual edits.
- [ ] Privacy review: docs warn against storing secrets, private diary content, or non-repo personal memory in tracked GAL files.

## Success Criteria

- [ ] GAL has one documented memory strategy: local Markdown files are authoritative, provider-local memory is not.
- [ ] Transient, session, and durable memory each have one clear owner location.
- [ ] Cross-provider handoff is documented as `/gal wrap-up` plus Cold Start Priority.
- [ ] No implementation task adds external services, databases, vector stores, or long-running memory daemons.
- [ ] Generated adapters are refreshed from source and show consistent runtime guidance.

## Risks

- This plan touches protected paths (`conventions/`, `workflows/`, `templates/`, and command templates), so it needs architect-reviewed planning before implementation.
- Duplicating the memory contract across too many docs can create drift; source-of-truth wording should live in `conventions/token-budget.md` and be summarized elsewhere.
- Treating all memory as repo Markdown can accidentally encourage committing private or secret data; privacy boundaries must be explicit.
- Scope can balloon if template propagation expands beyond `templates/project.md`; split a follow-up plan if implementation discovers a wider bootstrap gap.
- Updating generated adapters by hand would break the generated-adapter contract and create future sync drift.

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

- [x] OQ-001 - Template propagation should stay minimal in this plan: update `templates/project.md` only if the finalized contract reveals a real cold-start gap, and leave `templates/state.md` unchanged. *(raised by: planning, resolved by: architecture-review)*
- [x] OQ-002 - Documentation-level enforcement is enough for this plan; any validation or dependency enforcement belongs in a separate future plan if drift appears. *(raised by: planning, resolved by: architecture-review)*

## Approval

- Human approval: [pending]
- Architect review: [clear]
- Additional domain review: [not triggered]

## Review Results

### Architecture Review

#### Verdict: APPROVE

Deep-planning narrowed the original draft to the minimum viable contract change. The plan now keeps the authoritative memory contract in one source file, treats generated adapters as validation output rather than authored scope, and limits template propagation to a real cold-start gap only.

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| Make `conventions/token-budget.md` the authoritative memory contract | Keeps one normative source for memory loading and handoff semantics | Requires brief cross-references elsewhere instead of duplicate prose | OK |
| Keep `/gal wrap-up` plus `workflows/coding.md` as handoff owners | Reuses existing control-plane boundaries | Requires coordinated wording across two source files | OK |
| Limit template propagation to `templates/project.md` only when needed | Gives new repos cold-start guidance without rewriting state scaffolding | Some template wording may remain unchanged if no real gap exists | OK |
| Treat adapter regeneration as validation output only | Preserves the generated-file contract | Adds one sync and diff inspection pass before closure | OK |

#### Over-engineering Flags

- **OE-01** Generated adapters were listed like authored files. Keep them as sync outputs only; do not edit them directly.
- **OE-02** `templates/state.md` was in scope without a concrete gap. The current state template already owns session continuity and active-plan tracking; leave it unchanged in this plan.
- **OE-03** `docs/devguide.md` risked becoming a second spec. Keep it to a short pointer back to the normative contract.

#### Bug Surface

- **BUG-01** Medium: Duplicating normative memory wording across `conventions/token-budget.md`, `docs/devguide.md`, and `docs/personalization.md` will drift. Fix: make `conventions/token-budget.md` authoritative and summarize elsewhere.
- **BUG-02** Low: Contributors may edit generated adapters directly if the plan treats them as first-class source files. Fix: label regeneration as validation output only.

#### Recommended Changes

1. Keep the normative memory contract in `conventions/token-budget.md`, with `.dev/project.md` and `workflows/coding.md` only summarizing ownership and handoff.
2. Narrow template propagation to `templates/project.md` only if the finalized source contract reveals a cold-start gap for initialized repos.
3. Keep `docs/personalization.md` as the operator-facing boundary for repo memory versus machine-local or personal memory, and keep `docs/devguide.md` directional only.

#### What's Good (keep these)

- Reusing `.dev/project.md`, `.dev/state.md`, and `### Handoff Notes` aligns with the existing document-driven architecture.
- Rejecting external memory frameworks preserves the repo's no-database portability constraint.
- Keeping provider switching anchored on `/gal wrap-up` plus Cold Start Priority reuses existing workflow primitives instead of inventing a new memory lane.

### Business Review

Not triggered. The plan does not change business rules, pricing, permissions, notifications, onboarding, or eligibility logic.

### Design Review

Not triggered. The plan does not change customer-facing flows, layout, components, or accessibility-sensitive UI states.

### Engineering Review

Pending.

## Test Plan

Pending.

## Tasks

Pending.
