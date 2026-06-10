# Plan: 小 Context 模型可靠完成單一 task —— 自足 spec + 派工可達 + 品質閘可驗

## Approval

- Human approval: [pending]
- Architect review: **PARTIAL** — 原 R-001..R-004（template/自足 spec/擷取器）已 **APPROVE（2026-06-05）**；本次整合新增 R-005..R-008（派工 preflight、honest-pass 閘、測試陷阱守衛、refactor 自清）**尚待 architect 再審**（範圍擴大，須 `/deep-planning`）。
- Additional domain review: [not requested]

## Goal

讓 claude-haiku、gpt-5.4-mini、gemini-3.5-flash 這類**小/廉價模型**，在 `/gal pipeline` 被派工時，**只憑單一 task spec 就能正確、可驗地完成該 task**。原計畫聚焦「spec 可讀 + 自足」；本次整合擴大為**端到端可靠派工**：

1. **template 可讀性**：`templates/plan.md` 把 `## Approval` 移到最上層。
2. **task 自足化（核心）**：`/refining-plan` 的 `## Tasks` 改為自足、受預算約束的執行單元，派工層（`New-TaskSpec.ps1`）能完整擷取自足內容。
3. **派工可達（新）**：`gal-dispatch` 的 executor 必須 preflight 認證 + headless 能力，缺失時 fail-loud 附修復指引——不可靜默降級。
4. **品質閘可驗（新）**：spec 契約必須令 executor 跑聚焦 probe 並回報**真實證據**（executor-log 終態 `completed` + 可觀察 write-back），且避開已知測試陷阱、自清 refactor 殘留。

> **拆分說明（2026-06-05）**：原第三面向「review 模型指派 / sender==reviewer」已拆出為 [`refactor-golem-auditor.md`](refactor-golem-auditor.md)。
> **整合說明（2026-06-10）**：本次將 R-05/R-06/R-07（Rust port）review 期間反覆觀察到的派工/品質問題併入本計畫，詳見 `## Problems Observed`。其 **Rust 化部分**歸 [`refactor-gal-xmachine-rust-port.md`](refactor-gal-xmachine-rust-port.md)（pipeline crate 擁有 task-spec 與 preflight）；本計畫負責**規格契約 + 近期 shell 層修補**，作為該 Rust port 的規格來源，詳見 `## Cross-Plan Coordination`。

完成時必須成立：

- 新計畫的 `## Approval` 置頂。
- `/refining-plan` 產出的每個 task 自足、受預算約束，`New-TaskSpec.ps1` 能完整擷取多行 task 區塊 + **該 task 專屬**檔案。
- `gal-dispatch`／`gal doctor` 能偵測「executor 在 PATH 但未認證／不能 headless」並 fail-loud 附明確修復步驟。
- task 驗收契約要求回報真實 probe 證據（非裸 "tests pass"），且不踩 Windows installer-detection 檔名陷阱。

> **關鍵收斂（2026-06-05 架構審查）**：三模型 window 為 200K / 400K / 1M——spec 大小限制不是被 window 綁住，而是被「聚焦 + 成本」綁住。「自足」指**內嵌指令 + 精確指標**，而非檔案全文。

## Problems Observed（R-05/R-06/T-031/T-032 review 實證）

> 本次三輪 code review 發現的反覆性問題。每條都有 commit/log 佐證；這些是 R-005..R-008 的設計依據。

- **[OBS-1 派工根因] copilot 未認證 → 靜默 disconnected-partial**。`executor-routing.json` 將 CODER/TESTER 派給 `copilot`；本機 `copilot.exe` 在 PATH 但**未認證**（`Error: No authentication information found`），每次派工 exit 1 `disconnected-partial`、`session_id=none`。`gal-dispatch` 的 `is_available()` 只跑 `where copilot`（檢查 PATH），**不檢查認證/headless**，故 `gal doctor` 不告警。使用者因此「發不出去」，只能手動開 Copilot chat——這是本次需手動 review 的源頭。實證：`.dev/executor-logs/1781075693-DIAG-01-implement-copilot.log`。
- **[OBS-2 spec 截斷] `New-TaskSpec.ps1` 擷取器仍壞**。`$taskGoal` 仍 `Select-Object -First 1`（只取首行，`New-TaskSpec.ps1:103-104`），`## Affected Files` 仍 dump 整份 `## Files to Create or Modify`（`:113-118`）。即使派工成功，多行自足 task 會被截成首行 + 跨 task 噪音——R-002/R-004 形同虛設（與既有 BUG-01/02 同源，現確認未修）。
- **[OBS-3 測試陷阱重犯×2] Windows installer-detection 檔名陷阱**。`gal-engine` 測試 binary 檔名含 `install`/`setup`/`update`/`patch` 子字串會觸發 UAC（os error 740，需提權）而無法執行；`gal-engine` 無 asInvoker manifest。R-05 `install_family_r05`、R-06 `uninstall_r06`（"uninstall" 含 "install"）連兩次中招。更嚴重：經 pipe/RTK 代理時 cargo 的非零退出被讀成 0，**靜默遮蔽測試失敗**。
- **[OBS-4 謊報 PASS] honest-test-pass-bar 未被強制**。R-06 prompt 聲稱「`cargo test --test uninstall_r06` passes」，但該 probe 在 Windows 根本無法執行——PASS 不實。pipeline 沒有任何機制要求 PASS 聲稱須有 executor-log 終態 `completed` + 可觀察 write-back 背書。
- **[OBS-5 refactor 殘留 + 過期 caveat]** R-05 留下死碼 `build_shared_args`（T-024 改接後孤兒）與空轉測試（引用已刪腳本）；過期 `link.exe` caveat 從 R-05 被複製到 R-06（連錯兩次）。executor 未自清，靠 reviewer 事後攔。

## Requirements

> R-001..R-004 為原計畫（architect APPROVE 2026-06-05）；R-005..R-008 為本次整合新增（待再審）。

- [ ] **R-001 — Approval 置頂**：`templates/plan.md` 的 `## Approval` 移到標題之後、`## Goal` 之前；以區段名稱解析的命令行為不變。
- [ ] **R-002 — 自足 task 撰寫契約（指標式，非全文）**：`/refining-plan` 的 `## Tasks` 升級為每個 `T-NNN` 須含 (a) 確切目標檔路徑、(b) 具體改動、(c) 就地可驗的驗收條件、(d) 必要慣例/簽章/相依指標。不得內嵌檔案全文。
- [ ] **R-003 — spec 預算與切分**：每 task 套用 `<5KB` 量級指令/指標預算（理由＝聚焦+成本，非 window）；超限者切分為更小原子 task。
- [ ] **R-004 — 派工層擷取自足內容**：`New-TaskSpec.ps1` 擷取 `T-NNN` **完整多行區塊**（非首行），`## Affected Files` 收斂為**該 task 專屬**檔案（無指名才回退整段）。Rust bin 不變。修 OBS-2（即既有 BUG-01/02）。
- [ ] **R-005 — executor 派工 preflight（fail-loud，只檢查不代設）**：`gal-dispatch` offload 前須驗 executor **已認證且可 headless**（非僅在 PATH）；缺失時以明確修復指引 fail-loud（例：`copilot` 未認證 → 提示 `/login` 或設 `GH_TOKEN`），不靜默降級為 disconnected-partial。同步以 `base::HealthCheck` 暴露，令 `gal doctor` 聚合（對齊 T-031 模式）。修 OBS-1。**Rust 版折入 xmachine R-05**，見 Cross-Plan。
- [ ] **R-006 — honest-pass 驗收契約**：`/refining-plan` 的 task 驗收須要求 executor 跑**聚焦 probe** 並回報**真實終態證據**——executor-log 終態 `completed` + 可觀察 write-back（對齊 `Dispatch:` marker / `terminal_state`），禁止裸「tests pass」。pipeline gate 對缺證據的 PASS 聲稱視為未通過。修 OBS-4。
- [ ] **R-007 — 測試陷阱守衛**：task/spec 契約 + 輕量 CI/lint 須防 Windows installer-detection 檔名陷阱——`crates/**/tests/*.rs` 檔名不得含 `install`/`setup`/`update`/`patch` 子字串（或該 crate 補 asInvoker manifest）；測試結果擷取不得讓 spawn 失敗（含 740）經 pipe 被讀成 exit 0。修 OBS-3。
- [ ] **R-008 — refactor 自清契約**：task 契約要求 executor 在本 task 範圍內清除被改動孤兒化的碼/測試（死 helper、引用已刪檔的測試），並更新而非複製過期 caveat；reviewer gate 檢查之。修 OBS-5。

## Scope

**In scope**：`templates/plan.md` 區段重排（R-001）；`commands/refining-plan/SKILL.template.md` 契約升級（R-002/003/006/007/008）；`scripts/common/New-TaskSpec.ps1` 擷取升級（R-004）；`gal-dispatch`／doctor 的 executor preflight HealthCheck（R-005，近期 shell/Rust-bin 層最小實作）。

**Out of scope**：review 模型指派 / golem 合併（→ `refactor-golem-auditor.md`）；pipeline/task-spec 的**完整 Rust 化**（→ `refactor-gal-xmachine-rust-port.md` R-02）；遠端 SSH/zellij preflight（→ xmachine R-05）；`executor-routing.json` 結構變更；舊計畫回溯改寫；`New-TaskSpec` 的 Bash 對等。

> **受保護路徑警示**：R-001（`templates/`）、R-002/006/007/008（`commands/refining-plan/`）觸及受保護路徑；R-005 觸及 `gal-dispatch`（核心派工）——三者均須 `/deep-planning` architect 放行。`New-TaskSpec.ps1` 非受保護腳本。

## Cross-Plan Coordination

本計畫與 [`refactor-gal-xmachine-rust-port.md`](refactor-gal-xmachine-rust-port.md) 在 **task-spec 與 preflight** 兩處交會。所有權劃分（避免雙重實作）：

| 主題 | 本計畫（規格 + shell 近期修） | xmachine 計畫（Rust 終態） |
| --- | --- | --- |
| task-spec 自足契約（R-002/003/006/007/008） | 寫進 `refining-plan` SKILL；為 Rust 規格來源 | R-02：`pipeline` crate task-spec 吸收此契約 |
| 擷取器多行 + 每 task 檔案（R-004） | 升級 `New-TaskSpec.ps1` | R-02：Rust 重寫沿用此規格 |
| executor preflight（R-005） | `gal-dispatch`/doctor 近期最小 HealthCheck（本地 executor 認證/headless） | R-05：`HealthCheck` 統一暴露（原為 SSH/zellij/遠端 gal；本地 executor 一併納入） |
| honest-pass 閘（R-006） | `refining-plan` 契約 + `/gal-pipeline` skill gate | dispatch `terminal_state` 已提供證據面，pipeline crate 強制 |

**排序**：本計畫的 shell 層修補不阻斷 xmachine（後者依賴核心 R-00）；兩計畫同改 `New-TaskSpec.ps1`（本計畫改擷取器、auditor 改 agentMap），實作時對齊先後。

## Approach

### 面向一：template Approval 置頂（低風險，獨立）
- **Files**: `[MODIFY] templates/plan.md`（受保護）
- **What/Verify**: 整段上移；`/planning` 驗置頂，既有命令以區段名解析不受影響。

### 面向二：自足 task 契約 + 擷取（核心，R-002/003/004）
- **Files**: `[MODIFY] commands/refining-plan/SKILL.template.md`（受保護）、`[MODIFY] scripts/common/New-TaskSpec.ps1`
- **What**: refining-plan 加指標式自足 + 預算/切分；`New-TaskSpec.ps1` 擷取完整 `T-NNN` 區塊 + 每 task 檔案收斂；保留 `<5KB` 檢查。

### 面向三：派工可達 preflight（R-005）
- **Files**: `[MODIFY] crates/dispatch/src/dispatch.rs`（`is_available` 旁加 `is_dispatch_ready`：認證/headless 探測）、`[MODIFY] crates/dispatch/src/main.rs`（preflight gate 改 fail-loud 附指引）、`[MODIFY]` doctor 聚合（executor-routing HealthCheck）
- **What**: 對 routing 指名的 executor 做最小認證探測（如 copilot 的 token/login 狀態），未就緒 → 明確指引、非靜默降級；以 HealthCheck 入 `gal doctor`。
- **Verify**: 未認證 executor → `gal doctor` 告警 + dispatch 退出附修復步驟；已認證 → 正常 offload 並留 executor-log `completed`。

### 面向四：品質閘契約（R-006/007/008）
- **Files**: `[MODIFY] commands/refining-plan/SKILL.template.md`（受保護）、`[MODIFY] plugins/gal-core/commands/gal-pipeline/SKILL.template.md`（honest-pass gate 文字）、`[ADD]` 輕量測試檔名 lint（CI 或 pre-commit）
- **What**: 驗收須回報真實 probe 證據；測試檔名守衛；refactor 自清條款。

## Files to Create or Modify

- `[MODIFY] templates/plan.md`（受保護）— Approval 置頂
- `[MODIFY] commands/refining-plan/SKILL.template.md`（受保護）— 自足 + 預算 + honest-pass + 陷阱守衛 + 自清契約
- `[MODIFY] plugins/gal-core/commands/gal-pipeline/SKILL.template.md`（受保護）— honest-pass gate
- `[MODIFY] scripts/common/New-TaskSpec.ps1` — 多行區塊擷取 + 每 task 檔案收斂
- `[MODIFY] crates/dispatch/src/dispatch.rs`、`crates/dispatch/src/main.rs` — executor 認證/headless preflight，fail-loud
- `[MODIFY]` doctor 聚合（`crates/cli/src/main.rs` 或 domain HealthCheck）— executor-routing 就緒檢查
- `[ADD]` 測試檔名陷阱守衛（lint/CI）

## Test Cases

- [ ] `/planning` 新計畫 `## Approval` 置頂
- [ ] `/refining-plan` 對既有計畫仍以區段名正確寫回
- [ ] `New-TaskSpec.ps1` 對多行自足 task 擷取完整區塊（非首行）
- [ ] `New-TaskSpec.ps1` 的 `## Affected Files` 僅含該 task 指名檔案
- [ ] 未認證 executor → `gal doctor` 告警 + `gal-dispatch` fail-loud 附修復步驟（不留 disconnected-partial 假象）
- [ ] 已認證 executor → dispatch offload 成功並留 executor-log 終態 `completed`
- [ ] 含 `install`/`setup`/`update`/`patch` 子字串的新測試檔名被 lint 擋下
- [ ] task 驗收契約要求回報真實 probe 證據（裸 "tests pass" 不通過 gate）

## Success Criteria

- [ ] 核准狀態置頂可一眼掃讀。
- [ ] 「只給單一 spec、executor 自讀指名檔」情境下，小模型能正確、受預算約束完成 task。
- [ ] 派工到其他平台**真能送達或明確告知為何不能**（不再靜默「發不出去」）。
- [ ] 測試陷阱與謊報 PASS 在 spec/gate 層被擋，而非靠事後 review。

## Risks

- **受保護路徑 blast radius**：`templates/`、`commands/refining-plan/`、`gal-pipeline` SKILL、`gal-dispatch`。緩解：`/deep-planning` 再審；逐檔最小化；面向一可獨立先行。
- **`New-TaskSpec.ps1` 擷取正則邊界**：多行區塊邊界易誤切。緩解：明確邊界規則 + 單元測試覆蓋多/單/末尾 task。
- **executor preflight 的探測成本/誤判**：認證探測可能因工具差異誤報。緩解：最小化探測（優先讀 token/login 狀態而非實際 spawn）；fail-loud 訊息明確、可被使用者忽略覆寫。
- **與 xmachine 雙重實作**：preflight/task-spec 兩處交會。緩解：見 Cross-Plan 所有權表；本計畫只做規格 + shell 近期修，Rust 終態歸 xmachine。
- **honest-pass 與自足 token-budget 張力**：要求回報證據會略增 spec。緩解：證據＝指標（log 路徑 + 終態關鍵字），非貼全文。

## Open Questions

- [x] OQ-002 — task spec 大小上限？**已解**：三模型 window 200K/400K/1M，非容量瓶頸；預算依聚焦+成本設 `<5KB` 指令/指標。 *(architecture-review)*
- [x] OQ-003 — dispatch spec 在何處組裝？**已解**：`New-TaskSpec.ps1`（gal.ps1 呼叫，產 spec 後 `Get-Content | bin` 餵 stdin）；bin 僅轉送 stdin。 *(codebase-trace)*
- [ ] OQ-004 — R-005 executor preflight 的**近期落點**：(a) 加在 `gal-dispatch` bin（Rust，立即生效於所有派工）、(b) 加在 `gal.ps1`/`New-TaskSpec` shell 層、(c) 只做 `gal doctor` HealthCheck 告警不擋 dispatch？**建議 (a)+(c)**：bin 內 fail-loud + doctor 告警，避免靜默 disconnected-partial。 *(raised by: 2026-06-10 整合)*
- [ ] OQ-005 — R-005 的 Rust preflight 是否**完全併入 xmachine R-05**、本計畫只留契約與 doctor 文字？或本計畫先在 `dispatch` crate 落最小本地-executor preflight，xmachine 再擴遠端？**建議後者**（本地 executor 阻塞當前工作，值得先修）。 *(raised by: 2026-06-10 整合，與 xmachine 協調)*
- [ ] OQ-006 — honest-pass gate（R-006）強制力：pipeline 對「無 executor-log 證據的 PASS」是硬擋（BLOCK）或軟告警？**建議硬擋**，與 honest-test-pass-bar 一致。 *(raised by: 2026-06-10 整合)*

<!-- Format: - [ ] OQ-NNN — description *(raised by: command)* -->

## Review Results

### Architecture Review

#### Verdict: APPROVE *(2026-06-05, 僅涵蓋 R-001..R-004)*

方向成立、範圍可控、依賴已釐清。原計畫兩處承重缺陷已修：(1)「自足」改為指標式（非全文內嵌）；(2) spec 組裝點定位至 `New-TaskSpec.ps1`（現行擷取器只取首行）。

> **再審待辦（2026-06-10）**：R-005..R-008（派工 preflight、honest-pass 閘、陷阱守衛、自清契約）擴大範圍並觸及 `gal-dispatch` 核心與 `gal-pipeline` 契約面，須 `/deep-planning` architect 再審，並與 xmachine 計畫對齊 preflight/task-spec 所有權後，方可進 `/refining-plan`。

#### Trade-off Summary（R-001..R-004，原審）

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| 指標式自足 task（非全文） | 小 spec、低成本、聚焦 | refining-plan 撰寫需更嚴謹 | OK |
| `<5KB` 預算理由＝聚焦+成本 | 與真實瓶頸對齊 | 需教育撰寫者 | OK |
| `New-TaskSpec.ps1` 擷取多行 + 每 task 檔案 | 自足內容真正送達 | 正則邊界風險 | OK（BUG-01/02 納入 R-004） |

#### Bug Surface（原審，現經 R-05/R-06 review 確認仍存在）

- **[BUG-01] High（R-004，OBS-2 確認未修）**：`New-TaskSpec.ps1` `Select-Object -First 1` 截斷多行 task。
- **[BUG-02] Medium（R-004，OBS-2 確認未修）**：`## Affected Files` dump 整份檔案清單。

#### What's Good (keep these)

- 面向一（Approval 置頂）低風險、可先行。
- 預算理由錨定「聚焦+成本」而非 window 容量。
- review 模型重組拆出獨立計畫。

### Business Review
Not requested。

### Design Review
Not requested（無 customer-facing UI）。

### Engineering Review

Pending（`## Tasks` 與 `## Test Plan` 仍為 placeholder；R-005..R-008 須先過 `/deep-planning` 再審，再 `/refining-plan`）。

## Test Plan

Pending（待 `/refining-plan`）。

## Tasks

Pending（待再審 + `/refining-plan`）。
