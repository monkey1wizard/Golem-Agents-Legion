# 開發者指南

這份文件不是第二份正式規格。它的角色是維護者導航：幫你在修改 GAL repo 前，先判斷自己正在碰哪一層、該讀哪份真正的 source of truth，以及哪些規則不能破壞。

## 先判斷你在改哪一層

| 你要改什麼 | 先問自己 | 真正的來源文件 |
| --- | --- | --- |
| 設計原則、方法論邊界 | 這是 durable rule，還是一次性實作細節？ | [../docs/design-principles.md](../docs/design-principles.md) |
| `/gal` 指令表面、dispatch、alias | 這是 control plane contract 還是 runtime plumbing？ | [../docs/command-dispatch-architecture.md](../docs/command-dispatch-architecture.md)、[../docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md) |
| gstack-style specialist semantics | 這是 command index、deep contract，還是 workflow 教學？ | [command-index.md](command-index.md)、[../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md)、[../docs/gstack-workflow-guide.md](../docs/gstack-workflow-guide.md) |
| 安裝拓撲、skills 安裝位置、MCP merge | 這是 runtime install 問題，還是使用者入口說明？ | [../docs/installation-topology.md](../docs/installation-topology.md)、[../commands/commands.md](../commands/commands.md) |
| Repo-local state 與 plan artifacts | 哪個 artifact 應該擁有這份資訊？ | [../docs/per-repo-context.md](../docs/per-repo-context.md)、[../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| 多裝置或 worker 拓撲 | 這是主工作流的一部分，還是執行平面的擴充 lane？ | [mod/remote-worker.md](mod/remote-worker.md)、[../docs/remote-worker-architecture.md](../docs/remote-worker-architecture.md) |

如果你一開始就不知道自己在改哪一層，先不要動手改檔。先把這個判斷釐清，不然很容易把 README、devguide、command contracts 全部一起改壞。

## 不可破壞的規則

### 1. Markdown 才是 durable asset

- 方法論、規則與合約應存放在 repo 內的 Markdown 與 source files。
- 生成出的 adapters、runtime configs、烘焙 command files 都不是 source of truth。

### 2. `/gal` 只解決 control-plane 問題

- `/gal` 不應重新包一層 `/review`、`/qa`、`/ship` 等 specialist workflow。
- specialist commands 可以直接被呼叫，但它們必須回寫到 `/gal` 讀得懂的 artifacts。

### 3. Repo-local state 是 ownership boundary

- `.dev/`、`docs/plans/`、`docs/designs/`、`docs/qa-reports/` 等是可共享的主要檔案與輸出目錄。
- 不要把 GAL 的核心狀態移回 user-global storage。

### 4. 缺失工具不能被當作靜默成功

- GAL 採 skill-level tool routing，不是 repo-wide CLI-first 或 MCP-first。
- 每個依賴外部工具的 skill 都要明確定義 preferred path、fallback path、no-tool behavior。

### 5. 不要為單一 runtime 優化到破壞可攜性

- 對某個工具很順手，但會讓 Copilot / Gemini / Codex contract 分叉的改動，通常是壞改動。
- README、devguide、commands、setup scripts 都應優先服務跨 runtime parity。

### 6. 不要把摘要寫成第二份正式規格

- 如果某份文件只是幫人找路，就不要把完整 spec 再抄寫一遍。
- 摘要應該用來導讀與縮短跳轉，不應該創造另一份需要同步維護的版本。

### 7. Provider routing 應留在 workflow 層

- 不要做成「某個 agent 偵測到 gstack 或其他 provider 後，就自己切換人格、流程或 contract」。
- formal workflow 先決定 provider，再由 provider 產出內容，最後回寫到同一組主要檔案與計畫區段。

## 資訊應該寫在哪裡

| 資訊類型 | 正確位置 |
| --- | --- |
| 永久設計原則與架構理由 | `../docs/`、`../commands/`、`../scripts/`、`../workflows/` |
| Repo-local 工作上下文 | 目標 repo 的 `.dev/project.md`、`.dev/state.md` |
| 人類可讀的單一 feature plan | `docs/plans/<plan-slug>.md` |
| AI 執行工作檔與 specialist 回寫 | `.dev/plans/<plan-slug>.prompt.md` |
| 一次性 session continuity | plan 的 `### Handoff Notes` 與 `.dev/state.md` |

如果某個 plan 完成後仍然包含值得保留的知識，應把它提取回來源文件，而不是把 plan 永遠留下來當隱性知識庫。

## 常見改動的正確入口

### 變更 `/gal` 指令或 alias

1. 先確認這是 control plane 還是 specialist workflow。
2. 若是 control plane，先讀 [../docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md)。
3. 若牽涉 dispatch 或 alias，再讀 [../docs/command-dispatch-architecture.md](../docs/command-dispatch-architecture.md)。
4. 若最後會影響 runtime 呈現，再讀 [../commands/commands.md](../commands/commands.md) 與 [../docs/installation-topology.md](../docs/installation-topology.md)。

### 新增或修改 specialist command

1. 先看 [command-index.md](command-index.md) 確認它應該出現在哪個家族與哪個讀者入口。
2. 再看 [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) 確認 input reads、output artifacts、plan write-back contract。
3. 若你其實是在改 gstack workflow 教學，而不是 command contract，應該去 [../docs/gstack-workflow-guide.md](../docs/gstack-workflow-guide.md)。
4. 若新增的是 GAL-native planning 指令（`/planning`、`/deep-planning`、`/plan-to-prompt`），這屬於 GAL 自己的 workflow，不需要對接 gstack。

### 調整 setup、skills 安裝或 MCP merge

1. 先讀 [../docs/installation-topology.md](../docs/installation-topology.md)。
2. 再確認 [../commands/commands.md](../commands/commands.md) 是否也需要更新使用者能看見的 runtime surface 說明。
3. 不要把這類 runtime plumbing 細節塞回 README 首頁。

### 重構文件本身

1. 先確認這份文件的唯一工作是否明確。
2. 若某段內容已經被別的來源文件擁有，就改成摘要加連結，而不是再寫一份。
3. 若從首頁移出內容，必須同時給出新的落點，不要只做刪減。

## 自檢清單

提交前至少檢查這些問題：

- 這次改動有沒有新增第二份 source of truth？
- `/gal` 是否仍然只處理 control plane？
- specialist commands 是否仍然回寫到主要檔案與計畫區段？
- 有沒有把 repo-local state 推回 user-global path？
- 有沒有把工具缺失誤當成成功？
- 有沒有把 provider routing 偷綁進 agent persona、單一工具偵測或 runtime 特例？
- README、devguide、command index 是否各自維持單一明確角色？
- 新增或搬移的內容是否有正式落點與可用連結？

## 建議閱讀順序

| 讀這份文件的人 | 建議順序 |
| --- | --- |
| 第一次改 GAL repo 的維護者 | 這份指南 → [../docs/design-principles.md](../docs/design-principles.md) → [../docs/command-dispatch-architecture.md](../docs/command-dispatch-architecture.md) |
| 想理解 command surface 的人 | [command-index.md](command-index.md) → [../docs/gstack-command-contracts.md](../docs/gstack-command-contracts.md) |
| 想改 setup 或 runtime 安裝的人 | [../docs/installation-topology.md](../docs/installation-topology.md) → [../commands/commands.md](../commands/commands.md) |
| 想理解 feature plan 與 workflow semantics 的人 | [../docs/gstack-workflow-guide.md](../docs/gstack-workflow-guide.md) |

## 相關文件

- [readme.zh-Hant.md](readme.zh-Hant.md) — 使用者入口
- [command-index.md](command-index.md) — GAL 指令索引
- [mod/gstack.md](mod/gstack.md) — gstack 可選 provider 工作流模組
- [../docs/design-principles.md](../docs/design-principles.md) — 設計原則來源規格
- [../docs/command-dispatch-architecture.md](../docs/command-dispatch-architecture.md) — dispatch 來源規格
- [../docs/installation-topology.md](../docs/installation-topology.md) — 安裝拓撲來源規格
