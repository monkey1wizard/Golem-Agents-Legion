---
source: docs/collaborative-tools/xmachine.md
lang: zh-Hant
source_commit: PENDING
translated_at: 2026-06-02
status: current
---

# xmachine 協作執行契約

xmachine 是 GAL 的一項可選執行工具，能夠將範圍明確的任務從主要的控制節點（control node）卸載到可透過 SSH 存取的工作節點（work node）。它的運作不會修改 GAL 的控制平面（control plane）、儲存庫擁有的狀態模型（repo-owned state model）或 patch-first 的收斂規則。

`-WorkNode` 參數接受一個 xmachine 工作節點 ID。此 ID 必須是在 `~/.gal/config/xmachine.json` 的頂層 `xmachineNodeAliases` 物件下定義的節點別名（alias）；舊版 `nodes` 僅保留作為過渡 fallback，不一定要在當前開發的目標專案中。xmachine 會將這個別名解析為明確的 SSH 目標（例如 `user@host`），並使用本機的 SSH 客戶端來處理特定的連線設定，例如帳號、主機、通訊埠及金鑰設定。它的就緒狀態（Readiness）是基於本機而非儲存庫：GAL 會將驗證過的節點記錄在 `~/.gal/xmachine-nodes.json`，讓其他儲存庫可以重複使用。節點在通過 SSH、儲存庫、工具和工作節點的冒煙測試（smoke tests）後，會被標記為 `tooling-ready`。只有在通過獨立的 GAL pipeline smoke 關卡後，才會完全轉為 `readied` 狀態。

## 能力與特色

當啟用 xmachine 時，GAL 可以透過跨下列實作路徑的共享契約來執行任務：

- **Windows 工作節點派發 (Windows Work-Node Dispatch)**：透過 SSH 將任務執行派發到 Windows 機器。
- **POSIX 相容背景執行 (POSIX-Compatible Detached Execution)**：在 macOS 或 Linux 目標機器上，利用 Bash 加上可分離的 launcher 執行 bash-based 的本機非同步路徑。bounded direct-task 路徑會優先使用 Zellij，缺少 Zellij 時則會回退到 `nohup`。
- **標準化的執行產物 (Standardized Runtime Outputs)**：產生特定任務的產物，包含 `status.json`、`summary.md`、`runtime.log` 與 `result.patch`。
- **冒煙測試整合 (Smoke-Test Integration)**：提供預先定義的進入點與 patch-first 的擷取工作流，以驗證節點健康狀態。

若不使用 xmachine，GAL 將繼續使用其標準的本機控制平面與 agent 所屬的執行路徑。

## 核心原則與不變性

使用 xmachine 時，以下原則保持不變：

- `/gal` 指令家族依然負責管理公開的控制平面。
- 儲存庫本機的 Markdown 檔案依然是持久化狀態的唯一真相來源（source of truth）。
- 工作節點永遠不會直接進行 git commit。
- Patch 的審查與收斂永遠在主控制平面的 checkout 上進行。
- 專門的 GAL agents 依然保有各自的專家所有權。

## 啟動前檢查模型 (Preflight Check Model)

此工具遵循 [checking-contract.md](checking-contract.md) 中定義的共用 preflight 模型。

| 狀態 (Status) | xmachine 定義 | GAL 行為 |
| --- | --- | --- |
| `not-applicable` | 任務屬於本機控制平面，不需要卸載執行。 | 繼續標準的本機執行。 |
| `unavailable` | 目前機器或 checkout 缺乏所需的執行環境原語（primitives）。 | 回退到本機執行或文件記載的替代路徑。 |
| `available-but-needs-init` | xmachine 契約存在，但尚未驗證任何工作節點。 | 回報缺少設定；不會嘗試自動初始化。 |
| `available-but-not-ready` | 契約存在，但選定的節點僅處於 `tooling-ready` 狀態，尚未通過最終的 pipeline smoke 關卡。 | 回退到非 xmachine 路徑或選擇另一個節點。 |
| `ready` | 所請求的路徑至少有一個處於 readied 狀態的節點可用。 | 當使用者指定了 readied 的工作節點時，路由至 xmachine。 |

xmachine 是屬於儲存庫的公用程式，而非第三方套件。`Setup-Tools.ps1` 只負責回報契約就緒狀態與快取的節點狀態；它不會從外部來源安裝 xmachine。

## 機器前置需求

- **SSH 存取 (SSH Access)**：每個控制節點都必須擁有 SSH 客戶端，並具備存取目標工作節點的權限。
- **儲存庫可見度 (Repository Visibility)**：目標工作節點必須啟用 SSH，並能存取該儲存庫的 checkout。
- **身分驗證 (Authentication)**：你**必須**設定基於 SSH 金鑰的免密碼身分驗證。xmachine 腳本使用 `BatchMode=yes`；需要互動式輸入密碼的連線將會失敗。
- **背景執行 (Detached Execution)**：POSIX direct-task 派發需要可分離的 launcher。首選是 Zellij 加上系統的 `script` 公用程式；若只做 bounded direct task，也支援使用 `nohup` 作為 fallback。獨立的多步驟 pipeline runner 目前仍預期使用 Zellij。
- **POSIX 執行環境工具 (POSIX Runtime Tools)**：bash-based 本機路徑固定需要 `jq`。`git` 在 repo mode 時是必要條件，在純 execute mode 時則不是。
- **系統 PTY 公用程式 (System PTY Utility)**：POSIX 的 `script` 指令只在 launcher 走 Zellij 路徑時才需要。
- **AI 工具 CLI (AI Tool CLIs)**：選定的執行路徑可能需要已設定好的 AI CLI（例如 `gemini`、`copilot`、`claude` 或 `codex`），視當前使用的引擎家族而定。

### POSIX 冒煙測試批次 (POSIX Smoke-Test Batches)

POSIX 工作節點冒煙路徑會分三個獨立批次來檢查就緒狀態：

1. **POSIX 執行環境工具**：`jq`，以及 repo mode 冒煙路徑需要的 `git`
2. **背景 launcher**：`zellij` 加 `script`，或 `nohup`
3. **AI 工具 CLI**：當前任務所需的特定 AI 引擎（例如 `gemini` 與 `copilot`）。

### 就緒狀態進程 (Readiness Progression)

xmachine 的就緒狀態會分兩個可快取的階段推進：

1. **`tooling-ready`**：節點已通過 SSH 連線、runtime checkout 驗證、背景 launcher 檢查，以及工作節點冒煙路徑。若有持久 project checkout，冒煙路徑會用 repo mode；否則就改驗證 execute mode。
2. **`readied`**：節點通過了 `pipeline-smoke`，確認儲存庫本機的 GAL 進入點可以從該節點成功派發 pipeline。

只有 `readied` 節點才會被 `Setup-Tools.ps1` 認定為 `ready`。

## SSH 疑難排解

如果任務派發失敗並顯示 "Permission denied" 或一直提示輸入密碼：

1. **帳號確認**：確保你的連線字串指定了正確的遠端使用者（例如 `user@host`）。若未指定，SSH 預設會使用本機的使用者名稱。
2. **權限 (Linux/macOS)**：SSH 需要嚴格的目錄和檔案權限。在工作節點上執行：
    - `chmod 700 ~/.ssh`
    - `chmod 600 ~/.ssh/authorized_keys`
    - `chmod go-w ~`（確保家目錄不具備群組寫入權限）。
3. **Windows 系統管理員**：如果遠端使用者屬於 Windows 上的 `Administrators` 群組，OpenSSH 可能會忽略 `authorized_keys`。解決方法：
    - 將金鑰加入到 `C:\ProgramData\ssh\administrators_authorized_keys` 並設定嚴格的 ACL，**或者**
    - 在 `C:\ProgramData\ssh\sshd_config` 中將 `Match Group administrators` 區塊註解掉，並重新啟動 `sshd` 服務。
4. **手動測試**：請手動使用 `ssh -o BatchMode=yes user@host` 驗證連線。如果失敗，xmachine 也會跟著失敗。

## 術語表

| 術語 | 定義 |
| --- | --- |
| **控制節點 (Control Node)** | 使用者呼叫 GAL 並檢視任務結果的機器。 |
| **工作節點 (Work Node)** | 一個面向使用者的 ID，可解析為執行任務的特定 SSH 目標。 |
| **Tooling-Ready 節點** | 已通過基本連線與工具檢查，但尚未通過最終 pipeline smoke 關卡的節點。 |
| **Readied 節點** | 已通過所有就緒狀態關卡（包含 `pipeline-smoke`）且可供卸載任務的節點。 |

## 節點設定

在 `~/.gal/config/xmachine.json` 中定義工作節點，為工作節點提供穩定、好記的 ID。

- 頂層的 `xmachineNodeAliases` 物件以工作節點別名作為鍵值（key）。
- 每個節點都必須定義 `target` 值。
- `runtimeRepoPath` 為選填，用來指向遠端 GAL runtime checkout；若未設定，才會回退到節點層級的 `repoPath`。
- `repoPath` 現在比較適合表示 repo mode 要用的持久遠端 checkout，或舊設定中的 GAL runtime fallback 路徑。
- `repoMappings` 為選填，而且應只用在那些真的有同步到該節點、可長期依賴的遠端儲存庫。
- `target` 可以是來自 `.ssh/config` 的 `Host` 條目，或是直接的 `user@host` 字串。
- `-WorkNode` 參數必須與定義的別名相符。

**範例：**

```json
{
  "xmachineNodeAliases": {
    "node-name": {
      "target": "username@mechine-name",
      "repoPath": "/path/to/Golem-Agents-Legion",
      "runtimeRepoPath": "/path/to/Golem-Agents-Legion",
      "repoMappings": {
        "local-ai-tools": {
          "repoPath": "/path/to/local-ai-tools",
          "runtimeRepoPath": "/path/to/Golem-Agents-Legion"
        }
      }
    },
    "mac-mini": {
      "target": "username@username-mac-mini.local",
      "repoPath": "/Users/username/Golem-Agents-Legion",
      "runtimeRepoPath": "/Users/username/Golem-Agents-Legion",
      "repoMappings": {
        "local-ai-tools": {
          "repoPath": "/Users/username/Code/zawip/local-ai-tools",
          "runtimeRepoPath": "/Users/username/Golem-Agents-Legion"
        }
      }
    },
    "win11-pc": {
      "target": "alice@win11-pc",
      "repoPath": "%USERPROFILE%\\Golem-Agents-Legion"
    }
  }
}
```

在這個設定下，`-WorkNode node-name` 在開始 SSH 檢查前會解析為 `username@mechine-name`。

### 儲存庫路徑優先順序

`Test-Xmachine.ps1` 現在會把工作節點的 project/runtime 路徑分開解析。

目標專案 checkout 路徑依下列順序解析：

1. 明確指定的 `-WorkRepoPath`
2. 所選節點上的 `repoMappings.<current-repo>.repoPath`
3. `~/.gal/config/xmachine.json` 中所選節點的 `repoPath`
4. 現有本機快取 `~/.gal/xmachine-nodes.json` 中的路徑

GAL runtime checkout 路徑則依下列順序解析：

1. 所選節點上的 `repoMappings.<current-repo>.runtimeRepoPath`
2. 所選節點上的 `runtimeRepoPath`
3. 所選節點上的 `repoPath`
4. 已解析出的目標專案 checkout 路徑

如果沒有解析出目標專案 checkout，`Test-Xmachine.ps1` 會改成針對 runtime checkout 驗證 direct-task 的 execute mode，而不是因為缺少 repo path 直接失敗。

`Invoke-XmachinePipeline.ps1` 會依下列順序解析遠端目標專案與 GAL runtime 路徑：

1. 目標專案 checkout 的明確 `-WorkRepoPath`
2. 所選節點上的 `repoMappings.<current-repo>.repoPath`
3. 所選節點上的 `repoPath`

遠端 GAL runtime 路徑則依下列順序解析：

1. 明確指定的 `-RemoteRuntimeRepoPath`
2. 所選節點上的 `repoMappings.<current-repo>.runtimeRepoPath`
3. 所選節點上的 `runtimeRepoPath`
4. 已解析出的遠端目標專案路徑

這讓同一個節點可以同時承載 GAL runtime checkout 與不同的目標 repo checkout，而不必強迫所有 repo 共用同一路徑。

`Invoke-XmachineTask.ps1` 與 `Invoke-XmachineTask.sh` 在直接執行 task spec 時，也使用相同的 project/runtime 分流模型：

1. 目標專案 checkout 依序來自明確指定的 `-WorkRepoPath` / `--work-repo-path`，接著是 `repoMappings.<current-repo>.repoPath`。
2. GAL runtime checkout 依序來自明確指定的 `-RemoteRuntimeRepoPath` / `--remote-runtime-repo-path`，接著是 `repoMappings.<current-repo>.runtimeRepoPath`，再來是節點層級的 `runtimeRepoPath`、節點層級的 `repoPath`，最後才回退到已解析出的目標專案 checkout。

如果 direct task 沒有解析出目標專案 checkout，xmachine 現在預設會走 `execute` mode：只把 task spec stage 到遠端暫存 workspace、在那裡執行，完成後再清掉。這代表一般的一次性 test / run 任務不再需要先寫 `repoMappings`。

只有當某個目標 repo 真的有同步到工作節點，而且你要 direct task 直接跑在那份持久 checkout 上時，才需要寫 `repoMappings.<target-repo-folder>.repoPath`。mapping key 取自本機專案根目錄的資料夾名稱，也就是包含 `.dev/state.md` 的那一層；例如本機 checkout 位於 `C:\Code\zawip` 時，key 就是 `zawip`。

`Invoke-XmachinePipeline.ps1` 與 `Invoke-XmachinePipeline.sh` 目前仍然使用持久 repo mode。多步驟 plan 執行尚未切到預設 `execute` mode。

## `/gal xmachine ...` 簡寫

`/gal xmachine <node> to do <task-ref>` 是用來執行單一 active-plan task 的有邊界簡寫。

- dispatcher 會用 pipeline 的 task resolution，把 `FROM` 與 `STOP_AT` 設成同一個 task ref，然後透過 direct `Invoke-XmachineTask` 執行該 bounded task spec。
- direct offload 預設使用 `execute` mode，因此不要求工作節點上存在持久的目標 repo checkout。
- 除非工作節點真的有該目標 repo 的持久 checkout，否則這個 shorthand 不應新增 `repoMappings`，也不應傳入 `-WorkRepoPath`。
- 它是給 `TP-007`、`T-003` 這類 active-plan task ref 用的，不是通用的自由文字遠端提示。
- 所選節點仍然必須是 `readied`，而且該 task 必須存在於解析出的 plan 中。

## 支援的執行路徑

| 路徑 | 主要進入點 | 使用情境 |
| --- | --- | --- |
| **Windows 工作節點** | Windows PowerShell 控制節點 | 透過 SSH/SCP 向 Windows checkout 進行遠端爆發式執行（burst execution）。 |
| **POSIX 相容節點** | PowerShell 控制節點或直接的 shell | 在 macOS 或 Linux 上進行長時間執行的非同步工作；可用時優先走 Zellij，bounded direct task 則可回退到 `nohup`。 |

### 所有權邊界

xmachine 路徑被視為可拋棄式的執行環境：

- 工作節點只擁有暫時性的執行產物和選用的 `result.patch`。
- 控制節點負責管理審查、patch 應用以及持久化狀態的更新。
- 執行程序永遠不會直接 commit 到儲存庫或修改共享的計畫狀態。

### 任務規格所有權

xmachine 不會自行發想有邊界的任務規格 (task specs)。它只執行控制節點已經準備好、並透過 `-TaskSpec` 傳入的任務規格。

- `Invoke-XmachineTask.ps1` 要求提供的 `-TaskSpec` 路徑在派發開始前必須存在。
- 在 `execute` mode 中，task spec 必須足夠 self-contained，讓工作節點不需要目標 repo checkout 也能執行。請放入精確檔案內容、最小重現命令，或暫存 materialization 指令，不要指向只存在於控制節點或已過期遠端 checkout 的路徑。
- 在 `/gal pipeline` xmachine 模式下，pipeline 協調器會留在控制節點上，並可能產生每個階段的有邊界任務規格 Markdown 檔案，讓它可以只把目前的實作、測試、審查、安全性或驗證切片卸載出去。
- 這些 pipeline 產生的任務規格可能會存放在目標專案的 `.dev/` 目錄下，檔名如 `.dev/xmachine-t001-feature1.md`。
- 這些檔案只是供 xmachine 派發用的過渡性控制平面產物，而非像 `.dev/state.md` 或 `.dev/plans/<plan-slug>.prompt.md` 這樣的持久化工作流狀態。
- 因此，當你看到儲存庫本機出現 `.dev/xmachine-*.md` 檔案，通常表示是 `pipeline + xmachine 模式` 準備了一個有邊界的卸載任務；這並不代表工作節點自行建立了新的儲存庫狀態。

除了 `/gal pipeline` 之外，也可以直接使用 xmachine，但在這種情況下，呼叫者必須自行明確提供任務規格。xmachine 是去消耗該檔案，而不是撰寫它。

## 執行階段輸出契約 (Runtime Output Contract)

所有 xmachine 路徑都會為每次任務執行產生一組一致的產物。這些是執行產物，不是 GAL 的持久化狀態：

- `status.json`：單次任務執行的機器可讀狀態（狀態、時間、離開代碼等）。
- `summary.md`：任務執行的人類可讀摘要。
- `runtime.log`：原始執行日誌（stdout/stderr）。
- `result.patch`：repo mode 中變更產生的 git diff；execute mode 則是空檔案。

`status.json` 與儲存庫的 `.dev/state.md` 以及本機快取 `~/.gal/xmachine-nodes.json` 之間是不同的：

- `.dev/state.md` 追蹤儲存庫層級的計畫與工作階段連續性。
- `~/.gal/xmachine-nodes.json` 追蹤目前機器已驗證的工作節點。
- `status.json` 只回報特定 xmachine 任務的結果。

### 輸出目錄

- **遠端 Windows**：`C:\Windows\Temp\gal-xmachine\task-<taskId>`
- **本機非同步**：`/tmp/gal-xmachine/task-<taskId>`

### 擷取工作流 (Retrieval Workflow)

1. 工作節點完成任務。
2. 控制節點讀取 `status.json` 以獲取機器可讀的狀態，並讀取 `summary.md` 以獲取人類可讀的摘要。
3. 若發生錯誤，使用者或控制節點會檢查 `runtime.log`。
4. 若獲得批准，控制節點將審查並套用 `result.patch`。

## 冒煙測試資產 (Smoke Test Assets)

使用這些資產來驗證 xmachine 功能：

- [../../scripts/Test-Xmachine.ps1](../../scripts/Test-Xmachine.ps1)：用於節點驗證的控制節點 wrapper。
- [../../scripts/Test-Xmachine.sh](../../scripts/Test-Xmachine.sh)：POSIX 工作節點 smoke wrapper。
- [../../templates/task-xmachine-remote-smoke.md](../../templates/task-xmachine-remote-smoke.md)：Windows 遠端 smoke 範本。
- [../../templates/task-xmachine-local-smoke.md](../../templates/task-xmachine-local-smoke.md)：POSIX 本機 smoke 範本。

## 進入點 (Entry Points)

### 控制節點冒煙測試 (Control Node Smoke Test)

從 Windows 控制節點驗證並快取一個工作節點：

```powershell
.\scripts\Test-Xmachine.ps1 `
    -WorkNode office-win `
    -WorkRepoPath "C:\Code\Golem-Agents-Legion" `
    -Wait
```

如果選擇的節點在 `~/.gal/config/xmachine.json` 中定義了 `repoPath`，則可省略 `-WorkRepoPath`。

### 控制節點通用派發 (Control Node Generic Dispatch)

從 Windows 控制節點將任務規格派發到設定好的工作節點別名：

```powershell
.\scripts\Invoke-XmachineTask.ps1 `
  -WorkNode node-name `
  -TaskSpec ".\templates\task-xmachine-local-smoke.md" `
  -Wait
```

這個 wrapper 會透過 `~/.gal/config/xmachine.json` 解析節點別名，偵測遠端平台，透過適當的 xmachine 通道派發，並在指定 `-Wait` 時將 `status.json`、`summary.md`、`runtime.log` 與 `result.patch` 取回到本機的 `gal-results\<TaskId>` 目錄。

### Windows 工作節點

**派發任務 (Dispatch Task)：**

```powershell
.\scripts\Invoke-XmachineRemoteTask.ps1 `
    -RemoteHost windows-machine `
    -RemoteUser alice `
    -RemoteRepoPath "C:\Code\Golem-Agents-Legion" `
    -TaskSpec ".\some-task.md" `
    -TimeoutMinutes 30
```

**擷取結果 (Retrieve Results)：**

```powershell
.\scripts\Get-XmachineRemoteResult.ps1 `
    -RemoteHost windows-machine `
    -RemoteUser alice `
    -TaskId <TaskId> `
    -RemoteOutputDir "C:\Windows\Temp\gal-xmachine\task-<TaskId>" `
    -RemoteRepoPath "C:\Code\Golem-Agents-Legion" `
    -Wait
```

### POSIX 工作節點

**派發任務 (Local Shell)：**

```bash
bash scripts/Invoke-XmachineLocalTask.sh \
    --task-spec ./templates/task-xmachine-local-smoke.md \
    --repo-path /Users/yourname/Code/Golem-Agents-Legion \
    --timeout-minutes 30
```

**擷取結果 (Retrieve Results)：**

```bash
bash scripts/Get-XmachineLocalResult.sh \
    --task-id <TaskId> \
    --output-dir <OutputDir> \
    --wait \
    --repo-path /Users/yourname/Code/Golem-Agents-Legion
```

## 操作注意事項 (Operational Notes)

### Session 命名

若使用 Zellij，背景 session 會命名為 `task-<taskId>`；若回退到 `nohup`，則不會建立 Zellij session。

### Patch 處理

從控制節點審查並套用 `result.patch` 檔案：

```bash
git apply --stat path/to/result.patch
git apply --check path/to/result.patch
git apply path/to/result.patch
```

### 清理 (Cleanup)

- `Get-XmachineLocalResult.sh` 會移除拋棄式 worktree、任何對應的 Zellij session，以及輸出目錄，除非指定了 `--keep` 參數。
- `Get-XmachineRemoteResult.ps1` 會移除遠端的暫存資產，除非指定了 `-KeepRemote` 參數。

## 控制節點取用 (Control-Node Consumption)

當控制節點取回 xmachine 產出物時，只讀取回答當前問題所需的最少內容。

**預設取用順序**（回答完問題後即可停止）：

1. **`status.json`** — 機器可讀的結束碼與執行時間；足以確認成功或失敗。
2. **`summary.md`** — 人類可讀的執行結果摘要；在開啟任何原始日誌之前先閱讀此檔。
3. **`runtime.log`** — 僅在 `summary.md` 無法解釋失敗原因時才往上升級。先閱讀檔案尾端（錯誤、traceback）；只有在仍無法確定失敗原因時，才往前取更早的上下文。
4. **`result.patch`** — 供 diff 審查時取回；除非有特定行有爭議，否則不要將原始 diff 完整貼入對話。

**請勿**將完整的 `runtime.log` 載入為預設步驟。此檔案的用途是針對性的錯誤診斷，而非進度追蹤記錄。

## 參考資料 (Reference)

- [checking-contract.md](checking-contract.md)：共用的 preflight 模型。
- [../../scripts/scripts.md](../../scripts/scripts.md)：腳本清清單。
- [../../commands/commands.md](../../commands/commands.md)：公開命令介面。
- [../../workflows/coding.md](../../workflows/coding.md)：執行生命週期與所有權。
