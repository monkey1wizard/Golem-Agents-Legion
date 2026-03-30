---
name: land-and-deploy
description: "Deploy pipeline — merges an approved PR, waits for CI, waits for deploy to complete, then runs a canary health check against production. Requires /setup-deploy to have run once."
---

# /land-and-deploy

Merge the PR and verify it in production.

## Role

Deploy pipeline. Merge to verified-in-production.

## When to Use

- After PR is approved and CI is green
- When `/gal whats-next` reports PR open and ready to land

## Prerequisites

- `/setup-deploy` must have run once for this project. If deploy config is missing from `CLAUDE.md`: stop and tell the user to run `/setup-deploy` first.

## First-Run Safety

On the first run per project: perform a dry-run walk-through. Show every step you will take and confirm with the user before any irreversible action (merge, deploy command).

## Step 1 — Read Config

Read `CLAUDE.md` for the deploy configuration block written by `/setup-deploy`:
- Platform (Fly.io / Render / Vercel / Netlify / Heroku / GitHub Actions / custom)
- Production URL
- Deploy command
- Health check endpoint
- Status command

Read the active plan's `## Ship` section to get the PR number.

## Step 2 — Confirm PR Is Mergeable

Check:
- PR status: approved
- CI checks: all passing
- No merge conflicts

If any check fails: stop and tell the user what is blocking.

## Step 3 — Merge the PR

Merge using squash merge by default (preserves clean history).

Confirm merge with the user before running if this is the first deploy for the project.

## Step 4 — Wait for CI on Main

Monitor CI status on the main branch. Wait up to 10 minutes. Report status every 2 minutes.

If CI fails on main: report the failure and recommended rollback steps.

## Step 5 — Trigger Deploy and Wait

Run the deploy command from config. Wait for deploy completion. Report status.

If deploy fails: report the error output and recommend rollback.

## Step 6 — Canary Health Check

After deploy completes, run a quick canary pass:
1. Use `/browse goto <production-url>`
2. Check the health endpoint if configured
3. Check for console errors on key pages
4. Take a screenshot of the production landing page

If health check fails: report immediately. Do not mark deploy as verified.

## Step 7 — Write Back to Plan

In the active plan file, append:

```markdown
## Deploy

**Date:** <today>
**Platform:** <platform>
**Production URL:** <url>
**Version / commit:** <hash>
**Health check:** PASSED / FAILED
**Screenshot:** docs/screenshots/deploy-<date>.png
```

Tell the user: production URL, deploy timestamp, health check result.

Suggest: `/canary` for ongoing production monitoring, or `/retro` to reflect.
