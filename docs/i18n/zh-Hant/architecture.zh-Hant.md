---
source: docs/architecture.md
lang: zh-Hant
source_commit: PENDING
translated_at: 2026-10-04
type: Architecture
title: GAL 架構
description: GAL 的常設約束、crate 與分層邊界、儲存拓樸、遷移路徑、文件治理與 ADR 索引。
tags:
  - architecture
  - boundaries
  - crates
  - governance
  - arc42
status: stable
---

# 架構

## Technical Baselines

GAL 是一套以 Rust 撰寫的跨執行環境 AI 工作系統，在不同 AI 供應商之間維持統一的控制平面與工作流程合約。系統的技術基底由原始碼基準、部署基準與常設約束共同構成。

### 原始碼基準 (Source Baseline)

- **Rust 工作區**：以 Cargo 工作區組織，包含 7 個 crates（`gal-foundation`、`mcp`、`dispatch`、`pipeline`、`projection`、`gal-engine`、`cli`），最終編譯為單一二進位執行檔 `gal`。
- **無外部資料庫 (No DB)**：執行環境不相依任何關聯式或非關聯式資料庫。所有狀態皆以儲存庫內與本機檔案系統上的純文字檔案承載。
- **純文字來源合約**：所有工作流程、指令規格、代理程式角色定義與範本皆為版本控管的 Markdown 檔案，集中收錄於 `plugins/gal-core/` 之下。
- **正式語言基準**：依據 `PROJECT_LANGUAGE`，正式合約與核心文件的語意來源為英文 (`en`)。非英文規劃語言透過確定性的雜湊計算與等效性標記機制，確保語意與執行提示檔完全對齊。

### 部署基準 (Deployment Baseline)

- **六目標平台預先建置產出物**：GitHub Actions 釋出管道為 6 個平台提供預先建置二進位檔，包括 `x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`、`aarch64-unknown-linux-gnu`、`x86_64-apple-darwin`、`aarch64-apple-darwin` 以及 `x86_64-pc-windows-msvc`。
- **供應鏈完整性驗證**：釋出資產包含 SHA-256 `checksums.txt` 與 keyless cosign 簽章。安裝流程在解壓縮前驗證雜湊與簽章。
- **套件管理員驅動安裝**：終端使用者經由官方安裝指令碼（`install.sh` 與 `install.ps1`）、Homebrew Tap、WinGet 或 `cargo install --git` 安裝二進位檔。原有的內部安裝指令（如 `crates/setup`）已全數退役。
- **Windows 靜態 CRT**：Windows 平台釋出的二進位檔依合約強制採用靜態 C 執行環境（static-CRT），確保未安裝 Visual C++ 可轉散發套件的全新機器亦可直接執行。

### 常設約束 (Standing Constraints)

- **跨執行環境行為一致**：在 GitHub Copilot、Google Antigravity、OpenAI Codex、opencode 與 Claude Code 五大執行環境之間保持相同的控制平面指令語意，嚴防跨工具指令漂移。
- **儲存庫自有檔案為長期狀態**：儲存庫擁有的 `.dev/` 目錄、計畫檔案與研究產出物是跨工具延續工作階段的唯一真相來源，暫時性執行環境快取不得取代這些檔案。
- **產生的轉接器來自受控範本**：儲存庫本機的轉接器檔案必須由 `gal` 二進位檔根據受版本控管的來源合約產生，嚴禁手動編輯本機產出物。
- **純粹的 Git 歷史管理**：僅允許標準 `git merge`、`git commit` 與 `git branch`。嚴格禁止提議或執行 squash、rebase、`--ff-only` 或 `--no-ff`。並行開發工作一律透過獨立的 Git 工作樹隔離，不重寫歷史。
- **工作範圍不擅自縮減**：一旦專案擁有人確立了工作目標與範圍，任何代理程式不得主動提議縮小範圍，亦不得擅自拆分為未經核准的後續階段或設立保留清單。
- **嚴格的檔案與權杖預算**：專案索引檔 `.dev/project.md` 限制在 30,720 位元組以內，工作階段狀態檔 `.dev/state.md` 限制在 16,384 位元組以內。轉接器根目錄檔案中的具名工作流程服從標記 (Named Workflow Obedience marker) 與關鍵執行環境套件標記 (critical-pack markers) 必須位於位元組偏移量 32,768（Codex 的 `project_doc_max_bytes` 預設值）之前，以確保專案文件在部分讀取時仍能觸及合約約束。

## Directory Boundaries

系統實作嚴格的目錄隔離與單向相依性法則，確保架構清晰且杜絕循環相依。

### 相依性法則 (Dependency Law)

Crate 之間的相依性呈現嚴格的有向無環圖 (DAG)。相依關係僅能由上往下，禁止同層循環或向上引用。

```text
gal-foundation  (基礎層 — 無任何 gal 內部相依)
dispatch        (僅相依 gal-foundation)

dispatch   ─> gal-foundation
mcp        ─> gal-foundation
projection ─> gal-foundation
pipeline   ─> dispatch
gal-engine ─> gal-foundation, projection
cli        ─> gal-foundation, mcp, projection, dispatch, pipeline, gal-engine (gal 二進位檔 — 聚合全部)
```

### Crate 責任負面表列矩陣

| Crate | 單一職責 (Responsible for) | 禁止事項 (Must NOT) |
| --- | --- | --- |
| `gal-foundation` | 基礎設施：設定資料結構、路徑解析、平台抽象、JSON 與環境變數公用程式、執行環境登錄檔（`VALID_RUNTIMES`）、共用 MCP 型別、`HealthCheck` trait、算繪基本型別與 `secret_re`。 | 不得相依任何其他 gal crate，不得包含具體業務流程或 CLI 指令實作。 |
| `mcp` | MCP 產出物領域：專門處理 GAL 自身標準 `.mcp.json` 的 `HealthCheck`。 | 絕不寫入、修改或檢查任何主機現行的 MCP 設定檔（例如 `claude_desktop_config.json` 等）。 |
| `dispatch` | 無頭執行器呼叫：角色路由（`executorRouting`）、行程啟動與逾時管理、回寫驗證、本機與 SSH 遠端派送通道。 | 不得處理工作流程的任務拆分與多階段協調（此為 `pipeline` 職責），不得相依 gal-foundation 以外的其他 gal crate。 |
| `pipeline` | 本機工作流程協調：任務拆分、多階段派送協調、組合 `dispatch` 執行、組裝工作規格（task spec）。 | 不得直接呼叫底層行程啟動 API，不得直接將檔案投影至外部執行環境。 |
| `projection` | 檔案投影後端：將技能、指令、指示與代理程式角色投影至磁碟介面（經由接合點、符號連結或實體複本）。 | 不得介入儲存庫層級的轉接器根目錄產生（`AGENTS.md` 專由 CLI 的 `render.rs` 處理），不得管理無頭執行行程。 |
| `gal-engine` | 工作流程 CLI 基礎元件：標準外掛根目錄算繪（`plugins/gal-core/` 算繪至 `~/.gal/plugins/gal/`）、CLI 型別、工作流程 `doctor`、`git_filter` 登錄與翻譯檢查公用函式。 | 不得直接投影檔案至各代理程式的外部路徑，不得呼叫 `dispatch` 或 `pipeline`。 |
| `cli` | 二進位檔進入點：子指令派送、`gal doctor` 檢查聚合、儲存庫層級轉接器產生（`render.rs`）與內部檢查指令（`*-check` 家族）。 | 不得在未經預先檢查 (preflight) 的情況下寫入檔案，不得繞過下層 crate 的邊界直接操作內部細節。 |

工作流程接續（`from <task>`）與可執行的階段及任務引數屬於不同表示方式。格式錯誤的接續請求會在啟動行程前失敗，不會代換為預設任務。無論外層 Codex 工作階段使用哪種核准模式，Codex 子行程一律以 `--dangerously-bypass-approvals-and-sandbox` 啟動，在 Codex 沙盒外執行。原因是 Codex 的 Windows 沙盒目前會在執行任何指令之前就失敗。可用性、啟動、供應商回應、最終結果、收據與關卡證據各為不同階段。Codex 主機設定與復原步驟請參閱[安裝與初始化](./setup.zh-Hant.md#codex-管道執行設定)。工具讓出控制權後，原始工作階段的等待方式依 `gal-pipeline` 技能中 `Headless Executor Dispatch` 的規定處理。

### 儲存庫分層結構與保護路徑

```text
Golem-Agents-Legion/
├── plugins/
│   └── gal-core/                標準來源合約根目錄
│       ├── commands/            公開 /gal 指令介面來源合約 [受保護]
│       ├── conventions/         所有 golem 遵循的通用規則合約 [受保護]
│       ├── workflows/           工作流程合約 (coding, doc-sync, research) [受保護]
│       ├── templates/           儲存庫持久化狀態範本 [受保護]
│       ├── agents/              golem 代理程式合約 (*.agent.md)
│       ├── skills/              技能本體 (skills/<name>/SKILL.md)
│       ├── mcp.json             受版本控管的 MCP 清單
│       └── opencode.json        隨附之 opencode 執行環境設定
├── crates/                      Rust 工作區與執行階段核心 (7 個 crates) [受保護*]
├── packaging/                   安裝指令碼 (install.sh, install.ps1) 與鏡像匯出指令碼 (publish-public.sh)
├── docs/                        專案說明文件、主題文件與架構決策紀錄 (docs/adr/)
└── .dev/                        儲存庫工作狀態 (project.md, state.md, plans/, research/)
```

\* 受保護路徑包括 `crates/projection/`、`crates/gal-engine/src/render/` 以及 `plugins/gal-core/` 下的工作流程合約。修改任何受保護路徑皆屬於重大變更，實作前必須具備經架構師審查認可的計畫。

### 程式碼庫所有權矩陣

| 資訊類型 | 正確位置 | 負責元件與來源檔案 |
| --- | --- | --- |
| 長期方法論與合約 | `plugins/gal-core/` 追蹤之原始碼與文件 | 來源合約：`commands/`、`conventions/`、`workflows/`、`templates/`、`agents/` |
| 儲存庫工作脈絡 | 目標專案之 `.dev/project.md` 與 `.dev/state.md` | 目標儲存庫本機狀態，由 `gal init` 與各工作流程指令維護 |
| 人類可讀的功能計畫 | `.dev/plans/<slug>.md` | 規劃流程產出物，由規劃命令群與架構師共同審查維護 |
| 機器執行的工作檔案 | `.dev/plans/<slug>.prompt.md` | 執行提示檔，由 `/plan-to-prompt` 產出，為管道執行的可變記憶體 |
| 暫時性工作階段延續 | 執行提示檔之 `### Handoff Notes` 與 `.dev/state.md` | 暫時性交接筆記，於工作流程完成時由協調器收錄回持久化文件 |
| 機器本機遠端執行路由 | `~/.gal/config/config.json#executorRouting` | 使用者擁有的本機設定，包含 `sshTarget` 與 `remoteWorkdir`，僅適用於 `pipeline` 與 `research` 的無頭派送 |

### 工作樹本機執行環境

`source-worktree` 會使用正規工作樹中的不可變執行檔驗證。`downstream-installed` 儲存庫會使用套件管理器安裝的執行檔。只有套件管理器可以安裝或升級共用執行檔。`CoordinatorState.execution_binding` 是唯一長期執行綁定依據。可信收據會記錄這項依據的摘要。

```text
[ 開始 ]
   |
   v
{ 選擇執行路徑 }
   | source-worktree                  | downstream-installed
   v                                  v
[ target/gal-pipeline/ 中的           [ 套件管理器管理的
  私有執行檔 ]                         已安裝執行檔 ]
   |                                  |
   +------------------+---------------+
                      v
[ 驗證 CoordinatorState.execution_binding、
  執行檔路徑、雜湊與收據摘要 ]
   | 缺少或不符                       | 驗證通過
   v                                  v
{ 強制停止 }                      [ 執行可信操作 ]
                                      | 在安全區段需要重建
                                      v
                                [ 綁定至新的不可變版本，
                                  保留先前的執行檔內容 ]
                                      | 否則
                                      v
                                [ 使用已綁定的執行檔繼續 ]
```

### 管道嘗試所有權

管道會透過提示檔寫入租約，序列化對標準提示檔的寫入。管道先取得此租約，再建立不可變的提示檔快照並準備可變階段狀態。接著，管道會取得收據租約。只有在放置收據、擷取終端證據並確認行程清理完成後，才會依取得順序的反向順序釋放兩種租約。若無法確認行程已清理，GAL 會保留所有作用中的租約與復原產物。

管道嘗試會在收據租約期間擷取並驗證收據位元組及其摘要。階段回寫會使用這份不可變快照。最後的關卡檢查會確認磁碟上的收據與快照相符。GAL 絕不依鎖定時間推定所有權，也不會自動接管遭棄置的租約。

工作流程接續（`from <task>`）與可執行的階段或任務引數是不同介面。格式錯誤或無效的接續輸入會在啟動行程前失敗，且不會改為執行預設任務。Codex 子行程一律以 `--dangerously-bypass-approvals-and-sandbox` 啟動，在 Codex 沙盒外執行，不受父工作階段核准模式影響。原因是 Codex 的 Windows 沙盒目前會在執行任何指令之前就失敗。可用性查詢、行程啟動、供應商回應、終端執行狀態、收據驗證與關卡結果，皆分別追蹤為不同證據階段。Codex 使用者設定與復原流程請參閱[安裝與初始化](./setup.zh-Hant.md#codex-管道執行設定)。工具讓出控制權後，原始工作階段的等待方式依 `gal-pipeline` 技能中 `Headless Executor Dispatch` 的規定處理。

#### 受防護的協調器所有權

`legacy_interactive` 是預設的接續設定檔。它保留 Claude Code 與 OpenCode 既有由主機管理的監看器及接續迴圈、呼叫語法、行程結束行為、投影配置，以及 `EvidenceContract: v1` 收據、標記與變更語意。一般的 `gal pipeline <prompt>` 指令不會選擇受防護模式。

受信任的 Codex hook 交握流程必須先提出授權，而且該授權必須有效並只能使用一次。符合這項條件後，系統才會選用 `codex_stop_v1`。在此設定檔中，每次 `gal pipeline` 呼叫只執行一個會阻塞的機械階段。Rust 協調器負責任務與階段轉換、嘗試生命週期、重試狀態、具型別動作、證據檢查、日誌復原及有界狀態。階段遇到下列任一狀態就會返回：具型別的 ORCHESTRATOR 檢查點、最終結果、需要人員授權的阻礙，或無法復原的證據失敗。

協調器不做語意判斷。同一個 ORCHESTRATOR 工作階段負責任務品質審查、擴大邊界的判斷、收斂判斷及目標回溯驗證。協調器會持久化 `awaiting-orchestrator` 檢查點。ORCHESTRATOR 提交的收據必須繫結該檢查點、協調器修訂版、提示檔或規格雜湊及 commit，然後呼叫具型別的接續動作。後續會阻塞的 CLI 階段會重新開啟持久化狀態。GAL 不使用 daemon、遞迴 shell 呼叫、隱藏的監督程式，也不在程序內等待模型判斷。

進入受防護流程後，協調器狀態會保存在既有的 `.dev/pipeline/<plan-scope>/` 權威位置。單檔原子寫入會綁定提示檔與儲存庫、目前任務與階段、作用中的嘗試、重試狀態、最近一次通過驗證的證據、待處理的投影交易、ORCHESTRATOR 檢查點、啟用摘要與下一步動作。協調器只負責機制。語意決策仍由 ORCHESTRATOR 負責。若 Stop hook 阻止過早回覆，主機橋接層會在同一個工作階段中繼續執行。

#### 行程身分與派送證據

每次 v2 派送都會在啟動行程前儲存其意圖。嘗試身分會綁定主機、PID、作業系統行程建立身分、嘗試 ID、提示檔與儲存庫、基礎 commit 及預期階段。GAL 只會等待或附加至身分完全相符的嘗試，絕不只依 PID 推定所有權。若所有權不明或與記錄的身分衝突，GAL 不會接管租約、終止行程或重新派送工作。

`EvidenceContract: v2` 會增加計畫、任務、階段、嘗試、收據摘要、供應商工作階段、提示檔或規格摘要，以及受測 commit 或差異的綁定。在下游關卡判定嘗試前，成功的供應商工作階段身分與來源資訊必須同時符合派送結果、終端嘗試記錄及 v2 證據。v2 Agy 路徑會從該次嘗試所屬行程輸出或 SSH 標準輸出的有界 JSON 輸出中，讀取原生工作階段 UUID。輸出若缺漏、格式錯誤、重複、不相符、遭截斷或超出大小限制，皆屬於具型別的證據失敗。接續派送必須回傳指定的工作階段 UUID。

二進位檔關卡會根據綁定至同一嘗試與受測變更的證據，評估 v2 三條件 PASS 規則。較舊的 `completed` 證據不能驗證較新的程式碼。缺少或含糊的 v2 證據不會消耗語意 TEST 或 AUDIT 重試次數，也不會觸發自動重新派送。這些新增機制不會改變 v1 收據、結束代碼、公開的 `Dispatch:` 標記、CLI 或變更順序合約。

#### 持久化狀態與當機復原

受防護階段啟動供應商、變更任務游標或更新進度投影前，會先復原任何待處理的投影日誌，再依復原後的綁定執行全新的管道預檢。接續執行會使用該階段專屬的新預檢收據。若收據遺失、過期、失敗、格式錯誤或無法寫入，階段便會在啟動供應商或變更權限狀態前停止。若原協調器仍在執行，系統會以唯讀方式附加。若協調器已成為孤立狀態，只有在啟用摘要與嘗試身分相符時，才會依已驗證的持久化狀態繼續。所有權不明或身分衝突時，系統會以具型別的衝突停止。

提示檔、來源計畫與 `.dev/state.md` 之間的任務進度，使用持久化投影日誌。日誌會記錄預定交易、暫存的目標位元組，以及新舊雜湊。只有目標仍符合記錄的新舊雜湊之一時，復原程序才會套用替換內容。雜湊足以確認所有權時，復原程序會以冪等方式完成中斷的交易。若目標具有第三方雜湊，復原程序會停止並保留該內容。只有日誌標記投影已提交後，協調器才會推進持久化狀態。

```text
[ 受防護階段開啟協調器 ]
  ├── 無狀態 ──▶ [ 啟用前持久化狀態與派送意圖 ]
  └── 已有狀態
       ├── 精確相符的執行中擁有者 ──▶ [ 附加並等待，不重新派送 ]
       ├── 身分相符的孤立協調器 ──▶ [ 復原日誌，再執行全新預檢 ]
       └── 所有權不明或身分衝突 ──▶ （具型別的衝突，停止）
                              │
                              ▼
                    { 日誌目標雜湊？ }
                      ├── 舊雜湊 ──▶ [ 套用暫存替換內容 ]
                      ├── 新雜湊 ──▶ [ 記錄替換已完成 ]
                      └── 第三方雜湊 ──▶ （保留目標，衝突，停止）
                              │
                              ▼
                    [ 提交日誌，再推進協調器狀態 ]
```

`legacy_interactive` 保留原有由主機管理的復原與接續行為。它不需要 Codex 標記、canary、hook、授權或由協調器管理的接續流程。

## Build & Data Pipeline

GAL 的資料管道負責將版本控管的來源合約轉化為各 AI 工具可直接讀取的投影表面與本機轉接器。

### 所有權順序

架構文件僅界定所有權順序：來源合約分別進入儲存庫本機產生器與標準根目錄算繪器，再由投影後端把標準結果交付給各執行環境。[`projection.zh-Hant.md`](./projection.zh-Hant.md) 獨立規範完整目錄樹、執行環境路徑、傳輸機制、投影登錄檔與重新產生程序。[`configuration.zh-Hant.md`](./configuration.zh-Hant.md) 則規範機器本機設定邊界內由使用者管理的內容。

## Component & Module Model

GAL 的內部元件劃分恪守單一職責、嚴格命名與所有權隔離原則。

### 單一概念單一命名法則 (One-Concept-One-Place)

命名規則嚴格依循 [`docs/glossary.md`](../../glossary.md)。禁止使用含糊不清或過載的通用命名：

- 使用 `projection` 代表檔案投影後端，取代過載的 `adapters`。
- 外部執行環境提供者邏輯完全收納於 `projection` 模組內部。
- 使用 `executors` 代表無頭執行器抽象，取代原有的 `dispatch::adapters`。
- 退役的模組必須徹底自程式碼庫中刪除，不可留存無呼叫者的殘留檔案（例如已完全移除的 `crates/projection/src/agy.rs` 之 `AgyProjection`，以及已退役的 `crates/xmachine`）。
- 正式檔案、註解與合約中嚴格禁止出現計畫任務 ID（如 `T-NN`、`R-NN`）或過渡時期的遷移敘述，以維持長久文件的乾淨性。

### 算繪與投影所有權分離 (Render/Projection Ownership Split)

系統將「產生儲存庫層級轉接器」、「算繪機器層級標準外掛」與「投影執行環境介面」劃分為三個完全獨立的負責模組，生命週期互不干擾：

- **產生儲存庫轉接器**：唯一負責模組為 `crates/cli/src/gal/render.rs`。負責自專案的 `.dev/project.md` 擷取 8 個必要章節，並算繪出儲存庫根目錄的 `AGENTS.md`。`gal-engine` 與 `projection` 絕不介入此流程。
- **算繪標準外掛根目錄**：唯一負責模組為 `crates/gal-engine/src/render/`。負責將 `plugins/gal-core/` 結合本機可選個人層算繪至 `~/.gal/plugins/gal/`。
- **投影執行環境介面**：唯一負責模組為 `crates/projection/`。負責將標準根目錄產出物依各 AI 工具的原生規格投影至目標路徑（例如將技能寫入 `~/.agents/skills/`，將 Codex 代理程式角色寫入 `~/.codex/agents/`）。

### Golem 代理程式兩類呼叫結構規則

系統中的所有 golem 代理程式依據呼叫模式分為兩個截然不同的結構類別：

- **第一類：直接與討論角色 (Direct & Consult Roles)**
  - 包含 6 個角色：`golem-architect`、`golem-analyst`、`golem-designer`、`golem-releaser`、`golem-debugger`、`golem-steward`。
  - 這些角色支援獨立呼叫 `/gal <role>`，在沙盒中執行評估並產出結構化結論。
  - 其中四個角色（`architect`、`analyst`、`designer`、`releaser`）另有同名的獨立指令。其 `discuss` 引數會透過內部的 `gal consult-script golem-<role>` 啟動對話內諮詢：Claude Code 使用 `/<role> discuss`，外掛模式使用 `/gal:<role> discuss`，Codex 使用 `$<role> discuss`。已移除的 `/gal discuss <role>` 會回傳 `COMMAND: error` 並附上替代指令。debugger 沒有獨立指令，steward 的獨立指令不支援 `discuss`。
  - 四個審查角色的隔離直接呼叫會輸出 `ROUTING_SCOPE: codex-native-projection-only`。`planning` 設定只供應 Codex 原生代理程式投影，不會啟動執行器，SSH 欄位也不適用於它。
- **第二類：僅限協調器驅動角色 (Orchestrated-only Roles)**
  - 包含 4 個角色：`golem-implementer`、`golem-tester`、`golem-auditor`、`golem-researcher`。
  - **投影層結構限制**：投影後端（`crates/projection/src/lib.rs` 的 `update_agents`）在產生原生代理程式設定時，**嚴禁為這 4 個角色產生任何投影檔案**（不寫出 `*.agent.md` 或 `*.toml`）。
  - **fail-closed（預設阻擋）**：若直接呼叫（例如 `/gal tester` 或 `$tester`），系統一律回傳未知意圖錯誤。這 4 個角色只能由 `/gal pipeline` 等協調器以無頭子行程方式派送，避免未經協調器引導的脫軌執行。

## Integration & Adapter Boundaries

GAL 藉由明確邊界整合 GitHub Copilot、Google Antigravity、OpenAI Codex、opencode 與 Claude Code。架構文件僅界定來源合約、標準根目錄算繪、執行環境投影與儲存庫本機產生器之間的分工。實際目錄拓樸、傳輸方式、所有權預檢、各執行環境介面與 MCP 投影邊界，由 [`projection.zh-Hant.md`](./projection.zh-Hant.md) 獨立規範。機器本機的路由與設定請參閱 [`configuration.zh-Hant.md`](./configuration.zh-Hant.md)，選用外部工具請參閱 [`integrations.zh-Hant.md`](./integrations.zh-Hant.md)。

## Escalation & Migration Path

GAL 具備嚴格的架構防護與向後相容機制，以因應未來的擴充與遷移需求。

### 遷移先例與相容性模式

- **Serde 別名維持向前相容**：當設定欄位或內部模型屬性更名時，採用 `#[serde(alias = "...")]` 接受舊有命名，確保升級後不破壞現存的 `config.json` 與 `plugins.lock.json`。
- **過期目錄盡力清理 (Best-Effort Stale-Dir Cleanup)**：清理過期投影或退役根目錄時，僅在目標路徑具備明確 GAL 所有權標記且已清空時進行刪除。若目錄內包含使用者自訂檔案，則予以保留並發出非阻擋警告。
- **刻意變更產出物時的黃金測試重擷取 (Golden Re-Capture)**：當刻意調整轉接器範本或合約輸出格式時，必須同步執行測試更新並重新擷取黃金輸出，同時於提交紀錄中詳述變更理由。

### 架構擴展晉級關卡 (Escalation Fence)

當任何實作工作涉及以下結構性變更時，必須立即停步並提請專案擁有人執行 `/deep-planning` 取得架構師審查核准：

- 建立或刪除專案根層級設定檔（如 `Cargo.toml`、`package.json` 等）。
- 新增或移除外部套件相依性。
- 將程式碼檔案跨架構層移動。
- 建立新的全域介面或抽象基礎型別。
- 修改公用 API 簽章或跨 2 個以上取用端共用的模組介面。
- 修改被 3 個以上取用端共用的核心基礎類別。
- 觸碰任何受保護路徑 (Protected Paths)。

### 持久化資產清單 (Durable-Asset List)

以下資產為儲存庫中的持久化知識核心，不得因工作流程交接或版本迭代而遺失：

- **來源合約**：`plugins/gal-core/` 下的指令、慣例、工作流程合約與範本。
- **命名權威與呈現對照檔**：核心術語權威 [`docs/glossary.md`](../../glossary.md) 與繁體中文呈現標準 [`docs/i18n/zh-Hant/terminology.zh-Hant.md`](./terminology.zh-Hant.md)。
- **主題文件與架構決策紀錄**：`docs/` 下的各項專題指引文件與 `docs/adr/` 下的所有架構決策紀錄。
- **專案結構索引**：`.dev/project.md`（高密度專案事實與真實來源指標）。

## 文件治理

### 永久知識庫的唯一定義

永久知識庫僅有兩處：儲存庫根目錄的三份入口文件（`README.md`、`SECURITY.md`、`CONTRIBUTING.md`），加上整個 `docs/` 目錄樹。兩處皆以 canonical 的英文路徑為準。`.dev/plans/`（計畫在生命週期結束後會刪除）與 `.dev/research/`（調查用的暫存區）皆不屬永久知識庫。調查確立的事實必須移至 `docs/` 或納入 `.dev/plans/` 方為有效。

`.dev/project.md` 是 `docs/` 的壓縮索引，並非知識庫本體。它彙整並交叉引用永久文件，本身不保有獨立事實。知識的流向固定為：計畫的 `## Status` 或 `## Handoff Notes` 先落地至 `docs/`（永久保存），再由 `.dev/project.md` 收斂成索引。一項事實若需保留至計畫生命週期結束後，必須納入上述三份根目錄入口文件之一，或收錄於 `docs/` 底下未被排除的檔案中。僅留存於 `.dev/plans/`、`.dev/research/` 或其他 `.dev/` 路徑者，皆不視為永久資產。

### 標點規則：不用分號

本專案的任何語言、任何文件皆不使用分號，包含全形與半形分號（`;` / `；`）。使用分號代表一句話同時塞入兩個完整的想法，遇到這種情況必須進行改寫。兩個想法各自獨立時請拆分為兩句，一個想法依附另一個時，改用逗號搭配明確的連接詞（and、but、so、because、while，或中文對應詞）。以分號隔開的並列項目清單，應改為逗號清單或條列清單。

這條規則的核心目的在提升可讀性，而非單純追求形式統一。分號容易隱蔽前後子句間的邏輯關係，迫使讀者自行推敲重建。此規則涵蓋 Markdown 內文、表格儲存格，以及 ASCII 圖表和程式碼註解裡的說明文字。程式碼語法本身所需的分號則不受此限。

### 寫作品質權責

目前的代理程式須在送出前檢查草稿，將每項重要事實陳述與可取得的具體依據逐一比對。沒有依據的說法須移除或加上適當限定。沒有完成證據，不代表工作從未開始。使用者要求只提供指定產出時，必須遵守；但主機中優先順序較高的指令另有要求時除外。

GAL 負責可攜的寫作要求、專案術語，以及本儲存庫選用的寫作檢查整合。`docs/glossary.md` 定義語意名稱與英文用詞。zh-Hant 與 ja 術語呈現檔定義各語言的用詞與標點。寫作慣例定義跨語言的篇章結構與規則權責。可執行術語資料只從這些權威文件的明確結構區塊產生，不從周邊文字推導規則。

儲存庫檢查工具在文件根目錄的 `.dev/cache/writing-terms/<sourceHash>.json` 發布不可變的術語快照。已有快照時，工具會驗證後重用，不覆寫其他寫入者的資料。子行程會以實際讀取的權威文件核對父行程提供的來源雜湊。直接 CLI 與官方 MCP 呼叫會解析自己的文件根目錄，並要求已有相符快照。資料缺失、損壞或過期時，檢查會以作業失敗結束。使用這些直接入口前，仍須先明確產生快照。

作者須依實際收件者的知識，提供對方需要的背景。事實可以來自請求、提供的材料或實際工具觀察。作者須區分這些事實與推論、假設、建議、占位內容及未來承諾。所有面向人的訊息，包含進度更新，都須在撰寫過程中自我檢查準確性與可讀性。已設定的檢查工具可用時，作者須檢查尚未送出的草稿。這些義務不代表主機已能攔截串流輸出。

GAL Core 提供中立的最低要求，並連結完整寫作慣例。它不複製或要求個人 `accurate-answer` 指引。個人指引須遵守專案術語及必要輸出結構。個人指引不存在或關閉時，最低要求仍然適用。

呼叫端依明確意圖、支援的 `lang` 中繼資料、已知受管路徑，依序解析語系。只有受管的 `docs/i18n/zh-Hant/` 文件，才將 `zh-Hant` 對應至 `zh-TW`。此對應不代表所有繁體中文文件都屬於臺灣中文。指令、程式碼、路徑、連結目標、前置中繼資料、機器錨點、識別碼、收據及逐字引文仍受保護。

wrapper 支援 `--transport cli` 與 `--transport mcp`，兩個入口都會產生相同格式的 GAL 問題與執行報告。MCP 入口使用官方伺服器。交握成功時，報告也會記錄伺服器宣告的名稱與版本。原生 textlint CLI／MCP 輸出仍是底層診斷格式，不是完整 GAL 報告。

來源慣例檔存在時，精簡根指引與任務指引會引用該檔案。在其他已初始化的儲存庫中，指引會改指向 `~/.gal/plugins/gal/rules/gal.md` 的 writing-quality.md 段落，並將 `~` 展開為使用者家目錄。這是既有的已安裝標準規則檔，不需要私人原始碼工作區或個人寫作技能。Claude 仍透過精簡的 `@AGENTS.md` 匯入載入指引。

### 檔案命名

- `README.md` 僅保留給儲存庫根目錄的那一份，儲存庫中其餘檔案皆不可命名為 `README.md`。子目錄若需入口或索引文件，請使用 `guide.md`（人工整理的導覽）或 `index.md`（自動產生或清單式的索引）。
- `docs/integrations.md` 裡的工具或功能小節，一律使用工具自身的正式名稱。僅專屬 MCP server 的小節方可加上 `MCP` 限定詞（例如「Playwright MCP」），工作流程或方法學文件不可加上工具字尾。
- 對應特定技能的文件，名稱應與技能名稱維持一致。例如 `opencli-research` 技能由 `docs/integrations.md` 中的 OpenCLI 小節支援。
- 任何兩份互不相關的文件皆不可共用相同的檔案基底名稱 (basename)。

### 多語翻譯政策

Canonical 文件一律使用英文，置於一般預設路徑。翻譯檔置於 `docs/i18n/<lang>/`，命名為 `<name>.<lang>.md`。根目錄文件亦遵循相同的 locale 目錄規範，例如 `README.md` 對應 `docs/i18n/zh-Hant/README.zh-Hant.md`。

翻譯集合依已釋出文件清單進行封閉管理。每個 locale 必須同時提供 `README`、`SECURITY`、`CONTRIBUTING`、`architecture`、`configuration`、`workflows`、`projection`、`integrations` 與 `setup` 的完整譯本。`docs/adr/` 只提供英文正文，不設翻譯樹。英文術語權威亦不進行全文翻譯，而是經由每個 locale 的 `terminology.<lang>.md` 呈現檔進行對照。

翻譯僅改寫自然語言敘述，指令、路徑、設定鍵、產品名稱與其餘機器字面值保持不變。相對連結必須依翻譯檔案所在目錄深度重新計算，不可直接照搬 canonical 文件的路徑。

### 兩種翻譯產物

`docs/i18n/<lang>/` 僅存在兩類產物：已釋出文件的完整譯本，以及一份 locale 術語呈現檔。完整譯本追蹤各自的 canonical 來源。術語呈現檔僅規範該語言如何呈現 canonical 英文術語，不具語意權威，亦不重複 glossary 的定義。

### 翻譯新鮮度

`docs/i18n/` 底下除 README 外的翻譯產物，皆需在下節的 Open Knowledge Format 鍵組外，額外加上四個翻譯溯源鍵，使譯文與英文來源間的落差維持透明可追蹤。翻譯版 README 僅保留這四個鍵，並放在標題後方不顯示的 `gal:translation-metadata` HTML 註解中：

```yaml
---
source: README.md          # canonical 來源的儲存庫相對路徑（術語呈現檔填術語權威的路徑）
lang: zh-Hant
source_commit: <hash>      # 這份產物同步到的來源 commit，補上真正的雜湊前先填 PENDING
translated_at: 2026-06-02  # 這份產物最後一次改寫的日期
---
```

新鮮度僅由 `source_commit` 承載，不由 `status` 承載。`status` 為下節定義的 Open Knowledge Format 生命週期鍵，僅接受 `draft`、`stable` 與 `deprecated` 三個值，翻譯產物不得將其改寫為 `current` 或 `stale`。

`gal translation-freshness` 會掃描受管譯本，比對 `source_commit` 與來源文件的最新 commit，並回報 `current`、`stale`、`missing` 或 `unexpected`。此指令不讀取 `status` 欄位。`PENDING` 一律視為尚未完成新鮮度簽核，不可偽稱已同步。

由於 GAL 的 commit 需經人工審查後方能提交，剛同步完成的產物會先填寫 `PENDING`，待維護者提交後再補上真實的來源 commit。檢查機制會將其回報為 `stale`，以確保不被遺漏。落後於英文來源的譯本，仍須通過儲存庫的無效連結檢查，該檢查會掃描所有受追蹤的 Markdown 檔案，不因新鮮度差異而給予豁免。

新增或重寫譯本時，先填寫 `source_commit: PENDING`。待內容與連結驗證無誤並提交 canonical 來源後，再行補上實際來源 commit。新增語言時必須一次補齊整套受管譯本與術語呈現檔，不提供部分支援等級。

### 前置中繼資料鍵組

公開 README 以外的每份受管文件開頭皆需包含 Open Knowledge Format 前置中繼資料，鍵組固定為五個：`type`（封閉詞彙表：`Guide`／`Reference`／`Architecture`／`ADR`／`Policy`）、`title`、`description`（單一完整句子，需與 README 文件地圖中的說明完全一致）、`tags`、`status`（`draft`／`stable`／`deprecated`，預設為 `stable`）。公開 README 的說明由各自的文件地圖維護。這五個是 GAL 唯一採納的 OKF 鍵，[open-knowledge-format(OKF)](https://github.com/GoogleCloudPlatform/open-knowledge-format/blob/main/SPEC.md) 規格中的信任與生命週期鍵（`verified`、`stale_after`、`generated`、`sources`）目前皆不採用。排除這四個鍵的實測依據與重新評估條件，記錄於 [ADR 06](../../adr/06-documentation-and-state-governance.md)。編寫者無須重新論證此決定，僅需遵循這五個鍵填寫。

`docs/i18n/<lang>/` 底下除 README 外的翻譯產物，在這五個 OKF 鍵外再加上前述四個翻譯溯源鍵（`source`、`lang`、`source_commit`、`translated_at`），合計共九個鍵。翻譯版 README 沒有 OKF 鍵組，只在隱藏註解中保留四個溯源鍵。這四個溯源鍵僅用於描述譯本與來源之間的對應關係，不擴充 OKF 鍵組本身定義。

## ADR Index

本文件依 ISO/IEC/IEEE 42010 與 arc42 架構描述標準編寫，本節為架構決策紀錄 (ADR) 索引。架構決策的完整脈絡獨立收錄於各 ADR 檔案中，本節僅保留索引清單，以確保架構主體文件維持清晰精簡，避免過多決策歷史細節膨脹內文篇幅。

一項主題僅在單一檔案中進行權威描述：本文件負責結構與邊界的現狀規範，ADR 負責決策背景與被否決方案的脈絡，各主題的具體操作規則由各權威檔案負責，進入點請參閱 README 文件地圖。

所有決策檔案以英文撰寫並存放於 `docs/adr/` 目錄下，不另設翻譯樹或獨立索引檔。以下連結文字採 zh-Hant 呈現，指向的 ADR 正文仍以英文為準：

- [01: 投影、來源模型與轉接器](../../adr/01-projection-and-source-model.md)
- [02: 範圍與所有權邊界](../../adr/02-scope-and-ownership-boundaries.md)
- [03: 機器生命週期與執行檔身分](../../adr/03-machine-lifecycle-and-binary-identity.md)
- [04: 派送架構](../../adr/04-dispatch-architecture.md)
- [05: 關卡、證據與強制機制](../../adr/05-gates-evidence-and-enforcement.md)
- [06: 文件與狀態治理](../../adr/06-documentation-and-state-governance.md)
- [07: 釋出](../../adr/07-release.md)
- [08: 工作樹本機執行環境隔離](../../adr/08-worktree-local-runtime-isolation.md)
