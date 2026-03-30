# Plan: GAL 跨 AI 控制平面重構

## Goal

將 GAL 重構為跨 AI 工作控制平面：以 Markdown 為 canonical source，以 `.dev/` 為 repo-local state，以 `/gal` 為統一入口，吸收 GSD 的 stateful workflow 能力與 gstack 的 specialist operation layer，並支援 Copilot、Gemini 與後續多機 execution endpoints。

## Tier

T2

## Review Pack

- architect-full
- designer
- reviewer
- analyst

## Requirements

- [ ] `/gal` 必須從 dispatcher 升級為 orchestrator。
- [ ] Copilot 與 Gemini 必須共享同一套 state model、task contract 與 command semantics。
- [ ] `.dev/` 必須擴充為完整的 repo-local state layer。
- [ ] Coding workflow 的主要指令面必須直接使用 gstack 指令，而不是由 GAL 重新定義一套平行命名。
- [ ] `/gal` 只保留 control-plane 職責，不再承擔 coding workflow 的主要指令面。
- [ ] T0 / T1 / T2 不保留。
- [ ] gstack 的價值應被接入 GAL 的 state、task contract 與 review packs，而不是只當外部參考。
- [ ] plan files 只保留執行所需內容，不承載完整對話歷程。
- [ ] 多機 execution plane 必須服從同一套 orchestrator contract。

## Phases

### P1: Repo-Local State Model

- **Scope**
	- 定義 `.dev/project.md`、`.dev/state.md`、`.dev/requirements.md`、`.dev/roadmap.md`、`.dev/summary.md`、`.dev/threads/` 的職責。
	- 重新定義 plan files 與 `.dev/` 的邊界。
	- 調整 templates 以反映新的 state model。

- **Files**: `docs/per-repo-context.md`, `templates/project.md`, `templates/state.md`, `templates/plan.md`, `templates/requirements.md`, `templates/roadmap.md`, `templates/summary.md`
- **Verify**: 同一個任務可在不依賴長 chat 的情況下續接，且不同狀態資訊都有固定落點。

### P2: Adapter Parity

- **Scope**
	- 讓 Copilot 與 Gemini 讀取一致的 workflow、state 與 task contract。
	- 保持 generated adapters 為 disposable output。
	- 明確區分共享 contract 與 host capability asymmetry。

- **Files**: `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh`, `scripts/Sync-DevContext.ps1`, `scripts/sync-dev-context.sh`, `docs/installation-topology.md`, `scripts/scripts.md`
- **Verify**: 同一 repo 的狀態與 workflow 可被 Copilot 與 Gemini 一致理解。

### P3: Operation Layer And Role Model

- **Scope**
	- 定義雙層操作面：`/gal` 負責 control-plane；coding workflow 使用 GAL-native skill files 實作等效操作（見前置計畫 `gal-coding-workflow-native.prompt.md`）。
	- 注意：gstack 使用 Claude Code 環境，其 slash commands 無法在 Copilot 中直接執行。操作層以 GAL-native skills 實作，而非依賴 gstack 安裝。
	- 將 golem 降級為內部編排角色。
	- 將 coding workflow skill outputs 正式接入 GAL 的 state model、task contract 與結果回寫規則。
	- 定義 expert escape hatch 的保留邊界。

- **Prerequisite**: `docs/plans/gal-coding-workflow-native.prompt.md` P2–P4 完成
- **Files**: `docs/gstack-integration.md`, `commands/commands.md`, `agent/agents.md`, `model-roles.md`, `workflows/coding.md`
- **Verify**: 使用者可直接在 Copilot 中透過 GAL coding workflow skills 完成計畫審查與代碼審查，而 GAL 能持續維護 state 與 control-plane 邏輯。

### P4: `/gal` Orchestrator

- **Scope**
	- 將 `/gal` 從目前的 dispatcher + alias 兼容層，升級為正式的 control-plane orchestrator。
	- 收斂目標中的 control-plane 語意，包含 `start`、`status`、`next`、`do`、`research` 等操作。
	- 保留現有 `gal-*` 與舊子命令作為過渡期兼容層，直到新操作面完成接管後再退場。
	- 讓系統管理 state、runtime、offload、continuity 與結果回寫。
	- 將舊的 tier 分流完全移除，改由 task context 與 operation type 直接決定流程。

- **Files**: `scripts/gal.ps1`, `scripts/gal.sh`, `docs/command-dispatch-architecture.md`, `commands/gal/SKILL.md`, `commands/gal-*/SKILL.md`
- **Verify**: `/gal` 可穩定承擔 control-plane 工作，新的 control-plane 操作面可覆蓋 start / status / next / do / research，而不與 gstack coding workflow 指令重疊。

### P5: Execution Endpoints

- **Scope**
	- 將 notebook 與 Mac Mini 接入同一套 task contract。
	- 讓 orchestrator 根據任務特性決定是否 offload。
	- 避免遠端執行面長出第二套 workflow。

- **Files**: `docs/plans/infra-lan-worker-topology.prompt.md`, `docs/remote-worker-architecture.md`, `scripts/Invoke-GalRemoteTask.ps1`, `scripts/Start-GalWorker.ps1`, `scripts/Get-GalRemoteResult.ps1`
- **Verify**: 任務可由主控制面派送到遠端 endpoint，並正確回寫狀態與結果。

## Files to Create or Modify

- `docs/design-principles.md`
- `docs/per-repo-context.md`
- `docs/command-dispatch-architecture.md`
- `docs/installation-topology.md`
- `docs/gstack-integration.md`
- `templates/`
- `commands/`
- `agent/`
- `scripts/gal.ps1`
- `scripts/gal.sh`
- `scripts/Setup-Machine.ps1`
- `scripts/Sync-DevContext.ps1`
- `docs/remote-worker-architecture.md`

## Test Cases

- [ ] 使用者可直接用 gstack 指令完成 coding workflow，而不需手動選 tier 或 golem。
- [ ] Copilot 與 Gemini 對同一 repo 可共享 state 與 task contract。
- [ ] plan files 不再承載長對話，只保留執行所需資訊。
- [ ] 舊的 `gal-*` 與 `/gal plan` 等入口即使暫時存在，也只作為兼容層而非新的正式操作面。
- [ ] `/gal` 不會再與 gstack 指令形成平行的 coding workflow command surface。
- [ ] 遠端 execution plane 可在同一套 contract 下運作。

## Success Criteria

- [ ] GAL 成為 control plane，而不是 prompt / adapter 倉庫。
- [ ] Coding workflow 的主要指令面直接採用 gstack 指令。
- [ ] T0 / T1 / T2 完全移除。
- [ ] gstack 被接入 GAL，而不是與 GAL 並排疊加。
- [ ] repo-local state 與遠端 execution plane 能一致接線。

## Risks and Open Questions

- 高層操作層若設計不好，可能只是把舊複雜度換名字。
- 若 escape hatch 保留過多，可能重新膨脹成主要介面。
- Copilot 與 Gemini 的 host 差異可能讓 parity 只能做到 contract 層，而不是功能層。
- 遠端派工若太早接入，會把未完成的 orchestrator 放大成分散式混亂。

## Approval

- Architect verdict: pending
- Other required reviewers: pending
- Human approval: pending

---

## Status

Workflow: DRAFT
Step: 0 of 5
Last activity: 2026-03-30 — plan rewritten from scratch as a durable architecture plan
Next step: rewrite the public workflow and command docs to match this target architecture

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |

### Handoff Notes

本計畫只保留持久決策與可執行工作，不保留對話歷程。重點是把 GAL 從舊的 dispatcher + tier + golem 公開模型，改成以 repo-local state 與 control-plane 為核心，並讓 coding workflow 直接使用 gstack 指令。現有 `gal-*` 與舊 `/gal` 子命令可作為過渡期兼容層，但不應誤認為最終 public API。

## Test Results

[由 tester golem 在 TEST 階段填寫]

## Review Results

[由 reviewer golem 在 REVIEW 階段填寫]

## Debug Log

[若實作期間需要 debugger 介入，於此記錄]
