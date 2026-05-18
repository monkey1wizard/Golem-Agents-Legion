# 個人化設定 (Personalization)

這份文件保存了不適合放在 README 首頁的機器本機（machine-local）詳細資訊：佔位符（placeholders）、執行環境選擇（runtime selection）、模型路由（model routing）、MCP 覆寫、Obsidian 路由、工作時間設定（working-hours settings）以及何時該重新執行 setup。

## 執行環境選擇 (Runtime Selection)

機器安裝程式現在會將執行環境選擇保存在 `~/.gal/install-state.json` 中。

- `selectedRuntimes` 記錄了 GAL 應該管理哪些機器層級的目標。
- `primaryRuntime` 記錄了哪一個執行環境應該作為你的預設進入點。
- GAL 儲存庫仍然是 `agent/`、`skills/` 和 `commands/` 的唯一真相來源（source of truth）。主要執行環境（primary runtime）只會影響預設值和摘要，不會改變底層原始內容。

Antigravity 使用分離的介面：機器層級的技能與 MCP 設定位在 `~/.gemini/antigravity/` 下，而儲存庫本機的脈絡則來自於產生的 `.agents/rules/gal.md`，該檔案透過 Antigravity 文件的 `@filename` 規則語法來參考 `AGENTS.md`。`Update-Personalization` 讓此整合保持保守，不會修改使用者擁有的全域 Antigravity 規則檔案。

如果你想要更改所選的執行環境或主要的執行環境，請再次在 Windows 上執行 setup 並加上 `-Reconfigure` 參數，或在 macOS/Linux 加上 `--reconfigure`。

在 Windows 與 macOS/Linux 上，機器的設定介面現在已依據關注點（concern）拆分：

- `scripts/Setup-Machine.ps1` 執行完整流程
- `scripts/Update-Personalization.ps1` 更新 install-state、設定橋接、Antigravity 工作區規則策略、本機設定植入，以及 `gal-context.md`
- `scripts/Update-Skills.ps1` 更新 agents、skills、Antigravity 技能連結，以及 GAL 根連結
- `scripts/Update-Commands.ps1` 更新綁定的命令 skills、Antigravity 命令技能連結，以及原生 Gemini / Claude 命令檔案
- `scripts/Update-Mcp.ps1` 從追蹤的清單中更新 runtime MCP 設定，包含 Antigravity 的 `mcp_config.json`
- `scripts/setup-machine.sh` 執行完整流程
- `scripts/update-personalization.sh` 更新 install-state、設定橋接、Antigravity 工作區規則策略、本機設定植入，以及 `gal-context.md`
- `scripts/update-skills.sh` 更新 agents、skills、Antigravity 技能連結，以及 GAL 根連結
- `scripts/update-commands.sh` 更新綁定的命令 skills、Antigravity 命令技能連結，以及原生 Gemini / Claude 命令檔案
- `scripts/update-mcp.sh` 從追蹤的清單中更新 runtime MCP 設定，包含 Antigravity 的 `mcp_config.json`

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
| `<GAL_SKILLS>` | 技能安裝路徑 | 需要穩定本機路徑的輔助 skills |
| `<TEMP_DIR>` | 暫存輸出目錄 | PDF 與檔案處理工作流 |
| `<MCP_FILESYSTEM_PATHS>` | filesystem MCP server 允許的根路徑 | MCP 清單合併 |
| `<MCP_MEMORY_FILE_PATH>` | 存放持久化 MCP memory JSON 檔案的路徑 | MCP 清單合併 |
| `<CONTEXT7_API_KEY>` | 需要 Context7 API key 的執行環境設定 | MCP 清單合併 |

## 常見的個人化步驟 (Common Personalization Steps)

### 1. 模型路由 (Model routing)

- 複製 `../model-roles.example.md` 為 `../model-roles.local.md`。
- 只能在 `model-roles.local.md` 中更改提供者（provider）和模型對應。

### 2. 本機機密與路徑 (Local secrets and paths)

- 將機密、絕對路徑和機器專用的值放入 `../config.local.env`。
- 不要將本機的值寫入被 Git 追蹤的文件、命令範本或原始程式碼檔中。

### 2a. 命令技能的本機覆寫 (Command skill local overlays)

如果你想為特定的命令技能（command skill）加入要在 `Setup-Machine` 之後依然保留的機器本機自訂內容，請建立 `commands/<command>/SKILL.local.md`。

- `SKILL.local.md` 會被 Git 忽略，並被視為使用者擁有的機器本機輸入。
- `Setup-Machine` 會處理 `SKILL.template.md`，然後在重新生成 Gemini 和 Claude 命令檔案之前，將 `SKILL.local.md` 附加到生成的 `SKILL.md` 中。
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

預設情況下，工作時間限制是停用的。如果你希望 GAL 尊重你個人的工作日邊界，請在 `config.local.env` 中進行設定：

- `WORKING_HOURS_ENABLED=false` 保持所有工作時間邏輯關閉。
- `WORKDAY_START` 和 `WORKDAY_END` 描述你偏好的工作時段。
- `WRAP_UP_TIME` 會啟動提醒和關機視窗（shutdown-window）行為。
- `HARD_STOP_TIME` 定義 agents 拒絕進一步工作的時間點。

這些值是機器本機的偏好設定，而非被追蹤的儲存庫政策。

### 2d. xmachine 節點設定 (xmachine node config)

xmachine 節點定義是機器本機的，位於 `../xmachine.config.json`。

- 複製 `../xmachine.config.example.json` 為 `../xmachine.config.json`。
- 在頂層的 `nodes` 物件下定義每個工作節點。
- 將節點別名作為鍵值（key），並設定至少 `target` 和 `repoPath`。
- 當遠端 GAL runtime checkout 與目標 repo checkout 路徑不同時，加入 `runtimeRepoPath`。
- 當同一個工作節點承載多個目標 repo，且你希望 GAL 依目前本地 repo 名稱自動解析遠端路徑時，加入 `repoMappings`。
- 將 SSH 目標與儲存庫路徑保存在 `xmachine.config.json` 之中，而不是 `config.local.env`。

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

`scripts/Test-Xmachine.ps1` 會直接讀取 `xmachine.config.json`，所以編輯這個檔案不需重新執行 setup。

### 3. MCP 覆寫 (MCP overrides)

- 將追蹤的 GAL 原始設定保存在 `../mcp.json` 中。
- 將機器特有的 MCP 差異放入 `../mcp.local.json`。

專案特有或資料庫特有的 MCP server，通常應該放在 `../mcp.local.json`，而不是放進被追蹤的 `mcp.json`。對 Postgres 尤其如此，因為一台機器常常會同時處理多個 repo，而同一個 repo 也可能連到多個資料庫。

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

接著在 `config.local.env` 中加入對應變數，名稱可自行決定。`Update-Mcp.ps1` 與 `update-mcp.sh` 本來就會合併所有 local server 名稱，並從 `config.local.env` 解析任意 `${ENV_VAR}` placeholder。

### 4. 執行環境擁有的設定 (Runtime-owned config)

已安裝的執行環境設定（runtime configs）即使在 GAL 更新其管理的條目後，仍屬於使用者擁有。

| 執行環境 | 典型的 MCP 設定位置 |
| --- | --- |
| VS Code | 使用者的 `mcp.json` |
| Gemini CLI | `mcpServers` 下的 `settings.json` |
| Antigravity | `mcpServers` 下的 `~/.gemini/antigravity/mcp_config.json` |
| Codex CLI | `[mcp_servers.*]` 下的 `config.toml` |
| Claude Code | 透過 `claude mcp` 管理的 user-scope MCP 條目 |

GAL 現在將 `mcp.json` 加上 `mcp.local.json` 視為 MCP 的真相來源。重新執行 `Update-Mcp.ps1` 或 `update-mcp.sh` 時，只會覆寫受支援 runtime 內由 GAL 管理的 server 名稱，並保留其他由使用者定義的無關條目。

## 何時該重新執行 Setup (When To Rerun Setup)

當下列任何項目發生變更時，請重新執行 setup：

- `config.local.env`
- `mcp.json`
- `mcp.local.json`
- 任何 `commands/*/SKILL.local.md`
- `~/.gal/install-state.json`
- Obsidian 路由路徑或 Guide 模式
- 工作時間（working-hours）設定
- 模型路由或 runtime 安裝位置
- GAL 指令或 skill 安裝

`xmachine.config.json` 會由 xmachine 腳本直接讀取，不需要重新執行 setup。

如果只有單一關注點發生變更，請使用較狹窄範圍的腳本：

- 編輯 runtime 橋接器、`config.local.env` 或模型角色 local 檔案後，執行 `scripts/Update-Personalization.ps1`
- 更改 `agent/` 或 `skills/` 後，執行 `scripts/Update-Skills.ps1`
- 更改 `commands/*/SKILL.template.md` 或 `commands/*/SKILL.local.md` 後，執行 `scripts/Update-Commands.ps1`
- 更改 `mcp.json`、`mcp.local.json` 或 `config.local.env` 中與 MCP 相關的值後，執行 `scripts/Update-Mcp.ps1`
- 編輯 runtime 橋接器、`config.local.env` 或模型角色 local 檔案後，執行 `scripts/update-personalization.sh`
- 更改 `agent/` 或 `skills/` 後，執行 `scripts/update-skills.sh`
- 更改 `commands/*/SKILL.template.md` 或 `commands/*/SKILL.local.md` 後，執行 `scripts/update-commands.sh`
- 更改 `mcp.json`、`mcp.local.json` 或 `config.local.env` 中與 MCP 相關的值後，執行 `scripts/update-mcp.sh`

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

## 責任邊界 (Responsibility Boundary)

- 方法論與持久化契約保留在被追蹤的儲存庫檔案中。
- 機器本機（machine-local）的值保留在 `*.local.*` 檔案或 runtime 所屬的設定檔中。
- 協作工具的可用性（availability）屬於機器本機的設定與個人化範疇。協作工具的就緒性（readiness）則透過 [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) 屬於工作流 preflight 的範疇。
- 如果某個設定會造成跨機器的差異（drift），請先問問這是否應該放在被 Git 追蹤的原始碼檔案中，而不是本機的覆寫檔裡。

## 延伸閱讀 (Read Next)

- 主要的用戶進入點請見 [../README.md](../README.md)。
- 關於維護者面向的設定與 runtime 拓樸，請見 [devguide.md](devguide.md)。
- 腳本清單請見 [../scripts/scripts.md](../scripts/scripts.md)。
