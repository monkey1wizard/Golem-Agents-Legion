# 計畫：GAL 安裝模式與 Companion Plugin 發佈

## 目標

GAL 提供一套以供應商原生 plugin install 機制為主的安裝模式，讓一般使用者不需要 clone GAL repo，也不需要在 `~/.copilot/gal`、`~/.gemini/gal` 或 `~/.gemini/antigravity-cli/gal` 建立 repo-root shortcut 才能使用 GAL。現有 clone repo 加 symlink 的方式保留為 contributor/developer mode，不再是一般使用者的預設安裝路徑。

本計畫不再把所有技能都包進單一 `gal` 外掛程式。GAL 應拆成：

- `gal-core`：GAL 控制平面、golem agents、核心工作流、必要 conventions，以及少量真正屬於 GAL 本身的 skills。
- companion plugins：依語言或領域拆分的官方外掛程式，例如 `gal-dart`、`gal-flutter`、`gal-godot`、`gal-obsidian`、`gal-web`、`gal-game-assets`。
- upstream 或 mirrored plugins：GAL 可選擇管理來自外部 GitHub 或其他來源的 skills/plugin，但必須透過 catalog、lockfile、版本與授權資訊治理，而不是無版本地直接複製進 core repo payload。

這樣的架構讓不同使用者只安裝自己需要的 plugin，也讓目前 repo 中已從外部複製、可能早已落後上游版本的 language/domain skills 有明確的升級與追蹤模型。

## 需求

- [ ] 比較並明確記錄目前所有相關 platform 的 plugin 承載能力與安裝模型：AGY CLI、Copilot CLI、Codex、Claude Code，以及 runtime support boundary 內的 Gemini legacy lane 與 OpenCode lane。
- [ ] 將靜態 provider plugin 目標鎖定為 AGY CLI、Copilot CLI、Codex 與 Claude Code；Gemini 僅作 migration/compatibility lane，OpenCode 則記錄為獨立 plugin bridge 計畫，不硬塞進相同共同模型。
- [ ] 定義 `gal-core` 與 companion plugins 的邊界，避免把所有 skills 都永遠綁進單一 `gal` 外掛程式。
- [ ] 將 C# / Dart / Flutter / Go / Godot / Obsidian / game asset 等特定語言或特定領域 skills 移出 core，改為 provider-native installable 的官方 companion plugins。
- [ ] 定義 GAL-managed plugin catalog，追蹤官方 plugin 與第三方 upstream plugin 的來源、版本、授權、checksum、支援 provider、是否 vendor/mirror，以及是否預設安裝。
- [ ] 定義 `plugins.lock.json` 或同等 lockfile，解決從 GitHub 複製 skills 後長期漂移、無法確認版本、無法安全更新的問題。
- [ ] install mode 必須使用 provider-native plugin install、marketplace、plugin cache 或已記錄的 plugin staging 路徑，而不是 provider 目標目錄下的 repo-root shortcut。
- [ ] 保留 source mode 作為 GAL 開發者流程，明確標示其 `GAL_ROOT` shortcut 只屬於本機開發、生成與相容橋接。
- [ ] 將 `~/.copilot/gal`、`~/.gemini/gal`、`~/.gemini/antigravity-cli/gal` 視為 source mode 遺留橋接，不得出現在 install mode 的成功條件內。
- [ ] 將 GAL source contracts 轉成各 provider 可安裝的 plugin artifacts；生成品不得依賴原始 repo 的絕對路徑。
- [ ] 保持 provider-specific renderer 邊界：每個 provider 只輸出該 provider 文件支援的 manifest 欄位與元件。
- [ ] 不把 `scripts/` 提升為四 provider 共同承載。需要可執行能力時，使用 provider 原生能力、skill-local helper、hooks、`bin/`，或獨立的 GAL runtime install，而不是 provider 共用 `scripts/` 欄位。
- [ ] GAL-managed MCP 預設由 plugin artifact 或 provider-native plugin MCP 機制承載；local secrets 與 machine-local paths 只在本機 install/enable 階段解析。
- [ ] companion plugin 與 upstream plugin 的安裝、更新、解除安裝必須可重跑且可驗證，不得刪除使用者自有 provider 設定。
- [ ] 文件必須清楚區分 install mode、source mode、migration cleanup、core plugin、companion plugin、upstream plugin 與 provider-native limitations。

## 方法

### 與既有 provider packaging 主線的銜接

本計畫不是目前的實作主線。當前主線仍是 [feat-gal-provider-plugin-packaging.md](c:/Code/Golem-Agents-Legion/docs/plans/feat-gal-provider-plugin-packaging.md)，且 install mode / companion plugin 發佈必須建立在該計畫完成 AGY plugin-only 基底之後。

#### 前置條件

在本計畫進入 installer 與全面 rollout 前，`feat-gal-provider-plugin-packaging` 至少應完成以下收尾工作：

- `T-006`：將 AGY MCP 完整收斂到 plugin root `mcp_config.json`，並移除 plugin 外的 GAL-managed legacy MCP 內容。
- `T-007`：將 AGY 指令語料庫穩定渲染到 `rules/gal.md`，並完成 `Update-Personalization.*` 的 install/sync 邊界。
- `T-008`：完成 `Setup-Machine` / reinstall / uninstall 的 AGY plugin-only lifecycle 與 legacy cleanup。
- `T-009`：更新文件，讓共同基底、AGY renderer 1、plugin-root MCP 與 migration lane 邊界先被正式記錄。

理由是：本計畫的 install mode、plugin-aware MCP、migration cleanup 與 provider-native installer 都直接依賴這些基礎行為；若在 AGY packaging 主線尚未收口前提前實作，容易做出只適用單一 `gal` payload 的過渡安裝器，後續還要拆掉重來。

#### 可先做的工作

即使 `feat-gal-provider-plugin-packaging` 尚未完全結束，以下工作可先以規格、盤點或非 installer 的方式推進：

- skill inventory 與 `gal-core` / companion plugin 邊界草案。
- 第三方或 mirrored skills 的來源、license、checksum、upstream ref 盤點。
- `plugins/catalog.json` 與 `plugins.lock.json` schema 設計。
- 多 plugin package model 所需的資料結構規格。

這些工作可先形成文件、schema 或分類表，但不應在 AGY lifecycle 尚未穩定前直接承諾完整的 install/update/uninstall 流程。

#### 完成 provider packaging 後的建議順序

當 `feat-gal-provider-plugin-packaging` 完成後，本計畫建議按以下順序推進：

1. 定義 `gal-core` 與 companion plugins 邊界，完成 skills inventory 與分群。
2. 建立 `plugins/catalog.json` 與 `plugins.lock.json` 的 schema 與驗證規則。
3. 將 provider-neutral package model 從單一 `gal` payload 擴成多 plugin package。
4. 讓 `Setup-Machine.*` 支援 `source mode` / `install mode` 與 plugin set selection。
5. 最後才實作 provider-native installer / updater / uninstall flows。

#### 不應提前開工的工作

下列工作在 provider packaging 主線收尾前不應直接開工：

- `Install-GalPlugins.ps1` / `install-gal-plugins.sh` 的完整 installer 本體。
- Copilot / Codex / Claude 的 provider-native install/update/uninstall lifecycle。
- 任何假設 `gal-core` 已穩定拆分完成的 marketplace 或多 plugin 發佈流程。

### 步驟 1：鎖定平台與承載邊界

- **檔案**：`docs/devguide.md`、`docs/personalization.md`、`docs/personalization.zh-Hant.md`、`scripts/scripts.md`
- **內容**：新增 platform matrix，分開記錄「靜態 provider plugin 目標」與「目前 runtime support surface」。
- **驗證**：文件至少清楚標示 AGY CLI、Copilot CLI、Codex、Claude Code、Gemini legacy lane、OpenCode lane 的能力與不在本計畫內的邊界。

#### 平台支援矩陣

| Platform | 身分 | 目前狀態 | 本計畫角色 | 備註 |
| --- | --- | --- | --- | --- |
| AGY CLI / Antigravity CLI | 靜態 provider plugin | 可行 | renderer 1 | 已有 plugin 與 CLI features 文件，但 `plugins` 路徑在不同文件間存在差異，需實測確認。 |
| Copilot CLI | 靜態 provider plugin | 可行 | renderer 2 | 支援 `copilot plugin install`、cache、skills、agents、hooks、MCP。 |
| Codex | 靜態 provider plugin | 可行 | renderer 3 | 支援 marketplace、plugin cache、skills、MCP、apps、hooks。 |
| Claude Code | 靜態 provider plugin | 可行 | renderer 4 | 支援最完整，含 skills、commands、agents、MCP、LSP、hooks、bin、scripts、dependencies。 |
| Gemini CLI | legacy / migration lane | 不作第五 renderer | migration only | 應被視為向 AGY plugin 遷移的相容層，而非新的 plugin target。 |
| OpenCode | runtime support lane | 另案可行 | out of scope | repo 目前有 OpenCode runtime 安裝邏輯，但其 plugin 模型不應硬塞進這個四 provider 共同基底。 |

#### 供應商架構比較

| 供應商 | 安裝介面 | 清單 | 支援元件 | 執行階段/腳本形態 | 快取/更新模型 | GAL 意涵 |
| --- | --- | --- | --- | --- | --- | --- |
| AGY CLI | 以 plugin staging / discovery 為主；CLI features 文件顯示 `~/.gemini/antigravity-cli/plugins/<plugin>/`，較舊 plugins 文件仍提及 `~/.gemini/config/plugins/` 或 workspace `.agents/plugins/` | `plugin.json` | `skills/`、`agents/`、`rules/`、`mcp_config.json`、`hooks.json` | skill-local helper 可行；不要假設 provider-neutral root `scripts/` | staged plugin tree 由 CLI 載入；具體路徑需冒煙驗證 | 適合做 `gal-core` 與 companion plugins，但 AGY install target 仍需實證。 |
| Copilot CLI | `copilot plugin install SPECIFICATION`；支援本地路徑、repo、Git URL、市集 | `plugin.json` | `agents/`、`skills/`、`commands`、`hooks`、`.mcp.json`、LSP | 沒有已記錄的 provider-neutral plugin root `scripts`；應避免依賴未驗證的 plugin-local runtime 變數 | 安裝到 `~/.copilot/installed-plugins/...`；本地更新需重新 install | 適合 provider-native 安裝與多 plugin 分發。 |
| Codex | marketplace UI 與 `codex plugin marketplace add` | `.codex-plugin/plugin.json` | `skills/`、`.mcp.json`、`.app.json`、`hooks/`、`assets/` | hooks 可用 `PLUGIN_ROOT` / `PLUGIN_DATA`；hooks 預設仍受 feature flag 控制 | 安裝到 `~/.codex/plugins/cache/<marketplace>/<plugin>/<version>/`；啟用狀態在 `~/.codex/config.toml` | 最適合由 GAL 維護 curated marketplace 與可選 plugin catalog。 |
| Claude Code | `claude plugin install <plugin>`；可 user/project/local/managed scope | `.claude-plugin/plugin.json` 可選 | `skills/`、`commands/`、`agents/`、`hooks/`、`.mcp.json`、`.lsp.json`、monitors、themes、`bin/`、`scripts/`、dependencies | 支援 `${CLAUDE_PLUGIN_ROOT}`、`${CLAUDE_PLUGIN_DATA}`、`bin/`、`scripts/`、userConfig、dependencies | 市集 plugin 會被複製到 `~/.claude/plugins/cache`；可 list/update/uninstall/prune | 最完整的 companion plugin 與 dependency 管理目標。 |

### 步驟 2：定義 core plugin、companion plugin 與 upstream catalog

- **檔案**：`docs/devguide.md`、`docs/personalization.md`、`docs/personalization.zh-Hant.md`、新 `plugins/catalog.json`、新 `plugins.lock.json`
- **內容**：建立三層模型：
  - `gal-core`：只放 GAL 自身必要能力。
  - `gal-*` companion plugins：官方語言/領域外掛。
  - upstream/mirrored plugins：由 GAL catalog 管理的第三方來源。
- **驗證**：任何非 core 的語言技能都能被歸類到 companion 或 upstream plugin，而不是默認塞回 `gal-core`。

#### 建議的 companion plugin 分群

| 插件 | 建議內容 |
| --- | --- |
| `gal-core` | `/gal` 指令、golem agents、核心 workflows、必要 conventions、少量 core-only skills |
| `gal-dart` | Dart skills |
| `gal-flutter` | Flutter skills |
| `gal-godot` | Godot skills |
| `gal-game-assets` | game-2d / 3d / pixel / ui / graphics workflow |
| `gal-obsidian` | Obsidian CLI / markdown / bases / knowledge management |
| `gal-web` | webapp-testing、可能的 browser/devtools 類技能 |
| `gal-docs` | doc-coauthoring、markdown-formatting、pdf、opencli/defuddle 類文檔技能 |

### 步驟 3：治理第三方與鏡像來源

- **檔案**：新 `plugins/catalog.json`、新 `plugins.lock.json`、`docs/devguide.md`
- **內容**：若 skills 是從 GitHub 或其他外部來源複製而來，不再接受「直接複製到 `skills/` 目錄」這種無 provenance 模式。catalog 至少要記錄：
  - plugin ID
  - upstream repo / path / ref 或 release tag
  - source type（official / mirrored / forked / local）
  - license
  - checksum 或 commit SHA
  - 支援 provider
  - 是否允許自動更新
  - 是否屬於預設安裝集合
- **驗證**：任何 mirror/import 的 plugin 都能追溯上游版本與授權，且 lockfile 能檢測漂移。

### 步驟 4：新增 provider-neutral package model 的 plugin 集合層

- **檔案**：`scripts/common/ProviderPlugin.ps1`、`scripts/common/provider-plugin.sh`、新 `scripts/Build-ProviderPlugins.ps1`、新 `scripts/build-provider-plugins.sh`
- **內容**：現有共同模型目前會把整個 repo `skills/` 都納入單一 payload。需改成先從 catalog 組出 plugin package，再交給各 provider renderer。artifact root 改為 `dist/provider-plugins/<provider>/<plugin-id>/`，而非固定只有 `dist/provider-plugins/agy/gal/`。
- **驗證**：可針對 `gal-core`、`gal-dart` 等不同 plugin 輸出不同成品，且不包含儲存庫根目錄的符號連結。

### 步驟 5：保留 source mode，新增 install mode 的 plugin 選擇能力

- **檔案**：`scripts/common/Common.ps1`、`scripts/common/common.sh`、`scripts/Setup-Machine.ps1`、`scripts/setup-machine.sh`
- **內容**：
  - source mode：保留現有 symlink / contributor workflow。
  - install mode：改以 provider-native plugin 安裝 `gal-core` 與選取的 companion plugins。
  - 選項需支援 `core only`、`core + profile`、`core + explicit plugins`。
- **驗證**：dry run 能顯示所選 provider 與 plugin 集合，不再只顯示單一 `gal` 安裝行動。

### 步驟 6：解耦 `GAL_ROOT` 與來源簽出路徑

- **檔案**：`commands/*/SKILL.template.md`、`scripts/Update-Commands.ps1`、`scripts/update-commands.sh`、provider renderers
- **內容**：停止將絕對路徑 `{{GAL_ROOT}}` 烘焙進 install mode 的指令技能中。install mode 指令必須改為：
  - 呼叫 provider-native plugin 內已支援的 component；或
  - 呼叫穩定的 GAL runtime install 入口；或
  - 對不支援 plugin-local runtime 的 provider 明確退化，不假設有可用的 plugin root shell 變數。
- **驗證**：生成的 install mode 指令技能不包含來源簽出絕對路徑，也不包含 `~/.copilot/gal`、`~/.gemini/gal`、`~/.gemini/antigravity-cli/gal`。

### 步驟 7：實作 provider-native plugin installer 與 updater

- **檔案**：新 `scripts/Install-GalPlugins.ps1`、新 `scripts/install-gal-plugins.sh`、provider renderers、可能的 marketplace helpers
- **內容**：
  - AGY：依實測結果將 artifacts 同步到正確的 plugin staging root。
  - Copilot CLI：對生成的本地 plugin source 執行 `copilot plugin install`。
  - Codex：生成 marketplace 並以 `codex plugin marketplace add` 或 documented local marketplace 流程安裝。
  - Claude Code：使用 `claude plugin install`、`update`、`uninstall` 與 scope。
- **驗證**：每個 provider 都能用其原生 list/details/status 介面看到 `gal-core` 與至少一個 companion plugin。

### 步驟 8：將 MCP 移至 plugin-aware ownership

- **檔案**：`mcp.json`、`mcp.local.json`、`scripts/Update-Mcp.ps1`、`scripts/update-mcp.sh`、provider renderers
- **內容**：將 canonical MCP spec 與各 provider plugin install 分開；plugin artifacts 只攜帶可分享的規範值，本機 install/enable 階段才解析 local secrets 與 machine-local paths。
- **驗證**：使用者擁有的 MCP 項目保持不變，plugin artifacts 不序列化機密，且 `gal-core` 與 companion plugin 的 MCP ownership 明確。

### 步驟 9：新增 migration cleanup 與 drift cleanup

- **檔案**：`scripts/Setup-Machine.ps1`、`scripts/setup-machine.sh`、新 `scripts/Uninstall-Machine.ps1`、新 `scripts/uninstall-machine.sh`、`scripts/scripts.md`
- **內容**：
  - source mode 遺留捷徑清理
  - GAL-managed legacy skills 清理
  - 由舊版直接複製到 repo 的 mirrored/upstream skills 漂移檢查與搬遷計畫
- **驗證**：清理只移除 GAL-managed link、plugin 或受管 cache 項，不刪除使用者自有資料。

### 步驟 10：記錄使用者安裝與 companion plugin 流程

- **檔案**：`README.md`、`README.zh-Hant.md`、`docs/personalization.md`、`docs/personalization.zh-Hant.md`、`docs/devguide.md`
- **內容**：記錄：
  - install mode 為一般使用者預設路徑
  - source mode 為 contributor/developer 路徑
  - `gal-core` 與 companion plugins 的差異
  - provider-native limitations
  - upstream plugin source governance
- **驗證**：新使用者能以單一 provider 安裝 `gal-core`，並再選裝需要的 companion plugins，而無需 clone 本儲存庫。

## 需建立或修改的檔案

- `docs/devguide.md` - 平台矩陣、core/companion/upstream 架構、catalog 與 lockfile 規則。
- `docs/personalization.md` - 使用者導向的 install mode / source mode / companion plugins 指南。
- `docs/personalization.zh-Hant.md` - 繁體中文版指南。
- `scripts/scripts.md` - setup / install / update / uninstall / catalog 同步責任更新。
- `plugins/catalog.json` - 官方與第三方 plugin catalog。
- `plugins.lock.json` - 已解析來源、版本、checksum、provider 支援、license 的 lockfile。
- `scripts/common/ProviderPlugin.ps1` - 從 catalog 組出 plugin package，支援多個 plugin ID。
- `scripts/common/provider-plugin.sh` - Bash 對應。
- `scripts/Build-ProviderPlugins.ps1` - 建置多 provider、多 plugin artifacts 的 PowerShell 入口。
- `scripts/build-provider-plugins.sh` - Bash 對應入口。
- `scripts/Install-GalPlugins.ps1` - 安裝模式 provider-native plugin installer。
- `scripts/install-gal-plugins.sh` - Bash 對應 installer。
- `scripts/Setup-Machine.ps1` - 可感知 install mode / source mode / plugin set selection 的編排。
- `scripts/setup-machine.sh` - Bash 對應的可感知模式編排。
- `scripts/Uninstall-Machine.ps1` - 解除安裝受管 plugin 與 opt-in 的 legacy source mode links。
- `scripts/uninstall-machine.sh` - Bash 對應的解除安裝。
- `scripts/Update-Commands.ps1` - 停止讓 install mode 指令技能依賴烘焙的來源簽出路徑。
- `scripts/update-commands.sh` - Bash 對應。
- `scripts/Update-Mcp.ps1` - 具備 plugin 感知能力的 MCP ownership 與舊版清理邊界。
- `scripts/update-mcp.sh` - Bash 對應。

## 測試案例

- [ ] 平台矩陣清楚區分 AGY CLI、Copilot CLI、Codex、Claude Code、Gemini legacy lane 與 OpenCode lane。
- [ ] `gal-core` 與 companion plugins 的 catalog 可列出來源、版本、license、checksum、provider 支援與預設安裝策略。
- [ ] 任何 mirrored/upstream plugin 若缺少來源、license 或版本資訊，catalog 驗證失敗。
- [ ] provider-neutral package model 不再把整個 repo `skills/` 無條件包進單一 payload。
- [ ] 建置 `dist/provider-plugins/<provider>/<plugin-id>/` 時，不產生指向成品根目錄之外的符號連結。
- [ ] 針對 Copilot 的 install mode dry run 不建立 `~/.copilot/gal`，而是使用 `copilot plugin install` 或等效原生安裝路徑安裝 `gal-core` 與選定 companion plugin。
- [ ] 針對 AGY 的 install mode dry run 不建立 `~/.gemini/gal` 或 `~/.gemini/antigravity-cli/gal`，僅寫入正確的 AGY plugin staging 目標與受管理清理行動。
- [ ] 針對 Codex 的 install mode dry run 生成 marketplace 與 plugin artifacts，且不建立在 `~/.codex/skills/` 下的指令技能符號連結。
- [ ] 針對 Claude 的 install mode dry run 使用 `claude plugin install` 或 documented local/plugin marketplace 路徑，不寫入直接的 `~/.claude/skills/*` 符號連結。
- [ ] source mode 在明確選擇時，仍會為貢獻者建立目前的儲存庫根目錄捷徑。
- [ ] install mode 的指令技能不包含 `{{GAL_ROOT}}`、來源簽出絕對路徑或 provider `gal` 捷徑路徑。
- [ ] plugin-aware MCP 輸出保留使用者擁有的全域 MCP 項目，並僅在 migration cleanup 期間移除 GAL-managed 舊版項目。
- [ ] install mode 的解除安裝只移除 GAL-managed plugins / cache / staged artifacts，不刪除使用者擁有的 provider 設定。
- [ ] drift 檢查能辨識 repo 內 mirrored skills 已落後於 catalog 鎖定版本或 upstream ref。

## 成功標準

- [ ] 一般使用者可透過 provider-native install mode 安裝 `gal-core`，無須 clone 本儲存庫。
- [ ] 一般使用者可額外安裝自己需要的 companion plugins，而非被迫安裝全部語言/領域技能。
- [ ] `.copilot/gal`、`.gemini/gal` 與 `.gemini/antigravity-cli/gal` 捷徑不再為 install mode 所需。
- [ ] source mode 仍可供 GAL 貢獻者使用，並被明確記錄為開發工作流程。
- [ ] provider artifacts 符合各 provider 有記錄的 plugin 架構，而非偽造的共用目錄佈局。
- [ ] `gal-results/` 不被用於 provider plugin artifacts。
- [ ] install mode 指令技能不嵌入絕對的來源簽出路徑。
- [ ] MCP ownership 明確、具備 plugin 感知能力，且對本地機密安全。
- [ ] companion plugin 與 upstream plugin 的來源、版本、license 與 checksum 可追溯。
- [ ] repo 不再依賴無版本、無 provenance 的「從 GitHub 直接複製 skills 進 core」模型。
- [ ] OpenCode 與 Gemini legacy 的邊界被誠實記錄，而不是假裝它們已納入同一 provider plugin substrate。

## 風險

- 如果 `gal-core` 與 companion plugin 的邊界切得不清楚，最後仍會把所有 skills 慣性塞回 core，失去拆分意義。
- 如果指令技能仍需要來源簽出腳本，install mode 只會隱藏捷徑問題，而非解決它。
- 如果 Copilot CLI 缺乏已記錄的 plugin-local runtime 入口而計畫又強依賴它，install mode 可能只能部分退化而不能完整對等。
- 如果 AGY plugin 發現路徑與目前規劃不同，AGY install mode 在成為預設前可能需要 provider-specific fallback。
- 如果第三方 skills 沒有 catalog / lockfile / license 治理，GAL 會繼續累積漂移、過期與授權不明的 payload。
- 如果 MCP 本地值渲染過早，install artifact 可能洩漏 machine-local 機密或路徑。
- 如果 cleanup 在原生 plugin install 被驗證前就執行，使用者可能會失去仍可運作的 GAL 指令。
- 如果 source mode 與 install mode 共用太多程式碼卻沒有清晰模式檢查，未來變更可能會在 install mode 意外重建 `GAL_ROOT` 捷徑。

## 待確認問題

- [ ] OQ-001 — AGY CLI 應以 `~/.gemini/antigravity-cli/plugins/<plugin>/` 為唯一 install target，還是需要同時支援 plugins 文件中記錄的 `~/.gemini/config/plugins/` 或 workspace `.agents/plugins/` 發現模型？ *(提出者：planning)*
- [ ] OQ-002 — install mode 是否需要在 `~/.gal/` 下提供獨立的 GAL runtime install，供不支援 plugin-local scripts 的 provider 共用？ *(提出者：planning)*
- [ ] OQ-003 — Copilot CLI 是否存在足夠穩定、已記錄的 plugin-local runtime 入口；若沒有，GAL 是否應避免在 Copilot companion plugin 中承諾 plugin-local 腳本執行？ *(提出者：planning)*
- [ ] OQ-004 — install mode 應在一個 provider 通過驗證後立即成為預設，還是等 AGY 與 Copilot 都通過冒煙測試後才成為預設？ *(提出者：planning)*
- [ ] OQ-005 — 哪些 skills 必須保留在 `gal-core`，哪些必須搬到 companion plugins？ *(提出者：planning)*
- [ ] OQ-006 — 對第三方 skills，GAL 應採用直接依賴遠端 plugin、鏡像 vendor、還是 fork-and-own 模式；各模式的允許條件為何？ *(提出者：planning)*

## 簽核

- 人工核准：[待處理]
- 架構審查：[待處理]
- 額外領域審查：[未請求]

## 審查結果

### 架構審查

待處理。本計畫已不只是 install mode；它同時改變 plugin product boundary、skills ownership、provider packaging 與第三方來源治理，實作前應與現有 provider plugin packaging 主線整併後再進入 `/deep-planning`。

### 商業審查

待處理。若 companion plugins 會成為公開市集或官方 catalog 發佈承諾，需補做發佈與維護承諾評估。

### 設計審查

待處理。不涉及終端使用者 UI，但涉及 install surface、plugin 命名與 catalog 分群的體驗設計。

### 工程審查

待處理。

## 測試計畫

待處理。至少應覆蓋兩層驗證：

- provider packaging 完成後的銜接驗證：確認 AGY plugin-only lifecycle、plugin-root MCP、`rules/gal.md` 與 legacy cleanup 已穩定，不再依賴 plugin 外的 GAL-managed 面。
- install mode / companion plugin 驗證：確認多 plugin package、catalog / lockfile、mode selection 與 provider-native installer 是建立在已完成的 packaging 基底上，而非繞過它。

## 任務

### 依賴中的前置工作

- [ ] A-001 — 等待 `feat-gal-provider-plugin-packaging` 完成 `T-006`、`T-007`、`T-008`、`T-009`，作為本計畫的實作前置條件。

### 可先推進的規格工作

- [ ] A-002 — 盤點現有 skills，提出 `gal-core`、官方 companion plugins、upstream/mirrored plugins 的初步分群。
- [ ] A-003 — 定義 `plugins/catalog.json` 的 schema，至少涵蓋 plugin ID、source type、upstream ref、license、checksum、supported providers、default install set。
- [ ] A-004 — 定義 `plugins.lock.json` 的 schema 與 drift detection 規則。

### provider packaging 完成後的實作順序

- [ ] A-005 — 將 provider-neutral package model 從單一 `gal` payload 擴成多 plugin package。
- [ ] A-006 — 更新 `Setup-Machine.*` 使其支援 `source mode` / `install mode` 與 plugin selection。
- [ ] A-007 — 移除 install mode 對 `{{GAL_ROOT}}` 與來源簽出絕對路徑的依賴。
- [ ] A-008 — 實作 provider-native installer / updater / uninstall flows。
