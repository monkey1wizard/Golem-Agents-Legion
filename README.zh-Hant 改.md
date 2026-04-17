# Golem Agents Legion (GAL)

[English](../README.md) | 繁體中文

GAL 是一套 AI 工作系統，是為了協助使用者能夠更有步驟地開發，並方便在不同 AI 工具間切換使用。核心是 11 個分工明確的 Golem Agent 加上 `/gal` 控制平面，構成一個文件驅動開發（Document-Driven Development）的工作模型。所有持久狀態以 Markdown 檔案儲存於本地（`.dev/`、`docs/plans/`），讓 Copilot、Gemini CLI、Codex CLI與 GitHub Copilot 皆可執行同一套工作流程。

狀態管理借鏡自 [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done) 的 phase-based discipline：explicit state (`.dev/state.md`)、 verification gates 與結構化的執行生命週期，讓 `/gal status` 和 `/gal whats-next` 有能力投影整個 repo 的工作進度。

- [gstack](https://github.com/garrytan/gstack) start-up 模式的專業分工角色與指令。GAL 內中不少指令與 SKILLS是參考於此的，同時也作為可插拔的規劃模組（詳見 [mod/gstack.md](mod/gstack.md)）。

## 快速開始

1. git clone 本專案
`git clone https://github.com/monkey1wizard/golem-agents-legion.git`。

2. 進入資料夾後執行安裝， Windows 使用 `./scripts/Setup-Machine.ps1` ， macOS 使用 `./scripts/setup-machine.sh` ，完成後即可開始於專案中使用。

3. 進入專案資料夾內開啟 Claude Code / Codex CLI / Gemini CLI / GitHub Copilot 並執行：

```text
# Copilot / Gemini CLI（slash-command 介面）
/gal init

# Codex CLI（skill mention 介面，使用 $ 前綴，不是 /）
$gal init
```

## 控制指令

| 指令 | 用途 |
| --- | --- |
| `/gal init` | 初始化 repo：建立 `.dev/project.md` 與 `.dev/state.md` |
| `/gal status` | 完整狀態投影：活動企劃、審核/測試狀態、阻塞點、連續性 |
| `/gal whats-next` | 推薦單一下一步動作 |
| `/gal wrap-up` | 收斂工作：寫入 `### Handoff Notes` 與 `## Session Continuity` |
| `/gal research` | 進入研究工作流 |
| `/gal pipeline` | 逐任務自動串接 implementer → tester → reviewer → verifier |

## 開發工作流

```text
init -> planning ─┬─ (scoped feature) ──────────────> plan-to-prompt -> gal-pipeline
                  ↓                                        ↑
                  │                                        │
                  └─ (structural change) -> deep-planning ─┘
```

不是每個改動都需要走完全程，小範圍功能變更可以從 `/planning` 直接進 `/plan-to-prompt`，只有結構性改動才需要先經過 `/deep-planning` 收斂。

### planning

`/planning` 會把新的需求整理成正式的企劃文件(source plan)，寫入 `docs/plans/<plan-slug>.md`。

這個階段的重點是把你的目標、需求等內容整理成穩定的人類可讀文件，並在 `## Open Questions` 記錄尚未定案的項目。`/planning` 不會建立執行工作檔，它只決定目前的企劃文件是否已經適合直接進 `/plan-to-prompt`，或應先進 `/deep-planning` 做進一步收斂與架構審核。

### deep-planning

`/deep-planning` 會對既有企劃文件進行深度規劃，會把規劃材料整理回同一份 `docs/plans/<plan-slug>.md`，並把架構審核的結果寫回企劃文件的 `## Review Results > ### Architecture Review` 與 `## Approval > Architect review`。若架構審核仍有阻塞問題，工作就留在 deep-planning 繼續修正。只有在企劃文件已經收斂到足以進入執行階段時，才往下進 `/plan-to-prompt`。

如果此功能變更還需要商務、設計或工程審核，這些審核應視為企劃階段的進階項目審查，以此用來補強企劃文件內容，而不是取代企劃文件本身。

### plan-to-prompt

`/plan-to-prompt` 會把企劃文件依照模板轉換成 `.dev/plans/<plan-slug>.prompt.md` 作為執行階段使用，並更新狀態。完成後可執行 `/gal status` 或 `/gal whats-next` ，系統會掃描執行工作檔並回答你。有了執行工作檔了以後， `/gal pipeline` 才能正常執行。若企劃文件在轉換後又有變更，應重新執行 `/plan-to-prompt`，以確保企劃文件與 prompt 的內容保持一致。

## Golem Agents

GAL 的核心是 11 個專門化 agent，各自有獨立的 `.agent.md` 定義檔。分職而立的設計原則：

- **prompt 精簡**：每個 agent 只載入自己的職責定義，不浪費 context window
- **獨立性**：獨立的 tester / reviewer / verifier ，以確保驗證結果的可信度
- **可組合**：按任務風險等級決定啟用哪些 agent，不是一體全開

除了使用 `/gal` 指令以外，你也能直接呼叫 `golem-` 進行指定類型的工作。

### 分類

| 分類 | 啟動方式 | 成員 |
| --- | --- | --- |
| **Utility** | 任何時候直接呼叫 | debugger、scribe |
| **Domain** | 由指令或使用者直接諮詢 | architect、analyst、designer、researcher、librarian |
| **Pipeline** | 由 `/gal pipeline` 自動串接 | implementer、tester、reviewer、verifier |

### Utility Agents

| Agent | 職責 |
| --- | --- |
| **debugger** | 科學方法 bug 調查：假說、驗證、根因確認後才修 |
| **scribe** | 每日工作日記 + 宵禁系統執行者 |

### Domain Agents

Domain agents 提供專業諮詢，可以在任何階段被使用者或指令調用。

| Agent | 職責 |
| --- | --- |
| **architect** | 對抗式的企劃審核：權衡分析、過度設計偵測、bug surface、公開 API 風險 |
| **analyst** | 商業邏輯審核：ROI、domain 正確性、使用者影響 |
| **designer** | 視覺設計：UI/UX、設計系統一致性 |
| **researcher** | 本地優先的研究與結構化綜合，帶有 source attribution |
| **librarian** | Obsidian vault 寫入：inbox processing、知識萃取 |

### Pipeline Agents

Pipeline 是 GAL 的自動化執行核心。執行 `/gal pipeline` 後，每個任務會依序串接四個 agent：

```text
T-NNN ──> implementer ──> tester ──> reviewer ──> git commit ──> T-NNN+1
               ↑                         │
               │                         ↓
     auto-fix by review result <────── REJECT

所有任務完成後：──> verifier ──> 確認企劃目標達成
```

| Agent | 職責 | 關鍵規則 |
| --- | --- | --- |
| **implementer** | 依照企劃與當前 `T-NNN` 任務完成實作 | 一旦踩到架構邊界或發現企劃不足，必須停止並回 `/deep-planning` |
| **tester** | 根據規格與 public API 撰寫或補齊測試，驗證實作是否符合要求 | 不讀實作，且必須與 implementer 使用不同模型 |
| **reviewer** | 以資深工程師的標準審核變更差異、風險與完整性 | 若發現阻塞問題，必須退回 implementer 修正；使用之 AI 模型應不同於 implementer，且能力不應弱於 implementer |
| **verifier** | 在所有任務完成後，從企劃目標反向驗證成果是否真的達成 | 負責確認企劃是否可關閉，並把值得保留的知識抽回 `docs/` |

Pipeline agent 的正式執行權限只來自 `/gal pipeline`。單獨呼叫時視為諮詢，不會進入正式流程。Domain agent 和 utility agent 則可隨時直接呼叫。

### AI 模型與 Agent 規則

Pipeline 流程中，GAL 強制以不同模型進行審核與測試：

- Tester **必須**與 implementer 使用不同模型
- Reviewer **應**與 implementer 不同，能力不應弱於 implementer
- Planning 與 architect **應盡量**不同

上述規則在 `model-roles.local.md` 中設定。

### 宵禁系統

所有 agent 遵守宵禁邊界：

- **22:00**：非 scribe agent 阻擋（若當日日記未寫）
- **22:00–23:00**：日記已寫則可提議 `/gal wrap-up`
- **23:00**：所有 agent 停止，包括 scribe
- **Override**：使用者可說「override curfew」，單次有效

## 專案檔案 (Project files)

| 路徑 | 用途 |
| --- | --- |
| `.dev/project.md` | Repo 摘要、技術棧、目標、限制 |
| `.dev/state.md` | 活動企劃索引、阻塞點、工作階段連續性 |
| `.dev/learnings.jsonl` | Repo 本地制度化記憶 |
| `.dev/plans/<plan-slug>.prompt.md` | AI 執行工作檔案 |
| `CLAUDE.md` | Repo 本地操作備注 |
| `DESIGN.md` | Repo 層級設計治理 |
| `docs/designs/<plan-slug>/` | 企劃綁定的設計資產 |
| `docs/plans/<plan-slug>.md` | 人類可讀的企劃文件 |
| `docs/qa-reports/` | QA 報告 |
| `docs/research/` | 研究報告 |

### 執行工作檔

當你執行 `/plan-to-prompt` 之後，GAL 會建立 `.dev/plans/<plan-slug>.prompt.md`。你可以把它想成這個任務的工作看板。第一次使用時，主要只要看兩類資訊：

- `## Status > Workflow`：目前做到哪一個階段
- 其他回寫區段：規劃、測試、審核、交接的結果寫在哪裡

一般情況下，你不需要手動改 `Workflow:`。它會在建立執行工作檔時先設成 `DRAFT`，之後隨著工作的執行自動往下推進。

#### 工作狀態

| 狀態 | 代表什麼 |
| --- | --- |
| `DRAFT` | 執行工作檔剛建立，還沒正式進入任務執行 |
| `IMPLEMENT` | 正在做某個 `T-NNN` 任務的實作 |
| `TEST` | 實作已完成，正在測試 |
| `REVIEW` | 測試已通過，正在做程式碼審核 |
| `REVIEW — N阻塞問題s found` | 審核發現阻塞問題，必須修正後再重跑 |
| `ABSORBED` | 已確認目標達成，準備關閉企劃 |

#### 回寫區段

| 區段 | 你會在什麼時候看它 | 它的用途 |
| --- | --- | --- |
| `## Open Questions` | 還有需求、假設、邊界沒定案時 | 集中列出尚未解決的問題 |
| `## Tasks` | 要知道這個企劃實際要做哪些事時 | 任務清單，也是 pipeline 逐步執行的依據 |
| `## Analyze` | 想確認目前變更是否仍在原本企劃範圍內時 | 記錄 `/review` 對變更是否偏離企劃範圍 |
| `## Review Results` | 想看審核結果時 | 集中放各種審核的結果 |
| `## Test Plan` | 還沒開始測試，想知道應該測什麼時 | 記錄預計驗證的測試範圍 |
| `## Test Results` | 測試或 QA 跑完之後 | 記錄測試結果 |
| `### Handoff Notes` | 中途停下來，想知道上次做到哪裡時 | 提供下次接手時的上下文 |
| `## Ship` | 準備合併或已經完成 ship 時 | 記錄 ship 階段結果 |
| `## Deploy` | 已進入部署階段時 | 記錄部署結果與驗證 |

`/gal status` 和 `/gal whats-next` 主要就是讀這些已回寫的內容，來判斷目前進度與下一步，而不是只看單一欄位。

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
| REVIEW (R2) | architect / analyst / designer（條件加入） | 對稱的對抗式審核 |
| DOCUMENT (repo) | 使用者 / 任何模型 | 寫入 `docs/research/` |
| DOCUMENT (vault) | librarian | 寫入 Obsidian vault |

研究產出可以餵進企劃書或 vault 知識，不需要等開發流程完成。

## 延伸模組

GAL 支援可插拔的工作流擴充模組。此區塊正在整個重編，尚未完成

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
| [command-index.md](command-index.md) | 主要指令索引；先看現在該跑哪個 command |
| [devguide.md](devguide.md) | 維護者導航；先判斷自己正在改哪一層 |
| [personalization.md](personalization.md) | 本機模型路由、MCP 覆蓋與 rerun setup |
| [workflows/coding.md](workflows/coding.md) | 開發工作流狀態機參考 |
| [workflows/research.md](workflows/research.md) | 研究工作流狀態機參考 |
| [agent/agents.md](agent/agents.md) | Agent 定義、分類與啟用原則 |
| [mod/gstack.md](mod/gstack.md) | gstack 可選 provider 模組：規劃與 specialist 整合 |
| [mod/opencli.md](mod/opencli.md) | OpenCLI 路由與使用時機 |
| [mod/remote-worker.md](mod/remote-worker.md) | 遠端 worker 拓撲、所有權與 patch-first 收斂 |
| [mod/godot.md](mod/godot.md) | Godot C# 工作流模組 |
| [mod/graphworkflow.md](mod/graphworkflow.md) | AI-first 遊戲素材工作流模組 |

## 參考

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)

## 授權

MIT — 詳見 [LICENSE](../LICENSE)。
