# Plan: Install-Followups 殘留收尾（fix-install-followups-closeout）

## Approval

- Human approval: [pending]
- Architect review: **APPROVE(2026-06-10 全面 deep-planning;對照現碼重勘後範圍大幅縮減,見 ## Review Results)**
- Additional domain review: [not triggered]

> 收尾 `docs/observations/install-followups.md` 中無法由安裝驗證閉合的殘留項。**2026-06-10 重勘**:原計畫建於 Rust port(R-00..R-07)前的程式現實;port 已順手閉合約半數項目,且原協調對象 `feat-gal-rust-native-install` 已被 core/xmachine 兩計畫取代刪除。本版為對照現碼的重寫。

## Goal

把 `install-followups.md` 清空到只剩「已閉合」:完成仍需實際 code 變更或審查的殘留項,確認已被 port 閉合的項目並標記,然後瘦身/刪除該觀察檔。

## Governing Principle

每個殘留項對應 `install-followups.md` 既有 ID + **現勘確認的**檔案位置。安全項以實際行為/測試證明修復,不接受「看起來修好了」。

## 現況重勘（2026-06-10,對照現碼）

| 原項 | 原聲稱 | 現勘結果 | 處置 |
| --- | --- | --- | --- |
| S-1 regex | `gal-engine/src/providers/mod.rs` | **仍存在**,已遷至 `crates/providers/src/lib.rs:39-43`(anchored `^\$\{([A-Z0-9_]+)\}$`) | **R-01 修** |
| S-2 AGY unwrap | `providers/agy.rs` `to_str().unwrap()` panic | **該 pattern 已不存在**(`crates/providers/src/agy.rs` 無 `to_str().unwrap()`,疑被 port 重構修掉) | **R-V1 驗證後關閉** |
| FU-03 ledger | `run_uninstall()` 硬寫 `providers: []`/`"normal"` | **仍存在**(`crates/gal-engine/src/install.rs:595`),且 R-06 review 新增發現:中途 hard-fail 會在刪除 canonical root 後跳過 ledger 寫入 | **R-02 修(範圍擴大)** |
| FU-04 mode timeout | `join()` 無限阻塞,短逾時未實作 | **已實作**:`crates/base/src/mode.rs:82` `recv_timeout(Duration::from_secs(2))`(隨 R-00 遷入 base 時修) | **R-V2 驗證後關閉** |
| S-3 cosign | 再審未記錄 | **仍待辦**:`.github/workflows/release.yml` 已有 cosign,信任設定(workflow identity + Rekor)re-review 未記錄;與核心計畫 R-13/T-034 直接相關 | **R-03 審(掛 T-034)** |
| AGY M2 交易 | best-effort 未補交易/ledger | **仍待辦**(R-05/R-06 review 確認 AgyProjection 仍 best-effort) | **R-04 做** |
| MCP M2 serializers | 缺 AGY/Codex/OpenCode | **Codex 與 OpenCode 已存在**(`crates/mcp`:`write_codex_merged`、`CodexMcpConfig`、`OpenCodeMcpConfig`);**AGY 未見** | **R-05 縮為 AGY + 覆蓋驗證** |
| R5 commit-msg scope | optional | 仍 optional;路徑更正 `crates/cli` | **R-06 optional** |
| orphan plans | 確認兩計畫狀態 | `manage-external-plugins.md` 仍在;`feat-gal-file-memory-strategy` 有 34.6K prompt 但**不在** state.md Active Plans | **R-07 bookkeeping** |

**失效引用全數更正**:`feat-gal-rust-native-install`(已刪,由 core/xmachine 取代)、`crates/gal-engine/src/providers/`(→ `crates/providers/`)、`mode.rs`(→ `crates/base/src/mode.rs`)、`crates/gal-cli`(→ `crates/cli`)、bootstrap 簽核邊界(→ 核心計畫 architect 慣例)。

## Requirements

**修復（仍真實存在）**

- [ ] **R-01(S-1)secret-guard backstop regex** — `crates/providers/src/lib.rs::has_unresolved_secrets` 的 anchored regex 只攔整串 `${SECRET}`;對齊內嵌型(如 `Bearer ${API_KEY}`)。縱深防禦(上游 resolver 已攔)。
- [ ] **R-02(FU-03,擴大)uninstall ledger 精確度 + 必達** — `crates/gal-engine/src/install.rs::run_uninstall()`:(a) ledger entry 硬寫 `providers: []`/`mode: "normal"` 改用 `Ledger.last` 實際值;(b) 中途移除失敗(copilot/claude/dist hard-fail `?`)會跳過 ledger 寫入留無記錄半移除態——改為 best-effort 收集錯誤、ledger 必寫(記錄部分失敗),最後再回報錯誤。
- [ ] **R-03(S-3)cosign CI 信任 security re-review** — 簽章信任設定(workflow identity + Rekor)re-review 並記錄;以 `cosign verify-blob` + Rekor 查詢實證。**掛核心計畫 T-034(R-13 管線)完成後、對外發佈前**執行。
- [ ] **R-04(AGY M2)交易/ledger** — `crates/providers/src/agy.rs` 三 surface 補交易回滾 + ledger 整合(OE-A 延後項)。
- [ ] **R-05(MCP M2,縮減)AGY serializer + 覆蓋驗證** — `crates/mcp` 補 AGY serializer(若 AGY 的 MCP 設定面確認適用);以測試確認既有 Codex/OpenCode serializer 覆蓋完整(已存在但原計畫未驗收)。
- [ ] **R-06(R5,optional)commit-msg scope injection** — `crates/gal-engine/src/commit_msg.rs` + `crates/cli`:依 changed files 自動加 scope 前綴。最低優先。

**驗證後關閉（疑已被 port 閉合）**

- [ ] **R-V1(S-2)AGY 非 UTF-8 path** — 現碼無 `to_str().unwrap()`;寫一個非 UTF-8 home path 行為 probe(或 code-read 證明 lossy/Option 處理),確認不 panic 後在 install-followups 標 RESOLVED-BY-PORT(附 commit 證據)。
- [ ] **R-V2(FU-04)mode.rs dead-path timeout** — `crates/base/src/mode.rs:82` 已有 2s `recv_timeout`;確認單元測試覆蓋 dead-path 案例(無則補一個),標 RESOLVED-BY-PORT。

**Bookkeeping**

- [ ] **R-07 orphan plan 確認** — `manage-external-plugins.md`:讀後裁決關閉(superseded)或續做;`feat-gal-file-memory-strategy`:有 prompt 但不在 Active Plans,確認其真實狀態並讓 state.md 與實際一致。完成後更新 `install-followups.md` 收斂段。
- [ ] **R-08 install-followups 終態** — 全項閉合後,`docs/observations/install-followups.md` 瘦身為「全項已閉合+指向各 commit」或直接刪除(知識已沉澱於各計畫/commit)。

## Scope

**In scope**:上列 R-01..R-08、R-V1/R-V2 對應的 `crates/providers`、`crates/gal-engine`(install/commit_msg)、`crates/mcp`、`crates/base`(僅驗證)、`.github/workflows/release.yml`(僅審查)、`docs/observations/install-followups.md`、`.dev/state.md`。

**Out of scope**:安裝路徑/跨平台實機驗收(→ 核心計畫 R-10/R-13/T-034/T-035);重寫引擎架構;cosign 管線本體實作(→ 核心 T-034,本計畫只做其信任設定 re-review)。

> **受保護路徑**:`crates/gal-engine`、`crates/providers`、`crates/base` 屬核心計畫保護面。本次 deep-planning 已審;R-02 等修復實作期沿核心計畫慣例逐項簽核。**排序註記**:install 家族已於 R-05/R-06 port 完畢且穩定,R-01/R-02/R-V1/R-V2 隨時可做,不與核心計畫剩餘任務(T-034/T-035 release 面)衝突。

## Approach

| 階段 | 內容 | 前置 |
| --- | --- | --- |
| **M1 修復+驗證關閉** | R-01、R-02 點狀修+單元測試;R-V1、R-V2 驗證關閉 | 無(install 家族已穩定) |
| **M2 Features** | R-04 AGY 交易、R-05 AGY serializer+覆蓋驗證 | M1 可獨立 |
| **審查 gate** | R-03 cosign re-review | 核心 T-034 完成後、發佈前 |
| **Optional** | R-06 commit-msg scope | 隨時 |
| **Bookkeeping** | R-07 orphan 確認、R-08 install-followups 終態 | 全項後 |

每項修完即在 `install-followups.md` 對應 ID 標 RESOLVED(commit ref)。

## Files to Create or Modify

- `[MODIFY] crates/providers/src/lib.rs`(R-01)、`crates/providers/src/agy.rs`(R-04;R-V1 只讀/probe)
- `[MODIFY] crates/gal-engine/src/install.rs`(R-02)
- `[MODIFY] crates/mcp/src/lib.rs` 或新 serializer 模組(R-05)
- `[MODIFY] crates/base/src/mode.rs` 測試(R-V2,如需補測試)
- `[REVIEW] .github/workflows/release.yml`(R-03,審查為主)
- `[MODIFY] crates/gal-engine/src/commit_msg.rs` + `crates/cli`(R-06,如做)
- `[MODIFY] docs/observations/install-followups.md`、`.dev/state.md`

## Test Cases

- [ ] 內嵌型 `Bearer ${API_KEY}` 被 `has_unresolved_secrets` 攔下;整串型不回歸
- [ ] `run_uninstall` ledger 記錄實際 providers/mode;部分移除失敗仍寫 ledger(含失敗註記)後才回報錯
- [ ] 非 UTF-8 path 下 AGY 操作不 panic(R-V1 probe)
- [ ] dead-path `gal_root` 在 ~2s 內判定不可用(R-V2)
- [ ] AGY serializer 輸出符合其設定面格式;Codex/OpenCode 既有 serializer 有測試覆蓋
- [ ] cosign re-review 有書面記錄(verify-blob + Rekor 實證)

## Success Criteria

- [ ] R-01/R-02 修復且有單元測試;R-V1/R-V2 以證據關閉。
- [ ] R-03 在發佈前完成並記錄(或明確標 gate 未到)。
- [ ] R-04/R-05 完成且行為測試通過。
- [ ] R-07 兩 orphan 狀態確認、state.md 一致;R-08 install-followups 歸零。

## Risks

- **受保護核心面**:R-02 改 uninstall 錯誤語意(hard-fail→best-effort+必寫 ledger)是行為變更,須在 task spec 明寫新語意並補測試。緩解:architect 已審方向(見 Review Results),實作逐項簽核。
- **R-05 AGY serializer 適用性不明**:AGY 的 MCP 設定面可能與假設不同。緩解:task 內先 5 分鐘現勘,不適用則以證據關閉。
- **cosign re-review 依賴 CI 環境**:Mitigation:以 `cosign verify-blob` + Rekor 實證,不接受「workflow 跑過」。

## Review Results

### Architecture Review

#### Verdict: APPROVE *(2026-06-10 全面 deep-planning,重勘後)*

原計畫的根本問題是**建於過期前提**:協調對象已刪(feat-gal-rust-native-install)、四個檔案路徑全錯(crate 拆分後)、兩項聲稱的 bug 已被 port 修掉、一項聲稱缺的功能已存在 2/3。重寫後範圍誠實:2 修復 + 2 驗證關閉 + 2 M2 + 1 審查 gate + bookkeeping,全部錨定現碼勘察結果。

- **M1/M2 不拆計畫(原 OQ-02 裁定)**:重勘後 M1 只剩 2 個小修,拆計畫是過度流程;單計畫分階段即可。
- **R-02 範圍擴大正當**:R-06 post-merge review 的 ledger-skip 發現與原 FU-03 同函式同主題,合併一個 task 一次改對;「ledger 必寫」語意(best-effort 收集、必記、後報錯)是 uninstall 作為破壞性操作的正確錯誤模型。
- **R-03 掛 T-034 正確**:cosign 信任 re-review 審的是 release 管線的信任設定,管線本體在核心 T-034;審查先於發佈、後於管線,gate 順序成立。
- **驗證後關閉(R-V*)模式正確**:對「疑似已修」項不直接劃掉,以 probe/測試證據關閉——與 honest-pass 原則一致。

**Trade-off**:擴 R-02 語意(行為變更) vs 保留 parity(原樣硬寫)——uninstall 的 ledger 是事後審計的唯一線索,正確性優先於 parity,且 legacy 腳本已刪無 parity 對象。OK。
**Bug Surface**:R-02 錯誤語意改動(已列 Test Cases);R-05 AGY 適用性(已列 Risks)。
**Over-engineering check**:原 R-07「三 serializer」縮為「AGY+驗證」;M1/M2 不拆計畫;無新抽象。成立。

### Engineering Review

Pending — 下一步 `/refining-plan` 展開 `## Tasks` 與 `## Test Plan`。

## Tasks

Pending `/refining-plan`.

## Test Plan

Pending `/refining-plan`.
