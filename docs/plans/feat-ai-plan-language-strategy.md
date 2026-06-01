# 規劃：AI 規劃文件語言策略

## 目標

透過明確區分「人類可讀的來源文件語言」與「執行提示詞（prompt）語言」，來改善 AI 在 `/planning`、`/deep-planning` 與研究（`/gal research`、`/gal deep-research`）階段的文件寫作品質。核心目的是讓使用者在規劃與研究階段，能用自己提問的語言閱讀產出，避免「先翻成英文再回讀」造成的理解錯誤與語意流失。

確保**人類可讀的產出**（來源規劃文件 `docs/plans/*.md` 與研究報告 `docs/research/*.md`）以使用者輸入的語言撰寫以保持可讀性與使用者友善度,同時將**供 AI 讀取的執行提示詞**（`.dev/plans/*.prompt.md`）維持在純英文並針對 Token 使用量進行優化。

## 需求

- [ ] `docs/plans/` 中的來源規劃文件必須以使用者的主要語言（即他們在提示詞中使用的語言）生成，以確保可讀性與使用者友善度。
- [ ] `docs/research/` 中的研究報告必須以使用者的主要語言撰寫敘述與綜整內容，以減少「翻成英文再回讀」造成的理解錯誤。
- [ ] 執行提示詞（`.dev/plans/*.prompt.md`）必須完全以英文生成與維護，以最大化相容性並降低不同模型之間的 Token 消耗量。
- [ ] 執行 `/planning`、`/deep-planning` 與研究工作流的 AI 代理人不得在人類可讀產出中尷尬地混用英文與使用者輸入的語言。
- [ ] 從本地化的來源規劃文件轉換為英文執行提示詞的過程發生在 `/plan-to-prompt` 階段。
- [ ] 執行提示詞應積極採用節省 Token 的做法（例如縮寫、精簡指令），因為它們是供 AI 讀取的，而非供人類閱讀。
- [ ] 引用來源與證據連結（標題、URL、原文片段）在研究報告中保留原文,不翻譯;僅綜整敘述採用使用者語言。
- [ ] 必須支援本次指令的顯式語言指示（例如 `in zh-tw`、`in en`、`用英文寫`），且其優先級高於任何預設設定。
- [ ] 必須支援機器本地的 `PLAN_LANGUAGE` 參數（`config.example.env` → `config.local.env`），控制 `docs/plans/` 與 `docs/research/` 的落檔語言。此為個人偏好,不進版控。
- [ ] 必須支援專案層的 `PROJECT_LANGUAGE` 參數（欄位定義於 `templates/project.md`、值追蹤於各 repo 的 `.dev/project.md`），宣告專案**主文件(canonical)**的語言。語意:無語言中綴的主檔(如 `README.md`、`docs/x.md`)以 `PROJECT_LANGUAGE` 撰寫;帶中綴的 `<name>.<lang>.md`(如 `README.zh-Hant.md`)一律視為翻譯副本,註定不是主檔。此為專案共用身分,進版控。
- [ ] 兩個參數的用途、存放位置與優先級必須在參數設定頁（`config.example.env` 註解）與 `docs/personalization.md` 明確說明,避免使用者混淆 `PLAN_LANGUAGE` 與 `PROJECT_LANGUAGE`。
- [ ] plan/research 產出必須**整份以解析後的語言**撰寫(技術識別碼、程式碼片段、引用來源原文除外);不引入 frontmatter `lang:` 欄位 — 語言可由文件內容直接判讀。參數值本身採 BCP-47 代碼(`zh-TW`、`en`)。

## 非目標

- 不**批量重寫**既有 README 與 `docs/` 正式文件。`PROJECT_LANGUAGE`(專案身分、靜態)只在新建或編輯主文件時,作為主檔語言與翻譯命名(`<name>.<lang>.md`)依據;與本功能對 plan/research 的「每次動態解析」是不同機制、不同擁有者。
- 不改動跨模型/慣例性產物:`.dev/plans/*.prompt.md`（恆英文）、`.dev/state.md`、commit message、review/test/security 寫回結果 — 這些會被其他模型或工具消費,維持英文以保跨模型穩定。
- 不在共享方法論層（`conventions/`、`workflows/`、`commands/`）寫死任一語言。預設未設定時維持現行行為,其他使用者/repo 零影響(opt-in)。

## 方法

### 語言解析鏈（plan / research 動態層）

解析順序（高 → 低）:

1. 本次指令的顯式指示（`in zh-tw`、`in en`、`用英文寫`）
2. 機器本地 `PLAN_LANGUAGE`（`config.local.env`）
3. 自動偵測提問敘述語言
4. 後備預設 `en`

`PLAN_LANGUAGE` 的讀取沿用 `WORKING_HOURS_ENABLED` 先例:agent 於啟動檢查時讀取 `~/.gal/config/config.local.env`。範本指令須指明此來源。

`PROJECT_LANGUAGE` 不參與此鏈;它只在新建 README/正式 docs 或明確要求時,作為專案語言依據。

產出整份以解析後語言撰寫,不寫入 frontmatter `lang:`;語言由內容直接判讀。

### 來源規劃階段

當調用 `/planning` 或 `/deep-planning` 時，AI 必須偵測使用者請求的語言。它將完全以該語言生成並維護 `docs/plans/<slug>.md`。技術名詞（如 `docs/plans/`、特定檔案名稱或程式碼變數）可以保持英文，但說明敘述必須使用使用者的語言。

### 研究階段

當執行 `/gal research` 或 `/gal deep-research` 時，`golem-researcher` 必須偵測使用者請求的語言，並以該語言撰寫 `docs/research/<slug>.md` 的敘述與綜整段落。技術名詞、引用來源標題、URL 與原文片段保留原文。獨立的引用驗證（VERIFY）以相同語言回報。

### 執行提示詞階段

當執行 `/plan-to-prompt` 時，AI 將讀取本地化的來源規劃文件，並將語意意圖翻譯成高度壓縮、純英文的 `.dev/plans/<slug>.prompt.md`。

### 命令與工作流範本更新

- 更新規劃範本（`commands/planning/SKILL.template.md`、`commands/deep-planning/SKILL.template.md`），加上明確指令以要求使用使用者的輸入語言撰寫，並避免尷尬的語言混用。
- 更新研究工作流與代理人（`workflows/research.md`、`agent/golem-researcher.agent.md`），要求研究報告敘述採用使用者語言、引用來源保留原文。
- 更新 `commands/plan-to-prompt/SKILL.template.md` 以強制要求執行提示詞輸出純英文與 Token 壓縮。

## 建立或修改的檔案

- `commands/planning/SKILL.template.md` - 新增指令以使 `docs/plans/*.md` 符合使用者的輸入語言。
- `commands/deep-planning/SKILL.template.md` - 新增指令以使 `docs/plans/*.md` 符合使用者的輸入語言。
- `workflows/research.md` - 新增指令以使 `docs/research/*.md` 敘述符合使用者語言、引用來源保留原文。
- `agent/golem-researcher.agent.md` - 新增語言偵測與輸出語言規則，與工作流一致。
- `commands/plan-to-prompt/SKILL.template.md` - 新增指令以使用高度壓縮的英文撰寫 `.dev/plans/*.prompt.md`。
- `conventions/token-budget.md` - 新增引導說明 `.prompt.md` 檔案基於 Token 效率僅限英文，而來源規劃文件與研究報告則遵循使用者的語言。
- `config.example.env` - 新增 `PLAN_LANGUAGE` 參數與註解（機器本地、預設留空 = 自動偵測）。
- `templates/project.md` - 新增 `PROJECT_LANGUAGE` 欄位(專案層、追蹤);各 repo 的 `.dev/project.md` 填入實際值。
- `docs/personalization.md` - 明確說明 `PLAN_LANGUAGE` 與 `PROJECT_LANGUAGE` 的用途、存放位置與優先級差異。

## 測試案例

- [ ] 使用非英文提示詞（例如中文或西班牙文）執行 `/planning`。驗證 `docs/plans/<slug>.md` 是否以該語言撰寫，且無尷尬的英文混用。
- [ ] 對該非英文規劃文件執行 `/deep-planning`。驗證 AI 是否以相同的語言回覆並更新規劃文件。
- [ ] 對該非英文規劃文件執行 `/plan-to-prompt`。驗證 `.dev/plans/<slug>.prompt.md` 是否完全以精簡的英文生成。
- [ ] 驗證生成的 `.prompt.md` 檔案與來源規劃文件相比，其 Token 使用量是否經過積極優化。
- [ ] 使用非英文提示詞執行 `/gal research`。驗證 `docs/research/<slug>.md` 的敘述是否以該語言撰寫，且引用來源標題與 URL 保留原文。

## 測試計畫

| ID | 類型 | 描述 | 覆蓋 |
| --- | --- | --- | --- |
| TP-001 | manual | zh-TW 提示詞執行 `/planning` → `docs/plans/*.md` 整份 zh-TW、無尷尬混語、技術識別碼保留英文 | T-001 |
| TP-002 | manual | 對 zh-TW 提示詞加顯式 `in en` → 產出英文(顯式覆寫勝過自動偵測) | T-001 |
| TP-003 | manual | 設 `PLAN_LANGUAGE=zh-TW`、提示詞無顯式指示 → 產出 zh-TW(參數勝過自動偵測) | T-006, T-001 |
| TP-004 | manual | BUG-01:提示詞要求「新增處理 `in en` 指令的說明」→ 該字串不被當作輸出語言切換,產出維持解析語言 | T-001, T-002 |
| TP-005 | manual | 對 zh-TW 規劃文件執行 `/deep-planning` → 審查寫回同為 zh-TW、無混語 | T-002 |
| TP-006 | manual | 對 zh-TW 規劃文件執行 `/plan-to-prompt` → `.dev/plans/*.prompt.md` 純英文、意譯保真 | T-003 |
| TP-007 | manual | Token 比對:`.prompt.md` 相對來源 plan 經壓縮優化 | T-003 |
| TP-008 | manual | zh-TW 提示詞執行 `/gal research` → `docs/research/*.md` 敘述 zh-TW、引用標題/URL 原文保留 | T-005 |
| TP-009 | doc-review | `conventions/token-budget.md` 正確陳述三層語言政策(prompt 英文 / plan-research 解析鏈 / docs PROJECT_LANGUAGE) | T-004 |
| TP-010 | doc-review | `config.example.env` 含 `PLAN_LANGUAGE` 註解、預設留空 | T-006 |
| TP-011 | doc-review | `templates/project.md` 含 `PROJECT_LANGUAGE` 欄位 + 命名語意;本 repo `.dev/project.md` 已填值 | T-007 |
| TP-012 | doc-review | `docs/personalization.md` 與 `.zh-Hant.md` 副本一致說明兩參數用途/位置/優先級 | T-008 |
| TP-013 | build | 重生 adapter 後,生成的 `SKILL.md` 與 `CLAUDE.md` 等帶入新語言指令、無損壞 | T-009 |

## 成功標準

- [ ] 使用者能以自己的母語閱讀 `docs/plans/*.md` 與 `docs/research/*.md`。
- [ ] 來源規劃文件與研究報告中不再出現「糟糕的英文」或奇怪的語言混用。
- [ ] 專案主文件(無語言中綴)語言與 `PROJECT_LANGUAGE` 一致;翻譯以 `<name>.<lang>.md` 命名。
- [ ] 執行提示詞保持英文，以保留可預測的跨模型 AI 執行行為。
- [ ] 執行提示詞展現高 Token 效率。

## 風險

- **翻譯損失**：當從使用者的母語翻譯成壓縮的英文時，`/plan-to-prompt` 可能會失去細微的語意。
  - *緩解措施*：在命令範本中強調意譯（語意翻譯）而非直譯（字面翻譯）。
- **程式碼邊界滲漏**：程式碼區塊與技術名詞在來源規劃文件中可能會被錯誤翻譯。
  - *緩解措施*：指令要求 AI 對於技術識別碼與程式碼片段保持不翻譯。
- **引用完整性**：研究報告若翻譯引用來源標題或 URL,將破壞可追溯性與驗證。
  - *緩解措施*：明確要求引用來源、URL 與原文片段保留原文,僅綜整敘述本地化。

## 未決問題

- [x] OQ-001 — 已解決:`PROJECT_LANGUAGE` 是專案基礎設定值,語意為「主文件(canonical)語言」。帶 `.<lang>.` 中綴的檔案(如 `README.zh-Hant.md`)註定是翻譯副本、非主檔。欄位移至 `templates/project.md`,命名語意寫入文件。 *(raised by: architect-review, resolved by: deep-planning)*
- [x] OQ-002 — 已解決:放棄 frontmatter `lang:`,改用「plan/research 整份以解析後語言撰寫」指令。語言由內容直接判讀,不引入無解析器的新結構。 *(raised by: architect-review, resolved by: deep-planning)*

## 核准

- 人類核准：**核准**（2026-06-01）。
- 架構審查：APPROVE — OQ-001、OQ-002 已於 deep-planning 收斂(見 ## 審查結果 > ### 架構審查 > 收斂後更新)。
- 工程審查：CLEAR — 見 ## 審查結果 > ### 工程審查。下一步 `/plan-to-prompt`。
- 額外領域審查：[未觸發 — 不涉客戶面流程或商業規則]

## 審查結果

### 架構審查

**裁決：REVISE**(核心方向健全;兩個阻擋項須先收斂)

#### 取捨摘要

| 決策 | 效益 | 成本 | 裁決 |
| --- | --- | --- | --- |
| plan/research 跟隨使用者語言、prompt 恆英文 | 消除「翻英再回讀」理解損失;保跨模型執行穩定 | 純指令式,靠模型遵從,無確定性強制 | OK |
| 雙參數 `PLAN_LANGUAGE`/`PROJECT_LANGUAGE` | 個人/專案兩條軸分離,存放位置天然分開 | `PROJECT_LANGUAGE` 目前無消費者 | REVISE |
| `lang:` frontmatter 稽核 | 下游與人工可驗證解析語言 | 引入全新慣例,範本/解析器皆無 | REVISE |
| opt-in、預設未設不變 | 其他 repo/使用者零影響 | 無 | OK |

#### 過度設計旗標

- **[OE-01]** `PROJECT_LANGUAGE` 是無消費者的死參數:非目標明言「僅提供並記錄、不重寫既有 docs」,故無任何工作流讀它做事。違反 YAGNI(「為永不被配置的東西做配置」)。且 repo 既有 `README.zh-Hant.md` 平行翻譯慣例已覆蓋靜態層。→ 見 OQ-001:給單一消費者、延後、或明確降級為純宣告。
- **[OE-02]** `lang:` frontmatter 引入全新結構慣例,但無人解析、無範本承載。稽核效益真實但成本是新慣例 + 範本改動。→ 見 OQ-002:傾向簡化為「整份以解析語言撰寫」指令。

#### Bug 面

- **[BUG-01]** 低:顯式指示 `in en` / `in zh-tw` 與「描述內容中剛好出現該字串」無法區分(例如使用者要求「新增一段處理 `in en` 指令的說明」)。
  - 修正:範本明定指示只在「頂層、針對本次產出語言」時生效,不解析為內容一部分。

#### 範本/泛化缺口(Missing from Plan)

- **[GAP-01]** `PROJECT_LANGUAGE` 若保留,來源應為 `templates/project.md`(受保護路徑),而非僅本 repo 的 `.dev/project.md`;否則新初始化的 repo 不會有此欄位。
- **[GAP-02]** 需求第 22 條要求寫入 frontmatter,但檔案清單與任務皆未包含 `templates/plan.md` 與研究輸出結構的修改。要求與任務不一致(OQ-002 解決後補齊或移除)。
- **[GAP-03]** `PLAN_LANGUAGE` 的讀取機制應明確沿用 `WORKING_HOURS_ENABLED` 先例(agent 啟動時讀 `~/.gal/config/config.local.env`),範本指令需指明此來源,否則「如何取得參數值」懸空。

#### 效能

- 無顧慮。純文件/指令變更。

#### 值得保留

- plan/research(動態、個人)vs README/docs(靜態、專案)的二分,且對應到不同存放位置與擁有權 — 邊界乾淨、論證充分。
- opt-in 預設關 + 沿用 working-hours 模式 — 與既有慣例一致,不擴散風險。
- 引用來源/URL/原文片段保留原文 — 正確守住可追溯性。

#### 阻擋摘要

進 `/plan-to-prompt` 前須:解決 OQ-001(PROJECT_LANGUAGE 消費者或延後)、OQ-002(frontmatter 去留),並據此補齊 GAP-01/02、補上 GAP-03 的讀取來源說明。

#### 收斂後更新(deep-planning)

- **OQ-001 解決**:`PROJECT_LANGUAGE` 取得明確語意(主文件 canonical 語言 + `<name>.<lang>.md` 翻譯命名約定),並移至受保護的 `templates/project.md`。OE-01 解除 — 在文件驅動系統中,具明確語意的約定即是交付物,非死配置。
- **OQ-002 解決**:放棄 frontmatter,改「整份以解析語言撰寫」。OE-02、GAP-02 解除。
- **GAP-03 已補**:`PLAN_LANGUAGE` 讀取沿用 `WORKING_HOURS_ENABLED` 先例。
- **GAP-01 已補**:`PROJECT_LANGUAGE` 來源改為 `templates/project.md`。
- **更新裁決:APPROVE**。下一步 `/refining-plan` 鎖任務契約。

### 工程審查

**裁決:CLEAR**

依據:

- 範圍明確且自洽 — 只動暫時性工作層(`docs/plans/`、`docs/research/`)的撰寫語言 + 兩個 opt-in 參數;prompt/state/commit 維持英文,跨模型穩定性不受影響。
- 任務 T-001~T-009 各自可獨立完成與驗證,依賴順序已標(T-006 先、T-009 末);BUG-01 與完整解析鏈、`PLAN_LANGUAGE` 讀取來源已落入任務描述。
- 測試矩陣 TP-001~TP-013 對齊每一任務;涵蓋解析鏈優先級(TP-002/003)、BUG-01 反例(TP-004)、prompt 英文+Token(TP-006/007)、文件一致性(TP-009~012)、adapter 重生(TP-013)。
- 受保護路徑(`commands/`、`conventions/`、`templates/`)變更已經 deep-planning 架構審查 APPROVE。

已知限制(非阻擋):本功能為文件/指令式,靠模型遵從,無確定性程式強制 — 屬 GAL 文件驅動系統的固有性質,以手動驗收(manual TP)涵蓋。

<!-- ENG_REVIEW: CLEAR -->

## 任務

> 依賴順序:T-006(參數定義)先行,T-001/T-002/T-005 引用其名;T-009(adapter 重生)最後。其餘任務彼此獨立。

- [x] T-001 — 更新 `commands/planning/SKILL.template.md`:加入語言解析鏈(顯式指示 > `PLAN_LANGUAGE` > 自動偵測 > `en`)、指明 `PLAN_LANGUAGE` 讀自 `~/.gal/config/config.local.env`(沿用 `WORKING_HOURS_ENABLED` 先例)、明定顯式指示僅在頂層針對本次產出語言時生效(BUG-01,不解析內容中出現的相同字串)、整份以解析語言撰寫、技術識別碼與程式碼片段不翻譯、禁止尷尬混語。 *(140fe80759f552d6ee4519aeddd9af8049fe48be)*
- [x] T-002 — 更新 `commands/deep-planning/SKILL.template.md`:套用與 T-001 相同的解析鏈與撰寫規則,並要求審查寫回(架構/領域審查)也使用同一解析語言。 *(8d55ce426738e4aaec0927e08e8a2560e087b8fa)*
- [x] T-003 — 更新 `commands/plan-to-prompt/SKILL.template.md`:強制 `.dev/plans/*.prompt.md` 純英文 + Token 壓縮,並強調意譯(語意翻譯)而非直譯以降低翻譯損失。 *(62b4c534c8f4d03569b2abfe62801e6f1cad109f)*
- [x] T-004 — 更新 `conventions/token-budget.md`:記錄語言政策 — `.prompt.md` 恆英文(Token 效率);plan/research 跟隨解析鏈/`PLAN_LANGUAGE`;README/docs 跟隨 `PROJECT_LANGUAGE`。 *(e43bc1e982919bf6d081b5dd747439c06311f805)*
- [x] T-005 — 更新 `workflows/research.md` 與 `agent/golem-researcher.agent.md`:研究報告敘述/綜整採同一解析語言、整份撰寫,引用來源標題/URL/原文片段保留原文,VERIFY 以同語言回報。 *(2d821476ce4caf04763601d0699ad2611c1f2917)*
- [x] T-006 — 新增 `PLAN_LANGUAGE` 至 `config.example.env`(含註解、預設留空 = 自動偵測、值採 BCP-47)。確認其為 agent 直接讀取的行為設定,非 smudge/clean 佔位符。 *(4747b54d2c0354879a1bf7c05880e2701145f344)*
- [x] T-007 — 新增 `PROJECT_LANGUAGE` 欄位至 `templates/project.md`,並記錄 canonical/翻譯命名語意(無中綴主檔 = `PROJECT_LANGUAGE` 語言;`<name>.<lang>.md` = 翻譯)。本 repo 的 `.dev/project.md` 填入值。 *(8e2097eec500f05e48c36d1fe5c262cc27776c0d)*
- [ ] T-008 — 更新 `docs/personalization.md` 明確說明 `PLAN_LANGUAGE` 與 `PROJECT_LANGUAGE` 的用途、存放位置與優先級;同步更新翻譯副本 `docs/personalization.zh-Hant.md`。
- [ ] T-009 — 重新執行 `gal init`(或同步指令碼)以重新生成配接器檔案(`.github/copilot-instructions.md`、`CLAUDE.md`、`GEMINI.md`、`AGENTS.md` 以及生成的 `SKILL.md`),確認新語言指令已帶入生成輸出。
