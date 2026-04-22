# Plan: GAL 多機執行分工

## Status

- Workflow: REVIEW
- Last Updated: 2026-04-22
- Plan Focus: 只定義節點的執行屬性，不替使用者決定派工

## Goal

把 GAL 在不同電腦上的分工改成「描述執行能力與持續性」，而不是把哪一種工作固定指派給哪一台機器。

這份計畫只回答兩件事：

- GAL 如何描述不同 PC 的執行特性
- 為什麼需要 Zellij 來支撐可持續執行

## Core Rule

哪些工作交給哪一台機器，由 user 決定。

GAL 本身不決定：

- 哪個 task 一定要跑在 Mac Mini
- 哪個 task 一定要跑在 Win11 PC
- 哪個節點一定是預設核心節點

GAL 只負責描述執行屬性，讓使用者依照當下情境選擇執行位置。

## Scope

本計畫只處理多機執行分工，以及節點身份宣告（dispatch declaration）。

本計畫不處理：

- task contract 細節
- state model
- worktree policy
- patch integration policy
- 自動派工策略
- Discord / Telegram intake
- MemPalace integration
- `/gal` routing 決策

## Execution Properties

GAL 描述的不是固定職責，而是每個節點在執行面上的屬性。

至少要能表達：

- 這台機器是否適合長時間執行
- 這台機器是否常駐或容易關機
- 這台機器目前是否可用
- 這台機器是否具備某種額外能力

這些資訊是給 user 做判斷用，不是讓 GAL 代替 user 做判斷。

## Machines

### Mac Mini

目前已知屬性：

- 比較適合作為長時間執行的 machine
- 比較適合提供持續執行能力
- 可以搭配 Zellij 維持背景執行

這些是執行特性，不代表 GAL 之後會自動把某類工作固定派給 Mac Mini。

### Win11 PC

目前已知屬性：

- 比較可能為了省電而關機
- 可用性比較依賴當下是否開機
- 可以作為可用時的執行節點

這些是執行特性，不代表 Win11 只能跑短任務，也不代表 GAL 會替 user 排除它。

### Future LAN Node

未來如果加入其他節點，也沿用同一個原則：

- 先描述執行屬性
- 不預設固定職責
- 不讓 GAL 自動替 user 決定派工

## Division Model

| Machine | GAL 記錄的內容 | 仍由 user 決定的內容 |
| --- | --- | --- |
| Mac Mini | 是否適合長時間執行、是否可持續在線、是否可用 Zellij 維持背景工作 | 實際要不要把某個 task 交給它 |
| Win11 PC | 是否目前開機、是否可能關機、是否當下可用 | 實際要不要把某個 task 交給它 |
| Future LAN Node | 它的可用性與特殊能力 | 實際要不要把某個 task 交給它 |

## Dispatch Declaration (B+C Design)

GAL 採用 B+C 雙層宣告機制，讓節點身份與每次派工意圖都能被明確記錄，且不污染 git 歷史。

### Layer B — `config.local.env`（per-machine，git-ignored）

每台機器在初次設定時填寫：

```env
GAL_NODE_ID=mac-mini
GAL_NODE_CAPABILITIES=persistent,zellij
```

這兩個欄位描述這台機器的身份與能力，供 user 決定派工時參考。
GAL 不讀取這些值來自動派工。

### Layer C — `.dev/state.md` `## Session Execution Context`

每次 session 開始時，由 user 明確填寫派工意圖：

```markdown
## Session Execution Context

Dispatched node: mac-mini
Execution mode: persistent (Zellij)
Notes: win11 offline today
```

這個 block 是 user 和 AI 在 session 中共同參照的派工記錄。
Session 結束後由 `/gal wrap-up` 清空或歸檔。

### Decision 5 — 採用 B+C 的理由

| 考量 | 說明 |
| --- | --- |
| git-safe | `config.local.env` git-ignored，不污染 repo 歷史 |
| 輕量 | 不需要新的資料結構或 schema |
| user-controlled | 所有派工意圖由 user 明確填寫，GAL 不代替決定 |
| 跨機器一致 | 每台機器各自維護自己的 NODE_ID，互不干擾 |
| 可擴充 | 未來加入新節點只需在 `config.example.env` 補填欄位 |

## Why Zellij

加入 Zellij 的原因不是為了定義一套新的 terminal workflow，而是為了保留一個明確能力：

- 某些工作需要在發起端離線後仍然繼續執行。

目前最適合提供這種能力的節點是 Mac Mini，所以 Zellij 是用來補上「持續執行」這個屬性。

## Zellij Role In This Plan

- Zellij 代表的是持續執行能力，不是派工規則。
- Zellij 解決的是工作不能因 SSH 中斷、terminal 關閉或發起端下線而停止。
- 目前這個能力主要落在 Mac Mini，因為它比較適合長時間執行。
- 這不等於 GAL 會自動把所有長任務派給 Mac Mini；最後仍由 user 決定。

## Planning Decisions

### Decision 1

GAL 記錄節點的執行屬性，不記錄寫死的工作分配。

### Decision 2

Mac Mini 目前唯一能明確寫下的事實，是它比較適合作為長時間執行的 machine。

### Decision 3

Win11 PC 目前唯一能明確寫下的事實，是它比較可能關機，因此可持續性較不穩定。

### Decision 4

是否把工作交給 Mac Mini、Win11 PC 或其他節點，是 user 的決定，不是 GAL 的決定。

## Phases

### P1. Freeze the execution-property model

先固定 GAL 只描述執行屬性，不描述寫死的節點職責。

### P2. Expose persistent-execution capability

把 Zellij 對應的「持續執行」能力明確掛到合適節點上。

### P3. Keep dispatch user-directed

確保多機系統在語意上始終是 user-directed dispatch，而不是 GAL 自動派工。

## Success Criteria

- 文件不再寫死 Mac Mini 與 Win11 PC 的任務分配。
- 文件明確區分「執行屬性」與「派工決定」。
- Zellij 被描述為持續執行能力，而不是固定分工規則。
- 文件明確寫出最終由 user 決定工作交給哪個節點。

## Final Rule

GAL 只依照執行屬性描述節點，真正的工作分配由 user 決定。