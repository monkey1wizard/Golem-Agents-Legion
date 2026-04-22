# GAL 使用手冊

GAL（Golem Agents Legion）是一套以文件驅動的 AI 工作系統，適合獨立開發者在 GitHub Copilot、Gemini CLI、Codex CLI 和 Claude Code 之間保持穩定工作流程。

---

## 目錄

1. [安裝與初始化](#1-安裝與初始化)
2. [工作流程總覽](#2-工作流程總覽)
3. [指令參考](#3-指令參考)
4. [Golem Agents](#4-golem-agents)
5. [企劃工作流](#5-企劃工作流)
6. [研究工作流](#6-研究工作流)
7. [個人化設定](#7-個人化設定)
8. [儲存邊界](#8-儲存邊界)
9. [協作工具](#9-協作工具)

---

## 1. 安裝與初始化

### 1.1 環境需求

- Git
- 至少一個 AI 執行環境：GitHub Copilot（VS Code）、Gemini CLI、Codex CLI 或 Claude Code
- macOS/Linux 需 Bash；Windows 需 PowerShell

### 1.2 快速開始

**步驟一：Clone 倉庫**

```bash
git clone https://github.com/your-org/Golem-Agents-Legion.git
cd Golem-Agents-Legion
```

**步驟二：執行機器安裝腳本**

macOS/Linux：

```bash
./scripts/setup-machine.sh
```

Windows：

```powershell
./scripts/Setup-Machine.ps1
```

腳本會引導你選擇要啟用的執行環境，並安裝對應的指令與 skills。安裝狀態記錄於 `~/.gal/install-state.json`。

**步驟三：初始化 Repo**

在你要工作的 repo 根目錄執行：

```
/gal init
```

這會建立 `.dev/project.md` 與 `.dev/state.md`，並產生執行環境適配器（`CLAUDE.md`、`GEMINI.md` 等）。

### 1.3 執行環境選擇與重新設定

若需要新增或切換執行環境，重新執行安裝腳本並加上 `--reconfigure` 旗標：

```bash
# macOS/Linux
./scripts/setup-machine.sh --reconfigure

# Windows
./scripts/Setup-Machine.ps1 -Reconfigure
```

---

## 2. 工作流程總覽

GAL 的核心工作流程分為企劃、實作和研究三個主要路徑。

### 2.1 企劃到實作流程

```
/gal init
    │
    ▼
/planning          ← 建立 source plan（docs/plans/<slug>.md）
    │
    ▼
/deep-planning     ← 架構師審核，收斂為可實作狀態（可選）
    │
    ▼
/refining-plan     ← 寫入 Tasks、Test Plan 與工程審核
    │
    ▼
/plan-to-prompt    ← 產生執行 prompt（.dev/plans/<slug>.prompt.md）
    │
    ▼
/gal pipeline      ← 自動串接：implementer → tester → reviewer → verifier
```

**選擇規劃深度：**

| 情境 | 建議流程 |
| --- | --- |
| 明顯的本地修正 | 直接實作，可選 `golem-reviewer` / `golem-tester` |
| 有範圍的功能或已知原因的 bug | `/planning` → `/refining-plan` → `/plan-to-prompt` → 實作 |
| 結構性、跨切面或不確定的改動 | `/planning` → `/deep-planning` → `/refining-plan` → `/plan-to-prompt` → 實作 |

### 2.2 研究流程

```
/gal research       ← 標準：RESEARCH → VERIFY → DOCUMENT
/gal deep-research  ← 深度：RESEARCH → SYNTHESIZE → CROSS-REVIEW → VERIFY → DOCUMENT
```

### 2.3 狀態管理

- **`.dev/project.md`**：repo 壓縮摘要 + 索引
- **`.dev/state.md`**：活動企劃索引 + 跨 session 連續性
- **`.dev/plans/<slug>.prompt.md`**：執行 prompt，自追蹤進度

---

## 3. 指令參考

### 控制平面指令

| 指令 | 用途 |
| --- | --- |
| `/gal init` | 初始化 repo：建立 `.dev/` 結構與執行環境適配器 |
| `/gal status` | 完整狀態投影：活動企劃、審核/測試狀態、阻塞點、session 連續性 |
| `/gal whats-next` | 讀取 write-back 區段，推薦單一下一步動作 |
| `/gal wrap-up` | 收斂工作：壓縮上下文到 Handoff Notes，更新 state.md |
| `/gal research` | 標準結構化研究工作流（3 個狀態）|
| `/gal deep-research` | 高風險多來源深度研究（5 個狀態，含交互審核）|
| `/gal pipeline` | 逐任務自動串接所有 golem agents |

### 企劃指令

| 指令 | 用途 |
| --- | --- |
| `/planning` | 建立 source plan（`docs/plans/<type>-<slug>.md`）|
| `/deep-planning` | 架構師審核，補完 Open Questions、設計與工程問題 |
| `/refining-plan` | 寫入 Tasks、Test Plan 與 Engineering Review 到 source plan |
| `/plan-to-prompt` | 從已審核的 source plan 產生或刷新執行 prompt |

### 直接呼叫 Golem Agent

```
/gal [ask architect]       ← 諮詢模式，只讀，不寫
/gal [ask analyst]         ← 商業邏輯諮詢
/gal [run debugger]        ← 偵錯工具
/gal [golem-reviewer]      ← 程式碼審核
/gal [golem-tester]        ← 測試驗證
```

---

## 4. Golem Agents

### 4.1 三種類型

**Utility（隨時可呼叫）**

| Agent | 用途 |
| --- | --- |
| `golem-debugger` | 以科學方法系統性偵錯，必須確認根本原因才能修正 |
| `golem-notewriter` | Obsidian 寫入：工作日誌、知識提取、私人捕捉 |

**Domain（諮詢用）**

| Agent | 用途 |
| --- | --- |
| `golem-architect` | 對計畫進行對抗性審核：取捨、過度設計、bug 面 |
| `golem-analyst` | 商業邏輯審核：ROI、領域正確性、使用者影響 |
| `golem-designer` | 設計系統、UX 流程、視覺一致性審核 |
| `golem-researcher` | 深度研究、來源歸因、交叉驗證 |
| `golem-security` | OWASP Top 10 + STRIDE 安全審核 |
| `golem-releaser` | 發布準備、部署編排、文件同步 |

**Pipeline（自動串接）**

| Agent | 用途 |
| --- | --- |
| `golem-implementer` | 按照企劃執行程式碼，原子化 commit |
| `golem-tester` | 獨立撰寫測試（只讀 plan spec + 公開 API）|
| `golem-reviewer` | 審核正確性、安全性、架構合規性 |
| `golem-verifier` | 確認企劃目標達成，管理企劃生命週期結束 |

### 4.2 模型分離原則

GAL 強制以下模型分離，確保獨立驗證：

- `implementer` 和 `tester` **必須**使用不同 model
- `reviewer` **應**與 `implementer` 使用不同 model
- `reviewer` 模型層級 ≥ `implementer` 模型層級

模型對應設定於 `model-roles.local.md`（從 `model-roles.example.md` 複製後修改）。

---

## 5. 企劃工作流

### 5.1 檔案命名慣例

```
docs/plans/<type>-<slug>.md           ← source plan
.dev/plans/<type>-<slug>.prompt.md    ← execution prompt
```

類型前綴：`feat-`、`fix-`、`refactor-`、`sec-`、`perf-`、`infra-`

### 5.2 Execution Prompt 狀態值

| 狀態 | 代表 |
| --- | --- |
| `DRAFT` | 剛建立，尚未進入任務執行 |
| `IMPLEMENT` | 正在執行某 T-NNN 任務 |
| `TEST` | 實作完成，正在測試 |
| `REVIEW` | 測試通過，正在審核 |
| `REVIEW — N 個阻塞問題` | 審核發現阻塞，需修正後重跑 |
| `ABSORBED` | 目標達成，準備關閉企劃 |

### 5.3 Write-back 區段

執行 prompt 中的以下區段由各 golem 回寫，`/gal status` 與 `/gal whats-next` 讀取這些區段判斷下一步：

| 區段 | 負責 Agent |
| --- | --- |
| `## Open Questions` | 規劃階段寫入 |
| `## Tasks` | `golem-implementer` 執行 |
| `## Analyze` | `golem-reviewer` 分析 |
| `## Review Results` | `golem-reviewer` 審核結果 |
| `## Test Plan` | `golem-tester` 測試規劃 |
| `## Test Results` | `golem-tester` 測試結果 |
| `## Handoff Notes` | `/gal wrap-up` 寫入 |
| `## Release` | `golem-releaser` 發布資訊 |

### 5.4 Context Handoff

在切換工作區或結束 session 前執行：

```
/gal wrap-up
```

這會：
1. 將關鍵上下文壓縮到 plan 的 `## Status > ### Handoff Notes`
2. 更新 `.dev/state.md` 的 Session Continuity 區段
3. 提示提交變更

### 5.5 企劃生命週期

企劃是暫時的工作檔案，不是永久記錄：

1. `/planning` 建立 → `docs/plans/<type>-<slug>.md`
2. `/deep-planning` 收斂（可選）
3. `/refining-plan` 寫入 Tasks 和 Test Plan
4. `/plan-to-prompt` 建立執行 prompt
5. 執行 prompt 自追蹤進度
6. `golem-verifier` 確認目標達成
7. 知識提取到 `docs/`，企劃刪除

> **重要**：在 verification 和 handoff 完成前，**永不**刪除企劃。

---

## 6. 研究工作流

### 6.1 兩種模式

| 模式 | 流程 | 來源要求 | 適用情境 |
| --- | --- | --- | --- |
| `/gal research` | RESEARCH → VERIFY → DOCUMENT | 足夠回答問題即可 | 一般技術研究 |
| `/gal deep-research` | RESEARCH → SYNTHESIZE → CROSS-REVIEW → VERIFY → DOCUMENT | 至少 5 個來源 | 高風險、多衝突觀點 |

**重要**：`VERIFY` 階段必須使用與研究作者**不同**的 model 執行交叉驗證。

### 6.2 DOCUMENT 目標地

| 目標地 | 說明 |
| --- | --- |
| `repo`（預設） | 寫入 `docs/research/` |
| `private` | 寫入 Obsidian 私人 vault（需設定 `OBSIDIAN_VAULT`）|
| `knowledge` | 由 `golem-notewriter` 提取為可重用知識 |
| `none` | 不寫入任何地方 |

---

## 7. 個人化設定

### 7.1 機器本地設定原則

以下檔案都受 `.gitignore` 保護，**不應**提交到 repo：

- `config.local.env` — 機密、絕對路徑、Working Hours 設定
- `mcp-servers.local.json` — MCP 工具覆蓋（從 `mcp-servers.example.json` 複製）
- `model-roles.local.md` — 模型對應（從 `model-roles.example.md` 複製）
- `commands/*/SKILL.local.md` — 指令 skill 的機器本地延伸

### 7.2 常見個人化步驟

**步驟一：設定模型對應**

```bash
cp model-roles.example.md model-roles.local.md
```

編輯 `model-roles.local.md`，填入你的 model 名稱與工具。

**步驟二：設定路徑與機密**

```bash
cp config.example.env config.local.env
```

編輯 `config.local.env`，填入以下佔位符的實際值。

**步驟三：設定 MCP 工具**

```bash
cp mcp-servers.example.json mcp-servers.local.json
```

**步驟四：重新執行 setup**

```bash
./scripts/setup-machine.sh
```

### 7.3 config.local.env 佔位符完整表

| 佔位符 | 用途 |
| --- | --- |
| `OBSIDIAN_VAULT` | Obsidian vault 絕對路徑 |
| `OBSIDIAN_VAULT_NAME` | vault 顯示名稱 |
| `OBSIDIAN_GUIDE_PATH` | 個人 Obsidian guide 路徑（vault 相對路徑）|
| `OBSIDIAN_GUIDE_MODE` | `auto` / `guide` / `generic` |
| `OBSIDIAN_PRIVATE_RESEARCH_DIR` | 私人研究目錄（vault 相對路徑）|
| `OBSIDIAN_DIARY_DIR` | 工作日誌目錄（vault 相對路徑）|
| `OBSIDIAN_SCRATCH_DIR` | 快速暫存目錄（vault 相對路徑）|
| `OBSIDIAN_ARCHIVE_DIR` | 日誌封存目錄（vault 相對路徑）|
| `RESEARCH_DEFAULT_DEST` | `repo` / `private` / `knowledge` / `none` |
| `WORKING_HOURS_ENABLED` | Working Hours 是否啟用（`true` / `false`）|
| `WORKDAY_START` | 工作日開始時間（`HH:MM`）|
| `WORKDAY_END` | 工作日結束時間（`HH:MM`）|
| `WRAP_UP_TIME` | 收尾提醒開始時間（`HH:MM`）|
| `HARD_STOP_TIME` | 強制停止時間（`HH:MM`）|
| `LOCAL_SEARCH_PROJECT` | 本機語意搜尋專案 clone 路徑 |
| `GAL_SKILLS` | Skills 安裝路徑 |
| `TEMP_DIR` | 暫存輸出目錄 |
| `MCP_FILESYSTEM_PATHS` | filesystem MCP 允許的路徑 |
| `MCP_MEMORY_FILE_PATH` | MCP memory JSON 路徑 |
| `CONTEXT7_API_KEY` | Context7 API key |

### 7.4 Working Hours 設定

Working Hours 預設關閉。若要啟用，在 `config.local.env` 設定：

```env
WORKING_HOURS_ENABLED=true
WORKDAY_START=09:00
WORKDAY_END=18:00
WRAP_UP_TIME=17:30
HARD_STOP_TIME=19:00
```

**時間窗口行為：**

| 時間窗口 | 行為 |
| --- | --- |
| 工作時間內 | 正常運作，無限制 |
| 下班後（到 Wrap-up 前）| 可繼續，僅供參考 |
| Wrap-up Time 到 Hard Stop | 提醒收尾，建議執行 `/gal wrap-up` |
| Hard Stop 後 | **所有 agents 拒絕工作** |

若需臨時覆蓋，對 AI 說：`override working hours`

### 7.5 Obsidian 整合

Obsidian 支援為選用機器本地功能。GAL 使用 Obsidian 內建 `obsidian` CLI（需 Obsidian 1.12.7+，並在設定中啟用 CLI）。

**Guide 模式選項：**

| `OBSIDIAN_GUIDE_MODE` 值 | 行為 |
| --- | --- |
| `auto` | 若 guide 檔案存在則載入，否則使用 PARA 預設 |
| `guide` | 強制載入 guide（找不到時報錯）|
| `generic` | 跳過 guide，使用通用 PARA 規則 |

**常用預設路徑（vault 相對）：**

| 設定 | 典型值 |
| --- | --- |
| `OBSIDIAN_GUIDE_PATH` | `99_System/Guide.md` |
| `OBSIDIAN_PRIVATE_RESEARCH_DIR` | `10_Projects/Research_Private` |
| `OBSIDIAN_DIARY_DIR` | `10_Projects/Work_Journal` |
| `OBSIDIAN_ARCHIVE_DIR` | `30_Archives/Work_Journal` |

### 7.6 指令 Skill 本地延伸

若要在不影響 repo 追蹤的情況下自訂特定指令行為：

```
commands/<command>/SKILL.local.md
```

此檔案受 `.gitignore` 保護。`Setup-Machine` 會在產生 `SKILL.md` 時自動附加其內容。

> **注意**：不要直接編輯 `commands/<command>/SKILL.md`，它是產生的檔案，下次 setup 時會被覆蓋。

### 7.7 何時重新執行 Setup

以下任一項目變更時，重新執行 `setup-machine.sh`（或 `.ps1`）：

- `config.local.env`
- `mcp-servers.local.json`
- 任何 `commands/*/SKILL.local.md`
- `~/.gal/install-state.json`
- Obsidian 路徑或 Guide 模式
- Working Hours 設定
- 模型對應或執行環境安裝位置

---

## 8. 儲存邊界

GAL 明確區分 repo 共享狀態與私人知識庫：

| 類型 | 位置 | 說明 |
| --- | --- | --- |
| **Repo 共享** | `.dev/`、`docs/plans/`、`docs/research/` | 版本控制，跨機器可見 |
| **Obsidian 私人** | `$OBSIDIAN_VAULT/...` | 機器本地，個人知識庫 |
| **機器本地設定** | `config.local.env`、`*.local.*` | gitignored，不跨機器共享 |

**重要原則**：
- Repo 共享狀態的耐久格式是 Markdown 檔案
- 臨時執行環境輸出不應替換 `.dev/` 或企劃檔案
- 有價值的知識從企劃流向 `docs/`，再索引到 `.dev/project.md`

---

## 9. 協作工具

協作工具是可選的增強功能。所有工具在使用前都會執行統一 preflight 檢查：

```
applicability → availability → initialization → readiness → route 或 degrade
```

若工具不可用，系統會自動降級到原生替代方案，**不會**將工具缺失視為工作流錯誤。

### 9.1 安裝協作工具

```bash
./scripts/setup-tools.sh         # macOS/Linux
./scripts/Setup-Tools.ps1        # Windows
```

### 9.2 工具清單

| 工具 | 用途 |
| --- | --- |
| **graphify** | 從程式碼、文件、圖片產生互動式知識圖 |
| **OpenCLI** | 將網站/瀏覽器/Electron 應用轉為 CLI 介面 |
| **gstack** | Garry Tan 的 startup agent 集合（QA、設計、發布等）|
| **Remote Worker** | 跨機器遠端任務派發（B+C 節點）|
| **Godot C#** | Godot 4 C# repo 的場景、腳本、資源操作 |
| **AI-First 遊戲素材** | ComfyUI 驅動的 2D/3D 遊戲素材生成流程 |

### 9.3 graphify

`/gal init` 執行後，若 `graphify-out/` 目錄已存在，GAL 會在企劃和審核時自動讀取 `graphify-out/GRAPH_REPORT.md` 作為可選結構分析。

手動重新產生：

```
/graphify .
```

實作有較大改動後建議重新執行，以保持圖形資料與程式碼一致。

---

## 文件索引

| 文件 | 說明 |
| --- | --- |
| [README.zh-Hant.md](../README.zh-Hant.md) | 主要使用者入口，中文版 |
| [docs/manual.md](manual.md) | **本手冊**：完整使用指南 |
| [docs/devguide.md](devguide.md) | 維護者指南：runtime topology 和測試 |
| [docs/personalization.md](personalization.md) | 機器本地個人化設定完整參考 |
| [docs/collaborative-tools/](collaborative-tools/) | 各協作工具詳細說明 |
