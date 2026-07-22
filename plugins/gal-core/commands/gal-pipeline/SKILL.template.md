---
name: gal-pipeline
description: "GAL pipeline ($gal-pipeline / /gal pipeline / 跑 pipeline / 實作 / implement / run the pipeline). Entry Latch: run gal pipeline-preflight receipt BEFORE any implementation edit. Iterates T-NN tasks: implement → correctness gate → test → audit → commit → converge. Stops only on human-required blockers, retry ceiling (3), or working-hours Hard Stop."
---

# /gal-pipeline

Run the full implementation pipeline task by task: for each blocking `T-NN` task in the active plan, run implement → orchestrator correctness gate → test → audit in sequence, then advance to the next task. Prefer different AI vendors per `~/.gal/config/config.json#executorRouting` when the active runtime can actually enforce that split. After all blocking tasks complete, run a final goal-backward verify pass.

## Role

Pipeline orchestrator. Your job is to iterate through plan tasks automatically, advancing only when each task's commit + correctness gate + test + audit gate is fully clean, and stopping only when a genuine human-required condition is encountered.

## Entry Latch (binding — no edit before this passes)

**Before any implementation edit, you MUST:**

1. Run `gal pipeline-preflight <execution-prompt-path> --receipt .dev/pipeline/receipts/<slug>-preflight.receipt.md`
2. Read the receipt. Only `overall: pass` allows proceeding. `fail` or `not-run` → STOP immediately — do NOT make any file edits.
3. If the user sends multiple plan references in one request, expand them to an **ordered run list** and run each plan fully (preflight → task loop → goal-backward verify) before starting the next. Never interleave tasks across plans.

This latch is not optional. A pipeline that skips it is a `named workflow obedience failure`.

## Codex / PowerShell Invocation Safety

These rules keep the Codex orchestrator on the Rust dispatch path instead of silently degrading to in-chat role-play.

- **Quote the `#file:` token in PowerShell.** In PowerShell a bare `#` starts a comment, so an unquoted `#file:<prompt>` is discarded before `gal dispatch-script` ever sees it → the dispatcher receives no prompt path and emits `COMMAND: error`. Always paste the token single-quoted: `'#file:.dev/plans/<slug>.prompt.md'`. Bash/Zsh tolerate the unquoted form, but the quoted form is safe in every shell — prefer it everywhere. Every `gal dispatch-script … '#file:<execution-prompt-path>'` example below is written this way.
- **Explicit prompt path when more than one plan is active.** When `.dev/state.md` lists multiple active plans, do **not** let Step-1 resolution fall through to the first active-plan row. Require the user to name the plan explicitly with `'#file:<prompt>'` (or `@<prompt>`), and dispatch only that resolved execution prompt. Silent first-row selection is a wrong-plan hazard — stop and ask for the explicit path instead.
- **Sandbox receipt/log write denial is a HARD environment stop, not permission to role-play.** If `gal pipeline-preflight`, `gal boundary-check`, `gal pipeline-converge-check`, or a `gal pipeline … --phase …` dispatch fails because `.dev/pipeline/receipts/` or `.dev/executor-logs/` could not be written (Codex sandbox / access-denied), **STOP**. Rerun the exact same Rust command after the user grants the required write approval. Never fall back to implementing, testing, auditing, or "verifying" the phase in chat because a receipt/log write was denied — a denied write is missing evidence, and missing evidence is never a pass.

## When to Use

- After the engineering review lane has produced a `## Tasks` section and a `## Test Plan`
- When you want full task-by-task automation without manual intervention
- When the user says "start implementation", "run the pipeline", "implement and test", "跑 pipeline", "實作", or similar

## Syntax

```text
/gal pipeline [#file:<plan.md> | @<plan.md>] [from T-NN] [stop-at T-NN]
```

- **`#file:<plan.md>`**: use the referenced plan file as the pipeline input for this invocation. When present, it overrides `.dev/state.md` active-plan lookup for Step 1 only. If the referenced file is a source plan and the matching `.dev/plans/<slug>.prompt.md` exists, resolve to the execution prompt before continuing.
- **`@<plan.md>`**: treat OpenCode-style attached path arguments the same as `#file:<plan.md>` after stripping the leading `@`.
- **No arguments**: start from the first unchecked task, run until all tasks complete
- **`from T-NN`**: start from the specified task (skip earlier unchecked tasks)
- **`stop-at T-NN`**: after completing `T-NN`, stop before starting the next task and prompt the user
- **Resume**: if `Current Task` is set in `## Status`, resume from that task (overridden by explicit `from`)

### Remote Dispatch (SSH Lane)

Cross-machine execution is not a separate pipeline syntax or activation phrase — it is a property of the resolved route. When a `config.json#executorRouting` role entry for the current phase carries `sshTarget` + `remoteWorkdir`, the dispatch for that phase runs over SSH transparently; `/gal pipeline` syntax, task loop, and gates are unchanged either way. See `docs/manual.md` → Remote Execution (SSH Dispatch Lane) for the config keys and safety boundaries.

If the dispatcher emits `TASK_REF`, `FROM`, and `STOP_AT`, treat them as authoritative for this invocation.

## Model Assignment

Prefer a different AI vendor for each phase, using `~/.gal/config/config.json#executorRouting` as the desired role mapping and the active runtime config as the enforcement surface:

| Phase | Golem | Role | Why different |
| --- | --- | --- | --- |
| Implement | `golem-implementer` | CODER | Writes the code |
| Test | `golem-tester` | TESTER | Must not read implementation — writes tests from spec only |
| Audit | `golem-auditor` | AUDITOR | Must differ from CODER — independent deep-performance and security audit (single task) |
| Verify | ORCHESTRATOR (inline spec, always in-process) | — | Goal-backward plan verification, owned by the orchestrator — always in-process, never dispatched; see Step 3 |

The orchestrator's correctness gate is not dispatched: it is the pipeline's own full-context check for checklist 1–13 plus obvious performance before test. The end-of-run goal-backward verification (Step 3) is likewise orchestrator-owned and always runs in-process (never dispatched).

**Checking-role triangle:** ORCHESTRATOR (dispatch + correctness gate + goal-backward verify + lifecycle) · AUDITOR (single-task deep perf + security) · STEWARD (knowledge extraction → `docs/` + doc structure).

### Runtime Preflight

Before starting the task loop, resolve model separation in this order:

1. Treat runtime-enforced per-agent model routing as authoritative.
2. Treat `~/.gal/config/config.json#executorRouting` as the desired separation policy, not proof that the current runtime can enforce it.
3. If the active runtime cannot prove separate CODER, TESTER, and AUDITOR routes, degrade explicitly to same-runtime fallback. (Verify is always in-process and is not a route.)

OpenCode-specific rule:

- Different models only count when the active OpenCode configuration assigns agent-specific `model` values.
- Inherited subagent execution under the same primary agent model does not satisfy independent verification.
- If no verified per-agent model split exists, OpenCode must follow the documented same-runtime fallback instead of pretending multi-model verification is available.

### Same-Runtime Fallback Contract

If runtime preflight cannot prove separate CODER, TESTER, and AUDITOR routes:

- Mark the run internally and in any user-facing summary as `Verification Independence: DEGRADED_SAME_RUNTIME`.
- Keep implement, test, audit, and verify as separate bounded phase invocations with their normal durable write-back requirements. Same-runtime fallback does **not** collapse these phases into a single blended pass by default.
- Do not silently bundle implement + test + audit just to save tokens. Bundled same-runtime execution is allowed only when the user explicitly asks for it.
- If the user explicitly asks for bundled same-runtime execution, mark the run as `Verification Independence: DEGRADED_BUNDLED`, keep separate task-scoped `## Test Results` and `## Review Results` write-back, and state clearly that tester/auditor independence was reduced for this invocation.
- Same-runtime fallback never waives retry ceilings, protected-path escalation, audit STOP rules, interrupted-phase handoff, or final verify requirements.

### Loop-Log: In-Conversation Error Capture (hard step)

The headless dispatch path writes its own loop-log event (one per `terminal_state`) automatically. When a phase runs **in-conversation** (`DEGRADED_SAME_RUNTIME` — no dispatch attempt log exists for this phase at all), there is no dispatch process to do that — so the orchestrator MUST record its own failures itself. This is a **hard step**, not optional:

- When an in-conversation phase hits a durable failure or degrade — a **naming-gate block**, a **test failure**, a **compile error**, an **out-of-allowlist boundary violation**, a **retry**, or a **degrade** to same-runtime — append a structured loop-log event:

  ```powershell
  gal pipeline-log append --task T-NN --phase <phase> --role <ROLE> --kind <kind> --level <level> --msg "<short failure-only summary>"
  ```

  where `<kind>` is one of `naming-gate-block` / `test-fail` / `compile-error` / `executor-no-receipt` / `executor-timeout` / `boundary-violation` / `degrade` / `retry`, and `<level>` is `warning` (recoverable/degraded) or `error` (failure needing attention).

- **Failure-only**: do not log pass detail. One line per real failure/degrade, not per successful step.
- The subcommand writes to the machine-local gitignored loop-log (`.dev/pipeline/loop-log/<date>.ndjson`) and redacts obvious secrets from `--msg`. It is a pipeline-internal subcommand (like `gal naming-gate` / `gal finalize-check`), not part of the public `/gal` command surface.
- This append is a **convenience for forensics, not a workflow gate** — it never blocks a task. Its purpose is that the next debugging session can grep the loop-log instead of re-deriving what failed from chat memory (which is exactly the gap that forced manual bug-doc back-fills before).

### Headless Executor Dispatch

When `~/.gal/config/config.json#executorRouting` is present and maps the current phase's role to a CLI executor, the **self-contained `gal` binary** takes over dispatch: `gal dispatch-script` emits the headless `OFFLOAD` block, and `gal pipeline <prompt-or-plan-or-spec> --phase <p> --task <t> --receipt <target>` runs the executor **inside the `gal` binary's own process** (`dispatch::run`; distinct from a phase the orchestrator role-plays *in-conversation*) — there is no separate `gal-dispatch` sibling executable. When routing is absent, dispatch emits a regular `--- GAL DISPATCH ---` text block (backward-compatible, no regression).

**OFFLOAD is keyed on `(task, phase, routing)` — not on `.dev/state.md`.** A routed executor for the phase role is sufficient to emit the `OFFLOAD` block; the dispatcher does **not** parse `state.md` to decide whether to offload (the prior dependency on a `state.md` active-plan *table* silently disabled offload whenever `state.md` used a bullet list). The real safety gate (routing + executor-in-PATH + readiness probe) lives downstream in `gal pipeline`/`dispatch::run`.

**`gal pipeline` input resolution.** The dispatch target is a path, resolved by kind: a `*.prompt.md` execution prompt (preferred — `gal pipeline` reads it and materializes the per-(task,phase) spec internally, scoped to the single `T-NN`); a `.dev/plans/<slug>.md` source plan (auto-switched to the paired `.dev/plans/<slug>.prompt.md`, or a hard error pointing at `/refining-plan` + recorded human approval + `/plan-to-prompt` if no prompt exists — never dispatch a source plan); or an already-materialized task spec (fed as-is). The `OFFLOAD` block's `ACTION` carries the explicit prompt/plan path when one was provided.

**Orchestrator guard (state.md cross-check).** Because the Rust gate no longer reads `state.md`, the orchestrator owns the wrong-plan safety check: **before dispatching, confirm the prompt/plan being offloaded matches the active plan in `.dev/state.md` (active-plan row + session-continuity row).** This catches dispatching the wrong plan; it is the orchestrator's responsibility, not the dispatcher's.

Phase-to-role map (implement→CODER, test→TESTER, audit→AUDITOR). Verify is not a dispatch phase — it is ORCHESTRATOR-owned and always in-process.  
Supported executors: **claude / codex / opencode / copilot / agy** — all five are first-class headless executors. The prior `non-dispatchable` status for codex and copilot has been overturned by spike evidence.

#### How the Bin Dispatches

1. Read `~/.gal/config/config.json#executorRouting` → resolve `role → { executor, model }`.
2. Safety gate: only offload when executor CLI is available in PATH. Unavailable executor → text dispatch, exit 2.
3. Select the tool-specific adapter (invocation flags per spike-locked results):

| Tool     | Built-in headless flags (adapter-owned)                              | `effort` → native flag                  | Spec delivery |
|----------|----------------------------------------------------------------------|-----------------------------------------|---------------|
| claude   | `-p --output-format json --dangerously-skip-permissions --model <m>` | `--effort <value>`                      | stdin         |
| codex    | `exec --json -s workspace-write -m <m>`                              | `-c model_reasoning_effort="<value>"`   | stdin         |
| opencode | `run --format json --auto --agent build -m <m>`                     | `--variant <value>`                     | stdin         |
| copilot  | `-C <workdir> --allow-all --output-format json --model <m> --no-custom-instructions --disable-builtin-mcps [--disable-mcp-server <name>…] -p <spec>` | `--reasoning-effort <value>` | `-p` flag |
| agy      | `--dangerously-skip-permissions` (stdin)                            | unsupported → `unsupported-effort`      | stdin         |

**Permission-bypass flags vs `effort` — two different concepts.** The permission-bypass
flags (`--auto` for opencode, `--allow-all` for copilot, `--dangerously-skip-permissions`
for claude/agy) grant the secondary CLI full filesystem/tool access so it can write the
receipt headlessly; they are **not** reasoning controls. `effort` is a separate,
per-role reasoning-intensity hint (the `effort` route-entry key), mapped to each
executor's native reasoning flag above. Set it as `"effort": "high"` on the role — never
as a raw flag bag.

- **`effort` is fail-closed.** Only claude/codex/copilot/opencode honor it; agy (and any
  future executor that has not opted in) rejects it before spawn with `unsupported-effort`.
  A value outside `[A-Za-z0-9._-]+` is rejected with `invalid-effort`. Neither dispatches.
- **OpenCode** uses `--auto` (current permission-bypass flag; the old
  `--dangerously-skip-permissions` was removed from the CLI) and `--agent build` (the
  write-capable agent; the default/`plan` agent is read-only and silently produces no
  file changes).
- **Copilot** always adds `--no-custom-instructions` + `--disable-builtin-mcps` so its
  headless prompt-mode does not overflow the static context before writing the receipt;
  the disabled MCP-server names come from Copilot's own `~/.copilot/mcp-config.json`.
  Default tools are kept (the receipt needs the file `write` tool; `--available-tools
  write` was empirically over-restrictive). Copilot stays `model: auto` (Copilot Free is
  auto-only) and remains local-only.

4. Spawn the executor, feed spec, enforce timeout, kill process tree on timeout.
5. After exit 0: read the receipt file (`--receipt <path>`) to confirm write-back. Exit 0 without the file written → terminal state `no-receipt`. **Do not accept exit code 0 alone.**
6. Extract provider-native session/job id from stdout (tool-specific JSON field; agy scans `~/.agy/brain/`). Record in executor-log header and `Dispatch:` marker.
7. Write durable executor log and output the `Dispatch:` marker line to stdout.

#### Dispatch Observability Marker

The bin outputs to stdout after every run:

```
--- GAL DISPATCH ---
Dispatch: phase=<p> task=<T-NN> role=<ROLE> executor=<cli> model=<m> state=<terminal-state> session_id=<id|none> log=<path>
```

Degrade variants (exit 2, bin is the output owner):
```
Dispatch: phase=<p> task=<T-NN> role=<ROLE> executor=none reason=no-routing
Dispatch: phase=<p> task=<T-NN> role=<ROLE> executor=<cli> model=<m> reason=executor-unavailable
Dispatch: phase=<p> task=<T-NN> role=<ROLE> executor=<cli> model=<m> reason=executor-unauthenticated-confirmed
Dispatch: phase=<p> task=<T-NN> executor=<cli> model=<m> reason=log-error
```

`executor-unauthenticated-confirmed` fires when the executor is in PATH but its readiness probe reports unauthenticated for confirmed headless use (currently only copilot has a dedicated probe; other executors report unknown and are allowed through). `log-error` fires when the durable executor log could not be written.

Shim fallback (bin absent, shim is the output owner):
```
--- GAL DISPATCH ---
Dispatch: reason=bin-absent executor=text-dispatch
```

After every pipeline-phase dispatch, the orchestrator records the `Dispatch:` line in the execution prompt for that stage. Record before moving to the next phase or task.

#### Provider-Native Traceability

Every dispatch captures and records the provider-native session/job id:

| Tool     | Session id field              | Resume command                            |
|----------|-------------------------------|-------------------------------------------|
| claude   | JSON `session_id`             | `claude --resume <session_id>`            |
| codex    | NDJSON `thread_id`            | `codex exec resume <thread_id>`           |
| opencode | NDJSON `sessionID`            | `opencode run --session <sessionID>`      |
| copilot  | JSON `result.sessionId`       | `copilot --resume=<sessionId>`            |
| agy      | newest `~/.agy/brain/<uuid>/` | `agy --conversation <uuid>`               |

The session id appears in the executor-log header and in the `Dispatch:` marker line. If the id cannot be extracted, `session_id=none` is recorded (not a failure; traceability is a goal, not a gate for completion).

#### Three-Condition PASS Threshold (Honest Test Gate)

A dispatch is only PASS when **all three** hold:

1. Receipt file was written by the secondary tool (non-empty file exists at `--receipt <path>`)
2. The plan-scoped attempt log (`.dev/executor-logs/<plan-slug>/`, or the unscoped default location for raw/direct dispatch) records terminal state `completed`
3. Provider-native session record appears in the tool's own history / is resumable

Anything less is `no-receipt`, `disconnected-partial`, or `env-unverifiable`. "CLI accepted `--model`" ≠ "headless write-back verified." These are distinct claims.

#### Degrade Conditions

| Condition | Behavior |
| --- | --- |
| No routing file | Bin outputs text dispatch, exit 2 |
| Role has no routing entry | Bin outputs text dispatch, exit 2 |
| Executor not in PATH | Bin outputs text dispatch with `reason=executor-unavailable`, exit 2 |
| Executor in PATH but readiness probe = unauthenticated (copilot) | Bin outputs text dispatch with `reason=executor-unauthenticated-confirmed`, exit 2 |
| Durable executor log write fails | Bin outputs `reason=log-error`, exit 2 |
| Executor exit non-zero | Terminal state `disconnected-partial`, exit 1 |
| Write-back missing or empty after exit 0 | Terminal state `no-receipt`, exit 1 |
| Timeout | Terminal state `timeout`, exit 2; process tree killed |
| Bin absent | Shim outputs `Dispatch: reason=bin-absent`, exit 2 |

All degrade paths are backward-compatible — orchestrator falls back to role-playing the golem in conversation.

#### Commit Boundary

**The secondary CLI must not run `git commit` or `git push`.** The task spec explicitly forbids this. The commit boundary is held exclusively by the orchestrator. The bin enforces the commit boundary by not passing commit-related flags; the spec template must also omit any commit instruction to the executor.

**Autonomy changes none of this — zero new commit mechanism.** Default autonomous continuation does not relax the commit discipline: the orchestrator-exclusive commit boundary (dispatched vendors forbidden `git commit`/`git push`), the per-task atomic commit, and converge-before-commit (the 2g Task State Convergence Gate must pass before any state-recording commit) all hold unchanged. There is no checkpoint commit, summary commit, or "save progress" commit introduced at stop points or tranche boundaries — a resume marker is written to the prompt's `## Status`, not committed as a separate progress commit.

#### Execution Outcome Taxonomy

Every required phase (`implement`/`test`/`audit` — `verify` is orchestrator-owned and never dispatched) resolves to exactly one of three outcome literals, computed by `gal pipeline-converge-check`'s bound v2 receipt from the plan-scoped attempt-log evidence below:

| Outcome | Meaning |
| --- | --- |
| `in-conversation` | No dispatch attempt log exists for this phase at all — the phase happened without any headless dispatch (same-runtime fallback, or a phase legitimately handled without dispatch). |
| `dispatch-offload` | The phase's LATEST dispatch attempt terminated `completed`. |
| `recovered-in-conversation` | The phase's LATEST dispatch attempt terminated non-`completed` (a broken or timed-out dispatch), but the task's bound convergence (three-surface agreement + task-commit + cursor-cleared) otherwise still holds — a legitimate recovery, not silently erased failure history. |

An unterminated `started` marker as the latest attempt is not one of these three outcomes — it fails the bound v2 receipt unconditionally (never convergence-gated), because a marker that was never terminally rewritten proves nothing about how the attempt actually ended.

Older non-completed attempts are never deleted or overwritten — every attempt gets its own collision-safe path (see below), so the full attempt history stays on disk as forensic evidence even after a later attempt succeeds or the task recovers. Only the LATEST attempt's terminal state drives a phase's outcome classification.

#### Durable Run Record (Forensics)

Before any availability check or process spawn, the bin allocates a unique attempt path and durably writes a `terminal_state: started` marker there (marker-before-spawn). Marker-write failure prevents spawn. On terminal completion the SAME path is rewritten with the full log content; if that terminal rewrite fails, the file is left holding the `started` marker — intentional evidence of an unterminated attempt, not a bug.

The log directory is plan-scoped whenever the pipeline input is a Prompt or SourcePlan path: `.dev/executor-logs/<plan-slug>/`, derived automatically from the dispatch target's own filename (no CLI flag). Raw/direct dispatch keeps the unscoped default location, `.dev/executor-logs/`.

The bin writes `<log_dir>/<epoch_secs>-<subsec_nanos>-<attempt_seq>-<task_id>-<phase>-<executor>.log` containing:
- Header fields: `timestamp_start`, `timestamp_end`, `duration_ms`, `executor`, `phase`, `task_id`, `git_branch`, `git_head`, `exit_code`, `actual_model`, `terminal_state`, `session_id`
- Full `---STDOUT---` and `---STDERR---` sections

The `<subsec_nanos>-<attempt_seq>` pair makes every attempt's path collision-safe — two attempts for the same task/phase/executor triggered within the same wall-clock second still land at distinct paths, so rapid retries never overwrite an earlier attempt.

Terminal-state vocabulary:

| State | Meaning |
| --- | --- |
| `started` | Pre-spawn marker written before availability check/spawn; residue when the terminal rewrite never happened |
| `completed` | Exit 0 and write-back verified |
| `no-receipt` | Exit 0 but receipt file absent or empty |
| `timeout` | Killed by process-tree timeout |
| `timeout-no-output` | Killed by process-tree timeout after producing zero output — suggests a hang on an interactive prompt rather than genuine work |
| `timeout-midrun` | Killed by process-tree timeout after producing some output — was working, just too slow |
| `disconnected-partial` | Non-zero exit; possible partial write |
| `unavailable` | Executor CLI not found in PATH |

These logs are retained for forensics. At pipeline time `gal pipeline-converge-check` scans this same plan-scoped directory, classifies each required phase's LATEST attempt into one of the three outcome literals above, requires the task's bound convergence to hold, and writes a bound v2 receipt recording the result. `/gal finalize`'s check(g) does **not** read that receipt file — under finalize's zero-trust doctrine it re-establishes the same bound convergence in-process and rescans this same plan-scoped directory itself, per checked task. The plan-scoped directory — not a whole-root log scan — is the per-phase evidence of record; the receipt is the pipeline-time record of it, and check(g) re-verifies rather than trusts it.

**Legacy top-level baseline precondition.** A legacy unscoped log (sitting directly in `.dev/executor-logs/` outside any plan-scoped subdirectory) is never automatically attributed to a plan — the check(g) machinery only ever scans a plan's own scoped subdirectory, so a stray top-level log simply never enters evidence for any plan, never a false pass. Adopting plan-scoped logging in an existing repository is a one-time, manually-verified bootstrap step, not an automated gate: the person performing the adoption must confirm the top-level `.dev/executor-logs/` baseline is empty before relying on scoped logging as evidence of record; a non-empty baseline at that point is a hard stop requiring explicit human attribution before proceeding, not something the binary detects or enforces on its own.

#### ⚠️ SECURITY WARNING — bypass-permission

Headless executor adapters invoke secondary CLIs with a permission-bypass flag — `--dangerously-skip-permissions` (claude/agy), `--auto` (opencode), or `--allow-all` (copilot) — which grants the secondary CLI full filesystem and terminal access. A malicious or flawed agent contract could cause unintended file deletions, edits, or arbitrary command execution.

**Enable executor routing only in a trusted local environment.** The `Dispatch:` marker always appears before execution. The orchestrator must surface this warning to the user before first use.

### Runtime Step-Budget Preflight

Provider turn limits and OpenCode agent `steps` limits are hard runtime boundaries. GAL cannot remove them, so the pipeline must avoid treating a provider cutoff as a workflow decision.

Before starting the task loop:

1. If the active runtime is OpenCode, inspect the nearest repo `opencode.json` when present and note the active agent step budget when it is visible.
2. In OpenCode, enter **single-task tranche mode** by default. Only disable it when the user explicitly asks for a multi-task turn and the visible active-agent `steps` budget is high enough for that larger run.
3. If the active OpenCode agent is `build` and its `steps` value is `20` or lower, warn that even one full task may exceed the runtime budget and rely on the interrupted-phase handoff if the cutoff still happens.
4. In single-task tranche mode, complete at most one blocking task per invocation, including implement, the orchestrator correctness gate, test, and audit. A tranche boundary needs **no human-decision checkpoint** — per-task review is owned by the checking triangle (orchestrator correctness gate + auditor + end-of-run goal-backward verify), so there is nothing for a human to approve between tranches. Mechanical continuation is gated by the **B3 confirmed-ready allowlist** (see `### Runtime-Conditional Self-Reschedule (loop lane)`): a confirmed-ready runtime self-continues; otherwise (OpenCode is the actual tranche case and is **not** on the allowlist) the orchestrator marks the task complete and **leaves an auto-resumable resume marker** (an `Interrupted Phase` block a rerun or external scheduler picks up — not a human-decision stop). Do not claim a non-allowlist runtime can self-continue.

This is a normal continuation strategy, not a BLOCKED state. It prevents low-step agents from finishing multiple tasks and then being cut off mid-implementation on the next one.

**Runaway protection** is provided by the existing retry-ceiling-3, the human-required stops in `## Stop Conditions Reference`, and the finite task list — there is **no `max-duration`/`max-tasks` cap** (token cost is out of scope; budget is controlled via vendor/model choice in `~/.gal/config/config.json#executorRouting`).

If a runtime cutoff still occurs mid-phase and the next invocation sees `Workflow: IMPLEMENT`, `TEST`, `REVIEW`, `SECURITY`, or `VERIFY` with incomplete phase write-back, treat it as an interrupted phase and resume that phase before considering any new task.

---

## Step 1 — Read Plan and Verify Prerequisites

Select the plan file using this precedence order:

1. If the dispatcher emitted `PLAN: <path>`, or the raw command argument contains `@<path>` / `#file:<path>`, resolve that explicit path first. Strip a leading `@` before path resolution. If it points to a source plan and the matching `.dev/plans/<slug>.prompt.md` exists, use the execution prompt for this invocation.
2. Otherwise read the active plan file from `.dev/state.md`.

If the dispatcher emitted `TASK_REF`, validate that the referenced task exists in the selected plan before entering the task loop. If `FROM` and `STOP_AT` are both present, use them as the explicit execution bounds even when the user did not type `from` / `stop-at` directly in chat.

If an explicit `PLAN` path was provided but the file does not exist or is not a markdown plan/prompt file, stop and surface the exact path error.

The filesystem is authoritative for explicit plan resolution:

- If the explicit path already points to `.dev/plans/<slug>.prompt.md`, use it directly.
- If the explicit path points to `.dev/plans/<slug>.md` and `.dev/plans/<slug>.prompt.md` exists, switch to the prompt and use that as the pipeline file.
- If the explicit path points to a source plan and no prompt exists yet, use the source plan only to detect missing prerequisites, then stop with the required `/refining-plan` and `/plan-to-prompt` guidance instead of trying to execute against the source plan.

Verify:

- `## Tasks` exists with at least one blocking `T-NN` task
- `## Test Plan` exists in the plan file (required for golem-tester)
- No unresolved `BLOCKING` items in `## Review Results` at the root level
- Workflow state is not already `DONE`
- Any deferred or non-blocking follow-up lives outside `## Tasks` and is not used as a pipeline loop gate

If `Current Task` is set in `## Status` and no `from` argument was given, resume from that task. If `### Handoff Notes` contains an `OPEN` `Interrupted Phase` block, or if `Workflow` names a phase whose convergence gate is incomplete, resume that phase first.

Never run Step 2 task execution directly against a source plan when the matching execution prompt exists. `## Status`, retry counters, commit checkpoints, handoff notes, test results, and review results belong in `.dev/plans/<slug>.prompt.md`.

When an execution prompt exists, keep the paired source plan path in memory for closeout. The pipeline owns cross-file state convergence after each task passes all gates:

- `.dev/plans/<slug>.md` — human-readable source plan task checkbox and task commit note
- `.dev/plans/<slug>.prompt.md` — execution status, task checkbox, retry/review/test state, and resume markers
- `.dev/state.md` — active-plan last activity plus the matching per-plan session continuity row

Do not leave this convergence to implementer, tester, auditor, or a later chat. A task is not pipeline-complete until all three surfaces are updated and re-read successfully.

If prerequisites are not met: tell the user what is missing and stop. If the execution prompt is still stubbed, run `/refining-plan` on the source plan and then rerun `/plan-to-prompt` before attempting the pipeline again.

### Pipeline-Preflight Receipt Gate (binary, hard)

**Self-bootstrap first (mandatory when the plan touches `crates/`).** If any of the plan's `## Files to Create or Modify` entries (or task-block affected-file paths for individual tasks) are under `crates/`, **rebuild + reinstall `gal` before running any gate command in the task loop** (`cargo build --release` then install to the canonical root, and the Claude plugin-cache copy if present). Running a gate binary against pre-change code produces a receipt that describes the old binary, not the work under implementation — any `pass` from a stale binary is evidence of nothing. Use `gal --version`'s git stamp as the quick staleness probe when working in the GAL source repo. Skip the rebuild only when the plan touches no Rust source.

After the manual prerequisite check above, run:

```powershell
gal pipeline-preflight <execution-prompt-path> --receipt .dev/pipeline/receipts/<slug>-preflight.receipt.md
```

Read the receipt. **The receipt is the sole pass-basis — self-report is not accepted.**

- **`pass`** — all checks passed; continue to Step 2.
- **`fail` or `not-run`** — **STOP immediately**. Write or refresh an `Interrupted Phase — Step-1 / PREFLIGHT` block in `### Handoff Notes` with the receipt path and the failing check(s). Do not proceed to the task loop. Surface the exact binary output to the user.

The binary checks: `## Tasks` has ≥1 well-formed T-NN task; `## Test Plan` is present; no root-level `BLOCKING` marker; `Workflow` ≠ `DONE`; the prompt is the active plan in `.dev/state.md`. It also reads the resume cursor so the orchestrator can continue from the right task. `not-run` (e.g., wrong-plan guard fires because the prompt is not in state.md) is non-zero and is treated as fail.

### Retry And Blocker Handoff Contract

The active execution prompt's `## Status > ### Handoff Notes` is the durable human-takeover surface for pipeline failures. Do not leave takeover detail only in chat output.

When a task hits repeated failure or an immediate human-required stop, append or refresh a single task-scoped block in `### Handoff Notes` using this format:

```markdown
#### Retry Handoff — T-NN / [TEST | AUDIT]

- Status: OPEN | RESOLVED
- Problem: <latest blocking problem statement>
- Evidence:
  - Test Results: <latest task-scoped subsection or `not-applicable`>
  - Review Results: <latest task-scoped subsection or `not-applicable`>
  - Security Review: <latest task-scoped subsection or `not-applicable`>
- Attempts:
  1. <YYYY-MM-DD> — <attempt summary>
     - Result: <what changed or why it still failed>
     - Validation: <command, auditor verdict, or `not-run`>
     - Commit: <hash or `none`>
  2. ...
- Next human step: <exact next inspection or repair step>
```

Rules:

- Keep exactly one `OPEN` handoff block per `Current Task` and active phase. Update the existing block instead of appending duplicates.
- Record every retry-triggered fix attempt in order. By the third failed `TEST` or `AUDIT` round, the handoff must tell the human what was tried on attempts 1-3 without reconstructing history from chat.
- When a later rerun clears the issue, keep the block for history but change `Status` to `RESOLVED` and replace `Next human step` with the confirmation that cleared it.

### Interruption Handoff Contract

Use interruption handoff for non-decision runtime cutoffs such as OpenCode `steps` exhaustion, provider max-turn limits, context exhaustion, or terminal/tool availability ending a phase before its convergence gate is reached.

If the current invocation receives a runtime message equivalent to "maximum steps reached" or resumes and finds an incomplete current phase, write or refresh this block before doing any unrelated work:

```markdown
#### Interrupted Phase — T-NN / [IMPLEMENT | TEST | REVIEW | SECURITY | VERIFY]

- Status: OPEN | RESOLVED
- Cause: runtime-step-limit | context-limit | tool-unavailable | unknown
- Workflow at interruption: <Workflow value>
- Durable state present:
  - Task Base Commit: <hash or `missing`>
  - Task Final Commit: <hash or `missing`>
  - Worktree: clean | dirty | unknown
  - Test Results: <task-scoped subsection present? yes/no/not-applicable>
  - Review Results: <task-scoped subsection plus verdict present? yes/no/not-applicable>
- Resume action: <exact next command or phase action>
```

Rules:

- Keep exactly one `OPEN` interrupted-phase block per task and phase. Update it instead of appending duplicates.
- An interrupted phase is not the same as a failed phase. Do not increment `Test Retry Count` or `Review Retry Count` unless a real failing test or review finding exists.
- On resume, clear the interrupted-phase block only after the phase's normal convergence gate succeeds.
- If the interrupted phase is `IMPLEMENT` and the worktree is dirty, continue from the existing changes, run the scoped verification, commit, and record `Task Final Commit`. Do not start a new task.

### Runtime-Conditional Self-Reschedule (loop lane)

On a runtime turn/step cutoff the pipeline's default behavior is **continue-until-terminal**, but mechanical self-continuation is **runtime-conditional** and gated by a confirmed-ready allowlist:

1. **Always:** write/refresh the existing `Interrupted Phase` resume marker (above) so the run is auto-resumable by a rerun or external scheduler.
2. **Capability preflight:** check the active runtime against the **confirmed-ready allowlist** (currently `{Claude Code}`).
3. **If on the allowlist** and a loop primitive is available: self-re-enter the pipeline loop (continue-until-terminal) — e.g. Claude Code via `/loop` / `ScheduleWakeup` / background task.
4. **Otherwise:** silently **degrade to leaving the resume marker** — no install prompt, no error. A rerun or scheduler picks it up.

Honest-capability rule: loop is used **only on GAL-confirmed code agents**. An **upstream loop primitive existing ≠ GAL-confirmed** — Antigravity CLI (`/schedule`) and Copilot CLI (`/every`/`/after`) have upstream primitives but are **not** confirmed; Codex CLI and OpenCode have no native primitive. All non-allowlist runtimes degrade to the resume marker. **Adding a runtime to the allowlist requires explicit verification.** This is a behavior contract, not an implemented auto-scheduler — the per-runtime loop-primitive wiring is deferred (phase 2). The full capability matrix lives in the source plan.

---

## Step 2 — Task Loop

Repeat for each unchecked blocking `T-NN` task in `## Tasks` (in order, respecting `from` / `stop-at` and any single-task tranche mode). The pipeline **auto-advances** by default: when a task passes all gates the orchestrator moves to the next unchecked task automatically — no handback, no "keep running" prompt — and only stops on a genuine human-required condition (`## Stop Conditions Reference`), a `stop-at` boundary, or a runtime step-budget tranche.

### 2a — Working Hours Check

Pipeline execution is **exempt from working-hours by default** (see `conventions/working-hours.md` → Pipeline Execution Exemption). `/gal pipeline` is machine self-driving, not the user working late, so it does **not** stop at Wrap-up Time or Hard Stop and does **not** offer wrap-up between tasks. (Working-hours bounds interactive/chat work and direct golem calls only.) Proceed.

### 2b — Update Cursor

Update plan `## Status`:

```text
Current Task: T-NN
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0
Workflow: IMPLEMENT
Next step: implement T-NN
```

If `### Handoff Notes` contains an `OPEN` retry handoff for a previous task, mark it `RESOLVED` before starting the new task. Do not carry stale blocker state across tasks.

### 2c — Implement (CODER model)

**HEAD-Drift Discipline (implement phase):** Immediately before running the dispatch below, capture `git rev-parse HEAD`. After the dispatcher returns, before any orchestrator write step, capture `git rev-parse HEAD` again. Provenance compare: (a) the pre-dispatch HEAD must equal the written-back `Task Base Commit`; (b) the post-return HEAD must equal the written-back `Task Final Commit`. Either mismatch → **STOP immediately**. Record a Deviation in `## Status > ### Deviations` naming the unexpected commit(s) and do not proceed. This discipline applies to every implement dispatch including fix-mode re-dispatches in 2e and 2f.

Run:

```powershell
gal dispatch-script golem-implementer --pipeline-phase implement --task-scope T-NN '#file:<execution-prompt-path>'
```

**Always carry the active execution prompt path** (`#file:<execution-prompt-path>`, e.g. `#file:.dev/plans/<slug>.prompt.md`). The OFFLOAD dispatch target is that path — `gal pipeline` materializes the per-(task,phase) spec from it. Omitting the token makes the dispatcher emit `COMMAND: error` (it never falls back to an unmaterialized generated-spec path).

The dispatcher must emit `MODE: bound`, `DISPATCH_KIND: pipeline-phase`, `PIPELINE_PHASE: implement`, and `TASK_SCOPE: T-NN`. The implementer must:

1. Record `Task Base Commit` in `## Status` before any changes
2. Set `Current Task: T-NN` in `## Status` before reporting any implementation progress
3. Implement only the work required by `T-NN`
4. Record `Task Final Commit` in `## Status` when done
5. Ensure `git status` is clean before reporting complete
6. In pipeline fix mode, update the active `Retry Handoff` block in `### Handoff Notes` with the attempted remediation, validation result, and commit hash (if any)

**Hard Commit Gate:** If `Current Task` is missing or points at a different task, `git status` is not clean, or `Task Final Commit` is not recorded, do not proceed. Stop and surface the missing write-back instead of inferring completion from chat alone.

**Pre-commit allowlist diff guard (hard boundary):** Before creating the implementation commit, run `git diff --name-only` (plus `git status --porcelain` for new/untracked files) and compare every changed path against the task's allowlist — the affected-files set named in the task (task-block backtick paths, falling back to `## Files to Create or Modify`; the same allowlist the task spec emits). If any changed path is **outside** the allowlist, **STOP immediately**: do not commit. Treat the out-of-allowlist change as a boundary violation — revert it (`git checkout -- <path>` / remove the stray new file) or, if it is genuinely required, return to the task spec and widen the allowlist explicitly before re-running. Never silently accept an out-of-scope edit. (This is the hard counterpart to the task spec's soft allowlist directive; a dispatched executor that strayed beyond its files is caught here, at the orchestrator-owned commit boundary, before anything lands.)

**Boundary-Check Receipt Gate (binary, hard — runs before the implementation commit):**

```powershell
gal boundary-check <execution-prompt-path> --task T-NN --receipt .dev/pipeline/receipts/T-NN-boundary-check.receipt.md
```

Read the receipt. **The receipt is the sole pass-basis — self-report is not accepted.**

- **`pass`** — all changed files are within the task's affected-files allowlist; the orchestrator may create the implementation commit.
- **`fail`** — out-of-allowlist files detected. If the **only** out-of-allowlist change is a visibility-only edit reusing already-landed prior-task logic (confirmed by reading the diff — the `Task Base Commit..HEAD` range diff in dispatched mode), follow the **Boundary Widening Protocol** below. All other cases: **STOP immediately before committing.** Revert the stray changes or widen the allowlist in the task spec, then re-run the implementer for T-NN. Write or refresh a `Retry Handoff — T-NN / IMPLEMENT` block naming the violation paths from the receipt.
- **`not-run`** — the task names no affected files at all (no backtick paths in the task block and no `## Files to Create or Modify` entries), so there is no allowlist to check against. **STOP immediately.** Name the task's affected files (backtick paths in the task line, or a `## Files to Create or Modify` section) before re-running. `not-run` is never a pass; it is treated as fail. A standard-template prompt already carries this — no manual `Affected:` clause is required.

Do not create the implementation commit until this receipt reads `pass`.

**Dispatched-mode note:** The `gal boundary-check` receipt covers only residual uncommitted changes in the working tree. After the implementer's contract-mandated commit, `git diff HEAD` is empty, so the working-tree check passes vacuously for committed changes. In dispatched mode, the full boundary compare is carried by `git diff --name-only <Task Base Commit>..HEAD` against the allowlist; confirm this range shows only allowlisted paths before accepting the gate pass.

#### Boundary Widening Protocol

**Precondition (strict):** Use this protocol **only** when the boundary compare fails because the **sole** out-of-allowlist change is a visibility-only edit (for example, widening a symbol's visibility with no logic change) to a symbol that was already fully implemented and committed in a prior task — no new logic, no new behavior. Confirm by reading the diff: in dispatched mode, read the `Task Base Commit..HEAD` range diff. Anything else stays a **hard violation** — do not use this protocol to normalize scope creep.

1. **Widen the allowlist** — add the out-of-allowlist file to the task's affected-files list in the execution prompt with a one-line justification naming the reused symbol and the prior task that landed it.
2. **Commit the widening** — commit that allowlist edit alone as a doc-only commit (e.g., `docs(T-NN): widen allowlist — reuse <symbol> from T-MM`). Do not bundle it with implementation changes.
3. **Record a Deviation** — append a row to `## Status > ### Deviations` naming the reused symbol, the file, and the prior task.
4. **Re-run the boundary compare** — bundled mode: `gal boundary-check <execution-prompt-path> --task T-NN --receipt .dev/pipeline/receipts/T-NN-boundary-check.receipt.md`; dispatched mode: `git diff --name-only <Task Base Commit>..HEAD` against the now-widened allowlist. The gate must read `pass` before the implementation commit proceeds.

The implementation commit created for `T-NN` must stay scoped to the implementation itself. Do not use the implementation commit to record source-plan, execution-prompt, or `.dev/state.md` completion state for the task. Cross-surface progress or completion state belongs to the convergence step after all gates pass.

### 2d — Orchestrator Correctness Gate

Before test, the pipeline itself performs a full-context correctness gate. This gate is **not dispatched**.

Check checklist 1–13 from the plan's responsibility split, plus obvious performance problems visible from the implementation diff:

- task-spec conformance
- logical correctness
- boundary conditions at the obvious/local level
- error handling
- return values and side effects
- layer boundaries
- existing-pattern consistency
- abstraction fit (YAGNI)
- naming conventions
- file/module organization
- readability
- duplicate code
- dead code
- obvious performance issues

Check result:

- **Correctness gate fails**: **STOP immediately**. Return to implementer fix-mode for `T-NN`. Do not proceed to test. Write a `Retry Handoff — T-NN / IMPLEMENT` block that names the failed correctness checks and the next fix target.
- **Correctness gate passes**: update `## Status` `Workflow: TEST`, proceed to 2e.

### 2e — Test (TESTER model — different vendor from CODER)

Update plan `## Status`: set `Workflow: TEST`

**HEAD-Drift Discipline (test phase):** Immediately before running the dispatch below, capture `git rev-parse HEAD`. After the dispatcher returns, before any orchestrator write step, capture `git rev-parse HEAD` again. The tester must not commit; HEAD must be unchanged. Any drift → **STOP immediately**. Record a Deviation in `## Status > ### Deviations` naming the unexpected commit(s). Do not revert automatically. This discipline also applies to every implement fix-mode re-dispatch within this step.

Run:

```powershell
gal dispatch-script golem-tester --pipeline-phase test --task-scope T-NN '#file:<execution-prompt-path>'
```

**Carry the active execution prompt path** (`#file:<execution-prompt-path>`) — it is the OFFLOAD dispatch target; omitting it yields `COMMAND: error`.

The dispatcher must emit `MODE: bound`, `DISPATCH_KIND: pipeline-phase`, `PIPELINE_PHASE: test`, and `TASK_SCOPE: T-NN`. The tester writes a `### [T-NN] YYYY-MM-DD` subsection under `## Test Results`.

Check result:

- **No task-scoped subsection was written**: **STOP immediately**. Write `Retry Handoff — T-NN / TEST` with the missing write-back as the problem. Do not infer PASS or FAIL from chat alone.
- **`Workflow: TEST` is set but the latest task-scoped subsection is still missing or placeholder-only**: **STOP immediately**. Treat this as incomplete durable state, not as a passing or failing run.
- **A dispatched test phase reports PASS without matching evidence**: **STOP immediately**. For dispatched runs, PASS requires the named evidence shape for that task, including executor-log terminal state `completed` plus the observable write-back pointer. Write `Retry Handoff — T-NN / TEST` with the missing evidence as the problem. `DEGRADED_BUNDLED` runs still use reproducible `## Test Results` command output as their evidence and do not require executor logs.
- **Placement defect in the written-back subsection**: Before treating the `### [T-NN]` subsection as valid durable evidence, verify (a) its nearest enclosing `##` heading is `## Test Results` — not any other section — and (b) no existing Markdown table in the plan file has content inserted between its rows. If either defect is present: **STOP immediately**. Write `Retry Handoff — T-NN / TEST` with the placement defect as the problem. Do not treat a mis-placed or table-corrupting write-back as a passing or failing run.
- **All tests PASS**: update `## Status` `Workflow: AUDIT`, proceed to 2f
- **Any tests FAIL**:
  - Increment `Test Retry Count` in `## Status`
  - Refresh the active `Retry Handoff — T-NN / TEST` block with the latest failing test names, the current `## Test Results` subsection, and the next fix target
  - If `Test Retry Count` < 3: dispatch implementer to fix failing tests with `--pipeline-phase implement --task-scope T-NN --fix-mode '#file:<execution-prompt-path>'` (always carry the PowerShell-quoted prompt-path token), then re-run tester
  - If `Test Retry Count` = 3: **STOP**. Surface failures. Tell user the retry ceiling (3) has been reached for `T-NN`, include attempts 1-3 from the handoff block, and request human intervention

If the tests pass after one or more failed rounds, mark `Retry Handoff — T-NN / TEST` as `RESOLVED` and note the validation run that cleared it.

### 2f — Audit (AUDITOR model — different vendor from CODER and TESTER)

**HEAD-Drift Discipline (audit phase):** Immediately before running the dispatch below, capture `git rev-parse HEAD`. After the dispatcher returns, before any orchestrator write step, capture `git rev-parse HEAD` again. The auditor must not commit; HEAD must be unchanged. Any drift → **STOP immediately**. Record a Deviation in `## Status > ### Deviations` naming the unexpected commit(s). Do not revert automatically. This discipline also applies to every implement fix-mode re-dispatch within this step.

Run:

```powershell
gal dispatch-script golem-auditor --pipeline-phase audit --task-scope T-NN '#file:<execution-prompt-path>'
```

**Carry the active execution prompt path** (`#file:<execution-prompt-path>`) — it is the OFFLOAD dispatch target; omitting it yields `COMMAND: error`.

The dispatcher must emit `MODE: bound`, `DISPATCH_KIND: pipeline-phase`, `PIPELINE_PHASE: audit`, and `TASK_SCOPE: T-NN`. Invoke in task-scoped mode for `T-NN` with commit range `Task Base Commit..Task Final Commit`. The auditor writes a `### [T-NN] YYYY-MM-DD` subsection under `## Review Results`.

Check result:

- **No task-scoped subsection or verdict was written**: **STOP immediately**. Write `Retry Handoff — T-NN / AUDIT` with the missing write-back as the problem. Do not infer approval or block from chat alone.
- **`Workflow: AUDIT` is set but the latest task-scoped subsection still has no verdict**: **STOP immediately**. Treat this as incomplete durable state, not as approval.
- **A dispatched audit phase reports APPROVE without matching evidence**: **STOP immediately**. For dispatched runs, approval requires the named evidence shape for that task, including executor-log terminal state `completed` plus the observable review write-back pointer. Write `Retry Handoff — T-NN / AUDIT` with the missing evidence as the problem. `DEGRADED_BUNDLED` runs still rely on task-scoped `## Review Results` write-back instead of executor logs.
- **Placement defect in the written-back subsection**: Before treating the `### [T-NN]` subsection as valid durable evidence, verify (a) its nearest enclosing `##` heading is `## Review Results` — not any other section — and (b) no existing Markdown table in the plan file has content inserted between its rows. If either defect is present: **STOP immediately**. Write `Retry Handoff — T-NN / AUDIT` with the placement defect as the problem. Do not treat a mis-placed or table-corrupting write-back as valid approval or block evidence.
- **APPROVE (no BLOCKING)**: proceed to 2g
- **REQUEST_CHANGES or BLOCK (BLOCKING findings)**:
  - Increment `Review Retry Count` in `## Status`
  - Refresh the active `Retry Handoff — T-NN / AUDIT` block with the latest open BLOCKING findings, current review subsection, and the next fix target
  - If `Review Retry Count` < 3: dispatch implementer to fix BLOCKING issues with `--pipeline-phase implement --task-scope T-NN --fix-mode #file:<execution-prompt-path>` (always carry the prompt-path token), update `Task Final Commit`, then re-run auditor
  - If `Review Retry Count` = 3: **STOP**. Surface BLOCKING findings. Tell user the retry ceiling (3) has been reached for `T-NN`, include attempts 1-3 from the handoff block, and request human intervention

**Security / Protected Path escalation:** If any BLOCKING finding is a security vulnerability or Protected Path violation, **STOP immediately** regardless of retry count. Do not attempt an automated fix. Surface to human.

**Severity STOP rule:** If any high or critical audit findings remain open, **STOP immediately**. Preserve the audit STOP semantics even when the general retry path might otherwise continue.

If the audit passes after one or more failed rounds, mark `Retry Handoff — T-NN / AUDIT` as `RESOLVED` and note the auditor pass that cleared it.

### 2g — Mark Task Complete And Converge State

All gates passed for `T-NN`:

1. Resolve the durable state files for this task:

Execution prompt is the active `.dev/plans/<slug>.prompt.md`; source plan is the paired `.dev/plans/<slug>.md`; repo state is `.dev/state.md`.

1. Mark `T-NN` as complete in the execution prompt `## Tasks` and in the source plan `## Tasks`.

Preserve the existing task text. If the task line has no commit note, append `*(<Task Final Commit>)*`. If the task line already has a stale or missing audit note from an earlier correction, replace it with the final commit note only after the task truly passed implement + test + review.

1. Update the execution prompt `## Status`:

  ```text
   Last activity: YYYY-MM-DD — T-NN complete (commit: <Task Final Commit>)
  Current Task: —
  Task Base Commit: —
  Task Final Commit: —
  Test Retry Count: 0
  Review Retry Count: 0
  Next step: implement <next unchecked T-NN> | run verify | finalize
   ```

1. If the execution prompt uses a `### Completed Tasks` table or a `### Remaining Tasks` list inside `## Status`, update those summary surfaces too. Move `T-NN` into completed with `<Task Final Commit>`, remove it from remaining, and ensure the next unchecked task matches `Next step`.

1. Update `.dev/state.md`. Keep the active plan row valid. The `File` column may point at the source plan or the execution prompt, but `/gal status` and `/gal whats-next` must still be able to resolve the paired prompt. Set the active plan row `Last Activity` to `YYYY-MM-DD`. Update or create the matching `## Session Continuity` row for this plan, keyed by the paired source plan path. Set `Stopped At` to `T-NN complete (commit: <Task Final Commit>)`, `Next Step` to the next unchecked task, verify, finalize, or the explicit `stop-at` boundary, and refresh `Last Session` plus any needed context. Do not overwrite other plans' continuity rows.

1. Re-read all three files and run the **Task State Convergence Gate**.

The gate passes only when:

- source plan has `- [x] T-NN`
- execution prompt has `- [x] T-NN` when it carries a `## Tasks` task list
- execution prompt `## Status` no longer leaves `Current Task: T-NN` with stale commit markers after task closeout
- the matching `.dev/state.md` session continuity row no longer points at the completed task as unfinished
- source plan and execution prompt do not disagree about which blocking `T-NN` tasks are checked

If any convergence check fails, **STOP immediately** and write an `Interrupted Phase — T-NN / VERIFY` block explaining the missing write-back. Do not report the task complete from chat memory alone.

No git commit that records task progress or task completion in `.dev/plans/<slug>.md`, `.dev/plans/<slug>.prompt.md`, or `.dev/state.md` may be created or retained before this convergence gate passes. In-flight task-local edits may remain uncommitted while gates are still running, but the repository history must never contain a committed cross-surface disagreement.

If a state-recording commit was created too early and the three durable surfaces do not yet agree, **STOP immediately** and repair convergence before advancing to another task. Do not treat a premature state commit as an acceptable intermediate state.

**Pipeline-Converge-Check Receipt Gate (binary, hard — runs after the convergence write-backs above):**

After all three surfaces are updated and re-read, run:

```powershell
gal pipeline-converge-check <execution-prompt-path> --task T-NN --receipt .dev/pipeline/receipts/T-NN-converge-check.receipt.md
```

Read the receipt. **The receipt is the sole pass-basis — self-report is not accepted.**

- **`pass`** — source plan and prompt agree; T-NN is `[x]` on both surfaces; commit hash exists; cursor is cleared. The task is pipeline-complete.
- **`fail` or `not-run`** — **STOP immediately**. Write or refresh an `Interrupted Phase — T-NN / VERIFY` block in `### Handoff Notes` with the receipt path and the specific failing check(s). Do not advance to the next task. Repair the noted surface divergence, then re-run the binary until receipt reads `pass`.

`not-run` (e.g., missing commit hash on the task line) is non-zero and is treated as fail. The prose convergence self-check above is still required; the binary adds a machine-verifiable receipt that cannot be self-reported.

1. If `stop-at T-NN` was specified and this task matches: **STOP**. Report task complete and prompt user before starting the next task.

1. If single-task tranche mode is active: report the task complete and **leave an auto-resumable resume marker** (refresh the `Interrupted Phase` block so a rerun or external scheduler continues from the next unchecked task). This is a runtime step-budget tranche, **not** a human-decision stop — there is no checkpoint to approve (the checking triangle already reviewed the task). A confirmed-ready runtime (B3 allowlist) self-continues instead of stopping here.

1. Otherwise: **auto-advance** to the next unchecked task and return to 2a — automatically, with no handback and without asking. All gates passed, so the pipeline continues on its own until natural completion or a genuine human-required stop (`## Stop Conditions Reference`).

---

## Step 3 — Orchestrator Goal-Backward Verification

After all unchecked tasks are complete, the **ORCHESTRATOR** runs a plan-level goal-backward verification pass. Its spec is inlined below.

**Verification independence (policy):** goal-backward verification is **ORCHESTRATOR-owned and always runs in-process** — it is never dispatched to another model and there is no `verify` dispatch phase or `VERIFY` routing key. The cross-model independence guardrail is carried by **AUDITOR** (the independent ≠coder per-task deep audit), not by a separate verify dispatch. (This is distinct from the run-level `DEGRADED_SAME_RUNTIME` mark, which is about CODER/TESTER/AUDITOR vendor separation, not verify.)

Run **goal-backward verification only** (the four steps below). Do **NOT** perform lifecycle ending here (ABSORBED marking + plan-file deletion) — that is a separate ORCHESTRATOR post-finalize action (see Step 3 Lifecycle note below).

### Inline Goal-Backward Verification Spec

**Mindset:** Do NOT trust claims about what was done. Verify what ACTUALLY exists. Task completion ≠ goal achievement. Work backwards from the outcome across three levels:

- **Level 1 — Truths:** the plan's requirements as observable behaviors. Can a user actually do what the plan promised?
- **Level 2 — Files / Components:** what must EXIST for those truths to hold, with substantive content (not stubs)?
- **Level 3 — Wiring:** what must be CONNECTED (DI, routes, imports, subscriptions). A component can EXIST without being WIRED; a route can be DEFINED without being REACHABLE.

1. **Extract must-haves** from the plan's success criteria → Truths / Files-Components / Wiring.
2. **Verify each level** with evidence (PASS/FAIL per item). For browser-visible claims, record which runnable route produced evidence (Playwright MCP / Chrome DevTools MCP / Native Playwright / `No runnable browser route`); never mark a browser-visible claim VERIFIED without a runnable route.
3. **Run verification commands** — test suite + build (`cargo test` / `dotnet test` / `npm test`; build) and any plan smoke tests.
4. **Produce the verdict** — a must-have table + failed-item evidence + test/build status, ending in one of:
   - **VERIFIED** — all must-haves pass
   - **GAPS_FOUND** — some items fail (list them for the implementer)
   - **BLOCKED** — critical issues prevent verification

Check result:

- **VERIFIED**: proceed to Step 4 (final gate)
- **GAPS_FOUND**: **STOP**. Surface each gap with its description. Tell the user to resolve the gaps, then rerun `/gal pipeline` from the same plan once the blockers are cleared.
- **BLOCKED**: **STOP**. Surface the blocking condition. Tell user to resolve before finalize.

### Lifecycle Ending (ORCHESTRATOR, post-finalize — not part of the pipeline verify)

Marking the plan `ABSORBED` and deleting the plan files is a **post-finalize ORCHESTRATOR** action, never performed inside Step 3 or by the audit/verify phases. Sequence after finalize: STEWARD extracts durable knowledge → `docs/` (its charter), then ORCHESTRATOR marks `ABSORBED` and deletes `.dev/plans/<slug>.md` + `.dev/plans/<slug>.prompt.md` and trims the `.dev/state.md` rows. Never delete a plan that has not passed verification.

---

## Step 4 — Final Gate

Report the combined verdict:

```text
--- PIPELINE COMPLETE ---

Tasks completed: N of N
  T-NN  ✓ implement · test · review
  T-NN  ✓ implement · test · review · security (when run)
  ...

Goal verification: VERIFIED

Overall: READY TO FINALIZE
```

Tell the user the plan is ready for `/gal finalize`. If the user is pausing instead of landing the plan now, route them to `/gal wrap-up` rather than pushing them to finalize.

Also remind the user to rerun `/graphify .` before the next graph-aware planning or review pass so `graphify-out/` reflects the implementation that just completed.

If any task or the verify pass is blocked, report with detail:

```text
--- PIPELINE BLOCKED ---

Task:    T-NN
Phase:   [IMPLEMENT | TEST | REVIEW | SECURITY]
Reason:  [description]
Retry Count: N of 3 | not-applicable

Attempts tried:
1. [attempt summary]
2. [attempt summary]
3. [attempt summary]

Evidence to inspect:
- Handoff Notes: `## Status > ### Handoff Notes`
- Test Results: [latest task-scoped subsection or `not-applicable`]
- Review Results: [latest task-scoped subsection or `not-applicable`]
- Security Review: [latest task-scoped subsection or `not-applicable`]

Action required: [what the user needs to do]
```

The blocked output must mirror the active `Retry Handoff` block closely enough that a human can answer three questions immediately: what failed, what was tried already, and what artifact or file to inspect next.

---

## Non-Script Fallback

If the script cannot be run (e.g. macOS / Linux), run:

```bash
gal dispatch-script golem-implementer --pipeline-phase implement --task-scope T-NN '#file:.dev/plans/<slug>.prompt.md'
```

When routing maps the phase role to an executor this emits an `OFFLOAD` block whose `ACTION` runs `gal pipeline '<prompt-or-plan-path>' --phase … --task … --receipt …` (the `#file:` path is threaded through; omit it and the ACTION falls back to the materialized-spec path). Follow the block exactly. With no routing it emits a bound text block — role-play the golem in conversation. (Equivalent phase-marked invocations for tester and auditor; verify is not a dispatch phase — it is orchestrator-owned and always run in-process via the inline spec in Step 3.)

Or invoke each golem directly by asking the user to switch to the appropriate AI model and following the respective agent file:

- `plugins/gal-core/agents/golem-implementer.agent.md`
- `plugins/gal-core/agents/golem-tester.agent.md`
- `plugins/gal-core/agents/golem-auditor.agent.md`
- Verify: no persona file and no dispatch — the orchestrator runs the inline goal-backward spec in Step 3 always in-process

---

## Stop Conditions Reference

**Autonomy waives no human-required stop.** Default autonomous continuation removes only the working-hours stop and the manual keep-running/tranche checkpoint; every genuine human-required condition below still halts the pipeline. The four human-required stop classes are: **BLOCKING security / Protected Path**, **retry ceiling (3)**, **goal-backward verify GAPS_FOUND/BLOCKED**, and **three-surface convergence failure**.

| Condition | Action |
| --- | --- |
| BLOCKING security vuln or Protected Path | STOP immediately — human required |
| Conditional security audit leaves high or critical findings open | STOP immediately — human required |
| `Test Retry Count` reaches 3 | STOP before 4th attempt — human required |
| `Review Retry Count` reaches 3 | STOP before 4th attempt — human required |
| Goal-backward verify returns GAPS_FOUND or BLOCKED | STOP — surface gaps, human required |
| Three-surface convergence gate fails (2g) | STOP immediately — repair convergence before advancing |
| HEAD-drift detected after any dispatch returns | STOP immediately — record Deviation naming unexpected commit(s); no automatic revert |
| `stop-at T-NN` reached | STOP — prompt user before continuing |
| Single-task tranche complete (non-allowlist runtime) | Not a stop — leave auto-resumable resume marker; a rerun/scheduler (or a confirmed-ready runtime) continues |
| Runtime step or turn limit interrupts a phase | Write/refresh `Interrupted Phase — T-NN / PHASE`; confirmed-ready runtime self-re-enters the loop, else resume marker awaits rerun |
| All tasks + goal-backward verify VERIFIED | Natural completion — READY TO FINALIZE |
