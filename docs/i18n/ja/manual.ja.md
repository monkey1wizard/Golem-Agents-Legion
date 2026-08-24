---
source: docs/manual.md
lang: ja
source_commit: fb6a41930fdbbd4069ed2d76725be9c81f6e5cca
translated_at: 2026-08-15
status: current
---

# GAL ユーザーマニュアル

[English](../../manual.md) · **日本語** · [繁體中文](../zh-Hant/manual.zh-Hant.md)

GAL の日常利用ガイドである。インストール、リポジトリの初期セットアップ、ワークフロー操作、設定、パーソナライズ、ヘッドレスなエグゼキューター (executor) のルーティング、golem エージェントを扱いる。

本マニュアルは **GAL の操作**を詳述する。システムアーキテクチャ（コードベースと `~/.gal/` のトポロジー、リリース系譜）は [`docs/architecture.md`](../../architecture.md) に、メンテナー向けの手順（配布の仕組み、プロバイダー向けパッケージング、変更ガイドライン）は[開発者ガイド](../../devguide.md)に記載されている。これらのトピックは本マニュアルの対象外である。

## 概要

GAL は `~/.gal/plugins/gal/` を正規ルート (canonical root) のプラグインルートとして用いる。プロバイダー可視の対象は、コンテンツの所有者ではなく投影 (projection) として機能する。設定は `~/.gal/config/config.json` に保存される。`~/.gal/` のレイアウトと所有権境界については [architecture → `~/.gal/` runtime layout](../../architecture.md#gal-runtime-layout) を参照すること。

`gal` バイナリは、`.git` を境界とする現在の作業ディレクトリ探索と、その後のバイナリ側のパッケージ済みレイアウトを通じて、ソースルート (source root) を自動的に特定する。`devMode` と `galRoot` の設定キーは、必要とされず、読み取られもしない。

## Installing GAL

適切なプラットフォームのインストールオプションを選択すること。インストール後、リポジトリ内で `gal init` を実行してリポジトリ本機のアダプター（`CLAUDE.md`、`AGENTS.md` など）を生成する。[リポジトリでの初回実行](#リポジトリでの初回実行) を参照すること。

### インストールオプション

#### `cargo install --git`（ソースから）

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli
```

GAL は crates.io から除外されているため、`--git` フラグが必須である。Cargo ワークスペース内では `gal-cli` パッケージ名が必要である。インストールされるバイナリの名前は `gal` である。

#### Homebrew（macOS / Linux）

```bash
brew install monkey1wizard/tap/gal
```

これは macOS と Linux の標準的なパッケージマネージャーによるインストール方法である。

#### winget（Windows —— 先に確認）

```sh
winget install Monkey1Wizard.GAL
```

カタログの可視性がリリースに遅れる場合があるため、インストール前に `winget show Monkey1Wizard.GAL` で入手可能性を確認すること。ここでは `Get-Alias gal` によるシャドウの制限が適用されます。

#### 直接ダウンロード（リリースアーカイブ）

GitHub のページから `gal-<version>-<platform>-<arch>[.zip|.tar.gz]` をダウンロードし、内容を展開して、`gal`（または `gal.exe`）を `PATH` に追加します（macOS / Linux では `chmod +x gal` で実行権限を付与してください）。

#### curl（Linux / macOS）

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
```

スクリプトは `checksums.txt` に対して SHA-256 チェックサムを検証し（必須要件であり、不一致なら中止）、続いて cosign によるキーレス署名の検証を行う。署名検証は可用性については best-effort ですが、結果については厳格である。cosign が存在しない、または署名ファイルのダウンロードに失敗した場合、インストーラーは警告を発し、SHA-256 の結果のみを用いて続行する。cosign が実行され署名の不一致を報告した場合、展開や書き込みの前にインストールを中止する。バイナリは `~/.local/bin` に、GAL ソースペイロードは `~/.local/share/gal` にインストールされます。バージョンの上書きには `GAL_VERSION` を用いる。

#### irm（Windows）

```sh
irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
```

この方法は curl 経路と SHA-256 および cosign の検証の意味論を共有し、署名チェックの失敗時にはインストールを中止する点も同じである。`gal.exe` と GAL ソースペイロードを `%LOCALAPPDATA%\Programs\gal` に直接インストールします（このディレクトリをユーザーの `PATH` に追加すること。インストーラーが正確なコマンドを出力します）。バージョンの上書きには `$env:GAL_VERSION` を用いる。

注意：PowerShell の組み込みの `Get-Alias gal` がバイナリをシャドウすることがある。`gal` が別の対象に解決される場合は、絶対パスで実行ファイルを呼び出すること。

#### Claude / Codex マーケットプレイスから（チャット主導のバイナリインストール）

代わりに、GAL をプラグインとして見つけて、AI にセットアップの完了を承認することもできます。

1. Claude Code または Codex のプラグインマーケットプレイスで **GAL プラグイン**を見つけてインストールするか（「gal」で検索）、[マーケットプレイススナップショットブランチ](https://github.com/monkey1wizard/golem-agents-legion/tree/marketplace-snapshot)から追加する。
2. **「help me install gal」** と依頼してインストールを要求する。プラグインは `install-gal` スキルを提供する。同意すると、OS に適したオプション（Homebrew / winget / `cargo install --git`）を実行し、結果を検証し、プロセスを `gal init` へ導きます。オプションが利用できない場合は、そのまま報告されます。

スタンドアロンのプラグインだけでは、機能する GAL のインストールにはなりない。`install-gal` を除くすべてのコマンドは `gal` バイナリを必要とするため、インストールの完了には手順 2 が必須である。`install-gal` スキルはバイナリを同梱せず、実行前にコマンドのプレビューを要求し、事実に基づく成功状態のみを厳格に報告する。

---

引数なしで `gal` を実行すると、使い方が表示され、終了コード 0 で終了する。認識されないサブコマンドは終了コード 64 で失敗する。

### アップグレード（`gal update`）

`gal update` コマンドは、インストール済みのバージョンとプラットフォーム固有のアップグレード手順を出力する。自己更新はサポートされていない。アップグレードは元のインストール方法を用いる必要がある。

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli  # cargo
winget upgrade Monkey1Wizard.GAL                                                   # Windows
brew upgrade gal                                                                   # macOS / Linux
```

## リポジトリでの初回実行

### `gal init`

`gal init` コマンドは、初回実行時に `gal-core` テンプレートから 5 つのリポジトリ本機アダプタールート（`CLAUDE.md`、`AGENTS.md`、`GEMINI.md`、`.github/copilot-instructions.md`、`.agents/rules/gal.md`）を生成し、`.dev/` ディレクトリの骨格を作成する。`--force` フラグ付きで実行すると、アダプターを冪等に再生成する。

実行時には、触れたアダプターパスごとに 1 行を出力し、`Written`（内容の作成または変更）、`Unchanged`（バイト単位で同一、書き込みをスキップ）、`Removed`（選択されなかった、陳腐化した GAL 所有の条件付きレイヤー）に分類する。

> ⚠️ `--force` フラグはブートストラップ上書きを開始する。この操作は `.dev/project.md` をテンプレートで完全に置き換え、既存のリポジトリのプロジェクト内容を破棄する。初期化エラーを回避するためにこのフラグを使わないでこと。報告された問題は直接解決してください（以下を参照）。

### プロジェクト内の `.dev/project.md`

`.dev/project.md` ファイルは、アダプターのレンダリングに用いられる圧縮されたプロジェクト要約として機能する。8 つの H2 見出しは、**それぞれ厳密に一度ずつ必須**である。`What This Is`、`Tech Stack`、`Architecture`、`Constraints`、`Response Style`、`Freshness`、`Project Language`、`Protected Paths` である。

この要件は fail-closed（既定で遮断）として動作する。見出しの欠落や重複があると、`gal init` はレンダリング全体を拒否し、問題の見出しを特定して、アダプターファイルの書き込みを停止する。エラーは 1 回の実行につき欠落見出しを 1 つ報告する。複数の見出しが欠けているファイルは、反復的な修正パスが必要である。特定された見出しを（`plugins/gal-core/templates/project.md` の形式を参照して）手動で追加し、再実行して、以降の見出しを追記すること。

`.dev/project.md` ファイルには厳格なサイズ上限が課されます。サイズに基づく拒否は、実行の再試行ではなく内容の刈り込みを必要とする。レンダリング処理は、サイズ制約を回避するために部分的なアダプターセットを書き込むことを厳格に避ける。

### `gal doctor`

`gal doctor` コマンドは、バイナリ、正規ルート、実行環境の面、設定を含めて、ローカルセットアップの健全性を検証する。インストール後、アップグレード後、またはエージェントが GAL コマンドを検出できないときに、このコマンドを実行すること。

`gal doctor` は次の項目を検査する。

- **OpenCode 投影ドリフト** — `~/.config/opencode/` のコマンドとエージェントのファイルが正規ルートのレンダリング結果と一致しなくなった状態を検出する。古い内容は警告、欠落したファイルはエラーとして報告する。修復するには `gal refresh` を実行する。
- **Claude スキルサーフェス** — `~/.claude/skills/gal` がない場合、エラーではなく警告と手動作成手順を報告する。`gal refresh` はこのサーフェスを自動作成しない。

初期化済みのリポジトリ内では、5 つのアダプタールートを対象とする読み取り専用のアダプターサイズ助言表も追加で出力する。サイズ超過のアダプターはエラーではなく `[WARNING]` の所見を生成し、単発の実行失敗を防ぐ。

ヘッドレスなコーディングエージェント実行のためのオプトインのセルフテストについては、[Executor Self-Test](#executor-self-test-gal-doctor---executor-smoke) を参照すること。

### 実行環境ごとの GAL コマンドの起動

GAL コマンドは実行環境固有の仕組みで初期化されるため、トリガーが異なる。

| 実行環境 | トリガー | 備考 |
| --- | --- | --- |
| Claude Code | `/gal status` | ネイティブのプラグインコマンド |
| Codex | `$gal-status` | スキルとして公開（`$` 接頭辞、または `/skills`） |
| Copilot | `/gal-status` | スキルとして公開（一覧は `/skills list`） |
| Antigravity | `/gal-status` | スキルとして公開（Antigravity にはネイティブの `commands/` フォルダーがなく、コマンドは Agent Skills） |
| OpenCode | `/gal-status` | ネイティブの Markdown コマンド |

構文の違いに注意すること。Claude 以外の実行環境は、`gal status`（スペース）ではなく `gal-status`（ハイフン）を用いる。

### コマンド変更が反映されるタイミング

コマンド更新の可視性は、実行環境の読み込みの仕組みに依存する。

- **Claude Code** はプラグインをキャッシュする。更新後のコマンドを読み込むには **Claude Code を再起動**すること。
- **Antigravity** は起動時にコマンドを登録する。変更を読み込むには **agy を再起動**すること。
- **Copilot / OpenCode** は、新しいセッションごとにコマンドとスキルのファイルを直接読み取ります。
- **Codex** は、ドキュメントに記載された主要な動作として、アクティブなスレッド内でスキルの変更を自動検出する。変更が現れない場合のフォールバックとして、**Codex を再起動するか、新しいスレッドを開始**すること。

> **サポート境界：** GAL は Gemini CLI にコマンドまたはスキルを投影しません。廃止された Gemini CLI surface の利用者は Antigravity へ移行する必要があり、そのコマンド surface は `~/.gemini/antigravity-cli/skills/<name>/SKILL.md` です。

主要な Codex の挙動は以下のとおりである。

- **コンテキスト予算による省略は失敗ではない。** Codex は初期のスキル一覧を制限する。この制限を超えると、説明が短縮され、続いてスキルが一覧から省略されます。省略されたスキルは `$skill-name` で直接呼び出せる。
- **同名の二重表示。** 同一の名前でスキルを投影する複数のツールは、Codex のマージ機能を回避する。スキルセレクターに両方の項目が現れることがある。

## ワークフローの実行

ワークフローのフローチャートとフェーズ概要は [README](README.ja.md#gal-の仕組み) にある。この節では特に、ユーザーに求められる判断、手動入力、ワークフロー中断時に必要な操作を詳述する。

### 計画：あなたの判断

- `/planning` コマンドは、リクエストを `.dev/plans/<type>-<slug>.md` にあるソースプランへ変換する。計画フェーズはコラボレーションをサポートし、ユーザーは計画を自由に議論・統合・分割し、同時に golem エージェントへ相談できます。
- **未解決の問いはユーザーの解決が必要である。** `/deep-planning` コマンドは、`/refining-plan` の実行前にすべての `## Open Questions` 項目の解決を必須とする。問いはクラスゲーティングに従う。**H** クラスの問いは人間のみの権限を要求し、エージェントによるクローズを禁止する。**A** クラスの問いは、記録された根拠を伴う architect 役割のクローズを許する。**F** は偽の問いを示す。曖昧な問いは、ユーザーの入力を待つ H クラスに既定で分類されます。
- **承認には明示的な確認が必要である。** `/refining-plan` の収束後、計画の `## Approval` 節に固定順序の4行として承認を記録する: `- Human approval: [pending|approved]`、`- Architect review: [pending|clear|blocked|not-required]`、`- Design review: [not-requested|clear|blocked]`、`- Business review: [not-requested|clear|blocked]`。`/plan-to-prompt` コマンドは、`- Human approval: [approved]` という文字どおりの行を必須とし、この行を欠く実行プロンプト生成を拒否する。
- **複数のアクティブな計画は対象の指定を要求する。** 同時にアクティブな計画がある場合、計画コマンドの呼び出し時に計画ファイルを明示的に指定する必要がある。GAL は自動選択を厳格に避ける。

### Pipeline：開始・停止・再開

- **開始：** `/gal pipeline` を実行する。アクティブな計画が単一なら、実行プロンプトを自動的に解決する。複数のアクティブな計画がある場合は、プロンプトファイル（`.dev/plans/<slug>.prompt.md`）の明示的な指定が必要である。
- **実行フェーズ：** pipeline は、タスクごとに実装・テスト・監査・コミットの順序に従って、タスクを自律的に反復する。ワークフローが中断しない限り、ユーザーの介入は不要である。
- **修正の再試行には、新しい権限指示と実際の実装変更が必要である。** テストまたは監査が失敗すると、pipeline は一つの OPEN retry handoff を記録し、`--fix` 付きで implement を再ディスパッチする。task goal、handoff、affected-file allowlist、agent contract が前回と同じ場合、executor の spawn 前に再試行を拒否する。executor が完了しても affected implementation file が一つも変わらなければ、そのラウンドは非ゼロの `fix-round-no-change` で終了する。prompt、receipt、replay sidecar、executor log への書き込みは実装変更として数えない。
- **中断条件：** pipeline は、人間の判断を要するブロッカー、再試行の上限（検証失敗 3 回）に達したタスク、または設定された working-hours のハードストップに対してのみ停止する。中断時には、実行プロンプトが停止点と保留中の要件を詳述する中断フェーズのメモを記録する。
- **再開手順は再実行を要求する。** ブロッカーまたは根本原因を解決してから、同一の `/gal pipeline` コマンドを再実行する。replay refusal の場合は、再開前に記録済みの OPEN handoffへ実質的に異なる問題または次の操作を記述しなければならない。時刻や表示用 metadata だけを変更しても拒否は解除されない。実行は記録されたカーソルから再開する。完了済みのタスクは再実行されない。

### Finalize：着地するものと削除されるもの

`/gal finalize` コマンドは、完了した計画のクローズをオーケストレートする。前提条件として、すべてのタスクの完了と検証が強制され、それはゼロトラストの機械レシート（`gal finalize-check`）で証明されます。

- **着地するもの：** doc-sync の前に、タスク横断の総合的なブランチレビューが先行する。STEWARD エージェントは、計画の永続的な知識を `README.md` と `docs/` へ抽出しなければなりない。その後、main へのマージが続き、該当する場合はワークツリーの撤去を含みます。
- **削除されるもの：** `.dev/plans/` 内の計画ファイルは、ドキュメントがコミットされ、書き込み後の衛生チェックがパスした後にのみ削除されます。これにより、ファイル消去の前に永続レイヤーへの知識移転が保証されます。
- **保持されるもの：** 日付、計画、着地コミットを含むクローズアウトの行が `.dev/state.md` に書き込まれます。`gal-last-good` タグが着地コミットに割り当てられます。

### Wrap-up と Finalize の違い

`/gal wrap-up` コマンドは、着地ではなく**一時停止**として機能する。セッションの引き継ぎメモを実行プロンプトへ圧縮し、`.dev/state.md` 内のセッション連続性を更新し、コミットを実行する。これにより、どの実行環境でも一時停止点から正確に再開できます。このコマンドは何もクローズしない。計画途中の停止にはこのコマンドを実行すること。`/gal finalize` は完了した計画にのみ用いてこと。

## 設定（`~/.gal/config/config.json`）

機械本機の値は `~/.gal/config/config.json` に存在する。保持する機械パスは `galSkills`、勤務時間は `workingHours` にマッピングされます。その他のキーには `planLanguage`、`memoryHarvest`、`executorRouting`（[Headless Executors](#ヘッドレスエグゼキューター) で詳述）がある。ローカルの値を、追跡対象のドキュメント、コマンドテンプレート、ソースファイルに書き込むことは厳格に禁止する。

| プレースホルダー | 意味 | 一般的な用途 |
| --- | --- | --- |
| `<WORKING_HOURS_ENABLED>` | working-hours の強制が有効かどうか | オプトインの wrap-up とハードストップの強制 |
| `<WORKDAY_START>` / `<WORKDAY_END>` | `HH:MM` 形式の希望する勤務日 | Working Hours のスケジュール / After Hours の境界 |
| `<WRAP_UP_TIME>` / `<HARD_STOP_TIME>` | `HH:MM` 形式の wrap-up とハードストップ | シャットダウン期間 / 作業停止の動作 |
| `<GAL_SKILLS>` | 機械本機の GAL スキルディレクトリへの絶対パス | git filter と機械本機スキルの投影 |

### Secrets と MCP の上書き

追跡対象の `plugins/gal-core/mcp.json` が、gal 自身の MCP server の単一のソースである。`gal refresh` はこれを canonical `.mcp.json` へ逐語的にコピーし、`~/.gal/local/mcp.json` の個人 server をマージする。別個の上書きファイルはサポートされていない。

gal はプレースホルダーの置換を一切行わない。manifest に書かれた `${ENV_VAR}` はそのまま引き継がれ、解決されるとすればそれはファイルを読み込む MCP host 側であり、通常はプロセス環境変数から解決される。この種の変数は環境側に設定し、`config.json` には置かないこと。

Playwright MCP については、保守的で機械非依存な追跡対象エントリを維持すること。headed モード、ビューポートとデバイスエミュレーション、storage-state のパス、出力ディレクトリ、永続プロファイル、拡張機能と CDP の配線などのローカル専用ブラウザ動作パラメーターは、追跡対象 manifest 内の `${ENV_VAR}` プレースホルダーで実装する。これらは `config.json` から直接解決されます。

```json
{
  "servers": {
    "playwright": {
      "args": ["-y", "@playwright/mcp@latest", "--isolated", "--headless",
        "--storage-state", "${PLAYWRIGHT_MCP_STORAGE_STATE}",
        "--output-dir", "${PLAYWRIGHT_MCP_OUTPUT_DIR}"]
    }
  }
}
```

単一の機械上で 2 つの Postgres データベースを扱う設定の構文は以下のとおりである。

```json
{
  "servers": {
    "postgres-app": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_APP_URI}" }
    },
    "postgres-analytics": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_ANALYTICS_URI}" }
    }
  }
}
```

対応する値は、コーディングエージェントが動作する環境側にエクスポートする。MCP 設定内の `${ENV_VAR}` プレースホルダーは host のロード時に解決され、gal が解決するのではない。インストール済みの実行環境の MCP 設定はユーザーの所有下にあり、gal がそれらを書き込むことは決してない。storage-state ファイル、永続プロファイル、ブラウザの成果物、秘密に類するローカルファイルをコミットしないこと。

### Working Hours

working-hours の強制は既定で無効である。勤務日の境界を定めるには `~/.gal/config/config.json` の `workingHours` キー配下を設定する。`enabled` を `false` に設定すると無効のままである。`workdayStart` と `workdayEnd` のキーが稼働時間帯を定義する。`wrapUpTime` キーはリマインダーとシャットダウン手順を開始する。`hardStopTime` キーはエージェントの作業拒否を絶対的に発動する。これらの設定は、追跡対象のリポジトリポリシーではなく、機械本機の設定を表する。

### Plan Language

- `config.json` の `planLanguage` 設定は機械本機かつオプションである。明示的な指示がない場合の `.dev/plans/*.md` と `.dev/research/*.md` の既定の出力言語を決めます。解決の順序は、明示的な指示、次に `planLanguage`、次にプロンプト言語の自動検出、最後に `en` フォールバックである。
- 追跡対象のプロジェクトメタデータ `PROJECT_LANGUAGE` が、主要ドキュメントの正規言語を決めます。
- `.dev/plans/*.prompt.md` に一致するファイルは、クロスモデルの安定性を確保するため、厳格に英語のままとする。

英語以外の `planLanguage` 設定は 3 層の計画を生成する。第 1 層は `.dev/plans/<slug>.en.md` にある英語の意味論的草案で、技術的意味の権威として機能する。第 2 層は `.dev/plans/<slug>.md` にある現地化されたソースプランで、読み書きを意図する。見出し、パス、タスク ID、裁定は英語のままとし、叙述的な文章は現地化された言語を採用する。第 3 層は英語の実行プロンプトである。**現地化された計画の手動編集は完全にサポートされます。** 以降の計画段階のコマンドは編集を検出し、読み取り専用の照合ステップで一時停止して、続行の前に手動の意図を英語草案へ統合する。編集が黙って上書きされることはない。`/plan-to-prompt` の実行後、草案は削除され、計画ごとにちょうど 2 つの追跡対象ファイルが残ります。英語の `planLanguage` はこの仕組み全体を回避する。

### プロバイダーメモリの収穫

オプトインの仕組みにより、現在の会話中にコーディングエージェントが表面化させた有用な学びを GAL の追跡対象ファイルへ移し、セッション終了時のデータ損失を防ぐ。

- **既定オフ、機械本機。** `config.json#memoryHarvest.enabled` を `true` に設定して機能を有効化する。値が欠落または false なら無効のままである。
- **限定されたスコープ。** GAL は候補を特定するために、コーディングエージェントのチャット履歴やセッションファイルを開いたり、一覧したり、検索したりしない。
- **リポジトリスコープかつ秘匿処理済み。** 候補はリポジトリファイルへの直接の関連を必要とする。項目は可視化の前に言い換えと秘匿処理を受けます。生の引用、secrets、機械パス、個人的なメモは除外されます。
- **必須の承認。** 候補の承認は `/gal wrap-up` の間にのみ行われます。拒否または未回答の候補は書き込まれない。承認された候補は、暫定的で助言的なタスク記憶として、アクティブな計画の引き継ぎメモに入ります。
- **承認の制約。** 承認は昇格を保証しない。`/gal finalize` コマンドは、すべての昇格された学びの基準に独立した検証が合致した後にのみ、承認された候補を `docs/` へ昇格させます。

## パーソナライズ（`~/.gal/local/`）

機械本機の個人コンテンツは `~/.gal/local/` 配下に置かれ、正規ルートのレンダリングを通じてすべてのエージェントへ投影されます。`gal` バイナリは `local/skills/`、`local/mcp.json`、`local/conventions/` を読み取りますが、**これらのユーザー作成のパスへの書き込みや削除は厳格に避けます**。マシン間の同期は `gal` の機能の範囲外である。

### 個人のスキル / MCP / 規約

**レイアウト：**

```text
~/.gal/local/
  skills/
    <skill-name>/
      SKILL.md          ← hand-placed personal skill (gal read-only)
  mcp.json              ← personal MCP servers (same format as plugins/gal-core/mcp.json, gal read-only)
  conventions/
    <lang>.md           ← personal coding-style convention file (gal read-only, see below)
```

**有効化：** 有効化は設定フラグではなく存在に依存する。個人ルートにファイルを置くことがオプトインの操作である。ディレクトリやファイルが存在しなければ、レンダリング出力は Core のみのレンダリングとバイト単位で同一のままである。

**投影のルール：**

- 個人のスキルはコアコンテンツの後にマージされます。コアのスキルと名前が衝突する個人のスキルは、core-wins ポリシーの下で黙ってスキップされます。
- 個人の MCP server は正規の `.mcp.json` ファイルにマージされます。コアの server と名前が衝突する個人の server は、core-wins ポリシーの下でスキップされます。
- `gal doctor` コマンドは、個人のスキル数、個人の規約ファイル数、コア衝突によるスキップ数を報告する。

### コーディングスタイル規約

GAL Core はオーナー個人のコーディングスタイル規約を含みない。ハウススタイルは、次の 3 つの異なるソースを通じて下流のリポジトリへ伝播する。

**ソース 1 —— 個人の規約ファイル（常時オン、リポジトリごと）。** 同梱の例 `plugins/gal-core/templates/csharp-convention.example.md` を `~/.gal/local/conventions/csharp.md` に複製し（この宛先パスに厳密に一致させます）、内容を対象のスタイルに合わせて変更する。`.dev/project.md` に一致する `Language` 行を持つ対象リポジトリ内で `gal init` を実行する。システムは gal-core の規約プロセスを模して、そのファイルをリポジトリのアダプターへ注入する。

**ソース 2 —— インストール済みのエージェントプラグインの検出（読み取り専用、セットアップ不要）。** コーディングエージェント内にインストール済みの既存の公式言語プラグインは、対象リポジトリで `gal init` を実行するだけで済みます。GAL はプラグインのスキル名と出所をリポジトリの `Language` 行と突き合わせ、名前、出所パス、永続的な読み込み指示を含む **Detected Language Skills** 参照ブロックをレンダリングする。GAL はスキル内容の複製を厳格に避け、プラグインのインストール・更新・削除を行わない。新しくインストールされたプラグインの検出には、続いて `gal init` の実行が必要である。

**ソース 3 —— 個人のスキル（オンデマンド）。** `~/.gal/local/skills/<name>/` 配下に、明確な説明を含む `SKILL.md` ファイルを作成する。検出を初期化するには Claude Code を再起動する。他のエージェントは次の読み取りサイクルでファイルを検出する。エージェントは、ソース 1 と 2 の常時オンの注入とは対照的に、名指しされたときにのみこのソースを読み込みます。

**ソースの選択戦略：** 既存の公式プラグインがインストール済みなら、セットアップ不要を実現するためにソース 2 を優先する。ソース 1 は、最大の制御を伴う常時オンの個人スタイリングを提供する。ソース 3 は、オンデマンドのエージェント相談の要件に応えます。

**ミス契約：** 一致するソースを欠くリポジトリの `Language` 行は、言語固有の規約や参照ブロックを持たないアダプターとなる。この動作はエラー条件ではなく、設計上の黙ったスキップである。

ソース 1 と 2 を無効化するには、リポジトリの `.dev/project.md` の Tech Stack 表に `| Personal Conventions | off |` の行を挿入する。追跡対象のアダプターにオーナー機械のコンテンツが埋め込まれるのを防ぐため、この運用は公開リポジトリで推奨される。ソース 3 は影響を受けない。

### コマンドスキルのローカルオーバーレイ（`SKILL.local.md`）

コマンドスキルに機械本機のカスタマイズを実装するには、`plugins/gal-core/commands/<command>/SKILL.local.md` ファイルを生成する。

- このファイルは gitignore され、ユーザー所有の機械本機入力として動作する。
- ベイク処理は `SKILL.local.md` を生成される `SKILL.md` に追記する。
- `plugins/gal-core/commands/<command>/SKILL.md` を直接編集しないこと。これは置き換えの対象となる生成ファイルである。
- `SKILL.local.md` は補足的な指示内容に限定すること。二次的な frontmatter ブロックは除外すること。

### local-notes のルーティング

外部ノートは機械本機かつオプションのコンポーネントである。GAL はリポジトリ所有の状態をユーザー所有のノートから隔離する。コアの動作はプライベートなノートストアから独立して動作する。ポータブルな束縛契約は [`optional-capabilities.md`](../../../plugins/gal-core/conventions/optional-capabilities.md) にある。これはアプリ非依存で、既定でオフであり、機械本機バックエンドが整備済みのときにのみ実行されます。

存在しない、到達不能、または未初期化のバックエンドは、失敗の警告を生成することなく、標準のリポジトリ本機ワークフローへの劣化を引き起こす。オプトイン済みかつ到達可能なバックエンドのみが、ワークフローで明示的に許可された場所での読み取り・検索・書き込みを許する。状態が必須になることは決してない。ノートバックエンドを欠くリポジトリは、オプションのコンテキストソースがないという点だけが異なり、完全に接続されたインスタンスと同一に動作する。

ドキュメント化されたバックエンドの例には、Obsidian（`coddingtonbear/obsidian-local-rest-api`）、Logseq（`ergut/mcp-logseq`）、Joplin（`joplin-mcp`）、汎用の markdown vault（`vault-mcp`）、CJK 優先の取得（`SeekLink`）がある。この一覧は機能の同等性を保証しない。

リポジトリ所有の研究は、既定で `.dev/research/` ディレクトリに置かれます。

## Headless Executors

GAL は、会話ループの実行を回避して、pipeline のフェーズを二次的なヘッドレスのコーディングエージェント CLI へオフロードすることをサポートする。この設定は `~/.gal/config/config.json` 内の `executorRouting` 配下に実装する。

### ヘッドレスエグゼキューターのルーティング

`config.json#executorRouting` キーは、役割を**消費者ごと**に 2 つの異なるオブジェクトへ分類する。`pipeline` オブジェクトは実装・テスト・監査フェーズのヘッドレスなディスパッチを扱いる。`planning` オブジェクトは、計画段階のレビュー役割向けの Codex ネイティブサブエージェントのモデル選択を管理し、ヘッドレスなディスパッチは厳格に避ける。共有の `executors` 既定モデルブロックがこれらのオブジェクトに付随する。

```json
{
  "executorRouting": {
    "executors": {
      "claude":   "claude-haiku-4-5-20251001",
      "codex":    "gpt-5.4-mini",
      "opencode": "opencode/minimax-m3-free",
      "copilot":  "claude-haiku-4-5-20251001",
      "agy":      "gemini-2.5-flash"
    },
    "pipeline": {
      "CODER":   { "executor": "codex" },
      "TESTER":  { "executor": "opencode" },
      "AUDITOR": { "executor": "claude", "model": "claude-sonnet-4-6" }
    },
    "planning": {
      "ARCHITECT": { "executor": "codex", "model": "gpt-5.4", "effort": "high" },
      "ANALYST":   { "executor": "codex", "model": "gpt-5.4-mini" }
    }
  }
}
```

両グループとも閉じた役割の許可リストを強制する。`pipeline` の一覧は `{CODER, TESTER, AUDITOR}` を含みます。`planning` の一覧は `{ARCHITECT, ANALYST, DESIGNER, RELEASER}` を含みます。誤ったグループに割り当てられた役割や認識されない役割はスキップされ、正しいグループを示す警告を生成する。役割キーを `executorRouting` 直下に置くレガシーのフラット構造は廃止され、フォールバックなしで可視の警告を生成する。

**executors ブロック：** ツールごとの既定モデルを定義する。モデル指定を欠く役割の項目は、`executors[executor]` から既定を継承する。役割に明示されたモデル宣言は既定を上書きする。

**役割ごとの effort キー：** オプションの推論強度の指標を提供し、起動 (spawn) の前に各エグゼキューターのネイティブな推論フラグへマッピングされます。

| エグゼキューター | `effort` → ネイティブフラグ |
| --- | --- |
| claude | `--effort <value>` |
| codex | `-c model_reasoning_effort="<value>"` |
| copilot | `--reasoning-effort <value>` |
| opencode | `--variant <value>` |
| agy | 非対応 |

この仕組みは fail-closed で動作する。effort ヒントを尊重できない、または不正な値に遭遇したエグゼキューターは、ヒントを黙って無視するのではなく、起動の前にディスパッチを劣化させます。`effort` キーは純粋にヒントとしてのみ機能し、モデルセレクターとして動作したり、有料または名前付きのモデルを含意したりすることは決してない。

**アダプター動作の注記：**

- **OpenCode** は、書き込みを黙ってブロックする読み取り専用の既定を回避して書き込み可能なエージェントを有効化するため、`--agent build` を用いてディスパッチする。また権限バイパスフラグとして `--auto` を用いる。
- **Copilot** は、ヘッドレスのプロンプトモードがコンテキスト上限を超過するのを防ぐため、`--no-custom-instructions` と `--disable-builtin-mcps` を付加する。Copilot Free は厳格に **auto 専用**で動作し、`model: auto` とローカル専用の実行を維持する。

**役割の表：**

| 役割 | グループ | 目的 |
| --- | --- | --- |
| `CODER` | pipeline | 計画に従って実装コードを書きます。`TESTER` と異なる必要がありする |
| `TESTER` | pipeline | 計画の仕様と公開 API のみからテストを書きます。`CODER` と異なる必要がありする |
| `AUDITOR` | pipeline | 深いパフォーマンスとセキュリティを監査する。`CODER` と異なり、ティアは `CODER` 以上である必要がありする |
| `ARCHITECT` | planning | トレードオフ、過剰設計、バグを対象とする対抗的な計画レビュー |
| `ANALYST` | planning | ROI、ドメインの正しさ、ユーザー影響を対象とするビジネスロジックレビュー |
| `DESIGNER` | planning | UX、UI、DevEx のレビュー |
| `RELEASER` | planning | リリースフローの設計（読み取り専用） |

研究操作は**インプロセスで実行され、ヘッドレスなルーティングに抵抗します**。`RESEARCHER` 役割は `executorRouting` の項目を持ちない。`/gal research` と `/gal deep-research` のコマンドは会話ループ内で実行されます。有効なエグゼキューターは `claude`、`codex`、`opencode`、`copilot`、`agy` である。役割を省略すると、その実行は会話ループ内に留まります。

> ⚠️ **セキュリティ警告 —— 権限バイパス。** ヘッドレスなエグゼキューターアダプターは、`--dangerously-skip-permissions`（Claude Code、Antigravity/agy）、`--auto`（OpenCode）、`--allow-all`（Copilot）、`-s workspace-write`（Codex）を用いて二次的な CLI を起動する。これはローカルのファイルシステムとターミナルに対する**完全な信頼**を付与し、サンドボックスの保護を無効化する。エグゼキューターのルーティングは、信頼できる機械と環境でのみ有効化すること。リポジトリまたはエージェント契約が信頼できないソースに由来する場合は、有効化を禁止する。仕様は二次的な CLI が `git commit` や `git push` を実行することを禁じていますが、これは技術的な強制ではなく指示である。

### リモート実行（SSH ディスパッチレーン）

クロスマシン実行は、別個のコマンドではなく、解決されたルートの性質として機能する。`/gal pipeline` コマンドは、`CODER`、`TESTER`、`AUDITOR` がローカルとリモートのどちらの呼び出しに解決されても、同一の動作を維持する。そのフェーズを SSH 経由で実行するには、pipeline の役割の項目に `sshTarget` と `remoteWorkdir` を付加する。

```json
"pipeline": {
  "TESTER": {
    "executor": "claude",
    "model": "claude-sonnet-4-6",
    "sshTarget": "user@build-box",
    "remoteWorkdir": "/home/user/gal-remote"
  }
}
```

両フィールドは同時の存在を必須とする。部分的に設定された項目は警告を伴って読み込まれ、ディスパッチ時に明示的な失敗を引き起こす。

**前提条件** —— GAL はプロビジョニングを一切行わないため、これらはリモート機械上で直接設定すること。

- 対象へのパスワードなしの SSH アクセス（`BatchMode=yes`）。対話的なパスワードやパスフレーズの促しは、到達不能状態とみなされ、再試行を妨げます。
- ルートされたエージェント CLI（`claude`、`codex`、`agy`、`opencode`）は、リモート機械上でのインストールと認証を要求する。`gal` バイナリはリモート対象には不要である。
- `remoteWorkdir` 内の `git` リポジトリは、制御ノードの checkout コミットに一致し、かつすべてのディスパッチの前にクリーンな作業ツリーを備えている必要がある。

**期待される結果：** ディスパッチは、維持された SSH セッション上で同期的に実行され、ローカルのディスパッチと同じ寿命に一致する。変更を伴う実装フェーズが成功した後、GAL は未コミットのリモート差分を取得して制御ノードへ適用する。GAL は、ローカルへの適用が成功した後にのみ、リモートの checkout を（`git reset --hard && git clean -fd` で）クリーンな状態へリセットし、逆の実行順序を禁じます。ローカルの適用が失敗した場合は、検査のためにリモートの checkout を変更せずに残する。

`remoteWorkdir` は **GAL 専用の checkout** としてのみ機能させる必要がある。適用後のクリーンアップ処理は設計上破壊的に動作するため、対話的に用いる checkout を対象にしないこと。

**既知の制限（v1）：** copilot エージェントは、その CLI フラグ仕様の受け渡しが SSH コマンドラインをまたぐ転送に抵抗するため、リモートルーティングに抵抗する。そのようなルートは、黙った劣化ではなく明示的な失敗を引き起こす。Windows のリモート対象は、構成されたコマンドが POSIX ログインシェルを要求するため、サポートされていない。切断耐性の機能はないため、SSH セッションが切れると、再接続や再開の選択肢なしにディスパッチが失敗する。リモートレーンは、単発ディスパッチかつ手動再同期のモードで動作する。制御ノードの HEAD はタスク後のコミットで前進する一方、リモートの checkout は最終同期点で停止する。アクティブな実行中の以降のリモートディスパッチは、手動の checkout 同期が行われるまでガードに引っかかります。

ライブの pipeline タスクを回避して整備度を確認するには、下記の [Remote (SSH) Self-Test](#remote-ssh-self-test---transport-ssh) を用いる。付随する状態表は、リモート固有の失敗条件と復旧手順をドキュメント化している。

### ディスパッチの検査

各ディスパッチは **2 つ**の永続的なトレースを生成する。タスクに適したトレースを選択すること。

**ティア 1 —— GAL エグゼキューターログ（監査証跡、すべてのツールで統一）。** 各実行は `.dev/executor-logs/<plan-slug>/`、または直接ディスパッチの場合はスコープなしの `.dev/executor-logs/` に着地する。ファイルは `<timestamp>-<attempt>-<task>-<phase>-<executor>.log` の命名規則に従う。ヘッダーは、終了状態、終了コード、実際のモデル、git ブランチと HEAD、およびプロバイダーの `session_id` をドキュメント化する。`---STDOUT---` ブロックは、エージェントのメッセージ、コマンドの実行、ファイルの変更、トークン消費のメトリクスを含む、プロバイダーの完全なイベントストリームを捕捉する。エグゼキューターの動作を監査するにはこのログを読みます。この形式は 5 つのツールすべてで同一である。プロバイダー本機のチャット履歴は助言目的で機能し、このログがリポジトリ所有の公式な記録を構成する。

**ティア 2 —— プロバイダーネイティブのセッション再開（継続・分岐用）。** ログヘッダーは再開可能な `session_id` を記録する。この識別子を用いて、ネイティブのプロバイダー UI 内で会話を延長する。コマンドは統一されて**いません**。

| エグゼキューター | ネイティブビュー / 再開コマンド | ヘッドレスセッションの既定の可視性 |
| --- | --- | --- |
| **claude** | `claude --resume <session_id>` | 一覧に表示 |
| **codex** | `codex resume <uuid>`（UUID はフィルターを回避） | **非表示** —— ピッカーで見るには `codex resume --include-non-interactive` を用いる（cwd フィルターを無効化するには `--all` を追加） |
| **opencode** | `opencode run -s <session_id>`（継続）· `opencode export <session_id>`（JSON をダンプ）· `opencode session list`（閲覧） | 一覧に表示 |
| **copilot** | `copilot --resume=<session_id>` | `~/.copilot/session-store.db` に保存、id で再開（公開の一覧コマンドなし） |
| **agy** | `agy --conversation <uuid>` | `~/.gemini/antigravity-cli/brain/<uuid>/` 配下に保存、list サブコマンドなし —— id で閲覧 |

**一般的な指針：** ティア 1 のエグゼキューターログを読んでディスパッチを監査する。ネイティブツール内での再開操作には、対応するティア 2 のコマンドを用いる。codex ツールは、GAL の設定ではなく固有の動作により、既定でヘッドレスセッションを唯一非表示にする。

### Executor Self-Test (`gal doctor --executor-smoke`)

標準の `gal doctor` コマンドは、ヘッドレスなコーディングエージェント CLI を呼び出しない。`gal doctor --executor-smoke` の変種は、オプトインで反復可能なセルフテストであり、ライブの pipeline タスクが用いるのと同一のヘッドレスなディスパッチ経路を、サポートされる 5 つの CLI（`codex`、`claude`、`copilot`、`agy`、`opencode`）にわたって実行する。

```sh
# Self-test all five agents (default --transport local)
gal doctor --executor-smoke

# Filter to specific agents (repeatable)
gal doctor --executor-smoke --executor codex --executor claude

# CI/scheduler use: machine-readable JSON, non-zero exit on any non-pass row
gal doctor --executor-smoke --json --strict

# Bound the per-agent timeout (seconds, default 300)
gal doctor --executor-smoke --timeout 60
```

このコマンドは、ライブの `config.json#executorRouting` 設定から隔離された合成のルーティングファイルを用いて、真正な実ディスパッチのコード経路を起動する。1 行のレシートを書くことだけを目的とする最小のタスクを実行する。実行は、gitignore された、タイムスタンプ付きのディレクトリ（既定で `.dev/executor-smoke/runs/<utc-run-id>/local/`）内に永続化されます。出力には、JSON レポート、人間が読める表、エージェントごとのログ、レシートが含まれる。`.dev/executor-smoke/latest.json` ファイルは、最新の実行を永続的に指する。

**状態の定義：**

| 状態 | 意味 |
| --- | --- |
| `PASS` | 実ディスパッチが完了し、レシートが検証されした。 |
| `NOT_INSTALLED` | CLI が PATH にない。インストールすること。 |
| `NOT_AUTHENTICATED` | 未認証が確認されした。そのツールのログインコマンドを実行すること。 |
| `AUTH_UNKNOWN` | 整備度を確認できませんでした。上限付きの呼び出しを試み、実際の結果を報告する。 |
| `UNSUPPORTED` | サポートされる 5 エージェント以外の `--executor` 名である。決してディスパッチされない。 |
| `CONFIG_ERROR` | ディスパッチ前の問題（多くは安全でない `--report-dir`）である。書き込み前に捕捉されます。 |
| `CALL_FAILED` | エグゼキューターが非ゼロで終了しした。実行のエグゼキューターログを検査すること。 |
| `NO_RECEIPT` | エグゼキューターは 0 で終了しましたが、レシートを書きませんでした。終了 0 だけでは決して成功になりない。 |
| `TIMEOUT` | 上限付きのタイムアウトが経過しした。ツールが遅い場合は `--timeout` を広げるか、ハングした対話的な促しを調査すること。 |

`--strict` フラグを省略すると、実行完了時に終了コード 0 を強制する。レポートが終了コードではなく絶対的な真実の情報源として機能する。このツールはバックグラウンドサービスではなく、反復可能な CLI コマンドとして動作する。手動で実行するか、CI パイプラインに組み込むか、OS のスケジューラーで起動すること。

#### Remote (SSH) Self-Test (`--transport ssh`)

このセルフテストは、SSH 経由でリモート機械上のコーディングエージェントの整備度を検証する。同一のレポートスキーマと状態を維持しつつ、トランスポート機構を変更する。インストールと認証のプローブは**リモート機械上で**実行され、制御ノードからの推測に抵抗する。

```sh
gal doctor --executor-smoke --transport ssh --ssh-target <ssh-target> --remote-workdir <dedicated-checkout>

# CI/scheduler use
gal doctor --executor-smoke --transport ssh --ssh-target <ssh-target> --remote-workdir <dedicated-checkout> --strict --json
```

`--ssh-target` 変数は、非対話的なキーベースのアクセス（`BatchMode=yes`）をサポートする SSH ホストを定義する。`--remote-workdir` 変数は**必須**で、制御ノードの git HEAD に一致し、かつクリーンな状態の GAL 専用 checkout を指定する。生成されたレポートは `.dev/executor-smoke/runs/<utc-run-id>/ssh/` に出力されます。

**リモート固有の状態：**

| 状態 | 意味 | 是正 |
| --- | --- | --- |
| `SSH_UNREACHABLE` | 非対話的な SSH セッションを開けませんでした（エグゼキューターのプローブより前に確認）。 | 対象、ネットワーク、キーベース認証を確認すること。 |
| `REMOTE_GUARD_FAILED` | リモートの workdir が欠落、安全でない、dirty、または制御ノードの HEAD にない。 | 専用のリモート checkout を制御ノードの HEAD へ再同期し、クリーンな状態を確保すること。 |
| `REMOTE_FETCH_FAILED` | リモートプロセスは実行された可能性がありますが、そのレシートの取得に失敗しした。 | 実行のエグゼキューターログとリモートのレシートパスを検査すること。 |

リモートの copilot レポートは、リモートの制限に従って `UNSUPPORTED` 状態を生成する。この状態はパス数にも省略にも数えられない。`PASS` 状態は、終了完了**および**空でないレシートの取得成功を厳格に要求する。

## Git ヘルパー

### `gal commit-msg`

`gal commit-msg` コマンドは、`git-commits` スキルと `git-commit-msg` コマンドを支える決定論的なコミットヘルパーとして動作する。type と scope の分類は、**専ら**変更されたファイルパスと git status から導かれます。差分や本文のキーワードを無視し、メッセージ内容が分類を乗っ取るのを防ぐ。3 つのモードがある。

- **`gal commit-msg --context`** は、ファイル、決定論的な type と scope のベースラインヘッダー、ステージ済みの計画とプロンプトの要約、およびメッセージ起草エージェント向けの hunk ヘッダーを含む、コンパクトなステージ済み変更コンテキストを出力する。このモードは、生の差分より高いシグナル品質を、より低いトークンコストで提供する。
- **`gal commit-msg --print`** は、決定論的な type と scope の件名ヘッダーのみを出力する。
- **`gal commit-msg <file>`** は git commit-msg フックとして動作する。ステージ済み変更に基づいて空のメッセージを埋め、作成済みの内容の上書きを厳格に避ける。

`git-commit-msg` コマンドはメッセージの文言のみを生成し、git commit の実行を避ける。`git-commits` スキルはメッセージを生成し、**かつ**明示的なコミット意図を検出したときにコミットを実行する。

### Git フィルター（`gal clean` / `gal smudge`）

オプションの `gal-config` git フィルターは、追跡対象ファイルから機械本機の設定値を除去する。リポジトリごとに登録して適用する。

```bash
git config filter.gal-config.clean 'gal clean'
git config filter.gal-config.smudge 'gal smudge'
```

git 実行ファイルがこのフィルターを内部的に呼び出す。フィルターのエラーによる `git commit` の失敗を防ぐため、`gal` バイナリはアクティブな git の `PATH` 上に存在する必要がある。

## オーサリング支援ツール

### `text-flowcharts`

[`text-flowcharts`](../../../plugins/gal-core/skills/text-flowcharts/SKILL.md) スキルは、分岐ロジック、パイプライン、多段階プロセスを等幅プレーンテキストの決定木図として描画する。Claude Code では `/text-flowcharts`、Codex では `$text-flowcharts` で呼び出す。説明がレコード単位の制御フローを追う場合や、flowchart、流程圖、邏輯圖 を求めた場合にも自動的に有効化される。

各図は上端の入口から始まり、1 件のレコードが遭遇する条件を上から下へたどり、最後に等級付けされた終端結果に到達する。そのため読者はレコードを 1 件投入して、それがどこに着地するかを追える。語彙は意図的に小さく保たれている。ボックスはステップを表し、波括弧は判断を表し、すべての葉ノードは終端マーカーを持つ。

| 要素 | グリフ |
| --- | --- |
| フロー線と角 | `│ ─ ┌ ┐ └ ┘` |
| 接合（分岐、合流、交差） | `├ ┤ ┬ ┴ ┼` |
| 矢印（下、上、右、左） | `▼ ▲ ▶ ◀` |
| 終端の成功 | `√` |
| 意図的なスキップ | `>>\|` |
| 行き止まりまたは拒否 | `×` |

これらのグリフはフォントを追加せずに既定の等幅フォントで表示でき、非 CJK の読者にとっては 1 セル幅に収まり、UTF-8 のもとでプルリクエストのコメント、コードコメント、ターミナルでもそのまま維持される。出力に emoji を含まないため、古いマシンでも読みやすさを保つ。プロセスが実際に分岐する場合にのみこのスキルを使う。判断のない直線的な連なりは、番号付きリストの方が読みやすい。

## Golem Agents

GAL の専門エージェントは golem として動作する。この節は、機能、差別化、適用に関する**ユーザー向け**の視点を提供する。権威ある名簿と分類は [`plugins/gal-core/agents/agents.md`](../../../plugins/gal-core/agents/agents.md) に、ワークフローの意味論は [`plugins/gal-core/workflows/coding.md`](../../../plugins/gal-core/workflows/coding.md) にある。

### 能力表

| Golem | 何をするか | いつ使うか |
| --- | --- | --- |
| `golem-architect` | トレードオフ、過剰設計、バグ面、依存/API リスクを対象とする対抗的な計画レビュー | 構築の前に設計を圧力テストするには `/deep-planning` または `/gal architect` を用いする |
| `golem-analyst` | ROI、ドメインの正しさ、ユーザー影響を対象とするビジネスロジックレビュー | 変更が価格、権限、適格性、顧客可視のルールに影響するときに用いする |
| `golem-designer` | UI/UX 体験設計、DevEx、デザインシステム、アクセシビリティ、ライブ UI 監査 | 顧客向けのレイアウト、状態、コンポーネント作業、または開発者向けの DevEx に用いする |
| `golem-researcher` | ローカルファーストの調査、ソース横断の統合、参照可能な結果 | 証拠に基づく回答には `/gal research` または `/gal deep-research` を用いする |
| `golem-implementer` | 承認済みタスクの実装コードをアトミックなコミットで書きする | `/gal pipeline` 内の CODER フェーズを表しする |
| `golem-tester` | 計画の仕様のみから導かれる、仕様駆動のテストと実ブラウザ QA を生成しする | 異なるモデルを用いた独立検証を駆動する TESTER フェーズを表しする |
| `golem-auditor` | 単一タスクと全ブランチの深いパフォーマンスとセキュリティの監査を実行しする | `/gal pipeline`（タスク監査）または `/gal finalize`（全ブランチ監査）の AUDITOR フェーズを表する。この役割はオーケストレーター経由限定であり、素の `/gal auditor` 呼び出しを避けする |
| `golem-debugger` | フリーズ規律と根本原因の確認を用いる、科学的手法のバグ調査を行いする | 修正の前に規律ある調査を要するバグに用いする |
| `golem-steward` | ドキュメント構造、コードと文書の乖離、知識抽出、図の同期を管理しする | `/gal steward` を用いるか、計画開始時、refining 終了時、pipeline クローズアウト時に自動的に起動しする |
| `golem-releaser` | API と CICD の調査を通じて計画段階のリリースフローを設計し、設計助言を発しする | `/planning release-<slug>` の前に `/gal releaser`（隔離）または `/gal discuss releaser`（コンテキスト内）を用いする |

**チェック役割のトライアングル：** 品質保証の責務は三者構造を成する。**ORCHESTRATOR**（pipeline）は、タスクごとの正確性ゲーティング、実行終了時の後ろ向きの目標検証、計画ライフサイクルのクローズを指揮する。**AUDITOR** は単一タスクの深いパフォーマンスとセキュリティを管理する。**STEWARD** はドキュメント構造を統制する。

### 役割の呼び出し可能性マトリクス

| 役割 | 直接呼び出し可能か | モード |
| --- | --- | --- |
| **architect** | はい | `/gal architect`（隔離）または `/gal discuss architect`（コンテキスト内） |
| **analyst** | はい | `/gal analyst`（隔離）または `/gal discuss analyst`（コンテキスト内） |
| **designer** | はい | `/gal designer`（隔離）または `/gal discuss designer`（コンテキスト内） |
| **releaser** | はい | `/gal releaser`（隔離）または `/gal discuss releaser`（コンテキスト内）、計画段階の設計者であり、実行はしない |
| **debugger** | はい | `/gal debugger` |
| **steward** | はい | `/gal steward` |
| **implementer** | **オーケストレーター経由限定** | `/gal pipeline`（pipeline フェーズのコンテキスト）を通じてのみ |
| **tester** | **オーケストレーター経由限定** | `/gal pipeline`（pipeline フェーズのコンテキスト）を通じてのみ |
| **auditor** | **オーケストレーター経由限定** | `/gal pipeline`（pipeline フェーズ）または `/gal finalize`（ブランチ監査）を通じて |
| **researcher** | **オーケストレーター経由限定** | `/gal research` または `/gal deep-research` を通じてのみ |

一致するオーケストレーションコンテキストを欠いて `/gal <role>` を実行すると、4 つのオーケストレーター経由限定の役割すべてに対して `COMMAND: error` を返する。

### 相談のデュアルモード（`/gal discuss <role>`）

architect、analyst、designer、releaser の各役割は、2 つの呼び出しモードのみをサポートする。残りの役割は discuss 形式をサポートしない。

| モード | トリガー | 何が起きるか | 応答ラベル |
| --- | --- | --- | --- |
| **隔離**（既定） | `/gal <role>` | ネイティブのサブエージェントが役割を隔離して実行する。裁定と要約のみがメインコンテキストへ返りする | `[<role> · isolated]` |
| **コンテキスト内** | `/gal discuss <role>` | 役割の起動コアが現在の会話へ読み込まれます。アシスタントは先行する隔離の裁定からホットジョインし、トピックが変わるまでマルチターンで続けする | `[<role> · in-context]` |

**ホットジョイン：** トランスクリプトには隔離の裁定が含まれるため、コンテキスト内モードは役割の完全な再実行を回避して、ネイティブに進捗を再開できます。

Codex 実行環境は `/gal discuss <role>` を `$discuss-<role>` にマッピングする。Claude 実行環境は `/gal discuss <role>` をスラッシュコマンドとして直接実装する。

統合された [`adversarial-review`](../../../plugins/gal-core/skills/adversarial-review/SKILL.md) スキルは、これらのレビュー役割が用いる対象非依存のレビュー方法論を提供する。これはスティールマン論法、既定で反駁するロジック、証拠の規律、および明示的な APPROVE・REVISE・REJECT の裁定を強制する。

### Steward のライフサイクル分割

steward は計画フェーズと finalize フェーズの間に実行され、異なる操作を行う。これらのフェーズは厳格に交換不可能である。

- **計画フェーズ（`/deep-planning`）：計画文書の構造に焦点。** steward は、パス、slug、必須の節、言語の一貫性、図の同期を含む計画ファイルの属性を評価する。このフェーズ中の `docs/` への書き込みを禁止する。計画段階の文書は、実装と永続的な知識を欠きます。`docs/` への書き込みは、未構築で投機的な内容で読者向けのレイヤーを汚染する危険がある。
- **finalize フェーズ（`/gal finalize`）：永続的な知識の着地。** 完了した計画の永続的な知識は、厳格なルールとして `docs/` 内への着地を必須とする。steward は、実装された計画の知識を永続レイヤー（`README.md` と `docs/`）へ抽出する。続いて、そのインデックスが `.dev/project.md` へ同期されます。この着地プロセスは助言的ではなく強制されます。計画ファイルの削除は、steward が先に永続レイヤーのコミットを生成することを要求する。

この構造的分割は、知識抽出が実装後にのみアクセス可能な構築済みの知識を必要とすることを認めるものである。したがって、完成した計画を適切なドキュメントへ書き込むことは finalize 段階の操作であり続けます。計画時の steward は、計画文書のフォーマット維持に厳格に焦点を当てます。

### リリース計画レーン

リリースは独立した計画タイプ（`release-<slug>`）を構成し、`/gal pipeline` のフェーズや `/gal finalize` の手順から独立して動作する。

**フロー：**

1. `/gal releaser` コマンドは、デプロイ対象を検証してから、スキル、API、CLI 設定ファイルなどの読み取り専用のソースから、ローカルで利用可能なデプロイ能力を解決する。続いてリリースと devops のフローを設計する。この操作は厳格に読み取り専用で、ファイルの書き込み、コミット、実行を避ける。特定できない能力は not-available と報告され、捏造を防ぐ。
2. `/planning release-<slug>` コマンドは、生成された助言を `## Tasks` 節を持つソースプランへ実体化する。
3. 標準の `/gal pipeline` コマンドが、確定したリリース計画を実行する。
4. `/gal finalize` コマンドが、標準の計画と同一にリリース計画を着地させます。

このプロセスは、GAL 自身の CLI バイナリのリリースに加え、下流リポジトリのデプロイ（例：Web サービス、バックエンドサービス、firebase 系、npm パッケージ、Docker イメージ）をサポートする。運用の範囲は設計助言で止まります。releaser はデプロイのオーケストレーターとして動作することを禁じられ、カナリア、ロールバック、本番監視の機能を含みない。
