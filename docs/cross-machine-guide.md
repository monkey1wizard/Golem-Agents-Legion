# 跨主機非同步任務使用手冊

本手冊說明如何使用 GAL 的跨主機非同步派發功能，將 AI 工作任務分配到其他機器（Mac Mini 本機工作階段或遠端 Windows PC）背景執行，讓主工作機器維持可用狀態。

---

## 目錄

1. [架構概覽](#架構概覽)
2. [前置條件](#前置條件)
3. [一次性設定](#一次性設定)
4. [每次工作階段設定](#每次工作階段設定)
5. [撰寫任務規格](#撰寫任務規格)
6. [派發任務到 Mac Mini 本機](#派發任務到-mac-mini-本機)
7. [派發任務到遠端 Windows PC](#派發任務到遠端-windows-pc)
8. [查看執行進度](#查看執行進度)
9. [取回本機任務結果](#取回本機任務結果)
10. [取回遠端 Windows 任務結果](#取回遠端-windows-任務結果)
11. [套用結果 patch](#套用結果-patch)
12. [清理工作目錄](#清理工作目錄)
13. [常見問題](#常見問題)

---

## 架構概覽

```
主工作機 (Win11)
├── 撰寫任務規格 (.md)
├── Invoke-GalRemoteTask.ps1  ─── SSH ──▶  遠端 Windows PC
│                                              └── Start-GalWorker.ps1 (背景 Gemini CLI)
│                                              └── C:\Windows\Temp\gal-worker\<taskId>\
│
└── Invoke-GalLocalTask.sh    ─── SSH ──▶  Mac Mini 本機
                                              └── Zellij 工作階段 (gal-task-<id>)
                                              └── Start-GalWorker.sh (背景 Gemini CLI)
                                              └── /tmp/gal-worker/<taskId>/
```

**重要特性**：
- 任務派發後立即返回，不等待完成
- Mac Mini 任務在 Zellij 中執行，**即使 SSH 斷線、Win11 主機關機也不中斷**
- 每個任務有唯一 TaskId（格式：`yyyyMMdd-XXXXXX`，例如 `20260422-abc123`）

---

## 前置條件

### 全部機器

- `gemini` CLI 已安裝並在 `PATH` 中
- 已設定 `GEMINI_API_KEY` 環境變數（或 `gemini` 已有其他認證）
- Git 已安裝

### Mac Mini（用於本機非同步任務）

```bash
# 確認以下工具已安裝
zellij --version
jq --version
```

如未安裝：

```bash
brew install zellij jq
```

### 主工作機 → Mac Mini SSH 連線

```bash
# 測試免密碼 SSH
ssh macmini-hostname "echo ok"
```

如果需要輸入密碼，請先設定 SSH 金鑰認證：

```bash
ssh-keygen -t ed25519   # 如果沒有 SSH 金鑰
ssh-copy-id user@macmini-hostname
```

### 主工作機 → 遠端 Windows PC SSH 連線（可選）

```powershell
# 測試連線
ssh user@remote-pc "echo ok"
```

遠端 Windows PC 需安裝 OpenSSH Server（Windows 設定 → 應用程式 → 選用功能）。

---

## 一次性設定

每台機器上只需執行一次。

### 複製設定範本

在**每台機器**的 repo 根目錄執行：

```bash
# Mac Mini / Linux
cp config.example.env config.local.env
```

```powershell
# Windows
Copy-Item config.example.env config.local.env
```

### 填寫節點識別資訊

編輯 `config.local.env`（此檔案已被 `.gitignore`，不會上傳 Git），以下為範例：

```bash
# 主工作機（Win11）
GAL_NODE_ID=win11-main
GAL_NODE_CAPABILITIES=planning,review,dispatch

# Mac Mini
GAL_NODE_ID=macmini
GAL_NODE_CAPABILITIES=research,implementation,testing

# 遠端 Windows PC（如有）
GAL_NODE_ID=windows-worker
GAL_NODE_CAPABILITIES=implementation,testing
```

`GAL_NODE_ID` 是自由命名的機器識別名稱；`GAL_NODE_CAPABILITIES` 是逗號分隔的能力標籤，僅用於文件記錄，不影響執行邏輯。

---

## 每次工作階段設定

在開始跨機器任務前，在 `.dev/state.md` 的 `## Session Execution Context` 區塊填入派發資訊：

```markdown
## Session Execution Context

Dispatched node: macmini
Execution mode: async-local
Notes: 研究任務，預估 30 分鐘，Zellij 工作階段 gal-task-20260422-abc123
```

這個區塊讓整個工作階段期間都清楚知道任務在哪裡執行。

---

## 撰寫任務規格

使用 `templates/task.md` 作為範本建立任務說明檔：

```bash
cp templates/task.md task-my-research.md
```

任務規格是一份 Markdown 文件，說明要 AI 執行的工作內容、可用工具、預期輸出格式等。任務規格會被完整傳送到目標機器執行。

---

## 派發任務到 Mac Mini 本機

使用 `Invoke-GalLocalTask.sh` 派發任務到 Mac Mini，任務在 Zellij 中背景執行。

### 基本用法

```bash
bash scripts/Invoke-GalLocalTask.sh \
    --task-spec ./task-my-research.md \
    --repo-path /Users/yourname/Code/Golem-Agents-Legion
```

### 完整參數

```bash
bash scripts/Invoke-GalLocalTask.sh \
    --task-spec ./task-my-research.md \
    --repo-path /Users/yourname/Code/Golem-Agents-Legion \
    --timeout-minutes 60
```

| 參數 | 說明 | 預設值 |
| --- | --- | --- |
| `--task-spec` | 任務規格檔路徑（必填） | — |
| `--repo-path` | Mac Mini 上的 repo 絕對路徑（必填） | — |
| `--timeout-minutes` | 超時分鐘數 | `30` |
| `--task-id` | 自訂 TaskId（通常讓腳本自動產生） | 自動 |
| `--engine` | AI 引擎名稱 | `gemini` |

### 成功輸出範例

```
Task dispatched successfully!
  Task ID:     20260422-abc123
  Session:     gal-task-20260422-abc123
  Worktree:    /Users/yourname/Code/Golem-Agents-Legion-worker-20260422-abc123
  Output dir:  /tmp/gal-worker/20260422-abc123

Monitor:  zellij attach gal-task-20260422-abc123
Results:  bash scripts/Get-GalLocalResult.sh \
              --task-id 20260422-abc123 \
              --output-dir /tmp/gal-worker/20260422-abc123
```

記錄 **Task ID** 和 **Output dir**，稍後取回結果時需要。

---

## 派發任務到遠端 Windows PC

使用 `Invoke-GalRemoteTask.ps1` 透過 SSH 在遠端 Windows 機器背景執行任務。

### 基本用法

```powershell
.\scripts\Invoke-GalRemoteTask.ps1 `
    -RemoteHost windows-worker `
    -RemoteUser alice `
    -RemoteRepoPath "C:\Code\Golem-Agents-Legion" `
    -TaskSpec ".\task-my-research.md"
```

### 完整參數

| 參數 | 說明 | 預設值 |
| --- | --- | --- |
| `-RemoteHost` | 遠端主機名稱或 IP（必填） | — |
| `-RemoteUser` | SSH 使用者名稱（必填） | — |
| `-RemoteRepoPath` | 遠端機器上的 repo 路徑（必填） | — |
| `-TaskSpec` | 本機任務規格檔路徑（必填） | — |
| `-TimeoutMinutes` | 超時分鐘數 | `30` |
| `-RemoteScriptsPath` | 遠端腳本目錄，預設從 `-RemoteRepoPath` 自動推導 | (自動) |

### 成功輸出範例

```
[1/4] Creating remote output directory...
[2/4] Copying task spec to remote...
[3/4] Creating git worktree on remote...
[4/4] Launching background worker on remote...

Task dispatched successfully!
  Task ID:          20260422-abc123
  Remote output:    C:\Windows\Temp\gal-worker\20260422-abc123
  Remote worktree:  C:\Code\Golem-Agents-Legion-worker-20260422-abc123

Retrieve results with:
  .\scripts\Get-GalRemoteResult.ps1 `
      -RemoteHost windows-worker `
      -RemoteUser alice `
      -TaskId 20260422-abc123 `
      -RemoteOutputDir "C:\Windows\Temp\gal-worker\20260422-abc123"
```

腳本會印出完整的取回指令，可以直接複製使用。

---

## 查看執行進度

### Mac Mini 任務

```bash
# 在 Mac Mini 上連接 Zellij 工作階段查看即時輸出
zellij attach gal-task-20260422-abc123

# 或直接查看 status.json（從主工作機 SSH 過去）
ssh macmini "cat /tmp/gal-worker/20260422-abc123/status.json"
```

`status.json` 格式：

```json
{
  "taskId": "20260422-abc123",
  "status": "running",
  "startedAt": "2026-04-22T10:30:00Z",
  "finishedAt": null,
  "exitCode": null
}
```

`status` 可能的值：`running`、`success`、`failed`

### 遠端 Windows 任務

```powershell
# 透過 SSH 查看狀態
ssh alice@windows-worker "Get-Content 'C:\Windows\Temp\gal-worker\20260422-abc123\status.json'"
```

---

## 取回本機任務結果

使用 `Get-GalLocalResult.sh` 取回 Mac Mini 本機任務的結果。

### 等待完成後取回

```bash
bash scripts/Get-GalLocalResult.sh \
    --task-id 20260422-abc123 \
    --output-dir /tmp/gal-worker/20260422-abc123 \
    --wait
```

### 立即取回（不等待）

```bash
bash scripts/Get-GalLocalResult.sh \
    --task-id 20260422-abc123 \
    --output-dir /tmp/gal-worker/20260422-abc123
```

### 完整參數

| 參數 | 說明 | 預設值 |
| --- | --- | --- |
| `--task-id` | TaskId（必填） | — |
| `--output-dir` | 任務輸出目錄（必填） | — |
| `--wait` | 等待任務完成再取回 | — |
| `--poll-seconds` | 輪詢間隔秒數 | `30` |
| `--timeout-minutes` | 等待超時分鐘數 | `60` |
| `--keep` | 取回後保留 worktree 和 Zellij 工作階段 | 預設清除 |
| `--repo-path` | repo 路徑（清理 worktree 時需要） | — |

腳本會印出任務狀態、摘要內容，並提示 `result.patch` 的套用指令。預設會自動清除 worktree 和 Zellij 工作階段；加上 `--keep` 可保留。

---

## 取回遠端 Windows 任務結果

使用 `Get-GalRemoteResult.ps1` 透過 SCP 取回遠端 Windows 機器的任務結果。

### 等待完成後取回（建議）

```powershell
.\scripts\Get-GalRemoteResult.ps1 `
    -RemoteHost windows-worker `
    -RemoteUser alice `
    -TaskId 20260422-abc123 `
    -RemoteOutputDir "C:\Windows\Temp\gal-worker\20260422-abc123" `
    -Wait
```

### 立即取回（不等待）

```powershell
.\scripts\Get-GalRemoteResult.ps1 `
    -RemoteHost windows-worker `
    -RemoteUser alice `
    -TaskId 20260422-abc123 `
    -RemoteOutputDir "C:\Windows\Temp\gal-worker\20260422-abc123"
```

### 完整參數

| 參數 | 說明 | 預設值 |
| --- | --- | --- |
| `-RemoteHost` | 遠端主機名稱（必填） | — |
| `-RemoteUser` | SSH 使用者名稱（必填） | — |
| `-TaskId` | TaskId（必填） | — |
| `-RemoteOutputDir` | 遠端輸出目錄（必填） | — |
| `-LocalOutputDir` | 本機儲存路徑 | `.\gal-results\<TaskId>\` |
| `-Wait` | 等待任務完成再取回 | — |
| `-PollIntervalSeconds` | 輪詢間隔秒數 | `30` |
| `-TimeoutMinutes` | 等待超時分鐘數 | `60` |
| `-KeepRemote` | 取回後保留遠端 worktree 和輸出目錄 | 預設清除 |
| `-RemoteRepoPath` | 遠端 repo 路徑（清理 worktree 時需要） | — |

### 取回的檔案

腳本透過 SCP 取回以下檔案到本機 `.\gal-results\<TaskId>\`：

| 檔案 | 說明 |
| --- | --- |
| `status.json` | 執行狀態、開始/結束時間、exit code |
| `summary.md` | AI 工作摘要、已完成事項 |
| `worker.log` | 完整執行日誌 |
| `result.patch` | 程式碼變更（如有）的 git patch |

取回完成後，預設會自動清除遠端 worktree 和輸出目錄。

---

## 套用結果 patch

如果任務有產生程式碼變更，`result.patch` 會包含 git diff 格式的 patch。

### 檢視 patch 內容

```bash
cat gal-results/20260422-abc123/result.patch
```

### 套用 patch

```bash
git apply gal-results/20260422-abc123/result.patch
```

### 套用前預覽

```bash
git apply --stat gal-results/20260422-abc123/result.patch
git apply --check gal-results/20260422-abc123/result.patch
```

如果 patch 有衝突：

```bash
git apply --reject gal-results/20260422-abc123/result.patch
# 手動解決 .rej 檔案中的衝突
```

---

## 清理工作目錄

### Mac Mini 任務（自動清理）

`Get-GalLocalResult.sh` 預設會自動：
1. 移除 git worktree（`/Users/yourname/Code/Golem-Agents-Legion-worker-<taskId>`）
2. 終止 Zellij 工作階段（`gal-task-<taskId>`）
3. 保留輸出目錄（`/tmp/gal-worker/<taskId>`，含 patch/log）

如需手動清理：

```bash
# 查看所有 gal-related worktree
git worktree list

# 移除特定 worktree
git worktree remove /Users/yourname/Code/Golem-Agents-Legion-worker-20260422-abc123 --force

# 終止 Zellij 工作階段
zellij delete-session gal-task-20260422-abc123

# 清除輸出目錄（確認結果已取回後）
rm -rf /tmp/gal-worker/20260422-abc123
```

### 遠端 Windows 任務（自動清理）

`Get-GalRemoteResult.ps1` 預設會自動：
1. 讀取 `status.json` 取得 worktree 路徑
2. 移除遠端 git worktree
3. 移除遠端輸出目錄（`C:\Windows\Temp\gal-worker\<taskId>`）

如需保留遠端檔案（例如除錯用），加上 `-KeepRemote` 參數。

---

## 常見問題

### Q: Mac Mini Zellij 工作階段消失了？

確認 zellij 還在執行：

```bash
ssh macmini "zellij list-sessions"
```

如果工作階段不存在，查看輸出目錄是否有 log：

```bash
ssh macmini "cat /tmp/gal-worker/20260422-abc123/worker.log"
```

---

### Q: `status.json` 一直顯示 `running` 超過預期時間？

可能原因：
- Gemini CLI 卡住（網路問題、API 限流）
- 任務規格太複雜，需要更長時間

檢查 log：

```bash
# Mac Mini
ssh macmini "tail -50 /tmp/gal-worker/20260422-abc123/worker.log"

# 遠端 Windows
ssh alice@windows-worker "Get-Content 'C:\Windows\Temp\gal-worker\20260422-abc123\worker.log' -Tail 50"
```

---

### Q: `result.patch` 是空的？

如果任務是純研究或分析（不需要修改程式碼），`result.patch` 會是空檔案，這是正常的。結果在 `summary.md` 中。

---

### Q: SSH 連線失敗？

```bash
# 確認 SSH key 已加入
ssh-add -l

# 測試連線
ssh -v user@remote-host "echo ok"
```

---

### Q: `git apply` 失敗，顯示衝突？

patch 是基於任務派發時的 commit 產生的。如果主分支在任務執行期間有新 commit，可能需要手動 rebase 或 cherry-pick：

```bash
# 查看 patch 基於哪個 commit
head -5 gal-results/20260422-abc123/result.patch

# 或先建立一個測試分支試套用
git checkout -b test-apply-patch
git apply gal-results/20260422-abc123/result.patch
```
