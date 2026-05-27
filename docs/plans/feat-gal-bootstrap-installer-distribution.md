# 計畫：GAL Bootstrap Installer 與跨平台發布

## Goal

GAL 提供一條清楚且可管理的跨平台安裝與發現路徑：Windows 使用者可透過 `winget` 安裝，macOS 與 Linux 使用者可透過 `homebrew` 安裝；GitHub Releases 同時提供版本化 `zip` / `tar.gz` 作為手動 fallback，且其中封裝的仍是相同版本的單一可執行 binary。除此之外，一般使用者也應能在 Claude marketplace、Codex marketplace、Copilot marketplace 等 provider-native marketplace 中搜尋並找到 GAL，並在 provider-native lifecycle 支援時直接完成安裝或更新；若該 marketplace 只支援 discoverability，entry 也必須清楚導向 `winget`、`homebrew` 或 GitHub Releases。package manager 只負責 bootstrap payload 的安裝、升級與移除，GAL 自身仍擁有 `~/.gal/` runtime home、generated projections 與 machine-local state 的生命週期。

## Requirements

- [ ] 定義 GAL 對外的 canonical distribution model：每個支援平台以單一可執行 binary 作為 canonical bootstrap payload；GitHub Releases 為版本化 artifacts 的 source of truth，`winget` 與 `homebrew formula` 只消費同一條 binary lineage，`.zip` / `.tar.gz` 只是給手動安裝者下載相同版本 binary 的封裝，不直接管理 `~/.gal/` 內容。
- [ ] Windows 安裝路徑必須支援 `winget install`；macOS 與 Linux 安裝路徑必須支援 `brew install`。
- [ ] GitHub Releases 必須提供手動 fallback artifacts，至少包含 `.zip` 與 `.tar.gz`，讓無法使用 package manager 的環境仍可下載並自行安裝相同版本的單一可執行 binary。
- [ ] 一般使用者除 package manager 與 release archive 外，也必須能在 Claude marketplace、Codex marketplace、Copilot marketplace 等 provider-native marketplace 搜尋並找到 GAL 的 plugin/package entry；在 provider-native lifecycle 支援 install/update 的 lane 上，entry 也必須能直接完成對 canonical package lineage 的 install/update；若該 provider 只支援 discoverability，entry 必須清楚導向官方 install channels。
- [ ] 官方 support policy 必須明確區分 supported install channels 與 discoverability channels：`winget`、`homebrew`、GitHub Releases archives 為官方支援的安裝/升級路徑；provider-native marketplaces 預設為官方 discoverability surface，除非個別 provider 已被明確驗證可直接完成 canonical install/update。
- [ ] 明確區分 bootstrap installer 與 install mode plugin distribution：bootstrap installer 解決「如何把 GAL 放到機器上」，install mode plan 解決「GAL 安裝後如何管理 provider-native plugin artifacts 與 `~/.gal/` state」。
- [ ] 明確定義 GitHub Releases artifacts、package-manager manifests 與 provider marketplace submission artifacts 之間的關係：三者必須共享同一條 canonical release version lineage；允許 provider-required manifest wrapper、審核延遲或 metadata 差異，但不得形成獨立版本流。
- [ ] bootstrap payload 必須可安全升級與移除，但不得把 package manager 或 marketplace 的 ownership 擴張到 `~/.gal/` runtime state、使用者設定或 provider-generated outputs；預設 uninstall 只保留 user-owned config 與明確 user-authored local overrides，其餘 GAL-managed runtime/generated artifacts 應移除。
- [ ] 明確定義 bootstrap payload 的 install boundary、runtime ownership boundary、upgrade boundary、uninstall boundary 與 purge/reset boundary。
- [ ] 文件與 marketplace copy 不得承諾 GitHub Releases、`winget`、`homebrew`、provider marketplaces 永遠同步上架；必須以 GitHub Releases 為 canonical version source，並說明下游 channel 可能因審核或 publish latency 而延後收斂。
- [ ] raw shell installers（例如 `irm ... | iex`、`curl ... | sh`）只可作為 convenience/bootstrap fallback，不得成為唯一或 canonical distribution path。
- [ ] 設計 release artifact matrix，說明 Windows、macOS、Linux 各自對應的 package-manager manifest 或 Homebrew formula，以及封裝相同單一可執行 binary 的手動下載資產。
- [ ] 設計 marketplace publication matrix，說明 Claude、Codex、Copilot 等 provider 的 marketplace entry、submission prerequisites、discoverability constraints 與更新來源。
- [ ] 規劃版本發布流程，確保 GitHub Releases、`winget` manifest、`homebrew` formula 與 marketplace entries 都對齊同一 canonical release version lineage，並定義 downstream lag 的可接受範圍、驗證方式與 fallback 行為。
- [ ] 規劃版本發布流程時，必須一併定義 marketplace entry 的版本同步、審核/發布節奏、discoverability-only lane 的導流文案與 direct-install lane 的驗證規則，避免 marketplace 顯示版本與 bootstrap channel 漂移。
- [ ] 明確定義 bootstrap install 後的入口行為：如何建立可執行 `gal` CLI、如何初始化或更新 `~/.gal/`、何時觸發 install mode/source mode 分流。
- [ ] 明確記錄 package-managed files、GAL-managed runtime/generated files、user-owned config/state 之間的責任邊界，避免升級或移除時誤刪使用者資料，並避免 uninstall 後殘留第二套未受管 runtime。
- [ ] 明確定義使用者在 installer 完成後應如何備份、轉移與重建 GAL 設定：README 必須區分 `~/.gal/config/config.json`、`~/.gal/state/plugins.lock.json`、`~/.gal/config/xmachine.json`、明確 user-authored local overrides 與 secrets 來源等「應備份項目」，以及 package-managed bootstrap payload、provider plugin install tree、`~/.gal/generated/` projections 等「可重裝或可重建項目」。
- [ ] 官方支援承諾必須明確區分 canonical version lineage 與 downstream publish latency；不承諾所有 channel same-day parity，但必須保證使用者看得出哪個 channel 是最新 canonical release 及其 fallback。
- [ ] uninstall policy 預設必須保留 `~/.gal/` 下的 user-owned config 與明確 user-authored local overrides；package-manager uninstall 不得隱含保留或重建第二套 GAL-managed runtime/generated artifacts。若需要 destructive cleanup，必須設計成明確的 purge/reset flow。
- [ ] release governance 必須定義 downstream drift policy：GitHub Releases 先行、`winget` / `homebrew` 收斂、provider marketplaces best-effort 跟進；並提供 channel 落後時的官方 fallback 文案。
- [ ] 文件必須清楚說明官方安裝路徑、fallback 路徑、適用平台、限制與不保證事項。

## Approach

### Step 1: 定義 distribution architecture 與 ownership boundary

- **Files**: `docs/plans/feat-gal-bootstrap-installer-distribution.md`, `docs/devguide.md`, `docs/personalization.md`
- **What**: 定義 GitHub Releases、`winget`、`homebrew`、provider-native marketplace、手動下載 artifacts 與 `~/.gal/` runtime home 之間的責任切分，明確說明 bootstrap installer 與 install mode plugin distribution 是兩個相鄰但分離的架構面。
- **Verify**: 文件能清楚回答「誰負責安裝 CLI」「誰擁有 `~/.gal/`」「移除 package manager package 時哪些內容應保留」。

### Step 2: 定義 release artifact 與 package-manager ingestion model

- **Files**: `scripts/`, `README.md`, release/packaging docs to be determined
- **What**: 設計 GitHub Releases 應產出的 artifacts 組合：每平台的單一可執行 binary、package-manager ingestion 所需 metadata，以及封裝相同 binary 的手動 fallback `.zip` / `.tar.gz`；決定 `winget` 與 `homebrew formula` 應消費同一條 bootstrap binary lineage，並建立 channel/version lineage matrix，明確定義 marketplace submission artifact 是否只是 canonical release 的 wrapper。
- **Verify**: 能列出每個平台的 canonical install command、對應 single-binary release asset、checksum/provenance requirements、對應 marketplace entry、允許的 wrapper 差異、升級來源與 lag/fallback policy。

### Step 3: 規劃 bootstrap runtime contract

- **Files**: `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh`, `scripts/gal.ps1`, `scripts/gal.sh`, related install docs
- **What**: 定義 bootstrap install 後 CLI 入口如何定位 GAL runtime、如何初始化 `~/.gal/`、如何與 install mode/source mode contract 接軌，並避免 package manager 直接寫入 repo-local workflow state。
- **Verify**: bootstrap contract 可解釋首次安裝、重裝、升級與 machine migration 的行為，不依賴 clone repo 才能正常運作。

### Step 4: 規劃 upgrade, uninstall, 與 fallback UX

- **Files**: install/uninstall docs to be determined, `README.md`, `docs/devguide.md`
- **What**: 定義 `winget upgrade`, `brew upgrade`, marketplace entry 更新、手動 archive 更新、rollback 與 uninstall 的預期行為；明確規定預設 uninstall 保留 user-owned config，但移除 package-managed bootstrap payload 與 GAL-managed runtime/generated artifacts；完整 purge/reset 需另有顯式破壞性流程；補上 raw script convenience path 的安全界線與適用情境。同時定義 installer 完成後新版 `README.md` 必須如何指導使用者備份與轉移：哪些 `~/.gal/` 檔案需要帶走、哪些 generated/provider-managed artifacts 不需要備份、在新機器上應先重裝 bootstrap 再還原設定並重建 runtime/projections。
- **Verify**: 每種安裝與發現路徑都能回答如何升級、如何移除、哪些 `~/.gal/` 內容保留、哪些內容刪除、何時需要 explicit purge，以及如何回到手動 fallback。README 也必須能清楚回答「我要備份哪些檔案」「換機時先還原什麼」「哪些內容會由 installer/setup 自動重建」。

### Step 5: 驗證 Claude marketplace 作為安裝基準

- **Files**: `docs/plans/feat-gal-bootstrap-installer-distribution.md`, `docs/plans/feat-gal-install-mode-plugin-distribution.md`, `README.md`, marketplace docs to be determined
- **What**: 基於 agy-cli 已確認 gal plugin 依照 data struct 正確寫入，必須先驗證 Claude 作為基準：確保 Claude 能正確安裝 GAL plugin。定義 Claude marketplace 的 entry strategy、版本同步、installability classification，確保在 provider-native lifecycle 中直接安裝無誤。
- **Verify**: Claude 可正確從 marketplace 執行 GAL plugin 的 install/update，驗證基準無誤。

### Step 6: 擴展至其他 AI Provider Marketplaces (Codex, Copilot)

- **Files**: `docs/plans/feat-gal-bootstrap-installer-distribution.md`, `docs/plans/feat-gal-install-mode-plugin-distribution.md`, `README.md`, marketplace docs to be determined
- **What**: Claude 基準完成後，定義 Codex、Copilot 等其他 provider-native marketplace 的 entry strategy、搜尋名稱、顯示 metadata、版本同步、installability classification 與審核/發布節奏。若不支援 direct install，明確導向 GitHub Releases 或 package manager。
- **Verify**: 文件清楚規範各 marketplace 的搜尋、版本對齊、lag 導流與 direct install 規則。

## Files to Create or Modify

- `docs/plans/feat-gal-bootstrap-installer-distribution.md` — 新的 source plan，定義 bootstrap installer 與跨平台發布策略
- `.dev/state.md` — 將新 plan 登記為 active plan，供後續 `/deep-planning` 或 `/refining-plan` 使用
- `README.md` — 後續同步官方安裝方式、平台矩陣，以及 installer 完成後的備份/轉移設定指南
- `docs/plans/feat-gal-install-mode-plugin-distribution.md` — 後續對齊 provider-native marketplace artifact 與 install-mode canonical package 的關係
- `docs/devguide.md` — 後續補充 runtime ownership、install boundary 與 release flow
- `docs/personalization.md` — 後續補充 machine-local `~/.gal/` 與 bootstrap install 的責任分界
- `scripts/Setup-Machine.ps1` — 後續實作 Windows bootstrap/install flow
- `scripts/setup-machine.sh` — 後續實作 macOS/Linux bootstrap/install flow

## Test Cases

- [ ] Windows 使用者可透過 `winget` 安裝 GAL bootstrap payload，並在不 clone repo 的前提下取得可用 `gal` CLI。
- [ ] macOS 使用者可透過 `brew install` 安裝 GAL bootstrap payload，並正確建立或更新 `~/.gal/` runtime home。
- [ ] Linux 使用者可透過 `brew install` 安裝或用 GitHub Releases `.tar.gz` 手動安裝相同版本的單一可執行 binary。
- [ ] 無 package manager 的環境可用 GitHub Releases `.zip` / `.tar.gz` 下載並完成相同版本單一可執行 binary 的手動安裝。
- [ ] 使用者可在 Claude marketplace 搜尋 GAL 並找到對應 entry；若 Claude lifecycle 支援 direct install/update，該 entry 可直接操作 canonical package lineage；若否，entry 會清楚導向官方 install channel。
- [ ] 使用者可在 Codex marketplace 搜尋 GAL 並找到對應 entry；entry metadata 會揭露其對應 release 版本、是否可 direct install/update，以及 lag 時的 fallback。
- [ ] 使用者可在 Copilot marketplace 搜尋 GAL 並找到對應 entry；該 entry 對應 canonical version lineage，且不依賴與官方 plugin lifecycle 不相容的額外安裝步驟。
- [ ] `winget upgrade` 與 `brew upgrade` 不會覆寫或刪除使用者既有 `~/.gal/` config/state。
- [ ] uninstall package 時，package-managed payload 與 GAL-managed runtime/generated artifacts 會被移除，但 user-owned config 與明確 local overrides 仍保留，且與文件宣告一致。
- [ ] installer 完成後的 README 可清楚指導使用者備份與轉移：至少說明應備份 `~/.gal/config/config.json`、`~/.gal/state/plugins.lock.json`、`~/.gal/config/xmachine.json` 與必要的 local overrides / secrets 來源，並說明 package-managed bootstrap payload、provider plugin install tree 與 `~/.gal/generated/` 內容可於新機器重裝後重建。
- [ ] 當 marketplace、`winget`、`homebrew` 存在發布延遲時，使用者仍可從文件或 entry metadata 看出最新 canonical release 版本與官方 fallback。
- [ ] raw script convenience installer 能把使用者導向與 release artifacts 相容的 canonical payload，而不是額外創造第二套 distribution model。

## Success Criteria

- [ ] GAL 的官方安裝與發現模型可濃縮成一句話：`winget` / `homebrew formula` 為 managed install，GitHub Releases `.zip` / `.tar.gz` 只是封裝相同單一可執行 binary 的手動 fallback，而 Claude/Codex/Copilot marketplace 提供 provider-native discoverability，並在 provider-native lifecycle 支援時提供直接 install/update。
- [ ] bootstrap installer 與 install-mode plugin distribution 的邊界在文件與實作規劃中都清楚，不再混用。
- [ ] package manager ownership 與 `~/.gal/` ownership 清楚分離，升級與移除行為可預測。
- [ ] 預設 uninstall 只保留 user-owned config 與明確 local overrides；package-managed payload 與 GAL-managed runtime/generated artifacts 會被安全移除，完整 purge 另有顯式流程。
- [ ] installer 完成後的 README 能讓使用者在不備份整套安裝產物的前提下，正確完成 GAL 設定備份與換機重建：只帶走 user-owned config/state，重新安裝 bootstrap，再重建 provider-managed 與 generated artifacts。
- [ ] Windows、macOS、Linux 三條安裝路徑與各 provider marketplace entry 都對應到同一套 canonical release lineage，而不是多套彼此分離的版本治理流程。

## Risks

- `winget`、`homebrew` 與 GitHub Releases 的 artifact/metadata 若不同步，會造成版本漂移與支援成本上升。
- provider marketplace 的審核、metadata 規則或 discoverability policy 若與 release 流程脫節，會造成「能安裝但找不到」或「找得到但版本落後」的使用者體驗斷裂；因此必須把 lag policy 與 fallback message 視為正式 product contract。
- 若 bootstrap payload 與 install-mode runtime contract 切分不清，後續實作容易重複建立第二套 state/layout。
- 若 uninstall boundary 定義不清，可能誤刪 `~/.gal/` 中的 user-owned config，或在保留 config 的同時殘留第二套未受管 runtime/generated artifacts。
- 若一開始同時追求單一 binary、archive fallback、marketplace discoverability 與多 package-manager ingestion，scope 可能過大，需在 `/deep-planning` 收斂。
- 若對 provider marketplace 做出「可直接安裝且立即更新」的官方承諾，但實際仍受 provider review / listing policy 限制，會快速累積 support tickets 與信任損耗。
- 若沒有明確的 drift policy 與對外 fallback 文案，GitHub Releases、`winget`、`homebrew`、marketplace 顯示版本不一致時，使用者將無法判斷哪個 channel 才是官方最新版本。

## Open Questions

- [x] OQ-001 — bootstrap payload 的 canonical 形態定為每平台的單一可執行 binary；GitHub Releases 的 `.zip` / `.tar.gz` 只是給偏好自行安裝管理者下載相同版本 binary 的封裝，而不是第二套 payload。 *(raised by: planning, resolved by: user decision)*
- [x] OQ-002 — `homebrew` 採 `formula`；目前不採 `cask` 或混合策略。 *(raised by: planning, resolved by: user decision)*
- [x] OQ-003 — uninstall 預設保留 user-owned config 與明確 user-authored local overrides，移除 package-managed bootstrap payload 與 GAL-managed runtime/generated artifacts；完整 purge 另行提供 explicit reset flow。 *(raised by: planning, resolved by: deep-planning)*
- [x] OQ-004 — Claude、Codex、Copilot marketplace 的 entry 可各自有 provider-required manifest wrapper 與審核節奏，但都必須對應到同一條 canonical release version lineage，不得形成獨立版本流。 *(raised by: planning, resolved by: deep-planning)*
- [x] OQ-005 — 官方承諾包含 marketplace 可搜尋，且在 provider-native lifecycle 支援時可直接從 marketplace 完成安裝/更新；若該 provider 僅支援 discoverability，marketplace entry 必須導向官方 install channels。 *(raised by: planning, resolved by: deep-planning)*
<!-- Format: - [ ] OQ-NNN — description *(raised by: command)* -->
<!-- Resolved: - [x] OQ-NNN — description *(raised by: command, resolved by: engineering-review-lane)* -->
## Approval

- Human approval: pending
- Architect review: clear for `/refining-plan` after deep-planning revisions on 2026-05-25
- Additional domain review: business review completed; design not requested

## Review Results

### Architecture Review

Verdict: clear after revision.

- GitHub Releases 是 canonical version source；`winget`、`homebrew` 與 provider marketplace entries 必須共享同一條 canonical release lineage。provider-required wrapper 與審核延遲可接受，但不得形成獨立版本流。
- provider marketplace 的官方承諾必須拆成兩層：所有支援 lane 都必須可搜尋；只有 provider-native lifecycle 已驗證支援 install/update 的 lane，才承諾可直接從 marketplace 完成 install/update。其餘 lane 必須明確導向 `winget`、`homebrew` 或 GitHub Releases。
- uninstall boundary 已收斂為：預設保留 user-owned config 與明確 local overrides，移除 package-managed bootstrap payload 與 GAL-managed runtime/generated artifacts；完整 purge/reset 必須另有顯式破壞性流程。
- payload shape 與 Homebrew packaging 已收斂：canonical payload 為每平台單一可執行 binary，GitHub Releases `.zip` / `.tar.gz` 只封裝相同 binary；Homebrew 採 `formula`，不使用 `cask` 或混合策略。

### Business Review

Verdict: clear with support-policy guardrails.

- 官方支援承諾是「單一 canonical release lineage」，不是「所有 channel same-day parity」。若 marketplace 或 package registry 尚未同步到最新 release，文件與 entry metadata 必須指出 canonical release 與 fallback。
- marketplace 對一般使用者是官方 discovery surface，但不可把所有 provider 都宣稱成完整 direct-install lane；只有經驗證的 provider-native lifecycle 才能承諾直接 install/update。
- uninstall 不應預設刪除整個 `~/.gal/`；否則會造成 user config data-loss 風險與支援成本。保留 user config、移除 GAL-managed artifacts，較符合一般使用者預期。

### Design Review

Not requested.

### Engineering Review

CLEAR。此計畫已具備可實作邊界：它處理 GAL bootstrap payload 的 canonical release lineage、package-manager ingestion、provider marketplace discoverability、CLI entrypoint、`~/.gal/` ownership、upgrade / uninstall / purge policy，而不重新定義 install-mode plugin catalog、resolver 或 provider artifact lifecycle。實作應先鎖定文件與 release matrix，再加入 artifact build / validation、自動發布 metadata、bootstrap runtime contract 與 uninstall ownership tests。

工程限制如下：GitHub Releases 必須是 canonical version source；`winget`、Homebrew 與 marketplace entries 只能消費或包裝同一 release lineage；raw shell installer 只能是 convenience fallback；package-manager uninstall 不得刪除 user-owned `~/.gal/config/*`、lockfile、xmachine binding 或明確 local overrides；destructive cleanup 必須是顯式 purge/reset flow。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | documentation | Verify docs distinguish bootstrap installer distribution from install-mode plugin distribution, and define package-managed, GAL-managed, and user-owned boundaries. | T-001, T-010 |
| TP-002 | documentation | Verify release artifact matrix lists Windows, macOS, and Linux binary assets, `.zip` / `.tar.gz` wrappers, checksums, provenance, and supported install commands. | T-002 |
| TP-003 | build | Build or simulate per-platform release artifacts and confirm archive fallback assets wrap the same versioned single binary lineage. | T-003 |
| TP-004 | validation | Validate `winget` manifest metadata points to the canonical Windows release asset, version, checksum, license, and upgrade source. | T-004 |
| TP-005 | validation | Validate Homebrew formula metadata points to canonical macOS / Linux release assets, version, checksums, license, and upgrade source. | T-004 |
| TP-006 | documentation | Verify marketplace publication matrix defines Claude as baseline, followed by Codex and Copilot, covering submission artifacts, eligibility, and fallback copy. | T-009, T-010 |
| TP-007 | release | Run release-flow dry run and confirm GitHub Releases, `winget`, Homebrew, marketplace wrappers, and fallback copy share one canonical version lineage with downstream lag policy. | T-005 |
| TP-008 | smoke | Install bootstrap payload on Windows path and confirm `gal` CLI works without a source checkout and initializes or reuses `~/.gal/` correctly. | T-006 |
| TP-009 | smoke | Install bootstrap payload on macOS / Linux path and confirm `gal` CLI works without a source checkout and initializes or reuses `~/.gal/` correctly. | T-006 |
| TP-010 | integration | Verify bootstrap entrypoint can select or hand off to install mode / source mode without writing repo-local workflow state or assuming cloned repo paths. | T-006 |
| TP-011 | integration | Run upgrade checks for package-manager and archive paths and confirm user-owned `~/.gal/` config, lockfile, xmachine binding, local overrides, and secrets are preserved. | T-007 |
| TP-012 | cleanup | Run uninstall plan and confirm package-managed payload plus GAL-managed runtime/generated artifacts are removed while user-owned config/state/local overrides are retained. | T-008 |
| TP-013 | cleanup | Run explicit purge/reset dry run and confirm destructive deletion is opt-in, visible, and scoped to documented `~/.gal/` surfaces. | T-008 |
| TP-014 | documentation | Verify README backup / migration guidance lists what to back up, what is regenerated, and how to rebuild on a new machine. | T-011 |
| TP-015 | validation | Verify raw PowerShell / shell convenience installers redirect to canonical release artifacts and do not create a second distribution model. | T-012 |
| TP-016 | documentation | Verify provider marketplace copy does not promise same-day parity or direct install/update where provider-native lifecycle is not verified. | T-009, T-005 |
| TP-017 | cross-plan | Verify references to the install-mode plugin distribution plan preserve the boundary: bootstrap installs `gal`; install mode manages provider plugins and resolved `~/.gal/` state. | T-001, T-006 |

## Tasks

- [x] T-001 — 更新 distribution architecture 與 ownership boundary 文件，明確切分 bootstrap installer、install-mode plugin distribution、package-managed payload、GAL-managed runtime/generated state 與 user-owned config/state。
- [x] T-002 — 定義 release artifact matrix，列出 Windows、macOS、Linux canonical binary、`.zip` / `.tar.gz` fallback、checksum、provenance、license 與 package-manager ingestion metadata。
- [x] T-003 — 實作或規劃 release artifact build / packaging flow，確保所有手動 archive fallback 封裝同一版本的單一 executable binary。
- [x] T-004 — 建立 `winget` manifest 與 Homebrew formula 發布規格，確保兩者只消費 GitHub Releases canonical binary lineage。
- [x] T-005 — 定義 release governance 與 downstream drift policy，涵蓋 canonical version source、publish order、lag tolerance、verification commands 與 fallback messaging。
- [x] T-006 — 定義 bootstrap runtime contract，包含 `gal` CLI entrypoint、`~/.gal/` initialization / reuse、install mode / source mode handoff、以及無 source checkout 的首次啟動行為。 *(4b6d0f7)*
- [x] T-007 — 定義 upgrade behavior，確保 package-manager、marketplace direct-install lane 與 manual archive 更新不覆寫 user-owned `~/.gal/` config/state。 *(b8defe9)*
- [x] T-008 — 定義 uninstall 與 explicit purge/reset behavior，確保預設 uninstall 保留 user-owned config/local overrides 並移除 package-managed payload 與 GAL-managed runtime/generated artifacts。 *(4611ffc)*
- [x] T-009 — 以 Claude 作為基準，驗證現有 plugin 結構能否被 Claude 正確讀取與安裝，並建立 Claude marketplace 的 discoverability policy。 *(9afed87)*
- [x] T-010 — 待 Claude 驗證完成後，建立 Codex、Copilot 等其他 marketplace publication matrix，定義 direct-install eligibility、submission artifact 與 fallback link policy。
- [ ] T-011 — 更新 README、devguide 與 personalization docs，說明官方安裝路徑、fallback、備份/轉移、channel lag、uninstall boundary 與不保證事項。
- [ ] T-012 — 定義 raw PowerShell / shell convenience installer policy，確保它只導向 canonical release payload 且不成為唯一或第二套 distribution model。
