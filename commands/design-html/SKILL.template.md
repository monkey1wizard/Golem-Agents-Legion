---
name: design-html
description: "Converts an approved design mockup to production HTML. Reads variant-approved.json from /design-shotgun, generates self-contained HTML with live-reload, and iterates until done."
---

# /design-html

Convert an approved design mockup into production-ready HTML or a framework component.

## Role

Design engineer. Your job is to implement the approved mockup exactly — not to redesign it.

## When to Use

- After `/design-shotgun` produces `docs/designs/<plan-slug>/variant-approved.json`
- When you need to convert a visual design to runnable code

## Step 1 — Read Inputs

Read:
- `docs/designs/<plan-slug>/variant-approved.json` — approved design spec and key decisions
- `DESIGN.md` — color palette, typography, spacing scale (apply exactly)
- `package.json` if it exists — detect framework (React, Vue, Svelte, plain HTML)

If no `variant-approved.json` exists: ask the user to run `/design-shotgun` first, or describe the design they want implemented.

## Step 2 — Extract Implementation Spec

From `variant-approved.json` and any provided mockup image, extract:

- Layout structure (grid, flexbox, columns)
- Component hierarchy
- Color tokens used (map to `DESIGN.md` tokens where possible)
- Typography roles (display, body, UI)
- Interactive states (hover, active, focus, disabled)
- Responsive breakpoints

## Step 3 — Choose Layout Strategy

Apply the appropriate Pretext layout API based on UI type:

| UI Type | Strategy |
| --- | --- |
| Simple layouts, cards, marketing sections | `prepare() + layout()` |
| Chat, feed, sequential content | `walkLineRanges()` |
| Editorial, article, text-heavy | `layoutNextLine()` |
| Complex multi-panel UI | Full engine |

For framework projects: generate a component in the detected framework. For plain HTML or unknown: generate self-contained HTML.

## Step 4 — Generate HTML

Generate the HTML/component with:
- Inline styles using `DESIGN.md` token values (no external CSS dependencies unless the project already uses them)
- All states implemented (hover, active, empty, loading, error)
- Semantic HTML elements
- Basic accessibility attributes (aria-label, role) where applicable

## Step 5 — Live Preview

Spin up a live-reload server and open the output at `localhost:PORT`.

Take screenshots at:
- Desktop (1440px)
- Tablet (768px)
- Mobile (375px)

## Step 6 — Iterate

For each piece of user feedback:
- Make the smallest edit that addresses it
- Re-screenshot
- Ask: "Done, or another change?"

Continue until the user confirms done.

## Step 7 — Save Output

Save the finalized HTML:
- `docs/designs/<plan-slug>/handoff-final.html` (for standalone HTML)
- Or the framework component file at the appropriate location

Tell the user: output path, and that the next step is to copy this component into the actual app code.
