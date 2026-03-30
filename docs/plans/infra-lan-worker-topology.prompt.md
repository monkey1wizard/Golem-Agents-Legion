# Plan: GAL 遠端執行平面

## Goal

建立 GAL 的遠端執行平面，讓主控制面可將適合的任務派送到 notebook 與未來 Mac Mini 執行，並以同一套 task contract、state model 與結果回寫規則運作。

## Tier

T2

## Review Pack

- architect-full
- designer
- reviewer

## Requirements

- [ ] 本計畫是 `docs/plans/gal-cross-ai-orchestrator.prompt.md` 的 execution-plane 子計畫。
- [ ] 遠端執行面必須服從 `/gal` orchestrator 的 task contract 與 policy 決策。
- [ ] 使用者不需要手動指定 tier 或 golem 才能派工。
- [ ] 遠端派工必須能承接由 gstack coding workflow 指令產生的 task artifact 與結果回寫需求。
- [ ] 任務提交、執行、回收必須有穩定 contract；MVP 至少涵蓋 spec、status、summary、worker log、patch。
- [ ] branch return 不屬於 MVP 必要條件，僅能在 patch-first 流程穩定後再列入後續擴充。
- [ ] notebook MVP 先以 Windows LAN + SSH + PowerShell + Gemini CLI 為主。
- [ ] Mac Mini 後續必須能以相同 contract 加入為 async endpoint。
- [ ] 遠端 worker 不得維護第二套平行狀態系統。

## Phases

### P1: Engine Preflight And Contract

- **Scope**
	- 驗證 Gemini CLI 可作為第一個 headless worker engine。
	- 定義第一版 task / result contract，MVP 只要求 `status.json`、`summary.md`、`worker.log`、`result.patch`。
	- 鎖定 worktree lifecycle 與 runtime status 的落點，避免把執行期資料寫進 repo tracked state。
	- 先完成一次真正的無人工介入 preflight，再進入腳本化派工。

- **Files**: `docs/remote-worker-architecture.md`, `docs/per-repo-context.md`, `templates/task.md`
- **Verify**: notebook 上可完成一次 headless Gemini CLI 任務，且 task / result contract 與 worktree lifecycle 已固定。

### P2: Windows LAN MVP

- **Scope**
	- 定義主 PC 到 notebook 的單次派工與結果回收流程。
	- 使用 git worktree 作為 MVP 的唯一隔離模型，不將 branch return 納入第一版成功條件。
	- 補齊 timeout、retry、failure status 與 log policy。
	- 先支援 research / review / repo scan / 文件整理等文字導向任務。

- **Files**: `docs/remote-worker-architecture.md`, `docs/per-repo-context.md`, `templates/task.md`, `scripts/Invoke-GalRemoteTask.ps1`, `scripts/Start-GalWorker.ps1`, `scripts/Get-GalRemoteResult.ps1`
- **Verify**: 主控制面可派送 research / review 類任務到 notebook 並正確回收 `status + summary + raw log + patch`。

### P3: Mac Mini Endpoint

- **Scope**
	- 讓 Mac Mini 成為 always-on async endpoint。
	- 沿用 P1 / P2 contract，不重新定義 workflow。
	- 定義主要節點、備援節點與 endpoint health 檢查。

- **Files**: `docs/remote-worker-architecture.md`, `docs/installation-topology.md`, `model-roles.example.md`, `scripts/scripts.md`
- **Verify**: 同一 contract 可讓 notebook 與 Mac Mini 執行不同類型的任務。

## Files to Create or Modify

- `docs/remote-worker-architecture.md`
- `docs/installation-topology.md`
- `docs/per-repo-context.md`
- `templates/task.md`
- `scripts/Invoke-GalRemoteTask.ps1`
- `scripts/Start-GalWorker.ps1`
- `scripts/Get-GalRemoteResult.ps1`
- `scripts/scripts.md`
- `model-roles.example.md`

## Test Cases

- [ ] 主控制面可在不切換到 notebook 畫面的前提下派送任務。
- [ ] 遠端 worker 能在隔離工作樹中執行。
- [ ] 任務失敗會產生可讀狀態與錯誤結果。
- [ ] MVP 僅依賴 `status + summary + raw log + patch` 即可完成結果回收，不要求 branch return。
- [ ] orchestrator 可根據 task 特性決定是否 offload。
- [ ] Mac Mini 可在相同 contract 下承接長時間背景任務。

## Success Criteria

- [ ] 遠端執行面成為 `/gal` orchestrator 的延伸能力，而不是另一套 side channel。
- [ ] Gemini CLI 被驗證為第一個可用 worker engine，但 contract 不綁死在 Gemini。
- [ ] notebook 與 Mac Mini 可納入同一控制平面。

## Risks and Open Questions

- notebook 的 SSH、電源與網路穩定性是否足以支撐 worker 模式。
- Gemini CLI 的非互動穩定性是否足以承擔自動派工。
- 長時間任務是否需要 queue / service，而不只是單次 SSH fan-out。
- branch return 是否真的值得引入，或應持續維持 patch-first 回收策略。
- 遠端可見權限如何控制，避免敏感資訊過度暴露。

## Approval

- Architect verdict: pending
- Other required reviewers: pending
- Human approval: pending

---

## Status

Workflow: DRAFT
Step: 0 of 3
Last activity: 2026-03-30 — plan rewritten from scratch as a durable execution-plane plan
Next step: complete Gemini CLI preflight and lock the first patch-first task contract

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |

### Handoff Notes

本計畫只處理 execution plane，不承擔核心 state model 與 operation layer 的定義工作。所有遠端派工都必須由 `/gal` orchestrator 先完成工作判斷，再產出 task contract 給 endpoint 執行；coding workflow 本身則直接使用 gstack 指令面。MVP 先採 patch-first 回收，不把 branch return 當成第一階段成功條件。

## Test Results

[由 tester golem 在 TEST 階段填寫]

## Review Results

[由 reviewer golem 在 REVIEW 階段填寫]

## Debug Log

[若實作期間需要 debugger 介入，於此記錄]
