# GAL 與 gstack 的整合模型

## 這份文件是什麼

這份文件描述的是 **GAL 現在如何把 upstream gstack 當成 optional provider**，而不是如何把 gstack command surface 原封不動搬進 GAL。

如果你想知道：

- GAL 為什麼不再安裝 gstack-style planning commands
- 安裝 gstack 後，planning family 怎麼找到正確的 upstream skills
- GAL project files 和 provider artifacts 怎麼接線

就讀這份文件。

## 核心結論

- GAL 保留自己的 control plane 與 planning public surface：`/planning`、`/deep-planning`，以及在 implementation 前才執行的 `/plan-to-prompt`
- gstack 是 optional specialist provider，不是 GAL 的 public command surface
- planning-stage review 在 GAL 內以 **review lanes** 表達，不以 upstream command 名稱表達
- upstream gstack skill 名稱只存在於 provider routing 與 integration contract 中

## 為什麼不能直接沿用 gstack command surface

原因不是語意不相容，而是 ownership 與安裝面都不同：

1. gstack 的 host 假設是 Claude Code，不是 Copilot 的 control plane
2. GAL 必須維持 repo-local project files
3. 若 GAL 也重建 upstream 的 discovery 與 planning-review public surface，在同機安裝 upstream gstack 時會出現重複 planning commands

因此 GAL 不再把這些 upstream 名稱當作自己的 public command surface。

## 分層模型

### Layer A: GAL Public Surface

這一層是 GAL 自己負責的使用者入口。

| 類型 | Surface |
| --- | --- |
| Control plane | `/gal init`, `/gal status`, `/gal whats-next`, `/gal wrap-up`, `/gal research` |
| Planning family | `/planning`, `/deep-planning`, planning-stage review lanes, `/plan-to-prompt` |
| Other specialist commands | `/review`, `/qa`, `/ship`, `/design-review` 等 GAL-native skills |

### Layer B: Planning Review Lanes

這一層是 capability，不是固定 command 名稱。

| Lane | 用途 | Write-back target |
| --- | --- | --- |
| Business / Scope review | 檢查價值、範圍與優先順序 | source plan `## Review Results` + `## Open Questions` |
| Design review | 檢查 UX、state coverage、a11y、design-system fit | source plan `## Review Results` + `## Open Questions` |
| Engineering review | 檢查 architecture、test matrix、build readiness | source plan `## Review Results` + `## Open Questions` + `## Test Plan` + `## Tasks` + `<!-- ENG_REVIEW: CLEAR -->` |

### Layer C: Provider Routing

provider routing 發生在 workflow 層，而不是在 planning family 內直接寫死 gstack command 名稱。這些 review lanes 應被視為 specialized deep-planning passes，先寫回 source plan，再由 `/plan-to-prompt` 轉成 execution prompt。

| Review lane | Preferred provider when gstack is installed | Fallback when gstack is absent |
| --- | --- | --- |
| Business / Scope review | upstream business review provider | `/gal golem-analyst` |
| Design review | upstream design review provider | `/gal golem-designer` |
| Engineering review | upstream engineering review provider | `/gal golem-architect` |

如果未來需要完整 pipeline provider，可以另外在 workflow resolver 層把「full review pipeline」映射到 upstream 的 full planning review provider。但這個映射不應出現在 GAL 的 planning family user-facing contract 裡。

## 安裝與偵測 contract

GAL 對 gstack 只承認一種支援的安裝模式：Other AI Agents 模式。

```bash
git clone --single-branch --depth 1 https://github.com/garrytan/gstack.git ~/gstack
cd ~/gstack && ./setup
```

存在性檢查：

- `~/gstack/` 是否存在
- `~/gstack/setup` 是否存在
- `~/gstack/bin/` 是否存在

Windows 對應：

- `%USERPROFILE%\\gstack`
- `%USERPROFILE%\\.gstack`

這一層只回答「機器上是否有受支援的 gstack 安裝」，不直接回答「目前這個 repo 的 active plan 是否已有可採用的 review artifact」。

## Provider Artifacts 與 GAL Project Files

upstream provider 可以有自己的 project-scoped artifacts，但 GAL 的主要檔案仍維持 repo-local。

### GAL project files

- `.dev/state.md`
- `docs/plans/<plan-slug>.md`
- `.dev/plans/<plan-slug>.prompt.md`
- `DESIGN.md`
- `docs/designs/`
- `docs/qa-reports/`

### gstack provider artifacts

GAL 目前只承認這組最小 project-scoped contract：

```text
~/.gstack/projects/<slug>/
├── <branch>-reviews.jsonl
├── ceo-plans/
├── checkpoints/
├── designs/
├── evals/
└── learnings.jsonl
```

### 核心原則

- provider 產生內容可以被 GAL 讀
- 但正式 write-back 必須回到 GAL 自己的 plan files
- control plane 讀的是 GAL project files，不直接把 provider storage 當成主 state

## Review Log 欄位對應

當 `/gal` 檢查 gstack provider readiness 時，最重要的是 `<branch>-reviews.jsonl`。

所有 review entries 至少要有：

- `skill`
- `timestamp`
- `status`
- `commit`

lane 到 skill 的對應如下：

- engineering review lane → upstream engineering review provider 或 `review`
- design review lane → upstream design review provider
- business / CEO-style review lane → upstream business review provider

這些 skill 名稱是 provider contract，不是 GAL public command。

## Canonical Section Ownership

GAL 的 execution prompt 仍然維持自己的 section ownership：

- `## Open Questions`：由 `/planning` 建立 scaffold，review lanes 追加，只有 engineering review lane 可關閉
- `## Tasks`：只由 engineering review lane 初始化
- `## Analyze`：只由 `/review` 寫入

所以即使 provider 換了，`/gal status` 與 `/gal whats-next` 在 materialization 前後看的仍是同一套 canonical semantics：先看 source plan 的 planning-stage sections，materialization 後再看 execution prompt 的對應 sections。

## 結論

GAL 與 gstack 的整合方式不是共用 command surface，而是：

1. GAL 保留自己的 control plane 和 planning family
2. planning-stage reviews 以 lane 表達
3. lane 在 workflow 層決定要走 upstream gstack skill 還是 fallback golem
4. 結果先回寫到 source plan，完成 review 後再由 `/plan-to-prompt` 轉進 execution prompt

這樣才同時避免重複 commands、保留 provider 能力、又不失去 control-plane 可讀性。

這是 contract-driven orchestration，不是另一套 hidden workflow。
