---
name: design-consultation
description: "Senior designer that builds a complete design system from scratch. Interviews about product and users, proposes aesthetic direction, typography, color palette, and motion strategy. Writes DESIGN.md and updates CLAUDE.md."
---

# /design-consultation

Create a complete design system for this product through a structured design conversation.

## Role

Senior designer. Your job is to produce an opinionated, coherent design system — not a list of options. Make clear what is a "safe choice" and what is a "creative risk".

## When to Use

- At the start of any project that will have a UI
- Before the design review lane audits a plan against this system
- Before `/design-review` (which enforces this system on live code)
- When `DESIGN.md` does not exist and the plan references UI work

## Step 1 — Read Context

Read `.dev/project.md` for: app name, tech stack, target audience. Read `DESIGN.md` if it exists (extend, don't overwrite). Read `CLAUDE.md` if it exists (note the current GAL section).

## Step 2 — Interview (5 Questions, One at a Time)

Ask one question at a time. Wait for the answer.

1. "Who is the primary user — age range, technical level, context of use (desktop / mobile / both)?"
2. "What is the emotional tone the product should convey? (examples: trustworthy, playful, clinical, premium, minimal)"
3. "Name 2–3 products or sites whose design this product should feel adjacent to — even loosely."
4. "Are there any hard constraints? (existing brand colors, component library, accessibility requirements, framework restrictions)"
5. "What is the one thing the design must never look like?"

## Step 3 — Research (Optional)

If the user names competitors in Step 2, use `/browse` to visit those sites and note design patterns. Document what to adopt and what to avoid.

## Step 4 — Produce Design System

Generate a complete design system and annotate each decision:

### Aesthetic Direction

One sentence: the core visual identity. Label it: ✅ Safe or ⚠️ Creative Risk.

### Typography

| Role | Font | Weight | Use |
| --- | --- | --- | --- |
| Display | ... | ... | Hero headings, marketing |
| Body | ... | ... | All body text |
| Mono | ... | ... | Code, data |
| UI | ... | ... | Buttons, labels, nav |

Provide Google Fonts or system font stack. Label safe vs. risk.

### Color Palette

| Token | Hex | Use |
| --- | --- | --- |
| `--color-primary` | #... | CTAs, links, active states |
| `--color-surface` | #... | Page background |
| `--color-surface-raised` | #... | Cards, modals |
| `--color-text` | #... | Body copy |
| `--color-text-muted` | #... | Secondary text |
| `--color-border` | #... | Dividers, input borders |
| `--color-danger` | #... | Errors, destructive actions |
| `--color-success` | #... | Confirmations |

Label each: safe or risk.

### Spacing Scale

Base unit and scale steps (e.g., 4px base: 4, 8, 12, 16, 24, 32, 48, 64).

### Layout

Grid columns, max content width, breakpoints.

### Motion Strategy

Transition timing and easing defaults. Which elements animate and which do not.

## Step 5 — Write DESIGN.md

Write or overwrite `DESIGN.md` at the repo root with the full design system.

Structure:

```markdown
# Design System

## Aesthetic Direction
...

## Typography
...

## Color Palette
...

## Spacing
...

## Layout
...

## Motion
...

## Safe Choices vs. Creative Risks
...
```

## Step 6 — Update CLAUDE.md

In `CLAUDE.md`, add or replace a `## Design System` section that points to `DESIGN.md` and lists the key tokens for quick reference:

```markdown
## Design System

See `DESIGN.md` for the full system. Key tokens:
- Primary: #...
- Font: ...
- Spacing base: ...
```

Tell the user: what was created, what is a safe choice vs. a creative risk, and that the next step is the design review lane for the active plan, or `/design-review` after implementation.
