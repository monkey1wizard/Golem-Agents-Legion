# 規劃：文件守護者 Golem（golem-dockeeper）與活文件結構

## 目標

建立一個新的 GAL 專責代理人 `golem-dockeeper`，當**專責 doc manager**，把文件維護從 implement 等任務抽出來成獨立 agent，解決真正的問題：**改程式碼導致結構微幅變動後，文件逐步腐化、多個 plan 疊加後嚴重偏移，而人工難以完整抓出哪裡歪掉**。

它的職責是「從專案開始維持文件結構整潔」並「在每次小幅程式碼變更後同步更新文件」。分工原則：**機器負責窮舉找出「哪裡缺、哪裡舊」（人腦會漏的部分），人/AI 負責「怎麼寫對」**。

完成時必須成立的事實：每次有意義的程式碼變更後，受影響的文件區塊會被偵測、標記並更新，並能透過跨 plan 對帳補抓累積漂移。

## 關鍵決策

| # | 決策 | 結論 | 一句話理由 |
| --- | --- | --- | --- |
| D1 | 結構地圖格式 | **NDJSON（JSON Lines）** | AI-first 機器狀態檔；逐行精準局部更新、最小 diff、逐行 JSON Schema 驗證、解析零歧義 |
| D2 | 最終形式 | **Agent + Skill**（`golem-dockeeper` + `doc-sync`） | 代理人當狀態擁有者與 pipeline 掛勾點，程序抽成 skill 供主對話與其他代理人重用（比照 `golem-researcher` + `local-first-search`） |
| D3 | 結構地圖模型 | **雙軸**：code 軸（檔案粒度）+ doc 軸（章節） | 原始問題是文件腐化，文件章節才是更新與閱讀單位；節點數由文件規模約束、非程式碼規模，避免函式層級爆量。符號/函式層級不進地圖 |
| D4 | 觸發 / 運行模式 | **增量**（pipeline 收尾）+ **對帳**（手動，`lastSyncedRef..HEAD` 全 repo） | 增量保持整潔；對帳靠各節點記住的 commit SHA，補抓 pipeline 外與跨 plan 累積漂移 |
| D5 | 外部工具策略 | graphify、codebase-memory-mcp 皆 **advisory-only** | `git diff` 檔案層級為強制基礎；缺席完整運作不降級；不改 Setup-Machine 腳本，安裝由使用者計畫外自理 |
| D6 | 人類視圖 | 從 NDJSON **即時渲染**待辦清單 + 樹狀架構視圖 | NDJSON 機器讀寫好但扁平、人看不出形狀；投影即時生成、不落地成 tracked 檔（守 File-System Memory Contract 最小檔面） |

> **範圍邊界（無 bin）**：本計畫只交付 agent + skill + NDJSON 契約，全部由 AI runtime 直接執行，**不含任何編譯二進位**。其中確定性、機械性的引擎邏輯（git diff 分類、NDJSON schema 驗證/原子改行、覆蓋掃描、投影渲染等）在本計畫一律由 AI 執行。是否日後將這些機械邏輯原生化（編譯執行檔）屬獨立議題，**不在本計畫範圍**；本計畫自成完整、可獨立交付。

### 中間層定位

```text
機器完整圖譜                  本計畫的中間層                  最終文件
(graphify /                  structure-map.ndjson           docs/ + README
 codebase-memory-mcp)   ──►  (唯一 tracked 權威狀態)    ──►  (人讀、易腐化)
機器產物，人不可直讀           ├─ 待辦清單投影 (!= ok)
                             └─ 樹狀架構視圖投影        ◄── golem-dockeeper 維護
```

`golem-dockeeper` 是這層的擁有者：graphify / codebase-memory-mcp 的機器圖當 advisory 輸入，dockeeper 翻成人能審、能交辦的 NDJSON + 即時投影。

### NDJSON 格式決策依據（D1）

- **每節點一行** → 小幅變更只換對應一行，不誤傷其他節點；新增 append、刪除刪行。
- **逐行 diff** → commit 後一眼看出哪些節點被標 drift。
- **每行皆合法 JSON** → 可逐行套 JSON Schema 驗證，防止 AI 反覆讀寫把格式寫歪。
- **樹狀關係由 `path`/`anchor` 推導**，不做實體巢狀，避免巢狀編輯的 diff 噪音。
- 退路：需巢狀分組退純 JSON；規模成千上萬且需查詢才考慮 SQLite（會失去 git 可審查性，非本計畫範圍）。

## 需求

- [ ] 新增專責代理人 `golem-dockeeper`（工具型 golem），遵循既有 `agent/*.agent.md` 格式與啟動規則（working-hours、project_context、token-budget）。
- [ ] 新增 `doc-sync` skill 承載「NDJSON 讀寫 + docs 同步 + 投影渲染」可重用程序，由 `golem-dockeeper` 載入（`required skills`），並可被主對話與其他代理人重用。
- [ ] 提供 **NDJSON 活文件結構地圖**（雙軸：code 軸目錄/模組/檔案 + doc 軸章節），含 JSON Schema 供逐行驗證。
- [ ] 代理人能在小幅變更後執行增量同步：偵測變動檔案 → 對映受影響 code/doc 節點 → 只更新受影響範圍。
- [ ] 代理人能執行對帳：以各節點 `lastSyncedRef..HEAD` 全 repo 掃描，抓出 pipeline 外與跨 plan 累積漂移。
- [ ] 變更偵測**強制基礎**為原生 `git diff` + 檔案閱讀；graphify 與 codebase-memory-mcp 僅為選用能力車道，透過共用 preflight 契約（`docs/collaborative-tools/checking-contract.md`）解析狀態，不可用時靜默降級、不提示安裝。
- [ ] 不引入新的核心記憶層：檔案系統為權威記憶，遵循 `conventions/token-budget.md` 的 File-System Memory Contract；codebase-memory-mcp 的 SQLite 圖僅為 advisory 查詢來源。
- [ ] 將代理人登錄進 `agent/agents.md` 註冊表，透過 `gal init` / Sync-DevContext 重新生成配接器，使各 runtime 可發現。

## 方法

### 步驟 1：定義 golem-dockeeper 代理人契約

- **檔案**：`agent/golem-dockeeper.agent.md`
- **內容**：依現有 golem 格式撰寫 frontmatter（`name`、`description`、`tools`、`color`）與 `<role>`、`<classification>`、`<project_context>`、`<rules>`。
  - 分類：Category = Utility（獨立輔助）；typical activation = `/gal pipeline` 任務收尾掛勾 + 直接呼叫；`required skills: doc-sync`。
  - 核心職責：維護 NDJSON 結構地圖、偵測漂移、對映變更到文件節點、執行增量同步與對帳、渲染人類視圖、把驗證過的結構知識沉澱到 `docs/`。實際程序委派 `doc-sync` skill。
  - 規則：遵循 working-hours、token-budget（≤15% 指令預算、directed exploration、bounded output）、capability-first（不自動安裝/初始化外部工具）。
- **驗證**：格式與其他 `agent/*.agent.md` 一致；frontmatter 齊全（含 `required skills: doc-sync`）；可被註冊表索引。

### 步驟 1b：建立 doc-sync skill

- **檔案**：`skills/doc-sync/SKILL.md`
- **內容**：可重用程序——NDJSON 記錄結構與 schema 用法、逐行讀寫/局部更新規則、變更偵測來源優先序與降級、`docs/`/README 寫回規則、從 NDJSON 即時渲染待辦清單與樹狀架構視圖的投影規則。代理人與主對話皆載入此 skill 取得一致行為。
- **驗證**：格式符合既有 `skills/*/SKILL.md`；description 能正確觸發；代理人與主對話載入後行為一致。

### 步驟 2：設計 NDJSON 活文件結構地圖（D3）

- **檔案**：`docs/structure/structure-map.ndjson`（產物）、`docs/structure/structure-map.schema.json`（逐行驗證 JSON Schema）
- **共用欄位**：
  - `id` — 唯一且穩定 key（見各軸規則）。
  - `axis` — `code` / `doc`。
  - `kind` — code 軸：`dir` / `module` / `file`；doc 軸：`doc-section`。
  - `syncStatus` — `ok` / `drift` / `missing`。
  - `lastSynced` — 最後同步日期（顯示用）。
  - `lastSyncedRef` — 最後核對對應的 commit SHA。**對帳基準**：`git diff <lastSyncedRef>..HEAD` 吐出此節點自上次說真話以來的完整 diff，使累積漂移無法靜默複利。新節點為 `null`。
- **code 軸（`axis:"code"`）** — 偵測「有程式碼卻無文件」：
  - `id` = `path`（如 `src/auth/login.ts`）；樹狀關係由 `path` 推導。
  - `layer` — 對齊 CLAUDE.md Layer Map。
  - `docs` — 對應文件位置陣列（可含錨點）。空陣列 + 屬「應文件化」範圍 → `syncStatus:"missing"`。
  - 範例：`{"id":"src/auth/login.ts","axis":"code","kind":"file","layer":"service","docs":["docs/auth.md#login"],"syncStatus":"ok","lastSynced":"2026-05-30"}`
- **doc 軸（`axis:"doc"`）** — 偵測「章節與程式碼是否同步」：
  - `id` = `docPath#slug`；slug 用穩定 anchor，標題改名走節點生命週期（步驟 3），不以標題文字當 id。
  - `headingLevel` — 1–6；**預設只物化 h2/h3，h4–h6 僅在被 `codeRefs` 指向或實際變動時 lazy 物化**。
  - `title` — 章節標題（顯示用）。
  - `codeRefs` — 此章節文件化的程式碼路徑陣列（可含 `#symbol` 作 advisory 細節）。
  - 範例：`{"id":"docs/auth.md#login","axis":"doc","kind":"doc-section","headingLevel":3,"title":"Login","codeRefs":["src/auth/login.ts"],"syncStatus":"drift","lastSynced":"2026-05-20"}`
- **慣例**：行內 key 排序穩定；行序先 `axis` 再 `id`，使 diff 穩定可審。
- **覆蓋策略**：code 軸目標為**全 codebase 檔案粒度覆蓋**（行數=檔案數，給出完整 `missing` 偵測）；doc 軸覆蓋現有章節；符號層級不進地圖。
- **驗證**：逐行解析、套 schema 驗證（含 `axis` 條件式必填）、只換變動節點所在行；未變動行 byte 不動。

### 步驟 3：定義同步工作流（D4）

- **檔案**：`workflows/doc-sync.md`
- **運行模式**：
  - **增量（incremental）** — pipeline 任務收尾觸發；diff 範圍 = 該任務 commit 範圍；邊做邊保持整潔。
  - **對帳（reconcile）** — 手動呼叫 `golem-dockeeper` 觸發；diff 範圍 = 各節點 `lastSyncedRef..HEAD`，全 repo；抓 pipeline 外手動 commit 與多 plan 疊加漂移（git 記得所有變更，對帳只需各節點記得上次核對的 SHA）。
- **即時投影輸出**（皆從 NDJSON 渲染，不落地成 tracked 檔；NDJSON 為唯一 tracked 權威狀態）：
  - **待辦清單** — 濾出 `syncStatus != ok` 的行當 punch-list。
  - **架構樹視圖** — 依 `path`/`anchor` 渲染樹狀/縮排圖，每節點標 `syncStatus`（`✅ ok` / `⚠ missing` / `✏ drift`）與對應 docs，讓人一眼看出架構形狀與腐化處。
- **核心流程**：偵測變更範圍 → 對映受影響節點（code + doc 軸）→ 最小化更新（NDJSON 對應行 + 受影響 `docs/` 段落）→ 更新 `syncStatus` / `lastSynced` / `lastSyncedRef`。
- **變更偵測來源優先序**：原生 `git diff` 偵測「哪些檔案變動」為**強制基礎**；codebase-memory-mcp 若 `ready` 提供 advisory 符號訊號（精準命中含 `#symbol` 的 doc 章節）；graphify 報告存在則用社群/耦合訊號輔助跨模組影響。三者皆 advisory，缺席時檔案層級完整運作、不降級、不報錯、不阻斷。
- **新鮮度規則**：doc-sync **只能更新本次實際重新核對過的節點**；未核對節點一律保留先前狀態，禁止整批翻 `ok`。對 M(修改) 節點**保守標 `drift`（待複查）**，不擅自宣稱 `ok`、不擅自改正文（程式碼變不必然讓文件失效，屬啟發式判斷）。
- **節點生命週期**：以 `git diff -M --name-status` 窮舉分類路徑變化（`-M` 才認得改名，否則誤判成刪+加、製造假 `missing`）。**A**(新增)且無 doc → `missing`；**D**(刪除) → 刪 code 軸行，其 `codeRefs` 指向失效的 doc 軸行標 `drift`（不自動刪 doc 正文）；**R**(改名) → 更新 `id`/`path` 並標 `drift`；doc 標題改名 → 更新 `id` slug + `title` 並標 `drift`，同步修正其他行對舊 anchor 的參照。
- **commit 邊界**：doc-sync 的 NDJSON + `docs/` 寫入**另開獨立 `docs(sync): <task>` commit**，置於任務 commit 之後；不得 amend 已過 gate 的任務 commit。doc-sync 失敗不阻斷 pipeline；空變更不產生空 commit。
- **寫回目標**：NDJSON 結構地圖（逐行）、相關 `docs/` 文件；不接管 plan / state 的權威狀態。
- **驗證**：對一次小幅變更走完流程，僅受影響節點被更新，未觸碰節點 `syncStatus` 不變，寫入落在獨立 `docs(sync):` commit。

### 步驟 4：能力車道契約（D5）

- **檔案**：`docs/collaborative-tools/codebase-memory-mcp.md`（新契約）；於 `workflows/doc-sync.md` 引用 graphify 既有契約。不修改 Setup-Machine 腳本。
- **契約內容**：
  - 比照 `graphify.md` 的 preflight 表格（`not-applicable` / `unavailable` / `available-but-needs-init` / `available-but-not-ready` / `ready`）。**`ready` 以 MCP 語意定義**：MCP server 可達 **且** `index_status` 顯示本 repo 已索引（非比照 graphify 的「報告檔存在」）。
  - codebase-memory-mcp 為真正的 MCP server（SQLite 知識圖，FTS5 + Cypher-like 查詢，快取於 `~/.cache/codebase-memory-mcp/`）。GAL **advisory** 消費：`index_status` 查詢、git diff→symbol 影響映射、架構總覽、dead code 作漂移輔助訊號。
  - doc-sync **始終以 `git diff` 檔案層級為基礎**；`ready` 僅增強符號命中精度；非 `ready` 不降級主流程、不報錯、不阻斷、不在執行期硬性提示安裝。
- **驗證**：`ready` 時對含 `#symbol` 的 doc 章節有更精準命中；未安裝時仍完整完成於檔案層級，結果無缺漏。

### 步驟 5：登錄與配接器再生

- **檔案**：`agent/agents.md`（註冊表）、由 `gal init` / `scripts/Sync-DevContext.ps1` 生成的配接器
- **內容**：將 `golem-dockeeper` 加入註冊表；`doc-sync` skill 經配接器 Repo Skills 索引收錄；重跑同步指令碼再生 `CLAUDE.md`、`GEMINI.md`、`AGENTS.md`、`.github/copilot-instructions.md`。
- **驗證**：`/gal` 控制面可解析到新代理人；配接器含代理人與 `doc-sync` skill 索引項。

## 建立或修改的檔案

核心（必做）：

- `agent/golem-dockeeper.agent.md` — 新代理人契約（`required skills: doc-sync`）。
- `skills/doc-sync/SKILL.md` — 可重用同步程序 skill（NDJSON 讀寫 + docs 同步 + 投影渲染）。
- `agent/agents.md` — 註冊新代理人。
- `workflows/doc-sync.md` — 同步工作流契約（增量 + 對帳，以 `git diff` 為強制基礎）。
- `docs/structure/structure-map.ndjson` — NDJSON 活文件結構地圖首版骨架。
- `docs/structure/structure-map.schema.json` — 每行節點記錄的 JSON Schema。
- `templates/structure-map.template.ndjson` — 結構地圖來源範本。

外部工具（codebase-memory-mcp 由使用者計畫外自行安裝）：

- `docs/collaborative-tools/codebase-memory-mcp.md` — 能力車道與 preflight 契約。
- `conventions/token-budget.md`（小幅修改）— capability-first 段落補充 doc-sync 對 codebase-memory-mcp（preflight、降級）與 graphify（advisory）的引用。

## 測試案例

- [ ] 乾淨 repo（無 graphify、無 codebase-memory-mcp）對一次小幅變更觸發同步，驗證結構地圖與受影響文件被正確增量更新，且無安裝提示。
- [ ] 移動一檔造成結構漂移，驗證 NDJSON 對應行的 path/`syncStatus` 被更新，未受影響行 byte 不動。
- [ ] 驗證 NDJSON 每行皆通過 `structure-map.schema.json`；故意寫入不合法行能被偵測。
- [ ] graphify `GRAPH_REPORT.md` 存在時，驗證跨模組耦合訊號被納入受影響範圍判斷（advisory）。
- [ ] codebase-memory-mcp `ready` 時驗證對含 `#symbol` 的 doc 章節有更精準命中；`unavailable` 時驗證以檔案層級完整完成、無缺漏、不中斷。
- [ ] 新鮮度：對只觸碰部分檔案的變更跑同步，驗證未觸碰節點的 `syncStatus`/`lastSynced` 保持不變（不得整批翻 `ok`）。
- [ ] 節點生命週期：刪除一檔，驗證 code 軸行被刪、指向失效的 doc 軸行被標 `drift`；改名一檔，驗證 `id`/`path` 更新並標 `drift`。
- [ ] commit 邊界：pipeline 任務收尾同步，驗證寫入落在獨立 `docs(sync):` commit，任務 commit 未被 amend。
- [ ] 對帳模式：連續多次 pipeline 外手動 commit 後跑對帳，驗證以 `lastSyncedRef..HEAD` 抓出累積漂移、待辦清單列出 `missing`/`drift`/`orphan`，未核對節點狀態不變。
- [ ] 樹視圖投影：從 NDJSON 即時渲染樹狀架構視圖，驗證樹由 `path`/`anchor` 正確還原、各節點標對 `syncStatus`，且不產生任何 tracked 檔。
- [ ] doc 軸預設僅物化 h2/h3，h4–h6 僅在被 `codeRefs` 指向或實際變動時才出現。
- [ ] 驗證 `golem-dockeeper` 指令集大小符合 token-budget ≤15% 指令預算。
- [ ] 驗證代理人不把外部工具輸出當成文件權威狀態（檔案系統仍為唯一權威）。

## 成功標準

- [ ] 存在可運作的 `golem-dockeeper` 代理人與 `doc-sync` skill，且能被 `/gal` 控制面與各 runtime 配接器發現。
- [ ] 存在 schema-validated 的雙軸 NDJSON 結構地圖，記錄 code→doc 對映與同步狀態，支援逐行精準增量更新。
- [ ] 小幅變更後文件能增量同步，僅更新受影響範圍。
- [ ] 對帳模式能以 `lastSyncedRef..HEAD` 抓出 pipeline 外與跨 plan 累積漂移，並輸出待辦清單與樹狀架構視圖。
- [ ] code 軸達全 codebase 檔案粒度覆蓋，新增未文件化檔案一律被標 `missing`（不漏小檔）。
- [ ] graphify 與 codebase-memory-mcp 為純選用增強，缺席時流程零錯誤降級。
- [ ] 全流程不違反 File-System Memory Contract，未引入第二核心記憶層。

## 風險

- **結構地圖與真實狀態漂移**：地圖更新不及反成誤導來源。
  - *緩解*：每行內建 `lastSynced`/`lastSyncedRef`/`syncStatus`；`git diff` 偵測為強制基礎；對帳模式以 `lastSyncedRef..HEAD` 補抓累積漂移。
- **AI 反覆讀寫導致格式漂移**：多次編輯可能寫出不合法行。
  - *緩解*：產出 `structure-map.schema.json`，每行可套此 schema 驗證。本計畫為 AI 執行版，驗證為**盡力門檻**——AI 紀律 + 「機會性驗證」（preflight 偵測到 `python3`/`jq` 等可用時跑驗證器，否則退到 AI 自審）。跨 Win/Lin/Mac 無普遍直譯器，故此階段**不是程式強制的硬門檻**，須誠實標示。新鮮度不變式、穩定排序、節點生命週期分類同屬此盡力強度。
- **外部工具耦合風險**：把 codebase-memory-mcp / graphify 變成隱性必需依賴。
  - *緩解*：嚴格走 preflight 降級契約；缺席不得報錯或提示安裝；測試覆蓋「未安裝→檔案層級」路徑。
- **記憶層重複**：codebase-memory-mcp 自帶 SQLite 圖，可能被誤當權威記憶層。
  - *緩解*：契約明訂它僅為 advisory 訊號來源；權威記憶仍是 repo 內 Markdown / NDJSON。
- **代理人指令膨脹**：職責多元可能超出 token 預算。
  - *緩解*：格式與 preflight 細節移到獨立 workflow / 契約檔，代理人只引用。
- **跨 runtime 漂移**：新代理人未同步進所有配接器會破壞控制面一致性。
  - *緩解*：透過 `gal init` / Sync-DevContext 統一再生，不手改配接器。
- **節點規模膨脹**（雙軸後已大幅降低）：理論上 doc 軸深章節仍可能成長。
  - *緩解*：doc 軸預設只物化 h2/h3，h4–h6 lazy；必要時依頂層模組切分多個 `structure-map.<module>.ndjson`；不做函式層級全量抽取。

## 待實作期確認

- **配接器再生是否需改受保護腳本**：`scripts/Sync-DevContext.ps1:37` 以目錄掃描生成 Repo Skills 索引，故 `skills/doc-sync/` 自動收錄，無需改腳本。**需確認** `scripts/Setup-Machine.ps1` 對 `agent/*.agent.md` 的 symlink 是否同為目錄 glob——若是，新代理人自動納入，「不修改腳本」成立；若為手列清單，Step 5 會觸及受保護腳本，需回 deep-planning 重議範圍。

## 核准

- 人類核准：**核准**（2026-05-31）。
- 架構審查：**APPROVE**（2026-05-31，二次 deep-planning pass 維持；見 `## Review Results > ### Architecture Review`）。二次 pass 修正風險 #2 的硬門檻語言為盡力門檻（誠實標示，無普遍跨平台驗證器）。本變更觸及受保護的 `templates/`、`agent/`、`conventions/`，屬結構性變更；不修改 Setup-Machine 腳本（待實作期確認 agent symlink 為目錄 glob）。下一步 `/refining-plan`。
- 額外領域審查：[未觸發]

## Review Results

### Architecture Review

**裁決：APPROVE**（2026-05-31；2026-05-31 第二次 deep-planning pass 維持 APPROVE）

骨架無結構性錯誤：Agent + Skill 拆分屬正確先例；NDJSON 決策論證紮實；外部工具一律 advisory、檔案系統維持權威記憶，符合 File-System Memory Contract。原 4 項議題已在規劃對話收斂：

**第二次 pass 新增發現（已修正，不阻斷）**：no-bin 範圍決策使原「schema 驗證為硬門檻」的風險 #2 緩解語言過度承諾。已修正為**盡力門檻**——AI 紀律 + 機會性驗證（跨 Win/Lin/Mac 無普遍直譯器，無法程式強制）。新鮮度不變式、穩定排序、生命週期分類同屬盡力強度。此為誠實標示，非設計缺陷。graphify GRAPH_REPORT.md 已參照（227 檔 markdown-contract corpus，無影響本設計的隱性耦合）。

| 議題 | 收斂方式 |
| --- | --- |
| OE-01 函式層級粒度過度工程 | 改**雙軸模型**（code 檔案粒度 + doc 章節），節點數由文件規模約束 |
| BUG-01 commit 邊界未定義 | 獨立 `docs(sync):` commit，不 amend 任務 commit；失敗不阻斷、空變更不產生空 commit |
| BUG-02 漂移新鮮度（謊報 `ok`） | `lastSyncedRef` per-node + 增量/對帳雙模式；只更新本次核對節點；M 節點保守標 `drift` |
| MISS-01 孤兒/改名生命週期 | 對帳以 `git diff -M --name-status` 窮舉分類 A/D/R/M；D 不自動刪正文 |

**Trade-off 摘要**

| 決策 | 效益 | 成本 | 裁決 |
| --- | --- | --- | --- |
| NDJSON 中央結構地圖 | 逐行精準更新、最小 diff、逐行 schema 驗證 | 跨節點查詢較弱（樹靠 `path` 推導） | OK |
| 雙軸模型 | 對齊文件腐化問題、節點數受控、coverage 仍在 | 多一個 `axis` 維度 | OK |
| 增量 + 對帳雙模式 | 對帳補抓 pipeline 外與跨 plan 漂移 | 多一個 `lastSyncedRef` 欄位 | OK |
| 外部工具 advisory-only | 缺席零錯誤降級、不成隱性依賴 | MCP readiness 須以 MCP 語意定義 | OK |
| Agent + Skill 拆分 | 主對話與其他代理人可重用 | 多一個 skill 檔面 | OK |

**值得保留**：NDJSON 選型與退路階梯；外部工具一律 advisory、檔案系統唯一權威；preflight 降級為硬性需求且測試覆蓋無工具路徑。

### Business Review

Pending.

### Design Review

Pending.

### Engineering Review

**裁決：CLEAR**（2026-05-31）

7 個任務各自獨立可完成且可測，乾淨對應 7 個交付檔（schema → 範本/骨架 → skill → workflow → agent → 能力車道契約 → 註冊/再生），依賴方向明確（T-001 先於 T-002；T-003/T-004 引用 schema 與雙軸模型；T-007 收尾整合）。14 條 TP 涵蓋 schema 驗證、增量 + 對帳雙模式、新鮮度/生命週期不變式、投影、能力車道降級、token 預算與配接器註冊，與架構審查 APPROVE 的設計一致。

規劃對話兩項待補已折入任務：standalone reconcile 觸發與其 commit 邊界 → T-004；`Setup-Machine.ps1` agent symlink 為目錄 glob 之確認 → T-007（含手列清單則回 `/deep-planning` 的退路）。

無阻斷項。架構審查 APPROVE + 人類核准皆在位，可進 `/plan-to-prompt`。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | unit | NDJSON 每行通過 `structure-map.schema.json`；故意寫入不合法行（含違反 `axis` 條件式必填）能被偵測 | T-001 |
| TP-002 | unit | 範本與骨架 NDJSON 每行解析成功並通過 schema；行序先 `axis` 再 `id` | T-002 |
| TP-003 | integration | 乾淨 repo（無 graphify/codebase-memory-mcp）小幅變更觸發增量同步，結構地圖與受影響 docs 正確更新，無安裝提示 | T-003, T-004 |
| TP-004 | integration | 移動一檔造成結構漂移：對應行 `path`/`syncStatus` 更新，未受影響行 byte 不動 | T-003, T-004 |
| TP-005 | unit | 新鮮度：只觸碰部分檔案的變更，未核對節點 `syncStatus`/`lastSynced` 保持不變（不整批翻 `ok`）；M 節點保守標 `drift` | T-003, T-004 |
| TP-006 | unit | 節點生命週期：刪檔→code 行刪 + 指向失效 doc 行標 `drift`；改名→`id`/`path` 更新標 `drift`（`git diff -M` 正確辨識） | T-003, T-004 |
| TP-007 | integration | 增量 commit 邊界：pipeline 任務收尾寫入落在獨立 `docs(sync):` commit，任務 commit 未被 amend；失敗不阻斷、空變更不產空 commit | T-004 |
| TP-008 | integration | 對帳模式（含 standalone 直接呼叫）：多次 pipeline 外手動 commit 後對帳，以 `lastSyncedRef..HEAD` 抓累積漂移、輸出 `missing`/`drift`/`orphan`，未核對節點不變；standalone reconcile commit 邊界符合契約 | T-003, T-004 |
| TP-009 | unit | 投影：樹視圖由 `path`/`anchor` 還原並標對 `syncStatus`；待辦清單濾出 `!= ok`；兩者皆不產生 tracked 檔 | T-003 |
| TP-010 | unit | doc 軸預設僅物化 h2/h3；h4–h6 僅在被 `codeRefs` 指向或實際變動時才出現 | T-001, T-003 |
| TP-011 | integration | codebase-memory-mcp `ready`（MCP 可達且 `index_status` 已索引）時含 `#symbol` doc 章節更精準命中；`unavailable` 時檔案層級完整完成、不中斷 | T-006, T-004 |
| TP-012 | integration | graphify `GRAPH_REPORT.md` 存在時跨模組耦合訊號被納入受影響範圍判斷（advisory） | T-006, T-004 |
| TP-013 | manual | `golem-dockeeper` 指令集 ≤15% token-budget；驗證代理人不把外部工具輸出當文件權威狀態（檔案系統唯一權威） | T-005 |
| TP-014 | manual | 註冊與配接器：`/gal` 控制面解析到 `golem-dockeeper`；`CLAUDE/GEMINI/AGENTS/copilot` 配接器含 agent 與 `doc-sync` skill 索引；確認 `Setup-Machine.ps1` agent symlink 為目錄 glob | T-007 |

## Tasks

- [x] T-001 — 撰寫 `docs/structure/structure-map.schema.json`：雙軸節點 JSON Schema（共用欄位 `id`/`axis`/`kind`/`syncStatus`/`lastSynced`/`lastSyncedRef` + `axis` 條件式必填：code 軸 `path`/`layer`/`docs`，doc 軸 `headingLevel`/`title`/`codeRefs`）。 *(0442856b72f7d0e354740bee2443606c6ecd7dbc)*
- [x] T-002 — 建立 `templates/structure-map.template.ndjson` 範本與 `docs/structure/structure-map.ndjson` 首版骨架；行序先 `axis` 再 `id`、key 排序穩定，每行通過 T-001 schema。 *(823574cbc04e58c0189b417579b062f2058d7d42)*
- [x] T-003 — 建立 `skills/doc-sync/SKILL.md`：NDJSON 逐行讀寫/局部更新、schema 盡力驗證（機會性）、變更偵測來源優先序與降級、新鮮度規則、節點生命週期（`git diff -M`）、commit 邊界、待辦清單與樹視圖投影渲染。 *(7c05aae63005199e8bf90510b0f9e75611e6866b)*
- [x] T-004 — 撰寫 `workflows/doc-sync.md` 同步工作流契約：增量 + 對帳雙模式、核心流程、變更偵測優先序、寫回目標；**明定 standalone reconcile 觸發路徑與其 commit 邊界**（無 `<task>` 時用 `docs(sync): reconcile`，或僅 stage 由使用者 commit）。 *(a92b72103b4d471e0321bf55e36002aa2e3a1dac)*
- [x] T-005 — 撰寫 `agent/golem-dockeeper.agent.md`：frontmatter + `<role>`/`<classification>`(Utility)/`<project_context>`/`<rules>`，`required skills: doc-sync`；格式與 preflight 細節引用 workflow/skill，守 ≤15% token 預算。 *(a396f03cef2376e056df15b21d1f16c2bf5cd07c)*
- [x] T-006 — 撰寫 `docs/collaborative-tools/codebase-memory-mcp.md` 能力車道契約（preflight 表、MCP 語意 `ready`、advisory 消費、降級）；小幅修改 `conventions/token-budget.md` capability-first 段落引用 doc-sync 對 codebase-memory-mcp（preflight/降級）與 graphify（advisory）。 *(a9ea6f744509c51f8b80a8cf358eadfec846d4a5)*
- [ ] T-007 — 將 `golem-dockeeper` 加入 `agent/agents.md` 註冊表；重跑 `gal init` / `scripts/Sync-DevContext.ps1` 再生配接器（`CLAUDE.md`/`GEMINI.md`/`AGENTS.md`/`.github/copilot-instructions.md`）；**確認 `scripts/Setup-Machine.ps1` 對 `agent/*.agent.md` 的 symlink 為目錄 glob**（若為手列清單則回報並回 `/deep-planning` 重議範圍）。
