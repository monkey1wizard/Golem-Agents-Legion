---
name: gal-pipeline
description: "GAL pipeline ($gal-pipeline / /gal pipeline / 跑 pipeline / 實作 / implement / run the pipeline). Entry Latch: run gal pipeline-preflight receipt BEFORE any implementation edit. Iterates T-NN tasks: implement → correctness gate → test → audit → commit → converge. Voluntary final output requires a fresh pipeline-handback-check receipt."
---

# /gal-pipeline

Run the full implementation pipeline task by task: for each blocking `T-NN` task in the active plan, run implement → orchestrator correctness gate → test → audit in sequence, then advance to the next task. Prefer different AI vendors per `~/.gal/config/config.json#executorRouting` across CODER, TESTER, and AUDITOR. After all blocking tasks complete, run a final goal-backward verify pass.

## Role

Pipeline orchestrator. Your job is to iterate through plan tasks automatically, advancing only when each task's commit + correctness gate + test + audit gate is fully clean, and stopping only when a genuine human-required condition is encountered.

## Entry Latch (binding — no edit before this passes)

**Before any implementation edit, you MUST:**

1. Run `gal pipeline-preflight <execution-prompt-path>`; the default receipt is `.dev/pipeline/receipts/<plan-scope-key>/preflight.receipt.md`.
2. Read the receipt. Only `overall: pass` allows proceeding. `fail` or `not-run` → STOP immediately — do NOT make any file edits.
3. If the user sends multiple plan references in one request, expand them to an **ordered run list** and run each plan fully (preflight → task loop → goal-backward verify) before starting the next. Never interleave tasks across plans.

This latch is not optional. A pipeline that skips it is a `named workflow obedience failure`.

## Terminal-Reverify Entry Branch

When finalize routing identifies stale goal binding with otherwise clean passing state, enter the `terminal-reverify` branch with prompt input only:

1. Run `gal.exe pipeline-preflight --terminal-reverify <execution-prompt-path>` and read the fresh `terminal-reverify.receipt.md` receipt. Proceed only when it passes the terminal invariants: `Workflow: DONE`, every task checked, `Current Task` cleared, no open retry/interruption or human handoff, and a clean working tree. Ordinary preflight continues to reject `Workflow: DONE`; this branch bypasses only that ordinary rejection and retains marked-prompt contract validation, including dual-schema validation, transition-journal digest binding, and recording the bound marked terminal receipt chain.
2. On a passing receipt, the only authorized action is the ORCHESTRATOR-owned, in-process goal-backward verification followed by a fresh `pipeline-handback-check`. Use the resulting handback receipt as the authority for the next control-plane action, then return to finalize only when that receipt authorizes it.
3. The following actions are prohibited in this branch: dispatching implement, test, audit, or security phases; running executor work; committing; mutating or rewriting the prompt; repairing evidence; or weakening, bypassing, or fabricating any gate, receipt, digest, handoff, or goal-binding evidence. A failed terminal-reverify gate authorizes no action and must remain on its recorded recovery route.

## Pipeline Handback Authority

`gal.exe pipeline-handback-check <execution-prompt-path> [--stop-at T-NN]` is the sole authority for whether this invocation may return a voluntary final response. Narrative status, prompt prose, `Interrupted Phase` markers, runtime step limits, and model judgment never authorize final output.

Run the checker immediately after every task convergence gate and again immediately before every voluntary final response. Use the explicit execution prompt path and the same `--stop-at` target, when supplied. Read the fresh plan-scoped receipt; its only authoritative fields are exactly `(decision, reason, final_authorized, next_action)`:

```text
decision: continue | ready-to-finalize | human-required | retry-ceiling | stop-at
reason: none | security-protected-path | goal-gaps-blocked | head-drift | boundary-scope-decision | convergence-human-repair
final_authorized: true | false
next_action: exactly one closed control-plane action
```

For `continue`, the closed action forms are `gal.exe pipeline <prompt> from <task> [stop-at <target>]`, `run-goal-backward-verification`, `repair-stop-at-target <target>`, and `repair-stop-at-convergence <target>`. A pending valid future `stop-at` uses the pipeline action, carries the same target, and advances to the next unchecked task. The two repair actions require the orchestrator to repair the named invalid target or stale convergence state before rerunning the checker; they never mean "run the checker again unchanged."

Classification precedence is: reached and re-proven `stop-at`; retry ceiling; typed human-required producer with one valid handback; incomplete or invalid state as repair `continue`; pending valid future `stop-at` as next-task `continue` with the target preserved; all tasks checked without a fresh bound goal record as `run-goal-backward-verification`; fresh checked completion with a bound `VERIFIED` goal record as `ready-to-finalize`. Only `ready-to-finalize`, `human-required`, `retry-ceiling`, and a proven `stop-at` may have `final_authorized: true`; `continue` always has `reason: none` and `final_authorized: false`.

Every `human-required` result requires exactly one `OPEN` block under `### Handoff Notes` headed `#### Human Handback — <reason>` with fixed fields `Status`, `Reason`, `Task`, `Phase`, `Producer`, `Producer state`, `Git HEAD`, and `Next human step`; `head-drift` also requires `Baseline HEAD` and `Observed HEAD`. `Task` is `T-NN|none`; `Phase` is `IMPLEMENT|TEST|AUDIT|VERIFY|BOUNDARY|CONVERGE`. Reject duplicate `OPEN` blocks, stale bindings, wrong producer/reason/task/phase, or missing fields. Resolved historical blocks may remain in the notes and are ignored by the checker. Mark the block `RESOLVED` when the issue clears. For `goal-gaps-blocked`, write and validate this handback before hashing the prompt or writing the goal-verification record.

On `continue`, execute the literal `next_action` in the same invocation without asking the owner to keep running. `run-goal-backward-verification` is not a public CLI subcommand: it tells the ORCHESTRATOR to run Step 3 in-process, emit the goal-verification receipt, and rerun the checker. The checker validates freshness and state consistency only; it never performs semantic goal verification. Progress updates are commentary-only. On an authorized terminal decision, return only the receipt-backed final response. Runtime cutoff is recovery-only: write or refresh `Interrupted Phase`, leave a rerunnable resume marker, and never authorize final output or schedule Codex to wake itself. Do not add model-specific routing or host-level sampling claims.

## Codex / PowerShell Invocation Safety

These rules keep the Codex orchestrator on the Rust dispatch path instead of silently degrading to in-chat role-play.

- **Quote the `#file:` token in PowerShell.** In PowerShell a bare `#` starts a comment, so an unquoted `#file:<prompt>` is discarded before `gal dispatch-script` ever sees it → the dispatcher receives no prompt path and emits `COMMAND: error`. Always paste the token single-quoted: `'#file:.dev/plans/<slug>.prompt.md'`. Bash/Zsh tolerate the unquoted form, but the quoted form is safe in every shell — prefer it everywhere. Every `gal dispatch-script … '#file:<execution-prompt-path>'` example below is written this way.
- **Explicit prompt path when more than one plan is active.** When `.dev/state.md` lists multiple active plans, do **not** let Step-1 resolution fall through to the first active-plan row. Require the user to name the plan explicitly with `'#file:<prompt>'` (or `@<prompt>`), and dispatch only that resolved execution prompt. Silent first-row selection is a wrong-plan hazard — stop and ask for the explicit path instead.
- **Sandbox receipt/log write denial is a HARD environment stop, not permission to role-play.** If `gal pipeline-preflight`, `gal boundary-check`, `gal pipeline-converge-check`, or a `gal pipeline … --phase …` dispatch fails because `.dev/pipeline/receipts/` or `.dev/executor-logs/` could not be written (Codex sandbox / access-denied), **STOP**. Rerun the exact same Rust command after the user grants the required write approval. Never fall back to implementing, testing, auditing, or "verifying" the phase in chat because a receipt/log write was denied — a denied write is missing evidence, and missing evidence is never a pass.
- **Fix-mode fail-closed rules.** A retry is `gal.exe pipeline <prompt-or-source-plan> --phase implement --task T-NN --fix`. Zero or multiple OPEN `Retry Handoff — T-NN / IMPLEMENT|TEST|AUDIT` blocks, or an unchanged stable authority fingerprint, are pre-spawn stops; do not consume retry budget or dispatch. A completed fix executor whose affected implementation files are unchanged exits `fix-round-no-change`, appends the loop-log event, and is not accepted as a successful retry.

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
| Audit | `golem-auditor` | AUDITOR | Independent deep-performance and security audit (single task) |
| Verify | ORCHESTRATOR (inline spec, always in-process) | — | Goal-backward plan verification, owned by the orchestrator — always in-process, never dispatched; see Step 3 |

The orchestrator's correctness gate is not dispatched: it is the pipeline's own full-context check for checklist 1–13 plus obvious performance before test. The end-of-run goal-backward verification (Step 3) is likewise orchestrator-owned and always runs in-process (never dispatched).

**Checking-role triangle:** ORCHESTRATOR (dispatch + correctness gate + goal-backward verify + lifecycle) · AUDITOR (single-task deep perf + security) · STEWARD (knowledge extraction → `docs/` + doc structure).

### Runtime Preflight

Before starting the task loop, the orchestrator runs `gal pipeline-preflight` as the Entry Latch requires, reads its first stdout line as the resolved route context, announces that line once verbatim, and proceeds.

### Phase Separation Contract

The route statement printed by `gal pipeline-preflight` identifies the resolved executor and model for all three dispatch roles so a reader can see which specific check is weaker rather than only that something is.

- Keep implement, test, audit, and verify as separate bounded phase invocations with their normal durable write-back requirements.
- Do not silently bundle implement + test + audit just to save tokens. same-runtime bundling is allowed only when the user explicitly asks for it.
- If the user explicitly asks for bundled execution, mark the run as `Verification Independence: DEGRADED_BUNDLED`, keep separate task-scoped `## Test Results` and `## Review Results` write-back, and state clearly that same-runtime bundling reduced tester/auditor independence for this invocation.
- Never waive retry ceilings, protected-path escalation, audit STOP rules, interrupted-phase handoff, or final verify requirements.

### Loop-Log: In-Conversation Error Capture (hard step)

The headless dispatch path writes its own loop-log event (one per `terminal_state`) automatically. When a phase runs **in-conversation** (no dispatch attempt log exists for this phase at all), there is no dispatch process to do that — so the orchestrator MUST record its own failures itself. This is a **hard step**, not optional:

- When an in-conversation phase hits a durable failure or degrade — a **naming-gate block**, a **test failure**, a **compile error**, an **out-of-allowlist boundary violation**, or a **retry** — append a structured loop-log event:

  ```powershell
  gal pipeline-log append --task T-NN --phase <phase> --role <ROLE> --kind <kind> --level <level> --msg "<short failure-only summary>"
  ```

  where `<kind>` is one of `naming-gate-block` / `test-fail` / `compile-error` / `executor-no-receipt` / `executor-timeout` / `boundary-violation` / `degrade` / `retry`, and `<level>` is `warning` (recoverable/degraded) or `error` (failure needing attention).

- **Failure-only**: do not log pass detail. One line per real failure/degrade, not per successful step.
- The subcommand writes to the machine-local gitignored loop-log (`.dev/pipeline/loop-log/<date>.ndjson`) and redacts obvious secrets from `--msg`. It is a pipeline-internal subcommand (like `gal naming-gate` / `gal finalize-check`), not part of the public `/gal` command surface.
- This append is a **convenience for forensics, not a workflow gate** — it never blocks a task. Its purpose is that the next debugging session can grep the loop-log instead of re-deriving what failed from chat memory (which is exactly the gap that forced manual bug-doc back-fills before).

### Headless Executor Dispatch

When `~/.gal/config/config.json#executorRouting` is present and maps the current phase's role to a CLI executor, the **self-contained `gal` binary** takes over dispatch: `gal dispatch-script` emits the headless `OFFLOAD` block, and `gal pipeline <prompt-or-plan-or-spec> --phase <p> --task <t> [--receipt <target>]` runs the executor **inside the `gal` binary's own process** (`dispatch::run`; distinct from a phase the orchestrator role-plays *in-conversation*) — there is no separate `gal-dispatch` sibling executable. When routing is absent, dispatch emits a regular `--- GAL DISPATCH ---` text block (backward-compatible, no regression).

**OFFLOAD is keyed on `(task, phase, routing)` — not on `.dev/state.md`.** A routed executor for the phase role is sufficient to emit the `OFFLOAD` block; the dispatcher does **not** parse `state.md` to decide whether to offload (the prior dependency on a `state.md` active-plan *table* silently disabled offload whenever `state.md` used a bullet list). The real safety gate (routing + executor-in-PATH + readiness probe) lives downstream in `gal pipeline`/`dispatch::run`.

**`gal pipeline` input resolution.** The dispatch target is a path, resolved by kind: a `*.prompt.md` execution prompt (preferred — `gal pipeline` reads it and materializes the per-(task,phase) spec internally, scoped to the single `T-NN`); a `.dev/plans/<slug>.md` source plan (auto-switched to the paired `.dev/plans/<slug>.prompt.md`, or a hard error pointing at `/refining-plan` + recorded human approval + `/plan-to-prompt` if no prompt exists — never dispatch a source plan); or an already-materialized task spec (fed as-is). The `OFFLOAD` block's `ACTION` carries the explicit prompt/plan path when one was provided.

**Receipt freshness is dispatcher-owned.** Before a local executor starts, the binary resolves the receipt against `--workdir` and atomically acquires a dispatcher-owned per-path lease beneath `<workdir>/.dev/pipeline/receipts/.locks/`. It holds that lease from before invalidation through post-run verification. A competing GAL dispatch for the same resolved path therefore fails before spawn. A pre-existing path safely contained under the managed receipts root is invalidated; a pre-existing or link/reparse-mediated explicit path outside that root fails closed and is never deleted. Verification requires a non-empty regular file with no existing link/reparse component. Preparation occurs before attempt-log allocation, so preparation failure cannot leave a `started` marker. If child termination is unconfirmed after timeout cleanup or a wait I/O error, the guard deliberately retains a stale lease rather than reopening the receipt path to a possibly running child.

Remote dispatch creates an owner-token lease directory atomically in the remote managed root, checks every existing receipt component for links, and performs managed invalidation or the external non-existence guard before the executor. The lease remains remote-owned after the executor SSH process exits and is released only after the separate control-node fetch finishes; cleanup verifies the owner token. Fetch runs a second same-command component/link and final regular-file check before `cat --`, then atomically creates the single workdir-resolved control-side destination without overwrite. Fetch and cleanup SSH calls have fixed time bounds; both pipes are continuously drained while retaining at most 1 MiB per pipe, drains are joined only after auxiliary-process exit is confirmed, partial stdout after a pipe read error is rejected as `remote-receipt-fetch-read-failed`, and an oversized receipt fails closed as `remote-receipt-fetch-too-large`. Unconfirmed auxiliary cleanup uses a distinct `*-unconfirmed` reason. Wrapper freshness failures carry a dedicated sentinel, the exact lock path, and exit 73, while an executor's own exit 73 remains unchanged; the sentinel distinguishes ordinary executor provenance without rewriting it. A crash, unconfirmed timeout, or unconfirmed cleanup intentionally leaves a stale lease and later attempts fail closed until an operator inspects and removes that specific lock. Checked local/control-side release covers every prepared-guard exit and downgrades with `receipt-lease-cleanup-failed`; if a primary guard/log failure also occurred, reason/evidence preserves both with `+` or combined error text. Remote cleanup failure/timeout reasons likewise compose with an earlier fetch reason using `+`. The lease serializes GAL dispatches; it is not a filesystem sandbox against unrelated privileged processes racing path components between checks. Raw/direct dispatch keeps its active unscoped managed default, so flat receipts are not universally inert.

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
| agy      | `--dangerously-skip-permissions --model <m>`                         | `--effort <value>`                      | stdin         |

**Permission-bypass flags vs `effort` — two different concepts.** The permission-bypass
flags (`--auto` for opencode, `--allow-all` for copilot, `--dangerously-skip-permissions`
for claude/agy) grant the secondary CLI full filesystem/tool access so it can write the
receipt headlessly; they are **not** reasoning controls. `effort` is a separate,
per-role reasoning-intensity hint (the `effort` route-entry key), mapped to each
executor's native reasoning flag above. Set it as `"effort": "high"` on the role — never
as a raw flag bag.

- **`effort` is fail-closed.** Any adapter that has not opted in rejects effort before spawn with `unsupported-effort`.
  A value outside `[A-Za-z0-9._-]+` is rejected with `invalid-effort`. Neither dispatches.
- **agy** enforces an exclusive-or rule: for models with effort tiers, the tier must be supplied exactly once — either as a model slug suffix or via `--effort`, never both and never neither.
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

4. Acquire the atomic receipt lease and establish the link-aware freshness precondition, then spawn the executor, feed spec, enforce timeout, and kill the process tree on timeout. Receipt-preparation or lease failure is a pre-spawn fail-closed outcome and allocates no attempt log.
5. After exit 0: while still holding the lease, read the prepared receipt path to confirm a current-run non-empty regular-file write-back. Remote execution holds both control and remote leases through atomic fetch. Exit 0 without that evidence → terminal state `no-receipt`. **Do not accept exit code 0, pre-existing bytes, or another dispatch's write-back.**
6. For in-scope dispatches (the `audit` phase for every prompt, and the `test` phase on a markerless prompt; `implement`, `scaffold`, and `test` on a marked prompt are out of scope), receipt delivery is necessary but not sufficient for phase success: executor `completed` represents receipt delivery only, and the `gal` binary executes a binary-owned semantic commit to validate and append the staged receipt subsection to the execution prompt's `## Review Results` or `## Test Results` section before phase success. Direct execution-prompt edits by an in-scope executor are prohibited.
7. Extract provider-native session/job id from stdout (tool-specific JSON field; agy scans `~/.agy/brain/`). Record in executor-log header and `Dispatch:` marker.
8. Write durable executor log and output the `Dispatch:` marker line to stdout.

#### Agent Contract Resolution & Delivery

Before dispatch, for prompt/source-plan pipeline inputs, the bin resolves and reads the phase's authoritative agent contract (`agents/golem-{implementer|tester|auditor}.agent.md`) and embeds its exact bytes into the task spec under `## Agent Contract`. No dispatched executor, local or over the SSH lane, is ever instructed to read a control-node-only contract path — the spec is self-contained.

**Resolution order (first source root wins):**

| Tier | `contract_source` | Root |
| --- | --- | --- |
| 1 | `workdir` | the canonicalized `--workdir` itself, or its direct `plugins/gal-core` |
| 2 | `ancestor` | the nearest ancestor of workdir recognized as a GAL source root |
| 3 | `exe-side` | the directory beside the canonicalized running `gal` binary |
| 4 | `embedded` | materialized at `~/.gal/embedded-src` |

`workdir` beats `ancestor`/`exe-side`/`embedded`; `ancestor` beats `exe-side`/`embedded`; `exe-side` beats `embedded`. This keeps a trusted local GAL checkout authoritative even when a packaged `gal` binary is also on PATH — a repo vendoring `plugins/gal-core/` always resolves `workdir` regardless of the installed binary's own version, so `contract_source` is the inspection point for vendored-contract version skew.

**Inline delivery, never a path reference.** Once a root wins, its exact phase contract file is read once and rendered verbatim under `## Agent Contract` in the assembled task spec. This is what makes one spec self-contained for both local and SSH-lane dispatch.

**A corrupt winning root fails loudly and never falls through.** If the winning root's phase contract is missing, non-UTF-8, or unreadable, dispatch stops before spawn with a corruption error naming the tier and root. A corrupt higher-tier root is not treated as an absent one — resolution does not silently continue to a lower tier.

**All-miss stops before spawn with guided recovery.** If no tier yields a usable root, dispatch stops (exit 1) with all four tiers' outcomes listed plus both recovery routes: reinstall `gal` via the packaging channel, or run from a GAL source checkout.

**Raw/direct compatibility.** Raw `gal dispatch` and raw task-spec `gal pipeline` inputs neither resolve nor invent provenance. Their markers and executor-log headers stay byte-compatible with the pre-provenance format — no `contract=`/`contract_source=` fields appended.

#### Dispatch Observability Marker

The bin outputs to stdout after every run:

```
--- GAL DISPATCH ---
Dispatch: phase=<p> task=<T-NN> role=<ROLE> executor=<cli> model=<m> state=<terminal-state> session_id=<id|none> log=<path> [contract=<control-node-abs-path> contract_source=workdir|ancestor|exe-side|embedded]
```

The bracketed `contract`/`contract_source` pair appears only on pipeline-created dispatches for prompt/source-plan inputs — the same coupled path/source value carried through `SpawnConfig` and reused in the executor-log headers below. Raw/direct dispatch omits it.

**Provenance-gated `effort` field.** For pipeline-created dispatches only (provenance present), every success/degrade marker in the run carries a sanitized ` effort=<value|(default)>` field immediately before the `contract`/`contract_source` suffix, computed once after route resolution and reused unchanged for the rest of that run. Raw/direct dispatch and the `no-routing` degrade (no route was resolved, so no provenance) never carry an `effort` field — their markers stay byte-identical to the pre-effort format.

Degrade variants (exit 2, bin is the output owner) append the same optional `effort`/`contract`/`contract_source` suffix when provenance is present:
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

#### REPORT_LINE Announcement (exact-once, verbatim)

When routing produces an `OFFLOAD` block, `gal dispatch-script` pre-renders a `REPORT_LINE` field with the live phase, task, role, executor, model, and effort already substituted in:

```
Dispatched: <phase[ (fix)]> <T-NN> - <ROLE> as <executor>, model <model>, effort <effort>
```

The four supported forms are `implement`/CODER, `implement (fix)`/CODER, `test`/TESTER, and `audit`/AUDITOR. At each dispatch point — implement, test, audit, and every fix-mode redispatch — the orchestrator announces this line to the user **exactly once, verbatim**: copy `REPORT_LINE` as printed, character for character. Do not paraphrase, summarize, reorder its fields, or add adjacent narration about the dispatch itself (no "kicking this off in the background," no "I'll continue while this runs," no invented waiting/notification commentary) — that narration misstates phase sequencing the pipeline does not actually have. This announcement rule governs only the pre-dispatch narration line; it does not change the post-run `Dispatch:` marker recording above or any `## Test Results` / `## Review Results` / `## Status` write-back obligation, all of which are unchanged.

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

1. Receipt file was written by the secondary tool during the current run (the dispatcher held the resolved-path lease from freshness through verification/fetch, and a non-empty link-free regular file now exists at the prepared path)
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
| Local receipt preparation/lease fails, or an external explicit path already exists/is link-mediated | Bin outputs `reason=receipt-preparation-failed`, exit 2; no attempt log and no executor spawn |
| Remote receipt freshness/lease guard fails | Terminal state `disconnected-partial`, exit 1, with `reason=remote-receipt-freshness-failed`; no receipt fetch |
| Local/control-side receipt lease release fails | Terminal state `disconnected-partial`, exit 1, with `reason=receipt-lease-cleanup-failed`; exact lock path remains in stderr evidence |
| Remote receipt fetch fails or exceeds its time/memory bound | Terminal state `no-receipt`, exit 1, with `reason=remote-receipt-fetch-failed`, `remote-receipt-fetch-timeout`, `remote-receipt-fetch-read-failed`, `remote-receipt-fetch-too-large`, or an auxiliary-cleanup `*-unconfirmed` variant |
| Remote owner-token lease cleanup fails or exceeds its bound after any confirmed SSH exit | Terminal state `disconnected-partial`, exit 1, with `reason=remote-receipt-lease-cleanup-failed`, `remote-receipt-lease-cleanup-timeout`, or an auxiliary-cleanup `*-unconfirmed` variant; stale lease remains fail-closed. If fetch also failed, reasons are joined with `+` |
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
| `in-conversation` | No dispatch attempt log exists for this phase at all — the phase happened without any headless dispatch (a phase legitimately handled without dispatch). |
| `dispatch-offload` | The phase's LATEST dispatch attempt terminated `completed`. |
| `recovered-in-conversation` | The phase's LATEST dispatch attempt terminated non-`completed` (a broken or timed-out dispatch), but the task's bound convergence (three-surface agreement + task-commit + cursor-cleared) otherwise still holds — a legitimate recovery, not silently erased failure history. |

An unterminated `started` marker as the latest attempt is not one of these three outcomes — it fails the bound v2 receipt unconditionally (never convergence-gated), because a marker that was never terminally rewritten proves nothing about how the attempt actually ended.

Older non-completed attempts are never deleted or overwritten — every attempt gets its own collision-safe path (see below), so the full attempt history stays on disk as forensic evidence even after a later attempt succeeds or the task recovers. Only the LATEST attempt's terminal state drives a phase's outcome classification.

#### Durable Run Record (Forensics)

Before any availability check or process spawn, the bin allocates a unique attempt path and durably writes a `terminal_state: started` marker there (marker-before-spawn). Marker-write failure prevents spawn. On terminal completion the SAME path is rewritten with the full log content; if that terminal rewrite fails, the file is left holding the `started` marker — intentional evidence of an unterminated attempt, not a bug.

The log directory is plan-scoped whenever the pipeline input is a Prompt or SourcePlan path: `.dev/executor-logs/<plan-slug>/`, derived automatically from the dispatch target's own filename (no CLI flag). Raw/direct dispatch keeps the unscoped default location, `.dev/executor-logs/`.

The bin writes `<log_dir>/<epoch_secs>-<subsec_nanos>-<attempt_seq>-<task_id>-<phase>-<executor>.log` containing:
- Header fields: `timestamp_start`, `timestamp_end`, `duration_ms`, `executor`, `phase`, `task_id`, `git_branch`, `git_head`, `exit_code`, `actual_model`, `terminal_state`, `session_id`
- For pipeline-created dispatches with resolved provenance: sanitized `contract`/`contract_source` lines, identical between the pre-spawn `started` marker and the terminal record — absent entirely (byte-compatible legacy header) when provenance is absent, as with raw/direct dispatch
- Full `---STDOUT---` and `---STDERR---` sections

The `<subsec_nanos>-<attempt_seq>` pair makes every attempt's path collision-safe — two attempts for the same task/phase/executor triggered within the same wall-clock second still land at distinct paths, so rapid retries never overwrite an earlier attempt.

Terminal-state vocabulary:

| State | Meaning |
| --- | --- |
| `started` | Pre-spawn marker written before availability check/spawn; residue when the terminal rewrite never happened |
| `completed` | Exit 0 and write-back verified |
| `no-receipt` | Exit 0 but receipt file absent or empty |
| `timeout` | Killed by process-tree timeout |
| `timeout-no-output` | Killed by process-tree timeout after producing zero output before the kill. Compatibility state token only — observed output alone does not establish a cause |
| `timeout-midrun` | Killed by process-tree timeout after producing some output before the kill. Compatibility state token only — observed output alone does not establish a cause |
| `disconnected-partial` | Non-zero exit; possible partial write |
| `unavailable` | Executor CLI not found in PATH |

These logs are retained for forensics. At pipeline time `gal pipeline-converge-check` scans this same plan-scoped directory, classifies each required phase's LATEST attempt into one of the three outcome literals above, requires the task's bound convergence to hold, and writes a bound v2 receipt recording the result. `/gal finalize`'s check(g) does **not** read that receipt file — under finalize's zero-trust doctrine it re-establishes the same bound convergence in-process and rescans this same plan-scoped directory itself, per checked task. The plan-scoped directory — not a whole-root log scan — is the per-phase evidence of record; the receipt is the pipeline-time record of it, and check(g) re-verifies rather than trusts it.

**Legacy top-level baseline precondition.** A legacy unscoped log (sitting directly in `.dev/executor-logs/` outside any plan-scoped subdirectory) is never automatically attributed to a plan — the check(g) machinery only ever scans a plan's own scoped subdirectory, so a stray top-level log simply never enters evidence for any plan, never a false pass. Adopting plan-scoped logging in an existing repository is a one-time, manually-verified bootstrap step, not an automated gate: the person performing the adoption must confirm the top-level `.dev/executor-logs/` baseline is empty before relying on scoped logging as evidence of record; a non-empty baseline at that point is a hard stop requiring explicit human attribution before proceeding, not something the binary detects or enforces on its own.

#### ⚠️ SECURITY WARNING — bypass-permission

Headless executor adapters invoke secondary CLIs with a permission-bypass flag — `--dangerously-skip-permissions` (claude/agy), `--auto` (opencode), or `--allow-all` (copilot) — which grants the secondary CLI full filesystem and terminal access. A malicious or flawed agent contract could cause unintended file deletions, edits, or arbitrary command execution.

**Enable executor routing only in a trusted local environment.** The `Dispatch:` marker always appears before execution. The orchestrator must surface this warning to the user before first use.

### Runtime Step-Budget Preflight

Provider turn limits and OpenCode agent `steps` limits are hard runtime boundaries. GAL cannot remove them, so the pipeline must avoid treating a provider cutoff as a workflow decision.

Before starting the task loop, note any visible provider or OpenCode step limit only as a possible runtime cutoff. It does not create a tranche, checkpoint, handback, or final-authorization branch. Continue the task loop until its checker-authorized decision or a cutoff occurs.


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

For a marked prompt whose `### Test-First Generations` table has no rows, ORCHESTRATOR runs `gal test-first-transition generation <prompt> <task> <contract-digest> <reason> <old-digest>` once per task with `reason=init`. The generation producer reads and rewrites the prompt, owns the row placement and generation number, rejects `init` when a generation already exists for the task, and inserts each row after the last existing row. ORCHESTRATOR does not hand-compose ledger rows or route them through the `refresh` path in `### Marked-Prompt Status Write-Back`. A markerless prompt is untouched.

If `Current Task` is set in `## Status` and no `from` argument was given, resume from that task. If `### Handoff Notes` contains an `OPEN` `Interrupted Phase` block, or if `Workflow` names a phase whose convergence gate is incomplete, resume that phase first.

Never run Step 2 task execution directly against a source plan when the matching execution prompt exists. `## Status`, retry counters, commit checkpoints, handoff notes, test results, and review results belong in `.dev/plans/<slug>.prompt.md`.

When an execution prompt exists, keep the paired source plan path in memory for closeout. The pipeline owns cross-file state convergence after each task passes all gates:

- `.dev/plans/<slug>.md` — human-readable source plan task checkbox and task commit note
- `.dev/plans/<slug>.prompt.md` — execution status, task checkbox, retry/review/test state, and resume markers
- `.dev/state.md` — active-plan last activity plus the matching per-plan session continuity row

Do not leave this convergence to implementer, tester, auditor, or a later chat. A task is not pipeline-complete until all three surfaces are updated and re-read successfully.

If prerequisites are not met: tell the user what is missing and stop. If the execution prompt is still stubbed, run `/refining-plan` on the source plan and then rerun `/plan-to-prompt` before attempting the pipeline again.

### Pipeline-Preflight Receipt Gate (binary, hard)

**Self-bootstrap first (mandatory when the plan touches `crates/`).** If any of the plan's `## Files to Create or Modify` entries (or task-block affected-file paths for individual tasks) are under `crates/`, **rebuild + reinstall `gal` before running any gate command in the task loop**, wrapping the rebuild and every install target inside an advisory install lease. Running a gate binary against pre-change code produces a receipt that describes the old binary, not the work under implementation — any `pass` from a stale binary is evidence of nothing. The lease covers every install target: canonical root `~/.gal/plugins/gal/`, `~/.cargo/bin/gal.exe`, Claude plugin-cache copy when present. Rebuild and installation sit inside the lease in this order:

1. Create the marker's parent `~/.gal/.locks/` (this step may use `-Force` / `-p`).
2. Acquire the lease by creating `~/.gal/.locks/gal-install/` with the exact commands in the table below and no `-Force` / `-p`.
3. On "already exists" poll every `5 seconds` up to `900 seconds`, then stop and surface the marker path without removing it.
4. Run `cargo build --release` and install to all three targets — canonical root, `~/.cargo/bin/gal.exe`, Claude plugin-cache copy when present.
5. Record the pinned hash while still holding the lease by resolving the executable with the command in the table below and hashing it with SHA-256, storing `gal --version` output beside it as context that never participates in the comparison.
6. Release the marker on both the success and the error path, reporting the exact path if the release itself fails.

A later self-bootstrap repeats this cycle and replaces the pinned hash. With no Rust source touched the orchestrator takes no lease and runs no build, but still resolves and hashes the executable with the commands in the table below before its first gate command, so a pinned hash always exists.

| Step | PowerShell | Bash |
| --- | --- | --- |
| Create the marker's parent (may already exist) | `New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.gal\.locks"` | `mkdir -p "$HOME/.gal/.locks"` |
| Acquire the lease (never `-Force` / `-p`) | `New-Item -ItemType Directory -Path "$env:USERPROFILE\.gal\.locks\gal-install"` | `mkdir "$HOME/.gal/.locks/gal-install"` |
| Resolve the executable | `(Get-Command -CommandType Application gal \| Select-Object -First 1).Source` | `command -v gal` |
| Hash it | `(Get-FileHash -Algorithm SHA256 <path>).Hash` | `sha256sum <path>`, or `shasum -a 256 <path>` on macOS |

Three traps stated as rules, not left to the reader:
- `-Force` and `-p` are banned on the acquire step only. Both return success when the directory exists, turning the lease into a no-op with no error and no signal. They are required on the parent step, which is not the lock.
- `-CommandType Application` is mandatory in the PowerShell resolve. Bare `Get-Command gal` resolves to PowerShell's `gal` alias for `Get-Alias` and returns an empty `Source`, failing the hash step.
- Hash comparison is case-insensitive. `Get-FileHash` returns uppercase hex, `sha256sum` lowercase; same digest for the same file, so normalise before comparing.

Manual recovery for a stale marker left by a crashed run: an operator confirms no other GAL pipeline or finalize run is active, then removes the marker directory by hand. Always a human action. Never instruct an orchestrator to auto-remove, because auto-removal defeats the lease the first time a slow build is mistaken for a crash.

After the manual prerequisite check above, run:

```powershell
gal.exe pipeline-preflight <execution-prompt-path>
```

Read the receipt. Before trusting any gate command's receipt, re-resolve the executable and re-hash it with the commands in the self-bootstrap paragraph above, and compare case-insensitively against the pinned hash recorded during self-bootstrap. On mismatch, stop, do not trust the gate result that produced it, and report both `gal --version` strings to the human as context. **The receipt is the sole pass-basis — self-report is not accepted.**

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

### Runtime Cutoff Recovery

On a runtime turn/step cutoff, write or refresh the existing `Interrupted Phase` resume marker and leave a recovery-only rerun action. Do not self-re-enter the pipeline, schedule a wake-up, infer a human decision, or return final output. Codex has no host-level sampling control or self-wake authority; a later invocation must run preflight and the handback checker again.

---

## Step 2 — Task Loop

Repeat for each unchecked blocking `T-NN` task in `## Tasks` (in order, respecting `from` / `stop-at`). After a task passes all gates, run `pipeline-handback-check` and execute its `continue` action automatically in the same invocation. Do not ask the owner to keep running. Only checker-authorized terminal decisions or an unrecoverable runtime cutoff handoff end this invocation. Before trusting any gate command's receipt, re-resolve the executable and re-hash it with the commands in the self-bootstrap paragraph above, and compare case-insensitively against the pinned hash recorded during self-bootstrap. On mismatch, stop, do not trust the gate result that produced it, and report both `gal --version` strings to the human as context.

### Contract Marker Detection & Branch Selection

At the start of each task iteration, inspect the active execution prompt's pre-heading region (anchored after H1 and before the first `##` heading) for the explicit contract marker:

- **`test-first-v1` Branch**: Active when `Pipeline Contract: test-first-v1` is present in the execution prompt.
  - **Required Tasks (`Test-first: required`)**:
    Phase sequence: **2c Scaffold (conditional, CODER)** → **2d TESTER Expected Red** → **2e CODER Uncommitted Implement** → **2f ORCHESTRATOR Same-Command Green & Correctness Gate** → **2g AUDITOR Dirty-Tree Audit** → **2h ORCHESTRATOR Implementation Commit** → **2i State Convergence & State Commit**.
  - **Not-Applicable Tasks (`Test-first: not-applicable`)**:
    Phase sequence: **2e CODER Uncommitted Implement** → **2f ORCHESTRATOR Correctness Gate** → **2d TESTER Non-Red Probe (pass receipt)** → **2g AUDITOR Dirty-Tree Audit** → **2h ORCHESTRATOR Implementation Commit** → **2i State Convergence & State Commit**.
- **Legacy Branch (`legacy`)**: Active when the execution prompt is markerless (no marker line present).
  Phase sequence: **2e CODER Implement** → **2f ORCHESTRATOR Correctness Gate** → **2d TESTER Test** → **2g AUDITOR Audit** → **2h ORCHESTRATOR Implementation Commit** → **2i State Convergence & State Commit**.

### 2a — Working Hours Check

Pipeline execution is **exempt from working-hours by default** (see `conventions/working-hours.md` → Pipeline Execution Exemption). `/gal pipeline` is machine self-driving, not the user working late, so it does **not** stop at Wrap-up Time or Hard Stop and does **not** offer wrap-up between tasks. (Working-hours bounds interactive/chat work and direct golem calls only.) Neither boundary is a handback decision; proceed.

### 2b — Update Cursor

For a markerless prompt, update plan `## Status` directly:

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

### Marked-Prompt Status Write-Back

For a marked prompt, every pipeline write to `## Status`, `### Handoff Notes`, `### Deviations`, or a retry counter is compose-then-publish. Read the prompt's current bytes and old digest, compose the complete replacement bytes in memory, then publish them through the existing transition producer:

```powershell
gal test-first-transition refresh <prompt> <bytes> <old-digest>
```

The refresh records `kind=phase-rerun` and is the only publication path for these marked-prompt updates. This includes cursor changes, retry increments, handoff and deviation updates, interruption recovery, and task closeout status. Do not edit a marked prompt in place, weaken its digest binding, or introduce a new transition kind, a new journal, or a new command. A markerless prompt keeps the existing direct-write behavior unchanged.

- **Contract-Region Invariant**: After locking and before any journal or prompt write, the marked transition producer compares old/new contract-region digests (Goal, Requirements, task-contract, or Test Plan); contract-region changes require `contract-change`, while unauthorized edits are rejected before write.
- **Trackable Same-Slug Journal Ownership**: Transition journals must be Git-trackable under `.dev/pipeline/journal/<slug>/transition.journal.tsv`. Boundary check exemption is strictly for the running plan's own same-slug state-recording journal (`.dev/pipeline/journal/<slug>/transition.journal.tsv`); locks, backups, receipts, snapshots, or another plan's journal gain no exemption.

### 2c — Scaffold Phase (CODER model — test-first-v1 conditional)

Under `Pipeline Contract: test-first-v1`, when a task specifies `Scaffold: required`:

1. **HEAD-Drift Discipline (scaffold phase):** Capture `git rev-parse HEAD` before dispatch. HEAD must remain unchanged during scaffold construction.
2. Run:
   ```powershell
   gal.exe dispatch-script golem-implementer --pipeline-phase scaffold --task-scope T-NN '#file:<execution-prompt-path>'
   ```
   Always carry the PowerShell-quoted execution prompt path (`'#file:<execution-prompt-path>'`).
3. The implementer constructs only behavior-free seam stubs (type definitions, interface signatures, stubs returning default/unimplemented values) on `Production Paths`. The scaffold must remain strictly behavior-free, containing zero domain or behavioral logic.
4. **Commit Discipline:** No commit is produced and HEAD remains unchanged at `Task Base Commit`. The worktree remains dirty with behavior-free scaffold stubs.

### 2d — Test Phase (TESTER model — independent test author)

Update plan `## Status`: set `Workflow: TEST`

This phase executes under one of three branch conditions:
- **Case 1: `test-first-v1 required` (Expected Red Test Phase — runs before implementation)**
- **Case 2: `test-first-v1 not-applicable` (Non-Red Probe Pass Phase — runs after implementation & correctness gate)**
- **Case 3: `legacy` (Plain Test Phase — runs after implementation & correctness gate)**

**HEAD-Drift Discipline (test phase):** Immediately before running the dispatch below, capture `git rev-parse HEAD`. After the dispatcher returns, before any orchestrator write step, capture `git rev-parse HEAD` again. The tester must not commit; HEAD must be unchanged. Any drift → **STOP immediately**. Record a Deviation in `## Status > ### Deviations` naming the unexpected commit(s). Do not revert automatically. This discipline also applies to every implement fix-mode re-dispatch within this step.

**Post-Phase Boundary Compare (test phase):** When this phase actually dispatched, immediately after the HEAD-drift compare above, the orchestrator re-runs the boundary check as a phase-exit gate: delete its plan-scoped default receipt first, then run `gal boundary-check <execution-prompt-path> --task T-NN --boundary-kind post-test`. This check-command receipt does not pass through executor-dispatch freshness preparation, so its explicit delete remains load-bearing. HEAD-drift and this working-tree compare are the two halves of one phase-exit check. This re-run does **not** apply when 2d took the dispatch-necessity skip path (below), since nothing was dispatched.

**Dispatch-necessity rule.** Before running the dispatch below, resolve `T-NN`'s covering Test Plan rows: every `## Test Plan` row whose `Covers` cell names `T-NN`. A `Covers` cell naming no `T-NN` id (blank, or a value that is not a task id) contributes to no task's covering set. Skip the tester dispatch only when `T-NN` has at least one covering row and every covering row's `Type` is one of the `no-dispatch` values `grep`, `manual`, or `documentation`, matched case-insensitively. This is **fail-closed**: every other case dispatches as today, including an empty covering set (no covering rows at all) and any covering row whose `Type` is unrecognized or misspelled.

This covering-row rule applies to unmarked tasks only: for every marked `test-first-v1` task, the test-first axis governs regardless of the covering-row `Type` (the required branch authors probes and the not-applicable branch runs its locked non-red probe), so the TESTER phase is never skipped; marker activation does not replace or supersede the unmarked fail-closed rule because the two rules partition by marker presence.

On the skip path, do not run `gal dispatch-script`. Instead, the orchestrator runs the covering `grep` rows in-process and itself writes the `### [T-NN] YYYY-MM-DD` subsection under `## Test Results` with the real command output (a task covered only by `manual` or `documentation` rows still gets this subsection, recording that no automated command applies). Verification independence still holds on the skip path because the orchestrator, not the CODER, produces this evidence.

On the dispatch path, proceed as below.

Run:

```powershell
gal.exe dispatch-script golem-tester --pipeline-phase test --task-scope T-NN '#file:<execution-prompt-path>'
```

**Carry the active execution prompt path** (`#file:<execution-prompt-path>`) — it is the OFFLOAD dispatch target; omitting it yields `COMMAND: error`.

The dispatcher must emit `MODE: bound`, `DISPATCH_KIND: pipeline-phase`, `PIPELINE_PHASE: test`, and `TASK_SCOPE: T-NN`.

**Branch Evidence & Duty Requirements:**

- **Case 1 (`test-first-v1 required` expected red test phase):**
  - **Duty & Expected Red Evidence:** TESTER authors acceptance probes targeting the locked public seam in `Test Paths`. Every new acceptance probe must execute and fail for its referenced locked behavioral reason, emitting canonical per-probe expected red evidence (`probe_record_b64` receipt written to `.dev/pipeline/receipts/<plan-slug>/<T-NN>/.../probe-red.receipt.md`). Unrelated existing tests may stay green.
  - **Frozen Paths & Stop-Line:** TESTER freezes `Production Paths` and `Test Paths` at the locked seam. **Hard Stop-Line:** CODER must not add, remove, or modify test items at the locked seam, including when those items live in a file that is also a production path. All implementation work must take place strictly within `Production Paths` without modifying test items. (The snapshot freeze lane is not activated by this flow; the freeze is enforced via agent contract discipline.)
  - **Commit Discipline:** TESTER does not commit. HEAD remains unchanged at `Task Base Commit`.
  - **Probe Invocation:**
    ```powershell
    gal test-first-probe run <plan> <task> <generation> <contract_digest> test red <id> <selector> <argv_b64> <timeout_ms> [--env KEY=VALUE] [--expected-failure TEXT]
    ```
    - `<plan>` = plan slug
    - `<task>` = `T-NN`
    - `<generation>` = the task's current row in `### Test-First Generations`
    - `<contract_digest>` = the value from `gal test-first-probe contract-digest <prompt> <task>`
    - `<phase>`/`<expectation>` = `test`/`red`
    - `<id>` = probe ID from the task contract / test plan
    - `<selector>` = `acceptance`
    - `<argv_b64>` = probe command encoded with `gal test-first-probe encode-argv <arg>...`
    - `<timeout_ms>` = probe timeout in milliseconds
    - `--expected-failure` = expected-failure reference text from the task contract
    - Note: The printed `receipt=<path>` is the exact file `pipeline-converge-check` reads.
- **Case 2 (`test-first-v1 not-applicable` non-red probe pass phase):**
  - TESTER runs the locked non-red probe or spec test after implementation, producing an expectation=pass receipt (`probe-pass.receipt.md`).
  - **Probe Invocation:**
    ```powershell
    gal test-first-probe run <plan> <task> <generation> <contract_digest> test pass <id> <selector> <argv_b64> <timeout_ms> [--env KEY=VALUE]
    ```
    - `<plan>` = plan slug
    - `<task>` = `T-NN`
    - `<generation>` = the task's current row in `### Test-First Generations`
    - `<contract_digest>` = the value from `gal test-first-probe contract-digest <prompt> <task>`
    - `<phase>`/`<expectation>` = `test`/`pass`
    - `<id>` = probe ID from the task contract / test plan
    - `<selector>` = `non-red`
    - `<argv_b64>` = probe command encoded with `gal test-first-probe encode-argv <arg>...`
    - `<timeout_ms>` = probe timeout in milliseconds
    - Note: The printed `receipt=<path>` is the exact file `pipeline-converge-check` reads.
- **Case 3 (`legacy` plain test phase):**
  - TESTER runs spec tests after implementation and writes the complete task-scoped `### [T-NN] YYYY-MM-DD` subsection into the injected receipt file (`<task>-test.receipt.md`). Direct execution-prompt edits are prohibited; the `gal` binary validates and places the subsection into `## Test Results`.

**Check result:**

- **No task-scoped subsection was written (in-scope legacy branch)**: **STOP immediately**. Write `Retry Handoff — T-NN / TEST` with the missing write-back as the problem. Do not infer PASS or FAIL from chat alone.
- **`Workflow: TEST` is set but the latest task-scoped subsection is still missing or placeholder-only (in-scope legacy branch)**: **STOP immediately**. Treat this as incomplete durable state, not as a passing or failing run.
- **A dispatched test phase reports PASS without matching evidence**: **STOP immediately**. This executor-log terminal-state requirement applies to the dispatched path only. For dispatched runs on an in-scope branch (legacy branch), PASS requires executor delivery of the task-scoped receipt payload, binary semantic commit under `## Test Results`, and executor-log terminal state `completed`. For marked branches (`test-first-v1` required / not-applicable), PASS requires executor delivery of the probe receipt (`probe-red.receipt.md` or `probe-pass.receipt.md`) and executor-log terminal state `completed`. Write `Retry Handoff — T-NN / TEST` with the missing evidence as the problem. `DEGRADED_BUNDLED` runs still use reproducible `## Test Results` command output as their evidence and do not require executor logs. Skip-path runs (dispatch skipped under the dispatch-necessity rule above) use that same reproducible `## Test Results` command-output evidence shape as `DEGRADED_BUNDLED` runs — they do not require executor logs either, because no tester was dispatched.
- **Placement defect in the written-back subsection (in-scope legacy branch)**: Before treating the `### [T-NN]` subsection as valid durable evidence after binary placement, verify (a) its nearest enclosing `##` heading is `## Test Results` — not any other section — and (b) no existing Markdown table in the plan file has content inserted between its rows. If either defect is present: **STOP immediately**. Write `Retry Handoff — T-NN / TEST` with the placement defect as the problem. Do not treat a mis-placed or table-corrupting write-back as a passing or failing run.
- **All tests PASS**: update `## Status` `Workflow: AUDIT`, proceed to 2g
- **Any tests FAIL**:
  - Increment `Test Retry Count` in `## Status`
  - Refresh the active `Retry Handoff — T-NN / TEST` block with the latest failing test names, the current `## Test Results` subsection, and the next fix target
  - If `Test Retry Count` < 3: dispatch implementer to fix failing tests with `--pipeline-phase implement --task-scope T-NN --fix '#file:<execution-prompt-path>'` (always carry the PowerShell-quoted prompt-path token), then re-run tester
  - If `Test Retry Count` = 3: **STOP**. Surface failures. Tell user the retry ceiling (3) has been reached for `T-NN`, include attempts 1-3 from the handoff block, and request human intervention

If the tests pass after one or more failed rounds, mark `Retry Handoff — T-NN / TEST` as `RESOLVED` and note the validation run that cleared it.

### Test-First Lifecycle Contract

The lifecycle is a closed sequence owned by deterministic producers and consumers. The orchestrator must execute each boundary in order and stop on any missing, stale, ambiguous, or non-canonical evidence:

1. **Clean task entry:** capture `Task Base Commit`, require a clean worktree and empty index, resolve the marked task's generation and contract digest, and run the boundary gate before dispatch. A dirty tree, outside dirtiness, stale HEAD, staged member, or changed state surface is a stop, not an input to the task.
2. **Boundary and transition:** the boundary owner validates the declared paths and identity immediately before reads; the transition producer alone creates or replaces the prompt, status, generation, dispute, and retry records through crash-safe CAS and a committed journal. Orchestrator prose never repairs or invents a transition record.
3. **Runner and evaluator:** the deterministic runner owns canonical argv, environment, timeout, output, and probe receipts. The single evaluator owns marker selection, generation, phase order, dispute history, red/green identity, and expected-failure matching; `not-run`, spawn failure, timeout, and non-canonical evidence never prove a pass.
4. **Required and not-applicable branches:** a marked required task runs scaffold → expected-red TESTER probe → uncommitted implementation → exact same-command green rerun → dirty-tree audit → implementation commit. A marked not-applicable task runs uncommitted implementation → correctness gate → locked non-red TESTER probe → dirty-tree audit → implementation commit. Both marked branches defer the commit until after AUDITOR, because the deterministic commit gate itself requires an already-written correctness-gate-pass and AUDITOR-pass receipt as its own preconditions. A markerless legacy task keeps its existing Hard Commit Gate order instead: implement → correctness gate → commit → TESTER → audit, unchanged by activating the marker on other tasks.
5. **Dispute recovery:** ORCHESTRATOR alone classifies `probe-defect`, `implementation-defect`, and `contract-ambiguous`. `probe-defect` and `contract-ambiguous` restore the verified pre-test production baseline, increment the generation through the transition producer, and require fresh red evidence; `implementation-defect` retries in the same generation. Unknown or disputed state stops at the plan/refining boundary.
6. **Dirty-tree audit and commit gate:** AUDITOR reviews the uncommitted task diff for marked tasks, and the commit gate verifies the exact dirty-set, cached paths and hashes, base/parent/range invariants, receipt digests, and post-commit cleanliness before publishing the implementation commit. The auditor never commits and the orchestrator never commits before its receipt gates pass.
7. **Recovery seams:** every journal, candidate, backup, replacement, runner, evaluator, audit, and commit boundary has a bounded crash/timeout stop. A prepared, ambiguous, unknown, rollback-unconfirmed, or missing journal is non-pass; rollback outcomes are recorded and never silently converted to success. These are the canonical recovery outcomes, not best-effort narrative.
8. **Markerless legacy prompt semantics and later tamper stop:** a markerless prompt carries no transition journal and performs no digest binding. Readers retain support for historical `legacy-bootstrap` journal rows, but writable transitions reject creating new `legacy-bootstrap` rows. Later prompt tampering, including a prompt/status mutation not produced by the transition producer on a marked prompt, triggers the later prompt-tamper stop at the boundary.
9. **Markerless transition boundary:** for a markerless prompt, prompt updates continue as direct writes without journal binding. If a prompt is subsequently marked, all subsequent prompt/status mutations must go through the transition producer.

### 2e — Implement (CODER model — test-first-v1 & legacy branches)

**HEAD-Drift Discipline (implement phase):** Immediately before running the dispatch below, capture `git rev-parse HEAD`. After the executor returns, HEAD must be unchanged: the executor returns an uncommitted task-scoped diff. The orchestrator then runs correctness gate, audit, and boundary check; only after all pass does the orchestrator create the task commit, record `Task Final Commit`, and confirm a clean worktree. Any pre-return HEAD drift → **STOP immediately** and record a Deviation.

**Post-Phase Boundary Compare (implement phase):** Because this phase actually dispatched, immediately after the HEAD-drift compare above, the orchestrator re-runs the boundary check as a phase-exit gate: delete its plan-scoped default receipt at `.dev/pipeline/receipts/<plan-scope-key>/T-NN-boundary-check.receipt.md` first, then run `gal boundary-check <execution-prompt-path> --task T-NN --boundary-kind implementation-commit`. This explicit delete remains required because check-command receipts are written directly by the check binary and do not pass through executor-dispatch freshness preparation. HEAD-drift and this working-tree compare are the two halves of one phase-exit check — HEAD-drift confirms the commit provenance, the boundary compare confirms the changed files stayed inside the task's allowlist.

Run:

```powershell
gal.exe dispatch-script golem-implementer --pipeline-phase implement --task-scope T-NN '#file:<execution-prompt-path>'
```

**Always carry the active execution prompt path** (`#file:<execution-prompt-path>`, e.g. `#file:.dev/plans/<slug>.prompt.md`). The OFFLOAD dispatch target is that path — `gal pipeline` materializes the per-(task,phase) spec from it. Omitting the token makes the dispatcher emit `COMMAND: error` (it never falls back to an unmaterialized generated-spec path).

The dispatcher must emit `MODE: bound`, `DISPATCH_KIND: pipeline-phase`, `PIPELINE_PHASE: implement`, and `TASK_SCOPE: T-NN`. The implementer must:

1. Record `Task Base Commit` in `## Status` before any changes
2. Set `Current Task: T-NN` in `## Status` before reporting any implementation progress
3. Implement only the work required by `T-NN`
4. Return the uncommitted task-scoped diff without recording `Task Final Commit` or requiring a clean worktree
5. In pipeline fix mode, update the active `Retry Handoff` block in `### Handoff Notes` with the attempted remediation and validation result

**A dispatched implement phase reports PASS without matching evidence**: **STOP immediately**. 2e requires executor-log terminal state completed, and any other terminal state is a STOP that writes or refreshes a `Retry Handoff — T-NN / IMPLEMENT` block naming the missing write-back.

**Hard Commit Gate:** The orchestrator, not the executor, runs boundary and correctness gates, creates the task commit, records `Task Final Commit`, confirms `git status` is clean, and only then advances to test. If any required gate fails, do not commit or advance.

**Pre-commit allowlist diff guard (hard boundary):** Before creating the implementation commit, run `git diff --name-only` (plus `git status --porcelain` for new/untracked files) and compare every changed path against the task's allowlist — the affected-files set named in the task (task-block backtick paths, falling back to `## Files to Create or Modify`; the same allowlist the task spec emits). If any changed path is **outside** the allowlist, **STOP immediately**: do not commit. Treat the out-of-allowlist change as a boundary violation — revert it (`git checkout -- <path>` / remove the stray new file) or, if it is genuinely required, return to the task spec and widen the allowlist explicitly before re-running. Never silently accept an out-of-scope edit. (This is the hard counterpart to the task spec's soft allowlist directive; a dispatched executor that strayed beyond its files is caught here, at the orchestrator-owned commit boundary, before anything lands.)

**Boundary-Check Receipt Gate (binary, hard — runs before the implementation commit):**

```powershell
gal.exe boundary-check <execution-prompt-path> --task T-NN --boundary-kind implementation-commit
```

Read the receipt. **The receipt is the sole pass-basis — self-report is not accepted.**

- **`pass`** — all changed files are within the task's affected-files allowlist; the orchestrator may create the implementation commit.
- **`fail`** — out-of-allowlist files detected. If the **only** out-of-allowlist change is a visibility-only edit reusing already-landed prior-task logic (confirmed by reading the diff — the `Task Base Commit..HEAD` range diff in dispatched mode), follow the **Boundary Widening Protocol** below. All other cases: **STOP immediately before committing.** Revert the stray changes or widen the allowlist in the task spec, then re-run the implementer for T-NN. Write or refresh a `Retry Handoff — T-NN / IMPLEMENT` block naming the violation paths from the receipt.
- **`not-run`** — the task names no affected files at all (no backtick paths in the task block and no `## Files to Create or Modify` entries), so there is no allowlist to check against. **STOP immediately.** Name the task's affected files (backtick paths in the task line, or a `## Files to Create or Modify` section) before re-running. `not-run` is never a pass; it is treated as fail. A standard-template prompt already carries this — no manual `Affected:` clause is required.

Do not create the implementation commit until this receipt reads `pass`.

**Dispatched-mode note:** The `gal boundary-check <execution-prompt-path> --task T-NN --boundary-kind implementation-commit` receipt covers only residual uncommitted changes in the working tree. After the implementer's contract-mandated commit, `git diff HEAD` is empty, so the working-tree check passes vacuously for committed changes. In dispatched mode, the full boundary compare is carried by `git diff --name-only <Task Base Commit>..HEAD` against the allowlist; confirm this range shows only allowlisted paths before accepting the gate pass.

#### Boundary Widening Protocol (implement phase only)

**Scope:** This protocol applies to the implement phase (2e) only. Test and audit have no legitimate widening story — a test or audit dispatch has no business touching files outside the task's allowlist for any reason, visibility-only or otherwise, so a boundary-check failure in 2d or 2g is always a hard violation with no widening path.

**Precondition (strict):** Use this protocol **only** when the boundary compare fails because the **sole** out-of-allowlist change is a visibility-only edit (for example, widening a symbol's visibility with no logic change) to a symbol that was already fully implemented and committed in a prior task — no new logic, no new behavior. Confirm by reading the diff: in dispatched mode, read the `Task Base Commit..HEAD` range diff. Anything else stays a **hard violation** — do not use this protocol to normalize scope creep.

1. **Widen the allowlist** — add the out-of-allowlist file to the task's affected-files list in the execution prompt with a one-line justification naming the reused symbol and the prior task that landed it.
2. **Commit the widening** — commit that allowlist edit alone as a doc-only commit (e.g., `docs(T-NN): widen allowlist — reuse <symbol> from T-MM`). Do not bundle it with implementation changes.
3. **Record a Deviation** — append a row to `## Status > ### Deviations` naming the reused symbol, the file, and the prior task.
4. **Re-run the boundary compare** — bundled mode: `gal boundary-check <execution-prompt-path> --task T-NN --boundary-kind implementation-commit`; dispatched mode: `git diff --name-only <Task Base Commit>..HEAD` against the now-widened allowlist. The gate must read `pass` before the implementation commit proceeds.

The implementation commit created for `T-NN` must stay scoped to the implementation itself. Do not use the implementation commit to record source-plan, execution-prompt, or `.dev/state.md` completion state for the task. Cross-surface progress or completion state belongs to the convergence step after all gates pass.

### 2f — Same-Command Green Rerun & Orchestrator Correctness Gate

Under `Pipeline Contract: test-first-v1`, after implementation and before audit, ORCHESTRATOR reruns the same test command and probe identity to verify the red-to-green transition (same-command green probe rerun), emitting canonical pass evidence (`probe-green.receipt.md`). The compared identity is `id` / `selector` / `argv` / expected-failure ref / `timeout_ms`. The ambient process environment is **not** compared across the two receipts: TESTER's red is authored in a dispatched subprocess and this green rerun is authored in the orchestrator's own shell, so their `env_digest` values legitimately differ. Each receipt still self-verifies its own `probe_set_digest`, which does include `env_digest`.

```powershell
gal test-first-probe run <plan> <task> <generation> <contract_digest> green-rerun green <id> <selector> <argv_b64> <timeout_ms> [--env KEY=VALUE] [--expected-failure TEXT]
```

- `<plan>` = plan slug
- `<task>` = `T-NN`
- `<generation>` = the task's current row in `### Test-First Generations`
- `<contract_digest>` = the value from `gal test-first-probe contract-digest <prompt> <task>`
- `<phase>`/`<expectation>` = `green-rerun`/`green`
- `<id>` = probe ID from the task contract / test plan
- `<selector>` = `acceptance`
- `<argv_b64>` = probe command encoded with `gal test-first-probe encode-argv <arg>...`
- `<timeout_ms>` = probe timeout in milliseconds
- `--expected-failure` = same expected-failure reference used in the paired red probe (green-rerun must reuse red's `id` / `selector` / `argv` / expected-failure ref / `timeout_ms`; the ambient environment is not part of that comparison)
- Note: The printed `receipt=<path>` is the exact file `pipeline-converge-check` reads.

Before audit or commit, the pipeline itself performs a full-context correctness gate. This gate is **not dispatched**.

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
- **Correctness gate passes**: for a marked task, write a plan-scoped per-task correctness receipt through `resolve_receipt_path(Some(<execution-prompt-path>), "<task-id>-correctness.receipt.md")` containing the exact line `overall: pass`; then update `## Status` `Workflow: TEST` (or `Workflow: AUDIT` if Expected Red 2d already ran), and proceed to 2d (for legacy or test-first-v1 not-applicable) or 2g (for test-first-v1 required expected-red). A failing correctness gate writes no correctness receipt.

### 2g — Audit (AUDITOR model — independent audit)

**HEAD-Drift Discipline (audit phase):** Immediately before running the dispatch below, capture `git rev-parse HEAD`. After the dispatcher returns, before any orchestrator write step, capture `git rev-parse HEAD` again. The auditor must not commit; HEAD must be unchanged. Any drift → **STOP immediately**. Record a Deviation in `## Status > ### Deviations` naming the unexpected commit(s). Do not revert automatically. This discipline also applies to every implement fix-mode re-dispatch within this step.

**Post-Phase Boundary Compare (audit phase):** Because this phase actually dispatched, immediately after the HEAD-drift compare above, the orchestrator re-runs the boundary check as a phase-exit gate: delete its plan-scoped default receipt first, then run `gal boundary-check <execution-prompt-path> --task T-NN --boundary-kind post-audit`. This check-command receipt does not pass through executor-dispatch freshness preparation, so its explicit delete remains load-bearing. HEAD-drift and this working-tree compare are the two halves of one phase-exit check.

Run:

```powershell
gal.exe dispatch-script golem-auditor --pipeline-phase audit --task-scope T-NN '#file:<execution-prompt-path>'
```

**Carry the active execution prompt path** (`#file:<execution-prompt-path>`) — it is the OFFLOAD dispatch target; omitting it yields `COMMAND: error`.

The dispatcher must emit `MODE: bound`, `DISPATCH_KIND: pipeline-phase`, `PIPELINE_PHASE: audit`, and `TASK_SCOPE: T-NN`. Under `test-first-v1`, AUDITOR audits the uncommitted dirty worktree before commit (dirty-tree audit). Under `legacy`, AUDITOR audits commit range `Task Base Commit..Task Final Commit`. For all dispatches (in-scope for every prompt), AUDITOR writes the complete task-scoped `### [T-NN] YYYY-MM-DD` subsection into the injected receipt file (`<task>-audit.receipt.md`); direct execution-prompt edits are prohibited, and the `gal` binary validates and places the subsection into `## Review Results`.

Check result:

- **No task-scoped subsection or verdict was written**: **STOP immediately**. Write `Retry Handoff — T-NN / AUDIT` with the missing write-back as the problem. Do not infer approval or block from chat alone.
- **`Workflow: AUDIT` is set but the latest task-scoped subsection still has no verdict**: **STOP immediately**. Treat this as incomplete durable state, not as approval.
- **A dispatched audit phase reports APPROVE without matching evidence**: **STOP immediately**. For dispatched runs (in-scope for all prompts), approval requires executor delivery of the task-scoped receipt payload, binary semantic commit under `## Review Results`, and executor-log terminal state `completed`. Write `Retry Handoff — T-NN / AUDIT` with the missing evidence as the problem. `DEGRADED_BUNDLED` runs still rely on task-scoped `## Review Results` write-back instead of executor logs.
- **Placement defect in the written-back subsection**: Before treating the `### [T-NN]` subsection as valid durable evidence after binary placement, verify (a) its nearest enclosing `##` heading is `## Review Results` — not any other section — and (b) no existing Markdown table in the plan file has content inserted between its rows. If either defect is present: **STOP immediately**. Write `Retry Handoff — T-NN / AUDIT` with the placement defect as the problem. Do not treat a mis-placed or table-corrupting write-back as valid approval or block evidence.
- After write-back validation, for a marked task, write a plan-scoped per-task auditor receipt through `resolve_receipt_path(Some(<execution-prompt-path>), "<task-id>-auditor.receipt.md")` carrying the exact whole-line verdict marker recorded for that task, including `<!-- AUDIT_REVIEW: CLEAR -->` for an approval. A non-`APPROVE` verdict writes no pass marker.
- **APPROVE (no BLOCKING)**: proceed to 2h (Implementation Commit).
- **REQUEST_CHANGES or BLOCK (BLOCKING findings)**:
  - Increment `Review Retry Count` in `## Status`
  - Refresh the active `Retry Handoff — T-NN / AUDIT` block with the latest open BLOCKING findings, current review subsection, and the next fix target
  - If `Review Retry Count` < 3: dispatch implementer to fix BLOCKING issues with `--pipeline-phase implement --task-scope T-NN --fix '#file:<execution-prompt-path>'` (always carry the prompt-path token), update `Task Final Commit`, then re-run auditor
  - If `Review Retry Count` = 3: **STOP**. Surface BLOCKING findings. Tell user the retry ceiling (3) has been reached for `T-NN`, include attempts 1-3 from the handoff block, and request human intervention

**Security / Protected Path escalation:** If any BLOCKING finding is a security vulnerability or Protected Path violation, **STOP immediately** regardless of retry count. Do not attempt an automated fix. Surface to human.

**Severity STOP rule:** If any high or critical audit findings remain open, **STOP immediately**. Preserve the audit STOP semantics even when the general retry path might otherwise continue.

If the audit passes after one or more failed rounds, mark `Retry Handoff — T-NN / AUDIT` as `RESOLVED` and note the auditor pass that cleared it.

### 2h — Implementation Commit Boundary (ORCHESTRATOR inline)

After AUDITOR APPROVE pass, ORCHESTRATOR creates the deterministic implementation commit at HEAD:

1. **Pre-commit allowlist diff guard (hard boundary):** Run `git diff --name-only` (plus `git status --porcelain` for new files) and compare every changed path against `T-NN`'s allowlist. If any changed path is outside the allowlist, **STOP immediately**. Revert stray changes or follow Boundary Widening Protocol.
2. **Boundary-Check Receipt Gate (binary, hard):**
   ```powershell
   gal.exe boundary-check <execution-prompt-path> --task T-NN --boundary-kind implementation-commit
   ```
   Read the receipt. Must read `pass` before creating commit.
   - `pass` → proceed to commit.
   - `fail` → out-of-allowlist files detected. Follow Boundary Widening Protocol if applicable, else STOP.
   - `not-run` → no affected files declared. STOP.
3. **Implementation Commit Creation:**
   - For a task classified `Test-first: required`, run:
     ```powershell
     gal.exe test-first-commit run `
       --plan <execution-prompt-path> `
       --task T-NN `
       --base <Task Base Commit> `
       --production <locked Production Paths> `
       --test <locked Test Paths> `
       --correctness-receipt <plan-scoped per-task correctness receipt from 2f> `
       --auditor-receipt <plan-scoped per-task auditor receipt from 2g> `
       --receipt <plan-scoped implementation receipt>
     ```
     The command creates the implementation commit, captures the commit hash, records `Task Final Commit: <hash>` in `## Status`, and verifies `git status` is clean.
   - For a task classified `Test-first: not-applicable`, and for a legacy task, ORCHESTRATOR creates the single implementation commit for `T-NN` at HEAD, captures the commit hash, records `Task Final Commit: <hash>` in `## Status`, and verifies `git status` is clean.

### 2i — Mark Task Complete And Converge State

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

After all three surfaces are updated and re-read, run the state-recording boundary check:

```powershell
gal.exe boundary-check <execution-prompt-path> --task T-NN --boundary-kind state-recording
```

Read the receipt. It must read `pass` before running:

```powershell
gal.exe pipeline-converge-check <execution-prompt-path> --task T-NN
```

Read the receipt. **The receipt is the sole pass-basis — self-report is not accepted.**

- **`pass`** — source plan and prompt agree; T-NN is `[x]` on both surfaces; commit hash exists; cursor is cleared. The task is pipeline-complete.
- **`fail` or `not-run`** — **STOP immediately**. Write or refresh an `Interrupted Phase — T-NN / VERIFY` block in `### Handoff Notes` with the receipt path and the specific failing check(s). Do not advance to the next task. Repair the noted surface divergence, then re-run the binary until receipt reads `pass`.

`not-run` (e.g., missing commit hash on the task line) is non-zero and is treated as fail. The prose convergence self-check above is still required; the binary adds a machine-verifiable receipt that cannot be self-reported.

1. If `stop-at T-NN` was specified and this task matches: **STOP**. Report task complete and prompt user before starting the next task.

1. Run `gal.exe pipeline-handback-check <execution-prompt-path> [--stop-at T-NN]` now. Read the fresh receipt. On `decision: continue`, execute its one literal closed `next_action` immediately. A next-task action returns to 2a; `run-goal-backward-verification` continues to Step 3 in this same invocation; a stop-at repair action repairs the named state before rerunning the checker. Do not infer continuation or final authority from chat, task prose, or an interruption marker. On `human-required`, `retry-ceiling`, or a proven `stop-at`, follow the receipt's terminal action and preserve the required handback. A `ready-to-finalize` result is only terminal authority after all tasks are converged and the bound goal record is fresh.

---

## Step 3 — Orchestrator Goal-Backward Verification

After all unchecked tasks are complete, `pipeline-handback-check` returns the closed action `run-goal-backward-verification`. The **ORCHESTRATOR** consumes it in the same invocation and runs a plan-level goal-backward verification pass using the inlined spec below. Before any final response, write the goal record through `resolve_receipt_path(Some(<execution-prompt-path>), "goal-verification.receipt.md")`, then run `pipeline-handback-check` again. The checker does not execute this semantic pass; its fresh handback receipt, not this section's verdict alone, authorizes final output.

On the ordinary marked-lane path, after the last task converges and before writing the goal record, run `gal pipeline-preflight --terminal-reverify <execution-prompt-path>`, hash the resulting `terminal-reverify.receipt.md` with SHA-256, and record that digest as `terminal_receipt_sha256` in the goal record. This receipt binds `overall: pass`, `prompt_path`, `prompt_sha256`, and `head`, so it must be produced at this point. This is separate from the `## Terminal-Reverify Entry Branch` recovery lane, which uses the same command when stale goal binding is detected.

The goal record has one closed schema: matching `prompt_path`, `prompt_sha256`, `head`, `checked_tasks`, `terminal_receipt_sha256`, and `verdict: VERIFIED`; one or more continuous non-empty `must_have_1` through `must_have_N` fields starting at 1 with no gap or duplicate; and at least one non-empty `command`. A generic `evidence` field is advisory and never satisfies a must-have. `Current Task` is not copied into this record: the checker reads the canonical prompt directly and requires its cursor to be cleared. `Workflow` is advisory prompt metadata and is neither a goal-record field nor a goal-record binding.

**Verification independence (policy):** goal-backward verification is **ORCHESTRATOR-owned and always runs in-process** — it is never dispatched to another model and there is no `verify` dispatch phase or `VERIFY` routing key. The cross-model independence guardrail is provided by **AUDITOR** (the independent ≠coder per-task deep audit) when the resolved routes differ; when routes match, the audit is weaker because it shares the implementer model's blind spots, not by a separate verify dispatch.

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

- **VERIFIED**: write the bound goal record, run `pipeline-handback-check`, and proceed to Step 4 only when it returns `ready-to-finalize`.
- **GAPS_FOUND**: write exactly one `Human Handback — goal-gaps-blocked` block before hashing the prompt or writing the goal record; run the checker and return only its authorized `human-required` handback.
- **BLOCKED**: write the typed human handback, run the checker, and follow its receipt; never convert a model-authored verdict into final authority.

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

**Autonomy waives no human-required stop.** Default autonomous continuation removes only the working-hours stop and manual keep-running ambiguity; every checker-authorized human-required condition below still halts the pipeline. The human-required classes are **BLOCKING security / Protected Path**, **retry ceiling (3)**, **goal-backward verify GAPS_FOUND/BLOCKED**, **head drift**, **boundary scope decision**, and **three-surface convergence failure**. Runtime cutoff and ordinary task progress are recovery/continuation states, not handback authority.

| Condition | Action |
| --- | --- |
| BLOCKING security vuln or Protected Path | STOP immediately — human required |
| Conditional security audit leaves high or critical findings open | STOP immediately — human required |
| `Test Retry Count` reaches 3 | STOP before 4th attempt — human required |
| `Review Retry Count` reaches 3 | STOP before 4th attempt — human required |
| Goal-backward verify returns GAPS_FOUND or BLOCKED | STOP — surface gaps, human required |
| Three-surface convergence gate fails (2i) | STOP immediately — repair convergence before advancing |
| HEAD-drift detected after any dispatch returns | STOP immediately — record Deviation naming unexpected commit(s); no automatic revert |
| Post-phase boundary compare (2c/2e/2d/2g) reads `fail` or `not-run` | STOP immediately — revert or widen per phase's protocol (implement only), then re-run the phase |
| `stop-at T-NN` reached and re-proven by the checker | STOP — return the receipt-backed terminal decision |
| Runtime step or turn limit interrupts a phase | Write/refresh `Interrupted Phase — T-NN / PHASE`; leave a recovery-only resume marker. Codex does not self-wake and the marker cannot authorize final |
| Fresh handback receipt returns `ready-to-finalize` | Natural completion — READY TO FINALIZE |
