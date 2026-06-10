---
name: gal-init
description: "GAL alias for init. Use /gal-init to initialize .dev/ and docs/plans/ through the same bootstrap path as /gal init."
---

# /gal-init

Run the same bootstrap path as `/gal init` by calling the Rust `gal init-repo` command in the target repo root.

## Bootstrap Target

For `init`, the target repo may not contain `.dev/state.md` yet.

- Treat the current working directory as the target project root unless the user explicitly provided another target path.
- Keep the terminal current directory at that target project root so init writes `.dev/` and `docs/plans/` into the repo being bootstrapped.

## Invoke

**Windows:**

1. Run `gal init-repo [args]` in the target project root.
2. If the repo has no local `scripts/` yet, keep the terminal in the target root and call the installed `gal` binary from PATH.

**macOS / Linux:**

1. Run `gal init-repo [args]` in the target project root.
2. If the repo has no local `scripts/` yet, keep the terminal in the target root and call the installed `gal` binary from PATH.

Fresh repos commonly need the installed-binary path because local `scripts/` and `.dev/state.md` do not exist yet.

Pass any text the user typed after `/gal-init` as `[args]`.

On success, review `.dev/project.md`, then run `/gal status`.
