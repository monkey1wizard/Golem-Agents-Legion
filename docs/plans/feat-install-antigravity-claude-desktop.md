# Plan: Install GAL into Antigravity and Claude Desktop

## Goal

本計畫的目標是將 GAL (Golem-Agents-Legion) 整合並安裝至 **Antigravity CLI** 與 **Claude Desktop** 環境中。為求最佳的整合體驗與自動化更新管理，安裝策略將採取「優先嘗試原生外掛安裝 (Plugin Install)」，若該方式受環境或工具限制無法成功，再「退而求其次使用安裝腳本 (Script Install)」作為備案。

## Requirements

- [ ] 優先嘗試將 GAL 封裝為標準外掛（運用既有的 `.gal/plugin` 架構）並透過 Antigravity CLI 與 Claude Desktop 的原生外掛管理機制進行安裝。
- [ ] 若外掛安裝失敗，備妥 Fallback 機制，透過 PowerShell/Bash 腳本直接修改對應工具的設定檔（例如注入 `.mcp.json` 或修改全域 config）來完成手動註冊。
- [ ] 確保在兩個環境中，GAL 的 MCP Server、Agents、Skills 與 Commands 皆能順利被載入與執行。

## Approach

### Step 1: 嘗試透過 Plugin 安裝 Antigravity
- **Method**: 優先使用 Antigravity 原生的外掛安裝指令 (例如透過 CLI 指令引入 GAL 的 `plugin.json` 或對應設定)。
- **Verify**: 開啟 Antigravity 並確認是否成功載入 GAL 的 skills 與 MCP 設定。
- **Fallback**: 若不支援，則透過腳本修改 Antigravity 的設定檔（例如 `mcp.json` 或是將 `AGENTS.md` 自動注入至環境）。

### Step 2: 嘗試透過 Plugin 安裝 Claude Desktop
- **Method**: 根據 Claude Desktop 的外掛規範，嘗試將 GAL 作為外掛載入。
- **Verify**: 在 Claude Desktop 中測試 GAL 的 Agent 行為與 MCP Server 啟動狀態。
- **Fallback**: 透過 `Install-GalPlugins.ps1` 或是新增特定的設定腳本，直接修改 Claude Desktop 的 `claude_desktop_config.json` 來註冊 GAL 的 MCP 伺服器與相關設定。

### Step 3: 開發與完善安裝腳本 (Fallback Scripts)
- **Method**: 如果有任一環境必須依賴腳本安裝，需確保腳本支援跨平台 (Windows/macOS/Linux)，且在執行時能避免覆蓋使用者現有的其他自訂設定。

## Files to Create or Modify

- `[NEW/MODIFY] scripts/Install-AntigravityPlugin.ps1` & `.sh` (或整合至現有的 `Install-GalPlugins.ps1`)
- `[NEW/MODIFY] scripts/Install-ClaudeDesktopPlugin.ps1` & `.sh`
- `[MODIFY] docs/release-matrix.md`：記錄 Antigravity 與 Claude Desktop 的安裝支援度。

## Test Cases

- [ ] Test case 1 — 驗證 Antigravity 原生 Plugin 安裝流程是否成功，若失敗則驗證腳本安裝流程。
- [ ] Test case 2 — 驗證 Claude Desktop 原生 Plugin 安裝流程是否成功，若失敗則驗證設定檔注入腳本 (`claude_desktop_config.json`)。
- [ ] Test case 3 — 安裝完成後，在兩個工具中分別呼叫 GAL 的基本指令（如 `/gal status`）確認是否運作正常。

## Success Criteria

- [ ] 成功將 GAL 註冊進 Antigravity 與 Claude Desktop。
- [ ] 若外掛安裝法無法運作，腳本備案能 100% 順利接手並完成設定。
- [ ] 使用者在這些環境內能無縫使用所有 GAL 功能。

## Risks

- **設定檔覆蓋風險**：在 Fallback 使用腳本安裝時，修改 `claude_desktop_config.json` 或 Antigravity 設定可能會不慎清空使用者既有的設定，需實作安全的 JSON 合併邏輯。
- **Plugin API 變動**：Antigravity 或 Claude Desktop 的原生外掛介面若仍在快速迭代中，可能會導致外掛安裝失敗率較高。

## Open Questions

- [ ] OQ-001 — Claude Desktop 與 Antigravity 目前是否已完全開放公開的 Plugin Install API？ *(raised by: planning)*
- [ ] OQ-002 — Fallback 腳本是否應直接整併進 `scripts/Setup-Machine.ps1` 中，還是維持獨立安裝檔？ *(raised by: planning)*

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
