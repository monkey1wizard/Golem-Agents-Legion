# Plan: GAL Headless CLI Pipeline 互派測試 + 路由設定 JSON 化（含 per-role model）

## Goal

在 `headless-cli-pipeline.md`（已 VERIFIED、待釋出）的 headless executor 能力之上，交付兩件**有先後順序**的事：

1. **路由設定 JSON 化 + per-role model**（先做）：把 executor 路由設定從 NDJSON 改為 JSON，並在每個角色項加入 `model` 欄位。如此一份設定即同時承載 `role → executor` 與 `role → model`，成為 dispatch 路徑與未來 Rust 原生層的**單一機器讀取來源**，並讓「精準目標模型」有真正的注入路徑。
2. **Pipeline 互派測試矩陣**（後做）：以可重複、可觀測的方式驗證 Claude / Codex / Copilot / OpenCode 四環境間的 OFFLOAD / Fallback 分派行為，並一併驗證「Copilot / Codex / OpenCode 能否作為 Orchestrator 驅動 `/gal pipeline`」此一前提（OQ-003 即由本測試回答）。

> **單一設定來源決策**：model 併入路由 JSON，不另立 `model-config.json`，避免兩個 role-keyed 設定彼此漂移。`model-roles.md` 涵蓋的規劃側角色（ARCHITECT / ANALYST / DESIGNER / RESEARCHER…）非無頭執行對象，維持人類參考，不在本計畫遷移範圍。

## Requirements

- [ ] **路由設定 JSON 化**：`executor-routing.ndjson` 改為 JSON 格式，schema 為 role-keyed，每角色含 `executor` 與 `model`。`Read-ExecutorRouting` 改以 JSON 解析，回傳 `role → @{ executor; model }`；缺檔回 `$null`；malformed 不致命。
- [ ] **精準目標模型可交付**：指定模型能實際傳入次級 CLI——OpenCode `DeepSeek V4 Flash`、Claude Code `haiku-4.5`（codex / copilot 無無頭、不適用 executor model）。三支轉接器接受並傳遞 model。
- [ ] **播種對等**：`Update-Personalization`（PS 與 Bash）播種 JSON 路由範例到 `~/.gal/config/`，且既有 `config.local.env` 播種與 git smudge/clean 過濾流程不回歸。**處理既有 `.ndjson` 使用者的遷移**（見 Step 2）。
- [ ] **測試對象涵蓋度與排除一致性**：矩陣涵蓋四工具互為 Orchestrator / Executor；Copilot CLI 與 Codex 作為 Executor 一律降級為 `non-dispatchable`，與 `headless-cli-pipeline.md` 排除決定零衝突。
- [ ] **專案環境一致性**：啟動各 AI 工具時帶入與目前相同的 Project / 工作目錄（`Invoke-Executor.ps1 -WorkDir`），使歷史紀錄關聯同一專案。
- [ ] **可觀測足跡**：每格實際走法（offload / fallback / non-dispatchable）可由 `.dev/executor-logs/` 終態分類與執行 prompt `Dispatch:` 標記回查；log header 須能觀測到實際傳入的 model。

## Approach

### 階段一：路由設定 JSON 化 + per-role model（先行，零外部 CLI 依賴）

#### Step 1：JSON schema + 範例檔

- **Files**: `[NEW] executor-routing.example.json`（repo root，取代 `executor-routing.example.ndjson`）
- **What**: 定 role-keyed JSON schema，每角色含 `executor` 與 `model`。建議形如：

  ```json
  {
    "CODER":    { "executor": "claude",   "model": "haiku-4.5" },
    "TESTER":   { "executor": "opencode", "model": "DeepSeek V4 Flash" },
    "REVIEWER": { "executor": "claude",   "model": "haiku-4.5" },
    "VERIFIER": { "executor": "opencode", "model": "DeepSeek V4 Flash" }
  }
  ```

  `model` 可選；省略時轉接器使用該 CLI 預設模型。最終 schema 形狀於 `/refining-plan` 鎖定。
- **Verify**: 範例可被 `ConvertFrom-Json` / `jq` 解析；含分工註解（此檔為 dispatch 與 model 的單一來源，`model-roles.md` 僅人類參考）。

#### Step 2：reader + 播種（PowerShell + Bash）

- **Files**: `[MODIFY] scripts/common/Common.ps1`（`Read-ExecutorRouting`）、`[MODIFY] scripts/Update-Personalization.ps1`、`[MODIFY] scripts/update-personalization.sh`
- **What**:
  - `Read-ExecutorRouting` 由「逐行 NDJSON try-catch」改為「整檔 JSON 解析」，回傳 `role → @{ executor; model }`；malformed JSON 回 `$null` 不致命；缺檔回 `$null`。
  - 播種目標由 `executor-routing.ndjson` 改為 `executor-routing.json`（沿用 `xmachine.json` 的固定正規路徑、無 `.local` 中綴慣例，且 JSON 與該先例一致）。
  - **既有 `.ndjson` 遷移**：偵測 `~/.gal/config/executor-routing.ndjson` 存在但 `.json` 不存在時，至少給出明確遷移提示（或一次性轉換）；不靜默忽略舊檔（見 BUG-01）。
  - Bash 端 `update-personalization.sh` 對等播種 `.json`；Bash 執行器本身仍依 headless OE-01 延後。
- **Verify**: 乾淨 `~/.gal/config/` 下兩支腳本皆產出 `executor-routing.json`；`Read-ExecutorRouting` 對範例回正確 `role → @{executor; model}`；既有過濾流程不回歸；有舊 `.ndjson` 時有遷移路徑。

#### Step 3：三轉接器接受 model

- **Files**: `[MODIFY] scripts/executors/claude.ps1`、`scripts/executors/opencode.ps1`、`scripts/executors/agy.ps1`；視 dispatch 串接需要 `[MODIFY] scripts/executors/Invoke-Executor.ps1`、`scripts/gal.ps1`
- **What**: 轉接器新增 `-Model`，於無頭呼叫式帶入各 CLI 的模型旗標（claude `--model`、opencode `--model`、agy 待確認）。**model 旗標語法須以實測確認**（沿用 headless T-002 spike 模式，小範圍補確認）。`Invoke-Executor.ps1` / `gal.ps1` OFFLOAD 路徑把 `Read-ExecutorRouting` 取得的 model 透傳給轉接器。
- **Verify**: 設 CODER → `claude@haiku-4.5` 後，dispatch 實際以 `haiku-4.5` 啟動子行程，且 `.dev/executor-logs/` header 觀測得到該 model。

### 階段二：Pipeline 互派驗證（後做）

#### Step 4：設定路由 + 工作目錄一致性

- **Files**: `~/.gal/config/executor-routing.json`（使用者本機）
- **What**: 依各測試案例設定 `role → {executor, model}`，並確認 `-WorkDir` 帶入與目前相同的工作目錄。
- **Verify**: 任一格執行後，`.dev/executor-logs/` header 的 git branch/HEAD 與工作目錄與目前一致。

#### Step 5：執行測試矩陣

逐格檢查終端輸出、`.dev/executor-logs/<ts>-...log` 終態分類、執行 prompt `Dispatch:` 標記：

| 測試案例 | Orchestrator | 目標 Executor | 預期模式 | 預期終態 / 標記 |
| --- | --- | --- | --- | --- |
| **TC-01** | Copilot CLI | Claude | Offload | `completed` / `offload(executor=claude, receipt=ok)` |
| **TC-02** | Copilot CLI | OpenCode | Offload | `completed` / `offload(executor=opencode, receipt=ok)` |
| **TC-03** | Copilot CLI | Codex | Fallback | `non-dispatchable`（Codex 無無頭） |
| **TC-04** | Claude | Copilot CLI | Fallback | `non-dispatchable`（Copilot 無無頭） |
| **TC-05** | Claude | OpenCode | Offload | `completed` / `offload(executor=opencode, receipt=ok)` |
| **TC-06** | Claude | Codex | Fallback | `non-dispatchable` |
| **TC-07** | OpenCode | Copilot CLI | Fallback | `non-dispatchable` |
| **TC-08** | OpenCode | Claude | Offload | `completed` / `offload(executor=claude, receipt=ok)` |
| **TC-09** | OpenCode | Codex | Fallback | `non-dispatchable` |
| **TC-10** | Codex | Copilot CLI | Fallback | `non-dispatchable` |
| **TC-11** | Codex | Claude | Offload | `completed` / `offload(executor=claude, receipt=ok)` |
| **TC-12** | Codex | OpenCode | Offload | `completed` / `offload(executor=opencode, receipt=ok)` |

> **Orchestrator 能力即測試標的**（OQ-003 已解）：矩陣同時驗證「Copilot / Codex / OpenCode 能否驅動 `/gal pipeline`」。若某工具無法驅動，該結果記為環境不可驗證（env-unverifiable），屬有效測試輸出而非阻斷。codex / copilot 作為 Orchestrator 時其自身模型（`5.4-mini` / `MAI-Code-1-Flash`）由該互動工作階段設定，非腳本可控——只在 Orchestrator 角色有意義，不經路由 JSON 注入。

## Files to Create or Modify

- `[NEW] executor-routing.example.json` — role-keyed JSON 路由範例（含 `executor` + `model`），取代 `executor-routing.example.ndjson`
- `[MODIFY] scripts/common/Common.ps1` — `Read-ExecutorRouting` 改 JSON 解析、回 `role → @{executor; model}`
- `[MODIFY] scripts/Update-Personalization.ps1` — 播種改 `executor-routing.json`；處理舊 `.ndjson` 遷移；不動既有 `.env` 播種與 git 過濾名單
- `[MODIFY] scripts/update-personalization.sh` — Bash 對等
- `[MODIFY] scripts/executors/claude.ps1`、`opencode.ps1`、`agy.ps1` — 新增 `-Model` 並帶入無頭模型旗標
- `[MODIFY] scripts/executors/Invoke-Executor.ps1`、`scripts/gal.ps1` — OFFLOAD 路徑透傳 model

> 本計畫變更 `headless-cli-pipeline.md` 已交付的腳本面（routing 格式、reader、轉接器）。屬有意 supersede（NDJSON → JSON 與既有 `xmachine.json` 先例更一致），範圍受控於上列檔案。

## Success Criteria

- [ ] `Update-Personalization`（PS 與 Bash）在 `~/.gal/config/` 產出 `executor-routing.json`；既有 `config.local.env` 播種與 git 過濾流程不回歸；舊 `.ndjson` 使用者有遷移路徑。
- [ ] `Read-ExecutorRouting` 對 JSON 範例回正確 `role → @{executor; model}`；缺檔 `$null`；malformed 不致命。
- [ ] 指定模型可由 `.dev/executor-logs/` header 觀測到實際傳入次級 CLI（claude→haiku-4.5、opencode→DeepSeek V4 Flash）。
- [ ] **TC-01 至 TC-12 全部**執行，Offload / Fallback 與預期欄完全相符；codex / copilot 作為 executor 恆 `non-dispatchable`。
- [ ] 每格可由 `.dev/executor-logs/` 終態分類與 `Dispatch:` 標記回查；OQ-003（Orchestrator 能力）由矩陣結果作答。

## Risks

- **格式遷移斷層**：既有使用者持有 `executor-routing.ndjson`；切到 `.json` 後舊檔不被讀取。緩解：Step 2 偵測舊檔並提示/轉換，不靜默忽略。
- **CLI 模型旗標不穩定**：各 CLI 的 `--model` 旗標語法可能不同或變更。緩解：Step 3 比照 headless T-002 spike 先實測再寫入轉接器。
- **觸及已釋出腳本面**：改動 headless 計畫交付的 routing/reader/轉接器。緩解：變更受控於 Files 清單、最小化、保持 OFFLOAD 與降級語義不變。
- **Orchestrator 能力未知**：Copilot / Codex / OpenCode 驅動 pipeline 未經驗證。緩解：本身即測試標的，結果可記為 env-unverifiable。

## Open Questions

- [x] OQ-001 — 精準模型注入機制 *(raised by: deep-planning, resolved by: human)* → 於路由設定加 `model` 欄位，由轉接器 `-Model` 帶入。
- [x] OQ-002 — `model-config.json` 與 routing 的分工 / 漂移 *(raised by: deep-planning, resolved by: human)* → 不另立 `model-config.json`；model 併入路由 JSON，單一來源；`model-roles.md` 維持人類參考。
- [x] OQ-003 — Orchestrator 能否驅動 `/gal pipeline` *(raised by: deep-planning, resolved by: human)* → 由本測試矩陣回答，非前置阻斷。
- [x] OQ-004 — 是否拆計畫 *(raised by: deep-planning, resolved by: human)* → 不拆；階段一（改設定）先行，階段二（測試）後做。

## Approval

- Human approval: **approved**（2026-06-03）
- Architect review: **APPROVE**（2026-06-03；OQ-001~004 已解，BUG-01 阻斷由「model 併入單一路由 JSON」設計消除，餘項為受控施工注意，已入 Risks / Step）
- Additional domain review: [not requested]

## Review Results

### Architecture Review

#### Verdict: APPROVE *(2026-06-03，OQ 收斂後重審)*

四個 OQ 全數收斂後，先前阻斷項（精準模型無交付路徑）由設計消除：model 併入路由 JSON 成單一來源，dispatch 路徑與轉接器有明確注入點。方向受控、無過度抽象，APPROVE。

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| model 併入路由 JSON（單一來源） | 解 BUG-01 與雙設定漂移；dispatch 與 model 同源 | 觸及已釋出 routing/reader/轉接器 | OK（受控於 Files 清單） |
| NDJSON → JSON | 與 `xmachine.json` 先例一致；整檔解析比逐行簡單 | 既有 `.ndjson` 使用者需遷移 | OK（Step 2 處理遷移） |
| 不拆計畫、階段一先行 | 設定改完才有可測標的；零外部 CLI 依賴可先落地 | 單一計畫較長 | OK |
| `model-roles.md` 不遷移 | 規劃側角色非無頭對象，避免不必要範圍 | 兩處角色概念並存（執行 vs 規劃） | OK（已界定範圍） |

#### Bug Surface

- **[BUG-01] Medium**：格式由 NDJSON 改 JSON 後，既有 `executor-routing.ndjson` 使用者的舊檔不被新 reader 讀取，造成靜默失效（degrade 回文字分派而不自知）。Fix：Step 2 偵測舊 `.ndjson` 存在且 `.json` 缺檔時明確提示或一次性轉換。已入 Requirements / Risks。
- **[BUG-02] Low**：轉接器 `-Model` 帶入錯誤旗標名會讓次級 CLI 啟動失敗。Fix：Step 3 先 spike 確認各 CLI 模型旗標再寫入。
- **[BUG-03] Low（已修）**：原稿 Success Criteria 截斷為 TC-01~07，已更正為 TC-01~12 全覆蓋。

#### Missing from Plan

- **codex/copilot 的 executor model 為空要求**：二者恆 `non-dispatchable`，不經路由 JSON 注入 model；其模型只在 Orchestrator 角色由互動工作階段設定。已於矩陣註記釐清，路由 JSON 範例不含 codex/copilot 的 executor model。

#### Recommended Changes

1. 階段一完成後再進階段二（已排序）。
2. `/refining-plan` 時鎖定 routing JSON 最終 schema 形狀（role-keyed map vs routes 陣列）。
3. Step 3 的 model 旗標 spike 結果記入 `## Tasks` 或實作註解，供三轉接器一致採用。

#### What's Good (keep these)

- **單一設定來源**徹底消除雙 role-keyed 設定漂移風險。
- **與既有排除決定一致**：codex/copilot 作 executor 恆 `non-dispatchable`，未擴張不可能的無頭通道。
- **Orchestrator 能力作為測試標的**：把未知轉為可觀測輸出，而非假設或阻斷。
- **NDJSON → JSON 對齊 `xmachine.json` 先例**：降低設定面異質性。

### Engineering Review

#### Verdict: CLEAR *(2026-06-03)*

可建構性確認。依賴鏈清晰、無循環：

- **階段一基礎**：T-001（JSON schema/範例，鎖定 role-keyed map 形狀）→ T-002（`Read-ExecutorRouting` 改 JSON 解析）、T-003（播種 + 舊 `.ndjson` 遷移，PS+Bash）皆依 T-001 的 schema。
- **model 注入鏈**：T-004（三 CLI model 旗標 spike，不寫產品碼）前置於 T-005（三轉接器 `-Model`）；T-006（`Invoke-Executor` / `gal.ps1` OFFLOAD 透傳 model）依賴 T-002 reader 輸出 + T-005 轉接器介面。
- **階段二**：T-007（設定路由 + workdir 一致性，測試前置）依賴階段一全數完成；T-008（執行 TC-01~12）依賴 T-007。

受控範圍提醒：T-002 / T-005 / T-006 觸及 `headless-cli-pipeline.md` 已釋出腳本面（`Common.ps1`、`scripts/executors/*`、`gal.ps1`），屬架構審查放行的有意 supersede；實作須最小化、保持 OFFLOAD 與降級語義不變，不得擴大到其他分派路徑。

降級與排除一致性不受影響：缺 `.json` / 缺角色 / Exit 2 三條降級維持；codex / copilot 作 executor 恆 `non-dispatchable`，model 不經路由 JSON 注入。BUG-01（舊 `.ndjson` 靜默失效）由 T-003 遷移處理；BUG-02（model 旗標）由 T-004 spike 前置。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | unit | `executor-routing.example.json` 可被 `ConvertFrom-Json` / `jq` 解析；role-keyed map，每角色含 `executor`，`model` 可選；不含 codex/copilot 的 executor model。 | T-001 |
| TP-002 | unit | `Read-ExecutorRouting` 對 JSON 範例回 `role → @{executor; model}`；缺檔回 `$null`；malformed JSON 不致命（回 `$null`）；`model` 缺項時該角色 `model` 為 `$null`。 | T-002 |
| TP-003 | integration | 乾淨 `~/.gal/config/` 下執行 `Update-Personalization`（PS 與 Bash）產出 `executor-routing.json`；既有 `config.local.env` 播種與 git smudge/clean 過濾流程不回歸。 | T-003 |
| TP-004 | integration | `~/.gal/config/` 已有舊 `executor-routing.ndjson` 且無 `.json` 時，播種給出明確遷移提示或一次性轉換；不靜默忽略舊檔。 | T-003 |
| TP-005 | manual | 三 CLI（claude / opencode / agy）的無頭模型旗標語法已實測記錄（或標環境不可驗證）；供 T-005 一致採用。 | T-004 |
| TP-006 | integration | 裝了 Claude 時 `Invoke-Executor -Executor claude -Model haiku-4.5` 啟動子行程並以該 model 執行，`.dev/executor-logs/` header 觀測得到 model；OpenCode 同理 `DeepSeek V4 Flash`。 | T-005 |
| TP-007 | integration | 設 CODER → `claude@haiku-4.5` 時，`gal.ps1` OFFLOAD 區塊/`Invoke-Executor` 把 model 透傳給轉接器；未指定 model 時轉接器用 CLI 預設。 | T-006 |
| TP-008 | manual | 測試前置：`executor-routing.json` 依案例設定 `role → {executor, model}`，`-WorkDir` 帶入與目前相同工作目錄；`.dev/executor-logs/` header 的 git branch/HEAD 與目前一致。 | T-007 |
| TP-009 | integration | TC-01~12 全矩陣：Offload 格出 `completed` / `offload(executor=..., receipt=ok)`；Fallback 格出 `non-dispatchable`；逐格與預期欄相符；OQ-003 Orchestrator 能力由結果作答（不可驅動者記 env-unverifiable）。 | T-008 |

## Tasks

- [x] T-001 — 建立 `executor-routing.example.json` *(4b57439aa1d1050fd206f3ad7987185abe7d71e3)*（repo root，取代 `executor-routing.example.ndjson`）：**鎖定 role-keyed map schema**（`{ "CODER": { "executor": "claude", "model": "haiku-4.5" }, ... }`），`model` 可選；含分工註解（此檔為 dispatch + model 單一來源，`model-roles.md` 僅人類參考）；範例不含 codex/copilot 的 executor model。
- [x] T-002 — 修改 `scripts/common/Common.ps1` 的 `Read-ExecutorRouting` *(b4a7102bce55bdfd32297eeb8242c9e8566b2f0b)*：由逐行 NDJSON try-catch 改為整檔 JSON 解析，回傳 `role → @{ executor; model }`；缺檔回 `$null`；malformed JSON 回 `$null` 不致命；`model` 缺項回 `$null`。
- [x] T-003 — 修改 `scripts/Update-Personalization.ps1` 與 `scripts/update-personalization.sh` *(6ea5055a6e5dd179e660b63b5b1ba2d12960e9a9)*：播種目標由 `executor-routing.ndjson` 改為 `executor-routing.json`；偵測既有 `~/.gal/config/executor-routing.ndjson` 存在但 `.json` 缺檔時給明確遷移提示或一次性轉換；**不動**既有 `config.local.env` 播種與 git smudge/clean `trackedFilterFiles` 名單。
- [x] T-004 — CLI 無頭模型旗標 spike *(5552d0f0e805752d3e23a6e11e62d60fc051364c)* — claude: `--model`; opencode: `-m provider/model`; agy: env-unverifiable：實測確認 claude / opencode / agy 的無頭模型旗標語法（claude `--model`、opencode `--model`、agy 待確認），記錄為 T-005 依據；不寫產品碼。不可用者標環境不可驗證。
- [x] T-005 — 修改 `scripts/executors/claude.ps1`、`opencode.ps1`、`agy.ps1` *(f25e3e6de6db2ea30d8233beab1f13b42097ae7c)*：新增 `-Model` 參數，依 T-004 語法帶入無頭模型旗標；未指定時用 CLI 預設；可用性檢查與逾時邏輯不變。
- [x] T-006 — 修改 `scripts/executors/Invoke-Executor.ps1` 與 `scripts/gal.ps1` *(2be03159c009e779666276e5741e4ee04828d06b)*：OFFLOAD 路徑把 `Read-ExecutorRouting` 取得的 model 透傳給轉接器（`-Model`）；`.dev/executor-logs/` header 記錄實際傳入 model；保持 OFFLOAD 與降級語義不變。
- [x] T-007 — 測試前置 *(完成於 TC-05 執行前)*：`~/.gal/config/executor-routing.json` 按 TC 案例設定；`-WorkDir C:\Code\Golem-Agents-Legion` 補入 OFFLOAD ACTION（hotfix commit）；`opencode.json` 加 `default_agent: build` 覆蓋 global plan-mode 限制；working model for opencode = `opencode/minimax-m3-free`。
- [ ] T-008 — 執行 TC-01~12 互派矩陣（進行中）：TC-05（Claude→OpenCode）PASS。其餘 11 格待執行。

> **Deferred**: Bash 執行器（`invoke-executor.sh`、`claude.sh`、`opencode.sh`、`agy.sh` 對應 `-Model`、`gal.sh` 透傳）延後至有 Bash 主機可驗證時（沿用 `headless-cli-pipeline.md` OE-01）。本計畫 Bash 僅交付 T-003 的播種對等。
