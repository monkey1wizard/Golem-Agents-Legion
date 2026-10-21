---
source: docs/configuration.md
lang: zh-Hant
source_commit: cc5631ef3e38180f3835e67ba98f61666711f47d
translated_at: 2026-10-21
type: Guide
title: 設定
description: 管理 `~/.gal/config/config.json` 的機器本機設定，涵蓋執行器路由、宿主子代理程式的模型 tier、計畫語言、MCP 來源、個人層與本機筆記路由。
tags:
  - configuration
  - routing
  - mcp
  - personal-layer
status: stable
---

# 設定 (Configuration)

[English](../../configuration.md) · **繁體中文** · [日本語](../ja/configuration.ja.md)

## 本機設定檔 (config.json)

GAL 的本機設定集中在 `~/.gal/config/config.json`。這些設定值僅屬於當前機器，請勿將本機設定值寫入任何受追蹤的文件、指令範本或原始碼檔案。已知設定鍵包含 `galSkills`、`planLanguage`、`memoryHarvest`、`executorRouting` 與 `hostSubagentModels`。勤務時間的行為不是 `config.json` 的物件。代理程式在本機設定啟用時，會套用 [working-hours.md](../../../plugins/gal-core/conventions/working-hours.md) 所描述的值。`crates/` 底下沒有任何 crate 會讀取這些值。GAL 會從各執行環境自身的設定偵測外掛註冊狀態。既有的 `pluginMode` 屬性已退役且會忽略，`gal doctor` 會將它列為過時鍵。`schemaVersion` 屬於 `plugins.lock.json`，不屬於 `config.json`。`enabledPlugins` 是 GitHub Copilot 自身 `settings.json` 的設定，GAL 會將它作為註冊依據讀取。

已棄用的 `devMode`、`galRoot` 與 `secrets` 鍵會被直接忽略。`gal doctor` 偵測到 `devMode` 或 `galRoot` 時會發出不阻擋執行的警示。`secrets` 鍵已正式退役，原本供已移除的 MCP host 寫入器取用，gal 不會自此檔案讀取任何憑證資料。

### 主要欄位

| 欄位 | 必要 | 型別 | 說明 |
| --- | --- | --- | --- |
| `executorRouting` | 選用 | 物件 | 依取用端分組的角色路由，為系統唯一的路由來源。詳見下節說明。 |
| `galSkills` | 選用 | 字串 | 指向本機技能目錄的路徑，為唯一保留的本機路徑替代鍵。 |
| `hostSubagentModels` | 選用 | 物件 | 宿主子代理程式派送使用的宿主與 tier 模型覆寫。詳見[宿主子代理程式 (host subagent) 派送模式](#宿主子代理程式-host-subagent-派送模式)。 |
| `planLanguage` | 選用 | 字串 | `.dev/plans/*.md` 與 `.dev/research/*.md` 在未給定明確指示時的預設輸出語言。詳見下文。 |
| `memoryHarvest` | 選用 | 物件 | 內容格式為 `{ enabled: boolean }`，預設為關閉的供應商記憶收割設定。詳見下文。 |

外部筆記後端不屬於 Core 設定範疇。其可攜合約定義於 `plugins/gal-core/conventions/optional-capabilities.md`。儲存庫自有的研究成果預設置於 `.dev/research/`，GAL 的暫存資料則使用儲存庫相對路徑 `.dev/tmp`。

## 執行器路由 (executorRouting)

### 宿主子代理程式 (host subagent) 派送模式

角色有路由時，派送模式預設為 `routed`。目前對話中提出的請求會把模式固定為 `host-subagent`，並覆蓋有效的路由。完成審查一律使用其路由。路由無效時，若已固定宿主或處於完成審查階段，派送會停止。未固定宿主的一般派送中，無效路由會交由寬鬆的載入器處理。若該角色沒有可用的項目，派送會回報 `no-routing`。沒有路由但已固定宿主時，模式為 `host-subagent`。沒有路由也沒有固定宿主時，模式為 `text`。實際輸出的 `DISPATCH_MODE` 值是：routed 派送輸出 `offload`，另外兩個是 `host-subagent` 與 `text`。確切的切換語句、宿主選擇、嘗試生命週期、權限規則與 PASS 條件，定義在[管道 skill](../../../plugins/gal-core/commands/gal-pipeline/SKILL.template.md#dispatch-mode) 中。

宿主能力檢查必須確認所選宿主能產生子代理程式，並能取得其完成證據。Claude Code 使用 `--host claude`。具備能力的 Codex Desktop 或 Codex CLI 使用 `--host codex`。Codex Desktop 與 Codex CLI 的能力證據分開認定。實機驗收已在 Claude Code、Codex CLI 0.162.1 與 Codex Desktop 26.1013.31256 通過。Codex 的每個階段都以 `fork_turns: "none"` 以及指定的模型與 effort 產生新的預設子代理程式。能力檢查失敗或宿主無效時會直接拒絕，不會悄悄改用其他做法。

內建模型對應中，Claude 的 light 使用 Haiku，standard 使用 Sonnet，deep 使用 Opus，並支援以 Fable 作為明確指定的 deep 替代選項。Codex 的 light 使用 Luna，standard 使用 Sol，deep 使用 Astra。模型與 effort 是否可用，取決於宿主與帳號。任務內容需要 deep tier 時，可明確指定 deep 使用 Sol 或 Astra。明確設定 `inherit` 屬於降級的退出選項，會記錄未套用模型 tier 政策，也無法滿足模型 tier 驗收。模型或 effort 無法選用時，絕不可悄悄改用 inherit。

`hostSubagentModels` 是 `config.json` 的頂層鍵，可以覆寫宿主與 tier 的對應。格式是 `{"codex"|"claude": {"light"|"standard"|"deep": {"model": "...", "effort": "..."}}}`。限制如下：

- 只接受 `codex` 與 `claude` 兩種宿主，只接受 `light`、`standard` 與 `deep` 三種 tier。其他宿主或 tier 鍵都會發生錯誤。
- Codex 項目的 `model` 必須是 `gpt-6-luna`、`gpt-6.1-sol` 或 `gpt-6-astra`。`effort` 為必填，且必須是 `medium`、`high` 或 `xhigh`。
- Claude 項目的 `model` 必須是 `haiku`、`sonnet`、`opus` 或 `fable`。`effort` 可以省略。填寫時只能是 `low`、`medium`、`high`、`xhigh` 或 `max`。
- `inherit` 模型不能帶 `effort`。
- 模型或 effort 不支援、值不是物件，或項目格式錯誤，都會發生錯誤。嘗試會停止，不會退回預設對應。

開啟步驟是唯一的解析者，會記錄解析出的模型、effort、tier、tier 來源與覆寫原因。協調器以 `HOST_SUBAGENT_ATTEMPT:` 中的 `model=` 值作為產生子代理程式時的模型。只有在任務內容需要與計算結果不同的 tier 時，才會同時傳入 `--tier` 與 `--tier-reason`。在 guarded 接續模式下，一律使用計算出的 tier，不接受對話中的覆寫。

Codex 桌面版的核准模式與 `config.toml` 設定方式，請參閱[安裝與初始化](./setup.zh-Hant.md#codex-管道執行設定)。GAL 不會修改 Codex 的主機端設定。`executorRouting` 與 `timeoutSecs` 屬於 GAL 的設定項目，必須與 Codex 的核准及沙盒設定分開確認。

### Preflight 派送旗標

`gal pipeline-preflight` 使用以下旗標固定派送模式：

- `--dispatch routed` 或 `--dispatch host-subagent` 設定固定的模式。預設值為 `routed`。`--dispatch host-subagent` 必須同時提供 `--host`。
- `--host claude` 或 `--host codex` 固定宿主。其他值會被拒絕。
- `--host-session <host>:<session-id>` 指定協調器的工作階段。preflight 會驗證該工作階段符合固定的宿主與工作目錄。參照必須使用固定的宿主。`--host-session` 不可與 `--terminal-reverify` 併用。未提供時，記錄的協調器綁定為不可用。

### 消費者群組與角色項目

`executorRouting` 依取用端將角色劃分至三組物件中：負責派送的 `pipeline` 群組包含 `CODER`、`TESTER`、`AUDITOR`，負責審查的 `planning` 群組包含 `ARCHITECT`、`ANALYST`、`DESIGNER`、`RELEASER`，`research` 則為選用群組，以編號鍵 `"1"` 與 `"2"` 路由研究工作者。三組共用同一套 `executors` 預設區塊。此區塊為系統唯一的路由來源，早期扁平結構已退役不再解析，且會觸發命名警示，原先獨立的路由檔亦不再被讀取。

`planning` 群組僅供應 Codex 原生代理程式投影，`pipeline` 與 `research` 群組則使用無頭派送。詳見[規劃群組範圍](#規劃群組範圍)。

字面執行器名稱為 `claude`、`codex`、`opencode`、`copilot` 與 `agy`。`executor` 的值必須是其中之一，或是 `combinations` 項目的名稱。

以 `_` 開頭的鍵是註解。GAL 會在 `combinations`、各消費者群組、`research`，以及 `executorRouting` 的頂層中靜默略過這類鍵。`executors` 區塊不會略過它們。直接位於 `executorRouting` 下的扁平角色鍵已退役，會發出警示。其他無法辨識的頂層鍵也會發出警示。

#### 角色項目結構

角色項目位於所屬群組之下，例如 `executorRouting.pipeline.CODER`。

| 欄位 | 必要 | 型別 | 說明 |
| --- | --- | --- | --- |
| `executor` | 必要 | 字串 | 字面執行器名稱，或 `combinations` 的名稱。 |
| `model` | 選用 | 字串 | 覆寫組合與 `executors` 的預設模型。 |
| `effort` | 選用 | 字串 | 推理深度。覆寫組合與 `executors` 的預設 effort。 |
| `timeoutSecs` | 選用 | 正整數 | 覆寫預設逾時時間。`0` 會被忽略並發出警示。 |
| `sshTarget` | 選用 | 字串 | 遠端 SSH 目標。須與 `remoteWorkdir` 一併設定。 |
| `remoteWorkdir` | 選用 | 字串 | 遠端工作目錄。須與 `sshTarget` 一併設定。 |

角色鍵區分大小寫：

- `pipeline` 接受 `CODER`、`TESTER` 與 `AUDITOR`。
- `planning` 接受 `ARCHITECT`、`ANALYST`、`DESIGNER` 與 `RELEASER`。
- `research` 只接受[研究路由](#研究路由)所述的編號工作者鍵。

放錯群組的角色，或無法辨識的角色，會被略過並發出警示。

### 規劃群組範圍

`planning` 群組只適用於四個審查角色 `ARCHITECT`、`ANALYST`、`DESIGNER` 與 `RELEASER`。設定鍵必須大寫，例如 `ARCHITECT`。斜線指令名稱維持小寫。在 Codex 代理程式投影期間，GAL 會查出每個角色解析後的 `model` 與 `effort`，並寫入投影出的 Codex 代理程式定義。這項查詢不會啟動執行器，也不會把工作轉移到其他執行器。

- 直接呼叫 `/gal <role>`（隔離模式）會輸出 `ROUTING_SCOPE: codex-native-projection-only` 與一則 `ROUTING_NOTE`。該註記說明三件事：planning 設定只供應 Codex 原生代理程式投影、不會啟動任何執行器、實際使用的模型由主機決定。這段文字是靜態的契約陳述，不能證明任何設定已經套用。
- `planning` 角色中的 `executor` 鍵仍會被讀取。它指定查詢 `model` 與 `effort` 時所用的 `combinations` 項目或 `executors` 項目。優先順序為角色內嵌值、組合、`executors` 預設值。沒有 `planning` 項目的角色兩個值都不會取得，因此 Codex 沿用自己的模型。這個鍵不會選擇要啟動的程序。
- `timeoutSecs`、`sshTarget` 與 `remoteWorkdir` 對 planning 角色不生效，因為這些角色不會執行無頭程序。請只對 `pipeline` 與 `research` 角色設定這些欄位。
- GAL 不保證 Codex 以外的執行環境會套用解析出的 `model` 或 `effort`。無論有無 planning 設定，這些環境都維持各自的原生子代理程式行為。
- 透過 `/<role> discuss` 進行的對話內諮詢在目前工作階段中執行，不會啟動執行器。

### 完成審查路由

計畫層級的完成審查透過 `gal pipeline <execution-prompt> --phase finalize-review` 執行。此階段使用既有的 `pipeline.AUDITOR` 路由，不新增角色或設定鍵。

- 若設定檔、`executorRouting` 鍵、`pipeline` 群組或 `AUDITOR` 項目確實不存在，派送會回報 `no-routing`。這種情況允許改為在程序內審查，並標示 `Review Independence: DEGRADED_SAME_RUNTIME`。明確選用宿主子代理程式模式且能力檢查通過時，是另一個文件記載的審查例外。
- 若設定檔無法讀取或不是有效的 JSON，派送會回報 `routing-invalid` 並停止。`executorRouting` 或 `pipeline` 不是物件時，或 `AUDITOR` 項目格式錯誤、`executor` 為空、解析不出模型、只設定了 `sshTarget` 與 `remoteWorkdir` 其中之一時，結果相同。這些情況下 GAL 不會退回同一執行環境。此階段中，GAL 不檢查其他角色的項目。
- 執行器不存在、啟動失敗、逾時、結束碼不為零、收據缺漏或為空、缺少提供者的工作階段證據，同樣會使審查停止。這些情況都不會退回同一執行環境。
- 審查成功需要一份新的非空收據、以 `completed` 結束的嘗試日誌，以及提供者的工作階段證據。不需要產品差異。

未固定宿主時，其他派送的行為不變。對它們而言，無法讀取或格式錯誤的設定檔仍會降級為 `no-routing`。固定宿主時則會停止。

### combinations 登錄清單

`executorRouting.combinations` 是具名且可重複使用的執行器組合登錄清單。每一筆組合包含 `executor` 以及選用的 `model`、`effort`、`timeoutSecs`、`sshTarget` 與 `remoteWorkdir`。對 `planning` 角色，只會讀取 `model` 與 `effort`。角色可以直接指定組合名稱作為自身的 `executor` 值，隨後角色的各欄位設定會逐一覆寫組合的預設值。`sshTarget` 與 `remoteWorkdir` 必須成對提供。組合的 `executor` 必須是字面執行器名稱，不能是另一個組合。組合名稱不能與字面執行器名稱相同。不合格的組合會被拒絕並發出警示。

### executors 預設區塊

`executorRouting.executors` 的每個項目既可以是既有的純字串簡寫，也可以是一個物件：

| 欄位 | 必要 | 型別 | 說明 |
| --- | --- | --- | --- |
| `model` | 必要 | 字串 | 執行器的預設模型。 |
| `effort` | 選用 | 字串 | 執行器的預設推理深度 (effort)。 |
| `timeoutSecs` | 選用 | 正整數 | 執行器的預設逾時時間。`0` 會被忽略並發出警示。 |

針對已解析的角色，`effort` 與 `timeoutSecs` 的退回順序固定為：**角色行內設定 → 組合基底 → `executors[final.executor]` → `None`**，模型的查找鏈結則以空字串為終點。角色層級若明確設定 `timeoutSecs: 0`，會解析為 `None`，且不會向下退回至組合或執行器的預設值。

### 研究路由

`research` 納入派送表的方式與 `pipeline` 相同，兩者皆路由至無頭派送。這與 `planning` 不同，`planning` 保持僅驗證原則、絕不執行無頭派送。研究獨立性建立在三工作者模型上：`RESEARCHER#0`、`RESEARCHER#1` 與 `RESEARCHER#2` 以三位平行且彼此盲測（隔離各階段輸出）的工作者執行，三者皆回傳結果後由協調器裁定，而非由任一工作者代為裁定。此設計並未引入驗證派送階段，`executorRouting` 亦未新增 `verify` 鍵，`research` 僅負責路由工作者的挑選。群組鍵 `"1"` 路由 `RESEARCHER#1`，鍵 `"2"` 路由 `RESEARCHER#2`。鍵 `"0"` 會被略過並發出警示，因為工作者 #0 永遠是原生子代理程式。`RESEARCHER` 這類角色名稱的鍵同樣會被略過並發出警示。大於 `2` 的鍵會被載入，但沒有研究派送會讀取它們。沒有 `research` 項目的工作者，若有設定 `executors.agy` 預設值就使用該值，否則以原生子代理程式執行。

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
| `<GAL_SKILLS>` | `galSkills` 鍵指向的本機技能目錄絕對路徑 | `gal clean` 與 `gal smudge` Git 篩選器 |

`galSkills` 為目前唯一支援本機路徑替代的設定值，其他設定鍵不參與占位符替換。

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
3. 將合併後的伺服器清單，以 Agent Plugins 標準格式寫入 `~/.gal/plugins/gal/mcp.json`。此檔案中，`"type": "http"` 會改寫為 `"streamable-http"`。`stdio` 與 `sse` 等其他類型維持不變。`.mcp.json` 檔案仍保留 `"http"`。

伺服器設定內容不進行占位符替換，唯一的變更是步驟 3 的 `type` 改寫。伺服器設定中若含有 `${ENV_VAR}`，會由 MCP host 於載入檔案時解析，通常取自執行行程的環境變數。建議將這類值設定於編碼代理程式執行的環境中，切勿寫入 `config.json`。

GAL 不支援覆寫檔。如需調整核心伺服器，請編輯 `plugins/gal-core/mcp.json`，並從來源工作樹執行 `gal dev-apply`。如需調整個人伺服器，請編輯 `~/.gal/local/mcp.json`，再重新執行 `gal refresh`。Claude Code 需重新啟動或重新加入外掛，方能讀取更新後的設定。

個人層會在本機執行 `gal refresh` 或 `gal dev-apply` 時合併。建置公開的外掛市集快照時不會包含個人層，所以個人伺服器不會離開這台機器。

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

## 計畫語言 (planLanguage)

`config.json` 中的 `planLanguage` 為選用的本機設定，用以決定未給定明確指示時，`.dev/plans/*.md` 與 `.dev/research/*.md` 的預設輸出語言。解析優先順序為：明確指示 → `planLanguage` → 提示語言自動偵測 → 最後退回使用 `en`。受追蹤的 `PROJECT_LANGUAGE` 專案中繼資料負責規範主要文件的標準語言，與 `planLanguage` 屬於不同層級。`.dev/plans/*.prompt.md` 執行提示檔維持嚴格英文，以確保跨模型的穩定性。

若將 `planLanguage` 設定為非英文，系統會產生三層計畫架構：第一層為置於 `.dev/plans/<slug>.en.md` 的英文語意草稿，作為技術定義的權威依據。第二層為置於 `.dev/plans/<slug>.md` 的在地化來源計畫，供開發者閱讀與編輯，其中的標題、路徑、任務 ID 與判定結果維持英文，敘述文字則改用在地化語言。第三層為英文執行提示檔。系統完全支援在地化計畫的手動編輯。後續的規劃階段指令會偵測編輯內容，停在唯讀的協調步驟，將手動修改的意圖合併回英文草稿後再接續進行。手動編輯的內容絕不會遭到覆寫。執行 `/plan-to-prompt` 後，草稿檔案會自動刪除，每個計畫僅會保留兩份受追蹤的檔案。若 `planLanguage` 設定為英文，則會直接略過這整套雙語機制。

## 用量記帳輸入檔

執行 `gal usage collect` 時，用量記帳會讀取兩個可選的逐次執行輸入檔。這兩個檔案都不屬於 `~/.gal/config/config.json`，也都不含權杖或成本計數。合約是 [`usage-accounting.md`](../../../plugins/gal-core/conventions/usage-accounting.md)。

```text
gal usage collect --run <run-id> [--binding <path>] [--rates <path>]
```

### 綁定檔（`--binding`）

preflight 已記錄協調者綁定時，`--binding` 可以省略。執行 `gal pipeline-preflight` 時傳入 `--host-session <host>:<session-id>`，preflight 會依照固定的主機與工作目錄驗證該工作階段。驗證通過後，不帶 `--binding` 的 `gal usage collect --run <run-id>` 會使用這份已記錄的綁定。preflight 沒有記錄綁定時，收集會失敗並說明原因，這時必須傳入 `--binding`。

綁定檔會指定執行、工作階段與來源時段。它是一個 JSON 物件，只能包含以下欄位：`run`、`host_session`、`billing_mode`、`role`、`window`，以及可選的 `cursor`。

```json
{
  "run": {
    "version": 1,
    "run_id": "run-a",
    "canonical_worktree": "C:/work/repo",
    "plan_scope": "plan-a",
    "host_session": "codex:sess-a",
    "host_window": "window-a"
  },
  "host_session": "codex:sess-a",
  "billing_mode": "standard",
  "role": {"kind": "orchestrator"},
  "window": {"started_at": "2026-10-20T00:00:00.000Z"}
}
```

- 任何其他欄位都會使收集失敗。這項規則也涵蓋 `input_tokens` 等計數欄位，以及評估欄位。
- `run.run_id` 必須與 `--run` 的值相同。
- 若 `host_session` 與執行身分中的 host session 不同，收集會以重新綁定衝突失敗。這會在讀取任何來源之前發生。

### 費率集檔（`--rates`）

費率檔是擁有者提供的費率集，結構版本為 `1`。GAL 不會取得費率。

```json
{
  "schema_version": 1,
  "rate_set_id": "rates-a",
  "rates": [
    {
      "provider": "codex",
      "model": "gpt-x",
      "billing_mode": "standard",
      "category": "input",
      "unit": "per_million_tokens",
      "price": "2.5",
      "money_unit": {"currency": "USD"}
    }
  ]
}
```

- 費率以供應商、實際觀測到的模型、計費模式與單位為索引。設定的路由名稱絕不會取代實際觀測到的模型。
- 價格必須是十進位字串。負價格會使解析失敗。單次請求價格不能用來計算權杖計數。
- 未提供費率檔時，所有依費率推算的估算都維持未知，例如報告顯示 `estimate: unknown`。
- 費率集變更會建立新快照。先前的快照不會改變。費率集變更只會改變估算值。單位總計（例如重試任務的 160 與包含子項父層的 200）不會改變。
- `gal usage report` 只讀取已儲存的紀錄。`gal usage assess` 接受評估輸入，不需要綁定檔或費率檔。

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

## 儲存庫衛生檢查

`gal finalize-check` 包含 `project-md-bound`，用來檢查 `.dev/project.md` 是否超過正規化 LF 後 UTF-8 計算的 30,720 B 建議上限。`state-bound` 會檢查 `.dev/state.md` 的 16,384 B 上限及有限表格結構。衛生發現是這兩個有限索引中的可修復問題，支援的來源文件結構或路徑發現也屬建議事項。計畫、提示、收據、研究紀錄、核准證據、根轉接器、未支援的狀態衝突及同位置衝突則屬完整性發現。衛生發現會標示為 `advisory`，不會阻擋收據。檔案無法讀取及完整性問題仍會標示為 `fail`。`pass` 與 `advisory` 視為收據通過狀態，`fail` 與 `not-run` 則會阻擋。

在管道預檢及落地完整模式關卡之前，GAL 會以區段局部編輯修復 `.dev/project.md` 與 `.dev/state.md` 的衛生問題，然後重跑同一檢查一次。其他檔案不會自動修復。同一次呼叫中的關卡失敗後也不會繼續修復。若複檢後仍有問題，GAL 會依執行中的計畫 slug，在 `.dev/state.md` 新增或更新一筆 `Hygiene debt:` 列，並在交接及落地報告中列出。成功複檢後，會移除該列。`project-md-bound` 上限及所有發現分類、修復、還原、債務與完整性規則，以[`.dev` 衛生與完整性規範](../../../plugins/gal-core/conventions/token-budget.md#dev-findings-hygiene-and-integrity)為唯一依據。

即使其他路徑也未合併，`gal state-merge` 仍可合併 `.dev/state.md` 中支援且互不重疊的索引位置，並保留其他未合併路徑。相同位置衝突、權限敏感檔案衝突及支援範圍外的衝突仍會停止流程。翻譯文件的來源路徑、語言及來源提交記錄在本文的翻譯中繼資料中。英文文件是唯一規則來源。
