# Plan: golem-reviewer + golem-security 合併為 golem-auditor，正確性審查移入 orchestrator gate

## Approval

- Human approval: [pending]
- Architect review: [pending] — **建議走 `/deep-planning`**（golem 架構層級重組 + 觸及多個受保護路徑 + 跨 plan 依賴）
- Additional domain review: [not requested]

## Goal

把 code review 的兩種職責沿「**對不對 vs 安不安全/深層問題**」徹底切開，並重新指派執行者：

- **Orchestrator inline gate（正確性）**：跑 `/gal pipeline` 的會話本身，在收回 implement 結果的當下，用其完整計畫 context 確認「這段 code 有沒有正確完成 `T-NNN`」——含功能正確性、架構適配、程式碼品質、明顯效能問題。**不 dispatch**，是 implement 與 test 之間的 gate。通過才進 TESTER。
- **golem-auditor（深層 + 安全，永遠 dispatch）**：由 `golem-reviewer` 與 `golem-security` **合併**而成的單一稽核 specialist，負責深度效能與全部安全性審查。由 conditional（過去只在敏感 surface 才跑）**升格為常態**，每個 task 都跑。

完成時必須成立：

1. `golem-reviewer` 與 `golem-security` 合併為 `golem-auditor`（其一改名擴範圍、另一刪除並併入），無殘留死引用。
2. `/gal pipeline` 相位改為：implement → **orchestrator 正確性 gate** → test → **auditor（常態）** → verify。原 dispatched REVIEWER 相位移除。
3. 職責切分明確且文件化：orchestrator 做正確性/架構/品質（review 清單 1–13 + 明顯效能）；golem-auditor 做深度效能 + 安全（清單 14–23）。
4. `executor-routing.json` 的 `REVIEWER` 角色鍵改為 `AUDITOR`，引用全數更新或由 sync 重生。

## Requirements

- [ ] **R-001 — golem-auditor 角色建立**：`golem-security.agent.md` 改名 + 擴範圍為 `golem-auditor.agent.md`，吸收 `golem-reviewer` 的深度效能審查職責；`golem-reviewer.agent.md` 刪除。
- [ ] **R-002 — orchestrator 正確性 gate 正式化**：`/gal pipeline` 把「收回 implement 後的確認」正式寫成 implement→test 之間的 gate，職責為 review 清單 1–13 + 明顯效能；不通過退回 `--fix-mode`，通過才進 test。此 gate 是 orchestrator 本身的一部分，不是獨立 golem、不 dispatch。
- [ ] **R-003 — auditor 升格常態**：`golem-auditor` 由 conditional 改為每個 task 固定 dispatch；保留既有 security 嚴重度 STOP 規則（high/critical 開放即停）。
- [ ] **R-004 — 相位與契約重寫**：`commands/gal-pipeline/SKILL.template.md` 移除 dispatched REVIEWER 相位（2e）、新增 orchestrator 正確性 gate、把 2f 條件式 security 改為常態 auditor 相位；Model Assignment 表更新。
- [ ] **R-005 — 跨模型政策對齊**：`workflows/coding.md` 重寫 REVIEWER 角色（移除或重定義）、新增 AUDITOR；明示「正確性 gate 由 orchestrator 擔任、稽核由獨立 auditor dispatch」的獨立性語意。
- [ ] **R-006 — routing 鍵遷移**：`executor-routing.json` / `.example.json` 的 `REVIEWER` 鍵改 `AUDITOR`；`crates/gal-dispatch` 的 stage→role 對照（review→? / audit→AUDITOR）對齊；`New-TaskSpec.ps1` agentMap 的 `review→golem-reviewer` 改為 auditor 相位映射。
- [ ] **R-007 — blast radius 清零**：34 處 `golem-reviewer`/`golem-security`/`REVIEWER` 引用全數更新或由 sync 重生（含生成 adapter：AGENTS/CLAUDE/GEMINI/copilot-instructions/i18n README）；grep 驗無殘留死引用。

## Scope

**In scope**：`agent/` 的 golem 合併與刪除；`commands/gal-pipeline` 相位重構；`workflows/coding.md` 角色與政策；`executor-routing` 鍵遷移；`crates/gal-dispatch` stage→role 對照與 `New-TaskSpec.ps1` agentMap；全 repo 引用遷移；生成 adapter 由 sync 重生。

**Out of scope**：小 context task 撰寫優化（屬 `feat-small-context-task-authoring.md`，僅在 `New-TaskSpec.ps1` 與 routing schema 上有協調點，見 Risks）；orchestrator 正確性判斷的 LLM 內部機制（屬執行期行為，非契約）；golem-architect（規劃期審查，不在本次重組範圍）。

> **受保護路徑警示**：`commands/gal-pipeline/`、`workflows/coding.md`、`Sync-DevContext.*`（重生 adapter）皆為受保護路徑；本計畫為 golem 架構層級重組，**必須先過 `/deep-planning`**。

## Approach

### 面向一：golem 合併（golem-reviewer + golem-security → golem-auditor）

- **Files**: `[RENAME→EXPAND] agent/golem-security.agent.md → agent/golem-auditor.agent.md`、`[DELETE] agent/golem-reviewer.agent.md`、`[MODIFY] agent/agents.md`
- **What**: 以 golem-security 為基底改名 golem-auditor，吸收 golem-reviewer 的深度效能審查清單（14–16）；安全清單（17–23）保留；正確性/架構/品質（1–13）**移出** auditor（改由 orchestrator gate）。
- **Verify**: `golem-auditor.agent.md` 職責 = 深度效能 + 安全；`golem-reviewer.agent.md` 不存在；agents.md 索引更新。

### 面向二：pipeline 相位重構

- **Files**: `[MODIFY] commands/gal-pipeline/SKILL.template.md`（**受保護路徑**）、`[MODIFY] scripts/common/New-TaskSpec.ps1`
- **What**: 2c implement 後插入「2d orchestrator 正確性 gate」（清單 1–13 + 明顯效能，不通過退 fix）；原 2d test 順延；移除原 2e dispatched REVIEWER；原 2f conditional security 改為「auditor 常態相位」；Model Assignment 表反映新相位序。`New-TaskSpec.ps1` agentMap 對齊。
- **Verify**: 相位序為 implement → 正確性 gate → test → auditor → verify；無 REVIEWER dispatch；auditor 每 task 跑。

### 面向三：政策 + routing + blast radius

- **Files**: `[MODIFY] workflows/coding.md`（**受保護路徑**）、`[MODIFY] executor-routing.json/.example.json`、`[MODIFY] crates/gal-dispatch/src/stage.rs`、生成 adapter 由 `Sync-DevContext.*` 重生、其餘 docs 引用
- **What**: coding.md 重寫 REVIEWER→（正確性 gate 語意）+ 新增 AUDITOR；routing `REVIEWER`→`AUDITOR`；stage→role 對照更新；grep 掃 34 處引用全部遷移或重生。
- **Verify**: repo 無 `golem-reviewer`/`golem-security` 有效引用；生成 adapter 無死連結；routing schema 一致。

## Files to Create or Modify

- `[RENAME+EXPAND] agent/golem-security.agent.md → agent/golem-auditor.agent.md`
- `[DELETE] agent/golem-reviewer.agent.md`
- `[MODIFY] agent/agents.md` — golem 索引
- `[MODIFY] commands/gal-pipeline/SKILL.template.md`（**受保護路徑**）— 相位重構
- `[MODIFY] scripts/common/New-TaskSpec.ps1` — agentMap review→auditor 相位
- `[MODIFY] workflows/coding.md`（**受保護路徑**）— REVIEWER 重定義 + AUDITOR 新增
- `[MODIFY] executor-routing.json` / `executor-routing.example.json` — REVIEWER→AUDITOR 鍵
- `[MODIFY] crates/gal-dispatch/src/stage.rs` — stage→role 對照
- `[MODIFY] commands/gal-status`, `commands/gal-whats-next`, `commands/commands.md`, `templates/plan-prompt.md`, `docs/manual.md`, `README.md`, 其餘引用
- `[REGEN] AGENTS.md, CLAUDE.md, GEMINI.md, .github/copilot-instructions.md, docs/i18n/zh-Hant/README.zh-Hant.md` — 由 `Sync-DevContext.*` 重生

## Review 工作職責切分（決策基準表）

| # | 類別 | 工作項目 | Orchestrator gate | golem-auditor |
| --- | --- | --- | --- | --- |
| 1 | 功能正確性 | Task spec 符合性 | ✅ | |
| 2 | | 邏輯正確性 | ✅ | |
| 3 | | 邊界條件 | ⚠️ 基本 | ✅ 深度 |
| 4 | | 錯誤處理 | ✅ | |
| 5 | | 回傳值/副作用 | ✅ | |
| 6 | 架構適配 | 層級邊界 | ✅ | |
| 7 | | 既有模式一致性 | ✅ | |
| 8 | | 抽象適切性(YAGNI) | ✅ | |
| 9 | | 命名慣例 | ✅ | |
| 10 | | 檔案/模組組織 | ✅ | |
| 11 | 程式碼品質 | 可讀性 | ✅ | |
| 12 | | 重複程式碼 | ✅ | |
| 13 | | 無效程式碼 | ✅ | |
| 14 | 效能 | N+1 pattern | ⚠️ 明顯 | ✅ 深度 |
| 15 | | 無邊界資料載入 | ⚠️ 明顯 | ✅ 深度 |
| 16 | | 熱路徑同步 I/O | | ✅ |
| 17 | 安全性 | 輸入驗證 | | ✅ |
| 18 | | 認證/授權 | | ✅ |
| 19 | | Injection | | ✅ |
| 20 | | 機密資料處理 | | ✅ |
| 21 | | 信任邊界 | | ✅ |
| 22 | | 資料暴露 | | ✅ |
| 23 | | 權限提升 | | ✅ |

## Test Cases

- [ ] `golem-auditor.agent.md` 存在且職責為深度效能 + 安全；`golem-reviewer.agent.md` 已刪
- [ ] pipeline 相位序：implement → 正確性 gate → test → auditor → verify
- [ ] 正確性 gate 不通過時退 `--fix-mode`，不進 test
- [ ] auditor 每個 task 都跑（非 conditional），high/critical 開放即 STOP
- [ ] routing `AUDITOR` 鍵解析正確；`REVIEWER` 鍵不再被引用
- [ ] grep 無 `golem-reviewer`/`golem-security` 有效引用；生成 adapter 無死連結

## Success Criteria

- [ ] 兩個 golem 合併為 golem-auditor，職責邊界與 orchestrator gate 清楚切分且文件化。
- [ ] pipeline 在 test 前先由 orchestrator 確認正確性，省掉「對明顯錯誤的 code 跑測試」的浪費。
- [ ] 安全/深度稽核獨立性保留（auditor 永遠 dispatch）。
- [ ] 34 處引用清零，無回歸。

## Risks

- **大範圍引用遷移（34 檔）**：含受保護路徑與生成 adapter，與 model-roles 遷移同級。緩解：仿照 headless 計畫的階段化遷移 + grep 驗證；adapter 一律由 sync 重生不手改。
- **正確性審查獨立性流失（已知取捨）**：正確性/架構/品質改由 orchestrator 做，當 implement inline（orchestrator==CODER）時失去「全新視角」。緩解：深度+安全由獨立 auditor 保留；TESTER 獨立；接受正確性 gate 為「早期 catch」而非獨立 review。**此取捨需架構審查確認（見 OQ-001）。**
- **與 `feat-small-context-task-authoring` 的協調點**：兩計畫都改 `New-TaskSpec.ps1` 與 routing schema。緩解：本計畫負責 review/audit 相關鍵；兩計畫的 `New-TaskSpec.ps1` 改動需在實作時對齊（agentMap vs 擷取器，互不衝突但同檔）。建議先行其一、另一 rebase。
- **auditor 常態化的成本**：每 task 都跑稽核會增加 dispatch 成本。緩解：auditor 是 spec 導向 + 讀指名檔，成本可控；若過重可日後再加複雜度門檻（YAGNI，暫不設）。

## Open Questions

- [ ] OQ-001 — 正確性/架構/品質審查改由 orchestrator gate（implement inline 時無獨立視角），是否可接受？或需為複雜/保護路徑 task 保留一個降頻的獨立正確性審查？需架構審查裁定。 *(raised by: planning)*
- [ ] OQ-002 — golem-auditor 命名最終確認（vs auditor / inspector / 其他）；以及 `New-TaskSpec.ps1` agentMap 的相位鍵命名（`audit` 相位 vs 沿用 `review`）。 *(raised by: planning)*
- [ ] OQ-003 — 兩計畫共改 `New-TaskSpec.ps1` 與 routing schema 的施作順序（哪個先落地、另一個 rebase）。 *(raised by: planning)*

<!-- Format: - [ ] OQ-NNN — description *(raised by: command)* -->

## Review Results

### Architecture Review

Pending（建議 `/deep-planning`）。

### Business Review

Not requested。

### Design Review

Not requested（無 customer-facing UI）。

### Engineering Review

Pending。

## Test Plan

Pending。

## Tasks

Pending。
