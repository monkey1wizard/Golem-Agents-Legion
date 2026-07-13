---
name: gal
description: "GAL workflow control plane ($gal / /gal). Triggers: init (初始化), status (狀態/現況), whats-next (下一步), wrap-up (暫停/交接/session pause), research, deep-research, or any golem name. Named Workflow Obedience: when user invokes any GAL workflow or repo-work intent, load the matching $<skill> SKILL.md first — do NOT switch to generic coding."
---

# /gal

GAL control-plane entry point. Route based on the subcommand provided.

## Runtime Invocation Note

- In Copilot and Gemini command surfaces, this skill appears conceptually as `/gal`.
- In Codex CLI, custom skills are invoked via `/skills` or `$gal`, not `/gal`.
- For Codex explicit invocation, phrase the request like `$gal status` or `$gal init`.

## Command Routing

| Subcommand | What It Answers | Action |
| --- | --- | --- |
| `init` | How do I bootstrap this repo? | Run `gal init-repo [args]` in the target repo root |
| `status` | Where are we right now? | Follow the `/gal-status` procedure — do not run the script |
| `whats-next` | What do I do next? | Follow the `/gal-whats-next` procedure — do not run the script |
| `wrap-up` | How do I close this session cleanly? | Follow the `/gal-wrap-up` procedure — do not run the script |
| `finalize` | How do I land and close a completed plan? | Follow the `/gal-finalize` procedure — do not run the script |
| `research` | I need structured investigation | Run `gal dispatch-script research [args]` — follow output block |
| `deep-research` | I need multi-source investigation with cross-review | Run `gal dispatch-script deep-research [args]` — follow output block |
| `pipeline` | Task-driven autopilot: iterate T-NN tasks with implement → commit → test → review per task, final goal-backward verify pass; stop only on human-required blockers, retry ceiling, working-hours boundary, or `stop-at` | Run `gal dispatch-script pipeline` to **load and follow the `/gal-pipeline` procedure** — this selects the procedure, **not** immediate task/phase execution. Follow the output block: phase execution begins only after the Entry-Latch `gal pipeline-preflight` receipt passes and each phase is dispatched via `gal dispatch-script … '#file:<prompt>'`. |
| `git-commits` | Generate a scoped commit message (or execute the commit) following this repo's Conventional Commit format | Load and follow the installed `git-commits` skill procedure — do not run a script |
| `git-commit-msg` | Alias for `git-commits` | Load and follow the installed `git-commits` skill procedure — do not run a script |
| `<golem-name>` | Consult a golem (isolated — delegates to native subagent; response labeled `[<role> · isolated]`) | Run `gal dispatch-script <golem-name> [args]` — follow output block |
| `discuss <golem-name>` | Consult a golem in-context — loads activation-core into main conversation, hot-joins from prior transcript, multi-turn until topic change (response labeled `[<role> · in-context]`) | Run `gal dispatch-script discuss <golem-name> [args]` — follow output block |
| *(no args)* | Auto-detect and recommend | Locate the nearest ancestor repo root containing `.dev/state.md`, then follow the `/gal-whats-next` procedure |

## Remote Execution (SSH Dispatch Lane)

Cross-machine execution is not a separate `/gal` subcommand or activation phrase. A `config.json#executorRouting` role entry that carries `sshTarget` + `remoteWorkdir` routes that phase's dispatch over SSH transparently — `/gal pipeline` and golem dispatch behave identically whether the resolved route is local or remote. See `docs/remote-execution.md` for the config keys, remote prerequisites, and safety boundaries (destructive dedicated-checkout cleanup).

## Natural Language Pipeline Trigger

If the user's message contains any of the following intents, treat it as `/gal pipeline`:

- "start implementation"
- "implement and test"
- "implement and review"
- "run the pipeline"
- "auto implement"
- "開始實作"
- "開始實作並自動執行"
- "自動執行 review 和 test"

Treating a message as `/gal pipeline` means **loading and following the `/gal-pipeline` procedure**, not executing an implementation phase directly. The generic `gal dispatch-script pipeline` output block is procedure selection; actual phase execution (implement / test / audit) begins only after the Entry-Latch preflight receipt passes and each phase is dispatched via a phase-scoped `gal dispatch-script … '#file:<prompt>'`. Never skip the procedure and jump straight to editing code.

## Invoke (for script-dispatched subcommands)

Before invoking a dispatcher, choose the target project root as follows:

- For `init`, treat the current working directory as the target project root unless the user explicitly provided another target path. Fresh repos often do not have `.dev/state.md` yet.
- For other script-dispatched subcommands, walk upward from the current working directory or provided `#file:` path until `.dev/state.md` is found.

Keep the terminal current directory at that target project root so dispatcher state reads and plan paths resolve against the project being worked on.

**Windows:**

1. For `init`, run `gal init-repo [args]`.
2. For `status`, `whats-next`, `wrap-up`, and `finalize`, follow the installed skill procedures directly.
3. For `research`, `deep-research`, `pipeline`, or golem dispatch, run `gal dispatch-script [args]`. Use the `gal` binary on PATH, falling back to the GAL runtime checkout's binary while staying in the target project root.

**macOS / Linux:**

1. For `init`, run `gal init-repo [args]`.
2. For `status`, `whats-next`, `wrap-up`, and `finalize`, follow the installed skill procedures directly.
3. For `research`, `deep-research`, `pipeline`, or golem dispatch, run `gal dispatch-script [args]`. Use the `gal` binary on PATH, falling back to the GAL runtime checkout's binary while staying in the target project root.

The fallback runtime path is expected for two cases:

- fresh repos being bootstrapped with `init`, before local `scripts/` or `.dev/state.md` exist
- initialized plan-only projects that have `.dev/state.md` but do not contain GAL's `scripts/` directory

## Follow the Output

The script outputs a `--- GAL DISPATCH ---` block. Act on it exactly — no inference, no reinterpretation.

| Field | Meaning |
| --- | --- |
| `COMMAND` | Execute this workflow action: `init` / `error` / `suggest` |
| `ROLE` | Adopt this golem. Mutually exclusive with `COMMAND`. |
| `MODE` | `bound` = act with explicit execution authority · `consult` = advise only · `utility` = no restrictions |
| `CONSULT_MODE` | Optional. `in-context` = load activation-core into main conversation (hot-join from prior transcript; multi-turn until topic change). Absent = `isolated` (default: delegate to native subagent, only verdict/summary returns to main context). |
| `DISPATCH_KIND` | Optional orchestration context. `pipeline-phase` means `/gal pipeline` owns this dispatch rather than a direct user consult. |
| `PIPELINE_PHASE` | Optional pipeline phase marker: `implement` / `test` / `review` / `verify` / `security`. |
| `TASK_SCOPE` | Optional current task reference for pipeline-bound execution. |
| `FIX_MODE` | Optional retry hint. `true` means this pipeline-bound dispatch is a remediation round. |
| `READ` | Read this file before acting (may appear multiple times) |
| `PLAN` | Optional explicit plan file path for workflows that support file override. When present, prefer this plan over `.dev/state.md` active-plan lookup. |
| `FROM` | Optional lower execution bound for pipeline-style task iteration. |
| `STOP_AT` | Optional upper execution bound for pipeline-style task iteration. |
| `ACTION` | The specific instruction to execute |
| `ON_COMPLETE` | What to do after finishing |

## Consult Dual-Mode Contract

Consult roles (architect, analyst, designer, debugger, steward) can be invoked in two modes:

| Mode | Trigger | What happens | Response label |
| --- | --- | --- | --- |
| **Isolated** (default) | `/gal <role>` | Delegates to the runtime's native subagent; only the verdict/summary returns to main context — charter and role reasoning stay out of main context | `[<role> · isolated]` |
| **In-context** | `/gal discuss <role>` | Loads the golem's activation-core into main context; hot-joins from any prior isolated verdict in the transcript; multi-turn conversation until the topic changes; the full active-core reasoning runs in-thread | `[<role> · in-context]` |

Hot-join: when a prior isolated verdict for the same role is already in the transcript, in-context mode continues from that result instead of starting cold — the isolated verdict becomes the prior context for the in-context session.

**Orchestrated-only roles** (implementer, tester, auditor, researcher) cannot be invoked in either mode directly. They are only reachable via `/gal pipeline` (or `/gal research`). Bare `/gal auditor` or `/gal discuss auditor` → unknown-intent.

## Non-Script Procedures

For `status`, `whats-next`, `wrap-up`, and `finalize`, do not run the script. Instead, load and follow the corresponding installed skill:

- `status` → load the installed `gal-status` skill procedure
- `whats-next` → load the installed `gal-whats-next` skill procedure
- `wrap-up` → load the installed `gal-wrap-up` skill procedure
- `finalize` → load the installed `gal-finalize` skill procedure
- `git-commits` / `git-commit-msg` → load the installed `git-commits` skill procedure

Treat those delegated skill procedures as the single source of truth for substantive control-plane behavior.
