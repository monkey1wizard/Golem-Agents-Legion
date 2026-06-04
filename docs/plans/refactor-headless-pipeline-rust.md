# Plan: Headless Pipeline 原生化（Rust bin）+ 五工具互派 + provider 原生紀錄 + 誠實測試

> **Supersedes（合併後刪除）**: 本計畫取代以下兩份舊計畫；**將其必要資訊合併進本計畫後，完整刪除這兩個檔案**（依使用者 2026-06-04 指示，非僅標記 superseded）——
> - `docs/plans/headless-cli-pipeline.md`（已標 VERIFIED/釋出）
> - `docs/plans/headless-cli-pipeline-test.md`
>
> 兩份舊計畫建立在一個**承重但錯誤的假設**上：「codex / copilot 無無頭模式，故恆 `non-dispatchable`」。該假設由架構審查重複「REAFFIRMED」三次，但從未實測。使用者實測 `claude "Hi" --model claude-haiku-4.5` 與 `codex "Hi" -m gpt-5.4-mini` 皆可執行，`codex exec` 是有文件的非互動入口，且 OpenAI 官方 `codex-plugin-cc`（見 References）已示範「Claude Code 委派 → codex 背景作業 → 回傳 codex session id 供 `codex resume` 在原生 codex 檢視」——排除假設已被證據推翻。
>
> **合併保留清單（從舊計畫帶入本計畫的必要資訊）**：durable executor-log 設計與終態詞彙（`completed`/`no-receipt`/`timeout`/`disconnected-partial`/`unavailable`）、task-spec（<5KB、stdin 餵入、明令不得 commit）、逾時行程樹回收、OFFLOAD 降級語義、bypass-permission 安全警告、收檔證明探針（token 寫回）、既有 spike 結果（claude `--model`、opencode `-m provider/model`、agy 待測）、既有測試矩陣與既有證據（`.dev/tc-02-receipt.txt`、`tc-04a-receipt.txt`、`tc-05-receipt.txt` 及 `.dev/executor-logs/` 對應 log；已存在的 `copilot.ps1` adapter）供 re-audit。刪檔在本 deep-planning pass 末、確認合併完成後執行。

## Goal

把 `/gal pipeline` 的無頭執行核心（routing 解析 → stage→role 對照 → task spec → dispatch 子行程 → 逾時/行程樹回收 → 終態分類 → 收檔驗證 → **provider 原生 session 紀錄擷取** → 降級）**以 Rust 預編譯 bin 原生重寫**，取代現有 PowerShell + Bash dispatch/executor 腳本層。一併：修正錯誤的 codex/copilot 排除、把**五個工具（claude / codex / copilot / opencode / agy）**都納為**可派工的真 Executor**、讓 `executor-routing.json` **完全取代 `model-roles.md`** 成為角色設定單一來源，最後以**可觀測、不可造假**的測試矩陣證明跨工具互派真的發生。

完成時必須同時成立四件事：

1. **單一設定來源貫通**：bin 讀 `executor-routing.json`（per-role `executor` + `model`，階段一已交付）作為 dispatch + model 注入的唯一來源；`model-roles.md` 與 `model-roles.example.md` 移除，其跨模型政策散文移入 `workflows/coding.md`。
2. **排除假設被證據取代**：五工具的「可否無頭寫回」由 spike 實測決定，不再由文件斷言。
3. **provider 原生可追溯**：派工到任一 provider 後，該次執行在**該工具自己的原生 session/歷史**中可被查看（擷取並記錄 provider session/job id，使用者可在原生工具 resume/檢視）。
4. **測試只認收檔**：任一測試格 PASS 的唯一標準是「可觀測的 dispatch + receipt」——目標檔被次級工具真的寫回、`.dev/executor-logs/` 終態為 `completed`、且該次有可追溯的 provider session 紀錄。process 起動 / 只看 exit 0 / 「orchestrator 自己也能做」/「--model 被接受」都**不算 PASS**。

## Scope

**In scope**：Rust dispatch bin 核心、routing JSON 讀取、stage→role 對照、task spec 產生、子行程 spawn + stdin 餵入 + 逾時行程樹回收、durable executor log + 終態分類、收檔驗證、provider 原生 session id 擷取、bin 自身輸出降級文字分派、**五工具 executor adapter（claude / codex / copilot / opencode / agy）**、`model-roles.md` → `executor-routing.json` 完全取代與政策散文遷移、誠實測試矩陣。

**Out of scope**：本機 ↔ 遠端 SSH 跨執行模式抽象（沿用舊計畫界定，留待後續）；`gal.ps1` 控制平面中**非 dispatch** 的部分（status / whats-next 等）的全面原生化（留待 `plugin-bin-migration.md` 候選 E）；在 bin 內建跨模型政策**規則引擎**（tier/distinct 強制執行）——政策維持人類可讀散文於 `workflows/coding.md`，bin 不做規則計算（YAGNI，刻意決定）。

> **分發決策（2026-06-04）**：交付**預編譯二進位（bin）**，使用者**無需安裝 Rust 工具鏈**。此 bin 為**單一跨平台 dispatch/executor 實作，取代現有 PowerShell 與 Bash 的 dispatch/executor 腳本層**；因單一 bin 原生跨平台執行，**不再需要平行的 Bash 通道**（單一 bin 原生於 *nix 執行）。降級用的文字分派（`--- GAL DISPATCH ---`）由 bin 自身輸出；bin 完全缺失時由薄 shim 輸出最小文字分派（見 Step 3）。
>
> **專案方針（2026-06-04）**：整個 project 的 ps1/bash **逐步以 bin 取代並刪除**——每段腳本在「確認 bin 可用」後才刪除對應舊檔；僅保留 bin 無法取代者。本計畫先交付 dispatch/executor 切片，其餘 ps1/bash 隨後續逐步替換（與 `plugin-bin-migration.md` 候選 E 對齊）。adapter 一律純 Rust，不留過渡 shell-out。

> **角色設定單一來源決策（2026-06-04 使用者指示）**：`executor-routing.json` **完全取代 `model-roles.md`**。JSON 承載 role→{executor, model} 的**映射**（機器讀取 + 人類參考）；`model-roles.md` 的跨模型**政策**（CODER≠TESTER、REVIEWER tier ≥ CODER、planning review 規則等關係型規則）**不塞進 JSON**，改併入 `workflows/coding.md`（該層本已重述大部分）。`model-roles.md`、`model-roles.example.md` 刪除，所有引用（33 檔，含生成的 adapter、docs、scripts、.gitignore）更新或由 sync 重生。**此為大範圍 doc 遷移，獨立成階段三、與 bin/測試解耦**。

## References（各工具官方無頭 / CLI 文件 + 先例）

- **GitHub Copilot CLI（程式化執行）**：<https://docs.github.com/en/copilot/how-tos/copilot-cli/automate-copilot-cli/run-cli-programmatically>
- **OpenAI Codex CLI（reference）**：<https://developers.openai.com/codex/cli/reference>
- **Claude Code CLI（reference）**：<https://code.claude.com/docs/en/cli-reference>
- **Antigravity CLI（agy）**：<https://antigravity.google/docs/cli-using>
- **OpenCode CLI**：<https://opencode.ai/docs/cli/>
- **先例 — OpenAI `codex-plugin-cc`**：<https://github.com/openai/codex-plugin-cc> — Claude Code 委派 codex 背景作業、回傳 session id 供 `codex resume` 原生檢視（JS plugin，`/codex:rescue|status|result`，讀 `~/.codex/config.toml`）。直接示範本計畫的 provider 原生可追溯需求；adapter 的 codex 路徑可借鏡其 `codex exec` 背景作業 + session-id 擷取模式。

## Requirements

- [ ] **Rust 核心讀 routing**：Rust dispatcher 讀 `~/.gal/config/executor-routing.json`，解析 `role → { executor, model }`；缺檔/角色缺項/malformed 皆安全降級（不致命、不靜默成功）。
- [ ] **預設安全閘**：bin 預設**不**派工到其他 provider；僅當該 provider 已**成功設置**（routing 有對應角色 + adapter 可用 + 通過寫回 spike）才 offload，否則 inline 自行處理或文字分派。未設置 / 未驗證的 provider 不得被自動嘗試。
- [ ] **stage→role 對照在原生層**：implement→CODER、test→TESTER、review→REVIEWER、verify→VERIFIER 落在 Rust 核心，不再只存在於 SKILL 散文或 PS。
- [ ] **真 dispatch + 收檔驗證**：Rust 核心 spawn 次級 CLI（spec 經 stdin 或工具規定方式餵入），逾時以行程樹回收，**以讀取目標檔內容確認寫回**才判定完成；不得只看 exit code。
- [ ] **五工具皆為真 Executor**：claude / codex / copilot / opencode / **agy** 各有 adapter，能接 spec、無頭執行、就地寫回。codex 以 `codex exec`、copilot 以 `copilot ... --allow-all -p`、opencode 以 `opencode run`/`-m`、claude 以 `claude -p --model`、agy 依官方 CLI 文件——**確切無頭寫回呼叫式由 spike 鎖定**。
- [ ] **spike 前置阻斷門**：在任何 adapter 寫產品碼或任何測試格被允許 PASS **之前**，必須先有一份 spike 證明該工具「無頭模式真的把結果寫回檔案」（非「CLI 回應了 --model」）。spike 不可驗證者明確標 env-unverifiable，且該工具不得在矩陣中記 PASS。
- [ ] **誠實測試門檻（硬要求）**：測試格 PASS = 目標檔被次級工具寫回 **且** `.dev/executor-logs/` 終態 `completed`。任何不滿足者記 ⬜ 未執行 或 ❌，並據實說明。「CLI 接受 --model」與「無頭寫回已驗證」是兩個不同主張，不可混淆。
- [ ] **orchestrator 能力與 executor 能力分開驗證**：(a) executor 收檔能力；(b) orchestrator 能否**自主驅動 `/gal pipeline`** 並由 routing JSON 決定派發。兩者各自獨立記錄，不得用 (a) 冒充 (b)。
- [ ] **預編譯 bin、零 Rust 依賴**：交付預編譯二進位，使用者無需安裝 Rust；bin 為單一跨平台實作，取代 PowerShell 與 Bash 的 dispatch/executor 腳本層。
- [ ] **降級不回歸（分兩層）**：(a) bin 存在但無 routing / 無對應 executor / 不可 dispatch → **bin 自身輸出** `--- GAL DISPATCH ---` 文字分派；(b) bin 完全缺失（未分發到該機）→ **薄 shim 輸出最小文字分派**。兩層皆向後相容、降級可觀測（留 `Dispatch:` 標記）。
- [ ] **可觀測足跡保留**：每次 dispatch 在 `.dev/executor-logs/` 留 durable 紀錄（stdout/stderr、起訖/耗時、exit、終態分類、實際傳入 model），保留供鑑識；終態詞彙沿用 `completed`/`no-receipt`/`timeout`/`disconnected-partial`/`unavailable`。
- [ ] **commit 邊界不變**：次級工具不得跨 commit 邊界；commit 由 orchestrator 持有。既有安全模型（重試上限、受保護路徑升級、有條件資安審查、bypass-permission 安全警告）不受影響。
- [ ] **provider 原生可追溯**：每次 dispatch 擷取並記錄該 provider 的原生 session/job id 到 executor-log header 與 `Dispatch:` 標記；使用者能在原生工具（如 `codex resume <id>`）查看該次處理紀錄。dispatch 不得使用會丟棄原生歷史的模式。
- [ ] **`executor-routing.json` 完全取代 `model-roles.md`**：JSON 為角色設定單一來源（映射）；政策散文移入 `workflows/coding.md`；`model-roles.md`、`model-roles.example.md` 刪除，引用全數更新或由 sync 重生；不得殘留指向已刪檔的連結。
- [ ] **copilot orchestrator/executor 不對稱**：copilot 作 orchestrator 由 **VSCode Copilot Chat** 發出；作 executor 一律由 **copilot CLI** 接收。矩陣須反映此不對稱。
- [ ] **舊兩份計畫合併後刪除**：必要資訊併入本計畫後，`headless-cli-pipeline.md` 與 `headless-cli-pipeline-test.md` 完整刪除（非僅標記）。

## Approach

### 階段零：spike 前置阻斷門（先驗證，零產品碼）

#### Step 0：五工具「無頭寫回 + 原生 session 可追溯」spike

- **Files**: 無產品碼；結果記入本計畫 `## Open Questions` 收斂註與實作註解。
- **What**: 逐一實測兩件事：(1) 該工具能否無頭接 spec 並把結果**寫回指定檔案**（非只「CLI 回應了」）；(2) 該次執行是否**留下可在原生工具查看/resume 的 session/job id**，並能擷取該 id。依官方 CLI 文件（見 References）確認旗標：
  - **claude**：`claude -p --model <m> --dangerously-skip-permissions`，spec 經 stdin；token 寫回核對；擷取 claude session（可 `--resume`）。
  - **codex**：`codex exec`（+ `-m`、自動核准/sandbox 旗標），token 寫回核對；擷取 codex session id（`codex resume <id>` 可在原生 codex 檢視）。借鏡 `codex-plugin-cc` 背景作業 + session-id 模式。**這格是推翻舊排除的關鍵**。
  - **copilot**：`copilot -C <workdir> --model <m> --allow-all -p "<spec>"`，token 寫回核對；擷取 copilot session（沿用前 session TC-04 觀察，正式重證）。
  - **opencode**：`opencode run` / `opencode -m <provider/model>`（含 `default_agent=build`），token 寫回核對；擷取 opencode session。
  - **agy**：依 Antigravity CLI 文件實測無頭呼叫式與 session 擷取（使用者表示目前可用）。
- **Verify**: 每工具有明確結論：{寫回通過？session id 可擷取？} 或標 env-unverifiable。**任一工具未通過寫回 spike，其 adapter 與測試格不得標 PASS。**

### 階段一：Rust dispatch 核心（原生化，承重結構變更）

> 結構變更（新增 Cargo 專案、新抽象、改 dispatch 路徑），已經 `/deep-planning` 架構審查 **APPROVE**。下列為實作範圍；確切 crate 邊界與整合點於實作時細化。

#### Step 1：Rust crate 骨架 + routing 讀取 + stage→role

- **Files**: `[NEW] rust/gal-dispatch/Cargo.toml` 與 `src/`（確切路徑於實作時細化）
- **What**: 新 Rust crate，提供：routing JSON 解析（`role → {executor, model}`，安全降級）、stage→role 對照、CLI 入口（吃 `--phase`/`--task`/`--workdir`/`--timeout`）。
- **Verify**: 對範例 `executor-routing.json` 回正確映射；缺檔/malformed 回明確「無路由」訊號而非 panic。

#### Step 2：spawn + stdin + 逾時行程樹回收 + durable log

- **Files**: `rust/gal-dispatch/src/`（dispatch 模組）、`.dev/executor-logs/`
- **What**: spawn 次級 CLI、spec 餵入、逾時以行程樹回收（避免 node/子行程孤兒）、擷取 stdout/stderr 落地 log header（起訖/耗時/executor/task-phase/git branch/HEAD/exit/實際 model/終態）。終態分類沿用既有五詞彙。
- **Verify**: echo mock → `completed`；逾時 mock → `timeout` 且無孤兒；非零退出 → `disconnected-partial`；缺 CLI → `unavailable`。

#### Step 3：收檔驗證 + 降級（降級由 bin 自身輸出）

- **Files**: `rust/gal-dispatch/src/`、`[MODIFY] scripts/gal.ps1`（dispatch 分支縮為呼叫 bin 的薄 shim）、`[MODIFY/REMOVE] scripts/gal.sh`、`scripts/executors/*` dispatch 腳本層
- **What**: dispatch 後讀目標檔內容確認寫回才判 `completed`；半寫/含糊一律失敗並降級。**dispatch 與降級皆由 bin 持有**：bin 存在但無 routing / 無對應 executor / 不可 dispatch → bin 直接輸出 `--- GAL DISPATCH ---` 文字分派並留 `Dispatch:` 標記。**若 bin 完全缺失**（未分發到該機），薄 shim 仍輸出最小文字分派，確保無 bin 機器也能降級。PowerShell / Bash 不再各自重複 dispatch 邏輯。
- **Verify**: 設 CODER→claude 時 bin 走原生 dispatch 並驗證寫回；移除 routing → bin 輸出文字分派；移除 bin → shim 輸出最小文字分派；同一 bin 於 *nix 與 Windows 行為一致。

#### Step 4：五工具 executor adapter（含 session id 擷取）

- **Files**: adapter 為 **native Rust 模組**（不走過渡 shell-out）
- **What**: 依 Step 0 spike 鎖定的無頭呼叫式，為 claude / codex / copilot / opencode / agy 各建 adapter；存在性檢查、逾時、model 旗標注入、權威結果＝就地寫回（stdout 僅訊號）、**擷取 provider session/job id 寫入 log header 與 `Dispatch:` 標記**。
- **Verify**: 裝了對應 CLI 時各 adapter 能 spawn 子行程並就地寫回、且 log 含可追溯 session id；未裝回不可用訊號。

### 階段二：誠實測試矩陣（後做，依賴階段零+一）

#### Step 5：測試前置一致性

- **Files**: `~/.gal/config/executor-routing.json`（使用者本機）
- **What**: 依案例設定 `role → {executor, model}`；`-WorkDir C:\Code\Golem-Agents-Legion` 帶入與目前相同工作目錄；確認 log header 的 git branch/HEAD 與工作目錄一致。
- **Verify**: 任一格執行後 log header 與目前環境一致。

#### Step 6：執行互派矩陣（誠實門檻）

- **測試輪替順序（使用者預期）**：以 claude 為 orchestrator 先發 test，再換 codex，再 copilot（**由 VSCode Copilot Chat 發出**），再 opencode、agy，依序輪替；每個工具都當一次 orchestrator、也至少被當一次 executor。
- **每格 PASS 三條件**：收檔 receipt + log `completed` + **可追溯 provider session id**。同時分層記錄 (a) executor 收檔、(b) orchestrator 自主驅動 `/gal pipeline` 並由 routing 決定派發、(c) provider 原生可追溯。**prompt 不可塞寫死的 dispatch 指令**——須讓工具自己跑 `/gal pipeline` 由路由 JSON 決定派發，才算真測 orchestrator 能力。
- **copilot 不對稱**：orchestrator 欄的 copilot = VSCode Copilot Chat；executor 欄的 copilot = copilot CLI。
- **Re-audit**: 重新審計舊計畫宣稱的三格通過——TC-02 → `.dev/tc-02-receipt.txt` + `.dev/executor-logs/20260604-120618-*-opencode.log`；TC-04 → `tc-04a-receipt.txt`；TC-05 → `tc-05-receipt.txt`——區分它們是否只證明了 (a) executor 收檔，而 (b) orchestrator-drives-pipeline 與 (c) provider 原生可追溯 仍未證。
- **Verify**: 逐格與預期相符且可由 log + `Dispatch:` 標記 + provider session id 回查；無法達成者據實記 env-unverifiable / 未執行。矩陣為 **5×4 = 20 格**（每工具當一次 orchestrator、其餘四工具各當一次 executor；copilot orchestrator=VSCode、executor=CLI），結果回填卷末 `## Test Matrix`。

### 階段三：`model-roles.md` → `executor-routing.json` 完全取代（與 bin/測試解耦，可獨立施作）

#### Step 7：政策散文遷移 + 刪檔 + 引用更新

- **Files**: `[DELETE] model-roles.md`、`[DELETE] model-roles.example.md`、`[MODIFY] workflows/coding.md`（**受保護路徑**，吸收跨模型政策散文）、`[MODIFY] .gitignore`（移除 `model-roles.local.md`）、`[MODIFY] scripts/Update-Personalization.ps1` / `scripts/update-personalization.sh`（停止播種 model-roles 範例）、`[MODIFY] scripts/Sync-DevContext.ps1` / `scripts/sync-dev-context.sh`（**受保護路徑**，生成 adapter 的 Model Roles 段改源自 routing JSON 或移除）、其餘 docs 引用更新。
- **What**: 角色映射責任交給 `executor-routing.json`（已是單一機器讀取來源）；政策散文（CODER≠TESTER 等）併入 `workflows/coding.md`；刪兩檔；掃 33 處引用全部更新或由 sync 重生，不得殘留死連結。
- **Verify**: repo 內無對 `model-roles.md`/`model-roles.example.md` 的有效引用；`gal init`/sync 後生成的 adapter 不含指向已刪檔的連結；`workflows/coding.md` 保留跨模型政策語意。

> **合併後刪舊計畫**：確認本計畫已含舊計畫必要資訊（見檔頭合併保留清單）後，`[DELETE] docs/plans/headless-cli-pipeline.md`、`[DELETE] docs/plans/headless-cli-pipeline-test.md`。

## Files to Create or Modify

- `[NEW] rust/gal-dispatch/Cargo.toml` + `src/` — Rust dispatch 核心原始碼（crate 路徑於實作時細化）
- `[NEW] 預編譯二進位（bin）` — 隨 GAL 分發、使用者無需 Rust 工具鏈；每平台預編譯產物，種入路徑與 setup 整合於實作時細化
- `[MODIFY] scripts/gal.ps1`、`[MODIFY/REMOVE] scripts/gal.sh` — dispatch 分支縮為呼叫 bin 的薄 shim（bin 缺失時輸出最小文字分派）；不再各自重複 dispatch / 降級邏輯
- `[KEEP] executor-routing.json` / `executor-routing.example.json` — 階段一已交付，bin 讀同一來源；升格為角色設定單一來源（取代 model-roles）
- `[REMOVE/縮減] scripts/executors/*` dispatch 腳本層 — 由 bin 的 native Rust adapter 取代；新增 codex、agy 路徑；既有 `copilot.ps1`/`claude.ps1`/`opencode.ps1` dispatch 邏輯收斂進 bin
- `[MODIFY] commands/gal-pipeline/SKILL.template.md`（**受保護路徑**）— 更新 Headless Executor Dispatch 段：bin 核心、五工具、provider session 可追溯、誠實測試門檻、降級由 bin/shim 輸出
- `[DELETE] model-roles.md`、`[DELETE] model-roles.example.md` — 由 `executor-routing.json` 取代；政策散文移入 `workflows/coding.md`
- `[MODIFY] workflows/coding.md`（**受保護路徑**）— 吸收 model-roles 的跨模型政策散文
- `[MODIFY] .gitignore`、`scripts/Update-Personalization.*`、`scripts/Sync-DevContext.*`（含 .sh 對應；**Sync/Setup 為受保護路徑**）— 停止播種/生成 model-roles，改以 routing JSON 為來源
- `[DELETE] docs/plans/headless-cli-pipeline.md`、`[DELETE] docs/plans/headless-cli-pipeline-test.md` — 合併必要資訊入本計畫後刪除

## Test Cases

- [ ] 五工具 spike：各有 {token 寫回核對、原生 session id 可擷取} 結論 / env-unverifiable
- [ ] Rust 核心：範例 routing 解析正確；缺檔/malformed 安全降級
- [ ] dispatch：echo mock `completed`；逾時 `timeout` 無孤兒；非零 `disconnected-partial`；缺 CLI `unavailable`
- [ ] 收檔驗證：半寫/缺寫一律失敗並降級
- [ ] provider 可追溯：每次 dispatch 的 log header + `Dispatch:` 標記含可 resume 的 session/job id
- [ ] bin/shim 整合：有 bin+routing 走原生 dispatch；無 routing → bin 文字分派；無 bin → shim 最小文字分派
- [ ] model-roles 取代：repo 無 `model-roles.md` 有效引用；政策散文在 `workflows/coding.md`
- [ ] 互派矩陣每格：PASS 僅當 receipt + log `completed` + 可追溯 session；(a)/(b)/(c) 分層記錄

## Success Criteria

- [ ] Rust dispatch 核心讀同一 `executor-routing.json`，完成 spawn→逾時回收→收檔驗證→降級全鏈，行為可由 `.dev/executor-logs/` 終態回查。
- [ ] claude / codex / copilot / opencode / agy 至少各有一格「可觀測 dispatch + receipt（log `completed`）+ 可追溯 provider session」的真往返，codex/copilot 排除假設由證據推翻。
- [ ] 無 routing / 無 bin 的機器上，`/gal pipeline` 退回文字分派，行為完全不變。
- [ ] **5×4 = 20 格**互派矩陣全數執行並回填卷末 `## Test Matrix`；每格分層記錄 (a) executor 收檔、(b) orchestrator 自主驅動 pipeline、(c) provider 原生可追溯；不可達成者記 env-unverifiable，屬有效輸出。
- [ ] 舊兩份計畫的必要資訊已併入本計畫並完整刪除；repo 內無對 `model-roles.md`/`model-roles.example.md` 的有效引用。

## Risks

- **bin 跨平台建置與分發**：需為各平台預編譯、簽章、種入 `~/.gal`，與 `plugin-bin-migration.md` 候選 E 對齊。緩解：交付**預編譯二進位（使用者無需 Rust）**；bin 可執行但無 routing → bin 輸出文字分派；**bin 完全缺失 → 薄 shim 輸出最小文字分派**，無 bin 機器仍無回歸。
- **觸及受保護路徑與已釋出腳本面**：改 `SKILL.template.md`、`gal.ps1`、executor adapter。緩解：deep-planning 架構審查放行；最小化、保持 OFFLOAD 與降級語義不變。
- **CLI 無頭旗標不穩定/sandbox 阻擋寫回**：codex `exec` sandbox、各家權限旗標可能擋檔案寫入。緩解：Step 0 spike 先實測再寫 adapter；不可驗證者不記 PASS。
- **orchestrator 驅動 pipeline 未知**：Copilot / OpenCode / codex 能否載入 GAL skill 並跑 `/gal pipeline` 未證。緩解：本身即測試標的，結果可記 env-unverifiable，不阻斷。
- **再次以假設冒充驗證**：本計畫的存在就是為修正此類錯誤。緩解：誠實測試門檻為硬要求，spike 為阻斷門，(a)/(b)/(c) 分層。
- **model-roles 刪除的大範圍 blast radius**：33 檔引用、含受保護路徑（`workflows/coding.md`、`Sync-DevContext`）與生成 adapter；漏改會留死連結或讓跨模型 guardrail（CODER≠TESTER）遺失。緩解：階段三獨立施作、與 bin/測試解耦；政策散文明確落 `workflows/coding.md`；以 grep 驗無殘留引用。
- **provider 原生可追溯不一定每家都行**：某些工具的無頭/背景模式可能不留可 resume 的 session，或 id 不易擷取。緩解：Step 0 spike 逐家確認；不可追溯者該格不記完整 PASS、據實標示。
- **範圍捆綁過大**：bin 重寫 + 五工具 + 解除排除 + model-roles 遷移 + provider 紀錄 + 刪兩計畫同在一計畫。緩解：已分階段（零 spike → 一 bin → 二 測試 → 三 doc 遷移），階段三與前段解耦；`/refining-plan` 可按階段切 task 批次。
- **逐步刪 ps1/bash 的回歸風險**：專案級以 bin 取代並刪除 ps1/bash，若刪在 bin 尚未完整覆蓋前會破功能。緩解：使用者規則——**確認 bin 可用後才刪**；無法被 bin 取代者保留；每次刪除前後跑既有 smoke。

## Open Questions

無 — 規劃階段的所有疑問已收斂，決策已內化於 `## Goal`、`## Scope`、`## Requirements`、`## Approach` 與 `## Review Results`。歷史討論可由本檔 git 歷史回查。

## Approval

- Human approval: **direction approved**（2026-06-04；鎖定決策：Rust bin as vehicle、五工具納入需 spike、誠實測試門檻、`executor-routing.json` 完全取代 `model-roles.md`、合併後刪兩計畫、provider 原生可追溯、copilot orchestrator=VSCode/executor=CLI）
- Architect review: **APPROVE**（2026-06-04；見下方審查；所有規劃疑問已收斂，無殘留阻斷）
- Additional domain review: [not requested]

## Review Results

### Architecture Review

#### Verdict: APPROVE *(2026-06-04，deep-planning 架構審查)*

方向受控且已分階段（零 spike 阻斷門 → 一 bin → 二 測試 → 三 doc 遷移）。先前的承重缺陷（codex/copilot 排除為未驗證假設）由 spike-first 設計與 `codex-plugin-cc` 先例消除。下列 flags 已在本次收斂中處理。

**2026-06-04 補（收斂補記）**：矩陣定為 **5×4 = 20 格**、使用者親測回填卷末 `## Test Matrix`；adapter 採**純 Rust**，確認可用後刪舊 ps1/bash——**專案級 ps1/bash→Rust 為標準方針**，本計畫交付 dispatch 切片，刪除一律在 bin 取代確認後才執行（控制回歸）。所有規劃疑問已收斂，無殘留阻斷。

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| Rust 預編譯 bin 取代 ps1/bash | 單一跨平台實作、消 Bash 平行港、為候選 E 鋪路 | 跨平台建置/簽章/分發複雜度 | OK（bin 缺失有 shim 降級） |
| `executor-routing.json` 完全取代 `model-roles.md` | 角色設定單一來源、消雙來源漂移 | 33 檔遷移、政策散文需另置 | OK（階段三解耦、政策落 coding.md） |
| 五工具皆真 executor（含 agy/codex） | 推翻錯誤排除、互派矩陣完整 | 每家旗標/sandbox 風險 | OK（spike 前置阻斷） |
| provider 原生可追溯為硬需求 | 使用者可在原生工具查看派工紀錄 | 限制 invoke 模式、需擷取 session id | OK（codex-plugin-cc 已證可行） |
| 合併後刪兩舊計畫（非標記） | 消死計畫、單一真實來源 | 刪前須確認資訊已併入 | OK（檔頭列合併保留清單） |

#### Over-engineering Flags

- **[OE-01]** 不要在 bin 內建跨模型政策規則引擎（tier ≥、distinct 強制）。只有 5 個角色、規則屬人類判斷且 `workflows/coding.md` 已重述 → JSON 承載映射、政策維持散文即可。**已採納**。

#### Bug Surface

- **[BUG-01] Medium（已納入 T-014/T-015）**：model-roles 刪除漏改引用 → 死連結或跨模型 guardrail 遺失。Fix：階段三 grep 驗無殘留、政策明確落 `workflows/coding.md`。
- **[BUG-02] Medium**：bin 是降級輸出者，bin 缺失時無人降級。Fix：薄 shim 在 bin 缺失時輸出最小文字分派（已入 Step 3 / Requirements）。
- **[BUG-03] Low（已納入 T-001~003 spike）**：provider 無頭模式可能不留可 resume session。Fix：Step 0 spike 逐家確認，不可追溯該格不記完整 PASS。

#### Missing from Plan（已於本次收斂補入）

- provider 原生 session id 擷取（需求 + Step 0/4 + 矩陣第三條件）。
- copilot orchestrator/executor 不對稱（VSCode Chat vs CLI）。
- task-spec / <5KB / 不得 commit / bypass-permission 警告等舊計畫必要資訊（檔頭合併保留清單）。

#### Recommended Changes（已採納）

1. model-roles 遷移獨立成階段三、與 bin/測試解耦。
2. bin 不做政策規則引擎（OE-01）。
3. Step 0 spike 同時驗「寫回」與「原生 session 可追溯」。

#### What's Good (keep these)

- **spike-first 阻斷門**把「假設冒充驗證」從根消除。
- **誠實測試三條件**（receipt + log completed + 可追溯 session）不可造假。
- **分階段 + 階段三解耦**控制 model-roles 大遷移的 blast radius。
- **借鏡 `codex-plugin-cc`** 而非重造背景作業 / session 擷取輪子。

### Business Review

Not requested。

### Design Review

Not requested（無 customer-facing UI）。

### Engineering Review

#### Verdict: CLEAR *(2026-06-04)*

可建構性確認，依賴鏈無環：

- **階段零（阻斷門）**：T-001（claude/opencode/copilot 寫回+session 補證）、T-002（codex 新 spike，推翻排除關鍵）、T-003（agy 寫回未證）皆獨立、零產品碼，為後續 adapter/測試的前置門。
- **階段一**：T-004（crate 骨架+routing+stage→role+CLI 入口）獨立；T-005（spawn/stdin/逾時行程樹/durable log）依 T-004；T-006（收檔驗證+session id 擷取）依 T-005+spike；T-007（安全閘+bin 降級輸出）依 T-004+T-006；T-008（五純 Rust adapter）依 spike+T-004；T-009（gal.ps1/gal.sh shim→呼叫 bin、bin 缺失走 shim）依 T-004~008；T-010（SKILL 受保護路徑承重變更）依 T-009 整合語意；T-011（確認後刪舊 dispatch ps1）依 T-009+T-010。
- **階段二**：T-012（測試前置）依 T-008+T-009；T-013（5×4=20 矩陣回填+re-audit）依 T-012+spike 通過。
- **階段三（解耦）**：T-014（政策散文→coding.md）獨立；T-015（刪 model-roles+33 引用遷移）依 T-014；T-016（收尾驗證）依 T-015。

受保護路徑提醒：T-010（`commands/gal-pipeline/SKILL.template.md`）、T-014（`workflows/coding.md`）、T-015（`Sync-DevContext`/`Update-Personalization`）觸及受保護路徑，已由架構審查放行並各自獨立成任務；實作須最小化、不擴大到其他內容。

阻斷門紀律：T-001~003 任一工具 spike 未過（寫回或 session 不可追溯），該工具的 adapter（T-008）與矩陣格（T-013）不得標 PASS。刪舊 ps1（T-011）一律在 bin 取代確認後才執行。Bash 延後（受 Bash 主機可驗證性限制），bin 原生跨平台覆蓋的部分不另寫 Bash。

<!-- ENG_REVIEW: CLEAR -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | manual | claude / opencode / copilot 無頭寫回已知，補證 token 寫回 + 原生 session id 可擷取/可 resume。 | T-001 |
| TP-002 | manual | codex `codex exec`（+`-m`+sandbox/核准）token 寫回 + session id（`codex resume`）；借鏡 `codex-plugin-cc`。 | T-002 |
| TP-003 | manual | agy 依官方 CLI 文件實測無頭寫回 + session 擷取；不可驗證標 env-unverifiable。 | T-003 |
| TP-004 | unit | Rust 對範例 `executor-routing.json` 回正確 `role→{executor,model}`；缺檔/角色缺項/malformed 安全降級不 panic。 | T-004 |
| TP-005 | unit | stage→role 對照：implement→CODER、test→TESTER、review→REVIEWER、verify→VERIFIER。 | T-004 |
| TP-006 | integration | echo mock→`completed`；逾時→`timeout` 無孤兒行程；非零→`disconnected-partial`；缺 CLI→`unavailable`；log header 齊備。 | T-005 |
| TP-007 | integration | 收檔驗證：半寫/缺寫一律失敗並降級；log header + `Dispatch:` 含 provider session id。 | T-006 |
| TP-008 | integration | 安全閘：未成功設置的 provider 不被自動嘗試；無 routing/無對應 executor → bin 自身輸出文字分派。 | T-007 |
| TP-009 | integration | 五純 Rust adapter：裝了對應 CLI 能寫回 + 擷取 session id；未裝回不可用訊號。 | T-008 |
| TP-010 | integration | gal shim：有 bin+routing 走原生 dispatch；無 routing→bin 文字分派；無 bin→shim 最小文字分派；*nix/Windows 一致。 | T-009 |
| TP-011 | manual | SKILL 靜態審查：bin 核心、五工具、provider 可追溯、誠實三條件、降級由 bin/shim、commit 邊界、bypass 警告；受保護路徑變更已標明。 | T-010 |
| TP-012 | integration | 刪除被取代的舊 dispatch ps1 後，既有 smoke 不回歸；bin 無法取代者仍保留。 | T-011 |
| TP-013 | manual | 測試前置：routing 依案例設定、`-WorkDir` 一致、log header git branch/HEAD 與目前一致。 | T-012 |
| TP-014 | manual | 5×4=20 矩陣全數回填；每格 PASS 須 receipt + log `completed` + 可追溯 session（3 層驗收）；(a)/(b)/(c) 分層；re-audit 舊 TC-02/04/05。 | T-013 |
| TP-015 | manual | `workflows/coding.md` 保留跨模型政策語意（CODER≠TESTER 等）；`executor-routing.json` 含人類參考段。 | T-014 |
| TP-016 | integration | grep 無 `model-roles.md`/`model-roles.example.md` 有效引用；`gal init`/sync 後生成 adapter 無指向已刪檔的死連結。 | T-015 |
| TP-017 | manual | 舊兩計畫已移除、`.dev/state.md` 指向新計畫、repo 無殘留引用。 | T-016 |

## Tasks

### 階段零：spike 阻斷門（零產品碼，先驗證）

- [x] T-001 — claude / opencode / copilot 無頭寫回 + 原生 session id 擷取/可 resume 補證（寫回已知，補證可追溯）。*(2026-06-04)*
- [x] T-002 — codex 無頭 spike：`codex exec`（+`-m`+sandbox/核准旗標）token 寫回 + session id 擷取（`codex resume`），借鏡 `codex-plugin-cc`。**推翻舊排除關鍵**。*(2026-06-04: PASS)*
- [x] T-003 — agy 無頭 spike：依 Antigravity CLI 文件實測寫回 + session 擷取；不可驗證標 env-unverifiable。*(2026-06-04: PASS — brain dir scan)*

### 階段一：Rust bin 核心

- [x] T-004 — 建 `rust/gal-dispatch` crate：routing JSON 解析（`role→{executor,model}`，安全降級）+ stage→role 對照 + CLI 入口（`--phase/--task/--workdir/--timeout`）。*(77fef32)*
- [x] T-005 — spawn 次級 CLI + stdin 餵 spec + 逾時行程樹回收 + durable executor log（header + 終態五分類 `completed`/`no-receipt`/`timeout`/`disconnected-partial`/`unavailable`）。*(9a9371c)*
- [x] T-006 — 收檔驗證（讀目標檔內容才判 `completed`，半寫一律失敗）+ 擷取 provider session/job id 寫入 log header 與 `Dispatch:` 標記。*(56548fe)*
- [x] T-007 — 預設安全閘：僅當 provider 成功設置（routing 有角色 + adapter 可用 + spike 通過）才 offload，否則 inline；bin 自身輸出 `--- GAL DISPATCH ---` 降級文字分派。*(b102793)*
- [x] T-008 — 五**純 Rust** executor adapter（claude / codex / opencode / copilot / agy）：依 spike 旗標、存在性檢查、逾時、model 注入、就地寫回（stdout 僅訊號）、擷取 session id。**`codex` adapter 為新增**。*(b102793)*
- [x] T-009 — `scripts/gal.ps1` 與 `scripts/gal.sh` dispatch 分支縮為呼叫 bin 的薄 shim；bin 缺失→shim 輸出最小文字分派；不再各自重複 dispatch/降級邏輯。*(406abac)*
- [x] T-010 — 更新 `commands/gal-pipeline/SKILL.template.md`（**受保護路徑**）：bin 核心、五工具、provider 可追溯、誠實三條件、降級由 bin/shim、commit 邊界、bypass-permission 警告。*(caf353d)*
- [x] T-011 — **確認 bin 可用後**刪除被取代的舊 dispatch 腳本（`scripts/executors/*` dispatch 部分、`Invoke-Executor.ps1`）；bin 無法取代者保留；刪前後跑既有 smoke 防回歸。*(0d1e056; 7 個腳本刪除; 40/40 smoke 通過)*

### 階段二：5×4 互派測試矩陣

- [x] T-012 — 測試前置：`~/.gal/config/executor-routing.json` 依案例設定、`-WorkDir` 帶入相同工作目錄、log header 與目前環境一致。*(session_id log header 修正; 46 tests pass)*
- [ ] T-013 — 執行 5×4=20 格互派矩陣，回填 `## Test Matrix`；每格 3 條件 PASS（receipt + log `completed` + 可追溯 session）；(a) executor 收檔、(b) orchestrator 自驅 pipeline、(c) provider 原生可追溯 分層記錄；re-audit 舊 TC-02/04/05。

### 階段三：`model-roles.md` → `executor-routing.json` 取代（解耦）

- [x] T-014 — 跨模型政策散文（CODER≠TESTER、tier 等）遷入 `workflows/coding.md`（**受保護路徑**）；`executor-routing.json` 補人類參考段，升為角色設定單一來源。*(d7a8a5c)*
- [ ] T-015 — 刪 `model-roles.md`、`model-roles.example.md`；更新 33 處引用（停 `Update-Personalization.*` 播種、`Sync-DevContext.*` 生成段改源/移除、`.gitignore`、docs）；grep 驗無殘留死連結。
- [ ] T-016 — 收尾：確認舊兩計畫已移除、`.dev/state.md` 指向新計畫、repo 無殘留引用。

> **Deferred**：Bash 延後（受 Bash 主機可驗證性限制）；因 bin 原生跨平台，*nix dispatch 由 bin 覆蓋，僅 `gal.sh` 薄 shim（T-009）需處理，不另寫 Bash adapter。需 Bash 主機驗證的 smoke 留待有主機時補。

## Test Matrix（互派結果回填表）

> 使用者逐一開啟每個工具當 orchestrator、跑 `/gal pipeline` 由 routing JSON 派發到 executor，實測後回填。
> **PASS 三條件**：receipt 寫回 + `.dev/executor-logs/` 終態 `completed` + 可追溯 provider session id。任一缺即非 PASS。
> copilot 作 orchestrator = **VSCode Copilot Chat**、作 executor = **copilot CLI**。
> **驗收程序（3 層，缺一不可）**：(1) orchestrator 端 / 本 chat 可見派發；(2) 我方核對 `.dev/executor-logs/` 終態 `completed` + receipt 寫回；(3) **使用者進入目標工具原生介面，確認該次處理紀錄出現 / 可 resume**（使用者實測：原生 CLI 使用時本會跳出處理紀錄，故委派執行亦須留下同樣紀錄，看不到即不通過）。
> 狀態：✅ PASS / ❌ FAIL / ⬜ 未執行。

| TC | Orchestrator | Executor | 預期 | 實際走法 | 證據（receipt / log / session id） | 狀態 |
| --- | --- | --- | --- | --- | --- | --- |
| TC-01 | claude | codex | Offload | | | ⬜ |
| TC-02 | claude | copilot(CLI) | Offload | | | ⬜ |
| TC-03 | claude | opencode | Offload | | | ⬜ |
| TC-04 | claude | agy | Offload | | | ⬜ |
| TC-05 | codex | claude | Offload | | | ⬜ |
| TC-06 | codex | copilot(CLI) | Offload | | | ⬜ |
| TC-07 | codex | opencode | Offload | | | ⬜ |
| TC-08 | codex | agy | Offload | | | ⬜ |
| TC-09 | copilot(VSCode) | claude | Offload | | | ⬜ |
| TC-10 | copilot(VSCode) | codex | Offload | | | ⬜ |
| TC-11 | copilot(VSCode) | opencode | Offload | | | ⬜ |
| TC-12 | copilot(VSCode) | agy | Offload | | | ⬜ |
| TC-13 | opencode | claude | Offload | | | ⬜ |
| TC-14 | opencode | codex | Offload | | | ⬜ |
| TC-15 | opencode | copilot(CLI) | Offload | | | ⬜ |
| TC-16 | opencode | agy | Offload | | | ⬜ |
| TC-17 | agy | claude | Offload | | | ⬜ |
| TC-18 | agy | codex | Offload | | | ⬜ |
| TC-19 | agy | copilot(CLI) | Offload | | | ⬜ |
| TC-20 | agy | opencode | Offload | | | ⬜ |
