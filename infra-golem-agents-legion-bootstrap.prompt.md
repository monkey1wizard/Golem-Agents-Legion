# Plan: Bootstrap Golem Agents Legion — Portable Development System

> **Status: ABSORBED (partial)** — Design decisions extracted to implementation files.
> **⚠️ Agent `golem-` rename incomplete**: filenames renamed via `git mv`, but frontmatter `name:` fields, cross-references (`@name`), file links in `agents.md`, `.gitattributes`, and README stale paths are NOT updated. See 「golem- Rename 審計」 under Phase 4.
> Delete this file after all rename edits are verified and committed.
>
> 此計畫撰寫於 2026-03-24，最後更新 2026-03-27。
> **此文件是 GAL 的 bootstrap plan**——從零拉起整個系統的設計決策與實作步驟。
> **終態**：當所有設計決策已萃取到對應檔案（`workflows/`、`agents.md`、`model-roles.md`、`conventions/` 等），此文件將標記為 ABSORBED 並刪除。

---

## 背景：為什麼需要 Golem Agents Legion

### 問題

現有的 24 個 Copilot Skills（`~/.copilot/skills/`）和 `copilot-instructions.md` 全部鎖死在
GitHub Copilot 生態系。一旦 AI agent 工具改版（例如切換到 Claude Code、Antigravity、OmO），
所有知識無法遷移。同時：

- **跨機器問題**：Windows PC 的 skills 不會自動同步到即將到的 Mac Mini
- **跨工具問題**：Copilot skills 格式無法被 Gemini CLI / Claude Code 等工具讀取
- **跨 session 問題**：每次開新的 AI 對話，之前做到哪裡的 context 全部消失
- **方法論碎片化**：開發流程（plan → implement → test → review）沒有統一定義

### 分析過的方案

| 方案 | 結論 | 為什麼不採用 |
| --- | --- | --- |
| Fork GSD (get-shit-done) | 不可持續 | 39 releases / 3 months，fork 後無法追上游 |
| 裝 OmO (oh-my-openagent) | 需要換編輯器 | 依賴 OpenCode TUI，且需要 Kimi/GLM subscription |
| 用 LangGraph 自建 agent | Over-engineering | 需要自己寫 Python agent，一人維護一個 framework |
| 裝 OpenClaw (full) | 只作概念參考 | docs 太亂，不打算安裝使用 |

### 從三個專案借的設計模式（不借實作）

> **注意**：GAL 完成後將完整取代 GSD。goal-backward verification、scientific debugging、plan-as-prompt、state tracking 等概念已內化為 GAL 原生設計，不再標示「借自 GSD」。

| 來源 | 借什麼 | 不借什麼 |
| --- | --- | --- |
| GSD | Phase-based workflow + STATE.md + verification | Task() API, meta-prompt, npm package |
| OmO | Category-based model routing（指定角色不指定 model） | OpenCode TUI, Sisyphus orchestrator |
| LangGraph | Stateful workflow = 狀態機 + 持久化 + checkpoint | Python SDK, graph DSL |

### 最終結論

> **方法論用 Markdown 寫，工具設定自動生成。**
> 你擁有的是 `golem-agents-legion` 裡的方法論，不是任何工具的 config。
> 工具來了就接上去，走了就拔掉。你的知識不動。

---

## 目標

建立一個 **GitHub private repo（`golem-agents-legion`）** 作為個人開發方法論的 canonical source：

1. 跨機器同步（Windows PC ↔ Mac Mini）透過 `git pull`
2. Skills 版本控管 + symlink 到 `~/.copilot/skills/` 及 `~/.gemini/`
3. **多 Workflow 架構**：依工作性質切換不同狀態機（coding flow, research flow 等），各自定義 states、tiers、agent activation
4. Model routing table（角色 → 工具的 mapping，換工具只改這張表）
5. 10 個 Golem agents（planner, architect, analyst, implementer, tester, reviewer, verifier, debugger, scribe, librarian）
6. **雙工具支援（Copilot + Gemini CLI）**：Setup-Machine 同時建立兩套 symlinks；Sync-DevContext 生成 per-repo adapter
7. Adapter 自動生成（從 `.dev/project.md` 生成 copilot-instructions.md / GEMINI.md）
8. Curfew system（22:00 soft + 23:00 hard）+ Scribe diary → Obsidian

---

## 三層架構

```text
┌─────────────────────────────────────────────────────────┐
│ Layer 1: ~/golem-agents-legion/  (clone of GitHub repo, 永久)         │
│  ├── workflows/           ← 多 Workflow 狀態機（coding, research 等）│
│  ├── model-roles.md       ← Category-based model routing │
│  ├── agent/               ← 10 個 .agent.md 定義檔      │
│  ├── conventions/         ← 語言慣例 + curfew 規則       │
│  ├── templates/           ← Phase prompt + diary 模板    │
│  ├── skills/              ← Skills canonical src          │
│  └── scripts/             ← Setup + Sync 腳本            │
├─────────────────────────────────────────────────────────┤
│ Layer 1.5: ~/.copilot/ + ~/.gemini/  (Setup-Machine symlinks) │
│  ├── ~/.copilot/agents/*.agent.md   ← symlink agent/     │
│  ├── ~/.copilot/skills/*/           ← symlink skills/    │
│  ├── ~/.gemini/skills/*/            ← symlink skills/    │
│  └── ~/.gemini/gal-context.md       ← 生成：@file imports│
├─────────────────────────────────────────────────────────┤
│ Layer 2: <repo>/  (Per-Repo Context + Knowledge, 隨 repo 走) │
│  ├── .dev/project.md     ← 可攜式 project context       │
│  ├── .dev/state.md       ← 全局狀態索引 + session continuity │
│  ├── docs/               ← Canonical 知識庫 (knowledge sink) │
│  └── docs/plans/         ← 暫存任務記憶 (完成後刪除)      │
├─────────────────────────────────────────────────────────┤
│ Layer 3: Auto-generated adapters (可拋棄, 自動重生)       │
│  ├── .github/copilot-instructions.md  ← for Copilot     │
│  └── GEMINI.md                        ← for Gemini CLI   │
└─────────────────────────────────────────────────────────┘
```

---

## 跨機器架構

```text
┌─────────────────────────────────┐   ┌──────────────────────────────────┐
│ Windows PC (3060 Ti)            │   │ Mac Mini (Apple Silicon)         │
│                                 │   │                                  │
│ 主要開發機                       │   │ 角色 1: iOS 開發 (Xcode)         │
│ ├─ VS Code + Copilot            │   │ 角色 2: LLM 推理 (Ollama AS)     │
│ ├─ Gemini CLI                   │   │                                  │
│ ├─ Ollama (3060 Ti, 12GB VRAM)  │   │                                  │
│ └─ C:\Code\                     │   │                                  │
│                                 │   │                                  │
│ ~/golem-agents-legion/ → clone               │   │ ~/golem-agents-legion/ → clone                │
│ ~/.copilot/skills/ → symlinks   │   │ ~/.copilot/skills/ → symlinks    │
│ ~/.gemini/skills/  → symlinks   │   │ ~/.gemini/skills/  → symlinks    │
└─────────────────────────────────┘   └──────────────────────────────────┘
                    │                                │
                    └──── git push/pull ─────────────┘
                         (via GitHub)
```

---

## Repo 結構

```text
golem-agents-legion/
├── README.md                          ← Repo 說明 + 架構圖 (唯一的 README)
├── ROADMAP.md                         ← (planned) 從 bootstrap 抽出的鏈程碑
├── model-roles.md                     ← Model routing table (8 roles)
├── infra-golem-agents-legion-bootstrap.prompt.md  ← 此文件（bootstrap plan，完成後刪除）
│
├── workflows/                         ← 多 Workflow 狀態機
│   ├── coding.md                      ← Coding flow（T0/T1/T2 + 9-state machine）
│   ├── research.md                    ← (planned) Research flow
│   └── ...                            ← (planned) 依需求擴充
│
├── agent/                             ← 10 個 Golem agent 定義
│   ├── agents.md                      ← Agent 總覽 + review pack + 三類分類
│   ├── golem-planner.agent.md         ← PLAN state
│   ├── golem-architect.agent.md       ← DISCUSS state (技術審查)
│   ├── golem-analyst.agent.md         ← DISCUSS state (商業審查，條件式啟用)
│   ├── golem-implementer.agent.md     ← IMPLEMENT state + Scope Fence
│   ├── golem-tester.agent.md          ← TEST state (spec-only)
│   ├── golem-reviewer.agent.md        ← CROSS_REVIEW state
│   ├── golem-verifier.agent.md        ← VERIFY state + plan lifecycle
│   ├── golem-debugger.agent.md        ← utility (科學方法 debug)
│   ├── golem-scribe.agent.md          ← utility (日記 + curfew)
│   └── golem-librarian.agent.md       ← domain (Obsidian vault writes)
│
├── conventions/                       ← 可攜式規則
│   ├── conventions.md                 ← 規則總覽
│   ├── curfew.md                      ← 22:00 soft / 23:00 hard curfew
│   ├── universal.md                   ← (planned) 跨語言通則
│   ├── csharp.md                      ← (planned)
│   └── ...                            ← (planned) go, typescript, rust
│
├── templates/                         ← Prompt + file templates
│   ├── templates.md                   ← 模板總覽
│   ├── plan.md                        ← Plan scaffold template
│   ├── state.md                       ← Global index + session continuity 模板
│   ├── project.md                     ← Per-repo context 模板
│   └── diary.md                       ← Scribe diary 模板
│
├── skills/                            ← (planned) Skills canonical src
│
└── scripts/                           ← Setup + Sync 腳本
    ├── scripts.md                     ← 腳本總覽
    ├── gal.ps1 / gal.sh               ← `gal <subcommand>` dispatcher
    └── Init-Repo.ps1 / init-repo.sh   ← Repo bootstrap (adopt-existing)
```

---

## 📊 Implementation Steps

### Phase 1: Bootstrap Repo ✅ DONE

- [x] 在 GitHub 建立 `dotdev` private repo（原名 golem-agents-legion）
- [x] 寫 `README.md`（repo 說明 + 三層架構圖 + 跨機器架構圖）
- [x] 寫 `workflow.md`（9-state 開發狀態機：IDLE→PLAN→DISCUSS→APPROVE→IMPLEMENT→TEST→CROSS_REVIEW→VERIFY→DONE）
- [x] 寫 `model-roles.md`（8 roles：PLANNER, ARCHITECT, ANALYST, CODER, TESTER, REVIEWER, SCRIBE, LOCAL）
- [x] Initial commit + push

### Phase 1.5: Golem Agents ✅ DONE

- [x] 建立 `agent/` 資料夾，寫 9 個 `.agent.md`
  - [x] `planner.agent.md` — goal-backward planning，輸出 `docs/plans/`
  - [x] `architect.agent.md` — 對抗性技術審查（6 維度），APPROVE/REVISE/REJECT
  - [x] `analyst.agent.md` — 商業邏輯審查（6 維度），APPROVE/REVISE/REJECT
  - [x] `implementer.agent.md` — atomic commits，plan 追蹤
  - [x] `tester.agent.md` — spec-only（禁止讀 impl），必須不同 model
  - [x] `reviewer.agent.md` — OWASP + 5 維度，severity BLOCKING/WARNING/INFO
  - [x] `verifier.agent.md` — 3-level goal-backward verification
  - [x] `debugger.agent.md` — scientific method，cognitive biases table
  - [x] `scribe.agent.md` — 日記 + curfew enforcement，Obsidian CLI 整合
- [x] 寫 `agent/agents.md`（agent 總覽、dual review 圖、curfew 說明）
- [x] 寫 `conventions/curfew.md`（22:00 soft + 23:00 hard curfew，override 機制）
- [x] 寫 `templates/diary.md`（Scribe diary 模板，frontmatter + 6 sections）
- [x] 寫 `templates/state.md` + `templates/project.md`（跨 session 模板）

### Phase 2: Skills Migration ✅ DONE

#### 現有 Skills 盤點（2026-03-25 掃描）

`~/.copilot/skills/` 共 81 個 skill 資料夾：

- **GSD skills：57 個**（全部單檔 SKILL.md，共 ~81 KB）→ **全部不遷移**，GAL 已提供替代方法論
- **非 GSD skills：24 個**（21 自訂 + 3 第三方，共 ~530 KB）→ 依下表決策

#### 非 GSD Skills 分類與遷移決策

| # | Skill | 用途 | 來源 | 大小 | 遷移決策 | 備註 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `blazor-development` | Blazor WebAssembly/Server best practices | 自訂 | 4.6 KB | **→ conventions/** | 併入語言 convention |
| 2 | `clean-architecture` | .NET Clean Architecture 分層規則 | 自訂 | 4.0 KB | **→ conventions/** | 併入 csharp convention |
| 3 | `csharp-development` | C#/.NET 10/C# 14 coding conventions | 自訂 | 8.5 KB | **→ conventions/** | 主要內容 → `conventions/csharp.md` |
| 4 | `defuddle` | 用 Defuddle CLI 抽取網頁為 Markdown | 自訂 | 1.0 KB | **→ skills/** | 工具 skill，直接複製 |
| 5 | `doc-coauthoring` | 3 階段文件共同撰寫 workflow | 自訂 | 15.4 KB | **→ skills/** | 獨立 workflow，不與 GAL 狀態機衝突 |
| 6 | `git-commit` | Conventional Commits 格式規範 | 自訂 | 4.6 KB | **→ conventions/** | 併入 `conventions/universal.md` |
| 7 | `go-development` | Idiomatic Go coding guidelines | 自訂 | 2.2 KB | **→ conventions/** | → `conventions/go.md` |
| 8 | `json-canvas` | 建立/編輯 Obsidian `.canvas` 檔案 | 自訂 | 14.2 KB | **→ skills/** | 含 references/，需整組複製 |
| 9 | `local-first-search` | DuckDB+BGE-M3 向量搜尋 Obsidian Vault | 自訂 | 9.6 KB | **→ skills/** | 工具 skill |
| 10 | `logging` | 跨語言 structured logging 規範 | 自訂 | 4.3 KB | **→ conventions/** | 併入 `conventions/universal.md` |
| 11 | `markdownlint` | Markdown 格式化/linting 規則 | 自訂 | 2.4 KB | **→ conventions/** | 併入 `conventions/universal.md` |
| 12 | `mcp-builder` | 建置 MCP server 指南 | 第三方 | 118.9 KB | **→ skills/** | 第三方但有用，含 reference/+scripts/ |
| 13 | `obsidian-bases` | Obsidian Bases (.base 檔案) YAML views | 自訂 | 20.6 KB | **→ skills/** | 含 references/ |
| 14 | `obsidian-cli` | 透過 CLI 操作 Obsidian vault | 自訂 | 4.6 KB | **→ skills/** | 工具 skill |
| 15 | `obsidian-knowledge-management` | PARA+Zettelkasten 知識管理 protocol | 自訂 | 13.0 KB | **→ skills/** | 獨立方法論 |
| 16 | `obsidian-markdown` | Obsidian Flavored Markdown 語法 | 自訂 | 8.7 KB | **→ skills/** | 含 references/ |
| 17 | `pdf` | Python PDF 處理（讀取、合併、OCR、表單） | 自訂 | 59.7 KB | **→ skills/** | 含 scripts/，需整組複製 |
| 18 | `plan-first-development` | Plan-before-code workflow | 自訂 | 4.9 KB | **不遷移** | 功能已由 planner golem 取代 |
| 19 | `result-pattern` | Result\<T\> pattern（跨語言） | 自訂 | 4.1 KB | **→ conventions/** | 併入 `conventions/universal.md` |
| 20 | `rust-development` | Rust coding conventions | 自訂 | 2.7 KB | **→ conventions/** | → `conventions/rust.md` |
| 21 | `skill-creator` | 建立/評估/benchmark skills 的 meta-skill | 第三方 | 219.7 KB | **→ skills/** | 第三方，含 agents/+scripts/+eval-viewer/ |
| 22 | `testing-strategy` | Test-first 開發策略 | 自訂 | 3.5 KB | **不遷移** | 功能已由 tester golem 取代 |
| 23 | `typescript-development` | TypeScript strict-mode 規範 | 自訂 | 2.3 KB | **→ conventions/** | → `conventions/typescript.md` |
| 24 | `webapp-testing` | Playwright web app 測試 | 第三方 | 21.9 KB | **→ skills/** | 第三方，含 examples/+scripts/ |

#### 遷移摘要

| 決策 | 數量 | 說明 |
| --- | --- | --- |
| → `skills/` | 12 個 | 工具/方法論 skills，直接複製到 `skills/` |
| → `conventions/` | 10 個 | 語言規範/通則，抽取併入 conventions 檔案 |
| 不遷移 | 2 個 | 功能已由 GAL golem 取代（plan-first-development, testing-strategy） |
| GSD 不遷移 | 57 個 | 全部由 GAL 替代 |

#### Phase 2 執行步驟

- [x] 複製 12 個 skills 到 `skills/`（defuddle, doc-coauthoring, json-canvas, local-first-search, mcp-builder, obsidian-bases, obsidian-cli, obsidian-knowledge-management, obsidian-markdown, pdf, skill-creator, webapp-testing）
- [x] 從 10 個 convention-type skills 抽取內容到 `conventions/`（Phase 3 合併執行）
- [x] 寫 `scripts/Setup-Machine.ps1`（建立 symlinks：skills/ → ~/.copilot/skills/ + agent/ → ~/.copilot/agents/）
- [x] 在 Windows PC 跑一次 Setup-Machine.ps1 驗證 symlinks 正常
- [x] 驗證 VS Code Copilot 仍能正確觸發 skills + agents

### Phase 3: Conventions + Templates ✅ DONE

- [x] 從 skills 抽取 `conventions/universal.md`（跨語言通則：git-commit + logging + markdownlint + result-pattern）
- [x] 從 skills 抽取 `conventions/csharp.md`（csharp-development + clean-architecture + blazor-development 合併）
- [x] 從 skills 抽取 `conventions/go.md`（go-development）
- [x] 從 skills 抽取 `conventions/typescript.md`（typescript-development）
- [x] 從 skills 抽取 `conventions/rust.md`（rust-development）
- [x] 建立 `conventions/token-budget.md`（Token 使用規則）
- [x] 建立 `templates/agent.md`（Golem 建立模板）

### Phase 4: Design Sync（bootstrap 設計決策 → 實作檔案）— ✅ ABSORBED

此階段將 bootstrap 中的設計決策萃取到各實作檔案。完成後 bootstrap 標記 ABSORBED。

#### 4A: Workflow 遷移

- [x] 建立 `workflows/` 目錄
- [x] 建立 `workflows/coding.md`：從 `workflow.md` 遷移 + 擴充（Tier T0/T1/T2 分級、9-state machine、Scope Fence、review pack、architect-lite/full、analyst 條件式啟用）
- [x] 刪除 `workflow.md`（被 `workflows/coding.md` 取代）

#### 4B: Agent 與 Model 更新

- [x] 更新 `agent/agents.md`：移除 dual review 預設，改為 review pack + direct-call 原則 + golem 三類分類
- [x] 更新 `agent/implementer.agent.md`：state tracking → plan `## Status` + Scope Fence 禁止清單
- [x] 更新 `agent/verifier.agent.md`：擴充 plan lifecycle ending（verify goal → confirm docs updated → ABSORBED → 刪除 plan）
- [x] 更新 `agent/tester.agent.md`：output 寫入 plan `## Test Results`
- [x] 更新 `agent/reviewer.agent.md`：output 寫入 plan `## Review Results`
- [x] 更新 `agent/debugger.agent.md`：debug state 歸屬（有 plan → plan Debug Log / 無 plan → state.md Debug Session）
- [x] 更新 `agent/scribe.agent.md`：frontmatter 補 `edit` tool
- [x] 更新 `model-roles.md`：補 tier 分級（Frontier/Standard/Low-cost）+ architect-lite/full + reviewer pack 規則

#### 4C: Template 更新

- [x] 更新 `templates/plan.md`：追加 Status / Review Results / Test Results / Debug Log / Handoff Notes + Success Criteria
- [x] 更新 `templates/state.md`：重新定義為 global index + session continuity，移除 per-task state
- [x] 更新 `templates/project.md`：補 Source Documents / Verified Facts / Suspected Drift / Documentation Gaps
- [x] 更新 `templates/templates.md`：移除 3 個不存在的 prompt template 引用
- [x] 對齊 `templates/diary.md` 與 `agent/scribe.agent.md` 格式

#### 4D: README + ROADMAP

- [x] 全面重寫 `README.md`：對齊多 Workflow 架構 + 三層架構 + 跨機器架構 + repo naming
- [x] 從 bootstrap 抽出 `ROADMAP.md`：Phase 1~5 概要 + 里程碑進度

#### 4E: Script 更新

- [x] 更新 `scripts/gal.ps1`：補 `pause` 命令 + 修正 plan filename 格式（`{type}-{slug}` 非 `plan-{slug}`）
- [x] 更新 `scripts/gal.sh`：同步 Windows 版更新
- [x] 更新 `scripts/Init-Repo.ps1`：補 adopt-existing 模式（先 ingest README + docs/ + 設定檔，再生成 .dev/project.md）
- [x] 更新 `scripts/init-repo.sh`：同步 Windows 版更新

##### `golem-` Rename 審計 ✅ DONE（2026-03-27）

**全部完成：**
- ✅ `git mv` 10 個 agent 檔案（`planner.agent.md` → `golem-planner.agent.md` 等）
- ✅ `~/.copilot/agents/` 10 個 symlinks 指向新檔名
- ✅ 10 個 agent frontmatter `name:` 更新（`planner` → `golem-planner` 等）
- ✅ `agents.md` 10 個檔案連結更新 + 3 個 `@invocation` 引用更新
- ✅ `golem-scribe.agent.md` 6× `@scribe` → `@golem-scribe`
- ✅ `golem-librarian.agent.md` 2× `@librarian` → `@golem-librarian`
- ✅ `golem-verifier.agent.md` 1× `@librarian` → `@golem-librarian`
- ✅ `conventions/curfew.md` 5× `@scribe` → `@golem-scribe`
- ✅ `templates/diary.md` 1× `@scribe` → `@golem-scribe`
- ✅ `.gitattributes` `agent/scribe.agent.md` → `agent/golem-scribe.agent.md`
- ✅ `README.md` `agent/scribe.agent.md` → `agent/golem-scribe.agent.md`
- ✅ `README.zh-Hant.md` `agent/scribe.agent.md` → `agent/golem-scribe.agent.md`

### Phase 4G: Gemini 支援 + 文件對齊

**Setup-Machine Gemini 支援：**
- [x] 擴充 `Setup-Machine.ps1`：新增 `~/.gemini/skills/` symlinks（與 Copilot 同一 source）
- [x] 擴充 `Setup-Machine.ps1`：生成 `~/.gemini/gal-context.md`（含所有 skills 的 `@file` imports）
- [x] 擴充 `Setup-Machine.ps1 -Uninstall`：同時移除 `~/.gemini/` 下的 GAL symlinks + `gal-context.md`
- [x] 同步 `setup-machine.sh`：macOS 版 Gemini 支援
- [x] 在 Windows PC 跑一次驗證 `~/.gemini/skills/` symlinks + `gal-context.md` 正常
- [ ] 驗證 Gemini CLI 可透過 `@~/.gemini/gal-context.md` 載入 GAL skills

**文件對齊（adapter scope 2→Copilot+Gemini, agent count 9→10）：**
- [x] `README.md`：移除 CLAUDE.md / AGENTS.md / .cursorrules adapter 引用（lines 19, 42）
- [x] `README.md`：symlink 描述加入 `~/.gemini/`（line 88）
- [x] `README.md`：`agent/scribe.agent.md` → `agent/golem-scribe.agent.md`（line 117）
- [x] `README.md`：「9 golem agent」→「10 golem agent」（line 131）
- [x] `README.md`：skills 描述加入 Gemini symlink（line 134）
- [x] `README.zh-Hant.md`：同上 5 項對應修正（lines 88, 116, 131, 134 + adapter scope）
- [x] `ROADMAP.md`：新增 Phase 4G 列、「9 agent」→「10 agent」、Phase 4F 狀態修正為實際值
- [x] `scripts/scripts.md`：symlinks 表格加入 Gemini targets + Sync-DevContext flow 從 5 adapters 縮為 2

### Phase 5: Per-Repo Integration（order-parser-app 為首例）

- [ ] 建 `order-parser-app/.dev/project.md`（先吸收既有 `README` / `docs/` / 設定檔，再壓縮成可攜摘要）
- [ ] 建 `order-parser-app/.dev/state.md`（記錄 feat-basic-mode 當前狀態）
- [ ] 寫 `scripts/Sync-DevContext.ps1`（從 .dev/project.md 生成 adapter 檔案）
- [ ] 驗證 `gal init/plan/status/next/pause` 在 Windows + macOS 行為一致
- [ ] 跑 Sync 驗證生成的 copilot-instructions.md / GEMINI.md 內容正確
- [ ] 用 feat-basic-mode 跑一次完整 workflow 驗證狀態機（含 plan 生命週期：建立 → 執行 → 萃取 docs → 刪除）

---

## 設計決策

### DISCUSS State：Review Pack（T2 專屬）

- **僅在 Tier 2（策略性任務）啟動**——T0/T1 跳過整個 DISCUSS 和 APPROVE 階段
- T2 不再固定綁定 `architect + analyst`；改為 **完整 plan + review pack**
- **Architect-full 為 T2 預設必啟**：負責 trade-off、架構衝突、依賴污染、過度設計、public API 風險
- **Analyst 為條件式啟用**：僅在任務涉及商業規則、pricing、permission/policy、onboarding、對客行為改變時加入
- 其他 pack 可依任務型態加入 reviewer / debugger，但不應把所有 golem 綁死進每個 T2
- 進 IMPLEMENT 的條件是：**該任務所需的 reviewers 全數 APPROVE**，不是固定要求 analyst 永遠參與
- 原因：T2 代表高風險或高複雜度，不代表一定有商業語意；把 analyst 綁死在所有 T2 會造成流程膨脹與低價值審查

### 為什麼有 10 個 Agent 而非更少

- 每個 agent 有明確的 **單一職責**，避免 prompt 過長降低品質
- 目前 10 個 agent 主要服務 Coding Flow；workflow states（PLAN, DISCUSS×2, IMPLEMENT, TEST, CROSS_REVIEW, VERIFY）+ utilities（debugger, scribe, librarian）
- **多 Workflow 架構下，部分 agent 可跨 workflow 共用**：architect 和 analyst 是 Domain 類，不鎖死在 Coding Flow
- **不用全部啟用**：T0 可跳過所有計畫 agents；T1 預設為 planner-lite + architect-lite + implementer + tester + reviewer(lite) + verifier；T2 才啟動 full planner + architect-full + 條件式 review pack
- 未來新增 workflow（如 research flow）可新增專屬 agent（如 researcher），不影響現有 coding flow 的 agent 組合

### Curfew 系統

- **目的**：強制 22:00 後停止工作，23:00 絕對停止
- **機制**：所有 agent 的 instruction 內含 curfew 檢查邏輯
- **Scribe 例外**：22:00-23:00 間只有 scribe 能運作（寫日記用）
- **Override**：用戶說 "override curfew" 可繞過一次
- **日記存放**：Obsidian vault `10_Projects/Work_Journal/`，月末歸檔到 `30_Archives/`

### 為什麼 skills 用 symlink 不用複製

- `skills/` 是 canonical source（有版本控管）
- `~/.copilot/skills/` 是 symlink target（Copilot 讀這裡）
- 改一處 push 一次，兩台機器 `git pull` 後自動一致
- 不用複製 = 不會出現兩份不同步的問題

### 為什麼不用 git submodule

- 只有一個 repo (golem-agents-legion)，per-repo 的 `.dev/` 直接放在各 project repo 裡
- golem-agents-legion repo 不需要 include 其他 repo
- 簡單就好

### 跨工具策略：Copilot + Gemini CLI

GAL 目前支援兩個 AI 工具。兩者能力不對稱，需要不同的接入策略：

#### 能力矩陣

| 能力 | Copilot | Gemini CLI |
| --- | --- | --- |
| Custom Agents | ✅ `.agent.md` 自動發現 | ❌ 無 agent 概念（單一模型） |
| Skills 自動發現 | ✅ `~/.copilot/skills/*/SKILL.md` | ❌ 無 skill 目錄掃描 |
| 全域 Context | ❌ 無全域 system prompt | ✅ `~/.gemini/GEMINI.md` global context |
| Per-Repo Context | `copilot-instructions.md` | Repo 根目錄 `GEMINI.md` |
| 檔案引用 | 無 @file 語法 | ✅ `@file.md` 將檔案內容 inline |
| Context 階層 | flat（全域 skills + per-repo instructions） | hierarchical（global → repo → JIT `@file`） |

#### Layer 1.5：Setup-Machine 雙工具 Symlinks

`Setup-Machine.ps1` 為兩個工具建立 symlinks：

| Source (repo) | Copilot Target | Gemini Target |
| --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/*.agent.md` | —（Gemini 無 agent） |
| `skills/*/` | `~/.copilot/skills/*/` | `~/.gemini/skills/*/` |
| —（生成） | — | `~/.gemini/gal-context.md` |

Gemini 無法自動發現 skills，但 symlink 後可透過 `@file` 引用：
- `~/.gemini/gal-context.md`（Setup-Machine 生成）包含 `@skills/defuddle/SKILL.md` 等 imports
- 使用者在 `~/.gemini/GEMINI.md` 加一行 `@~/.gemini/gal-context.md` 即可載入全部 GAL skills
- **不覆寫** `~/.gemini/GEMINI.md`——使用者可能有自己的全域設定（如 Obsidian vault 規則）

#### Layer 3：Per-Repo Adapter（Sync-DevContext）

| Adapter | 生成內容 | Skills 處理 |
| --- | --- | --- |
| `copilot-instructions.md` | conventions + workflows + project context | 不含 skills（Copilot 自動發現） |
| `GEMINI.md` | conventions + workflows + project context + active skills **inlined** | Skills 內容 inline（因 Gemini 不自動發現） |

Gemini 的 per-repo `GEMINI.md`（Layer 3）會覆蓋 global `~/.gemini/GEMINI.md` 的 skills，
因此 `gal-context.md`（Layer 1.5）只在未啟用 Sync-DevContext 的 repo 中生效。

#### 設計原則

- **Setup-Machine = 全域基礎設施**：確保任意 repo（即使沒跑 `gal init`）都能用 GAL skills
- **Sync-DevContext = per-repo 精準載入**：只 inline 該 repo 的 Active Skills，降低 token
- **不侵入使用者設定**：不碰 `~/.gemini/GEMINI.md`，只生成 `gal-context.md` 供手動引用
- **Uninstall 對稱**：`Setup-Machine -Uninstall` 同時移除 `~/.copilot/` 和 `~/.gemini/` 的 GAL symlinks

### 為什麼 adapter 是自動生成的

- `copilot-instructions.md` 的內容 = `.dev/project.md` + `conventions/` + `workflows/` 的組合
- 手動同步三份文件遲早會不一致
- 自動生成 = single source of truth 在 `.dev/project.md`
- 換工具時只需加一個新的 output format
- `project.md` 有 `Active Skills` 欄位 → 生成 GEMINI.md 時直接 inline 指定 skill 的內容，解決 Gemini CLI 無法自動讀取 skills 的跨工具問題

### Memory Architecture：文件分層，而非長對話記憶

GAL 的核心不是把 context 留在 chat thread，而是把記憶外部化到文件，分成五層：

1. **Knowledge Base**：`docs/`——repo 的 canonical 知識庫。包含 `README`、architecture notes、ADR、API spec、feature docs。這是所有永久知識的歸屬地。Plan 完成後的知識萃取回流到這裡。
2. **Working Memory**：`.dev/project.md`。只保存 repo 的壓縮摘要、索引、constraints、protected paths，不重寫整份架構文件。指向 `docs/` 中的原始文件。
3. **Global State**：`.dev/state.md`。追蹤 repo 層級的全局狀態：活躍 plan 索引、repo-wide blockers、上次 session 摘要、跨 plan 的 decisions。**不再承載 per-task 進度**——per-task state 由 plan file 自帶。
4. **Task Memory（暫時性）**：`docs/plans/*.prompt.md`。單次任務的 goal、tier、review pack、requirements、approach、tests、risks，**加上自帶的 `## Status` 區段**（workflow state、step、deviations、decisions）。Plan 是暫存工作文件：建立 → 執行 → 完成後知識萃取進 `docs/` → 刪除。
5. **Ephemeral Log**：`.scratch_YYYYMMDD.md` + diary。Scribe 的即時記錄與每日壓縮。

原則：
- **越穩定的知識，越接近 Knowledge Base；越短期的知識，越接近 plan/scratch。**
- **Plan 是過程，docs 是結論。** Plan 完成後必須萃取進 `docs/`，然後刪除。不要讓 `docs/plans/` 無限堆積。
- **不在每一層複製同一份資訊**——否則只製造 drift 和 token 浪費。

### 既有 Repo 優先：Adopt Existing Docs，而非 Blank Scaffold

- `gal init` 對既有 repo 的預設模式應是 **adopt-existing**，不是 blank scaffold
- 如果 repo 內已經有 `README`、`docs/`、ADR、architecture notes、設定檔，init 必須先 ingest 這些內容，再生成 `.dev/project.md`
- `.dev/project.md` 應該是**壓縮摘要 + 索引**，不是第二份 README 或第二份 architecture doc
- 既有文件與程式碼若不一致，應記錄為 drift / gap，而不是靜默覆蓋既有說明
- 僅對全新或幾乎無文件的 repo，才允許 `--blank` 類型的初始化模式

建議的 init 流程：

1. 掃描 `README`、`docs/`、ADR、主要設定檔
2. 用 code 結構抽樣驗證關鍵事實（tech stack、layering、test framework、entry points）
3. 生成 `.dev/project.md` 的壓縮摘要
4. 生成 `.dev/state.md`
5. 確保 `docs/plans/` 存在，但不重建既有 docs

### `.dev/project.md` 的角色：摘要索引，不是第二份架構文件

- `project.md` 應保存**高密度摘要**，而不是複製 canonical docs 內容
- 建議補入下列欄位：`Source Documents`、`Verified Facts`、`Suspected Drift`、`Documentation Gaps`
- Agent 啟動時先看 `project.md`，只有在該摘要指出需要時，才回頭讀原始文件
- 這樣做的目的是：降低冷啟動 token 成本，同時保留可追溯性

### 多 Workflow 架構：依工作性質切換狀態機

GAL 不使用單一狀態機，而是依工作性質提供多個獨立 workflow。各 workflow 自帶：
- 自己的 state machine
- 自己的 tier 分級（如果需要）
- 自己的 agent activation 規則

共用的東西（conventions、model-roles、curfew）留在上層，不重複定義。

**分割原則：依 guardrails 需求，而非表面性質**

| 工作 | 表面分類 | 真正分界線 |
| --- | --- | --- |
| 讀一篇論文，寫摘要 | research | 無 state machine，utility 直接做 |
| 研究 3 個方案，產出 ADR | research | 需要 review gate（architect verify 結論品質） |
| 修一個 typo | coding → T0 | 無 state machine |
| 重構整個 module | coding → T2 | 完整 state machine |

分界線不是「coding vs research」，而是**這項工作是否需要 verification gate**。

**目前已定義的 Workflow**：

| Workflow | 檔案 | 狀態 |
| --- | --- | --- |
| Coding Flow | `workflows/coding.md` | 從 `workflow.md` 遷移 + 擴充 |
| Research Flow | `workflows/research.md` | (planned) |

擴充新 workflow 時，只需在 `workflows/` 新增檔案，不變動既有 workflows。

### 以下設計決策皆屬於 Coding Flow（`workflows/coding.md`）

### Coding Flow 三級進入點

- **T0（瑣碎）**：修 typo、明顯 bug、單檔修改 → 無 plan，直接 IMPLEMENT → TEST（可選）→ VERIFY（可選）
- **T1（標準）**：小功能、已知 root cause 的 bug fix → 輕量 plan + architect-lite（預設開啟）→ IMPLEMENT → TEST → REVIEW(lite) → VERIFY
- **T2（策略）**：新功能、架構變更、高風險修改 → 完整 plan + DISCUSS(review pack) + APPROVE → IMPLEMENT → TEST → REVIEW → VERIFY

升級規則：任何 tier 在實作中發現超出預期的複雜度，可升級至 T2。

### Architect 預設策略：Lite / Full 兩段式

- 不採用「architect 永遠全開」：這會把 trivial 任務也拖入高成本審查，造成審查通膨與噪音
- **T0**：不自動啟用 architect，但允許人類主動 consult
- **T1**：預設啟用 architect-lite，只檢查結構風險：跨 layer、DI/interface/public API、protected paths、明顯 over-engineering
- **T2**：強制啟用 architect-full，執行完整 trade-off review
- 這樣做的原因是：architect 應高頻介入中高風險改動，但不應在低風險任務中稀釋警訊

### Analyst 啟用規則：條件式，而非 T2 固定成員

- Analyst 不應綁定在所有 T2；這是錯誤耦合
- **必啟情況**：商業規則、pricing/billing、permission/policy、notification behavior、onboarding/funnel、eligibility/approval、任何改變對客可見結果的邏輯
- **通常不需 analyst**：純技術重構、infra/build/CI、效能優化、dependency upgrade、商業語意不變的 bug fix
- 原則：T2 由風險與複雜度決定，Analyst 由任務語意決定

### 架構防護：Scope Fence（Coding Flow T0/T1）

T0/T1 缺乏完整 architect-full 審查，為防止 golem 在修小 bug 時意外動到架構，實施雙層防護：

**Layer 1 — Implementer Scope Fence（預防）**

`implementer.agent.md` 包含 T0/T1 禁止操作清單，觸發任何一項需停止並請人類升級至 T2：

- 建立/刪除專案檔（.csproj, .sln, package.json 等）
- 新增/移除套件依賴
- 跨架構層搬移檔案
- 建立新 interface 或抽象基底類別
- 修改 DI 註冊 / 服務組合
- 變更被多模組使用的 public API 簽章
- 引入新設計模式
- 修改被 3+ 消費者使用的共用/核心/基底類別

**Layer 2 — T1 強制 Reviewer（偵測）**

T1 的 reviewer 強制執行（非可選），但僅檢查 correctness + architecture 兩個維度，略過完整 OWASP 掃描。

### Command Surface 原則：Intent-first，非 State-first

- 公開命令應描述**使用者意圖**，不是直接暴露 workflow state
- 建議最小 command surface：`/gal init`、`/gal plan`、`/gal next`、`/gal status`、`/gal pause`、`/gal sync`
- **不建議**把 `/gal implement`、`/gal test`、`/gal review` 這類 state command 當 public API；這會導致非法狀態跳轉、state.md 不一致、繞過 guardrail
- `/gal plan` 是 **thin adapter**，背後仍必須先做 tier classification（T0/T1/T2），不能另起一套 command-driven workflow
- `init-repo` 應是 repo bootstrap，不是 GSD 式的重量級 project governance；只建立 `.dev/project.md`、`.dev/state.md`、`docs/plans/`、必要 adapter 與 repo metadata
- shell / script 層可提供 `gal <subcommand>` dispatcher；真正的 slash command（`/gal ...`）由各 AI adapter 映射到同一組子命令
- `gal init` 的預設語意應是 ingest 既有 repo docs；不是忽略現有文件後直接寫入模板
- `gal plan` 在建立計畫前，應優先讀 `.dev/project.md`、`.dev/state.md` 與任務相關的既有 docs，而不是只丟空白 plan scaffold

### Direct Agent Invocation 原則：允許 consult，不允許破壞狀態機

- 可公開的 direct-call 應以 consult / utility 為主，例如：`/gal ask architect`、`/gal ask analyst`、`/gal run debugger`、`/gal run scribe`
- workflow-bound golem 不應無條件直接改變狀態；正式 state transition 應透過 `/gal next` 或 workflow engine 決定
- 無 plan 時，可向 architect / analyst 諮詢，但不應把 consult 輸出當作正式 APPROVE/REVIEW 結論
- Utility golem（debugger, scribe）可相對自由；workflow golem（implementer, tester, reviewer, verifier）必須檢查前置條件

### Protected Paths 機制

各 repo 的 `.dev/project.md` 加入 `## Protected Paths` 區段，列出架構關鍵檔案/路徑。Implementer 碰觸這些路徑時自動觸發 T2 升級。此機制為 per-repo 可配置，適應不同專案的架構邊界。

### Golem 三類分類

| 分類 | 綁定狀態機 | 範例 |
| --- | --- | --- |
| Workflow | 是（綁定特定 state） | planner, implementer, tester, reviewer, verifier |
| Utility | 否（任何 tier 均可呼叫） | debugger, scribe |
| Domain | 否（綁定特定 workflow，不綁單一 state） | architect, analyst（跨 workflow 可用）；未來：researcher, UI/UX |

啟動原則（見 `agent/agents.md`）：最小可行集、context 預算 ~15%、各 golem 可獨立運作。

### Skills 目錄：`skills/`（不含 GSD）

原名 `copilot-skills/` 綁定特定工具品牌，違反工具無關性原則，更名為 `skills/`。Phase 2 遷移範圍縮減：僅遷移自訂 skills，57 個 GSD workflow skills 不再需要（GAL 已提供替代方法論）。

### Model Routing：Tier 分級

在 `model-roles.md` 新增模型 tier 分級：

- **Tier 1（Frontier）**：PLANNER, ARCHITECT, ANALYST（強推理需求）
- **Tier 2（Standard）**：CODER, TESTER, REVIEWER（執行需求）
- **Tier 3（Low-cost）**：SCRIBE, DEBUGGER（低成本）

規則：reviewer 的 model tier ≥ 被 review 的 implementer model tier。

### Skill-to-Golem 關聯：反向引用

每個 `agent.md` 列出自己的 `Required Skills`（正向清單）；不在 skill 上標記 golem ownership（避免多對多維護問題）。可選 `skill-matrix.md` 作為衍生視圖。

### Token 管理：Convention 而非 Golem

專責 token 控制 golem 技術不可行（無跨工具 token API），且違反「人類是 orchestrator」設計。替代方案：`conventions/token-budget.md` 寫入規則 + scribe 現有摘要功能 + 可選 `scripts/Token-Report.ps1`。

Token 控制應明確制度化：

- Agent 啟動優先讀 `project.md` / `state.md` / 當前 plan，而不是回放長 chat
- 只有在 `project.md` 指向特定 canonical docs 時，才讀 `README` / `docs/` 原文
- 長對話產生的新事實，應盡快摘要回 `.dev/state.md` 或 plan，避免後續 session 重複消耗 token
- `gal status` / `gal next` 的長期目標不是單純顯示字串，而是提供低 token 的狀態摘要與下一步推導

### Plan 生命週期：Transient Task Memory

Plan 是暫時性工作文件，不是永久記錄。`docs/plans/` 是暫存區，生命週期 = 一個 feature cycle。

**流程**：

1. Planner 建立 plan（`docs/plans/feat-xxx.prompt.md`）
2. Plan 自帶 `## Status` 區段，追蹤自己的 workflow state、step、deviations、decisions
3. Implementer 執行期間更新 plan 的 Status 區段（非 state.md）
4. 完成後，verifier 驗證 goal 達成 + **確認相關知識已萃取回 `docs/`**
5. Verifier 標記 plan status 為 `ABSORBED`（已萃取進 docs）
6. Plan 刪除——task memory 歸零，無 orphaned state

**為什麼不永久保存 plan**：

- `docs/plans/` 無限堆積會造成分不清完成與否的 noise
- Plan 的價值在執行期間；完成後有價值的知識應從 plan 萃取進 `docs/`（永久），plan 本身可棄
- `docs/` 是 knowledge sink，plan 是 knowledge pipeline

**關鍵安全規則**：Delete plan 是 VERIFY → DONE 的最後一步。任何中途刪除 = 丟失 spec，implementer 和 verifier 會 break。

**Plan 內建 Status 區段格式**：

```markdown
## Status

Workflow: IMPLEMENT
Step: 3 of 7
Last activity: 2026-03-26 — completed auth endpoint
Next step: Add order parsing logic

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |

### Handoff Notes

[Context from `gal pause` — key insights, unresolved questions, current hypothesis]
```

### Cross-Worktree 隔離問題

VS Code 把每個 git worktree 視為獨立 workspace：

```text
C:\Code\my-project\              ← main worktree (workspace A)
C:\Code\my-project-feat-basic\   ← feat-basic worktree (workspace B)
```

**不通的東西**（per-workspace 隔離）：
- Copilot chat history
- Copilot `/memories/session/`
- VS Code `.vscode/` settings（各 worktree 可有自己的）

**通的東西**（git tracked，但不同 branch 可能內容不同）：
- `.dev/state.md`、`docs/plans/`、`docs/`

**解法：Plan 自帶 State（方案 A）**

把 per-task state 從 `state.md` 下放到 plan file 本身（見上方 `## Status` 區段）。

- **優點**：State 跟著 plan 走，每個 branch 的 plan 自帶完整 task context。切 worktree 時 AI 讀 plan file 就能恢復。
- **優點**：Plan 刪除時 state 自動清除——no orphaned state。
- **優點**：Token 最優——冷啟動只需讀一個 plan file 就有完整 task context。
- **影響**：`state.md` 退化為全局索引 + session continuity，不再承載 per-task 進度。

**state.md 新角色**：

| 原職責 | 新職責 |
| --- | --- |
| 追蹤單一 active plan 的 step 進度 | 列出所有活躍 plans 的索引（哪些 branch 有活躍 plan） |
| Per-task deviations / decisions | 跨 plan 的 global decisions、repo-level blockers |
| Session continuity | Session continuity（保留：Last session / Stopped at / Next step） |

**不採用（branch-scoped state files）的原因**：

- `state-feat-basic.md`、`state-fix-parser.md` 等命名耦合 branch name，rename 就 stale
- Proliferation：10 branches = 10 state files
- 不如 plan 自帶 state 乾淨——plan deletion = state cleanup

### `gal pause`：Cross-Worktree Context Handoff

即使 plan 自帶 state，切 worktree 後 AI 仍不知道你在前一個 workspace 裡「討論了什麼」。Plan 記錄的是結論，不是推導過程。

**解法**：`gal pause` 命令，在切換 worktree 前主動觸發。

**行為**：
1. 要求當前 AI session 壓縮關鍵 context 進 plan file 的 `## Status > ### Handoff Notes`
2. 更新 `state.md` 的 `Session Continuity` 區段
3. Commit 變更到當前 branch

**設計原則**：
- 手動觸發，不自動化——AI 不知道你何時要切 worktree
- 解決 90% 情境：大多數時候 plan 的 Status + state.md 的 Session Continuity 就夠了
- Edge case（長 debug session 或複雜架構討論）才需要 Handoff Notes

### Review / Test Output 持久化

Review findings 和 test results 不能只活在 chat session 裡——切 session 後 verifier 無法驗證「blocking issues 已修復」。

**規則**：

| 輸出類型 | 持久化位置 | 格式 |
| --- | --- | --- |
| Reviewer findings | Plan file 的 `## Review Results` 區段 | severity + finding + resolution |
| Test results (摘要) | Plan file 的 `## Test Results` 區段 | pass/fail counts + failure details |
| Test results (完整) | CI/test runner 輸出（不需 GAL 管理） | — |

**為什麼放在 plan file 內而非獨立檔案**：

- Plan 是 task 的 single source of truth；review 和 test 是 task 的一部分
- Plan 刪除時 review results 自動清除——不留 orphan
- Verifier 只需讀一個 plan file 就有完整 task 生命週期

### Debug Session State

`debugger.agent.md` 聲明「maintain debug state in `.dev/state.md`」，但 debug session 不屬於正常 workflow state，需獨立區段。

**規則**：

- 有活躍 plan 的 bug → debug state 寫入 plan file 的 `## Debug Log` 區段
- 無 plan 的 ad-hoc bug → debug state 寫入 `state.md` 的可選 `## Debug Session` 區段
- Debug session 結束後清除相關區段（保留結論記錄於 Deviations 或 Decisions）

### Bootstrap 文件終態：ABSORBED 後退場

此文件（`infra-golem-agents-legion-bootstrap.prompt.md`）本身就是一份 GAL plan——用 GAL 的方法論建構 GAL 本身（自舉）。

當前它同時承擔四個角色：

| 角色 | 正常歸屬 | 萃取後終態 |
| --- | --- | --- |
| 設計決策 (ADR) | `workflows/`、`agents.md`、`model-roles.md`、`conventions/` | 分散到各檔案 |
| Roadmap | `ROADMAP.md` | 抽出為獨立檔案 |
| 系統架構 | `README.md` | 濃縮版放入 README |
| 暫存任務計畫 | 無（完成後刪除） | 刪除 |

**生命週期**：當所有設計決策已萃取到對應檔案，此文件標記為 ABSORBED 並刪除。這與 plan lifecycle 規則一致：plan 是過程，不是結論。

---

## Files Summary（2026-03-27 ground-truth audit）

> 此快照由磁碟實際狀態驗證產生，取代先前未經驗證的規劃快照。

### ✅ 已完成（on disk, Phase 1–4）

| 路徑 | 說明 |
| --- | --- |
| `README.md` | Phase 4D 全面重寫（多 workflow + 三層架構 + 跨機器架構） |
| `README.zh-Hant.md` | 繁體中文版 README |
| `ROADMAP.md` | Phase 4D 從 bootstrap 抽出的里程碑 |
| `model-roles.md` | Phase 4B 更新（8 roles + tier 分級 + architect-lite/full + reviewer pack） |
| `workflows/coding.md` | Phase 4A 從 `workflow.md` 遷移 + 擴充（Tier + 9-state + Scope Fence + review pack） |
| `workflows/research.md` | Research Flow 狀態機 |
| `agent/agents.md` | Phase 4B 更新（review pack / direct-call / 三類分類）— ⚠️ 10 條檔案連結指向舊檔名（broken） |
| `agent/golem-planner.agent.md` | Goal-backward planning — ⚠️ `name: planner`（未更新） |
| `agent/golem-architect.agent.md` | 對抗性技術審查 (6 維度) — ⚠️ `name: architect`（未更新） |
| `agent/golem-analyst.agent.md` | 商業邏輯審查 (6 維度) — ⚠️ `name: analyst`（未更新） |
| `agent/golem-implementer.agent.md` | Phase 4B + Scope Fence — ⚠️ `name: implementer`（未更新） |
| `agent/golem-tester.agent.md` | Phase 4B + plan Test Results — ⚠️ `name: tester`（未更新） |
| `agent/golem-reviewer.agent.md` | Phase 4B + plan Review Results — ⚠️ `name: reviewer`（未更新） |
| `agent/golem-verifier.agent.md` | Phase 4B + plan lifecycle ending — ⚠️ `name: verifier`（未更新）, 1× `@librarian` |
| `agent/golem-debugger.agent.md` | Phase 4B debug state — ⚠️ `name: debugger`（未更新） |
| `agent/golem-scribe.agent.md` | Phase 4B + edit tool — ⚠️ `name: scribe`（未更新）, 6× `@scribe` |
| `agent/golem-librarian.agent.md` | Obsidian vault writes — ⚠️ `name: librarian`（未更新）, 2× `@librarian` |
| `conventions/conventions.md` | Convention 總覽 |
| `conventions/curfew.md` | 三層 curfew 規則 — ⚠️ 5× `@scribe`（未更新） |
| `conventions/universal.md` | 跨語言通則（git-commit + logging + markdownlint + result-pattern） |
| `conventions/csharp.md` | C# conventions（csharp-dev + clean-arch + blazor 合併） |
| `conventions/go.md` | Go conventions |
| `conventions/typescript.md` | TypeScript conventions |
| `conventions/rust.md` | Rust conventions |
| `conventions/token-budget.md` | Token 使用規則 |
| `templates/plan.md` | Phase 4C（Status / Review Results / Test Results / Debug Log / Handoff Notes） |
| `templates/templates.md` | Phase 4C（移除不存在的 prompt template 引用） |
| `templates/state.md` | Phase 4C（global index + session continuity） |
| `templates/project.md` | Phase 4C（Source Documents / Verified Facts / Suspected Drift / Documentation Gaps） |
| `templates/diary.md` | Phase 4C 對齊 scribe 格式 — ⚠️ 1× `@scribe`（未更新） |
| `templates/agent.md` | Golem 建立模板 |
| `scripts/Setup-Machine.ps1` | Phase 4E + broken symlink bug fix（僅支援 Copilot） |
| `scripts/setup-machine.sh` | Phase 4E 同步 |
| `scripts/gal.ps1` | Phase 4E（pause 命令 + plan filename 格式） |
| `scripts/gal.sh` | Phase 4E 同步 |
| `scripts/Init-Repo.ps1` | Phase 4E（adopt-existing 模式） |
| `scripts/init-repo.sh` | Phase 4E 同步 |
| `scripts/gal-smudge.sh` | Smudge filter |
| `scripts/gal-clean.sh` | Clean filter |
| `skills/defuddle/` | Defuddle CLI 網頁抽取 |
| `skills/doc-coauthoring/` | 3 階段文件共同撰寫 workflow |
| `skills/json-canvas/` | Obsidian .canvas 建立/編輯（含 references/） |
| `skills/local-first-search/` | DuckDB+BGE-M3 向量搜尋 |
| `skills/mcp-builder/` | MCP server 建置指南（第三方，含 reference/+scripts/） |
| `skills/obsidian-bases/` | Obsidian Bases YAML views（含 references/） |
| `skills/obsidian-cli/` | CLI 操作 Obsidian vault |
| `skills/obsidian-knowledge-management/` | PARA+Zettelkasten 知識管理 |
| `skills/obsidian-markdown/` | Obsidian Flavored Markdown（含 references/） |
| `skills/pdf/` | Python PDF 處理（含 scripts/） |
| `skills/skill-creator/` | Skills meta-skill（第三方，含 agents/+scripts/） |
| `skills/webapp-testing/` | Playwright web app 測試（第三方，含 examples/+scripts/） |

### ⚠️ `golem-` Rename 未完成（43 edits across 19 files）

| 類別 | 數量 | 詳情 |
| --- | --- | --- |
| frontmatter `name:` | 10 files | 全部仍為 `name: planner` 等舊名 |
| `agents.md` 檔案連結 | 10 broken links | Lines 18-27 指向 `planner.agent.md`（已不存在） |
| `@invocation` 引用 | 20 occurrences / 6 files | 需從 `@scribe` → `@golem-scribe` 等 |
| `.gitattributes` | 1 stale ref | `agent/scribe.agent.md` → `agent/golem-scribe.agent.md` |
| `README.md` | 1 stale ref | Line 117: `agent/scribe.agent.md` |
| `README.zh-Hant.md` | 1 stale ref | Line 116: `agent/scribe.agent.md` |

### 🔲 Phase 5 待完成

| 路徑 | 說明 |
| --- | --- |
| `scripts/Sync-DevContext.ps1` | Adapter 生成器（從 .dev/project.md 生成 copilot-instructions.md / GEMINI.md） |
| `order-parser-app/.dev/project.md` | Per-repo context（首例） |
| `order-parser-app/.dev/state.md` | Cross-session state（首例） |

---

## 參考資料

- [如何让AI指挥Multi-agent做研究](https://youtu.be/ir-53vLLOEY?si=T8pj4K16_wbPkDcL) — 影片示例「多代理協作」，展示任務分解、狀態同步、研究流程管控手法，概念可直接映射到 workflow 的「Plan/State/Review」環節。
- [GSD (get-shit-done)](https://github.com/gsd-build/get-shit-done) — Phase workflow + STATE.md
- [OmO (oh-my-openagent)](https://github.com/code-yeongyu/oh-my-openagent) — Category model routing
- [LangGraph](https://github.com/langchain-ai/langgraph) — Stateful workflow concept
- [CrewAI](https://github.com/crewaiinc/crewai) — AI collaboration platform
- [Pydantic AI](https://github.com/pydantic/pydantic-ai) — AI integration with Pydantic
- [OpenClaw](https://github.com/openclaw/openclaw) — Concept reference only (sandbox, channel design); not adopted
