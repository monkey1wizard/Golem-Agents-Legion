# Golem Agents Legion (GAL)

[English](README.md) | 繁體中文

一套可攜式、不綁定工具的開發方法論系統。

> **方法論用 Markdown 寫，工具設定自動生成。**
> 方法論在 `golem-agents-legion/`，不在任何工具的設定裡。
> 工具來來去去，你的知識留下來。

## 為什麼

AI 程式助手變化很快。你的技能和工作流程不應該被鎖在任何單一工具裡
（Copilot、Claude Code、Cursor、Gemini CLI 等）。

GAL 把**你知道什麼**跟**哪個工具讀它**分開：

- **知識**（工作流程、慣例、代理、技能）→ 這個 repo 裡的 Markdown 檔案
- **工具設定**（`copilot-instructions.md`、`GEMINI.md`、`CLAUDE.md`）→ 自動生成的轉接器

換工具時，更新一張路由表、重跑同步腳本。你的知識不需要搬家。

## 三層架構

```text
┌─────────────────────────────────────────────────────────────┐
│ 第一層：~/golem-agents-legion/（這個 repo — 你的大腦）        │
│  ├── workflows/         ← 狀態機（coding、research）         │
│  ├── agent/             ← 9 個 Golem 代理定義                │
│  ├── model-roles.md     ← 模型路由 + 分級系統                │
│  ├── conventions/       ← 可攜式語言規範                     │
│  ├── templates/         ← plan、state、project 樣板          │
│  ├── skills/            ← 技能來源（symlink 到工具目錄）      │
│  └── scripts/           ← 設定、初始化、同步、調度腳本        │
├─────────────────────────────────────────────────────────────┤
│ 第二層：<repo>/.dev/（每個專案各自追蹤）                      │
│  ├── project.md         ← 專案摘要 + 索引                    │
│  └── state.md           ← 活動計畫 + 工作階段延續             │
├─────────────────────────────────────────────────────────────┤
│ 第三層：自動生成的轉接器（用完即棄）                           │
│  ├── .github/copilot-instructions.md                        │
│  ├── GEMINI.md                                              │
│  ├── CLAUDE.md                                              │
│  └── AGENTS.md                                              │
└─────────────────────────────────────────────────────────────┘
```

## 9 個 Golem 代理

| 代理 | 分類 | 用途 |
| --- | --- | --- |
| **planner** | 工作流程 | 分析需求、產出計畫檔案 |
| **architect** | 領域 | 對抗式審查 — 抓取捨、過度設計、bug |
| **analyst** | 領域 | 商業邏輯審查 — ROI、領域正確性 |
| **implementer** | 工作流程 | 執行計畫，原子式 commit + Scope Fence |
| **tester** | 工作流程 | 只從規格寫測試（不讀實作程式碼） |
| **reviewer** | 工作流程 | 交叉審查：bug、安全、架構 |
| **verifier** | 工作流程 | 目標回推驗證 + 計畫生命週期結束 |
| **debugger** | 工具 | 科學方法除錯 |
| **scribe** | 工具 | 日誌 + 宵禁執行者 |

## 開發流程分級

| 分級 | 適用情境 | 流程 |
| --- | --- | --- |
| **T0**（瑣碎） | 打字錯誤、明顯 bug、單檔修改 | IMPLEMENT → DONE |
| **T1**（標準） | 小功能、已知原因修正 | PLAN → IMPLEMENT → TEST → REVIEW(lite) → VERIFY |
| **T2**（策略） | 新功能、架構變動、高風險 | PLAN → DISCUSS → APPROVE → IMPLEMENT → TEST → REVIEW → VERIFY |

完整狀態機請見 [workflows/coding.md](workflows/coding.md)。

## 快速開始

### Windows

```powershell
git clone https://github.com/monkey1wizard/golem-agents-legion.git ~/golem-agents-legion
~/golem-agents-legion/scripts/Setup-Machine.ps1
```

### macOS

```bash
git clone https://github.com/monkey1wizard/golem-agents-legion.git ~/golem-agents-legion
~/golem-agents-legion/scripts/setup-machine.sh
```

設定腳本會建立 symlink：`agent/` → `~/.copilot/agents/`、`skills/` → `~/.copilot/skills/`。
多台機器透過 `git push/pull` 共享相同定義。

## 跨機器設定

GAL 支援多機器工作流程。每台機器 clone 同一個 repo 並執行 `Setup-Machine` 建立
symlink。模型和工具因機器而異；方法論完全相同。

```text
┌────────────────────────────┐    ┌────────────────────────────┐
│ 機器 A                     │    │ 機器 B                      │
│ ├─ AI 程式工具              │    │ ├─ AI 程式工具             │
│ ├─ Ollama（本地模型）       │    │ ├─ Ollama（本地模型）       │
│ └─ 主要開發機               │    │ └─ 次要 / 行動開發機        │
│                            │    │                            │
│ ~/golem-agents-legion/     │    │ ~/golem-agents-legion/     │
└──────────┬─────────────────┘    └──────────┬─────────────────┘
           └────── git push/pull ────────────┘
```

角色如何對應到你的機器和模型，請見 [model-roles.md](model-roles.md)。

## 個人化設定

多個檔案包含 `<PLACEHOLDER>` 佔位符，clone 後需填入你的路徑：

| 佔位符 | 意義 | 檔案 |
| --- | --- | --- |
| `<OBSIDIAN_VAULT>` | Obsidian vault 的絕對路徑 | `agent/golem-scribe.agent.md`、`skills/obsidian-cli/`、`skills/local-first-search/` |
| `<OBSIDIAN_VAULT_NAME>` | Obsidian 顯示的 vault 名稱 | `skills/obsidian-cli/` |
| `<LOCAL_SEARCH_PROJECT>` | `obsidian-note-taking-assistant` clone 路徑 | `skills/local-first-search/`、`skills/obsidian-knowledge-management/` |
| `<GAL_SKILLS>` | Skills 安裝路徑（例如 `~/.copilot/skills`） | `skills/pdf/` |
| `<TEMP_DIR>` | 暫存輸出目錄 | `skills/pdf/` |

模型對應設定請複製 [`model-roles.example.md`](model-roles.example.md) 為 `model-roles.local.md` 後自訂。

## 重要檔案

| 路徑 | 用途 |
| --- | --- |
| [workflows/coding.md](workflows/coding.md) | 開發流程狀態機（分級 + Scope Fence + 審查包） |
| [model-roles.md](model-roles.md) | 模型路由 + 分級系統 — 換工具時更新這裡 |
| [agent/](agent/agents.md) | 9 個 Golem 代理定義 |
| [conventions/](conventions/conventions.md) | 可攜式語言規範（通用、C#、Go、TS、Rust） |
| [templates/](templates/templates.md) | plan、state、project、diary、agent 樣板 |
| [skills/](skills/) | Copilot 技能（來源目錄，symlink 到 `~/.copilot/skills/`） |
| [scripts/](scripts/scripts.md) | 機器設定、repo 初始化、工作流程調度 |
| [ROADMAP.md](ROADMAP.md) | 實作進度與里程碑 |

## 設計原則

1. **知識寫在 Markdown，不寫在程式碼裡** — Markdown 不會有 breaking changes
2. **轉接器用完即棄** — `copilot-instructions.md`、`GEMINI.md` 是自動生成的；隨時刪掉重跑
3. **方法論 > 工具** — 工具可以換；你的工作流程留下來
4. **計畫是暫態記憶** — 計畫建立、執行、知識萃取到 `docs/`，然後刪除
5. **人是指揮者** — Golem 是專家；人決定分級、範圍、何時推進

## 影響來源

借鑑概念（未使用其實作）：

- [GSD](https://github.com/gsd-build/get-shit-done) — 階段式工作流程、狀態追蹤、驗證關卡
- [OmO](https://github.com/code-yeongyu/oh-my-openagent) — 以分類為基礎的模型路由（角色，非模型）
- [LangGraph](https://github.com/langchain-ai/langgraph) — 帶狀態的工作流程，支援持久化與檢查點

## 授權

MIT — 詳見 [LICENSE](LICENSE)。
