---
source: docs/projection.md
lang: ja
source_commit: 8056cf0d53416d562b3f5546c0eb2aba11a01459
translated_at: 2026-09-23
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
2. **リポジトリレベルの生成（Repository-level generation）**: リポジトリの `.dev/project.md` をコンパイルし、`AGENTS.md` や `CLAUDE.md` などのリポジトリローカルなアダプターファイルを生成します。

プロジェクションパイプラインは、厳格な境界を持つ 3 つのモジュールに分割されています。

- **正規ルートのレンダリング（Canonical root rendering）**（`crates/gal-engine/src/render/`）: `plugins/gal-core/` のコア契約と、任意の `~/.gal/local/`（個人カスタマイズ）をコンパイルし、`~/.gal/plugins/gal/` の正規ルート（canonical root）を構築します。
- **ランタイム操作面へのプロジェクション（Runtime surface projection）**（`crates/projection/`）: 正規ルート内の成果物を、各対象ランタイムがサポートする形式へ変換し、それぞれの設定ディレクトリへ配布します。
- **リポジトリアダプターの生成（Repository adapter generation）**（`crates/cli/src/gal/render.rs`）: リポジトリ内の `.dev/project.md` をもとに `AGENTS.md` および `CLAUDE.md` を生成します。この処理は前述の 2 つのモジュールを呼び出すことなく完全に独立して動作します。

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

各ランタイムへ配布されるコマンドやスキルの内容は、事前にレンダリングされた正規ルートの内容とバイト単位で完全に一致（byte-identical）していなければなりません。環境間で生じる差異として許容されるのは、配信機構（シンボリックリンク、ジャンクション、物理コピー）、サンドボックスの権限設定、および呼び出しの起動構文のみであり、コマンドのプロンプト本文自体はすべてのランタイムで不変です。

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

## エージェントプロジェクションマトリクス

GAL は `*.agent.md` のロール定義を単純にコピーするのではなく、各ランタイムのネイティブなエージェントまたはスキルの形式へ適切に変換します。

| ランタイム | 対象形式 | プロジェクション先 | 変換機構 |
| --- | --- | --- | --- |
| Claude Code | GAL ヘッダー付き `*.agent.md` | `~/.gal/plugins/gal/agents/` | Claude のサブエージェントへ直接マッピング。 |
| OpenAI Codex | ネストされたテーブルのないフラットな `*.toml` | `~/.codex/agents/<name>.toml` | Codex TOML サブエージェント（`crates/projection/src/codex_agent.rs`）。 |
| Google Antigravity | 投影スキル | `~/.gemini/antigravity-cli/skills/` | スキルディレクトリ経由でマウント。ネイティブエージェントファイルは未検証。 |
| opencode | `mode: subagent` を持つ `<name>.md` | `~/.config/opencode/agents/<name>.md` | `render_opencode_agent` により生成。 |
| GitHub Copilot | Claude ツール名にマップした `*.agent.md` | `~/.copilot/agents/<name>.agent.md` | `pluginMode.copilot` が `false` の場合にフォールバックファイルを生成。`true` の場合は削除。 |

`~/.gal/config/config.json` 内の `pluginMode.copilot` 設定は、Copilot 向けフォールバック生成の有無を決定します。これを `true` に設定すると、ネイティブなプラグイン登録が完了しているとみなされ、GAL はフォールバックエージェントファイルを削除してコマンドスキルのみを維持します。

Codex のエージェントファイルは、ネストのないフラットな TOML 構造を採用しています。シリアライザー `serialize_codex_toml` は、ドキュメントルートに 6 つのフィールド（`name`、`description`、`developer_instructions`、`model`、`model_reasoning_effort`、`sandbox_mode`）を定義し、サブテーブルを使用しません。Codex は未認識のキーや旧式のネストキーを含むファイルを拒否するため、フラットなスキーマを厳格に適用することで、`codex doctor` チェック時にスキーマの乖離を即座に検知できるようにしています。

### ロール実行モード

GAL のエージェントロールは、以下の 2 つの実行モードをサポートしています。

- **隔離モード（Isolated Mode）**（`/gal architect`）: 専用のサブエージェントを起動してタスクを実行し、メインの会話には最終的な裁定要約のみを返します。
- **コンテキスト内モード（In-Context Mode）**（`/gal discuss architect`）: ロールの規律や開発指示を現在のセッションへ直接読み込み、マルチターンでの対話的な相談を可能にします。

Codex は動的なロール注入機能をネイティブに備えていないため、`gal refresh` は 4 つの計画レビューロール（`architect`、`analyst`、`designer`、`releaser`）向けに、`~/.agents/skills/` 配下へ補足的な `discuss-<role>/SKILL.md` スキルを生成します。これらのスキルを呼び出すことで、サブエージェントを起動することなく現在のスレッドにロールの指示を直接ロードできます。これらのディレクトリは、クリーンアップ処理による誤削除を防ぐため予約ディレクトリ一覧に登録されています。

## オーケストレーター駆動役割の除外

パイプラインの 4 つのロール（`implementer`、`tester`、`auditor`、`researcher`）について、`crates/projection/src/lib.rs` の `update_agents` がプロジェクションファイル（`*.agent.md` や `*.toml`）を生成することは**厳格に禁止**されています。これらのロールはパイプライン解決のために内部の `KNOWN_GOLEMS` レジストリに登録されていますが、ユーザー向けの直接呼び出しファイルは持ちません。これにより、監視の行き届かない単独実行を防止します。例えば `/gal tester` を直接実行しても未定義コマンドのエラーとなり、必ずオーケストレーターの統制下でのみ実行されるよう保証されます。

各ロールの定義は抽象的な権限スコープ（`read`、`edit`、`execute`、`search`、`web` など）を宣言します。`crates/projection/src/tool_map.rs` モジュールは、これらの抽象権限を各ランタイム固有のツール識別子へマッピングします。Codex では、サンドボックスの分離レベルがこれに応じて対応付けられます。計画レビューロールは `read-only` モードで実行され、パイプライン実装ロールは `workspace-write` モードで実行されます。

## リポジトリアダプター生成

`gal init` および `gal render-adapters` コマンドは、2 つのアダプター根本ファイル（`AGENTS.md` および `CLAUDE.md`）と、条件付き Rust ルールレイヤー（`.claude/rules/gal-rust.md` および `.github/instructions/gal-rust.instructions.md`）を生成します。これらのファイルは `crates/cli/src/gal/render.rs` によって生成されます。

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
2 つのアダプター根本ファイルと、条件付き Rust レイヤーを生成（Rust 有効時）
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

出力先パスは、`render.rs` 内で `REPO_ADAPTER_ROOTS`（`AGENTS.md`、`CLAUDE.md` 用）および `REPO_ADAPTER_CONDITIONAL_LAYERS`（条件付きルールファイル用）という 2 つのリストとして一元管理されています。アダプターレンダリング処理、CLI の状態レポート、および finalize 時の冪等性チェック（`finalize_check.rs::check_sync_idempotency`）は、すべてこれらの定義を参照します。なお、冪等性チェックはディスクへの書き込みを行わず、メモリ上でレンダリング結果を評価します。

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

`~/.gal/state/plugins.lock.json` レジストリファイルは、投影されたすべての資産を追跡します。GAL が管理するのは最上位の `_galProjection` 名前空間のみであり、他の最上位キーを変更することはありません。書き込みごとに、投影された資産とそのソースマッピングのアトミックなスナップショットが保存されます。

| `_galProjection` のサブフィールド | 型 | 説明 |
| --- | --- | --- |
| `schemaVersion` | 数値 | レジストリスキーマのバージョン番号（例: `2`）。 |
| `agentProjectionPaths` | 配列 | 管理対象のエージェントファイルパス一覧。 |
| `commandProjectionPaths` | 配列 | 管理対象のコマンドファイルパス一覧。 |
| `skillProjectionPaths` | 配列 | 管理対象のスキルファイルパス一覧。 |
| `discussSkillProjectionPaths` | 配列 | 管理対象の Codex 用 discuss スキルパス一覧。 |
| `codexAgentProjectionPaths` | 配列 | 管理対象の Codex 用エージェントパス一覧。 |
| `legacyProjectionPaths` | 配列 | 管理対象の旧式アダプターパス一覧。 |
| `sourceAttribution` | オブジェクト | 複数ソースの追跡を目的とした、ファイルパスとソース識別子のマッピング。 |
| `pluginOwned` | オブジェクト | ランタイム識別子（`claude`、`codex`、`copilot`、`agy`）とプラグイン分類キーのマッピング。 |

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

`gal doctor` コマンドは、以下の 3 つの形態の設定ドリフトを検出します。

1. **スキルのプロジェクション乖離**: 正規ソースと内容が異なる投影済みスキルを検出します。
2. **opencode プロジェクションのドリフト**: `--dry-run` モードにおいて、正規のレンダリング結果と乖離している `~/.config/opencode/` 内のファイルを警告します。解決するには `gal refresh` を実行します。
3. **バイナリの不一致（Binary skew）**: 組み込みの `GAL_GIT_STAMP` と、ワークスペースの `git rev-parse --short HEAD` を比較します。不一致がある場合は、`crates/` を変更した後に `PATH` 上のバイナリが再コンパイルされていないことを示します。

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

自動テストスイートは、`VALID_RUNTIMES` の全エントリについて、投影されたファイルと正規のビルド契約とのバイトレベルでの完全一致を検証しなければなりません。`AGENTS.md`、`CLAUDE.md`、または投影されたスキルを手動で直接編集することは決して行わないでください。すべての更新は、ソース契約から `gal render-adapters` または `gal refresh` を通じて反映させる必要があります。
