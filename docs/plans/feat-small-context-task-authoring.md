# Plan: 小 Context 模型可靠完成單一 task —— 自足 spec + 派工可達 + 品質閘可驗

## Approval

- Human approval: **approved 2026-06-10**（OQ-004/005/006 由使用者裁決,見 Decisions）
- Architect review: **APPROVE-with-conditions（2026-06-10 全面 deep-planning,C1..C4,見 ## Review Results）** — R-001..R-004 沿用 2026-06-05 原審 APPROVE;R-005..R-008 本次審畢。
- Additional domain review: [not requested]
- Engineering review: **CLEAR（2026-06-10,見 ## Review Results）** — 8 T-NNN + 11 TP;下一步 `/plan-to-prompt`。

## Goal

讓 claude-haiku、gpt-5.4-mini、gemini-3.5-flash 這類**小/廉價模型**,在 `/gal pipeline` 被派工時,**只憑單一 task spec 就能正確、可驗地完成該 task**。四個面向:

1. **template 可讀性**:`templates/plan.md` 把 `## Approval` 移到最上層。
2. **task 自足化(核心)**:`/refining-plan` 的 `## Tasks` 改為自足、受預算約束的執行單元,派工層(`New-TaskSpec.ps1`)能完整擷取自足內容。
3. **派工可達**:`gal-dispatch` 對 routing 指名的本地 executor preflight 認證/headless 能力,缺失 fail-loud 附修復指引,並入 `gal doctor`。
4. **品質閘可驗**:task 驗收契約要求真實 probe 證據(executor-log 終態 + 可觀察 write-back),pipeline 對無證據的 PASS **硬擋**;同時防測試檔名 UAC 陷阱、要求 refactor 自清。

完成時必須成立:

- 新計畫的 `## Approval` 置頂。
- `/refining-plan` 產出的每個 task 自足、受預算約束;`New-TaskSpec.ps1` 擷取完整多行 task 區塊 + 該 task 專屬檔案。
- 未認證/不能 headless 的 routed executor → `gal-dispatch` fail-loud 附明確修復步驟、`gal doctor` 告警;不再靜默 `disconnected-partial`。
- 無證據 PASS 被 pipeline 視為未通過(硬擋);`gal-engine` 測試檔名陷阱被守衛擋下。

## Decisions（已定,取代原 OQ-002..OQ-006）

| # | 決策 | 依據 |
| --- | --- | --- |
| D-1 | **spec 預算 `<5KB` 量級**,理由=聚焦+每次派工成本,非 window 容量(三模型 window 200K/400K/1M);「自足」=內嵌指令+精確指標,**不內嵌檔案全文**,executor 自讀指名檔 | 原 OQ-002,架構審查 2026-06-05 |
| D-2 | **spec 組裝點 = `scripts/common/New-TaskSpec.ps1`**(gal.ps1 呼叫,產 spec 後餵 stdin);Rust bin 只轉送不組裝 | 原 OQ-003,codebase-trace |
| D-3 | **preflight 落點 = `gal-dispatch` bin 內 fail-loud + `gal doctor` HealthCheck 告警**(a+c 並行;不靜默降級) | 原 OQ-004,使用者裁決 2026-06-10 |
| D-4 | **本計畫先在 `dispatch` crate 落最小「本地 executor」preflight;xmachine 計畫再擴遠端**(SSH/zellij/遠端 gal 歸 xmachine R-05) | 原 OQ-005,使用者裁決 2026-06-10 |
| D-5 | **honest-pass gate = 硬擋(BLOCK)**:pipeline 對無 executor-log 證據的 PASS 聲稱一律視為未通過,與 honest-test-pass-bar 一致 | 原 OQ-006,使用者裁決 2026-06-10 |

## Evidence（設計依據,實證摘錄）

R-05/R-06/T-031/T-032 review 期間實證(細節在各 commit 與 prompt `## Review Results`):

- **派工根因**:routing 把 CODER/TESTER 派給 `copilot`,本機 copilot CLI 在 PATH 但未認證 → 每次 dispatch `disconnected-partial`、`session_id=none`。`is_available()` 只跑 `where`,不驗認證;doctor 不告警。→ R-005。
- **spec 截斷**:`New-TaskSpec.ps1:103` 仍 `Select-Object -First 1`(task 只取首行)、`## Affected Files` dump 整份計畫檔案清單。→ R-004(原 BUG-01/02)。
- **測試陷阱連犯兩次**:`gal-engine` 測試 binary 檔名含 `install` 子字串觸發 Windows UAC(os error 740)無法執行(R-05 `install_family_r05`、R-06 `uninstall_r06`),且經 pipe 時非零退出被讀成 0,**靜默遮蔽失敗**;R-06 並據此謊報 PASS。→ R-006/R-007。
- **refactor 殘留**:死 helper(`build_shared_args`)、引用已刪腳本的空轉測試、過期 caveat 跨 phase 複製。→ R-008。

## Requirements

- [ ] **R-001 — Approval 置頂**:`templates/plan.md` 的 `## Approval` 移到標題之後、`## Goal` 之前;以區段名稱解析的命令(`/refining-plan`、`/plan-to-prompt`、`/deep-planning`、`New-TaskSpec.ps1`)行為不變。
- [ ] **R-002 — 自足 task 撰寫契約(指標式)**:`/refining-plan` 的每個 `T-NNN` 須含 (a) 確切目標檔路徑、(b) 具體改動、(c) 就地可驗的驗收條件、(d) 必要慣例/簽章/相依指標。不得內嵌檔案全文(D-1)。
- [ ] **R-003 — spec 預算與切分**:每 task `<5KB` 量級指令/指標預算;超限切分為原子 task(D-1)。
- [ ] **R-004 — 派工層擷取自足內容**:`New-TaskSpec.ps1` 擷取 `T-NNN` 完整多行區塊(至下一 `T-NNN`/區段邊界);`## Affected Files` 收斂為該 task 指名檔案(無指名才回退整段)。Rust bin 不變(D-2)。
- [ ] **R-005 — 本地 executor 派工 preflight(fail-loud)**:`dispatch` crate 在 offload 前對 routed executor 驗**認證/headless 就緒**(非僅 PATH);「確定未認證」→ fail-loud 附該工具修復步驟(如 copilot:`/login` 或 `GH_TOKEN`);「無法判定」→ 放行 + 警告(不可因探測不可靠而誤擋,見 C1)。同步以 `base::HealthCheck` 暴露 routed-executor 就緒狀態入 `gal doctor`(D-3/D-4)。
- [ ] **R-006 — honest-pass 驗收契約(硬擋)**:`/refining-plan` task 驗收須指明聚焦 probe 與證據形態(executor-log 終態 `completed` + 可觀察 write-back 指標);`/gal pipeline` 對無證據 PASS 一律視為未通過並走 retry/handoff(D-5)。
- [ ] **R-007 — 測試陷阱守衛**:加一個 `gal-engine` 內的自掃單元測試:`crates/gal-engine/tests/` 檔名不得含 `install`/`setup`/`update`/`patch` 子字串(或該 crate 已有 asInvoker manifest 則放行);`/refining-plan` 測試命名指引同步。確保 spawn 失敗(含 740)不被 pipe 讀成 exit 0 的注意事項寫入 tester 契約。
- [ ] **R-008 — refactor 自清契約**:`/refining-plan` task 契約加入:改動孤兒化的碼/測試(死 helper、引用已刪檔的測試)由同 task 清除;caveat 須更新不得跨 phase 複製;reviewer gate 檢查。

## Scope

**In scope**:`plugins/gal-core/templates/plan.md` 區段重排;`plugins/gal-core/commands/refining-plan/SKILL.template.md` 契約升級(R-002/003/006/007/008);`plugins/gal-core/commands/gal-pipeline/SKILL.template.md` honest-pass gate 文字;`scripts/common/New-TaskSpec.ps1` 擷取升級(R-004);`crates/dispatch` 本地 executor preflight + doctor HealthCheck(R-005);`gal-engine` 測試檔名自掃測試(R-007)。

**Out of scope**:review 模型指派 / golem 合併(→ `refactor-golem-auditor.md`);pipeline/task-spec 完整 Rust 化與**遠端** preflight(→ `refactor-gal-xmachine-rust-port.md` R-02/R-05);`executor-routing.json` 結構變更;舊計畫回溯改寫;`New-TaskSpec` Bash 對等;實際幫使用者認證任何 executor(憑證屬使用者)。

> **受保護路徑**:`templates/`、`commands/refining-plan/`、`commands/gal-pipeline/`、`crates/dispatch`(核心派工)。本次 deep-planning 已審(見 Review Results);實作期保護路徑任務按慣例逐項簽核。

## Cross-Plan Coordination

| 主題 | 本計畫 | 對手計畫 |
| --- | --- | --- |
| task-spec 自足契約(R-002/003/006/007/008) | 寫進 refining-plan/gal-pipeline SKILL,為 Rust 規格源 | xmachine R-02/T-004:`pipeline` crate task-spec 吸收此契約 |
| 擷取器升級(R-004) | 升級 `New-TaskSpec.ps1` | xmachine T-004 Rust 重寫沿用;auditor R-006 改同檔 agentMap(**本計畫先落,auditor rebase**) |
| executor preflight(R-005) | `dispatch` crate 本地 executor 最小實作 + doctor | xmachine R-05/T-008 擴遠端(SSH/zellij/遠端 gal),同一 `HealthCheck` 模式 |
| honest-pass 閘(R-006) | refining-plan 契約 + gal-pipeline skill 硬擋文字 | dispatch `terminal_state` 已供證據面;xmachine pipeline crate 日後內建強制 |

**施作順序(跨計畫路線見 `.dev/state.md ## Cross-Plan Route`)**:本計畫 → auditor → xmachine 對應 port。

## Approach

### 面向一:template Approval 置頂(R-001,低風險獨立)
- **Files**: `[MODIFY] plugins/gal-core/templates/plan.md`(受保護)
- **Verify**: `/planning` 新計畫置頂;既有命令以區段名解析不受影響。

### 面向二:自足 task 契約 + 擷取(R-002/003/004,核心)
- **Files**: `[MODIFY] plugins/gal-core/commands/refining-plan/SKILL.template.md`(受保護)、`[MODIFY] scripts/common/New-TaskSpec.ps1`
- **What**: refining-plan 加指標式自足 + 預算/切分;擷取器升級多行區塊 + 每 task 檔案收斂;保留 `<5KB` 檢查。
- **Verify**: 多行 task 擷取完整;Affected Files 僅該 task;單元測試覆蓋多/單/末尾 task 邊界。

### 面向三:本地 executor preflight(R-005)
- **Files**: `[MODIFY] crates/dispatch/src/dispatch.rs`(`is_available` 旁加 per-executor 就緒探測)、`[MODIFY] crates/dispatch/src/main.rs`(gate 訊息)、`[MODIFY]` doctor 聚合(`crates/cli/src/main.rs`,沿 T-031 模式)
- **What**: 探測順序=便宜優先(env token / config 檔 / CLI status 子命令),絕不為探測 spawn 完整 executor;三態語意:`ready` 放行、`definitely-unauthenticated` fail-loud 附指引、`unknown` 放行+警告。doctor 只檢查 routing 指名的 executor。
- **Verify**: 未認證 copilot → dispatch 退出附 `/login`/`GH_TOKEN` 指引 + doctor 告警;已認證 → 正常 offload 留 executor-log `completed`;探測不可判 → 放行附警告。

### 面向四:品質閘(R-006/007/008)
- **Files**: `[MODIFY] plugins/gal-core/commands/refining-plan/SKILL.template.md`、`[MODIFY] plugins/gal-core/commands/gal-pipeline/SKILL.template.md`(受保護)、`[ADD] crates/gal-engine/tests/` 檔名自掃測試
- **What**: 驗收契約=probe+證據指標;pipeline 硬擋無證據 PASS;檔名守衛為 crate 內單元測試(零 CI 基礎設施);自清條款。

## Files to Create or Modify

- `[MODIFY] plugins/gal-core/templates/plan.md`(受保護)— R-001
- `[MODIFY] plugins/gal-core/commands/refining-plan/SKILL.template.md`(受保護)— R-002/003/006/007/008
- `[MODIFY] plugins/gal-core/commands/gal-pipeline/SKILL.template.md`(受保護)— R-006 硬擋
- `[MODIFY] scripts/common/New-TaskSpec.ps1` — R-004
- `[MODIFY] crates/dispatch/src/dispatch.rs`、`crates/dispatch/src/main.rs` — R-005 preflight
- `[MODIFY] crates/cli/src/main.rs`(或 dispatch 內 HealthCheck)— R-005 doctor 聚合
- `[ADD] crates/gal-engine/tests/<檔名自掃>.rs` — R-007(檔名本身避開陷阱字)

## Test Cases

- [ ] `/planning` 新計畫 `## Approval` 置頂;既有命令區段名解析不變
- [ ] `New-TaskSpec.ps1` 多行 task 擷取完整(多/單/末尾 task 邊界)
- [ ] `## Affected Files` 僅含該 task 指名檔案;無指名回退整段
- [ ] 未認證 routed executor → dispatch fail-loud 附該工具修復步驟;doctor 告警
- [ ] 探測無法判定 → 放行 + 警告(不誤擋)
- [ ] 已認證 → offload 成功,executor-log 終態 `completed`
- [ ] 含 `install`/`setup`/`update`/`patch` 子字串的 `gal-engine` 測試檔名被自掃測試擋下
- [ ] pipeline 對無證據 PASS 走 retry/handoff(硬擋),不推進

## Success Criteria

- [ ] 核准狀態置頂可一眼掃讀。
- [ ] 「只給單一 spec、executor 自讀指名檔」情境下,小模型能正確完成 task 且受預算約束。
- [ ] 派工**真能送達或明確說出為何不能**;不再靜默 disconnected-partial。
- [ ] 謊報 PASS 與測試檔名陷阱在 gate/守衛層被擋,不靠事後人工 review。

## Risks

- **受保護路徑 blast radius**(templates/refining-plan/gal-pipeline/dispatch):逐檔最小化;面向一可獨立先行;實作期逐項簽核。
- **擷取器正則邊界**:多行邊界易誤切。緩解:明確邊界規則 + 單元測試。
- **preflight 探測誤判**:工具差異大,誤擋比漏擋傷害大。緩解:三態語意(C1),只在「確定未認證」才擋。
- **同檔協調**(`New-TaskSpec.ps1` 與 auditor):本計畫先落,auditor rebase(route 已定)。
- **honest-pass 與 spec 預算張力**:證據=指標(log 路徑+終態關鍵字),非貼全文。

## Review Results

### Architecture Review

#### Verdict: APPROVE-with-conditions *(2026-06-10 全面 deep-planning;R-001..R-004 沿用 2026-06-05 原審)*

四面向方向正確,範圍可控。R-005..R-008 將「review 期間靠人攔」的反覆失敗模式(未認證靜默降級、謊報 PASS、檔名陷阱、refactor 殘留)前移到 gate/守衛層,層次正確:契約歸 SKILL、機制歸 dispatch crate、守衛歸 crate 內測試(零新基礎設施)。

**Conditions(實作必守):**

- **C1(R-005)三態探測語意**:只在「確定未認證」才 fail-loud;探測不可判 → 放行+警告。誤擋會把可用派工變不可用,比漏擋更傷。探測必須便宜(env/config/status 子命令),絕不 spawn 完整 executor。
- **C2(R-005)doctor 只查 routed executor**:不掃描所有已知工具,只查 `executor-routing.json` 實際指名者;routing 檔缺失=INFO 非 ERROR。
- **C3(R-006)硬擋語意限派工相位**:dispatched 相位無證據 PASS=未通過走 retry/handoff;DEGRADED_BUNDLED 人工模式的證據=Test Results 內可重現的指令輸出(沿既有慣例),不要求 executor-log。
- **C4(R-007)守衛=crate 內自掃測試**:不建 CI lint 基礎設施;一個單元測試掃自己 tests/ 目錄檔名即可,陷阱字清單與 asInvoker 豁免內建。

**Trade-off Summary**

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| 指標式自足 + `<5KB`(D-1) | 小 spec、低成本、聚焦 | 撰寫更嚴謹 | OK(原審) |
| preflight 在 bin+doctor(D-3) | 派工失敗變可見、可修 | dispatch crate 增探測面 | OK,C1/C2 約束 |
| 本地先行、遠端歸 xmachine(D-4) | 立即解本地阻塞,不重複實作 | 兩段交付 | OK,同 HealthCheck 模式銜接 |
| honest-pass 硬擋(D-5) | 謊報 PASS 不可能推進 | 證據要求增少量 spec | OK,C3 限縮範圍 |
| 檔名守衛=自掃測試 | 零基礎設施、隨 cargo test 跑 | 僅防已知陷阱字 | OK,C4 |

**Bug Surface**:擷取器邊界(已列 Test Cases);探測誤判(C1 三態);硬擋誤殺人工模式(C3 排除)。
**Over-engineering check**:無 CI lint、無新 store、無 executor 自動認證(憑證歸使用者)——皆已排除,最小面成立。

### Business / Design Review

Not requested(無 customer-facing)。

### Engineering Review

#### Verdict: CLEAR *(2026-06-10)*

8 個 T-NNN 對映 R-001..R-008,每個 commit-size 可獨立驗。task 自身遵守本計畫 R-002 自足契約(file:line 指標、具體改動、就地驗收、慣例指標,不貼全文)作自舉示範。關鍵排序:T-04(dispatch 探測 fn)→ T-05(doctor 用該 fn);T-03→T-06→T-08 共改 `refining-plan/SKILL.template.md`,須依序疊加不同契約子節。受保護路徑(templates/refining-plan/gal-pipeline/dispatch)實作期逐項簽核。architect C1..C4 已逐條映入對應 task 驗收。跨計畫:T-06 改 `gal-pipeline/SKILL.template.md`,auditor 計畫亦重構同檔——本計畫先落,auditor rebase(Route ①→②)。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-01 | manual | `/planning` 產生新計畫,`## Approval` 在 `# Plan:` 後、`## Goal` 前;對既有計畫跑 `/refining-plan`+`/plan-to-prompt`+`New-TaskSpec.ps1` 仍以區段名正確解析(位置無關) | T-01 |
| TP-02 | unit (Pester/PS) | `New-TaskSpec.ps1` 對含多行、子彈列表的 task 擷取完整區塊至下一 `T-NNN`/`##` 邊界;單 task、末尾 task、單行 task 三邊界皆正確 | T-02 |
| TP-03 | unit (Pester/PS) | `## Affected Files` 僅含該 task 區塊指名檔案;task 未指名檔案時回退整份 `## Files to Create or Modify` | T-02 |
| TP-04 | manual | `refining-plan/SKILL.template.md` 的 Step 3/4 含指標式自足規格(a 路徑/b 改動/c 驗收/d 慣例指標)+ `<5KB` 預算 + 超限切分規則;明示「不內嵌檔案全文」 | T-03 |
| TP-05 | unit (Rust) | `dispatch` 的 per-executor 就緒探測對「確定未認證」回 fail 態、「無法判定」回 unknown 態;探測不 spawn 完整 executor(以 mock/便宜路徑驗) | T-04 |
| TP-06 | integration (Rust) | routed executor 確定未認證 → `gal-dispatch` 退出附該工具修復步驟(copilot:`/login`/`GH_TOKEN`);unknown → 放行+stderr 警告;ready → 正常 offload | T-04 |
| TP-07 | unit (Rust) | `gal doctor` 只對 `executor-routing.json` 指名的 executor 產就緒 finding;routing 檔缺失=INFO 非 ERROR(C2) | T-05 |
| TP-08 | manual | `refining-plan/SKILL.template.md` 驗收契約要求聚焦 probe + 證據形態(executor-log 終態 `completed` + write-back 指標);`gal-pipeline/SKILL.template.md` 對無證據 PASS 硬擋走 retry/handoff;DEGRADED_BUNDLED 證據=Test Results 指令輸出(C3) | T-06 |
| TP-09 | unit (Rust) | `gal-engine` 自掃測試:對含 `install`/`setup`/`update`/`patch` 子字串的 `crates/gal-engine/tests/*.rs` 檔名 assert 失敗(該測試自身檔名避開陷阱字);crate 有 asInvoker manifest 則豁免(C4) | T-07 |
| TP-10 | manual | `refining-plan/SKILL.template.md` 含 refactor 自清條款(孤兒碼/測試同 task 清除、caveat 更新不跨 phase 複製)+ orchestrator/reviewer gate 檢查點 | T-08 |
| TP-11 | manual | `cargo test --workspace` 綠;`cargo clippy --workspace --all-targets` 0 warning(R-004/005/007 Rust 改動回歸) | T-02,T-04,T-05,T-07 |

## Tasks

> 每個 task 自足:含目標檔路徑、具體改動、就地驗收、慣例指標。受保護路徑(templates/refining-plan/gal-pipeline/dispatch)實作期逐項簽核。

- [x] **T-01 (R-001) — templates/plan.md Approval 置頂**
  - 檔案:`plugins/gal-core/templates/plan.md`(受保護)。現況:`## Approval` 在 line 51(`## Open Questions` 後、`## Review Results` 前);`## Goal` 在 line 3。
  - 改動:整段 `## Approval`(含 3 行 bullet)上移到 `# Plan: [Feature Name]` 之後、`## Goal` 之前;其餘區段順序不變。
  - 驗收:TP-01。`/planning` 新計畫 Approval 置頂;以區段名稱解析的命令(`/refining-plan` Step 1/3/4、`/plan-to-prompt`、`New-TaskSpec.ps1` 的 `## Files to Create or Modify`/`T-NNN` 掃描)不受位置影響——grep 確認這些解析器皆用區段名(`^## Approval` 等)非行號。
  - 慣例:markdown-formatting skill;區段重排不得改 bullet 內容。

- [x] **T-02 (R-004) — New-TaskSpec.ps1 擷取器升級(多行 + 每 task 檔案)**
  - 檔案:`scripts/common/New-TaskSpec.ps1`(非受保護)。現況 BUG:`$taskGoal` 在 line 103-104 用 `Where-Object {... $TaskScope ...} | Select-Object -First 1`(只取首行);`## Affected Files` 擷取 line 113-118 從 `## Files to Create or Modify` 整段。
  - 改動:(a) `$taskGoal` 改為擷取 `T-NNN` 起始行至下一個 `^\s*-\s*\[.?\]\s*T-\d` 或 `^##` 區段邊界的完整多行區塊(保留縮排子彈);(b) `## Affected Files` 改為從擷取出的 task 區塊內掃 backtick 包裹的檔案路徑(如 `` `path/to/file` ``),僅收斂該 task 指名者;task 內無指名路徑時才回退原 `## Files to Create or Modify` 整段。保留既有 `<5KB` 警告檢查。
  - 驗收:TP-02、TP-03、TP-11。新增 Pester 測試覆蓋多行/單行/末尾/單 task 邊界與每 task 檔案收斂。
  - 慣例:result-pattern 不適用(PS);structured-logging;邊界正則須註解化說明。**先於 auditor 計畫對同檔 agentMap 的改動(Route ①→②)。**

- [x] **T-03 (R-002+R-003) — refining-plan 自足 task + 預算契約**
  - 檔案:`plugins/gal-core/commands/refining-plan/SKILL.template.md`(受保護)。插入點:Step 3「Write ## Tasks」(line 42)、Step 4「Write ## Test Plan」(line 53)。
  - 改動:Step 3 加「指標式自足 task」規格——每 `T-NNN` 須含 (a) 確切目標檔路徑、(b) 具體改動、(c) 就地可驗驗收、(d) 慣例/簽章/相依指標;明示**不內嵌檔案全文,executor 自讀指名檔**(D-1)。加 spec 預算 `<5KB` 量級(理由=聚焦+派工成本,非 window)+ 超限切分為原子 task 規則。
  - 驗收:TP-04。本計畫自身的 `## Tasks`(本區段)即符合此契約,作自舉示範。
  - 慣例:markdown-formatting;token-budget convention(`<5KB` 理由須對齊)。

- [x] **T-04 (R-005) — dispatch crate 本地 executor preflight(三態)**
  - 檔案:`crates/dispatch/src/dispatch.rs`(現有 `is_available(name)` 在 line 283)、`crates/dispatch/src/main.rs`(safety gate 在 is_available 檢查處)。受保護(核心派工)。
  - 改動:新增 `fn executor_readiness(executor: &str) -> Readiness`(三態 enum `Ready`/`Unauthenticated{hint}`/`Unknown`),探測**便宜優先**:env token(如 copilot:`GH_TOKEN`/`COPILOT_GITHUB_TOKEN`)→ config 檔 → 工具 status 子命令;**絕不 spawn 完整 executor 跑 spec**。main.rs 在 `is_available` 通過後加 readiness gate:`Unauthenticated` → 印修復指引(copilot:run `/login` 或 set `GH_TOKEN`)並退出(reason=`executor-unauthenticated-confirmed`);`Unknown` → 放行 + stderr 警告(C1:不誤擋);`Ready` → 照常。
  - 驗收:TP-05、TP-06、TP-11。已知 copilot 未認證實證見 `## Evidence`。
  - 慣例:rust convention;result-pattern;C1 三態語意逐條對應。每 executor 的探測法以小 helper 隔離,未支援者預設 `Unknown`(放行)。

- [x] **T-05 (R-005) — doctor routed-executor 就緒 HealthCheck**
  - 檔案:`crates/cli/src/main.rs`(doctor 聚合在 line 258-273,T-031 模式:`report.findings.extend(...HealthCheck.check())`)。依賴 T-04 的 `executor_readiness`。
  - 改動:加一個 `HealthCheck`(實作 `base::health::HealthCheck`),讀 `~/.gal/config/executor-routing.json` 的角色→executor 映射,對**每個被指名的** executor 呼 `executor_readiness`,`Unauthenticated`→warning finding 附修復指引、`Unknown`→info、`Ready`→無 finding;routing 檔缺失=單一 INFO finding,非 ERROR(C2)。在 cmd_doctor 聚合處 extend。
  - 驗收:TP-07、TP-11。`gal doctor` 對未認證 copilot 告警且只查 routed executors。
  - 慣例:rust;沿 T-031 既有三個 HealthCheck 的聚合寫法(McpProjection/Setup/SkillsProjection)。

- [x] **T-06 (R-006) — honest-pass 硬擋契約**
  - 檔案:`plugins/gal-core/commands/refining-plan/SKILL.template.md`(受保護,疊在 T-03 後)、`plugins/gal-core/commands/gal-pipeline/SKILL.template.md`(受保護;現況 2d test gate line 391、2e review line 418)。
  - 改動:refining-plan 驗收契約加「每 task 須指明聚焦 probe + 證據形態(executor-log 終態 `completed` + 可觀察 write-back 指標)」。gal-pipeline 的 test/review gate 加硬擋規則:dispatched 相位回報 PASS 但無對應 executor-log 終態證據 → 視為未通過,走既有 retry/handoff,不推進(D-5)。C3:硬擋限 dispatched 相位;DEGRADED_BUNDLED 人工模式的證據=`## Test Results` 內可重現指令輸出(沿既有慣例),不要求 executor-log。
  - 驗收:TP-08。對齊 honest-test-pass-bar(memory `feedback_honest_test_pass_bar`)。
  - 慣例:markdown-formatting。**注意:gal-pipeline SKILL 亦被 auditor 計畫 R-002/R-004 重構——本計畫先落硬擋文字,auditor rebase 時保留之。**

- [x] **T-07 (R-007) — gal-engine 測試檔名陷阱自掃測試**
  - 檔案:`[ADD] crates/gal-engine/tests/<避開陷阱字的檔名>.rs`(如 `test_filename_guard.rs`——自身不含 `install`/`setup`/`update`/`patch`)。
  - 改動:一個單元/整合測試,讀 `crates/gal-engine/tests/` 目錄列出 `*.rs` 檔名,assert 無檔名(去掉 `.rs`)含 `install`/`setup`/`update`/`patch` 子字串(case-insensitive);內建 asInvoker 豁免註記(若該 crate 日後加 manifest 則改為跳過)。陷阱字清單為測試內常數。
  - 驗收:TP-09、TP-11。實證:R-05 `install_family_r05`、R-06 `uninstall_r06` 連兩次中招(見 `## Evidence`)。同步在 `golem-tester` 契約或 refining-plan 測試命名指引加一句:測試檔名避開該四字、spawn 失敗(含 os error 740)不得經 pipe 被讀成 exit 0。
  - 慣例:rust;C4 守衛=crate 內自掃測試,零 CI 基礎設施。

- [ ] **T-08 (R-008) — refactor 自清契約**
  - 檔案:`plugins/gal-core/commands/refining-plan/SKILL.template.md`(受保護,疊在 T-06 後)。
  - 改動:task 撰寫契約加「自清」條款:改動孤兒化的碼/測試(死 helper、引用已刪檔的測試)須由**同一 task** 清除;phase 間 caveat 須更新、不得原文複製過期內容;orchestrator/reviewer gate 須檢查殘留。
  - 驗收:TP-10。實證:R-05 死 `build_shared_args`、空轉 AGY 測試、`link.exe` caveat 跨 R-05→R-06 複製(見 `## Evidence`)。
  - 慣例:markdown-formatting。
