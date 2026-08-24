---
source: README.md
lang: zh-Hant
source_commit: fc4d3b7046db5e731f0d21728e1e093b0dd10bf8
translated_at: 2026-08-06
status: current
---

# Golem-Agents-Legion

[English](../../../README.md) · [日本語](../ja/README.ja.md) · **繁體中文**

一套以 Rust 打造的跨供應商 (Cross-provider) AI 工作系統。GAL 能原生跨 Claude Code、Codex CLI、GitHub Copilot、Antigravity CLI 與 opencode 運作，讓你在同一開發生命週期中自由混用不同供應商的模型。編譯式二進位架構在跨供應商傳遞脈絡時將 Token 開銷降至最低，同時透過儲存庫自有的 Markdown 檔案，讓規劃、實作、測試、審查與研究在工具間無縫移轉。
如同大法師般掌控屬於你的魔像軍團，每個代理魔像 (golem agent) 在工作流程中皆受縛於特定職責，替你完成工作。此即命名為 Golem-Agents-Legion 的由來。

## 什麼是 GAL

- **儲存庫自有狀態** (`.dev/`)：取代受限於供應商的對話記憶。實現跨 AI 工具的脈絡持久化。
- **統一合約**：透過控制平面的操作介面 (`/gal …`) 與專門代理程式強制在五種執行環境中維持一致的工作流程。

## 為什麼選擇 GAL

- **架構層級的跨供應商設計**：規劃、實作、測試與稽核階段透過 `/gal pipeline` 派送給不同供應商的代理程式。REFINE-LOCK 迴圈收斂計畫。跨模型驗證是結構性的，非選用功能。
- **Rust 原生協調**：`gal` 二進位檔以編譯式程式管理協調、狀態投影與轉接器生成。跨供應商的脈絡傳遞所引入的結構性開銷低於純提示詞 (prompt-only) 架構，但此特性與每次 pipeline 執行的整體 token 消耗量無關。
- **成本控制**：各階段的執行模型可透過 `config.json#executorRouting` 進行設定。
- **無供應商綁定**：工作流程維持標準化且獨立於特定 AI 供應商。可隨時切換供應商，無須重新架構專案狀態。
- **狀態連續性**：執行狀態直接記錄至 `.dev/`。中斷的管道 (pipeline) 執行可從確切的記錄中斷點續行。

## 快速開始

主要 GAL 方法採用**安裝模式**。無須複製此儲存庫。

**先決條件：** 必須事先設定好一個支援的 AI 編碼執行環境 (coding runtime)。包含 Claude Code、Codex CLI、GitHub Copilot、Antigravity CLI 或 opencode。

1. **安裝 `gal` CLI：**

   ```bash
   curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
   ```

   ```sh
   irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
   ```

   包含 Homebrew、winget 與 cargo 在內的其他安裝方式詳列於[使用者手冊](../../manual.md#installing-gal)中。

2. **專案初始化**：在目標專案中開啟支援的執行環境並執行 `/gal init`。Codex 使用 `$gal init`。此動作會建立 `.dev/` 目錄並生成各代理程式的轉接器檔案。

3. **工作流程執行**：執行 `/gal status` 以確認儲存庫初始化並檢視後續步驟。執行 `/planning` 將請求轉換為初步計畫。

這些步驟代表通往功能完整且已初始化儲存庫的完整路徑。此基線狀態不需要選用的整合功能。後續內容提供隨需查閱的參考資料。

### 功能生命週期

```text
/gal init
   ↓
┌── Planning Phase ──────┐
│ /planning              │
│    ↓                   │
│ /deep-planning         │
│    ↓                   │
│ [OQ-completion gate]   │
│    ↓                   │
│ /refining-plan         │
│    ↓                   │
│ [human approval gate]  │
│    ↓                   │
│ /plan-to-prompt        │
└────────────────────────┘
   ↓
/gal pipeline
   ↓
/gal finalize
```

`/gal init` 指令會在完成前述步驟後為儲存庫建立一次性的骨架。後續節點在 [GAL 運作機制](#gal-運作機制) 中具有專屬區塊。規劃階段位於[規劃階段](#規劃階段)。執行位於 [Pipeline 管道](#pipeline)。落地位於 [Finalize 落地](#finalize)。

## 檔案架構

GAL 檔案存放於兩個不同位置。持久的儲存庫自有狀態存在於**專案資料夾內**。機器本機執行環境檔案存在於**主資料夾（`~`）內**。

**專案資料夾內容** —— 由 `gal init` 建立的持久、可審查且可比對差異的工作流程狀態：

```text
.dev/
├── project.md                 compressed project summary (cold-start first read)
├── state.md                   active plans index + session continuity
├── plans/<slug>.md            human-readable source plan (transient)
├── plans/<slug>.prompt.md     AI execution work file (mutable task memory)
├── plans/<slug>.en.md         EN semantic draft for non-English planLanguage (tracked, see manual)
└── research/                  research work files (non-durable, findings are promoted to docs/)
CLAUDE.md · AGENTS.md · GEMINI.md · .github/copilot-instructions.md · .agents/rules/gal.md
                               generated per-agent adapters (regenerate with `gal init --force`)
```

**主目錄內容** —— 機器本機執行環境元件，嚴格排除作為事實來源：

```text
~/.gal/
├── config/config.json         user-owned machine config: personalization, secrets, executorRouting — preserved across reinstalls
├── plugins/gal/               GAL-managed canonical plugin root (the runtime content owner)
├── generated/                 GAL-produced projections, rebuildable
├── state/plugins.lock.json    projection registry lockfile
└── active/<runtime>/          stable shortcut targets for AI tools
```

包含 `~/.claude/skills/gal` 與 `~/.copilot/skills/` 在內的供應商可見介面運作為標準根目錄投影。它們並非次要事實來源。完整的佈局文件存在於 [docs/architecture.md → Runtime Topology](../../architecture.md#repository--runtime-topology)。

私人筆記作為獨立的、使用者自有的邊界運作。這代表一項選用的個人化強化 (Personal Enhancement) 功能。預設為停用並在 `~/.gal/config/config.json` 內進行設定。GAL Core 在無此功能的情況下仍維持完整功能且嚴格避免假設特定的筆記佈局。此功能構成一項選用的能力而非協作工具。約束性合約請參考 [optional-capabilities.md](../../../plugins/gal-core/conventions/optional-capabilities.md)。設定指示請參考 [docs/manual.md](../../manual.md)。

## GAL 運作機制

### 指令

| 系列 | 指令 |
| --- | --- |
| 控制平面 | `/gal` (單獨執行將自動偵測下一步) 加上子指令 `init · status · whats-next · wrap-up · pipeline · finalize · research · deep-research` |
| 代理程式路由 | `/gal <golem-name>` (可直接呼叫的 golem) 與 `/gal discuss <role>` (脈絡內諮詢) |
| 規劃 | `/planning` · `/deep-planning` · `/refining-plan` · `/plan-to-prompt` |
| 獨立技能 | `/adversarial-review` (涵蓋計畫、差異、文件或決策的對抗性審查)、`git-commit-msg` (依據暫存變更產生 Conventional Commit 訊息),以及 `/text-flowcharts` (將分支邏輯繪製成等寬純文字決策樹圖) |

Codex 執行環境將使用 `$` 字首的相同名稱作為技能呼叫。例如使用 `$gal status` 而非 `/gal status`。完整的合約細節存在於 [plugins/gal-core/commands/commands.md](../../../plugins/gal-core/commands/commands.md)。

### Golem Agents

十個專門化的代理程式分為兩類：可直接呼叫與僅由協調器驅動。可直接呼叫的代理程式包含 `architect`、`analyst`、`designer`、`releaser`、`debugger` 與 `steward`。你可以在規劃階段與討論中透過 `/gal <role>` 呼叫它們。

完整的能力表、可呼叫性矩陣與雙模式細節位於 [docs/manual.md → Golem Agents](../../manual.md#golem-agents)。權威名冊位於 [agents.md](../../../plugins/gal-core/agents/agents.md)。工作流程合約位於 [coding.md](../../../plugins/gal-core/workflows/coding.md)。

### 規劃階段

- `/planning`：根據需求建立來源計畫 (source plan)。檔案會存放在 `.dev/plans/<type>-<slug>.md`。
- `/deep-planning`：執行先發散後收斂的架構審查。此流程包含必要的 architect 審查、視情況觸發的 analyst 與 designer 審查以及 steward 的計畫結構檢查。若涉及受保護路徑或結構性變更，此步驟為強制必備。完成後會輸出 `ARCH_REVIEW: CLEAR`。**嚴格規定：使用者在啟動 `/refining-plan` 之前必須先解決所有待釐清的問題。**
- `/refining-plan`：將實作合約寫入來源計畫中。計畫內的 `## Tasks` (T-NN) 與 `## Test Plan` (TP-NN) 區塊會在 architect 與 tester 的 Definition-of-Ready 驗證關卡中反覆迭代直到收斂。最終輸出 `ENG_REVIEW: CLEAR`。
- 所有的審查關卡均採用對抗性審查流程。此機制嚴格要求使用強化論證與預設反駁邏輯。必須給出明確的 APPROVE、REVISE 或 REJECT 裁決。當裁決為 REVISE 時將觸發硬性阻擋。若需要單獨使用此套審查方法，可透過 `/adversarial-review` 指令執行。
- 使用者在 `## Approval` 區塊留下的核准紀錄將作為確認草案的最後一道人工審查程序。必須取得這項明確的核准，`/plan-to-prompt` 指令才能產生 `.dev/plans/<slug>.prompt.md` 執行工作檔。

完整的規劃階段語意與細節位於 [plugins/gal-core/workflows/coding.md](../../../plugins/gal-core/workflows/coding.md#stage-35--definition-of-ready-gate-refine-lock-loop)。

### Pipeline

```text
per task (T-NN):

  ORCHESTRATOR (dispatches each phase, checks each return)
       |
       |dispatch task T-NN
       ↓                      ┌──( >3 )──► STOP (handoff)
  CODER (implement) ◄─────────┤
       ↓                      │ re-dispatch
  TESTER (unit test) ──FAIL───┤ implement
       ↓ pass                 │ ( fix counts ≤ 3 )
  AUDITOR (review) ──REJECT───┘
       ↓ pass
  ORCHESTRATOR (commit + 3-surface converge)
       ↓
  next task ↺

all tasks done:

  ORCHESTRATOR (goal-backward verify, in-process)
       ↓
  VERIFIED → /gal finalize
```

在執行期間，管道 (pipeline) 的每一個階段都會派送給 `config.json#executorRouting` 所定義的路由執行器 (executor)。這讓 implementer、tester 與 auditor 角色能使用**不同供應商的編碼代理程式 (coding agent)**。此設計確保獨立的跨模型驗證。

若單一任務連續三次驗證失敗，管道執行將會中止並交由使用者處理以防止陷入無限迴圈。正確性的驗證關卡由協調器 (orchestrator) 負責把關。`golem-auditor` 則負責管理每項任務的深度效能與安全性稽核。

使用者可透過 **SSH** 遠端跨機執行任務。操作規格請見 [docs/manual.md](../../manual.md#remote-execution-ssh-dispatch-lane)。完整工作流程語意詳見 [plugins/gal-core/workflows/coding.md](../../../plugins/gal-core/workflows/coding.md)。

### Finalize

- `/gal finalize` 指令作為薄協調器負責排程執行具破壞性的結案落地。為落實零信任的再次驗證機制，其執行嚴格受限於兩道前置關卡。必須取得通過驗證的 full-mode `gal finalize-check` 收據 (receipt)，且由 `ORCHESTRATOR` 執行的目標反推驗證必須成功。完整的執行流程包含：
  - **整體性審查**：針對所有合併後的任務進行一次全分支的整體評估。
  - **文件檢驗 (Doc-sync)**：由 `STEWARD` 將持久性知識萃取至 `docs/` 並建立嚴格的 commit hash 驗證關卡。
  - **合併、刪除與關閉**：執行 `git merge` 將分支合併至 main（若有需要）。清除所有工作樹。由 `ORCHESTRATOR` 執行計畫檔案刪除以結束其生命週期。只有在確實要交付產出物時才會執行釋出。

`/gal wrap-up` 指令的作用則完全不同。它作為非破壞性的工作階段**暫停**功能。可隨時呼叫來保存中途的進度與連續性，且不會觸發任何落地或結案動作。

### 研究

此工作流程獨立於 Coding Flow。可與開發作業並行或是作為獨立的調查任務執行。

- `/gal research` 指令會依序執行 RESEARCH → VERIFY → DOCUMENT 序列。
- 針對不確定性較高的主題，`/gal deep-research` 指令會在 VERIFY 階段前加入 SYNTHESIZE 與 CROSS-REVIEW 步驟。此指令要求至少須嘗試查詢五筆來源。

VERIFY 階段要求使用獨立的模型（必須與產出研究結果的模型不同）針對所有引用的參考資料進行反向查證。研究結果預設會儲存於 `.dev/research/` 以優先保留儲存庫自有的考證紀錄。若要將結果導向至個人的外部筆記工具，則必須選擇並啟用對應的機器本機筆記 (local-notes) 後端。完整的規範細節請參閱 [plugins/gal-core/workflows/research.md](../../../plugins/gal-core/workflows/research.md)。

## 整合工具

選用的外部工具強化特定的作業路線。**GAL 避免自動安裝並在無這些工具的情況下維持完整功能**。整合處理透過一個共用的五狀態預檢序列涵蓋適用性、可用性、初始化狀態、整備度、路由與降級。

| 工具 | 功能 | 文件 |
| --- | --- | --- |
| codebase-memory-mcp | 執行即時基於 MCP 的結構與符號查詢以在原生檔案偵測之後精煉定位 | [→](../../integrations.md#codebase-memory-mcp) |
| graphify | 將儲存庫檔案轉換為知識圖輸出至 `graphify-out/`。GAL 在規劃與審查期間取用此資料以識別跨模組耦合 | [→](../../integrations.md#graphify) |
| OpenCLI | 利用作用中的已登入工作階段將網站、瀏覽器工作階段、Electron 應用程式與本機工具轉換為可重複使用的 CLI 指令 | [→](../../integrations.md#opencli) |
| Playwright MCP | 提供使用者連接的瀏覽器能力以支援測試執行、設計稽核、動態頁面研究與瀏覽器可見的 MCP 評估 | [→](../../integrations.md#playwright-mcp) |

全面的整合細節、設定指示與路由合約位於 [docs/integrations.md](../../integrations.md)。

## 相關文件

此區塊為**文件職責邊界的單一權威**。每份頂層文件具有單一固定職責。若內容不符合某文件的列述，則應歸屬於符合其列述的文件。應移動內容而非重複建立。沒有其他文件能重新定義這些邊界。它們一律連結至此處。

| 文件 | 職責 | 解答的問題 |
| --- | --- | --- |
| [README](README.zh-Hant.md)（本檔） | 公開進入點：GAL 是什麼、為什麼存在、快速開始、工作流程概觀 | 「這是什麼？我該如何開始使用？」 |
| [使用手冊](../zh-Hant/manual.zh-Hant.md) | 終端使用者操作：安裝、首次執行、工作流程、設定、個人化、執行器、golem | 「我該如何進行 GAL 的日常操作？」 |
| [整合工具](../zh-Hant/integrations.zh-Hant.md) | 每個選用整合佔一個區塊 | 「GAL 如何與工具 X 協作？」 |
| [contributing](../../../CONTRIBUTING.md) | 貢獻者進入點：關卡、工作流程骨架、閱讀順序 | 「我想貢獻程式碼——第一步是什麼？」 |
| [architecture](../../architecture.md) | **系統內容與原因，以圖表呈現。** 結構、擁有權地圖與決策紀錄。圖表承載說明，行文提供輔助。使用者可閱讀此文件了解 GAL 的組合方式，不限於開發者 | 「這是如何組合起來的？為什麼是這個形狀？」 |
| [developer guide](../../devguide.md) | **如何修改它**：程序、內部迴圈、操作。行文與步驟清單，絕非結構的第二份複本。**僅限開發者** —— 假設讀者已簽出原始碼並有意修改 GAL | 「我想修改 X —— 步驟是什麼？」 |
| [naming](../../naming.md) | 每個核心術語的語意權威，以及退役術語的關卡輸入 | 「這個詞在這裡是什麼意思？」 |
| [plugins/gal-core/commands/commands.md](../../../plugins/gal-core/commands/commands.md) · [plugins/gal-core/agents/agents.md](../../../plugins/gal-core/agents/agents.md) · [plugins/gal-core/workflows/coding.md](../../../plugins/gal-core/workflows/coding.md) | 定義標準控制平面操作、代理程式職責與工作流程合約 | 「合約是什麼？」 |

**新貢獻者的閱讀順序：** `CONTRIBUTING.md`（起點） → `docs/architecture.md`（理解） → `docs/devguide.md`（修改）。

**兩份開發者導向文件的受眾區分。** `architecture.md` 以圖表為主，開放給所有讀者。無意修改程式碼的使用者也能閱讀以了解 GAL 對其機器的影響與原因。`devguide.md` 以程序為主，假設讀者已簽出原始碼並準備修改 GAL。新增內容時請詢問讀者屬於哪一類。欄位層級的 schema 表格、測試檔案位置或重建指令，即使描述結構，也應歸屬於 `devguide.md`。

`plugins/gal-core/` 下的標準合約永遠覆蓋任何文件的摘要。文件格式、命名與翻譯政策屬於獨立關注點，由 [devguide → Documentation Conventions](../../devguide.md#documentation-conventions) 負責管理。

## 參考資料

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)
- [rtk](https://github.com/rtk-ai/rtk)：一個 CLI 代理層 (proxy)。設計用於在開發指令輸出 (包含 git、cargo 與測試執行器) 抵達模型脈絡之前先行過濾並壓縮。此工具獨立於 GAL 整合而存在。在無 GAL 偵測或相依的情況下運作。強烈建議與 GAL 搭配使用。管道執行在每個任務會觸發許多 shell 指令。修剪輸出能有效最大化執行環境脈絡預算。

## 授權條款

採用 MIT 授權條款。請參考 [LICENSE](../../../LICENSE)。
