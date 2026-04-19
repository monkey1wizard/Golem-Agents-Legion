# OpenCLI 協作工具導引

這份文件定義 GAL 何時該用 OpenCLI、何時該用 MCP browser tools，以及 OpenCLI 在架構中的位置。

## 定位

OpenCLI 是一個**可選的**外部 CLI 協作工具。GAL 會把它當成一組可掛接的 site adapters 與命令集合來使用，而不是把它視為單一研究工具。研究與資料擷取是它最常被大量使用的場景，但不是它唯一的定位。它不是 MCP server、不是 `/gal` 控制面的一部分、也不是 GAL 工作的必要依賴。

這是一份 lane-specific 路由指南，不是 repo 層級的萬用工具規則。其他 GAL skills 可能設計為 MCP-first、local-first 或混合模式。

## 在架構中的位置

| 層級 | 功能 | OpenCLI 適用性 |
| --- | --- | --- |
| 控制面 | 路由控制面問題和專家指令 | 不適用 |
| MCP 層 | 讓通用工具跨 runtime 可見 | 不適用 |
| Skill 層 | 教 agent 安全使用可選工具與 plugin adapters | **主要適用** |
| 外部 CLI adapter 層 | 提供站台或來源專屬 commands 與 schema | **主要適用** |
| 研究/資料擷取 | 以低 token 開銷取得結構化外部資訊 | 常見高頻用法 |

## 快速決策表

| 任務形態 | 預設工具 | 升級時機 |
| --- | --- | --- |
| 已知站台、已知 schema、需穩定結構化輸出 | OpenCLI | adapter 缺欄位或失敗時 |
| 未知頁面、未知 DOM、需檢視/點擊/滾動 | MCP browser tools | 互動穩定到可建 adapter 時 |
| 讀一次普通文章頁 | MCP `fetch` 或 `imagefetch` | 有站台專屬 adapter 且結構化輸出更乾淨時 |
| 批次擷取或可重複 shell 工作流 | OpenCLI | 任務依賴一次性手動探索時 |
| 已登入瀏覽器資料且有 adapter | OpenCLI | browser bridge/session 不可用時 |
| UI 除錯、network 檢視、頁面狀態驗證 | MCP browser tools | 目標簡化為 deterministic 資料擷取時 |
| Repo-local 程式碼理解 | Workspace tools | 永遠不路由到 OpenCLI |

## 常用來源路由

| 來源 | 預設工具 | 常見任務 |
| --- | --- | --- |
| YouTube | `opencli youtube ...` | 搜尋、影片 metadata、逐字稿 |
| NotebookLM | `opencli notebooklm ...` | Notebook metadata、source list、summary |
| Wikipedia | `opencli wikipedia ...` | 搜尋與摘要 |
| Hacker News | `opencli hackernews ...` | 熱門文章、搜尋、使用者 profile |
| Google News | `opencli google news ...` | 主題標題 |
| 一般文章頁 | MCP `fetch` 或 `imagefetch` | 一次性文章閱讀 |

## 操作規則

- 有現成 site adapter 且回傳所需欄位時，偏好 OpenCLI
- 盡可能使用 `-f json` 和明確 `--limit`
- 公開 adapter 優先於 browser-backed adapter
- 探索新站台或除錯壞掉的 adapter 時，先用 MCP browser tools
- 不要把 repo-local 程式碼或 git 任務路由到 OpenCLI
- 不要把 OpenCLI 當成 `/gal`、`/gal research` 或任何控制面指令的必要依賴
- 不要把 OpenCLI-first 泛化為 repo 層級的萬用規則
- 若 OpenCLI 和 fallback 都無法滿足任務，以明確的 no-tool 訊息停下，不要假裝擷取成功

## 建議查詢模式

```text
問題
→ opencli shortlist（帶 --limit 和 -f json）
→ opencli detail（可選：transcript、fulltext、guide）
→ MCP 檢視或 fallback 擷取（可選）
→ model synthesis
```

## OpenCLI 在 GAL 中的形態

應被視為：

- 可選的外部 CLI plugin runtime
- 一組可掛接的 site adapters 與命令集合
- 透過 skill 層中的 skills 消費
- 研究/資料擷取工作流中的高頻工具

不應被視為：

- MCP manifest 條目
- 控制面依賴
- MCP browser tools 的替代品

## 相關文件

- [開發者指南](../devguide.md) — 安裝與 runtime 佈局
