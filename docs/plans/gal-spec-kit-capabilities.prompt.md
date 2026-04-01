# Plan: GAL 借鑑 spec-kit 的最小能力增量

## Goal

在不改變 GAL 既有 control-plane / specialist 分層的前提下，吸收 spec-kit 最有價值、且與現有命令鏈最相容的三項能力：clarify、analyze、tasks，使 plan artifact 不只承載目標與審查結果，也能承載未解問題、實作任務切分與跨階段一致性檢查。

最終效果應是：

1. 規劃階段能明確暴露尚未解決的不確定性，而不是把模糊點藏在自由文字中。
2. 工程審查完成後，plan 內能生成可執行的 tasks，供 implementation、review、qa、ship 共同消費。
3. review / ship / control-plane 能檢查需求、tasks、實作與測試之間是否出現 drift。

本計畫刻意不引入 spec-kit 的 preset / extension framework，也不改寫 GAL 的整體 command surface。範圍只限於在現有 plan contract 上新增最小但高價值的能力。

## Context

目前 GAL 已有成熟的 control-plane 與 specialist chain：`/office-hours` 建 source plan doc，`/plan-ceo-review`、`/plan-design-review`、`/plan-eng-review`、`/review`、`/qa`、`/ship` 都將結果寫回 active plan file，供 `/gal status`、`/gal whats-next` 與後續 specialist 消費。

另外，repo 的 artifact contract 已比這份計畫初稿更前進，而且已不只體現在 `README.md` 與 `README.zh-Hant.md`。目前 `docs/per-repo-context.md`、`docs/ai-agent-onboarding.md`、`docs/gal-control-plane-contracts.md`、`docs/gstack-command-contracts.md` 已經明確收斂為兩層 plan artifact：

- `docs/plans/<plan-slug>.md` 是 human-readable source plan doc
- `docs/plans/<plan-slug>.prompt.md` 是 AI execution work file，也是 control-plane 讀取的 canonical state vector

這代表本計畫不應再把 `.prompt.md` 視為一般性的「plan prompt 文件」設計空間，而必須保留目前 repo 已成立的 source-plan / execution-work-file 分工。這份檔案本身因為只有 `.prompt.md`、沒有對應 `.md` source plan doc，屬於歷史遺留例外；在本計畫內應把它視為被刷新中的 execution work file，而不是未來 `/office-hours` 應複製的 artifact 形狀。

現況缺口不在 command 數量，而在 plan artifact 缺三個中間層能力：

- 缺少正式的 clarify surface：不確定性雖存在於 `office-hours` forcing questions、CEO review 的 ambiguities、Eng review 的 architecture decisions 中，但目前仍只散落在自由文字與 review prose 中，沒有統一落點。
- 缺少正式的 tasks surface：`plan-eng-review` 已能形成 test matrix 與 buildable plan，但沒有將 implementation work breakdown 寫成 canonical tasks list。
- 缺少正式的 drift analysis surface：`/review`、`/ship`、`/gal status` 能看到 review/test 結果，但尚不能系統性回答「requirements、tasks、implementation、tests 是否仍一致」。

目前 repo 內真正需要補的是「已收斂 artifact contract 與仍落後的 plan shape / specialist behavior 之間的缺口」：

- `templates/plan.md` 仍是舊版 scaffold，只有 `## Risks and Open Questions`，沒有 `## Open Questions`、`## Tasks`、`## Analyze`
- `commands/office-hours/SKILL.md` 仍以舊版 source plan 結構建 plan，且尚未在 planning 階段初始化 paired active plan file，導致 clarify 沒有 canonical write target
- `workflows/coding.md` 與 `agent/golem-planner.agent.md` 不只描述舊的 plan shape 與舊欄位名稱，還仍把 planner 直接產出 `.prompt.md` 當作主要 plan artifact，與目前 `.md` / `.prompt.md` 分工不一致
- `commands/gal-status/SKILL.md` 目前只讀 `## Status`、`## Review Results`、`## Test Results`、`### Handoff Notes`
- `commands/plan-eng-review/SKILL.md` 目前只寫 `## Review Results` 與 `## Test Plan`
- `commands/review/SKILL.md`、`commands/ship/SKILL.md`、`commands/gal-whats-next/SKILL.md` 仍不知道 `## Open Questions`、`## Tasks`、`## Analyze` 這三個新 section 的存在

spec-kit 的價值不在於取代 GAL，而在於把這三個薄弱層補齊。從 GAL 現有架構來看，最適合的導入方式不是新增獨立 `/clarify`、`/analyze`、`/tasks` 命令，而是把能力嵌入既有命令鏈：

- `clarify` 主要在 planning chain 內產生與收斂
- `tasks` 主要在 Eng review clear 後產生，供後續階段消費
- `analyze` 主要在 review / ship / control-plane 用於偵測 drift

這能保留 GAL 既有的命令心智模型，同時借到 spec-kit 最務實的骨架。

## Requirements

- [x] plan template 必須新增 `## Open Questions` section，作為 clarify 的 canonical 落點
- [x] plan template 必須新增 `## Tasks` section，作為 tasks 的 canonical 落點
- [x] plan template 必須新增 `## Analyze` section，作為 drift / consistency 檢查的 canonical 落點
- [x] 本計畫必須保留目前已成立的 `docs/plans/<plan-slug>.md` source plan doc / `docs/plans/<plan-slug>.prompt.md` AI execution work file 分工，不得把 `.prompt.md` 重新定義回一般性的 source plan doc
- [x] `/office-hours` 必須在 planning 階段建立或初始化 paired active plan file `docs/plans/<plan-slug>.prompt.md`，讓 clarify 從 planning 開始就有 canonical write target
- [x] `/office-hours` 必須能把尚未解決的假設與模糊點寫入 `## Open Questions`
- [x] `/plan-ceo-review` 必須能把 scope ambiguities 與未決策項寫入 `## Open Questions`
- [x] `/plan-eng-review` 必須能關閉已解決的 open questions，並在 review clear 後生成 `## Tasks`
- [x] `/plan-design-review` 必須能檢查是否仍有設計相關的 open questions 未被處理
- [x] `/review` 必須能檢查 implementation 與 `## Tasks` / `## Requirements` 是否 drift，並將結果寫入 `## Analyze`
- [x] `/ship` 必須能把 `## Open Questions`、`## Tasks`、`## Analyze` 納入 readiness gate
- [x] `/gal status` 與 `/gal whats-next` 必須能讀取這三個 section，投影問題數量、任務完成度與 drift 狀態
- [x] `## Open Questions` 必須是唯一的 clarify canonical section；planning specialists 可以在各自 review prose 提及問題，但 authoritative list 只能維持在 top-level section
- [x] `## Tasks` 必須由 `/plan-eng-review` 作為唯一初始化者；implementation 階段只能更新完成狀態，不得重寫 task 語義
- [x] `## Analyze` 必須由 `/review` 作為唯一 verdict writer；`/ship`、`/gal status`、`/gal whats-next` 只能消費 verdict，不得各自重算 drift semantics
- [x] `/ship` 對 unresolved open questions、unfinished tasks、analyze drift 先以 readiness warnings 處理，不新增新的 hard gate；`ENG_REVIEW` 仍是唯一 required gate
- [x] `workflows/coding.md` 與 `agent/golem-planner.agent.md` 必須同時對齊新的 plan shape 與目前 `.md` / `.prompt.md` artifact split，避免 planner 文件與實際 contract 漂移
- [x] 功能完成後，`README.md` 與 `README.zh-Hant.md` 必須同步更新，說明新增的 plan sections 與使用者可見的 workflow 變化
- [x] 既有 `## Review Results`、`## Test Plan`、`## Test Results` contract 不得被破壞
- [x] 本計畫不得引入新的 public command，也不得要求使用者學一組新的 slash commands

## Phases

### P0: Plan Contract Extension

- **Scope**
  - 更新 `templates/plan.md`，新增 `## Open Questions`、`## Tasks`、`## Analyze`
  - 更新 `commands/office-hours/SKILL.md`，讓 planning 階段會建立或初始化 paired active plan file，並預先放入空的 `## Open Questions`、`## Tasks`、`## Analyze` scaffold
  - 更新 `workflows/coding.md`，把 workflow 對 plan scaffold、lifecycle，以及 `.md` / `.prompt.md` artifact split 的描述改為目前 repo contract
  - 更新 `agent/golem-planner.agent.md`，確保 planner agent 對 source plan doc / execution work file 的分工與 template 一致
  - 在 `docs/gstack-command-contracts.md` 中補上這三個 section 的 ownership 與 produce / re-check / consume 關係
  - 在 `docs/gal-control-plane-contracts.md` 中補上 `/gal status`、`/gal whats-next` 對這三個 section 的讀取責任

- **Files**: `templates/plan.md`, `commands/office-hours/SKILL.md`, `workflows/coding.md`, `agent/golem-planner.agent.md`, `docs/gstack-command-contracts.md`, `docs/gal-control-plane-contracts.md`
- **Verify**: active plan file `.prompt.md` 會在 planning 階段被初始化，並作為 clarify / tasks / analyze 的 canonical 容器；template、workflow、planner agent、artifact contract 與控制平面對 `.md` / `.prompt.md` 分工有一致理解

### P1: Clarify Integration

- **Scope**
  - 更新 `/office-hours`：將 forcing questions 後仍未收斂的問題寫入已存在的 `## Open Questions`
  - 更新 `/plan-ceo-review`：將 ambiguities、scope decisions 未決項寫入 `## Open Questions`
  - 更新 `/plan-design-review`：檢查設計相關的 open questions 是否仍待決
  - 明確定義 resolved / open 的狀態轉換規則與最小格式：stable ID、checklist state、source / resolution metadata

- **Files**: `commands/office-hours/SKILL.md`, `commands/plan-ceo-review/SKILL.md`, `commands/plan-design-review/SKILL.md`
- **Verify**: 規劃階段結束後，使用者能從單一 section 看到所有尚未決定的問題，而不是散落在 review prose 中

### P2: Tasks Generation

- **Scope**
  - 更新 `/plan-eng-review`：當 Eng Review clear 時，從 requirements / approach / review decisions 生成 `## Tasks`
  - 定義 tasks 的粒度：必須是可驗證、可勾選、可被 review / qa 消費的 atomic work items
  - 明確說明 tasks 不是重複 `## Approach`，而是 implementation-facing breakdown
  - 明確限制 `/plan-eng-review` 是 `## Tasks` 的唯一初始化者；implementation 只更新完成狀態

- **Files**: `commands/plan-eng-review/SKILL.md`, `docs/gstack-command-contracts.md`
- **Verify**: 同一份 plan 在工程審查結束後，已包含可直接進入實作的工作切分，而不是只有高階敘述

### P3: Drift Analysis And Readiness

- **Scope**
  - 更新 `/review`：比對 requirements、tasks 與實際 diff，將 drift 結果寫入 `## Analyze`
  - 更新 `/ship`：若仍有 open questions、未完成 tasks 或 analyze 顯示 drift，需在 readiness gate 中明確揭示，但先作為 warnings，不新增新的 hard gate
  - 更新 `/gal status`、`/gal whats-next`：投影 open question 數量、task completion、analyze verdict
  - 明確限制 `/review` 是 `## Analyze` 的唯一 verdict writer；control-plane 與 `/ship` 只消費 verdict，不重算 semantics

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
- `commands/office-hours/SKILL.md` — 在 planning 階段建立或初始化 paired execution work file，並預先放入三個新 section 的空 scaffold
- `workflows/coding.md` — 對齊新的 plan sections 與 `.md` / `.prompt.md` workflow lifecycle 說明
- `agent/golem-planner.agent.md` — 對齊 planner 產出 source plan doc 與 execution work file 的契約
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

- [x] 建立新 plan 後，`## Open Questions`、`## Tasks`、`## Analyze` 三個 section 皆存在且為空 scaffold
- [x] 執行 `/office-hours` 後，除了 source plan doc 外，也會建立 paired `.prompt.md`，其中已存在空的 `## Open Questions`、`## Tasks`、`## Analyze` scaffold
- [x] `workflows/coding.md` 與 `agent/golem-planner.agent.md` 對 plan 欄位與 `.md` / `.prompt.md` 分工的描述已改為新 scaffold，不再停留在舊版 `Risks and Open Questions` / `Approach` 心智模型與「planner 直接建立 `.prompt.md` 作為主要 plan artifact」的舊模型
- [x] 執行 `/office-hours` 後，未解答的假設會被寫入 `## Open Questions`
- [x] 執行 `/plan-ceo-review` 後，scope ambiguities 會被追加到 `## Open Questions`
- [x] 執行 `/plan-eng-review` 並通過後，plan 會生成可勾選的 `## Tasks`
- [x] 執行 `/review` 後，若實作偏離 tasks 或 requirements，`## Analyze` 會出現 drift 記錄
- [x] 執行 `/gal status` 時，可看到 open questions 數量、tasks 完成度與 analyze verdict
- [x] 執行 `/gal whats-next` 時，若仍有 open questions，建議回 planning；若 tasks 未完成，建議回 implementation；若 analyze 有 drift，建議先跑 review/fix
- [x] 執行 `/ship` 時，若三個新 sections 顯示未決問題或 drift，readiness gate 會明確揭示
- [x] 變更完成後，repo 內不會有任何文件把 `.prompt.md` 重新定義回 human-readable source plan doc
- [x] `README.md` 與 `README.zh-Hant.md` 都已說明 `## Open Questions`、`## Tasks`、`## Analyze` 的用途與命令責任分工

## Success Criteria

- [x] GAL 在不新增 public commands 的前提下，吸收 spec-kit 最有價値的 clarify / tasks / analyze 能力
- [x] plan artifact 成為從規劃、實作、審查到發佈的一致性主軸，而不只是一份 kickoff 文檔
- [x] planning chain、review chain 與 control-plane 會從 planning 開始就针對同一份 active `.prompt.md` 讀寫，不再出現 lifecycle 斷裂
- [x] 使用者能從單一 plan 看出還有哪些問題未決、實作該做哪些事、以及是否已發生 drift
- [x] template、workflow 文件、planner agent、specialist skills 與 control-plane 對 plan shape 以及 `.md` / `.prompt.md` artifact split 的理解重新收斂，不再同時存在舊版與新版 scaffold
- [x] README 與 README.zh-Hant 的敍述與新 artifact contract 保持一致，避免使用者文件落後於實作
- [x] control-plane 與 specialist layer 的既有分工保持成立，沒有因導入 spec-kit 概念而長出第二套 workflow

## Risks and Open Questions

- 目前 `README.md`、`README.zh-Hant.md`、`docs/per-repo-context.md`、`docs/ai-agent-onboarding.md`、`docs/gal-control-plane-contracts.md`、`docs/gstack-command-contracts.md` 都已對齊 `.md` / `.prompt.md` artifact split；若後續只更新部分內部 contract，會出現第二層 drift
- 若 `## Tasks` 粒度過粗，會重複 `## Approach`；若過細，則維護成本過高
- 若 `## Analyze` 寫得太抽象，最後會退化成另一段 review prose，而不是可消費的 drift signal
- 若 `clarify` 沒有清楚定義 resolved 規則，`## Open Questions` 容易累積過期問題
- 這份檔案本身是歷史遺留例外：目前只有 `.prompt.md`、缺少對應 source plan doc。實作時需避免把這個例外誤當成未來 plan artifact 的目標形狀
- Open Questions / Tasks / Analyze 的最小 schema 雖已大致收斂，但若不在 P0 寫成具體例子，implementation 時仍可能重新打開格式爭論

## Approval

- Architect verdict: satisfied after repo/documentation smoke check
- Human approval: pending manual deletion by user

---

## Status

Workflow: DONE
Step: 4 of 4
Last activity: 2026-04-01 — all four phases implemented; all requirements, test cases, and success criteria satisfied
Next step: plan can be deleted

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |
| P4 | Treat README alignment as a later documentation phase | README / README.zh-Hant are already aligned to the current command surface baseline; remaining README work is only about the new plan sections | The repo moved forward after the first draft, so README is no longer the slowest part of this plan |
| P0 baseline | Treated `.prompt.md` mostly as a generic plan prompt artifact | Repo has now converged on `.md` = source plan doc, `.prompt.md` = execution work file and canonical state vector | The internal contracts advanced after the original draft, so the plan must preserve this split instead of redesigning it |
| Lifecycle precondition | Left active plan initialization timing implicit | Explicitly moved paired `.prompt.md` initialization into planning so clarify has a canonical write target from `/office-hours` onward | Without this, the new sections would exist on paper but not in the actual planning lifecycle |

### Handoff Notes

這份計畫仍採最小變更策略：不新增 `/clarify`、`/analyze`、`/tasks` 三個獨立命令，而是把這些能力嵌入既有 planning → review → ship 鏈條。這樣既能借到 spec-kit 的結構化優勢，也不會破壞 GAL 已形成的命令心智模型。

2026-04-01 refresh：repo 已正式收斂為 `.md` = source plan doc、`.prompt.md` = execution work file / canonical state vector。這代表本計畫的第一步不是再討論 `.prompt.md` 應不應該承載狀態，而是保留這個分工，並把 `## Open Questions`、`## Tasks`、`## Analyze` 補進現有 execution-work-file contract。真正的缺口落在 template、office-hours lifecycle、workflow、planner、specialist skills 與 control-plane 尚未跟上這三個 section。

這次 refresh 另外補上了 implementation 前置決議：paired active plan file 必須在 planning 階段就初始化；`## Open Questions`、`## Tasks`、`## Analyze` 都屬於 execution work file 的 state sections；`/plan-eng-review` 是 `## Tasks` 的唯一初始化者；`/review` 是 `## Analyze` 的唯一 verdict writer；`/ship` 先將三個新 signals 視為 readiness warnings，而不是新增 hard gate。

## Test Results

2026-04-01 smoke check completed against the repo contract.

- Verified `templates/plan.md` exposes `## Open Questions`, `## Tasks`, and `## Analyze`
- Verified `/office-hours` initializes paired `.prompt.md` execution work files with all three scaffolds
- Verified `/plan-ceo-review`, `/plan-design-review`, and `/plan-eng-review` read/write `## Open Questions` and `## Tasks` according to ownership rules
- Verified `/review` writes `## Analyze`, and `/ship`, `/gal status`, `/gal whats-next` consume the new sections
- Verified workflow, planner, contract docs, onboarding docs, and both READMEs describe the `.md` / `.prompt.md` split consistently

Result: PASS

## Review Results

### Smoke Review

**Date:** 2026-04-01

- No remaining contract drift found in the target command docs
- Updated `docs/ai-agent-onboarding.md` and `docs/gstack-integration.md` so the docs set now matches the implemented execution-work-file model
- Plan bookkeeping reconciled: requirements, test cases, success criteria, status, and approval state now agree

<!-- STAFF_REVIEW: CLEAR -->