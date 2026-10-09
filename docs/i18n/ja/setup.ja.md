---
source: docs/setup.md
lang: ja
source_commit: 2a697bc89e4415d86d95ab79f9cb9c50c591fbfa
translated_at: 2026-10-01
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

**PowerShell のエイリアス競合について**: PowerShell には既定で `Get-Alias` に対応する組み込みエイリアス `gal` が存在します。対話型セッションでは、このエイリアスが原因で本来の `gal` バイナリの呼び出しが覆い隠される（シャドウされる）ことがあります。`gal` を呼び出した際に `Get-Alias` が起動してしまう場合は、明示的に `gal.exe` と呼ぶか、絶対パスを指定するか、または PowerShell の `$PROFILE` 内でエイリアスを削除してください。

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

プラグイン登録後に `gal refresh` を実行してください。GAL はローカルの登録状態を検出します。登録がない場合や確認できない場合はフォールバックファイルを投影し、状態が不明な場合は警告します。

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
3. `gal refresh` を実行します。Claude の登録を確認できた場合に限り、プラグインがコマンドを所有し、GAL はフォールバックコマンドを省略します。状態が不明な場合はフォールバックを維持して警告します。

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
4. `gal refresh` を実行します。Codex の登録を確認できた場合、プラグインがコアスキルを所有します。GAL は TOML エージェントとコマンドを引き続き投影します。

Codex パイプラインフックは、Codex プラグインマニフェストの `.codex-plugin/plugin.json#hooks` に含まれます。GAL はルートのデフォルトフックや Claude Code マニフェスト内のフックをインストールしません。更新後、Codex プラグインマニフェストを確認し、ガード付き継続を利用する前に Codex で GAL プラグインを信頼済みとして設定してください。信頼状態とフックの実行はランタイム上の事実です。`gal doctor` は静的なパッケージ状態とコマンド準備状態を報告できますが、信頼されたフックを Codex が実行したことを証明できるのは、ライブの同一セッションハンドシェイクだけです。

**共有スキルディレクトリに関する注意事項**: Codex と opencode は `~/.agents/skills` を共有します。opencode を選択している場合、Codex の登録が確認済みでも、GAL は opencode に必要な共有スキルを投影し続けます。

## Codex パイプライン実行のセットアップ

本節の手順は、2026-10-08 に確認した Windows 上の Codex CLI 0.161.0 と Codex Desktop 26.1002.7124.0 を対象としています。異なるバージョンを使用している場合は、Codex の公式ドキュメントで最新の動作を確認してください。公式の設定項目名および権限境界については、[permission modes](https://learn.chatgpt.com/docs/permission-modes)、[agent approvals and security](https://learn.chatgpt.com/docs/agent-approvals-security)、および [configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) を参照してください。

### 既知の問題 Codex Windows サンドボックスの障害

Windows では現在、Codex のサンドボックスがコマンドを実行する前に失敗します。GAL パイプラインはサンドボックス内では実行できません。これは Codex の不具合であり、GAL やリポジトリの問題ではありません。GAL 側で修正することもできません。

**症状。** サンドボックス内で実行するコマンドはすべて失敗し、`helper_unknown_error: setup refresh had errors` が表示されます。続いてエージェントは、ファイルを読み取れない、またはシェルを起動できないと報告します。エグゼキューター自体は起動していても、パイプラインのフェーズは Verdict `BLOCKED`、`no-receipt`、またはテスト 0 件で終了します。

**確認方法。** `%USERPROFILE%\.codex\.sandbox\sandbox.<date>.log` を開き、`setup refresh completed with errors` を検索してください。報告される原因は実行ごとに異なる場合があります。確認された原因には、使用中の Codex ランタイムファイル（os error 32）、deny ACE の失敗、write ACE の失敗があります。Codex の再起動、サンドボックスサービスの再起動、OS の再起動では解消しませんでした。マシンを直接確認するには、使い捨てのディレクトリで次のコマンドを実行します。

```powershell
codex exec --json -s workspace-write "Run the command Get-Location in PowerShell and report its output"
```

`command_execution` イベントが `exit_code` `-1` と `setup refresh had errors` を報告した場合、そのマシンは影響を受けています。

**GAL の対応。** GAL は、ディスパッチするすべての Codex 子プロセスを `--dangerously-bypass-approvals-and-sandbox` 付きで起動します。そのため子プロセスは Codex のサンドボックス外で実行され、他のエグゼキューターが権限バイパスのフラグで得るフルアクセスと同じ信頼レベルになります。以前の GAL は子プロセスを `-s workspace-write` で起動していました。この不具合の下では、その設定によってすべての Codex ディスパッチが失敗していました。

**必要な対応。** GAL パイプラインを動かす外側の Codex セッションは、サンドボックスなしで実行する必要があります。[必須の権限モード Full access](#必須の権限モード-full-access) を参照してください。**Approve for me**（auto-review）は回避策になりません。これは承認要求を誰がレビューするかを決めるだけであり、コマンドは障害の起きているサンドボックス内で実行されます。

**セキュリティへの影響。** サンドボックスがない場合、Codex はファイルシステム全体を読み書きでき、現在のアカウントで実行可能な任意のターミナルコマンドを実行できます。GAL パイプラインは、信頼できる開発マシンと信頼できるリポジトリでのみ実行してください。これは [workflows.ja.md](workflows.ja.md) の「セキュリティ上の警告: 権限のバイパス」が、すべてのエグゼキューターに対してすでに求めている条件です。

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
6. 初期化は新しいリポジトリに対してのみ実行してください。複数の計画がアクティブな場合は、対象のプロンプトファイルを明示的に指定してください。

### Codex のガード付き継続の準備

ガード付き `codex_stop_v1` プロファイルには、信頼された GAL Codex プラグインと、ライブの同一セッションフックハンドシェイクが必要です。モデル名、環境の推測、古いマーカー、または別ホストのマーカーがないことによって、このプロファイルが有効になることはありません。有効なグラントを伴わない通常の `gal pipeline <prompt>` 呼び出しは `legacy_interactive` のままです。フックが実行されない場合、ホストに依存しない通常の呼び出しは、Codex フックが存在しない、無効である、または信頼されていないことを検出できません。

フックは、明示的な実行プロンプトパス、対応する実行プロンプトへ解決できる `#file:` または `@` のソースプラン参照、および `.dev/state.md` にアクティブなプランが 1 件だけある場合のパスなし要求を受け付けます。アクティブプランの状態が欠落または曖昧な場合、パスが `.dev/plans/` の外にある場合、パス構文が安全でない場合、または要求に `from` や `stop-at` 修飾子が含まれる場合、フックは要求を拒否します。ガード付き継続はこれらの修飾子を黙って破棄しません。ガード付きコーディネーターが返す型付きのタスクおよびフェーズアクションを使用してください。

ガード付き継続を利用する前に、次の確認を完了してください。

1. Codex プラグインを登録した後、`gal refresh` を実行します。GAL は登録状態を自動検出します。
2. インストール済みの GAL Codex プラグインマニフェストに、GAL の `UserPromptSubmit`、`PreToolUse`、`Stop` フックが含まれていることを確認します。これらをルートの `hooks/hooks.json`、ルートの `plugin.json`、または Claude Code マニフェストに追加しないでください。
3. `gal doctor` を実行して静的なパッケージ状態とコマンド準備状態を確認します。Codex で GAL プラグインを信頼済みとして設定してください。静的診断では、ランタイムの信頼状態やフック実行を証明できません。
4. 信頼された対象ワークスペースで、曖昧さのないパイプライン要求を開始します。Codex が同一セッションの Stop カナリアを実行できるようにしてください。カナリアはブートストラップを確認し、グラントを含む正確な継続要求を返す必要があります。次の `UserPromptSubmit` は、ガード付き CLI エントリがグラントを消費する前に、同じセッションでその継続要求をバインドする必要があります。その後、ガード付きエントリは実装を開始する前に、新しいエントリラッチと意味的なタスクチェックポイントを実行します。
5. フックが存在しない、無効である、信頼されていない、またはカナリアが失敗した場合は、ガード付き継続を利用しないでください。明示的な `--require-codex-stop-v1` エントリは、グラントがないか無効な場合に `host-continuation-not-ready` を返して実装前に失敗する必要があります。通常の呼び出しは `legacy_interactive` のままです。ホストに依存しない CLI は、自身が Codex 上で実行されていることを推測できません。

ブートストラップが保留中の場合、Codex の `PreToolUse` フックは対応するすべてのツール呼び出しを拒否します。Stop カナリアが一度限りのグラントを作成した後は、同じセッション内のグラント付き最初のアクションだけを許可します。未対応の特殊ツールまたはホスト提供ツールの経路は、このブートストラップ保証の対象外であり、ブートストラップ継続には使用できません。ガード付きエントリがグラントを消費してコーディネーターのリビジョン 0 を作成すると、このブートストラップ制限は終了します。アクティブなフックは通常のツール使用に対して `neutral/pass` を返します。この結果はチェックポイントやフェーズ遷移を承認するものではありません。GAL のガード付きアクションは、引き続きコーディネーターのリビジョン、レシート、タスク品質チェックを適用します。Stop フックは、終端状態または型付きの人間対応要求に達するまで、早すぎる最終応答を阻止します。

ガード付き継続が停止した場合は、診断中もワークスペースとセッション識別子を変更しないでください。静的な準備状態を確認するには `gal doctor` を実行し、その後 `.dev/pipeline/<plan-scope>/` 内のコーディネーターと試行エビデンスを確認します。信頼、マニフェスト、またはハンドラーの問題を解決し、投影されたアセットが古い場合は `gal refresh` を実行してください。新しい信頼済みの同一セッションハンドシェイクを開始して、新しいグラントを取得します。グラントをコピー、手動作成、または再利用しないでください。識別情報またはエビデンスのチェックで競合が報告された場合は、記録された状態を保持し、再試行前に競合を解決してください。Codex のガード付き動作を使わずに継続するには、通常の `gal pipeline <prompt>` 経路と既存のホスト所有継続フローを使用します。

ディスパッチを実行するシェルのコンテキスト内に実行ファイルが存在しない場合、準備状態は **Missing** と報告されます。権限またはセキュリティポリシーによってコマンド探索が明示的にブロックされている場合は **Denied** と報告されます。結果が不完全または矛盾している場合は **Unknown** と報告されます。コマンド探索の失敗単体では Codex がアンインストールされているとは断定できません。Denied や Unknown の報告のみを根拠に再インストールを試みないでください。

### 必須の権限モード Full access

[Windows のサンドボックス障害](#既知の問題-codex-windows-サンドボックスの障害) が続いている間は、GAL パイプラインを実行する外側の Codex セッションでこのモードを使用してください。本ガイドの以前の版では、RTK と GAL のコマンドルールを組み合わせた **Approve for me**、またはカスタムの `workspace-write` 設定を推奨していました。どちらの方法も障害の起きているサンドボックス内でコマンドを実行するため、現在はパイプラインを実行できません。

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

GAL が Codex の承認設定やサンドボックス設定を変更することはありません。外側のセッションのモードはユーザーが決定します。外側のモードにかかわらず、ディスパッチされた Codex 子プロセスはサンドボックス外で実行されます。組織が管理する制限は、引き続き外側のセッションに適用されます。詳細については OpenAI 公式ドキュメントの[サンドボックスと承認](https://developers.openai.com/codex/sandboxing)を参照してください。

この設定で対応できない環境固有のケースや関連する問題がある場合は、[issue を作成](https://gitlab.com/monkey1wizard/Golem-Agents-Legion/-/issues/new)してください。
### ソース worktree のランタイムとリリース境界

各ソース worktree は、パイプラインのビルドキャッシュ、不変ランタイム世代、コーディネーターの状態を、それぞれ専用の正規 `target/gal-pipeline/` に保存します。これらのプライベートルートにより、ソース worktree 同士、および下流リポジトリで使うインストール済み GAL とを分離します。

リリース前に、隔離した fixture でパッケージマネージャー `artifact` を検証します。候補アーティファクトを fixture にインストールし、そこでバージョンとランタイムの動作を確認してください。fixture はユーザーの共有インストールから分離します。ソース worktree がインストール済み shim を昇格または置換することはありません。共有 shim は、パッケージマネージャーのリリースを通じてのみ変更されます。ソース更新では、プライベートな不変世代と安全な再バインドを使用します。下流リポジトリでは、引き続きインストール済み GAL を使用します。

### スモークテスト、パイプライン確認、ロールバック

設定のテストには、使い捨てのリポジトリと、単一ファイルを作成する最小限の計画を使用してください。対象の Codex スモークテストを実行し、続いて実装、テスト、独立監査、および中断フェーズからの再開を網羅するパイプラインを実行します。試行ログ、実行レシート、および最終ゲートの証拠を検査してください。各段階はそれぞれ異なる証拠を提供します。コマンド探索の成功はバイナリの利用可能性を証明し、プロセス起動は実行が開始されたことを証明し、プロバイダーの応答は API 通信が成立したことを検証します。有効な実行レシートと合格したゲート検査は、それぞれ独立して評価されます。

アクセスが拒否（Denied）された場合は、パイプラインの復旧地点を保持し、有効な権限モードでサポートされている場合に限り、その特定の有界な操作に対する Codex のネイティブ承認を要求してください。失敗の原因が不明（Unknown）な場合は、裏付けとなるデータなしに上流モデルや認証の欠陥と決めつけず、診断ログを保持した上で観測された段階を直接調査してください。TLS 証明書の検証を無効化したり、ユーザーのホームディレクトリ全体に権限を付与したり、実行中の試行の途中でルーティングを変更したりしてはなりません。

設定変更をロールバックする場合は、バックアップから本手順で変更した設定のみを復元し、Codex Desktop で以前のモードを再選択した上で、有効なタスクコンテキストを再確認してください。設定ファイル全体を無差別に置き換えたり、ユーザーの資格情報を削除したりしないでください。

### 開発者受け入れチェックリスト

各結果は、自動化されたパイプライン出力や finalize ゲートとは独立して記録してください。開発者自身が実施するまで、手動確認やライブ環境での確認項目は **NOT RUN**（未実行）のまま保持します。

- [ ] Codex Desktop および CLI のバージョンと、選択した権限モードを記録する。
- [ ] 資格情報データやトークンファイルを露出させることなく、ユーザーセッションのログインを確認する。
- [ ] ルーティングされた各エグゼキューターを確認し、使い捨てディレクトリでエグゼキュータースモークテストを実行する。
- [ ] 最小限のパイプラインを実行し、実装、テスト、独立監査、および再開の証拠を検査する。
- [ ] ご利用の環境において、Denied（拒否）および Unknown（不明）状態に対するエラーハンドリングと復旧動作を確認する。
- [ ] ロールバック対象の設定を記録し、以前の設定モードへ確実に復元できることを確認する。

### GitHub Copilot

GitHub Copilot は、Agent Plugins 1.0.0 パッケージとして GAL を読み込みます。

1. Copilot の設定で `~/.gal/plugins` をディレクトリマーケットプレイスとして登録します（これは `~/.gal/plugins/.claude-plugin/marketplace.json` を参照します）。その後 `gal` を有効化します。Copilot はファイルをコピーせず、インプレースで読み込みます。エージェントペルソナは `com.github.copilot/agents/` から、ワークスペースルールは `com.github.copilot/rules/` から、MCP ツールは `mcp.json` からそれぞれ読み込まれます。
2. `copilot plugin list` を実行し、`gal` が有効になっていることを確認します。
3. `gal refresh` を実行します。Copilot の登録を確認できた場合、プラグインがエージェントを所有します。Copilot はプラグインの `commands/` からコマンドを読み込まないため、GAL はコマンドスキルを引き続き投影します。

### Google Antigravity

Antigravity は、ローカルディレクトリ探索を通じて GAL を登録します。

1. 正規ルートから直接プラグインをインストールします。
   ```bash
   agy plugin install ~/.gal/plugins/gal
   ```
   これにより、正規ルートを指すジャンクションが `~/.gemini/antigravity-cli/plugins/gal` に作成されます。
2. 重複した `claude-code` インポートが存在する場合は整理します。以前に Antigravity が Claude Code からプラグインをインポートしていた場合、`agy plugin list` に重複した `gal` エントリ（`local-install` と `claude-code`）が表示されることがあります。`~/.gemini/config/import_manifest.json` を開き、`imports` 配列から `claude-code` のエントリを削除して `local-install` のみを残してください。その後 `agy plugin list` で有効なエントリが 1 つだけであることを確認します。
3. `gal refresh` を実行します。Antigravity の登録を確認できた場合、プラグインがコマンドスキルを所有します。GAL は正規プラグインリンクを維持します。

### opencode（プロジェクションフォールバック）

opencode は Markdown ベースのプラグインマニフェストに対応していません。その拡張モデルは、npm またはローカルディレクトリからインストールされる JavaScript および TypeScript モジュールを前提としています。そのため、opencode では GAL のファイルプロジェクションによるフォールバックを利用します。`gal refresh` を実行すると、Markdown コマンドが `~/.config/opencode/commands/` へ、エージェントが `~/.config/opencode/agents/` へ、コアスキルが `~/.agents/skills/` へそれぞれ投影されます。

### 対話的なインストール手順

Claude Code や Codex のマーケットプレイススナップショットを通じて GAL を試すユーザーは、チャット上で対話的に初期インストールを完了できます。

1. Claude Code または Codex のマーケットプレイスから「gal」で検索して **GAL プラグイン** をインストールするか、リポジトリのスナップショットブランチから読み込みます。
2. チャット内でアシスタントに対し、**「help me install gal」**（または「gal をインストールして」）と指示します。組み込みの `install-gal` スキルが実行確認を求めた後、適切なパッケージマネージャー（Homebrew、WinGet、または `cargo install --git`）を呼び出してバイナリをインストールします。インストール後、新規リポジトリでは `gal init` が、既存リポジトリでは `gal render-adapters` が実行されます。

マーケットプレイスのプラグイン単体では、完全な動作環境は整いません。各種ワークフローコマンドは、ネイティブの `gal` バイナリの存在に依存しています。バイナリの導入が完了した後は、前述の手順に従って各ランタイムのプラグイン登録を行ってください。

## リポジトリの初期化とアダプター保守

### リポジトリの初期化（`gal init`）

新しいリポジトリにおいて、`gal init` は `gal-core` のテンプレートからアダプターの根本ファイル（`AGENTS.md`）を作成し、`.dev/project.md` と `.dev/state.md` を含む `.dev/` ディレクトリを初期化します。リポジトリがすでに初期化済み、または中途半端に初期化されている場合、`gal init` はファイルを書き換えることなく直ちに中止し、終了コード `1` で終了します。

GAL が提供するリポジトリのルート指示ファイルは `AGENTS.md` だけです。リポジトリに `CLAUDE.md` などがあると、coding agent の読み込み設計によっては GAL の `AGENTS.md` が読み込まれない場合があります。

使用する coding agent のドキュメントで、対応する指示ファイル名、ファイルの探索方法、読み込みの優先順位を確認してください。`AGENTS.md` が読み込まれない場合は、そのドキュメントに従って設定してください。ファイルを生成しただけで、すべての coding agent が自動的に読み込むとは限りません。

| リポジトリの状態 | `.dev/project.md` | `.dev/state.md` | `gal init` の動作 | 終了コード |
| --- | --- | --- | --- | --- |
| 未初期化 | なし | なし | テンプレートから両方の状態ファイルを作成し、アダプター根本ファイルをレンダリングします。 | 0 |
| 初期化済み | あり | あり | `.dev/project.md` が存在するため中止。`gal render-adapters` の実行を案内します。 | 1 |
| 一部初期化 | あり | なし | `.dev/state.md` が欠落しているため中止。Git からの復元を案内します。 | 1 |
| 一部初期化 | なし | あり | `.dev/project.md` が欠落しているため中止。Git からの復元を案内します。 | 1 |

エラーメッセージには、次にとるべき具体的な復旧手順が示されます。

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

初期化済みのリポジトリにおいて、`gal render-adapters` は `.dev/project.md` を唯一の情報源としてアダプターファイルを更新します。

| リポジトリの状態 | `.dev/project.md` | `gal render-adapters` の動作 | 終了コード |
| --- | --- | --- | --- |
| 初期化済み | あり | `AGENTS.md`、および条件付きレイヤーを再生成し、廃止された古い根本ファイルを整理します。 | 0 |
| 未初期化 | なし | 中止: `gal render-adapters: .dev/project.md not found. Run gal init first.` | 1 |

なお、`gal render-adapters` コマンドが `.dev/state.md` を参照することはありません。

### アダプターのクリーンアップと廃止ファイルの整理

既存リポジトリの更新時、または以前のバージョンから移行する際、`gal render-adapters` は非推奨となった古い成果物を整理します。

- **廃止された根本ファイルの削除**: 過去のブリッジファイル（`CLAUDE.md`、`GEMINI.md`、`.github/copilot-instructions.md`、`.agents/rules/gal.md`）は廃止されました。アダプターの更新時、GAL はこれらのパスを検査します。ファイルの先頭行に GAL の生成マーカーが含まれている場合、そのファイルは削除され `pruned (GAL-owned)` として報告されます。ファイルの削除によって `.agents/rules/` や `.github/` が空になった場合、その親ディレクトリも削除されます。
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
- **アダプターサイズの分析**: 初期化済みリポジトリにおいて、`AGENTS.md` のバイトサイズをレポートします。推奨サイズを超過しているファイルに対しては、処理をブロックしない参考情報として `[WARNING]` 通知が出力されます。

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
