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
- `scripts/Get-StagedCommitMessage.ps1` 與 `scripts/get-staged-commit-message.sh` 有 AGY migration content-keyword special case：`Test-AgyMigration` 只要 staged diff 同時出現 `gemini` 與 `antigravity`/`agy` 字樣，就強制輸出固定 subject `switch gemini cli references to agy cli`、scope `antigravity` 與三條固定 bullets。**真正的失效模式不是「無 staged changes 時吐固定訊息」**（無 staged changes 已正確回 `No changes staged for commit.`），而是任何正常提及這兩個字的 staged diff（含本計畫檔本身）都會被 hijack 成 AGY migration 訊息。
- `docs/devguide.md` 已有 GitHub Releases / winget / Homebrew 發布規格，但目前沒有真正 end-to-end package-manager install lane。
- `docs/plans/headless-cli-pipeline.md` 與 `.dev/plans/headless-cli-pipeline.prompt.md` 宣稱完成 / VERIFIED，但 2026-06-02 安全稽核只證明 echo mock、routing parser、prompt-path OFFLOAD、unavailable/timeout durable log 可用。尚未證明 claude/opencode/agy 真實 receipt，且 `.dev/state.md` 對該 plan 同時存在 DRAFT 與 VERIFIED 語意。
- Headless 稽核另發現 `scripts/common/New-TaskSpec.ps1` 對單一 `T-NNN` 產生的 `Affected Files` 範圍過寬，`scripts/executors/Invoke-Executor.ps1` 在未讀檔驗證寫回前即把 exit 0 記為 `completed`，source-plan dispatch 會靜默 fallback 而 prompt-path dispatch 才能 OFFLOAD。

Graphify advisory：`graphify-out/GRAPH_REPORT.md` 建於 commit `66d0cab0`，目前 HEAD 為 `7464692841f535843fdf13018212e56506d4c074`，因此本計畫不依賴該 graph 作硬性範圍判斷，只作「install/release/scripts/provider surfaces 為跨模組高耦合」的弱訊號。

## Requirements

- [ ] **R-001 Rust bootstrap 入口**：建立可發布的 Rust `gal` binary，提供 `gal --version`、`gal install`、`gal update`、`gal doctor`、`gal uninstall` 的穩定入口。Phase 1 可委派現有 scripts，但 CLI 引數、錯誤分類與 exit code 由 Rust 管理。
- [ ] **R-002 package-manager 真實安裝**：提供 GitHub Releases canonical artifacts，並讓 `winget install Monkey1Wizard.GAL` 與 `brew install monkey1wizard/tap/gal` 成為可驗證安裝路徑，而不是文件規格。
- [ ] **R-003 bootstrap ownership**：package-manager 安裝只安裝 bootstrap binary 與必要 release metadata，不預塞 `~/.gal/` runtime state。first-run bootstrap 由 `gal install` 建立 machine-local intent，預設為一般模式。

> **模式模型（取代舊 `installMode` 命名，OQ-003 決議）**
>
> 權威訊號在 `~/.gal/config/config.json`：
> - **一般模式（normal）**：預設。`"devMode"` 不存在或為 `false`。從已安裝/打包的 source 渲染。package-manager 安裝走這條。
> - **開發者模式（dev mode）**：`"devMode": true` **且** `"galRoot"` usable。從 `galRoot` 指向的 repo working tree 渲染。
> - `devMode:true` 但 `galRoot` 不 usable → **報明確錯誤（指出哪項失敗）、exit 非零，不靜默 fallback** 到一般模式。
> - first-run 在 source checkout 內不自動切換；開發者顯式設 `devMode:true` + 有效 `galRoot` 後才以 repo 渲染。
>
> **`galRoot` usable predicate**（全過才算 usable）：
> 1. `galRoot` 非空字串。
> 2. 解析後路徑存在且為目錄（PS `Test-Path -PathType Container`／Bash `[ -d ]`）。
> 3. 可讀（能列出內容；dead UNC/網路路徑以短逾時快速失敗，不卡住）。
> 4. 是 GAL source checkout：同時含 `commands/`、`agent/`、`skills/`（缺一即不算，避免把任意目錄當 galRoot）。
>
> **`installMode` 欄位棄用**：模式解析只看 `devMode` + `galRoot`，不再讀 `installMode`（消除 devMode/installMode/galRoot 三欄漂移，落實 BUG-01）。migration 時一次性由舊 `installMode` 推導 `devMode`；之後 doctor 對殘留且與 `devMode` 矛盾的 `installMode` 發 warning。
- [ ] **R-004 install/update spine 收斂**：Rust 入口呼叫的 install/update 必須共用同一條流程：解析 machine config → 重渲染 canonical root → 逐 provider 投影/刷新 → 寫 provider ledger → doctor 可驗證。
- [ ] **R-005 dev mode 不可半收斂**：開發者模式（dev mode）下 update 也必須刷新 `~/.gal/plugins/gal/` 與所有 selected provider surfaces。不得只處理 Claude 或只靠 `Update-Skills` / `Update-Personalization` 副作用。
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

切片原則：每個 step 只 refactor 一個功能或一個 script 的單一行為，diff 小、可獨立驗證。Rust init 獨立成 P0-R，不阻擋任何 P0 script 修復。

### Slice 排序與依賴

| Slice | 目標 | 前置 | 可平行 |
| --- | --- | --- | --- |
| **P0-R** | Rust bootstrap skeleton（僅 `--version` + dispatch 骨架） | 無 | 與所有 P0-n 平行 |
| **P0-0** | 模式解析單一來源（devMode + galRoot，棄用 installMode） | 無 | render 步驟（P0-1/2/3）前置 |
| **P0-1..6** | runtime install truth，純 script 單一功能修復 | P0-1/2/3 依 P0-0 | 彼此平行（同檔修改注意 merge 順序） |
| **P1-1..2** | Rust↔script 委派契約 + doctor 聚合骨架 | P0-R；P1-2 另需 P0-1/3/5 | — |
| **P2-1..4** | package-manager release lane | **OQ-001 簽章決策、Security review**；P2-* 另需 P0-R | — |
| **P3-1..3** | state / plan / headless closeout（純 docs/state） | 無 | 隨時 |
| **P4-1** | release gate 收斂 | P1-2、P2-* | 最後 |

P0 各 step 之間無功能相依，可任意順序或平行。唯一例外：`Build-CorePlugin.*` 被 P0-1、P0-3、P0-5 各改一個功能分支，三者實作時須循序 rebase 避免衝突，但驗證仍各自獨立。

---

### P0-R：Rust bootstrap skeleton（獨立）

- **Files**: `Cargo.toml`, `crates/gal-cli/`, `crates/gal-core/`, `.gitignore`
- **What**: 初始化 workspace。`gal-cli` 提供 `--version` 與子命令 enum（`install`/`update`/`doctor`/`uninstall`/`dispatch-script`），此階段子命令僅做引數解析與 exit-code 分類，未接線者回明確 `not wired` 訊息。**不 parse machine config**（BUG-01）。`gal-core` Phase 1 僅含 path 解析、子命令 enum、exit-code 型別三項，**不含 release metadata model**（OE-02）。
- **Verify**: `cargo check`、`cargo test`、`cargo run -- --version`、`cargo run -- --help` 列出子命令；未接線子命令回 `not wired` 非靜默成功。

### P0-0：模式解析單一來源（單一功能，render 前置）

- **Files**: `scripts/common/Common.ps1`（`Get-ConfiguredInstallModeFromContext`/`Test-InstallModeFromContext`）, `scripts/common/common.sh`, `scripts/install-gal-plugins.sh`（config 寫入處）
- **What**: 模式解析改為唯一權威 = `config.json` `"devMode":true` + `galRoot` usable predicate（定義見 R-003）。停止讀 `installMode` 做決策；migration 一次性由舊 `installMode` 推導 `devMode`。新增 `Test-GalRootUsable`（PS）與 Bash 對等：非空 + 目錄 + 可讀(短逾時) + 含 `commands/`+`agent/`+`skills/`。
- **Verify**: `devMode:true` + 有效 `galRoot` → 解析為 dev mode。`devMode:true` + galRoot 不存在/缺契約目錄/dead path → 非零 exit + 指出失敗項，不 fallback。`devMode` 缺/false → 一般模式。殘留矛盾 `installMode` 不影響決策（僅 doctor warning）。PS 與 Bash 對等。

### P0-1：canonical render mode 收斂（單一功能，依賴 P0-0）

- **Files**: `scripts/Build-CorePlugin.ps1`, `scripts/build-core-plugin.sh`（僅 render-mode 分支）
- **What**: 一般模式與開發者模式（dev mode）都重渲染 `~/.gal/plugins/gal/`；dev mode 以 P0-0 解析出的 `galRoot` 為來源。render 必須 atomic（temp dir + swap）或冪等可重跑。移除只在 dev mode 處理 Claude 的收斂缺口。
- **Verify**: 隔離 home install 後改 source marker，不帶 `-Force` 重跑 update → canonical root 含 marker。kill-mid-render 後重跑仍收斂、無半渲染殘留。

### P0-2：provider projection 真相（單一功能）

- **Files**: `scripts/Build-ProviderPlugins.ps1`, `scripts/build-provider-plugins.sh`
- **What**: 所有 selected providers 都跑投影/刷新分支，不可只投 Claude。
- **Verify**: install 多 provider 後改 source 重跑 update → 每個 selected provider surface 都刷新含 marker。

### P0-3：AGY 三 surface 交易性修復（單一功能）

- **Files**: `scripts/Build-CorePlugin.ps1`, `scripts/build-core-plugin.sh`, `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`（僅 AGY projection 分支）
- **What**: AGY CLI/IDE/GUI-config 視為同一 provider 的三個 expected projections，並以交易處理：全成功或全回滾並寫 ledger 標降級面（BUG-03）。link-first，symlink/junction 權限失敗時 fallback host-copy 並寫 ledger。
- **Verify**: `~/.gemini/antigravity-cli/plugins/gal`、`~/.gemini/antigravity-ide/plugins/gal`、`~/.gemini/config/plugins/gal` 都存在含 `plugin.json`/`skills/`/`agents/`/`mcp_config.json`。刪 CLI surface 後 update 恢復。模擬 symlink 失敗 → 不留半投影、ledger 記 host-copy。`agy plugin validate` 可用時通過。

### P0-4：Update-Mcp 半殘修復（單一 script）

- **Files**: `scripts/Update-Mcp.ps1`, `scripts/update-mcp.sh`
- **What**: plugin root 尚未完整建立前不得只創 `mcp_config.json` 半殘目錄。MCP 寫入須依賴完整 plugin root 或觸發 canonical render。
- **Verify**: AGY plugin root 不存在時跑 `Update-Mcp.*` → 不產生只有 `mcp_config.json` 的半殘 root。

### P0-5：dockeeper/doc-sync 掃描完整性（單一功能）

- **Files**: `scripts/Build-CorePlugin.ps1`, `scripts/build-core-plugin.sh`（僅 agent/skill 列舉分支）；`agent/golem-dockeeper.agent.md`、`skills/doc-sync/SKILL.md`、`agent/agents.md` 作 acceptance marker
- **What**: renderer 改以目錄掃描或註冊表納入最新 agents/skills，移除舊 cache 或手列清單。
- **Verify**: `~/.gal/plugins/gal/agents` 出現 `golem-dockeeper`、`~/.gal/plugins/gal/skills` 出現 `doc-sync`，且 Claude skills-dir、AGY CLI surface、Copilot installed plugin surface 皆可見。

### P0-6：git-commit-msg keyword-hijack 修復（單一 script pair）

- **Files**: `scripts/Get-StagedCommitMessage.ps1`, `scripts/get-staged-commit-message.sh`, `skills/git-commits/SKILL.md`
- **What**: 移除 `Test-AgyMigration` content-keyword short-circuit（PS line 56-58、105-107、144-146、206-210，Bash 對等）。empty-staging 路徑已正確，不動。scope/subject/bullets 一律由實際 changed files 與 diff 結構推導；若保留 rename/migration 偵測，須以 changed-path 證據（實際改名/移動）而非 diff 內文關鍵字觸發。
- **Verify**: 內文含 `gemini`+`agy` 但無實際 rename 的 diff → 不輸出 `switch gemini cli references to agy cli`，scope 非 `antigravity`。empty-staging 仍回 `No changes staged for commit.`。

### P1-1：Rust↔scripts 委派契約（依賴 P0-R）

- **Files**: `crates/gal-cli/src/`, `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh`
- **What**: Rust 把 `install`/`update`/`uninstall`/`doctor` 轉成明確 script invocation。**scripts 持有 config/path 真相，Rust 不重算**（BUG-01）——Rust 只傳使用者意圖（子命令 + flags）與透明 passthrough。scripts 回標準 exit code + 機器可解析 summary。禁止 Rust 靜默吞錯。
- **Verify**: PS 與 Bash 各跑 dry-run，同一子命令找到正確 script 並保留 cwd/env/exit code。

### P1-2：doctor 聚合骨架（依賴 P0-R、P0-1/3/5）

- **Files**: `crates/gal-core/`, `crates/gal-cli/`, `scripts/Test-InstallGalPlugins.ps1`, `scripts/Test-BuildProviderPlugins.ps1`, Bash test 對等
- **What**: `gal doctor` read-only 聚合：canonical freshness、provider projection、ledger status、AGY surface integrity、commit helper health、plan lifecycle drift。定義 exit-code 分級（warning 不擋、error 擋 release）。package-manager / marketplace 檢查待 P2 後於 P4 併入。
- **Verify**: `doctor --dry-run` 不改檔；故意移除 AGY CLI projection 或 stale dockeeper marker → error exit 並指出修復面。

### P2-1：release artifact dry-run（依賴 OQ-001、Security review）

- **Files**: `scripts/Package-ReleaseArtifacts.ps1`, `scripts/package-release-artifacts.sh`, `docs/devguide.md`
- **What**: 產出 GitHub Releases canonical binary assets、fallback archives、`checksums.txt`、`artifact-manifest.json`。簽章用 **cosign keyless**（OQ-001）：CI 以 GitHub Actions OIDC 對 `checksums.txt` 簽章，產出 `.sig` + cert（或 cosign bundle）並推 Rekor。本機 dry-run 無 OIDC 時以明確 placeholder 標記，不假造簽章。
- **Verify**: 本機 dry-run 產生符合 `docs/devguide.md` release matrix 的 artifacts；CI 路徑 `cosign verify-blob` 對 `checksums.txt` 通過（驗 workflow identity + Rekor）。

### P2-2：winget manifest（依賴 P2-1）

- **Files**: `packaging/winget/`, `docs/devguide.md`
- **What**: 新增 winget manifest source/generation，指向 GitHub Release asset 與 sha256。
- **Verify**: winget manifest dry-run → `PackageIdentifier=Monkey1Wizard.GAL`，installer URL 與 sha256 指向 release artifact。

### P2-3：Homebrew formula/tap（依賴 P2-1、OQ-002）

- **Files**: `packaging/homebrew/`, `docs/devguide.md`
- **What**: 本 repo 先產生 Homebrew formula **template**（OQ-002 決議），指向同一 release lineage，release 後手動同步到 tap。不假設 `monkey1wizard/tap` 既有內容。
- **Verify**: Homebrew formula dry-run → `brew install monkey1wizard/tap/gal` 使用 release archive 並安裝 `bin/gal`。

### P2-4：Claude local/public marketplace 狀態模型（單一功能）

- **Files**: `scripts/Install-GalPlugins.ps1`, `scripts/install-gal-plugins.sh`, `docs/devguide.md`, `docs/manual.md`, `README.md`
- **What**: 拆開 Claude `skills-dir projection`、local marketplace cache、public community marketplace 三種狀態。local cache stale 由 doctor 報告，不得當 public marketplace 證據。
- **Verify**: doctor 報告 Claude active read surface / local cache version / public marketplace gate。README/manual 不再把 local 可見說成 public direct-install。

### P3-1：stale plans closeout（純 state/docs）

- **Files**: `.dev/state.md`, `docs/plans/*.md`, `docs/observations/install-followups.md` or `docs/research/install-followups.md`
- **What**: 完成且 verified 的 plan 移出 Active Plans；仍需觀察的非 active 項目寫入 follow-up log；pending approval plan 保留但排序低於本計畫。不刪仍作 install 事實來源的 source plan。同時移除 `.dev/state.md` 重複/矛盾的 plan 區段。
- **Verify**: `.dev/state.md` 第一列為本計畫，無重複區段。`/gal status` 不指向不存在的 `.dev/plans/*.prompt.md`。

<!-- ========================================================================
  PLAN CLOSEOUT CANDIDATES (OQ-004) — user 手動處理，本計畫不自動刪除
  決議：completed plan 留在 docs/plans/ 原位，只從 .dev/state.md Active Plans 移除。
  狀態盤點自 .dev/state.md (2026-06-02)。

  [可移出 Active + 其 .dev/plans/*.prompt.md 可刪]（verified/complete，非 install 事實來源）：
   - feat-golem-dockeeper            VERIFIED  ※ 等 P0-5 驗證 dockeeper/doc-sync 可見後再刪 prompt
   - fix-gal-pipeline-token-burn     VERIFY 完成，數值目標達成
   - feat-ai-plan-language-strategy  IMPLEMENT 完成(T-009)，等 verifier closeout 後
   - refactor-docs-restructure       IMPLEMENTED (Slices 1–5)，等 verification/closeout 後
   - feat-plugin-arch-migration      VERIFIED ※ 屬 install 架構事實來源，建議「移出 Active 但 source plan 留作 reference」

  [保留，勿刪]（install 事實來源 / 待本計畫處理 / 仍 active）：
   - fix-install-ownership-stabilization  install truth 主要來源；本計畫多個 P0 step 以其為反例
   - headless-cli-pipeline                P3-3 reclassify（VERIFIED 為 overclaim，R-015）；勿刪
   - plugin-bin-migration                 P3-2 邊界重訂；active
   - feat-pdf-chandra-upgrade             無關 active plan
   - fix-gal-bootstrap-install-convergence 本計畫

  [孤兒，待 user 確認]（不在 state.md Active Plans，可能 stale）：
   - feat-gal-file-memory-strategy   docs + prompt 都在，但未列 Active
   - manage-external-plugins         僅 docs，無 prompt，未列 Active
======================================================================== -->

### P3-2：plugin-bin-migration 邊界重訂（純 docs）

- **Files**: `docs/plans/plugin-bin-migration.md`, `.dev/state.md`
- **What**: 標明 Rust workspace / bootstrap CLI / package-manager payload 由本計畫交付。`plugin-bin-migration` 只聚焦把本計畫產出的 upstream `gal` binary 放進 plugin root `bin/` 並驗證 Claude Code PATH，不擁有 `gal-core` engines 或 install truth。
- **Verify**: 兩 plan 不再重疊或互擋；`plugin-bin-migration` 下一步只依賴本計畫的 binary source contract。

### P3-3：headless executor closeout（純 state/docs + 可選 script 修復）

- **Files**: `.dev/state.md`, `docs/plans/headless-cli-pipeline.md`, `.dev/plans/headless-cli-pipeline.prompt.md`, `scripts/executors/Invoke-Executor.ps1`, `scripts/common/New-TaskSpec.ps1`, `scripts/gal.ps1`, follow-up log
- **What**: 把 headless 完成狀態降回可驗證事實：保留 echo/mock 與 prompt-path OFFLOAD 證據，但把真實 claude/opencode/agy receipt、`Dispatch:` 實寫、source-plan fallback、Task Spec 範圍過寬、`completed` terminal-state 過度宣稱列為 follow-up。若不直接修，須移出 active state 並在 follow-up log 留可重開條目。`Invoke-Executor` exit-0 不得在未驗證寫回時記 `completed`（與 P0-6 同源教訓：弱訊號≠強證據）。
- **Verify**: `.dev/state.md` 不再同時把 headless plan 表示為 DRAFT 與 VERIFIED。真實 provider receipt 未跑前不得標 VERIFIED。`New-TaskSpec` 對單一 `T-NNN` 不再暴露整個 plan 的 file surface。

### P4-1：release gate 收斂（最後，依賴 P1-2、P2-*）

- **Files**: `crates/gal-core/`, `crates/gal-cli/`
- **What**: `gal doctor --release-gate` 在 P1-2 骨架上併入 package-manager metadata、Claude marketplace classification 檢查，聚合所有面成單一 gate。
- **Verify**: 發布前 `gal doctor --release-gate` exit 0；故意移除 AGY CLI projection、stale dockeeper marker、或缺 package-manager metadata 時 exit non-zero 並指出修復面。

## Files to Create or Modify

- `[CREATE]` `Cargo.toml` — Rust workspace root。
- `[CREATE]` `crates/gal-cli/` — user-facing `gal` binary。
- `[CREATE]` `crates/gal-core/` — shared path/config/release/doctor primitives。
- `[MODIFY]` `.gitignore` — 忽略 Rust `target/` 與 release scratch output。
- `[MODIFY]` `scripts/common/Common.ps1` / `scripts/common/common.sh` — 模式解析改 `devMode`+`galRoot` 權威、棄用 `installMode`、新增 `Test-GalRootUsable` 對等（P0-0）。
- `[MODIFY]` `scripts/Install-GalPlugins.ps1` / `scripts/install-gal-plugins.sh` — 統一一般/dev mode update spine、config 寫入改 devMode 推導、doctor、provider lifecycle。
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
- [ ] staged diff 內文含 `gemini`+`agy` 但無實際檔案 rename → commit helper 不輸出 `switch gemini cli references to agy cli`，scope 不被強制為 `antigravity`。
- [ ] 無 staged changes 執行 commit helper → 回 `No changes staged for commit.`（既有行為，回歸保護）。
- [ ] staged docs-only diff → commit helper 產生 docs 類型訊息，而非 AGY migration 訊息。
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

- **範圍過大**：本計畫同時碰 Rust、install、provider surfaces、package managers、plan closeout。緩解：已於 deep-planning 切成 P0-R（Rust init 獨立）+ P0-1..6（runtime install truth 單一功能 script 修復）+ P1（Rust 契約/doctor）+ P2（release packaging）+ P3（state/plan closeout）+ P4（release gate），每 step 單一功能、可獨立驗證，P0 spine 先行。
- **過早 Rust rewrite 破壞 provider behavior**：直接用 Rust 重寫 provider orchestration 容易遺失已知 provider quirks。緩解：Phase 1 Rust 只做入口與 contract，provider orchestration 先留 legacy engine。
- **package-manager 發布外部審核延遲**：winget / Homebrew review 不一定同日完成。緩解：GitHub Releases 作 canonical fallback，doctor/docs 明確標 downstream lag。
- **Windows symlink/junction 權限差異**：AGY/Copilot link-first 可能受權限影響。緩解：doctor 分類 link vs host-copy，fallback 必須寫 ledger 並可重跑刷新。
- **plan closeout 誤刪歷史證據**：completed plans 仍是 install 事實來源。緩解：只移出 active state，不刪 source plan，follow-up 另檔記錄。
- **VERIFIED overclaim 重演**：headless pipeline 已出現 echo-only / static OFFLOAD checks 被升格為 provider dispatch verified 的情況。緩解：closeout 必須要求 live receipt 或明確降級為 follow-up。doctor/state check 不接受文字宣稱。
- **Claude public marketplace 混淆**：local plugin manager 成功可能再次被誤稱 public marketplace。緩解：release gate 必須檢查 marketplace classification wording。

## Open Questions

- [x] OQ-001 — Release signing **鎖定 sigstore/cosign keyless**（GitHub Actions OIDC 身分簽，無長期私鑰）。簽章保護 GitHub Releases 直接下載路徑；winget/Homebrew 另靠 manifest sha256。Security review 只需確認 CI OIDC 信任設定（允許的 workflow identity / Rekor 透明日誌驗證），不再選工具。*(resolved — cosign keyless)*
- [x] OQ-002 — **本 repo 先產生 Homebrew formula template，release 後手動同步**（user 之後手改）。P2-3 產 `packaging/homebrew/` template，不假設 `monkey1wizard/tap` 既有內容。*(resolved)*
- [x] OQ-003 — 權威 = `~/.gal/config/config.json` 的 **`"devMode":true` + `galRoot` usable**；一般模式為預設（`devMode` 缺/false）。`installMode` 欄位棄用。`galRoot` usable predicate 與棄用細節見 R-003，解析實作見 P0-0。*(resolved)*
- [x] OQ-004 — **completed plan 留在 `docs/plans/` 原位，只從 `.dev/state.md` 移除**。本計畫於 P3-1 下方加 closeout-candidate 區塊註解供 user 手動處理。*(resolved)*
- [x] OQ-005 — **只建 release gate，public Claude marketplace submission 另開計畫**。P4-1 gate 只檢查 marketplace classification wording，不含 submission。*(resolved — gate only)*

## Approval

- Human approval: [pending]
- Architect review: [REVISE → 收斂中。OE-01/OE-02/BUG-01(步驟層)/BUG-02/失敗恢復已於切片解決；OQ-001..005 已決議。仍待 `/refining-plan`：R-004 措辭、PS/Bash 對等 TP、doctor exit-code 契約、產 ## Tasks/## Test Plan。完成後重跑 architect gate 轉 CLEAR。詳見 ## Review Results > ### Architecture Review]
- Security review: [required before implementation — install/update/uninstall, package-manager payload, release signing, provider projections, and path cleanup affect trust boundaries]
- Additional domain review: [release review required before package-manager publication]

## Review Results

### Architecture Review

**Verdict: REVISE** —方向正確、推理紮實，但不可作為單一可驗證 plan 進 `/plan-to-prompt`。必須先由 `/refining-plan` 切片並修掉以下 blocking 項，architect gate 才轉 CLEAR。

#### Trade-off Summary

| 決策 | 效益 | 成本 | 裁決 |
| --- | --- | --- | --- |
| Phase 1 Rust 只做入口 + 委派 scripts | 取得可發布 binary 與強型別 CLI 邊界，package manager 需要 binary 非 scripts | 多一層；若 gal-core 複製 config/path schema 即出現雙真相 | REVISE（見 BUG-01） |
| 保留 PS+Bash 作 legacy engine | 不丟已知 provider quirks 與既有測試 | 每次 script 改動現在要驗 PS+Bash+Rust 契約三面 | OK（須在 R-013 明列 3x 成本） |
| 同一 plan 涵蓋 Rust/包管/provider 修復/closeout/headless | 一次盤清 install truth | 12 steps×15 reqs 無法作單一切片獨立驗證 | REVISE（見 OE-01） |
| winget+Homebrew 納入 acceptance gate | 滿足使用者「正式 package-manager lane」要求 | 受外部審核延遲，signing 未定 | REVISE（見 BUG-02） |

#### Over-engineering / Scope Flags

- **[OE-01] 單體 plan 範圍過大**：本 plan 同時是 greenfield Rust workspace、release 工程、provider 修復、兩個 state closeout。這不是 over-abstraction，而是 over-scope——沒有任一中間點能宣告「可驗證完成」。`/refining-plan` **必須**切成 Risks 已點名的 4 個獨立可驗證切片，且明確排序：**(1) P0 spine**（Step 3 source/install 收斂 + Step 4 AGY CLI 修復 + Step 5 dockeeper/doc-sync + Step 8 commit-msg）→ **(2) Rust bootstrap + 契約**（Step 1-2）→ **(3) release packaging**（Step 6-7 + Step 11 doctor）→ **(4) state/headless closeout**（Step 9-10、12）。理由：P0 是純 script 修復、零 Rust 相依、立即可驗 runtime truth；Rust/包管/closeout 都可在 P0 綠燈後並行或串接，不該被 Rust skeleton 阻塞。
- **[OE-02] gal-core Phase 1 不要鋪 release metadata model**：Phase 1 binary 只需 `--version` + 子命令 dispatch 到 script。release metadata / artifact-manifest schema 在 Step 6 才真正被消費。提前在 gal-core 定義會在契約穩定前固化型別。Phase 1 gal-core 應壓到：path 解析、子命令 enum、exit-code 分類三項。

#### Bug Surface

- **[BUG-01] 高] config/path 雙解析漂移**：R-004 要 Rust「解析 machine config」，Step 3 的 scripts 也各自解析 machine config 與 path。若兩邊都 parse，兩個真相來源必然漂移（Rust 認為 selected providers = A，script 自行重算 = B）。**修正**：明定單一 owner——Phase 1 由 **scripts 續持有** config/path 真相，Rust 只傳「使用者意圖（子命令 + flags）」與不重算的透明 passthrough；Rust **不得** parse machine config 內容。R-004 的「解析 machine config」職責應改述為「呼叫 script 取得已解析 config 摘要」。未澄清前 Step 1-2 不可實作。
- **[BUG-02] 已解** release signing：OQ-001 鎖定 **cosign keyless**。P2-1 用 CI OIDC 簽 `checksums.txt`，P4-1/R-014 doctor 以 `cosign verify-blob` 驗。Security review 只確認 CI 信任設定（允許的 workflow identity + Rekor），非 blocking 工具選擇。
- **[BUG-03] 中 AGY 三 surface link-first 的部分失敗原子性**：Step 4 要對 CLI/IDE/GUI-config 三個 store 投影並盡量 symlink。Windows symlink/junction 權限差異（Risks 已點名）會造成「三投影中一兩個成功」的半收斂——比目前「CLI 完全缺失」更難診斷。**修正**：Step 4 須定義三投影為一個交易：全成功或全回滾＋寫 ledger 標記降級面，doctor（Step 11）必須能逐 surface 報 link/host-copy/missing，而非只報 plugin root 是否存在。
- **[BUG-04] 中 `completed` terminal-state 與 commit-msg 修復共用「內容關鍵字 ≠ 證據」教訓**：Step 12 的 `Invoke-Executor` exit-0-記-completed 與 Step 8 的 commit-msg keyword-hijack 是**同一類 bug**：把弱訊號（exit code / diff 內文字樣）當成強證據（寫回驗證 / 實際 rename）。建議在 plan 層級把「terminal/decision state 必須由結構性證據而非關鍵字或 exit code 推導」列為一條跨 step 的 invariant，避免 reviewer 各 step 重複發現。

#### Missing from Plan

- **回滾/失敗恢復未述** — ✅ **已解**：P0-1 已定 render 為 atomic（temp dir + swap）或冪等可重跑，並加 kill-mid-render 後重跑仍收斂的 Verify；P0-3 三投影交易性 + ledger 降級。
- **doctor 與 release-gate 的 exit-code 契約未定** — 🟡 **設計已定，TP 待落**：P1-2 已定「warning 不擋 / error 擋 release」分級；具體 exit-code TP 由 `/refining-plan` 落成（見下方 required #3）。
- **跨 runtime 驗證矩陣缺位** — 🟡 **結構已備，矩陣待寫**：R-013 要求 PS/Bash 對等，P0 各 step 已列 PS+Bash 檔；逐 TP 的 PS/Bash 對等矩陣由 `/refining-plan` 落成（見下方 required #2）。

#### What's Good (keep these)

- **以「source plan 完成 ≠ runtime install 完成」為核心問題框架**正確且稀有——多數計畫不會回頭稽核自己宣稱完成的 plan。Step 9/12 的 closeout 直接針對已驗證的 `.dev/state.md` 重複/DRAFT-VERIFIED 矛盾。
- **Phase 1 Rust 不重寫 provider orchestration**是正確的風險控制，守住 YAGNI。
- **拒絕把 echo-only / OFFLOAD 靜態檢查當 VERIFIED**（R-015、Risks VERIFIED overclaim）是必要的誠實 gate。
- **commit-msg 真實失效模式已在本次 review 校正**（內容關鍵字 hijack，非 empty-staging），Context/Step 8/Test Cases 已同步修正。

#### Required before architect CLEAR

已於本次切片解決（structural）：
- ✅ **OE-01**：Approach 已切成 P0-R / P0-1..6 / P1 / P2 / P3 / P4，每 step 單一功能/script，P0 spine 先行、Rust init 獨立 P0-R。
- ✅ **OE-02**：P0-R 明列 gal-core Phase 1 僅 path/enum/exit-code，不鋪 release metadata model。
- ✅ **BUG-01（步驟層）**：P0-R 與 P1-1 已明定 Rust 不 parse machine config、scripts 持有 config 真相。
- ✅ **BUG-02**：P2-1 已標 signing block 在 OQ-001 + Security review。
- ✅ **失敗恢復**：P0-1 atomic/kill-mid-render、P0-3 三投影交易性 + ledger 降級已入 step What/Verify。

仍須在 `/refining-plan` 完成才轉 CLEAR：
1. **BUG-01 措辭**：改寫 R-004「解析 machine config」→「呼叫 script 取得已解析 config 摘要」，與 P0-R/P1-1 一致。
2. **Test Plan PS/Bash 對等矩陣**：每個 P0 script 修復同列 PS 與 TP，強制 R-013 對等。
3. **doctor exit-code 契約**：P1-2/P4-1 的 warning-不擋 / error-擋 分級落成具體 TP。
4. 產出 `## Tasks`（依 P0-R..P4-1 切 T-NNN）與 `## Test Plan`（目前仍 placeholder）。

### Security Review

Pending.

### Release Review

Pending.

### Engineering Review

**Verdict: CLEAR**

實作契約已鎖定：18 個單一功能 T-NNN 對應 1:1 Approach slice，43 條 TP 對齊並涵蓋 architect 的三項 `/refining-plan` 必辦——(1) 每個 script 修復 PS + Bash 對等 TP；(2) doctor exit-code 分級 TP（TP-030）；(3) `galRoot` usable predicate / 模式解析 TP（TP-001..005）。依賴排序明確且可獨立驗證：P0-R 與 P0 群可平行、P0-1/2/3 依 T-002、P1 依 P0-R、P2 依 Security review、P4 收尾。架構性 blocking（OE-01 切片、OE-02 gal-core 限縮、BUG-01 config 單一真相、BUG-02 cosign keyless、失敗恢復 atomic/交易性）皆已 bake 進步驟。

非 blocking residuals（`/plan-to-prompt` 時順手對齊，不擋實作）：

- **R-004 措辭**：`## Requirements` R-004 仍寫「解析 machine config」。權威行為以 T-002/T-009 為準（scripts 持有 config 真相、Rust 不重算、只傳意圖）。建議 prompt 生成時把 R-004 改述為「呼叫 script 取得已解析 config 摘要」。屬文字一致性，不改變實作契約。
- **外部 gate（非工程缺口）**：T-011 之 `.sig` 依 Security review 確認 CI OIDC 信任設定；package-manager 上架受外部審核延遲（GitHub Releases 為 canonical fallback，doctor 標 downstream lag）。

切片粒度與測試矩陣足以開始實作。建議首切片 T-002（模式解析單一來源）與 T-001（Rust skeleton）並行，再進 T-003+ render 群。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

所有 runtime-surface / script 檢查在持久化隔離 home 內跑。script 修復一律 PS + Bash 對等（R-013）。

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | unit | PS：`galRoot` 有效（目錄 + 可讀 + 含 `commands`/`agent`/`skills`）+ `devMode:true` → 解析 dev mode | T-002 |
| TP-002 | unit | Bash：同 TP-001 對等 | T-002 |
| TP-003 | unit | `devMode:true` 但 galRoot 不存在 / 非目錄 / 缺契約目錄 / dead UNC → 非零 exit + 指出失敗項，不 fallback（PS + Bash） | T-002 |
| TP-004 | unit | `devMode` 缺或 false → 一般模式（PS + Bash） | T-002 |
| TP-005 | unit | 殘留 `installMode` 與 `devMode` 矛盾 → 不影響決策，僅 doctor warning | T-002, T-010 |
| TP-006 | unit | `cargo check` + `cargo test` 通過 | T-001 |
| TP-007 | manual | `cargo run -- --version` → 顯示 GAL version，exit 0 | T-001 |
| TP-008 | manual | `cargo run -- --help` 列出子命令；未接線子命令回 `not wired` 非靜默成功 | T-001 |
| TP-009 | integration | PS：隔離 home install 後改 source marker，不帶 `-Force` 重跑 update → canonical root 含 marker | T-003 |
| TP-010 | integration | Bash：同 TP-009 對等 | T-003 |
| TP-011 | integration | kill-mid-render 後重跑 → 收斂、無半渲染殘留（PS + Bash） | T-003 |
| TP-012 | integration | PS：多 provider install 後改 source 重跑 update → 每個 selected provider surface 都刷新含 marker | T-004 |
| TP-013 | integration | Bash：同 TP-012 對等 | T-004 |
| TP-014 | integration | `~/.gemini/antigravity-cli|antigravity-ide|config/plugins/gal` 三路徑都含 `plugin.json`/`skills`/`agents`/`mcp_config.json`（PS + Bash） | T-005 |
| TP-015 | integration | 刪 AGY CLI surface → 重跑 update → 恢復 | T-005 |
| TP-016 | integration | 模擬 symlink/junction 權限失敗 → 不留半投影、ledger 記 host-copy（交易回滾） | T-005 |
| TP-017 | integration | PS：AGY plugin root 不存在時跑 `Update-Mcp` → 不產生只有 `mcp_config.json` 的半殘 root | T-006 |
| TP-018 | integration | Bash：同 TP-017 對等 | T-006 |
| TP-019 | integration | canonical root `agents/` 出現 `golem-dockeeper`、`skills/` 出現 `doc-sync` | T-007 |
| TP-020 | integration | dockeeper/doc-sync 於 Claude skills-dir、AGY CLI surface、Copilot installed plugin surface 皆可見 | T-007 |
| TP-021 | unit | PS：diff 內文含 `gemini`+`agy` 但無實際 rename → 不輸出 `switch gemini cli references to agy cli`，scope 非 `antigravity` | T-008 |
| TP-022 | unit | Bash：同 TP-021 對等 | T-008 |
| TP-023 | unit | 無 staged changes → 回 `No changes staged for commit.`（回歸保護，PS + Bash） | T-008 |
| TP-024 | unit | staged docs-only diff → docs 類型訊息，非 AGY migration | T-008 |
| TP-025 | integration | PS dry-run：同一子命令 → 正確 script，保留 cwd/env/exit code | T-009 |
| TP-026 | integration | Bash dry-run：同 TP-025 對等 | T-009 |
| TP-027 | unit | Rust 委派路徑不讀 machine config 內容（只傳意圖 + passthrough） | T-009 |
| TP-028 | manual | `gal doctor --dry-run` 不修改任何檔 | T-010 |
| TP-029 | integration | 移除 AGY CLI projection 或 stale dockeeper marker → doctor error exit 並指出修復面 | T-010 |
| TP-030 | unit | doctor exit-code 分級：warning-only finding → exit 0；error finding → 非零 | T-010 |
| TP-031 | integration | release artifact 本機 dry-run → GitHub Releases canonical assets、archives、`checksums.txt`、`artifact-manifest.json`（符合 devguide matrix，PS + Bash） | T-011 |
| TP-032 | manual | CI 路徑 `cosign verify-blob` 對 `checksums.txt` 通過（驗 workflow identity + Rekor） | T-011 |
| TP-033 | unit | 本機 dry-run 無 OIDC → 簽章為明確 placeholder，不產生假 `.sig` | T-011 |
| TP-034 | unit | winget manifest dry-run → `PackageIdentifier=Monkey1Wizard.GAL`，installer URL + sha256 指向 release artifact | T-012 |
| TP-035 | unit | Homebrew formula template dry-run → `brew install monkey1wizard/tap/gal` 用 release archive 並安裝 `bin/gal` | T-013 |
| TP-036 | integration | Claude local cache stale → doctor 報 stale，不宣稱 public marketplace installed | T-014 |
| TP-037 | manual | README/manual 不再把 local plugin manager 可見說成 public marketplace direct-install | T-014 |
| TP-038 | manual | `.dev/state.md` 第一列為本計畫、無重複區段；`/gal status` 不指向不存在的 `.dev/plans/*.prompt.md` | T-015 |
| TP-039 | manual | `plugin-bin-migration.md` 不再與本計畫重疊；下一步只依賴本計畫 binary source contract | T-016 |
| TP-040 | manual | `.dev/state.md` 不再同時把 headless plan 表示為 DRAFT 與 VERIFIED | T-017 |
| TP-041 | unit | `New-TaskSpec -TaskScope T-001` → `Affected Files` 只含 task 相關檔，不暴露整份 plan file surface | T-017 |
| TP-042 | unit | `Invoke-Executor` exit 0 但未驗證寫回 → 不在 durable log 記 `completed` | T-017 |
| TP-043 | integration | `gal doctor --release-gate` 發布前 exit 0；移除 AGY CLI projection / stale dockeeper / 缺 package-manager metadata → 非零 + 指出修復面 | T-018 |

## Tasks

每個 T-NNN 對應一個 Approach slice（單一功能/script），可獨立完成與驗證。排序依「Slice 排序與依賴」表。script 修復一律 PS + Bash 對等。

**P0 — runtime install truth + Rust init（先行，P0-R 與 P0 群可平行；P0-1/2/3 依 T-002）**

- [ ] T-001 — [P0-R] 建立 Rust workspace（`Cargo.toml`/`crates/gal-cli`/`crates/gal-core`/`.gitignore`）：`--version` + 子命令 enum，僅引數解析 + exit-code 分類，未接線回 `not wired`；不 parse machine config；gal-core 僅 path/enum/exit-code 三型別。
- [ ] T-002 — [P0-0] 模式解析改唯一權威 `devMode:true` + `galRoot` usable，棄用 `installMode` 決策；新增 `Test-GalRootUsable`（PS）+ Bash 對等（非空/目錄/可讀短逾時/含 `commands`+`agent`+`skills`）；migration 由舊 `installMode` 一次性推導 `devMode`。改 `Common.ps1`/`common.sh`/`install-gal-plugins.sh`。
- [ ] T-003 — [P0-1] `Build-CorePlugin.*` render-mode 分支：一般與 dev mode 都重渲染 canonical root（dev mode 以 T-002 解析的 `galRoot` 為來源）；render atomic（temp+swap）或冪等；移除只在 dev mode 處理 Claude 的缺口。
- [ ] T-004 — [P0-2] `Build-ProviderPlugins.*`：所有 selected providers 都跑投影/刷新，不只 Claude。
- [ ] T-005 — [P0-3] AGY CLI/IDE/GUI-config 三投影交易化：全成功或全回滾並寫 ledger；link-first，權限失敗 fallback host-copy 並記 ledger。改 `Build-CorePlugin.*`/`Install-GalPlugins.*` AGY 分支。
- [ ] T-006 — [P0-4] `Update-Mcp.*`：plugin root 未完整建立前不得只創 `mcp_config.json` 半殘目錄；MCP 寫入依賴完整 plugin root 或觸發 canonical render。
- [ ] T-007 — [P0-5] `Build-CorePlugin.*` agent/skill 列舉改目錄掃描/註冊表，移除手列清單；`golem-dockeeper`/`doc-sync` 作 acceptance marker。
- [ ] T-008 — [P0-6] 移除 `Get-StagedCommitMessage.*` 的 `Test-AgyMigration` content-keyword short-circuit；scope/subject/bullets 由 changed files + diff 結構推導；rename 偵測改以 changed-path 證據。

**P1 — Rust 契約 + doctor 骨架（依 P0-R；T-010 另依 T-003/005/007）**

- [ ] T-009 — [P1-1] Rust 把 install/update/uninstall/doctor 轉成明確 script invocation；scripts 持有 config/path 真相、Rust 不重算只傳意圖 + 透明 passthrough；scripts 回標準 exit code + 機器可解析 summary；禁止 Rust 靜默吞錯。
- [ ] T-010 — [P1-2] `gal doctor` read-only 聚合：canonical freshness、provider projection、ledger、AGY surface integrity、commit helper health、plan lifecycle drift；定義 exit-code 分級（warning 不擋 / error 擋 release）。

**P2 — package-manager release lane（依 Security review；P2-* 另依 P0-R）**

- [ ] T-011 — [P2-1] `Package-ReleaseArtifacts.*` 產出 GitHub Releases canonical assets、archives、`checksums.txt`、`artifact-manifest.json`；簽章用 cosign keyless（CI OIDC 簽 `checksums.txt` + Rekor）；本機無 OIDC 時 placeholder 不假造 `.sig`。
- [ ] T-012 — [P2-2] `packaging/winget/` manifest source/generation，指向 release asset + sha256。
- [ ] T-013 — [P2-3] `packaging/homebrew/` formula **template**，指向同一 release lineage，release 後手動同步 tap。
- [ ] T-014 — [P2-4] 拆開 Claude skills-dir projection / local marketplace cache / public marketplace 三狀態；改 `Install-GalPlugins.*` + docs；stale cache 由 doctor 報告不當 public 證據。

**P3 — state / plan / headless closeout（純 docs/state，隨時可做）**

- [ ] T-015 — [P3-1] `.dev/state.md` closeout：completed/verified 移出 Active、移除重複/矛盾區段、本計畫置頂；非 active 觀察寫入 `docs/observations/install-followups.md`；不刪 install 事實來源 source plan。
- [ ] T-016 — [P3-2] 重訂 `plugin-bin-migration.md` 邊界：Rust workspace/bootstrap/payload 歸本計畫；該 plan 只做 upstream `gal` binary 的 plugin `bin/` exposure + Claude Code PATH。
- [ ] T-017 — [P3-3] headless closeout：state 不再同時 DRAFT/VERIFIED；未驗證 provider receipt 列 follow-up；`Invoke-Executor` exit-0 未驗證寫回不得記 `completed`；`New-TaskSpec` 單一 `T-NNN` 不暴露整份 plan file surface。

**P4 — release gate（最後，依 T-010 + P2-*）**

- [ ] T-018 — [P4-1] `gal doctor --release-gate` 在 T-010 骨架併入 package-manager metadata + Claude marketplace classification，聚合所有面為單一 gate。
