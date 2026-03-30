---
name: design-shotgun
description: "Visual design explorer. Generates 3 design variants using GPT Image API, opens a comparison board, and iterates until the user approves one. Records taste preferences for future generations."
---

# /design-shotgun

Generate multiple visual design variants for a UI component or screen. Present them for comparison. Iterate until one is approved.

## Role

Design explorer. Your job is to produce distinct, opinionated variants — not slight variations. Each variant should represent a different visual direction.

## When to Use

- When you need a visual mockup before writing code
- After `/design-consultation` establishes the design system
- Before `/design-html` (which converts the approved variant to HTML)

## Step 1 — Read Context

Read `DESIGN.md` if it exists — use the color palette, typography, and aesthetic direction as constraints. Read `.dev/project.md` for app name and target audience.

## Step 2 — Clarify the Subject

Ask: "What exactly are we designing? (e.g., landing page hero, login screen, dashboard card, nav bar)"

If the user already stated it, skip this question.

## Step 3 — Generate 3 Variants

Use the GPT Image API (or describe the variants in detail if image generation is not available) to produce 3 distinct visual directions:

- **Variant A** — safe, conventional. Follows established patterns for this UI type.
- **Variant B** — pushes 1–2 dimensions of the design system. More distinctive.
- **Variant C** — creative risk. Challenges layout conventions or visual hierarchy.

For each variant, provide:
- A name / one-line character description
- The key visual decisions made
- What makes it different from the others

## Step 4 — Present for Comparison

Open a comparison at `localhost:PORT` if image generation is available. Otherwise: present the three variants as text descriptions with detailed visual specs.

For each variant, offer actions:
- **Approve** → proceed to Step 5
- **Remix** → adjust specific parameters (colors, layout, density) and regenerate
- **Reject** → describe why and generate a replacement

## Step 5 — Record Taste Preferences

After the user approves a variant, note:
- Which variant type was chosen (safe / expressive / risk)
- What elements from rejected variants were cited
- Any explicit user preferences stated during iteration

These preferences inform future `/design-shotgun` calls.

## Step 6 — Save Approved Design

Save the approved variant:

- Image: `docs/designs/<slug>/approved.png` (if image generated)
- Spec: `docs/designs/<slug>/approved.json`

`approved.json` format:

```json
{
  "slug": "<slug>",
  "subject": "<what was designed>",
  "variant": "<A|B|C>",
  "character": "<one-line description>",
  "key_decisions": ["..."],
  "taste_notes": ["..."],
  "approved_date": "<today>"
}
```

Tell the user: variant approved, path to `approved.json`, and that `/design-html` will convert this to production HTML.
