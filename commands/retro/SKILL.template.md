---
name: retro
description: "Engineering Manager — analyzes git history for commits, LOC, test ratios, PR sizes, fix ratios, hotspot files, and shipping streaks. Writes a human-readable retro report and saves a JSON snapshot to docs/retros/."
---

# /retro

Reflect on what was built, how it was built, and how to build better.

## Role

Engineering manager running a retrospective. Data-driven, specific, and honest.

## When to Use

- After a sprint or major feature ships
- Weekly to track momentum
- When `/gal wrap-up` notes a session worth reflecting on

## Modes

| Command | Behavior |
| --- | --- |
| `/retro` | Analyze this repo since last retro snapshot |
| `/retro global` | Analyze all repos that have retro snapshots |

## Step 1 — Load Previous Snapshot

Check `docs/retros/` for a previous JSON snapshot. If found, compute deltas.

The snapshot date determines the analysis window (since last retro to now).

## Step 2 — Collect Git Metrics

```
git log --since=<last-retro-date> --numstat --format="%H %ae %ad %s"
```

Compute:
- **Commits**: total count
- **LOC added / removed**: from numstat
- **Contributors**: unique author emails
- **PR sizes**: commits by branch size (small < 5 files, medium 5–20, large > 20)
- **Fix ratio**: commits starting with `fix:` / total commits
- **Shipping streak**: consecutive days with at least one commit
- **Biggest ship**: the commit or PR with the most LOC

## Step 3 — Analyze Test Health

Find all test files (`*.test.*`, `*.spec.*`, `__tests__/`, `tests/`):
- **Total test files**: count
- **Tests added this period**: test files modified in window
- **Regression test commits**: commits starting with `test(qa):`
- **Test ratio trend**: (test LOC) / (implementation LOC) — flag if < 20%

## Step 4 — Find Hotspot Files

Files modified in the most commits during this window are hotspots:
- List the top 5 hotspot files with change count
- Note if any hotspot has no corresponding test file (risk area)

## Step 5 — Write the Retro Report

Structure:

```markdown
# Sprint Retro — <date>

## Summary

<2–3 sentence narrative of what was accomplished this period.>

## By the Numbers

| Metric | This Period | vs Last Period |
| --- | --- | --- |
| Commits | N | ±delta |
| LOC added | N | ±delta |
| Fix ratio | N% | ±delta |
| Test ratio | N% | ±delta |
| Shipping streak | N days | |

## Biggest Ship

<Brief description of the most significant commit or PR.>

## Test Health

<Assessment: improving / stable / declining. Specific numbers.>

## Hotspot Files

<Top 3 files changed most often. Risk assessment if untested.>

## Per-Contributor Highlights

<Specific praise per contributor. Growth opportunity if appropriate. Skip if solo project.>

## Next Period Focus

<1–3 concrete suggestions based on the data.>
```

## Step 6 — Save JSON Snapshot

Save to `docs/retros/YYYYMMDD.json` where `YYYYMMDD` is today's date (e.g. `docs/retros/20260401.json`):
```json
{
  "date": "<ISO date>",
  "window": { "from": "<date>", "to": "<date>" },
  "commits": N,
  "locAdded": N,
  "locRemoved": N,
  "fixRatio": 0.12,
  "testRatio": 0.24,
  "shippingStreak": N,
  "hotspots": ["src/api/handler.ts", "..."],
  "contributors": [{ "email": "...", "commits": N }]
}
```

Tell the user: key metrics, test health assessment, and the location of the saved report.
