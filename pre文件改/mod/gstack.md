# gstack 可選 provider 模組

gstack 是 GAL 的可插拔 specialist provider，不是 GAL 的核心依賴。GAL 有 `/planning`、`/deep-planning` 作為 GAL-native 規劃入口，並在 implementation 前用 `/plan-to-prompt` 將 reviewed source plan 轉成 execution prompt；architect、analyst、designer 提供 GAL-native 的對話型 fallback。沒有 gstack 時，GAL 仍能完整運作。

gstack 安裝後（`~/gstack`），這個模組主要提供兩層增強能力：

1. 進階規劃 provider：discovery-style feature planning、business/design/engineering review lanes，以及 full planning review pipeline provider。這些 review lanes 屬於 planning 之後、`/plan-to-prompt` 之前的 specialized deep-planning passes。這層是可選增強，不是唯一規劃路徑。
2. Specialist 執行指令：`/review`、`/qa`、`/ship`、`/design-review` 等指令遵循 write-back discipline，把結果回寫到 GAL 的主要檔案與計畫區段。這些指令不依賴規劃層。

另外要分清楚兩層偵測：

- 安裝存在性：機器上是否有受支援的 gstack 安裝（例如 `~/gstack`）。
- provider artifact readiness：目前 repo / branch 是否已有足夠的 review artifacts 可供 formal workflow 採用。

GAL 需要兩層都分開判斷；不能用 `.gstack` 或單一 artifact 存在與否，去取代完整的 provider contract 判斷。

## GAL 與 gstack 的關係

GAL 取用的是 gstack 的 workflow semantics，作為 optional provider 接入，而不是依賴 upstream runtime 作為核心依賴。

- gstack 的 host 假設是 Claude Code；GAL 的主控制面是 Copilot，Gemini CLI 與 Codex 是其他可接入 runtime。
- GAL 要求 repo-local project files；upstream gstack 的部分資料模型是 user-global。
- GAL 的目標是 tool-agnostic methodology，而不是把控制權外包給某個外部工具安裝。

做法是：維持 GAL-native 的 planning 與 execution command surface，並以 provider routing 讓 gstack 的規劃與審查能力在有安裝時接入 GAL 的主要檔案。

provider routing 發生在 workflow 層，不直接綁在單一 agent 身上。也就是說，GAL 不做成「agent 偵測到 gstack 就切換人格或流程」，而是先由 formal workflow 決定這次要走 upstream provider 還是 GAL-native fallback，再把結果回寫到同一組固定區段。

記住三件事：

1. `/gal` 只處理 control plane，不重複包一層 specialist workflow。
2. Specialist commands 直接執行工作，但必須回寫到 `/gal` 讀得懂的主要檔案與計畫區段。
3. GAL 採 repo-local state model，不使用 upstream gstack 的 user-global storage。

## 規劃工作流

> **注意**：本節說的是 gstack 風格的進階規劃能力如何接入 GAL。GAL 有 `/planning` 與 `/deep-planning` 作為 GAL-native 規劃入口，適合所有 risk weight 的情境。business / design / engineering review lanes 應被視為 specialized deep-planning passes，發生在 source plan 完成初稿之後、`/plan-to-prompt` 之前。provider 增強層則適合需要高品質 business, design, engineering review 的 Strategic-weight features。

### 核心前提

gstack 風格流程圍繞一個前提運作：

> **Design doc 是 per-feature，不是 per-product。**
> 它捕捉的是「這次具體變更背後的思考」，不是整個產品的完整規格。

這代表：

- 每次 discovery-style 規劃產出的 plan 應該對應一個可獨立交付的 feature。
- 如果你的 plan 涵蓋了整個產品，那它是 roadmap，不是 executable plan。
- 把 roadmap 拆成可執行的 feature plans 是人的責任，不是工具的責任。

### 流程圖

```text
Roadmap / product-level idea
    |
    v
人工挑出一個要先做的 feature
    |
    v
discovery-style planning provider or /planning
    output: 該 feature 的 source plan
    |
    v
選擇審查路徑
    |
    +--> 路徑 A: full planning review provider
    |      Business -> Design -> Engineering 依序審查
    |
    `--> 路徑 B: 手動逐步審查
                 1. business / scope review lane
                 2. design review lane
                 3. engineering review lane

兩條路徑都會收斂到:
    reviewed plan
    + task list
    + worktree parallelization strategy
    |
    v
/plan-to-prompt
    output: execution prompt
    |
    v
逐 task 實作
    - /gal-pipeline
    - 或手動執行
    |
    v
/review
    |
    v
/qa
    |
    v
/ship
    |
    v
回到 roadmap，人工挑下一個 feature
```

### 階段說明

#### 1. 人工拆分

provider 不會把一個 product roadmap 自動拆成多個 feature plan。若一份大文件同時涵蓋多個模組，正確做法是：

1. 把這份文件當作 roadmap 參考。
2. 先決定下一個要做的單一 feature。
3. 只針對那個 feature 產出 source plan。

每個 feature 應對應獨立 plan file、獨立 session、獨立 worktree。

#### 2. Discovery-style 規劃

這類 provider 擅長：

- 挑戰前提假設
- 問清楚目標使用者和核心問題
- 產出可供後續 deep-planning / review lanes 消費的 source plan

若你不需要這種強互動 discovery，也可以直接使用 `/planning` 或 `/deep-planning`。

#### 3. 審查管線

有兩種常見路徑：

- 自動路徑：full planning review provider 依序跑 business、design、engineering review。
- 手動路徑：分別跑三條 planning-stage review lanes，每步都保留人工介入空間。

template 與 contract 也刻意分流：visual / experience design 與 business scope 的輸出骨架，優先沿用 gstack-style specialist artifact；formal gate / verdict contract 與 code architecture 則保留 GSD-style。GAL 接的是 upstream 能力，不是整套上游格式的全盤繼承。

在 GAL 的語意中，這三條 review lanes 都屬於 planning 之後、implementation 之前的深度規劃環節。換句話說，它們不是 execution prompt 上的後續附加工序，而是 source plan 變成 implementation-ready plan 之前的 specialized deep-planning passes。

其中 engineering review lane 會補齊 build readiness、test matrix、task breakdown 與 parallelization strategy。它拆的是「一個 feature 內的實作步驟」，不是把一個大 plan 拆成多個 plan。這些內容應先寫回 source plan，再由 `/plan-to-prompt` 一次 materialize 到 execution prompt。

#### 4. Materialize 與實作

當 source plan 完成上述 deep-planning / review lanes 後，再執行 `/plan-to-prompt`。這一步不是做 planning review，而是把 reviewed source plan 轉成 execution prompt，讓後續 `/gal pipeline`、`/qa`、`/review` 等 stateful specialist flows 有穩定的機器可讀工作檔。

#### 5. 實作與收尾

審查完成後，plan 內會有具體 task list。之後可用 `/gal pipeline` 自動逐 task 執行，或手動一個一個做。

每個 task: implement → test → review → commit。

完成後再進入 `/review`、`/qa`、`/ship`、`/land-and-deploy` 這些 write-back specialist flows。

### 平行工作模型

gstack 支援多個平行 sprint，但前提仍是 feature 粒度清楚：

- 每個 sprint = 獨立的 feature
- 每個 feature = 獨立的 plan file
- 每個 feature = 獨立的 session / worktree
- 各自走完整個規劃、實作、審查、發佈流程

## Specialist 指令與能力參考

所有 specialist commands 直接執行工作。不需要先透過 `/gal` 才能呼叫，但輸出必須符合 `/gal` 讀取的 plan write-back contract。

### GAL-native 規劃入口

| 指令 | 用途 | 主要寫回 |
| --- | --- | --- |
| `/planning` | 從使用者需求與對話上下文產出 source plan | `docs/plans/<plan-slug>.md` |
| `/deep-planning` | 深化任何規劃文件為正式 plan | `docs/plans/<plan-slug>.md` |
| `/plan-to-prompt` | 將 reviewed source plan 轉換為 execution prompt | `.dev/plans/<plan-slug>.prompt.md` |

### Planning-stage review lanes

這些是 capability，不是 GAL public command 名稱。在實務上，它們可視為 `/deep-planning` 的 specialized passes。

| Lane | 用途 | 主要寫回 |
| --- | --- | --- |
| Business / Scope review | 調整 scope、ambition、價值排序 | source plan `## Review Results`、`## Open Questions` |
| Design review | 補齊 UX、state coverage、a11y、design-system fit | source plan `## Review Results`、`## Open Questions` |
| Engineering review | 補齊 architecture、test plan、tasks | source plan `## Review Results`、`## Test Plan`、`## Tasks` |
| Security review | 補齊 OWASP / STRIDE findings | plan `## Review Results`（security review） |

### 設計

| 指令 | 用途 | 主要寫回 |
| --- | --- | --- |
| `/design-consultation` | 建立 repo-level design system | `DESIGN.md`、`CLAUDE.md` |
| `/design-shotgun` | 生成多個視覺方向並核可一個變體 | `docs/designs/<plan-slug>/variant-approved.json` |
| `/design-html` | 核可 mockup 轉成可執行 HTML 或元件實作 | `docs/designs/<plan-slug>/handoff-final.html` |
| `/design-review` | 對照 `DESIGN.md` 進行 post-implementation 設計修正 | plan `## Review Results`、`docs/design-reports/` |

### 除錯與審查

| 指令 | 用途 | 主要寫回 |
| --- | --- | --- |
| `/investigate` | 根因優先的除錯流程，自動搭配 `/freeze` | plan debug session 或工作紀錄 |
| `/review` | staff engineer 等級的 diff 審查 | plan `## Review Results`、`## Analyze` |
| `/cso` | 資安審查與 findings 管理 | plan `## Review Results` 下的 security review |

### 瀏覽器與 QA

| 指令 | 用途 | 主要寫回 |
| --- | --- | --- |
| `/browse` | 提供其他流程使用的 browser primitive | session only |
| `/connect-chrome` | 切換成 headed Chrome | session only |
| `/setup-browser-cookies` | 將認證匯入 browser session | session only |
| `/qa` | 跑 test plan、修 bug、寫回 test results | plan `## Test Results`、`docs/qa-reports/` |
| `/qa-only` | 只做 bug report，不修改程式碼 | plan `## Test Results (Report Only)`、`docs/qa-reports/` |

### 發佈

| 指令 | 用途 | 主要寫回 |
| --- | --- | --- |
| `/ship` | merge 前最終關卡：測試、PR、文件 | plan `## Ship` |
| `/land-and-deploy` | 合併並驗證部署 | plan `## Deploy` |
| `/setup-deploy` | 建立 deploy config baseline | `CLAUDE.md` |
| `/document-release` | 同步已發佈程式碼與文件 | repo docs、PR 補充內容 |

### 記憶與守護

| 指令 | 用途 | 主要寫回 |
| --- | --- | --- |
| `/learn` | repo 本地制度化記憶管理 | `.dev/learnings.jsonl` |
| `/careful` | 破壞性操作前警告 | session only |
| `/freeze` | 限制編輯範圍 | session only |
| `/guard` | 同時啟用 careful 與 freeze | session only |
| `/unfreeze` | 解除 freeze 邊界 | session only |
| `/gstack-upgrade` | 委派到 upstream gstack 的升級 shim | local machine maintenance |

每個指令與 lane 的精確 reads / writes / plan sections 見 [../../docs/gstack-command-contracts.md](../../docs/gstack-command-contracts.md)。

## Upstream gstack 語意映射

這裡的「映射」不是宣稱 GAL 與 upstream gstack 完全等價，而是說使用者能在 GAL 中找到對應的 specialist workflow public surface，同時由 GAL 自己負責 state ownership、runtime 佈局與 control-plane 問題。

### 能力對照

| 能力 | upstream gstack 語意 | GAL 的實作差異 |
| --- | --- | --- |
| Discovery-style feature planning | per-feature design doc 啟動 | plan 寫入 `docs/plans/`，不是 `~/.gstack/projects/` |
| Engineering review lane | 產出 task list 與 test plan | 先寫回 source plan 的 `## Test Plan` 與 `## Tasks`，再由 `/plan-to-prompt` 轉入 execution prompt |
| `/review` | staff diff review | `## Analyze` 成為 control plane 可讀的 drift verdict |
| `/qa` | 執行 test plan 並回報結果 | `## Test Results` + `docs/qa-reports/` |
| `/design-shotgun` | 視覺方向探索 | 資產路徑在 `docs/designs/` |
| `/learn` | session / sprint learnings | 儲存在 `.dev/learnings.jsonl` |

### Artifact 路徑對照

| 用途 | upstream gstack | GAL |
| --- | --- | --- |
| source plan | `~/.gstack/projects/$SLUG/design.md` | `docs/plans/<plan-slug>.md` |
| execution prompt | 無明確對等 | `.dev/plans/<plan-slug>.prompt.md` |
| design variants | `~/.gstack/projects/$SLUG/designs/` | `docs/designs/<plan-slug>/` |
| QA reports | `.gstack/qa-reports/` | `docs/qa-reports/` |
| design reports | `.gstack/design-reports/` | `docs/design-reports/` |
| sprint learnings | `~/.gstack/projects/$SLUG/learnings.jsonl` | `.dev/learnings.jsonl` |

## Runtime 角色

| Runtime | 定位 |
| --- | --- |
| Copilot | 主控制面，擅長互動式 orchestration、review 對話與 plan refinement |
| Gemini CLI | worker runtime，適合跑 bounded specialist tasks |
| Codex CLI | 與 Copilot / Gemini 共用同一套 artifact contract，但用 `$` 作為 command 入口 |

這三者共享的是 artifacts 與 contracts，不是完全對稱的 host 能力。

## 相關文件

- [readme.zh-Hant.md](../readme.zh-Hant.md) — GAL 使用者入口
- [command-index.md](../command-index.md) — control plane 與 specialist commands 索引
- [../../docs/gstack-integration.md](../../docs/gstack-integration.md) — gstack provider 偵測、artifact readiness 與 project-scoped contract
- [../../docs/gstack-command-contracts.md](../../docs/gstack-command-contracts.md) — 每個 command 與 lane 的精確 reads / writes / plan sections
- [../../docs/gstack-workflow-guide.md](../../docs/gstack-workflow-guide.md) — upstream gstack workflow reference

## 深入閱讀

| 如果你要理解 | 讀這份 |
| --- | --- |
| 為什麼 GAL 不直接依賴 gstack | [../../docs/gstack-integration.md](../../docs/gstack-integration.md) |
| 每個 command 與 lane 的精確 reads / writes | [../../docs/gstack-command-contracts.md](../../docs/gstack-command-contracts.md) |
| feature splitting 與 workflow 教學 | [../../docs/gstack-workflow-guide.md](../../docs/gstack-workflow-guide.md) |
| `/gal` control plane 合約 | [../../docs/gal-control-plane-contracts.md](../../docs/gal-control-plane-contracts.md) |
| dispatch、alias 與 script architecture | [../../docs/command-dispatch-architecture.md](../../docs/command-dispatch-architecture.md) |
| runtime 安裝與 command surface 佈局 | [../../commands/commands.md](../../commands/commands.md)、[../../docs/installation-topology.md](../../docs/installation-topology.md) |
