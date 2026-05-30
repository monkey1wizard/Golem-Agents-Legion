# 規劃：文件守護者 Golem（golem-dockeeper）與活文件結構

## 目標

建立一個新的 GAL 專責代理人 `golem-dockeeper`，解決真正的問題：**每次改程式碼導致資料夾結構微幅變動後，文件逐步腐化、變得凌亂**。它的職責是「從專案開始就維持文件結構整潔」並「在每次小幅程式碼變更後同步更新文件」。

完成時必須成立的事實：每次有意義的程式碼變更後，受影響的文件區塊會被偵測、標記並更新，文件與真實程式碼結構保持一致。

### 範圍備註：中央結構檔為 AI-first，格式定為 NDJSON；外部工具為可選

使用者已澄清兩件事：

1. **中央結構地圖是 AI 在讀的機器狀態檔**，人類讀的是最終各個 `docs/`。因此格式以「解析確定性、schema 約束、精準局部更新、diff 可審」為準，**定案為 NDJSON（JSON Lines）**（見 OQ-001，已解決）。
2. **codebase-memory-mcp 由使用者在本計畫之外自行安裝**（OQ-005/006 已定案）——它是函式/介面層級的啟用器;本計畫**不修改 Setup-Machine 腳本**,改為經 preflight 能力車道消費。doc-sync **仍須在缺席時降級到模組層級**（CI、未裝該工具的環境皆可能無它）。graphify 維持 advisory-only。

無論是否接外部工具，代理人都必須能獨立運作。

### NDJSON 格式決策依據

- **每節點一行** → 小幅變更只換掉對應一行，局部更新不會誤傷其他節點；新增 append、刪除刪行。
- **逐行 diff** → commit 後一眼看出哪些節點被標記 drift，review 清楚。
- **每行皆合法 JSON** → 可逐行套 JSON Schema 驗證，防止 AI 反覆讀寫時把格式寫歪；解析零歧義。
- **訓練資料常見** → AI 讀寫可靠度高。
- **樹狀關係由 `path` 推導**，不做實體巢狀，避免巢狀編輯的 diff 噪音。
- 退路：若日後需要分組巢狀單一文件，退回純 JSON；規模成千上萬且需查詢，才考慮 SQLite（但會失去 git 可審查性，非本計畫範圍）。

## 需求

- [ ] 新增專責代理人 `golem-dockeeper`，定位為「文件維護 / 結構同步」工具型 golem，遵循既有 `agent/*.agent.md` 的格式與啟動規則（working-hours、project_context、token-budget）。
- [ ] 新增 `doc-sync` skill 承載「NDJSON 讀寫 + docs 同步」可重用程序，由 `golem-dockeeper` 載入（`required skills`），並可被主對話與其他代理人重用（OQ-002 已定案 Agent + Skill）。
- [ ] 提供一份 **NDJSON 活文件結構地圖**（living structure map），每行一個節點記錄，呈現目錄/模組/關鍵檔案與對應文件位置、層級、同步狀態與最後同步時間。樹狀關係由 `path` 推導。提供對應 JSON Schema 供逐行驗證。
- [ ] 代理人能在「小幅變更」後執行增量同步：偵測哪些目錄/檔案/介面變動，對映到需要更新的文件節點，只更新受影響範圍而非全量重寫。
- [ ] 變更偵測的**強制基礎**為原生 `git diff` + 檔案閱讀；graphify 與 codebase-memory-mcp 僅為「選用能力車道」（capability lane），透過共用 preflight 契約（`docs/collaborative-tools/checking-contract.md`）解析狀態；不可用時靜默降級，不得提示安裝。
- [ ] 若採用外部工具，釐清資料格式邊界：codebase-memory-mcp 底層是 SQLite 知識圖（非 HTML）、graphify 提供 `GRAPH_REPORT.md` 報告。代理人僅把它們當 advisory 訊號，不當成文件的權威狀態。
- [ ] codebase-memory-mcp 由使用者在計畫外自行安裝;doc-sync 經 preflight 能力車道消費它做函式/介面層級抽取,缺席時降級到模組層級,不報錯、不阻斷。本計畫不修改 Setup-Machine 腳本。
- [ ] 不引入新的核心記憶層：仍以檔案系統為權威記憶，遵循 `conventions/token-budget.md` 的 File-System Memory Contract。codebase-memory-mcp 的 SQLite 圖僅為 advisory 查詢來源,非權威記憶。
- [ ] 將代理人登錄進 `agent/agents.md` 註冊表，並透過 `gal init` / Sync-DevContext 重新生成配接器，使各 runtime 可發現此代理人。

## 方法

### 步驟 1：定義 golem-dockeeper 代理人契約

- **檔案**：`agent/golem-dockeeper.agent.md`
- **內容**：依現有 golem 格式撰寫 frontmatter（`name`、`description`、`tools`、`color`）與 `<role>`、`<classification>`、`<project_context>`、`<rules>` 區塊。
  - 分類：Category = Utility（獨立輔助），typical activation = `/gal pipeline` 任務收尾掛勾 + 直接呼叫;`required skills: doc-sync`。
  - 核心職責：維護 NDJSON 結構地圖、偵測結構漂移、對映變更到文件節點、執行增量文件更新、把驗證過的結構知識沉澱到 `docs/`。實際同步程序委派 `doc-sync` skill。
  - 規則：遵循 working-hours、token-budget（≤15% 指令預算、directed exploration、bounded output）、以及「不自動安裝/初始化外部工具」的 capability-first 規則。
- **驗證**：檔案格式與其他 `agent/*.agent.md` 一致；frontmatter 欄位齊全（含 `required skills: doc-sync`）；可被 agents 註冊表索引。

### 步驟 1b：建立 doc-sync skill

- **檔案**：`skills/doc-sync/SKILL.md`
- **內容**：承載可重用的同步程序——NDJSON 記錄結構與 schema 用法、逐行讀寫/局部更新規則、變更偵測來源優先序與降級、`docs/`/README 寫回規則。代理人與主對話皆載入此 skill 取得一致行為。
- **驗證**：SKILL.md 格式符合既有 `skills/*/SKILL.md`;description 能正確觸發;`golem-dockeeper` 與主對話載入後行為一致。

### 步驟 2：設計 NDJSON 活文件結構地圖

- **檔案**：`docs/structure/structure-map.ndjson`（產物）、`docs/structure/structure-map.schema.json`（逐行驗證用 JSON Schema）
- **記錄結構**（每行一個節點，深入到函式/介面層級——見 OQ-004）：
  - `id` — 唯一 key。對檔案層級用 `path`；對符號層級用 `path#symbol`（如 `src/auth/login.ts#loginUser`），樹狀/歸屬關係由此推導。
  - `kind` — `dir` / `module` / `file` / `interface` / `function` / `class`。
  - `layer` — 對齊 CLAUDE.md Layer Map 的層級。
  - `signature` — 函式/介面層級的簽名摘要（選填，供人查文件時對照）。
  - `docs` — 對應文件位置陣列（`docs/`、`README`、plan，可含錨點如 `docs/auth.md#login`）。
  - `syncStatus` — `ok` / `drift` / `missing` 等同步狀態。
  - `lastSynced` — 最後同步日期。
  - 範例（檔案層級）：`{"id":"src/auth/login.ts","kind":"file","layer":"service","docs":["docs/auth.md"],"syncStatus":"ok","lastSynced":"2026-05-30"}`
  - 範例（函式層級）：`{"id":"src/auth/login.ts#loginUser","kind":"function","layer":"service","signature":"loginUser(email, pwd): Result<Session>","docs":["docs/auth.md#login","README.md#auth"],"syncStatus":"drift","lastSynced":"2026-05-20"}`
- **慣例**：行內 key 排序穩定、以 `path` 排序行序，使 diff 穩定可審。
- **驗證**：代理人能逐行解析、套 schema 驗證、只換掉變動節點所在行；新增 append、刪除刪行；未變動行 byte 不動。

### 步驟 3：定義增量同步流程

- **檔案**：`workflows/doc-sync.md`（新文件同步工作流契約）
- **內容**：
  - 觸發點（OQ-003 已定案）：**主要掛勾 `/gal pipeline` 任務收尾**——在每個 T-NNN 任務通過 implement/test/review 的 commit gate 後執行 doc-sync,使文件隨任務逐步同步;**並支援使用者直接呼叫 `golem-dockeeper`** 做手動補同步。
  - 流程：偵測變更範圍 → 對映受影響節點 → 最小化更新（NDJSON 對應行 + 受影響 `docs/` 段落）→ 更新該行 `syncStatus` / `lastSynced`。
  - 變更偵測來源優先序（受 OQ-004 函式/介面層級影響）：原生 `git diff` 偵測「哪些檔案變動」為強制基礎;**符號（函式/介面）層級的抽取與 diff→symbol 影響映射,若 codebase-memory-mcp `ready` 則用它（這正是它的強項）**;若 graphify 報告存在則用社群/耦合訊號輔助跨模組影響。**降級策略**:無 codebase-memory-mcp 時,doc-sync 退化為「檔案/模組層級」精度仍可完整運作,函式層級節點維持上次狀態並標記 `stale-granularity`,不報錯、不阻斷。
  - 寫回目標：NDJSON 結構地圖（逐行）、相關 `docs/` 文件；不接管 plan / state 的權威狀態。
- **驗證**：對一次小幅變更走完流程，僅受影響節點被更新，未受影響文件不動。

### 步驟 4：能力車道契約（OQ-005 已定案）

- **檔案**：`docs/collaborative-tools/codebase-memory-mcp.md`（新契約）;於 `workflows/doc-sync.md` 引用 graphify 既有契約。**不修改 Setup-Machine 腳本**——安裝由使用者計畫外自理。
- **契約內容**：
  - 比照 `graphify.md` 的 preflight 表格（`not-applicable` / `unavailable` / `available-but-needs-init` / `available-but-not-ready` / `ready`）。
  - codebase-memory-mcp 為真正的 MCP server（SQLite 知識圖，FTS5 + Cypher-like 查詢，快取於 `~/.cache/codebase-memory-mcp/`）。GAL 消費方式：索引狀態查詢、git diff→symbol 影響映射、架構總覽、dead code 偵測作為函式層級漂移訊號。
  - doc-sync **依 preflight 降級**:`ready` → 函式層級;非 `ready` → 退模組層級並標記 `stale-granularity`;絕不報錯、不阻斷、不在執行期硬性提示安裝。
- **驗證**：工具 `ready` 時 doc-sync 達函式層級;未安裝（任一非 `ready` 狀態）時 doc-sync 仍能完整完成於模組層級。

### 步驟 5：登錄與配接器再生

- **檔案**：`agent/agents.md`（註冊表）、由 `gal init` / `scripts/Sync-DevContext.ps1` 生成的配接器
- **內容**：將 `golem-dockeeper` 加入註冊表；新 `doc-sync` skill 需被配接器的 Repo Skills 索引收錄;重跑同步指令碼以再生 `CLAUDE.md`、`GEMINI.md`、`AGENTS.md`、`.github/copilot-instructions.md`。
- **驗證**：`/gal` 控制面可解析到新代理人;配接器檔案含代理人與 `doc-sync` skill 的索引項。

## 建立或修改的檔案

核心（必做）：

- `agent/golem-dockeeper.agent.md` — 新代理人契約（`required skills: doc-sync`）。
- `skills/doc-sync/SKILL.md` — 可重用同步程序 skill（NDJSON 讀寫 + docs 同步）。
- `agent/agents.md` — 註冊新代理人。
- `workflows/doc-sync.md` — 文件增量同步工作流契約（以 `git diff` 為強制基礎）。
- `docs/structure/structure-map.ndjson` — NDJSON 活文件結構地圖產物首版骨架。
- `docs/structure/structure-map.schema.json` — 每行節點記錄的 JSON Schema。
- `templates/structure-map.template.ndjson` — 結構地圖來源範本（含 NDJSON 節點記錄樣板）。

外部工具(OQ-005 已定案,codebase-memory-mcp 由使用者計畫外自行安裝)：

- `docs/collaborative-tools/codebase-memory-mcp.md` — 能力車道與 preflight 契約。
- `conventions/token-budget.md`（小幅修改）— 在 capability-first 段落補充 doc-sync 對 codebase-memory-mcp（preflight、降級）與 graphify（advisory）的引用。

## 測試案例

- [ ] 在乾淨 repo（無 graphify、無 codebase-memory-mcp）對一次小幅程式碼變更觸發 `golem-dockeeper`，驗證結構地圖與受影響文件被正確增量更新，且無安裝提示。
- [ ] 移動一個檔案造成資料夾結構漂移，驗證 NDJSON 結構地圖對應行的 path/`syncStatus` 被更新，且未受影響行 byte 不動。
- [ ] 驗證 NDJSON 每行皆通過 `structure-map.schema.json` 驗證；故意寫入不合法行能被偵測。
- [ ] 當 graphify `GRAPH_REPORT.md` 存在時，驗證 docs-sync 將跨模組耦合訊號納入受影響範圍判斷（advisory）。
- [ ] 當 codebase-memory-mcp `ready` 時，驗證 doc-sync 達函式/介面層級精度（以其 diff→symbol 影響映射）；當其 `unavailable` 時驗證降級到模組層級並標記 `stale-granularity`，流程不中斷。
- [ ] 驗證 `golem-dockeeper` 指令集大小符合 token-budget ≤15% 指令預算。
- [ ] 驗證代理人不會把外部工具輸出當成文件權威狀態（檔案系統仍為唯一權威）。

## 成功標準

- [ ] 存在可運作的 `golem-dockeeper` 代理人與 `doc-sync` skill，且能被 `/gal` 控制面與各 runtime 配接器發現。
- [ ] 存在一份 schema-validated 的 NDJSON 活文件結構地圖，記錄目錄/模組到文件的對映與同步狀態，且支援逐行精準增量更新。
- [ ] 小幅變更後文件能增量同步，僅更新受影響範圍。
- [ ] graphify 與 codebase-memory-mcp 為純選用增強，缺席時流程零錯誤降級。
- [ ] 全流程不違反 File-System Memory Contract，未引入第二核心記憶層。

## 風險

- **結構地圖與程式碼真實狀態漂移**：地圖若更新不及，反而成為誤導來源。
  - *緩解*：每行內建 `lastSynced` 與 `syncStatus` 欄位；以 `git diff` 為基準的偵測為強制基礎，外部工具僅加強。
- **AI 反覆讀寫導致格式漂移**：代理人多次編輯可能寫出不合法行。
  - *緩解*：每行套 `structure-map.schema.json` 驗證為硬門檻；更新後驗證失敗則拒絕寫入。
- **外部工具耦合風險**：把 codebase-memory-mcp / graphify 變成隱性必需依賴。
  - *緩解*：嚴格走 preflight 降級契約；缺席不得報錯或提示安裝。
- **記憶層重複**：codebase-memory-mcp 自帶 SQLite 知識圖，可能被誤當成 GAL 的權威記憶層。
  - *緩解*：契約明訂它僅為 advisory 結構訊號來源；權威記憶仍是 repo 內 Markdown / NDJSON 檔案。
- **代理人指令膨脹**：職責多元可能超出 token 預算。
  - *緩解*：把結構地圖格式細節與 preflight 細節移到獨立 workflow / 契約檔，代理人只引用。
- **跨 runtime 漂移**：新代理人未同步進所有配接器會破壞控制面一致性。
  - *緩解*：透過 `gal init` / Sync-DevContext 統一再生，不手改配接器。
- **函式層級節點規模膨脹**（OQ-004 連動）：深入函式/介面層級後,大型 repo 的 NDJSON 行數可能爆量,拖慢讀寫並升高 token。
  - *緩解*：以變更範圍驅動的 lazy 填充(只記錄被觸碰過或有對應文件的符號);必要時依頂層模組切分多個 `structure-map.<module>.ndjson`;函式層級抽取優先委派 codebase-memory-mcp,避免代理人自行全量剖析。
- **假定外部工具必然存在**：因 codebase-memory-mcp 由使用者自行安裝,實作易誤把它當必備而省略降級。
  - *緩解*：preflight 降級為硬性需求;測試明確覆蓋「未安裝→模組層級」路徑,確保預設它在仍不等於硬依賴。

## 未決問題

- [x] OQ-001 — **結構地圖格式**：已定案 **NDJSON（JSON Lines）**。理由：中央結構檔為 AI-first 機器狀態檔，NDJSON 提供逐行精準局部更新、最小 diff、逐行 JSON Schema 驗證與高解析確定性。退路：需巢狀分組退純 JSON；規模爆炸才考慮 SQLite。*(raised by: planning, resolved by: planning)*
- [x] OQ-002 — **最終形式**：已定案 **Agent + Skill**。`golem-dockeeper` 代理人當文件狀態擁有者與 pipeline 掛勾點,把「NDJSON 讀寫 + docs 同步」程序抽成獨立 skill,供代理人載入,也讓主對話與其他代理人重用（比照 `golem-researcher` + `local-first-search`）。*(raised by: planning, resolved by: user)*
- [x] OQ-003 — **觸發時機**：已定案。主要掛勾 `/gal pipeline` 任務收尾（commit gate 後同步），並支援手動呼叫 `golem-dockeeper`。*(raised by: planning, resolved by: planning)*
- [x] OQ-004 — **結構地圖深度**：已定案,深入到**函式/介面層級**。理由:使用者讀文件時要能定位到需要的東西、讓 AI 維護 README 等文件更容易、且利於 AI 查詢。連動影響:NDJSON 節點新增符號層級記錄、節點規模上升、並使 codebase-memory-mcp 從「可有可無」上升為「函式層級抽取的建議啟用器」（見 OQ-005、新增規模風險）。*(raised by: planning, resolved by: planning)*
- [x] OQ-005 — **外部工具策略**：已定案。**codebase-memory-mcp 由使用者在本計畫之外自行安裝**,本計畫不修改 Setup-Machine 腳本。doc-sync 將其視為 preflight 能力車道:`ready` → 函式/介面層級;非 `ready` → 降級到模組層級並標記 `stale-granularity`,不報錯、不阻斷。graphify 維持 advisory-only（已安裝,有報告則用）。*(raised by: planning, resolved by: user)*
- [x] OQ-006 — codebase-memory-mcp 安裝：已定案由使用者計畫外自行處理（其 repo 提供 Windows `install.ps1`,可行性已確認）;不在本計畫範圍。*(raised by: planning, resolved by: user)*

## 核准

- 人類核准：[待定]
- 架構審查：[待定]（建議走 `/deep-planning`：本變更新增代理人與 skill、新增工作流與契約,並觸及受保護的 `templates/`、`agent/`、`conventions/`,屬結構性變更。不修改 Setup-Machine 腳本。）
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
