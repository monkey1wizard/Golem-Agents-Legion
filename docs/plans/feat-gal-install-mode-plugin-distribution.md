# 計畫：GAL 安裝模式外掛程式發佈

## 目標

GAL 提供一套以供應商原生 plugin install 機制為主的安裝模式，讓一般使用者不需要 clone GAL repo，也不需要在 `~/.copilot/gal`、`~/.gemini/gal` 或 `~/.gemini/antigravity-cli/gal` 建立 repo-root shortcut 才能使用 GAL。現有 clone repo 加 symlink 的方式保留為 contributor/developer mode，不再是一般使用者的預設安裝路徑。

同時將 C# / dart / flutter / golang 等等特定語言的 skills 移出GAL，改為使用 official plugin 的形式安裝，畢竟不同人需要的開發語言 skill 不同。

## 需求

- [ ] 比較 AGY CLI、Copilot CLI、Codex 與 Claude Code 的 plugin 架構、安裝方式、cache 行為、manifest 位置、支援承載與不支援承載。
- [ ] 定義新的 install mode，使用 provider-native plugin install、marketplace、plugin cache 或指定 plugin 目錄，而不是 provider 目標目錄下的 repo-root shortcut。
- [ ] 保留 source mode 作為 GAL 開發者流程，明確標示其 `GAL_ROOT` shortcut 只屬於本機開發與相容橋接。
- [ ] 將 `~/.copilot/gal`、`~/.gemini/gal`、`~/.gemini/antigravity-cli/gal` 視為 source mode 遺留橋接，不得出現在 install mode 的成功條件內。
- [ ] 將 GAL source contracts 轉成各 provider 可安裝的 plugin artifacts；生成品不得依賴原始 repo 的絕對路徑。
- [ ] 保持 provider-specific renderer 邊界：每個 provider 只輸出該 provider 文件支援的 manifest 欄位與元件。
- [ ] 不把 `scripts/` 提升為四 provider 共同承載。需要可執行能力時，使用 provider 原生能力、skill-local helper、hooks、`bin/`，或獨立的 GAL runtime install，而不是 provider 共用 `scripts/` 欄位。
- [ ] GAL-managed MCP 預設由 plugin artifact 或 provider-native plugin MCP 機制承載；local secrets 與 machine-local paths 只在本機 install/enable 階段解析。
- [ ] 安裝、更新、解除安裝必須可重跑且可驗證，不得刪除使用者自有 provider 設定。
- [ ] 文件必須清楚區分 install mode、source mode、migration cleanup 與 provider-native limitations。

## 方法

### 步驟 1：鎖定供應商外掛程式比較

- **檔案**：`docs/devguide.md`、`docs/personalization.md`、`docs/personalization.zh-Hant.md`、`scripts/scripts.md`
- **內容**：內聯四個 provider 的 plugin 架構比較，作為後續 renderer 與 installer 的規格來源。
- **驗證**：文件中有一張 provider matrix，至少覆蓋 manifest、install command/path、cache/update、skills、commands、agents、MCP、hooks/scripts/bin 與主要缺口。

#### 供應商架構比較

| 供應商 | 安裝介面 | 清單 | 支援元件 | 執行階段/腳本形態 | 快取/更新模型 | GAL 安裝模式意涵 |
| --- | --- | --- | --- | --- | --- | --- |
| AGY CLI | 外掛程式目錄發現；在實證冒煙驗證後將此計畫目標定為 `~/.gemini/antigravity-cli/plugins/gal/` | `plugin.json` 在外掛程式根目錄 | `skills/`、`rules/`、`mcp_config.json`、`hooks.json`；遷移文件也提到 Gemini 指令將變成技能 | 未記錄外掛程式根目錄有 `scripts/`；有記錄技能本地輔助腳本 | 在取得的文件中是目錄發現，而非市集安裝指令 | 直接生成 AGY 外掛程式目錄；從本計畫移除舊版 Gemini CLI 的 `~/.gemini/config/plugins/` 介面 |
| Copilot CLI | `copilot plugin install SPECIFICATION`；直接路徑、GitHub 儲存庫/子目錄、Git URL、市集 | `plugin.json`、`.plugin/plugin.json`、`.github/plugin/plugin.json` 或 `.claude-plugin/plugin.json` | `agents/`、`skills/`、`commands`、`hooks`、`.mcp.json`、LSP 設定 | 沒有供應商中立的外掛程式根目錄 `scripts`；指令與 hooks 是供應商欄位，而非共用的執行階段承載 | 安裝於 `~/.copilot/installed-plugins/...`；本地變更需重新安裝 | 繼 AGY 之後的首要早期安裝模式目標，因為它可從 repo/subdir 安裝且直接消除 `~/.copilot/gal` 的需求 |
| Codex | 外掛程式目錄 UI 加上市集；`codex plugin marketplace add` 用於本機/Git 市集來源 | `.codex-plugin/plugin.json` | `skills/`、`.mcp.json`、`.app.json`、`hooks/`、`assets/`；應用程式整合與 MCP 為一等公民 | Hooks 透過 `PLUGIN_ROOT` 可呼叫外掛程式根目錄檔案；清單中無頂層 `scripts/` 元件 | 安裝至 `~/.codex/plugins/cache/<marketplace>/<plugin>/<version>/`；啟用狀態在 `~/.codex/config.toml` | 生成 Codex 外掛程式與市集項目；若外掛程式文件未暴露則不捏造代理程式支援 |
| Claude Code | `claude plugin install <plugin>` 從市集安裝，包含使用者/專案/本地/受管理範圍 | `.claude-plugin/plugin.json` 為可選 | `skills/`、`commands/`、`agents/`、`hooks/`、`.mcp.json`、`.lsp.json`、監視器、主題、輸出樣式、`bin/` | 最強大的執行階段支援：`${CLAUDE_PLUGIN_ROOT}`、`${CLAUDE_PLUGIN_DATA}`、`bin/`、`scripts/`、hooks 與監視器 | 市集外掛程式被複製到 `~/.claude/plugins/cache`；由版本控制更新；解除安裝可移除外掛程式資料 | 渲染器可以最豐富，但 Claude 專屬的執行階段功能不得外洩至共用模型 |

### 步驟 2：定義安裝模式與來源模式

- **檔案**：`scripts/common/Common.ps1`、`scripts/common/common.sh`、`scripts/Setup-Machine.ps1`、`scripts/setup-machine.sh`、`scripts/Uninstall-Machine.ps1`、`scripts/uninstall-machine.sh`
- **內容**：新增明確的安裝模式邊界。安裝模式使用供應商原生的外掛程式成品，且不得建立供應商 `GAL_ROOT` 儲存庫捷徑。來源模式則保留目前符號連結行為供貢獻者與本地開發使用。
- **驗證**：試跑（dry run）能顯示安裝模式與來源模式的不同行動計畫。安裝模式不計劃建立 `~/.copilot/gal`、`~/.gemini/gal` 或 `~/.gemini/antigravity-cli/gal`。

### 步驟 3：新增供應商成品建置根目錄

- **檔案**：`scripts/common/ProviderPlugin.ps1`、`scripts/common/provider-plugin.sh`、`scripts/Build-ProviderPlugins.ps1`、`scripts/build-provider-plugins.sh`
- **內容**：將供應商專屬安裝成品建置到非 `gal-results/` 的生成位置，因為 `gal-results/` 屬於 xmachine 輸出。候選的被忽略根目錄：`dist/provider-plugins/<provider>/gal/`。
- **驗證**：生成的成品具決定性，不包含儲存庫根目錄的符號連結，且可在不更動 xmachine 輸出的情況下被刪除/重建。

### 步驟 4：取代儲存庫根目錄的指令串接

- **檔案**：`commands/*/SKILL.template.md`、`scripts/Update-Commands.ps1`、`scripts/update-commands.sh`、新供應商渲染器輔助程式
- **內容**：停止將絕對路徑 `{{GAL_ROOT}}` 烘焙進安裝模式的指令技能中。安裝模式指令必須呼叫供應商本地的外掛程式能力，或是穩定的已安裝 GAL 執行階段進入點（例如置於 GAL 管理的執行階段安裝根目錄下的 `gal` 指令）。
- **驗證**：生成的安裝模式指令技能不包含指向來源簽出目錄的絕對路徑，也不包含 `~/.copilot/gal` 或 `~/.gemini/gal` 的參考。

### 步驟 5：實作供應商原生安裝程式

- **檔案**：`scripts/Install-GalPlugin.ps1`、`scripts/install-gal-plugin.sh`、供應商渲染器腳本、設定腳本
- **內容**：為每個供應商新增安裝/更新/解除安裝操作：
  - AGY：在發現冒煙驗證後，於 `~/.gemini/antigravity-cli/plugins/gal/` 下具現化外掛程式目錄。
  - Copilot CLI：對生成的本地成品或儲存庫/子目錄來源執行 `copilot plugin install`。
  - Codex：生成市集項目，並在可用情況下透過 Codex 外掛程式市集流程安裝/啟用。
  - Claude Code：使用要求的範圍透過 `claude plugin install` 從市集或本地外掛程式來源安裝。
- **驗證**：每次供應商安裝皆可透過其原生指令或有記錄的發現介面，列出或檢查已安裝的 GAL 外掛程式。

### 步驟 6：將 MCP 移至可感知外掛程式的所有權

- **檔案**：`mcp.json`、`mcp.local.json`、`scripts/Update-Mcp.ps1`、`scripts/update-mcp.sh`、供應商渲染器
- **內容**：將規範 MCP 定義與供應商安裝實現分開。支援外掛程式的供應商若情況允許將接收外掛程式擁有的 MCP 設定；舊版全域設定清理只會移除來源模式中 GAL 管理的項目。
- **驗證**：使用者擁有的 MCP 項目保持不變。安裝模式輸出不會將機密序列化進入可分享的外掛程式成品中。

### 步驟 7：新增遷移清理

- **檔案**：`scripts/Setup-Machine.ps1`、`scripts/setup-machine.sh`、`scripts/Uninstall-Machine.ps1`、`scripts/uninstall-machine.sh`、`scripts/scripts.md`
- **內容**：在所選供應商的安裝模式通過驗證後，提供一條清理路徑來移除舊有 GAL 管理的儲存庫根目錄捷徑。清理必須是選擇性的（opt-in）或受安裝成功所約束。
- **驗證**：清理只移除解析到本 GAL 儲存庫的連結，並保留使用者擁有的資料夾或無關的符號連結。

### 步驟 8：記錄使用者安裝流程

- **檔案**：`README.md`、`README.zh-Hant.md`、`docs/personalization.md`、`docs/personalization.zh-Hant.md`、`docs/devguide.md`
- **內容**：將安裝模式記錄為預設使用者路徑、來源模式記錄為貢獻者路徑，並誠實記錄供應商限制。
- **驗證**：新使用者能夠為單一供應商安裝 GAL，而無需 clone 本儲存庫或建立供應商 `gal` 捷徑。

## 需建立或修改的檔案

- `docs/devguide.md` - 供應商外掛程式架構比較與安裝模式架構筆記。
- `docs/personalization.md` - 面向使用者的安裝模式與來源模式指南。
- `docs/personalization.zh-Hant.md` - 繁體中文版指南。
- `scripts/scripts.md` - 設定/安裝/解除安裝責任更新。
- `scripts/common/ProviderPlugin.ps1` - PowerShell 供應商成品清單與驗證輔助程式。
- `scripts/common/provider-plugin.sh` - Bash 供應商成品清單與驗證輔助程式。
- `scripts/Build-ProviderPlugins.ps1` - PowerShell 建置供應商外掛程式成品的進入點。
- `scripts/build-provider-plugins.sh` - Bash 建置供應商外掛程式成品的進入點。
- `scripts/Install-GalPlugin.ps1` - PowerShell 安裝模式供應商安裝程式。
- `scripts/install-gal-plugin.sh` - Bash 安裝模式供應商安裝程式。
- `scripts/Setup-Machine.ps1` - 可感知模式的編排與遷移清理約束。
- `scripts/setup-machine.sh` - Bash 對應的可感知模式編排。
- `scripts/Uninstall-Machine.ps1` - 解除安裝安裝模式成品與選用的來源模式連結。
- `scripts/uninstall-machine.sh` - Bash 對應的解除安裝。
- `scripts/Update-Commands.ps1` - 停止讓安裝模式的指令技能依賴烘焙的來源簽出路徑。
- `scripts/update-commands.sh` - Bash 的指令生成對等程式。
- `scripts/Update-Mcp.ps1` - 具備外掛程式感知能力的 MCP 所有權與舊版清理邊界。
- `scripts/update-mcp.sh` - Bash 對應的 MCP。

## 測試案例

- [ ] 供應商比較表涵蓋 AGY CLI、Copilot CLI、Codex 與 Claude Code 的安裝介面、清單、元件、執行階段/腳本形態、快取/更新模型與 GAL 意涵。
- [ ] 針對 Copilot 的安裝模式試跑（dry run）不建立 `~/.copilot/gal`，而是使用 `copilot plugin install` 或等效的供應商原生外掛程式安裝路徑。
- [ ] 針對 AGY 的安裝模式試跑不建立 `~/.gemini/gal` 或 `~/.gemini/antigravity-cli/gal`，僅寫入所選的 AGY 外掛程式成品路徑以及受管理的清理行動。
- [ ] 針對 Codex 的安裝模式試跑生成外掛程式成品及市集項目，且不建立在 `~/.codex/skills/` 下的指令技能符號連結。
- [ ] 針對 Claude 的安裝模式試跑使用 `claude plugin install` 或本地外掛程式來源，不寫入直接的 `~/.claude/skills/*` 符號連結。
- [ ] 來源模式在明確選擇時，仍會為貢獻者建立目前的儲存庫根目錄捷徑。
- [ ] 生成的安裝模式指令技能不包含 `{{GAL_ROOT}}`、來源簽出絕對路徑或供應商 `gal` 捷徑路徑。
- [ ] 生成的供應商成品不包含指向成品根目錄之外的符號連結。
- [ ] 具備外掛程式感知能力的 MCP 輸出保留使用者擁有的全域 MCP 項目，並僅在遷移清理期間移除 GAL 管理的舊版項目。
- [ ] 針對安裝模式的解除安裝會移除供應商原生的 GAL 外掛程式成品，但不刪除使用者擁有的供應商設定。

## 成功標準

- [ ] 一般使用者可透過供應商原生安裝模式安裝 GAL，無須 clone 本儲存庫。
- [ ] `.copilot/gal`、`.gemini/gal` 與 `.gemini/antigravity-cli/gal` 捷徑不再為安裝模式使用所需。
- [ ] 來源模式仍可供 GAL 貢獻者使用，並被記錄為開發工作流程。
- [ ] 供應商成品符合每個供應商有記錄的外掛程式架構，而非偽造的共用目錄佈局。
- [ ] `gal-results/` 不被用於供應商外掛程式成品。
- [ ] 安裝模式指令技能不嵌入絕對的來源簽出路徑。
- [ ] MCP 所有權明確、具備外掛程式感知能力，且對於本地機密是安全的。
- [ ] 遷移清理精準，且僅移除 GAL 管理的舊版捷徑或設定項目。

## 風險

- 如果指令技能仍需要來源簽出腳本，安裝模式將只會隱藏捷徑問題，而非解決它。
- 如果供應商成品複製了來自其他供應商不支援的欄位，安裝可能成功但無聲地丟失重要的 GAL 行為。
- 如果 AGY 外掛程式發現路徑與本計畫的目標不同，AGY 安裝模式在成為預設前可能需要供應商專屬的回退方案。
- 如果 MCP 本地值渲染過早，安裝成品可能會洩漏機器本地機密或路徑。
- 如果清理在原生外掛程式安裝被驗證前即執行，使用者可能會遺失可運作的 GAL 指令。
- 如果來源模式與安裝模式共用太多程式碼而沒有清晰的模式檢查，未來的設定變更可能會意外地在安裝模式中重建 `GAL_ROOT` 捷徑。

## 待確認問題

- [ ] OQ-001 — 目前 GAL 使用的 AGY CLI 建置是否能確切發現 `~/.gemini/antigravity-cli/plugins/gal/`，還是它儘管從本計畫中移除舊路徑，仍需要另一個不同的目前外掛程式根目錄？ *(提出者：planning)*
- [ ] OQ-002 — 安裝模式應該在 `~/.gal/` 下包含獨立的 GAL 執行階段安裝，還是要求每個供應商外掛程式成品都獨立包含而無共用的執行檔進入點？ *(提出者：planning)*
- [ ] OQ-003 — 關於 Copilot CLI，是否有一份有記錄的相當於 Claude `${CLAUDE_PLUGIN_ROOT}` 或 Codex `PLUGIN_ROOT` 的外掛程式根目錄執行階段變數，抑或是 GAL 必須避免在 Copilot 指令技能中執行外掛程式本地腳本？ *(提出者：planning)*
- [ ] OQ-004 — 安裝模式應該在一個供應商通過驗證後立即成為預設，還是等到 AGY 與 Copilot 雙雙通過安裝模式冒煙測試後才成為預設？ *(提出者：planning)*

## 簽核

- 人工核准：[待處理]
- 架構審查：[待處理]
- 額外領域審查：[未請求]

## 審查結果

### 架構審查

待處理。本計畫改變了受保護的設定腳本的安裝拓撲，實作前應經過 `/deep-planning` 流程。

### 商業審查

待處理。除非安裝模式變成公開的發佈或市集發布承諾，否則無須進行。

### 設計審查

待處理。不需要；沒有在範圍內的 UI 介面。

### 工程審查

待處理。

## 測試計畫

待處理。

## 任務

待處理。
