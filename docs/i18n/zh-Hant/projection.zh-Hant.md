---
source: docs/projection.md
lang: zh-Hant
source_commit: 8056cf0d53416d562b3f5546c0eb2aba11a01459
translated_at: 2026-09-21
type: Reference
title: 投影與執行環境介面
description: 說明 GAL 如何把來源合約算繪成標準根目錄、投影到各執行環境介面，並產生儲存庫本機轉接器、投影登錄清單與標準 MCP 清單。
tags:
  - projection
  - runtime
  - adapters
  - render
status: stable
---

# 投影與執行環境介面 (Projection & Runtime Surfaces)

## Projection 生命週期

GAL 的技能、指令、代理程式角色定義與範本，原始檔案皆集中於 `plugins/gal-core/` 並納入版本控管。各 AI 執行環境不會直接讀取這些來源檔案，必須經過算繪與投影處理才會正式生效。

GAL 經由兩條途徑將合約交付給各工具：
第一條為機器層級投影，將整理後的合約，配置至各工具各自的載入路徑。
第二條為儲存庫層級產生，將專案 `.dev/project.md` 轉換為本機轉接器檔案（如 `AGENTS.md`）。

整個轉換流程在架構上劃分為三個專責階段，各自有單一負責的模組，彼此職責互不重疊，變更亦循固定路徑處理：

- **來源合約算繪為標準根目錄**：由 `crates/gal-engine/src/render/` 單獨負責。此模組讀取 `plugins/gal-core/` 的內容，並合併使用者於本機 `~/.gal/local/` 定義的個人層，統一算繪至標準根目錄 `~/.gal/plugins/gal/`。
- **標準根目錄投影至各 AI 工具**：由 `crates/projection/` 單獨負責。此模組將標準根目錄中的內容，依各工具支援的規格格式，投影至各執行環境預期的載入路徑。
- **儲存庫設定轉為本機轉接器檔案**：由 `crates/cli/src/gal/render.rs` 單獨負責。此模組將儲存庫的 `.dev/project.md` 轉換為 `AGENTS.md` 與 `CLAUDE.md`。此步驟完全獨立，不呼叫上述兩個模組。

整體投影與產生流程如下：

```text
plugins/gal-core/（來源合約，受版本控管）
        │
        ├── ~/.gal/local/（個人層，檔案存在即合併）
        ▼
   標準根目錄算繪
   （兩者發生衝突時以核心合約優先，即 Core-Wins 原則）
        ▼
~/.gal/plugins/gal/
（單一標準根目錄，遵循 Claude 外掛格式規範）
        │
        │  依執行環境投影
        │  （視目標環境採用 junction、符號連結或檔案複製）
        │
        ├──▶ Claude Code
        │      外掛指令與外掛代理程式
        │
        ├──▶ Copilot
        │      ~/.copilot/skills/ ＋ ~/.copilot/agents/
        │
        ├──▶ Codex
        │      ~/.agents/skills/（複製檔案）＋ ~/.codex/agents/
        │
        ├──▶ Antigravity
        │      ~/.gemini/antigravity-cli/skills/ ＋ ~/.gemini/antigravity-cli/gal（符號連結，指向來源根目錄）
        │
        └──▶ opencode
               ~/.config/opencode/commands/ ＋ ~/.config/opencode/agents/
```

系統對此有嚴格約束：無論目標為哪一個執行環境，其載入的指令與技能內容，皆必須與標準根目錄中預先建置完成的內容維持逐位元組相同（byte-for-byte identical）。各執行環境之間唯一的差異，僅在於交付傳輸方式（如符號連結、junction 或檔案複製）、沙盒執行模式以及叫用指令的語法格式，指令與技能本身的內容在各環境間不得存在任何差異。具體的位元組等效性驗證方式，詳見本文最後一節。

## 標準根目錄投影 (Canonical Root Projection)

每台機器上僅能存在單一標準外掛根目錄，其固定路徑為 `~/.gal/plugins/gal/`。所有 AI 工具實際讀取的檔案，皆為此根目錄投影出的副本或連結，而非各自獨立的真相來源（source of truth）。投影至磁碟的具體方式視目標工具而定，包含目錄接合點（junction）、符號連結（symlink）或直接複製檔案。

投影資料流一律為單向向外推送，亦即由標準根目錄流向各目標工具。目標端不會寫入任何合約內容以外的資料，因此各工具介面所載入的檔案，皆可隨時由標準根目錄依確定性邏輯完整重建。

## 多重來源投影 (Multi-Source Projection)

標準根目錄結合了兩處來源：受版本控管的核心合約 `plugins/gal-core/`，以及本機端的使用者個人層 `~/.gal/local/`。合併兩處來源時，若遇到相同路徑或相同識別名稱的檔案衝突，一律以核心合約為準，個人層的對應項目將直接略過。這項規則稱為 Core-Wins 原則。

GAL 對個人層的處理原則相當明確：允許使用者擴充自訂內容，但嚴禁覆寫核心合約。個人層的啟用與否純粹取決於實體檔案是否存在，而非由設定檔的開關控制。若刪除某個個人層檔案，下次算繪標準根目錄時，該項目便會自動移除。`_galProjection` 命名空間以外的任何自訂欄位，GAL 均不予更動，算繪產生的結果也絕不會回寫至個人層檔案中。

## 執行環境指令載入 (Per-Runtime Command Loading)

GAL 控制平面的操作定義為一組標準指令（例如 `/gal status`）。Claude Code 與 opencode 具備原生的指令系統，Copilot、Codex 與 Antigravity 則缺乏原生斜線指令支援，必須改以「技能 (skill)」形式載入能力。將通用指令規格轉換為各執行環境專屬格式的職責，由投影層中的 `update_commands`（位於 `crates/projection/src/lib.rs`）統籌。`crates/gal-engine/src/render/` 不參與此階段，因為指令的單一真相來源始終保留在標準根目錄，即 `commands/` 目錄中。

| 執行環境 (Runtime) | 指令載入機制 | 投影目標 | 觸發方式 |
| --- | --- | --- | --- |
| Claude Code | 原生外掛指令 | 標準根目錄 `commands/` | `/gal status` |
| Copilot | 技能（不支援使用者互動式斜線指令） | `~/.copilot/skills/<name>/SKILL.md` | `/gal-status` |
| Codex | 技能（custom prompts 已棄用） | `~/.agents/skills/<name>/SKILL.md`（官方路徑） | `$gal-status` |
| Antigravity | 技能（無原生 `commands/`，以代理程式技能形式載入） | `~/.gemini/antigravity-cli/skills/<name>/SKILL.md` | `/gal-status` |
| opencode | 原生指令 | `~/.config/opencode/commands/<name>.md` | `/gal-status` |

原生支援指令系統的執行環境（Claude Code 與 opencode）不會額外投影指令技能，以避免同一指令出現重複項目。`~/.agents/skills` 目錄由 Codex、opencode 與 Copilot 共用，但該目錄下的指令技能僅在啟用 Codex 時才會寫入。若機器僅啟用 opencode，則僅保留其原生的指令檔案，不會重複寫入指令技能。此設計稱為「雙載入拆分 (dual-loading split)」，旨在防止使用者在介面上看到雙重來源的重複指令。

`write_command_skill` 在處理目標目錄時，會先依據明確的正向標記（`is_managed` 歸屬判定）確認該目錄確實由 GAL 管理，才會執行寫入，避免覆寫使用者自行建立的目錄。當使用者於設定中取消選取特定執行環境時，`remove_gal_command_skill` 會主動移除先前由 GAL 寫入的指令技能，防止殘留檔案導致重複載入。

## 代理程式投影矩陣 (Agent Projection Matrix)

GAL 將各 golem 代理程式的角色定義檔，轉換並投影至各執行環境所支援的代理程式或技能格式。此處並非直接複製原始的 `*.agent.md`，而是依目標環境的原生機制進行結構改寫。這個改寫策略被否決過哪些替代做法，記在 [ADR 01](../../adr/01-projection-and-source-model.md)。

| 執行環境 (Runtime) | 代理程式格式 | 投影目標 | 運作方式 |
| --- | --- | --- | --- |
| Claude Code | `*.agent.md`（含 GAL 標頭，維持原樣） | `~/.gal/plugins/gal/agents/`，對應至 Claude 子代理程式 | Claude 原生外掛子代理程式 |
| Codex | `*.toml`（扁平結構，欄位宣告於頂層，不使用巢狀子表格） | `~/.codex/agents/<name>.toml` | Codex TOML 子代理程式（`crates/projection/src/codex_agent.rs`） |
| Antigravity | 無專屬代理程式檔案，由技能機制提供 | `~/.gemini/antigravity-cli/skills/` | 代理程式介面由投影之技能承載，原生代理程式檔案載入機制尚未經驗證 |
| opencode | `<name>.md`（GAL 標頭後銜接 `mode: subagent` 前置資料，含描述、色彩與權限對應） | `~/.config/opencode/agents/<name>.md` | opencode 原生子代理程式，由 `render_opencode_agent` 算繪，逐檔寫入 |
| Copilot | `*.agent.md`（含 GAL 標頭，內容轉換為 Claude 規範之工具名稱） | `~/.copilot/agents/<name>.agent.md` | 退回機制 (fallback)。僅在設定中 `pluginMode.copilot` 未設為 `true` 時寫入。若設為 `true`，代表 Copilot 已完成原生外掛註冊，GAL 會移除此退回檔案並僅保留指令技能 |

Copilot 代理程式定義檔是否輸出，取決於 `~/.gal/config/config.json` 中的 `pluginMode.copilot` 旗標。誠如 [configuration.zh-Hant.md 的「主要欄位」](./configuration.zh-Hant.md#主要欄位)所述：設為 `true` 代表 Copilot 已啟用原生外掛註冊，此時 GAL 會移除多餘的退回檔案。若未設定或設為 `false`，GAL 則持續寫入退回檔案，以確保在缺乏原生外掛支援時角色定義仍可正常運作。

Codex 代理程式定義檔採用扁平 TOML 格式。`serialize_codex_toml` 將六項角色欄位（`name`、`description`、`developer_instructions`、`model`、`model_reasoning_effort`、`sandbox_mode`）直接宣告於檔案頂層，不包裝在子表格內。由於 Codex 載入時會嚴格校驗每個鍵值，一旦遭遇未知或巢狀舊鍵便會整份拒絕載入，並在 `codex doctor` 中回報語法錯誤。未來若 Codex 規格調整，此單一格式策略能即時凸顯問題並直接修正，避免因維護向後相容分支而隱匿非預期的格式錯誤。

### 角色叫用模式 (Role Invocation Modes)

GAL 代理程式角色提供兩種語意相異的叫用模式：

- **隔離模式 (Isolated Mode)**：例如 `/gal architect`。此模式啟動獨立的子代理程式來執行角色任務，執行完畢後僅將最終判定或結論傳回目前對話，隔離執行過程的中間細節。
- **脈絡內模式 (In-Context Mode)**：例如 `/gal discuss architect`。此模式不啟動子代理程式，而是將該角色的系統指示與行為準則直接載入至目前對話脈絡中，讓使用者能就特定專業領域就地進行多輪深度討論。

由於 Codex 缺乏原生直接將角色規格注入目前工作階段的機制，GAL 提供相應的配套方案：針對四個規劃審查角色（`golem-architect`、`golem-analyst`、`golem-designer`、`golem-releaser`），`gal refresh` 會額外於 `~/.agents/skills/` 產生對應的 `discuss-<role>/SKILL.md` 技能檔案。使用者於 Codex 叫用該技能時，該角色的指示將注入目前工作階段而不另啟子代理程式，達成與脈絡內模式同等的語意。這類技能目錄名稱會登記於共享技能目錄的保留清單中，以防清理流程將其誤刪。

## 僅限協調器驅動的角色排除 (Orchestrated-Only Exclusion)

針對 `implementer`、`tester`、`auditor`、`researcher` 四個角色，`update_agents`（位於 `crates/projection/src/lib.rs`）**嚴格禁止**為其產生投影檔案（不產生 `*.agent.md` 與 `*.toml`）。這些角色雖保留於 `KNOWN_GOLEMS` 清單中以供管道派送時查表解析，但不會產出任何可供終端使用者直接查閱或叫用的實體代理程式檔案。此設計旨在防止「裸叫用 (naked invocation)」：當使用者嘗試輸入 `/gal tester` 或 `$tester` 時，系統會立即回報未知意圖錯誤，避免在缺乏協調器治理下逕自執行未受審核的工作階段。

代理程式定義檔的 frontmatter 使用抽象權限識別字（如 `read`、`edit`、`execute`、`search`、`web`）宣告其能力範圍。`crates/projection/src/tool_map.rs` 是負責將這些抽象權限映射為各執行環境原生工具名稱的唯一模組。在沙盒管理方面，叫用模式亦決定了隔離層級：規劃審查角色在 Codex 中以 `read-only` 模式執行，建置類任務則使用 `workspace-write` 模式。

## 儲存庫本機轉接器產生 (Repo-Local Adapter Generation)

`gal init` 與 `gal render-adapters` 會產生兩份儲存庫本機的轉接器根目錄（`AGENTS.md` 與 `CLAUDE.md`），以及兩份條件式的 Rust 說明層（`.claude/rules/gal-rust.md` 與 `.github/instructions/gal-rust.instructions.md`）。上述四份輸出由 `crates/cli/src/gal/render.rs` 統一專責產生。首次初始化儲存庫時，由 `crates/cli/src/init_repo.rs::run_init_repo` 呼叫。後續重新產生時，則由 `crates/cli/src/commands/system.rs::render_adapters_in` 呼叫。此為 CLI 層級的專屬職責，`gal-engine` 與 `projection` 完全不介入，後者專注於維護 `~/.gal/` 下的機器層級投影，兩者的輸出目標與生命週期截然不同。

算繪這四份檔案的原子化流程如下。任何步驟檢驗失敗皆會立即中止，絕不殘留部分寫入的狀態：

```text
.dev/project.md
   │
   ▼
位元組上限檢查（30,720 B，LF 正規化 UTF-8）
   │  失敗 → 中止且不輸出任何內容（見 ADR 06）
   ▼
擷取 8 個必要 H2 章節（每個章節必須恰好出現一次）
   │  缺漏或重複 → 拒絕並明確標註問題章節
   ▼
算繪 2 份精簡根目錄與 2 份條件式 Rust 說明層（僅於啟用 Rust 時產生）
   │
   ▼
執行兩道前置驗證，全數通過方可寫入：
   ├─ 各根目錄檔案大小上限（32,768 B）
   ├─ 所有權標記衝突分類
   │  任一驗證失敗 → 流程立即中斷，不寫出任何殘存結果
   ▼
移除具標記的退役根目錄，並清理為空的父目錄
   ▼
執行實體寫入，並回報各檔案狀態（Written、Unchanged 或 Removed）
```

這四個輸出路徑由 `render.rs` 集中定義為單一權威清單：`REPO_ADAPTER_ROOTS`（包含 `AGENTS.md` 與 `CLAUDE.md` 兩個路徑）與 `REPO_ADAPTER_CONDITIONAL_LAYERS`（兩個條件層路徑）。算繪邏輯本體、`gal init` 與 `gal render-adapters` 的輸出報表，以及 `/gal finalize` 用於驗證算繪一致性的等冪性檢查（`finalize_check.rs::check_sync_idempotency`），皆共用此定義清單。該檢查僅在記憶體中比對內容而不寫入磁碟，其比對候選集合直接取自 `render.rs::render_candidates` 的回傳值，嚴禁於其他模組中複製此路徑清單。

在寫入磁碟前，`render_and_apply_repo_adapters` 會預先完成所有候選檔案的校驗。轉接器根目錄的寫入順序固定優先於條件層檔案，且兩者皆於單次呼叫中完成。若在實體寫入過程中發生罕見的檔案系統錯誤，GAL 刻意不實作跨檔案交易回復（transaction rollback），因為絕大多數潛在風險與格式問題已在前期預檢階段全數攔截。

`run_sync` 執行後會回傳 `ProjectionReport`，記錄已寫入與已移除之路徑。`init_repo.rs` 將此報告封裝至 `InitRepoReport.adapter_report`，最後由 `commands/system.rs` 格式化輸出為終端機報表，狀態分類為 `Written`、`Unchanged` 與 `Removed`。未來若需擴充報告欄位，應統一於此架構下調整，禁止另闢獨立輸出途徑。

算繪核心的單元與整合測試收錄於 `render.rs` 內的 `#[cfg(test)] mod tests`。實際調用算繪成果的模組測試，則依序配置於 `init_repo.rs` 與 `commands/system.rs` 旁。針對等冪性驗證，`finalize_check.rs` 內的 `sync_idempotency` 測試專門檢驗候選算繪的決定性（determinism）以及錯誤傳播行為，同樣採純記憶體比對而不寫入磁碟。

## 轉接器所有權預檢 (Adapter Ownership Preflight)

`classify_root_ownership` 僅依據轉接器檔案首行內容判斷所有權歸屬。若首行符合目標執行環境的產生標記（`SlimRuntime::generated_marker()`），該檔案即歸類為 `ManagedByGal`，後續套用時將直接原地更新。若不符合，則判定為使用者自維護的 `HandOwned`，GAL 會完整保留其內容且絕不覆寫。至於條件式 Rust 說明層，則檢查檔案內是否存在 HTML 註記 `<!-- GAL-generated: gal init -->` 來判定歸屬。

若既有檔案未帶有 GAL 的所有權標記，GAL 會將其視為不可覆寫的硬性衝突（hard collision）：轉接器套用流程將在寫入任何檔案前立即中止。標準修正方式是修改受版本控管的來源檔案，而非直接編輯轉接器產出物：八個必要章節應於 `.dev/project.md` 調整，條件層內文應於 `plugins/gal-core/conventions/rust.md` 修改，修正後再重新執行 `gal render-adapters`。只要既有檔案保留合法的產生標記，後續套用流程便能順暢地就地更新。

## 情境式 Rust 分層 (Situational Rust Layering)

精簡轉接器根目錄中絕不直接內嵌完整的慣例規範全文。針對 Rust 專屬內容，GAL 依執行環境特性採雙軌機制交付：

第一軌為經實證的原生條件層（即前述兩份條件式 Rust 說明層），專供支援「依主要原始程式碼路徑條件載入」的執行環境使用，提供完整慣例內文。
第二軌則於 `AGENTS.md` 頂層，針對 Codex、Copilot、Antigravity 與 opencode 注入單行指標（pointer）。該指示要求模型在進行 Rust 相關操作前主動讀取 `plugins/gal-core/conventions/rust.md`，藉此規避尚未經實證的複雜條件載入機制。在此類執行環境中，模型僅在觸及該觸發指示時動態載入 Rust 慣例。GAL 刻意避免採用未經充分驗證的非標準範圍規則，以確保投影行為與實際執行語意高度一致。

## 投影登錄清單 (Projection Registry)

`~/.gal/state/plugins.lock.json` 為投影登錄清單檔案。此檔案頂層唯一屬於 GAL 的欄位為 `_galProjection`，由 `projection::persist` 負責寫入，完整記錄 GAL 所管理的各項投影路徑及其對應來源。每次寫入皆原子化產出下一版本的完整狀態快照。重建之分類結構會與來源歸屬嚴格對齊，與本次無關的舊分類以及非 GAL 頂層欄位則完整保留。透過此機制，GAL 得以確保清理安全性：重新投影時絕不會誤刪非受管檔案，且整份登錄檔皆能由重新算繪的狀態依確定性規則完整復原。

| `_galProjection` 子欄位 | 型別 | 說明 |
| --- | --- | --- |
| `schemaVersion` | Number | `_galProjection` 物件本身的 schema 版本（例如 schema v2 為 `2`） |
| `agentProjectionPaths` | Array | GAL 管理的代理程式檔案路徑 |
| `commandProjectionPaths` | Array | GAL 管理的指令檔案路徑 |
| `skillProjectionPaths` | Array | GAL 管理的技能檔案路徑 |
| `discussSkillProjectionPaths` | Array | GAL 管理的 Codex 討論技能檔案路徑 |
| `codexAgentProjectionPaths` | Array | GAL 管理的 Codex 代理程式檔案路徑 |
| `legacyProjectionPaths` | Array | GAL 管理的舊式或轉接器檔案路徑 |
| `sourceAttribution` | Object | 各路徑與來源 ID 之映射表，供多來源投影追蹤歸屬 |
| `pluginOwned` | Object | 執行環境 ID（`claude`、`codex`、`copilot`、`agy`）至其原生外掛機制分類鍵之映射，用以排除非 GAL 投影管轄之項目 |

GAL 嚴格侷限於 `_galProjection` 單一命名空間。檔案中的其他任何頂層欄位皆非 GAL 管轄範疇，任何 `gal` 指令均嚴禁讀寫或修改。

## 標準 MCP 清單 (.mcp.json)

`~/.gal/plugins/gal/.mcp.json` 為標準 MCP 清單檔。`gal refresh` 會讀取受管的 `plugins/gal-core/mcp.json`，並依 Core-Wins 原則合併 `~/.gal/local/mcp.json` 中宣告的個人伺服器。此為 GAL 唯一負責產生的 MCP 設定檔。

| 欄位 | 必填 | 型別 | 說明 |
| --- | --- | --- | --- |
| `mcpServers` | 是 | Object | MCP 伺服器宣告，結構與受版本控管之 `plugins/gal-core/mcp.json` 相同 |
| `inputs` | 否 | Array | 提示驅動輸入定義，原樣透通傳遞，不做額外處理 |

依既定規範，版本控管之 `plugins/gal-core/mcp.json` 預設提供空的伺服器宣告映射，因此在未配置任何個人伺服器的機器上，算繪結果即為不含外部伺服器的空清單。此標準清單的算繪路徑與健康檢查機制始終保持就緒，未來若 GAL 核心需整合專屬伺服器，僅需於受管來源檔案中直接宣告即可。

GAL 在處理設定時不進行任何占位符替換，來源檔案中的環境變數語法（如 `${VAR}`）均原封不動保留。其具體解析完全委由載入該檔的 MCP host 於執行期處理，通常直接讀取行程環境變數。因此 GAL 本身不持有亦不儲存任何 MCP 機密金鑰，該清單檔亦不承載機敏憑證。

重新產生 MCP 清單時，來源為 `plugins/gal-core/mcp.json`，經合併後輸出至 `~/.gal/plugins/gal/.mcp.json`。伺服器宣告欄位由早期的 `servers` 正式更名為 `mcpServers`，以對齊標準協定規格。套件根目錄另包含一份遵循 Agent Plugins 1.0.0 規範的 `mcp.json`，帶有 `$schema` 與 `mcpServers` 定義，傳輸型別亦升級為 `streamable-http`。請注意，`gal update` 僅回報版本資訊，不觸發清單更新。唯有 `gal refresh` 才會依 Core-Wins 原則重新算繪受追蹤的 MCP 清單並合併本機自訂伺服器。所有與 GAL 無衝突的主機層原生 MCP 設定，均不受任何干擾並保持原樣。

## MCP 邊界 (MCP Boundary)

GAL 僅宣告並管理專屬的 MCP 伺服器清單，嚴禁寫入或修改任何主機現行 (in-flight) 的 MCP 設定檔。諸如 `claude_desktop_config.json`、`~/.copilot/mcp-config.json`、`~/.codex/config.toml`、`opencode.json` 或 `mcp_config.json` 等檔案，GAL 均不介入管理。維護共用的主機現行設定屬於相鄰宿主產品之職責，非 GAL 管轄範疇。

## 變更後重新投影與產生 (Regenerating After Changes)

修改 `plugins/gal-core/` 中的任何合約內容，皆不會自動同步至運作中的代理程式，GAL 亦不提供背後的熱重載或自動部署機制。若要讓變更正式生效，需依修改層級採取對應流程：

- 若僅修改 `plugins/gal-core/` 內容（技能、代理程式、指令或慣例）：執行 `gal refresh --source ./plugins/gal-core` 即可。
- 若涉及 `crates/` 中的 Rust 程式碼變更：須先重新編譯並安裝二進位執行檔（`cargo build --release -p gal-cli`，並將產物複製至 `PATH`，例如 `cp target/release/gal.exe ~/.cargo/bin/gal.exe`），隨後執行 `gal refresh --source ./plugins/gal-core`。

`gal refresh` 負責重新算繪標準根目錄，並同步投影至五大執行環境。若 `PATH` 中的二進位檔未同步更新，投影出的技能在呼叫時可能觸發 `unknown command` 等子指令錯誤。因此當 `crates/` 發生變更時，務必依序先更新二進位檔，再執行 `refresh`，順序切勿顛倒。在主要的技能目錄接合 (junction) 模式下，Claude Code 會就地動態載入 `gal@skills-dir`，技能與指令的修改於下一個對話回合即刻生效。若採用次要的 marketplace 複製模式，或剛更新過 `PATH` 上的執行檔，則需重啟 Claude Code 方能重新載入。其餘執行環境則於下一次叫用或開啟新工作階段時自動套用新技能。

本機開發循環 (inner loop) 對照表如下，變更所屬層級決定了後續的生效步驟：

| 變更層級與項目 | 核心生效步驟 | 後續驗證與載入步驟 |
| --- | --- | --- |
| `plugins/gal-core/` 的技能、代理程式、指令、慣例 | `gal refresh --source ./plugins/gal-core` | 於目標執行環境觸發指令以驗證 |
| `plugins/gal-core/mcp.json` 或 `~/.gal/local/mcp.json` | `gal refresh --source ./plugins/gal-core` | 於 MCP host 重新載入伺服器 |
| `crates/` 的 Rust 程式碼 | 重新編譯並安裝二進位檔至 `PATH`（`cargo build --release -p gal-cli`） | `gal refresh --source ./plugins/gal-core` |
| `.dev/project.md` 或 `plugins/gal-core/conventions/rust.md` | 編輯對應來源檔案 | `gal render-adapters` |
| `~/.gal/config/config.json` | 編輯設定檔 | 不經由投影流程，直接生效 |

開發時請特別留意：`gal refresh` 支援經由 `--source <path>` 指定明確的來源根目錄。在開發工作目錄下執行此指令時，務必加上 `--source ./plugins/gal-core`。否則 `gal refresh` 預設會在二進位檔所在目錄尋找來源（該預設適用於發行打包後的扁平拓撲），從 `~/.cargo/bin` 執行時將因找不到來源合約而報錯中斷。

`gal render-adapters` 與 `gal refresh` 職責截然不同。`gal render-adapters` 僅負責重建儲存庫本機的轉接器檔案，不影響機器層級的代理程式投影。若要將 `plugins/gal-core/` 的修改套用至各執行環境的代理程式，必須執行 `gal refresh`。另一方面，`gal init` 僅適用於尚未具備 `.dev/project.md` 的新儲存庫，用以建立初始狀態與轉接器。若於已完成初始化的儲存庫中執行將遭直接拒絕。若需重置 `.dev/project.md` 與 `.dev/state.md`，必須先手動刪除這兩個檔案後再執行 `gal init`。系統刻意不提供單鍵重置旗標，以嚴防誤觸導致專案長期狀態遺失。

若目標路徑存在未受鎖定檔保護或缺乏 GAL 標頭的既有檔案，`gal refresh` 為確保安全絕不逕行覆寫，而是回報 `preserved user-owned path` 並予以保留。開發者在確認該目錄確屬 GAL 舊產物後，應手動移除該目錄再行執行 `gal refresh`，警告便會消除。

`gal doctor` 能檢查三種不同類型的過期與不一致狀況：

1. **技能投影過期**：投影目錄中的技能內容與目前標準來源產生差異，將列為 doctor 診斷發現項 (findings)。
2. **opencode 投影漂移**：在 `gal doctor --dry-run` 模式下，若 `~/.config/opencode/` 檔案與標準根目錄預期算繪內容不一致即會標註，執行 `gal refresh` 即可修復。
3. **二進位檔與原始程式碼落差**：比對編譯時期嵌入的 `GAL_GIT_STAMP` 與當前工作區 `git rev-parse --short HEAD`。若兩者不符，代表 `crates/` 已修改但尚未重新編譯安裝，doctor 會明確提示重新建置與安裝。

評估新增支援其他 CLI 執行環境時，可依序檢視以下四項決策要素以確立整合路線：

1. 該執行環境是否具備可供 GAL 定位的機器層級全域設定目錄？
2. 該環境針對特定儲存庫的指引，能否直接沿用 `AGENTS.md`，抑或需要產生專屬的轉接器檔案？
3. 該環境若具備原生指令系統，則應由共享指令範本直接算繪對應指令。若缺乏原生指令支援，則需將預先建置的指令技能，配置至其支援的技能介面中。
4. 僅在該環境具備穩定且能安全容納非破壞性擴充的設定檔機制時，才額外實作設定檔合併橋接層。

變更代理程式角色名稱或調整派送路由清單後，來源端的異動不會自動傳遞至執行中的 `/gal` 介面，目前執行環境仍會維持先前的外掛載入狀態。此類異動屬於結算階段的手動維護步驟，由維護者於 PR 合併後主動執行，而非自動化 CI 管道任務。標準作業流程為：先執行 `gal refresh` 將最新來源合約投影至各執行環境，接著針對各已初始化儲存庫執行 `gal render-adapters`。角色遷移或更名時涉及的 `executorRouting` 設定與 `pluginMode` 開關說明，請參閱 [configuration.zh-Hant.md](./configuration.zh-Hant.md#執行器路由-executorrouting)。其投影結果會記錄於登錄檔的 `pluginOwned` 欄位中，已略過之路徑將自動退出投影，`gal refresh` 亦會清除先前殘留之實體檔案。

結算作業完成後，可於目標環境驗證兩項指標：確認已更名或退役的 golem 已無法被解析（例如叫用 `/gal old-golem` 會立即回傳未知意圖錯誤），且實體代理程式介面目錄中已不再殘留任何退役角色的定義檔案。

## 驗證與自我檢查 (Verification and Self-Check)

修改算繪或投影邏輯後，應逐一驗證下列檢核項目：

- 每個支援的執行環境，皆具備穩定的投影目標路徑。
- 產生之 `commands/*/SKILL.md` 中，不再含有 `{{GAL_ROOT}}` 等未替換的占位符。
- Antigravity 指令技能已正確重新產生於 `~/.gemini/antigravity-cli/skills/<name>/SKILL.md`，且已安裝的技能與代理程式均透過 agy 外掛根目錄解析。
- 共享技能目錄中僅保留可複用的通用技能，不包含重複的指令別名。
- 任何執行環境的現行 MCP 設定檔均未被寫入或竄改，GAL 僅負責算繪標準 `.mcp.json` 與套件根目錄的 `mcp.json`。
- `gal doctor` 健康檢查所需之機器層級全域路徑，均於 `DoctorPathContext::from_standard_paths` 中完成單次解析後傳入檢查函式，嚴禁於檢查蒐集函式內部再次呼叫 `user_home()` 自行推導。

自動化測試必須確保 `VALID_RUNTIMES` 中每個執行環境實際接收的內容，與標準建置合約維持嚴格的位元組等效性 (byte-for-byte equality)。各環境間的差異僅能存在於傳輸媒介與能力路由層級。嚴禁手動編輯 `AGENTS.md`、`CLAUDE.md` 或已算繪的 `commands/*/SKILL.md`，任何內容異動皆須回歸受版本控管的來源檔案，並經由 `gal render-adapters` 或 `gal refresh` 重新產生。
