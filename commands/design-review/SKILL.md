---
name: design-review
description: "Live-site visual audit against DESIGN.md. Finds design inconsistencies on the running app, makes minimal CSS fixes with one commit per fix, and reports Design Score + AI Slop Score."
---

# /design-review

Audit the running application's visual design against `DESIGN.md`. Fix inconsistencies with surgical commits.

## Role

Designer who codes. Your job is to make the live app match the intended design system — not to redesign it.

## When to Use

- After implementation, before `/ship`
- When `/gal status` reports Design Review: MISSING or FINDINGS-OPEN
- As a standalone visual quality check

## When NOT to Use

- Before implementation (use `/plan-design-review` instead)
- When `DESIGN.md` does not exist and there is no design baseline to audit against (run `/design-consultation` first)

## Step 1 — Read Inputs

Read:
- `DESIGN.md` — the authoritative design system to audit against
- The active plan file — note any open design decisions from `### Design Review`
- `.dev/state.md` — identify the active plan

Ask the user for the URL of the running application. If no URL is provided: look for a dev server command in `package.json` or README, start it, then use `localhost:PORT`.

## Step 2 — Open the Site

Use `/browse` to navigate to the URL. Take a full-page screenshot.

## Step 3 — 80-Item Visual Audit

Check the site against these categories. Log all findings before fixing anything.

**Typography (20 checks)**
- Font families match `DESIGN.md` typography roles
- Font weights are consistent with the scale
- Heading hierarchy is semantically correct (h1 → h2 → h3)
- Body text size is readable (minimum 14px)
- Line height is comfortable (1.4–1.6 for body)
- Letter spacing on display type is intentional
- Monospace font used for code/data elements
- No more than 3 font families in use

**Color (20 checks)**
- All color usage maps to `DESIGN.md` tokens
- No hardcoded hex values outside the token system
- Sufficient contrast on all text (WCAG AA: 4.5:1 for normal, 3:1 for large)
- Link colors are distinct from body text
- Error states use the danger token
- Success states use the success token
- Hover states are visually distinct

**Spacing & Layout (20 checks)**
- Spacing values are multiples of the base unit
- Consistent padding inside interactive elements
- Consistent gap between related elements
- No unexplained large gaps or compressed sections
- Content stays within max-width constraints
- Grid columns align correctly

**Components & States (20 checks)**
- Empty states are designed (not blank white areas)
- Loading states are present for async content
- Error states are styled, not browser defaults
- Focus indicators are visible (keyboard accessibility)
- Disabled states are visually distinct
- Interactive elements have hover states
- Modal/drawer overlays have consistent backdrop

## Step 4 — Risk Budget

Track a risk score before making any fix:

| Fix Type | Risk Points |
| --- | --- |
| CSS-only (color, font, spacing) | 0 |
| JSX/TSX attribute change | 1 |
| Component restructure | 3 |

**Hard limits:**
- Maximum 30 fixes per session
- Stop if cumulative risk score > 20
- Ask via `AskUserQuestion` for any fix with risk ≥ 3

## Step 5 — Fix Loop

For each finding in priority order (critical design violations first):

1. Locate the source file
2. Make the minimal change that fixes the finding
3. Commit: `style(design): FINDING-NNN — <description>`
4. Re-navigate to the page in the browser
5. Take a before/after screenshot pair
6. Confirm fixed before moving to the next

One commit per fix. Do not batch fixes into a single commit.

## Step 6 — Write Back to Plan

In the active plan file, append under `## Review Results`:

```markdown
### Design Review (Live)

**Date:** <today>
**URL audited:** <url>
**Design Score:** <A–F>
**AI Slop Score:** <low | medium | high>

#### Fixes Applied (<N> total)

| Finding | File | Fix | Commit |
| --- | --- | --- | --- |
| FINDING-001 | ... | ... | abc1234 |

#### Deferred (risk budget exceeded or requires design decision)

<List deferred items with brief rationale.>

<!-- DESIGN_REVIEW_LIVE: CLEAR -->
```

If any HIGH priority finding could not be fixed: write `<!-- DESIGN_REVIEW_LIVE: FINDINGS-OPEN -->`.

AI Slop Score: **low** = design feels specific to this product; **medium** = some generic patterns; **high** = could be any app.

Tell the user: Design Score, AI Slop Score, fixes applied count, deferred count.
