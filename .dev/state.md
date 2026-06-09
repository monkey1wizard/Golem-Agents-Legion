# Repository GAL state

## Active Plans

- .dev/plans/feat-gal-rust-native-install.prompt.md
- docs/plans/fix-install-followups-closeout.md
- docs/plans/refactor-golem-auditor.md
- docs/plans/feat-small-context-task-authoring.md

## Session Continuity

| Plan | Source Plan | Last Session | Stopped At | Next Step | Context |
| --- | --- | --- | --- | --- | --- |
| feat-gal-rust-native-install | docs/plans/feat-gal-rust-native-install.md | 2026-06-09 | T-006 complete (c882267) — check_skill_surface + check_bin_in_canonical_root added to doctor; ClaudeMarketplaceState removed; 165 gal-engine + 12 gal-cli tests pass | T-007 (P5 cross-platform): gal-engine Windows junction/Unix symlink +x path/permission alignment | 範圍裁決：移除官方 marketplace/`/plugin`；Claude skill 面 = `~/.claude/skills/gal`（非 legacy `plugins/gal`，oracle common.sh:75）；**subagent（dockeeper）活載入移入 Non-Goals 延後**（無 GAL 自有非官方面）。本機已乾淨重裝：canonical root 正確（13 agents 含 dockeeper、29 skills 含 doc-sync）、9 孤兒清除、doctor green。**驗證順序（SSH 連 mac-mini）：(1) mac-mini 純 normal mode 乾淨安裝先做（專測 packaged-source 自解析，最未驗）→ (2) 複製本機 ~/.gal/config 過去 → (3) Windows 後做、normal 後再 dev mode。TP-15(mac normal)/TP-16(win normal+dev)** |
| fix-install-followups-closeout | docs/plans/fix-install-followups-closeout.md | 2026-06-09 | Source plan drafted — 收尾 install-followups 殘留（A 子集歸 feat-gal，不在此） | /deep-planning — 觸及受保護 gal-engine 核心 + 與 feat-gal 檔案衝突協調，需 architect | 範圍 = B(hardening: S-1 regex/S-2 AGY panic/FU-03 ledger/FU-04 mode.rs timeout) + C(M2: S-3 cosign 再審/AGY 交易/MCP Codex+OpenCode serializers) + optional R5 commit-msg + bookkeeping(orphan plans)；R-04 與 feat-gal T-002 同改 mode.rs 須排序；OQ：hardening 與 M2 是否拆兩計畫由 architect 裁 |
| refactor-golem-auditor | docs/plans/refactor-golem-auditor.md | 2026-06-05 | Source plan drafted (/planning, 拆自 small-context plan) | /deep-planning — golem 架構重組 + 受保護路徑 + OQ-001 取捨需 architect | golem-reviewer + golem-security → golem-auditor（深度效能+安全，常態 dispatch）；正確性審查移入 orchestrator inline gate（清單 1–13+明顯效能）；REVIEWER→AUDITOR routing 鍵；34 檔引用遷移（model-roles 級 blast radius）；協調點：New-TaskSpec.ps1 agentMap + routing schema 與 small-context plan 同檔 |
| feat-small-context-task-authoring | docs/plans/feat-small-context-task-authoring.md | 2026-06-05 | /deep-planning DONE — architect APPROVE; OQ-002/003 resolved; 第三面向已拆出 | /refining-plan（Tasks/Test Plan 仍 placeholder） | 縮為 2 面向：(1) template Approval 置頂 (2) 自足 task + New-TaskSpec 擷取器升級（核心）；自足=指標式非全文（3 模型 window 200K/400K/1M 非瓶頸）；spec 組裝在 New-TaskSpec.ps1（bin 不動），擷取器須升級擷多行+每 task 檔案 (R-004, BUG-01/02)；review 模型重組已拆至 refactor-golem-auditor.md |
