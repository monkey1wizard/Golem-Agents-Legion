---
source: docs/setup.md
lang: zh-Hant
source_commit: 2a697bc89e4415d86d95ab79f9cb9c50c591fbfa
translated_at: 2026-09-30
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

完成外掛註冊後，請執行 `gal refresh`。GAL 會偵測本機註冊狀態。若註冊不存在或無法確認，GAL 會保留退回檔案，並在狀態不明時發出警告。

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
3. 執行 `gal refresh`。只有確認 Claude 已註冊時，外掛才會接管指令，GAL 才會省略指令退回檔案。註冊狀態不明時，GAL 會保留退回檔案並發出警告。

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
4. 執行 `gal refresh`。確認 Codex 註冊後，外掛會接管核心技能。GAL 仍會投影 TOML 代理程式與指令。

Codex 管道 hook 會包含在 Codex 外掛資訊清單的 `.codex-plugin/plugin.json#hooks` 中。GAL 不會安裝根目錄的預設 hook，也不會將 hook 加入 Claude Code 資訊清單。重新整理後，請確認 Codex 外掛資訊清單，並在 Codex 中信任 GAL 外掛，然後再依賴受保護接續功能。信任狀態與 hook 執行結果屬於執行環境事實。`gal doctor` 可回報靜態套件內容與指令就緒狀態，但只有即時的同一工作階段交握，才能證明 Codex 確實執行了受信任的 hook。

**共用 `~/.agents/skills` 注意事項**：Codex 與 opencode 共用 `~/.agents/skills`。選取 opencode 時，GAL 會持續投影此共用目錄，即使 Codex 註冊已確認也一樣，以保留 opencode 所需技能。

## Codex 管道執行設定

本節設定程序以 Codex CLI 0.156.1 為準，此為 2026-09-24 驗證之版本。若使用其他版本，請確認 Codex 官方文件與實際行為。官方設定名稱與權限邊界請參閱 [permission modes](https://learn.chatgpt.com/docs/permission-modes)、[agent approvals and security](https://learn.chatgpt.com/docs/agent-approvals-security) 與 [configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference)。

### 共用先決條件

1. 安裝 GAL 並確認 Codex CLI 完整安裝。在實際執行派送的 shell 環境中執行：

   ```pwsh
   gal.exe --version
   codex --version
   codex doctor
   Get-Command gal.exe -All
   Get-Command codex -All
   ```

   在其他 shell 中，請以該環境的指令查詢工具，確認實際解析的執行檔路徑。在 Windows 上，切勿為了解決查詢失敗而修改系統 `PATH`。若 PowerShell 將 `gal` 解析為內建別名，請直接呼叫 `gal.exe` 或指定完整路徑。
2. 另行執行 `codex login` 完成登入。切勿直接檢視或揭露權杖檔案。請一併檢查預定任務路由中的所有執行器，不要只檢查 Codex。
3. 開啟預定的受信任工作區。將網路存取限制在必要目標。僅在診斷明確指出特定路徑需要寫入時，才新增可寫入目錄。
4. 執行 `gal doctor`，接著在拋棄式測試目錄中執行 `gal doctor --executor-smoke --executor codex`。請注意 Smoke 測試會消耗供應商配額。此測試可驗證 CLI 就緒狀態與收據處理，但無法證明完整管道、獨立稽核或 Desktop 權限模式能正常運作。
5. 獨立確認 `executorRouting` 與 `timeoutSecs`，切勿將其與 Codex 核准設定混淆。請參閱[執行器路由](./configuration.zh-Hant.md#執行器路由-executorrouting)。
6. 僅對全新儲存庫執行初始化。若有多個作用中的計畫，請明確指定目標執行提示檔。

### Codex 受保護接續就緒條件

受保護的 `codex_stop_v1` 設定檔需要受信任的 GAL Codex 外掛，以及即時的同一工作階段交握。模型名稱、環境推測、過期標記，或其他主機標記不存在，都不能啟用此設定檔。沒有有效授權時，普通的 `gal pipeline <prompt>` 呼叫會維持 `legacy_interactive`。若 hook 根本沒有執行，主機中立的普通呼叫無法辨識 Codex hook 是否不存在、已停用或不受信任。

hook 接受明確的執行提示路徑、可解析為成對執行提示的 `#file:` 或 `@` 來源計畫參照，以及 `.dev/state.md` 中只有一個作用中計畫時省略路徑的請求。若作用中計畫狀態缺漏或有歧義、路徑位於 `.dev/plans/` 之外、路徑語法不安全，或請求包含 `from` 或 `stop-at` 修飾詞，hook 會拒絕請求。受保護接續不會默默忽略這些修飾詞。請改用受保護協調器回傳的具型別任務與階段動作。

若要依賴受保護接續功能，請先完成下列檢查：

1. 註冊 Codex 外掛後執行 `gal refresh`。GAL 會自動偵測註冊狀態。
2. 確認已安裝的 GAL Codex 外掛資訊清單包含 GAL 的 `UserPromptSubmit`、`PreToolUse` 與 `Stop` hook。請勿將這些 hook 加入根目錄的 `hooks/hooks.json`、根目錄的 `plugin.json` 或 Claude Code 資訊清單。
3. 執行 `gal doctor`，檢查靜態套件內容與指令就緒狀態。在 Codex 中信任 GAL 外掛。靜態診斷無法證明執行環境信任狀態或 hook 是否執行。
4. 目標工作區受信任時，請提出語意明確的管道請求。允許 Codex 執行同一工作階段的 Stop canary。canary 必須確認 bootstrap，並回傳確切的帶授權接續指令。下一個 `UserPromptSubmit` 必須在同一工作階段繫結該接續，受保護的 CLI 進入點才能消費授權。接著，受保護的進入點會執行兩項檢查：新的進入閘門與語意任務檢查點。這兩項檢查會在實作開始前完成。
5. 若 hook 不存在、已停用、不受信任，或 canary 失敗，請勿依賴受保護接續功能。明確指定 `--require-codex-stop-v1` 的進入點，在授權遺失或無效時，必須於實作前以 `host-continuation-not-ready` 失敗。普通呼叫會維持 `legacy_interactive`。主機中立的 CLI 無法推斷自己正在 Codex 中執行。

當 bootstrap 尚在等待時，Codex 的 `PreToolUse` hook 會拒絕所有受支援的工具呼叫。Stop canary 建立單次授權後，hook 只允許同一工作階段中確切的帶授權第一個動作。受支援範圍外的專用或託管工具路徑不在此 bootstrap 保證內，也不能由 bootstrap 接續使用。受保護進入點消費授權並建立 coordinator 修訂版 0 後，bootstrap 的全工具限制會結束。作用中的 hook 對一般工具使用回傳 neutral/pass。此結果不會授權檢查點或階段轉換。GAL 的受保護動作仍會強制檢查 coordinator 修訂版、收據與任務品質。Stop hook 會阻擋過早的最終回覆，直到狀態成為最終狀態或具型別的人工作業必要狀態。

若受保護接續中止，請在診斷期間維持工作區與工作階段識別不變。執行 `gal doctor` 檢查靜態就緒狀態，然後檢查 `.dev/pipeline/<plan-scope>/` 下的 coordinator 與嘗試證據。若投影資產已過期，請修正信任、資訊清單或 handler 問題，並執行 `gal refresh`。啟動新的受信任同一工作階段交握，以取得新的授權。請勿複製、手動建立或重複使用授權。若識別或證據檢查回報衝突，請保留已記錄的狀態，先解決衝突，再重試。若要不使用受保護的 Codex 行為繼續，請使用普通的 `gal pipeline <prompt>` 路徑及既有的主機負責接續流程。

若派送 shell 環境找不到執行檔，就緒狀態會回報 **Missing**。若指令查詢明確受到權限或安全政策阻擋，就緒狀態會回報 **Denied**。若結果不完整或互相衝突，就緒狀態會回報 **Unknown**。單次查詢失敗並不代表 Codex 未安裝，請勿僅根據 Denied 或 Unknown 嘗試重新安裝。

### Mode A：以 RTK 與 GAL 規則搭配 Approve for me

當信任已安裝的 RTK 與 GAL 執行檔，且希望其餘操作維持在 `workspace-write` 範圍內時，可使用此模式。Auto-review 僅決定符合條件之核准要求的審查者，本身不會放寬沙盒範圍。Codex 指令規則可提供所需例外：符合 `allow` 規則的指令會在沙盒外直接執行，不再顯示核准提示。

1. 備份 `%USERPROFILE%\.codex\config.toml`。將下列設定合併至該檔案中。請保留其他既有設定。若已有 `[windows]` 表格，請直接合併，切勿建立重複表格。

   ```toml
   approval_policy = "on-request"
   approvals_reviewer = "auto_review"
   sandbox_mode = "workspace-write"

   [windows]
   sandbox = "elevated"
   ```

   `windows.sandbox = "elevated"` 會選用 Windows 建議的原生沙盒實作，並不等同於 Full access。
2. 備份 `%USERPROFILE%\.codex\rules\default.rules`。先移除已被相同字首涵蓋的較窄項目，再新增下列規則：

   ```python
   prefix_rule(
       pattern = ["rtk"],
       decision = "allow",
       justification = "Allow RTK-managed commands to run outside the sandbox",
   )

   prefix_rule(
       pattern = [["gal", "gal.exe", "C:\\absolute\\path\\to\\gal.exe"]],
       decision = "allow",
       justification = "Allow trusted GAL entry points to run outside the sandbox",
   )
   ```

   請先找出這台電腦實際使用的 GAL 執行檔路徑，再將範例絕對路徑替換為實際查詢結果。在 Windows 上可執行 `Get-Command gal.exe -All` 查詢。保留 `gal` 與 `gal.exe`，以涵蓋直接依指令名稱呼叫的情況。較廣泛的 `rtk` 規則同時適用於 `rtk proxy gal ...` 及其他由 RTK 管理的指令。
3. 重新啟動 Codex 以重新載入 `config.toml` 與規則檔案，再為目標任務選取 **Approve for me**。
4. 請直接呼叫 RTK 或 GAL。若使用 `pwsh -Command ...` 或 `cmd /c ...` 等包裝指令，會改變指令字首，可能導致規則比對失敗。
5. 執行管道前，先確認規則與執行環境：

   ```powershell
   codex execpolicy check --rules "$env:USERPROFILE\.codex\rules\default.rules" --pretty rtk --version
   codex execpolicy check --rules "$env:USERPROFILE\.codex\rules\default.rules" --pretty gal.exe --version
   rtk --version
   gal.exe --version
   rtk proxy codex login status
   rtk proxy gal doctor --executor-smoke --executor codex --timeout 90
   ```

   兩次 `execpolicy check` 都必須回傳 `allow`。Doctor 必須能啟動 Codex，在無 TLS、沙盒或核准錯誤的情況下正常完成執行，並取得供應商回傳的最終結果。收據驗證屬於另一項獨立檢查。若 Doctor 回報 `NO_RECEIPT` 或其他收據錯誤，請檢查實際產生的收據與嘗試記錄檔。

這些規則僅允許符合條件的外層 RTK 或 GAL 行程跨越父層沙盒邊界。GAL 派送的 Codex 子行程仍受到 `workspace-write` 嚴格約束。指令規則無法覆寫組織管理的限制。詳情請參閱 OpenAI 官方的[沙盒與核准說明](https://developers.openai.com/codex/sandboxing)及[指令規則說明](https://developers.openai.com/codex/rules)。

若有所遺漏或是有相關問題，歡迎[發 issue](https://gitlab.com/monkey1wizard/Golem-Agents-Legion/-/issues/new)。

### Mode B：自訂 `config.toml`

1. 先備份既有檔案。將下列設定鍵合併至 `%USERPROFILE%\.codex\config.toml`。若已設定 `CODEX_HOME`，則合併至作用中的 `CODEX_HOME\config.toml`。請保留其他無關設定。最頂層的設定鍵須置於表格之前，並合併至現有的 `[sandbox_workspace_write]` 表格中，切勿建立重複表格。

   ```toml
   approval_policy = "on-request"
   approvals_reviewer = "auto_review"
   sandbox_mode = "workspace-write"

   [sandbox_workspace_write]
   network_access = true
   writable_roots = []
   ```

   除非特定操作需要其他路徑，否則請讓 `writable_roots` 保持空白。GAL 原始程式碼管道會將建置快取、不可變執行環境世代與協調器資料保存在正式原始程式碼 worktree 自有的 `target/gal-pipeline/` 目錄。請勿將共用 GAL 安裝、Cargo 或外掛快取目錄加入可寫入根目錄。TOML 不會展開 `~`；任何另經核准的路徑都必須是精確的絕對路徑。可寫入根目錄不會授予受保護 Git 中繼資料的存取權，也不會改變 GAL 子行程的沙盒限制。
2. 若無法開放網路存取，請設定 `network_access = false`。若有特定網路操作需求，應採用經審查的提權流程。僅在明確證實需要寫入權限時，才將精確的絕對路徑加入 `writable_roots`。切勿加入磁碟機根目錄或整個使用者個人目錄。僅需讀取權限的路徑不得授予寫入權限。若必要操作仍遭遇失敗，請保留診斷記錄，並針對該項有界操作請求原生核准，切勿預設 Custom 模式會解除所有沙盒限制。
3. 在 Desktop 中選取 **Custom**。若當前建置版本需要，請重新開啟或重啟 Codex，並再次確認任務生效的權限脈絡。
4. 請一併將指令列覆寫、專案與設定檔配置，以及組織安全政策納入考量。受組織管制的限制不代表本機設定有誤。CLI 設定檔可經由 `CODEX_HOME` 下的 `gal.config.toml` 並搭配 `codex --profile gal` 使用。請注意 Desktop 不會自動選取此 CLI 設定檔。

切勿使用 `approval_policy = "never"` 或 `sandbox_mode = "danger-full-access"` 作為設定捷徑。設定 `never` 僅會隱藏核准提示，並不會授予安全權限，且被派送的 GAL 子行程仍受 `workspace-write` 沙盒約束。

### 原始碼 worktree 執行環境與發布界線

每個原始程式碼 worktree 都會將管道建置快取、不可變執行環境世代與協調器狀態保存在自己的正式 `target/gal-pipeline/` 目錄。這些私有根目錄可隔離不同原始程式碼 worktree，也使其與下游儲存庫使用的已安裝 GAL 分開。

發布前，請在隔離的 fixture 中驗證套件管理器 `artifact`：將候選產物安裝至 fixture，在其中確認版本與執行行為，並讓 fixture 與使用者的共用安裝分開。原始程式碼 worktree 不會升級或取代已安裝的 shim。共用 shim 只會透過套件管理器的釋出版本變更。原始程式碼更新使用私有不可變世代與安全重新繫結。下游儲存庫繼續使用已安裝的 GAL。

### Smoke 測試、管道檢查與設定還原

請使用拋棄式儲存庫以及僅建立單一檔案的極小計畫來測試設定。先執行選定的 Codex smoke 測試，接著執行涵蓋實作、測試、獨立稽核與中斷接續的完整管道。檢查該次嘗試的日誌、執行收據與最終關卡證據。各階段提供不同層次的證據：指令查詢成功僅確認執行檔存在，行程啟動僅確認執行已展開，供應商回應則驗證了 API 通訊。有效的執行收據與通過關卡檢驗則分別獨立評估。

若存取遭拒，請保留管道復原點。僅在當前模式支援時，才針對該受限操作請求 Codex 原生核准。若失敗原因不明，請保留診斷日誌並直接調查已觀察到的階段，切勿在缺乏具體資料佐證下臆測為上游模型或認證問題。僅在針對該項有界操作取得明確的原生核准後，方可進行沙盒外的對照驗證。切勿關閉 TLS 憑證驗證、將整個使用者個人目錄授予存取權，或在進行中的嘗試期間變更執行器路由。

欲還原設定變更時，僅從備份還原經本程序修改的設定值，重新選取先前的 Codex Desktop 模式，並再次確認任務生效的權限脈絡。切勿直接取代整份設定檔，亦不得移除使用者憑證。

### 開發者驗收清單

驗收結果應記錄於自動化管道輸出與 finalize 關卡之外。凡未經開發者手動執行或即時檢查者，均維持標記為 **NOT RUN**。

- [ ] 記錄 Codex Desktop 與 CLI 版本，以及所選權限模式。
- [ ] 在使用者自己的工作階段確認登入狀態，不外洩憑證資料或權杖檔案。
- [ ] 確認每個已路由的執行器，並在拋棄式目錄中執行 smoke 測試。
- [ ] 執行最小管道，檢查實作、測試、獨立稽核與接續執行的證據。
- [ ] 確認在該環境中遭遇 Denied 與 Unknown 狀態時的錯誤處理與復原行為。
- [ ] 記錄設定還原步驟，並確認可還原至先前的設定模式。

### GitHub Copilot

GitHub Copilot 將 GAL 視為 Agent Plugins 1.0.0 套件取用：

1. 在 Copilot 設定中將 `~/.gal/plugins` 註冊為目錄型市集（讀取 `~/.gal/plugins/.claude-plugin/marketplace.json`），並啟用 `gal`。Copilot 會直接就地載入外掛而不複製檔案，從 `com.github.copilot/agents/` 載入代理程式、從 `com.github.copilot/rules/` 載入規則，並從 `mcp.json` 載入 MCP 設定。
2. 執行 `copilot plugin list`，確認 `gal` 已成功啟用。
3. 執行 `gal refresh`。確認 Copilot 註冊後，外掛會接管代理程式。GAL 仍會投影指令技能，因為 Copilot 不會從外掛的 `commands/` 載入指令。

### Google Antigravity

Antigravity 經由本機目錄探索機制安裝 GAL：

1. 自標準根目錄安裝外掛：
   ```bash
   agy plugin install ~/.gal/plugins/gal
   ```
   此指令會在 `~/.gemini/antigravity-cli/plugins/gal` 建立指向標準根目錄的接合點。
2. 清理重複的 `claude-code` 匯入項目：若 Antigravity 先前曾從 Claude Code 匯入過外掛，`agy plugin list` 可能會顯示兩筆 `gal` 項目（分別標示 `local-install` 與 `claude-code`）。由於 `agy` 未提供自 `import_manifest.json` 移除特定項目的指令，請手動開啟 `~/.gemini/config/import_manifest.json`，在 `imports` 陣列中移除 `claude-code` 物件，僅保留 `local-install` 項目。接著再次執行 `agy plugin list`，確認只留下一筆 `gal` 項目。
3. 執行 `gal refresh`。確認 Antigravity 註冊後，外掛會接管指令技能。GAL 仍會保留標準外掛連結。

### opencode（投影後備方案）

opencode 不使用 Markdown 外掛容器，其擴充架構仰賴經由 npm 或本機安裝的 JavaScript 與 TypeScript 模組。因此 opencode 採用 GAL 的檔案投影後備機制。執行 `gal refresh` 會自動將原生 Markdown 指令投影至 `~/.config/opencode/commands/`、代理程式投影至 `~/.config/opencode/agents/`，並將核心技能投影至 `~/.agents/skills/`。

### 次要路線：對話引導式安裝

若使用者經由 Claude Code 或 Codex 外掛市集快照分支發現 GAL，可使用對話引導方式完成初始安裝：

1. 在 Claude Code 或 Codex 外掛市集中搜尋並安裝 **GAL 外掛**（搜尋關鍵字「gal」），或自市集快照分支取得。
2. 在對話中向編碼代理程式 (coding agent) 提出 **"help me install gal"**。外掛內建的 `install-gal` 技能會主動徵求使用者同意，接著叫用對應的套件管理工具（Homebrew、winget 或 `cargo install --git`）安裝二進位檔，完成驗證後在未初始化的儲存庫中執行 `gal init`（若儲存庫已初始化則改為執行 `gal render-adapters`）。

單獨安裝市集外掛無法構成完整的 GAL 運作環境。所有工作流程指令皆以本機 `gal` 二進位檔為基礎，因此安裝執行檔為不可省略的步驟。安裝完成後，建議轉換為上述的主要本機外掛註冊方式。

## 儲存庫初始化與轉接器維護

### 首次初始化（`gal init`）

在全新未初始化的儲存庫中，`gal init` 會自 `gal-core` 範本產生一個儲存庫本機轉接器根目錄（`AGENTS.md`），並建立 `.dev/` 目錄結構（包含 `.dev/project.md` 與 `.dev/state.md`）。若儲存庫處於已初始化或半初始化狀態，`gal init` 會拒絕執行且不寫入任何內容，並以結束碼 `1` 退出。

GAL 唯一提供的儲存庫根目錄指示檔案是 `AGENTS.md`。若儲存庫中有 `CLAUDE.md` 或類似檔案，coding agent 本身的載入設計可能導致它不讀取 GAL 的 `AGENTS.md`。

請查閱你使用的 coding agent 說明文件，確認支援的指示檔名、檔案探索方式與載入優先順序。若工具不會載入 `AGENTS.md`，請依該工具的說明設定載入方式。不要假設產生檔案後，每個 coding agent 都會自動讀取它。

| 狀態 | `.dev/project.md` | `.dev/state.md` | `gal init` 行為 | 結束碼 |
| --- | --- | --- | --- | --- |
| 未初始化 | 不存在 | 不存在 | 全新啟動，自範本寫入這兩個檔案並算繪轉接器 | 0 |
| 已初始化 | 存在 | 存在 | 報錯，指出 `.dev/project.md` 已存在，提示改用 `gal render-adapters` 或刪除後重試 | 1 |
| 半初始化 | 存在 | 不存在 | 報錯，指出缺少 `.dev/state.md`，提示自版本控制還原或刪除後重試 | 1 |
| 半初始化 | 不存在 | 存在 | 報錯，指出缺少 `.dev/project.md`，提示自版本控制還原或刪除後重試 | 1 |

已初始化與半初始化狀態下的錯誤訊息如下：

```text
gal init: this repository is already initialized (.dev/project.md exists).
  To regenerate AGENTS.md from .dev/project.md, run: gal render-adapters
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
| 已初始化 | 存在 | 重新算繪 `AGENTS.md` 與條件層，並清理退役根目錄，每條路徑輸出一列 | 0 |
| 未初始化 | 不存在 | 報錯：`gal render-adapters: .dev/project.md not found. Run gal init first.` | 1 |

執行 `gal render-adapters` 時完全不會讀取或檢查 `.dev/state.md`。

### 轉接器維護與退役清理

維護既有儲存庫或從早期產生五個轉接器根目錄的版本升級時，執行 `gal render-adapters` 會自動協調本機檔案並執行退役清理移轉：

- **退役根目錄清理**：舊版的三個橋接根目錄（`CLAUDE.md`、`GEMINI.md`、`.github/copilot-instructions.md` 與 `.agents/rules/gal.md`）已正式退役。在產生或重新整理轉接器時，GAL 會檢查這些路徑。若檔案第一行帶有完全相符的 GAL 產生標記，GAL 會將其刪除並標記為 `pruned (GAL-owned)`。若刪除後使 `.agents/rules/` 或 `.github/` 成為空目錄，該目錄也會一併移除。
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
- **轉接器容量建議**：在已初始化的儲存庫內執行時，會額外顯示轉接器根目錄（`AGENTS.md`）的容量報告。轉接器檔案若超出建議大小只會標記 `[WARNING]` 警告，不會中斷正常操作。

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
