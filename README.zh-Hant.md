# Golem Agents Legion (GAL)

[English](README.md) | 繁體中文

GAL 是一套以 Markdown 為核心的 AI 工作系統，分為兩層：

- `/gal` 控制平面：負責 repo 初始化、狀態檢視、下一步推薦、工作階段收尾與研究入口
- 專家執行層：包含 GAL-native 規劃指令家族、provider-routed 規劃審查 lanes、QA、發佈、記憶管理與安全守護

重點不在於保留某個工具的 UX，而是讓方法論、狀態模型與指令合約都掌握在你手中，同時讓 Copilot、Gemini 與 Codex 執行同一套工作流程。

## GAL 是什麼

GAL 把持久的工作流程知識與工具特定的轉接器分開。

- 知識以 Markdown 形式存放在這個 repo 裡：工作流程、代理、慣例、樣板與技能
- Repo 本地的執行狀態存放於 `.dev/`，source plan 則存放於 `docs/plans/`
- 工具轉接器是生成出來的輸出物，不是真相來源

這個 repo 不是一個應用服務，而是 canonical 的方法論與指令表面。

## 來源與借鑑

GAL 不是單一上游的改名版，而是把幾個相鄰系統的長處重新組合起來。

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done) 提供的是 phase-based workflow discipline，也就是明確狀態、verification gates 與較嚴格的執行生命週期。
- [gstack](https://github.com/garrytan/gstack) 提供的是 specialist workflow semantics 與大量指令語彙，GAL 取用的是這些工作流語意，並以 GAL-native skills 重新實作，而不是依賴 upstream gstack 的 runtime 或儲存模型。
- [GitHub Spec Kit](https://github.com/github/spec-kit) 提供的是 portable command-kit 與 artifact-driven 的方向：repo 內可攜的 workflow artifacts、Markdown-native 文件，以及可安裝到不同 agent/runtime 的命令表面。

GAL 自己額外做的，是把這些來源收斂成「`/gal` 控制平面 + specialist execution layer」的分層，並把 repo-local 的主要檔案固定在 `.dev/`、`docs/plans/` 與相關輸出目錄。

簡單說，GAL 不是 spec-kit 或 gstack 的 fork，它是把 GSD、gstack、Spec Kit 的不同優點重組成一個更適合 Copilot、Gemini 與 Codex 的 repo-local operating model。

## 最終運作模型

GAL 現在採用控制平面與執行層的嚴格分層。

| 層級 | 職責 | 指令 |
| --- | --- | --- |
| 控制平面 | 初始化 repo、讀取狀態、推薦下一步、收斂連續性、路由研究 | `/gal init`、`/gal status`、`/gal whats-next`、`/gal wrap-up`、`/gal research` |
| 專家執行層 | 規劃、provider-routed 規劃審查、設計、除錯、審查、QA、發佈、記憶管理、安全守護 | `/planning`、`/deep-planning`、`/plan-to-prompt`、`/review`、`/qa`、`/ship` 及下方完整專家目錄 |

`/gal` 不重複實作專家行為。專家指令會把結果回寫到 `/gal` 讀取的主要檔案與計畫區段。

## 主要檔案 (Project files)

這些是 GAL 會讀取或更新的持久檔案與輸出位置。

| 路徑 | 用途 |
| --- | --- |
| `.dev/project.md` | Repo 摘要、技術棧、目標、限制 |
| `.dev/state.md` | 活動計畫索引、阻塞點、工作階段連續性 |
| `docs/plans/<plan-slug>.md` | 人類可讀的計畫文件（範圍、理由、需求） |
| `.dev/plans/<plan-slug>.prompt.md` | AI 執行工作檔案——可變清單、workflow 狀態、執行狀態、回寫目標 |
| `DESIGN.md` | Repo 層級設計治理（設計系統，非計畫專屬） |
| `CLAUDE.md` | Repo 本地操作備注，如部署設定與設計參考 |
| `docs/designs/<plan-slug>/` | 計畫綁定的設計資產：`variant-approved.json`、`variant-approved.png`、`handoff-final.html` |
| `docs/qa-reports/` | QA 報告：`YYYYMMDD-<plan-slug>.md` / `YYYYMMDD-<plan-slug>-report-only.md` |
| `docs/design-reports/` | 設計審查報告：`YYYYMMDD-<plan-slug>-rNN.md` |
| `docs/research/` | 研究筆記：`YYYYMMDD-<plan-slug>-<topic>.md` |
| `.dev/learnings.jsonl` | Repo 本地的制度化記憶 |

### 計畫執行 Sections

執行工作檔（`.prompt.md`）包含三個由專家指令寫入的 sections，各自有明確的所有權規則：

| Section | 寫入者 | 消費者（唯讀） | 用途 |
| --- | --- | --- | --- |
| `## Open Questions` | `/planning`（初始化 scaffold）、規劃階段 review lanes 追加，engineering review lane 關閉已解決項目 | `/ship`、`/gal status`、`/gal whats-next` | 未解決假設與決策的唯一 canonical list，ID 格式 `OQ-NNN` |
| `## Tasks` | engineering review lane（唯一初始化者，Eng Review CLEAR 後），實作階段只能更新完成狀態 | `/review`、`/qa`、`/ship`、`/gal status`、`/gal whats-next` | 可驗證的任務清單，ID 格式 `T-NNN` |
| `## Analyze` | `/review`（唯一寫入者，verdict：`CLEAR` / `DRIFT-OPEN` / `NOT-RUN`） | `/ship`、`/gal status`、`/gal whats-next` | Drift 檢查：變更是否偏離計畫範圍？ |

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
/planning
/deep-planning
/plan-to-prompt
/gal whats-next

# Codex CLI（skill mention 介面，使用 $ 前綴，不是 /）
$gal init
$planning
$deep-planning
$plan-to-prompt
$gal whats-next
```

最終模型中沒有公開的 `gal sync` 步驟。轉接器生成屬於安裝層的內部作業，不是使用者工作流程。

`Setup-Machine.ps1` 與 `setup-machine.sh` 也會合併一個 VS Code 使用者設定，讓 Copilot Chat 忽略 `~/.agents/skills`。這樣 VS Code 不會重複列出共享的 reusable skills，同時 Gemini 會改用 `~/.gemini/commands` 的原生指令，Codex 會改用 `~/.codex/skills` 的 command skills。

這兩支 setup 腳本現在也會把 `mcp-servers.example.json` 的 canonical MCP catalog，搭配 `mcp-servers.local.json` 的本地覆蓋，合併到 VS Code、Gemini CLI 與 Codex CLI 各自擁有的 MCP 設定檔中。

## 如何把 GAL 用在 AI-First 遊戲素材生產

GAL 也可以用來驅動一套以 AI 為核心的遊戲 2D / 3D assets 生產 workflow。

重點不是把所有圖形工具等量整合，而是先用 ComfyUI 做生成與變體探索，再把資產送進最小但足夠的後段工具做整理、結構化與導出。

### 支援的素材產線

| 產線 | 預設流程 |
| --- | --- |
| 2D 概念與插畫素材 | ComfyUI -> GIMP |
| Sprite 與像素素材 | ComfyUI -> Aseprite |
| UI、icon 與 HUD 素材 | ComfyUI -> Figma -> Inkscape |
| 3D 遊戲素材 | ComfyUI -> Blender |

### 建議工具組合

- `joenorton/comfyui-mcp-server` 作為生成層
- `maorcc/gimp-mcp` 處理點陣清理與導出
- `willibrandon/pixel-mcp` 處理 sprite、動畫與 spritesheet workflow
- `grab/cursor-talk-to-figma-mcp` 處理 UI / HUD 版面與元件
- `grumpydevorg/inkscape-mcps` 處理 SVG 清理與可預期導出
- `ahujasid/blender-mcp` 處理一般 3D 遊戲素材

### GAL 現在知道哪些內容

目前 repo 中的遊戲素材 workflow 指引主要放在：

- [docs/graphics-workflow.md](docs/graphics-workflow.md)，整理產線路由、handoff 規則與 output contract
- [docs/graphics-mcp-setup.md](docs/graphics-mcp-setup.md)，整理 MCP stack 的安裝與分工
- [docs/graphics-external-knowledge.md](docs/graphics-external-knowledge.md)，整理官方文件來源
- `skills/graphics-workflow` 與 `skills/game-*` 下面的 workflow skills，分別處理路由、2D、pixel、UI、3D 與最終導出

### 遊戲素材工作節奏

```text
/gal init
/planning
/deep-planning
/plan-to-prompt

# 然後搭配 game asset skills 實作：
# - graphics-workflow
# - game-2d-assets
# - game-pixel-assets
# - game-ui-assets
# - game-3d-assets
# - game-asset-export
```

請把 ComfyUI 視為預設入口，Blender、GIMP、Aseprite、Figma、Inkscape 則視為後段整理與導出工具，而不是另一個生成中心。

## 如何把 GAL 用在 Godot C Sharp

GAL 不會另外新增一個 Godot 專屬 agent。做法是讓現有的規劃、實作、審查、QA 指令，透過 convention、skill 與 MCP 工具，能直接操作 Godot 4 C# repo。

### 需要安裝什麼

Godot C# 專案的預設工具組合是：

- Godot 4.x with C# support
- VS Code 的 C# tooling，提供 IntelliSense 與診斷
- `Coding-Solo/godot-mcp`，負責啟動編輯器、執行專案、擷取 debug 輸出
- `n24q02m/better-godot-mcp`，負責離線編輯 `.tscn` 與其他資源
- `MingHuiLiu/godot4-runtime-mcp`，負責運行時場景樹、signal、log、screenshot 檢查

完整分工與安裝方式見 [docs/godot-mcp-setup.md](docs/godot-mcp-setup.md)。

### GAL 已經補上的 Godot 知識

目前 repo 中的 Godot 支援主要放在：

- [conventions/csharp.md](conventions/csharp.md)，包含 Godot runtime 規則、`partial class`、signals、exports、lifecycle methods
- [docs/godot-external-knowledge.md](docs/godot-external-knowledge.md)，整理官方 Godot C# 文件
- `skills/godot-*` 下面的 5 個 skill，分別處理 project ops、scene authoring、scripting、runtime debug、asset pipeline

### 在 Godot Repo 裡怎麼開始

進入目標 Godot C# repo 後：

```text
/gal init
/planning
/deep-planning
/plan-to-prompt
```

如果你用的是 Codex CLI：

```text
$gal init
$planning
$deep-planning
$plan-to-prompt
```

`/gal init` 應該會根據 `project.godot` 加上 `*.csproj` 判定這是一個 Godot C# repo，並把這件事寫進 `.dev/project.md`。

### 建議的 Godot 工作流

一般功能開發建議這樣跑：

```text
/gal init
/planning
/deep-planning
/plan-to-prompt

# 然後搭配 Godot skills / MCP 工具實作：
# - godot-project-ops
# - godot-scene-authoring
# - godot-scripting
# - godot-runtime-debug
# - godot-asset-pipeline

/review
/qa
/ship
```

### 工具怎麼選

按責任分工使用：

- build、import、export、CI 自動化：Godot CLI
- 啟動 editor、執行專案、抓 debug 輸出：`godot-mcp`
- 不啟動 editor 直接改 scene / resource：`better-godot-mcp`
- 檢查 live nodes、signals、logs、runtime state：`godot4-runtime-mcp`

### Godot CLI 範例

```bash
godot --headless --path <project> --build-solutions
godot --headless --path <project> --import
godot --headless --path <project> --export-release <preset> <output>
godot --path <project> -e
godot --path <project>
```

### 一個重要限制

Godot 遊戲程式碼和 GAL 外部工具的相容性目標不一樣：

- Godot runtime code 應維持在專案實際支援的版本，通常是 `.NET 8 / C# 12`
- 外部工具和 MCP server 可以使用較新的 runtime，因為 Godot 不會載入它們

寫 gameplay code 時，請以 [conventions/csharp.md](conventions/csharp.md) 中的 Godot runtime section 為準。

### 建議的工作節奏

控制平面的建議用法如下：

```text
/gal init
/planning
/deep-planning
/plan-to-prompt
/gal whats-next
/gal pipeline        # 自動串接實作 → 測試 → 審查
/ship
```

或是偏好手動控制：

```text
/gal init
/planning
/deep-planning
/plan-to-prompt
/gal whats-next
<實作>
/review
/qa
/ship
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
| `/gal pipeline` | 一鍵執行實作 → 測試 → 審查 | 活動計畫 `## Test Plan`、`model-roles.local.md` | 計畫 `## Status`、`## Test Results`、`## Review Results` | 以不同廠 AI 串接三個 golem；遇到阻塞點自動停下 |

### 可發現性 Alias

執行 `Setup-Machine` 後，這些 alias 會在 Copilot / Gemini 中作為 slash 指令可用，在 Codex 中則以同名 skill 出現。Gemini 會從 `~/.gemini/commands` 讀取原生指令，Codex 則會從 `~/.codex/skills` 讀取同一批 command 目錄。

| Alias | Copilot / Gemini | Codex CLI |
| --- | --- | --- |
| gal-init | `/gal-init` | `$gal-init` |
| gal-status | `/gal-status` | `$gal-status` |
| gal-whats-next | `/gal-whats-next` | `$gal-whats-next` |
| gal-wrap-up | `/gal-wrap-up` | `$gal-wrap-up` |
| gal-pipeline | `/gal-pipeline` | `$gal-pipeline` |

## 專家指令目錄

這些指令直接實作工作層，不需要經過 `/gal` 路由。

### 規劃

| 指令或 Lane | 用途 | 主要寫入 |
| --- | --- | --- |
| `/planning` | 建立或覆寫人類可讀的 source plan | `docs/plans/<plan-slug>.md`、`.dev/state.md` |
| `/deep-planning` | 將規劃中的文字材料收斂成 review-ready source plan | `docs/plans/<plan-slug>.md`、`.dev/state.md` |
| `/plan-to-prompt` | 由 source plan materialize 出 execution prompt | `.dev/plans/<plan-slug>.prompt.md`、`.dev/state.md` |
| business、design、engineering review lanes | 透過 provider routing 執行規劃階段審查；若已安裝 upstream gstack，映射到對應 skill，否則 fallback 到 `/gal golem-analyst`、`/gal golem-designer`、`/gal golem-architect` | `## Review Results`、`## Open Questions`，其中 engineering lane 另外寫 `## Test Plan`、`## Tasks`、`<!-- ENG_REVIEW: CLEAR -->` |
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
| `/review` | Staff 級別的程式碼變更審查，找出 CI 漏掉的問題 | 計畫 `## Review Results`、`## Analyze` |

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
| `/setup-deploy` | 一次性部署設定 | `CLAUDE.md` |
| `/document-release` | 更新文件以符合已發佈的程式碼 | Repo 文件、PR 內容 |

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
/planning
/deep-planning
/plan-to-prompt
/gal whats-next
/gal pipeline        # 實作 → 測試 → 審查（多廠 AI 串接）
/ship
```

或逐步手動執行：

```text
/gal init
/planning
/deep-planning
/plan-to-prompt
/gal whats-next
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
```

## 狀態邏輯

控制平面能運作，是因為專家指令會把可預測的區段回寫到活動計畫中。

| 區段 | 由誰寫入 | 由誰讀取 |
| --- | --- | --- |
| `## Review Results` | 審查類專家指令 | `/gal status`、`/gal whats-next` |
| `## Test Plan` | engineering review lane | `/qa`、`/qa-only` |
| `## Test Results` | `/qa`、`/qa-only` | `/gal status`、`/gal whats-next` |
| `## Ship` | `/ship` | `/gal status`、`/gal whats-next`、`/land-and-deploy` |
| `## Deploy` | `/land-and-deploy` | `/gal status`、`/gal whats-next` |
| `### Handoff Notes` | `/gal wrap-up` | `/gal status`、`/gal whats-next` |

### 狀態判定疑難排解

- 如果 `.dev/state.md` 不存在，表示 repo 尚未初始化。
- 如果 `.dev/state.md` 存在，GAL 應該從 `.dev/plans/<plan-slug>.prompt.md` 的 `## Status` 讀取活動 workflow。
- 如果 `.dev/state.md` 已存在但 GAL 仍無法投影狀態，應視為 state 結構異常，不是要重新執行 `/gal init`。

### VS Code 技能重複疑難排解

- VS Code 目前會同時掃描 `~/.copilot/skills` 與 `~/.agents/skills`。
- GAL 會刻意把共享 reusable skills 安裝在 `~/.agents/skills`，所以若沒有額外設定，VS Code 可能把部分技能列出兩次。
- 重新執行 `scripts/Setup-Machine.ps1` 或 `scripts/setup-machine.sh`，即可讓安裝腳本自動合併建議的 VS Code 設定。
- 如果你要手動修復既有安裝，請在 VS Code 使用者 `settings.json` 加入：

```json
"chat.agentSkillsLocations": {
  "~/.agents/skills": false
}
```

- 這只會讓 VS Code 忽略重複來源，不會移除 `~/.agents/skills`，因此共享 reusable skills 在 Gemini CLI 與 Codex CLI 仍可正常使用。

## 架構摘要

```text
~/golem-agents-legion/     canonical 方法論與指令來源
<repo>/.dev/              repo 本地狀態與連續性
docs/plans/<slug>.md      人類可讀的計畫文件
.dev/plans/<slug>.prompt.md  AI 執行工作檔案（可變狀態）
mcp-servers.example.json  受版本控制的 MCP 真相來源
mcp-servers.local.json    本地 MCP 啟停 / 覆蓋層（gitignored）
~/.copilot/skills/        已安裝的 Copilot skills
~/.gemini/skills/         舊版 Gemini runtime 目錄（setup 會清理）
~/.gemini/commands/       生成的 Gemini 原生 slash 指令
~/.agents/skills/         共用 reusable skills（VS Code 應忽略這個路徑）
~/.codex/skills/          已安裝的 Codex command skills
%APPDATA%/Code/User/mcp.json   由 setup 合併的 VS Code MCP 設定
~/.gemini/settings.json   由 setup 合併的 Gemini settings + mcpServers
~/.codex/config.toml      由 setup 合併的 Codex config + [mcp_servers.*]
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
| `<MCP_FILESYSTEM_PATHS>` | filesystem MCP 可讀取的根目錄清單（逗號分隔，可省略） | MCP manifest merge |
| `<MCP_MEMORY_FILE_PATH>` | 持久化 MCP memory JSON 檔路徑 | MCP manifest merge |
| `<CONTEXT7_API_KEY>` | 需要時提供給特定 runtime 的 Context7 API key | MCP manifest merge |
| `<OBSIDIAN_API_KEY>` | Obsidian Local REST API key | Obsidian MCP |
| `<OBSIDIAN_BASE_URL>` | Obsidian Local REST API base URL | Obsidian MCP |

模型路由設定請複製 [model-roles.example.md](model-roles.example.md) 為 `model-roles.local.md` 後自訂。

如果你需要 provider-specific 的 MCP 差異，請編輯 `mcp-servers.local.json` 後重新執行 Setup-Machine。manifest 會引用 `config.local.env` 裡的本地 secrets 與路徑值。

## 重要文件

| 路徑 | 用途 |
| --- | --- |
| [docs/ai-agent-onboarding.md](docs/ai-agent-onboarding.md) | AI 代理與維護者的閱讀順序 |
| [docs/gal-control-plane-contracts.md](docs/gal-control-plane-contracts.md) | `/gal` 讀寫合約的 canonical 定義 |
| [docs/gstack-integration.md](docs/gstack-integration.md) | GAL 如何在不暴露重複公開指令的前提下路由 optional gstack provider |
| [docs/gstack-command-contracts.md](docs/gstack-command-contracts.md) | provider 與 specialist contracts 的實作藍圖 |
| [docs/godot-mcp-setup.md](docs/godot-mcp-setup.md) | 推薦的 Godot C# MCP 工具鏈與選用規則 |
| [docs/godot-external-knowledge.md](docs/godot-external-knowledge.md) | 官方 Godot C# 文件索引與真相來源 |
| [docs/runtime-verification.md](docs/runtime-verification.md) | 指令與執行平面的 live/manual 驗證狀態 |
| [docs/command-dispatch-architecture.md](docs/command-dispatch-architecture.md) | Dispatch 模型與 alias 政策 |
| [commands/commands.md](commands/commands.md) | 已安裝的指令表面與 alias 架構 |
| [workflows/coding.md](workflows/coding.md) | 原始開發流程狀態機參考 |

## GAL 不再視為公開工作流程的部分

- 舊的分級式風險標籤已不再屬於目前的指令表面與規劃模型
- 使用 GAL 的專家指令不需要另外安裝 upstream gstack

## 授權

MIT — 詳見 [LICENSE](LICENSE).
