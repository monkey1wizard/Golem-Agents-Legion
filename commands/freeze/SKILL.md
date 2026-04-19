---
name: freeze
description: "Directory-scoped edit lock. Blocks file edits outside the specified path for the current session. Auto-activated by /investigate for the module being debugged. Not a security sandbox — accident prevention only."
---

# /freeze

Lock edits to a specific directory for the current session.

## Role

Edit boundary enforcer. Prevent accidental changes outside the active scope.

## When to Use

- Auto-activated by `/investigate` for the module being debugged
- When you want to ensure changes stay within a specific area
- When working on a focused refactor and want to prevent scope creep

## Activation

```
/freeze <path>
```

Example:
```
/freeze src/api/
```

After activation: any attempt to edit a file outside `src/api/` triggers a warning and confirmation before proceeding.

## Scope

Freeze applies to **file edits** only:
- Blocked: creating, editing, or overwriting files outside the boundary
- Not blocked: reading files outside the boundary
- Not blocked: shell commands (e.g. `grep`, `cat`) — this is not a security sandbox

## Warning Format

When an edit outside the boundary is attempted:
```
⚠️  Freeze active: src/api/
You are about to edit: src/components/Button.tsx
This is outside the freeze boundary.
Proceed anyway? (yes / cancel)
```

## Multiple Freeze Boundaries

Only one freeze boundary is active at a time. Running `/freeze <new-path>` replaces the previous boundary.

## Checking Freeze State

`/freeze status` — shows the current boundary path, or "No freeze active."

## Deactivation

Run `/unfreeze` to remove the boundary.

## No Plan Files

`/freeze` does not write to plan files. `/investigate` writes its own `## Debug Session` section.
