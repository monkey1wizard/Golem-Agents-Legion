# Plan: GAL artifact 路徑與檔名整合

## Goal

重新整理 GAL 在 `.dev/` 與 `docs/` 之間的 artifact 存放方式、路徑責任與命名規則，讓使用者只看檔案位置與檔名，就能大致回答三個問題：

1. 這個檔案是永久文件還是暫時執行記憶。
2. 這個檔案是哪個階段產生的。
3. 這個檔案屬於哪一個 plan、feature 或工作流。

最終效果不應只是把路徑重新命名，而應建立一套可推理的 artifact taxonomy：repo-level state 放哪裡、plan doc 放哪裡、AI execution prompt 放哪裡、衍生報告放哪裡、永久知識放哪裡，以及它們應如何命名。

## Context

目前 GAL 的 artifact 分布雖然已有基本邏輯，但對第一次接觸 repo 的人來說，仍有幾個可理解性問題：

- 高階邊界其實已經被文件化：`README.md`、`README.zh-Hant.md`、`docs/per-repo-context.md`、`docs/ai-agent-onboarding.md`、`workflows/coding.md` 都已明確區分 repo-level state 與 durable docs，但目前仍把 `docs/plans/*.prompt.md` 同時當作 plan 與 execution memory 來描述。
- 目前 repo 內大部分文件也已經一致使用 `docs/plans/*.prompt.md`，沒有真的出現一半寫 `.dev/plans/`、另一半寫 `docs/plans/` 的分裂現象。

- `docs/plans/*.prompt.md` 名稱同時承載「計畫」與「prompt」語意，但實際上這些檔案在 GAL 內是執行記憶、review/test state 容器與 handoff surface，不只是 prompt。
- `.prompt.md` 到目前為止更像是「conflated plan/prompt artifact」的歷史殘留，而不是被正向定義過的 artifact 類型；這使得 checklist、執行步驟、write-back target 與 human-readable plan doc 混在同一個語意層。
- `docs/` 直覺上應存放持久文件，但 `docs/plans/` 中的 plan 檔案又被設計為臨時 artifact，verify / absorb 後理論上應刪除，語意上與永久文檔混在一起。
- `docs/qa-reports/`、`docs/design-reports/`、`docs/benchmarks/` 等衍生報告目錄，若缺少統一命名規則，使用者很難從檔名看出它們對應哪個功能、哪一輪 QA、哪次 benchmark。
- `docs/designs/<slug>/` 的 slug 與 plan slug 不一定一致，造成設計變體與 plan artifact 的關聯需要靠人腦補。
- `.dev/state.md` 與 plan `## Status` 的角色雖已在文件中定義，但光看檔名與目錄名稱，仍不夠直觀地傳達 repo-level 與 plan-level 的分工。

2026-04-01 naming direction 已收斂為以下原則，這份計畫接下來以對齊文件與 command contract 為主，不再停留在抽象命名辯論：

- 本輪將 source plan doc 收斂為 `docs/plans/<plan-slug>.md`；`plan-slug` 本身使用 en-US feature 描述，沿用目前 repo 已採用的 descriptive kebab-case 風格。
- `.prompt.md` 必須在本輪被正向定義為 AI execution file format：它是和 AI 討論後，從 docs 與討論內容整理出的工作用執行檔，承載 per-task checklist、執行步驟、write-back target 與 mutable work state，而不是單純的 plan 副檔名。
- `.prompt.md` 的指令文本、checklist 與 execution notes 預設使用 en-US 撰寫；只有在需要保留原始產品文案、使用者輸入或引用內容時，才保留非英語片段。
- 目前 `docs/plans/*.prompt.md` 視為 legacy conflated artifact；migration 時需同時處理兩件事：把 source plan doc 與 prompt work file 的語意切開，並保留同一 `plan-slug` 作為關聯鍵。
- repo root `DESIGN.md` 是 repo-level、durable 的 design source of truth；`docs/designs/<plan-slug>/` 則是單一 plan 的 design artifacts 與 design handoff package。
- `docs/design-reports/` 是 design review / audit 的 durable 輸出，語意上不同於 `docs/designs/`。
- screenshots 分成兩種命名規則：plan-bound 一律使用 `<plan-slug>-NNN[-suffix].png`，其中 numeric sequence 是 canonical form，`suffix` 僅在 evidence workflow 需要時使用；沒有關聯 plan 的 ad-hoc screenshots 才使用 `<slug>-YYYYMMDD-HHmmss.png`，若連穩定 slug 都沒有則純 timestamp。
- durable report / evidence naming contract 收斂如下：`docs/qa-reports/YYYYMMDD-<plan-slug>.md`、`docs/qa-reports/YYYYMMDD-<plan-slug>-report-only.md`、`docs/design-reports/YYYYMMDD-<plan-slug>-rNN.md`、`docs/benchmarks/YYYYMMDD-HHmmss-<url-slug>.json`、`docs/research/YYYYMMDD-<plan-slug>-<topic>.md`、`docs/retros/YYYYMMDD.json`。
- `docs/designs/<plan-slug>/` 內的 canonical filenames 收斂為 `variant-approved.json`、`variant-approved.png`、`handoff-final.html`；plan `## Review Results` 只保留摘要與 verdict，完整 evidence 改放外部 report 與 screenshots。

換句話說，現在真正未完成的不是「先把邊界講清楚」，而是「既然邊界已經講清楚，是否還要接受目前這套命名與路徑語意」。本計畫的目標是讓 artifact 結構本身就能說明工作流程，而不是永遠依靠說明文件替它補語意。

## Requirements

[x] `.dev/` 與 `docs/` 的高階責任邊界已在 README、onboarding、per-repo context 與 workflow 文件中被明確定義
- [x] 必須把「`.prompt.md` 是 AI execution file format，而 `docs/plans/<plan-slug>.md` 是 human-readable source plan doc，兩者以 `plan-slug` 關聯」明確寫進文件與契約
- [x] 必須定義 plan 檔與 plan 衍生報告的關聯方式，避免 QA / design / benchmark 報告變成孤兒檔案
- [x] 必須定義 plan slug 的產生與重用規則，讓同一功能的衍生 artifact 能被一致追溯
[x] 必須定義 `docs/qa-reports/`、`docs/design-reports/`、`docs/benchmarks/`、`docs/research/`、`docs/retros/`、`docs/screenshots/` 的命名規則與是否保留在 `docs/`
[x] 必須把 `DESIGN.md` 與 `docs/designs/<plan-slug>/` 的責任切開，避免 design governance 與 plan-level design artifacts 混在一起
[x] 哪些 artifact 是 lifecycle-bound、哪些是 durable knowledge，已在現有文件中有明確說明；剩下要處理的是命名是否真正表達了這個差異
[x] 目前 README、onboarding、workflow、template、agent 與 contract 文件對 `docs/plans/*.prompt.md` 的描述仍混合了 plan 與 prompt 語意；本輪目標是先把 `.prompt.md` 的定義寫準，再完成 plan path 與 prompt artifact 的對齊
[x] 必須明確區分 per-task mutable checklist 與 reusable specialist checklist：前者屬於 prompt work file，後者留在 workflow、agent 與 skill contract
[x] 必須定義 prompt work file 的預設內容語言為 en-US，避免不同語言的 checklist / instruction style 破壞 agent 執行一致性
[x] 本計畫不得順手重寫 command surface；只處理 artifact taxonomy、路徑與命名規則

## Phases

### P0: Artifact Inventory And Taxonomy

- **Scope**
  - 整理目前已經存在於 README、onboarding、per-repo context、workflow 文件中的 artifact taxonomy，確認哪些邊界其實已經被說清楚
  - 補齊目前仍未被命名規則體現的部分：source plan doc 語意、prompt work file 語意、report 與 plan 的關聯、durable report 與 ephemeral execution artifact 的命名差異
  - 將 artifact 分成五層並標示哪些已完成文件化、哪些仍只靠慣例運作：repo-level durable state（`.dev/`、`DESIGN.md`）、human-readable source plan docs（target state: `docs/plans/*.md`）、AI execution prompt work files（`.prompt.md`，target path 與 migration policy 在後續階段收斂）、plan-bound design artifacts（`docs/designs/<plan-slug>/`）、durable reports / evidence（`docs/design-reports/`、`docs/qa-reports/`、`docs/benchmarks/`、`docs/screenshots/`、`docs/research/`、`docs/retros/`）
  - 明確記錄目前已存在的部分慣例與本輪要收斂的 target contract：QA report 採 `YYYYMMDD-<plan-slug>.md`、design report 採 `YYYYMMDD-<plan-slug>-rNN.md`、benchmark 採 `YYYYMMDD-HHmmss-<url-slug>.json` 並以相同 `url-slug` 的最近一次結果作為 comparison baseline、plan-bound screenshots 採 `<plan-slug>-NNN[-suffix].png`

- **Files**: `README.md`, `README.zh-Hant.md`, `docs/per-repo-context.md`, `docs/ai-agent-onboarding.md`, `workflows/coding.md`, `docs/gstack-command-contracts.md`, `docs/gal-control-plane-contracts.md`
- **Verify**: 已有文件中的 taxonomy 被整合成單一清單，且能清楚看出已解決問題與仍待設計的命名問題

### P1: Path And Naming Convention Design

- **Scope**
  - 將 source plan doc naming contract 寫死為 `docs/plans/<plan-slug>.md`，其中 `plan-slug` 為 en-US descriptive kebab-case；legacy `docs/plans/*.prompt.md` 視為 conflated old form，而不是 canonical target
  - 定義 `.prompt.md` 為 AI execution file format：由 docs 與討論內容整理出的工作用執行檔，承載 checklist、執行步驟、write-back target 與 mutable status；它不是單純的 plan 副檔名，也不是 durable doc 類型
  - 定義 prompt work file 內容語言：section headings、instructions、checklist items、write-back guidance 預設使用 en-US；只有 literal product copy、quoted text 或必須保留的原文內容可例外
  - 定義 `plan-slug = source plan file basename without .md`，並要求所有 plan-bound artifact 與 prompt work file 重用同一 slug
  - 定義 repo-level `DESIGN.md` 與 `docs/designs/<plan-slug>/` 的分工：前者只承載 design governance / system-level rules，後者承載 design variants / approved mockups / finalized handoff artifacts
  - 將 `docs/designs/<plan-slug>/` 的 canonical filenames 寫死為 `variant-approved.json`、`variant-approved.png`、`handoff-final.html`
  - 定義 `docs/design-reports/` 與 `docs/designs/` 的差異：前者是 review / audit outputs，後者是 design assets
  - 定義 screenshot naming：plan-bound 使用 `<plan-slug>-NNN[-suffix].png`，其中 `NNN` 從 `001` 開始遞增；`suffix` 只允許 evidence workflow 需要的語意標記，例如 `before`、`after`、`finding-001`。ad-hoc 使用 `<slug>-YYYYMMDD-HHmmss.png`，若無穩定 slug 則純 timestamp
  - 定義 QA / design / benchmark / research / retro 的 naming contract：`YYYYMMDD-<plan-slug>.md`、`YYYYMMDD-<plan-slug>-report-only.md`、`YYYYMMDD-<plan-slug>-rNN.md`、`YYYYMMDD-HHmmss-<url-slug>.json`、`YYYYMMDD-<plan-slug>-<topic>.md`、`YYYYMMDD.json`
  - 定義 source plan doc、prompt work file 與外部 evidence 的責任：source plan doc 保留 human-readable scope / rationale；prompt work file 承載 per-task mutable checklist 與 execution state；外部 report 與 screenshots 承載完整 evidence

- **Files**: `templates/plan.md`, `templates/templates.md`, `README.md`, `README.zh-Hant.md`
- **Verify**: 新使用者看到路徑與檔名時，不需回頭查契約文件，也能大致知道 artifact 的用途與所屬階段

### P2: Contract And Template Alignment

- **Scope**
  - 將 artifact taxonomy、prompt semantics 與命名規則同步更新到 control-plane contract、gstack command contract、per-repo context 與 templates
  - 確保各 specialist command 的 output path 說明與新的 taxonomy 一致，特別是 `/office-hours`、`/design-shotgun`、`/design-html`、`/design-review`、`/browse`、`/qa`、`/qa-only`、`/benchmark`、`/canary`、`/retro` 對 source plan doc、prompt work file、`plan-slug`、`DESIGN.md`、screenshots、benchmark baseline 比對的使用方式
  - 明確把 reusable specialist checklist 留在 workflow / agent / SKILL files，把 per-task mutable checklist 留在 prompt work file contract
  - 補上必要的 artifact mapping 表格，避免 README 與 contract 文件漂移

- **Files**: `docs/per-repo-context.md`, `docs/gstack-command-contracts.md`, `docs/gal-control-plane-contracts.md`, `templates/plan.md`, `templates/templates.md`, `docs/ai-agent-onboarding.md`, `workflows/coding.md`, `workflows/research.md`, `commands/office-hours/SKILL.md`, `commands/design-shotgun/SKILL.md`, `commands/design-html/SKILL.md`, `commands/design-review/SKILL.md`, `commands/browse/SKILL.md`, `commands/qa/SKILL.md`, `commands/qa-only/SKILL.md`, `commands/benchmark/SKILL.md`, `commands/canary/SKILL.md`, `commands/retro/SKILL.md`, `agent/golem-planner.agent.md`
- **Verify**: 任一文件或 plan-producing command 提到 artifact path 時，都遵守同一套命名與路徑慣例

### P3: Migration Policy

- **Scope**
  - 明確記錄本輪先做 semantic migration，再做 path migration：先把 `.prompt.md` 從 legacy plan extension 改定義為 AI execution file format，再決定 canonical prompt path 與 legacy artifact 的搬遷策略
  - source plan doc 的 canonical path 收斂為 `docs/plans/*.md`；既有 `docs/plans/*.prompt.md` 視為 conflated legacy artifact，更新引用時不得再把它描述為唯一正確的 target state
  - 定義新 naming contract 先對新產物生效；既有 QA / design / benchmark / screenshot artifacts 採 grandfathered policy，除非 command contract 明確要求補名
  - 說明哪些遷移是文件層決策，哪些需要腳本或 command 行為配合，避免 repo 同時存在一套 prompt semantics 與另一套 runtime 行為

- **Files**: `README.md`, `README.zh-Hant.md`, `docs/per-repo-context.md`, `docs/command-dispatch-architecture.md`
- **Verify**: artifact taxonomy 的切換有清楚過渡策略，不會讓 repo 同時存在兩套沒有說明的慣例

## Files to Create or Modify

- `docs/per-repo-context.md` — 重新定義 repo-level / plan-level / durable-report artifact taxonomy
- `docs/gstack-command-contracts.md` — 對齊各 specialist command 的 output artifact 路徑與命名規則
- `docs/gal-control-plane-contracts.md` — 對齊 control-plane 對 active plan 與 continuity artifact 的讀寫路徑
- `docs/ai-agent-onboarding.md` — 對齊新 taxonomy，避免 onboarding 文本停留在舊命名
- `workflows/coding.md` — 對齊 plan lifecycle 與檔名規則
- `templates/plan.md` — 反映新的 plan 檔命名與 artifact 關聯方式
- `templates/templates.md` — 更新 template output path 總表
- `commands/office-hours/SKILL.md` — 對齊新 plan output path 與檔名規則
- `commands/design-shotgun/SKILL.md` — 對齊 `docs/designs/<plan-slug>/` 與 design artifact 命名規則
- `commands/design-html/SKILL.md` — 對齊 `docs/designs/<plan-slug>/` 與 screenshot / finalized artifact 命名規則
- `commands/design-review/SKILL.md` — 定義 `docs/design-reports/` 與 screenshot 的 output naming contract
- `commands/browse/SKILL.md` — 定義 plan-bound vs ad-hoc screenshot naming contract
- `commands/qa/SKILL.md` — 對齊 `docs/qa-reports/YYYYMMDD-<plan-slug>.md` naming contract
- `commands/qa-only/SKILL.md` — 對齊 `docs/qa-reports/YYYYMMDD-<plan-slug>-report-only.md` naming contract
- `commands/benchmark/SKILL.md` — 對齊 `docs/benchmarks/YYYYMMDD-HHmmss-<url-slug>.json` 與 per-URL baseline comparison contract
- `commands/canary/SKILL.md` — 說明 canary artifacts 與 benchmark family 的區別，避免混用 comparison logic
- `commands/retro/SKILL.md` — 對齊 `docs/retros/YYYYMMDD.json` naming contract
- `agent/golem-planner.agent.md` — 對齊 planner agent 的 plan output convention
- `README.md` — 說明新的 artifact taxonomy 與使用者可預期的檔案位置
- `README.zh-Hant.md` — 以 zh-TW 說明新的 artifact taxonomy 與命名規則
- `commands/commands.md` — 若命令表面提到 artifact output path，需一併對齊

## Test Cases

- [ ] 新使用者只看 `.dev/` 與 `docs/` 目錄名稱，即可區分暫時工作記憶與持久文件
- [ ] 新使用者只看 `docs/plans/<plan-slug>.md` 檔名，即可辨識該檔案是 human-readable source plan doc，而不是 AI execution prompt
- [ ] 新使用者只看 `<plan-slug>.prompt.md` 的命名與契約，即可辨識該檔案是 AI 工作用執行檔，承載 checklist / execution state / write-back target，而不是 durable doc
- [ ] prompt work file 內的 section headings、instructions 與 checklist 預設為 en-US，新使用者不需再猜測執行語言或混用語系規則
- [ ] 新使用者只看 QA / design / benchmark report 檔名，即可推測其所屬 feature、輪次或日期
- [ ] 同一個 feature 的 design variants、QA reports、plan-bound screenshots 可以靠一致 `plan-slug` 關聯回 plan
- [ ] 新使用者只看 `DESIGN.md` 與 `docs/designs/<plan-slug>/` 的位置與名稱，即可區分 repo-level design system 與單一 plan 的 design assets
- [ ] screenshots 在有 active plan 時使用 `<plan-slug>-NNN[-suffix].png`，沒有關聯 plan 時才使用 timestamp naming
- [ ] benchmark 多次測試同一 URL 時，可以靠 `YYYYMMDD-HHmmss-<url-slug>.json` 與相同 `url-slug` 的最近一次結果完成穩定比較，而不會因撞名或混檔失去可讀性
- [ ] README、onboarding、workflow、template、agent 與 contracts 對 `.prompt.md` 的描述完成從「含混副檔名」到「AI execution file format」的遷移，並同步對齊 source plan doc path
- [ ] 舊 artifact 與新 artifact 的過渡規則可被文件單獨理解，不需依賴口頭補充

## Success Criteria

- [ ] artifact 的目錄位置本身就能表達它是 temporary working memory 還是 durable documentation
- [ ] artifact 的檔名本身就能表達它屬於哪個階段、哪個 feature，至少也能表達其 lifecycle 類別
- [ ] `.prompt.md` 的語意被正向定義為 AI 工作用 execution file format，而不是只被當成待刪除的舊 plan 副檔名
- [ ] prompt work file 的內容語言規則明確，預設使用 en-US，避免 instruction 與 checklist 語言漂移
- [ ] plan 與 plan 衍生報告之間的關聯不再依賴人腦補完，而是靠統一 slug 與命名規則即可追蹤
- [ ] 現有文件已建立的高階邊界不被破壞，新的命名與路徑決策是建立在既有共識上，而不是重新引入第二套 taxonomy
- [ ] `DESIGN.md` 與 `docs/designs/<plan-slug>/` 的責任分界清楚，前者是 design governance，後者是 plan-bound design artifacts
- [ ] source plan doc、prompt work file 與外部 report / screenshot evidence 的分工清楚：前者提供 human-readable scope / rationale，中者承載 mutable execution state，後者提供完整證據
- [ ] GAL 文件不再同時混用多套 path / naming 語意，降低新使用者的理解成本

## Risks and Open Questions

- 本輪若同時改寫 `.prompt.md` 語意與 path，容易把 semantic migration 與 path migration 混成一次大改；若只改了一半，會短期造成更多混亂
- planner / office-hours / workflow 目前對 `<type>-<slug>` 與 `<feature-slug>` 仍有文件層差異，本輪需先統一至少在 `plan-slug = en-US descriptive basename` 這層語意
- 若 `.prompt.md` 被定義為 AI execution file format，則還需明確決定 canonical prompt path 是否在本輪收斂，或只先收斂語意再延後 path 搬遷
- 若所有 durable outputs 都強制綁定 `plan-slug`，會讓 ad-hoc QA / benchmark / research 變得僵硬，因此需要清楚區分 plan-bound 與 ad-hoc 兩種模式
- 既有 screenshots 與 design/report artifacts 可能不符合新命名規則；若要求全面補名，會增加大量低價值清理成本
- benchmark 與 canary 都會在 `docs/benchmarks/` 留下 durable artifacts，若 comparison contract 沒寫清楚「benchmark 以相同 `url-slug` 的最近一次結果為 baseline、canary 保持獨立 family」，仍可能讓使用者混淆兩者用途

## Approval

- Architect verdict: approved — 本輪將 `.prompt.md` 重新定義為 AI execution file format，並同步收斂 source plan doc、`plan-slug`、`DESIGN.md` 與 artifact taxonomy 的關係
- Human approval: approved — 依此 naming direction 進入 contract / template / skill alignment

---

## Status

Workflow: REVIEW
Step: 4 of 4
Last activity: 2026-04-01 — P2 (Contract and Template Alignment) and P3 (Migration Policy) complete: all command SKILL files (qa, qa-only, retro, benchmark, canary, browse, design-shotgun, design-html, design-review, office-hours), SKILL templates, docs/per-repo-context.md, docs/gstack-command-contracts.md, docs/gal-control-plane-contracts.md, docs/ai-agent-onboarding.md, docs/gstack-integration.md, docs/design-principles.md, agent/golem-planner.agent.md, conventions/token-budget.md, templates/templates.md, README.md, README.zh-Hant.md updated to canonical artifact naming
Next step: verify test cases pass — reviewer to check that artifact naming contracts are consistent and complete across all updated files

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |
| P0 | Start by defining the `.dev/` vs `docs/` boundary from scratch | Repo audit showed that boundary is already documented in README, onboarding, per-repo context, and workflow docs | The remaining problem is not absence of taxonomy text; it is that naming still does not fully express the documented taxonomy |
| P1 | Re-open whether plan files should move or drop `.prompt.md` before any other naming work | Final decision changed from simple rename to a clearer semantic split: source plan docs use `docs/plans/*.md`, while `.prompt.md` must be defined as an AI execution file format | Renaming alone did not answer what `.prompt.md` should mean, so the plan now defines the artifact instead of merely deleting the suffix |

### Handoff Notes

這份計畫的核心不是單純「改檔名」，而是讓 artifact taxonomy 能直接揭示 workflow。只要使用者看到路徑與檔名，就應能判斷它是 repo-level state、plan-level working memory，還是 durable report / knowledge。這個目標若沒達成，再多的文件說明都只是補丁。

2026-03-31 repo audit 補充：高階 taxonomy 其實已經存在，而且 `docs/plans/*.prompt.md` 這條慣例目前在 README、contracts、workflow、planner agent、office-hours 之間相當一致。這讓問題收斂成一個更尖銳的決策：是要接受「語意不夠漂亮但已全面一致」的現況，還是要為了語意清晰承擔一次較大規模的遷移成本。

2026-04-01 決策收斂：本輪不再把 `.prompt.md` 當成單純應被移除的歷史副檔名，而是把它正向定義為 AI 工作用 execution file format。接下來的主軸是把 source plan doc、prompt work file、`plan-slug`、`DESIGN.md`、`docs/designs/<plan-slug>/`、`docs/design-reports/`、`docs/screenshots/`、`docs/benchmarks/`、`docs/research/` 的責任與命名規則寫進文件與 command contract。只要這些規則被對齊，使用者即使不先讀長篇說明，也能從檔名與位置推理出 artifact 的角色與歸屬。

## Test Results

[由 tester golem 在 TEST 階段填寫]

## Review Results

[由 reviewer golem 在 REVIEW 階段填寫]