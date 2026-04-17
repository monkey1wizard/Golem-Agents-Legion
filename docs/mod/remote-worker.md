# 遠端執行

GAL 執行平面——透過 SSH 將文字導向任務從主控 PC 派發到 LAN worker 節點。

## 節點拓撲

```text
主控 PC（控制面）                  筆電（LAN burst worker）
┌─────────────────────────┐      ┌───────────────────────────┐
│  /gal orchestrator      │      │  Start-GalWorker.ps1      │
│  Invoke-GalRemoteTask   │─SSH─▶│  Gemini CLI（headless）    │
│  Get-GalRemoteResult    │◀SCP──│  git worktree（隔離）      │
└─────────────────────────┘      └───────────────────────────┘
```

## 節點角色

| 節點 | 角色 | 常駐？ | 任務類型 |
| --- | --- | --- | --- |
| **主控 PC** | 控制面——提交任務、接收結果 | 是 | 所有互動式工作 |
| **筆電** | Burst worker——headless CLI 執行 | 否——按需喚醒 | 研究、審查、repo 掃描、文件 |
| **Mac Mini** | 常駐非同步端點（Phase 2） | 是 | 長時間/過夜任務，Discord / Telegram 接收的遠端人工觸發有界任務 |

## 所有權模型

### Worktree 分類

**主要 Feature Worktree** — 主控 PC 上進行開發、審查與狀態收斂的 worktree。是 `.dev/state.md`、`.dev/project.md` 與 `.dev/plans/<plan-slug>.prompt.md` 的唯一寫入者。`docs/plans/<plan-slug>.md` 是 source plan，可作為 scope 與 rationale 參考，但不是 live execution state 的主要寫入目標。

**可拋棄遠端 Worker Worktree** — 由 `Invoke-GalRemoteTask.ps1` 在 worker 節點為單一有界任務建立的 linked worktree。產出 temp artifacts 和可選的 repo-path 輸出（透過 `result.patch`），但不擁有正式狀態。

### 狀態層級與寫入規則

| 層級 | Artifacts | 主要 Feature Worktree | 可拋棄 Worker Worktree |
| --- | --- | --- | --- |
| Repo 層級正式狀態 | `.dev/project.md`、`.dev/state.md` | 可寫（節制） | 不可——永遠不寫 |
| Source plan | `docs/plans/<plan-slug>.md` | 可更新，但只限 scope / rationale / requirements 層級 | 預設不可 |
| Plan 層級執行 | `.dev/plans/<plan-slug>.prompt.md` | 是——主要寫入者 | 預設不可；僅在明確許可時以 patch-first 方式 |
| 遠端 runtime（暫態） | `status.json`、`summary.md`、`worker.log`、`result.patch` | 否 | 是——唯一擁有者，永不 commit |
| 持久輸出 | `docs/research/`、`docs/qa-reports/` 等 | 是 | 是——透過 result.patch；主控 PC 審閱後套用 |

### 狀態收斂流程

遠端任務完成後，主控 PC 必須關閉迴路：

1. 以 `Get-GalRemoteResult.ps1` 擷取 artifacts
2. 讀取 `summary.md` 並審閱 `result.patch`
3. 適當時套用 patch：`git apply result.patch`
4. 更新 `.dev/plans/<plan-slug>.prompt.md` 的對應區段；若變更屬於永久 scope/rationale，才回寫 `docs/plans/<plan-slug>.md`
5. 僅在 repo 層級 blockers、active-plan index 或 session continuity 改變時更新 `.dev/state.md`

### 規劃與執行分離

新架構下，planning artifacts 分成兩層：

- `docs/plans/<plan-slug>.md`：source plan，給人讀，承接 scope、理由、需求
- `.dev/plans/<plan-slug>.prompt.md`：execution prompt，給 `/gal status`、`/gal whats-next`、`/gal-pipeline` 與 specialist write-back 使用

這代表 remote worker 的預設政策是：

- 不直接擁有 source plan 的語義改寫權
- 不直接擁有 execution prompt 的正式寫入權
- 只回傳 findings、patch 與暫態 runtime artifacts，再由主控 PC 完成 state convergence

## 任務合約

### 輸入：Task Spec

單一 Markdown 檔案，遵循 task 模板：

```text
task-{YYYYMMDD}-{random6}.md
```

放在 worker 的任務輸出目錄中，不 commit 到 repo。

### 執行隔離

每個任務跑在 **linked git worktree** 中：

```text
{repoPath}-worker-{taskId}/
```

Worktree 在任務前建立、結果收集後移除。永不 commit。

### 輸出 Artifacts

所有輸出存在 worker 上的**暫態任務目錄**中：

```text
Windows: $env:TEMP\gal-worker\{taskId}\
macOS:   /tmp/gal-worker/{taskId}/

  status.json      — 機器可讀的狀態 + exit metadata
  summary.md       — 人類可讀的任務摘要與關鍵發現
  worker.log       — Gemini CLI 執行的原始 stdout/stderr
  result.patch     — worktree 中的 git diff（read-only 任務可能為空）
```

### status.json Schema

```json
{
  "taskId": "20260101-abc123",
  "status": "running | success | failed | timeout",
  "exitCode": 0,
  "startedAt": "ISO8601",
  "finishedAt": "ISO8601",
  "engine": "gemini-cli",
  "errorMessage": null
}
```

## 相關文件

- [開發者指南](../devguide.md) — 安裝拓撲與跨機器模型
- [readme](../readme.zh-Hant.md) — GAL 總覽
