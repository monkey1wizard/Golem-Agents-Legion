# Golem Agents Legion (GAL)

[English](README.md) | 繁體中文

GAL 是一套以 Markdown 為核心的 AI 工作系統，分為兩層：

- `/gal` 控制平面：負責 repo 初始化、狀態檢視、下一步推薦、工作階段收尾與研究入口
- gstack 風格的專家指令層：負責規劃、審查、QA、發佈、記憶管理與安全守護

重點不在於保留某個工具的 UX，而是讓方法論、狀態模型與指令合約都掌握在你手中，同時讓 Copilot、Gemini 與 Codex 執行同一套工作流程。

## GAL 是什麼

GAL 把持久的工作流程知識與工具特定的轉接器分開。

- 知識以 Markdown 形式存放在這個 repo 裡：工作流程、代理、慣例、樣板與技能
- Repo 本地的執行狀態存放於 `.dev/` 與 `docs/plans/`
- 工具轉接器是生成出來的輸出物，不是真相來源

這個 repo 不是一個應用服務，而是 canonical 的方法論與指令表面。

## 來源與借鑑

GAL 不是單一上游的改名版，而是把幾個相鄰系統的長處重新組合起來。

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done) 提供的是 phase-based workflow discipline，也就是明確狀態、verification gates 與較嚴格的執行生命週期。
- [gstack](https://github.com/garrytan/gstack) 提供的是 specialist workflow semantics 與大量指令語彙，GAL 取用的是這些工作流語意，並以 GAL-native skills 重新實作，而不是依賴 upstream gstack 的 runtime 或儲存模型。
- [GitHub Spec Kit](https://github.com/github/spec-kit) 提供的是 portable command-kit 與 artifact-driven 的方向：repo 內可攜的 workflow artifacts、Markdown-native 文件，以及可安裝到不同 agent/runtime 的命令表面。

GAL 自己額外做的，是把這些來源收斂成「`/gal` 控制平面 + specialist execution layer」的分層，並把 canonical state 固定在 repo 內的 `.dev/`、`docs/plans/` 與相關 artifact 目錄。

簡單說，GAL 不是 spec-kit 或 gstack 的 fork，它是把 GSD、gstack、Spec Kit 的不同優點重組成一個更適合 Copilot、Gemini 與 Codex 的 repo-local operating model。

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
| `.dev/state.md` | 活動計畫索引、阻塞點、工作階段連續性 |
| `docs/plans/<plan-slug>.md` | 人類可讀的計畫文件（範圍、理由、需求） |
| `docs/plans/<plan-slug>.prompt.md` | AI 執行工作檔案——可變清單、workflow 狀態、執行狀態、回寫目標 |
| `DESIGN.md` | Repo 層級設計治理（設計系統，非計畫專屬） |
| `CLAUDE.md` | Repo 本地操作備注，如部署設定與設計參考 |
| `docs/designs/<plan-slug>/` | 計畫綁定的設計資產：`variant-approved.json`、`variant-approved.png`、`handoff-final.html` |
| `docs/qa-reports/` | QA 報告：`YYYYMMDD-<plan-slug>.md` / `YYYYMMDD-<plan-slug>-report-only.md` |
| `docs/design-reports/` | 設計審查報告：`YYYYMMDD-<plan-slug>-rNN.md` |
| `docs/benchmarks/` | 效能基準線：`YYYYMMDD-HHmmss-<url-slug>.json`，canary 基準線：`canary-YYYYMMDD-HHmmss-<url-slug>.json` |
| `docs/retros/` | 回顧快照：`YYYYMMDD.json` |
| `docs/research/` | 研究筆記：`YYYYMMDD-<plan-slug>-<topic>.md` |
| `.dev/learnings.jsonl` | Repo 本地的制度化記憶 |

### 計畫執行 Sections

執行工作檔（`.prompt.md`）包含三個由專家指令寫入的 sections，各自有明確的所有權規則：

| Section | 寫入者 | 消費者（唯讀） | 用途 |
| --- | --- | --- | --- |
| `## Open Questions` | `/office-hours`（初始化）、`/plan-ceo-review`、`/plan-design-review`（追加），`/plan-eng-review` 關閉已解決項目 | `/ship`、`/gal status`、`/gal whats-next` | 未解決假設與決策的唯一 canonical list，ID 格式 `OQ-NNN` |
| `## Tasks` | `/plan-eng-review`（唯一初始化者，Eng Review CLEAR 後），實作階段只能更新完成狀態 | `/review`、`/qa`、`/ship`、`/gal status`、`/gal whats-next` | 可驗證的任務清單，ID 格式 `T-NNN` |
| `## Analyze` | `/review`（唯一寫入者，verdict：`CLEAR` / `DRIFT-OPEN` / `NOT-RUN`） | `/ship`、`/gal status`、`/gal whats-next` | Drift 檢查：diff 是否偏離計畫範圍？ |

消費者只讀取這些 sections 用於顯示與路由 — 不重算、不覆寫。

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
# Copilot / Gemini CLI（slash-command 介面）
/gal init
/gal status
/office-hours
/autoplan

# Codex CLI（skill mention 介面，使用 $ 前綴，不是 /）
$gal init
$gal status
$office-hours
$autoplan
```

最終模型中沒有公開的 `gal sync` 步驟。轉接器生成屬於安裝層的內部作業，不是使用者工作流程。

### 建議的工作節奏

控制平面的建議用法如下：

```text
/gal init
/office-hours
/autoplan
<實作>
/gal wrap-up
```

下次恢復工作時，先從這裡開始：

```text
/gal status
# 或
/gal whats-next
```

`/gal wrap-up` 應該在結束工作階段前執行；另外在任何你之後可能需要無痛續接的 checkpoint 後，也建議執行一次，例如完成 `T-001`、準備切換上下文、或準備交接給另一個 model / session 之前。

## 控制平面指令

這些是穩定的使用者端 `/gal` 指令。

> **CLI 呼叫差異**：Copilot CLI 與 Gemini CLI 使用 `/gal <subcommand>`。Codex CLI 則使用 `$gal <subcommand>`。在 Codex 中，`/` 前綴保留給 Codex 內建指令，不能用來呼叫自訂技能。

| 指令 | 使用時機 | 讀取 | 寫入 | 結果 |
| --- | --- | --- | --- | --- |
| `/gal init` | 為 repo 初始化 GAL 管理 | 現有 repo 文件與結構 | `.dev/project.md`、`.dev/state.md` | Repo 進入 GAL 管理狀態 |
| `/gal status` | 需要完整狀態投影 | `.dev/state.md`、活動執行計畫檔（`.prompt.md`） | 無 | 回報活動計畫、審查/測試狀態、阻塞點、連續性與準備度 |
| `/gal whats-next` | 想知道單一下一步 | `.dev/state.md`、活動執行計畫的狀態與結果 | 無 | 回傳一個推薦的下一步指令或任務 |
| `/gal wrap-up` | 結束工作階段，或停在一個有意義的 checkpoint | `.dev/state.md`、活動計畫 | `### Handoff Notes`、`## Session Continuity` | 收斂可恢復的上下文 |
| `/gal research` | 需要結構化調查 | 當前 repo 上下文 | 研究成果（依指示） | 進入研究工作流程 |

### 可發現性 Alias

這些 alias 在 Copilot / Gemini 中用於 slash 指令自動補全，在 Codex 中則以同名 skill 出現。

| Alias | Copilot / Gemini | Codex CLI |
| --- | --- | --- |
| gal-init | `/gal-init` | `$gal-init` |
| gal-status | `/gal-status` | `$gal-status` |
| gal-whats-next | `/gal-whats-next` | `$gal-whats-next` |
| gal-wrap-up | `/gal-wrap-up` | `$gal-wrap-up` |

## 專家指令目錄

這些指令直接實作工作層，不需要經過 `/gal` 路由。

### 規劃

| 指令 | 用途 | 主要寫入 |
| --- | --- | --- |
| `/office-hours` | YC 風格的 sprint 或功能啟動，建立新計畫 | 新的 `docs/plans/<plan-slug>.md` + `.prompt.md`、`.dev/state.md`、初始 `## Open Questions` |
| `/plan-ceo-review` | 從創辦人視角審查範圍與野心 | 計畫 `## Review Results`、`## Open Questions`（scope OQs） |
| `/plan-eng-review` | 架構與測試計畫關卡，`/ship` 前的必要條件 | 計畫 `## Review Results`、`## Test Plan`、`## Tasks`，關閉已解決的 `## Open Questions` |
| `/plan-design-review` | 實作前的 UX 與設計審查 | 計畫 `## Review Results`、`## Open Questions`（設計相關 OQs） |
| `/autoplan` | 串接 CEO、設計與工程審查並自動決策 | 計畫審查區段與測試計畫 |
| `/cso` | OWASP 加 STRIDE 資安審查 | 計畫 `## Review Results` |

### 設計

| 指令 | 用途 | 主要寫入 |
| --- | --- | --- |
| `/design-consultation` | 建立產品設計系統 | `DESIGN.md`、`CLAUDE.md` |
| `/design-shotgun` | 生成多個視覺變體並記錄審核結果 | `docs/designs/<plan-slug>/variant-approved.json` |
| `/design-html` | 將已審核的設計轉換為可執行的 HTML 或元件程式碼 | `docs/designs/<plan-slug>/handoff-final.html` |
| `/design-review` | 對照 `DESIGN.md` 對線上站台進行精準視覺修正 | 計畫 `## Review Results`、`docs/design-reports/` |

### 除錯與審查

| 指令 | 用途 | 主要寫入 |
| --- | --- | --- |
| `/investigate` | 根因優先的除錯工作流程 | 計畫 `## Debug Session` |
| `/review` | Staff 級別的 diff 審查，找出 CI 漏掉的問題 | 計畫 `## Review Results`、`## Analyze` |

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
/gal wrap-up
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

### 狀態判定疑難排解

- 如果 `.dev/state.md` 不存在，表示 repo 尚未初始化。
- 如果 `.dev/state.md` 存在，GAL 應該從 `docs/plans/<plan-slug>.prompt.md` 的 `## Status` 讀取活動 workflow。
- 如果 `.dev/state.md` 已存在但 GAL 仍無法投影狀態，應視為 state 結構異常，不是要重新執行 `/gal init`。

## 架構摘要

```text
~/golem-agents-legion/     canonical 方法論與指令來源
<repo>/.dev/              repo 本地狀態與連續性
docs/plans/<slug>.md      人類可讀的計畫文件
docs/plans/<slug>.prompt.md  AI 執行工作檔案（可變狀態）
~/.copilot/skills/        已安裝的 Copilot skills
~/.gemini/skills/         已安裝的 Gemini skills
~/.agents/skills/         已安裝的 Codex skills
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
| [docs/runtime-verification.md](docs/runtime-verification.md) | 指令與執行平面的 live/manual 驗證狀態 |
| [docs/command-dispatch-architecture.md](docs/command-dispatch-architecture.md) | Dispatch 模型與 alias 政策 |
| [commands/commands.md](commands/commands.md) | 已安裝的指令表面與 alias 架構 |
| [workflows/coding.md](workflows/coding.md) | 原始開發流程狀態機參考 |

## GAL 不再視為公開工作流程的部分

- T0/T1/T2 不再是新指令表面的主要使用者詞彙
- 使用 GAL 的專家指令不需要另外安裝 upstream gstack

## 授權

MIT — 詳見 [LICENSE](LICENSE).
