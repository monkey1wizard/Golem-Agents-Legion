# Plan: Plugin Bin Exposure (Downstream of Rust Bootstrap)

## Goal

讓 GAL 的 Claude-compatible plugin root 提供 Claude Code 官方 `bin/` 行為：外掛啟用時，agent 的 Bash tool 可直接呼叫 `gal` 公開 entrypoint，不需前綴 `bash`/`pwsh`。

本計畫現在是 `docs/plans/fix-gal-bootstrap-install-convergence.md` 的**下游包裝計畫**。Rust workspace、`gal` bootstrap CLI、`install/update/doctor`、package-manager artifacts、legacy script contract、以及「用 Rust 逐步取代舊功能」的整體策略都由 bootstrap/install convergence plan 擁有。本計畫只消費該 plan 產出的 `gal` binary，將它放進 plugin root 的 `bin/`，並驗證 Claude Code 啟用外掛後 PATH 可找到它。

**分期與「零依賴」語義**：本計畫不宣稱交付零執行期依賴。若 upstream `gal` binary 仍委派 scripts，plugin `bin/` exposure 只表示使用者不需鍵入 `bash`/`pwsh` 前綴。真正零依賴由 Rust bootstrap / deterministic-engine migration 的後續計畫達成。

## Decisions (Locked)

| # | 決策 | 結論 | 日期 |
| --- | --- | --- | --- |
| PB-001 | Rust ownership | `fix-gal-bootstrap-install-convergence.md` owns `crates/gal-cli`, `crates/gal-engine`, `gal install/update/doctor`, package-manager artifacts, and script delegation | 2026-06-02 |
| PB-002 | 本計畫範圍 | 只把 upstream 產出的本機 OS `gal` binary 放進 plugin root `bin/`，並驗證 Claude Code PATH 行為 | 2026-06-02 |
| PB-003 | Shell wrapper | 不新增 `bin/gal.sh` 或 `bin/gal.ps1`; plugin `bin/` 只放原生 executable | 2026-06-02 |
| PB-004 | Renderer 行為 | Renderer 只複製已存在的 upstream binary。若 binary source 不存在須 fail-loud，不自行建立 Rust workspace 或重寫 install 編排 | 2026-06-02 |
| PB-005 | Deterministic engines | doc-sync、plan/state converge、preflight、working-hours、headless/xmachine executor unification 皆移出本計畫，歸 bootstrap convergence 或後續專項 plan | 2026-06-02 |

## References

- Upstream bootstrap/install convergence: [fix-gal-bootstrap-install-convergence.md](fix-gal-bootstrap-install-convergence.md) owns Rust workspace creation, public `gal` binary behavior, package-manager install, provider projection convergence, and script retirement strategy. This plan must not duplicate that scope.
- Historical stabilization reference: [fix-install-ownership-stabilization.md](fix-install-ownership-stabilization.md) claimed idempotent plugin render stabilization, but later runtime-surface audit found convergence gaps. Treat it as context, not as sufficient proof.
- [Claude Code plugins](https://code.claude.com/docs/en/plugins) - plugin root `bin/` 中的 executables 在外掛啟用時加入 Bash tool 的 PATH。
- [Claude Code plugins-reference](https://code.claude.com/docs/en/plugins-reference) - 官方規格實測（2026-06）：`bin/` 為固定慣例資料夾「Executables added to the Bash tool's PATH … invokable as bare commands while the plugin is enabled」。**`plugin.json` manifest schema 無 `bin`/`platform`/`arch`/`executable` 欄位，亦無任何跨 OS 二進位選擇機制**——多平台分發由發佈者自理。版本以 `plugin.json:version` 或（省略時）git commit SHA 解析。`${CLAUDE_PLUGIN_ROOT}` 為安裝目錄絕對路徑、更新後變動、不可寫狀態。
- [docs/devguide.md](../devguide.md) - Claude plugin root 目前是 `~/.gal/plugins/gal/`。
- [conventions/token-budget.md](../../conventions/token-budget.md) - Rust `target/` and build outputs remain excluded from routine context; plugin `bin/` is an intentional rendered artifact.

## Requirements

- [ ] 定義 renderer 可消費的 upstream binary source contract：Windows 為 `gal.exe`，Unix 為 executable `gal`。此 binary 由 [fix-gal-bootstrap-install-convergence.md](fix-gal-bootstrap-install-convergence.md) 產出或安裝。
- [ ] `Build-CorePlugin.*` render plugin root 時建立 `bin/`，只複製本機 OS 的 upstream `gal` executable，不執行 `cargo build`，不建立 Rust workspace，不改 install/update/provider 編排語義。
- [ ] 若 upstream binary source 缺失或不可執行，renderer 必須 fail-loud，不得產出缺 `bin/gal` / `bin/gal.exe` 的半成品 plugin。
- [ ] plugin root 不包含 `bin/gal.sh` 或 `bin/gal.ps1` shell forwarding layer。
- [ ] 更新 docs 與 command contracts，說明 Claude plugin `bin/` exposure 是下游包裝。Rust bootstrap/install/package-manager 行為由 convergence plan 擁有。

## Approach

### Step 1: Resolve Upstream Binary Contract

- **Files**: `docs/plans/fix-gal-bootstrap-install-convergence.md`, `docs/devguide.md`
- **What**: 明確記錄 plugin renderer 從哪裡取得 upstream `gal` binary，例如 source checkout build output、installed package-manager binary、或 release artifact cache。此計畫不產出該 binary，只驗證 contract 是否存在且可執行。
- **Verify**: binary source missing 時 renderer 能 fail-loud。binary source present 時能讀取 version / basic status command。

### Step 2: Render Binary into the Plugin Artifact

- **Files**: `scripts/Build-CorePlugin.ps1`、`scripts/build-core-plugin.sh`
- **What**: GAL renderer 產出 plugin root 時建立 `bin/`，複製本機 OS 對應 binary：Unix 為無副檔名 `gal`（+x），Windows 為 `gal.exe`。`bin/` 依官方規格不需在 `plugin.json` 宣告。不做 cross-compile、Cargo workspace 初始化、package-manager artifact 生成或 provider 編排改寫。
- **Verify**: 本機 render 出的 plugin 資料夾含對應本機 OS 的原生執行檔。缺 upstream binary 時清楚失敗。

### Step 3: Update Documentation & Command Contracts

- **Files**: `scripts/scripts.md`、`README.md`、`README.zh-Hant.md`、`docs/devguide.md`、`commands/gal/SKILL.template.md` 等
- **What**: 說明 Claude plugin enabled context 中可直接呼叫 `gal`，但 `gal` binary 本身的 install/update/doctor/package-manager 行為由 upstream convergence plan 定義。
- **Verify**: 搜尋文件確認本計畫沒有再教導建立 Rust workspace 或實作 bootstrap CLI。

### Step 4: Verify Claude Plugin PATH Behavior

- **Files**: `scripts/Test-BuildProviderPlugins.ps1`, Claude plugin validation docs/tests if present
- **What**: 啟用 GAL Claude plugin 後，在 Claude Code Bash tool 環境直接執行 `gal --version` 或 `gal status`，確認 PATH 解析到 plugin root `bin/` 中的 native executable。
- **Verify**: Claude Code plugin-enabled shell 可找到 `gal`，且輸出與 upstream binary 一致。

## Delegated Rust Work

以下 Rust 工作已移出本計畫，避免與 [fix-gal-bootstrap-install-convergence.md](fix-gal-bootstrap-install-convergence.md) 重疊：

- `Cargo.toml`, `crates/gal-cli/`, `crates/gal-engine/` workspace 建立。
- `gal --version`, `gal install`, `gal update`, `gal doctor`, `gal uninstall`, `dispatch-script` 等 bootstrap CLI 行為。
- GitHub Releases / winget / Homebrew package-manager payload。
- Rust → legacy scripts invocation contract and exit-code taxonomy。
- `gal-engine` deterministic engines：doc-sync、plan/state convergence、preflight、working-hours、headless/xmachine executor unification。
- 任何 install/update/provider projection 編排改寫。

本計畫只在 upstream binary 已存在後，處理 plugin artifact 的 `bin/` exposure 與 Claude Code PATH 驗證。

## Files to Create or Modify

- `[MODIFY]` `scripts/Build-CorePlugin.ps1`, `scripts/build-core-plugin.sh` — copy upstream native `gal` binary into plugin root `bin/`; fail-loud when source binary is missing.
- `[MODIFY]` `scripts/Test-BuildProviderPlugins.ps1` or equivalent provider tests — assert plugin `bin/` contains only the native executable for the local OS.
- `[MODIFY]` `README.md`, `docs/devguide.md`, `docs/manual.md`, `scripts/scripts.md` — document plugin `bin/` exposure as downstream of bootstrap convergence.
- `[MODIFY]` `commands/gal/SKILL.template.md` and relevant generated-contract templates — describe `gal` as available in Claude plugin-enabled PATH once upstream binary is installed/rendered.
- `[MODIFY]` `docs/plans/fix-gal-bootstrap-install-convergence.md` only if the upstream binary source contract needs a tighter cross-link.

## Test Cases

- [ ] TP-001 - Upstream binary source present：renderer resolves the local OS `gal` binary source defined by [fix-gal-bootstrap-install-convergence.md](fix-gal-bootstrap-install-convergence.md) and verifies it is executable.
- [ ] TP-002 - Upstream binary source missing：`Build-CorePlugin.*` fails loudly and does not produce a plugin root missing `bin/gal` or `bin/gal.exe`.
- [ ] TP-003 - Windows render：plugin root contains `bin/gal.exe` and does not contain `bin/gal.ps1` or `bin/gal.sh`.
- [ ] TP-004 - Unix render：plugin root contains executable `bin/gal` and does not contain `bin/gal.ps1` or `bin/gal.sh`.
- [ ] TP-005 - Claude Code PATH smoke：with GAL plugin enabled, Bash tool can run bare `gal --version` or `gal status` and resolves the plugin root binary.
- [ ] TP-006 - Documentation grep：this plan no longer claims ownership of Rust workspace creation, package-manager install, `gal install/update/doctor`, or deterministic engine migration.

## Success Criteria

- [ ] Claude Code 外掛啟用後能直接以 `gal` 操作，不需前綴 `bash`/`pwsh`。
- [ ] Plugin root `bin/` 只含本機 OS native `gal` executable，不含 shell forwarding scripts。
- [ ] Renderer never produces a partial plugin artifact when upstream binary is missing.
- [ ] Rust bootstrap/install/update/doctor/package-manager ownership remains solely in [fix-gal-bootstrap-install-convergence.md](fix-gal-bootstrap-install-convergence.md).

## Risks

- **Upstream binary contract drift**：bootstrap convergence may choose a binary cache/release layout different from this plan's renderer expectation. Mitigation: lock the binary source contract before `/refining-plan` and fail-loud on mismatch.
- **Partial plugin artifact**：renderer could copy other plugin files but miss `bin/gal`, creating another false-ready install surface. Mitigation: `Build-CorePlugin.*` must abort before publishing when the binary is unavailable or not executable.
- **Scope creep back into Rust bootstrap**：implementation could reintroduce cargo build/workspace/CLI behavior here. Mitigation: this plan is downstream-only; any Rust bootstrap change returns to [fix-gal-bootstrap-install-convergence.md](fix-gal-bootstrap-install-convergence.md).
- **Claude PATH smoke availability**：real Claude Code plugin PATH behavior may require an installed/enabled plugin state that is hard to automate. Mitigation: include a manual smoke gate and record environment-unverifiable status explicitly rather than marking PASS.

## Open Questions

- [ ] OQ-001 - Upstream binary source contract：renderer 應從 package-manager installed `gal`、source checkout build output、release artifact cache，或 bootstrap convergence 定義的其他位置取得 binary？此問題必須在 `/refining-plan` 前由 [fix-gal-bootstrap-install-convergence.md](fix-gal-bootstrap-install-convergence.md) 鎖定。

## Approval

- Human approval: [pending]
- Architect review: [refresh required — prior APPROVE applied to superseded broader Rust wrapper / native-engine scope; current plan is narrower and downstream-only]
- Additional domain review: [not triggered]（無 customer-facing / business-rule 內容）

## Review Results

### Architecture Review

Pending refresh. Prior architecture review is preserved in git history but no longer applies cleanly after this plan was rebased to downstream plugin `bin/` exposure and Rust bootstrap/native-engine scope was moved to [fix-gal-bootstrap-install-convergence.md](fix-gal-bootstrap-install-convergence.md).
