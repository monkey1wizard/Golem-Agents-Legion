# Plan: Plugin Bin Migration

## Goal

將 GAL 專案架構與 Claude Code 的原生外掛架構對齊。藉由將公開的執行腳本（Public CLI）移至 `bin/` 目錄，確保當 GAL 被載入為外掛時，這些核心指令能自動被加入到 AI Bash 工具的 `PATH` 環境變數中，使 Agent 能夠無縫地直接呼叫這些指令，而不需要指定完整路徑。

## Requirements

- [ ] 核心 dispatcher（`gal.sh` / `gal.ps1`）必須移至 `bin/` 目錄。
- [ ] 設定與管理腳本（如 `setup-machine`、`uninstall-machine`、`init-repo`、`sync-dev-context` 等）必須移至 `bin/`。
- [ ] Xmachine 的進入點腳本（如 `Start-xMachine`、`Invoke-XmachineTask` 等）必須移至 `bin/`。
- [ ] 僅供內部呼叫的腳本（如 `update-*`、`build-*`、Git hooks 等）應維持在 `scripts/` 目錄。
- [ ] 更新所有被移動腳本內部的路徑解析邏輯（例如 `$RepoRoot` 的計算），以確保它們仍能正確定位專案根目錄與 `scripts/common`。
- [ ] 更新所有文件（包含 `scripts.md`）與系統呼叫中對這些腳本的相對路徑參照（從 `scripts/` 改為 `bin/`）。

## Approach

### Step 1: 建立 bin 目錄
- **Files**: `bin/`
- **What**: 在專案根目錄下建立 `bin/` 目錄，作為所有對外開放之可執行檔的新家。
- **Verify**: 目錄成功建立。

### Step 2: 轉移核心 CLI 與設定腳本
- **Files**: 
  - `scripts/gal.ps1` -> `bin/gal.ps1`
  - `scripts/gal.sh` -> `bin/gal.sh`
  - `scripts/Init-Repo.ps1` -> `bin/Init-Repo.ps1`
  - `scripts/init-repo.sh` -> `bin/init-repo.sh`
  - `scripts/Setup-Machine.ps1` -> `bin/Setup-Machine.ps1`
  - `scripts/setup-machine.sh` -> `bin/setup-machine.sh`
  - `scripts/Uninstall-Machine.ps1` -> `bin/Uninstall-Machine.ps1`
  - `scripts/uninstall-machine.sh` -> `bin/uninstall-machine.sh`
  - `scripts/Setup-Tools.ps1` -> `bin/Setup-Tools.ps1`
  - `scripts/setup-tools.sh` -> `bin/setup-tools.sh`
  - `scripts/Sync-DevContext.ps1` -> `bin/Sync-DevContext.ps1`
  - `scripts/sync-dev-context.sh` -> `bin/sync-dev-context.sh`
  - `scripts/Install-GalPlugins.ps1` -> `bin/Install-GalPlugins.ps1`
  - `scripts/install-gal-plugins.sh` -> `bin/install-gal-plugins.sh`
- **What**: 將上述檔案移至 `bin/` 目錄，並調整腳本內取得 `RepoRoot` 的相對路徑邏輯，確保其內部引用其他 `scripts/*` 腳本時路徑正確。
- **Verify**: 檔案位在 `bin/` 且能正常執行而不發生路徑解析錯誤。

### Step 3: 轉移 Xmachine 腳本
- **Files**: 
  - `scripts/Start-xMachine.*` -> `bin/Start-xMachine.*`
  - `scripts/Invoke-Xmachine*Task.*` -> `bin/Invoke-Xmachine*Task.*`
  - `scripts/Invoke-XmachinePipeline.*` -> `bin/Invoke-XmachinePipeline.*`
  - `scripts/Start-XmachinePipeline.*` -> `bin/Start-XmachinePipeline.*`
  - `scripts/Get-Xmachine*Result.*` -> `bin/Get-Xmachine*Result.*`
- **What**: 將對外開放的 Xmachine 派遣與執行腳本移至 `bin/` 並同樣修正路徑解析。
- **Verify**: Xmachine 腳本能在 `bin/` 下正常執行。

### Step 4: 更新內部參照與文件
- **Files**: 
  - `scripts/scripts.md`
  - 其他呼叫到這些腳本的 `scripts/` 內部文件或腳本
  - 潛在的 GitHub Actions workflow 檔案（如有）
- **What**: 使用全域搜尋將寫死的 `scripts/gal.sh`、`scripts/setup-machine.sh` 等路徑字串替換為 `bin/gal.sh` 等。
- **Verify**: 全域搜尋 `scripts/gal.` 及其他被轉移腳本的舊路徑不再出現殘留。

## Files to Create or Modify

- `[bin/]` — [New Directory] 存放對外的可執行腳本。
- `[scripts/scripts.md]` — 更新文件以反映新的腳本路徑與分類。
- 所有被移動的腳本 — 需要修改其內部的 `$RepoRoot` / `REPO_ROOT` 計算邏輯。

## Test Cases

- [ ] Test case 1 — 執行 `bin/gal.sh status` 應能正確解析專案狀態而不報路徑錯誤。
- [ ] Test case 2 — 在 Claude Code 外掛啟用下，直接輸入 `gal status` 應能成功執行（因為 `bin/` 自動加入 PATH）。
- [ ] Test case 3 — 執行 `bin/setup-machine.sh` 應能順利呼叫保留在 `scripts/` 內的 `update-personalization.sh` 等內部腳本。

## Success Criteria

- [ ] 所有列入轉移清單的核心指令都存在於 `bin/` 目錄中。
- [ ] 內部輔助腳本（如 `update-*`、`build-*`）依然安全地留在 `scripts/` 目錄中。
- [ ] 文件 `scripts.md` 已正確反映新架構，使用者知道去哪裡尋找指令。
- [ ] 所有的測試案例皆通過，無路徑中斷問題。

## Risks

- **相對路徑依賴斷裂風險**：腳本在計算本身所在目錄並向上推導根目錄時，如果沒有正確從 `bin/` 往上推導（原本是 `scripts/`），會導致無法載入 `scripts/common` 內的函式。需透過嚴格審查來避免。
- **跨平台差異**：PowerShell 和 Bash 腳本處理腳本路徑的語法不同，兩者都需要仔細驗證。

## Open Questions

- [ ] OQ-001 — 針對未來擴充：如果在 `bin/` 中的腳本數量增加，我們是否需要進一步區分供 Agent 使用的 `bin/agent/` 或維持全平坦結構？ *(raised by: planning)*

## Approval

- Human approval: [pending]
- Architect review: [pending]
- Additional domain review: [not requested]

## Review Results

### Architecture Review
Pending.

### Business Review
Pending.

### Design Review
Pending.

### Engineering Review
Pending.

## Test Plan
Pending.

## Tasks
Pending.
