---
source: docs/integrations.md
lang: ja
source_commit: c1c29b8634be5e67b648640f8e6c6af13fbab0a5
translated_at: 2026-07-23
status: current
---

# 統合ツール (Integrations)

[English](../../integrations.md) · **日本語** · [繁體中文](../zh-Hant/integrations.zh-Hant.md)

GAL が提供する、オプションの外部ツールに関する公開リファレンスである。いずれもオプション機能であり、コア実行環境 (runtime) の依存関係ではない。一つもインストールしていなくても、GAL は通常どおり動作する。約束的な動作（5 状態 preflight、structural-retrieval のルーティング、およびすべての golem エージェントが従うべき正直な劣化ルール）は、同梱の契約 (contract) ファイルに定義されている。[`plugins/gal-core/conventions/optional-capabilities.md`](../../../plugins/gal-core/conventions/optional-capabilities.md) を参照すること。本ファイルは人間向けのセットアップとリファレンスのみを扱い、あくまで参考情報であって拘束力は持たない。

以下の各節は同一のスキーマに従う。purpose、real GAL consumer、readiness check、setup boundary、safe fallback、managed-vs-consumed です。

## codebase-memory-mcp

- **Purpose** —— オプションの、MCP ベースの structural-retrieval 補助ツールです。ライブの symbol・構造検索を行い、ネイティブの `git diff` の後に file-to-symbol の影響マッピングを精緻化する。
- **Real GAL consumer** —— `doc-sync`（`codeRefs` に `#symbol` が含まれる場合に、影響を受ける文書の対象範囲を絞り込む）、および graphify に続く二次パスとしての一般的な structural-retrieval のレーンである。
- **Readiness check** —— MCP server に到達可能で、`index_status` が現在のリポジトリがインデックス済みかつクエリ可能であることを示していることである。特に Codex では、readiness は設定済みの server 一覧ではなく、セッション内で実際に**公開**されているツールに対して判定する必要がある。設定済みだが未公開の server は `ready` ではなく `unavailable` である。
- **Setup boundary** —— GAL はリポジトリのインストール・初期化・自動インデックス作成を行わない。インストール、インデックス作成、クエリの意味論は `codebase-memory-mcp` プロジェクト側の上流責務である。
- **Safe fallback** —— ネイティブの `git diff` に直接のファイル読み取りを加えたものが、必須のベースラインであり続ける。MCP の結果は精緻化するだけであり、file-level の検出を置き換えることは決してない。
- **Managed-vs-consumed** —— **managed** である。GAL の managed MCP manifest（`plugins/gal-core/mcp.json`、`DeusData/codebase-memory-mcp`）に記載されている。

## graphify

- **Purpose** —— オプションの、CLI 駆動の structural-context 機能です。事前構築された graph 成果物（`graphify-out/GRAPH_REPORT.md`）を取り込み、計画とレビューのレーンに補助的な structural context を提供する。すなわち god node、community、およびモジュール間の予想外の接続である。
- **Real GAL consumer** —— `/planning` と `/deep-planning`（スコープとモジュール境界の判断）、`golem-architect`（抽象化、結合度、影響範囲）、`golem-auditor`（予期しない community 間の影響）である。
- **Readiness check** —— リポジトリに `graphify-out/GRAPH_REPORT.md` が存在することである。オプションの `graphify-out/GAL_GRAPHIFY_VERSION.txt` は、インストール済みの `graphify --version` と比較して陳腐化を判定する。バージョンが異なり、かつレポートがその戳記より新しくない場合は、そのレポートを「ツールバージョンにより陳腐化」として扱う。
- **Setup boundary** —— GAL は graphify のインストール、graph の生成、陳腐化した成果物の再構築を行わない。インストール、graph の生成、クエリの意味論は上流の責務である。[safishamsi/graphify](https://github.com/safishamsi/graphify) を参照すること。
- **Safe fallback** —— ネイティブのコードベース読み取りと標準的な計画・レビューの動作へ劣化する。成果物が欠けていてもエラーやセットアップの促しは発生しない。
- **Managed-vs-consumed** —— consumed のみである。MCP server として配線されておらず、GAL は生成済みのレポートファイルを読むだけで、graphify にライブでクエリすることはない。

## OpenCLI

- **Purpose** —— オプションの外部 CLI 実行環境である。構造化された低トークン消費の外部取得（YouTube、NotebookLM、Wikipedia、Hacker News、Google News および類似のソース）のために、差し替え可能な site adapter を提供する。
- **Real GAL consumer** —— 研究ワークフロー（`/gal research`、`/gal deep-research`）、`opencli-research` スキルである。
- **Readiness check** —— `opencli --version` が成功し、かつ現在のタスクが、必要なフィールドを返せる既存の adapter に対応していることである。
- **Setup boundary** —— GAL は OpenCLI やその adapter を自動インストールしない。インストールと adapter の作成は上流の責務である。
- **Safe fallback** —— 標準的なページには MCP の `fetch`／`imagefetch`、操作・レンダリングには Playwright MCP または Chrome DevTools MCP、リポジトリ本機のコードには workspace ツールへフォールバックする。成功を偽装するのではなく、能力欠如を明示するメッセージで停止する。
- **Managed-vs-consumed** —— consumed のみである。MCP manifest の項目ではなく、直接呼び出す CLI 実行環境である。

## Playwright MCP

- **Purpose** —— オプションのブラウザ自動化機能です。ライブのブラウザ操作、accessibility snapshot、スクリーンショット、レスポンシブ検査、ブラウザ可視の評価に用いる。
- **Real GAL consumer** —— `golem-tester`（QA と回帰テスト）、`golem-designer`（ライブ UI 監査）、および local-first と structured-retrieval のチェックが失敗した後に動的ページのレンダリングを必要とする研究ワークフローである。
- **Readiness check** —— 実行環境 (runtime) が Playwright MCP server を解決して起動でき、ブラウザ／server の初期化が完了しており、現在のタスクにブラウザに適した対象があることである。
- **Setup boundary** —— GAL は、通常のテスト、レビュー、デザイン監査、研究の途中で、初回のブラウザ／server セットアップを自動実行しない。
- **Safe fallback** —— 深い診断には Chrome DevTools MCP、再利用可能な自動化にはネイティブの Playwright スクリプト、取得には OpenCLI／fetch／Defuddle／workspace ツールへフォールバックする。実際には実行していないのに、ブラウザ検証が成功したと主張することは決してない。
- **Managed-vs-consumed** —— **user-wired** である。Playwright MCP は GAL の managed MCP manifest（`plugins/gal-core/mcp.json`）には存在せず、この機能が欲しいユーザーが自身の実行環境設定で配線する。

local-notes（ユーザー自身が所有する外部ノートストア機能）はツール統合ではないため、本ファイルに対応する節はない。その約束的な意味論は [`optional-capabilities.md`](../../../plugins/gal-core/conventions/optional-capabilities.md) に、その運用セットアップは [`manual.md`](manual.ja.md) にあります。
