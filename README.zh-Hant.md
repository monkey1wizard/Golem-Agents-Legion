# Golem Agents Legion (GAL)

[English](README.md) | 繁體中文

GAL 是一套 AI 工作系統，目標是讓開發工作更有步驟，並能在不同 AI 工具之間切換而不遺失 context window 。核心由 11 個分工明確的 Golem Agent 加上 `/gal` 控制平面構成，採文件驅動開發模型。所有持久狀態都以本地 Markdown 檔案保存，例如 `.dev/` 與 `docs/plans/`，讓 GitHub Copilot、Gemini CLI、Codex CLI 共享同一套工作流程與檔案。

狀態管理借鏡自 [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done) 的 phase-based discipline：explicit state (`.dev/state.md`)、 verification gates 與結構化的執行生命週期，讓 `/gal status` 和 `/gal whats-next` 有能力投影整個 repo 的工作進度。

- [gstack](https://github.com/garrytan/gstack) 的 specialist workflow semantics 對 GAL 有明顯影響，但在 GAL 中它是可選的協作工具，不是核心依賴。詳見 [docs/collaborative-tools/gstack.md](docs/collaborative-tools/gstack.md)。

## 快速開始

1. git clone 本專案
`git clone https://github.com/monkey1wizard/golem-agents-legion.git`。

2. 進入資料夾後執行安裝， Windows 使用 `./scripts/Setup-Machine.ps1` ， macOS 使用 `./scripts/setup-machine.sh` ，完成後即可開始於專案中使用。

3. 進入目標 repo 後開啟 GitHub Copilot、Gemini CLI 或 Codex CLI，然後執行：

```text
# Copilot / Gemini CLI（slash-command 介面）
/gal init

# Codex CLI（skill mention 介面，使用 $ 前綴，不是 /）
$gal init
```

目前的主要安裝面與 command surface 以 GitHub Copilot、Gemini CLI、Codex CLI 為主。其他工具若能讀取 repo-local instructions，可沿用部分方法論，但不代表已納入同等安裝與 command contract。

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

## 協作工具

GAL 支援可選的協作工具（collaborative tools）。這些工具擴充特定 lane 的能力，但不改變 `/gal` 的 control-plane ownership，也不是核心依賴——沒有安裝任何一個，GAL 仍能完整運作。

### 共用 Preflight 機制

所有協作工具在使用前都走同一套 5 狀態 preflight 檢查：

```text
applicability → availability → initialization status → readiness → route / degrade
```

| 狀態 | 意義 |
| --- | --- |
| `not-applicable` | 目前 lane 或任務不需要此工具，直接跳過 |
| `unavailable` | 機器或 runtime 無法存取此工具，走 fallback |
| `available-but-needs-init` | 工具存在但尚未完成首次設定，不在正常流程中自動初始化 |
| `available-but-not-ready` | 已安裝且已初始化，但當前 repo 或任務缺少所需 artifact |
| `ready` | 適用且所有前置條件滿足，進入工具能力 |

核心行為規則：不自動安裝、不自動初始化、不以模糊成功語言掩蓋缺失。每個工具啟用的 lane 都有明確的 degrade path。完整規格見 [docs/collaborative-tools/checking-contract.md](docs/collaborative-tools/checking-contract.md)。

### graphify — 結構化 Context

當 repo 存在 `graphify-out/` 產出時，planning、architect review 與 staff review 會自動注入 knowledge graph context，提升結構感知的準確度。沒有產出時維持原有行為。詳見 [docs/collaborative-tools/graphify.md](docs/collaborative-tools/graphify.md)。

### OpenCLI — 結構化外部擷取

以 plugin 模式接入 agent workflow 的結構化外部資料擷取工具，用於研究與 context 補充。詳見 [docs/collaborative-tools/opencli.md](docs/collaborative-tools/opencli.md)。

### gstack — 規劃審核 Lane

GAL 以 `/planning`、`/deep-planning`、`/plan-to-prompt` 作為公開規劃入口，再把 business / design / engineering review lanes 映射到 upstream gstack skills 或 fallback golems。不安裝 gstack 仍走完整規劃流程。詳見 [docs/collaborative-tools/gstack.md](docs/collaborative-tools/gstack.md)。

### Remote Worker

跨機器的遠端任務派發與結果收集。詳見 [docs/collaborative-tools/remote-worker.md](docs/collaborative-tools/remote-worker.md)。

### Godot C Sharp

現有指令透過 convention、skill 與 MCP 工具直接操作 Godot 4 C# repo。詳見 [docs/collaborative-tools/godot.md](docs/collaborative-tools/godot.md)。

### AI-First 遊戲素材

以 ComfyUI 為生成入口，搭配後段工具做整理與導出。詳見 [docs/collaborative-tools/graphworkflow.md](docs/collaborative-tools/graphworkflow.md)。

## 個人化設定

環境占位符、模型路由、MCP 覆蓋與 rerun setup 的操作說明，集中在 [docs/personalization.md](docs/personalization.md)。

## 文件

`docs/` 主要用於快速閱讀與查找，`docs/collaborative-tools/` 則是協作工具與相鄰 lane 導引的快速入門及索引。

| 路徑 | 用途 |
| --- | --- |
| [docs/command-index.md](docs/command-index.md) | 指令對照表 |
| [docs/devguide.md](docs/devguide.md) | 開發者手冊 |
| [docs/personalization.md](docs/personalization.md) | 本機模型路由、MCP 覆蓋與 rerun setup |
| [docs/collaborative-tools/checking-contract.md](docs/collaborative-tools/checking-contract.md) | 協作工具共用 preflight 檢查契約 |
| [docs/collaborative-tools/graphify.md](docs/collaborative-tools/graphify.md) | graphify 結構化 context 契約 |
| [docs/collaborative-tools/opencli.md](docs/collaborative-tools/opencli.md) | OpenCLI 協作工具導引與使用時機 |
| [docs/collaborative-tools/gstack.md](docs/collaborative-tools/gstack.md) | gstack 協作工具契約：規劃與 specialist 整合 |
| [docs/collaborative-tools/remote-worker.md](docs/collaborative-tools/remote-worker.md) | 遠端 worker 拓撲、所有權與 patch-first 收斂 |
| [docs/collaborative-tools/godot.md](docs/collaborative-tools/godot.md) | Godot C# 工作流導引 |
| [docs/collaborative-tools/graphworkflow.md](docs/collaborative-tools/graphworkflow.md) | AI-first 遊戲素材工作流導引 |

## 參考

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)

## 授權

MIT — 詳見 [LICENSE](LICENSE)。
