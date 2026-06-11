# 企劃：管理外部 Plugins 與 Skills (Manage External Plugins and Skills)

> **狀態:STALE / DEFERRED — 非 active(2026-06-11,fix-install T-009/R-07 裁決)。** 這是一個**未核准的草稿**(Approval/Review/Tasks 全 Pending),且實作方法錨定在**已刪除的腳本** `scripts/Sync-DevContext.ps1` / `sync-dev-context.sh`(核心計畫 T-016/T-017 已 port 為 `crates/adapters` + `gal sync`)。功能目標(外部 plugin/skill vendor 命名空間 + 安全審查關卡)仍是合理的**未來 backlog**,但若要復活必須對照現行 `adapters` crate / `gal sync` 架構**重新規劃**,不可照本草稿執行。未列入 `.dev/state.md` Active Plans(正確)。

## 目標 (Goal)

讓 GAL 能夠作為統一的跨 AI Provider 管理中心，不只管理 MCP servers，也能管理從 GitHub 或網路上複製下來的外部 plugins 與 skills。這必須確保跨機器重建的穩定性、防止命名衝突，並確保外部程式碼在被注入到生成的轉接層 (adapters) 前，能經過安全的審查。

*(備註：目前 GAL 已包含一些 MCP，例如 [codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) 以及 [chandra](https://github.com/datalab-to/chandra)。我們希望確保這些以及未來的外部 skills/plugins 都能被妥善追蹤與管理。)*

## 需求 (Requirements)

- [ ] 支援外部 plugins/skills 的追蹤與版本控制 (例如：透過 `gal-dependencies.json` 或 Git Submodules)。
- [ ] 提供外部工具專屬的實體資料夾隔離 (例如：`plugins/vendor/` 或 `skills/external/`)。
- [ ] 實作命名空間策略 (例如：`@vendor/repo/skill-name`)，以避免與內建或本地技能發生命名衝突。
- [ ] 擴充 `scripts/Sync-DevContext.ps1` (及其 shell 對應版本)，讓它能掃描外部資料夾，並將找到的 `SKILL.md` 或 `plugin.json` 設定註冊到生成的 Provider adapters 中 (`AGENTS.md`, `GEMINI.md`, `CLAUDE.md` 等)。
- [ ] 建立安全審查關卡 (例如：透過 `golem-auditor` 掃描或人工審查)，外部技能必須通過審查後才允許執行。

## 實作方法 (Approach)

### 步驟 1: 定義依賴管理與儲存方式
- **內容**: 決定要使用 Git Submodules 還是自訂的 `gal-dependencies.json` 清單。建立 `plugins/vendor/` 目錄結構。
- **驗證方式**: 專案能夠將外部技能 clone 到 `plugins/vendor/` 並精確追蹤其版本，而不會污染根目錄的 Git 歷史紀錄。

### 步驟 2: 強制套用命名空間 (Namespacing)
- **內容**: 更新內部的技能載入邏輯，為外部工具加上 vendor 命名空間前綴。
- **驗證方式**: 名為 `git-commits` 的外部技能不會覆蓋本地的 `git-commits` 技能，而是能夠透過 `@vendor/name/git-commits` 來呼叫。

### 步驟 3: 擴充轉接層同步腳本 (Adapter Sync Scripts)
- **內容**: 修改 `scripts/Sync-DevContext.ps1` 與 `scripts/sync-dev-context.sh`，在生成 `AGENTS.md`、`GEMINI.md` 等檔案時將 `plugins/vendor/` 目錄納入考量。
- **驗證方式**: 執行 `/gal init` 後，Provider adapters 中能成功寫入帶有新命名空間的外部技能參考。

### 步驟 4: 實作安全審查關卡 (Security Review Gate)
- **內容**: 定義外部 Prompt 的審查工作流。可以是在安裝時整合 `golem-auditor` 掃描步驟，或是要求清單檔中必須明確留下人工的 `<!-- REVIEW: CLEAR -->` 註解，同步腳本才會將其納入。
- **驗證方式**: 未經審查的外部技能會被 `Sync-DevContext.ps1` 忽略或觸發警告，從而阻止其執行。

## 需新增或修改的檔案

- `scripts/Sync-DevContext.ps1` — 更新以掃描 vendor 資料夾並套用命名空間。
- `scripts/sync-dev-context.sh` — Bash 版本的 vendor 掃描對應更新。
- `docs/collaborative-tools/gstack.md` (或同等文件) — 記錄外部插件的安裝與審查流程。
- `gal-dependencies.json` (新增, 可選) — 用於追蹤外部 URL 與 commit hashes 的清單。

## 測試案例 (Test Cases)

- [ ] 安裝外部技能 -> 驗證它出現在 `plugins/vendor/` 且鎖定在特定的 commit。
- [ ] 對未審查的技能執行 `sync-dev-context` -> 驗證它被阻擋或跳出警告。
- [ ] 對已審查的技能執行 `sync-dev-context` -> 驗證它帶有正確的命名空間並出現在 `GEMINI.md` 中。
- [ ] 透過控制平面 (control plane) 觸發外部技能 -> 驗證它能執行且不會干擾本地技能。

## 成功標準 (Success Criteria)

- [ ] 外部 plugins/skills 可以在全新環境的機器上被穩定還原 (reproducibly restored)。
- [ ] 本地技能與外部技能可以有相同的 basename 而不會發生衝突。
- [ ] 未受信任的外部 prompt 無法在沒有通過審查關卡的情況下，自動注入到生效的 provider adapter 中。

## 風險 (Risks)

- 外部 plugins 可能包含複雜的資料夾結構，如果我們的 `SKILL.md` parser 不夠健全，可能會導致解析失敗。
- 採用 Git Submodules 可能會增加不習慣使用它的獨立開發者的負擔；自訂 JSON 清單可能比較簡單，但需要我們自己撰寫 fetch 邏輯。

## 待解決問題 (Open Questions)

- [ ] OQ-001 — 我們應該使用 Git Submodules 進行 vendor 追蹤，還是使用自訂的 `gal-dependencies.json` 配合輕量級的 fetch 腳本？ *(raised by: planning)*
- [ ] OQ-002 — 安全關卡應該是清單檔中的明確核准標記 (approval flag)，還是單純只要將檔案 commit 進 repo 就算通過？ *(raised by: planning)*

## 審核狀態 (Approval)

- 人工審核 (Human approval): [pending]
- 架構審核 (Architect review): [pending]
- 其他領域審核 (Additional domain review): [not requested]

## 審查結果 (Review Results)

### 架構審查 (Architecture Review)
Pending.

### 商業邏輯審查 (Business Review)
Pending.

### 設計審查 (Design Review)
Pending.

### 工程審查 (Engineering Review)
Pending.

## 測試計畫 (Test Plan)

Pending.

## 任務清單 (Tasks)

Pending.
