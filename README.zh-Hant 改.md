# Golem Agents Legion (GAL)

[English](../README.md) | 繁體中文

GAL 是一套 AI 工作系統，是為了協助使用者能夠更有步驟的開發，並方便在不同 AI 工具間切換使用。核心是 11 個分工明確的 Golem Agent 加上 `/gal` 控制平面，構成一個 artifact-driven 的開發模型。所有持久狀態以 Markdown 檔案儲存於本地（`.dev/`、`docs/plans/`），讓 Copilot、Gemini CLI、Codex CLI與 Github Copilot 皆可執行同一套工作流程。

狀態管理借鏡自 [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done) 的 phase-based discipline：明確狀態（`.dev/state.md`）、verification gates 與結構化的執行生命週期，讓 `/gal status` 和 `/gal whats-next` 有能力投影整個 repo 的工作進度。

- [gstack](https://github.com/garrytan/gstack) 提供 specialist workflow semantics 與指令語彙。GAL 以 GAL-native skills 重新實作這些語意，作為可插拔的規劃模組（詳見 [mod/gstack.md](mod/gstack.md)）。

## 快速開始

1. git clone 本專案
`git clone https://github.com/monkey1wizard/golem-agents-legion.git`。

2. 進入資料夾後執行安裝，windows 使用 `./scripts/Setup-Machine.ps1` ， macOS 使用 `./scripts/setup-machine.sh` ，完成後即可開始於專案中使用。

3. 進入專案資料夾內開啟 Claude Code / Codex cli / Gemini cli / Github Copilot 並執行：

```text
# Copilot / Gemini CLI（slash-command 介面）
/gal init

# Codex CLI（skill mention 介面，使用 $ 前綴，不是 /）
$gal init
```

## Golem Agents

GAL 的核心是 11 個專門化 agent，各自有獨立的 `.agent.md` 定義檔。分職而立的設計原則：

- **prompt 精簡**：每個 agent 只載入自己的職責定義，不浪費 context window
- **驗證可信**：獨立的 tester / reviewer / verifier 比單一 mega-agent 做所有事更可信
- **可組合**：按 task 風險等級決定啟用哪些 agent，不是一體全開

### 分類

| 分類 | 啟動方式 | 成員 |
| --- | --- | --- |
| **Pipeline** | 由 `/gal pipeline` 自動串接 | implementer、tester、reviewer、verifier |
| **Utility** | 任何時候直接呼叫 | debugger、scribe |
| **Domain** | 由指令或使用者直接諮詢 | architect、analyst、designer、researcher、librarian |

### Domain Agents

Domain agents 提供專業諮詢，可以在任何階段被使用者或指令調用。

| Agent | 職責 |
| --- | --- |
| **architect** | 對抗式的計畫審查——權衡分析、過度工程偵測、bug surface、公開 API 風險 |
| **analyst** | 商業邏輯審查——ROI、domain 正確性、使用者影響 |
| **designer** | 視覺設計、UX flow、accessibility、設計系統一致性 |
| **researcher** | 本地優先的研究與結構化綜合，帶有 source attribution |
| **librarian** | Obsidian vault 寫入——inbox processing、知識萃取 |

### Pipeline Agents

Pipeline 是 GAL 的自動化執行核心。`/gal pipeline` 逐 task 串接四個 agent：

```text
T-NNN ──> implementer ──> tester ──> reviewer ──> ✓ commit
                                         │
                                    如果 REJECT
                                         │
                                    implementer 修正後重跑

所有 task 完成後：──> verifier ──> 確認 plan 目標達成
```

每個 task 必須通過 implement → test → review → commit 才能往下。最後由 verifier 做 goal-backward 驗證，確認整個 plan 的目標已達成。

| Agent | 職責 | 關鍵規則 |
| --- | --- | --- |
| **implementer** | 讀取 plan spec，產出 code + atomic commit | 遵守 scope fence（Trivial/Standard 有禁止清單） |
| **tester** | 從 spec 與 public API 寫測試，不讀實作 | 必須與 implementer 使用不同模型 |
| **reviewer** | Staff engineer 等級的 diff 審查 | 應與 implementer 不同模型，能力不應弱於 implementer |
| **verifier** | Goal-backward 驗證 + plan lifecycle ending | 抽取知識到 `docs/`，標記 plan 可關閉 |

### Utility Agents

| Agent | 職責 |
| --- | --- |
| **debugger** | 科學方法 bug 調查：假說、驗證、根因確認後才修 |
| **scribe** | 每日工作日記 + 宵禁系統執行者 |

### 審查包組裝

Strategic weight 使用 composable review pack——按 task 需求組裝，不是固定的所有人都要看：

| 角色 | 何時加入 |
| --- | --- |
| Architect | 永遠加入（Strategic 為 full，Standard 為 lite） |
| Designer | 永遠加入（無 UI 面向時回報 no-impact verdict） |
| Analyst | 只在涉及商業邏輯、定價、權限或客戶可見變更時 |
| 其他（reviewer、debugger） | 視 task 類型決定 |

進入 IMPLEMENT 需要 pack 內所有成員 APPROVE。Analyst 不在 pack 內就不需要 analyst 批准。

### 模型角色強制

跨模型驗證是 GAL 的預設護欄：

- **Tester 必須與 implementer 使用不同模型**——獨立驗證
- **Reviewer 應與 implementer 不同**——新鮮視角
- **Reviewer 的能力不應弱於 implementer**——要能 review 得動
- **規劃作者與 architect 應盡量不同**——cross-check plan

模型映射在 `model-roles.local.md` 中設定。

### 直接呼叫 vs Pipeline 呼叫

| 類型 | 允許？ | 規則 |
| --- | --- | --- |
| Domain agent 諮詢 | 是 | 唯讀建議，沒有正式 verdict |
| Utility agent | 是 | 獨立 helper |
| Pipeline agent 諮詢 | 僅限諮詢 | 完整執行權限只來自 `/gal pipeline`，不來自 dispatcher state |

### 宵禁系統

所有 agent 遵守宵禁邊界：

- **22:00**：非 scribe agent 阻擋（若當日日記未寫）
- **22:00–23:00**：日記已寫則可提議 `/gal wrap-up`
- **23:00**：所有 agent 停止，包括 scribe
- **Override**：使用者可說「override curfew」，單次有效

## 控制平面

`/gal` 指令處理 repo 生命週期管理，不重複實作專家行為。

| 指令 | 用途 |
| --- | --- |
| `/gal init` | 初始化 repo：建立 `.dev/project.md` 與 `.dev/state.md` |
| `/gal status` | 完整狀態投影：活動計畫、審查/測試狀態、阻塞點、連續性 |
| `/gal whats-next` | 推薦單一下一步動作 |
| `/gal wrap-up` | 收斂 session：寫入 `### Handoff Notes` 與 `## Session Continuity` |
| `/gal research` | 進入研究工作流 |
| `/gal pipeline` | 逐 task 自動串接 implementer → tester → reviewer → verifier |

## 開發工作流

GAL 的開發工作流是 artifact-driven 的：控制平面讀取 plan artifacts 裡的 markdown sections 來判定狀態，不依賴特定的規劃指令。

### Risk Weight

不是每個變更都需要同樣的流程。Risk weight 決定啟用多少護欄：

| Weight | 適用場景 | 流程 | 需要 plan？ |
| --- | --- | --- | --- |
| **Trivial** | Typo 修正、明顯 bug、單檔編輯 | 直接實作 → 可選 review/QA | 不需要 |
| **Standard** | 小功能、已知原因的 bug fix、2–3 檔 | Plan（輕量）→ 實作 → `/review` → `/qa` → `/ship` | 輕量 plan |
| **Strategic** | 新 feature、架構變更、高風險、跨模組 | Plan（完整）→ review pack → 實作 → `/review` → `/qa` → `/ship` | 完整 plan |

**升級規則**：任何變更可以在進行中升級到 Strategic。停下來、完善 plan、啟用完整 review pack。

### Execution Lifecycle

| 階段 | 入口訊號 | 指令 | 主要 Artifacts |
| --- | --- | --- | --- |
| 草擬 plan | 無活動 plan | 規劃模組或手動建立 | `docs/plans/<slug>.md` + `.prompt.md`、`.dev/state.md` |
| 審查 plan | 草案存在、尚未鎖定 | 審查指令 | `## Open Questions`、`## Tasks`、`## Test Plan`、`## Review Results` |
| 實作 | Tasks 存在且有剩餘工作 | `/gal pipeline` 或手動 | `## Status`、`## Tasks`、code changes |
| Code review 與 QA | 實作到達有意義的 checkpoint | `/review`、`/qa` | `## Analyze`、`## Review Results`、`## Test Results` |
| 收尾或發佈 | 工作暫停或準備合併 | `/gal wrap-up`、`/ship`、`/land-and-deploy` | `### Handoff Notes`、`## Ship`、`## Deploy` |

### Scope Fence

Trivial 與 Standard weight 缺少完整 review pack，為防止意外的架構損傷，有兩層防禦：

**Layer 1 — Implementer Scope Fence**：implementer 的指令集包含禁止操作清單。觸發任何項目必須停下來請求升級到 Strategic：

- 建立或刪除專案檔（`.csproj`、`package.json` 等）
- 新增或移除 package dependency
- 跨架構層移動檔案
- 建立新 interface 或 abstract class
- 修改 DI registration 或 service composition
- 修改被 2+ consumer 使用的 public API signature

**Layer 2 — Standard Mandatory Reviewer**：Standard 的 reviewer 是必要的（不是可選），但只檢查 correctness + architecture 兩個維度。

### Protected Paths

每個 repo 的 `.dev/project.md` 包含 `## Protected Paths`，列出架構關鍵檔案。Trivial 或 Standard 工作觸碰 protected path 會自動觸發 Strategic 升級。

### Plan Lifecycle

Plan 是暫時的工作檔案，不是永久紀錄。`docs/plans/` 是 staging area。

1. 規劃指令建立 plan pair → `docs/plans/<type>-<slug>.md` + `.prompt.md`
2. 執行 plan 自我追蹤進度 → `## Status`
3. 測試寫入結果 → `## Test Results`
4. 審查寫入發現 → `## Review Results`、`## Analyze`
5. 驗證確認目標 → 萃取知識到 `docs/`，標記 plan 可關閉
6. **Plan 在 lifecycle 關閉後刪除** → task memory 歸零

## Canonical Artifacts

| 路徑 | 用途 |
| --- | --- |
| `.dev/project.md` | Repo 摘要、技術棧、目標、限制 |
| `.dev/state.md` | 活動計畫索引、阻塞點、工作階段連續性 |
| `docs/plans/<plan-slug>.md` | 人類可讀的計畫文件 |
| `docs/plans/<plan-slug>.prompt.md` | AI 執行工作檔案 |
| `DESIGN.md` | Repo 層級設計治理 |
| `CLAUDE.md` | Repo 本地操作備注 |
| `docs/designs/<plan-slug>/` | 計畫綁定的設計資產 |
| `docs/qa-reports/` | QA 報告 |
| `docs/design-reports/` | 設計審查報告 |
| `docs/research/` | 研究筆記 |
| `.dev/learnings.jsonl` | Repo 本地制度化記憶 |

### 計畫執行 Sections

執行工作檔（`.prompt.md`）包含三個由專家指令寫入的 sections：

| Section | 寫入者 | 消費者（唯讀） | 用途 |
| --- | --- | --- | --- |
| `## Open Questions` | `/planning`（初始化）、規劃階段 review lanes（追加），engineering review lane（關閉已解決項目） | `/ship`、`/gal status`、`/gal whats-next` | 未解決假設與決策的唯一 canonical list |
| `## Tasks` | engineering review lane（唯一初始化者），或手動建立 | `/review`、`/qa`、`/ship`、`/gal status` | 可驗證的任務清單 |
| `## Analyze` | `/review`（唯一寫入者） | `/ship`、`/gal status` | Drift 檢查：diff 是否偏離計畫範圍 |

### 狀態回寫邏輯

控制平面能運作，是因為專家指令會把可預測的區段回寫到活動計畫中：

| 區段 | 由誰寫入 | 由誰讀取 |
| --- | --- | --- |
| `## Review Results` | 審查類專家指令 | `/gal status`、`/gal whats-next` |
| `## Test Plan` | engineering review lane，或手動建立 | `/qa`、`/qa-only` |
| `## Test Results` | `/qa`、`/qa-only` | `/gal status`、`/gal whats-next` |
| `## Ship` | `/ship` | `/gal status`、`/land-and-deploy` |
| `## Deploy` | `/land-and-deploy` | `/gal status`、`/gal whats-next` |
| `### Handoff Notes` | `/gal wrap-up` | `/gal status`、`/gal whats-next` |

## 指令入口

GAL 的完整 specialist 指令索引、各指令的讀寫 artifact、upstream gstack 語意映射，集中在 [mod/gstack.md](mod/gstack.md)。

指令的精確合約與寫回欄位見 [docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md)。

## 研究工作流

研究是獨立於開發的工作流程，透過 `/gal research` 進入。研究工作流有自己的狀態機與 tier 系統，可以和開發工作流平行運作。

### Tier 系統

| Tier | 適用場景 | 狀態 | 產出 |
| --- | --- | --- | --- |
| R0（Quick） | 單一來源查找、預期有已知答案 | RESEARCH → DOCUMENT | 簡短筆記或 chat 回覆 |
| R1（Standard） | 多來源調查、需要比較 | RESEARCH → SYNTHESIZE → DOCUMENT | `docs/research/` 內的研究文件 |
| R2（Deep） | 未知領域、跨天、跨模組 | RESEARCH → SYNTHESIZE → REVIEW → DOCUMENT | 完整研究報告 + vault 知識萃取 |

### 生命週期

| 狀態 | 執行者 | 工作 |
| --- | --- | --- |
| RESEARCH | researcher | 本地優先的調查與證據收集 |
| SYNTHESIZE | researcher | 整理原始發現，辨識缺口與權衡 |
| REVIEW (R2) | architect / analyst / designer（條件加入） | 對稱的對抗式審查 |
| DOCUMENT (repo) | 使用者 / 任何模型 | 寫入 `docs/research/` |
| DOCUMENT (vault) | librarian | 寫入 Obsidian vault |

研究產出可以餵進 plan 或 vault 知識，不需要等開發流程完成。

## 延伸模組

GAL 支援可插拔的工作流擴充模組。

### OpenCLI

以 plugin 模式整合到 agent 工作流的結構化外部資料擷取工具。詳見 [mod/opencli.md](mod/opencli.md)。

### Remote Worker

跨機器的遠端任務派發與結果收集。詳見 [mod/remote-worker.md](mod/remote-worker.md)。

### gstack 規劃模組

gstack 現在是可選的 specialist provider。GAL 以 `/planning`、`/deep-planning`、`/plan-to-prompt` 作為公開規劃入口，再把 business / design / engineering review lanes 映射到 upstream gstack skills 或 fallback golems。你也可以完全不安裝 gstack，仍走完整規劃流程。

詳見 [mod/gstack.md](mod/gstack.md)。

### Godot C Sharp

現有指令透過 convention、skill 與 MCP 工具直接操作 Godot 4 C# repo。詳見 [mod/godot.md](mod/godot.md)。

### AI-First 遊戲素材

以 ComfyUI 為生成入口，搭配後段工具做整理與導出。詳見 [mod/graphworkflow.md](mod/graphworkflow.md)。

## 個人化設定

環境占位符、模型路由、MCP 覆蓋與 rerun setup 的操作說明，集中在 [personalization.md](personalization.md)。

## 深入閱讀

| 路徑 | 用途 |
| --- | --- |
| [docs/ai-agent-onboarding.md](../docs/ai-agent-onboarding.md) | AI 代理與維護者的閱讀順序 |
| [docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md) | `/gal` 讀寫合約的 canonical 定義 |
| [docs/gstack-integration.md](../docs/gstack-integration.md) | GAL 為何原生重新實作 gstack 語意 |
| [docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) | 專家技能的實作藍圖 |
| [docs/command-dispatch-architecture.md](../docs/command-dispatch-architecture.md) | Dispatch 模型與 alias 政策 |
| [docs/installation-topology.md](../docs/installation-topology.md) | 安裝層拓撲與 MCP 合併邏輯 |
| [workflows/coding.md](../workflows/coding.md) | 開發工作流狀態機參考 |
| [workflows/research.md](../workflows/research.md) | 研究工作流狀態機參考 |
| [agent/agents.md](../agent/agents.md) | Agent 定義、分類與啟用原則 |
| [mod/gstack.md](mod/gstack.md) | gstack 工作流模組：指令索引、語意映射與規劃流程 |
| [devguide.md](devguide.md) | 維護者決策地圖 |

## 參考

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)

## 授權

MIT — 詳見 [LICENSE](../LICENSE)。
