# Plan: Migrate CLI providers to .gal/plugin architecture

## Goal

目前 GAL 已經完成 Claude plugin 的架構遷移（將產出物統一渲染至 `.gal/plugin` 或特定的 canonical root，並透過 `Build-ClaudePlugin.ps1` 進行建置）。經過確認，Claude plugin 的資料夾結構 (`.claude-plugin/plugin.json`, `skills/`, `commands/`, `agents/`, `.mcp.json`) 已相容於 `agy`、`codex` 與 `copilot`。因此，本計畫的目標是讓 `agy-cli`、`codex` 與 `copilot` 直接共用此統一架構，無需為個別 provider 開發專屬的 renderer，從而達到一套架構、多方支援的目標。

## References / 參考資料

- [Antigravity CLI Features](https://antigravity.google/docs/cli-features)
- [OpenAI Codex Plugins Build Guide](https://developers.openai.com/codex/plugins/build)
- [GitHub Copilot CLI Reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-plugin-reference)

## Requirements

- [ ] 移除或廢棄個別的 renderer（如現有的 `Build-AgyPlugin.ps1`），因為所有支援的 CLI 工具都能直接使用 `.gal/plugins/` 下的結構。
- [ ] 調整 `Build-ClaudePlugin.ps1`，將其重新命名或定位為通用的 `Build-CorePlugin.ps1`（或其他更泛用的名稱），反映其產出物適用於所有 providers。
- [ ] 更新 `scripts/Build-ProviderPlugins.ps1`（及 `.sh` 版本），將 `agy`、`codex` 與 `copilot` 的設定指向共用的 renderer 產出物，並將它們的 `LifecycleStatus` 從 `not-implemented` 變更為 `implemented`。
- [ ] 確保每個 CLI 工具都能在載入時，直接讀取並套用此通用外掛結構（skills, commands, agents, mcp）。

## Approach

### Step 1: 泛用化 Renderer 腳本
- **Files**:
  - `scripts/Build-ClaudePlugin.ps1` / `.sh` -> `scripts/Build-CorePlugin.ps1` / `.sh` (或類似的通用命名)
- **What**: 將目前專屬於 Claude 的 renderer 腳本重新命名，並檢查內部邏輯，確認產出的 manifest (`plugin.json` 等) 在 `agy`、`codex` 與 `copilot` 環境中亦可被正確辨識與掛載。

### Step 2: 移除多餘的 Renderer
- **Files**:
  - `scripts/Build-AgyPlugin.ps1` / `.sh`
- **What**: 刪除 `Build-AgyPlugin.ps1`，因為 `agy-cli` 已經可以直接掛載通用的 `.gal/plugin` 結構。

### Step 3: 更新 Provider 註冊表
- **Files**:
  - `scripts/Build-ProviderPlugins.ps1`
  - `scripts/build-provider-plugins.sh`
- **What**: 更新 `Build-ProviderPlugins.ps1`，讓所有 provider (Claude, Agy, Codex, Copilot) 都依賴這個共用的 canonical root。調整 `copilot` 與 `codex` 的狀態為 `implemented`。

### Step 4: 更新測試與文件
- **Files**:
  - 相關的測試腳本 (例如 `scripts/Test-BuildProviderPlugins.ps1`)
  - `docs/release-matrix.md`
- **What**: 加入方針說明，指出 GAL 目前採用單一 artifact structure 支援所有 CLI provider，並更新自動化測試確保設定正確無誤。

## Files to Create or Modify

- `[DELETE] scripts/Build-AgyPlugin.ps1` & `.sh`
- `[MODIFY/RENAME] scripts/Build-ClaudePlugin.ps1` & `.sh` -> 改名為通用的 Renderer 腳本
- `[MODIFY] scripts/Build-ProviderPlugins.ps1` & `.sh`
- `[MODIFY] 相關的 Test 腳本與文件`

## Test Cases

- [ ] Test case 1 — 執行通用 renderer 後，檢查 `.gal/plugins/gal/` 內產生的結構是否完整。
- [ ] Test case 2 — 透過 `Build-ProviderPlugins.ps1` 安裝時，各個 provider (`claude`, `agy`, `codex`, `copilot`) 皆能無錯誤地將其指標指向 `.gal/plugins/gal/`。
- [ ] Test case 3 — 驗證 `copilot` 與 `codex` 環境實際讀取此架構時，能成功解析 agents 與 skills 而不會發生 schema 錯誤。

## Success Criteria

- [ ] `agy-cli`、`codex` 與 `copilot` 成功共用與 `claude` 完全相同的外掛資料夾結構。
- [ ] 專案中不再有冗餘的各別 provider renderer 腳本。
- [ ] 所有相關工具均能將 `LifecycleStatus` 標記為 `implemented`。

## Risks

- **CLI 解析限制差異**：雖然結構理論上相容，但各 CLI（特別是 Copilot 或 Codex）解析 `plugin.json` 與 markdown agents 時的嚴格程度可能有異，如果遭遇不相容欄位，可能需要微調通用 manifest。

## Open Questions

- [ ] OQ-001 — 是否需要將 `Build-ClaudePlugin.ps1` 正式更名為 `Build-CorePlugin.ps1` (或 `Build-GalPlugin.ps1`) 來明確反映這是給所有 CLI 用的通用結構？ *(raised by: planning)*

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
