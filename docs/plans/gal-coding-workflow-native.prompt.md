# Plan: GAL Coding Workflow 原生技能實作

## Goal

gstack 基於 Claude Code 執行環境，其 slash commands 無法在 Copilot 中直接運作。本計畫分兩層完成整合：

1. 先重做 GAL 的 control-plane command surface，讓 `/gal` 不再帶著 GSD 遺留命名，而是以使用者看得懂的 intent-led commands 暴露整合能力。
2. 再以 GAL-native skill files 實作最新 gstack README 所列、除 `/codex` 之外的完整 command surface，讓使用者在 GAL 中可直接使用同名 specialist operations。

最終效果不應是兩套 repo 並排，而應是：GAL control-plane 負責 repo bootstrap、state projection、continuity、routing、wrap-up、research；gstack-style commands 負責 coding workflow execution。執行面以 Copilot（主控制面）與 Gemini CLI（worker）為主。

## Context

`docs/gstack-integration.md` 的架構目標假設可直接呼叫 gstack 指令。但 gstack 屬於 Claude Code 生態系，GAL 使用 Copilot 為主控制面，兩者執行環境不相容。

目前最大的架構問題不是 command 數量不足，而是 `/gal status`、`/gal next`、`/gal pause` 這類 inherited commands 既沒有清楚名稱，也沒有把 gstack-style workflow 狀態投影成使用者可理解的 control-plane 視圖，因此整體看起來像兩個 repo 硬接在一起。

另一個關鍵問題是舊 GAL 以 `.dev/project.md` 的 `## Active Skills` 清單配合 `gal sync` 產生 adapters，這屬於靜態 allowlist / build-plumbing 思路，與目前已確認要採用的 gstack-style runtime routing 不一致。技能啟動應以 chat intent 與 specialist routing 為主，而不是由 repo 內手動列舉 skill 白名單決定。

解法方向：

- 吸收 gstack 各操作的**語意**，以 GAL 的 skill 格式重新實作，並在 GAL 內提供對等 command surface，而不是要求使用者切回 Claude Code 執行環境。
- 重做 GAL control-plane command surface，讓每個 `/gal` command 都能直接回答一個使用者問題，而不是沿用舊 workflow 名詞。
- 讓 `/gal` commands 成為對 specialist layer 的狀態投影與 orchestration interface，而不是另一套平行 workflow。
- 移除靜態 `Active Skills` -> `gal sync` 這條 user-facing 流程，把 skill 啟動責任交給 runtime routing；若仍需 adapter/context build，應降級為內部實作細節，而不是 public command surface。

目前已確認的 rename direction：

- `/gal next` -> `/gal whats-next`
- `/gal pause` -> `/gal wrap-up`
- `/gal plan` -> 移除，planning-stage specialist flow 交給 `/office-hours`、`/plan-*`、`/autoplan`
- `/gal sync` -> 移除 public command surface；其現有職責若仍存在，應轉為內部 plumbing，而不是使用者命令
- `/gal init`、`/gal research` 保留現名
- `/gal status` 保留現名，但必須重寫成完整 GAL state projection

以最新 gstack README 為準，目前需納入的操作如下：

- Sprint commands: `/office-hours`、`/plan-ceo-review`、`/plan-eng-review`、`/plan-design-review`、`/design-consultation`、`/design-shotgun`、`/design-html`、`/review`、`/investigate`、`/design-review`、`/qa`、`/qa-only`、`/cso`、`/ship`、`/land-and-deploy`、`/canary`、`/benchmark`、`/document-release`、`/retro`、`/browse`、`/setup-browser-cookies`、`/autoplan`、`/learn`
- Power tools and utility commands: `/careful`、`/freeze`、`/guard`、`/unfreeze`、`/connect-chrome`、`/setup-deploy`、`/gstack-upgrade`
- Out of scope for this plan: `/codex`。原因是它依賴外部 OpenAI Codex CLI 與獨立 second-opinion 流程，應在後續另開 multi-AI integration plan。

本計畫為 `gal-cross-ai-orchestrator.prompt.md` P3（Operation Layer And Role Model）的前置作業。

## Requirements

- [ ] 所有現有 GAL control-plane commands 都必須重新檢討命名與語意，不預設沿用 GSD 遺留名稱
- [ ] 每個 `/gal` command 都必須對應一個使用者可直接理解的 intent，例如「初始化這個 repo」、「現在我在哪裡」、「下一步做什麼」、「收尾並交接這段工作」、「進入研究流」
- [ ] `/gal` commands 必須讀寫正式定義的 canonical artifacts，而不是只顯示舊 `.dev/state.md` 欄位後就算完成整合
- [ ] `/gal status` 必須整合 `.dev/state.md`、active plan、`## Review Results`、`## Test Results`、blockers、continuity、specialist readiness，輸出完整 GAL-recorded state
- [ ] `/gal whats-next` 必須成為正式 command surface，取代 `/gal next`
- [ ] `/gal wrap-up` 必須成為正式 command surface，取代 `/gal pause`，明確負責 session close-out、continuity 更新與 handoff artifact 收斂
- [ ] `/gal plan` 必須自 public command surface 移除
- [ ] `/gal sync` 必須自 public command surface 移除
- [ ] 靜態 `Active Skills` allowlist 不得再作為主要 skill activation 機制；skill 啟動必須以 chat intent 與 runtime routing 為主
- [ ] 以最新 gstack README 為準，除 `/codex` 外的全部 command 都必須有對應的 GAL contract（input context、操作行為、output artifacts）
- [ ] 每個操作必須以 `commands/` 下的 SKILL.md 形式實作，可在 Copilot 與 Gemini CLI 中執行
- [ ] Skills 必須整合 GAL 的 `.dev/` state model：從 state 讀取 context，將結果回寫為 canonical artifacts
- [ ] 使用者不需要另外切換到 Claude Code 或額外安裝 upstream gstack；GAL 自身的安裝或 vendored layout 必須提供對應能力
- [ ] 使用者可直接沿用 gstack 指令名；除明確衝突外，命名應與上游保持一致
- [ ] `docs/gstack-integration.md` 必須更新，說明「GAL 內建 gstack-style command surface + GAL control-plane」的分工與好處
- [ ] `README.md` 必須改成類似 gstack README 的 command catalog 形式，完整表列全部可用指令，並清楚解釋整合後的使用方式與價值
- [ ] `README.md` 必須明確解釋最終核定的 `/gal` control-plane commands 的用途、輸入、輸出、何時使用、讀什麼、寫什麼，以及它們如何對應到 gstack-style specialist workflows
- [ ] GAL 必須保留 GSD 型 control-plane 職責：repo bootstrap、state inspection、session continuity、wrap-up/handoff、research workflow

## Phases

### P0: GAL Control-Plane Redesign

- **Scope**
  - 盤點目前所有 GAL commands：`/gal init`、`/gal plan`、`/gal status`、`/gal next`、`/gal pause`、`/gal sync`、`/gal research`，逐一說明它們目前為何不清楚、與整合目標衝突在哪裡
  - 定義新的 intent-led control-plane surface，不再把 inherited GSD names 當成當然正確
  - 為每個新 command 定義：使用者問題、讀取來源、輸出格式、寫回 artifacts、與 specialist commands 的邊界
  - 固定已拍板的 command decisions：保留 `/gal init`、`/gal research`；`/gal whats-next` 取代 `/gal next`；`/gal wrap-up` 取代 `/gal pause`；移除 `/gal plan`；移除 `/gal sync`
  - 重寫 `/gal status`，使其成為完整 GAL state projection，而不是舊 `.dev/state.md` viewer
  - 退役 `Active Skills` + `gal sync` 的靜態 allowlist 流程，改為 gstack-style runtime routing；若仍需 adapter/context build，降級為內部實作細節
  - 定義 legacy alias / migration policy，說明舊名稱要保留多久、是否只作為 discoverability alias

- **Files**: `docs/gal-control-plane-contracts.md`（新增）
- **Verify**: 使用者不需要知道 GSD 歷史也能理解每個 `/gal` command 在做什麼，且新的 control-plane surface 可被明確證明不是和 gstack specialist layer 平行重複；`/gal status` 能完整投影 GAL state，`/gal wrap-up` 能清楚收斂 session continuity。

### P1: Contract Analysis

- **Scope**
  - 以最新 gstack README command list 為 source of truth，分析除 `/codex` 外全部指令語意：input context、操作行為、output artifacts
  - 定義 GAL 的對應 skill interface contract
  - 對照 `gstack-integration.md` 與新的 control-plane contracts，識別每個操作的 gap、dependency 與應屬於 GAL control-plane 還是 specialist layer
  - 產出 `/gal` control-plane commands 與 gstack-style specialist commands 的語意分工矩陣，作為後續 README 說明基礎
  - 明確區分 runtime intent routing 與 internal capability/config plumbing，避免把靜態 allowlist 再引回 public UX

- **Files**: `docs/gstack-command-contracts.md`（新增）
- **Verify**: 每個目標 gstack 操作都有對應的 GAL contract 記錄，且已標示是 control-plane 整合還是 specialist operation；新的 `/gal` commands 如何投影 specialist state 也已明確定義。

### P2: Product, Plan, And Review Skills

- **Scope**
  - 實作 `/office-hours`、`/cso`、`/plan-ceo-review`、`/plan-eng-review`、`/plan-design-review`、`/autoplan`
  - 每個操作從 `.dev/` 或 plan file 讀取 context，產出結構化設計文檔、審查結果或 pipeline output
  - 結果回寫到 plan file 與 `.dev/state.md`，保留 review chaining 與 continuity

- **Files**: `commands/office-hours/SKILL.md`、`commands/cso/SKILL.md`、`commands/plan-ceo-review/SKILL.md`、`commands/plan-eng-review/SKILL.md`、`commands/plan-design-review/SKILL.md`、`commands/autoplan/SKILL.md`（全部新增）
- **Verify**: 在 Copilot 執行上述操作，可形成與 gstack 相同的 plan-stage pipeline，且 review/status 會回寫 canonical artifacts。

### P3: Design, Debug, Browser, And QA Skills

- **Scope**
  - 實作 `/design-consultation`、`/design-shotgun`、`/design-html`、`/design-review`
  - 實作 `/investigate`、`/review`、`/browse`、`/connect-chrome`、`/qa`、`/qa-only`、`/setup-browser-cookies`
  - 保持 debug / review / QA workflow 的 artifact handoff 與 canonical 回寫

- **Files**: `commands/design-consultation/SKILL.md`、`commands/design-shotgun/SKILL.md`、`commands/design-html/SKILL.md`、`commands/design-review/SKILL.md`、`commands/investigate/SKILL.md`、`commands/review/SKILL.md`、`commands/browse/SKILL.md`、`commands/connect-chrome/SKILL.md`、`commands/qa/SKILL.md`、`commands/qa-only/SKILL.md`、`commands/setup-browser-cookies/SKILL.md`（全部新增）
- **Verify**: 在 Copilot 中可執行 design/debug/browser/QA 類操作，且每步都能承接 plan context 與回寫結果。

### P4: Ship, Release, Memory, And Guardrails

- **Scope**
  - 實作 `/ship`、`/land-and-deploy`、`/canary`、`/benchmark`、`/setup-deploy`、`/document-release`、`/retro`、`/learn`
  - 實作 `/careful`、`/freeze`、`/guard`、`/unfreeze`、`/gstack-upgrade`
  - 確保這些 command 不繞過 GAL control-plane，而是使用 GAL 提供的 state、review logs、plan completion、session continuity

- **Files**: `commands/ship/SKILL.md`、`commands/land-and-deploy/SKILL.md`、`commands/canary/SKILL.md`、`commands/benchmark/SKILL.md`、`commands/setup-deploy/SKILL.md`、`commands/document-release/SKILL.md`、`commands/retro/SKILL.md`、`commands/learn/SKILL.md`、`commands/careful/SKILL.md`、`commands/freeze/SKILL.md`、`commands/guard/SKILL.md`、`commands/unfreeze/SKILL.md`、`commands/gstack-upgrade/SKILL.md`（全部新增）
- **Verify**: release / memory / safety 類操作可在 Copilot 中呼叫，且與 GAL state、review logs、research artifacts 一致接線。

### P5: README And Integration Explanation

- **Scope**
  - 依最新 gstack README 的呈現方式，重寫 `README.md` 的 command catalog，完整表列 GAL 中可用的全部 gstack-style commands 與 GAL control-plane commands
  - 明確說明整合模型：為什麼保留 `/gal` control-plane，同時又要吸收 gstack specialist workflows；這個分層比單純保留任一邊更好的理由是什麼
  - 為最終核定的 `/gal` control-plane commands 製作「何時用、讀什麼、寫什麼、與 specialist commands 差在哪」的操作說明
  - 特別說清楚 `/gal whats-next` 是怎麼從 current state、plan status、review results、QA readiness 等 artifact 算出下一步；以及 `/gal wrap-up` 到底在收斂什麼 artifact、為什麼需要它
  - 明確說明 skill 啟動以 chat intent / runtime routing 為主，而不是靠 `.dev/project.md` 的 `Active Skills` 白名單
  - 補齊 README 中從 repo bootstrap 到 specialist workflow 的實際起手路徑，避免使用者只看到 command 名稱卻看不出整合好處

- **Files**: `README.md`、`docs/gstack-integration.md`
- **Verify**: README 單獨閱讀即可讓使用者理解所有 command 的用途、整合價值，以及 `/gal` 與 gstack-style commands 的分工。

## Files to Create or Modify

- `docs/gal-control-plane-contracts.md` — 新增，定義新的 `/gal` command surface、rename 決策、讀寫 contract 與 migration 規則
- `docs/gstack-command-contracts.md` — 新增，定義每個操作的 input/output contract
- `docs/gstack-integration.md` — 修改：說明 GAL control-plane 與 gstack-style specialist operations 的分工、邊界與好處
- `README.md` — 修改：改為 gstack-style command catalog，完整表列 commands，並說明 `/gal` commands 的用途與整合路徑
- `docs/per-repo-context.md` — 修改：移除 `Active Skills` + `gal sync` 作為主要 user-facing 流程的描述
- `templates/project.md` — 修改：移除或降級 `## Active Skills`，避免把靜態 skill allowlist 當成主要 routing 設計
- `commands/plan-ceo-review/SKILL.md` — 新增
- `commands/plan-eng-review/SKILL.md` — 新增
- `commands/plan-design-review/SKILL.md` — 新增
- `commands/` — 新增目前 gstack 除 `/codex` 外全部對應 commands

## Test Cases

- [ ] 使用者只看新 `/gal` command names，就能大致理解它們做什麼，不需要知道 GSD 歷史
- [ ] 在 Copilot 中呼叫 `/gal status`，可完整呈現 GAL 已記錄的 repo 狀態、active plan 狀態、review/test 狀態、blockers、continuity 與 specialist readiness
- [ ] 在 Copilot 中呼叫 `/gal whats-next`，可依目前 plan status、review 結果、blockers 與 readiness 算出下一個 specialist command 或 control-plane action
- [ ] 在 Copilot 中呼叫 `/gal wrap-up`，可清楚說明自己在收斂什麼 artifact、為什麼要做，以及做完後如何恢復工作
- [ ] 在 Copilot 中呼叫 `/plan-eng-review`，可對當前 plan file 產出結構化工程審查
- [ ] 在 Copilot 中呼叫 `/review`，可對指定 code changes 產出結構化審查
- [ ] 在 Copilot 中可呼叫目前 gstack README 列出的全部 commands（`/codex` 除外），且不要求使用者額外切換到 Claude Code
- [ ] 審查結果可回寫到對應 plan file 的 `## Review Results` section
- [ ] GAL 自身安裝完成後，不需要再額外安裝 upstream gstack 即可執行上述操作
- [ ] Gemini CLI 可執行同一套 skill（via `@file` import）
- [ ] specialist skill 啟動可依 chat intent / runtime routing 發生，不需要依賴 `.dev/project.md` 的靜態 skill allowlist
- [ ] 舊 GAL command names 如需保留，僅作為 compatibility alias，且 README 不再把它們當主命令表述
- [ ] `README.md` 需有完整 command table，且能直接回答最終核定的 `/gal` commands 各自是做什麼

## Success Criteria

- [ ] 最終 `/gal` control-plane surface 不再讓使用者感受到 inherited GSD 命名殘留
- [ ] `/gal` 不再只是舊 state viewer，而是對 gstack-style specialist workflow 的正式 orchestration interface
- [ ] `/gal status` 成為 GAL 的完整狀態投影，而不是舊 `.dev/state.md` 欄位檢視器
- [ ] `/gal wrap-up` 清楚承接 continuity / handoff 職責，而不是抽象的「pause」
- [ ] GAL 可在 Copilot 環境中提供最新 gstack README 所列、除 `/codex` 外的完整 command surface
- [ ] 操作語意與 gstack 原始意圖一致
- [ ] 操作結果回寫到 GAL canonical artifacts
- [ ] 使用者不需要另外進入 Claude Code 環境才能使用這些 commands
- [ ] README 足以讓使用者理解整合好處，以及 `/gal` control-plane commands 的定位
- [ ] skill activation 主要由 runtime routing 驅動，而不是由靜態 `Active Skills` 白名單決定
- [ ] GSD 的 stateful workflow、task memory、continuity、orchestration loop 仍由 GAL control-plane 持有

## Risks and Open Questions

- 若只替換 command 名稱而不重做讀寫 contract，會變成重新命名版的舊 GAL，而不是整合架構
- `/gal status` 是否保留現名仍屬開放問題；本計畫不得假設任何既有 GAL 名稱必然保留
- `Active Skills` 若完全移除，需定義是否仍需要更高層的 capability gating 來保護有外部依賴的功能（如 browser / deploy / obsidian）
- gstack 各操作的確切語意需從文件與使用經驗推導，P1 分析品質直接影響後續實作
- Copilot 與 Claude Code 在 reasoning 模式上的差異，可能使某些操作的輸出品質不同
- `/ship` 若涉及 git push / PR 操作，需確認 Copilot 工具支援邊界
- `/browse`、`/connect-chrome`、`/setup-browser-cookies` 依賴額外瀏覽器能力與 side-panel/daemon 類資產，需確認在 GAL 中的承載方式
- `/gstack-upgrade` 在 GAL 內不應維持原本的 upstream 自更新語意，需重新定義成 GAL-compatible upgrade/internal refresh flow
- `/codex` 被刻意排除，避免把 OpenAI CLI second-opinion runtime 與本計畫的 Copilot/Gemini command parity 綁在一起

## Approval

- Architect verdict: pending
- Human approval: pending

---

## Status

Workflow: DRAFT
Step: 5 of 6
Last activity: P4 complete
Next step: P5 — 重寫 README.md（gstack-style command catalog）與 docs/gstack-integration.md（整合模型說明）

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |
| P0 | Create `docs/gal-control-plane-contracts.md` only | Also updated `commands/gal/SKILL.md`, `commands/gal/SKILL.template.md`, `commands/gal-status/SKILL.{md,template.md}`, `commands/commands.md`, `docs/command-dispatch-architecture.md`; created `commands/gal-whats-next/`, `commands/gal-wrap-up/`; demoted `gal-next`, `gal-pause`, `gal-plan` | The contract document alone would have been orphaned — the actual skill files needed to change for the contracts to be real |

### Handoff Notes

本計畫起因：gstack 使用 Claude Code 環境，與 GAL 主控制面 Copilot 不相容。`gstack-integration.md` 的「直接使用 gstack 指令」若要在 Copilot 中成立，必須改寫成 GAL-native skill files，而不是要求使用者切回 Claude Code。整合原則不是用 gstack 取代 GSD，而是由 GAL 持有 GSD 型 control-plane（state、continuity、handoff、sync、research），由 gstack-native skills 持有 specialist operations。

2026-03-30 更新決策：以最新 gstack README command list 為準，`/codex` 明確排除在本計畫外，其餘 commands 全數納入。另將 README 重寫納入本計畫正式交付物，要求用 gstack 式 command catalog 方式清楚說明整合好處，以及 `/gal init` 之外各 control-plane commands 的用途與分工。

2026-03-30 補充決策：目前 `/gal` surface 仍帶有 inherited GSD 命名殘留，使用者無法從 `/gal next`、`/gal pause` 等名稱直接理解整合作用，因此本計畫新增 P0 先重做 control-plane command surface。已確定 `/gal next` 改為 `/gal whats-next`；`/gal pause` 改為 `/gal wrap-up`；`/gal plan` 與 `/gal sync` 自 public command surface 移除；`/gal init` 與 `/gal research` 保留現名；`/gal status` 保留現名但必須重寫為完整 state projection。此外，舊 GAL 的 `Active Skills` + `gal sync` 靜態 allowlist 設計不再視為主要 routing 架構，後續改採 gstack-style runtime routing。

## Test Results

[由 tester golem 在 TEST 階段填寫]

## Review Results

[由 reviewer golem 在 REVIEW 階段填寫]
