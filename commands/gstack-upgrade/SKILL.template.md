---
name: gstack-upgrade
description: "Upgrade GAL to the latest version. Pulls the latest Golem-Agents-Legion repo and re-runs Setup-Machine.ps1 (Windows) or setup-machine.sh (macOS/Linux) to rebake all SKILL.md files from templates."
---

# /gstack-upgrade

Update GAL skills to the latest version.

## Role

Upgrade coordinator. Keep your installed skills current with the latest Golem-Agents-Legion release.

## When to Use

- When a new version of Golem-Agents-Legion is available
- When skills feel out of date or missing features
- Periodically as part of machine maintenance

## How GAL Upgrades Work

Unlike gstack (which has a binary installer), GAL upgrades work through Git:

1. Pull the latest `Golem-Agents-Legion` repository
2. Re-run the setup script to rebake SKILL.md files from templates

No binary to compile. The setup scripts handle all installation.

## Step 1 — Find Your GAL Installation

GAL is typically installed at:
- `~/Golem-Agents-Legion/` — default clone location
- Or wherever you cloned it during setup

If you're not sure where it is:
- Windows: `Get-ChildItem ~\Golem-Agents-Legion -ErrorAction SilentlyContinue`
- macOS/Linux: `ls ~/Golem-Agents-Legion`

## Step 2 — Pull Latest Changes

```bash
cd ~/Golem-Agents-Legion
git pull origin main
```

Check the git log to see what changed:
```bash
git log --oneline -10
```

## Step 3 — Re-Run Setup Script

**Windows (PowerShell):**
```powershell
cd ~/Golem-Agents-Legion
.\scripts\Setup-Machine.ps1
```

**macOS/Linux:**
```bash
cd ~/Golem-Agents-Legion
./scripts/setup-machine.sh
```

The setup script:
- Rebakes all SKILL.md files from SKILL.template.md files
- Updates VS Code / Copilot skill symlinks or copies
- Does not overwrite local customizations in `config.local.env`

## Step 4 — Verify

After setup completes, check that the updated skills are available:
- Reload VS Code (or the editor using GAL)
- Run `/gal status` to confirm the control plane is responsive

## What Changed

After pulling, check `CHANGELOG.md` or `git log --oneline` for new commands or behavior changes.

## No Plan Artifacts

`/gstack-upgrade` does not write to plan files — it is a machine maintenance operation.
