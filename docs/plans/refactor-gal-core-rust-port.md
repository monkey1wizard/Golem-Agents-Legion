# Plan: GAL 核心腳本層 Rust 原生化（refactor-gal-core-rust-port）

> **直接取代** `feat-gal-rust-native-install.md`（已刪）。前計畫只把安裝引擎寫成 Rust（~1000 行已 commit），卻把「取代 ps1/sh」當尾端任務延後 —— 與使用者「全面轉 Rust」要求不符。本計畫接管其全部成果,把**核心腳本層全部 Rust 化並刪除對應 ps1/sh**。xmachine 遠端執行子系統由姊妹計畫 `refactor-gal-xmachine-rust-port.md` 負責(該計畫依賴本計畫先抽出的 `base`/`dispatch`)。

## Approval

- Human approval: [approved at 2026-06-09]
- Architect review: **APPROVE(方向;二審架構分解 + 四審真機驗收方法論修正 CORR-01 均折入,2026-06-09)** — 見 ## Review Results。CORR-01:真機驗收 = 純 end-user 裝預編譯 artifact,絕不在測試機 build;新增 R-13 release 管線為前置。實作期 R-00/R-03/R-04/R-05 受保護核心須 architect 簽核。**~~Tasks 已變動 → 須重跑 `/plan-to-prompt` 刷新執行 prompt。~~ 已完成:prompt 已刷新,R-00 實作期 architect 批次簽核 APPROVE(3 條件,見 prompt ## Review Results),T-003..T-008 已執行完成。**
- Additional domain review: [not triggered]（無 customer-facing / business-rule）

## Goal

`scripts/` 中**所有非 xmachine** 的 Bash+PowerShell 功能由 Rust 原生實作並達行為 parity,對應 `.ps1`+`.sh` 成對刪除。終態:安裝/設定/MCP/regen/catalog/release/filter/translation 一律走 `gal` binary,`scripts/` **核心家族零 ps1/sh,無例外**。

## Motivation（兩個驅動,所有取捨的根據）

1. **消除雙實作維護成本** —— 現況每功能寫兩份(`.ps1`+`.sh`),改一處要同步兩處、已見行為飄移。單一 Rust = 寫一次,跨平台靠 `cfg!`。**鐵律:凡雙實作一律進 Rust,無「是否納入」餘地。**
2. **單一二進位加速 AI 呼叫** —— AI 高頻呼叫這些命令,編譯 binary 啟動即用,免 spawn bash/pwsh + source 共用 lib。binary 須精簡、低啟動延遲。

**零 ps1/sh 是硬性的,無例外**(經誠實檢驗):bootstrap 由 pkg-manager/Releases/cargo 取得 binary,免 GAL 腳本;git filter 改 `gal clean`/`gal smudge`(git config 指向 binary);shell completion 是 `gal completions` 的輸出檔非 source;遠端執行體即 binary;工具鏈安裝由 `gal setup` 呼系統 pkg-manager。`curl|sh` 便利安裝器(若提供)是 `gal release` 發佈產物,非 tracked source。

## 治理原則

- parity = 行為等價 **且** 活讀取面與 source 一致,不接受「Rust 能編譯」當完成。
- **刪除硬 gate = fixture-parity 綠**:fixture 於 P0 由現況可運作的腳本凍結;script 刪除前提是 Rust 輸出對齊該 fixture。
- monolithic script 須**全部** live consumer(非註解)皆 parity 才整檔刪。
- **混合態不變量**:遷移期任一 phase 邊界不得讓 consumer 呼到「已刪/半搬」面;每 phase 結束系統完整可運作。
- oracle-parity 測試刪 script 前先 reparent 為 `tests/fixtures/` snapshot 或行為測試。

## 已完成成果（接管 `feat-gal-rust-native-install`,引擎側已 commit）

install/render/doctor/mode/claude-skill 投影/bin 暴露/孤兒清理/跨平台/pkg-manager 收斂/oracle reparent，共 a192df7..bcc755a(228 test 綠)。

**R-00 架構解耦完成(本計畫,2026-06-09,T-003..T-008)**:6577 行 `gal-engine` god-crate 開始退役 —— 抽出 `base`(config/mode/ledger/paths/mcp 型別/platform/render/health 含 `HealthCheck` trait)與 `providers`(claude/copilot/agy 投影)兩個無前綴 crate;`gal-cli`→`cli`、`gal-dispatch`→`dispatch` 去前綴。workspace 現為 `base · providers · cli · gal-engine · dispatch`(gal-engine 以 `pub use` 過渡再匯出,呼叫面零改)。依賴鐵律驗證:base 無 GAL dep、providers 僅依 base、無環。commits:T-003 1ffd3de、T-004 5c1514e、T-005 8f78581、T-006 ba5fc69、T-007 c0f1f37、T-008 b190cf7;每步 `cargo test --workspace` = 228/0。architect 三條件閉合:BUG-01(MCP 型別入 base 先解環)、OE-01(base::render 範圍圍欄)、BUG-02(dispatch bin 名保留,gal.ps1 不動)。

**未竟(本計畫接手)**:真機 TP-02(mac-mini)/TP-03(Windows)端 end-user artifact 驗收待 R-13 管線(T-034)就緒;R-08 oracle reparent(T-009)為刪除任務前置,尚未開始。本機開發基準(T-002)已綠。

## Rust 目標架構

> **現況問題**:`gal-engine` 是 6577 行 god-crate(十個不相關 domain),"engine" 在 code 出現 **0 次** = 垃圾名 → **退役**。先解耦再 port。
>
> **命名慣例**:crate 為單一 `gal` binary 內部實作、不個別發佈 → workspace **無前綴**(同 rust-analyzer 的 `hir`/`ide`/`vfs`)。硬約束:foundation 不可叫 `core`(保留字)→ `base`;名錨定 domain 詞彙(`provider` 54× vs `runtime` 3× → `providers`)。binary 仍 `gal`(`cli` package + `[[bin]] name="gal"`)。現有 `gal-cli`/`gal-engine`/`gal-dispatch` 一併去前綴。

| 層 | crate | 職責 | 依賴 |
| --- | --- | --- | --- |
| foundation | `base` | config/mode/paths/install-state/provider-selection;`platform` 模組(symlink·junction·perms·atomic-swap);`render` 模組(模板渲染原語);`HealthCheck` trait | 無 GAL crate |
| capability | `providers` | Provider trait + claude/copilot/codex/agy 投影 | base |
| capability | `install` | canonical_root render + bin 暴露 + 孤兒清理 + uninstall ledger | base, providers |
| capability | `mcp` | MCP 配置 | base |
| capability | `adapters` | adapter/skill/command/personalization regen（Sync-DevContext + Update-*）,共用 `base::render` | base |
| capability | `release` | release packaging | base |
| capability | `vcs` | git filter(clean/smudge) + commit-msg 生成 | base |
| orchestrator | `setup` | 機器設定:**只編排** install+mcp+adapters + git-filter 註冊,無 domain 邏輯 | base, install, mcp, adapters, vcs |
| edge | `cli` | 薄入口,名詞分組子命令,`[[bin]] name="gal"`;聚合各 domain 的 `HealthCheck` 為 `gal doctor` | base + 各 domain |

**依賴鐵律**:`base` 不依賴任何 GAL crate;所有 domain 只依賴 `base`(+ install 依 providers);**無環,無 god-crate**。

**架構決定(本次 architect 審查折入)**:
- **doctor 依賴反轉** —— `HealthCheck` trait 定義在 `base`,各 domain 實作之;`cli`/doctor **聚合 trait 物件**,不直接依賴每個 domain 的內部。doctor 不變成 import 全 workspace 的 crate。
- **render 原語共用** —— `install`(canonical_root)與 `adapters`(CLAUDE.md 等)都渲染模板檔 → 渲染原語下沉 `base::render`,兩者複用,杜絕重複。
- **platform 自足** —— OS 原語(symlink/junction/perms)為 `base::platform` 自足模組,不與 config 糾纏;體量成長到需編譯隔離時再升 crate。
- **crate-vs-module 紀律** —— 只有具獨立依賴集/編譯隔離理由才升 crate;`vcs`/`release` 體量小,預設模組,architect 確認最終粒度。

子命令名詞分組:`gal install|update|doctor`、`gal mcp`、`gal setup`、`gal sync`、`gal release`、`gal git {clean,smudge}`。

## 全面盤點:每個核心 script 的 Rust 目標（「哪裡沒改到」總表）

> 狀態:✅ Rust 已有 / 🟡 模組存在但未接子命令或未 parity / ❌ Rust 完全沒有。xmachine 家族見姊妹計畫。

| Script（.ps1+.sh 成對,除非註明） | 職責 | Rust 目標 | 狀態 |
| --- | --- | --- | --- |
| `install-gal-plugins.{ps1,sh}` | 四 provider 安裝編排 | `install` / `gal install` | 🟡 Claude skill 面 parity;Copilot/Codex/AGY 編排未證 |
| `build-core-plugin.{ps1,sh}` | core plugin render | `install`(render) | 🟡 canonical render 有;AGY core-plugin build 未證 |
| `build-provider-plugins.{ps1,sh}` | provider plugin render | `install`/`providers` | 🟡 內部,隨上層 |
| `common/provider-plugin.{ps1,sh}` | provider 投影共用 | `providers` | 🟡 內部 lib |
| `update-mcp.{ps1,sh}`(1225/2014) | 跨 provider MCP 配置 | `mcp` / `gal mcp`(後端 `mcp.rs` 已存在) | 🟡 模組在,無子命令,未 parity |
| `update-skills/commands/personalization.{ps1,sh}` | adapter 再生 | `adapters` / `gal sync`·`gal update` | ❌ 無對應 |
| `Sync-DevContext.{ps1,sh}`(受保護) | repo-local adapter 生成 | `adapters` / `gal sync` | ❌ 無對應(與 update-* 同 `adapters` 後端) |
| `setup-machine.{ps1,sh}`(受保護) | 機器設定編排 | `setup` / `gal setup` | ❌ 無對應(推翻前計畫 KEEP) |
| `setup-tools.{ps1,sh}`(668/740) | 工具鏈安裝 | `setup` / `gal setup --tools` | ❌ 無對應 |
| `uninstall-machine.{ps1,sh}` | 解除安裝 | `install` ledger / `gal uninstall` | 🟡 存在,精確度未證 |
| `init-repo.{ps1,sh}` | repo `.dev/`/plans 初始化 | `install` / `gal init-repo` | 🟡 部分涵蓋,須確認 |
| `Resolve-GalCatalog.ps1`(單) | GAL catalog 解析 | `base`/`install` | ❌ 無對應 |
| `gal-clean.sh`/`gal-smudge.sh`(git filter) | personalizable smudge/clean | `vcs` / `gal clean`·`gal smudge` | ❌ 無對應;`setup` 註冊 filter |
| `Package-ReleaseArtifacts.{ps1,sh}` | release 打包 | `release` / `gal release`(已存在) | 🟡 是否全覆蓋須確認 |
| `Test-TranslationFreshness.{ps1,sh}` | 翻譯新鮮度 | `gal` 子命令或 doctor 子檢 | ❌ 無對應 |
| `common/Common.{ps1,sh}`(43 函式,僅 1 屬 xmachine) | 安裝/設定共用基建 | `base` | 🟡 散落;xmachine 部分留姊妹計畫,整檔刪需兩計畫皆 done |
| `gal.{ps1,sh}`(748/1051) | 入口 dispatcher | `cli` | 🟡 核心子命令多已有;xmachine/dispatch 入口屬姊妹計畫,整檔刪掛兩計畫尾端 |
| `Test-ResolveGalCatalog.ps1`、`tests/Test-InstallModeAuthority.ps1`、`test-install-acceptance.sh` | 安裝/catalog 測試 | Rust 測試/fixture | 🟡 reparent 後刪 |

## Requirements

- [x] **R-00 架構解耦（JIT,非大爆炸,受保護）** — 先抽**最小高槓桿**:`base`(config/mode/paths/install-state/provider-selection + `platform`/`render` 模組 + `HealthCheck` trait;render.rs 跨平台原語下沉)、確立 `providers`、`gal-cli`/`gal-dispatch` 去前綴。**其餘 domain crate(install/mcp/adapters/release/vcs/setup)的拆分排在各自 port phase 起點 JIT 做,不一次拆完** —— 避免對 228-test 綠基線做零功能大爆炸重構。依賴鐵律:base 無 GAL dep、無環、無 god-crate;doctor 走 `HealthCheck` trait 聚合。須 architect 簽核。 **✅ 基礎抽取完成(2026-06-09,T-003..T-008,1ffd3de..b190cf7,228 test 綠):base/providers 抽出、platform/render/health 下沉、cli/dispatch 去前綴。architect 批次簽核 APPROVE 三條件全閉合(BUG-01/OE-01/BUG-02)。剩餘 JIT domain 拆分(install/mcp/adapters/release/vcs/setup)依設計於各 port phase(R-02..R-06)起點進行;R-08 oracle reparent(T-009)為下一步。**
- [ ] **R-01 共用核心 → `base`** — `common.{ps1,sh}` 的安裝/設定函式(install mode、provider 選擇、symlink、install state、plugin root)落 `base`,供 install/setup/mcp/adapters 複用;xmachine 專屬函式不在此。
- [ ] **R-02 `gal mcp`** — `update-mcp.{ps1,sh}` 全面 Rust 化為 `gal mcp`,接 `mcp.rs`,四 provider parity。
- [ ] **R-03 adapter-regen + Sync-DevContext → `adapters`（受保護）** — `update-skills/commands/personalization` 與 `Sync-DevContext.{ps1,sh}` 收斂為**單一 `adapters` 後端**(皆生成 adapter 檔,語義同類);CLI 以 `gal sync`(init-time 生成)/`gal update`(增量 regen)暴露,共用 `adapters` + `base::render`。須 architect。
- [x] **R-04 `gal setup`（受保護）** — `setup-machine.{ps1,sh}` + `setup-tools.{ps1,sh}` 的機器設定/工具鏈編排 Rust 化為 `gal setup`(工具步驟以 `--tools` 暴露);含註冊 git filter(依 architect C-4 暫指 .sh,binary 切換在 T-025)。`setup` 只編排無 domain 邏輯。architect 簽核 APPROVE-with-conditions C-1..C-10。**✅ 完成(2026-06-10,T-018..T-021,2c68fe2..41ff61a,392 test 綠):setup crate(session/agy/legacy_plugins/tools/git_filter/health)只編排;machine surfaces/MCP 走 library call;Install-GalPlugins 為唯一 strangler spawn 點(T-024 改接);腳本對已刪、Uninstall-Machine 改接 `gal setup --uninstall`、docs/adapters 全面同步。**
- [ ] **R-05 安裝家族 parity + 刪除** — `install-gal-plugins`/`build-core-plugin`/`build-provider-plugins`/`provider-plugin` 四 provider 編排與 render 全面 parity;確認後成對刪除。
- [ ] **R-06 雜項 Rust 化** — `gal clean`/`gal smudge`(`vcs`)、`gal uninstall` parity、init-repo、catalog 解析、release packaging 併 `gal release`、translation freshness 各有 Rust 對應後成對刪除。
- [ ] **R-07 `gal` 入口核心子命令** — `gal.{ps1,sh}` 核心子命令(install/update/doctor/setup/mcp/sync/uninstall/clean/smudge)全走 Rust;入口檔整檔刪除待姊妹計畫 xmachine/dispatch 入口也 parity(跨計畫尾端共同收尾)。
- [ ] **R-08 oracle reparent 前置** — 以 live script 為 oracle 的測試,刪前改 fixture/行為測試。
- [ ] **R-09 跨平台正確** — Windows junction / Unix symlink / 權限 三平台對齊;隔離 home 驗證。
- [ ] **R-10 真機驗收 = 純 end-user 安裝 release artifact（硬 gate；修正）** — 真機(mac-mini、Windows normal)以**純 end-user** 身分安裝 **R-13 release 管線產出的預編譯 artifact**(brew / GitHub Releases / scp 該 release 檔),**絕不在測試機上裝 rust、build、或臨時跨編譯**。驗 doctor green、skill 面載入、canonical root 完整、packaged-source 自解析。**前置 = R-13 artifact 存在。**
- [ ] **R-11 doctor 涵蓋新面** — 各 domain 實作 `HealthCheck`;`gal doctor` 聚合並擴及 mcp/setup/sync/filter 健康檢查,缺口 fail-loud。
- [ ] **R-12 終態零 ps1/sh（硬性,無例外）** — 結束時 `scripts/` 不留任何核心家族 ps1/sh。
- [ ] **R-13 跨平台 release artifact 管線（新增；R-10 前置）** — 以 **CI(macOS/Linux/Windows runner,如 GitHub Actions matrix)** 產出各 target 的預編譯 `gal` artifact(macOS-arm64 至少)+ 打包 source(FHS/flat),發佈到 GitHub Releases / brew tap。end-user 與真機測試**只安裝這些 artifact**,永不在目標機 build。`gal release` 不能只產當前平台 —— 須有真正的跨平台產線。**治理原則:絕不在 end-user/測試機安裝工具鏈或 build。**

## Approach（逐期 parity→reparent→刪除）

| Phase | 目標 | 前置 |
| --- | --- | --- |
| **P0 盤點凍結 + 本機 dev 基準** | 每核心 script 凍結 parity fixture;在**開發機(Windows,有 rust)**重裝 dev-mode `gal install` 確立本機綠基準。**真機 end-user 驗收不在此**(見 P8/R-10/R-13) | — |
| **P0.5 架構解耦（R-00,受保護,JIT）** | 只抽 `base`+`providers`+去前綴,驗 228 test 綠;domain crate 拆分延到各 port phase | P0、architect |
| **P1 共用核心（R-01）** | common.{ps1,sh} 安裝/設定函式 → `base` | P0.5 |
| **P2 `gal mcp`（R-02）** | port update-mcp;parity;刪對 | P1 |
| **P3 regen+sync（R-03,受保護）** | port update-* + Sync-DevContext 為 `adapters`;architect;parity;刪對 | P1、architect |
| **P4 `gal setup`（R-04,受保護）** | port setup-machine+setup-tools+filter 註冊;architect;parity;刪對 | P1–P3、architect |
| **P5 安裝家族刪除（R-05）** | 四 provider parity;刪 install-gal-plugins/build-core/build-provider/provider-plugin | P1–P4 |
| **P6 雜項（R-06）** | clean/smudge、uninstall、init-repo、catalog、release packaging、translation;成對刪除 | P1 |
| **P7 入口核心子命令（R-07）** | gal 入口核心子命令全 Rust;入口檔刪除掛跨計畫尾端 | P2–P6 |
| **P7.5 release artifact 管線（R-13,R-10 前置）** | CI 跨平台產線產出預編譯 artifact(macOS-arm64 等)+ 打包 source,發佈 Releases/brew | P1–P7 |
| **P8 純 end-user 真機驗收 + 清空（R-10/R-12）** | 真機**安裝 R-13 artifact**(非 build):mac-mini normal + Windows normal;`scripts/` 核心家族清空;`cargo test` 不再 spawn 核心 live script | P7.5 |

每期 Verify:`cargo test` 綠;該期 superseded script 成對刪除;測試碼不再 spawn 該期 live script;隔離 home 對齊凍結 fixture。

## Files to Create or Modify

- `[CREATE]`(受保護,架構) `crates/base/`(foundation + platform/render 模組 + HealthCheck trait)、`crates/providers/`(R-00)。
- `[CREATE/MOVE]`(受保護,架構) `gal-engine` **退役**,JIT 拆 `crates/install/`、`crates/mcp/`、`crates/adapters/`、`crates/release/`、`crates/vcs/`;render 原語下沉 `base::render`。
- `[CREATE]` `crates/setup/`(orchestrator,只編排)。
- `[RENAME]` `gal-cli`→`crates/cli`(`[[bin]] name="gal"`)、`gal-dispatch`→`crates/dispatch`。
- `[MODIFY]` `crates/cli/`:名詞分組子命令接線 + doctor 聚合 `HealthCheck`。
- `[MODIFY]`(受保護,契約面) `plugins/gal-core/commands/gal/SKILL.template.md`、`gal-init/SKILL.template.md`:引用改指 Rust binary。
- `[MODIFY]` `.gitattributes` + `gal setup` 寫 `git config filter.gal-config.clean = gal clean`。
- `[MODIFY]` `tests/fixtures/`、`crates/*/tests/*`:oracle reparent。
- `[DELETE on parity]` 安裝家族、MCP、regen+sync(含 `Sync-DevContext.*`)、設定(`setup-machine.*`/`setup-tools.*`/`uninstall-machine.*`/`init-repo.*`/`Resolve-GalCatalog.ps1`)、雜項(`gal-clean.sh`/`gal-smudge.sh`/`Package-ReleaseArtifacts.*`/`Test-TranslationFreshness.*`/`Test-ResolveGalCatalog.ps1`/`tests/Test-InstallModeAuthority.ps1`/`test-install-acceptance.sh`)。
- `[DELETE on parity,跨計畫尾端]` `common/Common.{ps1,sh}`、`gal.{ps1,sh}` — 含 xmachine 共用,兩計畫皆 done 才整檔刪。

## Success Criteria

- [ ] `scripts/` 核心家族零 ps1/sh —— **硬性,無例外**。
- [ ] `gal mcp`/`gal setup`/`gal sync`/`gal clean`/`gal smudge` 子命令存在且與舊腳本 parity(對照 fixture)。
- [ ] 四 provider 安裝、adapter 生成、MCP 配置、機器設定全走 Rust,活讀取面與 source 一致。
- [ ] git filter 改 `gal clean`/`gal smudge`,smudge/clean 行為不變。
- [ ] **跨平台 release artifact 管線(R-13)產出 macOS-arm64 等預編譯 `gal`**;mac-mini + Windows 以**純 end-user** 裝該 artifact(無 rust/build)後 doctor green、skill 面載入、canonical root 完整。
- [ ] `cargo test` 綠且測試碼不再 spawn 任何核心 live `scripts/*.{ps1,sh}`。
- [ ] 各 domain 實作 `HealthCheck`,`gal doctor` 聚合,缺口 fail-loud。

## Risks

- **範圍大 + 受保護路徑（高）**:gal-engine 核心、Setup-Machine/Sync-DevContext(推翻 KEEP)。Mitigation:R-00/R-03/R-04/R-05 走 architect;CODER≠REVIEWER;推翻 KEEP 記入 Key Decisions。
- **跨計畫共用檔（高）**:`common.*`/`gal.{ps1,sh}` 與 xmachine 計畫共用。Mitigation:只搬出核心函式/子命令,物理刪除掛跨計畫尾端共同 gate。
- **真機驗收方法論(高,CORR-01)**:測試機若被當 build host(裝 rust/跨編譯)即違反「end-user 純裝 artifact」本質。Mitigation:R-13 先產 CI artifact;R-10/T-035 真機只裝預編譯檔;治理原則明令絕不在測試機 build。
- **無 macOS artifact / CI 未建(中)**:目前無 macOS 預編譯產物,真機 end-user 驗收阻斷直到 R-13 管線(macOS CI runner)就緒;連動 GitHub 公開 + Actions(見 git-publish-strategy 私記)。Mitigation:T-034 為 T-035 硬前置。
- **真機驗收延宕重演（中）**:前計畫真機沒驗就標完成。Mitigation:T-035 硬 gate,且只認 end-user artifact 安裝結果。
- **Bash/PS 行為分歧（中）**:雙實作本就可能不一致。Mitigation:P0 並列輸出明確裁基準並記錄。
- **與活躍計畫相鄰檔（中）**:`refactor-golem-auditor`/`feat-small-context`/`fix-install-followups`。Mitigation:排序協調。

## Decisions（已定,不再是 OQ）

- **零 ps1/sh 無例外** — bootstrap/filter/completion/遠端/工具鏈皆有非腳本路徑(見 Motivation)。
- **Sync-DevContext + update-* 合一為 `adapters`** — 皆生成 adapter 檔,單一後端;`gal sync`/`gal update` 為其 CLI 面。
- **setup-tools 納入 `gal setup`** — 雙實作即納入;工具下載副作用是實作關注非排除理由。
- **dispatch / 外部直呼 / bootstrap 腳本** — dispatch 屬姊妹計畫(`dispatch` crate 已完成,不重做);無外部直呼需遷移期保活;bootstrap 不留腳本。
- **真機驗收 = 純 end-user 裝預編譯 artifact(CORR-01)** — 測試機(mac-mini 等)一律當 end-user,裝 R-13 CI 產出的 artifact,**絕不在其上裝 rust/工具鏈或 build/跨編譯**。build host(開發機/CI)與 end-user 機角色嚴格分離。R-13 release 管線是真機驗收的硬前置。

## Review Results

### Architecture Review

**Verdict: APPROVE(方向)。** 二審(2026-06-09)應使用者要求,**首次完整評估整個 Rust 架構分解**(首審在架構定案前 APPROVE,確實漏了)。Motivation(全 Rust + 單 binary)正當;分層(base/providers/capability/orchestrator/edge)、依賴鐵律、engine 退役、fixture-gated 刪除均健全。本次新增四項架構發現,全折入。

#### Trade-off Summary

| 決策 | 效益 | 成本 | 裁決 |
| --- | --- | --- | --- |
| 全 Rust 取代雙實作 | 寫一次、消除飄移、binary 啟動快 | ~13.6K 行 port + 觸受保護核心 | OK |
| 無前綴 + engine 退役 + 名錨 domain | 去垃圾名、慣例一致 | 改現有 crate 名 | OK |
| JIT 解耦(非大爆炸) | 保 228-test 綠基線 | 解耦分散各 phase | OK |
| 每 domain 一 crate | 邊界清楚、編譯並行 | crate 數多 | OK(crate-vs-module 紀律約束,非過度) |
| 刪除 = fixture-parity 綠 + 混合態不變量 | 防「刪了才發現壞」 | fixture 維護 | OK |

#### 架構發現（本次新增,已折入）

- **[ARCH-01] doctor 依賴反轉** → `HealthCheck` trait 置 `base`,各 domain 實作,`cli`/doctor 聚合 trait 物件,不 import 全 workspace。已寫入架構 + R-11。
- **[ARCH-02] render 原語共用** → `install` 與 `adapters` 皆渲染模板,渲染原語下沉 `base::render` 複用,杜絕重複。已寫入架構 + R-03。
- **[ARCH-03] base 內聚** → OS 原語為 `base::platform` 自足模組,不與 config 糾纏;成長到需編譯隔離再升 crate。已寫入架構。
- **[ARCH-04] setup 須薄** → orchestrator 只編排、無 domain 邏輯,否則又成 god-crate。已寫入 R-04。

#### Bug Surface

- **[BUG-01](中)半搬態 consumer 失依** → 治理原則「混合態不變量」。
- **[BUG-02](低)`base`/`cli`/`dispatch` 泛名撞外部依賴** → path-dep 優先,實作確認無同名外部 dep。
- **[BUG-03](中)刪除憑「編譯過」放行 parity bug** → 刪除硬 gate=fixture-parity 綠。

#### Performance

- **[PERF-01]** binary 啟動快無量測 → `/refining-plan` 加 TP:cold-start vs 腳本路徑(非阻斷)。

#### What's Good

- Motivation 一刀解清 scope 型問題(雙實作=納入)。
- 依賴鐵律 + engine 退役 + doctor 反轉 = 健康 DAG,無 god-crate。
- fixture-gated 刪除 + 混合態不變量。

#### 三審：task/test 粒度（2026-06-09,使用者要求最終檢查）

- **[GRAN-01] 初版 task 過粗** → 已重切:T-003(原綁 7 單元的架構解耦)拆為 base/platform/render/HealthCheck/providers/rename 六步逐步驗綠;adapters 由「四對一起」拆為逐腳本 T-013..T-017;install 家族 parity 逐 provider;R-06 六項雜項各自成 task。每個 commit-size 可獨立驗。
- **[GRAN-02] 完整性補洞** → 補:workspace `Cargo.toml` members 維護(T-003)、各拆分後 import 重指、HealthCheck 改為**各 domain 在自己 port task 內實作**(非最後集中)、混合態不變量抽查 TP-25、git filter 跨平台 TP-17、每次拆 crate「拆後綠」TP-04..TP-09。

#### 四審：真機驗收方法論修正（2026-06-09,使用者裁決 —— 根本性,非執行誤)

- **[CORR-01](阻斷級修正)真機驗收必須是純 end-user 安裝預編譯 artifact,絕不在測試機 build/裝工具鏈。** 初版計畫把「真機驗收」寫成「乾淨環境 `gal install`」卻沒指明 binary 從何而來,導致執行時誤入「在 mac-mini 裝 rust / 從 Windows 跨編譯」的歧路 —— **這違反「mac-mini = 純 end-user」的本質**。end-user 不 build,只裝 release artifact。
- **根本缺口**:計畫從未要求「產出跨平台 release artifact」。前計畫僅做 homebrew/winget *模板*,無實體 artifact、無 macOS CI。沒有 artifact,根本無法以 end-user 身分測 mac-mini。
- **修正(已折入)**:新增 **R-13(跨平台 release artifact 管線,CI macOS/Linux/Windows runner)** 為 R-10 前置;R-10 改寫為「裝 R-13 artifact 的純 end-user 驗收」;拆 **T-034(管線)/T-035(end-user 真機驗收)**;T-002 收斂為「開發機 dev 基準」(開發機有 rust 合理),與 end-user 真機驗收分離;TP-02/03 改為「裝預編譯 artifact」。
- **治理原則(新增,永久)**:**任何 GAL 真機/end-user 驗證,測試機一律純 end-user 安裝預編譯檔,絕不在其上裝 rust/工具鏈或 build。** 開發機(build host)與 end-user 機(裝 artifact)角色嚴格分離。

**Verdict 維持 APPROVE** —— 修正為加法式(補 R-13 前置 + 角色分離),不動既有架構 DAG;反而補上「真機驗收方法論」這個一直缺的前提。

<!-- ARCH_REVIEW: APPROVE -->

### Business Review
Not triggered（無 business-rule）。

### Design Review
Not triggered（無 customer-facing UI）。

### Engineering Review

**Verdict: CLEAR.**（四審 2026-06-09:粒度 + 真機驗收方法論修正後)**35 個 T-NNN** 對映 R-00..R-13 / P0..P8。每個 = 獨立可驗、commit-size 單元(R-00 解耦 7 步 T-003..T-009;adapters 逐腳本;install 逐 provider;R-06 雜項逐項)。**CORR-01 折入**:T-002 收斂為開發機 dev 基準;新增 T-034(release artifact 管線)/T-035(純 end-user 真機驗收,裝預編譯 artifact,前置 T-034)。27 條 TP(含 TP-26 管線產出、TP-02/03 改為 end-user 裝 artifact、TP-26b 開發機基準)。實作約束新增:**真機/end-user 驗證一律純 end-user 裝預編譯檔,絕不在測試機 build/裝工具鏈**;T-034 是 T-035 硬前置。architect 四審 APPROVE。

**~~注意:本次 deep-planning 改了 Tasks/Test Plan...→ 執行 prompt 已過時,須重跑 `/plan-to-prompt` 刷新後才續實作。~~ 已解決(2026-06-09):prompt 已刷新並執行 R-00(T-003..T-008)完成,228 test 綠。**

**實作期約束(prompt 與執行須遵守):**

1. **受保護核心簽核** — T-003(R-00 抽 base/providers/去前綴)、T-007(R-03 adapters + Sync-DevContext)、T-008(R-04 setup,推翻 Setup-Machine KEEP)觸及 `gal-engine` 核心 + 受保護路徑,實作前須 architect 簽核;CODER≠REVIEWER,reviewer tier ≥ implementer。
2. **刪除硬 gate = fixture-parity 綠** — 任何 `[DELETE]` 任務的刪除動作須先對齊 P0 凍結 fixture;`T-004`(oracle reparent)必須先於其覆蓋的刪除任務。
3. **混合態不變量** — 每個 phase 邊界系統完整可運作,不得讓 consumer 呼到已刪/半搬面;`common.*`/`gal.{ps1,sh}` 物理刪除(T-013 尾)掛跨計畫共同 gate(與姊妹計畫皆 done)。
4. **JIT 解耦** — T-003 只抽 base+providers+去前綴並驗 228 test 綠;domain crate(install/mcp/adapters/release/vcs/setup)於各自 port 任務起點才拆。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

> 切細對應細任務。每個拆 crate / port 任務都帶「拆後 `cargo test` 仍綠」檢查;每個 port 帶 fixture-parity;install 家族 parity 逐 provider。

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-01 | integration | fixture freeze:每核心 script 的 `tests/fixtures/` 可重現現況可觀察輸出/副作用(刪除 oracle) | T-001 |
| TP-02 | integration（硬 gate） | **mac-mini 純 end-user**:無 rust/無 repo,**安裝 T-034 預編譯 macOS-arm64 artifact**(brew/Releases/scp release 檔)→ packaged-source 自解析、`~/.claude/skills/gal` Unix symlink + doc-sync 載入、canonical root 正確、無孤兒、doctor green。**不在測試機 build** | T-035 |
| TP-03 | integration（硬 gate） | **Windows 純 end-user**:安裝 T-034 預編譯 artifact(normal mode,packaged-source + junction)→ canonical root + skill 面 + doctor green。(開發機 dev-mode 基準另由 T-002/TP-26b 覆蓋) | T-035 |
| TP-26 | integration | T-034 release 管線:CI 產出各 target(至少 macOS-arm64)預編譯 `gal` + 打包 source,可從 Releases/brew 取得安裝 | T-034 |
| TP-26b | integration | 開發機(Windows,有 rust)dev-mode `gal install` doctor green(本機基準) | T-002 |
| TP-04 | unit | 抽 `base` 後 228 test 綠;`base` 不依賴任何 GAL crate;workspace `Cargo.toml` members 正確 | T-003 |
| TP-05 | unit | `base::platform` 下沉後 render/install 路徑改用之,228 test 綠;Windows junction / Unix symlink / 權限行為不變 | T-004 |
| TP-06 | unit | `base::render` 下沉後既有 render 改用之,228 test 綠,輸出 byte 不變 | T-005 |
| TP-07 | unit | `HealthCheck` trait 在 `base`;既有 doctor 檢查遷移為 trait 實作,doctor exit 分級不變 | T-006 |
| TP-08 | unit | 抽 `providers` 後 228 test 綠;install 依 providers、無環 | T-007 |
| TP-09 | unit | rename `cli`/`dispatch` 後 228 test 綠、`gal --version` 正常、無同名外部 dep 衝突 | T-008 |
| TP-10 | unit | T-009 reparent 後 `cargo test` 不再 spawn `Test-ResolveGalCatalog`/`Test-InstallModeAuthority`/`test-install-acceptance`,改讀 fixture | T-009 |
| TP-11 | unit | `common.*` → `base` 各函式群 == fixture(install mode、provider 選擇、symlink、install state、plugin root) | T-010 |
| TP-12 | parity | `gal mcp` 四 provider MCP 配置輸出 == fixture(逐 provider 斷言) | T-011 |
| TP-13 | unit | `update-mcp.{ps1,sh}` 刪除後無 consumer 失依;混合態不變量(刪除前後系統可運作) | T-012 |
| TP-14 | parity | `adapters`:update-skills / update-commands / update-personalization / Sync-DevContext 各自生成 == fixture;與 install 共用 `base::render` 無重複實作 | T-013..T-016 |
| TP-15 | integration | `gal sync`(init-time)/`gal update`(增量)走 `adapters` 後端,生成 adapter 檔 == fixture | T-016 |
| TP-16 | parity | `gal setup` 編排 == fixture;`gal setup --tools` parity = 狀態探測分類(4 態 × 4 工具,mock 工具存在性)+ 構造命令 parity(mock executor,byte 比對)+ `--check` 輸出 parity;實網 npm/pip 不入 fixture(architect C-5) | T-018, T-019 |
| TP-17 | integration（跨平台） | git filter 註冊冪等、由 `gal setup` 執行、指向 .sh(bash-backed);Windows/Unix 行為不變。no-bash 條款延至 T-025 binary 切換(architect C-4) | T-020 |
| TP-18 | parity | install 家族逐 provider parity:Claude / Copilot / Codex / AGY 安裝編排 + render == fixture | T-022, T-023 |
| TP-19 | unit | install 家族刪除後四 provider 活讀取面仍對齊 source(doctor green) | T-024 |
| TP-20 | parity | `vcs`(clean/smudge/commit-msg)、`gal uninstall`(ledger 精確)、init-repo、catalog、release-packaging、translation 各 == fixture | T-025..T-030 |
| TP-21 | unit | 各 domain 實作 `HealthCheck`;`gal doctor` 聚合,對缺 mcp/setup/sync/filter 各報非零指出修復面,完整安裝 exit 0 | T-031 |
| TP-22 | integration | `gal` 入口核心子命令全走 Rust;`gal`/`gal-init` SKILL.template 引用改指 binary;名詞分組子命令可呼 | T-032 |
| TP-23 | manual | `scripts/` 核心家族零 ps1/sh(grep);`cargo test` 綠且測試碼不再 spawn 任何核心 live `scripts/*.{ps1,sh}` | T-033 |
| TP-24 | perf | binary cold-start vs 腳本路徑量測(驗證 Motivation #2,非 gate) | T-032 |
| TP-25 | integration | 混合態不變量抽查:在 P2/P4 中途狀態(部分 Rust 部分腳本)系統完整可運作,無 consumer 呼到已刪/半搬面 | T-011..T-030 |

**真機 end-user 驗證(T-035,前置 T-034 artifact)**:① T-034 CI 產出 macOS-arm64 預編譯 artifact → ② mac-mini 以 end-user 裝該 artifact(brew/Releases/scp;**無 rust、無 repo、無 build**),經 SSH 驗 packaged-source 自解析 + Unix symlink + doc-sync + doctor green → ③ Windows normal 裝 artifact 驗。Linux 有 host 時補。**核心修正:測試機一律純 end-user 安裝預編譯檔,絕不在其上裝工具鏈或 build。**

## Tasks

> 切割原則:**一個 task = 一個獨立可驗、commit-size 的單元**。每個拆 crate / port 任務以「拆後 `cargo test` 綠」收尾;每個 domain 在自己的 port task 內**就實作 `HealthCheck`**(不留到最後)。

**P0 — 基準**
- [x] **T-001（P0）** — fixtures 捕捉慣例已建(`tests/fixtures/README.md`);per-domain JIT 捕捉(approach A)。已 Rust 化面以現有 Rust 行為契約測試為 oracle。
- [x] **T-002（P0,本機 dev 基準）** — **開發機(Windows,有 rust)** dev-mode 重裝 + `gal doctor` exit 0(本機綠基線)。**真機 end-user 驗收移至 T-035(裝 R-13 artifact,非在測試機 build);本任務只負責開發機基準。**

**R-00 架構解耦（受保護,architect;逐步驗綠,JIT）**
- [x] **T-003** — 建 workspace 骨架 + 抽 `base` crate:移入 config/mode/paths/install-state/provider-selection;更新 root `Cargo.toml` members;repoint importers;`cargo test` 綠。*(1ffd3de;含架構師 BUG-01 折入:MCP 共用型別移入 `base::mcp`,先解 providers→mcp 環依賴)*
- [x] **T-004** — 下沉 `base::platform`(symlink/junction/perms/atomic-swap);render/install 改用;Windows/Unix 行為不變;綠。*(5c1514e;create_dir_link/remove_dir_link/is_symlink_or_junction/atomic_swap;render+claude+agy 委派,23 provider test 綠)*
- [x] **T-005** — 下沉 `base::render`(模板渲染原語);既有 render 改用、輸出 byte 不變;綠。*(8f78581;OE-01 圍欄:僅下沉 atomic-render staging 原語 create_temp_render_dir;scan/render_canonical_root/manifest 仍屬 install 域;無 template-substitution 原語、無 adapters 消費者故不過度抽象)*
- [x] **T-006** — 在 `base` 定義 `HealthCheck` trait;遷移既有 doctor 檢查為 trait 實作;exit 分級不變。*(ba5fc69;Severity/DoctorFinding/DoctorReport 移入 base::health;9 個 check → 各自 HealthCheck struct;run_doctor 以 Vec<Box<dyn HealthCheck>> 依原條件順序聚合,finding 順序/訊息/exit 分級 byte 不變)*
- [x] **T-007** — 抽 `providers` crate(Provider trait + claude/copilot/codex/agy 投影,自 gal-engine 移出);install 依 providers;綠、無環。*(c0f1f37;providers GAL-deps=[base] 唯一,無 providers↔engine 環——BUG-01 於 T-003 預解之效;gal-engine `pub use providers` 再匯出,install.rs 零改;23 provider test 綠)*
- [x] **T-008** — rename `gal-cli`→`cli`(`[[bin]] name="gal"`)、`gal-dispatch`→`dispatch`;更新所有 import + Cargo;`gal --version` 正常;228 test 綠。*(b190cf7;BUG-02:dispatch package/lib 去前綴但 bin 輸出名保留 `gal-dispatch`,gal.ps1 shim 不受影響;gal.exe + gal-dispatch.exe 皆產出;`gal --version`→gal 0.1.0)*
- [x] **T-009（R-08,先於所有刪除）** — reparent oracle 測試(`Test-ResolveGalCatalog`/`tests/Test-InstallModeAuthority`/`test-install-acceptance.sh`)為 fixture/行為測試。*(317d342)*

**R-01 共用核心**
- [x] **T-010（R-01）** — port `common.{ps1,sh}` 安裝/設定函式進 `base`(install mode/provider 選擇/symlink/install state/plugin root);install/setup/mcp/adapters 改用。*(23557b1)*

**R-02 MCP**
- [x] **T-011（R-02）** — 拆 `mcp` crate;port `update-mcp` → `gal mcp` 後端 + `HealthCheck`;四 provider parity 對齊 fixture。*(263dd77；orchestrator parity 修補 a600c97 — resolver/local-merge/非破壞寫入,全盤檢查後重開修正)*
- [x] **T-012** — 對齊 fixture 後刪 `update-mcp.{ps1,sh}`,驗無 consumer 失依。*(bda5fb7)*

**R-03 adapters（受保護,architect）**
- [x] **T-013（R-03）** — 拆 `adapters` crate + port `update-skills` → 後端(共用 `base::render`)+ `HealthCheck`;parity。*(T-013 closeout: `crates/adapters` 最小 crate 落地，`update-skills` parity/HealthCheck/reviewer 修正完成；workspace 測試綠)*
- [x] **T-014** — port `update-commands` → `adapters`;parity。
- [x] **T-015** — port `update-personalization` → `adapters`;parity。
- [x] **T-016** — port `Sync-DevContext`(init-time)→ `adapters`;接 `gal sync`/`gal update` CLI;parity。*(T-016 closeout: `crates/adapters` 新增 sync/machine-update orchestration，`crates/cli` 接上 `gal sync` 與 `gal update --machine-only`，`Init-Repo` / `Setup-Machine` caller scripts 改走 Rust binary；reviewer APPROVE，workspace 測試綠)*
- [x] **T-017** — 四對(update-skills/commands/personalization + Sync-DevContext)parity 綠後成對刪除。*(T-017 closeout: 刪除 `Sync-DevContext` / `update-skills` / `update-commands` / `update-personalization` 四對 legacy scripts；`.dev/project.md`、generated adapters 與 live docs 全面改以 `gal sync` / `gal update --machine-only` 為唯一指引；reviewer APPROVE，workspace 測試綠)*

**R-04 setup（受保護,architect;實作期簽核 APPROVE-with-conditions C-1..C-10,2026-06-10,詳見 prompt ## Review Results）**
- [x] **T-018（R-04）** — 拆 `setup` crate(只編排,無 domain 邏輯);port `setup-machine` → `gal setup`;parity。*(2c68fe2)*
- [x] **T-019** — port `setup-tools` → `gal setup --tools`;parity。*(b92b8b0)*
- [x] **T-020** — 註冊 git filter(冪等,由 `gal setup` 執行):`.gitattributes` + `git config filter.gal-config.* = bash scripts/gal-clean.sh|gal-smudge.sh` + `required=true`,指向現存 .sh(architect C-4);binary `gal clean/smudge` 切換 + 原子改註冊移至 T-025。bash-backed 行為不變;no-bash 條款延至 T-025 驗。*(708ca94)*
- [x] **T-021** — `setup-machine.{ps1,sh}` + `setup-tools.{ps1,sh}` parity 綠後成對刪除。*(41ff61a；Uninstall-Machine 同 commit 改接 `gal setup --uninstall`)*

**R-05 安裝家族**
- [x] **T-022（R-05）** — 確認 `install-gal-plugins` 四 provider 安裝編排 parity(逐 provider 對齊 fixture:Claude/Copilot/Codex/AGY)。*(closeout: `gal install` 直接寫入四 provider lifecycle ledger / marketplace / projection root；聚焦測試 `provider_family_r05` 綠)*
- [x] **T-023** — 確認 `build-core-plugin`/`build-provider-plugins`/`provider-plugin` render parity。*(closeout: Rust canonical render 補齊 `.codex-plugin/plugin.json` 與 provider-neutral install artifacts，build-family 行為收斂進 `gal install` / render path)*
- [x] **T-024** — 全 parity 綠後刪 install 家族(含內部專屬腳本);驗活讀取面仍對齊。*(closeout: 刪除 `Install-GalPlugins.*` / `Build-CorePlugin.*` / `Build-ProviderPlugins.*` / `common/ProviderPlugin.*`，`gal setup` 與 `adapters` caller 改走 Rust，live docs 對齊)*

**R-06 雜項（逐項 port→parity→刪）**
- [x] **T-025** — `vcs`:`gal clean`/`gal smudge` + commit-msg;parity;刪 `gal-clean.sh`/`gal-smudge.sh`。*(closeout: Rust filter engine + CLI cutover landed; setup/adapters git config now registers `gal clean` / `gal smudge`; shell filter pair deleted after focused tests passed)*
- [ ] **T-026** — `gal uninstall` parity(ledger 精確);刪 `uninstall-machine.{ps1,sh}`。
- [ ] **T-027** — port `init-repo` → Rust;parity;刪對。
- [ ] **T-028** — port catalog 解析(`Resolve-GalCatalog`)→ Rust;parity;刪。
- [ ] **T-029** — release packaging 併 `gal release`;parity;刪 `Package-ReleaseArtifacts.{ps1,sh}`。
- [ ] **T-030** — port translation freshness → Rust;parity;刪對。

**R-11/R-07/R-10 收尾**
- [ ] **T-031（R-11）** — `cli` 聚合各 domain `HealthCheck` 為 `gal doctor`,擴及 mcp/setup/sync/filter,缺口 fail-loud。
- [ ] **T-032（R-07）** — port `gal.{ps1,sh}` 核心子命令進 `cli`(名詞分組);改 `gal`/`gal-init` SKILL.template 引用指 binary。入口檔物理刪除掛跨計畫尾端。
- [ ] **T-033（R-09/R-12,硬 gate）** — `scripts/` 核心家族清空(共用 `common.*`/`gal.{ps1,sh}` 與姊妹計畫共同尾端刪);`cargo test` 綠且測試碼不再 spawn 核心 live script(grep 驗)。

**R-13/R-10 純 end-user 真機驗收（修正:絕不在測試機 build）**
- [ ] **T-034（R-13,T-035 前置）** — 建跨平台 release artifact 管線:CI(macOS/Linux/Windows runner,如 GitHub Actions matrix)產出各 target 預編譯 `gal` + 打包 source(FHS/flat),發佈 GitHub Releases / brew tap。`gal release` 接線到 CI,不只產當前平台。
- [ ] **T-035（R-10,硬 gate;前置=T-034)** — **純 end-user 真機驗收**:在 mac-mini(macOS-arm64,無 rust/無 repo)與 Windows(normal)以 **end-user 身分安裝 T-034 的預編譯 artifact**(brew/Releases/scp release 檔),驗 packaged-source 自解析、Unix symlink/junction skill 面、doc-sync 載入、canonical root 完整、無孤兒、`gal doctor` green。**絕不在測試機裝 rust/build/跨編譯。**
