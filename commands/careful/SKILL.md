---
name: careful
description: "Accident prevention guardrails. Warns before destructive commands: rm -rf, DROP TABLE, git push --force, git reset --hard, kubectl delete, docker system prune, and similar. Whitelisted for common build artifact cleanup. User can always override."
---

# /careful

Activate accident-prevention warnings for destructive commands.

## Role

Guardrail for your session. Prevent accidents — not enforce access control.

## When to Use

- Any time you're working near risky operations
- When the user says "be careful" — activate automatically
- When working with production data, shared databases, or irreversible states

## Activation

Run `/careful` or say "be careful" to activate for the current session.

Deactivates automatically when the session ends.

## Guarded Commands

Warn before executing any of these:

| Command Pattern | Risk |
| --- | --- |
| `rm -rf <path>` (non-whitelisted) | Permanent file deletion |
| `DROP TABLE`, `DROP DATABASE` | Permanent data loss |
| `TRUNCATE <table>` | Permanent data loss |
| `git push --force`, `git push -f` | Overwrites remote history |
| `git reset --hard` | Discards local commits |
| `git checkout .`, `git restore .` | Discards local changes |
| `kubectl delete` | Removes production resources |
| `docker rm -f` | Force removes container |
| `docker system prune` | Removes all unused Docker data |

Warning format:
```
⚠️  Careful mode: This command is destructive.
Command: rm -rf ./data
Effect: Permanently deletes ./data and all its contents.
Proceed? (yes / cancel)
```

## Whitelist — No Warning Needed

Common build artifact cleanup is safe and should not trigger false alarms:

| Pattern | Reason |
| --- | --- |
| `rm -rf node_modules` | Rebuilt by npm install |
| `rm -rf dist` | Rebuilt by build |
| `rm -rf .next` | Rebuilt by Next.js build |
| `rm -rf __pycache__` | Rebuilt by Python |
| `rm -rf build` | Rebuilt by build |
| `rm -rf coverage` | Rebuilt by test runner |
| `rm -rf .turbo` | Turbo cache, safe to delete |

## Override

The user can always proceed. These are accident-prevention guardrails. If the user confirms: execute the command without further warnings for that specific invocation.

## No Plan Artifacts

`/careful` does not write to plan files.
