# Plan: GAL Rust 原生完整且正確安裝（feat-gal-rust-native-install）

> 本計畫整併並取代舊的 `plugin-bin-migration.md`（plugin `bin/` exposure），並吸收本 session 發現的「install 收斂缺口」（dockeeper 不可用之根因）。先前刪除的 `feat-plugin-arch-migration`（renderer/projection 統一）、`fix-install-ownership-stabilization`（install ownership/收斂）、`feat-golem-dockeeper`（agent 已 ship 但未進活讀取面）三個計畫範圍與本計畫高度重疊，知識已入 docs/Rust，但「在真實機器上正確安裝」從未閉合 —— 本計畫負責閉合它。

## Goal

以 Rust 為**單一實作**，端對端**正確且完整**地把 GAL source（所有 agents 含 `golem-dockeeper`、skills 含 `doc-sync`、commands、conventions、workflows、templates）渲染進 canonical root，並收斂進**本計畫範圍內、有 GAL 自有非官方路徑可走的活讀取面**，加上原生 `gal` binary 可由 plugin `bin/` 與 package manager 取得。安裝後 `gal doctor` 能證明活讀取面與 source 一致。

Claude 活載入面（本計畫範圍）= **GAL 自有直接投影的 skill 面** `~/.claude/skills/gal`（symlink 指向 canonical root；凍結 oracle 的持久投影面，`common.sh:75`），讓 `doc-sync` 等 **skill** 被 Claude 載入。**不**使用 Claude 官方 marketplace、`/plugin` install、version-bump 重載；亦**不**碰被 oracle 視為 legacy、每次刷新即刪的 `~/.claude/plugins/gal`（`common.sh:76`）。

> **範圍切割（2026-06-08 使用者裁決）**：Claude **subagent**（如 `golem-dockeeper`）的活載入由 Claude plugin 機制（版本化 cache，即官方 `claude plugin install` 流程）提供，**無已知 GAL 自有非官方面**。本計畫**只**負責把 agents 正確渲染進 canonical root（R-02，已驗），**不**處理 subagent 的 Claude 活載入 —— 該面延至後續計畫（屆時再決定是否採官方流程）。本計畫不放棄 dockeeper，只是不在此處理其 Claude 載入。

## Governing Principle

**安裝成功 = 活讀取面與 source 一致，而非「render 出正確 artifact」。** render 正確但沒收斂進 provider 實際載入的面，等於沒安裝。本計畫以「活讀取面對齊」為唯一驗收基準，不接受「temp dir 內容正確」作為完成證據。

## Context（為何整併重寫）

- **上游引擎已存在**：前置計畫 fix-gal-bootstrap-install-convergence（2026-06-08 全 22 任務完成並關閉刪除；殘留追蹤見 `docs/observations/install-followups.md`）已建 `crates/gal-engine`（`config`/`mode`/`render`/`install`/`providers/{claude,copilot,agy}`/`mcp`/`ledger`/`doctor`）。本計畫**消費**該引擎，不重建 workspace；負責「真實機器端對端安裝 + 活讀取面收斂 + bin 暴露 + 驗證 + 清理 + 跨平台」。
- **舊 plugin-bin-migration 範圍太窄**：它只做「把 upstream binary 放進 plugin `bin/`」，假設 install 收斂已由別處完成。實機證據顯示 install 收斂**根本沒完成**，故 bin exposure 單獨做沒意義 —— 兩者必須合一。

## 證實的根因（2026-06-08 實機診斷）

| # | 事實 | 證據 |
| --- | --- | --- |
| RC-1 | `gal` binary 不在 PATH | `command -v gal` 失敗；binary 僅在 `target/{debug,release}/gal.exe` |
| RC-2 | canonical root `~/.gal/plugins/gal` **不存在**（已閉合，見現況） | 路徑不存在 |
| RC-3 | Claude Code 載入的是早於 dockeeper 的**舊版實體 copy**（12 agents、無 `golem-dockeeper`、無 `doc-sync`） | skill base dir 證實；`plugin.json version=1.0.0` |
| RC-4 | 含 dockeeper 的正確 render 只在孤兒暫存（共 **9 個** `.gal-render-*`，已閉合） | `~/.gal/plugins/.gal-render-*/`；Claude Code 不載入這些 |
| RC-5 | 推定核心 bug | Rust provider projection 的**目標面**與 Claude Code **實際載入面**不一致；atomic render 的 temp→swap→cleanup 未閉合 |
| RC-6 | `galRoot` 來源解析不一致 | config `galRoot` 指 repo root，但 Rust `is_gal_root_usable`/render 期望 galRoot **直接**含 `agents/skills/commands`；實際 source 在 `plugins/gal-core/` 下。凍結 Bash oracle 內部 append `plugins/gal-core`（`provider-plugin.sh:413`），Rust 不 append → dev-mode install 失敗 |

## 現況（2026-06-08，本機）

- **乾淨重裝完成**：清除 `~/.gal/*` 只留 `~/.gal/config/`，release `gal install`（dev mode）一次性重建。**RC-2 / RC-4 / RC-5（孤兒 + canonical root）已閉合**：canonical root 正確建出（agents/ 13 含 `golem-dockeeper`、skills/ 29 含 `doc-sync`、commands/ 11、agy-agents/、rules/、manifests），無孤兒，`gal doctor` exit 0。
- **galRoot 已修（config 層）**：`galRoot` 改指 `…\plugins\gal-core`，dev-mode install 可裝。Rust 端正規解析列為 R-10。
- **未閉合**：RC-1（PATH，→ R-04 bin 暴露）；RC-3 拆兩半 —— skill 面（doc-sync）由 R-03 收斂到 `~/.claude/skills/gal`；**agent 面（dockeeper 仍從舊 cache 載入）移入 Non-Goals、延至後續計畫**。

## Requirements

- [ ] **R-01 單一 Rust 入口可端對端安裝**：release build 的 `gal` binary 經 `gal install` / `gal update` 完成完整安裝，無需 `bash`/`pwsh` 前綴；安裝流程全在 Rust（消費 bootstrap 引擎）。
- [ ] **R-02 完整 source 收斂（目錄掃描，非白名單）**：所有 source agents/skills/commands/conventions/workflows/templates 以目錄列舉渲染進 canonical root；新增 agent/skill（如 `golem-dockeeper`/`doc-sync`）自動納入，無硬編碼清單可漏。
- [ ] **R-03 skill 活讀取面對齊（核心修復；引擎核心變更）**：install/update 必須把 canonical root 收斂進 **GAL 自有 skill 面** `~/.claude/skills/gal`（symlink 指向 canonical root），讓 `doc-sync` 等 skill 被 Claude 載入。隨 canonical root 更新即時生效，不經官方流程，不碰 legacy `~/.claude/plugins/gal`。P0 實機驗證該 skill 面被 Claude 載入。**subagent（agents/）的 Claude 活載入不在本計畫**（見 Non-Goals）。此項改 `gal-engine` 核心 projection 契約，走 bootstrap 邊界 architect 審。
- [ ] **R-04 plugin `bin/` 暴露（吸收 plugin-bin-migration 全部範圍）**：published plugin root 的 `bin/` 含本機 OS 原生 `gal`(`.exe`)，Claude Code 啟用外掛後 Bash tool 可裸呼 `gal`；**不**放 `bin/gal.sh`/`bin/gal.ps1` shell wrapper；binary 缺失時 render fail-loud、不產半成品。
- [ ] **R-05 atomic + 清理**：render-to-temp-then-swap 必須在 swap 後刪除自身 temp；install/`gal doctor` 偵測並清理孤兒 `~/.gal/plugins/.gal-render-*`。
- [ ] **R-06 doctor 驗證活讀取面**：`gal doctor` 比對活讀取面與 source —— agent/skill 數一致、`golem-dockeeper`+`doc-sync` 存在、`bin/gal`(`.exe`) 存在且可執行、無孤兒 temp；缺口 → 非零 exit 指出修復面。
- [ ] **R-07 package-manager 安裝路徑**：消費 bootstrap P2 的 winget/Homebrew/GitHub Releases artifacts；package-manager 裝出的 `gal` 能完成同一套 install 收斂。
- [ ] **R-08 跨平台正確安裝**：Windows + macOS + Linux 的 PATH/symlink/junction/權限（Unix `+x`）正確；活讀取面對齊在三平台皆驗。
- [ ] **R-09 端對端驗收（本計畫範圍）**：乾淨環境安裝後，(a) Claude Code 可實際載入 `doc-sync`（skill 出現在 registry，經 `~/.claude/skills/gal`）；(b) `golem-dockeeper` 正確存在於 canonical root `agents/` 且 `gal doctor` 證實 source-complete。**dockeeper 作為可調用 Claude subagent 不在本計畫驗收**（見 Non-Goals）。
- [ ] **R-10 `galRoot` 來源解析正規修法（RC-6；引擎核心變更）**：`gal-engine` 在 galRoot（repo root）下自動解析 `plugins/gal-core`，對齊凍結 oracle，免除 config 繞過。**落地時須同一 commit 把本機 config `galRoot` 還原為 repo root**（否則雙重 append 壞掉，B-03）；引擎須容忍 galRoot 已是 gal-core 的舊形式。觸及受保護的 `gal-engine` 核心 + bootstrap 邊界，須 architect 審。
- [ ] **R-11 script 退役 = 刪除（本計畫的終態目標）**：本計畫**確實取代** ps1/bash，不是永久凍結。**每完成一個功能並驗證 Rust parity，即刪除對應的 ps1/bash（`.ps1` + `.sh` 成對）**。涉及的 script 家族：`Build-CorePlugin.*`/`build-core-plugin.sh`、`Build-ProviderPlugins.*`/`build-provider-plugins.sh`、`common/provider-plugin.sh`/`ProviderPlugin.ps1`、`Update-Mcp.*`、`Install-GalPlugins.ps1`/`install-gal-plugins.sh`、`common/Common.ps1`/`common.sh`（install 拓撲共用部分）、對應 `Test-*.ps1` 測試器、`gal.ps1` entry。**刪除前置**：(1) 該 script 所實作的**所有**功能皆已 Rust parity（monolithic script 須等其全部消費者都被取代）；(2) 仍把該 script 當 **live 測試 oracle** 的測試（如 `cross_platform_oracle_parity.rs`、`test-t022-ssh.sh`）須先改為 **snapshot fixture** 或行為測試，否則不得刪。**不在範圍**：`Setup-Machine.*`、`Sync-DevContext.*`（機器設定/adapter 生成，非 install 取代對象）。plan 結束時 superseded install 家族 script 不留。

## Non-Goals（本計畫不做，延後）

- **Claude subagent 活載入（agents/）**：`golem-dockeeper` 等 subagent 由 Claude plugin 機制（版本化 cache = 官方 `claude plugin install`）載入，無 GAL 自有非官方面。本計畫只把 agents 渲染進 canonical root，不處理其 Claude 活載入；延至後續計畫決定機制（可能採官方流程）。
- **官方 Claude marketplace / `/plugin` 流程**：不在本計畫採用或實作。
- **legacy `~/.claude/plugins/gal`**：不寫入、不依賴（oracle 視為 legacy 並移除）。

## Approach（Phases，逐 phase 可獨立驗證）

scripts 為**過渡 oracle**，非永久凍結：功能達 Rust parity 即刪對應 ps1/bash（R-11）。Rust 行為以 `cargo test` 驗證；安裝面在隔離 home 驗證。每個 phase 的「達 parity 後刪除對應 script」是該 phase 的收尾動作，前提是 oracle 測試已 reparent（R-11 前置）。

| Phase | 目標 | 前置 |
| --- | --- | --- |
| **P0 skill 載入面驗證** | 實機驗證 GAL 自有 skill 面 `~/.claude/skills/gal` 被 Claude Code 載入（doc-sync 可見）；對 Copilot/AGY 列出各自自有面 | — |
| **P1 收斂到 skill 活面** | 修 `providers/claude` 投影目標為 `~/.claude/skills/gal`（非 legacy `plugins/gal`）；canonical root 建出並由 GAL 自管 link 被 Claude 載入 | bootstrap 引擎、P0 |
| **P2 bin 暴露** | render `bin/gal`(`.exe`)；fail-loud；PATH smoke | P1 |
| **P3 atomic + 孤兒清理** | swap 後刪 temp；偵測清理 `.gal-render-*` | P1 |
| **P4 doctor 活面驗證 + gate** | doctor 比對活面 vs source；release gate 併入 | P1–P3 |
| **P5 跨平台 + pkg-manager** | macOS/Linux 安裝面對齊；winget/Homebrew/Releases 裝出的 binary 完成收斂 | P1–P4、bootstrap P2 |
| **P6 端對端驗收** | 乾淨環境 doc-sync 可載、canonical root agents 完整 | P1–P5 |
| **P7 script 退役（刪除）** | reparent oracle 測試為 snapshot fixture → 刪除 superseded install 家族 ps1/bash（R-11） | 各 phase parity 完成 |

### P0：skill 載入面驗證（Loading-Surface Contract）

- **Files**: `docs/devguide.md`、本計畫
- **What**: 以實機探測**驗證** GAL 自有 skill 面 `~/.claude/skills/gal`（symlink 指向 canonical root）確實被 Claude Code 載入 skill；鎖定 GAL 自管更新如何被 Claude 重新讀取。對 Copilot（manifest/host copy）、AGY（junction）列出各自實際自有面。**不**含 subagent 活載入（Non-Goals）。若 skill 面無法載入，回報 P0 阻斷。
- **Verify**: devguide 有「每 provider GAL 自有載入面」對照表；實機證實 `~/.claude/skills/gal` 被 Claude 載入（`doc-sync` 可見）。

### P1：Rust install 收斂到活讀取面（核心修復）

- **Files**: `crates/gal-engine/src/providers/claude.rs`（及 copilot/agy 視 P0 結果）、`install.rs`、`render.rs`
- **What**: 修正 provider projection 目標為 `~/.claude/skills/gal`（GAL 自管 symlink → canonical root），使 `gal install`/`update` 的 skill 面即時對齊 source；canonical root（`~/.gal/plugins/gal`）正確建出並作為單一來源。所有 agents/skills 以目錄掃描渲染進 canonical root（R-02）；其中 skill 經 skill 面被 Claude 載入，agent 僅落 canonical root（活載入延後）。
- **Verify**: 隔離 home `gal install` 後，`~/.claude/skills/gal` 解析到 canonical root 且 `doc-sync` 被 Claude 載入；canonical root agents/ 含 `golem-dockeeper`、skills/ 含 `doc-sync`。

### P2：plugin `bin/` 暴露（合併 plugin-bin-migration）

- **Files**: `crates/gal-engine/src/render.rs`/`install.rs`（bin 複製）、`scripts/Build-CorePlugin.*`（凍結期過渡）
- **What**: render published plugin root 時建立 `bin/`，複製本機 OS 原生 `gal`（Unix 無副檔名 +x、Windows `gal.exe`）；缺 binary 時 fail-loud；不放 shell wrapper。binary source = bootstrap 產出（source build 或 package-manager 安裝位）。
- **Verify**: 啟用 plugin 後 Bash tool 裸呼 `gal --version` 解析到 plugin `bin/`；Windows 有 `bin/gal.exe`、Unix 有可執行 `bin/gal`，無 `.sh`/`.ps1`。

### P3：atomic swap + 孤兒清理

- **Files**: `crates/gal-engine/src/render.rs`、`install.rs`、`doctor.rs`
- **What**: render-to-temp-then-swap 在成功 swap 後刪除自身 temp；中斷殘留由 install/doctor 以白名單（僅 GAL-owned `.gal-render-*` 前綴）偵測並清理。
- **Verify**: 重跑多次 install 後 `~/.gal/plugins/` 無 `.gal-render-*` 殘留；kill-mid-render 重跑收斂無半渲染。

### P4：doctor 驗證活讀取面 + release gate

- **Files**: `crates/gal-engine/src/doctor.rs`、`crates/gal-cli/src/`
- **What**: `gal doctor` 新增「活讀取面 vs source」檢查：agent/skill 數、`golem-dockeeper`+`doc-sync` 存在、`bin/gal`(`.exe`) 可執行、無孤兒 temp。`gal doctor --release-gate` 併入。
- **Verify**: 缺 dockeeper / 缺 bin / 有孤兒 → doctor 非零並指出修復面；完整安裝 → exit 0。

### P5：跨平台 + package-manager 安裝路徑

- **Files**: `crates/gal-engine/src/`（路徑/symlink/junction/權限）、`packaging/winget/`、`packaging/homebrew/`
- **What**: 三平台安裝面對齊（Windows junction、Unix symlink + `+x`）；winget/Homebrew/Releases 裝出的 `gal` 能完成同一收斂。消費 bootstrap P2 artifacts，不重做 release lane。
- **Verify**: macOS/Linux 隔離 home install → 活面對齊（== 凍結 Bash oracle 對既有正確面；dockeeper 以意圖斷言）；pkg-manager 裝出的 binary `gal install` 後 dockeeper 可見。

### P6：端對端驗收（本計畫範圍）

- **Files**: 測試/驗收腳本、`docs/devguide.md`
- **What**: 乾淨環境完整安裝後，於 Claude Code 確認 `doc-sync` skill 可用（經 `~/.claude/skills/gal`）；`gal doctor` 證實 canonical root `agents/` 含 `golem-dockeeper`、source-complete。
- **Verify**: 手動 smoke：新 Claude Code session 能用 `doc-sync`；doctor green。（dockeeper 作為可調用 subagent 的驗收延至後續計畫。）

### P7：script 退役（刪除 superseded ps1/bash）

- **Files**: `scripts/`（install 家族）、`crates/gal-engine/tests/cross_platform_oracle_parity.rs`、`scripts/test-t022-ssh.sh`
- **What**: 對每個達 Rust parity 的功能，刪除其對應 ps1/bash（成對）。**前置**：先把仍以 live script 為 oracle 的測試改為 snapshot fixture（凍結一份 oracle 輸出存進 `tests/fixtures/`）或改為純行為/intent 測試，移除對 `build-core-plugin.sh` 等的 runtime 依賴；再刪 script。monolithic script（如 `install-gal-plugins.sh` 涵蓋 Claude/Copilot/Codex/AGY）須等其**所有**消費者功能皆 parity 才整檔刪。`gal.ps1` entry 在 Rust binary 接管所有子命令後刪。
- **Verify**: `cargo test` 全綠且不再 spawn 任何 `scripts/*.sh`/`*.ps1`（grep 測試碼無 live script 呼叫）；`scripts/` 不再有 superseded install 家族檔；`Setup-Machine.*`/`Sync-DevContext.*` 保留。

## Files to Create or Modify

- `[MODIFY]`（引擎核心，bootstrap 擁有）`crates/gal-engine/src/providers/claude.rs`、`install.rs`、`render.rs`、`doctor.rs` — skill 收斂目標改為 `~/.claude/skills/gal`、bin 複製、atomic cleanup、活面驗證、galRoot 解析（R-10）。
- `[MODIFY]` `crates/gal-engine/src/mode.rs` — galRoot 在 repo root 下自動解析 `plugins/gal-core`（R-10/RC-6）。
- `[MODIFY]` `crates/gal-cli/src/main.rs` — doctor 活面檢查/exit 分級接線（如需）。
- `[DELETE on parity]`（R-11，P7）`scripts/Build-CorePlugin.ps1`/`build-core-plugin.sh`、`scripts/Build-ProviderPlugins.ps1`/`build-provider-plugins.sh`、`scripts/common/ProviderPlugin.ps1`/`provider-plugin.sh`、`scripts/Update-Mcp.*`、`scripts/Install-GalPlugins.ps1`/`install-gal-plugins.sh`、`scripts/common/Common.ps1`/`common.sh`（install 部分）、對應 `scripts/Test-*.ps1`、`scripts/gal.ps1` — 功能達 Rust parity + oracle 測試 reparent 後成對刪除。
- `[KEEP]` `scripts/Setup-Machine.*`、`scripts/Sync-DevContext.*` — 機器設定/adapter 生成，非本計畫取代對象。
- `[MODIFY]` `crates/gal-engine/tests/cross_platform_oracle_parity.rs`、`scripts/test-t022-ssh.sh` — 改為 snapshot fixture，解除對 live `build-core-plugin.sh` 的依賴（P7 前置）。
- `[MODIFY]` `docs/devguide.md`、`docs/manual.md`、`README.md` — GAL 自有載入面契約、bin exposure、正確安裝流程；移除/標示過時官方 marketplace 敘述（含 `README.md:44-48`）。
- `[MODIFY]` `crates/gal-engine/src/doctor.rs` — 移除/改寫 `ClaudeMarketplaceState` 官方 marketplace 三狀態分類，改為 GAL 自有面健康檢查（連帶清理，與 R-03/R-06 同步）。
- `[MODIFY]` `packaging/winget/`、`packaging/homebrew/` — 安裝後收斂驗證（消費 bootstrap artifacts）。
- `[DELETE]` `docs/plans/plugin-bin-migration.md` — 範圍併入本計畫。

## Tasks

- [x] **T-001（P0，gate）** — 實機探測並在 `docs/devguide.md` 寫「每 provider GAL 自有載入面」對照表：證實 `~/.claude/skills/gal` 被 Claude 當 skill 來源載入（`doc-sync` 可見）；列出 Copilot（manifest/host copy）、AGY（junction）各自實際自有面。**go/no-go**：skill 面無法載入則阻斷，回報後重評（不採官方流程）。*(f83f071)*
- [x] **T-002（R-10/RC-6，引擎核心）** — `crates/gal-engine/src/mode.rs`：galRoot 在 repo root 下自動解析 `plugins/gal-core`；容忍 galRoot 已是 gal-core 的舊形式（不雙重 append）。**同一 commit** 把本機 config `galRoot` 還原為 repo root。須 bootstrap 邊界 architect 簽核。*(a192df7)*
- [x] **T-003（P1，引擎核心）** — `crates/gal-engine/src/providers/claude.rs`（+`install.rs`/`render.rs`）：skill projection 目標改為 `~/.claude/skills/gal`（symlink → canonical root），不碰 legacy `~/.claude/plugins/gal`；`gal install`/`update` 後 skill 面即時對齊 source。依賴 T-001 證實面正確。須 bootstrap 邊界 architect 簽核。*(c3bf950)*
- [x] **T-004（P2）** — `render.rs`/`install.rs`：published plugin root 建 `bin/`，複製本機 OS 原生 `gal`（Unix +x、Windows `gal.exe`），缺 binary fail-loud、不放 `.sh`/`.ps1` wrapper。*(62bc53d)*
- [x] **T-005（P3）** — `render.rs`/`install.rs`/`doctor.rs`：atomic swap 成功後刪自身 temp；install/doctor 以白名單（`.gal-render-*` 前綴）偵測清理孤兒，doctor-first、不 delete-through。*(b0cc86d)*
- [x] **T-006（P4，引擎核心）** — `doctor.rs`/`crates/gal-cli/src/main.rs`：新增「活面 vs source」檢查（agent/skill 數、`golem-dockeeper`+`doc-sync` 存在、`bin/gal` 可執行、無孤兒），exit 分級；`--release-gate` 併入；移除/改寫 `ClaudeMarketplaceState` 官方 marketplace 三狀態分類為 GAL 自有面健康檢查。*(c882267)*
- [x] **T-007（P5 跨平台）** — `gal-engine`：Windows junction / Unix symlink + `+x` 三平台路徑/權限對齊；macOS/Linux 隔離 home install 驗證 skill 面對齊。*(04babd9)*
- [x] **T-008（P5 pkg-manager）** — `packaging/winget/`、`packaging/homebrew/`：消費 bootstrap P2 artifacts，pkg-manager 裝出的 `gal` 能完成同一套 install 收斂（安裝後收斂驗證，不重做 release lane）。*(93b41c7)*
- [x] **T-009（P6）** — 端對端驗收腳本 + `docs/devguide.md`：乾淨環境安裝後 Claude Code 可用 `doc-sync`、`gal doctor` green、canonical root agents/ 含 `golem-dockeeper`。 *(8615eb5)*
- [x] **T-010（P7 前置）** — reparent oracle 測試：`crates/gal-engine/tests/cross_platform_oracle_parity.rs`、`scripts/test-t022-ssh.sh` 改為 `tests/fixtures/` snapshot 或行為/intent 測試，移除對 live `scripts/*.{sh,ps1}` 的 runtime 依賴。**必須在 T-011 前完成。** *(d88d417)*
- [x] **T-011（P7，R-11）** — 刪除 superseded install 家族 ps1/bash（成對）：`Build-CorePlugin.*`/`build-core-plugin.sh`、`Build-ProviderPlugins.*`/`build-provider-plugins.sh`、`ProviderPlugin.ps1`/`provider-plugin.sh`、`Update-Mcp.*`、`Install-GalPlugins.*`/`install-gal-plugins.sh`、`Common.*`/`common.sh`（install 部分）、對應 `Test-*.ps1`、`gal.ps1`（Rust 接管所有子命令後）。monolithic script 須其全部消費者皆 parity 才整檔刪。**保留** `Setup-Machine.*`/`Sync-DevContext.*`。 *(bcc755a — partial; 4 Test-*.ps1 刪除；主要 ps1/bash 對延後：Setup-Machine 尚調用 install-gal-plugins.*)*
- [ ] **T-012（docs）** — `docs/devguide.md`/`docs/manual.md`/`README.md`：寫 GAL 自有載入面契約、bin exposure、正確安裝流程；移除/標示過時官方 marketplace 敘述（含 `README.md:44-48`）。
- [ ] **T-013（cleanup）** — 刪 `docs/plans/plugin-bin-migration.md`；文件 grep 確認無「plugin-bin 與 install 收斂分屬兩計畫」矛盾、無官方 marketplace 收斂策略殘留。

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-01 | manual | `docs/devguide.md`「GAL 自有載入面」對照表與實機一致；Claude 從 `~/.claude/skills/gal` 載入 skill 明確記載 | T-001 |
| TP-02 | integration/intent | 隔離 home `gal install` 後 `~/.claude/skills/gal` 解析到 canonical root 且 `doc-sync` 被 Claude 載入；canonical root agents/ 含 `golem-dockeeper`、skills/ 含 `doc-sync` | T-003 |
| TP-03 | parity | 既有正確 skills 在 skill 面 == 凍結 Claude oracle（逐檔；T-010 後改比 fixture） | T-003 |
| TP-04 | unit/integration | Windows render 出 `bin/gal.exe`、Unix 出可執行 `bin/gal`，無 `.sh`/`.ps1`；缺 binary → fail-loud | T-004 |
| TP-05 | manual | plugin 啟用後 Bash tool 裸呼 `gal --version` 解析 plugin `bin/` | T-004 |
| TP-06 | integration | 多次 install 後 `~/.gal/plugins/` 無 `.gal-render-*` 孤兒；kill-mid-render 重跑收斂無半渲染 | T-005 |
| TP-07 | unit/integration | doctor 對「缺 dockeeper / 缺 bin / 有孤兒」各報非零並指出修復面；完整安裝 exit 0；`--release-gate` 一致 | T-006 |
| TP-08 | unit | 移除 `ClaudeMarketplaceState` 後 doctor/release-gate 仍編譯且 exit 分級正確，無官方 marketplace 分類殘留 | T-006 |
| TP-09 | unit | config `galRoot` 指 repo root（非 `plugins/gal-core`）時 `gal install` 仍自動解析 source 並成功收斂；galRoot=gal-core 舊形式不雙重 append | T-002 |
| TP-10 | parity | macOS、Linux 隔離 home 安裝面對齊（T-010 後比 fixture）；pkg-manager 裝出的 binary `gal install` 後 skill 面收斂 | T-007, T-008 |
| TP-11 | manual | 乾淨環境安裝後 Claude Code 可用 `doc-sync`（smoke）；`gal doctor` green 且 canonical root agents/ 含 `golem-dockeeper` | T-009 |
| TP-12 | unit | T-010 後 `cargo test` 全綠且測試碼不再 spawn 任何 `scripts/*.{sh,ps1}`（grep 驗證）；parity 測試改讀 fixture | T-010 |
| TP-13 | manual | superseded install 家族 ps1/bash 已刪除；`Setup-Machine.*`/`Sync-DevContext.*` 仍在；`gal.ps1` 移除後 entry 全走 Rust | T-011 |
| TP-14 | manual | 文件 grep：repo 無「plugin-bin-migration 與 install 收斂分屬兩計畫」矛盾、無官方 marketplace 收斂策略殘留；`plugin-bin-migration.md` 已刪 | T-012, T-013 |
| TP-15 | integration | **mac-mini（Unix）純 normal mode** 乾淨安裝：packaged-source 自 binary 解析（`resolve_packaged_source_root`，從未實機驗）、`~/.claude/skills/gal` Unix symlink + `doc-sync` 載入、canonical root 正確、無孤兒、`gal doctor` green | T-001, T-003, T-007, T-009 |
| TP-16 | integration | **Windows 雙模式**：先 normal mode（packaged-source + junction）後 dev mode（galRoot；R-10 後 repo-root）乾淨安裝，兩模式皆 canonical root 正確 + skill 面對齊 + doctor green | T-002, T-003, T-007 |
| TP-17 | integration（**optional / 加強，非 gate**）| **mac-mini `/gal pipeline` write-back spike**：安裝完成後選擇性驗證 **3 個** executor（claude / opencode / copilot）的真實 headless write-back —— 目標檔被次要工具寫回 **且** `.dev/executor-logs/` 終態 `completed` **且** 留可 resume 的 native session id（honest-test-pass-bar）。**codex（預算）/ agy（無安裝路徑）標 ⬜ 未執行 + 原因，不標 PASS**（availability gap，非能力否定）。借 mac-mini 安裝場地順手做的加強驗證，**不阻斷本計畫完成**。 | optional（自足加強） |

## 驗證環境與順序（乾淨安裝實機驗證）

平台×模式分工與執行順序（2026-06-08 使用者裁決，搭配 SSH 連 mac-mini）：

1. **mac-mini（Unix，純 normal mode）— 先做**：經 SSH 在 mac-mini 乾淨安裝。mac-mini 無 repo，天然只能走 normal mode → 正好專測 **normal-mode packaged-source 路徑**（binary 自解析 source，`resolve_packaged_source_root`，此路徑從未實機驗，是 RC-3/R-03 normal 情境的關鍵驗證）。驗 Unix symlink `~/.claude/skills/gal` + `+x` + doc-sync 載入。（TP-15）
2. **設定檔搬移**：mac-mini 測完後，把本 Windows 機 `~/.gal/config/` 複製到 mac-mini（攜帶 machine-local 設定）。**注意**：現有 config `devMode=true` + `galRoot=Windows 路徑` 為 Windows-specific —— mac 上若要續測須對應調整（normal mode 設 `devMode=false`，或 dev mode 指向 mac 上的 repo path）；複製主要為帶 `*.local.*` 設定，非 dev/galRoot 欄位。
3. **Windows（本機）— 後做，雙模式**：先測 **normal mode**（packaged-source + junction），再測 **dev mode**（本機有 repo + galRoot，覆蓋 dev 路徑；R-10 後 galRoot 用 repo-root）。（TP-16）

| 平台 | 模式 | 對應 TP | 重點 |
| --- | --- | --- | --- |
| mac-mini (Unix) | normal | TP-15, TP-10 | packaged-source 自解析、Unix symlink、+x、doc-sync 載入 |
| Windows | normal | TP-16 | packaged-source、Windows junction |
| Windows | dev | TP-16（現況 dev install 已 green） | galRoot 解析（R-10 後 repo-root；現況 gal-core 繞過） |
| Linux | normal | TP-10 | 有 Linux host 時補（可經 `test-t022-ssh.sh` 後繼，T-010 reparent 後） |

順序理由：mac-mini 先做可在**乾淨、無 repo** 環境專測 normal-mode packaged 路徑（最未驗、風險最高）；Windows 留後因本機可同時覆蓋 normal + dev 兩模式。

**Optional 加強（TP-17，非 gate）**：mac-mini 安裝完成後，趁機選擇性跑 `/gal pipeline` 的 headless write-back spike，驗證 3 個可用 executor（claude / opencode / copilot）的真實 receipt（寫回 + executor-log `completed` + native session id）。codex（預算）/ agy（無安裝路徑）標 ⬜ 未執行。借 mac-mini 安裝場地順手做的自足加強驗證，**不阻斷本計畫**。

### install-followups 殘留對照（本計畫驗證能閉到哪）

`docs/observations/install-followups.md` 是已刪 bootstrap 計畫的殘留清單。本計畫的實機驗證（特別是 mac-mini）只閉得了其中「install 路徑 / 跨平台」子集；security / M2 / code-hardening 項目需另外改 code，**驗證跑完也不會自動閉**。

| install-followups 項目 | 本計畫驗證 | 結果 |
| --- | --- | --- |
| FU-01 normal-mode packaged-source（已 RESOLVED） | TP-15 | ✅ 閉 — mac-mini 純 normal mode 首次實機跑 `resolve_packaged_source_root`，正是 FU-01 待的實機證據 |
| FU-02 oracle-parity TP-014/015（partial） | TP-15/16 + T-010 | ✅ 吸收 — 隔離 home 安裝 + parity 改 fixture（R-11 刪 script 後 byte-parity → fixture-parity） |
| macOS/Linux 跨平台（T-022，code done、實機待跑） | TP-15 / TP-10 / T-007 / T-010 | ✅ macOS 半閉（mac-mini）；Linux 待 host |
| FU-04 `is_readable` UNC timeout | （T-002 順手可帶，同在 `mode.rs`） | ⚠️ 需 code 修，非驗證可閉；建議併入 T-002 |
| S-1 secret-guard regex | — | ❌ 需 code 修，範圍外 |
| S-2 AGY `unwrap()` panic | — | ❌ 需 code 修（AGY 大多 Non-Goals） |
| S-3 cosign CI 信任 | — | ❌ release 簽章，T-014 範圍外 |
| FU-03 uninstall ledger 不精確 | — | ❌ 需 code 修，範圍外 |
| R5 commit-msg scope injection | — | ❌ 範圍外 |
| AGY 交易/ledger（M2） | — | ❌ Non-Goals |
| MCP AGY/Codex/OpenCode（M2） | — | ❌ 範圍外 |

**TP-17 對 install-followups 的貢獻 = 0**：它是 headless executor pipeline spike，與 install 引擎殘留無關。

**結論**：跑完本計畫驗證（含 TP-15/16/17），install-followups 約**閉 3/11**（install 路徑 + macOS 跨平台）；其餘約 8 項屬 security / M2 / hardening，需另外改 code，不會因驗證而閉。要清空該檔需另開 hardening/M2 計畫處理 S-1/2、FU-03/04 等（其中 FU-04 可順手併入 T-002）。

## Success Criteria

- [ ] 乾淨環境以原生 `gal install`（或 package-manager 裝出的 `gal`）安裝後：canonical root 含全部 source agents（含 `golem-dockeeper`）與 skills（含 `doc-sync`）；Claude **skill 面** `~/.claude/skills/gal` 載入 `doc-sync`。（subagent 活載入為後續計畫範圍。）
- [ ] Claude Code 啟用 GAL 外掛後可裸呼 `gal`，plugin `bin/` 只含本機 OS 原生執行檔。
- [ ] `~/.gal/plugins/` 無孤兒 `.gal-render-*`；canonical root 存在且與 skill 面一致。
- [ ] `gal doctor` 能證明 canonical root == source（含 dockeeper/doc-sync）、skill 面/bin 對齊，缺口時 fail-loud。
- [ ] `galRoot` 指 repo root 即可安裝（R-10，免 config 繞過）。
- [ ] Windows/macOS/Linux 三平台安裝面皆對齊。
- [ ] 安裝/收斂/bin 全程 Rust 原生；**superseded install 家族 ps1/bash 已實際刪除**（非凍結保留），oracle 測試已 reparent 為 fixture；`Setup-Machine.*`/`Sync-DevContext.*` 保留。

## Risks

- **skill 面未被載入（中）**：若 `~/.claude/skills/gal` 實際未被 Claude 當 skill 來源載入，doc-sync 收斂落空。Mitigation：P0 實機驗證 skill 面被載入（doc-sync 可見）+ TP-09 兜底；此面為凍結 oracle 的持久投影面（`common.sh:75`），風險低於先前的 legacy 選擇。
- **agent 活載入缺口已切出（已知，延後）**：subagent 無 GAL 自有非官方面，本計畫不處理（Non-Goals）。Risk-acceptance：後續計畫負責；本計畫 doctor 仍報 canonical root agents/ 完整，避免「以為裝好」。
- **與 bootstrap 引擎範圍重疊（中）**：本計畫修 `providers/claude.rs`、`mode.rs` 等 bootstrap 擁有的檔案。Mitigation：本計畫只改「收斂目標 + bin + cleanup + 活面驗證 + galRoot 解析」delta；引擎核心契約變更回 bootstrap，交叉引用同步。
- **`galRoot` 來源解析（中，RC-6）**：正規修法須在 `gal-engine` 自動解析 `plugins/gal-core`，觸及受保護核心。Mitigation：R-10 回 bootstrap 邊界由 architect 審；本機已先以 config 繞過不擋進度。
- **跨平台安裝差異（中）**：junction/symlink/權限三平台不同。Mitigation：P5 各平台隔離 home 測試，Windows 先行。
- **孤兒清理誤刪（中）**：白名單僅限 GAL-owned `.gal-render-*` 前綴，doctor-first、不 delete-through。
- **過時官方 cache 殘留**：舊官方 `/plugin` cache（v1.0.0）與 GAL 自有面並存可能造成 Claude 載到舊版。Mitigation：doctor 偵測並報告 stale 官方 cache，必要時提示使用者移除舊官方註冊。
- **刪 script 時 oracle 測試失依（中，R-11/P7）**：`cross_platform_oracle_parity.rs`、`test-t022-ssh.sh` 等仍 spawn `build-core-plugin.sh` 作 live oracle；直接刪 script 會破壞測試的 parity 基準。Mitigation：刪前先 reparent —— 凍結一份 oracle 輸出為 `tests/fixtures/` snapshot，或改為行為/intent 測試；monolithic script 須所有消費者皆 parity 才整檔刪。P7 明列此前置。

## Approval

- Human approval: [approved at 2026-06-08]
- Architect review: **APPROVE（方向；REVISE 修正已折入 2026-06-08）** — 見 ## Review Results > ### Architecture Review。B-01（legacy 面）已改 `~/.claude/skills/gal`；B-02（agent 載入缺口）經使用者裁決移入 Non-Goals 延後；B-03/B-04 已折入。實作期 R-03/R-10 為引擎核心變更，須走 bootstrap 邊界 architect 簽核。下一步 `/refining-plan`。
- Additional domain review: [not triggered]（無 customer-facing / business-rule 內容）

## Review Results

### Architecture Review

**Verdict: REVISE → 修正已折入（2026-06-08），現 APPROVE（方向）。** Rust-native 單一實作 + 「活讀取面 = source」治理原則正確。初審兩個阻斷 + 兩個必折修正，已全數處理（見下方 Resolution）。下一步 `/refining-plan`（`## Tasks`、`## Test Plan` 尚未展開）。

**Resolution（2026-06-08）**：
- **B-01 已修**：skill 收斂面 `~/.claude/plugins/gal`（legacy）→ `~/.claude/skills/gal`（oracle 持久投影面）。Goal/R-03/P0/P1/Files/TP 同步。
- **B-02 已切出範圍（使用者裁決）**：subagent（dockeeper）無 GAL 自有非官方載入面 → agent 活載入移入 Non-Goals、延至後續計畫（非放棄）。Goal/R-09/Success Criteria/P6/TP-09/Risks 同步去前提化，doctor 仍驗 canonical root agents/ 完整避免假完成。
- **B-03 已折入**：R-10 加「正規修法與 config `galRoot` 還原同 commit、引擎容忍舊形式」但書。
- **B-04 已折入**：R-03/R-10 明標「引擎核心變更（bootstrap 擁有）」，走 bootstrap 邊界 architect 審；Files 標註。
- **內部矛盾已修**：載入面不再當既定事實，改為 P0 待證（skill 面）。

**Addendum（2026-06-08，使用者裁決：scripts 須真刪非凍結）**：新增 R-11 + P7 —— 本計畫**確實取代** ps1/bash，功能達 parity 即成對刪除對應 script（非永久凍結為 oracle）。架構面唯一新風險：多個 oracle-parity 測試（`cross_platform_oracle_parity.rs`、`test-t022-ssh.sh`）仍以 live `build-core-plugin.sh` 為基準，**直接刪會破壞測試**。已加守則：刪前先 reparent 為 `tests/fixtures/` snapshot 或行為測試；monolithic script 須全部消費者 parity 才整檔刪。此為加法式澄清，與「Rust 單一實作」目標一致，維持 APPROVE。

#### Trade-off Summary

| 決策 | 效益 | 成本 | 裁決 |
| --- | --- | --- | --- |
| Rust 單一實作、消費 bootstrap 引擎 | 消除三面對等稅、單一 runtime | R-03/R-10 實際改 `gal-engine` 核心（受保護），非純下游 | REVISE（邊界要誠實，見 B-04） |
| 「活讀取面 == source」為唯一驗收 | 擋掉「render 對但沒收斂」這類 bug | 需先證明哪個面才是活載入面 | OK（治理原則正確） |
| 目錄掃描非白名單（R-02） | 新 agent/skill 自動納入，免漏 dockeeper | — | OK |
| GAL 自有面 = `~/.claude/plugins/gal` symlink（D-01） | 不依賴官方流程 | **此面是凍結 oracle 明定的 legacy、每次刷新即刪**；且只能載 skill 不能載 agent | REJECT（見 B-01/B-02） |

#### Bug Surface

- **[B-01]（阻斷）收斂目標選到被刪除的 legacy 面。** R-03/Goal 指定 `~/.claude/plugins/gal` 為 GAL 自有載入面。但凍結 oracle 明確界定：`CLAUDE_PLUGIN_INSTALL_TARGET = ~/.claude/skills/gal`（持久 GAL 投影面，`common.sh:75`），而 `CLAUDE_LEGACY_PLUGIN_INSTALL_TARGET = ~/.claude/plugins/gal`（`common.sh:76`）是 **legacy，每次刷新由 `safe_unlink` 主動移除**（`install-gal-plugins.sh:495/505/523/587`）。本機 `installed_plugins.json` 亦證實 Claude 載入的是版本化 cache，非此 symlink。→ 本計畫若投到 `~/.claude/plugins/gal`，會重演「投到 Claude 不載入的面」這個本計畫要修的同一個錯。**修正**：skill 面改用 `~/.claude/skills/gal`（GAL 自有、非官方、非 legacy）。
- **[B-02]（阻斷，需使用者裁決）agent vs skill 載入缺口。** `~/.claude/skills/gal` 是 **skills 投影**，只讓 skill（`doc-sync`）可被發現；它**不載入 subagent**（`golem-dockeeper`）。Claude Code 的 subagent 來自已安裝 plugin 的 `agents/`（plugin 機制 → 版本化 cache，即官方 `claude plugin install` 流程）。換言之：移除官方流程後，**doc-sync（skill）有 GAL 自有路徑可走，但 golem-dockeeper（agent）很可能沒有任何非官方可載入面**。這直接威脅 R-09 / Success Criterion「dockeeper 可被調用」。此為計畫成立與否的關鍵，需使用者裁決（見 Recommended Changes #2 與下方 focused question）。
- **[B-03]（中）R-10 與 config 繞過耦合。** 現況 workaround 設 `galRoot=…\plugins\gal-core`。若 R-10 讓引擎在 galRoot 下**自動 append** `plugins/gal-core`，則 `galRoot=gal-core` 會雙重 append（`plugins/gal-core/plugins/gal-core`）而再次壞掉。**修正**：R-10 落地時必須同步把 config `galRoot` 還原為 repo root，且/或引擎容忍兩種形式（偵測 galRoot 是否已是 gal-core）。兩動作必須同一 commit。
- **[B-04]（中）「消費引擎、不重建」與實際範圍不符。** R-03（projection 目標）與 R-10（mode 解析）改的是 `gal-engine` **核心契約**（`providers/claude.rs`、`mode.rs`、`render.rs` —— 受保護路徑、bootstrap 擁有）。這不是薄下游計畫，而是重開剛關閉的 bootstrap 引擎核心。**修正**：把 R-03/R-10 明列為「引擎核心變更」，走 bootstrap 邊界 architect 審；本計畫對這兩項是 owner 而非 consumer，敘述要誠實。

#### Internal Consistency

- D-01 被當「已裁決事實」寫進 Goal/R-03/Success Criteria（symlink 就是活面），同時又在 P0/Risks 對沖（「可能無法載入則阻斷」）—— 自相矛盾。載入面在 P0 證實前是**假設**，不是前提。Goal/R-03/Success Criteria 須改成「P0 產出」而非「既定機制」。

#### Missing from Plan

- **Copilot/AGY 自有面**未具體化：計畫說「對 Copilot/AGY 同樣驗證自有面」，但 Copilot（manifest/host copy）、AGY（junction）機制各異，oracle 已有對應 target（`~/.copilot/installed-plugins…`、AGY junction）。P0 須逐 provider 列出，勿只做 Claude。
- **`gal update` 如何讓 Claude 重讀**：若採 `~/.claude/skills/gal` symlink → canonical root，skill 變更即時生效；但 agent 面（cache）需重裝/重載，計畫宣稱「即時生效」只對 symlink 面成立，agent 面未交代。與 B-02 同源。

#### What's Good（保留）

- 治理原則「活讀取面 == source、temp dir 對不算數」—— 正是前次 root cause 的正解，務必保留為驗收硬標準。
- R-02 目錄掃描非白名單 —— 正確，根除 dockeeper 被漏渲染那類 bug。
- P0 作為「不可載入就阻斷」的 gate —— 方向對；強化為**正式 go/no-go**，P1–P6 在 P0 證出可載入 agent 面前不得啟動。
- 消費 bootstrap 引擎、不重建 workspace（就「不改核心」的部分而言）—— 正確。
- atomic swap 後清 temp + doctor 活面檢查 —— 健全。

#### Recommended Changes

1. **改正 skill 收斂面**：`~/.claude/plugins/gal` → `~/.claude/skills/gal`（GAL 自有、非 legacy）。明確區分「skill 載入面」與「agent/subagent 載入面」為兩條獨立契約。
2. **解 B-02（阻斷，使用者裁決）**：在不採官方 `/plugin` 流程的前提下，golem-dockeeper 這類 subagent 沒有已知 GAL 自有載入面。三選一須由使用者拍板（見 focused question）：(a) agent 面例外接受官方 `claude plugin install`（skill 仍走 GAL 自有）；(b) 放棄「dockeeper 作為可載入 subagent」，agent 內容改以 skill 形態投影到 `~/.claude/skills/gal`；(c) P0 先做實機探測，若找到任何非官方 agent 載入面再定。
3. **P0 升為正式 go/no-go gate**：產出「每 provider 實際 agent/skill 載入面」實機證據；未證實前 P1–P6 凍結。
4. **R-10 加但書**：正規修法與 config `galRoot` 還原同 commit，避免雙重 append（B-03）。
5. **Goal/R-03/Success Criteria 去前提化**：載入面寫成 P0 待證假設。
6. **R-03/R-10 標為引擎核心變更**（B-04），走 bootstrap 邊界審。

<!-- ARCH_REVIEW: APPROVE -->

**Focused question（已解，2026-06-08）**：移除官方 marketplace/`/plugin` 後 subagent 無 GAL 自有載入面。使用者裁決：**agent 活載入「不在本計畫做、但不放棄」**，移入 Non-Goals 延至後續計畫。本計畫 skill 面（doc-sync）走 `~/.claude/skills/gal`；dockeeper 仍正確渲染進 canonical root。

### Engineering Review

**Verdict: CLEAR.** 13 個 T-NNN 對應 P0–P7 + R-10/R-11，各自獨立可完成可驗；14 條 TP 全數對映到 T-NNN，含 parity/intent/integration/manual 分型。範圍已由 architect REVISE→APPROVE 收斂（skill 面正確、agent 載入切出、引擎邊界誠實、script 真刪含 oracle reparent 守則）。可進 `/plan-to-prompt`。

**實作期約束（須在 prompt 與執行中遵守）：**

1. **T-001 是 go/no-go gate**：skill 面 `~/.claude/skills/gal` 若實機證實未被 Claude 載入，T-003 的投影目標需重評（不得退回官方流程），先回報。P1–P7 在 T-001 通過前不啟動。
2. **引擎核心 + 受保護路徑簽核**：T-002（`mode.rs`）、T-003（`providers/claude.rs`/`render.rs`/`install.rs`）、T-006（`doctor.rs`）改 bootstrap 擁有的 `gal-engine` 核心契約，屬受保護路徑。實作前須 bootstrap 邊界 architect 簽核（B-04），cross-model reviewer ≠ implementer。
3. **T-002 原子性**：galRoot 自動解析與本機 config `galRoot` 還原 repo root **必須同一 commit**，否則雙重 append 壞掉（B-03）；引擎須容忍 gal-core 舊形式。
4. **T-010 必須先於 T-011**：先把 `cross_platform_oracle_parity.rs`/`test-t022-ssh.sh` reparent 為 fixture，再刪 script，否則 parity 測試失依。monolithic script 須全部消費者 parity 才整檔刪。
5. **CODER ≠ TESTER ≠ REVIEWER**：依 model-roles，跨模型驗證；T-003/T-006 reviewer tier ≥ implementer。

**非阻斷（已具守則，不擋 CLEAR）**：上述 1–5 皆為執行紀律，計畫已在 R-10/R-11/P0/P7/Risks 寫明對應守則，無未解 planning 阻斷。

<!-- ENG_REVIEW: CLEAR -->
