---
source: docs/setup.md
lang: ja
source_commit: 4057c591bf08bc7ee0269f2cbee82bcac3850fa9
translated_at: 2026-09-23
type: Guide
title: セットアップと初期化
description: `gal` バイナリのインストール、対応ランタイムへのプラグイン登録、リポジトリアダプターの初期化と保守を説明します。
tags:
  - setup
  - install
  - runtime
  - init
status: stable
---

# セットアップと初期化

[English](../../setup.md) · **日本語** · [繁體中文](../zh-Hant/setup.zh-Hant.md)

## GAL のインストール

ご使用のオペレーティングシステムに適したインストール方法を選択してください。インストール完了後、対象リポジトリ内で `gal init` を実行することで、リポジトリローカルのアダプターファイル群が生成されます。

### インストール方法

#### ソースコードからのビルド（`cargo install --git`）

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli
```

GAL は crates.io には公開されていないため、`--git` フラグの指定が必須となります。Cargo ワークスペース内でビルドするため、パッケージ名として `gal-cli` を指定してください。生成される実行可能バイナリの名前は `gal` です。

#### Homebrew（macOS および Linux）

```bash
brew install monkey1wizard/tap/gal
```

macOS および Linux ユーザー向けとして推奨されるパッケージマネージャー経由のインストール方法です。

#### WinGet（Windows）

```sh
winget install Monkey1Wizard.GAL
```

コミュニティパッケージインデックスの反映は、新規リリースからわずかに遅れる場合があります。インストール前に `winget show Monkey1Wizard.GAL` を実行し、利用可能なバージョンを確認することをおすすめします。なお、Windows 環境では後述の PowerShell エイリアスに関する注意点をご確認ください。

#### リリースアーカイブからの直接ダウンロード

GitHub Releases ページから、対象プラットフォームに対応したアーカイブ（`gal-<version>-<platform>-<arch>[.zip|.tar.gz]`）を直接ダウンロードします。アーカイブを展開し、含まれる `gal` バイナリ（Windows では `gal.exe`）をシステムの `PATH` 上へ配置してください。macOS および Linux では、`chmod +x gal` を実行して実行権限を付与してください。

#### シェルスクリプト（Linux および macOS）

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
```

このインストールスクリプトは、ダウンロードしたアーカイブを `checksums.txt` の SHA-256 ハッシュ値と厳格に照合し、不一致があった場合は直ちに処理を中断します。続いて Cosign のキーレス署名検証を試行します。Cosign が導入されており無効な署名と判定された場合は、アーカイブの展開前に処理を中止します。Cosign が未導入であるか署名取得に失敗した場合は、警告を表示した上で SHA-256 検証のみを根拠にインストールを続行します。バイナリは `~/.local/bin` に配置され、ソースアセットは `~/.local/share/gal` に保存されます。特定のリリースをインストールしたい場合は、環境変数 `GAL_VERSION` を指定してください。

#### PowerShell スクリプト（Windows）

```sh
irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
```

Windows 用スクリプトも同様に SHA-256 および Cosign による署名検証を実施し、検証失敗時は処理を中断します。`gal.exe` およびソースアセットは `%LOCALAPPDATA%\Programs\gal` にインストールされます。このディレクトリをご自身のユーザー `PATH` 環境変数に追加してください。特定のバージョンを固定してインストールしたい場合は、`$env:GAL_VERSION` を設定します。

**PowerShell のエイリアス競合について**: PowerShell には既定で `Get-Alias` にマッピングされた組み込みエイリアス `gal` が存在します。対話型セッションでは、このエイリアスが原因で本来の `gal` バイナリの呼び出しが覆い隠される（シャドウされる）ことがあります。`gal` を呼び出した際に `Get-Alias` が起動してしまう場合は、明示的に `gal.exe` と呼ぶか、絶対パスを指定するか、または PowerShell の `$PROFILE` 内でエイリアスを削除してください。

```powershell
Remove-Item Alias:gal -Force -ErrorAction SilentlyContinue
```

### アップグレードとメンテナンス

現在インストールされているバージョンと、各プラットフォーム別のアップグレードコマンドを確認するには `gal update` を実行します。GAL バイナリ自身には自己更新機能は含まれていません。初期導入時に利用したパッケージマネージャー経由でアップグレードを行ってください。

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli  # Cargo
winget upgrade Monkey1Wizard.GAL                                                   # Windows
brew upgrade gal                                                                   # macOS / Linux
```

引数を付けずに `gal` を実行すると、使い方のヘルプが表示され終了コード `0` で終了します。認識されない不正なサブコマンドが指定された場合は、終了コード `64` で終了します。

## ランタイム別プラグイン登録

Markdown ベースのプラグインコンテナをサポートしているランタイムにおいては、プラグイン登録が主要なインストール手段となります。登録が完了したランタイムは、`gal refresh` によってレンダリングされた `~/.gal/plugins/gal/` の正規ルートから直接定義を読み込みます。Markdown プラグインをネイティブサポートしていないランタイム（opencode など）については、ファイルプロジェクションによるフォールバックを利用します。

プラグインアセットとローカルプロジェクションの間でコマンドやエージェントの表示が重複するのを防ぐため、`~/.gal/config/config.json` で対応する `pluginMode.<runtime>` キーを有効化し、`gal refresh` を実行してください。

### Claude Code

Claude Code は、ディレクトリベースのスキルプラグインとして正規ルートを読み込みます。

1. `~/.gal/plugins/gal` を参照するディレクトジャンクションまたはシンボリックリンクを `~/.claude/skills/gal` に作成します。
   - Windows（PowerShell）:
     ```powershell
     New-Item -ItemType Junction -Path "$env:USERPROFILE\.claude\skills\gal" -Target "$env:USERPROFILE\.gal\plugins\gal"
     ```
   - macOS / Linux:
     ```bash
     ln -s ~/.gal/plugins/gal ~/.claude/skills/gal
     ```
2. `claude plugin list` を実行し、`gal@skills-dir` が読み込まれていることを確認します。Claude Code はローカルキャッシュを介さず、このジャンクションをライブで直接参照します。
3. `~/.gal/config/config.json` で Claude のプラグインモードを有効化します。
   ```json
   {
     "pluginMode": {
       "claude": true
     }
   }
   ```
4. `gal refresh` を実行します。`pluginMode.claude` が `true` に設定されていると、GAL は `~/.claude/commands/*.md` へのコマンドの投影を停止し、既存のコピーを削除してスキルの重複表示を防止します。

マーケットプレイスのスナップショット経由でのインストール（`claude plugin add`）は補助的な代替手段であり、アセットを `~/.claude/plugins/cache/<marketplace>/gal/<version>/` にコピーして利用します。

### OpenAI Codex

Codex は、ローカルプラグインマーケットプレイスディレクトリを通じて GAL を読み込みます。

1. ローカルマーケットプレイスディレクトリを登録します。
   ```bash
   codex plugin marketplace add ~/.gal/plugins
   ```
   これにより、`gal refresh` によって生成された `~/.gal/plugins/.agents/plugins/marketplace.json` が読み込まれます。
2. プラグインを追加します。
   ```bash
   codex plugin add gal@gal
   ```
   Codex はプラグインファイルを `~/.codex/plugins/cache/gal/gal/<version>/` へコピーします。
3. `codex plugin list` を実行し、`gal@gal` が表示されることを確認します。
4. `~/.gal/config/config.json` で Codex のプラグインモードを有効化します。
   ```json
   {
     "pluginMode": {
       "codex": true
     }
   }
   ```
5. `gal refresh` を実行します。

**共有スキルディレクトリに関する注意事項**: Codex と opencode は `~/.agents/skills` パスを共有します。対象マシンで opencode を使用しない場合は、`pluginMode.codex: true` を設定することで `~/.agents/skills` へのスキル投影が停止され、既存ファイルが削除されます。両方のランタイムを同時に使用する場合は、opencode をサポートするために GAL は `~/.agents/skills` へのスキル投影を継続します。その結果、Codex 上ではプラグインエントリと投影エントリの双方が表示されます。

### GitHub Copilot

GitHub Copilot は、Agent Plugins 1.0.0 パッケージとして GAL を読み込みます。

1. Copilot の設定で `~/.gal/plugins` をディレクトリマーケットプレイスとして登録します（これは `~/.gal/plugins/.claude-plugin/marketplace.json` を参照します）。その後 `gal` を有効化します。Copilot はファイルをコピーせず、インプレースで読み込みます。エージェントペルソナは `com.github.copilot/agents/` から、ワークスペースルールは `com.github.copilot/rules/` から、MCP ツールは `mcp.json` からそれぞれ読み込まれます。
2. `copilot plugin list` を実行し、`gal` が有効になっていることを確認します。
3. `~/.gal/config/config.json` で Copilot のプラグインモードを有効化します。
   ```json
   {
     "pluginMode": {
       "copilot": true
     }
   }
   ```
4. `gal refresh` を実行します。`pluginMode.copilot` が `true` の場合、GAL は `~/.copilot/agents/*.agent.md` へのエージェント投影を停止し、古いコピーを削除します。なお、Copilot はプラグインの `commands/` ディレクトリからコマンドを解釈しないため、Copilot 向けのコマンドスキルは引き続き `~/.copilot/skills/` に保持されます。

### Google Antigravity

Antigravity は、ローカルディレクトリ探索を通じて GAL を登録します。

1. 正規ルートから直接プラグインをインストールします。
   ```bash
   agy plugin install ~/.gal/plugins/gal
   ```
   これにより、正規ルートを指すジャンクションが `~/.gemini/antigravity-cli/plugins/gal` に作成されます。
2. 重複した `claude-code` インポートが存在する場合は整理します。以前に Antigravity が Claude Code からプラグインをインポートしていた場合、`agy plugin list` に重複した `gal` エントリ（`local-install` と `claude-code`）が表示されることがあります。`~/.gemini/config/import_manifest.json` を開き、`imports` 配列から `claude-code` のエントリを削除して `local-install` のみを残してください。その後 `agy plugin list` で有効なエントリが 1 つだけであることを確認します。
3. `~/.gal/config/config.json` で Antigravity のプラグインモードを有効化します。
   ```json
   {
     "pluginMode": {
       "agy": true
     }
   }
   ```
4. `gal refresh` を実行します。`pluginMode.agy` が `true` に設定されると、GAL は `~/.gemini/antigravity-cli/skills/` へのコマンドスキルの投影を停止し、既存ファイルを削除します。

### opencode（プロジェクションフォールバック）

opencode は Markdown ベースのプラグインマニフェストに対応していません。その拡張モデルは、npm またはローカルディレクトリからインストールされる JavaScript および TypeScript モジュールを前提としています。そのため、opencode では GAL のファイルプロジェクションによるフォールバックを利用します。`gal refresh` を実行すると、Markdown コマンドが `~/.config/opencode/commands/` へ、エージェントが `~/.config/opencode/agents/` へ、コアスキルが `~/.agents/skills/` へそれぞれ投影されます。

### 対話的なインストール手順

Claude Code や Codex のマーケットプレイススナップショットを通じて GAL を試すユーザーは、チャット上で対話的に初期インストールを完了できます。

1. Claude Code または Codex のマーケットプレイスから「gal」で検索して **GAL プラグイン** をインストールするか、リポジトリのスナップショットブランチから読み込みます。
2. チャット内でアシスタントに対し、**「help me install gal」**（または「gal をインストールして」）と指示します。組み込みの `install-gal` スキルが実行確認を求めた後、適切なパッケージマネージャー（Homebrew、WinGet、または `cargo install --git`）を呼び出してバイナリをインストールします。インストール後、新規リポジトリでは `gal init` が、既存リポジトリでは `gal render-adapters` が実行されます。

マーケットプレイスのプラグイン単体では、完全な動作環境は整いません。各種ワークフローコマンドは、ネイティブの `gal` バイナリの存在に依存しています。バイナリの導入が完了した後は、前述の手順に従って各ランタイムのプラグイン登録を行ってください。

## リポジトリの初期化とアダプター保守

### リポジトリの初期化（`gal init`）

新しいリポジトリにおいて、`gal init` は `gal-core` のテンプレートからアダプターの根本ファイル（`AGENTS.md` および `CLAUDE.md`）を作成し、`.dev/project.md` と `.dev/state.md` を含む `.dev/` ディレクトリを初期化します。リポジトリがすでに初期化済み、または中途半端に初期化されている場合、`gal init` はファイルを書き換えることなく直ちに中止し、終了コード `1` で終了します。

| リポジトリの状態 | `.dev/project.md` | `.dev/state.md` | `gal init` の動作 | 終了コード |
| --- | --- | --- | --- | --- |
| 未初期化 | なし | なし | テンプレートから両方の状態ファイルを作成し、アダプター根本ファイルをレンダリングします。 | 0 |
| 初期化済み | あり | あり | `.dev/project.md` が存在するため中止。`gal render-adapters` の実行を案内します。 | 1 |
| 一部初期化 | あり | なし | `.dev/state.md` が欠落しているため中止。Git からの復元を案内します。 | 1 |
| 一部初期化 | なし | あり | `.dev/project.md` が欠落しているため中止。Git からの復元を案内します。 | 1 |

エラーメッセージには、次にとるべき具体的な復旧手順が示されます。

```text
gal init: this repository is already initialized (.dev/project.md exists).
  To regenerate AGENTS.md and CLAUDE.md from .dev/project.md, run: gal render-adapters
  To start over from the templates, delete .dev/project.md and .dev/state.md, then run gal init again.
```

```text
gal init: this repository is half-initialized: .dev/project.md exists but .dev/state.md is missing.
  Restore .dev/state.md from version control, or delete .dev/project.md and run gal init again.
  gal init does not overwrite .dev/project.md.
```

### アダプターの再生成（`gal render-adapters`）

初期化済みのリポジトリにおいて、`gal render-adapters` は `.dev/project.md` を唯一の情報源としてアダプターファイルを更新します。

| リポジトリの状態 | `.dev/project.md` | `gal render-adapters` の動作 | 終了コード |
| --- | --- | --- | --- |
| 初期化済み | あり | `AGENTS.md`、`CLAUDE.md`、および条件付きレイヤーを再生成し、廃止された古い根本ファイルを整理します。 | 0 |
| 未初期化 | なし | 中止: `gal render-adapters: .dev/project.md not found. Run gal init first.` | 1 |

なお、`gal render-adapters` コマンドが `.dev/state.md` を参照することはありません。

### アダプターのクリーンアップと廃止ファイルの整理

既存リポジトリの更新時、または以前のバージョンから移行する際、`gal render-adapters` は非推奨となった古い成果物を整理します。

- **廃止された根本ファイルの削除**: 過去のブリッジファイル（`GEMINI.md`、`.github/copilot-instructions.md`、`.agents/rules/gal.md`）は廃止されました。アダプターの更新時、GAL はこれらのパスを検査します。ファイルの先頭行に GAL の生成マーカーが含まれている場合、そのファイルは削除され `pruned (GAL-owned)` として報告されます。ファイルの削除によって `.agents/rules/` や `.github/` が空になった場合、その親ディレクトリも削除されます。
- **ユーザー独自ファイルの保護**: GAL の生成マーカーを持たないファイルは、ユーザー独自のファイルとして扱われます。GAL はこれらに触れることなく `kept (hand-owned)` として安全に保持します。
- **条件付きレイヤーの同期**: `.dev/project.md` で Rust 規約が有効化されている場合、メインアダプターと並行して条件付きルールファイル（`.claude/rules/gal-rust.md` および `.github/instructions/gal-rust.instructions.md`）が更新されます。Rust 規約が無効化されている場合、残存している古い GAL 管理のルールファイルは自動削除されます。

### プロジェクト仕様書（`.dev/project.md`）

`.dev/project.md` ファイルは、アダプター生成に使用される中核のプロジェクトプロファイルを定義します。このファイルには、**必ず以下の 8 つの H2 見出しがそれぞれ 1 回ずつ含まれている必要があります**: `What This Is`、`Tech Stack`、`Architecture`、`Constraints`、`Response Style`、`Freshness`、`Project Language`、`Protected Paths`。

検証は安全側に倒して厳格に実施されます（fail closed）。`gal init` がテンプレートからこのファイルを生成するため、検証によって手動編集による構造破壊を防止します。見出しの欠落や重複が存在する場合、`gal render-adapters` は処理を中断し、問題のある見出しを報告してファイル書き込みを一切行いません。エラーは 1 回の実行につき 1 件のみ報告されます。複数の見出しに不備がある場合は、`plugins/gal-core/templates/project.md` と見比べながら修正した上で再実行してください。

ファイルサイズ制限（`PROJECT_MD_MAX_BYTES`）も厳格に適用されます。ファイルサイズが上限バイト数を超過した場合は、内容を整理してサイズを収める必要があります。ジェネレーターがファイルサイズ制限内に収めるために不完全なアダプターファイルを部分出力することは決してありません。

`Tech Stack` 表の直下に、`.dev/project.md` は権威ある検証チェックブロックを含みます。

```markdown
<!-- gal:authoritative-check -->
```json
{
  "command": [
    "cargo test --workspace",
    "cargo clippy --workspace",
    "cargo fmt --check"
  ]
}
```
```

この配列に定義されたコマンド群は、シェルの変数展開を行わず、記述された順序で直列に実行されます。起動されたプロセスは、リポジトリルートへのディレクトリ移動を行わず、呼び出し元の現在の作業ディレクトリで実行されます。テストスイートを持たないドキュメント専用リポジトリなどの場合は、配列を `["true"]` と設定してください。パイプライン検証時には、`gal finalize-check` ゲートがこれらのコマンドを実行します。

## 診断の実行（`gal doctor`）

バイナリ、正規ルート、ランタイム設定、およびマシン設定全般の健全性を検証するには、`gal doctor` を実行します。このチェックは、初回セットアップ後、アップグレード後、またはエージェントがコマンドを認識しなくなった際に実行してください。

主な診断チェック項目は以下のとおりです。

- **opencode プロジェクションの同期状態**: `~/.config/opencode/` 内で正規ルートと内容が乖離しているコマンドやエージェントのファイルを検出します。古いファイルには警告が出力され、欠落しているファイルにはエラーが出力されます。不整合を解消するには `gal refresh` を実行します。
- **Claude スキルのジャンクション状態**: `~/.claude/skills/gal` が存在しない場合に警告を発し、手動作成手順を案内します。なお、`gal refresh` がこのリンクを自動作成することはありません。
- **アダプターサイズの分析**: 初期化済みリポジトリにおいて、`AGENTS.md` および `CLAUDE.md` のバイトサイズをレポートします。推奨サイズを超過しているファイルに対しては、処理をブロックしない参考情報として `[WARNING]` 通知が出力されます。

`gal doctor` を単体で実行しても、ヘッドレスコーディング CLI が起動することはありません。ヘッドレス実行の動作確認を行いたい場合は、`gal doctor --executor-smoke` を使用します。これにより、ヘッドレスディスパッチパイプラインを通じてモックタスクが実行され、`codex`、`claude`、`copilot`、`agy`、`opencode` における CLI の導入状態、ログイン状態、レシート生成がテストされます。リモート実行レーンをテストする場合は `--transport ssh` を、CI 環境向けには `--json --strict` をそれぞれ付与してください。ルーティングやモデル設定の詳細は [設定](configuration.ja.md#エグゼキュータールーティングexecutorrouting) を、実行ワークフローの詳細は [ワークフロー](workflows.ja.md#タスク実行サイクルと各ロール) を参照してください。

## ランタイムの呼び出し構文とキャッシュ挙動

### ランタイム別コマンド構文

各ランタイムによって機能の登録方式が異なるため、呼び出し構文もツールごとに異なります。

| ランタイム | コマンド構文 | 備考 |
| --- | --- | --- |
| Claude Code | `/gal status` | ネイティブのプラグインコマンド。 |
| OpenAI Codex | `/gal-status` | スキルとして登録。プレフィックス `$`（`$gal-status`）や `/skills` 構文も利用可能。 |
| GitHub Copilot | `/gal-status` | スキルとして登録され、`/skills list` から確認可能。 |
| Google Antigravity | `/gal-status` | エージェントスキルとして登録。 |
| opencode | `/gal-status` | ネイティブの Markdown コマンド。 |

構文の違いにご注意ください。Claude Code ではスペース区切りのコマンド（`/gal status`）を使用しますが、他のランタイムではハイフン区切りのスキル名（`/gal-status` または `$gal-status`）を使用します。

### 更新の反映遅延とキャッシュ挙動

コマンドの変更が実際に反映されるまでの時間は、各ランタイムのキャッシュ挙動によって異なります。

- **Claude Code**: `gal@skills-dir` ジャンクションを通じて正規ファイルを動的に読み込みます。変更は次回の会話ターンで即座に反映されます。なお、マーケットプレイス形式でインストールしている場合はクライアントの再起動が必要です。
- **Google Antigravity**: プロセスの起動時にコマンドを読み込みます。変更を反映させるには `agy` を再起動してください。
- **GitHub Copilot および opencode**: 新しい会話セッションを開始するタイミングで、コマンドおよびスキルの定義を読み込みます。
- **OpenAI Codex**: アクティブなスレッド内でスキルの更新を自動検知します。変更が反映されない場合は、Codex を再起動するか新しい会話を開始してください。

**非推奨となったランタイム**: GAL は Gemini CLI へのコマンドやスキルの投影を行いません。過去に Gemini CLI を使用していたユーザーは Google Antigravity へ移行してください。Antigravity では `~/.gemini/antigravity-cli/skills/<name>/SKILL.md` にスキルがマウントされます。

Codex ランタイムに関する留意事項:

- **コンテキスト予算による短縮・省略**: Codex は初期スキル一覧のサイズに上限を設けています。コンテキスト予算が逼迫すると、説明文が切り詰められたり、オートコンプリートメニューからスキルが省略されたりすることがあります。メニューから省略されたスキルであっても、`$skill-name` と直接入力することで確実に実行可能です。
- **重複したスキル登録**: 複数のツールが同一の名前でスキルを登録した場合、Codex は選択メニューに双方のエントリを表示します。
