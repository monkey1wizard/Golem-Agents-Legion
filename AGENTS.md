# AGENTS.md

High-signal instructions for OpenCode sessions in this repo.

## First Principles

- `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, and `.github/copilot-instructions.md` are generated artifacts. Do not hand-edit expecting persistence; rerun generation scripts instead.
- Executable source of truth lives in `scripts/`, `commands/`, `skills/`, `conventions/`, `workflows/`, and `templates/`.
- This repo is mostly Markdown + shell/PowerShell contracts, not an app with a normal compile/lint/test pipeline.

## Regeneration Flow (Most Common Mistake)

- After changing shared workflow content (`conventions/`, `workflows/`, `model-roles.md`, `.dev/project.md`, or `skills/*` list behavior), regenerate adapters:
  - Windows: `./scripts/Sync-DevContext.ps1 -TargetPath <repo>`
  - macOS/Linux: `./scripts/sync-dev-context.sh <repo>`
- `gal init` also regenerates these files, but it also creates `.dev/` state and may run graphify auto-init.

## Command/Skill Packaging

- `commands/*/SKILL.md` is baked from `SKILL.template.md` (+ optional `SKILL.local.md`) by update scripts; edit templates, then rerun:
  - Windows: `./scripts/Update-Commands.ps1`
  - macOS/Linux: `./scripts/update-commands.sh`
- If you change install/link behavior, run full machine orchestration:
  - Windows: `./scripts/Setup-Machine.ps1`
  - macOS/Linux: `./scripts/setup-machine.sh`

## Protected/High-Impact Paths

- Treat these as architecture-critical: `commands/`, `conventions/`, `workflows/`, `templates/`.
- Treat these as critical operational scripts: `scripts/Sync-DevContext.ps1`, `scripts/sync-dev-context.sh`, `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh`.
- For cross-platform parity, when changing a `.ps1` flow, check/update its `.sh` counterpart (and vice versa).

## Git Safety + Personalization

- `.githooks/pre-commit` scans staged content for leaked `config.local.env` values and blocks commit on detection.
- `.gitattributes` applies `filter=gal-config` to adapter/instruction files; placeholders are smudged/cleaned via `scripts/gal-smudge.sh` and `scripts/gal-clean.sh`.
- Never commit real machine-local paths/secrets into tracked docs; keep placeholder model intact.

## Repo Verification Commands

- No single "test all" command exists. Use targeted script validation based on changed area:
  - Machine setup changes: run corresponding `Setup-Machine` or `update-*` script with dry-run flags where available.
  - xmachine lane changes: `./scripts/Test-Xmachine.ps1` (Windows lane) or `./scripts/Test-Xmachine.sh` (local-async lane).
  - Init/sync changes: rerun init/sync scripts in a throwaway repo and verify generated adapter outputs.

## Control Plane Facts

- Shell entrypoint is `gal <subcommand>` via `scripts/gal.ps1` / `scripts/gal.sh`.
- Script-dispatched subcommands are limited (`init`, `research`, `deep-research`, `pipeline`); status/next/wrap-up are chat skill surfaces, not rich local CLI implementations.
