---
source: docs/configuration.md
lang: zh-Hant
source_commit: c894cbdc23f9c38454b267a5798d4726f5590cb1
translated_at: 2026-10-09
type: Guide
title: 設定
description: 管理 `~/.gal/config/config.json` 的機器本機設定，涵蓋執行器路由、工作時段、計畫語言、MCP 來源、個人層與本機筆記路由。
tags:
  - configuration
  - routing
  - mcp
  - personal-layer
status: stable
---

# 設定 (Configuration)

## 本機設定檔 (config.json)

GAL 的本機設定集中在 `~/.gal/config/config.json`。這些設定值僅屬於當前機器，請勿將本機設定值寫入任何受追蹤的文件、指令範本或原始碼檔案。已知設定鍵包含 `galSkills`、`workingHours`、`planLanguage`、`memoryHarvest` 與 `executorRouting`。GAL 會從各執行環境自身的設定偵測外掛註冊狀態。既有的 `pluginMode` 屬性已退役且會忽略，`gal doctor` 會將它列為過時鍵。`schemaVersion` 屬於 `plugins.lock.json`，不屬於 `config.json`。`enabledPlugins` 是 GitHub Copilot 自身 `settings.json` 的設定，GAL 會將它作為註冊依據讀取。

已棄用的 `devMode`、`galRoot` 與 `secrets` 鍵會被直接忽略。`gal doctor` 偵測到 `devMode` 或 `galRoot` 時會發出不阻擋執行的警示。`secrets` 鍵已正式退役，原本供已移除的 MCP host 寫入器取用，gal 不會自此檔案讀取任何憑證資料。

### 主要欄位

| 欄位 | 必要 | 型別 | 說明 |
| --- | --- | --- | --- |
| `executorRouting` | 選用 | 物件 | 依取用端分組的角色路由，為系統唯一的路由來源。詳見下節說明。 |
| `galSkills` | 選用 | 字串 | 指向本機技能目錄的路徑，為唯一保留的本機路徑替代鍵。 |
| `workingHours` | 選用 | 物件 | 工作時段強制設定，包含 `enabled`、`workdayStart`、`workdayEnd`、`wrapUpTime` 與 `hardStopTime`。時間字串一律使用 `HH:MM` 格式，`enabled` 為布林值。 |
| `planLanguage` | 選用 | 字串 | `.dev/plans/*.md` 與 `.dev/research/*.md` 在未給定明確指示時的預設輸出語言。詳見下文。 |
| `memoryHarvest` | 選用 | 物件 | 內容格式為 `{ enabled: boolean }`，預設為關閉的供應商記憶收割設定。詳見下文。 |

外部筆記後端不屬於 Core 設定範疇。其可攜合約定義於 `plugins/gal-core/conventions/optional-capabilities.md`。儲存庫自有的研究成果預設置於 `.dev/research/`，GAL 的暫存資料則使用儲存庫相對路徑 `.dev/tmp`。

## 執行器路由 (executorRouting)

Codex 桌面版的核准模式與 `config.toml` 設定方式，請參閱[安裝與初始化](./setup.zh-Hant.md#codex-管道執行設定)。GAL 不會修改 Codex 的主機端設定。`executorRouting` 與 `timeoutSecs` 屬於 GAL 的設定項目，必須與 Codex 的核准及沙盒設定分開確認。

`executorRouting` 依取用端將角色劃分至三組物件中：負責派送的 `pipeline` 群組包含 `CODER`、`TESTER`、`AUDITOR`，負責審查的 `planning` 群組包含 `ARCHITECT`、`ANALYST`、`DESIGNER`、`RELEASER`，`research` 則為選用群組，負責在三位平行研究工作者中路由至多兩位。三組共用同一套 `executors` 預設區塊。此區塊為系統唯一的路由來源，早期扁平結構已退役不再解析，且會觸發命名警示，原先獨立的路由檔亦不再被讀取。

`planning` 群組僅供應 Codex 原生代理程式投影，`pipeline` 與 `research` 群組則使用無頭派送。詳見[規劃群組範圍](#規劃群組範圍)。

### 規劃群組範圍

`planning` 群組只適用於四個審查角色 `architect`、`analyst`、`designer` 與 `releaser`。在 Codex 代理程式投影期間，GAL 會查出每個角色解析後的 `model` 與 `effort`，並寫入投影出的 Codex 代理程式定義。這項查詢不會啟動執行器，也不會把工作轉移到其他執行器。

- 直接呼叫 `/gal <role>`（隔離模式）會輸出 `ROUTING_SCOPE: codex-native-projection-only` 與一則 `ROUTING_NOTE`。該註記說明三件事：planning 設定只供應 Codex 原生代理程式投影、不會啟動任何執行器、實際使用的模型由主機決定。這段文字是靜態的契約陳述，不能證明任何設定已經套用。
- `planning` 角色中的 `executor` 鍵仍會被讀取。它指定查詢 `model` 與 `effort` 時所用的 `combinations` 項目或 `executors` 項目。優先順序為角色內嵌值、組合、`executors` 預設值。沒有 `planning` 項目的角色兩個值都不會取得，因此 Codex 沿用自己的模型。這個鍵不會選擇要啟動的程序。
- `timeoutSecs`、`sshTarget` 與 `remoteWorkdir` 對 planning 角色不生效，因為這些角色不會執行無頭程序。請只對 `pipeline` 與 `research` 角色設定這些欄位。
- GAL 不保證 Codex 以外的執行環境會套用解析出的 `model` 或 `effort`。無論有無 planning 設定，這些環境都維持各自的原生子代理程式行為。
- 透過 `/<role> discuss` 進行的對話內諮詢在目前工作階段中執行，不會啟動執行器。

### 完成審查路由

計畫層級的完成審查透過 `gal pipeline <execution-prompt> --phase finalize-review` 執行。此階段使用既有的 `pipeline.AUDITOR` 路由，不新增角色或設定鍵。

- 若設定檔、`executorRouting` 鍵、`pipeline` 群組或 `AUDITOR` 項目確實不存在，派送會回報 `no-routing`。只有這種情況允許改為在程序內審查，並標示 `Review Independence: DEGRADED_SAME_RUNTIME`。
- 若設定檔無法讀取或不是有效的 JSON，派送會回報 `routing-invalid` 並停止。`executorRouting` 或 `pipeline` 不是物件時，或 `AUDITOR` 項目格式錯誤、解析不出模型、只設定了 `sshTarget` 與 `remoteWorkdir` 其中之一時，結果相同。這些情況下 GAL 不會退回同一執行環境。此階段中，GAL 不檢查其他角色的項目。
- 執行器不存在、啟動失敗、逾時、結束碼不為零、收據缺漏或為空、缺少提供者的工作階段證據，同樣會使審查停止。這些情況都不會退回同一執行環境。
- 審查成功需要一份新的非空收據、以 `completed` 結束的嘗試日誌，以及提供者的工作階段證據。不需要產品差異。

其他階段的行為不變。對它們而言，無法讀取或格式錯誤的設定檔仍會降級為 `no-routing`。

### combinations 登錄清單

`executorRouting.combinations` 是具名且可重複使用的執行器組合登錄清單。每一筆組合包含 `executor` 以及選用的 `model`、`effort`、`timeoutSecs`、`sshTarget` 與 `remoteWorkdir`。對 `planning` 角色，只會讀取 `model` 與 `effort`。角色可以直接指定組合名稱作為自身的 `executor` 值，隨後角色的各欄位設定會逐一覆寫組合的預設值。`sshTarget` 與 `remoteWorkdir` 必須成對提供。

### executors 預設區塊

`executorRouting.executors` 的每個項目既可以是既有的純字串簡寫，也可以是一個物件：

| 欄位 | 必要 | 型別 | 說明 |
| --- | --- | --- | --- |
| `model` | 必要 | 字串 | 執行器的預設模型。 |
| `effort` | 選用 | 字串 | 執行器的預設推理深度 (effort)。 |
| `timeoutSecs` | 選用 | 正整數 | 執行器的預設逾時時間。`0` 在取用時會被直接拒絕。 |

針對已解析的角色，`effort` 與 `timeoutSecs` 的退回順序固定為：**角色行內設定 → 組合基底 → `executors[final.executor]` → `None`**，模型的查找鏈結則以空字串為終點。角色層級若明確設定 `timeoutSecs: 0`，會解析為 `None`，且不會向下退回至組合或執行器的預設值。

### 研究路由

`research` 納入派送表的方式與 `pipeline` 相同，兩者皆路由至無頭派送。這與 `planning` 不同，`planning` 保持僅驗證原則、絕不執行無頭派送。研究獨立性建立在三工作者模型上：`RESEARCHER#0`、`RESEARCHER#1` 與 `RESEARCHER#2` 以三位平行且彼此盲測（隔離各階段輸出）的工作者執行，三者皆回傳結果後由協調器裁定，而非由任一工作者代為裁定。此設計並未引入驗證派送階段，`executorRouting` 亦未新增 `verify` 鍵，`research` 僅負責路由工作者的挑選。

### 遠端執行

遠端執行在 `executorRouting` 內以角色為單位直接綁定，不需要額外的獨立檔案。`pipeline` 與 `research` 群組的角色可設定 `sshTarget` 與 `remoteWorkdir`，兩者必須成對提供。`planning` 群組的角色不會啟動無頭程序，因此這兩個欄位對它們不適用。這兩項設定值定義了本機的 SSH 目標與儲存庫路徑，屬本機敏感資訊，請勿對外分享。

機密邊界：`config.json` 為本機檔案，切勿外流。gal 不會自中讀取任何機密憑證資料，已退役的 `secrets` 鍵原供已移除的 MCP host 寫入器使用，目前無任何讀取機制。

### Codex 核准與沙盒設定

Codex 的核准政策與沙盒設定位於 Codex 的 `config.toml`內，或可經由 Codex Desktop 的任務模式選取。指令例外表則於 `~/.codex/rules/*.rules`另外設定。這些設定與 GAL 的 `executorRouting` 及 `timeoutSecs` 各自獨立，GAL 不會修改 Codex 的主機設定。符合 `allow` 規則的受信任外層如 RTK 或 GAL 指令可直接在父層沙盒外執行，故不再顯示核准提示。該規則不會改變派送後執行的子行程沙盒。經實測的設定、規則範例、驗證、復原與還原步驟請參閱 [Codex 管道執行設定](setup.zh-Hant.md#codex-管道執行設定)。

GAL 以無頭模式派送 Codex 時，會以 `--dangerously-bypass-approvals-and-sandbox` 啟動 Codex 子行程。子行程在 Codex 沙盒外執行，信任等級與其他執行器相同。原因是 Codex 的 Windows 沙盒目前會在執行任何指令之前就失敗，請參閱 [setup.zh-Hant.md](./setup.zh-Hant.md#已知問題-codex-windows-沙盒失效)。父層 Codex 工作階段的核准模式不會改變子行程的啟動方式。遭拒絕的操作不得視為已授權。無法觀察的權限狀態則一律歸類為 Unknown。

## 本機路徑占位符

`gal clean` 與 `gal smudge` 這兩套 Git 篩選器（詳見 [workflows.zh-Hant.md](./workflows.zh-Hant.md)）使用下列占位符，將本機路徑值自受追蹤的檔案中抽離：

| 占位符 | 意義 | 常見用途 |
| --- | --- | --- |
| `<GAL_SKILLS>` | 本機 GAL 技能目錄的絕對路徑 | Git 篩選器與本機技能投影 |
| `<GAL_SKILLS_BIN>` | `<GAL_SKILLS>` 底下的 `bin` 子目錄 | 與 `<GAL_SKILLS>` 同時比對時，較長的路徑優先比對取代 |

`galSkills` 為目前唯一支援本機路徑替代的設定值，工作時段等其他設定鍵不參與占位符替換。

## MCP server

GAL 的 MCP server 有兩個來源。`gal refresh` 會將兩者合併為單一清單，寫入標準根目錄 (canonical root) `~/.gal/plugins/gal/`。

| 來源 | 位置 | 說明 |
| --- | --- | --- |
| 核心 | 受追蹤的 `plugins/gal-core/mcp.json` | GAL 自身需要的伺服器。目前依政策保持空白的 `servers` 對應表，僅在 GAL 自身需要特定伺服器時新增。 |
| 個人層 | 本機的 `~/.gal/local/mcp.json` | 使用者自行加入的伺服器，僅存在於當前機器。檔案存在即代表啟用，不需要在 `config.json` 設定旗標。 |

兩個來源皆採用 `"servers"` 對應表格式。

### 合併與輸出

執行 `gal refresh` 時，GAL 依下列順序處理：

1. 將核心 `mcp.json` 的 `servers` 對應表改以 `mcpServers` 鍵寫入 `~/.gal/plugins/gal/.mcp.json`。
2. 若 `~/.gal/local/mcp.json` 存在，將其中的伺服器併入同一份 `mcpServers`。若個人項目與核心伺服器同名，個人項目會被略過，一律以核心為準。若個人檔案不存在或 JSON 格式錯誤，GAL 會保留步驟 1 的結果繼續執行，不中斷流程。
3. 將合併後的伺服器清單，以 Agent Plugins 標準格式寫入 `~/.gal/plugins/gal/mcp.json`。

伺服器設定內容會原封不動地傳遞，GAL 不進行任何占位符替換。伺服器設定中若含有 `${ENV_VAR}`，會由 MCP host 於載入檔案時解析，通常取自執行行程的環境變數。建議將這類值設定於編碼代理程式執行的環境中，切勿寫入 `config.json`。

GAL 不支援覆寫檔。如需調整伺服器，請直接編輯上述兩個來源之一，再重新執行 `gal refresh`。Claude Code 需重新啟動或重新加入外掛，方能讀取更新後的設定。

個人層只在本機的 `gal refresh` 中合併。建置公開的外掛市集快照時不會包含個人層，所以個人伺服器不會離開這台機器。

### 哪些執行環境會讀到

GAL 不會把 MCP server 投影 (projection) 到各執行環境的目錄，也不會修改任何執行環境自己的 MCP 設定檔。執行環境只有在以外掛方式載入 GAL、並讀取外掛內的 MCP 檔案時，才會用到這些伺服器。

| 執行環境 | 讀取的檔案 | 狀態 |
| --- | --- | --- |
| Claude Code | `.mcp.json` | GAL 依設計提供 |
| Codex | `.mcp.json` | GAL 依設計提供 |
| GitHub Copilot | `mcp.json` | GAL 依設計提供 |
| Google Antigravity | 無 | GAL 未定義也未驗證 Antigravity 是否讀取外掛內的 MCP 檔案 |
| opencode | 無 | 不會讀到。opencode 不以外掛方式載入 GAL，而投影不包含 MCP server |

「依設計提供」表示 GAL 會產出該執行環境預定讀取的檔案。實際是否載入由各執行環境自己決定，而且前提是使用者已依 [setup.zh-Hant.md](./setup.zh-Hant.md) 的步驟註冊 GAL 外掛。

GAL 不會修改下列執行環境自己的 MCP 設定檔：`claude_desktop_config.json`、`~/.copilot/mcp-config.json`、`~/.codex/config.toml`、`opencode.json` 與 `mcp_config.json`。這些檔案屬於使用者。若要讓某個執行環境使用 GAL 外掛以外的伺服器，請直接編輯該執行環境自己的設定檔。

### 範例

以下是使用者自行加入第三方 MCP server 的範例，不是 GAL 的內建功能。同一種伺服器可以用不同名稱加入多份，各自讀取不同的環境變數。`postgres-mcp` 是第三方 MCP server，由使用者透過 `uvx` 自行取得。

```json
{
  "servers": {
    "postgres-app": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_APP_URI}" }
    },
    "postgres-analytics": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_ANALYTICS_URI}" }
    }
  }
}
```

把這份內容存成 `~/.gal/local/mcp.json`，在環境中匯出 `POSTGRES_MCP_APP_URI` 與 `POSTGRES_MCP_ANALYTICS_URI`，再執行 `gal refresh`，兩個伺服器就會出現在標準根目錄的 MCP 檔案中。

## 工作時段 (workingHours)

工作時段的強制機制預設為停用。在 `~/.gal/config/config.json` 的 `workingHours` 鍵底下設定數值，即可建立工作日邊界。將 `enabled` 設為 `false` 可維持停用狀態。`workdayStart` 與 `workdayEnd` 定義可操作的工作時段，`wrapUpTime` 會啟動收尾提醒與結束流程，`hardStopTime` 則會強制拒絕繼續執行工作。這些設定屬於本機偏好，並非受追蹤的儲存庫政策。

## 計畫語言 (planLanguage)

`config.json` 中的 `planLanguage` 為選用的本機設定，用以決定未給定明確指示時，`.dev/plans/*.md` 與 `.dev/research/*.md` 的預設輸出語言。解析優先順序為：明確指示 → `planLanguage` → 提示語言自動偵測 → 最後退回使用 `en`。受追蹤的 `PROJECT_LANGUAGE` 專案中繼資料負責規範主要文件的標準語言，與 `planLanguage` 屬於不同層級。`.dev/plans/*.prompt.md` 執行提示檔維持嚴格英文，以確保跨模型的穩定性。

若將 `planLanguage` 設定為非英文，系統會產生三層計畫架構：第一層為置於 `.dev/plans/<slug>.en.md` 的英文語意草稿，作為技術定義的權威依據。第二層為置於 `.dev/plans/<slug>.md` 的在地化來源計畫，供開發者閱讀與編輯，其中的標題、路徑、任務 ID 與判定結果維持英文，敘述文字則改用在地化語言。第三層為英文執行提示檔。系統完全支援在地化計畫的手動編輯。後續的規劃階段指令會偵測編輯內容，停在唯讀的協調步驟，將手動修改的意圖合併回英文草稿後再接續進行。手動編輯的內容絕不會遭到覆寫。執行 `/plan-to-prompt` 後，草稿檔案會自動刪除，每個計畫僅會保留兩份受追蹤的檔案。若 `planLanguage` 設定為英文，則會直接略過這整套雙語機制。

## 供應商記憶收割 (memoryHarvest)

記憶收割是選擇加入 (opt-in) 的機制，將編碼代理程式在當前對話中歸納出的實務經驗轉移至 GAL 的受追蹤檔案中，避免工作階段結束時遺失寶貴脈絡。

- **預設關閉、本機限定：** 將 `config.json#memoryHarvest.enabled` 設為 `true` 即可啟用，設定值缺失或為 `false` 時維持停用狀態。
- **範圍嚴格受限：** GAL 絕不會主動開啟、列舉或搜尋編碼代理程式的交談歷史記錄或工作階段檔案來尋找候選內容。
- **侷限於儲存庫且去除識別：** 候選內容必須直接關聯至儲存庫內的特定檔案，且在對外呈現前需經過改寫與去識別化。原始對話引文、機密資訊、本機路徑與個人筆記一律不予收錄。
- **強制人工核准：** 候選內容僅能在執行 `/gal wrap-up` 期間進行核准。遭到拒絕或未處理的候選內容絕不會寫入檔案。經核准的內容會寫入當前進行中計畫的交接筆記，作為暫存且具參考性質的任務記憶。
- **核准的限制：** 核准並不保證晉升為正式文件。`/gal finalize` 僅在獨立驗證達到既定標準後，才會將核准的候選內容收錄至 `docs/` 中。

## 機器本機設定

### 個人內容根目錄 (`~/.gal/local/`)

本機個人內容存放於 `~/.gal/local/` 目錄下，經由標準根目錄算繪並投影至所有執行環境。`gal` 會讀取 `local/skills/`、`local/mcp.json` 與 `local/conventions/`，但**嚴格禁止寫入或刪除這些由使用者建立的目錄與檔案**。跨機器同步不屬 `gal` 的職責範圍。

#### 目錄配置

```text
~/.gal/local/
  skills/
    <skill-name>/
      SKILL.md          ← 自行放置的個人技能（gal 僅唯讀）
  mcp.json              ← 個人伺服器（格式同 plugins/gal-core/mcp.json，gal 僅唯讀）
  conventions/
    <lang>.md           ← 個人編碼風格慣例檔（gal 僅唯讀，詳見下文說明）
```

#### 啟用方式

啟用完全依據檔案是否存在，不依賴任何設定旗標。只要將檔案放入個人根目錄 `~/.gal/local/`，即視為選擇啟用。若目錄或檔案不存在，算繪結果將與純 Core 模式逐位元組完全一致。

#### 投影規則

- 個人技能在核心內容之後進行合併。若個人技能的名稱與核心技能衝突，個人項目會被直接略過，一律以核心為準 (core-wins)。
- 個人伺服器會併入標準的 `.mcp.json`。若個人伺服器與核心伺服器同名，個人項目亦會被略過，一律以核心為準。
- `gal doctor` 會回報個人技能數量、個人慣例檔數量，以及因與核心衝突而略過的次數。

### 編碼風格慣例

GAL Core 本身不包含任何個人專屬的編碼風格慣例。團隊自訂風格 (house style) 可經由三個獨立來源注入下游儲存庫中：

#### 來源一：個人慣例檔（永遠啟用、依儲存庫生效）

將隨附範例 `plugins/gal-core/templates/csharp-convention.example.md` 複製至 `~/.gal/local/conventions/csharp.md`，並修改為目標風格。個人慣例檔僅在主檔名 (stem) 為 `csharp`、`typescript` 或 `go` 三者之一時方會被採納，其餘主檔名一律予以排除，且不支援跨語言的主檔名。`typescript` 主檔名可同時對應儲存庫 `Language` 欄位中的 TypeScript 與 JavaScript，但主檔名本身不可命名為 `javascript`。原先依賴舊版「一律納入」行為的使用者，可將慣例檔主檔名更名為 `csharp`、`typescript` 或 `go` 以還原設定。在 `.dev/project.md` 的 `Language` 欄位符合條件的儲存庫中執行 `gal render-adapters`，系統即會將該檔案注入儲存庫轉接器中，處理機制比照 gal-core 內建慣例。

#### 來源二：已安裝外掛偵測（唯讀、零設定）

編碼代理程式中若已安裝官方語言外掛，僅需在目標儲存庫中執行一次 `gal render-adapters`。GAL 會將外掛技能的名稱與來源對應至 `.dev/project.md` 的 `Language` 欄位，算繪出一段 **Detected Language Skills** 參考區塊，註明技能名稱與持續載入的指示。GAL 嚴格禁止複製技能內容，亦不會主動安裝、更新或移除外掛。新安裝的外掛必須再次執行 `gal render-adapters` 方能被偵測納入。

#### 來源三：個人技能（依需求隨需載入）

在 `~/.gal/local/skills/<name>/` 目錄下建立一份包含清楚描述的 `SKILL.md`。Claude Code 需重新啟動方能完成偵測初始化，其餘代理程式則會在下一個讀取週期自動偵測。代理程式僅在被指名時才會載入此來源，不會像來源一與來源二般強制全域注入。

#### 來源挑選建議

若已具備官方語言外掛，建議優先採用來源二以達到零設定體驗。來源一適合維護全域常駐的個人風格並保有最高控制權，來源三則專供隨需諮詢的專門技能使用。

#### 未命中合約 (miss contract)

若儲存庫的 `Language` 欄位找不到任何符合的來源，轉接器即不會包含語言慣例或參考區塊。此為刻意設計的略過機制，非屬系統錯誤。

#### 停用來源一與來源二

在 `.dev/project.md` 的 Tech Stack 表格中加入 `| Personal Conventions | off |` 一列，即可停用來源一與來源二。公開儲存庫強烈建議加上此設定，以避免受追蹤的轉接器嵌入開發者個人的本機資訊。來源三則不受此設定影響。

### 指令技能的覆寫檔 (`SKILL.local.md`)

若需替指令技能加入本機專屬客製化內容，請建立 `plugins/gal-core/commands/<command>/SKILL.local.md`：

- 此檔案已列入 gitignore，屬使用者所有的本機輸入檔。
- 在產生最終的 `SKILL.md` 前，彙整流程會自動將 `SKILL.local.md` 的內容附加於後。
- 切勿直接修改 `plugins/gal-core/commands/<command>/SKILL.md`，該檔案為自動產生的產出物，隨時會被覆寫替換。
- `SKILL.local.md` 僅用於放置補充的指示內容，切勿包含第二份 front-matter。

### 本機筆記路由 (local-notes)

外部筆記屬於本機且選用的擴充元件。GAL 將儲存庫自身的狀態與使用者自有的筆記明確切分，核心運作完全不相依於任何私人筆記儲存機制。可攜的約束合約定義於 [`optional-capabilities.md`](../../../plugins/gal-core/conventions/optional-capabilities.md)。此功能獨立於特定應用程式、預設保持關閉，且僅在確認本機後端就緒時才會啟動。

後端若不存在、無法連線或尚未初始化，系統會自動降級為標準的儲存庫本機工作流程，不發出任何失敗警示。僅有經由明確選擇加入且保持連線的後端，方能在工作流程明確授權的環節進行讀取、檢索或寫入。筆記狀態絕非系統運作的必要條件。未配置筆記後端的儲存庫，除缺少一項選用的脈絡來源外，其餘行為與完整連線的環境完全一致。

目前已記載的後端實作範例包含 Obsidian (`coddingtonbear/obsidian-local-rest-api`)、Logseq (`ergut/mcp-logseq`)、Joplin (`joplin-mcp`)、通用 Markdown 筆記庫 (`vault-mcp`)，以及支援 CJK 優先檢索的 `SeekLink`。本清單不保證各實作間具備完整功能對等性。

儲存庫自有的研究成果預設存放於 `.dev/research/` 目錄下。

### 對話內角色派送

architect、analyst、designer 與 releaser 的獨立指令技能會使用 `gal consult-script <role>` 進行對話內諮詢。這項派送在本機執行，不使用 SSH pipeline executor 路由。舊版 `/gal discuss <role>` 會回報錯誤，並指出應改用的獨立指令。
