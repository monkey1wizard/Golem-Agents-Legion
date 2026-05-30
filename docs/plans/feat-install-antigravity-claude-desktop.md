# Plan: 將 GAL 安裝至 Claude Code、Claude Desktop 與 Antigravity

## Goal

讓 GAL (Golem-Agents-Legion) 能在三個目標環境被正確安裝與載入：**Claude Code (CLI)**、**Claude Desktop (GUI app)** 與 **Antigravity CLI**。本計畫的首要任務是修正目前 **Claude Code 與 Claude Desktop 尚未正確安裝 GAL** 的問題，並把 Claude Desktop 這個全新且能力受限的目標納入既有的安裝架構，而不是另建一套並行的安裝腳本。

> 範圍校正：原始計畫把「Claude Desktop」與「Claude Code」混為一談，並假設兩者都能載入完整的 agents/skills/commands。經查證後，這個假設只對 Claude Code 與 Antigravity 成立；Claude Desktop 是 **MCP-only host**，能力上限不同，必須分開處理。

## Phasing & Priority（分期與優先序，已由 OQ-002／OQ-004 決議）

- **本計畫先於 `docs/plans/feat-plugin-arch-migration.md` 啟動**：必須先完成 Claude 的設置，才繼續其他 provider 的遷移。
- **Phase 1 — 自用安裝（最高優先，本計畫範圍）**：先把 GAL 正確安裝到作者自己的機器。
  - **P1：Claude**（Claude Code + Claude Desktop）。
  - **P2：Antigravity**。
- **Phase 2 — 上架 marketplace（後續目標）**：將 GAL 發佈到 Claude 等 marketplace 供他人下載。
- **設計收斂**：正本永遠是 `~/.gal/plugins/gal`（R-6 link-first）。Claude Code 自用優先用 **symlink 就地載入**；若 link 無法持久載入，才退而產出 **local/git marketplace** manifest（指向同一正本、不複製），而這份 marketplace 未來公開即完成 Phase 2 上架，**不需重做**。Claude Desktop 自用做設定檔安全合併（link 不適用），`.mcpb` 作為可分發成品列入 Phase 2。

## References（已查證官方文件）

- [Claude Code — Create plugins](https://code.claude.com/docs/en/plugins)：外掛 manifest 為 `.claude-plugin/plugin.json`（**僅** `plugin.json` 放在 `.claude-plugin/` 內，其餘元件放外掛根目錄）；元件目錄為 `skills/`、`commands/`、`agents/`、`hooks/hooks.json`、`.mcp.json`、`.lsp.json`、`bin/`、`settings.json`。本地開發載入用 `claude --plugin-dir <root>`（亦支援 `.zip` 與 `--plugin-url`）；`/reload-plugins` 熱載。**沒有** path-based 的 `claude plugin install <local-path>`；持久安裝走 marketplace（`/plugin marketplace add <repo>` + `/plugin install`）或 skills-dir 自動載入（`claude plugin init` 產生 `~/.claude/skills/<name>/`，下次 session 以 `<name>@skills-dir` 載入）。`claude plugin validate` 可驗證 artifact。
- [MCP — Connect to local MCP servers (Claude Desktop)](https://modelcontextprotocol.io/quickstart/user)：設定檔位置 macOS `~/Library/Application Support/Claude/claude_desktop_config.json`、Windows `%APPDATA%\Claude\claude_desktop_config.json`；結構為 `{ "mcpServers": { "<name>": { "command", "args", "env" } } }`；需**完全重啟** Claude Desktop 才會載入。Windows 有 `${APPDATA}` ENOENT 陷阱（需在 `env` 補 `APPDATA`）。
- [Anthropic — Desktop Extensions (`.mcpb` / 原 `.dxt`)](https://www.anthropic.com/engineering/desktop-extensions)：將 MCP server 與相依封裝為單一 `.mcpb`，含 `manifest.json`（`mcpb_version`、`server` 區塊），雙擊一鍵安裝、自動更新、keychain 存密鑰。
- Antigravity CLI（`agy`，已取代 Gemini CLI）：skills 於 `~/.gemini/antigravity-cli/skills/`（全域）、`~/.gemini/skills/`（共享）、`.agents/skills/`（per-workspace）；MCP 設定於 `~/.gemini/config/mcp_config.json`，且 HTTP server 的 `url` 欄位須改名為 `serverUrl`；舊 `~/.gemini/` extensions 會被轉成 Plugins；`agy inspect` 可檢視已載入的 MCP/Plugins/Hooks。來源：[Configuring MCP Servers and Skills for Antigravity CLI and IDE](https://medium.com/google-cloud/configuring-mcp-servers-and-skills-for-antigravity-cli-and-ide-a938c7eebb78)、[Antigravity CLI Hands-On Guide](https://dev.to/arindam_1729/antigravity-cli-a-hands-on-guide-to-googles-terminal-coding-agent-5bc7)、[官方 CLI features 頁](https://antigravity.google/docs/cli-features)（JS 渲染，未能直接擷取內文）。

## Current State（現況分析）

這是本次修訂的核心，所有需求與步驟都建立在以下已驗證事實上。

### 既有安裝架構

- 標準外掛產出根目錄是 `~/.gal/plugins/gal`（canonical root），**不是**原計畫寫的 `.gal/plugin`。
- 安裝協調由 `scripts/Install-GalPlugins.ps1` / `scripts/install-gal-plugins.sh` 統一負責，供應商套件由 `scripts/Build-ProviderPlugins.ps1` / `scripts/build-provider-plugins.sh` 建置。**沒有**、也不應該有 per-provider 的 `Install-AntigravityPlugin.ps1` / `Install-ClaudeDesktopPlugin.ps1`。
- 供應商模型定義於 `plugins/catalog.json`，目前 `supportedProviders` 為 `["claude", "copilot", "codex", "agy"]`。**Claude Desktop 不是已知供應商**，在 catalog、腳本、provider lane 中都不存在。

### 各目標的能力上限

| 目標 | GAL 供應商 | 安裝機制 | 可載入元件 |
| --- | --- | --- | --- |
| **Claude Code (CLI)** | `claude` | `claude plugin validate/install`、`claude --plugin-dir`，投影到 `~/.claude/plugins` | agents / skills / commands / MCP（完整） |
| **Antigravity CLI** | `agy` | managed shortcut 指向 `~/.gemini/antigravity-cli/plugins/gal` | plugin.json / mcp_config.json / skills / agents / rules（完整） |
| **Claude Desktop (GUI)** | （無，需新增） | 合併 MCP 設定進 `claude_desktop_config.json`，或封裝為 `.mcpb` desktop extension | **僅 MCP server**；不支援 agents/skills/commands |

### GAL 的 MCP 實際內容

`mcp.json` 是一份**第三方 MCP server 的精選目錄**（chrome-devtools、firebase、github、microsoftdocs、markitdown、playwright、context7），GAL **本身不提供自有的 MCP server**。因此「把 GAL 裝進 Claude Desktop」實際上只能是：把這份精選 MCP server 清單注入 Claude Desktop 的設定，而非載入 GAL 的工作流程指令或 golem agents。

### Claude Code 為何「沒有正確安裝」

`Install-GalPlugins.ps1` 內的 `Invoke-ClaudePluginLifecycle` 已實作 Claude Code 的生命週期（validate → 投影 → 記錄 managed.json），但會依本地 `claude` CLI 能力降級：

- 找不到 `claude` CLI 時，只建 artifact、不驗證。
- CLI 不支援 `claude plugin validate` 時，記為 artifact-only。
- 本地 path-based install 未被 CLI 文件化時，退回 `claude --plugin-dir` session-load 後備。

→ 需在實作前先**診斷本機目前落在哪一條降級路徑**，再決定修正點。這是任務層級的調查，不是架構問題。

## Requirements

- [ ] R-1（Claude Code）：診斷並修正 `claude` 供應商安裝路徑，使 GAL 的 agents/skills/commands/MCP 能在 Claude Code 被正確載入；沿用既有 `Install-GalPlugins.ps1` 的 `Invoke-ClaudePluginLifecycle`，不另建腳本。**前提待驗證**：官方文件未記載「在 `~/.claude/plugins` 放 symlink 即自動載入」的途徑；Claude Code 的持久載入是 marketplace 安裝、skills-dir、或 `--plugin-dir`。因此 R-6 link-first 對 Claude Code 是否成立，必須先由診斷 spike 證實，不可預設。
- [ ] R-2（Claude Desktop）：新增 Claude Desktop 整合，將 `mcp.json` 中**可用且免額外認證**的 stdio server **安全合併**進 `claude_desktop_config.json`（保留使用者既有設定）。需認證/帶密鑰/HTTP-remote 的 server（github、microsoftdocs、context7）**Phase 1 不自動注入**，列為 opt-in 或走 Connectors（見 R-7、Risks）。`.mcpb` 封裝延至 Phase 2。
- [ ] R-7（Desktop 注入安全與可逆）：Desktop MCP 注入必須 (a) 不寫入未解析的密鑰佔位符（如 `${CONTEXT7_API_KEY}`）；(b) 將 GAL 寫入的 server key 記錄到 `managed.json` 之類的 ledger，使 uninstall 只移除 GAL 自己加的條目、不動使用者既有 server；(c) 寫入前備份、冪等可重入。
- [ ] R-3（Antigravity）：驗證 `agy` 供應商安裝在 `~/.gemini/antigravity-cli/plugins/gal` 仍正常；本計畫不重做 AGY 架構（見 Risks 與 in-flight 計畫衝突）。
- [ ] R-4（能力誠實）：文件與成功標準必須區分各目標能力上限。**不得**宣稱 Claude Desktop 能載入 agents/skills/commands。
- [ ] R-5（架構一致）：所有新增工作都接到既有的 catalog/provider/orchestration 模型，不得旁路腳本。Claude Desktop 依 OQ-003 定案歸屬於 **MCP 投影層的特例目標**（非 plugin provider），其登記點是 MCP 投影層而非 plugin renderer/lifecycle。
- [ ] R-6（link-first 全域原則）：所有 GAL 檔案正本只存在於 `~/.gal`（canonical `~/.gal/plugins/gal`）。每個 provider 目標**優先以 symlink/shortcut 連到 `~/.gal/plugins`**，使內容單一來源、就地更新、零複製漂移。**只有在目標 host 的格式無法消費 link 時，才退而 render/generate**（且僅產生 host 必需的最小檔案，例如 manifest 或 schema 轉換後的設定，內容主體仍指向 `~/.gal/plugins/gal`）。此原則與現有程式碼一致：`Sync-ClaudePluginProjection` 已用 `New-SafeSymlink`、AGY 已用 managed shortcut。

## Approach

### Step 1: 修正 Claude Code (CLI) 安裝
- **Diagnose（gating spike，必為第一個任務）**：以 `pwsh -File scripts/Install-GalPlugins.ps1 -DryRun` 與 `claude plugin validate --help`、`claude plugin install --help` 判定本機落點；**並實測** symlink 投影後 Claude Code 是否真的會自動載入該外掛（無 marketplace、無 `--plugin-dir`）。這個 spike 的結果決定 Primary 是 link 還是 marketplace——在它有結論前，不得把任一條當成既定主路徑。
- **Primary 候選 A（link-first，依 R-6，待 spike 證實）**：以 symlink 把 canonical `~/.gal/plugins/gal` 投影到 Claude Code 的外掛探索位置（現有 `Sync-ClaudePluginProjection`／`New-SafeSymlink`），讓其就地載入正本、零複製。**風險**：官方文件未保證裸 symlink 會被自動探索；若不成立則此候選作廢。
- **Primary 候選 B（local marketplace，spike 否定 A 時轉正）**：產出最小 `.claude-plugin/marketplace.json` 指向 `~/.gal/plugins/gal`，走 `/plugin marketplace add` + `/plugin install`。即使走 B，marketplace 來源仍指向同一正本以符合 R-6 的「單一正本、不複製」精神。
- **Phase 2 收斂**：無論 spike 選 A 或 B，候選 B 的 `marketplace.json` 都是 Phase 2 公開上架的同一份成品（公開同 repo 即完成），不重做。`--plugin-dir`／skills-dir 僅作開發與最終後備。
- **Fix**：依 spike 結論，讓 `Install-GalPlugins.ps1` 走定案的 Primary 路徑並更新 `managed.json`；若為 B，追加 marketplace manifest 並串接 `/plugin` 生命週期。
- **Verify**：link 路徑下啟動 Claude Code 能列出 GAL 的 commands/agents/skills 並執行 `/gal status`；若走 fallback，另驗 `/plugin install`／`/plugin update`／`/plugin uninstall`。

### Step 2: 新增 Claude Desktop (GUI) MCP 整合
- **link-first 不適用（依 R-6 落入 generate 例外）**：Claude Desktop 沒有外掛根目錄可供 symlink，唯一介面是共享的 `claude_desktop_config.json`（使用者既有設定也在裡面）。因此這是 R-6 明訂的「link 無法消費 → 才 generate」例外：必須做 schema 轉換 + 安全合併，而非連結。
- **Schema 轉換（關鍵）**：GAL `mcp.json` 與 Claude Desktop 設定 schema **不相同**，不能原樣複製：
  - 頂層 key `servers` → `mcpServers`。
  - `type: "stdio"` server（chrome-devtools、firebase、markitdown、playwright）：保留 `command`/`args`/`env`，移除 `type`。
  - `type: "http"` server（github、microsoftdocs、context7）：Claude Desktop 設定檔**不原生支援** HTTP/remote server，且這三者多需認證（context7 要 `CONTEXT7_API_KEY`、github-mcp 要 token）。**Phase 1 不自動注入**——避免在使用者機器上產生一堆連不上的紅燈 server。列為 opt-in 或引導使用者走 Connectors/Integrations UI。
  - server key 含斜線（`chromedevtools/chrome-devtools-mcp`、`upstash/context7`）必須正規化為合法名稱。
- **Phase 1 注入清單（限縮）**：僅注入免認證、可直接 stdio 啟動者：`chrome-devtools`、`firebase`、`markitdown`、`playwright`。
- **Method**：偵測設定檔（Windows `%APPDATA%\Claude\claude_desktop_config.json`；macOS `~/Library/Application Support/Claude/claude_desktop_config.json`），先備份，再以**冪等安全合併**把轉換後的 server 併入 `mcpServers`；保留使用者既有 server，將 GAL 寫入的 key 記錄到 ledger（`managed.json`）供精準 uninstall（R-7）。**絕不寫入未解析的密鑰佔位符**（如 `${CONTEXT7_API_KEY}`）。處理 Windows `${APPDATA}` ENOENT 陷阱（必要時在 `env` 補 `APPDATA`）。
- **Alternative**：封裝為 `.mcpb`（原 `.dxt`）desktop extension（含 `manifest.json` 的 `mcpb_version`/`server`），一鍵安裝、可自動更新、密鑰入 keychain；維護成本較高（見 OQ-002）。
- **Verify**：完全重啟 Claude Desktop，輸入框右下出現 MCP 指示器且 GAL server 工具可呼叫；確認既有 server 未被覆蓋；必要時查 `%APPDATA%\Claude\logs\mcp*.log` / `~/Library/Logs/Claude`。
- **限制聲明**：Claude Desktop 不會、也不應出現 GAL 的 commands/agents/skills（MCP-only）。

### Step 3: 驗證 Antigravity 並協調架構衝突
- **Primary（link-first，依 R-6）**：`agy` 安裝應為 managed shortcut／symlink，由 `~/.gemini/antigravity-cli/plugins/gal` 連到 canonical `~/.gal/plugins/gal`（即現有行為），不另複製外掛內容。
- **Method**：以 dry run 驗證 `agy` 供應商投影仍是指向 `~/.gal/plugins/gal` 的 link；以 `agy inspect` 確認 GAL 的 MCP/Plugins/skills 已載入。
- **Schema 注意**：Antigravity 共享 MCP 走 `~/.gemini/config/mcp_config.json`，且 HTTP server 的 `url` 欄位須改名為 `serverUrl`（與 Claude Desktop 的 HTTP 處理又不同），轉換邏輯需各目標獨立。
- **Coordinate**：因 `feat-plugin-arch-migration.md` 正在改寫 AGY/Copilot/Codex 的 renderer，本步驟必須排在該遷移之後或與其協調，避免同時改動相同腳本（見 Risks 與 OQ-004）。

## Files to Create or Modify

- `[NEW, fallback only] .claude-plugin/marketplace.json`（或 GAL 產出到 `~/.gal` 下的等效 marketplace manifest）：**僅在 link-first 無法持久載入時才產生**，指向同一正本 `~/.gal/plugins/gal`、不複製內容；亦為 Phase 2 公開上架的同一份成品。
- `[MODIFY] scripts/Install-GalPlugins.ps1` 與 `scripts/install-gal-plugins.sh`：以 link-first 確保 Claude Code symlink 投影（`Sync-ClaudePluginProjection`）成立；link 不足時才追加 marketplace manifest 並串接 `/plugin` 生命週期；新增 Claude Desktop 的 MCP 注入協調呼叫。
- `[NEW] Claude Desktop MCP 注入邏輯`：優先放在既有 MCP 投影層（`scripts/Update-Mcp.ps1` / `scripts/update-mcp.sh`）或新增專責函式於 `scripts/common/ProviderPlugin.ps1`，需含安全 JSON 合併。**不**新增 per-provider 安裝腳本。
- `[MODIFY] plugins/catalog.json`（限縮）：依 OQ-003，Claude Desktop **不**加入 `supportedProviders`（那會牽動 plugin renderer/lifecycle）。最多在執行期 runtime 選取層加一個 MCP-only 目標標記；若 MCP 投影層不需 catalog 介入，本檔可不動。
- `[MODIFY] docs/release-matrix.md`：記錄 Claude Code、Claude Desktop（MCP-only）、Antigravity 的安裝支援度與能力差異。
- `[VERIFY ONLY] scripts/Build-ProviderPlugins.ps1` 等 AGY 相關腳本：除非與 `feat-plugin-arch-migration.md` 協調，否則不在此計畫改動。

## Test Cases

- [ ] TC-1（Claude Code）：`pwsh -File scripts/Install-GalPlugins.ps1 -DryRun` 顯示 `claude` 走到 validation/投影成功路徑；接著 `claude --plugin-dir` 能載入並執行 `/gal status`。
- [ ] TC-2（Claude Desktop 合併安全性）：對含有既有 `mcpServers` 的 `claude_desktop_config.json` 執行注入，斷言既有 server 全數保留、GAL server 正確加入、JSON 結構有效。
- [ ] TC-3（Claude Desktop 載入）：重啟 Claude Desktop 後注入的 MCP server 可被呼叫。
- [ ] TC-4（能力邊界）：確認 Claude Desktop **未**出現 GAL commands/agents/skills（負向測試，符合 R-4）。
- [ ] TC-5（Antigravity）：AGY dry run 顯示 managed shortcut 仍指向 `~/.gemini/antigravity-cli/plugins/gal`。
- [ ] TC-6（跨平台）：Windows 與 macOS 路徑解析皆正確；Bash 與 PowerShell 行為一致。

## Success Criteria

- [ ] Claude Code 能正確載入並執行完整 GAL 外掛（agents/skills/commands/MCP）。
- [ ] Claude Desktop 取得 GAL 精選中**免認證的 stdio MCP server**（chrome-devtools、firebase、markitdown、playwright），使用者既有設定 100% 保留，且無明文密鑰落地；uninstall 能只移除 GAL 寫入的 key。
- [ ] 文件明確標示 Claude Desktop 為 MCP-only，不過度宣稱其能載入 agents/skills/commands。
- [ ] Antigravity 安裝經驗證仍正常，且本計畫未與 in-flight 遷移計畫造成腳本衝突。
- [ ] 所有新增工作均接入既有 catalog/orchestration 模型，未引入旁路的 per-provider 安裝腳本。
- [ ] 遵守 R-6 link-first：能用 link 的目標（Claude Code、Antigravity）以 symlink/shortcut 連到 `~/.gal/plugins/gal`，未複製外掛內容；render/generate 僅出現在 link 無法消費的場合（Claude Desktop MCP 合併、必要時的 marketplace manifest），且只產出最小必需檔案。

## Risks

- **與 in-flight 計畫衝突（高）**：`feat-plugin-arch-migration.md`（AGY/Copilot/Codex → core renderer）與 `plugin-bin-migration.md` 正在改動 `Build-ProviderPlugins.*`、`Install-GalPlugins.*`。本計畫若同時改 AGY 路徑會造成衝突，必須排序在後或明確協調。
- **設定檔覆蓋風險（高）**：注入 `claude_desktop_config.json` 時若未用安全 JSON 合併，可能清空使用者既有 MCP server，需冪等合併與備份。
- **能力誤宣（中）**：把 Claude Desktop 當成完整外掛 host 會導致無法兌現的承諾；MCP-only 是硬限制。
- **Claude Code link-first 未獲文件保證（高）**：官方文件未記載「裸 symlink 進 `~/.claude/plugins` 即自動載入」。若 spike 證實不成立，R-6 對 Claude Code 失效，Primary 必須改為 local marketplace（候選 B）。這是本計畫最大技術未知，必須以**第一個 gating spike 任務**消解，不可在未證實前往下做。
- **Claude Code 無 path-install（中）**：文件已確認無 `claude plugin install <path>`；若不採 marketplace 或 skills-dir，安裝只能是非持久的 `--plugin-dir` session load。持久化需求逼出 marketplace artifact 的額外建置工作（OQ-002）。
- **Desktop 注入壞 server／洩密（高）**：盲目注入需認證或 HTTP-remote 的 server 會在使用者機器產生連不上的紅燈，甚至寫入明文密鑰佔位符。Phase 1 必須限縮為免認證 stdio server，密鑰一律不落地（R-2、R-7）。
- **Desktop uninstall 誤刪使用者 server（中）**：MCP 設定無命名空間概念，若不記錄 GAL 寫入的 key ledger，uninstall 無法只移除自己加的條目（R-7）。
- **MCP schema 不相容（中）**：GAL `mcp.json`、Claude Desktop、Antigravity 三者 schema 不同（`servers` vs `mcpServers`、stdio `type` 處理、HTTP 用 `mcp-remote` vs `serverUrl`）。直接複製會壞；HTTP-type server 在 Claude Desktop 設定檔甚至不原生支援，可能須走 Connectors UI 而非檔案注入。
- **Desktop 設定路徑跨平台差異（中）**：Windows/macOS 的 `claude_desktop_config.json` 位置不同，偵測需平台分支；Windows 另有 `${APPDATA}` ENOENT 陷阱。

## Open Questions

- [x] OQ-001 — Claude Code 透過 `claude plugin` 生命週期已可安裝；Claude Desktop 則無公開 plugin install API，僅能透過 `claude_desktop_config.json` 或 `.mcpb` desktop extension 整合。*(raised by: planning, resolved by: codebase + capability review)*
- [x] OQ-005 — 文件確認 Claude Code **沒有** path-based `claude plugin install <path>`；`Invoke-ClaudePluginLifecycle` 停在 `session-load-only` 是 CLI 現況而非 bug。實際本機路徑仍須以 `-DryRun` 在 Step 1 確認，但「正確安裝」的瓶頸已從「修 bug」改為「選持久化策略」。*(raised by: planning, resolved by: Claude Code plugins doc)*
- [x] OQ-006 — Claude Desktop 與 GAL `mcp.json` 的 schema 不同（`servers`→`mcpServers`、stdio 去 `type`、HTTP 需 `mcp-remote`/Connectors），且 Antigravity 用 `serverUrl`。各目標需獨立的 MCP 轉換器，不可共用原樣複製。*(raised by: planning, resolved by: MCP + Antigravity docs)*
- [x] OQ-002 — 依 R-6 link-first：Claude Code 優先 **symlink 就地載入**，link 無法持久時才退而 **local/git marketplace**（指向同一正本，未來公開同 repo 即完成上架，不重做）；skills-dir 與 `--plugin-dir` 僅作開發/後備。Claude Desktop 因無可連結根目錄，自用採 **設定檔安全合併**，`.mcpb` 列入 Phase 2。*(raised by: planning, resolved by: user — 自用優先、link-first、終點為 marketplace 上架)*
- [x] OQ-004 — 本計畫 **先於** `feat-plugin-arch-migration.md` 啟動：先完成 Claude（P1）、再 Antigravity（P2），其餘 provider 由後續遷移計畫處理。*(raised by: planning, resolved by: user)*
- [x] OQ-003 — 定案：Claude Desktop **不**列為 plugin 意義上的正式 provider，而是 **MCP 投影層的特例目標**。理由由 R-6 + R-4 推得：它無外掛根目錄可 link、不 host agents/skills/commands、只能合併 MCP，因此不應走 `Build-ProviderPlugins`/plugin renderer/lifecycle。實務上若 orchestration 需要它可被選取，僅給一個 **MCP-only 的最小能力標記**（不開 agents/skills/commands 元件旗標），安裝路徑由 MCP 投影層擁有。*(raised by: planning, resolved by: derived from R-6 link-first + R-4 capability honesty)*

## Approval

- Human approval: [pending]
- Architect review: [clear — APPROVE AFTER REVISION; revisions applied 2026-05-30; T-001 診斷 spike 為實作先決]
- Additional domain review: [not triggered — no business/design surface]

## Review Results

### Architecture Review

#### Verdict: APPROVE AFTER REVISION（修訂已套用，clear 進入 `/refining-plan`）

計畫範圍校正紮實、決策已收斂、與既有架構（`~/.gal/plugins/gal` 正本、`Install-GalPlugins.ps1` 協調、catalog provider 模型）對齊良好，且誠實面對 Claude Desktop 的能力上限。以下為對抗式審查發現，已逐項回寫計畫。

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| 本計畫先於 `feat-plugin-arch-migration.md` | 先打通 Claude 自用安裝，符合作者優先序 | 兩計畫都碰 `Install-GalPlugins.ps1`，需嚴守邊界 | OK（見 BUG-01 邊界約束） |
| Claude Code link-first 為主、marketplace 為輔 | 零複製、單一正本 | link 自動載入未獲文件保證，可能整個前提不成立 | REVISE → 改為 spike 先決（已套用） |
| Claude Desktop 走設定檔安全合併 | 成本最低、即時可用 | 共享檔、schema 不同、易誤刪/洩密 | OK（限縮+ledger 後，見 BUG-02/03） |
| `.mcpb` 延到 Phase 2 | 避免 YAGNI | Phase 1 安裝體驗較陽春 | OK |
| Desktop 不入 `supportedProviders`（OQ-003） | 不污染 plugin renderer/lifecycle | runtime 選取層需另作 MCP-only 標記 | OK（已修正 Files 矛盾） |

#### Over-engineering Flags

- **OE-01** `.mcpb` 與 HTTP/Connectors 整合若放進 Phase 1 即過度建置。計畫已正確延後至 Phase 2 / opt-in，保持 YAGNI。維持現狀即可。
- **OE-02** 勿為 Claude Desktop 在 `catalog.json` 建完整 provider 抽象（會牽動 renderer/lifecycle）。OQ-003 已定為 MCP-only 特例目標；Files 段原本仍寫「成為正式目標」已修正。

#### Bug Surface

- **BUG-01** 高：與 `feat-plugin-arch-migration.md`（將 `Build-ClaudePlugin.*` 更名為 `Build-CorePlugin.*`、刪 `Build-AgyPlugin.*`、改 `Install-GalPlugins.*`）及 `plugin-bin-migration.md` 並行改 `Install-GalPlugins.ps1`。
  - 會壞於：本計畫若改動 Claude renderer 或 AGY 路徑，遷移計畫 rebase 時衝突。
  - 修正：本計畫**只**碰 `Invoke-ClaudePluginLifecycle`（Claude-specific）與**新增**的 Desktop MCP 注入路徑；AGY 維持 verify-only；不得更名/移動 renderer。（已於 R-3、Files `[VERIFY ONLY]`、Risks 記載；實作須嚴守。）
- **BUG-02** 高：Desktop 注入需認證/HTTP server（github、microsoftdocs、context7）會產生連不上的紅燈，且 `mcp.json` 內含 `${CONTEXT7_API_KEY}` 佔位符。
  - 會壞於：原樣複製到 `claude_desktop_config.json`，server 全紅或寫入明文佔位符。
  - 修正：Phase 1 限縮為免認證 stdio（chrome-devtools/firebase/markitdown/playwright）；密鑰不落地。（已套用 R-2、R-7、Step 2、Success Criteria。）
- **BUG-03** 中：`claude_desktop_config.json` 的 `mcpServers` 無命名空間；uninstall 無法分辨哪些是 GAL 加的。
  - 會壞於：移除時誤刪使用者既有 server，或殘留 GAL server。
  - 修正：將 GAL 寫入的 key 記到 `managed.json` ledger，uninstall 只刪 ledger 內的 key。（已套用 R-7。）
- **BUG-04** 低：server key 含斜線（`chromedevtools/chrome-devtools-mcp`、`upstash/context7`）作為 `mcpServers` key 不一致/不雅。
  - 修正：注入前正規化 key。（已套用 Step 2。）

#### Performance Concerns

無。本計畫產生靜態設定/連結與本地 CLI 生命週期檢查，無執行期熱路徑或無界資料載入。

#### Missing from Plan（修訂後）

- 測試未串接既有 `Test-InstallGalPlugins.ps1`：Desktop 安全合併與 Claude Code 路徑應有自動化斷言，而非僅手動。→ 交 `/refining-plan` 納入 `## Test Plan`／`## Tasks`。
- 任務順序未定：診斷 spike 必須是 T-001（gating），否則 link-first 是未證實前提。→ 交 `/refining-plan` 固定為第一任務。

#### Recommended Changes（均已回寫，除標註者外）

1. 把 Claude Code link-first 由「主路徑」降為「spike 待證的候選 A」，候選 B（local marketplace）並列；spike 出結論前不選定。（已套用）
2. Phase 1 Desktop 只注入免認證 stdio server；密鑰不落地；建立 key ledger。（已套用）
3. 修正 Files 段對 `catalog.json` 的描述以符合 OQ-003。（已套用）
4. `/refining-plan` 須將「診斷 spike」設為 T-001 gating，並把 Desktop 合併測試接進 `Test-InstallGalPlugins.ps1`。（交辦 `/refining-plan`）

#### What's Good（保留）

- 嚴守單一正本 `~/.gal/plugins/gal` 與既有協調腳本，拒絕 per-provider 旁路腳本。
- 對 Claude Code / Desktop / Antigravity 三者能力差異誠實分層（R-4）。
- 分期（自用 → 上架）讓自用 marketplace 與公開上架共用同一成品，不重做。
- 與 in-flight 計畫的衝突已被點名並劃定 verify-only 邊界。

<!-- ENG_REVIEW: 架構面 clear；實作前提條件為 T-001 診斷 spike 必須先證實 Claude Code 載入機制。Tasks/Test Plan 仍為 placeholder，下一步為 /refining-plan。 -->

### Business Review
未觸發。計畫不涉及定價、權限、引導或客戶可見商業規則（Phase 2 上架屬後續，非本計畫範圍）。

### Design Review
未觸發。計畫不涉及 UI 佈局、視覺、元件或無障礙流程。

### Engineering Review

#### Verdict: CLEAR

計畫已可建構：範圍校正完整、決策收斂、架構審查 clear、與既有腳本對齊、in-flight 衝突邊界明確。任務切分滿足「各自可獨立完成且可測」，並以 T-001 gating spike 消解最大未知（Claude Code 載入機制）。三目標的能力差異與 Desktop 注入安全（限縮清單、ledger、密鑰不落地）皆有對應任務與測試。

實作硬性約束（沿用架構審查，實作時必守）：

- **T-001 為 gating**：在 spike 對 Claude Code 載入機制有結論前，不得開 T-002 以後的 Claude Code 實作。
- **任務邊界**：本計畫只碰 `Invoke-ClaudePluginLifecycle` 與新增的 Desktop MCP 注入；AGY 為 verify-only；**不得**更名/移動 renderer（與 `feat-plugin-arch-migration.md` 的 `Build-CorePlugin` 更名衝突，BUG-01）。
- **Desktop 安全四要件**：限縮注入清單、key 正規化、`managed.json` ledger、安全合併+備份且密鑰不落地（R-2/R-7）。
- **跨平台**：PowerShell 與 Bash 行為一致；Bash 依賴不可用時，將該限制明記為阻礙而非靜默跳過。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | manual/spike | 實測：將 `~/.gal/plugins/gal` 以 symlink 投影到 Claude Code 探索位置後，未用 marketplace／`--plugin-dir` 是否自動載入；記錄結論並選定候選 A 或 B | T-001 |
| TP-002 | unit | `Install-GalPlugins.ps1` `-DryRun` 對 `claude` 顯示定案路徑（A：link 投影；B：marketplace manifest），且 `managed.json` 記錄正確、無殘留 `Build-AgyPlugin`/`Build-ClaudePlugin` 文字衝突 | T-002 |
| TP-003 | manual/integration | 依定案路徑安裝後啟動 Claude Code，可列出 GAL commands/agents/skills 並執行 `/gal status`；若走 B，另驗 `/plugin install`／`/plugin update`／`/plugin uninstall` | T-002, T-003 |
| TP-004 | unit | MCP 轉換器：`servers`→`mcpServers`、stdio 去 `type`、**只**輸出 Phase 1 四個免認證 server、key 正規化、**不輸出**密鑰佔位符（如 `${CONTEXT7_API_KEY}`） | T-004 |
| TP-005 | unit | 安全合併：對既有 `mcpServers` 注入後，使用者既有 server 全保留、GAL server 正確加入、JSON 有效、重複執行冪等、寫入前已備份；ledger 記錄 GAL key | T-005 |
| TP-006 | unit | Uninstall 只移除 ledger 內 GAL key，不動使用者既有 server | T-005 |
| TP-007 | manual | Windows `%APPDATA%\Claude\claude_desktop_config.json` 與 macOS `~/Library/Application Support/Claude/...` 路徑解析正確；完全重啟後 GAL server 可呼叫；必要時處理 `${APPDATA}` ENOENT | T-005 |
| TP-008 | manual/negative | Claude Desktop **未**出現 GAL commands/agents/skills（能力邊界，R-4） | T-004, T-005 |
| TP-009 | integration | `agy` dry-run：managed shortcut 仍指向 `~/.gal/plugins/gal`（link，非複製）；`agy inspect` 可用時確認 GAL MCP/Plugins/skills 已載入；確認本計畫未改 AGY renderer/腳本 | T-006 |
| TP-010 | integration | `scripts/Test-InstallGalPlugins.ps1` 納入 TP-002/004/005/006 斷言後全綠；Bash 不可用時記為環境阻礙 | T-008 |
| TP-011 | lint | 變更文件（plan、`docs/release-matrix.md`）通過 markdownlint，且無過度宣稱 Desktop 能力 | T-007 |

## Tasks

> 依優先序排列：T-001…T-003、T-008 為 P1 Claude Code；T-004…T-005、T-008 為 P1 Claude Desktop；T-006 為 P2 Antigravity；T-007 文件。T-001 為 gating，T-002 依賴其結論。

- [ ] T-001 — （gating spike）診斷本機 `claude` CLI 能力並**實測 symlink 自動載入**，輸出 Claude Code Primary 決策（候選 A link／候選 B marketplace），把結論回寫 Step 1。驗證 TP-001。
- [ ] T-002 — 依 T-001 決策在 `Invoke-ClaudePluginLifecycle`（`Install-GalPlugins.ps1` + `install-gal-plugins.sh` 同等）實作定案 Claude Code 安裝路徑（A：持久 link 投影；B：產出 `.claude-plugin/marketplace.json` 指向 `~/.gal/plugins/gal` + 串接 `/plugin` 生命週期），更新 `managed.json`。不更名/移動 renderer。驗證 TP-002。
- [ ] T-003 — 驗證 Claude Code 端到端載入：commands/agents/skills 可見、`/gal status` 可執行；若走 B，驗證 install/update/uninstall 生命週期。驗證 TP-003。
- [ ] T-004 — 實作 MCP schema 轉換器（純函式，置於 MCP 投影層）：`servers`→`mcpServers`、stdio 去 `type`、限縮為 `chrome-devtools`/`firebase`/`markitdown`/`playwright`、key 正規化、過濾密鑰佔位符。驗證 TP-004。
- [ ] T-005 — 實作 Claude Desktop 設定偵測（Win/macOS）、備份、冪等安全合併進 `mcpServers`、`managed.json` key ledger、對應 uninstall、Windows `${APPDATA}` 處理；接進 `Install-GalPlugins.ps1` 協調與 Bash 同等。驗證 TP-005、TP-006、TP-007、TP-008。
- [ ] T-006 — （verify-only）以 dry-run 確認 `agy` managed shortcut 仍是指向 `~/.gal/plugins/gal` 的 link；`agy inspect` 可用時確認載入；確認未改 AGY renderer/腳本。記錄 AGY MCP `serverUrl` 差異為觀察，不在本計畫改寫。驗證 TP-009。
- [ ] T-007 — 更新 `docs/release-matrix.md`：記錄 Claude Code／Claude Desktop（MCP-only）／Antigravity 的支援度與能力差異，遵守 R-4 能力誠實。驗證 TP-011。
- [ ] T-008 — 將 Desktop 轉換/合併與 Claude Code 路徑斷言接進 `scripts/Test-InstallGalPlugins.ps1`（涵蓋 TP-002/004/005/006），確保自動化而非僅手動。驗證 TP-010。
