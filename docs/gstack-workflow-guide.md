# gstack 設計流程指南

本文說明 upstream gstack（garrytan/gstack）的設計使用流程。
理解這套流程是正確使用 GAL specialist commands 的前提。

## 核心設計假設

gstack 的所有命令都圍繞一個前提運作：

> **design doc 是 per-feature，不是 per-product。**
> 它捕捉的是「這次具體變更背後的思考」，不是整個產品的完整規格。

這代表：

- 每次 `/office-hours` 產出的 plan 應該對應一個可獨立交付的 feature
- 如果你的 plan 涵蓋了整個產品，那它是 roadmap，不是 executable plan
- 把 roadmap 拆成可執行的 feature plans 是人的責任，不是工具的責任

## 流程圖

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

## 實際範例: labyrinth

以製作 labyrinth 為例，假設你已經透過一次 `/office-hours` 得到一份很大的plan，內容同時包含：

- 迷宮生成
- 玩家移動與視角
- 鑰匙 / 門 / 關卡進程
- 敵人與戰鬥
- 介面與教學提示

這份文件很可能是正確方向，但它的性質比較接近 product roadmap，不是可以直接丟進 gstack 流程的單一 feature plan。

錯誤期待是：

- 先拿這份大 plan 跑 `/plan-eng-review`
- 再期待它自動幫你拆成 3 到 5 份小 plan
- 最後再幫你排成逐步可執行的 sprint

gstack 不做這件事。

正確做法是人工先拆 feature，再讓每個 feature 各自走完整條流程。

例如可以先拆成這樣：

1. 第一個 feature: 迷宮生成最小可玩版本
2. 第二個 feature: 玩家移動、碰撞與第一人稱視角
3. 第三個 feature: 鑰匙、門與基本進程控制
4. 第四個 feature: 第一種敵人與最小戰鬥循環
5. 第五個 feature: HUD、提示與新手引導

然後不是拿整份 labyrinth 大 plan 一次做完，而是這樣執行：

1. 先選「迷宮生成最小可玩版本」
2. 只針對這個 feature 跑一次 `/office-hours`
3. 對這個 feature 跑 `/autoplan` 或 `/plan-ceo-review`、`/plan-design-review`、`/plan-eng-review`
4. 讓 reviewed plan 產出 task list 與 worktree parallelization
5. 實作、`/review`、`/qa`、`/ship`
6. 完成後再回到 labyrinth roadmap，挑下一個 feature

換句話說，labyrinth 的大 plan 可以保留，因為它有方向價值；但真正進入 gstack 流程的，永遠應該是「其中一個可獨立交付的 feature」。

## 階段說明

### 1. 人工拆分 — 流程的起點

gstack 沒有「大 plan 自動拆成小 plan」的機制。

如果你做了一次 `/office-hours` 然後得到一個涵蓋 10 個模組的大 plan，正確做法是：

1. 把這個 plan 當作 roadmap 參考
2. 自己決定先做哪個 feature
3. 對每個 feature 分別跑 `/office-hours`

每個 feature 獨立一個 plan file、獨立一條 session。

### 2. /office-hours — 產出 design doc

`/office-hours` 模擬 YC office hours 風格的對話。它會：

- 挑戰你的前提假設
- 問清楚目標使用者和核心問題
- 產出一份 design doc

這份 design doc 就是後續所有審查和實作的基礎。

### 3. 審查管線 — 兩種路徑

#### 路徑 A: /autoplan（自動化）

依序執行 CEO → Design → Eng review，自動做出大部分決策，只在真正需要人類判斷的品味問題才停下來。適合有信心的 feature。

#### 路徑 B: 手動逐步審查

分開執行三個審查命令，每步都有機會介入調整。

##### /plan-ceo-review — 商業方向

有四種模式，由 AI 自動判斷：

| 模式 | 說明 |
| --- | --- |
| EXPANSION | 10-star vision，放大野心 |
| SELECTIVE EXPANSION | 保留核心，在特定面向加碼 |
| HOLD SCOPE | 範圍正確，只做 polish |
| SCOPE REDUCTION | 範圍太大，必須刪東西 |

重點：這四種模式都是在調整「同一個 plan」的範圍，不是產出新 plan。

##### /plan-design-review — 設計審查

檢查 UX flow、accessibility、設計系統一致性。

##### /plan-eng-review — 工程審查

這是最關鍵的審查步驟，產出：

- **Scope Challenge**: 超過 8 個檔案或 2 個新 class 就建議縮減
- **Architecture / Code Quality / Tests / Performance** 四大面向的 review
- **Worktree Parallelization Strategy**: 分析 task 之間的依賴關係，產出可平行執行的 lane

Worktree Parallelization 拆的是「一個 feature 內的實作步驟」，不是「把一個大 plan 拆成多個 plan」。

### 4. 實作 — 逐 task 執行

審查完成後 plan 內會有具體的 task list。
用 `/gal-pipeline` 自動逐 task 執行，或手動一個一個做。

每個 task: implement → test → review → commit。

### 5. 收尾 — review → qa → ship

- `/review`: staff engineer 等級的 diff 審查
- `/qa`: 在真實瀏覽器中跑 test plan
- `/ship`: 推分支、開 PR、觸發 `/document-release`

## 平行工作模型

gstack 支援 10-15 個平行 sprint。這不是一個 plan 內的平行，而是：

- 每個 sprint = 獨立的 feature
- 每個 feature = 獨立的 plan file
- 每個 feature = 獨立的 session / worktree
- 互不干擾，各自走完整個 office-hours → ship 流程

## 常見誤解

### 「跑 /office-hours 產出大 plan，然後用 /plan-eng-review 拆成小 plan」

不對。`/plan-eng-review` 審查的是一個 feature 的 plan，它會產出 task list 和 worktree parallelization，但不會把一個 plan 拆成多個 plan。

### 「/plan-ceo-review 的 SCOPE REDUCTION 會幫我拆 plan」

不對。SCOPE REDUCTION 是刪掉多餘的東西，讓 plan 回到可執行的大小。被刪掉的部分如果以後要做，由人決定是否開新的 `/office-hours`。

### 「gstack 執行再慢慢拆」

gstack 的設計不支援這個流程。正確做法是從一開始就以 feature 為單位思考，每個 feature 對應一次 `/office-hours`。

如果你已經有了 product-level 的 plan，把它當作參考文件，從中挑 feature 出來分別執行。

## 對 GAL 的意義

GAL 採用 gstack 的 specialist command surface，但保留自己的狀態管理（`/gal init`、`/gal status`、`/gal whats-next`、`/gal wrap-up`）。

使用者在 GAL 中的正確工作方式：

1. 用 `/gal init` 初始化 repo
2. 決定要做哪個 feature
3. 用 `/office-hours` 產出該 feature 的 plan
4. 用 `/autoplan` 或手動審查完善 plan
5. 用 `/gal-pipeline` 或手動實作
6. 用 `/review` → `/qa` → `/ship` 收尾
7. `/gal wrap-up` 結束 session
8. 回到步驟 2，做下一個 feature
