# Runtime Verification

GAL 的 live/manual 驗證清單。這份文件會在 plan 檔清理後繼續保留。

這裡只追蹤真實執行驗證，不追蹤規劃過程。用途是記錄 command surface、跨 runtime 一致性，以及 execution plane 的 smoke test 狀態。

## 狀態說明

| 狀態 | 意義 |
| --- | --- |
| 待驗證 | 尚未在 live 環境實際執行 |
| 已驗證 | 已手動執行，且觀察到行為符合預期 |
| 失敗 | 已測出實際錯誤，修正後需重測 |

## Control Plane 與 Specialist 驗證

| 能力 | Runtime | 狀態 | 最後驗證 | 備註 |
| --- | --- | --- | --- | --- |
| `/gal status` 可投影 active plans、review/test 狀態、blockers、continuity、readiness | Copilot chat command | 待驗證 | — | 需要有真實 `.dev/state.md` 與 active plan 資料的 repo |
| `/gal whats-next` 可依記錄中的 workflow state 推導單一下一步 | Copilot chat command | 待驗證 | — | 需要至少一份帶有真實 workflow state 的 active plan |
| `/gal wrap-up` 可正確更新 handoff 與 session continuity | Copilot chat command | 待驗證 | — | 需要針對 live plan 與 `.dev/state.md` 驗證寫回結果 |
| `/plan-eng-review` 可對 active plan 寫入結構化 engineering review | Copilot chat command | 待驗證 | — | 需要 active plan fixture 與輸出檢查 |
| `/review` 可對指定 code changes 寫入結構化 code review | Copilot chat command | 待驗證 | — | 需要 target diff 與 active plan fixture |

## 跨 Runtime 驗證

| 能力 | Runtime | 狀態 | 最後驗證 | 備註 |
| --- | --- | --- | --- | --- |
| 同一份 skill 可透過 Gemini `@file` import 載入 | Gemini CLI | 待驗證 | — | 需要具備 GAL 安裝的真實 Gemini 環境 |
| Mac Mini 的 bash worker 可用與 Windows 相同的 task/result contract 產出 artifacts | Mac Mini over SSH | 待驗證 | — | 需先實作 `Start-GalWorker.sh`，確認 `status.json`、`summary.md`、`worker.log`、`result.patch` 完整一致 |
| Discord / Telegram 遠端入口可把人類請求轉成 bounded task 並進入同一 control-plane path | Mac Mini bridge service | 待驗證 | — | 入口層不可直接執行任意命令，也不可繞過 endpoint selection |
| Mac Mini 的 Gemma 4 / Breeze 2 LOCAL lane 可完成預定的本機任務而不與 Gemini contract 混淆 | Mac Mini local inference | 待驗證 | — | 需定義 invocation 與資源限制，確認不干擾 headless worker |

## Execution Plane 驗證

| 能力 | Runtime | 狀態 | 最後驗證 | 備註 |
| --- | --- | --- | --- | --- |
| `Start-GalWorker.ps1` 在 worktree path 無效時仍會寫出 `status.json` | 直接執行 PowerShell | 已驗證 | 2026-03-31 | 已用不存在的 `WorktreePath` 驗證。script 會輸出 failed `status.json`，不會直接崩潰 |
| `Start-GalWorker.ps1 -TimeoutMinutes` 超時後寫出 timeout `status.json` 並結束 | 直接執行 PowerShell | 待驗證 | — | 可在本機以短 timeout + 長任務模擬；確認 `status` 欄位為 "timeout"、`worktreePath` 欄位存在 |
| `Start-GalWorker.sh` 在 worktree path 無效時仍會寫出 `status.json` | 直接執行 bash | 待驗證 | — | Mac worker 必須與 Windows worker 保持相同失敗 artifact contract |
| `Start-GalWorker.sh` 可在 timeout 後寫出 timeout `status.json` 並結束 | 直接執行 bash | 待驗證 | — | 需確認 Mac worker 逾時後仍保留 `worker.log` 與 `worktreePath` |
| `Get-GalRemoteResult.ps1` 可從 `status.json` 的 `worktreePath` 欄位清除孤立 worktree | Windows PC → Windows notebook over SSH | 待驗證 | — | 需完整 E2E run；確認不需 `-RemoteRepoPath` fallback 即可完成清除 |
| `Invoke-GalRemoteTask.ps1` 若 worker 啟動失敗，可自動清除孤立 worktree | Windows PC → Windows notebook over SSH | 待驗證 | — | 需模擬 worker 啟動失敗（例如 script path 錯誤）驗證 cleanup 觸發 |
| 完整 remote dispatch → worker run → result retrieval 流程可跑通 | Windows PC → Windows notebook over SSH | 待驗證 | — | contract 與 scripts 已存在，但尚未完成端到端 live 驗證 |
| 完整 remote dispatch → worker run → result retrieval 流程可跑通 | Windows PC → Mac Mini over SSH | 待驗證 | — | 需先完成 endpoint profile abstraction 與 `Start-GalWorker.sh` |

## 備註

- 穩定的 command contract 見 [docs/gal-control-plane-contracts.md](gal-control-plane-contracts.md)。
- specialist 語意對照見 [docs/gstack-command-contracts.md](gstack-command-contracts.md)。
- 本檔刻意不保留 rollout 歷史，只保留目前可驗證的 runtime 證據。
