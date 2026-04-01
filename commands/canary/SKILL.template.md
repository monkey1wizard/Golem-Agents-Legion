---
name: canary
description: "SRE post-deploy monitoring. Cycles through key pages in real Chromium, checks console errors, performance regressions, visual anomalies, and page failures. Alerts on any regression."
---

# /canary

Post-deploy production monitoring. Catch regressions before users do.

## Role

SRE on watch. Monitor production until confident or an alert fires.

## When to Use

- Immediately after `/land-and-deploy`
- As a scheduled sanity check on a production URL
- When the user wants a before/after comparison after a deploy

## Input

Ask the user for:
1. The production URL to monitor (or read from `## Deploy` in active plan)
2. Whether a previous baseline exists (`docs/benchmarks/canary-YYYYMMDD-HHmmss-<url-slug>.json`)

## Step 1 — Load Baseline (if exists)

Read the most recent `docs/benchmarks/canary-YYYYMMDD-HHmmss-<url-slug>.json` for this URL. Match by `url-slug` — the same derivation rule as `/benchmark` (lowercase, hyphens, trimmed). This provides:
- Pre-deploy screenshot hashes for visual comparison
- Previous console error count
- Previous p95 load times for key pages

If no baseline: this run creates the baseline. Tell the user.

**Note:** Canary baselines (`canary-YYYYMMDD-HHmmss-<url-slug>.json`) are a separate artifact family from `/benchmark` baselines (`YYYYMMDD-HHmmss-<url-slug>.json`). Do not mix them for comparison purposes.

## Step 2 — Define Key Pages

From the active plan's `## Test Plan` or from the site's navigation: identify 3–7 key pages to monitor.

Default set if no plan context:
- `/` — homepage / landing
- The primary authenticated view
- Any page recently changed (from git log)

## Step 3 — Monitor Cycle

For each key page:

1. `goto <url>` — measure time to `networkidle`
2. Take a screenshot
3. Read all console messages (filter info, flag warnings + errors)
4. Measure: load time, largest asset, total resources

Record per page: load time (ms), console errors (count), screenshot.

## Step 4 — Compare Against Baseline

If a baseline exists, flag regressions:

| Metric | Alert Threshold |
| --- | --- |
| New console errors | Any new error |
| Load time regression | > 20% slower than baseline |
| Page failure (4xx/5xx) | Any |
| Visual diff | Significant layout change (report, don't auto-alert) |

## Step 5 — Alert on Regression

If any alert threshold is crossed:
- Report immediately: page, metric, delta, screenshot
- Ask the user: "This looks like a regression. Roll back, investigate, or continue monitoring?"

Do not continue silently if a P0 alert fires (page failure or data-loss console error).

## Step 6 — Save Baseline

Save current readings to `docs/benchmarks/canary-YYYYMMDD-HHmmss-<url-slug>.json` where `YYYYMMDD-HHmmss` is the current timestamp (e.g. `canary-20260401-143500-myapp-com.json`):
```json
{
  "date": "<ISO date>",
  "url": "<base url>",
  "pages": [
    { "path": "/", "loadMs": 340, "consoleErrors": 0 }
  ]
}
```

## Step 7 — Write Back to Plan (if active plan)

If an active plan has a `## Deploy` section, append:

```markdown
### Canary Report

**Date:** <today>
**Cycles run:** <N>
**Alerts fired:** <0 | list>
**Result:** HEALTHY / REGRESSION-DETECTED
```

Tell the user: pages checked, alerts fired (if any), baseline saved location.
