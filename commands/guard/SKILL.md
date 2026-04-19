---
name: guard
description: "Maximum safety mode. Combines /careful (destructive command warnings) and /freeze (directory-scoped edit lock) in a single command. Use when touching production systems, live data, or any session where accidental lateral damage must be prevented."
---

# /guard

Activate maximum safety mode for the session.

## Role

Combined safety activator. One command to enable all available accident-prevention guardrails.

## When to Use

- Touching production systems
- Debugging with live data
- Working in a shared environment where lateral changes could affect other developers
- Any session where you want "no surprises" protection

## What /guard Does

`/guard` combines two guardrails in one command:

### 1. /careful — Destructive Command Warnings

Activates warnings before any destructive command:
- `rm -rf` (non-whitelisted paths)
- `DROP TABLE`, `DROP DATABASE`, `TRUNCATE`
- `git push --force`, `git reset --hard`, `git checkout .`
- `kubectl delete`, `docker rm -f`, `docker system prune`

See `/careful` for the full list and whitelist.

### 2. /freeze — Directory Edit Boundary

Prompts for a directory to lock edits within.

```
/guard
> Which directory should edits be locked to? (press enter to skip freeze, or type a path)
```

If a path is provided: activates `/freeze <path>`.
If skipped: only `/careful` is active.

## Override

Both guardrails allow user override. Nothing is hard-blocked — these are accident-prevention layers, not access control.

## Deactivating

- To remove the freeze boundary: run `/unfreeze`
- Careful mode and freeze deactivate automatically at session end

## No Plan Files

`/guard` does not write to plan files.
