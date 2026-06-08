# Plan: GAL Rust 原生完整且正確安裝（feat-gal-rust-native-install）

> 本計畫整併並取代舊的 `plugin-bin-migration.md`（plugin `bin/` exposure），並吸收本 session 發現的「install 收斂缺口」（dockeeper 不可用之根因）。先前刪除的 `feat-plugin-arch-migration`（renderer/projection 統一）、`fix-install-ownership-stabilization`（install ownership/收斂）、`feat-golem-dockeeper`（agent 已 ship 但未進活讀取面）三個計畫的範圍與本計畫高度重疊，其知識已入 docs/Rust，但「在真實機器上正確安裝」這件事從未閉合 —— 本計畫負責閉合它。

## Goal

以 Rust 為**單一實作**，端對端**正確且完整**地安裝 GAL：source 的所有 agents（含 `golem-dockeeper`）、skills（含 `doc-sync`）、commands、conventions、workflows、templates 與原生 `gal` binary，必須真正收斂進**每個 provider 實際載入的讀取面**（特別是 Claude Code 真正載入的 plugin 快取），並可由 plugin `bin/` 與 package manager 取得 `gal`。安裝後，`gal doctor` 能證明活讀取面與 source 一致，且 Claude Code 能實際調用 `golem-dockeeper` 與 `doc-sync`。

## Governing Principle

**安裝成功 = 活讀取面與 source 一致，而非「render 出正確 artifact」。** render 正確但沒收斂進 provider 實際載入的面，等於沒安裝。本計畫以「活讀取面對齊」為唯一驗收基準，不接受「temp dir 內容正確」作為完成證據。

## Context（為何整併重寫）

- **上游引擎已存在**：`fix-gal-bootstrap-install-convergence.md` 已建 `crates/gal-engine`（`config`/`mode`/`render`/`install`/`providers/{claude,copilot,agy}`/`mcp`/`ledger`/`doctor`，T-001~T-021 完成）。本計畫**消費**該引擎，不重建 workspace；負責「真實機器端對端安裝 + 活讀取面收斂 + bin 暴露 + 驗證 + 清理 + 跨平台」。
- **舊 plugin-bin-migration 的範圍太窄**：它只做「把 upstream binary 放進 plugin `bin/`」，假設 install 收斂已由別處完成。實機證據顯示 install 收斂**根本沒完成**，所以 bin exposure 單獨做沒有意義 —— 兩者必須合一。

## 證實的根因（2026-06-08 實機診斷）

| # | 事實 | 證據 |
| --- | --- | --- |
| RC-1 | `gal` binary 不在 PATH | `command -v gal` 失敗；binary 僅在 `target/debug/gal.exe` |
| RC-2 | canonical root `~/.gal/plugins/gal` **不存在** | 路徑不存在 |
| RC-3 | Claude Code 實際從 **`~/.claude/plugins/cache/gal/gal/1.0.0/`** 載入（實體 copy） | skill base dir 證實；該面 **12 agents、無 `golem-dockeeper`、無 `doc-sync` skill**；plugin.json `version=1.0.0` |
| RC-4 | 含 dockeeper 的正確 render（13 agents + doc-sync，Claude 格式正確）**只在孤兒暫存** | `~/.gal/plugins/.gal-render-*/`（共 **9 個**孤兒目錄）；Claude Code 不載入這些 |
| RC-5 | 推定核心 bug | Rust provider projection 的**目標面**與 Claude Code **實際載入面**（marketplace plugin 快取）不一致；且 atomic render 的 temp→swap→cleanup 未閉合（留 9 個孤兒） |

一句話：**已發布的 plugin 是早於 dockeeper 的 v1.0.0 舊版,Rust 引擎雖能 render 正確內容,卻從未把它收斂進 Claude Code 真正載入的面,且 binary 未安裝。**

## Requirements

- [ ] **R-01 單一 Rust 入口可端對端安裝**：release build 的 `gal` binary 經 `gal install` / `gal update` 完成完整安裝，無需 `bash`/`pwsh` 前綴；安裝流程全在 Rust（消費 bootstrap 引擎）。
- [ ] **R-02 完整 source 收斂（目錄掃描，非白名單）**：所有 source agents/skills/commands/conventions/workflows/templates 以目錄列舉渲染進 canonical root；新增 agent/skill（如 `golem-dockeeper`/`doc-sync`）自動納入，無硬編碼清單可漏。
- [ ] **R-03 活讀取面對齊（核心修復）**：明確界定並鎖定每個 provider「Claude Code/Copilot/AGY 實際載入的面」契約；install/update 必須收斂到該面。Claude Code 面 = marketplace plugin 快取（`~/.claude/plugins/cache/gal/gal/<version>/`），含 agents/、skills/、commands/、`.claude-plugin/plugin.json`；version 變更須能觸發 Claude Code 重載。
- [ ] **R-04 plugin `bin/` 暴露（吸收 plugin-bin-migration 全部範圍）**：published plugin root 的 `bin/` 含本機 OS 原生 `gal`(`.exe`)，Claude Code 啟用外掛後 Bash tool 可裸呼 `gal`；**不**放 `bin/gal.sh`/`bin/gal.ps1` shell wrapper；binary 缺失時 render fail-loud、不產半成品。
- [ ] **R-05 atomic + 清理**：render-to-temp-then-swap 必須在 swap 後刪除自身 temp；install/`gal doctor` 偵測並清理孤兒 `~/.gal/plugins/.gal-render-*`。
- [ ] **R-06 doctor 驗證活讀取面**：`gal doctor` 比對活讀取面與 source —— agent/skill 數一致、`golem-dockeeper`+`doc-sync` 存在、`bin/gal`(`.exe`) 存在且可執行、version 一致、無孤兒 temp；缺口 → 非零 exit 指出修復面。
- [ ] **R-07 package-manager 安裝路徑**：消費 bootstrap P2 的 winget/Homebrew/GitHub Releases artifacts；package-manager 裝出的 `gal` 能完成同一套 install 收斂。
- [ ] **R-08 跨平台正確安裝**：Windows + macOS + Linux 的 PATH/symlink/junction/權限（Unix `+x`）正確；活讀取面對齊在三平台皆驗。
- [ ] **R-09 端對端驗收**：乾淨環境安裝後，Claude Code 可實際調用 `golem-dockeeper` 與 `doc-sync`（subagent/skill 出現在 registry）。

## Approach（Phases，逐 phase 可獨立驗證）

凍結 scripts 不 revert（作 oracle）。Rust 行為以 `cargo test` 驗證；安裝面在隔離 home 驗證。

| Phase | 目標 | 前置 |
| --- | --- | --- |
| **P0 載入面契約** | 鎖定每 provider「實際載入面」官方契約（Claude Code plugin 載入順序、cache vs skills-dir、version 解析、`${CLAUDE_PLUGIN_ROOT}`） | — |
| **P1 收斂到活面** | 修 `providers/claude` 投影目標為實際載入面；canonical root 建出；version bump 觸發重載 | bootstrap 引擎、P0 |
| **P2 bin 暴露** | render `bin/gal`(`.exe`)；fail-loud；PATH smoke | P1 |
| **P3 atomic + 孤兒清理** | swap 後刪 temp；偵測清理 `.gal-render-*` | P1 |
| **P4 doctor 活面驗證 + gate** | doctor 比對活面 vs source；release gate 併入 | P1–P3 |
| **P5 跨平台 + pkg-manager** | macOS/Linux 安裝面對齊；winget/Homebrew/Releases 裝出的 binary 完成收斂 | P1–P4、bootstrap P2 |
| **P6 端對端驗收** | 乾淨環境 dockeeper/doc-sync 可調用 | P1–P5 |

### P0：載入面契約（Loading-Surface Contract）

- **Files**: `docs/devguide.md`、本計畫
- **What**: 以官方文件 + 實機探測，鎖定 Claude Code 載入 plugin agents/skills/commands 的**確切來源**（`~/.claude/plugins/cache/<marketplace>/<plugin>/<version>/` vs `~/.claude/skills/gal/` skills-dir projection），含 version 解析（`plugin.json:version` 或 git SHA）、更新觸發機制、`${CLAUDE_PLUGIN_ROOT}` 語義。對 Copilot、AGY 同樣鎖定實際載入面。
- **Verify**: devguide 有「每 provider 實際載入面」對照表；與 RC-3 實機觀察一致。

### P1：Rust install 收斂到活讀取面（核心修復）

- **Files**: `crates/gal-engine/src/providers/claude.rs`（及 copilot/agy 視 P0 結果）、`install.rs`、`render.rs`
- **What**: 修正 provider projection 目標，使 `gal install`/`update` 把完整 canonical root 收斂進 **P0 鎖定的實際載入面**（Claude Code marketplace plugin 快取）；canonical root（`~/.gal/plugins/gal`）正確建出並作為單一來源；version 變更時更新 plugin.json version 以觸發 Claude Code 重載。dockeeper/doc-sync 以目錄掃描自動納入（R-02）。
- **Verify**: 隔離 home `gal install` 後，活載入面 agents/ 含 13 個（含 `golem-dockeeper`）、skills/ 含 `doc-sync`；canonical root 存在且 == 活面。

### P2：plugin `bin/` 暴露（合併 plugin-bin-migration）

- **Files**: `crates/gal-engine/src/render.rs`/`install.rs`（bin 複製）、`scripts/Build-CorePlugin.*`（凍結期過渡）
- **What**: render published plugin root 時建立 `bin/`，複製本機 OS 原生 `gal`（Unix 無副檔名 +x、Windows `gal.exe`）；缺 binary 時 fail-loud；不放 shell wrapper。binary source = bootstrap 產出（source build 或 package-manager 安裝位）。
- **Verify**: 啟用 plugin 後 Bash tool 裸呼 `gal --version` 解析到 plugin `bin/`；Windows 有 `bin/gal.exe`、Unix 有可執行 `bin/gal`，無 `.sh`/`.ps1`。

### P3：atomic swap + 孤兒清理

- **Files**: `crates/gal-engine/src/render.rs`、`install.rs`、`doctor.rs`
- **What**: render-to-temp-then-swap 在成功 swap 後刪除自身 temp；中斷殘留與既有 9 個孤兒 `.gal-render-*` 由 install/doctor 以白名單（僅 GAL-owned `.gal-render-*` 前綴）偵測並清理。
- **Verify**: 重跑多次 install 後 `~/.gal/plugins/` 無 `.gal-render-*` 殘留；kill-mid-render 重跑收斂無半渲染。

### P4：doctor 驗證活讀取面 + release gate

- **Files**: `crates/gal-engine/src/doctor.rs`、`crates/gal-cli/src/`
- **What**: `gal doctor` 新增「活讀取面 vs source」檢查：agent/skill 數、`golem-dockeeper`+`doc-sync` 存在、`bin/gal`(`.exe`) 可執行、version 一致、無孤兒 temp。`gal doctor --release-gate` 併入。
- **Verify**: 缺 dockeeper / 缺 bin / 有孤兒 → doctor 非零並指出修復面；完整安裝 → exit 0。

### P5：跨平台 + package-manager 安裝路徑

- **Files**: `crates/gal-engine/src/`（路徑/symlink/junction/權限）、`packaging/winget/`、`packaging/homebrew/`
- **What**: 三平台安裝面對齊（Windows junction、Unix symlink + `+x`）；winget/Homebrew/Releases 裝出的 `gal` 能完成同一收斂。消費 bootstrap P2 artifacts，不重做 release lane。
- **Verify**: macOS/Linux 隔離 home install → 活面對齊（== 凍結 Bash oracle 對既有正確面；dockeeper 以意圖斷言）；pkg-manager 裝出的 binary `gal install` 後 dockeeper 可見。

### P6：端對端驗收

- **Files**: 測試/驗收腳本、`docs/devguide.md`
- **What**: 乾淨環境（無既有快取）完整安裝後，於 Claude Code 確認 `golem-dockeeper` 出現在 subagent registry、`doc-sync` skill 可用。
- **Verify**: 手動 smoke：新 Claude Code session 能調用 dockeeper；doctor green。

## Files to Create or Modify

- `[MODIFY]` `crates/gal-engine/src/providers/claude.rs`、`install.rs`、`render.rs`、`doctor.rs` — 收斂目標改為實際載入面、bin 複製、atomic cleanup、活面驗證。
- `[MODIFY]` `crates/gal-cli/src/main.rs` — doctor 活面檢查/exit 分級接線（如需）。
- `[MODIFY]` `scripts/Build-CorePlugin.*`（凍結期過渡的 bin render，達 parity 後退休）。
- `[MODIFY]` `docs/devguide.md`、`docs/manual.md`、`README.md` — 實際載入面契約、bin exposure、正確安裝流程。
- `[MODIFY]` `packaging/winget/`、`packaging/homebrew/` — 安裝後收斂驗證（消費 bootstrap artifacts）。
- `[DELETE]` `docs/plans/plugin-bin-migration.md` — 範圍併入本計畫。
- `[MODIFY]` `docs/plans/fix-gal-bootstrap-install-convergence.md` — 交叉引用改指本計畫（取代 plugin-bin-migration 的 P3-2 角色）。

## Test Cases

- [ ] TP-01（P0）— devguide 載入面對照表與實機 RC-3 一致；Claude Code 載入來源被明確記載。
- [ ] TP-02（P1, intent）— 隔離 home `gal install` 後活載入面 agents/ 含 `golem-dockeeper`、skills/ 含 `doc-sync`；canonical root == 活面。
- [ ] TP-03（P1, parity）— 既有正確 agents/skills 在活面 == 凍結 Claude oracle（逐檔）。
- [ ] TP-04（P2）— Windows render 出 `bin/gal.exe`、Unix 出可執行 `bin/gal`，無 `.sh`/`.ps1`；缺 binary → fail-loud。
- [ ] TP-05（P2）— plugin 啟用後 Bash tool 裸呼 `gal --version` 解析 plugin `bin/`。
- [ ] TP-06（P3）— 多次 install 後無 `.gal-render-*` 孤兒；既有 9 個孤兒被清理；kill-mid-render 收斂。
- [ ] TP-07（P4）— doctor 對「缺 dockeeper / 缺 bin / 有孤兒 / version 不符」各報非零並指出修復面；完整安裝 exit 0。
- [ ] TP-08（P5, parity）— macOS、Linux 安裝面對齊凍結 Bash oracle；pkg-manager 裝出的 binary 完成收斂。
- [ ] TP-09（P6）— 乾淨環境安裝後 Claude Code 可調用 `golem-dockeeper` + `doc-sync`（手動 smoke）。
- [ ] TP-10 — 文件 grep：本計畫合併後，repo 不再有「plugin-bin-migration 與 install 收斂分屬兩計畫」的矛盾敘述。

## Success Criteria

- [ ] 乾淨環境以原生 `gal install`（或 package-manager 裝出的 `gal`）安裝後，Claude Code **實際載入面**含全部 source agents（含 `golem-dockeeper`）與 skills（含 `doc-sync`）。
- [ ] Claude Code 啟用 GAL 外掛後可裸呼 `gal`，plugin `bin/` 只含本機 OS 原生執行檔。
- [ ] `~/.gal/plugins/` 無孤兒 `.gal-render-*`；canonical root 存在且與活面一致。
- [ ] `gal doctor` 能證明活讀取面 == source（含 dockeeper/doc-sync/bin/version），缺口時 fail-loud。
- [ ] Windows/macOS/Linux 三平台安裝面皆對齊。
- [ ] 安裝/收斂/bin 全程 Rust 原生；凍結 scripts 僅過渡期 oracle，達 parity 後退休。

## Risks

- **載入面契約判斷錯誤（高）**：若 P0 把 Claude Code 實際載入面判錯，整個收斂打到空處。Mitigation：P0 以實機探測（本 session RC-3 已證 marketplace cache 為載入面）+ 官方文件雙重確認，並以 TP-09 端對端驗收兜底。
- **marketplace 快取由 Claude Code 管理（中高）**：直接寫入 `~/.claude/plugins/cache/...` 可能被 Claude Code 的 plugin 更新機制覆蓋或忽略。Mitigation：P0 鎖定正確的「更新觸發」路徑（version bump / 官方 update 流程），不做未受支援的手動竄改作為終態。
- **與 bootstrap 引擎範圍重疊（中）**：本計畫修 `providers/claude.rs` 等 bootstrap 擁有的檔案。Mitigation：本計畫只改「收斂目標 + bin + cleanup + 活面驗證」delta；引擎核心契約變更回 bootstrap。交叉引用須同步更新。
- **跨平台安裝差異（中）**：junction/symlink/權限三平台不同。Mitigation：P5 各平台隔離 home 測試,Windows 先行。
- **孤兒清理誤刪（中）**：白名單僅限 GAL-owned `.gal-render-*` 前綴，doctor-first、不 delete-through。
- **過時 v1.0.0 快取殘留**：升級後舊版快取可能與新版並存。Mitigation：doctor 偵測 stale version 並報告。

## Open Questions

- [ ] OQ-01（P0 阻斷）— Claude Code 對 plugin 更新的**正式觸發契約**為何？version bump 後 Claude Code 何時/如何重新填充 `~/.claude/plugins/cache/.../<version>/`？是否需經官方 marketplace/`/plugin` 流程，或可由 `gal` 直接寫快取 + bump version？此題須在 `/refining-plan` 前鎖定。
- [ ] OQ-02 — Claude Code 真正的 agent 載入面是 marketplace plugin 快取、skills-dir projection（`~/.claude/skills/gal`）、或兩者並存？（RC-3 指向前者；P0 須最終確認並記錄優先序。）
- [ ] OQ-03 — 本機既有 9 個孤兒 `.gal-render-*` 與缺失 canonical root：是先手動清理重裝，或由新 `gal install` 一次性收斂 + 清理？

## Approval

- Human approval: [pending]
- Architect review: [required — `/deep-planning`]。本計畫觸及受保護路徑（`scripts/`、`crates/gal-engine` 核心、install 拓撲）且與 bootstrap 引擎範圍交界，須 architect 審 trade-off、收斂目標正確性、與 bootstrap 的邊界切割，方可進 `/plan-to-prompt`。
- Additional domain review: [not triggered]（無 customer-facing / business-rule 內容）

## Review Results

### Architecture Review

Pending（`/deep-planning`）。重點待審：(1) P0 載入面契約是否正確（marketplace cache vs skills-dir）；(2) 與 bootstrap 的檔案/契約邊界（避免雙寫 `providers/claude.rs`）；(3) 直接寫 Claude-managed 快取的合法性與更新觸發（OQ-01）。
