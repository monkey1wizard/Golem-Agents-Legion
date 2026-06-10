# Plan: golem-reviewer + golem-security 合併為 golem-auditor,正確性審查移入 orchestrator gate

## Approval

- Human approval: **approved 2026-06-10**(OQ-001 接受、OQ-002 命名確認,見 Decisions)
- Architect review: **APPROVE-with-conditions(2026-06-10 全面 deep-planning,C1..C5,見 ## Review Results)**
- Additional domain review: [not requested]
- Engineering review: **CLEAR（2026-06-10,見 ## Review Results）** — 4 T-NNN + 8 TP;下一步(Route ② · small-context 落地後)`/plan-to-prompt`。

## Goal

把 code review 的兩種職責沿「**對不對 vs 安不安全/深層問題**」切開,並重新指派執行者:

- **Orchestrator inline gate(正確性)**:跑 `/gal pipeline` 的會話本身,在收回 implement 結果當下,用其完整計畫 context 確認「這段 code 有沒有正確完成 `T-NNN`」——功能正確性、架構適配、程式碼品質、明顯效能。**不 dispatch**,是 implement 與 test 之間的 gate,通過才進 TESTER。
- **golem-auditor(深層 + 安全,永遠 dispatch)**:`golem-reviewer` 與 `golem-security` 合併的單一稽核 specialist,負責深度效能 + 全部安全審查,由 conditional 升格為**每 task 常態**。

完成時必須成立:

1. `golem-security` 改名擴範圍為 `golem-auditor`、`golem-reviewer` 刪除,無殘留死引用。
2. pipeline 相位:implement → **orchestrator 正確性 gate** → test → **auditor(常態)** → verify;dispatched REVIEWER 相位移除。
3. 職責切分文件化:orchestrator 做清單 1–13 + 明顯效能;auditor 做深度效能 + 安全(14–23)。
4. routing 鍵 `REVIEWER` → `AUDITOR`;dispatch 相位鍵 `review` → `audit`;引用全數更新或由 `gal sync` 重生。

## Decisions（已定,取代原 OQ-001..OQ-003）

| # | 決策 | 依據 |
| --- | --- | --- |
| D-1 | **接受正確性 gate 由 orchestrator 擔任**(implement inline 時無獨立視角的取捨成立):深度+安全由獨立 auditor 保留、TESTER 獨立、VERIFIER 獨立;正確性 gate 定位為「早期 catch」非獨立 review;受保護路徑 task 另有 architect 簽核層 | 原 OQ-001,使用者接受 2026-06-10;architect C3 補強 |
| D-2 | **命名 = `golem-auditor`**;dispatch 相位鍵改 **`audit`**(role=`AUDITOR`),**乾淨切換、不留 `review` alias**——repo 內呼叫者(gal-pipeline SKILL、gal.ps1、New-TaskSpec)全在同批改,無外部呼叫者,殘留 alias 違反 R-007 清零 | 原 OQ-002,使用者確認命名;相位鍵由 architect 裁定 2026-06-10 |
| D-3 | **施作順序:`feat-small-context-task-authoring` 先落地,本計畫 rebase 後接續,xmachine T-004 最後 port 合併後行為**。理由:(1) 擷取器修復(BUG-01/02)是任何 dispatched spec(含 auditor 相位)可靠性的前置;(2) 本計畫對 `New-TaskSpec.ps1` 只改 agentMap,疊在升級後擷取器上衝突最小;(3) xmachine 的 Rust task-spec 應 port 兩計畫收斂後的最終行為,避免 port 兩次 | 原 OQ-003,architect 裁定 2026-06-10;路線見 `.dev/state.md ## Cross-Plan Route` |

## Requirements

- [ ] **R-001 — golem-auditor 建立**:`plugins/gal-core/agents/golem-security.agent.md` 改名+擴範圍為 `golem-auditor.agent.md`,吸收 golem-reviewer 的深度效能清單(14–16);安全清單(17–23)保留;正確性/架構/品質(1–13)移出;`golem-reviewer.agent.md` 刪除;`agents.md` 索引更新。
- [ ] **R-002 — orchestrator 正確性 gate 正式化**:`plugins/gal-core/commands/gal-pipeline/SKILL.template.md` 把「收回 implement 後的確認」寫成 implement→test 之間的 gate(清單 1–13+明顯效能);不通過退 fix-mode,通過才進 test;不 dispatch。
- [ ] **R-003 — auditor 升格常態**:每 task 固定 dispatch;保留 security 嚴重度 STOP 規則(high/critical 開放即停)。
- [ ] **R-004 — 相位與契約重寫**:gal-pipeline SKILL 移除 dispatched REVIEWER 相位、插入正確性 gate、conditional security 改常態 auditor 相位;Model Assignment 表更新。
- [ ] **R-005 — 跨模型政策對齊**:`plugins/gal-core/workflows/coding.md` 移除/重定義 REVIEWER、新增 AUDITOR;明示「正確性 gate=orchestrator、稽核=獨立 auditor dispatch」語意。
- [ ] **R-006 — routing/相位鍵遷移**:`~/.gal/config/executor-routing.json` + repo `executor-routing.example.json` 的 `REVIEWER`→`AUDITOR`;`crates/dispatch/src/stage.rs` `Phase::Review("review"/REVIEWER)`→`Phase::Audit("audit"/AUDITOR)`(乾淨切換,D-2);`scripts/common/New-TaskSpec.ps1` agentMap `review→golem-reviewer.agent.md` 改 `audit→golem-auditor.agent.md`。
- [ ] **R-007 — blast radius 清零**:全 repo `golem-reviewer`/`golem-security`/`REVIEWER` 引用(現勘 **29 檔**,含生成 adapter)全數更新或由 `gal sync` 重生;grep 驗無殘留。

## Scope

**In scope**:`plugins/gal-core/agents/` golem 合併;`plugins/gal-core/commands/gal-pipeline/` 相位重構;`plugins/gal-core/workflows/coding.md`;executor-routing 鍵遷移;`crates/dispatch/src/stage.rs` 相位鍵;`New-TaskSpec.ps1` agentMap;全 repo 引用遷移;adapter 由 `gal sync` 重生(不手改)。

**Out of scope**:小 context task 撰寫優化(→ `feat-small-context-task-authoring.md`,先行,本計畫 rebase);orchestrator 正確性判斷的 LLM 內部機制;golem-architect(規劃期審查不動);xmachine 的 Rust pipeline/task-spec port(→ xmachine 計畫,port 本計畫收斂後行為)。

> **受保護路徑**:`commands/gal-pipeline/`、`workflows/coding.md`、`agents/`(契約面)、`crates/dispatch`。本次 deep-planning 已審;實作期逐項簽核。

## Approach

### 面向一:golem 合併(R-001)
- **Files**: `[RENAME+EXPAND] plugins/gal-core/agents/golem-security.agent.md → golem-auditor.agent.md`、`[DELETE] plugins/gal-core/agents/golem-reviewer.agent.md`、`[MODIFY] plugins/gal-core/agents/agents.md`
- **Verify**: auditor 職責=深度效能+安全;reviewer 檔不存在;索引更新。

### 面向二:pipeline 相位重構(R-002/003/004)
- **Files**: `[MODIFY] plugins/gal-core/commands/gal-pipeline/SKILL.template.md`(受保護)、`[MODIFY] scripts/common/New-TaskSpec.ps1`
- **What**: 2c implement 後插「2d orchestrator 正確性 gate」;原 2e dispatched REVIEWER 移除;2f conditional security → 常態 auditor 相位;agentMap `audit` 鍵對齊。
- **Verify**: 相位序 implement→gate→test→auditor→verify;gate 不通過退 fix 不進 test;auditor 每 task 跑。

### 面向三:政策 + routing + 清零(R-005/006/007)
- **Files**: `[MODIFY] plugins/gal-core/workflows/coding.md`(受保護)、`[MODIFY] executor-routing.example.json` + 機器本地 `executor-routing.json`、`[MODIFY] crates/dispatch/src/stage.rs`、29 檔引用、adapter 由 `gal sync` 重生
- **What**: coding.md 角色重寫;stage.rs `Phase::Audit`("audit"/AUDITOR)乾淨切換(parse 不再接受 "review",錯誤訊息列新相位集);grep 掃引用全遷移。
- **Verify**: repo 無 `golem-reviewer`/`golem-security` 有效引用;`gal-dispatch --phase audit` 解析正確、`--phase review` 報錯列出有效相位;adapter 無死連結。

## Files to Create or Modify

- `[RENAME+EXPAND] plugins/gal-core/agents/golem-security.agent.md → golem-auditor.agent.md`
- `[DELETE] plugins/gal-core/agents/golem-reviewer.agent.md`
- `[MODIFY] plugins/gal-core/agents/agents.md`
- `[MODIFY] plugins/gal-core/commands/gal-pipeline/SKILL.template.md`(受保護)
- `[MODIFY] scripts/common/New-TaskSpec.ps1` — agentMap(疊在 small-context 升級後,D-3)
- `[MODIFY] plugins/gal-core/workflows/coding.md`(受保護)
- `[MODIFY] executor-routing.example.json` + 指引使用者更新機器本地 `~/.gal/config/executor-routing.json`
- `[MODIFY] crates/dispatch/src/stage.rs` — Phase::Review→Audit(含測試更新)
- `[MODIFY]` 其餘引用(commands.md、gal-status/whats-next、templates/plan-prompt.md、docs/manual.md、README 等,現勘 29 檔)
- `[REGEN] AGENTS.md、CLAUDE.md、GEMINI.md、.github/copilot-instructions.md、i18n README` — 由 `gal sync` 重生

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

- [ ] `golem-auditor.agent.md` 存在且職責=深度效能+安全;`golem-reviewer.agent.md` 已刪
- [ ] pipeline 相位序:implement → 正確性 gate → test → auditor → verify
- [ ] 正確性 gate 不通過退 fix-mode,不進 test
- [ ] auditor 每 task 跑(非 conditional);high/critical 開放即 STOP
- [ ] `gal-dispatch --phase audit` → role AUDITOR;`--phase review` 報錯且錯誤訊息列出有效相位
- [ ] routing `AUDITOR` 鍵解析正確;repo 無 `REVIEWER` 鍵殘留(機器本地檔由使用者按指引更新)
- [ ] grep 無 `golem-reviewer`/`golem-security` 有效引用;adapter 無死連結

## Success Criteria

- [ ] 合併完成,職責邊界與 gate 切分文件化。
- [ ] test 前先由 orchestrator 確認正確性,省「對明顯錯誤的 code 跑測試」的浪費。
- [ ] 稽核獨立性保留(auditor 永遠 dispatch)。
- [ ] 29 檔引用清零,無回歸。

## Risks

- **大範圍引用遷移(29 檔)**:含受保護路徑與生成 adapter。緩解:階段化遷移 + grep gate;adapter 一律 `gal sync` 重生。
- **正確性審查獨立性流失**:已由 D-1 明確接受;補強=auditor/TESTER/VERIFIER 三層獨立保留 + 受保護路徑另有 architect 簽核。
- **同檔協調**:`New-TaskSpec.ps1` 與 small-context 同檔。緩解:D-3 排序,本計畫 rebase。
- **auditor 常態化成本**:每 task 一次 dispatch。緩解:spec 導向+讀指名檔;複雜度門檻 YAGNI 暫不設,證明過重再加。
- **機器本地 routing 檔不可由 repo 強制**:`~/.gal/config/executor-routing.json` 屬使用者。緩解:example 檔更新 + 升級指引;dispatch 對舊 `REVIEWER` 鍵在 `audit` 相位查無路由時的 fallback 訊息明確指出鍵改名。

## Review Results

### Architecture Review

#### Verdict: APPROVE-with-conditions *(2026-06-10 全面 deep-planning)*

方向正確:「對不對」是 orchestrator 已有完整 context 的廉價判斷,dispatch 出去反而丟 context;「深層+安全」需要獨立視角與專注清單,常態化後消除 conditional 觸發的判斷漏洞(R-06 review 實證:security-sensitive 變更的觸發判斷本身會漏)。合併消除 reviewer/security 職責重疊(邊界條件、效能兩邊都聲稱)。

**Conditions(實作必守):**

- **C1 安全語意不降級**:合併過程 golem-security 的 STOP 規則(high/critical 開放即停)逐字保留進 auditor;遷移 commit 內 diff 須可證無語意刪減。
- **C2 乾淨切換的原子性**:stage.rs 相位鍵、gal-pipeline SKILL、agentMap、routing example 四者**同一 commit** 落地;任何中間 commit 不得存在「skill 發 audit 但 bin 只認 review」的裂縫。
- **C3 獨立性補強文件化**:coding.md 重寫時明示三層獨立保留(auditor dispatch、TESTER≠CODER、VERIFIER≠CODER)+ 受保護路徑 architect 簽核;正確性 gate 定位「早期 catch」白紙黑字,防止日後誤解為獨立 review 替代品。
- **C4 機器本地 routing 遷移指引**:dispatch 在 `audit` 相位查無路由時,訊息須明說「`REVIEWER` 鍵已改名 `AUDITOR`,請更新 ~/.gal/config/executor-routing.json」——不可只報 no-routing。
- **C5 排序依賴**:本計畫在 small-context 落地後才動 `New-TaskSpec.ps1`(D-3);若 small-context 延宕,agentMap 改動可先行但須在其 rebase 清單明列。

**Trade-off Summary**

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| 正確性 gate 入 orchestrator | 全 context 判斷、省一次 dispatch、早期 catch | implement inline 時無獨立視角 | OK(D-1 已接受+C3 補強) |
| reviewer+security 合併 | 消除職責重疊、單一稽核入口 | 單 agent 清單變長 | OK(清單 14–23 聚焦) |
| auditor 常態化 | 消除 conditional 觸發漏判 | 每 task 一次 dispatch 成本 | OK(YAGNI 門檻後補) |
| 相位鍵乾淨切換無 alias | 清零無殘留、語意單一 | 機器本地 routing 需手動跟進 | OK(C4 指引) |

**Bug Surface**:中間態裂縫(C2 原子性);機器本地 routing 漏改(C4);安全清單語意流失(C1)。
**Over-engineering check**:無 alias 層、無複雜度門檻、無新 store——最小面成立。
**Performance**:每 task 多一次 auditor dispatch,換掉原 conditional REVIEWER dispatch,淨增量≈0(原 REVIEWER 每 task 也 dispatch);實際淨變化=conditional security 升常態的增量,可控。

### Business / Design Review

Not requested。

### Engineering Review

#### Verdict: CLEAR *(2026-06-10)*

4 個 T-NNN 對映 R-001..R-007。task 粒度刻意對齊 architect C2 原子性:**相位鍵切換(stage.rs + gal-pipeline SKILL 相位流 + agentMap + routing example)合為單一 T-02,須一個 commit 落地**——拆開會產生「skill 發 `audit` 但 bin 只認 `review`」的中間裂縫。T-01(agent 合併)先行(SKILL 需引用 golem-auditor);T-03(coding.md 政策)、T-04(29 檔清零 + sync 重生)接續。C1(安全 STOP 逐字保留)、C3(三層獨立性文件化)、C4(routing 改鍵指引)映入對應 task 驗收。**跨計畫排序(C5/D-3)**:T-02 對 `New-TaskSpec.ps1` agentMap 的改動疊在 small-context T-02 擷取器升級之上——本計畫整體在 Route ① 落地後啟動。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-01 | manual | `golem-auditor.agent.md` 存在,職責=深度效能(清單 14–16)+ 安全(17–23);`golem-reviewer.agent.md` 已刪;`agents.md` 索引更新且無指向已刪檔的連結 | T-01 |
| TP-02 | manual (diff) | 合併後 auditor 逐字保留 golem-security 的嚴重度 STOP 規則(high/critical 開放即停);以 T-01 commit diff 證無語意刪減(C1) | T-01 |
| TP-03 | unit (Rust) | `crates/dispatch/src/stage.rs`:`Phase::Audit` → role `AUDITOR`、`parse("audit")` Ok、`parse("review")` Err 且錯誤訊息列出有效相位集(含 audit、不含 review);既有 phase 測試更新 | T-02 |
| TP-04 | manual | `gal-pipeline/SKILL.template.md` 相位序=implement → orchestrator 正確性 gate → test → auditor(常態)→ verify;無 dispatched REVIEWER 相位;Model Assignment 表更新;agentMap `audit→golem-auditor.agent.md`;`executor-routing.example.json` 鍵=`AUDITOR` | T-02 |
| TP-05 | integration | `gal-dispatch --phase audit` 解析為 AUDITOR 角色並路由;`--phase review` 報錯;`audit` 相位查無路由時訊息明說「`REVIEWER` 鍵已改名 `AUDITOR`,請更新 ~/.gal/config/executor-routing.json」(C4) | T-02 |
| TP-06 | manual | `workflows/coding.md` 移除/重定義 REVIEWER、新增 AUDITOR;明示三層獨立(auditor dispatch、TESTER≠CODER、VERIFIER≠CODER)+ 受保護路徑 architect 簽核;正確性 gate 定位「早期 catch」非獨立 review(C3) | T-03 |
| TP-07 | manual (grep gate) | 全 repo 無 `golem-reviewer`/`golem-security`/`REVIEWER`(routing 鍵)有效引用(現勘 29 檔);生成 adapter 由 `gal sync` 重生無死連結;機器本地 `~/.gal/config/executor-routing.json` 由使用者按指引更新(不在 repo gate 內) | T-04 |
| TP-08 | integration (Rust) | `cargo test --workspace` 綠(stage.rs 相位改名回歸);`cargo clippy --workspace --all-targets` 0 warning | T-02 |

## Tasks

> 每個 task 自足(file:line 指標、具體改動、就地驗收、慣例指標)。受保護路徑(agents/gal-pipeline/coding.md/dispatch)實作期逐項簽核。整體在 small-context 計畫落地後啟動(D-3/C5)。

- [ ] **T-01 (R-001) — golem 合併為 golem-auditor**
  - 檔案:`[RENAME+EXPAND] plugins/gal-core/agents/golem-security.agent.md → golem-auditor.agent.md`、`[DELETE] plugins/gal-core/agents/golem-reviewer.agent.md`、`[MODIFY] plugins/gal-core/agents/agents.md`。現況:golem-security 有 Confidence Gate + Step 5 Write-Back「若 high/critical 開放則 `FINDINGS-OPEN`」(line 117);golem-reviewer 的深度效能在 Bug Pattern Scan(N+1 等,line 50-53)。
  - 改動:以 golem-security 為基底改名 golem-auditor,吸收 golem-reviewer 的深度效能審查職責(清單 14–16:N+1、無邊界載入、熱路徑同步 I/O);安全清單(17–23)保留;正確性/架構/品質(1–13)**不**併入(改由 orchestrator gate,屬 T-02)。**C1:golem-security 的嚴重度 STOP/FINDINGS-OPEN 規則逐字保留**。刪 golem-reviewer;`agents.md` 索引移除 reviewer/security、加入 auditor。
  - 驗收:TP-01、TP-02。
  - 慣例:markdown-formatting;C1 以 diff 證無語意刪減。

- [ ] **T-02 (R-002+R-003+R-004+R-006) — 相位鍵 + pipeline 原子切換【C2:單一 commit】**
  - 檔案(**全部同一 commit**):`crates/dispatch/src/stage.rs`(`Phase::Review`/`"review"`/`REVIEWER` 在 line 28-50)、`plugins/gal-core/commands/gal-pipeline/SKILL.template.md`(2c implement line 368、2d test 391、2e review 418、2f conditional security 445、Model Assignment 表 56-63)、`scripts/common/New-TaskSpec.ps1`(agentMap line 149 `review = ...golem-reviewer.agent.md`)、`executor-routing.example.json`(`REVIEWER` 鍵)。
  - 改動:(1) stage.rs `Phase::Review`→`Phase::Audit`,`as_str()`→`"audit"`,`role()`→`"AUDITOR"`,`parse()` 接受 `"audit"`、拒 `"review"` 且錯誤訊息列有效相位集;更新相關測試。(2) gal-pipeline SKILL:2c 後插「2d orchestrator 正確性 gate」(清單 1–13+明顯效能,不通過退 fix-mode 不進 test,不 dispatch),原 test 順延,移除 2e dispatched REVIEWER,2f conditional security 改「auditor 常態相位」(每 task dispatch,保留 high/critical STOP),Model Assignment 表反映新相位序。(3) New-TaskSpec agentMap `review`→`audit = ...golem-auditor.agent.md`(**疊在 small-context T-02 升級後的擷取器上**)。(4) routing example `REVIEWER`→`AUDITOR`。
  - 驗收:TP-03、TP-04、TP-05、TP-08。**C2:四檔同 commit,無中間裂縫**。
  - 慣例:rust(stage.rs)+ markdown(SKILL);依賴 T-01(SKILL/agentMap 引用 golem-auditor)。

- [ ] **T-03 (R-005) — coding.md 跨模型政策對齊**
  - 檔案:`plugins/gal-core/workflows/coding.md`(受保護)。
  - 改動:移除或重定義 REVIEWER 角色、新增 AUDITOR;明示語意「正確性 gate=orchestrator(全 context、早期 catch,非獨立 review 替代品)、深度+安全稽核=獨立 auditor dispatch」;**C3:白紙黑字三層獨立保留**(auditor dispatch、TESTER≠CODER、VERIFIER≠CODER)+ 受保護路徑另有 architect 簽核。
  - 驗收:TP-06。
  - 慣例:markdown-formatting;與 T-02 的相位序一致(不得矛盾)。

- [ ] **T-04 (R-007) — blast radius 清零 + adapter 重生**
  - 檔案:現勘 **29 檔** `golem-reviewer`/`golem-security`/`REVIEWER` 引用(`commands/commands.md`、`commands/gal-status`、`commands/gal-whats-next`、`templates/plan-prompt.md`、`docs/manual.md`、`README.md` 等);`[REGEN] AGENTS.md、CLAUDE.md、GEMINI.md、.github/copilot-instructions.md、docs/i18n README` 由 `gal sync` 重生(不手改)。
  - 改動:grep 掃全 repo,把人手維護的引用遷移為 auditor/AUDITOR/audit 相位;生成 adapter 一律 `gal sync` 重生;最後 grep gate 驗無 `golem-reviewer`/`golem-security` 有效引用、adapter 無死連結。機器本地 `~/.gal/config/executor-routing.json` 屬使用者,不在 repo gate(C4 指引由 T-02 的 dispatch 訊息承擔)。
  - 驗收:TP-07。
  - 慣例:token-budget(generated-artifact exclusion——adapter 不手改);階段化遷移 + grep gate。
