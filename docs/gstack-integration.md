# GAL 目標架構：吸收 GSD 能力與 gstack 操作層

## 目的

GAL 的目標不是維持一套獨立於 GSD 與 gstack 的舊工作流，也不是把兩者原封不動搬進來。

GAL 的目標是成為一個跨 AI 工作控制平面：

- 以 Markdown 為 canonical source
- 以 repo-local state 為工作記憶
- 以 `/gal` 為統一入口
- 可跨 Copilot 與 Gemini 執行
- 可把工作派送到不同 execution endpoint

## 核心決策

### 1. GSD 提供的是核心能力，不是要照搬的產品表面

GAL 應吸收 GSD 類能力：

- stateful workflow
- context engineering
- task memory
- summary / continuity
- orchestration loop

GAL 不需要照搬 GSD 的命名、安裝方式或全部 command surface。

### 2. gstack 提供的是 coding workflow 的主要操作層

GAL 在 coding workflow 上應直接使用 gstack 指令面。

也就是說，對於規劃、審查、QA、發佈這些工作，主要操作層應直接採用 gstack 的指令，而不是再由 GAL 重新包一套命名相近的平行指令。

GAL 應吸收的重點是：

- `/office-hours`
- `/plan-ceo-review`
- `/plan-eng-review`
- `/plan-design-review`
- `/review`
- `/qa`
- `/ship`
- multi-model second opinion 思維

重點不是保留 gstack 的 repo 結構，而是直接採用它已經成熟的 coding workflow 操作層。

### 3. `/gal` 是 control plane，不是單次 dispatcher

`/gal` 的責任應該是：

- 讀取與更新 repo-local state
- 判斷目前任務屬於哪種工作流
- 選擇需要的 specialist operations
- 決定使用哪個 runtime
- 在必要時派工到遠端 worker
- 將結果壓回 canonical artifacts

因此，`/gal` 不應停留在只輸出 dispatch block 的層級。

### 4. T0 / T1 / T2 不保留

T0 / T1 / T2 不應保留在新的 GAL 架構裡。

原因很直接：

- 它們屬於舊 GAL 的 workflow 分流模型
- 這個模型會迫使使用者先理解內部流程分類，再開始工作
- 一旦 coding workflow 直接採用 gstack 指令，舊 tier 分流就成為多餘抽象

新的系統應直接以操作意圖與 task context 決定需要哪些 guardrails，而不是保留 T0 / T1 / T2 這組中介分類。

### 5. golem 應退到內部編排層

planner、architect、designer、reviewer、tester、verifier 仍然有存在價值。

但它們主要應該是 orchestrator 的內部角色，而不是主要 user-facing API。

若保留 direct golem invocation，也只應作為 expert escape hatch。

### 6. Copilot 與 Gemini 共享同一套 contract，但不必假裝完全對稱

GAL 必須讓 Copilot 與 Gemini 共享：

- 同一套 state model
- 同一套 `/gal` 語意
- 同一套 task contract
- 同一套結果回寫規則

但不需要假裝兩者有完全相同的 host 能力。

目前較合理的假設是：

- Copilot 偏互動型主控制面
- Gemini CLI 偏可腳本化的 worker engine

## 使用者操作層

GAL 的使用者操作層應拆成兩類。

### A. GAL control-plane 指令

這一層負責 state、連續性、派工與控制面工作。

- `/gal start`
- `/gal status`
- `/gal next`
- `/gal do`
- `/gal research`

這組指令是目標中的正式操作面，不等同於目前 repo 內仍存在的過渡期 `gal init`、`gal plan`、`gal pause` 等舊入口。
這些舊入口在重構完成前可繼續存在作為兼容層，但不應被視為新的長期 command surface。

### B. Coding workflow 指令

這一層直接使用 gstack 指令。

- `/office-hours`
- `/plan-ceo-review`
- `/plan-eng-review`
- `/plan-design-review`
- `/review`
- `/qa`
- `/ship`

也就是說，未來 GAL 不應再自行定義 `/gal plan`、`/gal review`、`/gal qa`、`/gal ship` 這類與 gstack 平行的 coding workflow 指令面。

## 狀態模型

GAL 需要完整的 repo-local state layer，而不只是一份簡短索引。

目標最小集合：

- `.dev/project.md`
- `.dev/state.md`
- `.dev/requirements.md`
- `.dev/roadmap.md`
- `.dev/summary.md`
- `.dev/threads/`
- `docs/plans/*.prompt.md`

其中：

- `.dev/` 負責 repo 層級的持久狀態
- `docs/plans/*.prompt.md` 負責 task-local execution memory

plan file 不應承載整段對話歷程，只應保留執行所需的規格、狀態、驗證與 handoff。

## 過渡期規則

在 GAL 尚未完成內化前：

- 若已安裝 gstack，仍可直接使用 `/office-hours`、`/plan-eng-review`、`/review`、`/qa`、`/ship` 等指令
- 這些指令目前仍是外部 specialist operations
- 輸出結果必須回寫到 GAL 的 canonical artifacts
- repo 內現有的 `/gal init`、`/gal plan`、`/gal pause` 等入口可暫時作為 bootstrap 與兼容層，但它們不代表最終 public control-plane API

這代表過渡期可以共用，但 canonical ownership 仍在 GAL。

完成重構後，coding workflow 仍直接使用 gstack 指令；改變的是它們會被正式接入 GAL 的 state 與 contract，而不是被另一套 `/gal` coding 指令取代。

## 不做的事

GAL 不應做以下事情：

- 繼續把舊的 tier flow 保留為主要使用者介面
- 保留 T0 / T1 / T2 作為新的正式 workflow 概念
- 繼續把 direct golem invocation 當成主要操作模式
- 把 gstack 是否安裝，當成 GAL 是否可工作的前提
- 把 Copilot 與 Gemini 硬做成完全對稱的 host
- 把遠端派工獨立成第二套 workflow
- 直接假設可呼叫 gstack slash commands：gstack 屬於 Claude Code 生態系，在 Copilot 環境中無法直接執行（見 `docs/plans/gal-coding-workflow-native.prompt.md`）

## 實作順序

1. 重寫 repo-local state model
2. 做實 Copilot / Gemini adapter parity
3. 重寫使用者操作層，將 coding workflow 明確切到 gstack 指令面
4. 將 `/gal` 升級成 orchestrator，專注 control-plane 工作
5. 接上 notebook / Mac Mini execution plane

## 對應計畫

- `docs/plans/gal-cross-ai-orchestrator.prompt.md`
- `docs/plans/infra-lan-worker-topology.prompt.md`

## 結論

完整吸收 GSD 與 gstack 後，GAL 應保留的不是舊 command tree，也不是舊 tier 分流，而是 control plane。

GAL 的核心價值應收斂為：

- canonical methodology
- repo-local state
- cross-runtime parity
- orchestrator contract
- execution endpoint contract

這才是 GAL 應該持有、而不應外包出去的部分。
