# Plan: 將 GAL 安裝架構遷移至 `~/.gal`

## Goal

將 GAL 的安裝檔、生成的供應商外掛（provider plugins），以及個人化設定從暫時的 `$RepoRoot/dist` 及 Repository 根目錄遷移至一個集中管理的 `~/.gal` 目錄中。這將確保 Git 儲存庫保持乾淨，並正式確立安裝架構。

## Requirements

- [ ] 所有 GAL 生成的 provider plugin 必須存放在 `~/.gal/dist`（或相對應的供應商目錄）。
- [ ] 所有 AI 工具（Claude, Codex, Copilot, Gemini）必須透過 shortcut/symlink 指向 `~/.gal` 內的對應資料夾。
- [ ] Repository 根目錄不得包含任何個人化設定檔（如 `*.local.*`）。
- [ ] 個人化設定檔必須被讀取與寫入至 `~/.gal/config`。

## Approach

### Step 1: 建立並切換至 `~/.gal` 結構
- **Files**: `scripts/common/Common.ps1`
- **What**: 重新定義變數（如 `GalGeneratedRoot`、`AgyPluginArtifactRoot`、`ClaudePluginArtifactRoot` 及各種設定檔路徑），使其指向 `~/.gal/dist` 與 `~/.gal/config`，取代原有的 `$RepoRoot/dist` 與 Repository 根目錄。
- **Verify**: 腳本內的路徑變數皆正確解析至使用者的 `~/.gal` 目錄下。

### Step 2: 更新建置與安裝腳本
- **Files**: `scripts/Build-ProviderPlugins.ps1`, `scripts/common/ProviderPlugin.ps1`, `scripts/Install-GalPlugins.ps1`
- **What**: 調整外掛建置的輸出目錄至新的 `~/.gal/dist`，並將建立給 AI 工具的捷徑（shortcut/symlink）目標也更新至 `~/.gal/dist/...`。
- **Verify**: 執行建置腳本後，檔案會正確出現在 `~/.gal/dist`，而捷徑也能正確運作。

### Step 3: 遷移與清理 Repository 根目錄
- **Files**: `Repository 根目錄`
- **What**: 由於 `~/.gal` 內部已經有一份完整的 personalized settings，腳本完全不需要處理搬移邏輯，只需修改路徑讓系統直接讀取 `~/.gal/config` 即可。然後從 Repository 工作區中移除舊的 `dist` 目錄以及任何 `.local.` 檔案。
- **Verify**: `git status` 顯示沒有任何未追蹤的 `.local.` 檔案，且 `dist` 資料夾不再由建置腳本在 repo 中生成。

## Files to Create or Modify

- `scripts/common/Common.ps1` — 重新定義 GAL 目錄與設定檔的路徑。
- `scripts/common/ProviderPlugin.ps1` — 修改與 `dist` 相關的路徑邏輯。
- `scripts/Build-ProviderPlugins.ps1` — 更新生成的 provider plugin 儲存路徑。
- `scripts/Install-GalPlugins.ps1` — 更新 AI 工具的安裝/捷徑目標路徑。

## Test Cases

- [ ] 執行 `Setup-Machine.ps1` → 生成的檔案正確落於 `~/.gal/dist` 與 `~/.gal/config`。
- [ ] 啟動 Claude/Gemini CLI → 能順利載入位於 `~/.gal/dist/provider-plugins/` 的 GAL 外掛。

## Success Criteria

- [ ] `~/.gal/dist` 內包含所有預期生成的 provider plugins。
- [ ] `~/.gal/config` 內存放所有的 `*.local.*` 個人化設定。
- [ ] Repository 根目錄保持完全乾淨（無 `dist` 與 `*.local.*`）。

## Risks

- 遷移設定檔可能導致現有使用者的舊設定抓不到，需要安裝腳本提供自動遷移或是明確的警告提示。（已透過 OQ-001 確認不需搬移）
- AI 工具（如 Claude Code）對外掛路徑可能有特殊要求，需確保 symlink 能夠被正常解析。

## Open Questions

<!-- Resolved: - [x] OQ-001 — 存在於 Repository 根目錄的舊版個人化設定檔（`*.local.*`），應該由安裝腳本自動搬移，還是直接忽略並交由使用者手動搬移？ *(raised by: planning, resolved by: user)* -->

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
