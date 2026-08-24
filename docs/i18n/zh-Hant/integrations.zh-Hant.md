---
source: docs/integrations.md
lang: zh-Hant
source_commit: c1c29b8634be5e67b648640f8e6c6af13fbab0a5
translated_at: 2026-07-23
status: current
---

# 整合工具 (Integrations)

[English](../../integrations.md) · [日本語](../ja/integrations.ja.md) · **繁體中文**

GAL 針對可選外部工具的公開參考文件。每一項都是可選能力，絕非核心執行環境 (runtime) 的相依套件——即使一個都沒安裝，GAL 仍能正常運作。約束性行為（五態 preflight、structural-retrieval 路由，以及每個 golem 代理程式都必須遵守的誠實降級規則）定義於隨附的慣例 (convention) 檔：[`plugins/gal-core/conventions/optional-capabilities.md`](../../../plugins/gal-core/conventions/optional-capabilities.md)。本檔僅供人類閱讀的設定與參考用途——僅供參考，不具約束力。

以下每一節都遵循同一套結構：purpose、real GAL consumer、readiness check、setup boundary、safe fallback、managed-vs-consumed。

## codebase-memory-mcp

- **Purpose** —— 可選的、以 MCP 為基礎的 structural-retrieval 輔助工具，用於即時 symbol 與結構查詢，在原生 `git diff` 之後精煉 file-to-symbol 的影響對應。
- **Real GAL consumer** —— `doc-sync`（當 `codeRefs` 包含 `#symbol` 時，收斂受影響文件的鎖定範圍）、以及作為 graphify 之後第二輪的一般 structural-retrieval 環節。
- **Readiness check** —— MCP server 可連線，且 `index_status` 證明目前的儲存庫已完成索引且可查詢。特別是在 Codex 中，readiness 必須依照該工作階段中實際**曝露**出來的工具來判斷，而非依已設定的 server 清單——已設定但未曝露的 server 狀態是 `unavailable`，而非 `ready`。
- **Setup boundary** —— GAL 不負責安裝、初始化，或自動為儲存庫建立索引。安裝、索引與查詢語意皆屬 `codebase-memory-mcp` 專案的上游責任。
- **Safe fallback** —— 原生 `git diff` 加上直接讀檔仍是必要的基準線。MCP 的結果只能精煉，絕不能取代 file-level 偵測。
- **Managed-vs-consumed** —— **managed**。列在 GAL 的 managed MCP manifest 中(`plugins/gal-core/mcp.json`、`DeusData/codebase-memory-mcp`)。

## graphify

- **Purpose** —— 可選的 CLI 驅動 structural-context 能力。取用預先建置好的 graph 產出物(`graphify-out/GRAPH_REPORT.md`)，為規劃與審查環節提供輔助性的 structural context:god node、community，以及跨模組間出乎意料的連結。
- **Real GAL consumer** —— `/planning` 與 `/deep-planning`（範圍與模組邊界判斷）、`golem-architect`（抽象層次、耦合度、影響範圍）、`golem-auditor`（未預期的跨 community 影響）。
- **Readiness check** —— 儲存庫中存在 `graphify-out/GRAPH_REPORT.md`。可選的 `graphify-out/GAL_GRAPHIFY_VERSION.txt` 會與已安裝的 `graphify --version` 比對是否過期。若版本不同且報告未新於該版本戳記，則視該報告為「因工具版本而過期」。
- **Setup boundary** —— GAL 不負責安裝 graphify、產生 graph，也不重建過期的產出物。安裝、graph 產生與查詢語意皆屬上游責任：[safishamsi/graphify](https://github.com/safishamsi/graphify)。
- **Safe fallback** —— 降級為原生程式碼閱讀與標準的規劃/審查行為。缺少產出物時絕不產生錯誤或設定提示。
- **Managed-vs-consumed** —— 僅為 consumed。並未被接為 MCP server，GAL 只讀取已產生的報告檔案，從不即時查詢 graphify。

## OpenCLI

- **Purpose** —— 可選的外部 CLI 執行環境，提供可插拔的 site adapter，用於結構化、低權杖 (token) 消耗的外部檢索（YouTube、NotebookLM、Wikipedia、Hacker News、Google News 及類似來源）。
- **Real GAL consumer** —— 研究工作流程(`/gal research`、`/gal deep-research`)、`opencli-research` 技能。
- **Readiness check** —— `opencli --version` 執行成功，而且目前任務對應到一個既有、能回傳所需欄位的 adapter。
- **Setup boundary** —— GAL 不會自動安裝 OpenCLI 或其 adapter。安裝與 adapter 撰寫皆屬上游責任。
- **Safe fallback** —— 一般網頁改用 MCP `fetch`/`imagefetch`，互動/render 需求改用 Playwright MCP 或 Chrome DevTools MCP，儲存庫本機程式碼改用 workspace 工具。應明確以「能力缺失」訊息中止，而非假裝成功。
- **Managed-vs-consumed** —— 僅為 consumed。不是 MCP manifest 項目，而是直接呼叫的 CLI 執行環境。

## Playwright MCP

- **Purpose** —— 可選的瀏覽器自動化能力，用於即時瀏覽器互動、accessibility snapshot、截圖、響應式版面檢查，以及瀏覽器可見內容的評估。
- **Real GAL consumer** —— `golem-tester`（QA 與迴歸測試）、`golem-designer`（即時 UI 稽核）、以及在 local-first 與 structured-retrieval 檢查都失敗後、需要動態頁面 render 的研究工作流程。
- **Readiness check** —— 執行環境 (runtime) 能解析並啟動 Playwright MCP server、瀏覽器/server 初始化已完成，且目前任務有適合瀏覽器處理的目標。
- **Setup boundary** —— GAL 不會在一般測試、審查、設計稽核或研究過程中自動執行首次瀏覽器/server 設定。
- **Safe fallback** —— 深入診斷時退回 Chrome DevTools MCP，可重複使用的自動化改用原生 Playwright 指令碼，或以 OpenCLI/fetch/Defuddle/workspace 工具做檢索。若瀏覽器驗證其實沒有執行，絕不宣稱它已成功。
- **Managed-vs-consumed** —— **user-wired**。Playwright MCP 不在 GAL 的 managed MCP manifest 中(`plugins/gal-core/mcp.json`)，而是使用者想要此能力時，依自己的執行環境設定自行接上的。

local-notes（使用者自有的外部筆記儲存能力）不是工具整合，本檔中沒有對應章節——其約束性語意定義於 [optional-capabilities.md](../../../plugins/gal-core/conventions/optional-capabilities.md)，其操作設定則定義於 [使用手冊](manual.zh-Hant.md)。
