# Plan: Install-Followups 殘留收尾（fix-install-followups-closeout）

> 收尾 `docs/observations/install-followups.md` 中**無法由安裝驗證閉合**的殘留項（security hardening、低優先 code 修、M2 features、bookkeeping）。安裝路徑/跨平台子集（FU-01、FU-02、macOS）由 `feat-gal-rust-native-install` 的實機驗證閉合，**不在本計畫**。

## Goal

把 `install-followups.md` 清空到只剩「已由他計畫閉合」與「已完成」：完成需要實際 code 變更或審查的殘留項 —— secret-guard / AGY robustness / ledger 精確度 / dead-path timeout（hardening），cosign CI 信任再審、AGY 交易/ledger、MCP 其餘 serializers（M2），commit-msg scope injection（optional），以及 orphan plan 確認（bookkeeping）。完成後 `install-followups.md` 可瘦身或刪除。

## Governing Principle

每個殘留項都對應 `install-followups.md` 既有 ID（FU-/S-/R5）+ 具體檔案。本計畫不重述根因，只**閉合**。安全項以實際行為/測試證明修復，不接受「看起來修好了」。

## Context

- `install-followups.md` 是已刪 bootstrap 計畫（fix-gal-bootstrap-install-convergence）的 durable 殘留清單。
- **A 子集（FU-01 normal-mode packaged-source、FU-02 oracle-parity、macOS/Linux 跨平台實機）由 `feat-gal-rust-native-install` 的 TP-15/16 + 跨平台任務閉合**；本計畫不碰（Non-Goals），但依賴其完成以宣告 install-followups 全閉。
- 本計畫多數項觸及 `crates/gal-engine/src/`（受保護、bootstrap 擁有）→ 須 `/deep-planning` architect 審 + bootstrap 邊界簽核。

## Requirements

**M1 — Hardening（小 code，高 CP 值）**

- [ ] **R-01（S-1）secret-guard backstop regex** — `crates/gal-engine/src/providers/mod.rs::has_unresolved_secrets` 的 anchored regex `^\$\{([A-Z0-9_]+)\}$` 只攔整串 `${SECRET}`；對齊內嵌型（如 `Bearer ${API_KEY}`）。非洩密（上游 resolver 已攔），屬縱深防禦。
- [ ] **R-02（S-2）AGY 路徑 `unwrap()` panic** — `providers/agy.rs` `create_link`/junction 移除的 `path.to_str().unwrap()` 對非 UTF-8 home path panic；改 graceful best-effort error（AGY 非致命）。
- [ ] **R-03（FU-03）uninstall ledger 精確度** — `install.rs::run_uninstall()` 硬寫 `providers: []` / `mode: "normal"`；改用 `Ledger.last` 實際值。
- [ ] **R-04（FU-04）`is_readable` dead-path timeout** — `mode.rs::is_gal_root_usable()` spawn thread 但 `join()` 無限阻塞，文件宣稱的 dead-path 短逾時未實作。**與 `feat-gal-rust-native-install` T-002 協調**（同改 `mode.rs`）：T-002 先落地則本項接續，避免衝突。

**M2 — Features / 審查（較大工程）**

- [ ] **R-05（S-3）cosign CI 信任 security re-review** — bootstrap T-014 已落地 cosign keyless code，但簽章信任設定（workflow identity + Rekor）的 security re-review 未記錄；**package-manager 發布前**完成。
- [ ] **R-06（AGY M2）交易/ledger** — AGY 三 surface best-effort 已完成（bootstrap T-010）；補完整交易回滾 + ledger 整合（OE-A 延後項）。
- [ ] **R-07（MCP M2）AGY/Codex/OpenCode serializers** — Claude Desktop + Copilot CLI serializers 已完成（bootstrap T-009）；補 AGY、Codex CLI、OpenCode（TOML）serializers。
- [ ] **R-08（R5，optional）commit-msg scope injection** — `process_commit_msg()` 目前只保留訊息（no-hijack 已確認）；補依 changed files 自動加 scope 前綴。非必要、最低優先。

**Bookkeeping**

- [ ] **R-09 orphan plan 確認** — `install-followups.md` 列的 orphan：`manage-external-plugins.md`（無執行脈絡，確認關閉或續做）；`feat-gal-file-memory-strategy.md`（**注意：已有 `.dev/plans/...prompt.md`，非真 orphan，確認狀態**）。確認後更新 `.dev/state.md` 與 install-followups 收斂段。

## Non-Goals

- **A 子集（FU-01 / FU-02 / macOS·Linux 跨平台實機）** — 由 `feat-gal-rust-native-install` 閉合，不在本計畫重做。Linux 實機待 host 亦歸該計畫。
- 不重寫 bootstrap 引擎架構；只做點狀 hardening + 補完 M2 serializer/交易，沿用既有契約。

## Approach

| 階段 | 內容 | 前置 |
| --- | --- | --- |
| **M1 Hardening** | R-01..R-04 點狀 code 修 + 單元測試 | R-04 與 feat-gal T-002 協調 |
| **M2 Features** | R-05 cosign 再審、R-06 AGY 交易/ledger、R-07 MCP 其餘 serializers | M1 可獨立 |
| **Optional** | R-08 commit-msg scope injection | 隨時 |
| **Bookkeeping** | R-09 orphan plan 確認 + install-followups 收斂 | 全項完成後 |

- 每項以 `install-followups.md` 對應 ID 為單位，修完即在該檔標 RESOLVED（commit ref）。
- M1 四項皆小且互相獨立，可平行；M2 三項各自較大，建議拆獨立 task。
- R-04 排在 feat-gal T-002 之後（或由 T-002 順手帶，屆時本項標「已由 T-002 閉合」）。

## Files to Create or Modify

- `[MODIFY]`（引擎核心，bootstrap 擁有，須簽核）`crates/gal-engine/src/providers/mod.rs`（R-01）、`providers/agy.rs`（R-02、R-06）、`install.rs`（R-03）、`mode.rs`（R-04）。
- `[MODIFY/CREATE]` `crates/gal-engine/src/providers/{codex,opencode}.rs` 或 mcp serializer 模組（R-07）。
- `[MODIFY]` MCP/AGY serializer 對應測試。
- `[MODIFY]` `.github/workflows/`（release 簽章相關，R-05 審查；如需）。
- `[MODIFY]` `crates/gal-cli/src/`（R-08 commit-msg，如做）。
- `[MODIFY]` `docs/observations/install-followups.md` — 完成項標 RESOLVED；全閉後瘦身/刪除。
- `[MODIFY]` `.dev/state.md` — R-09 orphan plan 確認結果。

## Success Criteria

- [ ] R-01/R-02/R-03/R-04 修復且有單元測試覆蓋（secret 內嵌型被攔、非 UTF-8 path 不 panic、uninstall ledger 記實際值、dead-path 短逾時生效）。
- [ ] R-05 cosign CI 信任 security re-review 完成並記錄（或確認 pkg 尚未發布、明確標 gate）。
- [ ] R-06 AGY 交易/ledger、R-07 MCP AGY/Codex/OpenCode serializers 完成且 oracle/行為測試通過。
- [ ] R-09 orphan plan 狀態確認、`.dev/state.md` 與 install-followups 一致。
- [ ] `install-followups.md` 殘留歸零（A 子集由 feat-gal 閉合、其餘由本計畫閉合），檔案可刪或僅留歷史註記。

## Risks

- **與 feat-gal-rust-native-install 同改 `mode.rs`（R-04）/`agy.rs`（R-02 vs R-06）衝突** — Mitigation：R-04 排在 feat-gal T-002 後；AGY 兩項（robustness R-02、交易 R-06）在同檔，合併為一個 AGY task 一次改。
- **M2 serializer 量體（R-07）** — Codex/OpenCode TOML 格式各異，可能比預期大。Mitigation：拆 per-provider sub-task，沿用既有 Claude/Copilot serializer 結構。
- **觸及受保護引擎核心** — Mitigation：`/deep-planning` architect 審 + bootstrap 邊界簽核；cross-model reviewer ≠ implementer。
- **cosign 信任設定（R-05）需實際 CI 環境驗證** — Mitigation：以 `cosign verify-blob` + Rekor 查詢實證，不接受「workflow 跑過」當通過。

## Open Questions

- [ ] OQ-01 — R-04（`mode.rs` dead-path timeout）由本計畫做，還是併入 `feat-gal-rust-native-install` T-002？*(raised by: install-followups closeout)*
- [ ] OQ-02 — M2 features（R-06 AGY 交易、R-07 MCP serializers）是否該與 hardening 同計畫，或拆獨立 M2 計畫？本計畫先納入、由 architect 裁是否拆。*(raised by: install-followups closeout)*
- [ ] OQ-03 — `manage-external-plugins.md` 關閉或續做？`feat-gal-file-memory-strategy` 已有 prompt，確認其 active 狀態。*(raised by: install-followups closeout)*

## Approval

- Human approval: [pending]
- Architect review: [required — `/deep-planning`]。觸及受保護的 `crates/gal-engine/src/` 核心（providers/mode/install）+ bootstrap 邊界，須 architect 審 scope 切割（hardening vs M2 是否同計畫）、與 feat-gal 的檔案衝突協調，方可進 `/refining-plan`。
- Additional domain review: [not triggered]（無 customer-facing / business-rule 內容）

## Review Results

### Architecture Review

Pending（`/deep-planning`）。重點待審：(1) hardening（M1）與 M2 features 是否該拆兩計畫；(2) 與 `feat-gal-rust-native-install` 在 `mode.rs`/`agy.rs` 的檔案衝突排序；(3) R-07 MCP serializer 量體是否需拆子任務。

### Engineering Review

Pending（`/refining-plan`）。`## Tasks` 與 `## Test Plan` 待 `/refining-plan` 展開。

## Tasks

Pending `/refining-plan`.

## Test Plan

Pending `/refining-plan`.
