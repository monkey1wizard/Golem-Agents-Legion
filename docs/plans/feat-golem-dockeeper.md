# 規劃：文件守護者 Golem（golem-dockeeper）與活文件結構

## 目標

建立一個新的 GAL 專責代理人 `golem-dockeeper`，負責「從專案開始就維持文件結構整潔」並「在每次小幅程式碼變更後同步更新文件」。它以一份人類可讀的 HTML 結構地圖作為文件骨架的單一視覺化來源，並把 graphify 與 codebase-memory-mcp 當作「選用的結構訊號來源」（advisory），用來偵測資料夾結構漂移與變更影響範圍，避免文件因程式碼演進而逐步腐化。

完成時必須成立的事實：每次有意義的程式碼變更後，受影響的文件區塊與結構地圖會被偵測、標記並更新，且更新流程不依賴任何單一外部工具是否安裝。

## 需求

- [ ] 新增專責代理人 `golem-dockeeper`，定位為「文件維護 / 結構同步」工具型 golem，遵循既有 `agent/*.agent.md` 的格式與啟動規則（working-hours、project_context、token-budget）。
- [ ] 提供一份 HTML「活文件結構地圖」（living structure map），以人類可瀏覽的單一檔案呈現：目錄樹、模組/層級、關鍵檔案與對應文件位置，作為文件骨架的視覺化索引。
- [ ] 代理人能在「小幅變更」後執行增量同步：偵測哪些目錄/檔案/介面變動，對映到需要更新的文件節點，只更新受影響範圍而非全量重寫。
- [ ] 將 graphify 與 codebase-memory-mcp 定義為「選用能力車道」（capability lane），透過共用 preflight 契約（`docs/collaborative-tools/checking-contract.md`）解析狀態；不可用時靜默降級為原生檔案閱讀，不得提示安裝。
- [ ] 釐清資料格式邊界：HTML 結構地圖是「人類可讀的文件產物」；codebase-memory-mcp 的底層是 SQLite 知識圖（非 HTML）；graphify 提供 `GRAPH_REPORT.md` 報告。代理人消費這些訊號，但不把它們當成文件的權威狀態。
- [ ] 不引入新的核心記憶層：仍以檔案系統為權威記憶，遵循 `conventions/token-budget.md` 的 File-System Memory Contract。
- [ ] 將代理人登錄進 `agent/agents.md` 註冊表，並透過 `gal init` / Sync-DevContext 重新生成配接器，使各 runtime 可發現此代理人。

## 方法

### 步驟 1：定義 golem-dockeeper 代理人契約

- **檔案**：`agent/golem-dockeeper.agent.md`
- **內容**：依現有 golem 格式撰寫 frontmatter（`name`、`description`、`tools`、`color`）與 `<role>`、`<classification>`、`<project_context>`、`<rules>` 區塊。
  - 分類：Category = Utility（獨立輔助，不綁定 pipeline 狀態），typical activation = 直接呼叫或在實作/審查 checkpoint 後觸發。
  - 核心職責：維護 HTML 結構地圖、偵測結構漂移、對映變更到文件節點、執行增量文件更新、把驗證過的結構知識沉澱到 `docs/`。
  - 規則：遵循 working-hours、token-budget（≤15% 指令預算、directed exploration、bounded output）、以及「不自動安裝/初始化外部工具」的 capability-first 規則。
- **驗證**：檔案格式與其他 `agent/*.agent.md` 一致；frontmatter 欄位齊全；可被 agents 註冊表索引。

### 步驟 2：設計 HTML 活文件結構地圖

- **檔案**：`docs/structure/structure-map.html`（產物）、`templates/structure-map.template.html`（來源範本）
- **內容**：定義一份自包含、零外部依賴的 HTML 範本，呈現：
  - 目錄樹與層級對映（對齊 CLAUDE.md 的 Layer Map）。
  - 每個模組/目錄 → 對應文件（`docs/`、`README`、plan）的連結節點。
  - 「最後同步時間」與「漂移狀態」標記欄位，供代理人寫入。
  - 以 `data-*` 屬性或內嵌 JSON 區塊承載結構資料，讓代理人能可靠地差異比對與部分更新。
- **驗證**：在瀏覽器開啟可正確顯示結構樹；代理人可解析其資料區塊並只更新變動節點。

### 步驟 3：定義增量同步流程

- **檔案**：`workflows/doc-sync.md`（新文件同步工作流契約）
- **內容**：
  - 觸發點：實作完成的 checkpoint、`/gal pipeline` 任務收尾、或使用者直接呼叫 `golem-dockeeper`。
  - 流程：偵測變更範圍 → 對映受影響文件節點 → 最小化更新（結構地圖 + 受影響 `docs/` 段落）→ 標記同步狀態。
  - 變更偵測來源優先序：原生 `git diff` 為基準；若 codebase-memory-mcp `ready` 則用其變更影響映射加強；若 graphify 報告存在則用社群/耦合訊號輔助判斷跨模組影響。
  - 寫回目標：HTML 結構地圖、相關 `docs/` 文件；不接管 plan / state 的權威狀態。
- **驗證**：對一次小幅變更走完流程，僅受影響節點被更新，未受影響文件不動。

### 步驟 4：定義選用能力車道契約（preflight 與降級）

- **檔案**：`docs/collaborative-tools/codebase-memory-mcp.md`（新契約）；於 `workflows/doc-sync.md` 引用 graphify 既有契約
- **內容**：
  - 比照 `graphify.md` 的 preflight 表格（`not-applicable` / `unavailable` / `available-but-needs-init` / `available-but-not-ready` / `ready`）。
  - codebase-memory-mcp 為真正的 MCP server（SQLite 知識圖，FTS5 + Cypher-like 查詢，快取於 `~/.cache/codebase-memory-mcp/`）。定義 GAL 消費它的方式：索引狀態查詢、git diff 影響映射、架構總覽生成、dead code 偵測作為文件漂移訊號。
  - 明確非目標：不自動索引、不自動安裝、缺席不得報錯或提示安裝；缺席時用原生 `git diff` + 檔案閱讀降級。
- **驗證**：在工具缺席的乾淨環境，doc-sync 流程仍可完整完成。

### 步驟 5：登錄與配接器再生

- **檔案**：`agent/agents.md`（註冊表）、由 `gal init` / `scripts/Sync-DevContext.ps1` 生成的配接器
- **內容**：將 `golem-dockeeper` 加入註冊表；於規劃文件中註明需重跑同步指令碼以再生 `CLAUDE.md`、`GEMINI.md`、`AGENTS.md`、`.github/copilot-instructions.md`。
- **驗證**：`/gal` 控制面可解析到新代理人；配接器檔案含其索引項。

## 建立或修改的檔案

- `agent/golem-dockeeper.agent.md` — 新代理人契約。
- `agent/agents.md` — 註冊新代理人。
- `templates/structure-map.template.html` — HTML 活文件結構地圖來源範本。
- `docs/structure/structure-map.html` — 生成的結構地圖產物（首版骨架）。
- `workflows/doc-sync.md` — 文件增量同步工作流契約。
- `docs/collaborative-tools/codebase-memory-mcp.md` — codebase-memory-mcp 選用能力車道與 preflight 契約。
- `conventions/token-budget.md`（小幅修改）— 在 capability-first 段落補充 doc-sync 對 graphify / codebase-memory-mcp 的降級引用（如有需要）。

## 測試案例

- [ ] 在乾淨 repo（無 graphify、無 codebase-memory-mcp）對一次小幅程式碼變更觸發 `golem-dockeeper`，驗證結構地圖與受影響文件被正確增量更新，且無安裝提示。
- [ ] 移動一個檔案造成資料夾結構漂移，驗證 HTML 結構地圖的目錄樹與漂移狀態標記被更新。
- [ ] 當 graphify `GRAPH_REPORT.md` 存在時，驗證 docs-sync 將跨模組耦合訊號納入受影響範圍判斷（advisory）。
- [ ] 當 codebase-memory-mcp `ready` 時，驗證以其 git diff 影響映射縮小更新範圍；當其 `unavailable` 時驗證降級為 `git diff`。
- [ ] 驗證 `golem-dockeeper` 指令集大小符合 token-budget ≤15% 指令預算。
- [ ] 驗證代理人不會把外部工具輸出當成文件權威狀態（檔案系統仍為唯一權威）。

## 成功標準

- [ ] 存在可運作的 `golem-dockeeper` 代理人，且能被 `/gal` 控制面與各 runtime 配接器發現。
- [ ] 存在一份可瀏覽的 HTML 活文件結構地圖，能顯示目錄/模組到文件的對映與同步狀態。
- [ ] 小幅變更後文件能增量同步，僅更新受影響範圍。
- [ ] graphify 與 codebase-memory-mcp 為純選用增強，缺席時流程零錯誤降級。
- [ ] 全流程不違反 File-System Memory Contract，未引入第二核心記憶層。

## 風險

- **HTML 產物與程式碼真實狀態漂移**：HTML 結構地圖若更新不及，反而成為誤導來源。
  - *緩解*：地圖內建「最後同步時間」與「漂移狀態」欄位；以 `git diff` 為基準的偵測為強制基礎，外部工具僅加強。
- **外部工具耦合風險**：把 codebase-memory-mcp / graphify 變成隱性必需依賴。
  - *緩解*：嚴格走 preflight 降級契約；缺席不得報錯或提示安裝。
- **記憶層重複**：codebase-memory-mcp 自帶 SQLite 知識圖，可能被誤當成 GAL 的權威記憶層。
  - *緩解*：契約明訂它僅為 advisory 結構訊號來源；權威記憶仍是 repo 內 Markdown / HTML 檔案。
- **代理人指令膨脹**：職責多元可能超出 token 預算。
  - *緩解*：把結構地圖格式細節與 preflight 細節移到獨立 workflow / 契約檔，代理人只引用。
- **跨 runtime 漂移**：新代理人未同步進所有配接器會破壞控制面一致性。
  - *緩解*：透過 `gal init` / Sync-DevContext 統一再生，不手改配接器。

## 未決問題

- [ ] OQ-001 — 最終形式應為「單一 golem 代理人」、或「代理人 + 一個產生 HTML 結構地圖的 skill」、或純 skill？（建議：golem 代理人為主，HTML 生成邏輯可後續抽成 skill）*(raised by: planning)*
- [ ] OQ-002 — 「小幅變更」的觸發是手動呼叫為主，或要與 `/gal pipeline` 任務收尾自動掛鉤？*(raised by: planning)*
- [ ] OQ-003 — HTML 結構地圖的範圍：只到目錄/模組層級，或要深入到函式/介面層級（後者更依賴 codebase-memory-mcp）？*(raised by: planning)*
- [ ] OQ-004 — codebase-memory-mcp 在 Windows 的安裝/連線是否已具備，將影響其車道預設狀態與測試可行性。*(raised by: planning)*

## 核准

- 人類核准：[待定]
- 架構審查：[待定]（建議走 `/deep-planning`：本變更新增代理人、新增工作流與契約、觸及 `templates/` 與 `agent/` 等受保護路徑，屬結構性變更）
- 額外領域審查：[未觸發]

## Review Results

### Architecture Review

Pending.

### Business Review

Pending.

### Design Review

Pending.

### Engineering Review

Pending.

## Test Plan

Pending.

## Tasks

Pending.
