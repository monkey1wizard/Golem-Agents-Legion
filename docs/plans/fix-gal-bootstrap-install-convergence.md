# 計畫：GAL Bootstrap / Install 收斂與 Rust 漸進取代

## Goal

讓 GAL 的安裝真相重新可信：使用者可以透過正式發布的 `gal` Rust binary 進行 bootstrap、install、update、doctor 與 package-manager 安裝。repo source 變更後，重跑一次 update 會確定性地重渲染 `~/.gal/plugins/gal/`，並讓 Claude、AGY、Copilot、Codex 等啟用 provider 的實際讀取面都看到同一份最新 GAL。完成後，Rust 成為穩定公開入口與逐步取代 scripts 的基底，舊 PowerShell/Bash scripts 則先作為被 Rust 管理的 legacy engine，後續再逐塊退休。

## Context

本計畫源自 2026-06-02 的 install truth 盤點。當前可觀測事實：

- `docs/plans/fix-install-ownership-stabilization.md`、`docs/plans/feat-plugin-arch-migration.md`、`docs/plans/feat-golem-dockeeper.md` 等 source plan 宣稱完成，但本機 runtime surfaces 未收斂。
- `~/.gal/plugins/gal/` 正本未包含 `golem-dockeeper` 或 `doc-sync`，代表 source 已完成但 canonical plugin root 未重渲染。
- `~/.gemini/antigravity-cli/plugins/gal` 不存在，AGY CLI 無法呼叫 GAL。AGY IDE / GUI-config surface 與 CLI surface 分歧。
- Claude 本機 plugin manager cache 仍是 `gal@gal` v1.0.0，不等同 public marketplace 發布完成。
- `.dev/state.md` 仍列出多個 terminal 或 stale plan，且多數 `.dev/plans/*.prompt.md` 已不存在。
- `scripts/Get-StagedCommitMessage.ps1` 與 `scripts/get-staged-commit-message.sh` 有 AGY migration hard-coded special case，導致 `git-commit-msg` 可能輸出固定舊訊息。
- `docs/devguide.md` 已有 GitHub Releases / winget / Homebrew 發布規格，但目前沒有真正 end-to-end package-manager install lane。
- `docs/plans/headless-cli-pipeline.md` 與 `.dev/plans/headless-cli-pipeline.prompt.md` 宣稱完成 / VERIFIED，但 2026-06-02 安全稽核只證明 echo mock、routing parser、prompt-path OFFLOAD、unavailable/timeout durable log 可用。尚未證明 claude/opencode/agy 真實 receipt，且 `.dev/state.md` 對該 plan 同時存在 DRAFT 與 VERIFIED 語意。
- Headless 稽核另發現 `scripts/common/New-TaskSpec.ps1` 對單一 `T-NNN` 產生的 `Affected Files` 範圍過寬，`scripts/executors/Invoke-Executor.ps1` 在未讀檔驗證寫回前即把 exit 0 記為 `completed`，source-plan dispatch 會靜默 fallback 而 prompt-path dispatch 才能 OFFLOAD。

Graphify advisory：`graphify-out/GRAPH_REPORT.md` 建於 commit `66d0cab0`，目前 HEAD 為 `7464692841f535843fdf13018212e56506d4c074`，因此本計畫不依賴該 graph 作硬性範圍判斷，只作「install/release/scripts/provider surfaces 為跨模組高耦合」的弱訊號。

## Requirements

- [ ] **R-001 Rust bootstrap 入口**：建立可發布的 Rust `gal` binary，提供 `gal --version`、`gal install`、`gal update`、`gal doctor`、`gal uninstall` 的穩定入口。Phase 1 可委派現有 scripts，但 CLI 引數、錯誤分類與 exit code 由 Rust 管理。
- [ ] **R-002 package-manager 真實安裝**：提供 GitHub Releases canonical artifacts，並讓 `winget install Monkey1Wizard.GAL` 與 `brew install monkey1wizard/tap/gal` 成為可驗證安裝路徑，而不是文件規格。
- [ ] **R-003 bootstrap ownership**：package-manager 安裝只安裝 bootstrap binary 與必要 release metadata，不預塞 `~/.gal/` runtime state。first-run bootstrap 由 `gal install` 建立 machine-local intent。
- [ ] **R-004 install/update spine 收斂**：Rust 入口呼叫的 install/update 必須共用同一條流程：解析 machine config → 重渲染 canonical root → 逐 provider 投影/刷新 → 寫 provider ledger → doctor 可驗證。
- [ ] **R-005 source mode 不可半收斂**：`installMode=source` 時也必須刷新 `~/.gal/plugins/gal/` 與所有 selected provider surfaces。不得只處理 Claude 或只靠 `Update-Skills` / `Update-Personalization` 副作用。
- [ ] **R-006 AGY 三 surface 修復**：AGY CLI、IDE、GUI-config 三個 plugin store 都必須存在 GAL，且盡可能 link-first 指向 `~/.gal/plugins/gal/`。CLI surface 缺失為本計畫的 P0 bug。
- [ ] **R-007 dockeeper/doc-sync 可見**：`golem-dockeeper` 與 `doc-sync` 必須從 repo source 渲染進 `~/.gal/plugins/gal/`，並被 Claude、AGY、Copilot 等實際讀取面發現。
- [ ] **R-008 Claude marketplace 誠實分類**：local plugin manager / skills-dir projection / public marketplace 三種狀態必須分開記錄。不得把本機 `gal@gal` cache 說成 public marketplace 發布完成。
- [ ] **R-009 plan lifecycle closeout**：盤點 completed / VERIFIED / implemented source plans，正式關閉已完成者，刪除或歸檔 orphaned execution prompts，並把仍需觀察的 follow-up 移到獨立追蹤文件。`.dev/state.md` 只保留真正 active plan。
- [ ] **R-010 git-commit-msg 修復**：移除 AGY migration hard-coded output，commit helper 必須以 staged diff / changed files 推導訊息。沒有 staged changes 時應明確回報，不可吐舊固定訊息。
- [ ] **R-011 Rust 漸進取代策略**：本計畫先讓 Rust 成為穩定入口與 package-manager payload，再逐步把 deterministic 子系統搬進 Rust。provider-specific orchestration 在 contract 穩定前可留在 scripts。
- [ ] **R-012 plugin-bin 邊界重訂**：更新 `plugin-bin-migration.md` 的前置與範圍，明確標示該 plan 只做 upstream `gal` binary 的 plugin `bin/` exposure。Rust bootstrap、package-manager payload、`gal-core` 與 deterministic engines 由本計畫或未來專項 plan 擁有。
- [ ] **R-013 雙 runtime 等價**：只要保留 scripts 作 legacy engine，就必須保持 PowerShell / Bash 對等或明確記錄環境限制。Rust 入口不得只包 Windows 路徑。
- [ ] **R-014 doctor 為 release gate**：`gal doctor` 必須能 read-only 檢查 canonical root、provider surfaces、provider ledgers、package-manager metadata、plan lifecycle state 與 known stale host caches。
- [ ] **R-015 headless executor truth closeout**：將 headless CLI pipeline 從「已完成」重新分類為「已實作但真實 provider receipt 未驗證」，修正或另開 follow-up 追蹤 claude/opencode/agy receipt、`Dispatch:` 寫回、Task Spec 範圍、`completed` terminal-state 語意與 `.dev/state.md` 收斂。

## Non-Goals

- 不在第一切片一次性用 Rust 重寫 Claude / AGY / Copilot / Codex 的 provider-specific orchestration。
- 不把 provider 目錄內 unknown / user-owned 檔案納入 GAL 所有權。
- 不把 public Claude marketplace submission 假裝成已完成。本計畫只建立可驗證 gate 與 release lineage。
- 不刪除 completed source plans 作為歷史證據。關閉後可從 active state 移除，但仍可保留於 `docs/plans/` 或移到後續整理流程。
- 不把 doc-sync 的語意判斷、文件正文改寫或 code→doc 首次關聯全自動 Rust 化。

## Scope Decision

| 項目 | 決定 | 理由 |
| --- | --- | --- |
| 優先序 | 本計畫為最高優先，先於 `plugin-bin-migration.md` 實作 | install truth 不可信時，先做 bin 只會包住壞狀態 |
| Rust 角色 | 先做 bootstrap CLI、package-manager payload、doctor、script delegation | 先建立可發布入口與強型別邊界，再逐步退休 scripts |
| Scripts 角色 | Phase 1 作為 legacy engine，被 Rust 呼叫與約束 | provider orchestration 脆弱且已有大量測試，先收斂 contract 再搬 |
| Package managers | winget / Homebrew 必須納入本計畫 acceptance gate | 使用者明確要求 install 必須提供正式 package-manager lane |
| Existing plans | 以完成 plan 作事實來源與反例，並在本計畫中做 active-state closeout | 修正「source plan 完成 ≠ runtime install 完成」的流程缺口 |
| Headless pipeline | 不再把 echo-only / OFFLOAD block 靜態檢查視為 provider dispatch 驗證 | 必須有真實 receipt / write-back proof 才可標 VERIFIED |

## Approach

### Step 1：建立 Rust bootstrap CLI skeleton

- **Files**: `Cargo.toml`, `crates/gal-cli/`, `crates/gal-core/`, `.gitignore`
- **What**: 初始化 Rust workspace。`gal-cli` 提供 `--version`、`install`、`update`、`doctor`、`uninstall`、`dispatch-script` 等穩定子命令。`gal-core` 放路徑解析、config schema、錯誤型別與 release metadata 模型。Phase 1 子命令可委派 `scripts/Install-GalPlugins.*`、`scripts/Setup-Machine.*`、`scripts/gal.*`。
- **Verify**: `cargo check`、`cargo test`、`cargo run -- --version`、`cargo run -- doctor --dry-run` 成功，缺少 legacy script 時 fail-loud。

### Step 2：定義 Rust → scripts legacy engine contract

- **Files**: `crates/gal-cli/src/`, `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh`
- **What**: Rust 負責解析使用者意圖與環境，將 install/update/uninstall/doctor 轉成明確 script invocation。scripts 回傳標準 exit code 與機器可解析 summary。禁止 Rust 靜默吞錯或讓 scripts 自由改寫未宣告狀態。
- **Verify**: 對 Windows PowerShell 與 Bash fallback 分別跑 dry-run，確認同一 Rust 子命令能找到正確 script，並保留工作目錄、環境變數與 exit code。

### Step 3：修正 source/install mode 的 canonical render spine

- **Files**: `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`, `scripts/Build-CorePlugin.ps1`, `scripts/build-core-plugin.sh`
- **What**: 不論 `installMode=source` 或 `installMode=install`，update 都必須重渲染 `~/.gal/plugins/gal/`，並對所有 selected providers 跑投影/刷新分支。移除只在 source mode 處理 Claude 的特殊收斂缺口。
- **Verify**: 在持久化隔離 home 內，先 install，再修改 source 中的 agent/skill marker，不帶 `-Force` 重跑 `gal update`，canonical root 與 provider surfaces 都包含 marker。

### Step 4：修復 AGY 三 surface 與 MCP 半殘路徑

- **Files**: `scripts/Build-CorePlugin.ps1`, `scripts/build-core-plugin.sh`, `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `scripts/Update-Mcp.ps1`, `scripts/update-mcp.sh`
- **What**: 將 AGY CLI、IDE、GUI-config surface 視為同一 provider 的三個 expected projections。`Update-Mcp.*` 不得在 plugin root 尚未完整建立時只創出 `mcp_config.json` 半殘目錄。MCP 寫入必須依賴完整 plugin root 或觸發 canonical render。
- **Verify**: `~/.gemini/antigravity-cli/plugins/gal`、`~/.gemini/antigravity-ide/plugins/gal`、`~/.gemini/config/plugins/gal` 都存在且含 `plugin.json`、`skills/`、`agents/`、`mcp_config.json`。`agy plugin validate` 可用時通過。

### Step 5：讓 dockeeper/doc-sync 進入所有讀取面

- **Files**: `scripts/Build-CorePlugin.ps1`, `scripts/build-core-plugin.sh`, `agent/golem-dockeeper.agent.md`, `skills/doc-sync/SKILL.md`, `agent/agents.md`
- **What**: 確認 core renderer 以目錄掃描或註冊表方式包含最新 agents/skills。若 renderer 使用舊 cache 或手列清單，改成 source-of-truth 掃描或顯式註冊驗證。`golem-dockeeper` 與 `doc-sync` 作為 acceptance marker。
- **Verify**: `~/.gal/plugins/gal/agents` 出現 `golem-dockeeper`，`~/.gal/plugins/gal/skills` 出現 `doc-sync`。Claude skills-dir、AGY CLI surface、Copilot installed plugin surface 皆可見。

### Step 6：實作 package-manager release lane

- **Files**: `scripts/Package-ReleaseArtifacts.ps1`, `scripts/package-release-artifacts.sh`, `docs/devguide.md`, `README.md`, `docs/manual.md`, `packaging/winget/`, `packaging/homebrew/`, release workflow files if present
- **What**: 產出 GitHub Releases canonical binary assets、fallback archives、`checksums.txt`、`checksums.txt.sig`、`artifact-manifest.json`。新增 winget manifest 與 Homebrew formula/tap 更新流程，所有下游都指向同一 release lineage。
- **Verify**: 本機 dry-run 可產生符合 `docs/devguide.md` release matrix 的 artifacts。winget/Homebrew manifest 指向 GitHub Release asset 與 sha256。`gal --version` 為 package-manager smoke test。

### Step 7：修正 Claude local / public marketplace 狀態模型

- **Files**: `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `docs/devguide.md`, `docs/manual.md`, `README.md`
- **What**: 將 Claude `skills-dir projection`、local marketplace cache、public community marketplace 三種狀態拆開。Local cache stale 應由 doctor 報告，不得作為 public marketplace 可用證據。
- **Verify**: `gal doctor` 能報告 Claude active read surface、local cache version、public marketplace gate。README/manual 不再把 local plugin manager 可見說成 public marketplace direct-install。

### Step 8：修復 `git-commit-msg`

- **Files**: `scripts/Get-StagedCommitMessage.ps1`, `scripts/get-staged-commit-message.sh`, `skills/git-commits/SKILL.md`, command prompt files if needed
- **What**: 移除 AGY migration hard-coded subject/bullets。改為先檢查 staged changes，無 staged changes 時輸出明確 no-op 訊息或非零 exit。有 staged changes 時以檔案與 diff 推導 Conventional Commit scope 與 subject。
- **Verify**: 無 staged changes 時不再輸出 `refactor(antigravity): switch gemini cli references to agy cli`。合成不同 staged diffs 時 subject 會依 diff 改變。

### Step 9：關閉 stale plans 與建立 follow-up observation log

- **Files**: `.dev/state.md`, `docs/plans/*.md`, `docs/observations/install-followups.md` or `docs/research/install-followups.md`
- **What**: 盤點 active plans：完成且 verified 者移出 Active Plans。仍需後續觀察但非 active work 的項目寫入 follow-up observation log。pending approval 的 plan 保留但排序低於本計畫。不得刪除仍作為 install 事實來源的 source plan。
- **Verify**: `.dev/state.md` 第一列為本計畫。terminal/stale plan 不再作為 active execution work。`/gal status` 不會指向不存在的 `.dev/plans/*.prompt.md`。

### Step 10：重訂 `plugin-bin-migration` 邊界

- **Files**: `docs/plans/plugin-bin-migration.md`, `.dev/state.md`
- **What**: 標明 Rust workspace / bootstrap CLI / package-manager payload 由本計畫交付。`plugin-bin-migration` 後續只聚焦把本計畫產出的 upstream `gal` binary 放進 plugin root `bin/` 並驗證 Claude Code PATH，不再擁有 `gal-core` deterministic engines 或 install truth。
- **Verify**: 兩個 plan 不再互相重疊或互相阻擋。`plugin-bin-migration` 的下一步只依賴本計畫輸出的 binary source contract。

### Step 11：Doctor 與 release gate 收斂

- **Files**: `crates/gal-core/`, `crates/gal-cli/`, `scripts/Test-InstallGalPlugins.ps1`, `scripts/Test-BuildProviderPlugins.ps1`, Bash test equivalents
- **What**: `gal doctor` 聚合 Rust 檢查與 legacy script doctor：canonical freshness、provider projection、ledger status、package-manager metadata、Claude marketplace classification、AGY surface integrity、plan lifecycle drift、commit helper health。
- **Verify**: 發布前 `gal doctor --release-gate` exit 0。故意移除 AGY CLI projection 或 stale dockeeper marker 時 exit non-zero 並指出修復面。

### Step 12：Headless executor closeout 與 follow-up 分流

- **Files**: `.dev/state.md`, `docs/plans/headless-cli-pipeline.md`, `.dev/plans/headless-cli-pipeline.prompt.md`, `scripts/executors/Invoke-Executor.ps1`, `scripts/common/New-TaskSpec.ps1`, `scripts/gal.ps1`, `docs/observations/install-followups.md` or `docs/research/install-followups.md`
- **What**: 將 headless pipeline 的完成狀態降回可驗證事實：保留已通過的 echo/mock 與 prompt-path OFFLOAD 證據，但把 claude/opencode/agy receipt、execution prompt `Dispatch:` 實寫、source-plan fallback、Task Spec 範圍過寬、`completed` terminal-state 過度宣稱列為未收斂 follow-up。若本計畫不直接修 headless，必須把它移出 active execution state 並在 follow-up observation log 中留下可重開 plan 的明確條目。
- **Verify**: `.dev/state.md` 不再同時把 headless plan 表示為 DRAFT 與 VERIFIED。`Invoke-Executor` 不再把未驗證寫回的 exit 0 記為 `completed`。`New-TaskSpec` 對單一 `T-NNN` 不再暴露整個 plan 的 file surface。真實 claude/opencode/agy receipt 未跑前不得標 VERIFIED。

## Files to Create or Modify

- `[CREATE]` `Cargo.toml` — Rust workspace root。
- `[CREATE]` `crates/gal-cli/` — user-facing `gal` binary。
- `[CREATE]` `crates/gal-core/` — shared path/config/release/doctor primitives。
- `[MODIFY]` `.gitignore` — 忽略 Rust `target/` 與 release scratch output。
- `[MODIFY]` `scripts/Install-GalPlugins.ps1` / `scripts/install-gal-plugins.sh` — 統一 source/install mode update spine、doctor、provider lifecycle。
- `[MODIFY]` `scripts/Build-ProviderPlugins.ps1` / `scripts/build-provider-plugins.sh` — selected provider projection truth。
- `[MODIFY]` `scripts/Build-CorePlugin.ps1` / `scripts/build-core-plugin.sh` — canonical root render 與 Rust binary integration。
- `[MODIFY]` `scripts/Update-Mcp.ps1` / `scripts/update-mcp.sh` — 避免 AGY plugin root 半殘 MCP 寫入。
- `[MODIFY]` `scripts/Get-StagedCommitMessage.ps1` / `scripts/get-staged-commit-message.sh` — 移除 hard-coded AGY migration output。
- `[MODIFY]` `scripts/Package-ReleaseArtifacts.ps1` / `scripts/package-release-artifacts.sh` — release artifact dry-run 與 canonical metadata。
- `[CREATE]` `packaging/winget/` — winget manifest source or generation templates。
- `[CREATE]` `packaging/homebrew/` — Homebrew formula/tap source or generation templates。
- `[MODIFY]` `README.md`, `docs/manual.md`, `docs/devguide.md` — install status、package-manager、marketplace truth。
- `[MODIFY]` `docs/plans/plugin-bin-migration.md` — 邊界重訂與前置關係。
- `[MODIFY]` `docs/plans/headless-cli-pipeline.md` and `.dev/plans/headless-cli-pipeline.prompt.md` — reclassify unverified headless receipt/state facts or link to follow-up。
- `[MODIFY]` `scripts/executors/Invoke-Executor.ps1`, `scripts/common/New-TaskSpec.ps1`, `scripts/gal.ps1` — if headless gaps are fixed in this plan instead of only logged。
- `[MODIFY]` `.dev/state.md` — active plan 排序與 closeout 收斂。
- `[CREATE]` `docs/observations/install-followups.md` or `[CREATE]` `docs/research/install-followups.md` — 非 active follow-up 觀察紀錄。

## Test Cases

- [ ] `cargo run -- --version` → 顯示 GAL version，exit 0。
- [ ] `cargo run -- doctor --dry-run` → 不修改檔案，列出 canonical root、selected providers、package-manager metadata 狀態。
- [ ] `gal update` in source mode → 重渲染 `~/.gal/plugins/gal/` 並投影所有 selected providers，不只 Claude。
- [ ] 修改 `agent/golem-dockeeper.agent.md` 或 `skills/doc-sync/SKILL.md` marker → `gal update` → canonical root 與 provider surfaces 都看到 marker。
- [ ] 移除 `~/.gemini/antigravity-cli/plugins/gal` → `gal update` → AGY CLI surface 被恢復。
- [ ] `Update-Mcp.*` 在 AGY plugin root 不存在時 → 不產生只有 `mcp_config.json` 的半殘 plugin root。
- [ ] Claude local cache stale → `gal doctor` 報告 stale cache，但不宣稱 public marketplace installed。
- [ ] 無 staged changes 執行 commit helper → 不輸出固定 AGY migration commit message。
- [ ] staged docs-only diff → commit helper 產生 docs/refactor 類型訊息，而非 AGY migration 訊息。
- [ ] release artifact dry-run → 產生 GitHub Releases canonical assets、archives、checksums、artifact manifest。
- [ ] winget manifest dry-run → `PackageIdentifier=Monkey1Wizard.GAL`，installer URL 與 sha256 指向 release artifact。
- [ ] Homebrew formula dry-run → `brew install monkey1wizard/tap/gal` 使用 release archive 並安裝 `bin/gal`。
- [ ] `.dev/state.md` closeout 後 → 不再指向不存在的 `.dev/plans/*.prompt.md` 作 active execution work。
- [ ] `plugin-bin-migration.md` 更新後 → Rust bootstrap scope 不再與本計畫重複。
- [ ] Headless plan closeout 後 → `.dev/state.md` 對 headless plan 不再同時呈現 DRAFT 與 VERIFIED。未驗證 provider receipt 被列入 follow-up。
- [ ] `Test-ExecutorReceipt.ps1 -Executor claude|opencode|agy` 未實跑成功前 → headless plan 不得宣稱真實 provider receipt passed。
- [ ] `Invoke-Executor.ps1` exit 0 without verified write-back → 不得在 durable log 中標成 `completed`。
- [ ] `New-TaskSpec.ps1 -TaskScope T-001` → `Affected Files` 只包含 task-relevant files，或明確標記仍需人工收斂，不可把整份 plan 的 file surface 當作 bounded spec。

## Success Criteria

- [ ] `gal` Rust binary 是正式 install/update/doctor 入口，可由 package-manager 安裝。
- [ ] winget、Homebrew、GitHub Releases 三條 install lane 共享同一 versioned binary lineage。
- [ ] `gal update` 能讓 repo source 變更傳播到 `~/.gal/plugins/gal/` 與所有 selected provider read surfaces。
- [ ] AGY CLI、AGY IDE、AGY GUI-config 三個 surface 都能找到完整 GAL plugin。
- [ ] `golem-dockeeper` 與 `doc-sync` 在 canonical root 和 provider read surfaces 可見。
- [ ] Claude local cache、skills-dir projection、public marketplace 三種狀態在 docs 與 doctor 中被誠實區分。
- [ ] `git-commit-msg` 不再吐固定 AGY migration 訊息。
- [ ] `.dev/state.md` 不再把 terminal plan 或 orphaned prompt 當 active execution work。
- [ ] `plugin-bin-migration.md` 改為 downstream plugin `bin/` exposure plan，只依賴本計畫輸出的 upstream binary source contract，而不是重做 install/bootstrap。
- [ ] Headless CLI pipeline 不再以 OFFLOAD block presence 或 echo-only smoke 作為 VERIFIED 證據。真實 provider receipt/write-back proof 缺口被修復或移入 follow-up observation log。

## Risks

- **範圍過大**：本計畫同時碰 Rust、install、provider surfaces、package managers、plan closeout。緩解：先經 `/deep-planning`，再由 `/refining-plan` 切成 P0 spine、provider repair、release packaging、state closeout 四個可獨立驗證切片。
- **過早 Rust rewrite 破壞 provider behavior**：直接用 Rust 重寫 provider orchestration 容易遺失已知 provider quirks。緩解：Phase 1 Rust 只做入口與 contract，provider orchestration 先留 legacy engine。
- **package-manager 發布外部審核延遲**：winget / Homebrew review 不一定同日完成。緩解：GitHub Releases 作 canonical fallback，doctor/docs 明確標 downstream lag。
- **Windows symlink/junction 權限差異**：AGY/Copilot link-first 可能受權限影響。緩解：doctor 分類 link vs host-copy，fallback 必須寫 ledger 並可重跑刷新。
- **plan closeout 誤刪歷史證據**：completed plans 仍是 install 事實來源。緩解：只移出 active state，不刪 source plan，follow-up 另檔記錄。
- **VERIFIED overclaim 重演**：headless pipeline 已出現 echo-only / static OFFLOAD checks 被升格為 provider dispatch verified 的情況。緩解：closeout 必須要求 live receipt 或明確降級為 follow-up。doctor/state check 不接受文字宣稱。
- **Claude public marketplace 混淆**：local plugin manager 成功可能再次被誤稱 public marketplace。緩解：release gate 必須檢查 marketplace classification wording。

## Open Questions

- [ ] OQ-001 — Release signing system 使用 GPG 還是 sigstore？`checksums.txt.sig` 需要先鎖定工具與金鑰管理方式。*(raised by: planning)*
- [ ] OQ-002 — Homebrew tap 是否使用 `monkey1wizard/tap` 既有 repo，或本 repo 先產生 formula template 由 release 手動同步？*(raised by: planning)*
- [ ] OQ-003 — Rust binary 的 first-run bootstrap 是否預設寫入 `installMode=install`，或在 source checkout 內偵測 repo root 時保留 `installMode=source`？*(raised by: planning)*
- [ ] OQ-004 — Completed source plans 關閉後是否留在 `docs/plans/`，或移到 dedicated archive 目錄？目前建議先留原位，只從 `.dev/state.md` 移除。*(raised by: planning)*
- [ ] OQ-005 — Public Claude marketplace submission 是否納入本計畫最後 release gate，或只建立 gate 並另開 marketplace publication plan？目前建議只建立 gate，submission 另案。*(raised by: planning)*

## Approval

- Human approval: [pending]
- Architect review: [required — Rust bootstrap, package-manager distribution, protected install scripts, and active-state lifecycle all cross structural boundaries]
- Security review: [required before implementation — install/update/uninstall, package-manager payload, release signing, provider projections, and path cleanup affect trust boundaries]
- Additional domain review: [release review required before package-manager publication]

## Review Results

### Architecture Review

Pending.

### Security Review

Pending.

### Release Review

Pending.

### Engineering Review

Pending.

## Test Plan

Pending. `/refining-plan` must turn the test cases above into ordered TP IDs with explicit PowerShell, Bash, Rust, package-manager dry-run, and local runtime-surface checks.

## Tasks

Pending. `/refining-plan` must split implementation into at least these slices: Rust bootstrap skeleton, legacy script contract, source/install update spine, AGY/provider projection repair, dockeeper/doc-sync propagation, package-manager artifacts, commit helper fix, headless executor truth closeout, plan closeout, and release/doctor gates.
