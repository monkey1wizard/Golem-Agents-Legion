# Golem-Agents-Legion

<!-- gal:translation-metadata
source: README.md
lang: ja
source_commit: b25bab880abfab7cb3e40517f4693c457523d5a0
translated_at: 2026-10-09
-->

[English](../../../README.md) · **日本語** · [繁體中文](../zh-Hant/README.zh-Hant.md)

GAL は Rust で構築された、ドキュメント駆動型の AI 開発ワークフローエンジンです。リポジトリが所有する Markdown ファイル群（`.dev/`）を通じて、計画、実装、テスト、レビュー、調査にわたる構造化された開発ライフサイクルを提供します。単一の AI ツール（Claude Code、Codex CLI、GitHub Copilot、Antigravity CLI、opencode など）のみでも完結して動作し、必要に応じて複数のプロバイダーやモデルを自由に組み合わせたり切り替えたりする柔軟性も備えています。コンパイル済み Rust バイナリが、決定論的な品質ゲートのもとで開発を推進します。

専属のゴーレム軍団を指揮するように、各 golem エージェント (golem agent) がワークフロー内でそれぞれの役割を果たし、協調して開発タスクを完遂します。これが「Golem-Agents-Legion」というプロジェクト名の由来です。

## 主な機能と特徴

- **単一ツールで即座に動作、柔軟なマルチランタイム対応**: お好みの AI ランタイム（Claude Code、Codex CLI、GitHub Copilot、Antigravity CLI、opencode）1 つだけで全ワークフローを完全に実行でき、複数サブスクリプションは不要です。必要に応じてツールの切り替えや特定フェーズへの別モデル割り当ても自由に行えます。
- **ドキュメント駆動型の構造化ライフサイクル**: 仕様策定、計画、レビュー、実装の各段階に設けた品質ゲート（REFINE-LOCK ループ）により、コード変更に着手する前に要件を決定論的に収束させます。
- **リポジトリ所有の状態管理とチェックポイント再開 (`.dev/`)**: プラットフォーム固有のチャット履歴に依存せず、ローカルの Markdown ファイルで開発コンテキストを永続化します。パイプラインが途中で中断しても、直近の記録チェックポイントから正確に再開可能です。
- **統一された運用契約とベンダーロックインの排除**: 共通の操作体系 (`/gal …`) と役割別 golem エージェント契約により、サポート対象の全環境で一貫したワークフローを維持し、ツールを切り替えてもプロジェクト資産の再構築は不要です。
- **ネイティブ Rust コンパイルエンジン**: `gal` バイナリがオーケストレーション、状態の投影、アダプター生成をローカルで高速に処理し、プロンプトのみに頼るフレームワークのような構造的オーバーヘッドやコンテキスト膨張を防ぎます。
- **柔軟なモデルルーティングとコスト最適化**: 開発フェーズ（計画、実装、監査など）ごとに、`~/.gal/config/config.json#executorRouting` で最も費用対効果の高いモデルや高性能なモデルを自在に割り当てられます。

## クイックスタート

本リポジトリをクローンすることなく、GAL を直接インストールして使い始めることができます。

**前提条件:** Claude Code、Codex CLI、GitHub Copilot、Antigravity CLI、opencode のうち、少なくとも 1 つの対応 AI ランタイムがインストールされている必要があります。

1. **`gal` CLI をインストールします。**

   ```bash
   curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
   ```

   ```sh
   irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
   ```

   Homebrew、WinGet、ソースコードからのビルドなど、その他のインストール手順については [セットアップ](setup.ja.md) を参照してください。

2. **プロジェクトを初期化します。** 対象プロジェクトのディレクトリで AI ランタイムを起動し、`/gal init`（Codex CLI では `$gal init`）を実行します。これにより `.dev/` ディレクトリが初期化され、必要なエージェントアダプターが自動生成されます。

   **指示ファイルの読み込みを確認してください。** GAL が提供するリポジトリのルート指示ファイルは `AGENTS.md` だけです。使用する coding agent がこのファイルを読み込むことを確認してください。リポジトリに `CLAUDE.md` などがあると、ツールの読み込み設計によっては `AGENTS.md` が読み込まれない場合があります。対応するファイル名と読み込みの優先順位は、使用する coding agent のドキュメントを参照してください。詳しくは[セットアップガイド](setup.ja.md)を参照してください。

3. **作業を開始します。** `/gal status` でリポジトリの現在の状態と推奨される次のアクションを確認します。`/planning` を実行すると、タスク要件から最初の計画ファイルが生成されます。

これでリポジトリの初期化は完了し、すぐに作業を開始できます。オプションのエグゼキュータールーティングやライブプロバイダー検証を利用する場合は、追加の設定が必要です。サポートされている権限モードについては [Codex パイプライン実行のセットアップ](setup.ja.md#codex-パイプライン実行のセットアップ) を、ディスパッチ境界と証拠の評価段階については [パイプライン実行順序と証拠](workflows.ja.md#パイプライン実行順序と証拠) を参照してください。その他のトピックについては、後述の [ドキュメントマップ](#ドキュメントマップ) を参照してください。

### 機能ライフサイクル

```text
/gal init
   ↓
┌── Planning Phase ──────┐
│ /planning              │
│    ↓                   │
│ /deep-planning         │
│    ↓                   │
│ [OQ-completion gate]   │
│    ↓                   │
│ /refining-plan         │
│    ↓                   │
│ [human approval gate]  │
│    ↓                   │
│ /plan-to-prompt        │
└────────────────────────┘
   ↓
/gal pipeline
   ↓
[automatic finalize]
```

パイプラインは計画のゴールを検証したあと、最後のステップとして finalize を実行します。そのため、通常の `/gal pipeline` を 1 回実行すると、計画が反映（land）されるか、未完了の Owner Acceptance 行や失敗したゲートなどの具体的な停止で終わります。停止した場合、計画は反映されず、未完了の確認も承認されたことにはなりません。`/gal finalize` を自分で実行するのは、finalize に明示的に入る場合、または停止後に復旧する場合に限ります。

`/gal init` は初回のみリポジトリの骨格を構築します。計画フェーズ、パイプライン実行、finalize の詳細な手順については [ワークフロー](workflows.ja.md) を参照してください。

## ドキュメントマップ

以下の表は公開ドキュメントの概要です。各トピックには単一の権威ファイルが存在し、関連文書はルールを重複定義することなくその権威ファイルを参照します。

この README の説明は本表で管理します。ほかの文書の「単一の責務」は、各文書の front matter にある `description` と一致させます。差異がある場合は、対象文書の `description` に合わせて本表を更新します。

| 文書 | 単一の責務 |
| --- | --- |
| [README.ja.md](README.ja.md) | GAL の概要、クイックスタート、機能ライフサイクルの要約、ドキュメントマップを提供する公開エントリーポイント。 |
| [setup.ja.md](setup.ja.md) | `gal` バイナリのインストール、対応ランタイムへのプラグイン登録、リポジトリアダプターの初期化と保守を説明します。 |
| [configuration.ja.md](configuration.ja.md) | `executorRouting`、稼働時間、計画言語、MCP ソース、個人レイヤー、local-notes のルーティングを含む `~/.gal/config/config.json` のマシンローカル設定を管理します。 |
| [workflows.ja.md](workflows.ja.md) | golem エージェントの役割と呼び出し方法、計画ワークフロー、自動パイプライン実行、finalize、復旧、オーサリング支援を説明します。 |
| [projection.ja.md](projection.ja.md) | GAL がソース契約を canonical root にレンダリングし、各ランタイム操作面へプロジェクションして、リポジトリアダプター、プロジェクションレジストリ、MCP マニフェストを生成する方法を説明します。 |
| [architecture.ja.md](architecture.ja.md) | GAL のアーキテクチャ制約、クレートとレイヤーの境界、ストレージトポロジー、移行経路、ドキュメント統制、ADR 索引を詳述します。 |
| [integrations.ja.md](integrations.ja.md) | オプション外部統合のリファレンスです。運用目的、準備状況の確認、設定境界、グレースフルデグラデーションを扱います。 |
| [SECURITY.ja.md](SECURITY.ja.md) | GAL のサポート対象バージョン、脆弱性報告手順、脅威モデル、セキュリティ信頼境界を定義します。 |
| [CONTRIBUTING.ja.md](CONTRIBUTING.ja.md) | GAL のコントリビューション指針です。ブランチ規約、コミット標準、ビルドとテストのゲート、レビューチェックリストを扱います。 |

用語の権威およびアーキテクチャ決定記録（ADR）は、固定された翻訳セットの外部で管理されています。

| 文書 | 単一の責務 |
| --- | --- |
| [`terminology.ja.md`](terminology.ja.md) | 日本語文書の用語、表記、翻訳境界を定義するロケール別プロファイル。 |
| [`docs/glossary.md`](../../glossary.md) | GAL の用語に関する単一の意味的権威です。命名規則、canonical term registry、廃止語を扱います。 |
| [ADR](../../adr/) | 評価した選択肢と却下理由を記録する Architecture Decision Record です。索引は architecture.md にあります。 |

## 関連プロジェクト・参考リソース

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)
- [rtk](https://github.com/rtk-ai/rtk): `git` や `cargo`、テストランナーなどの CLI 出力を、モデルのコンテキストへ送信する前にフィルタリング・圧縮するプロキシツールです。GAL と `rtk` の併用を強く推奨します。パイプライン処理では多数のシェルコマンドが実行されるため、出力を適切に要約・圧縮することでコンテキストウィンドウの消費を大幅に節約できます。

## ライセンス

MIT ライセンスです。詳細は [LICENSE](../../../LICENSE) を参照してください。
