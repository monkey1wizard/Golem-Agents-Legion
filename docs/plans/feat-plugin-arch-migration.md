# Plan: 遷移 CLI 供應商至 `.gal/plugins/gal` 架構

## Goal

將 AGY、Copilot 與 Codex 遷移至同一個 GAL 的標準外掛產出根目錄 `~/.gal/plugins/gal` 下，由單一的核心 Renderer 負責建置，同時保留各供應商已文件化的進入點與生命週期規則。實作過程必須每個任務僅處理一個供應商，且順序固定為：AGY、Copilot、Codex。

## References

- [Antigravity CLI Features](https://antigravity.google/docs/cli-features) - AGY 外掛需要一個暫存的外掛目錄於 `~/.gemini/antigravity-cli/plugins/<plugin_name>/` 下，並包含根目錄的 `plugin.json`，以及可選的 `mcp_config.json`、`skills/`、`agents/` 和 `rules/`。
- [OpenAI Codex Plugins Build Guide](https://developers.openai.com/codex/plugins/build) - Codex 外掛需要 `.codex-plugin/plugin.json`；外掛市場會將本地端外掛路徑解析為相對於市場根目錄的路徑。
- [GitHub Copilot CLI Plugin Reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-plugin-reference) - Copilot CLI 支援供應商的生命週期指令、根目錄或相容位置的 manifest、`agents/`、`skills/`、`commands` 以及 `.mcp.json`。

## Requirements

- [ ] 將以 Claude 命名的 renderer 替換為不特定於供應商的核心 renderer，命名為 `Build-CorePlugin.ps1` 與 `build-core-plugin.sh`。
- [ ] 核心 renderer 必須產出單一共享的 artifact root，而不是為各供應商建立獨立的套件樹，且該 root 必須包含所有供應商經過驗證載入所需的進入點。
- [ ] 共享的 artifact root 必須保持對 Claude 的相容性，並加入 AGY、Copilot 與 Codex 所需的供應商標記，但不引入特定於供應商的 renderer。
- [ ] 只有在 AGY 供應商任務更新了所有目前呼叫它們的腳本、測試與文件參照後，才能移除 `Build-AgyPlugin.ps1` 與 `build-agy-plugin.sh`。
- [ ] 更新 `Build-ProviderPlugins.ps1` 與 `build-provider-plugins.sh`，讓每個供應商都指向這個共享的核心產出 root，但只有在該供應商專屬的任務內，且生命週期檢查通過後，才能將其標記為 `implemented`。
- [ ] 每個實作任務只能完成一個供應商的遷移。任務順序是固定的：首先是 AGY，接著是 Copilot，最後是 Codex。
- [ ] 除非目前的 Codex CLI/文件已驗證其有效，否則 Codex 支援不可宣稱已啟用 agent。Codex 任務可以將 `agents/` 作為非活躍的共享內容保留在 root 內，但其成功的門檻在於支援組件的 Codex schema/生命週期相容性。

## Approach

### Step 1: AGY 供應商遷移
- **Provider**: 僅限 AGY
- **Files**: `scripts/Build-ClaudePlugin.ps1`, `scripts/build-claude-plugin.sh`, `scripts/Build-AgyPlugin.ps1`, `scripts/build-agy-plugin.sh`, `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`, `scripts/Update-Personalization.ps1`, `scripts/update-personalization.sh`, `scripts/Update-Skills.ps1`, `scripts/update-skills.sh`, `scripts/Update-Commands.ps1`, `scripts/update-commands.sh`, 以及 AGY 相關的測試與文件。
- **What**: 將 Claude renderer 重新命名為核心 renderer，讓它產出共享 root，並包含 AGY 需要的根目錄 `plugin.json`、`mcp_config.json`、`rules/gal.md`、`skills/` 與 `agents/`。接著移除獨立的 AGY renderer，並將 AGY 安裝投影路線導向這個共享 root。
- **Verify**: 進行 AGY 的 dry run 以及隔離的 home 目錄建置，證明 managed shortcut 正確指向共享的核心 root，舊的 AGY renderer 參照已被清除，且現有的 Claude artifact 檢查依然通過。

### Step 2: Copilot 供應商遷移
- **Provider**: 僅限 Copilot
- **Files**: `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`, `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `scripts/Test-BuildProviderPlugins.ps1`, `scripts/Test-InstallGalPlugins.ps1`, 以及 Copilot 發布與個人化文件。
- **What**: 將 Copilot 透過其原生的外掛生命週期，與同一個核心產出的 root 進行連接。確保 manifest 介面有明確暴露 Copilot 可讀的 `agents`、`skills`、`commands` 與 `mcpServers` 路徑，而不是依賴 Claude 的後備行為。
- **Verify**: 只有在本地路徑 install/list/update/uninstall 或已文件化的後備測試通過後，Copilot 的 dry run 才會回報使用核心 renderer 並進入 implemented 生命週期。

### Step 3: Codex 供應商遷移
- **Provider**: 僅限 Codex
- **Files**: `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`, `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `scripts/Test-BuildProviderPlugins.ps1`, `scripts/Test-InstallGalPlugins.ps1`, 可選的 Codex 市場中介資料, 以及 Codex 發布與個人化文件。
- **What**: 加入 Codex 的 `.codex-plugin/plugin.json` 以及將指向共享核心產出 root 的市場/生命週期連接起來。除非供應商在任務中驗證了 agents，否則將 Codex 的宣稱支援項目限制在它能驗證的組件上，特別是 skills 與 MCP。
- **Verify**: Codex 的 dry run 與供應商生命週期測試通過且沒有 schema 錯誤，且 Codex 僅能在此任務中從 `not-implemented` 變更為 `implemented`。

## Files to Create or Modify

- `[RENAME/MODIFY] scripts/Build-ClaudePlugin.ps1` - 變更為 `scripts/Build-CorePlugin.ps1` 並負責產出共享的核心 artifact root。
- `[RENAME/MODIFY] scripts/build-claude-plugin.sh` - 變更為 `scripts/build-core-plugin.sh` 以維持 Bash 同等功能。
- `[DELETE] scripts/Build-AgyPlugin.ps1` - 在 AGY 的參照轉移至核心 renderer 後移除。
- `[DELETE] scripts/build-agy-plugin.sh` - 在 AGY Bash 的參照轉移至核心 renderer 後移除。
- `[MODIFY] scripts/Build-ProviderPlugins.ps1` - 供應商的 build plan 與執行，現在每次會將單一供應商指向核心產出 root。
- `[MODIFY] scripts/build-provider-plugins.sh` - 供應商 build plan 與執行的 Bash 版本。
- `[MODIFY] scripts/Install-GalPlugins.ps1` 與 `scripts/install-gal-plugins.sh` - 安裝模式下的協作訊息與供應商生命週期呼叫。
- `[MODIFY] scripts/Update-Personalization.ps1`, `scripts/update-personalization.sh`, `scripts/Update-Skills.ps1`, `scripts/update-skills.sh`, `scripts/Update-Commands.ps1`, 與 `scripts/update-commands.sh` - 移除對 AGY renderer 的呼叫與過時的 dry-run 文字。
- `[MODIFY] scripts/Test-BuildProviderPlugins.ps1`, `scripts/Test-InstallGalPlugins.ps1`, 與 `scripts/Test-ProviderPluginPackage.ps1` - 更新供應商的斷言 (assertions) 與 artifact 檢查。
- `[MODIFY] scripts/scripts.md`, `docs/devguide.md`, `docs/personalization.md`, `docs/personalization.zh-Hant.md`, 與 `docs/release-matrix.md` - 在文件中記錄共享的核心 renderer 與各供應商的生命週期關卡。
- `[CREATE/MODIFY] 僅在 Codex 生命週期驗證需要時，建立 Codex 的市場中介資料。`

## Test Cases

- [ ] TP-001 - 執行 `pwsh -File scripts/Test-ProviderPluginPackage.ps1`；預期結果：供應商中立的套件模型，在具備 AGY、Copilot、Codex 與 Claude 功能旗標下仍能通過驗證。
- [ ] TP-002 - 執行 `pwsh -File scripts/Test-BuildProviderPlugins.ps1`；預期結果：AGY、Copilot、Codex 與 Claude 的供應商計畫，僅對已完成的任務回報核心 renderer 及正確的個別生命週期狀態。
- [ ] TP-003 - 執行 `pwsh -File scripts/Test-InstallGalPlugins.ps1`；預期結果：安裝模式的 dry run 顯示核心 renderer，且無殘留的 `Build-AgyPlugin` 或 `Build-ClaudePlugin` 文字。
- [ ] TP-004 - 執行 `pwsh -File scripts/Build-ProviderPlugins.ps1 -Providers agy -DryRun -PassThru`；預期結果：在 T-001 之後，AGY 計畫使用 managed shortcut 模式、核心 renderer、共享 canonical root 以及 implemented 狀態。
- [ ] TP-005 - 執行 `pwsh -File scripts/Build-ProviderPlugins.ps1 -Providers copilot -DryRun -PassThru`；預期結果：僅在 T-002 之後，Copilot 計畫使用原生生命週期模式、核心 renderer、共享 canonical root 以及 implemented 狀態。
- [ ] TP-006 - 執行 `pwsh -File scripts/Build-ProviderPlugins.ps1 -Providers codex -DryRun -PassThru`；預期結果：僅在 T-003 之後，Codex 計畫使用原生生命週期或市場模式、核心 renderer、共享 canonical root 以及 implemented 狀態。
- [ ] TP-007 - 在隔離的 `USERPROFILE` 中建置核心 artifact；預期結果：`~/.gal/plugins/gal` 應包含 `.claude-plugin/plugin.json`、任何 Copilot 要求的 root manifest 路徑、`.codex-plugin/plugin.json`、`skills/`、`commands/`、`agents/`、`.mcp.json`、AGY 的 `plugin.json`、AGY 的 `mcp_config.json`，以及依需求的 `rules/gal.md`。
- [ ] TP-008 - 若 Bash 依賴可用，執行 `bash scripts/build-provider-plugins.sh --providers agy --dry-run`，然後對 `copilot` 與 `codex` 重複此步驟；預期結果：Bash 的輸出與 PowerShell 的供應商狀態及 renderer 名稱一致。
- [ ] TP-009 - 若有供應商 CLI，僅對該任務所屬供應商執行原生 install/list/update/uninstall 的 smoke tests；預期結果：不應出現 schema 錯誤，且除了已文件化的快取/捷徑之外，不會在共享核心 root 以外建立特定於供應商的套件樹。
- [ ] TP-010 - 在已變更的文件上執行 Markdown diagnostics；預期結果：不應有 markdownlint 錯誤，且除非做為遷移歷史明確記錄，否則不應出現過時的供應商 renderer 名稱。

## Success Criteria

- [ ] AGY 成功使用共享的核心產出 artifact root，不再依賴 `Build-AgyPlugin.ps1` 或 `build-agy-plugin.sh`。
- [ ] Copilot 透過原生的外掛生命週期成功使用核心產出的 artifact root，並驗證其 agents、skills、commands 與 MCP 路徑。
- [ ] Codex 透過其文件化的外掛生命週期或市場流程成功使用核心產出的 artifact root，並在無 schema 錯誤的情況下驗證了受支援的組件。
- [ ] 只有在專屬的供應商任務通過後，`Build-ProviderPlugins.ps1` 與 `build-provider-plugins.sh` 才會將 AGY、Copilot 與 Codex 顯示為 implemented。
- [ ] 共享的 renderer 名稱與文件，不再暗示 Claude 獨佔核心 artifact 的架構。
- [ ] Claude 的基準行為，在 renderer 更名的過程中不能退步 (regress)。

## Risks

- **供應商進入點不相符 (Provider entrypoint mismatch)**：AGY、Copilot 與 Codex 並未使用完全相同的 manifest 進入點。唯有核心 renderer 產出每個供應商所需的標記時，共享的 root 才算有效。
- **虛假的 implementation 宣告**：在一次修改中切換所有 `LifecycleStatus` 會重演先前的風險，亦即在生命週期被驗證前，就把 Copilot 與 Codex 標記為完成。
- **移除 AGY renderer 後的參照斷裂**：`Update-Personalization`、`Update-Skills`、`Update-Commands`、測試及文件，目前都會直接呼叫或描述 `Build-AgyPlugin`。
- **過度宣稱 Codex agent 功能**：目前的 Codex 外掛文件以 skills、MCP、apps 與 hooks 為主。在 Codex 任務驗證其支援前，切勿將啟用 Codex agent 列為成功標準。
- **Force/Overwrite 行為**：使用一個共享的 artifact root 意味著重複建置必須維持一致的 `-Force` 行為以及對部分失敗的處理，確保某個供應商的任務不會損毀另一個供應商已經驗證過的 root。

## Open Questions

- [x] OQ-001 - 將 `Build-ClaudePlugin.ps1` 與 `build-claude-plugin.sh` 重新命名為 `Build-CorePlugin.ps1` 與 `build-core-plugin.sh`；這比將供應商中立的 artifact 保留為 Claude 命名的腳本還要清晰。*(raised by: planning, resolved by: architecture review)*
- [x] OQ-002 - 共用的 artifact root 可以包含特定於供應商的 manifest 標記，但特定於供應商的 renderer 則不在考量範圍內。應由一個 renderer 掌控所有靜態的 artifact 檔案。*(raised by: architecture review, resolved by: architecture review)*
- [x] OQ-003 - Codex agents 不算是強制的成功門檻，直到 Codex 生命週期驗證證明其支援為止。Codex 依然必須容忍共享的 `agents/` 目錄而不產生 schema 錯誤，否則 Codex 任務必須在 Codex manifest 中明確將其排除。*(raised by: architecture review, resolved by: architecture review)*

## Approval

- Human approval: [pending]
- Architect review: [clear]
- Additional domain review: [not triggered]

## Review Results

### Architecture Review

#### Verdict: APPROVE AFTER REVISION

原始計畫過於廣泛，因為它將 Claude 外掛的架構視為對所有供應商皆相容。目前的供應商文件顯示，共享 root 是可行的，但前提是它作為一個能容納各供應商特定 manifest 標記的超集 (superset) root。修訂後的計畫保留了架構目標，移除了特定於供應商的 renderer，並透過限制每個任務僅驗證一個供應商來防止虛假的完成宣告。

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| 將 Claude renderer 更名為核心 renderer | 將 Claude 的所有權從供應商中立的 artifact 移除 | 需要大量的參照與測試更新 | OK |
| 產出單一超集 artifact root | 避免 renderer 的分歧與重複的套件樹 | 核心 renderer 必須驗證多個 manifest schemas | OK |
| 以 AGY, Copilot, Codex 的順序來收尾各供應商 | 賦予每個供應商隔離的生命週期關卡 | 比一次性切換所有狀態還要慢 | OK |
| 保持 `LifecycleStatus` 的供應商範疇 | 防止尚未支援的 direct-install 宣稱 | Copilot 與 Codex 在它們專屬的任務通過前將維持 pending 狀態 | OK |
| 限制 Codex agent 的宣稱 | 符合現有的供應商文件及功能旗標 | Codex 未必能立即公開所有的共享組件 | OK |

#### Over-engineering Flags

- **OE-01** 如果供應商差異僅限於 manifest 檔案與安裝生命週期中介資料，則沒有必要為每個供應商建立獨立的 renderer。保留一個 renderer，並將供應商分支邏輯移到驗證/協作層中。
- **OE-02** 在三個具體的生命週期被證明可行之前，建立一個新的供應商安裝抽象層只會增加不必要的變動。在發生實質重複之前，請直接在 `Build-ProviderPlugins.*` 中使用各供應商區塊。

#### Bug Surface

- **BUG-01** 中：移除 `Build-AgyPlugin.*` 若沒有同時更新 `Update-Personalization`、`Update-Skills`、`Update-Commands`、測試以及文件，將會破壞 install 模式的重新整理功能。T-001 必須清除每一個生效中的參照。
- **BUG-02** 中：除非指令路徑是明確定義的，否則 Copilot 可能無法從 Claude 備用的 manifest 載入 `commands/`。T-002 必須驗證 Copilot 的 manifest 介面，而非僅檢查資料夾的存在。
- **BUG-03** 中：Codex 市場路徑會相對於市場根目錄進行解析。T-003 在宣稱生命週期支援前，必須先測試本地市場路徑的語意是否正確。
- **BUG-04** 低：當 root 已經存在時，針對各供應商重建共享 root 可能會失敗。核心 renderer 必須維持現有的 `-Force` 行為，且測試必須涵蓋重複建置的情境。

#### Performance Concerns

無。此計畫改變了靜態 artifact 的產生方式及本地 CLI 生命週期的檢查；它並未增加執行時期的背景服務或無限的資料載入。

#### Missing from Plan

修訂後無。剩下的不確定性在於實作期間供應商 CLI 的可用性，這應歸屬在任務層級的測試結果而非架構問題。

#### Recommended Changes

1. 優先實作 AGY，因為它是唯一具有獨立 renderer 的供應商，且有最高破壞風險。
2. 在 AGY 任務期間，即使共享 root 已經包含 Copilot 或 Codex 的 manifest 標記，也不要將其標記為 implemented。
3. 在更名 renderer 時，保留 Claude artifact 的相容性，然後更新測試以證明更名並未導致現有的基準功能退步。
4. 將 Codex 的支援限制在有文件的組件內，除非 Codex CLI 在 T-003 中能夠驗證 agents。

#### What's Good

- 計畫保留了 GAL 典範的 `~/.gal/plugins/gal` 所有權模型。
- 供應商的順序創造了小而容易審查的實作單元。
- renderer 的更名讓未來的維護更加直覺。

### Business Review

未觸發。計畫並未改變定價、權限、引導流程或客戶可見的商業規則。

### Design Review

未觸發。計畫並未改變 UI 佈局、視覺設計、元件或具無障礙要求的流程。

### Engineering Review

#### Verdict: CLEAR

架構修訂後，計畫已可建立。它定義了一個 renderer、一個 canonical artifact root、每個任務對應一個供應商，以及明確的供應商生命週期關卡。實作必須遵守依序任務的邊界：T-001 只能結束 AGY，T-002 只能結束 Copilot，而 T-003 只能結束 Codex。

受保護路徑提醒：此工作會影響 `scripts/` 及文件，但不需要更動 `commands/`、`conventions/`、`workflows/` 或 `templates/`，除非實作發現了新的跨 runtime 的合約變更。如果發生此情況，請先退回 `/deep-planning` 後再擴大範圍。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

在完成每個供應商的任務後，執行 `## Test Cases` 中的測試案例，但請確保只有當前供應商的生命週期狀態會在該任務中改變。現有的供應商狀態在輪到其專屬任務之前必須維持不變。

針對每個供應商任務的最低結案要求：

- PowerShell 中靜態供應商計畫的斷言 (assertion)。
- 在隔離的 home 目錄中進行核心 artifact 形狀的斷言。
- 安裝模式的 dry-run 斷言。
- 當供應商 CLI 可用時，進行供應商原生的生命週期 smoke 測試；若無法取得，請將缺失的 CLI 記錄為阻礙，且不要將該供應商標記為 implemented。
- 每一個變更過的 Markdown 檔案皆須通過 Markdown diagnostics。

## Tasks

- [ ] T-001 - AGY：將獨立的 AGY renderer 替換為核心 renderer，將 AGY 的安裝投影更新至共享 root，刪除 `Build-AgyPlugin.ps1` 與 `build-agy-plugin.sh`，並更新 AGY 的測試/文件。確認驗證 TP-001, TP-002, TP-003, TP-004, TP-007, TP-008 (若可用), TP-009 (若 AGY 可用), 及 TP-010。
- [ ] T-002 - Copilot：透過 Copilot 原生的外掛生命週期，將其與核心產出的 root 相連接，暴露 Copilot 可讀的組件路徑，並更新 Copilot 的測試/文件。確認驗證 TP-001, TP-002, TP-003, TP-005, TP-007, TP-008 (若可用), TP-009 (若 Copilot 可用), 及 TP-010。
- [ ] T-003 - Codex：透過 Codex 有文件記錄的外掛生命週期或市場流程，將其與核心產出的 root 相連接，加入 `.codex-plugin/plugin.json`，將 Codex 的組件支援宣稱限制在經過驗證的項目，並更新 Codex 的測試/文件。確認驗證 TP-001, TP-002, TP-003, TP-006, TP-007, TP-008 (若可用), TP-009 (若 Codex 可用), 及 TP-010。
