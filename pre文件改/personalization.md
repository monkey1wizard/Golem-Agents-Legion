# 個人化設定

這份文件承接 README 首頁不適合展開的本機設定細節：環境占位符、模型路由、MCP 覆蓋與 rerun setup 的時機。

## 需要填入的占位符

| 佔位符 | 意義 | 常見用途 |
| --- | --- | --- |
| `<OBSIDIAN_VAULT>` | Obsidian vault 的絕對路徑 | Obsidian 代理與技能 |
| `<OBSIDIAN_VAULT_NAME>` | Obsidian 顯示名稱 | Obsidian 技能 |
| `<LOCAL_SEARCH_PROJECT>` | 本地搜尋專案 clone 路徑 | Local-first 與知識管理技能 |
| `<GAL_SKILLS>` | Skills 安裝路徑 | 部分輔助技能 |
| `<TEMP_DIR>` | 暫存輸出目錄 | PDF 技能 |
| `<MCP_FILESYSTEM_PATHS>` | filesystem MCP 可讀取的根目錄清單 | MCP manifest merge |
| `<MCP_MEMORY_FILE_PATH>` | 持久化 MCP memory JSON 檔路徑 | MCP manifest merge |
| `<CONTEXT7_API_KEY>` | 特定 runtime 需要的 Context7 API key | MCP manifest merge |
| `<OBSIDIAN_API_KEY>` | Obsidian Local REST API key | Obsidian MCP |
| `<OBSIDIAN_BASE_URL>` | Obsidian Local REST API base URL | Obsidian MCP |

## 最常見的個人化步驟

### 1. 模型路由

- 複製 `../model-roles.example.md` 為 `../model-roles.local.md`。
- 只在 `model-roles.local.md` 中調整你的 provider、角色與模型映射。

### 2. 本機密鑰與路徑

- 把本機 secrets、絕對路徑與環境相關值放到 `../config.local.env`。
- 不要把這些值直接寫進 repo-tracked docs 或 command templates。

### 3. MCP 覆蓋

- 如果你需要 provider-specific 或 machine-specific 的 MCP 差異，修改 `../mcp-servers.local.json`。
- tracked 的 baseline 保留在 `../mcp-servers.example.json`。

### 4. 重新執行 setup

出現以下情況時，應重新執行 setup script：

- 你改了 `config.local.env`
- 你改了 `mcp-servers.local.json`
- 你改了 model routing 或 runtime 安裝位置
- 你更新了 GAL command / skill 安裝

Windows:

```powershell
./scripts/Setup-Machine.ps1
```

macOS:

```bash
./scripts/setup-machine.sh
```

## 設定責任邊界

- canonical methodology 與 docs 應留在 repo tracked files。
- machine-local values 應留在 `*.local.*` 或本機 runtime config。
- 如果某個設定會讓不同機器分叉，先確認它是不是應該進 canonical docs，而不是直接進 local file。

## 相關文件

- [readme.zh-Hant.md](readme.zh-Hant.md) — 使用者入口
- [devguide.md](devguide.md) — 維護者導航
- [../docs/installation-topology.md](../docs/installation-topology.md) — runtime 安裝與 MCP 佈局
