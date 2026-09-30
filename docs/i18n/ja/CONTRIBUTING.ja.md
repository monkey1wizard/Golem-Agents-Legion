---
source: CONTRIBUTING.md
lang: ja
source_commit: b31b1130a00351b7876515572feb79b5fc6f4ff7
translated_at: 2026-09-23
type: Policy
title: GAL へのコントリビューション
description: GAL のコントリビューション指針です。ブランチ規約、コミット標準、ビルドとテストのゲート、レビューチェックリストを扱います。
tags:
  - contributing
  - commits
  - gates
  - review
status: stable
---

# GAL へのコントリビューション

GAL は Rust ワークスペースと、ドキュメント駆動のワークフロー契約群によって構成されています。コードの変更に着手する前に、まず [アーキテクチャ](architecture.ja.md) を通読してシステムの全体構造とレイヤー境界を把握してください。すべての変更は、後述の検証ゲートを通じて確認する必要があります。なお、準拠すべき中核のワークフロー契約は [`coding.md`](../../../plugins/gal-core/workflows/coding.md) です。

## ブランチとコミット

- `main` からフィーチャーブランチ（feature branch）を作成し、Pull Request を提出してください。なお、リポジトリの運用慣例として、ソロ開発における計画ドキュメントの更新に限り、`main` への直接コミットが認められています。
- コミットメッセージには、`feat(gal-engine): ...`、`refactor(projection): ...`、`docs(...): ...` のようにスコープを指定した Conventional Commits 形式を使用してください。
- 各コミットは、単一かつ独立して revert（取り消し）可能な論理単位の変更にとどめてください。
- pre-commit フックは、命名規則ゲート（naming gate）、`.dev/` コミットポリシー、および個人環境パスの漏洩チェックを検証します。pre-commit フック実行時に `gal` コマンドが `PATH` から呼び出せる状態であることを確認してください。

## ビルドとテスト

ローカル環境で以下の検証スイート全体を実行します。

```text
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
cargo fmt --check
gal naming-gate
gal render-adapters
```

すべての検証を警告なくクリーンにパス（clean pass）させる必要があります。`gal render-adapters` を連続して実行した際、差分（diff）が空でなければなりません。

単体テスト（unit test）は対象ソースファイル末尾の `mod tests` ブロック内に配置し、統合テスト（integration test）は `crates/<crate>/tests/` 配下に配置します。`crates/gal-engine/tests/*.rs` 配下のテストファイル名には、`install`、`setup`、`update`、`patch` という単語を含めないでください。Windows 環境では、これらのキーワードを含む実行可能ファイルが UAC（ユーザーアカウント制御）の昇格ヒューリスティックに抵触し、`asInvoker` マニフェストが存在しない場合にプロセス起動エラー（エラーコード 740）を引き起こすためです。

`plugins/gal-core/` 配下のファイルを編集した場合は、`gal refresh --source ./plugins/gal-core` を実行してください。`crates/` 配下のコードを変更した場合は、プロジェクトを再コンパイルして新しいバイナリを `PATH` に配置した上で、再度 refresh コマンドを実行します。プロジェクションのライフサイクルに関する詳細は [プロジェクション](projection.ja.md) を参照してください。

## コントリビューターゲート

小規模な修正を除くすべての変更は、標準のライフサイクル（計画策定、必要に応じたディーププランニング、計画の洗練、人間によるレビューと承認、プロンプト生成、実装、テスト、監査、着地）の順序に従って進める必要があります。保護対象パス（Protected Paths）に触れる作業を行う場合は、事前にレビュー・承認済みの計画が必須となります。権威ある保護対象パスの一覧は `.dev/project.md` で管理されています。

レビューを依頼する前に、以下の点を確認してください。

- クレート間の境界と依存関係のルールを厳格に遵守していること。
- 公開 CLI の操作体系が、サポート対象のすべてのランタイムで完全に一致していること。
- 生成されたアダプターが `plugins/gal-core/` 内のソーステンプレートと完全に同期していること。
- 新規追加または変更されたテストがすべて実行され、パスしていること。スキップ（`NotRun`）されたテストをパスと誤認しないでください。
- ドキュメントが現在の挙動を正しく反映しており、権威ファイルで定義されたルールを重複して記述していないこと。
- 識別子や専門用語が [`docs/glossary.md`](../../glossary.md) の正規定義に厳密に従っていること。

## 規約とワークフロー契約

コードを記述する前に、作業内容に関連する契約ドキュメントを確認してください。これらの文書は、システムの挙動に関する公式な基準となります。

| 契約 | 確認するタイミング |
| --- | --- |
| [`workflows/coding.md`](../../../plugins/gal-core/workflows/coding.md) | 実装開始前。準拠必須のコーディングワークフロー。 |
| [`workflows/doc-sync.md`](../../../plugins/gal-core/workflows/doc-sync.md) | コード変更によるドキュメントへの影響を分析するとき。 |
| [`workflows/research.md`](../../../plugins/gal-core/workflows/research.md) | `/gal research` または `/gal deep-research` を実行するとき。 |
| [`conventions/rust.md`](../../../plugins/gal-core/conventions/rust.md) | Rust クレートや Cargo 設定を変更する前。 |
| [`conventions/naming.md`](../../../plugins/gal-core/conventions/naming.md) | クレート、モジュール、CLI コマンド、設定キー、エージェントロールを新規追加または改名するとき。 |
| [`conventions/task-atomicity.md`](../../../plugins/gal-core/conventions/task-atomicity.md) | 計画をアトミックな `T-NN` タスクへ分解するとき。 |
| [`conventions/task-quality.md`](../../../plugins/gal-core/conventions/task-quality.md) | タスク要件を作成するとき。パイプラインのタスクリンターが本ファイルを直接参照します。 |
| [`conventions/open-questions.md`](../../../plugins/gal-core/conventions/open-questions.md) | クラス H、A、F の未解決課題（`## Open Questions`）を定義または解決するとき。 |
| [`conventions/handoff-notes.md`](../../../plugins/gal-core/conventions/handoff-notes.md) | タスクの引き継ぎメモや再試行の承認を記録するとき。 |
| [`conventions/token-budget.md`](../../../plugins/gal-core/conventions/token-budget.md) | トークン予算に関する完全なポリシー。要約は後述。 |
| [`conventions/minimalism.md`](../../../plugins/gal-core/conventions/minimalism.md) | 新たな抽象化や設定オプションの追加要否を評価するとき。 |
| [`conventions/core-vs-personal.md`](../../../plugins/gal-core/conventions/core-vs-personal.md) | コア契約と個人レイヤー（オーバーレイ）のどちらに配置すべきか判断するとき。 |
| [`conventions/self-bootstrap.md`](../../../plugins/gal-core/conventions/self-bootstrap.md) | `crates/` の変更に伴う再ビルドおよび再インストールの運用時。 |
| [`conventions/working-hours.md`](../../../plugins/gal-core/conventions/working-hours.md) | ラップアップ時間（Wrap-up Time）やハードストップ（Hard Stop）の適用有無を確認するとき。 |
| [`conventions/optional-capabilities.md`](../../../plugins/gal-core/conventions/optional-capabilities.md) | オプションの外部統合を追加する前。5 段階の事前確認とグレースフルデグラデーションを定義。 |

## トークン規律

本リポジトリで作業を行うすべての開発者および自動エージェントには、以下のプラクティスが適用されます。権威あるポリシーは [`conventions/token-budget.md`](../../../plugins/gal-core/conventions/token-budget.md) に定義されています。

- **自動生成ファイルを不必要に読み込まない**: アダプター生成の監査を行う場合を除き、自動生成されたアダプター（`AGENTS.md`、`CLAUDE.md`）やビルド成果物（`bin/`、`obj/`）をコンテキストに読み込まないでください。これらのファイルは肥大化しやすく頻繁に更新され、テンプレート以上の追加情報を持ちません。
- **対象を絞り込んでファイルを読み込む**: アクティブなタスクで明示的に指定されたファイル、およびその直接の依存ファイルのみを読み込みます。コールドスタート時にコードベース全体をモデルコンテキストへ一括ロードすることは避けてください。
- **エラーや失敗を簡潔に報告する**: ビルドコマンドの失敗時は、最初の失敗箇所（ファイル名、行番号）と 1 行の要約のみを出力します。テスト実行時は失敗したテスト名とアサーション内容のみを報告します。リンターの場合は違反ルールと該当箇所に絞ります。完全なログが必要な場合はディスクへ退避し、必要な行範囲のみを抽出して検査してください。
- **コンテキスト圧迫に体系的に対処する**: タスク進行中にコンテキスト長の上限が近づいた場合は、次の 3 つの対処を実施します。まず、アクティブプランの `### Handoff Notes` に現在のタスク名、直近に完了した操作、主要な決定事項を記録します。次に、`.dev/state.md` の `## Session Continuity` 内にある該当計画の行を更新し、`Stopped At` と `Next Step` を設定します。最後に、独立した `CONTEXT.md` などの独自ファイルを作成せず、状態は必ずアクティブプランと `.dev/state.md` に集約してください。

## メンテナー用サブコマンド

以下のメンテナンス用サブコマンドは、リポジトリメンテナーおよびリリースワークフロー専用です。

| サブコマンド | 責務 |
| --- | --- |
| `gal restore [--yes]` | ソースリポジトリを直近の `gal-last-good` タグの状態へ戻し、ソース層と生成層の双方を復元します。 |
| `gal release --version <v>` | `checksums.txt`、`artifact-manifest.json`、パッケージマネージャー用マニフェストを含むリリース成果物を生成します。 |
| `gal release-notes` | マージされた変更点からリリースノートを自動生成します。 |
| `gal marketplace-snapshot --source <dir> --out <dir>` | 個人設定やホストバイナリを除外し、公開マーケットプレイス用プラグインツリーを出力します。 |
| `gal naming-gate` | コミット前チェック（pre-commit）において用語登録簿の規則を適用します。 |
| `gal translation-freshness` | `docs/i18n` 配下の翻訳の鮮度状態を評価・レポートします。 |

### 復元処理の挙動（`gal restore`）

| 対象 | 復元処理の挙動 |
| --- | --- |
| **ソース層** | `git reset --hard gal-last-good` および `git clean -fd` を実行します。作業ディレクトリをタグ付けされたコミットの状態と完全に一致させ、未追跡ファイルを削除します。 |
| **生成層** | `gal refresh` を呼び出し、復元されたソースファイルから正規ルートとランタイムへのプロジェクションを再生成します。 |

`gal restore` コマンドがマシンローカルの設定を環境間で移動することはありません。

**`gal-last-good` タグ**: `/gal finalize` を実行すると、完了コミットに対して `gal-last-good` という軽量 Git タグが作成されます。`gal restore` コマンドはこのタグのみを復元のベースラインとして信頼します。タグが存在しない場合、コマンドは推測や `HEAD` へのフォールバックを行わず、具体的な対処方法を案内して安全に fail-closed します。初期化直後で `/gal finalize` が一度も実行されていないリポジトリではこのタグが存在しないため、`gal restore` は安全に実行を拒否します。

**安全バックアップ**: 変更を加える前に、`gal restore` は現在の `HEAD` から `gal-restore-backup-<ts>` というバックアップブランチを必ず作成します。このチェックは必須です。バックアップブランチが作成できない場合、作業ツリーを変更することなく直ちに処理を中止します。

**確認ステップ**: `--yes` フラグを付けずに実行した場合、`gal restore` は未コミットの変更点およびタグからの先行コミット数を表示した上で、使い方を案内して終了します。これにより安全なドライランプレビューが提供されます。

## トラブルシューティング

### ロックファイルの書き込み順序

プロジェクション処理中、`update_skills()` は `update_commands()` の完了を待たずにロックファイルを保存します。この途中で処理が中断された場合、`plugins.lock.json` に opencode エージェントが登録されていても、コマンドのリンクが未完了の状態になることがあります。`gal refresh` は冪等（idempotent）であるため、再度コマンドを実行することで完全に整合した状態へ復旧できます。

### 古いスキルディレクトリのクリーンアップ

`~/.config/opencode/skills/` ディレクトリは `legacy_paths` として追跡されています。ロックファイルの所属情報、`.gal-managed` マーカー、または検証済みのシンボリックリンク先によって GAL の所有権が確認できる場合、`remove_if_gal_owned_dir` が自動的に削除します。ロックファイルによる追跡よりも前に作成され、所有権マーカーを持たないディレクトリについては、安全のため削除を行わず、`gal doctor` が未管理ディレクトリとして報告します。内容が過去の古い GAL プロジェクションのみであることを確認した上で、手動で削除してください。

```powershell
Remove-Item -Recurse -Force "$env:USERPROFILE\.config\opencode\skills"
```

この安全チェックにより、プロジェクション先ディレクトリ内に配置されたユーザー独自の個人ファイルが誤って削除される事態を防止します。

## 開発支援スキル

GAL には、コード実装や技術文書作成を支援する複数の組み込みスキルが用意されています。

| スキル | 用途 |
| --- | --- |
| [`result-pattern`](../../../plugins/gal-core/skills/result-pattern/SKILL.md) | 想定される運用上の失敗に対して Result パターンを適用し、例外はシステム障害に限定します。 |
| [`structured-logging`](../../../plugins/gal-core/skills/structured-logging/SKILL.md) | ログフィールドの構成、適切なログレベルの選定、出力フォーマットの標準化を行います。 |
| [`markdown-formatting`](../../../plugins/gal-core/skills/markdown-formatting/SKILL.md) | プロジェクトドキュメント全体の Markdown 書式とスタイルを標準化します。 |
| [`doc-coauthoring`](../../../plugins/gal-core/skills/doc-coauthoring/SKILL.md) | 技術提案書、仕様書、設計ドキュメントをユーザーと共同作成します。 |
| [`doc-sync`](../../../plugins/gal-core/skills/doc-sync/SKILL.md) | NDJSON 構造マップの保守、およびコードとドキュメント間の乖離を検出します。 |
| [`local-first-search`](../../../plugins/gal-core/skills/local-first-search/SKILL.md) | 外部ノートや外部バックエンドを参照する前に、まずローカルリポジトリ内のファイルを検索します。 |
| [`defuddle`](../../../plugins/gal-core/skills/defuddle/SKILL.md) | Web ページから不要なノイズを除去したクリーンな Markdown を抽出し、トークン消費を抑えます。 |
| [`skill-creator`](../../../plugins/gal-core/skills/skill-creator/SKILL.md) | 再利用可能なエージェントスキルの開発、修正、評価を行います。 |
| [`mcp-builder`](../../../plugins/gal-core/skills/mcp-builder/SKILL.md) | カスタム Model Context Protocol（MCP）サーバーの構築とテストを行います。 |
| [`install-gal`](../../../plugins/gal-core/skills/install-gal/SKILL.md) | 新しい環境への `gal` バイナリのインストールと検証を行います。 |
| [`adversarial-review`](../../../plugins/gal-core/skills/adversarial-review/SKILL.md) | レビュー担当ロールが実装計画、差分、アーキテクチャの主張を批判的に監査するための手法を提供します。 |
| [`opencli-research`](../../../plugins/gal-core/skills/opencli-research/SKILL.md) | OpenCLI を使用してトークン消費を抑えながら構造化された外部データを取得します。 |
| [`text-flowcharts`](../../../plugins/gal-core/skills/text-flowcharts/SKILL.md) | 条件分岐や多段プロセスを等幅テキストのダイアグラムとして可視化・記述します。 |
| [`git-commits`](../../../plugins/gal-core/skills/git-commits/SKILL.md) | リポジトリ基準に準拠した Conventional Commit メッセージを作成し、コミットを実行します。 |

## オプションのローカルレビューツール

オプションの `security-guidance` プラグインは、Claude Code 向けに自動パターンマッチング、ターンごとの差分分析、コミット契機のセキュリティレビューを提供します。本プラグインは助言専用（advisory-only）として動作し、ファイルの書き込みやコミットをブロックすることはなく、必須の CI チェックでもありません。プラグイン評価に伴うトークン消費はコントリビューター自身のアカウントで負担されます。

本プラグインは基本フックに Python 3.7 以上、エージェント評価に Python 3.10 以上を必要とします。初回実行時、`~/.claude/security/` に仮想環境がセットアップされます。Windows 環境では、`python3` コマンドが Windows ストアのアプリ実行エイリアスではなく、有効な Python インストール先を解決していることを確認してください。ツールが GAL の信頼境界に関わる問題を検出した場合は、[セキュリティポリシー](SECURITY.ja.md) と照合してください。なお、リリース版における脆弱性は、前述の非公開 Advisory 窓口から報告してください。
