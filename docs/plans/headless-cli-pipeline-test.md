# Plan: GAL Headless CLI Pipeline Test & Default Model Migration

## Goal

1. **Headless CLI Pipeline 互派測試計畫**：規劃一個測試矩陣，逐個驗證 `docs/plans/headless-cli-pipeline.md` 在實作完成後，Claude, Codex, Copilot, OpenCode 這四個環境是否都能透過 pipeline 互相分配工作。
2. **預設模型 (Default Model) 設定檔轉移至 JSON**：將原先存放於 Markdown (`model-roles.md` / `model-roles.local.md`) 中的模型設定，改為 JSON 格式存放於 `~/.gal/config/`，以利未來程式讀取。

## Requirements

- **測試對象涵蓋度**：必須涵蓋 Claude, Copilot, Codex, OpenCode 互相派遣任務的能力。
- **無頭 (Headless) 執行限制**：依據前期規劃，Copilot CLI 與 Codex 刻意排除無頭執行能力，它們只能作為分派方 (Orchestrator)，不能作為接收執行方 (Executor)。若指定其為 Executor，須確保系統正確降級 (Fallback)。
- **精準目標模型**：各環境在測試時的指定模型如下，以確保測試與實際預期一致：
  - **OpenCode**: `DeepSeek V4 Flash`
  - **Copilot (發起方與接收方實測皆使用 CLI)**: `MAI-Code-1-Flash`
  - **Claude Code**: `haiku-4.5`
  - **Codex**: `5.4-mini`
- **專案環境一致性**：啟動各 AI 工具時，必須指定與目前相同的 Project / 工作目錄 (Working Directory)，以確保日後檢視歷史紀錄時，所有操作都能正確關聯到同一專案。

## Approach

### 階段一：設定檔 JSON 遷移
- **`model-config.example.json`**：在專案根目錄建立 JSON 範例檔，取代舊有的 `model-roles.example.md`。結構需包含 `defaultModel` 以及 `roles` 映射。
- **播種邏輯修改**：更新 `scripts/Update-Personalization.ps1` 與 `scripts/update-personalization.sh`，將 `model-config.example.json` 複製到 `~/.gal/config/model-config.json`，並移除舊有 `model-roles.example.md` 的複製與 git checkout tracking 邏輯。
- **備註**：暫不主動刪除舊有的 Markdown 檔案，僅調整系統的播種與讀取目標。

### 階段二：Pipeline 互派驗證
設定 `~/.gal/config/executor-routing.ndjson` 並執行以下測試矩陣，檢查終端機輸出與 `.dev/executor-logs/`：

| 測試案例 | Orchestrator (發起方) | 目標 Executor (接收方) | 預期分派模式與行為 |
| -------- | --------------------- | ---------------------- | ------------------ |
| **TC-01** | **Copilot CLI** | **Claude** | **Offload (背景執行)**：Copilot 啟動 Claude (`haiku-4.5`) 處理任務，並於結束後讀取 `receipt` 確認狀態。 |
| **TC-02** | **Copilot CLI** | **OpenCode** | **Offload (背景執行)**：Copilot 啟動 OpenCode (`DeepSeek V4 Flash`) 處理任務，檢驗 Task Spec 正確建立及清理。 |
| **TC-03** | **Copilot CLI** | **Codex** | **Fallback (降級)**：因 Codex 不支援無頭執行，應預期出現 `non-dispatchable` 並降級為文字分派。 |
| **TC-04** | **Claude** | **Copilot CLI** | **Fallback (降級)**：因 Copilot CLI 不支援無頭執行，觸發降級文字提示。 |
| **TC-05** | **Claude** | **OpenCode** | **Offload (背景執行)**：Claude (`haiku-4.5`) 啟動 OpenCode (`DeepSeek V4 Flash`)，確認雙方無頭互派正常。 |
| **TC-06** | **Claude** | **Codex** | **Fallback (降級)**：因 Codex 不支援無頭執行，觸發降級文字提示。 |
| **TC-07** | **OpenCode** | **Copilot CLI** | **Fallback (降級)**：同理，應觸發降級文字提示。 |
| **TC-08** | **OpenCode** | **Claude** | **Offload (背景執行)**：OpenCode (`DeepSeek V4 Flash`) 啟動 Claude (`haiku-4.5`) 驗證互派能力。 |
| **TC-09** | **OpenCode** | **Codex** | **Fallback (降級)**：同理，應觸發降級文字提示。 |
| **TC-10** | **Codex** | **Copilot CLI** | **Fallback (降級)**：同理，應觸發降級文字提示。 |
| **TC-11** | **Codex** | **Claude** | **Offload (背景執行)**：Codex (`5.4-mini`) 發起任務，背景啟動 Claude (`haiku-4.5`) 執行實作。 |
| **TC-12** | **Codex** | **OpenCode** | **Offload (背景執行)**：Codex (`5.4-mini`) 發起任務，背景啟動 OpenCode (`DeepSeek V4 Flash`) 執行實作。 |

## Files to Create or Modify

- `[NEW] docs/plans/headless-cli-pipeline-test.md` — 本計畫書。
- `[NEW] model-config.example.json` — 新的預設模型與角色 JSON 設定檔範例。
- `[MODIFY] scripts/Update-Personalization.ps1` — 調整播種邏輯以支援 JSON 設定檔。
- `[MODIFY] scripts/update-personalization.sh` — 調整 Bash 播種邏輯以支援 JSON 設定檔。

## Success Criteria

- 能夠成功執行 `Update-Personalization` 腳本，並在 `~/.gal/config/` 下產生 `model-config.json`。
- 測試矩陣的 TC-01 到 TC-07 都能在符合「專案環境一致性」與「精準目標模型」的條件下執行，且 Offload/Fallback 的實際行為與預期完全相符。
- 能夠在對應工具的歷史紀錄（包含 `.dev/executor-logs/`）中觀測到正確的派發足跡。

## Risks

- **CLI 參數變更**：外部工具 (如 Claude Code, OpenCode) 變更了指定 Project 或指定模型的參數，導致腳本啟動失敗。緩解：將參數封裝於 executor script 中並集中測試。
- **設定檔遷移斷層**：舊有使用者可能只保留了 Markdown 設定檔。緩解：可加入警告提示使用者手動建立 JSON，並提供 `model-config.example.json` 供參考。
