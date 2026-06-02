# 計畫：文件架構重構

> 來源：`golem-dockeeper` 全樹狀結構診斷 (2026-06-02)。僅限於計畫階段 — 此計畫目前尚未產生任何檔案移動。

## 目標

`docs/` 累積了組織性的偏移：一個檔名衝突、一個工具分散在兩個位置、一個混合指南/參考/工具素材的平面未分組根目錄、一個不完整的來源文件索引、過時的計畫生命週期殘留物，以及一個幾乎沒有涵蓋實際樹狀結構的 structure-map。當此計畫完成時，必須符合以下條件：

- 每個文件都有且只有一個標準的存放位置：標準指南/參考文件留在 `docs/` 根目錄，其他主題文件位於目的明確的子目錄。
- `installation-topology.md` 與 `release-matrix.md` 整理併入 `devguide.md`，原檔刪除；`personalization` → canonical `docs/manual.md`；所有翻譯統一進 `docs/i18n/<lang>/`；`docs/` 根目錄收斂為 `devguide` + `manual` + 索引 + 子目錄（`collaborative-tools/`、`i18n/`、`plans/`、`research/`、`structure/`）。
- 沒有任何兩個內容不相關的文件共用相同的基礎檔名。
- 三讀者三文件零重複：**README**＝壓縮地圖（≤400 本文、語言切換 head、深度委派來源文件）；**manual**＝使用者操作手冊（細部操作）；**devguide**＝maintainer/架構/發佈。
- 內容反映現況：以 plugin 模式後版本為準，移除過時 source-mode / provider-status 措辭；架構只在 devguide 講一次。
- `project.md` 來源文件索引與 `structure-map.ndjson` 皆能完整且真實地反映 `docs/` 樹狀結構。
- completed / VERIFIED 計畫保留為對照資料（install 改寫的事實來源）；本計畫不購除計畫。
- 未來的文件由書面的命名慣例與書面的翻譯政策規範。

## 執行模式 (Execution Mode)

- **僅文件變更，不走 pipeline**：本計畫只動 `docs/`、`README*`、`.dev/` 索引與 structure-map，無程式碼/測試，故不使用 `/gal pipeline`。
- **AI 直接執行、不代為 commit**：AI 直接編輯檔案並把變更留在 working tree / staged；**AI 不執行任何 git commit**。由使用者親自審視結果後自行 commit。
- **建議 commit 切片順序**（讓使用者逐片審查，而非一次審 ~20 檔 mega-diff；每片可獨立 commit）：
  1. **Slice 1 — 安全改名/移動**（低風險）：P1 graphworkflow 去重（→`blender-mcp.md`/`graphics-workflow.md`）、P2 xmachine 合併、`personalization*`→`manual*` 改名 + 全 repo 參照更新。
  2. **Slice 2 — devguide 合併**：吸收 `installation-topology.md`+`release-matrix.md`、6-H2 重構、刪原檔、anchor 重映。
  3. **Slice 3 — README 壓縮地圖 + manual 重寫**（最高風險·內容）：README ≤400、manual 操作手冊、install 敘述對齊 completed plans。
  4. **Slice 4 — 索引/慣例/結構**：`project.md` 來源文件、devguide 命名+翻譯政策節、structure-map 重掃。
  5. **Slice 5 — 轉接器重生**：`Sync-DevContext` 重生 `CLAUDE/GEMINI/AGENTS/copilot`（衍生物，最後做）。
- 每片做完先跑連結檢查（T2）再交使用者 commit；doc-sync 風格的成果可留 staged 不自動 commit。

## 需求

- [ ] R1 (P1) — 無檔名衝突：`docs/graphworkflow.md` (Blender MCP 檢查) 與 `docs/collaborative-tools/graphworkflow.md` (美術工作流程) 依名稱與內容進行區分。
- [ ] R2 (P2) — `xmachine` 具有單一的文件存放位置 (合約 + 範例放在一起)。
- [ ] R3 (P3) — `docs/` 根目錄保留標準指南/參考文件的平面入口；非標準主題文件存放於特定目的的子目錄下。
- [ ] R4 (P4 · 多語言可擴展翻譯政策) — 書面政策，設計為**語言與檔案皆可成長**（近期確定加 `ja`，tier 可能由 2 升 3）：
  - canonical = English（原位主檔名，不動）。**翻譯全進 `docs/i18n/<lang>/`**，檔名為 **`<name>.<lang>.md`**（資料夾＋檔名都帶語言＝刻意冗餘換自描述；basename 非 `README.md` 故滿足「README 唯一」R5）。
  - **鏡像路徑規則**：root `README.md` → `docs/i18n/<lang>/README.<lang>.md`；`docs/X.md` → `docs/i18n/<lang>/X.<lang>.md`（保留 `collaborative-tools/` 等子路徑）。root README 頂部語言切換連結指向 i18n 副本（R18）。
  - **可翻譯 allowlist**（非「全部文件」）：政策內明列允許翻譯的文件集合；目前 = `README`、`manual`（＋按需加入的少數工具文件＝tier 2）。新增文件或語言 = 在 allowlist/i18n 加一項，面積成長受控且可見。
  - maintainer/契約類（`devguide`、`collaborative-tools/*` 契約、`research`、`plans`）預設 EN-only，不在 allowlist。
  - **慣例變更**：此佈局取代 project.md「Project Language」現載的 `<name>.<lang>.md` 慣例 → 須一併更新 project.md（methodology memory 變更）。
- [ ] R5 (P5) — 存在書面的文件/檔案命名慣例 (`-mcp` 後綴規則、技能名稱對齊)；**`README.md` 專屬 root 唯一檔案**，子區入口文件改用 `guide.md` / `index.md`，全 repo 不得有第二個 `README.md`；README 的語言副本為 `docs/i18n/<lang>/README.<lang>.md`（basename 帶 infix、非 `README.md`，故不違反）。
- [ ] R6 (對照保留) — 本計畫**不**購除任何計畫；completed / VERIFIED 計畫保留為 install 改寫的對照資料。plan 生命週期購除另案處理，不屬本計畫。
- [ ] R7 (P7) — `project.md` 來源文件列出每一份標準文件；重新產生的轉接器 (adapters) 能夠反映它。
- [ ] R8 (P8) — `structure-map.ndjson` 涵蓋每一個 `docs/` 節點並具備真實的 `syncStatus`。
- [ ] R9 (合併) — `installation-topology.md`（maintainer 導覽圖）與 `release-matrix.md`（release/dist 參考）內容整理併入 `devguide.md`，原檔刪除，全 repo 連結改指 devguide 對應節。
- [ ] R10 (README ≤400 壓縮地圖) — README 重構為「壓縮地圖」：本文（**排除**頂部語言切換 head、`References`、`License`）≤400 行（理想 ~280）。每個清單/參考節壓成 compact table + 連結，README 給地圖不給全文。**委派目標分層**：使用者「怎麼用」的深度 → `manual.md`（新手學習路徑）；canonical 契約 → `commands/commands.md`、`agent/agents.md`、`workflows/coding.md`（reference 連結，非新手教學入口，因屬 agent-facing 契約/protected paths）。安裝狀態如實標 deferred 並連出（OQ-008）；不細講 git/clone。
- [ ] R19 (README↔manual 邊界規則) — 明文一條規則防再重複：README「How GAL Works」只給**一眼心智模型 + 連結**（是什麼）；manual「Daily Use」給**實際操作步驟**（怎麼做）。同一主題不得在兩處都展開。
- [ ] R11 (dev mode 落點) — 「如何開啟 dev mode」內容夠短，只放在 README 新增的精簡 Dev Mode 節（clone + `Setup-Machine`）；devguide **不**重複此啟用說明，只連回 README。
- [ ] R12 (personalization → manual) — `personalization.md` → **`docs/manual.md`**（canonical EN）；`personalization.zh-Hant.md` → **`docs/i18n/zh-Hant/manual.zh-Hant.md`**（使用者操作手冊），吸收所有細部操作：runtime/config/placeholders/companion、daily-use 走查、headless routing、when-to-rerun，以及由 README 移入的 backup/migration/uninstall 與 install-status 細節。內容改寫為 plugin 模式，架構/擁有權仍委派 devguide。全 repo `personalization*` 參照改指新位置。
- [ ] R13 (P9 · 決策) — `godot.md`、`graphics-workflow.md` 現為 zh-canonical → **補寫 EN canonical（由 zh 內容翻譯，留原位）+ 現有 zh 內容移入 `docs/i18n/zh-Hant/collaborative-tools/<name>.zh-Hant.md`** 並加新鮮度 front-matter。完成後全 docs 皆 EN canonical。*(人工決策 2026-06-02)*
- [ ] R14 (P10) — `graphify-execution-guide.md`（1059 行，逐步教學 vs 契約混雜）處理：拆章或移出 contract 目錄（見 OQ-007）。
- [ ] R15 (P11) — 併入 devguide 時順手統一標題風格：`release-matrix` 的編號 H2（`## 1.`…）去編號，與全 repo 無編號 H2 一致。
- [ ] R16 (P12 · 多語言新鮮度標記) — 每個 `docs/i18n/<lang>/` 翻譯檔 front-matter 帶 `source`（repo 相對路徑）/ `lang` / `source_commit` / `translated_at`；一個 check 掃 `docs/i18n/**` 比對 `git log -1 --format=%H -- <source>` 與 `source_commit`，逐 (doc, lang) 報 `current|stale|missing`。機制隨語言/文件數線性擴展，不擴 structure-map schema。
- [ ] R20 (多語言擴展就緒) — 命名標籤、allowlist、新鮮度 check 三者設計即支援 N 語言；`ja` 為已知近期語言，機制須 JP-ready。**本計畫不產生 `ja` 檔案內容**（屬後續內容工作），僅確保政策與 check 可直接容納。
- [ ] R21 (i18n 薄路標) — 新增 `docs/i18n/guide.md`（EN-only meta，不列 allowlist）：用途、資料夾/命名範例、現有語言清單、「如何加一個語言/翻譯」步驟 + front-matter 模板，並連回 devguide 權威政策。**不複製**政策 rationale，避免雙處 drift。
- [ ] R18 (語言切換 head) — canonical（`README.md`、`docs/manual.md`）與各 `docs/i18n/<lang>/` 副本頂部加語言切換連結（canonical ↔ 各語言）；該 head 不計入 README 400 行預算。
- [ ] R17 (devguide 狠整併) — devguide 由 18 個平鋪 H2 重構為 **6 個 H2**：`Overview & Dev Setup` / `Install Mode vs Source Mode` / `Codebase & Runtime Structure` / `Distribution & Release Architecture` / `Making Changes` / `Conventions`。標註版 `Codebase Data Structure` 與 `.gal Data Structure` tree 自帶「用途·層·owner·protected·schema」，吸收並取代 `Start By Finding The Right Layer`、`Where Information Belongs`、`Owning Surfaces`、`Runtime File Schemas`。合併重疊節（`Runtime Topology`+topology `Runtime Flow`、`Distribution Architecture`+`Ownership Boundaries`、`Provider Packaging`+`Support Tiers`、`Verify`+`Self-Check`）。刪除整個 Quick Reference（`Suggested Reading Order`、`Related Files`、獨立 `Self-Check`）。dev-mode 啟用不在 devguide 重複。

## 方法

### 步驟 1：鎖定目標分類架構

- **檔案**：此計畫、`docs/` (提議的樹狀結構)
- **內容**：確認子目錄集合及其包含的內容：
  - `docs/` 根目錄 — 收斂為 `devguide`（吸收 installation-topology + release-matrix）、`manual`（原 personalization 改名，canonical EN）、可選 `README.md` 索引。
  - `docs/i18n/<lang>/` — **新樹**：所有語言翻譯統一落點（`zh-Hant`、未來 `ja`…）。
  - `docs/collaborative-tools/` — 每個功能/工具一個檔案；保留此名稱，不重新命名為 `docs/tools/`。
  - `docs/plans/`、`docs/research/`、`docs/structure/` — 角色不變。
- **驗證**：每個現有的 `docs/*.md` 在進行任何移動前，書面上皆對應到且僅對應到一個標準位置。

### 步驟 2：解決檔名衝突 (P1)

- **檔案**：`docs/graphworkflow.md`、`docs/collaborative-tools/graphworkflow.md`
- **內容**：將根目錄的 `graphworkflow.md` 重新命名為 `blender-mcp.md` (與 `codebase-memory-mcp.md`、`playwright-mcp.md` 放在一起)。將 `collaborative-tools/graphworkflow.md` 重新命名為 `graphics-workflow.md` 以匹配其支援的技能 `graphics-workflow`。
- **驗證**：沒有任何兩個 `docs/**/*.md` 共用基礎檔名；這兩個新名稱都可從索引中找到。

### 步驟 3：整合 xmachine (P2)

- **檔案**：`docs/collaborative-tools/xmachine.md`、`docs/xmachine/examples/`
- **內容**：選擇一個存放位置；將 `examples/` 集合移動到合約旁 (或所選的工具子目錄下)。移除現在已經為空的 `docs/xmachine/`。
- **驗證**：`find docs -ipath '*xmachine*'` 僅解析出單一子目錄。

### 步驟 4：合併 + 改寫根目錄文件 (P3, R9–R12, R15)

- **檔案**：`docs/devguide.md`、`docs/installation-topology.md`、`docs/release-matrix.md`、`docs/personalization(.zh-Hant).md`、`README(.zh-Hant).md`
- **內容**：
  - **devguide 6-H2 狠整併（R17）**：見下方目標藍圖。`installation-topology.md` 五塊與 `release-matrix.md` 九塊**不整段貼上**，而是按主題拆散歸位後刪除原檔。
  - README 安裝區整段重構（R10/R11）：詳見步驟 4b。
  - personalization 改寫為 plugin 模式後版本（R12）。

#### devguide 目標藍圖（18 H2 → 6 H2）

```text
## Overview & Dev Setup              dev 安裝 → README#dev-mode（不重複）
## Install Mode vs Source Mode
## Codebase & Runtime Structure
   ### Codebase Data Structure        標註版 tree：用途·層·owner·protected
   ### .gal Data Structure            標註版 tree：用途·owner·schema
## Distribution & Release Architecture
   ### Ownership Boundaries · Provider Plugin Packaging & Support Tiers ·
       Catalog & Lockfile · Release Artifact Matrix(去編號) · Bootstrap Distribution Doc Ownership
## Making Changes
   ### Rules & Fences · Before Editing Install · Change Entry Points ·
       Adding a New CLI Runtime · Verify & Self-Check · Drift Queue
## Conventions
   ### Documentation Conventions(命名+翻譯) · Token Discipline
```

| 原節 | 去向 |
| --- | --- |
| Start By Finding The Right Layer / Where Information Belongs / Owning Surfaces(topology) / Runtime File Schemas | 塌進標註版 tree |
| Runtime Topology + topology Runtime Flow | 合併進 Distribution（setup→`~/.gal` 一條流程） |
| Distribution Architecture + topology Ownership Boundaries | 合併為 `Ownership Boundaries` |
| Plugin Support Tiers | 併入 `Provider Plugin Packaging & Support Tiers` |
| Self-Check | 併入 `Verify & Self-Check` |
| Suggested Reading Order / Related Files / 整個 Quick Reference | 刪除 |
| topology Before Editing Install / Drift Queue | 移入 `Making Changes` |
| release-matrix 9 節 | 去編號 → `Release Artifact Matrix` 內 H3 |
- **驗證**：`docs/` 根目錄只剩 devguide、manual、可選索引，以及 `i18n/`/`plans/`/`research/`/`structure/`/`collaborative-tools/`；`installation-topology.md`、`release-matrix.md` 不再存在且無斷連。

### 步驟 4b：README 壓縮地圖 (R10, R11, R18)

- **檔案**：`README.md`（canonical）、`docs/i18n/zh-Hant/README.zh-Hant.md`（由 root `README.zh-Hant.md` 遷入）
- **內容**：README 變「壓縮地圖」，深度全部委派來源文件；本文（排除 head/References/License）≤400、理想 ~280。

```text
<head: 🌐 English | 繁體中文>          ← 語言切換, 不計入 400 (R18)
## What is GAL              ~15
## Quick Start              ~30   plugin: 安裝 → /gal init → 第一個 workflow
## Dev Mode                 ~12   clone+Setup-Machine 一塊 → devguide
## How GAL Works            ~80   心智模型 + table/連結，不放全文
      ### Lifecycle 走查 · Commands→commands.md · Golem Agents→agents.md ·
          Workflow→workflows/coding.md · Research→research 文件
## Files & Storage          ~30   標註版 tree
## Collaborative Tools      ~18   一段 + 表 → collaborative-tools/
## Learn More / Next        ~15   連結 manual · devguide · …
(## References / ## License        不計入 400)
```

| 原 README 內容 | 去向 |
| --- | --- |
| Prerequisites | 壓成 Quick Start 一行前置 |
| Current Installer Status（散文） | 壓成 Quick Start deferred callout；channel 細節 → devguide/release-matrix |
| Install, Fallbacks, Migration（backup/uninstall/migration） | **→ `manual.md`**（使用者 task-time 操作） |
| Golem Agents（~84 行散文） | compact 表 + 連 `agent/agents.md` |
| Commands 全清單 | compact 表 + 連 `commands/commands.md` |
| Development Workflow 細節 | 只留 lifecycle 圖 + 連 `workflows/coding.md` |
| Storage Boundaries + Project Files | 合併 `Files & Storage`，標註 tree |
| Personalization | **→ `manual.md`**（README 只連結） |

- **驗證**：README 本文（排除 head/References/License）≤400；每個壓縮節都有對應來源連結且無斷連；`docs/i18n/zh-Hant/README.zh-Hant.md` 鏡像 + 語言切換 head；root 不再有 `README.zh-Hant.md`。

### 步驟 4c：personalization → manual 操作手冊 (R12, R16, R18)

- **檔案**：`personalization.md`→`docs/manual.md`（canonical EN）、`personalization.zh-Hant.md`→`docs/i18n/zh-Hant/manual.zh-Hant.md`
- **內容**：改名/遷移並重構為使用者操作手冊，吃下所有細部操作；架構/模式/擁有權委派 devguide（不複述）。

```text
<head: 🌐 English | 繁體中文>          (R18)
## Overview
## First-Time Setup          ### Runtime Selection · Configuration & Placeholders · Companion Plugins
## Daily Use                 ### The Workflow · Commands in Practice · Golem Agents in Practice
## Machine Operations        ### Headless Routing · When To Rerun · Backup, Migration & Uninstall
## Install Status & Channels  簡短 → devguide / release-matrix
```

- **驗證**：無 `personalization*` 殘留參照；`docs/manual.md` 不複述 devguide 架構；`docs/i18n/zh-Hant/manual.zh-Hant.md` 鏡像 + 新鮮度標記（R16）。

### 步驟 5：掃描所有交互參照 (cross-references)

- **檔案**：每個連結至被移動文件的追蹤檔案；`.dev/project.md`；`xmachine.config.example.json`；受影響的 `skills/**/SKILL.md`
- **內容**：用 graphify、codebase-memory-mcp 與直接檔案層掃描交叉盤點參照；更新相對連結。更新 `project.md` 來源文件 (驅動 P7 步驟)。透過 `Sync-DevContext` 重新產生轉接器 — **請勿**手動編輯 `CLAUDE.md` / `GEMINI.md` / `AGENTS.md`。
- **驗證**：全儲存庫相對連結檢查發現零個損壞的目標；轉接器乾淨地重新產生。

### 步驟 6：保留 completed plans 作為對照資料（不購除）

- **範圍**：本計畫 **不** 刪除任何 `docs/plans/` 或 `.dev/plans/` 計畫。
- **理由**：completed / VERIFIED 計畫（特別是 install 相關：`feat-plugin-arch-migration.md`、install-ownership、headless-cli 等）是本次 README/manual/devguide 改寫的**事實對照來源**；使用者需用它們判斷 install 失敗的真正原因，整理完成前不得移除。
- **動作**：僅把這些計畫當輸入閱讀；plan 生命週期購除移出本計畫範圍，另案處理。

### 步驟 7：撰寫命名與多語言翻譯慣例 (P4, P5, R4, R13, R16, R20)

- **檔案**：`docs/devguide.md`（`Documentation Conventions` 節）、`.dev/project.md`（Project Language 慣例）、`docs/i18n/<lang>/`（新樹）、`docs/godot.md`、`docs/graphics-workflow.md`、新鮮度 check 腳本
- **內容**：
  - **命名慣例**：工具文件 `-mcp` 後綴政策、技能名對齊。
  - **多語言翻譯政策（R4/R20）**：canonical=EN 原位；翻譯全進 `docs/i18n/<lang>/`（鏡像路徑規則，見 R4）；**可翻譯 allowlist**（目前 README、manual；可成長）；maintainer/契約類預設 EN-only。**更新 project.md「Project Language」**：檔名 `<name>.<lang>.md` 保留，但位置由「與 canonical 同層 sibling」改為 `docs/i18n/<lang>/` 資料夾。
  - **遷移既有翻譯**（檔名 `.zh-Hant` 不變，只移進資料夾）：`README.zh-Hant.md` → `docs/i18n/zh-Hant/README.zh-Hant.md`；`personalization.zh-Hant.md` → `docs/i18n/zh-Hant/manual.zh-Hant.md`（stem 改 manual）；`xmachine.zh-Hant.md` → `docs/i18n/zh-Hant/collaborative-tools/xmachine.zh-Hant.md`（xmachine 列入 allowlist）。移動後重寫其相對連結。
  - **新鮮度機制（R16）**：翻譯 front-matter（`source`/`lang`/`source_commit`/`translated_at`）+ 跨語言 check 報 `(doc,lang)=current|stale|missing`。
  - **i18n 薄路標（R21）**：新增 `docs/i18n/guide.md`，只放用途+資料夾範例+語言清單+加語言步驟+front-matter 模板+連回 devguide；不複製政策內文。
  - **canonical 語言正規化（R13）**：`godot.md`、`graphics-workflow.md` 補 EN canonical（由 zh 翻譯，留原位），原 zh → `docs/i18n/zh-Hant/collaborative-tools/<name>.zh-Hant.md` + front-matter。
- **驗證**：政策已撰寫並被索引參照；project.md 慣例已更新；`docs/i18n/zh-Hant/` 下無斷連；check 對現有翻譯給出 `(doc,lang)` 狀態；godot/graphics-workflow 皆 EN canonical，無 zh-canonical 殘留；root 與 docs 根目錄不再有 `*.zh-Hant.md`。

### 步驟 8：擴充 structure-map (P8)

- **檔案**：`docs/structure/structure-map.ndjson` (+ 如果只有文件的節點需要結構描述檔案的話)
- **內容**：重新掃描目前 `docs/` 樹狀結構，並以 doc-sync 現有 schema 產生 doc-axis `doc-section` 節點；預設涵蓋每個標準文件的 h2/h3，h4-h6 僅在被 `codeRefs` 引用或實際變更時物化。使用 graphify 與 codebase-memory-mcp 作為 advisory lanes，但以直接檔案掃描作為必備基準。保留未接觸行的每一個位元組。
- **驗證**：每個 `docs/**/*.md` 都經過重新掃描並有相應 doc-section 覆蓋判定；結構描述驗證通過；排序穩定 (先 `axis` 後 `id`)。

## 建立或修改的檔案

- `docs/index.md` (可選索引；**不命名為 README.md**，R5) — 新架構；不建立 `docs/guides/`
- `docs/collaborative-tools/blender-mcp.md`、`docs/collaborative-tools/graphics-workflow.md` (重新命名/移入) — P1
- **刪除（內容併入 devguide）**：`docs/installation-topology.md`、`docs/release-matrix.md` — R9
- **改名**：`docs/personalization.md`→`docs/manual.md`（canonical EN，R12）
- **新增** `docs/i18n/guide.md`（薄路標·EN-only，R21）
- **新樹 `docs/i18n/<lang>/`**（翻譯統一落點，R4）：
  - `personalization.zh-Hant.md` → `docs/i18n/zh-Hant/manual.zh-Hant.md`
  - `README.zh-Hant.md` → `docs/i18n/zh-Hant/README.zh-Hant.md`
  - `docs/collaborative-tools/xmachine.zh-Hant.md` → `docs/i18n/zh-Hant/collaborative-tools/xmachine.zh-Hant.md`
  - `godot`/`graphics-workflow` 的 zh → `docs/i18n/zh-Hant/collaborative-tools/<name>.zh-Hant.md`
- 保留 docs/ 根：`devguide.md`、`manual.md`（canonical）
- 已移動：xmachine 範例 → `collaborative-tools/examples/`
- 大幅改寫：`docs/devguide.md`（吸收 topology + release-matrix、6-H2、命名/翻譯政策節）、`README.md` + `docs/i18n/zh-Hant/README.zh-Hant.md`（≤400 壓縮地圖 + 語言切換 head）、`docs/manual.md` + `docs/i18n/zh-Hant/manual.zh-Hant.md`（plugin 模式操作手冊，吸收 README 細部操作）
- **慣例更新**：`.dev/project.md` Project Language 節改為 `docs/i18n/<lang>/<name>.<lang>.md` 佈局（infix 保留，sibling → 資料夾）
- `.dev/project.md` — 來源文件索引 (P7)
- `.dev/state.md` — Active Plans / Session Continuity 列 (P6)
- `docs/structure/structure-map.ndjson` — 完整的文件涵蓋範圍 (P8)
- 跨追蹤文件、`skills/**/SKILL.md`、`xmachine.config.example.json` 的交互參照連結更新
- 重新產生 (非手動編輯)：`CLAUDE.md`、`GEMINI.md`、`AGENTS.md`、`.github/copilot-instructions.md`

## 測試案例

- [ ] T1 — `find docs -name '*.md' | basename-collision check` → 零個不相關的衝突。
- [ ] T2 — 移動後進行全儲存庫相對連結檢查 → 零個損壞的連結。
- [ ] T3 — `project.md` 來源文件集合 == 重新掃描後的 `docs/**/*.md` 標準集合 (排除仍在生命週期中的 `docs/plans/`)。
- [ ] T4 — `Sync-DevContext` 重新產生轉接器，後續無需手動 diff。
- [ ] T5 — `structure-map.ndjson` 可解析、通過 schema 驗證，並且每個 `docs/**/*.md` 都已被重新掃描且有對應 h2/h3 doc-section 覆蓋判定。

## 成功標準

- [ ] SC1 — 每個文件都有一個標準的存放位置；根目錄標準文件保留平面入口；無不相關的基礎檔名衝突 (R1, R3)。
- [ ] SC2 — 單一的 xmachine 存放位置 (R2)。
- [ ] SC3 — 來源文件索引與 structure-map 皆完整涵蓋 `docs/` (R7, R8)。
- [ ] SC4 — completed / VERIFIED 計畫完整保留為對照資料；本計畫未刪任何計畫 (R6)。
- [ ] SC5 — 已撰寫命名與翻譯慣例並被參照 (R4, R5)。
- [ ] SC6 — 沒有損壞的交互參照；轉接器從原始碼重新產生，而非手動編輯。

## 風險

- **高連結變動率 (High link churn)** — 參照計數：`personalization` 15，`collaborative-tools/` 19，`xmachine` 12，`devguide` 9，`installation-topology`/`release-matrix`/`graphworkflow(collab)` 各 4。任何移動若掃描不完整，都有破壞連結的風險 → 步驟 5 的連結檢查是嚴格的把關條件。
- **觸及產生的轉接器** — `CLAUDE.md`/`GEMINI.md`/`AGENTS.md` 是衍生的；它們「必須」透過 `Sync-DevContext` (一個受保護的腳本) 重新產生，而非手動編輯。
- **completed plans 為對照、不可先刪** — install 相關 completed/VERIFIED 計畫是本次改寫的事實來源；本計畫不購除，purge 另案。
- **標準文件的預期** — OQ-002 已決定保留 `devguide`/`manual` 等標準文件於 `docs/` 根目錄平面入口；後續實作不得再將其搬移至 `guides/`。
- **devguide 體積膨脹** — 吸收 installation-topology(78) + release-matrix(336) 後 devguide 將達 ~1000–1100 行，逼近單檔可讀性上限（見 OQ-007）。
- **README 安裝敘述準確性** — plugin/install 模式對 Claude/Codex/Copilot 仍 deferred；README 改寫不得 overclaim 尚未 ship 的 native install 路徑，否則與 repo 現況及 release-matrix 政策衝突（見 OQ-008）。
- **內容改寫需現況真實來源** — `manual` / README / devguide 的 install 改寫屬內容（非純搬移），須以 install 相關 completed plans（`feat-plugin-arch-migration.md` 等）＋當前 `scripts/Install-GalPlugins.*`、`~/.gal` 佈局為事實來源，避免把舊敘述換成另一種錯誤敘述。使用者並要藉此確認 install 失敗的真正原因。
- **i18n 遷移的相對連結與慣例變更** — 翻譯移入 `docs/i18n/<lang>/` 後，檔內相對連結需回指 canonical（多為 EN-only，可控）；此佈局**改變 project.md 既載慣例的「位置」**（sibling → 資料夾，檔名 `<name>.<lang>.md` infix 保留）＝methodology memory 變更，須一併更新並重生轉接器。連結檢查（T2）須涵蓋 `docs/i18n/**` 與 anchor 層級（MISS-03）。
- **跨領域 / 架構師邊界 (Cross-cutting / architect fence)** — 此次重構修改了轉接器、來源文件索引及許多連結 → 超出了直接實作的範圍；在 `/plan-to-prompt` 之前需要 `/deep-planning` 架構師的審查。

## 待解決問題 (Open Questions)

- [x] OQ-001 — 決定：保留 `docs/collaborative-tools/` 名稱，不重新命名為 `docs/tools/`。*(人工決策 2026-06-02；graphify 顯示 README 已將 Collaborative Tools 視為現有概念，且參照變動率高)*
- [x] OQ-002 — 決定：標準指南/參考文件保持在 `docs/` 根目錄的平面結構，不建立 `docs/guides/` 搬移 `devguide` / `personalization`。*(人工決策 2026-06-02)*
- [x] OQ-003 — 決定：命名與翻譯慣例新增為 `docs/devguide.md` 的一節。*(人工決策 2026-06-02)*
- [x] OQ-005 — 決定：需要重新掃描 `docs/`。現有 schema 支援 doc-axis `doc-section`，不先新增 file-only doc record shape；本計畫以重新掃描後的 h2/h3 doc-section 物化作為覆蓋基準，只有掃描證明不足時才回到 schema 變更。*(graphify detect + codebase-memory/doc-sync 檢查 2026-06-02)*
- [x] OQ-007 — 決定：接受單一大型 devguide，installation-topology 與 release-matrix 全部成為 devguide 內的節（~1100 行）。*(人工決策 2026-06-02；R9/R15)*
- [x] OQ-008 — 決定：README Quick Start 如實標註 deferred — plugin 模式為方向，明列 AGY 已可用、Claude/Codex/Copilot 仍 deferred，與 release-matrix 政策一致。*(人工決策 2026-06-02；R10)*
- [x] OQ-009 — 決定（翻譯範圍/擴展）：翻譯面積目前鎖 tier 2（README + manual + 按需少數工具文件），但政策須**允許成長**（不保證不升 tier 3）；語言**多語言可擴展**，`ja` 為已知近期語言。機制（命名標籤 + allowlist + 新鮮度 check）即支援 N 語言；本計畫不產 `ja` 內容。`godot`/`graphics-workflow` 補 EN canonical + zh 副本。*(人工決策 2026-06-02；R4/R13/R16/R20)*
- [x] OQ-010 — 決定（翻譯佈局）：所有翻譯**全進 `docs/i18n/<lang>/`**，檔名保留 `<name>.<lang>.md`（如 `README.zh-Hant.md`，自描述且滿足 README 唯一）；canonical 留原位；舊 sibling 慣例改為資料夾；既有 `*.zh-Hant.md` 直接遷入 i18n（檔名不變）。*(人工決策 2026-06-02；R4/R18/R5)*

## 審核 (Approval)

- 人工審核：[待處理]
- 架構師審核：[CLEAR — APPROVE，4 條件已折入（2026-06-02）]
- 其他領域審核：[未要求 — 內部文件方法，無 customer-facing 業務/設計面]

## 審核結果

### 架構審核

**Verdict: APPROVE**（4 項條件已於本次 deep-planning 折入計畫）。範圍合理、無過度工程；風險集中在內容正確性與大型 diff 審查，非結構性缺陷。

**Trade-off Summary**

| 決策 | 益處 | 成本 | 判定 |
| --- | --- | --- | --- |
| devguide 吸收 topology+release-matrix（單檔 ~1100 行） | 單一 maintainer 入口、零跨檔跳轉 | release 參考的高頻 churn 耦合進 maintainer guide | OK（OQ-007 已決） |
| README ≤400 壓縮地圖、深度委派 | 最快理解、可讀性上限內 | 委派目標若是 agent-facing 契約，新手體驗下降 | REVISE→已修：usage→manual，契約僅 reference（R10） |
| 3 文件零重複（README/manual/devguide） | 各一讀者、無重複維護 | README「glance」與 manual「practice」主題重疊易再漂移 | REVISE→已修：加邊界規則（R19） |
| 直接執行 + 人工 commit | 文件變更可由人審視把關 | 一次 ~20 檔 mega-diff 難審 | REVISE→已修：5 段 commit 切片（執行模式） |

**Findings（皆已折入）**

- **BUG-01**（已修）：步驟 4 驗證殘留 `personalization`，rename 後應為 `manual` → 已更正。
- **OE/REVISE-01**（已修）：README 委派目標未分層 → R10 區分 usage(manual) vs reference(契約)。
- **MISS-01**（已修）：manual-commit 模式缺 commit 切片 → 新增「執行模式」5 段切片順序。
- **MISS-02**（已修）：README↔manual 主題重疊缺防漂移規則 → 新增 R19。
- **MISS-03**（待實作注意）：刪 `installation-topology.md`/`release-matrix.md` 後，他檔的 **section-anchor 連結**（如 `release-matrix.md#package-manager-publication`）需重映到 `devguide.md#...`，比檔案層 rename 難；步驟 5 連結檢查須涵蓋 anchor 層級，不只檔名。

**What's Good（保留）**：強制連結檢查 gate（T2）、不手改轉接器（Sync-DevContext）、completed plans 留作 install 事實來源、structure-map 重掃、標註 tree 取代散落導覽節。

**未決阻斷**：無。`## Tasks` / `## Test Plan` 仍為 placeholder → 非架構阻斷，屬 `/refining-plan` 範圍。

### 業務審核

待處理。

### 設計審核

待處理。

### 工程審核

**Verdict: CLEAR.**

Rationale：
- 範圍純文件（`docs/`、`README*`、`.dev/` 索引、structure-map、一個新鮮度 check 腳本），無程式邏輯/runtime 風險。
- 15 個 T-NNN 各自獨立可完成、可測；對齊執行模式 5 段 commit 切片，依風險遞增（機械 → 結構 → 內容 → 索引 → 轉接器），讓使用者逐片審後自行 commit。
- 主要執行風險＝(a) 內容正確性、(b) 大型 diff、(c) anchor 重映；分別由「completed plans 為事實來源」「切片化」「T-015 連結+anchor 檢查 gate」緩解。
- 架構師已 CLEAR（4 條件折入）；所有 OQ-001~010 已決；無未決阻斷。

工程注意（非阻斷）：
- T-011 新鮮度 check 須遵 repo 雙 runtime（PS/Bash）慣例；可先落單一 runtime，另一 runtime 補齊前於 devguide 註明。
- TP-006 README 行數量測須明確定義「排除 head/References/License」的計法，避免判定爭議。

<!-- ENG_REVIEW: CLEAR -->

## 測試計畫

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | manual | `docs/**/*.md` 無不相關 basename 衝突；`blender-mcp.md`/`graphics-workflow.md` 可從索引達 | T-001 |
| TP-002 | script | `find docs -ipath '*xmachine*'` 僅單一子樹；無空 `docs/xmachine/` | T-002 |
| TP-003 | script | 無 `personalization*` 殘留參照；root 與 docs 根目錄無 `*.zh-Hant.md`；i18n 既有翻譯就位 | T-003 |
| TP-004 | manual | devguide 恰 6 個 H2、含標註 `Codebase`/`.gal` tree；`installation-topology.md` 已刪且無斷連 | T-004 |
| TP-005 | manual | `Release Artifact Matrix` 去編號入 devguide；`release-matrix.md` 已刪；舊 anchor 已重映 | T-005 |
| TP-006 | script | README 本文（排除 head/References/License）≤400 行；How GAL Works 委派連結全部有效 | T-006 |
| TP-007 | manual | manual 涵蓋 First-Time/Daily/Machine Ops/Install Status；install 敘述對齊 completed plans；不複述 devguide 架構 | T-007 |
| TP-008 | manual | `godot`/`graphics-workflow` 有 EN canonical；zh 在 i18n；全 docs 無 zh-canonical 殘留 | T-008 |
| TP-009 | manual | devguide `Documentation Conventions` 含命名+多語言政策+新鮮度；被索引/路標參照 | T-009 |
| TP-010 | manual | `docs/i18n/guide.md` 存在、不複製政策內文、連回 devguide 權威節 | T-010 |
| TP-011 | script | 新鮮度 check 對現有翻譯輸出 `(doc,lang)=current\|stale\|missing`；故意改 source 後該翻譯轉 `stale` | T-011 |
| TP-012 | script | `project.md` Source Documents 集合 == 重掃 `docs/**/*.md`（排除 `docs/plans/`）；Project Language 為 i18n 佈局 | T-012 |
| TP-013 | script | `structure-map.ndjson` 解析 + schema 驗證；每 `docs/**/*.md` 有 doc-section；排序 `axis`→`id` 穩定 | T-013 |
| TP-014 | script | `Sync-DevContext` 重生後再跑一次無 diff（idempotent）；轉接器反映新 project.md | T-014 |
| TP-015 | script | 全 repo 相對連結 + section-anchor 檢查 0 斷連（涵蓋 `docs/i18n/**`） | T-015 |

## 工作項目

> 任務依「執行模式」5 段 commit 切片分組；每片做完先過連結檢查再交使用者 commit。AI 不代為 commit。

**Slice 1 — 安全改名/移動（機械、低風險）**

- [x] T-001 — P1 去重：root `graphworkflow.md` → `collaborative-tools/blender-mcp.md`（H1 改 `Blender MCP`）；`collaborative-tools/graphworkflow.md` → `graphics-workflow.md`；更新全 repo 參照。
- [x] T-002 — P2 合併：`docs/xmachine/examples/` → `collaborative-tools/examples/`；移除空的 `docs/xmachine/`；更新參照。
- [x] T-003 — 改名/遷移（純機械）：`personalization.md`→`docs/manual.md`（僅改 stem）；建 `docs/i18n/zh-Hant/` 並遷入 `README.zh-Hant.md`、`personalization.zh-Hant.md`→`manual.zh-Hant.md`、`xmachine.zh-Hant.md`→`collaborative-tools/xmachine.zh-Hant.md`；更新所有 `personalization*`/翻譯參照。

**Slice 2 — devguide 合併（結構）**

- [x] T-004 — devguide 6-H2 重構：前置標註版 `Codebase`/`.gal` tree（吸收 Start-By-Finding-Layer / Where-Info-Belongs / Owning-Surfaces / Runtime-File-Schemas）、合併重疊節、刪 Quick Reference、dev-mode 連回 README；吸收 `installation-topology.md` 五塊後**刪原檔**。
- [x] T-005 — `release-matrix.md` 九節去編號 → devguide `Release Artifact Matrix`；**刪原檔**；section-anchor 重映（`release-matrix.md#…`/`installation-topology.md#…` → `devguide.md#…`）。

**Slice 3 — 內容改寫（最高風險，以 completed plans 為事實來源）**

- [ ] T-006 — README ≤400 壓縮地圖：語言切換 head、Quick Start(plugin)/Dev Mode/Install Status(deferred)/Distribution&Migration、How GAL Works compact 表委派 `commands.md`/`agents.md`/`workflows`、Files&Storage 標註 tree、Learn More；`docs/i18n/zh-Hant/README.zh-Hant.md` 鏡像。
- [ ] T-007 — `manual.md` 內容改寫為 plugin 模式操作手冊（First-Time Setup / Daily Use / Machine Ops 含 backup·migration·uninstall / Install Status），吸收 README 移入操作、委派架構給 devguide；`manual.zh-Hant.md` 鏡像。
- [ ] T-008 — `godot.md`/`graphics-workflow.md` 補 EN canonical（由 zh 翻譯，留原位）；原 zh → `docs/i18n/zh-Hant/collaborative-tools/<name>.zh-Hant.md` + 新鮮度 front-matter。

**Slice 4 — 索引/慣例/結構**

- [ ] T-009 — devguide `Documentation Conventions` 節：命名慣例（`-mcp`、技能名對齊、`README.md` 唯一 R5）+ 多語言翻譯政策（allowlist、`docs/i18n/<lang>/<name>.<lang>.md`、鏡像規則）+ 新鮮度機制描述。
- [ ] T-010 — 新增 `docs/i18n/guide.md`（薄路標：用途+範例+語言清單+加語言步驟+front-matter 模板+連回 devguide，不複製政策）；可選 `docs/index.md` 索引。
- [ ] T-011 — 翻譯新鮮度：為每個 `docs/i18n/**` 翻譯加 front-matter（`source`/`lang`/`source_commit`/`translated_at`）+ 建 check（掃 i18n、比對 source commit，報 `(doc,lang)=current|stale|missing`；遵雙 runtime PS/Bash 慣例）。
- [ ] T-012 — `.dev/project.md`：更新 Source Documents 索引（全 docs、`manual` 取代 personalization、無 topology/release-matrix）+ Project Language 慣例改 `docs/i18n/<lang>/<name>.<lang>.md`。
- [ ] T-013 — `docs/structure/structure-map.ndjson` 重掃：為每個 `docs/**/*.md`（含 i18n）產 doc-axis `doc-section` 節點、真實 `syncStatus`、排序穩定，未動行 byte 保留。

**Slice 5 — 轉接器重生（最後）**

- [ ] T-014 — 跑 `Sync-DevContext` 重生 `CLAUDE/GEMINI/AGENTS/copilot`（衍生物，不手改）。
- [ ] T-015 — 全 repo 相對連結 + section-anchor 檢查（涵蓋 `docs/i18n/**`），修正所有斷連（MISS-03 收尾）。
