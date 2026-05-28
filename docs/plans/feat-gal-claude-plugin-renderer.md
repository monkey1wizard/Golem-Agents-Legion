# Plan: GAL Claude Plugin Renderer

## Goal

GAL installs into Claude Code as a real Claude plugin package, not as legacy user-scope `~/.claude/skills` and `~/.claude/commands` projections. The Claude artifact must render to a plugin root such as `dist/provider-plugins/claude/gal/`, with `.claude-plugin/plugin.json`, `skills/`, `commands/`, `agents/`, and `.mcp.json` laid out according to the Claude Code plugin contract documented in [Claude Code Plugins](https://code.claude.com/docs/en/plugins).

The install flow must make that plugin available through Claude's plugin lifecycle when the local Claude CLI supports it, and must not claim verified direct install until `claude plugin validate` and install/update/uninstall checks pass.

## Requirements

- [ ] Render GAL as a Claude plugin root, not as loose entries under `~/.claude/skills` or `~/.claude/commands`.
- [ ] Generate `.claude-plugin/plugin.json` at the plugin root, with plugin metadata sufficient for Claude validation and plugin manager display.
- [ ] Generate `skills/<name>/SKILL.md` under the plugin root for reusable GAL skills.
- [ ] Generate command markdown under plugin-root `commands/`, preserving the current GAL command behavior without requiring global `~/.claude/commands` files.
- [ ] Generate Claude-compatible agent definitions under plugin-root `agents/` when the source agent can be represented safely.
- [ ] Generate plugin-root `.mcp.json` from the existing GAL MCP source, preserving user-owned secret and local override boundaries.
- [ ] Add a Claude renderer to provider plugin build orchestration so the Claude lane is no longer `not-yet-implemented` after verification.
- [ ] Update install, update, uninstall, and purge flows to manage the Claude plugin surface and clean old GAL-managed legacy projections.
- [ ] Use a single Claude plugin architecture everywhere. Source-mode workflows may invoke builders from the repo, but Claude consumption must still go through the plugin-shaped surface and must not retain any legacy projection path.
- [ ] Update shared setup context, runtime detection, and install-state logic so Claude is identified from the plugin surface and install lifecycle rather than legacy `~/.claude/skills` or `~/.claude/commands` probes.
- [ ] Remove Claude legacy projection behavior from both PowerShell and POSIX concern scripts; source mode must not keep a parallel GAL-managed loose Claude path.
- [ ] Restrict plugin-root `.mcp.json` to portable, non-secret GAL-managed entries; secrets, tokens, machine-local paths, and user overrides must remain outside the rendered plugin artifact.
- [ ] Keep Claude user-scope MCP registration behavior distinct from plugin-artifact rendering; provider CLI bridge commands may be used for optional smoke/install lanes, but they must not be treated as the rendered plugin `.mcp.json` source of truth.
- [ ] Normalize or drop non-Claude-compatible agent frontmatter during rendering so emitted `agents/` entries are valid Claude plugin agents without manual follow-up.
- [ ] Bundle the canonical GAL skill set selected by package inputs and lockfile resolution into the Claude plugin artifact; companion-skill uncertainty must not block plugin rendering.
- [ ] Update tests so Claude plugin structure, validation readiness, and legacy projection cleanup are executable checks.
- [ ] Update documentation only after implementation status is true; do not claim direct-install support before lifecycle validation passes.

## Approach

### Step 1: Define the Claude renderer output contract

- **Files**: `scripts/common/ProviderPlugin.ps1`, `scripts/common/provider-plugin.sh`, `docs/devguide.md`
- **What**: Define the exact Claude plugin artifact root and install target semantics. The plugin root contains `.claude-plugin/plugin.json`, `skills/`, `commands/`, `agents/`, and `.mcp.json`. Only `plugin.json` belongs inside `.claude-plugin/`.
- **Verify**: A documented output tree can be checked without relying on `~/.claude/skills` or `~/.claude/commands`.

### Step 2: Implement the Claude plugin renderer

- **Files**: `scripts/Build-ClaudePlugin.ps1`, `scripts/build-claude-plugin.sh`, `scripts/common/ProviderPlugin.ps1`, `scripts/common/provider-plugin.sh`
- **What**: Create a renderer parallel to the AGY renderer, reusing the canonical provider package model while emitting Claude-specific structure. The renderer writes to `dist/provider-plugins/claude/gal/` and supports a forced rebuild.
- **Verify**: Running the renderer creates the expected plugin root with manifest, skills, commands, agents, and MCP config.

### Step 3: Wire Claude into provider build orchestration

- **Files**: `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`
- **What**: Replace the Claude `not-yet-implemented` renderer entry with the new Claude renderer. Return artifact root, install target, and lifecycle status in the build plan.
- **Verify**: `Build-ProviderPlugins` can build only the Claude provider and reports a real artifact root.

### Step 4: Move all Claude lifecycle handling to the plugin path

- **Files**: `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh`
- **What**: Install, update, and uninstall the Claude plugin artifact through the provider-native path when available. If the local Claude CLI supports `claude plugin validate` and local plugin install, use those commands. If only session loading is available, expose that as a smoke-only lane and do not leave a parallel legacy Claude path in place.
- **Verify**: Install, update, uninstall, and purge checks can distinguish a verified Claude plugin install from a built artifact or session-only load.

### Step 5: Delete legacy Claude projection architecture

- **Files**: `scripts/Update-Skills.ps1`, `scripts/update-skills.sh`, `scripts/Update-Commands.ps1`, `scripts/update-commands.sh`, `scripts/Update-Mcp.ps1`, `scripts/update-mcp.sh`, `scripts/common/Common.ps1`, `scripts/common/common.sh`, `scripts/Uninstall-Machine.ps1`
- **What**: Follow Claude's documented migration path by rendering GAL into the plugin root, moving Claude-facing commands, skills, agents, and portable MCP config into the plugin artifact, and then removing GAL-managed legacy `.claude/skills` and `.claude/commands` entries plus any shared runtime detection or uninstall logic that still treats those legacy paths as the supported Claude surface.
- **Verify**: Claude usage leaves GAL content in the plugin surface only, the old projection architecture is removed after migration to avoid duplicate load paths, and source/install/uninstall flows no longer infer Claude state from loose projection files.

### Step 6: Add focused tests and validation commands

- **Files**: `scripts/Test-BuildProviderPlugins.ps1`, `scripts/Test-InstallGalPlugins.ps1`, `scripts/Test-ProviderPluginPackage.ps1`
- **What**: Add tests for Claude artifact shape, manifest presence, skills, commands, agents, MCP output, build plan wiring, uninstall cleanup, and legacy projection absence. Include optional `claude plugin validate` smoke when the CLI supports it.
- **Verify**: Focused test scripts fail if Claude falls back to `not-yet-implemented` or writes only legacy projection files.

### Step 7: Update docs and release matrix after verification

- **Files**: `docs/devguide.md`, `docs/release-matrix.md`, `docs/personalization.md`, `docs/plans/feat-gal-bootstrap-installer-distribution.md`
- **What**: Replace stale wording that says Claude is only discoverability-first or `not-yet-implemented`, but only after the renderer and lifecycle checks are real. Document any remaining limitations, such as local CLI support for install versus validate-only smoke.
- **Verify**: Documentation matches observed install behavior and does not overclaim marketplace or direct-install status.

## Files to Create or Modify

- `scripts/Build-ClaudePlugin.ps1` — create the Windows Claude plugin renderer.
- `scripts/build-claude-plugin.sh` — create the POSIX Claude plugin renderer.
- `scripts/common/ProviderPlugin.ps1` — add Claude artifact/install path helpers and reusable renderer helpers if needed.
- `scripts/common/provider-plugin.sh` — add shell-side Claude helper parity.
- `scripts/Build-ProviderPlugins.ps1` — wire the Claude provider lane to the renderer.
- `scripts/build-provider-plugins.sh` — wire the POSIX Claude provider lane to the renderer.
- `scripts/Install-GalPlugins.ps1` — add Claude plugin install/update/uninstall behavior.
- `scripts/install-gal-plugins.sh` — add POSIX Claude plugin install/update/uninstall behavior.
- `scripts/Setup-Machine.ps1` — remove legacy Claude projection behavior and install the Claude plugin surface instead.
- `scripts/setup-machine.sh` — keep POSIX setup behavior aligned with the plugin-only Claude architecture.
- `scripts/common/Common.ps1` — remove Claude legacy runtime detection/install-state assumptions and track Claude via plugin lifecycle state.
- `scripts/common/common.sh` — keep shell-side runtime detection/install-state logic aligned with the plugin-only Claude architecture.
- `scripts/Update-Skills.ps1` — delete Claude projection logic and keep only non-Claude update behavior that still applies.
- `scripts/update-skills.sh` — delete POSIX Claude projection logic and keep only non-Claude update behavior that still applies.
- `scripts/Update-Commands.ps1` — delete Claude projection logic and keep only non-Claude update behavior that still applies.
- `scripts/update-commands.sh` — delete POSIX Claude projection logic and keep only non-Claude update behavior that still applies.
- `scripts/Update-Mcp.ps1` — route Claude plugin MCP into plugin-root `.mcp.json` using only portable non-secret GAL-managed entries and keep any optional Claude CLI MCP bridge behavior separate.
- `scripts/update-mcp.sh` — keep POSIX MCP projection behavior aligned with the plugin-root `.mcp.json` contract and legacy Claude cleanup.
- `scripts/Uninstall-Machine.ps1` — update uninstall/purge documentation and cleanup semantics so Claude plugin surfaces replace legacy loose-path assumptions.
- `scripts/Test-BuildProviderPlugins.ps1` — validate Claude renderer and artifact shape.
- `scripts/Test-InstallGalPlugins.ps1` — validate install/uninstall cleanup and legacy projection absence.
- `docs/devguide.md` — update implementation status and runtime topology after verification.
- `docs/release-matrix.md` — update Claude direct-install eligibility after verification.
- `docs/personalization.md` — document machine-local Claude plugin state and migration boundaries.

## Test Cases

- [ ] `Build-ProviderPlugins.ps1 -Providers claude -Force` creates `dist/provider-plugins/claude/gal/`.
- [ ] Claude artifact contains `.claude-plugin/plugin.json`.
- [ ] Claude artifact contains `skills/<name>/SKILL.md` for GAL-managed skills included in the canonical package.
- [ ] Claude artifact contains `commands/*.md` for GAL control-plane commands.
- [ ] Claude artifact contains `agents/*.md` for supported golem agents.
- [ ] Claude artifact contains `.mcp.json` when MCP projection is enabled.
- [ ] Claude build plan no longer reports `Renderer = not-yet-implemented` after implementation.
- [ ] `claude plugin validate <artifact> --strict` passes when the local Claude CLI supports validation.
- [ ] Claude source mode and install flow do not create GAL-managed loose entries under `~/.claude/skills` or `~/.claude/commands`, and the old Claude projection path is absent from the supported flow.
- [ ] Claude runtime detection, setup state, and uninstall logic no longer rely on GAL-managed loose entries under `~/.claude/skills` or `~/.claude/commands` to decide whether Claude is installed.
- [ ] Uninstall removes the GAL-managed Claude plugin artifact or provider-native install entry while preserving user-owned Claude settings.
- [ ] Purge removes GAL-managed Claude plugin state and old GAL-managed legacy projections without deleting unrelated user-owned Claude content.
- [ ] Documentation does not mark Claude direct install as verified unless lifecycle validation has passed.

## Success Criteria

- [ ] GAL has a real Claude plugin renderer that produces a Claude-compatible plugin root.
- [ ] The Claude provider build lane is implemented and no longer reports `not-yet-implemented` after the renderer lands.
- [ ] Claude uses plugin structure in both source mode and install mode, not loose `~/.claude/skills` and `~/.claude/commands` projection.
- [ ] The old legacy projection is cleaned and removed as a Claude architecture path, not merely deprecated.
- [ ] Focused tests prove plugin structure, build orchestration, install lifecycle, and cleanup behavior.
- [ ] Docs and release matrix accurately distinguish built artifact, validated plugin, provider-native direct install, and marketplace discoverability.

## Risks

- Claude's plugin format is complete and documented; the risk is implementation drift from the documented structure or migration flow, not a missing Claude plugin capability.
- Claude CLI lifecycle commands may differ by version. The plan must separate artifact build, validation, session loading, and user-scope install instead of treating them as one state.
- Migration must remove GAL-managed legacy `.claude/skills` / `.claude/commands` entries after conversion, because leaving both surfaces in place creates duplicate commands and confusing load order.
- MCP projection may need provider-specific filtering to avoid writing secrets or machine-local values into a portable plugin artifact.
- Agent frontmatter may need Claude-specific normalization before agents can be included safely.
- Updating the release matrix before validation would repeat the same overclaim that caused this issue.

## Approval

- Human approval: pending
- Architect review: clear after 2026-05-28 revision
- Additional domain review: Claude plugin lifecycle validation required

## Review Results

### Architecture Review

Verdict: conditional clear after revision.

- The plan correctly targets the real missing slice: a Claude plugin renderer plus validated Claude lifecycle handling. It no longer mistakes legacy `~/.claude/skills` / `~/.claude/commands` projection or marketplace discoverability for provider-native Claude plugin install support.
- The implementation scope must also include shared setup context and runtime detection in `scripts/common/Common.ps1` and `scripts/common/common.sh`; otherwise install-state detection can continue to treat loose Claude projection files as evidence of a valid Claude install even after the renderer lands.
- PowerShell-only cleanup is insufficient. The POSIX concern scripts that still project Claude loose files must be included in the migration surface so source mode and setup behavior stay aligned across platforms.
- Claude MCP work must be split into two surfaces: plugin-root `.mcp.json` for portable non-secret artifact rendering, and optional Claude CLI user-scope MCP bridge commands for smoke or local lifecycle lanes. The latter is not evidence that the plugin artifact itself is correct.
- Source mode must be covered explicitly in acceptance tests. It is not enough for install mode to stop writing legacy Claude projection files if source-mode concern scripts can still recreate them.
- Uninstall and purge documentation must be updated along with behavior so the supported Claude surface is unambiguously the plugin path, not legacy loose paths.

### Engineering Review

CLEAR. The plan now has a valid execution boundary for implementation. T-001 through T-007 are independently testable, the renderer contract is scoped to a real Claude plugin root rather than legacy projection paths, and the revised task surface includes the shared runtime-detection and POSIX migration paths that would otherwise leave the old Claude architecture partially active.

Key constraints for execution:

- Treat `.claude-plugin/plugin.json` as the only file inside `.claude-plugin/`; `skills/`, `commands/`, `agents/`, and `.mcp.json` must stay at the plugin root.
- Keep plugin artifact rendering separate from Claude CLI user-scope install or MCP bridge behavior; CLI lifecycle commands are validation/install lanes, not the artifact schema.
- Do not mark Claude direct install as complete until build output, `claude plugin validate`, install/update/uninstall behavior, and legacy cleanup checks all pass.

<!-- ENG_REVIEW: CLEAR -->

### Design Review

Not requested.

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | build | Build Claude provider artifact and verify `dist/provider-plugins/claude/gal/` exists. | T-001, T-002 |
| TP-002 | validation | Verify `.claude-plugin/plugin.json` exists and contains required Claude plugin metadata. | T-001, T-002 |
| TP-003 | validation | Verify plugin-root `skills/`, `commands/`, `agents/`, and `.mcp.json` outputs. | T-002 |
| TP-004 | validation | Verify Claude build plan reports a real renderer and artifact root. | T-003 |
| TP-005 | smoke | Run `claude plugin validate` against the artifact when CLI support is available. | T-004 |
| TP-006 | cleanup | Verify source mode and install mode do not write GAL-managed loose Claude skills or command files. | T-005 |
| TP-007 | cleanup | Verify shared setup context and install-state detection no longer infer Claude install status from loose Claude projection files. | T-005 |
| TP-008 | cleanup | Verify uninstall and purge clean GAL-managed Claude plugin and remove legacy projection surfaces without deleting user-owned Claude files. | T-004, T-005 |
| TP-009 | validation | Verify plugin-root `.mcp.json` excludes secrets, machine-local paths, and user overrides even when local MCP inputs exist for CLI bridge flows. | T-002, T-005 |
| TP-010 | documentation | Verify docs only claim direct install after renderer and lifecycle checks pass. | T-007 |

## Tasks

- [x] T-001 — Define the Claude plugin output contract and path helpers for artifact root, install target, manifest, skills, commands, agents, and MCP.
- [x] T-002 — Implement `Build-ClaudePlugin.ps1` and `build-claude-plugin.sh` using the canonical provider package model.
- [x] T-003 — Wire Claude into `Build-ProviderPlugins.ps1` and `build-provider-plugins.sh`, replacing `not-yet-implemented` with the real renderer.
- [x] T-004 — Implement Claude plugin install/update/uninstall behavior in `Install-GalPlugins.ps1` and `install-gal-plugins.sh`, with explicit validation of local Claude CLI support.
- [x] T-005 — Delete legacy Claude `~/.claude/skills` and `~/.claude/commands` projection code paths, shared install-state detection, and uninstall assumptions so Claude uses the plugin architecture only across PowerShell and POSIX flows.
- [x] T-006 — Add focused tests for Claude artifact shape, build plan wiring, lifecycle validation, and cleanup behavior.
- [x] T-007 — Update docs and release matrix to reflect the verified Claude plugin implementation status without overclaiming marketplace direct install.
