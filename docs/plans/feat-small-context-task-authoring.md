# Plan: 小 Context 模型可獨立完成單一 task 的計畫撰寫優化

## Approval

- Human approval: [pending]
- Architect review: **APPROVE（post-revision）** *(2026-06-05；見 `## Review Results > ### Architecture Review`；OQ-002/003 已由本次審查收斂)*
- Additional domain review: [not requested]

## Goal

讓 claude-haiku、gpt-5.4-mini、gemini-3.5-flash 這類**小/廉價模型**，在 `/gal pipeline` 被派工時，**只憑單一 task spec 就能正確完成該 task**——不需把整份計畫塞進 spec。為此優化兩個面向：

1. **template 可讀性**：`templates/plan.md` 把 `## Approval` 移到最上層，讓計畫的核准/閘門狀態一眼可見。
2. **task 自足化（核心）**：`/refining-plan` 產出的 `## Tasks` 改為**自足、受預算約束**的執行單元，且派工層（`New-TaskSpec.ps1`）能完整擷取自足內容餵給 executor。

> **拆分說明（2026-06-05）**：原第三面向「review 模型指派 / sender==reviewer」已**拆出為獨立計畫** [`refactor-golem-auditor.md`](refactor-golem-auditor.md)（golem-reviewer + golem-security 合併為 golem-auditor、正確性審查移入 orchestrator gate）。該重組屬 golem 架構層級，範圍大於本計畫，故獨立追蹤。兩計畫的協調點僅在 `scripts/common/New-TaskSpec.ps1`（本計畫改擷取器、另一計畫改 agentMap）與 routing schema，互不衝突但同檔，實作時需對齊先後。

完成時必須成立：

- 新計畫由 `/planning` 產生時，`## Approval` 出現在檔案頂部。
- `/refining-plan` 產出的每個 task 是自足、受預算約束的 spec，且 `New-TaskSpec.ps1` 能完整擷取該 task 的多行內容與**該 task 專屬**的檔案清單。

> **關鍵收斂（2026-06-05 架構審查）**：三個小模型的 context window 分別為 200K / 400K / 1M token——**spec 大小限制不是被 window 綁住，而是被「聚焦 + 成本」綁住**。因此「自足」指**內嵌指令 + 精確指標（哪些檔、做什麼改動、驗收條件、相關慣例路徑）**，而**非內嵌檔案全文**；executor 用自己的大 window 去讀被指名的 repo 檔案。

## Requirements

- [ ] **R-001 — Approval 置頂**：`templates/plan.md` 的 `## Approval` 區段移到標題之後、`## Goal` 之前；既有以區段名稱（非位置）解析計畫的命令（`/refining-plan`、`/plan-to-prompt`、`/deep-planning`、`New-TaskSpec.ps1`）行為不受影響。
- [ ] **R-002 — 自足 task 撰寫契約（指標式，非全文內嵌）**：`/refining-plan` 的 `## Tasks` 撰寫規則升級為：每個 `T-NNN` 須含 (a) 確切目標檔路徑、(b) 具體改動、(c) 就地可驗的驗收條件、(d) 必要的**慣例/簽章/相依指標（路徑與名稱）**。**不得內嵌檔案全文**——executor 以自身 window 讀取被指名的檔案。
- [ ] **R-003 — spec 預算與切分**：`/refining-plan` 為每個 task 套用 spec 大小目標（沿用 `<5KB` 量級，理由為聚焦 + 每次派工成本，非 window 容量）；會使 spec 超限的 task 必須切分為更小的原子 task。預算為**指令/指標**的大小，不含 executor 另行讀取的檔案內容。
- [ ] **R-004 — 派工層擷取自足內容**：`scripts/common/New-TaskSpec.ps1` 須能擷取 `T-NNN` 的**完整多行 task 區塊**（非只第一行），並把 spec 的 `## Affected Files` 收斂為**該 task 專屬**的檔案，而非整份計畫的檔案清單。Rust bin 不變（僅轉送 stdin）。

## Scope

**In scope**：`templates/plan.md` 區段重排；`commands/refining-plan/SKILL.template.md` 的 task/test 撰寫契約升級（指標式自足 + 預算 + 切分）；`scripts/common/New-TaskSpec.ps1` 的擷取升級（多行 task 區塊 + 每 task 檔案收斂）。

**Out of scope**：review 模型指派 / golem 合併（拆至 `refactor-golem-auditor.md`）；`gal-dispatch` bin 的 Rust 改動（spec 內容不經 bin，僅 stdin 轉送）；`executor-routing.json` 結構變更；既有 docs/plans 舊計畫回溯改寫（template 變更只影響新生成計畫）；`New-TaskSpec` 的 Bash 對等（沿用 headless 計畫的 Bash 延後）。

> **受保護路徑警示**：R-001（`templates/`）、R-002（`commands/refining-plan/`）觸及受保護路徑，已由本次 `/deep-planning` 架構審查放行。`scripts/common/New-TaskSpec.ps1` **非**受保護腳本（受保護者僅 Sync-DevContext / Setup-Machine 及其 .sh），可直接修改。

## Approach

### 面向一：template Approval 置頂（低風險，獨立）

- **Files**: `[MODIFY] templates/plan.md`（**受保護路徑**）
- **What**: 將 `## Approval` 整段上移到 `# Plan:` 標題之後、`## Goal` 之前；其餘區段順序不變。
- **Verify**: `/planning` 產生新計畫確認 Approval 置頂；對既有計畫跑 `/refining-plan`、`/plan-to-prompt`、`New-TaskSpec.ps1` 仍以區段名稱正確解析（位置無關）。

### 面向二：自足 task 撰寫契約 + 派工層擷取（核心）

- **Files**: `[MODIFY] commands/refining-plan/SKILL.template.md`（**受保護路徑**）、`[MODIFY] scripts/common/New-TaskSpec.ps1`
- **What**:
  - refining-plan `## Step 3 — Write ## Tasks`：加入「指標式自足 task」規格（R-002）與 spec 預算/切分規則（R-003）。明示「不內嵌檔案全文，只給路徑 + 改動 + 驗收 + 慣例指標」。
  - `New-TaskSpec.ps1`（R-004）：
    - `$taskGoal` 擷取由「單行 `Select-Object -First 1`」升級為「擷取 `T-NNN` 起至下一個 `T-NNN`/區段邊界的完整區塊」。
    - `## Affected Files` 由「整段 `## Files to Create or Modify`」收斂為「該 task 區塊指名的檔案」；無指名時才回退整段。
    - 保留 `<5KB` 目標檢查；超限警告維持（切分責任在 refining-plan 撰寫端）。
- **Verify**: 取一個多行自足 task，跑 `New-TaskSpec.ps1` 確認 spec 含完整 task 區塊 + 僅該 task 檔案 + ≤ 預算；模擬「只給 spec、executor 自讀指名檔」情境足以完成。

## Files to Create or Modify

- `[MODIFY] templates/plan.md`（**受保護路徑**）— Approval 置頂
- `[MODIFY] commands/refining-plan/SKILL.template.md`（**受保護路徑**）— 指標式自足 task + 預算 + 切分契約
- `[MODIFY] scripts/common/New-TaskSpec.ps1` — 多行 task 區塊擷取 + 每 task 檔案收斂

## Test Cases

- [ ] `/planning` 產生的新計畫，`## Approval` 位於檔案頂部
- [ ] `/refining-plan` 對既有計畫仍能以區段名稱正確寫回 `## Tasks` / `## Test Plan` / `### Engineering Review`
- [ ] `New-TaskSpec.ps1` 對多行自足 task 擷取完整區塊（非僅首行）
- [ ] `New-TaskSpec.ps1` 的 `## Affected Files` 僅含該 task 指名檔案
- [ ] 依新契約產出的 task 單獨作為 spec 時自足且 ≤ 預算

## Success Criteria

- [ ] 新計畫的核准狀態置頂可一眼掃讀。
- [ ] `/refining-plan` 產出的 task + `New-TaskSpec.ps1` 擷取，在「只給單一 spec、executor 自讀指名檔」情境下足以正確完成，且受預算約束。

## Risks

- **受保護路徑 blast radius**：`templates/`、`commands/refining-plan/` 為受保護路徑。緩解：架構審查已放行；逐檔最小化；面向一可獨立先行。
- **`New-TaskSpec.ps1` 擷取升級的正則風險**：多行區塊邊界判定（下一 `T-NNN` / 區段 / 列表縮排）易誤切。緩解：以明確邊界規則 + 單元測試覆蓋多 task、單 task、末尾 task。
- **與 `refactor-golem-auditor` 的協調點**：兩計畫都改 `New-TaskSpec.ps1`（本計畫改擷取器、另一改 agentMap）。緩解：互不衝突但同檔，實作時對齊先後、其一 rebase。
- **自足 task 與 token-budget 慣例張力**：內嵌過多會放大計畫體積。緩解：指標式（非全文）+ 大小預算 + 切分；只放小模型不會自帶的指標。

## Open Questions

- [x] OQ-002 — task spec 大小上限數值與依據？**已解（網路查證 + 架構審查）**：三模型 window 為 200K（claude-haiku-4.5）/ 400K（gpt-5.4-mini）/ 1M（gemini-3.5-flash），**非容量瓶頸**。預算依「聚焦 + 每次派工成本」設於 `<5KB` 量級的**指令/指標**大小；executor 自讀被指名檔案，不把檔案全文計入預算。 *(raised by: planning, resolved by: architecture-review)*
- [x] OQ-003 — dispatch spec 在何處組裝？**已解（程式追蹤）**：組裝在 `scripts/common/New-TaskSpec.ps1`（gal.ps1 line 960 呼叫，產生 spec 檔後 `Get-Content … | bin` 餵 stdin）；**Rust bin 僅轉送 stdin，不組裝**。現行擷取只取 task 首行且 dump 整份檔案清單 → R-004 須升級該擷取器；bin 不動。 *(raised by: planning, resolved by: codebase-trace)*

> OQ-001（sender==reviewer / review 模型指派）已隨第三面向拆出至 [`refactor-golem-auditor.md`](refactor-golem-auditor.md)。

<!-- Format: - [ ] OQ-NNN — description *(raised by: command)* -->
<!-- Resolved: - [x] OQ-NNN — description *(raised by: command, resolved by: engineering-review-lane)* -->

## Review Results

### Architecture Review

#### Verdict: APPROVE *(2026-06-05, deep-planning 架構審查；含本次收斂修訂)*

方向成立、範圍可控、依賴已釐清。原計畫有兩處承重缺陷已於本次修訂消除：(1) 把「自足」誤設為內嵌檔案全文——三模型 window 皆 ≥200K，內嵌全文是 over-engineering 且與 token-budget 慣例衝突；改為指標式。(2) 未定位 spec 組裝點——已追蹤至 `New-TaskSpec.ps1`，且發現現行擷取器只取 task 首行，會讓自足 task 在派工層被悄悄截斷。原第三面向（review 模型指派）已拆出獨立計畫。

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| 指標式自足 task（非全文內嵌） | 小 spec、低成本、聚焦；executor 用大 window 自讀檔 | refining-plan 撰寫需更嚴謹 | OK |
| `<5KB` 預算理由改為聚焦+成本（非 window） | 與真實瓶頸對齊；不被 window 數字誤導 | 需教育撰寫者預算含義 | OK |
| `New-TaskSpec.ps1` 擷取多行區塊 + 每 task 檔案 | 自足內容真正送達 executor | 正則邊界風險，需測試 | OK（BUG-01/02 已納入 R-004） |

#### Over-engineering Flags

- **[OE-02] 已採納**：自足 task 不內嵌檔案全文。200K~1M window 下，executor 自讀被指名檔案即可；內嵌全文純放大 spec 與成本。

#### Bug Surface

- **[BUG-01] High（已納入 R-004）**：`New-TaskSpec.ps1` 現以 `Select-Object -First 1` + 單行正則擷取 task → 多行自足 task 會被截成首行，R-002 形同虛設。Fix：擷取 `T-NNN` 完整區塊至下一邊界。
- **[BUG-02] Medium（已納入 R-004）**：spec 的 `## Affected Files` 現 dump 整份 `## Files to Create or Modify` → 小模型收到跨 task 噪音，可能改錯檔。Fix：收斂為該 task 指名檔案。

#### Missing from Plan（已於本次收斂補入）

- spec 組裝點定位（`New-TaskSpec.ps1`）與其擷取器升級（R-004）。
- 「自足 = 指標式而非全文」的明確定義（R-002、OE-02）。

#### What's Good (keep these)

- 面向一（Approval 置頂）低風險、與其餘解耦，可先行。
- 把預算理由錨定在「聚焦 + 成本」而非 window 容量——避免被大 window 數字誤導成「反正塞得下」。
- 把 review 模型重組拆出獨立計畫，控制本計畫範圍。

### Business Review

Not requested。

### Design Review

Not requested（無 customer-facing UI）。

### Engineering Review

Pending（`## Tasks` 與 `## Test Plan` 仍為 placeholder；下一步 `/refining-plan`）。

## Test Plan

Pending.

## Tasks

Pending.
