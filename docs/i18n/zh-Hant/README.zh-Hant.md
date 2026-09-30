# Golem-Agents-Legion

<!-- gal:translation-metadata
source: README.md
lang: zh-Hant
source_commit: PENDING
translated_at: 2026-09-24
-->

[English](../../../README.md) · [日本語](../ja/README.ja.md) · **繁體中文**

GAL 是一套以 Rust 撰寫、以文件為核心 (document-driven) 的 AI 開發工作流程引擎。它能完整獨立運作於單一 AI 工具中（如 Claude Code、Codex CLI、GitHub Copilot、Antigravity CLI 或 opencode），亦具備跨供應商自由切換與混用模型的彈性。經由儲存庫自有的 Markdown 檔案 (`.dev/`)，GAL 將規格、規劃、實作、測試、審查與研究狀態持久化，徹底告別供應商對話紀錄鎖定。系統由編譯式 Rust 二進位檔在嚴謹的品質檢核點下推進任務。
就像指揮專屬的魔像軍團，每位 golem 代理程式 (golem agent) 在工作流程中各司其職，協同完成開發任務。這也是專案命名為 Golem-Agents-Legion 的由來。

## 核心特性

- **單一工具即開即用，兼具跨環境彈性**：只需一個慣用的 AI 執行環境（Claude Code、Codex CLI、GitHub Copilot、Antigravity CLI 或 opencode）即可完整執行全部流程，無需多重訂閱。亦能隨時切換工具，或將特定階段指派給不同模型。
- **文件驅動的結構化生命週期**：藉由規格、規劃、審查與實作的階段檢核點（REFINE-LOCK 迴圈），在動手修改程式碼前完成確定性的需求收斂與風險審查。
- **儲存庫自有狀態與中斷點接續 (`.dev/`)**：以專案本地的 Markdown 檔案完整記錄開發脈絡，擺脫平台對話紀錄鎖定。管線執行若中途中斷，可直接從確切的記錄中斷點接續。
- **統一控制平面與零供應商鎖定**：透過標準化指令介面 (`/gal …`) 與 golem 代理程式合約，讓工作流程在所有支援環境中維持一致，更換工具無需重整專案資產。
- **原生 Rust 編譯引擎**：`gal` 二進位檔於本機高效處理協調、狀態投影與轉接器生成，避免純提示詞架構帶來的結構性開銷與脈絡膨脹。
- **彈性模型路由與成本控制**：可於 `~/.gal/config/config.json#executorRouting` 依據不同開發階段（如規劃、實作、稽核），靈活配置最經濟或最強大的模型。

## 快速開始

GAL 主要採用**安裝模式**，使用者無須複製 (clone) 此儲存庫。

**先決條件：** 必須預先設定好支援的 AI 編碼執行環境 (coding runtime)，包含 Claude Code、Codex CLI、GitHub Copilot、Antigravity CLI 或 opencode。

1. **安裝 `gal` CLI：**

   ```bash
   curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
   ```

   ```sh
   irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
   ```

   包含 Homebrew、winget 與從原始程式碼編譯在內的其他安裝方式詳列於 [docs/setup.md](../../setup.md) 中。

2. **專案初始化**：在目標專案中開啟支援的執行環境並執行 `/gal init`（Codex 使用 `$gal init`）。此動作會建立 `.dev/` 目錄並產生各代理程式的轉接器檔案。

3. **工作流程執行**：執行 `/gal status` 以確認儲存庫初始化並檢視後續步驟。執行 `/planning` 將請求轉換為初步計畫。

完成上述步驟後，儲存庫即完成初始化並具備核心功能。選用的執行器路由與即時供應商檢查須另行設定。Codex 的兩種支援權限模式請參閱 [Codex 管道設定](./setup.zh-Hant.md#codex-管道執行設定)，派送邊界與證據階段請參閱[執行生命週期](./workflows.zh-Hant.md#派送紀錄與診斷)。其他主題文件請見下方[文件地圖](#文件地圖)。

### 功能生命週期

```text
/gal init
   ↓
┌── Planning Phase ──────┐
│ /planning              │
│    ↓                   │
│ /deep-planning         │
│    ↓                   │
│ [OQ-completion gate]   │
│    ↓                   │
│ /refining-plan         │
│    ↓                   │
│ [human approval gate]  │
│    ↓                   │
│ /plan-to-prompt        │
└────────────────────────┘
   ↓
/gal pipeline
   ↓
/gal finalize
```

`/gal init` 指令會在完成前述步驟後為儲存庫建立一次性的骨架。規劃階段每個節點的完整操作說明位於 [docs/workflows.md](../../workflows.md)，管道 (pipeline) 執行與落地 (finalize) 的完整說明同樣位於該份文件。

## 文件地圖

此表是已釋出文件之單一扁平地圖。每個主題只有一份權威檔案，其他文件僅提供連結，不重述合約內容。

這份 README 的說明由本表維護。其他文件的說明與各文件 front matter 的 `description` 逐字相符；若有差異，以該文件的 `description` 為準並修正本表。

| 文件 | 單一職責 |
| --- | --- |
| [README.zh-Hant.md](./README.zh-Hant.md) | GAL 的公開進入點，涵蓋專案摘要、快速開始、功能生命週期總覽與文件地圖。 |
| [setup.zh-Hant.md](./setup.zh-Hant.md) | 安裝 gal 二進位檔、向各執行環境註冊外掛，並初始化與維護儲存庫轉接器。 |
| [configuration.zh-Hant.md](./configuration.zh-Hant.md) | 管理 `~/.gal/config/config.json` 的機器本機設定，涵蓋執行器路由、工作時段、計畫語言、MCP 來源、個人層與本機筆記路由。 |
| [workflows.zh-Hant.md](./workflows.zh-Hant.md) | 說明 Golem 角色與呼叫方式，並涵蓋規劃、管道、落地、復原、git 與撰寫輔助指令。 |
| [projection.zh-Hant.md](./projection.zh-Hant.md) | 說明 GAL 如何把來源合約算繪成標準根目錄、投影到各執行環境介面，並產生儲存庫本機轉接器、投影登錄檔與標準 MCP 清單。 |
| [architecture.zh-Hant.md](./architecture.zh-Hant.md) | GAL 的常設約束、crate 與分層邊界、儲存拓樸、遷移路徑、文件治理與 ADR 索引。 |
| [integrations.zh-Hant.md](./integrations.zh-Hant.md) | GAL 可選外部整合工具的參考說明，包含用途、就緒判定、設定邊界與缺少時的安全退化行為。 |
| [terminology.zh-Hant.md](./terminology.zh-Hant.md) | zh-Hant 譯文的術語呈現標準，規定已定案的英文術語在繁體中文行文中的唯一寫法。 |
| [SECURITY.zh-Hant.md](./SECURITY.zh-Hant.md) | 說明 GAL 支援的版本、私密通報漏洞的流程，以及產品的安全性模型與信任邊界。 |
| [CONTRIBUTING.zh-Hant.md](./CONTRIBUTING.zh-Hant.md) | 提交 GAL 原始碼變更的入口，涵蓋 branch 與 commit 規則、建置與測試關卡、貢獻者檢查與選用的本機審查工具。 |

術語的語意權威與 ADR 只有英文正本，不做整套 zh-Hant 翻譯：

| 文件 | 單一職責 |
| --- | --- |
| [`docs/glossary.md`](../../glossary.md) | GAL 英文術語的唯一語意權威，涵蓋命名規則、術語登錄表與退役術語清單。 |
| [ADR](../../adr/) | 每份檔案各自記錄一項架構決策，集中索引由 architecture 擁有。 |

## 參考資料

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)
- [rtk](https://github.com/rtk-ai/rtk)：CLI 代理層 (proxy)，可在開發指令輸出（包含 git、cargo 與測試執行器）送入模型脈絡前先行過濾並壓縮。此工具獨立運作，不需 GAL 偵測或相依，強烈建議與 GAL 搭配使用。管道執行時每個任務都會觸發許多 shell 指令，修剪輸出能有效最大化執行環境的脈絡預算。

## 授權條款

採用 MIT 授權條款。請參考 [LICENSE](../../../LICENSE)。
