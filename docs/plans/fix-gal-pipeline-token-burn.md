# Plan: GAL Pipeline Token Burn Reduction

## Goal

在不削弱 GAL 的安全保證下，降低 `/gal pipeline` 的 token 消耗，並特別防止 OpenCode Go 的每日與每週額度因為多餘的啟動上下文、重複的階段上下文載入，以及可避免的同 runtime 開銷而過早耗盡。

## Baseline Burn Surface

在首次實作這個計畫前的歷史基準線：

| 檔案 | 行數 | 位元組 | 備註 |
| ------ | ------- | ------- | ------- |
| `AGENTS.md` | 1,121 | 57,284 | 生成的轉接器 |
| `.github/copilot-instructions.md` | 1,069 | 53,923 | 生成的轉接器 |
| `CLAUDE.md` | 1,121 | 57,203 | 生成的轉接器 |
| `GEMINI.md` | 1,121 | 57,313 | 生成的轉接器 |
| `golem-implementer.agent.md` | 220 | 11,199 | 在 L30 讀取 copilot-instructions.md |
| `golem-tester.agent.md` | 319 | 11,082 | 在 L39 讀取 copilot-instructions.md |
| `golem-reviewer.agent.md` | 285 | 11,316 | 在 L30 讀取 copilot-instructions.md |
| `golem-verifier.agent.md` | 188 | 7,448 | |
| `.dev/project.md` | 98 | 5,450 | 實際的真相來源 (source of truth) |

在基準線上，四個生成的轉接器幾乎是相同的複製檔案。只有前 ~10 行不同，且 OpenCode 啟動時會載入兩個大型生成的轉接器。綁定 pipeline 的 agent 合約也指示每個階段都要再進行一次生成轉接器的讀取。

## Current Verified Surface

於 2026-05-29 針對目前的 repository 進行檢查：

| 表面 | 目前狀態 | 差距 |
| --- | --- | --- |
| OpenCode 啟動 | `opencode.json` 僅載入 `AGENTS.md`。 | 啟動負載無差距。 |
| 生成轉接器大小 | `AGENTS.md` 約 40 KB；經過語言範圍的嵌入 (embedding) 後，供應商轉接器約 39-41 KB。 | 基準線表格僅為歷史記錄。 |
| 綁定 Pipeline 的 agents | Implementer、tester、reviewer、security 與 verifier 將生成轉接器視為已載入的 runtime 載體，並將 `.dev/project.md` 作為後備。 | Designer / analyst / architect 仍提及 `copilot-instructions.md`，但他們在此計畫中並非綁定 pipeline 的 agents。 |
| 分派器中介資料 | PowerShell 與 Bash 分派器 (dispatchers) 會發出 `CONTEXT_CARRY`、`PIPELINE_CONTEXT_MODE`、`PIPELINE_CONTEXT_FILES` 與 `CONVENTION_HINTS`。 | Bash runtime 的執行仍需要在具備 Bash 的主機上進行欄位驗證。 |
| 指令合約 | `commands/gal-pipeline/SKILL.template.md` 包含同 runtime 降級標記、commit 邊界收斂關卡、重試上限、受保護路徑升級、中斷階段交接以及最終驗證者需求。 | Repo 來源目前僅有 `SKILL.template.md`；生成的 `commands/gal-pipeline/SKILL.md` 是本地烤好的 (baked) 輸出，且在 `Update-Commands` 執行前可能不存在。 |
| Token-burn 測試腳本 | `scripts/Test-PipelineTokenBurn.ps1` 現在會優先選擇烤好的 `commands/gal-pipeline/SKILL.md` 並在缺少時退回 `commands/gal-pipeline/SKILL.template.md`，以解析目前的 repo 佈局。該腳本在目前僅有 source 佈局的 repo 中通過了 55 項檢查。 | 在 source 佈局驗證上無差距。 |

## Success Criteria (Numeric)

| 指標 | 目前 | 目標 |
| -------- | --------- | -------- |
| OpenCode 啟動負載 | 基準線 ~111 KB；目前 ~40 KB | ≤60 KB |
| 單一任務 3 階段總上下文載入 | 基準線 ~270 KB；最後一次測量目前為 42,163 位元組 | ≤80 KB |
| 5 任務 pipeline 總冗餘上下文 | 基準線 ~1,071 KB；最後一次測量目前為 47,159 位元組 | ≤200 KB |

## Requirements

- [x] `/gal pipeline` 必須繼續遵守現有的安全模型：實作、測試、審查、有條件的資安、驗證者、重試上限、中斷階段交接，以及受保護路徑升級都必須保持完整。
- [x] OpenCode 必須停止在啟動時載入重複的大型生成轉接器；repo 專屬的 OpenCode 橋接通道應只載入單一具權威性的指令載體。
- [x] 綁定 pipeline 的 golem agents 必須停止在一般的階段執行期間重新讀取生成的轉接器，除非任務明確與轉接器內容有關。
- [x] 為了節省 token，同 runtime 的後備預設不能默默地破壞規格驅動測試與審查的獨立性。
- [x] 任何同 runtime 的綑綁最佳化都必須是明確的、被記錄為降級的驗證獨立性，並且仍必須為測試與審查產生獨立的持久寫回資料。
- [x] 在任何記錄任務進度或完成的 git commit 之前，跨 `docs/plans/<slug>.md`、`.dev/plans/<slug>.prompt.md` 與 `.dev/state.md` 這三個表面的持久狀態必須完全收斂；執行中的任務本地編輯可以內部暫存，但任何 committed 狀態都不能包含跨表面的分歧。
- [x] 分派與 agent 變更必須傾向於精簡的注入上下文以及按需讀取，而非重複完整的 cold-start 載入。
- [x] 生成轉接器內容應在生成層，使用選擇性的語言範圍嵌入 (language-scoped embedding) 來進行壓縮，而非進行全文複製或指標間接引用。
- [x] 個人化的 runtime 指令映射（包含個人化的 `AGENTS.md`），必須位於 `~/.gal/generated` 或供應商可見的 `.gal` 映射路徑下，而非 GAL source repository 的根目錄。
- [x] 變更必須保留 Copilot、Antigravity CLI、Codex CLI、Claude Code 以及 OpenCode 橋接通道間的跨 runtime 一致性。
- [x] 生成的轉接器僅維持作為衍生輸出；任何修正都必須在 source 檔案與同步腳本中編寫，不得手動編輯生成的轉接器檔案。
- [x] 實作必須產出相較於上述數字目標，可衡量的改善前後證據。

## Approach

### Problem Statement

P0 與 P0b 已經解決了 `/gal pipeline` 重複手動呼叫的問題，但並未減少每項任務的總 token 消耗。其餘的高成本表面包含：

1. OpenCode 啟動時從 `opencode.json` 載入兩個大型生成的轉接器 (~111 KB，內容幾乎相同)。
2. 綁定 pipeline 的 agents 在每個階段重新讀取 `copilot-instructions.md`，儘管它早已作為系統指令載入 (~54 KB × N 個階段)。
3. 所有轉接器不分任務語言，直接嵌入全部六個慣例 (convention) 檔案 (共 ~734 行，其中對於單一任務來說約 500 行是無關的)。
4. 分派並未提供精簡的上下文區塊，導致 agents 執行冗餘的檔案讀取來確定階段狀態。

此計畫嚴格依照 ROI/風險順序修復這些表面，並將較高風險的收斂與綑綁變更留待稍後、經過明確審查的階段進行。

---

### Step 1 (P0-a): Immediate OpenCode startup diet

- **Files**: `opencode.json`
- **What**: 更改 OpenCode 的 `instructions` 陣列，只載入 `AGENTS.md`，並移除 `.github/copilot-instructions.md`。`AGENTS.md` 是具權威性的跨 CLI 載體；Copilot 專屬的轉接器對 OpenCode 而言是多餘的。
- **Expected saving**: 每個工作階段 ~54 KB。
- **Verify**: OpenCode 載入一個生成的轉接器而非兩個。比較前後的啟動負載。確認在橋接通道下 repo 的行為沒有改變。

---

### Step 2 (P0-b): Remove generated-adapter re-reads from pipeline-bound agents

- **Files**: `agent/golem-implementer.agent.md`, `agent/golem-tester.agent.md`, `agent/golem-reviewer.agent.md`, `agent/golem-security.agent.md`, `agent/golem-verifier.agent.md`
- **What**: 更新綁定 pipeline 的上下文規則，讓這些 agents 將生成的轉接器視為已載入的 runtime 載體。在一般的階段執行期間，他們應該讀取 `.dev/project.md` (98 行)、`.dev/state.md`、活躍的執行 prompt，以及該任務需要的特定 source-of-truth 慣例或程式碼檔案。
- **Fallback rule**: 如果偵測不到系統轉接器 (例如裸終端機呼叫)，agents 會將 `.dev/project.md` (98 行) 作為後備 — 而非 `copilot-instructions.md` (1,069 行)。這使後備成本減少了 10 倍。
- **Expected saving**: 在一個 5 任務的 pipeline 執行中約減少 810 KB。
- **Verify**: 綁定 pipeline 的 agent 文件不再指示進行 `.github/copilot-instructions.md`、`AGENTS.md`、`CLAUDE.md` 或 `GEMINI.md` 的例行讀取。

---

### Step 3 (P1): Language-scoped convention loading

- **Files**: `scripts/gal.ps1`, `scripts/gal.sh`, pipeline agent contracts if needed
- **What**: 分派器偵測活躍任務的主要語言 (從 prompt 描述或 `.dev/project.md` tech stack)，並僅注入相關的語言慣例，加上兩個跨語言慣例 (`token-budget.md`、`working-hours.md`)。該階段會排除所有其他的語言慣例。
- **Why this beats pointer-based compression**: 選擇性載入才是真正的消除內容。基於指標的壓縮將讀取成本從系統指令載入階段轉移到 runtime 的工具呼叫階段，並增加了檔案 I/O 開銷 — 淨節省幾乎為零。透過語言範圍進行排除，消除了從不需要的內容。
- **Example**: 一個 Go 任務載入 `go.md` (56 行) + `token-budget.md` (145 行) + `working-hours.md` (90 行) = 291 行，而不是全部的 734 行 (~減少 60%)。
- **Verify**: Go 任務的分派輸出不包含 C#、Rust 或 TypeScript 的慣例內容。C# 任務不包含 Go、Rust 或 TypeScript 內容。

---

### Step 4 (P2): Compact dispatch-level injected context

- **Files**: `scripts/gal.ps1`, `scripts/gal.sh`
- **What**: 為綁定 pipeline 的分派輸出擴充精簡的上下文區塊 (~20-40 行)，包含活躍計畫路徑、來源計畫路徑、任務參考、階段、重試計數器、commit 標記與下一個未核取的任務。
- **What not to do**: 不注入完整的 `.dev/project.md`、完整的計畫文字或完整的慣例。
- **Context-carry flag**: 如果 runtime 支援同工作階段上下文繼承 (例如 Claude Code 對話)，任務的第 2 與第 3 階段僅會收到差異注入 (階段名稱、重試計數器)。Cold-start 的 runtime 會收到完整的精簡區塊。分派器應發出 `CONTEXT_CARRY: true/false` 欄位，讓 agent 合約能相對應地進行分支判斷。
- **Expected saving**: 在一個 5 任務的 pipeline 執行中約減少 120-225 KB。
- **Verify**: 綁定 pipeline 的 agents 可使用注入的上下文作為取得本地階段狀態的第一來源，且只有在缺乏注入時才退回 `.dev/state.md` 或 prompt。

---

### Step 5 (P3): Selective language-scoped adapter embedding at generator layer

- **Files**: `scripts/Sync-DevContext.ps1`, `scripts/sync-dev-context.sh`
- **What**: 讀取 `.dev/project.md` 的 `Tech Stack` 區塊來生成轉接器，以決定要嵌入哪些語言慣例。僅嵌入該專案實際使用的語言慣例，加上兩個跨語言慣例。保持 `AGENTS.md` 為權威的共用指令載體，但也保留供應商專屬的轉接器，因為供應商可能會自動讀取 `CLAUDE.md`、`GEMINI.md` 或 `.github/copilot-instructions.md`，而 GAL 無法可靠地禁用該行為。
- **Constraint**: 不要使用指標或索引替換慣例內容 — 轉接器必須保持自我完備。節省來自於選擇性包含，而非間接參考。
- **Constraint**: 個人化的 runtime 指令映射 (包含個人化 `AGENTS.md`)，必須生成於 `~/.gal/generated` 或供應商可見的 `.gal` 映射路徑下，不可寫入 GAL source repository 根目錄。
- **Constraint**: 這裡觸及了受保護的路徑並更改了所有 runtime 表面；這需要明確的審查，並且必須保留一個結尾換行符號以及目前的生成檔案語義。
- **Expected saving**: 每個轉接器大小縮減量與被排除的語言成比例；轉接器維持自我完備並有效。
- **Verify**: 重新生成的轉接器僅包含該專案實際的語言慣例。四個轉接器目標在重新生成後，跨 runtime 行為均經驗證。

---

### Step 6: Re-evaluate same-runtime bundling after context slimming lands

- **Files**: `commands/gal-pipeline/SKILL.template.md`, 生成的 `commands/gal-pipeline/SKILL.md` (當 `Update-Commands` 產生它時), pipeline agent contracts if needed
- **What**: 重新評估同 runtime 後備機制是否應支援明確選用的 (opt-in) 綑綁模式來執行實作、測試與審查。
- **Default**: 不要將綑綁作為預設，因為這會削弱測試者的獨立性。
- **If accepted**: 將此執行標記為「驗證獨立性降級」，並且仍要求針對測試與審查產出獨立的持久化 `## Test Results` 與 `## Review Results` 次區塊。
- **Verify**: 合約清楚區分預設的同 runtime 後備與選用的降級綑綁模式。

---

### Step 7: Enforce commit-boundary state convergence

- **Files**: `commands/gal-pipeline/SKILL.template.md`, 生成的 `commands/gal-pipeline/SKILL.md` (當 `Update-Commands` 產生它時), `.dev/state.md` 語義 (若需要)
- **What**: 更新 pipeline 合約，使得任何記錄任務進度或任務完成的 git commit，只有在 `docs/plans/<slug>.md`、`.dev/plans/<slug>.prompt.md` 以及 `.dev/state.md` 針對相關任務狀態達成一致之後才能進行。只有在未 commit 的進行中任務內部，才允許延後收斂。
- **Constraint**: 不要引入一個隱式的、可被 commit 的 `Convergence Pending` 狀態。如果未來提出了明確的待定狀態模型，必須先更新 `status`、`whats-next` 與 commit 關卡才能發布。
- **Verify**: 在任何 pipeline commit 之前，三個持久狀態表面都會被重新讀取並檢查是否一致；不能有任何 commit 記錄著來源計畫、執行 prompt 與 `.dev/state.md` 間的分歧。

---

## Files to Create or Modify

### Required authored changes

- `opencode.json` — 為 OpenCode 移除重複的生成轉接器啟動載入。
- `agent/golem-implementer.agent.md` — 移除例行生成的轉接器重讀；加入後備規則 (`.dev/project.md` 而非 `copilot-instructions.md`)。
- `agent/golem-tester.agent.md` — 在移除例行生成轉接器重讀的同時保留規格驅動的驗證；加入後備規則。
- `agent/golem-reviewer.agent.md` — 轉換為精簡的 pipeline 綁定上下文並移除例行的生成轉接器重讀；加入後備規則。
- `agent/golem-security.agent.md` — 維持稽核範圍在任務層級，避免不必要的大範圍重讀。
- `agent/golem-verifier.agent.md` — 消耗精簡的完成上下文同時保留逆向目標驗證；加入後備規則。
- `scripts/gal.ps1` — 發出精簡的綁定 pipeline 注入上下文；加入 `CONTEXT_CARRY` 欄位；發出語言範圍慣例注入。
- `scripts/gal.sh` — 映射上述行為。
- `scripts/Test-PipelineTokenBurn.ps1` — 針對啟動負載、任務範圍慣例提示、delta 模式分派行為、轉接器瘦身，以及烤好的 pipeline 合約標記，提供可重複的驗證。

### Conditional or later-stage changes

- `scripts/Sync-DevContext.ps1` — 選擇性的語言範圍轉接器嵌入。
- `scripts/sync-dev-context.sh` — Bash 端映射。
- `commands/gal-pipeline/SKILL.template.md` — 同 runtime 後備措辭、綑綁模式降級標記、重試/受保護路徑/中斷/最終驗證者安全關卡，以及 commit 邊界收斂的硬關卡。
- `conventions/token-budget.md` — 只有在確認此為共用方法論而非專案本地調校時，才加入關於語言範圍慣例載入的可重用教訓。
- `.dev/state.md` — 只有當 commit 邊界收斂語義需要工作階段連續性措辭變更時。

### Generated validation outputs (do not edit directly)

- `AGENTS.md`, `.github/copilot-instructions.md`, `CLAUDE.md`, `GEMINI.md` — 在任何 sync 腳本變更後進行驗證；不要直接編輯。
- `commands/*/SKILL.md` — 來自 `Update-Commands` 的本地烤好指令輸出；若存在則在生成後驗證，但 source 編輯應留在 `SKILL.template.md`。

## Tasks

- [x] T-001 — 移除重複的 OpenCode 啟動轉接器載入，讓 OpenCode 僅使用 `AGENTS.md` 作為其啟動指令載體。
  Verify: `opencode.json` 包含且僅有一個 `instructions` 項目，那就是 `AGENTS.md`。
- [x] T-002 — 更新綁定 pipeline 的 agent 合約，停止例行性的生成轉接器重讀，並在偵測不到 runtime 轉接器時使用 `.dev/project.md` 作為精簡後備。
  Verify: implementer、tester、reviewer、security 與 verifier 合約包含了排除生成轉接器與 `.dev/project.md` 後備規則。
- [x] T-003 — 在 PowerShell 與 Bash 分派器中加入語言範圍的慣例選擇以及精簡的 pipeline 中介資料。
  Verify: 分派輸出包含 `CONTEXT_CARRY`、`PIPELINE_CONTEXT_MODE`、`PIPELINE_CONTEXT_FILES` 與 `CONVENTION_HINTS`；任務層級語言提示優先於專案 tech-stack 後備。
- [x] T-004 — 在 PowerShell 與 Bash 轉接器生成器中加入選擇性的語言範圍轉接器嵌入。
  Verify: 生成的轉接器僅包含慣例概覽、`token-budget.md`、`working-hours.md` 以及專案相關的語言慣例檔案。
- [x] T-005 — 以外加的同 runtime 降級後備、選用式綑綁模式降級標記、重試上限、受保護路徑升級、中斷階段交接、最終驗證者與 commit 邊界收斂關卡，來更新 `/gal pipeline` source 合約。
  Verify: `commands/gal-pipeline/SKILL.template.md` 包含這些合約標記；當生成的 `commands/gal-pipeline/SKILL.md` 存在時僅作為驗證輸出。
- [x] T-006 — 修正 `scripts/Test-PipelineTokenBurn.ps1`，讓指令合約驗證優先解析目前的 repo 佈局：偏好 `commands/gal-pipeline/SKILL.template.md`，或者在隔離的驗證流程中透過 `Update-Commands` 生成/讀取本地烤好的 `commands/gal-pipeline/SKILL.md`。
  Verify: 測試不再單純因為受追蹤的 source 缺少 `commands/gal-pipeline/SKILL.md` 而失敗。
- [x] T-007 — 重新執行 token-burn 驗證，並針對計畫中的數字目標記錄目前的修改前後指標。
  Verify: `scripts/Test-PipelineTokenBurn.ps1` 通過並印出啟動負載、單任務 3 階段載體負載，以及 5 任務載體負載。
- [x] T-008 — 針對可丟棄的腳手架計畫/prompt 執行一個有界的 `/gal pipeline stop-at T-NNN` 場景，並確認實作、測試與審查仍寫入獨立的持久區塊。
  Verify: 腳手架來源計畫、執行 prompt 與 `.dev/state.md` 在沒有記錄 committed 分歧的情況下達成收斂。
- [x] T-009 — 確認瘦身變更沒有使重試上限、受保護路徑升級、中斷階段交接或最終驗證者行為倒退。
  Verify: 靜態合約檢查，加上針對每個安全標記所做的腳手架或解析器檢查。
- [x] T-010 — 在具備 Bash 的主機上，驗證 Bash 分派器與轉接器生成的同等性，或者明確記錄主機的限制。
  Verify: Bash 分派器發出與 PowerShell 相同的精簡欄位與慣例提示；Bash 轉接器生成鏡像了 PowerShell 的語言範圍嵌入。

## Test Plan

| ID | 類型 | 描述 | 涵蓋 |
| --- | --- | --- | --- |
| TP-001 | 手動 | 檢查 `opencode.json` 並確認 OpenCode 在啟動時只載入 `AGENTS.md`。 | T-001 |
| TP-002 | 靜態 | 審查五個綁定 pipeline 的 agent 合約，並確認它們排除例行性生成轉接器重讀，並使用 `.dev/project.md` 作為後備載體。 | T-002 |
| TP-003 | 整合 | 從 PowerShell 與 Bash 通道派發具有代表性的 Go 與 C# 任務範圍階段，並確認發出的 `CONTEXT_CARRY`、`PIPELINE_CONTEXT_MODE`、`PIPELINE_CONTEXT_FILES` 與 `CONVENTION_HINTS` 符合任務語言優先權。 | T-003, T-010 |
| TP-004 | 整合 | 透過兩個 sync 路徑重新生成轉接器，並確認只嵌入了慣例概覽、`token-budget.md`、`working-hours.md` 與專案相關的語言慣例。 | T-004, T-010 |
| TP-005 | 靜態 | 檢查 `commands/gal-pipeline/SKILL.template.md` 與任何烤好的 `SKILL.md` 輸出，確認存在同 runtime 降級後備措辭、選用式綑綁模式標記、重試上限、受保護路徑升級、中斷階段交接、最終驗證者與 commit 邊界收斂關卡。 | T-005, T-009 |
| TP-006 | 腳本 | 針對僅有 source 佈局的 repo 執行 `scripts/Test-PipelineTokenBurn.ps1`，確認指令合約驗證可以解析 `SKILL.template.md` 或是隔離出來的烤好輸出，而不是假設存在受追蹤的 `commands/gal-pipeline/SKILL.md`。 | T-006 |
| TP-007 | 腳本 | 重新執行 token-burn 驗證，並針對計畫中的數字目標記錄啟動負載、單任務 3 階段載體負載與 5 任務載體負載。 | T-007 |
| TP-008 | 整合 | 執行一個有界限的可丟棄 `/gal pipeline stop-at T-NNN`，確認實作、測試與審查仍寫入獨立的持久 prompt 區塊，同時來源計畫、執行 prompt 與 `.dev/state.md` 在紀錄進度的 commit 前達到收斂。 | T-008 |
| TP-009 | 靜態 | 在 token-burn 變更後，重新檢查 pipeline 合約以及針對重試上限、受保護路徑升級、中斷階段交接與最終驗證者行為的任何腳手架輸出。 | T-009 |
| TP-010 | 手動 | 如果目前主機無法使用 Bash，在驗證輸出中記錄下這個明確的主機限制，而非聲稱具有同等性。 | T-010 |

## Risks

- 如果 OpenCode 的單一轉接器變更不小心移除了必需的 runtime 指令，OpenCode 行為可能會偏移。修復工作必須保留一個具權威性的轉接器，並在變更後驗證行為。
- 如果 agent 文件停止讀取生成的轉接器，但 `.dev/project.md` 或 source 慣例不完整，專案規則可能會在 pipeline 模式下丟失。使用 `.dev/project.md` 後備規則可以緩解此問題。
- 如果分派器中的語言偵測不準確，可能會注入錯誤的慣例。偵測邏輯必須保守：在模稜兩可時，寧願包含也不要排除。
- 如果同 runtime 綑綁成為預設，測試者的獨立性會削弱，並會增加錯誤的信心。綑綁如果發布，必須保持為明確選用。
- 如果選擇性轉接器嵌入執行得不小心，跨 runtime 行為可能會產生分歧。生成層變更必須經過審查，並透過重新生成的輸出進行驗證。
- 如果收斂延後到了 git commit 邊界之後，repository 歷史紀錄將會記錄不一致的持久狀態。在任何記錄任務進度或完成的 commit 之前，pipeline 必須收斂並重新讀取全部三個狀態表面。
- 此計畫涉及 `commands/`、`conventions/`、`workflows/`、`templates/` 與 sync 腳本之下的受保護路徑，因此實作應停留在已審查的規畫中，而非伺機進行編輯。

## References

- Existing research: [docs/research/pipeline-multi-invocation-overhead.md](../research/pipeline-multi-invocation-overhead.md)
- Active workflow contract: [workflows/coding.md](../../workflows/coding.md)
- Token discipline contract: [conventions/token-budget.md](../../conventions/token-budget.md)
- Current pipeline contract source: [commands/gal-pipeline/SKILL.template.md](../../commands/gal-pipeline/SKILL.template.md)
- OpenCode bridge config: [opencode.json](../../opencode.json)
- OpenCode CLI stats reference: [opencode.ai/docs/cli](https://opencode.ai/docs/cli/)
- Architect analysis: [pipeline_token_burn_analysis.md](../../.gemini/antigravity/brain/aa206f41-de3a-42d3-bf73-3d50cefee377/pipeline_token_burn_analysis.md)

## Open Questions

- [x] OQ-001 — 是否有一種安全的精簡轉接器格式，能讓所有當前的 runtime 消費而不會丟失 cold-start 行為？已由人類決策解決：`AGENTS.md` 仍然是權威的共用指令載體，但供應商專屬轉接器 (如 `CLAUDE.md`、`GEMINI.md` 與 `.github/copilot-instructions.md`) 必須保留，因為供應商可能會自動讀取它們，且 GAL 無法可靠地禁用該行為。精簡轉接器的方向是保守的：保留供應商特定檔案作為生成的 runtime 載體，避免手動編輯，僅透過生成器控制的選擇性嵌入來為它們瘦身，並將個人化 runtime 映射放在 `~/.gal/generated` 或供應商可見的 `.gal` 映射路徑下，而不是在 GAL source repo 中。
- [x] OQ-002 — 狀態收斂能改為漸進式，而不需要引入新的「待定狀態」概念嗎？或者說需要有明確的待定狀態？已由人類決策解決：在執行中的任務內部，狀態收斂可以被延後，但所有的持久狀態表面必須在記錄任務進度或完成的 git commit 之前同步。任何 committed 狀態都不能包含 `docs/plans/<slug>.md`、`.dev/plans/<slug>.prompt.md` 與 `.dev/state.md` 之間的分歧。
- [x] OQ-003 — OpenCode 是否揭露了可靠的 token 遙測，可以用於未來的迭代以取代啟發式的任務邊界估計？已解決：原生的 `opencode stats` 僅提供彙總資料，但 OpenCode 會在本地儲存原始工作階段資料 (SQLite/JSON)。社群工具 (如 CodeBurn) 已經展示對話層級的分析是可行的。因此，GAL 應該加入自己的階段標記與測量包裝器來解析本地工作階段資料，而不是僅依賴彙總資料，這樣才能將用量精確地關聯到 pipeline 任務邊界。

## Approval

- Human approval: [completed]
- Architect review: [not required for remaining T-006..T-010 validation/script-test tasks unless scope expands into protected paths]
- Additional domain review: [security review only if the implementation changes trust-boundary behavior]

## Review Results

### Engineering Review

#### Verdict: CLEAR

此計畫已準備好進行實作。範圍已縮小到剩餘的驗證與腳本強化任務，每項任務都能獨立測試，且只要保留在已獲批准的驗證表面內，尚未解決的工作就不需要重新開啟架構檢討。

目前的來源計畫已經包含了不得妥協的關鍵防護：同 runtime 降級必須保持明確、commit 邊界收斂必須在三個持久狀態表面中維持，以及生成的轉接器僅維持為衍生輸出。剩下的差距是在 `scripts/Test-PipelineTokenBurn.ps1` 中針對腳手架寫回、Bash 主機同等性與指令佈局驗證的受限執行檢查，而不是缺失設計決策。

2026-05-29 更新：`scripts/Test-PipelineTokenBurn.ps1` 現在能接受目前僅限 source 的 repo 佈局，當烤好的輸出不存在時，它會從 `commands/gal-pipeline/SKILL.md` 退回到 `commands/gal-pipeline/SKILL.template.md`。腳本通過了 55 項檢查，確認了當前可量測結果：啟動負載 `40914` bytes，單任務 3 階段載體負載 `42941` bytes，以及 5 任務載體負載 `51049` bytes。這彌平了當前 repo 狀態下 T-001 到 T-005 的舉證差距，同時將 Bash 主機 runtime 同等性留作獨立且受限於環境的後續追蹤。

2026-05-29 更新：T-010 已透過記錄為受限於主機環境而關閉。靜態檢查確認 Bash 分派器仍會發出精簡的 pipeline 中介資料欄位 (`CURRENT_TASK`、`TASK_BASE_COMMIT`、`TASK_FINAL_COMMIT`、`TEST_RETRY_COUNT`、`REVIEW_RETRY_COUNT`、`CONTEXT_CARRY`、`PIPELINE_CONTEXT_MODE`、`PIPELINE_CONTEXT_FILES`、`CONVENTION_HINTS`)，並且 Bash 轉接器生成器仍然會從 `.dev/project.md` 執行語言範圍的慣例選擇。由於在此 Windows 主機上透過 WSL 執行 `C:\WINDOWS\System32\bash.exe` 會因為缺少 `/bin/bash` 而失敗，因此 repo 中明確記錄了該限制，而並未聲稱有活的 Bash-lane 通過測試。

只有當剩餘的工作擴展超出驗證範圍、進入新的受保護路徑行為、更改跨 runtime 語義，或引入預設的綑綁執行模式時，才需要返回 `/deep-planning`。

<!-- ENG_REVIEW: CLEAR -->
