# 計畫：GAL 安裝模式與 Companion Plugin 發布

## 目標

GAL 提供一套以 provider-native plugin install、catalog 與 lockfile 為核心的安裝模式。規劃基準改為：以 Claude plugin data struct 作為最完整的 canonical schema，由 GAL 先產出 Claude-compatible 完整 plugin package，並將 install mode 的 machine-local config、resolved state、plugin store、generated config 與 provider projections 統一收斂到 `~/.gal/`。其他 AI tools 只透過 `~/.gal/active/<provider>/` 此類 GAL-managed stable targets 取得 shortcut 或掛載。一般使用者不需要 clone GAL repo，也不需要建立指向 repo root 的 shortcut 才能使用 GAL。現有 clone repo 加 symlink 的方式保留為 source mode / contributor mode，不再是一般使用者的預設路徑。

本計畫的核心定位是：GAL 是一個 curated plugin catalog 與 lockfile orchestrator，不是跨 runtime 的萬用 plugin runtime，也不是把所有外部 skills 重新打包成 GAL 自有外掛的發布系統。

- `gal-core`：GAL 控制平面、golem agents、核心工作流、必要 conventions，以及少量真正屬於 GAL 本身的 skills。
- companion plugins：依語言或領域拆分、由外部 upstream 維護的 provider-native plugins，例如 [dart-lang/skills](https://github.com/dart-lang/skills)、[flutter/skills](https://github.com/flutter/skills)、[dotnet/skills](https://github.com/dotnet/skills)。GAL 只負責 catalog、相容性記錄、預設安裝集合、版本鎖定與 lifecycle orchestration，不把這些外部來源重新收編成 GAL 自有語言外掛。
- upstream 或 mirrored plugins：除預設 companion set 之外，GAL 可選擇管理其他外部 skills/plugin，但必須透過 catalog、lockfile、來源版本、授權與 checksum 治理，而不是無版本地直接複製進 core repo payload。

使用者承諾：install mode 是給想直接使用 GAL 的一般使用者；它提供可重現的 plugin set、安全 update/uninstall、沒有 machine-local source path，並以 Claude-compatible package 為基準，把 machine-local state 統一放進 `~/.gal/`，再視 provider 需求從 `~/.gal/active/<provider>/` 掛載。source mode 是給 GAL 貢獻者；它允許本機 repo 即時掛載與 local override。migration cleanup 只服務既有 GAL 安裝，不應成為新使用者的成功路徑。

## 需求

- [ ] 比較並明確記錄所有相關 platform 的 plugin 支援能力與安裝模型：AGY CLI、Copilot CLI、Codex、Claude Code，以及 runtime support boundary 內的 Gemini migration lane 與 OpenCode bridge lane。
- [ ] 將 primary provider-native install targets 鎖定為 AGY CLI、Copilot CLI、Codex 與 Claude Code；Gemini 僅作 migration/compatibility lane，OpenCode 僅作 bridge/degraded lane，不納入四 provider 靜態 renderer substrate。
- [ ] 將 GAL 定義為 catalog + lockfile orchestrator：`catalog -> lockfile -> resolved plugin set -> provider-native install spec`。
- [ ] 明確採用 Claude plugin data struct 作為 canonical schema 與 renderer baseline，其他 provider 透過 schema mapping 或 `~/.gal/active/<provider>/` capability-level shortcut 掛載取得可用子集。
- [ ] 定義 `gal-core` 與外部 companion plugins 的邊界，避免把所有 skills 都永遠綁進單一 `gal` 外掛程式。
- [ ] 初始 default profile 只能安裝 `gal-core`；所有 companion plugins 必須透過明確 profile 或 explicit plugin selection opt in。
- [ ] install mode 不得在單一 provider 通過後立即成為一般使用者預設；至少需 Claude-compatible canonical package、AGY baseline regression 與 Copilot native install smoke 均通過。
- [ ] 外部 provider-native companion plugins 目前只以已確認存在的八個 upstream skills repos 為準：`dart-lang/skills`、`flutter/skills`、`dotnet/skills`、`anthropics/skills`、`samber/cc-skills-golang`、`twostraws/swift-agent-skills`、`kepano/obsidian-skills`、`actionbook/rust-skills`；其餘如 game asset、Godot 等 skills 先視為 GAL-owned / internal，除非後續另行確認對應 upstream。
- [ ] 定義 plugin support tiers：`official-gal`、`curated-upstream`、`mirrored`、`forked`、`local`，並明確記錄內容維護者、相容性責任、更新責任與自動更新政策。
- [ ] 定義 GAL-managed `plugins/catalog.json`，追蹤 `gal-core` 與外部 companion/upstream plugins 的來源、版本、授權、checksum、支援 provider、component map、install strategy、default profiles、local override 以及是否 vendor/mirror。
- [ ] 定義 `~/.gal/state/plugins.lock.json`，解決從 GitHub 複製 skills 後長期漂移、無法確認版本、無法安全更新的問題。
- [ ] install mode 以 Claude-compatible package 與 provider-native install 為優先；若其他 provider 缺少完整對等支援，允許使用 `~/.gal/active/<provider>/` 此類 GAL-managed stable target 作為 shortcut 轉址目標，但 repo-root shortcut、baked source checkout path、`GAL_ROOT` link 仍不得成為 install mode 成功條件。
- [ ] capability-level links 可出現在 install mode、source mode、local override、migration cleanup 或明確標示的 degraded/bridge lane，但 install mode 中只允許指向 `~/.gal/active/<provider>/` 或其對應的 GAL-managed projection，不得指向 repo root，且必須在 dry run output 中可見。
- [ ] install mode 的 machine-local config、resolved state、plugin store、generated MCP/xmachine/provider outputs 應統一集中於 `~/.gal/`，其中個人化設定集中於 `~/.gal/config/config.json`、xmachine routing 集中於 `~/.gal/config/xmachine.json`、MCP projection 集中於 `~/.gal/generated/mcp/managed.json`，並由 GAL 明確管理 ownership 與 uninstall 邊界。
- [ ] 當既有 `config.local.env` 遷移為 `~/.gal/config/config.json` 時，原本的 env keys 需重新分流：`GAL_SKILLS` 應移除、`MCP_MEMORY_FILE_PATH` 保留為 machine-local config、`MCP_FILESYSTEM_PATHS` 降為僅在 filesystem MCP 仍存在時才輸出的 optional compatibility field、`CONTEXT7_API_KEY` 可在 render `~/.gal/generated/mcp/managed.json` 時直接 materialize 成最終 header。
- [ ] 保留 source mode 作為 GAL 開發者流程，明確標示其 `GAL_ROOT` shortcut 只屬於本機開發、產生、local override 與相容橋接。
- [ ] 將 `~/.copilot/gal`、`~/.gemini/gal`、`~/.gemini/antigravity-cli/gal` 視為 source mode 或 legacy migration bridge，不得出現在 install mode 的成功條件內。
- [ ] 將 `gal-core` source contracts 先轉成 Claude-compatible canonical plugin artifact，再對應為各 provider 可安裝或可 shortcut 掛載的 artifacts；產生物不得依賴原始 repo 的絕對路徑。
- [ ] 保持 provider-specific renderer 邊界：Claude artifact 作為 canonical schema，其他 provider 只輸出該 provider 文件支援的 manifest 欄位與元件，或掛載其可使用之子集。
- [ ] Copilot CLI 目前只承諾官方文件已記錄的 plugin components 與 lifecycle：`agents`、`skills`、`commands`、`hooks`、`mcpServers`、`lspServers`、marketplace、GitHub/Git URL/local path install；不承諾 plugin-local `bin` 或一般 script runtime execution。
- [ ] 不把 `scripts/` 提升為四 provider 共同支援。需要可執行能力時，使用 provider 原生能力、skill-local helper、hooks、`bin/`，或獨立的 GAL runtime install，而不是 provider-neutral `scripts/` 欄位。
- [ ] GAL-managed MCP 預設由 plugin artifact 或 provider-native plugin MCP 機制承載；local secrets 與 machine-local paths 只在本機 install/enable 階段解析。
- [ ] companion plugin 與 upstream plugin 的安裝、更新、解除安裝必須可重跑且可驗證，不得刪除使用者自有 provider 設定。
- [ ] 文件必須清楚區分 install mode、source mode、migration cleanup、core plugin、companion plugin、upstream plugin、support tier、bridge lane 與 provider-native limitations。

## 參考文件

- [GitHub Copilot CLI plugin docs](https://docs.github.com/en/copilot/concepts/agents/copilot-cli/about-cli-plugins)
- [Claude Code plugin docs](https://code.claude.com/docs/en/plugins)
- [Codex plugin docs](https://developers.openai.com/codex/plugins/build)
- [Antigravity CLI docs](https://antigravity.google/docs/cli-features)
- [OpenCode plugin docs](https://opencode.ai/docs/en/plugins/)

## Non-Goals

- 不建立萬用 plugin runtime。
- 不以 repo-root shortcut 冒充 canonical artifact；Claude baseline 只作 schema 與 package 基準，不代表所有 provider 都必須原生支援完整能力。
- 不把 OpenCode 做成第五個 primary renderer。
- 不把 install-mode symlink fallback 視為成功條件。
- 不建立 provider-neutral `scripts/`、`runtimeScripts` 或隱含 plugin-root shell API。
- 不在 catalog governance 完成前大量刪除 repo 內既有 copied skills。

## Approach

### 與既有 provider packaging 主線的銜接

本計畫建立在 [feat-gal-provider-plugin-packaging.md](feat-gal-provider-plugin-packaging.md) 已完成的 AGY plugin-only 基底上。根據 `.dev/state.md`，該計畫已完成 `T-006`、`T-007`、`T-008`、`T-009`，因此本計畫不再等待該前置作業，而是在實作前加入 baseline regression guard：確認 AGY plugin-only lifecycle、plugin-root MCP、`rules/gal.md` 與 legacy cleanup 仍然穩定。

既有 provider-neutral package model 不再是整個多 plugin 生態的架構中心。它應被縮小為 `gal-core` renderer input；新的中心抽象是以 Claude plugin data struct 為基準的 catalog resolver：

```text
plugins/catalog.json
  -> ~/.gal/state/plugins.lock.json
  -> resolved plugin set
  -> provider-native install spec
  -> Claude-compatible canonical package
  -> provider-specific installer / renderer / shortcut mapping
```

### 平台與 runtime lane policy


| Lane | Platform | 本計畫角色 | 成功條件 |
| --- | --- | --- | --- |
| Primary install target (canonical) | Claude Code | canonical schema / canonical renderer / installer target | `claude plugin install`、scope、update、uninstall 等 provider-native lifecycle |
| Primary install target (near-parity) | Copilot CLI | Claude-compatible structure 可幾乎原樣使用；`agents/`、`skills/`、`hooks.json`、`.mcp.json`、`lsp.json` 與 Claude 共用相同目錄慣例 | `/plugin install`、marketplace、GitHub/Git URL/local path；directory layout 已與 Claude 對齊，不需 shortcut |
| Primary install target | Codex | Claude baseline 的 mapped renderer / marketplace target | `codex plugin install`、documented marketplace / cache install path，部分元件可原生讀取 |
| Primary install target (shortcut) | AGY CLI / Antigravity CLI | Claude baseline 的 mapped renderer + catalog-aware install target | provider-native plugin staging / install，或指向 `~/.gal/active/agy/` 的 capability shortcut；不依賴 repo-root shortcut |
| Migration lane | Gemini CLI | legacy cleanup and compatibility only | 不作第五 renderer；只清理或遷移既有 GAL-managed Gemini surface |
| Bridge lane | OpenCode | 後續獨立 bridge plan | 可讀取 catalog/lockfile，掛載 `~/.gal/active/opencode/` capability shortcut，但不承諾 primary install parity |
| Deferred / unsupported | 其他 runtime | 不在本計畫內 | 缺少 documented install 或 skill discovery 時不納入 |

### Shortcut 與 degraded bridge policy

採納 `Architectural-Analysis.md` v2 (2026-05-22) 提出的 Golden Standard + selective shortcut 策略，根據跨平台驗證結果，Copilot CLI 現在與 Claude Code 共用幾乎相同的 plugin 目錄結構（`skills/`、`agents/`、`hooks.json`、`.mcp.json`），因此**只有 AGY CLI 與 OpenCode 需要 capability-level shortcut**。政策如下：

- Claude-compatible canonical package 是單一真實來源；Copilot CLI 可幾乎原樣使用相同 plugin structure（`skills/`、`agents/*.agent.md`、`hooks.json`、`.mcp.json`）。
- Claude Code、Copilot CLI 與 Codex 使用各自的 provider-native `plugin install` 機制安裝 canonical package，**不需要 shortcut**。
- AGY CLI 與 OpenCode 因缺乏 native `plugin install` CLI，允許使用 `~/.gal/active/<provider>/` 作為 GAL-managed stable target 做細粒度 shortcut 轉址。
- Install mode 禁止 repo-root shortcut、baked source path 與隱藏 `GAL_ROOT` 依賴。
- Capability-level links 可用於 install mode、source mode、local override、migration cleanup 或明確標示的 degraded/bridge lane。
- Install mode 中的 capability-level links 必須只指向 `~/.gal/active/<provider>/` 或其對應的 GAL-managed projection；source mode 才能指向使用者明確指定的 local override。
- Capability-level links 必須在 dry run output、install-state 與 uninstall plan 中可見。
- Bridge lane 不得讓使用者誤以為該 runtime 已達 primary provider-native install parity。

### Plugin support tiers

| Tier | 內容來源 | GAL 責任 | 更新策略 |
| --- | --- | --- | --- |
| `official-gal` | GAL repo / GAL release artifact | GAL 維護內容、測試、安裝與迴歸 | 可由 GAL release 直接更新 |
| `curated-upstream` | 外部 upstream plugin，例如 [dart-lang/skills](https://github.com/dart-lang/skills) | GAL 驗證 metadata、provider compatibility、預設 profile 與 lockfile；內容由 upstream 維護 | 依 lockfile pin 更新；允許手動或受控更新 |
| `mirrored` | 外部 upstream 的受管 mirror | GAL 負責 provenance、license、checksum 與 mirror drift | 不允許無版本複製；更新需 drift check |
| `forked` | GAL 或使用者維護的 fork | fork owner 負責 divergence 與修復 | 必須記錄 fork base、diff policy 與更新策略 |
| `local` | `file://` 或本機 path override | 只用於 source mode / contributor override | 不進入可分享 lockfile，或以 machine-local override 記錄 |

### 初始 companion candidates

目前確認可列為 external companion / upstream plugin skills 的範圍，僅限本計畫已記錄並確認的八個 repos。其餘像 game asset、Godot、GStack framework-style assets 等，不在本計畫的 external plugin skills 清單內，預設保留在 `gal-core` 或 GAL-owned internal skill inventory。

| Lane | 初始 upstream candidates | Catalog policy |
| --- | --- | --- |
| Dart | [dart-lang/skills](https://github.com/dart-lang/skills) | `curated-upstream` candidate；需記錄 ref、license、component map 與 provider 支援 |
| Flutter | [flutter/skills](https://github.com/flutter/skills) | `curated-upstream` candidate；需驗證 Flutter-specific skills 是否可跨 provider 使用 |
| .NET | [dotnet/skills](https://github.com/dotnet/skills) | `curated-upstream` candidate；需盤點 C# / MSBuild / NuGet guidance 與 GAL conventions 的重疊 |
| Claude ecosystem | [anthropics/skills](https://github.com/anthropics/skills) | `curated-upstream` candidate；需盤點 Claude 官方 skills 與 `gal-core` 的重疊與覆寫政策 |
| Go | [samber/cc-skills-golang](https://github.com/samber/cc-skills-golang) | `curated-upstream` candidate；需記錄 provider 支援、license 與 drift policy |
| Swift | [twostraws/swift-agent-skills](https://github.com/twostraws/swift-agent-skills) | `curated-upstream` candidate；需記錄 provider 支援、license 與 drift policy |
| Obsidian | [kepano/obsidian-skills](https://github.com/kepano/obsidian-skills) | `curated-upstream` candidate；需盤點與現有 Obsidian workflows 的重疊與覆寫政策 |
| Rust | [actionbook/rust-skills](https://github.com/actionbook/rust-skills) | `curated-upstream` candidate；需記錄 provider 支援、license 與 drift policy |

### Default profile 與預設啟用政策

初始 `default` profile 只安裝 `gal-core`。八個 curated-upstream companion candidates 全部保持 opt-in，透過 named profiles 或 explicit plugin selection 啟用，例如 Dart、Flutter、.NET、Go、Swift、Rust、Obsidian 或 Claude ecosystem profile。這降低首次安裝的授權、支援與漂移風險，也避免使用者誤以為 GAL 對所有外部 skill 內容負完整維護責任。

Install mode 在本計畫完成前保持 opt-in。一般使用者預設只有在下列三個 guard 全部通過後才可切換：Claude-compatible canonical package 可穩定 build/install、AGY plugin-only baseline regression 通過、Copilot native install smoke 通過。這讓預設切換同時覆蓋 canonical package、shortcut lane 與 near-parity native lane，而不是把單一 provider 成功誤當成整體安裝模型成熟。

### Catalog 與 lockfile schema

為了讓使用者能穩定管理「想裝哪些 plugins」、方便轉移/備份/更新，資料面應拆成 repo-owned catalog source 與 `~/.gal/` machine-local runtime home，而不是只靠 scattered configs：

- `plugins/catalog.json`：repo 內的 GAL 維護 catalog source 與 metadata registry。
- `~/.gal/config/config.json`：user-managed machine config，集中記錄個人化設定、plugin/profile/provider selections、install/source mode、`galRoot`、`devMode` 與其他 machine-local GAL preferences。預設採 JSON，因為現有 provider plugin manifests 與 marketplace manifests 都是 JSON。
- `~/.gal/state/plugins.lock.json`：resolver 依據 `~/.gal/config/config.json` 與 `plugins/catalog.json` 產生的 resolved lockfile，記錄實際安裝來源、版本、checksum 與 component map。
- `~/.gal/config/xmachine.json`：machine-local xmachine binding file，集中記錄 xmachine node aliases、default node、machine profiles、local plugin paths、provider path overrides 與其他 routing/binding 設定。
- `~/.gal/store/plugins/`：Claude-compatible canonical packages 與 companion plugins 的受管 store。
- `~/.gal/generated/mcp/managed.json`：GAL 產生並擁有的 MCP projection，取代 install mode 下 repo-root `mcp.local.json` 的角色，並可直接提供 render 後的 `CONTEXT7_API_KEY` header 與其他 machine-local resolved MCP values。
- `~/.gal/generated/xmachine/managed.json`：GAL 產生並擁有的 xmachine projection。
- `~/.gal/generated/providers/`：GAL 產生的 provider config projections。
- `~/.gal/active/<provider>/`：提供各 AI tools 使用的 stable shortcut targets；consumer 不直接指向 store path。

`galRoot` 與 `devMode` 屬於個人化 machine config，應集中記錄在 `~/.gal/config/config.json`；xmachine routing、node alias 與其他 machine bindings 則集中記錄在 `~/.gal/config/xmachine.json`。兩者都不寫入 `~/.gal/state/plugins.lock.json`。

既有 `config.local.env` 遷移到 `~/.gal/config/config.json` 時，設定項目的處理原則如下：

- Obsidian routing 類設定保留於 `config.json`，例如 `obsidianVault`、`obsidianVaultName`、`obsidianGuidePath`、`obsidianGuideMode`、`obsidianPrivateResearchDir`、`obsidianDiaryDir`、`obsidianScratchDir`、`obsidianArchiveDir`、`researchDefaultDest`。
- 本機工具與工作流設定保留於 `config.json`，例如 `localSearchProject`、`tempDir`、working-hours 相關欄位。
- `GAL_SKILLS` 不再保留，因為 install mode 與新的 `~/.gal` runtime home 不再依賴共享 skills 安裝路徑。
- `mcpMemoryFilePath` 保留於 `config.json`，因為現有 memory MCP server 仍需要 machine-local 持久化檔案路徑。
- `mcpFilesystemPaths` 不作為必填欄位；只有在 filesystem MCP server 仍存在於 resolved MCP projection 時才作為 optional compatibility field 輸出。
- `context7ApiKey` 屬於 machine-local secret，可作為 `config.json` 的輸入，並在 render `~/.gal/generated/mcp/managed.json` 時直接 materialize 成最終 header；不再要求 runtime 於執行時二次讀取 env placeholder。

`plugins/catalog.json` 至少應支援：

- `pluginId`
- `displayName`
- `supportTier`
- `sourceType`：`official-gal`、`curated-upstream`、`mirrored`、`forked`、`local`
- `upstream`：repo、path、release tag 或 commit ref
- `license`
- `checksum` 或 commit SHA policy
- `componentMap`：skills、agents、commands、MCP、hooks、bin、docs 等可用元件
- `supportedProviders`
- `installStrategy`
- `defaultProfiles`
- `allowAutoUpdate`
- `localOverridePolicy`

`~/.gal/state/plugins.lock.json` 至少應支援：

- resolved `pluginId`
- resolved source URL/repo/path/ref
- resolved version、release tag 或 commit SHA
- resolved checksum
- resolved license
- resolved component map
- selected providers and profiles
- lock timestamp and resolver version
- drift detection metadata

`~/.gal/config/config.json` 至少應支援：

- `schemaVersion`
- `galRoot`
- `devMode`
- `obsidianVault`
- `obsidianVaultName`
- `obsidianGuidePath`
- `obsidianGuideMode`
- `obsidianPrivateResearchDir`
- `obsidianDiaryDir`
- `obsidianScratchDir`
- `obsidianArchiveDir`
- `researchDefaultDest`
- `localSearchProject`
- `tempDir`
- `mcpMemoryFilePath`
- `mcpFilesystemPaths` (optional compatibility field)
- `context7ApiKey`
- `workingHoursEnabled`
- `workdayStart`
- `workdayEnd`
- `wrapUpTime`
- `hardStopTime`
- `defaultProfile`
- `profiles`
- `enabledPlugins`
- `disabledPlugins`
- `providerSelections`
- `installMode`：`install` 或 `source`
- `updateChannel` 或 `allowAutoUpdate`
- `preferredProviders`
- `userSettings`

`~/.gal/config/xmachine.json` 至少應支援：

- `schemaVersion`
- `defaultXmachineNode`
- `xmachineNodeAliases`
- `machineProfiles`
- `localPluginPaths`
- `providerPathOverrides`
- `additionalBindings`

### Source mode 與 install mode contract

| Mode | 目標對象 | `gal-core` 來源 | 外部 plugins 來源 | Shortcut policy |
| --- | --- | --- | --- | --- |
| Install mode | 一般使用者 | Claude-compatible canonical package + provider-native install source | `~/.gal/config/config.json` + `~/.gal/state/plugins.lock.json` resolved upstream package | 允許 provider-specific shortcut，但只能指向 `~/.gal/active/<provider>/`；禁止 repo-root shortcut |
| Source mode | GAL contributor | `~/.gal/config/config.json.galRoot` 指定的本機 GAL repo | `~/.gal/state/plugins.lock.json` resolved cache，或 `~/.gal/config/xmachine.json` 指定的 explicit local override | 可使用 repo link，但必須標示為 source mode |
| Migration cleanup | 既有 GAL 使用者 | 既有 managed surfaces | 既有 managed external skills | 只移除 GAL-managed legacy link/cache，不刪使用者自有設定 |
| Bridge/degraded lane | OpenCode 或缺少 primary install parity 的 runtime | GAL-managed cache/artifact | resolved package subset | 只允許 capability-level link；不視為 primary install 成功 |

補充原則：

- `~/.gal/config/config.json` 是可備份、可搬移的「我的 GAL 個人化設定與我想裝什麼」檔案。
- `~/.gal/state/plugins.lock.json` 是可重現的「實際解到什麼版本」檔案。
- `~/.gal/config/xmachine.json` 是 machine-local 的「我本機如何做 xmachine routing / local override / binding」檔案，不應作為團隊共享設定。
- 若使用者只想在單機穩定地切換 install/source mode，`galRoot` 與 `devMode` 由 `~/.gal/config/config.json` 控制；xmachine routing 則由 `~/.gal/config/xmachine.json` 控制。
- `GAL_SKILLS` 不再屬於新 config surface；若舊安裝仍存在該值，只能視為 migration input，不應再出現在最終 `config.json` schema。
- `context7ApiKey` 一旦被 render 進 `~/.gal/generated/mcp/managed.json`，該 generated file 就必須被視為 machine-local secret-bearing artifact，不得進入 tracked repo 或可分享 lockfile。

### 實作順序

1. 驗證 AGY provider packaging baseline 仍然穩定，並把 A-001 從等待前置改成 regression guard。
2. 盤點現有 `skills/`，判定哪些屬於 `gal-core`，哪些應轉為 external companion/upstream catalog entries。
3. 定義 `plugins/catalog.json` schema、`~/.gal/config/config.json` schema、support tiers、component map 與 catalog validation。
4. 定義 `~/.gal/state/plugins.lock.json` schema、`~/.gal/config/xmachine.json` machine-local binding schema、`~/.gal/generated/mcp/managed.json` projection schema、resolver output 與 drift detection。
5. 先定義 Claude-compatible canonical package schema，作為 `gal-core` 與 companion plugins 的完整 data struct 基準。
6. 讓 provider renderers 接受 resolved plugin set，並保留 upstream package identity；其他 provider 若缺少完整支援則從 `~/.gal/active/<provider>/` 掛載可使用之子集。
7. 更新 `Setup-Machine.*` 支援 install mode、source mode、profile selection、explicit plugin selection、`~/.gal/` runtime home、`~/.gal/active/<provider>/` shortcut layout、`~/.gal/config/config.json` machine config 與 `~/.gal/config/xmachine.json` machine binding。
8. 移除 install mode 對 `{{GAL_ROOT}}`、source checkout absolute path 與 provider `gal` repo-root shortcut 的依賴，但保留對 `~/.gal/active/<provider>/` 的 GAL-managed shortcut mapping。
9. 在 catalog governance 與 migration UX 清楚後，才移除或搬遷 repo 內 copied external skills。
10. 逐 provider 實作 native install/update/uninstall，順序仍為 AGY baseline、Copilot CLI、Codex、Claude Code；OpenCode 另案。

## Files to Create or Modify

- `docs/devguide.md` - 平台矩陣、runtime lane policy、catalog resolver、support tiers、source/install mode contract。
- `docs/personalization.md` - 使用者導向的 install mode / source mode / companion plugins / support tier 指南。
- `docs/personalization.zh-Hant.md` - 繁體中文版指南。
- `scripts/scripts.md` - setup / install / update / uninstall / catalog sync / resolver 職責更新。
- `plugins/catalog.json` - `gal-core`、external companion、mirrored、forked、local plugins 的 catalog。
- `~/.gal/config/config.json` - user-managed machine config，記錄個人化設定、要啟用的 plugins、profiles、providers、`galRoot` 與 `devMode`。
- `~/.gal/state/plugins.lock.json` - 已解析來源、版本、checksum、provider 支援、license、profiles 的 lockfile。
- `~/.gal/config/xmachine.json` - machine-local xmachine binding file，記錄 xmachine node aliases、machine profiles、local overrides、provider path overrides 與額外 routing bindings。
- `~/.gal/store/plugins/` - Claude-compatible canonical packages 與 companion plugins 的 GAL-managed store。
- `~/.gal/generated/mcp/managed.json` - 由 GAL render 的 MCP projection，包含 render 後的 `mcpMemoryFilePath`、optional `mcpFilesystemPaths` 與可直接 materialize 的 `context7ApiKey` header。
- `~/.gal/generated/xmachine/managed.json` - 由 GAL render 的 xmachine projection。
- `~/.gal/active/<provider>/` - 各 AI tools 的 stable shortcut targets。
- `scripts/common/ProviderPlugin.ps1` - 彙整為 `gal-core` package builder，並接受 resolver output，而不是無條件包整個 repo `skills/`。
- `scripts/common/provider-plugin.sh` - Bash 對應。
- `scripts/Build-ProviderPlugins.ps1` - 先建置 Claude-compatible canonical package，再建置多 provider、多 resolved plugin artifacts / shortcut layouts 的 PowerShell 入口。
- `scripts/build-provider-plugins.sh` - Bash 對應入口。
- `scripts/Install-GalPlugins.ps1` - install mode provider-native plugin installer / updater / uninstall orchestrator，含 `~/.gal/` runtime home 與 `~/.gal/active/<provider>/` shortcut mapping。
- `scripts/install-gal-plugins.sh` - Bash 對應 installer。
- `scripts/Setup-Machine.ps1` - 支援 install mode / source mode / plugin set selection / local override 的編排。
- `scripts/setup-machine.sh` - Bash 對應的模式編排。
- `scripts/Uninstall-Machine.ps1` - 解除安裝 GAL-managed plugins / cache / staged artifacts 與 opt-in legacy source mode links。
- `scripts/uninstall-machine.sh` - Bash 對應的解除安裝。
- `scripts/Update-Commands.ps1` - 停止讓 install mode 指令技能依賴 baked source checkout path。
- `scripts/update-commands.sh` - Bash 對應。
- `scripts/Update-Mcp.ps1` - plugin-aware MCP ownership 與舊版清理邊界。
- `scripts/update-mcp.sh` - Bash 對應。

## Test Cases

- [ ] 平台矩陣清楚區分 AGY CLI、Copilot CLI、Codex、Claude Code、Gemini migration lane 與 OpenCode bridge lane。
- [ ] `plugins/catalog.json` 可列出 `gal-core`、external companion plugins、mirrored plugins、forked plugins 與 local override entries，且每筆都有 support tier。
- [ ] 初始 `default` profile 只解析並安裝 `gal-core`；八個 curated-upstream companion candidates 只有在 named profile 或 explicit selection 中才會進入 resolved plugin set。
- [ ] install mode default guard 在 Claude-compatible canonical package、AGY baseline regression 或 Copilot native install smoke 任一未通過時維持 opt-in，不把 install mode 設為一般使用者預設。
- [ ] `~/.gal/config/config.json` 可表達 user 想安裝哪些 plugins / profiles / providers，並集中記錄個人化設定、`galRoot` 與 `devMode`。
- [ ] `~/.gal/config/config.json` schema 不再包含 `GAL_SKILLS`；既有安裝若仍提供該值，只能在 migration 階段被忽略或轉換，不得留在新 schema。
- [ ] 任何 `curated-upstream`、`mirrored` 或 `forked` plugin 缺少來源、license、ref 或 checksum policy 時，catalog 驗證失敗。
- [ ] `~/.gal/state/plugins.lock.json` 能固定 upstream ref、checksum、license、provider support、profiles 與 resolver version。
- [ ] `~/.gal/config/config.json` 可透過 `galRoot` + `devMode` 穩定地切換 source mode，而 `~/.gal/config/xmachine.json` 保持 xmachine routing / binding 邊界清楚。
- [ ] `~/.gal/generated/mcp/managed.json` 會輸出 resolved `mcpMemoryFilePath`，並只在 filesystem MCP 仍存在時輸出 optional `mcpFilesystemPaths`。
- [ ] `~/.gal/generated/mcp/managed.json` 可直接包含 render 後的 Context7 auth header，而不再依賴 runtime 重新解析 `${CONTEXT7_API_KEY}` placeholder。
- [ ] drift check 能辨識 repo 內 copied/mirrored skills 與 lockfile ref/checksum 不一致。
- [ ] provider-neutral package model 不再把整個 repo `skills/` 無條件打包至單一 `gal` payload。
- [ ] `gal-core` package builder 只包含 GAL-owned skills、commands、agents、conventions、workflows 與 MCP boundary。
- [ ] 建置 `dist/provider-plugins/<provider>/<plugin-id>/` 時，不產生指向成品根目錄之外的符號連結。
- [ ] install mode dry run 顯示 provider、resolved plugin set、support tiers、install strategy、canonical package source、是否使用 `~/.gal/active/<provider>/` shortcut mapping、是否有 bridge/degraded lane。
- [ ] Copilot install mode dry run 優先使用 `copilot plugin install` 或 documented native install/cache path；若需轉址，僅建立指向 `~/.gal/active/copilot/` 的 capability shortcut。
- [ ] Copilot plugin manifest / marketplace entry 只使用官方 plugin reference 已記錄的 component fields，且不依賴 plugin-local `bin` 或一般 script runtime。
- [ ] AGY install mode dry run 不建立 `~/.gemini/gal` 或 `~/.gemini/antigravity-cli/gal` 的 repo-root shortcut，僅寫入正確的 AGY plugin staging 目標與必要的 `~/.gal/active/agy/` 受管理轉址。
- [ ] Codex install mode dry run 產生 marketplace 或 plugin artifacts，若需 shortcut 僅建立指向 `~/.gal/active/codex/` 的 capability link，不建立 repo-root command skill symlink。
- [ ] Claude install mode dry run 以 `claude plugin install` 或 documented local/plugin marketplace path 安裝 canonical package，並輸出其他 provider 的 mapped artifact/shortcut plan。
- [ ] source mode 明確選擇時，仍可為 contributor 依 `~/.gal/config/config.json.galRoot` 建立 repo-root shortcut，且 install-state 標示為 source mode。
- [ ] capability-level link 在 install mode 存在時只會指向 `~/.gal/active/<provider>/`；source mode / local override 才可指向其他明確本機路徑，且 dry run 可見。
- [ ] install mode 的指令技能不包含 `{{GAL_ROOT}}`、source checkout absolute path 或 provider `gal` repo-root shortcut path。
- [ ] plugin-aware MCP 輸出保留使用者擁有的全域 MCP 設定，並僅在 migration cleanup 期間移除 GAL-managed legacy items。
- [ ] uninstall 只移除 GAL-managed plugins / cache / staged artifacts / explicitly managed legacy links，不刪除使用者自有 provider 設定。

## Success Criteria

- [ ] 一般使用者可透過 provider-native install mode 安裝 `gal-core`，無須 clone 本儲存庫。
- [ ] 初始 default profile 安裝面保持最小，只包含 `gal-core`；companion plugins 以 opt-in profile 或 explicit selection 管理。
- [ ] 一般使用者可額外安裝 catalog 中選定的外部 companion plugins，而非被迫安裝全部語言/領域 skills。
- [ ] 使用者能從文件回答：應選 install mode 或 source mode、某 companion plugin 是誰維護、update/uninstall 會移除什麼。
- [ ] 使用者能透過 `~/.gal/config/config.json` 備份、轉移與重建個人化設定與 plugin set，並透過 `galRoot` + `devMode` 啟動穩定的 dev/source mode。
- [ ] 使用者不需要再設定 `GAL_SKILLS` 此類共享 skills 路徑，install mode 仍可正常運作。
- [ ] `.copilot/gal`、`.gemini/gal` 與 `.gemini/antigravity-cli/gal` repo-root shortcut 不再是 install mode 所需；若 provider 需要轉址，統一落在 `~/.gal/active/<provider>/`。
- [ ] source mode 仍可供 GAL contributors 使用，並被明確記錄為開發工作流程。
- [ ] provider artifacts 符合各 provider 有記錄的 plugin 架構，而非偽造共同目錄結構。
- [ ] `gal-results/` 不被用於 provider plugin artifacts。
- [ ] install mode 指令技能不嵌入絕對 source checkout path。
- [ ] MCP ownership 明確、具備 plugin awareness，且對本地機密安全。
- [ ] external companion plugin 與 upstream plugin 的來源、版本、license、support tier 與 checksum 可追溯。
- [ ] repo 不再依賴無版本、無 provenance 的「從 GitHub 直接複製 skills 進 core」模型。
- [ ] OpenCode 與 Gemini legacy 的邊界被誠實記錄，而不是假裝它們已納入同一 provider plugin substrate。

## Risks

- 如果 catalog resolver 與 provider renderer 邊界不清楚，現有 provider-neutral package model 會繼續把整個 repo `skills/` 打包成單一 payload。
- 如果 support tier 不清楚，使用者會誤以為 GAL 對 Dart、Flutter、.NET、Obsidian 等外部 plugin 內容負完整維護責任。
- 如果 Claude baseline 與其他 provider 的 mapping 邊界不清楚，canonical schema 可能膨脹成 fake cross-runtime runtime contract。
- 如果 OpenCode 被塞進 primary provider set，計畫會混淆 static provider plugin 與 JS/TS bridge plugin 兩種不同模型。
- 如果 `~/.gal/active/<provider>/` shortcut mapping 實際上仍偷偷依賴 source checkout scripts，install mode 只會隱藏 shortcut 問題，而非解決它。
- GitHub 官方 Copilot CLI plugin reference（2026-05-22 重新驗證）記錄 `agents`、`skills`、`commands`、`hooks`、`mcpServers`、`lspServers`、marketplace、Git URL / local path install，與 Claude Code 使用幾乎相同的目錄慣例；但仍未記錄 plugin-local `bin/` / general script runtime。若計畫強依賴 `bin/` 能力，Copilot lane 在該功能面向降級，但其他元件已達 near-parity。
- 如果第三方 skills 沒有 catalog / lockfile / license governance，GAL 會繼續累積漂移、過期與授權不明的 payload。
- 如果 MCP 本地值渲染過早，install artifact 可能洩漏 machine-local 機密或路徑。
- 如果 `context7ApiKey` 被 materialize 進 `~/.gal/generated/mcp/managed.json` 之後沒有清楚標示為 machine-local secret-bearing artifact，generated MCP config 可能被誤提交或誤分享。
- 如果 cleanup 在 provider-native install 被驗證前就執行，使用者可能會失去仍可運作的 GAL 指令。
- 如果 source mode 與 install mode 共用太多程式碼卻沒有清晰 mode guard，未來變更可能會在 install mode 意外重建 `GAL_ROOT` shortcut。

## Open Questions

- [x] OQ-001 — `feat-gal-provider-plugin-packaging` 是否仍是未完成前置？*(提出者：deep-planning，解決方式：repo state)* 否。`.dev/state.md` 顯示該計畫已完成；本計畫改為先跑 AGY baseline regression guard。
- [x] OQ-002 — install mode 是否需要在 `~/.gal/` 下提供獨立 GAL runtime install，供不支援 plugin-local scripts 的 provider 使用？*(提出者：planning，解決方式：user decision)* 需要，但排程上必須在 Claude Code plugin 能穩定運作之後再做；它是後續 runtime lane，不是目前這份計畫的先行 gate。相關 machine-local 個人化設定統一收斂於 `~/.gal/config/config.json`，而非 repo-root `.local` 檔案。
- [x] OQ-003 — Copilot CLI 是否存在足夠穩定、已記錄的 plugin-local runtime 入口；若沒有，GAL 是否應避免在 Copilot companion plugin 中承諾 plugin-local script execution？*(提出者：planning，解決方式：official docs)* 依 GitHub 官方 Copilot CLI plugin reference（2026-05-22 重新驗證），Copilot 現已具備與 Claude Code 近乎對等的 plugin 元件支援：`agents/*.agent.md`、`skills/` with `SKILL.md`、`hooks.json`、`.mcp.json`、`lsp.json`、marketplace、Git URL 與 local path install。目錄結構與 Claude Code canonical package 幾乎完全相容，因此 Copilot lane **不再需要 capability-level shortcut**，可直接使用 native `/plugin install`。但仍未記錄 plugin-local `bin/` 或一般 script runtime，因此本計畫仍應避免在 Copilot companion plugins 中承諾 plugin-local script execution。
- [x] OQ-004 — install mode 應在一個 provider 通過驗證後立即成為預設，還是等 AGY 與 Copilot 都通過 smoke test 後才成為預設？*(提出者：planning，解決方式：deep-planning architecture review)* 不在單一 provider 通過後立即成為預設。Install mode 需至少 Claude-compatible canonical package、AGY baseline regression 與 Copilot native install smoke 均通過，才可作為一般使用者預設；此前保持 opt-in。
- [x] OQ-005 — 哪些 skills 必須保留在 `gal-core`，哪些必須轉為 external companion/upstream catalog entries？*(提出者：planning，解決方式：user decision)* 已定案：目前只有 `dart-lang/skills`、`flutter/skills`、`dotnet/skills`、`anthropics/skills`、`samber/cc-skills-golang`、`twostraws/swift-agent-skills`、`kepano/obsidian-skills`、`actionbook/rust-skills` 這八個 repos 算 external companion / upstream catalog entries；其餘像 game asset、Godot 等 skills 目前保留在 GAL 內建 inventory。
- [x] OQ-006 — 對第三方 skills，GAL 應採用 curated-upstream、mirrored、forked 或 local-only 模式；各模式的允許條件為何？*(提出者：planning，解決方式：user decision + policy synthesis)* GAL 的角色是協助使用者管理哪些 plugins 已安裝，以便轉移、備份、更新，因此第三方 skills 的預設模式應為 `curated-upstream`：只要 upstream repo 存在、可直接安裝、license/provenance/checksum 可追溯，就由 GAL 記錄與鎖定安裝狀態。`mirrored` 只用於 upstream 無法直接穩定消費但仍需受管快取的情況；`forked` 只用於必須維護有意識差異或修補相容性的情況；`local-only` 只用於 source mode / explicit local override，不作共享預設。
- [x] OQ-007 — 初始 default profile 應包含哪些 companion plugins，哪些只作 opt-in？*(提出者：business-review，解決方式：deep-planning architecture review)* 初始 default profile 只包含 `gal-core`。八個 curated-upstream companion candidates 全部透過 named profiles 或 explicit plugin selection opt in。

## Approval

- 人工核准：[待處理]
- Architect review: [clear - deep-planning gate passed; next step is `/refining-plan`]
- 額外領域審查：[商業審查已執行；設計審查未觸發]

## Review Results

### Architecture Review

重新審查：2026-05-25 deep-planning + 跨平台官方文件重新驗證。

結論：**通過 deep-planning gate，可進入 `/refining-plan`；不得直接進入 implementation 或 `/plan-to-prompt`。**

原始計畫方向正確但不可直接 build：它把多 plugin artifact、provider-neutral package model、OpenCode bridge 與 shortcut fallback 混在同一層。修正後的架構中心改為 catalog resolver 與 lockfile，並明確以 Claude plugin data struct 作為 canonical schema；現有 provider-neutral package model 只作為 `gal-core` renderer input，不再代表整個外部 companion plugin 生態。

Graphify structural context 已納入：`Setup Utilities & Script Updates`、`MCP Configuration Management`、`GAL Core State & Plan Management`、`Adapter Content Generation` 與 `Tool Status & Discovery` 是本計畫主要碰觸的社群；`Invoke-UpdateCommands()`、`.dev/state.md`、`Get-GraphifyStatus()`、`Test-CommandAvailable()` 與 `Invoke-UpdateMcp()` 等 god nodes 顯示本計畫必須保持 setup、MCP、state 與 adapter generation 的邊界清楚。

#### 權衡總結

| 決策 | 效益 | 成本 | 結論 |
| --- | --- | --- | --- |
| 使用 catalog + lockfile orchestration | 解決 provenance、drift、repeatable setup | 需要 resolver/schema/version policy | OK |
| 將 existing provider-neutral package model 彙整為 `gal-core` input | 保留已完成 AGY packaging 成果 | 需要拆出 external plugin resolver | OK |
| 採用 Claude plugin data struct 作為 canonical schema | 先以最完整結構規劃，再對應到其他 provider | 需要嚴格限制 schema 與 runtime contract 的邊界 | OK |
| 允許 capability-level link 指向 `~/.gal/active/<provider>/` | 保留跨 provider 變通能力且不回指 repo root | 需要 dry run 與 uninstall visibility | OK |
| 初始 default profile 只包含 `gal-core` | 降低首次安裝、授權與支援風險 | 使用者需明確選 companion profiles | OK |
| install mode 預設切換需 Claude canonical、AGY 與 Copilot guard | 避免單 provider 成功被誤當成整體成熟 | 預設切換較慢 | OK |
| OpenCode 留在 bridge lane | 避免混淆 static provider plugin 與 JS/TS plugin | OpenCode parity 需另案 | OK |
| 支援 support tiers | 降低使用者與維護責任混淆 | catalog schema 較完整 | OK |

#### 已修正的 blocking issues

- **BLK-01**：架構中心已從 provider-neutral package model 改成 catalog resolver + lockfile。
- **BLK-02**：Golden Standard + selective shortcut 已被採用，但限制為「Claude canonical schema + `~/.gal/active/<provider>/` managed shortcut mapping」，不允許 repo-root shortcut。
- **BLK-03**：OpenCode 保持 bridge lane，不列入四 primary install targets。
- **BLK-04**：install mode 與 source mode 已建立明確 operational contract。
- **BLK-05**：`feat-gal-provider-plugin-packaging` 已從等待前置改為 baseline regression guard。
- **BLK-06**：install mode default activation 與 initial default profile 已以保守預設關閉。

#### 仍需在 `/refining-plan` 鎖定

- Catalog schema 的 JSON shape 與 required fields。
- `~/.gal/config/config.json`、`~/.gal/config/xmachine.json` 與 `~/.gal/generated/mcp/managed.json` 的欄位與優先序。
- Lockfile drift detection 的具體規則。
- 初始 `gal-core` skill inventory 與 external plugin 搬遷清單的實際盤點輸出。
- 每個 primary provider 的 first implementation target 與 smoke command。
- `~/.gal/active/<provider>/` shortcut mapping 只應套用於缺少 native install 或 parity 的 provider，不應自動套用到所有非 Claude provider。
- `context7ApiKey` 與 `~/.gal/generated/mcp/managed.json` 必須明確標示為 non-shareable secret-bearing machine-local state。
- `Setup-Machine.*` 變更必須拆成 schema/catalog、resolver、AGY regression、path decoupling、provider lifecycle 等可驗證階段，避免一次大型重寫。
- Legacy repo-root links 與 global MCP cleanup 必須有 ownership tests，確保 uninstall 不刪 user-owned provider config。
- 既有 provider-plugin substrate 只能收斂為 `gal-core` input，不應再建立第二套平行 package model。

### Business Review

結論：**已吸收；支援分層、default profile 與預設切換 policy 已納入 source plan。**

本計畫的使用者價值不是只有「不需要 clone」，而是 provider-native install、predictable updates、safe uninstall、無 absolute local paths、較低 support burden。修正後已加入 user promise、support tiers、runtime lane policy 與 mode contract。

商業風險以保守預設處理：install mode 保持 opt-in 到 canonical、AGY 與 Copilot guards 全部通過；初始 default profile 只包含 `gal-core`，companion plugins 只透過 opt-in profile 或 explicit selection 啟用。

Fallback analyst review（2026-05-25）結論：**CLEAR，無 blocking findings。** `/refining-plan` 需把 support tier 承諾轉成使用者文件 acceptance criteria，明確回答內容由誰維護、相容性由誰負責、各 tier 的 auto-update 行為，以及 install/update/uninstall dry run 會如何顯示 ownership boundary、secret-bearing generated MCP files 與 provider settings preserved。

所有範例與預設流程都必須維持 companion plugins opt-in，不得讓 language packs 看起來像 default bundled payload。驗收也需要覆蓋「使用者在安裝前能辨識 plugin 是 GAL-maintained、upstream-maintained 或 local-only」。

### Design Review

未觸發。本計畫沒有顧客面對的 UI、layout、visual states 或 accessibility surfaces；僅涉及 install/onboarding wording 與 catalog 分群。

### Engineering Review

待處理。下一步 `/refining-plan` 應填入可實作的任務、測試計畫、資料 shape 與工程審查結論。

## Test Plan

- Schema validation：`plugins/catalog.json`、`~/.gal/config/config.json`、`~/.gal/state/plugins.lock.json`、`~/.gal/config/xmachine.json`、`~/.gal/generated/mcp/managed.json` required fields、support tier、provider support、checksum policy、license policy。
- Resolver validation：catalog + selected profile 產生 deterministic resolved plugin set，且 lockfile round-trip 穩定。
- Drift validation：copied/mirrored skills 與 lockfile ref/checksum 不一致時失敗。
- Mode validation：install mode 不產生 repo-root shortcut，但可產生指向 `~/.gal/active/<provider>/` 的 managed capability links；source mode 透過 `~/.gal/config/config.json.galRoot` 與 `devMode` 明確標示 repo link；bridge lane 顯示 capability-level links。
- Provider validation：AGY baseline plugin-only lifecycle 先通過，再逐 provider 驗證 native install/list/status/update/uninstall。
- Security validation：plugin artifacts 不包含 local secrets、machine-local paths 或 `~/.gal/generated/mcp/managed.json` 之外的未受管 MCP secret values；`context7ApiKey` 若被 materialize，必須只存在 machine-local generated MCP config。
- Cleanup validation：uninstall 只移除 GAL-managed plugin/cache/staged/legacy items，保留 user-owned provider config。

## Tasks

### Phase 0：Baseline guard

- [ ] A-001 — 驗證 [feat-gal-provider-plugin-packaging.md](feat-gal-provider-plugin-packaging.md) 完成後的 AGY plugin-only baseline 仍然通過，包含 plugin-root MCP、`rules/gal.md`、legacy cleanup 與 `Setup-Machine` reinstall/uninstall。

### Phase 1：Inventory 與 catalog governance

- [ ] A-002 — 盤點現有 `skills/`，提出 `gal-core`、external companion plugins、upstream/mirrored/forked/local entries 的初步分群。
- [ ] A-003 — 定義 `plugins/catalog.json` schema，涵蓋 plugin ID、support tier、source type、upstream ref、license、checksum policy、component map、supported providers、install strategy、default profiles、local override policy。
- [ ] A-004 — 定義 `~/.gal/config/config.json`、`~/.gal/state/plugins.lock.json`、`~/.gal/config/xmachine.json`、`~/.gal/generated/mcp/managed.json` schema 與 drift detection / precedence 規則。
- [ ] A-005 — 定義 default profile 與 opt-in plugin profile policy。

### Phase 2：Resolver 與 package boundary

- [ ] A-006 — 實作 catalog resolver，輸出 deterministic resolved plugin set。
- [ ] A-007 — 定義 Claude-compatible canonical package schema，並將現有 provider-neutral package model 彙整為 `gal-core` package input，不再無條件收集整個 repo `skills/`。
- [ ] A-008 — 讓 provider renderers 接收 resolver output 並保留 external upstream package identity；對非 Claude provider 產生 `~/.gal/active/<provider>/` shortcut mapping。

### Phase 3：Mode selection 與 path decoupling

- [ ] A-009 — 更新 `Setup-Machine.*` 支援 install mode / source mode / migration cleanup / bridge lane、`~/.gal/config/config.json` machine config、`~/.gal/config/xmachine.json` machine binding 與 `~/.gal/active/<provider>/` shortcut layout。
- [ ] A-010 — 移除 install mode 對 `{{GAL_ROOT}}`、source checkout absolute path 與 provider repo-root shortcut path 的依賴；保留對 `~/.gal/active/<provider>/` 的 GAL-managed redirect。
- [ ] A-011 — 實作 local override policy，允許 source mode 對外部 plugin 使用 explicit local path。

### Phase 4：Provider-native lifecycle

- [ ] A-012 — 實作 provider-native installer / updater / uninstall flows，先維持 AGY baseline，再循序擴展 Copilot CLI、Codex、Claude Code。
- [ ] A-013 — 將 OpenCode 記錄為 bridge/degraded lane，另案處理 plugin bridge，不納入本計畫 primary renderer。

### Phase 5：Migration 與 docs

- [ ] A-014 — 在 catalog governance 與 migration UX 明確後，規劃 repo copied external skills 的移除、mirror 或 fork 搬遷。
- [ ] A-015 — 更新 README、繁中 README、personalization docs、devguide 與 scripts inventory，清楚說明 user promise、support tiers、mode selection、update/uninstall 與 provider limitations。
