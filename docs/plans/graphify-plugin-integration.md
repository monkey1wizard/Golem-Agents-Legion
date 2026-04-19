# Plan: Graphify as Opt-in Detected Plugin

## Goal

讓 GAL 在偵測到目標 repo 已有 graphify 產出（`graphify-out/`）時，自動將 knowledge graph context 注入到 planning、architect review、staff review 等工作流中，提升結構感知的準確度，並以 `docs/mod/graphify.md` 明確定義 GAL 與 graphify 的 plugin contract。GAL 不主動安裝 graphify，也不將其列為必要依賴，偵測到就用，沒有就維持現有行為。

## Background

### 研究來源

- 評估報告：`docs/research/graphify-plugin-evaluation.md`
- 全 repo 實測數據：`.sandbox/graphify-full/`（2026-04-18）

### 驗證結果摘要

| 指標 | 數值 |
| --- | --- |
| 全 repo AST 提取 | 33 code files → 210 nodes, 274 edges, 28 communities |
| 提取耗時 | 0.3s（零 token 成本） |
| Token 節省倍率 | 平均 12.4x（最高 43.2x） |
| EXTRACTED 邊比例 | 95%（INFERRED 5%, avg confidence 0.8） |
| God nodes 準確度 | 正確辨識 MCP 連接、Skill 評估迴圈、Review 基礎設施 |
| Community 偵測品質 | 與人工模組劃分一致，singleton 正確標記獨立 script |
| INFERRED 邊品質 | 10/10 人工驗證合理，0 false positives |

### 設計原則

graphify 的定位與 gstack 相同：**可插拔的 specialist provider，不是核心依賴**。

1. GAL 不安裝、不 require graphify
2. 偵測信號：`graphify-out/GRAPH_REPORT.md` 存在於 repo root
3. `docs/mod/graphify.md` 是 GAL x graphify contract 的主要說明文件；README 與 devguide 最多只做導引，不複製完整規則
4. 有就讀、沒有就跳過，所有 workflow 修改都必須是條件式（`if exists`）
5. MCP server 在 `mcp-servers.example.json` 中維持 `enabled: false`，只作為 Phase B pilot，由使用者在 `mcp-servers.local.json` 自行啟用

## Requirements

- [x] R-01 — `docs/mod/graphify.md` 定義 GAL x graphify contract：定位、availability vs readiness、GAL 消費的檔案與輸出、非目標範圍，以及 upstream handoff
- [x] R-02 — `/deep-planning` Step 1 在 `graphify-out/GRAPH_REPORT.md` 存在時讀取它作為結構 context
- [x] R-03 — `golem-architect` 在 `<project_context>` 階段讀取 `GRAPH_REPORT.md`，用 god nodes 和 communities 輔助 trade-off 分析，且在 Level 1 僅將 INFERRED edges 視為 advisory signal
- [x] R-04 — `/review` Step 1 在 `GRAPH_REPORT.md` 存在時讀取它，交叉比對變更是否跨越 community 邊界
- [x] R-05 — `/planning` Step 1 在 `GRAPH_REPORT.md` 存在時讀取它，用 communities 判斷 scope 是否跨模組
- [x] R-06 — graphify 不存在時，所有修改過的 workflows 行為完全不變（zero regression）
- [x] R-07 — `mcp-servers.example.json` 可加入 graphify MCP server 設定，但必須維持 disabled by default，且只作為 Phase B pilot，不得成為 Phase A 的前置條件

## Approach

### Step 1: 建立 graphify 模組契約文件（Phase A）

- **Files**: `docs/mod/graphify.md`
- **What**: 新增一篇 GAL x graphify 模組文件，專門說明 graphify 在 GAL 架構中的位置、何時可用、GAL 如何偵測與使用它、哪些邊界不會改變，以及詳細安裝與完整 CLI 用法應回到 upstream GitHub。這篇文件作為主要 contract 文件，不把 `docs/devguide.md` 變成第二份規格。
- **Verify**: 文件存在，且至少包含定位、What This Module Covers、What Does Not Change、availability vs readiness、GAL 消費的檔案與輸出、整合層級、非目標範圍與 Read Next

### Step 2: 條件式報告讀取（Phase A）

- **Files**: `commands/deep-planning/SKILL.md`, `commands/deep-planning/SKILL.template.md`
- **What**: 在 Step 1 "Gather Inputs" 加入一行：`Read graphify-out/GRAPH_REPORT.md if it exists — use god nodes, communities, and surprising connections as structural context for the plan.`
- **Verify**: SKILL.md 包含條件讀取語句，且語意為 "if exists"

### Step 3: Architect agent context（Phase A）

- **Files**: `agent/golem-architect.agent.md`
- **What**: 在 `<project_context>` section 加入：`Read graphify-out/GRAPH_REPORT.md if it exists. Use god nodes to identify core abstractions, communities for module boundary awareness, and surprising connections for hidden coupling. Treat INFERRED edges as advisory unless live query support is enabled.`
- **Verify**: Agent file 包含 graphify context 指引

### Step 4: Staff review context（Phase A）

- **Files**: `commands/review/SKILL.md`, `commands/review/SKILL.template.md`
- **What**: 在 Step 1 "Read Changes" 加入條件讀取，指引 reviewer 交叉比對：新增的 import 是否建立非預期的 cross-community edge。
- **Verify**: SKILL.md 包含條件讀取語句

### Step 5: Planning context（Phase A）

- **Files**: `commands/planning/SKILL.md`
- **What**: 在 Step 1 "Gather Inputs" 加入條件讀取，用 communities 輔助 scope 判斷。
- **Verify**: SKILL.md 包含條件讀取語句

### Step 6: MCP server 設定（Phase B pilot，non-blocking）

- **Files**: `mcp-servers.example.json`
- **What**: 加入 graphify server entry，所有 provider 設為 `enabled: false`，僅作為本地 opt-in pilot。這一步不改變 Phase A 的完成定義，也不引入任何 graphify 安裝或 setup 自動化：

  ```json
  "graphify": {
    "description": "Knowledge graph query server for architecture-aware planning and review.",
    "providers": {
      "vscode": {
        "key": "graphify",
        "enabled": false,
        "config": {
          "command": "python",
          "args": ["-m", "graphify.serve", "graphify-out/graph.json"]
        }
      },
      "gemini": {
        "key": "graphify",
        "enabled": false,
        "config": {
          "command": "python",
          "args": ["-m", "graphify.serve", "graphify-out/graph.json"]
        }
      },
      "codex": {
        "key": "graphify",
        "enabled": false,
        "config": {
          "command": "python",
          "args": ["-m", "graphify.serve", "graphify-out/graph.json"]
        }
      }
    }
  }
  ```

- **Verify**: `mcp-servers.example.json` 語法正確，graphify entry 存在且全部 disabled，且文件仍明確表達這是 pilot 而非必要依賴

## Files to Create or Modify

- `docs/mod/graphify.md` — GAL x graphify 模組契約與使用邊界
- `commands/deep-planning/SKILL.md` — 加入條件式 GRAPH_REPORT.md 讀取
- `commands/deep-planning/SKILL.template.md` — 同步修改
- `agent/golem-architect.agent.md` — 加入 graphify context 到 `<project_context>`
- `commands/review/SKILL.md` — 加入條件式讀取
- `commands/review/SKILL.template.md` — 同步修改
- `commands/planning/SKILL.md` — 加入條件式讀取
- `mcp-servers.example.json` — 加入 graphify server entry（disabled，Phase B pilot）

## Test Cases

- [x] TC-01 — Preconditions: repo 含 `graphify-out/GRAPH_REPORT.md`，但未啟用 graphify MCP server。執行 `/deep-planning`，確認輸出引用報告中的 god nodes、communities 或 surprising connections
- [x] TC-02 — Preconditions: 同一個 repo 移除 `graphify-out/`。執行 `/deep-planning`，確認 workflow 不報錯且仍按既有非 graphify 路徑運作
- [x] TC-03 — Preconditions: repo 含 `graphify-out/GRAPH_REPORT.md`。執行 architect review 路徑，確認 graphify 被視為 advisory context，而不是 provider switch 或 hard dependency
- [x] TC-04 — Preconditions: repo 含 `graphify-out/graph.json`，並在 `mcp-servers.local.json` 啟用 graphify。驗證 `query_graph`、`god_nodes` 或 `shortest_path` 可作為 pilot follow-up 使用
- [x] TC-05 — 讀 `docs/mod/graphify.md`，確認它把安裝與完整 graphify 使用導回 upstream，而不是複製一份完整手冊

## Success Criteria

- [x] SC-01 — `docs/mod/graphify.md` 成為 GAL x graphify contract 的主要說明文件；README 與 devguide 不被擴寫成第二份規格
- [x] SC-02 — 所有 workflow 修改都是 `if exists` 條件式，graphify 不存在時零行為差異
- [x] SC-03 — GAL 不包含任何 `pip install graphify`、自動安裝邏輯，或把 graphify 假裝成必備工具的敘述
- [x] SC-04 — `mcp-servers.example.json` 中 graphify 若存在，則在 VS Code、Gemini、Codex 皆維持預設 disabled
- [x] SC-05 — Phase A 可獨立完成並交付；Phase B 仍是 optional pilot，不阻塞 Phase A 完成
- [x] SC-06 — 有 graphify 輸出檔案的 repo 中，planning、architect review、staff review 能引用結構證據（god nodes、communities、surprising connections）

## Risks

- **Graph staleness** — `GRAPH_REPORT.md` 可能落後於最新 commit。Mitigation：graphify 內建 `graphify hook install` 可設定 post-commit 自動 AST-only rebuild（免費）。GAL 不負責管理 graph freshness。
- **Availability 與 readiness 混淆** — machine 上有 graphify，不代表 repo 已經有 `graphify-out/` 輸出檔案。Mitigation：在 `docs/mod/graphify.md` 明確區分 provider availability 與 repo readiness，workflow 只依檔案 readiness 啟用。
- **文件重複與 drift** — 若把 graphify contract 同時寫進 devguide、README、mod 文件，之後很容易不一致。Mitigation：以 `docs/mod/graphify.md` 為唯一契約文件，其他文件只保留短導引。
- **Token budget** — `GRAPH_REPORT.md` 約 500–2000 tokens，在 GAL token-budget convention 可接受範圍內。MCP queries 有 `--budget N` 參數可控制。
- **Semantic extraction 成本** — AST-only 提取免費且 sub-second。Document semantic extraction 需要 LLM tokens，由使用者自行決定是否執行。GAL 只消費已產出的檔案與報告。

## Open Questions

- [x] OQ-001 — graphify 是否適合作為 GAL planning 和 architect review 的 context source？ *(raised by: research, resolved by: empirical test — 12.4x token reduction, 100% community accuracy)*
- [x] OQ-002 — Level 1 是否需要先加入 confidence threshold 之類的 live-query heuristics？ *(raised by: planning, resolved by: architecture-review)* — 不需要。Phase A 只消費 report，INFERRED edges 僅作 advisory signal；數值閾值留待 Phase B pilot 再評估
- [x] OQ-003 — graphify MCP pilot 應與 Phase A 同一個 PR 交付，還是作為後續 follow-up？ *(raised by: planning, resolved by: implementation)* — 可與 Phase A 同一個 PR 交付，只要 `mcp-servers.example.json` 維持 disabled by default，且不把本地啟用或 live-query 驗證變成完成 gate

## Approval

- Human approval: [pending]
- Architect review: [clear]
- Additional domain review: [not requested]

## Review Results

### Architecture Review

**Date:** 2026-04-19

**Verdict:** APPROVE

這份 plan 已修正前一輪 architect review 的主要問題：

- 文檔落點已收斂到 `docs/mod/graphify.md`，不再把 `docs/devguide.md` 擴寫成第二份規格
- 交付已拆成 **Phase A report-based integration** 與 **Phase B MCP pilot**，避免把低風險整合和跨 runtime pilot 綁成同一個完成定義
- Level 1 不再引入缺乏驗證依據的 confidence threshold，改為把 INFERRED edges 視為 advisory signal
- Test cases 已補上 report-present / report-absent / local MCP enablement 等前置條件，具備可驗收性

目前的 trade-off 是合理的：Phase A 提供最小可交付價值，Phase B 保留為 opt-in pilot，不會把 graphify 變成 GAL 的核心依賴。

### Business Review

Not applicable — internal tooling enhancement.

### Design Review

Not applicable — no UI changes.

### Engineering Review

Pending.

### Staff Review

**Date:** 2026-04-19
**Changes reviewed:** working tree vs main

#### Auto-Fixed (0 items)

None.

#### Flagged for Decision (0 items)

None.

#### Verdict

<!-- STAFF_REVIEW: CLEAR -->

已檢查本次變更是否把 graphify 升格為核心依賴，結果沒有。`docs/mod/graphify.md` 把 graphify 明確界定為 optional provider；workflow 變更全都以 `if exists` 為條件；`mcp-servers.example.json` 的 graphify providers 在 VS Code、Gemini、Codex 三者均維持 `enabled: false`。

已完成正向驗證：

- 在 repo root 執行 `graphify update .`，成功產生 `graphify-out/GRAPH_REPORT.md` 與 `graphify-out/graph.json`
- 以 black-box simulation 驗證 `/planning`、`/deep-planning`、`golem-architect` 在 `GRAPH_REPORT.md` 存在時，會引用 god nodes、communities、surprising connections 與 advisory-only 的 INFERRED edges
- 在 `mcp-servers.local.json` 以本地 opt-in 啟用 `graphify` 的 VS Code provider
- 以 live MCP client 對 `graphify.serve graphify-out/graph.json` 成功呼叫 `god_nodes`、`query_graph`、`shortest_path`

本次實測使用到的結構證據包括：

- god nodes：`MCPConnection`（11 edges）、`run_loop()`（9 edges）、`create_connection()`（7 edges）、`ReviewHandler`（7 edges）
- communities：Community 2（MCP connection handling）、Community 4（evaluation and tool calling）、Community 5（state management）
- surprising connections：`is_server_ready() -> create_connection()`、`main() -> create_connection()`、`run_loop() -> generate_html()`，皆作為 advisory signal 使用

因為 `/planning`、`/deep-planning`、`golem-architect`、`/review` 在 GAL 中是 prompt-driven workflow，不是可直接執行的腳本命令，本次 TC-01 與 TC-03 的驗證方式是 black-box simulation；TC-04 則以真實 MCP server + client 呼叫完成。

## Test Plan

Phase A:

- 在含 `graphify-out/GRAPH_REPORT.md` 的 repo 驗證 `/planning`、`/deep-planning`、architect review、`/review` 皆會條件式讀取報告
- 在不含 `graphify-out/` 的 repo 驗證上述 workflows 維持既有行為且不報錯
- 檢查 `docs/mod/graphify.md` 是否只描述 GAL x graphify contract，並明確把安裝與詳細用法導回 upstream

Phase B pilot:

- 在 `mcp-servers.local.json` 手動啟用 graphify 後驗證 `query_graph`、`god_nodes`、`shortest_path` 是否可作為 targeted follow-up
- 驗證 `mcp-servers.example.json` 仍為 disabled by default，不造成跨 runtime 預設行為改變

## Tasks

- T-001 — Create `docs/mod/graphify.md` as the main GAL x graphify module document
- T-002 — Add conditional GRAPH_REPORT.md read to `/deep-planning` SKILL.md + template
- T-003 — Add graphify context block to `golem-architect.agent.md` without hard-coded Level 1 confidence thresholds
- T-004 — Add conditional read to `/review` SKILL.md + template
- T-005 — Add conditional read to `/planning` SKILL.md
- T-006 — Add graphify MCP server entry to `mcp-servers.example.json` as a disabled-by-default Phase B pilot
- T-007 — Verify zero-regression by running planning/review flows on a repo without `graphify-out/`
- T-008 — Validate local MCP pilot only after explicit enablement in `mcp-servers.local.json`
