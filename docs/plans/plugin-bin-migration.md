# Plan: Plugin Bin Migration (Rust Implementation)

## Goal

讓 GAL 的 Claude-compatible plugin root 提供 Claude Code 官方 `bin/` 行為：外掛啟用時，agent 的 Bash tool 可直接呼叫 `gal` 公開 entrypoint，不需前綴 `bash`/`pwsh`。捨棄 `bash/ps1` 轉發層（thin wrapper）的妥協方案，改以 **Rust** 開發控制平面的原生二進位，提供跨平台穩定性與強型別引數解析，並為逐步將 agent-runtime 確定性硬邏輯原生化鋪路。

**分期與「零依賴」語義**：「直譯器無關 / Zero-dependency」是 **end-state，非 Phase 1 即達成**。

- **Phase 1（wrapper dispatch）**：`gal` bin 進 PATH、移除 `bin/gal.sh`/`bin/gal.ps1` 轉發層、外掛 `bin/` 只含原生執行檔。但 bin 內部仍委派 `scripts/gal.*`，故直譯器（pwsh/bash）仍須在場——使用者不需「鍵入」前綴，不等於機器「不需安裝」。
- **End-state**：唯有 agent-runtime 確定性引擎（候選 A–D）原生化、bin 不再委派 scripts 後，才真正零執行期依賴。

## Decisions (Locked)

| # | 決策 | 結論 | 日期 |
| --- | --- | --- | --- |
| OQ-001 | Rust 專案目錄 | `crates/` workspace：`crates/gal-cli`（外殼）+ `crates/gal-core`（解析/不變式核心），對外編譯單顆 `gal` bin | 2026-06-01 |
| OQ-002 | 立刻原生化 init/setup？ | 否。Phase 1 僅引數分派 wrapper；安裝/設定編排續留 `scripts/` | 2026-06-01 |
| OQ-003 | 無 Rust 環境 fallback？ | 否。Dev 自理 Rust；renderer 為每台機器本機 `cargo build`，無需 CI matrix；多平台預編譯分發 out-of-scope | 2026-06-01 |
| OQ-004 | crate / bin 邊界 | **單一 `gal` bin + `gal-core` 共用 crate**，候選 A–D 皆為 `gal` 子命令（依轉換範圍盤點：bin 候選量體小且內聚） | 2026-06-01 |
| OQ-005 | 候選原生化時序 | `golem-dockeeper` 已進行中（驅動候選 A）；候選 A binize 待其 AI 版穩定後啟動；候選 B（Tier 1）不必等 A，可獨立先行 | 2026-06-01 |
| OQ-007 | 候選 C/D 是否獨立開工 | 否。待 `gal-core` 完成後順帶加 `gal preflight` / `gal clock` 子命令 | 2026-06-01 |

唯一未決項見 `## Open Questions`（OQ-006，Phase-2 施工細節、不阻斷）。

## References

- Prerequisite stabilization: [fix-install-ownership-stabilization.md](fix-install-ownership-stabilization.md) now owns the idempotent `Build-CorePlugin.*` / install-lifecycle stabilization that must land before this plan adds the Rust cargo-build hook to the renderer.
- [Claude Code plugins](https://code.claude.com/docs/en/plugins) - plugin root `bin/` 中的 executables 在外掛啟用時加入 Bash tool 的 PATH。
- [Claude Code plugins-reference](https://code.claude.com/docs/en/plugins-reference) - 官方規格實測（2026-06）：`bin/` 為固定慣例資料夾「Executables added to the Bash tool's PATH … invokable as bare commands while the plugin is enabled」；**`plugin.json` manifest schema 無 `bin`/`platform`/`arch`/`executable` 欄位，亦無任何跨 OS 二進位選擇機制**——多平台分發由發佈者自理。版本以 `plugin.json:version` 或（省略時）git commit SHA 解析。`${CLAUDE_PLUGIN_ROOT}` 為安裝目錄絕對路徑、更新後變動、不可寫狀態。
- [docs/devguide.md](../devguide.md) - Claude plugin root 目前是 `~/.gal/plugins/gal/`。
- [scripts/scripts.md](../../scripts/scripts.md) - 現有 public 與 internal scripts 清單（轉換範圍盤點來源）。
- [conventions/token-budget.md](../../conventions/token-budget.md) - 須將 Rust `target/` 納入 exclusion，並確保 `bin/` 為實際外掛輸出。
- [docs/plans/feat-golem-dockeeper.md](feat-golem-dockeeper.md) - dockeeper 只交付 agent + skill（無 bin）；其確定性引擎為候選 A。
- [commands/gal-status/SKILL.template.md](../../commands/gal-status/SKILL.template.md)、[commands/gal-pipeline/SKILL.template.md](../../commands/gal-pipeline/SKILL.template.md) - 控制平面狀態投影與「三面收斂閘」目前以散文指令要求 LLM 機械解析 markdown，是候選 B 的來源。
- [conventions/working-hours.md](../../conventions/working-hours.md) - 工時邊界判定（候選 D）。
- [docs/collaborative-tools/checking-contract.md](../collaborative-tools/checking-contract.md) - 能力車道 preflight 五態（候選 C）。
- [graphify-out/2026-05-21_2/GRAPH_REPORT.md](../../graphify-out/2026-05-21_2/GRAPH_REPORT.md) - Community 8「GAL Core State & Plan Management」（`Get-ActivePlanPath`/`Get-StateContext`/`Resolve-PlanPath`/`Unwrap-MarkdownCode`）佐證狀態解析已是內聚的確定性子系統。

## Requirements

- [ ] 初始化 Rust Cargo workspace（`crates/gal-cli` + `crates/gal-core`）作為控制平面進入點。建議 `clap`（引數解析）+ `anyhow`（錯誤）。
- [ ] 透過 `cargo build` 產出單一二進位：`bin/gal`（Unix，含執行位元）與 `bin/gal.exe`（Windows）。
- [ ] 實作指令路由：Phase 1 為絕對穩定的強型別 wrapper，正確解析引數並委派現有 `scripts/`。中長期逐步將 **agent-runtime 確定性引擎（候選 A–D）** 原生化。**安裝/設定編排（`init`/`setup-machine`/`sync`/MCP 合併/adapter 生成）不原生化、續留 `scripts/`**（安裝期一次性、直譯器必在場；見「轉換範圍盤點」）。
- [ ] 本機 plugin renderer（`Build-CorePlugin.*`）打包時執行 `cargo build --release`，將本機 OS 二進位複製到 plugin root 的 `bin/`。
- [ ] 移除先前提議的 `bin/gal.sh` 與 `bin/gal.ps1` 轉發腳本需求。
- [ ] 更新 docs 與 command contracts，確立 Rust 編譯出的 `gal` 為公開入口點。

## Approach

### Step 1: Initialize Rust CLI Workspace
- **Files**: `Cargo.toml`（workspace）、`crates/gal-cli/`、`crates/gal-core/`
- **What**: 初始化 workspace。`gal-cli` 為對外二進位外殼（`clap` 子命令），`gal-core` 為共用核心（解析/不變式，供候選 A–D 重用）。
- **Verify**: `cargo check` 與 `cargo build` 成功編譯出 `gal` 執行檔。

### Step 2: Implement Dispatch Logic
- **Files**: `crates/gal-cli/src/`
- **What**: 實作 Phase 1 分派。`gal status` 等指令安全跨 OS 呼叫 `scripts/gal.* status`，正確處理環境變數、跳脫字元、退出碼。`clap` 的 `TrailingVarArg` 須正確轉傳。
- **Verify**: `cargo run -- status` 等同直接執行 `scripts/gal.ps1 status`。

### Step 3: Render Binary into the Plugin Artifact
- **Files**: `scripts/Build-CorePlugin.ps1`、`scripts/build-core-plugin.sh`
- **What**: GAL renderer 為**每台機器本機**產出 plugin root（非分發預編譯樹），故 render 時 `cargo build --release` 只產出**該機 OS** 的單一二進位，複製到 `bin/`：Unix 為無副檔名 `gal`（+x），Windows 為 `gal.exe`。`bin/` 依官方規格**不需在 `plugin.json` 宣告**。**不需 cross-compile / CI matrix**（每台機器各自 render 本機 OS binary）。對 `Build-CorePlugin.*` 的改動**僅限新增此建置掛鉤**（cargo build + 複製 + Unix `chmod +x`），不重寫其既有編排語義。
- **Verify**: 本機 render 出的 plugin 資料夾含對應本機 OS 的原生執行檔，且能被 Claude Code Bash tool 透過 PATH 直接執行。
- **Verify（fail-loud）**: render 時若 `cargo` 不在 PATH，`Build-CorePlugin.*` 須以**清楚錯誤訊息**中止（指向 OQ-003：renderer 本機即需 Rust 工具鏈），不得產出缺 `bin/gal` 的半成品 plugin。

### Step 4: Update Documentation & Command Contracts
- **Files**: `scripts/scripts.md`、`README.md`、`README.zh-Hant.md`、`docs/devguide.md`、`commands/gal/SKILL.template.md` 等
- **What**: 說明公開介面已轉移至 Rust 二進位，修改先前的系統要求措辭。
- **Verify**: 搜尋文件確保不再教導使用者以 `scripts/` 為首要互動入口。

## Native-Logic Migration Candidates

可逐步原生化（Rust）的硬邏輯候選，作為「將確定性硬邏輯原生取代」的 backlog。候選來自兩個來源：

1. **`golem-dockeeper`**（見 [feat-golem-dockeeper.md](feat-golem-dockeeper.md)）——只交付 agent + skill + NDJSON 契約、刻意無 bin；其確定性引擎為候選 A。
2. **scripts 以外的散文式確定性邏輯**——目前以散文指令要求 LLM 在執行期機械推導的確定性邏輯，最大宗在控制平面命令 skill 與 conventions：plan/state 收斂（B）、preflight（C）、工時（D）。

**NEED-for-bin 門檻**：須同時滿足 (a) 確定性/不變式、非語意判斷；(b) LLM 反覆執行不可靠或會違反不變式；(c) 須在 runtime × OS 矩陣普遍運作而無普遍直譯器可依賴。不滿足者留在 agent/skill。各候選一律比照 dockeeper 紀律：**先上 AI 執行版並通過測試，binize 為後續最佳化、非前置條件**。

**優先序**：B（Tier 1）> A（dockeeper 驅動，進行中）> C（Tier 2）> D（Tier 3）> E（依賴 headless executor + `gal-core` 就緒，最後啟動）。Phase 1（wrapper dispatch）不含任何候選，保持 thin。

### 轉換範圍盤點（bin 數量的決策依據）

盤點 `scripts/`（PS1 canonical ~11.6k 行 / 32 檔，bash 鏡像 ~8.2k 行 / 27 檔）+ scripts 外散文邏輯，依「是否為 agent-runtime 確定性 bin 候選」分群：

| 群組 | 代表檔（PS1 行數） | 小計 | bin 候選？ |
| --- | --- | --- | --- |
| **安裝/設定編排** | Update-Mcp(1707)、Install-GalPlugins(717)、Setup-Tools(626)、Build-CorePlugin(588)、ProviderPlugin(518)、Update-Commands(344)、Update-Skills(306)、Update-Personalization(272)、Resolve-GalCatalog(271)、Sync-DevContext(208)、Setup-Machine(205)、Init-Repo(169)、Build-ProviderPlugins(165)、Package-ReleaseArtifacts(97)、Uninstall-Machine(40) | **~6,233** | **否**——安裝期一次性、互動執行、直譯器必在場；跨 5 runtime 的 symlink/junction/MCP/adapter OS 整合原生化成本巨大、執行期效益近零 |
| 開發測試腳本 | Test-*.ps1（380/219/147/107/105/65） | ~1,023 | **否**——測 scripts 本身 |
| gal dispatcher | gal.ps1(797) | 797 | Phase 1 wrapper 取代其分派角色 |
| 共用 lib | Common.ps1(804) | 804（混合） | **部分**——僅 Community 8 狀態切片（6 函式）屬候選 B；其餘屬安裝期 |
| git commit / filter | Get-StagedCommitMessage(228)、gal-smudge/clean.sh | ~228 | **否**——commit msg 屬語意；filter 綁 git config |
| **xmachine 執行車道** | Invoke-XmachinePipeline(510)、Invoke-XmachineTask(472)、Start-xMachine(298)、Get-XmachineRemoteResult(190)…（Test-Xmachine 763 為測試） | ~1,760 核心 | **候選 E**——**部署對象不同**（遠端節點），是唯一可能正當的第二顆 bin 驅動 |

**結論**：看似「檔案很多」的量體（~6.2k 行、過半）幾乎全在安裝/設定編排，而那**不是 bin 候選**。真正的 agent-runtime 確定性 bin 候選（A/B/C/D）量體小且內聚——A（doc-sync）與 D（工時）目前甚至無對應 scripts。扣除安裝/設定後，**單一 `gal` bin + 子命令完全可行**（git/cargo 承載遠多於此）。

**「排除」≠「不碰」（安裝腳本邊界澄清）**：安裝/設定編排被列為**非 bin 候選**，指的是其**編排邏輯不被 Rust 原生取代**（續留 PowerShell/bash），**不**代表 Phase 1 完全不觸碰任何安裝腳本。Phase 1 仍會**修改一支** render/build 腳本——`Build-CorePlugin.*`（屬上表「安裝/設定編排」群組，588 行）——但僅為**最小整合掛鉤**：插入 `cargo build --release` 並把本機 OS 二進位複製進 plugin root 的 `bin/`。這是「放置編譯產物的接縫」，與「把安裝編排邏輯改寫成 Rust」是兩件事。判準：Phase 1 對安裝腳本的改動**只增建置掛鉤、不重寫編排語義**；任何要求改寫 symlink/junction/MCP 合併/adapter 生成語義的需求，仍受架構升級柵欄（OE-01）擋下、退回 `/deep-planning`。

### 為什麼這些要原生化（跨 runtime × OS 矩陣）

GAL 須在 **Claude、ChatGPT/Codex、Gemini/Antigravity、opencode × Windows/Linux/macOS** 下運作。此矩陣下無任何直譯器普遍保證：`python3` 在 Windows 不保證、`bash` 在 Windows 不原生、`pwsh` 在 Mac/Linux 不原生。唯一「零執行期依賴、跨 OS、跨 AI runtime」且能被各 runtime 的 Bash tool 直接呼叫的形式，就是編譯後的原生二進位。這是採 Rust bin 的核心理由（不是效能，是直譯器無關性）。

### 候選 A：doc-sync 引擎（dockeeper 驅動，進行中）

純機械/不變式邏輯，LLM 反覆讀寫最不可靠：

| 候選操作 | 為何適合 bin |
| --- | --- |
| `git diff -M --name-status` 解析與 A/D/R/M 分類 | 純解析，跨平台需穩定輸出 |
| NDJSON 逐行 schema 驗證 | 守「AI 把 NDJSON 寫歪」；不變式由程式保證 |
| NDJSON 原子局部更新（換行/append/刪行/穩定排序） | LLM 對大檔排序與精準改行不可靠 |
| 節點生命週期套用（改名靠 `-M` 辨識） | 規則固定、可單元測試 |
| 新鮮度不變式（只更新本次核對節點，禁整批翻 `ok`） | 不變式用程式碼強制 |
| 覆蓋掃描 / 投影渲染 | 機械列舉 + 確定性輸出 |

建議子命令：`gal doc-sync validate / apply / classify-diff / coverage / render-tree / render-todo`。
**邊界（留 AI）**：M 節點 `drift→ok` 確認、code→doc 初次關聯、改寫文件正文。
**時序**：dockeeper 先以 AI 執行版上線並通過測試後，才逐項換成 bin 呼叫。

### 候選 B：控制平面 plan/state 解析與收斂引擎（Tier 1）

`/gal status`、`/gal whats-next`、`/gal pipeline` 目前以散文指令要求 LLM 在執行期機械解析 repo-owned markdown 狀態檔。純解析 + 不變式，pipeline 已對其違反硬性 STOP，但偵測卻靠 LLM 肉眼讀 markdown。與候選 A 同類。

| 候選操作 | 來源 | 為何適合 bin |
| --- | --- | --- |
| `.dev/state.md` Active Plans 表解析 + 主 plan 解析 | gal-status Step 1 | 解析表、判定 terminal 相、解析相對路徑 |
| source plan ↔ prompt 路徑配對 | gal-status / pipeline Step 1 | 固定規則 |
| `T-NNN` / `OQ-NNN` 勾選計數 | gal-status | 純計數，LLM 易誤數 |
| source/prompt 任務同步檢查 | gal-status | 勾選態比對、輸出歧異 id |
| 審查標記讀取 | gal-status | `<!-- STAFF_REVIEW / ANALYZE / SECURITY_REVIEW / DESIGN_REVIEW_LIVE / ENG_REVIEW -->` 機械抽取 |
| **三面收斂閘** | gal-pipeline Step 2g | source ↔ prompt ↔ state.md 一致性；不變式：history 不得含已 commit 的跨檔歧異 |
| graphify 新鮮度分級 | gal-status | 版本戳 + mtime 比較 |

建議子命令：`gal state show` / `gal plan tasks` / `gal plan sync-check` / `gal plan converge --check`（回 exit code）。
**邊界（留 AI）**：specialist readiness 建議、下一步路由、審查/測試判決。bin 只投影事實 + 閘控不變式。

### 候選 C：能力車道 preflight 解析器（Tier 2）

`checking-contract.md` 五態（`not-applicable`/`unavailable`/`available-but-needs-init`/`available-but-not-ready`/`ready`）屬確定性跨 OS 偵測：binary 是否在 PATH（Windows `PATHEXT` 陷阱）、MCP 是否可達、產物/版本戳是否新鮮。
- **不獨立開工**：待 `gal-core` 存在後順帶提供 `gal preflight <tool>`（OQ-007）。
- **邊界**：route/degrade 決策留 workflow 層；bin 只回報五態事實。

### 候選 D：工時邊界解析器（Tier 3）

`working-hours.md` 時窗判定（4 個 `HH:MM` + 系統時鐘 + diary 是否存在 → 區間 + 動作）目前每個 agent 啟動各自從散文重新推導。
- 核心價值是**決策集中化**（各 runtime 對同一時鐘給一致裁決），非直譯器無關性——故 Tier 3、最末。
- **不獨立開工**：待 `gal-core` 存在後順帶提供 `gal clock` / `gal working-hours status`（OQ-007）。

### 候選 E：跨執行模式統一執行器（local headless + ssh remote）

控制平面有兩個形狀相同、實作相反的派工執行器：

| 執行器 | 既有碼 | 結果回收 |
| --- | --- | --- |
| 本機 headless | `scripts/executors/Invoke-Executor.ps1`（PowerShell 版先交付） | 就地寫回工作樹 |
| 遠端 xmachine | `scripts/Invoke-XmachineTask.ps1` | patch-first（回 `result.patch`） |

兩者抽象一致（準備 spec → 執行 → 驗證結果）但收斂模型相反、無共用具體碼。目前刻意維持兩支獨立腳本、不在 PowerShell 層硬做 `-Transport` 開關。
- **為何進 backlog**：`gal-core` 存在後可用 Rust 強型別定義統一 executor trait + 兩 transport 實作，把 spec 準備、結果驗證契約、commit 邊界不變式抽到共用層。
- **前置條件（硬）**：本機 headless executor 已上線穩定，且 `gal-core` 已存在。
- **bin 邊界**：因遠端節點部署對象不同，是唯一可能正當的第二顆 bin（slim `gal-exec`）；但單 `gal` bin 遠端部署亦可行，**是否拆出待 E 實作時再定**。

### 共同核心：`gal-core` crate

候選 A（NDJSON）與候選 B（markdown plan/state）本質同一問題：確定性解析 repo-owned 狀態檔 + 不變式閘控寫回 + 投影渲染，皆守 File-System Memory Contract（檔案系統為唯一權威、bin 不持第二記憶層）。即使對外暴露為不同子命令，內部共用一個 `gal-core` crate（C/D 亦掛入）。

## Files to Create or Modify

- `[CREATE]` `Cargo.toml`（workspace）、`crates/gal-cli/`、`crates/gal-core/` 等 Rust 原始碼。
- `[MODIFY]` `scripts/Build-CorePlugin.ps1`、`scripts/build-core-plugin.sh` - 加入 `cargo build --release` 並複製本機 OS 執行檔至 `bin/`。
- `[MODIFY]` `.gitignore` - 忽略 Rust `target/`。
- `[MODIFY]` `scripts/scripts.md`、`README.md`、`README.zh-Hant.md`、`docs/devguide.md`、`docs/personalization.md` - 以 Rust 二進位為入口更新說明。
- `[MODIFY]` `commands/gal/SKILL.template.md` 及相關 command templates - 公開介面指向二進位。
- `[MODIFY]` `conventions/token-budget.md` - Rust `target/` 列為預設排除。

## Test Cases

- [ ] TP-001 - `cargo run -- status`：成功執行並返回正確狀態。
- [ ] TP-002 - `cargo build --release` 產生獨立二進位，終端機直接 `gal status` 表現與開發模式一致。
- [ ] TP-003 - 透過更新後的 `Build-CorePlugin.*` 本機 render 外掛：`bin/` 出現本機 OS 原生執行檔、無編譯錯誤。
- [ ] TP-004 - 啟用 Claude 外掛後於 Claude Code 直接輸入 `gal status`：Bash tool 透過 PATH 找到並執行。
- [ ] TP-005 - 帶複雜參數（引號、路徑）：Rust 正確解析轉傳，不因 shell 跳脫崩潰。

## Success Criteria

Phase 1 驗收：

- [ ] Claude Code 外掛啟用後能直接以 `gal` 操作，不需前綴 `bash`/`pwsh`。
- [ ] 本機 render 出之本機 OS 二進位在該平台穩定運作。
- [ ] 基礎設施不再依賴 `bin/gal.sh` 與 `bin/gal.ps1` 轉發層。
- [ ] 外掛 `bin/` 只含編譯好的原生執行檔。

End-state（隨候選 A–D 原生化逐步達成）：直譯器無關、bin 不再委派 scripts。

## Risks

- **編譯工具鏈依賴（已決策接受）**：dev 本機須裝 Rust。決定：接受、不提供預編譯 fallback（renderer 本機即需 cargo）。跨平台編譯複雜度已迴避（每台機器只 render 自己 OS 的 binary）。**行為變更**：本案後 `Build-CorePlugin.*` render 將**硬性要求 cargo 在場**（先前不需）。緩解：render 時 cargo 缺席須 fail-loud（見 Step 3 Verify），避免產出缺 `bin/gal` 的半成品 plugin。
- **安裝腳本邊界誤判（範圍蔓延變體）**：因 Phase 1 仍會修改 `Build-CorePlugin.*`，後續實作可能被誘導順手把安裝編排語義也「一起原生化/重寫」。緩解：對安裝腳本只准新增建置掛鉤（cargo build + 複製 + chmod），任何編排語義改寫一律退回 `/deep-planning`（OE-01 柵欄）。
- **漸進式轉移同步問題**：Phase 1 仍呼叫 `scripts/`，引數傳遞寫錯會導致指令失效。緩解：`clap` 的 `TrailingVarArg` 須正確處理，並對每個轉傳指令做對照測試。
- **候選 B 收斂閘 binize 回歸**：三面收斂閘現由 pipeline 散文 STOP 把守；binize 後 exit-code 契約若錯，可能放行已 commit 的跨檔歧異。緩解：shadow 期（見 OQ-006）。
- **候選 B 變第二記憶層**：state 引擎若落地新狀態檔即違反 File-System Memory Contract。緩解：bin 只投影既有檔案 + 回 exit code、不落地任何新檔；readiness/路由留 AI。
- **範圍蔓延**：Phase 1 fence 為零候選 thin wrapper；安裝/設定編排排除於 bin 之外；候選一律「先 AI 版、binize 為後續最佳化」；單 `gal` bin + 共用 `gal-core` 控制產物數量。

## Open Questions

- [ ] OQ-006 -（Phase-2 施工細節，不阻斷 Phase 1）候選 B 的「三面收斂閘」改寫為 Rust 後，是否先讓 bin `converge --check` 與現行 AI 散文檢查並行比對一段時間（shadow 期）、確認一致再切換為唯一把關？建議切換準則用量化門檻（如連續 N≥10 次真實任務一致、含至少 1 次故意製造的歧異被雙方同時抓到），而非時間。屬候選 B 實作期決定。

## Approval

- Human approval: [pending]
- Architect review: **APPROVE**（2026-06-01，第二輪再確認；見 `## Review Results`）。轉換範圍盤點證明 bin 候選量體小 → 單一 `gal` bin + `gal-core` 共用 crate；安裝/設定編排排除、續留 scripts。第二輪複審確認「安裝腳本邊界」無範圍回歸：Phase 1 對 `Build-CorePlugin.*` 僅加 cargo 建置掛鉤、不重寫編排語義；新增 fail-loud 與「只准加掛鉤」兩護欄。OQ-001/002/003/004/005/007 結案，OE-01 過度範圍已修正。剩 OQ-006 為 Phase-2 施工細節、不阻斷。
- Additional domain review: [not triggered]（無 customer-facing / business-rule 內容）

## Review Results

### Architecture Review

#### Verdict: APPROVE（2026-06-01；第二輪 deep-planning 再確認）

策略方向正確：控制平面向直譯器無關的編譯二進位靠攏，理由是 runtime × OS 矩陣下無普遍直譯器（非效能）。經三輪 deep-planning 收斂後無阻斷項。

**第二輪再審（安裝腳本邊界）**：本輪複審聚焦「計畫是否把安裝腳本拉進範圍」之疑慮。結論：**邊界一致、無範圍回歸**。澄清點——「安裝/設定編排排除」指**編排語義不被 Rust 取代**，非「Phase 1 不碰任何安裝腳本」；Phase 1 確會修改 `Build-CorePlugin.*`（屬安裝群組）但僅新增 cargo 建置掛鉤（編譯產物放置接縫），不重寫其編排語義。此區分已寫入「安裝腳本邊界澄清」段、Step 3 Verify 與 Risks。新增兩道護欄：(1) render 時 cargo 缺席須 fail-loud；(2) 安裝腳本只准加建置掛鉤、任何編排語義改寫退回 `/deep-planning`。OE-01 柵欄續生效，verdict 維持 APPROVE。

**關鍵決策依據（轉換範圍盤點）**：看似龐大的 scripts 量體（PS1 ~11.6k 行 / 32 檔）過半（~6.2k）是安裝/設定編排，而那不是 bin 候選——安裝期一次性、互動執行、直譯器必在場，bin 的直譯器無關性不適用。扣除後，真正的 agent-runtime 確定性 bin 候選（A–D）量體小且內聚。→ 單一 `gal` bin + 子命令完全可行，crate 邊界（OQ-004）因此收斂為單 bin。

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| Rust 原生 bin 取代 .sh/.ps1 轉發層 | 直譯器無關、強型別解析、跨 OS 穩定 | 需 Rust 工具鏈（已接受） | OK |
| Phase 1 為委派 scripts 的 wrapper | 快速上線 PATH entrypoint、可漸進 | Phase 1 不交付零依賴（語義已校正） | OK |
| 單一 `gal` bin + `gal-core` 共用 crate | 發布最簡、PATH 單入口、核心不重複、符合單一公開入口 Goal | bin 體積隨子命令成長（git/cargo 證明可承載） | OK |
| 安裝/設定編排續留 scripts、不原生化 | 省下 ~6.2k 行高成本低效益移植 | 安裝期仍需 pwsh/bash（本就在場） | OK |
| 候選 E 可能拆 slim `gal-exec` | 遠端節點部署對象不同 | 第二顆 bin 維運成本 | 延後至 E 實作時定 |

#### Over-engineering Flags

- **[OE-01] init/setup/sync 原生化過度範圍（已修正）**：原 Requirements/Step 2 宣稱將安裝/設定硬邏輯原生化，成本巨大、執行期效益近零。已修正為排除、續留 scripts；bin 只收 agent-runtime 確定性引擎（A–D）。此修正同時是 OQ-004 收斂為單 bin 的根因。

#### Active Risks（已緩解，見 `## Risks`）

- 候選 B 收斂閘 binize 回歸 → shadow 期（OQ-006）。
- 候選 B 變第二記憶層 → bin 只投影不落地。

#### What's Good

- 「先盤點再定 bin 數」是正確架構紀律：量體決策不靠原則空談，盤點直接證偽過度範圍、省下 ~6.2k 行無謂移植。
- 候選盤點 NEED gate 嚴謹誠實（候選 D 自承價值是決策集中化、主動降 Tier 3）。
- 候選 A+B 同類、共用 `gal-core` 的洞察正確；皆守 File-System Memory Contract。
- Phase 1 fence 為零候選 thin wrapper，候選一律「先 AI 版、binize 為後續最佳化」。

#### 下一步

架構審查 APPROVE。待**人類核准** → `/refining-plan` 鎖 `## Tasks` / `## Test Plan`（以 Phase 1 wrapper dispatch 為首個實作切片）→ `/plan-to-prompt`。
