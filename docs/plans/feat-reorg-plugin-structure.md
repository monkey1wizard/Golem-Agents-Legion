# Plan: 重構根目錄功能至單一扁平化外掛架構 (feat-reorg-plugin-structure)

## Goal

為解決專案根目錄過於散亂的問題，將所有「會發佈與打包的外掛內容」收攏至統一的 `plugins/gal-core/` 目錄中。
採用「單一扁平根目錄 + 複合進入點（Flat Root with Multiple Entrypoints）」策略，以 Claude Code 的外掛架構為基礎標準，同時向下相容 Antigravity、Copilot 與 Codex 的載入需求。

## Governing Principle: Flat Shared Root

不再讓外掛的內部功能模組散落在專案根目錄。所有發佈內容必須放置於單一外掛根目錄內，並透過各平台專屬的識別標記檔（Manifest）與 MCP 設定檔達成跨平台共用，徹底消除重複存放的問題。本機開發時可直接將 AI 工具的擴充路徑指向此目錄（Link-first）。

## Current State vs Target State

### Current State (散亂的根目錄)
- `agent/`
- `skills/`
- `commands/`
- `conventions/`
- `workflows/`
- `templates/`
...全部直接存在於專案根目錄。

### Target State (收攏後)
```text
Golem-Agents-Legion/
├── .dev/                        # GAL 專案本地開發與規劃狀態
├── .tmp/                        # 暫存與輸出快取 (整合 cache, gal-results, graphify-out, .sandbox)
├── Cargo.toml                  # Rust Workspace 設定檔
├── Cargo.lock
├── target/                     # Rust 共用編譯目錄
│
├── crates/                     # Rust 工作區
│   ├── gal-cli/                # 命令行工具
│   ├── gal-engine/             # 核心引擎 (由 gal-core 更名以避免衝突)
│   └── gal-dispatch/           # 任務分發器
│
├── plugins/                    # AI 外掛目錄
│   └── gal-core/               # 收攏後的 GAL AI 核心外掛 (Flat Shared Root)
│       ├── .claude-plugin/     # Claude Code 載入進入點
│       ├── .codex-plugin/      # Codex CLI 載入進入點
│       ├── plugin.json         # Antigravity 載入進入點
│       ├── opencode.json       # OpenCode 載入進入點 (由原根目錄移入)
│       ├── mcp.json            # MCP 外掛依賴定義 (單一來源，由原根目錄移入)
│       ├── agents/             # GAL AI 代理人
│       ├── skills/             # GAL AI 技能模組
│       ├── commands/           # 跨平台指令定義
│       ├── conventions/        # AI 規則與慣例
│       ├── workflows/          # AI 協同工作流程
│       ├── templates/          # AI 狀態模板與格式
│       └── hooks/              # Git hooks 腳本 (原 .githooks/ 與 hooks/ 合併)
│
├── docs/                       # 專案自身的開發計畫與研究文件
├── scripts/                    # 自動化、投影與建置腳本
│   └── packaging/              # 原根目錄的打包腳本
├── .graphify_detect.json       # 外部工具 (Graphify) 識別標記 (保留於根目錄)
├── README.md, LICENSE          # 基本專案說明
└── AGENTS.md, CLAUDE.md...     # 自動生成的各平台 Adapter
```

## Requirements

- [ ] **外掛目錄搬移**：將根目錄的 `agent/`（更名為 `agents/`）、`skills/`、`commands/`、`conventions/`、`workflows/`、`templates/`、`hooks/`（含 `.githooks`）全數移入 `plugins/gal-core/`。
- [ ] **進入點與 MCP 依賴封裝**：將散落於根目錄的 `opencode.json` 與 `mcp.json` 移入 `plugins/gal-core/`，確保核心外掛自給自足。
- [ ] **過時設定檔清理**：直接刪除根目錄已過時的本機設定範本（`config.example.env`, `xmachine.config.example.json`, `executor-routing.example.*`），依據文件這些設定現已全數改為 `~/.gal/config/config.json` 統管。
- [ ] **暫存與輸出目錄清理**：將 `cache/`、`gal-results/`、`graphify-out/` 與 `.sandbox/` 整併進 `.tmp/` 下，並更新 `.gitignore`。
- [ ] **Rust 模組重新命名**：將 Rust 工作區的 `crates/gal-core` 更名為 `crates/gal-engine`，避免與 AI 外掛整合包 `plugins/gal-core` 產生命名衝突。
- [ ] **多供應商進入點共存**：確保 `plugins/gal-core/` 內保留所有支援平台的載入所需 `plugin.json` 與 MCP 設定檔。
- [ ] **腳本重構**：更新 `scripts/` 下所有的打包、建置與投影腳本（例如 `Build-CorePlugin.ps1`），將硬編碼的路徑改為指向新目錄（包含移入 `scripts/` 的 `packaging/` 目錄）。
- [ ] **文件更新**：清理 `docs/` 下提及舊路徑的文件，確保文件與新架構對齊。

## Tasks (Detailed Execution Steps)

### 階段 1: 目錄結構重構與腳本更新 (Directory Restructure & Scripts)

#### Group 1A: 快取與輸出目錄搬移 (.tmp)
- [x] **T-01**: 確認 `xmachine.config.example.json` 存在於根目錄（準備後續移入範本區）。確認 `config.example.env` 已清理。
- [x] **T-02**: 將專案根目錄的 `cache/`, `gal-results/`, `graphify-out/`, `.sandbox/` 移動至 `.tmp/` 目錄內。(注意追蹤檔案需使用 `git mv` 搬移)。
- [x] **T-03**: 更新根目錄的 `.gitignore`，移除 `graphify-out/`, `gal-results/`, `cache/`, `.sandbox/` 的個別忽略，統一為 `.tmp/` 規則。
- [x] **T-04**: 修正腳本中的 `gal-results/` 路徑，導向 `.tmp/gal-results/`：
  - `Invoke-XmachineTask.ps1` (L581)
  - `Invoke-XmachineTask.sh` (L417, L512)
  - `Get-XmachineRemoteResult.ps1` (L25, L71)
  - `ProviderPlugin.ps1` (L537, L637-639) — provider leak-check 驗證字串
  - `provider-plugin.sh` (L509, L615-617) — 同上 Bash 版
  - `scripts/scripts.md` (L204) — 文件內路徑
- [x] **T-05 (TEST)**: 針對受影響的腳本進行基礎測試（如檢查 Xmachine 路徑解析是否正確），驗證 `.tmp/` 路徑變更沒有引發錯誤。
- [x] **T-06 (DOCS)**: 測試通過後，更新 `docs/` 與技能文件內關於 `gal-results/` 的參照：
  - `commands/gal-pipeline/SKILL.template.md` (L294, L389, L591)
  - `docs/` 下任何提及 `gal-results/` 的文件

#### Group 1B: 核心插件目錄建立 (plugins/gal-core)
- [x] **T-07**: 建立 `plugins/gal-core/` 目錄結構。
- [x] **T-08**: 搬移 `agent/` (改名為 `agents/`), `skills/`, `commands/`, `conventions/`, `workflows/`, `templates/`, `mcp.json`, `opencode.json` 至 `plugins/gal-core/`。同時將 `executor-routing.example.json` 與 `xmachine.config.example.json` 移入 `plugins/gal-core/templates/`。
- [x] **T-09**: 將 `.githooks/` 與 `hooks/` 合併移至 `plugins/gal-core/hooks/`，並執行 `git config core.hooksPath plugins/gal-core/hooks`。
- [x] **T-10**: 修正 `scripts/` 內各腳本路徑（按子項目逐一處理）：
  - **T-10a**: `Sync-DevContext.ps1` / `sync-dev-context.sh` — `conventions/`, `workflows/`, `skills/` 發現路徑改為 `plugins/gal-core/` 下。
  - **T-10b**: `ProviderPlugin.ps1` / `provider-plugin.sh` — `conventions/`, `workflows/`, `mcp.json` 來源路徑。
  - **T-10c**: `Init-Repo.ps1` / `init-repo.sh` — `templates/project.md`, `templates/state.md` 改為 `plugins/gal-core/templates/`。
  - **T-10d**: `Update-Personalization.ps1` / `update-personalization.sh` — 三項修正：(1) `executor-routing.example.json` 來源改為 `plugins/gal-core/templates/`；(2) `core.hooksPath` 從 `.githooks` 改為 `plugins/gal-core/hooks`；(3) 清理已廢棄的 `config.example.env` 複製邏輯。
  - **T-10e**: `Build-CorePlugin.ps1` / `build-core-plugin.sh` — `skills/`, `commands/`, `mcp.json` 來源路徑。
  - **T-10f**: `Get-StagedCommitMessage.ps1` / `.sh` — git staged path 檢查中的 `mcp.json`, `opencode.json`, `commands/`, `skills/` 改為新路徑。
  - **T-10g**: `New-TaskSpec.ps1` — `agent/golem-*.agent.md` 改為 `agents/golem-*.agent.md` 並更新為 `plugins/gal-core/agents/` 路徑。
  - **T-10h**: `Test-*.ps1` 測試腳本 — 更新 `agent/`, `skills/`, `commands/` 等的路徑。
  - **T-10i**: `common/Common.ps1` / `common.sh` — `McpSourceFile`, `commands/` 等 GAL source 路徑。
  - **T-10j**: `Update-Skills.ps1` / `update-skills.sh`, `update-commands.sh` — `skills/`, `commands/` 路徑。
  - **T-10k**: `Test-Xmachine.ps1` / `.sh` — `templates/task-xmachine-*.md` 路徑。
  - **T-10l**: `scripts/scripts.md` — 更新文件內所有舊路徑參照。
- [x] **T-10m**: 修正 `commands/` 與 `skills/` 內部的 Markdown 模板中 `agent/` → `agents/` 的路徑：
  - `commands/deep-planning/SKILL.template.md` (L71)
  - `commands/gal-pipeline/SKILL.template.md` (L612-616)
  - `skills/godot-scripting/SKILL.md` (L17, L42) — `conventions/csharp.md` 相對路徑
- [x] **T-11 (TEST)**: 執行 `Sync-DevContext.ps1` 與安裝腳本，驗證是否能順利讀取 `plugins/gal-core/` 且產生正確的 Adapter 檔，並能正確找到範本。
- [x] **T-12 (DOCS)**: 測試通過後，修正 `docs/` 與所有 MD 文件中關於舊目錄結構（如 `agent/`, `skills/`）的參照，並更新 `manual.md` 中 `xmachine.config.example.json` 的路徑。

#### Group 1C: 打包腳本搬移 (packaging)
- [x] **T-13**: 將 `packaging/` 移入 `scripts/` 中。
- [x] **T-14**: 確認 `packaging/` 目錄內腳本是否使用相對路徑自引用；若有外部引用則修正。（初步搜尋未發現 `scripts/` 內有直接 `packaging/` 路徑參照，可能為空操作。）
- [x] **T-15 (TEST)**: 執行打包腳本測試 (如 `Build-ProviderPlugins.ps1`) 確保搬移後能正常讀取。

### 階段 2: Rust 專案重構 (Rust Reorg)

- [x] **T-16**: 將 `crates/gal-core` 目錄更名為 `crates/gal-engine`。
- [x] **T-17**: 更新 `crates/gal-engine/Cargo.toml`：package name 改為 `gal-engine`，lib name 改為 `gal_engine`。
- [x] **T-18**: 更新根目錄 `Cargo.toml` 內的 members 參照改為 `crates/gal-engine`。
- [x] **T-19**: 更新 `crates/gal-cli/Cargo.toml` 依賴：`gal-core = { path = "../gal-core" }` 改為 `gal-engine = { path = "../gal-engine" }`。
- [x] **T-20**: 全面修改所有 `use gal_core::` 為 `use gal_engine::`，涵蓋：
  - `crates/gal-cli/src/main.rs` (12 處)
  - `crates/gal-engine/examples/fu02_render_smoke.rs` (L22-24, 3 處)
  - `crates/gal-engine/examples/verify_agy_surfaces.rs` (L8, 1 處)
  - `crates/gal-engine/tests/mcp_provider_oracle_parity.rs` (L10-13, 4 處)
- [x] **T-21 (TEST)**: 執行 `cargo build --workspace` 確保整體 Rust 專案編譯正常無誤。
- [x] **T-22 (DOCS)**: 測試通過後，搜尋並修正 `docs/` 內（如 `devguide.md` 等）關於 `crates/gal-core` 或 `gal-core` 專案名稱的參照，改為 `gal-engine`。

- **腳本路徑依賴斷裂**：`scripts/` 中的腳本高度依賴根目錄結構，搬移後若漏改路徑將導致 `gal init` 或打包失敗。
- **相對路徑連結失效**：各 Markdown 檔案內的相對連結（如 `../skills/`）在結構改變後可能變成壞結尾，需要全面檢查與替換。
