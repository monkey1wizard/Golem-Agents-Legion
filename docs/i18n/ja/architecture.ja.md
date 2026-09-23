---
source: docs/architecture.md
lang: ja
source_commit: 6b1f407daaa9e816a2203bcb91ebd7856c0d33ad
translated_at: 2026-09-23
type: Architecture
title: GAL アーキテクチャ
description: GAL のアーキテクチャ制約、クレートとレイヤーの境界、ストレージトポロジー、移行経路、ドキュメント統制、ADR 索引を詳述します。
tags:
  - architecture
  - boundaries
  - crates
  - governance
  - arc42
status: stable
---

# アーキテクチャ

[English](../../architecture.md) · **日本語** · [繁體中文](../zh-Hant/architecture.zh-Hant.md)

## 技術ベースライン

GAL は Rust で構築されたクロスランタイム開発ワークフローエンジンです。複数の AI プロバイダーにまたがり、統一されたコントロールプレーンと標準化されたワークフロー契約を維持します。本システムの技術基盤は、ソースベースライン、デプロイベースライン、および常設のアーキテクチャ制約から構成されます。

### ソースベースライン

- **Rust ワークスペース**: 7 つのクレート（`gal-foundation`、`mcp`、`dispatch`、`pipeline`、`projection`、`gal-engine`、`cli`）で構成される Cargo ワークスペースであり、単一の統合バイナリ `gal` へコンパイルされます。
- **外部データベース非依存**: リレーショナルデータベースやドキュメントデータベースを一切使用しません。すべての状態は、リポジトリ内およびローカルファイルシステム上のプレーンテキストファイルに永続化されます。
- **バージョン管理されるソース契約**: ワークフロー契約、コマンド仕様、エージェントロール定義、およびテンプレートは、`plugins/gal-core/` 配下の Markdown ファイルとして Git で追跡・管理されます。
- **英語正本ベースライン**: リポジトリメタデータの `PROJECT_LANGUAGE` 設定に基づき、契約および主要ドキュメントの権威ある情報源は英語（`en`）と定められています。英語以外の計画成果物は、決定論的ハッシュと等価性スタンプ（equivalence stamp）によって、英語の実行プロンプトとの意味的等価性を維持します。

### デプロイベースライン

- **事前コンパイル済みクロスプラットフォームターゲット**: リリースパイプラインは、`x86_64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`、`aarch64-unknown-linux-gnu`、`x86_64-apple-darwin`、`aarch64-apple-darwin`、`x86_64-pc-windows-msvc` の 6 つのターゲット環境向けにビルド済みバイナリを生成します。
- **サプライチェーン検証**: リリースアーカイブには SHA-256 チェックサム（`checksums.txt`）および Cosign のキーレス署名（keyless cosign signature）が付属します。インストールスクリプトはアーカイブ展開前にチェックサムと署名の検証を行います。
- **パッケージマネージャーによる配布**: 公式シェルスクリプト（`install.sh`、`install.ps1`）、Homebrew Tap、WinGet、または `cargo install --git` を使用してインストールします。旧式のセルフインストールコマンド（例: `crates/setup`）は廃止されました。
- **Windows 上の静的 CRT リンク**: Windows リリースバイナリは C ランタイム（CRT）を静的リンクしており、Visual C++ 再頒布可能パッケージが導入されていないクリーンな環境でもそのまま動作します。

### 常設のアーキテクチャ制約

- **クロスランタイムでの同等性**: Claude Code、OpenAI Codex、GitHub Copilot、Google Antigravity、opencode においてコントロールプレーンコマンドの挙動を完全に一致させ、ツール間の動作乖離を防止します。
- **リポジトリ所有の状態管理**: `.dev/` ディレクトリ、計画ファイル、調査ドキュメントが、セッション継続性の唯一の情報源（Single Source of Truth）となります。揮発性のランタイムキャッシュがリポジトリ側のファイルに優先することは決してありません。
- **テンプレート駆動のアダプター生成**: リポジトリアダプターファイルは、ソーステンプレートから `gal` バイナリによって自動生成されます。生成されたアダプターファイルを手動で直接編集することは禁止されています。
- **標準的な Git 履歴の維持**: リポジトリ操作には標準の `git merge`、`git commit`、`git branch` を使用します。コミットのスカッシュ（squash）、リベース（rebase）、`--ff-only`、`--no-ff` の使用は禁止されています。並行タスクは履歴を書き換えるのではなく、独立した Git ワークツリー（worktree）上で実行します。
- **スコープの完全性**: プロジェクト所有者が開発目標とスコープ境界を定めた後、エージェントが一方的にスコープを縮小したり、未承認のフェーズへ機能を分割したり、勝手な KEEP リスト（残存リスト）を導入したりすることは禁止されています。
- **厳格なトークンおよびファイル予算**: プロジェクトインデックス `.dev/project.md` は最大 30,720 バイト、`.dev/state.md` は最大 16,384 バイトに制限されます。また、アダプターファイル内のワークフロー遵守マーカー（workflow obedience marker）は、Codex の既定の `project_doc_max_bytes` の境界であるバイトオフセット 32,768 より前に配置し、部分読み取りが行われた場合でも重要な制約が確実に読み込まれるようにします。

## ディレクトリ境界と依存関係の法則

コードベースは厳格なモジュール分離と、下位方向のみを指す一方向の依存階層を徹底し、循環参照を防止します。

### クレート依存グラフ

クレート間の依存関係は、厳密に下位方向のみを指す有向非巡回グラフ（DAG）を形成します。

```text
gal-foundation  （基盤レイヤー: 内部 gal クレートへの依存ゼロ）
dispatch        （gal-foundation のみに直接依存）

dispatch   ─> gal-foundation
mcp        ─> gal-foundation
projection ─> gal-foundation
pipeline   ─> dispatch
gal-engine ─> gal-foundation, projection
cli        ─> gal-foundation, mcp, projection, dispatch, pipeline, gal-engine（バイナリエントリーポイント）
```

### クレート責務マトリクス（禁止境界）

| クレート | 責務 | 禁止される操作 |
| --- | --- | --- |
| `gal-foundation` | 設定構造、パスユーティリティ、OS 抽象化、ランタイムレジストリ（`VALID_RUNTIMES`）、MCP スキーマ、`HealthCheck` トレイト、`secret_re` などの共有基盤。 | 他の GAL クレートへの依存、および CLI コマンドの実装。 |
| `mcp` | MCP 成果物の管理: 正規 `.mcp.json` のヘルスチェックなど。 | `claude_desktop_config.json` など、ホスト管理の設定ファイルの検査、編集、書き込み。 |
| `dispatch` | ヘッドレス実行: ロールルーティング（`executorRouting`）、プロセスタイムアウト、write-back 検証、SSH 実行レーン。 | `pipeline` が担当する複数フェーズのワークフローオーケストレーション、および `gal-foundation` 以外のクレートへの依存。 |
| `pipeline` | ワークフローオーケストレーション: タスク分解、複数フェーズのディスパッチ、タスク仕様の組み立て。 | OS のプロセス生成 API の直接呼び出し、およびランタイムファイルのプロジェクション。 |
| `projection` | ファイルプロジェクションエンジン: スキル、コマンド、指示文、エージェントを各ランタイム操作面へ配布。 | リポジトリレベルのアダプター（`AGENTS.md`、`CLAUDE.md`）の生成、およびヘッドレスエグゼキュータープロセスの管理。 |
| `gal-engine` | ワークフロー CLI プリミティブ: 正規ルートのレンダリング（`~/.gal/plugins/gal/`）、CLI 型定義、doctor チェック、Git フィルターフック、翻訳検証。 | 外部ツールのパスへの直接プロジェクション、および `dispatch` や `pipeline` の直接呼び出し。 |
| `cli` | 実行可能エントリーポイント: 引数解析、`gal doctor` の集約、アダプター生成（`render.rs`）、内部検証ゲート。 | 事前検証（preflight）の回避、および境界を越えたクレート内部構造への直接アクセス。 |

### リポジトリ構造と保護対象パス（Protected Paths）

```text
Golem-Agents-Legion/
├── plugins/
│   └── gal-core/                正規のソース契約群
│       ├── commands/            公開 /gal コマンドテンプレート [Protected]
│       ├── conventions/         共有ルールおよびエンジニアリング規約 [Protected]
│       ├── workflows/           ワークフロー契約（coding, doc-sync, research） [Protected]
│       ├── templates/           リポジトリ状態テンプレート [Protected]
│       ├── agents/              Golem エージェント契約（*.agent.md）
│       ├── skills/              再利用可能スキル（skills/<name>/SKILL.md）
│       ├── mcp.json             バージョン管理される MCP マニフェスト
│       └── opencode.json        バンドルされた opencode 設定
├── crates/                      Rust ワークスペース（7 クレート） [Protected*]
├── packaging/                   インストールスクリプトおよびミラーエクスポート
├── docs/                        ドキュメント、ガイド、アーキテクチャ決定記録（docs/adr/）
└── .dev/                        リポジトリの運用状態（project.md, state.md, plans/, research/）
```

\* 保護対象パス（Protected Paths）には、`crates/projection/`、`crates/gal-engine/src/render/`、および `plugins/gal-core/` 内のコア契約が含まれます。保護対象パスへの変更は重大なアーキテクチャ変更とみなされ、実装前にレビュー・承認された計画が必須となります。

### コードベース所有権マトリクス

| 成果物の種類 | 保存場所 | 所有コンポーネント |
| --- | --- | --- |
| 方法論およびコアルール | `plugins/gal-core/` | ソース契約（`commands/`、`conventions/`、`workflows/`、`templates/`、`agents/`）。 |
| プロジェクトの運用コンテキスト | `.dev/project.md` および `.dev/state.md` | 対象リポジトリのローカル状態。`gal init` およびワークフローコマンドによって維持。 |
| 人間が読める機能計画 | `.dev/plans/<slug>.md` | 計画コマンドおよびアーキテクトレビューを通じて共同作成される計画成果物。 |
| モデル実行用プロンプト | `.dev/plans/<slug>.prompt.md` | `/gal pipeline` の作業メモリとして機能する自動生成された実行プロンプト。 |
| セッション引き継ぎメモ | プロンプト内および `.dev/state.md` の `### Handoff Notes` | finalize 処理中に恒久的ドキュメントへ統合される一時的なメモ。 |
| マシンローカルな実行ルーティング | `~/.gal/config/config.json#executorRouting` | `sshTarget` や `remoteWorkdir` を含む、ユーザー管理のマシン設定。 |

## データ変換パイプライン

ビルドパイプラインは、Git で追跡されるソース契約群を、各ランタイムのプロジェクション操作面およびリポジトリローカルのアダプターファイルへ変換します。

### アーキテクチャ統制

本ドキュメントはモジュールの所有権とデータフローを規定します。ソース契約は、リポジトリアダプター生成器と正規ルートレンダラーへ別々に渡されます。その後、プロジェクションエンジンがコンパイル済みのアセットを各ランタイムへ配布します。ファイルレイアウト、パス解決、トランスポート、プロジェクションレジストリに関する技術的な詳細は [プロジェクション](projection.ja.md) で定義されています。また、ユーザーが設定可能な設定境界は [設定](configuration.ja.md) で定義されています。

## コンポーネントモデル

内部コンポーネントは単一責任の原則に従って分離され、曖昧さのない統一された用語を使用します。

### 用語規則

使用する用語は、[`docs/glossary.md`](../../glossary.md) の権威ある用語集に厳格に従います。

- ファイル配布エンジンには、曖昧な「adapters」という呼称ではなく `projection` を使用します。
- 外部ランタイム向けのロジックは、厳格に `projection` クレート内にカプセル化します。
- ヘッドレス CLI ランナーには、過去の `dispatch::adapters` に代えて `executors` を使用します。
- 非推奨となったコンポーネントは、未使用のスタブを残すことなく完全に削除します（例: `crates/xmachine` の完全削除）。
- 一時的なタスク識別子（`T-NN`、`R-NN`）や移行経緯の叙述を、恒久的なドキュメント内に残してはいけません。

### アダプター生成とプロジェクションの分離

リポジトリアダプター生成、正規プラグインレンダリング、ランタイム操作面へのプロジェクションは、互いに独立した 3 つのモジュールで実行されます。

1. **リポジトリアダプター生成**（`crates/cli/src/gal/render.rs`）: `.dev/project.md` の必須 8 セクションを抽出し、`AGENTS.md` および `CLAUDE.md` を生成します。`gal-engine` や `projection` を呼び出すことはありません。
2. **正規プラグインレンダリング**（`crates/gal-engine/src/render/`）: `plugins/gal-core/` と `~/.gal/local/` を統合し、`~/.gal/plugins/gal/` を構築します。
3. **ランタイム操作面へのプロジェクション**（`crates/projection/`）: 正規ルートから `~/.agents/skills/` や `~/.codex/agents/` などのツールディレクトリへ成果物を投影します。

### Golem エージェントロールの分類

エージェントペルソナは、その呼び出し能力に基づいて以下の 2 つのクラスに大別されます。

- **クラス 1: 直接呼び出しおよび対話相談が可能なロール**
  - `golem-architect`、`golem-analyst`、`golem-designer`、`golem-releaser`、`golem-debugger`、`golem-steward` の 6 ロール。
  - `/gal <role>` で直接呼び出すことができ、隔離されたサンドボックス内で評価を実行して構造化された要約を返します。
  - レビュー系ロールは `/gal discuss <role>` による対話相談にも対応しており、ロールの指示文をアクティブセッションへ直接注入して会話を継続できます。
- **クラス 2: オーケストレーター駆動のロール**
  - `golem-implementer`、`golem-tester`、`golem-auditor`、`golem-researcher` の 4 ロール。
  - **プロジェクション制約**: プロジェクションエンジン（`crates/projection/src/lib.rs` の `update_agents`）が、これら 4 ロールのプロジェクションファイル（`*.agent.md` または `*.toml`）を作成することは厳格に禁止されています。
  - **安全な fail-closed 設計**: `/gal tester` や `$tester` などを直接実行しても未定義コマンドエラーとなります。これらのロールは、`/gal pipeline` などのオーケストレーターから起動されるヘッドレスなサブプロセスとしてのみ実行されます。

## システム境界と統合

GAL は、明確に定義された境界を介して各ツールランタイムと連携します。本ドキュメントでは契約、正規レンダラー、プロジェクション、リポジトリアダプター間の構造的分離を定義しています。具体的なトランスポート機構とファイルパスは [プロジェクション](projection.ja.md) を、設定スキーマは [設定](configuration.ja.md) を、オプションの外部統合は [統合](integrations.ja.md) をそれぞれ参照してください。

## 進化と移行のポリシー

アーキテクチャ上のガードレールにより、後方互換性の維持と安全なアップグレードを実現します。

### 互換性維持のパターン

- **Serde フィールドエイリアス**: 設定キーや内部属性の名前を変更する際は、`#[serde(alias = "...")]` 属性を付与して、既存の `config.json` や `plugins.lock.json` との互換性を維持します。
- **安全なディレクトリクリーンアップ**: 古いディレクトリは、明示的な GAL 所有権マーカーが存在し、かつ空である場合にのみ削除されます。ユーザーファイルが含まれている場合は処理を中断しない警告を出力し、ディレクトリを安全に保持します。
- **Golden テストの同期更新**: テンプレートの出力形式や契約フォーマットを意図的に変更した場合は、対応するテストスイートおよびゴールデンテストのフィクスチャを、変更理由のドキュメントとともに同一コミット内で更新しなければなりません。

### エスカレーション基準

以下の作業が必要となった場合は、直ちに実装を中断し、`/deep-planning` を通じてアーキテクトレビューを要求してください。

- ルート設定ファイル（例: `Cargo.toml`、`package.json`）の新規作成または削除。
- 外部依存クレートの追加または削除。
- アーキテクチャレイヤー境界をまたぐファイルの移動。
- 新たなグローバルインターフェースや抽象基本型の導入。
- 複数の利用者が参照する公開 API シグネチャまたは共有モジュールインターフェースの変更。
- 3 つ以上のコンポーネントで共有される中核的な抽象概念の変更。
- 保護対象パス（Protected Paths）内のファイルの編集。

### 恒久的ナレッジコア

以下の成果物はリポジトリの恒久的なナレッジベースを構成するものであり、セッションをまたいで確実に維持されなければなりません。

- **ソース契約**: `plugins/gal-core/` 配下のコマンド、規約、ワークフロー、およびテンプレート。
- **用語の権威**: [`docs/glossary.md`](../../glossary.md) の正規用語集、ならびに [`docs/i18n/zh-Hant/terminology.zh-Hant.md`](../zh-Hant/terminology.zh-Hant.md) および [`terminology.ja.md`](terminology.ja.md) のロケールプロファイル。
- **公式ドキュメントおよび ADR**: `docs/` 配下のトピックガイド、および `docs/adr/` 配下のアーキテクチャ決定記録。
- **プロジェクト仕様**: 高密度なプロジェクトメタデータを保持する `.dev/project.md` マニフェスト。

## ドキュメント統制

### 恒久ナレッジの境界

恒久的なナレッジベースが配置される場所は、ルートの 3 ドキュメント（`README.md`、`SECURITY.md`、`CONTRIBUTING.md`）および正規の `docs/` ディレクトリの 2 箇所に厳格に限定されます。

`.dev/plans/`（一時的な機能計画）や `.dev/research/`（探索的な調査メモ）などのディレクトリは、恒久ナレッジベースの外部に位置付けられます。開発中に判明した知見は、`docs/` へ統合されるか、または計画内で正式化された場合にのみ恒久化されます。`.dev/project.md` ファイルは、孤立した事実を無秩序に蓄積するのではなく、`docs/` を参照する圧縮された索引として機能します。

### 句読点ポリシー: セミコロンの禁止

本リポジトリのすべてのドキュメントでは、言語を問わず半角および全角のセミコロン（`;` / `；`）の使用を禁止しています。セミコロンは複数の思考を単一の文に詰め込む傾向を生みます。執筆者は、独立した思考を別々の文に分割するか、適切な接続詞を用いて文構造を明確にしてください。セミコロンで区切られた一覧は、読点区切りまたは箇条書きとして表現します。

このポリシーは可読性と論理構造の明確さを最優先するためのものであり、本文、表のセル、図、コードコメントに適用されます。なお、プログラミング言語の構文上不可欠なセミコロンは例外として認められます。

### ファイル命名規約

- `README.md` はリポジトリルート専用のファイル名です。サブディレクトリで目次ドキュメントが必要な場合は、厳選された概要には `guide.md` を、構造化された一覧には `index.md` を使用します。
- `docs/integrations.md` 内のツールセクションには正式な製品名を使用します。MCP サーバー専用のセクションにのみ `MCP` サフィックスを付与します（例: 「Playwright MCP」）。
- 特定のスキルに対応するドキュメントは、そのスキルと同一の名前を使用します。例えば `opencli-research` スキルは、`docs/integrations.md` 内の OpenCLI セクションに対応付けられます。
- 関連のないドキュメント同士で同一のファイルベース名を共有してはいけません。

### 多言語翻訳アーキテクチャ

正本となるドキュメントは英語で記述され、標準のファイルパスに配置されます。翻訳されたファイルは `docs/i18n/<lang>/` 配下に `<name>.<lang>.md` の形式で配置されます。ルートドキュメントも同様の規約に従います（例: `README.md` は `docs/i18n/zh-Hant/README.zh-Hant.md` に対応）。

翻訳の対象は、`README`、`SECURITY`、`CONTRIBUTING`、`architecture`、`configuration`、`workflows`、`projection`、`integrations`、`setup` という閉じた公式ドキュメント群に限定されています。`docs/adr/` 配下の決定記録は英語専用であり、翻訳ツリーは持ちません。また、英語の命名権威は全文翻訳ではなく、各ロケールの `terminology.<lang>.md` 表示プロファイルを通じて表現されます。

翻訳で変更されるのは地の文（prose）のみです。コマンド名、コード、パス、設定キー、識別子は一切翻訳しません。相対リンクは各言語ディレクトリの階層深度に合わせて再計算されます。

### 翻訳鮮度の追跡

翻訳ファイルには、Open Knowledge Format メタデータに加えて 4 つの由来キー（provenance key）が付与されます。

```yaml
---
source: README.md          # 英語正本への相対パス
lang: ja
source_commit: <hash>      # 翻訳元となった英語正本のコミットハッシュ（ドラフト時は PENDING）
translated_at: 2026-06-02  # 翻訳を更新した日付
---
```

翻訳の鮮度は `status` ではなく `source_commit` に基づいて評価されます。`status` フィールドはドキュメントのライフサイクル成熟度（`draft`、`stable`、`deprecated`）を表すものであり、翻訳の鮮度を示す目的で使用してはいけません。

`gal translation-freshness` コマンドは、`source_commit` と最新の Git コミット履歴を比較し、各ファイルを `current`、`stale`、`missing`、`unexpected` のいずれかとしてレポートします。新しく作成されたドラフト翻訳は、英語正本の変更がマージされるまで `source_commit: PENDING` として記録されます。

### Front Matter メタデータ仕様

管理対象のドキュメントは、以下の 5 つのキーを含む Open Knowledge Format（OKF）の front matter から始まります。

- `type`: ドキュメント種別（`Guide`、`Reference`、`Architecture`、`ADR`、`Policy`）。
- `title`: ドキュメントのタイトル。
- `description`: `README.md` のドキュメントマップ表と完全に一致する 1 文の要約。
- `tags`: 分類タグの一覧。
- `status`: ライフサイクルの成熟度（`draft`、`stable`、`deprecated`）。

翻訳ドキュメントでは、これら 5 つのキーに 4 つの翻訳由来キーが加わり、合計 9 つの front matter フィールドで構成されます。

## アーキテクチャ決定記録（ADR）索引

本ドキュメントは、ISO/IEC/IEEE 42010 および arc42 のアーキテクチャ記述標準に準拠しています。中核となるアーキテクチャ解説の焦点を明確に保つため、設計の歴史的経緯や却下された代替案は `docs/adr/` 配下の個別 ADR ファイルに記録・保管されています。

- [01: Projection, Source Model, and Adapters](../../adr/01-projection-and-source-model.md)
- [02: Scope and Ownership Boundaries](../../adr/02-scope-and-ownership-boundaries.md)
- [03: Machine Lifecycle and Binary Identity](../../adr/03-machine-lifecycle-and-binary-identity.md)
- [04: Dispatch Architecture](../../adr/04-dispatch-architecture.md)
- [05: Gates, Evidence, and Enforcement](../../adr/05-gates-evidence-and-enforcement.md)
- [06: Documentation and State Governance](../../adr/06-documentation-and-state-governance.md)
- [07: Release](../../adr/07-release.md)
