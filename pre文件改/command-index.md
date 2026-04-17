# 指令索引

這份文件是 GAL 的主要 command index。GAL 有 GAL-native 的規劃 workflow family（`/planning`、`/deep-planning`、`/plan-to-prompt`），以及涵蓋完整開發生命週期的 specialist commands。gstack 是可插拔的 specialist provider——安裝後能提升 planning review 品質，但不是 GAL 的核心依賴。

- 如果你想知道「現在應該跑哪個指令」，先看這份。
- 如果你想知道「某個指令精確讀寫哪些 artifacts」，再往下跳到 [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md)。
- 如果你想知道「gstack provider 整合設計」，看 [../docs/gstack-integration.md](../docs/gstack-integration.md)。

## 先記住三件事

1. `/gal` 只處理 control plane，不重複包一層 specialist workflow。
2. specialist commands 直接執行工作，但必須回寫到 `/gal` 讀得懂的主要檔案與計畫區段。
3. GAL 採 repo-local state model，不使用 upstream gstack 的 user-global storage 作為核心狀態邊界。

## gstack 作為可選 provider

GAL 有 GAL-native 的 workflow 與 specialist commands，不依賴 upstream gstack runtime 運作。

- gstack 的 host 假設是 Claude Code；GAL 的主控制面是 Copilot，Gemini CLI 與 Codex 是其他可接入 runtime。
- GAL 要求 repo-local project files；upstream gstack 的部分資料模型是 user-global。
- GAL 的目標是 tool-agnostic methodology，而不是把控制權外包給某個外部工具安裝。

所以 GAL 的做法是：挑出與 GAL 相容的 gstack-style specialist commands 與工作流語意，將其實作成 GAL-native skills，並把結果回寫到 GAL 自己的 artifacts。

## Layer A：Control Plane

這層回答「現在是什麼狀態、下一步做什麼、怎麼乾淨收尾」這些控制面問題。

| 指令 | 回答的問題 | 主要讀寫 | 深入規格 |
| --- | --- | --- | --- |
| `/gal init` | 怎麼讓 repo 進入 GAL 管理？ | 建立 `.dev/project.md`、`.dev/state.md` | [../docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md) |
| `/gal status` | 現在做到哪裡？ | 讀 `.dev/state.md` 與活動 plan | [../docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md) |
| `/gal whats-next` | 下一步做什麼？ | 讀 state、review/test/ship/deploy 狀態 | [../docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md) |
| `/gal wrap-up` | 怎麼乾淨地結束這次 session？ | 收斂 `### Handoff Notes` 與 continuity | [../docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md) |
| `/gal research` | 怎麼進入結構化研究？ | 依 skill 路由研究工作流 | [../docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md) |
| `/gal pipeline` | 怎麼逐 task 自動推進實作、測試、審查與驗證？ | 讀活動 plan、`## Tasks`、`## Test Plan` 與 model routing，回寫 plan 狀態 | [../docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md) |

如果你要看 `/gal` 的 dispatch、alias 與 script contract，不在這份文件展開，直接看 [../docs/command-dispatch-architecture.md](../docs/command-dispatch-architecture.md)。

## Layer B：Specialist Commands

這層直接執行工作。你不需要先透過 `/gal` 才能呼叫它們，但它們的輸出必須符合 `/gal` 讀取的 plan write-back contract。

### 規劃

| 指令 | 用途 | 主要寫回 | 深入規格 |
| --- | --- | --- | --- |
| `/planning` | 從使用者需求與對話上下文產出 source plan | `docs/plans/<plan-slug>.md` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/deep-planning` | 深化任何規劃文件為正式 plan（包含 review-lane 類型的規劃深化） | `docs/plans/<plan-slug>.md` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/plan-to-prompt` | 在 source plan 完成 planning-stage 深化與審查後，轉換為 execution prompt | `.dev/plans/<plan-slug>.prompt.md` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/cso` | 資安審查與 findings 管理 | plan `## Review Results` 下的 security review | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |

Planning-stage review lanes 是 provider-routed capabilities，不是 GAL public commands：business / scope review、design review、engineering review 應視為 specialized deep-planning passes，先寫回 source plan 的同一組固定區段，再由 `/plan-to-prompt` 轉成 execution prompt。

### 設計

| 指令 | 用途 | 主要寫回 | 深入規格 |
| --- | --- | --- | --- |
| `/design-consultation` | 建立 repo-level design system | `DESIGN.md`、`CLAUDE.md` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/design-shotgun` | 生成多個視覺方向並核可一個變體 | `docs/designs/<plan-slug>/variant-approved.json` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/design-html` | 核可 mockup 轉成可執行 HTML 或元件實作 | `docs/designs/<plan-slug>/handoff-final.html` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/design-review` | 對照 `DESIGN.md` 進行 post-implementation 設計修正 | plan `## Review Results`、`docs/design-reports/` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |

### 除錯與審查

| 指令 | 用途 | 主要寫回 | 深入規格 |
| --- | --- | --- | --- |
| `/investigate` | 根因優先的除錯流程，自動搭配 `/freeze` | plan debug session 或工作紀錄 | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/review` | staff engineer 等級的 diff 審查 | plan `## Review Results`、`## Analyze` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |

### 瀏覽器與 QA

| 指令 | 用途 | 主要寫回 | 深入規格 |
| --- | --- | --- | --- |
| `/browse` | 提供其他流程使用的 browser primitive | session only | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/connect-chrome` | 切換成 headed Chrome | session only | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/setup-browser-cookies` | 將認證匯入 browser session | session only | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/qa` | 跑 test plan、修 bug、寫回 test results | plan `## Test Results`、`docs/qa-reports/` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/qa-only` | 只做 bug report，不修改程式碼 | plan `## Test Results (Report Only)`、`docs/qa-reports/` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |

### 發佈

| 指令 | 用途 | 主要寫回 | 深入規格 |
| --- | --- | --- | --- |
| `/ship` | merge 前最終關卡：測試、PR、文件 | plan `## Ship` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/land-and-deploy` | 合併並驗證部署 | plan `## Deploy` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/setup-deploy` | 建立 deploy config baseline | `CLAUDE.md` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/document-release` | 同步已發佈程式碼與文件 | repo docs、PR 補充內容 | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |

### 記憶與守護

| 指令 | 用途 | 主要寫回 | 深入規格 |
| --- | --- | --- | --- |
| `/learn` | repo 本地制度化記憶管理 | `.dev/learnings.jsonl` | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/careful` | 破壞性操作前警告 | session only | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/freeze` | 限制編輯範圍 | session only | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/guard` | 同時啟用 careful 與 freeze | session only | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/unfreeze` | 解除 freeze 邊界 | session only | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| `/gstack-upgrade` | 更新 GAL command/skill 安裝 | local machine maintenance | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |

## gstack 語意如何映射到 GAL

這裡的「映射」不是宣稱 GAL 與 upstream gstack 完全等價，而是說使用者能在 GAL 中找到對應的 specialist workflow public surface，同時由 GAL 自己負責 state ownership、runtime 佈局與 control-plane 問題。

### 快速對照

| 指令 | upstream gstack 語意 | GAL 的實作差異 |
| --- | --- | --- |
| discovery-style feature planning | per-feature design doc 啟動 | plan 寫入 `docs/plans/`，不是 `~/.gstack/projects/` |
| engineering review lane | 產出 task list 與 test plan | 先寫回 source plan 的 `## Test Plan` 與 `## Tasks`，再由 `/plan-to-prompt` 轉入 execution prompt |
| `/review` | staff diff review | `## Analyze` 成為 control plane 可讀的 drift verdict |
| `/qa` | 執行 test plan 並回報結果 | `## Test Results` + `docs/qa-reports/` |
| `/design-shotgun` | 視覺方向探索 | 資產路徑在 `docs/designs/` |
| `/learn` | session / sprint learnings | 儲存在 `.dev/learnings.jsonl` |

### Artifact Path Quick Reference

| 用途 | upstream gstack | GAL |
| --- | --- | --- |
| source plan | `~/.gstack/projects/$SLUG/design.md` | `docs/plans/<plan-slug>.md` |
| execution prompt | 無明確對等 | `.dev/plans/<plan-slug>.prompt.md` |
| design variants | `~/.gstack/projects/$SLUG/designs/` | `docs/designs/<plan-slug>/` |
| QA reports | `.gstack/qa-reports/` | `docs/qa-reports/` |
| design reports | `.gstack/design-reports/` | `docs/design-reports/` |
| sprint learnings | `~/.gstack/projects/$SLUG/learnings.jsonl` | `.dev/learnings.jsonl` |

完整 artifact ownership 與 plan section rules 請看 [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md)。

## gstack 風格工作流在 GAL 中的正確用法

GAL 沿用 gstack 的一個核心假設：discovery-style 規劃產出的 plan 應該是 per-feature，不是 per-product roadmap。

- 大 plan 要由人先拆成可獨立交付的 feature。
- engineering review lane 會拆 task 與 parallelization lane，不會把一個大 plan 拆成多個 plan。
- `SCOPE REDUCTION` 是縮小當前 plan，不是自動生成新 plan。

如果你要看完整的 labyrinth 範例、feature splitting 思路與手動審查路徑，直接看 [../docs/gstack-workflow-guide.md](../docs/gstack-workflow-guide.md)。

## Runtime 角色

| Runtime | 定位 |
| --- | --- |
| Copilot | 主控制面，擅長互動式 orchestration、review 對話與 plan refinement |
| Gemini CLI | worker runtime，適合跑 bounded specialist tasks |
| Codex CLI | 與 Copilot / Gemini 共用同一套 artifact contract，但用 `$` 作為 command 入口 |

這三者共享的是 artifacts 與 contracts，不是完全對稱的 host 能力。

## 往哪裡繼續讀

| 如果你要理解 | 讀這份 |
| --- | --- |
| gstack provider 整合設計與 GAL 分層 | [../docs/gstack-integration.md](../docs/gstack-integration.md) |
| 每個 command 的 reads / writes / plan sections | [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| feature splitting 與 workflow 教學 | [../docs/gstack-workflow-guide.md](../docs/gstack-workflow-guide.md) |
| `/gal` control plane 合約 | [../docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md) |
| dispatch、alias 與 script architecture | [../docs/command-dispatch-architecture.md](../docs/command-dispatch-architecture.md) |
| runtime 安裝與 command surface 佈局 | [../commands/commands.md](../commands/commands.md)、[../docs/installation-topology.md](../docs/installation-topology.md) |
