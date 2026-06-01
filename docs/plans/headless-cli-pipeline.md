# Plan: GAL Headless CLI Pipeline Orchestration

## Goal

讓 `/gal pipeline` 的各階段（implement / test / review / verify）能改由**本機背景的外部 AI CLI 以無頭（headless）模式執行**，而非全部由對話迴圈中的同一個模型自己接手。每個階段在自己的子行程、自己的 context 中跑，達成兩個目標：(1) 各階段 token 消耗隔離；(2) 真正的跨模型驗證（implement / test / review 可走不同 CLI）。

範圍刻意收斂為**本機 headless 一種執行模式**。任何「本機 ↔ 遠端 SSH」的跨執行模式統一抽象**不在本計畫範圍**，留待控制平面原生化（Rust）時於共用核心層處理。

## Requirements

- [ ] `gal.ps1` 在 Pipeline 分派時，能讀取 `~/.gal/config/executor-routing.ndjson`，依「階段 → 角色 → executor CLI」解析出該階段要呼叫的 CLI 名稱。`model-roles.local.md` 保留為人類可讀的參考文件，不作為腳本的機器解析來源。
- [ ] 腳本層持有「階段 → 角色」對照（implement→CODER、test→TESTER、review→REVIEWER、verify→VERIFIER）；目前此對照只存在於 SKILL 散文，需落到可解析的腳本層。
- [ ] 針對每個支援無頭模式的 CLI（Claude Code、OpenCode、Gemini CLI），存在一個薄的 executor 轉接腳本，並有一支統一的本機入口 `Invoke-Executor.ps1`。
- [ ] 分派實作任務前，能產生一份精簡、自足的 Task Spec 檔案，控制送入次級 CLI 的上下文大小（目標 < 5KB）。
- [ ] 當 CLI 不存在、不可用或逾時，系統優雅降級回現有的對話文字分派模式（`--- GAL DISPATCH ---`）。
- [ ] 次級 CLI 執行後，主 Pipeline 必須以**讀取檔案內容**確認結果已寫入預定持久化路徑（如 `## Test Results`、`## Review Results`），才判定階段完成——不得只看 Exit Code。
- [ ] 既有安全模型（重試上限、受保護路徑升級、有條件資安審查、commit 邊界收斂關卡）不受影響；次級 CLI **不得跨越 commit 邊界**，commit 由 orchestrator 持有。
- [ ] 先交付 PowerShell 通道；Bash 通道延後（見 Tasks 末 Deferred 與 OE-01）。

## Approach

### Step 1：路由設定 + 階段→角色對照

- **Files**: `executor-routing.example.ndjson`（repo root）、`scripts/common/Common.ps1`
- **What**:
  - 建立 `executor-routing.example.ndjson` 範例於 **repo 根目錄**（與 `model-roles.example.md`、`config.example.env` 同處），由 Setup-Machine 播種到 `~/.gal/config/executor-routing.ndjson`。每行一個 JSON 物件，例如 `{"role":"CODER","executor":"claude"}`。
  - 在 `Common.ps1` 新增 `Read-ExecutorRouting`：讀 `~/.gal/config/executor-routing.ndjson`，回傳 hashtable（如 `@{ CODER = "claude"; TESTER = "opencode" }`）；檔案不存在回 `$null`（不報錯）；malformed 行逐行 `try-catch` 跳過。
  - 在腳本層新增「階段 → 角色」對照（implement→CODER、test→TESTER、review→REVIEWER、verify→VERIFIER）。
- **Verify**: `Read-ExecutorRouting` 對 3 行 NDJSON 回正確 hashtable；缺檔回 `$null`；malformed 行被跳過。

### Step 2：CLI 無頭語法 spike（先驗證、再實作）

- **Files**: 無產品碼（結果記入本計畫或實作註解）
- **What**: 在建任何轉接器之前，先實測確認三個 CLI 的無頭呼叫式與 **stdin 餵入行為**，避免把假設寫進三個轉接器：
  - Claude Code：`claude -p` + bypass-permission 旗標的實際名稱與輸出格式。
  - OpenCode：`opencode run` 無頭/自動核准的實際旗標。
  - Gemini CLI：`gemini -p` 的實際行為。
  - 確認每個 CLI 能否從 **stdin** 讀取 prompt（而非把整份 spec 當命令列參數）。
- **Verify**: 三個 CLI 的確認呼叫式 + stdin 行為已記錄；不可用者明確標記為「環境不可驗證」。

### Step 3：本機統一執行器 `Invoke-Executor.ps1`

- **Files**: `scripts/executors/Invoke-Executor.ps1`
- **What**: 本機唯一入口。參數：`-Executor`（CLI 名稱）、`-TaskSpecPath`、`-WorkDir`、`-TimeoutMinutes`。**不含 `-Transport`**（local-only）。Exit Code：`0` 成功、`1` 任務失敗、`2` CLI 不可用或逾時。內建 `echo` mock executor 供測試。spec 一律經 **stdin** 餵入次級 CLI，不當命令列參數（避免引號/`$`/backtick/換行與命令長度問題）。逾時以 `taskkill /T` 或 Job object **回收整個行程樹**（claude/node 會 spawn 子行程，單純 `Stop-Process` 會留孤兒）。
- **Verify**: `Invoke-Executor -Executor echo` → Exit 0；`-Executor nonexistent` → Exit 2；逾時 mock → Exit 2 且無孤兒行程。

### Step 4：三個 executor 轉接器

- **Files**: `scripts/executors/claude.ps1`、`scripts/executors/opencode.ps1`、`scripts/executors/gemini.ps1`
- **What**: 每個轉接器把統一介面翻成對應 CLI 的無頭指令（用 Step 2 實測的語法），預設 bypass-permission 以避免無頭時卡權限提示：
  - 呼叫前 `Get-Command` 檢查 CLI 是否存在，不存在回 Exit 2。
  - 逾時邏輯同 Step 3（行程樹回收）。
  - **權威結果 = 就地檔案寫回**；CLI 的 stdout（含任何 JSON 輸出）只作成功/失敗訊號，不作結果來源。
  - **Codex CLI 刻意排除**：不支援無頭/非互動執行，無等效旗標。
  - **安全警告**：bypass-permission 等同完全信任次級 CLI 對檔案系統與終端機的存取。此警告須寫入 SKILL 與 `docs/personalization.md`（見 Step 6、Step 7）。
- **Verify**: 裝了對應 CLI 時 `Invoke-Executor -Executor <cli>` 能啟動子行程並就地寫回；未裝回 Exit 2。

### Step 5：Task Spec 產生器

- **Files**: `scripts/common/New-TaskSpec.ps1`
- **What**: 從活躍執行 prompt（`.dev/plans/<slug>.prompt.md`）為指定 `T-NNN` 任務組裝自足 Markdown spec：
  - 任務目標（對應 `## Tasks` 行）、受影響檔案（篩自 `## Files to Create or Modify`）、當前 git branch/HEAD。
  - 寫回路徑指示（告訴次級 CLI 結果要寫到哪個檔案的哪個 section）。
  - 語言慣例提示——**重用既有 `Get-PipelineDispatchMetadata` 的 `CONVENTION_HINTS` 機制**（`gal.ps1`），不另立平行邏輯。
  - Agent 合約路徑（如 `agent/golem-implementer.agent.md`）。
  - **明令次級 CLI 不得 git commit/push**（commit 邊界由 orchestrator 持有）。
  - 輸出 `.dev/task-specs/T-NNN-<phase>.md`，視為暫態控制平面產出物，含 Pipeline 結束時的清理邏輯。
- **Verify**: 對含 3 個 Task 的 prompt 執行 `New-TaskSpec -TaskScope T-001 -Phase implement`，輸出 < 5KB，含目標/受影響檔案/寫回路徑/「不得 commit」指示；Pipeline 結束後 `.dev/task-specs/` 被清理。

### Step 6：整合進分派器（OFFLOAD 模式，dispatch 不 spawn）

- **Files**: `scripts/gal.ps1`
- **What**: `gal.ps1 dispatch` 現況一律輸出文字區塊、從不啟動子行程，且既有的執行器慣例（OFFLOAD）就是「dispatch 輸出指示 → 對話 AI 另跑執行器腳本」。本步比照該慣例：
  - 在 `--pipeline-phase` 路徑，用「階段→角色」+ `Read-ExecutorRouting` 解析 executor。
  - 解析到 executor → 輸出 **OFFLOAD 區塊**，內含中介資料 + 指示對話 AI 執行 `Invoke-Executor.ps1 -Executor <cli> -TaskSpecPath <spec> -Wait`；並標明：exit 0 → 驗證寫回；exit 2 → 退回在本對話扮演該 golem。**dispatch 本身不 spawn**。
  - 無 routing 檔、或該角色無對應 executor、或 exit 2 → 維持現有文字分派（`--- GAL DISPATCH ---`），完全向後相容。
- **Verify**: `executor-routing.ndjson` 設 CODER=claude 時，分派 implement 階段會輸出「執行 Invoke-Executor.ps1」的 OFFLOAD 指示（而非 dispatch 自己 spawn）；移除 routing 檔後退回 `--- GAL DISPATCH ---`。

### Step 7：Pipeline 合約（SKILL）變更——受保護路徑

- **Files**: `commands/gal-pipeline/SKILL.template.md`（**受保護路徑，承重變更**）
- **What**: 在 `### Same-Runtime Fallback Contract` 之後、`### Runtime Step-Budget Preflight` 之前新增 `### Headless Executor Dispatch` 段，描述：
  - routing 存在時走 OFFLOAD；降級條件與行為。
  - **OFFLOAD 成功時 orchestrator 不得再自己扮演該 golem**，改為驗證子行程的就地寫回；exit 2 才退回扮演。
  - **次級 CLI 不得跨越 commit 邊界**，commit 由 orchestrator 持有（既有 commit-gate 不變）。
  - **bypass-permission 醒目安全警告**：等同完全信任次級 CLI，僅在受信任本機環境啟用。
  - Task Spec 的暫態性質。
- **Verify**: 靜態審查確認上述語意齊備、安全關卡未削弱、bypass-permission 警告醒目、受保護路徑變更已標明。

### Step 8：Executor smoke test 與設定文件

- **Files**: `scripts/executors/Test-Executor.ps1`、`docs/personalization.md`、`model-roles.md`、`model-roles.example.md`
- **What**:
  - `Test-Executor.ps1`：接受 `-Executor`，對指定 CLI 跑極簡 echo 任務，驗證可用性、無頭能力與 Exit Code。
  - `docs/personalization.md`：新增 executor-routing 設定指引 + bypass-permission 安全警告。
  - `model-roles.md` / `model-roles.example.md`：註記 `executor-routing.ndjson` 為腳本的機器讀取來源，Markdown 表格僅人類參考。
- **Verify**: `Test-Executor -Executor echo` 正確回報；文件含設定指引與安全警告。

## Files to Create or Modify

- `[NEW] executor-routing.example.ndjson` — 路由設定範例（**repo root**，由 Setup-Machine 播種到 `~/.gal/config/`）
- `[NEW] scripts/executors/Invoke-Executor.ps1` — 本機統一執行器入口（含 echo mock）
- `[NEW] scripts/executors/claude.ps1` — Claude Code 轉接器
- `[NEW] scripts/executors/opencode.ps1` — OpenCode 轉接器
- `[NEW] scripts/executors/gemini.ps1` — Gemini CLI 轉接器
- `[NEW] scripts/executors/Test-Executor.ps1` — executor smoke test
- `[NEW] scripts/common/New-TaskSpec.ps1` — Task Spec 產生器
- `[MODIFY] scripts/common/Common.ps1` — 新增 `Read-ExecutorRouting` 與「階段→角色」對照
- `[MODIFY] scripts/gal.ps1` — `--pipeline-phase` 路徑：解析 routing、產生 Task Spec、輸出 OFFLOAD 指示；無 routing 降級
- `[MODIFY] commands/gal-pipeline/SKILL.template.md` — 新增 Headless Executor Dispatch 段（受保護路徑承重變更，含安全警告與 orchestrator 行為改變）
- `[MODIFY] docs/personalization.md` — executor-routing 設定指引與安全警告
- `[MODIFY] model-roles.md` — 說明 `executor-routing.ndjson` 為腳本讀取來源
- `[MODIFY] model-roles.example.md` — 引導同步維護 `executor-routing.ndjson`

## Success Criteria

- [ ] 已安裝對應 CLI 的機器上執行 `/gal pipeline` 時，implement 階段能透過 OFFLOAD 指示啟動本機 CLI 子行程在背景完成工作並就地寫回。
- [ ] 未安裝任何外部 CLI、或無 routing 檔的機器上，`/gal pipeline` 退回原有對話文字分派模式，行為完全不變。
- [ ] Task Spec 穩定 < 5KB，次級 CLI 不需額外讀 `.dev/project.md` 或生成的轉接器。
- [ ] 重試上限、受保護路徑升級、有條件資安審查、commit 邊界收斂關卡，在 headless 與降級模式下均正常；次級 CLI 不會跨越 commit 邊界。
- [ ] 結果判定一律以讀取檔案內容確認寫回為準，不只依賴 Exit Code。

## Risks

- **CLI 參數不穩定**：廠商 CLI 可能改無頭旗標語法。緩解：轉接器設計為薄、獨立模組；Step 2 先 spike 確認再實作。
- **無頭授權死鎖**：CLI 可能因 API key 過期或需登入卡住等待輸入。緩解：嚴格 Timeout + 行程樹回收 + 降級。
- **Task Spec 不夠自足**：spec 缺上下文會讓次級 CLI 自行大量讀檔，反增 token。緩解：spec 含足夠寫回路徑與 agent 合約指示。
- **寫回路徑不一致**：次級 CLI 是不受控外部行程，可能不遵守寫回指示，導致主 Pipeline 卡在驗證。緩解：驗證讀檔案內容（section 是否出現/變動），不只看 Exit Code。
- **bypass-permission 安全影響**：次級 CLI 有完整檔案系統與終端機存取，惡意或有缺陷的 agent 合約可能造成非預期刪改/執行。緩解：文件醒目警告 + 使用者自主啟用 + spec 明令不得 commit。

## Open Questions

- [x] OQ-001 — Provider/executor 路由設定來源？— **已解決**：引入 `~/.gal/config/executor-routing.ndjson` 作為腳本讀取的權威來源（每行一 JSON 物件 `{"role":"CODER","executor":"claude"}`）；`model-roles.local.md` 保留為人類可讀參考。*(resolved by: human decision)*
- [x] OQ-002 — bypass-permission 在 Pipeline 中是否可接受？— **已解決**：預設使用 bypass-permission；此決定須在 SKILL 與 `docs/personalization.md` 以醒目方式警告使用者。*(resolved by: human decision)*
- [x] OQ-003 — 是否與遠端 SSH 執行共用單一 `-Transport` 介面？— **已解決：不共用**。本計畫只做本機 headless（無 `-Transport`）。理由：本機 headless（就地寫回）與遠端 SSH 執行器（patch-first、遠端永不 commit）收斂模型相反、無共用具體碼；在 PowerShell 層硬合併＝一介面掛兩套互斥實作。跨執行模式統一抽象移交控制平面原生化（Rust）的 native-logic backlog 處理，不在本計畫範圍。*(resolved by: human decision)*
- [x] OQ-004 — 「派工目標 CLI」如何命名以免與既有「安裝宿主 provider」語意（`ProviderPlugin.ps1`、`Build-ProviderPlugins.ps1`、`~/.gal/dist/providers/`、`compatibleProviders`/`providerCapabilities`，集合 {claude,copilot,codex,agy}）混淆？— **已解決**：採 `executor` 語彙（`scripts/executors/`、`Invoke-Executor.ps1`、`executor-routing.ndjson`、`-Executor`）。既有 provider 子系統不動；兩概念字面分離，未來併入原生控制平面時也不再撞名。*(resolved by: human decision；trivially reversible find-replace if changed)*

<!-- Format: - [ ] OQ-NNN — description *(raised by: command)* -->

## Approval

- Human approval: [pending]
- Architect review: [APPROVE — 2026-06-01；local-only headless executor，命名 executor，OFFLOAD 模式，受保護路徑變更獨立成項，F-01..F-11 全數收進 Tasks]
- Additional domain review: [not requested]

## Review Results

### Architecture Review

#### Verdict: APPROVE *(2026-06-01)*

收斂後方向清楚且範圍受控：只做本機 headless、走既有 OFFLOAD 分派慣例、不引入跨執行模式抽象。先前 deep re-think 的 11 項發現已全數在決策或 Tasks 中收斂（見下）。

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| 命名採 `executor`（不沿用 provider） | 與既有「安裝宿主 provider」子系統字面分離，維護者不需靠 context 猜語意 | 計畫文字 find-replace（實作未開始，零代價） | OK |
| 只做 local，不做 `-Transport` 統一 | 範圍最小、無過度抽象；避免一介面掛兩套互斥實作 | 跨執行模式統一延後到 Rust 原生層 | OK |
| 走 OFFLOAD 分派慣例（dispatch 不 spawn） | 與既有執行器分派模型一致；維持統一文字分派合約 | orchestrator 需新增「驗證寫回 vs 自行扮演」分支邏輯 | OK |
| 每個 CLI 一個薄轉接器 + 統一入口 | 隔離廠商 CLI 變化；單點維護 | 多個小檔 | OK |
| 預設 bypass-permission | 無頭必須避免卡權限提示 | 完全信任次級 CLI；以文件警告 + 自主啟用 + 禁止 commit 緩解 | OK |
| 降級回文字分派作為 fallback | 完全向後相容、不強制安裝額外 CLI | 降級路徑長期可能成為逃逸口、headless 得不到充分測試 | OK（可接受，已記錄） |

#### 已收斂的 deep re-think 發現

- **F-01 命名**：OQ-004 決議採 executor，字面分離既有 provider 子系統。
- **F-02 transport 過度抽象**：OQ-003 決議只做 local，統一抽象移交 Rust 原生層 backlog。
- **F-03 dispatch 不 spawn**：Step 6 改走 OFFLOAD 指示模式，與既有執行器慣例一致。
- **F-04 受保護路徑承重變更**：Step 7 / T-009 獨立成項，明列 orchestrator 行為改變與 commit 邊界。
- **F-05 範例檔落點**：移至 repo root，Setup-Machine 播種。
- **F-06 spec 傳遞**：改 stdin，不當命令列參數。
- **F-07 結果通道**：權威結果＝就地檔案寫回；stdout 僅訊號。
- **F-08 逾時行程樹**：`taskkill /T` 或 Job object 回收整樹。
- **F-09 CLI 語法假設**：Step 2 先 spike 再實作。
- **F-10 階段→角色對照**：T-001 落到腳本層。
- **F-11 commit 邊界**：spec 明令次級 CLI 不得 commit；orchestrator 持有。

#### Bug Surface

- **[BUG-01] Medium**：次級 CLI 是不受控外部行程，可能忽略寫回路徑指示。Fix：結果驗證讀檔案內容（對應 section 是否出現/變動），不只 Exit Code。已入 Requirements 與 Step 6/Step 4 Verify。
- **[BUG-02] Low**：`executor-routing.ndjson` malformed 行。Fix：逐行 `try-catch` parse 跳過（T-001）。

#### What's Good

- **Task Spec 隔離**控制送入次級 CLI 的上下文，是解 token 燃燒的正確方法。
- **優雅降級**讓系統在任何環境（含無外部 CLI）行為不變。
- **OFFLOAD 慣例對齊**讓 headless 執行與既有分派模型共享同一條路徑語意。

### Engineering Review

#### Verdict: CLEAR

可建構性：T-001（路由+對照）與 T-003（統一入口）為基礎；T-002（spike）前置於 T-004~006（轉接器）；T-005（task spec）獨立；T-008（整合）依賴 T-001+T-003+T-005；T-009（SKILL）依賴整合語意；T-007/T-010/T-011 獨立。依賴鏈清晰、無循環。

受保護路徑提醒：T-009 改 `commands/gal-pipeline/SKILL.template.md`（受保護路徑），已由架構審查放行並獨立成任務；實作不得擴大到其他 `commands/` 內容。

降級路徑完整：無 routing 檔 / 角色無 executor / Exit 2（不可用或逾時）三條皆退回 `--- GAL DISPATCH ---`。安全模型不受影響：重試上限、受保護路徑升級、有條件資安審查、commit 邊界收斂關卡均在主 Pipeline 層，且 spec 明令次級 CLI 不得 commit。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | unit | `Read-ExecutorRouting` 解析 3 行 NDJSON 回正確 hashtable；缺檔回 `$null`；malformed 行跳過續行。「階段→角色」對照回正確角色。 | T-001 |
| TP-002 | manual | Step 2 語法 spike：三個 CLI 的無頭呼叫式 + stdin 行為已記錄（或標記環境不可驗證）。 | T-002 |
| TP-003 | unit | `Invoke-Executor -Executor echo` → Exit 0；`-Executor nonexistent` → Exit 2；逾時 mock → Exit 2 且無孤兒行程。 | T-003 |
| TP-004 | integration | 裝了 Claude Code 時 `Invoke-Executor -Executor claude -TaskSpecPath <echo-spec>` 啟動子行程並就地寫回；未裝回 Exit 2。 | T-004 |
| TP-005 | integration | OpenCode 同 TP-004。 | T-005 |
| TP-006 | integration | Gemini CLI 同 TP-004。 | T-006 |
| TP-007 | unit | `New-TaskSpec -TaskScope T-001 -Phase implement` 對含 3 Task 的 prompt 產出 < 5KB，含目標/受影響檔案/寫回路徑/「不得 commit」指示；Pipeline 結束 `.dev/task-specs/` 清理。 | T-007 |
| TP-008 | integration | `executor-routing.ndjson` 設 CODER=claude 時，`gal.ps1` 分派 implement 輸出「執行 Invoke-Executor.ps1」OFFLOAD 指示（dispatch 不自行 spawn）；移除 routing → 退回 `--- GAL DISPATCH ---`。 | T-008 |
| TP-009 | manual | 靜態審查 `SKILL.template.md`：OFFLOAD 成功時 orchestrator 不自扮演、次級 CLI 不得 commit、降級語義、bypass-permission 警告皆在；受保護路徑變更已標明。 | T-009 |
| TP-010 | integration | `Test-Executor -Executor echo` 正確回報可用性與 Exit Code。 | T-010 |
| TP-011 | manual | `docs/personalization.md`、`model-roles.md`/`model-roles.example.md` 含 executor-routing 設定指引與 bypass-permission 安全警告。 | T-011 |

## Tasks

- [ ] T-001 — 建立 `executor-routing.example.ndjson`（repo root，每行 `{"role":"CODER","executor":"claude"}`）；在 `scripts/common/Common.ps1` 新增 `Read-ExecutorRouting`（讀 `~/.gal/config/executor-routing.ndjson` → hashtable，缺檔回 `$null`，malformed 行 try-catch 跳過）與「階段→角色」對照（implement→CODER、test→TESTER、review→REVIEWER、verify→VERIFIER）。
- [ ] T-002 — CLI 無頭語法 spike：實測確認 Claude Code / OpenCode / Gemini 的無頭呼叫式與 stdin 餵入行為，記錄為 T-004~006 依據。不寫產品碼。
- [ ] T-003 — 建立 `scripts/executors/Invoke-Executor.ps1` 本機統一入口：`-Executor`、`-TaskSpecPath`、`-WorkDir`、`-TimeoutMinutes`（**無 `-Transport`**）。Exit 0/1/2。內建 echo mock。spec 經 stdin 餵入。逾時用 `taskkill /T` 或 Job object 回收整個行程樹。
- [ ] T-004 — 建立 `scripts/executors/claude.ps1`：`Get-Command claude` 檢查；依 T-002 語法以 stdin 餵 spec 呼叫 claude（bypass-permission）；權威結果＝就地寫回、stdout 僅訊號；逾時/不存在回 Exit 2。
- [ ] T-005 — 建立 `scripts/executors/opencode.ps1`：依 T-002 語法呼叫 OpenCode（非互動自動核准）；可用性檢查與逾時同 T-004。
- [ ] T-006 — 建立 `scripts/executors/gemini.ps1`：依 T-002 語法呼叫 Gemini CLI；可用性檢查與逾時同 T-004。
- [ ] T-007 — 建立 `scripts/common/New-TaskSpec.ps1`：從 `.dev/plans/<slug>.prompt.md` 提取指定 T-NNN 的目標、受影響檔案、git branch/HEAD、寫回路徑指示、Agent 合約路徑；慣例提示**重用 `Get-PipelineDispatchMetadata` 的 CONVENTION_HINTS**；spec 內**明令次級 CLI 不得 git commit/push**；輸出 `.dev/task-specs/T-NNN-<phase>.md`（< 5KB）；含 Pipeline 結束清理。
- [ ] T-008 — 修改 `scripts/gal.ps1`：在 `--pipeline-phase` 路徑用「階段→角色」+ `Read-ExecutorRouting` 解析 executor。解析到 → 輸出 **OFFLOAD 區塊**指示對話 AI 執行 `Invoke-Executor.ps1 -Executor <cli> -TaskSpecPath <spec> -Wait`（標明 exit 0→驗證寫回、exit 2→退回扮演）；**dispatch 不自行 spawn**。無 routing/無對應 executor → 維持現有文字分派。
- [ ] T-009 — 修改 `commands/gal-pipeline/SKILL.template.md`（**受保護路徑**）：在 `### Same-Runtime Fallback Contract` 後、`### Runtime Step-Budget Preflight` 前新增 `### Headless Executor Dispatch` 段：OFFLOAD 觸發與降級語義；**OFFLOAD 成功時 orchestrator 改為驗證寫回、不自扮演 golem，exit 2 才扮演**；**次級 CLI 不得跨越 commit 邊界，commit 由 orchestrator 持有**；bypass-permission 醒目安全警告；Task Spec 暫態性質。
- [ ] T-010 — 建立 `scripts/executors/Test-Executor.ps1` smoke test：`-Executor` 對指定 CLI 跑極簡 echo，驗證可用性、無頭能力、Exit Code。
- [ ] T-011 — 更新文件：`docs/personalization.md` 新增 executor-routing 設定指引 + bypass-permission 安全警告；`model-roles.md` 與 `model-roles.example.md` 註記 `executor-routing.ndjson` 為腳本機器讀取來源、Markdown 表格僅人類參考。

> **Deferred**: Bash 版本（`invoke-executor.sh`、`claude.sh`、`opencode.sh`、`gemini.sh`、`new-task-spec.sh`、`test-executor.sh`、`gal.sh` 對應修改）延後至有 Bash 主機可驗證時實作（OE-01：Bash runtime 同等性受限於主機環境）。
