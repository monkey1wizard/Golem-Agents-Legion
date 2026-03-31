# Plan: GAL artifact 路徑與檔名整合

## Goal

重新整理 GAL 在 `.dev/` 與 `docs/` 之間的 artifact 存放方式、路徑責任與命名規則，讓使用者只看檔案位置與檔名，就能大致回答三個問題：

1. 這個檔案是永久文件還是暫時執行記憶。
2. 這個檔案是哪個階段產生的。
3. 這個檔案屬於哪一個 plan、feature 或工作流。

最終效果不應只是把路徑重新命名，而應建立一套可推理的 artifact taxonomy：repo-level state 放哪裡、plan-level state 放哪裡、衍生報告放哪裡、永久知識放哪裡，以及它們應如何命名。

## Context

目前 GAL 的 artifact 分布雖然已有基本邏輯，但對第一次接觸 repo 的人來說，仍有幾個可理解性問題：

- 高階邊界其實已經被文件化：`README.md`、`README.zh-Hant.md`、`docs/per-repo-context.md`、`docs/ai-agent-onboarding.md`、`workflows/coding.md` 都已明確把 `docs/plans/` 描述為 temporary execution memory，而把 `docs/` 描述為長期文件的沉澱區。
- 目前 repo 內大部分文件也已經一致使用 `docs/plans/*.prompt.md`，沒有真的出現一半寫 `.dev/plans/`、另一半寫 `docs/plans/` 的分裂現象。

- `docs/plans/*.prompt.md` 名稱同時承載「計畫」與「prompt」語意，但實際上這些檔案在 GAL 內是執行記憶、review/test state 容器與 handoff surface，不只是 prompt。
- `docs/` 直覺上應存放持久文件，但 `docs/plans/` 中的 plan 檔案又被設計為臨時 artifact，verify / absorb 後理論上應刪除，語意上與永久文檔混在一起。
- `docs/qa-reports/`、`docs/design-reports/`、`docs/benchmarks/` 等衍生報告目錄，若缺少統一命名規則，使用者很難從檔名看出它們對應哪個功能、哪一輪 QA、哪次 benchmark。
- `docs/designs/<slug>/` 的 slug 與 plan slug 不一定一致，造成設計變體與 plan artifact 的關聯需要靠人腦補。
- `.dev/state.md` 與 plan `## Status` 的角色雖已在文件中定義，但光看檔名與目錄名稱，仍不夠直觀地傳達 repo-level 與 plan-level 的分工。

換句話說，現在真正未完成的不是「先把邊界講清楚」，而是「既然邊界已經講清楚，是否還要接受目前這套命名與路徑語意」。本計畫的目標是讓 artifact 結構本身就能說明工作流程，而不是永遠依靠說明文件替它補語意。

## Requirements

[x] `.dev/` 與 `docs/` 的高階責任邊界已在 README、onboarding、per-repo context 與 workflow 文件中被明確定義
- [ ] 必須重新檢討 `docs/plans/*.prompt.md` 的命名是否仍符合其實際語意
- [ ] 必須定義 plan 檔與 plan 衍生報告的關聯方式，避免 QA / design / benchmark 報告變成孤兒檔案
- [ ] 必須定義 plan slug 的產生與重用規則，讓同一功能的衍生 artifact 能被一致追溯
- [ ] 必須定義 `docs/qa-reports/`、`docs/design-reports/`、`docs/benchmarks/`、`docs/research/`、`docs/retros/` 的命名規則與是否保留在 `docs/`
[x] 哪些 artifact 是 lifecycle-bound、哪些是 durable knowledge，已在現有文件中有明確說明；剩下要處理的是命名是否真正表達了這個差異
[x] 目前 README、onboarding、workflow、template、agent 與 contract 文件對 plan path 的描述大致已對齊在 `docs/plans/*.prompt.md`
- [ ] 本計畫不得順手重寫 command surface；只處理 artifact taxonomy、路徑與命名規則

## Phases

### P0: Artifact Inventory And Taxonomy

- **Scope**
  - 整理目前已經存在於 README、onboarding、per-repo context、workflow 文件中的 artifact taxonomy，確認哪些邊界其實已經被說清楚
  - 補齊目前仍未被命名規則體現的部分：plan 主檔語意、report 與 plan 的關聯、durable report 與 ephemeral memory 的命名差異
  - 將 artifact 分成三類：repo-level durable state、plan-level ephemeral working memory、durable reports / knowledge，並標示哪些已完成文件化、哪些仍只靠慣例運作

- **Files**: `README.md`, `README.zh-Hant.md`, `docs/per-repo-context.md`, `docs/ai-agent-onboarding.md`, `workflows/coding.md`, `docs/gstack-command-contracts.md`, `docs/gal-control-plane-contracts.md`
- **Verify**: 已有文件中的 taxonomy 被整合成單一清單，且能清楚看出已解決問題與仍待設計的命名問題

### P1: Path And Naming Convention Design

- **Scope**
  - 決定 plan 主檔是否保留在 `docs/plans/`，或改為更明確表達暫時性的路徑
  - 決定 `.prompt.md` 是否保留，或改成更能表達執行記憶用途的名稱
  - 定義 plan slug 命名規則，以及 plan 衍生目錄或報告檔案如何重用同一 slug
  - 定義 QA report、design report、benchmark、design variants 的命名樣式，至少要能看出 feature 與時間或輪次

- **Files**: `templates/plan.md`, `templates/templates.md`, `README.md`, `README.zh-Hant.md`
- **Verify**: 新使用者看到路徑與檔名時，不需回頭查契約文件，也能大致知道 artifact 的用途與所屬階段

### P2: Contract And Template Alignment

- **Scope**
  - 將 artifact taxonomy 與命名規則同步更新到 control-plane contract、gstack command contract、per-repo context 與 templates
  - 確保各 specialist command 的 output path 說明與新的 taxonomy 一致
  - 補上必要的 artifact mapping 表格，避免 README 與 contract 文件漂移

- **Files**: `docs/per-repo-context.md`, `docs/gstack-command-contracts.md`, `docs/gal-control-plane-contracts.md`, `templates/plan.md`, `templates/templates.md`, `docs/ai-agent-onboarding.md`, `workflows/coding.md`, `commands/office-hours/SKILL.md`, `agent/golem-planner.agent.md`
- **Verify**: 任一文件或 plan-producing command 提到 artifact path 時，都遵守同一套命名與路徑慣例

### P3: Migration Policy

- **Scope**
  - 定義舊路徑與新路徑如何過渡，例如既有 `docs/plans/*.prompt.md` 是否保留、轉址、逐步淘汰，或在 absorb 前不追溯遷移舊 plan
  - 定義已存在的 QA / design / benchmark / design artifact 是否需要補名、搬移或只針對新 artifact 生效
  - 說明哪些遷移是文件層決策，哪些需要腳本或 command 行為配合

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
- `agent/golem-planner.agent.md` — 對齊 planner agent 的 plan output convention
- `README.md` — 說明新的 artifact taxonomy 與使用者可預期的檔案位置
- `README.zh-Hant.md` — 以 zh-TW 說明新的 artifact taxonomy 與命名規則
- `commands/commands.md` — 若命令表面提到 artifact output path，需一併對齊

## Test Cases

- [ ] 新使用者只看 `.dev/` 與 `docs/` 目錄名稱，即可區分暫時工作記憶與持久文件
- [ ] 新使用者只看 plan 檔名，即可辨識該檔案是 plan、執行記憶或 prompt scaffold，而不會誤解其角色
- [ ] 新使用者只看 QA / design / benchmark report 檔名，即可推測其所屬 feature、輪次或日期
- [ ] 同一個 feature 的 design variants、QA reports、benchmark artifacts 可以靠一致 slug 關聯回 plan
- [x] README、onboarding、workflow、template、agent 與 contracts 目前對 plan path 的描述已大致一致指向 `docs/plans/*.prompt.md`
- [ ] 舊 artifact 與新 artifact 的過渡規則可被文件單獨理解，不需依賴口頭補充

## Success Criteria

- [ ] artifact 的目錄位置本身就能表達它是 temporary working memory 還是 durable documentation
- [ ] artifact 的檔名本身就能表達它屬於哪個階段、哪個 feature，至少也能表達其 lifecycle 類別
- [ ] plan 與 plan 衍生報告之間的關聯不再依賴人腦補完，而是靠統一 slug 與命名規則即可追蹤
- [ ] 現有文件已建立的高階邊界不被破壞，新的命名與路徑決策是建立在既有共識上，而不是重新引入第二套 taxonomy
- [ ] GAL 文件不再同時混用多套 path / naming 語意，降低新使用者的理解成本

## Risks and Open Questions

- 因為 repo 目前其實已經廣泛對齊 `docs/plans/*.prompt.md`，任何路徑或副檔名變更都會比初稿估計影響更多文件與技能
- 若將 plan 從 `docs/` 移出，會提升語意清晰度，但也可能破壞既有文件連結與工作習慣
- 若保留 `.prompt.md`，會持續保留歷史語意包袱；若移除，則需處理既有計畫檔與模板慣例
- 若所有衍生報告都強制綁定 plan slug，對 ad-hoc QA / benchmark / research 可能會變得過度僵硬
- 若 durable 與 ephemeral 的分類太理想化，可能會忽略某些 artifact 同時具備兩種性質的實際情況

## Approval

- Architect verdict: pending
- Human approval: pending

---

## Status

Workflow: DRAFT
Step: 1 of 4
Last activity: 2026-03-31 — repo audit refreshed; high-level taxonomy already documented, naming and linkage gaps remain
Next step: decide whether semantic clarity is worth changing `docs/plans/*.prompt.md`, given the repo is already widely aligned on that convention

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |
| P0 | Start by defining the `.dev/` vs `docs/` boundary from scratch | Repo audit showed that boundary is already documented in README, onboarding, per-repo context, and workflow docs | The remaining problem is not absence of taxonomy text; it is that naming still does not fully express the documented taxonomy |

### Handoff Notes

這份計畫的核心不是單純「改檔名」，而是讓 artifact taxonomy 能直接揭示 workflow。只要使用者看到路徑與檔名，就應能判斷它是 repo-level state、plan-level working memory，還是 durable report / knowledge。這個目標若沒達成，再多的文件說明都只是補丁。

2026-03-31 repo audit 補充：高階 taxonomy 其實已經存在，而且 `docs/plans/*.prompt.md` 這條慣例目前在 README、contracts、workflow、planner agent、office-hours 之間相當一致。這讓問題收斂成一個更尖銳的決策：是要接受「語意不夠漂亮但已全面一致」的現況，還是要為了語意清晰承擔一次較大規模的遷移成本。

## Test Results

[由 tester golem 在 TEST 階段填寫]

## Review Results

[由 reviewer golem 在 REVIEW 階段填寫]