# Plan: GAL Pipeline Token Burn Reduction

## Goal

Reduce `/gal pipeline` token consumption without weakening GAL's safety guarantees, and specifically prevent OpenCode Go daily and weekly quota from being exhausted too early by redundant startup context, repeated phase context loading, and avoidable same-runtime overhead.

## Measured Burn Surface

Before executing any step, the following concrete sizes were verified:

| File | Lines | Bytes | Notes |
|------|-------|-------|-------|
| `AGENTS.md` | 1,121 | 57,284 | Generated adapter |
| `.github/copilot-instructions.md` | 1,069 | 53,923 | Generated adapter |
| `CLAUDE.md` | 1,121 | 57,203 | Generated adapter |
| `GEMINI.md` | 1,121 | 57,313 | Generated adapter |
| `golem-implementer.agent.md` | 220 | 11,199 | Reads copilot-instructions.md at L30 |
| `golem-tester.agent.md` | 319 | 11,082 | Reads copilot-instructions.md at L39 |
| `golem-reviewer.agent.md` | 285 | 11,316 | Reads copilot-instructions.md at L30 |
| `golem-verifier.agent.md` | 188 | 7,448 | |
| `.dev/project.md` | 98 | 5,450 | Actual source of truth |

The four generated adapters are near-identical clones. Only the first ~10 lines differ (adapter-specific header); the remainder is verbatim concatenation of `.dev/project.md` + all six convention files. OpenCode startup loads two of these (~111 KB of duplicated content). The agent contracts instruct a third read of `copilot-instructions.md` per phase, in direct violation of the repo's own `conventions/token-budget.md` generated-artifact exclusion rule.

## Success Criteria (Numeric)

| Metric | Current | Target |
|--------|---------|--------|
| OpenCode startup payload | ~111 KB (2 adapters) | ≤60 KB (1 adapter) |
| Single-task 3-phase total context load | ~270 KB | ≤80 KB |
| 5-task pipeline total redundant context | ~1,071 KB | ≤200 KB |

## Requirements

- [ ] `/gal pipeline` must continue to honor the existing safety model: implement, test, review, conditional security, verifier, retry ceilings, interrupted-phase handoff, and protected-path escalation must remain intact.
- [ ] OpenCode must stop loading duplicate large generated adapters at startup; the repo-local OpenCode bridge should load a single authoritative instruction carrier.
- [ ] Pipeline-bound golem agents must stop re-reading generated adapters during normal phase execution unless the task is explicitly about adapter content.
- [ ] Same-runtime fallback must not silently collapse spec-driven testing and review independence by default just to save tokens.
- [ ] Any same-runtime bundling optimization must be explicit, documented as degraded verification independence, and must still produce separate durable write-back for test and review.
- [ ] Three-surface durable state convergence across `docs/plans/<slug>.md`, `.dev/plans/<slug>.prompt.md`, and `.dev/state.md` must remain the baseline contract unless a new explicit pending-state model is introduced.
- [ ] Dispatch and agent changes must favor compact injected context and on-demand reads over repeated full cold-start loading.
- [ ] Generated adapter content should be compressed at the generator layer using selective language-scoped embedding rather than full-body duplication or pointer indirection.
- [ ] Changes must preserve cross-runtime alignment across Copilot, Antigravity CLI, Codex CLI, Claude Code, and the OpenCode bridge lane.
- [ ] Generated adapters remain derived outputs only; any fix must be authored in source files and sync scripts, not by hand-editing generated adapter files.
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
- **What**: Generate adapters by reading the `Tech Stack` section of `.dev/project.md` to determine which language conventions to embed. Only embed conventions for languages the project actually uses, plus the two cross-language conventions.
- **Constraint**: Do not replace convention bodies with pointers or indexes — adapters must remain self-contained. The saving comes from selective inclusion, not indirection.
- **Constraint**: This touches protected paths and changes every runtime surface; it requires explicit review and must preserve one trailing newline plus current generated-file semantics.
- **Expected saving**: Per-adapter size reduction proportional to excluded languages; adapters remain self-contained and valid.
- **Verify**: Regenerated adapters contain only the project's actual language conventions. Cross-runtime behavior is verified for all four adapter targets after regeneration.

---

### Step 6 (Later): Re-evaluate same-runtime bundling only after context slimming lands

- **Files**: `commands/gal-pipeline/SKILL.template.md`, `commands/gal-pipeline/SKILL.md`, pipeline agent contracts if needed
- **What**: Reassess whether same-runtime fallback should support an explicit opt-in bundled mode for implement, test, and review.
- **Default**: Do not make bundling the default because it weakens tester independence.
- **If accepted**: Mark the run as degraded verification independence and still require separate durable `## Test Results` and `## Review Results` subsections.
- **Verify**: The contract clearly distinguishes default same-runtime fallback from opt-in degraded bundling.

---

### Step 7 (Later): Revisit batch state convergence last

- **Files**: `commands/gal-pipeline/SKILL.template.md`, `commands/gal-pipeline/SKILL.md`, `.dev/state.md` semantics if needed
- **What**: Only after prior reductions land, evaluate whether convergence can move to an explicit `Convergence Pending` model for invocation-end or N-task batching.
- **Constraint**: `status` and `whats-next` semantics must be updated before batching is allowed.
- **Verify**: Pending convergence is represented explicitly and remains resumable, not mistaken for success or drift.

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

### Conditional or later-stage changes

- `scripts/Sync-DevContext.ps1` — selective language-scoped adapter embedding after first-wave fixes are validated.
- `scripts/sync-dev-context.sh` — Bash-side mirror.
- `conventions/token-budget.md` — add a reusable lesson on language-scoped convention loading only if confirmed as shared methodology rather than repo-local tuning.
- `.dev/state.md` — only if a `Convergence Pending` model is introduced.

### Generated validation outputs (do not edit directly)

- `AGENTS.md`, `.github/copilot-instructions.md`, `CLAUDE.md`, `GEMINI.md` — validate after any sync-script change; do not edit directly.

## Test Plan

- [ ] Measure OpenCode startup payload before and after Step 1. Confirm ≤60 KB target is met.
- [ ] Confirm pipeline-bound agent contracts no longer require routine reads of generated adapters after Step 2.
- [ ] Confirm each agent contract includes the `.dev/project.md` fallback rule (not `copilot-instructions.md`).
- [ ] Run a Go task through the dispatcher and confirm the injected context contains only `go.md`, `token-budget.md`, and `working-hours.md` conventions after Step 3.
- [ ] Run a C# task and confirm no Go, Rust, or TypeScript convention content appears.
- [ ] Run a bounded `/gal pipeline stop-at T-NNN` scenario and confirm implement, test, and review still produce separate durable write-back.
- [ ] Verify `CONTEXT_CARRY` field appears in dispatch output and that Phase 2/3 receive delta-only injection when applicable after Step 4.
- [ ] After any sync-script change, regenerate adapters and verify they still contain valid adapter rules, the expected language conventions, and the two cross-language conventions.
- [ ] Compare single-task 3-phase total context load before and after. Confirm ≤80 KB target is met.
- [ ] Compare 5-task pipeline total redundant context before and after. Confirm ≤200 KB target is met.
- [ ] Confirm no regression to retry ceilings, protected-path escalation, interrupted-phase handoff, or final verifier behavior.

## Risks

- If the OpenCode single-adapter change accidentally removes required runtime instructions, OpenCode behavior may drift. The fix must keep one authoritative adapter and verify behavior after the change.
- If agent docs stop reading generated adapters but `.dev/project.md` or source conventions are incomplete, project rules may be lost in pipeline mode. The `.dev/project.md` fallback rule mitigates this.
- If language detection in the dispatcher is inaccurate, the wrong conventions may be injected. The detection logic must be conservative: when ambiguous, include rather than exclude.
- If same-runtime bundling becomes the default, tester independence weakens and false confidence increases. Bundling must stay opt-in if it ships at all.
- If selective adapter embedding is done carelessly, cross-runtime behavior may diverge. The generator-layer change must be reviewed and validated through regenerated outputs.
- If convergence batching is introduced without status semantics, `status` and `whats-next` may misread pending work as drift or completion.
- This plan touches protected paths under `commands/`, `conventions/`, `workflows/`, `templates/`, and sync scripts, so implementation should stay under reviewed planning rather than opportunistic edits.

## References

- Existing research: [docs/research/pipeline-multi-invocation-overhead.md](../research/pipeline-multi-invocation-overhead.md)
- Active workflow contract: [workflows/coding.md](../../workflows/coding.md)
- Token discipline contract: [conventions/token-budget.md](../../conventions/token-budget.md)
- Current pipeline contract: [commands/gal-pipeline/SKILL.md](../../commands/gal-pipeline/SKILL.md)
- OpenCode bridge config: [opencode.json](../../opencode.json)
- Architect analysis: [pipeline_token_burn_analysis.md](../../.gemini/antigravity/brain/aa206f41-de3a-42d3-bf73-3d50cefee377/pipeline_token_burn_analysis.md)

## Open Questions

- [ ] OQ-001 — Is there a safe compact adapter format that all current runtimes can consume without losing cold-start behavior? (Relevant to Step 5 selective embedding.)
- [ ] OQ-002 — Can state convergence be made incremental without introducing a new pending-state concept, or is explicit pending state required? (Relevant to Step 7.)
- [ ] OQ-003 — Does OpenCode expose reliable token telemetry that can be used in a future iteration to replace heuristic task-boundary estimation?

## Approval

- Human approval: [pending]
- Architect review: [required]
- Additional domain review: [security review only if the implementation changes trust-boundary behavior]

## Review Results

### Engineering Review

Pending.
