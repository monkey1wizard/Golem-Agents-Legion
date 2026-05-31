# Plan: 遷移 CLI 供應商至 `.gal/plugins/gal` 架構

## Goal

將 AGY、Copilot 與 Codex 統一到單一 GAL 正本根目錄 `~/.gal/plugins/gal` 下，由單一核心 Renderer 建置成一個**包含各供應商進入點標記的 superset canonical root**，使每個供應商**優先以 symlink/shortcut 連到該正本**（link-first），只有在 host 格式無法消費 link 時才退而 generate/dist 最小必需檔案。實作過程必須每個任務僅處理一個供應商，且順序固定為：AGY、Copilot、Codex。

> 範圍更新（2026-05-30）：`feat-install-antigravity-claude-desktop.md` 已完成（VERIFIED），它**取代了本計畫中各供應商的安裝／投影／生命週期機制部分**。本計畫的剩餘獨特價值收斂為：**renderer 統一**（superset canonical root）+ **link-first 收掉各供應商獨立 dist 樹** + Copilot/Codex 接線。詳見「Superseded」一節。

> 範圍更新（2026-05-31，實機盤點 + `agy` v1.0.3 實測）：發現兩件先前 plan 未涵蓋的事實——(1) **AGY 有三個獨立的 per-surface plugin store**（CLI / IDE / GUI-config），先前 plan 與安裝計畫只投影 CLI + GUI，**漏掉 IDE（`~/.gemini/antigravity-ide/plugins/`），該 surface 目前完全沒有 gal**；(2) 存在數個**未使用的殘留 folder/file**（`~/.gal/dist/provider-plugins/claude/`、空殼 `~/.antigravitycli`），應在本計畫一併清除。詳見新增的「Verified Findings (2026-05-31)」一節。AGY 任務範圍因此擴充：除 renderer 統一 + link-first 外，須補齊 IDE surface 並清理殘留。

## Governing Principle: Link-first（最高治理原則）

所有 GAL 檔案正本只存在於 `~/.gal/plugins/gal`（canonical superset root）。決策順序固定為：

1. **Link**：若 host 能消費 symlink/junction，provider 的外掛目錄一律連到 `~/.gal/plugins/gal`，不另複製。
2. **Host-managed copy**：若 host 用自己的安裝指令管理外掛庫（如 Claude Code marketplace install → `~/.claude/plugins/cache`、AGY GUI `agy plugin install` → `~/.gemini/config/plugins`），則由 host 指令從**正本**複製；GAL 不維護第二份 rendered 樹。
3. **Generate/dist（最後手段）**：只有當 host 需要與正本不同的 manifest schema 且無法 link、也無 host 安裝指令可用時，才 render 最小必需檔案，且內容主體仍指向正本。

目標終態：移除 `~/.gal/dist/provider-plugins/<provider>/` 這類 GAL 自維護的第二份 rendered 樹（除作為 transient build scratch 外），讓 superset canonical root 成為唯一來源。

## Superseded / Completed by feat-install-antigravity-claude-desktop.md

以下機制已由已完成的安裝計畫交付並驗證，本計畫**不得重做、只能保留並接手遷移**：

| 項目 | 安裝計畫交付狀態 | 對本計畫的影響 |
| --- | --- | --- |
| Claude Code 安裝 | marketplace 安裝（`claude plugin marketplace add ~/.gal/plugins` + `claude plugin install gal`），正本即 `~/.gal/plugins/gal`，marketplace.json 指向正本 | Claude 已用正本，無需遷移；renderer 更名時不可破壞此路徑 |
| Claude Desktop | MCP-only 安全合併進 `claude_desktop_config.json` + ledger（`Update-Mcp.ps1`） | 非 plugin provider，**不在本計畫範圍**；不得被 renderer/lifecycle 變更波及 |
| AGY 安裝機制 | CLI junction `~/.gemini/antigravity-cli/plugins/gal` → dist；GUI 經 `agy plugin install` → `~/.gemini/config/plugins/gal`（+ `~/.gemini/config/import_manifest.json` 登記）。兩路徑在 `Build-AgyPlugin.ps1` 的 `-Install` 區塊，已驗證 | AGY 的**CLI + GUI 安裝/投影已完成**；但 **IDE surface（`~/.gemini/antigravity-ide/plugins/`）未被覆蓋，是本計畫須補的缺口**（見 Verified Findings）。本計畫 AGY 任務剩餘範圍 = renderer 統一 + link-first 評估 + **補 IDE surface** + 清理殘留 |
| `Install-GalPlugins.ps1` / `Update-Mcp.ps1` | 已含 `Invoke-ClaudePluginLifecycle`、marketplace 串接、Desktop MCP 合併 | 遷移時**必須保留**這些邏輯，不可回退 |
| 測試 | `Test-InstallGalPlugins.ps1` + 新增 `Test-UpdateMcpProjection.ps1` 已涵蓋 Claude/Desktop 路徑 | 遷移改動需維持這些測試綠燈 |

## Verified Findings (2026-05-31)

實機盤點 `~/.gemini` 與 `~/.gal`，並以 `agy` v1.0.3 實測。以下為 plan 後續任務必須據以實作的事實：

### AGY 有三個獨立的 per-surface plugin store

| Surface | runtime plugin 目錄 | 目前 gal 狀態 |
| --- | --- | --- |
| CLI（agy-cli） | `~/.gemini/antigravity-cli/plugins/` | ✅ 有（junction → `~/.gal/dist/provider-plugins/agy/gal`） |
| IDE（agy-ide） | `~/.gemini/antigravity-ide/plugins/` | ❌ **缺**（僅有 `Google.securecoder`，無 gal） |
| Desktop GUI 2.0（agy2） | `~/.gemini/config/plugins/` + `~/.gemini/config/import_manifest.json` | ✅ 有（real copy；manifest 登記 `source: local-install`） |

三個 store **各自獨立**（securecoder 只在 IDE store、不在 CLI store，足證非同一份）。`agy plugin install/list` 這套指令管理的是 `~/.gemini/config/`（import_manifest），各 surface runtime 仍各讀自己的 `plugins/` 子目錄。**目標「三 surface 都跑 gal」必須三個 store 都投影到 gal。**

### `agy` v1.0.3 plugin 能力（實測）

- 子指令：`list / import [gemini|claude] / install <target> / uninstall / enable / disable / validate [path] / link <marketplace> <target>`。
- `install` 支援 `plugin@marketplace`；另有 `link <mp> <target>`「Generate link to a marketplace」。→ **AGY 的 link-first 走 marketplace 路徑可行**（與 Claude 同模式），不必然只能靠 junction/copy。
- `agy plugin validate ~/.gal/plugins/gal` → **失敗**：`missing plugin.json`。正本目前是 Claude 形狀（`.claude-plugin/plugin.json`），AGY 要的是**根 `plugin.json`**。
- `agy plugin validate ~/.gal/dist/provider-plugins/agy/gal` → **通過**：39 skills / 12 agents / 7 mcpServers，commands 與 hooks `skipped (not found)`。

**結論**：superset canonical root 完成前，AGY 無法 link/validate 正本；`agy plugin validate ~/.gal/plugins/gal` 通過是 link-first 收斂的硬性 acceptance gate。

### 未使用的殘留 folder/file（本計畫應清除）

| 路徑 | 現況 | 處置 |
| --- | --- | --- |
| `~/.gal/dist/provider-plugins/claude/` | Claude 已走 marketplace → 正本（`~/.gal/plugins/.claude-plugin/marketplace.json` → `./gal`），此 dist 副本**無人使用** | 刪除（vestigial） |
| `~/.antigravitycli`（home 下空目錄） | 空殼，無任何 surface 使用 | 刪除 |
| `~/.gal/dist/provider-plugins/agy/` | 目前 CLI junction 的目標 | **僅在** link-first 收斂（三 surface 改指正本）並驗證後才刪 |

> 範圍界線：`~/.antigravity`、`~/.antigravity-ide`、`~/.antigravity_cockpit` 是 **Antigravity 編輯器（VS Code fork）自身資料**（`.vsix` 編輯器擴充、cache），**非 GAL agent plugin、非本計畫所有**，一律不得動。GAL 清理僅限上表 GAL-owned 路徑。

## References

- [Antigravity CLI Features](https://antigravity.google/docs/cli-features) / [CLI Plugins](https://antigravity.google/docs/cli-plugins) - AGY 外掛目錄於 `~/.gemini/<surface>/plugins/<plugin_name>/`（surface = `antigravity-cli` / `antigravity-ide` / `config`），含根 `plugin.json`，及可選 `mcp_config.json`、`skills/`、`agents/`、`rules/`。`agy plugin` 支援 `install <plugin@marketplace>` 與 `link <marketplace> <target>`。

- [OpenAI Codex Plugins Build Guide](https://developers.openai.com/codex/plugins/build) - Codex 外掛需要 `.codex-plugin/plugin.json`；外掛市場會將本地端外掛路徑解析為相對於市場根目錄的路徑。
- [GitHub Copilot CLI Plugin Reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-plugin-reference) - Copilot CLI 支援供應商的生命週期指令、根目錄或相容位置的 manifest、`agents/`、`skills/`、`commands` 以及 `.mcp.json`。

## Requirements

- [ ] **R-LINKFIRST（最高優先，見 Governing Principle）**：每個供應商目標優先 link 到 `~/.gal/plugins/gal`；不能 link 才用 host 安裝指令從正本複製；都不行才 generate 最小檔案。終態移除 GAL 自維護的 `~/.gal/dist/provider-plugins/<provider>/` 第二份 rendered 樹。
- [ ] **R-AGY-IDE（補缺口）**：AGY 安裝/投影必須覆蓋**全部三個 surface**——CLI（`~/.gemini/antigravity-cli/plugins/`）、IDE（`~/.gemini/antigravity-ide/plugins/`）、GUI-config（`~/.gemini/config/plugins/` + `import_manifest.json`）。先前邏輯漏了 IDE，導致 agy-ide 無 gal。核心 renderer/協作層搬遷 AGY 安裝邏輯時必須同時補上 IDE surface，並以 link-first 決策（能 link 正本則 link，不行才 host-copy）。
- [ ] **R-CLEANUP（清理未使用項）**：移除 GAL-owned 殘留——`~/.gal/dist/provider-plugins/claude/`（marketplace 已走正本，副本無人用）與空殼 `~/.antigravitycli`；`~/.gal/dist/provider-plugins/agy/` 僅在三 surface link-first 收斂並驗證後才刪。不得觸碰非 GAL-owned 的 `~/.antigravity*` 編輯器資料目錄。
- [ ] 將以 Claude 命名的 renderer 替換為不特定於供應商的核心 renderer，命名為 `Build-CorePlugin.ps1` 與 `build-core-plugin.sh`。
- [ ] 核心 renderer 必須產出單一共享的 superset canonical root，而不是為各供應商建立獨立的套件樹，且該 root 必須**同時包含各供應商驗證載入所需的進入點標記**（例如 `.claude-plugin/plugin.json`、AGY 的根 `plugin.json` 與 `mcp_config.json`、Codex 的 `.codex-plugin/plugin.json`、Copilot 可讀的 manifest 路徑），使能 link 的供應商可直接連到正本而非各自的 dist。
- [ ] 共享的 artifact root 必須保持對 Claude 的相容性，並加入 AGY、Copilot 與 Codex 所需的供應商標記，但不引入特定於供應商的 renderer。
- [ ] 移除 `Build-AgyPlugin.ps1` 與 `build-agy-plugin.sh` 前，必須：(a) 更新所有呼叫它們的腳本、測試與文件參照；(b) **將其 `-Install` 區塊已驗證的 AGY 安裝邏輯（CLI junction + GUI/IDE `agy plugin install`）完整搬遷至核心 renderer 或協作層，不可遺失**（此邏輯由 `feat-install-antigravity-claude-desktop.md` 新增並驗證）。
- [ ] 更新 `Build-ProviderPlugins.ps1` 與 `build-provider-plugins.sh`，讓每個供應商都指向這個共享的核心產出 root，但只有在該供應商專屬的任務內，且生命週期檢查通過後，才能將其標記為 `implemented`。
- [ ] 遷移 `Install-GalPlugins.ps1` / `Update-Mcp.ps1` 時，**必須保留** `feat-install-antigravity-claude-desktop.md` 已交付的 Claude Code marketplace lifecycle 與 Claude Desktop MCP 合併邏輯，不得回退。
- [ ] 每個實作任務只能完成一個供應商的遷移。任務順序是固定的：首先是 AGY，接著是 Copilot，最後是 Codex。
- [ ] 除非目前的 Codex CLI/文件已驗證其有效，否則 Codex 支援不可宣稱已啟用 agent。Codex 任務可以將 `agents/` 作為非活躍的共享內容保留在 root 內，但其成功的門檻在於支援組件的 Codex schema/生命週期相容性。

## Approach

### Step 1: AGY 供應商遷移
- **Provider**: 僅限 AGY
- **已完成（由安裝計畫交付，不重做）**：AGY 的 **CLI + GUI** 安裝/投影——CLI junction（`~/.gemini/antigravity-cli/plugins/gal`）與 GUI `agy plugin install`（→ `~/.gemini/config/plugins/gal` + `import_manifest.json`）已在 `Build-AgyPlugin.ps1` 的 `-Install` 區塊實作並驗證。**注意：IDE surface 未涵蓋，屬本任務新增缺口。**
- **剩餘範圍**：
  - (1) renderer 統一——把 Claude renderer 更名為核心 renderer，產出 **superset canonical root**，使其同時含 AGY 需要的根 `plugin.json`、`mcp_config.json`、`rules/gal.md`、`skills/`、`agents/`。**Acceptance gate**：`agy plugin validate ~/.gal/plugins/gal` 必須通過（目前因缺根 `plugin.json` 而失敗）。
  - (2) **補 IDE surface（R-AGY-IDE）**——把 gal 投影到 `~/.gemini/antigravity-ide/plugins/gal`，使 agy-ide 也能載入。以 link-first 決策：能 link 正本則 link，不行才 host-copy。
  - (3) **link-first 收斂**——評估三個 surface（CLI junction、IDE、GUI）能否改為直接指向 `~/.gal/plugins/gal`（正本 superset）或經 `agy plugin link/install <plugin@marketplace>` 走 marketplace，藉此收掉 `~/.gal/dist/provider-plugins/agy/gal` 獨立 dist。GUI 的 `agy plugin install` 即使仍是 host-managed copy，來源也改為正本。
  - (4) **清理殘留（R-CLEANUP）**——刪 `~/.gal/dist/provider-plugins/claude/` 與空殼 `~/.antigravitycli`；`~/.gal/dist/provider-plugins/agy/` 待三 surface 收斂驗證後刪。
  - (5) 刪除 `Build-AgyPlugin.ps1` / `build-agy-plugin.sh`，把其 `-Install` 安裝邏輯（CLI + GUI + **新增 IDE**）搬遷至核心 renderer/協作層（不可遺失）。
- **Files**: `scripts/Build-ClaudePlugin.ps1`, `scripts/build-claude-plugin.sh`, `scripts/Build-AgyPlugin.ps1`, `scripts/build-agy-plugin.sh`, `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`, `scripts/Update-Personalization.ps1`, `scripts/update-personalization.sh`, `scripts/Update-Skills.ps1`, `scripts/update-skills.sh`, `scripts/Update-Commands.ps1`, `scripts/update-commands.sh`, 以及 AGY 相關的測試與文件。
- **Verify**: AGY dry run 與隔離 home 建置，證明 (a) AGY 進入點來自 superset canonical root 且 `agy plugin validate ~/.gal/plugins/gal` 通過，(b) link-first 收斂後 AGY 不再依賴獨立 dist（或明確記錄為何仍需 dist），(c) **三個 surface（CLI / IDE / GUI）皆能載入 gal**，IDE store 不再缺，(d) 舊 AGY renderer 參照已清除，(e) 安裝計畫交付的 Claude/Desktop 路徑與測試未退步，(f) 殘留 `~/.gal/dist/provider-plugins/claude/` 與 `~/.antigravitycli` 已移除，且非 GAL-owned 的 `~/.antigravity*` 編輯器目錄未被觸碰。

### Step 2: Copilot 供應商遷移
- **Provider**: 僅限 Copilot
- **Files**: `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`, `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `scripts/Test-BuildProviderPlugins.ps1`, `scripts/Test-InstallGalPlugins.ps1`, 以及 Copilot 發布與個人化文件。
- **What**: 將 Copilot 透過其原生的外掛生命週期，與同一個核心產出的 root 進行連接。確保 manifest 介面有明確暴露 Copilot 可讀的 `agents`、`skills`、`commands` 與 `mcpServers` 路徑，而不是依賴 Claude 的後備行為。
- **Verify**: 只有在本地路徑 install/list/update/uninstall 或已文件化的後備測試通過後，Copilot 的 dry run 才會回報使用核心 renderer 並進入 implemented 生命週期。

### Step 3: Codex 供應商遷移
- **Provider**: 僅限 Codex
- **Files**: `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`, `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `scripts/Test-BuildProviderPlugins.ps1`, `scripts/Test-InstallGalPlugins.ps1`, 可選的 Codex 市場中介資料, 以及 Codex 發布與個人化文件。
- **What**: 加入 Codex 的 `.codex-plugin/plugin.json` 以及將指向共享核心產出 root 的市場/生命週期連接起來。除非供應商在任務中驗證了 agents，否則將 Codex 的宣稱支援項目限制在它能驗證的組件上，特別是 skills 與 MCP。
- **Verify**: Codex 的 dry run 與供應商生命週期測試通過且沒有 schema 錯誤，且 Codex 僅能在此任務中從 `not-implemented` 變更為 `implemented`。

## Files to Create or Modify

- `[RENAME/MODIFY] scripts/Build-ClaudePlugin.ps1` - 變更為 `scripts/Build-CorePlugin.ps1` 並負責產出共享的核心 artifact root。
- `[RENAME/MODIFY] scripts/build-claude-plugin.sh` - 變更為 `scripts/build-core-plugin.sh` 以維持 Bash 同等功能。
- `[DELETE] scripts/Build-AgyPlugin.ps1` - 在 AGY 參照轉移至核心 renderer **且其 `-Install` 區塊的 AGY 安裝邏輯（CLI junction + `agy plugin install` GUI/IDE）已搬遷至核心/協作層後**才可移除。此邏輯由 `feat-install-antigravity-claude-desktop.md` 新增並驗證，刪除前必須完整保留。
- `[DELETE] scripts/build-agy-plugin.sh` - 在 AGY Bash 參照轉移至核心 renderer 後移除（Bash 同等需確認 `agy plugin install` 對應路徑）。
- `[MODIFY] scripts/Build-ProviderPlugins.ps1` - 供應商的 build plan 與執行，現在每次會將單一供應商指向核心產出 root。
- `[MODIFY] scripts/build-provider-plugins.sh` - 供應商 build plan 與執行的 Bash 版本。
- `[MODIFY] scripts/Install-GalPlugins.ps1` 與 `scripts/install-gal-plugins.sh` - 安裝模式下的協作訊息與供應商生命週期呼叫。注意：需保留 `feat-install-antigravity-claude-desktop.md` 所新增的 `Invoke-ClaudePluginLifecycle` 與 Claude Desktop MCP 注入邏輯。
- `[MODIFY] scripts/Update-Personalization.ps1`, `scripts/update-personalization.sh`, `scripts/Update-Skills.ps1`, `scripts/update-skills.sh`, `scripts/Update-Commands.ps1`, 與 `scripts/update-commands.sh` - 移除對 AGY renderer 的呼叫與過時的 dry-run 文字。
- `[MODIFY] scripts/Test-BuildProviderPlugins.ps1`, `scripts/Test-InstallGalPlugins.ps1`, 與 `scripts/Test-ProviderPluginPackage.ps1` - 更新供應商的斷言 (assertions) 與 artifact 檢查。
- `[MODIFY] scripts/scripts.md`, `docs/devguide.md`, `docs/personalization.md`, `docs/personalization.zh-Hant.md`, 與 `docs/release-matrix.md` - 在文件中記錄共享的核心 renderer 與各供應商的生命週期關卡。
- `[CREATE/MODIFY] 僅在 Codex 生命週期驗證需要時，建立 Codex 的市場中介資料。`
- `[MODIFY] AGY 安裝/投影邏輯（搬遷後的核心 renderer/協作層）` - 新增 **IDE surface** 投影 `~/.gemini/antigravity-ide/plugins/gal`，使 CLI / IDE / GUI 三個 surface 都覆蓋（R-AGY-IDE）。
- `[DELETE] ~/.gal/dist/provider-plugins/claude/` - vestigial；Claude 已走 marketplace → 正本，此副本無人使用（R-CLEANUP）。
- `[DELETE] ~/.antigravitycli` - home 下空殼目錄，無 surface 使用（R-CLEANUP）。
- `[DELETE-CONDITIONAL] ~/.gal/dist/provider-plugins/agy/` - 僅在三 surface link-first 收斂並驗證後刪除（R-CLEANUP / R-LINKFIRST）。
- `[DO-NOT-TOUCH] ~/.antigravity, ~/.antigravity-ide, ~/.antigravity_cockpit` - Antigravity 編輯器自身資料，非 GAL-owned，明確排除於清理範圍外。

## Test Cases

- [ ] TP-001 - 執行 `pwsh -File scripts/Test-ProviderPluginPackage.ps1`；預期結果：供應商中立的套件模型，在具備 AGY、Copilot、Codex 與 Claude 功能旗標下仍能通過驗證。
- [ ] TP-002 - 執行 `pwsh -File scripts/Test-BuildProviderPlugins.ps1`；預期結果：AGY、Copilot、Codex 與 Claude 的供應商計畫，僅對已完成的任務回報核心 renderer 及正確的個別生命週期狀態。
- [ ] TP-003 - 執行 `pwsh -File scripts/Test-InstallGalPlugins.ps1`；預期結果：安裝模式的 dry run 顯示核心 renderer，且無殘留的 `Build-AgyPlugin` 或 `Build-ClaudePlugin` 文字。
- [ ] TP-004 - 執行 `pwsh -File scripts/Build-ProviderPlugins.ps1 -Providers agy -DryRun -PassThru`；預期結果：在 T-001 之後，AGY 計畫使用 managed shortcut 模式（已由前一計畫確立）、核心 renderer、共享 canonical root 以及 implemented 狀態。
- [ ] TP-005 - 執行 `pwsh -File scripts/Build-ProviderPlugins.ps1 -Providers copilot -DryRun -PassThru`；預期結果：僅在 T-002 之後，Copilot 計畫使用原生生命週期模式、核心 renderer、共享 canonical root 以及 implemented 狀態。
- [ ] TP-006 - 執行 `pwsh -File scripts/Build-ProviderPlugins.ps1 -Providers codex -DryRun -PassThru`；預期結果：僅在 T-003 之後，Codex 計畫使用原生生命週期或市場模式、核心 renderer、共享 canonical root 以及 implemented 狀態。
- [ ] TP-007 - 在隔離的 `USERPROFILE` 中建置核心 artifact；預期結果：`~/.gal/plugins/gal` 應包含 `.claude-plugin/plugin.json`、任何 Copilot 要求的 root manifest 路徑、`.codex-plugin/plugin.json`、`skills/`、`commands/`、`agents/`、`.mcp.json`、AGY 的 `plugin.json`、AGY 的 `mcp_config.json`，以及依需求的 `rules/gal.md`。
- [ ] TP-008 - 若 Bash 依賴可用，執行 `bash scripts/build-provider-plugins.sh --providers agy --dry-run`，然後對 `copilot` 與 `codex` 重複此步驟；預期結果：Bash 的輸出與 PowerShell 的供應商狀態及 renderer 名稱一致。
- [ ] TP-009 - 若有供應商 CLI，僅對該任務所屬供應商執行原生 install/list/update/uninstall 的 smoke tests；預期結果：不應出現 schema 錯誤，且除了已文件化的快取/捷徑之外，不會在共享核心 root 以外建立特定於供應商的套件樹。
- [ ] TP-010 - 在已變更的文件上執行 Markdown diagnostics；預期結果：不應有 markdownlint 錯誤，且除非做為遷移歷史明確記錄，否則不應出現過時的供應商 renderer 名稱。
- [ ] TP-011 - （AGY，T-001）在隔離 home 跑 AGY 安裝/投影後，斷言**三個 surface** 各自存在 gal：`~/.gemini/antigravity-cli/plugins/gal`、`~/.gemini/antigravity-ide/plugins/gal`、`~/.gemini/config/plugins/gal`（含 `import_manifest.json` 登記）；並 `agy plugin validate <each>` 通過。預期結果：IDE surface 不再缺 gal，三 surface 皆可載入。
- [ ] TP-012 - （AGY，T-001）清理斷言：`~/.gal/dist/provider-plugins/claude/` 與 `~/.antigravitycli` 不存在；`~/.antigravity`、`~/.antigravity-ide`、`~/.antigravity_cockpit` 仍原封不動；`~/.gal/dist/provider-plugins/agy/` 僅在三 surface 已收斂指向正本時才被移除（否則保留並記錄為「真的不能 link」例外）。預期結果：GAL-owned 殘留已清除，非 GAL-owned 編輯器資料未受影響。

## Success Criteria

- [ ] AGY 成功使用共享的核心產出 artifact root，不再依賴 `Build-AgyPlugin.ps1` 或 `build-agy-plugin.sh`。
- [ ] AGY 的**三個 surface（agy-cli / agy-ide / agy2 GUI）皆能載入 gal**；先前缺漏的 IDE surface（`~/.gemini/antigravity-ide/plugins/`）已補齊。
- [ ] GAL-owned 殘留（`~/.gal/dist/provider-plugins/claude/`、`~/.antigravitycli`）已移除；非 GAL-owned 的 `~/.antigravity*` 編輯器目錄未被觸碰。
- [ ] Copilot 透過原生的外掛生命週期成功使用核心產出的 artifact root，並驗證其 agents、skills、commands 與 MCP 路徑。
- [ ] Codex 透過其文件化的外掛生命週期或市場流程成功使用核心產出的 artifact root，並在無 schema 錯誤的情況下驗證了受支援的組件。
- [ ] 只有在專屬的供應商任務通過後，`Build-ProviderPlugins.ps1` 與 `build-provider-plugins.sh` 才會將 AGY、Copilot 與 Codex 顯示為 implemented。
- [ ] 共享的 renderer 名稱與文件，不再暗示 Claude 獨佔核心 artifact 的架構。
- [ ] Claude 的基準行為，在 renderer 更名的過程中不能退步 (regress)。

## Risks

- **供應商進入點不相符 (Provider entrypoint mismatch)**：AGY、Copilot 與 Codex 並未使用完全相同的 manifest 進入點。唯有核心 renderer 產出每個供應商所需的標記時，共享的 root 才算有效。
- **虛假的 implementation 宣告**：在一次修改中切換所有 `LifecycleStatus` 會重演先前的風險，亦即在生命週期被驗證前，就把 Copilot 與 Codex 標記為完成。
- **移除 AGY renderer 後的參照斷裂**：`Update-Personalization`、`Update-Skills`、`Update-Commands`、測試及文件，目前都會直接呼叫或描述 `Build-AgyPlugin`。
- **遺失已驗證的 AGY 安裝邏輯（高）**：`Build-AgyPlugin.ps1` 的 `-Install` 區塊現含 `feat-install-antigravity-claude-desktop.md` 新增並驗證的雙路徑安裝（CLI junction + GUI/IDE `agy plugin install`）。直接刪除而未搬遷會讓 Antigravity 2.0 GUI 失去 GAL。刪除前必須先把此邏輯搬進核心 renderer/協作層並重新驗證。
- **回退 Claude Code / Desktop（高）**：`Install-GalPlugins.ps1` / `Update-Mcp.ps1` 已含 Claude Code marketplace lifecycle 與 Claude Desktop MCP 合併（已 VERIFIED）。renderer 更名與供應商接線時絕不可破壞或回退這些邏輯；`Test-InstallGalPlugins.ps1` 與 `Test-UpdateMcpProjection.ps1` 必須維持綠燈。
- **link-first 對 AGY 不一定成立（中）**：AGY 需要與 Claude 不同的根 `plugin.json` 與 `mcp_config.json`。只有在核心 renderer 確實把這些標記做進 superset canonical root 後，AGY CLI junction 才能改指正本、收掉獨立 dist。已實測：目前 `agy plugin validate ~/.gal/plugins/gal` 因缺根 `plugin.json` 而失敗，故此 gate 未過前不得收 dist。若 superset 不可行，AGY 仍須保留最小 dist（記錄為「真的不能 link」的例外）。
- **AGY IDE surface 缺漏（中）**：AGY 有三個獨立 per-surface store（CLI / IDE / GUI-config），先前安裝邏輯只投影 CLI + GUI，**IDE（`~/.gemini/antigravity-ide/plugins/`）沒有 gal**。搬遷 AGY 安裝邏輯時若沿用舊的兩 surface 假設，agy-ide 會持續無 gal。T-001 必須顯式新增 IDE 投影並以 TP-011 斷言三 surface 全綠。
- **清理誤刪非 GAL-owned 資料（中）**：`~/.antigravity`、`~/.antigravity-ide`、`~/.antigravity_cockpit` 是 Antigravity 編輯器（VS Code fork）自身的擴充/快取資料，名稱與 GAL 殘留相近但**不可刪**。清理腳本必須以白名單方式只刪 `~/.gal/dist/provider-plugins/claude/`、`~/.antigravitycli`，避免誤傷編輯器資料。
- **過度宣稱 Codex agent 功能**：目前的 Codex 外掛文件以 skills、MCP、apps 與 hooks 為主。在 Codex 任務驗證其支援前，切勿將啟用 Codex agent 列為成功標準。
- **Force/Overwrite 行為**：使用一個共享的 artifact root 意味著重複建置必須維持一致的 `-Force` 行為以及對部分失敗的處理，確保某個供應商的任務不會損毀另一個供應商已經驗證過的 root。

## Open Questions

- [x] OQ-001 - 將 `Build-ClaudePlugin.ps1` 與 `build-claude-plugin.sh` 重新命名為 `Build-CorePlugin.ps1` 與 `build-core-plugin.sh`；這比將供應商中立的 artifact 保留為 Claude 命名的腳本還要清晰。*(raised by: planning, resolved by: architecture review)*
- [x] OQ-002 - 共用的 artifact root 可以包含特定於供應商的 manifest 標記，但特定於供應商的 renderer 則不在考量範圍內。應由一個 renderer 掌控所有靜態的 artifact 檔案。*(raised by: architecture review, resolved by: architecture review)*
- [x] OQ-003 - Codex agents 不算是強制的成功門檻，直到 Codex 生命週期驗證證明其支援為止。Codex 依然必須容忍共享的 `agents/` 目錄而不產生 schema 錯誤，否則 Codex 任務必須在 Codex manifest 中明確將其排除。*(raised by: architecture review, resolved by: architecture review)*

## Approval

- Human approval: [approved 2026-05-31]
- Architect review: [re-confirmed APPROVE 2026-05-31 — 2026-05-31 additions (R-AGY-IDE, R-CLEANUP, TP-011, TP-012) are coherent with the approved architecture; structural decisions unchanged; see Architecture Review Update note (2026-05-31)]
- Additional domain review: [not triggered]

## Review Results

### Architecture Review

#### Update note (2026-05-30)

`feat-install-antigravity-claude-desktop.md` 完成後本計畫範圍更新：(1) 各供應商安裝/投影/lifecycle 機制已交付，標為 Superseded；(2) link-first 提升為最高治理原則；(3) AGY 任務縮小為 renderer 統一 + link-first 收斂 + 搬遷已驗證的 `agy plugin install` 邏輯。原架構決策（單一核心 renderer、superset canonical root、供應商依序收尾）**維持不變**，故下方原 verdict 仍適用；但因新增「不可回退 Claude/Desktop」「不可遺失 AGY 安裝邏輯」兩條硬約束，建議實作前做一次輕量架構 re-confirm。

#### Update note (2026-05-31) — Re-confirm: APPROVE

實機盤點推翻原審查的「Missing from Plan: 修訂後無」結論：AGY 實際有三個獨立 per-surface plugin store（CLI / IDE / GUI-config），原 plan 與安裝計畫只覆蓋 CLI + GUI，**IDE surface 漏投影**。新增 R-AGY-IDE（補 IDE）與 R-CLEANUP（清 GAL-owned 殘留）兩條需求、TP-011/TP-012 兩個測試、及對應 Risk。

Re-confirm 評估（2026-05-31）：
- R-AGY-IDE：第三個 surface 投影是既有 junction/copy 模式的直接延伸，無新抽象或依賴；架構上與 CLI + GUI 路徑同質。**CLEAR**
- R-CLEANUP：白名單式刪除 GAL-owned 死碼，明確排除非 GAL-owned 目錄；風險已在 Risks 節記錄。**CLEAR**
- TP-011/TP-012：新測試涵蓋新需求，無架構影響。**CLEAR**

結構決策（單一核心 renderer、superset root、依序收尾、LifecycleStatus 供應商範疇）**全部維持不變**。Re-confirm：**APPROVE**。

#### Verdict: APPROVE AFTER REVISION（原始；下方內容為更新前審查，仍有效）

原始計畫過於廣泛，因為它將 Claude 外掛的架構視為對所有供應商皆相容。目前的供應商文件顯示，共享 root 是可行的，但前提是它作為一個能容納各供應商特定 manifest 標記的超集 (superset) root。修訂後的計畫保留了架構目標，移除了特定於供應商的 renderer，並透過限制每個任務僅驗證一個供應商來防止虛假的完成宣告。

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| 將 Claude renderer 更名為核心 renderer | 將 Claude 的所有權從供應商中立的 artifact 移除 | 需要大量的參照與測試更新 | OK |
| 產出單一超集 artifact root | 避免 renderer 的分歧與重複的套件樹 | 核心 renderer 必須驗證多個 manifest schemas | OK |
| 以 AGY, Copilot, Codex 的順序來收尾各供應商 | 賦予每個供應商隔離的生命週期關卡 | 比一次性切換所有狀態還要慢 | OK |
| 保持 `LifecycleStatus` 的供應商範疇 | 防止尚未支援的 direct-install 宣稱 | Copilot 與 Codex 在它們專屬的任務通過前將維持 pending 狀態 | OK |
| 限制 Codex agent 的宣稱 | 符合現有的供應商文件及功能旗標 | Codex 未必能立即公開所有的共享組件 | OK |

#### Over-engineering Flags

- **OE-01** 如果供應商差異僅限於 manifest 檔案與安裝生命週期中介資料，則沒有必要為每個供應商建立獨立的 renderer。保留一個 renderer，並將供應商分支邏輯移到驗證/協作層中。
- **OE-02** 在三個具體的生命週期被證明可行之前，建立一個新的供應商安裝抽象層只會增加不必要的變動。在發生實質重複之前，請直接在 `Build-ProviderPlugins.*` 中使用各供應商區塊。

#### Bug Surface

- **BUG-01** 中：移除 `Build-AgyPlugin.*` 若沒有同時更新 `Update-Personalization`、`Update-Skills`、`Update-Commands`、測試以及文件，將會破壞 install 模式的重新整理功能。T-001 必須清除每一個生效中的參照。
- **BUG-02** 中：除非指令路徑是明確定義的，否則 Copilot 可能無法從 Claude 備用的 manifest 載入 `commands/`。T-002 必須驗證 Copilot 的 manifest 介面，而非僅檢查資料夾的存在。
- **BUG-03** 中：Codex 市場路徑會相對於市場根目錄進行解析。T-003 在宣稱生命週期支援前，必須先測試本地市場路徑的語意是否正確。
- **BUG-04** 低：當 root 已經存在時，針對各供應商重建共享 root 可能會失敗。核心 renderer 必須維持現有的 `-Force` 行為，且測試必須涵蓋重複建置的情境。

#### Performance Concerns

無。此計畫改變了靜態 artifact 的產生方式及本地 CLI 生命週期的檢查；它並未增加執行時期的背景服務或無限的資料載入。

#### Missing from Plan

2026-05-31 re-confirm 後：無。R-AGY-IDE（IDE surface 補齊）與 R-CLEANUP（殘留清理）已納入 Requirements、Tasks、Test Cases、Risks，覆蓋所有已知缺口。剩下的不確定性在於實作期間供應商 CLI 的可用性，這應歸屬在任務層級的測試結果而非架構問題。

#### Recommended Changes

1. 優先實作 AGY，因為它是唯一具有獨立 renderer 的供應商，且有最高破壞風險。
2. 在 AGY 任務期間，即使共享 root 已經包含 Copilot 或 Codex 的 manifest 標記，也不要將其標記為 implemented。
3. 在更名 renderer 時，保留 Claude artifact 的相容性，然後更新測試以證明更名並未導致現有的基準功能退步。
4. 將 Codex 的支援限制在有文件的組件內，除非 Codex CLI 在 T-003 中能夠驗證 agents。
5. 由於 `feat-install-antigravity-claude-desktop.md` 已完成，在實作 `Install-GalPlugins.ps1` 時必須小心保留其中引入的 Claude Code / Desktop 邏輯。

#### What's Good

- 計畫保留了 GAL 典範的 `~/.gal/plugins/gal` 所有權模型。
- 供應商的順序創造了小而容易審查的實作單元。
- renderer 的更名讓未來的維護更加直覺。

### Business Review

未觸發。計畫並未改變定價、權限、引導流程或客戶可見的商業規則。

### Design Review

未觸發。計畫並未改變 UI 佈局、視覺設計、元件或具無障礙要求的流程。

### Engineering Review

#### Verdict: CLEAR

架構修訂後，計畫已可建立。它定義了一個 renderer、一個 canonical artifact root、每個任務對應一個供應商，以及明確的供應商生命週期關卡。實作必須遵守依序任務的邊界：T-001 只能結束 AGY，T-002 只能結束 Copilot，而 T-003 只能結束 Codex。

受保護路徑提醒：此工作會影響 `scripts/` 及文件，但不需要更動 `commands/`、`conventions/`、`workflows/` 或 `templates/`，除非實作發現了新的跨 runtime 的合約變更。如果發生此情況，請先退回 `/deep-planning` 後再擴大範圍。

<!-- ENG_REVIEW: CLEAR — re-confirmed 2026-05-31; R-AGY-IDE and R-CLEANUP additions are coherent with approved architecture; ## Tasks section added; plan is implementation-ready -->

## Tasks

- [x] T-001 — AGY：rename `Build-ClaudePlugin.ps1` → `Build-CorePlugin.ps1`，產出 superset canonical root（含根 `plugin.json`/`mcp_config.json`/`rules/gal.md`；gate：`agy plugin validate ~/.gal/plugins/gal` 通過），補 IDE surface（`~/.gemini/antigravity-ide/plugins/gal`），link-first 收斂三 surface，搬遷 AGY 安裝邏輯並刪 `Build-AgyPlugin.ps1`/`build-agy-plugin.sh`，清理殘留（`dist/provider-plugins/claude/`、空殼 `~/.antigravitycli`），確認 Claude/Desktop 路徑與測試未退步。 *(573fe7c)*
  Verify: TP-001, TP-002, TP-003, TP-004, TP-007, TP-008 (若可用), TP-009 (若 AGY 可用), TP-010, TP-011, TP-012。
- [x] T-002 — Copilot：以 link-first 把 Copilot 外掛目錄連到 superset canonical root，暴露 Copilot 可讀的 agents/skills/commands/mcpServers 路徑，更新 Copilot 測試/文件。 *(2ca9305)*
  Verify: TP-001, TP-002, TP-003, TP-005, TP-007, TP-008 (若可用), TP-009 (若 Copilot 可用), TP-010。
- [x] T-003 — Codex：以 link-first 把 Codex 連到 superset canonical root，加入 `.codex-plugin/plugin.json` 標記，依 Codex lifecycle/marketplace 接線，組件支援宣稱限於已驗證項目，更新 Codex 測試/文件。 *(3f8ca6e)*
  Verify: TP-001, TP-002, TP-003, TP-006, TP-007, TP-008 (若可用), TP-009 (若 Codex 可用), TP-010。

## Test Plan

在完成每個供應商的任務後，執行 `## Test Cases` 中的測試案例，但請確保只有當前供應商的生命週期狀態會在該任務中改變。現有的供應商狀態在輪到其專屬任務之前必須維持不變。

針對每個供應商任務的最低結案要求：

- PowerShell 中靜態供應商計畫的斷言 (assertion)。
- 在隔離的 home 目錄中進行核心 artifact 形狀的斷言。
- 安裝模式的 dry-run 斷言。
- 當供應商 CLI 可用時，進行供應商原生的生命週期 smoke 測試；若無法取得，請將缺失的 CLI 記錄為阻礙，且不要將該供應商標記為 implemented。
- 每一個變更過的 Markdown 檔案皆須通過 Markdown diagnostics。

> 範圍提醒：AGY/Claude/Desktop 的**安裝機制已由 `feat-install-antigravity-claude-desktop.md` 完成**。本計畫剩餘為 renderer 統一 + link-first 收斂 + Copilot/Codex 接線。每個任務以 link-first 為決策準則：能 link 正本就 link，不行才 host-copy，最後才 generate。

- [x] T-001 - AGY：將 Claude renderer 更名為核心 renderer 並產出 superset canonical root（acceptance gate：`agy plugin validate ~/.gal/plugins/gal` 通過）；**補 IDE surface**——把 gal 投影到 `~/.gemini/antigravity-ide/plugins/gal`，使 CLI / IDE / GUI 三 surface 都載入；評估並（若可行）將三 surface 改指 `~/.gal/plugins/gal` 正本或經 `agy plugin link/install <plugin@marketplace>` 收掉獨立 dist；把 `Build-AgyPlugin.ps1` `-Install` 的安裝邏輯（junction + GUI `agy plugin install` + **新增 IDE**）搬遷至核心/協作層後刪除 `Build-AgyPlugin.ps1` 與 `build-agy-plugin.sh`；**清理殘留**——刪 `~/.gal/dist/provider-plugins/claude/` 與 `~/.antigravitycli`（白名單式，不得碰 `~/.antigravity*` 編輯器目錄），`~/.gal/dist/provider-plugins/agy/` 僅在三 surface 收斂後刪；更新 AGY 測試/文件；確認 Claude/Desktop 路徑與其測試未退步。確認驗證 TP-001, TP-002, TP-003, TP-004, TP-007, TP-008 (若可用), TP-009 (若 AGY 可用), TP-010, **TP-011, TP-012**。
- [x] T-002 - Copilot：以 link-first 把 Copilot 外掛目錄連到 superset canonical root（能 link 則 link，不行才依 Copilot 原生 lifecycle host-copy），暴露 Copilot 可讀的組件路徑，並更新 Copilot 的測試/文件。確認驗證 TP-001, TP-002, TP-003, TP-005, TP-007, TP-008 (若可用), TP-009 (若 Copilot 可用), 及 TP-010。
- [x] T-003 - Codex：以 link-first 把 Codex 連到 superset canonical root，加入 `.codex-plugin/plugin.json` 標記於正本，依 Codex 文件化 lifecycle/市場流程接線（不行才 host-copy/generate），將 Codex 的組件支援宣稱限制在經過驗證的項目，並更新 Codex 的測試/文件。確認驗證 TP-001, TP-002, TP-003, TP-006, TP-007, TP-008 (若可用), TP-009 (若 Codex 可用), 及 TP-010。
