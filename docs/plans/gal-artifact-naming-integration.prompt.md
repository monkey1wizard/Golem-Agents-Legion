# Plan: GAL artifact 路徑與檔名整合

## Goal

重新整理 GAL 在 `.dev/` 與 `docs/` 之間的 artifact 存放方式、路徑責任與命名規則，讓使用者只看檔案位置與檔名，就能大致回答三個問題：

1. 這個檔案是永久文件還是暫時執行記憶。
2. 這個檔案是哪個階段產生的。
3. 這個檔案屬於哪一個 plan、feature 或工作流。

最終效果不應只是把路徑重新命名，而應建立一套可推理的 artifact taxonomy：repo-level state 放哪裡、plan-level state 放哪裡、衍生報告放哪裡、永久知識放哪裡，以及它們應如何命名。

## Context

目前 GAL 的 artifact 分布雖然已有基本邏輯，但對第一次接觸 repo 的人來說，仍有幾個可理解性問題：

- `docs/plans/*.prompt.md` 名稱同時承載「計畫」與「prompt」語意，但實際上這些檔案在 GAL 內是執行記憶、review/test state 容器與 handoff surface，不只是 prompt。
- `docs/` 直覺上應存放持久文件，但 `docs/plans/` 中的 plan 檔案又被設計為臨時 artifact，verify / absorb 後理論上應刪除，語意上與永久文檔混在一起。
- `docs/qa-reports/`、`docs/design-reports/`、`docs/benchmarks/` 等衍生報告目錄，若缺少統一命名規則，使用者很難從檔名看出它們對應哪個功能、哪一輪 QA、哪次 benchmark。
- `docs/designs/<slug>/` 的 slug 與 plan slug 不一定一致，造成設計變體與 plan artifact 的關聯需要靠人腦補。
- `.dev/state.md` 與 plan `## Status` 的角色雖已在文件中定義，但光看檔名與目錄名稱，仍不夠直觀地傳達 repo-level 與 plan-level 的分工。

這些問題不是資訊不存在，而是 taxonomy 與 naming 沒有把語意直接表達出來。本計畫的目標是讓 artifact 結構本身就能說明工作流程。

## Requirements

- [ ] 必須定義 `.dev/` 與 `docs/` 的高階責任邊界，讓使用者可從目錄層級判斷 artifact 是暫時還是持久
- [ ] 必須重新檢討 `docs/plans/*.prompt.md` 的命名是否仍符合其實際語意
- [ ] 必須定義 plan 檔與 plan 衍生報告的關聯方式，避免 QA / design / benchmark 報告變成孤兒檔案
- [ ] 必須定義 plan slug 的產生與重用規則，讓同一功能的衍生 artifact 能被一致追溯
- [ ] 必須定義 `docs/qa-reports/`、`docs/design-reports/`、`docs/benchmarks/`、`docs/research/`、`docs/retros/` 的命名規則與是否保留在 `docs/`
- [ ] 必須明確說明哪些 artifact 是 lifecycle-bound，任務結束後可刪；哪些是 durable knowledge，應長期保留
- [ ] README、template 與 contract 文件中的 artifact path 說明必須一致，不得一處講 `.dev/plans/`、另一處仍寫 `docs/plans/`
- [ ] 本計畫不得順手重寫 command surface；只處理 artifact taxonomy、路徑與命名規則

## Phases

### P0: Artifact Inventory And Taxonomy

- **Scope**
  - 完整盤點目前 `.dev/`、`docs/plans/`、`docs/qa-reports/`、`docs/design-reports/`、`docs/designs/`、`docs/benchmarks/`、`docs/retros/`、`docs/research/` 的語意與生命週期
  - 將 artifact 分成三類：repo-level durable state、plan-level ephemeral working memory、durable reports / knowledge
  - 定義每一類 artifact 的目錄邊界與保留策略

- **Files**: `docs/per-repo-context.md`, `docs/gstack-command-contracts.md`, `docs/gal-control-plane-contracts.md`
- **Verify**: 每一種 artifact 都能被歸到明確類別，且其存放位置與保留策略不再互相矛盾

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

- **Files**: `docs/per-repo-context.md`, `docs/gstack-command-contracts.md`, `docs/gal-control-plane-contracts.md`, `templates/plan.md`, `templates/templates.md`
- **Verify**: 任一文件提到 artifact path 時，都遵守同一套命名與路徑慣例

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
- `templates/plan.md` — 反映新的 plan 檔命名與 artifact 關聯方式
- `templates/templates.md` — 更新 template output path 總表
- `README.md` — 說明新的 artifact taxonomy 與使用者可預期的檔案位置
- `README.zh-Hant.md` — 以 zh-TW 說明新的 artifact taxonomy 與命名規則
- `commands/commands.md` — 若命令表面提到 artifact output path，需一併對齊

## Test Cases

- [ ] 新使用者只看 `.dev/` 與 `docs/` 目錄名稱，即可區分暫時工作記憶與持久文件
- [ ] 新使用者只看 plan 檔名，即可辨識該檔案是 plan、執行記憶或 prompt scaffold，而不會誤解其角色
- [ ] 新使用者只看 QA / design / benchmark report 檔名，即可推測其所屬 feature、輪次或日期
- [ ] 同一個 feature 的 design variants、QA reports、benchmark artifacts 可以靠一致 slug 關聯回 plan
- [ ] README、templates 與 contracts 對 plan path 的描述完全一致
- [ ] 舊 artifact 與新 artifact 的過渡規則可被文件單獨理解，不需依賴口頭補充

## Success Criteria

- [ ] artifact 的目錄位置本身就能表達它是 temporary working memory 還是 durable documentation
- [ ] artifact 的檔名本身就能表達它屬於哪個階段、哪個 feature，至少也能表達其 lifecycle 類別
- [ ] plan 與 plan 衍生報告之間的關聯不再依賴人腦補完，而是靠統一 slug 與命名規則即可追蹤
- [ ] GAL 文件不再同時混用多套 path / naming 語意，降低新使用者的理解成本

## Risks and Open Questions

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
Step: 0 of 4
Last activity: 2026-03-31 — plan created from artifact naming and storage discussion
Next step: decide whether to optimize for semantic clarity first or migration cost first

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |

### Handoff Notes

這份計畫的核心不是單純「改檔名」，而是讓 artifact taxonomy 能直接揭示 workflow。只要使用者看到路徑與檔名，就應能判斷它是 repo-level state、plan-level working memory，還是 durable report / knowledge。這個目標若沒達成，再多的文件說明都只是補丁。

## Test Results

[由 tester golem 在 TEST 階段填寫]

## Review Results

[由 reviewer golem 在 REVIEW 階段填寫]