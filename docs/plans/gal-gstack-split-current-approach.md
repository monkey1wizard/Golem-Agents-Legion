# GAL 與 gstack 拆分方案：目前已確定的作法

## 目的

這份文件只記錄目前已經確定的方向，用來作為後續重構與文件更新的共同基線。

這不是最終完整規格，但主要分層、輸出方向、workflow 家族、template 選型與 provider contract 已經拍板。後續討論應建立在這些前提上，而不是回到零開始。

## 核心結論

1. GAL 不會退位；它仍是整個系統的 kernel。
2. gstack 會被拆成 optional specialist provider，而不是核心依賴。
3. execution prompt 的最終路徑固定為 `.dev/plans/<plan-slug>.prompt.md`。
4. 不保留舊版相容性，也不設計 migration procedure。
5. GAL 內會有`planning`、`deep-planning`，以及獨立的 `plan-to-prompt` 指令。
6. architect、designer、analyst 會朝向較籠統、可對話、無 gstack 時可用的 fallback 能力發展。
7. visual 與 experience design、business-analyst template 採 gstack-style format。
8. formal gate / verdict contract 與 code architecture template 採 GSD-style format。
9. gstack 的支援安裝模式固定採 Other AI Agents 方式，也就是 source checkout 放在 `~/gstack` 後再執行 `./setup`。
10. gstack provider 的 project-scoped contract 以 `~/.gstack/projects/<slug>/` 為核心；`/gal` 主要檢查 `<branch>-reviews.jsonl` 與少數已明確定義的輸出目錄。

## 分層與角色

### GAL

GAL 持續擁有以下責任：

- `/gal` control plane
- repo-local project files
- 多 runtime 共用的 workflow contract
- 多機器協作與 remote worker 收斂模型
- plan lifecycle 與 pipeline lifecycle
- specialist 結果的 write-back 規則
- 沒有 gstack 時的對話型 fallback 架構

也就是說，GAL 仍負責 repo-local 主要檔案的最終狀態。即使沒有 gstack，GAL 也必須能獨立運作。

### gstack

gstack 的角色是可插拔的 specialist provider，不是控制平面，也不負責主要檔案的最終狀態。

GAL 目前只把它視為：

- planning review 的內容生成能力來源
- design review 的內容生成能力來源
- CEO-style 或 business-scope review 的內容生成能力來源
- 其他快速演化 specialist workflow 的外部提供者

不論內容由誰產生，最終都必須回寫到 GAL 自己的主要檔案與計畫檔案。

### Domain Agents

architect、designer、analyst 三個 domain agents 的定位，不是要達到與 gstack 同等的精度、速度與工具深度。

目前確定的角色是：

- 做較籠統的工作
- 偏對話型，可在 chat 中完成
- 沒有 gstack 時，能完成相近類型的工作
- 輸出比 gstack 粗、較慢，結構化輔助能力也較少

它們比較接近 GAL-native fallback 與對話型專業助手，而不是 gstack 的一比一替代品。

## Workflow 與檔案模型

### Source Plan 與 Execution Prompt

source plan 與 execution prompt 持續分離，並由 GAL 自己持有。

- `docs/plans/<plan-slug>.md`：human-readable source plan
- `.dev/plans/<plan-slug>.prompt.md`：mutable execution state

其中：

- source plan 給人閱讀與理解範圍、理由、需求
- execution prompt 是 pipeline 與 specialist write-back 的主要工作檔

### Workflow 家族

GAL 內部至少有三個由指令明確觸發的 workflow / command family：

| Workflow / Command Family | 主要輸入 | 正式輸出 |
| --- | --- | --- |
| `planning` | 使用者需求、對話上下文 | `docs/plans/<plan-slug>.md` |
| `deep-planning` | 任何規劃中的文字文件，包含既有 plan、gstack plan、research 結果與其他中間文件 | `docs/plans/<plan-slug>.md` |
| `plan-to-prompt` | `docs/plans/<plan-slug>.md` | `.dev/plans/<plan-slug>.prompt.md` |

另外：

- `planning` 會吸收少量 discovery 方法，但不直接暴露 gstack-style command surface
- `deep-planning` 專門負責反覆審查、拆 plan、細分架構、任務拆解、review 準備
- `deep-planning` 的輸入不限制於 plan 檔；只要是規劃中的文字文件，都可以拿來收斂成正式 plan
- `plan-to-prompt` 是獨立指令，不混在`planning`或 `deep-planning` 內

`planning`與 `deep-planning` 都屬於明確指令觸發，不需要額外定義隱式自動切換邊界。

## Template 與 Contract 選型

不同 lane 不需要全部沿用同一個上游來源。

### 採 gstack-style 的部分

- visual 與 experience design template
- business-analyst template

這代表：

- design specialist workflow 優先對接 gstack-style 的設計輸出結構
- business scope、產品價值、premise challenge 一類的輸出骨架，優先沿用 gstack-style specialist output

### 採 GSD-style 的部分

- formal gate / verdict contract
- code architecture template

這代表：

- GAL 不採 gstack 的 review report 格式作為 kernel contract
- GAL 使用較乾淨、較 provider-neutral 的 GSD-style gate 與 verdict 邏輯
- 架構分析、component responsibilities、data flow、recommended structure、anti-patterns 等內容，以 GSD-style architecture template 為正式基底

gstack 的 engineering review 可以提供內容，但不作為 GAL 正式架構文件的格式來源。

## Formal Workflow 路由原則

provider 選擇發生在 workflow 層，不直接綁在單一 agent 身上。

目前已確定的原則是：

- 不做成「agent 自己偵測到 gstack 就切換人格或流程」
- 應做成「GAL 的 formal workflow 先選 provider，再由 provider 產出內容」
- 產出完成後，仍由 GAL 的 write-back contract 接住結果並寫回指定 sections

這個原則主要適用於：

- engineering review
- design review
- business or CEO-style review

也就是說，GAL 的 planning family 應使用 review-lane / capability wording，而不是直接把 upstream gstack skill 名稱暴露成 user-facing next steps。

## gstack Provider Contract

### 偵測原則

不能把 `.gstack` 目錄是否存在，當作唯一或主要的安裝判斷訊號。

原因是：

- `.gstack` 可能只是 repo-local output scope
- gstack 本身還有獨立於 `docs/` 的生成結果資料夾
- 檔案是否存在，不等於 workflow provider 是否可用

因此 `/gal` 應把「是否安裝 gstack」與「這次 workflow 是否有足夠 provider files 可用」視為兩層不同判斷。

### 安裝 contract

GAL 對 gstack 只承認一種支援的安裝模式：Other AI Agents 模式。

也就是：

```bash
git clone --single-branch --depth 1 https://github.com/garrytan/gstack.git ~/gstack
cd ~/gstack && ./setup
```

這代表 GAL 不需要再兼容 gstack 的其他歷史安裝形態作為正式 contract，例如 vendored copy、repo-local `.claude/skills/gstack`、或其他 host-specific install layout。

`/gal` 在做安裝存在性檢查時，應優先判斷這個 source checkout 是否存在且可用，而不是先看 `~/.gstack/`。

### 路徑正規化

文件中的 `~` 一律代表目前使用者的 home directory，不代表固定字串路徑。

各平台應對應為：

- macOS / Linux：`~/gstack` 與 `~/.gstack`
- Windows：`%USERPROFILE%\gstack` 與 `%USERPROFILE%\.gstack`

因此在 Windows 上，概念上通常會落在 `C:\Users\<username>\` 底下，但 GAL 實作時不應硬編碼 `C:\Users\{username}`，而應透過使用者 home 的環境變數或等價 API 解析。

### 安裝存在性檢查

在目前 contract 下，`/gal` 可用以下條件判斷 gstack 是否已安裝：

- `~/gstack/` 是否存在
- `~/gstack/setup` 是否存在
- `~/gstack/bin/` 是否存在

在 Windows 上，以上三項分別對應到 `%USERPROFILE%\gstack\...`。

這一層只回答「機器上是否有受支援的 gstack 安裝」，不直接回答「目前 repo / branch 是否已有可採用的 formal review files」。

### project-scoped contract

GAL 不需要吃下 gstack repo 裡所有歷史性或周邊輸出，只承認一組最小且可驗證的 project-scoped 結構：

```text
~/.gstack/projects/<slug>/
├── <branch>-reviews.jsonl
├── ceo-plans/
├── checkpoints/
├── designs/
├── evals/
└── learnings.jsonl
```

各部分用途如下：

- `<branch>-reviews.jsonl`：branch-scoped 的 formal review state 主來源
- `ceo-plans/`：CEO-style 或 business-scope specialist output
- `checkpoints/`：跨 session 的中間收斂點與 context recovery file
- `designs/`：design exploration、approved choice、design audit、finalized output 的 project-scoped 容器
- `evals/`：gstack 自己的 eval output；GAL 可知道其存在，但不把它當成 planning gate 的必要前置
- `learnings.jsonl`：project-scoped learnings；屬於輔助訊號，不是 formal gate 主來源

### `designs/` 目錄

GAL 不需要理解所有 design binary 細節，但需要承認以下檔案類型：

```text
~/.gstack/projects/<slug>/designs/
├── <screen-or-session>/
│   ├── approved.json
│   ├── finalized.html
│   ├── finalized.json
│   ├── variant-*.png
│   └── ...
└── design-audit-<YYYYMMDD>/
  ├── design-audit-<domain>.md
  ├── design-baseline.json
  └── screenshots/
```

另外，gstack source 也有 branch-scoped 的 top-level filename 慣例，例如：

- `*-design-*.md`
- `*-$BRANCH-ceo-handoff-*.md`
- `*-design-audit-*.md`

GAL 可以讀這些檔案作為 context，但不把它們當作唯一主索引。真正給 `/gal` 做狀態判斷的主入口，仍然是 `<branch>-reviews.jsonl` 與上面幾個明確子目錄。

### provider file 檢查欄位

`/gal` 對 gstack provider 的檢查，採「先看目錄，再看 branch review log，再看檔案輔助訊號」的順序。

#### 基本存在性

- `~/.gstack/projects/<slug>/` 是否存在
- `~/.gstack/projects/<slug>/<branch>-reviews.jsonl` 是否存在

若不存在 review log，視為 provider files 不足，不視為已完成 formal review。

#### review log 共通欄位

對所有 review entries，`/gal` 至少檢查：

- `skill`
- `timestamp`
- `status`
- `commit`

其中：

- `timestamp` 用於新鮮度判斷
- `commit` 用於和目前 HEAD 比對，避免誤用過期 review
- `status` 是 provider 原始狀態，GAL 之後再正規化為自己的正式 gate 狀態

#### engineering review

當 entry 的 `skill` 是 upstream engineering review provider 或 `review` 時，`/gal` 檢查：

- `status`
- `unresolved`
- `critical_gaps`
- `issues_found`
- `mode`
- `commit`

其中 upstream engineering review provider 是 planning review lane 的主要來源；`review` 可作為已實作分支的補充訊號，但不取代 planning files。

#### design review

當 entry 的 `skill` 是 upstream design review provider 時，`/gal` 檢查：

- `status`
- `initial_score`
- `overall_score`
- `unresolved`
- `decisions_made`
- `commit`

#### business / CEO-style review

當 entry 的 `skill` 是 upstream business review provider 時，`/gal` 檢查：

- `status`
- `unresolved`
- `critical_gaps`
- `mode`
- `scope_proposed`
- `scope_accepted`
- `scope_deferred`
- `commit`

#### design file 輔助訊號

在 design lane，`/gal` 可額外檢查：

- `designs/*/approved.json` 是否存在
- `designs/*/finalized.html` 是否存在
- `designs/design-audit-*/design-baseline.json` 是否存在

這些訊號只用來幫助判斷 design workflow 曾經走到哪個階段，不直接取代 formal review log。

#### 非 gate 類輔助訊號

以下項目可讀，但不直接作為 gate：

- `learnings.jsonl`
- `evals/`
- `checkpoints/` 內的 markdown file
- `ceo-plans/` 內的原始 markdown file

它們的用途是 context recovery、補充判讀與 downstream prompt enrichment，而不是正式 verdict 來源。

## Domain Agents 檔案落點

目前已確定三個 domain agents 的正式輸出落點如下：

| Agent | 輸出落點 | 正式格式方向 |
| --- | --- | --- |
| architect | plan 內 | GSD-style code architecture template |
| designer | `docs/design/`，必要時同步更新 `DESIGN.md` | gstack-style visual / experience design template |
| analyst | `docs/research/` | gstack-style business-analyst template |

也就是說，三個 domain agents 雖然在互動方式上偏對話型 fallback，但一旦需要落成正式輸出，仍有固定的 template 與欄位骨架可依附。

其中 designer lane 的結構進一步固定為：

- `docs/design/`：存放多個 design 修改書、設計提案、補充說明等 supporting docs
- `DESIGN.md`：repo-level 的 design 主檔與治理文件
- 並不是每次 designer 輸出都必須改動 `DESIGN.md`；只有在設計系統主張、token、原則或 repo-level baseline 真的改變時才需要同步更新

## 遷移原則

後續重構時，應遵守以下原則：

- 直接以新 output path 與新 workflow contract 為準，不為舊版保留相容層
- 舊文件只作為參考，不作為必須被轉換的正式輸入
- 先把 provider routing 抽出來，再討論各 agent 的最終細節
- 先把 GAL-native 的`planning`與 `deep-planning` 建立起來，再決定 gstack 如何接入 formal review
- 任何委派給 gstack 的結果，都必須能無損回寫到 GAL 的主要檔案與計畫檔案

## 目前狀態

目前沒有剩餘的高層未定案項目；後續只剩實作層細節。
