# Research Brief: MemPalace 作為 GAL 共用長期記憶層的可行性

## Research Question

在現有 GAL 架構下，是否適合把 MemPalace 納入 MCP manifest 與 runtime 安裝流程，讓 Gemini、Copilot、Codex、Claude 盡可能共用同一套長期記憶。若可行，哪些 runtime 已可透過 GAL 接上，哪些仍只能手動整合，且這套設計對 infra-lan-worker-topology plan 的真正含義是什麼。

## Scope

- 研究對象：GAL 的 MCP 管理層、VS Code / Gemini CLI / Codex CLI / Claude Code 的接線方式、MemPalace 的 MCP 與 plugin/hook 整合
- 研究重點：同一個 MCP server definition 是否足以形成同一個 shared memory system
- 非目標：直接實作 MemPalace manifest、直接修改 Setup-Machine、直接把 MemPalace 納入 remote worker contract

## Assumptions

- 本文中的 `Copilot` 預設指 VS Code 內的 GitHub Copilot / agent surface，而不是 Copilot CLI。
- 若要討論 Copilot CLI，需另外說明，因為它在 GAL 目前的安裝拓樸中與 VS Code MCP 不是同一條路。

## Raw Findings

- GAL 目前把 MCP 視為獨立 manifest layer，並將 provider config 合併到 VS Code、Gemini CLI、Codex CLI；Claude Code 目前明確標為 deferred，不在現行 merge flow。來源：[devguide.md](../devguide.md) 與 [README.md](../../README.md)
- GAL 的 machine install 說明顯示，Gemini CLI 透過 `settings.json` bridge 讀取 skills 並合併 `mcpServers`，Codex CLI 會附加 `[mcp_servers.*]`，而 Claude Code CLI 仍屬 future。來源：[devguide.md](../devguide.md)
- `mcp-servers.example.json` 的 provider 目前只覆蓋 `vscode`、`gemini`、`codex`，沒有 `claude` provider。這表示 GAL 現在的 MCP source of truth 尚未把 Claude 當成同級自動配置目標。來源：[mcp-servers.example.json](../mcp-servers.example.json)
- MemPalace 上游 README 把自己定位成 local-first 的長期 AI memory system：以本地 ChromaDB 保留原始對話內容，並用 MCP tools、knowledge graph、搜尋與 hooks 提供回憶能力，而不是 workflow state store。來源：[MemPalace README](https://github.com/milla-jovovich/mempalace)
- MemPalace 明確支援 Gemini CLI 的 MCP 註冊與 `PreCompress` hook，可在 context 壓縮前自動保存記憶。來源：[MemPalace Gemini guide](https://github.com/milla-jovovich/mempalace/blob/main/examples/gemini_cli_setup.md)
- MemPalace 已提供 Codex plugin，包含 skills 與 auto-save hooks，能在 session stop 與 context compaction 前保存記憶。來源：[MemPalace Codex plugin](https://github.com/milla-jovovich/mempalace/tree/main/.codex-plugin)
- MemPalace README 顯示 Claude Code 有 plugin 與 hooks 路線，也可透過 MCP 使用其 19 個工具。來源：[MemPalace README](https://github.com/milla-jovovich/mempalace)
- MemPalace README 將對話來源描述為 Claude、ChatGPT、Copilot 等 conversation exports / chat traces 都可以被 mine，但這不等於每個 runtime 都已有對等的 first-party auto-save integration。來源：[MemPalace README](https://github.com/milla-jovovich/mempalace)
- MCP 只標準化 tool access，不保證 backend identity。只有在多個 runtime 最終都連到同一個 palace backend 時，它們才真的在讀寫同一份記憶；若每台機器各自使用自己的預設本地 palace path，記憶仍會分岔。這是 MemPalace local-first 設計的直接推論，不是 MCP 自動提供的能力。來源：[MemPalace README](https://github.com/milla-jovovich/mempalace)
- 以目前的新架構而言，GAL 已明確要求 remote worker 不得維護第二套平行狀態系統；正式 workflow state 仍必須留在 `.dev/state.md` 與 `.dev/plans/<plan-slug>.prompt.md`，source plan 則留在 `docs/plans/<plan-slug>.md`。這些都不是外部記憶庫可以接管的對象。來源：[mod/remote-worker.md](../mod/remote-worker.md)、[README.md](../../README.md) 與 [commands/commands.md](../../commands/commands.md)

## Synthesis

### 核心判斷

結論不是「把 MemPalace 用 MCP 安裝進 GAL 後，四個 runtime 就自然共用同一個 memory system」，而是：

1. GAL 可以把同一份 MemPalace MCP 定義佈到部分 runtime。
2. 只有當這些 runtime 實際連到同一個 palace backend 時，它們才真的共用記憶。
3. 即使共用 backend，不同 runtime 的寫入成熟度與 auto-save 能力仍不對稱。

也就是說，MCP 解的是「工具如何被看見」，不是「資料如何天然同步」。

### Runtime Verdict Matrix

| Runtime | 是否可透過 GAL 使用共同 memory | 判斷 | 原因 |
| --- | --- | --- | --- |
| Gemini CLI | 是 | `YES` | GAL 已能管理 Gemini 的 `mcpServers`；MemPalace 也有明確的 Gemini MCP + hook 路線。若指向同一 backend，可成為共享記憶客戶端。 |
| VS Code Copilot | 部分可行 | `PARTIAL` | GAL 已能管理 VS Code 的 MCP config，因此 Copilot / agent surface 可取得相同 MemPalace tools；但 MemPalace 沒有公開等同 Gemini / Codex / Claude 的 Copilot first-party auto-save 路線，較像 read-first integration。 |
| Copilot CLI | 目前不成立 | `NO` | GAL 對 Copilot CLI 目前只記錄 skills/agents 安裝，沒有現成的 MCP merge path；若使用者說的 `Copilot` 其實是 Copilot CLI，答案應比 VS Code Copilot 更保守。 |
| Codex CLI | 是 | `YES` | GAL 已能管理 Codex 的 `mcp_servers`；MemPalace 也已提供 Codex plugin 與 auto-save hooks。若指向同一 backend，可與 Gemini 共用記憶池。 |
| Claude Code | 不能透過 GAL 直接做到 | `NO` | MemPalace 本身支援 Claude plugin / MCP / hooks，但 GAL 目前未把 Claude 納入 MCP merge flow；因此可以手動整合，但不能說是「透過 GAL 已完成」。 |

### 對 infra-lan-worker-topology plan 的真正意義

這份研究比較支持以下說法：

- MemPalace 可以成為 GAL 的 shared long-term memory sidecar。
- 它不應成為 GAL 的正式 workflow state。
- 它也不應介入 source plan 與 execution prompt 的分工；前者是人類可讀計畫，後者是 `.dev/plans/` 下的可變 execution memory。
- 它能幫 Gemini、VS Code Copilot、Codex 在同一台機器上共用外部記憶查詢層。
- 它不能只靠 MCP 就自動解決多機共享記憶與一致寫入問題。

### 為什麼「同一台機器」與「跨機器」要分開看

若 Main PC 上同時有 VS Code Copilot、Gemini CLI、Codex CLI，並且它們都被 GAL 配到同一個 MemPalace backend，這件事相對直接。

但你的 infra plan 關心的是 Main PC、Windows sub PC / notebook、Mac Mini 三類節點。到了多機情境，問題不再只是 MCP：

1. 每台機器是否真的指向同一個 palace backend。
2. 該 backend 是單機本地目錄、網路共享磁碟、還是某種受控 service host。
3. ChromaDB + SQLite 是否允許你接受的多機寫入模型與故障語義。

因此，「透過 GAL 讓多個 runtime 共用記憶」與「透過 GAL 讓多台機器安全共用同一個 MemPalace backend」是兩個不同難度等級的問題。前者可先做，後者不能被 MCP 名詞掩蓋。

## Architecture Review

### Verdict

REVISE

### What Is Realistically Achievable Now

- 讓 Gemini、VS Code Copilot、Codex 在 GAL 管理下取得同一組 MemPalace MCP tools。
- 在單機或單一主要 backend 前提下，讓這些 runtime 查詢同一份長期記憶。
- 讓 Gemini 與 Codex 具有較成熟的 auto-save / write path。
- 讓 Claude 保持手動或 out-of-band 整合，而不是硬塞進 GAL 目前的自動安裝拓樸。

### Blocking Issues

- 把「MCP 已安裝」直接等同於「shared memory 已成立」會高估設計完成度。
- 把「VS Code Copilot」與「Copilot CLI」混為一談，會讓 runtime wiring 與支援矩陣失真。
- 若沒有先定義主要的 MemPalace backend 宿主與寫入政策，多機共享記憶只會變成多份 local palace 的並列存在。
- 若讓 MemPalace 介入 `.dev/state.md`、`.dev/plans/<plan-slug>.prompt.md`、source plan 或 remote task status，會直接撞上 GAL 已定義好的正式檔案模型。

### Warnings

- MemPalace 比較像長期記憶檢索層，不像 orchestration state store。
- VS Code Copilot 最可能先成為 read-mostly client，而不是與 Gemini/Codex 同等成熟的 write client。
- Claude 能用 MemPalace，不代表 Claude 已納入 GAL 的 setup contract。
- 若未來要跨 Main PC、Windows worker、Mac Mini 共享同一份 palace，應優先設計 single-host ownership 或 service wrapper，而不是直接依賴 network share 上的多機本地資料庫寫入。

## Recommended Integration Shape

1. 在 GAL 中把 MemPalace 定位成 optional shared-memory sidecar，而不是 control-plane state layer。
2. 先支援 `vscode`、`gemini`、`codex` 三個 provider 的同一份 MemPalace MCP manifest；Claude 保持手動整合。
3. 先採單一主要 backend host，再讓各 runtime 連到同一 backend；不要先做 multi-writer network share。
4. 定義 runtime write policy：Gemini、Codex 可寫；VS Code Copilot 先視為 read-first；Claude 另行管理。
5. 明確禁止用 MemPalace 取代 `.dev/state.md`、`.dev/plans/<plan-slug>.prompt.md`、`docs/plans/<plan-slug>.md`、`status.json`、`summary.md` 等 GAL 正式狀態與 runtime 檔案。

## Recommended Next Step

若要把這件事納入 infra-lan-worker-topology 的後續工作，建議先新增一個非常小的 integration spike，而不是直接宣稱「GAL 已具備跨 runtime 共同記憶能力」：

1. 在單機上為 VS Code、Gemini、Codex 指向同一個 MemPalace backend。
2. 驗證三者是否都能成功看到同一批搜尋結果。
3. 驗證 Gemini 與 Codex 的寫入是否會如預期更新同一 backend。
4. 將 VS Code Copilot 的能力暫定為 read-first，直到有可靠的 save adapter。
5. 把 Claude 留在 phase-2 或 manual lane，避免現在就修改 GAL 的 provider matrix。

## Sources

- Repo docs: [devguide.md](../devguide.md)
- Repo docs: [commands/commands.md](../../commands/commands.md)
- Repo docs: [README.md](../../README.md)
- Repo docs: [mod/remote-worker.md](../mod/remote-worker.md)
- Repo manifest: [mcp-servers.example.json](../mcp-servers.example.json)
- MemPalace README: [github.com/milla-jovovich/mempalace](https://github.com/milla-jovovich/mempalace)
- MemPalace Gemini guide: [examples/gemini_cli_setup.md](https://github.com/milla-jovovich/mempalace/blob/main/examples/gemini_cli_setup.md)
- MemPalace Codex plugin: [.codex-plugin](https://github.com/milla-jovovich/mempalace/tree/main/.codex-plugin)
