---
source: docs/manual.md
lang: ja
source_commit: 23f0d6298c1974d148a4f215c99a85bbeb5ec9d0
translated_at: 2026-09-08
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

適切なプラットフォームのインストールオプションを選択すること。インストール後、リポジトリ内で `gal init` を実行してリポジトリ本機のアダプター（`AGENTS.md` と `CLAUDE.md`）を生成する。[リポジトリでの初回実行](#リポジトリでの初回実行) を参照すること。

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

### 実行環境ごとのプラグイン登録（Plugin Registration）

プラグイン登録は、Markdown プラグインコンテナをサポートする実行環境における主要な読み込み経路です。登録されたすべての実行環境は、`gal refresh` によって `~/.gal/plugins/gal/` にレンダリングされる単一の標準ルートから直接読み込みます。Markdown プラグインコンテナを持たない実行環境（現在は OpenCode のみ）は、代わりにファイル投影のフォールバックを使用します。

プラグインが提供するアセットと投影されたファイルの間でスキルやエージェントの一覧が重複するのを防ぐため、各実行環境の設定は `~/.gal/config/config.json` で `pluginMode.<runtime>` を有効化し、`gal refresh` を実行することで完了します。

#### Claude Code

Claude Code は標準ルートをライブな skills-directory プラグインとして登録します。

1. 標準ルート `~/.gal/plugins/gal` を指すジャンクションまたはシンボリックリンク `~/.claude/skills/gal` を作成します。
   - Windows（PowerShell）：
     ```powershell
     New-Item -ItemType Junction -Path "$env:USERPROFILE\.claude\skills\gal" -Target "$env:USERPROFILE\.gal\plugins\gal"
     ```
   - macOS / Linux：
     ```bash
     ln -s ~/.gal/plugins/gal ~/.claude/skills/gal
     ```
2. `claude plugin list` を実行し、`gal@skills-dir` が一覧に表示され読み込まれていることを確認します。この skills-directory モードでは、Claude Code はキャッシュを使用せず、ジャンクションから直接インプレースで読み込みます。
3. `~/.gal/config/config.json` で Claude プラグインモードを有効化します。
   ```json
   {
     "pluginMode": {
       "claude": true
     }
   }
   ```
4. `gal refresh` を実行します。`pluginMode.claude` が true の場合、GAL はコマンドファイルを `~/.claude/commands/*.md` に書き込む処理をスキップし、以前のコピーを削除して、プラグインスキルとの重複を防ぎます。

マーケットプレイス経路はスナップショット利用者向けの副次的な経路として残り、`claude plugin add` 経由のインストールではファイルが `~/.claude/plugins/cache/<marketplace>/gal/<version>/` にコピーされます。

#### Codex

Codex はローカルのプラグインディレクトリから GAL を登録します。

1. ローカルプラグインのマーケットプレイスディレクトリを登録します。
   ```bash
   codex plugin marketplace add ~/.gal/plugins
   ```
   これは `gal refresh` が生成した `~/.gal/plugins/.agents/plugins/marketplace.json` を読み取ります。
2. プラグインを追加します。
   ```bash
   codex plugin add gal@gal
   ```
   Codex はプラグインを `~/.codex/plugins/cache/gal/gal/<version>/` にコピーします。
3. `codex plugin list` を実行し、一覧に `gal@gal` が表示されることを確認します。
4. `~/.gal/config/config.json` で Codex プラグインモードを有効化します。
   ```json
   {
     "pluginMode": {
       "codex": true
     }
   }
   ```
5. `gal refresh` を実行します。

**共有 `~/.agents/skills` に関する注意点：** Codex と OpenCode は `~/.agents/skills` を共有します。マシン上で OpenCode が選択されていない場合、`pluginMode.codex: true` を設定するとコアスキルの `~/.agents/skills` への投影がスキップされ、以前のコピーが削除されます。同じマシン上で Codex と OpenCode の両方が選択されている場合、GAL は OpenCode の動作を維持するためにコアスキルを `~/.agents/skills` に投影し続けるため、Codex にはプラグインスキルと投影スキルの両方が表示されます。

#### GitHub Copilot

GitHub Copilot は GAL を Agent Plugins 1.0.0 パッケージとして利用します。

1. Copilot の設定または構成で `~/.gal/plugins` をディレクトリマーケットプレイスとして登録し（`~/.gal/plugins/.claude-plugin/marketplace.json` を読み取ります）、`gal` を有効にします。Copilot はコピーを行わずにプラグインをインプレースでライブに読み込み、`com.github.copilot/agents/` からエージェントを、`com.github.copilot/rules/` からルールを、`mcp.json` から MCP 構成を検出します。
2. `copilot plugin list` を実行し、`gal` が有効になっていることを確認します。
3. `~/.gal/config/config.json` で Copilot プラグインモードを有効化します。
   ```json
   {
     "pluginMode": {
       "copilot": true
     }
   }
   ```
4. `gal refresh` を実行します。`pluginMode.copilot` が true の場合、GAL はエージェントファイルの `~/.copilot/agents/*.agent.md` への投影をスキップし、以前のコピーを削除します。なお、Copilot はプラグインの `commands/` を読み込まないため、Copilot コマンドスキルは `~/.copilot/skills/` 配下に引き続き投影されます。

#### Antigravity

Antigravity はローカルディレクトリ検出を通じて GAL をインストールします。

1. 標準ルートからプラグインをインストールします。
   ```bash
   agy plugin install ~/.gal/plugins/gal
   ```
   これにより、`~/.gemini/antigravity-cli/plugins/gal` に標準ルートを指すジャンクションが作成されます。
2. 重複した `claude-code` インポートの削除：Antigravity が以前に Claude Code からプラグインをインポートしていた場合、`agy plugin list` は 2 つの `gal` エントリ（`local-install` と `claude-code`）を報告します。`agy` には `import_manifest.json` から特定のインポートを削除するコマンドがないため、テキストエディタで `~/.gemini/config/import_manifest.json` を開き、`imports` 配列から `claude-code` オブジェクトを削除して、`local-install` エントリのみを残します。その後、`agy plugin list` を実行して `gal` エントリが 1 つだけになっていることを確認します。
3. `~/.gal/config/config.json` で Antigravity プラグインモードを有効化します。
   ```json
   {
     "pluginMode": {
       "agy": true
     }
   }
   ```
4. `gal refresh` を実行します。`pluginMode.agy` が true の場合、GAL はコマンドスキルの `~/.gemini/antigravity-cli/skills/` への投影をスキップし、以前のコピーを削除します。

#### OpenCode（投影フォールバック）

OpenCode は Markdown プラグインコンテナを使用しません。その拡張モデルは npm またはローカルパッケージを介してインストールされる JavaScript または TypeScript コードモジュールに依存しているためです。したがって、OpenCode はプラグイン登録の代わりに GAL のファイル投影フォールバックを使用します。`gal refresh` を実行すると、ネイティブ Markdown コマンドが `~/.config/opencode/commands/` に、エージェントが `~/.config/opencode/agents/` に、コアスキルが `~/.agents/skills/` に投影されます。

#### チャット主導のバイナリインストール（副次的マーケットプレイス経路）

Claude Code または Codex のプラグインマーケットプレイスのスナップショットブランチを通じて GAL を見つけたユーザーは、チャット主導のセットアップを利用できます。

1. Claude Code または Codex のプラグインマーケットプレイスで **GAL プラグイン**を見つけてインストールするか（「gal」で検索）、[マーケットプレイススナップショットブランチ](https://github.com/monkey1wizard/golem-agents-legion/tree/marketplace-snapshot)から追加します。
2. エージェントに **「help me install gal」** と依頼してインストールを要求します。プラグインは `install-gal` スキルを提供し、同意を求めた上で適切なパッケージマネージャー（Homebrew、winget、または `cargo install --git`）を実行してバイナリを検証し、未初期化のリポジトリでは `gal init` を、すでに初期化済みのリポジトリでは `gal render-adapters` を実行します。

スタンドアロンのマーケットプレイスプラグインだけでは完全な GAL 環境を提供できません。すべてのワークフローコマンドには `gal` バイナリが必要であるため、バイナリのインストールが必須です。`gal` をインストールした後は、日常的な使用のために上記の主要なインプレース登録手順に従ってください。

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

`gal init` コマンドは、未初期化のリポジトリでの初回実行時に `gal-core` テンプレートから 2 つのリポジトリ本機アダプタールート（`AGENTS.md` と `CLAUDE.md`）を生成し、`.dev/` ディレクトリ（`.dev/project.md` と `.dev/state.md`）の骨格を作成する。リポジトリがすでに初期化済み、または半初期化状態である場合、`gal init` は実行を拒否し、何も書き込まずに終了コード 1 で終了する。

| 状態 | `.dev/project.md` | `.dev/state.md` | `gal init` | 終了コード |
| --- | --- | --- | --- | --- |
| 未初期化 | なし | なし | 新規ブートストラップ。テンプレートから両ファイルを書き込み、アダプターをレンダリングする | 0 |
| 初期化済み | あり | あり | エラー。`.dev/project.md` を特定し、`gal render-adapters` および削除後の init を案内する | 1 |
| 半初期化 | あり | なし | エラー。存在するファイルと欠落しているファイルを特定し、バージョン管理または削除後の init を案内する | 1 |
| 半初期化 | なし | あり | 同上（対称） | 1 |

```
gal init: this repository is already initialized (.dev/project.md exists).
  To regenerate AGENTS.md and CLAUDE.md from .dev/project.md, run: gal render-adapters
  To start over from the templates, delete .dev/project.md and .dev/state.md, then run gal init again.
```

```
gal init: this repository is half-initialized: .dev/project.md exists but .dev/state.md is missing.
  Restore .dev/state.md from version control, or delete .dev/project.md and run gal init again.
  gal init does not overwrite .dev/project.md.
```

### `gal render-adapters`

`gal render-adapters` コマンドは、初期化済みのリポジトリにおいて `.dev/project.md` からリポジトリ本機アダプターを再生成する。

| 状態 | `.dev/project.md` | `gal render-adapters` | 終了コード |
| --- | --- | --- | --- |
| 初期化済み | あり | `AGENTS.md`、`CLAUDE.md`、条件付きレイヤーを再レンダリングし、廃止されたルートを刈り込む。パスごとに 1 行を出力 | 0 |
| 未初期化 | なし | エラー。`gal render-adapters: .dev/project.md not found. Run gal init first.` | 1 |

`.dev/state.md` は読み取られず、言及もされない。

### アダプターの保守と `gal refresh` による移行

初期化済みリポジトリを保守する場合や、5 つのアダプタールートを生成していた以前の GAL バージョンからアップグレードする場合、`gal render-adapters` を実行するとリポジトリ本機のアダプターが調整され、刈り込み移行（prune-migration）が適用される。

- **廃止されたルートの刈り込み：** 3 つの旧ブリッジルート（`GEMINI.md`、`.github/copilot-instructions.md`、`.agents/rules/gal.md`）は廃止された。アダプターの生成またはリフレッシュ時、GAL はこれらの廃止パスを検査する。廃止ファイルが 1 行目に完全一致する GAL 生成マーカーを持つ場合、GAL はそのファイルを削除し、`pruned (GAL-owned)` として報告する。ファイルを削除した結果 `.agents/rules/` または `.github/` が空になった場合、空の親ディレクトリも削除される。
- **手動所有ファイルの保持：** 廃止ルートのパスにあるファイルで GAL 生成マーカーを持たないものは、ユーザー作成として扱われる。GAL はそのファイルをそのまま保持し、`kept (hand-owned)` として報告する。
- **条件付きレイヤー：** `.dev/project.md` で Rust 規約が有効になっている場合、条件付きレイヤー（`.claude/rules/gal-rust.md` と `.github/instructions/gal-rust.instructions.md`）が 2 つのアダプタールートとともに更新される。無効になっている場合、陳腐化した GAL 所有の条件付きレイヤーは削除される。

### プロジェクト内の `.dev/project.md`

`.dev/project.md` ファイルは、アダプターのレンダリングに用いられる圧縮されたプロジェクト要約として機能する。8 つの H2 見出しは、**それぞれ厳密に一度ずつ必須**である。`What This Is`、`Tech Stack`、`Architecture`、`Constraints`、`Response Style`、`Freshness`、`Project Language`、`Protected Paths` である。

この要件は fail-closed（既定で遮断）として動作する。`gal init` はこのファイルをテンプレートから書き出すため、このチェックが対象とするのは手動で編集された `.dev/project.md` である。見出しの欠落や重複があると、`gal render-adapters` はレンダリング全体を拒否し、問題の見出しを特定して、アダプターファイルの書き込みを停止する。エラーは 1 回の実行につき欠落見出しを 1 つ報告する。複数の見出しが欠けているファイルは、反復的な修正パスが必要である。特定された見出しを（`plugins/gal-core/templates/project.md` の形式を参照して）手動で追加し、再実行して、以降の見出しを追記すること。

`.dev/project.md` ファイルには厳格なサイズ上限が課されます。サイズに基づく拒否は、実行の再試行ではなく内容の刈り込みを必要とする。レンダリング処理は、サイズ制約を回避するために部分的なアダプターセットを書き込むことを厳格に避ける。

`Tech Stack` 表の下で、`.dev/project.md` は `<!-- gal:authoritative-check -->` マーカーを持ち、それに続く `json` コードフェンスが `{"command": [...]}` の形状を定義する。`command` 配列の各要素は空白で分割され、シェル補間なしにそのまま実行される。生成されるコマンドには作業ディレクトリが設定されないため、各コマンドはリポジトリルートへ自動的にリセットされるのではなく、`gal finalize-check` を呼び出したプロセスの作業ディレクトリを継承する。実行すべきコードコマンドを持たないドキュメント専用リポジトリでは、この配列を `["true"]` に設定することが回復経路となる。`gal finalize-check` ゲートは、パイプラインの最終検証の際にこの authoritative check フェンスを消費する。

### `gal doctor`

`gal doctor` コマンドは、バイナリ、正規ルート、実行環境の面、設定を含めて、ローカルセットアップの健全性を検証する。インストール後、アップグレード後、またはエージェントが GAL コマンドを検出できないときに、このコマンドを実行すること。

`gal doctor` は次の項目を検査する。

- **OpenCode 投影ドリフト** — `~/.config/opencode/` のコマンドとエージェントのファイルが正規ルートのレンダリング結果と一致しなくなった状態を検出する。古い内容は警告、欠落したファイルはエラーとして報告する。修復するには `gal refresh` を実行する。
- **Claude スキルサーフェス** — `~/.claude/skills/gal` がない場合、エラーではなく警告と手動作成手順を報告する。`gal refresh` はこのサーフェスを自動作成しない。

初期化済みのリポジトリ内では、2 つのアダプタールート（`AGENTS.md` と `CLAUDE.md`）を対象とする読み取り専用のアダプターサイズ助言表も追加で出力する。サイズ超過のアダプターはエラーではなく `[WARNING]` の所見を生成し、単発の実行失敗を防ぐ。

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

- **Claude Code** は主要な skills-directory ジャンクション経由で `gal@skills-dir` としてキャッシュなしでインプレースに読み込むため、コマンドやスキルの更新は新しいターンで即座に反映されます。（副次的なマーケットプレイスコピーモードを使用している場合は、更新されたプラグインを再読み込みするために Claude Code を再起動してください）。
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

### Test-first パイプライン操作

`Pipeline Contract: test-first-v1` とマークされたタスクは、確定的なレシート駆動のライフサイクルを使用する。マーカーは計画の移行（transition）によって生成され、エグゼキューターによって生成されることはない。マーカーのないプロンプトは `legacy` であり、マークされたタスクは `Test-first: required` または `Test-first: not-applicable` のいずれかである。テストの有無やエージェントの意見から適用可能性を推測してはならない。

#### 適用可能性、runner 文法、および安全なパス

- **Required：** CODER はドメインロジックを含まない scaffold のみを作成できる。TESTER は凍結された probe を実行し `Expected failures` の赤（red）の結果を記録する。CODER はコミットせずに実装を行う。ORCHESTRATOR はまったく同じコマンドを再実行して緑（green）を取得する。AUDITOR は dirty tree をレビューし、最後に ORCHESTRATOR が実装コミットを実行する。
- **Not-applicable：** CODER は赤の probe なしで実装を行う。ORCHESTRATOR は correctness gate を実行する。TESTER はロックされた非赤の probe を実行する。AUDITOR は dirty tree をレビューし、その後に実装コミットが発生する。
- **Legacy：** 既存の implement → correctness gate → commit → test → audit の順序が維持される。legacy タスクが黙って test-first に昇格することはない。

Probe runner の文法は `gal test-first-probe run <plan> <task> <generation> <contract_digest> <phase> <expectation> <id> <selector> <argv_b64> <timeout_ms> [--env KEY=VALUE] [--expected-failure TEXT]` である。10 個の位置引数はすべて必須であり順序が固定されている。`argv_b64` は base64url エンコードされた正規 child argv であり、生の末尾引数リストではない。文法内に `--` argv 終端記号は存在しない。`argv_b64` を素の引数リストから生成する唯一の公認手段は `gal test-first-probe encode-argv <arg>...` であり、呼び出し側が `probe_evidence.rs` のエンコード形式からバイナリフレームを手で組み立ててはならない。runner は環境、タイムアウト、stdout/stderr のキャプチャ、終了ステータス、およびレシートを所有し、`<plan>`/`<task>` から計画スコープのレシートパスを自律的に導出する。shell pipeline や手書きのレシートで代用することは厳禁である。`SKILL.template.md` の §2d（Case 1 の赤、Case 2 の非赤パス）と §2f（緑の再実行）には、`gal test-first-probe run` の正確な呼び出しとパラメーターの取得元がインラインで明記されており、マークされたタスクはそこに記載された手順のみに従って `pipeline-converge-check: pass` へ到達する。

Production パスと test パスは 2 つの名前付きリストであり、互いに素な 2 つのリストではない。テスト対象の seam が private である場合、`Test-first: required` のタスクは同じファイルを両方に記載してよい（private な `fn` はそのファイルの内部からのみ可視であるため、その単体テストは同じファイルの `mod tests` ブロックに置く必要がある）。これによって可能になる凍結はファイル単位ではなく内容単位である。すなわち CODER は、production パスでもあるファイルの中であっても、ロックされた seam のテスト項目を追加・削除・変更してはならず、TESTER 側の production 内容に対する鏡像の凍結も同じ仕組みで働く。各パスはリポジトリルートの下で解決され、`ValidatedRepoPath` として検証されなければならない。絶対パス、`..` トラバーサル、シンボリックリンク、ジャンクション、リパースポイント、および非正規ファイルは fail closed となる。バリデーターは一般的なファイルシステムサンドボックスではない。無関係の特権プロセスがパスコンポーネントをレースさせる可能性があるため、消費証明（consuming proof）は使用直前に identity を再確認する。

#### Canonical-root 信頼アンカーと observable-checkpoint 脅威モデル

Canonical source root（ソースチェックアウト内の `plugins/gal-core/`、または GAL が選択したパッケージ化 canonical root）は、契約の信頼アンカーである。プロバイダーから見える投影、エグゼキュータープロンプト、レシート、およびログは派生した証拠であり、新しい権威にはならない。上位の破損した root はディスパッチを停止し、下位の root へフォールスルーすることは決してない。

証拠は observable checkpoints でのみ受け入れられる。起動前のレシート準備、子プロセスの termination、レシート発行、transition の書き込み、boundary evaluation、audit、および commit 検証である。タイムアウト、wait I/O エラー、部分出力、欠損した terminal record、または未確認の termination は不確実性の観察であり、成功の証明ではない。プロセスリースは同一の receipt identity に対する GAL ディスパッチをシリアル化するが、無関係の特権レーサーを防ぐことはできない。

Hard link やレーサーの置換は信頼保証の対象外である。既存のリンク／リパースコンポーネント、hard-link の曖昧さ、リンクされたレシートターゲット、および非 UTF-8 ターゲットは拒否される。Unix および Windows の identity チェックは、プラットフォームで利用可能な完全な 128-bit identity（Unix では device/inode、Windows では volume/file identity）を使用し、切り捨てられた値やパスのみの比較は使用しない。

消費操作（consuming operation）は、identity と regular-file のチェックに再び合格した後にのみファイルを再オープンまたは消費する。いかなるパス遷移も以前の証明を無効化する。この再確認は消費証明の一部であり、省略可能な最適化ではない。

#### Transitions、generations、および証拠

プロンプト／ステータスの変更は、transition writer によって生成される消費パス遷移（consuming path transitions）である。各 transition は prior digest、next digest、generation、producer、および outcome を記録する。マークされた transition 生成端は、ロック後かつ journal やプロンプトへの書き込み前に新旧の contract-region digest を比較し、`contract-change` の下でのみ変更を許可する。Transition journal は Git で追跡可能である。実行中の計画自体の `.dev/pipeline/journal/<slug>/transition.journal.tsv` のみが、すべての境界種別（`state-recording`、`post-test`、`post-audit`、`implementation-commit`）において `gal boundary-check` の allowlist から免除される。Lock、backup、receipt、snapshot、および他の計画の journal は、いかなる境界種別でも境界免除を受けない。ベースラインの復元や dispute recovery では generation がインクリメントされる。implementation defect は同一の generation で再試行される。エグゼキューターは、証拠を捏造するために、ロックされた seam の凍結されたテスト項目を追加・削除・変更したり、prompt markers、journals、receipts を編集したりしてはならない。凍結はファイル全体ではなく seam の内容に適用されるため、同一ファイルを対象とするタスクでも、そのファイル内の production の編集は引き続き許可される。

Evaluator は red と green の証拠を同一の正規 argv、task、phase、generation、および identity にバインドする。Expected failure は、要求された赤の probe が規定された方法で失敗したことを意味する。`NotRun`、spawn failure、タイムアウト、欠損した証拠、外部の証拠、または非正規パスは決してパスとはみなされない。確認された terminal record のない `started` 端末マーカーは未終了の試行であり、fail closed となる。

#### Disputes、recovery、cleanup、およびコミット

**ORCHESTRATOR** のみが不一致を `probe-defect`、`implementation-defect`、または `contract-ambiguous` に分類できる。Probe defect と contract ambiguity は検証済みの production ベースラインを復元し、generation をインクリメントし、新しい赤の証拠を要求する。implementation defect は同一の generation で再試行される。未知または未解決の dispute は plan/refining 境界で停止する。緑を取得するためにテストを編集してはならない。

AUDITOR は未コミットの正確な task diff をレビューする。実装コミットは、correctness、test、および audit のレシートがすべてパスするまで延期される。エグゼキューターは決して `git commit` や `git push` を実行しない。

この境界には決定論的な commit gate が存在する。ステージング前の clean index、正確な dirty-set とキャッシュされたパス／ハッシュ、base と parent のコミット、range diff、receipt digests、およびコミット後の清潔度を検証する。**ただし、まだ接続されていない：** `gal test-first-commit run` を呼び出す契約は存在しないため、現在 orchestrator は通常の `git commit` を使用しており、これらの検査は実行されない。前の段落は、パイプラインが実際に強制している延期規則として読むこと。ゲートが検証済みであるという主張として読んではならない。

マーカーのないプロンプトは、明示的な無バインドセマンティクスを持つ legacy の動作を維持する。Legacy の順序は `implement → correctness gate → implementation commit → test → audit` のままであり、scaffold、赤のフェーズ、または推論された test-first 契約は存在しない。マーカーのないプロンプトは transition-journal バインドを実行せず、journal も作成しない。`legacy-bootstrap` CLI 生成端および書き込み端は削除されている。transition 読み取り端は既存の `legacy-bootstrap` 行の歴史的記録互換性を維持するが、書き込み可能な transition は新しい bootstrap 記録の作成を拒否する。マーカーのないプロンプトはバインドされず、journal も作成しない。

`test-first-cleanup` は計画スコープ、全か無か（all-or-nothing）、かつクラッシュセーフな操作である。削除を行う前に、閉じた cleanup 所有権に対して 2 つのオプションのルート（`.dev/pipeline/receipts/<slug>` および `.dev/pipeline/snapshots/<slug>`）およびすべてのエントリを事前検証する。欠損しているオプションのルートは記録された no-op としてパスする。追跡対象ファイル、非 cleanup 所有権のエントリ、リンクやリパースの曖昧さ、または事前検証の失敗は、ゼロ変形で拒否され、失敗レシートを発行する。安全な cleanup は、削除前にルートをクアランティンし、同じ親への移動と子 identity を検証し、確定的パスまたは失敗レシートを書き込む。`.dev/pipeline/receipts/.locks/` ディレクトリ全体を削除してはならない。子プロセスや SSH ヘルパーが残っていないことを確認した上で、指定された stale lease のみを検査して削除すること。

### Finalize：着地するものと削除されるもの

`/gal finalize` コマンドは、完了した計画のクローズをオーケストレートする。前提条件として、すべてのタスクの完了と検証が強制され、それはゼロトラストの機械レシート（`gal finalize-check`）で証明されます。`gal finalize-check` は読み取り専用作業表面保証（read-only work-surface guarantee）を提供し、検査対象のリポジトリ作業表面（アダプター、ソースファイル、ドキュメント、`.dev/project.md`、`.dev/state.md`、計画ファイル、実行プロンプト）に対する変更を一切行いません。許可される唯一のファイルシステム書き込みは、呼び出し元が指定したパスへの明示的なレシートファイルのみです。

- **着地するもの：** doc-sync の前に、トップダウンで要件を 1 つずつ辿るレビューが、finalize 自身のランタイムでブランチ全体に対して一度だけ実行される。レビューは各要件について L1 Truths、L2 Files、L3 Wiring、L4 Trust boundaries の 4 レイヤーを評価し、その結果を `## Review Results` の配下に、要件ごとに 1 行を持つ `### Finalize Review <date>` テーブルと findings のリストとして書き込む。続いて STEWARD エージェントが、計画の永続的な知識を `README.md` と `docs/` へ抽出する。その後、main へのマージが続き、該当する場合はワークツリーの撤去を含みます。
- **1つのコンフリクト形状は自動的に解決される。** main へのマージ時にコンフリクトが発生し、未マージのパスセットが正確に `{.dev/state.md}` である場合、finalize は内部の `gal state-merge` リゾルバーを実行し、計画をキーとするテーブルを行ごとにマージして結果をステージングする。Exit 0 は着地を継続する。`STATE_MERGE: unresolved` はリポジトリが試行前とバイト単位で同一であることが証明された状態で停止し、`STATE_MERGE: rollback-unconfirmed` は `.dev/state.md` が変更された可能性があるとして確認を要求して停止する。他のすべてのコンフリクト形状は無条件停止を維持する。仕組みと拒否された代替案：`docs/architecture.md`。
- **削除されるもの：** `.dev/plans/` 内の計画ファイルは、ドキュメントがコミットされ、書き込み後の衛生チェックがパスした後にのみ削除されます。これにより、ファイル消去の前に永続レイヤーへの知識移転が保証されます。
- **保持されるもの：** 日付、計画、着地コミットを含むクローズアウトの行が `.dev/state.md` に書き込まれます。`### Finalize Review <date>` テーブルに記録された非ブロッキングの findings はすべて `.dev/state.md` の `## Follow-ups` セクションへ upsert され、新しいものから順に並べられて最新 5 行に切り詰められるため、計画ファイルが削除された後もそれらの findings は残ります。`gal-last-good` タグが着地コミットに割り当てられます。

### Finalize ゲート失敗後の復旧

アクションを起こす前に `gal finalize-check` を実行し、すべての行を読んでください。フルモード検査行は、この順序で `authoritative-command`、`naming-gate`、`sync-idempotency`、`finalize-mode`、`project-source-doc-existence`、`state-bound`、`contract-roster-parity`、`doc-link-resolution`、および最終の `working-tree-clean` であり、そのすべてが `pass` でなければなりません。ルートに `plugins/gal-core/` を持つリポジトリでは 9 行、持たないリポジトリでは 7 行になります。`contract-roster-parity` と `doc-link-resolution` は、そのディレクトリが存在する場合にのみ適用されるためです。同じ行がタスクごとに繰り返されることはありません。`sync-idempotency` 検査行は、アダプターをディスクに適用するのではなく、2 回の candidate render にわたる読み取り専用の候補レンダリング決定性（read-only candidate-render determinism）を検証します。candidate とディスクの差異はレポート専用のディスクドリフト（report-only disk drift：`in-sync`、`drifted`、`absent-on-disk`、`extra-marker-owned`、`filter-personalized`）として報告され、ゲートを失敗させることはありません。終了ステータスまたは起動エラー、有界の `stdout`/`stderr`、元のバイト数、および切り捨てフラグを持つのは `authoritative-command` の行のみです。最終の `working-tree-clean` 行は、終了ステータス、dirty エントリ数、およびステータス出力サイズを持ちます。他の行は要約テキストのみを持つため、`naming-gate` の行にコマンド系フィールドがないのは正常であり、欠陥ではありません。衛生専用モードのレシート行（`project-source-doc-existence`、`state-bound`、`durable-layer-commit`、`finalize-review-shape`、`contract-roster-parity`、`doc-link-resolution`、および最終の `working-tree-clean`）は、計画ファイルの削除前に、書き込み後の清潔度に加えて `### Finalize Review <date>` テーブルの機械的な形状を検証します。`NotRun` または欠損した証拠は失敗であり、パスではありません。

障害をクラス別にルーティングしてください：

- 権威コマンド／ツールの失敗、または汚れたツリーには、個別の remediation plan が必要です。finalize の内部で修復を行わないでください。
- `### Finalize Review <date>` テーブルに空または不正なセルがあると、衛生専用レシートの `finalize-review-shape` が失敗します。シーケンス 1 を再実行して形状の妥当なテーブルを生成し、その後で衛生専用チェックをやり直してください。
- 非 `DONE` ワークフローで真に未チェックの作業が残っている場合は、通常の `/gal pipeline` に戻ります。`DONE` に加えて未チェックまたは矛盾する状態が存在する場合はターミナル破損（terminal corruption）です。停止して必要な手動作業交接を作成してください。

失敗した finalize ゲートは、自動修復、コミット、または証拠の書き換えを一切許可しません。

ターミナルリカバリを行う場合は、コミット済みの状態がクリーンであることを確認した後にのみ、以下の順序を実行してください：

```powershell
gal.exe pipeline-preflight --terminal-reverify <execution-prompt-path>
# terminal-reverify レシートがパスすると、オーケストレーターはプロセス内で goal-backward verification を実行します。
gal.exe pipeline-handback-check <execution-prompt-path>
gal.exe finalize-check <execution-prompt-path>
```

`terminal-reverify` はプロンプト専用かつ読み取り専用です。`Workflow: DONE`、すべてのタスクがチェック済み、current-task カーソルがクリア済み、未解決の再試行／中断または手動交接がなく、ツリーがクリーンであることが要求されます。マークされた transition-journal ダイジェストの同一性を再利用し、バインドされたマーク付きターミナルレシートチェーンを記録します。診断用の `terminal-binding` はマークされた transition-journal ダイジェスト同一性とターミナルレシート SHA-256 バインディングを明示的に評価しますが、マーカーのないプロンプトは明示的な legacy/no-binding セマンティクス（journal なし、ダイジェストバインディングなし）を宣言します。実装、テスト、監査、セキュリティ作業、または remediation の派生、プロンプトの変更、証拠の修復、ゲートの弱体化、コミットの作成を行うことはできません。ファイナライズを許可する新しい handback レシートからのみ進行し、その後フル finalize チェックを再実行してください。

### エージェント契約の解決

プロンプトまたはソース計画から構築された pipeline フェーズをディスパッチする前に、`gal` は権威あるエージェント契約（`agents/golem-{implementer|tester|auditor}.agent.md`）を特定し、その正確なバイト列をエグゼキューターに送信されるタスク仕様に埋め込む。ローカルまたは SSH レーンのいずれ経由でディスパッチされたエグゼキューターも、制御ノード専用の契約パスを開くよう指示されることはなく、仕様は自己完結している。

**解決順序（最初のソースルートが勝利）：**

| ティア | `contract_source` | ルート |
| --- | --- | --- |
| 1 | `workdir` | 正規化された `--workdir` 自体、またはその直下の `plugins/gal-core` |
| 2 | `ancestor` | GAL ソースルートとして認識される workdir の最も近い祖先 |
| 3 | `exe-side` | 実行中の `gal` バイナリの隣のディレクトリ |
| 4 | `embedded` | `~/.gal/embedded-src` に具現化されたコピー |

`workdir` は `ancestor`、`exe-side`、および `embedded` よりも優先される。これにより、PATH 上に異なるバージョンのパッケージ化された `gal` バイナリが存在する場合でも、信頼できるローカルの GAL チェックアウトが権威を維持する。`plugins/gal-core/` をベンダリングしているリポジトリは、インストールされているバイナリのバージョンに関係なく、常に自身の契約を解決する。ベンダリングされたチェックアウトを維持している場合、バージョン乖離の確認は `Dispatch:` マーカー上の `contract_source`（[ディスパッチの検査](#ディスパッチの検査) を参照）で行う。

**障害の形状による復旧の違い：**

- **勝利ルートの破損** —— 最高位のルートが見つかったが、選択されたフェーズ契約が欠損しているか、UTF-8 でないか、読み取り不能である場合。ディスパッチは起動前に停止し、ティアとルートを指名するエラーを表示する。下位ティアへのフォールスルーは決して行われないため、下位ティアがカバーすると仮定せず、指名されたルートのファイルを修正すること。
- **全ミス** —— いかなるティアも使用可能なルートを提供しない場合。ディスパッチは停止し（終了コード 1）、全ティアの結果が一覧表示され、両方の復旧経路（パッケージングチャネル経由で `gal` を再インストールするか、GAL ソースチェックアウトからコマンドを実行する）が提示される。

**素の／直接ディスパッチの例外。** 素の `gal dispatch` および素のタスク仕様 `gal pipeline` 入力は、このプロベナンスを解決したり捏造したりすることはない。それらのマーカーおよびエグゼキューターログヘッダーは、プロベナンス導入前の形式とバイト単位で互換性を維持する。これらのパスでは `contract=`/`contract_source=` フィールドが存在しないことが期待される。

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

**ソース 1 —— 個人の規約ファイル（常時オン、リポジトリごと）。** 同梱の例 `plugins/gal-core/templates/csharp-convention.example.md` を `~/.gal/local/conventions/csharp.md` に複製し、内容を対象のスタイルに合わせて変更する。個人の規約ファイルは、その stem が `csharp`、`typescript`、`javascript`、`go` のいずれかである場合にのみ選択され、それ以外の stem は除外される。言語横断の stem は存在しない。従来の常時取り込みの動作に依存していたオーナーは、規約ファイルの stem をこれらのエイリアスのいずれかに改名することで復旧できる。`.dev/project.md` に一致する `Language` 行を持つ対象リポジトリ内で `gal render-adapters` を実行する。システムは gal-core の規約プロセスを模して、そのファイルをリポジトリのアダプターへ注入する。

**ソース 2 —— インストール済みのエージェントプラグインの検出（読み取り専用、セットアップ不要）。** コーディングエージェント内にインストール済みの既存の公式言語プラグインは、対象リポジトリで `gal render-adapters` を実行するだけで済みます。GAL はプラグインのスキル名と出所をリポジトリの `Language` 行と突き合わせ、名前と永続的な読み込み指示を含む **Detected Language Skills** 参照ブロックをレンダリングする。GAL はスキル内容の複製を厳格に避け、プラグインのインストール・更新・削除を行わない。新しくインストールされたプラグインの検出には、続いて `gal render-adapters` の実行が必要である。

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

3 つのグループすべてが閉じた役割の許可リストを強制する。`pipeline` の一覧は `{CODER, TESTER, AUDITOR}` を含みます。`planning` の一覧は `{ARCHITECT, ANALYST, DESIGNER, RELEASER}` を含みます。`research` の一覧は番号付きキー `"1"` と `"2"` のみを受け付け、どちらも省略可能で、グループ自体も省略可能である。誤ったグループに割り当てられた役割やキー、および認識されない項目はスキップされ、正しいグループを示す警告を生成する。役割キーを `executorRouting` 直下に置くレガシーのフラット構造は廃止され、フォールバックなしで可視の警告を生成する。

**executors ブロック：** ツールごとの既定モデルを定義する。モデル指定を欠く役割の項目は、`executors[executor]` から既定を継承する。役割に明示されたモデル宣言は既定を上書きする。

**役割ごとの effort キー：** オプションの推論強度の指標を提供し、起動 (spawn) の前に各エグゼキューターのネイティブな推論フラグへマッピングされます。

| エグゼキューター | `effort` → ネイティブフラグ |
| --- | --- |
| claude | `--effort <value>` |
| codex | `-c model_reasoning_effort="<value>"` |
| copilot | `--reasoning-effort <value>` |
| opencode | `--variant <value>` |
| agy | `--effort <value>` |

各エグゼキューターで受け入れられる `effort` の値は以下の通りです：

| エグゼキューター | 受け入れられる `effort` の値 |
| --- | --- |
| claude | `low`, `medium`, `high`, `xhigh`, `max` |
| codex | `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` |
| copilot | `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` |
| opencode | provider-specific |
| agy | `low`, `medium`, `high` |

GAL は argv の安全性のみを検証し、ディスパッチの前にエグゼキューター固有の語彙を検証することはありません。

Antigravity (`agy`) は推論 effort に対して排他的論理和 (exclusive-or) ルールを強制します：effort ティアをサポートするモデルの場合、推論強度は正確に 1 回だけ指定されなければなりません —— effort サフィックス付きのモデル slug（`gemini-3.7-flash-medium` など）として指定するか、ベース slug を使用して `--effort <value>` 経由で指定するかのいずれかであり、両方を指定したり、どちらも指定しなかったりすることはできません。effort 設定なしでベース slug を指定すると agy のエラー (`requires --effort`) が発生し、effort サフィックス付きの slug と一緒に `--effort` を渡すと agy の衝突エラー (`conflicts with --effort`) が発生します。effort ティアを持たないモデルは、あらゆる形式の `--effort` を拒否します。

独自の `model` を持たない役割は `executors.agy` を継承するため、この排他的論理和ルールはすべての agy ルートに適用されます。有効な設定は、すべての agy ルートにわたって次の 2 つの合法的な形状のいずれかを使用する必要があります：

1. **House shape (推奨)：** すべての agy 役割が明示的な `effort` を保持し、`executors.agy` はベース slug（例えば `gemini-3.7-flash`）のままにしておきます。これは medium 優先の既定値と一致します。
2. **合法的な代替案：** `executors.agy` に effort サフィックス付きの slug（例えば `gemini-3.7-flash-medium`）を保持させ、いずれの agy 役割にも `effort` を設定しません。

2 つの形状を混在させること（役割に明示的な `effort` を設定しながら `executors.agy` で effort サフィックス付きの slug を使用すること）は、agy の衝突エラー (`conflicts with --effort`) を引き起こします。

この仕組みは fail-closed で動作する。effort ヒントを尊重できない、または不正な値に遭遇したエグゼキューターは、ヒントを黙って無視するのではなく、起動の前にディスパッチを劣化させます。`effort` キーは純粋にヒントとしてのみ機能し、モデルセレクターとして動作したり、有料または名前付きのモデルを含意したりすることは決してない。

**役割ごとの `timeoutSecs` キー：** その役割のディスパッチタイムアウトをオーバーライドするオプションの正の整数であり、ローカルおよび SSH（リモート）ルートに同一に適用される：

```json
"pipeline": {
  "CODER": { "executor": "codex", "timeoutSecs": 900 }
}
```

`timeoutSecs` を省略した役割は、変更なしの 300 秒の CLI 既定値を維持する。`0` を設定した役割は、ロード時に警告とともに拒否され、キーが存在しないかのように動作する —— ゼロが意味のあるタイムアウトになることはない。`timeoutSecs` の設定は `dispatch-script` のコマンドライン文法を一切変更せず、ルーティングが解決された後にバイナリが適用するタイムアウト値のみを変更する。

**アダプター動作の注記：**

- **OpenCode** は、書き込みを黙ってブロックする読み取り専用の既定を回避して書き込み可能なエージェントを有効化するため、`--agent build` を用いてディスパッチする。また権限バイパスフラグとして `--auto` を用いる。
- **Copilot** は、ヘッドレスのプロンプトモードがコンテキスト上限を超過するのを防ぐため、`--no-custom-instructions` と `--disable-builtin-mcps` を付加する。Copilot Free は厳格に **auto 専用**で動作し、`model: auto` とローカル専用の実行を維持する。

**役割の表：**

| 役割 | グループ | 目的 |
| --- | --- | --- |
| `CODER` | pipeline | 計画に従って実装コードを書きます。可能な場合は `TESTER` と異なることが推奨される |
| `TESTER` | pipeline | 計画の仕様と公開 API のみからテストを書きます。可能な場合は `CODER` と異なることが推奨される |
| `AUDITOR` | pipeline | 深いパフォーマンスとセキュリティを監査する。可能な場合は `CODER` と異なり、ティアは `CODER` 以上であることが推奨される |
| `ARCHITECT` | planning | トレードオフ、過剰設計、バグを対象とする対抗的な計画レビュー |
| `ANALYST` | planning | ROI、ドメインの正しさ、ユーザー影響を対象とするビジネスロジックレビュー |
| `DESIGNER` | planning | UX、UI、DevEx のレビュー |
| `RELEASER` | planning | リリースフローの設計（読み取り専用） |
| `RESEARCHER` | research | 3 つの並列かつ相互に不可視なワーカー（`RESEARCHER#0`/`#1`/`#2`）の 1 つとして研究課題を調査する |

**省略可能な `research` グループ。** `/gal research` と `/gal deep-research` は、それぞれ `#0`、`#1`、`#2` と番号付けされた 3 つの並列 `RESEARCHER` ワーカーを起動する。`executorRouting` 配下の `research` オブジェクトは番号付きキー `"1"` と `"2"` のみを受け付け、`"0"` は決して受け付けない。どちらのキーも省略可能で、`research` グループ全体も省略可能である。3 つのワーカーがどこに解決されるかにかかわらず、3 つのブリーフの比較、主張の裁定、最終ドキュメントの執筆は ORCHESTRATOR が所有しインプロセスで実行する。ルーティングが影響するのは、各ワーカー自身の調査がどこで実行されるかだけである。

- **`#0` は固定である。** `RESEARCHER#0` は常にネイティブサブエージェントである。設定キーを持たず、`executorRouting` で参照されることもない。設定できる `research."0"` の項目は存在しない。
- **`#1` と `#2` はそれぞれ独立に、同じ 3 分岐ルールにより、次の優先順位で解決される：**
  1. 明示的な `research."1"`（または `research."2"`）の項目があれば、それが優先される。
  2. なければ `executors.agy` の優先設定が適用される。ワーカーは `executors.agy` をモデルとして `agy` にルーティングされ、そのルートに `effort` は設定されない。
  3. どちらでもなければ、ワーカーは `#0` と同様にネイティブサブエージェントにフォールバックする。
- **agy の排他的論理和ルール（[上記](#ヘッドレスエグゼキューターのルーティング)）との相互作用：** 分岐 2 がワーカーを `agy` に解決する場合、そのルートは意図的に `effort` キーを持たない。排他的論理和ルールは引き続き適用されるため、`executors.agy` 自体が推論強度を供給しなければならない。すなわち effort 接尾辞付きのモデルスラッグ（例：`gemini-3.7-flash-medium`）を使用する。`executors.agy` が接尾辞のないベーススラッグである場合、この分岐では `--effort` を明示的に設定することが許されないため、agy のルートは標準の `requires --effort` エラーで失敗する。

**設定なしの場合の結果：** `research` グループをまったく設定していない場合、3 つのワーカー `#0`、`#1`、`#2` はすべてネイティブサブエージェントに解決される。`/gal research` や `/gal deep-research` を使うためにリポジトリが `executorRouting` に手を加える必要はない。

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

**終端状態の語彙。** ログヘッダーはディスパッチの終了状況を以下の終端状態に分類する。

| 終端状態 | 意味 |
| --- | --- |
| `completed` | プロセスが 0 で終了し、レシートの送達が確認され、ワークディレクトリにも想定どおりの変更がある。 |
| `no-receipt` | プロセスは 0 で終了したが、想定されたレシートファイルが存在しない、空、または未確認である。 |
| `workdir-escape` | 過去のログ語彙とマッチの網羅性のために残されているのみで、本番のディスパッチ経路では生成されない。 |
| `no-writeback` | プロセスが 0 で終了しレシートも送達したが、porcelain と内容の比較において、割り当てられた git worktree に変更が一切ない。 |
| `timeout` | 設定されたタイムアウトが経過したためプロセスが終了した。 |
| `disconnected-partial` | プロセスが非ゼロのステータスコードで終了した。 |
| `unavailable` | ルーティングされたエグゼキューターの CLI バイナリが PATH 上に見つからなかった。 |

`no-writeback` 状態には重要な運用上の境界がある。

- **フェーズ範囲：** `no-writeback` は `scaffold` と `implement` フェーズにのみ適用され、`audit` と `test` は対象外である。後者は契約上の成果物をすべて gitignore されたレシートファイル（`.dev/pipeline/receipts/`）経由で提供するため、git worktree のスナップショットからは見えない。
- **同一フラグ境界：** 追跡対象または未追跡のファイルがディスパッチの前後で同じ `git status --porcelain` フラグを保つ場合（あらかじめ dirty だったファイルなど）、SHA-256 の内容ダイジェスト比較によって内容変更を検出し、実際に成果物があるケースが `no-writeback` と誤分類されないようにする。
- **gitignore 書き込み境界：** あるフェーズの書き込みが gitignore されたパスにのみ着地する場合、エグゼキューターが実際に作業していても `no-writeback` として報告される。ワークツリーのステータススナップショットはリポジトリの変更を追跡し、gitignore されたパスを無視するためである。
- **提供契約との結合：** この分類器は現在のフェーズ提供契約、すなわち `scaffold` と `implement` は追跡対象のリポジトリファイルで提供し、`audit` と `test` はレシートで提供するという前提を符号化している。あるフェーズの提供方法を変更する場合は、この分類器も見直す必要がある。

**`contract`/`contract_source` フィールドの読み取り。** プロンプト／ソース計画入力に対して pipeline が作成したディスパッチは、`Dispatch:` マーカー行およびエグゼキューターログの開始ヘッダーと終端ヘッダーの両方に `contract=<control-node-abs-path> contract_source=workdir|ancestor|exe-side|embedded` を付加する。これらは同じ結合された値であるため、3 つのいずれからもエグゼキューターが実際にどのエージェント契約に沿ってどこから実行されたかを確認できる。`contract_source=workdir` または `ancestor` はローカルの GAL チェックアウトが契約を提供したことを意味し（動作が古く見える場合はそのチェックアウトを確認する）、`exe-side` はパッケージ化されたバイナリ自体のバンドルコピー、`embedded` は `~/.gal/embedded-src` に具現化されたフォールバックコピーを意味する。素の `gal dispatch` および素のタスク仕様 `gal pipeline` の実行は、これらのフィールドを完全に省略する —— それらの欠損は期待される動作であり、欠陥ではない。

**プロベナンスでゲートされた `effort` フィールドの読み取り。** pipeline が作成したディスパッチのみ（プロベナンスが存在する場合）、実行内のすべての成功／劣化マーカーは `contract`/`contract_source` 接尾辞の直前に浄化された ` effort=<value|(default)>` フィールドを保持する。これはルーティング解決後に一度計算され、その実行の残りの期間で変更されずに再利用される。素の／直接ディスパッチおよび `no-routing` 劣化（ルートが解決されなかったためプロベナンスなし）は `effort` フィールドを保持せず、それらのマーカーは effort 導入前の形式とバイト単位で同一を維持する。ルーティングが `OFFLOAD` ブロックを生成する場合、`gal dispatch-script` は `REPORT_LINE` フィールド（`Dispatched: <phase[ (fix)]> <T-NN> - <ROLE> as <executor>, model <model>, effort <effort>`）も事前レンダリングし、オーケストレーターは各ディスパッチ時点でこれを逐語的に一度だけアナウンスする。

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

#### レシートリース失敗の復旧

レシートリースは、2 つの GAL ディスパッチが互いの決定論的なレシートパスを認証するのを防ぐ。`reason=receipt-preparation-failed` または `reason=remote-receipt-freshness-failed` は、アクティブまたは期限切れのリースの可能性として扱い、レシートを広範に削除する許可として扱わないこと。正確なローカル `.dev/pipeline/receipts/.locks/<hash>.lock` については stderr を読み取ること。リモートの鮮度 stderr には `lock=.dev/pipeline/receipts/.locks/<hash>.lockdir` が含まれ、その `owner` ファイルがクリーンアップの権限となる。削除する前に、一致する GAL/SSH エグゼキューターがまだアクティブでないことを独立して確認すること。特にタイムアウトや wait I/O エラーの後は、それらの結果自体が終止 (`termination`) を証明するわけではないためである。その後、指定されたロックファイルのみ、または指定されたリモート `owner` ファイルとそれに続く空になったロックディレクトリのみを削除すること。`.locks/` ディレクトリ全体を絶対にクリアしないこと。

`remote-receipt-fetch-failed` は、実行後のソースが欠落、空、リンク、非正規、または取得できなかったことを意味する。`remote-receipt-fetch-timeout` は、上限付き取得の期限が切れたことを意味する。`remote-receipt-fetch-read-failed` は、パイプ読み取りエラーにより GAL が認証を拒否した一部の stdout が残ったことを意味する。`remote-receipt-fetch-too-large` は、ヘルパーがソースをドレインしたが 1 MiB の stdout 上限を超えて保持することを拒否したことを意味する。Stderr にも同じ保持上限がある。`remote-receipt-fetch-*-unconfirmed` または `remote-receipt-lease-cleanup-*-unconfirmed` の理由は、ローカルの補助 SSH プロセスが確認済みの終了証拠を提供しなかったため、その上限付きパイプドレインが結合されなかったことを意味する。`receipt-lease-cleanup-failed` は、チェックされたローカル/制御側のロック削除が正常または早期リターンパスで失敗したことを意味する。正確な stderr パスを使用し、結果をダウングレードした状態に維持すること。クリーンアップがプライマリガード/ログ失敗を伴う場合、マーカーは理由トークンを `+` で結合するか、返されたエラーテキストが両方を指定する。両方を調査すること。`remote-receipt-lease-cleanup-failed` および `remote-receipt-lease-cleanup-timeout` は、owner-token のクリーンアップが確認されなかったため、エグゼキューターが既に失敗していた場合でもロックが意図的に fail-closed のままになることを意味する。エグゼキューター自身の終了 73 は終了 73 のままとなり、ラッパーの専用の鮮度センチネルが存在しない限り、鮮度失敗にはならない。

#### フェーズ書き戻し失敗の復旧

対象となるディスパッチ（任意のプロンプトに対する `audit`、およびマーカーなしプロンプトに対する `test`）では、エグゼキューターの終端状態 `completed`（終了コード 0）は `<task>-<phase>.receipt.md` へのレシート送達を表すものであり、フェーズの完了やプロンプトへの配置を表すものではない。プロンプトの配置は `gal` コントロールノードが所有し、ディスパッチ完了後に fail-closed（既定で遮断）のセマンティック書き戻しゲートを実行する。

**観測できる失敗の兆候：** ペイロード検証またはレンダリングが失敗した場合、`gal` は `phase-writeback semantic failure for task <T-NN>` を stderr に出力し、パイプラインループログに構造化エラーレコードを追記し、非ゼロで終了し、ディスク上の実行プロンプトファイルをバイト単位で不変のまま残す。これが**プロンプト不変の保証**である。

**範囲と例外：**

- ローカル実行経路と SSH 実行経路はいずれも、同一のコントロールノード側セマンティックゲートでレシートペイロードを処理し、同一の決定的なプロンプト配置結果を生成する。
- `PipelineInput::RawSpec` と `gal dispatch` の直接実行はセマンティック書き戻しゲートを迂回し、レシートのみに留まる。これらのモードでは権威ある実行プロンプトファイルが存在しないためである。

**よくある原因と復旧手順：**

- **投影されたエージェント契約の陳腐化：** エグゼキューターが不正な形式のレシート内容（コードフェンスで囲まれていない余分な見出し、判定マーカーの欠落など）を出力する場合、またはプロンプトを直接編集しようとする場合は、`gal refresh` を実行してリポジトリローカルおよび投影されたエージェント契約を現在の規則に更新する。
- **レシートペイロードの形式不正：** レシートファイル（`.dev/pipeline/receipts/...`）またはパイプラインループログを調べ、具体的な検証エラーを特定する。例えば `### [T-NN] YYYY-MM-DD` ヘッダーの欠落、余分な H2/H3 見出し、タスク ID の不一致などである。
- **修復と再実行：** 契約が陳腐化していた場合はまず `gal refresh` を実行し、レシートまたはディスパッチ側の問題を解消したうえで `/gal pipeline` を再実行する。オペレーターが Markdown のサブセクションを実行プロンプトファイルへ手作業で移動したりコピー＆ペーストしたりしてはならない。プログラムによる配置こそが、正しい H2（`## Test Results` または `## Review Results`）配下での決定的な順序を保証する。

### ディスパッチされたワーカー境界とプロジェクト指示分離

`/gal pipeline` がタスクフェーズをヘッドレスエグゼキューターにディスパッチする際、セカンダリのコーディングエージェントはオーケストレーターの役割ではなくワーカーの役割で実行される。ディスパッチされたワーカーがリポジトリレベルの指示（ワークフロー遵守ルールやオーケストレーターゲート要件など）をパイプライン制御コマンドを実行するためのコマンドと解釈するのを防ぐため、GAL は 2 つの補完的なメカニズムを通じて役割の分離を確立する：タスク仕様境界と、アダプターレベルでのプロジェクト指示抑制である。

#### ディスパッチされたワーカー境界

レンダリングされたすべてのタスク仕様には、メタデータ区切り記号の後、`## Task Goal` の前の目立つ位置に `## Dispatched Worker Boundary` ブロック（旧 `## IMPORTANT: Orchestrator Gate Boundary`）が埋め込まれる。この節は：
- ワーカーをオーケストレーターではなく、単一のスコープ付きフェーズエグゼキューターとして特定する。
- 必要なすべてのパイプラインゲートはオーケストレーターによって所有および充足されるため、任意の `gal` サブコマンド（`gal pipeline-preflight`、`gal pipeline-handback-check`、`gal boundary-check`、`gal pipeline-converge-check`、`gal pipeline` を含む）の呼び出しを明示的に禁止する。
- ワークフローの `SKILL.md` ファイルの読み込みまたは実行を明示的に禁止する。
- エージェントにオーケストレーターワークフローの実行を指示するリポジトリレベルの指示やスキル指示を上書きする。ディスパッチされたワーカーは、スコープ付きのタスク目標、ファイル許可リスト、および埋め込まれた `## Agent Contract` にのみ従う必要がある。

#### プロジェクト指示分離マトリクス

エグゼキューター CLI によっては、起動時にリポジトリレベルの指示ファイル（`AGENTS.md` または `CLAUDE.md`）がモデルの指示コンテキストに自動的に読み込まれる場合がある。サポートされている場合、GAL ディスパッチアダプターはフラグを渡してプロジェクトレベルの指示注入を抑制し、自己完結型のタスク仕様が干渉なしに実行を統制できるようにする：

| エグゼキューター | 露出状態 | 抑制メカニズム | 検証バージョン | 分離の振る舞い |
| --- | --- | --- | --- | --- |
| **codex** | 露出 (`AGENTS.md`) | `-c project_doc_max_bytes=0` | codex-cli 0.149.1 | リポジトリレベルの `AGENTS.md` を抑制するため、無条件で `-c project_doc_max_bytes=0` を渡す。グローバルの `~/.codex/AGENTS.md` レイヤーはアクティブのまま維持される。 |
| **copilot** | 露出 | `--no-custom-instructions`, `--disable-builtin-mcps` | Copilot CLI | カスタム指示と組み込み MCP を無効化するフラグを追加し、プロンプトモードの自己完結性を確保する。 |
| **claude** | 露出 (`CLAUDE.md`) | `--setting-sources user` | Claude Code 2.1.251 | ユーザー設定を保持しつつ、リポジトリレベルの `CLAUDE.md` とプロジェクト設定を抑制するため、無条件で `--setting-sources user` を渡す。 |
| **opencode** | `NotRun` | なし | opencode 1.18.25 | 露出プローブのテスト中にプロバイダー側のタイムアウトが発生した；アダプターの抑制は適用されない。未露出ではなく結論不能として扱われる。 |
| **agy** | 設計上露出 | なし（タスク仕様境界の優先） | agy 1.1.23 | ワークスペースのアクセスを制限しリポジトリコンテキストを提供するために意図的に `--add-dir` を渡す；モデルの指示コンテキストではタスク仕様の `## Dispatched Worker Boundary` が優先される。 |

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
| **auditor** | **オーケストレーター経由限定** | `/gal pipeline`（pipeline フェーズ）を通じて。auditor にブランチ全体モードは存在しない。`--finalize-branch-audit` トークンは依然として認識されるが、返るのは非ディスパッチの `COMMAND: error` ブロックである。`/gal finalize` は同一ランタイムで自身のトップダウンレビューを実行し、`Review Independence: DEGRADED_SAME_RUNTIME` を記録して、要件×L1-L4 のテーブルを書き込む。 |
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
