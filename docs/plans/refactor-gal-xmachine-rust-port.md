# Plan: GAL xmachine 子系統 Rust 原生化（refactor-gal-xmachine-rust-port）

> 姊妹計畫 `refactor-gal-core-rust-port.md` 負責核心安裝/設定層;本計畫負責 **執行編排層**:`pipeline`(本地多任務:切小+多 provider)與 `xmachine`(pipeline 的遠端 SSH+zellij 升級)。兩計畫共同取代 `feat-gal-rust-native-install.md`(已刪)。**本計畫依賴核心計畫先抽出的 `base` 與去前綴的 `dispatch`。**

## Approval

- Human approval: [approved at 2026-06-09]
- Architect review: **APPROVE(方向;二審 2026-06-09 折入)** — 二審補完整架構評估:`Transport` trait 反轉、session 紀錄重用 executor-log、xmachine 實作 HealthCheck、xmachine 獨立 crate 之 dep-隔離理由。**OQ-X1/X2/X3 全解,已內化為 Decisions。** 實作期 `pipeline`/`xmachine` crate + 受保護契約面須 architect 簽核;依賴核心計畫 R-00 先抽 `base`/`dispatch`。下一步 `/refining-plan`。
- Additional domain review: [not triggered]（無 customer-facing / business-rule）

## Goal

xmachine 子系統(約 6028 行 `.ps1`+`.sh`)的 SSH 遠端執行、本地/遠端 task dispatch、pipeline 編排、結果回收,全部由 Rust 原生實作並達行為 parity,對應 ps1/sh 成對刪除。終態:執行編排一律走 `gal` binary,`scripts/` **xmachine 家族零 ps1/sh**。

## Motivation（同核心計畫）

① 消除雙實作維護成本(xmachine 是最重的雙實作,~6028 行兩份);② 單一二進位加速 AI(pipeline/dispatch 是 AI 熱路徑,遠端執行體 = 遠端的 `gal` binary 最一致,免在遠端佈署腳本叢)。

## Rust 目標架構（dispatch → pipeline → xmachine，逐層升級）

> **關鍵關係(使用者澄清)**:現行 `pipeline` 已優化「**把 task 切小 + 分派給其他 provider**」(本地)。**`xmachine` = 此 pipeline 的純升級** —— 同一套編排,加 **SSH + zellij 遠端 transport** 跑多機,並**留 SSH/zellij session 紀錄供追述**。各層不重做下層。命名沿用核心計畫:workspace 無前綴,binary `gal`。

| 層 | crate | 職責 | 依賴 |
| --- | --- | --- | --- |
| foundation | `base` | (核心計畫 R-00 抽出) config/mode/paths/platform/provider-selection + `HealthCheck` trait | 無 GAL crate |
| 原語 | `dispatch` | (已存在,去前綴) 單 task→單 executor:spawn+write-back+session+5 adapter(TP-17 機制) | base |
| 編排 | `pipeline` | task 切小 + 多 provider 分派 + 多 stage 編排;**`Transport` trait**(local impl 在此);**task-spec(New-TaskSpec)歸此** | base, dispatch |
| 升級 | `xmachine` | pipeline 的遠端升級:實作 `Transport`(SSH+zellij)+ 遠端結果回收 + session 紀錄 | base, dispatch, pipeline |
| edge | `cli` | `gal pipeline`、`gal xmachine`、`gal dispatch`;doctor 聚合 | pipeline, xmachine, dispatch |

**依賴鐵律**:DAG = `base ← dispatch ← pipeline ← xmachine`,無環。**一套編排,兩 transport**(local / SSH+zellij);xmachine **不重做** pipeline/dispatch/routing/stage,只加遠端層。

**架構決定(本次 architect 審查折入)**:
- **`Transport` trait 反轉** —— trait 定義在 `pipeline`,local impl 在 pipeline、SSH+zellij impl 在 `xmachine`。`pipeline` 不依賴 `xmachine`(依賴方向正確);xmachine 是可插拔 transport。
- **xmachine 獨立 crate 之理由** —— SSH/zellij 帶重依賴,隔離在自己的 crate 讓 `pipeline` 對純本地用途保持精簡。
- **session 紀錄重用 executor-log** —— R-04c 的 SSH/zellij session 紀錄**延伸 dispatch 既有的 `.dev/executor-logs/` 機制**(加 host/transport/session-id 欄位),**不另建紀錄 store**(對齊 file-memory 契約)。
- **xmachine 實作 `base::HealthCheck`** —— preflight(SSH/zellij/遠端 gal)以 `HealthCheck` 暴露,`gal doctor` 用與核心計畫一致的聚合方式收集。

## 使用前提（USER 自理,GAL 只 preflight 不代設）

- **SSH** —— 使用者自設可連線 SSH。**GAL 無法協助處理 SSH(硬邊界)**,只 preflight + 缺失 fail-loud 附指引。
- **zellij** —— 遠端 session 多工依賴,使用者自裝;GAL preflight only。
- **遠端 `gal`** —— 遠端執行體須已有相容 `gal`(使用者自裝);GAL preflight 版本(不符則拒絕 + 指引更新),**不代 scp**。

## 全面盤點:每個 xmachine script 的 Rust 目標

> Rust 現況:`dispatch` 原語已完成;`pipeline`/`xmachine` 待建。

| Script | 行數 | 職責 | Rust 目標 |
| --- | --- | --- | --- |
| `Start-xMachine.{ps1,sh}` | 346/347 | session 啟動入口 | `gal xmachine start` |
| `Start-XmachinePipeline.{ps1,sh}` | 115/144 | pipeline 啟動入口 | `gal pipeline` / `gal xmachine pipeline` |
| `Invoke-XmachineTask.{ps1,sh}` | 583/514 | task 執行主體 | `xmachine`/`pipeline` task 後端 |
| `Invoke-XmachineLocalTask.{ps1,sh}` | 76/207 | 本地 task 執行 | `pipeline`(local transport,委派 `dispatch`) |
| `Invoke-XmachineRemoteTask.{ps1,sh}` | 162/112 | 遠端 SSH task 執行 | `xmachine`(SSH+zellij transport) |
| `Invoke-XmachinePipeline.{ps1,sh}` | 618/381 | pipeline 編排(切小+多 provider) | `pipeline`(本地)+ `xmachine`(遠端升級) |
| `Get-XmachineLocalResult.sh`(單) | 134 | 本地結果回收 | `pipeline` result |
| `Get-XmachineRemoteResult.ps1`(單) | 233 | 遠端結果回收 | `xmachine` result |
| `Test-Xmachine.{ps1,sh}` | 952/105 | 測試器 | Rust 測試/fixture |
| `Test-PipelineTokenBurn.ps1`(單) | — | pipeline token 用量測試 | Rust 測試 |
| `test-t022-ssh.sh`(單) | — | SSH 跨平台 smoke(T-010 已部分 reparent) | Rust 行為測試/fixture |
| `common/New-TaskSpec.ps1`(單) | 249 | task spec 組裝 | **`pipeline` crate** task-spec(歸 pipeline,不歸 xmachine) |
| `common/Common.{ps1,sh}` 的 xmachine 部分 | — | `resolve_xmachine_config_path` 等 | `base`/`xmachine`;共用檔整檔刪掛兩計畫尾端 |
| `gal.{ps1,sh}` 的 xmachine/dispatch 入口 | — | `xmachine)`/`dispatch)` 分支 | `cli`;入口整檔刪掛兩計畫尾端 |

## 契約面（受保護,須同步改）

xmachine 被 GAL 契約直接引用,Rust 化後引用改指 `gal xmachine`/`gal pipeline`:
- `plugins/gal-core/commands/gal-pipeline/SKILL.template.md`
- `plugins/gal-core/templates/task-xmachine-{local,remote}-smoke.md`
- `plugins/gal-core/agents/agents.md`

## Requirements

- [ ] **R-00 架構:組合不重做（受保護）** — `pipeline`/`xmachine` 為新 crate,依賴 `base`+`dispatch`;本地執行委派 `dispatch`,遠端為 `Transport` 的 SSH+zellij impl。**不得重做 dispatch/routing/stage。** `Transport` trait 置 `pipeline`,xmachine 實作之。須 architect。
- [ ] **R-01 pipeline 後端** — task 切小 + 多 provider 分派 + 多 stage 編排,組合 `dispatch`,local transport;Rust 化為 `gal pipeline`。
- [ ] **R-02 task-spec 歸 `pipeline`** — `New-TaskSpec.ps1` 的 task-spec 組裝進 `pipeline`(非 xmachine);small-context 的擷取器升級(多行 + 每 task 檔案)當此 Rust 化的規格;golem-auditor 的 routing 改動歸 `dispatch::routing`,不落 task-spec。
- [ ] **R-03 xmachine 遠端後端** — SSH+zellij `Transport` impl + 遠端結果回收;`gal xmachine {start,pipeline,task}`。
- [ ] **R-04 契約面同步（受保護）** — gal-pipeline SKILL、task-xmachine 模板、agents.md 引用改指 `gal xmachine`/`gal pipeline`。須 architect。
- [ ] **R-05 前提 preflight（只檢查不代設）** — 遠端調用前 preflight SSH 可連 / zellij 已裝 / 遠端 `gal` 相容,缺失 fail-loud 附修復指引。**GAL 不代設 SSH、不代裝 zellij、不代 scp gal。** 以 `base::HealthCheck` 暴露供 `gal doctor` 聚合。
- [ ] **R-06 session 紀錄（追述）** — 遠端執行留 session 紀錄(host、transport、SSH/zellij session id、時間、結果路徑),**延伸 dispatch 的 `.dev/executor-logs/`,不另建 store**。
- [ ] **R-07 跨機 SSH parity** — 真實跨機驗遠端執行/結果回收與舊腳本等價:win→mac-mini(Unix,涵蓋 win→linux) + win→win(Win11 筆電)。**遠端 `gal` = 純 end-user 安裝核心計畫 R-13 release 管線的預編譯 artifact(絕不在遠端 build/跨編譯;對齊核心 CORR-01),前置 = R-13 artifact 存在。**
- [ ] **R-08 oracle reparent 前置** — `Test-Xmachine`/`test-t022-ssh.sh`/`Test-PipelineTokenBurn` 刪前改 fixture/行為測試。
- [ ] **R-09 共用檔協調刪除** — `common.{ps1,sh}` xmachine 函式搬 Rust 後,與核心計畫共同在兩計畫皆 done 時整檔刪 `common.*`、`gal.{ps1,sh}`。
- [ ] **R-10 終態零 ps1/sh（xmachine 層）** — 結束時 `scripts/` 不留任何 xmachine 家族 ps1/sh。

## Approach

| Phase | 目標 | 前置 |
| --- | --- | --- |
| **P0 盤點凍結** | 凍結 xmachine/pipeline parity fixture;盤點 SSH 行為基準 | — |
| **P1 `pipeline` + task-spec** | port 本地編排(切小+多 provider)+ task-spec + `Transport` trait,local impl;組合 `dispatch` | P0、核心 R-00(base/dispatch) |
| **P2 `xmachine` 遠端 transport** | SSH+zellij `Transport` impl + 遠端結果回收 + preflight + session 紀錄;跨機 parity | P1 |
| **P3 入口 + 契約面（R-04,受保護）** | `gal pipeline`/`gal xmachine`/`gal dispatch` 入口;改契約引用;architect | P1–P2、architect |
| **P4 reparent + 刪除（R-08/R-10）** | 測試改 fixture;刪 xmachine 全家族 + New-TaskSpec | P1–P3 |
| **P5 共同收尾（R-09）** | 與核心計畫共同刪 `common.*`/`gal.{ps1,sh}` | 兩計畫皆 done |

每期 Verify:`cargo test` 綠;該期 superseded script 刪除;測試碼不再 spawn 該期 live script;跨機行為對齊凍結 fixture。

## Files to Create or Modify

- `[CREATE]`(受保護,架構) `crates/pipeline/`:切小+多 provider+多 stage 編排 + `Transport` trait(local impl) + task-spec;組合 `dispatch`。依賴 base, dispatch。
- `[CREATE]`(受保護,架構) `crates/xmachine/`:`Transport` 的 SSH+zellij impl + 遠端結果回收 + preflight(HealthCheck) + session 紀錄。依賴 base, dispatch, pipeline。
- `[MODIFY]` `crates/dispatch/`(已由核心計畫去前綴):若需,擴 adapter 介面供遠端複用,避免重做 spawn/write-back。
- `[MODIFY]` `crates/cli/`:`gal pipeline`/`gal xmachine`/`gal dispatch` 接線 + xmachine HealthCheck 入聚合。
- `[MODIFY]`(受保護,契約面) `plugins/gal-core/commands/gal-pipeline/SKILL.template.md`、`plugins/gal-core/templates/task-xmachine-{local,remote}-smoke.md`、`plugins/gal-core/agents/agents.md`。
- `[MODIFY]` `tests/fixtures/`、`crates/*/tests/*`:xmachine oracle reparent。
- `[DELETE on parity]` xmachine 家族:`Start-xMachine.{ps1,sh}`、`Start-XmachinePipeline.{ps1,sh}`、`Invoke-Xmachine{Task,LocalTask,RemoteTask,Pipeline}.{ps1,sh}`、`Get-Xmachine{Local,Remote}Result.*`、`Test-Xmachine.{ps1,sh}`、`Test-PipelineTokenBurn.ps1`、`test-t022-ssh.sh`、`common/New-TaskSpec.ps1`。
- `[DELETE on parity,跨計畫尾端]` `common/Common.{ps1,sh}`、`gal.{ps1,sh}` — 與核心計畫共同收尾。

## Success Criteria

- [ ] `scripts/` 不留任何 xmachine 家族 ps1/sh。
- [ ] `gal pipeline`/`gal xmachine`/`gal dispatch` 存在且與舊腳本 parity(對照 fixture)。
- [ ] 跨機 SSH:win→mac-mini(Unix)+ win→win(Win11)遠端執行 + 結果回收與舊腳本等價。
- [ ] 前提未就緒時 preflight fail-loud 附指引;GAL 不代設 SSH/zellij/遠端 gal。
- [ ] session 紀錄留於 `.dev/executor-logs/`(含 host/transport/session-id),可追述。
- [ ] 契約面引用改指 `gal xmachine`/`gal pipeline`。
- [ ] `cargo test` 綠且測試碼不再 spawn 任何 xmachine live `scripts/*.{ps1,sh}`。

## Risks

- **SSH 遠端 parity（高）**:跨機行為難隔離測,環境差異大。Mitigation:P0 凍結 fixture;P2 真機跨機(win→mac + win→win)。
- **使用者前提未就緒（高）**:SSH/zellij/遠端 gal 是使用者責任,GAL 無法代設(SSH 硬邊界)。Mitigation:R-05 preflight 只檢查 + fail-loud,文件明列前提。
- **跨計畫共用檔（高）**:`common.*`/`gal.{ps1,sh}` 與核心計畫共用。Mitigation:P5 共同收尾 gate。
- **cross-plan 依賴閘（中）**:pipeline/xmachine 依賴核心計畫 R-00 的 base/dispatch。Mitigation:P1 前置核心 R-00;排序協調。
- **pipeline 語義複雜（中）**:token-burn/多 stage 狀態多。Mitigation:P1 以 fixture 鎖既有語義再替換。

## Decisions（已定,不再是 OQ）

- **xmachine = pipeline 純遠端升級** — dispatch(原語,已完成,共用)→ pipeline(本地切小+多 provider)→ xmachine(遠端 SSH+zellij)。三層不重做下層。
- **task-spec 歸 `pipeline`,xmachine 不碰** — 故 xmachine 退出 New-TaskSpec 三方爭用;small-context 擷取器升級 → pipeline task-spec Rust 規格;golem-auditor routing → `dispatch::routing`。協調在 pipeline 層,不阻斷本計畫。
- **跨機測試矩陣** — win→mac-mini(涵蓋 win→linux,皆 Unix SSH 目標)+ win→win(使用者 Win11 筆電,SSH 待設定)。Windows-as-remote 不劃出範圍。
- **SSH/zellij/遠端 gal = 使用者前提** — GAL 只 preflight 不代設。

## Review Results

### Architecture Review

**Verdict: APPROVE(方向)。** 二審(2026-06-09)補上首審缺的完整架構評估。核心決定正確:**`xmachine` 組合 `dispatch`+`pipeline` 而非重做**,三層 `dispatch→pipeline→xmachine` altitude 分明(xmachine=pipeline 純遠端升級)。使用者澄清(pipeline 已切小+多 provider、SSH/zellij 為前提、留 session 紀錄)全折入。

#### Trade-off Summary

| 決策 | 效益 | 成本 | 裁決 |
| --- | --- | --- | --- |
| dispatch→pipeline→xmachine 三層 | 不重造下層;依賴方向正確(DAG) | 三層鏈 | OK |
| `Transport` trait 置 pipeline | xmachine 可插拔,pipeline 不依賴 xmachine | 一個 trait | OK(反轉正確) |
| xmachine 獨立 crate | SSH/zellij 重依賴隔離,pipeline 保精簡 | crate 數 | OK |
| 遠端 + SSH/zellij/遠端gal 為使用者前提 | 遠端免佈署腳本叢;GAL 不碰 SSH | 前提未就緒則落空 | OK(R-05 preflight) |
| session 紀錄重用 executor-log | 不另建 store,對齊 file-memory 契約 | 擴欄位 | OK |
| 全 Rust(~6028 行) | 消除最重雙實作、AI 熱路徑 | 範圍大 + SSH parity 難測 | OK |

#### 架構發現（本次新增,已折入）

- **[ARCH-X1] `Transport` trait 反轉** → trait 置 `pipeline`,local impl 在 pipeline、SSH+zellij 在 xmachine;依賴方向 pipeline ← xmachine,無環。
- **[ARCH-X2] session 紀錄重用 executor-log** → R-06 延伸 `.dev/executor-logs/`,不另建 store(對齊 file-memory 契約,避免第二記憶層)。
- **[ARCH-X3] xmachine 實作 `base::HealthCheck`** → preflight 以統一 trait 暴露,`gal doctor` 一致聚合。
- **[ARCH-X4] cross-plan 依賴閘** → pipeline/xmachine 硬依賴核心 R-00 的 base/dispatch,排序明列為 Risk。

#### Bug Surface

- **[BUG-X1](高)遠端前提未就緒則落空** → R-05 preflight,GAL 只檢查不代設。
- **[BUG-X2](中)遠端/本地 gal 版本不一致** → preflight 拒絕 + 指引更新(不自動更新,與「不代設」一致)。

#### Performance

- **[PERF-X1]** SSH 每 task 重建連線會慢 → `/refining-plan` 評估連線複用(非阻斷,先 fixture 鎖語義)。

#### What's Good

- 組合 dispatch+pipeline 不重做;三層 altitude 清楚。
- `Transport` trait 反轉 + HealthCheck 一致 + session 紀錄重用 executor-log = 與核心計畫架構決定一致。
- 跨機矩陣由兩台真機(mac + Win11)務實覆蓋。
- TP-17 沿用 honest-test-pass-bar。

#### 三審：task/test 粒度（2026-06-09,使用者要求最終檢查）

- **[GRAN-X1] 初版 task 過粗** → 已重切:T-002(原綁 pipeline+task-spec+Transport+local impl)拆為 crate-骨架/編排-port/task-spec-port;T-003(原綁 SSH+zellij+結果回收+preflight+session 紀錄 五單元)拆為 T-005..T-009 五步。15 task,每個 commit-size 可獨立驗。
- **[GRAN-X2] test 補洞** → zellij 斷線/重連(TP-06)、結果回收(TP-07)、preflight 不代設(TP-08)、session 紀錄 schema(TP-09)、cli 接線(TP-11)各自獨立 TP;跨機硬 gate 含 win→win(Win11)。

<!-- ARCH_REVIEW: APPROVE -->

### Business Review
Not triggered（無 business-rule）。

### Design Review
Not triggered（無 customer-facing UI）。

### Engineering Review

**Verdict: CLEAR.**（三審 2026-06-09 重切粒度後)15 個 T-NNN 對映 R-00..R-10 / P0..P5,**每個 = 一個獨立可驗、commit-size 單元**(pipeline 切為 crate-骨架/編排-port/task-spec-port;xmachine 切為 SSH-transport/zellij/結果回收/preflight/session-紀錄 五步);13 條 TP 覆蓋,含跨機硬 gate(win→mac + win→win)與接管 TP-17。architect 三審 APPROVE。可進 `/plan-to-prompt`。

**實作期約束(prompt 與執行須遵守):**

1. **cross-plan 依賴閘** — T-002(`pipeline` 骨架)硬依賴**核心計畫 R-00(T-003..T-009 抽出 `base`、去前綴 `dispatch`)**;未完成前不啟動。
2. **受保護核心 + 契約面簽核** — pipeline/xmachine crate 任務 + 契約面 T-012 實作前須 architect 簽核;CODER≠AUDITOR。
3. **不重做下層** — 組合 `dispatch`,不重造 spawn/write-back/session/routing/stage;`Transport` trait 置 `pipeline`,xmachine 實作(方向 pipeline ← xmachine)。
4. **前提只檢查不代設** — T-008 preflight 對 SSH/zellij/遠端 gal 只檢查 + fail-loud,**絕不代設**;T-009 session 紀錄延伸 `.dev/executor-logs/` 不另建 store。
5. **刪除 gate** — T-010(oracle reparent)先於 T-014 刪除;`common.*`/`gal.{ps1,sh}`(T-015)掛兩計畫共同尾端 gate。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-01 | integration | fixture freeze:xmachine/pipeline 現況可觀察輸出/副作用可重現;SSH 行為基準 | T-001 |
| TP-02 | unit | `pipeline` crate 骨架 + `Transport` trait + local impl 組合 `dispatch`(不重造);`cargo test` 綠 | T-002 |
| TP-03 | parity | `pipeline` 切小 + 多 provider 分派 + 多 stage 編排輸出 == fixture | T-003 |
| TP-04 | parity | task-spec(`New-TaskSpec`)組裝 == fixture;含 small-context 多行擷取 + 每 task 檔案收斂 | T-004 |
| TP-05 | integration | `xmachine` SSH `Transport` impl 遠端執行 == fixture | T-005 |
| TP-06 | integration | zellij session 多工正確;斷線/重連行為 == fixture | T-006 |
| TP-07 | integration | 遠端結果回收 == fixture | T-007 |
| TP-08 | unit | preflight:SSH 不可連 / zellij 缺 / 遠端 gal 版本不符 各 fail-loud 附指引;GAL 無 scp/install 副作用(不代設) | T-008 |
| TP-09 | unit | session 紀錄落 `.dev/executor-logs/`(host/transport/session-id/時間/結果路徑),不另建 store | T-009 |
| TP-10 | unit | T-010 後 `cargo test` 不再 spawn `Test-Xmachine`/`test-t022-ssh.sh`/`Test-PipelineTokenBurn`;改讀 fixture | T-010 |
| TP-11 | integration | `cli`:`gal pipeline`/`gal xmachine`/`gal dispatch` 可呼;xmachine `HealthCheck` 入 `gal doctor` 聚合 | T-011 |
| TP-12 | integration | 契約面(SKILL/模板/agents.md)引用改指 `gal xmachine`/`gal pipeline`;`/gal pipeline` 走 Rust | T-012 |
| TP-13 | integration（硬 gate） | 跨機 SSH parity:win→mac-mini(Unix,涵蓋 win→linux)+ win→win(Win11 筆電)遠端執行+回收 == 舊腳本 | T-013 |
| TP-14 | manual | `scripts/` xmachine 家族零 ps1/sh(grep);`cargo test` 綠且不 spawn xmachine live script | T-014, T-015 |
| **TP-17** | integration（要重做;加強,非 gate） | **mac-mini `/gal pipeline` write-back spike**:驗 3 executor(claude/opencode/copilot)真實 headless write-back —— 目標檔被寫回 **且** `.dev/executor-logs/` 終態 `completed` **且** 留可 resume native session id。codex(預算)/agy(無安裝路徑)標 ⬜ 未執行附原因,不標 PASS。 | T-013 |

**TP-17 場地**:借核心計畫 TP-02(mac-mini 安裝)同一趟 SSH session 順手做。**honest-test-pass-bar**:PASS 須同時 (a)目標檔被次要工具寫回 (b)executor-log 終態 `completed` (c)可 resume native session id;三者缺一不得 PASS。

## Tasks

> 一個 task = 一個獨立可驗、commit-size 單元;各 domain 在自己 port task 內就實作 `HealthCheck`。

**P0**
- [ ] **T-001（P0）** — 凍結 xmachine/pipeline parity fixtures;盤點 SSH 行為基準。

**R-01/R-02 pipeline（受保護,architect;前置=核心 R-00）**
- [ ] **T-002** — 建 `pipeline` crate 骨架:定義 `Transport` trait + local impl 組合 `dispatch`(不重造);`cargo test` 綠。
- [ ] **T-003** — port 切小 + 多 provider 分派 + 多 stage 編排 → `pipeline`;parity。
- [ ] **T-004** — port task-spec(`New-TaskSpec`,吸收 small-context 多行擷取 + 每 task 檔案收斂規格)→ `pipeline`;parity。

**R-03/R-05/R-06 xmachine（受保護,architect）**
- [ ] **T-005** — 建 `xmachine` crate + SSH `Transport` impl;遠端執行 parity。
- [ ] **T-006** — zellij session 多工整合;斷線/重連行為 parity。
- [ ] **T-007** — 遠端結果回收;parity。
- [ ] **T-008（R-05）** — preflight(`HealthCheck`:SSH/zellij/遠端 gal,只檢查不代設,fail-loud 附指引)。
- [ ] **T-009（R-06）** — session 紀錄延伸 `.dev/executor-logs/`(host/transport/session-id/時間/結果路徑),不另建 store。

**reparent / 入口 / 契約**
- [ ] **T-010（R-08,先於刪除）** — reparent `Test-Xmachine`/`test-t022-ssh.sh`/`Test-PipelineTokenBurn` 為 fixture/行為測試。
- [ ] **T-011（R-04/P3）** — `cli` 接線 `gal pipeline`/`gal xmachine`/`gal dispatch` + xmachine `HealthCheck` 入 doctor 聚合。
- [ ] **T-012（R-04,受保護,architect）** — 改契約面(gal-pipeline SKILL/task-xmachine 模板/agents.md)引用指 Rust binary。

**parity / 刪除 / 收尾**
- [ ] **T-013（R-07,硬 gate;前置=核心 R-13 artifact）** — 跨機 SSH parity:win→mac-mini(涵蓋 win→linux)+ win→win(Win11 筆電),遠端裝**核心 R-13 預編譯 artifact**(純 end-user,不在遠端 build);對齊 fixture;順手做 TP-17 write-back spike。
- [ ] **T-014（R-10/P4）** — 對齊 fixture 後刪 xmachine 家族 ps1/sh + `common/New-TaskSpec.ps1`。
- [ ] **T-015（R-09/P5,跨計畫尾端）** — 與核心計畫共同刪 `common/Common.{ps1,sh}` + `gal.{ps1,sh}`(兩計畫皆 done 才整檔刪)。
