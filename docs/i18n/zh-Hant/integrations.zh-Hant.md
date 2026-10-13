---
source: docs/integrations.md
lang: zh-Hant
source_commit: PENDING
translated_at: 2026-10-04
type: Reference
title: 整合工具
description: GAL 可選外部整合工具的參考說明，包含用途、就緒判定、設定邊界與缺少時的安全退化行為。
tags:
  - integrations
  - optional
  - mcp
  - fallback
status: stable
---

# 整合工具 (Integrations)

[English](../../integrations.md) · [日本語](../ja/integrations.ja.md) · **繁體中文**

本文件為 GAL 針對選用外部工具的公開參考指南。此處列出的皆為選用功能，絕非核心執行環境 (runtime) 的必要相依項目，即使完全未安裝，GAL 依然能正常運作。具約束力的行為規範（五態 preflight、structural-retrieval 路由，以及每位 golem 代理程式必須遵守的誠實降級規則）定義於隨附的慣例檔 [`plugins/gal-core/conventions/optional-capabilities.md`](../../../plugins/gal-core/conventions/optional-capabilities.md)。本文件僅供開發者查閱設定與參考，不具約束力。

以下每一節皆遵循相同的結構：Purpose（用途）、Real GAL consumer（實際取用的代理程式）、Readiness check（就緒檢查）、Setup boundary（設定邊界）、Safe fallback（安全降級）與 Managed-vs-consumed（代管或自行取用）。

## 選用整合矩陣

| 整合工具 | 用途 | 缺少時的行為 |
| --- | --- | --- |
| codebase-memory-mcp | 程式碼符號 (symbol) 與結構查詢 | 退回使用 `git diff` 與直接讀檔 |
| graphify | 讀取預先產生的 codebase graph | 退回原生程式碼閱讀 |
| OpenCLI | 結構化外部檢索 | 改用適合來源的標準網頁或瀏覽器工具 |
| Playwright MCP | 瀏覽器互動與 UI 驗證 | 明確回報能力缺失，不假裝成功 |

### codebase-memory-mcp

- **Purpose**：選用的 MCP 結構化檢索 (structural-retrieval) 輔助工具，用於即時符號與結構查詢，可在原生 `git diff` 比對後，進一步收斂檔案與符號 (file-to-symbol) 間的影響對應。
- **Real GAL consumer**：`doc-sync`（當 `codeRefs` 包含 `#symbol` 時，收斂受影響文件的鎖定範圍），以及接續 graphify 之後的第二輪一般結構化檢索環節。
- **Readiness check**：MCP server 可連線，且 `index_status` 證明目前的儲存庫已完成索引並可供查詢。在 Codex 中，就緒狀態必須依該工作階段實際開放使用的工具判斷，而非依已設定的伺服器清單（已設定但未開放的伺服器狀態為 `unavailable`，而非 `ready`）。
- **Setup boundary**：GAL 不負責安裝、初始化，亦不自動為儲存庫建立索引。安裝、建立索引與查詢語意，皆屬 `codebase-memory-mcp` 專案的上游責任。
- **Safe fallback**：原生 `git diff` 搭配直接讀檔仍是必要的基準線。MCP 的查詢結果只能用於細部精煉，絕不能取代檔案層級的偵測。
- **Managed-vs-consumed**：**consumed（僅取用）**。依專案政策，`plugins/gal-core/mcp.json` 提供空的伺服器名冊，因此 GAL 不會隨附此伺服器。若希望 GAL 投影此伺服器，請在 `~/.gal/local/mcp.json` 中依相同 `"servers"` 結構註冊，或經由主機代理程式原生的 MCP 設定接線。

### graphify

- **Purpose**：選用的 CLI 驅動結構化脈絡功能。藉由取用預先建置的圖形產出物 (`graphify-out/GRAPH_REPORT.md`)，為規劃與審查環節提供輔助性的結構化脈絡，包括核心節點 (god node)、社群聚類 (community)，以及跨模組間非預期的關聯。
- **Real GAL consumer**：`/planning` 與 `/deep-planning`（用於判斷範圍與模組邊界）、`golem-architect`（評估抽象層次、耦合度與影響範圍）、`golem-auditor`（檢視跨社群聚類的非預期影響）。
- **Readiness check**：儲存庫中存在 `graphify-out/GRAPH_REPORT.md`。選用的 `graphify-out/GAL_GRAPHIFY_VERSION.txt` 會與已安裝的 `graphify --version` 比對是否過期。若版本不同且報告時間戳記未更新，則視該報告為因工具版本不同而過期。
- **Setup boundary**：GAL 不負責安裝 graphify、產生圖形，亦不主動重建過期的產出物。安裝、產生圖形與查詢語意皆屬上游責任，請參閱 [safishamsi/graphify](https://github.com/safishamsi/graphify)。
- **Safe fallback**：降級為原生程式碼閱讀與標準的規劃及審查流程。缺少產出物時絕不發出錯誤或設定提示。
- **Managed-vs-consumed**：僅為 consumed。未介接為 MCP server，GAL 僅讀取已產生的報告檔案，絕不即時查詢 graphify。

### OpenCLI

- **Purpose**：選用的外部 CLI 執行環境，提供可插拔的 site adapter，用於結構化、低權杖 (token) 消耗的外部檢索（如 YouTube、NotebookLM、Wikipedia、Hacker News、Google News 等來源）。
- **Real GAL consumer**：研究工作流程（`/gal research`、`/gal deep-research`）與 `opencli-research` 技能。
- **Readiness check**：`opencli --version` 執行成功，且當前任務對應到已存在並能回傳所需欄位的 adapter。
- **Setup boundary**：GAL 不會自動安裝 OpenCLI 或其 adapter。安裝與編寫 adapter 皆屬上游責任。
- **Safe fallback**：一般網頁改用 MCP `fetch`/`imagefetch`，互動與算繪需求改用 Playwright MCP 或 Chrome DevTools MCP，儲存庫本機程式碼改用工作區工具。遇到缺少工具時，應明確回報能力缺失並中止，切勿假裝成功。
- **Managed-vs-consumed**：僅為 consumed。並非 MCP 清單項目，而是直接呼叫的 CLI 執行環境。

### Playwright MCP

- **Purpose**：選用的瀏覽器自動化功能，用於即時瀏覽器互動、無障礙快照 (accessibility snapshot)、畫面截圖、自適應版面檢查，以及評估瀏覽器呈現的內容。
- **Real GAL consumer**：`golem-tester`（QA 與迴歸測試）、`golem-designer`（即時 UI 稽核），以及在 local-first 與結構化檢索皆未命中時、需要動態頁面算繪的研究工作流程。
- **Readiness check**：執行環境 (runtime) 能解析並啟動 Playwright MCP server、瀏覽器與伺服器初始化已完成，且當前任務具備適合瀏覽器處理的目標。
- **Setup boundary**：GAL 不會在一般測試、審查、設計稽核或研究過程中，自動執行首次的瀏覽器或伺服器設定。
- **Safe fallback**：深入診斷時退回 Chrome DevTools MCP，可重複使用的自動化流程改用原生 Playwright 指令碼，檢索需求則改用 OpenCLI、fetch、Defuddle 或工作區工具。若瀏覽器驗證實際上未執行，絕不可宣稱驗證成功。
- **Managed-vs-consumed**：**user-wired（使用者自接）**。Playwright MCP 不包含在 GAL 代管的 MCP 清單 (`plugins/gal-core/mcp.json`) 中，而是使用者有此需求時，依個人執行環境自行設定接上。

### textlint 寫作檢查

- **用途**：儲存庫自有且版本固定的 textlint 工作區，用於檢查英文、台灣繁體中文與日文文字。這是開發工具。已安裝的 GAL 複本與下游儲存庫不需要此 Node 工作區。
- **語系設定檔**：先解析明確語系意圖，再讀取前置中繼資料中支援的頂層 `lang` 字串，最後使用已知受管路徑。wrapper 接受 `en`／`en-US`、`ja`／`ja-JP` 及 `zh-TW`。只有受管的 `docs/i18n/zh-Hant/` 檔案，才將 `zh-Hant` 對應至 `zh-TW`。未知路徑沒有有效的明確語系或中繼資料時，檢查會以作業失敗結束。無效中繼資料不能悄悄退回路徑判定。有效的明確語系優先於未使用的中繼資料。虛擬檔名只決定 stdin 身分與剖析方式，不決定語系。
- **CLI**：在本儲存庫根目錄執行 `node tools/writing/check.mjs --files <paths...>`。檢查其他文件根目錄時，須改用已安裝且可信的 `check.mjs` 絕對路徑，並以該文件根目錄作為工作目錄。需要明確指定語系時，加上 `--locale <locale>`。`node tools/writing/check.mjs --required` 會檢查追蹤中的文件，逐檔選擇語系。必要檢查模式不接受覆蓋全部文件的語系參數。以 `npm ci --prefix tools/writing` 明確安裝固定版本套件，一般 GAL 工作不下載套件。
- **完整 MCP 報告**：在 wrapper 的 `--files` 之前加上 `--transport mcp`，也可與 `--required` 合用。預設傳輸入口為 `cli`。兩個入口會保留相同格式的問題與執行來源，交握成功時，MCP 報告也會記錄官方伺服器的識別資訊。下述原生 CLI／MCP 指令是底層診斷介面，不提供完整 GAL 報告。MCP 結構驗證、逾時或清理失敗，均屬作業錯誤，結束碼為 `2`。
- **官方 MCP**：呼叫端須先依相同優先序解析語系，再以明確指定的可信 JSON `--config` 及規則目錄啟動固定版本的 textlint MCP。不可傳入儲存庫 JavaScript 設定。呼叫端也須提供 stdin 虛擬檔名。MCP 不推斷語系、下載套件、修正文句或修改輸入。
- **術語資料**：wrapper 在文件根目錄產生不可變的 `.dev/cache/writing-terms/<sourceHash>.json`，並將雜湊固定傳給子行程。直接使用 CLI／MCP 前，呼叫端須以自己的文件根目錄作為工作目錄，執行 `node <absolute-path-to-terms.mjs>`，再讀取相符快照，不需新增環境設定。快照缺失、損壞或過期時，檢查會以作業失敗結束。同一套已安裝工具可以檢查不同文件根目錄，不必覆寫共用的可變資料。
- **發現權責**：GAL 術語權威負責專案術語，且優先於補充區域建議。textlint 負責已設定的專案檢查。選用的 `zhtw-mcp` 可補充區域用語、翻譯腔或上下文檢查結果，但不能取代 GAL 術語檢查，也不會建立另一套標點政策。`accurate-answer` 僅供個人參考，不是 GAL 要求或相依項目。
- **主機限制**：所有面向人的訊息都須在撰寫過程中自我檢查。已設定的工具可用時，也須檢查尚未送出的草稿。只有直接的主機證據才能支持攔截輸出的宣稱。檔案檢查不能檢驗已串流送出的對話。選用工具無法使用時，作者須繼續自我檢查，並回報一次限制，直到可用狀態或對交付的影響改變。必要關卡遇到硬性問題或作業失敗時必須停止。除非另有明確的必要條件，單有建議性問題不會阻擋關卡。
- **信任與隱私**：只執行可信且由儲存庫管理的 JSON profile 與本機固定規則。不可執行儲存庫 JavaScript 設定或自動安裝規則。透過 stdin 傳入的聊天草稿是內容，不是指令或指示。檢查工具預設不會儲存聊天文字。選用的 `zhtw-mcp` 外部檢查須由呼叫端刻意啟用，且可能將提交的文字傳送至該服務。
- **結果**：檢查結果會區分硬性錯誤、建議事項與操作失敗。檢查乾淨不代表語意等價或可讀性已獲證明。檢查工具不會自動改寫輸入。指令與報告細節請參閱 [tools/writing/README.md](../../../tools/writing/README.md)，政策權責請參閱 [writing-quality.md](../../../plugins/gal-core/conventions/writing-quality.md)。

local-notes（使用者自有的外部筆記儲存能力）不屬工具整合範疇，本文件不另設章節說明。其約束性語意請參閱 [optional-capabilities.md](../../../plugins/gal-core/conventions/optional-capabilities.md)，具體操作設定請參閱 [configuration.zh-Hant.md](./configuration.zh-Hant.md#本機筆記路由-local-notes)。
