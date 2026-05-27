# Developer Guide

This document is maintainer navigation, not a second specification. Use it to decide which layer you are changing, which source files own that layer, and which rules you must not break.

## Start By Finding The Right Layer

| If you are changing... | Ask first... | Read these source files |
| --- | --- | --- |
| `/gal` command surface, aliases, or dispatch | is this control-plane behavior or runtime plumbing? | [../commands/commands.md](../commands/commands.md), [../scripts/scripts.md](../scripts/scripts.md) |
| planning flow or optional collaborative-tool semantics | is this GAL-native planning, optional gstack behavior, or workflow teaching? | [../commands/commands.md](../commands/commands.md), [collaborative-tools/gstack.md](collaborative-tools/gstack.md), [../workflows/coding.md](../workflows/coding.md) |
| setup, install topology, baked command files, or MCP merge | is this machine-layer install or repo-layer adapter generation? | [../scripts/scripts.md](../scripts/scripts.md), `scripts/Setup-Machine.ps1`, `scripts/Update-*.ps1`, `scripts/setup-machine.sh`, `scripts/update-*.sh` |
| templates and plan lifecycle | which file should own this information? | [../templates/templates.md](../templates/templates.md), [../workflows/coding.md](../workflows/coding.md) |
| xmachine execution behavior | is this part of the main workflow or an execution-plane extension? | [collaborative-tools/xmachine.md](collaborative-tools/xmachine.md), xmachine scripts under `scripts/` |
| Godot or graphics workflows | is this repo-wide methodology or a module-specific lane? | [collaborative-tools/godot.md](collaborative-tools/godot.md), [collaborative-tools/graphworkflow.md](collaborative-tools/graphworkflow.md) |

If you cannot tell which layer you are touching, stop and resolve that first. Most broken refactors in GAL come from mixing README, docs, templates, scripts, and command contracts in one change.

## Non-Negotiable Rules

### 1. Markdown owns the durable contract

- Methodology, rules, and contracts live in tracked Markdown and source files.
- Generated adapters, baked command files, and runtime configs are outputs, not source inputs.

### 2. `/gal` only solves control-plane problems

- `/gal` should not wrap a second copy of tester, reviewer, designer, security, debugger, or releaser work.
- Execution-stage specialist behavior belongs in agents.
- Planning commands can run directly because they are still part of the public command surface.

### 3. Repo-local state is the ownership boundary

- `.dev/`, `docs/plans/`, `docs/designs/`, and similar repo-local files are the shared working state.
- Do not move GAL's core state back into user-global storage.

### 4. Missing tools must not look like success

- GAL uses skill-level routing, not one repo-wide CLI-first or MCP-first rule.
- Each external-tool skill should define a preferred path, a fallback path, and a no-tool behavior.

### 5. Do not optimize one runtime by breaking portability

- If a change makes Copilot, Antigravity, and Codex diverge in contract or file flow, it is usually the wrong change.
- README, docs, templates, and setup scripts should preserve cross-runtime parity first.

### 6. Navigation docs must not become a second spec

- If a document exists to help humans find the real source, keep it short and directional.
- Summaries are useful. Duplicate contracts are not.

### 7. Collaborative-tool routing belongs to the workflow layer

- Do not make an agent silently switch personas or contracts because a collaborative tool was detected.
- The workflow decides the collaborative tool first, then the tool writes back into the same repo-owned files.

The shared preflight model lives in [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md).

## Bootstrap Distribution Documentation Ownership

When you change bootstrap installer behavior or release policy, keep the document roles separate:

- `README.md` explains the user-facing install paths, manual fallback, backup and migration guidance, uninstall boundaries, lag expectations, and non-guarantees.
- `docs/release-matrix.md` owns the detailed canonical release lineage, marketplace matrix, submission rules, lag windows, and fallback copy.
- `docs/personalization.md` owns the machine-local restore boundary: what the user carries forward, what GAL regenerates, and how install mode versus source mode affect migration.

Do not duplicate the full release matrix in this guide. Point maintainers to the owning docs, then keep this file focused on layer boundaries and the rules that must not drift.

Bootstrap-distribution guardrails:

- GitHub Releases is the canonical version source.
- `winget`, `homebrew`, and provider marketplace entries must all map back to that same release lineage.
- Package-manager uninstall and GAL-managed uninstall must preserve user-owned machine intent.
- Provider marketplaces are discoverability-first unless a provider-native renderer and lifecycle have been verified end to end.
- Lag between downstream channels is expected; the user-facing fallback remains GitHub Releases.

## Runtime Topology For Setup Work

This section absorbs the setup topology that maintainers need when changing `Setup-Machine`, the `Update-*` scripts, command installation, or MCP wiring.

### Four Runtime Layers

| Layer | Location | Purpose |
| --- | --- | --- |
| Layer 1 | the GAL repo | main methodology source |
| Layer 1.5 | tool config directories such as `~/.copilot/`, `~/.gemini/`, `~/.gemini/antigravity-cli/`, `~/.codex/`, `~/.claude/`, plus `~/.gal/install-state.json` | installed skills, generated commands, runtime-facing symlinks, and machine-local runtime selection |
| Layer 2 | `<target-repo>/.dev/` | per-repo working context and state |
| Layer 3 | generated adapter files in the target repo | shared instructions and runtime-specific shims |

Naming note: upstream docs still use the full product name `Antigravity CLI` and the path segment `antigravity-cli`, but Google also exposes `AGY CLI` as the short name. In GAL-owned helper and function names, prefer `Agy` or `agy` for internal identifiers; keep `Antigravity CLI` and `antigravity-cli` for user-facing labels, runtime keys, and upstream-owned paths.

### Cross-Runtime Surface

| Runtime | Machine-layer install | Command surface | Notes |
| --- | --- | --- | --- |
| Copilot | `~/.copilot/agents/` and `~/.copilot/skills/` | installed command skills | supports custom agents and slash-command discovery |
| Antigravity CLI | `~/.gemini/antigravity-cli/plugins/gal/` (plugin-root) | installed named skills via plugin | primary Google CLI runtime; installs as a provider plugin at `~/.gemini/antigravity-cli/plugins/gal/` carrying skills, agents, rules, and MCP config as a self-contained tree; AGY is renderer 1 on the common package model, not the architecture itself |
| Gemini CLI | `~/.gemini/commands/`, `~/.gemini/gal-context.md`, `~/.gemini/settings.json`, and `~/.gemini/gal/` | generated native command files plus compatibility bridges | archived compatibility runtime; keep only the remaining surfaces listed below until AGY fully replaces them |
| Codex CLI | `~/.codex/skills/` and shared `~/.agents/skills/` | installed named skills | uses `$skill` invocation, not custom slash commands |
| Claude Code | `~/.claude/skills/`, `~/.claude/commands/`, and user-scope `claude mcp` config | generated command markdown plus repo-local `CLAUDE.md` | MCP install is managed through the Claude CLI |

### Layer 1.5 Install Topology

| Source in repo | Copilot target | Gemini target | Antigravity target | Codex target | Claude target |
| --- | --- | --- | --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | not installed | `~/.gemini/antigravity-cli/plugins/gal/agents/` | not installed | not installed |
| `skills/*/` | `~/.copilot/skills/` | imported from repo paths via `~/.gemini/gal-context.md` | `~/.gemini/antigravity-cli/plugins/gal/skills/` | `~/.agents/skills/` | `~/.claude/skills/` |
| `commands/*/` | `~/.copilot/skills/<command>/` | `~/.gemini/commands/<command>.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/<command>/` | `~/.codex/skills/<command>/` | `~/.claude/commands/<command>.md` |
| repo root | `~/.copilot/gal/` | `~/.gemini/gal/` | `~/.gemini/antigravity-cli/plugins/gal/` (plugin tree) | not required | not required |

### Generated Runtime Files

| Generated file | Why it exists |
| --- | --- |
| `commands/*/SKILL.md` | baked command prompt with absolute `GAL_ROOT` plus any gitignored `SKILL.local.md` overlay |
| `~/.gemini/commands/*.toml` | Gemini-native command surface generated from the baked command skill |
| `~/.claude/commands/*.md` | Claude-native command surface generated from the baked command skill |
| `~/.gemini/gal-context.md` | reusable shared skill imports for Gemini |

### Archived Gemini CLI Surfaces

These are the remaining Gemini CLI compatibility surfaces that still exist on purpose. Treat them as archived bridges to be retired gradually as AGY reaches parity. Do not expand them unless the change is explicitly about keeping Gemini compatibility working during that transition.

| Archived surface | Owning files | Why it still exists | Expected retirement path |
| --- | --- | --- | --- |
| Gemini runtime selection, path constants, and install-state detection | `scripts/common/Common.ps1`, `scripts/common/common.sh` | Setup still needs to detect and manage Gemini-specific compatibility outputs such as `~/.gemini/commands/`, `~/.gemini/settings.json`, `~/.gemini/gal-context.md`, and `~/.gemini/gal/`. | Remove once no GAL-managed Gemini install target remains. |
| Gemini native command generation | `scripts/Update-Commands.ps1`, `scripts/update-commands.sh` | GAL still bakes `commands/*/SKILL.md` into `~/.gemini/commands/*.toml` for the legacy Gemini native slash-command surface. | Replace when AGY skill or plugin surfaces are the only Google command entry point GAL supports. |
| Gemini shared-skill context bridge | `scripts/Update-Personalization.ps1`, `scripts/update-personalization.sh` | `~/.gemini/gal-context.md` still imports repo skills for Gemini compatibility. | Remove when Gemini no longer needs repo-skill imports for GAL. |
| Gemini settings.json bridge | `scripts/Update-Personalization.ps1`, `scripts/update-personalization.sh` | `~/.gemini/settings.json` still gets `AGENTS.md` and `GEMINI.md` in `context.fileName` for legacy Google-runtime loading. | Remove when Google-side loading is fully owned by AGY runtime surfaces instead of Gemini settings. |
| Gemini `GAL_ROOT` link and legacy skill cleanup | `scripts/Update-Skills.ps1`, `scripts/update-skills.sh` | GAL still manages `~/.gemini/gal/` and cleans old GAL-managed `~/.gemini/skills/*` remnants during migration. | Remove when no Gemini runtime path needs a stable repo link and no legacy cleanup is needed. |
| Legacy Gemini MCP cleanup | `scripts/Update-Mcp.ps1`, `scripts/update-mcp.sh` | Gemini is no longer the MCP owner, but GAL still removes old Gemini MCP entries from `~/.gemini/settings.json` so AGY MCP ownership stays clean. | Remove after legacy Gemini MCP residue no longer exists in supported installs. |
| `GEMINI.md` generated adapter filename | `scripts/Sync-DevContext.ps1`, `scripts/sync-dev-context.sh`, `scripts/Init-Repo.ps1`, `scripts/init-repo.sh` | The repo still emits `GEMINI.md` as a Google-runtime compatibility adapter filename even though AGY is the primary Google CLI runtime. | Rename or remove only when Google-runtime consumers no longer depend on the `GEMINI.md` carrier. |
| xmachine Gemini headless execution lane | `scripts/Start-xMachine.ps1`, `scripts/Start-xMachine.sh` | xmachine remote execution still invokes Gemini CLI headlessly and maps Gemini exit codes. | Replace when xmachine is migrated to AGY or another runtime end-to-end. |

If you are removing one of these archived surfaces, also audit the matching maintainer guidance in [../scripts/scripts.md](../scripts/scripts.md), [personalization.md](personalization.md), and [personalization.zh-Hant.md](personalization.zh-Hant.md) so the docs stop describing a retired bridge.

### Install-State

The installer persists machine-local runtime selection in `~/.gal/install-state.json`.

- `selectedRuntimes` controls which machine-layer targets GAL should manage.
- `primaryRuntime` controls defaults and summaries only.
- The tracked GAL repo remains the primary source for agents, skills, and commands.

### MCP Management

The MCP manifest is a separate install concern from skills.

| File | Scope | Role |
| --- | --- | --- |
| `mcp.json` | tracked | single GAL MCP source of truth |
| `mcp.local.json` | local only | machine-specific overrides and enablement |
| `config.local.env` | local only | secrets and local values referenced by the manifest |
| `xmachine.config.json` | local only | machine-local xmachine node definitions keyed by work-node alias |

The merged MCP manifest is centered on `servers` and may also include optional top-level `inputs` when a runtime supports prompt-backed values such as a PAT entry.

For Playwright MCP specifically:

- Keep `mcp.json` limited to the tracked safe startup contract: canonical `playwright` key plus conservative core flags such as `--isolated` and `--headless`.
- Put headed mode, viewport or device emulation, storage-state paths, output directories, optional capability flags, persistent profile paths, extension or CDP wiring, and similar machine-local behavior in `mcp.local.json`.
- Put secret-like paths or environment-backed local values referenced by those overrides in `config.local.env`.
- Do not track browser artifacts, storage-state files, persistent profile directories, or secret files in the repo.

`Update-Mcp.ps1` and `update-mcp.sh` use the tracked manifest as the source of truth for GAL-managed server names:

- VS Code: overwrite tracked server entries inside user `mcp.json`
- Antigravity CLI: write GAL-managed MCP to plugin-root `~/.gemini/antigravity-cli/plugins/gal/mcp_config.json` under `mcpServers`; the global `~/.gemini/antigravity-cli/mcp_config.json` is only touched for legacy cleanup of old GAL-managed entries
- JSON-based runtime bridges also preserve managed top-level `inputs` entries by input `id` when the merged manifest includes them.
- Codex CLI: regenerate tracked `[mcp_servers.*]` sections inside `config.toml`
- Claude Code: remove and re-add tracked user-scope servers through the `claude mcp` CLI

Provider-owned config still stays user-owned. GAL only takes ownership of the server names declared in the tracked manifest, preserves unrelated user-defined entries, and removes the GAL-managed legacy Gemini MCP names previously written into `settings.json`.

### Why `GAL_ROOT` Exists

`~/.copilot/gal/` and `~/.gemini/gal/` give installed command skills one stable path back to the source repo. That keeps generated command prompts small and deterministic.

For AGY, the plugin tree at `~/.gemini/antigravity-cli/plugins/gal/` replaces the old `~/.gemini/antigravity-cli/gal/` symlink as the managed install surface. The plugin is self-contained and does not require an external `GAL_ROOT` symlink; setup removes the legacy `GAL_ROOT` symlink during pre-cleanup.

## Provider Plugin Packaging

GAL uses a provider-neutral plugin package model. Source contracts in the repo are the single source of truth; each provider plugin is a generated artifact rendered by a provider-specific renderer.

### Common Package Model

The common package (`scripts/common/ProviderPlugin.ps1`, `scripts/common/provider-plugin.sh`) carries:

| Field | Source | Shared across all four providers? |
| --- | --- | --- |
| `metadata` | repo name, display name, version diagnostics, generation timestamp | conceptually yes |
| `skills` | `skills/<name>/SKILL.md` | yes |
| `commandSkills` | `commands/*/SKILL.md` | yes, as skill bundles |
| `mcpSpec` | `mcp.json` plus `mcp.local.json` boundary info | conceptually yes, but resolved local values stay out |
| `instructionCorpus` | `.dev/project.md`, required conventions, workflows, `model-roles.md`, generated indexes | content yes, path no |
| `agents` | `agent/*.agent.md` | optional; projected to three of four providers |

The common model explicitly excludes: provider-specific output paths, resolved machine-local secrets or paths, `runtimeScripts`, plugin-root `scripts/`, `gal-results/`, and hooks (deferred from v1).

### Provider Lane Policy

GAL targets are classified by provider-native install capability, not by a uniform renderer model.

| Lane | Platform | Role | Success criteria |
| --- | --- | --- | --- |
| Primary install target (canonical) | Claude Code | canonical schema / canonical renderer / installer target | `claude plugin install`, scope, update, uninstall via provider-native lifecycle |
| Primary install target (near-parity) | Copilot CLI | Claude-compatible structure; `agents/`, `skills/`, `hooks.json`, `.mcp.json`, `lsp.json` share the same directory conventions as Claude | `/plugin install`, marketplace, GitHub/Git URL/local path; directory layout already aligns with Claude, no shortcut needed |
| Primary install target | Codex | Claude baseline mapped renderer / marketplace target | `codex plugin install`, documented marketplace / cache install path, some components readable natively |
| Primary install target (shortcut) | AGY CLI / Antigravity CLI | Claude baseline mapped renderer + catalog-aware install target | provider-native plugin staging / install, or capability shortcut pointing to `~/.gal/active/agy/`; no repo-root shortcut dependency |
| Migration lane | Gemini CLI | legacy cleanup and compatibility only | not a fifth renderer; only cleanup or migration of existing GAL-managed Gemini surfaces |
| Bridge lane | OpenCode | deferred independent bridge plan | can read catalog/lockfile, mount `~/.gal/active/opencode/` capability shortcut, but does not promise primary install parity |
| Deferred / unsupported | other runtimes | out of scope | no documented install or skill discovery pathway |

Shortcut policy:

- Claude-compatible canonical package is the single source of truth.
- Copilot CLI can consume Claude-compatible structure natively and does not need a shortcut.
- AGY CLI and OpenCode, lacking native `plugin install` CLI, may use `~/.gal/active/<provider>/` as GAL-managed stable targets for capability-level shortcut redirection.
- Install mode forbids repo-root shortcuts, baked source paths, and hidden `GAL_ROOT` dependencies.
- Capability-level links in install mode must only point to `~/.gal/active/<provider>/` or its GAL-managed projection; source mode may point to user-specified local overrides.
- Bridge lane must not imply primary provider-native install parity to users.

### AGY Renderer (Implementation Status)

AGY is renderer 1, not the architecture. `Build-AgyPlugin` renders the common package into `dist/provider-plugins/agy/gal/` and installs to `~/.gemini/antigravity-cli/plugins/gal/`. The AGY plugin carries `plugin.json`, `skills/`, `agents/`, `rules/gal.md`, and `mcp_config.json`. It does not generate `hooks.json`, `scripts/`, marketplace metadata, provider stubs, or `gal-results/`.

Setup/reinstall removes all prior GAL-managed AGY content (legacy skills directory, `GAL_ROOT` symlink, global MCP entries, prior plugin installs) before installing the clean plugin tree.

Current implementation status:

- AGY is the only provider-native lifecycle slice implemented end to end in install mode today.
- Copilot CLI, Codex, and Claude Code still remain native-install target lanes in the architecture, but their concrete installer/update/uninstall flows are not implemented yet.
- Bootstrap packaging, official install channels, and marketplace discoverability do not belong to this install-mode surface; they are handled by the separate bootstrap-installer planning track.

### Future Renderer Sequence

After AGY validation, the planned renderer sequence is: Copilot CLI → Codex → Claude Code. Each will reuse the common base with its own layout and install lifecycle. No future renderer should copy the AGY layout.

## Distribution Architecture and Ownership Boundaries

GAL explicitly separates how the CLI is installed (Bootstrap Installer) from how provider plugins are managed (Install-Mode Plugin Distribution). This boundary ensures that package managers do not overwrite user data and that GAL plugins can update independently of the CLI payload.

### 1. Bootstrap Installer Distribution

- **What it is**: How the GAL CLI (`gal` executable) gets onto the user's machine.
- **Channels**: `winget` (Windows), `homebrew` (macOS/Linux), and GitHub Releases `.zip` / `.tar.gz` (manual fallbacks). See [docs/release-matrix.md](release-matrix.md) for the exact artifact lineage.
- **Ownership**: The package manager owns the **package-managed payload** (the single executable binary). It handles upgrades and removals of the CLI itself, but it must **never** manage or delete `~/.gal/` contents.

### Bootstrap Runtime Contract

After the package-managed `gal` binary is on disk, the first launch contract is:

1. Expose a runnable `gal` CLI from the package-managed install location.
2. Detect whether `~/.gal/config/config.json` already exists.
3. If no machine config exists, seed a minimal install-mode config and create the managed runtime roots under `~/.gal/`.
4. Resolve the default profile into `~/.gal/state/plugins.lock.json`.
5. Render provider-native projections under `~/.gal/generated/` and any required stable targets under `~/.gal/active/<provider>/`.
6. Hand off to install mode by default for bootstrap installs; source mode only begins after the user explicitly sets `installMode=source` plus `galRoot` and `devMode`.

The canonical end-user bootstrap contract must work without a cloned repo. Repo-local workflow state such as `.dev/`, `docs/plans/`, or repo-root skill links is never a first-launch requirement for the installed package payload. The repo-owned `Setup-Machine.*` scripts are the development and packaging harness that should mirror the same install-mode-first branching and `~/.gal/` ownership rules, but they are not themselves the final end-user package payload.

### First-Launch Branching Rules

The install-mode/source-mode split happens only after GAL has a machine config to read:

- **Bootstrap install with no existing machine config**: seed `installMode=install`, leave contributor-only repo bindings disabled, and build provider-native projections from `~/.gal/`.
- **Existing machine config with `installMode=install`**: reuse `~/.gal/` and refresh lockfile plus projections without creating repo-root links.
- **Existing machine config with `installMode=source`**: reuse the explicit `galRoot` and `devMode` settings, then allow contributor-only source links and local overrides.

This keeps package-managed first launch safe for end users while preserving an explicit contributor path.

### Upgrade Contract

Bootstrap upgrades must rerun the same install-mode-first contract without widening ownership.

- **`winget upgrade` / `brew upgrade`**: replace only the package-managed `gal` binary, then let the refreshed CLI re-enter the bootstrap runtime contract. The refresh may regenerate `~/.gal/generated/`, refresh provider projections, and update `~/.gal/state/plugins.lock.json` when the resolved profile changes, but it must not overwrite `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, explicit local overrides, or secret sources.
- **Provider-native direct-install or direct-update lanes**: if a marketplace lane is later verified to support canonical install and update, that lane still behaves like a bootstrap payload upgrade rather than a runtime reset. It may replace the packaged GAL payload and re-run GAL-managed projection refresh, but it must preserve user-owned config and keep install mode versus source mode unchanged unless the user edits machine config explicitly.
- **Manual archive refresh**: replacing the extracted `gal` binary from a GitHub Releases `.zip` or `.tar.gz` is allowed only as a payload swap. Users may rerun the bootstrap entrypoint afterward to refresh lockfile and generated projections, but manual archive updates must not delete or reset existing `~/.gal/config/*`, xmachine bindings, local overrides, or secrets.
- **Schema or runtime migrations**: when an upgrade needs to evolve GAL-managed runtime state, migrations must be additive or explicitly reversible. They may rewrite GAL-owned generated or cached state, but they must not silently migrate user-owned config into a new location or clear values that the user would need to reconstruct manually.

The decisive rule is simple: upgrades may refresh the payload, the lockfile, and GAL-managed generated state, but they must preserve the user's machine intent.

### Uninstall Contract

Default uninstall must stay narrower than a machine reset.

- **Package-manager uninstall**: removes only the package-managed `gal` payload. It must not delete `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, `~/.gal/state/plugins.lock.json`, explicit local overrides, or secret-bearing sources.
- **GAL-managed uninstall**: removes GAL-owned runtime outputs that can be rebuilt, including provider-native plugin install targets that GAL owns, `~/.gal/store/plugins`, and generated projections under `~/.gal/generated/mcp`, `~/.gal/generated/xmachine`, and `~/.gal/generated/providers`.
- **Preserved surfaces**: uninstall keeps the user's machine intent intact. That includes install/source mode choice, `galRoot`, `devMode`, xmachine bindings, lockfile state, explicit local overrides, and secret sources.
- **No implicit second runtime**: uninstall must not leave behind a second GAL-managed runtime tree that the next install would treat as authoritative. Rebuildable GAL-owned runtime outputs are removed; preserved user-owned config remains as input for the next install.

### Purge And Reset Boundary

Purge or reset is a separate destructive lane, not part of default uninstall.

- The explicit purge entrypoint is `Uninstall-Machine -Purge -ConfirmPurge` or `uninstall-machine.sh --purge --confirm-purge`, and it must remain opt-in, visible, and dry-runnable before destructive execution.
- Purge/reset may remove preserved machine-local intent such as `config.json`, `xmachine.json`, lockfile state, local overrides, install-state metadata, and secret-bearing generated surfaces, but only after an explicit destructive confirmation step.
- Neither package-manager uninstall nor default GAL-managed uninstall may simulate purge/reset by deleting preserved surfaces automatically.
- The practical rule is simple: uninstall removes what GAL can safely rebuild; purge/reset removes what the user would otherwise need to carry forward.

### 2. Install-Mode Plugin Distribution

- **What it is**: How provider-native plugins (like Claude, AGY, Copilot plugins) are resolved, installed, and updated.
- **Channels**: Provider-native marketplaces, driven by the `plugins/catalog.json` and resolved into `~/.gal/state/plugins.lock.json`.
- **Ownership**: GAL owns the plugin catalog and resolution. It manages provider-specific installation paths and the `~/.gal/` plugin store.

### 3. State Ownership Boundaries in `~/.gal/`

When a package manager uninstalls or upgrades the GAL CLI, it must respect these boundaries:

- **Package-Managed Payload**: The `gal` binary. Owned by `winget` / `homebrew`. Can be safely deleted during uninstall.
- **GAL-Managed Runtime and Generated State**: `~/.gal/store/plugins`, `~/.gal/generated/mcp`, `~/.gal/generated/xmachine`, `~/.gal/generated/providers`, and provider-native plugin installations that GAL explicitly owns. These can be safely regenerated or reinstalled if the CLI is rebuilt on a new machine. They are removed during a GAL-managed uninstall.
- **User-Owned Config and State**: `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, `~/.gal/state/plugins.lock.json`, explicit user-authored local overrides, and secrets. Owned by the user. Must be preserved during any package-manager uninstall or default GAL-managed uninstall. Full deletion requires an explicit, destructive purge flow.

During upgrade, treat `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, explicit local overrides, and secret-bearing sources as read-preserve surfaces. The installer may read them to determine install mode, provider selection, or migration steps, but it must not replace them with defaults merely because a newer bootstrap payload was installed.

## Install Mode vs Source Mode

GAL uses two distinct operational modes.

| Mode | Audience | `gal-core` source | External plugin source | Shortcut policy |
| --- | --- | --- | --- | --- |
| Install mode | general users | Claude-compatible canonical package + provider-native install source | `~/.gal/config/config.json` + `~/.gal/state/plugins.lock.json` resolved upstream packages | provider-specific shortcuts only to `~/.gal/active/<provider>/`; repo-root shortcut forbidden |
| Source mode | GAL contributors | `~/.gal/config/config.json.galRoot` pointing to local GAL repo | `~/.gal/state/plugins.lock.json` resolved cache, or `~/.gal/config/xmachine.json` explicit local override | repo link allowed but must be marked as source mode |
| Migration cleanup | existing GAL users | existing managed surfaces | existing managed external skills | only removes GAL-managed legacy links/cache, never deletes user-owned config |
| Bridge/degraded lane | OpenCode or runtimes lacking primary install parity | GAL-managed cache/artifact | resolved package subset | only capability-level links; not treated as primary install success |

Key rules:

- `galRoot` and `devMode` are controlled by `~/.gal/config/config.json`.
- Xmachine routing is controlled by `~/.gal/config/xmachine.json`.
- `GAL_SKILLS` is no longer part of the config surface; existing values are only migration input.
- `context7ApiKey`, once rendered into `~/.gal/generated/mcp/managed.json`, is machine-local secret-bearing state — never tracked or shared.

## Plugin Support Tiers

| Tier | Content source | GAL responsibility | Update strategy |
| --- | --- | --- | --- |
| `official-gal` | GAL repo / GAL release artifact | GAL maintains content, testing, installation, and regression | updated directly by GAL releases |
| `curated-upstream` | external upstream (e.g. `dart-lang/skills`) | GAL verifies metadata, provider compatibility, default profile, and lockfile; content maintained by upstream | updated per lockfile pin; manual or controlled updates allowed |
| `mirrored` | managed mirror of external upstream | GAL responsible for provenance, license, checksum, and mirror drift | no unversioned copies; updates require drift check |
| `forked` | fork maintained by GAL or user | fork owner responsible for divergence and fixes | must record fork base, diff policy, and update strategy |
| `local` | `file://` or local path override | source mode / contributor override only | does not enter shareable lockfile; recorded as machine-local override |

Default profile: initial `default` profile installs only `gal-core`. All companion plugins are opt-in through named profiles or explicit plugin selection.

## Catalog and Lockfile Architecture

GAL is a catalog + lockfile orchestrator, not a universal plugin runtime.

```text
plugins/catalog.json
  → ~/.gal/state/plugins.lock.json
  → resolved plugin set
  → provider-native install spec
  → Claude-compatible canonical package
  → provider-specific installer / renderer / shortcut mapping
```

Core layout under `~/.gal/`:

| Path | Purpose |
| --- | --- |
| `~/.gal/config/config.json` | user-managed machine config: personalization, plugin/profile/provider selections, `galRoot`, `devMode` |
| `~/.gal/config/xmachine.json` | machine-local xmachine binding: node aliases, machine profiles, local overrides |
| `~/.gal/state/plugins.lock.json` | resolved lockfile: installed sources, versions, checksums, component maps |
| `~/.gal/store/plugins/` | GAL-managed store of canonical packages and companion plugins |
| `~/.gal/generated/mcp/managed.json` | GAL-produced MCP projection, replaces repo-root `mcp.local.json` in install mode |
| `~/.gal/generated/xmachine/managed.json` | GAL-produced xmachine projection |
| `~/.gal/generated/providers/` | GAL-produced provider config projections |
| `~/.gal/active/<provider>/` | stable shortcut targets for AI tools; consumers do not point directly at store paths |

The `plugins/catalog.json` in the repo is the authoritative catalog source and metadata registry. The `~/.gal/state/plugins.lock.json` is the deterministic machine-local resolution that can be backed up, transferred, and re-resolved.

Initial external companion candidates: `dart-lang/skills`, `flutter/skills`, `dotnet/skills`, `anthropics/skills`, `samber/cc-skills-golang`, `twostraws/swift-agent-skills`, `kepano/obsidian-skills`, `actionbook/rust-skills` — all `curated-upstream`, all opt-in.

## Runtime File Schemas

This section defines the structure, required fields, optional fields, secret boundaries, precedence rules, and drift metadata for the four authoritative `~/.gal/` runtime files.

### `~/.gal/config/config.json` — User-Managed Machine Config

Concentrates personalization, plugin/profile/provider selections, install/source mode, and `galRoot`/`devMode` in one user-owned file. This file is designed to be backed up and transferred between machines.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | yes | integer | schema version for migration |
| `galRoot` | source-mode only | string | absolute path to local GAL repo clone |
| `devMode` | no | boolean | enables contributor tooling; defaults false |
| `obsidianVault` | no | string | absolute path to Obsidian vault |
| `obsidianVaultName` | no | string | display name of the vault |
| `obsidianGuidePath` | no | string | vault-relative path to personal Guide |
| `obsidianGuideMode` | no | string | `auto`, `guide`, or `generic` |
| `obsidianPrivateResearchDir` | no | string | vault-relative private research directory |
| `obsidianDiaryDir` | no | string | vault-relative work diary directory |
| `obsidianScratchDir` | no | string | vault-relative scratch log directory |
| `obsidianArchiveDir` | no | string | vault-relative diary archive directory |
| `researchDefaultDest` | no | string | `repo`, `private`, `knowledge`, or `none` |
| `localSearchProject` | no | string | clone path for local search project |
| `tempDir` | no | string | temp output directory |
| `mcpMemoryFilePath` | no | string | path to persistent MCP memory JSON file |
| `mcpFilesystemPaths` | no | array | optional compatibility field; only output when filesystem MCP is present |
| `context7ApiKey` | no | string | **SECRET-BEARING** — do not share; materialized into `mcp/managed.json` |
| `workingHoursEnabled` | no | boolean | enables working-hours enforcement |
| `workdayStart` | no | string | `HH:MM` format |
| `workdayEnd` | no | string | `HH:MM` format |
| `wrapUpTime` | no | string | `HH:MM` format |
| `hardStopTime` | no | string | `HH:MM` format |
| `defaultProfile` | no | string | default profile name; defaults to `default` |
| `profiles` | no | object | map of profile name → plugin list |
| `enabledPlugins` | no | array | explicit plugin selections |
| `disabledPlugins` | no | array | explicit plugin exclusions |
| `providerSelections` | no | object | per-provider enablement |
| `installMode` | no | string | `install` or `source` |
| `updateChannel` | no | string | update policy |
| `allowAutoUpdate` | no | boolean | per-plugin auto-update override |
| `preferredProviders` | no | array | ordered provider preference |
| `userSettings` | no | object | free-form user extensions |

**Secret boundary**: `context7ApiKey` is the only secret-bearing field. Once materialized into `~/.gal/generated/mcp/managed.json`, that generated file becomes machine-local secret-bearing state — never commit or share it.

**Precedence**: `config.json` is the user-facing input. Profile selections, explicit enabled/disabled lists, and provider selections are resolved together. Explicit `enabledPlugins`/`disabledPlugins` override profile-level settings.

**Excluded fields**: `GAL_SKILLS` is intentionally removed. Existing values are migration input only and must not appear in the final schema.

### `~/.gal/state/plugins.lock.json` — Resolved Lockfile

Deterministic machine-local resolution produced by the resolver from `plugins/catalog.json` and `~/.gal/config/config.json`. Designed to be backed up, transferred, and re-resolved.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | yes | integer | lockfile schema version |
| `resolverVersion` | yes | string | resolver tool version that produced this lock |
| `lockTimestamp` | yes | string | ISO 8601 timestamp |
| `plugins` | yes | array | resolved plugin entries |

Each resolved plugin entry carries:

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `pluginId` | yes | string | matches catalog `pluginId` |
| `resolvedSource` | yes | object | `{type, repo, ref, path}` |
| `resolvedVersion` | yes | string | release tag, version, or commit SHA |
| `resolvedChecksum` | yes | string | actual checksum of resolved content |
| `resolvedLicense` | yes | string | confirmed license identifier |
| `resolvedComponentMap` | yes | object | confirmed available components |
| `selectedProviders` | yes | array | providers this plugin is active for |
| `selectedProfiles` | yes | array | profiles that selected this plugin |
| `installTimestamp` | yes | string | ISO 8601 when resolved and installed |

**Drift detection metadata**: each plugin entry includes `resolvedChecksum` compared against catalog `checksumPolicy`. When the lockfile checksum differs from the catalog policy's expected value, the resolver must flag drift. Local overrides (source mode only) are recorded in `~/.gal/config/xmachine.json`, not in the lockfile.

### `~/.gal/config/xmachine.json` — Machine-Local Xmachine Binding

Machine-local binding file for xmachine routing. Not a team-shared configuration.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | yes | integer | binding schema version |
| `defaultXmachineNode` | no | string | default work node alias |
| `xmachineNodeAliases` | no | object | map of alias → `{target, repoPath, runtimeRepoPath}` |
| `machineProfiles` | no | object | machine-specific profile overrides |
| `localPluginPaths` | no | array | source-mode local plugin path overrides |
| `providerPathOverrides` | no | object | per-provider path mapping |
| `additionalBindings` | no | object | free-form extension bindings |

**Secret boundary**: `xmachine.json` may contain SSH targets and repo paths. Treat as machine-local and do not share.

### `~/.gal/generated/mcp/managed.json` — GAL-Produced MCP Projection

Generated file owned by GAL that replaces repo-root `mcp.local.json` in install mode. Rendered from `mcp.json` + `mcp.local.json` boundary info with machine-local values resolved.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | yes | integer | generated schema version |
| `generatedAt` | yes | string | ISO 8601 generation timestamp |
| `generatedBy` | yes | string | tool version that produced this file |
| `mcpServers` | yes | object | resolved MCP server configurations |
| `inputs` | no | array | resolved prompt-backed inputs |
| `_metadata` | yes | object | generation metadata |

**Server entry shape** (per server):

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `command` | conditional | string | CLI command (absent when `serverUrl` present) |
| `args` | conditional | array | CLI args (absent when `serverUrl` present) |
| `serverUrl` | conditional | string | HTTP MCP endpoint (absent when `command` present) |
| `env` | no | object | environment variables (resolved, no placeholders) |
| `headers` | no | object | HTTP headers (resolved, no placeholders) |

**Secret boundary**: `context7ApiKey` is materialized directly into `headers.CONTEXT7_API_KEY` — no `${CONTEXT7_API_KEY}` placeholder remains. This file is **secret-bearing machine-local state**. It must never enter a tracked repo, shared lockfile, or team configuration. Mark it in `.gitignore`.

**Resolved values rules**:

- `mcpMemoryFilePath` is resolved to its absolute value.
- `mcpFilesystemPaths` appears only when filesystem MCP is present in the resolved set; otherwise omitted.
- No runtime placeholder resolution is required — all values are fully materialized.

### Schema Precedence Chain

```text
plugins/catalog.json (repo-tracked, authoritative catalog source)
  ↓ resolved with
~/.gal/config/config.json (user-owned, machine-local preferences)
  ↓ produces
~/.gal/state/plugins.lock.json (machine-local, deterministic, backup-safe)
  ↓ renders into
~/.gal/generated/mcp/managed.json (machine-local, secret-bearing, never shared)
~/.gal/generated/xmachine/managed.json (machine-local, non-secret)
~/.gal/generated/providers/ (machine-local provider projections)
```

`~/.gal/config/xmachine.json` is an independent leaf — it does not feed into the resolver chain but controls xmachine routing and local overrides for source mode.

## Common Change Entry Points

### Changing `/gal` or alias behavior

1. Read [../commands/commands.md](../commands/commands.md).
2. Check whether the change is contract-level behavior or only install/runtime presentation.
3. If it affects generated command files, inspect the setup scripts and the relevant `commands/*/SKILL.template.md`.

### Adding or changing a planning command

1. Place it in the right family via [../commands/commands.md](../commands/commands.md).
2. Update the owning prompt in `commands/<command>/SKILL.template.md`.
3. Confirm the write-back target fits the existing plan sections and workflow state machine.
4. If it changes optional collaborative-tool semantics, also update [collaborative-tools/gstack.md](collaborative-tools/gstack.md) and [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) when shared preflight behavior changes.

### Adding or changing an execution specialist

1. Update the owning prompt in `agent/<golem>.agent.md`.
2. Confirm the write-back target fits the existing plan sections and workflow lifecycle.
3. Update [../agent/agents.md](../agent/agents.md), [../commands/commands.md](../commands/commands.md), and any README sections that route users to that specialist.
4. Do not reintroduce the behavior as a standalone public command unless it is truly control-plane or planning work.

### Changing setup, installation, or MCP merge

1. Read [../scripts/scripts.md](../scripts/scripts.md).
2. Decide which concern owns the change first: `Update-Personalization`, `Update-Skills`, `Update-Commands`, `Update-Mcp`, or the top-level orchestrator.
3. Windows and macOS/Linux both use the split Setup-Machine plus concern-script stack. Keep the two entrypoint families aligned unless the change is intentionally platform-specific.
4. Check whether `commands/commands.md` should also change because the user-visible runtime surface changed.
5. Keep README focused on entry points, keep setup plumbing here and in the source scripts.

### Refreshing MCP vs. Regenerating Adapters

- Run `Update-Mcp.ps1` or `update-mcp.sh` after changing `mcp.json`, `mcp.local.json`, or MCP-related values in `config.local.env`. This refreshes runtime MCP config only.
- Run `Sync-DevContext.ps1` or `sync-dev-context.sh` after changing source-of-truth content that should regenerate repo-local adapters such as `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md`.
- Run `Setup-Machine.ps1` or `setup-machine.sh` when you need the full concern stack refreshed in one pass.

### Refactoring docs themselves

1. Make sure each doc has one clear job.
2. If another source file already owns the contract, summarize it and link out instead of copying it.
3. If you remove content from one reader entry point, give it a clear new landing page.

## Adding A New CLI Runtime

1. Decide whether the CLI has a machine-layer config directory that GAL can target.
2. Decide whether its repo-facing instruction file can reuse `AGENTS.md` or needs another generated adapter.
3. If the runtime supports native commands, generate them from the same shared command templates instead of building a second workflow source. If it does not, install the same baked command skills into the runtime's supported skill surface.
4. Add any config-merge bridge only if the runtime has a stable, user-owned config file that can safely accept additive changes.

## Verify Setup Changes

After changing install or setup logic, verify at least these points:

- the stable repo symlink exists for each supported runtime that needs one
- generated `commands/*/SKILL.md` files no longer contain `{{GAL_ROOT}}`
- Gemini native command files were regenerated from the baked command content
- Antigravity installed skills and agents resolve through `~/.gemini/antigravity-cli/plugins/gal/`
- AGY plugin-root `mcp_config.json` is the sole GAL-managed MCP source for AGY; no GAL-managed MCP entries remain in the global `mcp_config.json` or `settings.json`
- shared skill directories contain reusable skills only, not duplicated command aliases
- MCP reruns update tracked server entries correctly without clobbering unrelated provider-owned config

## Where Information Belongs

| Information type | Right home |
| --- | --- |
| durable methodology and contracts | tracked source docs and source files |
| repo working context | `.dev/project.md` and `.dev/state.md` in the target repo |
| human-readable feature plan | `docs/plans/<plan-slug>.md` |
| machine-readable execution work file | `.dev/plans/<plan-slug>.prompt.md` |
| temporary session continuity | `### Handoff Notes` plus `.dev/state.md` |
| machine-local xmachine node config | `xmachine.config.json` |

If a completed plan contains knowledge that should survive, extract it back into a durable source file instead of leaving the plan as hidden long-term documentation.

## Self-Check

Before you finish a maintainer change, ask:

- Did I create a second source of truth?
- Does `/gal` still only solve control-plane problems?
- Do specialist commands still write back to repo-owned files?
- Did I accidentally move state back into a user-global path?
- Can a missing tool still fail loudly instead of pretending to succeed?
- Did I keep collaborative-tool routing at the workflow layer?
- Do README, `commands/commands.md`, and this guide still have distinct jobs?

## Suggested Reading Order

| Reader | Suggested order |
| --- | --- |
| first-time GAL maintainer | this guide → [../commands/commands.md](../commands/commands.md) → [../scripts/scripts.md](../scripts/scripts.md) |
| maintainer changing command behavior | [../commands/commands.md](../commands/commands.md) → [../scripts/scripts.md](../scripts/scripts.md) |
| maintainer changing setup | this guide → [../scripts/scripts.md](../scripts/scripts.md) |
| maintainer changing workflow semantics | [../commands/commands.md](../commands/commands.md) → [collaborative-tools/gstack.md](collaborative-tools/gstack.md) → [../workflows/coding.md](../workflows/coding.md) |

## Related Files

- [../README.md](../README.md) for the primary user entry point.
- [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) for shared collaborative-tool preflight behavior.
- [collaborative-tools/gstack.md](collaborative-tools/gstack.md) for optional collaborative-tool behavior.
- [../commands/commands.md](../commands/commands.md) for the control-plane contract and runtime surface.
- [../scripts/scripts.md](../scripts/scripts.md) for the script inventory and setup behavior.
- [../templates/templates.md](../templates/templates.md) for template ownership.

## Token Discipline

These rules apply to all maintainer and agent work in this repo. The full policy lives in [../conventions/token-budget.md](../conventions/token-budget.md). The developer-facing summary is here.

### Generated-Artifact Exclusion

Do not read generated adapters (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, `.github/copilot-instructions.md`) or build outputs (`bin/`, `obj/`) unless the current task is explicitly about auditing those generated files. They are large, frequently regenerated, and contain no information not already in their source templates.

### Directed Exploration

Before reading any file, confirm it is named in the current task or is a direct dependency of a task-named file. Stop reading when you have the information needed. Do not load the full codebase as a cold-start step.

### Failure-Focused Output

When running builds or tests, emit:

- Build: first error with file and line reference. On success, one summary line only.
- Tests: failing test names and assertion messages only. Do not echo passing test names.
- Lint: files and rule violations only. On a clean pass, one summary line only.

Store full logs on disk when needed; retrieve specific lines selectively rather than piping entire logs into context.

### Context-Pressure Recovery

When context is near the limit during an active task:

1. Write the current task name, last completed step, and any key decisions to `### Handoff Notes` in the active plan's `## Status` section.
2. Write or update the matching plan row in `.dev/state.md` `## Session Continuity` with `Stopped At` and `Next Step`.
3. Do **not** create a separate `CONTEXT.md` file — the plan and state files are the only durable session state stores.

This ensures the next session can resume without re-deriving context.
