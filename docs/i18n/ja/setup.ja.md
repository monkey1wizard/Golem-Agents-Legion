---
source: docs/setup.md
lang: ja
source_commit: 11281aff62c1a3761fd3da35c420e6ec92374054
translated_at: 2026-10-21
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

マシンに GAL をセットアップするには、次の 4 つの手順を実行します。

1. 以下のいずれかの方法で `gal` バイナリをインストールします。
2. `gal refresh` を一度実行します。このコマンドは `~/.gal/plugins/gal/` に正規ルート (canonical root) をレンダリングし、GAL のファイルをコーディングエージェント (coding agent) へ投影 (projection) します。すべてのランタイム (runtime) はこのルートまたはその投影から GAL を読み込むため、ランタイムが GAL を使用できるようになる前に、この手順を実行する必要があります。
3. [ランタイム別プラグイン登録](#ランタイム別プラグイン登録) の説明に従い、使用する各ランタイムにプラグインを登録します。その後、GAL が登録を検出できるように `gal refresh` を再度実行します。
4. GAL を使用する各リポジトリで `gal init` を実行します。[リポジトリの初期化](#リポジトリの初期化gal-init) を参照してください。

Windows では、PowerShell が `gal` という名前だけの呼び出しを組み込みエイリアスとして解決します。[PowerShell のエイリアスに関する注意](#powershell-のエイリアス競合) の説明に従い、Windows では `gal.exe` を使用してください。

### インストール方法

#### ソースコードからのビルド（`cargo install --git`）

```bash
cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli --tag v0.3.2 --locked
```

`v0.3.2` は、GitHub Releases から選んだ目的のリリースタグに置き換えてください。リリースタグには先頭の `v` が付きます。`--tag` を指定しない場合、Cargo は公開リポジトリの現在の `main` ブランチをビルドします。`--locked` フラグを指定すると、リポジトリで追跡されている `Cargo.lock` の依存関係バージョンでビルドします。

GAL は crates.io には公開されていないため、`--git` フラグの指定が必須です。crates.io にある同名のクレート `gal-cli` は、無関係のツールです。`--git` を付けずに `cargo install gal-cli` を実行すると、GAL ではなくそのツールがインストールされます。GAL の Cargo ワークスペースでは `gal-cli` がパッケージ名であり、インストールされる実行ファイルの名前は `gal` です。

#### Homebrew（macOS および Linux）

```bash
brew install monkey1wizard/tap/gal
```

macOS および Linux ユーザー向けとして推奨される、パッケージマネージャー経由のインストール方法です。

#### WinGet（Windows）

```powershell
winget install Monkey1Wizard.GAL
```

コミュニティパッケージインデックスの更新は、リリースより遅れる場合があります。インストール前に `winget show Monkey1Wizard.GAL` を実行し、利用可能なバージョンを確認してください。

#### バイナリの直接ダウンロード

各 GitHub リリースでは、次のアセットが公開されます。

- ターゲットごとのアーカイブ。名前は `gal-<tag>-<platform>-<arch>.tar.gz` です。Windows のアーカイブでは代わりに `.zip` を使用します。各アーカイブには `gal` バイナリと GAL のソースアセットが含まれます。
- Windows のアーキテクチャごとの単体バイナリ。名前は `gal-<tag>-windows-<arch>.exe` です。
- `checksums.txt`。各アーカイブの SHA-256 ハッシュを記載します。
- `checksums.txt.sig` と `checksums.txt.pem`。`checksums.txt` の cosign 署名と証明書です。
- `artifact-manifest.json`。リリース成果物のマニフェストです。
- `install.sh` と `install.ps1`。インストールスクリプトです。

各リリースは 6 つのターゲットをビルドします。Windows、macOS、Linux のそれぞれについて、`x64` と `arm64` を提供します。`<tag>` には先頭の `v` が含まれます。たとえば、v0.3.2 の x64 Windows アーカイブは `gal-v0.3.2-windows-x64.zip` です。プラットフォーム名は `windows`、`darwin`、`linux` です。アーキテクチャ名は `x64` と `arm64` です。

アーカイブを展開し、`gal` バイナリを `PATH` 上に配置してください。Windows ではバイナリ名は `gal.exe` です。macOS と Linux では、展開したバイナリに実行権限がない場合に `chmod +x gal` を実行してください。ソースアセットがバイナリと同じ場所にない場合、`gal refresh` はバイナリに埋め込まれたソースアセットのコピーを使用します。[ソースルートの探索順序](#ソースルートの探索順序) を参照してください。

#### シェルスクリプト（Linux および macOS）

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
```

このスクリプトは、ダウンロードしたアーカイブを `checksums.txt` の SHA-256 ハッシュと照合します。ハッシュが一致しない場合、またはエントリが存在しない場合は、インストールを中止します。`cosign` が `PATH` 上にある場合、スクリプトはファイルを展開する前に、キーレス cosign で `checksums.txt` の署名も検証します。cosign の検証に失敗した場合は、どのような失敗であってもインストールを中止します。`cosign` がインストールされていない場合、または署名ファイルをダウンロードできない場合、スクリプトは警告を表示し、SHA-256 検証のみで処理を続行します。

スクリプトは、バイナリを `~/.local/bin` に、ソースアセットを `~/.local/share/gal` にインストールします。新しいソースアセットをコピーする前に、`~/.local/share/gal` ディレクトリ全体を削除します。そのため、このディレクトリに他のファイルを置かないでください。`~/.local/bin/gal` は上書きされます。特定のリリースをインストールするには、先頭の `v` を含むそのタグを `GAL_VERSION` に設定します。スクリプトはこの値をダウンロード URL のリリースタグとして使用するため、`v` のない `0.3.2` ではダウンロードに失敗します。

```bash
curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | GAL_VERSION=v0.3.2 bash
```

#### PowerShell スクリプト（Windows）

```powershell
irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
```

Windows 用スクリプトはベータ版です。クリーンなマシンでの検証はまだ行われておらず、実行時にベータ版である旨の警告を表示します。シェルスクリプトと同じ SHA-256 と cosign の検証を実行し、同じ検証失敗で処理を中止します。

スクリプトは、`gal.exe` とソースアセットを `%LOCALAPPDATA%\Programs\gal` にインストールします。新しいファイルをコピーする前に、そのディレクトリにすでにあるものをすべて削除します。このディレクトリに他のファイルを置かないでください。スクリプトは `PATH` を変更しません。`%LOCALAPPDATA%\Programs\gal` をご自身でユーザー `PATH` に追加してください。特定のリリースをインストールするには、先頭の `v` を含むそのタグを `$env:GAL_VERSION` に設定します。たとえば `$env:GAL_VERSION = 'v0.3.2'` のように指定します。

#### PowerShell のエイリアス競合

PowerShell には、`Get-Alias` に対応する `gal` という組み込みエイリアスがあります。スクリプトを含むすべての PowerShell セッションで、`gal` という名前だけで呼び出すと、GAL バイナリではなく `Get-Alias` が実行されます。`gal.exe` を明示的に呼び出すか、バイナリの完全パスを使用するか、PowerShell の `$PROFILE` でエイリアスを削除してください。

```powershell
Remove-Item Alias:gal -Force -ErrorAction SilentlyContinue
```

### ソースルートの探索順序

`gal refresh` には、`skills/`、`agents/`、`commands/` を含む GAL のソースルートが必要です。次の順序で探索します。

1. `gal` バイナリを含むディレクトリ。最初にフラットな配置（バイナリと並んだソース）を確認します。次に、同じプレフィックス配下の `bin/gal` に対する `share/gal` を確認します。その後、バイナリのディレクトリから上位へたどった各ディレクトリを確認します。
2. バイナリに埋め込まれたソースペイロード。GAL はこのコピーを `~/.gal/embedded-src` に書き出して使用します。

`gal dev-apply` は、GAL のソースチェックアウトを展開します。次のオプションを使用できます。

- `--source <path>` はソースルートを指定します。省略した場合は、現在のディレクトリから見つかったチェックアウトを使用します。チェックアウトが見つからない場合、コマンドはエラーで終了します。
- `--root <path>` は書き込み先を既存のルートディレクトリに限定します。`--source` の指定が必要です。

`gal dev-apply` は、GAL のソースチェックアウトに対してのみ使用してください。マシンのセットアップには `gal refresh` を使用してください。

### アップグレードとメンテナンス

GAL は自身を更新しません。`gal update` は、インストール済みのバージョンと、Cargo、WinGet、Homebrew の 3 種類のアップグレードコマンドを表示します。インストールに使用したのと同じ方法でアップグレードしてください。

| インストール方法 | アップグレード |
| --- | --- |
| Cargo | 同じ `cargo install --git ... gal-cli --tag <new tag> --locked` コマンドを新しいタグで実行します。Cargo がインストール済みのバイナリを置き換えます。同じリビジョンを再インストールするには `--force` を追加します。 |
| Homebrew | `brew upgrade gal` |
| WinGet | `winget upgrade Monkey1Wizard.GAL` |
| シェルまたは PowerShell スクリプト | インストールスクリプトを再度実行します。 |
| 直接ダウンロード | 新しいリリースアセットをダウンロードし、`PATH` 上のバイナリを置き換えます。 |

アップグレードのたびに、次の手順を実行してください。

1. `gal refresh` を実行して、新しい正規ルートと投影をレンダリングします。
2. 初期化済みの各リポジトリで `gal render-adapters` を実行し、そのリポジトリのアダプター (adapter) ファイルを新しいバージョンに合わせて更新します。
3. [更新の反映遅延とキャッシュ](#更新の反映遅延とキャッシュ) の説明に従い、GAL をキャッシュするランタイムを再読み込みします。

引数を付けずに `gal` を実行すると、使い方のヘルプが表示され、終了コード `0` で終了します。認識されないサブコマンドを指定すると、終了コード `64` で終了します。

## ランタイム別プラグイン登録

Markdown ベースのプラグインコンテナに対応したランタイムでは、プラグイン登録が GAL を接続する主な方法です。Claude Code、GitHub Copilot、Google Antigravity は、`gal refresh` がレンダリングする `~/.gal/plugins/gal/` の正規ルートからプラグインを直接読み込みます。OpenAI Codex は、インストール時にプラグインを自身のキャッシュへコピーします。opencode は Markdown プラグインに対応していないため、代わりにファイル投影を使用します。

プラグインを登録した後、`gal refresh` を実行してください。GAL はランタイムごとに、ローカルにある登録の証拠を探します。登録が確認できた場合、GAL はプラグインが置き換えるフォールバックファイルの投影を停止します。登録がない場合、GAL はフォールバックファイルを投影します。証拠が不確かな場合、GAL はフォールバックファイルを投影し、警告を表示します。

GAL は、`~/.gal/install-state.json` の `selectedRuntimes` に指定されたランタイムに対してのみ投影します。有効な名前は `claude`、`codex`、`copilot`、`antigravity`、`opencode` です。それ以外の名前は無視されます。ファイルが存在しない場合、または有効な名前がない場合は、5 つすべてのランタイムを選択します。選択されていないランタイムについては、GAL が管理する成果物を削除します。

個人用の MCP サーバーを追加するには、`~/.gal/local/mcp.json` の `servers` オブジェクトに記述します。GAL はそれを、レンダリングされた `mcp.json` にマージします。個人用サーバーがコアのサーバーと同じ名前の場合は、コアのサーバーが優先され、個人用のエントリは破棄されます。

### Claude Code

Claude Code は、正規ルートをディレクトリベースのスキルプラグインとして読み込みます。

1. `~/.gal/plugins/gal` を指すディレクトリジャンクションまたはシンボリックリンクを `~/.claude/skills/gal` に作成します。
   - Windows（PowerShell）:

     ```powershell
     New-Item -ItemType Junction -Path "$env:USERPROFILE\.claude\skills\gal" -Target "$env:USERPROFILE\.gal\plugins\gal"
     ```

   - macOS / Linux:

     ```bash
     ln -s ~/.gal/plugins/gal ~/.claude/skills/gal
     ```

2. 新しい Claude Code セッションを開始し、`claude plugin list` を実行して `gal@skills-dir` が読み込まれていることを確認します。Claude Code は、セッションの開始時に skills-dir プラグインを読み込みます。ファイルはキャッシュにコピーされず、ジャンクションを通じて読み込まれます。
3. `gal refresh` を実行します。GAL が Claude の登録を確認できた場合、フォールバックコマンドを `~/.claude/commands/` に投影しません。それ以外の場合は、フォールバックコマンドを投影します。

`gal refresh` は `~/.claude/skills/gal` のリンクを作成しません。リンクがない場合は `gal doctor` が警告します。

補助的な方法として、`gal refresh` が `~/.gal/plugins/.claude-plugin/marketplace.json` に書き出すローカルマーケットプレイスから GAL をインストールすることもできます。

```bash
claude plugin marketplace add ~/.gal/plugins
claude plugin install gal@gal
```

この方法では、プラグインが `~/.claude/plugins/cache/<marketplace>/gal/<version>/` にコピーされます。このコピーは `gal refresh` を実行しても変わりません。また、プラグインの更新を適用するには Claude Code の再起動が必要です。

### OpenAI Codex

Codex は、ローカルプラグインマーケットプレイスディレクトリを通じて GAL を読み込みます。

1. ローカルマーケットプレイスディレクトリを登録します。

   ```bash
   codex plugin marketplace add ~/.gal/plugins
   ```

   Codex は、`gal refresh` が生成する `~/.gal/plugins/.agents/plugins/marketplace.json` を読み込みます。
2. プラグインを追加します。

   ```bash
   codex plugin add gal@gal
   ```

   Codex はプラグインファイルを `~/.codex/plugins/cache/gal/gal/<version>/` にコピーします。`gal refresh` はこのコピーを更新しません。GAL をアップグレードした後は、`codex plugin remove gal@gal` でプラグインを削除し、再度追加して、改めて信頼済みとして設定してください。
3. `codex plugin list` を実行し、`gal@gal` が表示されることを確認します。
4. `gal refresh` を実行します。GAL は、Codex の TOML エージェントを `~/.codex/agents/` に、コマンドスキルを `~/.agents/skills/` に常に投影します。Codex の登録が確認でき、かつ opencode が選択されていない場合は、プラグインがコアスキルを提供し、GAL はコアスキルの投影を停止します。

Codex パイプラインフックは、Codex プラグインマニフェストの `.codex-plugin/plugin.json#hooks` に含まれます。GAL はルートのデフォルトフックや Claude Code マニフェスト内のフックをインストールしません。更新後、Codex プラグインマニフェストを確認し、ガード付き継続を利用する前に Codex で GAL プラグインを信頼済みとして設定してください。信頼状態とフックの実行はランタイム上の事実です。`gal doctor` は静的なパッケージ状態とコマンド準備状態を報告できますが、信頼されたフックを Codex が実行したことを証明できるのは、ライブの同一セッションハンドシェイクだけです。

**共有スキルディレクトリに関する注意事項**: Codex と opencode は `~/.agents/skills` を共有します。opencode が選択されている場合、Codex の登録が確認済みであっても、GAL はこの共有インターフェースへの投影を維持します。Codex の登録によって、opencode が必要とするファイルが削除されることはありません。

### GitHub Copilot

GitHub Copilot は、Agent Plugins 1.0.0 パッケージとして GAL を読み込みます。

1. Copilot の設定で `~/.gal/plugins` をディレクトリマーケットプレイスとして登録し、そのマーケットプレイスから `gal` プラグインを有効化します。Copilot はプラグインファイルをコピーせず、その場で読み込みます。エージェントペルソナは `com.github.copilot/agents/` から、ワークスペースルールは `com.github.copilot/rules/` から読み込まれます。GAL は `mcp.json` ファイルも同梱していますが、そのサーバー一覧は空です。GAL は MCP server を提供しません。
2. `copilot plugin list` を実行し、`gal` プラグインが有効になっていることを確認します。
3. `gal refresh` を実行します。Copilot の登録が確認できた場合は、プラグインがエージェントを提供し、GAL はエージェントの投影を停止します。Copilot はプラグインの `commands/` ディレクトリからコマンドを読み込まないため、GAL は引き続きコマンドスキルを `~/.copilot/skills/` に投影します。また、GAL は `~/.copilot/gal` を `~/.gal` へのディレクトリリンクとして維持します。`~/.copilot/gal` 配下に書き込まれたファイルは、`~/.gal` に書き込まれます。

### Google Antigravity

Antigravity は、ローカルディレクトリ探索を通じて GAL を登録します。

1. 正規ルートからプラグインを直接インストールします。

   ```bash
   agy plugin install ~/.gal/plugins/gal
   ```

2. `agy plugin list` を実行し、`gal` がちょうど 1 回だけ表示されることを確認します。Antigravity が以前に Claude Code からプラグインをインポートしていた場合、一覧に `local-install` と `claude-code` に由来する重複した `gal` エントリが表示されることがあります。その場合は `~/.gemini/config/import_manifest.json` を開き、`imports` 配列から `claude-code` のエントリを削除して `local-install` のみを残し、再度 `agy plugin list` を実行してください。
3. `gal refresh` を実行します。`~/.gemini/antigravity-cli/plugins/gal` が、`.claude-plugin/plugin.json` の `name` が `gal` であるディレクトリに解決される場合、GAL は Antigravity を登録済みとして扱います。マニフェストを読み取れない場合や名前が異なる場合、GAL は登録を不確かなものとして扱い、コマンドスキルを投影して警告を表示します。登録が確認できた場合は、プラグインがコマンドスキルを提供し、GAL は `~/.gemini/antigravity-cli/skills/` へのコマンドスキルの投影を停止します。Antigravity が登録済みかどうかにかかわらず、GAL は `~/.gemini/antigravity-cli/gal` のリンクが `~/.gal/source` を指す状態を維持します。

`~/.gemini/antigravity-cli/plugins/gal` が存在するのに `agy plugin list` に `gal` が表示されない場合でも、GAL は Antigravity を登録済みとして扱い、コマンドスキルの投影を省略します。その結果、Antigravity では GAL のコマンドを使用できなくなります。`agy plugin list` に `gal` が表示されるまで、`agy plugin install` でプラグインをインストールしてください。

### opencode（投影フォールバック）

opencode は Markdown ベースのプラグインマニフェストに対応していません。その拡張モデルは、npm またはローカルディレクトリからインストールされる JavaScript および TypeScript モジュールに依存しています。そのため、opencode では GAL のファイル投影によるフォールバックを使用します。`gal refresh` を実行すると、Markdown コマンドが `~/.config/opencode/commands/` に、エージェントが `~/.config/opencode/agents/` に、コアスキルが `~/.agents/skills/` に投影されます。

### 非推奨のランタイム

GAL は Gemini CLI へのコマンドやスキルの投影を行いません。以前に Gemini CLI を使用していたユーザーは、Google Antigravity へ移行してください。

### 対話的なインストール

Claude Code または Codex で GAL プラグインがすでに読み込まれているのに `gal` バイナリがない場合は、アシスタントにバイナリのインストールを依頼できます。チャットで **「help me install gal」** と伝えてください。すると、組み込みの `install-gal` スキルが次のように動作します。

1. `gal --version` がすでに動作するかどうかを確認します。
2. オペレーティングシステムに合ったパッケージマネージャーとして、Homebrew、WinGet、または `cargo install --git` のいずれかを選択します。
3. 実行する正確なインストールコマンドを表示し、コマンドを実行する前にユーザーの確認を待ちます。
4. `gal --version` で結果を確認します。Cargo でインストールした場合は、`gal refresh` も実行します。
5. 残りの手順を案内します。残りの手順は、`gal refresh` の実行と、その後の新しいリポジトリでの `gal init`、または初期化済みリポジトリでの `gal render-adapters` の実行です。

プラグイン単体では、動作する環境は整いません。ワークフローコマンドはネイティブの `gal` バイナリに依存しています。バイナリをインストールした後、前述の手順に従って各ランタイムを登録してください。

## Codex パイプライン実行のセットアップ

本節の手順は、2026-10-08 に確認した Windows 上の Codex CLI 0.161.0 と Codex Desktop 26.1002.7124.0 を対象としています。異なるバージョンを使用している場合は、Codex の公式ドキュメントで最新の動作を確認してください。公式の設定項目名および権限境界については、[permission modes](https://learn.chatgpt.com/docs/permission-modes)、[agent approvals and security](https://learn.chatgpt.com/docs/agent-approvals-security)、および [configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) を参照してください。

### 既知の問題 Codex Windows サンドボックスの障害

Windows では現在、Codex のサンドボックスがコマンドを実行する前に失敗します。GAL パイプライン (pipeline) はサンドボックス内では実行できません。これは Codex の不具合であり、GAL やリポジトリの問題ではありません。GAL 側で修正することもできません。

**症状。** サンドボックス内で実行するコマンドはすべて失敗し、`helper_unknown_error: setup refresh had errors` が表示されます。続いてエージェントは、ファイルを読み取れない、またはシェルを起動できないと報告します。エグゼキューター (executor) 自体は起動していても、パイプラインのフェーズは Verdict `BLOCKED`、`no-receipt`、またはテスト 0 件で終了します。

**確認方法。** `%USERPROFILE%\.codex\.sandbox\sandbox.<date>.log` を開き、`setup refresh completed with errors` を検索してください。報告される原因は実行ごとに異なります。確認された原因には、使用中の Codex ランタイムファイル（os error 32）、deny ACE の失敗、write ACE の失敗があります。Codex の再起動、サンドボックスサービスの再起動、OS の再起動では解消しませんでした。マシンを直接確認するには、使い捨てのディレクトリで次のコマンドを実行します。

```powershell
codex exec --json -s workspace-write "Run the command Get-Location in PowerShell and report its output"
```

`command_execution` イベントが `exit_code` `-1` と `setup refresh had errors` を報告した場合、そのマシンは影響を受けています。

**GAL の対応。** GAL は、ディスパッチ (dispatch) するすべての Codex 子プロセスを `--dangerously-bypass-approvals-and-sandbox` 付きで起動します。そのため子プロセスは Codex のサンドボックス外で実行され、他のエグゼキューターが権限バイパスのフラグで得るフルアクセスと同じ信頼レベルになります。以前の GAL は子プロセスを `-s workspace-write` で起動していました。この不具合の下では、その設定によってすべての Codex ディスパッチが失敗していました。

**必要な対応。** GAL パイプラインを動かす外側の Codex セッションは、サンドボックスなしで実行してください。[必須の権限モード Full access](#必須の権限モード-full-access) を参照してください。**Approve for me**（auto-review）は回避策になりません。これは承認要求を誰がレビューするかを決めるだけであり、そのコマンドは障害の起きているサンドボックス内で引き続き実行されます。

**セキュリティへの影響。** サンドボックスがない場合、Codex はファイルシステム全体を読み書きでき、現在のアカウントで実行可能な任意のターミナルコマンドを実行できます。[権限バイパスに関する警告](workflows.ja.md#セキュリティ上の警告-権限のバイパス) がすべてのエグゼキューターに対してすでに求めているとおり、GAL パイプラインは、信頼できる開発マシンと信頼できるリポジトリでのみ実行してください。

**Codex が修正された後。** 上記のテストコマンドを再実行してください。その時点で、外側のセッションをサンドボックス内で実行する構成に戻せるようになります。GAL のリリースで別途案内するまで、GAL は Codex 子プロセスを引き続きサンドボックスなしで起動します。

### 共通の前提条件

1. GAL をインストールし、Codex CLI が完全にインストールされていることを確認します。ディスパッチを実行するシェルセッションで、以下を実行してください。

   ```powershell
   gal.exe --version
   codex --version
   codex doctor
   Get-Command gal.exe -All
   Get-Command codex -All
   ```

   他のシェルを使用している場合は、そのシェルのコマンド探索機能を用いて、解決された実行ファイルを確認してください。Windows 環境では、コマンド探索の失敗に対処する目的でシステムの `PATH` を変更しないでください。PowerShell が `gal` を組み込みエイリアスとして解決してしまう場合は、`gal.exe` を直接呼び出すか、完全パスを指定してください。

2. 別途 `codex login` を実行して認証を完了します。トークンファイルを直接表示したり確認したりしないでください。Codex だけでなく、予定されているタスクに設定されたすべてのエグゼキューターを確認してください。
3. 信頼できる対象ワークスペースを開きます。ネットワークアクセスは必要な宛先のみに制限してください。診断によって特定の書き込み先パスが必要であると判明した場合に限り、書き込み可能ディレクトリを追加してください。
4. `gal doctor` を実行し、使い捨てのテスト用ディレクトリで `gal doctor --executor-smoke --executor codex` を実行します。スモークテストはプロバイダーのクォータを消費することに注意してください。このテストは CLI の準備状態とレシートの処理を確認するものであり、完全なパイプライン実行、独立監査、または Desktop の権限モードの成功を保証するものではありません。
5. Codex の承認設定とは独立して、`executorRouting` および `timeoutSecs` を設定します。詳細は [設定](configuration.ja.md#エグゼキュータールーティングexecutorrouting) を参照してください。
6. 初期化は新しいリポジトリに対してのみ実行してください。複数の計画がアクティブな場合は、対象のプロンプトを明示的に指定してください。

### Codex のガード付き継続の準備

ガード付き `codex_stop_v1` プロファイルには、信頼された GAL Codex プラグインと、ライブの同一セッションフックハンドシェイクが必要です。モデル名、環境の推測、古いマーカー、または別ホストのマーカーがないことによって、このプロファイルが有効になることはありません。有効なグラントを伴わない通常の `gal pipeline <prompt>` 呼び出しは `legacy_interactive` のままです。フックが実行されない場合、ホストに依存しない通常の呼び出しは、Codex フックが存在しない、無効である、または信頼されていないことを検出できません。

フックは、明示的な実行プロンプト (execution prompt) のパス、対応する実行プロンプトへ解決できる `#file:` または `@` のソースプラン (source plan) 参照、および `.dev/state.md` にアクティブなプランが 1 件だけある場合のパスなし要求を受け付けます。アクティブプランの状態が欠落または曖昧な場合、パスが `.dev/plans/` の外にある場合、パス構文が安全でない場合、または要求に `from` や `stop-at` 修飾子が含まれる場合、フックは要求を拒否します。ガード付き継続はこれらの修飾子を黙って破棄しません。ガード付きコーディネーターが返す型付きのタスクおよびフェーズアクションを使用してください。

ガード付き継続を利用する前に、次の確認を完了してください。

1. Codex プラグインを登録した後、`gal refresh` を実行します。GAL は登録状態を自動検出します。
2. インストール済みの GAL Codex プラグインマニフェストに、GAL の `UserPromptSubmit`、`PreToolUse`、`Stop` フックが含まれていることを確認します。これらをルートの `hooks/hooks.json`、ルートの `plugin.json`、または Claude Code マニフェストに追加しないでください。
3. `gal doctor` を実行して静的なパッケージ状態とコマンド準備状態を確認します。Codex で GAL プラグインを信頼済みとして設定してください。静的診断では、ランタイムの信頼状態やフック実行を証明できません。
4. 信頼された対象ワークスペースで、曖昧さのないパイプライン要求を開始します。Codex が同一セッションの Stop カナリアを実行できるようにしてください。カナリアはブートストラップを確認し、グラントを含む正確な継続要求を返す必要があります。次の `UserPromptSubmit` は、ガード付き CLI エントリがグラントを消費する前に、同じセッションでその継続要求をバインドする必要があります。その後、ガード付きエントリは実装を開始する前に、新しいエントリラッチと意味的なタスクチェックポイントを実行します。
5. フックが存在しない、無効である、信頼されていない、またはカナリアが失敗した場合は、ガード付き継続を利用しないでください。明示的な `--require-codex-stop-v1` エントリは、グラントがないか無効な場合に `host-continuation-not-ready` を返して実装前に失敗する必要があります。通常の呼び出しは `legacy_interactive` のままです。ホストに依存しない CLI は、自身が Codex 上で実行されていることを推測できません。

ブートストラップには、時間と試行回数の固定上限があります。保留中のブートストラップは 120 秒後に期限切れとなり、グラントは発行から 30 秒後に期限切れとなります。Stop フックが 1 回のブートストラップで処理する Stop イベントは最大 3 件です。

ブートストラップが保留中の間、Codex の `PreToolUse` フックは対応するすべてのツール呼び出しを拒否します。Stop カナリアが一度限りのグラントを作成した後は、同じセッション内で、グラントを含む正確な最初のアクションだけを許可します。未対応の特殊ツールまたはホスト提供ツールの経路は、このガード付きブートストラップの保証の対象外であり、ブートストラップ継続には使用できません。ガード付きエントリがグラントを消費し、コーディネーターをセッションにバインドすると、このブートストラップ制限は終了します。その後、フックは通常のツール使用を許可します。この結果は、チェックポイントやフェーズ遷移を承認するものではありません。GAL のガード付きアクションは、引き続きコーディネーターのリビジョン、レシート、タスク品質チェックを適用します。

`Stop` フックはブートストラップ中にのみ動作します。カナリアを発行し、グラントを含む継続が実行されるまで最終応答をブロックします。保留中のブートストラップがない場合、`Stop` フックは応答をそのまま終了させます。パイプラインの状態は読み取らず、実行中のパイプラインが終端状態に達するまでパイプラインを開いたままにすることもありません。

ガード付き継続が停止した場合は、診断中もワークスペースとセッション識別子を変更しないでください。静的な準備状態を確認するには `gal doctor` を実行し、その後 `.dev/pipeline/<plan-scope>/` 内のコーディネーターと試行エビデンスを確認します。信頼、マニフェスト、またはハンドラーの問題を解決し、投影されたアセットが古い場合は `gal refresh` を実行してください。新しい信頼済みの同一セッションハンドシェイクを開始して、新しいグラントを取得します。グラントをコピー、手動作成、または再利用しないでください。識別情報またはエビデンスのチェックで競合が報告された場合は、記録された状態を保持し、再試行前に競合を解決してください。Codex のガード付き動作を使わずに継続するには、通常の `gal pipeline <prompt>` 経路と既存のホスト所有継続フローを使用します。

ディスパッチを実行するシェルのコンテキスト内に実行ファイルが存在しない場合、準備状態は **Missing** と報告されます。権限またはセキュリティポリシーによってコマンド探索が明示的にブロックされている場合は **Denied** と報告されます。結果が不完全または矛盾している場合は **Unknown** と報告されます。コマンド探索の失敗だけでは、Codex がアンインストールされているとは断定できません。Denied または Unknown の探索結果だけを根拠に Codex を再インストールしないでください。

### 必須の権限モード Full access

[Windows のサンドボックス障害](#既知の問題-codex-windows-サンドボックスの障害) が続いている間は、GAL パイプラインを実行するすべての外側の Codex セッションでこのモードを使用してください。本ガイドの以前の版では、RTK と GAL のコマンドルールを組み合わせた **Approve for me**、またはカスタムの `workspace-write` 設定を推奨していました。どちらの方法も障害の起きているサンドボックス内でコマンドを実行するため、現在はどちらでもパイプラインを実行できません。

1. Codex Desktop で、そのタスクの権限モードとして **Full access** を選択します。
2. Codex CLI を使う場合は、`--dangerously-bypass-approvals-and-sandbox` 付きでセッションを開始するか、`%USERPROFILE%\.codex\config.toml` でサンドボックスモードを設定します。先にファイルをバックアップしてください。関係のない設定は保持し、このキーはすべてのテーブルより前に置きます。

   ```toml
   sandbox_mode = "danger-full-access"
   ```

   `approvals_reviewer = "auto_review"` などの承認設定は残してもかまいません。ただし、それによってサンドボックスが有効に戻ることはありません。
3. Codex を再起動し、`config.toml` を再読み込みさせます。
4. パイプラインを実行する前に、実行環境を確認します。

   ```powershell
   gal.exe --version
   codex login status
   gal.exe doctor --executor-smoke --executor codex --timeout 90
   ```

   doctor は Codex を起動し、TLS、サンドボックス、承認のエラーなしにプロバイダーの最終結果まで到達する必要があります。レシートの検証は別のチェックです。doctor が `NO_RECEIPT` などのレシートエラーを報告した場合は、生成されたレシートと試行ログを確認してください。

GAL が Codex の `config.toml` にある承認設定やサンドボックス設定を変更することはありません。外側のセッションのモードはユーザーが決定します。ただし GAL は、`~/.codex/agents/<name>.toml` に投影する各 Codex エージェントファイルには `sandbox_mode` の値を書き込みます。外側のモードにかかわらず、ディスパッチされた Codex 子プロセスはサンドボックスなしで実行されます。組織が管理する制限は、引き続き外側のセッションに適用されます。詳細については、OpenAI 公式ドキュメントの[サンドボックスと承認の動作](https://developers.openai.com/codex/sandboxing)を参照してください。

この設定で対応できない環境固有のケースや関連する問題がある場合は、[issue を作成](https://github.com/monkey1wizard/Golem-Agents-Legion/issues/new)してください。

### スモークテスト、パイプライン確認、ロールバック

設定のテストには、使い捨てのリポジトリと、単一ファイルを作成する最小限の計画を使用してください。対象の Codex スモークテストを実行し、続いて実装、テスト、独立監査、および中断フェーズからの再開を網羅するパイプラインを実行します。試行ログ、実行レシート、および最終ゲートの証拠を検査してください。各段階はそれぞれ異なる証拠を提供します。コマンド探索の成功はバイナリの利用可能性を確認し、プロセス起動は実行が開始されたことを確認し、プロバイダーの応答は API 通信を検証します。有効な実行レシートと合格したゲート検査は、それぞれ独立して評価されます。

アクセスが拒否された場合は、パイプラインの復旧地点を保持し、有効な権限モードでサポートされている場合に限り、その特定の範囲を限定した操作に対してのみ Codex のネイティブ承認を要求してください。失敗の原因が不明な場合は、裏付けとなるデータなしに上流モデルや認証の欠陥と決めつけず、診断ログを保持した上で、観測された段階を直接調査してください。TLS 証明書の検証を無効化したり、ユーザーのホームディレクトリ全体に権限を付与したり、実行中の試行の途中で実行ルーティングを変更したりしてはなりません。

設定変更をロールバックする場合は、バックアップから変更した設定のみを復元し、Codex Desktop で以前のモードを再選択した上で、有効なタスクコンテキストを再確認してください。設定ファイル全体を置き換えたり、ユーザーの資格情報を削除したりしないでください。

### 開発者受け入れチェックリスト

各結果は、自動化されたパイプライン出力や finalize ゲートとは独立して記録してください。開発者自身が実施するまで、手動確認やライブ環境での確認項目は **NOT RUN** のまま残してください。

- [ ] Codex Desktop および CLI のバージョンと、選択した権限モードを記録する。
- [ ] 資格情報データやトークンファイルを露出させることなく、ユーザーセッションのログインを確認する。
- [ ] ルーティングされた各エグゼキューターを確認し、使い捨てディレクトリでエグゼキュータースモークテストを実行する。
- [ ] 最小限のパイプラインを実行し、実装、テスト、独立監査、および再開の証拠を検査する。
- [ ] ご利用の環境において、Denied および Unknown 状態に対するエラーハンドリングと復旧動作を確認する。
- [ ] ロールバック対象の設定を記録し、以前の設定モードへ復元できることを確認する。

## リポジトリの初期化とアダプター保守

### リポジトリの初期化（`gal init`）

新しいリポジトリでは、`gal init` が `gal-core` のテンプレートからアダプターのルートファイル（`AGENTS.md`）を作成し、`.dev/project.md` と `.dev/state.md` を含む `.dev/` を初期化します。リポジトリがすでに初期化済み、または一部だけ初期化されている場合、`gal init` はファイルを書き込まずに中止し、終了コード `1` で終了します。

GAL が提供するリポジトリの指示ルートファイルは `AGENTS.md` だけです。コーディングエージェントの指示読み込みの設計によっては、既存の `CLAUDE.md` などのファイルがあると、エージェントが GAL の `AGENTS.md` を読み込まない場合があります。

使用するコーディングエージェントのドキュメントで、対応する指示ファイル名、ファイルの探索方法、読み込みの優先順位を確認してください。エージェントが `AGENTS.md` を読み込まない場合は、そのドキュメントに記載された方法で設定してください。ファイルを生成しただけで、すべてのコーディングエージェントが自動的に読み込むとは考えないでください。

| リポジトリの状態 | `.dev/project.md` | `.dev/state.md` | `gal init` の動作 | 終了コード |
| --- | --- | --- | --- | --- |
| 未初期化 | なし | なし | テンプレートから両方の状態ファイルを作成し、アダプターのルートファイルをレンダリングします。 | 0 |
| 初期化済み | あり | あり | `.dev/project.md` が存在するため中止します。`gal render-adapters` の実行を案内します。 | 1 |
| 一部初期化 | あり | なし | `.dev/state.md` が欠落しているため中止します。Git からの復元を案内します。 | 1 |
| 一部初期化 | なし | あり | `.dev/project.md` が欠落しているため中止します。Git からの復元を案内します。 | 1 |

エラーメッセージには、実行可能な復旧手順が示されます。

```text
gal init: this repository is already initialized (.dev/project.md exists).
  To regenerate AGENTS.md from .dev/project.md, run: gal render-adapters
  To start over from the templates, delete .dev/project.md and .dev/state.md, then run gal init again.
```

```text
gal init: this repository is half-initialized: .dev/project.md exists but .dev/state.md is missing.
  Restore .dev/state.md from version control, or delete .dev/project.md and run gal init again.
  gal init does not overwrite .dev/project.md.
```

### アダプターの再生成（`gal render-adapters`）

初期化済みのリポジトリでは、`gal render-adapters` が `.dev/project.md` を唯一の権威ある情報源としてアダプターファイルを更新します。リポジトリルートから実行してください。

| リポジトリの状態 | 条件 | `gal render-adapters` の動作 | 終了コード |
| --- | --- | --- | --- |
| 初期化済み | `.dev/project.md` あり | `AGENTS.md` と条件付きレイヤーを再生成し、廃止されたルートファイルを整理します。 | 0 |
| 初期化済み | `AGENTS.md` または条件付きレイヤーのファイルが GAL の生成マーカーなしで存在する | ユーザー所有のファイルを保護するため、アダプターファイルを一切書き込まずに中止します。 | 1 |
| 未初期化 | `.dev/project.md` なし | 中止: `gal render-adapters: .dev/project.md not found. Run gal init first.` | 1 |
| 任意 | 引数が渡された | 中止: `gal render-adapters: unknown option`。 | 64 |

`gal render-adapters` コマンドは `.dev/state.md` を参照しません。

### アダプターのクリーンアップと廃止ファイル

既存のリポジトリを更新するとき、または 5 つのアダプタールートファイルを生成していたバージョンから移行するとき、`gal render-adapters` は非推奨となった成果物を整理します。

- **廃止されたルートファイルの削除**: 以前のブリッジファイル（`CLAUDE.md`、`GEMINI.md`、`.github/copilot-instructions.md`、`.agents/rules/gal.md`）は廃止されました。アダプターの更新時、GAL はこれらのパスを検査します。ファイルの先頭行に GAL の生成マーカーが含まれている場合、そのファイルは削除され、`pruned (GAL-owned)` として報告されます。ファイルの削除によって `.agents/rules/` や `.github/` が空になった場合は、そのディレクトリも削除されます。
- **ユーザーファイルの保持**: GAL の生成マーカーを持たないファイルは、ユーザーのファイルとして扱われます。GAL はこれらに手を加えず、`kept (hand-owned)` として報告します。
- **条件付きレイヤー**: `.dev/project.md` で Rust 規約が有効になっている場合、条件付きルールファイル（`.claude/rules/gal-rust.md` および `.github/instructions/gal-rust.instructions.md`）がメインのアダプターと一緒に更新されます。Rust 規約が無効になっている場合は、古くなった GAL 管理のルールファイルが削除されます。

GAL は、`pruned (GAL-owned)` と `kept (hand-owned)` の報告を、標準エラー出力に警告行として出力します。

### プロジェクト仕様（`.dev/project.md`）

`.dev/project.md` ファイルは、アダプターのレンダリングに使用される中核のプロジェクトプロファイルを定義します。このファイルには、**次の 8 つの必須 H2 セクションをそれぞれちょうど 1 回ずつ含める必要があります**: `What This Is`、`Tech Stack`、`Architecture`、`Constraints`、`Response Style`、`Freshness`、`Project Language`、`Protected Paths`。ファイルにはその他の H2 セクションを含めてもかまいません。`gal init` が用意するテンプレートには、`Key Decisions` や `Verified Facts` などの追加セクションが含まれています。

検証は fail-closed（既定で遮断）で行われます。`gal init` がテンプレートからこのファイルを用意するため、検証は手動編集による構造の破損を防ぐためのものです。必須セクションが欠落または重複している場合、`gal render-adapters` は実行を中止し、問題のある見出しを報告し、ファイルを一切書き込みません。1 回の実行で報告されるエラーは 1 件だけです。複数の見出しが欠落している場合は、`plugins/gal-core/templates/project.md` と照らし合わせて修正し、コマンドを再実行してください。

次の 2 つのサイズ制限が適用されます。

- **`.dev/project.md` のサイズ予算**: 改行コードを正規化した後のファイルサイズが 30,720 バイトを超える場合、`gal render-adapters` は警告を表示したうえで、アダプターのレンダリングを実行します。予算内に収まるようにファイルを削ってください。
- **`AGENTS.md` の上限**: レンダリング後の `AGENTS.md` が 32,768 バイトを超える場合、`gal render-adapters` は中止し、アダプターファイルを一切書き込みません。ジェネレーターが、この上限に収めるために不完全なアダプターファイルを出力することはありません。レンダリング結果が上限内に収まるまで `.dev/project.md` を削ってください。

`Tech Stack` 表の直下に、`.dev/project.md` は権威ある検証チェックブロックを含みます。

````markdown
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
````

`gal finalize-check` ゲートは、full モードでこれらのコマンドを実行します。コマンドは 1 つずつ順番に実行され、前のコマンドが失敗した場合でもすべてのコマンドが実行されます。失敗した各コマンドは、失敗したチェックとして報告されます。

GAL はこれらのコマンドの実行にシェルを使用しません。各コマンドを空白で分割し、最初の語をプログラムとして起動します。引用符、パイプ、`&&`、変数展開、シェルの組み込みコマンドなどのシェル機能は動作しません。各プログラムは、`PATH` 上の実行ファイルとして存在している必要があります。

`gal finalize-check` は、現在の作業ディレクトリをリポジトリルートとして扱います。リポジトリルートから実行してください。サブディレクトリから実行すると `.dev/project.md` を見つけられず、チェックを未実行として報告します。

テストスイートのないドキュメントリポジトリでは、チェックを実行するすべてのマシンに存在する単一のコマンドを配列に設定してください。たとえば `["git --version"]` です。Windows では `["true"]` を使用しないでください。Windows にはデフォルトで `true` 実行ファイルがないため、チェックが失敗します。

## 診断の実行（`gal doctor`）

バイナリ、正規ルート、ランタイム設定、およびマシン設定全般の健全性を検証するには、`gal doctor` を実行します。このチェックは、初回セットアップ後、アップグレード後、またはエージェントがコマンドを認識しなくなった際に実行してください。

主な診断チェック項目は以下のとおりです。

- **opencode の投影の同期状態**: `~/.config/opencode/` 内で正規ルートと内容が乖離しているコマンドやエージェントのファイルを検出します。古いファイルには警告が出力され、欠落しているファイルにはエラーが出力されます。不整合を解消するには `gal refresh` を実行します。
- **Claude スキルのジャンクション**: `~/.claude/skills/gal` が存在しない場合に警告を発し、手動での作成手順を表示します。`gal refresh` コマンドがこのリンクを自動作成することはありません。
- **アダプターサイズの分析**: 初期化済みリポジトリにおいて、`AGENTS.md` のバイト数を報告します。推奨サイズを超過しているファイルに対しては、処理を中断しない参考情報として `[WARNING]` 通知が出力されます。

`gal doctor` を単体で実行しても、ヘッドレスのコーディング CLI を通じてタスクが実行されることはありません。ただし、Codex がルーティングされている場合は、範囲を限定した準備状態の確認として `codex exec --help` を実行します。

ヘッドレス実行をテストするには、`gal doctor --executor-smoke` を使用します。スモークテストは、パイプラインが使用するのと同じディスパッチ経路を通じて最小限の実タスクを送信します。そのため、各プロバイダーを呼び出し、クォータを消費します。`codex`、`claude`、`copilot`、`agy`、`opencode` について、CLI のインストール状態、ログイン状態、レシート生成をテストします。スモークテストは、`executorRouting` で設定したモデルではなく、独自に組み込まれたデフォルトのモデルを使用します。

スモークテストを調整するには、次のオプションを使用します。

- `--executor <name>` は 1 つのエグゼキューターをテストします。
- `--timeout <seconds>` はエグゼキューターごとのタイムアウトを設定します。デフォルトは 300 秒です。
- `--transport ssh` はリモート実行レーンをテストします。`--ssh-target <target>` と `--remote-workdir <dir>` も必要です。
- `--json` は機械可読な出力を表示し、`--strict` はいずれかの行が合格しなかった場合に終了コード `1` で終了します。CI では両方を使用してください。

ルーティングとモデルの設定については [設定](configuration.ja.md#エグゼキュータールーティングexecutorrouting) を参照してください。実行ワークフローについては [ワークフロー](workflows.ja.md#タスク実行サイクルと各ロール) を参照してください。

## ランタイムの呼び出し構文とキャッシュ

### ランタイム別コマンド構文

各ランタイムによって機能の登録方式が異なるため、呼び出し構文もツールごとに異なります。

| ランタイム | コマンド構文 | 備考 |
| --- | --- | --- |
| Claude Code | `/gal status` または `/gal-status` | `/gal status` は引数 `status` を付けて `gal` コマンドを実行し、status の手順へルーティングされます。 |
| OpenAI Codex | `$gal-status` または `$gal status` | Codex は、カスタムスキルを `/gal` ではなく、`$` または `/skills` を通じて呼び出します。 |
| GitHub Copilot | `/gal-status` | スキルとして登録されます。 |
| Google Antigravity | `/gal-status` | エージェントスキルとして登録されます。 |
| opencode | `/gal-status` | ネイティブの Markdown コマンドです。 |

### 更新の反映遅延とキャッシュ

コマンドの更新が反映されるまでの時間は、各ランタイムのキャッシュ挙動によって異なります。

- **Claude Code**: セッションの開始時に `gal@skills-dir` プラグインを読み込みます。`gal refresh` の後は、新しい Claude Code セッションを開始してください。マーケットプレイス経由でインストールした場合はキャッシュされたコピーが保持されるため、再インストールと再起動が必要です。
- **OpenAI Codex**: 自身のキャッシュからプラグインを読み込みます。このキャッシュは `gal refresh` では更新されません。GAL をアップグレードした後は、[OpenAI Codex](#openai-codex) の説明に従ってプラグインを再インストールしてください。GAL が `~/.agents/skills/` に投影したスキルは、そのディレクトリから読み込まれます。変更が反映されない場合は、Codex を再起動するか、新しい会話を開始してください。
- **Google Antigravity**: プロセスの起動時にコマンドを読み込みます。変更を反映させるには `agy` を再起動する必要があります。
- **GitHub Copilot および opencode**: 新しい会話セッションを開始するたびに、コマンドおよびスキルの定義を読み込みます。

Codex ランタイムに関する留意事項:

- **コンテキスト予算による切り詰め**: Codex は初期スキル一覧のサイズに上限を設けています。コンテキスト予算が厳しい場合、説明文を短縮したり、オートコンプリートメニューからスキルを省略したりします。省略されたスキルも、`$skill-name` を使用して実行できます。
- **重複したスキル一覧**: 複数のツールが同一の名前でスキルを登録した場合、Codex は選択メニューに両方のエントリを表示します。
