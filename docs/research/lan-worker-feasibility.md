# Research Brief: 主 PC 透過 LAN 將 CLI 任務派送到 notebook worker 的可執行性

## Research Question

在現有 GAL 架構下，是否適合把少用的 Windows notebook 變成 LAN 上的無頭 CLI worker，由主 PC 透過 SSH 與 AI CLI 派送任務。若可行，MVP 應如何收斂，且未來 Mac Mini 加入後要如何演進。

## Scope

- 研究對象：Windows PC、Windows notebook、未來 Mac Mini
- 研究重點：OpenSSH、Git worktree、Gemini CLI 的非互動模式與自動化能力
- 非目標：遠端桌面、多機 GPU 推理、完整 distributed queue system

## Raw Findings

- GAL 目前只支援跨機同步方法論與本地工具安裝，沒有內建 LAN/SSH 遠端執行、worker orchestration 或 async queue。來源：repo 文件 [docs/installation-topology.md](../installation-topology.md) 與 [docs/command-dispatch-architecture.md](../command-dispatch-architecture.md)
- Microsoft Learn 確認 Windows 11 可啟用 OpenSSH Server，並可直接透過 `ssh user@host` 遠端登入。OpenSSH for Windows 屬正式支援能力。來源：[OpenSSH for Windows](https://learn.microsoft.com/en-us/windows-server/administration/openssh/openssh_install_firstuse)
- Git 官方文件確認 `git worktree` 適合建立 linked worktree 做隔離執行，並支援 `add`、`lock`、`list --porcelain`、`prune`、`repair` 等操作，可作為遠端任務工作樹生命週期基礎。來源：[git-worktree](https://git-scm.com/docs/git-worktree)
- Gemini CLI 官方 README 與配置文件確認支援非互動模式 `gemini -p`、`--output-format json|stream-json`、設定檔階層、MCP、hooks、approval mode、sandbox，以及工作樹相關選項。來源：[Gemini CLI README](https://github.com/google-gemini/gemini-cli/blob/main/README.md) 與 [Gemini CLI configuration](https://github.com/google-gemini/gemini-cli/blob/main/docs/reference/configuration.md)
- Gemini CLI 官方 cheatsheet 顯示 `--worktree` 目前屬 experimental，需要 `experimental.worktrees` 開啟，適合視為輔助能力，不適合作為 GAL 隔離模型的唯一基礎。來源：[Gemini CLI cheatsheet](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/cli-reference.md)
- Gemini CLI 官方 troubleshooting 提供可腳本化的 exit codes，例如 41（auth）、42（input）、44（sandbox）、52（config）、53（turn limit），適合 worker 腳本做錯誤分流。來源：[Gemini CLI troubleshooting](https://github.com/google-gemini/gemini-cli/blob/main/docs/resources/troubleshooting.md)
- Gemini CLI 原始碼與文件顯示某些 consent / auth 流程在 non-interactive mode 仍可能讀取 stdin，且 `security.disableYoloMode` 或 `admin.secureModeEnabled` 會影響自動執行能力，因此不能預設任務一定可完全無人值守。來源：[extensions consent code](https://github.com/google-gemini/gemini-cli/tree/main/packages/cli/src/config/extensions/consent.ts) 與 [Gemini CLI configuration](https://github.com/google-gemini/gemini-cli/blob/main/docs/reference/configuration.md)
- 你的 notebook 在目前角色設計中本來就不是 local inference 節點，而是 planning / review / remote work 節點。因此它更適合吸收 SSH CLI 背景任務，而不是承接 GPU 型工作。來源：[model-roles.local.md](../../model-roles.local.md)

## Synthesis

### 可行性判斷

結論是可行，但可行的是「主 PC 透過 LAN 把文字導向、可非同步的任務派送到 notebook 的無頭 CLI worker」，不是把 notebook 當成遠端加速卡或第二螢幕。

OpenSSH for Windows 已足以擔任最低層 transport。`git worktree` 已足以擔任最低層隔離機制。Gemini CLI 已足以擔任第一個 worker engine，因為它具備非互動 prompt、JSON 輸出與可辨識 exit code。這三者組合後，可以支撐一個很薄的 MVP：

1. 主 PC 建立 task spec
2. 透過 SSH 要求 notebook 進入指定 repo
3. 在 notebook 建立 linked worktree
4. 用 Gemini CLI 執行單次任務
5. 產出 `summary + log + patch`
6. 主 PC 回收結果

### 主要限制

- 最大限制不是 LAN 頻寬，而是 worker engine 的非互動可靠性與 notebook 的電源/睡眠穩定性。
- 不能把 branch 輸出、worker loop、Mac Mini 規劃、ACP、hooks、sandbox、multi-engine abstraction 一次塞進 MVP，否則設計會比需求先爆炸。
- 不應依賴 Gemini CLI 的 experimental `--worktree` 來保證隔離。隔離應由 GAL 自己用 `git worktree` 管理。

### 架構建議

- 主 PC：control plane，負責提交任務、接收結果、保留主要互動畫面
- notebook：LAN burst worker，負責一次一任務的 research / review / repo scan / docs 整理
- Mac Mini：Phase 2 才加入，定位為 always-on async endpoint / service host

### 第一版協定建議

- 輸入：單一 task spec 檔
- 執行期狀態：放在本機 temp 或 worker state 目錄，不放進 repo tracked artifacts
- 輸出：`status.json`、`summary.md`、`worker.log`、`result.patch`
- 隔離：每任務一個 linked worktree
- transport：OpenSSH + PowerShell
- worker engine：Gemini CLI

## Architecture Review

### Verdict

REVISE

### Blocking Issues

- 目前 plan 尚未先鎖定第一個 worker engine 的非互動前提，卻已經假設 Step 3 可直接自動化執行。
- 目前 plan 把 durable artifact、runtime status、結果輸出混在 `.dev/` 慣例中，容易造成髒狀態與殘留執行期資料。
- 目前 plan 將 `branch 或 patch` 並列成同級輸出，但兩者的同步與清理成本不同，MVP 應只先保留 patch。

### Warnings

- notebook 適合作為 burst worker，不適合先被設計成 always-on service。
- Gemini CLI 可當第一引擎，但不能預設所有 auth / consent 都能完全無 stdin 自動跑完。
- `git worktree` 在 Windows 上可用，但應把 `lock / prune / repair / list --porcelain` 納入腳本生命週期，而不是只做 `add/remove`。

### Recommended MVP Scope Cut

- 只做 Windows PC -> Windows notebook
- 只做 OpenSSH + PowerShell 單次任務
- 只做 Gemini CLI 單一引擎
- 只做 research / review / repo scan / 文件整理
- 只做 `status + summary + raw log + patch`
- 延後 branch return、worker loop、Mac Mini routing、Discord-triggered tasks、ACP、hooks、多引擎抽象

### Plan Reordering Recommendation

建議把原本 plan 的 Step 2 拆成兩段：

1. 先做 engine preflight，驗證 Gemini CLI 在 notebook 上是否能完成真正的 headless 任務
2. 再定義 task/result contract 與 worktree lifecycle
3. 之後才實作 Windows LAN MVP 腳本
4. Mac Mini 規劃改成 Phase 2，不放在 MVP 主線

## Recommended Next Step

將現有 plan 改成兩階段：

- Phase 1：Windows SSH 單次 worker MVP
- Phase 2：Mac Mini always-on async expansion

同時先做一個非常小的 preflight：在 notebook 上手動驗證一次 `gemini -p ... --output-format json` 能在無人工介入下完成指定 repo 任務。若這一步失敗，就不應先寫 orchestration 腳本。

## Sources

- Repo docs: [docs/installation-topology.md](../installation-topology.md)
- Repo docs: [docs/command-dispatch-architecture.md](../command-dispatch-architecture.md)
- Repo plan: [docs/plans/infra-lan-worker-topology.prompt.md](../plans/infra-lan-worker-topology.prompt.md)
- Microsoft Learn: [OpenSSH for Windows](https://learn.microsoft.com/en-us/windows-server/administration/openssh/openssh_install_firstuse)
- Git docs: [git-worktree](https://git-scm.com/docs/git-worktree)
- Gemini CLI README: [README.md](https://github.com/google-gemini/gemini-cli/blob/main/README.md)
- Gemini CLI configuration: [configuration.md](https://github.com/google-gemini/gemini-cli/blob/main/docs/reference/configuration.md)
- Gemini CLI cheatsheet: [cli-reference.md](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/cli-reference.md)
- Gemini CLI troubleshooting: [troubleshooting.md](https://github.com/google-gemini/gemini-cli/blob/main/docs/resources/troubleshooting.md)
