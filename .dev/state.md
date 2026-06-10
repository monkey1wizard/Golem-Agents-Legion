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
| refactor-gal-core-rust-port | docs/plans/refactor-gal-core-rust-port.md | 2026-06-10 | T-018 complete (commit: 2c68fe2) — R-04 architect sign-off APPROVE-with-conditions (C-1..C-10, recorded in prompt ## Review Results); `setup` crate split (session/agy/legacy_plugins/health), `gal setup` wired (CommandKind 10), 377 workspace tests green, live dry-run parity vs Setup-Machine verified incl. identical failure on degraded machine state; reviewer APPROVE (0 blocking, 4 info). Run mode: DEGRADED_BUNDLED (user-directed). Known dev-machine issue (pre-existing, out of scope): `~/.gal/plugins/gal` canonical root missing — `gal install` should restore. | T-019 (R-04) — port setup-tools → `gal setup --tools` | R-04 in progress (stop-at T-021)。T-018 done：setup crate 只編排,machine surfaces/MCP 走 library call,Install-GalPlugins 為唯一 strangler spawn 點(T-024 改接)。C-6 正規化已記錄(sh installMode 退役、dry-run MCP 抑制、ripgrep 自動安裝延至 T-019、偵測簡化)。Windows UAC 陷阱:setup-*.exe 測試二進位需嵌 asInvoker manifest(build.rs)。 |
| refactor-gal-xmachine-rust-port | docs/plans/refactor-gal-xmachine-rust-port.md | 2026-06-09 | /plan-to-prompt DONE — 執行 prompt 生成(英文,15 T-NNN + 13 TP,Workflow=DRAFT) | 待核心計畫 R-00(T-003..T-009)落地後啟動 T-002 | 執行 prompt = .dev/plans/refactor-gal-xmachine-rust-port.prompt.md。T-002 硬依賴核心 base/dispatch。 | 架構 `base←dispatch←pipeline←xmachine`(DAG):pipeline 已做切小+多 provider(task-spec 歸此),xmachine=pipeline 純遠端升級(SSH+zellij)。二審折入:`Transport` trait 置 pipeline(xmachine 實作,反轉)、session 紀錄重用 `.dev/executor-logs/`(不另建 store)、xmachine 實作 base::HealthCheck、xmachine 獨立 crate=SSH/zellij dep 隔離。SSH/zellij/遠端 gal=使用者前提,GAL 只 preflight 不代設。跨機:win→mac(涵蓋 linux)+win→win(Win11)。**TP-17 要重做**借 TP-15 場地。依賴核心 R-00 先抽 base/dispatch。 |
| fix-install-followups-closeout | docs/plans/fix-install-followups-closeout.md | 2026-06-09 | Source plan drafted — 收尾 install-followups 殘留（A 子集歸 feat-gal，不在此） | /deep-planning — 觸及受保護 gal-engine 核心 + 與 feat-gal 檔案衝突協調，需 architect | 範圍 = B(hardening: S-1 regex/S-2 AGY panic/FU-03 ledger/FU-04 mode.rs timeout) + C(M2: S-3 cosign 再審/AGY 交易/MCP Codex+OpenCode serializers) + optional R5 commit-msg + bookkeeping(orphan plans)；R-04 與 feat-gal T-002 同改 mode.rs 須排序；OQ：hardening 與 M2 是否拆兩計畫由 architect 裁 |
| refactor-golem-auditor | docs/plans/refactor-golem-auditor.md | 2026-06-05 | Source plan drafted (/planning, 拆自 small-context plan) | /deep-planning — golem 架構重組 + 受保護路徑 + OQ-001 取捨需 architect | golem-reviewer + golem-security → golem-auditor（深度效能+安全，常態 dispatch）；正確性審查移入 orchestrator inline gate（清單 1–13+明顯效能）；REVIEWER→AUDITOR routing 鍵；34 檔引用遷移（model-roles 級 blast radius）；協調點：New-TaskSpec.ps1 agentMap + routing schema 與 small-context plan 同檔 |
| feat-small-context-task-authoring | docs/plans/feat-small-context-task-authoring.md | 2026-06-05 | /deep-planning DONE — architect APPROVE; OQ-002/003 resolved; 第三面向已拆出 | /refining-plan（Tasks/Test Plan 仍 placeholder） | 縮為 2 面向：(1) template Approval 置頂 (2) 自足 task + New-TaskSpec 擷取器升級（核心）；自足=指標式非全文（3 模型 window 200K/400K/1M 非瓶頸）；spec 組裝在 New-TaskSpec.ps1（bin 不動），擷取器須升級擷多行+每 task 檔案 (R-004, BUG-01/02)；review 模型重組已拆至 refactor-golem-auditor.md |
