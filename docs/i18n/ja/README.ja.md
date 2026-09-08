---
source: README.md
lang: ja
source_commit: fc4d3b7046db5e731f0d21728e1e093b0dd10bf8
translated_at: 2026-08-06
status: current
---

# Golem-Agents-Legion

[English](../../../README.md) · **日本語** · [繁體中文](../zh-Hant/README.zh-Hant.md)

Rust で構築された、クロスプロバイダー (cross-provider) の AI 作業システムである。GAL は Claude Code、Codex CLI、GitHub Copilot、Antigravity CLI、opencode をまたいでネイティブに動作し、単一の開発ライフサイクルの中で異なるプロバイダーのモデルを自由に組み合わせることができる。コンパイル済みバイナリのアーキテクチャにより、プロバイダー間でコンテキストを受け渡す際のトークン負荷を最小化し、同時にリポジトリ所有の Markdown ファイルによって、計画・実装・テスト・レビュー・研究をツール間でシームレスに移行できる。
大魔法使いが golem の軍団を操るように、各 golem エージェントがワークフロー上の固有の役割に縛られて機能する。これが Golem-Agents-Legion という名前の由来である。

## GAL とは

- **リポジトリ所有の状態** (`.dev/`)：ベンダーに固定されたチャット記憶を置き換え、AI ツールをまたいだコンテキストの永続化を可能にする。
- **統一された単一の契約**：制御プレーンの操作面 (`/gal …`) と専門エージェントが、5 つの実行環境 (runtime) にわたって一貫したワークフローを強制する。

## なぜ GAL なのか

- **設計段階からクロスプロバイダー**：計画・実装・テスト・監査の各フェーズは、`/gal pipeline` を通じて異なるプロバイダーのエージェントへディスパッチされる。REFINE-LOCK ループが計画を収束させ、クロスモデルの検証は構造的に組み込まれており、任意の追加機能ではない。
- **Rust ネイティブのオーケストレーション**：`gal` バイナリは、オーケストレーション、状態の投影 (projection)、アダプター生成をコンパイル済みバイナリとして管理する。プロバイダー間のコンテキスト移行に伴う構造的な負荷は、プロンプトのみ (prompt-only) の手法より低く、これは pipeline 実行ごとの総トークン消費量とは独立した性質である。
- **コスト制御**：各フェーズの実行モデルは `config.json#executorRouting` で設定できる。
- **ベンダーロックインなし**：ワークフローは標準化されており、特定の AI プロバイダーから独立している。プロジェクトの状態を再設計することなく、いつでもプロバイダーを切り替え可能である。
- **状態の連続性**：実行状態は `.dev/` に直接記録される。中断された pipeline 実行は、記録された正確なブレークポイントから再開する。

## クイックスタート

GAL の主要な方法論は**インストールモード**を用いる。このリポジトリを clone する必要はない。

**前提条件：** サポートされている AI コーディング実行環境（Claude Code、Codex CLI、GitHub Copilot、Antigravity CLI、opencode のいずれか）を事前にセットアップしておく必要がある。

1. **`gal` CLI をインストールする：**

   ```bash
   curl -fsSL https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.sh | bash
   ```

   ```sh
   irm https://raw.githubusercontent.com/monkey1wizard/golem-agents-legion/main/packaging/install.ps1 | iex
   ```

   Homebrew、winget、cargo を含む他のインストール方法は、[ユーザーマニュアル](../../manual.md#installing-gal)に詳しく記載されている。

2. **プロジェクトの初期化**：対象プロジェクト内でサポートされている実行環境を開き、`/gal init` を実行します。Codex では `$gal init` を用いる。この操作により `.dev/` ディレクトリが作成され、各エージェント向けのアダプターファイルが生成される。

3. **ワークフローの実行**：`/gal status` を実行してリポジトリの初期化を確認し、以降の手順を確認する。`/planning` を実行して、リクエストを初期計画へ変換する。

これらの手順は、機能する初期化済みリポジトリへ至る完全な道のりである。このベースライン状態にオプションの統合は必要ない。以降の内容は必要に応じて参照するリファレンスである。

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
/gal finalize
```

`/gal init` コマンドは、前述の手順に従ってリポジトリの骨格を一度だけ作成する。以降のノードは [GAL の仕組み](#gal-の仕組み) 内に専用の節を持つ。計画フェーズは [計画フェーズ](#計画フェーズ)、実行は [Pipeline](#pipeline)、着地は [Finalize](#finalize) にある。

## ファイルとストレージ

GAL のファイルは 2 つの異なる場所に存在する。永続的でリポジトリ所有の状態は**プロジェクトフォルダー内**に、機械本機の実行環境ファイルは**ホームフォルダー（`~`）内**に存在する。

**プロジェクトフォルダーの内容** —— `gal init` が作成する、永続的でレビュー可能かつ差分の取れるワークフロー状態である。

```text
.dev/
├── project.md                 compressed project summary (cold-start first read)
├── state.md                   active plans index + session continuity
├── plans/<slug>.md            human-readable source plan (transient)
├── plans/<slug>.prompt.md     AI execution work file (mutable task memory)
├── plans/<slug>.en.md         EN semantic draft for non-English planLanguage (tracked, see manual)
└── research/                  research work files (non-durable, findings are promoted to docs/)
CLAUDE.md · AGENTS.md · GEMINI.md · .github/copilot-instructions.md · .agents/rules/gal.md
                               generated per-agent adapters (regenerate with gal render-adapters)
```

**ホームディレクトリの内容** —— 機械本機の実行環境コンポーネントであり、真実の情報源からは厳格に除外される。

```text
~/.gal/
├── config/config.json         user-owned machine config: personalization, secrets, executorRouting — preserved across reinstalls
├── plugins/gal/               GAL-managed canonical plugin root (the runtime content owner)
├── generated/                 GAL-produced projections, rebuildable
├── state/plugins.lock.json    projection registry lockfile
└── active/<runtime>/          stable shortcut targets for AI tools
```

`~/.claude/skills/gal` や `~/.copilot/skills/` を含むプロバイダー可視の面は、二次的な真実の情報源ではなく、正規ルート (canonical root) の投影として機能する。レイアウトの完全なドキュメントは [docs/architecture.md → Runtime Topology](../../architecture.md#repository--runtime-topology) にある。

プライベートノートは、独立したユーザー所有の境界として動作する。これはオプションのパーソナル拡張 (Personal Enhancement) であり、既定では無効で、`~/.gal/config/config.json` 内で設定する。GAL Core はこの機能なしでも完全な機能を維持し、特定のノートのレイアウトを仮定することを厳格に避ける。この機能はコラボレーションツールではなく、オプションの拡張機能である。約束的な契約は [optional-capabilities.md](../../../plugins/gal-core/conventions/optional-capabilities.md) を、セットアップ手順は [docs/manual.md](../../manual.md) を参照すること。

## GAL の仕組み

### コマンド

| 系統 | コマンド |
| --- | --- |
| 制御プレーン | `/gal`（単独実行で次の手順を自動検出）に加え、サブコマンド `init · status · whats-next · wrap-up · pipeline · finalize · research · deep-research` |
| エージェントルート | `/gal <golem-name>`（直接呼び出し可能な golem）と `/gal discuss <role>`（コンテキスト内での相談） |
| 計画 | `/planning` · `/deep-planning` · `/refining-plan` · `/plan-to-prompt` |
| 独立スキル | `/adversarial-review`（計画、差分、文書、決定を対象とする対抗的レビュー）、`git-commit-msg`（ステージ済み変更からの Conventional Commit メッセージ生成）、および `/text-flowcharts`（分岐ロジックを等幅プレーンテキストの決定木図として描画） |

Codex 実行環境では、同一の名前を `$` 接頭辞付きのスキルとして呼び出す（例：`/gal status` の代わりに `$gal status`）。完全な契約の詳細は [plugins/gal-core/commands/commands.md](../../../plugins/gal-core/commands/commands.md) にある。

### Golem エージェント

10 個の専門エージェントは 2 つのクラスに分かれる。直接呼び出し可能なものと、オーケストレーター経由限定のものである。直接呼び出し可能なエージェントは `architect`、`analyst`、`designer`、`releaser`、`debugger`、`steward` で、計画フェーズや相談の中で `/gal <role>` を通じて呼び出せる。

完全な能力表、呼び出し可能性マトリクス、デュアルモードの詳細は [docs/manual.md → Golem Agents](../../manual.md#golem-agents) にある。権威ある名簿は [agents.md](../../../plugins/gal-core/agents/agents.md) に、ワークフロー契約は [coding.md](../../../plugins/gal-core/workflows/coding.md) にある。

### 計画フェーズ

- `/planning`：リクエストに基づいて `.dev/plans/<type>-<slug>.md` にソースプラン (source plan) を作成する。
- `/deep-planning`：発散してから収束するアーキテクチャレビューを実行する。これは必須の architect レビュー、条件付きの analyst と designer のレビュー、および steward による計画構造チェックを含む。Protected Paths や構造的変更ではこの手順が必須であり、`ARCH_REVIEW: CLEAR` を出力する。**厳格な要件：`/refining-plan` を開始する前に、ユーザーがすべての未解決の問いを解決することが必須である。**
- `/refining-plan`：実装契約をソースプランに書き込む。`## Tasks`（T-NN）と `## Test Plan`（TP-NN）の各節は、architect と tester による Definition-of-Ready ゲートを収束まで反復し、`ENG_REVIEW: CLEAR` を出力する。
- 対抗的レビュープロセスがすべてのレビューゲートを統制する。これはスティールマン論法 (steel-man)、既定で反駁するロジック、および明示的な APPROVE・REVISE・REJECT の裁定を強制し、REVISE はハードなストップラインを生じる。`/adversarial-review` コマンドはこの方法論への独立したアクセスを提供する。
- `## Approval` 節に記録される人間の承認が、草案を確定させる最終の手動レビューとして機能する。この明示的な承認は、`/plan-to-prompt` コマンドが `.dev/plans/<slug>.prompt.md` 実行作業ファイルを生成できるようになるための必須の前提条件である。

計画の完全な意味論は [plugins/gal-core/workflows/coding.md](../../../plugins/gal-core/workflows/coding.md#stage-35--definition-of-ready-gate-refine-lock-loop) にある。

### Pipeline

```text
per task (T-NN):

  ORCHESTRATOR (dispatches each phase, checks each return)
       |
       |dispatch task T-NN
       ↓                      ┌──( >3 )──► STOP (handoff)
  CODER (implement) ◄─────────┤
       ↓                      │ re-dispatch
  TESTER (unit test) ──FAIL───┤ implement
       ↓ pass                 │ ( fix counts ≤ 3 )
  AUDITOR (review) ──REJECT───┘
       ↓ pass
  ORCHESTRATOR (commit + 3-surface converge)
       ↓
  next task ↺

all tasks done:

  ORCHESTRATOR (goal-backward verify, in-process)
       ↓
  VERIFIED → /gal finalize
```

実行中、pipeline の各フェーズは `config.json#executorRouting` に定義されたルート済みのエグゼキューター (executor) へディスパッチされる。これにより implementer、tester、auditor の各役割が**異なるベンダーのコーディングエージェント (coding agent)** を用いることができ、独立したクロスモデル検証を構造として保証する。

単一のタスクが 3 回を超えて検証に失敗すると、pipeline の実行は停止してユーザーへエスカレートし、無限ループを防ぐ。正確性ゲートはオーケストレーターが制御し、`golem-auditor` がタスクごとの深いパフォーマンスとセキュリティの監査を担う。

ユーザーは **SSH** を通じてリモートのクロスマシンタスクを実行できる。手順は [docs/manual.md](../../manual.md#remote-execution-ssh-dispatch-lane) の仕様を参照すること。完全なワークフローの意味論は [plugins/gal-core/workflows/coding.md](../../../plugins/gal-core/workflows/coding.md) にある。

### Finalize

- `/gal finalize` コマンドは、破壊的な完了着地を順序立てる薄いオーケストレーターとして機能する。ゼロトラストの再検証を強制するため、実行はパスした full-mode の `gal finalize-check` レシートと、オーケストレーターによる後ろ向きの目標検証の成功に厳格にゲートされる。その順序は以下を含む。
  - **総合レビュー**：結合されたすべてのタスクにわたる全ブランチ評価を実行する。
  - **文書検証（Doc-sync）**：`STEWARD` が永続的な知識を `docs/` へ抽出し、厳格な commit hash ゲートを確立する。
  - **マージ・削除・クローズ**：（必要なら）main への `git merge` を実行し、ワークツリー (worktree) があれば撤去し、`ORCHESTRATOR` が計画ファイルの削除を行ってライフサイクルをクローズする。リリースは、実際の成果物を出荷するときにのみ発生する。

`/gal wrap-up` コマンドの動作は異なる。これはいつでも呼び出せる非破壊的なセッションの**一時停止**として機能し、進行中の連続性を記録するだけで、着地やクローズの動作は行わない。

### 研究

このワークフローは Coding Flow から独立して動作し、開発と並行して、あるいは独立した調査として実行できる。

- `/gal research` コマンドは RESEARCH → VERIFY → DOCUMENT の順序を実行する。
- `/gal deep-research` コマンドは、曖昧さの高いトピックに対して VERIFY の前に SYNTHESIZE と CROSS-REVIEW の操作を挿入する。最低 5 つのソースを試みることを要求する。

VERIFY フェーズでは、結果の著者とは異なる独立したモデルが、引用されたすべての参照を逆方向に検証することを要求する。出力は、リポジトリ所有の証拠を優先するため、既定で `.dev/research/` に置かれる。結果を個人の外部ノートへ振り向けるには、オプトイン済みの機械本機ノート (local-notes) バックエンドが必要である。完全な契約の詳細は [plugins/gal-core/workflows/research.md](../../../plugins/gal-core/workflows/research.md) にある。

## 統合ツール

オプションの外部ツールは特定の作業レーンを強化する。**GAL は自動インストールを避け、これらのツールなしでも完全な機能を維持する**。統合は、適用性、可用性、初期化状態、整備度、ルーティング、劣化を扱う共通の 5 状態 preflight 順序を通じて処理される。

| ツール | 機能 | ドキュメント |
| --- | --- | --- |
| codebase-memory-mcp | ネイティブのファイル検出の後、ライブの MCP ベースの構造・symbol 検索を実行して対象の絞り込みを精緻化する | [→](../../integrations.md#codebase-memory-mcp) |
| graphify | リポジトリのファイルを知識グラフへ変換し `graphify-out/` へ出力する。GAL は計画とレビューの際にこのデータを取り込み、モジュール間の結合を識別する | [→](../../integrations.md#graphify) |
| OpenCLI | ログイン済みのアクティブなセッションを活用し、Web サイト、ブラウザセッション、Electron アプリ、本機ツールを再利用可能な CLI コマンドへ変換する | [→](../../integrations.md#opencli) |
| Playwright MCP | ユーザーが配線したブラウザ機能を提供し、テスト実行、デザイン監査、動的ページの研究、ブラウザ可視の MCP 評価を支援する | [→](../../integrations.md#playwright-mcp) |

包括的な統合の詳細、セットアップ手順、ルーティング契約は [docs/integrations.md](../../integrations.md) にある。

## ドキュメント

この節は**文書責任境界に関する単一の権威**である。すべてのトップレベル文書は一つの固定された役割を持つ。ある文書の行に一致しない内容は、その内容が一致する行を持つ文書に属する。重複させるのではなく移動すること。他のどの文書もこれらの境界を再定義せず、ここへリンクする。

| 文書 | 役割 | 答える問い |
| --- | --- | --- |
| [README](README.ja.md)（本ファイル） | 公開入口：GAL とは何か、なぜ存在するか、クイックスタート、ワークフロー概要 | 「これは何で、どう使い始めるのか」 |
| [手順書](../ja/manual.ja.md) | エンドユーザー操作：インストール、初回実行、ワークフロー、設定、パーソナライズ、エグゼキューター、golem | 「GAL を日々どう運用するのか」 |
| [統合ツール](../ja/integrations.ja.md) | オプション統合ごとに 1 節 | 「GAL はツール X とどう連携するのか」 |
| [contributing](../../../CONTRIBUTING.md) | 貢献者入口：ゲート、ワークフローの骨格、読む順序 | 「貢献したい。最初の一歩は何か」 |
| [architecture](../../architecture.md) | **システムの構成と設計理由を図示。** 構造、所有権マップ、決定記録で構成。図を主体とし文章で補足する。開発者に限らず、コードを変更しないユーザーも GAL の全体像を理解するために参照可能 | 「これはどう組み合わさり、なぜこの形なのか」 |
| [developer guide](../../devguide.md) | **システムの変更手順**：手順、内部ループ、運用方法。文章と手順リストを中心に記述。**開発者専用** —— ソースコードのチェックアウトと GAL の改変意図を前提とする | 「X を変更したい。手順は何か」 |
| [naming](../../naming.md) | すべての中核用語の意味の権威、および廃止用語ゲートの入力 | 「この語はここで何を意味するのか」 |
| [plugins/gal-core/commands/commands.md](../../../plugins/gal-core/commands/commands.md) · [plugins/gal-core/agents/agents.md](../../../plugins/gal-core/agents/agents.md) · [plugins/gal-core/workflows/coding.md](../../../plugins/gal-core/workflows/coding.md) | 正規の制御プレーン操作、エージェント責務、ワークフロー契約を定義 | 「契約は何か」 |

**新しい貢献者の読む順序：** `CONTRIBUTING.md`（開始）→ `docs/architecture.md`（理解）→ `docs/devguide.md`（変更）。

**2 つの開発者向け文書の読者の切り分け。** `architecture.md` は図優先で、あらゆる読者に開かれている。コードに触れる意図のない読者も、GAL が自分の機械に何をするのか、そしてなぜかを理解するために読める。`devguide.md` は手順優先で、読者がソースを checkout 済みで GAL を変更しようとしていることを前提とする。内容を追加するときは、読者がどちらなのかを問うこと。フィールドレベルの schema 表、テストファイルの場所、再構築コマンドは、たとえ構造を記述していても `devguide.md` に属する。

`plugins/gal-core/` 配下の正規契約は、それらに関するどの文書の要約よりも常に優先される。文書のフォーマット、命名、翻訳ポリシーは独立した関心事であり、[devguide → Documentation Conventions](../../devguide.md#documentation-conventions) が管理する。

## 参考資料

- [Get Shit Done (GSD)](https://github.com/gsd-build/get-shit-done)
- [GitHub Spec Kit](https://github.com/github/spec-kit)
- [gstack](https://github.com/garrytan/gstack)
- [rtk](https://github.com/rtk-ai/rtk)：git、cargo、テストランナーなどの開発コマンド出力を、モデルコンテキストへ届く前にフィルタして圧縮するために設計された CLI プロキシです。このツールは GAL の統合とは独立して存在し、GAL の検出や依存なしに動作します。GAL との併用を強く推奨します。pipeline の実行はタスクごとに多数のシェルコマンドを起動するため、出力の刈り込みは実行環境のコンテキスト予算を効果的に最大化します。

## ライセンス

MIT ライセンスの下で提供される。[LICENSE](../../../LICENSE) を参照すること。
