# Plan: 將 GAL 安裝架構遷移至 `~/.gal`

## Goal

將 GAL 的安裝檔、產生的供應商外掛（provider plugins），以及個人化設定從暫時的 `$RepoRoot/dist` 及 Repository 根目錄遷移至一個集中管理的 `~/.gal` 目錄中。這將確保 Git 儲存庫保持乾淨，並正式確立標準安裝架構。

## Requirements

- [ ] 所有 GAL 產生的供應商外掛（provider plugin）必須存放在 `~/.gal/dist`（或相對應的供應商目錄）。
- [ ] 所有 GAL 管理的執行期捷徑或符號連結（runtime shortcut/symlink）必須透過 `~/.gal/active/<provider>` 或供應商安裝目標指向 `~/.gal` 內的對應資料夾；供應商不可直接指向 `$RepoRoot/dist`。
- [ ] 已實作的供應商生命週期（AGY、Claude）必須讓供應商可見的安裝目標（provider-visible install target）以符號連結/捷徑（symlink/shortcut）指向 `~/.gal/dist/provider-plugins/<provider>/gal`，避免複製（copy）後產生雙份狀態。
- [ ] 尚未實作轉譯器（renderer）的供應商（Copilot、Codex）不得在本次變更中假裝已完成原生安裝（native install）；建置計畫（build plan）必須維持 `not-implemented` 並通過測試確認。
- [ ] Repository 根目錄不得包含任何個人化設定檔（如 `*.local.*`、`xmachine.config.json`）。
- [ ] 個人化設定檔必須被讀取與寫入至 `~/.gal/config`；舊版（legacy）檔名可保留於該目錄中作為相容層。
- [ ] 個人化後的 runtime instruction projections（包含個人化 `AGENTS.md`）必須產生於 `~/.gal/generated` 或 provider-visible `.gal` 投射路徑，不得寫入 GAL source repository root。
- [ ] `GAL_SKILLS` 不得成為新的正式設定面（config surface）。它只能作為一次性的 migration input：用來把既有 skills 依 plugin 架構收編到 `.gal` 管理面，完成後必須自 `config.local.env` 移除。
- [ ] xmachine 的標準本機設定（canonical machine-local config）必須改為 `~/.gal/config/xmachine.json`，儲存庫根目錄下的 `xmachine.config.json` 僅能作為過渡備用方案（fallback）。

## Architecture Boundaries

- `~/.gal/config/`：使用者管理的本機輸入檔案（machine-local input）。包含 `config.json`、過渡期的 `config.local.env`、`model-roles.local.md`、`mcp.local.json`，以及標準的 `xmachine.json`。
- `~/.gal/store/plugins/` 與 `~/.gal/state/plugins.lock.json`：plugin 架構的 canonical 管理面。若既有 `GAL_SKILLS` 指向舊的 skills 目錄，應以 migration/import 的方式把可保留的 skills 收編到此管理面或其對應的 local plugin override，而不是在 `config.json` 或 `config.local.env` 中長期保留 `GAL_SKILLS` 路徑。
- `~/.gal/generated/`：GAL 產生的本機投射（machine-local projection），例如 MCP、xmachine 管理的投射，以及個人化後的 runtime instruction projections（包含個人化 `AGENTS.md`）。這不是使用者手寫的設定，也不得寫回 GAL source repository root。
- `~/.gal/dist/`：轉譯器（renderer）與發布打包（release packaging）的輸出根目錄。供應商外掛產出物（provider plugin artifact）應落在 `~/.gal/dist/provider-plugins/<provider>/gal`。
- `~/.gal/source/`：源始碼模式相容性連結（source-mode compatibility link），指向目前的 GAL 儲存庫檢出路徑（repo checkout）。Copilot/Gemini/Antigravity 的舊版 `GAL_ROOT` 連結若仍需存在，應指向此 `.gal` 中介，而不是直接指向儲存庫根目錄。
- `~/.gal/active/<provider>`：供執行期（runtime）與測試使用的穩定捷徑路徑名稱。消費者不應依賴 store/dist 內部的版本路徑。
- Repository 根目錄：只保留源合約（source contracts）、範例檔與文件。`config.local.env`、`mcp.local.json`、`model-roles.local.md`、`xmachine.config.json` 都是被忽略的工作區本機狀態（ignored working-tree local state），完成後應從儲存庫根目錄中移除。
- 產生的轉接器檔案（`.github/copilot-instructions.md`、`AGENTS.md`、`CLAUDE.md`、`GEMINI.md`）不得手動修改；若文件路徑規則改變，應修改源文件/範本後再由既有同步流程產生。

## Approach

### Step 1: 修改 PowerShell 共用模組的路徑定義

- **Files**: [Common.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/common/Common.ps1), [ProviderPlugin.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/common/ProviderPlugin.ps1)
- **What**:
  - 在 `Common.ps1` 增加 `GalDistRoot = ~/.gal/dist`，供應商產出物根目錄（provider artifact root）皆從此衍生，避免散落硬編碼路徑。
  - `Common.ps1` L108: `AgyPluginArtifactRoot` 從 `$repoRoot/dist/provider-plugins/agy/gal` 改為 `~/.gal/dist/provider-plugins/agy/gal`。
  - `Common.ps1`: `McpLocalFile` 從 `$repoRoot/mcp.local.json` 改為 `~/.gal/config/mcp.local.json`。
  - `ProviderPlugin.ps1` L423 `Get-AgyPluginArtifactRoot`: 同上。
  - `ProviderPlugin.ps1` L440 `Get-ClaudePluginArtifactRoot`: 從 `$RepoRoot/dist/provider-plugins/claude/gal` 改為 `~/.gal/dist/provider-plugins/claude/gal`。
- **Verify**: 用 `rg 'dist/provider-plugins|RepoRoot.*dist|repo_root.*dist' scripts/` 確認無殘留的儲存庫根目錄產出物（repo-root artifact）輸出引用；文件字串需同步更新或列入白名單（allowlist）。

### Step 2: 修改 Bash 共用模組的路徑定義

- **Files**: [common.sh](file:///c:/Code/Golem-Agents-Legion/scripts/common/common.sh), [provider-plugin.sh](file:///c:/Code/Golem-Agents-Legion/scripts/common/provider-plugin.sh)
- **What**:
  - 在 `common.sh` 增加 `GAL_DIST_ROOT="$HOME/.gal/dist"`，供應商產出物根目錄皆從此衍生。
  - `common.sh` L48: `AGY_PLUGIN_ARTIFACT_ROOT` 從 `$REPO_ROOT/dist/provider-plugins/agy/gal` 改為 `$HOME/.gal/dist/provider-plugins/agy/gal`。
  - `common.sh` L72: `MCP_LOCAL_FILE` 從 `$REPO_ROOT/mcp.local.json` 改為 `$HOME/.gal/config/mcp.local.json`。
  - `provider-plugin.sh` L405 `get_agy_plugin_artifact_root()`: 同上。
  - `provider-plugin.sh` L416 `get_claude_plugin_artifact_root()`: 從 `$repo_root/dist/provider-plugins/claude/gal` 改為 `$HOME/.gal/dist/provider-plugins/claude/gal`。
- **Verify**: 用 `rg 'dist/provider-plugins|REPO_ROOT.*/dist|repo_root.*/dist' scripts/` 確認無殘留的儲存庫根目錄產出物輸出引用；文件字串需同步更新或列入白名單。

### Step 3: 修改個人化設定檔讀取路徑

- **Files**: [Update-Personalization.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Update-Personalization.ps1), [update-personalization.sh](file:///c:/Code/Golem-Agents-Legion/scripts/update-personalization.sh), [Update-Mcp.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Update-Mcp.ps1), [update-mcp.sh](file:///c:/Code/Golem-Agents-Legion/scripts/update-mcp.sh), [gal-clean.sh](file:///c:/Code/Golem-Agents-Legion/scripts/gal-clean.sh), [gal-smudge.sh](file:///c:/Code/Golem-Agents-Legion/scripts/gal-smudge.sh)
- **What**:
  - `Update-Personalization.ps1` L215, L231: `config.local.env` 和 `model-roles.local.md` 改從 `~/.gal/config/` 讀取。
  - `update-personalization.sh` L192, L206: 同上。
  - 若 `~/.gal/config/config.local.env` 或 `~/.gal/config/model-roles.local.md` 不存在，從儲存庫的 `*.example.*` 建立至 `~/.gal/config/`，不要寫回儲存庫根目錄。
  - 將 `config.local.env` 中既有的 `GAL_SKILLS` 值視為一次性的 migration input，而非持久設定：讀取其指向的 skills 目錄，將需要保留的 skills 依既有 plugin 架構匯入/收編到 `.gal` 管理面（例如 local plugin override、store、lockfile 所描述的可解析來源），不得只是把原始路徑重新寫進 `~/.gal/config/config.json`。
  - 完成匯入後，從新生成或更新後的 `~/.gal/config/config.json` / `~/.gal/config/config.local.env` 中移除 `GAL_SKILLS`；它只可用於 migration，不能出現在最終 schema。
  - `Update-Mcp.ps1` L994: `config.local.env` 改從 `~/.gal/config/` 讀取。
  - `update-mcp.sh` L793: 同上。
  - `gal-clean.sh` L13 和 `gal-smudge.sh` L10: 主路徑（primary path）改為 `$HOME/.gal/config/config.local.env`，並提供儲存庫根目錄的舊版備用路徑（repo-root legacy fallback）。
  - `gal-clean.sh` 不可無條件透傳（passthrough）：若找不到任何設定檔且輸入內容含有疑似本機絕對路徑或敏感金鑰值（secret-like value），必須 fail closed（失敗並拒絕提交）；只有純佔位符（placeholder-only）內容可透傳並在 stderr 輸出警告。
  - `provider-plugin.sh` L139: `mcp_local_file` 同上。
  - `build-agy-plugin.sh` L209: `mcp.local.json` 同上。
- **Verify**: 用 `rg 'config\.local\.env|mcp\.local\.json|model-roles\.local|GAL_SKILLS' scripts/ docs/` 確認：
  - 所有讀寫路徑皆指向 `~/.gal/config/`，只保留明確標註為 legacy fallback、migration input 或文件訊息的引用。
  - `GAL_SKILLS` 只剩 migration/input/filter 相容用途，且不再出現在最終 config schema 或持久設定輸出中。

### Step 4: 遷移 xmachine 本機設定路徑

- **Files**: [gal.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/gal.ps1), [gal.sh](file:///c:/Code/Golem-Agents-Legion/scripts/gal.sh), [Invoke-XmachineTask.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Invoke-XmachineTask.ps1), [Invoke-XmachineTask.sh](file:///c:/Code/Golem-Agents-Legion/scripts/Invoke-XmachineTask.sh), [Invoke-XmachinePipeline.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Invoke-XmachinePipeline.ps1), [Invoke-XmachinePipeline.sh](file:///c:/Code/Golem-Agents-Legion/scripts/Invoke-XmachinePipeline.sh), [Test-Xmachine.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Test-Xmachine.ps1), [xmachine.md](file:///c:/Code/Golem-Agents-Legion/docs/collaborative-tools/xmachine.md), [xmachine.zh-Hant.md](file:///c:/Code/Golem-Agents-Legion/docs/collaborative-tools/xmachine.zh-Hant.md)
- **What**:
  - 將 xmachine 標準設定路徑改為 `~/.gal/config/xmachine.json` / `GalXmachineConfigFile`。
  - 儲存庫根目錄下的 `xmachine.config.json` 僅保留作為過渡備用；若使用了此備用方案，輸出明確警告（warning），提示使用者將檔案移至 `~/.gal/config/xmachine.json`。
  - `Update-Mcp.ps1` / `update-mcp.sh` 已有 `GalXmachineConfigFile` 與舊版儲存庫設定的概念，需確認所有分派器/測試包裝器（dispatcher/test wrapper）採用同一個解析輔助程式（resolution helper）。
  - 文件範例使用通用別名 `node-name`，不要寫入真實機器的別名。
- **Verify**: 用 `rg 'xmachine\.config\.json' scripts docs` 確認剩餘引用皆為舊版備用、範例說明或遷移警告（migration warning）。

### Step 5: 修改源始碼模式執行期橋接符號連結（source-mode runtime bridge symlink）指向

- **Files**: [Update-Skills.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Update-Skills.ps1), [update-skills.sh](file:///c:/Code/Golem-Agents-Legion/scripts/update-skills.sh), [Common.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/common/Common.ps1), [common.sh](file:///c:/Code/Golem-Agents-Legion/scripts/common/common.sh)
- **What**:
  - 增加 `GalSourceRoot` / `GAL_SOURCE_ROOT`，路徑為 `~/.gal/source`。
  - 在源始碼模式下建立 `~/.gal/source -> $RepoRoot` 的連結，再讓 `~/.copilot/gal`、`~/.gemini/gal`、`~/.gemini/antigravity-cli/gal` 指向 `~/.gal/source`。
  - 在解除安裝（uninstall）/ 執行期停用（runtime disable）時清除供應商端連結（provider-facing link）；只有當沒有執行期仍需要 source link 時才移除 `~/.gal/source`。
  - 安裝模式的供應商外掛生命週期（install-mode provider plugin lifecycle）不依賴此 source link；它僅使用 `~/.gal/dist`、`~/.gal/generated`、`~/.gal/active`。
- **Verify**: `Setup-Machine.ps1 -DryRun` 與 `setup-machine.sh --dry-run` 顯示供應商端 `GAL_ROOT` 連結的目標路徑位於 `~/.gal/source`，而不是儲存庫根目錄。

### Step 6: 修改建置腳本的輸出目標與捷徑導向

- **Files**: [Build-AgyPlugin.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Build-AgyPlugin.ps1), [build-agy-plugin.sh](file:///c:/Code/Golem-Agents-Legion/scripts/build-agy-plugin.sh), [Build-ClaudePlugin.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Build-ClaudePlugin.ps1), [build-claude-plugin.sh](file:///c:/Code/Golem-Agents-Legion/scripts/build-claude-plugin.sh), [Package-ReleaseArtifacts.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Package-ReleaseArtifacts.ps1), [package-release-artifacts.sh](file:///c:/Code/Golem-Agents-Legion/scripts/package-release-artifacts.sh)
- **What**:
  - 將建置腳本的產出物輸出路徑改為 `~/.gal/dist/provider-plugins/<provider>/gal`，不再寫入 `$RepoRoot/dist`。
  - `Build-AgyPlugin.ps1 -Install` / `build-agy-plugin.sh --install` 不應再複製（copy）產出物至供應商安裝目標；改為讓 `~/.gemini/antigravity-cli/plugins/gal` 符號連結/捷徑指向 `~/.gal/dist/provider-plugins/agy/gal`。
  - Claude 既有的 `Sync-ClaudePluginProjection` 已經是符號連結投射；確認產出物根目錄改到 `~/.gal/dist` 後生命週期狀態（lifecycle state）與階段冒煙命令（session smoke command）保持同步。
  - `Build-ProviderPlugins.ps1` / `build-provider-plugins.sh` 的 AGY `ShortcutTarget` 應指向 `~/.gal/active/agy`，而該捷徑應解析至 `~/.gal/dist/provider-plugins/agy/gal` 或供應商可見的符號連結，不能解析回儲存庫根目錄的 `dist`。
  - `Package-ReleaseArtifacts.ps1` L35: `$OutputDir` 預設值從 `.\dist\release` 改為 `$HOME/.gal/dist/release`。
  - `package-release-artifacts.sh` L17: `OUTPUT_DIR` 同上。
- **Verify**: 執行建置後 `$RepoRoot/dist` 不存在且 `~/.gal/dist` 包含產出物；供應商可見的外掛目標解析後應位於 `~/.gal/dist`。

### Step 7: 修改測試腳本

- **Files**: [Test-BuildProviderPlugins.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Test-BuildProviderPlugins.ps1), [Test-InstallGalPlugins.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Test-InstallGalPlugins.ps1), 以及對應的 Bash 冒煙測試（若存在）
- **What**:
  - L39: `$claudeArtifactRoot` 從 `$repoRoot/dist/provider-plugins/claude/gal` 改為 `~/.gal/dist/provider-plugins/claude/gal`。
  - L83: AGY `plugin.json` 路徑同步更新。
  - 測試不可污染真實的 `~/.gal`。PowerShell 測試需以暫存的 home / 暫存的 `USERPROFILE` 進行隔離，Bash 測試需以暫存的 `HOME` 進行隔離，並在 `finally` / `trap` 中進行清理。
  - 增加斷言（assertion）：Copilot、Codex 維持 `not-implemented`；AGY 與 Claude 產出物根目錄位於暫存的 `~/.gal/dist` 中；供應商端捷徑不可解析到儲存庫根目錄的 `dist`。
- **Verify**: 測試腳本在新路徑下皆能通過。

### Step 8: 更新文件與使用者說明

- **Files**: [devguide.md](file:///c:/Code/Golem-Agents-Legion/docs/devguide.md), [personalization.md](file:///c:/Code/Golem-Agents-Legion/docs/personalization.md), [personalization.zh-Hant.md](file:///c:/Code/Golem-Agents-Legion/docs/personalization.zh-Hant.md), [xmachine.md](file:///c:/Code/Golem-Agents-Legion/docs/collaborative-tools/xmachine.md), [xmachine.zh-Hant.md](file:///c:/Code/Golem-Agents-Legion/docs/collaborative-tools/xmachine.zh-Hant.md), [scripts.md](file:///c:/Code/Golem-Agents-Legion/scripts/scripts.md), [README.md](file:///c:/Code/Golem-Agents-Legion/README.md), [README.zh-Hant.md](file:///c:/Code/Golem-Agents-Legion/README.zh-Hant.md)（僅限受路徑變更影響的段落）
- **What**:
  - 將儲存庫根目錄下的本機檔案說明改為 `~/.gal/config/`。
  - 將供應商外掛產出物輸出說明改為 `~/.gal/dist/provider-plugins/...`。
  - 將 `xmachine.config.json` 的標準說明改為 `~/.gal/config/xmachine.json`，儲存庫根目錄下的檔案僅作為舊版備用。
  - 不手動修改產生的轉接器檔案（generated adapters）；必要時在實作完成後執行既有的同步流程重新產生。
- **Verify**: 用 `rg 'config\.local\.env|mcp\.local\.json|model-roles\.local|xmachine\.config\.json|dist/provider-plugins' docs README*.md scripts/scripts.md` 檢查剩餘的引用皆為範例、舊版備用或明確的遷移說明。

### Step 9: 最終比對 repo-root 個人設定與 `.gal` 對應內容

- **Files**: Repository 根目錄
- **What**:
  - 此步驟必須緊接在 Step 8 之後；只有在 Step 8 的文件更新已完成且文件搜尋檢驗通過後，才可進行。
  - 刪除前必須再次比對 repo-root 個人設定檔與 `~/.gal/config` / `~/.gal/dist` 對應內容，確認已完整複製到正確位置：
    - `config.local.env` ↔ `~/.gal/config/config.local.env`
    - `mcp.local.json` ↔ `~/.gal/config/mcp.local.json`
    - `model-roles.local.md` ↔ `~/.gal/config/model-roles.local.md`
    - `xmachine.config.json` ↔ `~/.gal/config/xmachine.json`（注意檔名不同，但內容與用途需一致）
    - `dist/` ↔ `~/.gal/dist/`（確認 repo-root 不再是唯一產出來源）
  - 比對方式不得只看檔案存在與否；必須至少逐檔確認內容一致或能說明 canonical 轉換後等價，並在必要時用 diff/hash/結構化比對留下可重跑的驗證命令。
- **Verify**:
  - 先執行文件 gate：`rg 'config\.local\.env|mcp\.local\.json|model-roles\.local|xmachine\.config\.json|dist/provider-plugins' docs README*.md scripts/scripts.md`，確認剩餘引用皆為範例、舊版備用或遷移說明。
  - 再執行資料比對 gate：逐一比對 repo-root 個人檔與 `~/.gal/config` 對應檔內容一致，並確認 `dist/` 的必要內容已出現在 `~/.gal/dist/`。

### Step 10: 清理 Repository 根目錄

- **Files**: Repository 根目錄
- **What**:
  - 此步驟必須是最後一步；只有在 Step 9 的資料比對 gate 全部通過後，才可執行。
  - 手動刪除 `dist/` 與 repo-root `.local.` 檔案（`config.local.env`、`mcp.local.json`、`model-roles.local.md`、`xmachine.config.json`）。
- **Verify**:
  - 執行 `git status --short --ignored -- config.local.env mcp.local.json model-roles.local.md xmachine.config.json dist`；清理前應只顯示 ignored files，清理後無任何輸出。
  - `git diff` 不應包含對刪除這些被忽略本機檔案的變動。

## Files to Create or Modify

- [Common.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/common/Common.ps1) — 新增 `GalDistRoot`、`GalSourceRoot`，重定義 `AgyPluginArtifactRoot`、`McpLocalFile`、以及 xmachine 設定輔助程式使用的路徑。
- [ProviderPlugin.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/common/ProviderPlugin.ps1) — 修改供應商產出物根路徑輔助程式與 MCP 本機覆蓋邊界（local override boundary）。
- [common.sh](file:///c:/Code/Golem-Agents-Legion/scripts/common/common.sh) — 新增 `GAL_DIST_ROOT`、`GAL_SOURCE_ROOT`，重定義 `AGY_PLUGIN_ARTIFACT_ROOT`、`MCP_LOCAL_FILE`。
- [provider-plugin.sh](file:///c:/Code/Golem-Agents-Legion/scripts/common/provider-plugin.sh) — 修改產出物根路徑輔助程式與 `mcp_local_file`。
- [Update-Skills.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Update-Skills.ps1) / [update-skills.sh](file:///c:/Code/Golem-Agents-Legion/scripts/update-skills.sh) — 源始碼模式下 `GAL_ROOT` 的供應商端符號連結改指向 `~/.gal/source`。
- [Update-Personalization.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Update-Personalization.ps1) / [update-personalization.sh](file:///c:/Code/Golem-Agents-Legion/scripts/update-personalization.sh) — `config.local.env`、`model-roles.local.md` 改為讀寫 `~/.gal/config/`。
- [Resolve-GalCatalog.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Resolve-GalCatalog.ps1)（若需要）— 將來自 `GAL_SKILLS` 的既有 skills 目錄收編為符合 plugin 架構的 local source/import，而非保留環境變數路徑。
- [Update-Mcp.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Update-Mcp.ps1) / [update-mcp.sh](file:///c:/Code/Golem-Agents-Legion/scripts/update-mcp.sh) — `config.local.env`、`mcp.local.json`、以及 xmachine 綁定改走 `~/.gal/config/`。
- [gal-clean.sh](file:///c:/Code/Golem-Agents-Legion/scripts/gal-clean.sh) / [gal-smudge.sh](file:///c:/Code/Golem-Agents-Legion/scripts/gal-smudge.sh) — Git 過濾器主設定路徑、舊版備用、以及 fail-closed 的清理（clean）行為。
- [gal.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/gal.ps1) / [gal.sh](file:///c:/Code/Golem-Agents-Legion/scripts/gal.sh) — xmachine 設定解析改用 `~/.gal/config/xmachine.json`，儲存庫根目錄的備用方案僅作為警告路徑。
- [Invoke-XmachineTask.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Invoke-XmachineTask.ps1) / [Invoke-XmachineTask.sh](file:///c:/Code/Golem-Agents-Legion/scripts/Invoke-XmachineTask.sh) — 同上。
- [Invoke-XmachinePipeline.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Invoke-XmachinePipeline.ps1) / [Invoke-XmachinePipeline.sh](file:///c:/Code/Golem-Agents-Legion/scripts/Invoke-XmachinePipeline.sh) — 同上。
- [Test-Xmachine.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Test-Xmachine.ps1) — 同上，並更新錯誤訊息與冒煙測試設定說明。
- [Build-AgyPlugin.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Build-AgyPlugin.ps1) / [build-agy-plugin.sh](file:///c:/Code/Golem-Agents-Legion/scripts/build-agy-plugin.sh) — 產出物根路徑、AGY 安裝投射、以及 MCP 本機覆蓋路徑。
- [Build-ClaudePlugin.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Build-ClaudePlugin.ps1) / [build-claude-plugin.sh](file:///c:/Code/Golem-Agents-Legion/scripts/build-claude-plugin.sh) — 產出物根路徑與說明文字。
- [Build-ProviderPlugins.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Build-ProviderPlugins.ps1) / [build-provider-plugins.sh](file:///c:/Code/Golem-Agents-Legion/scripts/build-provider-plugins.sh) — 建置計畫與 AGY 捷徑目標驗證。
- [Install-GalPlugins.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Install-GalPlugins.ps1) / [install-gal-plugins.sh](file:///c:/Code/Golem-Agents-Legion/scripts/install-gal-plugins.sh) — 解除安裝擁有權（uninstall ownership）、Claude 生命週期狀態、以及包含 `~/.gal/dist` 的供應商投射清理。
- [Package-ReleaseArtifacts.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Package-ReleaseArtifacts.ps1) / [package-release-artifacts.sh](file:///c:/Code/Golem-Agents-Legion/scripts/package-release-artifacts.sh) — 發布產出物預設輸出與範例。
- [Test-BuildProviderPlugins.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Test-BuildProviderPlugins.ps1) / [Test-InstallGalPlugins.ps1](file:///c:/Code/Golem-Agents-Legion/scripts/Test-InstallGalPlugins.ps1) — 暫存 home 隔離、產出物根目錄、以及捷徑斷言。
- [devguide.md](file:///c:/Code/Golem-Agents-Legion/docs/devguide.md), [personalization.md](file:///c:/Code/Golem-Agents-Legion/docs/personalization.md), [personalization.zh-Hant.md](file:///c:/Code/Golem-Agents-Legion/docs/personalization.zh-Hant.md), [xmachine.md](file:///c:/Code/Golem-Agents-Legion/docs/collaborative-tools/xmachine.md), [xmachine.zh-Hant.md](file:///c:/Code/Golem-Agents-Legion/docs/collaborative-tools/xmachine.zh-Hant.md), [scripts.md](file:///c:/Code/Golem-Agents-Legion/scripts/scripts.md), [README.md](file:///c:/Code/Golem-Agents-Legion/README.md), [README.zh-Hant.md](file:///c:/Code/Golem-Agents-Legion/README.zh-Hant.md) — 使用者可見路徑說明。

## Test Cases

- [ ] `Setup-Machine.ps1 -DryRun` → 所有執行期、設定、供應商產出物路徑均顯示為 `~/.gal/...`，源始碼模式的供應商端連結指向 `~/.gal/source`。
- [ ] `bash scripts/setup-machine.sh --dry-run` → Bash 輸出與 PowerShell 完全等價。
- [ ] `Build-ProviderPlugins.ps1 -Force` → `~/.gal/dist/provider-plugins/agy/gal/plugin.json` 與 `~/.gal/dist/provider-plugins/claude/gal/.claude-plugin/plugin.json` 存在，且 `$RepoRoot/dist` 不存在。
- [ ] `scripts/build-provider-plugins.sh --force` → Bash 建置產出完全相同的拓撲結構。
- [ ] AGY 安裝投射 → `~/.gemini/antigravity-cli/plugins/gal` 解析至 `~/.gal/dist/provider-plugins/agy/gal`，為符號連結而非複製，且非儲存庫根目錄的 `dist`。
- [ ] Claude 投射 → `~/.claude/plugins/gal` 解析至 `~/.gal/dist/provider-plugins/claude/gal`；Claude 生命週期狀態的產出物根路徑保持同步。
- [ ] Git clean/smudge 過濾器 → 當 `~/.gal/config/config.local.env` 存在時正常進行替換；缺少設定且內容僅含純佔位符時透傳並警告；缺少設定且內容疑似含有本機路徑時則 fail closed。
- [ ] `GAL_SKILLS` migration → 以 repo-root `config.local.env` 中的 `GAL_SKILLS` 作為一次性輸入，將既有 skills 收編到 `.gal` 的 plugin 管理面後，最終 `config.local.env` / `config.json` 中不再保留此欄位。
- [ ] xmachine 包裝器 → 使用 `~/.gal/config/xmachine.json` 成功解析 `node-name`；只有當儲存庫根目錄的舊版設定檔案存在時才成功退回備用並輸出警告。
- [ ] `Test-BuildProviderPlugins.ps1` → 在暫存的 home 下成功通過，並確認 Copilot/Codex 仍為 `not-implemented` 原生安裝通道。
- [ ] `Setup-Machine.ps1 -Uninstall` / `setup-machine.sh --uninstall` → 清除 GAL 管理的供應商端投射、`~/.gal/active/*`、`~/.gal/dist`；除非明確指示清除（explicit purge），否則不刪除 `~/.gal/config`。
- [ ] 文件收尾 gate → Step 9 開始前，所有 doc 搜尋結果已更新，不再把 repo-root 本機檔案描述成 canonical 路徑。
- [ ] 個人設定比對 gate → 刪除前逐一比對 `config.local.env`、`mcp.local.json`、`model-roles.local.md`、`xmachine.config.json` 與 `~/.gal/config` 對應內容一致或等價。
- [ ] 最終清理 gate → 僅在文件 gate 與資料比對 gate 都通過後，repo-root 個人設定檔與 `dist/` 才被刪除。

## Success Criteria

- [ ] `~/.gal/dist` 內包含所有已實作的供應商外掛產出物，且儲存庫根目錄的 `dist/` 不再被產生。
- [ ] `~/.gal/config` 內存放所有本機輸入檔案：`config.local.env`、`model-roles.local.md`、`mcp.local.json`、`xmachine.json`。
- [ ] 既有 `GAL_SKILLS` 所指向的 skills 已依 plugin 架構收編到 `.gal` 管理面，且 `GAL_SKILLS` 不再作為最終 machine-local 設定的一部分。
- [ ] 供應商端可見的外掛安裝目標與源始碼模式執行期連結都經過 `~/.gal` 中介，不直接指向儲存庫根目錄的 `dist` 或儲存庫根目錄的本機設定。
- [ ] 在刪除 repo-root 個人設定前，已完成一次最終資料比對，確認內容都已落在正確的 `.gal` 對應位置。
- [ ] Repository 根目錄保持乾淨：無 `dist/`、無 `config.local.env`、無 `mcp.local.json`、無 `model-roles.local.md`、無 `xmachine.config.json`。
- [ ] PowerShell 與 Bash 雙軌行為一致，且測試時以暫存的 home 進行徹底隔離。
- [ ] 文件與來源文件不再引導使用者將本機設定放置於儲存庫根目錄；而且此文件更新已在最終清理前完成。產生的轉接器檔案不進行手動修改。

## Risks

- **Git 過濾器 Fail-open 風險**：缺少設定時若直接使用 `cat` 透傳，會使已套用本機路徑（smudged）的內容被提交至儲存庫中。過濾器（clean filter）必須精準區分「純佔位符透傳」與「疑似含有本機值時 fail closed」。
- **xmachine 退回備用機制漂移**：若直接刪除儲存庫根目錄的 `xmachine.config.json` 但未更新包裝器，遠端分派（remote dispatch）將會壞掉。必須先遷移包裝器，才能清理儲存庫根目錄。
- **測試污染真實家目錄**：將產出物根路徑移至 `~/.gal` 後，既有測試若未設置暫存的 home，將會直接修改到使用者的真實安裝目錄。測試必須完全隔離並予以清理。
- **AGY 複製造成狀態分歧**：若 `-Install` 繼續使用複製的方式處理產出物，`~/.gal/dist` 與供應商安裝目標將會分裂。必須改用符號連結/捷徑進行投射。
- **文件與轉接器漂移**：README、個人化設定文件、腳本文件、以及產生的轉接器目前仍有針對儲存庫根目錄本機檔案的描述。若僅修改程式碼而不修改源頭文件，會導致使用者依然按照舊路徑操作。
- **刪除前漏比對風險**：若只因為 `~/.gal/config` 中存在同名檔就直接刪除 repo-root 個人設定，可能把尚未同步的最新本機內容刪掉。最終 cleanup 必須建立在逐檔比對通過之上。
- **`GAL_SKILLS` 直接保留風險**：若只是把舊的 `GAL_SKILLS` 路徑原封不動搬進 `.gal/config`，會繞過 catalog/lockfile/plugin ownership，讓新架構同時存在一條未納管的 skills 搜尋路徑。必須把它當 migration input 處理，收編後移除。

## Open Questions

<!-- Resolved: - [x] OQ-001 — 存在於 Repository 根目錄的舊版個人化設定檔（`*.local.*`），應該由安裝腳本自動搬移，還是直接忽略並交由使用者手動搬移？ *(raised by: planning, resolved by: user — ~/.gal/config 內已有完整對應，不需搬移)* -->

## Approval

- Human approval: [completed]
- Architect review: [done]
- Additional domain review: [not requested]

## Review Results

### Architecture Review

#### 審查結論：同意 (APPROVE)

先前的計畫無法直接建置，因為它在尚未遷移所有 xmachine 讀取器的情況下刪除了 repo-root 上的 xmachine 設定檔、導致 AGY 基於複製的安裝狀態與 `~/.gal/dist` 產生分歧，且未將測試與真實的使用者家目錄進行隔離。此修訂版本修復了這些阻礙，且範圍控制得足夠精準，以便於實作。

#### 折衷方案摘要 (Trade-off Summary)

| 決策 | 好處 | 代價 | 審查結論 |
| --- | --- | --- | --- |
| `dist` 產出移至 `~/.gal/dist` | Repo 保持乾淨；artifact 與 release output 成為 machine-local state | Debug 需查看 `~/.gal/dist`，測試要隔離 home | OK |
| Local config 移至 `~/.gal/config` | Repo 不再承載私人設定；符合既有 `config.json` / `xmachine.json` topology | 過渡期同時存在 legacy `.env` 與 JSON config | OK |
| xmachine canonical path 改為 `~/.gal/config/xmachine.json` | 刪除 repo-root `xmachine.config.json` 後 wrappers 不會壞 | 需更新多個 dispatcher/test wrapper，並保留短期 fallback | OK |
| Provider install target 改為 symlink projection | `~/.gal/dist` 成為單一 artifact source of truth | 某些 provider/平台可能對 symlink/shortcut 有限制，需要 smoke | OK |
| Copilot/Codex 維持 `not-implemented` | 避免本次變更假裝支援尚未完成的轉譯器 (renderer) | 使用者仍看不到 Copilot/Codex native plugin install | OK |

#### 潛在漏洞與缺陷 (Bug Surface)

- **[BUG-01] 高風險**: `gal-clean.sh` 若在缺少設定時直接透傳（passthrough），會把已套用本機路徑（smudged）的內容提交。
  - 觸發時機: 使用者刪除 `~/.gal/config/config.local.env` 後執行 `git add`。
  - 修復方案: 以主路徑 `~/.gal/config/config.local.env` 為主、legacy repo 為備用 fallback，僅限 placeholder-only 的內容可直接透傳；若偵測到疑似本機絕對路徑或敏感金鑰值則必須 fail closed（拒絕通過並失敗中斷）。
- **[BUG-02] 高風險**: 在刪除 repo-root 的 `xmachine.config.json` 之前未更新 `gal.*`、`Invoke-Xmachine*`、`Test-Xmachine*`，將會使 xmachine 的路由直接失效。
  - 觸發時機: 使用者清理 root 後執行 `/gal xmachine node-name to do ...` 或進行 xmachine 的冒煙測試（smoke test）。
  - 修復方案: 統一 xmachine 的設定解析輔助程式（resolution helper）：優先使用 canonical 的 `~/.gal/config/xmachine.json`，若使用 legacy root fallback 則輸出警告提示。
- **[BUG-03] 中風險**: `Test-BuildProviderPlugins.ps1` 改用 `~/.gal/dist` 後會污染開發者的真實家目錄。
  - 觸發時機: 在開發機上執行測試，覆寫了真實的 GAL 產出物（artifact）。
  - 修復方案: 使用暫存的 `USERPROFILE` / `HOME` 進行隔離，並在 `finally` / `trap` 區塊中進行清理。
- **[BUG-04] 中風險**: AGY `-Install` 複製產出物會導致 `~/.gal/dist` 與 provider 安裝目標之間產生雙份狀態。
  - 觸發時機: 重新建置產出物後，provider 目標載入的仍是舊版複製品。
  - 修復方案: provider 安裝目標改用符號連結（symlink）或捷徑（shortcut）投射（projection）。

#### 計畫遺漏事項 (Missing from Plan)

在此版本修訂後無遺漏。Bash 的對等性、Release 打包、xmachine、文件、測試、source-mode 連結，以及 provider 捷徑導向均已明確定義。

#### 建議的變更 (Recommended Changes)

1. 優先在 common 模組中實作路徑根目錄，再行遷移消費者（consumers）。這能減少重複的 `$HOME/.gal/...` 硬編碼常值。
2. 將 `~/.gal/config/config.local.env` 和 `~/.gal/config/mcp.local.json` 視為相容性過渡檔案，而非長期採用的架構。`config.json` 和自動生成的投射仍是未來的架構方向。
3. 將 `GAL_SKILLS` 明確視為 migration input：先把其指向的既有 skills 收編到 `.gal` 的 plugin 管理面，再移除此參數，不要把它重新包裝成新的永久設定。
4. 不要將此次範圍擴大到 Copilot/Codex 的 renderer 實作。在有獨立的供應商生命週期計畫之前，讓這些管道明確維持 `not-implemented`。

#### 優秀的設計點（應予保留）

- `~/.gal` 本就已經是文件記載的 runtime home，因此本次變更屬於整合優化，而非引進新平台。
- 對於本機而言，不自動遷移 root 的本地檔案是正確的決定，因為 `~/.gal/config` 內已有一份完整的複本。
- 將 repo-root 的設定作為短期 fallback 能降低在發布過渡期間的損壞風險，而又無須使其成為標準規範。

### 商業審查 (Business Review)

未要求。

### 設計審查 (Design Review)

未要求。

### 工程審查 (Engineering Review)

Verdict: CLEAR

此計畫已具備可實作的工程契約。工作切片與風險邊界一致，將 `GAL_SKILLS` 明確降為一次性 migration input，沒有把它錯誤保留成新的正式設定面；同時也保留了 Copilot/Codex `not-implemented` 邊界、xmachine fallback 過渡策略，以及「先更新文件與完成資料比對、最後才清理 repo-root」的收尾 gate。沒有發現需要退回 `/deep-planning` 的阻塞項。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | static | 執行 Step 1/2 的 path-search gate，確認 PowerShell/Bash 共用路徑與 provider artifact helper 不再輸出至 repo-root `dist/`。 | T-001 |
| TP-002 | integration | 執行 `scripts/Setup-Machine.ps1 -DryRun`，確認 `config`、`dist`、`generated`、`active` 的路徑都解析至 `~/.gal/`，且 repo-root local config 不再是 primary input。 | T-002 |
| TP-003 | integration | 準備含舊 `GAL_SKILLS` 值的 `config.local.env`，執行遷移後確認既有 skills 已依 plugin 架構收編到 `.gal` 管理面，且最終 `config.local.env` / `config.json` 不再保留 `GAL_SKILLS`。 | T-003 |
| TP-004 | manual | 以 `~/.gal/config/xmachine.json` 中的 `node-name` 執行 xmachine 解析與 fallback 警告驗證，確認 wrappers 與測試入口都使用同一解析邏輯。 | T-004 |
| TP-005 | integration | 執行 `scripts/Setup-Machine.ps1 -DryRun` 與 `bash scripts/setup-machine.sh --dry-run`，確認 source-mode `GAL_ROOT` 橋接連結導向 `~/.gal/source` 而非 repo root。 | T-005 |
| TP-006 | integration | 在暫存 home 下執行 `scripts/Test-BuildProviderPlugins.ps1`，確認 AGY/Claude 產出位於 `~/.gal/dist`，Copilot/Codex 仍為 `not-implemented`。 | T-006 |
| TP-007 | manual | 執行 provider build/install smoke，確認 AGY/Claude provider-visible install target 解析至 `~/.gal/dist/provider-plugins/...`，且不是複製品或 repo-root `dist`。 | T-006 |
| TP-008 | integration | 在暫存 `HOME` / `USERPROFILE` 下執行 PowerShell 與 Bash 測試流程，確認不污染真實 `~/.gal`，且新增斷言涵蓋 `not-implemented` 與 shortcut target。 | T-007 |
| TP-009 | static | 執行文件搜尋 gate，確認文件與操作說明已更新，對 repo-root local config、`GAL_SKILLS`、provider plugin dist 的剩餘引用只保留 legacy fallback、migration input 或明確遷移說明。 | T-008 |
| TP-010 | manual | 在 cleanup 前逐一比對 repo-root 個人設定與 `.gal` 對應檔內容；若原本存在 `GAL_SKILLS`，額外確認它已完成收編且移除。 | T-009 |
| TP-011 | manual | 只有在 TP-009 與 TP-010 通過後，執行 repo-root cleanup，並以 `git status --short --ignored -- config.local.env mcp.local.json model-roles.local.md xmachine.config.json dist` 驗證清理結果。 | T-010 |

## Tasks

- [x] T-001 — 更新 PowerShell/Bash 共用根路徑與 provider artifact helper，讓 `dist` 與 local config primary path 全部轉向 `~/.gal`。
- [x] T-002 — 遷移 `config.local.env`、`model-roles.local.md`、`mcp.local.json` 的讀寫與 filter primary path 至 `~/.gal/config`，並保留明確的 legacy fallback。
- [x] T-003 — 將 `GAL_SKILLS` 視為一次性 migration input，把既有 skills 收編到 `.gal` 的 plugin 管理面後從最終設定中移除。
- [x] T-004 — 將 xmachine canonical path 統一為 `~/.gal/config/xmachine.json`，並讓 wrappers / tests 共用同一 fallback 警告邏輯。
- [x] T-005 — 將 source-mode `GAL_ROOT` 橋接連結改為經過 `~/.gal/source`，避免 provider 直接依賴 repo-root link。
- [x] T-006 — 將 provider build/install/release output 遷移至 `~/.gal/dist`，並維持 AGY/Claude 的 symlink projection 與 Copilot/Codex `not-implemented` 邊界。
- [x] T-007 — 更新 PowerShell/Bash 測試以使用暫存 home 隔離，並補足 `.gal/dist`、shortcut target、`not-implemented` 斷言。
- [x] T-008 — 更新受影響的來源文件與操作說明；必要時走既有同步流程重產 generated adapters，但不手改 generated 檔。
- [ ] T-009 — 在 cleanup 前完成 repo-root 個人設定與 `.gal` 對應內容的最終比對，包含 `GAL_SKILLS` 收編完成與移除確認。
- [ ] T-010 — 僅在文件與資料比對 gate 通過後，清理 repo-root 被忽略的本機設定檔案與 `dist/`。
