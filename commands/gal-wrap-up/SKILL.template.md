---
name: gal-wrap-up
description: "GAL — wrap up and hand off the current session. Converges handoff artifacts, updates session continuity, and prepares the repo for clean resumption by any session or machine."
---

# /gal wrap-up

Close out the current work session and leave the repo in a resumable state.

## What This Does

- Compresses key task context into the active plan's `### Handoff Notes`
- Updates `.dev/state.md` session continuity with the current stopped-at state
- Reports which artifacts changed and are ready to commit
- Leaves a clear signal that can be read by `/gal whats-next` in a future session

## Step 1 — Collect Active State

Run this command in the terminal to collect all state files:

**Windows:**
```
{{GAL_ROOT}}\scripts\gal.ps1 state-dump
```

**macOS / Linux:**
```
{{GAL_ROOT}}/scripts/gal.sh state-dump
```

The output is a structured dump. Check the `KIND:` line:

- `KIND: uninitialized` → output **Repo not initialized — run `/gal init`.**
- `KIND: state-error` → output the `ERROR:` line and suggest inspecting `.dev/state.md`.
- `KIND: idle` → output **No active session to wrap up.**
- `KIND: active` → proceed to Step 2 using the file contents from the dump.

From the dump, extract:
1. `.dev/state.md` — active plan reference and current session continuity
2. The active plan file — full `## Status`, `### Handoff Notes`, `## Review Results`, `## Test Results`

## Step 2 — Update Handoff Notes

In the active plan's `### Handoff Notes`, write or replace:

- What was accomplished in this session (one paragraph max — compressed, not a diary)
- What is NOT yet done that the plan still requires
- The exact next step to take when resuming (command name, file to open, or task to start)
- Any open decisions, discovered blockers, or context loss risks

If previous Handoff Notes exist and are now stale, replace them entirely. Keep only what is needed to resume — this is working memory, not a record.

## Step 3 — Update Session Continuity

In `.dev/state.md` under `## Session Continuity`, update all fields:

```
Last session: [today's date, approximate time]
Stopped at: [one-line description of the last completed action]
Next step: [exact action to take when resuming]
Context: [active plan name, workflow state, any key state needed to restore]
```

## Step 4 — Report and Suggest Commit

Tell the user:

1. What was written to `### Handoff Notes`
2. What was written to `## Session Continuity`
3. Which files changed and need to be committed

Suggest the git commit:

```
git add .dev/state.md docs/plans/<active-plan>.prompt.md
git commit -m "chore: session wrap-up — <one-line summary of stopped-at>"
```

Do not commit on the user's behalf. Suggest only.
