# GAL 與 gstack 的整合模型

## 這份文件是什麼

這不是歷史討論筆記，也不是理想化願景稿。

這份文件說明的是目前已拍板的整合模型：

- 為什麼 GAL 不能直接依賴 upstream gstack
- 為什麼最終形態是「GAL control plane + gstack-style specialist commands」
- Copilot 與 Gemini 在這個模型中各自扮演什麼角色
- canonical artifacts 與 command contract 如何接線

如果你要理解 README 裡的 command catalog 為什麼長這樣，讀這份文件。

## 問題定義

gstack 是為 Claude Code 生態系設計的 slash command 系統。

GAL 的主控制面不是 Claude Code，而是 Copilot。Gemini CLI 則是另一個可接入的 runtime。這代表一個基本現實：

**GAL 不能把「直接執行 upstream gstack slash commands」當成整合策略。**

原因不是語意不相容，而是 host runtime 不相容。

如果 README 寫成「直接用 gstack」，在 Copilot 裡就是假話。真正可行的方式只有一種：

**把 gstack 的工作流語意重新實作成 GAL-native skills。**

## 核心決策

### 1. GAL 保留 control plane，不保留另一套平行 coding workflow

GAL 持有的是控制面責任：

- repo bootstrap
- state projection
- continuity / wrap-up
- next-action recommendation
- research entry point

這些工作由 `/gal` surface 負責。

GAL 不應再另外定義一組 `/gal review`、`/gal qa`、`/gal ship` 之類與 specialist layer 平行的命令面。那會變成重複抽象。

### 2. coding workflow 直接採用 gstack-style command surface

對使用者來說，真正的工作層命令就是：

- `/office-hours`
- `/plan-ceo-review`
- `/plan-eng-review`
- `/plan-design-review`
- `/autoplan`
- `/review`
- `/qa`
- `/ship`
- 以及 design / browser / release / memory / guardrail 全部 specialist commands

也就是說，GAL 吸收的是 gstack 的操作語意，不是 upstream repo 或 Claude Code runtime 依賴。

### 3. specialist commands 必須回寫到 GAL canonical artifacts

這是整合能成立的關鍵。

specialist commands 不是各做各的。它們必須把結果寫回 GAL 讀得懂的 artifact：

- plan `## Review Results`
- plan `## Test Plan`
- plan `## Test Results`
- plan `## Ship`
- plan `## Deploy`
- plan `### Handoff Notes`
- `.dev/state.md`

`/gal status` 與 `/gal whats-next` 不需要知道 specialist 是怎麼做事的，它們只需要讀這些 canonical sections。

### 4. repo-local state 是真正的 ownership boundary

upstream gstack 會把許多資料寫到 user-global 路徑，例如 `~/.gstack/projects/$SLUG/`。

GAL 不採用這個模型。

GAL 的 ownership boundary 是 repo-local：

- `.dev/`
- `docs/plans/`
- `docs/designs/`
- `docs/qa-reports/`
- `docs/benchmarks/`
- `docs/retros/`

這讓團隊成員看到的是同一組 artifacts，而不是每個人本機各自一份不可見的狀態。

## 最終分層

### Layer A: Control Plane

這層回答的是使用者的控制面問題。

| Command | 回答的問題 |
| --- | --- |
| `/gal init` | 怎麼讓這個 repo 進入 GAL 管理？ |
| `/gal status` | 現在工作做到哪裡？ |
| `/gal whats-next` | 我現在下一步做什麼？ |
| `/gal wrap-up` | 我怎麼乾淨地收尾這次 session？ |
| `/gal research` | 我要進入結構化研究流程 |

這層不做 specialist execution。它負責讀 state、投影 state、推薦下一步、收斂 continuity。

### Layer B: Specialist Execution

這層直接執行工作。

| Family | Commands |
| --- | --- |
| Planning | `/office-hours`, `/plan-ceo-review`, `/plan-eng-review`, `/plan-design-review`, `/autoplan`, `/cso` |
| Design | `/design-consultation`, `/design-shotgun`, `/design-html`, `/design-review` |
| Debug / Review | `/investigate`, `/review` |
| Browser / QA | `/browse`, `/connect-chrome`, `/setup-browser-cookies`, `/qa`, `/qa-only` |
| Ship / Release | `/ship`, `/land-and-deploy`, `/canary`, `/benchmark`, `/setup-deploy`, `/document-release`, `/retro` |
| Memory / Guardrails | `/learn`, `/careful`, `/freeze`, `/guard`, `/unfreeze`, `/gstack-upgrade` |

這些命令不需要通過 `/gal` 才能執行，但它們的輸出必須符合 `/gal` 會讀取的 contract。

## 為什麼不是「保留舊 GAL 命令，再包一層」

這條路已被否決，原因很直接。

如果 GAL 同時保留：

- `/gal review`
- `/gal qa`
- `/gal ship`

又再提供：

- `/office-hours`
- `/review`
- `/qa`
- `/ship`

那使用者就必須先理解兩套命令面的差別，才知道要做什麼。這是純粹的複雜度，沒有帶來能力。

因此 final model 是：

- `/gal` 只保留 control-plane 問題
- specialist commands 直接成為 work-layer public surface

## 為什麼不是「直接依賴 gstack 安裝」

這也被否決。

原因：

1. gstack 的 host 假設是 Claude Code，不是 Copilot。
2. GAL 的價值之一是 tool-agnostic methodology；若把 upstream gstack 安裝變成前置條件，控制權就外包出去。
3. GAL 需要 repo-local canonical artifacts；upstream gstack 的部分儲存模型是 user-global，不符合 GAL 的 state ownership。

所以 GAL 的做法是：

- 讀 gstack 的 workflow contract
- 用 GAL 自己的 SKILL.md 實作相同語意
- 把結果回寫到 GAL 自己的 canonical artifacts

## Copilot 與 Gemini 的角色

兩者共享的是 contract，不是 host 能力對稱。

### Copilot

- 主控制面
- 擅長互動式 orchestration
- 適合 `/gal status`、`/gal whats-next`、review 對話、plan refinement

### Gemini CLI

- worker runtime
- 適合被派去執行具體 specialist 任務
- 與 Copilot 共用同一套 state model、同一套 skill contract、同一套 artifact write-back 規則

整合目標不是假裝兩者完全一樣，而是讓它們對同一組 repo artifacts 做一致操作。

## Artifact 接線規則

### `.dev/project.md`

repo 的背景、技術棧、目標與限制。

### `.dev/state.md`

control plane 的索引：

- active plans
- blockers
- session continuity

### `docs/plans/<plan-slug>.md` — Source Plan Doc

單一 feature 或 sprint 的 human-readable plan document。

建立時包含範圍、理由與需求；建立後不被 specialist commands 修改。

### `docs/plans/<plan-slug>.prompt.md` — AI 執行工作檔案

單一 feature 或 sprint 的 canonical execution memory。

這裡承接：

- `## Goal`
- `## Context`
- `## Scope`
- `## Review Results`
- `## Test Plan`
- `## Test Results`
- `## Ship`
- `## Deploy`
- `### Handoff Notes`

兩個檔案以相同的 `plan-slug` 作為關聯鍵。控制平台讀取 `.prompt.md` 來輸出狀態。

### 其他 supporting artifacts

- `DESIGN.md`
- `docs/designs/`
- `docs/qa-reports/`
- `docs/benchmarks/`
- `docs/retros/`
- `.dev/learnings.jsonl`

## `/gal status` 與 `/gal whats-next` 如何成立

這兩個命令不是靠內建魔法推論，而是靠 specialist commands 的回寫 contract。

例如：

- `/plan-eng-review` 寫 `<!-- ENG_REVIEW: CLEAR -->`
- `/qa` 寫 `## Test Results`
- `/review` 寫 `<!-- STAFF_REVIEW: CLEAR -->`
- `/ship` 寫 `## Ship` 與 PR URL
- `/land-and-deploy` 寫 `## Deploy`

所以 `/gal whats-next` 能根據已存在的 artifacts 判斷：

- 還沒做 eng review，就先做 `/plan-eng-review`
- review 已清，test plan 已有，就做 `/qa`
- QA 已清、review 也過了，就做 `/ship`
- 已有 PR 且 deploy config 已設定，就做 `/land-and-deploy`

這是 contract-driven orchestration，不是另一套 hidden workflow。

## 過渡期差異

舊文件中出現過的下列概念，現在都不再是主 public model：

- T0/T1/T2 作為新 command surface 的主要詞彙
- 「直接呼叫 upstream gstack」作為整合方法

這些如果還出現在舊文件裡，應視為待遷移描述，而不是目前設計。

## 結論

GAL 整合 gstack 的正確方式，不是依賴 upstream runtime，也不是複製另一套平行 workflow。

正確方式是：

1. GAL 持有 control plane
2. gstack-style commands 成為 specialist execution layer
3. 全部 specialist results 回寫到 GAL canonical artifacts
4. Copilot 與 Gemini 共享 contract，但不強求 host 能力完全對稱

這樣做的結果是：

- 使用者得到一個可理解的 command surface
- repo 得到可追蹤、可共享的狀態與 artifacts
- methodology 不依賴 Claude Code 或任何單一工具生態
