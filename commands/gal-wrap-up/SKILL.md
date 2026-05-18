---
name: gal-wrap-up
description: "GAL — wrap up and hand off the current session. Converges handoff updates, updates session continuity, and prepares the repo for clean resumption by any session or machine."
---

# /gal wrap-up

Close out the current work session and leave the repo in a resumable state.

## What This Does

- Compresses key task context into the active plan's `### Handoff Notes`
- Updates `.dev/state.md` session continuity with the current stopped-at state
- Reports which files changed and are ready to commit
- Leaves a clear signal that can be read by `/gal whats-next` in a future session
- Reminds the user to rerun `/graphify .` before the next graph-aware planning or review pass when this session changed repo structure or implementation work

## Step 1 — Collect Active State

Starting from the current working directory or opened workspace folder, walk upward to the nearest ancestor directory that contains `.dev/state.md`. Treat that ancestor as the repo root and read `.dev/state.md` there.

- If no ancestor directory contains `.dev/state.md`, output **Repo not initialized — run `/gal init`.**
- If `.dev/state.md` exists but there is no active plan entry under `## Active Plans`, output **No active session to wrap up.**
- If `.dev/state.md` exists and names an active plan, read that plan's execution file from the `File` column. Resolve markdown-wrapped relative paths against the current repo root. If the row points to `docs/plans/<slug>.md`, prefer `.dev/plans/<slug>.prompt.md` when it exists.
- If the active plan file is missing or its `## Status` section does not expose a `Workflow:` field, output the exact repo-state error and suggest inspecting `.dev/state.md` plus the referenced active plan file.

From `.dev/state.md` and the active plan file, extract:

1. `.dev/state.md` — active plan reference and current session continuity
2. The active plan file — full `## Status`, `### Handoff Notes`, `## Review Results`, `## Test Results`

## Step 2 — Update Handoff Notes

In the active plan's `### Handoff Notes`, write or replace:

- What was accomplished in this session (one paragraph max — compressed, not a diary)
- What is NOT yet done that the plan still requires
- The exact next step to take when resuming (command name, file to open, or task to start)
- Any open decisions, discovered blockers, or context loss risks

Treat `/gal wrap-up` as the required handoff path before pausing work, switching providers, or switching machines. The handoff target is the active `.dev/plans/<slug>.prompt.md` plus `.dev/state.md`, not provider-local transcript memory.

If previous Handoff Notes exist and are now stale, replace them entirely. Keep only what is needed to resume — this is working memory, not a record.

## Step 3 — Update Session Continuity

In `.dev/state.md` under `## Session Continuity`, update all fields:

```text
Last session: [today's date, approximate time]
Stopped at: [one-line description of the last completed action]
Next step: [exact action to take when resuming]
Context: [active plan name, current phase marker if any, any key state needed to restore]
```

## Step 4 — Report and Suggest Commit

Tell the user:

1. What was written to `### Handoff Notes`
2. What was written to `## Session Continuity`
3. Which files changed and need to be committed
4. If this session changed code, structure, or plan-relevant architecture context and the repo uses graphify, remind them to rerun `/graphify .` before the next graph-aware planning or review pass

Suggest the git commit:

```bash
git add .dev/state.md docs/plans/<active-plan>.prompt.md
git commit -m "chore: session wrap-up — <one-line summary of stopped-at>"
```

Do not commit on the user's behalf. Suggest only.
