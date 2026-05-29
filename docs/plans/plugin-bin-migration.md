# Plan: Plugin Bin Migration

## Goal

讓 GAL 的 Claude-compatible plugin root 提供 Claude Code 官方 `bin/` 行為：當外掛啟用時，agent 的 Bash tool 可以直接呼叫 `gal` 等公開 entrypoints，而不需要知道 repo 內部的 `scripts/` 路徑。實作策略是新增 tracked `bin/` wrapper/shim 並將它們投影到 plugin artifact root；不要把現有 `scripts/` 實作檔整批搬家。

## References

- [Claude Code plugin docs](https://code.claude.com/docs/en/plugins) - plugin root 支援 `bin/`，其中 executables 會在外掛啟用時加入 Bash tool 的 `PATH`。
- [docs/devguide.md](../devguide.md) - Claude plugin root 目前是 `~/.gal/plugins/gal/`，`skills/`、`commands/`、`agents/` 與 `.mcp.json` 位於 plugin root。
- [scripts/scripts.md](../../scripts/scripts.md) - 目前所有 public 與 internal scripts 均列在 `scripts/`，且大量內部呼叫仍以 `scripts/` 為實作位置。
- [conventions/token-budget.md](../../conventions/token-budget.md) - 目前將 `bin/` 當作 build-output exclusion，實作時必須釐清 tracked root `bin/` wrapper 與 build output `bin/` 的差異。

## Requirements

- [ ] 建立 tracked root `bin/` 作為 public executable wrapper surface；`scripts/` 仍是 implementation surface。
- [ ] 新增 `bin/gal` 和平台別 wrapper（至少 `bin/gal.sh`、`bin/gal.ps1`），使外掛 PATH 中可直接呼叫 `gal`，並正確委派至 `scripts/gal.*`。
- [ ] 為 bootstrap/setup 類 public entrypoints 提供 wrapper：`init-repo`, `setup-machine`, `uninstall-machine`, `setup-tools`, `sync-dev-context`, `install-gal-plugins`，含 PowerShell 與 Bash 可用形式。
- [ ] 為 xmachine public entrypoints 提供 wrapper：`Invoke-XmachineTask`, `Invoke-XmachinePipeline`, `Invoke-XmachineLocalTask`, `Invoke-XmachineRemoteTask`, `Start-xMachine`, `Start-XmachinePipeline`, `Get-XmachineLocalResult`, and `Get-XmachineRemoteResult`，含現有平台支援邊界。
- [ ] 不搬移 internal implementation scripts：`Update-*`, `update-*`, `Build-*`, `build-*`, `Resolve-GalCatalog.*`, `Test-*`, git smudge/clean filters, and `scripts/common/*` remain under `scripts/` unless a later plan explicitly re-architects them.
- [ ] 更新 plugin renderer（目前是 `Build-ClaudePlugin.*`；若 provider-core plan 先落地則為 `Build-CorePlugin.*`）以複製 tracked `bin/` wrappers 到 `~/.gal/plugins/gal/bin/`。
- [ ] 更新 docs and command contracts so public usage prefers `gal` or `bin/gal`, while internal orchestration remains allowed to call `scripts/*` implementation paths.
- [ ] Clarify any generated-artifact guidance that treats `bin/` as build output so future agents do not ignore the new tracked root `bin/` source wrappers.

## Approach

### Step 1: Define the public wrapper contract

- **Files**: `bin/`, `scripts/scripts.md`, `docs/devguide.md`, `conventions/token-budget.md` if needed
- **What**: Establish `bin/` as a tracked source directory for thin public wrappers only. Do not duplicate implementation logic there. Each wrapper resolves the repo root from its own location, then invokes the matching `scripts/` implementation with all original arguments preserved.
- **Verify**: A static inventory distinguishes public wrappers from internal implementation scripts, and generated-output guidance no longer treats root `bin/` wrappers as disposable build output.

### Step 2: Add core GAL and bootstrap wrappers

- **Files**: `bin/gal`, `bin/gal.sh`, `bin/gal.ps1`, plus wrappers for init/setup/sync/install/uninstall/setup-tools entrypoints
- **What**: Add minimal wrappers for the public CLI and machine/bootstrap commands. Bash wrappers should be executable and should `exec` the target script. PowerShell wrappers should forward unbound arguments and preserve exit codes.
- **Verify**: From repo root, `bin/gal status` or platform equivalent resolves the same dispatch path as `scripts/gal.*`; setup/sync wrappers reach concern scripts without changing `scripts/common` imports.

### Step 3: Add xmachine wrappers without changing xmachine internals

- **Files**: `bin/Invoke-XmachineTask.*`, `bin/Invoke-XmachinePipeline.*`, `bin/Invoke-XmachineLocalTask.sh`, `bin/Invoke-XmachineRemoteTask.*`, `bin/Start-xMachine.*`, `bin/Start-XmachinePipeline.*`, `bin/Get-XmachineLocalResult.sh`, `bin/Get-XmachineRemoteResult.ps1`
- **What**: Expose xmachine public entrypoints through `bin/` wrappers but keep remote staging, cleanup, and internal `scripts/` references intact until a separate xmachine topology plan changes them. Platform-specific wrappers may exist only where the underlying implementation exists.
- **Verify**: Wrapper-level smoke tests and static xmachine path checks prove no remote runner path was accidentally changed from implementation location to wrapper location.

### Step 4: Render `bin/` into the plugin artifact

- **Files**: `scripts/Build-ClaudePlugin.ps1`, `scripts/build-claude-plugin.sh`, or their provider-neutral successors; provider plugin tests
- **What**: Copy tracked `bin/` wrapper files into the plugin artifact root at `bin/`, preserving executable mode for Bash files. The renderer must not copy all of `scripts/` into the plugin root.
- **Verify**: An isolated artifact build contains `bin/gal`, `bin/gal.sh`, and the expected wrappers, while `scripts/` is absent from the plugin root.

### Step 5: Update public docs and command contracts conservatively

- **Files**: `scripts/scripts.md`, `README.md`, `README.zh-Hant.md`, `docs/personalization.md`, `docs/personalization.zh-Hant.md`, `docs/devguide.md`, `commands/gal/SKILL.template.md`, `commands/gal-init/SKILL.template.md`, `commands/gal-pipeline/SKILL.template.md`, xmachine docs/templates as needed
- **What**: Public instructions should prefer `gal` when plugin PATH is available, `./bin/gal` from a repo checkout, or `scripts/gal.*` as an implementation fallback. Do not globally replace every `scripts/` reference; internal implementation, remote staging, and setup concern docs may still correctly point to `scripts/`.
- **Verify**: Documentation no longer teaches `scripts/` as the primary public entrypoint, but still preserves accurate implementation and fallback references.

## Files to Create or Modify

- `[CREATE] bin/` - tracked public wrapper directory added to the repo and copied into plugin artifacts.
- `[CREATE] bin/gal`, `bin/gal.sh`, `bin/gal.ps1` - primary GAL public wrapper entrypoints.
- `[CREATE] bin/init-repo*`, `bin/setup-machine*`, `bin/uninstall-machine*`, `bin/setup-tools*`, `bin/sync-dev-context*`, `bin/install-gal-plugins*` - bootstrap/setup wrappers.
- `[CREATE] bin/Invoke-Xmachine*`, `bin/Start-xMachine*`, `bin/Start-XmachinePipeline*`, `bin/Get-Xmachine*Result*` - xmachine public wrappers matching existing implementations.
- `[MODIFY] scripts/Build-ClaudePlugin.ps1` and `scripts/build-claude-plugin.sh` - copy tracked `bin/` wrappers into the plugin root, unless superseded by `Build-CorePlugin.*` from the provider-core migration.
- `[MODIFY] scripts/Test-BuildProviderPlugins.ps1` or related provider artifact tests - assert plugin artifacts include `bin/` wrappers.
- `[MODIFY] scripts/scripts.md`, `README.md`, `README.zh-Hant.md`, `docs/devguide.md`, `docs/personalization.md`, `docs/personalization.zh-Hant.md` - document the public wrapper surface.
- `[MODIFY] commands/gal/SKILL.template.md`, `commands/gal-init/SKILL.template.md`, `commands/gal-pipeline/SKILL.template.md`, and selected xmachine docs/templates - prefer `gal`/`bin/gal` public usage without deleting correct `scripts/` implementation fallbacks.
- `[MODIFY] conventions/token-budget.md` only if the tracked root `bin/` wrapper directory needs an explicit exception from build-output exclusion wording.

## Test Cases

- [ ] TP-001 - Run `pwsh -File bin/gal.ps1 status` on Windows; expected result: same dispatch behavior as `pwsh -File scripts/gal.ps1 status`.
- [ ] TP-002 - Run `bash bin/gal.sh status` or `bin/gal status` on a host with Bash available; expected result: same dispatch behavior as `bash scripts/gal.sh status`.
- [ ] TP-003 - Run `pwsh -File bin/Setup-Machine.ps1 -DryRun` and `pwsh -File bin/Sync-DevContext.ps1 -TargetPath .`; expected result: wrappers reach existing implementation and preserve argument forwarding.
- [ ] TP-004 - Run Bash setup/sync wrappers on a Bash-capable host, or record the same Windows WSL host limitation used by prior validation.
- [ ] TP-005 - Run wrapper smoke checks for xmachine public wrappers with dry-run/static-safe arguments where available; expected result: wrappers call the existing scripts without changing remote runner paths.
- [ ] TP-006 - Build the Claude/core plugin artifact in an isolated home; expected result: plugin root contains `bin/` wrappers and does not contain a copied `scripts/` implementation tree.
- [ ] TP-007 - If Claude CLI is available, run `claude plugin validate <plugin-root> --strict`; expected result: adding `bin/` does not invalidate the plugin.
- [ ] TP-008 - Search changed docs and command templates; expected result: public guidance prefers `gal` or `bin/gal`, while remaining `scripts/` references are implementation fallbacks or internal paths.
- [ ] TP-009 - Run Markdown diagnostics on changed docs; expected result: no markdownlint errors, no bare URLs, exactly one trailing newline.

## Success Criteria

- [ ] The plugin artifact exposes `bin/gal` on the plugin PATH so Claude Code's Bash tool can run `gal` while the plugin is enabled.
- [ ] The repo has a tracked root `bin/` public wrapper surface and keeps `scripts/` as the implementation surface.
- [ ] No internal setup, sync, xmachine, provider-build, or test script breaks because of path relocation.
- [ ] The plugin renderer copies `bin/` wrappers but not the full `scripts/` tree into the plugin artifact.
- [ ] Public docs describe `gal`/`bin/gal` as the normal user-facing entrypoint and preserve `scripts/` only where it remains an implementation fallback.
- [ ] Protected-path changes are limited to the reviewed docs/templates/scripts named in this plan.

## Risks

- **Wrapper drift**: Thin wrappers can fall out of sync with `scripts/` implementations. Keep wrappers generic and argument-preserving; do not copy business logic into them.
- **Executable bit loss**: Bash wrappers must remain executable when rendered into the plugin artifact. Tests must inspect file mode on POSIX hosts or record host limits on Windows.
- **Overbroad replacement**: Replacing every `scripts/` reference with `bin/` would break internal orchestration and remote xmachine staging. Update public docs selectively.
- **Protected-path churn**: `Sync-DevContext.*`, `Setup-Machine.*`, command templates, and token-budget guidance are protected. Implementation must stay inside this reviewed scope.
- **Cross-plan collision**: The provider-core migration may rename `Build-ClaudePlugin.*` to `Build-CorePlugin.*`. Apply the renderer copy step to whichever script owns the shared plugin artifact root when implementation begins.

## Open Questions

- [x] OQ-001 - Keep `bin/` flat for public executable wrappers. Do not introduce `bin/agent/`; PATH surfaces work best with predictable top-level command names. *(raised by: planning, resolved by: architecture review)*
- [x] OQ-002 - Do not physically move `scripts/` implementation files into `bin/`. `bin/` is the public wrapper surface; `scripts/` remains the implementation surface. *(raised by: architecture review, resolved by: architecture review)*
- [x] OQ-003 - If the provider-core migration lands first, apply plugin artifact rendering changes to `Build-CorePlugin.*`; otherwise apply them to `Build-ClaudePlugin.*`. *(raised by: architecture review, resolved by: architecture review)*

## Approval

- Human approval: [pending]
- Architect review: [clear]
- Additional domain review: [not triggered]

## Review Results

### Architecture Review

#### Verdict: APPROVE AFTER REVISION

The original plan was too risky because it proposed moving many implementation scripts into `bin/`. That would force repo-wide path rewrites across setup, sync, xmachine, tests, docs, and protected surfaces. Claude's plugin contract only requires executable files under plugin-root `bin/` to be added to the Bash tool PATH; it does not require implementation files to live there. The revised plan satisfies the plugin requirement with tracked wrappers and renderer projection while preserving the current `scripts/` implementation topology.

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| Use `bin/` wrappers instead of moving scripts | Minimal breakage; preserves internal paths and tests | Adds thin wrapper files to maintain | OK |
| Keep `scripts/` as implementation surface | Avoids widespread path rewrites and protected-path churn | Public and implementation paths differ | OK |
| Render `bin/` into plugin artifact | Gives Claude Bash PATH the desired commands | Renderer must preserve executable bits | OK |
| Update docs selectively | Keeps internal references correct | Requires review discipline; no blind search/replace | OK |

#### Over-engineering Flags

- **OE-01** Moving every public-looking script into `bin/` is unnecessary. A wrapper layer solves the Claude PATH problem with far less blast radius.
- **OE-02** Creating nested `bin/agent/` or per-domain subfolders would make PATH discovery worse. Use flat command names and keep organization in `scripts/`.

#### Bug Surface

- **BUG-01** High: Physical moves would break `$PSScriptRoot`/`SCRIPT_DIR` assumptions and `scripts/common` imports across setup and xmachine. Use wrappers.
- **BUG-02** Medium: Bash wrapper executable bits can be lost on Windows or during artifact copy. Add artifact-mode validation where possible.
- **BUG-03** Medium: Remote xmachine paths currently stage or call `scripts/Start-xMachine.*` and related implementation files. Do not rewrite those paths to `bin/` until xmachine staging is redesigned.
- **BUG-04** Low: Token-budget docs currently list `bin/` as build-output exclusion. If root `bin/` becomes tracked source, clarify the exception.

#### Performance Concerns

None. Wrapper invocation overhead is negligible and does not affect pipeline token budget or runtime data loading.

#### Missing from Plan

None after revision. The plan now includes wrapper scope, renderer projection, docs, command-template updates, and validation boundaries.

#### Recommended Changes

1. Implement wrapper files first and keep them logic-free.
2. Update the plugin renderer second so the same tracked wrappers appear under plugin-root `bin/`.
3. Update command templates and docs last, with selective public-entrypoint wording instead of global `scripts/` replacement.
4. Return to `/deep-planning` if implementation requires moving implementation scripts, redesigning xmachine staging, or changing bootstrap packaging policy.

#### What's Good

- The goal correctly identifies Claude plugin-root `bin/` as the right user-facing mechanism.
- Keeping internal scripts out of `bin/` preserves the existing setup and xmachine topology.
- The plan keeps the public command surface easy for agents to use: `gal` is the primary command.

### Business Review

Not triggered. This plan does not change pricing, permissions, onboarding, eligibility, or other customer-facing business rules.

### Design Review

Not triggered. This plan does not change UI layout, visual design, components, or accessibility-sensitive flows.

### Engineering Review

#### Verdict: CLEAR

The plan is ready for implementation as a scoped wrapper/projection migration. It touches protected setup/sync and command-template surfaces, but the scope is now explicit and avoids the high-risk file-move path.

Implementation must not close by claiming all old `scripts/` references are gone; many are correct implementation references. The acceptance gate is that public usage and plugin PATH use `bin/`/`gal`, while internal implementation and fallback references remain accurate.

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

Run TP-001 through TP-009. On this Windows host, Bash runtime tests may hit the known WSL `/bin/bash` limitation; record that limitation instead of claiming live Bash parity when it occurs.

Implementation closeout must include:

- wrapper argument-forwarding checks for PowerShell and Bash where available
- plugin artifact shape check for `bin/`
- renderer check that `scripts/` was not copied into the plugin root
- documentation/command-template search showing public `bin/gal` guidance and retained internal `scripts/` fallbacks
- markdown diagnostics for all changed Markdown files

## Tasks

- [ ] T-001 - Add tracked root `bin/` wrappers for core GAL, bootstrap/setup, sync/install, and xmachine public entrypoints while keeping `scripts/` as implementation. Verify TP-001, TP-002, TP-003, TP-004, and TP-005 as host capabilities allow.
- [ ] T-002 - Update the Claude/core plugin renderer to copy tracked `bin/` wrappers into plugin-root `bin/` without copying the full `scripts/` implementation tree. Verify TP-006 and TP-007 when Claude CLI is available.
- [ ] T-003 - Update public docs, command templates, and any needed generated-artifact wording so `gal`/`bin/gal` is the public entrypoint and `scripts/` remains an implementation fallback only. Verify TP-008 and TP-009.
