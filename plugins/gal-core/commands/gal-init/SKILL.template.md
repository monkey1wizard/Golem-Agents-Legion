---
name: gal-init
description: "GAL alias for init. Use /gal-init to initialize .dev/ and docs/plans/ through the same bootstrap path as /gal init."
---

# /gal-init

Run the same bootstrap path as `/gal init`, then follow the output block exactly.

## Bootstrap Target

For `init`, the target repo may not contain `.dev/state.md` yet.

- Treat the current working directory as the target project root unless the user explicitly provided another target path.
- Keep the terminal current directory at that target project root so init writes `.dev/` and `docs/plans/` into the repo being bootstrapped.

## Invoke

**Windows:**

1. If `.\scripts\gal.ps1` exists in the target project, run `.\scripts\gal.ps1 dispatch init [args]`.
2. Otherwise run the GAL runtime checkout's `scripts\gal.ps1 dispatch init [args]` while staying in the target project root.

**macOS / Linux:**

1. If `./scripts/gal.sh` exists in the target project, run `./scripts/gal.sh dispatch init [args]`.
2. Otherwise run the GAL runtime checkout's `scripts/gal.sh dispatch init [args]` while staying in the target project root.

Fresh repos commonly need the runtime-checkout fallback because local `scripts/` and `.dev/state.md` do not exist yet.

Pass any text the user typed after `/gal-init` as `[args]`.

## Follow the Output

The script outputs a `--- GAL DISPATCH ---` block. Act on it exactly.
