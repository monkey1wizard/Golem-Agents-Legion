# Repository GAL state

## Active Plans

- docs/plans/feat-gal-rust-native-install.md
- docs/plans/refactor-golem-auditor.md
- docs/plans/feat-small-context-task-authoring.md

## Session Continuity

| Plan | Source Plan | Last Session | Stopped At | Next Step | Context |
| --- | --- | --- | --- | --- | --- |
| feat-gal-rust-native-install | docs/plans/feat-gal-rust-native-install.md | 2026-06-08 | Source plan drafted — 合併 plugin-bin-migration（已刪）+ dockeeper 不可用之 install 收斂修復 | /deep-planning — 觸及受保護路徑（scripts/、gal-engine 核心、install 拓撲）+ 與 bootstrap 引擎邊界，需 architect | 實機根因 RC-1..5：gal 不在 PATH、canonical root 缺、Claude Code 載入 stale v1.0.0 快取（無 dockeeper/doc-sync）、9 個孤兒 .gal-render-*、收斂目標 ≠ 實際載入面；OQ-01 Claude plugin 更新觸發契約須先鎖；消費 bootstrap Rust 引擎，本計畫負責端對端正確安裝+活面收斂+bin+清理+驗證 |
| refactor-golem-auditor | docs/plans/refactor-golem-auditor.md | 2026-06-05 | Source plan drafted (/planning, 拆自 small-context plan) | /deep-planning — golem 架構重組 + 受保護路徑 + OQ-001 取捨需 architect | golem-reviewer + golem-security → golem-auditor（深度效能+安全，常態 dispatch）；正確性審查移入 orchestrator inline gate（清單 1–13+明顯效能）；REVIEWER→AUDITOR routing 鍵；34 檔引用遷移（model-roles 級 blast radius）；協調點：New-TaskSpec.ps1 agentMap + routing schema 與 small-context plan 同檔 |
| feat-small-context-task-authoring | docs/plans/feat-small-context-task-authoring.md | 2026-06-05 | /deep-planning DONE — architect APPROVE; OQ-002/003 resolved; 第三面向已拆出 | /refining-plan（Tasks/Test Plan 仍 placeholder） | 縮為 2 面向：(1) template Approval 置頂 (2) 自足 task + New-TaskSpec 擷取器升級（核心）；自足=指標式非全文（3 模型 window 200K/400K/1M 非瓶頸）；spec 組裝在 New-TaskSpec.ps1（bin 不動），擷取器須升級擷多行+每 task 檔案 (R-004, BUG-01/02)；review 模型重組已拆至 refactor-golem-auditor.md |
