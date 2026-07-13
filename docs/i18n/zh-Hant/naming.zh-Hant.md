# GAL 命名權威(Naming Authority)

> 這是 [`docs/naming.md`](../../naming.md) 的 **zh-Hant 翻譯**。正式版(canonical)為英文;
> 兩者衝突時以英文版為準。程式碼識別字、檔案路徑、命令名、產品名、術語 key 與
> `RETIRED-TERMS` 機器區塊一律保留原文,不翻譯。

> **GAL 中每個核心術語意義與命名方式的唯一真實來源(single source of truth)。**
> 當程式碼、註解、文件或計畫中的名稱有歧義時,以本檔為準。
>
> - 正式語言:`en`(依 `PROJECT_LANGUAGE`)。zh-Hant 翻譯位於
>   `docs/i18n/zh-Hant/naming.zh-Hant.md`。
> - 程式碼識別字、檔案路徑、命令名、產品名為字面值 —— 永不翻譯。
> - `docs/architecture.md` 描述**結構**(crate DAG / 相依法則),並連結此處取得**詞彙**。
>   本檔不重複 DAG。
> - 由 `infra-naming-authority` 計畫擁有。核心術語錨定到涵蓋它的**最中立權威** ——
>   標準組織(ISO/IEC、NIST)與 Linux Foundation 的 Agentic AI Foundation(治理 GAL 實作的兩個標準
>   MCP + AGENTS.md)—— 優先於廠商來源。見[權威分層](#part-b--core-external-concepts-authority-anchored)
>   與 [Sources](#sources)。
>
> **詞彙表慣例(Glossary conventions)。** B–D 部分的術語表遵循 **arc42 §12「Glossary」**結構
> (單一專案級術語登錄),並以 **Domain-Driven Design「Ubiquitous Language」**紀律擴充:每個術語帶
> **Status**(`Validated` = 現行正式術語 · `Deprecated` = 已退役、勿用)與 **Aliases**(可接受的同義詞,
> 以及不可混淆的錯誤名稱)。退役術語另以
> [機器可讀區塊](#retired-terms-gate-input)輸出,供 naming gate 讀取。

## 如何使用本文件

1. 在引入一個可能有多重意義的名稱前,先在此查找。若尚未登錄,先依命名邏輯 Rule 6 登錄再使用。
2. 程式碼註解與長存文件陳述的是**長存意圖(durable intent)**,絕非**出處(provenance)**
   (見 Rule 4)—— plan-task ID 與遷移歷史不應出現在出貨產物中。
3. 當 GAL 兩處對某術語不一致時,以本檔為準;修正分歧的那一方。

---

## Part A — Naming Logic(規則)

以下是本專案用來鑄造與消歧名稱的專屬規則。

1. **一詞一義(One term, one meaning)。** 一個名詞在全專案只對應一個概念。若某概念需要命名而顯而易見的字
   已被佔用,鑄造一個帶限定詞的名稱 —— 不要重用裸字。
2. **保留裸字,其餘一律限定。** 兩個裸字被*保留*給單一 GAL 意義,可不加限定使用:
   - **`agent`** → **golem agent**(GAL 自有的專家 AI-agent 人格)。這是刻意的產品設計選擇:
     GAL 的 agent 就是 golem agent,所以裸寫「agent」即指它們。
   - **`model`** → **LLM**(大腦)。在 GAL 中「model」與「LLM」是固定同義詞。

   其他*任何*可能被 `agent`/`adapter`/`surface` 這類英文字搶走的概念都必須加限定:外部 AI 編程工具一律是
   **`coding agent`**(絕不裸寫「agent」),另有 **`MCP server`**、**`(file) projection backend`**、
   **`generated runtime adapter`**、**`runtime surface`**。在程式碼識別字與長存文件中,禁止裸用未保留的多義字。
3. **實體 vs 角色(Entities vs roles)。** 同一個外部 **coding agent** 依其*在某軸上扮演的角色*命名:作為
   adapter 生成目標時是 `runtime`,被派生執行某 dispatch 階段時是 `executor`,消費 MCP server 時是
   `MCP host`。絕不把某軸的角色名用到另一軸,也絕不把 coding agent 裸稱為「agent」(那個字屬於 golem agent)。
4. **出處存於計畫記憶,不存於長存產物。** plan-task ID(`T-NNN`、`R-NN`、`TP-NNN`、`FU-NN`、`BUG-X`)
   與遷移敘述(「repointed at…」、「strangler seam」、「until then…」)屬於*出處(provenance)*。它們只允許出現在
   `.dev/**`。出貨的程式碼、註解與長存文件只陳述*長存意圖*。
5. **名稱反映當前擁有者,而非遷移歷史。** 模組/檔案/符號名稱必須描述它*現在*的角色/crate
   (例如 `install_orchestration`,而非 `legacy_plugins`)。
6. **新的易多義術語先在此登錄再使用**,連同其軸與擁有者。
7. **識別字構成(house style)。** 多字識別字是*如何構成*的:
   - **優先序 —— 通用標準優先,house style 次之。** 語言的通用標準具權威性,衝突時優先:Rust 即為
     [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html) + RFC 430
     (大小寫、trait/constructor 慣例、縮寫)。本 GAL house style **只**適用於標準留給自由選擇之處 ——
     選字、字序、headword 形式 —— **絕不**覆寫標準。
   - **資料**(types、fields、variables、modules)= **(動詞→形容詞) + 名詞** —— 把動詞轉成分詞/動名詞形容詞
     修飾一個中心名詞:`CodingAgent`、`DefinedName`、`ResolvedRoot`、`GeneratedAdapter`。
   - **函式 / 方法** = **動詞開頭的動作片語**(動詞保持動詞):`define_name`、`resolve_runtime`、
     `generate_adapter` —— *不是*形容詞形(`defined_name()` ✗,那是在命名結果而非動作)。
   - **正式 headword = `UpperCamelCase`,不加底線**(`CodingAgent`)。這是在本檔登錄並作為型別名稱使用的形式。
     **大小寫則交由語言/項目種類的標準決定** —— Rust 依 RFC 430:fields、locals、modules、functions 用
     `snake_case`(`coding_agent`),consts 用 `SCREAMING_SNAKE_CASE`。本規則只固定*選字、字序與 headword 形式*
     —— 大小寫永遠由語言擁有。
8. **不用通用桶名(No generic bucket names)。** 不要用包山包海的複數容器字為 crate/module/file 命名 ——
   `adapters`、`services`、`providers`、`utils`、`helpers`、`common`、`managers`、`handlers`、`misc`。它們會變成雜物抽屜,
   重新製造 Rule 1 禁止的多義。用單元的**單一職責**命名:`projection`(非 `adapters`)、`executors`
   (非 `dispatch::adapters`)、`mcp::serializers`(非 `providers`)。若此類通用字真的無可避免,全專案**最多只能有一個**,
   且必須在此登錄其範圍。
9. **不可意外重名;標準槽位是唯一例外。** 力求一個資料夾名、檔名或函式名在專案中**只出現一次**,讓搜尋能跳到單一處。
   不要在不同位置重用同一識別字指涉不同事物。
   **例外 —— 由外部標準或刻意的 GAL 慣例固定的名稱*本就*該重複**,因為每次出現都是同一個結構槽位(不是多義):
   - Cargo 佈局(每 crate 重複一次):`src/`、`src/lib.rs`、`src/main.rs`、`mod.rs`、`Cargo.toml`、`tests/`、`build.rs`。
   - GAL 命令標準(每命令目錄重複一次):`SKILL.md`、`SKILL.template.md`。
   - Trait / constructor 方法名(協定槽位,非衝突):`new`、`default`、`from_*`、`to_*`、`as_*`、`into_*`、`iter`、`fmt`,
     以及任何實作共享 trait 的方法。
   - 各 runtime 的 adapter 標準檔名:`AGENTS.md`、`CLAUDE.md`、`GEMINI.md`、`.github/copilot-instructions.md`
     (每 repo 一個;字面外部 token)。

   不屬於已認可標準槽位的重複,即為 Rule-1 / Rule-8 違規 —— 改名。
   (註:`README.md` 在此**不是**可重複槽位 —— GAL 政策是恰好**一個**正式 `README.md`;翻譯用 `README.<lang>.md`。)

> **大小寫權威:** Rust 遵循 [Rust API Guidelines — Naming](https://rust-lang.github.io/api-guidelines/naming.html)
> (C-CASE,建於 RFC 430):types/traits/enum-variants 用 `UpperCamelCase`;functions/methods/vars/modules/crates 用
> `snake_case`;consts/statics 用 `SCREAMING_SNAKE_CASE`;縮寫視為一個字(`Uuid`,非 `UUID`)。見
> `plugins/gal-core/conventions/rust.md`。

---

## Part B — Core external concepts (authority-anchored)

以下定義錨定到權威來源。它們是 GAL 向更廣生態借用的詞彙;不要在本地重新定義。

**權威分層**(優先採用涵蓋該術語的最中立來源):

1. **標準組織**(中立、正式)—— ISO/IEC JTC 1/SC 42、NIST。最適合 *AI agent* 此一屬(genus)與核心 AI 術語。
2. **Linux Foundation — Agentic AI Foundation (AAIF)**(自 2025-12-09 起的中立託管方)—— 治理 GAL 實作的兩個開放標準:
   **MCP** 與 **AGENTS.md**(及 goose)。高於任何單一廠商。
3. **廠商第一方** —— Anthropic / OpenAI / Google。對*自家產品名*具權威;類別性宣稱在未被第 1–2 層複述前視為行銷語。
4. **百科** —— Wikipedia。中立但屬第三級。

| Term | Status | Aliases / a.k.a. | 正式定義 | 權威 | **勿**混淆於 |
| --- | --- | --- | --- | --- | --- |
| **AI agent** (genus) | Validated | intelligent agent | 一個感知、決策、並朝目標行動的實體;定義為正式 AI 術語。 | **ISO/IEC 22989:2022**(在 110+ 術語中定義「AI agent」);NIST AI Agent Standards Initiative | chatbot;LLM。(在 GAL 中裸寫 **`agent`** = golem agent —— 見 Part C;golem agent 與 coding agent 都是此屬的*實例*。) |
| **model** / **LLM** | Validated | LLM(固定同義) | 在大量文本上訓練以處理語言任務的神經網路。即「大腦」。 | Wikipedia,*Large language model*;ISO/IEC 22989(「model」、「AI system」) | 建於其上的 agent 或 chatbot |
| **chatbot** / **AI assistant** | Validated | AI assistant | 建於 LLM 上的消費者端對話應用(ChatGPT、Claude、Gemini *這些 app*)。 | Wikipedia | LLM 本身;coding agent |
| **coding agent** | Validated | agentic coding tool; agentic development platform | 一種會讀程式庫、編輯檔案、執行命令、並與開發工具整合的 AI agent。GAL 鎖定的類別。 | **中立**:Linux Foundation **AAIF** 將 AGENTS.md 定義為給「**AI coding agents**」的指引。**廠商**:Anthropic「agentic coding tool」(Claude Code)、OpenAI「coding agent」(Codex)、Google「agentic development platform」(Antigravity)。 | 它跑的 model;它執行所在的 surface |
| **surface** / **interface** | Validated | interface | coding agent *在何處*執行:terminal/CLI、IDE、桌面 app、web。一個 agent 可有數個。 | Anthropic:Claude Code「available in your terminal, IDE, desktop app, and browser」 | agent 本身(「CLI」≠ agent 的類別) |
| **MCP** | Validated | Model Context Protocol | Model Context Protocol ——「連接 AI models 到工具、資料與應用的通用標準協定」。 | **Linux Foundation AAIF**(自 2025-12-09 託管;由 Anthropic 捐贈);規格見 modelcontextprotocol.io | — |
| **MCP host** | Validated | (agent 的 MCP 角色) | 協調 MCP clients 的 AI 應用(coding agent **就是** MCP host)。 | MCP spec,*Architecture*(以 Claude Code 為 host 範例) | MCP server;~~MCP provider~~(無此角色) |
| **MCP client** | Validated | — | host 內部持有對單一 MCP server 連線的元件。 | MCP spec | host 或 server |
| **MCP server** | Validated | the provider | 向 clients **提供**工具/資源/情境的程式。*server* 才是 provider。 | MCP spec | host;~~「MCP provider」~~指 agent(方向相反) |
| **AGENTS.md standard** | Validated | — | 「給 **AI coding agents** 一致的專案專屬指引來源的簡單通用標準。」GAL 在生成 AGENTS.md 時一併生成各 agent 變體。 | **Linux Foundation AAIF**(由 OpenAI 捐贈,2025-12-09) | GAL 自創之物 |
| ~~**MCP provider**~~ | **Deprecated** | → 改用 **MCP host** + **per-runtime MCP server config** | (已退役)草擬期對「我們為其寫 MCP 設定的 coding agent」的稱呼。方向相反:agent 是 *host*,server 才是 *provider*。 | 由 `infra-naming-authority` 退役 | —(見 retired-terms gate input) |

> **退役術語:** `MCP provider`(舊草稿用來指「我們為其寫 MCP 設定的 coding agent」)是**錯的**並已移除。
> agent 是 **MCP host**;**server** 才是 provider。正確的 GAL 概念是 *per-runtime MCP server configuration*(見 Part C)。

---

## Part C — GAL-internal vocabulary(外部概念如何對應到 GAL)

這裡是把 GAL 自有識別字釘到上述外部概念的地方。

> Rule 8 的 crate/module 改名(`adapters`→`projection`;`providers` 併入 `mcp::serializers`;
> `dispatch::adapters`→`executors`)**已落地**(`refactor-rust-architecture`,2026-06-14 VERIFIED + 關閉)。
> 下表 Owner 欄為**現行**的 crate/module 名稱。

| GAL term | Status | Aliases | = 哪個概念 | 為何用此名 / 軸 | Owner |
| --- | --- | --- | --- | --- | --- |
| **agent** (bare) = **golem agent** | Validated | golem agent(保留裸字 `agent`) | GAL 的專家 AI-agent 人格(golem-architect、golem-tester…) | **保留字。** 在 GAL 裸寫「agent」即指 golem agent —— 刻意的產品設計。golem agent 是真正的 AI agent(ISO 屬);它是 coding agent 所扮演的*角色*。**不是** coding agent、runtime 或 executor。 | `plugins/gal-core/agents/` |
| **runtime** | Validated | (coding agent,generation 軸) | GAL 鎖定的一個 **coding agent** | GAL 對 coding agent 的程式碼把手。維持 `runtime`(絕不裸寫 `agent`),因為 **`agent` 保留給 golem agent**。軸:*adapter 生成的目標*。 | `base::runtime`(`VALID_RUNTIMES`) |
| **executor** | Validated | (coding agent,dispatch 軸) | 為某 dispatch 階段以 headless 方式調用的 **coding agent** | 軸:*由誰執行此任務*。與 runtime 同一實體,不同角色。 | `crates/dispatch` |
| **model** = **LLM** | Validated | LLM(保留) | coding agent 使用的 LLM「大腦」 | **保留字。** 在 GAL,「model」固定只指 LLM(例 `claude-opus-4-8`)。由 executor-routing 選擇。 | `~/.gal/config/config.json#executorRouting` |
| **per-runtime MCP server config** | Validated | (取代 ~~MCP provider~~) | 為每個 runtime 寫的 MCP **host** 設定 | 取代錯誤的「MCP provider」。GAL 序列化每個 runtime 的 MCP host 設定(agent 是 host;它消費 servers)。子集:6 個 runtime 中的 5 個(不含 `gemini`)。 | `crates/mcp`(`mcp::serializers`) |
| **projection surface** (surface) | Validated | surface | runtime 在機器上看見 GAL 內容之處 | 軸:*內容落地之處*(skill / command / machine surface)。 | `crates/projection` |
| **projection** | Validated | — | 把正式內容具現到某 surface 的動作 | junction / symlink / copy / atomic-swap。 | `crates/projection` |
| **generated runtime adapter** | Validated | runtime adapter | 為某 runtime 生成、供其讀取的指令檔 | 是產品,不是程式碼模組。各 runtime 對應的檔案見成員表。 | `crates/projection` + `gal-engine::render` |
| **(file) projection backend** | Validated | — | 把 skills/commands 轉成 surface 檔案的程式碼 | 軸:*程式碼角色*。不是「executor adapter」。 | `crates/projection` |
| **canonical root** | Validated | — | 已 render 的超集 plugin 根 | `~/.gal/plugins/gal/`;安裝產物。 | `gal-engine::render` |
| **source root** | Validated | — | GAL 源碼目錄,透過 cwd-walk 自動解析 | 不需設定 `galRoot`/`devMode`。 | `crates/cli::render` |
| **sync / install / setup** | Validated | — | repo-local adapters / canonical-root render / machine surfaces | 三個不同的命令邊界(見 `docs/devguide.md`)。 | `projection` / `gal-engine` / `setup` |
| **source plan / execution prompt / state** | Validated | — | `.dev/plans/<slug>.md` / `.dev/plans/<slug>.prompt.md` / `.dev/state.md` | 三層計畫記憶(見 `plugins/gal-core/workflows/coding.md`)。 | repo |
| **dispatch / pipeline** | Validated | — | executor 派生(本地 + SSH 遠端 lane)/ 本地編排 | 編排 DAG:`dispatch ← pipeline`。遠端(SSH)執行是組合進 `dispatch::run` 的一條 spawn lane,不是獨立 crate(已退役的 `xmachine` crate 與 `pipeline::orchestration` 鷹架皆已移除)。 | 同名 crates |
| **plan-task ID** | Validated | T/R/TP/FU/BUG | `T/R/TP/FU/BUG` 識別字 | *出處(Provenance)。* 只允許出現在 `.dev/**`(Rule 4)。 | `.dev` |

> **已定案(2026-06-14):** 程式碼把手**維持 `runtime`**(不改名為 `coding_agent`)。裸 `agent` 因產品設計
> **保留給 golem agent**,故外部把手永遠不能是裸 `agent`;`runtime`(零變動)勝過加限定的 `coding_agent`。
> 正式外部術語 = **`coding agent`**(永遠加限定),內部把手 = **`runtime`**。

---

## Retired Terms (gate input)

供 naming gate(`crates/gal-engine/src/naming_gate.rs`,由 `infra-naming-authority` 計畫引入)讀取的機器可讀清單。
標記之間的每一非註解行都是一個**退役片語**,不得出現在長存 surface(除 `.dev/**` 之外的一切)。
比對為不分大小寫、整片語比對。plan-task ID 出處(`T-NNN`、`R-NN`、`TP-NNN`、`FU-NN`、`BUG-X`)由 gate 中另一條 regex 強制,
不列於此。

```text
# RETIRED-TERMS v1 — one retired phrase per line; lines starting with `#` and blank lines are ignored.
# Format is stable: the gate reads only this fenced block, by these BEGIN/END comment fences.
# RETIRED-TERMS:BEGIN
MCP provider
# RETIRED-TERMS:END
```

---

## Part D — Member registry(GAL 鎖定的 coding agents)

六個 `VALID_RUNTIMES` 全是 **coding agents**,不是 models。正式偏好順序對應 `base::runtime::VALID_RUNTIMES`。

| runtime key | Coding agent(產品) | 廠商 | 生成的 adapter 檔 | per-runtime MCP config? |
| --- | --- | --- | --- | --- |
| `copilot` | GitHub Copilot | GitHub / Microsoft | `.github/copilot-instructions.md` | yes |
| `antigravity` (`agy`) | Google Antigravity | Google | `.agents/rules/gal.md` | yes |
| `gemini` | Gemini CLI | Google | `GEMINI.md` | **no** |
| `codex` | OpenAI Codex | OpenAI | `AGENTS.md` | yes |
| `opencode` | opencode | open source (SST) | `AGENTS.md` | yes |
| `claude` | Claude Code | Anthropic | `CLAUDE.md` | yes |

> **觀察項(2026-06-13):** Google 正在把 **Gemini CLI → Antigravity CLI** 轉換;Gemini CLI 的消費者存取
> 預定於 **2026-06-18** 變更。GAL 同時列出 `gemini` 與 `antigravity` 為 runtime —— 待轉換落地後重新檢視是否合併。

---

## Sources

Part B 的權威錨點,依分層(2026-06-13 驗證)。

**Tier 1 — Standards bodies (neutral, formal):**

- ISO/IEC 22989:2022 — *Artificial intelligence — concepts and terminology* (ISO/IEC JTC 1/SC 42; defines "AI agent" + 110 terms): <https://webstore.ansi.org/standards/iso/isoiec229892022>
- NIST — AI Agent Standards Initiative (agentic AI terminology/governance): <https://www.nist.gov/>

**Tier 2 — Linux Foundation, Agentic AI Foundation (AAIF) (neutral steward, announced 2025-12-09; governs the two standards GAL implements):**

- LF press release — formation of the AAIF (MCP + goose + AGENTS.md): <https://www.linuxfoundation.org/press/linux-foundation-announces-the-formation-of-the-agentic-ai-foundation>
- Anthropic — donating MCP to the AAIF: <https://www.anthropic.com/news/donating-the-model-context-protocol-and-establishing-of-the-agentic-ai-foundation>
- OpenAI — co-founding the AAIF (AGENTS.md): <https://openai.com/index/agentic-ai-foundation/>
- MCP spec — Architecture (host / client / server roles): <https://modelcontextprotocol.io/docs/concepts/architecture>
- AGENTS.md — open standard for AI coding agents: <https://agents.md>

**Tier 3 — Vendor first-party (product-level / illustrative):**

- Anthropic — Claude Code ("agentic coding tool"): <https://code.claude.com/docs/en/overview>
- OpenAI — Codex ("coding agent"): <https://openai.com/codex/>
- Google — Antigravity ("agentic development platform") + Gemini CLI→Antigravity CLI transition: <https://developers.googleblog.com/build-with-google-antigravity-our-new-agentic-development-platform/>, <https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli/>

**Tier 4 — Encyclopedic (neutral, tertiary):**

- Wikipedia — *Large language model* (model vs agent vs chatbot): <https://en.wikipedia.org/wiki/Large_language_model>
