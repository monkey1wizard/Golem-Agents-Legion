# Plan: Plugin Bin Migration (Rust Implementation)

## Goal

讓 GAL 的 Claude-compatible plugin root 提供 Claude Code 官方 `bin/` 行為：當外掛啟用時，agent 的 Bash tool 可以直接呼叫 `gal` 等公開 entrypoints。
捨棄原本依賴 `bash/ps1` 轉發層（Thin Wrapper）的妥協方案，改以 **Rust** 開發控制平面的原生二進位檔（Binary）。這能提供極致的跨平台穩定性、無環境依賴（Zero-dependency），並為未來逐步將 `scripts/` 的硬邏輯（Hard logic）原生化鋪路。

### 分期與「零依賴」語義校正

「Zero-dependency / 直譯器無關」是 **end-state**，**非 Phase 1 即達成**：

- **Phase 1（wrapper dispatch）**：`gal` bin 進入 PATH、移除提議中的 `bin/gal.sh`/`bin/gal.ps1` 轉發層、外掛 `bin/` 只含原生執行檔。但 bin 內部仍 **委派 `scripts/gal.*`**，故**直譯器（pwsh/bash）仍須存在**——使用者不需再「鍵入」`pwsh`/`bash` 前綴，不等於機器上「不需安裝」。
- **End-state（直譯器無關）**：唯有當候選 A–D 等硬邏輯原生化、bin 不再委派 scripts 後，才真正零執行期依賴。

Success Criteria 中的跨平台/零依賴項須照此分期解讀；Phase 1 驗收以「PATH entrypoint + 移除 .sh/.ps1 轉發層」為準，end-state 零依賴隨候選原生化逐步達成。

## References

- [Claude Code plugin docs](https://code.claude.com/docs/en/plugins) - plugin root 支援 `bin/`，其中 executables 會在外掛啟用時加入 Bash tool 的 `PATH`。
- [docs/devguide.md](../devguide.md) - Claude plugin root 目前是 `~/.gal/plugins/gal/`。
- [scripts/scripts.md](../../scripts/scripts.md) - 目前所有 public 與 internal scripts 均列在 `scripts/`。
- [conventions/token-budget.md](../../conventions/token-budget.md) - 須將 Rust `target/` 納入 exclusion，並確保 `bin/` 為實際外掛輸出。
- [docs/plans/feat-golem-dockeeper.md](feat-golem-dockeeper.md) - dockeeper 只交付 agent + skill（無 bin）；其確定性引擎邏輯延後到本計畫原生化，候選清單見下方「Native-Logic Migration Candidates」。
- [commands/gal-status/SKILL.template.md](../../commands/gal-status/SKILL.template.md)、[commands/gal-pipeline/SKILL.template.md](../../commands/gal-pipeline/SKILL.template.md) - 控制平面狀態投影與任務收尾「三面收斂閘」目前以**散文指令**要求 LLM 機械解析 plan/state markdown，是 scripts 以外最大的確定性 bin 候選（見候選 B）。
- [conventions/working-hours.md](../../conventions/working-hours.md) - 工時邊界判定為純時窗運算，目前每個 agent 啟動時各自從散文重新推導（見候選 D）。
- [docs/collaborative-tools/checking-contract.md](../collaborative-tools/checking-contract.md) - 能力車道 preflight 的五態解析屬確定性跨 OS 偵測（見候選 C）。
- [graphify-out/2026-05-21_2/GRAPH_REPORT.md](../../graphify-out/2026-05-21_2/GRAPH_REPORT.md) - Community 8「GAL Core State & Plan Management」（cohesion 0.27：`Get-ActivePlanPath`/`Get-StateContext`/`Resolve-PlanPath`/`Unwrap-MarkdownCode`）佐證狀態解析已是內聚的確定性子系統。

## Requirements

- [ ] 初始化 Rust Cargo 專案（如 `crates/gal-cli/` 或 `cli/`）作為 GAL 的控制平面進入點。
- [ ] 透過 `cargo build` 產出跨平台單一二進位檔：`bin/gal` (Mac/Linux) 與 `bin/gal.exe` (Windows)。
- [ ] 實作指令路由（Routing）：初期 Rust 執行檔可作為絕對穩定的強型別 Wrapper，正確解析引數並委派給現有 `scripts/`；中長期逐步將 `init`, `setup`, `sync` 等硬邏輯用 Rust 原生取代。
- [ ] 確保 `Build-ClaudePlugin` 等建置腳本在打包時會執行 `cargo build --release`，並將產出的二進位檔複製到 plugin root 的 `bin/` 內。
- [ ] 移除先前提議的 `bin/gal.sh` 與 `bin/gal.ps1` 腳本需求。
- [ ] 更新 docs and command contracts，確立 Rust 編譯出的 `gal` 為公開入口點。

## Approach

### Step 1: Initialize Rust CLI Project
- **Files**: `Cargo.toml`, `src/main.rs` (建立於如 `cli/` 目錄下)
- **What**: 初始化 Rust workspace/project。建議引入 `clap` 處理 CLI 引數解析，以及 `anyhow` 處理錯誤。
- **Verify**: `cargo check` 與 `cargo build` 能成功編譯出基礎的 `gal` 執行檔。

### Step 2: Implement Dispatch & Execution Logic
- **Files**: `src/main.rs` (及其模組)
- **What**: 實作核心的啟動與分派邏輯。第一階段為了快速遷移，Rust 接收到如 `gal status` 時，能安全地跨 OS 呼叫 `scripts/gal.* status`，解決環境變數、跳脫字元與退出碼（Exit Code）的捕捉問題。
- **Verify**: `cargo run -- status` 能達到與直接執行 `scripts/gal.ps1 status` 相同的效果。

### Step 3: Render Binary into the Plugin Artifact
- **Files**: `scripts/Build-ClaudePlugin.ps1`, `scripts/build-claude-plugin.sh` (或後續提供商中立的腳本)
- **What**: 更新建置流程。打包外掛前先執行 `cargo build --release`（必要時指定 target），然後將編譯完成的原生二進位檔放置到 plugin artifact 的 `bin/` 目錄。
- **Verify**: 建置出的外掛資料夾包含原生 `bin/gal` 執行檔，且能被 Claude Code 的 Bash tool 直接讀取並執行。

### Step 4: Update Documentation & Command Contracts
- **Files**: `scripts/scripts.md`, `README.md`, `README.zh-Hant.md`, `docs/devguide.md`, `commands/gal/SKILL.template.md` 等
- **What**: 說明公開介面已轉移至 Rust 二進位檔。修改先前的系統要求（例如不再強依賴 PowerShell 才能做基本指令入口）。
- **Verify**: 搜尋文件確保不再教導使用者將 `scripts/` 作為首要的互動入口，而是使用原生的 `gal` 二進位檔。

## Native-Logic Migration Candidates

本節記錄可逐步原生化（Rust）的硬邏輯候選，作為 Step 2「將 `scripts/` 硬邏輯原生取代」的具體 backlog。候選來自兩個來源：

1. **`golem-dockeeper`**（見 [feat-golem-dockeeper.md](feat-golem-dockeeper.md)）——該計畫只交付 agent + skill + NDJSON 契約，刻意**不含任何 bin**；其確定性引擎延後到本計畫實作（候選 A）。
2. **scripts 以外的散文式確定性邏輯**——應使用者要求，盤點 `scripts/` 之外、目前以**散文指令要求 LLM 在執行期機械推導**的確定性邏輯。最大宗在控制平面命令 skill 與 conventions：plan/state 解析與收斂閘（候選 B）、能力 preflight（候選 C）、工時邊界（候選 D）。

**候選共同準則（NEED-for-bin 門檻）**：一項邏輯要進 backlog，須同時滿足——(a) 確定性/不變式，非語意判斷；(b) LLM 反覆執行不可靠或會違反不變式；(c) 須在 runtime × OS 矩陣普遍運作，而無普遍直譯器可依賴。不滿足者留在 agent/skill。各候選一律比照 dockeeper 紀律：**先上 AI 執行版並通過測試，binize 為後續最佳化、非前置條件**。

**優先序**：候選 B（Tier 1，NEED 最強）> 候選 A（已有獨立計畫驅動）> 候選 C（Tier 2）> 候選 D（Tier 3）> 候選 E（依賴 headless executor + `gal-state` 就緒，最後啟動）。Phase 1（`gal` wrapper dispatch）不含任何候選，保持 thin。

### 為什麼這些要原生化（跨 runtime × OS 矩陣）

golem-dockeeper 須在 **Claude、ChatGPT/Codex、Gemini/Antigravity、opencode × Windows/Linux/macOS** 下運作。此矩陣下**沒有任何直譯器是普遍保證的**：`python3` 在 Windows 不保證、`bash` 在 Windows 不原生、`pwsh` 在 Mac/Linux 不原生。唯一「零執行期依賴、跨 OS、跨 AI runtime」且能被各 runtime 的 Bash tool 直接呼叫的形式，就是**編譯後的原生二進位**。這正是本計畫採 Rust bin 的核心理由（不是效能，是直譯器無關性），dockeeper 的確定性核心因此適合併入。

### 候選 A：doc-sync 引擎（確定性、適合 Rust 原生）

以下為純機械/不變式邏輯，LLM 反覆讀寫最不可靠，最該由原生二進位保證：

| 候選操作 | 說明 | 為何適合 bin |
| --- | --- | --- |
| git diff 解析與分類 | `git diff -M --name-status <ref>..HEAD` → A/D/R/M 路徑分類 | 純解析，跨平台需穩定輸出 |
| NDJSON 逐行 schema 驗證 | 每行套 `structure-map.schema.json`，不合法即拒寫 | 守「AI 把 NDJSON 寫歪」（dockeeper 風險 #2）；不變式由程式保證 |
| NDJSON 原子局部更新 | 換行 / append / 刪行 / 穩定排序（先 `axis` 再 `id`） | LLM 對大檔排序與精準改行不可靠 |
| 節點生命週期套用 | A/D/R/M → 節點操作（改名靠 `-M` 辨識） | 規則固定、可單元測試 |
| 新鮮度不變式 | 只更新本次核對節點，禁止整批翻 `ok` | 不變式用程式碼強制，LLM 無從違反 |
| 覆蓋掃描 | 走訪 repo 檔案，找出無 `docs` 的 code 檔 → `missing` | 機械列舉 + 跨平台檔案系統走訪 |
| 投影渲染 | 從 NDJSON 渲染待辦清單與樹狀架構視圖 | 純格式化、確定性輸出 |

建議 bin 子命令：`doc-sync validate` / `apply` / `classify-diff` / `coverage` / `render-tree` / `render-todo`。

### 邊界：留在 agent/skill 的語意工作（**不原生化**）

以下屬判斷/編輯，必須留在 `golem-dockeeper` agent + `doc-sync` skill，不可進 bin：

- M(修改) 節點的 `drift`→`ok` 確認（文件內文是否仍正確，屬語意判斷）。
- code→doc 初次關聯（`codeRefs` 對映的語意推斷）。
- 改寫文件正文、決定目標文件結構。

分工原則：**bin 守 NDJSON 完整性與機械計算；AI 守語意判斷與寫字。** `git` 本身普遍可假設（git repo + coding agent 皆有），故 git 呼叫不需進 bin，但其輸出的解析/分類進 bin 以求跨平台穩定。NDJSON 仍是唯一 tracked 權威狀態，bin 是操作它的工具、不持有第二記憶層（守 File-System Memory Contract）。

### 整合方式

- bin 以 subprocess 被各 AI runtime 的 shell/Bash tool 呼叫；per-OS 選對執行檔（Unix 無副檔名 + 執行位元、Windows `.exe`）。
- 與 `gal` CLI 的關係見 OQ-004（同 crate 子命令 vs 獨立 `gal-docsync` crate）。
- dockeeper 先以 AI 執行版上線並通過其測試案例後，才把上述確定性操作逐項換成 bin 呼叫——bin 化是 dockeeper 的後續最佳化，非其前置條件。

### 候選 B：控制平面 plan/state 解析與收斂引擎（Tier 1，scripts 外 NEED 最強）

`/gal status`、`/gal whats-next`、`/gal pipeline` 目前以**散文指令**要求 LLM 在執行期機械解析 repo-owned markdown 狀態檔。這是 scripts 以外最強的 bin 候選：純解析 + 不變式，pipeline 已對其違反**硬性 STOP**，但偵測卻靠 LLM 肉眼讀 markdown。它與候選 A（doc-sync）**同類**——皆為「確定性解析 repo 狀態檔 + 不變式閘控寫回」。

| 候選操作 | 來源（散文位置） | 為何適合 bin |
| --- | --- | --- |
| `.dev/state.md` Active Plans 表解析 + 主 plan 解析 | gal-status Step 1 | 解析 markdown 表、判定 terminal 相、解析 markdown-wrapped 相對路徑 |
| source plan ↔ prompt 路徑配對 | gal-status / gal-pipeline Step 1 | `docs/plans/<slug>.md` ↔ `.dev/plans/<slug>.prompt.md` 固定規則 |
| `T-NNN` / `OQ-NNN` 勾選計數 | gal-status Spec Readiness | 純計數（已勾/總數、未解 OQ）；LLM 易誤數 |
| source/prompt 任務同步檢查 | gal-status（HEALTHY/MISMATCH） | 兩檔 blocking `T-NNN` 勾選態比對，輸出歧異 id |
| 審查標記讀取 | gal-status Review & Test | `<!-- STAFF_REVIEW / ANALYZE / SECURITY_REVIEW / DESIGN_REVIEW_LIVE / ENG_REVIEW -->` 機械抽取 |
| **三面收斂閘** | gal-pipeline Step 2g | source plan ↔ prompt ↔ state.md 三檔一致性；**不變式**：history 不得含已 commit 的跨檔歧異 |
| graphify 新鮮度分級 | gal-status Graphify Freshness | 版本戳比對 + mtime 比較 → NOT-PRESENT/FRESH/STALE/UNSTAMPED |

建議 bin 子命令：`gal state show`（投影 status）/ `gal plan tasks`（計數）/ `gal plan sync-check`（source↔prompt）/ `gal plan converge --check`（三面收斂閘，回 exit code）。
**邊界（留在 AI）**：specialist readiness 建議、下一步路由判斷、審查/測試本身的判決推理。bin 只投影事實與閘控不變式，AI 解讀與決策。

### 候選 C：能力車道 preflight 解析器（Tier 2）

`docs/collaborative-tools/checking-contract.md` 的五態（`not-applicable`/`unavailable`/`available-but-needs-init`/`available-but-not-ready`/`ready`）解析屬確定性跨 OS 偵測：binary 是否在 PATH（Windows `PATHEXT` 陷阱）、MCP 是否可達、產物/版本戳是否存在且新鮮。

- 為何適合 bin：跨 OS 偵測邊界（執行位元、`.exe`、PATH 解析）正是散文與 shell 最易寫歪處。
- 降 Tier 理由：消費端多為 advisory；且與現有 scripts（`Get-GraphifyStatus`/`Test-CommandAvailable`/`Get-GstackStatus`）重疊。**僅在候選 B 的狀態 crate 已存在後**再順帶提供 `gal preflight <tool>`，不獨立開工。
- 邊界（留在 AI）：route/degrade 的工作流決策仍由 workflow 層持有；bin 只回報五態事實。

### 候選 D：工時邊界解析器（Tier 3）

`conventions/working-hours.md` 的時窗判定（讀 4 個 `HH:MM` + 系統時鐘 + 今日 diary 是否存在 → 區間 Working/AfterHours/WrapUp/HardStop + 動作 allow/block/offer-wrap-up）目前由**每個 agent 啟動**與 `gal pipeline` 2a 各自從散文重新推導。

- 為何可進 backlog：確定性、跨 runtime×OS、每次啟動都跑。
- 降至 Tier 3 的誠實理由：核心價值是**決策集中化**（讓各 runtime 對同一時鐘給出一致裁決），**而非直譯器無關性**——`HH:MM` 比較 LLM 本就做得來。NEED 強度低於候選 B/C。建議 `gal clock` / `gal working-hours status` 回結構化裁決，但排序最末。

### 候選 E：跨執行模式統一執行器（local headless + ssh remote）

控制平面已有（或即將有）兩個形狀相同、實作相反的「派工執行器」：

| 執行器 | 既有碼 | 在哪跑 | 結果回收 | commit 邊界 |
| --- | --- | --- | --- | --- |
| 本機 headless | `scripts/executors/Invoke-Executor.ps1`（PowerShell 版先交付）| 本地 spawn AI CLI 子行程 | **就地寫回**工作樹 | 子行程不得 commit；orchestrator 持有 |
| 遠端 xmachine | `scripts/Invoke-XmachineTask.ps1` | SSH 遠端工作節點 | **patch-first**（回 `result.patch`）| 工作節點永不 commit；本地審 patch |

兩者抽象形狀一致（準備 task spec → 執行 → 驗證結果），但**收斂模型相反、無共用具體碼**。目前刻意維持兩支獨立 PowerShell 腳本，**不在 PowerShell 層硬做 `-Transport` 開關**（會變成一個介面掛兩套互斥實作）。

- **為何進 backlog**：當控制平面原生化到 Rust、`gal-state` 核心存在後，可在 Rust 用強型別定義統一的 executor trait + 兩個 transport 實作（local / ssh），把「spec 準備、結果驗證契約、commit 邊界不變式」抽到共用層，各 transport 只實作執行與結果回收。強型別 + 跨 OS + 可測，遠優於 PowerShell 的執行期字串開關。
- **NEED gate**：屬「決策集中化 + 跨 OS 穩定」，非純直譯器無關性。
- **前置條件（硬）**：本機 headless executor（`Invoke-Executor.ps1` PowerShell 版）已上線並穩定，且 `gal-state` crate 已存在。在此之前不動工。
- **邊界**：統一只在「執行器骨架」層；patch-first vs 就地寫回的差異留在各 transport 實作，不強行抹平。
- **建議介面**：Rust executor trait，或 `gal exec --transport local|ssh ...` 子命令。

降至獨立候選（非 Tier 1）：它依賴 headless executor 與 `gal-state` 皆就緒，且價值是「概念統一」而非解某個現行不變式違反。排序在候選 B/A 之後、與 C/D 同層視情況啟動。

### 共同核心：候選 A 與候選 B 應共用狀態 crate

候選 A（NDJSON 結構地圖）與候選 B（markdown plan/state）本質同一問題：**確定性解析 repo-owned 狀態檔 + 不變式閘控寫回 + 投影渲染**。兩者都守 File-System Memory Contract（檔案系統為唯一權威、bin 不持第二記憶層）。因此即使對外暴露為不同子命令，內部宜共用一個 `gal-state` 核心模組/crate。此決策與 OQ-004（crate 邊界）合併處理。

## Files to Create or Modify

- `[CREATE] Cargo.toml`, `src/main.rs` 等 Rust 原始碼檔案。
- `[MODIFY] scripts/Build-ClaudePlugin.ps1` (及其他 build 腳本) - 加入 Rust 編譯流程並複製執行檔至外掛的 `bin/` 目錄。
- `[MODIFY] .gitignore` - 忽略 Rust 的 `target/` 目錄。
- `[MODIFY] scripts/scripts.md`, `README.md`, `README.zh-Hant.md`, `docs/devguide.md`, `docs/personalization.md` - 更新文件說明以 Rust 二進位檔為入口。
- `[MODIFY] commands/gal/SKILL.template.md` 及相關 command templates - 將公開介面指向二進位檔。
- `[MODIFY] conventions/token-budget.md` - 確保 Rust `target/` 被列為預設排除範圍。

## Test Cases

- [ ] TP-001 - 本機執行 `cargo run -- status`；預期結果：成功執行並返回正確狀態。
- [ ] TP-002 - 執行 `cargo build --release` 產生獨立二進位檔，直接於終端機執行 `gal status`；預期結果：成功執行，表現與開發模式一致。
- [ ] TP-003 - 透過更新後的 `Build-ClaudePlugin.ps1` 建置外掛；預期結果：外掛的 `bin/` 目錄出現編譯好的原生執行檔，且無編譯錯誤。
- [ ] TP-004 - 啟用 Claude 外掛後，在 Claude Code 中直接輸入 `gal status`；預期結果：Bash tool 能透過 PATH 找到二進位檔並順利執行。
- [ ] TP-005 - 驗證帶有多個複雜參數（如引號、路徑）的指令；預期結果：Rust 正確解析並傳遞引數，不會因為 Shell 跳脫字元而崩潰。

## Success Criteria

- [ ] Claude Code 外掛啟用後，能直接透過 `gal` 指令操作，不需前綴 `bash` 或 `pwsh`。
- [ ] 二進位檔能跨平台（Windows, macOS, Linux）穩定運作。
- [ ] 基礎設施不再依賴 `bin/gal.sh` 與 `bin/gal.ps1` 腳本轉發層。
- [ ] 外掛的 `bin/` 資料夾內只包含編譯好的原生執行檔。

## Risks

- **編譯工具鏈依賴**：開發者（Dev）本機必須安裝 Rust (Cargo) 才能進行修改與建置，提高了開發門檻。
- **跨平台編譯 (Cross-compilation) 複雜度**：若要在 macOS 上打出 Linux 或 Windows 的二進位檔，可能需要額外的 target 設定或使用 CI/CD 處理。
- **漸進式轉移的同步問題**：初期 Rust 仍需呼叫 `scripts/`，若引數傳遞邏輯寫錯，會導致部分指令失效。需確保 `clap` 的 `TrailingVarArg` 處理正確。
- **候選範圍蔓延**：候選 A–D 皆為獨立子問題。若全塞進同一 `gal` 二進位的子命令，wrapper 會從 thin 變 monolith。緩解：Phase 1 僅做 dispatch wrapper、零候選；候選一律比照 dockeeper「先 AI 版、binize 為後續最佳化」；crate 邊界以 OQ-004 先定（候選 A+B 共用 `gal-state` 核心，C/D 視情況掛入）。
- **候選 B 把投影誤升為權威**：state 引擎若同時持有解析結果與決策，易違反 File-System Memory Contract（變成第二記憶層）。緩解：bin 只投影既有檔案事實 + 閘控不變式（回 exit code），不落地任何新狀態檔；readiness/路由判斷留在 AI。
- **跨檔不變式回歸風險**：候選 B 的「三面收斂閘」現由 pipeline 散文 STOP 把守，binize 後若 exit code 契約寫錯，可能放行已 commit 的跨檔歧異。緩解：binize 前先以候選 B 的 `converge --check` 與現行散文閘並行比對一段時間（shadow），一致後才讓 bin 成為唯一閘。

## Open Questions

- [ ] OQ-001 - Rust 專案的目錄位置應該命名為 `cli/`、`src-gal/` 還是 `crates/gal-cli/`？
- [ ] OQ-002 - 是否要立刻將 `init`, `setup-machine` 等邏輯原生化到 Rust 中，還是第一階段僅實作引數分派（Dispatch/Wrap）？
- [ ] OQ-003 - 對於不具備 Rust 環境的貢獻者，是否需要提供預先編譯好的 fallback 機制或強制透過 GitHub Actions 進行外掛發布？
- [ ] OQ-004 - crate 邊界：候選 A（doc-sync）與候選 B（plan/state 收斂）應作為 `gal` 二進位的子命令（`gal doc-sync ...` / `gal state ...`），還是獨立 crate/二進位？已知兩者宜共用 `gal-state` 核心模組——問題收斂為「單一 `gal` bin + 共用核心 crate」vs「`gal` + 獨立 `gal-state` bin」。影響發布產物數量與 wrapper 重量。
- [ ] OQ-005 - 候選原生化時序：候選 A 待 `golem-dockeeper` AI 版上線驗證後啟動；候選 B（Tier 1）是否值得先於候選 A 進 Phase 2（因其不變式 pipeline 已硬性把守、回歸風險可量測）？
- [ ] OQ-006 - 候選 B 的「三面收斂閘」binize 後，是否採 shadow 期（bin `converge --check` 與散文閘並行比對）再切換為唯一閘？shadow 期長度與切換準則為何？
- [ ] OQ-007 - 候選 C（preflight）與候選 D（工時）是否真有獨立 binize 價值，還是只在 `gal-state` crate 已存在後順帶提供子命令即可？（兩者 NEED 強度分別為 Tier 2 / Tier 3。）

## Approval

- Human approval: [pending]
- Architect review: **REVISE**（2026-06-01；見 `## Review Results > ### Architecture Review`）。候選盤點（B/C/D）穩固；阻斷項為 OQ-003（跨平台 release/CI 產線未定）與 OQ-004（crate 邊界須先於任何候選定案）。Zero-dependency 分期語義已在 Goal 校正。
- Additional domain review: [not triggered]（無 customer-facing / business-rule 內容）

## Review Results

### Architecture Review

#### Verdict: REVISE（2026-06-01，deep-planning pass）

策略方向正確：控制平面向直譯器無關的編譯二進位靠攏，理由是 **runtime × OS 矩陣下無普遍直譯器**（非效能）。候選盤點是本次 pass 的強項。阻斷不在方向，而在兩個發布期必答未知與分期語義。

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| Rust 原生 bin 取代 .sh/.ps1 轉發層 | 直譯器無關、強型別引數解析、跨 OS 穩定 | 開發需 Rust 工具鏈；跨編譯/發布複雜度 | OK（方向） |
| Phase 1 為委派 scripts 的 wrapper | 快速上線 PATH entrypoint、可漸進 | Phase 1 **不**交付零依賴（仍需 pwsh/bash） | OK（已校正語義） |
| 候選 A+B 共用 `gal-state` 核心 | 同類問題不重複造輪、收斂 crate | 須先定邊界才好動工 | OK |
| 候選 C/D 順帶提供、不獨立開工 | 避免範圍蔓延 | — | OK |

#### Bug Surface / Risk

- **[BUG-01] 候選 B 收斂閘 binize 回歸**：三面收斂閘現由 pipeline 散文 STOP 把守；binize 後 exit-code 契約若錯，可能放行已 commit 的跨檔歧異。已於 Risks 以 shadow 期（`converge --check` 與散文閘並行比對）緩解，OK。
- **[BUG-02] 候選 B 變第二記憶層**：state 引擎若落地新狀態檔即違反 File-System Memory Contract。已於 Risks 限定「只投影 + 回 exit code、不落地」，OK。

#### Missing from Plan（阻斷）

- **[MISS-01] 發布產線未定（OQ-003）**：本計畫整個交付物就是「被分發的二進位」。無可重現的跨平台 build/release matrix，就無法驗證 Success Criteria #2「跨平台穩定運作」，也無法服務無 Rust 的貢獻者。這是發布阻斷，須在 Phase 1 收尾前定最小三平台 CI build + 預編譯 artifact 發布策略。
- **[MISS-02] crate 邊界未定（OQ-004）**：候選 A+B 已知宜共用核心；但「單一 `gal` bin + 共用核心 crate」vs「`gal` + 獨立 `gal-state` bin」未定。此決策影響發布產物數量與 wrapper 重量，須先於任何候選原生化定案，否則 Phase 2 會返工。

#### Recommended Changes

1. 解 OQ-003：定最小 CI matrix（Win/macOS/Linux build artifact）+ 預編譯 fallback，使 Success Criteria #2 可驗證、貢獻者免裝 Rust。
2. 解 OQ-004：在開任何候選前定 crate 邊界（建議單 `gal` bin + 共用 `gal-state` 核心 crate，發布產物最少）。
3. （已於本 pass 完成）Goal 已加入分期與零依賴語義校正，Success Criteria 照此解讀。

#### What's Good（保留）

- **候選盤點 NEED gate 嚴謹且誠實**：候選 D 自承價值是「決策集中化」而非直譯器無關性、主動降為 Tier 3，避免為 binize 而 binize。
- **候選 A+B 同類、共用核心的洞察正確**：皆為「確定性解析 repo-owned 狀態檔 + 不變式閘控寫回」，且皆守 File-System Memory Contract。
- **Phase 1 已被 fence 為零候選 thin wrapper**，候選一律比照 dockeeper「先 AI 版、binize 為後續最佳化」，正確控制範圍蔓延。

#### 下一步

解決 MISS-01（OQ-003）與 MISS-02（OQ-004）後重跑 `/deep-planning` 取得 APPROVE，再進 `/refining-plan`（`## Tasks` / `## Test Plan` 目前仍為高層占位，尚未鎖定實作契約）。
