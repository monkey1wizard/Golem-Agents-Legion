# Plan: 收斂 GAL 安裝架構到單一 Canonical Root

## Goal

將目前 GAL 安裝流程中混合存在的兩套模型收斂成單一、可驗證、可測試的 canonical root，避免 build、install、lifecycle metadata、tests、docs 各自指向不同位置。

這份計畫不覆寫既有已完成的 [migrate-install-architecture.md](../plans/migrate-install-architecture.md)；它只處理目前實作與既有計畫之間已經發生的分歧，並把後續工作收斂成一條可落地的修正路線。

## Problem Statement

目前程式碼已經進入混合狀態：

- 共用 schema 已定義 canonical plugin root 為 `~/.gal/plugins/<plugin-id>/`。
- Claude renderer 已直接將內容寫入 `~/.gal/plugins/gal/`。
- 但部分 build plan、artifact metadata、tests、docs、以及既有 helper 仍把 `~/.gal/dist/provider-plugins/<provider>/gal` 當成 artifact root 或正式輸出路徑。
- install orchestration 與 provider lifecycle 並未完全跟上新的 canonical root，導致 `Install-GalPlugins`、`Build-ProviderPlugins`、`Build-ClaudePlugin` 的行為與說明不一致。

結果是：

1. 使用者無法單靠 install flow 判斷哪個路徑才是 source of truth。
2. Claude build 可成功 materialize 到 `~/.gal/plugins/gal`，但 install orchestration 不一定會自動走到相同結果。
3. 測試與文件仍有明顯比例假設 `~/.gal/dist/provider-plugins/...` 是實際 artifact root。

## Requirements

- [ ] 必須選定一個單一 canonical root，並讓 build、install、projection、tests、docs 全部收斂到同一模型。
- [ ] 任何 provider-visible install target、active shortcut、`dist` 內的 projection / conversion output、release artifact 都不得反向成為 canonical root。
- [ ] 若 canonical root 選為 `~/.gal/plugins/<plugin-id>/`，則 `dist/` 必須明確降級為 release/package output；不得再作 runtime source of truth。
- [ ] 若保留 `~/.gal/dist/provider-plugins/<provider>/gal`，則它只能是 projection/package layer，且實際 canonical content owner 仍需明確記錄。
- [ ] `Install-GalPlugins` 必須能在 source mode 與 install mode 下都 materialize 正確的 canonical root，而不是只建立空目錄或只更新部分 metadata。
- [ ] dev mode 下若有 GAL 相關變更，必須輸出到 `.gal/dist/commits/` 以隔離變更；不得把這些隔離輸出誤當 canonical root。
- [ ] `Build-ProviderPlugins`、provider lifecycle state、provider shortcut/install target、以及 smoke command 必須對同一個 root 達成一致。
- [ ] `GAL_SKILLS` migration、`xmachine` canonical config、`~/.gal/source` source-mode bridge、`~/.gal/config` local config 主路徑等已完成切片不得退回舊拓撲。
- [ ] Copilot/Codex native install 仍維持 `not-implemented`，不得因這次收斂順便假裝完成。
- [ ] 測試必須以 temp `HOME` / `USERPROFILE` 驗證收斂後的單一路徑，不污染真實 `~/.gal`。
- [ ] 文件必須與實作完全一致，且不得手改 generated adapters。

## Decision To Lock

在開始實作前，必須先明確鎖定以下決策：

- **推薦方向**：以 `~/.gal/plugins/<plugin-id>/` 作為 canonical plugin root。
- **理由**：目前 schema、Claude renderer、`GAL_SKILLS` migration、shared helpers 已經偏向這個模型；相較於退回 `dist` canonical，收斂成本較低。
- **衍生規則**：
  - `~/.gal/plugins/gal/` = canonical content owner
  - `~/.gal/dist/` = package output、conversion output、managed projection / metadata 與 release output 的共用層；不是 canonical content owner
  - `~/.gal/dist/commits/` = dev mode 下 GAL 變更的隔離輸出，不是 canonical content owner
  - `~/.claude/plugins/gal` = provider-visible install target; when the provider supports the standard layout directly, it should point at the canonical root rather than a conversion layer
  - `~/.gal/active/<provider>` = stable shortcut alias, not content owner
  - 只有需要轉換才能用的 provider 才消費 `~/.gal/dist/` 內的 conversion output

若人類決策改為保留 `dist` 為 canonical，則本計畫需整體改寫，不應混用兩種模型。

## Current Verified State

- 已存在 `Get-GalPluginRoot()` / `get_gal_plugin_root()` helper。
- `Build-ClaudePlugin` 目前寫入 `~/.gal/plugins/gal`。
- `Install-GalPlugins` 的 Claude lifecycle 目前也讀取 `Get-GalPluginRoot -PluginId 'gal'`。
- `Build-ProviderPlugins` 仍記錄 Claude artifact root 為 `Get-ClaudePluginArtifactRoot()`，其定義仍落在 `~/.gal/dist/provider-plugins/claude/gal`。
- `Test-BuildProviderPlugins.ps1` 等測試仍以 `~/.gal/dist/provider-plugins/...` 為主要斷言。
- `GAL_SKILLS` migration 已改為寫入 `~/.gal/plugins/gal/skills/...`。
- `~/.gal/config`、`~/.gal/source`、`~/.gal/config/xmachine.json` 的主路徑遷移已經落地，不應回退。

## Approach

### Step 1: 鎖定 canonical root 與層級責任

- **Files**: `docs/plans/fix-gal-install-canonical-root-convergence.md`, `scripts/common/Common.ps1`, `scripts/common/common.sh`, `scripts/common/ProviderPlugin.ps1`, `scripts/common/provider-plugin.sh`
- **What**:
  - 鎖定 `~/.gal/plugins/<plugin-id>/` 為 canonical root。
  - 將 `dist`、`active`、provider install target 的責任明確分層，避免再引入額外中介層。
  - 補齊 common/schema 層的 naming 與 comments，避免同一 repo 同時存在兩套語意。
- **Verify**: 不再有任何 source-of-truth 描述把 `dist/provider-plugins/<provider>/gal` 當成 canonical root。

### Step 2: 收斂 Claude build 與 lifecycle metadata

- **Files**: `scripts/Build-ClaudePlugin.ps1`, `scripts/build-claude-plugin.sh`, `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`
- **What**:
  - 讓 Claude build、build plan、lifecycle state、validation target、projection target 全部使用同一個 canonical root。
  - 如果 `Build-ProviderPlugins` 仍需要 `ArtifactRoot` 欄位，則改為記錄 canonical root，或明確拆成 `canonicalRoot` 與 `packageOutputRoot`。
  - 讓 install orchestration 自動走到同一條 materialization path，而不是只在手動跑 `Build-ClaudePlugin` 時成功。
- **Verify**: `Install-GalPlugins.ps1 -Replace -Force` 之後，`~/.gal/plugins/gal/.claude-plugin/plugin.json` 必須存在，且 `.claude/plugins/gal` 解析到相同 root 或其明確 projection。

### Step 3: 明確降級 `dist` 為 package/release output

- **Files**: `scripts/common/ProviderPlugin.ps1`, `scripts/common/provider-plugin.sh`, `scripts/Package-ReleaseArtifacts.ps1`, `scripts/package-release-artifacts.sh`, `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`
- **What**:
  - 保留 `~/.gal/dist` 作為 release/package output。
  - 明確把 `~/.gal/dist/commits/` 定義為 dev mode 的隔離變更輸出；GAL 變更在 dev mode 下應送入此處，而不是混入 canonical runtime root 的責任判定。
  - 移除或重命名所有把 `dist/provider-plugins/...` 誤當 runtime artifact root 的 metadata 欄位。
  - 若某些 provider 仍需 package bundle，該 bundle 必須從 canonical root 產生，而不是自己成為 canonical root。
- **Verify**: `dist` 只承擔 package/release、conversion、managed metadata、以及 dev mode `.gal/dist/commits/` 隔離輸出；正常 source/install lifecycle 不依賴 `dist` 才能載入 Claude plugin。

### Step 4: 收斂 AGY / active shortcut / dist provider metadata

- **Files**: `scripts/Build-AgyPlugin.ps1`, `scripts/build-agy-plugin.sh`, `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`, `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`
- **What**:
  - 明確採用一條規則：只有需要格式轉換的 provider 才使用 `dist` 內的 conversion output；可直接消費標準結構的 provider 應直接指向 canonical root。
  - 依此規則，AGY 與 Claude 若其檔案結構可直接套用標準 root，則直接消費 canonical root，不再為它們引入額外 conversion layer。
  - `~/.gal/active/<provider>` 只作 alias，不作 content owner。
  - provider lifecycle metadata 必須落在 `~/.gal/dist/providers/<provider>/managed.json` 或等價的 `dist` 管理路徑，並記錄 `canonicalRoot`、`projectionRoot`、`installTarget`、`packageOutputRoot`，不得混稱 `artifactRoot`。
- **Verify**: `active` shortcut 與 provider lifecycle state 不再讓人誤判 `dist` 是 canonical root。

### Step 5: 收斂 tests 到單一模型

- **Files**: `scripts/Test-BuildProviderPlugins.ps1`, `scripts/Test-InstallGalPlugins.ps1`, `scripts/Test-UpdateMcpProjection.ps1`, Bash smoke tests
- **What**:
  - 全部改用 temp home 驗證 `~/.gal/plugins/gal` canonical root。
  - 若 `dist` 仍有 package output 測試，需額外分離為 package/release lane，不和 runtime install lane 混在一起。
  - 保留 Copilot/Codex `not-implemented` 斷言。
- **Verify**: 測試同時能證明 canonical root、provider projection、以及 package output 三者邊界清楚。

### Step 6: 重新驗證 install/uninstall/convergence

- **Files**: `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `scripts/Build-ClaudePlugin.ps1`, `scripts/build-claude-plugin.sh`, tests
- **What**:
  - 驗證 source mode reinstall、Claude validation、provider projection、uninstall cleanup。
  - 驗證 dev mode 下 GAL 變更會落到 `~/.gal/dist/commits/`，且不會讓 `dist` 被誤判成 canonical root。
  - 驗證已完成的 `~/.gal/config`、`~/.gal/source`、`GAL_SKILLS` migration、`xmachine` canonical config 不受回歸影響。
- **Verify**: 重新安裝後使用者可以從磁碟與 CLI 輸出直接看出 canonical root、projection、package output 各自的位置。

### Step 7: 更新 docs 與 user-facing contract

- **Files**: `docs/devguide.md`, `docs/personalization.md`, `docs/personalization.zh-Hant.md`, `README.md`, `README.zh-Hant.md`, `scripts/scripts.md`, `docs/release-matrix.md`
- **What**:
  - 文件明確說明 canonical root 與 package output 的差異。
  - 更新 dev mode / source mode / install mode 的 path 說明。
  - 明確說明 dev mode 下的 GAL 變更會輸出到 `~/.gal/dist/commits/`，其用途是隔離變更，不是 runtime source of truth。
  - 說明 `~/.gal/plugins/gal` 會被直接 materialize，provider target 只是指向它或其 projection。
- **Verify**: 文件搜尋不再同時宣稱 `plugins/gal` 與 `dist/provider-plugins/...` 都是 source of truth。

## Files to Create or Modify

- `docs/plans/fix-gal-install-canonical-root-convergence.md` — 新 source plan，處理目前混合架構的收斂。
- `scripts/common/Common.ps1` — canonical root / package output / provider shortcut terminology 收斂。
- `scripts/common/common.sh` — Bash 對等收斂。
- `scripts/common/ProviderPlugin.ps1` — schema、validation、root metadata 命名收斂。
- `scripts/common/provider-plugin.sh` — Bash 對等收斂。
- `scripts/Build-ClaudePlugin.ps1` / `scripts/build-claude-plugin.sh` — canonical build target 與 package output 邏輯清晰分離。
- `scripts/Build-ProviderPlugins.ps1` / `scripts/build-provider-plugins.sh` — provider build plan metadata 收斂。
- `scripts/Install-GalPlugins.ps1` / `scripts/install-gal-plugins.sh` — install orchestration 與 provider lifecycle 自動 materialization 收斂。
- `scripts/Build-AgyPlugin.ps1` / `scripts/build-agy-plugin.sh` — AGY projection/package lane 收斂。
- `scripts/Test-BuildProviderPlugins.ps1`, `scripts/Test-InstallGalPlugins.ps1`, `scripts/Test-UpdateMcpProjection.ps1` — tests 收斂到單一模型。
- `docs/devguide.md`, `docs/personalization.md`, `docs/personalization.zh-Hant.md`, `README.md`, `README.zh-Hant.md`, `scripts/scripts.md`, `docs/release-matrix.md` — 使用者可見契約更新。

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | static | 搜尋 `plugins/gal`、`dist/provider-plugins/claude/gal`、`artifactRoot`、`canonicalRoot` 的引用，確認不再互相矛盾。 | T-001, T-002 |
| TP-002 | integration | `./scripts/Build-ClaudePlugin.ps1 -Force` 後，`~/.gal/plugins/gal/.claude-plugin/plugin.json` 存在。 | T-002 |
| TP-003 | integration | `./scripts/Install-GalPlugins.ps1 -Replace -Force` 後，自動 materialize `~/.gal/plugins/gal`，且 `.claude/plugins/gal` 指向正確 root/projection。 | T-002 |
| TP-004 | integration | `Build-ProviderPlugins` 在同一 temp home 下輸出一致的 provider metadata；不再把 `dist` 誤標成 Claude runtime canonical root。 | T-002, T-003 |
| TP-005 | integration | dev mode 下 GAL 變更會輸出到 `~/.gal/dist/commits/`，且該隔離輸出不會被 runtime install lane 視為 canonical root。 | T-003 |
| TP-006 | integration | package/release lane 需要時才建立 `~/.gal/dist`；正常 runtime install 不依賴 `dist`。 | T-003 |
| TP-007 | integration | `Test-BuildProviderPlugins.ps1` 與 `Test-InstallGalPlugins.ps1` 在 temp home 下通過，並確認 Copilot/Codex 維持 `not-implemented`。 | T-005 |
| TP-008 | regression | `GAL_SKILLS` migration 仍寫入 `.gal/plugins/gal/skills`，最終 config 不保留 `GAL_SKILLS`。 | T-006 |
| TP-009 | regression | `~/.gal/config/xmachine.json`、legacy fallback warning、`~/.gal/source` bridge 行為不回退。 | T-006 |
| TP-010 | static | docs 搜尋不再同時把 `dist` 與 `plugins/gal` 描述為 canonical root，且會正確區分 `~/.gal/dist/commits/` 的 dev mode 用途。 | T-007 |

## Success Criteria

- [ ] `~/.gal/plugins/gal` 被明確且一致地視為 GAL canonical plugin root。
- [ ] `Build-ClaudePlugin`、`Build-ProviderPlugins`、`Install-GalPlugins`、Claude lifecycle state 對 canonical root 的定義完全一致。
- [ ] `~/.gal/dist` 僅作 package/release output、conversion output 與 provider managed metadata；不再被 runtime install path 視為 canonical content owner。
- [ ] dev mode 下 GAL 變更會被隔離到 `~/.gal/dist/commits/`，且此隔離輸出不會與 canonical root 混淆。
- [ ] 使用者重新安裝後，可直接在磁碟上看到 `~/.gal/plugins/gal` materialize 成功。
- [ ] 測試、文件、CLI 輸出不再對 canonical root 提供互相矛盾的資訊。
- [ ] `~/.gal/config`、`~/.gal/source`、`GAL_SKILLS` migration、`xmachine` canonical config 等已完成切片不回退。

## Risks

- 若在未鎖定 canonical root 前繼續修補 install/build，會讓混合模型更難收斂。
- 若把 `dist` 完全刪掉但 package/release lane 仍依賴它，會造成另一種回歸；必須先分離 runtime 與 package lane。
- 若只修 Claude renderer 而不修 `Build-ProviderPlugins` 與 tests，repo 會繼續提供互相矛盾的訊號。
- 若 install orchestration 只在手動 build 時成功、在正常 reinstall 時失敗，使用者仍會認為新架構沒有真正落地。
- 若 docs 不同步更新，使用者會依舊把 `dist` 或 repo-root local state 當成正式路徑。

## Approval

- Human approval: [completed]
- Architect review: [done]
- Additional domain review: [not requested]

## Review Results

### Engineering Review

Clear. Closeout verification on 2026-05-29 aligned AGY install/projection behavior with the canonical-root contract, updated xmachine dispatch readers to accept `xmachineNodeAliases`, restored Windows PowerShell 5.1 compatibility in the affected install/build entry points, reconciled MCP projection expectations, and updated stale user-facing docs. Remaining test limits are environmental rather than architectural.

## Tasks

- [x] T-001 — 鎖定 canonical root 決策，停止 `plugins` / `dist` 雙模型並存。
- [x] T-002 — 收斂 Claude build / install / lifecycle metadata 到同一 canonical root。
- [x] T-003 — 明確降級 `dist` 為 package/release output，移除 runtime source-of-truth 歧義。
- [x] T-004 — 收斂 AGY / active shortcut / dist provider metadata 的責任邊界。
- [x] T-005 — 更新 PowerShell/Bash tests 到單一模型並保留 temp-home 隔離。
- [x] T-006 — 重新驗證 reinstall / uninstall / migration / xmachine / source bridge 全部不回歸。
- [x] T-007 — 更新 docs 與 user-facing contract，說明 canonical root、projection、package output 的差異。

## Closeout

### Checklist

- [x] T-008 — Make AGY install/projection follow the canonical root contract instead of copying package output into the provider target.
- [x] T-009 — Make all xmachine dispatch readers accept canonical `xmachineNodeAliases` while retaining legacy `nodes` fallback.
- [x] T-010 — Restore Windows PowerShell 5.1 compatibility for install/build scripts.
- [x] T-011 — Reconcile MCP projection test expectations with current managed bridge behavior.
- [x] T-012 — Update stale user-facing docs that still point to repo-root local config paths.
- [x] T-013 — Close plan/state bookkeeping for the completed install architecture work.
- [x] T-014 — Run verification gates and record results.

### Closeout Results

- 2026-05-29: T-008 complete. `Build-AgyPlugin` now projects the provider install target to AGY package output via symlink/junction instead of copying. `Build-ProviderPlugins` materializes `~/.gal/plugins/gal` before AGY package projection, so AGY-only provider builds no longer leave the canonical root absent. Temp-home verification confirmed canonical root, package output, provider projection, and active alias all exist, with provider/active paths as reparse points.
- 2026-05-29: T-009 complete. `Invoke-XmachineTask`, `Invoke-XmachinePipeline`, `Test-Xmachine`, and Bash xmachine dispatch readers now prefer `xmachineNodeAliases` and fall back to legacy `nodes`. Temp-home probes against canonical `~/.gal/config/xmachine.json` now discover `node-name` instead of failing on a missing `nodes` object.
- 2026-05-29: T-010 complete. Removed PowerShell 7-only assumptions from install/build paths. Windows PowerShell 5.1 now passes `scripts\Setup-Machine.ps1 -DryRun` and `scripts\Build-ProviderPlugins.ps1 -DryRun`; static scan found no remaining three-argument `Join-Path`, `ConvertFrom-Json -AsHashtable`, or `Get-FileHash -InputStream` uses in scripts.
- 2026-05-29: T-011 complete. `Test-UpdateMcpProjection.ps1` now asserts the actual Copilot CLI bridge contract: enabled managed entries are installed, while disabled GitHub bridge aliases are not reintroduced.
- 2026-05-29: T-012 complete. Updated stale docs and examples that still described repo-root `mcp.local.json`, repo-root `config.local.env`, or runtime-checkout `xmachine.config.json` as canonical user-edit surfaces.
- 2026-05-29: T-013 complete. Repo state was closed out so the install-architecture prompt/state no longer remain parked in `IMPLEMENT` after the closeout pass.
- 2026-05-29: T-014 complete. Verification passed for `pwsh scripts/Test-BuildProviderPlugins.ps1`, `pwsh scripts/Test-InstallGalPlugins.ps1`, `pwsh scripts/Test-UpdateMcpProjection.ps1`, `powershell scripts/Setup-Machine.ps1 -DryRun`, `powershell scripts/Build-ProviderPlugins.ps1 -DryRun`, and `git status --short --ignored -- config.local.env mcp.local.json model-roles.local.md xmachine.config.json dist`. `Test-Xmachine.ps1` was not run as a full smoke test because this machine did not provide a configured remote `-WorkNode`; the canonical config/schema paths were still verified locally during T-009.
