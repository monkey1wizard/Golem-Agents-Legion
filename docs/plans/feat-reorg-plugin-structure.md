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
- [x] **T-01**: 確認並清理殘留的過時設定檔（如 `config.example.env`）。你已手動刪除的 `xmachine.config.example.json` 需要復原，以便後續移入範本區。
- [ ] **T-02**: 將專案根目錄的 `cache/`, `gal-results/`, `graphify-out/`, `.sandbox/` 移動至 `.tmp/` 目錄內。(注意 `gal-results/` 內有部分追蹤檔案如 `20260507-MNFtiI/`，需使用 `git mv` 搬移)。
- [ ] **T-03**: 建立 `plugins/gal-core/` 目錄結構。
- [ ] **T-04**: 搬移 `agent/` 至 `plugins/gal-core/`，並重新命名為 `agents/`。
- [ ] **T-05**: 搬移 `skills/`, `commands/`, `conventions/`, `workflows/`, `templates/` 至 `plugins/gal-core/`。同時將 `executor-routing.example.json`, `executor-routing.example.ndjson` 以及 `xmachine.config.example.json` 移入 `plugins/gal-core/templates/` 以作為安裝時的預設範本。
- [ ] **T-06**: 將 `.githooks/` 與 `hooks/` 目錄合併移至 `plugins/gal-core/hooks/`。**關鍵操作**：必須同時執行 `git config core.hooksPath plugins/gal-core/hooks` 以確保本機 Git hooks 正常運作。
- [ ] **T-07**: 將 `mcp.json` 與 `opencode.json` 移至 `plugins/gal-core/`。
- [ ] **T-08**: 將 `packaging/` 移入 `scripts/` 中。
- [ ] **T-09**: 更新根目錄的 `.gitignore`，移除原本對 `cache/`, `gal-results/`, `graphify-out/`, `.sandbox/` 的忽略，並新增對 `.tmp/` 的忽略規則。
- [ ] **T-10**: 修正 `scripts/` 中的路徑：包含 `.tmp/gal-results/` 的修正，以及將各功能目錄 (`agent/`, `skills/`, `templates/`, `mcp.json` 等) 導向 `plugins/gal-core/`，並修正 `packaging/` 導向 `scripts/packaging/`。
- [ ] **T-11 (TEST)**: 執行建置與投影腳本 (如 `Sync-DevContext.ps1`)，驗證是否能順利讀取 `plugins/gal-core/` 並產生正確的 Adapter 檔。
- [ ] **T-12 (DOCS)**: 測試通過後，更新 `docs/` 與技能文件 (包含 `SKILL.template.md`) 內關於 `gal-results/`, `agent/`, `skills/` 等目錄參照。同時修正 `manual.md` 中 `xmachine.config.example.json` 的路徑，確保全部反映 `plugins/gal-core/` 與 `.tmp/` 的新結構。

### 階段 2: Rust 專案重構 (Rust Reorg)
- [ ] **T-13**: 將 `crates/gal-core` 目錄更名為 `crates/gal-engine`。
- [ ] **T-14**: 更新 `crates/gal-engine/Cargo.toml` 內的 package name 為 `gal-engine`。
- [ ] **T-15**: 更新根目錄 `Cargo.toml` 內的 members 參照，由 `crates/gal-core` 改為 `crates/gal-engine`。
- [ ] **T-16**: 更新 `crates/gal-cli/Cargo.toml` 依賴改為 `gal-engine`，並修改 `crates/gal-cli/src/main.rs` 內的 `use gal_core::...` 為 `use gal_engine::...`。
- [ ] **T-17 (TEST)**: 執行 `cargo build --workspace` 確保 Rust 專案更名後編譯正常。
- [ ] **T-18 (DOCS)**: 測試通過後，搜尋並修正 `docs/` 內（如 `devguide.md` 等）關於 `crates/gal-core` 或 `gal-core` 專案名稱的參照，改為 `gal-engine`。

- **腳本路徑依賴斷裂**：`scripts/` 中的腳本高度依賴根目錄結構，搬移後若漏改路徑將導致 `gal init` 或打包失敗。
- **相對路徑連結失效**：各 Markdown 檔案內的相對連結（如 `../skills/`）在結構改變後可能變成壞結尾，需要全面檢查與替換。
