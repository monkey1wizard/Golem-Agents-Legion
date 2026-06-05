# Repository GAL state

## Active Plans
- docs/plans/refactor-golem-auditor.md
- docs/plans/feat-small-context-task-authoring.md
- .dev\plans\refactor-headless-pipeline-rust.prompt.md

## Session Continuity

| Plan | Last Session | Stopped At | Next Step | Context |
|------|-------------|-----------|----------|---------|
| docs/plans/refactor-golem-auditor.md | 2026-06-05 | Source plan drafted (/planning, 拆自 small-context plan) | /deep-planning — golem 架構重組 + 受保護路徑 + OQ-001 取捨需 architect | golem-reviewer + golem-security → golem-auditor（深度效能+安全，常態 dispatch）；正確性審查移入 orchestrator inline gate（清單 1–13+明顯效能）；REVIEWER→AUDITOR routing 鍵；34 檔引用遷移（model-roles 級 blast radius）；協調點：New-TaskSpec.ps1 agentMap + routing schema 與 small-context plan 同檔 |
| docs/plans/feat-small-context-task-authoring.md | 2026-06-05 | /deep-planning DONE — architect APPROVE; OQ-002/003 resolved; 第三面向已拆出 | /refining-plan（Tasks/Test Plan 仍 placeholder） | 縮為 2 面向：(1) template Approval 置頂 (2) 自足 task + New-TaskSpec 擷取器升級（核心）；自足=指標式非全文（3 模型 window 200K/400K/1M 非瓶頸）；spec 組裝在 New-TaskSpec.ps1（bin 不動），擷取器須升級擷多行+每 task 檔案 (R-004, BUG-01/02)；review 模型重組已拆至 refactor-golem-auditor.md |
| docs/plans/refactor-headless-pipeline-rust.md | 2026-06-05 | Wrap-up — all phases closed; matrix 17/20 PASS | OPTIONAL: run TC-06/07/08 (codex orch → copilot/opencode/agy) — goal already met | Phases 0–3 COMPLETE (46 tests pass); matrix 17/20 PASS, TC-06/07/08 unexecuted but combinatorially redundant (codex-orch proven TC-05; executors proven elsewhere); all Success Criteria met; codex/copilot exclusion overturned; agy.rs cross-platform brain-dir fix landed; executor-routing.json is sole role-config source; plan ready for deletion after user accepts 17/20 or runs final 3 |
