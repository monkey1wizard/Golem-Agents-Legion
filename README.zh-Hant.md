# Golem Agents Legion (GAL)

[English](README.md) | 繁體中文

GAL 是一套以 Markdown 為核心的 AI 工作系統，分為兩層：

- `/gal` 控制平面：負責 repo 初始化、狀態檢視、下一步推薦、工作階段收尾與研究入口
- gstack 風格的專家指令層：負責規劃、審查、QA、發佈、記憶管理與安全守護

重點不在於保留某個工具的 UX，而是讓方法論、狀態模型與指令合約都掌握在你手中，同時讓 Copilot 與 Gemini 執行同一套工作流程。

## GAL 是什麼

GAL 把持久的工作流程知識與工具特定的轉接器分開。

- 知識以 Markdown 形式存放在這個 repo 裡：工作流程、代理、慣例、樣板與技能
- Repo 本地的執行狀態存放於 `.dev/` 與 `docs/plans/`
- 工具轉接器是生成出來的輸出物，不是真相來源

這個 repo 不是一個應用服務，而是 canonical 的方法論與指令表面。

## 最終運作模型

GAL 現在採用控制平面與執行層的嚴格分層。

| 層級 | 職責 | 指令 |
| --- | --- | --- |
| 控制平面 | 初始化 repo、讀取狀態、推薦下一步、收斂連續性、路由研究 | `/gal init`、`/gal status`、`/gal whats-next`、`/gal wrap-up`、`/gal research` |
| 專家執行層 | 規劃、設計、除錯、審查、QA、發佈、記憶管理、安全守護 | `/office-hours`、`/plan-eng-review`、`/review`、`/qa`、`/ship` 及下方完整專家目錄 |

`/gal` 不重複實作專家行為。專家指令會把結果回寫到 `/gal` 讀取的 canonical artifacts。

## Canonical Artifacts

這些是持久的狀態模型檔案。

| 路徑 | 用途 |
| --- | --- |
| `.dev/project.md` | Repo 摘要、技術棧、目標、限制 |
| `.dev/state.md` | 活動計畫、阻塞點、工作階段連續性 |
| `docs/plans/*.prompt.md` | 單一功能或 sprint 的執行記憶 |
| `DESIGN.md` | 產品設計系統 |
| `CLAUDE.md` | Repo 本地操作備注，如部署設定與設計參考 |
| `docs/designs/` | 設計變體、審核結果、定稿 mockup |
| `docs/qa-reports/` | QA 報告 |
| `docs/benchmarks/` | 效能與 canary 基準線 |
| `docs/retros/` | 回顧快照 |
| `.dev/learnings.jsonl` | Repo 本地的制度化記憶 |

## 快速開始

### Windows

```powershell
git clone https://github.com/monkey1wizard/golem-agents-legion.git ~/golem-agents-legion
~/golem-agents-legion/scripts/Setup-Machine.ps1
```

### macOS

```bash
git clone https://github.com/monkey1wizard/golem-agents-legion.git ~/golem-agents-legion
~/golem-agents-legion/scripts/setup-machine.sh
```

然後在目標 repo 內執行：

```text
/gal init
/gal status
/office-hours
/autoplan
```

最終模型中沒有公開的 `gal sync` 步驟。轉接器生成屬於安裝層的內部作業，不是使用者工作流程。

## 控制平面指令

這些是穩定的使用者端 `/gal` 指令。

| 指令 | 使用時機 | 讀取 | 寫入 | 結果 |
| --- | --- | --- | --- | --- |
| `/gal init` | 為 repo 初始化 GAL 管理 | 現有 repo 文件與結構 | `.dev/project.md`、`.dev/state.md` | Repo 進入 GAL 管理狀態 |
| `/gal status` | 需要完整狀態投影 | `.dev/state.md`、活動計畫檔案 | 無 | 回報活動計畫、審查/測試狀態、阻塞點、連續性與準備度 |
| `/gal whats-next` | 想知道單一下一步 | `.dev/state.md`、計畫狀態與結果 | 無 | 回傳一個推薦的下一步指令或任務 |
| `/gal wrap-up` | 結束工作階段 | `.dev/state.md`、活動計畫 | `### Handoff Notes`、`## Session Continuity` | 收斂可恢復的上下文 |
| `/gal research` | 需要結構化調查 | 當前 repo 上下文 | 研究成果（依指示） | 進入研究工作流程 |

### 可發現性 Alias

這些 alias 是為了 slash 指令自動補全而存在，不是主要指令表。

| Alias | 狀態 | 替代指令 |
| --- | --- | --- |
| `/gal-init` | 啟用中 | `/gal init` |
| `/gal-status` | 啟用中 | `/gal status` |
| `/gal-whats-next` | 啟用中 | `/gal whats-next` |
| `/gal-wrap-up` | 啟用中 | `/gal wrap-up` |

## 專家指令目錄

這些指令直接實作工作層，不需要經過 `/gal` 路由。

### 規劃

| 指令 | 用途 | 主要寫入 |
| --- | --- | --- |
| `/office-hours` | YC 風格的 sprint 或功能啟動，建立新計畫 | 新的 `docs/plans/*.prompt.md`、`.dev/state.md` |
| `/plan-ceo-review` | 從創辦人視角審查範圍與野心 | 計畫 `## Review Results` |
| `/plan-eng-review` | 架構與測試計畫關卡，`/ship` 前的必要條件 | 計畫 `## Review Results`、`## Test Plan` |
| `/plan-design-review` | 實作前的 UX 與設計審查 | 計畫 `## Review Results` |
| `/autoplan` | 串接 CEO、設計與工程審查並自動決策 | 計畫審查區段與測試計畫 |
| `/cso` | OWASP 加 STRIDE 資安審查 | 計畫 `## Review Results` |

### 設計

| 指令 | 用途 | 主要寫入 |
| --- | --- | --- |
| `/design-consultation` | 建立產品設計系統 | `DESIGN.md`、`CLAUDE.md` |
| `/design-shotgun` | 生成多個視覺變體並記錄審核結果 | `docs/designs/<slug>/approved.json` |
| `/design-html` | 將已審核的設計轉換為可執行的 HTML 或元件程式碼 | `docs/designs/<slug>/finalized.html` |
| `/design-review` | 對照 `DESIGN.md` 對線上站台進行精準視覺修正 | 計畫 `## Review Results`、`docs/design-reports/` |

### 除錯與審查

| 指令 | 用途 | 主要寫入 |
| --- | --- | --- |
| `/investigate` | 根因優先的除錯工作流程 | 計畫 `## Debug Session` |
| `/review` | Staff 級別的 diff 審查，找出 CI 漏掉的問題 | 計畫 `## Review Results` |

### 瀏覽器與 QA

| 指令 | 用途 | 主要寫入 |
| --- | --- | --- |
| `/browse` | 供其他指令使用的 Playwright 瀏覽器能力原語 | 僅限當前工作階段 |
| `/connect-chrome` | 將瀏覽器工作切換為有頭 Chrome | 僅限當前工作階段 |
| `/setup-browser-cookies` | 將真實瀏覽器認證匯入 Playwright | 僅限當前工作階段 |
| `/qa` | 完整 QA 流程，含修復迴圈與迴歸測試 | 計畫 `## Test Results`、`docs/qa-reports/` |
| `/qa-only` | QA bug 報告，不修改程式碼 | 計畫 `## Test Results (Report Only)`、`docs/qa-reports/` |

### 發佈

| 指令 | 用途 | 主要寫入 |
| --- | --- | --- |
| `/ship` | 合併前的最終關卡：測試、覆蓋率、PR、文件 | 計畫 `## Ship` |
| `/land-and-deploy` | 合併並驗證正式環境部署 | 計畫 `## Deploy` |
| `/canary` | 部署後對正式環境的持續監控 | 基準線、選擇性計畫備注 |
| `/benchmark` | 使用真實瀏覽器測量效能並比對基準 | `docs/benchmarks/`、選擇性計畫 `## Performance` |
| `/setup-deploy` | 一次性部署設定 | `CLAUDE.md` |
| `/document-release` | 更新文件以符合已發佈的程式碼 | Repo 文件、PR 內容 |
| `/retro` | 帶有 repo 指標與快照的回顧報告 | `docs/retros/` |

### 記憶管理與安全守護

| 指令 | 用途 | 主要寫入 |
| --- | --- | --- |
| `/learn` | Repo 本地制度化記憶管理員 | `.dev/learnings.jsonl` |
| `/careful` | 在執行破壞性指令前發出警告 | 僅限當前工作階段 |
| `/freeze` | 將編輯範圍限制在指定目錄邊界內 | 僅限當前工作階段 |
| `/guard` | 同時啟用 `/careful` 與 `/freeze` | 僅限當前工作階段 |
| `/unfreeze` | 移除當前的 freeze 邊界 | 僅限當前工作階段 |
| `/gstack-upgrade` | 拉取最新版 GAL 並重新執行設定 | 機器維護，僅限本機 |

## 典型流程

### 新功能

```text
/gal init
/office-hours
/autoplan
<實作>
/review
/qa
/ship
```

### Bug 調查

```text
/gal status
/investigate
/review
/qa
/gal wrap-up
```

### 正式環境發佈

```text
/ship
/land-and-deploy
/canary
/retro
```

## 狀態邏輯

控制平面能運作，是因為專家指令會把可預測的區段回寫到活動計畫中。

| 區段 | 由誰寫入 | 由誰讀取 |
| --- | --- | --- |
| `## Review Results` | 審查類專家指令 | `/gal status`、`/gal whats-next` |
| `## Test Plan` | `/plan-eng-review` | `/qa`、`/qa-only` |
| `## Test Results` | `/qa`、`/qa-only` | `/gal status`、`/gal whats-next` |
| `## Ship` | `/ship` | `/gal status`、`/gal whats-next`、`/land-and-deploy` |
| `## Deploy` | `/land-and-deploy` | `/gal status`、`/canary` |
| `### Handoff Notes` | `/gal wrap-up` | `/gal status`、`/gal whats-next` |

## 架構摘要

```text
~/golem-agents-legion/     canonical 方法論與指令來源
<repo>/.dev/              repo 本地狀態與連續性
docs/plans/*.prompt.md    活動執行記憶
~/.copilot/skills/        已安裝的 Copilot skills
~/.gemini/skills/         已安裝的 Gemini skills
```

方法論是可攜帶的。轉接器是用完即棄的。

## 個人化設定

clone 後需填入你的環境特定佔位符：

| 佔位符 | 意義 | 檔案 |
| --- | --- | --- |
| `<OBSIDIAN_VAULT>` | Obsidian vault 的絕對路徑 | Obsidian 代理與技能 |
| `<OBSIDIAN_VAULT_NAME>` | Obsidian 顯示的 vault 名稱 | Obsidian 技能 |
| `<LOCAL_SEARCH_PROJECT>` | 本地搜尋專案 clone 路徑 | Local-first 與知識管理技能 |
| `<GAL_SKILLS>` | Skills 安裝路徑 | 部分輔助技能 |
| `<TEMP_DIR>` | 暫存輸出目錄 | PDF 技能 |

模型路由設定請複製 [model-roles.example.md](model-roles.example.md) 為 `model-roles.local.md` 後自訂。

## 重要文件

| 路徑 | 用途 |
| --- | --- |
| [docs/ai-agent-onboarding.md](docs/ai-agent-onboarding.md) | AI 代理與維護者的閱讀順序 |
| [docs/gal-control-plane-contracts.md](docs/gal-control-plane-contracts.md) | `/gal` 讀寫合約的 canonical 定義 |
| [docs/gstack-integration.md](docs/gstack-integration.md) | GAL 為何原生重新實作 gstack 語意 |
| [docs/gstack-command-contracts.md](docs/gstack-command-contracts.md) | 專家技能的實作藍圖 |
| [docs/command-dispatch-architecture.md](docs/command-dispatch-architecture.md) | Dispatch 模型與 alias 政策 |
| [commands/commands.md](commands/commands.md) | 已安裝的指令表面與 alias 架構 |
| [workflows/coding.md](workflows/coding.md) | 原始開發流程狀態機參考 |

## GAL 不再視為公開工作流程的部分

- T0/T1/T2 不再是新指令表面的主要使用者詞彙
- 使用 GAL 的專家指令不需要另外安裝 upstream gstack

## 授權

MIT — 詳見 [LICENSE](LICENSE).
