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

## Current Boundary

這份計畫現在不再從零開始追蹤整個整合。`docs/plans/gal-coding-workflow-native.prompt.md` 已先完成大量重疊工作，特別是：

- `/gal` control-plane contract 與 public command surface 文件
- gstack-style specialist command catalog 與對應 `commands/*/SKILL.md`
- `README.md`、`docs/gstack-integration.md`、`docs/command-dispatch-architecture.md` 等 public docs 重寫
- Copilot / Gemini 安裝拓撲與 shared adapter model 的大部分文件化

本計畫現在只追蹤剩餘的 orchestrator 級整合缺口：

- `scripts/gal.ps1` / `scripts/gal.sh` 仍以舊 dispatcher + `plan/next/pause/sync` 公開入口為主
- `scripts/Sync-DevContext.ps1` / `scripts/sync-dev-context.sh` 仍要求 `## Active Skills` 並以 `/gal sync` 為使用者可見語意
- `workflows/coding.md`、`model-roles.md`、`agent/agents.md`、`templates/state.md` 仍保留 tier / golem 為主要 public model
- `.dev/` 的完整 repo-local state layer 尚未補齊 `requirements.md`、`roadmap.md`、`summary.md`、`threads/` 等落點
- execution-plane contract 與遠端 worker artifacts 尚未落地

## Requirements

- [ ] `/gal` 必須從 dispatcher 升級為 orchestrator。
- [ ] Copilot 與 Gemini 必須共享同一套 state model、task contract 與 command semantics。
- [ ] `.dev/` 必須擴充為完整的 repo-local state layer。
- [x] Coding workflow 的主要指令面必須直接使用 gstack 指令，而不是由 GAL 重新定義一套平行命名。
- [ ] `/gal` 只保留 control-plane 職責，不再承擔 coding workflow 的主要指令面。
- [ ] T0 / T1 / T2 不保留。
- [ ] gstack 的價值應被接入 GAL 的 state、task contract 與 review packs，而不是只當外部參考。
- [ ] plan files 只保留執行所需內容，不承載完整對話歷程。
- [ ] 多機 execution plane 必須服從同一套 orchestrator contract。

## Phases

### P1: Repo-Local State Model

- **Status**: partial
- **Scope**
	- 補齊 `.dev/project.md`、`.dev/state.md` 以外的 canonical state 落點：`.dev/requirements.md`、`.dev/roadmap.md`、`.dev/summary.md`、`.dev/threads/`。
	- 收斂 plan files 與 `.dev/` 的邊界，避免同一份狀態在多處重複。
	- 更新 templates，讓新的 state model 可直接被初始化與續接使用。

- **Already Landed**: `docs/per-repo-context.md`, `templates/project.md`, `templates/state.md`
- **Remaining Files**: `templates/plan.md`, `templates/requirements.md`, `templates/roadmap.md`, `templates/summary.md`, `.dev/threads/` contract docs
- **Verify**: 同一個任務可在不依賴長 chat 的情況下續接，且不同狀態資訊都有固定落點。

### P2: Adapter Parity

- **Status**: partial
- **Scope**
	- 保留 generated adapters 為 disposable output，但把 `gal sync` 與靜態 `Active Skills` allowlist 徹底降級為內部 plumbing 或移除。
	- 讓 Copilot 與 Gemini 共享同一套 state / task contract，同時清楚標示 host capability asymmetry。
	- 將安裝與 repo-local adapter 語意對齊 final public model，而不是舊 GAL workflow。

- **Already Landed**: `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh`, `docs/installation-topology.md`
- **Remaining Files**: `scripts/Sync-DevContext.ps1`, `scripts/sync-dev-context.sh`, `scripts/scripts.md`
- **Verify**: 同一 repo 的狀態與 workflow 可被 Copilot 與 Gemini 一致理解，且使用者不需要再執行 public `gal sync` 流程。

### P3: Operation Layer And Role Model

- **Status**: mostly complete at docs/skill layer, incomplete at workflow convergence layer
- **Scope**
	- 承接前置計畫 `docs/plans/gal-coding-workflow-native.prompt.md` 已完成的 specialist command surface。
	- 把 golem 從 public mental model 收斂為內部編排角色，而不是使用者必須理解的第一層操作語彙。
	- 把 workflow、model roles、agent docs 與新的 control-plane / specialist split 對齊。

- **Prerequisite**: `docs/plans/gal-coding-workflow-native.prompt.md` 已完成其 P2–P4 範圍
- **Already Landed**: `docs/gstack-integration.md`, `commands/commands.md`
- **Remaining Files**: `agent/agents.md`, `model-roles.md`, `workflows/coding.md`
- **Verify**: 使用者可直接在 Copilot 中透過 GAL coding workflow skills 完成計畫審查與代碼審查，而 GAL 能持續維護 state 與 control-plane 邏輯，且 public docs 不再要求先理解 tier / golem。

### P4: `/gal` Orchestrator

- **Status**: not complete
- **Scope**
	- 將 `/gal` 從目前的 dispatcher + alias 兼容層，升級為正式的 control-plane orchestrator。
	- 收斂新的 control-plane 語意，覆蓋 `init`、`status`、`whats-next`、`wrap-up`、`research`。
	- 保留過渡期 alias 只做 discoverability，不再讓舊 `plan/next/pause/sync` 充當 public API。
	- 讓系統管理 state、runtime、offload、continuity 與結果回寫，而不是只輸出舊式 dispatch block。

- **Files**: `scripts/gal.ps1`, `scripts/gal.sh`, `docs/command-dispatch-architecture.md`, `commands/gal/SKILL.md`, `commands/gal-*/SKILL.md`
- **Verify**: `/gal` 可穩定承擔 control-plane 工作，新的 control-plane 操作面可覆蓋 `init / status / whats-next / wrap-up / research`，且不再暴露平行的舊 coding workflow command surface。

### P5: Execution Endpoints

- **Status**: not started in this plan; delegated as subplan, not yet implemented
- **Scope**
	- 將 notebook 與 Mac Mini 接入同一套 task contract。
	- 讓 orchestrator 根據任務特性決定是否 offload。
	- 避免遠端執行面長出第二套 workflow。

- **Related Subplan**: `docs/plans/infra-lan-worker-topology.prompt.md`
- **Files**: `docs/remote-worker-architecture.md`, `scripts/Invoke-GalRemoteTask.ps1`, `scripts/Start-GalWorker.ps1`, `scripts/Get-GalRemoteResult.ps1`
- **Verify**: 任務可由主控制面派送到遠端 endpoint，並正確回寫狀態與結果。

## Files to Create or Modify

- `templates/requirements.md`
- `templates/roadmap.md`
- `templates/summary.md`
- `workflows/coding.md`
- `model-roles.md`
- `agent/agents.md`
- `templates/state.md`
- `scripts/gal.ps1`
- `scripts/gal.sh`
- `scripts/Sync-DevContext.ps1`
- `scripts/sync-dev-context.sh`
- `scripts/scripts.md`
- `docs/remote-worker-architecture.md`
- `scripts/Invoke-GalRemoteTask.ps1`
- `scripts/Start-GalWorker.ps1`
- `scripts/Get-GalRemoteResult.ps1`

## Test Cases

- [x] 使用者可直接用 gstack 指令完成 coding workflow，而不需手動選 tier 或 golem。
- [ ] Copilot 與 Gemini 對同一 repo 可共享 state 與 task contract。
- [ ] plan files 不再承載長對話，只保留執行所需資訊。
- [ ] 舊的 `gal-*` 與 `/gal plan` 等入口即使暫時存在，也只作為兼容層而非新的正式操作面。
- [ ] `/gal` 不會再與 gstack 指令形成平行的 coding workflow command surface。
- [ ] 遠端 execution plane 可在同一套 contract 下運作。

## Success Criteria

- [ ] GAL 成為 control plane，而不是 prompt / adapter 倉庫。
- [x] Coding workflow 的主要指令面直接採用 gstack 指令。
- [ ] T0 / T1 / T2 完全移除。
- [ ] gstack 被接入 GAL，而不是與 GAL 並排疊加。
- [ ] repo-local state 與遠端 execution plane 能一致接線。

## Risks and Open Questions

- 若只重寫文件而不改 runtime，GAL 仍會停留在「文件是新模型、腳本是舊模型」的分裂狀態。
- `Sync-DevContext` 若繼續要求 `## Active Skills`，會讓 public docs 與實際操作語意持續矛盾。
- tier / golem 若只在部分文件退場，使用者仍會被迫理解兩套 mental model。
- 遠端派工若早於 orchestrator contract 收斂，會把當前語意分裂放大成分散式混亂。

## Approval

- Architect verdict: pending
- Other required reviewers: pending
- Human approval: pending

---

## Status

Workflow: IN PROGRESS
Step: 3 of 5
Last activity: 2026-03-31 — audited overlap with `gal-coding-workflow-native.prompt.md` and narrowed this plan to remaining orchestrator/runtime work
Next step: rewrite `scripts/gal.ps1` and `scripts/gal.sh`, then remove public reliance on `gal sync` and `## Active Skills`

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |
| P1-P3 | Start as a clean-sheet orchestrator integration | Large parts of command-surface and specialist-layer work were already delivered by `gal-coding-workflow-native.prompt.md` | Avoid duplicate tracking and keep this plan focused on residual orchestration gaps |
| P2 | Finish adapter parity after state-model work | Setup and topology docs landed before runtime cleanup | The installation surface was easier to stabilize than the old `gal sync` plumbing |
| P5 | Define execution plane directly inside this plan | Execution-plane work was split into `infra-lan-worker-topology.prompt.md` | Keep remote-worker contract design isolated from core `/gal` cleanup |

### Handoff Notes

`gal-coding-workflow-native` 已完成 command surface、specialist skills、README 與多數 integration docs 的重疊範圍。這份 plan 現在真正剩下的 blocker 是 runtime 與 public model 收斂：`scripts/gal.*` 仍暴露 `plan/next/pause/sync`；`Sync-DevContext.*` 仍要求 `## Active Skills`；tier / golem public model 仍殘留於 `workflows/coding.md`、`model-roles.md`、`agent/agents.md`、`templates/state.md`；execution-plane artifacts 尚未建立。只有在這些縫補完後，這份 orchestrator plan 才能關閉。

## Test Results

[由 tester golem 在 TEST 階段填寫]

## Review Results

[由 reviewer golem 在 REVIEW 階段填寫]

## Debug Log

[若實作期間需要 debugger 介入，於此記錄]
