# 個人化設定 (Personalization)

這份文件保存了不適合放在 README 首頁的機器本機（machine-local）詳細資訊：佔位符（placeholders）、執行環境選擇（runtime selection）、模型路由（model routing）、MCP 覆寫、Obsidian 路由、工作時間設定（working-hours settings）以及何時該重新執行 setup。

## 執行環境選擇 (Runtime Selection)

機器安裝程式現在會將執行環境選擇保存在 `~/.gal/install-state.json` 中。

- `selectedRuntimes` 記錄了 GAL 應該管理哪些機器層級的目標。
- `primaryRuntime` 記錄了哪一個執行環境應該作為你的預設進入點。
- GAL 儲存庫仍然是 `agent/`、`skills/` 和 `commands/` 的唯一真相來源（source of truth）。主要執行環境（primary runtime）只會影響預設值和摘要，不會改變底層原始內容。

Antigravity CLI（AGY）是 GAL 在 Google 系上的主要終端 runtime。GAL 以供應商外掛程式（provider plugin）的形式安裝至 AGY，安裝位置為 `~/.gemini/antigravity-cli/plugins/gal/`，此目錄承載 skills、agents、rules 與 MCP 設定，構成一個自包含的外掛程式樹。該外掛程式是由 `Build-AgyPlugin` 從供應商中立的共同套件模型（common package model）渲染而成的生成成品；AGY 是渲染器 1，而非架構本身。`Update-Personalization` 讓整合保持保守，不會修改使用者擁有的全域 Antigravity 規則檔案，也不會建立 repo-local 的 `.agents` 內容。

如果你想要更改所選的執行環境或主要的執行環境，請再次在 Windows 上執行 setup 並加上 `-Reconfigure` 參數，或在 macOS/Linux 加上 `--reconfigure`。

在 Windows 與 macOS/Linux 上，機器的設定介面現在已依據關注點（concern）拆分：

- `scripts/Setup-Machine.ps1` 執行完整流程
- `scripts/Update-Personalization.ps1` 更新 install-state、legacy Gemini 設定橋接與 `gal-context.md`、Antigravity 外掛程式整合（透過 `Build-AgyPlugin` 渲染 `rules/gal.md`），以及本機設定植入
- `scripts/Update-Skills.ps1` 更新 agents、Antigravity 外掛程式 skills（透過 `Build-AgyPlugin`）、其餘共享技能連結，以及 GAL 根連結
- `scripts/Update-Commands.ps1` 更新綁定的命令 skills、Antigravity 外掛程式命令 skills（透過 `Build-AgyPlugin`），以及 legacy Gemini / Claude 原生命令檔案
- `scripts/Update-Mcp.ps1` 從追蹤的清單中更新 runtime MCP 設定，包含 Antigravity 外掛程式根目錄的 `mcp_config.json`
- `scripts/setup-machine.sh` 執行完整流程
- `scripts/update-personalization.sh` 更新 install-state、legacy Gemini 設定橋接與 `gal-context.md`、Antigravity 外掛程式整合（透過 `Build-AgyPlugin` 渲染 `rules/gal.md`），以及本機設定植入
- `scripts/update-skills.sh` 更新 agents、Antigravity 外掛程式 skills（透過 `Build-AgyPlugin`）、其餘共享技能連結，以及 GAL 根連結
- `scripts/update-commands.sh` 更新綁定的命令 skills、Antigravity 外掛程式命令 skills（透過 `Build-AgyPlugin`），以及 legacy Gemini / Claude 原生命令檔案
- `scripts/update-mcp.sh` 從追蹤的清單中更新 runtime MCP 設定，包含 Antigravity 外掛程式根目錄的 `mcp_config.json`

## 安裝模式 vs 原始碼模式 (Install Mode vs Source Mode)

GAL 透過 `~/.gal/config/config.json` 控制 install mode 與 source mode 兩種運作模式。

- **Install mode**：給只想使用 GAL 的終端使用者。你不需要 clone repo。透過 `winget`（Windows）或 `homebrew`（macOS/Linux）安裝後，GAL 會管理自己的 `~/.gal/` runtime home。AGY 已支援完整的 provider-native lifecycle，不需要 source checkout。Claude 目前也支援 plugin artifact rendering、在本機 CLI 可用時的 strict validation、lifecycle-state tracking 與 session-load smoke，但 direct provider-native install 仍取決於實際 CLI capability，尚未是已驗證的預設路徑。

- **Source mode**：給 GAL 貢獻者。保留本機 GAL repo checkout，將 `~/.gal/config/config.json` 裡的 `galRoot` 指向該路徑，並啟用 `devMode`。這會保留 live local override、直接掛載 repo skills，以及不經過正式封裝就測試變更的能力。

無論哪一種模式，`~/.gal/plugins/gal/` 都是 canonical plugin root。像 `~/.claude/plugins/gal` 這類 provider-visible target，以及 `~/.gal/active/<provider>/` 這類 stable alias，都只是 projection 或 shortcut，不是內容擁有者。`~/.gal/dist/` 僅保留給 package output、managed metadata、conversion output，以及 dev mode 的 `~/.gal/dist/commits/` 隔離輸出；它不是 runtime source of truth。

### `~/.gal/` 資料夾結構

```text
~/.gal/
|-- active/
|   |-- agy/                    # 需要 capability shortcut 時的穩定 provider alias
|   `-- opencode/               # 啟用時使用的穩定 bridge-lane alias
|-- config/
|   |-- config.json             # 機器意圖：installMode、galRoot、plugins、runtime selection overrides
|   |-- config.local.env        # 機器本機 env 值與 secrets
|   |-- mcp.local.json          # 本機 MCP overrides
|   |-- model-roles.local.md    # 本機模型角色對應覆寫
|   `-- xmachine.json           # 機器本機 xmachine node 定義
|-- dist/
|   |-- commits/                # dev mode 下 GAL 變更的隔離輸出；永遠不是 runtime source of truth
|   |-- provider-plugins/
|   |   `-- agy/
|   |       `-- gal/            # AGY package output / conversion output
|   `-- providers/
|       `-- claude/
|           `-- managed.json    # Claude lifecycle metadata：canonicalRoot、projectionRoot、installTarget、packageOutputRoot
|-- generated/
|   |-- mcp/
|   |   `-- managed.json        # GAL-managed MCP projection state
|   `-- xmachine/
|       `-- managed.json        # GAL-managed xmachine projection state
|-- install-state.json          # selectedRuntimes 與 primaryRuntime
|-- plugins/
|   `-- gal/                    # canonical GAL plugin root
|       |-- .claude-plugin/
|       |   `-- plugin.json     # 位於 canonical root 之下的 Claude manifest
|       |-- .mcp.json           # portable GAL-managed Claude MCP config
|       |-- agents/
|       |-- commands/
|       `-- skills/
|-- source/                     # source mode repo-linked workflow 使用的 bridge roots
`-- state/
  `-- plugins.lock.json       # 解析後的 plugin catalog state
```

這張圖應該用 ownership 來讀，而不只是看路徑：`plugins/gal` 擁有 canonical runtime content，`dist/` 擁有可重建的 package 與 metadata output，`active/` 擁有 stable alias，而 `config/` 與 `state/` 則承載 machine intent。

GAL 支援兩種由 `~/.gal/config/config.json` 控制的操作模式：

- **安裝模式（Install mode）** — 給只想使用 GAL 的一般使用者。你不需要 clone 儲存庫。透過 `winget`（Windows）或 `homebrew`（macOS/Linux）安裝，GAL 自行管理 `~/.gal/` runtime home。所有 provider-native 外掛安裝、更新與解除安裝都不需要 source checkout。這是最終預設模式，但需 Claude、AGY 與 Copilot 三個 smoke guard 全部通過後才會正式切換。

- **原始碼模式（Source mode）** — 給 GAL 貢獻者。保留 GAL 儲存庫的本機 clone，在 `~/.gal/config/config.json` 中設定 `galRoot` 指向該路徑，並啟用 `devMode`。這讓你可以直接掛載本機 skills、測試變更，無需經過封裝。

切換模式：

- 在 `~/.gal/config/config.json` 中將 `installMode` 設為 `install` 或 `source`。
- 在 source mode 下，也需設定 `galRoot` 指向本機 GAL repo 路徑，並可選擇啟用 `devMode`。
- 切換後重新執行 `Setup-Machine`。

目前邊界說明：

- install mode 的資料模型與 `~/.gal/` ownership contract 已經生效。
- AGY 的 install-mode lifecycle slice 已完成並驗證。
- Copilot CLI、Codex 與 Claude Code 的 native lifecycle 仍屬後續 deferred follow-up。
- bootstrap 打包、`winget` / `homebrew`、release archives 與 marketplace discoverability 屬於獨立的 bootstrap-installer plan，不屬於這份 personalization 文件的範圍。

## 伴隨外掛與支援分層 (Companion Plugins and Support Tiers)

GAL 將 `gal-core` 保持精簡：控制平面、golem agents、核心工作流、必要慣例與少量 GAL 自有 skills。其餘皆為你可選擇加入的外部伴隨外掛。

**支援分層（Support tiers）** 說明內容由誰維護：

| 分層 | 維護者 | 自動更新 | 範例 |
| --- | --- | --- | --- |
| `official-gal` | GAL repo / release artifacts | 是，透過 GAL releases | `gal-core` |
| `curated-upstream` | 外部 upstream repo；GAL 鎖定版本 | 受控，依 lockfile pin | `dart-lang/skills` |
| `mirrored` | 外部 upstream 的受管 mirror | 不允許無版本複製 | 需要受管快取的 upstream |
| `forked` | fork 擁有者（GAL 或使用者） | 手動，需記錄 fork base | 已修補的 upstream skill fork |
| `local` | 你，僅限 source-mode override | 永不共享 | `file://` 本機路徑 |

**預設 profile**：初始 `default` profile 只安裝 `gal-core`。所有伴隨外掛均為 opt-in。透過 named profiles（例如 `dart`、`flutter`、`dotnet`）或 explicit plugin selection 在 `~/.gal/config/config.json` 中啟用。

**已知伴隨候選**（全部 `curated-upstream`，全部 opt-in）：

- `dart-lang/skills` — Dart
- `flutter/skills` — Flutter
- `dotnet/skills` — .NET / C#
- `anthropics/skills` — Claude 生態系
- `samber/cc-skills-golang` — Go
- `twostraws/swift-agent-skills` — Swift
- `kepano/obsidian-skills` — Obsidian
- `actionbook/rust-skills` — Rust

Game asset、Godot、GStack 框架類 skills 保留在 `gal-core`（GAL 自有，非外部伴隨），除非後續確認另有 upstream。

你的外掛選擇、profiles 與 resolver 輸出存放於：

- `~/.gal/config/config.json` — 你想安裝什麼
- `~/.gal/state/plugins.lock.json` — 實際解析並鎖定的版本

換機時請備份 `~/.gal/config/config.json` 與 `~/.gal/state/plugins.lock.json`。package-managed payload、provider plugin install tree 與 `~/.gal/generated/` 內容可於重新安裝後重建。

## 你可能需要填寫的佔位符 (Placeholders You May Need To Fill)

| 佔位符 | 意義 | 常見用途 |
| --- | --- | --- |
| `<OBSIDIAN_VAULT>` | Obsidian vault 的絕對路徑 | Obsidian agents 與 skills |
| `<OBSIDIAN_VAULT_NAME>` | vault 的顯示名稱 | Obsidian skills |
| `<OBSIDIAN_GUIDE_PATH>` | 相對於 vault 的個人 Obsidian 指南路徑 | notewriter 與 Obsidian 知識工作流 |
| `<OBSIDIAN_GUIDE_MODE>` | `auto`、`guide` 或 `generic` | Obsidian 寫入時的可選指南載入模式 |
| `<OBSIDIAN_PRIVATE_RESEARCH_DIR>` | 相對於 vault 的私人研究擷取目錄 | `/gal research` 私人筆記路由 |
| `<OBSIDIAN_DIARY_DIR>` | 相對於 vault 的工作日記目錄 | notewriter 日記模式 |
| `<OBSIDIAN_SCRATCH_DIR>` | 相對於 vault 的快速草稿紀錄目錄 | notewriter 日記模式 |
| `<OBSIDIAN_ARCHIVE_DIR>` | 相對於 vault 的日記封存目錄 | notewriter 日記模式 |
| `<RESEARCH_DEFAULT_DEST>` | 研究輸出的預設持久化目的地 | `repo`、`private`、`knowledge` 或 `none` |
| `<WORKING_HOURS_ENABLED>` | 本機是否啟用工作時間強制限制 | 選擇性啟用收工與強制停工限制 |
| `<WORKDAY_START>` | 偏好的工作日開始時間，格式為 `HH:MM` | 工作時間排程 |
| `<WORKDAY_END>` | 偏好的工作日結束時間，格式為 `HH:MM` | 下班時間 (After Hours) 邊界 |
| `<WRAP_UP_TIME>` | 收工時間 (Wrap-up Time)，格式為 `HH:MM` | 關機視窗 (shutdown-window) 行為 |
| `<HARD_STOP_TIME>` | 強制停工時間 (Hard Stop)，格式為 `HH:MM` | 停止工作行為 |
| `<LOCAL_SEARCH_PROJECT>` | 本機搜尋專案的 clone 路徑 | local-first 與知識管理 skills |
| `<GAL_ROOT>` | 本機 GAL repo clone 路徑（僅 source mode） | source mode 貢獻者工作流 |
| `<TEMP_DIR>` | 暫存輸出目錄 | PDF 與檔案處理工作流 |
| `<MCP_FILESYSTEM_PATHS>` | filesystem MCP server 允許的根路徑 | MCP 清單合併 |
| `<CONTEXT7_API_KEY>` | 需要 Context7 API key 的執行環境設定 | MCP 清單合併（materialize 至 `~/.gal/generated/mcp/managed.json`） |

## 常見的個人化步驟 (Common Personalization Steps)

### 1. 模型路由 (Model routing)

- 將 `model-roles.example.md` 複製到 `~/.gal/config/model-roles.local.md`。
- 只能在 `~/.gal/config/model-roles.local.md` 中更改提供者（provider）和模型對應。

### 2. 本機機密與路徑 (Local secrets and paths)

- 將機密、絕對路徑和機器專用的值放入 `~/.gal/config/config.local.env`。
- 不要將本機的值寫入被 Git 追蹤的文件、命令範本或原始程式碼檔中。

### 2a. 命令技能的本機覆寫 (Command skill local overlays)

如果你想為特定的命令技能（command skill）加入要在 `Setup-Machine` 之後依然保留的機器本機自訂內容，請建立 `commands/<command>/SKILL.local.md`。

- `SKILL.local.md` 會被 Git 忽略，並被視為使用者擁有的機器本機輸入。
- `Setup-Machine` 會處理 `SKILL.template.md`，然後在重新生成 legacy Gemini 與 Claude 命令檔案之前，將 `SKILL.local.md` 附加到生成的 `SKILL.md` 中。
- **請勿**直接編輯 `commands/<command>/SKILL.md`。它仍是產生的檔案，且會在下次執行 setup 時被覆蓋。
- `SKILL.local.md` 僅能包含額外的指示內容。請勿在裡面加入第二個 frontmatter 區塊。

### 2b. Obsidian 路由 (Obsidian routing)

Obsidian 支援是機器本機且可選的。GAL 將儲存庫擁有的狀態與使用者擁有的筆記分開：

- GAL 的 Obsidian 自動化現在使用內建的 `obsidian` CLI，而非舊的 Local REST API MCP 橋接器。
- 儲存庫擁有的研究預設會留在 `docs/research/`。
- 當設定了 `OBSIDIAN_VAULT` 時，私人擷取和可重複使用的知識會路由進你的 Obsidian vault。
- 如果你希望 GAL 遵循你自己的筆記庫規則，請設定 `OBSIDIAN_GUIDE_PATH`，並將 `OBSIDIAN_GUIDE_MODE` 設為 `auto` 或強制設為 `guide`。
- 如果你不維護個人指南，請將 `OBSIDIAN_GUIDE_PATH` 留空，或將 `OBSIDIAN_GUIDE_MODE` 設為 `generic`。

相對於 Vault 的路徑不應包含 vault 根目錄，也不應以斜線（`/`）結尾。

建議的預設值：

| 設定 | 典型值 |
| --- | --- |
| `OBSIDIAN_GUIDE_PATH` | `Guide.md` |
| `OBSIDIAN_PRIVATE_RESEARCH_DIR` | `/Projects/Research_Private` |
| `OBSIDIAN_DIARY_DIR` | `/Projects/Work_Journal` |
| `OBSIDIAN_SCRATCH_DIR` | `/Projects/Work_Journal` |
| `OBSIDIAN_ARCHIVE_DIR` | `/Archives/Work_Journal` |
| `RESEARCH_DEFAULT_DEST` | `repo` |

### 2c. 工作時間 (Working Hours)

預設情況下，工作時間限制是停用的。如果你希望 GAL 尊重你個人的工作日邊界，請在 `~/.gal/config/config.local.env` 中進行設定：

- `WORKING_HOURS_ENABLED=false` 保持所有工作時間邏輯關閉。
- `WORKDAY_START` 和 `WORKDAY_END` 描述你偏好的工作時段。
- `WRAP_UP_TIME` 會啟動提醒和關機視窗（shutdown-window）行為。
- `HARD_STOP_TIME` 定義 agents 拒絕進一步工作的時間點。

這些值是機器本機的偏好設定，而非被追蹤的儲存庫政策。

### 2d. xmachine 節點設定 (xmachine node config)

xmachine 節點定義是機器本機的，位於 `~/.gal/config/xmachine.json`。

- 將 `xmachine.config.example.json` 複製到 `~/.gal/config/xmachine.json`。
- 在頂層的 `nodes` 物件下定義每個工作節點。
- 將節點別名作為鍵值（key），並設定至少 `target` 和 `repoPath`。
- 當遠端 GAL runtime checkout 與目標 repo checkout 路徑不同時，加入 `runtimeRepoPath`。
- 當同一個工作節點承載多個目標 repo，且你希望 GAL 依目前本地 repo 名稱自動解析遠端路徑時，加入 `repoMappings`。
- 將 SSH 目標與儲存庫路徑保存在 `~/.gal/config/xmachine.json` 之中，而不是 `~/.gal/config/config.local.env`。

範例：

```json
{
  "nodes": {
    "mac-mini": {
      "target": "username@username-mac-mini.local",
      "repoPath": "/Users/username/Golem-Agents-Legion",
      "runtimeRepoPath": "/Users/username/Golem-Agents-Legion",
      "repoMappings": {
        "local-ai-tools": {
          "repoPath": "/Users/username/Code/zawip/local-ai-tools",
          "runtimeRepoPath": "/Users/username/Golem-Agents-Legion"
        }
      }
    }
  }
}
```

在這個範例裡，GAL 可以持續使用同一個 `mac-mini` 節點別名，同時把 `Golem-Agents-Legion` 與 `local-ai-tools` 路由到不同的遠端 checkout。

`scripts/Test-Xmachine.ps1` 會直接讀取 `~/.gal/config/xmachine.json`，repo-root 檔案只保留為有警告的遷移 fallback，因此編輯 canonical 檔案不需重新執行 setup。

### 3. MCP 覆寫 (MCP overrides)

- 將追蹤的 GAL 原始設定保存在 `../mcp.json` 中。
- 將機器特有的 MCP 差異放入 `~/.gal/config/mcp.local.json`。

專案特有或資料庫特有的 MCP server，通常應該放在 `~/.gal/config/mcp.local.json`，而不是放進被追蹤的 `mcp.json`。對 Postgres 尤其如此，因為一台機器常常會同時處理多個 repo，而同一個 repo 也可能連到多個資料庫。

對 Playwright MCP 也是同樣原則：被追蹤的 `mcp.json` 應只保留保守、與機器無關的預設；有頭模式、viewport 或 device 模擬、storage-state 路徑、輸出目錄、選用 capability flags、persistent profile 路徑、extension 或 CDP 連線等 local-only 瀏覽器行為，都應放在 `~/.gal/config/mcp.local.json`。

Playwright 的 local-only override 範例：

```json
{
  "servers": {
    "playwright": {
      "args": [
        "-y",
        "@playwright/mcp@latest",
        "--isolated",
        "--headless",
        "--storage-state",
        "${PLAYWRIGHT_MCP_STORAGE_STATE}",
        "--output-dir",
        "${PLAYWRIGHT_MCP_OUTPUT_DIR}"
      ]
    }
  }
}
```

這個 override 會取代 `playwright` 的整個 `args` 清單，因此你仍然需要把想保留的安全預設一併寫回去，例如 `--isolated` 與 `--headless`。`command` 與 `type` 仍會透過既有的 deep-merge 行為沿用被追蹤條目中的值。

這些 env var 應放在 `~/.gal/config/config.local.env`，而被引用的檔案或目錄應放在 repo 追蹤範圍之外。不要把 storage-state、persistent browser profile、browser output artifacts 或任何類似秘密的本地檔案提交進 repo。

同一台機器上有兩個 Postgres 資料庫時，可參考：

```json
{
  "servers": {
    "postgres-app": {
      "type": "stdio",
      "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": {
        "DATABASE_URI": "${POSTGRES_MCP_APP_URI}"
      }
    },
    "postgres-analytics": {
      "type": "stdio",
      "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": {
        "DATABASE_URI": "${POSTGRES_MCP_ANALYTICS_URI}"
      }
    }
  }
}
```

接著在 `~/.gal/config/config.local.env` 中加入對應變數，名稱可自行決定。`Update-Mcp.ps1` 與 `update-mcp.sh` 本來就會合併所有 local server 名稱，並從 `~/.gal/config/config.local.env` 解析任意 `${ENV_VAR}` placeholder。

如果你的 MCP host 支援 prompt-backed `inputs`，`~/.gal/config/mcp.local.json` 也可以放 top-level `inputs`。這適合像 GitHub remote MCP 這種要用 PAT、但不想把 token 寫回被追蹤檔的情境。例如你可以在 local override 中宣告 `servers.github` 與 `inputs.github_mcp_pat`；GAL 在同步時會保留這個 `inputs` 區塊，並把舊的 `github-mcp-server` 受管名稱清掉，避免同時出現 OAuth 與 PAT 兩個 GitHub 入口。

### 4. 執行環境擁有的設定 (Runtime-owned config)

已安裝的執行環境設定（runtime configs）即使在 GAL 更新其管理的條目後，仍屬於使用者擁有。

| 執行環境 | 典型的 MCP 設定位置 |
| --- | --- |
| VS Code | 使用者的 `mcp.json` |
| Antigravity CLI | `~/.gemini/antigravity-cli/plugins/gal/mcp_config.json`（外掛程式根目錄） |
| Codex CLI | `[mcp_servers.*]` 下的 `config.toml` |
| Claude Code | 透過 `claude mcp` 管理的 user-scope MCP 條目 |

GAL 現在將 `mcp.json` 加上 `~/.gal/config/mcp.local.json` 視為 MCP 的唯一來源。重新執行 `Update-Mcp.ps1` 或 `update-mcp.sh` 時，只會覆寫受支援 runtime 內由 GAL 管理的 server 名稱，並保留其他由使用者定義的無關條目。對 AGY 而言，GAL 管理的 MCP 會寫入外掛程式根目錄的 `mcp_config.json`（`~/.gemini/antigravity-cli/plugins/gal/mcp_config.json`）；全域的 `~/.gemini/antigravity-cli/mcp_config.json` 僅在清理舊的 GAL 管理條目時才會被觸及。即使 Gemini 的舊指令和內容相容層仍保留，且會移除先前寫進 `settings.json` 的 GAL 管理 Gemini MCP 條目。

## 何時該重新執行 Setup (When To Rerun Setup)

當下列任何項目發生變更時，請重新執行 setup：

- `~/.gal/config/config.local.env`
- `mcp.json`
- `~/.gal/config/mcp.local.json`
- 任何 `commands/*/SKILL.local.md`
- `~/.gal/install-state.json`
- Obsidian 路由路徑或 Guide 模式
- 工作時間（working-hours）設定
- 模型路由或 runtime 安裝位置
- GAL 指令或 skill 安裝

`~/.gal/config/xmachine.json` 會由 xmachine 腳本直接讀取，不需要重新執行 setup。

如果只有單一關注點發生變更，請使用較狹窄範圍的腳本：

- 編輯 runtime 橋接器、`~/.gal/config/config.local.env` 或 `~/.gal/config/model-roles.local.md` 後，執行 `scripts/Update-Personalization.ps1`
- 更改 `agent/` 或 `skills/` 後，執行 `scripts/Update-Skills.ps1`
- 更改 `commands/*/SKILL.template.md` 或 `commands/*/SKILL.local.md` 後，執行 `scripts/Update-Commands.ps1`
- 更改 `mcp.json`、`~/.gal/config/mcp.local.json` 或 `~/.gal/config/config.local.env` 中與 MCP 相關的值後，執行 `scripts/Update-Mcp.ps1`
- 編輯 runtime 橋接器、`~/.gal/config/config.local.env` 或 `~/.gal/config/model-roles.local.md` 後，執行 `scripts/update-personalization.sh`
- 更改 `agent/` 或 `skills/` 後，執行 `scripts/update-skills.sh`
- 更改 `commands/*/SKILL.template.md` 或 `commands/*/SKILL.local.md` 後，執行 `scripts/update-commands.sh`
- 更改 `mcp.json`、`~/.gal/config/mcp.local.json` 或 `~/.gal/config/config.local.env` 中與 MCP 相關的值後，執行 `scripts/update-mcp.sh`

如果你變更了會餵給 repo-local 產生 adapter 的 source-of-truth 內容，例如 `.github/copilot-instructions.md`、`AGENTS.md`、`CLAUDE.md`、`GEMINI.md` 的來源資料，請另外重新執行 `scripts/Sync-DevContext.ps1` 或 `scripts/sync-dev-context.sh`。`Update-Mcp` 不會重新產生這些 adapter 檔案。

Windows：

```powershell
./scripts/Setup-Machine.ps1
./scripts/Setup-Machine.ps1 -Reconfigure
./scripts/Update-Personalization.ps1
./scripts/Update-Skills.ps1
./scripts/Update-Commands.ps1
./scripts/Update-Mcp.ps1
```

macOS/Linux：

```bash
./scripts/setup-machine.sh
./scripts/setup-machine.sh --reconfigure
./scripts/update-personalization.sh
./scripts/update-skills.sh
./scripts/update-commands.sh
./scripts/update-mcp.sh
```

## 供應商外掛程式封裝 (Provider Plugin Packaging)

GAL 使用供應商中立的外掛程式套件模型（provider-neutral plugin package model）。儲存庫中的原始碼合約是唯一的事實來源；每個供應商外掛程式是由供應商專屬渲染器（renderer）渲染而成的生成成品。

### 共同基底 (Common Base)

共同套件模型承載 metadata、可重用技能、指令技能（作為技能套件）、規範 MCP 規格、指令語料庫，以及具備能力旗標的可選代理程式。它明確排除：

- 供應商專屬的輸出路徑
- 已解析的機器本機機密或路徑
- `runtimeScripts` 或外掛程式根目錄 `scripts/`
- `gal-results/`
- Hooks（自 v1 推遲）

### AGY 作為渲染器 1 (AGY as Renderer 1)

AGY 是第一個渲染器，而非架構本身。`Build-AgyPlugin` 會把 package output 產生在 `~/.gal/dist/provider-plugins/agy/gal/`，而 install orchestration 則維持 `~/.gal/plugins/gal/` 為 canonical plugin root，只有在需要 capability shortcut 時才使用 `~/.gal/active/agy/` 這個 stable alias。AGY 外掛程式承載：

- `plugin.json` — 含穩定 `name: gal` 的清單
- `skills/` — 可重用技能與指令技能
- `agents/` — 代理程式定義
- `rules/gal.md` — 合併的指令語料庫
- `mcp_config.json` — MCP 伺服器設定（外掛程式根目錄）

Setup/reinstall 會在安裝乾淨的外掛程式樹之前，移除所有既有的 GAL 管理 AGY 內容（舊版 skills 目錄、`GAL_ROOT` 符號連結、全域 MCP 條目、先前的外掛程式安裝）。

### Gemini 遷移路徑 (Gemini Migration Lane)

Gemini CLI 不是第五個渲染器。它是 AGY 的遷移/相容性路徑。既有的 Gemini 專屬清理與橋接邏輯保留在 AGY 渲染器的關注範圍內，不進入供應商中立的基底。

### 未來的渲染器順序 (Future Renderer Sequence)

在 AGY 驗證之後，計畫的渲染器順序為：Copilot CLI → Codex → Claude Code。每個渲染器將以自己的佈局與安裝生命週期重用共同基底。未來的渲染器不應複製 AGY 的佈局。

## 責任邊界 (Responsibility Boundary)

- 方法論與持久化契約保留在被追蹤的儲存庫檔案中。
- 機器本機（machine-local）的值保留在 `*.local.*` 檔案或 runtime 所屬的設定檔中。
- 協作工具的可用性（availability）屬於機器本機的設定與個人化範疇。協作工具的就緒性（readiness）則透過 [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) 屬於工作流 preflight 的範疇。
- 如果某個設定會造成跨機器的差異（drift），請先問問這是否應該放在被 Git 追蹤的原始碼檔案中，而不是本機的覆寫檔裡。

## 延伸閱讀 (Read Next)

- 主要的用戶進入點請見 [../README.md](../README.md)。
- 關於維護者面向的設定與 runtime 拓樸，請見 [devguide.md](devguide.md)。
- 腳本清單請見 [../scripts/scripts.md](../scripts/scripts.md)。
