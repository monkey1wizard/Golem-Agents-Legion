---
source: docs/workflows.md
lang: zh-Hant
source_commit: d150f2f4a2e42214916c655d5bdd6680842d8217
translated_at: 2026-10-09
type: Guide
title: 工作流程與輔助工具
description: 說明 Golem 角色與叫用方式，並涵蓋規劃、管道、落地、復原、git 與撰寫輔助指令。
tags:
  - workflow
  - golems
  - pipeline
  - finalize
  - invocation
status: stable
---

# 工作流程與輔助工具

## 必要時才詢問，其餘持續工作

已獲授權的步驟若不需要使用者輸入，代理程式 (agent) 會持續工作，直到要求的工作完成，或必要的 GAL 關卡阻擋進度為止。每則進度更新都會寫明具體的下一個動作，代理程式接著就執行它。單純的進度回報不會結束回合，也不需要確認才能繼續。

代理程式只在以下情況詢問：缺少使用者輸入就無法繼續時、執行刪除資料或強制推送 (force-push) 等破壞性動作之前，或修改儲存庫以外的檔案之前。先前明確給予的授權，在其說明的範圍內持續有效。必要的 GAL 核准、收據關卡、受保護路徑審查、執行器 (executor) 輸出保管、工作時段規則與儲存庫限制仍然適用。

## Golem 角色與叫用方式

### 什麼是 golem 代理程式

GAL 專屬代理程式 (golem) 各自具備特定的特化職責，功能、角色定位與適用情境皆不同。本文件由使用者視角說明各角色的挑選與使用方式。權威的角色名冊與分類收錄於 [`plugins/gal-core/agents/agents.md`](../../../plugins/gal-core/agents/agents.md)，工作流程語意則收錄於 [`plugins/gal-core/workflows/coding.md`](../../../plugins/gal-core/workflows/coding.md)。

### 能力表

| Golem | 功能 | 使用時機 |
| --- | --- | --- |
| `golem-architect` | 對抗式計畫審查，涵蓋取捨、過度設計、潛在錯誤面與相依性/API 風險 | 建置前使用 `/deep-planning` 或 `/gal architect` 針對設計進行壓力測試 |
| `golem-analyst` | 商業邏輯審查，涵蓋投資報酬率、領域正確性與使用者影響 | 變更涉及定價、權限、資格或客戶可見規則時 |
| `golem-designer` | UI/UX 與 DevEx 體驗設計、設計系統、無障礙性與即時 UI 稽核 | 處理對外排版、狀態、元件工作，或面向開發者的 DevEx 時 |
| `golem-researcher` | 本機優先 (local-first) 調查、跨來源合成與具備實證的調查結果 | 使用 `/gal research` 或 `/gal deep-research` 取得有憑有據的答案 |
| `golem-implementer` | 依已核准的計畫，以原子提交撰寫實作程式碼 | 代表 `/gal pipeline` 的 CODER 階段 |
| `golem-tester` | 依計畫規格產生測試，並執行真實瀏覽器 QA | 代表 `/gal pipeline` 的 TESTER 階段，推動獨立驗證 |
| `golem-auditor` | 對單一任務執行深度效能與安全性稽核 | 代表 `/gal pipeline` 的 AUDITOR 階段。僅限協調器驅動，不支援裸叫用 `/gal auditor` |
| `golem-debugger` | 以科學方法調查錯誤，採凍結紀律並確認根源 | 錯誤需要先進行有紀律的調查再行修復時，執行 `/gal debugger` |
| `golem-steward` | 管理文件結構、程式碼與文件的偏移、知識擷取與圖表同步 | 使用 `/gal steward`，或在計畫開啟、細化結束與管道結案時自動觸發 |
| `golem-releaser` | 以 API 與 CICD 研究設計規劃階段的釋出流程，並提出設計建議 | 在 `/planning release-<slug>` 之前使用 `/gal releaser`（隔離）或 `/releaser discuss`（脈絡內） |

**角色審查三角**：品質保證職責由三方組成。**ORCHESTRATOR**（管道）掌管個別任務的正確性關卡、執行結束時的目標回推驗證，以及計畫生命週期的結案作業。**AUDITOR** 負責單一任務的深度效能與安全性。**STEWARD** 維護文件結構的完整，並保留持久知識。

### Golem 叫用矩陣

| 角色 | 可直接叫用？ | 叫用方式 |
| --- | --- | --- |
| **architect** | 是 | `/gal architect`（隔離）或 `/architect discuss`（脈絡內）。獨立指令：是。支援 `discuss`：是 |
| **analyst** | 是 | `/gal analyst`（隔離）或 `/analyst discuss`（脈絡內）。獨立指令：是。支援 `discuss`：是 |
| **designer** | 是 | `/gal designer`（隔離）或 `/designer discuss`（脈絡內）。獨立指令：是。支援 `discuss`：是 |
| **releaser** | 是 | `/gal releaser`（隔離）或 `/releaser discuss`（脈絡內）。獨立指令：是。支援 `discuss`：是。規劃階段設計者，不執行 |
| **debugger** | 是 | `/gal debugger`。無獨立指令，也不支援 `discuss` |
| **steward** | 是 | `/gal steward`。有獨立指令，但該指令不支援 `discuss` |
| **implementer** | **僅限協調器驅動** | 僅在 `/gal pipeline` 的 `CODER` 階段叫用 |
| **tester** | **僅限協調器驅動** | 僅在 `/gal pipeline` 的 `TESTER` 階段叫用 |
| **auditor** | **僅限協調器驅動** | 僅在 `/gal pipeline` 的 `AUDITOR` 階段叫用。auditor 不支援整個分支的模式。`--finalize-branch-audit` 旗標仍會被辨識，但也一律回傳非派送 (dispatch) 的 `COMMAND: error` 區塊。`/gal finalize` 的審查會沿用此路由，經由 `gal pipeline <execution-prompt> --phase finalize-review` 派送，並標記 `Review Independence: full`。只有在路由不存在時，才允許在行程內審查，並標記 `Review Independence: DEGRADED_SAME_RUNTIME`。審查會產出需求對 L1-L4 的表格 |
| **researcher** | **僅限協調器驅動** | 僅經由 `/gal research` 或 `/gal deep-research` |

在未處於相符協調脈絡的情況下執行 `/gal <role>`，上述四個僅限協調器驅動的角色一律回傳 `COMMAND: error`。

### 對話內角色諮詢

architect、analyst、designer 與 releaser 支援獨立呼叫和對話內諮詢。

| 模式 | 指令 | 行為 | 回應標籤 |
| --- | --- | --- | --- |
| **隔離**（預設） | `/gal <role>` | 由原生子代理程式執行，只將摘要與判定帶回主對話。 | `[<role> · isolated]` |
| **對話內** | `/<role> discuss` | 載入角色指示，並在目前對話中持續交流。 | `[<role> · in-context]` |

**中途加入**：從隔離模式切換到對話內討論時，助理會把先前的隔離判定保留在對話紀錄中，直接接續討論，不會重新執行最初的評估。

四個角色各有一個同名的獨立指令：`architect`、`analyst`、`designer` 與 `releaser`。指令可接受選用的第一個引數 `discuss`，比對時視為完整單字且不分大小寫。帶有 `discuss` 時，指令會移除該字一次，並以其餘文字執行內部子指令 `gal consult-script golem-<role>`。沒有 `discuss` 時，指令以隔離模式執行 `gal dispatch-script golem-<role>`。`gal consult-script` 屬於內部指令，與 `gal finalize-check`、`gal pipeline-log` 相同，不屬於公開的 `/gal` 介面。

獨立指令的寫法依執行環境而異：

| 執行環境 | 隔離 | 對話內 |
| --- | --- | --- |
| Claude Code | `/gal <role>` | `/<role> discuss` |
| Claude Code 外掛模式 | `/gal <role>` | `/gal:<role> discuss` |
| Codex | `$gal <role>` | `$<role> discuss` |

已移除的 `/gal discuss <role>` 與 `$discuss-<role>` 不再可用。`/gal discuss <role>` 會回傳 `COMMAND: error` 並附上替代指令。`/gal <role>` 後面的第一個內容字若是 `discuss`，同樣回傳 `COMMAND: error`，且不會以隔離模式執行。`$discuss-<role>` 技能會在 `gal refresh` 後消失，沒有替代用的轉址。

**直接呼叫的設定範圍**：四個角色的隔離直接呼叫會輸出 `ROUTING_SCOPE: codex-native-projection-only` 與一則 `ROUTING_NOTE`。該註記說明三件事：`planning` 設定只供應 Codex 原生代理程式投影、不會啟動任何執行器、實際使用的模型由主機決定。discuss 呼叫會輸出 `CONSULT_MODE: in-context`，沒有這則註記。詳見[規劃群組範圍](./configuration.zh-Hant.md#規劃群組範圍)。

所有審查角色都採用 [`adversarial-review`](../../../plugins/gal-core/skills/adversarial-review/SKILL.md) 方法：先建立最有力的解讀，預設抱持懷疑，堅守證據標準，並明確給出 `APPROVE`、`REVISE` 或 `REJECT` 判定。

### Steward 生命週期分割

steward 在規劃與落地兩個階段執行截然不同的操作，兩者嚴格禁止互換。

- **規劃階段 (`/deep-planning`)：專注於計畫文件結構。** steward 評估計畫檔案的結構屬性，包含路徑、代稱、必備區段、語言一致性與圖表同步。此階段禁止寫入 `docs/`。規劃階段的文件尚未實作，亦無持久知識，未建置的推測性內容一旦寫入 `docs/`，便會汙染讀者可見的文件層。
- **落地階段 (`/gal finalize`)：持久知識落地。** 已完成的計畫知識必須寫入 `docs/`，此為強制規則。steward 將已實作的計畫知識擷取至持久層（`README.md` 與 `docs/`），再將索引同步至 `.dev/project.md`。在刪除計畫檔案之前，steward 必須先產生持久層的提交。

知識擷取需要先有建置完備的知識，而該知識僅於實作後才會存在。因此，將完成的計畫寫為正式文件是落地階段的工作，不屬於規劃階段。規劃階段的 steward 僅負責維護計畫文件的格式，兩者職責嚴格劃分。

### 釋出計畫路徑

釋出流程以專屬的計畫類型（`release-<slug>`）進行，與一般功能的管道各自獨立運作。

**流程：**

1. `/gal releaser` 先確認部署目標，再由唯讀來源（技能、API 與 CLI 設定檔）調查本機可用的部署能力，據此設計釋出與 DevOps 流程。此步驟為嚴格唯讀，不寫入檔案、不建立提交、亦不執行。無法辨識的能力會回報 `not-available`，藉此杜絕憑空編造。
2. `/planning release-<slug>` 將產生的建議具體化為一份來源計畫 (source plan)，內含 `## Tasks` 區段。
3. 以標準的 `/gal pipeline` 執行已核准的釋出計畫。
4. `/gal finalize` 循標準計畫相同的程序將釋出計畫落地結案。

此流程支援 GAL 自身 CLI 二進位檔的釋出，亦支援下游儲存庫的部署工作（例如網頁服務、後端服務、Firebase 專案、npm 套件與 Docker 映像檔）。操作範圍僅止於設計建議。releaser 不負責部署協調，亦不涵蓋金絲雀釋出 (canary)、還原 (rollback) 或正式環境監控。

## 執行工作流程

流程圖和各階段概覽請參閱 [README](./README.zh-Hant.md#功能生命週期)。本節詳述使用者決策、手動操作以及工作流程中斷後的必要復原動作。

### 查詢目前狀態與下一步

`/gal status` 讀取 `.dev/state.md` 與使用中的執行提示檔，輸出完整的狀態投影：使用中的計畫、工作流程位置、審查與測試結果、阻擋項目、工作階段延續性與專家角色就緒度。

`/gal whats-next` 讀取相同的兩份檔案，但僅回答一項核心問題：現在該執行哪一道指令。它由目前工作目錄向上尋找最近一層含有 `.dev/state.md` 的祖先目錄作為儲存庫根目錄，接著推薦單一後續指令並附帶最精簡的必要脈絡。不確定下一步該做什麼時可使用此指令，需要完整盤點時則使用 `/gal status`。

### 規劃：您的決策

`/planning` 將使用者需求轉換為來源計畫，存放於 `.dev/plans/<type>-<slug>.md`。規劃階段支援人機協作，使用者可自由討論、合併或分割計畫，亦可隨時諮詢 golem 代理程式。

**開放問題由使用者收斂**：`/deep-planning` 要求必須先解決所有 `## Open Questions` 項目，才能執行 `/refining-plan`。問題依三種分級處理：**H** 級僅能由人類決策，**A** 級允許架構角色依記錄的理由自行決策，**F** 代表非實質問題。未標明分級的問題一律視為 H 級。

**核准必須明確記錄**：`/refining-plan` 收斂之後，會將核准狀態記錄於計畫的 `## Approval` 區段，格式固定為四行。`/plan-to-prompt` 要求其中必須包含 `- Human approval: [approved]`，若缺少此行即拒絕產生執行提示檔。

**任務大小與擁有者檢查**：`/refining-plan` 會寫出以行為為單位的任務，每個測試點都由代理程式執行。只有人類能做的檢查，放入選用的 `## Owner Acceptance` 區段。只有擁有者能滿足的環境需求，放入選用的 `## Preconditions` 區段。`/plan-to-prompt` 會把這兩個區段逐位元組相同地複製到執行提示檔。

**多份計畫同時進行時必須指定目標**：若有多份使用中的計畫並行，叫用任何規劃指令時皆須明確指定計畫檔案路徑。GAL 絕不自動代選。

### 匯入外部計畫

`/gal import-plan <source>` 可將在其他工具中完成的計畫匯入 GAL，略過 `/planning` → `/deep-planning` → `/refining-plan` → `/plan-to-prompt` 的標準流程。`<source>` 可為實體檔案路徑，亦可為直接貼上的計畫文字。此路徑支援 Spec-Kit 的 `tasks.md`、BMAD stories、GSD 計畫、Claude 或 Codex 原生規劃模式的產出，以及自由格式的計畫文字。

整個交接過程只需使用者執行兩道指令。第一道為 `/gal import-plan <source>`，GAL 會將來源轉換為 `.dev/plans/<slug>.prompt.md`，同時產生配對的佔位計畫，最後執行 fail-closed（預設阻擋）的 `gal prompt-check --assemble-dry-run --receipt <path>` 關卡。這道關卡檢查三項要件：各任務具備詳盡標題、任務規格能成功組裝，且受影響檔案的允許清單不可為空。若關卡未通過，收據會立即中斷匯入流程，計畫亦不會被登錄。第二道指令為 `/gal pipeline`，僅在匯入收據通過後執行。若目前存在多份使用中的計畫，必須明確指定剛匯入的提示檔路徑。

產生的提示檔會在 `## Status` 區段加入 `imported-from:` 行，記錄來源工具名稱與來源路徑（若可取得），或標記為 `pasted`。此行僅作為出處紀錄，外部文件一律視為資料，不對 GAL 下達任何指示。

此快速路徑刻意略過工程審查與架構師審核，因此嚴謹深度不及標準規劃流程。其正確性依賴三道防線：匯入合約規範的任務不可分割規則、將粗粒度工作拆解為彼此獨立且可測試的任務，以及組裝前的試跑關卡。

### 管道：啟動、停止、繼續

執行 `/gal pipeline` 即可啟動工作流程。若只有單一使用中的計畫，GAL 會自動解析出對應的執行提示檔。若有多份計畫，必須明確指定提示檔路徑（`.dev/plans/<slug>.prompt.md`）。

對於舊版或手動協調器程序，請在修改實作前執行 `gal pipeline-preflight <execution-prompt-path>`，並要求 `.dev/pipeline/<plan-scope-key>/preflight.receipt.md` 的 `overall: pass`。收據缺漏、失敗或未執行時，入口會停止。獨立收據只代表此手動程序的入口證據，不授權進入受防護的 `codex_stop_v1` 區段。在這次 preflight 之前，協調器會執行選用的 `## Preconditions` 表中每一個 `Check command`。若任何結果與 `Expected result` 不符，管道會在第一個任務之前停止，並列出未滿足的列。

對於 `codex_stop_v1`，受防護的驅動器會先檢查投影日誌。若日誌記錄尚未完成的作業，驅動器會先復原該作業。接著，它會確認執行中的執行檔與已安裝執行檔符合釘住的執行檔雜湊。若證明缺漏或不相符，驅動器會記錄 `self-bootstrap` 檢查點並返回。請自舉根目錄簽出，並透過檢查點授權的動作繼續。證明相符時，驅動器會在行程內叫用 `pipeline_preflight::cmd_pipeline_preflight`，並要求與該區段繫結的新鮮收據。繫結收據若缺漏、過期、失敗、格式錯誤或無法寫入，區段會在啟動供應商或修改權威資料前停止。

GAL 預設使用 `legacy_interactive` 延續設定檔。此設定檔保留 Claude Code 與 OpenCode 目前由主機掌握的延續行為、叫用語法、行程結束行為、投影配置，以及 v1 證據、收據、標記與修改順序語意。一般 `gal pipeline <prompt>` 叫用仍使用此設定檔。執行器名稱、環境、過期標記，以及缺少其他主機標記，都不會選取設定檔。

受信任的 Codex hook 交握程序必須先提出授權，而且該授權必須有效並只能使用一次。符合這項條件後，系統才能使用 `codex_stop_v1` 設定檔。設定檔名稱描述延續合約，不描述執行器。關於 hook 信任、就緒檢查與自舉復原，請參閱 [Codex 管道執行設定](./setup.zh-Hant.md#codex-管道執行設定)。

請使用 `gal pipeline <prompt> --require-codex-stop-v1 --status` 讀取受界限的協調器狀態。狀態讀取為唯讀，不會變更協調器、游標或嘗試檔案。受防護的延續程序接受檢查點收據提交或繼續動作。手動復原動作必須同時指定任務與階段。當前關卡會提供允許的具型別動作及其修訂繫結。

在 `codex_stop_v1` 中，每次 `gal pipeline` 叫用都執行一個會阻塞的機械區段。`.dev/pipeline/<plan-scope>/` 下的持久協調器會記錄提示繫結、目前任務與階段、嘗試、重試狀態、已驗證證據、待處理投影交易、檢查點、啟用摘要與下一個動作。區段遇到下列任一狀態就會返回：ORCHESTRATOR 檢查點、最終結果、需要人類權限的阻擋項目，或不可復原的證據失敗。它不會在行程內等待語意判斷。

ORCHESTRATOR 保留任務品質審查、範圍擴張判斷、收斂判斷與目標回推驗證。任務開始前，任務品質檢查點必須通過。驗證後的進度投影前，邊界與收斂檢查必須通過。最後一個任務收斂後，ORCHESTRATOR 完成 `goal_backward` 檢查點。對於 `awaiting-orchestrator` 檢查點，請提交繫結該檢查點、協調器修訂、提示或規格雜湊及提交的收據。接著執行目前關卡返回的單一具型別繼續動作。後續叫用會重新開啟持久協調器。

讀取協調器狀態或復原動作時，請依下列受防護區段流程處理：

```text
管道入口
  -> 復原待處理的投影日誌
       -> 第三種雜湊衝突：保留目標並停止
  -> 檢查釘住的執行檔證明
       -> 缺漏或不相符：記錄 self-bootstrap 檢查點並返回
            -> 自舉根目錄簽出，透過授權動作繼續
  -> 新鮮且繫結的入口檢查通過
       -> 失敗或缺漏：在啟動供應商或修改權威資料前停止
  -> 任務品質檢查點
       -> 拒絕：在任務工作前停止
  -> 派送並驗證嘗試證據
       -> 不完整或衝突：具型別證據失敗
  -> 必要時進入 ORCHESTRATOR 檢查點
       -> 提交繫結收據並執行具型別繼續動作
  -> 一次具型別轉移
       -> 重試／下一個任務／任務完成
       -> 需要人類處理或終端失敗
  -> 最後任務之後進入 goal_backward 檢查點
       -> goal-verified 或終端驗證失敗
```

在日誌復原步驟中，只有目標仍符合記錄的舊雜湊時，GAL 才會套用暫存替換。目標符合新雜湊時，GAL 會記錄替換已套用。第三種雜湊會停止復原並保留目標。只有日誌記錄投影已提交後，協調器才會前進。新鮮入口檢查收據若缺漏、過期、失敗、格式錯誤或無法寫入，會在啟動供應商或修改權威資料前停止。使用中的擁有者以唯讀方式附加。若孤兒狀態的識別相符，GAL 會從已驗證的持久狀態繼續。未知或衝突的擁有者會以具型別衝突停止。

管道會依序執行任務品質審查、實作與任務提交、獨立驗證測試、稽核、三表面收斂與回交檢查。每個任務收斂後，回交檢查器會返回下一個動作。所有任務收斂後，ORCHESTRATOR 會在行程內執行目標回推驗證。取得新的已驗證 `goal-verified` 收據後，管道會把 `/gal finalize` 當作最後一個節點執行。因此一次正常的叫用，會以計畫落地或一個明確的停止點作結。只有具型別的人類必要決策、尚未完成的 Owner Acceptance 列，或其他繫結停止規則需要操作者介入。管道啟動本身不是接受的證據。唯讀的 terminal-reverify 不會接續進入 finalize。

若判定為 `continue`，回交動作為 `/gal pipeline <prompt> from T-NN [stop-at T-MM]`。`stop-at` 目標在待處理期間仍保留於動作中。檢查器只接受此封閉工作流程語法及指定的復原動作。終端判定使用 `continue_action: none`。`gal.exe pipeline <prompt> from T-NN` 不是有效的 CLI 叫用方式。

重試回合需要更新授權及具體程式碼修改。測試或稽核失敗後，管道會記錄未解決的重試交接筆記，並以 `--fix` 重新派送實作工作者。若後續嘗試重複相同的目標、交接筆記、檔案允許清單與合約，重試會在執行前被拒絕。若執行器正常結束卻未修改允許清單內的檔案，該回合會以 `fix-round-no-change` 失敗，因為只更新提示紀錄或收據不算實質變更。

管道會在未解決的阻擋項目需要人類決策、任務達到三次失敗的重試上限、儲存庫分支分歧、範圍邊界受質疑或目標差距檢查失敗時停止。在受防護模式中，這些結果會以具型別回交或終端狀態返回。只有收據通過後，目標驗證才會返回 `goal-verified`。`human-required`、`retry-ceiling`、`stop-at` 與終端驗證失敗會停止管道，且不攜帶可執行的繼續動作。每個 `human-required` 停止都會寫入一個回交區塊，除了 `Next human step` 之外，還包含 `What to check`、`Expected result` 與 `Pass/fail rule`，讓擁有者只看該區塊就能判定通過或失敗。自動駕駛管道不受互動式工作時段 Hard Stop 限制。執行時限會產生中斷筆記與僅供復原的繼續標記，不會產生未獲證明的核准或授權最終回應。

舊版中斷發生時，請解決根本原因並重新執行相同的 `/gal pipeline` 指令。受防護程序中斷時，請使用已授權的具型別動作。該動作會重新開啟同一份提示與同一個協調器。只有啟用摘要與嘗試識別相符時，GAL 才會繼續。未知擁有者或衝突識別需要人類處理，且絕不授權自動接管、終止行程或重新派送。若重播偵測拒絕先前嘗試，請以不同的補救細節更新交接筆記。已完成的任務會保留，後續執行會自動略過。

### 管道：任務迴圈與角色

管道執行的是一個任務迴圈，依序為：任務檢查、游標寫入、實作、協調、實作提交、測試、稽核、收斂。其中 ORCHESTRATOR 負責任務檢查，以純 `git commit` 建立實作提交，並掌管游標更新、收斂判定與移交落地階段。

**任務檢查**：ORCHESTRATOR 讀取 `plugins/gal-core/conventions/task-quality.md`，逐條確認該任務完整回答了每一項適用的檢核項目。只要有任一項目缺漏，管道會在派送前立即中斷，並要求回到 `/refining-plan`。

**實作**：CODER 僅接收三項輸入：任務本身、其實作合約與儲存庫。它僅更動允許清單內列出的檔案，並回傳執行器日誌。CODER 不建立提交。

**協調與提交**：ORCHESTRATOR 比對各角色的持久回傳，確認所有更動路徑皆符合任務允許清單，執行 `gal boundary-check` 後以純 `git commit` 建立 `Task Final Commit`。

**測試**：TESTER 針對 `Task Final Commit` 執行，涵蓋每一條適用的 `TP-NN` 行，並將結果記錄於 `## Test Results` 區段。TESTER 不建立提交。

**稽核**：AUDITOR 審查 `Task Base Commit..Task Final Commit` 區間，將具備實證的判定記錄於 `## Review Results` 區段。

**收斂**：ORCHESTRATOR 評估各角色的持久回傳與收據。發現不符時重新派送該角色。測試或稽核失敗時，以 `--fix` 重新派送實作角色，並將修復後的提交記錄為新的 `Task Final Commit`。

各個角色彼此獨立。任何被派送的角色皆不會收到其他角色的回傳、交接筆記、收據或 diff 差異。

### 落地：哪些內容併入主線，哪些被刪除

`/gal finalize` 負責編排已完成目標驗證計畫的結案程序。完整的 `gal finalize-check` 會檢查儲存庫層級指令、命名、轉接器穩定性、文件連結、狀態界限與乾淨工作樹，不會重新評估每個任務。此關卡不會變更程式碼或文件，只會寫出指定的驗證收據。

**進入路徑**：

- 路徑 A：擁有者要求 `/gal finalize`，且回交收據帶有 `decision: goal-verified` 與 `voluntary_response_authorized: true`。finalize 停止後的每一次續行都走這條路徑。
- 路徑 B：正常的 `/gal pipeline` 完成。管道在同一次叫用中產生並驗證同一份收據，然後自行進入 finalize。不需要第二個指令，也不需要在合併或刪除前做例行確認。
- 單靠已儲存的收據、狀態行或先前的最終回應，不會啟動 finalize。兩條路徑都保留完整模式關卡、審查停止、衝突規則、執行器輸出保管、衛生關卡與不推送規則。

**Owner Acceptance**：人工接受與落地權限是兩件事。在任何實質的 finalize 工作之前，finalize 會先讀取 `## Owner Acceptance` 表，以及 `### Handoff Notes` 底下的 `#### Owner Acceptance Evidence` 表。

- 沒有表、沒有列，或每一列都已記錄為 `accepted` 或 `waived`：finalize 不經核可提示直接繼續。
- 有任何未完成的列：finalize 會逐字顯示這些列，寫入 `Interrupted Phase — finalize / ACCEPTANCE` 標記並停止。標記會列出這些列、檢查項目、預期結果、通過與否的判定規則，以及續行用的 `/gal finalize` 指令。
- 擁有者對所顯示的列明確表示確認，會記錄為 `accepted`。明確指名這些列的豁免，會記錄為 `waived`，並且不會被回報為實測通過。單純的 finalize 要求或續行指示，兩者都不算。
- 在合併之前，或在計畫已位於主線分支時於拆除與刪除之前，finalize 會重新檢查已接受的目標。已接受的準則或目標若有實質變更，受影響的列需要重新接受。無關的中繼資料或文件變更不會使接受失效。

**獨立審查**：由上而下的審查會交給由 `AUDITOR` 路由選定的全新執行器執行。路由與失敗規則請見 [Finalize Review Route](../../configuration.md#finalize-review-route)。

- 審查者會收到來源計畫的目標、需求、成功準則與測試計畫，以及權威指令。分支差異與工作樹由審查者自行讀取。審查規格不含提示歷史、收據或判定。
- 成功需要一份全新的非空收據、以 `completed` 結束的嘗試紀錄，以及供應端的工作階段證據。審查會記錄為一個受管理的 `### Finalize Review <date>` 區塊，並標記 `Review Independence: full`。
- 只有 `AUDITOR` 路由不存在時，才允許在行程內審查，並標記為 `Review Independence: DEGRADED_SAME_RUNTIME`。設定或路由格式錯誤、執行器無法使用、啟動失敗、逾時、證據不完整或結果無效，都會讓 finalize 停止，且不會退回其他做法。
- 審查者不會提交、合併、貼標籤或刪除計畫檔案。文件同步、Git 操作與生命週期權限仍由協調器掌握。

**落地保留項目**：文件同步前，先進行由上而下的逐項需求審查，分為四層記錄：L1 事實、L2 檔案、L3 接線、L4 信任邊界。每一項需求於 `## Review Results` 記錄為一張 `### Finalize Review <date>` 表格，採一需求一列並附帶發現清單。有效的審查即使含有失敗的儲存格或阻擋性發現，也會作為證據交付，並不代表核准落地。接著由協調器依 `golem-steward` 的契約，在行程內將計畫累積的持久知識擷取至 `README.md` 與 `docs/`，並自行審閱文件內容。機械式檢查無法證明文件正確。最後執行併入主線操作，適用時亦一併移除工作樹。

**停止與復原**：每一次 finalize 停止，都會在 `### Handoff Notes` 留下 `Interrupted Phase — finalize` 標記，續行動作是 `/gal finalize`。不要在 `DONE` 的提示上重新啟動一般管道。記錄合併完成之後，finalize 不會再次合併，而是從拆除與結案繼續。

**自動解決的特定衝突**：當併入主線發生衝突，且未合併的路徑集合恰好僅有 `{.dev/state.md}` 時，落地流程會執行內建的 `gal state-merge` 解析器，以計畫為鍵逐列合併兩側表格。解析器以狀態碼 0 結束時落地流程繼續推進。回報 `STATE_MERGE: unresolved` 時則停止落地，並驗證儲存庫維持位元組一致。其餘任何衝突樣態一律無條件停止。

**落地刪除項目**：`.dev/plans/` 下的計畫檔案，必須待文件提交完成且寫入後的衛生檢查通過後才會刪除。此順序確保知識已遷移至持久層後檔案才被清除。計畫檔案刪除後，落地流程執行 `gal pipeline-clean` 移除 `.dev/pipeline/<plan-slug>/`。

**落地狀態寫回**：結案行寫入 `.dev/state.md`，內容包含日期、計畫名稱與落地提交。`### Finalize Review <date>` 表格中記錄的各項非阻擋發現，會插入 `.dev/state.md` 的 `## Follow-ups` 區段，新項目排在前面並保留最新五筆。如此一來，即使計畫檔案已被刪除，這些發現仍可持續留存。最後將 `gal-last-good` 標籤指向本次落地提交。

## 無頭派送

### 概念

GAL 可將部分管道階段的執行移出對話迴圈，交由另一無頭編碼代理程式 CLI 處理。此機制由 `~/.gal/config/config.json` 的 `executorRouting` 設定驅動。設定鍵結構、`combinations` 登錄清單、退回順序與遠端欄位，請參閱 [configuration.zh-Hant.md](./configuration.zh-Hant.md#執行器路由-executorrouting)。本節僅說明派送本身的行為、診斷與復原。

### 角色表

| 角色 | 群組 | 職責 |
| --- | --- | --- |
| `CODER` | pipeline | 依計畫撰寫實作程式碼。條件允許時應與 `TESTER` 不同 |
| `TESTER` | pipeline | 僅依計畫規格與公開 API 撰寫測試。條件允許時應與 `CODER` 不同 |
| `AUDITOR` | pipeline | 稽核深度效能與安全性。條件允許時應與 `CODER` 不同，且層級不得低於 `CODER` |
| `ARCHITECT` | planning | 對抗式計畫審查，涵蓋取捨、過度設計與潛在錯誤 |
| `ANALYST` | planning | 商業邏輯審查，涵蓋投資報酬率、領域正確性與使用者影響 |
| `DESIGNER` | planning | UX、UI 與開發者體驗審查 |
| `RELEASER` | planning | 釋出流程設計，嚴格唯讀 |
| `RESEARCHER` | research | 由三位平行且彼此獨立盲測的工作者之一調查研究問題（`RESEARCHER#0`、`#1`、`#2`） |

`pipeline` 群組採用無頭派送。`planning` 群組僅為 Codex 原生代理程式投影提供 `model` 與 `effort`，不會執行無頭派送，也不會啟動執行器，並忽略 `timeoutSecs`、`sshTarget` 與 `remoteWorkdir`。`research` 群組為選用項目，最多路由三位工作者中的兩位。

### 執行器轉接器行為

- **opencode** 以 `--agent build` 派送，啟用可寫入代理程式以繞過預設會靜默阻擋寫入的限制，並以 `--auto` 作為權限略過旗標。
- **Copilot** 附加 `--no-custom-instructions` 與 `--disable-builtin-mcps`，避免無頭提示模式超出脈絡額度。Copilot Free 嚴格僅支援 `auto`，必須維持 `model: auto`，且僅能在本機執行。

### 安全警告：權限繞過

無頭執行器派送會使用各執行環境的權限選項：Claude Code 與 Antigravity 使用 `--dangerously-skip-permissions`，opencode 使用 `--auto`，Copilot 使用 `--allow-all`。這些選項可能授予本機檔案系統與終端機廣泛存取權。GAL 以 `--dangerously-bypass-approvals-and-sandbox` 啟動 Codex，因此 Codex 子行程同樣在沙盒外執行。Codex 的 Windows 沙盒目前會在執行任何指令之前就失敗，詳見 [setup.zh-Hant.md](./setup.zh-Hant.md#已知問題-codex-windows-沙盒失效)。關於 Codex 模式選取、拒絕復原與回復，請參閱 [setup.zh-Hant.md](./setup.zh-Hant.md#codex-管道執行設定)。

請只在受信任的開發系統與安全儲存庫中啟用執行器路由。請勿對不受信任的原始碼執行自動路由。未驗證的任務合約也不得使用自動路由。雖然代理程式提示禁止執行 `git commit` 或 `git push`，但作業系統控制不會強制此指示。

### 管道執行順序與證據

管道嘗試會使用提示寫入租約，將對同一份權威提示檔的寫入序列化。協調器會先取得此租約，再讀取提示快照並準備可變的階段狀態。接著，它會取得收據租約，直到確認收據置入、終端證據收集與行程清理完成為止。租約會以相反順序釋放。不同提示檔不會共用提示寫入租約。GAL 絕不只依鎖定時間接管遺留租約。復原租約必須驗證擁有者、確認行程樹已終止，並執行授權清理。若無法確認清理，GAL 會保留所有租約與復原產物。

嘗試持有收據租約時，會擷取已驗證的收據位元組與其摘要。階段回寫會直接使用此不可變快照，不會從已釋放的磁碟收據路徑重新讀取。最終關卡會確認磁碟證據符合此擷取快照。

管道關卡只會從目前嘗試評估證據。二進位檔可用性、行程啟動、供應商回應、行程結束狀態、收據驗證與關卡結果都是不同階段。空白日誌、仍在執行的工作者或一段時間沒有活動，都不代表成功或失敗。`PASS` 必須同時滿足管道三項條件與獨立稽核通過。重試必須產生獨立證據，協調器核准不能取代缺少的稽核。

工作流程延續（使用 `from <task>`）與可執行派送（使用明確的階段與任務引數）是不同介面。格式錯誤或過期的延續輸入會在啟動行程前驗證失敗，絕不退回執行初始任務。`stop-at` 等執行界限嚴格屬於工作流程層。工具交還控制權後，協調器會等待原本的主機工作階段，規範見 `gal-pipeline` 技能的 `Headless Executor Dispatch`。

```text
可用性：不存在 -> Missing；觀察到拒絕 -> Denied
  不完整或衝突的查詢 -> Unknown；可用候選 -> 啟動
  -> 供應商回應 -> 終端結果 + 目前收據 -> smoke 結果
  [僅在明確限定的外部比較中使用原生核准]

外層 Codex 權限脈絡 -> 必要時使用原生核准
  -> 驗證輸入 -> 提示寫入租約 -> 不可變提示快照
  -> 收據租約 -> 已啟動證據 -> 啟動子行程
       -> 子行程在 Codex 沙盒外執行
       -> 終端結果 -> 收據置入 -> 最終證據
  -> 確認清理後以相反順序釋放租約
  -> 管道三條件 PASS + 獨立稽核 -> 下一個序列階段

拒絕的要求 -> 真實失敗 + 復原點
未知的清理 -> 保留租約與產物；不自動接管
主機工具交還控制權 -> 等待原本的主機工作階段
  -> 若仍無法確認結束：寫入 Interrupted Phase，保留輸出，不重新派送
```

離線確定性測試是必要管道驗證的基線。選用的即時整合檢查必須在預先授權且可拋棄的環境中明確選擇執行。涵蓋 Desktop 模式選取、使用者登入與即時環境行為的開發者驗收測試，會以獨立證據追蹤。省略的手動或即時檢查標記為 **NOT RUN**，而非 `PASS`，且不會阻擋管道執行或結案。執行這些檢查時，請遵循 [Codex 設定檢查清單](./setup.zh-Hant.md#開發者驗收清單)。

### 遠端執行（SSH 派送通道）

跨機器執行直接在 `executorRouting` 中設定，是已解析路由的屬性，並非另一道獨立指令。無論 `CODER`、`TESTER` 與 `AUDITOR` 解析為本機或遠端叫用，`/gal pipeline` 的行為皆完全相同。

**先決條件**（須直接於遠端機器預先備妥，GAL 不提供任何自動佈建）：

- 目標主機支援免密碼 SSH 存取（`BatchMode=yes`）。互動式密碼或通行密碼提示皆視為連線失敗，並阻擋重試。
- 被路由的代理程式 CLI（`claude`、`codex`、`agy`、`opencode`）必須已於遠端機器安裝並完成登入。`gal` 二進位檔**無須**存在於遠端。
- `remoteWorkdir` 中的 git 儲存庫，在每次派送前皆必須與控制節點簽出的 commit 一致，且工作樹必須乾淨。

**預期行為**：派送透過維持中的 SSH 工作階段同步執行，生命週期與本機派送一致。可變動的實作階段成功完成後，GAL 會將遠端未提交的變更差異取回並套用至控制節點。僅在控制節點本機套用成功後，GAL 才會將遠端簽出重設為乾淨狀態（`git reset --hard && git clean -fd`），嚴禁顛倒執行順序。若本機套用失敗，遠端簽出原樣保留以供除錯檢查。

`remoteWorkdir` 僅能指向 **GAL 專用的簽出目錄**。切勿指向用於日常互動式工作的簽出，因為套用後的清理流程依設計具備破壞性。

**已知限制**：Copilot 不適合遠端路由，因其傳遞 CLI 旗標的方式無法跨 SSH 命令列轉送，此類路由將觸發明確錯誤中斷而非靜默降級。Windows 遠端目標不受支援，因其組合之指令需要 POSIX 登入 shell。系統不具備斷線續存機制，SSH 連線一旦中斷，派送即宣告失敗，不自動重新連線亦不重試。遠端通道採單次派送、人工重新同步的模式：控制節點的 HEAD 於每次任務提交後前進，遠端簽出則停留在前次同步點，同一次執行中較晚的遠端派送將觸發保護機制而中斷，直到簽出經手動重新同步為止。

### 被派送工作者的邊界

`/gal pipeline` 將任務階段派送給無頭執行器時，次要編碼代理程式的角色為工作者，而非協調器。為避免被派送的工作者將儲存庫層級指示（例如具名工作流程遵守規則或協調器關卡要求）誤當成控制管道的指令執行，GAL 運用兩道互補機制建立角色隔離。

第一道為任務規格邊界。每份算繪出的任務規格皆內嵌 `## Dispatched Worker Boundary` 區塊，置於中繼資料分隔線後、`## Task Goal` 之前。此區塊將工作者定界為單一範圍的階段執行器而非協調器，明確禁止叫用任何 `gal` 子指令（包含 `gal pipeline-preflight`、`gal pipeline-handback-check`、`gal boundary-check`、`gal pipeline-converge-check` 與 `gal pipeline`），因為各項必要的管道關卡皆由協調器掌握且已獲滿足，同時明確禁止載入或執行任何工作流程 `SKILL.md`。此區塊會覆寫任何要求代理程式執行協調器工作流程的儲存庫層級指示或技能指示。被派送的工作者僅能遵循限定範圍的任務目標、檔案允許清單，以及內嵌的 `## Agent Contract`。

第二道為專案指示隔離。依執行器 CLI 特性，儲存庫層級指示檔案（`AGENTS.md` 或 `CLAUDE.md`）可能於啟動時自動載入模型的指示脈絡中。在支援的執行環境中，GAL 派送轉接器會傳入抑制專案指示注入的旗標，確保自成一體的任務規格能獨立主導執行內容。

| 執行器 | 曝露狀態 | 抑制機制 | 驗證版本 | 隔離行為 |
| --- | --- | --- | --- | --- |
| **codex** | 曝露（`AGENTS.md`） | `-c project_doc_max_bytes=0` | codex-cli 0.149.1 | 無條件傳入 `-c project_doc_max_bytes=0` 抑制儲存庫層級的 `AGENTS.md`，全域 `~/.codex/AGENTS.md` 層依然生效 |
| **copilot** | 曝露 | `--no-custom-instructions`、`--disable-builtin-mcps` | Copilot CLI | 附加旗標停用自訂指示與內建 MCP，維持提示模式自成一體 |
| **claude** | 曝露（`CLAUDE.md`） | `--setting-sources user` | Claude Code 2.1.251 | 無條件傳入 `--setting-sources user` 抑制儲存庫層級的 `CLAUDE.md` 與專案設定，保留使用者設定 |
| **opencode** | `NotRun` | 無 | opencode 1.18.25 | 曝露探測於測試期間遭遇供應商逾時，轉接器不套用任何抑制。此結果視為未定論，而非未曝露 |

## 派送紀錄與診斷

每次派送皆會留下**兩份**持久紀錄，依任務目標建立。

若主機工具在派送結束前先交還控制權，協調器會等待原本的主機工作階段結束。若仍無法確認派送行程已結束，協調器會記錄 `Interrupted Phase`，保留現有輸出，且不會重新派送。相關規範見 `gal-pipeline` 技能的 `Headless Executor Dispatch`。

### 第一層：GAL 執行器日誌

此為一致格式的稽核軌跡，涵蓋五大工具。每次執行記錄存放於 `.dev/pipeline/<plan-slug>/<task>/`，直接派送則存放於 `.dev/pipeline/<yyyymmdd>/test-direct/`。檔名格式為 `<timestamp>-<attempt>-<task>-<phase>-<executor>.log`。標頭記錄終端狀態、結束碼、實際使用的模型、git 分支與 HEAD，以及供應商的 `session_id`。`---STDOUT---` 區塊完整收錄供應商的事件串流，涵蓋代理程式訊息、指令執行、檔案修改與權杖消耗。欲稽核執行器的實際操作請查閱此日誌。供應商本機的對話紀錄僅供參考，此日誌才是儲存庫正式採納的憑據。

日誌標頭將派送結果分類為下列終端狀態：

| 終端狀態 | 意義 |
| --- | --- |
| `completed` | 行程以 0 結束，確認取得收據，且工作樹具備預期變更 |
| `no-receipt` | 行程以 0 結束，但預期的收據檔案不存在、內容為空，或無法確認 |
| `workdir-escape` | 僅供歷史日誌詞彙與比對完整性保留，正式派送路徑不會產生 |
| `no-writeback` | 行程以 0 結束並回傳收據，但指派之 git 工作樹經比對顯示零更動 |
| `timeout` | 行程因超過設定之逾時上限而遭終止 |
| `timeout-no-output` | 尚未產生任何輸出即因行程樹逾時而遭終止。此為相容性狀態標記，單憑輸出無法斷定成因 |
| `timeout-midrun` | 產生部分輸出後因行程樹逾時而遭終止。此為相容性狀態標記 |
| `disconnected-partial` | 行程以非零狀態碼結束離開 |
| `unavailable` | 於 `PATH` 中找不到被路由的執行器 CLI 二進位檔 |

此表為終端狀態詞彙的唯一權威來源。`started` 僅為啟動前的嘗試標記，非終端狀態。

`no-writeback` 具備四項操作邊界：

- **階段範圍**：`no-writeback` 嚴格僅適用於 `implement` 階段。排除 `audit` 與 `test`，因這兩個階段合約要求的產出全數存放於 `.dev/pipeline/` 下受 gitignore 忽略的收據檔案中，git 工作樹快照無法偵測該處檔案。
- **同旗標邊界**：受追蹤或未受追蹤的檔案於派送前後若維持相同之 `git status --porcelain` 狀態（例如原已修改的檔案），改採 SHA-256 內容摘要比對偵測變更，避免將實質交付誤判為 `no-writeback`。
- **gitignore 寫入邊界**：若某階段的變更全數落於受 gitignore 忽略的路徑，即便執行器確實完成寫入，仍回報 `no-writeback`，因工作樹狀態快照僅追蹤儲存庫變更，不考量被忽略的路徑。
- **交付合約耦合**：此分類器緊扣當前階段的交付合約，`scaffold` 與 `implement` 經由受追蹤的儲存庫檔案交付，`audit` 與 `test` 則經由收據交付。若任一階段調整交付方式，必須重新審視此分類邏輯。

### 判讀 `contract` 與 `contract_source` 欄位

由提示檔或來源計畫建立的管道派送，會在 `Dispatch:` 標記行以及執行器日誌的啟動標頭與終端標頭上，附加 `contract=<控制節點絕對路徑> contract_source=workdir|ancestor|exe-side|embedded`。三個位置記載的是同一組耦合值，任一處皆能反映執行器實際執行哪一份代理程式合約及其來源出處。`contract_source=workdir` 或 `ancestor` 代表合約來自本機 GAL 簽出，行為看似過期時應優先檢查該處簽出。`exe-side` 代表封裝二進位檔隨附的副本。`embedded` 代表於 `~/.gal/embedded-src` 具體化之備援副本。原始 `gal dispatch` 與原始任務規格形式之 `gal pipeline` 完全不帶此類欄位，未顯示屬於預期行為，非屬瑕疵。

### 判讀具出處防護的 `effort` 欄位

僅具備出處之管道派送方含有此欄位。該次執行的每筆成功或降級標記，皆會在 `contract` 與 `contract_source` 字尾前附帶經過淨化的 `effort=<值|(default)>` 欄位。該值於路由解析後計算一次，後續整輪維持不變。原始派送、直接派送，以及 `no-routing` 降級（未解析出路由，因而無出處資訊），一律不帶 `effort` 欄位，其標記與加入 `effort` 前之格式維持逐位元組相同。當路由產生 `OFFLOAD` 區塊時，`gal dispatch-script` 會一併預先算繪 `REPORT_LINE` 欄位（`Dispatched: <phase[ (fix)]> <T-NN> - <ROLE> as <executor>, model <model>, effort <effort>`），供協調器於各派送點原樣宣告一次。

### 第二層：供應商原生工作階段續行

日誌標頭記錄可供續行之 `session_id`。利用此識別碼可在供應商的原生介面中接續對話。各工具指令有所不同：

| 執行器 | 原生檢視／續行指令 | 無頭工作階段的預設可見性 |
| --- | --- | --- |
| **claude** | `claude --resume <session_id>` | 列出顯示 |
| **codex** | `codex resume <uuid>`（UUID 會略過篩選） | **隱藏**，須加上 `codex resume --include-non-interactive` 方會出現於選擇器，加上 `--all` 可停用工作目錄篩選 |
| **opencode** | `opencode run -s <session_id>` 續行、`opencode export <session_id>` 匯出 JSON、`opencode session list` 瀏覽 | 列出顯示 |
| **copilot** | `copilot --resume=<session_id>` | 存放於 `~/.copilot/session-store.db`，僅能依 ID 續行，無公開列表指令 |
| **agy** | `agy --conversation <uuid>` | 存放於 `~/.gemini/antigravity-cli/brain/<uuid>/`，無列表子指令，僅能依 ID 瀏覽 |

**通用原則**：欲稽核派送請查閱第一層執行器日誌，欲接續操作請使用第二層指令返回原生工具。Codex 預設隱藏無頭工作階段為其原生行為，並非 GAL 的設定所致。

### 執行器自我測試

`gal doctor --executor-smoke` 循實際管道任務完全相同的無頭派送路徑，逐一測試 `codex`、`claude`、`copilot`、`agy` 與 `opencode` 五大 CLI。它使用合成的路由檔執行，與正式之 `config.json#executorRouting` 隔離，僅執行寫出一行收據的最小任務。測試結果持久保存於受 gitignore 忽略的時間戳目錄中，預設路徑為 `.dev/pipeline/<yyyymmdd>/test-executor-smoke-<HHMMSS>Z/local/`，內容涵蓋 JSON 報告、供閱讀的表格，以及各代理程式的日誌與收據。安裝與叫用方式請參閱 [setup.zh-Hant.md](./setup.zh-Hant.md#第一次-doctor-檢查)。

| 狀態 | 意義 |
| --- | --- |
| `PASS` | 實際派送完成，且收據通過驗證 |
| `NOT_INSTALLED` | 已確認目前派送環境找不到該 CLI。查詢被拒或無法判定時，不會回報為未安裝。請安裝該 CLI，並確認它位於 `PATH` 上 |
| `NOT_AUTHENTICATED` | 確認未登入，請執行該工具的登入指令 |
| `ACCESS_DENIED` | 存取執行器啟動路徑時遭拒，不會啟動供應端。請檢查執行檔權限與相關政策 |
| `AVAILABILITY_UNKNOWN` | 無法解析執行器啟動路徑，不會啟動供應端。請檢查設定的執行檔路徑 |
| `AUTH_UNKNOWN` | 就緒狀態無法確認，將嘗試一次有界叫用並回報實際結果。單次搜尋或查詢失敗不代表未安裝 |
| `UNSUPPORTED` | `--executor` 名稱不在五大受支援代理程式中，絕不派送 |
| `CONFIG_ERROR` | 派送前即發現問題（常見於不安全的 `--report-dir`），於寫入前即行攔截 |
| `CALL_FAILED` | 執行器以非零結束碼離開，請檢查該次執行的執行器日誌 |
| `NO_RECEIPT` | 執行器以 0 結束卻未產生收據。結束碼 0 單獨存在時不代表成功 |
| `TIMEOUT` | 超過有界逾時時間。若工具本身執行較慢可放寬 `--timeout`，否則應檢查是否卡在互動式提示 |

遠端自我測試使用 `--transport ssh --ssh-target <目標> --remote-workdir <專用簽出>`，報告結構與狀態代碼維持不變，僅傳輸方式不同。安裝與驗證探測一律直接在**遠端機器上**執行，無法由控制節點推論。報告輸出至 `.dev/pipeline/<yyyymmdd>/test-executor-smoke-<HHMMSS>Z/ssh/`。`--remote-workdir` 為必填參數，且必須指向與控制節點 git HEAD 一致且狀態乾淨的 GAL 專用簽出。

| 遠端專屬狀態 | 意義 | 處理方式 |
| --- | --- | --- |
| `SSH_UNREACHABLE` | 無法建立非互動式 SSH 工作階段，此項目於任何執行器探測前優先檢查 | 檢查目標主機連線、網路與金鑰認證 |
| `REMOTE_GUARD_FAILED` | 遠端工作目錄不存在、不安全、不乾淨，或未對齊控制節點 HEAD | 將專用遠端簽出重新同步至控制節點 HEAD 並清理乾淨 |
| `REMOTE_FETCH_FAILED` | 遠端行程或已完成，但取回收據失敗 | 檢查該次執行的執行器日誌與遠端收據路徑 |

遠端 Copilot 依其遠端限制回報 `UNSUPPORTED`，此狀態不視為通過，亦不會被略過。`PASS` 嚴格要求終端完成**且**成功取回非空的收據。

若省略 `--strict` 參數，執行完畢一律以狀態碼 0 離開。報告本身才是唯一的真實依據，結束碼並非絕對標準。

## Pipeline 復原

### 從落地關卡失敗中復原

請先執行 `gal finalize-check`，並在採取任何行動前完整檢視每一行輸出。完整模式的檢查行依序為 `authoritative-command`、`naming-gate`、`sync-idempotency`、`finalize-mode`、`project-source-doc-existence`、`state-bound`、`contract-roster-parity`、`doc-link-resolution`，最後是 `working-tree-clean`，每一行都必須是 `pass`。儲存庫根目錄存在 `plugins/gal-core/` 時共九行，不存在時則為七行，因為 `contract-roster-parity` 與 `doc-link-resolution` 僅在該目錄存在時適用。這些檢核行不會因任務數量增加而重複出現。

`sync-idempotency` 驗證唯讀候選算繪在兩次計算之間是否維持確定性，並不將轉接器套用至磁碟。候選內容與磁碟內容的差異僅視為磁碟漂移回報，不致使此關卡判定失敗。

僅有 `authoritative-command` 行記載結束狀態或啟動錯誤。最後的 `working-tree-clean` 行記載結束狀態、未清理條目計數與狀態輸出大小。其餘每一行皆僅記載摘要。

衛生專用收據另外驗證七行：`project-source-doc-existence`、`state-bound`、`durable-layer-commit`、`finalize-review-shape`、`contract-roster-parity`、`doc-link-resolution` 與最後的 `working-tree-clean`。此收據確認寫入後的狀態維持乾淨，且 `### Finalize Review <date>` 表格的格式結構正確，所有檢查必須於計畫檔案刪除前全數完成。`NotRun` 或缺少佐證一律判定為失敗，不予通過。

關卡失敗須依層級分流處理：

- 權威指令失敗、工具故障或工作樹不乾淨時，須另開立補救計畫處理，切勿於落地流程中順帶修改。
- `### Finalize Review <date>` 表格若出現空白或無效儲存格，將導致衛生專用收據的 `finalize-review-shape` 判定失敗。請重新執行審查流程以產出格式合規的表格，再行複檢衛生專用收據。
- 工作流程狀態若非 `DONE` 且確實尚有未經檢查的工作，應返回 `/gal pipeline` 繼續推進。
- 工作流程狀態雖為 `DONE`，卻同時伴隨未檢查或互相矛盾的狀態，此屬終端損毀。請立即停止並寫出必要的人工作業交接筆記。

未通過的落地關卡絕不授權任何自動修復、提交或竄改佐證。

終端復原僅能在已提交之狀態確認乾淨後方可執行，固定步驟如下：首先執行 `gal.exe pipeline-preflight --terminal-reverify <execution-prompt-path>`。在 terminal-reverify 收據通過的前提下，ORCHESTRATOR 於同一行程內執行目標回推驗證。最後依序執行 `gal.exe pipeline-handback-check <execution-prompt-path>` 與 `gal.exe finalize-check <execution-prompt-path>`。

### 工作樹的執行環境交接

`stable PATH shim` 是由 `package-manager` 提供的入口。它會從命令啟動的目錄解析工作樹根目錄，並在信任執行環境狀態前，先交接給該根目錄對應的執行檔。根目錄是最近一個屬於 GAL 原始程式碼簽出的上層目錄。找不到時改用 git 頂層目錄，仍找不到時則使用啟動目錄。因此，從任何子目錄啟動的命令，都會與從根目錄啟動的命令使用相同的繫結。解析出的執行檔根目錄必須明確指定。`explicit-root` 是來源驗收執行唯一可寫入的位置。協調器繫結是受保護執行的唯一持久權威。繫結缺失或不可信時，操作會停止。繫結與收據以同一種正規化形式記錄路徑：移除 Windows 的 verbatim 前置字元 `\\?\`，並使用 `/` 作為分隔字元。

工作樹沒有有效繫結時，shim 會傳回包含 `execution_binding_sha256` 的交接資料。這是交接資料或收據會攜帶的摘要。協調器會先驗證交接資料與明確指定的執行檔根目錄，再對工作樹執行 `safe rebind`。若嘗試仍在執行、檢查點尚未使用，或投影交易尚未完成，`safe rebind` 會遭拒。重新繫結成功後會發布新的不可變世代，並保留先前世代及其證據。

接受閘門收據前，請確認收據摘要同時符合已儲存的檢查點與目前繫結。任一摘要不符時，請停止並先處理繫結。復原工作樹時，不要重新安裝 GAL，也不要刪除共用安裝狀態。

共用 shim 只會透過套件管理器的釋出版本變更。來源更新使用不可變世代與安全重新繫結，不必替換共用 shim。

只有透過繫結的執行檔執行 `doctor --verify-receipt` 並通過後，才能信任關卡收據。若收據摘要與已儲存的檢查點或目前繫結不符，請立即停止。此時不可信任收據，也不可重新執行關卡。

```text
stable shim -> explicit executable root -> coordinator binding
     |                                  |
     +-> digest handback -> validate ---+
                                   | valid
                                   v
                           safe rebind -> immutable generation
                                   | unsafe
                                   v
                               hard stop
gate receipt -> checkpoint digest + binding digest -> accept or stop
```

### 從收據租約失敗中復原

收據租約的用途是避免兩次 GAL 派送對同一個確定性收據路徑產生衝突。看到 `reason=receipt-preparation-failed` 或 `reason=remote-receipt-freshness-failed` 時，請將其視為可能有活躍中或殘留的租約，切勿當作可大範圍刪除收據的授權。

請由 stderr 檢視確切的本機鎖定檔案路徑 `.dev/pipeline/.locks/<hash>.lock`。遠端新鮮度檢查的 stderr 會附帶 `lock=.dev/pipeline/.locks/<hash>.lockdir`，其中的 `owner` 檔案才是清理的依據。刪除前，請務必先獨立確認無任何對應的 GAL 或 SSH 執行器仍在運作，尤其在剛發生逾時或等待 I/O 錯誤之後更須確認，因這兩種結果本身無法證明行程已終止。確認完畢後，僅移除該具名的鎖定檔案，或具名的遠端 `owner` 檔案與其清空後的鎖定目錄。**絕對不可刪除整個 `.locks/` 目錄。**

各失敗原因的意義如下：

| 原因標記 | 意義 |
| --- | --- |
| `remote-receipt-fetch-failed` | 執行後的來源不存在、內容為空、為符號連結、非一般檔案，或無法取回 |
| `remote-receipt-fetch-timeout` | 有界取回作業逾時 |
| `remote-receipt-fetch-read-failed` | 管道讀取錯誤導致僅存部分 stdout，GAL 拒絕信任該內容 |
| `remote-receipt-fetch-too-large` | 輔助程式已讀畢來源，但拒絕保留超過 1 MiB 上限之 stdout 內容，stderr 附帶相同上限值 |
| `receipt-lease-cleanup-failed` | 受檢的本機鎖定於正常或提前返回路徑移除失敗。請依 stderr 提供的確切路徑處理，並維持降級結果 |
| `remote-receipt-lease-cleanup-failed` 或 `-timeout` | owner 標記清理未獲確認，因此鎖定刻意維持 fail-closed（預設阻擋），即便執行器本身已失敗亦然 |

原因標記若以 `-unconfirmed` 結尾，代表本機輔助 SSH 行程未給出確認的結束證據，因此受上限保護的管道讀取將不予採納。當清理失敗伴隨主要防護或日誌失敗時，標記會以 `+` 將原因標記串聯，或於錯誤文字中同時指名兩者，兩邊皆須追查。執行器本身的結束碼 73 即為 73，除非出現包裝層專屬的新鮮度哨兵標記，否則不視為新鮮度失敗。

### 從階段回寫失敗中復原

對於範圍內的派送（任何提示檔上的 `audit`，以及未帶標記的提示檔上的 `test`），終端狀態 `completed`（結束碼 0）**僅代表收據已送達** `<task>-<phase>.receipt.md`，並不代表該階段已完成，亦不代表提示內容已被置入。提示置入由 `gal` 控制節點掌握，它於派送完成後執行一道 fail-closed（預設阻擋）的語意回寫關卡。

管道繫結的測試與稽核收據，第一行必須是 `### [T-NN] YYYY-MM-DD`。執行器不得在該標題前加入中繼資料或其他前言。控制節點遇到中繼資料優先的收據時，會拒絕收據且不修改執行提示檔。有效測試收據會置於 `## Test Results` 下，有效稽核收據會置於 `## Review Results` 下。

**可觀察之失敗徵象**：當酬載驗證或算繪失敗時，`gal` 會對 stderr 輸出 `phase-writeback semantic failure for task <T-NN>`，並於管道迴圈日誌附加一筆結構化錯誤紀錄，隨後以非零結束碼離開，同時確保磁碟上的執行提示檔維持逐位元組不變。此機制稱為**提示檔不變性保證**。

範圍與例外：

- 本機與 SSH 兩條執行路徑送回的收據酬載，皆通過相同的控制節點語意關卡，產生完全相同的確定性提示置入結果。
- `PipelineInput::RawSpec` 與直接叫用之 `gal dispatch` 會繞過語意回寫關卡，維持純收據形式，因這兩種模式不存在權威的執行提示檔。

常見成因與復原程序：

- **投影出的代理程式合約過期**：若執行器回傳格式錯誤的收據內容（例如多出未加圍籬的標題或缺少判定標記），或試圖直接編輯提示檔，請執行 `gal refresh` 將儲存庫本機與投影出的代理程式合約同步至當前規範。
- **收據酬載格式錯誤**：檢查收據檔案（`.dev/pipeline/<plan-slug>/...`）或管道迴圈日誌，找出具體的驗證錯誤，例如缺少 `### [T-NN] YYYY-MM-DD` 標頭、多出 H2 或 H3 標題，或任務 ID 不符。
- **修復後重新執行**：若合約過期，請先執行 `gal refresh` 解決收據或派送問題，再重新執行 `/gal pipeline`。操作者絕不可手動將 Markdown 子區段剪下搬移或複製貼上至執行提示檔，因確定性的排序是由二進位置入機制保證的，它能確保內容精確落於正確的 H2 區段（`## Test Results` 或 `## Review Results`）之下。

## 內部關卡與輔助子指令

下列 `gal` 子指令由工作流程內部叫用，未列於公開的 `/gal` 指令介面。日常操作無須手動執行，但在查閱關卡失敗訊息時會看見這些名稱。

| 子指令 | 階段 | 職責 |
| --- | --- | --- |
| `gal planning-check` | 規劃 | 規劃階段的結構收據 |
| `gal refining-check` | 細化 | 來源計畫的結構收據 |
| `gal prompt-check` | 產生提示 | 提示檔結構檢查，`--assemble-dry-run` 另外驗證任務規格能否組裝 |
| `gal planning-stamp` | 規劃 | 為計畫蓋上規劃權威戳記，`--equivalence <prompt>` 驗證非英文計畫與英文執行提示檔的語意等效 |
| `gal pipeline-preflight` | 管道入口 | 入口關卡。第一次實作編輯前必須回傳 `pass`。`--terminal-reverify` 用於終端復原 |
| `gal boundary-check` | 每個任務提交前 | 比對實際變更的檔案與任務的允許清單。允許清單以 `## Tasks` 中該任務項目以反引號標示的路徑為起點，讀取範圍到下一個任務項目或 `###` 標題為止。如果被點名的路徑是 `## Files to Create or Modify` 某一列的主要路徑，該列上所有以反引號標示的路徑都會納入。若該項目沒有路徑，則改用 `## Files to Create or Modify` 下的全部路徑 |
| `gal pipeline-converge-check` | 每個任務收斂 | 驗證該任務的持久回傳與收據是否一致 |
| `gal pipeline-handback-check` | 管道結束 | 驗證回交證據，決定是否可移交落地階段 |
| `gal pipeline-log append` | 全程 | 對管道迴圈日誌附加一筆結構化紀錄，參數為 `--task`、`--phase`、`--role`、`--kind`、`--level`、`--msg` 與 `--log-ptr` |
| `gal pipeline-clean` | 落地 | 移除 `.dev/pipeline/<plan-slug>/` |
| `gal state-merge` | 落地 | 以計畫為鍵逐列合併 `.dev/state.md` 的表格 |
| `gal finalize-check` | 落地 | 落地先決條件的零信任收據，分完整模式與衛生專用模式 |
| `gal dispatch-script` | 派送 | 算繪對話控制平面的派送區塊，包含 `OFFLOAD` 與預先算繪的 `REPORT_LINE` |
| `gal dispatch` | 派送 | 原始派送進入點。不解析出處，亦不走語意回寫關卡 |

維護者專用的子指令（`gal restore`、`gal release`、`gal release-notes`、`gal marketplace-snapshot`）請參閱 [CONTRIBUTING.zh-Hant.md](./CONTRIBUTING.zh-Hant.md#維護者專用子指令)。

## 其他生命週期操作

### 代理程式合約解析

派送由提示檔或來源計畫所建立的管道階段前，`gal` 會先尋找權威代理程式合約（`agents/golem-{implementer|tester|auditor}.agent.md`），讀取精確位元組內容並內嵌至任務規格中，隨規格一併派送給執行器。無論執行器是在本機或經由 SSH 通道執行，皆無須讀取僅控制節點有權存取的合約路徑，因規格內容本身已自成一體。

解析順序依下列四個層級進行，由第一個成功產生來源根目錄的層級勝出：

1. `workdir`：規範化後的 `--workdir` 本身，或其直接的 `plugins/gal-core` 子目錄。
2. `ancestor`：`workdir` 最近一層被認可為 GAL 來源根目錄的祖先目錄。
3. `exe-side`：執行中 `gal` 二進位檔同層目錄。
4. `embedded`：於 `~/.gal/embedded-src` 具體化之內嵌來源。

`workdir` 優先於其餘三層。這確保受信任的本機 GAL 簽出保有權威性：即便 `PATH` 上另有不同版本的封裝 `gal` 二進位檔，只要儲存庫包含 `plugins/gal-core/`，它仍會解析本身的合約，不受已安裝版本的影響。若您維護包含 GAL 原始程式碼的簽出，`Dispatch:` 標記中的 `contract_source` 欄位即為檢查版本偏差之處。

復原方式依失敗樣態而定：

- **勝出根目錄損毀**：已找到優先權最高的根目錄，但其選定的階段合約遺失、非 UTF-8 編碼或無法讀取。派送將在啟動執行器前停止，錯誤訊息將指明受影響的層級與根目錄。此情況絕不自動退回較低層級，請直接於指名的根目錄修復檔案，切勿依賴較低層級代為處理。
- **內嵌來源具體化失敗**：若較高優先層級均未命中，且內嵌內容存在但無法暫存、擷取或以原子方式交換，GAL 會回報失敗的操作、路徑與底層 I/O 原因。這不同於內嵌內容不存在。管道派送、`gal init`、`gal refresh` 與 `gal render-adapters` 會在既有的來源解析邊界保留此錯誤。請檢查指定路徑並修復 GAL 主目錄，或重新安裝 GAL。
- **全數未命中**：四個層級皆未產出可用的根目錄。派送將以狀態碼 1 終止，列出各層級結果並提供兩條復原途徑：經由封裝通道重新安裝 `gal`，或改由 GAL 來源簽出執行指令。

原始 `gal dispatch` 與原始任務規格形式之 `gal pipeline` 輸入屬於例外，兩者不解析亦不自創出處資訊。其標記與執行器日誌標頭維持與舊版出處格式之位元組相容性，因此這兩條路徑本即不會出現 `contract=` 或 `contract_source=` 欄位。

### 暫停對比落地

`/gal wrap-up` 的作用在於暫停，而非落地結案。它將工作階段的交接筆記壓縮寫入執行提示檔，更新 `.dev/state.md` 中的工作階段延續資訊，隨後建立一次提交。這讓任何執行環境皆能由暫停點精確接續。此指令不會結束任何計畫。已完成的計畫應使用 `/gal finalize` 結案，進行中的計畫才使用 `/gal wrap-up`。

## Git 輔助指令

### `gal commit-msg`

`gal commit-msg` 為確定性的提交輔助工具，用於支援 `git-commits` 技能與 `git-commit-msg` 指令。其類型與範圍分類完全依據更動的檔案路徑與 git 狀態推導，刻意忽略 diff 差異內容與本文關鍵字，避免訊息文字影響分類結果。此指令具備三種模式：

- `gal commit-msg --context` 輸出已暫存變更的精簡脈絡，包含檔案清單、確定性推導出的類型與範圍基準標題、已暫存的計畫與提示摘要，以及供訊息草擬代理程式參考的 hunk 標頭。此模式以較低的權杖成本提供較高的訊號品質。
- `gal commit-msg --print` 僅輸出確定性推導出的類型與範圍主旨標題。
- `gal commit-msg <file>` 作為 git 的 commit-msg hook 運作。它依據已暫存變更填入空白訊息，絕不覆寫任何既有內容。

`git-commit-msg` 指令僅產生提交訊息文字，不執行 `git commit`。`git-commits` 技能則在產生訊息後，於偵測到明確的提交意圖時才正式執行提交。

### Git 篩選 (`gal clean` 和 `gal smudge`)

選用的 `gal-config` git 篩選器可將機器本機設定值由受追蹤檔案中抽離。於個別儲存庫註冊的方式如下：

```bash
git config filter.gal-config.clean "gal clean"
git config filter.gal-config.smudge "gal smudge"
```

此篩選器由 git 執行檔於內部叫用。`gal` 二進位檔必須存在於 git 當下使用的 `PATH` 中，否則篩選器錯誤將導致 `git commit` 失敗。

## 撰寫輔助工具

### `text-flowcharts`

[`text-flowcharts`](../../../plugins/gal-core/skills/text-flowcharts/SKILL.md) 技能將分支邏輯、管道與多步驟流程呈現為等寬文字決策樹圖表。在 Claude Code 中以 `/text-flowcharts` 叫用。當說明涉及逐筆紀錄控制流程，或要求繪製 flowchart、流程圖或邏輯圖時，該技能亦會自動啟動。

每張圖表引導單一紀錄由上方入口出發，依序通過其滿足的條件並抵達分級終端結果，使讀者能藉由代入單一項目觀察其流向。圖表符號集刻意保持精簡：方框容納處理步驟，括號標示決策條件，末端節點則標註終端狀態標記。

| 元素 | 字元 |
| --- | --- |
| 流線與轉角 | `│ ─ ┌ ┐ └ ┘` |
| 交點（分支、匯流、交叉） | `├ ┤ ┬ ┴ ┼` |
| 箭頭（下、上、右、左） | `▼ ▲ ▶ ◀` |
| 終端成功 | `√` |
| 刻意跳過 | `>>\|` |
| 死胡同或遭拒 | `×` |

這些字元可在預設等寬字型中正常顯示，無須額外安裝字型。在非 CJK 環境下各佔單一字元寬度，在 UTF-8 環境下的 Pull Request 留言、程式碼審查留言與終端機中皆能原樣保留。輸出不包含表情符號 (Emoji)，在舊型終端機上依然具備良好可讀性。請僅在流程確實存在分支時使用該技能，若為無決策分支的線性序列，使用編號清單會更易閱讀。

## 訊息與文件的文字品質

作者須將尚未送出草稿中的每項重要事實陳述，與可取得的具體依據逐一比對。沒有依據的說法須移除或加上適當限定；沒有完成證據，不代表工作從未開始。使用者要求只提供指定產出時，必須遵守；但主機中優先順序較高的指令另有要求時除外。這是目前代理程式的語意審查義務，不代表已有自動事實查核或輸出攔截機制。

送出所有面向人的訊息前，作者都須在撰寫過程中檢查草稿，確認準確、易讀且意思完整。進度更新與專責代理程式的報告也適用。作者須依實際收件者的背景撰寫，並區分請求、提供材料及實際工具觀察中的事實，以及推論、假設、建議、占位內容與未來承諾。不能只因事實來自工具而非請求，就認定它是捏造。

已設定的檢查工具可用且語言設定檔適用時，作者須檢查尚未送出的草稿。探索到指引、載入指引、執行工具及攔截輸出，是不同的觀察結果。檔案檢查不能檢驗已串流送出的對話。檢查沒有回報問題時，只能確認已執行的確定性規則未發現問題，不能證明事實正確或文字易讀。

若問題需要修正，最多嘗試兩次。每次修正都須重新檢查，並保留事實及受保護內容。改動語意的修正必須捨棄並復原。只有建議性問題時，互動訊息可以使用最後一份語意有效的草稿，並視需要揭露未解問題。選用工具無法使用時，作者仍須自我檢查，並回報一次限制，直到可用狀態或對交付的影響改變。不遞迴檢查檢查器自己的狀態訊息，也不為每則訊息派遣 auditor。

若要取得完整報告，請在 wrapper 的 `--files` 之前指定 `--transport cli` 或 `--transport mcp`，也可將傳輸入口與 `--required` 合用。預設入口為 CLI。官方 MCP 入口會保留相同格式的問題及執行中繼資料。只有上游診斷輸出，仍不足以構成完整 GAL 報告。MCP 結構驗證、逾時或清理失敗等作業問題，會以結束碼 `2` 結束。

必要的文件關卡遇到硬性問題或作業失敗時必須停止。工具無法使用、當機或逾時都屬作業失敗。除非另有明確的必要條件，單有建議性問題不會阻擋關卡。作者須回報阻擋位置及處理方式。工作流程要求 `PROSE_AUDIT` 時，這項獨立語意審查仍然必要，textlint 不能取代它。貢獻者指令及報告說明見 [`CONTRIBUTING.md`](../../../CONTRIBUTING.md#writing-quality-checks) 與 [`tools/writing/README.md`](../../../tools/writing/README.md)。
