# gstack 可選 provider 模組

gstack 是 GAL 的可插拔 specialist provider，不是 GAL 的核心依賴。GAL 有 `/planning`、`/deep-planning`、`/plan-to-prompt` 作為 GAL-native 規劃入口；architect、analyst、designer 提供 GAL-native 的對話型 fallback。沒有 gstack 時，GAL 仍能完整運作。

gstack 安裝後（`~/gstack`），這個模組提供兩層增強能力：

1. **進階規劃 on-ramp**（可選）：`/office-hours`、`/autoplan`、`/plan-*-review` 提供 gstack 風格的結構化 feature 規劃流程，產出高品質的 plan artifacts。這層不是唯一規劃路徑——GAL-native 的 `/planning`、`/deep-planning` 不依賴此層。
2. **Specialist 執行指令**：`/review`、`/qa`、`/ship` 等指令遵循 write-back discipline，把結果回寫到 GAL 的 canonical artifacts。這些指令不依賴規劃層。

## GAL 與 gstack 的關係

GAL 取用的是 gstack 的 workflow semantics，作為 optional provider 接入，而不是依賴 upstream runtime 作為核心依賴。

- gstack 的 host 假設是 Claude Code；GAL 的主控制面是 Copilot，Gemini CLI 與 Codex 是其他可接入 runtime。
- GAL 要求 repo-local canonical artifacts；upstream gstack 的部分資料模型是 user-global。
- GAL 的目標是 tool-agnostic methodology，而不是把控制權外包給某個外部工具安裝。

做法是：生成 GAL-native 的 planning 與 execution commands（`/planning`、`/deep-planning`、`/plan-to-prompt` 等），並以 provider routing 讓 gstack 的 specialist review artifact 在有安裝時接入 GAL 的 canonical artifacts。

記住三件事：

1. `/gal` 只處理 control plane，不重複包一層 specialist workflow。
2. Specialist commands 直接執行工作，但必須回寫到 `/gal` 讀得懂的 canonical artifacts。
3. GAL 採 repo-local state model，不使用 upstream gstack 的 user-global storage。

## 規劃工作流

> **注意**：本節說的是 gstack 風格的進階規劃流程。GAL 有 `/planning` 與 `/deep-planning` 作為 GAL-native 規劃入口，適合所有 risk weight 的情境。本節的 gstack 流程適合需要高品質 CEO / Design / Eng review 的 Strategic-weight features。

### 核心前提

gstack 的所有命令都圍繞一個前提運作：

> **Design doc 是 per-feature，不是 per-product。**
> 它捕捉的是「這次具體變更背後的思考」，不是整個產品的完整規格。

這代表：

- 每次 `/office-hours` 產出的 plan 應該對應一個可獨立交付的 feature
- 如果你的 plan 涵蓋了整個產品，那它是 roadmap，不是 executable plan
- 把 roadmap 拆成可執行的 feature plans 是人的責任，不是工具的責任

### 流程圖

```text
Roadmap / product-level idea
    |
    v
人工挑出一個要先做的 feature
    |
    v
/office-hours
    output: 該 feature 的 design doc
    |
    v
選擇審查路徑
    |
    +--> 路徑 A: /autoplan
    |      CEO -> Design -> Eng 依序自動審查
    |
    `--> 路徑 B: 手動逐步審查
                 1. /plan-ceo-review
                 2. /plan-design-review
                 3. /plan-eng-review

兩條路徑都會收斂到:
    reviewed plan
    + task list
    + worktree parallelization strategy
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

#### 1. 人工拆分 — 流程的起點

gstack 沒有「大 plan 自動拆成小 plan」的機制。如果你做了一次 `/office-hours` 然後得到一個涵蓋 10 個模組的大 plan，正確做法是：

1. 把這個 plan 當作 roadmap 參考
2. 自己決定先做哪個 feature
3. 對每個 feature 分別跑 `/office-hours`

每個 feature 獨立一個 plan file、獨立一條 session。

#### 2. /office-hours — 產出 design doc

`/office-hours` 模擬 YC office hours 風格的對話。它會：

- 挑戰你的前提假設
- 問清楚目標使用者和核心問題
- 產出一份 design doc

這份 design doc 就是後續所有審查和實作的基礎。

#### 3. 審查管線 — 兩種路徑

##### 路徑 A: /autoplan（自動化）

依序執行 CEO → Design → Eng review，自動做出大部分決策，只在真正需要人類判斷的品味問題才停下來。適合有信心的 feature。

##### 路徑 B: 手動逐步審查

分開執行三個審查命令，每步都有機會介入調整。

###### /plan-ceo-review — 商業方向

有四種模式，由 AI 自動判斷：

| 模式 | 說明 |
| --- | --- |
| EXPANSION | 10-star vision，放大野心 |
| SELECTIVE EXPANSION | 保留核心，在特定面向加碼 |
| HOLD SCOPE | 範圍正確，只做 polish |
| SCOPE REDUCTION | 範圍太大，必須刪東西 |

這四種模式都是在調整「同一個 plan」的範圍，不是產出新 plan。

###### /plan-design-review — 設計審查

檢查 UX flow、accessibility、設計系統一致性。

###### /plan-eng-review — 工程審查

這是最關鍵的審查步驟，產出：

- **Scope Challenge**: 超過 8 個檔案或 2 個新 class 就建議縮減
- **Architecture / Code Quality / Tests / Performance** 四大面向的 review
- **Worktree Parallelization Strategy**: 分析 task 之間的依賴關係，產出可平行執行的 lane

Worktree Parallelization 拆的是「一個 feature 內的實作步驟」，不是「把一個大 plan 拆成多個 plan」。

#### 4. 實作 — 逐 task 執行

審查完成後 plan 內會有具體的 task list。用 `/gal pipeline` 自動逐 task 執行，或手動一個一個做。

每個 task: implement → test → review → commit。

#### 5. 收尾 — review → qa → ship

- `/review`: staff engineer 等級的 diff 審查
- `/qa`: 在真實瀏覽器中跑 test plan
- `/ship`: 推分支、開 PR、觸發 `/document-release`

### 實際範例：labyrinth

以製作 labyrinth 為例，假設你已經透過一次 `/office-hours` 得到一份很大的 plan，內容同時包含：

- 迷宮生成
- 玩家移動與視角
- 鑰匙 / 門 / 關卡進程
- 敵人與戰鬥
- 介面與教學提示

這份文件可能方向正確，但性質比較接近 product roadmap，不是可以直接丟進 gstack 流程的單一 feature plan。

正確做法是人工先拆 feature，再讓每個 feature 各自走完整條流程：

1. 迷宮生成最小可玩版本
2. 玩家移動、碰撞與第一人稱視角
3. 鑰匙、門與基本進程控制
4. 第一種敵人與最小戰鬥循環
5. HUD、提示與新手引導

然後逐一執行：

1. 選「迷宮生成最小可玩版本」
2. 只針對這個 feature 跑 `/office-hours`
3. 跑 `/autoplan` 或手動三步審查
4. 實作、`/review`、`/qa`、`/ship`
5. 完成後回到 roadmap，挑下一個 feature

大 plan 可以保留（有方向價值），但真正進入 gstack 流程的，永遠是「一個可獨立交付的 feature」。

### 平行工作模型

gstack 支援 10–15 個平行 sprint：

- 每個 sprint = 獨立的 feature
- 每個 feature = 獨立的 plan file
- 每個 feature = 獨立的 session / worktree
- 互不干擾，各自走完整個 `/office-hours` → `/ship` 流程

## Specialist 指令參考

所有 specialist commands 直接執行工作。不需要先透過 `/gal` 才能呼叫，但輸出必須符合 `/gal` 讀取的 plan write-back contract。

規劃類指令是可選的 on-ramp；其餘指令屬於 GAL 的核心執行層，無論 plan 怎麼產生都能運作。

### 規劃

| 指令 | 用途 | 主要寫回 |
| --- | --- | --- |
| `/office-hours` | 以單一 feature 為單位產出 plan doc 與 execution work file | `docs/plans/<plan-slug>.md`、`.prompt.md`、`## Open Questions` |
| `/plan-ceo-review` | 從創辦人視角調整 scope 與 ambition | plan `## Review Results`、`## Open Questions` |
| `/plan-design-review` | 實作前補齊 UX 與設計決策 | plan `## Review Results`、`## Open Questions` |
| `/plan-eng-review` | 補齊 architecture、test plan、tasks | plan `## Review Results`、`## Test Plan`、`## Tasks` |
| `/autoplan` | 串接 CEO → Design → Eng review 並自動決策 | 同上三個 review 的所有輸出 |
| `/cso` | OWASP + STRIDE 資安審查 | plan `## Review Results`（security review） |

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
| `/canary` | 部署後監控 | `docs/benchmarks/` 或部署備注 |
| `/benchmark` | 真實瀏覽器效能測量與基準比對 | `docs/benchmarks/` |
| `/setup-deploy` | 建立 deploy config baseline | `CLAUDE.md` |
| `/document-release` | 同步已發佈程式碼與文件 | repo docs、PR 補充內容 |
| `/retro` | 工程回顧與 snapshot | `docs/retros/` |

### 記憶與守護

| 指令 | 用途 | 主要寫回 |
| --- | --- | --- |
| `/learn` | repo 本地制度化記憶管理 | `.dev/learnings.jsonl` |
| `/careful` | 破壞性操作前警告 | session only |
| `/freeze` | 限制編輯範圍 | session only |
| `/guard` | 同時啟用 careful 與 freeze | session only |
| `/unfreeze` | 解除 freeze 邊界 | session only |
| `/gstack-upgrade` | 更新 GAL command/skill 安裝 | local machine maintenance |

每個指令的精確 reads / writes / plan sections 見 [../docs/gstack-command-contracts.md](../../docs/gstack-command-contracts.md)。

## Upstream gstack 語意映射

這裡的「映射」不是宣稱 GAL 與 upstream gstack 完全等價，而是說使用者能在 GAL 中找到對應的 specialist workflow public surface，同時由 GAL 自己負責 state ownership、runtime 佈局與 control-plane 問題。

### 指令對照

| 指令 | upstream gstack 語意 | GAL 的實作差異 |
| --- | --- | --- |
| `/office-hours` | per-feature design doc 啟動 | plan 寫入 `docs/plans/`，不是 `~/.gstack/projects/` |
| `/plan-eng-review` | 產出 task list 與 test plan | `## Test Plan` 與 `## Tasks` 寫回活動 plan |
| `/review` | staff diff review | `## Analyze` 成為 control plane 可讀的 drift verdict |
| `/qa` | 執行 test plan 並回報結果 | `## Test Results` + `docs/qa-reports/` |
| `/design-shotgun` | 視覺方向探索 | 資產路徑在 `docs/designs/` |
| `/benchmark` | performance baseline | baseline 寫入 `docs/benchmarks/` |
| `/learn` | session / sprint learnings | 儲存在 `.dev/learnings.jsonl` |

### Artifact 路徑對照

| 用途 | upstream gstack | GAL |
| --- | --- | --- |
| source plan | `~/.gstack/projects/$SLUG/design.md` | `docs/plans/<plan-slug>.md` |
| execution memory | 無明確對等 | `docs/plans/<plan-slug>.prompt.md` |
| design variants | `~/.gstack/projects/$SLUG/designs/` | `docs/designs/<plan-slug>/` |
| QA reports | `.gstack/qa-reports/` | `docs/qa-reports/` |
| design reports | `.gstack/design-reports/` | `docs/design-reports/` |
| benchmark baselines | 無明確對等 | `docs/benchmarks/` |
| sprint learnings | `~/.gstack/projects/$SLUG/learnings.jsonl` | `.dev/learnings.jsonl` |

## Runtime 角色

| Runtime | 定位 |
| --- | --- |
| Copilot | 主控制面，擅長互動式 orchestration、review 對話與 plan refinement |
| Gemini CLI | worker runtime，適合跑 bounded specialist tasks |
| Codex CLI | 與 Copilot / Gemini 共用同一套 artifact contract，但用 `$` 作為 command 入口 |

這三者共享的是 artifacts 與 contracts，不是完全對稱的 host 能力。

## 深入閱讀

| 如果你要理解 | 讀這份 |
| --- | --- |
| 為什麼 GAL 不直接依賴 gstack | [../../docs/gstack-integration.md](../../docs/gstack-integration.md) |
| 每個 command 的精確 reads / writes | [../../docs/gstack-command-contracts.md](../../docs/gstack-command-contracts.md) |
| feature splitting 與 workflow 教學 | [../../docs/gstack-workflow-guide.md](../../docs/gstack-workflow-guide.md) |
| `/gal` control plane 合約 | [../../docs/gal-control-plane-contracts.md](../../docs/gal-control-plane-contracts.md) |
| dispatch、alias 與 script architecture | [../../docs/command-dispatch-architecture.md](../../docs/command-dispatch-architecture.md) |
| runtime 安裝與 command surface 佈局 | [../../commands/commands.md](../../commands/commands.md)、[../../docs/installation-topology.md](../../docs/installation-topology.md) |
