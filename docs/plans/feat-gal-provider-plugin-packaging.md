# 計畫：GAL 供應商外掛程式封裝

## 目標

GAL 建立一套以 AGY CLI、Copilot CLI、Codex 與 Claude Code 四個供應商共同最小基底為核心的供應商外掛程式封裝架構。GAL 原始碼合約保持為唯一的事實來源，供應商外掛程式為生成的安裝成品。AGY 僅為第一個渲染器與遷移目標；後續順序固定為 Copilot CLI、Codex、Claude Code。共同模型必須能重用於四者，而不是把 AGY 佈局當成架構本身。

供應商外掛程式應把 GAL 管理的 MCP 檔案預設安裝在各自 plugin root 底下。共同模型不保留供應商輸出路徑，也不保留已解析的 machine-local 機密或路徑值。`scripts/` 不是四個 provider 共同支援的 plugin root 承載，因此不屬於 provider-neutral package model；若未來特定 provider 需要可執行 helper，必須透過該 provider 原生機制處理，例如 skill-local helper scripts、hooks、`bin/` 或獨立 runtime install。

對 AGY CLI 而言，本計畫將其視為全新的 plugin-only 安裝模式。所有既有的 GAL 管理 AGY 內容，不論是舊的 skills 連結、外部 runtime 路徑、全域 MCP 片段或其他散落在 plugin root 之外的受管資產，都必須在 setup/reinstall 時移除，並改由 `~/.gemini/antigravity-cli/plugins/gal/` 內的單一 plugin tree 承載。

## 需求

- [ ] 定義一個供應商中立的外掛程式套件模型，能夠代表 AGY CLI、Copilot CLI、Codex 與 Claude Code 四者之間的共同最小基底。
- [ ] 將 GAL 原始碼合約視為唯一的撰寫介面：`skills/`、`commands/`、`agent/`、`conventions/`、`workflows/`、`mcp.json`、範本及儲存庫狀態均保持在原始碼中管理。
- [ ] 將 `scripts/` 保留為 GAL source/runtime tooling，不納入 provider-neutral plugin payload。
- [ ] 將每個供應商外掛程式目錄視為生成的輸出，永遠不作為手動編輯的原始碼根目錄。
- [ ] 先建立共同基底，再以 AGY 作為第一個供應商渲染器進行實作。
- [ ] 保留循序的遷移路徑：AGY 優先，二 Copilot CLI，三 Codex，最後 Claude Code，每個均透過其專屬渲染器及驗證流程進行。
- [ ] 不強制使用通用的磁碟外掛程式資料夾。清單路徑、MCP 檔案名稱、指令介面、代理程式檔案名稱、快取行為及生命週期指令仍依供應商而有所不同。
- [ ] 以 `skills/<name>/SKILL.md` 作為主要的可攜式承載，因為四個靜態外掛程式供應商均支援此結構。
- [ ] 將 MCP 表示為規範的伺服器清單與 local-only 覆寫邊界，再由各渲染器在 plugin root 內寫入供應商專屬的 MCP 檔案，例如 AGY 的 `mcp_config.json` 或其他供應商的 `.mcp.json`。GAL 管理的 MCP 預設安裝位置是 plugin 內部，而非 plugin 外部的散落設定檔。
- [ ] 共同模型不得包含供應商專屬輸出路徑，也不得包含從 `mcp.local.json`、`config.local.env`、憑證或機器路徑解析出的最終值。
- [ ] 將代理程式表示為可選承載：AGY、Copilot CLI 與 Claude Code 會渲染代理程式，而 Codex 明確略過此項。
- [ ] 將 GAL 指令表示為指令技能（command skills），而非通用的扁平指令格式。Claude `commands/*.md` 是後續的渲染器選項，而非共同模型。
- [ ] 將指令集（instructions）表示為指令語料庫，而非 AGY 的 `rules/`。AGY 可渲染 `rules/gal.md`，非 AGY 渲染器必須選擇供應商原生的指令載體。
- [ ] 將 hooks 從共同基底中推遲。四個供應商中均存在 hooks，但其啟用方式、事件模型與信任邊界在 v1 中差異過大，不適合建立安全的共同合約。
- [ ] 將本地已設定的成品與未來可分享的套件分開。已解析的 `mcp.local.json`、`config.local.env`、憑證及機器路徑僅能進入被忽略的本地輸出，不得進入可分享的共同模型或範本。
- [ ] Windows PowerShell 與 Bash 流程必須共用相同的供應商套件合約與 AGY 渲染器行為。
- [ ] `gal-results/` 屬於 xmachine output，不得用作 provider plugin staging 或 generated plugin artifact root。
- [ ] AGY 目標路徑固定為 `~/.gemini/antigravity-cli/plugins/gal/`；`~/.gemini/config/plugins/` 視為舊 Gemini CLI 方法，不屬於本計畫。
- [ ] AGY setup/reinstall 必須先移除所有既有的 GAL 管理 AGY 舊安裝內容，再以 plugin root 作為唯一的受管安裝面；不得保留 plugin 外的 GAL 受管殘留。
- [ ] Gemini 不是第五個 renderer，而是 AGY migration/compatibility lane；現有 Gemini-specific cleanup 與 bridge 邏輯不得進入 provider-neutral substrate。
- [ ] 文件必須說明共同基底、plugin-root MCP、AGY 優先遷移、Gemini migration lane，以及後續 Copilot CLI、Codex、Claude Code 的渲染器順序，且不得暗示所有供應商均使用 AGY 佈局。

## 方法

### 架構修正

先前以 AGY 為優先的框架是錯誤的，因為它將 AGY 外掛程式佈局過度置於設計核心。修正後的架構如下：

```text
GAL 原始碼合約
  -> 供應商中立外掛程式套件模型
  -> 供應商專屬渲染器
  -> 供應商專屬生成外掛程式成品
  -> 供應商專屬安裝或同步生命週期
```

共同層是資料模型與驗證合約，而非目錄結構。沒有任何供應商會直接使用它。每個供應商渲染器擁有其所寫入的具體檔案。MCP 檔案會進入 plugin 成品，但其落點仍由 renderer 決定，而不是由共同模型決定。

### 文件已鎖定的外部結論

- AGY plugin 是 namespaced bundle；plugin root 有 `plugin.json` required marker，並可承載 `skills/`、`rules/`、`mcp_config.json` 與 `hooks.json`。AGY CLI features 文件也列出 plugin 目錄可含 `agents/` 與 `mcp_config.json`。
- AGY 使用獨立 `mcp_config.json`；remote MCP transport 使用 `serverUrl`，不是 `url`。
- AGY plugin root 的 `mcp_config.json` 有官方文件支撐；全域 `~/.gemini/antigravity-cli/mcp_config.json` 不再作為 GAL 的持續安裝面，只允許作為移除舊 GAL 受管內容時的 legacy cleanup 觸點。
- AGY 最小 `plugin.json` 可只有 `name`，且 `name` 可省略並由目錄名稱推導。本 renderer 仍應輸出穩定 `name: gal` 以便測試與管理。
- Gemini CLI extensions 會遷移為 Antigravity plugins，可透過 `agy plugin import gemini` 轉換；Gemini commands 會轉成 skills。Gemini 因此是 migration lane，不是 GAL 的第五個 provider renderer。
- `scripts/` 不是 AGY、Copilot CLI、Codex 與 Claude Code 的共同 plugin root 承載。只有 skill-local helper scripts 或 provider-specific hooks/bin/scripts 能作為特定供應商能力使用。

### 最小共同基底

| 共同欄位 | GAL 來源 | 是否為四者共用？ | 渲染器職責 |
| --- | --- | --- | --- |
| `metadata` | 儲存庫名稱、顯示名稱、版本診斷、生成時間戳記 | 概念上是 | 在所需路徑以僅包含支援欄位的方式發出供應商清單 |
| `skills` | `skills/<name>/SKILL.md` | 是 | 複製至 `skills/<name>/SKILL.md` |
| `commandSkills` | `commands/*/SKILL.md` | 以技能套件的形式共用 | 渲染為 `skills/<command>/SKILL.md`；不假設為扁平指令檔案 |
| `mcpSpec` | `mcp.json` 加上 `mcp.local.json` 的邊界資訊 | 概念上是，但共同模型內不保留已解析 local 值 | 在 plugin root 內發出 `mcp_config.json` 或 `.mcp.json`，並只在本地 render/install 階段解析 machine-local 值 |
| `instructionCorpus` | `.dev/project.md`、必要 `conventions/`、必要 `workflows/`、`model-roles.md`、生成的索引 | 內容上是，路徑上否 | 渲染至 AGY `rules/gal.md`、指令技能或供應商原生指令介面 |
| `agents` | `agent/*.agent.md` | 否，但可投影至四者中的三者 | 為 AGY、Copilot CLI、Claude Code 渲染；為 Codex 以明確的不支援記錄略過 |
| `hooks` | v1 中無 | 無安全共同合約 | v1 中所有渲染器均略過 |

最小共同性因此為：metadata、skills、指令技能視作 skills、規範的 MCP 規格、指令語料庫、具備能力旗標的可選代理程式，以及不含 hooks、不含 `runtimeScripts`。

### 供應商渲染器矩陣

| 供應商 | 清單輸出 | Skills 輸出 | Agents 輸出 | 指令輸出 | MCP 輸出 | v1 行動 |
| --- | --- | --- | --- | --- | --- | --- |
| AGY CLI | `plugin.json` | `skills/<name>/SKILL.md` | `agents/*.md` | `rules/gal.md` | `mcp_config.json` | 優先實作 |
| Copilot CLI | `plugin.json` | `skills/<name>/SKILL.md` | `agents/*.agent.md` | 指令技能或供應商支援的儲存庫情境 | `.mcp.json` | 未來渲染器 2 |
| Codex | `.codex-plugin/plugin.json` | `skills/<name>/SKILL.md` | 不支援，明確略過 | 指令技能或供應商支援的儲存庫情境 | `.mcp.json` | 未來渲染器 3 |
| Claude Code | `.claude-plugin/plugin.json` | `skills/<name>/SKILL.md` | 含 Claude frontmatter 的 `agents/*.md` | 指令技能，而非外掛程式根目錄的 `CLAUDE.md` | `.mcp.json` 或行內供應商形式 | 未來渲染器 4 |

OpenCode 不屬於靜態四供應商基底。其外掛程式模型是 JavaScript 或 TypeScript 擴展通道，應由後續的 OpenCode 橋接計畫處理，而非複製靜態外掛程式目錄。

### 遷移順序

1. 建立供應商中立的套件清單與驗證。
2. 實作 AGY 渲染器，並將 AGY 安裝/更新/解除安裝遷移至 `~/.gemini/antigravity-cli/plugins/gal/`。
3. 在後續計畫中使用根目錄 `plugin.json`、`skills/`、`agents/*.agent.md` 與 `.mcp.json` 新增 Copilot CLI 渲染器。
4. 在後續計畫中使用 `.codex-plugin/plugin.json`、`skills/` 與 `.mcp.json` 新增 Codex 渲染器，略過代理程式。
5. 在後續計畫中使用 `.claude-plugin/plugin.json`、`skills/`、`agents/`、指令技能與 Claude 專屬 MCP 處理方式新增 Claude Code 渲染器。

每個供應商遷移均必須通過相同的共同基底測試加上渲染器專屬佈局測試。後續渲染器不得修改共同基底，除非供應商在共用模型中暴露了真正的缺口。

### AGY 優先實作

AGY 為第一個渲染器，因為它是目前直接寫入的痛點。AGY 渲染器在 ignored generated artifact root `dist/provider-plugins/agy/gal/` 下寫入生成的本地成品，然後設定程序先移除 plugin root 之外所有既有的 GAL 管理 AGY 舊安裝內容，再將該目錄樹同步至 `~/.gemini/antigravity-cli/plugins/gal/`。`gal-results/` 不得使用，因為它屬於 xmachine output。

AGY 渲染器輸出：

- `plugin.json`
- `skills/`
- `agents/`
- `rules/gal.md`
- `mcp_config.json`

AGY v1 明確不生成 `hooks.json`、`scripts/`、市集 metadata、Copilot/Codex/Claude 目錄或供應商存根。不支援及已推遲的介面使用**略過，而非存根**。

### 規範套件驗證

供應商套件建置在任何渲染器寫入檔案前，必須驗證以下規則：

- skill 名稱與指令技能名稱不衝突
- 每個共同承載項目均有來源路徑及生成目標意圖
- 供應商專屬路徑（如 `.codex-plugin/`、`.claude-plugin/`、`rules/` 或 `mcp_config.json`）不出現在供應商中立套件模型內
- 解析後的機器本地值、機密、憑證與機器路徑不出現在共同模型或可分享的套件成品中
- `runtimeScripts`、plugin-root `scripts/` 或 `gal-results/` 不出現在 provider-neutral package model 或 AGY renderer output
- 不支援的供應商功能被記錄為已略過，而非無聲地丟棄

### AGY 渲染器步驟

#### 步驟 1：新增共同套件清單

- **檔案**：`scripts/common/ProviderPlugin.ps1`、`scripts/common/provider-plugin.sh`、`scripts/common/Common.ps1`、`scripts/common/common.sh`
- **內容**：將 metadata、skills、指令技能、代理程式、指令語料庫及規範 MCP 參考收集至供應商中立的套件物件中。Gemini-specific paths、cleanup 與 bridge 邏輯只標記為 AGY migration concern，不進入共同模型。
- **驗證**：物件不包含任何供應商輸出路徑、`runtimeScripts`、plugin-root `scripts/`、`gal-results/`、已解析的 local secrets 或 machine-local 路徑，且可在不寫入 AGY 檔案的情況下進行檢查。

#### 步驟 2：新增渲染器驗證

- **檔案**：`scripts/common/ProviderPlugin.ps1`、`scripts/common/provider-plugin.sh`、測試輔助程式或腳本驗證輔助程式
- **內容**：驗證共同套件不變式、供應商能力旗標、不支援的元件略過、移除 `runtimeScripts` 及 local-only 成品邊界。
- **驗證**：無效的名稱衝突、供應商路徑洩漏、已解析 local 值洩漏、`runtimeScripts` 洩漏及 `gal-results/` 洩漏在渲染前即失敗。

#### 步驟 3：定義 AGY manifest 與 generated artifact root

- **檔案**：`scripts/common/ProviderPlugin.ps1`、`scripts/common/provider-plugin.sh`、可選的 `templates/provider-plugins/agy/plugin.json`
- **內容**：定義 AGY `plugin.json` 的最小輸出與 generated artifact root。`plugin.json` 至少輸出穩定 `name: gal`；generated artifact root 使用 `dist/provider-plugins/agy/gal/`，不使用 `gal-results/`。
- **驗證**：manifest JSON 可解析，artifact root 不在 xmachine output 底下，且不含 provider-neutral model 不允許的欄位。

#### 步驟 4：實作 AGY skills 與 command skills 渲染器

- **檔案**：`scripts/Build-AgyPlugin.ps1`、`scripts/build-agy-plugin.sh`、`scripts/Update-Commands.ps1`、`scripts/update-commands.sh`、`scripts/Update-Skills.ps1`、`scripts/update-skills.sh`
- **內容**：將共同套件中的 reusable skills 與 command skills 渲染至 AGY plugin `skills/`。停止直接 AGY skill 符號連結，並在 setup/reinstall 時移除既有的 GAL 管理 AGY skill 安裝殘留；此階段 Copilot CLI 與 Codex 的既有指令技能行為保持不變。
- **驗證**：AGY 不再需要直接的 `~/.gemini/antigravity-cli/skills/<name>/` GAL-managed links，且任何既有 GAL 管理 AGY skills 舊安裝都會被清除；非 AGY 的既有安裝不受影響。

#### 步驟 5：實作 AGY agents 渲染器

- **檔案**：`scripts/Build-AgyPlugin.ps1`、`scripts/build-agy-plugin.sh`、`scripts/Update-Skills.ps1`、`scripts/update-skills.sh`
- **內容**：將 GAL `agent/*.agent.md` 投影為 AGY 支援的 `agents/` payload。Codex agent skip 只在共同模型中記錄，不在 AGY output 產生存根。
- **驗證**：AGY plugin artifact 包含 agent payload；不產生 Copilot/Codex/Claude-only 檔案。

#### 步驟 6：將 AGY MCP 路由至 plugin root

- **檔案**：`scripts/Update-Mcp.ps1`、`scripts/update-mcp.sh`、AGY renderer helpers
- **內容**：重用現有 MCP 規格與 AGY 轉換，但將 GAL 管理的 AGY MCP 寫入 plugin root `mcp_config.json`。共同模型只攜帶 canonical MCP 規格與 local-only 邊界；已解析的本地值只在本地 render/install 階段物化。setup/reinstall 必須移除所有既有的 GAL 管理 AGY MCP 舊安裝內容與指向 plugin 外位置的殘留，確保 plugin root 成為唯一的 GAL 受管 MCP 載體。
- **驗證**：plugin root `mcp_config.json` 成為 GAL 的唯一權威來源；任何既有的 GAL 管理 AGY MCP 舊安裝或舊指標都會被移除，且不留下 plugin 外的 GAL 受管 MCP 內容。

#### 步驟 7：渲染 AGY 指令集而不復活 `.agents`

- **檔案**：`scripts/Update-Personalization.ps1`、`scripts/update-personalization.sh`、同步輔助程式
- **內容**：將指令語料庫渲染至 AGY 的 `rules/gal.md`。語料庫來源需明確列出為 `.dev/project.md`、必要 conventions、必要 workflows、`model-roles.md` 與生成索引。僅保留儲存庫本地 `.agents` 清理，不重新引入 `.agents` 作為安裝介面。
- **驗證**：AGY 具有精簡的 `rules/gal.md`；儲存庫不獲得新的 `.agents` 輸出。

#### 步驟 8：更新設定生命週期

- **檔案**：`scripts/Setup-Machine.ps1`、`scripts/setup-machine.sh`、解除安裝/更新輔助程式
- **內容**：讓設定程序建置共同套件、渲染 AGY、先清空所有既有的 GAL 管理 AGY 舊安裝面，再同步 `~/.gemini/antigravity-cli/plugins/gal/` 並更新受管理狀態。清除範圍包含舊版直接寫入資產、外部 AGY runtime 連結、plugin 外的 GAL 管理 skills、MCP 與其他 legacy 指標。解除安裝移除 GAL-managed plugin directory 與其對應的 legacy GAL-managed AGY 殘留，不自動恢復舊 symlink 模式。
- **驗證**：重新安裝會先移除既有 GAL 管理 AGY 舊內容，再確定性地覆寫 `plugins/gal/`；解除安裝會移除受管理的外掛程式輸出與對應 legacy 殘留，不留下 plugin 外的 GAL 受管 AGY 內容。

#### 步驟 9：更新文件

- **檔案**：`docs/personalization.md`、`docs/personalization.zh-Hant.md`、`docs/devguide.md`、`scripts/scripts.md`
- **內容**：說明共同基底、AGY 渲染器、plugin-root MCP、移除共同 `runtimeScripts`、循序供應商遷移順序、Gemini migration lane，以及本地與可分享成品邊界。
- **驗證**：文件不再將 AGY 描述為架構，而是將其描述為共用供應商套件模型上的 renderer 1。

### 參考資料

- [Antigravity Plugins](https://antigravity.google/docs/plugins) - AGY plugin root、`plugin.json` marker、skills、rules、MCP、hooks。
- [Antigravity Skills](https://antigravity.google/docs/skills) - skill-local `scripts/` 是 skill helper resource，不是 provider-neutral plugin root payload。
- [Antigravity Gemini CLI Migration](https://antigravity.google/docs/gcli-migration) - Gemini extensions 遷移為 AGY plugins，Gemini commands 轉為 skills。
- [GitHub Copilot CLI Creating a plugin](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/plugins-creating) - Copilot plugin root、`plugin.json`、agents、skills、hooks、MCP。
- [GitHub Copilot CLI plugin reference](https://docs.github.com/en/copilot/reference/cli-plugin-reference) - install specs、file locations、component path fields、cache/load precedence。
- [OpenAI Codex Build plugins](https://developers.openai.com/codex/plugins/build) - Codex `.codex-plugin/plugin.json`、skills、MCP、apps、hooks、assets、marketplace cache。
- [Claude Code Plugins reference](https://code.claude.com/docs/en/plugins-reference) - Claude `.claude-plugin/plugin.json`、skills、commands、agents、MCP、hooks、`bin/`、provider-specific scripts support。

## 需建立或修改的檔案

### AGY 優先實作所需的原始碼變更

- `scripts/common/ProviderPlugin.ps1` - PowerShell 共同供應商套件清單與驗證輔助程式，包含規範的 MCP 邊界與明確排除 `runtimeScripts`。
- `scripts/common/provider-plugin.sh` - Bash 共同供應商套件清單與驗證輔助程式。
- `scripts/common/Common.ps1` - 共用的 AGY 外掛程式生成成品路徑、安裝路徑與生命週期狀態整合。
- `scripts/common/common.sh` - Bash 對應的共用路徑與生命週期狀態。
- `scripts/Build-AgyPlugin.ps1` - 從共同套件模型生成外掛程式樹至 `dist/provider-plugins/agy/gal/` 的 AGY 渲染器。
- `scripts/build-agy-plugin.sh` - Bash 對應的 AGY 渲染器。
- `scripts/Setup-Machine.ps1` - 透過建置與外掛程式同步的 AGY 設定生命週期。
- `scripts/setup-machine.sh` - Bash 對應的設定生命週期。
- `scripts/Update-Commands.ps1` - AGY 指令技能透過外掛程式渲染器路由，而非 AGY 的輸出保持不變。
- `scripts/update-commands.sh` - Bash 對應的指令路由。
- `scripts/Update-Skills.ps1` - AGY 可重用技能與代理程式承載透過外掛程式渲染器路由。
- `scripts/update-skills.sh` - Bash 對應的技能與承載路由。
- `scripts/Update-Mcp.ps1` - AGY MCP 寫入外掛程式根目錄 `mcp_config.json`，並移除所有 plugin 外的 GAL 管理 AGY MCP 舊安裝內容。
- `scripts/update-mcp.sh` - Bash 對應的 MCP。
- `scripts/Update-Personalization.ps1` - AGY `rules/gal.md` 投影與 `.agents` 清理邊界。
- `scripts/update-personalization.sh` - Bash 對應的個人化。
- `docs/personalization.md` - 面向使用者的供應商外掛程式封裝與 AGY 優先安裝指南。
- `docs/personalization.zh-Hant.md` - 中文版指南。
- `docs/devguide.md` - 維護者架構指南，說明共同基底與供應商渲染器。
- `scripts/scripts.md` - 腳本職責與生命週期文件。

### 生成的輸出

- `dist/provider-plugins/agy/gal/` - 被忽略的本地 AGY 外掛程式成品。
- `dist/provider-plugins/agy/gal/plugin.json` - 生成的 AGY 清單。
- `dist/provider-plugins/agy/gal/skills/` - 生成的技能與指令技能。
- `dist/provider-plugins/agy/gal/agents/` - 生成的 AGY 代理程式承載。
- `dist/provider-plugins/agy/gal/rules/gal.md` - 生成的 AGY 指令投影。
- `dist/provider-plugins/agy/gal/mcp_config.json` - 生成的 AGY 外掛程式根目錄 MCP 設定檔。

### 明確排除的輸出

- `gal-results/agy-plugin/gal/` - 排除，因為 `gal-results/` 屬於 xmachine 輸出。
- 任何外掛程式根目錄 `scripts/` 承載 - 排除於 v1 共同模型與 AGY 渲染器之外。
- `~/.gemini/config/plugins/` - 排除，視為舊版 Gemini CLI 方法。

### 推遲的供應商渲染器變更

- Copilot CLI 渲染器檔案推遲至 AGY 驗證後的後續計畫。
- Codex 渲染器檔案推遲至 Copilot CLI 驗證後的後續計畫。
- Claude Code 渲染器檔案推遲至 Codex 驗證後的後續計畫。
- OpenCode 橋接檔案推遲至獨立的外掛程式程式碼整合計畫。

## 測試案例

- [ ] 共同套件清單包含 metadata、skills、指令技能、規範 MCP 規格、指令語料庫及可選代理程式，不包含供應商輸出路徑、已解析本地值、`runtimeScripts`、外掛程式根目錄 `scripts/` 或 `gal-results/` 路徑。
- [ ] 共同驗證在技能與指令技能名稱衝突時失敗。
- [ ] 共同驗證在 AGY 專屬路徑、Copilot 專屬路徑、Codex 專屬路徑或 Claude 專屬路徑出現在供應商中立套件模型中時失敗。
- [ ] 共同驗證在已解析的 local-only MCP 或 env 值進入供應商中立模型或可分享外掛程式套件表面時失敗。
- [ ] 共同驗證在 `runtimeScripts`、外掛程式根目錄 `scripts/` 或 `gal-results/` 出現在供應商中立套件模型或 AGY 輸出計畫中時失敗。
- [ ] AGY 渲染器建立 `dist/provider-plugins/agy/gal/plugin.json`、`skills/`、`agents/`、`rules/gal.md` 與 `mcp_config.json`。
- [ ] AGY 渲染器不建立 `hooks.json`、`scripts/`、`.codex-plugin/`、`.claude-plugin/`、Copilot 專屬檔案或空的供應商存根。
- [ ] AGY 外掛程式承載不包含符號連結或對 `~/.gemini/antigravity-cli/gal/` 的必要參考，且 setup 後 plugin 外不存在 GAL 管理 AGY 舊安裝內容。
- [ ] 設定完成後，可從外掛程式 `skills/` 中發現 AGY 指令技能與可重用技能。
- [ ] Copilot CLI 與 Codex 現有的指令技能安裝行為在 AGY 遷移期間保持不變。
- [ ] AGY 外掛程式根目錄 `mcp_config.json` 是由規範 MCP 規格加上本地安裝時解析所生成。
- [ ] PowerShell 上的設定會先移除既有的 GAL 管理 AGY 舊安裝內容，再安裝或更新 `~/.gemini/antigravity-cli/plugins/gal/`，且不會散佈新的 AGY 直接寫入資產。
- [ ] Bash 上的設定產生相同的生成 AGY 外掛程式佈局與舊安裝移除行為。
- [ ] 文件描述共同基底、AGY 優先渲染器、外掛程式根目錄 MCP、移除 `runtimeScripts`、Gemini 遷移路徑，以及未來的 Copilot CLI、Codex、Claude Code 渲染器順序。

## 成功標準

- [ ] GAL 擁有供應商中立的外掛程式套件模型，代表 AGY CLI、Copilot CLI、Codex 與 Claude Code 之間的最少共同承載。
- [ ] AGY 被實作為共同模型上的第一個渲染器，而非核心架構。
- [ ] 共同模型沒有供應商專屬輸出路徑、沒有已解析的機器本地機密或路徑，且沒有 `runtimeScripts`。
- [ ] AGY 外掛程式的安裝、更新與解除安裝透過 `~/.gemini/antigravity-cli/plugins/gal/` 進行管理。
- [ ] AGY 外掛程式承載使用外掛程式根目錄 MCP，不需要 `gal-results/`、外掛程式根目錄 `scripts/` 或外部 `~/.gemini/antigravity-cli/gal/` 執行階段連結，且 plugin 外不殘留任何 GAL 管理 AGY 安裝內容。
- [ ] GAL 原始碼合約保持由儲存庫擁有，永遠不會被暫存的外掛程式輸出取代。
- [ ] MCP 擁有權明確：規範 MCP 由原始碼擁有，外掛程式根目錄 `mcp_config.json` 是渲染器輸出，而所有既有的 GAL 管理 AGY MCP 舊安裝內容都被移除。
- [ ] Gemini 被文件記錄為 AGY 遷移/相容性路徑，而非第五個渲染器。
- [ ] 未來的 Copilot CLI、Codex 與 Claude Code 渲染器可以重用共同基底，而無需複製 AGY 佈局。
- [ ] 在擁有供應商專屬的安全性與生命週期審查之前，Hooks 保持在 v1 之外。
- [ ] 文件解釋了共用基底與循序供應商遷移順序。

## 風險

- 如果共同模型包含供應商路徑，後續渲染器將繼承 AGY 耦合並重蹈此設計錯誤。
- 如果共同模型過於抽象，實作者可能會建立一個無法快速發布 AGY 的框架。將基底限制為 metadata、skills、指令技能、MCP 規格、指令與可選代理程式。
- 如果指令技能需要無法以技能指令表達的可執行行為，此計畫必須選擇供應商專屬輔助機制，而非重新引入共同的 `runtimeScripts`。
- 如果指令技能被視為扁平指令，非 Claude 供應商將獲得錯誤的形狀。在共同模型中將指令視為技能套件。
- 如果指令被視為 AGY `rules/`，非 AGY 供應商將獲得錯誤的載體。將指令視為內容，而非路徑。
- 如果 hooks 進入 v1，遷移將成為生命週期自動化與安全性專案，而非封裝。推遲 hooks。
- 如果將本地設定完成的成品與可分享套件混淆，MCP 機密或機器路徑可能會外洩至未來的市集或團隊成品中。
- 如果 legacy AGY 清理邊界辨識錯誤，setup 可能刪除非 GAL 管理的使用者內容，或遺留 plugin 外的舊資產。清理規則必須精準辨識 GAL-managed 舊內容並完整移除。
- 如果後續供應商渲染器工作在 AGY 驗證共同基底之前就開始，供應商專屬的修復可能會過早改變模型。

## 待確認問題

- [x] OQ-001 - AGY CLI、Copilot CLI、Codex 與 Claude Code 之間最小的共同承載為何？ *(提出者：使用者架構修正，解決方式：計畫重寫)* Metadata、skills、視為技能的指令技能、規範 MCP 規格、指令語料庫，以及具備能力旗標的可選代理程式。Hooks 與 `runtimeScripts` 排除在 v1 之外。
- [x] OQ-002 - AGY 是架構還是僅是第一個渲染器？ *(提出者：使用者架構修正，解決方式：計畫重寫)* AGY 僅是渲染器 1。架構為 `GAL 原始碼合約 -> 供應商中立套件模型 -> 供應商渲染器 -> 生成的供應商成品`。
- [x] OQ-003 - Copilot CLI、Codex 與 Claude Code 是否應在同一次 AGY 遷移中實作？ *(提出者：使用者架構修正，解決方式：計畫重寫)* 否。它們的渲染器是在 AGY 驗證共同基底後的循序後續遷移，順序為 Copilot CLI -> Codex -> Claude Code。
- [x] OQ-004 - 不支援的供應商元件應如何處理？ *(提出者：使用者架構修正，解決方式：計畫重寫)* 不支援的元件透過能力 metadata 明確略過。不要生成空的存根。
- [x] OQ-005 - `gal-results/` 是否應作為 AGY 外掛程式暫存區？ *(提出者：架構審查，解決方式：使用者決定)* 否。`gal-results/` 屬於 xmachine，不在本計畫範圍內。
- [x] OQ-006 - Gemini 是第五個渲染器嗎？ *(提出者：架構審查，解決方式：文件與使用者決定)* 否。Gemini 是 AGY 遷移/相容性路徑。

## 簽核

- 人工核准：[待處理]
- 架構審查：[通過]
- 額外領域審查：[未觸發]

## 審查結果

### 架構審查

重新審查：2026-05-20 架構修正後。

結論：**核准** (APPROVE)

重寫後的計畫現在符合預期的架構。核心抽象是供應商中立的套件模型，而非 AGY 佈局。AGY 仍是第一個遷移目標，而 Copilot CLI、Codex 與 Claude Code 則是未來在相同共同基底上的渲染器，並嚴格按照該順序進行。`runtimeScripts` 與 `gal-results/` 已從共同模型與 AGY 渲染器計畫中移除。

#### 權衡總結

| 決策 | 效益 | 成本 | 結論 |
| --- | --- | --- | --- |
| 在 AGY 渲染器前建置共同基底 | 防止 AGY 佈局成為架構 | 增加小型的清單與驗證層 | OK |
| 保持供應商輸出路徑由渲染器擁有 | 允許所有四個外掛程式供應商使用正確的佈局 | 需要每個供應商渲染器的測試 | OK |
| 將指令視為技能套件 | 適用於所有四個靜態外掛程式供應商 | 在 v1 中不使用 Claude 扁平指令 | OK |
| 從共同模型中移除 `runtimeScripts` | 避免發明虛假的四供應商承載 | 供應商專屬可執行輔助程式需要後續設計 | OK |
| 將 GAL 管理的 MCP 與既有 AGY 受管內容完全收斂到外掛程式根目錄下 | 建立乾淨的 plugin-only 安裝面並移除外部 legacy 受管資產 | 需要精準的 legacy 清理與本地渲染/安裝解析規則 | OK |
| 將指令視為內容，而非 `rules/` | 避免 AGY 專屬耦合 | 每個渲染器必須選擇指令載體 | OK |
| 推遲 hooks | 避免過早的生命週期與安全性範圍 | v1 中沒有基於 hook 的自我修復 | OK |
| 僅先實作 AGY 渲染器 | 解決目前痛點並同時證明共同模型 | Copilot/Codex/Claude 需等待後續計畫 | OK |

#### 架構發現

- 先前的計畫在結構上是錯誤的，因為它以 AGY 為中心，並將其他供應商作為 AGY 的未來改編。
- 修正後的設計將共同套件模型作為中心，並使每個供應商（包含 AGY）成為渲染器。
- 最小共同基底刻意保持極小。Skills 仍是最強大的共用原語。MCP 作為規範規格加上本地邊界 metadata 進行分享，而非供應商檔案形狀，也不是已解析的本地值。
- `runtimeScripts` 被移除，因為外掛程式根目錄 `scripts/` 並非真正的共用供應商原語。
- `gal-results/` 被移除，因為它屬於 xmachine 輸出。
- 循序遷移是正確的控制面。AGY 首先證明基底；Copilot CLI、Codex 與 Claude Code 依序跟進，而不強制將所有供應商專屬語意塞入第一個實作中。

#### 剩餘限制

- 除非後續計畫明確擴大範圍，否則不要在 AGY 優先的實作中加入 Copilot CLI、Codex 或 Claude Code 渲染器。
- 不要重新引入 AGY 指令執行對外部 `GAL_ROOT` 執行階段連結的相依性。
- 不要重新引入供應商中立的 `runtimeScripts` 或外掛程式根目錄 `scripts/` 作為 v1 共同承載。
- 不要加入 OpenCode 靜態渲染器。OpenCode 需要獨立的程式碼外掛程式橋接計畫。
- 不要將 `gal-results/` 用於供應商外掛程式成品。

### 商業審查

未觸發。本計畫改變了封裝與安裝拓撲，而非定價、權限、入職、資格或顧客可見的商業規則。

### 設計審查

未觸發。本計畫沒有顧客面對的 UI、佈局、視覺狀態或無障礙介面。

### 工程審查

結論：**通過** (CLEAR)

實作合約清晰且已界定範圍。任務涵蓋了共同基底、驗證、AGY 渲染器、AGY 遷移、設定生命週期與文件。本計畫刻意推遲了非 AGY 渲染器，同時保留了它們的架構路徑與執行順序。

<!-- ENG_REVIEW: CLEAR -->

## 測試計畫

| ID | 類型 | 描述 | 涵蓋 |
| --- | --- | --- | --- |
| TP-001 | 腳本冒煙測試 | 共同供應商套件清單僅包含供應商中立欄位，並拒絕供應商輸出路徑、`runtimeScripts`、`gal-results/` 與已解析的本地值。 | T-001, T-002 |
| TP-002 | 腳本冒煙測試 | 共同驗證在技能與指令技能名稱衝突、不支援的元件存根以及供應商路徑外洩時失敗。 | T-002 |
| TP-003 | 腳本冒煙測試 | `Build-AgyPlugin.ps1` 從共同套件模型在 `dist/provider-plugins/agy/gal/` 下渲染 AGY 外掛程式輸出。 | T-003, T-004, T-005, T-006, T-007 |
| TP-004 | 腳本冒煙測試 | `build-agy-plugin.sh` 渲染與 PowerShell 路徑相同的 AGY 外掛程式結構。 | T-003, T-004, T-005, T-006, T-007 |
| TP-005 | 迴歸測試 | AGY 指令技能、可重用技能與代理程式透過外掛程式承載路由，且 setup 會移除既有的 GAL 管理 AGY 連結與舊安裝內容。 | T-004, T-005, T-008 |
| TP-006 | 迴歸測試 | Copilot CLI 與 Codex 現有的指令技能安裝行為在 AGY 遷移期間保持不變。 | T-004 |
| TP-007 | 整合測試 | AGY 外掛程式根目錄 `mcp_config.json` 由規範 MCP 規格生成，且所有 plugin 外的 GAL 管理 AGY MCP 舊安裝內容都被精確移除。 | T-006, T-008 |
| TP-008 | 整合測試 | AGY `rules/gal.md` 由指令語料庫生成，且不重新建立儲存庫本地的 `.agents`。 | T-007 |
| TP-009 | 手動冒煙測試 | PowerShell 與 Bash 設定會先移除既有的 GAL 管理 AGY 舊安裝內容，再安裝或更新 `~/.gemini/antigravity-cli/plugins/gal/`，將 MCP 保留在外掛程式根目錄下，且不散佈新的 AGY 直接寫入資產。 | T-008 |
| TP-010 | 文件審查 | 文件描述共同基底、AGY 渲染器 1、外掛程式根目錄 MCP、移除 `runtimeScripts`、Gemini 遷移路徑，以及推遲的 Copilot CLI、Codex、Claude Code 渲染器順序。 | T-009 |

## 任務

- [x] T-001 — 新增針對 metadata、skills、指令技能、規範 MCP 規格、指令語料庫及可選代理程式的供應商中立套件清單輔助程式。 *(6a68390)*
- [x] T-002 — 新增針對名稱衝突、供應商路徑外洩、不支援元件略過、僅限本地成品邊界、`runtimeScripts` 排除及 `gal-results/` 排除的共同套件驗證。 *(1dcf191)*
- [x] T-003 — 定義 AGY 生成成品根目錄與 `plugin.json` 清單輸出，且不使用 `gal-results/`。 *(227ed7d)*
- [x] T-004 — 在 PowerShell 與 Bash 中實作 AGY 可重用技能與指令技能渲染器。 *(Update-Skills 呼叫 Build-AgyPlugin -Force -Install 取代 AGY skill symlinks；Update-Commands 僅清理 legacy AGY command-skill symlinks；兩者均含 install/uninstall legacy cleanup)*
- [x] T-005 — 在 PowerShell 與 Bash 中實作 AGY 代理程式渲染器。 *(227ed7d)*
- [x] T-006 — 透過外掛程式根目錄 `mcp_config.json` 路由 AGY MCP，並移除所有 plugin 外的 GAL 管理 AGY MCP 舊安裝內容。 *(Build-AgyPlugin.ps1/sh: MCP rendering now converts servers→mcpServers, url→serverUrl, removes type, merges local overrides; Update-Mcp.ps1/sh: Update-AgyMcpConfig writes to plugin root mcp_config.json, cleans up global AGY MCP entries on install, removes plugin root MCP on uninstall)*
- [x] T-007 — 從指令語料庫渲染 AGY `rules/gal.md`，不重新引入儲存庫本地的 `.agents`。 *(c6a58ca: Update-Personalization.ps1/sh now calls Build-AgyPlugin -Force -Install to render rules/gal.md from instruction corpus; uninstall removes plugin directory; repo-local .agents cleanup preserved)*
- [x] T-008 — 圍繞 `~/.gemini/antigravity-cli/plugins/gal/` 更新設定、重新安裝、解除安裝與舊版清理生命週期，先移除所有既有的 GAL 管理 AGY 舊安裝內容，再建立乾淨的 plugin-only 安裝面。 *(2504d4a: Setup-Machine.ps1/sh now runs AGY legacy pre-cleanup before concern scripts — removes legacy skills directory, GAL_ROOT symlink, existing plugin install, and GAL-managed MCP entries from global mcp_config.json)*
- [ ] T-009 — 更新文件以描述共同基底、AGY 優先實作、外掛程式根目錄 MCP、移除 `runtimeScripts`、Gemini 遷移路徑、未來的供應商渲染器順序以及成品信任邊界。
