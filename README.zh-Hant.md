# Golem Agents Legion (GAL)

[English](README.md) | 繁體中文

現在的 AI 工具（如 GitHub Copilot, Gemini CLI, Claude Code）更替速度很快，很多設定像是 skills/agents 等僅可於特定工具內使用，工作進度也難銜接，容易遺失先前的對話脈絡與規劃。此外，複雜的專案也常因為缺乏明確的狀態管理，導致開發進度難以追蹤。因此設計了此系統以同步各個工具間的設定及工作狀態，並加速整個開發流程。

GAL 是一套為開發工作帶來結構化流程的 AI 工作系統。它將你的「開發計畫」、「目前狀態」與「審查紀錄」全部儲存在專案本地的 Markdown 檔案中（`.dev/` 與 `docs/`）。無論你今天用哪一個 AI CLI 工具開啟專案，都能無縫接續昨天的工作。系統核心包含 12 個專職的 Golem Agent 與 `/gal` 控制平面，以文件驅動的開發模式運作。

持久化的狀態資料分為兩個儲存邊界：儲存庫共享的狀態以本機 Markdown 檔案形式存放。使用者的個人筆記則可選擇寫入自行設定的 Obsidian Vault，保留儲存庫之外的私人筆記空間。狀態管理機制參考了 Get Shit Done (GSD) 的階段式準則，讓 `/gal status` 和 `/gal whats-next` 能夠完整呈現專案目前的工作進度。

若有多個電腦設備，你也可使用 xmachine 能幫你把 AI 任務透過 SSH 路由到遠端工作節點執行，以最大化資源利用率。（需自行先設定完 SSH 連線、Zellji、ai cli 工具）

## 前置需求 (Prerequisites)

在開始使用 GAL 之前，請確保你的環境具備以下條件：

1. **終端機環境**：必須具備 Bash (macOS/Linux/Git Bash) 或 PowerShell (Windows)。
2. **AI CLI 工具**：必須安裝至少一款支援的 AI 指令列工具（如 GitHub Copilot CLI, Gemini CLI, Codex CLI, 或 Claude Code）。
3. **基礎工具**：確保已安裝 Git，以便進行版本控制與狀態追蹤。

## 快速開始

1. **複製（Clone）此專案**：
   `git clone https://github.com/monkey1wizard/golem-agents-legion.git`

2. **安裝與設定**：
   進入專案目錄後執行安裝腳本。Windows 使用 `./scripts/Setup-Machine.ps1`，macOS/Linux 使用 `./scripts/setup-machine.sh`。安裝完成後就能在你的儲存庫中使用 GAL 指令。

3. **啟動 GAL**：
   進入你的目標儲存庫（Target Repository）後開啟你偏好的支援 runtime，然後用該 runtime 慣用的 command 或 skill 入口呼叫 GAL。

   ```text
   # 常見斜線指令介面
   /gal init

   # Codex CLI（技能提及介面，使用 $ 前綴，不是 /）
   $gal init
   ```

   各 runtime 的入口差異請看 `scripts/scripts.md` 與 `docs/devguide.md`。

## 範例：用 GAL 跑完一個功能的生命週期

**情境：如何在專案中新增一個 JWT 登入功能？**

1. **初始化專案**：在專案內輸入 `/gal init`，建立基礎狀態檔。
2. **發想與規劃**：輸入 `/planning` 並告訴 AI「我要做一個 JWT 登入功能」。AI 會與你討論並將規格寫入 `docs/plans/`，再使用 `/deep-planning` 仔細審核規劃書。
3. **鎖定規格**：輸入 `/refining-plan` 與 `/plan-to-prompt`，讓 AI 將人類可讀的規格轉化為 AI 可執行的「任務清單」與「測試計畫」。
4. **自動實作與驗證**：輸入 `/gal pipeline`，GAL 會自動指派 Implementer（寫程式） -> Tester（寫測試） -> Reviewer（審查程式碼）。
5. **收工**：輸入 `/gal wrap-up` 紀錄今天進度。明天換別的 AI 工具開啟專案，依然能無縫接續！

## 指令

| 指令 | 用途 |
| --- | --- |
| `/gal init` | 初始化儲存庫：建立 `.dev/project.md` 與 `.dev/state.md` |
| `/gal status` | 完整狀態呈現：活動企劃、審查/測試狀態、阻擋點、連續性 |
| `/gal whats-next` | 推薦單一下一步動作 |
| `/gal wrap-up` | 收斂工作：寫入 `### Handoff Notes` 與 `## Session Continuity` |
| `/gal research` | 進入研究工作流 |
| `/gal deep-research` | 進入多來源研究工作流，包含交叉審查 |
| `/gal pipeline` | 逐任務自動串接 implementer → tester → reviewer，若變更涉及安全性敏感面（security-sensitive surface）則插入條件式 `golem-security` 審查，最後再由 verifier 收尾 |
| `/planning` | 建立規劃文件（source plan） |
| `/deep-planning` | 把規劃文件收斂到可實作 |
| `/refining-plan` | 把 `## Tasks`、`## Test Plan` 與工程審查結果寫入規劃文件 |
| `/plan-to-prompt` | 產生執行工作檔（execution prompt） |

## 開發工作流

```text
         init
          │
          v
       planning
          │
          v
     source plan (docs/plans/*)
          │
     ┌────┴──────────────┐
     │ 依內容需要時       │ 若使用 deep-planning
     │ 直接啟用           │ architect 必定啟動
     │ analyst/designer  │ analyst/designer
     │ 進行審查           │ 依內容併行啟動
     └────┬──────────────┘
          │ 
          v
     refining-plan
          │ 
          v
     plan-to-prompt
          │
          v
     gal-pipeline
```

### planning

`/planning` 會把新的需求整理成正式的規劃文件（source plan），寫入 `docs/plans/<plan-slug>.md`。

這個階段的重點是把你的目標、需求等內容整理成穩定的人類可讀文件，並在 `## Open Questions` 記錄尚未定案的項目。`/planning` 不會建立執行工作檔，它只決定目前的規劃文件是否應該先進入 `/deep-planning` 做進一步收斂與架構審查，或可以直接進入 `/refining-plan` 鎖定實作契約。

### deep-planning

`/deep-planning` 會對既有規劃文件進行深度規劃，且必定會啟動 architect 審查。規劃材料會整理回同一份 `docs/plans/<plan-slug>.md`，並把架構審查結果寫回規劃文件的 `## Review Results > ### Architecture Review` 與 `## Approval > Architect review`。若架構審查仍有阻擋問題，工作就停留在 deep-planning 繼續修正。只有在規劃文件已經收斂到足以進入執行階段時，才往下進入 `/refining-plan`，再進入 `/plan-to-prompt`。

若規劃涉及商業邏輯、定價、權限、通知、新手引導（onboarding）或身分驗證，analyst 會與 architect 同時啟動。若涉及客戶接觸面（customer-facing flows）、版面配置、狀態、元件（components）或無障礙設計，designer 也會一併加入。如果只需要商業或設計層面的專家審查，不打算進行完整架構檢視，也可以直接針對規劃文件啟用對應的 Domain Lane。

### refining-plan

`/refining-plan` 完成 prompt 執行前必須定案的三個關鍵章節：`## Tasks`（列出 `T-NNN` 任務清單）、`## Test Plan`（建立與任務對應的 `TP-NNN` 測試矩陣），以及 `## Review Results > ### Engineering Review`（標記為 CLEAR (`<!-- ENG_REVIEW: CLEAR -->`) 或 BLOCKING 並說明阻擋問題）。這個步驟透過執行細部設計來銜接 planning 與 `/plan-to-prompt` 之間的流程。`/refining-plan` 不涉及程式碼實作、測試執行或 `## Status` 變更。若已安裝 gstack，也可改用其 `plan-eng-review` 替代。

### plan-to-prompt

`/plan-to-prompt` 會把規劃文件依照範本轉換成 `.dev/plans/<plan-slug>.prompt.md` 供執行階段使用，並更新狀態。完成後可執行 `/gal status` 或 `/gal whats-next`，系統會掃描執行工作檔並回答你。產生執行工作檔後，就能正常執行 `/gal pipeline`。若規劃文件在轉換後又有變更，應重新執行 `/plan-to-prompt`，以確保規劃文件與執行工作檔內容保持一致。

## Golem Agents

GAL 的核心是 12 個專門化 agent，各自有獨立的 `.agent.md` 定義檔。分職而立的設計原則：

- **prompt 精簡**：每個 agent 只載入自己的職責定義，不浪費脈絡視窗（context window）
- **獨立性**：獨立的 tester / reviewer / verifier，以確保驗證結果的可信度
- **可組合**：按任務風險等級決定啟用哪些 agent，不是一體全開

除了使用 `/gal` 指令以外，你也能直接呼叫 `golem-` 進行指定類型的工作。

### 分類

| 分類 | 啟動方式 | 成員 |
| --- | --- | --- |
| **Utility** | 任何時候直接呼叫 | debugger、notewriter |
| **Domain** | 由指令或使用者直接諮詢 | architect、analyst、designer、researcher、security、releaser |
| **Pipeline** | 由 `/gal pipeline` 自動串接 | implementer、tester、reviewer、verifier |

### Utility Agents

| Agent | 職責 |
| --- | --- |
| **debugger** | 科學方法 bug 調查：假說、驗證、根因確認後才修 |
| **notewriter** | Obsidian 寫入總入口：私人研究擷取（research capture）、工作日記、收件匣（inbox）、知識萃取、收工儀式（shutdown ritual） |

### Domain Agents

Domain agents 提供專業諮詢，可以在任何階段被使用者或指令調用。

| Agent | 職責 |
| --- | --- |
| **architect** | 對抗式的規劃審查：權衡分析、過度設計偵測、bug 表面區域、公開 API 風險 |
| **analyst** | 商業邏輯審查：ROI、領域（Domain）正確性、使用者影響 |
| **designer** | 設計系統建立、視覺探索、design-to-code 建置、即時 UI 稽核 |
| **researcher** | 本機優先的研究與結構化統整，帶有來源歸屬（source attribution） |
| **security** | 實作階段的 OWASP 與 STRIDE 安全性審查 |
| **releaser** | 發布準備（release prep）、部署編排（deploy orchestration）、文件同步 |

### Pipeline Agents

Pipeline 是 GAL 的自動化執行核心。它的固定主鏈仍是四個 agent，但若實作後的變更觸及安全性敏感面，`/gal pipeline` 會在 task closeout 前插入條件式 `golem-security` 審查。

```text
T-NNN ──> implementer ──> tester ──> reviewer ──> [conditional security] ──> git commit ──> T-NNN+1
               ↑                         │
               │                         ↓
     auto-fix by review result <────── REJECT

所有任務完成後：──> verifier ──> 確認規劃目標達成
```

`[conditional security]` 代表只有在變更觸及身分驗證、敏感資料處理、輸入處理、公開 API 介面，或部署/環境信任邊界時，才會啟動 `golem-security`。

| Agent | 職責 | 關鍵規則 |
| --- | --- | --- |
| **implementer** | 依照規劃與當前 `T-NNN` 任務完成實作 | 一旦觸及架構邊界或發現規劃不足，必須停止並返回 `/deep-planning` |
| **tester** | 根據規格與公開 API 撰寫或補齊測試，必要時執行瀏覽器 QA | spec mode 不讀實作，且必須與 implementer 使用不同模型 |
| **reviewer** | 以資深工程師的標準審查變更差異、風險與完整性 | 若發現阻擋問題，必須退回 implementer 修正。使用之 AI 模型應不同於 implementer，且能力不應弱於 implementer |
| **verifier** | 在所有任務完成後，從規劃目標反向驗證成果是否真的達成 | 負責確認規劃是否可關閉，並把值得保留的知識抽回 `docs/` |

`golem-security` 屬於 domain agent，由 `/gal pipeline` 在安全性敏感變更時有條件啟動，不參與常態執行。Domain 與 utility agent 可隨時直接呼叫，pipeline agent 亦可在明確界定的工作中直接呼景呼叫。

### AI 模型與 Agent 規則

Pipeline 流程中，GAL 強制以不同模型進行審查與測試：

- Tester **必須**與 implementer 使用不同模型
- Reviewer **應**與 implementer 不同，能力不應弱於 implementer
- Planning 與 architect **應盡量**不同

上述規則在 `model-roles.local.md` 中設定。

### Working Hours

Working Hours 改為 **預設關閉** 的本機設定。只有當使用者在 `config.local.env` 啟用後，agent 才會依照設定的工作時段、After Hours、Wrap-up Time 與 Hard Stop 執行提醒與停工。

- **Working Hours off**：所有 agent 正常工作
- **After Hours**：超過工作時段後，到 Wrap-up Time 前仍可工作
- **Wrap-up Time**：若當日日記未寫，非 `notewriter` agent 會阻擋並引導進入收工儀式
- **Hard Stop**：所有 agent 停止工作，包括 `notewriter`
- **Override**：使用者可說 `override working hours`，單次有效

可至 `config.local.env` 裡面變更 `WORKING_HOURS_ENABLED`、`WORKDAY_START`、`WORKDAY_END`、`WRAP_UP_TIME`、`HARD_STOP_TIME`以設定工作時段，詳情請見 [docs/personalization.zh-Hant.md](docs/personalization.zh-Hant.md)。

## 儲存邊界

GAL 將持久化資料分為兩個儲存邊界：

- **儲存庫共享狀態**：`.dev/`、`docs/plans/`、`docs/research/`。這些檔案受 Git 管理，適合需要和儲存庫一起追蹤、審查與協作的工作成果。
- **使用者私人筆記庫**：Obsidian Vault。其位置由 `config.local.env` 的 `OBSIDIAN_VAULT` 與 `OBSIDIAN_VAULT_NAME` 設定，並可再透過 `OBSIDIAN_PRIVATE_RESEARCH_DIR`、`OBSIDIAN_DIARY_DIR`、`OBSIDIAN_ARCHIVE_DIR` 指定細部路徑。

若是 GAL 這個 source repo 本身，`.dev/` 內部要再細分：只有 `.dev/project.md` 應進 Git。`.dev/state.md`、`.dev/plans/` 與其他 `.dev/*` 工作流暫存檔都應視為 contributor 本機狀態。

若使用者設定了 `OBSIDIAN_GUIDE_PATH` 且 Guide 存在，`notewriter` 會依該 Guide 工作。若未設定或找不到，則改走 generic mode，而不會因缺少 Guide 而中止。

## 專案檔案 (Project files)

| 路徑 | 用途 |
| --- | --- |
| `.dev/project.md` | 儲存庫摘要、技術堆疊（Tech Stack）、目標、限制 |
| `.dev/state.md` | 活動規劃索引、阻擋點、工作階段連續性 |
| `.dev/plans/<plan-slug>.prompt.md` | AI 執行工作檔 |
| `CLAUDE.md` | 儲存庫本機操作備註 |
| `DESIGN.md` | 儲存庫層級設計治理 |
| `docs/designs/<plan-slug>/` | 規劃綁定的設計資產 |
| `docs/plans/<plan-slug>.md` | 人類可讀的規劃文件 |
| `docs/research/` | 儲存庫共享研究報告（預設 research 輸出） |

### 執行工作檔

當你執行 `/plan-to-prompt` 之後，GAL 會建立 `.dev/plans/<plan-slug>.prompt.md`。你可以把它想成這個任務的工作看板。第一次使用時，主要只要看兩類資訊：

- `## Status > Workflow`：目前做到哪一個階段
- 其他回寫區段：規劃、測試、審查、交接的結果寫在哪裡

一般情況下，你不需要手動改 `Workflow:`。它會在建立執行工作檔時先設成 `DRAFT`，之後隨著工作的執行自動往下推進。

#### 工作狀態

| 狀態 | 代表什麼 |
| --- | --- |
| `DRAFT` | 執行工作檔剛建立，還沒正式進入任務執行 |
| `IMPLEMENT` | 正在做某個 `T-NNN` 任務的實作 |
| `TEST` | 實作已完成，正在測試 |
| `REVIEW` | 測試已通過，正在做程式碼審查（Code Review） |
| `REVIEW — 發現 N 個阻擋問題` | 審查發現阻擋問題，必須修正後再重跑 |
| `ABSORBED` | 已確認目標達成，準備關閉企劃 |

#### 回寫區段

| 區段 | 你會在什麼時候看它 | 它的用途 |
| --- | --- | --- |
| `## Open Questions` | 還有需求、假設、邊界沒定案時 | 集中列出尚未解決的問題 |
| `## Tasks` | 要知道這個企劃實際要做哪些事時 | 任務清單，也是 pipeline 逐步執行的依據 |
| `## Analyze` | 想確認目前變更是否仍在原本企劃範圍內時 | 記錄 reviewer 對變更是否偏離企劃範圍 |
| `## Review Results` | 想看審核結果時 | 集中放 reviewer、designer、security 等審核結果 |
| `## Test Plan` | 還沒開始測試，想知道應該測什麼時 | 記錄預計驗證的測試範圍 |
| `## Test Results` | 測試或 browser QA 跑完之後 | 記錄測試結果 |
| `### Handoff Notes` | 中途停下來，想知道上次做到哪裡時 | 提供下次接手時的上下文（Context） |
| `## Release` | 準備合併、部署或同步文件時 | 記錄 release 階段結果 |

`/gal status` 和 `/gal whats-next` 主要就是讀這些已回寫的內容，來判斷目前進度與下一步，而不是只看單一欄位。

## 研究工作流

研究是獨立於開發的工作流程，可以和開發工作流平行運作。預設 durable 輸出仍寫到 `docs/research/`，但你也可以在研究過程中明確指定要存到私人筆記區或只回傳結果不落地。

| 模式 | 適用時機 | 流程 | 來源要求 |
| --- | --- | --- | --- |
| `/gal research` | 標準結構化調查 | RESEARCH → VERIFY → DOCUMENT | 足夠回答問題即可 |
| `/gal deep-research` | 高風險、高模糊度或跨主題調查 | RESEARCH → SYNTHESIZE → CROSS-REVIEW → VERIFY → DOCUMENT | 至少嘗試 5 個來源 |

兩種模式都強制要求 VERIFY 由**不同於研究作者的 model** 執行。`deep-research` 的 CROSS-REVIEW 是來源間一致性審查，不是架構或商業審核。缺口類型決定回退目標：

```text
IDLE → RESEARCH → SYNTHESIZE → CROSS-REVIEW → VERIFY → DOCUMENT → DONE
         ↑            ↑             ↑             │
         └─ evidence ─┴─ synthesis ─┴─ source/ref ┘
```

DOCUMENT 階段的目標地有四種：

- `repo`：寫到 `docs/research/`，這是預設值
- `private`：寫到 `OBSIDIAN_PRIVATE_RESEARCH_DIR`
- `knowledge`：交給 `notewriter`轉成可重用的長期知識筆記
- `none`：只回傳結果，不做 durable write

## 協作工具

GAL 支援多種協作工具（collaborative tools），主要分為兩大類：

- **工作流強化**：如 `graphify`、`gstack`、`OpenCLI` 等，用於強化特定查詢能力或 agent skills。
- **Pipeline 與執行環境**：如 `xMachine`、`Blender pipeline` 等，用於跨平台任務執行或專業資產管線。

**重要聲明**：所有的協作工具都**需要使用者自行安裝**相應的特定工具後才能正常運作。**GAL 不會幫忙安裝這些工具**。即使未安裝任何協作工具，GAL 的核心流程仍能完整運作。

### 啟動前檢查機制

所有協作工具在使用前都走同一套啟動前檢查機制：

```text
applicability → availability → initialization status → readiness → route / degrade
```

| 狀態 | 意義 |
| --- | --- |
| `not-applicable` | 目前工作流程或任務不需要此工具，直接跳過 |
| `unavailable` | 機器無法存取此工具，或是無法執行，走備援方案 |
| `available-but-needs-init` | 工具存在但尚未完成首次設定，不在正常流程中自動初始化 |
| `available-but-not-ready` | 已安裝且已初始化，但當前儲存庫或任務缺少所需產物 |
| `ready` | 適用且所有前置條件滿足，進入工具能力 |

核心行為規則：不主動安裝、不主動初始化、不以模糊成功語言掩蓋缺失。每個工具啟用的流程都有明確的降級路線。完整規格見 [docs/collaborative-tools/checking-contract.md](docs/collaborative-tools/checking-contract.md)。

### graphify

圖形資料結構工具。它會將資料夾內的所有檔案進行圖形化分析，產出的檔案放置於 `graphify-out/`，能加強後續 AI 的查詢能力。GAL 只會在儲存庫已經存在 `graphify-out/GRAPH_REPORT.md` 等 graphify 產物時使用它。`gal init` 不會自動產生這些檔案。`setup-tools`、`/gal status`、`/gal whats-next` 可以檢查既有 stamped report 是否仍與目前安裝的 graphify 版本一致，但若 repo 沒有 graphify 產物，GAL 仍會照常走非 graphify 流程。詳見 [docs/collaborative-tools/graphify.md](docs/collaborative-tools/graphify.md)。

### OpenCLI

把網站、瀏覽器工作階段、Electron 應用程式與本機工具轉換成命令列介面（CLI）。你可以重用已登入的瀏覽器、把即時操作流程自動化，並把重複動作整理成可重複使用的 CLI 指令，詳見 [docs/collaborative-tools/opencli.md](docs/collaborative-tools/opencli.md)。

### gstack

由 Y Combinator 總裁兼 CEO Garry Tan 創建，將其新創經驗轉換成 AI agents，詳見 [docs/collaborative-tools/gstack.md](docs/collaborative-tools/gstack.md)。

### xmachine

xmachine 是 GAL 的協作執行工具，能透過 SSH 將工作任務路由到已準備好的工作節點。目前已文件化的執行 lane 包括 Windows 工作節點以及 POSIX 相容的 shell 工作節點之背景執行模式。詳見 [docs/collaborative-tools/xmachine.zh-Hant.md](docs/collaborative-tools/xmachine.zh-Hant.md)。

### Godot C Sharp

製作中。現有指令透過慣例（convention）、技能（skill）與 MCP 工具直接操作 Godot 4 C# 儲存庫。詳見 [docs/collaborative-tools/godot.md](docs/collaborative-tools/godot.md)。

### AI-First 遊戲素材

製作中。以 ComfyUI 為生成入口，搭配後段工具做整理與匯出。詳見 [docs/collaborative-tools/graphworkflow.md](docs/collaborative-tools/graphworkflow.md)。

## 個人化設定

與本機環境相關但不適合放在 README 首頁的設定都集中在 [docs/personalization.zh-Hant.md](docs/personalization.zh-Hant.md)。內容包含環境佔位符的填寫方式、執行環境的選擇與重新設定、模型路由、MCP 覆寫、Obsidian Vault 路徑、私人研究目錄、可選 Guide 路徑、Working Hours 設定，以及什麼情況下需要重新執行 setup。若你要調整本機使用的 AI 工具、模型角色對應或 MCP 設定，請看此份文件。

## 文件

`docs/` 主要用於快速閱讀與查找，`docs/collaborative-tools/` 則是協作工具與流程導引的快速入門及索引。

| 路徑 | 用途 |
| --- | --- |
| [docs/devguide.md](docs/devguide.md) | 開發者手冊 |
| [docs/personalization.zh-Hant.md](docs/personalization.zh-Hant.md) | 本機模型路由、MCP 覆寫等個人化指引 |
| [docs/collaborative-tools/checking-contract.md](docs/collaborative-tools/checking-contract.md) | 協作工具共用 preflight 檢查契約 |
| [docs/collaborative-tools/graphify.md](docs/collaborative-tools/graphify.md) | 圖形結構化工具 |
| [docs/collaborative-tools/opencli.md](docs/collaborative-tools/opencli.md) | OpenCLI 工具指引與使用時機 |
| [docs/collaborative-tools/gstack.md](docs/collaborative-tools/gstack.md) | gstack 協作工具契約：規劃與專家 agents 整合 |
| [docs/collaborative-tools/xmachine.zh-Hant.md](docs/collaborative-tools/xmachine.zh-Hant.md) | xmachine Execution Lane、所有權模型、smoke test 與 patch-first 收斂 |
| [docs/collaborative-tools/godot.md](docs/collaborative-tools/godot.md) | Godot C# 工作流導引 |
| [docs/collaborative-tools/graphworkflow.md](docs/collaborative-tools/graphworkflow.md) | AI-first 遊戲素材工作流導引 |

## 參考

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)
- [rtk](https://github.com/rtk-ai/rtk)：可過濾及壓縮傳送給 LLM 的指令輸出，減少 token 消耗，強力建議安裝。

## 授權

MIT — 詳見 [LICENSE](LICENSE)。
