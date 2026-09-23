---
source: docs/setup.md
lang: zh-Hant
source_commit: 4057c591bf08bc7ee0269f2cbee82bcac3850fa9
translated_at: 2026-09-21
type: Guide
title: 安裝與初始化
description: 安裝 gal 二進位檔、向各執行環境註冊外掛，並初始化與維護儲存庫轉接器。
tags:
  - setup
  - install
  - runtime
  - init
status: stable
---

# 安裝與初始化 (Setup and Initialization)

## 安裝 GAL

請依您的作業系統與使用需求選擇合適的安裝方式。安裝完成後，即可在儲存庫內執行 `gal init` 產生儲存庫本機轉接器 (adapter)，詳見後續章節說明。

### 安裝選項

#### `cargo install --git`（從原始程式碼編譯）

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli
```

GAL 未上架至 crates.io，因此 `--git` 旗標為必要引數。在 Cargo 工作區中必須明確指定 `gal-cli` 套件名稱。編譯完成後產生的二進位檔名稱為 `gal`。

#### Homebrew（macOS 與 Linux）

```bash
brew install monkey1wizard/tap/gal
```

此為 macOS 與 Linux 平台上最推薦的套件管理員安裝管道。

#### winget（Windows，請先探測可用性）

```sh
winget install Monkey1Wizard.GAL
```

由於套件目錄索引的更新可能落後於實際版本釋出，安裝前請先使用 `winget show Monkey1Wizard.GAL` 確認版本可用性。在 Windows 環境下請注意 PowerShell 別名遮蔽問題。

#### 直接下載

請造訪 GitHub Releases 頁面下載符合您平台的封存檔 `gal-<version>-<platform>-<arch>[.zip|.tar.gz]`。解壓縮後將二進位檔 `gal`（Windows 下為 `gal.exe`）置於系統的 `PATH` 目錄中。在 macOS 與 Linux 環境下，請確認已使用 `chmod +x gal` 賦予執行權限。

#### curl 安裝指令碼（Linux 與 macOS）

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
```

安裝指令碼會依據 `checksums.txt` 嚴格比對 SHA-256 總和檢查碼，若雜湊不符將立即中止安裝。完成雜湊比對後會接續進行 cosign 無金鑰簽章驗證。簽章驗證的可用性採盡力而為，但結果判定維持嚴格標準：若系統未安裝 cosign 或簽章檔案下載失敗，指令碼將發出警告並僅憑 SHA-256 結果繼續安裝。若 cosign 成功執行但回報簽章不符，安裝流程會在解壓縮與寫入磁碟前立即中止。二進位檔將安裝至 `~/.local/bin`，GAL 來源內容則存放於 `~/.local/share/gal`。若需安裝特定版本，可設定環境變數 `GAL_VERSION`。

#### irm 安裝指令碼（Windows）

```sh
irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
```

此安裝管道具備與 curl 指令碼相同的 SHA-256 與 cosign 驗證邏輯，簽章不符時同樣會強制中止。安裝程式會將 `gal.exe` 與來源內容安裝至 `%LOCALAPPDATA%\Programs\gal`（請將此路徑新增至使用者 `PATH`，安裝程式會顯示相應指令）。若需指定版本，請設定 `$env:GAL_VERSION`。

**PowerShell 別名遮蔽注意**：PowerShell 內建的別名 `Get-Alias gal`（指向 `Get-Alias` 本身）可能會遮蔽 GAL 二進位檔。若輸入 `gal` 時解析為其他指令，請改用絕對路徑呼叫，或在設定檔中移除該別名。

### 升級與版本維護

執行 `gal update` 指令可檢視目前已安裝的版本資訊以及平台專屬升級指示。GAL 不提供二進位檔自我更新功能，升級作業必須經由原始安裝管道執行：

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli  # cargo
winget upgrade Monkey1Wizard.GAL                                                   # Windows
brew upgrade gal                                                                   # macOS / Linux
```

直接執行 `gal`（不帶任何引數）會印出使用說明並以結束碼 `0` 正常結束。若輸入無法辨識的子指令，則會以結束碼 `64` 回報錯誤。

## 依執行環境註冊外掛

外掛註冊是支援 Markdown 外掛容器之執行環境 (runtime) 的主要載入路線。所有已註冊的執行環境皆直接讀取由 `gal refresh` 算繪於標準根目錄 (canonical root) `~/.gal/plugins/gal/` 中的內容。若執行環境缺乏 Markdown 外掛容器（目前僅有 opencode），則改用檔案投影 (projection) 作為後備方案。

為避免外掛提供的資產與本機投影檔案產生重複的技能或代理程式清單，每個執行環境設定完成後的標準收尾動作，都是在 `~/.gal/config/config.json` 中啟用對應的 `pluginMode.<runtime>` 並執行 `gal refresh`。

### Claude Code

Claude Code 將標準根目錄掛載為即時的 skills-directory 外掛：

1. 建立指向標準根目錄 `~/.gal/plugins/gal` 的 `~/.claude/skills/gal` 接合點 (junction) 或符號連結：
   - Windows（PowerShell）：
     ```powershell
     New-Item -ItemType Junction -Path "$env:USERPROFILE\.claude\skills\gal" -Target "$env:USERPROFILE\.gal\plugins\gal"
     ```
   - macOS / Linux：
     ```bash
     ln -s ~/.gal/plugins/gal ~/.claude/skills/gal
     ```
2. 執行 `claude plugin list`，確認清單中列出 `gal@skills-dir` 且狀態為已載入。在此模式下，Claude Code 會直接就地讀取接合點目標，不使用快取。
3. 在 `~/.gal/config/config.json` 中啟用 Claude 外掛模式：
   ```json
   {
     "pluginMode": {
       "claude": true
     }
   }
   ```
4. 執行 `gal refresh`。當 `pluginMode.claude` 為 true 時，GAL 會跳過將指令檔案寫入 `~/.claude/commands/*.md` 並清理既有複本，避免與外掛技能清單重複。

市集安裝管道僅作為次要路線，供使用 `claude plugin add` 安裝快照版本的使用者使用，該方式會將檔案複製至 `~/.claude/plugins/cache/<marketplace>/gal/<version>/`。

### OpenAI Codex

Codex 經由本機外掛市集目錄註冊 GAL：

1. 註冊本機外掛市集目錄：
   ```bash
   codex plugin marketplace add ~/.gal/plugins
   ```
   此步驟會讀取 `gal refresh` 算繪產生的 `~/.gal/plugins/.agents/plugins/marketplace.json`。
2. 新增外掛：
   ```bash
   codex plugin add gal@gal
   ```
   Codex 會將外掛檔案複製至 `~/.codex/plugins/cache/gal/gal/<version>/`。
3. 執行 `codex plugin list`，確認清單中包含 `gal@gal`。
4. 在 `~/.gal/config/config.json` 中啟用 Codex 外掛模式：
   ```json
   {
     "pluginMode": {
       "codex": true
     }
   }
   ```
5. 執行 `gal refresh`。

**共用 `~/.agents/skills` 注意事項**：Codex 與 opencode 共用 `~/.agents/skills` 目錄。若本機未啟用 opencode，設定 `pluginMode.codex: true` 會停止將核心技能投影至 `~/.agents/skills` 並清理既有檔案。若同一台機器同時使用 Codex 與 opencode，GAL 會持續將核心技能投影至 `~/.agents/skills` 以維持 opencode 運作，此時 Codex 將同時看見外掛技能與投影技能。

### GitHub Copilot

GitHub Copilot 將 GAL 視為 Agent Plugins 1.0.0 套件取用：

1. 在 Copilot 設定中將 `~/.gal/plugins` 註冊為目錄型市集（讀取 `~/.gal/plugins/.claude-plugin/marketplace.json`），並啟用 `gal`。Copilot 會直接就地載入外掛而不複製檔案，從 `com.github.copilot/agents/` 載入代理程式、從 `com.github.copilot/rules/` 載入規則，並從 `mcp.json` 載入 MCP 設定。
2. 執行 `copilot plugin list`，確認 `gal` 已成功啟用。
3. 在 `~/.gal/config/config.json` 中啟用 Copilot 外掛模式：
   ```json
   {
     "pluginMode": {
       "copilot": true
     }
   }
   ```
4. 執行 `gal refresh`。當 `pluginMode.copilot` 為 true 時，GAL 會停止將代理程式投影至 `~/.copilot/agents/*.agent.md` 並清理既有檔案。Copilot 指令技能仍會保留於 `~/.copilot/skills/`，因為 Copilot 不會自外掛的 `commands/` 載入指令。

### Google Antigravity

Antigravity 經由本機目錄探索機制安裝 GAL：

1. 自標準根目錄安裝外掛：
   ```bash
   agy plugin install ~/.gal/plugins/gal
   ```
   此指令會在 `~/.gemini/antigravity-cli/plugins/gal` 建立指向標準根目錄的接合點。
2. 清理重複的 `claude-code` 匯入項目：若 Antigravity 先前曾從 Claude Code 匯入過外掛，`agy plugin list` 可能會顯示兩筆 `gal` 項目（分別標示 `local-install` 與 `claude-code`）。由於 `agy` 未提供自 `import_manifest.json` 移除特定項目的指令，請手動開啟 `~/.gemini/config/import_manifest.json`，在 `imports` 陣列中移除 `claude-code` 物件，僅保留 `local-install` 項目。接著再次執行 `agy plugin list`，確認只留下一筆 `gal` 項目。
3. 在 `~/.gal/config/config.json` 中啟用 Antigravity 外掛模式：
   ```json
   {
     "pluginMode": {
       "agy": true
     }
   }
   ```
4. 執行 `gal refresh`。當 `pluginMode.agy` 為 true 時，GAL 會跳過將指令技能投影至 `~/.gemini/antigravity-cli/skills/` 並清理既有檔案。

### opencode（投影後備方案）

opencode 不使用 Markdown 外掛容器，其擴充架構仰賴經由 npm 或本機安裝的 JavaScript 與 TypeScript 模組。因此 opencode 採用 GAL 的檔案投影後備機制。執行 `gal refresh` 會自動將原生 Markdown 指令投影至 `~/.config/opencode/commands/`、代理程式投影至 `~/.config/opencode/agents/`，並將核心技能投影至 `~/.agents/skills/`。

### 次要路線：對話引導式安裝

若使用者經由 Claude Code 或 Codex 外掛市集快照分支發現 GAL，可使用對話引導方式完成初始安裝：

1. 在 Claude Code 或 Codex 外掛市集中搜尋並安裝 **GAL 外掛**（搜尋關鍵字「gal」），或自市集快照分支取得。
2. 在對話中向編碼代理程式 (coding agent) 提出 **"help me install gal"**。外掛內建的 `install-gal` 技能會主動徵求使用者同意，接著叫用對應的套件管理工具（Homebrew、winget 或 `cargo install --git`）安裝二進位檔，完成驗證後在未初始化的儲存庫中執行 `gal init`（若儲存庫已初始化則改為執行 `gal render-adapters`）。

單獨安裝市集外掛無法構成完整的 GAL 運作環境。所有工作流程指令皆以本機 `gal` 二進位檔為基礎，因此安裝執行檔為不可省略的步驟。安裝完成後，建議轉換為上述的主要本機外掛註冊方式。

## 儲存庫初始化與轉接器維護

### 首次初始化（`gal init`）

在全新未初始化的儲存庫中，`gal init` 會自 `gal-core` 範本產生兩個儲存庫本機轉接器根目錄（`AGENTS.md` 與 `CLAUDE.md`），並建立 `.dev/` 目錄結構（包含 `.dev/project.md` 與 `.dev/state.md`）。若儲存庫處於已初始化或半初始化狀態，`gal init` 會拒絕執行且不寫入任何內容，並以結束碼 `1` 退出。

| 狀態 | `.dev/project.md` | `.dev/state.md` | `gal init` 行為 | 結束碼 |
| --- | --- | --- | --- | --- |
| 未初始化 | 不存在 | 不存在 | 全新啟動，自範本寫入這兩個檔案並算繪轉接器 | 0 |
| 已初始化 | 存在 | 存在 | 報錯，指出 `.dev/project.md` 已存在，提示改用 `gal render-adapters` 或刪除後重試 | 1 |
| 半初始化 | 存在 | 不存在 | 報錯，指出缺少 `.dev/state.md`，提示自版本控制還原或刪除後重試 | 1 |
| 半初始化 | 不存在 | 存在 | 報錯，指出缺少 `.dev/project.md`，提示自版本控制還原或刪除後重試 | 1 |

已初始化與半初始化狀態下的錯誤訊息如下：

```text
gal init: this repository is already initialized (.dev/project.md exists).
  To regenerate AGENTS.md and CLAUDE.md from .dev/project.md, run: gal render-adapters
  To start over from the templates, delete .dev/project.md and .dev/state.md, then run gal init again.
```

```text
gal init: this repository is half-initialized: .dev/project.md exists but .dev/state.md is missing.
  Restore .dev/state.md from version control, or delete .dev/project.md and run gal init again.
  gal init does not overwrite .dev/project.md.
```

### 重新產生轉接器（`gal render-adapters`）

在已初始化的儲存庫中，`gal render-adapters` 會以 `.dev/project.md` 為單一來源重新產生儲存庫本機轉接器。

| 狀態 | `.dev/project.md` | `gal render-adapters` 行為 | 結束碼 |
| --- | --- | --- | --- |
| 已初始化 | 存在 | 重新算繪 `AGENTS.md`、`CLAUDE.md` 與條件層，並清理退役根目錄，每條路徑輸出一列 | 0 |
| 未初始化 | 不存在 | 報錯：`gal render-adapters: .dev/project.md not found. Run gal init first.` | 1 |

執行 `gal render-adapters` 時完全不會讀取或檢查 `.dev/state.md`。

### 轉接器維護與退役清理

維護既有儲存庫或從早期產生五個轉接器根目錄的版本升級時，執行 `gal render-adapters` 會自動協調本機檔案並執行退役清理移轉：

- **退役根目錄清理**：舊版的三個橋接根目錄（`GEMINI.md`、`.github/copilot-instructions.md` 與 `.agents/rules/gal.md`）已正式退役。在產生或重新整理轉接器時，GAL 會檢查這些路徑。若檔案第一行帶有完全相符的 GAL 產生標記，GAL 會將其刪除並標記為 `pruned (GAL-owned)`。若刪除後使 `.agents/rules/` 或 `.github/` 成為空目錄，該目錄也會一併移除。
- **保留手動編輯檔案**：位於退役路徑上的檔案若缺乏 GAL 產生標記，將視為使用者自訂內容。GAL 會予以保留不加變動，並標記為 `kept (hand-owned)`。
- **條件層管理**：若 `.dev/project.md` 中啟用了 Rust 慣例，條件層檔案（`.claude/rules/gal-rust.md` 與 `.github/instructions/gal-rust.instructions.md`）會與兩大轉接器根目錄同步更新。若停用 Rust 慣例，過時的 GAL 擁有條件層檔案將被自動移除。

### 專案摘要檔案（`.dev/project.md`）

`.dev/project.md` 是專為轉接器算繪而設計的精簡專案摘要。檔案中**嚴格要求必須且只能出現一次**以下八個 H2 區段：`What This Is`、`Tech Stack`、`Architecture`、`Constraints`、`Response Style`、`Freshness`、`Project Language`、`Protected Paths`。

此檢查採取 fail-closed（預設阻擋）原則。因為 `gal init` 是以範本產出此檔案，這道防線主要防範手動編輯時發生的結構缺損。若有區段遺漏或重複，`gal render-adapters` 會拒絕整個算繪作業，明確指出有問題的標題並中止寫入。每次執行僅會回報一個錯誤標題。若缺失多個區段，請依據 `plugins/gal-core/templates/project.md` 的格式逐一補齊後重試。

`.dev/project.md` 設有嚴格的容量上限（`PROJECT_MD_MAX_BYTES`）。若因檔案過大而遭拒，必須精簡文字內容而非重複嘗試。算繪流程絕不會為了遷就檔案大小而寫入不完整的轉接器集合。

在 `Tech Stack` 表格下方，`.dev/project.md` 包含 `<!-- gal:authoritative-check -->` 標記與隨後的 `json` 程式碼圍籬，用於定義 `{"command": [...]}` 結構。`command` 陣列中的每個元素會依空白切分並原樣執行，不進行 shell 變數內插。由於產生的行程不會另外指定工作目錄，所有指令會直接繼承呼叫 `gal finalize-check` 者的工作目錄，不會自動切換至儲存庫根目錄。若為沒有建置或測試指令的純文件儲存庫，可將陣列內容設為 `["true"]`。`gal finalize-check` 關卡會在最終管道驗證時依據此圍籬執行檢查。

## 第一次 Doctor 檢查

`gal doctor` 指令用於全面檢查本機安裝環境的健康狀態，涵蓋二進位檔、標準根目錄、執行環境介面與本機設定。建議在初次安裝後、版本升級後，或是編碼代理程式 (coding agent) 無法辨識 GAL 指令時執行。

`gal doctor` 的主要檢查項目包含：

- **opencode 投影漂移**：偵測 `~/.config/opencode/` 下的指令與代理程式檔案是否與標準根目錄的算繪結果脫節。過期內容會發出警告，遺漏檔案則會回報錯誤，可執行 `gal refresh` 進行修復。
- **Claude 技能介面**：當 `~/.claude/skills/gal` 缺失時發出警告（非錯誤），並提供手動建立指示。`gal refresh` 不會自動建立此介面。
- **轉接器容量建議**：在已初始化的儲存庫內執行時，會額外顯示轉接器根目錄（`AGENTS.md` 與 `CLAUDE.md`）的容量報告。轉接器檔案若超出建議大小只會標記 `[WARNING]` 警告，不會中斷正常操作。

標準的 `gal doctor` 不碰任何無頭編碼代理程式 CLI。要測試這些 CLI，請改用選擇加入的 `gal doctor --executor-smoke`，它會走與實際管道任務完全相同的無頭派送路徑，逐一驗證 `codex`、`claude`、`copilot`、`agy` 與 `opencode` 五支 CLI 是否已安裝、已登入且能寫出收據。加上 `--transport ssh` 可以改測遠端機器，加上 `--json --strict` 可供 CI 判讀。無頭派送的角色路由與執行器設定，請參閱 [configuration.zh-Hant.md](./configuration.zh-Hant.md#執行器路由-executorrouting)，實際派送流程請參閱 [workflows.zh-Hant.md](./workflows.zh-Hant.md#管道任務迴圈與角色)。

## 執行環境指令觸發與更新機制

### 各執行環境指令觸發方式

GAL 指令依各執行環境特有的機制初始化，因此在不同工具中的呼叫語法略有差異：

| 執行環境 | 觸發語法 | 說明 |
| --- | --- | --- |
| Claude Code | `/gal status` | 原生外掛指令 |
| Codex | `/gal-status` | 作為技能公開（也可使用 `$` 字首，或經由 `/skills` 叫用） |
| GitHub Copilot | `/gal-status` | 作為技能公開（可經由 `/skills list` 檢視） |
| Google Antigravity | `/gal-status` | 作為技能公開（Antigravity 無原生 `commands/` 目錄，指令皆為代理程式技能） |
| opencode | `/gal-status` | 原生 Markdown 指令 |

請特別注意語法格式：非 Claude 執行環境皆使用帶連字號的 `gal-status`，而非帶空白的 `gal status`。

### 指令變更生效時機

指令與技能檔案更新後的生效時間，取決於各執行環境的載入機制：

- **Claude Code**：經由 `gal@skills-dir` 接合點即時讀取標準根目錄，不具本機快取，因此指令與技能變更會在下一個對話回合中立即生效。（若使用次要市集複製模式，則需重新啟動 Claude Code 方能載入）。
- **Google Antigravity**：於程式啟動時載入指令，變更後需**重新啟動 agy**。
- **GitHub Copilot 與 opencode**：在每個新的工作階段直接讀取指令與技能檔案。
- **OpenAI Codex**：在同一個作用中的交談對話串內會自動偵測技能變更。若更新未能順利顯示，可嘗試**重新啟動 Codex 或開啟全新對話串**。

**支援邊界提示**：GAL 已全面停止向 Gemini CLI 投影指令或技能。原 Gemini CLI 使用者應移轉至 Antigravity，其指令介面位於 `~/.gemini/antigravity-cli/skills/<name>/SKILL.md`。

Codex 的關鍵載入行為：

- **脈絡預算省略非故障**：Codex 對初始技能清單長度設有限制。超出預算時會先縮短說明文字，隨後將部分技能自清單中省略。被省略的技能依然可以直接以 `$skill-name` 完整叫用。
- **同名技能雙重列出**：若有多個工具投影了相同名稱的技能，Codex 不會進行去除重複，兩筆項目皆可能出現在技能選擇器中。
