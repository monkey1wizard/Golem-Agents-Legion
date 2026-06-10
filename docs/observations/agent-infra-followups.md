# Agent Infrastructure Follow-ups

可累積的 GAL agent(子代理/派工)層觀察與功能項收集檔。**用途**:把零散、跨計畫、不屬任何單一功能計畫的 agent 問題與功能想法集中於此;累積到一定量後一併走 `/planning`(觸及 `plugins/gal-core/agents/` protected path 時走 `/deep-planning`)合併處理。

**狀態圖例**:`OPEN`(待處理)、`INVESTIGATING`、`RESOLVED`(附 commit)、`WONTFIX`(附理由)、`MERGED`(已併入某計畫,附 slug)。

每條請記:現象、證據、根因評估、處置候選、嚴重度、發現來源。**不接受「看起來修好了」**——RESOLVED 須附 commit 或可復現證據。

---

## AI-01 — golem-architect subagent 派工瞬時故障(emit tool-call as prose,0 真實 tool_uses)

- **狀態**:OPEN
- **嚴重度**:LOW(單次、非確定性復現;有 fallback——主代理可直接接手 review)
- **發現來源**:2026-06-10,`/refining-plan` 後對 `docs/plans/fix-install-followups-closeout.md` 派 `gal:golem-architect` 做 architect review 時。

### 現象

`Agent(subagent_type: gal:golem-architect)` 回傳內容失序:開頭以 `@.dev/project.md@.dev/state.md` 互動式檔案引用語法起手,接著宣告「those don't exist」,然後把一個 `<invoke name="Read">…</invoke>` 區塊當**純文字**寫進訊息體,末尾混入 harness 的 `0.00 lines` system-reminder。整次 spawn 未產出任何有效 review。

### 證據

- spawn 回傳 `tool_uses: 0`、`duration_ms: 6050` — 該 agent **從未真正發出任何 tool call**。
- 回傳文字含字面 `<invoke name="Read">` markup(模型「敘述」工具呼叫,而非以 tool-use token 發出)。
- agent 宣稱 `.dev/project.md` / `.dev/state.md` 不存在,但兩檔**實際都存在**(`.dev/project.md` 7.0K、`.dev/state.md` 7.2K)—— 證明它是**幻覺**,根本沒讀。

### 根因評估

| 候選 | 判定 |
| --- | --- |
| agent 定義畸形(含字面 `<invoke>` 範例誘發模仿) | ❌ 排除。`plugins/gal-core/agents/golem-architect.agent.md` 的 `<output_format>` 用 ```markdown fence,無 tool-call XML 範例 |
| `<project_context>` 用 `@file` 風格指示讀 project.md/state.md | ⚠️ 次因(放大失序);但兩檔存在,故非缺檔導致 |
| 模型把 tool call 敘述成 prose、never 發 tool-use token(`tool_uses:0`) | ✅ **主因**。典型 subagent 執行層 glitch,多為非確定性瞬時故障 |

**結論**:主因為瞬時 subagent 執行 glitch,非 `golem-architect.agent.md` 的程式缺陷,亦非任一功能計畫可修。當次以「主代理直接執行 architect review」成功 fallback(見該 plan `### Architecture Review` / 對話記錄)。

### 處置候選(待合併時定奪)

1. **韌性微修(可選,觸 protected path)**:在 `golem-architect.agent.md` `<project_context>` 明確化降級——缺 `.dev/project.md` 時改讀 `CLAUDE.md` 並繼續,**勿宣告中止**;移除可能被誤解為互動式 `@file` 的措辭。觸及 `plugins/gal-core/agents/`(跨 runtime 契約 protected),須走 `/deep-planning`。**同類 `<project_context>` 寫法見於其他 `*.agent.md`,合併時一併套用。**
2. **觀察優先**:若視為瞬時 glitch,不修程式;下次復現升級為 `INVESTIGATING`,並蒐集 spawn transcript 佐證是否模型/路由相關。

### 待復現確認的問題

- 是否與特定 executor 路由(`~/.gal/config/executor-routing.json`)或模型 tier 相關?
- 是否其他 golem agent(同 `<project_context>` 模板)有相同失序傾向?

---

<!-- 後續 agent 問題/功能項由此續寫:AI-02、AI-03 … 同上結構 -->
