# 規劃：AI 規劃文件語言策略

## 目標

透過明確區分「人類可讀的來源規劃文件語言」與「執行提示詞（prompt）語言」，來改善 `/planning` 與 `/deep-planning` 階段的 AI 規劃文件寫作品質。確保來源規劃文件（`docs/plans/*.md`）主要以使用者輸入的語言撰寫以保持使用者友善度，同時將執行提示詞（`.dev/plans/*.prompt.md`）維持在純英文並針對 Token 使用量進行優化。

## 需求

- [ ] `docs/plans/` 中的來源規劃文件必須以使用者的主要語言（即他們在提示詞中使用的語言）生成，以確保可讀性與使用者友善度。
- [ ] 執行提示詞（`.dev/plans/*.prompt.md`）必須完全以英文生成與維護，以最大化相容性並降低不同模型之間的 Token 消耗量。
- [ ] 執行 `/planning` 與 `/deep-planning` 的 AI 代理人不得在來源規劃文件中尷尬地混用英文與使用者輸入的語言。
- [ ] 從本地化的來源規劃文件轉換為英文執行提示詞的過程發生在 `/plan-to-prompt` 階段。
- [ ] 執行提示詞應積極採用節省 Token 的做法（例如縮寫、精簡指令），因為它們是供 AI 讀取的，而非供人類閱讀。

## 方法

### 來源規劃階段
當調用 `/planning` 或 `/deep-planning` 時，AI 必須偵測使用者請求的語言。它將完全以該語言生成並維護 `docs/plans/<slug>.md`。技術名詞（如 `docs/plans/`、特定檔案名稱或程式碼變數）可以保持英文，但說明敘述必須使用使用者的語言。

### 執行提示詞階段
當執行 `/plan-to-prompt` 時，AI 將讀取本地化的來源規劃文件，並將語意意圖翻譯成高度壓縮、純英文的 `.dev/plans/<slug>.prompt.md`。

### 命令範本更新
- 更新規劃範本（`commands/planning/SKILL.template.md`、`commands/deep-planning/SKILL.template.md`），加上明確指令以要求使用使用者的輸入語言撰寫，並避免尷尬的語言混用。
- 更新 `commands/plan-to-prompt/SKILL.template.md` 以強制要求執行提示詞輸出純英文與 Token 壓縮。

## 建立或修改的檔案

- `commands/planning/SKILL.template.md` - 新增指令以使 `docs/plans/*.md` 符合使用者的輸入語言。
- `commands/deep-planning/SKILL.template.md` - 新增指令以使 `docs/plans/*.md` 符合使用者的輸入語言。
- `commands/plan-to-prompt/SKILL.template.md` - 新增指令以使用高度壓縮的英文撰寫 `.dev/plans/*.prompt.md`。
- `conventions/token-budget.md` - 新增引導說明 `.prompt.md` 檔案基於 Token 效率僅限英文，而來源規劃文件則遵循使用者的語言。

## 測試案例

- [ ] 使用非英文提示詞（例如中文或西班牙文）執行 `/planning`。驗證 `docs/plans/<slug>.md` 是否以該語言撰寫，且無尷尬的英文混用。
- [ ] 對該非英文規劃文件執行 `/deep-planning`。驗證 AI 是否以相同的語言回覆並更新規劃文件。
- [ ] 對該非英文規劃文件執行 `/plan-to-prompt`。驗證 `.dev/plans/<slug>.prompt.md` 是否完全以精簡的英文生成。
- [ ] 驗證生成的 `.prompt.md` 檔案與來源規劃文件相比，其 Token 使用量是否經過積極優化。

## 成功標準

- [ ] 使用者能以自己的母語閱讀 `docs/plans/*.md`。
- [ ] 來源規劃文件中不再出現「糟糕的英文」或奇怪的語言混用。
- [ ] 執行提示詞保持英文，以保留可預測的跨模型 AI 執行行為。
- [ ] 執行提示詞展現高 Token 效率。

## 風險

- **翻譯損失**：當從使用者的母語翻譯成壓縮的英文時，`/plan-to-prompt` 可能會失去細微的語意。
  - *緩解措施*：在命令範本中強調意譯（語意翻譯）而非直譯（字面翻譯）。
- **程式碼邊界滲漏**：程式碼區塊與技術名詞在來源規劃文件中可能會被錯誤翻譯。
  - *緩解措施*：指令要求 AI 對於技術識別碼與程式碼片段保持不翻譯。

## 未決問題

無。

## 核准

- 人類核准：[待定]
- 架構審查：[待定]
- 額外領域審查：[未觸發]

## 任務

- [ ] T-001 — 更新 `commands/planning/SKILL.template.md`，指示以使用者的輸入語言撰寫來源規劃文件。
- [ ] T-002 — 更新 `commands/deep-planning/SKILL.template.md`，指示以使用者的輸入語言撰寫深度規劃文件。
- [ ] T-003 — 更新 `commands/plan-to-prompt/SKILL.template.md`，指示以嚴格且壓縮 Token 的英文生成 `.prompt.md`。
- [ ] T-004 — 更新 `conventions/token-budget.md`，以反映 `.prompt.md` 與來源規劃文件的語言政策。
- [ ] T-005 — 重新執行 `gal init`（或同步指令碼）以重新生成配接器（adapter）檔案（`.github/copilot-instructions.md`、`CLAUDE.md`、`GEMINI.md`、`AGENTS.md` 以及生成的 `SKILL.md`）。
