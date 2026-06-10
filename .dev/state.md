# Repository GAL state

## Cross-Plan Route（2026-06-10 全面 deep-planning 定案,依此順序執行）

1. **① feat-small-context-task-authoring** — `/refining-plan` → `/plan-to-prompt` → 實作(R-001..R-008,含 dispatch crate 本地 executor preflight)。契約源頭,最先落地。
2. **② refactor-golem-auditor** — ① 落地後啟動(同檔 `New-TaskSpec.ps1`,D-3 排序):`/refining-plan` → 實作(golem 合併 + `audit` 相位乾淨切換 + 29 檔清零)。
3. **③ 核心 T-034(R-13 release 管線)** — 與 ①② 平行可做;是核心 T-035 與 xmachine T-013 的共同硬前置。需使用者先定 GitHub 公開決策。
4. **④ fix-install-followups-closeout** — 重勘版已 APPROVE:M1(R-01/R-02/R-V1/R-V2)隨時可做;R-03 cosign 審查掛 T-034 後、發佈前;M2 獨立。
5. **⑤ xmachine** — T-001(fixture 凍結)隨時;T-002+(pipeline crate)前置核心 R-00 已滿足;**T-004(task-spec port)等 ①② 收斂**;T-013 等 ③ artifact。auditor 的 stage.rs `audit` 相位改名應先於 xmachine T-003 編排 port,避免 port 舊相位名。
6. **⑥ 核心 T-035** — 等 ③;**核心 T-033** — 等 xmachine T-015(兩計畫共同尾端,最後一步)。

派工注意:本機 copilot CLI 未認證(dispatch 根因,使用者保留現狀);headless 派工恢復前,pipeline 實作相位以 DEGRADED_BUNDLED 或使用者手動跨平台執行。

## Active Plans

- .dev/plans/refactor-gal-core-rust-port.prompt.md
- .dev/plans/refactor-gal-xmachine-rust-port.prompt.md
- docs/plans/fix-install-followups-closeout.md
- .dev/plans/refactor-golem-auditor.prompt.md
- .dev/plans/feat-small-context-task-authoring.prompt.md

## Session Continuity

| Plan | Source Plan | Last Session | Stopped At | Next Step | Context |
| --- | --- | --- | --- | --- | --- |
| refactor-gal-core-rust-port | docs/plans/refactor-gal-core-rust-port.md | 2026-06-10 | **T-032 complete; T-033 blocked** — shell entrypoints forward Rust-owned core subcommands and the `gal` / `gal-init` templates are updated, but `gal.*` / `common.*` deletion is still reserved for the xmachine plan's `T-015` joint end-gate. Post-merge review (2026-06-10) of T-031/T-032 PASS, no fix needed: T-031 doctor aggregation has zero overlap with run_doctor's internal checks and is correctly composed at the cli root; T-032 shell forwarding verified ($repoRoot/$script_dir defined, $script_root→$script_dir is a fix, no duplicate arms, no switch fall-through) and live-smoke-tested (`gal.ps1 doctor` → `target/debug/gal.exe`). No UAC-trap test file this round. Full `cargo test --workspace` (405 passed) + clippy green. | Wait for xmachine plan T-015, then re-evaluate T-033 | Hard blocker: live xmachine-family consumers still import `scripts/gal.ps1`, `scripts/gal.sh`, `scripts/common/Common.ps1`, and `scripts/common/common.sh` (`Invoke-XmachineTask.*`, `Invoke-XmachinePipeline.*`, `Test-Xmachine.ps1`). |
| refactor-gal-xmachine-rust-port | docs/plans/refactor-gal-xmachine-rust-port.md | 2026-06-10 | /plan-to-prompt DONE — 執行 prompt 生成(英文,15 T-NNN + 13 TP,Workflow=DRAFT);核心 R-00 前置已滿足 | Route ⑤:T-001 隨時可做;T-002+ 可啟動;T-004 等 Route ①②收斂;T-013 等 Route ③(T-034 artifact);stage.rs `audit` 改名(②)應先於 T-003 | 執行 prompt = .dev/plans/refactor-gal-xmachine-rust-port.prompt.md。T-002 硬依賴核心 base/dispatch。 | 架構 `base←dispatch←pipeline←xmachine`(DAG):pipeline 已做切小+多 provider(task-spec 歸此),xmachine=pipeline 純遠端升級(SSH+zellij)。二審折入:`Transport` trait 置 pipeline(xmachine 實作,反轉)、session 紀錄重用 `.dev/executor-logs/`(不另建 store)、xmachine 實作 base::HealthCheck、xmachine 獨立 crate=SSH/zellij dep 隔離。SSH/zellij/遠端 gal=使用者前提,GAL 只 preflight 不代設。跨機:win→mac(涵蓋 linux)+win→win(Win11)。**TP-17 要重做**借 TP-15 場地。依賴核心 R-00 先抽 base/dispatch。 |
| fix-install-followups-closeout | docs/plans/fix-install-followups-closeout.md | 2026-06-10 | **/deep-planning DONE(全面重勘版)— architect APPROVE** — 對照現碼重寫:S-2/FU-04 已被 Rust port 閉合(改 R-V1/R-V2 驗證關閉)、MCP serializers 縮為 AGY-only(codex/opencode 已存在 crates/mcp)、FU-03 擴大併入 R-06 review 的 ledger-skip 發現、失效引用(feat-gal-rust-native-install/舊路徑)全數更正 | /refining-plan(Route ④;M1 隨時可做,R-03 cosign 掛核心 T-034) | 2 修復(R-01 regex/R-02 ledger 精確+必達)+2 驗證關閉(R-V1 AGY path/R-V2 mode timeout)+M2(R-04 AGY 交易/R-05 AGY serializer)+R-03 cosign 審查 gate+bookkeeping(R-07 orphan/R-08 終態);M1/M2 不拆計畫(重勘後 M1 僅 2 小修) |
| refactor-golem-auditor | docs/plans/refactor-golem-auditor.md | 2026-06-10 | **/plan-to-prompt DONE — 執行 prompt 已生成(英文)** `.dev/plans/refactor-golem-auditor.prompt.md`,Workflow=DRAFT、Step 0 of 4。4 T-NNN + 8 TP。T-01 agent 合併(C1 STOP 逐字保留)→ T-02 相位鍵+pipeline 原子切換(C2 單 commit)→ T-03 coding.md 政策(C3)→ T-04 29 檔清零+sync 重生。架構 APPROVE-with-conditions C1..C5、ENG CLEAR 已帶入 prompt | **BLOCKED on Route ①** — 等 feat-small-context 落地後啟動(T-02 agentMap 疊在 ① 擷取器升級上,D-3/C5);T-01 技術上獨立但整計畫排在 ① 後 | C1 安全 STOP 語意逐字保留;C2 stage.rs+SKILL+agentMap+routing example 同 commit 原子落地;C3 三層獨立性文件化;C4 機器本地 routing 改鍵指引(dispatch 訊息明說 REVIEWER→AUDITOR);C5 排序依賴 ① |
| feat-small-context-task-authoring | docs/plans/feat-small-context-task-authoring.md | 2026-06-10 | **T-06 complete** — the task contract now names required evidence and the pipeline now blocks dispatched PASS or APPROVE results that lack it, while preserving DEGRADED_BUNDLED fallback evidence rules. | Implement T-07 — add the gal-engine filename guard test | Route ① active. Run mode remains DEGRADED_BUNDLED while local copilot stays unauthenticated. The remaining work is the filename guard plus the final self-clean contract clause. |
