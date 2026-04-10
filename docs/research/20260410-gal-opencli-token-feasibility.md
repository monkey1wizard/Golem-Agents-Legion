# Research Brief: OpenCLI 作為 GAL 低 token 資訊擷取層的可行性

Operational guidance based on this research now lives in [../opencli-routing.md](../opencli-routing.md).

## Research Question

你已安裝 `opencli`。它實際上是什麼、該怎麼用、以及是否適合和 GAL 結合，讓 GAL 在查資料時用更少 token 拿到更需要的資訊。

## Scope

- 研究對象：`jackwener/opencli` 官方 README、文件站、repo 內 skill / adapter 文件
- 對照對象：GAL control plane 與 `/gal research` 的邊界
- 驗證層級：文件研究 + 本機 `opencli 1.7.0` 抽樣驗證
- 非目標：這次不直接把 OpenCLI 整合進 GAL 指令流程，也不新增 repo 內 skill

## Raw Findings

- OpenCLI 的定位不是向量資料庫、不是通用 RAG framework、也不是 GAL 的替代品。它的核心定位是把網站、瀏覽器 session、Electron app、既有 CLI 統一包成 deterministic CLI surface。
- 官方主打的價值是「同一個命令面」，來源可包含 public API、已登入的 Chrome session、Electron/CDP、以及註冊進來的外部 CLI。
- 內建命令支援結構化輸出格式：`table`、`json`、`yaml`、`md`、`csv`。如果目標是省 token，`-f json` 是最有價值的輸出模式。
- OpenCLI 區分兩種主要取數模式：
  - public commands：可直接查公開來源，不依賴瀏覽器登入
  - browser / cookie commands：重用你已登入的 Chrome/Chromium session，適合登入後資料面
- 官方文件明確把它定位成 AI agent ready runtime，包含：
  - `browser`：直接操控頁面
  - `explore` / `synthesize` / `generate`：把網站能力做成 CLI
  - `cascade`：探測 auth strategy
  - packaged skills：給 agent 走 usage / browser / explorer / oneshot
- 內建 coverage 很廣，包含公開來源與高價值查詢面，例如 `google`、`wikipedia`、`hackernews`、`arxiv`、`lesswrong`、`36kr`，以及登入型來源例如 `xiaohongshu`、`zhihu`、`weread`、`notebooklm`。
- `notebooklm` adapter 特別值得注意。它不只是列出 notebook，還能對目前 notebook 執行 `source-list`、`source-get`、`source-guide`、`source-fulltext`、`summary`、`history`。這很接近「先用現成知識庫壓縮，再讓 LLM 吃更少內容」的模式。
- `web read` 也存在，能把任意網頁轉成 Markdown。這可以當通用 fallback，但 token 效率通常不如站點專用 adapter。
- 官方 README 強調 built-in deterministic commands 在 runtime 是 zero LLM cost。要注意，這描述的是執行現有 adapter 時，不是說探索新網站能力完全沒有模型成本。
- 對 GAL 而言，`/gal research` 的 canonical contract 是「進入結構化研究流程」，而不是把第三方資料擷取 runtime 內建到 control plane。GAL control plane 應維持薄而穩定。
- GAL 目前的設計重點是：控制平面和專家執行層分離、repo 可攜、跨 Copilot / Gemini / Codex 一致。這代表任何依賴 Chrome extension、登入狀態、特定本機工具的能力，都不適合直接變成 `/gal` 的硬依賴。
- 本機驗證結果：
  - `opencli --version` 回傳 `1.7.0`
  - `opencli google -h`、`opencli wikipedia -h`、`opencli hackernews -h`、`opencli notebooklm -h`、`opencli web -h` 皆可正常顯示命令面
  - `opencli hackernews top --limit 3 -f json` 可直接回傳結構化 JSON
  - `opencli wikipedia summary "Large language model" -f json` 可直接回傳摘要 JSON
- 本機還觀察到一個實務 caveat：`opencli list` 與多個 help 命令會輸出大量 warning，指出 `~/.opencli/clis` 底下若仍有舊 YAML adapter，現在會被忽略，因為新版已不再支援 YAML adapter，需改成 TypeScript `cli()` 註冊格式。

## What OpenCLI Is Good For

### 1. 先縮小資料面，再交給模型

這是它最適合幫 GAL 省 token 的地方。

如果某個研究問題本來會走這種路徑：

1. 讓模型直接上網搜
2. 看很多頁
3. 自己再整理

那 OpenCLI 可以把前兩步壓縮成 deterministic retrieval，例如：

```bash
opencli hackernews search "Model Context Protocol" --limit 5 -f json
opencli wikipedia summary "Large language model" -f json
opencli google news "MCP" --limit 5 --lang en --region US -f json
```

這時候模型只要讀幾筆已結構化的結果，而不是整個搜尋結果頁或大量 HTML。

### 2. 查登入後資料面

如果資料在登入後網站裡，OpenCLI 的價值比一般 web fetch 高很多，因為它能重用現成 Chrome session。這對下列場景很有價值：

- 小紅書、知乎、微信讀書、閒魚等需要 cookie / page state 的來源
- NotebookLM 這種本來就已經幫你做過資料整理的環境
- 某些桌面 AI app 或 Electron app 的讀取與導出

### 3. 讓研究輸入變成機器可控的 JSON

GAL 真正省 token 的關鍵，不只是「少抓資料」，而是把抓回來的資料控制在穩定 schema。OpenCLI 很適合這件事，因為它輸出的欄位通常已經固定，例如 title / url / snippet / summary / content。

## What OpenCLI Is Not Good For

### 1. 它不是通用答案引擎

OpenCLI 負責抓資料，不負責高品質跨來源綜合判斷。最後的 synthesis 還是要由模型做。

### 2. 它不適合成為 `/gal` control plane 的核心依賴

原因很直接：

- 不是每台機器都有裝 OpenCLI
- 不是每個 runtime 都有 Chrome extension 與登入 session
- 某些 adapter 會受到 DOM drift、登入失效、站點改版影響

如果把它塞進 `/gal` 的硬依賴，會破壞 GAL 現在刻意維持的可攜性與穩定性。

### 3. 它不能取代 repo-local code reading

GAL 在 repo 內工作時，查本地程式碼仍應該優先使用 `rg`、workspace search、read_file、semantic search。OpenCLI 對 repo-local code understanding 幫助很有限。

## How To Use OpenCLI Well

建議你把它當成「可選擇的資料面工具」，不是萬用入口。

### 基本操作順序

1. 確認版本與命令面

```bash
opencli --version
opencli list
```

1. 如果是 browser-backed command，先確保：

- Chrome / Chromium 開著
- 目標站點已登入
- Browser Bridge extension 已裝好
- `opencli doctor` 可通過基本檢查

1. 優先用站點專用命令，而不是一開始就走通用抓網頁

```bash
opencli wikipedia summary "Large language model" -f json
opencli hackernews top --limit 10 -f json
opencli arxiv search "model context protocol" --limit 5 -f json
```

1. 只有在沒有專用 adapter 時，才退到通用面

```bash
opencli google search "topic" --limit 5 -f json
opencli web read --url https://example.com
```

### 對 token 最友善的使用原則

- 永遠先加 `--limit`
- 優先 `-f json`
- 先用 summary / search / guide，再讀 fulltext
- 先用 public adapter，再用 browser adapter
- 只把 shortlist 後的結果交給模型，不要整包丟

### 對 NotebookLM 的最佳用法

如果你的知識已經整理進 NotebookLM，這通常是最值得和 GAL 結合的路徑：

```bash
opencli notebooklm list -f json
opencli notebooklm open <notebook> -f json
opencli notebooklm source-list -f json
opencli notebooklm source-guide "Quarterly report" -f json
opencli notebooklm source-fulltext "Quarterly report" -f json
opencli notebooklm summary -f json
```

對 agent 來說，`source-guide` 比直接吃整份 `source-fulltext` 更省 token；只有需要引用原文時才升級到 fulltext。

## Can It Help GAL Use Fewer Tokens?

### Short Answer

可以，但要用在對的層。

### The Correct Framing

OpenCLI 不會直接讓 GAL 的 reasoning token 變少。

它能做的是減少 GAL 在「取得外部資料」這一段需要讀的原始內容量，讓送進模型的上下文更小、更乾淨、更結構化。

### Best-Case Scenarios

以下情境最適合：

- 明確站點或明確資料面：例如 HN、Wikipedia、36Kr、arXiv
- 已有 adapter 且欄位穩定
- 已登入私有來源，需要從現成 session 取資料
- 已有 NotebookLM / 現成知識整理，可先拿 summary 或 guide

### Weak Scenarios

以下情境幫助有限：

- 完全未知的新網站，還沒有 adapter
- 需要跨十幾個來源做深度綜合
- 問題本身不是 retrieval heavy，而是 reasoning heavy
- repo 內程式碼分析

## Integration Recommendation For GAL

## Verdict

ADOPT, BUT AS AN OPTIONAL RESEARCH SIDE-CAR

不要把 OpenCLI 併入 `/gal` control plane 本體。應該把它放在 research / browse / learn 類型工作中的可選資料擷取層。

## Recommended Integration Shape

### Phase 1: 零侵入使用

先不改 GAL core，只在研究流程中明確加入一條原則：

- 如果問題可以由 OpenCLI 現成 adapter 低成本取得結構化資料，先用 OpenCLI
- 再把小而乾淨的結果交給 golem-researcher 做 synthesis

這一階段甚至只需要一份 routing cheat sheet。

### Phase 2: 新增可選 skill，而不是改 `/gal`

如果要正式收編，建議新增一個 specialist skill 或 research helper，例如：

- `skills/opencli-research/`
- 或 `/browse` / `/learn` 之類的 specialist procedure 補上 OpenCLI routing 規則

這個 skill 的責任應該是：

1. 辨識問題屬於哪種來源
2. 選擇對應 OpenCLI adapter
3. 強制使用 `--limit` 與 `-f json`
4. 只把 shortlist / summary 傳給模型
5. 需要時才升級到 full page / fulltext

### Phase 3: 只在 high-value lanes 深度整合

如果未來真的要更深整合，優先整合這些 lane：

- `notebooklm`
- `wikipedia`
- `hackernews`
- `arxiv`
- `google news`
- 少數你常用且已登入的 browser sources

不要一開始就追求「所有 OpenCLI adapter 都進 GAL」。那會把維護面炸開。

## Proposed Operating Pattern

對 GAL research 工作流，建議使用這個順序：

1. 先判斷問題是否屬於 OpenCLI 已覆蓋來源
2. 若是，先取 `json` shortlist 或 summary
3. 只對 shortlist 項目做二次讀取
4. 最後再讓 golem-researcher 輸出結論與報告

簡化後大概像這樣：

```text
User question
-> source routing
-> opencli <site> <command> --limit N -f json
-> shortlist
-> optional opencli web read / notebooklm source-fulltext
-> model synthesis
-> GAL research artifact
```

## Risks And Caveats

- Browser-backed adapters 依賴登入 session、extension、DOM 穩定性
- 站點改版會讓某些 adapter 漂移
- 本機若還留有舊 YAML adapter，現在新版會忽略它們；需要轉成 TypeScript adapter
- OpenCLI 命令面很大，若沒有 routing discipline，反而會讓 agent 在工具選擇上浪費時間
- 若把它做成 `/gal` 硬依賴，會傷害 GAL 的可攜性

## Recommendation

目前最務實的答案是：

- 你可以把 OpenCLI 和 GAL 結合
- 但應該把它定位成「research data plane side-car」
- 不應把它併進 `/gal` control plane 核心

如果目標是「讓 GAL 用更少 token 找到需要資訊」，最有效的做法不是讓 GAL 全面改走 OpenCLI，而是只在適合的研究 lane 上，先用 OpenCLI 做 deterministic retrieval，再把縮過的結果交給模型。

## Recommended Next Step

最值得先做的不是改 core，而是做一份極小的 OpenCLI routing guide，收斂 5 到 8 個高價值命令面，例如：

- `wikipedia`
- `hackernews`
- `arxiv`
- `google news`
- `web read`
- `notebooklm`

目前這一步已落地為 [../opencli-routing.md](../opencli-routing.md)、[../opencli-coverage.md](../opencli-coverage.md) 與 `skills/opencli-research/SKILL.md`。

## Sources

- Upstream repo: `jackwener/OpenCLI` README, docs, and adapter docs
- OpenCLI docs site: `https://opencli.info/`
- GAL docs: [docs/command-dispatch-architecture.md](../command-dispatch-architecture.md)
- GAL docs: [docs/gal-control-plane-contracts.md](../gal-control-plane-contracts.md)
- GAL docs: [commands/commands.md](../../commands/commands.md)
- Local verification: `opencli --version`, `opencli list`, `opencli google -h`, `opencli wikipedia -h`, `opencli hackernews -h`, `opencli notebooklm -h`, `opencli web -h`, `opencli hackernews top --limit 3 -f json`, `opencli wikipedia summary "Large language model" -f json`
