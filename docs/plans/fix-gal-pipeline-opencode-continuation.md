# Plan: Fix GAL Pipeline Continuation In OpenCode Single-Model Runtimes

## Goal

`/gal pipeline` should continue reliably through implement -> test -> review -> verifier when run from OpenCode with Kimi K2.6 or another single-runtime environment. The pipeline contract must stop pretending that every runtime can satisfy multi-model dispatch automatically, and it must treat phase write-back into the active execution prompt as the authoritative continuation signal.

## Requirements

- [ ] `/gal pipeline` must distinguish between direct golem consultation and bound pipeline phase execution.
- [ ] Pipeline phase dispatch must not reuse a `consult` contract when the phase requires plan write-back, commit capture, and retry-loop continuation.
- [ ] The dispatcher contract must remain cross-runtime aligned across PowerShell and Bash.
- [ ] The pipeline must keep multi-model verification as the preferred path when `model-roles.local.md` can actually assign distinct models or providers.
- [ ] The pipeline must detect when the current runtime cannot satisfy the multi-model assumption and must degrade explicitly instead of silently stopping after implement.
- [ ] OpenCode plus Kimi K2.6 must have a documented and stateful same-runtime fallback path.
- [ ] Same-runtime fallback must state the quality trade-off clearly: it is a constrained fallback, not equivalent to independent cross-model verification.
- [ ] Implement, test, and review phases must each have an explicit post-phase convergence gate tied to execution-prompt write-back.
- [ ] The continuation signal after implement must require `Task Base Commit`, `Task Final Commit`, and a clean worktree before test can start.
- [ ] The continuation signal after test must require a task-scoped `## Test Results` subsection before PASS or FAIL is evaluated.
- [ ] The continuation signal after review must require a task-scoped `## Review Results` subsection and verdict before APPROVE or BLOCK is evaluated.
- [ ] If a dispatched phase returns without the required write-back, the pipeline must stop with a precise diagnostic and write a `Retry Handoff` block instead of appearing to succeed.
- [ ] The dispatcher contract for pipeline golems must align with the documented `MODE` meanings in GAL command docs.
- [ ] Documentation for OpenCode command generation and runtime topology must match what the scripts actually generate.
- [ ] Non-blocking deferred or follow-up items in execution prompts must not remain in `## Tasks` and must not cause ambiguous continuation or false pipeline incompleteness.
- [ ] OpenCode or other low step-budget runtimes must avoid starting multiple tasks in one invocation when that risks a runtime cutoff mid-phase.
- [ ] If a runtime cutoff happens anyway, the next `/gal pipeline` invocation must resume the interrupted phase from durable plan/worktree state instead of asking the user to manually reconstruct progress from chat.

## Approach

### Diagnosis Summary

The current failure mode is a contract mismatch rather than one isolated bug.

- `commands/gal-pipeline/SKILL.template.md` describes a multi-phase autopilot, but the dispatcher scripts only emit `--- GAL DISPATCH ---` blocks and do not execute downstream phases themselves.
- `commands/commands.md` classifies pipeline golems such as `golem-implementer`, `golem-tester`, `golem-reviewer`, and `golem-verifier` with default mode `consult`, while `commands/gal/SKILL.template.md` defines `consult` as advisory rather than bound execution.
- OpenCode agent `mode` is already defined by the runtime as `primary`, `subagent`, or `all`, so GAL should not overload `MODE` to express pipeline binding semantics.
- OpenCode supports per-agent `model` assignment, but subagents inherit the invoking primary agent's model unless a distinct model is explicitly configured. In this repo's current `opencode.json`, no pipeline-specific agent model split is defined, so OpenCode + Kimi K2.6 should currently be treated as a same-runtime path unless proven otherwise by explicit config.
- `opencode.json` does not document or constrain GAL pipeline behavior specifically.
- The current repo `opencode.json` sets the OpenCode `build` agent to `steps: 12`; the Kimi K2.6 field run completed T-001 and T-002, then hit the runtime step ceiling during T-003 implementation before validation and commit.
- Execution prompts can remain in an ambiguous state when blocking work is done but deferred or non-blocking follow-up items remain mixed into `## Tasks`.

### Resolved Decisions

- **Decision D-001** - Encode pipeline-bound execution as an additional dispatch field rather than a new `MODE`. Keep direct consult semantics intact, and add an explicit pipeline-only signal such as `DISPATCH_KIND=pipeline-phase` or equivalent so pipeline-aware runtimes can distinguish bound execution from advisory golem consultation without conflicting with OpenCode's documented agent `mode` meanings.
- **Decision D-002** - Treat explicit same-runtime fallback as the default OpenCode behavior unless per-agent model routing is configured and verified. OpenCode can route different agents to different underlying models, but that only exists when the config assigns agent-specific `model` values. Without that explicit configuration, pipeline subagents should be assumed to share the current primary runtime model.
- **Decision D-003** - Treat `## Tasks` as the blocking completion gate only. Deferred work, optional follow-up, or non-blocking polish must move to a separate non-blocking section instead of remaining inside `## Tasks`, so pipeline readiness can be determined from one authoritative blocking task list.
- **Decision D-004** - Treat runtime step exhaustion as an interruption, not a pipeline failure. In OpenCode, `/gal pipeline` should default to single-task tranche mode and stop cleanly after one completed task unless the user explicitly requests a larger multi-task turn with enough visible step budget. If a cutoff still happens mid-phase, the next invocation resumes the interrupted phase from plan/worktree state before starting any new task.

### Step 1: Split direct consult mode from pipeline-bound phase mode

- **Files**: `commands/commands.md`, `commands/gal/SKILL.template.md`, `commands/gal-pipeline/SKILL.template.md`
- **What**: Make the command contract explicit: direct `/gal golem-*` invocations may remain consult-oriented, but pipeline-owned phase calls must run with a separate bound-execution dispatch field that permits write-back and continuation.
- **Verify**: The docs no longer imply that a `consult` phase can satisfy pipeline execution requirements.

### Step 2: Add an explicit pipeline-phase dispatch signal

- **Files**: `scripts/gal.ps1`, `scripts/gal.sh`
- **What**: Add an emitted dispatch field that identifies pipeline-owned phase execution, so the runtime can distinguish `consult` golem calls from bound pipeline phases without redefining OpenCode's documented `mode` values.
- **Verify**: PowerShell and Bash emit matching dispatch fields and mode semantics for the same inputs.

### Step 3: Add runtime and model preflight for `/gal pipeline`

- **Files**: `commands/gal-pipeline/SKILL.template.md`
- **What**: Before the task loop, inspect `model-roles.local.md` and the active runtime configuration to determine whether CODER, TESTER, REVIEWER, and VERIFIER can actually be assigned independently in the current runtime context.
- **Verify**: The pipeline emits an explicit warning or fallback path instead of silently assuming multi-vendor verification is available.

### Step 4: Define the OpenCode single-runtime fallback

- **Files**: `commands/gal-pipeline/SKILL.template.md`
- **What**: Add a sequential fallback for OpenCode + Kimi K2.6 and similar same-runtime environments: implement, confirm status write-back, run tester, confirm test write-back, run reviewer, confirm review write-back, then continue. This fallback remains the default OpenCode path unless agent-specific model routing is explicitly configured and verified.
- **Verify**: The fallback is explicit, documented, and clearly marked as lower-rigor than independent cross-model verification.

### Step 5: Add post-phase convergence gates

- **Files**: `commands/gal-pipeline/SKILL.template.md`, `agent/golem-implementer.agent.md`, `agent/golem-tester.agent.md`, `agent/golem-reviewer.agent.md`
- **What**: Tighten the phase contracts so the pipeline treats prompt write-back as the phase completion signal.
- **Verify**: Missing commit markers, missing task-scoped test results, or missing task-scoped review verdicts become explicit stop conditions.

### Step 6: Review OpenCode command and subagent surfaces

- **Files**: `opencode.json`, `scripts/Update-Commands.ps1`, `scripts/update-commands.sh`, `scripts/Update-Skills.ps1`, `scripts/update-skills.sh`, `scripts/scripts.md`
- **What**: Confirm how OpenCode commands and subagents are generated, decide whether pipeline-specific guidance belongs in repo-local OpenCode config, and document the real topology.
- **Verify**: The docs match the generated command and subagent surfaces, including OpenCode targets.

### Step 7: Separate blocking tasks from non-blocking follow-up

- **Files**: `commands/gal-pipeline/SKILL.template.md`, `commands/gal-whats-next/SKILL.template.md`, `commands/gal-status/SKILL.template.md`, related execution prompt template guidance if needed
- **What**: Clarify that `## Tasks` contains blocking work only, and move deferred or non-blocking follow-up into a separate section so the pipeline and status surfaces do not confuse intentionally deferred work with blocked required work.
- **Verify**: A plan with blocking work complete and deferred follow-up outside `## Tasks` recommends verification or handoff instead of re-entering the main task loop.

### Step 8: Regenerate generated outputs

- **Files**: generated `commands/*/SKILL.md`, OpenCode command outputs, and runtime adapters as validation outputs only
- **What**: Regenerate from source templates and scripts after the authored changes land.
- **Verify**: Generated artifacts reflect the corrected contract with no hand-edited divergence.

### Step 9: Add runtime step-budget resilience

- **Files**: `commands/gal-pipeline/SKILL.template.md`, `commands/gal-whats-next/SKILL.template.md`, `agent/golem-implementer.agent.md`, `opencode.json`
- **What**: Add OpenCode step-budget preflight, default single-task tranche mode, interrupted-phase handoff, and implementer checkpoint guidance. Raise the repo default OpenCode `build.steps` enough for a single bounded task.
- **Verify**: A Kimi K2.6 run with a low step budget stops cleanly after a completed task, or resumes an interrupted `IMPLEMENT` phase from the recorded current task and dirty worktree instead of asking the user to restart manually.

## Files To Create Or Modify

### Required authored changes

- `commands/commands.md` - align golem classification and default mode guidance with pipeline execution reality.
- `commands/gal/SKILL.template.md` - make dispatcher `MODE` semantics explicit for direct consults versus bound pipeline phases.
- `commands/gal-pipeline/SKILL.template.md` - add runtime preflight, single-runtime fallback, stronger continuation gates, and blocking-task-only semantics for `## Tasks`.
- `scripts/gal.ps1` - emit explicit pipeline-phase dispatch semantics for PowerShell.
- `scripts/gal.sh` - emit explicit pipeline-phase dispatch semantics for Bash.
- `agent/golem-implementer.agent.md` - tighten completion/write-back expectations for pipeline use.
- `agent/golem-tester.agent.md` - require task-scoped result write-back before the pipeline treats testing as complete.
- `agent/golem-reviewer.agent.md` - require task-scoped review verdict write-back before the pipeline treats review as complete.
- `scripts/scripts.md` - document OpenCode command generation targets and runtime topology accurately.

### Conditional authored changes

- `opencode.json` - only if repo-local OpenCode config needs pipeline-specific step budget or warning guidance.
- `commands/gal-status/SKILL.template.md` - only if blocking-vs-non-blocking task separation affects current readiness output.
- `commands/gal-whats-next/SKILL.template.md` - if blocking-vs-non-blocking task separation affects next-step recommendation or an interrupted phase must take priority over generic implementation advice.
- execution prompt template guidance - only if the source of ambiguity is shared task-section structure rather than command logic.

### Generated validation outputs

- `commands/*/SKILL.md`
- OpenCode generated command files
- runtime adapters refreshed through the existing setup or sync path

## Test Cases

- [ ] Dispatcher mode check: direct golem dispatch still uses consult semantics where intended, but pipeline-owned phase dispatch emits an explicit bound or pipeline-phase signal.
- [ ] Shell parity check: `scripts/gal.ps1` and `scripts/gal.sh` emit equivalent dispatch semantics for the same golem and phase input.
- [ ] Multi-model preflight check: when `model-roles.local.md` supports distinct CODER, TESTER, REVIEWER, and VERIFIER assignments, the pipeline stays on the preferred multi-model path.
- [ ] Single-runtime fallback check: OpenCode + Kimi K2.6 enters an explicit same-runtime fallback instead of stopping after implement.
- [ ] Implement convergence check: if `Task Final Commit` is missing or the worktree is dirty, the pipeline stops before test with a precise diagnostic.
- [ ] Test convergence check: if no task-scoped `## Test Results` subsection is written for the current task, the pipeline stops instead of assuming PASS or FAIL.
- [ ] Review convergence check: if no task-scoped `## Review Results` subsection and verdict are written for the current task, the pipeline stops instead of assuming approval or block.
- [ ] Retry handoff check: when any phase returns without required write-back, the pipeline records a `Retry Handoff` block with actionable next steps.
- [ ] Blocking-task readiness check: an execution prompt with blocking work complete and deferred follow-up moved outside `## Tasks` does not loop indefinitely or misreport completion status.
- [ ] Documentation topology check: `scripts/scripts.md` correctly names OpenCode command generation and no longer omits generated command targets that the scripts actually write.
- [ ] Generated artifact check: regenerated command skills and runtime outputs reflect source-of-truth changes without manual edits.
- [ ] OpenCode step-budget check: with `agent.build.steps` set low or unknown, `/gal pipeline` completes no more than one task before stopping cleanly with a continuation instruction.
- [ ] Interrupted implementation resume check: if `Workflow: IMPLEMENT` is active, `Task Final Commit` is missing, and the worktree is dirty, the next `/gal pipeline` invocation resumes the existing task rather than starting another task.

## Success Criteria

- [ ] GAL has one explicit contract for bound pipeline phase execution that does not rely on `consult` semantics.
- [ ] `/gal pipeline` no longer silently assumes that every runtime can satisfy a multi-model workflow.
- [ ] OpenCode + Kimi K2.6 has a documented continuation path that preserves stateful progression through test and review.
- [ ] Phase completion is decided by durable execution-prompt write-back rather than vague runtime success.
- [ ] PowerShell and Bash dispatchers remain behaviorally aligned.
- [ ] Deferred or non-blocking follow-up items no longer create false pipeline incompleteness or ambiguous handoff state because `## Tasks` is reserved for blocking work.
- [ ] OpenCode runtime documentation matches actual generated commands and subagents.
- [ ] OpenCode Kimi K2.6 no longer reaches a runtime step cutoff after completing earlier tasks and starting a later task in the same invocation.

## Risks

- Changing dispatcher semantics can accidentally widen direct golem permissions if consult and bound modes are not separated carefully.
- If same-runtime fallback wording is too soft, users may mistake it for full independent verification rather than a degraded mode.
- If preflight is too strict, the pipeline could become unusable in runtimes that can in fact route distinct models but do not expose that mapping clearly.
- Blocking-vs-non-blocking task separation can drift if the prompt template and status commands are not updated consistently.
- This work touches protected paths in `commands/` and execution scripts, so implementation should remain architect-reviewed before code changes proceed.

## References

- Diagnostic notes captured for the OpenCode + Kimi K2.6 continuation issue
- `commands/gal-pipeline/SKILL.template.md`
- `commands/commands.md`
- `commands/gal/SKILL.template.md`
- `scripts/gal.ps1`
- `scripts/gal.sh`
- `model-roles.local.md`
- `https://opencode.ai/docs/agents/`
- `.dev/plans/feat-gal-file-memory-strategy.prompt.md`

## Open Questions

- [x] OQ-001 - Resolved: bound pipeline phase execution should be encoded as an additional dispatch field consumed by pipeline-aware runtimes, not as a new `MODE`. This preserves OpenCode's documented `mode` meanings (`primary`, `subagent`, `all`) and prevents direct consult semantics from widening unintentionally.
- [x] OQ-002 - Resolved: OpenCode does support routing agents to distinct underlying models through agent-specific `model` configuration, but same-runtime fallback should be treated as the default behavior until that routing is explicitly configured and verified in the active repo/runtime.
- [x] OQ-003 - Resolved: `## Tasks` should be reserved for blocking tasks only, while deferred or non-blocking follow-up work should move to a separate non-blocking section. Pipeline, status, and next-step readiness should be derived from the blocking task list only.

## Approval

- Human approval: [pending]
- Architect review: [required]
- Additional domain review: [not triggered]
