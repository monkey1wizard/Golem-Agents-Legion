# Plan: GAL 統一檔案記憶體策略

## Goal

GAL 擁有一套權威的檔案系統記憶體模型，可跨越不同對話、專案、供應商 (providers)、聊天互動以及實作 agents 運作。Copilot、Antigravity CLI、Codex、Claude Code 以及未來的 runtime 都必須在不依賴供應商本地的聊天歷史、外部記憶體框架、向量儲存庫、資料庫或手動編輯的生成轉接器 (generated adapters) 的情況下，能夠復原並更新相同的專案層級 Markdown 記憶體。

## Requirements

- [ ] 跨工作階段的記憶體必須僅能從 repo 檔案中復原：`.dev/state.md`、`.dev/plans/<slug>.prompt.md` 以及原始計畫交接內容，必須讓新工作階段能在沒有先前聊天歷史的情況下恢復執行。
- [ ] 跨供應商的記憶體必須使用檔案交接，而非供應商記憶體：供應商 A 透過 GAL 工作流程檔案寫入 repo 狀態，然後供應商 B 讀取相同的 cold-start 堆疊，並接續使用相同的已知事實。
- [ ] 跨專案的記憶體必須是明確且有界限的：專案的各項事實保留在每個目標 repo 中，而可重複使用的 GAL 方法論只能透過 GAL 的 source docs、conventions、workflows、templates、commands 與 agents 進行推廣 (promoted)。
- [ ] 聊天互動與實作 agents 必須使用相同的記憶體基底：一般的 AI 聊天、`/gal status`、`/gal whats-next`、`/gal pipeline` 以及 golem 專家都讀取相同的 cold-start 優先順序，並且只將結果寫回各自擁有的 GAL 檔案中。
- [ ] 此計畫必須將 GAL 視為在儲存層已經具備類 Remi 特性：專案擁有的 Markdown 仍然是預設的記憶體基底，工作重點在於強化其操作、分層與整合，而不是替換儲存方式。
- [ ] 此策略必須為 GAL 實際需要的核心操作（擷取、編碼、摘要、推廣與修剪）加入輕量級的記憶體操作合約。
- [ ] 此策略必須加入一個低開銷的學習迴圈，讓重複發生的錯誤成為持久的經驗教訓：在實作、審查、測試或除錯期間發現的錯誤，必須具備從任務記憶體推廣到專案記憶體，再推廣到共用 GAL 方法論的資格（當有證據顯示它們可重複使用時）。
- [ ] 此策略必須在不使用背景常駐程式或沉重的索引服務的情況下，逐漸提高準確性：經驗教訓的整合必須由事件觸發且基於檔案。
- [ ] GAL 記憶體必須保持為本地端、基於檔案系統、可進行版本控制、可審查，並與現有的 Document-driven / No Database 架構保持一致。
- [ ] 此策略必須明確拒絕將 Mem0 模式與類 Mem0 的中介軟體模式用於核心 GAL 狀態：不使用雙存儲向量/圖形堆疊、不使用記憶體中介軟體服務、不使用資料庫支援的語意層，也不使用隱藏的擷取層（除非未來有經過架構師審查的計畫改變此架構）。
- [ ] 此策略必須明確拒絕將 Zep、類 MemGPT 的外部框架、供應商本地記憶體 API、常駐記憶體常駐程式、向量儲存庫與資料庫作為核心 GAL 狀態（除非未來有經過架構師審查的計畫改變此架構）。
- [ ] 生成的 runtime 轉接器必須維持作為由 source 文件與設定腳本衍生出的輸出物；它們可以將記憶體合約帶給供應商，但絕不能成為資料來源 (source of truth)。
- [ ] 隱私界線必須明確：repo 記憶體僅儲存與 repo 相關的上下文，絕不儲存秘密、私人日記內容、本機路徑或屬於本機設定或 Obsidian 中的個人記憶。

## Approach

### Memory Model

此計畫將記憶體視為檔案所有權合約，而非一項新服務。

| 記憶體範圍 | 權威位置 | 寫入者 | 讀取者 | 界線行為 |
| --- | --- | --- | --- | --- |
| 共用的 GAL 方法論記憶體 | `conventions/`、`workflows/`、`commands/`、`agent/`、`templates/`、持久性文件 | 經過審查的 source 變更 | 生成的轉接器、指令 skills、聊天 agents、golem 專家 | 設計上即為跨專案；這些是可重複使用的 GAL 規則，而非專案事實 |
| 專案持久記憶體 | `.dev/project.md`、選定的長期文件 | 規劃、驗證、發布、明確的文件更新 | 所有供應商與 agents 在 cold start 期間 | 在單一 repo 內跨工作階段與跨供應商；只有在刻意萃取到共用 GAL 方法論後，才會跨專案 |
| 專案工作階段記憶體 | `.dev/state.md` | `/gal wrap-up`、規劃指令、控制平面狀態更新 | `/gal status`、`/gal whats-next`、聊天 agents、golem 專家 | 保持活躍計畫、阻礙項目與下一步能在跨工作階段與供應商間恢復 |
| 任務執行記憶體 | `.dev/plans/<slug>.prompt.md` 的 `## Status`、`### Handoff Notes`、任務、分析、測試、審查、除錯區塊 | `/plan-to-prompt`、`/gal wrap-up`、實作者、測試者、審查者、除錯者、驗證者 | 聊天與實作 agents | 活躍任務的主要跨工作階段工作記憶體 |
| 來源規劃記憶體 | `docs/plans/<slug>.md` | `/planning`、`/deep-planning`、`/refining-plan`、規劃審查管道 | `/plan-to-prompt`、審查者、人類 | 可審查的來源計畫；在 prompt 生成後不作為可變的執行狀態 |
| 私有或本機記憶體 | `config.local.env`、`mcp.local.json`、`xmachine.config.json`、使用者 Obsidian vault | 使用者或僅限本機的 agents | 僅有明確導向本機/私有上下文的工作流程 | 絕不是權威的 GAL 專案狀態；絕不是另一個供應商恢復 repo 所必需的 |
| 生成的轉接器記憶體表面 | `.github/copilot-instructions.md`、`AGENTS.md`、`CLAUDE.md`、`GEMINI.md`、runtime 指令輸出 | 僅限 Sync 與 setup 腳本 | 供應商在啟動時 | 將來源記憶體規則帶入 runtime；不可作為資料來源進行編輯 |

### Reference Positioning

此計畫將 YouTube 的比較內容作為篩選過濾器，而非要求重現每一個框架的標準。

- **接受 Remi 基準**：GAL 已經具有最類似 Remi 的行為，因為其持久記憶體是 repo 擁有的 Markdown、對使用者透明、可由 git 審查且可依檔案尋址。
- **拒絕 Mem0 模式**：GAL 不應為了模仿 Mem0 而加入中介軟體式的記憶體協調、雙存儲向量加上圖形持久化，或獨立的記憶體服務。
- **接受核心附加功能**：僅保留能以低成本直接提升準確性的部分：明確的記憶體分層、明確的記憶體操作，以及事件觸發的經驗教訓整合。
- **改進目標**：保留類 Remi 的儲存層，然後加入更清晰的記憶體操作、分層、交接規則與經驗教訓整合，讓系統能在不實質增加負載的情況下提高準確性。
- **效能規則**：優先選擇事件觸發的檔案更新與有界的讀取，而不是背景索引、繁重的擷取基礎設施或永遠在線的記憶體 worker。

### Core Learning Loop

GAL 的最小可行學習系統不是一個新的框架，而是一個建立在現有檔案上的推廣 (promotion) 管道。

1. **擷取 (Retrieve)**：在任務開始、供應商切換、審查、測試或除錯進入時，從 cold-start 堆疊讀取最少且相關的 repo 記憶體。
2. **編碼 (Encode)**：當確認了新的事實、阻礙、決定或失敗時，優先將其寫入活躍的執行記憶體檔案中。
3. **摘要 (Summarize)**：在 wrap-up 或檢查點邊界時，將活躍的工作階段壓縮至 `### Handoff Notes` 與 `.dev/state.md`。
4. **推廣 (Promote)**：當某個經驗教訓被驗證且可能再次發生時，將它從任務記憶體往上移至專案記憶體，然後只有在它能推廣至多個 repo 時，才推廣到共用 GAL 方法論。
5. **修剪 (Prune)**：移除或替換過時、被證明無效或被取代的經驗教訓，以保持檔案記憶體的可靠性。

這個迴圈是由事件觸發的。它絕不能需要背景服務、向量索引或永遠在線的 worker。

### Promotion And Pruning Gates

推廣與修剪必須基於規則，而不是直覺。

- **任務 -> 專案推廣關卡**：僅當經驗教訓具備已驗證的根本原因，且可能在目前 repo 內再次發生時才進行推廣。可接受的證據包括經確認的除錯結果、審查結果、帶有診斷資訊的失敗測試、驗證者結果，或針對相同 repo 工作流程的重複手動修正。
- **專案 -> 共用 GAL 推廣關卡**：僅當經驗教訓改變了 GAL 方法論而非單一 repo 的在地實踐時才進行推廣。該經驗教訓必須影響 GAL 的 convention、workflow、template、command、agent 或其他可重複使用的 source 合約，而且必須能跨 repo 重複使用，不能綁定於單一專案的實作細節。
- **無猜測推廣規則**：假設、暫時的解決方案與未經驗證的解釋必須保留在任務記憶體中，絕不可推廣至持久性專案記憶體或共用 GAL 方法論中。
- **修剪關卡**：當後續證據證明某個持久的經驗教訓無效、被新規則取代、參照的來源表面已不存在，或該經驗教訓因為所屬的工作流程改變而過時，就將其移除或替換。
- **推廣成本規則**：推廣是手動且由事件觸發的，並侷限於現有的 repo 檔案。不允許背景 agent、常駐程式、語意索引或自動批量推廣。

### Step 1: 讓記憶體合約具備權威性
- **Files**: `conventions/token-budget.md`
- **What**: 加入檔案系統記憶體合約 (File-System Memory Contract)，定義每個記憶體範圍、擁有者檔案、寫入者、讀取者、推廣路徑以及五項核心操作：擷取、編碼、摘要、推廣與修剪。聲明聊天 agents 與實作 agents 使用相同的 cold-start 堆疊，且供應商本地聊天記憶體最多只具備建議性質。
- **Verify**: 讀取者能夠回答任務記憶體、工作階段記憶體、專案記憶體、共用 GAL 方法論記憶體、私有記憶體與生成的轉接器輸出歸屬於何處，以及每個核心記憶體操作何時應執行。

### Step 2: 編碼跨工作階段與跨供應商的交接
- **Files**: `workflows/coding.md`, `commands/gal-wrap-up/SKILL.template.md`
- **What**: 明確說明在暫停、切換供應商或切換機器之前，`/gal wrap-up` 是必要的交接路徑。修正 wrap-up 措辭，讓來源計畫的行優先選擇 `.dev/plans/<slug>.prompt.md`（若存在），因為任務執行記憶體位於 `.dev/plans/` 之下，而非 `docs/plans/`。
- **Verify**: 供應商 A 可以在寫入 `### Handoff Notes` 與 `.dev/state.md` 後停止；供應商 B 僅能透過讀取 repo 檔案來恢復執行。

### Step 3: 讓聊天與實作 agents 消耗相同的記憶體
- **Files**: `commands/plan-to-prompt/SKILL.template.md`, `templates/plan-prompt.md`, `agent/agents.md`
- **What**: 明確說明執行 prompt 是對話控制平面操作與 golem 實作/審查/測試/除錯 agents 共用的可變工作記憶體。不要建立獨立的聊天記憶體管道。
- **Verify**: `/gal status`、`/gal whats-next`、`/gal pipeline` 與 golem 專家都指向同一個活躍的執行 prompt，且不依賴供應商聊天紀錄記憶體。

### Step 4: 定義有界的跨專案記憶體
- **Files**: `.dev/project.md`, `templates/project.md`, `docs/devguide.md` *(僅當主要合約在執行步驟 1-3 與 5 之後仍有模糊之處才進行後續追蹤)*
- **What**: 在文件中說明每個目標 repo 都擁有自己的專案記憶體，而跨專案的重複使用只會透過刻意推廣至共用 GAL 方法論檔案（如 conventions、workflows、templates、commands、agents 與持久性文件）來達成。除非主要合約檔案無法讓界線足夠清晰，否則請暫緩這些編輯。
- **Verify**: 透過 GAL 初始化的新專案可以解釋相同的記憶體堆疊，但專案 A 的事實不會默默出現在專案 B 中，除非被推廣至共用 GAL source 檔案。

### Step 5: 加入經驗教訓整合規則
- **Files**: `conventions/token-budget.md`, `workflows/coding.md`, `docs/devguide.md`
- **What**: 定義最小的學習迴圈，以減少重複錯誤。指定哪些發現保留在任務記憶體中、哪些必須推廣至專案記憶體，以及哪些可以推廣至共用 GAL 方法論。保持推廣由證據與重複發生性把關，而非直覺。
- **Verify**: 文件解釋 GAL 應該如何隨著時間變得更準確，而不需要發明新服務或將未體驗證的猜測寫入持久記憶體。

### Step 6: 加入操作員隱私與本機記憶體指南
- **Files**: `docs/personalization.md`
- **What**: 解釋 repo 記憶體、本機設定、Obsidian/私有筆記與供應商本機記憶體之間的界線。清楚說明秘密、日記內容、個人筆記與本機路徑都留在受追蹤的 GAL 記憶體之外，除非明確導向私有的本機目的地。
- **Verify**: 文件說明私有記憶體的歸屬，並警告不要在受追蹤的 GAL 檔案中儲存私有或機密資料。

### Step 7: 透過生成的 runtime 表面進行傳播
- **Files**: `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, generated command skills
- **What**: 在 source 檔案變更後，執行現有的 sync/setup 路徑，並僅將生成的差異作為驗證輸出進行檢查。不要手動編輯生成的轉接器或烤好的 (baked) 指令檔案。
- **Verify**: 每個受支援的供應商都會從生成的由 source 衍生的轉接器中收到相同的記憶體規則，且生成的輸出不包含任何手動編寫的差異。

## Files to Create or Modify

### Phase 1: 必須編寫的變更

- `conventions/token-budget.md` - 具權威性的檔案系統記憶體合約、cold-start 堆疊、記憶體所有權表、無外部記憶體規則，以及經驗教訓推廣/修剪關卡。
- `workflows/coding.md` - 跨工作階段、跨供應商以及聊天/agent 交接的語意。
- `commands/gal-wrap-up/SKILL.template.md` - 供應商切換的 wrap-up 行為與正確的 `.dev/plans/<slug>.prompt.md` 執行記憶體解析。
- `commands/plan-to-prompt/SKILL.template.md` - 執行 prompt 生成的措辭，將 `.dev/plans/<slug>.prompt.md` 指定為聊天與 agents 共用的可變記憶體。
- `templates/plan-prompt.md` - 供共用可變執行記憶體檔案使用的範本措辭。

### Phase 2: 有條件編寫的變更

- `.dev/project.md` - 僅當 Phase 1 的合約在實務上無法清楚界定跨專案邊界時，才加入精簡的驗證事實。
- `templates/project.md` - 僅當新 repo 無法從主要 source 檔案推論出合約時，才傳播最小的 cold-start 措辭。
- `docs/devguide.md` - 僅當貢獻者需要明確的導航輔助以找到權威合約時，才加入簡短的維護者指引。
- `docs/personalization.md` - 僅當在 source 合約更新後，目前面向使用者的隱私與機器本機記憶體邊界仍有模糊時，才加入面向操作員的指南。
- `agent/agents.md` - 僅當在 Phase 1 之後 agent 索引的措辭仍與主要記憶體合約衝突時，才進行更新。

### 生成的驗證輸出

- `.github/copilot-instructions.md` - 僅限同步後的生成轉接器輸出；驗證重新生成的內容，但不要直接編輯。
- `AGENTS.md` - 僅限同步後的生成轉接器輸出；驗證重新生成的內容，但不要直接編輯。
- `CLAUDE.md` - 僅限同步後的生成轉接器輸出；驗證重新生成的內容，但不要直接編輯。
- `GEMINI.md` - 僅限同步後的生成轉接器輸出；驗證重新生成的內容，但不要直接編輯。
- `commands/*/SKILL.md` - 僅限 setup/sync 後的生成指令輸出；驗證重新生成的內容，但不要直接編輯。

## Test Cases

- [ ] 跨工作階段恢復：在 `/gal wrap-up` 之後，新的工作階段能夠從 `.dev/state.md` 加上 `.dev/plans/<slug>.prompt.md` 恢復活躍計畫、最後完成的工作、下一步、阻礙與任務狀態，而無須讀取先前的聊天。
- [ ] 跨供應商交接：一個供應商寫入交接筆記與狀態，然後不同的供應商可以執行 `/gal status` 或 `/gal whats-next`，並從 repo 檔案中回報相同的目前進度。
- [ ] 跨專案界線：第二個初始化的 repo 從 GAL templates 與轉接器繼承記憶體合約，但不會從第一個 repo 收到特定專案的事實，除非這些事實被刻意推廣至共用的 GAL source 文件中。
- [ ] 聊天/Agent 對等性：一般聊天回答、`/gal pipeline` 與 golem 專家都能從 cold-start 堆疊中識別出相同的活躍計畫與任務狀態。
- [ ] Remi-baseline 審查：實作加強了基於檔案的記憶體操作與分層，而沒有引入 Mem0 式的中介軟體、向量/圖形雙重儲存或獨立的記憶體服務。
- [ ] 學習迴圈審查：已確認重複發生的錯誤可以透過明確的推廣規則，從任務記憶體追蹤到專案記憶體或共用的 GAL 方法論中。
- [ ] 操作合約審查：文件定義了 GAL 應該何時擷取、編碼、摘要、推廣與修剪記憶體。
- [ ] 推廣關卡審查：文件定義了哪些證據足以作為任務至專案推廣、專案至共用推廣以及修剪的依據。
- [ ] 生成轉接器驗證：重新生成的供應商檔案包含了自 source 檔案衍生的相同記憶體合約，且沒有直接手動編輯轉接器。
- [ ] 負面外部記憶體檢查：沒有任何實作任務引入 Mem0、Zep、MemGPT 類框架、供應商本地記憶體 API、資料庫、向量儲存庫或長時間運作的記憶體常駐程式作為核心 GAL 狀態。
- [ ] 隱私審查：文件警告機密、私密日記、個人筆記與機器本地路徑不屬於受追蹤的 GAL 記憶體。

## Success Criteria

- [ ] GAL 擁有清晰的記憶體拓撲結構，明確涵蓋跨工作階段、跨供應商、跨專案、聊天以及實作-agent 的行為。
- [ ] 每個記憶體範圍都有唯一的擁有者位置、單一寫入路徑以及單一讀取路徑。
- [ ] GAL 被明確定位為類 Remi 的檔案記憶體系統並具備強化的操作規則，而非 Mem0 類的記憶體中介軟體堆疊。
- [ ] GAL 擁有輕量級的學習迴圈，透過已驗證的經驗教訓推廣與修剪，讓重複犯錯的機率隨時間降低。
- [ ] GAL 定義了精確度所需的最少記憶體操作，而沒有增加新的服務層。
- [ ] GAL 定義了明確的推廣與修剪關卡，讓持久記憶體的變更是基於證據而非直覺。
- [ ] 供應商的切換透過 `/gal wrap-up` 加上 Cold Start 優先級運作，而非透過供應商本地的聊天記憶體。
- [ ] 跨專案記憶體是有邊界的：可重複使用的方法論透過 GAL source 檔案與 templates 分享；專案特定事實保留在其所屬的 repo 內。
- [ ] 聊天互動與 golem 實作 agents 使用相同的受 repo 擁有的記憶體堆疊以及活躍的執行 prompt。
- [ ] 任何核心 GAL 狀態都不依賴外部服務、資料庫、向量儲存庫或特定於 runtime 的記憶體 API。
- [ ] 生成的轉接器與指令輸出會從 source 刷新，並顯示一致的 runtime 導引。

## Risks

- 跨專案記憶體很容易被誤解為 repo 間會自動分享事實。實作時必須說明預設情況下只有共用的方法論能跨專案。
- 如果 Mem0 類型的想法被零碎地複製進來，repo 可能會在聲稱保持基於檔案的同時，意外重新建立起一個隱藏的中介軟體層。實作必須保持儲存與擷取的透明度以及檔案可尋址性。
- 如果經驗教訓推廣過於寬鬆，GAL 可能會將暫時的猜測變成持久的規則，導致準確度隨時間降低。實作必須在推廣前要求證據，並允許修剪過時的教訓。
- 在多份文件間複製記憶體合約可能會導致偏離。將規範性的措辭保存在 `conventions/token-budget.md` 中，並在其他地方使用簡短的指標。
- 如果指令 prompt 或 agent 文件指向不同的狀態檔案，聊天/agent 對等性就會失敗。實作必須讓 `.dev/state.md` 加上 `.dev/plans/<slug>.prompt.md` 成為共用的活躍工作基礎。
- 如果手動編輯，生成的轉接器可能會偏離。將轉接器的更改視為僅用於 sync 驗證輸出。
- 如果 repo 記憶體的描述過於廣泛，隱私界線可能會變弱。此計畫必須將機密、私密筆記、日記內容與本地路徑排除在受追蹤的 GAL 記憶體之外。
- 此計畫涉及受保護路徑 (`conventions/`、`workflows/`、`templates/`、command templates 與 agent docs)，因此在實作前需要架構師審查過的計畫。

## References

- YouTube 分析來源：[Agent记忆框架怎么选?5大Agent Memory项目工程级横向对比](https://youtu.be/BVwpVRpbph4?si=uu4hyF3RKVslH2Iv)
- GitHub repository 進入點：[README.md](https://github.com/monkey1wizard/Golem-Agents-Legion/blob/main/README.md)
- GitHub memory-loading 與 handoff 合約：[conventions/token-budget.md](https://github.com/monkey1wizard/Golem-Agents-Legion/blob/main/conventions/token-budget.md)
- GitHub 執行工作流合約：[workflows/coding.md](https://github.com/monkey1wizard/Golem-Agents-Legion/blob/main/workflows/coding.md)
- GitHub 使用者引導區塊：[docs/personalization.md](https://github.com/monkey1wizard/Golem-Agents-Legion/blob/main/docs/personalization.md)

## Open Questions

無。

<!-- Format: - [ ] OQ-NNN - description *(raised by: command)* -->
<!-- Resolved: - [x] OQ-NNN - description *(raised by: command, resolved by: engineering-review-lane)* -->

- [x] OQ-001 - 跨專案記憶體意味著共用的 GAL 方法論與 templates 預設能跨專案；專案特定的事實仍保留在擁有的 repo 中，除非被刻意推廣至共用的 GAL source 檔案中。*(raised by: replanning, resolved by: source-plan rewrite)*
- [x] OQ-002 - 聊天互動與實作 agents 必須使用相同的記憶體基底：在 prompt 生成後的 `.dev/project.md`、`.dev/state.md` 與 `.dev/plans/<slug>.prompt.md`。*(raised by: replanning, resolved by: source-plan rewrite)*
- [x] OQ-003 - 外部記憶體系統不在核心 GAL 狀態的範圍內；如果這種基於檔案的模型無法滿足主要目標，該計畫應該停止，而不是去最佳化邊緣文件。*(raised by: replanning, resolved by: source-plan rewrite)*
- [x] OQ-004 - YouTube 比較影片應該用於加強 GAL 現有的類 Remi 方向，而非引入 Mem0 類型的中介軟體層。*(raised by: user follow-up, resolved by: source-plan rewrite)*
- [x] OQ-005 - 其餘比較方法中所需的附加功能僅為有用之最少組件：跨工作階段的連續性、分層記憶體的使用、明確的操作，以及一個能減少重複錯誤的低開銷學習迴圈。*(raised by: user follow-up, resolved by: source-plan rewrite)*

## Approval

- Human approval: [clear]
- Architect review: [clear]
- Additional domain review: [not triggered]

## Review Results

### Architecture Review

#### Verdict: APPROVE

這份重寫的內容使實際的記憶體架構變得明確。該計畫現在將跨工作階段、跨供應商、跨專案、聊天以及實作 agent 之間的同等性視為核心需求，而不是文件清理的附帶效應。

#### Trade-off Summary

| 決策 | 效益 | 成本 | 結論 |
| --- | --- | --- | --- |
| 使用受 repo 擁有的 Markdown 作為權威的記憶體基底 | 跨供應商、跨機器、跨工作階段與 git 歷史皆具備可攜性 | 需要紀律嚴明的寫回操作與精簡的交接筆記 | OK |
| 將 GAL 視為已具備類 Remi 特性，並改善操作而非替換儲存方式 | 保持現有低負載架構與透明的檔案 | 需要明確的記憶體操作規則才能在不增加新服務的情況下提升準確度 | OK |
| 透過推廣與修剪加入輕量級學習迴圈 | 讓 GAL 能隨著時間從重複的錯誤中學習並改善 | 需要證據規範以防猜測變成持久規則 | OK |
| 將跨專案記憶體限制為共用方法論，而非自動分享事實 | 防止私人或特定專案的上下文洩漏 | 使用者必須刻意將可重用的知識推廣到 GAL source docs | OK |
| 讓 `.dev/plans/<slug>.prompt.md` 成為共用的可變任務記憶體 | 提供聊天與 golem agents 一個單一的活躍工作來源 | 實作依賴執行狀態前需要先生成 prompt | OK |
| 拒絕針對核心狀態使用 Mem0 模式與其他外部記憶體框架 | 維持無資料庫的架構與 runtime 可攜性 | 降低了跨無關專案的自動語意召回能力 | OK |
| 將生成的轉接器視為衍生載體 | 維護單一真相來源 (source of truth) | 修改 source 後需要執行同步驗證 | OK |

#### Over-engineering Flags

- **OE-01** Mem0 模式會在儲存層表現已經如 Remi 般的 repo 之上，增加第二套記憶體架構。請保留檔案基底，並專注於改進操作。
- **OE-02** 跨專案的事實資料庫會模糊 repo 之間的界線，並增加洩漏專案特定上下文的風險。請改為透過 GAL source docs 與 templates 來共用方法論。
- **OE-03** 在每一份文件中重複完整的記憶體表格會導致偏離。請將規範性的表格置於 `conventions/token-budget.md` 中，在其他地方使用連結或摘要。
- **OE-04** 主動式學習迴圈應保持為事件觸發並維持小規模。不要將其變成背景 agent 艦隊或永遠在線的摘要器。

#### Bug Surface

- **BUG-01** 中度：如果 `/gal wrap-up` 將 source 計畫列解析為 `docs/plans/<slug>.prompt.md`，它將會錯過位於 `.dev/plans/` 下真正的執行 prompt。在實作 Step 2 時需修正指令 template。
- **BUG-02** 中度：如果對話導向指令與 golem agents 指向不同的記憶體檔案，供應商交接將會產生不一致的狀態。透過將 `.dev/state.md` 加上 `.dev/plans/<slug>.prompt.md` 指定為共用的活躍記憶體基底來修正。
- **BUG-03** 低度：如果跨專案記憶體的措辭含糊，使用者可能會期望各 repo 間會有自動召回機制。透過明確劃分共用方法論與專案事實來修正。
- **BUG-04** 中度：如果在沒有嚴格界線的情況下加入了類 Mem0 的中介資料或擷取層，GAL 可能會偏向隱藏的非檔案記憶體行為，導致使用者無法檢查或審查。請維持所有權威狀態均具備檔案可尋址性，並明確拒絕中介軟體的儲存層來修正。
- **BUG-05** 中度：如果每一次失敗都立刻推廣，GAL 會累積許多雜訊或矛盾的教訓。修正方法為推廣前需確認已驗證的根本原因或重複訊號。

#### Performance Concerns

無。此計畫只加入了 Markdown 的合約清晰度與轉接器重新生成；並未增加 runtime 的資料載入或新的背景服務。

#### Missing from Plan

重寫後已無遺漏。主要目標現在已經可以透過跨工作階段、跨供應商、跨專案以及聊天/agent 對等場景來進行測試。

#### Recommended Changes

1. 優先在 `conventions/token-budget.md` 實作記憶體所有權表格；所有其他檔案應當指回該文件。
2. 明確宣告 GAL 將維持類 Remi 的檔案基底，且不打算實作 Mem0 模式。
3. 加入最低限度的記憶體操作合約：擷取、編碼、摘要、推廣以及修剪。
4. 加入明確的經驗教訓推廣規則，以期在不引入新的 runtime 服務的情況下，讓重複犯錯率隨時間下降。
5. 在同一實作階段中修正 `/gal wrap-up` 執行 prompt 的解析與切換供應商時的措辭。
6. 更新 `plan-to-prompt` 與執行 prompt 範本，確保聊天與 agents 共用同一個可變任務記憶體檔案。
7. 除非後續由架構師審查的計畫引入更強的機制，否則請保持跨專案記憶體明確限制在共用的 GAL 方法論與 templates 範圍內。

#### What's Good (keep these)

- 此計畫保留了 GAL 的文件驅動與無資料庫架構。
- 此計畫明確訂定了失敗條件：如果基於檔案的模型無法提供跨工作階段、跨供應商、跨專案、聊天以及 agent 記憶體服務，那麼邊緣的最佳化就毫無用處。
- 此計畫維持了生成的轉接器只是原始真相的傳遞載體，而不是新的真相來源。

#### Scope Refinement

- **僅限 Phase 1**：首先在 `conventions/token-budget.md`、`workflows/coding.md`、`commands/gal-wrap-up/SKILL.template.md`、`commands/plan-to-prompt/SKILL.template.md` 與 `templates/plan-prompt.md` 中落實具權威性的合約及共用的活躍記憶體路徑。
- **僅當需要時執行 Phase 2**：只有當 Phase 1 關於真相來源的整理確實留下了實質上的模糊點時，才去改動 `.dev/project.md`、`templates/project.md`、`docs/devguide.md`、`docs/personalization.md` 或 `agent/agents.md`，不要將其與必須完成的實作綁定。
- **生成的輸出僅用於驗證**：重新生成的轉接器與指令輸出只在同步後作為檢查目標，不是被編寫的實作表面。

### Business Review

未觸發。計畫並未改變商業規則、定價、權限、通知、引導流程或資格邏輯。

### Design Review

未觸發。計畫並未改變面向客戶的流程、佈局、元件或具無障礙要求的 UI 狀態。

### Engineering Review

#### Verdict: CLEAR

此計畫健全且已準備好可供實作。它劃定了必需的架構界線、明確拒絕對核心 GAL 狀態使用外部記憶體中介軟體、點出了 Phase 1 中需要進行的首要 source-of-truth 檔案更新，並將後續可能的 Phase 2 編輯限制在「需證明存在模糊點」才執行，而不是全部綑綁進必要實作中。

工程層面考量：

- 必要的工作嚴格對齊一小組具權威性的 source 檔案，這使得即使更動觸及了受保護的路徑，變更依舊易於審查。
- 計畫定義了正面行為與負面限制，使得實作在改善檔案型記憶體模型的同時，不至於偏離成另一個隱藏的記憶體架構。
- 交接路徑、共用的執行記憶體基底，以及生成輸出傳遞的需求均為明確且可被測試的。
- 有條件的後續修改被適當地延後，避免了範圍蔓延並減少了文件偏移的發生。

目前沒有任何工程上的阻礙需要退回 `/deep-planning` 階段。人力審查 (Human approval) 雖然在 `## Approval` 下仍為待處理 (pending) 狀態，但這是屬於治理狀態的問題，不是 source 計畫本身的工程缺陷。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | 類型 | 描述 | 涵蓋 |
| --- | --- | --- | --- |
| TP-001 | 文件審查 | 確認 `conventions/token-budget.md` 規範了記憶體範圍、擁有者檔案、寫入者、讀取者、cold-start 行為、擷取/編碼/摘要/推廣/修剪操作、推廣關卡、修剪關卡，以及明確拒絕核心 GAL 狀態使用外部記憶體中介軟體。 | T-001 |
| TP-002 | 文件審查 | 確認 `workflows/coding.md` 將 `.dev/state.md` 加上 `.dev/plans/<slug>.prompt.md` 指定為控制平面聊天與專門 agents 的共用恢復基底，並使 `/gal wrap-up` 成為供應商或機器切換時的交接路徑。 | T-002 |
| TP-003 | 文件審查 | 確認 `commands/gal-wrap-up/SKILL.template.md` 可以正確解析 `.dev/plans` 底下的活躍執行檔案，不再指向 `docs/plans/<slug>.prompt.md`；驗證關於供應商切換及機器切換的交接語言是否基於檔案。 | T-003 |
| TP-004 | 文件審查 | 確認 `commands/plan-to-prompt/SKILL.template.md` 將執行 prompt 定義為共用的可變工作檔案，並在重新整理時會保留屬於執行端的段落，而不另創獨立的聊天記憶體管道。 | T-004 |
| TP-005 | 文件審查 | 確認 `templates/plan-prompt.md` 符合共用記憶體合約，並明確分離了穩定的規劃內容與可變的執行狀態。 | T-005 |
| TP-006 | 整合測試 | 執行現有的 sync/setup 重新生成路徑並檢查生成的差異，以驗證供應商轉接器及生成的指令輸出是否正確攜帶相同的記憶體合約，且未經手動竄改。 | T-006 |
| TP-007 | 手動測試 (若觸發) | 如果有更新 `docs/personalization.md`，請確認隱私指引是否有妥善將秘密、日記內容、個人筆記與本地機器路徑隔離在受追蹤的 GAL 記憶體外。 | T-007 |
| TP-008 | 手動測試 (若觸發) | 如果有更新 `docs/devguide.md`，請確認它僅提供貢獻者指向權威合約的引導，而沒有重複規範性規則。 | T-008 |
| TP-009 | 手動測試 (若觸發) | 如果有更新 `templates/project.md`，請確認新初始化的 repos 有成功繼承「有界的跨專案記憶體」措辭，且未複製特定專案的事實。 | T-009 |
| TP-010 | 手動測試 (若觸發) | 如果有更新 `.dev/project.md`，請確認其僅加入了關於跨專案界線的簡潔事實摘要。 | T-010 |
| TP-011 | 手動測試 (若觸發) | 如果有更新 `agent/agents.md`，請確認其措辭與工作流程和指令範本使用的共用執行記憶體基底維持一致。 | T-011 |

## Tasks

- [x] T-001 — 更新 `conventions/token-budget.md` 中的權威性檔案系統記憶體合約：涵蓋記憶體範圍、擁有者檔案、寫入者、讀取者、cold-start 使用法、擷取/編碼/摘要/推廣/修剪操作、推廣關卡、修剪關卡，以及對核心 GAL 狀態的明確無外部記憶體規則。
  驗證：僅透過該文件就能讓讀者辨識出任務、工作階段、專案、共用方法論、私有以及生成的記憶體表面其擁有者位置和推廣路徑。
- [x] T-002 — 更新 `workflows/coding.md` 以編碼跨工作階段和跨供應商的交接語意，強制透過 `.dev/state.md` 加上 `.dev/plans/<slug>.prompt.md` 進行基於檔案的恢復，並宣告控制平面對話加上專業 agents 會共用同一個執行記憶體基底。
  驗證：工作流程文本明確指出 `/gal wrap-up` 是交接路徑，並為對話和專家恢復時命名了相同的受 repo 擁有檔案。
- [x] T-003 — 更新 `commands/gal-wrap-up/SKILL.template.md` 使得活躍計畫解析優先採用 `.dev/plans/<slug>.prompt.md`，而非 `docs/plans/<slug>.prompt.md`，並讓 wrap-up 的指南能明確涵蓋透過純 repo 檔案的暫停、供應商切換及機器切換。
  驗證：該 template 解析了 `.dev/plans` 之下的執行 prompt，且其交接語言是供應商中立的並基於檔案。
- [x] T-004 — 更新 `commands/plan-to-prompt/SKILL.template.md` 讓執行 prompt 被定義為控制平面對話與專家寫回 (write-back) 流程的共用可變工作檔案，由優化過的原始計畫做基礎而來，無須發明獨立的聊天記憶體管道。
  驗證：指令文本指向 `.dev/plans/<slug>.prompt.md` 作為共同的可變執行檔案，並在刷新時保留受執行層擁有的區塊。
- [x] T-005 — 更新 `templates/plan-prompt.md` 以讓模板骨架的用詞符合共用記憶體合約，並明確識別出哪些區塊是穩定的計畫內容，哪些是可變的執行狀態。
  驗證：模板標題及區塊指引需與 plan-to-prompt 及工作流文件所使用的 source 合約措辭相符。
- [x] T-006 — 透過現有的 sync/setup 路徑重新生成衍生的 runtime 及指令輸出，並僅將生成的差異視為驗證輸出進行審視。
  驗證：生成的轉接器及預先烤好的 (baked) 指令檔案是否皆反映了更新後的記憶體合約，且未含手寫分歧點。
- [ ] T-007 — (選擇性，延後處理) 只有在 Phase 1 過後，隱私或機器本地記憶體邊界仍有模糊不清時，才更新 `docs/personalization.md`。
  驗證：該文件明確將機密、日記內容、個人筆記以及本機機器路徑從追蹤中的 GAL 記憶體排除。
- [ ] T-008 — (選擇性，延後處理) 只有當貢獻者在 Phase 1 之後仍需要被明確指引到權威記憶體合約時，才更新 `docs/devguide.md`。
  驗證：該文件加入簡短的導航指引，而不重複寫規範性合約。
- [ ] T-009 — (選擇性，延後處理) 只有當新初始化的 repos 無法從核心的合約檔案推導出跨專案記憶體界線時，才更新 `templates/project.md`。
  驗證：模板只加入最少的 cold-start 必要措辭，以保持專案事實的本地性及方法論的共用性。
- [ ] T-010 — (選擇性，延後處理) 只有在 Phase 1 之後此 repo 仍需針對跨專案記憶體界線有一精要的已驗證事實時，才更新 `.dev/project.md`。
  驗證：repo summary 增加簡短事實，但不重述整個合約。
- [ ] T-011 — (選擇性，延後處理) 只有在 Phase 1 過後，若 agent 索引措辭仍與共用執行記憶體合約發生衝突時，才更新 `agent/agents.md`。
  驗證：agent 指南需與工作流及指令範本指向相同的 repo 擁有的記憶體堆疊。
