---
name: gal-whats-next
description: "GAL whats-next ($gal-whats-next / /gal whats-next / 下一步 / what should I do next / next action). Reads .dev/state.md + active .prompt.md, then recommends the single next command ($gal-pipeline / $gal-finalize / $refining-plan / etc.) with the minimum required context."
---

# /gal whats-next

Determine what to do right now based on the current recorded GAL state.

## Step 1 — Collect State

Starting from the current working directory or opened workspace folder, walk upward to the nearest ancestor directory that contains `.dev/state.md`. Treat that ancestor as the repo root and read `.dev/state.md` there.

- If no ancestor directory contains `.dev/state.md`, output **Repo not initialized — run `/gal init`.**
- If `.dev/state.md` exists but there is no active plan entry under `## Active Plans`, output **No active plan. Use `/planning` to start sprint planning.**
- Resolve every row in `## Active Plans`. Table order is the default priority order, but do not collapse to the first non-terminal row when current recorded state does not actually disambiguate priority.
- First collect the non-terminal active rows (`Complete`, `Done`, `Verified`, and `Closed` are terminal). Then determine whether one row has a stronger execution signal than the others. Stronger signals are: an explicit blocker, an OPEN `Retry Handoff` or OPEN `Interrupted Phase` block, a current task already set, failing tests, BLOCKING review findings, task checkbox mismatch between source and prompt, or incomplete durable write-back for an active workflow phase.
- If exactly one non-terminal row has the strongest signal, treat it as the **target active plan**.
- If multiple non-terminal rows remain in the same coarse lifecycle state and none has a stronger signal than the others, treat them as an **ambiguous active set**. In that case, read each candidate plan's execution file and paired source plan as needed, and do not silently select only the first row.
- When a row points to `.dev/plans/<slug>.md`, prefer `.dev/plans/<slug>.prompt.md` when it exists, but also keep the source plan path for source/prompt task-sync checks. If the row points directly to `.dev/plans/<slug>.prompt.md`, also resolve the paired `.dev/plans/<slug>.md` when it exists.
- If the active plan file is missing, output the exact repo-state error and suggest inspecting `.dev/state.md` plus the referenced active plan file.

From `.dev/state.md` and the resolved target plan or ambiguous active-set plan files, extract these data points:

1. `.dev/state.md` — active plans table, blockers, full session continuity table, and the continuity row that matches the target plan's paired source plan path
2. Active plan `## Status` — plan phase marker if present, current step, next step, current task, task base commit, task final commit
3. Active plan `## Review Results` — any BLOCKING findings
4. Active plan `## Test Results` — pass / fail / pending
5. Active plan `### Handoff Notes` — interrupted work context and OPEN retry handoffs
6. Active plan `## Open Questions` — count of unresolved OQ-NN items
7. Active plan `## Tasks` — blocking task completion state
8. Paired source plan `## Tasks` — compare checkbox state against the execution prompt when both files exist
9. Active plan `## Deferred Follow-up` — advisory non-blocking work when present
10. Active plan `## Analyze` — CLEAR / DRIFT-OPEN / NOT-RUN verdict
11. Whether the active plan scope touches authentication, data storage, input handling, or public API surface
12. Graphify freshness from `graphify-out/GRAPH_REPORT.md`, the optional `graphify-out/GAL_GRAPHIFY_VERSION.txt`, and current `graphify --version` when available. Classify as `NOT-PRESENT`, `FRESH`, `STALE-BY-TOOL-VERSION`, or `UNSTAMPED`. Treat a report as `FRESH` whenever `GRAPH_REPORT.md` exists and GAL cannot prove a stale-by-tool-version mismatch; only mark it stale when the stamped version differs and `GRAPH_REPORT.md` is not newer than the stamp file. If the report exists without a version stamp, keep it `FRESH` and note that version verification is unavailable. Treat graphify as advisory context only, never as the gating next action for normal GAL flow.
13. Whether more than one non-terminal active plan remains equally runnable or equally paused after checking for stronger execution signals

## Step 2 — Decide

Apply this decision tree in order:

| Condition | Next Action |
| --- | --- |
| No active plan, no work in progress | Repo is already initialized; use `/planning` to start sprint planning |
| More than one non-terminal active plan remains equally ranked after checking blockers, OPEN handoffs, current task state, failing tests, review findings, task-sync mismatch, and active workflow write-back | Do not pick only the first row; list each tied active plan and tell the user to resolve plan priority before implementation starts |
| Localized source plan (non-`en` `planLanguage`) has a planning-authority metadata block whose rendered-source hash no longer matches (draft health = pending reconcile) | The next action is a reconcile: re-enter the relevant planning-stage command (`/deep-planning` / `/refining-plan` / `/plan-to-prompt`), which stops at a read-only reconcile preflight before any other work. Never treat the EN draft (`.dev/plans/<slug>.en.md`) as an active plan |
| Active plan points to source plan only, no execution prompt yet | Run `/refining-plan` to lock the implementation contract into the source plan |
| No eng review recorded | Run the engineering review lane for the source plan through the configured provider, or use `/refining-plan` as the fallback, then record human approval in `## Approval`, then refresh the prompt with `/plan-to-prompt` |
| Plan reviewed, tasks exist, implementation not started | Describe the first implementation task from the plan |
| `### Handoff Notes` contains an OPEN `Retry Handoff` block | Resume that exact task and phase through `/gal pipeline`; do not start a new task |
| `### Handoff Notes` contains an OPEN `Interrupted Phase` block | Resume that exact task and phase through `/gal pipeline`; do not start a new task |
| Paired source plan and execution prompt disagree on any blocking `T-NN` checkbox | Resume `/gal pipeline` for state convergence; do not start a new task until source plan, prompt, and `.dev/state.md` agree |
| `Workflow: IMPLEMENT`, `Current Task` is set, and `Task Final Commit` is missing | Resume `/gal pipeline` on that task; implementation state was not durably closed |
| `Workflow: TEST` and the current task has no task-scoped `## Test Results` subsection yet | Resume `/gal pipeline`; test phase write-back is incomplete |
| `Workflow: REVIEW` and the current task has no task-scoped review verdict yet | Resume `/gal pipeline`; review phase write-back is incomplete |
| Implementation in progress, `### Handoff Notes` present | Resume from the exact "next step" in Handoff Notes |
| Implementation complete, no test results | `golem-tester` in `browser-qa` or `spec` mode, depending on the missing verification surface |
| Tests failing | Return to implementation — summarize what needs fixing |
| Tests passing, no audit recorded | `golem-auditor` for audit |
| Review has BLOCKING findings | Address the BLOCKING items — return to implementation |
| Audit clean, no audit verdict recorded | `golem-auditor` for the required audit before release work |
| `<!-- ANALYZE: DRIFT-OPEN -->` present | Code changes have drifted from plan scope — address deviations, then re-run `golem-auditor` to update verdict |
| `## Tasks` has incomplete blocking items and no BLOCKING findings | Return to implementation — list remaining T-NN tasks |
| All blocking `## Tasks` are complete and only `## Deferred Follow-up` remains | Continue toward verification, release prep, or wrap-up — do not reopen the implementation loop for advisory follow-up alone |
| Open OQs remain in `## Open Questions` | Note count as advisory — do not block; continue to next step |
| Review clean, plan goal-backward VERIFIED | `/gal finalize` to land the plan, or `/gal wrap-up` if the user is pausing instead of landing |
| Blocker listed in `.dev/state.md` | State the blocker and what resolves it before any other action |
| Matching session continuity row plus explicit OPEN interruption evidence in `### Handoff Notes` shows interrupted work | Resume from that row's `Stopped at` and `Next step` in `.dev/state.md` `## Session Continuity` |

## Step 3 — Output

State in plain language:

1. **Where you are** — one sentence describing the current position in the plan lifecycle
2. **Next action** — the single command, review lane, or task to start
3. **Open this first** — which file or context is needed to begin
4. **Graphify note** — include only when graphify freshness is `STALE-BY-TOOL-VERSION`, or when the user explicitly asked about graphify stamping. If stale, say GAL can continue without graphify and the user may refresh graphify artifacts manually if they want updated graph context. If the report is merely unstamped, do not mention graphify unless the user explicitly asked about graphify stamping; when asked, say the report is still usable and version verification is unavailable.

If there is an ambiguous active set instead of one clear target plan:

1. **Where you are** — say the repo has multiple equally-ranked active plans and current recorded state does not prove which one should go first.
2. **Next action** — say to resolve active-plan priority before starting implementation.
3. **Active plans to compare** — list every tied plan with plan name, file path, current phase, and next step.
4. **Open this first** — point to `.dev/state.md` plus the tied execution prompts.
5. **Graphify note** — same stale-only rule as above.

When more than one active plan exists, either mention which plan row was selected and why, or if no row is clearly prior, list every tied plan and state that current recorded state does not disambiguate priority.

Treat `.dev/state.md` `## Session Continuity` as a checkpoint and resume-hint surface, not standalone proof of interrupted work. A continuity row only changes the next action when the active prompt also contains explicit OPEN interruption evidence in `### Handoff Notes`, or when the current workflow phase already has incomplete durable write-back markers.

Do not present multiple unrelated options. If priority is ambiguous, the single next step is to resolve plan priority, and the output must list every tied active plan instead of pretending the first row is authoritative.
