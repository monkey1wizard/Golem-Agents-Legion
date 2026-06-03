# GAL State

<!-- Per-task state (workflow step, deviations, test and review results) lives in the plan file's ## Status section, not here. This file tracks repo-level concerns only. -->

## Active Plans

| Plan | File | Plan Phase | Last Activity |
| --- | --- | --- | --- |
| GAL Bootstrap / Install 收斂與 Rust 漸進取代 | .dev/plans/fix-gal-bootstrap-install-convergence.prompt.md | IMPLEMENT | 2026-06-03 (T-007 complete) |
| Install/Update/Uninstall 統一 + 傳播穩定化 | .dev/plans/fix-install-ownership-stabilization.prompt.md | IMPLEMENT | 2026-06-01 |
| Upgrade PDF Skill to Chandra OCR | docs/plans/feat-pdf-chandra-upgrade.md | REFINING | 2026-05-30 |
| Plugin Bin Migration | docs/plans/plugin-bin-migration.md | DEEP-PLANNING | 2026-06-01 |
| Migrate CLI providers to `.gal/plugins/gal` architecture | docs/plans/feat-plugin-arch-migration.md | VERIFIED | 2026-06-01 |
| GAL Pipeline Token Burn Reduction | .dev/plans/fix-gal-pipeline-token-burn.prompt.md | VERIFY | 2026-05-29 |
| Doc Keeper Golem (golem-dockeeper) | .dev/plans/feat-golem-dockeeper.prompt.md | VERIFIED | 2026-06-01 |
| AI 規劃文件語言策略 | .dev/plans/feat-ai-plan-language-strategy.prompt.md | IMPLEMENT | 2026-06-01 |
| Headless CLI Pipeline Orchestration | .dev/plans/headless-cli-pipeline.prompt.md | DRAFT | 2026-06-01 |
| Docs Architecture Restructure | docs/plans/refactor-docs-restructure.md | IMPLEMENTED (Slices 1–5 done) | 2026-06-02 |

<!-- When more than one plan is active, table order is priority order. `/gal whats-next` and `/gal wrap-up` use the first non-terminal row; if all rows are terminal, they fall back to the first row. -->

## Global Decisions

| Date | Decision | Rationale | Scope |
| --- | --- | --- | --- |

## Blockers

None.

## Session Continuity

<!-- Keep one row per active plan. Match rows by the paired source plan path. -->

| Plan | Source Plan | Last Session | Stopped At | Next Step | Context |
| --- | --- | --- | --- | --- | --- |
| GAL Bootstrap / Install 收斂與 Rust 漸進取代 | docs/plans/fix-gal-bootstrap-install-convergence.md | 2026-06-03 19:45 | T-007 complete (DEGRADED_SAME_RUNTIME), commit 8f536a4 | Start T-008 implementation (Rust MCP core+safe-merge) | R1+R2 phases complete (6 tasks); R3 started (T-007 done). T-007 added `render_copilot_manifest()`, `generate_plugin_version()`, chrono dependency. Manifest matches oracle, BUG-02 fixed (explicit component paths). 36/36 tests pass. Next: T-008 MCP core. |
| Install/Update/Uninstall 統一 + 傳播穩定化 | docs/plans/fix-install-ownership-stabilization.md | 2026-06-01 | T-014 complete (commit: eeb2a3ce3d08d4fe930b97e971f9a5f0d4392a73) | final verification and release closeout | T-013/T-014 已完成：rerun propagation 現在有 durable regression，會在同一 isolated home 內以 mutable temp source root 驗證「改 source → 不帶 `-Force` 重跑」能把新內容送到 canonical、AGY linked projection、Claude skills-dir projection，以及 Copilot refreshed host copy；maintainer docs 也已對齊 canonical/projection/doctor/ledger vocabulary，並在 `plugin-bin-migration.md` 補上 prerequisite cross-link。使用者仍要求在本次 invocation 跳過 executable bash 驗證；`plugin-bin-migration.md` 仍有既存 markdownlint spacing diagnostics，與本次 cross-link 無關。 |
| Upgrade PDF Skill to Chandra OCR | docs/plans/feat-pdf-chandra-upgrade.md | 2026-05-30 | Planning and Engineering Review completed. | Human approval, then `/plan-to-prompt` for execution. | Chandra OCR replacing pdfplumber/pytesseract for extraction, keeping pypdf for manipulation. |
| Plugin Bin Migration | docs/plans/plugin-bin-migration.md | 2026-06-01 | 轉換範圍盤點完成（PS1 ~11.6k 行/32 檔）：過半 ~6.2k 為安裝/設定編排=非 bin 候選、續留 scripts；bin 候選 A–D 量體小且內聚。**Architect APPROVE**（pass 4，安裝腳本邊界再確認：Phase 1 改 `Build-CorePlugin.*` 僅加 cargo 建置掛鉤、不重寫編排語義；新增 fail-loud + 「只准加掛鉤」護欄；無範圍回歸）。OQ-001/002/004 結案=單一 `gal` bin + `gal-core` 共用 crate；OE-01 過度範圍（init/setup 原生化）已修正 | **人類核准** → `/refining-plan` 鎖 ## Tasks/## Test Plan（以 Phase 1 wrapper dispatch 為首切片）→ `/plan-to-prompt` | 計畫已整份重寫清理：已結案 OQ/BUG/MISS 刪除、決策 bake 進 Decisions(Locked) 表。OQ-005 結案(dockeeper 進行中)、OQ-007 結案(待 gal-core 順帶加)；僅剩 OQ-006(候選 B 收斂閘 shadow 期，Phase-2 施工細節、不阻斷)。安裝/設定編排不原生化；候選 E 可能是唯一第二顆 bin、待 E 實作再定 |
| Migrate CLI providers to `.gal/plugins/gal` architecture | docs/plans/feat-plugin-arch-migration.md | 2026-06-01 | Verifier VERIFIED (10/10 + 3/3 tests pass; all 8 SC confirmed) | golem-releaser | All 3 tasks + verifier complete. DEGRADED_SAME_RUNTIME. Ready for release. |
| GAL Pipeline Token Burn Reduction | docs/plans/fix-gal-pipeline-token-burn.md | 2026-05-29 21:46 | Wrap-up handoff refreshed after final verifier pass | Release or close the plan; no implementation, test, or review work remains | Workflow=VERIFY; all tasks complete; numeric targets met; Bash runtime limitation recorded explicitly |
| Doc Keeper Golem (golem-dockeeper) | docs/plans/feat-golem-dockeeper.md | 2026-06-01 | Verifier VERIFIED | release prep | dockeeper/doc-sync/NDJSON/workflow/advisory-MCP/registry-adapter 目標已達成；`.dev` 狀態檔為磁碟層 durable state，source plan task closeout 已逐項落 git |
| AI 規劃文件語言策略 | docs/plans/feat-ai-plan-language-strategy.md | 2026-06-01 | T-009 complete (commit: 1d6d0ae241e3461542a84346c82e8ef23848a9b6); adapters and baked command skills were regenerated and now carry the language-policy updates | 等待 verifier closeout | Verification Independence: DEGRADED_SAME_RUNTIME; T-006 已完成(4747b54d2c0354879a1bf7c05880e2701145f344); T-001 已完成(140fe80759f552d6ee4519aeddd9af8049fe48be); T-002 已完成(8d55ce426738e4aaec0927e08e8a2560e087b8fa); T-003 已完成(62b4c534c8f4d03569b2abfe62801e6f1cad109f); T-004 已完成(e43bc1e982919bf6d081b5dd747439c06311f805); T-005 已完成(2d821476ce4caf04763601d0699ad2611c1f2917); T-007 已完成(8e2097eec500f05e48c36d1fe5c262cc27776c0d); T-008 已完成(108da609b6739c3bd03888be747fc031725e81b8); T-009 已重建 adapters 與 command skills |
| Headless CLI Pipeline Orchestration | docs/plans/headless-cli-pipeline.md | 2026-06-02 | Verifier VERIFIED (17/17 SC, 0 GAP). | golem-releaser | All 13 tasks complete. DEGRADED_SAME_RUNTIME. READY FOR RELEASE. |
| Docs Architecture Restructure | docs/plans/refactor-docs-restructure.md | 2026-06-02 | Slice 1–4 完成（T-001~T-013）。Slice 4 含 T-008（godot/graphics EN canonical + zh→i18n）、T-009 devguide Documentation Conventions、T-010 i18n/guide.md、T-011 翻譯 front-matter + Test-TranslationFreshness(PS+Bash)、T-012 project.md 索引+Project Language、T-013 structure-map 修正 | Plan implemented — verification/closeout next（人工 commit 後）。後續非阻斷項：zh 重譯（README/manual/xmachine 標 stale）、全 docs h2/h3 structure-map 物化（dockeeper reconcile）、blender-mcp.md 仍 zh-canonical（R13 未涵蓋）、R14 graphify-execution-guide 拆分 | Slice 5 完成：T-014 重生 4 adapters（乾淨，僅保留 project.md 索引的「was X」註記）、T-015 連結掃描修好 restructure 造成的斷鏈（blender-mcp→freecad、godot.zh）、xmachine.zh 標 stale。剩餘 6 個 broken link 皆 pre-existing 或 docs/.dev plans（設計上排除）。 |

## Recently Closed

| Plan | Source Plan | Closed On | Outcome |
| --- | --- | --- | --- |
| Install GAL into Claude Code / Claude Desktop / Antigravity | docs/plans/feat-install-antigravity-claude-desktop.md | 2026-05-31 | All 8 tasks complete + post-pipeline AGY GUI fix. Claude Code marketplace, Claude Desktop MCP safe-merge, Antigravity 2.0 GUI all working. Closed via cross-plan verification (feat-plugin-arch-migration.md Superseded). Plan and prompt deleted. |
| Migrate GAL install architecture to ~/.gal | docs/plans/migrate-install-architecture.md | 2026-05-29 | Closeout complete. Canonical `.gal` install architecture is verified; remaining Bash and remote xmachine smoke limits are environmental only. |
