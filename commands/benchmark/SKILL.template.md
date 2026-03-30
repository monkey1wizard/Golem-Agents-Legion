---
name: benchmark
description: "Performance Engineer — measures page load, Core Web Vitals (LCP, CLS, INP), resource counts, and transfer size using real Chromium. Saves a baseline and shows before/after comparison if a previous baseline exists."
---

# /benchmark

Measure real performance in real Chromium. Know your numbers before and after changes.

## Role

Performance engineer. Measure, baseline, compare.

## When to Use

- Before starting implementation on a potentially perf-sensitive feature (to capture baseline)
- After implementation, before `/ship`
- Before and after any infrastructure change
- When the user suspects a performance regression

## Input

Ask the user for:
1. URL(s) to benchmark (or read from active plan / `## Deploy`)
2. Whether to compare against a saved baseline

## Step 1 — Load Previous Baseline (if exists)

Check `docs/benchmarks/` for a previous run for this URL.

If found: this run will produce a before/after comparison. Tell the user which baseline will be used.

If not found: this run creates the baseline.

## Step 2 — Warm Up

Navigate to each URL once (throw away results) to populate browser cache for a cache-warm test.

## Step 3 — Measure (5 runs, averaged)

For each URL, run 5 measurements and average the results.

**Metrics to capture:**

| Metric | Tool | Target |
| --- | --- | --- |
| Page load time | `networkidle` timestamp | < 2s |
| LCP (Largest Contentful Paint) | Performance API | < 2.5s |
| CLS (Cumulative Layout Shift) | Layout instability observer | < 0.1 |
| INP (Interaction to Next Paint) | Event timing | < 200ms |
| Total resources | HAR | minimize |
| Total transfer size | HAR | < 1MB |
| JS bundle size | Resource timing | flag if > 300KB |
| Time to First Byte | Resource timing | < 600ms |

## Step 4 — Identify Regressions (if baseline exists)

Flag any metric that is worse than baseline by:

| Degradation | Severity |
| --- | --- |
| > 50% slower | Critical |
| 20–50% slower | Warning |
| < 20% slower | Informational |

## Step 5 — Identify Top Optimization Opportunities

Regardless of baseline comparison, report:
- Largest uncompressed asset
- Any render-blocking scripts
- Images without width/height (causes CLS)
- Fonts loaded without `font-display: swap`

## Step 6 — Save Baseline

Save to `docs/benchmarks/<date>-<url-slug>.json`:
```json
{
  "date": "<ISO date>",
  "url": "<url>",
  "runs": 5,
  "metrics": {
    "loadMs": { "avg": 420, "p95": 510 },
    "lcp": 1.8,
    "cls": 0.02,
    "inp": 95,
    "transferKB": 380,
    "resourceCount": 42
  }
}
```

## Step 7 — Write Back to Plan (if active plan)

If an active plan exists, append:

```markdown
## Performance

**Date:** <today>
**URL:** <url>

| Metric | Value | vs Baseline | Target |
| --- | --- | --- | --- |
| Load time | 420ms | +12ms (+3%) | < 2000ms |
| LCP | 1.8s | -0.2s (-10%) ✅ | < 2.5s |

#### Top Optimization Opportunities

1. <finding>
2. <finding>
```

Tell the user: key metrics, regressions found (if any), baseline saved location.
