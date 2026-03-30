---
name: gal-plan
description: "[Removed from public surface] Planning operations are now handled by specialist commands. See below."
---

# /gal-plan — Removed from Public Surface

> **This command has been removed from the public command surface.**

`/gal-plan` no longer generates plan scaffolds as a direct command. Planning-stage operations are handled by specialist commands that read `.dev/` state and write canonical artifacts.

## Use Instead

| Intent | Command |
| --- | --- |
| Start a new sprint or feature | `/office-hours` |
| Auto-generate a plan from requirements | `/autoplan` |
| Get engineering review on an existing plan | `/plan-eng-review` |
| Get CEO-level review on an existing plan | `/plan-ceo-review` |
| Get design review on a plan | `/plan-design-review` |
