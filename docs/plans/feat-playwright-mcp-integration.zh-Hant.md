# 計畫：Playwright MCP 整合

## 目標

GAL 將 Microsoft Playwright MCP 作為受管理且已完成預檢的瀏覽器能力，提供給所有依賴瀏覽器可觀察行為的工作流程：測試、驗證、設計稽核、動態頁面研究，以及 MCP/伺服器評估。此整合不得擴展公開 `/gal` 命令介面、不得繞過本地優先研究規則、不得削弱瀏覽器信任邊界，也不得在可重用指令碼更適合時取代成本更低的原生 Playwright 或 CLI 自動化。

## 需求

- [ ] Playwright MCP 必須出現在 GAL 受管理的 MCP 來源檔，或明確記錄的本地選用路徑中，並對瀏覽器狀態、網路範圍、檔案存取與機密資訊採取保守預設。
- [ ] 若預設納入追蹤設定，受管理伺服器鍵名必須是 `playwright`，與上游範例一致，避免各執行時期出現長名稱別名漂移。
- [ ] 支援的執行時期 MCP 橋接器必須一致地將 Playwright MCP 傳播到 VS Code、Copilot CLI、Gemini、Antigravity、Codex、opencode 與 Claude Code，且不得覆寫無關的使用者擁有 MCP 伺服器。
- [ ] 預設追蹤設定必須使用隔離、非持久化的瀏覽器狀態，且不得匯入使用者 cookie、持久化使用者設定檔、瀏覽器擴充功能、CDP 端點、不受限制的檔案存取或機密檔案。
- [ ] 本地覆寫必須負責所有非預設瀏覽器設定檔行為：有頭模式、viewport/裝置、儲存狀態、輸出目錄、選用能力旗標、extension/CDP/remote 端點、來源 allow/block 清單，以及任何機器本地的類機密路徑。
- [ ] 瀏覽器自動化預檢必須遵循共享協作工具檢查模型：適用性、可用性、初始化狀態、就緒狀態、路由與降級。
- [ ] `golem-tester` 的 browser-qa 模式必須知道何時使用 Playwright MCP、Chrome DevTools MCP、內建瀏覽器工具、原生 Playwright 指令碼，或未來的 Playwright CLI/SKILL 路由。
- [ ] 當計畫目標依賴可觀察的瀏覽器行為時，`golem-verifier` 必須能使用 Playwright MCP 證據進行目標反向驗證。
- [ ] `golem-designer` 的 audit 模式必須能在 UI 已執行時，使用 Playwright MCP 進行即時 UI 檢查、響應式檢查、無障礙快照、截圖、annotation/highlight 證據與漂移驗證。
- [ ] `golem-researcher` 必須將 Playwright MCP 視為動態頁面研究輔助工具，而非本地優先搜尋、OpenCLI 結構化檢索、來源歸因或獨立參考驗證的替代品。
- [ ] `mcp-builder` 指引必須提到：當 MCP 伺服器或工具暴露瀏覽器可見流程時，Playwright MCP 可作為選用評估路由；API-only MCP 評估仍應優先使用成本較低的 MCP Inspector 或 CLI 路徑。
- [ ] 文件必須說明如何安裝與更新 MCP 橋接器、如何用 `mcp.local.json` 設定非預設 Playwright 行為、輸出產物如何保存，以及何時需要重新執行設定。
- [ ] 驗證必須證明 MCP manifest 可解析、更新指令碼可在 Windows 與 macOS/Linux 路徑中解析此伺服器、無關的 provider-owned MCP 條目會被保留，且 Playwright MCP 不可用時，專家指引會誠實地降級或回報阻塞。
- [ ] 不應為 Playwright 引入新的公開 `/gal` 命令；該能力必須由現有專家代理人、技能與工作流程消費。

## 使用面矩陣

| 使用面 | 何時使用 Playwright MCP | 何時偏好其他路由 |
| --- | --- | --- |
| Browser QA | 任務需要即時導覽、點擊、表單、鍵盤、dialog、分頁、檔案上傳/drop、快照或截圖。 | 可重用回歸測試應落在原生 Playwright 指令碼或未來 Playwright CLI/SKILL。 |
| 無障礙與斷言 | 任務受益於 accessibility-tree 快照、可見文字/值檢查或 locator 產生。 | 靜態 markup 檢查或 unit/widget test 能以較低成本證明需求。 |
| 設計稽核 | 任務需要響應式 viewport、視覺證據、即時 UI 漂移檢查、annotation 或截圖。 | 沒有執行中的 UI，或審查仍停留在規劃階段。 |
| Console/network 診斷 | Playwright MCP 擁有所需 console 或 network 工具，且瀏覽器 session 已存在。 | Chrome DevTools MCP 對該失敗提供更深的 protocol、performance 或 network 檢查。 |
| Session 與 storage state | 測試需要隔離的 storage-state 設定、cookie/localStorage/sessionStorage 檢查或重設。 | 流程需要真實使用者 cookie 或未由本地覆寫明確提供的持久化本機瀏覽器狀態。 |
| 動態頁面研究 | 來源需要渲染、互動或只能透過瀏覽器導覽取得，且仍需保留可引用證據。 | 本地文件、OpenCLI、fetch、Defuddle 或靜態來源能以較少瀏覽器狀態回答。 |
| MCP/伺服器評估 | 開發中的 MCP 暴露瀏覽器可見流程或 Web UI，需要實際操作。 | MCP Inspector、unit test 或 CLI evaluation 已能覆蓋伺服器，無需瀏覽器自動化。 |
| PDF 與產物擷取 | 計畫明確需要瀏覽器渲染截圖、video/trace 或 PDF 輸出。 | 產物會洩漏敏感狀態，或驗證並不需要產物。 |
| 遠端/無頭執行 | 無頭 work node 或 IDE worker 需要 HTTP/SSE transport 或 standalone server。 | 本地執行時期能直接啟動 stdio，且不需要共享瀏覽器 context。 |

## 方法

### 步驟一：鎖定上游合約與信任邊界

- **檔案**：`docs/research/playwright-mcp-integration.md`、`docs/plans/feat-playwright-mcp-integration.zh-Hant.md`
- **內容**：記錄目前上游調用方式、Node 需求、stdio 與 HTTP transport 支援、設定檔行為、能力旗標、安全警告，以及 MCP 與 CLI 的取捨。將 `@playwright/mcp@latest` 視為版本敏感，實作前必須重新驗證旗標。
- **驗證**：研究筆記引用上游 README 章節，且計畫在開始腳本或 manifest 編輯前明確列出安全預設假設。

### 步驟二：將 Playwright MCP 加入受管理的 MCP 設定

- **檔案**：`mcp.json`、`scripts/Update-Mcp.ps1`、`scripts/update-mcp.sh`
- **內容**：新增 `playwright` 伺服器條目，預設使用 `npx -y @playwright/mcp@latest --isolated --headless`，除非實作驗證證明需要不同的非互動式調用方式。追蹤預設不得包含選用 caps、持久化設定檔、storage state、輸出路徑、CDP、extension、有頭模式或 network allow/block 清單。若早期嘗試已建立 `microsoft/playwright-mcp`、`playwright-mcp` 或正規化套件名等 legacy key，只為這些已知 GAL-managed legacy key 加入清理。
- **驗證**：dry-run 或 temp-home 執行顯示每個支援執行時期都收到 `playwright` 伺服器、無關 provider-owned MCP 條目仍保留，且沒有執行時期收到長正規化 Playwright key。

### 步驟三：定義協作工具預檢合約

- **檔案**：`docs/collaborative-tools/playwright-mcp.md`、`docs/collaborative-tools/checking-contract.md`
- **內容**：記錄適用性、可用性檢查、首次瀏覽器初始化、就緒需求、路由與降級路徑。納入 no-tool 行為：缺少 Playwright MCP 必須被回報或降級到文件化路由，不能算作瀏覽器驗證成功。
- **驗證**：tester、designer、verifier、researcher 與 mcp-builder 指引都能將任務對應到 `not-applicable`、`unavailable`、`available-but-needs-init`、`available-but-not-ready` 或 `ready`。

### 步驟四：更新瀏覽器測試、驗證與設計稽核合約

- **檔案**：`skills/webapp-testing/SKILL.md`、`agent/golem-tester.agent.md`、`agent/golem-verifier.agent.md`、`agent/golem-designer.agent.md`
- **內容**：定義依任務形狀選擇路由。Playwright MCP 優先用於探索式互動、無障礙快照、表單、檔案上傳/drop、響應式 viewport 檢查、截圖、隔離 context 內的 storage/session 檢查，以及 browser-backed verifier 證據。Chrome DevTools 仍優先用於深層 protocol 診斷。原生 Playwright 指令碼或未來 CLI/SKILL 路由仍優先用於可重用自動化回歸流程。
- **驗證**：browser-qa、designer audit 與 verifier 輸出能說明使用的路由，誠實回報 PASS/FAIL/BLOCKED，並將證據寫回正確的計畫區段，而不聲稱不可用工具已執行。

### 步驟五：更新研究路由，不削弱證據規則

- **檔案**：`workflows/research.md`、`agent/golem-researcher.agent.md`、`docs/collaborative-tools/opencli.md`，選用 `skills/opencli-research/SKILL.md`
- **內容**：將 Playwright MCP 定位在本地優先搜尋與適用的結構化檢索之後，用於動態或互動式頁面。要求保留 URL、筆記、截圖或已儲存輸出產物，讓獨立 verifier 能反向核查。
- **驗證**：研究流程在適用時仍預設使用本地優先與 OpenCLI；Playwright MCP 使用必須留下來源證據，且不能取代獨立參考驗證。

### 步驟六：更新 MCP 開發與評估指引

- **檔案**：`skills/mcp-builder/SKILL.md`，選用 mcp-builder reference docs
- **內容**：將 Playwright MCP 加為選用評估路由，適用於瀏覽器可見的 MCP server 行為、Web UI、產生的文件頁面與使用者流程。API-only 或 read-only server 品質檢查仍優先使用 MCP Inspector、unit test 與 CLI evaluation。
- **驗證**：mcp-builder 指引不會鼓勵每個 MCP server 都使用瀏覽器自動化，且清楚區分 browser-visible evaluation 與一般 MCP protocol/tool evaluation。

### 步驟七：更新維運者與維護者文件

- **檔案**：`docs/devguide.md`、`docs/personalization.md`、`docs/personalization.zh-Hant.md`、`scripts/scripts.md`，選用 `README.md`、選用 `config.example.env`
- **內容**：記錄 `mcp.json` 加 `mcp.local.json` 的所有權、設定重跑命令、本地覆寫範例、安全產物目錄與禁止追蹤的值。說明 extension、CDP、持久化設定檔、storage state、有頭模式、選用 caps 與 network origin 規則都是本地選擇。
- **驗證**：維護者能判斷設定屬於追蹤預設、本地覆寫、本地 env 或工作流程文件；使用者能辨識哪個 setup 命令會刷新 runtime MCP config。

### 步驟八：重新產生衍生轉接器並驗證漂移

- **檔案**：`.github/copilot-instructions.md`、`AGENTS.md`、`CLAUDE.md`、`GEMINI.md`、產生的命令技能
- **內容**：來源編輯完成後，執行既有 sync/setup 路徑。將產生的轉接器視為驗證輸出，不手動編輯。
- **驗證**：產生的轉接器反映來源衍生指引，且沒有引入手動 adapter 漂移。

## 待建立或修改的檔案

- `docs/research/playwright-mcp-integration.md` - 本計畫的上游合約與能力證據。
- `mcp.json` - 若核准為預設受管理能力，加入追蹤的 `playwright` MCP server。
- `scripts/Update-Mcp.ps1` - Windows 的 `playwright` bridge profile、legacy-key 清理與保留行為。
- `scripts/update-mcp.sh` - macOS/Linux 的 `playwright` bridge profile、legacy-key 清理與保留行為。
- `docs/collaborative-tools/playwright-mcp.md` - 新增 Playwright MCP 預檢、路由、能力、安全與降級合約。
- `docs/collaborative-tools/checking-contract.md` - 將 Playwright MCP 加入目前工具對應清單。
- `docs/collaborative-tools/opencli.md` - 在動態頁面/browser fallback 路由中明名 Playwright MCP。
- `skills/webapp-testing/SKILL.md` - 更新瀏覽器工具順序、選用 MCP 依賴、路由選擇與 no-tool 行為。
- `skills/mcp-builder/SKILL.md` - 新增 browser-visible MCP 評估指引。
- `agent/golem-tester.agent.md` - 更新 browser-qa 路由選擇與寫回期待。
- `agent/golem-verifier.agent.md` - 新增目標反向 browser evidence 指引。
- `agent/golem-designer.agent.md` - 新增 Playwright MCP 作為 audit 模式 browser route。
- `agent/golem-researcher.agent.md` - 新增動態瀏覽器研究路由與證據邊界。
- `workflows/research.md` - 當動態頁面需要瀏覽器工作時，將 Playwright MCP 加入研究協作工具預檢。
- `docs/devguide.md` - 受管理 MCP server 新增與驗證的維護者指引。
- `docs/personalization.md` - 使用者面向的本地覆寫與 setup rerun 指引。
- `docs/personalization.zh-Hant.md` - 若英文 personalization 文字變動，更新繁體中文鏡像。
- `scripts/scripts.md` - 若 MCP 更新行為變動，更新指令碼清單與驗證指引。
- `README.md` - 若 Playwright MCP 成為文件化 collaborative tool 頁面，選用更新導覽。
- `config.example.env` - 只有在實作引入文件化 env-backed 本地覆寫範例時，才選用加入 placeholder。
- `.github/copilot-instructions.md`、`AGENTS.md`、`CLAUDE.md`、`GEMINI.md`、`commands/*/SKILL.md` - sync/setup 後僅作為產生的驗證輸出。

## 測試案例

- [ ] Manifest parse：`mcp.json` 是有效 JSON，且包含具有核准 command、args 與保守預設旗標的 `playwright` server。
- [ ] 非互動式首次執行：選定的 `npx` 調用方式不會在正常受管理 runtime 啟動時卡在安裝確認提示。
- [ ] Windows MCP dry run：`scripts/Update-Mcp.ps1` 為支援的 selected runtimes 解析 Playwright MCP，並將 server key 對應為 `playwright`。
- [ ] Bash MCP dry run：`scripts/update-mcp.sh` 為支援的 selected runtimes 解析 Playwright MCP，並將 server key 對應為 `playwright`。
- [ ] Runtime preservation：GAL-managed 條目刷新時，無關 provider-owned MCP 條目仍會保留。
- [ ] Legacy cleanup：只有 GAL-managed 或已文件化 legacy alias 的長/正規化 Playwright server key 會被移除，不會刪除 user-owned 條目。
- [ ] Local override：`mcp.local.json` 可覆寫 viewport、有頭模式、輸出目錄、storage state、選用 caps 或本地 profile 旗標，而不更改追蹤預設。
- [ ] Safe defaults：追蹤預設不使用持久化 user data、瀏覽器 extension、CDP endpoint、不受限制的檔案存取、saved session、追蹤的 secrets 或使用者 cookies。
- [ ] Browser QA routing：`golem-tester` 能為即時互動選擇 Playwright MCP，並將 PASS/FAIL/BLOCKED 結果寫入 `## Test Results`。
- [ ] Design audit routing：`golem-designer` 能為即時 UI 檢查選擇 Playwright MCP，並回報 screenshots/snapshot 證據，而不聲稱不可用的 browser execution 已發生。
- [ ] Verifier routing：`golem-verifier` 能使用 Playwright MCP 證據確認可觀察 UI 目標，然後仍產生目標反向的 truth/file/wiring 結果。
- [ ] Research routing：`golem-researcher` 只在本地優先與適用的結構化檢索檢查之後使用 Playwright MCP，並記錄可供獨立驗證的來源證據。
- [ ] MCP-builder routing：browser-visible MCP evaluation 可使用 Playwright MCP；API-only MCP server 檢查仍使用 MCP Inspector、unit test 或 CLI evaluation。
- [ ] No-tool behavior：Playwright MCP 不可用時，工作流程回報缺少 browser path 或降級到文件化替代方案，不聲稱 browser validation 已發生。
- [ ] Generated adapter verification：重新產生的 provider 檔案反映來源變更，且未被手動編輯。

## 成功條件

- [ ] GAL 透過現有 MCP setup 或明確核准的本地選用路徑，公開 Playwright MCP 作為受管理的瀏覽器能力。
- [ ] canonical managed server key 在會暴露 MCP server name 的 runtime bridge 中都是 `playwright`。
- [ ] 測試、驗證、設計稽核、研究與 browser-visible MCP evaluation 工作流程，都能透過現有 specialist contract 路由到 Playwright MCP。
- [ ] 缺少 Playwright MCP 能力時有明確降級行為，且不能被誤認為成功的 browser execution。
- [ ] 預設瀏覽器自動化是隔離、無頭、非持久化，且不包含追蹤 secrets、使用者 cookies、使用者瀏覽器狀態、不受限制的檔案存取、extension attachment 或 CDP endpoint。
- [ ] 本地覆寫可明確選用 stateful 或 headed browser 行為，而不污染追蹤的 repo 設定。
- [ ] PowerShell 與 Bash 路徑之間的跨執行時期 MCP 設定保持對齊。
- [ ] 公開 `/gal` 命令介面保持不變。
- [ ] Chrome DevTools MCP、OpenCLI、原生 Playwright 指令碼與未來 Playwright CLI/SKILL 路由，仍可用於更適合它們的場景。
- [ ] 文件告知使用者何時需要重新執行 setup，以及本地 Playwright MCP 客製化應放在哪裡。
- [ ] 產生的 adapter 由來源刷新，且不顯示手動漂移。

## 風險

- 瀏覽器自動化會擴展信任邊界，因為代理人可以瀏覽網站、提交表單、檢查 storage、觀察敏感渲染內容並產生產物。預設必須隔離，本地覆寫必須明確。
- 上游 `@playwright/mcp@latest` 可能漂移。實作必須先驗證目前旗標、capability 名稱與工具類別，再鎖定文件或測試。
- 除非設定 `--isolated`，上游預設會使用 persistent profile。若追蹤設定漏掉此旗標，會默默把登入/session 狀態保存在機器本地 Playwright cache 目錄。
- Storage、network routing、DevTools、vision、PDF、testing assertions、extension connection 與 CDP 等選用能力都很有用，但會擴大行為範圍；不應一次全部在追蹤預設啟用。
- `--secrets` 不是安全邊界。文件不得暗示它能讓敏感瀏覽變得安全。
- 部分機器可能缺少 Node.js、npm、Playwright browsers、display support 或首次 browser installation state。這應產生 `available-but-needs-init` 或 `available-but-not-ready`，而不是假的 ready state。
- Runtime config 格式不同。PowerShell 與 Bash bridge 必須針對 VS Code、Copilot CLI、Gemini、Antigravity、Codex、opencode 與 Claude Code 驗證。
- Playwright MCP 與 Chrome DevTools MCP、OpenCLI、原生 Playwright 指令碼、Playwright CLI/SKILL 使用面重疊。應依任務形狀路由，而不是宣告單一 browser tool 永遠優先。
- 研究使用可能退化成未受支援的瀏覽。本地優先搜尋、來源歸因與獨立驗證仍為強制。
- 若直接編輯產生的 adapter 會造成漂移。重新產生必須維持來源衍生。
- 本計畫涉及受保護的工作流程介面、setup 行為、agent contract 與產生 adapter 語義，因此實作前需要 architecture review。

## 參考資料

- 上游儲存庫：[microsoft/playwright-mcp](https://github.com/microsoft/playwright-mcp)
- 上游 README snapshot：[raw README](https://raw.githubusercontent.com/microsoft/playwright-mcp/main/README.md)
- 研究筆記：`docs/research/playwright-mcp-integration.md`
- GAL MCP 來源檔：`mcp.json`
- GAL MCP 設定路徑：`scripts/Update-Mcp.ps1`、`scripts/update-mcp.sh`
- 瀏覽器測試技能：`skills/webapp-testing/SKILL.md`
- 設計代理人：`agent/golem-designer.agent.md`
- 研究工作流程：`workflows/research.md`
- 協作工具檢查合約：`docs/collaborative-tools/checking-contract.md`
- MCP 安全指引：[MCP Security Best Practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)

## 待解決問題

- [x] OQ-001 - 對於選定執行時期，Playwright MCP 應從追蹤的 `mcp.json` 預設啟用，還是先以文件化 `mcp.local.json` 選用方式發布，因為它擴展了 browser/network capability？*(提出者：planning，解決者：architecture-review)*  
  **決策**：使用追蹤 `mcp.json`，但只納入保守的 `playwright` 預設。這與現有受管理 browser tooling 一致，能保持 runtime bridge 對齊，也避免每位使用者都手寫 local config。風險由 `--isolated`、`--headless`、不使用 persistent profile、不使用 extension/CDP、不開 unrestricted file access，以及明確 local override 文件控制。
- [x] OQ-002 - 追蹤預設是否應強制 `--isolated` 與 `--headless`，還是將 headless/window/profile options 全部留給 local override？*(提出者：planning，解決者：architecture-review)*  
  **決策**：追蹤預設強制 `--isolated` 與 `--headless`。Local override 可選用 headed mode、viewport/device、storage state、output directory 或 persistent profile。
- [x] OQ-003 - Playwright MCP 是否應成為 `webapp-testing` 的第一 browser MCP route，還是 Chrome DevTools 在深度 inspection 任務中維持第一？*(提出者：planning，解決者：architecture-review)*  
  **決策**：依任務形狀路由。Playwright MCP 在 live interaction、accessibility snapshots、forms、files、responsive checks、storage/session setup 與 browser-backed assertions 上優先。當 Chrome DevTools 在 console/protocol/performance/network diagnostics 上工具集更強時，Chrome DevTools 仍優先。可重用自動化仍優先使用原生 Playwright 指令碼或 Playwright CLI/SKILL 路由。
- [x] OQ-004 - Storage、network、DevTools、vision、PDF、testing assertions、extension 或 CDP 等選用 Playwright MCP capabilities 是否應在追蹤預設啟用？*(提出者：deep-planning，解決者：architecture-review)*  
  **決策**：不啟用。追蹤設定只保留安全 core startup。選用能力與 stateful connection 屬於 `mcp.local.json` 或 machine-local env-backed override，且需要明確任務理由。

<!-- 格式：- [ ] OQ-NNN - 描述 *(提出者：command)* -->
<!-- 已解決：- [x] OQ-NNN - 描述 *(提出者：command，解決者：engineering-review-lane)* -->

## 核准

- 人工核准：[待定]
- 架構師審查：[clear]
- 其他領域審查：[實作變更 MCP/browser capability 後需要 security review]

## 審查結果

### 架構審查

Verdict：APPROVE for refined planning；進入 `/plan-to-prompt` 前先執行 `/refining-plan`。

#### Trade-off Summary

| 決策 | 收益 | 成本 | Verdict |
| --- | --- | --- | --- |
| 以安全預設將 `playwright` 追蹤於 `mcp.json` | Runtime setup 一致且可被發現 | 將受管理 browser/network capability 加入預設 setup | OK with isolation/headless safeguards |
| 使用 `playwright` 作為 canonical key | 符合上游範例並避免 alias normalization bug | 若早期已產生長 key，需要 cleanup | OK |
| 依任務形狀路由，而非讓 Playwright 永遠第一 | 保留 Chrome DevTools、OpenCLI 與原生 script 的優勢 | 需要更多文件與 agent decision logic | OK |
| 將 optional caps 與 persistent state 保持 local-only | 降低意外暴露 cookies、files 與 browser state 的風險 | 進階 workflow 需要明確 override 文件 | OK |
| 納入 designer 與 mcp-builder 使用面 | 捕捉 GAL 既有 browser-adjacent workflow | 稍微擴大計畫範圍 | OK，因為它們是既有 contract，不是新公開 surface |

#### Over-engineering Flags

- **OE-01** 不要新增 `/gal playwright` 命令。現有 specialist contract 已擁有 browser QA、verification、design audit 與 research。
- **OE-02** 不要在 MCP bridge 上方建立新的抽象層。既有 manifest 加 runtime bridge functions 已足夠；只在必要處加入 profile 與 cleanup。
- **OE-03** 不要預設啟用所有上游 capability。廣義 caps 會擴展 storage、network、artifact 或 execution 行為，應留在 local override。

#### Bug Surface

- **BUG-01 High**：`.dev/state.md` 若指向不存在的 plan 檔，GAL status 與後續 `/refining-plan` 會失敗。保留單一 zh-Hant 計畫時，state 必須持續指向 `docs/plans/feat-playwright-mcp-integration.zh-Hant.md`。
- **BUG-02 High**：除非設定 `--isolated`，上游預設使用 persistent profile。缺少 `--isolated` 的追蹤設定會默默保留 login/session state。
- **BUG-03 Medium**：`npx @playwright/mcp@latest` 視 npm 行為可能在首次執行提示。實作必須驗證非互動式調用，可能是 `npx -y @playwright/mcp@latest`。
- **BUG-04 Medium**：若 manifest 使用 package-like name，runtime key normalization 可能產生長 key。應直接使用 `playwright`，且只清理已知 GAL-managed legacy names。
- **BUG-05 Medium**：Browser artifacts 可能包含敏感渲染內容。Output directories 與 saved sessions 必須是 local、explicit，且不得被追蹤。
- **BUG-06 Medium**：Remote/headless 使用可能因缺少 Node、browsers、display 或 first-run install state 失敗。Preflight 必須區分 unavailable、needs-init、not-ready 與 ready。

#### 原計畫缺口

- `golem-designer` audit mode 的 browser use。
- `mcp-builder` browser-visible MCP evaluation use。
- 上游 README 中 MCP-vs-CLI 的取捨。
- Network、storage、DevTools、vision、PDF、testing assertions、tracing/video 與 config inspection 等 optional capability handling。
- 以 canonical `playwright` manifest key 取代 bridge aliasing 的更簡單方案。
- 非互動式 npx first-run 行為。
- Artifact/output directory safety。
- Remote/headless 或 HTTP transport considerations。

#### 已套用建議變更

1. 讓 `.dev/state.md` 指向磁碟上實際保留的 zh-Hant source plan。
2. 擴展 goal 與 requirements，納入 design audit 與 MCP/server evaluation。
3. 新增 use-surface matrix，讓代理人依任務形狀選擇正確 browser 或 non-browser route。
4. 以保守預設解決四個 architecture questions。
5. 新增 durable research note，保存上游事實與 version-sensitive assumptions。

#### 保留的優點

- 原計畫正確避免新增公開命令。
- 原計畫已保護 local-first research 與 independent verification。
- 原計畫正確將 generated adapters 視為 source-derived output，而非手動編輯來源。
- 原計畫正確辨識 browser state 與 secrets 是核心 trust-boundary risk。

### 商業審查

未觸發。

### 設計審查

未作為 planning-stage design review 觸發，因為本計畫不修改 customer-facing UI。 refined plan 會將 Playwright MCP 加為未來 `golem-designer` live UI audit 的選用路由。

### 工程審查

#### Verdict: CLEAR

此計畫已具備可實作的工程合約，適合進入 `/plan-to-prompt`。範圍、風險邊界、任務路由與驗證方向都已明確，且沒有需要退回 `/deep-planning` 才能解開的架構阻塞。

工程判定理由：

- 追蹤設定、本地覆寫、協作工具預檢、specialist routing 與產生 adapter 驗證之間的責任邊界已分清，實作時不需要再自行發明新命令或新抽象層。
- `playwright` canonical key、`--isolated --headless` 保守預設、legacy cleanup 範圍與 no-tool 降級語意都已被寫成可驗證的合約，而不是模糊方向。
- 計畫已把 browser-adjacent surface 補齊到 tester、verifier、designer、researcher 與 mcp-builder，避免只改 manifest 卻漏掉實際消費能力的上層契約。
- 文件面與 setup/sync 面都被納入，能降低「實作存在但使用者不知道如何重新套用設定」這種常見落差。

沒有工程 blocker 需要回到 `/deep-planning`。`## 核准` 中的人工核准與後續 security review 仍屬治理與實作後審查節點，不是 source plan 本身的缺陷。

<!-- ENG_REVIEW: CLEAR -->

## 測試計畫

| ID | 類型 | 說明 | Covers |
| --- | --- | --- | --- |
| TP-001 | doc review | 確認 `docs/research/playwright-mcp-integration.md` 已記錄上游調用方式、Node 需求、stdio/HTTP transport、profile 行為、能力旗標、安全警告與 MCP-vs-CLI 取捨，且能支撐追蹤預設的安全假設。 | T-001 |
| TP-002 | integration | 驗證 `mcp.json` 中的 `playwright` server 是有效受管理條目，預設命令與 args 符合核准的保守設定，且不含 local-only capability 或 stateful 參數。 | T-002 |
| TP-003 | integration | 在 Windows 路徑檢查 `scripts/Update-Mcp.ps1` 能將 `playwright` 正確橋接到支援 runtime、保留無關 provider-owned 條目，且只清理已文件化的 GAL-managed legacy key。 | T-003 |
| TP-004 | integration | 在 Bash 路徑檢查 `scripts/update-mcp.sh` 與 Windows 行為對齊：使用 `playwright` canonical key、保留無關條目，並處理同一組 legacy key。 | T-004 |
| TP-005 | doc review | 確認 `docs/collaborative-tools/playwright-mcp.md` 與 `docs/collaborative-tools/checking-contract.md` 定義 `not-applicable`、`unavailable`、`available-but-needs-init`、`available-but-not-ready`、`ready` 及對應降級語意。 | T-005 |
| TP-006 | doc review | 確認 `skills/webapp-testing/SKILL.md`、`agent/golem-tester.agent.md`、`agent/golem-verifier.agent.md`、`agent/golem-designer.agent.md` 已明確區分 Playwright MCP、Chrome DevTools MCP、原生 Playwright 與無工具降級路由。 | T-006 |
| TP-007 | doc review | 確認 `workflows/research.md`、`agent/golem-researcher.agent.md`、`docs/collaborative-tools/opencli.md`（以及若有需要的 `skills/opencli-research/SKILL.md`）保留本地優先與結構化檢索優先權，並要求 Playwright MCP 留下可反查證據。 | T-007 |
| TP-008 | doc review | 確認 `skills/mcp-builder/SKILL.md` 只把 Playwright MCP 納入 browser-visible evaluation，不把它誤寫成所有 MCP server 的預設評估路由。 | T-008 |
| TP-009 | manual | 確認 `docs/devguide.md`、`docs/personalization.md`、`docs/personalization.zh-Hant.md`、`scripts/scripts.md`（以及若有需要的 `README.md`、`config.example.env`）已說明 setup 重跑方式、本地覆寫歸屬、產物保存位置與不可追蹤值。 | T-009 |
| TP-010 | integration | 執行既有 sync/setup regeneration 路徑後，確認 `.github/copilot-instructions.md`、`AGENTS.md`、`CLAUDE.md`、`GEMINI.md` 與產生命令技能只反映來源變更，未出現手動漂移。 | T-010 |

上方測試案例仍是實作合約必須覆蓋的測試池；此表將它們收斂成與任務對齊的執行矩陣。

## 任務

- [ ] T-001 — 鎖定並更新上游 Playwright MCP 研究依據，確認目前 CLI/transport/profile/capability/security 合約，並把 version-sensitive 假設寫進 `docs/research/playwright-mcp-integration.md`。
  Verify: 研究筆記足以支撐 `playwright` canonical key、`npx -y @playwright/mcp@latest --isolated --headless` 預設、local-only override 邊界與 no-security-boundary 警告。
- [ ] T-002 — 在 `mcp.json` 中加入受管理的 `playwright` server，維持保守預設，只保留安全 core startup，不引入持久化 profile、saved session、CDP、extension、unrestricted file access 或其他 local-only capability。
  Verify: manifest 可解析，且追蹤預設只包含已核准的 command、args 與安全預設旗標。
- [ ] T-003 — 更新 `scripts/Update-Mcp.ps1`，讓 Windows 橋接器正確傳播 `playwright`，保留無關 provider-owned 條目，並只清理已知 GAL-managed legacy Playwright key。
  Verify: Windows dry run 產生的 runtime config 使用 `playwright` canonical key，且不會誤刪使用者自有條目。
- [ ] T-004 — 更新 `scripts/update-mcp.sh`，讓 macOS/Linux 橋接器與 Windows 行為對齊，包含 canonical key、legacy cleanup 範圍與 provider-owned 條目保留語意。
  Verify: Bash dry run 與 PowerShell dry run 對同一組輸入 manifest 產生一致的 Playwright bridge 行為。
- [ ] T-005 — 新增並更新 Playwright MCP 協作工具合約文件，包含 `docs/collaborative-tools/playwright-mcp.md` 與 `docs/collaborative-tools/checking-contract.md`，把 applicability、availability、init、ready、route 與 degrade 規則寫清楚。
  Verify: 文件能讓 specialist 明確判定 `not-applicable`、`unavailable`、`available-but-needs-init`、`available-but-not-ready`、`ready`，且不把缺少工具誤判為驗證成功。
- [ ] T-006 — 更新瀏覽器相關 specialist contract 與技能，涵蓋 `skills/webapp-testing/SKILL.md`、`agent/golem-tester.agent.md`、`agent/golem-verifier.agent.md`、`agent/golem-designer.agent.md`，把 Playwright MCP、Chrome DevTools MCP、原生 Playwright 指令碼與降級路由依任務形狀分流。
  Verify: tester、verifier、designer 的輸出契約都能誠實標示實際使用的瀏覽器路由與 PASS/FAIL/BLOCKED 結果。
- [ ] T-007 — 更新研究與 MCP 評估路由，涵蓋 `workflows/research.md`、`agent/golem-researcher.agent.md`、`docs/collaborative-tools/opencli.md`，以及需要時的 `skills/opencli-research/SKILL.md`、`skills/mcp-builder/SKILL.md`，讓 Playwright MCP 只在適當的動態頁面或 browser-visible evaluation 場景出現。
  Verify: 研究流程仍維持本地優先與來源歸因，mcp-builder 只在 browser-visible server evaluation 時引入 Playwright MCP。
- [ ] T-008 — 更新維護者與使用者文件，涵蓋 `docs/devguide.md`、`docs/personalization.md`、`docs/personalization.zh-Hant.md`、`scripts/scripts.md`，並視需要最小化調整 `README.md` 或 `config.example.env`，說明 setup 重跑、本地覆寫、輸出產物與禁止追蹤值。
  Verify: 讀者能分辨哪些設定屬於追蹤預設、`mcp.local.json` 覆寫、machine-local 值，以及何時需要重新執行 setup/sync。
- [ ] T-009 — 透過既有 sync/setup 路徑重新產生衍生 adapter 與命令輸出，將 `.github/copilot-instructions.md`、`AGENTS.md`、`CLAUDE.md`、`GEMINI.md` 與產生命令技能視為驗證輸出而非手動編輯來源。
  Verify: 產生檔案反映來源變更且無手動 adapter drift。
- [ ] T-010 — 執行跨平台與跨工作流程驗證，覆蓋 manifest parse、首次非互動式啟動、runtime preservation、legacy cleanup、local override、safe defaults、browser routing、research routing、mcp-builder routing、no-tool behavior 與 generated adapter verification。
  Verify: 測試結果可對應回本計畫的需求、成功條件與上方測試矩陣，並能誠實回報 BLOCKED 或降級情況。
