# 計畫：GAL Bootstrap / Install 收斂（Rust-first 取代）

## Goal

讓 GAL 安裝真相可信，並以 Rust 成為 install / update / render / provider 邏輯的**單一實作**。使用者透過正式發布的 `gal` binary 進行 install / update / doctor 與 package-manager 安裝；repo source 變更後重跑一次 update 確定性重渲染 `~/.gal/plugins/gal/`，讓 Claude / Copilot（先）與 AGY（後）的讀取面看到同一份最新 GAL。

**策略：Rust-first 直接取代。** 凍結現有 PowerShell / Bash scripts 作為 Rust 重寫的 oracle 與過渡期 Claude/Copilot 可用實作，達 parity 後退休。**不再做 bash/ps1 修改。**

## Context

2026-06-02 install truth 盤點 + 2026-06-03 Rust-first 轉向。當前事實：

- 既有 source plan（`fix-install-ownership-stabilization`、`feat-plugin-arch-migration`、`feat-golem-dockeeper`）宣稱完成，但 runtime surfaces 未收斂：`~/.gal/plugins/gal/` 未含 `golem-dockeeper`/`doc-sync`；`~/.gemini/antigravity-cli/plugins/gal` 缺失。
- **Claude/Copilot 目前可用；AGY 僅文書用途、可等。** 故 install-truth-now 不緊迫，採 Rust-first、不再投資雙 shell。
- `docs/devguide.md` 有 GitHub Releases / winget / Homebrew 規格，但無 end-to-end package-manager install lane。
- `headless-cli-pipeline` 的 VERIFIED 為 overclaim（只證明 echo/mock + prompt-path OFFLOAD，未證真實 provider receipt），且 `.dev/state.md` 同時呈 DRAFT/VERIFIED，待 closeout。
- **已完成**：T-001 Rust skeleton；T-002 mode 解析（script，作 oracle）；T-003 canonical render（script，作 oracle）；commit-msg keyword-hijack 已修（凍結 script，commit `f64f517`）。

**關鍵限制（架構稽核 BUG-A）**：凍結 script 對 **Claude/Copilot steady-state 是正確 oracle**，但對「本計畫要修的東西」（dockeeper 渲染、AGY 投影）是**壞 oracle**——這些必須以**意圖行為**實作並用意圖斷言驗證，不可對凍結 script 做 byte-parity（否則複製 bug）。

## Requirements

- [ ] **R-001 Rust 入口**：可發布的 `gal` binary，提供 `--version` / `install` / `update` / `doctor` / `uninstall` 穩定入口；CLI 引數、錯誤分類、exit code 由 Rust 管理。
- [ ] **R-002 package-manager 真實安裝**：GitHub Releases canonical artifacts，並讓 `winget install Monkey1Wizard.GAL` 與 `brew install monkey1wizard/tap/gal` 成為可驗證安裝路徑。
- [ ] **R-003 bootstrap ownership**：package-manager 安裝只裝 bootstrap binary + 必要 release metadata，不預塞 `~/.gal/` runtime state。first-run 預設一般模式。

> **模式模型（config 權威，OQ-003）**
>
> 權威訊號在 `~/.gal/config/config.json`：
> - **一般模式**：預設。`"devMode"` 不存在或 `false`。從打包 source 渲染。
> - **開發者模式（dev mode）**：`"devMode": true` **且** `"galRoot"` usable。從 `galRoot` 的 repo working tree 渲染。
> - `devMode:true` 但 `galRoot` 不 usable → **報明確錯誤、exit 非零、不靜默 fallback**。
>
> **`galRoot` usable predicate**（全過才算）：非空字串 → 路徑存在且為目錄 → 可讀（dead UNC/網路路徑短逾時快速失敗）→ 同時含 `commands/`+`agent/`+`skills/`。
>
> **`installMode` 欄位棄用**：只看 `devMode` + `galRoot`，不讀 `installMode`。migration 由舊 `installMode` 一次性推導 `devMode`，doctor 對殘留矛盾 `installMode` 發 warning。

- [ ] **R-004 install/update spine（Rust 原生）**：一條流程：Rust 解析 config（Rust 擁有 config 真相）→ 重渲染 canonical root → 逐 provider 投影/刷新 → 寫 ledger → doctor 可驗證。
- [ ] **R-005 dev mode 不可半收斂**：dev mode 下 update 必須刷新 canonical root 與所有 selected provider surfaces，不得只處理 Claude。
- [ ] **R-006 AGY 三 surface（低優先）**：AGY CLI/IDE/GUI-config 應存在 GAL、link-first。使用者僅文書用、可等：R3 最後做，先 **best-effort copy**；交易/ledger 等正式修 AGY 再加（見 OE-A）。
- [ ] **R-007 dockeeper/doc-sync 可見（意圖，非 oracle）**：`golem-dockeeper`/`doc-sync` 必須從 repo source 渲染進 canonical root 並被 Claude/Copilot 讀取面發現。此為 bug-fix delta，以意圖斷言驗證。
- [ ] **R-008 Claude marketplace 誠實分類**：local plugin manager / skills-dir projection / public marketplace 三狀態分開記錄；local cache 不得當 public marketplace 證據。
- [ ] **R-009 plan lifecycle closeout**：盤點 completed/VERIFIED/implemented source plans，關閉已完成者、歸檔 orphaned prompts、follow-up 移獨立文件。`.dev/state.md` 只留真正 active plan。
- [ ] **R-010 commit-msg（已修）**：keyword-hijack 已在凍結 script 移除。Rust 化為選配（R5/T-012），可延後或移出本計畫。
- [ ] **R-011 Rust-first 取代策略**：Rust 直接實作 config→mode→render→projection→doctor，對齊凍結 Claude/Copilot oracle，達 parity 後退休 scripts。
- [ ] **R-012 plugin-bin 邊界重訂**：`plugin-bin-migration.md` 只做 upstream `gal` binary 的 plugin `bin/` exposure；Rust workspace / payload / `gal-core` 由本計畫擁有。
- [ ] **R-013 Rust 單一跨平台 runtime**：Rust 須涵蓋 Windows + macOS + Linux 路徑/symlink/junction。凍結 scripts 僅作 oracle、不要求對等。**跨平台 Rust 測試為 load-bearing**（見 BUG-C）。
- [ ] **R-014 doctor 為 release gate**：`gal doctor` read-only 檢查 canonical root、provider surfaces、ledgers、package-manager metadata、plan lifecycle state、known stale host caches。
- [ ] **R-015 headless executor truth closeout**：headless pipeline 由「已完成」改為「已實作但真實 provider receipt 未驗證」，修正或另開 follow-up。

## Non-Goals

- 不一次性把 doc-sync 的語意判斷、文件正文改寫或 code→doc 首次關聯 Rust 化。
- 不把 provider 目錄內 unknown / user-owned 檔案納入 GAL 所有權。
- 不把 public Claude marketplace submission 假裝完成（只建可驗證 gate + release lineage）。
- 不刪除 completed source plans 作歷史證據（只移出 active state）。

## Scope Decision

| 項目 | 決定 | 理由 |
| --- | --- | --- |
| 取代策略 | Rust-first 直接取代；不再做 bash/ps1 修改 | Claude/Copilot 可用、AGY 可等；消除 PS/Bash/Rust 三面對等稅與雙寫 drift |
| Rust 角色 | config/mode/render/projection/doctor 全在 Rust 原生實作 | 單一型別 runtime；atomic render 比 shell 可靠 |
| Scripts 角色 | 凍結，作 Rust oracle 與過渡期 Claude/Copilot 實作；達 parity 退休 | 對著正確 oracle 重寫比盲寫安全；過渡期不中斷 |
| Oracle 範圍 | 僅 Claude/Copilot steady-state 可 byte-parity；dockeeper/AGY 等 bug-fix delta 用意圖斷言 | 凍結 script 正是 bug 來源，不能 match 壞行為（BUG-A） |
| AGY | 低優先、R3 最後、先 best-effort | 僅文書用可等；完整交易/ledger 屬過度工程（OE-A） |
| Package managers | winget / Homebrew 納入 acceptance gate | 使用者要求正式 install lane |
| Existing plans | 完成 plan 作事實來源與反例，本計畫做 active-state closeout | 修「source plan 完成 ≠ runtime install 完成」 |

## Milestones

定義可發布里程碑（解 MISS-A：避免「可裝 binary」被推到全引擎完成之後）。

| 里程碑 | 內容 | 對應 |
| --- | --- | --- |
| **M1 — Claude/Copilot native parity** | Rust 原生 install/update 對 Claude+Copilot == oracle；`gal` 入口切 Rust；doctor 基本可用。**第一個值得發布的 binary。** | R1–R4（不含 AGY） |
| **M2 — 全覆蓋** | AGY 三 surface（含交易/ledger）、MCP 完整、跨平台（macOS/Linux）測試 | R3 AGY + R-013 |
| **M3 — release lane + gate** | package-manager artifacts、winget/Homebrew、release gate | P2 + P4 |

P2（package-manager lane）以 **M1 parity** 為前置即可開始，不必等 M2 AGY 完成。

## Approach

凍結 scripts 不 revert（T-002/T-003 是 oracle）。各 phase 可獨立驗證。

| Phase | 目標 | 前置 |
| --- | --- | --- |
| **已完成** | T-001 skeleton、T-002 mode（oracle）、T-003 render（oracle）、commit-msg 修復 | — |
| **R1** | gal-core 擁有 config + mode | T-001 |
| **R2** | Rust canonical render | R1、T-003 oracle |
| **R3** | Rust provider projection（Claude→Copilot→MCP→AGY） | R2 |
| **R4** | Rust 入口接管 + doctor + 退休 scripts（= M1） | R3 Claude/Copilot |
| **R5** | `gal commit-msg`（Rust，選配） | T-001 |
| **P2** | package-manager release lane | M1 parity、Security review |
| **P3** | state / plan / headless closeout（純 docs/state） | 隨時 |
| **P4** | release gate 收斂 | R4 doctor、P2 |

### R1：gal-core 擁有 config + mode

- **Files**: `crates/gal-core/src/config.rs`, `crates/gal-core/src/mode.rs`
- **What**: serde 讀/解析 `~/.gal/config/config.json` 成型別 model（Rust 擁有 config 真相）；缺檔/缺 key 有明確預設、不 panic。port T-002 mode predicate（devMode + galRoot usable + no-fallback error + dead-path 短逾時）。
- **Verify**: Rust unit tests 覆蓋 mode 矩陣；對同一 config，Rust mode == 凍結 PS `Test-InstallModeAuthority` oracle。

### R2：Rust canonical render

- **Files**: `crates/gal-core/src/render.rs`, `crates/gal-cli/src/`
- **What**: 從 galRoot（dev）或 packaged source 渲染 canonical root，**atomic temp dir + swap**；目錄掃描列舉 agents/skills（自動納入 `golem-dockeeper`/`doc-sync`）。
- **Oracle 範圍（BUG-A）**：Rust render 對 **Claude/Copilot 既有正確結構**可與凍結 oracle byte-parity；但 **dockeeper/doc-sync 屬 bug-fix delta**（凍結 script 可能未渲染），以意圖斷言驗證，**不要求 match 凍結 oracle**。
- **Verify**: 既有正確結構 == oracle；canonical root `agents/` 含 `golem-dockeeper`、`skills/` 含 `doc-sync`（意圖斷言）；kill-mid-render 重跑收斂、無半渲染/temp 殘留。

### R3：Rust provider projection（Claude → Copilot → MCP → AGY）

- **Files**: `crates/gal-core/src/providers/{claude,copilot,mcp,agy}.rs`
- **What**:
  - **Claude + Copilot**：skills-dir / installed-plugin projection，逐檔對齊現行可用 oracle。
  - **MCP merge**：plugin root 未完整前不寫半殘 `mcp_config.json`。**量體警示（BUG-D）**：取代的 `Update-Mcp.*` 是全 repo 最大 script（~78KB）；`/refining-plan` 須把 MCP 拆成子任務、標真實量體，勿當單一 bullet。
  - **AGY（低優先，OE-A）**：先 **best-effort copy** 把三 surface（CLI/IDE/GUI-config）建起；交易回滾 + ledger 降級**延後**到正式修 AGY（M2）時再加。
- **Verify**: Claude/Copilot 投影 == oracle（逐檔）；MCP 不產生半殘 root；AGY 三 surface 存在含必要檔（best-effort 階段不要求交易性）。

### R4：Rust 入口接管 + doctor + 退休 scripts（= M1）

- **Files**: `crates/gal-cli/src/`, `scripts/gal.ps1`（entry 切換）, `scripts/*`（凍結標記）
- **What**: `gal install`/`update`/`uninstall` 接 Rust 原生實作（config→render→projection→ledger，一條 path，R-004）。Claude+Copilot parity 達標後切換 `gal` 入口 `gal.ps1`→Rust binary。`gal doctor` read-only 聚合：canonical freshness、provider projection、ledger、plan lifecycle drift；exit-code 分級（warning 不擋 / error 擋 release）。
- **過渡期單一 entry（BUG-B）**：達 parity 前禁止舊 `gal.ps1` 與新 `gal` 交替使用（避免 Rust 與凍結 script 雙解析 config 漂移）。切換為原子動作：要嘛全走 script、要嘛全走 Rust。
- **Verify**: 隔離 home 內 `gal install`/`update` 對 Claude+Copilot == oracle；`gal doctor --dry-run` 不改檔，缺 projection/stale marker → error exit 指出修復面；切換後 scripts 不再被 entry 呼叫。

### R5：`gal commit-msg`（Rust，選配/最低優先）

- **Files**: `crates/gal-cli/src/`, git hook 設定
- **What**: Rust 化 commit-msg（port 已在 script 修好的邏輯）：由 changed files + diff 推導、rename 以 changed-path 證據；rewire git hook；退役 PS/Bash helper。**hijack 已在凍結 script 修掉，此 task 純為 runtime 統一，可延後或移出本計畫。**
- **Verify**: 內文含 `gemini`+`agy` 無 rename → 不 hijack、scope 非 `antigravity`；empty staging → 明確 no-op。

### P2：package-manager release lane（依 M1 parity、Security review）

- **P2-1 release artifact**（release 子命令、CI workflow、`docs/devguide.md`）：產出 GitHub Releases assets、archives、`checksums.txt`、`artifact-manifest.json`；cosign keyless（CI OIDC 簽 `checksums.txt` + Rekor）；本機無 OIDC → placeholder，不假造 `.sig`。
- **P2-2 winget**（`packaging/winget/`）：manifest 指向 release asset + sha256。
- **P2-3 Homebrew**（`packaging/homebrew/`）：先產 formula **template**，release 後手動同步 tap。
- **P2-4 Claude marketplace 狀態模型**（doctor + `docs/`, `README.md`）：拆 skills-dir / local cache / public marketplace 三狀態；stale cache 由 doctor 報告。

### P3：state / plan / headless closeout（純 docs/state）

- **P3-1 stale plans closeout**（`.dev/state.md`, `docs/plans/*.md`, `docs/observations/install-followups.md`）：completed/verified 移出 Active、移除重複/矛盾區段、本計畫置頂；非 active 觀察寫 follow-up；不刪 install 事實來源 source plan。
- **P3-2 plugin-bin 邊界重訂**（`docs/plans/plugin-bin-migration.md`, `.dev/state.md`）：該 plan 只做 upstream binary 的 plugin `bin/` exposure + Claude Code PATH。
- **P3-3 headless closeout**（`.dev/state.md`, `docs/plans/headless-cli-pipeline.*`, `scripts/executors/Invoke-Executor.ps1`, `scripts/common/New-TaskSpec.ps1`）：state 不再同時 DRAFT/VERIFIED；未驗證 receipt 列 follow-up；`Invoke-Executor` exit-0 未驗證寫回不得記 `completed`；`New-TaskSpec` 單一 `T-NNN` 不暴露整份 plan file surface。

<!--
PLAN CLOSEOUT CANDIDATES (OQ-004) — user 手動處理，本計畫不自動刪除。completed plan 留 docs/plans/ 原位，只從 .dev/state.md 移除。
可移出 Active（verified/complete）：feat-golem-dockeeper（等 dockeeper 可見驗證後）、fix-gal-pipeline-token-burn、feat-ai-plan-language-strategy、refactor-docs-restructure、feat-plugin-arch-migration（source 留 reference）。
保留勿刪：fix-install-ownership-stabilization（install 事實源）、headless-cli-pipeline（P3-3 reclassify）、plugin-bin-migration（P3-2）、feat-pdf-chandra-upgrade、本計畫。
孤兒待確認：feat-gal-file-memory-strategy、manage-external-plugins。
-->

### P4：release gate 收斂（最後）

- **Files**: `crates/gal-core/`, `crates/gal-cli/`
- **What**: `gal doctor --release-gate` 在 R4 doctor 骨架上併入 package-manager metadata + Claude marketplace classification，聚合為單一 gate。
- **Verify**: 發布前 `gal doctor --release-gate` exit 0；缺 Claude/Copilot projection、stale dockeeper、或缺 package-manager metadata → 非零 + 指出修復面。

## Files to Create or Modify

- `[CREATE]` `Cargo.toml`、`crates/gal-cli/`、`crates/gal-core/`（含 `config.rs`/`mode.rs`/`render.rs`/`providers/`）、`.gitignore`（已建）。
- `[CREATE]` `packaging/winget/`、`packaging/homebrew/`。
- `[CREATE]` `docs/observations/install-followups.md`。
- `[MODIFY]` `scripts/gal.ps1` — entry 切換到 Rust binary（R4）；其餘 `scripts/*` 僅加凍結標記，不改邏輯。
- `[MODIFY]` `README.md`, `docs/manual.md`, `docs/devguide.md` — install / package-manager / marketplace truth。
- `[MODIFY]` `docs/plans/plugin-bin-migration.md` — 邊界重訂。
- `[MODIFY]` `docs/plans/headless-cli-pipeline.md`、`.dev/state.md` — closeout。
- `[FROZEN]` `scripts/common/Common.ps1`/`common.sh`、`scripts/Build-CorePlugin.*`、`scripts/Build-ProviderPlugins.*`、`scripts/Update-Mcp.*`、`scripts/Install-GalPlugins.*`、`scripts/Get-StagedCommitMessage.*` — 作 oracle，**不再修改**。

## Success Criteria

- [ ] `gal` Rust binary 是正式 install/update/doctor 入口，可由 package-manager 安裝。
- [ ] winget、Homebrew、GitHub Releases 共享同一 versioned binary lineage。
- [ ] `gal update` 讓 repo source 變更傳播到 canonical root 與所有 selected provider read surfaces。
- [ ] `golem-dockeeper`/`doc-sync` 在 canonical root 與 Claude/Copilot 讀取面可見。
- [ ] AGY 三 surface 可被找到（best-effort；完整交易性為 M2）。
- [ ] Claude local cache / skills-dir projection / public marketplace 在 docs 與 doctor 誠實區分。
- [ ] `.dev/state.md` 不再把 terminal plan / orphaned prompt 當 active execution work。
- [ ] `plugin-bin-migration.md` 改為 downstream plugin `bin/` exposure plan，只依賴本計畫的 binary source contract。
- [ ] headless pipeline 不再以 OFFLOAD/echo smoke 當 VERIFIED；真實 receipt 缺口被修或移入 follow-up。

## Risks

- **Rust 重寫遺失 provider quirks（已接受）**：緩解 — 凍結 scripts 過渡期仍服務 Claude/Copilot；既有正確面以 oracle parity 驗；AGY 容許暫不完美、最後做。
- **oracle 只對已正確面有效（BUG-A）**：dockeeper/AGY 等 bug-fix delta 不能 match 壞 oracle，須意圖斷言；Test Plan 已區分。
- **過渡期雙解析 drift（BUG-B）**：達 parity 前單一 entry 政策。
- **跨平台覆蓋變 load-bearing（BUG-C）**：Windows 先行；macOS/Linux Rust 測試列 M2，不可默默漏。
- **MCP 量體（BUG-D）**：`/refining-plan` 須拆 MCP 子任務。
- **可發布 binary 延後（MISS-A）**：以 M1（Claude/Copilot parity）為最小可發布里程碑，P2 不必等 AGY。
- **package-manager 外部審核延遲**：GitHub Releases 作 canonical fallback，doctor/docs 標 downstream lag。
- **plan closeout 誤刪歷史**：只移出 active state，不刪 source plan，follow-up 另檔。
- **headless VERIFIED overclaim 重演**：closeout 要求 live receipt 或明確降級；doctor/state 不接受文字宣稱。

## Open Questions

全部已決議：

- [x] **OQ-001** Release signing = **sigstore/cosign keyless**（CI OIDC，無長期私鑰）。Security review 只確認 CI 信任設定（workflow identity + Rekor）。
- [x] **OQ-002** 本 repo 先產 Homebrew formula **template**，release 後手動同步。
- [x] **OQ-003** 模式權威 = `devMode` + `galRoot` usable；`installMode` 棄用（定義見 R-003）。
- [x] **OQ-004** completed plan 留 `docs/plans/` 原位，只從 `.dev/state.md` 移除（清單見 P3-1 註解）。
- [x] **OQ-005** 只建 release gate，public Claude marketplace submission 另開計畫。

## Approval

- Human approval: [approved at 2026-06-03]
- Architect review: REVISE（Rust-first）→ 見 ## Review Results。blocking 已折入本修訂（oracle 範圍、單一 entry、AGY best-effort、milestones、MCP 量體警示）；建議 `/refining-plan` 落地後重跑確認。
- Security review: [required before P2 — install/update/uninstall、release signing CI 信任設定、provider projections、path cleanup 觸及信任邊界]
- Release review: [required before package-manager publication]

## Review Results

### Architecture Review

**Verdict: REVISE（方向 APPROVE）。** Rust-first 是使用者明示接受的合理 trade-off（消除三面對等稅、單一型別 runtime）。以下 blocking 已折入本修訂；其餘交 `/refining-plan` 落地。

| 決策 | 效益 | 成本 | 裁決 |
| --- | --- | --- | --- |
| Rust-first 直接取代 | 消除對等稅、單一 runtime、render 更可靠 | 重寫全引擎；可發布 binary 延後 | APPROVE w/ M1（MISS-A） |
| 凍結 script 作 oracle | 對齊基準、降盲寫風險 | oracle 只對已正確面有效 | REVISE→已折入（BUG-A） |
| AGY 低優先 + 完整交易 | 匹配用途 | 對容忍壞掉的 surface 過度工程 | REVISE→best-effort（OE-A） |
| 取消 R-013 PS/Bash 對等 | 省對等稅 | Rust 須 day-1 跨平台 | REVISE→M2 scoped（BUG-C） |

**Findings 與處置：**

- **BUG-A（高，oracle-vs-bugfix）** — 凍結 script 是 bug 來源，對 dockeeper/AGY 是壞 oracle；byte-parity 會複製 bug。**已折入**：Scope Decision「Oracle 範圍」、R2「Oracle 範圍」、Test Plan 區分 parity vs 意圖斷言。
- **BUG-B（中，過渡期雙解析）** — 達 parity 前 Rust 與凍結 script 並行解析 config 會 drift。**已折入**：R4「過渡期單一 entry」。
- **BUG-C（中，跨平台）** — 取消對等後 Rust 須跨平台，macOS/Linux 測試無 owner。**已折入**：R-013 改述 + M2 scoped + T-021；`/refining-plan` 須補 macOS/Linux TP 細節。
- **BUG-D（中，MCP 量體）** — `Update-Mcp.*` ~78KB，T-008 嚴重低估。**已折入**：R3 量體警示；`/refining-plan` 須拆子任務。
- **OE-A（AGY 過度工程）** — 低優先卻要完整交易/ledger 矛盾。**已折入**：R-006/R3 改 best-effort，交易延後 M2。
- **OE-B（R5 redundant）** — commit-msg 已修，Rust 化純 runtime 統一。**已折入**：R5/T-012 標選配、可移出。
- **MISS-A（binary 里程碑）** — 全引擎 parity 才能發布太遠。**已折入**：## Milestones M1=Claude/Copilot parity 為最小可發布。

**What's Good**：消除三面對等稅是真實簡化；oracle parity 對已正確面是強 de-risk；Claude/Copilot 先行、AGY 降級的排序合理；T-002/T-003 不 revert 作對照基準正確。

**Required before CLEAR（交 `/refining-plan`）**：(1) Test Plan 明確標哪些 TP 是 oracle-parity、哪些是意圖斷言（已初步區分，須複核）；(2) 補 macOS/Linux 跨平台 TP 細節（T-021）；(3) 拆 MCP 子任務（T-008）。

### Engineering Review

**Verdict: CLEAR**（Rust-first 契約）。

22 個 T-NNN（T-001..T-003 + commit-msg 已完成；T-004..T-022 待辦）1:1 對應 Approach phase，每個獨立可完成可驗證。30 條 TP 對齊並落地架構稽核的三項 required：

- **oracle vs intent 分離（BUG-A）**：TP 明確標 `parity`（對凍結 Claude/Copilot oracle byte 比對：TP-005/006/009/012/014/029/030）與 `intent`（驗意圖、不對 bug 來源比對：TP-007 dockeeper、TP-013 AGY）。TP-006（render parity, Claude/Copilot 既有結構）與 TP-007（dockeeper intent）不再衝突。
- **MCP 拆解（BUG-D）**：78KB `Update-Mcp.*` 拆成 T-008（core + safe-merge：保留 user entry、idempotent、secret guard、不寫半殘 root）與 T-009（per-provider serializers：M1 先 Claude+Copilot parity，AGY/Codex/OpenCode 列 M2）。TP-010/011/012 對應。
- **跨平台（BUG-C）**：T-022 + TP-029/030 明列 macOS、Linux 各對凍結 Bash oracle parity（M2）。

依賴與里程碑清楚：M1（Claude/Copilot native parity = R1–R4）為最小可發布 binary；P2 以 M1 為前置、不必等 AGY；AGY/MCP-全 serializer/跨平台列 M2。BUG-B（過渡期單一 entry）入 T-011；OE-A（AGY best-effort）入 T-010；OE-B（R5 選配）入 T-013。

可立即開工 T-004（gal-core config model），不被任何 blocking 阻擋。架構稽核的 REVISE 為「方向 APPROVE + 折入式修正」，其 required 已於本次落地；建議實作中若觸及 MCP serializer 或 AGY 交易性再回看 R3 量體警示。

<!-- ENG_REVIEW: CLEAR -->

### Security Review

Pending（P2 前必做）。

### Release Review

Pending（package-manager publication 前必做）。

## Test Plan

Rust 行為以 `cargo test` 驗證；runtime-surface 在隔離 home 內驗證。**Oracle 範圍（BUG-A）**：標 `parity` 者對凍結 Claude/Copilot oracle byte 比對；標 `intent` 者驗意圖行為，**不**對凍結 script 比對（因 script 為 bug 來源）。

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | unit | `cargo test` 通過 | T-001 |
| TP-002 | manual | `gal --version`→version exit 0；`--help` 列子命令；未接線回 `not wired` 非零 | T-001 |
| TP-003 | unit | gal-core 解析真實 `config.json` 成 model；缺檔/缺 key → 明確預設、不 panic | T-004 |
| TP-004 | unit | Rust mode 矩陣：usable→dev、false/缺→normal、unusable→error 不 fallback、dead-path 短逾時 | T-005 |
| TP-005 | parity | 同一 config，Rust mode == 凍結 PS `Test-InstallModeAuthority` oracle | T-005 |
| TP-006 | parity | Rust render 對 **Claude/Copilot 既有正確結構** == 凍結 `build-core-plugin` oracle | T-006 |
| TP-007 | intent | render 後 canonical root `agents/` 含 `golem-dockeeper`、`skills/` 含 `doc-sync`（意圖斷言，**不**對 oracle 比對） | T-006 |
| TP-008 | integration | kill-mid-render 後重跑收斂、無半渲染/temp 殘留 | T-006 |
| TP-009 | parity | Rust Claude skills-dir + Copilot projection == 現行 script projection oracle（逐檔） | T-007 |
| TP-010 | unit | MCP core：變數/placeholder 解析正確；未解析 secret 不寫入 | T-008 |
| TP-011 | integration | MCP safe-merge：保留既有 user entry、只覆寫 GAL-managed entry、重跑 idempotent；plugin root 不完整時不寫半殘 `mcp_config.json` | T-008 |
| TP-012 | parity | MCP per-provider serializer：Claude Desktop + Copilot CLI 輸出 == 凍結 oracle（逐格式；AGY/Codex/OpenCode 列 M2） | T-009 |
| TP-013 | intent | Rust AGY 三 surface 存在含必要檔（best-effort；交易/ledger 屬 M2，不在此驗） | T-010 |
| TP-014 | parity | 隔離 home：`gal install`/`update` 對 Claude+Copilot == oracle；改 source marker 重跑 → 傳播到 canonical + provider surfaces | T-011 |
| TP-015 | integration | 入口切換後 `gal` 走 Rust binary；凍結 scripts 不再被 entry 呼叫；過渡期單一 entry | T-011 |
| TP-016 | manual | `gal doctor --dry-run` 不改檔 | T-012 |
| TP-017 | integration | 移除 Claude/Copilot projection 或 stale dockeeper marker → doctor error exit 指出修復面 | T-012 |
| TP-018 | unit | doctor exit-code 分級：warning-only→0；error→非零 | T-012 |
| TP-019 | unit | `gal commit-msg`：含 `gemini`+`agy` 無 rename → 不 hijack；empty staging → no-op | T-013 |
| TP-020 | integration | release artifact dry-run → assets/archives/`checksums.txt`/`artifact-manifest.json` 符合 release matrix | T-014 |
| TP-021 | manual | CI `cosign verify-blob` 對 `checksums.txt` 通過；本機無 OIDC → placeholder 不假造 `.sig` | T-014 |
| TP-022 | unit | winget manifest dry-run → `PackageIdentifier=Monkey1Wizard.GAL`，URL+sha256 指向 release artifact | T-015 |
| TP-023 | unit | Homebrew formula template dry-run → `brew install monkey1wizard/tap/gal` 用 release archive 並裝 `bin/gal` | T-016 |
| TP-024 | integration | Claude local cache stale → doctor 報 stale、不宣稱 public installed；README/manual 不混淆 | T-017 |
| TP-025 | manual | `.dev/state.md` 第一列為本計畫、無重複區段；`/gal status` 不指向不存在的 prompt | T-018 |
| TP-026 | manual | `plugin-bin-migration.md` 不再與本計畫重疊；下一步只依賴本計畫 binary contract | T-019 |
| TP-027 | manual | `.dev/state.md` 不再同時把 headless plan 表示為 DRAFT 與 VERIFIED；未驗證 receipt 列 follow-up | T-020 |
| TP-028 | integration | `gal doctor --release-gate` exit 0；缺 Claude/Copilot projection / stale dockeeper / 缺 pkg metadata → 非零 + 指出修復面 | T-021 |
| TP-029 | parity | **macOS** 上 Rust install/render/Claude+Copilot projection == 凍結 Bash oracle（M2） | T-022 |
| TP-030 | parity | **Linux** 上 Rust install/render/Claude+Copilot projection == 凍結 Bash oracle（M2） | T-022 |

## Tasks

**已完成（凍結 script / oracle，不 revert）**

- [x] T-001 — Rust workspace skeleton：`--version`/`--help`/子命令 enum/exit-code/`not wired`；不 parse config。
- [x] T-002 — mode 解析（script，oracle）：devMode+galRoot usable + no-fallback + dead-path 逾時，棄用 installMode。
- [x] T-003 — canonical render（script，oracle）：一般/dev 都重渲染、atomic temp+swap。
- [x] commit-msg — keyword-hijack 已移除（凍結 script，`f64f517`）。Rust 化 = R5/T-012 選配。

**R1 — gal-core config + mode（依 T-001）**

- [x] T-004 — gal-core config model（serde 讀 `config.json`，Rust 擁有 config 真相，缺檔/key 明確預設）。 *(b03e904)*
- [x] T-005 — gal-core mode resolution（port T-002 predicate + no-fallback + dead-path；== 凍結 PS oracle）。 *(1172295)*

**R2 — Rust canonical render（依 R1、T-003 oracle）**

- [x] T-006 — Rust render atomic temp+swap；目錄掃描 agents/skills；Claude/Copilot 結構 == oracle（parity），dockeeper/doc-sync 以意圖斷言；kill-mid-render 收斂。 *(a6891e0)*

**R3 — Rust provider projection（依 R2）**

- [ ] T-007 — Rust Claude skills-dir + Copilot installed-plugin projection，逐檔 == oracle。 *(8f536a4)*
- [ ] T-008 — Rust MCP **core + safe-merge**（provider-agnostic）：manifest model、變數/placeholder 解析、managed-vs-user 合併（只動 GAL-managed entry、保留 user entry、idempotent）、未解析 secret 不寫入、plugin root 未完整前不寫半殘 `mcp_config.json`。（拆自 ~78KB `Update-Mcp.*`，BUG-D。）
- [ ] T-009 — Rust MCP **per-provider serializers**：各 provider 不同 config 格式（Claude Desktop / Copilot CLI / AGY / Codex / OpenCode；含 TOML）。**M1 先做 Claude + Copilot 並逐檔 == oracle；AGY/Codex/OpenCode 列 M2。** xmachine binding（legacy skills store）不在此 scope。
- [ ] T-010 — Rust AGY 三 surface **best-effort**（低優先；交易/ledger 延後 M2）。

**R4 — Rust 入口接管 + doctor + 退休 scripts（= M1，依 R3 Claude/Copilot）**

- [ ] T-011 — `gal install`/`update`/`uninstall` 接 Rust 原生（config→render→projection→ledger）；Claude+Copilot parity 後切 entry；過渡期單一 entry（BUG-B）。
- [ ] T-012 — `gal doctor` read-only（Rust）：canonical freshness、provider projection、ledger、plan lifecycle drift；exit-code 分級。

**R5 — commit-msg（Rust，選配/最低優先）**

- [ ] T-013 — Rust 化 commit-msg（port 已修邏輯）；rewire git hook；退役 PS/Bash helper。**可延後或移出本計畫。**

**P2 — package-manager release lane（依 M1 parity、Security review）**

- [ ] T-014 — release artifact（Rust/CI）+ cosign keyless。
- [ ] T-015 — winget manifest。
- [ ] T-016 — Homebrew formula template。
- [ ] T-017 — Claude marketplace 三狀態模型（doctor + docs）。

**P3 — state / plan / headless closeout（純 docs/state，隨時）**

- [ ] T-018 — `.dev/state.md` closeout + follow-up log。
- [ ] T-019 — `plugin-bin-migration.md` 邊界重訂。
- [ ] T-020 — headless closeout（state reclassify + `Invoke-Executor`/`New-TaskSpec` 收斂）。

**P4 — release gate（最後，依 T-012 + P2）**

- [ ] T-021 — `gal doctor --release-gate` 聚合 pkg-manager metadata + marketplace classification。

**跨平台（BUG-C，M2）**

- [ ] T-022 — macOS/Linux 上 Rust install/render/projection 對應 oracle 驗證（oracle = 凍結 Bash scripts；R-013）。
