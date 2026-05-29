# Plan: GAL Pipeline Token Burn Reduction

## Goal

Reduce `/gal pipeline` token consumption without weakening GAL's safety guarantees, and specifically prevent OpenCode Go daily and weekly quota from being exhausted too early by redundant startup context, repeated phase context loading, and avoidable same-runtime overhead.

## Baseline Burn Surface

Historical baseline before this plan's first implementation pass:

| File | Lines | Bytes | Notes |
| ------ | ------- | ------- | ------- |
| `AGENTS.md` | 1,121 | 57,284 | Generated adapter |
| `.github/copilot-instructions.md` | 1,069 | 53,923 | Generated adapter |
| `CLAUDE.md` | 1,121 | 57,203 | Generated adapter |
| `GEMINI.md` | 1,121 | 57,313 | Generated adapter |
| `golem-implementer.agent.md` | 220 | 11,199 | Reads copilot-instructions.md at L30 |
| `golem-tester.agent.md` | 319 | 11,082 | Reads copilot-instructions.md at L39 |
| `golem-reviewer.agent.md` | 285 | 11,316 | Reads copilot-instructions.md at L30 |
| `golem-verifier.agent.md` | 188 | 7,448 | |
| `.dev/project.md` | 98 | 5,450 | Actual source of truth |

At baseline, the four generated adapters were near-identical clones. Only the first ~10 lines differed, and OpenCode startup loaded two large generated adapters. Pipeline-bound agent contracts also instructed another generated-adapter read per phase.

## Current Verified Surface

Checked on 2026-05-29 against the current repository:

| Surface | Current state | Gap |
| --- | --- | --- |
| OpenCode startup | `opencode.json` loads only `AGENTS.md`. | None for startup payload. |
| Generated adapter size | `AGENTS.md` is ~40 KB; provider adapters are ~39-41 KB after language-scoped embedding. | Baseline table is historical only. |
| Pipeline-bound agents | Implementer, tester, reviewer, security, and verifier treat generated adapters as already-loaded runtime carriers and use `.dev/project.md` as fallback. | Designer / analyst / architect still mention `copilot-instructions.md`, but they are not pipeline-bound agents in this plan. |
| Dispatcher metadata | PowerShell and Bash dispatchers emit `CONTEXT_CARRY`, `PIPELINE_CONTEXT_MODE`, `PIPELINE_CONTEXT_FILES`, and `CONVENTION_HINTS`. | Bash runtime execution still needs field validation on a host with Bash available. |
| Command contract | `commands/gal-pipeline/SKILL.template.md` contains same-runtime degraded markers, commit-boundary convergence gates, retry ceilings, protected-path escalation, interrupted-phase handoff, and final verifier requirements. | Repo source currently has only `SKILL.template.md`; generated `commands/gal-pipeline/SKILL.md` is a local baked output and may not exist until `Update-Commands` runs. |
| Token-burn test script | `scripts/Test-PipelineTokenBurn.ps1` exists and covers startup payload, dispatch metadata, adapter slimming, and command contract markers. | The script currently assumes `commands/gal-pipeline/SKILL.md` exists; update it to resolve template first or create baked output in an isolated temp path before validation. |

## Success Criteria (Numeric)

| Metric | Current | Target |
| -------- | --------- | -------- |
| OpenCode startup payload | Baseline ~111 KB; current ~40 KB | ≤60 KB |
| Single-task 3-phase total context load | Baseline ~270 KB; last measured current 42,163 bytes | ≤80 KB |
| 5-task pipeline total redundant context | Baseline ~1,071 KB; last measured current 47,159 bytes | ≤200 KB |

## Requirements

- [ ] `/gal pipeline` must continue to honor the existing safety model: implement, test, review, conditional security, verifier, retry ceilings, interrupted-phase handoff, and protected-path escalation must remain intact.
- [x] OpenCode must stop loading duplicate large generated adapters at startup; the repo-local OpenCode bridge should load a single authoritative instruction carrier.
- [x] Pipeline-bound golem agents must stop re-reading generated adapters during normal phase execution unless the task is explicitly about adapter content.
- [x] Same-runtime fallback must not silently collapse spec-driven testing and review independence by default just to save tokens.
- [x] Any same-runtime bundling optimization must be explicit, documented as degraded verification independence, and must still produce separate durable write-back for test and review.
- [x] Three-surface durable state convergence across `docs/plans/<slug>.md`, `.dev/plans/<slug>.prompt.md`, and `.dev/state.md` must be complete before any git commit that records task progress or completion; in-flight task-local edits may be staged internally, but no committed state may contain cross-surface disagreement.
- [x] Dispatch and agent changes must favor compact injected context and on-demand reads over repeated full cold-start loading.
- [x] Generated adapter content should be compressed at the generator layer using selective language-scoped embedding rather than full-body duplication or pointer indirection.
- [x] Personalized runtime instruction projections, including personalized `AGENTS.md`, must live under `~/.gal/generated` or the provider-visible `.gal` projection path, not in the GAL source repository root.
- [ ] Changes must preserve cross-runtime alignment across Copilot, Antigravity CLI, Codex CLI, Claude Code, and the OpenCode bridge lane.
- [x] Generated adapters remain derived outputs only; any fix must be authored in source files and sync scripts, not by hand-editing generated adapter files.
- [ ] The implementation must produce measurable before-and-after evidence against the numeric targets above.

## Approach

### Problem Statement

P0 and P0b already solved the repeated manual re-invocation problem for `/gal pipeline`, but they did not reduce total token burn per task. The remaining high-cost surfaces are:

1. OpenCode startup loads two large generated adapters from `opencode.json` (~111 KB, near-identical content).
2. Pipeline-bound agents re-read `copilot-instructions.md` per phase despite it already being loaded as system instructions (~54 KB × N phases).
3. All adapters embed all six convention files regardless of task language (~734 lines of which ~500 are irrelevant per task).
4. Dispatch provides no compact context block, so agents perform redundant file reads to determine phase state.

This plan fixes surfaces in strict ROI/risk order and leaves higher-risk convergence and bundling changes for a later, explicitly reviewed stage.

---

### Step 1 (P0-a): Immediate OpenCode startup diet

- **Files**: `opencode.json`
- **What**: Change the OpenCode `instructions` array to load only `AGENTS.md`, removing `.github/copilot-instructions.md`. `AGENTS.md` is the authoritative cross-CLI carrier; the Copilot-specific adapter is redundant for OpenCode.
- **Expected saving**: ~54 KB per session.
- **Verify**: OpenCode loads one generated adapter instead of two. Compare startup payload before and after. Confirm repo behavior is unchanged under the bridge lane.

---

### Step 2 (P0-b): Remove generated-adapter re-reads from pipeline-bound agents

- **Files**: `agent/golem-implementer.agent.md`, `agent/golem-tester.agent.md`, `agent/golem-reviewer.agent.md`, `agent/golem-security.agent.md`, `agent/golem-verifier.agent.md`
- **What**: Update pipeline-bound context rules so these agents treat generated adapters as already-loaded runtime carriers. During normal phase execution they should read `.dev/project.md` (98 lines), `.dev/state.md`, the active execution prompt, and only the specific source-of-truth convention or code files needed for that task.
- **Fallback rule**: If no system adapter is detectable (e.g. bare terminal invocation), agents read `.dev/project.md` (98 lines) as the fallback — not `copilot-instructions.md` (1,069 lines). This is a 10× reduction in fallback cost.
- **Expected saving**: ~810 KB over a 5-task pipeline run.
- **Verify**: Pipeline-bound agent docs no longer instruct routine reads of `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md`.

---

### Step 3 (P1): Language-scoped convention loading

- **Files**: `scripts/gal.ps1`, `scripts/gal.sh`, pipeline agent contracts if needed
- **What**: The dispatcher detects the primary language of the active task (from prompt description or `.dev/project.md` tech stack) and injects only the relevant language convention plus the two cross-language conventions (`token-budget.md`, `working-hours.md`). All other language conventions are excluded for that phase.
- **Why this beats pointer-based compression**: Selective loading is true content elimination. Pointer-based compression moves the read cost from system-instructions time to runtime tool-call time and adds file I/O overhead — net savings are near zero. Language-scoped exclusion eliminates content that is never needed.
- **Example**: A Go task loads `go.md` (56 lines) + `token-budget.md` (145 lines) + `working-hours.md` (90 lines) = 291 lines instead of all 734 lines (~60% reduction).
- **Verify**: Dispatch output for a Go task does not contain C#, Rust, or TypeScript convention content. A C# task does not contain Go, Rust, or TypeScript content.

---

### Step 4 (P2): Compact dispatch-level injected context

- **Files**: `scripts/gal.ps1`, `scripts/gal.sh`
- **What**: Extend pipeline-bound dispatch output with a compact context block (~20-40 lines) containing active plan path, source plan path, task ref, phase, retry counters, commit markers, and next unchecked task.
- **What not to do**: Do not inject full `.dev/project.md`, full plan text, or full conventions.
- **Context-carry flag**: If the runtime supports same-session context carry-over (e.g. Claude Code conversation), Phase 2 and Phase 3 of a task only receive the delta injection (phase name, retry counters). Cold-start runtimes receive the full compact block. The dispatcher should emit a `CONTEXT_CARRY: true/false` field so agent contracts can branch accordingly.
- **Expected saving**: ~120-225 KB over a 5-task pipeline run.
- **Verify**: Pipeline-bound agents can use the injected context as the first source for phase-local state and fall back to `.dev/state.md` or the prompt only when injection is absent.

---

### Step 5 (P3): Selective language-scoped adapter embedding at generator layer

- **Files**: `scripts/Sync-DevContext.ps1`, `scripts/sync-dev-context.sh`
- **What**: Generate adapters by reading the `Tech Stack` section of `.dev/project.md` to determine which language conventions to embed. Only embed conventions for languages the project actually uses, plus the two cross-language conventions. Keep `AGENTS.md` as the canonical shared instruction carrier, but keep provider-specific adapters because providers may auto-read `CLAUDE.md`, `GEMINI.md`, or `.github/copilot-instructions.md` and GAL cannot reliably disable that behavior.
- **Constraint**: Do not replace convention bodies with pointers or indexes — adapters must remain self-contained. The saving comes from selective inclusion, not indirection.
- **Constraint**: Personalized runtime instruction projections, including personalized `AGENTS.md`, must be generated under `~/.gal/generated` or provider-visible `.gal` projection paths, not written into the GAL source repository root.
- **Constraint**: This touches protected paths and changes every runtime surface; it requires explicit review and must preserve one trailing newline plus current generated-file semantics.
- **Expected saving**: Per-adapter size reduction proportional to excluded languages; adapters remain self-contained and valid.
- **Verify**: Regenerated adapters contain only the project's actual language conventions. Cross-runtime behavior is verified for all four adapter targets after regeneration.

---

### Step 6: Re-evaluate same-runtime bundling after context slimming lands

- **Files**: `commands/gal-pipeline/SKILL.template.md`, generated `commands/gal-pipeline/SKILL.md` when `Update-Commands` has produced it, pipeline agent contracts if needed
- **What**: Reassess whether same-runtime fallback should support an explicit opt-in bundled mode for implement, test, and review.
- **Default**: Do not make bundling the default because it weakens tester independence.
- **If accepted**: Mark the run as degraded verification independence and still require separate durable `## Test Results` and `## Review Results` subsections.
- **Verify**: The contract clearly distinguishes default same-runtime fallback from opt-in degraded bundling.

---

### Step 7: Enforce commit-boundary state convergence

- **Files**: `commands/gal-pipeline/SKILL.template.md`, generated `commands/gal-pipeline/SKILL.md` when `Update-Commands` has produced it, `.dev/state.md` semantics if needed
- **What**: Update the pipeline contract so any git commit that records task progress or task completion happens only after `docs/plans/<slug>.md`, `.dev/plans/<slug>.prompt.md`, and `.dev/state.md` agree on the relevant task state. Convergence may be delayed only inside an uncommitted in-flight task.
- **Constraint**: Do not introduce an implicit `Convergence Pending` state that can be committed. If a future explicit pending-state model is proposed, `status`, `whats-next`, and commit gates must be updated before it can ship.
- **Verify**: Before any pipeline commit, the three durable state surfaces are re-read and checked for agreement; no commit records source plan, execution prompt, and `.dev/state.md` drift.

---

## Files to Create or Modify

### Required authored changes

- `opencode.json` — remove duplicate generated-adapter startup loading for OpenCode.
- `agent/golem-implementer.agent.md` — remove routine generated-adapter reread; add fallback rule (`.dev/project.md` not `copilot-instructions.md`).
- `agent/golem-tester.agent.md` — preserve spec-driven verification while removing routine generated-adapter rereads; add fallback rule.
- `agent/golem-reviewer.agent.md` — shift to compact pipeline-bound context and remove routine generated-adapter rereads; add fallback rule.
- `agent/golem-security.agent.md` — keep audit scope task-scoped and avoid unnecessary broad rereads.
- `agent/golem-verifier.agent.md` — consume compact completion context while retaining goal-backward verification; add fallback rule.
- `scripts/gal.ps1` — emit compact pipeline-bound injected context; add `CONTEXT_CARRY` field; emit language-scoped convention injection.
- `scripts/gal.sh` — mirror the above behavior.
- `scripts/Test-PipelineTokenBurn.ps1` — repeatable verification for startup payload, task-scoped convention hints, delta-mode dispatch behavior, adapter slimming, and baked pipeline contract markers.

### Conditional or later-stage changes

- `scripts/Sync-DevContext.ps1` — selective language-scoped adapter embedding.
- `scripts/sync-dev-context.sh` — Bash-side mirror.
- `commands/gal-pipeline/SKILL.template.md` — same-runtime fallback wording, bundled-mode degraded marker, retry/protected-path/interruption/final-verifier safety gates, and commit-boundary convergence hard gate.
- `conventions/token-budget.md` — add a reusable lesson on language-scoped convention loading only if confirmed as shared methodology rather than repo-local tuning.
- `.dev/state.md` — only if commit-boundary convergence semantics require session-continuity wording changes.

### Generated validation outputs (do not edit directly)

- `AGENTS.md`, `.github/copilot-instructions.md`, `CLAUDE.md`, `GEMINI.md` — validate after any sync-script change; do not edit directly.
- `commands/*/SKILL.md` — local baked command output from `Update-Commands`; validate after generation when present, but source edits belong in `SKILL.template.md`.

## Tasks

- [x] T-001 — Remove duplicate OpenCode startup adapter loading so OpenCode uses `AGENTS.md` as its single startup instruction carrier.
  Verify: `opencode.json` contains exactly one `instructions` entry and it is `AGENTS.md`.
- [x] T-002 — Update pipeline-bound agent contracts to stop routine generated-adapter rereads and use `.dev/project.md` as the compact fallback when no runtime adapter is detectable.
  Verify: implementer, tester, reviewer, security, and verifier contracts contain the generated-adapter exclusion and `.dev/project.md` fallback rule.
- [x] T-003 — Add language-scoped convention selection and compact pipeline metadata to PowerShell and Bash dispatchers.
  Verify: dispatch output includes `CONTEXT_CARRY`, `PIPELINE_CONTEXT_MODE`, `PIPELINE_CONTEXT_FILES`, and `CONVENTION_HINTS`; task-scoped language hints win over project tech-stack fallback.
- [x] T-004 — Add selective language-scoped adapter embedding to PowerShell and Bash adapter generators.
  Verify: generated adapters include only the conventions overview, `token-budget.md`, `working-hours.md`, and project-relevant language convention files.
- [x] T-005 — Update `/gal pipeline` source contract with same-runtime degraded fallback, opt-in bundled-mode degraded marker, retry ceilings, protected-path escalation, interrupted-phase handoff, final verifier, and commit-boundary convergence gates.
  Verify: `commands/gal-pipeline/SKILL.template.md` contains those contract markers; generated `commands/gal-pipeline/SKILL.md` is validation output only when present.
- [ ] T-006 — Fix `scripts/Test-PipelineTokenBurn.ps1` so command-contract validation resolves the current repo layout first: prefer `commands/gal-pipeline/SKILL.template.md`, or generate/read local baked `commands/gal-pipeline/SKILL.md` through `Update-Commands` in an isolated validation flow.
  Verify: the test no longer fails solely because tracked source lacks `commands/gal-pipeline/SKILL.md`.
- [ ] T-007 — Re-run token-burn verification and record the current before/after metrics against the numeric targets.
  Verify: `scripts/Test-PipelineTokenBurn.ps1` passes and prints startup payload, single-task 3-phase carrier load, and five-task carrier load.
- [ ] T-008 — Run a bounded `/gal pipeline stop-at T-NNN` scenario against a disposable fixture plan/prompt and confirm implement, test, and review still write separate durable sections.
  Verify: fixture source plan, execution prompt, and `.dev/state.md` converge without recording a committed disagreement.
- [ ] T-009 — Confirm the slimming changes did not regress retry ceilings, protected-path escalation, interrupted-phase handoff, or final verifier behavior.
  Verify: static contract checks plus focused fixture or parser checks cover each safety marker.
- [ ] T-010 — Validate Bash parity for dispatcher and adapter generation on a host with Bash available, or record the host limitation explicitly.
  Verify: Bash dispatch emits the same compact fields and convention hints as PowerShell; Bash adapter generation mirrors PowerShell language-scoped embedding.

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | manual | Inspect `opencode.json` and confirm OpenCode loads only `AGENTS.md` at startup. | T-001 |
| TP-002 | static | Review the five pipeline-bound agent contracts and confirm they exclude routine generated-adapter rereads and use `.dev/project.md` as the fallback carrier. | T-002 |
| TP-003 | integration | Dispatch representative Go and C# task-scoped phases from both PowerShell and Bash lanes and confirm `CONTEXT_CARRY`, `PIPELINE_CONTEXT_MODE`, `PIPELINE_CONTEXT_FILES`, and `CONVENTION_HINTS` are emitted with task-language precedence. | T-003, T-010 |
| TP-004 | integration | Regenerate adapters through both sync paths and confirm only the conventions overview, `token-budget.md`, `working-hours.md`, and project-relevant language conventions are embedded. | T-004, T-010 |
| TP-005 | static | Inspect `commands/gal-pipeline/SKILL.template.md` and any baked `SKILL.md` output to confirm same-runtime degraded fallback wording, opt-in bundled-mode marker, retry ceilings, protected-path escalation, interrupted-phase handoff, final verifier, and commit-boundary convergence gates are present. | T-005, T-009 |
| TP-006 | script | Run `scripts/Test-PipelineTokenBurn.ps1` against the source-only repo layout and confirm command-contract validation resolves `SKILL.template.md` or isolated baked output instead of assuming tracked `commands/gal-pipeline/SKILL.md`. | T-006 |
| TP-007 | script | Re-run token-burn verification and record startup payload, single-task three-phase carrier load, and five-task carrier load against the numeric targets in this plan. | T-007 |
| TP-008 | integration | Execute a bounded disposable `/gal pipeline stop-at T-NNN` run and confirm implement, test, and review still write separate durable prompt sections while the source plan, execution prompt, and `.dev/state.md` converge before any recorded progress commit. | T-008 |
| TP-009 | static | Re-check the pipeline contract and any focused fixture output for retry ceilings, protected-path escalation, interrupted-phase handoff, and final verifier behavior after the token-burn changes. | T-009 |
| TP-010 | manual | If Bash is unavailable on the current host, record the explicit host limitation in the validation output instead of claiming parity. | T-010 |

## Risks

- If the OpenCode single-adapter change accidentally removes required runtime instructions, OpenCode behavior may drift. The fix must keep one authoritative adapter and verify behavior after the change.
- If agent docs stop reading generated adapters but `.dev/project.md` or source conventions are incomplete, project rules may be lost in pipeline mode. The `.dev/project.md` fallback rule mitigates this.
- If language detection in the dispatcher is inaccurate, the wrong conventions may be injected. The detection logic must be conservative: when ambiguous, include rather than exclude.
- If same-runtime bundling becomes the default, tester independence weakens and false confidence increases. Bundling must stay opt-in if it ships at all.
- If selective adapter embedding is done carelessly, cross-runtime behavior may diverge. The generator-layer change must be reviewed and validated through regenerated outputs.
- If convergence is delayed past a git commit boundary, the repository history can record inconsistent durable state. The pipeline must converge and re-read all three state surfaces before any commit that records task progress or completion.
- This plan touches protected paths under `commands/`, `conventions/`, `workflows/`, `templates/`, and sync scripts, so implementation should stay under reviewed planning rather than opportunistic edits.

## References

- Existing research: [docs/research/pipeline-multi-invocation-overhead.md](../research/pipeline-multi-invocation-overhead.md)
- Active workflow contract: [workflows/coding.md](../../workflows/coding.md)
- Token discipline contract: [conventions/token-budget.md](../../conventions/token-budget.md)
- Current pipeline contract source: [commands/gal-pipeline/SKILL.template.md](../../commands/gal-pipeline/SKILL.template.md)
- OpenCode bridge config: [opencode.json](../../opencode.json)
- OpenCode CLI stats reference: [opencode.ai/docs/cli](https://opencode.ai/docs/cli/)
- Architect analysis: [pipeline_token_burn_analysis.md](../../.gemini/antigravity/brain/aa206f41-de3a-42d3-bf73-3d50cefee377/pipeline_token_burn_analysis.md)

## Open Questions

- [x] OQ-001 — Is there a safe compact adapter format that all current runtimes can consume without losing cold-start behavior? Resolved by human decision: `AGENTS.md` remains the canonical shared instruction carrier, but provider-specific adapters such as `CLAUDE.md`, `GEMINI.md`, and `.github/copilot-instructions.md` must remain because providers may auto-read them and GAL cannot reliably disable that behavior. The compact-adapter direction is conservative: keep provider-specific files as generated runtime carriers, avoid hand-editing them, slim them only through generator-controlled selective embedding, and place personalized runtime projections under `~/.gal/generated` or provider-visible `.gal` projection paths rather than in the GAL source repo.
- [x] OQ-002 — Can state convergence be made incremental without introducing a new pending-state concept, or is explicit pending state required? Resolved by human decision: state convergence may be delayed inside an in-flight task, but all durable state surfaces must be synchronized before any git commit that records task progress or completion. No committed state may contain disagreement between `docs/plans/<slug>.md`, `.dev/plans/<slug>.prompt.md`, and `.dev/state.md`.
- [x] OQ-003 — Does OpenCode expose reliable token telemetry that can be used in a future iteration to replace heuristic task-boundary estimation? Resolved: The native `opencode stats` only provides aggregate data, but OpenCode stores raw session data locally (SQLite/JSON). Community tools (like CodeBurn) demonstrate that turn-level analysis is possible. Therefore, GAL should add its own phase markers and measurement wrapper to parse the local session data, rather than relying only on aggregate stats, to correlate usage precisely with pipeline task boundaries.

## Approval

- Human approval: [completed]
- Architect review: [not required for remaining T-006..T-010 validation/script-test tasks unless scope expands into protected paths]
- Additional domain review: [security review only if the implementation changes trust-boundary behavior]

## Review Results

### Engineering Review

#### Verdict: CLEAR

The plan is implementation-ready. Scope is narrowed to the remaining validation and script-hardening tasks, each task is independently testable, and the unresolved work does not require reopening architecture as long as it stays within the already-approved validation surface.

The current source plan already captures the key guardrails that must not regress: same-runtime degradation must stay explicit, commit-boundary convergence must hold across the three durable state surfaces, and generated adapters remain derived outputs only. The remaining gaps are bounded execution checks for fixture write-back, Bash-host parity, and command-layout validation in `scripts/Test-PipelineTokenBurn.ps1`, not missing design decisions.

Return to `/deep-planning` only if the remaining work expands beyond validation into new protected-path behavior, changes cross-runtime semantics, or introduces a default bundled execution mode.

<!-- ENG_REVIEW: CLEAR -->
