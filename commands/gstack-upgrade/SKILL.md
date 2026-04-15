---
name: gstack-upgrade
description: "Compatibility shim. Redirects to the upstream gstack upgrade command from the user's ~/gstack installation instead of updating GAL."
---

# /gstack-upgrade

Compatibility shim for upstream gstack maintenance.

## Role

Upgrade coordinator. If upstream gstack is installed on this machine, hand control to its own upgrade command instead of updating GAL here.

## When to Use

- When the user explicitly asks to upgrade upstream gstack
- When the machine has a `~/gstack` install and the user wants its command surface refreshed
- Periodically as part of machine maintenance

## What This Command Does

This command is not GAL's self-updater anymore. It is a compatibility shim that points the user at the upstream gstack install.

If upstream gstack is not installed at `~/gstack`, stop and tell the user to use GAL's own repo update workflow instead.

## Step 1 — Find Upstream gstack

Look for an upstream install at:
- `~/gstack/`
- `~/gstack/gstack-upgrade/`

If the install is missing: stop and tell the user that no upstream gstack install was found.

## Step 2 — Delegate To Upstream

Use the upstream `gstack-upgrade` command or its local command folder from the `~/gstack` install.

Do not invent a GAL-native upgrade procedure here.

If the host runtime needs a concrete path, use the upstream install under `~/gstack/` and follow its own upgrade instructions there.

## Step 3 — Verify

After the upstream upgrade completes, confirm that the upstream gstack command surface is available again.

If the user wanted to update GAL itself, point them to the GAL repo update workflow instead of using `/gstack-upgrade`.

## No Plan Artifacts

`/gstack-upgrade` does not write to plan files — it is a machine maintenance operation.
