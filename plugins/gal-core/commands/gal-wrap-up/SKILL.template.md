---
name: gal-wrap-up
description: "GAL wrap-up ($gal-wrap-up / /gal wrap-up / 暫停 / 交接 / pause session / hand off / wrap up). Non-destructive session pause: compresses ## Handoff Notes, updates .dev/state.md session continuity, commits. Distinct from $gal-finalize (plan landing / 落地). Use when pausing work mid-plan."
---

# /gal wrap-up

Close out the current work session and leave the repo in a resumable state.

## What This Does

- Compresses key task context into the active plan's `### Handoff Notes`
- Updates the matching `.dev/state.md` session continuity row with the current stopped-at state
- Reports which files changed and are ready to commit
- Leaves a clear signal that can be read by `/gal whats-next` in a future session
- Reminds the user to rerun `/graphify .` before the next graph-aware planning or review pass when this session changed repo structure or implementation work

## Step 1 — Collect Active State

Starting from the current working directory or opened workspace folder, walk upward to the nearest ancestor directory that contains `.dev/state.md`. Treat that ancestor as the repo root and read `.dev/state.md` there.

- If no ancestor directory contains `.dev/state.md`, output **Repo not initialized — run `/gal init`.**
- If `.dev/state.md` exists but there is no active plan entry under `## Active Plans`, output **No active session to wrap up.**
- If `.dev/state.md` exists and names one or more active plans, resolve all rows in `## Active Plans`. Table order is priority order. Select the **wrap-up target** as the first row whose plan phase is not terminal (`Complete`, `Done`, `Verified`, `Closed`); if all rows are terminal, fall back to the first row. Read that plan's execution file from the `File` column. Resolve markdown-wrapped relative paths against the current repo root. If the row points to `.dev/plans/<slug>.md`, prefer `.dev/plans/<slug>.prompt.md` when it exists.
- If the active plan file is missing or its `## Status` section does not expose a `Workflow:` field, output the exact repo-state error and suggest inspecting `.dev/state.md` plus the referenced active plan file.

From `.dev/state.md` and the active plan file, extract:

1. `.dev/state.md` — active plan reference and the full session continuity table plus the row matching the wrap-up target's paired source plan path
2. The active plan file — full `## Status`, `### Handoff Notes`, `## Review Results`, `## Test Results`

## Step 2 — Update Handoff Notes

In the active plan's `### Handoff Notes`, write or replace:

- What was accomplished in this session (one paragraph max — compressed, not a diary)
- What is NOT yet done that the plan still requires
- The exact next step to take when resuming (command name, file to open, or task to start)
- Any open decisions, discovered blockers, or context loss risks
- If the active plan is a non-English localized source with a planning-authority metadata block (the EN-draft flow — see `workflows/coding.md`), **preserve the EN draft path and its `draft-hash` / `rendered-source-hash`** in the handoff so the next session can detect a pending reconcile and re-enter the read-only reconcile preflight. The tracked EN draft (`.dev/plans/<slug>.en.md`), while it still exists, and the source plan's inline `prompt-hash` / `equivalence-verdict` fields are the resume anchors, not provider-local memory.

Treat `/gal wrap-up` as the required handoff path before pausing work, switching providers, or switching machines. The handoff target is the active `.dev/plans/<slug>.prompt.md` plus `.dev/state.md`, not provider-local transcript memory.

If previous Handoff Notes exist and are now stale, replace them entirely. Keep only what is needed to resume — this is working memory, not a record. **Exception:** preserve any existing `#### Approved Memory-Harvest Candidates` subsection (written by Step 2a) verbatim until `/gal finalize` consumes or promotes it — a human explicitly approved that content, so it survives the replace exactly as the EN-draft resume anchors above do; never drop it as stale.

## Step 2a — Provider-Memory Harvest (opt-in)

This is the sole proposal/approval inlet for provider memory (see `conventions/token-budget.md` → Provider-Memory Harvest for the canonical contract). This step never lists, opens, globs, parses, writes, edits, or deletes provider-memory paths — it considers only content the active runtime has already surfaced in this conversation.

1. **Disabled** — read `config.json#memoryHarvest.enabled`. Missing or `false`: silent no-op, skip the rest of this step entirely, do not mention Harvest in the wrap-up report.
2. **No-active-prompt** — enabled, but the wrap-up target's `File` row has no materialized `.dev/plans/<slug>.prompt.md` (no Handoff Notes write target exists yet): propose nothing, record `Harvest: not-run — no active execution prompt`, continue to Step 3 normally.
3. **Empty** — enabled, execution prompt exists, but no provider-memory content has been surfaced in the current task context: record `Harvest: not-run — no eligible provider memory surfaced`, continue to Step 3 normally.
4. **Excluded** — a surfaced item lacks a current-repo anchor verifiable against repo files, or carries a raw quote, secret, absolute/provider path, cross-project content, or personal content: exclude it silently; it never becomes a candidate.
5. **Rejected** — a candidate that passed anchor verification and redaction is proposed to the human for explicit per-candidate approval. Rejected or left unanswered: no write, candidate is dropped.
6. **Approved** — write the paraphrased, redacted candidate into `### Handoff Notes` under `#### Approved Memory-Harvest Candidates`, marked `provisional` and `advisory`. This is task execution memory only, not a promotion — see `conventions/token-budget.md` for the unchanged promotion gate.

## Step 3 — Update Session Continuity

In `.dev/state.md` under `## Session Continuity`, update or create exactly one row for the wrap-up target. Match rows by paired source plan path and leave other plans' rows untouched.

Use this shape:

| Plan | Source Plan | Last Session | Stopped At | Next Step | Context |
| --- | --- | --- | --- | --- | --- |
| [active plan name] | [.dev/plans/plan-slug.md] | [today's date, approximate time] | [one-line description of the last completed action] | [exact action to take when resuming] | [current phase marker if any, any key state needed to restore] |

## Step 4 — Report and Suggest Commit

Tell the user:

1. What was written to `### Handoff Notes`
2. What was written to the matching row in `## Session Continuity`
3. The Provider-Memory Harvest outcome from Step 2a — the approved candidate(s) written, or the exact `Harvest: not-run — ...` line; omit entirely only when Step 2a took the Disabled silent no-op path
4. Which files changed and need to be committed
5. If this session changed code, structure, or plan-relevant architecture context and the repo uses graphify, remind them to rerun `/graphify .` before the next graph-aware planning or review pass

Suggest the git commit:

```bash
git add .dev/state.md .dev/plans/<active-plan>.prompt.md
git commit -m "chore: session wrap-up — <one-line summary of stopped-at>"
```

Do not commit on the user's behalf. Suggest only.

## Step 5 — Reverse-Prompt finalize When the Plan Is Complete

If the wrap-up target plan is in a **completed state** — every blocking task in `## Tasks` is `[x]` **and** the execution prompt records an ORCHESTRATOR goal-backward verification verdict of VERIFIED — the right next move is usually `/gal finalize` (land + close), not another pause. Offer it once:

```text
This plan looks complete (all tasks done + VERIFIED). If you want to land and close it out,
I can run /gal finalize — but only if you confirm. Otherwise this wrap-up leaves it cleanly resumable.
```

Rules (mirroring the working-hours wrap-up offer):

- **Offer once per invocation**, then stop. Do not repeat or nag.
- **Do not auto-run `/gal finalize`.** Run it only after an explicit user confirmation such as `yes, run finalize`.
- If the plan is **not** in a completed state, skip this step silently — wrap-up is a non-destructive pause and must not push landing.

This is a mutual redirect, not a turf war: `/gal finalize` reverse-prompts `/gal wrap-up` for "just pausing", and wrap-up reverse-prompts finalize for "actually done". See `workflows/coding.md` → finalize ↔ wrap-up Boundary.
