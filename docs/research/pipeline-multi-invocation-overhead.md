# Research: Pipeline Multi-Invocation Overhead

Date: 2026-05-25
Updated: 2026-05-25 — P0 and P0b implemented; steps removed entirely from opencode.json; document updated to reflect final state

## Question

Why does `/gal pipeline` require repeated manual re-invocation to complete a multi-task plan, and can this overhead be reduced without sacrificing GAL's safety guarantees?

## Background

During field use of `/gal pipeline` on the GAL repository itself (plans `feat-gal-bootstrap-installer-distribution` and `feat-gal-install-mode-plugin-distribution`), the pipeline consistently stopped after completing at most one task per invocation. A plan with N tasks required the user to manually type `/gal pipeline` at least N times, plus additional invocations when test or review retries were needed. This report traces the root causes through the source contracts and identifies optimization opportunities.

## Sources

- `commands/gal-pipeline/SKILL.md` — pipeline orchestrator definition (503 lines)
- `agent/golem-implementer.agent.md` — implementer agent contract (219 lines)
- `agent/golem-tester.agent.md` — tester agent contract (318 lines)
- `agent/golem-reviewer.agent.md` — reviewer agent contract (284 lines)
- `agent/golem-verifier.agent.md` — verifier agent contract (187 lines)
- `agent/golem-security.agent.md` — security audit agent contract (123 lines)
- `scripts/gal.ps1` — PowerShell dispatch script (671 lines)
- `opencode.json` — OpenCode runtime configuration (138 lines)
- `conventions/token-budget.md` — token and context management rules
- `workflows/coding.md` — coding flow workflow definition
- `model-roles.md` — model role assignment and routing rules
- `.dev/state.md` — current repo state (3 active plans)

## Root Cause Analysis

### Cause 1: Single-Task Tranche Mode Is the Default (MITIGATED)

**Location**: `commands/gal-pipeline/SKILL.md` lines 79-91

The pipeline originally entered "single-task tranche mode" by default when running under OpenCode or any runtime with a visible step budget. The original contract text:

> In OpenCode, enter **single-task tranche mode** by default. Only disable it when the user explicitly asks for a multi-task turn and the visible active-agent `steps` budget is high enough for that larger run.

After completing one task (implement + test + review + optional security), the pipeline stopped cleanly and told the user to rerun `/gal pipeline`. This was documented as "a normal continuation strategy, not a BLOCKED state."

**Why this existed**: The design prevented a low-step agent from being cut off mid-implementation on a subsequent task. If the runtime had 40 steps and a single task consumed 28-35 steps, attempting a second task risked an ungraceful cutoff that left the worktree dirty and the plan state inconsistent.

**Impact before mitigation**: A plan with 5 tasks required at minimum 5 manual invocations. With test or review retries, the count increased further.

**Mitigation (P0b, implemented 2026-05-25)**: The contract now reads:

> If the active OpenCode agent's `steps` value is **60 or higher**, do NOT enter single-task tranche mode — proceed with multi-task execution the same way non-OpenCode runtimes do.

Combined with P0 (build steps raised to 80), the pipeline now runs multiple tasks per invocation under OpenCode, matching the behavior of Claude and ChatGPT-5.4 runtimes.

### Cause 2: Step Budget Is Too Low for Multi-Task Execution (MITIGATED)

**Location**: `opencode.json` line 80

The original `build` agent configuration:

```json
"build": {
  "steps": 40
}
```

A single task cycle in OpenCode consumes approximately:

| Phase | Step Consumption | Breakdown |
| --- | --- | --- |
| Implement | 10-15 steps | Read plan + read context files + write code + git commit + update status |
| Test | 8-12 steps | Read spec + write tests + execute tests + write results to plan |
| Review | 6-10 steps | Read diff + analyze + write results to plan |
| State convergence | 4-6 steps | Update 3 files (source plan, execution prompt, state.md) + re-read verification |
| **Total per task** | **28-43 steps** | |

With 40 steps, the pipeline could barely complete one full task cycle. There was no headroom for a second task, so single-task tranche mode was the correct and safe default under this configuration.

**Mitigation (P0, implemented 2026-05-25)**: `build` steps raised to 80. All other agent step budgets also raised (`plan` 8→16, `general` 10→15, `explore` 6→8) based on analysis of actual command step consumption. See Appendix A for the full before/after comparison.

### Cause 3: Repeated Context Loading Across Dispatches

Each golem agent's `<project_context>` section requires reading the same foundational files on every invocation:

| File | Read by | Purpose |
| --- | --- | --- |
| `.dev/plans/<slug>.prompt.md` | All pipeline golems | Execution spec and status |
| `.dev/project.md` | All pipeline golems | Project architecture and conventions |
| `.dev/state.md` | All pipeline golems | Active plan index and session continuity |
| `.github/copilot-instructions.md` | Implementer, tester | Project rules |
| Language conventions | Implementer, tester | Coding standards |

For a single task, these files are read 3 times (once per phase dispatch). For N tasks, they are read 3N times. Each read costs 1-2 steps, meaning 6-12 steps per task are spent on redundant context loading alone.

### Cause 4: Per-Task State Convergence Gate

**Location**: `commands/gal-pipeline/SKILL.md` lines 375-385

After every task passes all gates, the pipeline must:

1. Mark `T-NNN` complete in the execution prompt `## Tasks`
2. Mark `T-NNN` complete in the source plan `## Tasks`
3. Update execution prompt `## Status` (clear current task, reset counters)
4. Update `.dev/state.md` session continuity row
5. Re-read all three files
6. Verify convergence: source plan has `- [x] T-NNN`, execution prompt has `- [x] T-NNN`, state.md no longer points at the completed task

This gate consumes 4-6 steps per task. It exists to prevent state drift between the three durable surfaces, but it is executed per-task rather than per-pipeline-run.

### Cause 5: No Auto-Continue Mechanism

The pipeline has no built-in way to automatically re-invoke itself after a tranche completes. The user must manually type `/gal pipeline` each time. There is no `opencode.json` command alias or loop construct that would allow the runtime to continue without human input.

### Cause 6: Same-Runtime Model Fallback Still Splits Dispatches

**Location**: `commands/gal-pipeline/SKILL.md` lines 66-77

When the active runtime cannot enforce per-agent model routing (which is the case for OpenCode subagents that inherit the primary agent model), the pipeline degrades to "same-runtime fallback." However, even in this fallback mode, it still dispatches implementer, tester, and reviewer as separate invocations. Each dispatch pays the full context-loading cost without gaining the independent-verification benefit that separate models would provide.

The OpenCode-specific rule states:

> Different models only count when the active OpenCode configuration assigns agent-specific `model` values. Inherited subagent execution under the same primary agent model does not satisfy independent verification.

This means the pipeline is paying the overhead of multi-dispatch without receiving the verification quality that justifies that overhead.

## Complete Stop Conditions Reference

Every condition that causes `/gal pipeline` to stop and require re-invocation:

| # | Condition | Location | Human Action Required? | Frequency in Practice |
| --- | --- | --- | --- | --- |
| 1 | Single-task tranche complete | SKILL.md:389 | Rerun `/gal pipeline` | **Every task** (primary bottleneck) |
| 2 | Runtime step limit reached | SKILL.md:81-92 | Rerun `/gal pipeline` | Occasional (when steps exhaust mid-phase) |
| 3 | Test retry count reaches 3 | SKILL.md:285 | Inspect failures, fix manually | Rare |
| 4 | Review retry count reaches 3 | SKILL.md:310 | Inspect BLOCKING findings, fix manually | Rare |
| 5 | Security vulnerability or Protected Path violation | SKILL.md:312 | Human review required | Rare |
| 6 | Conditional security audit leaves high/critical findings | SKILL.md:343 | Human remediation required | Rare |
| 7 | Working-hours boundary active | SKILL.md:220-225 | Confirm wrap-up or override | Time-dependent |
| 8 | `stop-at T-NNN` reached | SKILL.md:387 | User decides to continue | User-specified |
| 9 | State convergence check fails | SKILL.md:375-385 | Fix inconsistent state | Rare |
| 10 | xmachine phase returns non-success | SKILL.md:262 | Inspect remote artifacts | Only in xmachine mode |
| 11 | Missing write-back (no test/review subsection) | SKILL.md:278-279, 303-304 | Investigate golem failure | Rare |

Conditions 3-11 are legitimate safety stops that should remain. Condition 1 (single-task tranche) is now mitigated by P0+P0b: when `build` steps ≥ 60, the pipeline no longer enters single-task tranche mode. Condition 2 (runtime step limit) remains possible but is less likely with 80 steps; the interrupted-phase handoff handles it gracefully when it occurs.

## Quantified Impact

### Before P0+P0b (build steps = 40, single-task tranche always on)

For a plan with 5 tasks under the original configuration:

| Metric | Value |
| --- | --- |
| Minimum invocations of `/gal pipeline` | 5 (one per task) |
| Typical invocations (with 1-2 retries) | 6-8 |
| Steps consumed per invocation | 28-40 |
| Steps spent on redundant context loading | 6-12 per task (across 3 dispatches) |
| Steps spent on per-task state convergence | 4-6 per task |
| Total steps for 5 tasks | 140-200 (across 5+ invocations) |
| Theoretical minimum (single invocation, no redundancy) | ~80-120 steps |

The overhead from redundant context loading and per-task convergence alone accounted for approximately 30-40% of total step consumption.

### After P0+P0b (steps removed, auto-disable tranche when undefined)

For a plan with 5 tasks under the current configuration:

| Metric | Value |
| --- | --- |
| Minimum invocations of `/gal pipeline` | **1** (all tasks in one invocation) |
| Typical invocations (with 1-2 retries) | **1-2** (retries may exhaust context, requiring resume) |
| Steps consumed per invocation | Unlimited (model runs until it stops) |
| Steps spent on redundant context loading | 6-12 per task (unchanged — P1 would address this) |
| Steps spent on per-task state convergence | 4-6 per task (unchanged — P2 would address this) |
| Total steps for 5 tasks | 140-200 (across 1-2 invocations) |

The total step count is unchanged (same work is done), but the number of manual invocations drops from 5+ to 1 because the pipeline no longer stops after every task.

## Optimization Proposals

### P0: Remove Agent Step Budgets ✅ IMPLEMENTED

**Change**: In `opencode.json`, remove all `steps` values from all agent types.

**Implemented values** (2026-05-25):

| Agent | Before | After | Rationale |
| --- | --- | --- | --- |
| `build` | 40 → 80 → **removed** | undefined | No step limit; agent runs until model stops or user interrupts, matching Claude/Codex behavior |
| `plan` | 8 → 16 → **removed** | undefined | No step limit; `/deep-planning` can complete without artificial cutoff |
| `general` | 10 → 15 → **removed** | undefined | No step limit; `/gal wrap-up` and other commands have full headroom |
| `explore` | 6 → 8 → **removed** | undefined | No step limit; exploration is unconstrained |

**Effect**: All agents now behave identically to Claude Code and Codex — they continue executing until the model chooses to stop or the user interrupts. The pipeline no longer needs single-task tranche mode as a safety mechanism because there is no step budget to exhaust.

**Risk**: Low. The pipeline's own safety stops (retry ceilings, security escalation, working hours, convergence gates) remain intact and are the primary control mechanism. The only new risk is that a runaway agent could consume more tokens than expected, but this is the same risk profile as Claude Code and Codex.

**Limitation**: This is a machine-local configuration change, not a portable contract change. Each repo's `opencode.json` must be adjusted independently.

### P0b: Auto-Disable Single-Task Tranche When Steps Are Unlimited or Sufficient ✅ IMPLEMENTED

**Change**: In `commands/gal-pipeline/SKILL.md` lines 85-86, change the Runtime Step-Budget Preflight logic to handle three cases:

1. `steps` is **not set** (undefined) → do NOT enter single-task tranche mode (multi-task execution)
2. `steps` is set and **≥ 60** → do NOT enter single-task tranche mode (multi-task execution)
3. `steps` is set and **< 60** → enter single-task tranche mode (safety fallback for low-budget configs)

**Before** (original contract text):

> In OpenCode, enter **single-task tranche mode** by default. Only disable it when the user explicitly asks for a multi-task turn and the visible active-agent `steps` budget is high enough for that larger run.

**After** (implemented contract text):

> If the active OpenCode agent's `steps` value is **not set** (undefined), do NOT enter single-task tranche mode — proceed with multi-task execution the same way non-OpenCode runtimes do. If `steps` is set and is **60 or higher**, also proceed with multi-task execution. If `steps` is set and below 60, enter single-task tranche mode by default.

**Why this matters**: P0 alone (removing step limits) was insufficient because the original contract required the pipeline to check for a numeric step budget. With `steps` removed, the preflight logic needed to recognize "undefined" as equivalent to "unlimited" and skip single-task tranche mode accordingly.

**Effect**: OpenCode with no `steps` limit now behaves identically to Claude and ChatGPT-5.4 runtimes — the pipeline runs multiple tasks in a single invocation without requiring manual re-invocation.

**Risk**: Low. The interrupted-phase handoff mechanism (SKILL.md:186-213) still handles runtime cutoffs gracefully if they occur for other reasons (context window exhaustion, provider turn limits, etc.).

### P1: Merge Dispatches Under Same-Runtime Fallback

**Change**: When the pipeline detects that it is running under same-runtime fallback (no per-agent model routing), merge the implement + test + review phases into a single dispatch that runs all three sequentially within one context.

**Effect**: Eliminates 2 of 3 context-loading cycles per task. Saves approximately 6-12 steps per task.

**Risk**: Medium. Requires modifying `commands/gal-pipeline/SKILL.md` to add a same-runtime fast path. The merged dispatch must still produce the same durable write-back (separate `## Test Results` and `## Review Results` subsections). The verification quality is no worse than the current fallback (which already runs all phases under the same model), but the contract must explicitly acknowledge this trade-off.

**Implementation sketch**:

```text
In Step 2, after Runtime Step-Budget Preflight:
  If same-runtime fallback is active:
    - Run implement, test, and review as a single sequential dispatch
    - The dispatch target reads the plan once, implements, then tests, then reviews
    - Each phase still writes its own subsection to the execution prompt
    - State convergence still runs after the combined dispatch
  Else:
    - Follow the existing multi-dispatch path
```

### P2: Batch State Convergence

**Change**: Defer state convergence from per-task to per-invocation (or per-N-tasks). After each task, update only the execution prompt. After the last task in the current invocation (or after every N tasks), run the full three-file convergence gate.

**Effect**: Saves 4-6 steps per task for all but the last task in an invocation. For a 5-task plan run in a single invocation, this saves 16-24 steps.

**Risk**: Medium. If the runtime is interrupted before convergence runs, the source plan and `state.md` may be stale relative to the execution prompt. The interrupted-phase handoff already handles this scenario, but the contract must document that convergence is deferred rather than immediate.

**Implementation sketch**:

```text
In Step 2g (Mark Task Complete And Converge State):
  After marking T-NNN complete in the execution prompt:
    If this is the last task in the current invocation, or N tasks have completed since last convergence:
      Run full three-file convergence gate (existing logic)
    Else:
      Skip convergence for now
      Record "Convergence pending: T-NNN through T-NNN+K" in execution prompt ## Status
```

### P3: Pipeline-Level Context Prefetch

**Change**: Before entering the task loop, read all shared context files once and pass a summary to each golem dispatch, rather than having each golem read them independently.

**Effect**: Eliminates most redundant reads. Saves approximately 4-8 steps per task (after the first).

**Risk**: High. Requires changing the agent interface: each golem's `<project_context>` section currently specifies its own read list. Adding a "prefetched context" input channel means modifying all pipeline agent contracts and the dispatch mechanism. There is also a risk that the prefetched summary becomes stale if the plan is updated mid-task.

**Implementation complexity**: Significant. Requires changes to:
- `commands/gal-pipeline/SKILL.md` (prefetch logic)
- `agent/golem-implementer.agent.md` (accept prefetched context)
- `agent/golem-tester.agent.md` (accept prefetched context)
- `agent/golem-reviewer.agent.md` (accept prefetched context)
- `scripts/gal.ps1` (dispatch mechanism to carry prefetched data)

### P4: Auto-Continue Command

**Change**: Add an `opencode.json` command alias that detects an incomplete pipeline and automatically re-invokes `/gal pipeline` without requiring manual input.

**Effect**: Eliminates the manual re-typing overhead. The user still sees the tranche-completion messages but does not need to act on them.

**Risk**: Low for the command alias itself. However, auto-continue could mask legitimate stop conditions if the user is not paying attention. The implementation should require the user to confirm the first auto-continue and then allow subsequent ones to proceed automatically within the same session.

**Implementation sketch**:

```json
"command": {
  "gal-pipeline-continue": {
    "description": "Continue the active pipeline from the last completed task",
    "template": "Check .dev/state.md for the active plan. If the plan has unchecked tasks, run /gal pipeline. If the plan is complete, report completion."
  }
}
```

## Priority Matrix

| Priority | Proposal | Step Savings (per task) | Implementation Effort | Risk | Portable? | Status |
| --- | --- | --- | --- | --- | --- | --- |
| P0 | Increase step budget | Enables multi-task (indirect) | Trivial (config change) | Low | No (machine-local) | ✅ Done |
| P0b | Auto-disable tranche at ≥ 60 steps | Eliminates manual multi-task request | Low (SKILL.md) | Low | Yes | ✅ Done |
| P1 | Merge dispatches under same-runtime fallback | 6-12 steps | Medium (SKILL.md + agent contracts) | Medium | Yes | Pending |
| P2 | Batch state convergence | 4-6 steps (all but last task) | Medium (SKILL.md) | Medium | Yes | Pending |
| P3 | Pipeline-level context prefetch | 4-8 steps (after first task) | High (all pipeline agents + dispatch) | High | Yes | Pending |
| P4 | Auto-continue command | Eliminates manual re-invocation | Low (opencode.json) | Low | No (runtime-specific) | Pending |

## Recommended Implementation Order

1. **P0 ✅ DONE** — changed `opencode.json` `steps` values: build 40→80, plan 8→16, general 10→15, explore 6→8.
2. **P0b ✅ DONE** — changed `commands/gal-pipeline/SKILL.md` preflight logic to auto-disable single-task tranche when steps ≥ 60.
3. **P4 next** — add the auto-continue command alias. This eliminates the remaining manual re-invocation edge case when steps do exhaust mid-pipeline.
4. **P1 after P4** — merge dispatches under same-runtime fallback. This is the highest-value portable optimization because it eliminates the largest source of redundant step consumption without requiring agent interface changes.
5. **P2 after P1** — batch state convergence. This compounds the savings from P1 by reducing per-task overhead further.
6. **P3 last** — pipeline-level context prefetch. Only pursue if P1+P2 savings are insufficient, given the high implementation cost and risk.

## Open Questions

1. **What is the optimal step budget?** The original 40-step limit was chosen conservatively. Field data from actual pipeline runs would help determine the real step consumption distribution per task type (documentation-only tasks vs. code-heavy tasks vs. tasks requiring browser QA). The current 80-step budget is based on conservative estimates; actual field data may show it can be lowered or should be raised.
2. ~~**Should single-task tranche mode be opt-in rather than opt-out?**~~ **RESOLVED** — P0b implemented: single-task tranche mode is now auto-disabled when step budget ≥ 60, making it effectively opt-in for well-configured runtimes. It remains the default only for low-budget configurations where it is a genuine safety measure.
3. **How does the same-runtime fallback affect verification quality in practice?** The pipeline contract acknowledges that same-runtime fallback does not provide independent verification, but it still pays the dispatch overhead. Field data on whether same-runtime test+review catches real bugs would inform whether P1's merged dispatch is acceptable.
4. **Can the convergence gate be made incremental?** Instead of re-reading all three files after every task, the pipeline could track which surfaces were modified and only re-verify those. This would reduce convergence cost without deferring it entirely.
5. **Should the step-budget threshold (60) be configurable?** Currently hardcoded in the SKILL.md contract. If different repos or task types consistently need different thresholds, this could become a `.dev/project.md` or `opencode.json` setting.

## Appendix A: opencode.json Agent Configuration

### Before (original)

```json
"agent": {
  "build": {
    "steps": 40
  },
  "plan": {
    "steps": 8
  },
  "general": {
    "steps": 10
  },
  "explore": {
    "steps": 6
  }
}
```

### After (implemented 2026-05-25)

```json
"agent": {
  "build": {
    "permission": { "bash": { ... } }
  },
  "plan": {},
  "general": {},
  "explore": {}
}
```

All `steps` fields have been removed. Agents now run until the model chooses to stop or the user interrupts, matching Claude Code and Codex behavior.

### Rationale for removal

| Agent | Original | Why removed |
| --- | --- | --- |
| `build` | 40 | Pipeline: single task consumes 28-43 steps; 40 was barely enough for one task. Removing the limit allows the pipeline to run all tasks in one invocation. |
| `plan` | 8 | `/deep-planning` requires 9-12 steps; 8 was insufficient. Removing the limit allows full planning passes. |
| `general` | 10 | `/gal wrap-up` requires 6-8 steps; 10 had no retry headroom. Removing the limit gives full headroom. |
| `explore` | 6 | Minor buffer for slightly deeper searches. Removing the limit allows unconstrained exploration. |

### Step consumption by command (for reference)

| Command | Agent | Min Steps | Old Budget | New Budget | Verdict |
| --- | --- | --- | --- | --- | --- |
| `/gal pipeline` | `build` | 28-43/task | 40 ❌ | unlimited ✅ | Multi-task now feasible |
| `/planning` | `plan` | 5-6 | 8 ✅ | unlimited ✅ | Unconstrained |
| `/deep-planning` | `plan` | 9-12 | 8 ❌ | unlimited ✅ | Unconstrained |
| `/refining-plan` | `plan` | 5 | 8 ✅ | unlimited ✅ | Unconstrained |
| `/plan-to-prompt` | `plan` | 6-7 | 8 ⚠️ | unlimited ✅ | Unconstrained |
| `/gal status` | `general` | 3-4 | 10 ✅ | unlimited ✅ | Unconstrained |
| `/gal whats-next` | `general` | 3-4 | 10 ✅ | unlimited ✅ | Unconstrained |
| `/gal wrap-up` | `general` | 6-8 | 10 ⚠️ | unlimited ✅ | Unconstrained |
| `/gal init` | `general` | 1-2 | 10 ✅ | unlimited ✅ | Unconstrained |

## Appendix B: Pipeline Phase Dispatch Flow

### Before optimization (original, build steps = 40)

The dispatch flow for a single task under `/gal pipeline`:

```text
/gal pipeline
  │
  ├─ Step 1: Read plan + verify prerequisites (2-3 steps)
  │
  ├─ Step 2a: Working hours check (1 step)
  │
  ├─ Step 2b: Update cursor in execution prompt (1-2 steps)
  │
  ├─ Step 2c: Implement
  │   ├─ gal.ps1 dispatch golem-implementer (1 step)
  │   ├─ Implementer reads: execution prompt, project.md, state.md, conventions (4-6 steps)
  │   ├─ Implementer writes code (2-4 steps)
  │   ├─ Implementer commits + updates status (2-3 steps)
  │   └─ Subtotal: 9-14 steps
  │
  ├─ Step 2d: Test
  │   ├─ Update Workflow: TEST in execution prompt (1 step)
  │   ├─ gal.ps1 dispatch golem-tester (1 step)
  │   ├─ Tester reads: execution prompt, project.md, state.md, public API (4-6 steps)
  │   ├─ Tester writes + runs tests (2-4 steps)
  │   ├─ Tester writes results to execution prompt (1-2 steps)
  │   └─ Subtotal: 9-14 steps
  │
  ├─ Step 2e: Review
  │   ├─ gal.ps1 dispatch golem-reviewer (1 step)
  │   ├─ Reviewer reads: execution prompt, project.md, state.md, diff (4-6 steps)
  │   ├─ Reviewer writes results to execution prompt (1-2 steps)
  │   └─ Subtotal: 6-9 steps
  │
  ├─ Step 2g: State convergence
  │   ├─ Update execution prompt tasks + status (1-2 steps)
  │   ├─ Update source plan tasks (1 step)
  │   ├─ Update state.md session continuity (1 step)
  │   ├─ Re-read all 3 files (3 steps)
  │   ├─ Verify convergence (1 step)
  │   └─ Subtotal: 7-8 steps
  │
  └─ STOP (single-task tranche complete)
      Total: 35-51 steps (exceeds 40-step budget in worst case)
```

### After P0+P0b (steps removed, auto-disable tranche when undefined)

```text
/gal pipeline
  │
  ├─ Step 1: Read plan + verify prerequisites (2-3 steps)
  │
  ├─ Task 1:
  │   ├─ Working hours check + update cursor (2-3 steps)
  │   ├─ Implement (9-14 steps)
  │   ├─ Test (9-14 steps)
  │   ├─ Review (6-9 steps)
  │   ├─ State convergence (7-8 steps)
  │   └─ Subtotal: 33-48 steps
  │
  ├─ Task 2: (continues automatically — no STOP)
  │   ├─ Working hours check + update cursor (2-3 steps)
  │   ├─ Implement (9-14 steps)
  │   ├─ Test (9-14 steps)
  │   ├─ Review (6-9 steps)
  │   ├─ State convergence (7-8 steps)
  │   └─ Subtotal: 33-48 steps
  │
  ├─ Task 3: (continues automatically — no STOP)
  │   └─ ... (same pattern)
  │
  ├─ ... (continues until all tasks complete or model chooses to stop)
  │
  └─ Post-loop verifier + final gate
      Total: unlimited — pipeline runs all tasks in one invocation
      If context window exhausts or provider cutoff occurs: interrupted-phase handoff resumes on next invocation
```

## Appendix C: Proposed Optimized Flow (P1 + P2 Combined)

This appendix describes the future state if P1 (merge dispatches) and P2 (batch convergence) are implemented on top of the already-completed P0+P0b changes.

```text
/gal pipeline
  │
  ├─ Step 1: Read plan + verify prerequisites (2-3 steps)
  │  └─ Also prefetch: project.md, state.md, conventions (included above)
  │
  ├─ Task loop (multi-task when step budget allows):
  │   │
  │   ├─ Update cursor (1 step)
  │   │
  │   ├─ Combined implement + test + review dispatch (same-runtime fallback)
  │   │   ├─ gal.ps1 dispatch golem-implementer --combined-phases (1 step)
  │   │   ├─ Read context once (4-6 steps, not 12-18)
  │   │   ├─ Implement (2-4 steps)
  │   │   ├─ Test (2-4 steps)
  │   │   ├─ Review (2-3 steps)
  │   │   ├─ Write all results to execution prompt (2-3 steps)
  │   │   └─ Subtotal: 13-21 steps (vs. 24-37 in current flow)
  │   │
  │   ├─ Update execution prompt only (1-2 steps, deferred convergence)
  │   │
  │   └─ Continue to next task (no stop)
  │
  ├─ Batch state convergence (after all tasks or at invocation boundary)
  │   ├─ Update source plan tasks (1 step)
  │   ├─ Update state.md session continuity (1 step)
  │   ├─ Re-read + verify (4 steps)
  │   └─ Subtotal: 6 steps (once, not per-task)
  │
  └─ Report completion or STOP at legitimate safety boundary
```

Estimated savings for a 5-task plan in a single invocation:

| Metric | Before P0+P0b | After P0+P0b (current) | After P1+P2 (future) |
| --- | --- | --- | --- |
| Context loading steps | 60-90 (3 reads × 5 tasks) | 60-90 (unchanged) | 4-6 (1 read) |
| State convergence steps | 35-40 (7-8 × 5 tasks) | 35-40 (unchanged) | 6 (1 batch) |
| Total steps | 175-255 | 175-255 | 80-120 |
| Manual invocations | 5+ | 1 | 1 |
| Tasks per invocation | 1 | 4-5 | 4-5 |
