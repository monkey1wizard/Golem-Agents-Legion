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

## Execution Plane 驗證

| 能力 | Runtime | 狀態 | 最後驗證 | 備註 |
| --- | --- | --- | --- | --- |
| `Start-GalWorker.ps1` 在 worktree path 無效時仍會寫出 `status.json` | 直接執行 PowerShell | 已驗證 | 2026-03-31 | 已用不存在的 `WorktreePath` 驗證；script 會輸出 failed `status.json`，不會直接崩潰 |
| 完整 remote dispatch → worker run → result retrieval 流程可跑通 | Windows PC → Windows notebook over SSH | 待驗證 | — | contract 與 scripts 已存在，但尚未完成端到端 live 驗證 |

## 備註

- 穩定的 command contract 見 [docs/gal-control-plane-contracts.md](gal-control-plane-contracts.md)。
- specialist 語意對照見 [docs/gstack-command-contracts.md](gstack-command-contracts.md)。
- 本檔刻意不保留 rollout 歷史，只保留目前可驗證的 runtime 證據。
