# Plan: Install / Update / Uninstall 統一 + 傳播穩定化

## Goal

統一 GAL 的 install / update / uninstall 三動詞，讓我在 repo 改完 `skills/`、`agent/`、`commands/`、`mcp.json` 等 source 後，**重跑一次 update 就確定性地**把變更傳播到：

1. 正本 `~/.gal/plugins/gal/`（canonical root），以及
2. 每一個啟用的 provider 讀取面——**Claude、AGY、Copilot、VS Code Copilot、Codex** 五者共用同一條投影/刷新流程。

核心真相：**「改了 source → 重跑 update（不需特殊旗標）→ 五個 provider 用起來都感受到修改」**。

範圍深度為 **Option B**（五 provider 原生生命週期皆於本計畫完成），但**全部在 PowerShell/Bash 腳本層**實作。Rust 化（`gal` bin、cargo-build-into-bin、native 候選 A–E）**不在本計畫**，全歸 [plugin-bin-migration.md](plugin-bin-migration.md)。

`.gal` 統一管理 **canonical content、machine intent、lockfile、provider projection intent、provider ledger**；不宣稱直接擁有每個 provider 目錄裡的所有資料。所有權分類是讓「強制重渲染」安全的前置安全模型，非與傳播分離的另一條工作線。

## Scope Decision

| 項目 | 決定 | 理由 |
| --- | --- | --- |
| 深度 | **Option B**：五 provider（Claude / AGY / Copilot / VS Code Copilot / Codex）的 install/update/uninstall 生命週期皆於本計畫完成 | 要讓全部五個 runtime 共用修改後結果 |
| 實作層 | **腳本層（PS1 + sh）**，非 Rust | 安裝/設定編排續留 scripts |
| Rust 邊界 | **完全切出** → [plugin-bin-migration.md](plugin-bin-migration.md) | 該計畫已 Architect APPROVE、專責所有 Rust 工作 |
| 重疊點 | `Build-CorePlugin.*`：本計畫先落地**冪等 renderer**，plugin-bin 之後在其上加 `cargo build --release` 複製本機 bin | 排序「stabilization 先、bin 後」 |
| 內部排序 | spine（Step 1–2）必先落地，再做 provider 深度（Step 3–4），各 provider 車道可獨立測 | 控 Option B 範圍與風險 |

## Problem Statement — Root Cause（已在程式碼證實）

「改完卻沒 update 到 `.gal`、用起來沒感覺」的根因是**沒有可運作的冪等 update 路徑**，加上**部分 provider 根本沒有投影分支**：

| # | 證實的根因 | 位置 | 後果 |
| --- | --- | --- | --- |
| RC-1 | 正本已存在且未帶 `-Force` → **直接 throw**「Artifact root already exists」 | `Build-CorePlugin.ps1:189-194`、`build-core-plugin.sh:141-142` | 第二次起的更新會炸或被跳過 |
| RC-2 | 整條鏈 `Setup-Machine`(第5步) → `Install-GalPlugins` → `Build-ProviderPlugins` → `Build-CorePlugin` **從頭到尾未傳 `-Force`** | `Setup-Machine.ps1:162-208`、`Install-GalPlugins.ps1:810,821`（`$Force` 預設 $false） | 正常重跑永遠停在第一版正本 |
| RC-3 | 實際渲染迴圈**只有 `agy`、`claude` 有分支**；`copilot`、`codex` 雖標 `implemented`/`native-install` 卻無對應動作 | `Build-ProviderPlugins.ps1:146-164`；`build-provider-plugins.sh:80-100` | Copilot 零投影；宣稱與實作矛盾 |
| RC-4 | Codex 投影**寄生在 AGY**：只有 `agy` 觸發 `Build-CorePlugin -Install` 的 Surface 4 才裝 Codex；未選 AGY 時 Codex 零投影 | `Build-CorePlugin.ps1:160,558-603` | Codex 是否更新取決於有沒有選 AGY |
| RC-5 | host 複製版走味：Claude marketplace / Codex `plugin add` 是 host 自己複製；`Install-ClaudePluginViaMarketplace` 偵測「已安裝」又無 `-Replace` 就 `return` | `Install-GalPlugins.ps1:464-491` | 即使正本重渲染，host 複製面仍 stale（唯活載入面即時反映） |
| RC-6 | host 複製受 **plugin `version` 閘**：官方以 `version` 為 update cache key，`Build-CorePlugin` 寫死 `version:1.0.0` 且 `~/.gal/plugins` 非 git → 版本恆定 → `claude plugin update` 永遠「already at latest」 | `Build-CorePlugin.ps1`（多處 `version='1.0.0'`） | 跨 provider 凍結元兇；實機 Claude cache 凍於 2026-05-30 |
| RC-7 | 測試從不重現真實情境：`Test-InstallGalPlugins.ps1` 每次都明確帶 `-Force` 且用隔離 `$testHome` | `Test-InstallGalPlugins.ps1:258,360,371,375,433` | 「第二次不帶 -Force 重跑」的 bug 綠燈出貨 |

## Core Synthesis（所有權 ⇄ 傳播）

傳播正確性與所有權分類是**同一問題兩面**：

- 正本 `~/.gal/plugins/gal/` 依定義是 **GAL 完全擁有、可重建**的內容（底下不該有任何使用者資料）。
- 「正本是 GAL-exclusive」⇒ **每次重跑都砍掉重建（force-replace）是安全的** ⇒ 這正是讓 update 傳播生效的關鍵。
- 因此所有權 taxonomy 是「積極重渲染」的**安全前置**：先用所有權邊界證成冪等重渲染（spine），再把每個 provider 接上同一條投影/刷新流程。

## Non-Goals

- **不在本計畫實作任何 Rust**：`gal` bin、`cargo build`、native 候選 A–E 全歸 [plugin-bin-migration.md](plugin-bin-migration.md)。本計畫僅把 `Build-CorePlugin.*` 改成冪等 renderer，作為該計畫加 `cargo build` 的前置。
- 不把 setup/install 編排原生化；安裝期腳本仍是正確位置。
- 不清理 provider 目錄中無法證明為 GAL-owned 的資料。
- 不把 Claude 的目錄形狀硬套到其他 provider；Claude 是 **schema baseline，非 path baseline**。
- 不做公開 marketplace 散布（讓未安裝者下載 GAL）——那是另計畫的散布軌（git-hosted + commit-SHA 版本）；本計畫只做本機 install/update/uninstall，並確保 rendered plugin 可發布（manifest validate 過、版本策略可送更新）。
- 不改 package-manager bootstrap payload，除非驗證時發現 install ownership 必須先改 payload contract。
- 不重做 renderer 方向或 AGY renderer migration（[feat-plugin-arch-migration.md](feat-plugin-arch-migration.md) 已 VERIFIED）。

## Ownership Model

### Directory Classes

| Class | Owner | Examples | GAL action |
| --- | --- | --- | --- |
| Canonical content | GAL | `~/.gal/plugins/gal/` | **每次 update 冪等重渲染（force-replace、path-stable）**；僅 managed uninstall 移除 |
| Machine intent | User + GAL schema | `~/.gal/config/config.json`、`~/.gal/config/xmachine.json`、`~/.gal/state/plugins.lock.json` | 預設保留；僅明確確認才 purge |
| Provider projection / link | GAL-owned projection | `~/.claude/skills/gal`、`~/.gemini/antigravity-cli/plugins/gal`、`~/.copilot/installed-plugins/gal-copilot/gal`、`~/.gal/active/<provider>/` | 從正本確定性重建（link-first 即時反映） |
| Host-managed cache / copy | Provider host | Claude marketplace cache、Codex plugin store | 視為可棄之 provider 輸出；**update 時若該面是 runtime 實際讀取面則強制刷新（並 bump version）**，否則僅透過 provider lifecycle 或文件化的 link-first takeover 修改 |
| Provider registry / config | Provider host + GAL entries | Copilot CLI plugin list、Claude marketplace registration、MCP config entries | 僅更新 GAL-owned entries；記 ledger |
| Legacy GAL artifact | GAL | source-mode `~/.copilot/gal`（整 home 捷徑）、舊 `~/.copilot/skills/gal*`、舊 `.copilot/agents` GAL links、stale `~/.gal/dist/provider-plugins/*` | 偵測、分類；僅 dry-run 確認或 managed cleanup gate 後移除 |
| User-owned / unknown | User or provider | `~/.copilot`、`~/.claude`、`.gemini`、`.codex` 內無關檔 | 永不自動移除 |

### `.gal` Authority Boundary

`.gal` 為權威：`~/.gal/plugins/gal/` 正本、`config.json` provider 選擇與 install/source mode intent、`plugins.lock.json` 解析版本、`~/.gal/dist/providers/<provider>/managed.json` provider lifecycle ledger、`~/.gal/generated/*`、`~/.gal/active/<provider>/`。

`.gal` 非權威：`~/.copilot/` 與 `~/.claude/` 全部、provider CLI registries 中非 GAL-owned 的 entries、host-managed marketplace cache 內容（除非 GAL 明確以 link-first 取代並記錄）。

## Lifecycle Status Vocabulary

ledger 的 `status` / `readSurface` 採 4 態（**participle + noun**），取代被濫用的 `implemented`，分兩軸：

| 狀態 | 軸 | 意義 |
| --- | --- | --- |
| `linked-projection` | 讀取面 | provider 透過活連結就地讀正本（symlink/junction/skills-dir）、即時 |
| `refreshed-copy2-host` | 讀取面 | provider 不吃連結 → 正本複製進其 host store；本次 update 已刷新成正本（版本閘者並 bump `version`） |
| `unprojected-artifact` | 讀取面 | 正本已渲染於 `.gal`，但此機器未能投影到該 provider（runtime CLI 不在/不支援） |
| `unsupported-lane` | GAL 能力 | GAL 無此 provider 的 install lane（非核心/未接 lane；Option B 五核心 provider 落此＝測試守門值，代表脊柱漏接） |

對比邏輯：**有投影**（link `linked-projection` / copy `refreshed-copy2-host`）→ **無投影**（`unprojected-artifact`）→ **無 lane**（`unsupported-lane`）。

## Provider Contracts（Option B 深度）

通用紀律 — **capability-probe-first**（推廣 Claude 既有 `Get-ClaudeCliLifecycleSupport` 模式）：任一 provider lifecycle 動作前，先探測真實 CLI 子命令 help，導出 install mode；唯有「探測成功＋實際投影/安裝＋驗證通過」才可標 `linked-projection`/`refreshed-copy2-host`，否則誠實降級為 `unprojected-artifact`/`unsupported-lane` 並於 ledger 記實際模式。**禁止再出現「宣稱 implemented 但迴圈無動作」。**

### Claude（baseline，須保持 green）

- Canonical root：`~/.gal/plugins/gal/`。
- **本地活載入面**：personal-scope **skills-directory plugin** `~/.claude/skills/gal/` → 正本（link-first、就地、不複製），Claude 自動以 `gal@skills-dir` 載入；SKILL.md 即時、agents/`.mcp.json` 需 `/reload-plugins`/重啟。readSurface=`linked-projection`。
- **退役**：`claude plugin install`（複製進 `~/.claude/plugins/cache/...`、受 `version` 閘、寫死 `1.0.0` 永凍）作為本地面；`~/.claude/plugins/gal` junction 非官方自動載入位置 → 降為 legacy/清理。
- Lifecycle：probe `claude plugin validate/--help`；validate 正本 → 連結 `~/.claude/skills/gal`。
- Ledger：`~/.gal/dist/providers/claude/managed.json`。
- 不得 regress Claude Desktop MCP 處理。

### AGY

- 已有 link-first（三面：CLI junction、IDE junction、GUI-config）。本計畫不重開 renderer migration；只確保 install/update/doctor 與 ledger 一致分類 AGY 投影，且冪等重渲染後 junction 仍存活（path-stable）。readSurface=`linked-projection`。

### Copilot（含 VS Code Copilot）

多面分治，**不可假設 `.copilot` 與 `.claude` 同所有權語義**：

| Surface | Owner | 本計畫 GAL contract |
| --- | --- | --- |
| Copilot CLI plugin discovery | Copilot host | probe-first：用文件化的 plugin install/list/update/uninstall，或明確記錄的 local marketplace / link-first 策略 |
| `~/.copilot/installed-plugins/gal-copilot/gal` | Host cache 或 GAL link-first | **Copilot 唯一外掛面**；判定 runtime 讀取面後優先 link-first 連結正本（實機已是 symlink→正本），否則 `refreshed-copy2-host` |
| `~/.copilot/gal` → `~/.gal` | source-mode `GAL_ROOT` 捷徑（整 home） | **install mode 不建**；非外掛面；doctor 標 legacy、cleanup 在 install mode 移除（可證：symlink target=`~/.gal`） |
| `~/.copilot/skills/gal*`、`~/.copilot/agents/*.agent.md`(GAL marker) | Legacy source-mode artifacts | plugin 模式下為 cleanup 候選（gated） |
| Copilot MCP config | Provider config + GAL entries | 僅更新 GAL-owned entries |
| **VS Code Copilot skill visibility** | VS Code / Copilot 擴充 | **併入 Copilot `.copilot` 面刷新**（與 CLI 共用 skills/agents 面），不另立獨立 lifecycle |

### Codex

- provider-native marketplace/plugin 投影語義。本計畫**解除 Codex 對 AGY 的寄生**（RC-4），讓 Codex 在統一脊柱中**獨立**取得 marketplace descriptor 註冊＋`plugin add`/update/uninstall，capability-probe-first。
- Codex marketplace 若同屬版本閘語義，套 R-PROP-008（每 render 變更 `version` 或活載入），避免重蹈 Claude `1.0.0` 凍結。

## Requirements

### 傳播 / 統一

- [ ] **R-PROP-001 冪等更新**：任一 install/update 入口重跑時，正本必從當前 repo source 重新渲染（原子替換、path-stable 以保既有投影連結有效），**不再因「已存在」而 throw**，正常更新**不需 `-Force`**。
- [ ] **R-PROP-002 全 provider 投影刷新**：每個啟用 provider 在每次 update 後其 runtime 讀取面皆反映最新正本——能 link-first 即連結正本；只能 host 複製者，update **必強制刷新**（不得因「已安裝」早退）。
- [ ] **R-PROP-003 狀態誠實性（capability-probe-first）**：lifecycle 先探測真實 CLI 能力再行動；按 `Lifecycle Status Vocabulary` 誠實標示，ledger 記實際模式。取代 `implemented` overclaim。
- [ ] **R-PROP-004 統一脊柱**：install/update/uninstall 對五 provider 共用同一條「渲染正本一次 → 逐 provider 投影/刷新 → 逐 provider 寫 ledger」流程；無 provider 留在「宣稱卻無動作」或「寄生他人」。
- [ ] **R-PROP-005 走味偵測**：doctor 可偵測正本是否落後 repo source、host 複製是否與正本分歧；read-only 報告、不刪檔（與 R-OWN-004 合流）。
- [ ] **R-PROP-006 讀取面判定**：對每個 provider 明確判定 runtime 實際載入面（活連結 vs host 複製），據此選 link-first 或 forced-refresh，並記於 contract/ledger。
- [ ] **R-PROP-007 真實重跑回歸測試**：測試須在**持久化（測試內不每步清空）的隔離 home** 下，模擬「改 source → 不帶 `-Force` 再 update」並驗證**每個 provider 讀取面**反映變更，涵蓋 host-copy 強制刷新。修正 RC-7 盲區。
- [ ] **R-PROP-008 更新可送達（version / in-place）**：更新路徑須真正送達——優先**活載入/就地**繞過版本閘（Claude skills-dir、AGY junction、Copilot installed-plugins symlink）；若 provider 僅支援 marketplace-copy（受 `version` 閘），則每次 render **變更 plugin `version`**（content-hash/timestamp）使 `update` 生效。**禁止寫死 `version:1.0.0`**（RC-6）。

### 所有權

- [ ] **R-OWN-001**：在 maintainer docs 定義 provider directory classes，install 工作從所有權分類起步、非路徑猜測。
- [ ] **R-OWN-002**：為每個啟用 provider 加 lifecycle ledger（比照 Claude `managed.json`：provider、canonicalRoot、projectionRoot 或 hostTarget、installTarget、validation status、lifecycle mode、readSurface、generatedAt、notes）。單一共用形狀、provider 專屬欄位 additive。
- [ ] **R-OWN-003**：修正 lifecycle 狀態回報，只在實際被編排且驗證時標對應 ready 狀態（由 R-PROP-003 強化）。
- [ ] **R-OWN-004**：加 dry-run provider doctor，分類 canonical / expected projection / host-managed / legacy GAL / unknown，不刪任何檔。
- [ ] **R-OWN-005**：Copilot plugin 模式 cleanup policy：source-mode `~/.copilot/gal`、舊 `~/.copilot/skills/gal*`、舊 `.copilot/agents` GAL links 僅在符合 GAL-owned markers/links 時移除。
- [ ] **R-OWN-006**：source-mode contributor 路徑與 install-mode plugin 路徑分離；source 仍可用 repo-root links/整 home 捷徑，install 不可。
- [ ] **R-OWN-007**：更新 install docs，`.gal` 為 intent 與 canonical content 的權威，非 provider 目錄的全擁有者。
- [ ] **R-OWN-008**：Rust `bin/gal` 完全移交 [plugin-bin-migration.md](plugin-bin-migration.md)；本計畫僅交付冪等 renderer 作為其前置。人類核准本計畫後，於該計畫補一行 prerequisite 交叉連結（`Build-CorePlugin.*` 重疊：stabilization 先、bin 後）。

## Approach

### Step 1：冪等正本重渲染（spine 核心修正）

- **Files**：[../../scripts/Build-CorePlugin.ps1](../../scripts/Build-CorePlugin.ps1)、[../../scripts/build-core-plugin.sh](../../scripts/build-core-plugin.sh)
- **What**：移除「已存在即 throw」守衛（RC-1）；改為每次**原子重渲染**正本（render 到 temp → 於同一 canonical 路徑替換內容，保 junction/symlink 以路徑解析有效）；plugin `version` 改為內容雜湊/時間戳（RC-6，R-PROP-008）；正常 update 不需 `-Force`。
- **Verify**：正本存在時重跑不再 throw；改 source 後重跑正本內容與 `version` 更新；AGY junction 與 Claude skills-dir 連結仍解析有效。

### Step 2：渲染所有選定 provider + 接上統一脊柱

- **Files**：[../../scripts/Build-ProviderPlugins.ps1](../../scripts/Build-ProviderPlugins.ps1)、[../../scripts/build-provider-plugins.sh](../../scripts/build-provider-plugins.sh)、[../../scripts/Install-GalPlugins.ps1](../../scripts/Install-GalPlugins.ps1)、[../../scripts/install-gal-plugins.sh](../../scripts/install-gal-plugins.sh)
- **What**：渲染正本一次後，對**所有**選定 provider 進入投影/刷新分支（補上 copilot、codex 真正動作，解除 Codex 對 AGY 的寄生 RC-3/RC-4）；即使未選 claude/agy，正本仍須被渲染。
- **Verify**：任一 provider 子集（含只選 copilot）都會渲染正本並對該 provider 投影；dry-run build plan 的 lifecycle 與實際動作一致。

### Step 3：provider lifecycle — capability-probe-first

- **Files**：`Install-GalPlugins.*`、`scripts/common/ProviderPlugin.ps1` + `scripts/common/provider-plugin.sh`
- **What**：把 Claude 的 capability-probe 抽成 provider-neutral helper；Copilot/Codex 先探測真實 CLI（`copilot`/`gh copilot`、`codex` 的 plugin/marketplace help），導出 install mode；各自實作 install/update/uninstall。
- **Verify**：無對應 CLI 時誠實降級（`unprojected-artifact`）、不 throw、不 overclaim；有 CLI 時 install→update(改 source 後刷新)→uninstall 各自可驗證。

### Step 4：逐 provider 投影 / 刷新 + 讀取面判定

- **Files**：`Install-GalPlugins.*`、common helpers
- **What**：判定每 provider runtime 實際讀取面（R-PROP-006）並優先 link-first 活載入——Claude→`~/.claude/skills/gal`（skills-dir）、AGY→junction、Copilot→`~/.copilot/installed-plugins/gal-copilot/gal` symlink；僅能 host 複製者 update 強制刷新、不早退（RC-5）且每 render 變更 `version`（RC-6）；VS Code Copilot 併入 Copilot `.copilot` 面刷新。
- **Verify**：改 source 後 update，五 provider 讀取面皆反映變更（link 即時、host-copy 已刷新且 version 已變）。

### Step 5：逐 provider ledger（managed.json）

- **Files**：`Install-GalPlugins.*`、common
- **What**：為每個啟用 provider 寫 `~/.gal/dist/providers/<provider>/managed.json`（比照 Claude 形狀 + `readSurface`）。
- **Verify**：install/update（含 dry-run preview）為每個啟用 provider 寫/預覽 ledger，status 取自 `Lifecycle Status Vocabulary`。

### Step 6：Doctor（read-only 分類 + 走味）

- **Files**：`Install-GalPlugins.*`（`-Doctor`/`-Check`；未來 `gal install doctor` 由 plugin-bin-migration 承接）
- **What**：read-only 分類 canonical / expected projection / host-managed / legacy GAL / unknown，並偵測正本落後 source、host 複製與正本分歧；永不刪檔、不把 unknown 標 GAL-owned。
- **Expected Copilot output shape**：

```text
CANONICAL:
  ~/.gal/plugins/gal
EXPECTED PROJECTION:
  ~/.copilot/installed-plugins/gal-copilot/gal -> ~/.gal/plugins/gal
HOST-MANAGED:
  (provider marketplace copies, if any)
LEGACY GAL:
  ~/.copilot/gal -> ~/.gal        (source-mode whole-home shortcut; install-mode cleanup candidate)
  ~/.copilot/skills/gal*
  ~/.copilot/agents/*.agent.md with GAL source markers
USER-OWNED / UNKNOWN:
  everything else
STALE?:
  canonical older than repo source / host-copy diverged from canonical / plugin version unchanged
```

- **Verify**：doctor 不刪檔、不誤標 unknown；能標出 stale。

### Step 7：保守 cleanup gates

- **Files**：`Install-GalPlugins.*`、install tests
- **What**：僅移除可證明的 GAL-owned legacy（symlink target / GAL marker header / known generated path / ledger entry）；預設 uninstall 保 machine intent；purge 仍需明確確認；source-mode 與 install-mode 路徑分離。
- **Verify**：隔離 home 測試證明 cleanup 移除 legacy GAL link，未碰無關 `.copilot`/`.claude` 檔。

### Step 8：統一 install/update/uninstall 動詞面

- **Files**：`Install-GalPlugins.*`、[../../scripts/Setup-Machine.ps1](../../scripts/Setup-Machine.ps1)、[../../scripts/setup-machine.sh](../../scripts/setup-machine.sh)（**受保護路徑，見 fence**）、[../../scripts/scripts.md](../../scripts/scripts.md)
- **What**：確立三動詞語義——install（首次）、update（冪等刷新＝本計畫核心）、uninstall（預設保 intent / purge 明確）。因 renderer 已 idempotent-by-default，Setup-Machine 第5步呼叫理想上改動極小或無；若需傳遞 update 語義則為窄範圍 wiring。
- **Verify**：重跑 Setup-Machine 不再因 canonical 已存在而失敗；三動詞跨 PS1+sh 對齊。

### Step 9：Docs + 跨計畫邊界

- **Files**：[../installation-topology.md](../installation-topology.md)、[../devguide.md](../devguide.md)、[../../scripts/scripts.md](../../scripts/scripts.md)；[plugin-bin-migration.md](plugin-bin-migration.md)（**僅人類核准後**加一行 prerequisite 交叉連結）
- **What**：docs 改為「`.gal` 為 intent + canonical content 之權威，非 provider 目錄全擁有」；lifecycle 狀態措辭對齊實際；記錄本計畫為 plugin-bin-migration 在 `Build-CorePlugin.*` 的前置；Rust 全歸該計畫。
- **Verify**：搜尋文件無 Copilot/Codex「同時 planned 又 implemented」矛盾；本計畫不含任何 Rust 實作範圍。

### Protected Path Fence

本 `/deep-planning` 通過後，**授權**實作對 `Setup-Machine.ps1` / `setup-machine.sh` 的**窄範圍 propagation wiring** 變更（僅對齊冪等 update 語義）。`Sync-DevContext.ps1` / `sync-dev-context.sh` 與本計畫無關、**仍 fenced**。若實作發現需更動 Sync-DevContext.* 或需改 package-manager bootstrap payload，**停止並回 `/deep-planning`**。

## Files to Create or Modify

- `[CREATE]` `docs/plans/fix-install-ownership-stabilization.md` — 本 source plan。
- `[MODIFY]` `Build-CorePlugin.ps1` / `build-core-plugin.sh` — 冪等原子重渲染、移除 throw 守衛、path-stable、`version` 改內容雜湊/時間戳。
- `[MODIFY]` `Build-ProviderPlugins.ps1` / `build-provider-plugins.sh` — 渲染所有選定 provider、修 lifecycle 狀態語義、解除 Codex 寄生。
- `[MODIFY]` `Install-GalPlugins.ps1` / `install-gal-plugins.sh` — 統一脊柱、capability-probe-first、逐 provider 投影/刷新/ledger、doctor、保守 cleanup gates。
- `[MODIFY]` `scripts/common/ProviderPlugin.ps1` / `scripts/common/provider-plugin.sh`、`scripts/common/Common.ps1` / `scripts/common/common.sh` — provider-neutral capability-probe 與 path 分類 helpers。
- `[MODIFY]` `Setup-Machine.ps1` / `setup-machine.sh` — 窄範圍 update 語義 wiring（受保護，已 fence 授權）。
- `[MODIFY]` `Test-BuildProviderPlugins.ps1`、`Test-InstallGalPlugins.ps1`（+ 對應 bash 測試）— 加真實重跑傳播回歸（持久化隔離 home、模擬改 source、不帶 -Force）。
- `[MODIFY]` `docs/installation-topology.md`、`docs/devguide.md`、`scripts/scripts.md` — 所有權/lifecycle 措辭對齊。
- `[MODIFY]` `plugin-bin-migration.md` — **僅人類核准後**加 prerequisite 交叉連結。
- **明確不建立**：任何 Rust / `Cargo.toml` / `crates/` 檔。

## Test Cases

- [ ] TP-001 — provider build-plan 測試：Copilot/Codex 狀態不再 overclaim，除非編排與驗證存在。
- [ ] TP-002 — Claude install dry-run：保留既有 lifecycle state、projection、validation、fallback 語言。
- [ ] TP-003 — Copilot install dry-run（隔離 home）：doctor 分類 canonical / expected projection / host-managed / legacy GAL / unknown。
- [ ] TP-004 — Copilot cleanup dry-run（合成 `.copilot` 資料）：只列 GAL-owned legacy 供移除。
- [ ] TP-005 — Copilot cleanup（隔離 home）：無關 `.copilot` 檔不動。
- [ ] TP-006 — install-mode uninstall（隔離 home）：移除可重建 runtime 輸出，保 `.gal/config`、lockfile、local overrides、secrets。
- [ ] TP-007 — 明確 purge dry-run：machine-local state 移除僅在明確 purge 時列出。
- [ ] TP-008 — 變更 docs/plans 的 Markdown 診斷。
- [ ] TP-009 — 搜尋 docs 無矛盾 Copilot/Codex lifecycle 宣稱。
- [ ] **TP-010 重跑傳播回歸（核心）**：持久化隔離 home，install → 改 source → **不帶 `-Force` 再 update** → 正本內容更新、AGY junction 與 Claude skills-dir 面反映變更。
- [ ] **TP-011 host-copy 強制刷新**：對 host-copy provider，改 source 後 update 強制刷新複製、不早退；複製內容 = 正本。
- [ ] **TP-012 capability-probe 誠實降級**：無 Copilot/Codex CLI 時 lifecycle 不 throw、不 overclaim、ledger 記 `unprojected-artifact`。
- [ ] **TP-013 render-all-selected**：只選 copilot（不選 claude/agy）時正本仍被渲染並對 copilot 投影。
- [ ] **TP-014 junction 存活**：force re-render 後既有 provider 連結仍解析有效（path-stable）。
- [ ] **TP-015 更新可送達**：版本閘 provider 改 source 後 update，plugin `version` 已變更（非恆 `1.0.0`）、`update` 不再回「already at latest」；Claude 走 `~/.claude/skills/gal` 活載入時改 SKILL.md 即時可見（免 version bump）。

## Success Criteria

- [ ] 改 repo source 後重跑 update（**不帶 `-Force`**），五 provider（Claude/AGY/Copilot/VS Code Copilot/Codex）讀取面**皆反映變更**。
- [ ] 無「宣稱 implemented 但迴圈無動作」或「寄生他人」之 provider；統一脊柱涵蓋五者，核心五者皆非 `unsupported-lane`。
- [ ] 正本為冪等重渲染；force-replace 因正本為 GAL-exclusive 而安全，且 junction/symlink path-stable 存活。
- [ ] host-copy 讀取面在 update 時被強制刷新且 `version` 變更、不走味。
- [ ] lifecycle 狀態誠實（capability-probe-first，依 `Lifecycle Status Vocabulary`）；每 provider 有 ledger 與 doctor 分類。
- [ ] Copilot plugin-mode cleanup 僅在所有權可證明時移除舊 artifacts。
- [ ] 測試**重現真實重跑情境**（持久化 home、模擬改 source、不帶 -Force），非僅 always-Force isolated。
- [ ] Maintainers 能在改 install code 前辨識 provider path 屬 canonical / projection / host-managed / registry / legacy / unknown。
- [ ] Claude baseline 維持 green；不 regress Claude Desktop MCP。
- [ ] **Rust 範圍完全不在本計畫**；`Build-CorePlugin.*` 重疊點以「stabilization 先、bin 後」排序，邊界乾淨。

## Risks

- **force-replace 砍正本破壞投影連結**：Mitigation — path-stable re-create + 原子 render-to-temp-then-swap + junction 存活測試（TP-014）。
- **半渲染中斷留下壞正本**：Mitigation — 原子替換（先渲染完整 temp，再替換）。
- **誤把 host-managed cache 當 GAL-owned 移除**：Mitigation — doctor first、cleanup 僅憑 GAL markers/symlink target/known path/ledger。
- **Option B 範圍大、易蔓延**：Mitigation — 內部排序，spine（Step 1–2）必先落地；provider 深度（Step 3–4）capability-gated、逐 provider 可獨立測。
- **未驗證 provider CLI 形狀（Copilot 多面、Codex marketplace）**：Mitigation — capability-probe-first，不猜命令；無 CLI 誠實降級（見 Implementation-time verifications）。
- **與 plugin-bin-migration 在 `Build-CorePlugin.*` 重疊**：Mitigation — stabilization 先落地冪等 renderer、bin 後加 cargo；交叉連結僅人類核准後加。
- **受保護路徑 Setup-Machine.* 變更**：Mitigation — 本 deep-planning 授權窄範圍 wiring；Sync-DevContext.* 仍 fenced；需更動則回 deep-planning。

## Open Questions

規劃期 OQ 已全數結案、決策已 bake 進正文。以下為**實作期驗證項**（不阻斷計畫，於對應 Approach Step 的 Verify 完成）：

- [ ] **IV-1（Copilot）**：實測 Copilot CLI 是否真從 `~/.copilot/installed-plugins/gal-copilot/gal` 連結載入（成立 → `linked-projection`），否則退 `refreshed-copy2-host`。
- [ ] **IV-2（Claude）**：實測 personal-scope `~/.claude/skills/gal` 的 `gal@skills-dir` 活載入行為與 agents/`.mcp.json` reload 需求；確認可安全退役 marketplace-copy 本地安裝。
- [ ] **IV-3（Codex）**：實測 Codex marketplace 載入面與更新版本閘行為，定 `linked-projection` 或 `refreshed-copy2-host` + version 策略。

## Approval

- Human approval: **approved**（2026-06-01）
- Architect review: **APPROVE（conditional / phased）**（2026-06-01；見 `## Review Results > ### Architecture Review`）。方向正確且根因已證實；條件（capability-probe-first、spine 先落地、原子 path-stable 重渲染、真實重跑回歸測試、version 可送達、Rust 完全切出）已編入計畫。規劃期未決項全數結案，僅餘實作期驗證（IV-1..3，不阻斷）。
- Additional domain review: [not triggered]（CLI/install 工具，無 customer-facing UI 或 business-rule 內容）

## Review Results

### Architecture Review

#### Verdict: APPROVE（conditional / phased）（2026-06-01）

方向正確：把「所有權穩定化」校正為「**install/update/uninstall 統一 + 確定性傳播**」，正中實際痛點。根因**已在程式碼與官方文件雙重證實**（RC-1..RC-7），計畫據此施工，風險可控。Option B 範圍大，但條件已全數編入計畫，故 APPROVE 而非 block。

**關鍵洞察（Core Synthesis）**：傳播與所有權是同一問題兩面。正本為 GAL-exclusive ⇒ force-rebuild 安全 ⇒ 傳播生效。計畫以所有權邊界證成冪等重渲染，並以此排序（spine 先、provider 深度後）。

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| 冪等原子重渲染、移除 throw、不需 -Force | update 真正傳播；消除 RC-1/RC-2 | 需 path-stable + 原子替換保 junction | OK |
| 統一脊柱涵蓋五 provider | 消除 RC-3/RC-4「無動作/寄生」 | 五 provider 編排與測試成本 | OK（spine 先、逐車道測） |
| capability-probe-first 推廣 | 不再 overclaim；不猜 CLI 形狀 | 每 provider 需探測 + 降級分支 | OK |
| host-copy update 強制刷新 + bump version | 消除 RC-5/RC-6 走味與版本凍結 | 須先判定 read-surface（R-PROP-006） | OK |
| Claude 改 skills-dir 活載入 | 對齊「裝完即自動看到最新」end-state、繞過版本閘 | 退役既有 marketplace-copy 路徑 | OK |
| Option B 腳本層、Rust 切出 | 直接解痛點、不延後 Rust 計畫 | 腳本層 provider 深度量體大 | OK（內部排序 fence） |

#### Over-engineering Flags

- **[OE-01] 勿為 VS Code Copilot 另立獨立 lifecycle**：併入 Copilot `.copilot` 面刷新即可。
- **[OE-02] 勿讓 per-provider ledger schema 各自發散**：單一共用形狀、provider 專屬欄位 additive（R-OWN-002）。
- **[OE-03] 勿把 doctor/cleanup 鍍金**：doctor 維持 read-only 分類 + staleness；cleanup 僅憑證據 gated。
- **[OE-04] `-Force` 旗標去留**：renderer 既 idempotent-by-default，`-Force` 對正本重渲染已多餘 → 廢除或僅留作明確破壞性語義（留 `/refining-plan` 定）。

#### What's Good

- **根因先證實再施工**：RC-1..RC-7 皆有檔案行號/官方文件佐證，計畫不靠臆測。
- **測試盲區本身列為 finding（RC-7）**：always-Force isolated 從不重現真實重跑 → R-PROP-007/TP-010。
- **所有權 ⇄ 傳播的綜合**正確且有施工後果（force-rebuild 安全性由所有權邊界證成）。
- **capability-probe-first 推廣**把 Claude 既有良好模式一般化，從源頭防止 overclaim 復發。
- **官方文件收緊根因**：Claude stale 真因含 plugin `version` 寫死 `1.0.0`（version 即 update cache key）；定位官方 skills-dir 活載入為正解，對齊使用者 end-state；R-PROP-008 將「禁止寫死 version」一般化為跨 provider 規則。
- **Rust 邊界乾淨**：腳本層解痛點、Rust 全歸 plugin-bin-migration，重疊點以排序解。

#### 下一步

架構審查 APPROVE（conditional）。待**人類核准** → `/refining-plan` 鎖 `## Tasks` / `## Test Plan`（以 Step 1–2 spine 為首切片，再逐 provider 車道）→ `/plan-to-prompt`。

### Business Review

Not triggered — 無 pricing / permissions / onboarding / eligibility 等 customer-visible business logic。

### Design Review

Not triggered — CLI/install 工具，無 customer-facing layout / states / components / 無障礙互動。

### Engineering Review

#### Verdict: CLEAR（2026-06-01）

實作合約可鎖、可建：

- **根因明確、任務對症**：T-001..014 逐一對應 RC-1..7 與 R-PROP/R-OWN；spine（T-001..004）先落地，provider 深度（T-005..009）capability-gated 可獨立測，無強耦合。
- **每任務可獨立完成且可測**：Test Plan TP-001..018 全覆蓋 T-001..014；核心傳播回歸（TP-010/011/015）在持久化隔離 home 重現真實重跑（修 RC-7 盲區），非 always-Force。
- **跨 runtime 對等**：所有任務標 PS1 + sh 對等，符合 GAL cross-runtime 約束。
- **受保護路徑已治理**：T-012 對 `Setup-Machine.*` 的變更限窄範圍 update 語義 wiring，由本 deep-planning fence 授權；`Sync-DevContext.*` 與 bootstrap payload 仍 fenced，越界即回 `/deep-planning`。
- **Rust 邊界乾淨**：無任務建立 Rust/`Cargo.toml`/`crates/`；`Build-CorePlugin.*` 重疊點以「stabilization 先、bin 後」排序，交叉連結（T-014）僅人類核准後加。
- **殘留非阻斷**：IV-1..3（Copilot 連結載入 / Claude skills-dir / Codex marketplace 的實機載入行為）為實作期驗證，已被 capability-probe-first + 誠實降級（`unprojected-artifact`/`refreshed-copy2-host` fallback）涵蓋，任一結果計畫皆可收斂，不阻斷鎖定。

無阻斷項。可進 `/plan-to-prompt`。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

矩陣對齊 T-NNN（PS1 + sh 對等）。integration 一律在**持久化隔離 home**（非每步清空）下跑，以重現真實重跑情境（RC-7）。

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | unit | provider build-plan 狀態不再 overclaim（無編排+驗證不得標 ready） | T-002, T-003 |
| TP-002 | integration | Claude install dry-run 保留 lifecycle state/validation/fallback 語言 | T-005 |
| TP-003 | integration | Copilot install dry-run（隔離 home）：doctor 分類 canonical/projection/host-managed/legacy/unknown | T-007, T-010 |
| TP-004 | unit | Copilot cleanup dry-run（合成 `.copilot`）：只列 GAL-owned legacy | T-011 |
| TP-005 | integration | Copilot cleanup（隔離 home）：無關 `.copilot` 檔不動 | T-011 |
| TP-006 | integration | install-mode uninstall：移除可重建輸出、保 config/lockfile/overrides/secrets | T-011, T-012 |
| TP-007 | unit | 明確 purge dry-run：machine-local 移除僅在 purge 時列出 | T-011, T-012 |
| TP-008 | manual | 變更 docs/plans 的 Markdown 診斷 | T-014 |
| TP-009 | manual | 搜尋 docs 無矛盾 Copilot/Codex lifecycle 宣稱 | T-014 |
| TP-010 | integration | **重跑傳播回歸（核心）**：持久化 home、改 source、不帶 `-Force` 再 update → 正本更新、AGY junction + Claude skills-dir 反映 | T-001, T-002, T-005, T-006, T-013 |
| TP-011 | integration | host-copy 強制刷新：改 source 後 update 重複製、不早退；複製=正本 | T-009, T-013 |
| TP-012 | unit | capability-probe 誠實降級：無 CLI lifecycle 不 throw、ledger 記 `unprojected-artifact` | T-003, T-004 |
| TP-013 | integration | render-all-selected：只選 copilot（不選 claude/agy）正本仍渲染並投影 copilot | T-002, T-007 |
| TP-014 | unit | junction 存活：force re-render 後既有連結 path-stable 有效 | T-001, T-006 |
| TP-015 | integration | 更新可送達：版本閘 provider update 後 `version` 已變、不「already at latest」；Claude skills-dir 改 SKILL.md 即時 | T-001, T-005, T-009 |
| TP-016 | unit | ledger 形狀：每 provider `managed.json` 共用 schema + `readSurface` ∈ 4 態 | T-004 |
| TP-017 | integration | Codex 獨立 lifecycle（不依賴選 AGY）：解除寄生後 Codex 仍註冊 + install/update | T-008 |
| TP-018 | manual | `Setup-Machine` 重跑不因 canonical 已存在而失敗；三動詞 PS1+sh 對齊 | T-012 |

## Tasks

切片順序：spine（T-001..004）必先落地 → 逐 provider 車道（T-005..009）→ doctor/cleanup/verb（T-010..012）→ 回歸測試與文件（T-013..014）。每項 PS1 + sh 對等。

- [x] T-001 — `Build-CorePlugin.*` 冪等原子重渲染：移除「已存在即 throw」守衛，改 render-to-temp → 同 canonical 路徑替換（path-stable 保 junction/symlink）；plugin `version` 改內容雜湊/時間戳取代寫死 `1.0.0`。*(7bfb0929765ac5e8a7860b7ea607de563627915b)*（Step 1；R-PROP-001/008；RC-1/RC-6）
- [x] T-002 — `Build-ProviderPlugins.*` + `Install-GalPlugins.*` 接統一脊柱：渲染迴圈對**所有**選定 provider 進投影/刷新分支（補 copilot/codex、解除 Codex 對 AGY 寄生）；未選 claude/agy 時正本仍須渲染。*(56a171d003c3376392fecee926f616d3c62e6e90)*（Step 2；R-PROP-004；RC-3/RC-4）
- [x] T-003 — provider-neutral capability-probe helper：把 Claude `Get-ClaudeCliLifecycleSupport` 抽到 `scripts/common/ProviderPlugin.*`，供 Copilot/Codex 共用並導出 install mode。*(4878e44f5841ae516b8b12aebdeddec81b0696c2)*（Step 3；R-PROP-003）
- [x] T-004 — Lifecycle Status Vocabulary + ledger：為每個啟用 provider 寫 `~/.gal/dist/providers/<provider>/managed.json`（共用形狀 + `readSurface`），status 取 4 態。*(16c4608a152d1445b12805ae3de128afe047f046)*（Step 5；R-OWN-002/003；R-PROP-003）
- [ ] T-005 — Claude 改 skills-dir 活載入：投影 `~/.claude/skills/gal` → 正本（link-first 就地），退役 marketplace-copy 本地安裝、`~/.claude/plugins/gal` junction 降 legacy；readSurface=`linked-projection`。（Step 4 Claude；R-PROP-002/006/008；IV-2）
- [ ] T-006 — AGY 投影刷新驗證：冪等重渲染後三面 junction 存活（path-stable）；ledger readSurface=`linked-projection`。（Step 4 AGY）
- [ ] T-007 — Copilot lifecycle（probe-first）：CLI install/update/uninstall；link-first `~/.copilot/installed-plugins/gal-copilot/gal` → 正本，否則 `refreshed-copy2-host` + bump version；VS Code Copilot 併入 `.copilot` 面刷新。（Step 3/4 Copilot；IV-1）
- [ ] T-008 — Codex lifecycle（probe-first）：解除 AGY 寄生，獨立 marketplace descriptor 註冊 + `plugin add`/update/uninstall；版本閘套 R-PROP-008。（Step 3/4 Codex；IV-3）
- [ ] T-009 — host-copy 強制刷新：update 對 host-copy provider 不因「已安裝」早退，重新複製 + bump version。（Step 4；R-PROP-002；RC-5）
- [ ] T-010 — Doctor（`Install-GalPlugins.* -Doctor`/`-Check`）：read-only 分類 canonical/expected projection/host-managed/legacy GAL/unknown + 走味偵測（正本落後 source、host-copy 分歧、version 未變）；`~/.copilot/gal` 標 LEGACY；永不刪檔。（Step 6；R-PROP-005；R-OWN-004）
- [ ] T-011 — 保守 cleanup gates：僅移除可證明 GAL-owned legacy（含 source-mode `~/.copilot/gal`，憑 symlink target/GAL marker/known path/ledger）；預設 uninstall 保 intent、purge 明確；source/install 路徑分離。（Step 7；R-OWN-005/006）
- [ ] T-012 — 統一 install/update/uninstall 動詞面 + `Setup-Machine.*` 窄範圍 update 語義 wiring（受保護，已 fence 授權）；scripts.md 對齊。（Step 8）
- [ ] T-013 — 真實重跑傳播回歸測試（`Test-InstallGalPlugins.*` + `Test-BuildProviderPlugins.*` + bash 對等）：持久化隔離 home、模擬改 source、**不帶 `-Force`** 重跑 update，逐 provider 讀取面驗證；含 host-copy 強制刷新與 version 變更。（R-PROP-007；RC-7）
- [ ] T-014 — Docs + 跨計畫邊界：`installation-topology.md`/`devguide.md`/`scripts.md` 所有權與 lifecycle 措辭對齊；**人類核准後**於 `plugin-bin-migration.md` 加一行 prerequisite 交叉連結。（Step 9；R-OWN-007/008）
