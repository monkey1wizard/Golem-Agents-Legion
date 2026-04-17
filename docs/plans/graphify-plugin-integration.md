# Plan: Graphify as Opt-in Detected Plugin

## Goal

讓 GAL 在偵測到目標 repo 已有 graphify 產出（`graphify-out/`）時，自動將 knowledge graph context 注入到 planning、architect review、staff review 等工作流中，提升結構感知的準確度。GAL 不主動安裝 graphify，也不將其列為必要依賴——偵測到就用，沒有就維持現有行為。

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
3. 有就讀、沒有就跳過——所有 SKILL.md 修改都是條件式（`if exists`）
4. MCP server 在 `mcp-servers.example.json` 中 `enabled: false`，由使用者在 `mcp-servers.local.json` 自行啟用

## Requirements

- [ ] R-01 — `/deep-planning` Step 1 在 `graphify-out/GRAPH_REPORT.md` 存在時讀取它作為結構 context
- [ ] R-02 — `golem-architect` 在 `<project_context>` 階段讀取 `GRAPH_REPORT.md`，用 god nodes 和 communities 輔助 trade-off 分析
- [ ] R-03 — `/review` Step 1 在 `GRAPH_REPORT.md` 存在時讀取它，交叉比對變更是否跨越 community 邊界
- [ ] R-04 — `/planning` Step 1 在 `GRAPH_REPORT.md` 存在時讀取它，用 communities 判斷 scope 是否跨模組
- [ ] R-05 — `mcp-servers.example.json` 包含 graphify MCP server 設定（disabled by default）
- [ ] R-06 — `docs/devguide.md` 記載 graphify detection 規則
- [ ] R-07 — graphify 不存在時，所有修改過的 SKILL.md 行為完全不變（zero regression）

## Approach

### Step 1: 條件式 artifact 讀取（Level 1）

- **Files**: `commands/deep-planning/SKILL.md`, `commands/deep-planning/SKILL.template.md`
- **What**: 在 Step 1 "Gather Inputs" 加入一行：`Read graphify-out/GRAPH_REPORT.md if it exists — use god nodes, communities, and surprising connections as structural context for the plan.`
- **Verify**: SKILL.md 包含條件讀取語句，且語意為 "if exists"

### Step 2: Architect agent context

- **Files**: `agent/golem-architect.agent.md`
- **What**: 在 `<project_context>` section 加入：`Read graphify-out/GRAPH_REPORT.md if it exists. Use god nodes to identify core abstractions, communities for module boundary awareness, and surprising connections for hidden coupling. Filter INFERRED edges below 0.7 confidence.`
- **Verify**: Agent file 包含 graphify context 指引

### Step 3: Staff review context

- **Files**: `commands/review/SKILL.md`, `commands/review/SKILL.template.md`
- **What**: 在 Step 1 "Read Changes" 加入條件讀取，指引 reviewer 交叉比對：新增的 import 是否建立非預期的 cross-community edge。
- **Verify**: SKILL.md 包含條件讀取語句

### Step 4: Planning context

- **Files**: `commands/planning/SKILL.md`
- **What**: 在 Step 1 "Gather Inputs" 加入條件讀取，用 communities 輔助 scope 判斷。
- **Verify**: SKILL.md 包含條件讀取語句

### Step 5: MCP server 設定（Level 2）

- **Files**: `mcp-servers.example.json`
- **What**: 加入 graphify server entry，所有 provider 設為 `enabled: false`：

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

- **Verify**: `mcp-servers.example.json` 語法正確，graphify entry 存在且全部 disabled

### Step 6: Detection 文件化

- **Files**: `docs/devguide.md`
- **What**: 新增 "## Graphify Knowledge Graph Detection" section，記載偵測規則和 artifact 說明：
  - 偵測信號：`graphify-out/GRAPH_REPORT.md` 存在
  - 對應 artifact：`graph.json`（可查詢圖）、`graph.html`（視覺化）、`GRAPH_REPORT.md`（摘要報告）
  - MCP server 啟用方式
  - graphify 不是 GAL 核心依賴
- **Verify**: Section 存在且內容準確

## Files to Create or Modify

- `commands/deep-planning/SKILL.md` — 加入條件式 GRAPH_REPORT.md 讀取
- `commands/deep-planning/SKILL.template.md` — 同步修改
- `agent/golem-architect.agent.md` — 加入 graphify context 到 `<project_context>`
- `commands/review/SKILL.md` — 加入條件式讀取
- `commands/review/SKILL.template.md` — 同步修改
- `commands/planning/SKILL.md` — 加入條件式讀取
- `mcp-servers.example.json` — 加入 graphify server entry（disabled）
- `docs/devguide.md` — 加入 graphify detection section

## Test Cases

- [ ] TC-01 — 在有 `graphify-out/GRAPH_REPORT.md` 的 repo 執行 `/deep-planning`，確認 agent 讀取了報告內容
- [ ] TC-02 — 在沒有 `graphify-out/` 的 repo 執行 `/deep-planning`，確認行為與修改前完全一致
- [ ] TC-03 — 在有 graph 的 repo 啟用 MCP server，透過 `query_graph`、`god_nodes`、`shortest_path` 驗證結構查詢
- [ ] TC-04 — 確認 `golem-architect` 在有 graphify context 時引用 god nodes 和 communities 進行 trade-off 分析
- [ ] TC-05 — 確認 `/review` 在有 graphify context 時交叉比對 community 邊界

## Success Criteria

- [ ] SC-01 — 所有 SKILL.md 修改都是 `if exists` 條件式，graphify 不存在時零行為差異
- [ ] SC-02 — `mcp-servers.example.json` 中 graphify 預設 disabled
- [ ] SC-03 — GAL 不包含任何 `pip install graphify` 或自動安裝邏輯
- [ ] SC-04 — `per-repo-context.md` 記載了完整的 detection 規則
- [ ] SC-05 — 有 graphify 的 repo 中，architect review 能引用結構證據（god nodes、communities、surprising connections）

## Risks

- **Graph staleness** — `GRAPH_REPORT.md` 可能落後於最新 commit。Mitigation：graphify 內建 `graphify hook install` 可設定 post-commit 自動 AST-only rebuild（免費）。GAL 不負責管理 graph freshness。
- **Token budget** — `GRAPH_REPORT.md` 約 500–2000 tokens，在 GAL token-budget convention 可接受範圍內。MCP queries 有 `--budget N` 參數可控制。
- **Semantic extraction 成本** — AST-only 提取免費且 sub-second。Document semantic extraction 需要 LLM tokens，由使用者自行決定是否執行。GAL 只消費已產出的 artifact。

## Open Questions

- [x] OQ-001 — graphify 是否適合作為 GAL planning 和 architect review 的 context source？ *(raised by: research, resolved by: empirical test — 12.4x token reduction, 100% community accuracy)*
- [ ] OQ-002 — 是否需要 Level 3 深度整合（cross-community 自動 escalation、god-node change detection）？ *(raised by: planning)* — 建議 Level 2 MCP 驗證後再評估

## Approval

- Human approval: [pending]
- Architect review: [pending]
- Additional domain review: [not requested]

## Review Results

### Architecture Review

Pending.

### Business Review

Not applicable — internal tooling enhancement.

### Design Review

Not applicable — no UI changes.

### Engineering Review

Pending.

## Test Plan

Pending — Level 1 changes are single-sentence SKILL.md additions; Level 2 is a disabled-by-default MCP config entry. Testing primarily through manual `/deep-planning` and `/review` runs on repos with and without graphify.

## Tasks

- T-001 — Add conditional GRAPH_REPORT.md read to `/deep-planning` SKILL.md + template
- T-002 — Add graphify context block to `golem-architect.agent.md`
- T-003 — Add conditional read to `/review` SKILL.md + template
- T-004 — Add conditional read to `/planning` SKILL.md
- T-005 — Add graphify MCP server entry to `mcp-servers.example.json`
- T-006 — Add Graphify detection section to `docs/devguide.md`
- T-007 — Verify zero-regression: run `/deep-planning` and `/review` on a repo without `graphify-out/`
