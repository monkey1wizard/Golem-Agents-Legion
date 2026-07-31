---
source: docs/naming.md
lang: zh-Hant
source_commit: cf2b5a0b3734d995e7c054b0387e1c2667ba84fd
translated_at: 2026-07-23
status: current
---

# zh-Hant 術語呈現檔

> **這並非翻譯。** 本檔為 [`docs/naming.md`](../../naming.md) 的 **zh-Hant 呈現檔**，不具任何語意權威。每個 GAL 核心術語的意義，一律以 `docs/naming.md` 為唯一真實來源，該檔涵蓋 Part A 規則、Part B/C 術語表以及退役術語清單，本檔不重複這些內容。
>
> 本檔只回答一件事：已定案的英文術語，在 zh-Hant 行文中該怎麼寫。本檔不解釋術語的意義。若本檔與 `docs/naming.md` 對某個術語的意義有出入，一律以 `docs/naming.md` 為準，並回頭修正本檔。

## 文件職責

本檔提供所有 zh-Hant 譯文（`README.zh-Hant.md`、`manual.zh-Hant.md`、`integrations.zh-Hant.md`）共用的用字標準。翻譯或修改任何 zh-Hant 文件之前，必須先在此查出該詞的寫法。若查不到，而該詞又可能寫成兩種以上的形式，必須先把它登錄進本檔再動筆。本檔的目標只有一個：讓同一個英文術語在所有譯文裡只有一種中文寫法。

本檔用三張表達成這件事，各自的權責不同：

1. [GAL 核心術語呈現表](#gal-核心術語呈現表)：與 `docs/naming.md` 的 Part B、Part C、Part D 逐條對應，必須維持完整覆蓋。新增或刪除任何一列之前，都要先確認 `docs/naming.md` 有對應的登錄。
2. [一般技術用語：臺灣用語對照表](#一般技術用語臺灣用語對照表)：兩岸寫法不同的一般技術詞彙。這類詞不是 GAL 專屬概念，`docs/naming.md` 不管轄它們。
3. [譯文一致性補充詞](#譯文一致性補充詞)：`docs/naming.md` 尚未登錄、兩岸寫法也沒有明顯差異，但譯文裡出現過多種寫法的詞。這張表只固定寫法，不賦予任何語意權威。

## 排序規則：一律依英文字母排列

**本檔所有表格的列，一律依英文詞條的字母順序排列。** 排序時不分大小寫，並忽略反引號與開頭的符號，例如 `_galProjection` 排在字母 g、`(file) projection backend` 排在字母 f。詞條含多個字時逐字比較，例如 `source plan` 排在 `source root` 之前。

新增詞條時插進正確的字母位置，不要直接附加在表尾。這條規則有兩個作用：查表的人可以直接用字母定位，不必從頭讀到尾，而重複登錄的詞會因為相鄰而立刻現形。核心術語表不依 `docs/naming.md` 的原始列序排列，完整性改由「來源」欄核對，兩者不衝突。

## 用字紅線：只用臺灣用語

**所有 zh-Hant 文件一律使用臺灣慣用的資訊技術用語，不得使用中國大陸用語。** 此規則沒有例外，也不因為某個大陸用詞「看得懂」就放行。用語混雜會讓讀者懷疑譯文的來源與可信度。判斷有疑義時，以本檔的 [一般技術用語：臺灣用語對照表](#一般技術用語臺灣用語對照表) 為準。對照表沒收錄的詞，比照臺灣一般出版品與官方文件的習慣處理，不得照搬大陸譯法。

字形上全文使用正體中文，不得混入任何簡化字。因為這條規則已經涵蓋所有簡化字，下方對照表的「大陸用語」欄一律以正體字書寫，例如寫「函數」而不是「函数」。原因是譯文裡真正會出現的錯誤，是被轉成正體字的大陸**詞彙**，而不是簡化字形，用正體字列出才能被搜尋與檢查工具抓到。

機械檢查可用 [zhtw-mcp](https://github.com/sysprog21/zhtw-mcp)，它依教育部《重訂標點符號手冊》《國字標準字體》與 OpenCC 的兩岸詞彙資料判讀。工具的判讀結果不會自動覆蓋本檔，兩者衝突時要逐條核對後再決定，因為工具不理解表格語意，會把本檔「禁止寫法」欄裡刻意列出的大陸用語當成違規。

## 標點：不使用分號

**本儲存庫的所有文件都不使用分號，此規則不分語言，全形與半形分號都在內。** 完整規則定義於 [`docs/devguide.md` 標點符號規範](../../devguide.md#punctuation-no-semicolons)。中文寫作的處理方式如下：會想用分號，代表一個句子裡塞了兩個完整的意思，必須改寫。兩件事各自獨立就斷成兩句，兩件事有主從關係就改用逗號並補上連接詞，把關係講明白。用分號串長句只是把推敲邏輯的工作丟給讀者，這也是譯文最常見的可讀性問題。

括號與冒號依內容決定形式。括號內是純英文或程式碼字面值時用半形，例如「執行環境 (runtime)」與「（`plugins/gal-core/mcp.json`）」中的前者。括號內是中文說明或中英混合的子句時用全形，例如「約束性行為（五態 preflight、structural-retrieval 路由）」。冒號銜接中文子句時用全形，兩側都是英文時用半形。

## 五種呈現模式

`docs/naming.md` 的「Semantic Authority vs. Locale Presentation Profiles」一節定義了一組固定的呈現模式。以下說明 zh-Hant 對每種模式的實際判斷方式：

- **keep-en（保留原文）**：直接使用英文原詞，完全不翻譯。所有機器字面值都屬於這類，包含指令名（`gal init`、`/gal pipeline`）、設定鍵（`executorRouting`、`sshTarget`）、檔案路徑（`.dev/plans/`）、程式碼識別字、角色檔名（`golem-architect`）以及產品名（GAL、Claude Code、Codex）。翻譯這類詞會讓讀者對不上實際要輸入的內容，所以永遠保留原文。
- **bilingual-first-use（首次雙語）**：這是 GAL 核心概念的預設模式。同一份文件裡第一次出現時寫成「中文詞 (English)」，之後單用中文詞或英文原詞都可以。**同一份文件內不得出現該詞的第二種中文寫法**。[GAL 核心術語呈現表](#gal-核心術語呈現表) 逐條固定了每個詞的中文寫法。
- **localized（在地化翻譯）**：一般性、非 GAL 專屬的技術詞彙。直接用臺灣慣用譯法，不附註英文。對照表列於下方。
- **transliterated（音譯）**：依讀音而非字義轉寫。**GAL 目前沒有任何術語採用此模式。** GAL 的專屬名詞多半是產品名或程式碼識別字，一律走 keep-en。此處只記錄這個模式目前是空的，不硬造範例。
- **contextual（依語境）**：少數詞的呈現方式取決於上下文。判斷原則是：當句子在討論這個術語本身，例如說明命名規則或列舉角色名稱時，優先用 keep-en。當它只是敘述句裡順帶提到的概念時，依 bilingual-first-use 或 localized 處理。

## GAL 核心術語呈現表

本表與 `docs/naming.md` 的 Part B、Part C、Part D 維持逐條對應，「來源」欄標出該詞登錄在哪一個 Part，方便核對是否漏列。每個詞只有一種合法的中文寫法。「禁止寫法」欄列出實際發生過或容易誤用的寫法。

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

MCP 這組詞全部走 keep-en，原因是 `host`、`client`、`server` 在 MCP 規格裡各有嚴格定義的角色。若譯成「主機／用戶端／伺服器」，讀者容易搞錯方向，因為代理程式才是 host，server 是提供工具的那一方。保留原文可以直接對應 `docs/naming.md` Part B 的定義。

`docs/naming.md` 的退役術語清單不重複列入本檔，包含它記載的那個已被 `MCP host` 取代的舊詞。退役清單由命名關卡直接讀取 `docs/naming.md`，本檔重複列出只會多一份會過期的副本。

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
| `_galProjection` | Part C | keep-en | 一律保留原文，不加中文 | 任何中譯 |
| `generated runtime adapter` | Part C | bilingual-first-use | 轉接器 (adapter)，需要完整指稱時寫「產生的執行環境轉接器」 | 適配器、介面卡 |
| `golem agent` | Part C | keep-en + 補述 | `golem` 保留原文，需要補述時寫「golem 代理程式」 | 哥倫代理、魔像代理 |
| `local-first` | Part C | bilingual-first-use | 本機優先 (local-first) | 本地優先 |
| `local-notes` | Part C | bilingual-first-use | 本機筆記 (local-notes) | 本地筆記 |
| `per-runtime MCP server config` | Part C | bilingual-first-use | 個別執行環境的 MCP server 設定 (per-runtime MCP server config) | 各運行時 MCP 服務器配置、MCP 伺服器設定 |
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

- `state` 的中文寫法兩岸相同，列在此處只是為了讓查表的人確認「狀態」就是正解，不需要另尋譯法。
- `pipeline` 有兩個層次。單獨出現時指 GAL 的實作管道，寫「管道 (pipeline)」。出現在 `config.json#executorRouting.pipeline` 裡時是設定鍵，保留原文不翻譯。行文若同時談到兩者，先講清楚哪一個。

### Part D — 成員登錄表

| 英文術語 | 來源 | 呈現模式 | zh-Hant 固定寫法 | 禁止寫法 |
| --- | --- | --- | --- | --- |
| `generated adapter file` | Part D | localized | 產生的轉接器檔案 | 生成適配器文件 |
| `per-runtime MCP config` | Part D（`per-runtime MCP server config` 的簡稱） | bilingual-first-use | 個別執行環境的 MCP server 設定 | 各運行時 MCP 配置 |
| `runtime key`（含六個代碼與產品名） | Part D | localized + keep-en | 概念本身寫「執行環境代碼」。代碼與產品名保留原文：`copilot`、`antigravity`（`agy`）、`gemini`、`codex`、`opencode`、`claude`，以及 GitHub Copilot、Google Antigravity、Gemini CLI、OpenAI Codex、opencode、Claude Code | 運行時鍵、runtime 鍵值、產品名的任何中譯 |
| `VALID_RUNTIMES` | Part D | keep-en | 一律保留原文，不加中文 | 任何中譯 |
| `vendor`（含供應商名稱） | Part D | localized + keep-en | 概念本身寫「供應商」。供應商名稱保留原文：GitHub、Microsoft、Google、OpenAI、Anthropic。`open source (SST)` 寫「開放原始碼專案（SST）」 | 提供商、供貨商、廠商、供應商名稱的任何中譯 |

「禁止寫法」欄混合了兩類詞：一類是大陸用語，另一類雖然是臺灣用語，但本專案不採用它當作這個英文術語的寫法。例如 `vendor` 的「廠商」屬於後者，它是道地的臺灣用語，只是本專案統一寫「供應商」。兩類都同樣不得使用。欄位標成「—」表示這個詞沒有已知的誤用寫法。

## 一般技術用語：臺灣用語對照表

這些詞不是 GAL 專屬術語，`docs/naming.md` 不管轄它們，一律採用 localized 模式直接寫中文。左欄是唯一合法寫法，右欄是**不得使用**的大陸用語，一律以正體字書寫。

**這是概念層級的禁用，不是全域禁字。** 右欄有些詞在臺灣另有正當用法，例如「文件」指 document、「目錄」指 directory、「對象」指日常語意的對象。禁用只在該詞被拿來表達左欄那個英文概念時成立。

兩岸寫法相同、可以直接使用的常見詞：編譯、重構、部署、觸發、目錄、索引、封裝、序列化。這類詞不列入下表。

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

**用語一致性提醒**：現行譯文曾同時出現「相依」與「依賴」兩種寫法。一律採用「相依」、「相依性」、「相依套件」，不得使用「依賴」。

**`render` 為什麼是「算繪」**：依中華民國教育部辭典，「渲染」本義是國畫的水墨暈染技法，也引申為文字上的誇大，語意與 rendering（計算後繪製）相反。臺灣的標準譯法是「算繪」。本檔先前把兩者寫反，已更正。

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

「文件」一詞在臺灣專指 document，也就是文書或文件檔，不是 file。行文提到 file 一律寫「檔案」，提到 document 才寫「文件」，兩者不得互換。

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

以下三個詞容易配錯概念，各自的正確寫法如下。`gate` 寫「關卡」，不可寫「閘道」，因為閘道是 gateway 的固定寫法。`slug` 寫「代稱」，指用在網址或檔名的短字串，不可寫「識別碼」，那是 identifier 的固定譯法，兩者是不同的概念。`axis` 寫「軸向」，不可寫「維度」，因為維度是 dimension 的固定寫法。

## 譯文一致性補充詞

這些詞在 `docs/naming.md` 沒有登錄，兩岸寫法也沒有明顯差異，但譯文裡實際出現過多種寫法。本表只固定寫法，**不賦予任何語意權威**。若其中任何一個變成 GAL 的載重概念，必須先在 `docs/naming.md` 登錄，再移進 [GAL 核心術語呈現表](#gal-核心術語呈現表)。

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

其中 `fail-closed`、`finalize`／`land`、`orchestrated-only`、`loud failure`、`handoff notes` 已經出現在 GAL 的約束性合約文字裡，性質上比其餘幾個更接近核心術語。它們是 `docs/naming.md` 的候選登錄項目，登錄之後應該移進核心表。

## 與 naming.md 的關係

本檔只提供呈現指引。術語的意義解釋、命名規則（Part A）、術語登錄表（Part B/C）、成員清單（Part D）與退役術語清單，一律以英文版 [`docs/naming.md`](../../naming.md) 為準，它永遠是語意權威。

維護方式如下。[GAL 核心術語呈現表](#gal-核心術語呈現表) 必須與 `docs/naming.md` 的 Part B/C/D 逐條對應，`docs/naming.md` 新增一個詞條，本檔就要補一列，反過來也一樣。刪除或改寫核心表的任何一列之前，先確認 `docs/naming.md` 有對應的登錄。另外兩張表涵蓋 `docs/naming.md` 不管轄的一般詞彙，可以獨立增修，但不得用來定義或改寫任何 GAL 核心概念的意義。
