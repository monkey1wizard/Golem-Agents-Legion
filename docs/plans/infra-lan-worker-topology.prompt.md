# Plan: GAL 遠端執行平面

## Goal

建立 GAL 的遠端執行平面，讓主控制面可把適合的任務分派到 Windows sub PC / notebook 與未來 Mac Mini 執行，並以同一套 task contract、state model 與結果回寫規則運作。

本計畫的重點不是單純「能 SSH 到另一台機器」，而是把多台設備的責任邊界定清楚：哪類任務應留在主機、哪類任務可 offload 到 burst worker、哪類任務應交給 always-on endpoint。

## Tier

T2

## Review Pack

- architect-full
- reviewer

## Requirements

- [x] 本計畫是 GAL control plane 的 execution-plane 子計畫。
- [x] 在任何 remote dispatch 發生前，必要的 canonical artifacts 必須先存在：`.dev/project.md`、`.dev/state.md`、`docs/plans/<plan-slug>.md`、`docs/plans/<plan-slug>.prompt.md`。
- [x] 必須明確區分兩種 worktree class：primary feature worktree 與 disposable remote worker worktree。
- [ ] 遠端執行面已服從 `/gal` orchestrator 的 task contract 與 policy 決策。
- [ ] 使用者不需要手動指定 endpoint、tier 或 golem 就能派工。
- [ ] 遠端派工已能承接由 GAL 專家指令（specialist commands）產生的 task artifact 與結果回寫需求。
- [ ] 任務提交、執行、回收 contract 已完成 live verification。MVP 至少涵蓋 spec、status、summary、worker log、patch。
- [ ] Mac Mini 可作為外出時的 Discord / Telegram async intake endpoint，但訊息入口只能轉譯成同一份 task contract，不可形成第二套 workflow。
- [x] branch return 不屬於 MVP 必要條件，僅能在 patch-first 流程穩定後再列入後續擴充。
- [x] 第一個 worker class 鎖定為 Windows LAN + SSH + PowerShell + Gemini CLI。
- [ ] Mac Mini 已能以相同 contract 加入為 async endpoint。
- [x] 遠端 worker 不得維護第二套平行狀態系統。執行期資料應落在 temp / ephemeral artifact，而非 repo tracked state。
- [x] `.dev/state.md` 是 repo-level index 與 continuity artifact，不是 per-task log。
- [x] `docs/plans/<plan-slug>.prompt.md` 是詳細的 per-task execution state，coding、debug、test、review、handoff 的細部進度應優先寫入此檔案。
- [x] formal progress query 與 live remote-task query 必須分開定義：前者讀 canonical artifacts，後者讀 task-scoped temp artifacts。
- [x] plan section 的 remote write policy 必須以 section / command 為單位明確定義，而不是默許 remote worker 可任意寫入 plan。

## Execution Partitioning Model

### State Layers

| Layer | Artifacts | 用途 | 查詢方式 |
| --- | --- | --- | --- |
| Repo-level canonical state | `.dev/project.md`, `.dev/state.md` | repo 摘要、active plans、blockers、session continuity | 先讀 `.dev/state.md` 判斷目前 repo-level 位置 |
| Plan-level execution state | `docs/plans/<plan-slug>.md`, `docs/plans/<plan-slug>.prompt.md` | 單一 feature / task 的範圍、workflow step、分析、測試、review、handoff | 讀 active plan 的 `.prompt.md` 取得詳細進度 |
| Remote runtime artifacts | `task.md`, `status.json`, `summary.md`, `worker.log`, `result.patch` | 單次 remote task 的執行中狀態與回收結果 | 讀 task-scoped temp artifacts，而不是 `.dev/state.md` |

### Worktree Classes

| Worktree Class | 角色 | 可以寫什麼 | 不可以寫什麼 |
| --- | --- | --- | --- |
| Primary Feature Worktree | 目前正在實作 / debug / test 該 feature 的主要工作樹 | code、active plan prompt、必要時 `.dev/state.md`、允許的 canonical docs | 不應把 remote temp artifacts 當成正式 state |
| Disposable Remote Worker Worktree | 由 sub PC / notebook / Mac Mini 建立的暫時執行工作樹 | bounded task 的 code / docs 變更、task-scoped temp artifacts、允許的 patch-first outputs | 不直接擁有 `.dev/state.md`，不默許直接擁有 plan sections |

### Node Roles

| Node Class | 主要責任 | 適合的任務 | 不該承擔的責任 |
| --- | --- | --- | --- |
| Main PC (Control Plane) | `/gal` 狀態投影、任務判斷、task spec 產生、結果審核、patch 決策 | 互動式規劃、即時 coding、需要人類判斷的 review 與 merge 決策 | 長時間背景批次執行、把 worker state 當成正式 state source |
| Windows Burst Worker (sub PC / notebook) | 單次、可界定邊界的 headless 任務執行 | research、review、repo scan、docs 整理、可在單一 worktree 完成的文字導向工作 | 持久 queue/service、需要中途人工批准的任務、秘密資料過多的任務 |
| Mac Mini Async Endpoint | always-on 背景執行、夜間或長時間任務 | 長時間研究、批次掃描、可延後讀結果的 review / QA / docs 工作 | 重新定義另一套 contract、直接成為 control plane |

### Writer Ownership Matrix

| Artifact / Section | Primary Feature Worktree | Disposable Remote Worker Worktree | Notes |
| --- | --- | --- | --- |
| `.dev/project.md` | 少量、明確的 repo-level 更新 | 否 | 通常由 `/gal init` 或明確的 repo-level 文件工作更新 |
| `.dev/state.md` | 可更新，但只限 active-plan index、blockers、session continuity、next step 改變時 | 否 | repo-level index，不是 per-task log |
| `docs/plans/<plan-slug>.md` | 可更新 | 預設否 | source plan，不是 live runtime 狀態 |
| `docs/plans/<plan-slug>.prompt.md` | 是，為主要 writer | 預設否；僅在後續 policy 明確允許時才可透過 patch-first 提案 | 詳細 per-task state 的正式來源 |
| `docs/research/`, `docs/qa-reports/`, `docs/design-reports/` 等 durable outputs | 是 | 是，限 patch-first 任務 | remote 可產生這些檔案，但由 Main PC 審核整合 |
| `status.json`, `summary.md`, `worker.log` | 否 | 是 | 僅存在 task-scoped temp，不屬於 canonical state |

### Artifact Ownership And Commit Strategy

#### 1. Canonical-path outputs

對於 research、docs rewrite、repo scan、某些 QA / review report 類任務，remote worker 可以在自己的 worktree 中直接修改 canonical 路徑，例如 `docs/research/`。這些變更會透過 `git diff HEAD` 被收進 `result.patch`，由 Main PC 回收後審核與整合。

#### 2. Ephemeral task artifacts

`status.json`、`summary.md`、`worker.log`、以及 task-scoped 的 `result.patch` 副本，都屬於單次執行的 temp artifacts。它們用來回答「remote task 現在跑到哪裡」，不是 repo 的正式進度檔。

#### 3. Plan / control-plane state

plan prompt 與 `.dev/state.md` 是 canonical workflow state。即使 remote worker 回傳 findings、summary、patch，最終把結果翻譯回 `## Status`、`## Analyze`、`## Review Results`、`## Test Results` 或 `.dev/state.md` 的動作，仍應由 Main PC 在 primary feature worktree 收斂。

### Progress Query Method

#### Query 1: 現在整個 repo 的正式進度是什麼？

1. 先讀 `.dev/state.md`。
2. 從 active plans、blockers、session continuity 判斷目前 repo-level 狀態。
3. 再讀對應的 active plan prompt 取得該 feature 的細部進度。

#### Query 2: 某個 feature / plan 做到哪裡？

1. 讀 `docs/plans/<plan-slug>.prompt.md`。
2. 以 `## Status`、`## Tasks`、`## Analyze`、`## Test Results`、`## Review Results`、`### Handoff Notes` 為主。
3. source plan `.md` 只提供 scope / rationale，不是主要的 live progress 檔案。

#### Query 3: 某個 remote task 現在跑到哪裡？

1. 讀 task-scoped 的 `status.json`、`summary.md`、`worker.log`。
2. 不要把 `.dev/state.md` 或 plan prompt 當成 remote task 的即時監控來源。
3. remote task 回收後，再由 Main PC 把正式狀態收斂回 canonical artifacts。

#### Example: 基本 coding 完成，但尚未 test

- active plan prompt 應立即更新，例如 `## Status` 從 implementing 轉成 test-pending。
- `.dev/state.md` 只在 next step、active-plan index、blocker、session continuity 改變時更新。
- 不需要把每個 coding 細節寫進 `.dev/state.md`。

#### Example: debug 中

- active plan prompt 應更新 `## Status`、`## Analyze`，必要時更新 `### Handoff Notes`。
- `.dev/state.md` 只在 debug 已形成 repo-level blocker、跨 plan 決策、或新的 continuity 需求時更新。
- remote debug assistance 若存在，應回傳 findings artifacts，再由 Main PC 決定如何寫回 plan。

### Endpoint Selection Policy

| 任務特性 | 應派到哪裡 | 原因 |
| --- | --- | --- |
| 高互動、需求還在變、需要人即時看輸出 | Main PC | 這類任務需要控制面與作者共用同一上下文 |
| 有明確 task spec、一次跑完、失敗可重試 | Windows Burst Worker | 最適合 burst offload，能把主機畫面留給互動工作 |
| 長時間、可背景跑、結果稍後再收 | Mac Mini Async Endpoint | always-on 節點才適合承接夜間與延遲回收任務 |
| 需要 secrets、瀏覽器登入、人類 approval、衝突解決 | 暫留 Main PC | 目前 remote worker contract 還不適合處理中途人工介入 |

### Specialist Command Offload Policy

| Command / Output Class | Offload Policy | Canonical Write Strategy |
| --- | --- | --- |
| `/gal research`, bounded repo scan, docs rewrite | 可 offload | remote 直接產生 canonical-path outputs，Main PC 以 patch-first 整合 |
| `/review`, `/qa` | 條件式 | remote 可執行 bounded analysis 或 test run，但正式 `## Review Results` / `## Test Results` 預設由 Main PC 回寫 |
| `/plan-eng-review`, `/office-hours`, `/ship`, `/gal wrap-up` | Main PC only | 這些命令直接擁有 plan sections 或 repo-level state，不應交給 disposable remote worker |
| `## Status`, `## Tasks`, `## Analyze` 的常規更新 | Primary Feature Worktree only | 屬於主要 workflow state，不預設由 remote worker 持有 |

### Mac Mini Resource Allocation

| Runtime / Tool | 在 Mac Mini 的角色 | 是否屬於目前 remote contract |
| --- | --- | --- |
| Gemini CLI | Mac Mini async endpoint 的唯一 headless worker engine | 是 |
| `Start-GalWorker.sh` | Mac 專用 worker adapter，負責產生與 Windows 相同的 artifacts | 是 |
| Discord / Telegram bridge | 外出時的人類遠端入口，將請求轉成 bounded task spec | 否，屬於 intake layer |
| Copilot CLI、VS Code、Codex CLI | 已安裝於 Mac Mini，供互動式或手動操作使用 | 否 |
| MLX-LM + Gemma 4 / Breeze 2 | Apple Silicon 本機推理 lane，承接 LOCAL / private 類任務 | 否 |
| OpenClaw | 不安裝，不納入規劃 | 否 |

Mac Mini 的設計原則是「同一份 task/result contract，不同 worker adapter」。目前 execution plane 不把 Copilot CLI、VS Code 或 Codex CLI 納入自動 dispatch；自動化任務仍以 Gemini CLI 為唯一 headless engine。Discord / Telegram 若作為外出入口，也只能把請求送進同一條 bounded task path，而不是直接繞過 control plane 執行任意命令。

### Local AI Allocation

| Model / Lane | 適合承接的工作 | 不應承接的工作 |
| --- | --- | --- |
| Gemma 4 | 通用摘要、分類、草稿整理、多語內容壓縮、低風險的背景前處理 | 需要 repo-wide correctness 的最終判斷、正式 review 結論、canonical state 寫回 |
| Breeze 2 | 台灣繁中語氣修正、筆記整理、私有資料初步歸類、Obsidian 類在地中文任務 | 高風險架構判斷、複雜跨檔推理、正式測試/安全審核 |
| Gemini CLI | headless remote worker、需要較高穩定性的 research / review / docs 任務 | 不應與 LOCAL lane 混為同一層 contract |

### How To Do It

1. 在 primary feature worktree 中，先確保 `.dev/project.md`、`.dev/state.md`、source plan、active plan prompt 已建立且足以描述目前工作。
2. 若 local workflow state 已發生變化（例如 coding 完成但 tests 尚未執行、或 debug 已開始），先更新 active plan prompt；若 repo-level index / blocker / continuity 改變，再更新 `.dev/state.md`。
3. Main PC 再由 `/gal` 或相應 specialist command 判斷任務是否值得 offload。
4. Main PC 產生單一 task spec，內容只描述目標、限制、輸出要求與必要讀取檔案。task spec 應足以讓 worker 執行 bounded task，而不是把 plan ownership 一併交出去。
5. Control plane 依任務特性選 endpoint：Main PC、Windows burst worker、或未來的 Mac Mini async endpoint。
6. 若選 remote endpoint，Main PC 透過 SSH/SCP 與 endpoint profile（OS、shell、temp root、worker entry、repo path）把 task spec 送到遠端 temp 目錄，並在 remote 端建立 disposable worktree。
7. 遠端 endpoint 在 linked git worktree 中執行單次任務，Windows 使用 `Start-GalWorker.ps1`，Mac Mini 使用 `Start-GalWorker.sh`；兩者都必須輸出 `status.json`、`summary.md`、`worker.log`、`result.patch`。若 task type 允許，也可在 canonical 路徑產生 patch-first outputs。
8. 遠端 worker 不直接擁有 `.dev/state.md`；對 plan prompt 也採 default deny，除非後續 policy 對特定 section / command 明確開放。
9. Main PC 回收 artifacts，檢查狀態、閱讀 summary、審核 patch，必要時重派或拆小任務。
10. Main PC 在 primary feature worktree 中完成最後的 state convergence：更新 plan prompt、必要時更新 `.dev/state.md`、再決定是否整合 patch。

### Current Reality vs Target Model

| 項目 | 目前 repo 狀態 | 目標狀態 |
| --- | --- | --- |
| Windows burst worker 腳本 | 已有 | 需完成 live E2E 驗證 |
| Primary feature worktree vs disposable remote worker worktree 的 ownership 規則 | 本 plan 已開始定義，但 supporting docs 尚未全部同步 | 成為跨文件一致規則 |
| formal progress query 與 live remote-task query 的分流 | 本 plan 已開始定義，但尚未經 supporting docs 驗證 | 查正式進度與查即時任務狀態時不再混淆 |
| `/gal` 自動決定 offload | 尚未實作 | 由 control plane 依 task type 自動選 endpoint |
| 多 endpoint registry / health check | 尚未實作 | 可列出 notebook、sub PC、Mac Mini 的可用性與角色 |
| Mac Mini async endpoint | SSH 可連入；Gemini CLI、Copilot CLI、VS Code、Codex CLI 已安裝；bash worker 與 endpoint profile abstraction 尚未實作 | 實際可接單、回報狀態、回收結果 |
| Mac Mini local inference lane | 方向已確認改用 MLX-LM，不使用 OpenClaw；尚未接到 control-plane policy | LOCAL / private 任務可由 Apple Silicon 本機推理承接，但不與 Gemini headless contract 混用 |
| Discord / Telegram remote intake | 尚未實作 | 外出時可把 bounded task 送到 Mac Mini，再由 control plane 決定是本機處理、排隊，或轉回主工作流 |

## Phases

### P1: Contract And Worker Semantics

- **Scope**
	- 鎖定 repo-level canonical state、plan-level execution state、remote runtime artifacts 三層模型。
	- 鎖定 primary feature worktree 與 disposable remote worker worktree 的 ownership 邊界。
	- 鎖定第一版 task / result contract，MVP 只要求 `status.json`、`summary.md`、`worker.log`、`result.patch`。
	- 鎖定 worktree lifecycle 與 runtime status 的落點，避免把執行期資料寫進 repo tracked state。
	- 定義 remote worker 只負責執行，不擁有第二套 state model。

- **Files**: `docs/remote-worker-architecture.md`, `docs/per-repo-context.md`, `templates/task.md`
- **Repo State**: plan 層面的 ownership 規則已在本檔定義，但 supporting docs 尚未全部同步，完整 live verification 也尚未完成。
- **Verify**: 至少完成一次真實的 Windows remote worker 任務，並確認三層 state model 與 worktree ownership 邊界在操作上可成立。

### P2: Windows Burst Worker MVP

- **Scope**
	- 讓主 PC 可把單次任務派送到 Windows sub PC / notebook。
	- 使用 git worktree 作為 MVP 的唯一隔離模型，不將 branch return 納入第一版成功條件。
	- 補齊 timeout、retry、failure status 與 log policy。
	- 驗證 disposable remote worker worktree 只回傳 artifacts / patch，而不直接成為 canonical state owner。
	- 先支援 research / review / repo scan / docs 類任務。

- **Files**: `scripts/Invoke-GalRemoteTask.ps1`, `scripts/Start-GalWorker.ps1`, `scripts/Get-GalRemoteResult.ps1`, `docs/runtime-verification.md`
- **Repo State**: 三支腳本已存在，但完整 dispatch -> execute -> retrieve 的 live 驗證仍待完成。
- **Verify**: 主控制面可派送 research / review 類任務到某台 Windows worker，並正確回收 `status + summary + raw log + patch`。

### P3: Control-Plane Offload Policy

- **Scope**
	- 把「是否 offload、派到哪台機器」從人工作業提升成 control-plane policy。
	- 定義 endpoint registry、health check、task type -> endpoint class 的對應規則。
	- 定義 specialist command 的 offload matrix，以及哪些 plan sections / state writes 只能在 primary feature worktree 發生。
	- 讓 `/gal` 與 specialist commands 產生的 task artifact 能透過同一 decision path 被送往 remote endpoint。

- **Files**: `docs/remote-worker-architecture.md`, `docs/gal-control-plane-contracts.md`, `docs/command-dispatch-architecture.md`, `model-roles.example.md`
- **Verify**: 不需使用者手動指定「這次送 notebook 還是 Mac Mini」，control plane 可依 task 特性做出可解釋的 routing 決策。

### P4: Mac Mini Async Endpoint

- **Scope**
	- 讓 Mac Mini 成為 always-on async endpoint。
	- 沿用 P1 / P2 / P3 contract，不重新定義 workflow。
	- 新增 `Start-GalWorker.sh`，以 bash 取代 PowerShell 作為 Mac worker entrypoint。
	- 在 control plane 引入 endpoint profile abstraction，至少包含 OS、shell、temp root、worker entry、repo path、timeout policy。
	- 鎖定 Gemini CLI 為 Mac Mini 的唯一 headless worker engine；Copilot CLI、VS Code、Codex CLI 只視為互動式工具，不納入 MVP remote dispatch。
	- 定義 Discord / Telegram 作為外出時的 async intake adapter，但它們只能把人類請求轉成同一份 task contract，不可變成第二套 side channel。
	- 把 MLX-LM 定位為 Apple Silicon LOCAL lane，而不是 execution plane 的第二套 worker contract。
	- 為 Gemma 4 與 Breeze 2 分配 LOCAL lane 工作：Gemma 4 偏通用摘要/分類；Breeze 2 偏繁中在地化整理、筆記與私有資料前處理。
	- 定義主要節點、備援節點與 endpoint health 檢查。

- **Files**: `docs/remote-worker-architecture.md`, `docs/installation-topology.md`, `model-roles.example.md`, `scripts/scripts.md`
- **Repo State**: Mac Mini 已確認安裝 Gemini CLI、Copilot CLI、VS Code、Codex CLI，但 bash worker、endpoint profile abstraction 與 live verification 尚未完成。
- **Verify**: 同一 contract 可讓 Windows burst worker 與 Mac Mini async endpoint 執行不同類型的任務；Mac worker 需以 bash 產出與 Windows 相同的 artifacts；Discord / Telegram intake 只能送出 bounded tasks，不可直接形成另一套執行面。

## Files to Create or Modify

| File | 目前狀態 | 還缺什麼 |
| --- | --- | --- |
| `docs/plans/infra-lan-worker-topology.prompt.md` | 正在修訂 | 鎖定 state layers、worktree classes、query method、writer ownership matrix |
| `docs/remote-worker-architecture.md` | 已存在 | 補上 multi-endpoint policy、Main PC / Windows / Mac Mini 分工與 health model |
| `docs/installation-topology.md` | 已存在 | 補上 Mac Mini 作為 async endpoint 的實際接線方式 |
| `docs/per-repo-context.md` | 已存在 | 確認 remote task artifact 與 canonical artifact 的 ownership boundary |
| `templates/task.md` | 已存在 | 補上 endpoint-neutral task spec 約束 |
| `scripts/Invoke-GalRemoteTask.ps1` | 已存在 | 完成 live validation，未來支援 endpoint selection abstraction |
| `scripts/Start-GalWorker.ps1` | 已存在 | 補強 summary extraction、timeout、錯誤可觀測性 |
| `scripts/Start-GalWorker.sh` | 尚未建立 | 實作 Mac bash worker，輸出與 Windows worker 完全相同的 artifact contract |
| `scripts/Get-GalRemoteResult.ps1` | 已存在 | 補強回收與 cleanup 的穩定性驗證 |
| `scripts/scripts.md` | 已存在 | 補上多 endpoint 操作方式 |
| `model-roles.example.md` | 已存在 | 補上 Main PC / burst worker / async endpoint 的角色映射 |
| `docs/runtime-verification.md` | 已存在 | 補上完整 Windows E2E 驗證結果與未來 Mac Mini 驗證紀錄 |

### Supporting Context Already In Repo

- `docs/gal-control-plane-contracts.md` — `/gal` 控制面指令的 read/write contract
- `docs/gstack-command-contracts.md` — specialist command 的 read/write contract 與 artifact mapping
- `docs/gstack-integration.md` — GAL 採用 gstack-style specialist semantics 的整合模型
- `docs/command-dispatch-architecture.md` — control plane 與 dispatcher 的角色邊界
- `docs/research/lan-worker-feasibility.md` — LAN worker 可行性研究與 MVP scope cut 建議

## Test Cases

- [ ] 在 remote dispatch 前，primary feature worktree 已具備必要 canonical artifacts：`.dev/project.md`、`.dev/state.md`、source plan、active plan prompt。
- [ ] 在 primary feature worktree 中，coding complete but tests pending 會先更新 active plan prompt；`.dev/state.md` 只在 next step、blocker、active-plan index 或 continuity 改變時更新。
- [ ] 在 debug flow 中，active plan prompt 會更新 `## Status`、`## Analyze`、必要時 `### Handoff Notes`；`.dev/state.md` 只在 repo-level 狀態改變時更新。
- [ ] formal repo progress 可透過 `.dev/state.md` + active plan prompt 重建；live remote-task status 則透過 `status.json` / `summary.md` / `worker.log` 觀察。
- [ ] 任一 Windows burst worker 能在隔離工作樹中執行單次任務。
- [ ] 任務失敗會產生可讀狀態與錯誤結果。
- [ ] MVP 僅依賴 `status + summary + raw log + patch` 即可完成結果回收，不要求 branch return。
- [ ] disposable remote worker 不直接成為 `.dev/state.md` owner，且不默許直接寫入 active plan prompt。
- [ ] orchestrator 可根據 task 特性決定是否 offload，並解釋為何選 Main PC、Windows worker 或 Mac Mini。
- [ ] Mac Mini 可在相同 contract 下承接長時間背景任務。
- [ ] 外出時可透過 Discord / Telegram 把 bounded task 送到 Mac Mini，且該入口仍遵守 control-plane policy。
- [ ] Gemma 4 與 Breeze 2 的 LOCAL lane 任務邊界清楚，不會與 Gemini headless contract 混淆。

## Success Criteria

- [ ] 遠端執行面成為 `/gal` orchestrator 的延伸能力，而不是另一套 side channel。
- [ ] primary feature worktree 與 disposable remote worker worktree 的責任邊界清楚，且不產生 split-brain state。
- [ ] Windows burst worker 可穩定承接 bounded text-oriented 任務，且不污染 canonical state。
- [ ] 查 repo 正式進度、查單一 plan 進度、查 live remote-task 狀態三者的方法清楚且不互相混淆。
- [ ] Main PC、Windows sub PC / notebook、Mac Mini 可納入同一控制平面，但保有清楚責任邊界。
- [ ] 外出情境下的 Discord / Telegram 遠端入口是 intake layer，不會變成 control plane 之外的 side channel。

## Risks and Open Questions

- notebook / sub PC 的 SSH、電源與網路穩定性是否足以支撐 burst worker 模式。
- Gemini CLI 在單點腳本層面可用，但完整無人值守 dispatch 是否穩定，仍需 live verification。
- 長時間任務是否需要 queue / service，而不只是單次 SSH fan-out。
- branch return 是否真的值得引入，或應持續維持 patch-first 回收策略。
- 遠端可見權限如何控制，避免敏感資訊過度暴露。
- 哪些 plan sections 若未來真的要允許 remote patch-first 回寫，才能在不造成 split-brain state 的前提下成立。
- endpoint registry 應放在什麼層級管理，才能不把 per-machine 細節硬編進 control plane。
- Discord / Telegram bridge 應該直接掛在 Mac Mini，還是先進一層 task inbox / queue，再交給 control plane 判斷。
- Gemma 4 與 Breeze 2 應採 CLI、服務模式，還是輕量 daemon，才能兼顧 Apple Silicon 效能與維運成本。

## Approval

- Architect verdict: pending
- Other required reviewers: pending
- Human approval: pending

---

## Status

Workflow: IMPLEMENT
Step: 2 of 4
Last activity: 2026-04-09 — 補充 Mac Mini 還承擔外出情境的 Discord / Telegram async intake；Apple Silicon LOCAL lane 明確規劃 Gemma 4 與 Breeze 2 的任務分工；plan 與長期筆記已同步這些決策
Next step: 先完成 Windows burst worker E2E 驗證，再落地 endpoint profile abstraction、`Start-GalWorker.sh` 與 Discord / Telegram intake adapter，讓 Mac Mini 能以同一 contract 進入 live verification

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |
| 舊版狀態判定 | 把多項 requirement 與 test case 標為已完成 | 全數退回到「待 live verification」或「待 control-plane policy 實作」 | repo 目前擁有的是 contract 與腳本，不是已完成的多機 orchestration rollout |
| 舊版 phase 結構 | 直接從 Windows notebook MVP 跳到 Mac Mini | 插入獨立的 control-plane offload policy phase | 多台設備分工的核心不是「再多一台機器」，而是先定義誰負責選 endpoint 與為何這樣選 |
| worktree 方法定義 | 原先把所有 worktree 視為同一類 | 明確區分 primary feature worktree 與 disposable remote worker worktree | feature 開發的 canonical state 更新，與 remote bounded execution 的 artifact 回收，不應混為同一 ownership 模型 |

### Handoff Notes

本計畫只處理 execution plane，不承擔核心 state model 與 operation layer 的定義工作。所有遠端派工都必須由 `/gal` control plane 先判斷任務是否值得 offload，再產出 endpoint-neutral task contract 給 remote endpoint 執行。specialist commands 採用 GAL 原生重新實作的 gstack workflow 語意，但它們不應各自維護另一套 remote execution policy。

本檔目前新增的核心方法定義如下：

1. `primary feature worktree` 與 `disposable remote worker worktree` 是兩種不同 ownership class。
2. `.dev/state.md` 是 repo-level index / continuity，不是 per-task log。
3. `docs/plans/<plan-slug>.prompt.md` 是詳細的 per-task execution state。
4. formal progress query 讀 canonical artifacts；live remote-task query 讀 task-scoped temp artifacts。
5. remote worker 允許對某些 canonical docs 採 patch-first，但不預設擁有 `.dev/state.md` 或 plan sections。
6. Mac Mini 採 bash worker adapter，仍服從同一份 artifact contract；Gemini CLI 是唯一 headless engine。
7. Copilot CLI、VS Code、Codex CLI 雖已安裝於 Mac Mini，但目前不納入 automated remote dispatch。
8. MLX-LM 屬於 Apple Silicon LOCAL lane，不與 execution plane 的 Gemini contract 混為同一層。
9. Discord / Telegram 在規劃中只作為遠端人類入口，必須轉譯成 bounded task spec 再交給 control plane。
10. Gemma 4 與 Breeze 2 屬於 LOCAL lane 模型，不直接成為 remote worker engine。

目前 repo 已有 contract 文件與 Windows worker 腳本，尚未完成的核心工作是四件事：

1. 完成一次真實的 Windows burst worker 端到端驗證。
2. 把 endpoint selection 與 specialist command offload matrix 從人工作業提升為 control-plane policy。
3. 落地 endpoint profile abstraction，讓 Windows 與 Mac 可共用 dispatch/retrieve 流程，但各自選用正確的 shell 與 worker entry。
4. 實作 `Start-GalWorker.sh`，讓 Mac Mini 以 bash 產出與 Windows 相同的 artifacts。
5. 持續同步 supporting docs，讓 `per-repo-context`、`remote-worker-architecture`、`runtime-verification` 對上述 ownership 規則保持一致。

## Test Results

[由 tester golem 在 TEST 階段填寫]

## Review Results

[由 reviewer golem 在 REVIEW 階段填寫]

## Debug Log

[若實作期間需要 debugger 介入，於此記錄]
