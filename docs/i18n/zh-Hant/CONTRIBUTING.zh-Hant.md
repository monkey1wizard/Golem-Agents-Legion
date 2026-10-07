---
source: CONTRIBUTING.md
lang: zh-Hant
source_commit: PENDING
translated_at: 2026-10-04
type: Policy
title: 貢獻 GAL
description: 提交 GAL 原始碼變更的入口，涵蓋 branch 與 commit 規則、建置與測試關卡、貢獻者檢查與選用的本機審查工具。
tags:
  - contributing
  - commits
  - gates
  - review
status: stable
---

# 貢獻 GAL (Contributing to GAL)

GAL 為 Rust workspace，也是由文件驅動的工作流程合約集合。修改前請先閱讀 [architecture.zh-Hant.md](./architecture.zh-Hant.md) 理解系統架構與各層邊界，再依本文件的關卡要求完成變更。具約束力的工作流程以 [`coding.md`](../../../plugins/gal-core/workflows/coding.md) 為準。

## Branch 與 Commit

- 請從 `main` 建立分支 (branch) 並發起 PR。依既有儲存庫慣例，單人規劃文件可以直接提交至 `main`。
- 使用帶有範圍 (scope) 的 Conventional Commits 格式，例如 `feat(gal-engine): ...`、`refactor(projection): ...` 或 `docs(...): ...`。
- 每個 commit 僅能包含一項可獨立還原的邏輯變更。
- Pre-commit hook 會執行命名關卡 (naming gate)、`.dev` 提交政策與個人路徑洩漏掃描。執行 hook 時必須確認可從 `PATH` 找到 `gal` 執行檔。

## 建置與測試

```text
cargo build --workspace --target-dir target/gal-pipeline/build
cargo test --workspace
cargo clippy --workspace
cargo fmt --check
gal naming-gate
gal render-adapters
```

所有關卡皆必須順利通過。再次執行 `gal render-adapters` 時不得產生任何差異 (diff)。

單元測試 (Unit tests) 置於來源檔底部的 `mod tests`。整合測試 (Integration tests) 則置於 `crates/<crate>/tests/`。`crates/gal-engine/tests/*.rs` 的檔名切勿包含 `install`、`setup`、`update` 或 `patch`，除非該 crate 隨附 `asInvoker` manifest，否則在 Windows 環境下可能遭遇 spawn error 740。

原始程式碼工作樹需要 GAL 執行檔時，請使用上方的私有目標目錄進行增量建置。驗證執行檔後，請將它發布到 `target/gal-pipeline/bin/<sha256>/` 下的新 `immutable` 世代。已發布的世代不得覆寫。請在隔離的 `fixture` 目錄中驗證套件管理器發行套件。只有 `package manager` 可以在共用安裝中安裝或升級釋出版本。原始程式碼工作樹不得安裝或取代共用執行檔或投影。如需交接與重新繫結規則，請參閱 [`self-bootstrap.md`](../../../plugins/gal-core/conventions/self-bootstrap.md)。完整的投影生命週期請參閱 [projection.zh-Hant.md](./projection.zh-Hant.md)。

## 貢獻者關卡

重大或非瑣碎的變更必須循序通過：planning、必要的 deep planning、refining、human approval、prompt generation、implementation、test、audit 與 finalize。實作受保護路徑 (Protected Paths) 之前，必須先具備經由架構師審查 (architect-reviewed) 的計畫，完整受保護路徑清單記載於 `.dev/project.md`。

送出變更前請逐一確認：

- 變更符合 crate 與各層邊界規範。
- 公開指令介面 (command surface) 與所有執行環境維持一致。
- 受追蹤的來源合約與產生的轉接器 (generated adapters) 之間未發生漂移。
- 新增或修改的測試皆已實際執行，且未將 `NotRun` 誤判為通過。
- 文件與連結確實反映當前系統行為，且未複製其他權威檔案的合約內容。
- 名稱完全遵守 [`docs/glossary.md`](../../glossary.md) 的權威定義。

## 慣例與工作流程合約

貢獻程式碼前，請依當前任務研讀對應的合約檔案。這些檔案是約束力的來源，本文件僅提供指引，不重複其內容。

| 合約 | 研讀時機 |
| --- | --- |
| [`workflows/coding.md`](../../../plugins/gal-core/workflows/coding.md) | 進行任何實作工作前（此為具約束力的編碼工作流程） |
| [`workflows/doc-sync.md`](../../../plugins/gal-core/workflows/doc-sync.md) | 程式碼變更需要進行文件影響分析時 |
| [`workflows/research.md`](../../../plugins/gal-core/workflows/research.md) | 執行 `/gal research` 或 `/gal deep-research` 時 |
| [`conventions/rust.md`](../../../plugins/gal-core/conventions/rust.md) | 進行任何 Rust 或 Cargo 工作前（此規則絕無例外） |
| [`conventions/naming.md`](../../../plugins/gal-core/conventions/naming.md) | 新增或重新命名任何 crate、模組、指令、設定鍵或角色時 |
| [`conventions/task-atomicity.md`](../../../plugins/gal-core/conventions/task-atomicity.md) | 將工作拆解為 `T-NN` 任務時 |
| [`conventions/task-quality.md`](../../../plugins/gal-core/conventions/task-quality.md) | 細化任務內容時（管道的任務檢查直接以本檔為準） |
| [`conventions/open-questions.md`](../../../plugins/gal-core/conventions/open-questions.md) | 撰寫或收斂 `## Open Questions` 時（H、A、F 三種分級定義於此） |
| [`conventions/handoff-notes.md`](../../../plugins/gal-core/conventions/handoff-notes.md) | 撰寫交接筆記或處理重試授權時 |
| [`conventions/token-budget.md`](../../../plugins/gal-core/conventions/token-budget.md) | 權杖紀律的完整政策，下節提供開發者摘要 |
| [`conventions/minimalism.md`](../../../plugins/gal-core/conventions/minimalism.md) | 評估特定抽象層或設定選項是否應當存在時 |
| [`conventions/core-vs-personal.md`](../../../plugins/gal-core/conventions/core-vs-personal.md) | 判定特定內容屬於核心合約或個人層時 |
| [`conventions/self-bootstrap.md`](../../../plugins/gal-core/conventions/self-bootstrap.md) | 建置私有執行檔世代、驗證發行 fixture 或升級穩定 shim 版本時 |
| [`conventions/working-hours.md`](../../../plugins/gal-core/conventions/working-hours.md) | 判斷目前是否受到收尾時間 (Wrap-up Time) 或硬性停止 (Hard Stop) 約束時 |
| [`conventions/optional-capabilities.md`](../../../plugins/gal-core/conventions/optional-capabilities.md) | 使用任何選用外部能力前（五態 preflight 與誠實降級規則定義於此） |

## 權杖紀律

下列規則適用於本儲存庫中所有維護者與代理程式的工作。完整政策記載於 [`conventions/token-budget.md`](../../../plugins/gal-core/conventions/token-budget.md)，此處僅列出開發摘要：

- **請勿讀取自動產生的檔案：** 除非當前任務正是要稽核這些檔案，否則請勿讀取產生的轉接器（`AGENTS.md`）或建置輸出（`bin/`、`obj/`）。這些檔案體積龐大且經常重新產生，並不包含來源範本以外的資訊。
- **定向探索：** 讀取任何檔案前，先確認該檔案已被當前任務指名，或是指名檔案的直接相依項目。取得所需資訊後即應停止讀取，切勿在冷啟動階段將整個程式碼庫載入脈絡中。
- **僅輸出失敗資訊：** 執行建置時僅需輸出第一個錯誤連同檔案與行號，建置成功時僅輸出一行摘要。測試僅需輸出失敗的測試名稱與斷言訊息，切勿印出通過的測試名稱。程式碼檢查 (Lint) 僅輸出檔案與違規規則，完全通過時僅輸出一行摘要。若需要完整日誌，請儲存至磁碟後再精準擷取特定行數，切勿將整份日誌全數倒入脈絡中。
- **脈絡壓力復原：** 任務進行中若脈絡接近上限，請依序執行三項步驟：第一，將當前任務名稱、最後完成的步驟以及關鍵決策寫入使用中計畫 `## Status` 區段的 `### Handoff Notes`。第二，在 `.dev/state.md` 的 `## Session Continuity` 更新對應的計畫列，填寫 `Stopped At` 與 `Next Step`。第三，切勿另外建立 `CONTEXT.md`，計畫檔與狀態檔即為唯一合法的持久工作階段狀態儲存處。

## 文字品質檢查

本儲存庫使用固定版本的 textlint 工作區檢查英文、台灣繁體中文與日文。這是開發工具。已安裝的 GAL 與下游專案不需要本檢出版本的 Node 工作區，也不會新增 GAL 設定鍵。終端使用者文件 [`docs/setup.md`](../../../docs/setup.md) 與 [`docs/configuration.md`](../../../docs/configuration.md) 不包含此開發工具。

請在儲存庫根目錄安裝鎖定檔中的相依套件，再執行必要檢查：

```text
npm ci --prefix tools/writing
node tools/writing/check.mjs --required
```

檢查檔案時，以 `--files` 傳入路徑。wrapper 先採用有效的明確 `--locale`，再讀取支援的 `lang` 前置中繼資料，最後使用已知受管路徑。明確語系接受 `en`／`en-US`、`zh-TW` 及 `ja`／`ja-JP`。未知路徑須有有效中繼資料或明確語系。除非有效的明確語系已使中繼資料不被使用，否則無效中繼資料會造成作業失敗。必要檢查模式逐檔選擇語系，不接受 `--locale`。官方 MCP 呼叫端須先依相同優先序判定，再選擇可信 JSON `--config`。虛擬檔名不決定語系。有限的中繼資料語法及 MCP 指令見 [`tools/writing/README.md`](../../../tools/writing/README.md)。

wrapper 會為目前文件根目錄發布不可變的術語快照。直接使用 CLI／MCP 前，須以該文件根目錄作為工作目錄，執行 `node <absolute-path-to-terms.mjs>`。此絕對路徑必須指向已安裝且可信的腳本。直接入口會拒絕缺失、損壞或過期的快照。檢查不會自動修正送入的文字。診斷建議會指出規則來源，即使提供替代字串，也只供人工採用。

wrapper 預設使用 `cli` 傳輸入口。若要透過官方 MCP 取得相同的完整報告，請在本儲存庫根目錄執行 `node tools/writing/check.mjs --transport mcp --files <paths...>`。`--files` 會將後續參數全部視為路徑，因此 `--transport` 與 `--locale` 都須放在它之前。必要檢查模式也接受 `--transport mcp`。兩個入口都保留問題建議與來源、版本、設定檔雜湊、輸入雜湊、術語來源及略過的檢查。報告會記錄傳輸入口。交握成功時，也會記錄 MCP server名稱與版本。原生 textlint CLI／MCP 指令提供的是上游診斷，不是這份完整報告。

結束碼 `0` 表示檢查完成且沒有硬性問題。結束碼 `1` 表示檢查完成但有硬性問題。結束碼 `2` 表示檢查未能完成。單有建議性問題不會阻擋必要的檢查關卡。一般撰寫時，若選用工具無法使用，作者須逐則自我檢查，並回報一次限制，直到可用狀態或對交付的影響改變。若必要檢查無法使用、當機或逾時，關卡必須停止。作者須回報阻擋位置或工具復原方式。

`schemaVersion=1` 術語資料的欄位與獨立的報告結構，請參閱 [`tools/writing/README.md`](../../../tools/writing/README.md#terminology-data)。精確相依版本、規則權責與來源，請參閱 [`tools/writing/rule-provenance.md`](../../../tools/writing/rule-provenance.md)。

## 維護者專用子指令

下列 `gal` 子指令不屬於公開的 `/gal` 指令介面，專供維護者與釋出流程使用：

| 子指令 | 職責 |
| --- | --- |
| `gal restore [--yes]` | 將 GAL 來源儲存庫還原至上一個 `gal-last-good` 標記，來源層與衍生層一併還原 |
| `gal release --version <v>` | 產生釋出產出物，包含 `checksums.txt`、`artifact-manifest.json`，以及選用的 `--winget` 與 `--homebrew` 套件管理員資訊清單 |
| `gal release-notes` | 產生釋出說明 |
| `gal marketplace-snapshot --source <dir> --out <dir>` | 將公開的市集外掛樹算繪至指定輸出目錄。僅包含核心內容，不執行環境投影、不寫入 `~/.gal`、不含個人層，亦不附帶主機二進位檔 |
| `gal naming-gate` | 命名關卡，由 pre-commit hook 自動呼叫 |
| `gal translation-freshness` | 回報 `docs/i18n` 的翻譯新鮮度 |

### `gal restore` 的還原語意

| 層級 | restore 的具體行為 |
| --- | --- |
| **來源層** | 執行 `git reset --hard gal-last-good` 並搭配 `git clean -fd`。工作樹狀態會與標記的提交完全一致。標記後新增的受追蹤檔案與所有未追蹤檔案皆會全數移除 |
| **衍生層** | 委派給 `gal refresh`，自還原後的來源重新建置標準根目錄與各執行環境投影 |

`gal restore` 不會跨機器複製本機設定。

**`gal-last-good` 標記：** `/gal finalize` 會在落地的提交點加上輕量 Git 標籤 `gal-last-good`，此操作屬非致命性質。`gal restore` 將該標籤視為**唯一基準線**。若標籤不存在，指令會以 fail-closed 模式終止並提供具體建議訊息，絕不會退回 HEAD 或進行猜測。在全新自舉之後、首次執行 `/gal finalize` 之前，此標籤並不存在，期間 `gal restore` 會乾脆地拒絕執行。

**安全防護機制：** 進行任何變更前，`gal restore` 會在當前的 HEAD 建立 `gal-restore-backup-<ts>` 分支。此為**硬性前置條件**，若備份分支建立失敗，指令會立即中止且完全不更動工作樹，確保不會遺失任何內容。

**確認關卡：** 若未加上 `--yes` 參數，指令僅會印出即將被捨棄的範圍（包含未提交的變更以及領先標籤的提交筆數），隨後以使用方式錯誤結束。此為刻意設計的預演 (dry-run) 路徑。

## 疑難排解

### 鎖定檔寫入順序

`update_skills()` 在 `update_commands()` 執行前即會持久化鎖定檔。若行程在這兩項操作之間異常終止，鎖定檔可能顯示 opencode 代理程式已登錄但指令尚未登錄。此狀態具備安全性，因為 `gal refresh` 具備等冪性，再次執行即可自動收斂至正確狀態。

### 孤立的技能目錄

`~/.config/opencode/skills/` 已列入 `legacy_paths` 陣列中。在具備明確 GAL 所有權證據時（如先前的鎖定檔歸屬、`.gal-managed` 標記或已驗證的 GAL 目標），會由 `remove_if_gal_owned_dir` 自動清除。若該目錄早於鎖定檔歸屬且缺乏所有權證據，故障防護機制會予以保留，此時 `gal doctor` 無法從鎖定檔辨識該目錄。請先手動檢查確認該目錄僅含過時的 GAL 投影，再行移除：

```powershell
Remove-Item -Recurse -Force "$env:USERPROFILE\.config\opencode\skills"
```

此項故障防護設計是刻意為之，目的在避免誤刪使用者恰好存放於 GAL 管理路徑下的個人檔案。

## 開發輔助技能

下列技能隨 GAL 一併釋出，用於輔助編寫程式碼與技術文件。這些技能皆為輔助工具，不構成強制關卡：

| 技能 | 用途 |
| --- | --- |
| [`result-pattern`](../../../plugins/gal-core/skills/result-pattern/SKILL.md) | 設計錯誤處理時，以 Result 模式處理預期內的失敗，例外僅保留給非預期的系統錯誤 |
| [`structured-logging`](../../../plugins/gal-core/skills/structured-logging/SKILL.md) | 增加日誌、挑選日誌等級與規劃日誌欄位結構時使用 |
| [`markdown-formatting`](../../../plugins/gal-core/skills/markdown-formatting/SKILL.md) | 編寫或修改本專案任何 Markdown 內容時使用 |
| [`doc-coauthoring`](../../../plugins/gal-core/skills/doc-coauthoring/SKILL.md) | 與使用者協同編寫技術文件、提案或規格書時使用 |
| [`doc-sync`](../../../plugins/gal-core/skills/doc-sync/SKILL.md) | 維護 NDJSON 結構地圖並偵測程式碼與文件間的漂移時使用 |
| [`local-first-search`](../../../plugins/gal-core/skills/local-first-search/SKILL.md) | 回答觀念問題或規劃筆記相關工作前，優先檢索儲存庫自有的技術材料 |
| [`defuddle`](../../../plugins/gal-core/skills/defuddle/SKILL.md) | 擷取網頁內容時，萃取乾淨的 Markdown 格式以節省權杖 |
| [`skill-creator`](../../../plugins/gal-core/skills/skill-creator/SKILL.md) | 建立、修改或評測技能時使用 |
| [`mcp-builder`](../../../plugins/gal-core/skills/mcp-builder/SKILL.md) | 建置 MCP server 時使用 |
| [`install-gal`](../../../plugins/gal-core/skills/install-gal/SKILL.md) | 在新環境安裝或啟用 `gal` 二進位檔時使用 |
| [`adversarial-review`](../../../plugins/gal-core/skills/adversarial-review/SKILL.md) | 審查計畫、差異、文件、決策或論點時，審查角色共用的方法學 |
| [`opencli-research`](../../../plugins/gal-core/skills/opencli-research/SKILL.md) | 使用 OpenCLI 進行低權杖、結構化的外部檢索時使用 |
| [`text-flowcharts`](../../../plugins/gal-core/skills/text-flowcharts/SKILL.md) | 說明分支邏輯或多步驟流程時，繪製等寬純文字決策樹 |
| [`git-commits`](../../../plugins/gal-core/skills/git-commits/SKILL.md) | 產生符合本專案規範的提交訊息，或執行 git commit 動作時使用 |

## 選用的本機審查工具

`security-guidance` 是一套選用的 Claude Code 外掛，藉由 hooks 機制提供確定性特徵比對 (deterministic pattern matching)、每回合 diff 的背景模型審查，以及在 `git commit` 或 `git push` 時觸發的代理程式審查 (agentic review)。此外掛不會阻擋寫入或提交，亦非專案強制相依或合併關卡 (merge gate)。模型審查所產生的權杖消耗由貢獻者自行負擔。

此外掛需要 Python 3.7 以上版本，agentic layer 則需 Python 3.10 以上。初次執行時會在 `~/.claude/security/` 建立虛擬環境。Windows 使用者應確認 `python3` 未指向 Microsoft Store 的別名存根 (alias stub)。工具回報若觸及 GAL 的安全邊界，請依 [SECURITY.zh-Hant.md](./SECURITY.zh-Hant.md) 進行分流，若發現已釋出版本的漏洞，請經由該文件說明的私密回報管道通報。
