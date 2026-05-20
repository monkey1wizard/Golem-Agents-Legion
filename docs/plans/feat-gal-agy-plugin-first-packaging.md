# Plan: GAL AGY Plugin-First Packaging

## Goal

把 GAL 對 AGY CLI 的安裝模式改為 plugin-first：AGY 相關能力不再由 `Setup-Machine` 直接把 skills、command-skills、agents、rules、MCP 設定散寫到多個 AGY 目錄，而是由 GAL repo 產出一個可重建、可版本化、可更新的 AGY plugin bundle，再交由 AGY 的 plugin 載入機制使用。其他 runtime 例如 Copilot、Claude Code、Codex、OpenCode 先維持現況，不跟著這次一起改造。

## Requirements

- [ ] 本計畫只把 AGY CLI 改為 plugin-first；Copilot、Claude Code、Codex、OpenCode 的現有安裝與產生流程保持不變。
- [ ] GAL repo 仍然是唯一 source of truth；plugin 只是 AGY 專用發佈與安裝產物，不得變成手改來源。
- [ ] AGY plugin 產物必須對齊官方 plugin 目錄形態：`plugin.json` 為必需，並可包含 `skills/`、`agents/`、`rules/`、`mcp_config.json`、`hooks.json`。
- [ ] 既有 AGY 專用安裝邏輯必須從「直接寫入散落目錄」收斂成「建置 plugin bundle + 安裝或同步 plugin」。
- [ ] 既有 `commands/*/SKILL.md` 的 GAL command surface 若要進入 AGY，必須經由建置流程轉成 AGY 相容的 plugin skill 內容，而不是再發明第二套 AGY 命令格式。
- [ ] 既有 `skills/`、`agent/`、`conventions/`、`workflows/` 等 source surface 不能因為 AGY plugin 化而被搬到 AGY 專屬目錄結構中當成新的來源。
- [ ] AGY plugin 安裝不得在此 repo 重新引入 `.agents/` 作為必要安裝面；本 repo 先維持不產生 repo-local `.agents`。
- [ ] AGY 的 MCP 必須以 plugin 內 `mcp_config.json` 為主，不再由 GAL 長期維護舊 Gemini MCP 設定或額外散寫共享設定檔。
- [ ] plugin 更新、重裝、移除必須有明確 lifecycle，不可留下舊版 skills 或過期 MCP 設定殘骸。
- [ ] Windows PowerShell 與 Bash 流程必須維持同一個 plugin build/install contract。
- [ ] 文件必須清楚區分「AGY plugin bundle」與「跨 runtime 的 GAL source contracts」，避免未來把其他 runtime 也錯誤綁到 AGY plugin 結構。
- [ ] 這次設計要參考 Codex、Claude Code、Copilot CLI、OpenCode、AGY 的 plugin 機制，但不得為了追求跨平台統一而破壞 AGY-first 的實作簡化目標。
- [ ] plugin 方案必須保留後續擴充空間：若未來其他 runtime 也要 plugin 化，應能重用同一個中介 build graph，而不是重寫一套完全不同的來源模型。

## Cross-Ecosystem Findings

### Shared patterns worth copying

| Ecosystem | Relevant pattern | Implication for GAL |
| --- | --- | --- |
| AGY | plugin 是可部署 bundle，可含 skills、agents、rules、MCP、hooks，安裝後 staged 到 `~/.gemini/antigravity-cli/plugins/<plugin_name>/` | GAL 對 AGY 的最自然整合面就是生成 plugin bundle，而不是直接散寫各子系統目錄 |
| Claude Code | plugin manifest 與 component paths 分離，plugin root 只放 component directories，manifest 只做索引與 metadata | GAL 應把 AGY plugin 視為 generated package，保留清楚的 manifest + payload 邊界 |
| Codex | plugin 以 marketplace/catalog 載入，但仍以 manifest + skills/hooks/MCP/apps 的 package 形式交付 | GAL 可先不做 marketplace，但 build 產物應可版本化、可列目錄、可升級 |
| Copilot CLI | plugin 是包含 manifest、agents、skills、hooks、MCP 的目錄，重新安裝會從 cache 更新 | GAL 應明確定義 reinstall/update 流程，而不是期待 CLI 自動讀 source repo |
| OpenCode | plugin 是 code-first hook/tool extension，不依賴同一種 manifest bundle | 不應硬把所有 runtime 折成 AGY/Claude/Codex 風格的單一 plugin schema；應維持 source contracts，再對不同 runtime 各自 render |

### Constraints and warnings

- AGY 與 Copilot CLI 的 plugin 結構很接近，但 AGY 額外強調 `rules/` 與 `mcp_config.json`；GAL 不能只照抄 Copilot 結構。
- Claude Code 與 Codex 都有快取與版本概念；這代表 GAL 若要 plugin-first，也需要有「建置版本」與「覆蓋/清理舊版」規則。
- Codex 與 Claude 的 hooks 功能很強，但權限與 lifecycle 比 skills 更敏感；GAL v1 不應為了 plugin 化而急著把大量現有行為搬進 hooks。
- OpenCode plugin 是事件與程式碼擴充，不是內容 bundle；它更適合拿來提醒我們保留 source-level abstraction，而不是逼迫所有 runtime 用同一個 on-disk plugin schema。
- 多數 ecosystem 都把 plugin 視為安裝產物而非作者直接編輯目錄；GAL 也應遵守這個分層，避免手改 staged plugin。

### Design decision from the comparison

這次不做「GAL 全 runtime plugin 化」，而是做「GAL source contracts + AGY renderer」。

- Source layer 保持現況：`skills/`、`commands/`、`agent/`、`conventions/`、`workflows/`、`mcp.json` 等仍是作者維護面。
- Build layer 新增 AGY plugin bundle generation。
- Install layer 將 AGY 的 machine setup 從 direct-write 改為 plugin install/sync。
- Other runtimes 暫不跟動；只把這次抽出的 build abstraction 做到未來可重用。

## Approach

### Architecture summary

導入一個 AGY-specific generated artifact，例如 `gal-results/` 或另一個明確的 generated staging root 下的 `gal-agy-plugin/`，由它承接 AGY 所需檔案：

- `plugin.json`
- `skills/`
- `agents/`
- `rules/`
- `mcp_config.json`
- `hooks.json`（若首版真的需要；否則先不生成）

`Setup-Machine` 與相關更新腳本不再直接把 GAL source 逐一連結或寫入 AGY skills/MCP surface，而是：

1. 解析 GAL source contracts
2. 生成 AGY plugin bundle
3. 同步 bundle 到 AGY plugin staging path
4. 清理上一版 GAL managed plugin 殘留
5. 驗證 AGY 可發現該 plugin

### Build model

建立一個「canonical source -> AGY plugin projection」的映射，不直接把 repo 結構原樣暴露給 AGY。

| GAL source | AGY plugin projection | Rule |
| --- | --- | --- |
| `skills/<name>/SKILL.md` | `skills/<name>/SKILL.md` | 直接投影，保留 skill 名稱與內容 |
| `commands/*/SKILL.md` 或 template-baked command skills | `skills/<command-name>/SKILL.md` | 視為 AGY skill bundle，不維持獨立 `commands/` 安裝面 |
| `agent/*.agent.md` | `agents/*.md` 或 AGY 相容 agent 檔名 | 只投影對 AGY 有意義的 agent surface |
| `conventions/` + `workflows/` + 其他 AGY runtime instructions | `rules/` | 經 build 整理成 AGY rules，而不是直接把整個 repo 資料夾塞進 plugin |
| `mcp.json` + `mcp.local.json` merge result | `mcp_config.json` | 保留現有 merge 與 env resolution，但輸出改由 plugin 持有 |

### Step 1: 定義 AGY plugin 作為新安裝邊界

- **Files**: `docs/plans/feat-gal-agy-plugin-first-packaging.md` only in this planning phase; implementation likely touches `scripts/common/Common.ps1`, `scripts/common/common.sh`, `scripts/Update-Skills.ps1`, `scripts/update-skills.sh`, `scripts/Update-Commands.ps1`, `scripts/update-commands.sh`, `scripts/Update-Mcp.ps1`, `scripts/update-mcp.sh`
- **What**: 把 AGY runtime 的 owner boundary 從「多個 setup 腳本各自管理自己的落點」改成「一個 GAL-managed AGY plugin」。
- **Verify**: 能清楚回答 AGY 相關 source 由誰產生、安裝到哪裡、由誰清理、由誰驗證。

### Step 2: 新增 AGY plugin bundle build graph

- **Files**: likely new build helpers under `scripts/` plus PowerShell/Bash shared mapping code
- **What**: 建立一個從 source contracts 生成 plugin payload 的流程，至少包含 manifest、skills、agents、rules、MCP projection。若 AGY hooks 首版沒有必要，就先讓 `hooks.json` 保持 absent。
- **Verify**: 在不碰 AGY home 目錄的前提下，先在 generated staging path 產出完整 plugin tree，且結構符合 AGY 官方 plugin layout。

### Step 3: 將 command-skill 安裝改為 plugin projection

- **Files**: `scripts/Update-Commands.ps1`, `scripts/update-commands.sh`, command baking helpers
- **What**: 停止把 AGY command skills 當成直接安裝目標；改成先烘焙 command skill，再投影到 plugin `skills/`。
- **Verify**: AGY 仍能透過 plugin 使用 `/gal*` 相關能力，但 machine setup 不再直接把 command surface 散寫到 AGY 目錄。

### Step 4: 將 reusable skills 安裝改為 plugin projection

- **Files**: `scripts/Update-Skills.ps1`, `scripts/update-skills.sh`
- **What**: 停止將 GAL skill 直接連到 AGY global skills path；改由 plugin payload 的 `skills/` 承接。
- **Verify**: AGY 的 skill inventory 來自 GAL plugin，而非單獨的 GAL skills 連結。

### Step 5: 將 AGY MCP 安裝改為 plugin-owned `mcp_config.json`

- **Files**: `scripts/Update-Mcp.ps1`, `scripts/update-mcp.sh`
- **What**: 保留現有 `mcp.json` / `mcp.local.json` / env merge 邏輯，但輸出改寫到 generated plugin `mcp_config.json`，再由 plugin install/sync 負責部署。
- **Verify**: GAL-managed MCP 不再依賴 Gemini legacy 設定檔；AGY plugin 單獨持有 MCP 設定。

### Step 6: 收斂 AGY rules 與 runtime instructions

- **Files**: likely `scripts/Update-Personalization.ps1`, `scripts/update-personalization.sh`, sync helpers, possibly new build template files
- **What**: 把目前對 AGY 有意義的 repo-owned instructions 收斂成 plugin `rules/`，但不重新把 repo `.agents/` 當成安裝面。
- **Verify**: AGY 有足夠 rules/context 可載入，且 repo 仍然不會被 setup 腳本新增 `.agents`。

### Step 7: 將 machine setup 改為 plugin install or sync lifecycle

- **Files**: `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh`, shared common helpers
- **What**: machine setup 對 AGY 的職責改為：

  1. 建置 plugin bundle
  2. 安裝或覆蓋 `~/.gemini/antigravity-cli/plugins/gal/`
  3. 更新 GAL managed install-state
  4. 清理舊的 direct-write legacy surfaces

- **Verify**: 跑完 setup 之後，AGY 目錄中的 GAL 變更集中在 plugin staging path，而不是散落於多個 AGY 子路徑。

### Step 8: 補齊 uninstall and upgrade semantics

- **Files**: uninstall/setup/update script surfaces and docs
- **What**: 定義 GAL managed plugin 的版本與清理規則。至少需要：

  - reinstall 時覆蓋同名 plugin
  - uninstall 時刪除 GAL managed plugin payload
  - legacy migration 時移除舊 GAL-managed AGY direct-write assets

- **Verify**: 更新與卸載後不會遺留舊版 skills、agents、rules 或 MCP 設定。

### Step 9: 文件與 generated adapters 對齊

- **Files**: `docs/personalization.md`, `docs/personalization.zh-Hant.md`, `docs/devguide.md`, `scripts/scripts.md`, generated adapters only after source changes
- **What**: 說清楚 AGY 現在是 plugin-first，而其他 runtime 不是。避免文件重新把 AGY 描述成 direct-write install。
- **Verify**: repo 文件、setup 行為、實際 AGY home layout 三者一致。

## Files To Create Or Modify

### Required authored changes in the future implementation

- `scripts/Setup-Machine.ps1` - 將 AGY 安裝流程改為 plugin bundle sync。
- `scripts/setup-machine.sh` - 與 PowerShell 對齊的 Bash 流程。
- `scripts/common/Common.ps1` - 增加 AGY plugin staging/install path 與 lifecycle state。
- `scripts/common/common.sh` - 與 PowerShell 對齊。
- `scripts/Update-Skills.ps1` - 將 AGY skills output 改為 plugin projection。
- `scripts/update-skills.sh` - 與 PowerShell 對齊。
- `scripts/Update-Commands.ps1` - 將 AGY command-skill output 改為 plugin projection。
- `scripts/update-commands.sh` - 與 PowerShell 對齊。
- `scripts/Update-Mcp.ps1` - 將 AGY MCP output 改為 plugin-owned `mcp_config.json`。
- `scripts/update-mcp.sh` - 與 PowerShell 對齊。
- `scripts/Update-Personalization.ps1` - 若 AGY rules/instructions projection 需要在這裡收斂，則更新其責任分界。
- `scripts/update-personalization.sh` - 與 PowerShell 對齊。
- `docs/personalization.md` - 更新 AGY plugin-first 安裝說明。
- `docs/personalization.zh-Hant.md` - 同步中文說明。
- `docs/devguide.md` - 更新維護者視角的 AGY runtime 拓樸。
- `scripts/scripts.md` - 更新腳本責任與 AGY plugin lifecycle。

### Likely new files or generated outputs

- AGY plugin build helper files under `scripts/` or `templates/`.
- Generated AGY plugin staging tree under a generated path such as `gal-results/` or another explicit build directory.
- `plugin.json` template or generated manifest for AGY plugin.
- generated `rules/` payload for AGY plugin.

### Explicitly out of scope for this plan

- Copilot plugin migration
- Claude Code plugin migration
- Codex plugin migration
- OpenCode plugin migration
- repo-local `.agents` reintroduction for AGY in this repo

## Test Cases

- [ ] Bundle structure check: generated AGY plugin tree contains `plugin.json` and any generated `skills/`, `agents/`, `rules/`, `mcp_config.json` expected for the current repo state.
- [ ] PowerShell install check: `./scripts/Setup-Machine.ps1` installs or updates the GAL AGY plugin under `~/.gemini/antigravity-cli/plugins/<plugin_name>/` without scattering direct-write GAL assets elsewhere.
- [ ] Bash install check: Bash setup path produces the same AGY plugin layout and cleanup behavior.
- [ ] Legacy cleanup check: a machine previously using GAL direct-write AGY setup is migrated so old GAL-managed AGY skill/MCP surfaces are removed or no longer authoritative.
- [ ] No repo `.agents` check: setup and sync do not recreate `.agents` in this repo as part of AGY support.
- [ ] Skill availability check: AGY can discover GAL reusable skills from the plugin.
- [ ] Command availability check: AGY can access projected GAL command skills from the plugin-generated skill surface.
- [ ] MCP check: AGY loads GAL-managed MCP servers from plugin `mcp_config.json` and no longer depends on Gemini legacy MCP files for GAL-managed entries.
- [ ] Reinstall/update check: rerunning setup replaces the existing GAL AGY plugin cleanly instead of duplicating content.
- [ ] Uninstall check: removing GAL-managed AGY install removes the plugin payload and related managed residue.
- [ ] Docs alignment check: personalization and devguide docs describe the same AGY plugin-first flow that the scripts actually perform.
- [ ] Cross-runtime isolation check: Copilot, Claude Code, Codex, and OpenCode outputs remain unchanged after the AGY plugin-first migration except where documentation explicitly mentions AGY behavior.

## Success Criteria

- [ ] AGY support in GAL has one clear install boundary: the GAL AGY plugin.
- [ ] GAL source contracts remain repo-owned and are not replaced by hand-edited staged plugin files.
- [ ] AGY machine setup becomes simpler because direct skill, command, and MCP writes collapse into one plugin sync path.
- [ ] The repo does not reintroduce `.agents` as a required AGY install surface.
- [ ] AGY-specific behavior is isolated enough that future plugin-first work for other runtimes can reuse the abstraction without forcing the change now.
- [ ] Documentation and setup scripts describe the same architecture.
- [ ] MCP ownership is unambiguous: GAL-managed AGY MCP config lives in the plugin bundle.
- [ ] Reinstall, update, and uninstall behaviors are deterministic and do not leave stale GAL-managed AGY artifacts behind.

## Risks

- 如果 build graph 設計太貼近 AGY 目錄細節，未來其他 runtime 想重用時會再次耦合到 AGY-specific layout；需要保留 source-level intermediate mapping。
- 如果 plugin projection 直接複製 repo 全量內容，而不是只投影 AGY 需要的 surface，plugin 會變胖且難以維護。
- 如果 command-skill projection 規則不清楚，AGY plugin 內 skill 名稱可能與既有 reusable skills 衝突。
- 如果 legacy cleanup 不完整，使用者會看到 plugin 與舊 direct-write assets 同時存在，導致載入順序與實際來源混淆。
- 如果文件只更新安裝方式而未更新 ownership 說明，未來維護者仍會去手改 staged plugin，重新引入 drift。
- hooks 若過早納入 v1，可能把原本只是靜態內容 bundle 的需求變成高權限 lifecycle automation；首版應盡量延後。

## References

- [OpenAI Codex Build plugins](https://developers.openai.com/codex/plugins/build)
- [Claude Code Plugins reference](https://code.claude.com/docs/en/plugins-reference)
- [GitHub Copilot CLI Creating a plugin](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/plugins-creating)
- [OpenCode Plugins](https://opencode.ai/docs/plugins/)
- [Antigravity CLI Features](https://antigravity.google/docs/cli-features)

## Open Questions

- [ ] OQ-001 - GAL AGY plugin 的固定名稱與版本策略要採用什麼格式，才能同時支援簡單覆蓋更新與未來可能的版本診斷？
- [ ] OQ-002 - `rules/` 應該由哪些 source contracts 組成最小可行集合，才能讓 AGY 有足夠上下文但不把整個 repo instruction surface 無差別灌入？
- [ ] OQ-003 - AGY plugin 首版是否完全不生成 `hooks.json`，還是需要最小 lifecycle hook 來做 setup/self-check？
- [ ] OQ-004 - command-skill projection 命名要如何避免與 reusable skill 名稱衝突，同時保留 `/gal ...` 的心智模型？

## Approval

- Human approval: [pending]
- Architect review: [recommended]
- Additional domain review: [not triggered]
