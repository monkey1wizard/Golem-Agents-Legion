# Plan: GAL Provider Plugin Packaging

## Goal

把 GAL 對 AGY CLI 的安裝模式改為 plugin-first：AGY 相關能力不再由 `Setup-Machine` 直接把 skills、command-skills、agents、rules、MCP 設定散寫到多個 AGY 目錄，而是由 GAL repo 產出一個可重建、可版本化、可更新、可卸載的 AGY plugin bundle，再交由 AGY 的 plugin 載入機制使用。GAL 本體仍然是 README 所描述的跨 provider 文件驅動工作系統：source contracts、repo-owned memory、agents、skills、MCP manifest、pipeline workflow 不被 AGY plugin schema 取代。其他 runtime 例如 Copilot、Claude Code、Codex、OpenCode 先維持現況，不跟著這次一起改造。

## Requirements

- [ ] 本計畫只把 AGY CLI 改為 plugin-first；Copilot、Claude Code、Codex、OpenCode 的現有安裝與產生流程保持不變，但不代表這些 runtime 不適合未來 plugin 化。
- [ ] plugin 化的目標是讓 provider-specific 安裝、更新、診斷與卸載更容易；本計畫先解 AGY，不把 GAL 的跨 runtime source model 改成任何單一 provider 的 plugin model。
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

Reference check date: 2026-05-20.

| Ecosystem | Relevant pattern | Implication for GAL |
| --- | --- | --- |
| AGY | plugin 是 namespaced deployable bundle；`plugin.json` 是必需 marker，`skills/`、`agents/`、`rules/`、`mcp_config.json`、`hooks.json` 可選；安裝後 staged 到 `~/.gemini/antigravity-cli/plugins/<plugin_name>/` | GAL 對 AGY 的自然整合面就是一個 `gal` plugin bundle，而不是直接散寫 `skills/`、`mcp_config.json`、`gal/` 等多個 AGY path |
| Claude Code | plugin 是 self-contained directory；manifest 位於 `.claude-plugin/plugin.json`，component directories 在 plugin root；支援 skills、agents、hooks、MCP、LSP、monitors，且有 user/project/local/managed scopes 與 cache/version lifecycle | GAL 應學它的分層：manifest 只描述 payload，payload 不能依賴 plugin root 外的任意 source path；更新與卸載要有明確狀態 |
| Codex | plugin manifest 位於 `.codex-plugin/plugin.json`；skills、hooks、`.mcp.json`、apps、assets 在 plugin root；透過 marketplace/catalog 安裝到 cache，local plugin 變更後要更新來源並重啟 | GAL 不需要在 AGY 首版做 marketplace，但需要把 plugin 當成 package：有版本、可重建、可覆蓋、可檢查 |
| Copilot CLI | plugin root 必須有 `plugin.json`，可含 agents、skills、hooks、`.mcp.json`；local install 會 cache，變更後必須重新 install，uninstall 使用 manifest name | GAL AGY install 也應以 manifest name `gal` 作為唯一刪除與覆蓋邊界，避免用 path 猜測殘留物 |
| OpenCode | plugin 是 JavaScript/TypeScript module，可 hook events、注入 tools、改 shell env；不是靜態內容 bundle，local plugins 直接從 `.opencode/plugins/` 載入，npm plugins 由 Bun cache | 不能把所有 runtime 折成單一 manifest-bundle schema；GAL 應維持 source contracts，再按 runtime renderer 投影 |

### Constraints and warnings

- AGY 與 Copilot CLI 的 plugin 結構很接近，但 AGY 額外強調 `rules/` 與 `mcp_config.json`；GAL 不能只照抄 Copilot 結構。
- Claude Code 與 Codex 都有快取與版本概念；這代表 GAL 若要 plugin-first，也需要有「建置版本」與「覆蓋/清理舊版」規則。
- Codex 與 Claude 的 hooks 功能很強，但權限與 lifecycle 比 skills 更敏感；GAL v1 不應為了 plugin 化而急著把大量現有行為搬進 hooks。
- OpenCode plugin 是事件與程式碼擴充，不是內容 bundle；它更適合拿來提醒我們保留 source-level abstraction，而不是逼迫所有 runtime 用同一個 on-disk plugin schema。
- 多數 ecosystem 都把 plugin 視為安裝產物而非作者直接編輯目錄；GAL 也應遵守這個分層，避免手改 staged plugin。

### Design decision from the comparison

這次不做「GAL 全 runtime plugin 化」，而是做「GAL source contracts + AGY renderer」。原因不是其他 provider 不適合 plugin 管理，而是每個 provider 的 plugin schema、載入規則、cache/update lifecycle、MCP 檔名、rules/instructions 語意不同；一次硬做成單一 universal plugin 會把抽象層做歪。

- Source layer 保持現況：`skills/`、`commands/`、`agent/`、`conventions/`、`workflows/`、`mcp.json` 等仍是作者維護面。
- Build layer 新增 provider-neutral inventory/projection metadata，但首版只實作 AGY plugin renderer。
- Install layer 將 AGY 的 machine setup 從 direct-write 改為 plugin install/sync。
- Other runtimes 暫不跟動；後續應新增 Codex、Copilot CLI、Claude Code、OpenCode 各自 renderer，而不是拿 AGY layout 直接套用。

### Provider plugin packaging outlook

plugin-first 對 GAL 的長期方向是合理的，尤其 GAL 本來就是在管理多 provider 的 MCP、skills、agents、pipeline。差別在於「用 plugin 管理」可以是共同產品方向，但「plugin 目錄內容」不是共同格式。

| Runtime | Similarity to AGY | Long-term GAL packaging call | Key differences from AGY |
| --- | --- | --- | --- |
| AGY | baseline | This plan: first renderer and migration target | `plugin.json` at staged plugin root, AGY `rules/`, AGY `mcp_config.json`, AGY install root under `~/.gemini/antigravity-cli/plugins/` |
| Codex | high | Strong candidate for next plugin renderer after AGY | Manifest lives under `.codex-plugin/plugin.json`; MCP file is `.mcp.json`; marketplace/catalog/cache semantics differ; no AGY `rules/` equivalent as-is |
| Copilot CLI | medium-high | Good candidate for a later renderer if GAL wants managed Copilot CLI install instead of current repo adapter flow | Root `plugin.json`; supports agents/skills/hooks/`.mcp.json`, but install/reinstall/cache and GitHub Copilot command semantics are provider-specific |
| Claude Code | medium | Useful for managed distribution, but renderer must respect Claude scopes and component semantics | Manifest is `.claude-plugin/plugin.json`; plugin root `CLAUDE.md` is not the same as repo `CLAUDE.md`; scopes, managed installs, LSP/monitors, and validation rules are unique |
| OpenCode | low as static bundle | Treat as a separate event/plugin integration lane, not as a copied AGY-style bundle | Plugin is JS/TS code that hooks events/tools/env; local/npm loading differs; content directories alone are not the main abstraction |

Implementation implication: create reusable source inventory first, then provider-specific renderers such as `Build-AgyPlugin`, future `Build-CodexPlugin`, future `Build-CopilotPlugin`, future `Build-ClaudePlugin`, and a separate OpenCode plugin bridge if needed. The shared part is GAL's canonical source graph; the provider plugin bundle is generated output.

### Management verdict

採用 plugin 對 AGY 是比較好管理的選擇。更廣義地說，plugin-first 也很可能是 GAL 管理 Codex、Copilot CLI、Claude Code 的正確長期方向；只是本計畫的 implementation boundary 只到 AGY。

理由：

- 安裝面更單純：目前 AGY 由 `Update-Skills`、`Update-Commands`、`Update-Mcp`、`Update-Personalization` 分別碰不同 AGY path；plugin-first 後只剩 build bundle + sync `plugins/gal/`。
- 更新面更可預測：重新建置同一個 `gal` plugin，再原子覆蓋 staged plugin，比逐一比對 symlink 與 JSON merge 更容易診斷。
- 卸載面更乾淨：刪掉 GAL-managed `plugins/gal/` 並清理 legacy direct-write residue，即可移除 AGY 端 GAL；不必追多個散落目錄。
- source drift 風險更低：plugin 是 generated artifact，不是新 source root；維護者仍改 `skills/`、`commands/`、`agent/`、`conventions/`、`workflows/`、`mcp.json`。

限制：GAL 不應「整體變成某一家 provider 的 plugin」。GAL 的核心價值是跨 Copilot、AGY、Codex、Claude Code、OpenCode 的文件狀態與工作流連續性；若把 source contracts 塞進 AGY plugin 當新主體，會破壞跨 provider 目標。正確說法是：`GAL source contracts -> provider-specific plugin projection`，本計畫只先落地 `AGY projection`。

### Current AGY direct-write surface

目前 AGY 相關寫入面分散如下，這正是 plugin-first 要收斂的範圍：

| Current surface | Current owner | Plugin-first target |
| --- | --- | --- |
| `~/.gemini/antigravity-cli/skills/<skill>/` | `scripts/Update-Skills.ps1`, `scripts/update-skills.sh` symlink reusable skills | `plugins/gal/skills/<skill>/` generated copy |
| `~/.gemini/antigravity-cli/skills/<command>/` | `scripts/Update-Commands.ps1`, `scripts/update-commands.sh` symlink baked command skills | `plugins/gal/skills/<command>/` generated copy |
| `~/.gemini/antigravity-cli/mcp_config.json` | `scripts/Update-Mcp.ps1`, `scripts/update-mcp.sh` merge GAL-managed MCP into global AGY config | `plugins/gal/mcp_config.json` generated from resolved manifest |
| `~/.gemini/antigravity-cli/gal/` | `scripts/Update-Skills.ps1`, `scripts/update-skills.sh` stable repo symlink | remove as required AGY surface; plugin content must be self-contained |
| repo-local `.agents/*` legacy residue | `scripts/Update-Personalization.ps1`, `scripts/update-personalization.sh` cleanup only | keep cleanup, do not reintroduce as AGY install surface |

## Approach

### Architecture summary

導入一個 AGY-specific generated artifact：`gal-results/agy-plugin/gal/`。`gal-results/` 已是 git-ignored generated output；install/sync 時再把此 tree 覆蓋到 `~/.gemini/antigravity-cli/plugins/gal/`。產物形態對齊 AGY 官方 plugin layout：

- `plugin.json`
- `skills/`
- `agents/`
- `rules/`
- `mcp_config.json`
- `hooks.json`（首版不生成；未來只有出現明確 lifecycle 需求才加入）

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
| `skills/<name>/SKILL.md` | `skills/<name>/SKILL.md` | generated copy；保留 skill 名稱與支援檔；不 symlink 到 source repo |
| `commands/*/SKILL.md` 或 template-baked command skills | `skills/<command-name>/SKILL.md` | 視為 AGY skill bundle；沿用現有 GAL command skill 名稱；build-time 檢查不得與 reusable skill 撞名 |
| `agent/*.agent.md` | `agents/<agent-name>.md` | generated copy；只做 AGY 需要的 filename/frontmatter normalization，不改 source agent |
| `.dev/project.md`、`conventions/`、`workflows/`、`model-roles.md`、skill index | `rules/gal.md` | 產生最小 AGY runtime rule；不把整個 repo 或所有 generated adapters 原樣塞進 plugin |
| `mcp.json` + `mcp.local.json` merge result | `mcp_config.json` | 保留現有 env resolution 與 AGY `serverUrl` conversion，但輸出改由 plugin 持有 |

Build graph 原則：首版只建立 AGY renderer，不做完整跨 runtime plugin framework。共用的是 source inventory 與 projection metadata；AGY layout、manifest 檔名、MCP 檔名、install path 都留在 AGY-specific renderer 內。這讓 Codex 這種高度接近 AGY 的 runtime 未來可以重用大部分 inventory/projection 邏輯，但仍輸出 `.codex-plugin/plugin.json`、`.mcp.json` 等 Codex 自己的形態。

### Step 1: 定義 AGY plugin 作為新安裝邊界

- **Files**: `docs/plans/feat-gal-provider-plugin-packaging.md` only in this planning phase; implementation likely touches `scripts/common/Common.ps1`, `scripts/common/common.sh`, `scripts/Update-Skills.ps1`, `scripts/update-skills.sh`, `scripts/Update-Commands.ps1`, `scripts/update-commands.sh`, `scripts/Update-Mcp.ps1`, `scripts/update-mcp.sh`
- **What**: 把 AGY runtime 的 owner boundary 從「多個 setup 腳本各自管理自己的落點」改成「一個 GAL-managed AGY plugin」。
- **Verify**: 能清楚回答 AGY 相關 source 由誰產生、安裝到哪裡、由誰清理、由誰驗證。

### Step 2: 新增 AGY plugin bundle build graph

- **Files**: `scripts/Build-AgyPlugin.ps1`, `scripts/build-agy-plugin.sh`, possibly `templates/agy-plugin/plugin.json`
- **What**: 建立一個從 source contracts 生成 plugin payload 的流程，至少包含 manifest、skills、agents、rules、MCP projection。首版不生成 `hooks.json`。
- **Verify**: 在不碰 AGY home 目錄的前提下，先在 generated staging path 產出完整 plugin tree，且結構符合 AGY 官方 plugin layout。

### Step 3: 將 command-skill 安裝改為 plugin projection

- **Files**: `scripts/Update-Commands.ps1`, `scripts/update-commands.sh`, command baking helpers
- **What**: 停止把 AGY command skills 當成直接安裝目標；改成先烘焙 command skill，再投影到 plugin `skills/`。若 command skill 名稱與 reusable skill 名稱衝突，build fail，不自動改名。
- **Verify**: AGY 仍能透過 plugin 使用 `/gal*` 相關能力，但 machine setup 不再直接把 command surface 散寫到 AGY 目錄。

### Step 4: 將 reusable skills 安裝改為 plugin projection

- **Files**: `scripts/Update-Skills.ps1`, `scripts/update-skills.sh`
- **What**: 停止將 GAL skill 直接連到 AGY global skills path；改由 plugin payload 的 `skills/` 承接，並在 legacy cleanup 移除舊 AGY direct-write GAL skill links。
- **Verify**: AGY 的 skill inventory 來自 GAL plugin，而非單獨的 GAL skills 連結。

### Step 5: 將 AGY MCP 安裝改為 plugin-owned `mcp_config.json`

- **Files**: `scripts/Update-Mcp.ps1`, `scripts/update-mcp.sh`
- **What**: 保留現有 `mcp.json` / `mcp.local.json` / env merge 邏輯，但輸出改寫到 generated plugin `mcp_config.json`，再由 plugin install/sync 負責部署。AGY 不再長期維護 global `~/.gemini/antigravity-cli/mcp_config.json` 中的 GAL-managed entries。
- **Verify**: GAL-managed MCP 不再依賴 Gemini legacy 設定檔；AGY plugin 單獨持有 MCP 設定。

### Step 6: 收斂 AGY rules 與 runtime instructions

- **Files**: likely `scripts/Update-Personalization.ps1`, `scripts/update-personalization.sh`, sync helpers, possibly new build template files
- **What**: 把目前對 AGY 有意義的 repo-owned instructions 收斂成 plugin `rules/gal.md`，但不重新把 repo `.agents/` 當成安裝面。`rules/gal.md` 應只承載 AGY 啟動 GAL 所需的 control-plane、memory、workflow、skill discovery 規則，避免整包 adapter 重複灌入。
- **Verify**: AGY 有足夠 rules/context 可載入，且 repo 仍然不會被 setup 腳本新增 `.agents`。

### Step 7: 將 machine setup 改為 plugin install or sync lifecycle

- **Files**: `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh`, shared common helpers
- **What**: machine setup 對 AGY 的職責改為：

  1. 建置 plugin bundle
  2. 安裝或覆蓋 `~/.gemini/antigravity-cli/plugins/gal/`
  3. 更新 GAL managed install-state
  4. 清理舊的 AGY direct-write legacy surfaces：`skills/<gal-managed>/`、AGY global MCP managed entries、`gal/` repo link

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
- `scripts/Build-AgyPlugin.ps1` - 建置 git-ignored AGY plugin staging tree。
- `scripts/build-agy-plugin.sh` - 與 PowerShell 對齊的 Bash build helper。
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

- AGY plugin build helper files under `scripts/`.
- Optional AGY plugin manifest template under `templates/agy-plugin/` if generated manifest logic would otherwise duplicate JSON construction in two scripts.
- Generated AGY plugin staging tree under `gal-results/agy-plugin/gal/`.
- `plugin.json` template or generated manifest for AGY plugin.
- generated `rules/` payload for AGY plugin.

### Explicitly out of scope for this plan

- Copilot plugin migration in this implementation phase
- Claude Code plugin migration in this implementation phase
- Codex plugin migration in this implementation phase
- OpenCode plugin migration in this implementation phase
- repo-local `.agents` reintroduction for AGY in this repo

## Test Cases

- [ ] Bundle structure check: generated AGY plugin tree contains `plugin.json` and any generated `skills/`, `agents/`, `rules/`, `mcp_config.json` expected for the current repo state.
- [ ] Self-contained bundle check: generated AGY plugin payload does not contain symlinks or references that require `~/.gemini/antigravity-cli/gal/` to resolve normal skills, agents, rules, or MCP config.
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
- 若 plugin payload 仍靠 `~/.gemini/antigravity-cli/gal/` 回連 repo，plugin 會只是舊 direct-write 的包裝紙，不是真正可管理的 bundle。

## References

- [OpenAI Codex Build plugins](https://developers.openai.com/codex/plugins/build)
- [Claude Code Plugins reference](https://code.claude.com/docs/en/plugins-reference)
- [GitHub Copilot CLI Creating a plugin](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/plugins-creating)
- [OpenCode Plugins](https://opencode.ai/docs/plugins/)
- [Antigravity CLI Features](https://antigravity.google/docs/cli-features)

## Open Questions

- [x] OQ-001 - GAL AGY plugin 的固定名稱與版本策略要採用什麼格式，才能同時支援簡單覆蓋更新與未來可能的版本診斷？ *(raised by: planning, resolved by: deep-planning)* 固定 plugin folder/name 為 `gal`，manifest display name 為 `Golem Agents Legion`。`plugin.json` 寫入 build-time version diagnostic：優先用 repo tag 或 `git describe --tags --always --dirty`，無 git 時 fallback 到 `0.0.0-local`。安裝永遠覆蓋 `~/.gemini/antigravity-cli/plugins/gal/`，版本先用於診斷與未來 upgrade，不讓版本化阻礙本機覆蓋更新。
- [x] OQ-002 - `rules/` 應該由哪些 source contracts 組成最小可行集合，才能讓 AGY 有足夠上下文但不把整個 repo instruction surface 無差別灌入？ *(raised by: planning, resolved by: deep-planning)* 首版只生成 `rules/gal.md`，來源為 `.dev/project.md` 的 project/constraints/protected-path summary、`conventions/conventions.md` index、`conventions/token-budget.md` 的 cold-start/memory rules、`workflows/coding.md` 的 GAL lifecycle、`model-roles.md` 的 role separation、以及 generated skill/agent/command index。不要把完整 `AGENTS.md`、`CLAUDE.md`、`GEMINI.md` 或所有 skill body 直接塞入 rules。
- [x] OQ-003 - AGY plugin 首版是否完全不生成 `hooks.json`，還是需要最小 lifecycle hook 來做 setup/self-check？ *(raised by: planning, resolved by: deep-planning)* 首版完全不生成 `hooks.json`。setup/self-check 由 `Setup-Machine` 和 explicit verification tests 負責；hooks 等到有明確事件需求與安全 review 後再加入。
- [x] OQ-004 - command-skill projection 命名要如何避免與 reusable skill 名稱衝突，同時保留 `/gal ...` 的心智模型？ *(raised by: planning, resolved by: deep-planning)* 沿用現有 command skill directory names，例如 `gal`、`gal-status`、`planning`、`deep-planning`。build 階段檢查 `commands/*` 與 `skills/*` 名稱集合；若撞名就 fail 並要求 source rename，不自動加 prefix。AGY plugin namespace 由 `gal` plugin 承擔；GAL control-plane 心智模型仍由 `gal`/`gal-*` command skill 內容維持。

## Approval

- Human approval: [pending]
- Architect review: [clear]
- Additional domain review: [not triggered]

## Review Results

### Architecture Review

Verdict: APPROVE after deep-planning revisions.

Trade-off summary:

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| Make AGY plugin-first only | Collapses AGY install/update/uninstall into one managed boundary | Adds AGY build renderer and legacy migration work | OK |
| Keep GAL source contracts unchanged | Preserves cross-provider GAL purpose from README | Plugin build must copy/project content instead of using live repo symlinks | OK |
| Generate `plugins/gal/` as self-contained payload | Makes reinstall and uninstall deterministic | Staged plugin may duplicate skill/agent text already in repo | OK |
| Do not generate hooks in v1 | Avoids high-permission lifecycle automation before need is proven | No automatic plugin self-healing on AGY session start | OK |
| Use provider-specific renderers, starting with AGY | Avoids forcing AGY, Codex, Copilot CLI, Claude Code, and OpenCode into one incorrect on-disk schema | Requires later renderers for Codex/Copilot/Claude/OpenCode instead of one universal folder | OK |

Over-engineering flags:

- **OE-01** A generic cross-runtime plugin framework would be premature. Build only `Build-AgyPlugin` now, but keep source inventory/projection metadata provider-neutral enough that future Codex, Copilot CLI, Claude Code, and OpenCode renderers do not need to rediscover the GAL source graph.

Bug surface:

- **BUG-01** Legacy residue can shadow plugin payload if old `~/.gemini/antigravity-cli/skills/<gal-managed>/` or global `mcp_config.json` entries remain. Fix: setup must clean only GAL-managed legacy direct-write artifacts and preserve user-owned entries.
- **BUG-02** Symlinked plugin payload would keep the old install coupling. Fix: generated AGY plugin must be self-contained copies plus generated JSON/rules; no required `~/.gemini/antigravity-cli/gal/` repo link.
- **BUG-03** Command/reusable skill name collision could make AGY load the wrong component. Fix: fail the build on name collision instead of silently renaming.

Missing from plan before this pass:

- The plan did not explicitly answer whether plugin was better for management across providers. It is likely better as a long-term provider install strategy for AGY, Codex, Copilot CLI, and Claude Code, but each needs its own renderer and none should replace GAL's cross-runtime source model.
- Open questions were still unresolved, which would block implementation decisions around name, rules, hooks, and command projection.
- The current AGY direct-write surfaces were not listed concretely enough for legacy cleanup.

What's good:

- The plan correctly isolates the first implementation to AGY without denying future provider-specific plugin renderers.
- The plan correctly treats `mcp.json` plus `mcp.local.json` as source and AGY `mcp_config.json` as projection.
- The plan correctly refuses repo-local `.agents` reintroduction for this repo.

### Business Review

Not triggered. This plan changes installation topology, not pricing, permissions, onboarding, eligibility, or customer-visible business rules.

### Design Review

Not triggered. This plan has no customer-facing UI, layout, visual states, or accessibility surface.

### Engineering Review

Pending `/refining-plan`.

## Test Plan

Pending `/refining-plan`. Seed cases are listed in `## Test Cases`.

## Tasks

Pending `/refining-plan`.