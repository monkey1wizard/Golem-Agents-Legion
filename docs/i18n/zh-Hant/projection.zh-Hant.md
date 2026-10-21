---
source: docs/projection.md
lang: zh-Hant
source_commit: e425b72104e9cc3914bca2fe99d7dd89660ae486
translated_at: 2026-10-21
type: Reference
title: 投影與執行環境介面
description: 說明 GAL 如何將來源合約算繪至標準根目錄、投影到各執行環境介面，並產生儲存庫轉接器、投影索引與 MCP manifest。
tags:
  - projection
  - runtime
  - adapters
  - render
status: stable
---

# 投影與執行環境介面

## 投影生命週期

GAL 的技能、指令、代理程式定義與範本，都是位於 `plugins/gal-core/` 下且受版本控管的 Markdown 來源檔案。AI 執行環境不會直接讀取這些檔案，必須先經過算繪與投影才會生效。

GAL 經由兩條管道交付合約：

1. **機器層級投影**：將組裝後的合約複製或連結到各執行環境的設定目錄。這些執行環境都受到支援。
2. **儲存庫層級產生**：將 `.dev/project.md` 編譯成儲存庫本機轉接器檔案，例如 `AGENTS.md`。

投影管道分為三個邊界明確的模組：

- **標準根目錄算繪**（`crates/gal-engine/src/render/`）：將 `plugins/gal-core/` 與選用的 `~/.gal/local/` 個人自訂內容編譯至 `~/.gal/plugins/gal/`。
- **執行環境介面投影**（`crates/projection/`）：使用各工具支援的格式，將標準根目錄資產投影到各目標執行環境的設定目錄。
- **儲存庫轉接器產生**（`crates/cli/src/gal/render.rs`）：將儲存庫的 `.dev/project.md` 編譯成 `AGENTS.md`。此流程獨立執行，不會呼叫前兩個模組。

```text
plugins/gal-core/（來源合約，受版本控管）
        │
        ├── ~/.gal/local/（個人覆蓋層，存在時合併）
        ▼
   標準根目錄算繪
   （衝突時核心合約優先）
        ▼
~/.gal/plugins/gal/
（遵循 Agent Plugins 規範的標準根目錄）
        │
        │  依執行環境投影
        │  （依目標環境採用 junction、符號連結或檔案複製）
        │
        ├──▶ Claude Code
        │      外掛指令與子代理程式
        ├──▶ GitHub Copilot
        │      ~/.copilot/skills/ ＋ ~/.copilot/agents/
        ├──▶ OpenAI Codex
        │      ~/.agents/skills/（檔案複製）＋ ~/.codex/agents/
        ├──▶ Google Antigravity
        │      ~/.gemini/antigravity-cli/skills/ ＋ ~/.gemini/antigravity-cli/gal 符號連結，指向 ~/.gal/source
        └──▶ opencode
               ~/.config/opencode/commands/ ＋ ~/.config/opencode/agents/
```

交付至執行環境的指令與技能內容，必須與算繪完成的標準根目錄逐位元組相同。交付機制、沙盒權限與叫用觸發語法可能不同。執行環境需要主機專屬指示時，共用文字也可能不同。這些差異不會改變共用指令的行為。

## 標準根目錄架構

每台機器恰好維護一個位於 `~/.gal/plugins/gal/` 的標準根目錄。AI 執行環境使用的檔案都是此目錄投影出的副本或符號連結。這些檔案不是獨立的真相來源。投影是從標準根目錄向目標執行環境單向同步推送，執行環境不會寫回修改，因此檔案能隨時確定性地重建。

投影來源是 GAL 的來源根目錄（source root），也就是包含 `skills/`、`agents/` 與 `commands/` 的目錄。在檢出版本中，它是 `plugins/gal-core/`。技能與代理程式投影都從這個根目錄讀取。來源搜尋順序見後文「來源搜尋順序」。

標準根目錄包含下列路徑。各執行環境的投影都從這些路徑複製或連結。

| 路徑 | 內容 |
| --- | --- |
| `skills/<name>/SKILL.md` | 來源根目錄中的技能。 |
| `commands/<name>.md` | 指令檔案。Claude Code 透過外掛載入。 |
| `agents/<name>.md` | Claude Code 用的代理程式檔案，前置資料只保留 Claude 所需的鍵。這裡不套用協調器專用角色排除清單。 |
| `agy-agents/<name>.agent.md` | 代理程式的原始副本。OpenCode 漂移檢查會讀取這些檔案。 |
| `com.github.copilot/agents/<name>.agent.md` | Copilot 用的代理程式檔案，工具名稱已轉換，並排除協調器專用角色。 |
| `com.github.copilot/rules/` | `rules/` 內檔案的 Copilot 副本。 |
| `rules/gal.md` | GAL 指示語料。 |
| `.claude-plugin/plugin.json` | Claude Code 外掛 manifest。 |
| `.codex-plugin/plugin.json` | Codex 外掛 manifest，包含勾點處理器。 |
| `.mcp.json` 與 `mcp.json` | Claude 格式與 Agent Plugins 格式的 MCP manifest。 |
| `bin/` | GAL 執行檔。只在機器本機算繪時寫出。 |

標準根目錄之外，GAL 還會寫出 `~/.gal/plugins/.claude-plugin/marketplace.json` 與 `~/.gal/plugins/.agents/plugins/marketplace.json`。選取 Antigravity 時，也會建立指向來源根目錄的 `~/.gal/source` 連結。內嵌來源會展開至 `~/.gal/embedded-src`。

GAL 置換標準根目錄時，會先把舊的根目錄改名為備份，再把新產生的內容移到原位。舊根目錄無法改名時，GAL 改用非原子的原地同步：覆寫有變更的檔案，並移除過時的項目。另一個行程開啟著根目錄時會發生這種情況。在 Windows 上，行程的工作目錄位於根目錄時也會發生。`~/.gal/embedded-src` 適用相同規則。行程若在原地同步期間被中止，根目錄可能只更新了一部分，下一次執行 `gal refresh` 即可修復。

## 多重來源合併

標準根目錄會合併核心合約（`plugins/gal-core/`）與機器本機個人覆蓋層（`~/.gal/local/`）。路徑或識別碼衝突時，核心合約優先，並捨棄個人項目。這是 Core-Wins 規則。使用者可以加入補充技能或設定，但不得覆寫核心合約。個人覆蓋層只依檔案存在啟用，刪除個人檔案後，下一次 `gal refresh` 會移除其資產。GAL 不會修改 `_galProjection` 以外的檔案，也不會寫回個人覆蓋層目錄。

## 各執行環境的指令投影

GAL 指令（例如 `/gal status`）在 `commands/` 下定義。Claude Code 與 opencode 提供原生指令系統，Copilot、Codex 與 Antigravity 則將指令載入為技能。`crates/projection/src/lib.rs::update_commands` 會將標準規格轉換成各執行環境的結構：

| 執行環境 | 指令系統 | 投影路徑 | 觸發語法 |
| --- | --- | --- | --- |
| Claude Code | 原生外掛指令 | 標準根目錄 `commands/` | `/gal status` |
| GitHub Copilot | 代理程式技能 | `~/.copilot/skills/<name>/SKILL.md` | `/gal-status` |
| OpenAI Codex | 代理程式技能 | `~/.agents/skills/<name>/SKILL.md` | `$gal-status` |
| Google Antigravity | 代理程式技能 | `~/.gemini/antigravity-cli/skills/<name>/SKILL.md` | `/gal-status` |
| opencode | 原生 Markdown 指令 | `~/.config/opencode/commands/<name>.md` | `/gal-status` |

原生指令系統不會收到重複的指令技能投影。`~/.agents/skills/` 由 Codex、opencode 與 Copilot 共用，但只有啟用 Codex 時才會投影指令技能。寫入前，`write_command_skill` 會驗證 `is_managed` 所有權標記。停用執行環境時，`remove_gal_command_skill` 會清理先前投影的指令檔案。當 Claude Code 外掛未註冊時，Claude Code 會使用後備路徑。此時 GAL 會將指令寫入 `~/.claude/commands/<name>.md`，並保留該路徑上的使用者檔案。

## Codex 外掛勾點投影

GAL 將 Codex 管道勾點保存在 `plugins/gal-core/codex/hooks.json`。算繪時，GAL 會將此片段合併至 `.codex-plugin/plugin.json#hooks`。片段定義 `UserPromptSubmit`、`PreToolUse` 與 `Stop` 處理器，每個處理器都會叫用 `gal pipeline-host-hook`。

勾點只屬於 manifest 資產。GAL 不會建立 `hooks/hooks.json`，也不會將勾點加入根目錄 `plugin.json` 或 `.claude-plugin/plugin.json`。GAL 不會將勾點投影至 Claude Code 或 OpenCode，兩者保留既有的主機管理管道接續行為。

```text
plugins/gal-core/codex/hooks.json
          │
          ▼
.codex-plugin/plugin.json#hooks
          ├──▶ Codex 外掛勾點處理器
          ├──▶ 不建立根目錄 hooks/hooks.json
          ├──▶ 不加入根目錄 plugin.json 勾點
          ├──▶ 不加入 .claude-plugin/plugin.json 勾點
          └──▶ 不投影 OpenCode 勾點
```

Codex 勾點分支只有在受信任的即時勾點交握完成後，才會啟用受防護的接續模式。`gal doctor` 可以回報靜態封裝與指令就緒狀態，但不能證明 Codex 信任或執行處理器。同一工作階段的 canary 由 `Stop` 確認 bootstrap 並回傳精確的授權延續提示，再由 `UserPromptSubmit` 綁定相同工作階段。消費授權本身不代表已授權實作工作，受防護入口仍須透過新的入口閘門與語意任務檢查點。

bootstrap 進行期間，`PreToolUse` 拒絕所有受支援的工具呼叫。canary 完成後，只允許該工作階段中精確的授權首個動作。專用與託管工具路徑不在此保證範圍內。受防護入口建立協調器修訂版 0 後，bootstrap 限制結束。相符的作用中設定檔對一般工具使用回傳 neutral/pass，但此結果不會授權檢查點或階段轉換。受防護 GAL 動作仍驗證協調器修訂版、收據與任務品質。`Stop` 會阻擋過早的最終回覆，直到協調器終止或回報具型別的人工作業必要狀態。

若接續停止，請保持工作區與工作階段識別不變。先執行 `gal doctor`，再檢視 `.dev/pipeline/<plan-scope>/` 下的協調器與嘗試證據。修正信任、manifest 或處理器問題後，若投影資產過期，請執行 `gal refresh`。請為新的授權啟動新的受信任同工作階段交握。不要複製、手動建立或重用授權。衝突解決前請保留衝突狀態。若要在沒有 Codex 受防護行為的情況下繼續，請使用一般的 `gal pipeline <prompt>`。

沒有有效授權的一般 `gal pipeline <prompt>` 維持 `LegacyInteractive`，並保留既有 v1 收據、標記、結束與修改行為。主機中立的叫用無法推斷從未執行的 Codex 勾點是否不存在、停用或不受信任。明確要求授權的入口在授權遺失或無效時，會於實作前失敗。Codex 勾點只投影至 Codex 外掛，Claude Code 與 OpenCode 保留既有接續行為。

GAL 的共用技能與指令文字可以包含 Codex 專用指示。這些差異不允許將 Codex 勾點投影到其他主機，也不會改變其他主機的叫用與接續合約。

## 代理程式投影矩陣

GAL 將 `*.agent.md` 角色定義轉換成執行環境原生的代理程式或技能格式，而不是直接複製檔案：

| 執行環境 | 目標格式 | 投影目標 | 機制 |
| --- | --- | --- | --- |
| Claude Code | 標準根目錄的 `agents/<name>.md`，前置資料限於 Claude 所需的鍵，`tools` 轉換為 Claude 工具名稱 | `~/.gal/plugins/gal/agents/` | 以外掛子代理程式載入。標準算繪不會在此目錄套用協調器專用角色排除清單。 |
| OpenAI Codex | 不含巢狀表格的扁平 `*.toml` | `~/.codex/agents/<name>.toml` | Codex TOML 子代理程式（`crates/projection/src/codex_agent.rs`）。協調器專用角色會被略過。 |
| Google Antigravity | 投影技能 | `~/.gemini/antigravity-cli/skills/` | 只投影技能目錄。代理程式保留在標準根目錄的 `agy-agents/<name>.agent.md`，沒有投影程序會將其寫入此處。原生代理程式檔案尚未驗證。 |
| opencode | 帶有 `mode: subagent` 前置資料的 `<name>.md` | `~/.config/opencode/agents/<name>.md` | 由 `render_opencode_agent` 算繪。 |
| GitHub Copilot | 對應至 Claude 工具名稱的 `*.agent.md` | `~/.copilot/agents/<name>.agent.md` | 除非 GAL 確認外掛已註冊，否則會投影退回檔案。協調器專用角色會被略過。指令技能仍會投影。 |

GAL 會從 Copilot 的 `enabledPlugins` 偵測註冊狀態。確認註冊後，外掛會接管代理程式。若註冊不存在或狀態不明，GAL 會保留退回代理程式。狀態不明時也會發出警告。下次 refresh 會套用註冊狀態變更，且只會清理 GAL 管理的退回檔案。GAL 會保留使用者檔案衝突。Codex 代理程式檔案採用扁平 TOML。`serialize_codex_toml` 在根層定義 `name`、`description`、`developer_instructions`、`model`、`model_reasoning_effort`、`sandbox_mode` 六個欄位。Codex 會拒絕未知或舊式巢狀鍵，`codex doctor` 因而能立即顯示格式差異。

### 角色執行模式

- **隔離模式**（`/gal architect`）：啟動專用子代理程式，只將摘要判定回傳至主要對話。
- **脈絡內模式**（`/architect discuss`）：將角色規則與開發者指示直接注入目前工作階段，支援多輪互動式諮詢。

四個獨立角色指令來自 `plugins/gal-core/commands/{architect,analyst,designer,releaser}/SKILL.template.md`。既有的指令掃描器與算繪器會像處理其他指令一樣處理它們，沒有新增投影路徑。每個指令預設執行 `gal dispatch-script golem-<role>`，當第一個引數是 `discuss` 時，則執行內部的 `gal consult-script golem-<role>`。

算繪產物是一件事，執行環境的安裝模式是另一件事：

- **標準產物**：算繪器在標準根目錄下，為每個角色寫出一個指令與一個對應的內建技能。重複算繪的結果相同。
- **安裝模式**：Claude Code 與 opencode 使用原生指令。在 Claude Code 外掛模式中，外掛以 `/gal:<role>` 提供指令，不會另外寫出後備檔案。Copilot、Codex 與 Antigravity 透過既有的後備投影取得指令技能。

投影不再建立 `discuss-<role>` 技能，`discussSkillProjectionPaths` 也已退役。`gal refresh` 會刪除四個已退役且由 GAL 管理的目錄，並保留沒有管理紀錄的使用者目錄。沒有任何轉址取代已移除的 `$discuss-<role>` 技能。請依執行環境使用下列替代方式：

| 執行環境 | 替代方式 |
| --- | --- |
| Claude Code | `/<role> discuss` |
| Claude Code 外掛模式 | `/gal:<role> discuss` |
| Codex | `$<role> discuss` |

Codex 原生代理程式定義（`~/.codex/agents/<name>.toml`）及其 `model` 與 `effort` 投影維持不變。

## 排除協調器驅動的角色

對 `implementer`、`tester`、`auditor` 與 `researcher`，`crates/projection/src/lib.rs` 中的 `ORCHESTRATED_ONLY_GOLEM_NAMES` 列出原生子代理程式投影必須略過的角色。同一檔案中的 `update_skills` 會先檢查此清單，才會把 Copilot 後備代理程式寫入 `~/.copilot/agents/`，並把 Codex TOML 代理程式寫入 `~/.codex/agents/`。標準算繪會將同一份清單套用到 `com.github.copilot/agents/`。但標準根目錄的 `agents/` 不套用此清單，因此這四個角色仍保留在其中。這些角色保留在 `KNOWN_GOLEMS` 中，讓管道能解析並在協調器控制下派送。角色定義的 `read`、`edit`、`execute`、`search` 與 `web` 權限，會由 `crates/projection/src/tool_map.rs` 轉換成執行環境工具。在 Codex 中，規劃審查使用 `read-only`，管道實作使用 `workspace-write`。

## 儲存庫轉接器產生

GAL 不再產生 `CLAUDE.md`。重新產生轉接器時，GAL 會移除帶有原始 GAL 產生標記的舊 `CLAUDE.md`，並保留使用者自行維護的同名檔案。

對於包含技能索引的轉接器，產生的 `AGENTS.md` 會以兩個獨立區段列出儲存庫內容。`Repo Skills` 列出探索到的技能。`Repo Commands` 列出在儲存庫指令目錄下找到的真實 `SKILL.template.md` 指令，每項附上目錄名稱與相對於儲存庫的路徑，並依名稱排序。同名的技能與指令會各自出現在自己的區段。GAL 不會列出根目錄或模板不存在的指令，也不會為來自外部來源的指令虛構路徑。

產生 `AGENTS.md` 不代表你的 coding agent 一定會載入它。若儲存庫中有 `CLAUDE.md` 或類似指示檔案，工具本身的載入設計可能導致它略過 `AGENTS.md`。支援的檔名、檔案探索方式與載入優先順序，請以你的 coding agent 說明文件為準。詳見[安裝說明](setup.zh-Hant.md)。

`gal init` 與 `gal render-adapters` 會產生 `AGENTS.md`、`.claude/rules/gal-rust.md` 與 `.github/instructions/gal-rust.instructions.md`。這些檔案由 `crates/cli/src/gal/render.rs` 產生：

```text
.dev/project.md
   │
   ▼
位元組數檢查（建議上限 30,720 B，LF 正規化 UTF-8）
   │  超過上限 → 輸出一則警告並繼續（見 conventions/token-budget.md）
   ▼
驗證 8 個必要 H2 章節標題（每個恰好一次）
   │  缺少或重複 → 拒絕並回報問題標題
   ▼
產生 1 個轉接器根目錄與 2 個條件式 Rust 層（啟用 Rust 時）
   │
   ▼
執行寫入前驗證：大小上限 32,768 B、所有權標記與衝突檢查
   │  失敗 → 立即中止，不進行部分寫入
   ▼
移除 GAL 所有的退役根目錄檔案，包含 CLAUDE.md並清理空的父目錄
   ▼
寫入檔案並回報 Written、Unchanged 或 Removed
```

目標路徑集中定義於 `render.rs` 的 `REPO_ADAPTER_ROOTS` 與 `REPO_ADAPTER_CONDITIONAL_LAYERS`。轉接器算繪、CLI 狀態報表與 `finalize_check.rs::check_sync_idempotency` 都使用這些定義。等冪性檢查在記憶體中評估算繪結果，不會寫入磁碟。`render_and_apply_repo_adapters` 會先驗證候選內容，再依序寫入根目錄與條件層。系統不提供跨檔案回復。同步流程會回傳追蹤已修改與已移除路徑的 `ProjectionReport`，報告摘要使用 `Written`、`Unchanged`、`Removed`。

### 已安裝版本與來源建置的投影所有權

套件管理器負責已安裝的 GAL 執行檔。`gal refresh` 會使用已安裝的 GAL 更新共用投影。`gal dev-apply` 會將來源檢出部署至該投影，且只能由維護者手動執行。來源工作樹的驗收執行使用 `gal dev-apply --source <path> --root <path>`，且只會在該 root 下投影。GAL 會在寫入任何檔案前，對不存在、相對路徑、使用者設定檔及共用安裝目錄等根目錄採用 `reject-before-write`。`HOME` 與 `USERPROFILE` 等環境變數不能授權投影根目錄。來源建置輸出必須留在工作樹內。這個路徑是該工作樹專屬的私有執行環境路徑。

### 來源搜尋順序

`gal refresh` 依下列順序尋找來源根目錄：

1. GAL 執行檔所在目錄，接著是其上層目錄。
2. 展開至 `~/.gal/embedded-src` 的內嵌來源。

兩者都找不到時，`gal refresh` 會以錯誤結束，且不寫入任何檔案。它不會搜尋目前目錄。`gal dev-apply` 使用 `--source` 指定來源。未提供時，使用目前的檢出版本。`--root` 必須與 `--source` 一起使用。

`gal render-adapters` 的搜尋方式不同。它從目前目錄往上找，直到第一個包含 `.git` 的祖先目錄。接著檢查執行檔所在目錄及其上層目錄，最後使用內嵌來源。

## 轉接器所有權驗證

寫入前，GAL 會把每個根目錄轉接器與條件式層歸類為下列三種狀態之一。

| 狀態 | 根目錄轉接器（`AGENTS.md`） | 條件式層 |
| --- | --- | --- |
| `Absent` | 檔案不存在。GAL 建立它。 | 檔案不存在。Rust 慣例被選取時，GAL 建立它。 |
| `GalOwned` | 第一行是完全相符的生成標記。GAL 就地更新。 | 檔案包含 `<!-- GAL-generated: gal init -->`。GAL 更新它。Rust 慣例不再被選取時，則移除它。 |
| `Collision` | 檔案存在但沒有標記。GAL 在寫入前中止整個呼叫。 | 檔案存在但沒有標記。GAL 在寫入前中止整個呼叫。 |

`crates/cli/src/gal/render.rs` 中的 `classify_root_ownership` 與 `classify_layer_ownership` 執行這些判定。任一條件式層發生 `Collision`，整個呼叫都會中止，因此不會只寫入其中一層。

寫入前，GAL 會移除已退役的根目錄轉接器 `.agents/rules/gal.md`、`.github/copilot-instructions.md`、`CLAUDE.md` 與 `GEMINI.md`。只有當第一行等於該路徑記錄的生成標記時才會移除。沒有標記的檔案會保留，並回報為人工維護。缺少 GAL 標記時，產生流程會在修改前中止。若要更新設定，請修改 `.dev/project.md` 或 `plugins/gal-core/conventions/rust.md`，再執行 `gal render-adapters`。

## 條件式 Rust 規則層

轉接器根目錄不會嵌入完整慣例文字。支援依目錄載入規則的環境（例如 Claude Code 與 GitHub Copilot）會收到 `.claude/rules/gal-rust.md` 與 `.github/instructions/gal-rust.instructions.md`。共用的 `AGENTS.md` 根目錄由 GAL 為 Codex CLI、Copilot CLI、Antigravity 與 opencode 算繪，沒有原生規則層。當 Rust 慣例被選取時，GAL 會在慣例清單之後加入 `## Rust Convention Trigger` 區段，要求模型在 Rust 或 Cargo 工作前讀取 `plugins/gal-core/conventions/rust.md`。

## 投影索引（`plugins.lock.json`）

`~/.gal/state/plugins.lock.json` 會追蹤投影資產。GAL 只管理頂層 `_galProjection`，其他頂層欄位不變。投影流程結束時，若投影狀態有改變，GAL 才會重寫此檔案。乾跑模式不寫入。重寫時會保留 `_galProjection` 以外的頂層鍵，並在 `_galProjection` 中記錄投影資產及其來源對應關係：

| `_galProjection` 子欄位 | 型別 | 說明 |
| --- | --- | --- |
| `schemaVersion` | Number | 索引版本，例如 `2`。 |
| `agentProjectionPaths` | Array | 受管理的代理程式檔案路徑。 |
| `commandProjectionPaths` | Array | 受管理的指令檔案路徑。 |
| `skillProjectionPaths` | Array | 受管理的技能檔案路徑。 |
| `discussSkillProjectionPaths` | Array | 已退役的中繼資料，僅在清理舊的 GAL 管理 consult 技能時讀取。 |
| `codexAgentProjectionPaths` | Array | 受管理的 Codex 代理程式路徑。 |
| `legacyProjectionPaths` | Array | 受管理的舊式轉接器路徑。 |
| `sourceAttribution` | Object | 檔案路徑與來源識別碼的對應關係。 |
| `pluginOwned` | Object | `claude`、`codex`、`copilot`、`agy` 等執行環境識別碼與外掛分類鍵的對應關係。 |

`pluginOwned` 會依本次 refresh 的註冊快照推導，只記錄因確認外掛擁有權而省略的分類。`Unknown` 不會記錄外掛擁有權。Claude 外掛擁有指令，Codex 外掛擁有核心技能，Copilot 外掛擁有代理程式，Antigravity 外掛擁有指令技能。Codex TOML 代理程式與指令、Copilot 指令技能，以及 Antigravity 標準外掛連結仍會投影。OpenCode 一律取得原生指令、原生代理程式與共用 `~/.agents/skills`。因此選取 OpenCode 時，即使 Codex 已註冊，GAL 仍會投影共用技能。

GAL 會在投影開始前觀察一次註冊狀態，並讓技能與指令階段共用這份不可變快照。執行環境註冊是路線判斷的唯一依據，不再使用另一個 `pluginMode` 偏好設定，因此同一次 refresh 不會混用互相矛盾的狀態。若證據缺少或無法讀取，GAL 會選擇投影後備方案，因為保留 GAL 管理的重複項，比在尚未確認外掛擁有權時刪除後備項目更安全。

GAL 僅在 `_galProjection` 命名空間內操作。其他外部鍵既不會被解析，也不會被修改。

## 標準 MCP manifest（`.mcp.json`）

`~/.gal/plugins/gal/.mcp.json` 會在 `gal refresh` 期間依 Core-Wins 規則合併 `plugins/gal-core/mcp.json` 與 `~/.gal/local/mcp.json` 中的個人伺服器後產生：

| 欄位 | 必填 | 型別 | 說明 |
| --- | --- | --- | --- |
| `mcpServers` | 是 | Object | 符合 `plugins/gal-core/mcp.json` 結構的伺服器定義。 |
| `inputs` | 否 | Array | 原樣傳遞的提示驅動輸入定義。 |

`plugins/gal-core/mcp.json` 預設包含空的伺服器物件。沒有個人伺服器的機器會產生空 manifest。環境變數語法（`${VAR}`）會原樣保留，MCP host 在執行期解析。GAL 不會在 MCP manifest 中儲存認證資料或祕密。產生期間，來源的 `"servers"` 鍵會改名為 `"mcpServers"`。套件根目錄也包含符合 Agent Plugins 1.0.0 規範的 `mcp.json`。`gal update` 只顯示版本資訊，manifest 會透過 `gal refresh` 或 `gal dev-apply` 重新產生。

## MCP 設定邊界

GAL 只管理自己的外掛 manifest。它絕不會編輯主機管理的 `claude_desktop_config.json`、`~/.copilot/mcp-config.json`、`~/.codex/config.toml`、`opencode.json` 或 `mcp_config.json`。主機工具設定由使用者管理。

## 變更後重新產生

`plugins/gal-core/` 的變更不會自動熱載入：

- **修改 `plugins/gal-core/`**：從檢出根目錄執行 `gal dev-apply`。
- **修改 `crates/` 中的 Rust 程式碼**：先執行 `cargo build --release -p gal-cli` 並將二進位檔安裝至 `PATH`，再從檢出根目錄執行 `gal dev-apply`。

| 修改元件 | 必要指令 | 驗證步驟 |
| --- | --- | --- |
| `plugins/gal-core/` 的技能、代理程式、指令或慣例 | 從檢出根目錄執行 `gal dev-apply` | 在目標執行環境執行更新後的指令。 |
| 核心 MCP 設定（`plugins/gal-core/mcp.json`） | `gal dev-apply` | 在 MCP host 重新載入伺服器。 |
| 個人 MCP 設定（`~/.gal/local/mcp.json`） | `gal refresh` | 在 MCP host 重新載入伺服器。 |
| `crates/` 的 Rust 原始程式碼 | 重新編譯並將二進位檔安裝至 `PATH` | 執行 `gal dev-apply`。 |
| `.dev/project.md` 或 Rust 慣例 | 儲存來源檔案 | 執行 `gal render-adapters`。 |
| `~/.gal/config/config.json` | 儲存設定檔 | 不經投影流程，立即生效。 |

請從檢出的根目錄執行 `gal dev-apply` 來部署該檢出版本，因為 `gal refresh` 會算繪已安裝的 GAL，並忽略此檢出版本。請區分 `gal render-adapters`（更新儲存庫本機轉接器）與 `gal refresh`（更新機器層級代理程式投影）。若要重設已初始化的儲存庫，請先手動刪除 `.dev/project.md` 與 `.dev/state.md`，再執行 `gal init`。

### 診斷偵測（`gal doctor`）

`gal doctor` 會偵測四種設定漂移：

1. **技能投影差異**：標記與標準來源不同的投影技能。
2. **opencode 投影漂移**：標示 `~/.config/opencode/` 中與標準算繪結果不同的檔案。執行 `gal refresh` 可修復。
3. **二進位檔偏差**：比較 `GAL_GIT_STAMP` 與 `git rev-parse --short HEAD`。不一致表示修改了 `crates/` 卻未重新編譯 `PATH` 上的二進位檔。
4. **外掛註冊證據**：`gal doctor` 與投影共用相同的註冊觀察器。證據不明時會列出來源與原因並發出警告。註冊證據只代表本機設定，不能證明執行中的環境已載入外掛。既有 `pluginMode` 值會由過時鍵檢查回報。`gal doctor --dry-run` 會被接受，但所有檢查都是唯讀，因此此旗標不會改變結果。

評估新的 AI 執行環境時，請確認它是否支援機器層級設定目錄、能否直接使用 `AGENTS.md`、是否支援原生指令或技能路徑，以及是否提供非破壞性的設定擴充機制。代理程式重新命名或路由變更後，維護者必須執行 `gal dev-apply`，並在本機儲存庫執行 `gal render-adapters`。

## 驗證清單

更新算繪或投影邏輯時，請確認：

- 所有支援的執行環境都有有效的投影目標目錄。
- `commands/*/SKILL.md` 中沒有未展開的範本變數，例如 `{{GAL_ROOT}}`。
- Antigravity 指令技能能在 `~/.gemini/antigravity-cli/skills/<name>/SKILL.md` 正常重新產生。
- 共用技能目錄只包含通用技能，沒有重複別名。
- 沒有修改主機層級 MCP 設定檔。
- `gal doctor` 透過 `DoctorPathContext::from_standard_paths` 一次解析系統路徑，而不是個別呼叫 `user_home()`。

自動化測試必須驗證 `VALID_RUNTIMES` 中所有執行環境的投影檔案與標準建置合約逐位元組相同。不同環境只能在傳輸媒介與能力路由層級存在差異。不要手動編輯 `AGENTS.md` 或投影技能，所有更新都必須從來源合約經由 `gal render-adapters`、`gal refresh` 或 `gal dev-apply` 產生。
