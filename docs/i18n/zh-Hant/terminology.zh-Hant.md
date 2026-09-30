---
source: docs/glossary.md
lang: zh-Hant
source_commit: a6215edfb68294314a0836a3af48411d6465faf7
translated_at: 2026-09-23
type: Reference
title: zh-Hant 術語呈現檔
description: zh-Hant 譯文的術語呈現標準，規定已定案的英文術語在繁體中文行文中的唯一寫法。
tags:
  - terminology
  - locale
  - presentation
  - zh-hant
status: stable
---

# zh-Hant 術語呈現檔

本檔案並非翻譯文件，而是繁體中文（zh-Hant）的術語呈現規範，不具備獨立的語意權威。本規範專注於一項核心職責：規範定案英文術語於繁體中文譯文中的呈現方式。術語的權威定義一律以 [`docs/glossary.md`](../../glossary.md) 為準，本檔案不另行重新定義或重複說明，分工原則請參閱[與 glossary.md 的關係](#與-glossarymd-的關係)。

本檔案提供 `docs/i18n/zh-Hant/` 目錄下所有受管譯文共用的用詞標準。在翻譯或修訂任何繁體中文文件之前，應先於本規範中確認對應術語的呈現寫法。若查無相應詞條且該詞存在多種譯法時，應先將該詞納入本檔案收錄後再行撰寫，以確保同一英文術語在全專案譯文中維持一致的繁體中文寫法。

本檔案以三張表格劃分權責：

1. [GAL 核心術語呈現表](#gal-核心術語呈現表)：完整涵蓋 `docs/glossary.md` 的 Part B、Part C 與 Part D。新增或刪除任何一列之前，均須先確認 `docs/glossary.md` 具備對應的收錄項目。
2. [一般技術用語：臺灣用語對照表](#一般技術用語臺灣用語對照表)：收錄兩岸用詞不同的一般技術詞彙。此類詞彙非 GAL 專屬概念，不屬於 `docs/glossary.md` 管轄範圍。
3. [譯文一致性補充詞](#譯文一致性補充詞)：收錄 `docs/glossary.md` 尚未收錄、兩岸用詞亦無顯著差異，但譯文中曾出現多種寫法的詞彙。此表僅固定呈現形式，不賦予語意權威。

## 排序規則：一律依英文字母排列

本檔案所有表格的列，一律依英文詞條的字母順序排列。排序時不分大小寫，並忽略反引號與開頭的符號，例如 `_galProjection` 排在字母 g、`(file) projection backend` 排在字母 f。詞條含多個字時逐字比較，例如 `source plan` 排在 `source root` 之前。

新增詞條時，請將其插入正確的英文字母排序位置，切勿直接附加於表尾。此規則有助於讀者直接依字母檢索，亦能使重複收錄的詞條因相鄰而便於及早發現。核心術語表不依 `docs/glossary.md` 的原始列序排列，完整性改由「來源」欄核對，兩者並不衝突。

## 用字紅線：只用臺灣用語

所有 zh-Hant 文件一律使用臺灣慣用的資訊技術用語，嚴格禁止使用中國大陸用語。此規則沒有例外，亦不因特定大陸詞彙「可被理解」而放行。用語混雜會損害技術文件的專業性與讀者信任。判斷有疑義時，以本檔案之 [一般技術用語：臺灣用語對照表](#一般技術用語臺灣用語對照表) 為準。對照表未收錄之詞彙，比照臺灣資訊出版品與官方標準慣例處理，不得沿用大陸譯法。

字形上全文使用正體中文，不得混入簡化字。為使自動檢查工具能有效比對，下方對照表的「大陸用語」欄一律以正體字書寫（例如寫「函數」而非「函数」）。因為譯文中實際容易出現的瑕疵，是被機械式轉換為正體字的大陸**詞彙**，而非簡化字形本身，以正體字列出才能被搜尋與檢查工具確實捕捉。

自動檢查可搭配 [zhtw-mcp](https://github.com/sysprog21/zhtw-mcp) 工具，該工具依據中華民國教育部《重訂標點符號手冊》、《國字標準字體》與兩岸技術詞彙對照資料進行分析。工具的檢查結果不會自動覆寫本檔案的規範，若有衝突應以人工逐條核對為準，因語法分析工具無法完全理解表格的說明語意，可能會將「禁止寫法」欄位中列出的對照詞彙誤判為違規。

## 繁中標點的呈現方式

儲存庫層級的分號政策定義於 [docs/architecture.md 的「Punctuation Policy: Semicolons Prohibited」](../../architecture.md#punctuation-policy-semicolons-prohibited)。以下僅說明繁中譯文如何依該政策改寫。若出現使用分號的意圖，通常代表單一語句中混雜了多個獨立句意，應予以拆解改寫。若兩件事物相互獨立，應斷為兩句。若兩者具備主從或因果關係，應使用逗號並搭配明確的轉折與關聯詞，例如「因此」、「然而」、「同時」或「另一方面」，以釐清關係並避免堆疊長句。

括號與冒號依內容決定形式。括號內為純英文或程式碼字面值時使用半形，例如「執行環境 (runtime)」與「（`plugins/gal-core/mcp.json`）」中的前者。括號內為中文說明或中英混合之子句時使用全形，例如「約束性行為（五態 preflight、structural-retrieval 路由）」。冒號銜接中文子句時使用全形，兩側皆為英文時使用半形。

## 五種呈現模式

[`docs/glossary.md`](../../glossary.md) 的「Semantic Authority vs. Locale Presentation Profiles」一節定義了五種呈現模式。繁體中文的具體判斷原則如下：

- **keep-en（保留原文）**：直接使用英文原詞，不作翻譯。所有機器可讀的字面值皆屬此類，包含指令名稱（`gal init`、`/gal pipeline`）、設定鍵值（`executorRouting`、`sshTarget`）、檔案路徑（`.dev/plans/`）、程式碼識別字、角色定義檔名（`golem-architect`）與產品名稱（GAL、Claude Code、Codex）。若翻譯此類字詞，將導致讀者無法對照實際操作輸入，因此一律保留原文。
- **bilingual-first-use（首次雙語）**：GAL 核心概念的預設呈現模式。於同一份文件內初次出現時，採用「中文詞 (English)」格式，後續可單獨使用中文詞或英文原文。**同一份文件內不得出現該詞的第二種中文寫法。** [GAL 核心術語呈現表](#gal-核心術語呈現表) 逐條固定了每個詞的中文寫法。
- **localized（在地化翻譯）**：一般性、非 GAL 專屬的通用技術詞彙，直接採用臺灣慣用技術譯名，不附註英文。對照表詳見後文。
- **transliterated（音譯）**：依讀音而非字義轉寫。**GAL 目前尚無術語採用此模式。** GAL 的專有名詞多為產品名稱或程式碼識別字，一律採用 keep-en。此處僅保留模式定義，不額外編造範例。
- **contextual（依語境決定）**：少數詞彙的呈現方式取決於上下文脈絡。當語句旨在討論術語本身（如說明命名規則或列舉角色名稱）時，優先採用 keep-en；若僅為一般敘述中提及之概念，則依 bilingual-first-use 或 localized 處理。

## GAL 核心術語呈現表

本表完整涵蓋 `docs/glossary.md` 的 Part B、Part C 與 Part D。「來源」欄標出該詞登錄在哪一個 Part，方便核對是否漏列。canonical 中合併於同一列的概念，可依繁體中文查表需要拆成多列。每個詞只有一種合法的中文寫法。「禁止寫法」欄列出實際發生過或容易誤用的寫法。

### Part B — 外部概念

| 英文術語 | 來源 | 呈現模式 | zh-Hant 固定寫法 | 禁止寫法 |
| --- | --- | --- | --- | --- |
| `agentic coding tool` / `agentic development platform` | Part B（`coding agent` 的別名） | keep-en | 保留原文。需要中文敘述時一律回到「編碼代理程式」 | 智能體編碼工具、智能體開發平臺 |
| `AGENTS.md standard` | Part B | bilingual-first-use | AGENTS.md 標準 (AGENTS.md standard) | AGENTS.md 規範 |
| `AI agent`（屬概念） | Part B | bilingual-first-use | AI 代理程式 (AI agent) | AI 代理、智慧代理、智能代理 |
| `chatbot` / `AI assistant` | Part B | localized | 聊天機器人 / AI 助理 | 智能助手、智能助理 |
| `coding agent` | Part B | bilingual-first-use | 編碼代理程式 (coding agent) | 編程代理、程式代理、代碼代理 |
| `MCP` | Part B | keep-en | 保留原文 | 模型脈絡協定、模型上下文協議 |
| `MCP client` | Part B | keep-en | 保留原文 | MCP 用戶端、MCP 客戶端 |
| `MCP host` | Part B | keep-en | 保留原文 | MCP 主機、MCP 宿主 |
| `MCP server` | Part B | keep-en | 保留原文 | MCP 伺服器、MCP 服務端 |
| `model` / `LLM` | Part B、Part C（保留裸字） | keep-en（縮寫） | 寫 `LLM`，泛指時寫「模型 (model)」 | 大模型、語言大模型 |
| `surface` / `interface` | Part B | localized | 介面 | 界面、接口 |

MCP 這組詞全部採用 keep-en，原因在於 `host`、`client`、`server` 在 MCP 規格中具有嚴格定義的角色。若譯成「主機／用戶端／伺服器」，讀者容易搞錯架構方向，因為代理程式本體為 host，server 才是提供工具的一方。保留原文可直接對應 `docs/glossary.md` Part B 的定義。

`docs/glossary.md` 所列之退役術語清單不於本檔案重複列出。命名檢查關卡會直接讀取 `docs/glossary.md`，重複列出僅會徒增過期副本的維護成本。

### Part C — GAL 內部詞彙

| 英文術語 | 來源 | 呈現模式 | zh-Hant 固定寫法 | 禁止寫法 |
| --- | --- | --- | --- | --- |
| `agent`（保留裸字，即 golem agent） | Part C | bilingual-first-use | 代理程式 (agent) | 代理、智慧代理、代理人 |
| `canonical root` | Part C | bilingual-first-use | 標準根目錄 (canonical root) | 正典根目錄、標準根、規範根目錄 |
| `dispatch` | Part C | bilingual-first-use | 派送 (dispatch) | 派發、分發、調度、分派 |
| `execution prompt` | Part C | bilingual-first-use | 執行提示檔 (execution prompt) | 執行提示詞、執行 prompt |
| `executor` | Part C | bilingual-first-use | 執行器 (executor) | 執行者、執行程式 |
| `executorRouting` | Part C | keep-en | 一律保留原文，不加中文 | 任何中譯 |
| `executorRouting.pipeline`（角色群組） | Part C | keep-en + 補述 | 保留原文，需要補述時寫「`pipeline` 角色群組」 | 管道組、流水線組 |
| `executorRouting.planning`（角色群組） | Part C | keep-en + 補述 | 保留原文，需要補述時寫「`planning` 角色群組」 | 規劃組、計劃組 |
| `(file) projection backend` | Part C | bilingual-first-use | 投影後端 (projection backend) | 映射後端、投射後端 |
| `GAL MCP boundary` | Part C | bilingual-first-use | GAL 的 MCP 邊界 (GAL MCP boundary) | 各運行時 MCP 服務器配置、MCP 伺服器設定 |
| `_galProjection` | Part C | keep-en | 一律保留原文，不加中文 | 任何中譯 |
| `generated runtime adapter` | Part C | bilingual-first-use | 轉接器 (adapter)，需要完整指稱時寫「產生的執行環境轉接器」 | 適配器、介面卡 |
| `golem agent` | Part C | keep-en + 補述 | `golem` 保留原文，需要補述時寫「golem 代理程式」 | 哥倫代理、魔像代理 |
| `local-first` | Part C | bilingual-first-use | 本機優先 (local-first) | 本地優先 |
| `local-notes` | Part C | bilingual-first-use | 本機筆記 (local-notes) | 本地筆記 |
| `Personal Enhancement` | Part C | bilingual-first-use | 個人化強化 (Personal Enhancement) | 個人增強 |
| `pipeline` | Part C | bilingual-first-use | 管道 (pipeline) | 流水線、管線、流程管道 |
| `plan-task ID` | Part C | bilingual-first-use | 計畫任務 ID (plan-task ID) | 計劃任務標識 |
| `projection` | Part C | bilingual-first-use | 投影 (projection) | 映射、投射 |
| `projection surface` | Part C | bilingual-first-use | 投影介面 (projection surface) | 映射界面、投射介面 |
| `runtime` | Part C | bilingual-first-use | 執行環境 (runtime) | 執行時期、運行時、執行階段 |
| `secrets` | Part C | keep-en | 一律保留原文，不加中文 | 任何中譯 |
| `source plan` | Part C | bilingual-first-use | 來源計畫 (source plan) | 原始計畫、原始碼計畫、源計畫 |
| `source root` | Part C | bilingual-first-use | 來源根目錄 (source root) | 源碼根、源根目錄 |
| `state` | Part C | localized | 狀態 | — |

- `state` 的中文寫法兩岸相同，列在此處是為了讓查表者確認「狀態」即為正解，無需另尋譯法。
- `pipeline` 具有兩個層次：單獨出現時代表 GAL 的實作管道，寫為「管道 (pipeline)」；若以 `config.json#executorRouting.pipeline` 形式出現時屬於設定鍵值，應保留原文不予翻譯。若行文同時提及兩者，應於上下文中明確辨析。

### Part D — 成員登錄表

| 英文術語 | 來源 | 呈現模式 | zh-Hant 固定寫法 | 禁止寫法 |
| --- | --- | --- | --- | --- |
| `generated adapter file` | Part D | localized | 產生的轉接器檔案 | 生成適配器文件 |
| `runtime key`（含五個代碼與產品名） | Part D | localized + keep-en | 概念本身寫「執行環境代碼」。代碼與產品名保留原文：`copilot`、`antigravity`（`agy`）、`codex`、`opencode`、`claude`，以及 GitHub Copilot、Google Antigravity、OpenAI Codex、opencode、Claude Code | 運行時鍵、runtime 鍵值、產品名的任何中譯 |
| `VALID_RUNTIMES` | Part D | keep-en | 一律保留原文，不加中文 | 任何中譯 |
| `vendor`（含供應商名稱） | Part D | localized + keep-en | 概念本身寫「供應商」。供應商名稱保留原文：GitHub、Microsoft、Google、OpenAI、Anthropic。`open source (SST)` 寫「開放原始碼專案（SST）」 | 提供商、供貨商、廠商、供應商名稱的任何中譯 |

「禁止寫法」欄包含兩類詞彙：一類是大陸用語，另一類雖屬臺灣常見用語，但本專案不採用其作為特定英文術語的對應寫法。例如 `vendor` 的「廠商」即屬後者，其雖為道地臺灣用語，但本專案統一採用「供應商」。兩類用語皆不得在受管譯文中使用。欄位標示為「—」則代表該詞目前尚無已知之誤用寫法。

## 一般技術用語：臺灣用語對照表

此類詞彙非 GAL 專屬術語，不在 `docs/glossary.md` 管轄範圍內，一律採用 localized 模式直接撰寫中文。左欄為專案唯一採用寫法，右欄為**嚴格禁用**的大陸用語，對照表一律以正體字書寫。

**此為概念層級的禁用，並非全域文字過濾。** 右欄部分詞彙在臺灣文意中另有合法用法，例如「文件」指稱 document、「目錄」指稱 directory、「對象」指稱一般日常語意之對象。禁用僅在該詞被用於表達左欄英文技術概念時成立。

兩岸用語相同且可直接使用的常見詞彙（例如編譯、重構、部署、觸發、目錄、索引、封裝、序列化等）不另行列入下表。

### 開發與程式

| 英文原文 (en-US) | 臺灣用語（採用） | 大陸用語（禁用） |
| --- | --- | --- |
| algorithm | 演算法 | 算法 |
| argument / parameter | 引數 / 參數 | 實參 / 形參 |
| array | 陣列 | 數組 |
| boolean | 布林值 | 布爾值 |
| build | 建置 | 構建 |
| checkout（git） | 簽出 | 檢出、拉取 |
| class | 類別 | 類 |
| command | 指令 | 命令 |
| debug | 除錯 | 調試 |
| dependency | 相依性 / 相依套件 | 依賴 / 依賴包 |
| function | 函式 | 函數 |
| implement | 實作 | 實現 |
| nested | 巢狀 | 嵌套 |
| object | 物件 | 對象 |
| open source | 開放原始碼 | 開源 |
| override / overwrite | 覆寫 | 覆蓋、重寫 |
| package | 套件 | 包 |
| plugin | 外掛 | 插件 |
| process | 行程 | 進程 |
| program / code | 程式、程式碼 | 程序、代碼 |
| project | 專案 | 項目 |
| recursion | 遞迴 | 遞歸 |
| render | 算繪 | 渲染 |
| repository | 儲存庫 | 倉庫、程式碼倉 |
| return | 回傳 | 返回 |
| string / character | 字串 / 字元 | 字符串 / 字符 |
| thread | 執行緒 | 線程 |
| variable / constant | 變數 / 常數 | 變量 / 常量 |

**用語一致性原則**：受管譯文一律統一採用「相依」、「相依性」、「相依套件」，嚴格禁止使用「依賴」。

**`render` 譯為「算繪」之緣由**：依中華民國教育部重編國語辭典修訂本，「渲染」本義為國畫水墨暈染技法，引申為文字誇大之意，與電腦圖學與軟體架構中「計算後繪製輸出（rendering）」之語意不合。臺灣資訊領域之標準規範譯名為「算繪」，故本專案一律統一採用「算繪」。

### 檔案與系統

| 英文原文 (en-US) | 臺灣用語（採用） | 大陸用語（禁用） |
| --- | --- | --- |
| bit / byte | 位元 / 位元組 | 比特 / 字節 |
| cache | 快取 | 緩存 |
| clipboard | 剪貼簿 | 剪貼板 |
| file | 檔案 | 文件 |
| folder | 資料夾 | 文件夾 |
| hard drive | 硬碟 | 硬盤 |
| memory | 記憶體 | 內存 |
| queue | 佇列 | 隊列 |
| screen | 螢幕 | 屏幕 |
| shortcut | 捷徑 | 快捷方式 |
| stack | 堆疊 | 棧、堆棧 |
| timeout | 逾時 | 超時 |
| uninstall | 解除安裝 | 卸載 |

「文件」一詞在臺灣技術語境專指 document，即文書或說明檔案，並非 file。行文提及 file 一律寫為「檔案」，提及 document 始寫為「文件」，兩者不得混用。

### 網路與安全

| 英文原文 (en-US) | 臺灣用語（採用） | 大陸用語（禁用） |
| --- | --- | --- |
| account | 帳號 | 賬號、帳戶 |
| certificate / credential | 憑證 | 證書、憑據 |
| client | 用戶端 | 客戶端 |
| hash | 雜湊 | 哈希、散列 |
| key | 金鑰 | 密鑰 |
| link / URL | 連結 / 網址 | 鏈接 |
| login / logout | 登入 / 登出 | 登錄 / 註銷 |
| network | 網路 | 網絡 |
| server | 伺服器 | 服務器 |
| session | 工作階段 | 會話 |
| signature | 簽章 | 簽名 |
| token | 權杖 | 令牌 |
| user | 使用者 | 用戶 |

### 流程與品質

| 英文原文 (en-US) | 臺灣用語（採用） | 大陸用語（禁用） |
| --- | --- | --- |
| artifact（建置產出） | 產出物 | 製品、工件 |
| audit | 稽核 | 審計 |
| compatible | 相容 | 兼容 |
| configuration / settings | 設定 | 配置 |
| context | 脈絡 | 上下文 |
| convention | 慣例 | 約定 |
| default | 預設 | 默認 |
| import / export | 匯入 / 匯出 | 導入 / 導出 |
| information / message | 資訊 / 訊息 | 信息 / 消息 |
| lockfile | 鎖定檔 | 鎖文件 |
| optimize | 最佳化 | 優化 |
| orchestrator / orchestration | 協調器 / 協調 | 統籌者、編排 |
| performance | 效能 | 性能 |
| plan | 計畫 | 計劃 |
| quality | 品質 | 質量 |
| receipt | 收據 | 回執 |
| registry | 登錄檔 | 註冊表、註冊中心 |
| release（軟體） | 釋出 | 發佈、發布 |
| restore | 還原 | 恢復 |
| review | 審查 | 評審 |
| support | 支援 | 支持 |
| troubleshoot | 疑難排解 | 故障排除 |
| workflow | 工作流程 | 工作流 |

針對容易混淆之概念，其正確對應如下：`gate` 譯為「關卡」，不可寫為「閘道」（gateway 的固定譯名）；`slug` 譯為「代稱」（用於網址或檔名之簡短字串），不可寫為「識別碼」（identifier 的固定譯名）；`axis` 譯為「軸向」，不可寫為「維度」（dimension 的固定譯名）。

## 譯文一致性補充詞

此類詞彙尚未登錄於 `docs/glossary.md`，兩岸用詞亦無明顯差異，但譯文中曾出現多種寫法。本表僅固定呈現寫法，**不賦予任何語意權威**。若其中任何詞彙晉升為 GAL 核心架構概念，必須先於 `docs/glossary.md` 登錄，再行移入 [GAL 核心術語呈現表](#gal-核心術語呈現表)。

| 英文術語 | 呈現模式 | zh-Hant 固定寫法 | 禁止寫法 |
| --- | --- | --- | --- |
| `contract` | localized | 合約 | 契約、約定 |
| `fail-closed` | keep-en + 補述 | 首次寫「fail-closed（預設阻擋）」，之後單寫 `fail-closed` | 失效關閉、封閉失效 |
| `finalize` / `land` | localized | 落地 | 落實、著陸 |
| `handoff notes` | localized | 交接筆記 | 交接說明、移交筆記 |
| `headless` | localized | 無頭 | 無介面、無頭部 |
| `hunk`（diff） | keep-en | 保留原文，需要時寫「hunk 標頭」 | 大塊、區塊標頭 |
| `loud failure` | localized | 明確報錯失敗 | 響亮失敗、大聲失敗 |
| `orchestrated-only` | localized | 僅限協調器驅動 | 僅限編排、只能被編排 |
| `skill` | localized | 技能 | 技巧 |
| `spawn` | keep-en + 補述 | 首次寫「啟動 (spawn)」，之後單寫「啟動」 | 衍生、孵化、派生 |
| `steel-man` | bilingual-first-use | 強化論證 (steel-man) | 鐵人論證、稻草人反面 |
| `worktree`（git） | localized | 工作樹 | 工作區、工作目錄 |

其中 `fail-closed`、`finalize`／`land`、`orchestrated-only`、`loud failure`、`handoff notes` 已頻繁出現於 GAL 約束性合約文字中，性質相較於一般詞彙更接近核心術語。待未來於 `docs/glossary.md` 正式收錄後，將同步移入核心術語表。

## 與 glossary.md 的關係

本檔案僅提供呈現指引。術語的意義定義、命名規則（Part A）、術語對照表（Part B/C）、成員清單（Part D）與退役術語清單，一律以英文正本 [`docs/glossary.md`](../../glossary.md) 為準，該檔案永遠具備最終語意權威。

維護規範說明：[GAL 核心術語呈現表](#gal-核心術語呈現表) 必須完整對應 `docs/glossary.md` 的 Part B、Part C 與 Part D。當 `docs/glossary.md` 增刪詞條時，本檔案必須同步檢視並更新對應列。在刪除或修改核心表任何項目之前，請務必先確認 `docs/glossary.md` 的最新收錄狀態。其餘兩張表格涵蓋一般技術詞彙，可獨立擴充維護，但不得用於定義或更動任何 GAL 核心概念之語意。
