# Plan: GAL Headless CLI Pipeline Orchestration

## Goal

讓 GAL 的 Pipeline 分派器 (`gal.ps1`) 能以無頭 (headless) 模式直接在背景呼叫外部 AI CLI 工具 (Claude Code、OpenCode、Gemini CLI) 來執行特定階段的工作，而非僅仰賴對話迴圈中的角色切換。藉此實現 `model-roles.local.md` 的真正跨 Provider 執行能力、隔離各階段的 Token 消耗，並利用 xmachine 已有的「Task Spec → 背景執行 → 結果回收」模式作為設計骨幹。

## Requirements

- [ ] `gal.ps1` 在 Pipeline 分派時，能讀取 `~/.gal/config/provider-routing.ndjson`，將 CODER、TESTER、REVIEWER 角色解析為對應的 CLI 工具名稱。`model-roles.local.md` 保留為人類可讀的參考文件，不再作為腳本的解析來源。
- [ ] 針對每個已驗證支援無頭模式的 CLI (Claude Code `claude -p`、OpenCode `opencode run`、Gemini CLI `gemini -p`)，存在一個標準化的 Provider 轉接腳本，提供統一的呼叫介面。
- [ ] Pipeline 分派實作任務前，能產生一份精簡的、自足的 Task Spec 檔案 (`T-NNN-task-spec.md`)，以控制送入次級 CLI 的上下文大小。
- [ ] 當 CLI 工具不存在、不可用或逾時，系統能優雅降級回現有的對話文字分派模式 (`--- GAL DISPATCH ---`)。
- [ ] 次級 CLI 執行完畢後，主 Pipeline 腳本必須能驗證對方已將結果寫入預定的持久化路徑（如 `## Test Results`、`## Review Results`），才判定階段完成。
- [ ] 既有的安全模型（重試上限、受保護路徑升級、有條件的資安審查、commit 邊界收斂關卡）不受影響。
- [ ] PowerShell 與 Bash 雙通道均須實作，維持跨平台一致性。

## Approach

### Step 1: 定義 Provider 轉接層的標準介面

- **Files**: `scripts/providers/Invoke-Provider.ps1`, `scripts/providers/invoke-provider.sh`
- **What**: 建立統一的 Provider 呼叫介面。接受以下參數：`-Provider` (cli 名稱)、`-TaskSpecPath` (任務 Spec 檔案路徑)、`-WorkDir` (工作目錄)、`-TimeoutMinutes` (逾時上限)。回傳標準化的 Exit Code：`0` 成功、`1` 任務失敗、`2` CLI 不可用或逾時。此介面刻意模仿 xmachine 的 `Invoke-XmachineTask.ps1` 模式：呼叫者準備 Task Spec、轉接器負責執行、呼叫者負責結果驗證。
- **Verify**: `Invoke-Provider -Provider "echo" -TaskSpecPath "test.md"` 能成功呼叫一個 echo mock 並回傳 Exit Code 0。

### Step 2: 實作已驗證的 Provider 轉接器

- **Files**: `scripts/providers/claude.ps1`, `scripts/providers/opencode.ps1`, `scripts/providers/gemini.ps1` (以及對應的 `.sh` 版本)
- **What**: 每個轉接器負責將統一介面翻譯為對應 CLI 的無頭指令。所有轉接器預設使用 bypass-permission 模式，以避免無頭執行時因權限提示而死鎖：
  - Claude Code: `claude -p "$(Get-Content $TaskSpecPath)" --output-format json --dangerously-skip-permissions`
  - OpenCode: `opencode run -q -f json "$(Get-Content $TaskSpecPath)"` （OpenCode 在非互動模式下自動核准權限）
  - Gemini CLI: `gemini -p "$(Get-Content $TaskSpecPath)"`
  - 每個轉接器在呼叫前先用 `Get-Command` / `which` 檢查 CLI 是否存在，不存在則回傳 Exit Code 2。
  - 每個轉接器設定 Process Timeout，逾時則殺掉子行程並回傳 Exit Code 2。
  - **安全警告**：bypass-permission 模式等同於完全信任次級 CLI 的行為。使用者必須確保只在受信任的本機環境中啟用 Headless CLI Dispatch。此警告必須寫入 `commands/gal-pipeline/SKILL.template.md` 的 `## Headless CLI Dispatch` 段落，以及 `docs/personalization.md` 的相關設定指引中。
- **Verify**: 在有安裝 Claude Code 的環境中，`Invoke-Provider -Provider "claude" -TaskSpecPath "echo-test.md"` 能啟動 `claude.exe` 子行程並正確回傳結果。在未安裝的環境中，回傳 Exit Code 2。

### Step 3: 實作 Task Spec 產生器

- **Files**: `scripts/common/New-TaskSpec.ps1`, `scripts/common/new-task-spec.sh`
- **What**: 從活躍的執行 prompt (`.dev/plans/<slug>.prompt.md`) 中，為指定的 `T-NNN` 任務提取以下資訊，組裝成一份自足的 Markdown Spec 檔案：
  - 任務目標（從 `## Tasks` 中對應的行）
  - 受影響的檔案列表（從計畫的 `## Files to Create or Modify` 中篩選）
  - 當前 Git branch 與 HEAD commit
  - 寫回路徑指示（告訴次級 CLI 應將結果寫入哪個檔案的哪個 section）
  - 適用的語言慣例提示（延用 `CONVENTION_HINTS` 機制）
  - Agent 合約檔案路徑（如 `agent/golem-implementer.agent.md`）
  - Spec 檔案寫入 `.dev/task-specs/T-NNN-<phase>.md`，視為暫態控制平面產出物（與 xmachine 的 `.dev/xmachine-*.md` 同類）。
- **Verify**: 對一個含有 3 個 Task 的執行 prompt 執行 `New-TaskSpec -TaskScope T-001 -Phase implement`，輸出檔案小於 5KB 且包含必要的寫回路徑指示。

### Step 4: 整合 Provider 呼叫到 Pipeline 分派器

- **Files**: `scripts/gal.ps1`, `scripts/gal.sh`
- **What**: 在現有的 `dispatch` 命令分支中，當偵測到 `--pipeline-phase` 標記時，新增以下邏輯：
  1. 讀取 `~/.gal/config/provider-routing.ndjson`，解析該階段對應角色的 Provider CLI 名稱。NDJSON 格式為每行一個 JSON 物件，例如 `{"role":"CODER","provider":"claude"}`。
  2. 呼叫 `New-TaskSpec` 產生 Task Spec。
  3. 呼叫 `Invoke-Provider -Provider <resolved-cli> -TaskSpecPath <spec-path> -Wait`。
  4. 檢查 Exit Code：
     - `0`：繼續現有的結果驗證流程（檢查 `## Test Results` 等）。
     - `1`：觸發重試迴圈。
     - `2`：降級回對話文字分派模式，印出 `--- GAL DISPATCH ---` 並附帶降級警告。
  - **降級觸發點**：Exit Code 2、`provider-routing.ndjson` 不存在、或該角色未指定 Provider CLI。這三種情況都退回原有的文字分派模式，確保向後相容。
- **Verify**: 在 `provider-routing.ndjson` 中設定 CODER=claude，執行 `gal.ps1 dispatch golem-implementer --pipeline-phase implement --task-scope T-001` 時，能觀察到 `claude.exe` 子行程啟動。移除 `provider-routing.ndjson` 後重新執行，觀察到退回 `--- GAL DISPATCH ---` 文字分派。

### Step 5: 更新 Pipeline 合約文件

- **Files**: `commands/gal-pipeline/SKILL.template.md`
- **What**: 在 `## Model Assignment` 與 `## Same-Runtime Fallback Contract` 之間，新增 `## Headless CLI Dispatch` 段落，描述：
  - 當分派器偵測到 Provider 設定時，自動使用無頭 CLI 執行。
  - 降級條件與行為。
  - Task Spec 的產生與暫態性質。
  - 與 xmachine 的關係：兩者共用 `Invoke-Provider` 統一介面與 Task Spec → 執行 → 結果回收的模式。Headless CLI 為預設的本機子行程模式；xmachine 為進階的 SSH 遠端模式，在 `Invoke-Provider` 內部根據 `-Transport local|ssh` 參數決定路徑。
  - **安全警告段落（必須醒目）**：Headless CLI Dispatch 預設使用 bypass-permission 模式（Claude Code 的 `--dangerously-skip-permissions`、OpenCode 的非互動自動核准）。這等同於完全信任次級 CLI 對檔案系統與終端機的存取權限。使用者必須確保僅在受信任的本機環境中啟用此功能。
- **Verify**: 靜態審查合約文件，確認降級語義、安全關卡未被削弱，且 bypass-permission 警告段落存在且醒目。

## Files to Create or Modify

- `[NEW] scripts/providers/Invoke-Provider.ps1` — 統一的 Provider 呼叫介面 (PowerShell)
- `[NEW] scripts/providers/invoke-provider.sh` — 統一的 Provider 呼叫介面 (Bash)
- `[NEW] scripts/providers/claude.ps1` — Claude Code CLI 轉接器
- `[NEW] scripts/providers/opencode.ps1` — OpenCode CLI 轉接器
- `[NEW] scripts/providers/gemini.ps1` — Gemini CLI 轉接器
- `[NEW] scripts/providers/claude.sh` — Claude Code CLI 轉接器 (Bash)
- `[NEW] scripts/providers/opencode.sh` — OpenCode CLI 轉接器 (Bash)
- `[NEW] scripts/providers/gemini.sh` — Gemini CLI 轉接器 (Bash)
- `[NEW] scripts/common/New-TaskSpec.ps1` — Task Spec 產生器 (PowerShell)
- `[NEW] scripts/common/new-task-spec.sh` — Task Spec 產生器 (Bash)
- `[NEW] ~/.gal/config/provider-routing.example.ndjson` — Provider 路由設定範例（NDJSON 格式）
- `[MODIFY] scripts/gal.ps1` — Pipeline 分派時讀取 `provider-routing.ndjson`、產生 Task Spec、呼叫 Provider 轉接器
- `[MODIFY] scripts/gal.sh` — Bash 映射
- `[MODIFY] commands/gal-pipeline/SKILL.template.md` — 新增 Headless CLI Dispatch 合約段落（含 bypass-permission 安全警告）
- `[MODIFY] docs/personalization.md` — 新增 Provider Routing 設定指引與安全警告
- `[MODIFY] model-roles.md` — 說明 `provider-routing.ndjson` 取代 Markdown 表格作為腳本的機器讀取來源
- `[MODIFY] model-roles.example.md` — 引導使用者同步維護 `provider-routing.ndjson`

## Success Criteria

- [ ] 在已安裝 Claude Code 的機器上，執行 `/gal pipeline` 時，Implement 階段能觀察到 `claude.exe` 子行程在背景啟動並完成工作。
- [ ] 在未安裝任何外部 CLI 的機器上，執行 `/gal pipeline` 時，系統退回原有的對話文字分派模式，行為完全不變。
- [ ] Task Spec 檔案大小穩定小於 5KB，且次級 CLI 不需要額外讀取 `.dev/project.md` 或生成的轉接器。
- [ ] Pipeline 的重試上限、受保護路徑升級、有條件的資安審查與 commit 邊界收斂關卡，在 Headless CLI 模式與降級模式下均正常運作。

## Risks

- **CLI 參數不穩定**：各廠商的 CLI 工具可能在後續版本中更改無頭模式的參數語法。將轉接器設計為獨立的、薄的模組，降低維護成本。
- **無頭模式的授權死鎖**：CLI 可能因為 API Key 過期或需要登入而卡在等待人類輸入。必須在 `Start-Process` 時設定嚴格的 Timeout，逾時後殺掉子行程並降級。
- **Task Spec 不夠自足**：如果 Task Spec 缺少必要的上下文，次級 CLI 可能會自行去讀取大量檔案，反而增加 Token 消耗。Spec 必須包含足夠的指示讓次級 CLI 不需要額外探索。
- **寫回路徑不一致**：次級 CLI 可能不遵守 Task Spec 中指定的寫回路徑，導致主 Pipeline 無法偵測到結果。必須在結果驗證階段嚴格檢查檔案內容，而非僅依賴 Exit Code。
- **bypass-permission 的安全影響**：預設使用 bypass-permission 模式意味著次級 CLI 有完整的檔案系統與終端機存取權限。惡意或有缺陷的 Agent 合約可能導致不預期的檔案刪除或指令執行。此風險透過文件警告與使用者自主啟用來緩解。

## Open Questions

- [x] OQ-001 — `model-roles.local.md` 是否應保留為唯一的 Provider 路由設定來源，還是引入一個平行的機器可讀格式供腳本讀取？*(raised by: deep-planning, resolved by: human decision)* — **已解決**：引入 `~/.gal/config/provider-routing.ndjson` 作為腳本讀取的權威來源。NDJSON 格式與另一個計畫預計引入的方向一致。`model-roles.local.md` 保留為人類可讀的參考文件。每行一個 JSON 物件，例如 `{"role":"CODER","provider":"claude"}`。
- [x] OQ-002 — `--dangerously-skip-permissions` (Claude Code) 與自動核准權限 (OpenCode) 在 Pipeline 中使用是否可接受？*(raised by: deep-planning, resolved by: human decision)* — **已解決**：預設使用 bypass-permission 模式。此決定必須寫入文件中以醒目方式提醒使用者，包括 `commands/gal-pipeline/SKILL.template.md` 的 Headless CLI Dispatch 段落與 `docs/personalization.md` 的設定指引。
- [x] OQ-003 — Headless CLI Dispatch 與 xmachine 是否應共用同一個 `Invoke-Provider` 介面？*(raised by: deep-planning, resolved by: human decision)* — **已解決**：合併為統一的 `Invoke-Provider` 介面。本機子行程為預設模式 (`-Transport local`)；SSH 遠端執行為進階功能 (`-Transport ssh`)。現有 `Invoke-XmachineTask.ps1` 的核心邏輯將被重構進 `Invoke-Provider` 中，`Invoke-XmachineTask.ps1` 退化為一個呼叫 `Invoke-Provider -Transport ssh` 的薄 wrapper。

<!-- Format: - [ ] OQ-NNN — description *(raised by: command)* -->
<!-- Resolved: - [x] OQ-NNN — description *(raised by: command, resolved by: engineering-review-lane)* -->

## Approval

- Human approval: [pending]
- Architect review: [see below]
- Additional domain review: [not requested]

## Review Results

### Architecture Review

#### Verdict: REVISE (OQ 已解決，待更新後可重新評估為 APPROVE)

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| 以 xmachine 的 Task Spec 模式作為設計骨幹 | 重用已驗證的模式；控制節點/工作節點的職責分離清晰 | 需要確保本機子行程模式與 SSH 遠端模式的 Task Spec 格式能共用 | OK |
| 為每個 Provider CLI 建立獨立的轉接腳本 | 隔離變化；廠商 CLI 更新時只需修改一個檔案 | 6 個新檔案（3 PS + 3 Bash）的維護成本 | OK |
| 使用 `provider-routing.ndjson` 作為機器讀取來源 | 腳本解析穩定；與其他計畫的 NDJSON 方向對齊 | 使用者需同時維護 `.ndjson` 與 `model-roles.local.md` 兩份文件 | OK — OQ-001 已解決 |
| 降級回對話文字分派作為 Fallback | 完全向後相容；不強制使用者安裝額外 CLI | 降級路徑可能在長期內變成逃逸口，導致 Headless 模式永遠得不到充分測試 | OK — 可接受但需記錄 |
| 預設使用 bypass-permission 模式 | 無頭模式必須避免卡在權限提示 | 完全信任次級 CLI 的行為，可能導致不預期的檔案修改或指令執行 | OK — OQ-002 已解決；需在文件中醒目警告 |
| 合併 Invoke-Provider 介面，xmachine 走 SSH transport | 統一概念模型；減少重複程式碼 | `Invoke-XmachineTask.ps1` 需重構為薄 wrapper | OK — OQ-003 已解決 |

#### Over-engineering Flags

- **[OE-01]** 目前計畫列出 6 個 Bash 轉接腳本，但 GAL 的 Bash runtime 同等性在前一個計畫中已被記錄為「受限於主機環境」。建議 Step 2 先只實作 PowerShell 版本，Bash 版本延後到有 Bash 主機可驗證時。

#### Bug Surface

- **[BUG-01] Medium**: Task Spec 中的「寫回路徑指示」如果被次級 CLI 忽略，主 Pipeline 會卡在結果驗證階段。次級 CLI 是一個不受控的外部行程，沒有保證它會遵守指示。
  - Will break when: 次級 CLI 的 Agent 合約未被正確載入，或 CLI 版本更新後行為改變。
  - Fix: 結果驗證不能僅依賴 Exit Code，必須實際讀取檔案並檢查對應 section 的內容變化（例如 `## Test Results` 的 timestamp 或 task-scoped subsection 是否存在）。
- **[BUG-02] Low** (已緩解): `model-roles.local.md` 的 Markdown 表格解析風險已透過 OQ-001 解決。腳本將改為讀取 `provider-routing.ndjson`，不再解析 Markdown 表格。殘餘風險僅為 NDJSON 格式本身的 malformed 行，可透過逐行 `try-catch` parse 處理。

#### Performance Concerns

無。此計畫的核心目的就是降低 Token 消耗，不會引入新的效能瓶頸。

#### Missing from Plan

- 未提及 `Codex CLI` 的轉接器。根據研究，Codex CLI 不支援無頭模式，不應列入轉接目標。計畫應明確排除它並說明原因。
- 未提及 `.dev/task-specs/` 目錄的清理機制。Task Spec 是暫態產出物，需要在任務完成或 Pipeline 結束時清理。
- 未提及 Provider 轉接器的 Smoke Test 機制。應參考 xmachine 的 `Test-Xmachine.ps1` 建立 `Test-Provider.ps1`，讓使用者能驗證本機的 CLI 可用性。

#### Recommended Changes

1. ~~**OQ-001 必須在實作前決定**~~ — 已解決：使用 `provider-routing.ndjson`。
2. **先做 PowerShell、延後 Bash** — 根據前一個計畫的 T-010 結論，目前無法在此主機上驗證 Bash 運行。先集中精力在 PowerShell 轉接器，Bash 版本標記為 deferred。
3. **新增 `Test-Provider.ps1`** — 提供使用者一個入口來驗證本機的 CLI 可用性，類似 xmachine 的 smoke test。
4. ~~**OQ-002 需決定**~~ — 已解決：預設 bypass-permission，文件中醒目警告。
5. ~~**OQ-003 需決定**~~ — 已解決：合併介面，SSH 為進階 transport。

#### What's Good (keep these)

- **xmachine 模式對齊** — 重用 Task Spec → 執行 → 結果回收的模式是正確的架構決策。這讓兩個系統（本機 Headless 與遠端 xmachine）能共享概念模型與驗證邏輯。
- **優雅降級設計** — 不強制使用者安裝額外 CLI，讓系統在任何環境中都能運作。
- **Task Spec 隔離** — 控制送入次級 CLI 的上下文大小，是解決 Token 燃燒的正確方法。

### Business Review

Pending.

### Design Review

Pending.

### Engineering Review

#### Verdict: CLEAR

所有 Open Questions 已解決。架構審查的 REVISE 項目已於 OQ 決策輪中收斂。以下為工程評估：

**可建構性 (Buildability)**：計畫的 9 個任務各自獨立可完成。T-001（NDJSON 解析）與 T-002（統一介面）為基礎層，T-003–T-005（轉接器）依賴 T-002，T-006（Task Spec 產生器）獨立於轉接層，T-007（整合）依賴 T-001+T-002+T-006，T-008（Smoke Test）依賴 T-002，T-009（文件）獨立。依賴鏈清晰，無循環。

**Bash 延後決策**：正確。目前主機無法驗證 Bash，先做 PowerShell 降低範圍與風險。Bash 版本在未來有主機可驗證時再補齊。

**降級路徑**：`provider-routing.ndjson` 不存在 → 退回文字分派。CLI 不可用 → Exit Code 2 → 退回文字分派。逾時 → 殺掉子行程 → Exit Code 2 → 退回文字分派。三條降級路徑覆蓋完整。

**安全模型**：bypass-permission 決策已記錄。既有的重試上限、受保護路徑升級、有條件的資安審查與 commit 邊界收斂關卡不受 Headless CLI 路徑影響，因為這些關卡在主 Pipeline 層級運作，不受次級 CLI 的執行模式影響。

**風險緩解**：BUG-01（寫回路徑不一致）透過檔案內容驗證（而非僅 Exit Code）緩解。BUG-02 已透過 NDJSON 格式降為 Low。Timeout 機制防止死鎖。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | unit | `Read-ProviderRouting` 能正確解析含 3 行 NDJSON 的檔案，回傳正確的 hashtable。檔案不存在時回傳 `$null`。含 malformed 行時跳過該行並繼續。 | T-001 |
| TP-002 | unit | `Invoke-Provider -Provider "echo"` 使用內建 mock 成功執行並回傳 Exit Code 0。`-Provider "nonexistent"` 回傳 Exit Code 2。 | T-002 |
| TP-003 | integration | 在已安裝 Claude Code 的環境中，`Invoke-Provider -Provider "claude" -TaskSpecPath <echo-test.md>` 能啟動 `claude.exe` 子行程。未安裝時回傳 Exit Code 2。 | T-003 |
| TP-004 | integration | 在已安裝 OpenCode 的環境中，`Invoke-Provider -Provider "opencode" -TaskSpecPath <echo-test.md>` 能啟動 `opencode.exe` 子行程。未安裝時回傳 Exit Code 2。 | T-004 |
| TP-005 | integration | 在已安裝 Gemini CLI 的環境中，`Invoke-Provider -Provider "gemini" -TaskSpecPath <echo-test.md>` 能啟動 `gemini` 子行程。未安裝時回傳 Exit Code 2。 | T-005 |
| TP-006 | unit | `New-TaskSpec -TaskScope T-001 -Phase implement` 對一個含有 3 個 Task 的執行 prompt 產生的 Spec 檔案小於 5KB，且包含任務目標、受影響檔案、寫回路徑指示。Pipeline 結束後 `.dev/task-specs/` 被清理。 | T-006 |
| TP-007 | integration | `provider-routing.ndjson` 設定 CODER=claude 時，`gal.ps1` Pipeline 分派 golem-implementer 能觀察到 `claude.exe` 子行程啟動。移除 `provider-routing.ndjson` 後退回 `--- GAL DISPATCH ---` 文字分派。 | T-007 |
| TP-008 | manual | 靜態審查 `SKILL.template.md`、`personalization.md`、`model-roles.md`，確認 Headless CLI Dispatch 段落存在、bypass-permission 安全警告醒目、降級語義與安全關卡未被削弱。 | T-009 |


## Tasks

- [ ] T-001 — 建立 `provider-routing.example.ndjson` 範例檔與 NDJSON 解析函式 (`Read-ProviderRouting`) 於 `scripts/common/Common.ps1`。每行格式 `{"role":"CODER","provider":"claude"}`。解析函式讀取 `~/.gal/config/provider-routing.ndjson`，回傳 hashtable `@{ CODER = "claude"; TESTER = "opencode" }`。檔案不存在時回傳 `$null`（不報錯）。
- [ ] T-002 — 建立 `scripts/providers/Invoke-Provider.ps1` 統一呼叫介面。參數：`-Provider`、`-TaskSpecPath`、`-WorkDir`、`-TimeoutMinutes`、`-Transport` (預設 `local`)。Exit Code 語義：`0` 成功、`1` 任務失敗、`2` CLI 不可用或逾時。內含 echo mock provider 供測試。
- [ ] T-003 — 建立 `scripts/providers/claude.ps1` 轉接器。呼叫前用 `Get-Command claude` 檢查可用性。呼叫 `claude -p <spec-content> --output-format json --dangerously-skip-permissions`。設定 `-TimeoutMinutes` 逾時後殺掉子行程回傳 Exit Code 2。
- [ ] T-004 — 建立 `scripts/providers/opencode.ps1` 轉接器。呼叫 `opencode run -q -f json <spec-content>`。OpenCode 非互動模式自動核准權限。可用性檢查與 Timeout 邏輯同 T-003。
- [ ] T-005 — 建立 `scripts/providers/gemini.ps1` 轉接器。呼叫 `gemini -p <spec-content>`。可用性檢查與 Timeout 邏輯同 T-003。
- [ ] T-006 — 建立 `scripts/common/New-TaskSpec.ps1` Task Spec 產生器。從 `.dev/plans/<slug>.prompt.md` 提取指定 T-NNN 的任務目標、受影響檔案、Git branch/HEAD、寫回路徑指示、慣例提示、Agent 合約路徑。輸出至 `.dev/task-specs/T-NNN-<phase>.md`。包含 Pipeline 結束時的清理邏輯（刪除 `.dev/task-specs/` 下的暫態檔案）。
- [ ] T-007 — 修改 `scripts/gal.ps1`，在 Pipeline 分派路徑中整合 `Read-ProviderRouting`、`New-TaskSpec`、`Invoke-Provider`。偵測到 `provider-routing.ndjson` 且角色有對應 Provider 時走 Headless CLI 路徑；否則降級回現有的文字分派模式。
- [ ] T-008 — 建立 `scripts/providers/Test-Provider.ps1` smoke test 腳本。接受 `-Provider` 參數，對指定 CLI 執行一個極簡的 echo 任務（如 `"回覆 OK"`），驗證 CLI 可用性、無頭執行能力與 Exit Code 正確性。
- [ ] T-009 — 更新文件：(a) `commands/gal-pipeline/SKILL.template.md` 在 `### Same-Runtime Fallback Contract` 之後、`### Runtime Step-Budget Preflight` 之前新增 `### Headless CLI Dispatch` 段落（含 bypass-permission 安全警告）；(b) `docs/personalization.md` 新增 Provider Routing 設定指引；(c) `model-roles.md` 與 `model-roles.example.md` 新增 `provider-routing.ndjson` 說明。

> **Deferred**: Bash 版本 (`invoke-provider.sh`, `claude.sh`, `opencode.sh`, `gemini.sh`, `new-task-spec.sh`, `gal.sh` 對應修改) 延後至有 Bash 主機可驗證時實作，遵循架構審查 OE-01 建議。
