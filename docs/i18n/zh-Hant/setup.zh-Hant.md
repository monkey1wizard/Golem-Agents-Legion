---
source: docs/setup.md
lang: zh-Hant
source_commit: 11281aff62c1a3761fd3da35c420e6ec92374054
translated_at: 2026-10-21
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

在一台機器上設定 GAL 需要四個步驟：

1. 使用下列其中一種方式安裝 `gal` 二進位檔。
2. 執行一次 `gal refresh`。此指令會在 `~/.gal/plugins/gal/` 算繪外掛的標準根目錄 (canonical root)，並將 GAL 檔案投影 (projection) 到您的編碼代理程式 (coding agent)。每個執行環境 (runtime) 都從這個根目錄或其投影讀取 GAL，因此必須先執行此步驟，執行環境才能使用 GAL。
3. 依照[依執行環境註冊外掛](#依執行環境註冊外掛)的說明，向您使用的每個執行環境註冊外掛。接著再次執行 `gal refresh`，讓 GAL 偵測到這些註冊。
4. 在每個要使用 GAL 的儲存庫中執行 `gal init`。請參閱[初始化儲存庫](#初始化儲存庫-gal-init)。

在 Windows 上，PowerShell 會將單獨的名稱 `gal` 解析為內建別名。請在該環境改用 `gal.exe`，詳見 [PowerShell 別名說明](#powershell-別名衝突)。

### 安裝方式

#### 從原始程式碼建置 (`cargo install --git`)

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli --tag v0.3.2 --locked
```

請將 `v0.3.2` 替換為您想從 GitHub Releases 安裝的釋出標籤。釋出標籤保留開頭的 `v`。若未指定 `--tag`，Cargo 會建置公開儲存庫目前的 `main` 分支。`--locked` 旗標會使用儲存庫所追蹤之 `Cargo.lock` 中的相依套件版本進行建置。

GAL 未上架至 crates.io，因此 `--git` 旗標為必要引數。crates.io 上有一個同名的 crate `gal-cli`，但它是無關的工具。若執行 `cargo install gal-cli` 而未加上 `--git`，安裝的會是該工具，而不是 GAL。在 GAL 的 Cargo 工作區中，`gal-cli` 是套件名稱，安裝後的執行檔名稱則為 `gal`。

#### Homebrew（macOS 與 Linux）

```bash
brew install monkey1wizard/tap/gal
```

這是推薦給 macOS 與 Linux 使用者的套件管理員安裝途徑。

#### WinGet（Windows）

```powershell
winget install Monkey1Wizard.GAL
```

社群套件索引的更新可能落後於版本釋出。安裝前，請先執行 `winget show Monkey1Wizard.GAL`，確認該版本是否已可取得。

#### 直接下載二進位檔

每個 GitHub 釋出版本都會提供下列資產：

- 每個目標的封存檔，名稱為 `gal-<tag>-<platform>-<arch>.tar.gz`。Windows 封存檔則改用 `.zip`。每個封存檔都包含 `gal` 二進位檔與 GAL 來源資產。
- 每個 Windows 架構各有一個獨立的二進位檔，名稱為 `gal-<tag>-windows-<arch>.exe`。
- `checksums.txt`，記錄每個封存檔的 SHA-256 雜湊值。
- `checksums.txt.sig` 與 `checksums.txt.pem`，是 `checksums.txt` 的 cosign 簽章與憑證。
- `artifact-manifest.json`，是釋出產出物的清單。
- `install.sh` 與 `install.ps1`，是安裝指令碼。

每個釋出版本會建置六個目標：Windows、macOS 與 Linux，各有 `x64` 與 `arm64`。`<tag>` 包含開頭的 `v`。例如，v0.3.2 的 x64 Windows 封存檔為 `gal-v0.3.2-windows-x64.zip`。平台名稱為 `windows`、`darwin` 與 `linux`。架構名稱為 `x64` 與 `arm64`。

請解壓縮封存檔，並將 `gal` 二進位檔放到您的 `PATH` 中。在 Windows 上，二進位檔為 `gal.exe`。在 macOS 與 Linux 上，若解壓縮後的二進位檔不具執行權限，請執行 `chmod +x gal`。若來源資產不在二進位檔旁邊，`gal refresh` 會使用內嵌在二進位檔中的來源資產副本。請參閱[來源根目錄的查找順序](#來源根目錄的查找順序)。

#### Shell 指令碼（Linux 與 macOS）

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
```

指令碼會依據 `checksums.txt` 中的 SHA-256 雜湊驗證下載的封存檔。雜湊不符或缺少對應項目時，安裝會中止。若 `cosign` 位於您的 `PATH` 中，指令碼還會在解壓縮任何檔案之前，以無金鑰 (keyless) cosign 驗證 `checksums.txt` 的簽章。任何 cosign 驗證失敗都會中止安裝。若未安裝 `cosign`，或無法下載簽章檔案，指令碼會印出警告，並僅以 SHA-256 驗證繼續安裝。

指令碼會將二進位檔安裝至 `~/.local/bin`，並將來源資產安裝至 `~/.local/share/gal`。在複製新的來源資產之前，指令碼會刪除整個 `~/.local/share/gal` 目錄，因此請勿在該目錄保存其他檔案。`~/.local/bin/gal` 會被直接覆寫。若要安裝特定釋出版本，請將 `GAL_VERSION` 設為該版本的標籤，並包含開頭的 `v`。指令碼會將此值當作下載網址中的釋出標籤，因此不含 `v` 的 `0.3.2` 會下載失敗：

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | GAL_VERSION=v0.3.2 bash
```

#### PowerShell 指令碼（Windows）

```powershell
irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
```

Windows 指令碼目前為 Beta 版。它尚未在乾淨的機器上驗證過，執行時會印出 Beta 警告。它會執行與 shell 指令碼相同的 SHA-256 與 cosign 檢查，並在相同的驗證失敗情況下中止。

指令碼會將 `gal.exe` 與來源資產安裝至 `%LOCALAPPDATA%\Programs\gal`。在複製新檔案之前，它會刪除該目錄中既有的所有內容。請勿在該目錄存放其他檔案。指令碼不會變更您的 `PATH`。請自行將 `%LOCALAPPDATA%\Programs\gal` 新增至使用者 `PATH`。若要安裝特定釋出版本，請將 `$env:GAL_VERSION` 設為該版本的標籤，並包含開頭的 `v`，例如 `$env:GAL_VERSION = 'v0.3.2'`。

#### PowerShell 別名衝突

PowerShell 有一個名為 `gal` 的內建別名，對應至 `Get-Alias`。在每個 PowerShell 工作階段中（包含指令碼），單獨的名稱 `gal` 都會執行 `Get-Alias`，而不是 GAL 二進位檔。請明確呼叫 `gal.exe`、使用二進位檔的完整路徑，或在 PowerShell 的 `$PROFILE` 中移除此別名：

```powershell
Remove-Item Alias:gal -Force -ErrorAction SilentlyContinue
```

### 來源根目錄的查找順序

`gal refresh` 需要包含 `skills/`、`agents/` 與 `commands/` 的 GAL 來源根目錄。它依下列順序查找：

1. `gal` 二進位檔所在的目錄。先檢查扁平配置（來源與二進位檔並排），接著檢查同一前綴下 `bin/gal` 旁的 `share/gal`，然後檢查二進位檔目錄的各個上層目錄。
2. 內嵌在二進位檔中的來源資產。GAL 會將此副本寫入 `~/.gal/embedded-src` 並加以使用。

`gal dev-apply` 會部署 GAL 來源檢出目錄。可用下列選項：

- `--source <path>` 指定來源根目錄。省略時，使用從目前目錄找到的檢出目錄。若找不到檢出目錄，指令會以錯誤結束。
- `--root <path>` 將寫入範圍限制在既有的根目錄之下。此選項必須與 `--source` 一起使用。

請只對 GAL 來源檢出目錄使用 `gal dev-apply`。機器設定請使用 `gal refresh`。

### 升級與維護

GAL 不會自我更新。`gal update` 會印出已安裝的版本，以及 Cargo、WinGet 與 Homebrew 三種升級指令。請使用與安裝時相同的方式升級：

| 安裝方式 | 升級 |
| --- | --- |
| Cargo | 以新標籤執行相同的 `cargo install --git ... gal-cli --tag <new tag> --locked` 指令。Cargo 會取代已安裝的二進位檔。若要重新安裝相同的修訂版本，請加上 `--force`。 |
| Homebrew | `brew upgrade gal` |
| WinGet | `winget upgrade Monkey1Wizard.GAL` |
| Shell 或 PowerShell 指令碼 | 再次執行安裝指令碼。 |
| 直接下載 | 下載新的釋出資產，並取代您 `PATH` 中的二進位檔。 |

每次升級後，請完成下列步驟：

1. 執行 `gal refresh`，算繪新的標準根目錄與投影。
2. 在每個已初始化的儲存庫中執行 `gal render-adapters`，將其轉接器 (adapter) 檔案更新至新版本。
3. 依照[更新延遲與快取](#更新延遲與快取)的說明，重新載入會快取 GAL 的執行環境。

不帶引數執行 `gal` 會顯示使用說明，並以結束碼 `0` 結束。無法辨識的子指令會以結束碼 `64` 結束。

## 依執行環境註冊外掛

對於支援 Markdown 外掛容器的執行環境，外掛註冊是將 GAL 連接到該執行環境的主要方式。Claude Code、GitHub Copilot 與 Google Antigravity 會直接從 `gal refresh` 所算繪的標準根目錄 `~/.gal/plugins/gal/` 讀取外掛。OpenAI Codex 會在您安裝外掛時，將外掛複製到它自己的快取中。opencode 不支援 Markdown 外掛，因此改用檔案投影。

註冊外掛後，請執行 `gal refresh`。GAL 會尋找每個執行環境的本機註冊證據。確認已註冊時，GAL 會停止投影由外掛取代的後備檔案。沒有註冊時，GAL 會投影後備檔案。證據不確定時，GAL 會投影後備檔案並印出警告。

GAL 只會投影到 `~/.gal/install-state.json` 中 `selectedRuntimes` 列出的執行環境。有效名稱為 `claude`、`codex`、`copilot`、`antigravity` 與 `opencode`。GAL 會忽略其他名稱。若檔案不存在，或清單中沒有有效名稱，GAL 會選取全部五個執行環境。對於未選取的執行環境，GAL 會移除它所管理的產出物。

若要新增個人 MCP server，請將其放在 `~/.gal/local/mcp.json` 的 `servers` 物件下。GAL 會將這些項目合併到算繪後的 `mcp.json`。若個人 server 與核心 server 同名，核心 server 優先，GAL 會捨棄個人項目。

### Claude Code

Claude Code 會將標準根目錄載入為以目錄為基礎的技能外掛：

1. 在 `~/.claude/skills/gal` 建立指向 `~/.gal/plugins/gal` 的目錄接合點 (junction) 或符號連結：
   - Windows（PowerShell）：

     ```powershell
     New-Item -ItemType Junction -Path "$env:USERPROFILE\.claude\skills\gal" -Target "$env:USERPROFILE\.gal\plugins\gal"
     ```

   - macOS / Linux：

     ```bash
     ln -s ~/.gal/plugins/gal ~/.claude/skills/gal
     ```

2. 啟動新的 Claude Code 工作階段，接著執行 `claude plugin list`，確認 `gal@skills-dir` 已載入。Claude Code 會在工作階段啟動時載入 skills-dir 外掛。它透過接合點讀取檔案，不會將檔案複製到快取。
3. 執行 `gal refresh`。GAL 確認 Claude 已註冊時，不會將後備指令投影到 `~/.claude/commands/`。否則，GAL 會投影後備指令。

`gal refresh` 不會建立 `~/.claude/skills/gal` 連結。連結遺失時，`gal doctor` 會發出警告。

作為次要選項，您也可以從 `gal refresh` 寫入 `~/.gal/plugins/.claude-plugin/marketplace.json` 的本機市集安裝 GAL：

```bash
claude plugin marketplace add ~/.gal/plugins
claude plugin install gal@gal
```

此途徑會將外掛複製到 `~/.claude/plugins/cache/<marketplace>/gal/<version>/`。執行 `gal refresh` 時，此副本不會改變，而且 Claude Code 需要重新啟動才能套用外掛更新。

### OpenAI Codex

Codex 透過本機外掛市集目錄載入 GAL：

1. 註冊本機市集目錄：

   ```bash
   codex plugin marketplace add ~/.gal/plugins
   ```

   Codex 會載入 `gal refresh` 所產生的 `~/.gal/plugins/.agents/plugins/marketplace.json`。
2. 新增外掛：

   ```bash
   codex plugin add gal@gal
   ```

   Codex 會將外掛檔案複製到 `~/.codex/plugins/cache/gal/gal/<version>/`。`gal refresh` 不會更新此副本。升級 GAL 後，請使用 `codex plugin remove gal@gal` 移除外掛，再重新新增並重新信任它。
3. 執行 `codex plugin list`，確認 `gal@gal` 已出現。
4. 執行 `gal refresh`。GAL 一律會將 Codex TOML 代理程式 (agent) 投影到 `~/.codex/agents/`，並將指令技能投影到 `~/.agents/skills/`。確認 Codex 已註冊且未選取 opencode 時，核心技能由外掛提供，GAL 會停止投影這些技能。

Codex 管道 (pipeline) hook 包含在 Codex 外掛資訊清單的 `.codex-plugin/plugin.json#hooks` 中。GAL 不會安裝根目錄的預設 hook，也不會將 hook 加入 Claude Code 資訊清單。重新整理後，請確認 Codex 外掛資訊清單，並在 Codex 中信任 GAL 外掛，然後再依賴受保護接續功能。信任狀態與 hook 執行結果屬於執行環境事實。`gal doctor` 可回報靜態套件內容與指令就緒狀態，但只有即時的同一工作階段交握，才能證明 Codex 確實執行了受信任的 hook。

**共用技能目錄注意事項**：Codex 與 opencode 共用 `~/.agents/skills`。只要選取了 opencode，即使 Codex 註冊已確認，GAL 仍會持續投影這個共用介面。Codex 註冊不會移除 opencode 所需的檔案。

### GitHub Copilot

GitHub Copilot 將 GAL 載入為 Agent Plugins 1.0.0 套件：

1. 在 Copilot 設定中將 `~/.gal/plugins` 註冊為目錄型市集，接著從該市集啟用 `gal` 外掛。Copilot 會就地讀取外掛檔案，不會複製。它從 `com.github.copilot/agents/` 載入代理程式角色，並從 `com.github.copilot/rules/` 載入工作區規則。GAL 也隨附一個 `mcp.json` 檔案，但其中的伺服器清單是空的。GAL 不提供 MCP server。
2. 執行 `copilot plugin list`，確認 `gal` 外掛已啟用。
3. 執行 `gal refresh`。確認 Copilot 已註冊時，代理程式由外掛提供，GAL 會停止投影它們。GAL 仍會將指令技能投影到 `~/.copilot/skills/`，因為 Copilot 不會從外掛的 `commands/` 目錄讀取指令。此外，GAL 會將 `~/.copilot/gal` 維持為指向 `~/.gal` 的目錄連結。寫入 `~/.copilot/gal` 的檔案，實際上會寫入 `~/.gal`。

### Google Antigravity

Antigravity 透過本機目錄探索機制註冊 GAL：

1. 直接從標準根目錄安裝外掛：

   ```bash
   agy plugin install ~/.gal/plugins/gal
   ```

2. 執行 `agy plugin list`，確認 `gal` 只出現一次。若 Antigravity 先前曾從 Claude Code 匯入外掛，清單可能會顯示來自 `local-install` 與 `claude-code` 的重複 `gal` 項目。此時請開啟 `~/.gemini/config/import_manifest.json`，從 `imports` 陣列中移除 `claude-code` 項目，只保留 `local-install`，然後再次執行 `agy plugin list`。
3. 執行 `gal refresh`。當 `~/.gemini/antigravity-cli/plugins/gal` 解析為目錄，且該目錄中 `.claude-plugin/plugin.json` 的 `name` 為 `gal` 時，GAL 會將 Antigravity 視為已註冊。若該資訊清單無法讀取或名稱不符，GAL 會將登錄視為不確定，投影指令技能並印出警告。確認已註冊時，指令技能由外掛提供，GAL 會停止將它們投影到 `~/.gemini/antigravity-cli/skills/`。無論 Antigravity 是否已註冊，GAL 都會讓 `~/.gemini/antigravity-cli/gal` 連結持續指向 `~/.gal/source`。

若 `~/.gemini/antigravity-cli/plugins/gal` 存在，但 `agy plugin list` 未顯示 `gal`，GAL 仍會將 Antigravity 視為已註冊並略過指令技能。此時 Antigravity 將沒有任何 GAL 指令。請使用 `agy plugin install` 安裝外掛，直到 `agy plugin list` 顯示 `gal` 為止。

### opencode（投影後備方案）

opencode 不支援以 Markdown 為基礎的外掛資訊清單。其擴充模型仰賴透過 npm 或本機目錄安裝的 JavaScript 與 TypeScript 模組。因此，opencode 採用 GAL 的檔案投影後備機制。執行 `gal refresh` 會將 Markdown 指令投影到 `~/.config/opencode/commands/`、將代理程式投影到 `~/.config/opencode/agents/`，並將核心技能投影到 `~/.agents/skills/`。

### 已淘汰的執行環境

GAL 不會將指令或技能投影到 Gemini CLI。原 Gemini CLI 使用者應改用 Google Antigravity。

### 對話式安裝

若 GAL 外掛已在 Claude Code 或 Codex 中載入，但缺少 `gal` 二進位檔，您可以請助理安裝二進位檔。請在對話中輸入 **"help me install gal"**。內建的 `install-gal` 技能接著會依下列方式運作：

1. 檢查 `gal --version` 是否已可正常運作。
2. 依您的作業系統選擇套件管理員：Homebrew、WinGet 或 `cargo install --git`。
3. 顯示確切的安裝指令，並在執行該指令之前等待您確認。
4. 以 `gal --version` 驗證結果。若為 Cargo 安裝，它還會執行 `gal refresh`。
5. 告訴您剩下的步驟：先執行 `gal refresh`，再於新儲存庫中執行 `gal init`，或在已初始化的儲存庫中執行 `gal render-adapters`。

只有外掛並不能提供可運作的環境。工作流程指令仰賴原生的 `gal` 二進位檔。安裝二進位檔後，請依照上述說明註冊每個執行環境。

## Codex 管道執行設定

本節設定程序以 Windows 上的 Codex CLI 0.161.0 與 Codex Desktop 26.1002.7124.0 為準，這是 2026-10-08 觀察到的版本。若使用其他版本，請在 Codex 官方文件中確認目前的行為。官方設定名稱與權限邊界請參閱 [permission modes](https://learn.chatgpt.com/docs/permission-modes)、[agent approvals and security](https://learn.chatgpt.com/docs/agent-approvals-security) 與 [configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference)。

### 已知問題 Codex Windows 沙盒失效

在 Windows 上，Codex 沙盒目前會在執行任何指令之前就失敗。GAL 管道無法在沙盒內執行。這是 Codex 的缺陷，不是 GAL 或儲存庫的問題，GAL 也無法修復它。

**症狀。** 所有在沙盒內執行的指令都會失敗，並顯示 `helper_unknown_error: setup refresh had errors`。代理程式接著會回報無法讀取檔案或無法啟動 shell。即使執行器 (executor) 本身已經啟動，管道階段仍會以 Verdict `BLOCKED`、`no-receipt` 或零項測試結束。

**確認方式。** 開啟 `%USERPROFILE%\.codex\.sandbox\sandbox.<date>.log`，搜尋 `setup refresh completed with errors`。每次執行回報的原因可能不同。已觀察到的原因包括 Codex 執行環境檔案正被使用（os error 32）、deny ACE 失敗，以及 write ACE 失敗。重新啟動 Codex、重新啟動沙盒服務與重新開機，都沒有排除這個失敗。若要直接測試某台機器，請在拋棄式目錄執行下列指令：

```powershell
codex exec --json -s workspace-write "Run the command Get-Location in PowerShell and report its output"
```

若 `command_execution` 事件回報 `exit_code` 為 `-1`，且訊息為 `setup refresh had errors`，表示該機器受到影響。

**GAL 的處理方式。** GAL 派送 (dispatch) 每個 Codex 子行程時，都會使用 `--dangerously-bypass-approvals-and-sandbox` 啟動。因此子行程不在 Codex 沙盒內執行，信任等級與其他執行器透過權限繞過旗標取得的完整存取權相同。較早的 GAL 版本以 `-s workspace-write` 啟動子行程。在此缺陷下，該設定會使每次 Codex 派送都失敗。

**您必須採取的行動。** 請讓驅動 GAL 管道的外層 Codex 工作階段在沒有沙盒的情況下執行。請參閱[必要權限模式 Full access](#必要權限模式-full-access)。**Approve for me**（auto-review）並不是替代解法。它只決定由誰審查核准要求，其指令仍在失效的沙盒內執行。

**安全影響。** 沒有沙盒時，Codex 可以讀寫整個檔案系統，也可以執行您的帳號能執行的任何終端機指令。請只在受信任的開發機器與受信任的儲存庫中執行 GAL 管道，這也是[權限繞過警告](./workflows.zh-Hant.md#安全警告權限繞過)對所有執行器的既有要求。

**Codex 修復之後。** 請再次執行上方的測試指令。屆時即可再次恢復在沙盒內執行外層工作階段。在 GAL 釋出版本另行說明之前，GAL 仍會以無沙盒方式啟動 Codex 子行程。

### 共用先決條件

1. 安裝 GAL 並確認 Codex CLI 已完整安裝。在實際執行派送的 shell 工作階段中執行：

   ```powershell
   gal.exe --version
   codex --version
   codex doctor
   Get-Command gal.exe -All
   Get-Command codex -All
   ```

   在其他 shell 中，請以該 shell 的指令查詢工具確認實際解析的執行檔。在 Windows 上，請勿為了解決查詢失敗而修改系統 `PATH`。若 PowerShell 將 `gal` 解析為內建別名，請直接呼叫 `gal.exe` 或指定其完整路徑。

2. 請另行使用 `codex login` 完成 Codex 驗證。請勿直接揭露或檢視權杖檔案。請確認預定任務所設定的所有執行器，而不只是 Codex。
3. 開啟您受信任的目標工作區。將網路存取限制在必要的目的地。只有在診斷指出所需的特定路徑時，才新增可寫入目錄。
4. 執行 `gal doctor`，接著在拋棄式測試目錄中執行 `gal doctor --executor-smoke --executor codex`。請注意 smoke 測試會消耗供應商配額。此測試可驗證 CLI 就緒狀態與收據處理，但無法證明完整管道、獨立稽核或 Desktop 權限模式能夠成功。
5. 請獨立於 Codex 核准設定之外，設定 `executorRouting` 與 `timeoutSecs`。請參閱 [configuration.zh-Hant.md](./configuration.zh-Hant.md#執行器路由-executorrouting)。
6. 只對新的儲存庫執行初始化。若有多個作用中的計畫，請明確指定目標執行提示檔 (execution prompt)。

### Codex 受保護接續就緒條件

受保護的 `codex_stop_v1` 設定檔需要受信任的 GAL Codex 外掛，以及即時的同一工作階段 hook 交握。模型名稱、環境推測、過期標記，或其他主機的標記不存在，都不能啟用此設定檔。沒有有效授權時，普通的 `gal pipeline <prompt>` 呼叫會維持 `legacy_interactive`。若 hook 根本沒有執行，主機中立的普通呼叫無法偵測 Codex hook 是否不存在、已停用或不受信任。

hook 接受明確的執行提示檔路徑、可解析為成對執行提示檔的 `#file:` 或 `@` 來源計畫 (source plan) 參照，以及在 `.dev/state.md` 中只有一個作用中計畫時省略路徑的請求。若作用中計畫狀態缺漏或有歧義、路徑位於 `.dev/plans/` 之外、路徑語法不安全，或請求包含 `from` 或 `stop-at` 修飾詞，hook 會拒絕請求。受保護接續不會默默捨棄這些修飾詞。請改用受保護協調器回傳的具型別任務與階段動作。

在依賴受保護接續功能之前，請先完成下列檢查：

1. 註冊 Codex 外掛後執行 `gal refresh`。GAL 會自動偵測註冊狀態。
2. 確認已安裝的 GAL Codex 外掛資訊清單包含 GAL 的 `UserPromptSubmit`、`PreToolUse` 與 `Stop` hook。請勿將這些 hook 加入根目錄的 `hooks/hooks.json`、根目錄的 `plugin.json` 或 Claude Code 資訊清單。
3. 執行 `gal doctor`，檢查靜態套件內容與指令就緒狀態。在 Codex 中信任 GAL 外掛。靜態診斷無法證明執行環境信任狀態或 hook 是否執行。
4. 請在受信任的目標工作區中提出語意明確的管道請求。允許 Codex 執行同一工作階段的 Stop canary。canary 必須確認 bootstrap，並回傳確切的帶授權接續指令。下一個 `UserPromptSubmit` 必須在同一工作階段中繫結該接續，受保護的 CLI 進入點才能消費授權。接著，受保護的進入點會在實作開始之前，執行新的進入閘門與語意任務檢查點。
5. 若 hook 不存在、已停用、不受信任，或 canary 失敗，請勿依賴受保護接續功能。明確指定 `--require-codex-stop-v1` 的進入點，在授權遺失或無效時，必須於實作前以 `host-continuation-not-ready` 失敗。普通呼叫會維持 `legacy_interactive`。主機中立的 CLI 無法推斷自己正在 Codex 中執行。

bootstrap 有固定的時間與嘗試次數上限。等待中的 bootstrap 會在 120 秒後失效，授權則會在核發後 30 秒失效。對於同一個 bootstrap，Stop hook 最多處理 3 個 Stop 事件。

當 bootstrap 尚在等待時，Codex 的 `PreToolUse` hook 會拒絕所有受支援的工具呼叫。Stop canary 建立單次授權後，hook 只允許同一工作階段中確切的帶授權第一個動作。不受支援的專用或託管工具路徑不在此 bootstrap 保證範圍內，也不能由 bootstrap 接續使用。受保護進入點消費授權並將協調器繫結至該工作階段後，這項 bootstrap 限制就會結束。hook 接著會允許一般的工具使用。此結果不會授權檢查點或階段轉換。GAL 的受保護動作仍會強制檢查協調器修訂版、收據與任務品質。

`Stop` hook 只在 bootstrap 期間作用。它會發出 canary，並阻擋最終回覆，直到帶授權的接續執行為止。沒有等待中的 bootstrap 時，`Stop` hook 會讓回覆正常結束。它不會讀取管道狀態，也不會讓執行中的管道一直保持開啟，直到管道進入最終狀態為止。

若受保護接續中止，請在診斷期間維持工作區與工作階段識別不變。執行 `gal doctor` 檢查靜態就緒狀態，然後檢查 `.dev/pipeline/<plan-scope>/` 下的協調器與嘗試證據。請解決信任、資訊清單或 handler 問題，若投影資產已過期，請執行 `gal refresh`。啟動新的受信任同一工作階段交握，以取得新的授權。請勿複製、手動建立或重複使用授權。若識別或證據檢查回報衝突，請保留已記錄的狀態，先解決衝突，再重試。若要在不使用受保護 Codex 行為的情況下繼續，請使用普通的 `gal pipeline <prompt>` 路徑及既有的主機負責接續流程。

若派送 shell 環境找不到執行檔，就緒狀態會回報 **Missing**。若指令查詢明確受到權限或安全政策阻擋，就緒狀態會回報 **Denied**。若結果不完整或互相衝突，就緒狀態會回報 **Unknown**。僅憑查詢失敗，並不能斷定 Codex 未安裝。請勿只因查詢結果為 Denied 或 Unknown 就重新安裝 Codex。

### 必要權限模式 Full access

在 [Windows 沙盒缺陷](#已知問題-codex-windows-沙盒失效)存在期間，每個執行 GAL 管道的外層 Codex 工作階段都必須使用此模式。本指南較早的版本建議搭配 RTK 與 GAL 指令規則使用 **Approve for me**，或使用自訂的 `workspace-write` 設定。這兩種做法都在失效的沙盒內執行指令，因此目前都無法執行管道。

1. 在 Codex Desktop 中，為該任務選取 **Full access** 權限模式。
2. 使用 Codex CLI 時，請以 `--dangerously-bypass-approvals-and-sandbox` 啟動工作階段，或在 `%USERPROFILE%\.codex\config.toml` 設定沙盒模式。請先備份該檔案。保留無關的設定，並將此鍵放在任何資料表之前：

   ```toml
   sandbox_mode = "danger-full-access"
   ```

   `approvals_reviewer = "auto_review"` 等核准設定可以保留，但它不會讓沙盒恢復作用。
3. 重新啟動 Codex，使其重新載入 `config.toml`。
4. 執行管道之前，請確認執行環境：

   ```powershell
   gal.exe --version
   codex login status
   gal.exe doctor --executor-smoke --executor codex --timeout 90
   ```

   doctor 必須能啟動 Codex，並在沒有 TLS、沙盒或核准錯誤的情況下取得供應商的最終結果。收據驗證是另一項獨立檢查。若 doctor 回報 `NO_RECEIPT` 或其他收據錯誤，請檢查產生的收據與嘗試日誌。

GAL 絕不會變更您 Codex `config.toml` 中的核准或沙盒設定。外層工作階段的模式仍由您決定。不過，GAL 會在它投影到 `~/.codex/agents/<name>.toml` 的每個 Codex 代理程式檔案中寫入 `sandbox_mode` 值。無論外層使用哪種模式，派送的 Codex 子行程都在沒有沙盒的情況下執行。組織管理的限制仍適用於外層工作階段。詳情請參閱 OpenAI 官方的[沙盒與核准行為](https://developers.openai.com/codex/sandboxing)說明。

若此設定未涵蓋特定環境的情況，或您遇到相關問題，請[回報問題](https://github.com/monkey1wizard/Golem-Agents-Legion/issues/new)。

### Smoke 測試、管道驗證與還原

請使用拋棄式儲存庫，以及只建立單一檔案的最小計畫來測試設定。先執行選定的 Codex smoke 測試，接著執行涵蓋實作、測試、獨立稽核，以及階段中斷後接續的管道。檢查嘗試日誌、執行收據與最終關卡證據。各階段提供不同的證據：指令查詢成功可確認執行檔可用，行程啟動可確認執行已開始，供應商回應則可驗證 API 通訊。有效的執行收據與通過的關卡檢查則分別獨立評估。

若存取遭拒，請保留管道復原點。只有在目前的權限模式支援時，才針對該特定的受限操作請求 Codex 原生核准。若失敗原因不明，請保留診斷日誌並直接調查已觀察到的階段，不要在缺乏佐證資料的情況下臆測為上游模型或驗證缺陷。切勿關閉 TLS 憑證驗證、對整個使用者家目錄授予權限，或在進行中的嘗試期間變更執行路由。

若要還原設定變更，請只從備份還原已修改的設定，重新選取先前的 Codex Desktop 模式，並再次確認實際生效的任務脈絡。請勿取代整份設定檔，也不要移除使用者憑證。

### 開發者驗收清單

請獨立於自動化管道輸出與落地關卡之外記錄結果。手動檢查與即時環境檢查在開發者實際執行之前，請維持標記為 **NOT RUN**。

- [ ] 記錄 Codex Desktop 與 CLI 版本，以及所選的權限模式。
- [ ] 確認使用者工作階段的登入狀態，且不揭露憑證資料或權杖檔案。
- [ ] 確認每個已路由的執行器，並在拋棄式目錄中執行執行器 smoke 測試。
- [ ] 執行最小管道，並檢查實作、測試、獨立稽核與接續的證據。
- [ ] 確認您的環境在 Denied 與 Unknown 狀態下的錯誤處理與復原行為。
- [ ] 記錄還原設定，並確認可以還原先前的設定模式。

## 儲存庫初始化與轉接器維護

### 初始化儲存庫 (`gal init`)

在新的儲存庫中，`gal init` 會從 `gal-core` 範本建立轉接器根目錄 (`AGENTS.md`)，並初始化 `.dev/`，其中包含 `.dev/project.md` 與 `.dev/state.md`。若儲存庫已初始化或部分初始化，`gal init` 會中止且不寫入任何檔案，並以結束碼 `1` 結束。

GAL 只提供 `AGENTS.md` 作為儲存庫的指示根檔案。視您的編碼代理程式載入指示的設計而定，既有的 `CLAUDE.md` 或類似檔案可能會使代理程式略過 GAL 的 `AGENTS.md`。

請查閱您的編碼代理程式說明文件，確認其支援的指示檔名、檔案探索方式與載入優先順序。若代理程式不會載入 `AGENTS.md`，請依其說明文件所述的方式設定。請勿假設產生檔案後，每個編碼代理程式都會自動載入它。

| 儲存庫狀態 | `.dev/project.md` | `.dev/state.md` | `gal init` 動作 | 結束碼 |
| --- | --- | --- | --- | --- |
| 未初始化 | 不存在 | 不存在 | 從範本建立兩個狀態檔案，並算繪轉接器根目錄。 | 0 |
| 已初始化 | 存在 | 存在 | 因 `.dev/project.md` 已存在而中止。指示改用 `gal render-adapters`。 | 1 |
| 部分初始化 | 存在 | 不存在 | 因缺少 `.dev/state.md` 而中止。指示從 git 還原。 | 1 |
| 部分初始化 | 不存在 | 存在 | 因缺少 `.dev/project.md` 而中止。指示從 git 還原。 | 1 |

錯誤訊息會提供可據以處理的修正步驟：

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

### 重新產生轉接器 (`gal render-adapters`)

在已初始化的儲存庫中，`gal render-adapters` 會以 `.dev/project.md` 作為唯一的權威來源來更新轉接器檔案。請從儲存庫根目錄執行此指令。

| 儲存庫狀態 | 條件 | `gal render-adapters` 動作 | 結束碼 |
| --- | --- | --- | --- |
| 已初始化 | `.dev/project.md` 存在 | 重新產生 `AGENTS.md` 與條件層，並清除已退役的根目錄。 | 0 |
| 已初始化 | `AGENTS.md` 或某個條件層檔案存在，但沒有 GAL 產生標記 | 中止且不寫入任何轉接器檔案，以保護使用者擁有的檔案。 | 1 |
| 未初始化 | `.dev/project.md` 不存在 | 中止：`gal render-adapters: .dev/project.md not found. Run gal init first.` | 1 |
| 任何狀態 | 傳入了引數 | 中止：`gal render-adapters: unknown option`。 | 64 |

`gal render-adapters` 指令不會檢查 `.dev/state.md`。

### 轉接器清理與退役檔案

更新既有儲存庫，或從會產生五個轉接器根目錄的版本移轉時，`gal render-adapters` 會清理已淘汰的產出物：

- **移除退役根目錄**：舊的橋接檔案（`CLAUDE.md`、`GEMINI.md`、`.github/copilot-instructions.md` 與 `.agents/rules/gal.md`）已退役。重新整理轉接器時，GAL 會檢查這些路徑。若檔案第一行包含 GAL 產生標記，GAL 會刪除該檔案，並回報為 `pruned (GAL-owned)`。若移除檔案後使 `.agents/rules/` 或 `.github/` 變成空目錄，該目錄也會一併移除。
- **保留使用者檔案**：缺少 GAL 產生標記的檔案會被視為使用者檔案。GAL 不會變動這些檔案，並將其回報為 `kept (hand-owned)`。
- **條件層**：在 `.dev/project.md` 中啟用 Rust 慣例時，條件規則檔案（`.claude/rules/gal-rust.md` 與 `.github/instructions/gal-rust.instructions.md`）會與主要轉接器一起更新。若停用 Rust 慣例，過時的 GAL 管理規則檔案會被移除。

GAL 會將 `pruned (GAL-owned)` 與 `kept (hand-owned)` 回報以警告行的形式印在標準錯誤輸出上。

### 專案規格 (`.dev/project.md`)

`.dev/project.md` 檔案定義轉接器算繪所使用的核心專案設定檔。此檔案**必須包含下列八個必要的 H2 區段，且每個區段只能出現一次**：`What This Is`、`Tech Stack`、`Architecture`、`Constraints`、`Response Style`、`Freshness`、`Project Language` 與 `Protected Paths`。此檔案可以包含其他 H2 區段。`gal init` 所提供的範本包含 `Key Decisions` 與 `Verified Facts` 等額外區段。

驗證採 fail-closed（預設阻擋）原則。由於 `gal init` 從範本提供此檔案，這些檢查主要防範手動編輯造成的結構損壞。若必要區段遺漏或重複，`gal render-adapters` 會中止執行、回報有問題的標題，且不寫入任何檔案。每次執行只會回報一個錯誤。若遺漏多個標題，請對照 `plugins/gal-core/templates/project.md` 補齊，然後重新執行指令。

適用的大小限制有兩項：

- **`.dev/project.md` 預算**：當檔案在行尾正規化後超過 30,720 位元組時，`gal render-adapters` 會印出警告，但仍會算繪轉接器。請精簡檔案，使其維持在預算之內。
- **`AGENTS.md` 上限**：當算繪後的 `AGENTS.md` 將超過 32,768 位元組時，`gal render-adapters` 會中止，且不寫入任何轉接器檔案。產生器絕不會為了符合此上限而輸出不完整的轉接器檔案。請精簡 `.dev/project.md`，直到算繪後的檔案符合上限為止。

在 `Tech Stack` 表格下方，`.dev/project.md` 包含一個權威檢查區塊：

````markdown
<!-- gal:authoritative-check -->
```json
{
  "command": [
    "cargo test --workspace",
    "cargo clippy --workspace",
    "cargo fmt --check"
  ]
}
```
````

`gal finalize-check` 關卡會在完整模式下執行這些指令。它會依序逐一執行，即使先前的指令失敗，也會執行每一個指令。每個失敗的指令都會回報為一項失敗的檢查。

GAL 不會使用 shell 執行這些指令。它會依空白切分每個指令，並將第一個字當作程式啟動。引號、pipe、`&&`、變數內插與 shell 內建指令等 shell 功能都無法運作。每個程式都必須是存在於 `PATH` 上的執行檔。

`gal finalize-check` 會將目前的工作目錄視為儲存庫根目錄。請從儲存庫根目錄執行它。若從子目錄執行，它會找不到 `.dev/project.md`，並將檢查回報為未執行。

在沒有測試套件的文件儲存庫中，請將陣列設為單一指令，且該指令必須存在於每台執行此檢查的機器上，例如 `["git --version"]`。請勿在 Windows 上使用 `["true"]`。Windows 預設沒有 `true` 執行檔，因此檢查會失敗。

## 執行診斷 (`gal doctor`)

執行 `gal doctor` 可驗證二進位檔、標準根目錄、執行環境設定與機器設定的系統健康狀態。請在初次設定後、每次升級後，或代理程式無法辨識指令時執行此檢查。

主要的診斷檢查包括：

- **opencode 投影同步**：標記 `~/.config/opencode/` 中與標準根目錄不一致的指令與代理程式檔案。過期的檔案會產生警告，遺漏的檔案則會產生錯誤。請執行 `gal refresh` 解決這些差異。
- **Claude 技能接合點**：當 `~/.claude/skills/gal` 遺失時發出警告，並印出手動建立的步驟。`gal refresh` 指令不會自動建立此連結。
- **轉接器大小分析**：在已初始化的儲存庫中，回報 `AGENTS.md` 的位元組數。超出建議大小的檔案會產生資訊性的 `[WARNING]` 通知，但不會中斷操作。

單獨執行 `gal doctor` 不會透過無頭編碼 CLI 執行任務。不過在路由到 Codex 時，它會執行 `codex exec --help` 作為範圍受限的就緒探測。

若要測試無頭執行，請使用 `gal doctor --executor-smoke`。smoke 測試會透過管道所使用的同一條派送路徑送出一個最小的真實任務，因此會呼叫每個供應商並消耗配額。它會測試 `codex`、`claude`、`copilot`、`agy` 與 `opencode` 的 CLI 安裝、登入狀態與收據產生。smoke 測試使用自己內建的預設模型，而不是您在 `executorRouting` 中設定的模型。

使用下列選項調整 smoke 測試：

- `--executor <name>` 只測試一個執行器。
- `--timeout <seconds>` 設定每個執行器的逾時時間。預設為 300 秒。
- `--transport ssh` 測試遠端執行通道。它還需要 `--ssh-target <target>` 與 `--remote-workdir <dir>`。
- `--json` 會印出機器可讀的輸出，`--strict` 則會在任何一列未通過時以結束碼 `1` 結束。請在 CI 中同時使用兩者。

路由與模型設定請參閱 [configuration.zh-Hant.md](./configuration.zh-Hant.md#執行器路由-executorrouting)。執行工作流程請參閱 [workflows.zh-Hant.md](./workflows.zh-Hant.md#管道任務迴圈與角色)。

## 執行環境叫用語法與快取

### 各執行環境的指令語法

由於各執行環境註冊功能的方式不同，觸發語法也因工具而異：

| 執行環境 | 指令語法 | 說明 |
| --- | --- | --- |
| Claude Code | `/gal status` 或 `/gal-status` | `/gal status` 會以引數 `status` 執行 `gal` 指令，並轉送至 status 程序。 |
| OpenAI Codex | `$gal-status` 或 `$gal status` | Codex 以 `$` 或透過 `/skills` 叫用自訂技能，而不是使用 `/gal`。 |
| GitHub Copilot | `/gal-status` | 註冊為技能。 |
| Google Antigravity | `/gal-status` | 註冊為代理程式技能。 |
| opencode | `/gal-status` | 原生 Markdown 指令。 |

### 更新延遲與快取

指令更新生效所需的時間，取決於各執行環境的快取行為：

- **Claude Code**：在工作階段啟動時載入 `gal@skills-dir` 外掛。執行 `gal refresh` 後，請啟動新的 Claude Code 工作階段。市集安裝會保留快取副本，需要重新安裝並重新啟動。
- **OpenAI Codex**：從它自己的快取讀取外掛，而 `gal refresh` 不會更新該快取。升級 GAL 後，請依照 [OpenAI Codex](#openai-codex) 的說明重新安裝外掛。GAL 投影到 `~/.agents/skills/` 的技能則從該目錄讀取。若變更未出現，請重新啟動 Codex 或開啟新的對話。
- **Google Antigravity**：在行程啟動時載入指令。變更後需要重新啟動 `agy`。
- **GitHub Copilot 與 opencode**：在每個新對話工作階段開始時讀取指令與技能定義。

Codex 執行環境注意事項：

- **脈絡預算截斷**：Codex 會限制初始技能清單的大小。在脈絡預算緊縮時，它會縮短說明，或將技能從自動完成選單中省略。被省略的技能仍可使用 `$skill-name` 執行。
- **重複的技能清單項目**：若多個工具以相同名稱註冊技能，Codex 會在其選擇選單中列出這兩個項目。
