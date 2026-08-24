---
source: docs/manual.md
lang: zh-Hant
source_commit: fb6a41930fdbbd4069ed2d76725be9c81f6e5cca
translated_at: 2026-08-15
status: current
---

# GAL 使用手冊

[English](../../manual.md) · [日本語](../ja/manual.ja.md) · **繁體中文**

GAL 日常使用指南涵蓋安裝、初始儲存庫設定、工作流程操作、設定、個人化、無頭執行器 (executor) 路由以及 golem 代理程式。

本手冊詳細說明 **GAL 操作**。系統架構（程式碼庫與 `~/.gal/` 拓樸、發行血緣）記載於 [`docs/architecture.md`](../../architecture.md)。維護者程序（發行機制、提供者打包、修改指南）記載於 [開發者指南](../../devguide.md)。本手冊排除這些主題。

## 概觀

GAL 使用 `~/.gal/plugins/gal/` 作為標準根目錄 (canonical root)。提供者可見目標作為投影 (projection) 運作，而非內容擁有者。設定儲存於 `~/.gal/config/config.json`。關於 `~/.gal/` 佈局與擁有權邊界，請參閱 [架構 → `~/.gal/` 執行環境佈局](../../architecture.md#gal-runtime-layout)。

`gal` 二進位檔會透過以 `.git` 為界的目前工作目錄搜尋，接著進行二進位檔封裝佈局，自動定位其來源根目錄 (source root)。不需亦不會讀取 `devMode` 與 `galRoot` 設定鍵。

## 安裝 GAL

選擇合適的平台安裝選項。安裝完成後，於儲存庫內執行 `gal init` 以產生儲存庫本機轉接器（`CLAUDE.md`、`AGENTS.md` 等）— 請參閱 [在您的儲存庫首次執行](#在您的儲存庫首次執行)。

### 安裝選項

#### `cargo install --git` (從原始碼)

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli
```

GAL 未包含於 crates.io — 必須使用 `--git` 旗標。Cargo 工作區內必須使用 `gal-cli` 套件名稱。安裝的二進位檔名稱為 `gal`。

#### Homebrew (macOS / Linux)

```bash
brew install monkey1wizard/tap/gal
```

此為 macOS 與 Linux 的標準套件管理員安裝方法。

#### winget (Windows — 請先探測)

```sh
winget install Monkey1Wizard.GAL
```

目錄可見度可能落後於發行版本，安裝前請透過 `winget show Monkey1Wizard.GAL` 驗證可用性。此處適用 `Get-Alias gal` 遮蔽限制。

#### 直接下載 (壓縮檔)

從 GitHub 頁面下載 `gal-<version>-<platform>-<arch>[.zip|.tar.gz]`，解壓縮內容，並將 `gal`（或 `gal.exe`）新增至 `PATH`（macOS / Linux：請透過 `chmod +x gal` 確保執行權限）。

#### curl (Linux / macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
```

指令碼會比對 `checksums.txt` 驗證 SHA-256 總和檢查碼（強制需求，不符則中止），接著進行 cosign 無金鑰簽章驗證。簽章驗證的可用性採盡力而為，但結果判定嚴格。若 cosign 不存在或簽章檔案下載失敗，安裝程式會發出警告並僅使用 SHA-256 結果繼續。若 cosign 執行並回報簽章不符，安裝會在解壓縮或寫入前中止。二進位檔安裝至 `~/.local/bin`，GAL 原始碼負載安裝至 `~/.local/share/gal`。版本覆寫使用 `GAL_VERSION`。

#### irm (Windows)

```sh
irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
```

此方法與 curl 路徑共用 SHA-256 及 cosign 驗證，包含簽章檢查失敗時中止安裝。將 `gal.exe` 與 GAL 原始碼負載直接安裝至 `%LOCALAPPDATA%\Programs\gal`（將此目錄新增至使用者 `PATH`，安裝程式會輸出確切指令）。版本覆寫使用 `$env:GAL_VERSION`。

注意：PowerShell 內建的 `Get-Alias gal` 可能遮蔽二進位檔。若 `gal` 解析為替代目標，請改用絕對路徑呼叫執行檔。

#### 透過 Claude / Codex 市集 (對話式二進位檔安裝)

或者，將 GAL 作為外掛程式，並授權 AI 完成設定：

1. 在 Claude Code 或 Codex 外掛程式市集中找出並安裝 **GAL 外掛程式**（搜尋「gal」），或從 [市集快照分支](https://github.com/monkey1wizard/golem-agents-legion/tree/marketplace-snapshot) 新增。
2. 透過 **"help me install gal"** 要求安裝。外掛程式提供 `install-gal` 技能。同意後，會執行適用於該作業系統的選項（Homebrew / winget / `cargo install --git`），驗證結果，並將程序導向 `gal init`。若選項無法使用，會直接回報。

獨立的外掛程式不構成可運作的 GAL 安裝。除 `install-gal` 外的所有指令皆需要 `gal` 二進位檔，因此步驟 2 是完成安裝的必備條件。`install-gal` 技能不包含二進位檔綑綁，執行前需預覽指令，並嚴格回報事實成功狀態。

---

單獨執行 `gal` 指令（不加任何參數）會印出使用說明，並以結束碼 0 結束。若輸入無法辨識的子指令，則會以結束碼 64 失敗。

### 升級 (`gal update`)

`gal update` 指令會輸出已安裝版本及平台專屬升級指示。不支援自我更新。升級必須使用原始安裝方法：

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli  # cargo
winget upgrade Monkey1Wizard.GAL                                                   # Windows
brew upgrade gal                                                                   # macOS / Linux
```

## 在您的儲存庫首次執行

### `gal init`

`gal init` 指令會從 `gal-core` 範本產生五個儲存庫本機轉接器根目錄（`CLAUDE.md`、`AGENTS.md`、`GEMINI.md`、`.github/copilot-instructions.md`、`.agents/rules/gal.md`），並於首次執行時建立 `.dev/` 目錄骨架。加上 `--force` 旗標執行會以冪等方式重新產生轉接器。

執行過程會針對每個更動的轉接器路徑輸出單列，分類為 `Written`（建立或修改內容）、`Unchanged`（位元組完全相同，略過寫入）或 `Removed`（未選取的過時 GAL 擁有條件層）。

> ⚠️ `--force` 旗標會執行啟動載入 (bootstrap) 覆寫。此動作會以範本完全取代 `.dev/project.md` 並捨棄現有儲存庫專案內容。避免使用此旗標略過初始化錯誤。請直接解決回報的問題（參閱下文）。

### 您的專案的 `.dev/project.md`

`.dev/project.md` 檔案是專為轉接器算繪所精簡的專案摘要。檔案中**嚴格規定必須且只能出現一次**以下八個 H2 區段：`What This Is`、`Tech Stack`、`Architecture`、`Constraints`、`Response Style`、`Freshness`、`Project Language`、`Protected Paths`。

此檢查機制採 fail-closed（預設阻擋）運作。若有區段缺失或重複，`gal init` 會拒絕整個算繪作業、指出有問題的標題，並停止寫入轉接器檔案。錯誤訊息每次執行僅會回報單一缺失標題，若缺失多個區段需反覆修正。請手動補上系統提示的區段（格式參考 `plugins/gal-core/templates/project.md`），重新執行指令，再依序補齊後續區段。

`.dev/project.md` 檔案強制執行嚴格的容量上限。若因大小超標遭拒，必須精簡內容而非重試。算繪程序絕對不會為了規避大小限制，而寫入不完整的轉接器集合。

### `gal doctor`

`gal doctor` 指令用於檢查本機環境的健康狀態，範圍涵蓋二進位檔、標準根目錄、執行環境介面與設定。建議在初次安裝、版本升級後，或當代理程式無法辨識 GAL 指令時執行此檢查。

`gal doctor` 會檢查以下項目：

- **OpenCode 投影漂移** — 偵測 `~/.config/opencode/` 的指令與代理程式檔案是否不再符合標準根目錄的算繪結果。過期內容會回報警告，缺失檔案則回報錯誤。執行 `gal refresh` 即可修復。
- **Claude 技能介面** — `~/.claude/skills/gal` 缺失時回報警告（而非錯誤），並提供手動建立指示。`gal refresh` 不會自動建立此介面。

若在已初始化的儲存庫內執行，該指令會額外印出一份唯讀的容量建議表，列出五個轉接器根目錄的檔案大小。若轉接器檔案過大，系統僅會提示 `[WARNING]` 警告而非錯誤，藉此避免導致單次執行失敗。

若需針對無頭編碼代理程式進行選擇性的自我測試，請參閱 [執行器自我測試](#執行器自我測試-gal-doctor---executor-smoke) 說明。

### 依執行環境觸發 GAL 指令

GAL 指令透過各執行環境 (runtime) 特有的機制初始化，因而產生不同的觸發方式：

| 執行環境 | 觸發方式 | 說明 |
| --- | --- | --- |
| Claude Code | `/gal status` | 原生外掛程式指令 |
| Codex | `$gal-status` | 作為技能公開（`$` 字首，或 `/skills`） |
| Copilot | `/gal-status` | 作為技能公開（透過 `/skills list` 瀏覽） |
| Antigravity | `/gal-status` | 作為技能公開（Antigravity 缺乏原生 `commands/` 資料夾，指令為代理程式技能） |
| OpenCode | `/gal-status` | 原生 Markdown 指令 |

注意語法差異。非 Claude 執行環境使用 `gal-status`（連字號）而非 `gal status`（空白）。

### 指令變更何時生效

指令更新可見度取決於執行環境的載入機制：

- **Claude Code** 會快取外掛程式。**重新啟動 Claude Code** 以載入更新的指令。
- **Antigravity** 於啟動時註冊指令。**重新啟動 agy** 以載入變更。
- **Copilot / OpenCode** 在每個新工作階段直接讀取指令與技能檔案。
- **Codex** 在使用中執行緒內依據記錄的主要行為自動偵測技能變更。若變更未出現，**重新啟動 Codex 或初始化新執行緒**作為備用方案。

> **支援邊界：** GAL 不再向 Gemini CLI 投影指令或技能。使用已退役 Gemini CLI 表面的使用者必須遷移至 Antigravity，其指令表面為 `~/.gemini/antigravity-cli/skills/<name>/SKILL.md`。

關鍵 Codex 行為包含：

- **因脈絡預算而被省略並非失敗。** Codex 限制初始技能清單。超出此限制會導致描述縮短，隨後將技能從清單中省略。遭省略的技能仍可透過 `$skill-name` 直接呼叫。
- **同名雙重列出。** 多個投影相同名稱技能的工具會繞過 Codex 合併功能。兩筆項目皆可能出現在技能選擇器中。

## 執行工作流程

工作流程圖與階段概觀位於 [README](../../../README.md#how-gal-works)。本節特別詳細說明需要使用者做出的決策、手動輸入與工作流程中斷時的必要動作。

### 規劃：使用者的決策

- `/planning` 指令將請求轉換為位於 `.dev/plans/<type>-<slug>.md` 的來源計畫 (source plan)。規劃階段支援協作，允許使用者自由討論、合併或分割計畫，並同時諮詢 golem 代理程式。
- **未決問題需由使用者解決。** `/deep-planning` 指令強制在執行 `/refining-plan` 前解決所有 `## Open Questions` 項目。問題遵循分級關卡。**H** 級問題需僅限人類權限，禁止代理程式關閉。**A** 級問題允許 architect 角色在附上記錄理由後關閉。**F** 表示虛假問題。模稜兩可的問題預設為 H 級分類，等待使用者輸入。
- **核准需明確確認。** `/refining-plan` 收斂後，於計畫的 `## Approval` 區段依固定順序記錄四行核准欄位：`- Human approval: [pending|approved]`、`- Architect review: [pending|clear|blocked|not-required]`、`- Design review: [not-requested|clear|blocked]`、`- Business review: [not-requested|clear|blocked]`。`/plan-to-prompt` 指令要求該段落含有逐字的 `- Human approval: [approved]` 這一行，若缺乏此行則拒絕產生執行提示檔。
- **多個使用中計畫需指定目標。** 同時有多個使用中計畫時，在叫用規劃指令期間需明確指定計畫檔案。GAL 嚴格避免自動選取。

### 管道：啟動、停止、繼續

- **啟動：** 執行 `/gal pipeline`。單一使用中計畫會觸發自動執行提示檔解析。多個使用中計畫需明確指定提示檔（`.dev/plans/<slug>.prompt.md`）。
- **執行階段：** 管道依序自動反覆執行每項任務的實作、測試、稽核與提交程序。除非工作流程中斷，否則無須使用者介入。
- **修正重試必須具備新的權威指示與實際實作變更。** 測試或稽核失敗後，管道會寫入唯一一個 OPEN retry handoff，並以 `--fix` 重新派送 implement。若 task goal、handoff、affected-file allowlist 或 agent contract 與前次嘗試相同，系統會在 executor spawn 前拒絕重試。若 executor 完成後未變更任何 affected implementation file，該回合會以非零的 `fix-round-no-change` 結束；prompt、receipt、replay sidecar 與 executor log 的寫入都不算實作變更。
- **中斷條件：** 管道僅因人類決策阻礙、任務達到重試上限（三次驗證失敗），或設定的工作時間硬性停止而暫停。中斷時，執行提示檔會記錄中斷階段註記，詳述停止點與待辦需求。
- **繼續協定需要重新執行。** 解決阻礙或根本原因後，重新執行相同的 `/gal pipeline` 指令。若遭到 replay refusal，重新開始前必須讓已記錄的 OPEN handoff 描述實質不同的問題或下一步；僅變更時間戳記或其他顯示 metadata 無法解除拒絕。執行會從記錄的游標處繼續。已完成的任務會略過重新執行。

### Finalize 落地收尾：保留與刪除的內容

`/gal finalize` 指令負責協調已完成計畫的結案流程。前置條件要求所有任務都已完成且通過驗證，並以零信任的機器收據（`gal finalize-check`）作為證明。

- **會落地的內容：** 在文件同步前會進行全面的跨任務分支審查。STEWARD 代理程式必須將計畫的持久知識擷取至 `README.md` 與 `docs/`。隨後進行合併至 main 的作業，若適用則包含工作樹拆除。
- **會刪除的內容：** 僅在文件成功提交且寫入後的專案整潔度檢查（hygiene check）通過後，才會移除 `.dev/plans/` 內的計畫檔案。此程序保證在檔案抹除前，已將知識安全轉移至持久層。
- **會保留的內容：** 在 `.dev/state.md` 中寫入包含日期、計畫與落地提交的結案列，並在落地提交上標記 `gal-last-good` 標籤。

### 代理程式合約解析

在派送以提示檔或來源計畫建構的管道階段之前，`gal` 會先定位權威的代理程式合約（`agents/golem-{implementer|tester|auditor}.agent.md`），並將其確切內容嵌入送給執行器的任務規格中。無論是本機派送，還是透過 SSH 通道派送，任何被派送的執行器都不會被要求開啟僅控制節點可見的合約路徑，任務規格本身即為自足內容。

**解析順序（第一個命中的來源根目錄勝出）：**

| 層級 | `contract_source` | 根目錄 |
| --- | --- | --- |
| 1 | `workdir` | 經過正規化的 `--workdir` 本身，或其直屬的 `plugins/gal-core` |
| 2 | `ancestor` | workdir 最近的祖先目錄中，可被辨識為 GAL 來源根目錄者 |
| 3 | `exe-side` | 正在執行的 `gal` 二進位檔旁的目錄 |
| 4 | `embedded` | 具體化於 `~/.gal/embedded-src` |

`workdir` 的優先權高於 `ancestor`、`exe-side` 與 `embedded`。即使 PATH 上同時存在另一個版本的封裝版 `gal` 二進位檔，這個順序仍能確保受信任的本機 GAL 檢出版本保有權威地位。換句話說，只要儲存庫內建版本控管了 `plugins/gal-core/`，其合約解析結果一律以自身版本為準，不受已安裝二進位檔版本的影響。若您維護的是內建版本，`Dispatch:` 標記上的 `contract_source`（參見〔檢查派送〕(#檢查派送)）就是檢查版本落差的地方。

**復原方式依失敗型態而異：**

- **命中的根目錄本身已損壞** — 找到了排名最高的根目錄，但其對應階段的合約檔案缺失、非 UTF-8 編碼，或無法讀取。派送會在啟動執行器前停止，並回報錯誤，指出該層級與根目錄。此情況絕不會繼續退回下一層級解析，因此請直接修復所指名根目錄下的檔案，而非期待較低層級能夠代為補上。
- **全部未命中** — 沒有任何層級能提供可用的根目錄。派送會停止（結束碼 1），並列出每個層級的結果，同時附上兩條復原路徑：透過套件發佈通道重新安裝 `gal`，或改由 GAL 原始碼檢出版本執行該指令。

**原始 / 直接派送的例外情形。** 原始的 `gal dispatch` 與原始任務規格式 `gal pipeline` 輸入，從不解析或憑空產生這項來源資訊。它們的標記與執行器日誌標頭，在位元組層級上與導入來源資訊之前的格式保持相容，也就是說這些路徑上不會出現 `contract=` / `contract_source=` 欄位。

### 收工 (Wrap-up) 與落地 (Finalize) 比較

`/gal wrap-up` 指令作為**暫停**而非落地功能。它將工作階段交接筆記壓縮至執行提示檔中，更新 `.dev/state.md` 內的工作階段連續性，並執行提交。這使得任何執行環境都能準確在中斷點繼續。該指令不會關閉任何項目。在計畫中途停止時執行此指令。`/gal finalize` 嚴格保留給已完成的計畫使用。

## 設定 (`~/.gal/config/config.json`)

本機數值儲存於 `~/.gal/config/config.json`。保留的機器路徑對應至 `galSkills`，工作時間對應至 `workingHours`。其他鍵值包含 `planLanguage`、`memoryHarvest` 與 `executorRouting`（詳見 [無頭執行器](#無頭執行器)）。嚴格禁止將本機數值寫入受追蹤的文件、指令範本或原始碼檔案中。

| 預留位置 | 意義 | 常見用途 |
| --- | --- | --- |
| `<WORKING_HOURS_ENABLED>` | 是否啟用工作時間強制執行 | 選擇性加入的收工與硬性停止強制執行 |
| `<WORKDAY_START>` / `<WORKDAY_END>` | 偏好工作日，格式為 `HH:MM` | 工作時間排程 / 下班後邊界 |
| `<WRAP_UP_TIME>` / `<HARD_STOP_TIME>` | 收工與硬性停止時間，格式為 `HH:MM` | 關機視窗 / 停止工作行為 |
| `<GAL_SKILLS>` | 機器本機 GAL 技能目錄的絕對路徑 | git 篩選器與機器本機技能投影 |

### 安全憑證與 MCP 覆寫

受版控的 `plugins/gal-core/mcp.json` 是 GAL 自有 MCP server 的唯一來源。`gal refresh` 會把它逐字複製成 canonical `.mcp.json`，並併入 `~/.gal/local/mcp.json` 內的個人 server。系統不支援使用獨立的覆寫檔案。

GAL 不做任何預留位置替換。清單內寫的 `${ENV_VAR}` 會原封不動被帶過去，若要解析，是由載入該檔案的 MCP host 自行處理，通常來自行程環境變數。請把這類變數設在環境中，不要放進 `config.json`。

針對 Playwright MCP，請在受版控的設定中保持保守且與機器無關的設定值。若需設定僅限本機的瀏覽器參數（包含有頭模式、視窗與裝置模擬、儲存狀態路徑、輸出目錄、持久設定檔，以及擴充功能與 CDP 佈線），請在受版控的清單內使用 `${ENV_VAR}` 預留位置。這些變數會直接由 `config.json` 解析替換：

```json
{
  "servers": {
    "playwright": {
      "args": ["-y", "@playwright/mcp@latest", "--isolated", "--headless",
        "--storage-state", "${PLAYWRIGHT_MCP_STORAGE_STATE}",
        "--output-dir", "${PLAYWRIGHT_MCP_OUTPUT_DIR}"]
    }
  }
}
```

單一機器上雙 Postgres 資料庫的設定語法：

```json
{
  "servers": {
    "postgres-app": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_APP_URI}" }
    },
    "postgres-analytics": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_ANALYTICS_URI}" }
    }
  }
}
```

請在編碼代理程式執行時所處的環境中匯出對應的值。MCP 設定內的 `${ENV_VAR}` 預留位置由 host 在載入時解析，不是由 GAL 解析。已安裝的執行環境 MCP 設定檔保留使用者擁有權，GAL 永遠不會寫入它們。請嚴格避免將儲存狀態（storage-state）檔案、持久設定檔、瀏覽器暫存檔以及含有敏感資訊的本機檔案提交至版本控制。

### 工作時間

工作時間限制預設為停用。在 `~/.gal/config/config.json` 的 `workingHours` 區塊下可設定工作日時間邊界，將 `enabled` 設為 `false` 即可保持停用。`workdayStart` 與 `workdayEnd` 用於定義可作業的時段。`wrapUpTime` 負責啟動提醒與收工程序。`hardStopTime` 則觸發代理程式的絕對拒絕執行狀態。這些設定屬於機器本機偏好，而非受追蹤的儲存庫政策。

### 計畫語言

- `config.json` 內的 `planLanguage` 是機器本機的選用設定。在無明確指示時，由此設定決定 `.dev/plans/*.md` 與 `.dev/research/*.md` 的預設輸出語言。解析順序依次為：明確指示、`planLanguage`、提示語言自動偵測，最後退回 `en` 作為備用。
- 受版本控制追蹤的 `PROJECT_LANGUAGE` 專案中繼資料（metadata），用於決定主要文件的標準語言。
- 所有符合 `.dev/plans/*.prompt.md` 的檔案嚴格限定為英文，以確保跨模型的執行穩定性。

若 `planLanguage` 不設為英文，系統會產生三層架構的計畫檔案：第一層為英文語意草稿（`.dev/plans/<slug>.en.md`），作為技術意義的權威基準。第二層為在地化的來源計畫檔（`.dev/plans/<slug>.md`），供閱讀與手動編輯。其中的標題、路徑、任務 ID 與執行判定（verdicts）皆保持英文，僅敘述段落採用在地化語言。第三層為英文的執行提示檔。

**系統完全支援手動編輯在地化計畫檔**。後續的規劃指令會自動偵測手動修改，並暫停於唯讀的協調步驟，將手動變更合併回英文草稿後才繼續執行，以防止系統靜默覆寫使用者的變更。執行 `/plan-to-prompt` 後會自動刪除英文草稿，使每個計畫僅保留兩個受追蹤的檔案。若 `planLanguage` 設為英文，則會直接略過此三層機制。

### 供應商記憶擷取 (Provider-Memory Harvest)

此選擇性機制可將編碼代理程式 (coding agent) 在目前對話中所發掘的實用經驗，轉移至受 GAL 追蹤的檔案中，避免在工作階段終止時遺失。

- **預設停用，僅限本機。** 將 `config.json#memoryHarvest.enabled` 設為 `true` 即可啟用。遺漏或 `false` 的值皆維持停用狀態。
- **範圍受限。** GAL 避免為了識別候選項目，而去開啟、列出或搜尋編碼代理程式的對話紀錄或工作階段檔案。
- **限於儲存庫範圍且經過刪減。** 候選項目必須具備指向儲存庫檔案的直接連結。項目在公開可見之前會經過改寫與刪減。原始引述、安全憑證、機器路徑與個人筆記皆被嚴格排除。
- **強制核准。** 候選項目的核准流程專屬於 `/gal wrap-up` 期間進行。遭拒絕或未回覆的候選項目將不會被寫入。經核准的項目會作為臨時性、建議性質的任務記憶，進入作用中計畫（active plan）的交接筆記。
- **核准限制。** 獲得核准並不保證獲得提升（promotion）。`/gal finalize` 指令必須在獨立驗證符合所有提升經驗的標準後，才會將核准的候選項目提升至 `docs/`。

## 個人化 (`~/.gal/local/`)

本機個人內容位於 `~/.gal/local/` 下，並透過標準根目錄算繪投影至所有代理程式。`gal` 二進位檔讀取 `local/skills/`、`local/mcp.json` 與 `local/conventions/`，但**嚴格避免寫入或刪除這些使用者建立的路徑**。跨機器同步不屬於 `gal` 功能範圍。

### 個人技能 / MCP / 慣例

**佈局：**

```text
~/.gal/local/
  skills/
    <skill-name>/
      SKILL.md          ← hand-placed personal skill (gal read-only)
  mcp.json              ← personal MCP servers (same format as plugins/gal-core/mcp.json, gal read-only)
  conventions/
    <lang>.md           ← personal coding-style convention file (gal read-only, see below)
```

**啟用：** 啟動取決於存在與否而非設定旗標。將檔案置於個人根目錄構成選擇加入動作。缺少的目錄或檔案維持算繪輸出與純 Core 算繪位元組完全相同。

**投影規則：**

- 個人技能在核心內容後合併。名稱與核心技能衝突的個人技能會在核心優先政策下被靜默略過。
- 個人 MCP server 合併至標準 `.mcp.json` 檔案。名稱與核心 server 衝突的個人 server 會在核心優先政策下被略過。
- `gal doctor` 指令回報個人技能、個人慣例檔案，與核心衝突略過次數。

### 程式碼風格慣例

GAL Core 排除擁有者個人的程式碼風格慣例。內部風格透過三種不同來源傳播至下游儲存庫：

**來源 1 — 個人慣例檔案（常駐，每個儲存庫）。** 將隨附的範例從 `plugins/gal-core/templates/csharp-convention.example.md` 複製至 `~/.gal/local/conventions/csharp.md`，路徑必須完全一致，並修改內容以反映目標風格。在 `.dev/project.md` 中有相符 `Language` 列的任何目標儲存庫內執行 `gal init`。系統會把檔案注入儲存庫轉接器，處理方式比照 gal-core 的慣例。

**來源 2 — 安裝的代理程式外掛程式偵測（唯讀，零設定）。** 在編碼代理程式內安裝的現有官方語言外掛程式僅需在目標儲存庫執行 `gal init`。GAL 將外掛程式技能名稱與來源關聯至儲存庫 `Language` 列，並算繪一個包含名稱、來源路徑與常駐載入指示的 **Detected Language Skills** 參考區塊。GAL 嚴格避免複製技能內容並禁止安裝、更新或移除外掛程式。新安裝的外掛程式需要後續執行 `gal init` 進行偵測。

**來源 3 — 個人技能（隨需）。** 在 `~/.gal/local/skills/<name>/` 下編寫包含明確描述的 `SKILL.md` 檔案。重新啟動 Claude Code 初始化偵測，而其他代理程式在下一次讀取週期偵測檔案。代理程式僅在被指名時載入此來源，這與來源 1 與 2 的常駐注入形成對比。

**來源選取策略：** 若已有官方外掛程式安裝，優先選擇來源 2 達成零設定。來源 1 提供具最大控制權的常駐個人風格。來源 3 滿足隨需代理程式諮詢需求。

**缺失合約：** 缺乏相符來源的儲存庫 `Language` 列將產生不含特定語言慣例或參考區塊的轉接器。此行為構成靜默的設計跳過而非錯誤條件。

在儲存庫的 `.dev/project.md` Tech Stack 表格中插入 `| Personal Conventions | off |` 列以停用來源 1 與 2。建議在公開儲存庫採用此實踐，防止受追蹤轉接器嵌入擁有者機器內容。來源 3 不受影響。

### 指令技能本機覆疊 (`SKILL.local.md`)

若要為指令技能實作本機客製化，產生檔案 `plugins/gal-core/commands/<command>/SKILL.local.md`：

- 此檔案受 gitignore 保護，且作為使用者擁有的本機輸入運作。
- 烘焙程序將 `SKILL.local.md` 附加至產生的 `SKILL.md` 中。
- 避免直接編輯 `plugins/gal-core/commands/<command>/SKILL.md`。其為受取代影響的產生檔案。
- 將 `SKILL.local.md` 限制於補充指示內容。排除次要 frontmatter 區塊。

### 本機筆記路由

外部筆記為本機且選用的元件。GAL 隔離儲存庫擁有的狀態與使用者擁有的筆記。核心行為獨立於私人筆記存放區運作。可攜綁定合約位於 [`optional-capabilities.md`](../../../plugins/gal-core/conventions/optional-capabilities.md)。它與應用程式無關，預設為關閉，並僅在本機後端就緒時執行。

不存在、無法連線或未初始化的後端會觸發降級至標準的儲存庫本機工作流程，不會產生失敗警報。只有已選擇加入且可連線的後端，才允許在工作流程明確授權之處進行讀取、搜尋或寫入。這些狀態一律非強制性。缺乏筆記後端的儲存庫運作方式與完全連線的執行個體相同，僅在缺乏選用脈絡來源上有所不同。

記載的後端範例包含 Obsidian（`coddingtonbear/obsidian-local-rest-api`）、Logseq（`ergut/mcp-logseq`）、Joplin（`joplin-mcp`）、通用 markdown 金庫（`vault-mcp`），以及 CJK 優先檢索（`SeekLink`）。此清單不保證功能對等。

儲存庫擁有的研究預設為 `.dev/research/` 目錄。

## 無頭執行器

GAL 支援將管道階段轉移至次要的無頭編碼代理程式 CLI，避開對話迴圈執行。在 `~/.gal/config/config.json` 內的 `executorRouting` 下實作此設定。

### 無頭執行器路由

`config.json#executorRouting` 鍵依消費者將角色分類為兩個不同的物件。`pipeline` 物件處理實作、測試與稽核階段的無頭派送。`planning` 物件管理規劃階段審查角色的 Codex 原生子代理程式模型選擇，並嚴格避免無頭派送。共用的 `executors` 預設模型區塊伴隨這些物件：

```json
{
  "executorRouting": {
    "executors": {
      "claude":   "claude-haiku-4-5-20251001",
      "codex":    "gpt-5.4-mini",
      "opencode": "opencode/minimax-m3-free",
      "copilot":  "claude-haiku-4-5-20251001",
      "agy":      "gemini-2.5-flash"
    },
    "pipeline": {
      "CODER":   { "executor": "codex" },
      "TESTER":  { "executor": "opencode" },
      "AUDITOR": { "executor": "claude", "model": "claude-sonnet-4-6" }
    },
    "planning": {
      "ARCHITECT": { "executor": "codex", "model": "gpt-5.4", "effort": "high" },
      "ANALYST":   { "executor": "codex", "model": "gpt-5.4-mini" }
    }
  }
}
```

這兩個群組皆強制執行封閉式角色允許清單。`pipeline` 清單包含 `{CODER, TESTER, AUDITOR}`。`planning` 清單包含 `{ARCHITECT, ANALYST, DESIGNER, RELEASER}`。指派至錯誤群組或無法識別的角色會觸發略過，並產生指定正確群組的警告。過去直接在 `executorRouting` 下使用角色鍵的扁平化結構已退役，會產生可見的警告且無退回機制。

**executors 區塊：** 定義每項工具的預設模型。缺乏模型指定的角色項目繼承自 `executors[executor]` 的預設值。角色上的明確模型宣告取代預設值。

**每個角色 effort 鍵：** 提供選用的推論強度指示器，在啟動 (spawn) 前對應至各個執行器的原生推論旗標：

| 執行器 | `effort` → 原生旗標 |
| --- | --- |
| claude | `--effort <value>` |
| codex | `-c model_reasoning_effort="<value>"` |
| copilot | `--reasoning-effort <value>` |
| opencode | `--variant <value>` |
| agy | unsupported |

此機制採 fail-closed（預設阻擋）運作。無法遵守推論提示或遭遇格式錯誤值的執行器，會在啟動前降級派送，而非靜默忽略該提示。`effort` 鍵僅作為提示運作，從不作為模型選擇器，亦不暗示付費或命名模型。

**每個角色 `timeoutSecs` 鍵：** 可選填的正整數，會覆寫該角色的派送逾時，本地與 SSH（遠端）路徑一律適用同一套覆寫值：

```json
"pipeline": {
  "CODER": { "executor": "codex", "timeoutSecs": 900 }
}
```

角色未設定 `timeoutSecs` 時，仍會維持原本不變的 300 秒 CLI 預設值。角色若將此鍵設為 `0`，載入時會被拒絕並記錄警告，行為等同於完全沒有設定這個鍵——因為數值為零從來就不是有意義的逾時。設定 `timeoutSecs` 不會改變任何 `dispatch-script` 命令列語法，只會改變 routing 解析完成後，binary 實際套用的逾時數值。

**轉接器行為注意事項：**

- **OpenCode** 使用 `--agent build` 進行派送以啟用具備寫入能力的代理程式，避開靜默阻擋寫入的唯讀預設值。它也利用 `--auto` 作為權限略過旗標。
- **Copilot** 附加 `--no-custom-instructions` 與 `--disable-builtin-mcps`，避免無頭 prompt 模式超出脈絡限制。Copilot Free 嚴格以**僅限自動**運作，維持 `model: auto` 與僅限本機執行。

**角色表：**

| 角色 | 群組 | 用途 |
| --- | --- | --- |
| `CODER` | pipeline | 依照計畫編寫實作程式碼。必須與 `TESTER` 不同 |
| `TESTER` | pipeline | 僅從計畫規格與公開 API 編寫測試。必須與 `CODER` 不同 |
| `AUDITOR` | pipeline | 稽核深度效能與安全性。必須與 `CODER` 不同，層級 >= `CODER` |
| `ARCHITECT` | planning | 涵蓋取捨、過度設計與錯誤的對抗式計畫審查 |
| `ANALYST` | planning | 涵蓋投資報酬率、領域正確性與使用者影響的商業邏輯審查 |
| `DESIGNER` | planning | UX、UI 與 DevEx 審查 |
| `RELEASER` | planning | 釋出流程設計（唯讀） |

研究作業**在對話程序內執行，且不接受無頭路由**。`RESEARCHER` 角色缺乏 `executorRouting` 項目。`/gal research` 與 `/gal deep-research` 指令於對話迴圈內執行。有效的執行器包含 `claude`、`codex`、`opencode`、`copilot` 與 `agy`。省略角色會保留其於對話迴圈內執行。

> ⚠️ **SECURITY WARNING — bypass-permission.** 無頭執行器轉接器使用 `--dangerously-skip-permissions`（Claude Code、Antigravity/agy）、`--auto`（OpenCode）、`--allow-all`（Copilot）或 `-s workspace-write`（Codex）觸發次要 CLI。此舉授予對本機檔案系統與終端機的**完全信任**，並使沙盒保護失效。嚴格在受信任的機器與環境中啟動執行器路由。當儲存庫或代理程式合約源自未受信任的來源時，禁止啟動。規格禁止次要 CLI 執行 `git commit` 或 `git push`，這代表指示而非技術性強制執行。

### 遠端執行 (SSH 派送通道)

跨機器執行作為已解析路由的屬性運作，而非獨立指令。無論 `CODER`、`TESTER` 或 `AUDITOR` 解析為本機或遠端叫用，`/gal pipeline` 指令維持相同行為。將 `sshTarget` 與 `remoteWorkdir` 附加至管道角色項目中，以透過 SSH 執行該階段：

```json
"pipeline": {
  "TESTER": {
    "executor": "claude",
    "model": "claude-sonnet-4-6",
    "sshTarget": "user@build-box",
    "remoteWorkdir": "/home/user/gal-remote"
  }
}
```

這兩個欄位必須同時存在。只設定其中一個的項目在載入時會發出警告，並於派送期間明確報錯失敗。

**先決條件** — 直接在遠端機器上設定這些項目，GAL 不提供任何佈建：

- 對目標的無密碼 SSH 存取（`BatchMode=yes`）。互動式密碼或密碼短語提示屬無法連線狀態並防止重試。
- 被路由的代理程式 CLI（`claude`、`codex`、`agy`、`opencode`）需在遠端機器安裝與驗證。遠端目標不需要 `gal` 二進位檔。
- `remoteWorkdir` 中的 `git` 儲存庫必須符合控制節點簽出提交，並在每次派送前具有乾淨的工作樹。

**預期結果：** 派送在維持的 SSH 工作階段上同步執行，符合本機派送生命週期。成功的變更實作階段後，GAL 擷取未提交的遠端差異並套用至控制節點。GAL 嚴格在成功的本機套用後重置遠端簽出（`git reset --hard && git clean -fd`）至乾淨狀態，禁止逆向執行順序。本機套用失敗使遠端簽出維持未修改狀態以供檢查。

`remoteWorkdir` 必須專作**GAL 專用簽出**。避免針對互動式使用的簽出，因後置套用清理程序在設計上為破壞性運作。

**已知限制 (v1)：** copilot 代理程式無法遠端路由，因為它指定 CLI 旗標的方式無法跨 SSH 指令列轉發。此類路由會明確報錯失敗，而非靜默降級。Windows 遠端目標仍不支援，因為組成的指令需要 POSIX 登入 shell。此通道缺乏斷線存活功能，SSH 工作階段一旦中斷，派送即失敗，無法重新連線或續行。遠端通道以單次派送加手動重新同步的模式運作。控制節點 HEAD 會在任務提交後前進，而遠端簽出停在最後的同步點。同一次執行中的後續遠端派送會觸發防護，直到手動完成簽出同步為止。

不必動用實際的管道任務，可改用下述 [遠端 (SSH) 自我測試](#遠端-ssh-自我測試---transport-ssh) 確認就緒狀態。隨附的狀態表記錄僅限遠端的失敗條件與復原程序。

### 檢查派送

每次派送產生**兩個**持久追蹤。為任務選取合適的追蹤：

**第一層 — GAL 執行器記錄檔（稽核追蹤，跨所有工具一致）。** 每次執行記錄於 `.dev/executor-logs/<plan-slug>/` 或未定範圍的 `.dev/executor-logs/`，供直接派送使用。檔案遵循 `<timestamp>-<attempt>-<task>-<phase>-<executor>.log` 命名慣例。標頭記錄終端狀態、結束碼、實際模型、git 分支與 HEAD，以及提供者的 `session_id`。`---STDOUT---` 區塊擷取完整提供者事件流，涵蓋代理程式訊息、指令執行、檔案修改與權杖消耗指標。讀取此記錄檔稽核執行器動作。格式在所有五種工具中維持相同。提供者本機對話記錄提供諮詢用途，而此記錄檔構成官方儲存庫擁有的紀錄。

**讀取 `contract` / `contract_source` 欄位。** 針對提示檔或來源計畫輸入所建立的管道派送，會在 `Dispatch:` 標記列，以及執行器記錄檔的啟動與終端標頭上，附加 `contract=<control-node-abs-path> contract_source=workdir|ancestor|exe-side|embedded`，這三處的數值彼此耦合一致，因此任一處都能告訴你執行器實際依循的代理程式合約內容，以及來自哪個層級。`contract_source=workdir` 或 `ancestor` 代表由本機 GAL 檢出版本提供合約（行為顯得過時時應檢查該檢出版本）。`exe-side` 代表封裝版二進位檔自帶的內建副本。`embedded` 代表退回使用 `~/.gal/embedded-src` 具體化的備援副本。原始的 `gal dispatch` 與原始任務規格式 `gal pipeline` 執行會完全省略這些欄位，其缺席屬預期行為，並非缺陷。

**讀取受來源佐證閘控的 `effort` 欄位。** 僅限管道所建立的派送（存在來源佐證時），該次執行中每個成功／降級標記都會攜帶一個經過淨化的 ` effort=<value|(default)>` 欄位，緊接在 `contract`/`contract_source` 後綴之前，此欄位於路由解析完成後只計算一次，並在該次執行剩餘期間原封不動地重複使用。原始 / 直接派送，以及 `no-routing` 降級情形（未解析出路由，因此沒有來源佐證）從不攜帶 `effort` 欄位，其標記維持與加入 effort 前的格式逐位元組相同。當路由產生 `OFFLOAD` 區塊時，`gal dispatch-script` 也會預先算好一個 `REPORT_LINE` 欄位（`Dispatched: <phase[ (fix)]> <T-NN> - <ROLE> as <executor>, model <model>, effort <effort>`），協調器會在每個派送時機一字不差地公告恰好一次。

**第二層 — 提供者原生工作階段繼續（用於繼續/分支）。** 記錄檔標頭記錄可繼續的 `session_id`。使用此識別碼在原生的提供者 UI 內延續對話。指令**缺乏**一致性：

| 執行器 | 原生檢視 / 繼續指令 | 無頭工作階段的預設可見度 |
| --- | --- | --- |
| **claude** | `claude --resume <session_id>` | 列出 |
| **codex** | `codex resume <uuid>`（UUID 略過篩選器） | **隱藏** — 使用 `codex resume --include-non-interactive`（加上 `--all` 停用 cwd 篩選）在選取器中查看 |
| **opencode** | `opencode run -s <session_id>`（繼續） · `opencode export <session_id>`（傾印 JSON） · `opencode session list`（瀏覽） | 列出 |
| **copilot** | `copilot --resume=<session_id>` | 儲存於 `~/.copilot/session-store.db`，透過 ID 繼續（無公用列出指令） |
| **agy** | `agy --conversation <uuid>` | 儲存於 `~/.gemini/antigravity-cli/brain/<uuid>/`，無列出子指令 — 透過 ID 瀏覽 |

**一般準則：** 透過讀取第一層執行器記錄檔稽核派送。使用對應的第二層指令在原生工具內進行繼續作業。基於固有行為而非 GAL 設定，codex 工具預設特別隱藏無頭工作階段。

### 執行器自我測試 (`gal doctor --executor-smoke`)

標準 `gal doctor` 指令避免呼叫無頭編碼代理程式 CLI。`gal doctor --executor-smoke` 變體構成選擇性、可重複的自我測試，跨越五個支援的 CLI 執行即時管道任務所用的相同無頭派送路徑：`codex`、`claude`、`copilot`、`agy`、`opencode`。

```sh
# Self-test all five agents (default --transport local)
gal doctor --executor-smoke

# Filter to specific agents (repeatable)
gal doctor --executor-smoke --executor codex --executor claude

# CI/scheduler use: machine-readable JSON, non-zero exit on any non-pass row
gal doctor --executor-smoke --json --strict

# Bound the per-agent timeout (seconds, default 300)
gal doctor --executor-smoke --timeout 60
```

此指令會觸發真實的派送程式碼路徑，並使用一份與線上 `config.json#executorRouting` 設定互相獨立的合成路由檔案。它執行的是一個只負責寫入單行收據的最小任務。執行結果儲存於被 gitignore 忽略、具時間戳記的目錄中，預設為 `.dev/executor-smoke/runs/<utc-run-id>/local/`。輸出包含 JSON 報告、人類可讀的表格、各代理程式的記錄檔與收據。`.dev/executor-smoke/latest.json` 檔案持續指向最近期的執行。

**狀態定義：**

| 狀態 | 意義 |
| --- | --- |
| `PASS` | 實際派送已完成且收據經過驗證。 |
| `NOT_INSTALLED` | PATH 上無此 CLI。請安裝。 |
| `NOT_AUTHENTICATED` | 確認未經驗證。執行該工具的登入指令。 |
| `AUTH_UNKNOWN` | 無法確認就緒狀態。嘗試進行有界呼叫並回報實際結果。 |
| `UNSUPPORTED` | 五個支援代理程式以外的 `--executor` 名稱。永不派送。 |
| `CONFIG_ERROR` | 派送前的問題（通常是不安全的 `--report-dir`）。在任何寫入前捕捉。 |
| `CALL_FAILED` | 執行器以非零結束。檢查執行器記錄檔。 |
| `NO_RECEIPT` | 執行器以 0 結束但從未寫入收據。僅以 0 結束絕不構成成功。 |
| `TIMEOUT` | 超出有界逾時時間。若工具緩慢請放寬 `--timeout`，或調查卡住的互動式提示。 |

省略 `--strict` 旗標會強制執行完成時以 0 結束。報告才是絕對的事實來源，而非結束碼。此工具作為可重複執行的 CLI 指令運作，而非背景服務。可手動執行、整合進 CI 流程，或透過作業系統排程器觸發。

#### 遠端 (SSH) 自我測試 (`--transport ssh`)

此自我測試透過 SSH 驗證遠端機器上的編碼代理程式就緒狀態。在改變傳輸機制的同時，維持相同的報告結構描述與狀態。安裝與驗證探測**在遠端機器上執行**，不可由控制節點推斷。

```sh
gal doctor --executor-smoke --transport ssh --ssh-target <ssh-target> --remote-workdir <dedicated-checkout>

# CI/scheduler use
gal doctor --executor-smoke --transport ssh --ssh-target <ssh-target> --remote-workdir <dedicated-checkout> --strict --json
```

`--ssh-target` 變數定義支援非互動式、金鑰存取的 SSH 主機（`BatchMode=yes`）。`--remote-workdir` 變數為**必要**，且指定與控制節點 git HEAD 相符並具備乾淨狀態的專屬 GAL 簽出。產生的報告輸出至 `.dev/executor-smoke/runs/<utc-run-id>/ssh/`。

**遠端專屬狀態：**

| 狀態 | 意義 | 修正方式 |
| --- | --- | --- |
| `SSH_UNREACHABLE` | 無法開啟非互動式 SSH 工作階段（在任何執行器探測前檢查）。 | 檢查目標、網路與金鑰驗證。 |
| `REMOTE_GUARD_FAILED` | 遠端工作目錄遺失、不安全、不乾淨，或不在控制節點 HEAD。 | 重新同步專屬遠端簽出至控制節點 HEAD，並確保乾淨狀態。 |
| `REMOTE_FETCH_FAILED` | 遠端行程可能已執行，但擷取其收據失敗。 | 檢查執行器記錄檔與遠端收據路徑。 |

為符合遠端限制，遠端 copilot 報告產生 `UNSUPPORTED` 狀態。此狀態既不計為通過，也不會被省略。`PASS` 狀態嚴格要求執行終結完成**並**成功擷取到非空白的收據。

## Git 輔助工具

### `gal commit-msg`

`gal commit-msg` 指令作為決定性提交輔助工具運作，支援 `git-commits` 技能與 `git-commit-msg` 指令。類型與範圍分類**僅**源自變更的檔案路徑與 git 狀態。它忽略差異與內文關鍵字，防止訊息內容劫持分類。存在三種模式：

- **`gal commit-msg --context`** 輸出精簡的已暫存變更脈絡，包含檔案、決定性類型與範圍基準標頭、已暫存的計畫與提示檔摘要，以及專供起草訊息的代理程式使用的 hunk 標頭。此模式以更低的權杖成本，提供比原始差異更高的訊號品質。
- **`gal commit-msg --print`** 僅輸出決定性類型與範圍主旨標頭。
- **`gal commit-msg <file>`** 作為 git commit-msg 掛鉤運作。它根據已暫存變更填入空白訊息，並嚴格避免覆寫作者編寫的內容。

`git-commit-msg` 指令僅產生訊息字詞，避免執行 git commit。`git-commits` 技能產生訊息**並**在偵測到明確提交意圖時執行提交。

### Git 篩選器 (`gal clean` / `gal smudge`)

選用的 `gal-config` git 篩選器消除了受追蹤檔案中的本機設定值。為每個儲存庫套用註冊：

```bash
git config filter.gal-config.clean 'gal clean'
git config filter.gal-config.smudge 'gal smudge'
```

git 執行檔於內部叫用此篩選器。`gal` 二進位檔需存在於有效的 git `PATH` 上，以防因篩選器錯誤造成 `git commit` 失敗。

## 撰寫輔助工具

### `text-flowcharts`

[`text-flowcharts`](../../../plugins/gal-core/skills/text-flowcharts/SKILL.md) 技能會把分支邏輯、管線與多步驟流程繪製成等寬純文字的決策樹圖。在 Claude Code 裡以 `/text-flowcharts` 呼叫，在 Codex 裡以 `$text-flowcharts` 呼叫。當你的說明在追蹤單筆記錄的控制流程，或是你要求繪製 flowchart、流程圖、邏輯圖時，它也會自動啟用。

每張圖都從頂端的入口開始，跟著單一筆記錄一路往下穿過它所遇到的條件，最後落在一個分級的終端結果，讓讀者可以丟入一筆資料就看清它會停在哪裡。這套詞彙刻意保持精簡：方框代表步驟，大括號代表判斷，而每個葉節點都帶著一個結尾標記。

| 元素 | 符號 |
| --- | --- |
| 流向線與轉角 | `│ ─ ┌ ┐ └ ┘` |
| 接點（分岔、匯流、交叉） | `├ ┤ ┬ ┴ ┼` |
| 箭頭（下、上、右、左） | `▼ ▲ ▶ ◀` |
| 終端成功 | `√` |
| 刻意跳過 | `>>\|` |
| 死路或遭拒 | `×` |

這些符號在預設等寬字型裡即可顯示，不需另裝字型，對非 CJK 讀者而言各佔一格，並且能在 UTF-8 環境下的 pull request 留言、程式碼註解與終端機中原樣保留。輸出不含 emoji，因此在較舊的機器上也能維持易讀。只有當流程確實會分支時才使用這個技能，若是一連串沒有判斷的直線步驟，改用編號清單會更好讀。

## Golem 代理程式

GAL 專家代理程式作為 golem 運作。本節從**使用者面向**提供關於功能、差異與應用的觀點。具權威性的名冊與分類位於 [`plugins/gal-core/agents/agents.md`](../../../plugins/gal-core/agents/agents.md)。工作流程語意位於 [`plugins/gal-core/workflows/coding.md`](../../../plugins/gal-core/workflows/coding.md)。

### 能力表

| Golem | 功能 | 使用時機 |
| --- | --- | --- |
| `golem-architect` | 對抗式計畫審查，涵蓋取捨、過度設計、潛在錯誤面，與相依性/API 風險 | 建置前使用 `/deep-planning` 或 `/gal architect` 進行設計壓力測試 |
| `golem-analyst` | 商業邏輯審查，涵蓋投資報酬率、領域正確性，與使用者影響 | 當變更影響定價、權限、資格或客戶可見規則時使用 |
| `golem-designer` | UI/UX 體驗設計、DevEx、設計系統、無障礙性，與即時 UI 稽核 | 用於面向客戶的排版、狀態、元件工作，或面向開發者的 DevEx |
| `golem-researcher` | 本機優先 (local-first) 調查、跨來源合成，與可供參考的調查結果 | 使用 `/gal research` 或 `/gal deep-research` 尋找以證據為基礎的答案 |
| `golem-implementer` | 透過原子提交為已核准任務編寫實作程式碼 | 代表 `/gal pipeline` 中的 CODER 階段 |
| `golem-tester` | 僅由計畫規格與公開 API 衍生的規格導向測試與真實瀏覽器 QA | 代表以不同模型推動獨立驗證的 TESTER 階段 |
| `golem-auditor` | 執行單一任務與整個分支的深度效能與安全性稽核 | 代表 `/gal pipeline`（任務稽核）或 `/gal finalize`（分支稽核）中的 AUDITOR 階段。此角色僅限協調器驅動，不可裸呼叫 `/gal auditor` |
| `golem-debugger` | 進行採用凍結紀律與根源確認的科學方法錯誤調查 | 當錯誤需要在修復前先做有紀律的調查時使用 |
| `golem-steward` | 管理文件結構、程式碼與文件的偏移、知識擷取，與圖表同步 | 使用 `/gal steward`，或於計畫開啟、細化結束或管道結案時自動觸發 |
| `golem-releaser` | 透過 API 與 CICD 研究設計規劃階段的釋出流程，並提出設計建議 | 在 `/planning release-<slug>` 之前使用 `/gal releaser`（隔離）或 `/gal discuss releaser`（脈絡內） |

**檢查角色三角：** 品質保證職責形成三方結構。**ORCHESTRATOR**（管道）掌管個別任務的正確性關卡、執行結束時的目標回推驗證，以及計畫生命週期的關閉。**AUDITOR** 管理單一任務深度效能與安全性。**STEWARD** 控制文件結構。

### 角色叫用矩陣

| 角色 | 可直接呼叫？ | 模式 |
| --- | --- | --- |
| **architect** | 是 | `/gal architect`（隔離）或 `/gal discuss architect`（脈絡內） |
| **analyst** | 是 | `/gal analyst`（隔離）或 `/gal discuss analyst`（脈絡內） |
| **designer** | 是 | `/gal designer`（隔離）或 `/gal discuss designer`（脈絡內） |
| **releaser** | 是 | `/gal releaser`（隔離）或 `/gal discuss releaser`（脈絡內），規劃設計師，不執行 |
| **debugger** | 是 | `/gal debugger` |
| **steward** | 是 | `/gal steward` |
| **implementer** | **僅限協調器驅動** | 僅限透過 `/gal pipeline`（管道階段脈絡） |
| **tester** | **僅限協調器驅動** | 僅限透過 `/gal pipeline`（管道階段脈絡） |
| **auditor** | **僅限協調器驅動** | 透過 `/gal pipeline`（管道階段）或 `/gal finalize`（分支稽核） |
| **researcher** | **僅限協調器驅動** | 僅限透過 `/gal research` 或 `/gal deep-research` |

若缺乏相符的協調脈絡就執行 `/gal <role>`，這四種僅限協調器驅動的角色都會產生 `COMMAND: error`。

### 諮詢雙模式 (`/gal discuss <role>`)

architect、analyst、designer 與 releaser 角色僅支援兩種叫用模式。其餘角色不支援討論格式。

| 模式 | 觸發方式 | 行為 | 回應標籤 |
| --- | --- | --- | --- |
| **隔離** (預設) | `/gal <role>` | 原生子代理程式隔離執行該角色。僅將裁決與摘要傳回主脈絡 | `[<role> · isolated]` |
| **脈絡內** | `/gal discuss <role>` | 角色啟動核心載入至目前對話。助理從先前的隔離裁決熱加入，並進行多輪對話直至主題變更 | `[<role> · in-context]` |

**熱加入：** 對話記錄中已包含隔離模式的裁決，因此脈絡內模式能原生接續進度，不必重新執行整個角色。

Codex 執行環境將 `/gal discuss <role>` 對應為 `$discuss-<role>`。Claude 執行環境則直接以斜線指令實作 `/gal discuss <role>`。

整合的 [`adversarial-review`](../../../plugins/gal-core/skills/adversarial-review/SKILL.md) 技能提供這些審查角色所使用的、不分目標的審查方法。它強制要求強化論證 (steel-man)、預設反駁邏輯、證據紀律，以及明確的 APPROVE、REVISE 或 REJECT 裁決。

### Steward 生命週期分割

steward 於規劃與落地階段執行，進行不同操作。這些階段維持嚴格不可互換：

- **規劃階段 (`/deep-planning`)：專注於計畫文件結構。** steward 評估計畫檔案屬性，包含路徑、代稱、強制區段、語言一致性與圖表同步。禁止在此階段寫入 `docs/`。規劃階段的文件缺乏實作與持久知識。寫入 `docs/` 有使未建置的推測性內容汙染讀者可見層的風險。
- **落地階段 (`/gal finalize`)：持久知識落地。** 已完成計畫的持久知識嚴格要求落地於 `docs/` 中。steward 將已實作的計畫知識擷取至持久層（`README.md` 與 `docs/`）。隨後，索引同步至 `.dev/project.md`。此落地程序具強制性，而非建議性。刪除計畫檔案要求 steward 必須先產生持久層提交。

此結構分割確認知識擷取需要建置好的知識，而這僅能在實作後取得。因此，將完成的計畫寫入正式文件維持為落地階段操作。規劃階段的 steward 嚴格專注於維持計畫文件格式。

### 釋出計畫通道

釋出構成不同的計畫類型（`release-<slug>`），並獨立於 `/gal pipeline` 階段與 `/gal finalize` 步驟運作。

**流程：**

1. `/gal releaser` 指令在解析來自唯讀來源（包含技能、API 與 CLI 設定檔）的本機可用部署能力前驗證部署目標。隨後設計釋出與 devops 流程。此操作嚴格為唯讀，避免寫入檔案、提交或執行。無法識別的能力會產生無法使用的報告，防止偽造。
2. `/planning release-<slug>` 指令將產生的建議具現化為包含 `## Tasks` 區段的來源計畫。
3. 標準 `/gal pipeline` 指令執行已完成的釋出計畫。
4. `/gal finalize` 指令落地釋出計畫，與標準計畫相同。

此程序支援 GAL 自身 CLI 二進位釋出，以及下游儲存庫部署（例如，網頁服務、後端服務、firebase 類、npm 套件、Docker 映像檔）。操作範圍止於設計建議。releaser 禁止作為部署協調器，並省略金絲雀釋出 (canary)、回退 (rollback) 與正式環境監控功能。
