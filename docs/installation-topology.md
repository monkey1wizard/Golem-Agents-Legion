# Installation Topology

This document is a maintainer navigation map for GAL install work. It is not a second install specification; when behavior changes, update the owning source file listed below and keep this page directional.

## Quick Answer

- The currently supported hands-on path in this repo is still the contributor/source path: clone GAL, run `Setup-Machine.*`, then use `/gal init` from the target repository.
- The future end-user package path is defined by the bootstrap distribution contract, but package-manager install lanes are not shipped end to end in this repo yet.
- The machine-local runtime content owner is `~/.gal/plugins/gal/`. Provider-visible paths such as `~/.claude/skills/gal`, `~/.copilot/installed-plugins/gal-copilot/gal`, `~/.gemini/antigravity-cli/plugins/gal`, or `~/.gal/active/<provider>/` are projections or aliases.
- `Install-GalPlugins.* -Check` and `Setup-Machine.* -Check` are the read-only provider doctor entrypoints. They classify canonical content, projections, host-managed copies, and legacy GAL artifacts without mutating machine state.
- Bootstrap CLI installation and provider plugin installation are separate concerns. Do not fix one by widening ownership in the other.

## Owning Surfaces

| If you are changing... | Primary owner | Supporting files |
| --- | --- | --- |
| User-facing install copy | [../README.md](../README.md), [../README.zh-Hant.md](../README.zh-Hant.md) | [release-matrix.md](release-matrix.md) |
| Maintainer install navigation | [devguide.md](devguide.md), [../scripts/scripts.md](../scripts/scripts.md), this file | [personalization.md](personalization.md), [personalization.zh-Hant.md](personalization.zh-Hant.md) |
| Release channel lineage | [release-matrix.md](release-matrix.md) | [../README.md](../README.md) |
| Machine-local restore, mode switching, backup boundaries | [personalization.md](personalization.md), [personalization.zh-Hant.md](personalization.zh-Hant.md) | [devguide.md](devguide.md) |
| Full machine setup orchestration | [../scripts/Setup-Machine.ps1](../scripts/Setup-Machine.ps1), [../scripts/setup-machine.sh](../scripts/setup-machine.sh) | `Update-*` scripts |
| Install-mode provider orchestration | [../scripts/Install-GalPlugins.ps1](../scripts/Install-GalPlugins.ps1), [../scripts/install-gal-plugins.sh](../scripts/install-gal-plugins.sh) | `Resolve-GalCatalog.*`, `plugins/catalog.json` |
| Provider build plan and lifecycle status | [../scripts/Build-ProviderPlugins.ps1](../scripts/Build-ProviderPlugins.ps1), [../scripts/build-provider-plugins.sh](../scripts/build-provider-plugins.sh) | `scripts/common/ProviderPlugin.*` |
| Canonical plugin root rendering | [../scripts/Build-CorePlugin.ps1](../scripts/Build-CorePlugin.ps1), [../scripts/build-core-plugin.sh](../scripts/build-core-plugin.sh) | `agent/`, `skills/`, `commands/`, `mcp.json` |
| Repo initialization and generated adapters | [../scripts/Init-Repo.ps1](../scripts/Init-Repo.ps1), [../scripts/init-repo.sh](../scripts/init-repo.sh), [../scripts/Sync-DevContext.ps1](../scripts/Sync-DevContext.ps1), [../scripts/sync-dev-context.sh](../scripts/sync-dev-context.sh) | `.dev/project.md`, `.dev/state.md` |
| Public command surface | [../commands/commands.md](../commands/commands.md) | `commands/*/SKILL.template.md` |

## Runtime Flow

```text
Contributor/source path:
GAL checkout
  -> Setup-Machine.*
  -> Update-Personalization / Update-Skills / Update-Commands / Update-Mcp
  -> Install-GalPlugins.*
  -> Build-ProviderPlugins.*
  -> Build-CorePlugin.*
  -> ~/.gal/plugins/gal/ plus provider projections

Target repo bootstrap:
target repo cwd
  -> gal init
  -> Init-Repo.*
  -> Sync-DevContext.*
  -> .dev/ plus generated adapters

Future package-managed path:
winget / Homebrew / GitHub Release payload
  -> gal binary first launch
  -> install-mode runtime contract
  -> ~/.gal/ runtime roots and provider projections
```

## Ownership Boundaries

- `Setup-Machine.*` is the development and packaging harness. It should mirror the install-mode rules, but it is not the final end-user package payload.
- Package managers own only the package-managed `gal` binary payload.
- GAL owns rebuildable runtime outputs such as provider projections, generated MCP state, generated xmachine state, and the canonical plugin root.
- GAL also owns the per-provider ledgers under `~/.gal/dist/providers/<provider>/managed.json`; provider directories outside provable GAL-managed projections remain host-owned or user-owned.
- Users own `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, `~/.gal/state/plugins.lock.json`, explicit local overrides, and secret sources.
- `~/.gal/dist/` is package output, conversion output, managed metadata, or dev-mode isolation. It is not the runtime source of truth.
- Install mode must not depend on repo-root links, baked local checkout paths, or hidden `GAL_ROOT` assumptions.

## Before Editing Install

1. Decide which concern is changing: bootstrap payload, machine setup, provider plugin lifecycle, repo init, generated adapters, or documentation copy.
2. Read the primary owner from the table above, then read only the directly called scripts or docs for that concern.
3. Check source mode and install mode separately. A fix for contributor setup may be wrong for package-managed first launch.
4. Preserve user-owned machine intent during upgrade and default uninstall.
5. If the change affects public install wording, update English and Traditional Chinese docs together or explicitly mark the translation drift.
6. Run the narrowest install tests that cover the changed lane. For provider packaging, start with `Test-BuildProviderPlugins.ps1` and `Test-InstallGalPlugins.ps1`.

## Current Drift Queue

- [../scripts/scripts.md](../scripts/scripts.md) referenced this file before it existed; this page now closes that broken navigation link.
- [../README.zh-Hant.md](../README.zh-Hant.md) does not yet mirror the English README's `Install, Fallbacks, and Migration` section.
- [personalization.zh-Hant.md](personalization.zh-Hant.md) has duplicated install/source mode wording, and the later copy contains older provider-status language. Sync it from [personalization.md](personalization.md) before changing install status claims.
- Provider lifecycle status claims are split across docs and scripts. Before changing public claims for Claude, Copilot, or Codex, compare [../README.md](../README.md), [devguide.md](devguide.md), [../scripts/scripts.md](../scripts/scripts.md), [../scripts/Build-ProviderPlugins.ps1](../scripts/Build-ProviderPlugins.ps1), [../scripts/Install-GalPlugins.ps1](../scripts/Install-GalPlugins.ps1), and the latest verified plan [plans/feat-plugin-arch-migration.md](plans/feat-plugin-arch-migration.md).
