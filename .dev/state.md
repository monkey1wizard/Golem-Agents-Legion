# Repository GAL state

## Active Plans

- .dev/plans/refactor-gal-core-rust-port.prompt.md
- .dev/plans/refactor-gal-xmachine-rust-port.prompt.md
- docs/plans/fix-install-followups-closeout.md
- docs/plans/refactor-golem-auditor.md
- docs/plans/feat-small-context-task-authoring.md

## Session Continuity

| Plan | Source Plan | Last Session | Stopped At | Next Step | Context |
| --- | --- | --- | --- | --- | --- |
| refactor-gal-core-rust-port | docs/plans/refactor-gal-core-rust-port.md | 2026-06-09 | T-006 complete (commit: ba5fc69) — `HealthCheck` trait + Severity/DoctorFinding/DoctorReport 移入 base::health;9 個 doctor check 遷移為 trait 實作;exit 分級 byte 不變;228 綠。Pipeline 執行中:FROM=T-003, STOP_AT=T-008, DEGRADED_SAME_RUNTIME。 | T-007 — 抽 `providers` crate(claude/copilot/agy 投影自 gal-engine 移出;install 依 providers;無環——BUG-01 已於 T-003 預先解,MCP 型別在 base) | 執行 prompt runnable;P0 done。R-00 進行中(T-003/T-004/T-005/T-006 ✓,續 T-007/T-008)。架構師條件 BUG-02(T-008 保留 gal-dispatch bin 名)待 T-008 落實;OE-01(T-005)/BUG-01(T-003)已落實。真機 end-user 驗收(T-035)前置=T-034 R-13 CI 產 macOS-arm64 artifact。治理:測試機純 end-user 裝預編譯檔,絕不 build。xmachine 依賴本計畫 R-00。 | 全 scripts/ 核心層 Rust 化。架構:無前綴 base/providers/install/mcp/adapters/release/vcs/setup/cli,engine 退役,JIT 解耦。Motivation=①消雙實作②單 binary;終態零 ps1/sh 無例外。刪除 gate=fixture-parity 綠+混合態不變量。已 done:a192df7..bcc755a;本計畫 T-003=1ffd3de、T-004=5c1514e、T-005=8f78581、T-006=ba5fc69。 |
| refactor-gal-xmachine-rust-port | docs/plans/refactor-gal-xmachine-rust-port.md | 2026-06-09 | /plan-to-prompt DONE — 執行 prompt 生成(英文,15 T-NNN + 13 TP,Workflow=DRAFT) | 待核心計畫 R-00(T-003..T-009)落地後啟動 T-002 | 執行 prompt = .dev/plans/refactor-gal-xmachine-rust-port.prompt.md。T-002 硬依賴核心 base/dispatch。 | 架構 `base←dispatch←pipeline←xmachine`(DAG):pipeline 已做切小+多 provider(task-spec 歸此),xmachine=pipeline 純遠端升級(SSH+zellij)。二審折入:`Transport` trait 置 pipeline(xmachine 實作,反轉)、session 紀錄重用 `.dev/executor-logs/`(不另建 store)、xmachine 實作 base::HealthCheck、xmachine 獨立 crate=SSH/zellij dep 隔離。SSH/zellij/遠端 gal=使用者前提,GAL 只 preflight 不代設。跨機:win→mac(涵蓋 linux)+win→win(Win11)。**TP-17 要重做**借 TP-15 場地。依賴核心 R-00 先抽 base/dispatch。 |
| fix-install-followups-closeout | docs/plans/fix-install-followups-closeout.md | 2026-06-09 | Source plan drafted — 收尾 install-followups 殘留（A 子集歸 feat-gal，不在此） | /deep-planning — 觸及受保護 gal-engine 核心 + 與 feat-gal 檔案衝突協調，需 architect | 範圍 = B(hardening: S-1 regex/S-2 AGY panic/FU-03 ledger/FU-04 mode.rs timeout) + C(M2: S-3 cosign 再審/AGY 交易/MCP Codex+OpenCode serializers) + optional R5 commit-msg + bookkeeping(orphan plans)；R-04 與 feat-gal T-002 同改 mode.rs 須排序；OQ：hardening 與 M2 是否拆兩計畫由 architect 裁 |
| refactor-golem-auditor | docs/plans/refactor-golem-auditor.md | 2026-06-05 | Source plan drafted (/planning, 拆自 small-context plan) | /deep-planning — golem 架構重組 + 受保護路徑 + OQ-001 取捨需 architect | golem-reviewer + golem-security → golem-auditor（深度效能+安全，常態 dispatch）；正確性審查移入 orchestrator inline gate（清單 1–13+明顯效能）；REVIEWER→AUDITOR routing 鍵；34 檔引用遷移（model-roles 級 blast radius）；協調點：New-TaskSpec.ps1 agentMap + routing schema 與 small-context plan 同檔 |
| feat-small-context-task-authoring | docs/plans/feat-small-context-task-authoring.md | 2026-06-05 | /deep-planning DONE — architect APPROVE; OQ-002/003 resolved; 第三面向已拆出 | /refining-plan（Tasks/Test Plan 仍 placeholder） | 縮為 2 面向：(1) template Approval 置頂 (2) 自足 task + New-TaskSpec 擷取器升級（核心）；自足=指標式非全文（3 模型 window 200K/400K/1M 非瓶頸）；spec 組裝在 New-TaskSpec.ps1（bin 不動），擷取器須升級擷多行+每 task 檔案 (R-004, BUG-01/02)；review 模型重組已拆至 refactor-golem-auditor.md |
