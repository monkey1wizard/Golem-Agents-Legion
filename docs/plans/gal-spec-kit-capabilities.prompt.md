# Plan: GAL 借鑑 spec-kit 的最小能力增量

## Goal

在不改變 GAL 既有 control-plane / specialist 分層的前提下，吸收 spec-kit 最有價值、且與現有命令鏈最相容的三項能力：clarify、analyze、tasks，使 plan artifact 不只承載目標與審查結果，也能承載未解問題、實作任務切分與跨階段一致性檢查。

最終效果應是：

1. 規劃階段能明確暴露尚未解決的不確定性，而不是把模糊點藏在自由文字中。
2. 工程審查完成後，plan 內能生成可執行的 tasks，供 implementation、review、qa、ship 共同消費。
3. review / ship / control-plane 能檢查需求、tasks、實作與測試之間是否出現 drift。

本計畫刻意不引入 spec-kit 的 preset / extension framework，也不改寫 GAL 的整體 command surface。範圍只限於在現有 plan contract 上新增最小但高價值的能力。

## Context

目前 GAL 已有成熟的 control-plane 與 specialist chain：`/office-hours` 建 plan，`/plan-ceo-review`、`/plan-design-review`、`/plan-eng-review` 產生規劃與審查結果，`/review`、`/qa`、`/ship` 在後續階段消費這些結果。

現況缺口不在 command 數量，而在 plan artifact 缺三個中間層能力：

- 缺少正式的 clarify surface：不確定性雖存在於 `office-hours` forcing questions、CEO review 的 ambiguities、Eng review 的 architecture decisions 中，但沒有統一落點。
- 缺少正式的 tasks surface：`plan-eng-review` 已能形成 test matrix 與 buildable plan，但沒有將 implementation work breakdown 寫成 canonical tasks list。
- 缺少正式的 drift analysis surface：`/review`、`/ship`、`/gal status` 能看到 review/test 結果，但尚不能系統性回答「requirements、tasks、implementation、tests 是否仍一致」。

spec-kit 的價值不在於取代 GAL，而在於把這三個薄弱層補齊。從 GAL 現有架構來看，最適合的導入方式不是新增獨立 `/clarify`、`/analyze`、`/tasks` 命令，而是把能力嵌入既有命令鏈：

- `clarify` 主要在 planning chain 內產生與收斂
- `tasks` 主要在 Eng review clear 後產生，供後續階段消費
- `analyze` 主要在 review / ship / control-plane 用於偵測 drift

這能保留 GAL 既有的命令心智模型，同時借到 spec-kit 最務實的骨架。

## Requirements

- [ ] plan template 必須新增 `## Open Questions` section，作為 clarify 的 canonical 落點
- [ ] plan template 必須新增 `## Tasks` section，作為 tasks 的 canonical 落點
- [ ] plan template 必須新增 `## Analyze` section，作為 drift / consistency 檢查的 canonical 落點
- [ ] `/office-hours` 必須能把尚未解決的假設與模糊點寫入 `## Open Questions`
- [ ] `/plan-ceo-review` 必須能把 scope ambiguities 與未決策項寫入 `## Open Questions`
- [ ] `/plan-eng-review` 必須能關閉已解決的 open questions，並在 review clear 後生成 `## Tasks`
- [ ] `/plan-design-review` 必須能檢查是否仍有設計相關的 open questions 未被處理
- [ ] `/review` 必須能檢查 implementation 與 `## Tasks` / `## Requirements` 是否 drift，並將結果寫入 `## Analyze`
- [ ] `/ship` 必須能把 `## Open Questions`、`## Tasks`、`## Analyze` 納入 readiness gate
- [ ] `/gal status` 與 `/gal whats-next` 必須能讀取這三個 section，投影問題數量、任務完成度與 drift 狀態
- [ ] 功能完成後，`README.md` 與 `README.zh-Hant.md` 必須同步更新，說明新增的 plan sections 與使用者可見的 workflow 變化
- [ ] 既有 `## Review Results`、`## Test Plan`、`## Test Results` contract 不得被破壞
- [ ] 本計畫不得引入新的 public command，也不得要求使用者學一組新的 slash commands

## Phases

### P0: Plan Contract Extension

- **Scope**
  - 更新 `templates/plan.md`，新增 `## Open Questions`、`## Tasks`、`## Analyze`
  - 在 `docs/gstack-command-contracts.md` 中補上這三個 section 的 ownership 與 produce / re-check / consume 關係
  - 在 `docs/gal-control-plane-contracts.md` 中補上 `/gal status`、`/gal whats-next` 對這三個 section 的讀取責任

- **Files**: `templates/plan.md`, `docs/gstack-command-contracts.md`, `docs/gal-control-plane-contracts.md`
- **Verify**: plan artifact 本身已能作為 clarify / tasks / analyze 的 canonical 容器，且控制平面知道如何讀取它們

### P1: Clarify Integration

- **Scope**
  - 更新 `/office-hours`：將 forcing questions 後仍未收斂的問題寫入 `## Open Questions`
  - 更新 `/plan-ceo-review`：將 ambiguities、scope decisions 未決項寫入 `## Open Questions`
  - 更新 `/plan-design-review`：檢查設計相關的 open questions 是否仍待決
  - 明確定義 resolved / open 的狀態轉換規則

- **Files**: `commands/office-hours/SKILL.md`, `commands/plan-ceo-review/SKILL.md`, `commands/plan-design-review/SKILL.md`
- **Verify**: 規劃階段結束後，使用者能從單一 section 看到所有尚未決定的問題，而不是散落在 review prose 中

### P2: Tasks Generation

- **Scope**
  - 更新 `/plan-eng-review`：當 Eng Review clear 時，從 requirements / approach / review decisions 生成 `## Tasks`
  - 定義 tasks 的粒度：必須是可驗證、可勾選、可被 review / qa 消費的 atomic work items
  - 明確說明 tasks 不是重複 `## Approach`，而是 implementation-facing breakdown

- **Files**: `commands/plan-eng-review/SKILL.md`, `docs/gstack-command-contracts.md`
- **Verify**: 同一份 plan 在工程審查結束後，已包含可直接進入實作的工作切分，而不是只有高階敘述

### P3: Drift Analysis And Readiness

- **Scope**
  - 更新 `/review`：比對 requirements、tasks 與實際 diff，將 drift 結果寫入 `## Analyze`
  - 更新 `/ship`：若仍有 open questions、未完成 tasks 或 analyze 顯示 drift，需在 readiness gate 中明確揭示
  - 更新 `/gal status`、`/gal whats-next`：投影 open question 數量、task completion、analyze verdict

- **Files**: `commands/review/SKILL.md`, `commands/ship/SKILL.md`, `commands/gal-status/SKILL.md`, `commands/gal-whats-next/SKILL.md`
- **Verify**: 使用者可從 status / next 直接知道目前卡在未決問題、未完成 tasks，還是 implementation drift

### P4: README Alignment

- **Scope**
  - 更新 `README.md`，說明 plan 新增的 `## Open Questions`、`## Tasks`、`## Analyze` sections 與它們在 workflow 中的角色
  - 更新 `README.zh-Hant.md`，對齊相同的 artifact 與 workflow 說明
  - 確保 README 中對 `/office-hours`、`/plan-eng-review`、`/review`、`/ship`、`/gal status`、`/gal whats-next` 的描述，反映這三個新 section 的產生與消費關係

- **Files**: `README.md`, `README.zh-Hant.md`
- **Verify**: 使用者只看 README 即可理解這次新增能力如何接入既有 workflow，而不必額外閱讀 contract 文件

## Files to Create or Modify

- `templates/plan.md` — 擴充 plan canonical sections，加入 clarify / tasks / analyze 容器
- `docs/gstack-command-contracts.md` — 補上各 command 對新 sections 的 produce / re-check / consume 契約
- `docs/gal-control-plane-contracts.md` — 補上 control-plane 對新 sections 的讀取與決策責任
- `commands/office-hours/SKILL.md` — 將未收斂問題寫入 `## Open Questions`
- `commands/plan-ceo-review/SKILL.md` — 將 scope ambiguities 與未決項寫入 `## Open Questions`
- `commands/plan-design-review/SKILL.md` — 檢查設計面未決問題
- `commands/plan-eng-review/SKILL.md` — 生成 `## Tasks`，並關閉已解決的 open questions
- `commands/review/SKILL.md` — 生成 `## Analyze` 的 drift 檢查結果
- `commands/ship/SKILL.md` — 將三個新 sections 納入 readiness gate
- `commands/gal-status/SKILL.md` — 投影 clarify / tasks / analyze 狀態
- `commands/gal-whats-next/SKILL.md` — 依三個新 sections 推薦單一步驟
- `README.md` — 更新使用者文件，說明新 sections 與 workflow 影響
- `README.zh-Hant.md` — 更新繁中使用者文件，對齊相同說明

## Test Cases

- [ ] 建立新 plan 後，`## Open Questions`、`## Tasks`、`## Analyze` 三個 section 皆存在且為空 scaffold
- [ ] 執行 `/office-hours` 後，未解答的假設會被寫入 `## Open Questions`
- [ ] 執行 `/plan-ceo-review` 後，scope ambiguities 會被追加到 `## Open Questions`
- [ ] 執行 `/plan-eng-review` 並通過後，plan 會生成可勾選的 `## Tasks`
- [ ] 執行 `/review` 後，若實作偏離 tasks 或 requirements，`## Analyze` 會出現 drift 記錄
- [ ] 執行 `/gal status` 時，可看到 open questions 數量、tasks 完成度與 analyze verdict
- [ ] 執行 `/gal whats-next` 時，若仍有 open questions，建議回 planning；若 tasks 未完成，建議回 implementation；若 analyze 有 drift，建議先跑 review/fix
- [ ] 執行 `/ship` 時，若三個新 sections 顯示未決問題或 drift，readiness gate 會明確揭示
- [ ] `README.md` 與 `README.zh-Hant.md` 都已說明 `## Open Questions`、`## Tasks`、`## Analyze` 的用途與命令責任分工

## Success Criteria

- [ ] GAL 在不新增 public commands 的前提下，吸收 spec-kit 最有價值的 clarify / tasks / analyze 能力
- [ ] plan artifact 成為從規劃、實作、審查到發佈的一致性主軸，而不只是一份 kickoff 文檔
- [ ] 使用者能從單一 plan 看出還有哪些問題未決、實作該做哪些事、以及是否已發生 drift
- [ ] README 與 README.zh-Hant 的敘述與新 artifact contract 保持一致，避免使用者文件落後於實作
- [ ] control-plane 與 specialist layer 的既有分工保持成立，沒有因導入 spec-kit 概念而長出第二套 workflow

## Risks and Open Questions

- 若 `## Tasks` 粒度過粗，會重複 `## Approach`；若過細，則維護成本過高
- 若 `## Analyze` 寫得太抽象，最後會退化成另一段 review prose，而不是可消費的 drift signal
- 若 `clarify` 沒有清楚定義 resolved 規則，`## Open Questions` 容易累積過期問題
- `/ship` 是否只揭示 readiness 缺口，還是要將某些缺口升級為 hard block，仍需進一步決策

## Approval

- Architect verdict: pending
- Human approval: pending

---

## Status

Workflow: DRAFT
Step: 0 of 4
Last activity: 2026-03-31 — plan created from spec-kit gap analysis
Next step: confirm whether clarify / tasks / analyze should all land in one batch or be phased with plan contract first

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |

### Handoff Notes

這份計畫刻意採最小變更策略：不新增 `/clarify`、`/analyze`、`/tasks` 三個獨立命令，而是把這些能力嵌入既有 planning → review → ship 鏈條。這樣既能借到 spec-kit 的結構化優勢，也不會破壞 GAL 已形成的命令心智模型。

## Test Results

[由 tester golem 在 TEST 階段填寫]

## Review Results

[由 reviewer golem 在 REVIEW 階段填寫]