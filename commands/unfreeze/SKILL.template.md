---
name: unfreeze
description: "Remove the active /freeze boundary. Allows edits everywhere again. The freeze hook stays registered for the session but permits all paths. Run /freeze again to set a new boundary."
---

# /unfreeze

Remove the active freeze boundary.

## Role

Unlock. Allow edits everywhere again.

## When to Use

- After `/investigate` completes and the module lock is no longer needed
- After a focused implementation phase when you want to work across multiple directories
- Whenever the freeze boundary has served its purpose

## Operation

Removes the current `/freeze` boundary. File edits everywhere are permitted again.

The session hook stays registered (so you can run `/freeze <path>` again at any time without re-activating).

## Confirmation

```
Freeze removed. Edits are now permitted everywhere.
Previous boundary: src/api/
```

## If No Freeze Is Active

```
No freeze boundary was active. Nothing to remove.
```

## No Plan Files

`/unfreeze` does not write to plan files.
