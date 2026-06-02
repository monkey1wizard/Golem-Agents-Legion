# Plan: GAL Headless CLI Pipeline Orchestration

## Goal

讓 `/gal pipeline` 的各階段（implement / test / review / verify）能改由**本機背景的外部 AI CLI 以無頭（headless）模式執行**，而非全部由對話迴圈中的同一個模型自己接手。每個階段在自己的子行程、自己的 context 中跑，達成兩個目標：(1) 各階段 token 消耗隔離；(2) 真正的跨模型驗證（implement / test / review 可走不同 CLI）。

範圍刻意收斂為**本機 headless 一種執行模式**。任何「本機 ↔ 遠端 SSH」的跨執行模式統一抽象**不在本計畫範圍**，留待控制平面原生化（Rust）時於共用核心層處理。

**上游關係**：`docs/plans/plugin-bin-migration.md` 明確把「本機 headless executor 已上線穩定」列為其原生化候選 E 的硬前置依賴。本計畫即該依賴的上游交付，兩計畫對齊、無衝突。

## Requirements

- [ ] `gal.ps1` 在 Pipeline 分派時，能讀取 `~/.gal/config/executor-routing.ndjson`，依「階段 → 角色 → executor CLI」解析出該階段要呼叫的 CLI 名稱。`model-roles.local.md` 保留為人類可讀的參考文件，不作為腳本的機器解析來源。
- [ ] 腳本層持有「階段 → 角色」對照（implement→CODER、test→TESTER、review→REVIEWER、verify→VERIFIER）；目前此對照只存在於 SKILL 散文，需落到可解析的腳本層。
- [ ] 針對每個支援無頭模式的 CLI（Claude Code、OpenCode、Antigravity CLI / agy），存在一個薄的 executor 轉接腳本，並有一支統一的本機入口 `Invoke-Executor.ps1`。
- [ ] 分派實作任務前，能產生一份精簡、自足的 Task Spec 檔案，控制送入次級 CLI 的上下文大小（目標 < 5KB）。
- [ ] 當 CLI 不存在、不可用或逾時，系統優雅降級回現有的對話文字分派模式（`--- GAL DISPATCH ---`）。
- [ ] 次級 CLI 執行後，主 Pipeline 必須以**讀取檔案內容**確認結果已寫入預定持久化路徑（如 `## Test Results`、`## Review Results`），才判定階段完成——不得只看 Exit Code。
- [ ] 既有安全模型（重試上限、受保護路徑升級、有條件資安審查、commit 邊界收斂關卡）不受影響；次級 CLI **不得跨越 commit 邊界**，commit 由 orchestrator 持有。
- [ ] 每次 pipeline 階段分派須留下可觀測標記，記錄分派模式（offload / fallback / inline）、executor 名稱、收檔證明（receipt）與結果，寫入執行 prompt。五個 provider（claude / codex / copilot / agy / opencode）皆列入觀測面；僅無頭能力者（claude / opencode / agy）能產生實際 receipt，codex / copilot 因無無頭模式恆記為 `non-dispatchable`（此即其可觀測結果）。
- [ ] 提供小型收檔測試 script，以「令 executor 寫入隨機 token 再驗證檔案內容」證明次級 CLI **真的收到** spec（而非只看 process 起動或 Exit Code）。
- [ ] 每次 dispatch 留下一份 **durable 執行紀錄**（GAL 自寫、不依賴廠商 transcript）：擷取次級 CLI 的 stdout/stderr、起訖時間與耗時、exit code、終態分類，寫入 `.dev/executor-logs/`。用於事後回查三類問題：**逾時/無回應**、**回覆格式完全錯誤**（沒寫回或寫到錯地方）、**接收方處理一半斷線**（崩潰/被 kill 的部分完成）。此紀錄**保留供鑑識**（與暫態 task-spec 不同，不在 pipeline 結束即清），上限為保留最近 N 份或至下次同任務 dispatch。
- [ ] 先交付 PowerShell 通道；Bash 通道延後（見 Tasks 末 Deferred 與 OE-01）。

## Approach

### Step 1：路由設定 + 階段→角色對照

- **Files**: `executor-routing.example.ndjson`（repo root）、`scripts/common/Common.ps1`
- **What**:
  - 建立 `executor-routing.example.ndjson` 範例於 **repo 根目錄**（與 `model-roles.example.md`、`config.example.env` 同處）。播種擁有者為 **`Update-Personalization.ps1`**（非 Setup-Machine）：比照其既有 `config.example.env`→`config.local.env`、`model-roles.example.md`→`model-roles.local.md` 複製區塊（`scripts/Update-Personalization.ps1`），把範例複製到 `$context.GalConfigRoot`（即 `~/.gal/config/`）。每行一個 JSON 物件，例如 `{"role":"CODER","executor":"claude"}`。
  - 命名先例：腳本機器讀取的設定採 `xmachine.json` 模式——固定正規路徑、無 `.local` 中綴。故播種目標檔名為 `executor-routing.ndjson`（不是 `.local`）。
  - 在 `Common.ps1` 新增 `Read-ExecutorRouting`：**比照既有 `Resolve-XmachineConfigRecord`**（`Common.ps1`）以 `Get-GalUserHome` 解析正規路徑 `~/.gal/config/executor-routing.ndjson`，回傳 hashtable（如 `@{ CODER = "claude"; TESTER = "opencode" }`）；檔案不存在回 `$null`（不報錯）；malformed 行逐行 `try-catch` 跳過。
  - 在腳本層新增「階段 → 角色」對照（implement→CODER、test→TESTER、review→REVIEWER、verify→VERIFIER）。
- **Verify**: `Read-ExecutorRouting` 對 3 行 NDJSON 回正確 hashtable；缺檔回 `$null`；malformed 行被跳過。

### Step 2：CLI 無頭語法 spike（先驗證、再實作）

- **Files**: 無產品碼（結果記入本計畫或實作註解）
- **What**: 在建任何轉接器之前，先實測確認三個 CLI 的無頭呼叫式與 **stdin 餵入行為**，避免把假設寫進三個轉接器：
  - Claude Code：`claude -p` + bypass-permission 旗標的實際名稱與輸出格式。
  - OpenCode：`opencode run` 無頭/自動核准的實際旗標。
  - Antigravity CLI（`agy`）：無頭/非互動呼叫式與自動核准旗標的實際名稱與輸出格式（CLI 安裝於 `~/.gemini/antigravity-cli`，命令名以實測為準）。
  - 確認每個 CLI 能否從 **stdin** 讀取 prompt（而非把整份 spec 當命令列參數）。
- **Verify**: 三個 CLI 的確認呼叫式 + stdin 行為已記錄；不可用者明確標記為「環境不可驗證」。

### Step 3：本機統一執行器 `Invoke-Executor.ps1`

- **Files**: `scripts/executors/Invoke-Executor.ps1`
- **What**: 本機唯一入口。參數：`-Executor`（CLI 名稱）、`-TaskSpecPath`、`-WorkDir`、`-TimeoutMinutes`。**不含 `-Transport`**（local-only）。Exit Code：`0` 成功、`1` 任務失敗、`2` CLI 不可用或逾時。內建 `echo` mock executor 供測試。spec 一律經 **stdin** 餵入次級 CLI，不當命令列參數（避免引號/`$`/backtick/換行與命令長度問題）。逾時以 `taskkill /T` 或 Job object **回收整個行程樹**（claude/node 會 spawn 子行程，單純 `Stop-Process` 會留孤兒）。
  - **Durable 執行紀錄（鑑識）**：擷取子行程 stdout/stderr 串流落地 `.dev/executor-logs/<ts>-T-NNN-<phase>-<executor>.log`，含 header（起訖時間、耗時、executor、task/phase、spec 路徑、git branch/HEAD、exit code、終態）。終態分類（亦作 Step 6/7 `Dispatch:` 標記的 `reason=`，不另立詞彙）：`completed`（exit 0 + 寫回驗證過）/ `no-receipt`（跑了但寫回缺失或格式錯）/ `timeout`（逾時被回收）/ `disconnected-partial`（子行程非預期非零退出、可能半寫）/ `unavailable`（CLI 不存在）。此 log **保留**供事後回查，僅留最近 N 份（非 pipeline 結束即清）。
  - **半途斷線收斂**：因次級 CLI 不得 commit，斷線只留未提交的工作樹變更；orchestrator 以讀檔驗證判定，**半寫/含糊一律視為失敗並降級**，不接受部分結果；durable log 保留斷線前的輸出供診斷。
- **Verify**: `Invoke-Executor -Executor echo` → Exit 0；`-Executor nonexistent` → Exit 2；逾時 mock → Exit 2 且無孤兒行程；每次呼叫於 `.dev/executor-logs/` 留一份含終態分類的紀錄；逾時 mock 的紀錄標 `timeout`、非零退出 mock 標 `disconnected-partial`。

### Step 4：三個 executor 轉接器

- **Files**: `scripts/executors/claude.ps1`、`scripts/executors/opencode.ps1`、`scripts/executors/agy.ps1`
- **What**: 每個轉接器把統一介面翻成對應 CLI 的無頭指令（用 Step 2 實測的語法），預設 bypass-permission 以避免無頭時卡權限提示：
  - 呼叫前 `Get-Command` 檢查 CLI 是否存在，不存在回 Exit 2。
  - 逾時邏輯同 Step 3（行程樹回收）。
  - **權威結果 = 就地檔案寫回**；CLI 的 stdout（含任何 JSON 輸出）只作成功/失敗訊號，不作結果來源。
  - **Codex / Copilot 刻意排除（無無頭執行）**：二者雖在 `fix-install-ownership-stabilization`（已完成）取得真實 plugin lifecycle（`codex plugin add`、`gh copilot plugin install`——是「把 GAL 裝進該 host」），但**無無頭/非互動執行旗標**，無法作為 executor 收 spec 並就地寫回。**lifecycle 整合 ≠ 執行能力**；二者在本計畫恆為 `non-dispatchable`。
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
  - **可觀測標記**：OFFLOAD 區塊一併標明本次分派模式與 executor，供 orchestrator 寫回時記錄 `Dispatch:` 行（見 Step 7），讓降級不再靜默。
- **Verify**: `executor-routing.ndjson` 設 CODER=claude 時，分派 implement 階段會輸出「執行 Invoke-Executor.ps1」的 OFFLOAD 指示（而非 dispatch 自己 spawn）；移除 routing 檔後退回 `--- GAL DISPATCH ---`。

### Step 7：Pipeline 合約（SKILL）變更——受保護路徑

- **Files**: `commands/gal-pipeline/SKILL.template.md`（**受保護路徑，承重變更**）
- **What**: 在 `### Same-Runtime Fallback Contract` 之後、`### Runtime Step-Budget Preflight` 之前新增 `### Headless Executor Dispatch` 段，描述：
  - routing 存在時走 OFFLOAD；降級條件與行為。
  - **OFFLOAD 成功時 orchestrator 不得再自己扮演該 golem**，改為驗證子行程的就地寫回；exit 2 才退回扮演。
  - **次級 CLI 不得跨越 commit 邊界**，commit 由 orchestrator 持有（既有 commit-gate 不變）。
  - **bypass-permission 醒目安全警告**：等同完全信任次級 CLI，僅在受信任本機環境啟用。
  - **分派可觀測標記**：orchestrator 在寫回時於對應區塊記一行 `Dispatch: offload(executor=<cli>, receipt=<ok|no-receipt>, exit=<n>)` / `fallback(reason=<no-routing|no-executor|exit2-...>)` / `inline(...)`，使每階段實際走法可 grep。codex / copilot 永遠記為 `non-dispatchable`。
  - Task Spec 的暫態性質。
- **Verify**: 靜態審查確認上述語意齊備、安全關卡未削弱、bypass-permission 警告醒目、受保護路徑變更已標明。

### Step 8：Executor smoke test 與設定文件

- **Files**: `scripts/executors/Test-Executor.ps1`、`scripts/executors/Test-ExecutorReceipt.ps1`、`docs/personalization.md`、`model-roles.md`、`model-roles.example.md`
- **What**:
  - `Test-Executor.ps1`：接受 `-Executor`，對指定 CLI 跑極簡 echo 任務，驗證可用性、無頭能力與 Exit Code。
  - `Test-ExecutorReceipt.ps1`：**收檔證明探針**。接受 `-Executor`（五個 provider + echo mock），令 executor 把隨機 token 寫進 receipt 檔，再驗證檔案內容＝真的收到 spec。headless 者（claude/opencode/agy）實測收檔；codex/copilot 恆回 `non-dispatchable`；echo mock 無外部依賴可驗證探針本身。Exit 0 收到 / 1 跑了但沒收到 / 2 不可用、逾時或不可派工。
  - `docs/personalization.md`：新增 executor-routing 設定指引 + bypass-permission 安全警告。
  - `model-roles.md` / `model-roles.example.md`：註記 `executor-routing.ndjson` 為腳本的機器讀取來源，Markdown 表格僅人類參考。
- **Verify**: `Test-Executor -Executor echo` 正確回報；文件含設定指引與安全警告。

## Files to Create or Modify

- `[NEW] executor-routing.example.ndjson` — 路由設定範例（**repo root**，由 `Update-Personalization.ps1` 播種到 `~/.gal/config/executor-routing.ndjson`，無 `.local` 中綴）
- `[MODIFY] scripts/Update-Personalization.ps1` — 比照既有 example→local 複製區塊，新增 `executor-routing.example.ndjson` → `~/.gal/config/executor-routing.ndjson` 播種
- `[NEW] scripts/executors/Invoke-Executor.ps1` — 本機統一執行器入口（含 echo mock；擷取 stdout/stderr + 終態分類，寫 durable 紀錄到 `.dev/executor-logs/`）
- `[NEW] .dev/executor-logs/` — per-dispatch durable 執行紀錄輸出目錄（保留供鑑識，僅留最近 N 份）
- `[NEW] scripts/executors/claude.ps1` — Claude Code 轉接器
- `[NEW] scripts/executors/opencode.ps1` — OpenCode 轉接器
- `[NEW] scripts/executors/agy.ps1` — Antigravity CLI（agy）轉接器
- `[NEW] scripts/executors/Test-Executor.ps1` — executor smoke test
- `[NEW] scripts/executors/Test-ExecutorReceipt.ps1` — 收檔證明探針（token 寫回驗證；初版已建，含 echo mock，vendor 旗標待 Step 2 spike 確認）
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
- [ ] 每階段分派在執行 prompt 留下 `Dispatch:` 標記，可 grep 出實際走 offload / fallback / inline 與 executor；五個 provider 皆可觀測，codex / copilot 恆為 `non-dispatchable`。
- [ ] `Test-ExecutorReceipt.ps1` 能以 token 寫回證明 headless executor 真的收到 spec；codex / copilot 回 `non-dispatchable`；echo mock 回 `ok`。
- [ ] 每次 dispatch 在 `.dev/executor-logs/` 留一份 durable 紀錄，含 stdout/stderr、耗時、exit code 與終態分類；逾時、格式錯、半途斷線三類問題皆能事後從紀錄回查並對應到終態（`timeout` / `no-receipt` / `disconnected-partial`）。

## Risks

- **CLI 參數不穩定**：廠商 CLI 可能改無頭旗標語法。緩解：轉接器設計為薄、獨立模組；Step 2 先 spike 確認再實作。
- **無頭授權死鎖**：CLI 可能因 API key 過期或需登入卡住等待輸入。緩解：嚴格 Timeout + 行程樹回收 + 降級。
- **Task Spec 不夠自足**：spec 缺上下文會讓次級 CLI 自行大量讀檔，反增 token。緩解：spec 含足夠寫回路徑與 agent 合約指示。
- **寫回路徑不一致**：次級 CLI 是不受控外部行程，可能不遵守寫回指示，導致主 Pipeline 卡在驗證。緩解：驗證讀檔案內容（section 是否出現/變動），不只看 Exit Code。
- **bypass-permission 安全影響**：次級 CLI 有完整檔案系統與終端機存取，惡意或有缺陷的 agent 合約可能造成非預期刪改/執行。緩解：文件醒目警告 + 使用者自主啟用 + spec 明令不得 commit。

## Approval

- Human approval: **approved**（2026-06-01）
- Architect review: [APPROVE — 2026-06-01；local-only headless executor，命名 executor，OFFLOAD 模式，受保護路徑變更獨立成項，F-01..F-11 全數收進 Tasks。2026-06-01 codebase 變動後重審：DRIFT-01（播種擁有者 Update-Personalization 非 Setup-Machine）已修正，其餘腳本層假設全數重驗成立。fix-install-ownership-stabilization 完成後再審：codex/copilot 排除 REAFFIRMED（新 lifecycle＝安裝整合非無頭執行）、詞彙分離已記、腳本層假設仍成立。鑑識紀錄增補（T-013 durable executor log + 終態分類）：不依賴廠商 transcript、詞彙與 T-012 共用、設保留上限，APPROVE 維持]
- Additional domain review: [not requested]

## Review Results

### Architecture Review

#### Verdict: APPROVE *(2026-06-01；codebase 變動後重審)*

收斂後方向清楚且範圍受控：只做本機 headless、走既有 OFFLOAD 分派慣例、不引入跨執行模式抽象。先前 deep re-think 的 11 項發現已全數在決策或 Tasks 中收斂（見下）。

#### Re-verification（2026-06-01，install 系統大改後）

對照現行 codebase 重驗計畫所有腳本層假設：

- **[DRIFT-01] 已修正**：範例檔播種擁有者是 `Update-Personalization.ps1`（複製到 `$context.GalConfigRoot` = `~/.gal/config/`），**非 Setup-Machine**。計畫先前誤指 Setup-Machine。已更新 Step 1 / Files / T-001。腳本機器讀取設定的命名先例為 `xmachine.json`（`Resolve-XmachineConfigRecord` 讀固定正規路徑、無 `.local` 中綴）——故播種目標檔名定為 `executor-routing.ndjson`，`Read-ExecutorRouting` 比照 `Resolve-XmachineConfigRecord` 以 `Get-GalUserHome` 解析。
- **驗證成立、無需改動**：`Get-PipelineDispatchMetadata` + `CONVENTION_HINTS`（`gal.ps1:291-342`，T-007 重用有效）、`--pipeline-phase` 分派路徑與 `DISPATCH_KIND=pipeline-phase`（`gal.ps1:852-867`，T-008 掛載點有效）、`--- GAL DISPATCH ---` 文字分派（`Write-Dispatch`）、OFFLOAD 慣例（`OFFLOAD` 欄位現用於 xmachine 路徑，Step 6 對齊）、`SKILL.template.md` 錨點段落（`Same-Runtime Fallback Contract` 與 `Runtime Step-Budget Preflight` 仍在，T-009 插入點有效）、agent 合約路徑（`agent/golem-implementer.agent.md` 等齊備）。
- **上游關係確認**：`plugin-bin-migration.md` 把本 headless executor 列為原生化候選 E 的硬前置依賴；本計畫為上游交付，無衝突。已記入 Goal。

重驗結論：唯一實質 drift 已修正，其餘假設全數成立，APPROVE 維持。

#### 可觀測性增補（2026-06-01）

新增「分派可觀測標記 + 收檔證明」(T-012)：每階段在執行 prompt 記 `Dispatch:` 行，五 provider 全列入觀測面。**範圍判斷**：codex（已記排除、無等效無頭旗標）與 copilot（無無頭 executor 轉接器）無法真正收檔，故恆記 `non-dispatchable`——不擴張成替二者新建無頭通道（與既有排除決定一致、避免不可能的範圍）。收檔證明採 token 寫回驗證，是既有「讀檔驗證」契約的自然延伸，直接緩解 Trade-off 表所記「降級靜默」風險。增補小、不改依賴鏈，APPROVE 維持。

#### Re-verification（2026-06-01，`fix-install-ownership-stabilization` 完成後）

該計畫 T-001..T-014 全數完成，codebase 現有：真實 codex/copilot lifecycle、Claude skills-dir 活載入（T-005）、provider-neutral capability-probe（`Get-ProviderCliLifecycleSupport`，T-003）、install 端 Lifecycle Status Vocabulary（T-004）。對本計畫重驗：

- **codex/copilot 排除 REAFFIRMED**：新 capability-probe 探的是 **plugin 安裝生命週期**（`codex plugin marketplace add`、`gh copilot plugin install`——把 GAL 裝進 host），**非無頭執行**。二者仍無 `run -p` 式無頭執行旗標 → 作為 executor 恆 `non-dispatchable`，結論不變。已於 Step 4 加「lifecycle 整合 ≠ 執行能力」釐清，避免「codex/copilot 已整合、為何不能執行」的混淆。
- **詞彙分離**：install 端 Lifecycle Status Vocabulary（`linked-projection`/`refreshed-copy2-host`/`unprojected-artifact`/`unsupported-lane`，描述 provider 安裝讀取面）與本計畫 dispatch 詞彙（`offload`/`fallback`/`inline` + `receipt` + `non-dispatchable`，描述 pipeline 分派與收檔）是**不同軸、刻意分離**，如同 executor vs provider（F-01）。實作不得混用兩套詞彙。
- **Claude skills-dir（T-005）不影響 headless claude executor**：headless 是 spawn `claude -p` 子行程，與 GAL plugin 如何投影進 `~/.claude` 正交。
- **腳本層假設仍成立**：DRIFT-01 的播種擁有者 `Update-Personalization.ps1` 區塊在 T-012 動 install 動詞面後仍完整；`gal.ps1` 分派錨點（`Get-PipelineDispatchMetadata`、`--- GAL DISPATCH ---`、`--pipeline-phase`、`DISPATCH_KIND`）未被 fix-install 觸及。
- **probe helper 重用評估**：executor 轉接器只需 `Get-Command <cli>` 存在性檢查；`Get-ProviderCliLifecycleSupport` 為 install-mode 重量級 help 解析，對「能否收 stdin 並寫回」過重，**不重用**為刻意決定。

重驗結論：fix-install 完成未動搖本計畫任何假設，排除決定獲再確認，APPROVE 維持。

#### 鑑識紀錄增補（2026-06-01）

新增 T-013「durable 執行紀錄」：`Invoke-Executor` 擷取子行程 stdout/stderr + header 落地 `.dev/executor-logs/`，終態分類 `completed`/`no-receipt`/`timeout`/`disconnected-partial`/`unavailable`，供事後回查逾時、格式錯、半途斷線三類問題。**設計判斷**：

- **不依賴廠商 transcript**（各家位置/格式各異、headless 多為一次性 print），GAL 自寫紀錄；stdout 仍只是訊號，權威結果仍為就地寫回（不違反 F-07）。
- **詞彙不發散**：終態分類即 `Dispatch:` 的 `reason=` 列舉，與 T-012 共用，避免又一套平行詞彙。
- **保留 vs 暫態**：與 task-spec（pipeline 結束即清）刻意區隔——log 保留供鑑識但設上限（最近 N 份），避免無界堆積。
- **半途斷線正確性**：因次級 CLI 不得 commit，斷線只留未提交工作樹；orchestrator 半寫一律視為失敗並降級，log 留斷線前輸出。直接強化既有「降級靜默」與「寫回不一致」風險緩解。

增補小、不改依賴鏈（T-013 依賴 T-003、SKILL 部分屬 T-009 同段），APPROVE 維持。

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

- **F-01 命名**：採 `executor` 語彙（`scripts/executors/`、`Invoke-Executor.ps1`、`executor-routing.ndjson`、`-Executor`），與既有「安裝宿主 provider」子系統（`ProviderPlugin.ps1`、`~/.gal/dist/providers/`、集合 {claude,copilot,codex,agy}）字面分離；provider 子系統不動。
- **F-02 transport 過度抽象**：只做本機 headless（無 `-Transport`）；本機就地寫回與遠端 SSH（patch-first、遠端永不 commit）收斂模型相反，跨執行模式統一抽象移交 Rust 原生層 backlog。
- **F-03 dispatch 不 spawn**：Step 6 改走 OFFLOAD 指示模式，與既有執行器慣例一致。
- **F-04 受保護路徑承重變更**：Step 7 / T-009 獨立成項，明列 orchestrator 行為改變與 commit 邊界。
- **F-05 範例檔落點**：移至 repo root，由 `Update-Personalization.ps1` 播種到 `~/.gal/config/executor-routing.ndjson`（無 `.local` 中綴；見 DRIFT-01）。
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

#### Verdict: CLEAR *(2026-06-01 re-refine：含 T-012、修正依賴編號)*

可建構性（依賴鏈，已修正先前編號誤植 task-spec=T-007 非 T-005）：

- **基礎**：T-001（路由+階段→角色對照）、T-003（`Invoke-Executor.ps1` 統一入口，含 echo mock）——皆獨立。
- **spike 前置**：T-002（無頭語法 spike）前置於 T-004/T-005/T-006（claude/opencode/agy 轉接器）；三轉接器另依賴 T-003 統一介面。
- **獨立**：T-007（`New-TaskSpec`，讀 prompt + 重用 CONVENTION_HINTS）、T-010（smoke test）、T-011（文件）。
- **整合**：T-008（gal.ps1 分派）依賴 T-001 + T-003 + T-007；T-009（SKILL）依賴 T-008 整合語意。
- **可觀測性**：T-012(a) 分派標記依賴 T-008 + T-009；T-012(b) 收檔探針 `Test-ExecutorReceipt.ps1` 獨立（初版已建、echo 路徑可跑，claude/opencode/agy 旗標待 T-002 補齊）。
- **鑑識紀錄**：T-013 durable 執行紀錄依賴 T-003（Invoke-Executor 基底），SKILL 部分屬 T-009 同段；終態分類與 T-012 的 `Dispatch:` `reason=` 共用列舉、不另立詞彙。

依賴鏈清晰、無循環。

受保護路徑提醒：T-009 改 `commands/gal-pipeline/SKILL.template.md`（受保護路徑），已由架構審查放行並獨立成任務；T-012(a) 與 T-013 對 SKILL 的新增均限於 T-009 同段內，實作不得擴大到其他 `commands/` 內容。

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
| TP-006 | integration | Antigravity CLI（agy）同 TP-004。 | T-006 |
| TP-007 | unit | `New-TaskSpec -TaskScope T-001 -Phase implement` 對含 3 Task 的 prompt 產出 < 5KB，含目標/受影響檔案/寫回路徑/「不得 commit」指示；Pipeline 結束 `.dev/task-specs/` 清理。 | T-007 |
| TP-008 | integration | `executor-routing.ndjson` 設 CODER=claude 時，`gal.ps1` 分派 implement 輸出「執行 Invoke-Executor.ps1」OFFLOAD 指示（dispatch 不自行 spawn）；移除 routing → 退回 `--- GAL DISPATCH ---`。 | T-008 |
| TP-009 | manual | 靜態審查 `SKILL.template.md`：OFFLOAD 成功時 orchestrator 不自扮演、次級 CLI 不得 commit、降級語義、bypass-permission 警告皆在；受保護路徑變更已標明。 | T-009 |
| TP-010 | integration | `Test-Executor -Executor echo` 正確回報可用性與 Exit Code。 | T-010 |
| TP-011 | manual | `docs/personalization.md`、`model-roles.md`/`model-roles.example.md` 含 executor-routing 設定指引與 bypass-permission 安全警告。 | T-011 |
| TP-012 | integration | `Test-ExecutorReceipt -Executor echo` → `receipt=ok` Exit 0；`-Executor codex`/`copilot` → `non-dispatchable` Exit 2；裝了 claude/opencode/agy 時 token 寫回檔案內容相符。 | T-012 |
| TP-013 | integration | 設 CODER=claude 跑 implement，執行 prompt 出現 `Dispatch: offload(executor=claude, receipt=ok, ...)`；移除 routing → `Dispatch: fallback(reason=no-routing)`。 | T-012 |
| TP-014 | integration | `Invoke-Executor` 每次呼叫於 `.dev/executor-logs/` 留紀錄含 header + stdout/stderr + 終態：echo→`completed`；逾時 mock→`timeout`；非零退出 mock→`disconnected-partial`；缺 CLI→`unavailable`；保留上限只留最近 N 份。 | T-013 |

## Tasks

- [x] T-001 — 建立 `executor-routing.example.ndjson`（repo root，每行 `{"role":"CODER","executor":"claude"}`）；在 `scripts/Update-Personalization.ps1` 新增播種區塊（比照 `config.example.env`/`model-roles.example.md`），把範例複製到 `~/.gal/config/executor-routing.ndjson`（無 `.local` 中綴）；在 `scripts/common/Common.ps1` 新增 `Read-ExecutorRouting`（比照 `Resolve-XmachineConfigRecord` 以 `Get-GalUserHome` 解析正規路徑 → hashtable，缺檔回 `$null`，malformed 行 try-catch 跳過）與「階段→角色」對照（implement→CODER、test→TESTER、review→REVIEWER、verify→VERIFIER）。
- [x] T-002 — CLI 無頭語法 spike：實測確認 Claude Code / OpenCode / Antigravity CLI（agy）的無頭呼叫式與 stdin 餵入行為，記錄為 T-004~006 依據。不寫產品碼。
- [ ] T-003 — 建立 `scripts/executors/Invoke-Executor.ps1` 本機統一入口：`-Executor`、`-TaskSpecPath`、`-WorkDir`、`-TimeoutMinutes`（**無 `-Transport`**）。Exit 0/1/2。內建 echo mock。spec 經 stdin 餵入。逾時用 `taskkill /T` 或 Job object 回收整個行程樹。
- [ ] T-004 — 建立 `scripts/executors/claude.ps1`：`Get-Command claude` 檢查；依 T-002 語法以 stdin 餵 spec 呼叫 claude（bypass-permission）；權威結果＝就地寫回、stdout 僅訊號；逾時/不存在回 Exit 2。
- [ ] T-005 — 建立 `scripts/executors/opencode.ps1`：依 T-002 語法呼叫 OpenCode（非互動自動核准）；可用性檢查與逾時同 T-004。
- [ ] T-006 — 建立 `scripts/executors/agy.ps1`：依 T-002 語法呼叫 Antigravity CLI（agy，非互動自動核准）；可用性檢查與逾時同 T-004。
- [ ] T-007 — 建立 `scripts/common/New-TaskSpec.ps1`：從 `.dev/plans/<slug>.prompt.md` 提取指定 T-NNN 的目標、受影響檔案、git branch/HEAD、寫回路徑指示、Agent 合約路徑；慣例提示**重用 `Get-PipelineDispatchMetadata` 的 CONVENTION_HINTS**；spec 內**明令次級 CLI 不得 git commit/push**；輸出 `.dev/task-specs/T-NNN-<phase>.md`（< 5KB）；含 Pipeline 結束清理。
- [ ] T-008 — 修改 `scripts/gal.ps1`：在 `--pipeline-phase` 路徑用「階段→角色」+ `Read-ExecutorRouting` 解析 executor。解析到 → 輸出 **OFFLOAD 區塊**指示對話 AI 執行 `Invoke-Executor.ps1 -Executor <cli> -TaskSpecPath <spec> -Wait`（標明 exit 0→驗證寫回、exit 2→退回扮演）；**dispatch 不自行 spawn**。無 routing/無對應 executor → 維持現有文字分派。
- [ ] T-009 — 修改 `commands/gal-pipeline/SKILL.template.md`（**受保護路徑**）：在 `### Same-Runtime Fallback Contract` 後、`### Runtime Step-Budget Preflight` 前新增 `### Headless Executor Dispatch` 段：OFFLOAD 觸發與降級語義；**OFFLOAD 成功時 orchestrator 改為驗證寫回、不自扮演 golem，exit 2 才扮演**；**次級 CLI 不得跨越 commit 邊界，commit 由 orchestrator 持有**；bypass-permission 醒目安全警告；Task Spec 暫態性質。
- [ ] T-010 — 建立 `scripts/executors/Test-Executor.ps1` smoke test：`-Executor` 對指定 CLI 跑極簡 echo，驗證可用性、無頭能力、Exit Code。
- [ ] T-011 — 更新文件：`docs/personalization.md` 新增 executor-routing 設定指引 + bypass-permission 安全警告；`model-roles.md` 與 `model-roles.example.md` 註記 `executor-routing.ndjson` 為腳本機器讀取來源、Markdown 表格僅人類參考。
- [ ] T-012 — 可觀測性：(a) `scripts/gal.ps1` 在 OFFLOAD 區塊標明分派模式與 executor、SKILL（T-009 同段）規定 orchestrator 寫回時記 `Dispatch:` 行（offload/fallback/inline，含 receipt 與降級原因，codex/copilot 恆 `non-dispatchable`）；(b) 完成 `scripts/executors/Test-ExecutorReceipt.ps1` 收檔證明探針（token 寫回驗證，五 provider + echo；初版已建，補齊 claude/opencode/agy 旗標需 T-002 spike）。
- [ ] T-013 — Durable 執行紀錄（鑑識）：`Invoke-Executor.ps1`（建於 T-003）擷取子行程 stdout/stderr + header（起訖/耗時/executor/task-phase/spec/git/exit）寫 `.dev/executor-logs/<ts>-T-NNN-<phase>-<executor>.log`，並做終態分類 `completed`/`no-receipt`/`timeout`/`disconnected-partial`/`unavailable`（同 `Dispatch:` 的 `reason=` 列舉、不另立詞彙）；保留上限只留最近 N 份；SKILL（T-009 同段）記錄「半寫/含糊一律視為失敗並降級、commit 由 orchestrator 持有」與如何用此紀錄回查逾時/格式錯/斷線。依賴 T-003，與 T-012 的 `Dispatch:` reason 對齊。

> **Deferred**: Bash 版本（`invoke-executor.sh`、`claude.sh`、`opencode.sh`、`agy.sh`、`new-task-spec.sh`、`test-executor.sh`、`test-executor-receipt.sh`、`gal.sh` 對應修改）延後至有 Bash 主機可驗證時實作（OE-01：Bash runtime 同等性受限於主機環境）。
