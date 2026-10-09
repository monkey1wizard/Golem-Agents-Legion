---
source: docs/projection.md
lang: ja
source_commit: bddd9fb0bdbb559a106f86152ebf55a7d0494b25
translated_at: 2026-10-01
type: Reference
title: プロジェクションとランタイム操作面
description: GAL がソース契約を canonical root にレンダリングし、各ランタイム操作面へプロジェクションして、リポジトリアダプター、プロジェクションレジストリ、MCP マニフェストを生成する方法を説明します。
tags:
  - projection
  - runtime
  - adapters
  - render
status: stable
---

# プロジェクションとランタイム操作面

[English](../../projection.md) · **日本語** · [繁體中文](../zh-Hant/projection.zh-Hant.md)

## プロジェクションのライフサイクル

GAL のスキル、コマンド、エージェント定義、テンプレートは、`plugins/gal-core/` 配下に配置されたバージョン管理対象の Markdown ソースファイルです。各 AI ランタイムがこれらのファイルを直接読み込むことはなく、レンダリングおよびプロジェクション（投影）の工程を経て初めて有効化されます。

GAL は以下の 2 つの経路で契約（contracts）を配信します。

1. **マシンレベルのプロジェクション（Machine-level projection）**: 組み立てられた契約群を、サポート対象の各ランタイムの設定ディレクトリへコピーまたはリンクします。
2. **リポジトリレベルの生成（Repository-level generation）**: リポジトリの `.dev/project.md` をコンパイルし、`AGENTS.md` のリポジトリローカルなアダプターファイルを生成します。

プロジェクションパイプラインは、厳格な境界を持つ 3 つのモジュールに分割されています。

- **正規ルートのレンダリング（Canonical root rendering）**（`crates/gal-engine/src/render/`）: `plugins/gal-core/` のコア契約と、任意の `~/.gal/local/`（個人カスタマイズ）をコンパイルし、`~/.gal/plugins/gal/` の正規ルート（canonical root）を構築します。
- **ランタイム操作面へのプロジェクション（Runtime surface projection）**（`crates/projection/`）: 正規ルート内の成果物を、各対象ランタイムがサポートする形式へ変換し、それぞれの設定ディレクトリへ配布します。
- **リポジトリアダプターの生成（Repository adapter generation）**（`crates/cli/src/gal/render.rs`）: リポジトリ内の `.dev/project.md` をもとに `AGENTS.md` を生成します。この処理は前述の 2 つのモジュールを呼び出すことなく完全に独立して動作します。

```text
plugins/gal-core/（バージョン管理されるソース契約群）
        │
        ├── ~/.gal/local/（個人オーバーレイ層、存在する場合に統合）
        ▼
   正規ルートのレンダリング（Canonical Root Render）
   （Core-Wins 規則による衝突解決）
        ▼
~/.gal/plugins/gal/
（Agent Plugins 仕様に準拠した正規ルート）
        │
        │  ランタイム別プロジェクション
        │  （対象に応じてジャンクション、シンボリックリンク、またはファイルコピー）
        │
        ├──▶ Claude Code
        │      プラグインコマンドおよびサブエージェント
        │
        ├──▶ GitHub Copilot
        │      ~/.copilot/skills/ + ~/.copilot/agents/
        │
        ├──▶ OpenAI Codex
        │      ~/.agents/skills/（ファイルコピー）+ ~/.codex/agents/
        │
        ├──▶ Google Antigravity
        │      ~/.gemini/antigravity-cli/skills/ + 正規ルートへのシンボリックリンク
        │
        └──▶ opencode
               ~/.config/opencode/commands/ + ~/.config/opencode/agents/
```

各ランタイムへ配布されるコマンドやスキルの内容は、事前にレンダリングされた正規ルートの内容とバイト単位で完全に一致（byte-identical）していなければなりません。環境ごとに配信機構、サンドボックス権限、呼び出しの起動構文が異なる場合があります。ランタイムにホスト固有の指示が必要な場合は、共有される文章も異なることがあります。これらの差異によって、共有コマンドの動作が変わることはありません。

### インストール版とソースビルドのプロジェクション所有権

`package-manager` は、インストール済み GAL 実行ファイルと `shared-target` の共有ランタイムプロジェクションを管理します。この共有プロジェクションを更新できるのは、パッケージマネージャーによるインストールまたは昇格だけです。ソースワークツリーの受け入れ実行では `--root` と `--source` の両方が必要で、呼び出し側が明示した `explicit-root` の下に限りランタイムファイルをプロジェクションできます。GAL は存在しないルート、相対パス、ユーザープロファイル、および共有インストール先のルートについて、ファイルを書き込む前に `reject-before-write` を適用します。`--root` を指定しない `gal refresh` は共有リフレッシュを実行するため、パッケージマネージャーだけが実行してください。`HOME` や `USERPROFILE` などの環境変数は、プロジェクションルートの指定として認められません。ソースビルドの出力は、そのワークツリー専用のプライベートなランタイム領域に保持してください。

## 正規ルート（Canonical Root）アーキテクチャ

各マシンは、`~/.gal/plugins/gal/` に単一の正規ルートを保持します。AI ランタイムが参照するファイル群は、すべてこのディレクトリから投影されたコピーまたはシンボリックリンクであり、独立した情報源（source of truth）ではありません。

プロジェクションは、正規ルートから各対象ランタイムへの一方向のプッシュ同期として動作します。ランタイム側からの変更の書き戻しは行われないため、ランタイム側のファイルは常に正規ルートから決定論的に再構築できます。

## 複数ソースのマージ

正規ルートは、バージョン管理されるコア契約（`plugins/gal-core/`）と、マシンローカルな個人オーバーレイ（`~/.gal/local/`）を統合して生成されます。ファイルパスや識別子が衝突した場合、コア側の契約が優先され、個人側のエントリは破棄されます（Core-Wins 規則）。

ユーザーは補足的なスキルや設定を自由に追加できますが、コア契約を上書きすることは禁止されています。個人オーバーレイは、設定フラグを必要とせず、ファイルの存在そのものによって有効化されます。個人ファイルを削除すると、次回の `gal refresh` 実行時に対応する生成成果物も自動的にクリーンアップされます。GAL が `_galProjection` 名前空間の外部のファイルを変更することはなく、個人オーバーレイディレクトリへ書き戻すこともありません。

## ランタイム別コマンドプロジェクション

GAL のコマンド（例: `/gal status`）は、`commands/` 配下に正規定義されています。各ランタイムによってコマンドの処理方式が異なります。Claude Code と opencode はネイティブなコマンドシステムを備えていますが、GitHub Copilot、Codex、Google Antigravity はコマンドをスキルとして読み込みます。`crates/projection/src/lib.rs::update_commands` のプロジェクションロジックは、正規仕様を各ランタイム固有の構造へ変換します。

| ランタイム | コマンドシステム | プロジェクション先 | 起動構文 |
| --- | --- | --- | --- |
| Claude Code | ネイティブプラグインコマンド | 正規ルートの `commands/` | `/gal status` |
| GitHub Copilot | エージェントスキル | `~/.copilot/skills/<name>/SKILL.md` | `/gal-status` |
| OpenAI Codex | エージェントスキル | `~/.agents/skills/<name>/SKILL.md` | `$gal-status` |
| Google Antigravity | エージェントスキル | `~/.gemini/antigravity-cli/skills/<name>/SKILL.md` | `/gal-status` |
| opencode | ネイティブ Markdown コマンド | `~/.config/opencode/commands/<name>.md` | `/gal-status` |

ネイティブなコマンドシステムを持つランタイム（Claude Code および opencode）に対しては、重複したコマンドスキルのプロジェクションを行わないことで、コマンド一覧の重複表示を防ぎます。なお、`~/.agents/skills/` は Codex、opencode、Copilot で共有されるパスですが、ここへコマンドスキルが投影されるのは Codex が有効になっている場合に限られます。opencode のみが有効な場合は、ネイティブのコマンドファイルのみが作成されます。

ファイル書き込みを行う前に、`write_command_skill` は所有権マーカー（`is_managed`）を検証し、ユーザー独自のディレクトリを誤って上書きしないよう保護します。設定で特定のランタイムが無効化された場合は、`remove_gal_command_skill` が以前に投影されたコマンドファイルを自動的に削除します。

## Codex プラグインフックのプロジェクション

GAL は Codex パイプラインフックを、Codex 専用のソースフラグメント `plugins/gal-core/codex/hooks.json` に保持します。レンダリング時に、GAL はこのフラグメントを `.codex-plugin/plugin.json#hooks` にマージします。このフラグメントは `UserPromptSubmit`、`PreToolUse`、`Stop` の各ハンドラーを定義します。各ハンドラーは GAL 内部の `gal pipeline-host-hook` コマンドを呼び出します。

フックは manifest 専用のアセットです。GAL はルートの `hooks/hooks.json` を作成せず、ルートの `plugin.json` または `.claude-plugin/plugin.json` にフックを追加しません。GAL はこれらのフックを Claude Code や OpenCode にプロジェクションしません。Claude Code と OpenCode は、既存のホスト管理によるパイプライン接続動作を維持します。

```text
plugins/gal-core/codex/hooks.json
          │
          ▼
.codex-plugin/plugin.json#hooks
          ├──▶ Codex プラグインフックハンドラー
          ├──▶ ルートの hooks/hooks.json は作成しない
          ├──▶ ルートの plugin.json にフックを追加しない
          ├──▶ .claude-plugin/plugin.json にフックを追加しない
          └──▶ OpenCode へフックをプロジェクションしない
```

Codex フック分岐は、信頼できるライブフックハンドシェイクの後にのみ、ガード付き継続を有効にします。`gal doctor` は静的なパッケージングとコマンドの準備状態を報告できますが、Codex がハンドラーを信頼または実行したことは証明できません。同一セッションの canary がこのライブ証明を提供します。`Stop` ハンドラーが bootstrap を確認して正確な grant 付き継続を返し、その後 `UserPromptSubmit` がガード付き CLI 入口で grant を消費する前に、同じセッションに継続をバインドします。grant の消費だけでは実装作業を承認しません。ガード付き入口は、先に fresh entry latch と semantic task checkpoint を通過する必要があります。

bootstrap が保留中の場合、`PreToolUse` ハンドラーはサポート対象のすべてのツール呼び出しを拒否します。canary の後は、そのセッションで正確な grant 付き最初のアクションだけを許可します。特殊なツールパスとホスト提供のツールパスはこの保証の対象外であり、bootstrap 継続には使用できません。ガード付き入口が grant を消費して coordinator revision 0 を作成すると、bootstrap の制限は終了します。一致するアクティブプロファイルでは、通常のツール使用に対してフックは neutral/pass を返します。この結果は checkpoint またはフェーズ遷移を承認しません。ガード付き GAL アクションは、coordinator revision、receipt、task-quality check を引き続き検証します。coordinator が terminal になるか、型付きの人手対応必須状態を報告するまで、`Stop` ハンドラーは早すぎる最終応答をブロックします。

ガード付き継続が停止した場合は、診断中もワークスペースとセッション識別情報を変更しないでください。静的な準備状態を確認するために `gal doctor` を実行し、`.dev/pipeline/<plan-scope>/` 配下の coordinator と attempt の証拠を調べます。信頼、manifest、またはハンドラーの問題を修正し、投影済みアセットが古い場合は `gal refresh` を実行します。新しい grant には、信頼できる同一セッションの新しいハンドシェイクを開始します。grant をコピー、手作業で作成、または再利用しないでください。競合を解決するまで、競合する識別情報または証拠の状態を保持します。ガード付き Codex 動作を使わず既存のホスト管理フローで継続するには、通常の `gal pipeline <prompt>` を使用します。

有効な grant のない通常の `gal pipeline <prompt>` 呼び出しは `LegacyInteractive` のままとなり、既存の v1 receipt、marker、終了、変更動作を保持します。ホストに依存しない通常の呼び出しでは、フックが実行されなかった場合に、Codex フックが存在しない、無効、または信頼されていないことを推測できません。grant 必須のガード付き入口は、grant がないか無効な場合、実装前に失敗します。Codex フックは Codex プラグインにのみプロジェクションされます。Claude Code と OpenCode は既存のホスト管理による継続動作を保持します。

GAL の共有スキルとコマンド本文には、Codex 固有の指示を含めることがあります。ホストが必要とする場合、このような文章の差異は許容されます。これらの差異によって Codex フックを別のホストへプロジェクションしたり、そのホストの呼び出しおよび継続の契約を変更したりすることはできません。

## エージェントプロジェクションマトリクス

GAL は `*.agent.md` のロール定義を単純にコピーするのではなく、各ランタイムのネイティブなエージェントまたはスキルの形式へ適切に変換します。

| ランタイム | 対象形式 | プロジェクション先 | 変換機構 |
| --- | --- | --- | --- |
| Claude Code | GAL ヘッダー付き `*.agent.md` | `~/.gal/plugins/gal/agents/` | Claude のサブエージェントへ直接対応付け。 |
| OpenAI Codex | ネストされたテーブルのないフラットな `*.toml` | `~/.codex/agents/<name>.toml` | Codex TOML サブエージェント（`crates/projection/src/codex_agent.rs`）。 |
| Google Antigravity | 投影スキル | `~/.gemini/antigravity-cli/skills/` | スキルディレクトリ経由でマウント。ネイティブエージェントファイルは未検証。 |
| opencode | `mode: subagent` を持つ `<name>.md` | `~/.config/opencode/agents/<name>.md` | `render_opencode_agent` により生成。 |
| GitHub Copilot | Claude ツール名にマップした `*.agent.md` | `~/.copilot/agents/<name>.agent.md` | GAL が登録を確認できない場合にフォールバックファイルを投影します。コマンドスキルは引き続き投影します。 |

GAL は Copilot の `enabledPlugins` 設定から登録状態を検出します。登録を確認できた場合、プラグインがエージェントを所有します。登録がない場合や状態を確認できない場合はフォールバックエージェントを維持し、不明な場合は警告します。登録状態の変更は次の refresh で反映され、GAL が管理するフォールバックファイルだけを削除または復元します。ユーザー所有の競合パスは保持します。

Codex のエージェントファイルは、ネストのないフラットな TOML 構造を採用しています。シリアライザー `serialize_codex_toml` は、ドキュメントルートに 6 つのフィールド（`name`、`description`、`developer_instructions`、`model`、`model_reasoning_effort`、`sandbox_mode`）を定義し、サブテーブルを使用しません。Codex は未認識のキーや旧式のネストキーを含むファイルを拒否するため、フラットなスキーマを厳格に適用することで、`codex doctor` チェック時にスキーマの乖離を即座に検知できるようにしています。

### ロール実行モード

GAL のエージェントロールは、以下の 2 つの実行モードをサポートしています。

- **隔離モード（Isolated Mode）**（`/gal architect`）: 専用のサブエージェントを起動してタスクを実行し、メインの会話には最終的な裁定要約のみを返します。
- **コンテキスト内モード（In-Context Mode）**（`/architect discuss`）: ロールの規律や開発指示を現在のセッションへ直接読み込み、マルチターンでの対話的な相談を可能にします。

4 つのスタンドアロンロールコマンドは `plugins/gal-core/commands/{architect,analyst,designer,releaser}/SKILL.template.md` から生成されます。既存のコマンドスキャナーとレンダラーが他のコマンドと同様に処理し、新しいプロジェクション経路はありません。各コマンドは既定で `gal dispatch-script golem-<role>` を実行し、第 1 引数が `discuss` の場合は内部の `gal consult-script golem-<role>` を実行します。

レンダリング結果とランタイムのインストールモードは別の事実です。

- **正規の生成物**: レンダラーは、正規ルートの下にロールごとに 1 つのコマンドと、対応する 1 つのビルド済みスキルを書き出します。繰り返しレンダリングしても同一の結果になります。
- **インストールモード**: Claude Code と opencode はネイティブコマンドを使用します。Claude Code のプラグインモードでは、プラグインがコマンドを `/gal:<role>` として提供し、追加のフォールバックファイルは書き込まれません。Copilot、Codex、Antigravity は、既存のフォールバックプロジェクションを通じてコマンドスキルを受け取ります。

プロジェクションは `discuss-<role>` スキルを生成しなくなり、`discussSkillProjectionPaths` は廃止されました。`gal refresh` は、廃止された GAL 管理の 4 つのディレクトリを削除し、記録のないユーザーディレクトリは保持します。削除された `$discuss-<role>` スキルに代わるリダイレクトはありません。ランタイムごとの代替手段は次のとおりです。

| ランタイム | 代替手段 |
| --- | --- |
| Claude Code | `/<role> discuss` |
| Claude Code プラグインモード | `/gal:<role> discuss` |
| Codex | `$<role> discuss` |

Codex ネイティブエージェント定義（`~/.codex/agents/<name>.toml`）と、その `model` および `effort` のプロジェクションは変更ありません。

## オーケストレーター駆動役割の除外

パイプラインの 4 つのロール（`implementer`、`tester`、`auditor`、`researcher`）について、`crates/projection/src/lib.rs` の `update_agents` がプロジェクションファイル（`*.agent.md` や `*.toml`）を生成することは**厳格に禁止**されています。これらのロールはパイプライン解決のために内部の `KNOWN_GOLEMS` レジストリに登録されていますが、ユーザー向けの直接呼び出しファイルは持ちません。これにより、監視の行き届かない単独実行を防止します。例えば `/gal tester` を直接実行しても未定義コマンドのエラーとなり、必ずオーケストレーターの統制下でのみ実行されるよう保証されます。

各ロールの定義は抽象的な権限スコープ（`read`、`edit`、`execute`、`search`、`web` など）を宣言します。`crates/projection/src/tool_map.rs` モジュールは、これらの抽象権限を各ランタイム固有のツール識別子へ対応付けます。Codex では、サンドボックスの分離レベルもこれらの権限に対応します。計画レビューロールは `read-only` モードで実行され、パイプライン実装ロールは `workspace-write` モードで実行されます。

## リポジトリアダプター生成

GAL は `CLAUDE.md` を生成しません。アダプターを再生成すると、元の GAL 生成マーカーがある古い `CLAUDE.md` は削除されます。ユーザーが管理する同名ファイルは保持されます。

スキルインデックスを含むアダプターでは、生成される `AGENTS.md` がリポジトリの内容を 2 つの独立したセクションに列挙します。`Repo Skills` は検出されたスキルを列挙します。`Repo Commands` は、リポジトリのコマンドディレクトリ配下で見つかった実在の `SKILL.template.md` コマンドを、ディレクトリ名とリポジトリ相対パスとともに名前順で列挙します。同名のスキルとコマンドは、それぞれ別のセクションに現れます。ルートまたはテンプレートが存在しないコマンドは列挙されず、外部ソース由来のコマンドのパスを作り出すこともありません。

`AGENTS.md` を生成しても、使用する coding agent が読み込むとは限りません。`CLAUDE.md` などの指示ファイルがあると、ツールの読み込み設計によっては `AGENTS.md` が読み込まれない場合があります。対応するファイル名、ファイルの探索方法、読み込みの優先順位は、その coding agent のドキュメントを参照してください。詳しくは[セットアップガイド](setup.ja.md)を参照してください。

`gal init` および `gal render-adapters` コマンドは、1 つのアダプター根本ファイル（`AGENTS.md`）と、条件付き Rust ルールレイヤー（`.claude/rules/gal-rust.md` および `.github/instructions/gal-rust.instructions.md`）を生成します。これらのファイルは `crates/cli/src/gal/render.rs` によって生成されます。

```text
.dev/project.md
   │
   ▼
バイト数検証（最大 30,720 B、LF 正規化済み UTF-8）
   │  失敗時 → ファイルを変更せず処理を中止（ADR 06 参照）
   ▼
必須となる 8 つの H2 見出しの検証（各見出しが 1 回ずつ出現すること）
   │  欠落または重複時 → 違反見出しを報告して処理を拒否
   ▼
1 つのアダプター根本ファイルと、条件付き Rust レイヤーを生成（Rust 有効時）
   │
   ▼
書き込み前の事前検証を実行:
   ├─ 根本ファイルごとのファイルサイズ検証（最大 32,768 B）
   ├─ 所有権マーカーの検証および衝突チェック
   │  失敗時 → 部分的なファイル書き込みを行わず直ちに中止
   ▼
廃止された古い根本ファイルを削除し、空の親ディレクトリを整理
   │
   ▼
ファイルを書き込み、状態をレポート（Written、Unchanged、または Removed）
```

出力先パスは、`render.rs` 内で `REPO_ADAPTER_ROOTS`（`AGENTS.md` 用）および `REPO_ADAPTER_CONDITIONAL_LAYERS`（条件付きルールファイル用）という 2 つのリストとして一元管理されています。アダプターレンダリング処理、CLI の状態レポート、および finalize 時の冪等性チェック（`finalize_check.rs::check_sync_idempotency`）は、すべてこれらの定義を参照します。なお、冪等性チェックはディスクへの書き込みを行わず、メモリ上でレンダリング結果を評価します。

`render_and_apply_repo_adapters` 関数は、ファイル書き込みを行う前に出力候補を検証します。アダプター根本ファイルは、条件付きレイヤーよりも前に単一の不可分な処理として書き込まれます。すべての検証が事前チェックで行われるため、複数ファイルにまたがるロールバック機構はあえて設けていません。

同期ルーチンは、変更または削除されたパスを追跡する `ProjectionReport` を返します。レポート表示モジュールはこれを CLI 向けの要約フォーマット（`Written`、`Unchanged`、`Removed`）に整形して出力します。

## アダプター所有権の検証

`classify_root_ownership` 関数は、アダプターファイルの先頭行を検査します。ヘッダーが生成マーカー（`SlimRuntime::generated_marker()`）と一致する場合、そのファイルは `ManagedByGal`（GAL 管理下）としてマークされ、インプレースで更新されます。マーカーが存在しない場合、ファイルはユーザー保守（`HandOwned`）として分類され、GAL は一切の変更を加えません。条件付き Rust ルールファイルについては、`<!-- GAL-generated: gal init -->` マーカーの有無によって所有権を判定します。

既存のファイルに GAL の所有権マーカーが存在しない場合、意図しないデータ消失を防ぐため、ファイルを書き換える前に処理全体を直ちに中止します。設定を更新したい場合は、バージョン管理されたソースファイル（`.dev/project.md` または `plugins/gal-core/conventions/rust.md`）を編集した上で、`gal render-adapters` を再実行してください。

## 条件付き Rust ルールレイヤー

アダプターの根本ファイルに規約の全文を直接埋め込むことはしません。Rust プロジェクトの場合、GAL は各ランタイムの機能に応じて以下の 2 つのアプローチでルールを配布します。

1. **条件付きルールファイル**: ディレクトリベースのルール読み込みをサポートするランタイム（Claude Code や GitHub Copilot）向けには、専用のルールファイル（`.claude/rules/gal-rust.md` および `.github/instructions/gal-rust.instructions.md`）を生成します。
2. **ディレクティブポインター**: Codex、Antigravity、opencode 向けには、`AGENTS.md` の先頭に 1 行の指示文を挿入し、Rust 関連のタスクを開始する前に `plugins/gal-core/conventions/rust.md` を読み込むようモデルへ指示します。

## プロジェクションレジストリ（`plugins.lock.json`）

`~/.gal/state/plugins.lock.json` レジストリファイルは、投影されたすべての資産を追跡します。GAL が管理するのは最上位の `_galProjection` 名前空間のみであり、他の最上位キーを変更することはありません。書き込みごとに、投影された資産とそのソースとの対応関係のアトミックなスナップショットが保存されます。

| `_galProjection` のサブフィールド | 型 | 説明 |
| --- | --- | --- |
| `schemaVersion` | 数値 | レジストリスキーマのバージョン番号（例: `2`）。 |
| `agentProjectionPaths` | 配列 | 管理対象のエージェントファイルパス一覧。 |
| `commandProjectionPaths` | 配列 | 管理対象のコマンドファイルパス一覧。 |
| `skillProjectionPaths` | 配列 | 管理対象のスキルファイルパス一覧。 |
| `discussSkillProjectionPaths` | 配列 | 廃止されたメタデータ。古い GAL 管理の consult スキルをクリーンアップするときにのみ読み取られます。 |
| `codexAgentProjectionPaths` | 配列 | 管理対象の Codex 用エージェントパス一覧。 |
| `legacyProjectionPaths` | 配列 | 管理対象の旧式アダプターパス一覧。 |
| `sourceAttribution` | オブジェクト | 複数ソースの追跡を目的とした、ファイルパスとソース識別子の対応付け。 |
| `pluginOwned` | オブジェクト | ランタイム識別子（`claude`、`codex`、`copilot`、`agy`）とプラグイン分類キーの対応付け。 |

`pluginOwned` は今回の refresh の登録スナップショットから導出され、登録が確認されたために省略した分類だけを記録します。`Unknown` はプラグイン所有として記録しません。Claude はコマンド、Codex はコアスキル、Copilot はエージェント、Antigravity はコマンドスキルを所有します。Codex TOML エージェントとコマンド、Copilot コマンドスキル、Antigravity の正規プラグインリンクは引き続き投影します。OpenCode には常にネイティブコマンド、ネイティブエージェント、共有 `~/.agents/skills` を提供します。そのため OpenCode を選択すると、Codex の登録が確認済みでも共有スキルを投影します。

GAL は投影を開始する前に登録状態を一度だけ観察し、その不変スナップショットをスキル処理とコマンド処理で共有します。ランタイム登録を唯一のルーティング根拠とし、別個の `pluginMode` 設定を使用しないため、同じ refresh 内で矛盾する状態が混在しません。証拠が存在しない場合や読み取れない場合は、プラグイン所有が確認されていないフォールバックを削除するよりも GAL 管理の重複を残す方が安全であるため、投影フォールバックを選択します。

GAL は操作を `_galProjection` 名前空間内に厳格に制限します。外部のキーをパースしたり改変したりすることはありません。

## 正規 MCP マニフェスト（`.mcp.json`）

`~/.gal/plugins/gal/.mcp.json` に配置されるマニフェストは、`gal refresh` の実行時に `plugins/gal-core/mcp.json` と `~/.gal/local/mcp.json` の個人サーバーを Core-Wins 規則に基づいて統合することで生成されます。

| フィールド | 必須 | 型 | 説明 |
| --- | --- | --- | --- |
| `mcpServers` | 必須 | オブジェクト | `plugins/gal-core/mcp.json` のスキーマに準拠したサーバー定義。 |
| `inputs` | 任意 | 配列 | そのまま渡されるプロンプト駆動の入力定義。 |

既定では、`plugins/gal-core/mcp.json` のサーバーマップは空です。個人サーバーを持たないマシンの場合、生成されるマニフェストも空になります。将来的にコア機能で MCP サーバーが必要になった場合は、ソースファイルに宣言を追加するだけで自動的にマニフェストへ反映されます。

環境変数の記法（`${VAR}`）は加工されずにそのまま保持されます。これらの変数は、プロセスの起動時に MCP ホスト側で解決されます。GAL が MCP マニフェスト内に認証情報やシークレットを直接保存することはありません。

マニフェスト生成時、プロトコル仕様に準拠するため、ソースの `"servers"` キーは `"mcpServers"` へ改名されます。また、パッケージルートには Agent Plugins 1.0.0 仕様に準拠した `mcp.json` も配置されます。なお、`gal update` はバージョン情報を表示するのみであり、マニフェストの再生成は `gal refresh` を通じてのみ行われます。

## MCP 設定の境界

GAL は自身が生成するプラグインマニフェストのみを管理します。`claude_desktop_config.json`、`~/.copilot/mcp-config.json`、`~/.codex/config.toml`、`opencode.json`、`mcp_config.json` など、ホスト管理の設定ファイルを編集することは決してありません。ホスト側のツール設定はユーザー自身が管理します。

## 変更後の再生成手順

`plugins/gal-core/` 配下に加えた変更は、自動的にはホットリロードされません。変更したレイヤーに応じて適切な再生成コマンドを実行してください。

- **`plugins/gal-core/` の変更**（スキル、エージェント、コマンド、規約）: `gal refresh --source ./plugins/gal-core` を実行します。
- **`crates/` 配下の Rust コードの変更**: バイナリを再ビルドして `PATH` 上へ配置（`cargo build --release -p gal-cli`）した上で、`gal refresh --source ./plugins/gal-core` を実行します。

`gal refresh` を実行する前にバイナリを最新化しておくことが必須です。順序を誤ると、新機能に対応するバイナリサポートが存在せず、サブコマンドが失敗する恐れがあります。ジャンクションモードを使用している場合、Claude Code は次回の会話ターンで変更を即座に読み込みます。マーケットプレイス形式でインストールしている場合は、アプリケーションの再起動が必要です。

| 変更したコンポーネント | 実行が必要なコマンド | 確認手順 |
| --- | --- | --- |
| `plugins/gal-core/` のスキル、エージェント、コマンド、規約 | `gal refresh --source ./plugins/gal-core` | 対象のランタイムで更新されたコマンドを実行します。 |
| コアまたは個人の MCP 設定（`mcp.json`） | `gal refresh --source ./plugins/gal-core` | MCP ホスト側でサーバーを再読み込みします。 |
| `crates/` 配下の Rust ソースコード | バイナリを再ビルドして `PATH` へ配置 | `gal refresh --source ./plugins/gal-core` を実行します。 |
| `.dev/project.md` または Rust 規約 | ソースファイルを保存 | `gal render-adapters` を実行します。 |
| `~/.gal/config/config.json` | 設定ファイルを保存 | プロジェクションを実行することなく即時反映されます。 |

開発環境で `gal refresh` を実行する際は、必ず `--source ./plugins/gal-core` を明示してください。このフラグを省略すると、`gal refresh` は実行可能ファイルの隣からアセットを探そうとするため、`~/.cargo/bin` などから実行した際に失敗する場合があります。

`gal render-adapters`（リポジトリローカルのアダプターファイルを更新）と `gal refresh`（マシンレベルのエージェントプロジェクションを更新）の役割の違いにご注意ください。初期化済みのリポジトリをリセットしたい場合は、手動で `.dev/project.md` と `.dev/state.md` を削除してから `gal init` を再実行します。

### 診断による不整合検出（`gal doctor`）

`gal doctor` コマンドは、以下の 4 つの形態の設定ドリフトを検出します。

1. **スキルのプロジェクション乖離**: 正規ソースと内容が異なる投影済みスキルを検出します。
2. **opencode プロジェクションのドリフト**: `--dry-run` モードにおいて、正規のレンダリング結果と乖離している `~/.config/opencode/` 内のファイルを警告します。解決するには `gal refresh` を実行します。
3. **バイナリの不一致（Binary skew）**: 組み込みの `GAL_GIT_STAMP` と、ワークスペースの `git rev-parse --short HEAD` を比較します。不一致がある場合は、`crates/` を変更した後に `PATH` 上のバイナリが再コンパイルされていないことを示します。
4. **プラグイン登録の証拠**: `gal doctor` は投影と同じ登録オブザーバーを使用します。証拠が不明な場合、ソースと理由を含む警告を出します。登録の証拠はローカル設定を示すもので、実行中のランタイムがプラグインを読み込んだことを証明しません。既存の `pluginMode` 値は廃止キーの確認で報告されます。

新しい AI ランタイムの統合を検討する際は、以下の 4 つの設計基準に従います。

1. そのランタイムがマシンレベルの設定ディレクトリをサポートしているか確認する。
2. リポジトリ指示として `AGENTS.md` を直接利用できるか、それとも専用アダプターが必要かを判断する。
3. ネイティブコマンドがサポートされている場合は共有テンプレートからレンダリングし、非対応の場合はサポートされているスキルパスへコマンドスキルを投影する。
4. 設定ブリッジは、そのランタイムが既存設定を破壊しない安全な拡張機構を備えている場合にのみ実装する。

エージェントの改名やルーティング変更を行った場合、メンテナーは `gal refresh` を実行して最新の定義を投影し、さらに各ローカルリポジトリで `gal render-adapters` を実行してアダプターを同期させる必要があります。

## 検証チェックリスト

レンダリングやプロジェクションのロジックを変更した際は、以下の項目を確認してください。

- サポート対象のすべてのランタイムにおいて、有効なプロジェクション先ディレクトリが存在すること。
- レンダリングされた `commands/*/SKILL.md` 内に、未展開のテンプレート変数（例: `{{GAL_ROOT}}`）が残っていないこと。
- Google Antigravity のコマンドスキルが `~/.gemini/antigravity-cli/skills/<name>/SKILL.md` に正常に再生成されること。
- 共有スキルディレクトリに一般スキルが正しく配置され、重複エイリアスが存在しないこと。
- ホストレベルの MCP 設定ファイルが変更されていないこと。
- `gal doctor` のパス解決関数が、個別に `user_home()` を呼び出すのではなく、`DoctorPathContext::from_standard_paths` を通じて一度だけ解決されていること。

自動テストスイートは、`VALID_RUNTIMES` の全エントリについて、投影されたファイルと正規のビルド契約とのバイトレベルでの完全一致を検証しなければなりません。`AGENTS.md`、または投影されたスキルを手動で直接編集することは決して行わないでください。すべての更新は、ソース契約から `gal render-adapters` または `gal refresh` を通じて反映させる必要があります。
