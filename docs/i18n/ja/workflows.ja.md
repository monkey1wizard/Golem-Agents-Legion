---
source: docs/workflows.md
lang: ja
source_commit: c176446a028e431dce518eb4e1e6010046d04321
translated_at: 2026-10-23
type: Guide
title: ワークフローと支援機能
description: golem エージェントの役割と呼び出し方法、計画ワークフロー、自動パイプライン実行、finalize、復旧、オーサリング支援を説明します。
tags:
  - workflows
  - agents
  - pipeline
  - recovery
status: stable
---

# ワークフローと支援機能

[English](../../workflows.md) · **日本語** · [繁體中文](../zh-Hant/workflows.zh-Hant.md)

## 必要な場合にのみ確認して作業を続ける

承認済みのステップでユーザー入力が不要な場合、エージェント (agent) は依頼された作業が完了するか、必須の GAL ゲートが進行を止めるまで作業を続けます。各進捗報告には具体的な次の操作を記載し、エージェントはその操作を実行します。進捗報告だけでターンを終えることはなく、続行の確認も求めません。

エージェントが確認を求めるのは、次の場合です。ユーザー入力がないと先に進めない場合、データの削除や force-push などの破壊的な操作の前、リポジトリ外のファイルを変更する前です。以前に明示された承認は、その範囲内で有効です。必須の GAL 承認、レシートゲート、Protected Paths のレビュー、executor 出力の保全、稼働時間ポリシー、リポジトリの制約は引き続き適用されます。

## Golem の役割と呼び出し

### Golem エージェントの概要

GAL は、専門特化された AI エージェント群を「golem」と呼ばれる明確なペルソナとして体系化しています。各 golem は明確な責務、運用スコープ、およびツール統合を持っています。正規のエージェント定義は [`plugins/gal-core/agents/agents.md`](../../../plugins/gal-core/agents/agents.md) に、正式なワークフロー契約は [`plugins/gal-core/workflows/coding.md`](../../../plugins/gal-core/workflows/coding.md) にそれぞれ規定されています。

### 能力マトリクス

| Golem | 主な責務 | 代表的な呼び出し例 |
| --- | --- | --- |
| `golem-architect` | トレードオフ、過剰設計、潜在的なバグ要因、API や依存関係のリスクを評価する批判的な設計レビュー。 | `/deep-planning` 実行時に提案のストレステストを行うか、または `/gal architect` を実行します。 |
| `golem-analyst` | ROI、ドメイン知識の整合性、エンドユーザーへの影響を検証するビジネスロジック分析。 | 価格設定、権限管理、請求、顧客向けルールの変更時に実行します。 |
| `golem-designer` | デザインシステム、UX/UI レイアウト、アクセシビリティ指針、およびライブフロントエンド監査。 | UI コンポーネント、ユーザーインタラクション、または開発者体験（DevEx）の調整時に実行します。 |
| `golem-researcher` | ローカルファーストのコードベース調査、複数ソースの統合、エビデンスに基づく事実調査。 | 不慣れな技術や未調査のトピックを調べる際に `/gal research` または `/gal deep-research` を使用します。 |
| `golem-implementer` | 承認されたタスク仕様の実装コードをアトミックな Git コミットで記述します。 | `/gal pipeline` の `CODER` フェーズにおいて動作します。 |
| `golem-tester` | 計画仕様からテストケースを導出し、自動化テストまたは実ブラウザー QA を実行します。 | `/gal pipeline` の `TESTER` フェーズで動作し、独立した検証を駆動します。 |
| `golem-auditor` | 単一タスクに対する集中的なセキュリティおよびパフォーマンス監査を実行します。 | `/gal pipeline` の `AUDITOR` フェーズで動作します。オーケストレーター経由でのみ起動されます。 |
| `golem-debugger` | 科学的手法、問題の再現環境の固定（フリーズ規律）、根本原因の特定を用いた不具合調査。 | 修正に取りかかる前に、リグレッションの原因を切り分けるため `/gal debugger` を実行します。 |
| `golem-steward` | ドキュメント構造の維持、コードと文書の乖離監視、恒久ナレッジの抽出、図の同期。 | `/gal steward` を実行するか、計画作成時、refining 完了時、finalize 時に自動的に起動されます。 |
| `golem-releaser` | デプロイツールの調査とリリース計画の策定。デプロイの実行は行わず、助言のみを提供します。 | `/planning release-<slug>` の前に、`/gal releaser`（隔離モード）または `/releaser discuss`（コンテキスト内）を実行します。 |

**品質保証の三角形（品質トライアングル）**: 検証の責務は、明確に異なる 3 者によって分担されています。**ORCHESTRATOR** はタスクごとの正当性ゲート、最終的な目標検証、および計画ライフサイクルの状態遷移を統括します。**AUDITOR** は個々のタスク差分に対して徹底的なセキュリティとパフォーマンスの検査を行います。**STEWARD** はドキュメントの構造的整合性を守り、恒久的なナレッジの維持を担います。

### Golem 呼び出しマトリクス

| ロール | 直接呼び出しの可否 | 呼び出し構文 |
| --- | --- | --- |
| **architect** | 可能 | `/gal architect`（隔離）または `/architect discuss`（コンテキスト内）。スタンドアロンコマンド: あり。`discuss` 対応: あり。 |
| **analyst** | 可能 | `/gal analyst`（隔離）または `/analyst discuss`（コンテキスト内）。スタンドアロンコマンド: あり。`discuss` 対応: あり。 |
| **designer** | 可能 | `/gal designer`（隔離）または `/designer discuss`（コンテキスト内）。スタンドアロンコマンド: あり。`discuss` 対応: あり。 |
| **releaser** | 可能 | `/gal releaser`（隔離）または `/releaser discuss`（コンテキスト内）。スタンドアロンコマンド: あり。`discuss` 対応: あり。計画段階の助言専用ペルソナ。 |
| **debugger** | 可能 | `/gal debugger`。スタンドアロンコマンドはなく、`discuss` にも対応しません。 |
| **steward** | 可能 | `/gal steward`。スタンドアロンコマンドはありますが、`discuss` には対応しません。 |
| **implementer** | **不可**（オーケストレーター駆動限定） | `/gal pipeline` の `CODER` フェーズを通じてのみ実行。 |
| **tester** | **不可**（オーケストレーター駆動限定） | `/gal pipeline` の `TESTER` フェーズを通じてのみ実行。 |
| **auditor** | **不可**（オーケストレーター駆動限定） | `/gal pipeline` の `AUDITOR` フェーズを通じてのみ実行。旧来の `--finalize-branch-audit` フラグは非推奨であり `COMMAND: error` を返します。Finalize のレビューはこのルートを `gal pipeline <execution-prompt> --phase finalize-review` で再利用し、`Review Independence: full` を記録します。ルートが存在しない場合は、プロセス内レビューが許可され、`Review Independence: DEGRADED_SAME_RUNTIME` が記録されます。ホストサブエージェント (host subagent) モードを明示的に選び、能力チェックに合格した場合は、文書化された `DEGRADED_HOST_SUBAGENT` の例外が許可されます。 |
| **researcher** | **不可**（オーケストレーター駆動限定） | `/gal research` または `/gal deep-research` を通じてのみ実行。 |

オーケストレーター駆動のロールに対して、許可されたパイプラインコンテキスト外から `/gal <role>` を直接実行しようとすると、`COMMAND: error` が返されます。

### コンテキスト内でのロール相談

architect、analyst、designer、releaser は、単独呼び出しとコンテキスト内相談に対応します。

| モード | コマンド | 動作 | 応答タグ |
| --- | --- | --- | --- |
| **隔離**（既定） | `/gal <role>` | ネイティブサブエージェントが実行し、要約と判定だけをメイン会話へ返します。 | `[<role> · isolated]` |
| **コンテキスト内** | `/<role> discuss` | ロールの指示を読み込み、現在の会話で相談を続けます。 | `[<role> · in-context]` |

**ホットジョイン**: 隔離モードからコンテキスト内の相談に切り替えるとき、アシスタントは直前の隔離モードの判定を会話履歴に残します。最初の評価をやり直さずに相談を続けます。

4 つのロールには、それぞれ同名のスタンドアロンコマンド `architect`、`analyst`、`designer`、`releaser` があります。コマンドは任意の第 1 引数 `discuss` を受け付け、大文字小文字を区別せず単語全体で照合します。`discuss` がある場合、コマンドはその単語を 1 回だけ取り除き、残りのテキストを付けて内部サブコマンド `gal consult-script golem-<role>` を実行します。ない場合は、隔離モードで `gal dispatch-script golem-<role>` を実行します。`gal consult-script` は `gal finalize-check` や `gal pipeline-log` と同じ内部コマンドであり、公開の `/gal` サーフェスには含まれません。

スタンドアロンコマンドの表記はランタイムによって異なります。

| ランタイム | 隔離 | コンテキスト内 |
| --- | --- | --- |
| Claude Code | `/gal <role>` | `/<role> discuss` |
| Claude Code プラグインモード | `/gal <role>` | `/gal:<role> discuss` |
| Codex | `$gal <role>` | `$<role> discuss` |

削除された形式 `/gal discuss <role>` と `$discuss-<role>` は使用できません。`/gal discuss <role>` は `COMMAND: error` を返し、代替コマンドを案内します。`/gal <role>` の後ろの最初の内容語が `discuss` の場合も `COMMAND: error` を返し、隔離モードでは実行されません。`$discuss-<role>` スキルは `gal refresh` の後に消え、代わりのリダイレクトはありません。

**直接呼び出しにおける設定の適用範囲**: 4 つのロールの隔離モード直接呼び出しは、`ROUTING_SCOPE: codex-native-projection-only` と `ROUTING_NOTE` を出力します。この注記は次の 3 点を述べます。`planning` 設定は Codex ネイティブエージェントのプロジェクションにのみ反映されること、エグゼキューターは起動されないこと、実際のモデルはホストが決定することです。discuss 呼び出しは `CONSULT_MODE: in-context` を出力し、この注記は出力しません。[Planning グループの適用範囲](./configuration.ja.md#planning-グループの適用範囲)を参照してください。

すべてのレビューロールは [`adversarial-review`](../../../plugins/gal-core/skills/adversarial-review/SKILL.md) の手法を使います。最初にスティールマン解釈を立て、既定では懐疑的に検討し、証拠の基準を守ります。判定は `APPROVE`、`REVISE`、`REJECT` のいずれかで明示します。

### 各フェーズにおける Steward の責務

steward エージェントは、計画フェーズと finalize フェーズにおいて明確に異なる役割を果たします。

- **計画フェーズ（`/deep-planning`）**: 計画文書の構造的妥当性を担保します。ファイル命名規則、必須セクションの網羅性、言語の一貫性、および図の整合性を検証します。このフェーズで steward が `docs/` へ書き込みを行うことは禁止されています。未実装で投機的な設計内容によって公開ドキュメントが汚染されるのを防止するためです。
- **Finalize フェーズ（`/gal finalize`）**: 恒久的なナレッジを公式ドキュメントへ定着させます。steward は、検証完了した計画から得られた知識を `README.md` および `docs/` へ同期し、続いて `.dev/project.md` のインデックスを更新します。このドキュメントコミットが完了するまで、計画ファイルを削除することはできません。

ナレッジの抽出は、機能が完全に実装され検証された後にのみ実行されます。計画策定段階の steward は計画ファイルの書式維持に専念し、作成中の作業ドラフトと正規ドキュメントとの厳格な分離を維持します。

### リリースワークフロー（`release-<slug>`）

リリース作業は、通常の機能パイプラインから独立して動作する専用の計画タイプ（`release-<slug>`）として体系化されています。

1. `/gal releaser` を実行してデプロイの準備状況を評価します。エージェントは読み取り専用の操作で利用可能なスキル、API、CLI ツールを検査し、適切なリリースワークフローを設計します。このコマンドがファイルの書き込み、コミット、または実際のデプロイを行うことはありません。未対応の機能は `not-available` として報告されます。
2. `/planning release-<slug>` を実行し、提案されたリリース手順を `## Tasks` セクションを持つ実体的なタスク計画へ落とし込みます。
3. `/gal pipeline` を実行し、承認されたリリース計画を実行します。
4. `/gal finalize` を実行してリリースの完了を記録し、標準の計画完了手順を完了します。

このプロセスは、GAL 自身の CLI バイナリのパッケージングだけでなく、下流アプリケーションのデプロイ（コンテナビルド、npm パッケージ、Web サービス、クラウドインフラなど）も幅広くサポートします。releaser の責務はアーキテクチャおよび自動化の設計助言にとどまります。実環境へのデプロイコマンドの実行、カナリアリリース、本番環境の監視などを releaser 自身が担うことはありません。

## ワークフローの実行

視覚的なワークフロー図やライフサイクル全体の流れについては、[機能ライフサイクル](README.ja.md#機能ライフサイクル) を参照してください。本節では、手動チェックポイント、人間による判断介入、および復旧手順について解説します。

### 状態と推奨される次の操作の確認

`/gal status` コマンドは、`.dev/state.md` およびアクティブな実行プロンプトを検査し、完全な状態プロジェクションを出力します。アクティブプラン、現在のフェーズ、テスト結果、ブロッカー、継続性ポインター、およびエージェントの準備状況を確認できます。

`/gal whats-next` コマンドは、同様の状態ファイルを検査して「次に実行すべきコマンドは何か」という単一の疑問に端的に答えます。最も近い親ディレクトリの `.dev/state.md` を特定し、最小限のコンテキストとともに推奨コマンドを出力します。状況を手早く把握したい場合は `/gal whats-next` を、詳細な診断情報が必要な場合は `/gal status` をそれぞれ使用してください。

### 計画段階における人間の意思決定

`/planning` コマンドを実行すると、`.dev/plans/<type>-<slug>.md` に単一のソースプランのひな型が作成されます。本文は計画の `planLanguage` で書き、機械用のアンカーは英語のまま残します。`/plan-to-prompt` はこのソースから実行プロンプトを直接生成します。計画フェーズは対話的であり、要件の調整、計画の統合、専門 golem への相談を自由に行うことができます。

**未解決課題（Open Questions）の解決**: どの作成者も `## Tasks` を書く前に、`## Open Questions` 内のすべての課題を解決しておく必要があります。質問は以下の 3 つのクラスに分類されます。

- **Class H**: 人間の操作者が直接判断しなければならない重要事項。
- **Class A**: `architect` ロールが判断理由を明記した上で自律的に解決できる技術事項。
- **Class F**: 軽微な論点や形式的な事項。
クラス表記がない質問は、既定で Class H として扱われます。

**承認の記録**: task 契約が `gal refining-check` に合格した後、人間の決定が計画ファイルの `## Approval` セクションに記録されます。`/plan-to-prompt` コマンドを実行するには、このセクションに `- Human approval: [approved]` が含まれていることが必須です。人間による明示的な承認が記録されていない場合、実行プロンプトの生成は拒絶されます。

**task の大きさとオーナー確認**: `## Tasks` を書く作成者は、task-quality と task-atomicity の規約に従います。`/refining-plan` は任意の作成者の 1 つです。すべてのテストポイントはエージェントが実行します。人間にしかできない確認は、任意の `## Owner Acceptance` セクションに置きます。オーナーだけが満たせる環境条件は、任意の `## Preconditions` セクションに置きます。`/plan-to-prompt` は両セクションを実行プロンプトへバイト単位で同一にコピーします。

**対象計画の明示的指定**: 複数の計画がアクティブな場合は、計画コマンドの引数に対象の計画パスを明示的に指定してください。GAL が複数のアクティブプランの中から自動的に推測して選択することはありません。

### 外部計画のインポート

外部ツールで作成された計画をインポートし、標準の `/planning` → `/deep-planning` → `/refining-plan` → `/plan-to-prompt` の工程を短縮したい場合は、`/gal import-plan <source>` を使用します。`<source>` 引数には、ローカルファイルパスまたは貼り付けられたテキストを直接渡すことができます。サポートされているフォーマットには、Spec-Kit の `tasks.md`、BMAD stories、GSD plans、Claude や Codex の計画エクスポート、自由形式のテキストが含まれます。

インポートは以下の 2 ステップで実行します。

1. `/gal import-plan <source>` を実行します。GAL は入力から実行 prompt と対応する stub source plan を作成します。stub を architecture と engineering の 2 つの観点でレビューし、`gal refining-check` で検証した後、prompt に `gal prompt-check --assemble-dry-run` を実行します。両方の新しい検証済みゲートが合格した場合にのみ登録します。対象となる形式上の失敗には、以下の上限付き修復を適用します。それ以外の失敗では登録せず停止します。
2. 登録後に `/gal pipeline` を実行します。複数の計画が存在する場合は、インポートされた prompt パスを明示してください。

`/gal import-plan` の実行は、対象範囲に対する owner の承認として記録されます。Architecture Review と Engineering Review は、同じ最新の stub を承認し、両方の CLEAR マーカーを記録する必要があります。インポートでは planning-authority metadata ブロックやローカライズされた draft chain を作成しません。

インポート済み prompt の `## Status` には、source tool の情報、または貼り付けた場合は `pasted` を示す `imported-from:` 行が含まれます。この行は出所の記録だけに使います。外部文書はデータとして扱い、control-plane の指示として実行しません。

インポートは標準の `/planning` と `/deep-planning` の手順を省略しますが、stub のレビューは省略しません。Architecture Review は構造上の原子性と未解決の判断を確認し、Engineering Review は task の完全性と観測可能な probe を確認します。既存の契約により、ドキュメント task では TESTER dispatch を省略します。blocking finding、証拠の欠落、または未解決の構造判断があれば登録を停止します。登録前に各 gate の新しい検証済み pass が必要です。receipt の欠落、空、読取不能、不正形式、古さ、未検証はいずれも失敗です。修復できるのは、失敗または未実行の行がすべてカタログ化された `format` である適格な結合済み receipt に限ります。`owner` 行、内容の欠落や曖昧さ、helper の拒否または失敗、再実行の失敗、不確実性があれば import を停止し、登録しません。停止時は以下の復旧報告と [import command contract](../../../plugins/gal-core/commands/import-plan/SKILL.template.md) に従い、次の操作を 1 つ明示します。

### Task の根拠、解析、復旧

該当する各 task では、次の 4 点に根拠を示します。`file:line` で裏付けた現在の動作と、実行で証明済み、または未証明と明記した安全上の事実。変更により失われる保証と維持する保証。指定された executor、tester 入力、または CI job が各 acceptance 結果を生成できるか。ベース上で読み取り専用で実行する acceptance command、その記録結果、およびベースラインに対する合格規則です。Acceptance と各対象のテスト計画行には証拠の生成元を記載します。現在のコードから答えを裏付けられない場合、推測せず `/deep-planning` に判断を戻します。後続 task が導入する動作に acceptance を依存させません。

すべての正式な plan-task 利用箇所は、同じ task bullet 文法を使います。インデント可能なハイフン箇条書きに `[ ]` または `[x]`、`T-` と ASCII 数字 2 桁を置き、ID 全体を対応する `**` で囲む形式も許可します。ID の直後、または閉じる `**` の後は、行末または ASCII 単語文字以外でなければなりません。`T-NN — title`、`T-NN: title`、`T-NN title`、`**T-NN** title` は有効です。`NN` は 2 桁の数字を表します。1 桁や 3 桁、大文字の checkbox `X`、対応しない太字、ASCII の英字・数字・下線が続く形式は無効です。task に見える不正な bullet は診断され、直前の task 本文に取り込まれません。チェック済み状態と必須セクションの方針は各利用箇所が個別に適用します。一般的な synthetic task assembly の動作は維持します。

Rust task の acceptance では、task の明示的な Files 一覧を使います。列挙された `.rs` パスが属する Cargo package は `cargo metadata --no-deps --format-version 1` で特定します。Acceptance 内のリテラルな `cargo test` command を調べ、信頼できる任意の `rtk` prefix を許可します。各 package は `-p` / `--package` または `--workspace` で選択し、該当 package を除外してはなりません。列挙されたファイルに `#[cfg(test)]` module がある場合、未フィルターの package test または `--all-targets` が必要です。`--lib` または `--bins` は、対象 root が直接特定され、selector の対象に含まれる場合だけ有効です。filter、`--no-run`、`--doc`、integration-only の `--test`、実行時の `--ignored` や `--skip` など、test を省く引数は拒否します。不明な wrapper、alias、shell 展開は対象範囲の証明になりません。このチェックは metadata とファイルを調べますが、acceptance command は実行しません。Rust source path がない task はこの範囲チェックに合格します。

失敗または未実行の gate 行を修復できるのは、新しく取得した繫結済み receipt で、該当するすべての行がカタログ化された修復を伴う `format` と示されている場合だけです。`format` は、同じ内容への意味を変えない表示上の変更で既存 validator が通る場合を指します。`owner` 行が 1 つでもあれば、receipt 全体の修復を拒否します。内容の欠落や曖昧さ、古いまたは非結合の証拠、作業上の失敗、不確実性、未対応の不具合は owner 対応です。helper を呼ぶ前に、target が追跡済み、commit 済み、clean で staged entries がないことを確認します。条件を満たさない場合は helper を呼ばず、round を消費せず停止します。owner に target を commit してもらい、元の gate を再実行して、新しく適格となった receipt だけを使います。helper は同時変更された target も拒否します。候補修復は task と requirement の意味・順序、承認、レビュー標識、test 行、ファイル一覧、証拠、status、verdict、未知の内容を維持します。不一致があれば公開前に候補を拒否します。許可された文書修復後は、元の gate を再実行し、新しい検証済み合格を要求します。

1 回の invocation で文書と phase-payload の修復に共有できるのは最大 2 round です。文書 round は helper 1 回の呼び出しで、成功・失敗にかかわらず終了後に 1 round を消費します。phase-payload round は `FORMAT_REPAIR` が出力された場合だけ消費します。`--format-repair-context` は `gal pipeline <prompt>` にだけ渡し、executor の引数や `gal format-repair` には渡しません。2 round を消費したらこの option を省略します。通常の write-back は続きますが、payload 修復は無効です。payload は検証済みの不変 receipt snapshot からメモリー内で導出します。先頭の UTF-8 BOM を最大 1 個削除するか、既存の連続した `Command:` / `Result:` の一意な組に `Evidence:` を追加し、既存 lease の下で通常の validator と配置処理を行います。元の bytes を保持し、両方の hash を記録します。複数候補、孤立 marker、複数の evidence marker がある場合は、round を消費せず修復を拒否します。payload 修復では commit を作成しません。3 回目の round は開始しません。

新しい linked worktree entry では、実行ファイルを bind する前、または attempt を開始する前に、prompt が linked `HEAD` または記録した primary tip のどちらかに存在し、記録した primary tip が linked `HEAD` の祖先であることを要求します。tip が祖先でなく、両方の worktree が clean なら、記録済み commit を通常の `git merge` で統合して再確認します。prompt が両方の commit にない場合、primary checkout に branch がない場合、tip が移動した場合、履歴に共通の祖先がない場合、prompt path が worktree の外または安全でない場所を指す場合、有効な custody がある場合は変更せず停止します。squash、rebase、`--ff-only`、`--no-ff` は使いません。競合時は path を記録し、この merge のみを abort して復元を検証します。復元できなければ残存 path を報告します。merge 成功後は新しい実行ファイル binding と gate 証拠が必要です。この復旧は [`format_repair::recover_primary_tip`](../../../crates/cli/src/commands/format_repair.rs) で実装します。`--terminal-reverify` は読み取り専用で、新しく進んだ primary tip を merge しません。

復旧を停止する場合は、失敗した check、receipt path、receipt 行の要約または `gal format-repair: <reason>` の正確な出力、確認した内容、そして次に行う 1 つの具体的な操作を報告します。失敗した receipt と executor 出力を保持し、残存 path を報告します。古い合格を主張したり、executor 出力を復元したり、無関係なファイルを削除したり、成功した修復 commit を取り消したりしません。停止種別ごとの具体的な次の操作は[bounded format-recovery contract](../../../plugins/gal-core/conventions/token-budget.md#stop-and-next-action) が定めます。

この説明は操作者向けの要約です。詳細な task grounding の契約は[task-quality](../../../plugins/gal-core/conventions/task-quality.md) が定めます。共通 task bullet 文法は [`task_spec`](../../../crates/pipeline/src/task_spec.rs) により適用され、この要約では受理・拒否される形式を示します。文書と payload の修復は bounded format-recovery contract が定めます。linked worktree の復旧は [`format_repair::recover_primary_tip`](../../../crates/cli/src/commands/format_repair.rs) に実装されています。[self-bootstrap](../../../plugins/gal-core/conventions/self-bootstrap.md) は実行ファイル binding と receipt 検証を扱い、linked worktree の復旧は扱いません。

### パイプライン実行: 開始、一時停止、再開

タスクの実行を開始するには `/gal pipeline` を実行します。単一の計画のみがアクティブな場合、GAL はその実行プロンプトを自動的にターゲットとします。複数の計画がアクティブな場合は、対象プロンプトのパス（`.dev/plans/<slug>.prompt.md`）を明示してください。

レガシーまたは手動のオーケストレーター手順では、実装を編集する前に `gal pipeline-preflight <execution-prompt-path>` を実行し、`.dev/pipeline/<plan-scope-key>/preflight.receipt.md` の `overall: pass` を確認します。レシートが失敗、欠落、または未実行の場合、開始を停止します。`--host-session <session-ref>` を指定すると、preflight 時に指定した orchestrator セッションを束縛します。preflight は指定されたセッションの存在、作業ディレクトリの一致、および一意性を確認し、自分でセッションを選ぶことはありません。`--host-session` は `--terminal-reverify` と併用できません。`--dispatch <routed|host-subagent>` と `--host <claude|codex>` でディスパッチモードを固定できます。`host-subagent` には `--host` が必要です。スタンドアロンレシートが開始許可の証拠となるのは、この手動手順に限られます。ガード付きの `codex_stop_v1` セグメントへの開始は許可しません。この preflight の前に、オーケストレーターは任意の `## Preconditions` 表にあるすべての `Check command` を実行します。結果が `Expected result` と一致しない場合、パイプラインは最初の task の前に停止し、未達の行を示します。

Rust リポジトリでは、任意の `## Preconditions` 表に `Check command` が `cargo fmt --all -- --check` となる行を追加し、期待結果を記載する。このチェックが失敗した場合は、最初の task を開始する前に、整形のみの変更をコミットする。ベースラインに未整形のファイルがあると、フォーマッターの変更が task 範囲外に広がり、境界チェックの失敗と追加の修復ディスパッチが発生する。

範囲を限定した Rust task では、変更した `.rs` ファイルだけを `rustfmt --edition <crate edition> --config skip_children=true <changed .rs files>` で整形する。edition には crate の値を指定する。`skip_children` は、rustfmt が `mod` 宣言をたどって他のファイルを処理することを防ぐ。workspace 全体へのチェックは読み取り専用のままにし、変更していないファイルだけで失敗した場合はベースラインの問題として報告する。

`codex_stop_v1` では、ガード付きドライバーが最初に保留中の projection journal を復旧します。次に、実行中の実行ファイルとインストール済みの実行ファイルが、固定された実行ファイルハッシュと一致することを確認します。証明が欠落または不一致の場合、ドライバーは `self-bootstrap` チェックポイントを記録して戻ります。ルートチェックアウトをブートストラップし、チェックポイントが示す許可済みアクションで再開します。証明が一致した場合、ドライバーは `pipeline_preflight::cmd_pipeline_preflight` をプロセス内で呼び出し、そのセグメントに束縛された新しいレシートを要求します。束縛されたレシートが欠落、古い、失敗、不正形式、または書き込み不能の場合、プロバイダーの起動や権威状態の変更の前にセグメントを停止します。

GAL は既定で `legacy_interactive` 継続プロファイルを使用します。このプロファイルは、既存の Claude Code と OpenCode のホスト所有継続、呼び出し構文、プロセス終了動作、projection レイアウト、v1 の証拠、レシート、マーカー、および変更順序の意味を維持します。通常の `gal pipeline <prompt>` 呼び出しはこのプロファイルにとどまります。エグゼキューター名、環境、古いマーカー、または別のホストマーカーの不在によってプロファイルが選択されることはありません。

`codex_stop_v1` プロファイルは、信頼された Codex フックのハンドシェイクが有効な単回使用グラントを提示した後に限り利用できます。プロファイル名はエグゼキューターではなく、継続契約を表します。フックの信頼、準備状態の確認、およびブートストラップ復旧については、[Codex パイプライン実行のセットアップ](setup.ja.md#codex-パイプライン実行のセットアップ) を参照してください。

`gal pipeline <prompt> --require-codex-stop-v1 --status` を使用すると、制限された coordinator 状態を読み取れます。status は読み取り専用です。coordinator、cursor、または attempt ファイルは変更しません。ガード付き継続では、チェックポイントレシートの送信または resume アクションを受け付けます。手動復旧アクションには task と phase の両方を指定する必要があります。現在のゲートが、許可された型付きアクションとその revision の束縛を示します。型付きアクションの種類は次のとおりです。

- `start_task`、`resume_task`、`retry_task`: task の phase を再開します。task ID と phase の両方が必要です。
- `resume_host_subagent`: checkpoint の公開後にドライバーが出力した束縛を使い、ホストサブエージェントの試行を再開します。
- `await_orchestrator`: 指定された `checkpoint_id` のチェックポイントレシートを送信します。その後、オーケストレーターが返された resume アクションを実行します。
- `complete` と `blocked`: 制限された status を読み取るだけです。

`codex_stop_v1` では、`gal pipeline` の各呼び出しが 1 つのブロッキングな機械処理セグメントを実行します。`.dev/pipeline/<plan-scope>/` 配下の永続 coordinator は、prompt の束縛、現在の task と phase、attempt、retry 状態、検証済み証拠、保留中の projection transaction、checkpoint、activation digest、および next action を記録します。セグメントは ORCHESTRATOR チェックポイント、終端結果、人間の権限を要するブロッカー、または回復不能な証拠エラーで戻ります。意味判断をプロセス内で待機しません。

ORCHESTRATOR は task-quality review、boundary-widening judgment、convergence judgment、および goal-backward verification を保持します。task が開始する前に、その task-quality checkpoint が通過しなければなりません。検証済みの進行を projection する前に、boundary と convergence のチェックが通過しなければなりません。最後の task が収束した後、ORCHESTRATOR は `goal_backward` checkpoint を完了します。ORCHESTRATOR checkpoint では、`await_orchestrator` アクションで示されるとおり、その checkpoint、coordinator revision、prompt または specification hash、および commit に束縛されたレシートを送信します。次に、現在のゲートが返した 1 つの型付き resume アクションを実行します。後続の呼び出しは永続 coordinator を再開します。

guarded モードでは、失敗した test または audit の round が retry handoff を書き込み、implement fix mode を通じて再試行します。`--submit-checkpoint-receipt` オプションは `task_quality` と `boundary_widening` の checkpoint を消費します。`pipeline-handback-check` コマンドは `goal_backward` checkpoint を消費します。

`legacy_interactive` のルーティング済み headless 実行では、条件を満たすコミット前の task 境界チェック失敗に対し、task 試行ごとに最大 1 回の境界修復ラウンドを実行する。適用条件は、implement ディスパッチが `completed` で終了し、変更のないベースラインが記録され、OPEN の `IMPLEMENT` retry handoff が存在せず、Boundary Widening Protocol の例外が適用されないことである。executor は implement fix mode で、列挙された許可リスト外のパスを `HEAD` の内容に戻し、列挙された未追跡パスを削除する。task に必要なパスのみ残し、その理由をレシートに記載する。許可リスト内のファイルは変更してはならない。逆方向のガード `boundary-repair-touched-scope` は、これらのファイルを変更したラウンドを拒否し、イベントを記録して出力を保持し、エラーで終了する。境界の再チェックが通れば handoff を解決済みにし、task を続行する。それ以外の結果は `gal-pipeline` 契約の handback 規則に従う。

coordinator の status と復旧アクションを読む場合は、次のガード付きセグメントフローを使用します。

```text
pipeline entry
  -> 保留中の projection journal を復旧
       -> third-hash conflict: target を保持して停止
  -> 固定実行ファイル証明を確認
       -> missing or mismatch: self-bootstrap checkpoint を記録して戻る
            -> ルートチェックアウトをブートストラップし、許可済みアクションで再開
  -> fresh bound preflight pass
       -> fail or missing: provider 起動または権威状態の変更の前に停止
  -> task-quality checkpoint
       -> rejected: task 作業の前に停止
  -> attempt 証拠をディスパッチして検証
       -> incomplete or conflicting: 型付き証拠エラー
  -> 必要な場合は ORCHESTRATOR checkpoint
       -> 束縛されたレシートを送信し、型付き resume アクションを実行
  -> 1 つの型付き遷移
       -> retry / 次の task / task 完了
       -> human-required または終端エラー
  -> 最後の task の後に goal_backward checkpoint
       -> goal-verified または終端検証エラー
```

journal の復旧時に、GAL は target が記録済みの old hash と一致する場合にだけ staged replacement を適用します。target が new hash と一致する場合は、replacement が適用済みであることを記録します。3 つ目の hash がある場合は復旧を停止し、target を保持します。coordinator は journal が projection を committed と記録した後にだけ進みます。新しい preflight レシートが欠落、古い、失敗、不正形式、または書き込み不能の場合、provider 起動や権威状態の変更の前に停止します。稼働中の所有者には読み取り専用で接続します。一致する孤立 coordinator は、検証済みの永続状態から再開します。不明または競合する所有権は型付き conflict で停止します。

パイプラインは、task-quality review、実装と task commit、独立した検証テスト、監査レビュー、3 面の収束、および handback 検査を順次処理します。各 task が収束すると handback checker が次のアクションを返します。すべての task が収束した後、ORCHESTRATOR はプロセス内で goal-backward verification を実行します。新しく検証された `goal-verified` レシートを得ると、パイプラインは最後のノードとして `/gal finalize` を実行します。そのため、通常の 1 回の呼び出しは、計画の着地か、具体的な停止のどちらかで終わります。型付き human-required 判断、未完了の Owner Acceptance 行、または別の拘束力のある停止規則が必要な場合に限り、操作者の介入を求めます。パイプラインの起動は承認の証拠になりません。読み取り専用の terminal-reverify が finalize に連鎖することはありません。

`continue` 判定では、handback アクションは `/gal pipeline <prompt> from T-NN [stop-at T-MM]` です。任意の `stop-at` target は保留中もアクションに残ります。checker はこの閉じた workflow 文法と、指定された復旧アクションだけを受け付けます。終端判定では `continue_action: none` を使用します。`gal.exe pipeline <prompt> from T-NN` 形式は有効な CLI 呼び出しではありません。

再試行（リトライ）ラウンドの実行には、更新された承認と具体的なコード変更が求められます。テストまたは監査で失敗が発生した場合、パイプラインは未解決の再試行引き継ぎメモを記録し、`--fix` フラグを付与して実装ワーカーを再ディスパッチします。後続の試行において、以前とまったく同じ目標、引き継ぎメモ、許可リスト、契約が繰り返された場合、実行開始前に再試行が拒否されます。また、エグゼキューターが正常終了しても許可リスト内のファイルが一切変更されていなかった場合、プロンプトのログやレシートの更新だけでは実質的な作業とみなされないため、`fix-round-no-change` として失敗扱いになります。

パイプラインは、未解決のブロッカーが人間の判断を要する場合、task が 3 回の失敗による retry 上限に達した場合、リポジトリのブランチが分岐した場合、スコープ境界が問題になった場合、または goal-gap check が失敗した場合に停止します。ガード付きモードでは、これらの結果を型付き handback または終端 status として返します。goal verification は、そのレシートが通過した後にだけ `goal-verified` を返します。`human-required`、`retry-ceiling`、`stop-at`、および終端検証エラーはパイプラインを停止させ、実行可能な継続アクションを持ちません。すべての `human-required` 停止は handback ブロックを 1 つ書き出し、`Next human step` に加えて `What to check`、`Expected result`、`Pass/fail rule` を含めます。オーナーはそのブロックだけで合否を判断できます。自動実行パイプラインは対話的な稼働時間の Hard Stop の対象外です。実行時間の上限に達した場合は、中断メモと復旧専用の resume marker を出力します。承認を生成したり、final response を許可したりすることはありません。

レガシーの中断から復旧するには、根本原因を解消して同じ `/gal pipeline` コマンドを再実行します。ガード付きの中断から復旧するには、許可された型付きアクションで同じ prompt と coordinator を再開します。GAL は activation digest と attempt identity が一致する場合にだけ再開します。不明または競合する identity は人間による解決を必要とし、自動的な reclaim、プロセス終了、または再ディスパッチを許可しません。以前の試行が replay detection によって拒否された場合は、引き継ぎメモに異なる是正内容を記録します。完了済みの task は保持され、後続の実行時に自動的にスキップされます。

### タスク実行サイクルと各ロール

各タスク内において、パイプラインは厳格な実行ループ（タスク検証、カーソル更新、コード実装、調整、コミット作成、テスト、監査、収束確認）を巡回します。ORCHESTRATOR はタスク検証を監視し、標準的な Git コミットを作成し、追跡カーソルを更新し、最終完了の準備状況を判定します。

- **タスク検証**: ORCHESTRATOR は、`plugins/gal-core/conventions/task-quality.md` に照らしてタスク基準を検査します。必須要件が欠落している場合、パイプラインはディスパッチ前に停止し、task の作成者が要件を補い、計画がチェックに合格するまで待ちます。
- **実装**: `CODER` は、タスク仕様、エージェント契約、およびリポジトリコンテキストを受け取ります。許可リストに指定されたファイルのみを変更し、Git コミットを作成することなく実行ログを返します。
- **調整とコミット**: ORCHESTRATOR は変更されたファイルが許可リストと一致していることを確認し、`gal boundary-check` を実行した上で、標準の `git commit` を用いて「Task Final Commit」を作成します。
- **テスト**: `TESTER` は「Task Final Commit」を検証し、`TP-NN` の定義に一致するテストケースを実行して、その結果を `## Test Results` 配下に記録します。テスターがコミットを作成することはありません。
- **監査**: `AUDITOR` は「Task Base Commit..Task Final Commit」の変更差分を検査し、エビデンスに裏付けられた指摘事項を `## Review Results` に追加します。
- **収束確認**: ORCHESTRATOR は出力とレシートを評価します。不整合がある場合は担当ロールを再ディスパッチします。テストや監査で失敗した場合は、`--fix` フラグを付けて実装ロールを再ディスパッチし、得られた変更を更新された「Task Final Commit」に記録します。

各ロールは互いに独立して動作します。ディスパッチされたワーカーが、他のロールが生成した内部引き継ぎメモ、生の差分、レシートを直接受け取ることはありません。

### クイックフィックスレーン

クイックフィックスレーンは、計画ファイルも finalize も使わずに、失敗した小さなプロジェクトチェックを修復します。契約は [`gal-quickfix`](../../../plugins/gal-core/commands/gal-quickfix/SKILL.template.md) で、`$gal-quickfix` として呼び出します。

`.dev/project.md` のプロジェクトチェックコマンドが 1 つ以上失敗し、修復が上限値に収まる場合は、`/planning` ではなくこのレーンを選びます。上限値は `quickfix` キーの `maxFiles`、`maxLines`、`maxAttempts` で、[設定](configuration.ja.md#クイックフィックスレーン-quickfix)で説明しています。修復は Protected Path と依存関係ファイルに触れてはいけません。実行中の `/gal pipeline` があってもいけません。修復に設計判断が必要な場合や上限値を超える場合は、`/planning` を使います。

このレーンは Claude Code（`--host claude`）と Codex（`--host codex`）で動作します。手順は次のとおりです。

1. 失敗を再現します。
2. `gal quickfix-run --open --host <claude|codex>` でアテンプトを開きます。
3. 出力されたモデルでホストサブエージェント (host subagent) を 1 つ起動します。ファイルを編集するのはサブエージェントだけです。
4. `gal quickfix-run --close <attempt> --session <host>:<id>` でアテンプトを閉じます。クローズ処理は 2 つの作業ツリースナップショットの差分から変更を測り、コピーではなく実際の作業ツリーでゲートを実行します。
5. 結果をオーナーに報告します。
6. オーナーの確認後、`gal quickfix-run --commit <attempt>` を実行します。
7. オーナーの再確認後、`gal quickfix-run --merge <attempt>` を実行します。

クイックフィックスのコミットは `quickfix/<attempt>` ブランチに作成されます。コミットに含まれるのはアテンプト中に変更されたファイルだけで、`.dev/` 配下のものは決して含まれません。アテンプトが行っていない作業ツリーの変更はそのまま残ります。コミットとクローズの出力は、それらを `OUTSIDE:` 行として一覧にします。マージは元のブランチへの通常の `git merge` です。

### Finalize: 成果物の永続化とクリーンアップ

`/gal finalize` コマンドは、goal-verified になった計画の正式な完了処理を調整します。完全な `gal finalize-check` ゲートは、リポジトリレベルのコマンド、命名、アダプターの安定性、ドキュメントリンク、状態の上限、およびクリーンな作業ツリーを確認します。各 task を再評価することはありません。ゲートはコードやドキュメントを変更せず、検証レシートだけを書き出します。full mode の失敗行は hard stop であり、形式修復の対象ではありません。finalize が管理対象の内容を書いた後に限り、失敗または未実行行がすべて適格なカタログ化済み `format` である新しい結合済み `hygiene-only` receipt は bounded format recovery を使えます。文書と payload に共有する上限は invocation ごとに 2 round で、修復ごとに新しい検証済み gate 再実行が必要です。

**開始経路**:

- 経路 A: オーナーが `/gal finalize` を要求した場合です。handback レシートには `decision: goal-verified` と `voluntary_response_authorized: true` が必要です。finalize の停止後の再開は、すべてこの経路を使います。
- 経路 B: 通常の `/gal pipeline` の完了です。パイプラインが同じ呼び出しの中で同じレシートを生成して検証し、自ら finalize に入ります。2 つ目のコマンドも、マージや削除の前の定型的な確認も不要です。
- 保存済みのレシート、ステータス行、または以前の最終応答だけでは finalize は開始されません。どちらの経路でも、完全モードのゲート、レビュー停止、競合規則、executor 出力の保全、衛生ゲート、および push しない規則は維持されます。

**Owner Acceptance**: 人による承認は、着地の権限とは別です。実質的な finalize 作業の前に、finalize は 2 つの表を読みます。1 つは `## Owner Acceptance` 表です。もう 1 つは `### Handoff Notes` 配下の `#### Owner Acceptance Evidence` 表です。

- 表がない、行がない、またはすべての行が `accepted` か `waived` として記録されている場合、finalize は承認を求めずに続行します。
- 未完了の行がある場合、finalize はその行をそのまま表示し、`Interrupted Phase — finalize / ACCEPTANCE` マーカーを書いて停止します。マーカーには、行、確認すること、期待される結果、合否規則、再開に使う `/gal finalize` コマンドが含まれます。
- 表示された行に対するオーナーの明示的な確認は `accepted` として記録されます。行を指定した明示的な免除は `waived` として記録され、測定された合格として報告されることはありません。単なる finalize 要求や再開の指示は、そのどちらでもありません。
- マージの前、または計画がすでに main ブランチにある場合はティアダウンと削除の前に、finalize は承認済みの対象を再確認します。承認済みの基準または対象に実質的な変更があれば、影響を受ける行の再承認が必要です。無関係なメタデータやドキュメントの変更は、承認を無効にしません。

**独立レビュー**: トップダウンレビューは、`AUDITOR` ルートが選んだ新しい executor で実行されます。ルートと失敗時の規則は [Finalize Review Route](../../configuration.md#finalize-review-route) を参照してください。

- レビュアーが受け取るのは、元の計画の目標、要件、成功基準、テスト計画、および公式コマンドです。ブランチの差分と作業ツリーは、レビュアー自身が読みます。仕様には、プロンプトの履歴、レシート、判定は含まれません。
- 成功には、新しい空でないレシート、`completed` で終わる試行ログ、およびプロバイダーのセッション証拠が必要です。レビューは、`Review Independence: full` を持つ 1 つの管理対象 `### Finalize Review <date>` ブロックとして記録されます。
- `AUDITOR` ルートが存在しない場合は、プロセス内レビューが許可され、`Review Independence: DEGRADED_SAME_RUNTIME` として記録されます。ホストサブエージェントを明示的に選び、能力チェックに合格した場合は、ホストレビューの例外が許可され、`Review Independence: DEGRADED_HOST_SUBAGENT` として記録されます。設定やルートの不備、executor の利用不可、起動の失敗、タイムアウト、証拠の不足、無効な結果は、フォールバックなしで finalize を停止します。
- レビュアーは、コミット、マージ、タグ付け、計画ファイルの削除を行いません。ドキュメント同期、Git 操作、ライフサイクルの権限はオーケストレーターが保持します。

**恒久的な更新処理**:

- ドキュメントの同期に先立ち、変更内容に対して 4 つのレイヤー（L1 事実、L2 ファイル、L3 連携、L4 信頼境界）にわたる要件監査がトップダウンで実施されます。調査結果は `## Review Results` 配下の `### Finalize Review <date>` 表に記録されます。不合格のセルやブロッキングの指摘を含む有効なレビューは、証拠として受け渡され、着地の承認にはなりません。
- オーケストレーターが、`golem-steward` の契約に従ってプロセス内で検証済みの知見を抽出し、`README.md` および `docs/` へ反映します。ドキュメントの内容もオーケストレーター自身がレビューします。機械的なチェックだけでは、ドキュメントの正しさは証明できません。
- メインラインブランチへのマージが実行され、一時的なワークツリーが使用されていた場合は削除されます。

**停止と復旧**: finalize の停止ごとに、`### Handoff Notes` に `Interrupted Phase — finalize` マーカーが残ります。再開の操作は `/gal finalize` です。`DONE` のプロンプトで通常のパイプラインを再起動してはいけません。マージの記録後は、finalize が再度マージすることはなく、ティアダウンとクローズから続行します。

**自動競合解決**: 競合したパスに `.dev/state.md` が含まれる場合、finalize は `gal state-merge` を呼び出します。`gal state-merge` は、サポートされる互いに重ならない行と領域を決定論的にマージし、ほかの未マージパスには手を加えません。解決に成功した場合、残りの競合は[境界付きインデックスの規約](../../../plugins/gal-core/conventions/token-budget.md#dev-findings-hygiene-and-integrity)に従って処理されます。`STATE_MERGE: unresolved` が報告された場合は、`.dev/state.md` を変更せずに処理を中断し、未解決の各箇所を `<file>:<first>-<last>` 形式で、セクション、行キー、base、ours、theirs のテキストとともに示します。権限を持つファイルやほかのパスの競合では、処理を停止します。`STATE_MERGE: rollback-unconfirmed` が報告された場合、失敗した統合を巻き戻して作業ツリーと index を元に戻したことを証明できていません。リポジトリは呼び出し前の状態と異なる可能性があります。この結果は停止として扱い、再試行の前にリポジトリを確認してください。無害な結果とみなしてはいけません。

**成果物のクリーンアップ**:

- `.dev/plans/` 内の計画ファイルは、ドキュメントのコミットが完了し、衛生チェック（hygiene check）をパスした後にのみ削除されます。作業ドラフトを削除する前に、確実に知識を恒久化するためです。
- 計画ファイルが削除された後、finalize は `gal pipeline-clean` を実行して `.dev/pipeline/<plan-slug>/` をクリーンアップします。

**状態の記録**:

- finalize は `.dev/state.md` の `## Recent Close-outs` 表に 1 行を upsert します。列は `Date`、`Plan`、`Landing`、`Result` です。`Landing` は durable-layer のコミットハッシュです。この表は新しい順に最大 2 行を保持します。
- `### Finalize Review <date>` 内のブロッキングでない指摘事項は、最新 5 件を上限として `.dev/state.md` の `## Follow-ups` セクションへ転記されます。
- ローカルの `refs/gal/last-good` 復旧参照を finalize のコミットに更新します。バージョンリリースタグと `vMAJOR.MINOR.PATCH[-PRERELEASE]` のトリガーは変更ありません。

### 復旧参照の移行

`gal restore` は `refs/gal/last-good` を最初に解決します。存在するのにコミットとして解決できない場合は fail closed し、フォールバックしません。正規参照がない場合に限り、正確な旧形式の移行入力 `refs/tags/gal-last-good` を参照します。`HEAD` は推測しません。

`--yes` なしでは、選択した基準と保留中の変更をプレビューし、参照、`HEAD`、作業ツリーを変更せず終了します。旧タグだけがある場合は警告しますが、移行しません。確認実行では、現在の `HEAD` にバックアップブランチを作成し、旧コミットから create-only compare-and-swap で正規参照を作成してから reset と clean を行います。書き込み失敗や競合があれば reset 前に停止し、旧タグは自動変更しません。

削除前に、`refs/gal/last-good` が意図したコミットを指し、復旧が成功することを確認します。その後、必要なら旧タグをローカルで削除できます。リモートの削除は別の手動操作です。ローカル移行と必要なリモート復旧用コピーを先に確認してください。GAL はリモート参照を自動削除、push、強制更新しません。通常の push、fetch、Sync は `refs/gal/*` を転送しませんが、明示的なカスタム refspec と `git push --mirror` は転送する場合があります。

## 検証とレビューの契約

ワークフローの動作は、各領域の正式な契約が定義する。本節では現在の要件を要約し、正式な契約を示す。

### パイプラインの判定と優先順位

Handback の判定は `continue`、`goal-verified`、`human-required`、`retry-ceiling`、`stop-at`、`design-defect` の6種類である。分類の前に証拠を検証する。既存の安全、Protected Path、high/critical、境界範囲による停止が優先される。これらの確認後に `design-defect` を判定し、既存の `retry-ceiling`、`stop-at`、その他の分類より優先する。終端判定には `continue_action: none` が付き、ディスパッチや finalize を許可しない。

### Verification Impact と Verification Map

各タスクは Verification Impact オブジェクトに4つの Boolean フラグを明記する。フラグは `contract_docs`、`code_logic`、`cli_surface`、`cross_boundary` であり、少なくとも1つを true にする。true のフラグが要求するレイヤーの和集合を使う。`contract_docs` は `contract-doc`、`code_logic` は `unit`、`cli_surface` は `cli`、`cross_boundary` は `integration` を要求する。Map には追加の有効なレイヤーも指定できる。レビュー担当者はフラグがタスクを正しく表すか確認する。パーサーは構文と網羅性を検証する。

Verification Map には Task、Layer、Proof Command、Expected、Producer、Evidence の6列がある。各タスクは、必要な各レイヤーについて少なくとも1つの有効な行を持つ。各行には既知のタスク ID とレイヤーを指定し、すべてのセルとコマンドを埋め、タスクとレイヤーの組み合わせを重複させない。Preflight と prompt check は Proof Command をテキストとして扱い、実行しない。

### Red-proof とリポジトリ検証

動作を変更する場合、独立した TESTER が動作テストを作成し、変更していない同一テストを Task Base Commit と Task Final Commit の分離スナップショットで実行する。Red-proof には、base で対象のアサーションが失敗し、final で成功することが必要である。コンパイル失敗、テスト選択数がゼロの場合、環境エラーは red-proof に該当しない。ドキュメントのみのタスクでは、動作 red-proof が適用されない理由を記録する。TESTER は正式な final commit を変更しない。

リポジトリ検証ドライバーには Launch、Doctor、Drive、Evidence、Cleanup、Helpers の6セクションがある。インデックス付き機能の手順、エンドツーエンドの証明、ドリフト保守ループを含む。Cleanup は絶対パスのルートを検証した後、今回の試行で作成したディレクトリだけを削除できる。証拠は削除対象の外に置き、既存の worktree を削除しない。保守上の失敗は `doc-drift`、`harness-gap`、`product-gap` に分類し、ドキュメントの成功として扱わない。

### トリミング、削除、コメント

コード、設定、契約文をトリミングまたは削除する前に、ソース位置から保護対象の動作と条件を特定する。直接参照と動的な実行経路を確認し、変更後にも同等のガードが残ることを確かめる。検索結果は手掛かりであり、削除の根拠ではない。位置を特定した証拠が条件の無条件化を支持しない限り、条件付きガードを維持する。到達可能性または同等性が不明な場合は変更を延期し、`upgrade when <condition>` と観測可能なトリガーを記録する。

コメント禁止ルールは追加または変更した内容に適用する。5つの例外は、法的表示、公開 API 文書、安全不変条件、外部互換性、必要なツール指示である。制約を型、アサーション、テストで表現してから、その制約で不要になった文章だけを削除する。必要な安全知識と条件は残す。新しい抑制指示には理由を記し、監査する。既存の抑制範囲を広げない。`MUST KILL` は範囲を限定した提案であり、作業範囲の拡張を許可しない。

### Mistake Register と Consumer Matrix

Mistake Register には `Recurring Mistakes` と `Subsystem Failure Modes` の2節がある。レビュー担当者はレビュー前に読み、根拠に基づく内容だけを wrap-up または finalize で更新する。同じ証拠から重複項目を作らず、原因が不明な場合に確定した原因として記載しない。

共用 trait、CLI シグネチャ、テンプレート契約を作るタスクは、直接および間接の利用者を含む Blast-Radius Consumer Matrix を用意する。利用者はタスク対象ファイルまたは互換性の証拠で示す。計画がどのワークフローで作成されたかにかかわらず、すべてのアーキテクチャレビューで4つの設計上のレッドフラグを確認する：Split ownership、Two ways、Hand-synced list、Importable internals。4つの Verification Impact フラグは必要な検証レイヤーを決め、prompt に明記する。計画テンプレートがフラグの文法を定義し、prompt 生成時にフラグを保持する。レビュー担当者は宣言が実態を反映しているか確認する。

### Finalize の L4 結果

該当する各 L4 セルには `new-trust-boundaries` と `guard-preservation` の両方を記録する。各結果には `result`、空でない `reason`、空でない `evidence` の位置が必要である。結果には `pass` または `not-applicable` を使う。両方が pass の場合、外側のセルは `PASS` とする。両方が not-applicable の場合に限り `N/A` とする。外側の失敗を内側の結果で覆すことはできない。証拠の位置はリポジトリ相対パスと正の行番号で示し、ファイルと行が存在する必要がある。

[パイプライン実行](../../workflows.md#pipeline-execution-start-pause-resume)、[finalize](../../workflows.md#finalization-artifact-persistence-and-cleanup)、および正式な契約 `plugins/gal-core/conventions/task-quality.md`、`plugins/gal-core/workflows/coding.md`、`plugins/gal-core/commands/gal-finalize/SKILL.template.md` を参照する。

## ヘッドレスディスパッチ

### ホストサブエージェントディスパッチ

ディスパッチモードは preflight pin とルートから決まります。無効なルートは常に停止します。現在のセッションで会話によって要求されると、モードは `host-subagent` に固定され、有効なルートより優先されます。それ以外の場合、ルートがあれば `routed`、ルートも固定ホストもなければ `text` を使います。`finalize-review` では、AUDITOR ルートがあれば `routed` のままです。`/gal` コマンドは、[パイプライン skill](../../../plugins/gal-core/commands/gal-pipeline/SKILL.template.md#dispatch-mode) に記載された切り替えと復帰のフレーズを認識します。ルート実行に戻るには、未解決のホストサブエージェント試行がないことと、新しい `run_id` を得る preflight の再実行が必要です。既存の run pin は変更できません。

各タスクフェーズは、2 段階のホストサブエージェント試行を使います。オープンでは、プロンプトとレシートのリースを取得し、スナップショットと不変の manifest を保存してから、started ログと `HOST_SUBAGENT_ATTEMPT:` 行を公開します。オーケストレーターは、具体化された spec パス、エグゼキューター (executor) と同じフェーズ契約、正確なタスク範囲を渡して、汎用のホストサブエージェント (host subagent) を 1 つ起動します。`golem-implementer`、`golem-tester`、`golem-auditor` は使いません。サブエージェントは commit や push をせず、実行プロンプトも編集しません。テストと監査の結果はレシートだけで返します。返却後、クローズは manifest の同一性、リースの所有権、ホストセッションの証拠、プロンプトのダイジェスト、作業ツリーの変更、新しいレシートのスナップショットを検証してから、意味的な配置と最終ログの公開を行います。オーケストレーターは、ホストツールの最終結果を受け取ってからクローズします。タイムアウトはクローズの根拠になりません。preflight、タスク品質、境界、収束、リトライ、commit、最終検証の判断は、サブエージェントの権限ではありません。

dispatch-offload になれるのは、有効で完了したホスト試行だけです。PASS には、新しい空でない通常のレシート、`completed` で終わる試行ログ、検証済みのホスト証拠、プロファイルの管理チェックが必要です。no-writeback の終了状態だけでは PASS になりません。オーケストレーターは `Verification Independence: DEGRADED_HOST_SUBAGENT` を記録します。finalize-review の例外では `Review Independence: DEGRADED_HOST_SUBAGENT` を記録しますが、4 層レビューとその証拠要件はそのまま残ります。[finalize skill](../../../plugins/gal-core/commands/gal-finalize/SKILL.template.md) を参照してください。

ホストサブエージェントの試行は、次の終了ステータスのいずれかで終わります。各ステータスは試行ログのヘッダーに記録されます。PASS につながりうるのは `completed` だけで、ほかのステータスはすべて PASS を妨げます。

| ステータス | 説明 |
| --- | --- |
| `completed` | クローズがホストセッションの証拠を検証し、作業ツリーまたはレシートの証拠を確認して、レシートを配置した状態。 |
| `no-writeback` | `implement` フェーズで作業ツリーが変化しなかった状態。 |
| `host-session-invalid` | ホストセッションの検証に失敗した、またはモデルポリシーが適用される状況でモデルの証拠がなかった状態。 |
| `model-tier-mismatch` | ホストセッションのモデルまたはエフォートが、選択された tier と一致しなかった状態。 |
| `placement-failed` | レシートの配置が完了しなかった状態。 |
| `lease-owner-mismatch` | リースの所有者が試行の所有者と異なる状態。 |
| `lease-missing` | 試行のリースが見つからない状態。 |
| `identity-mismatch` | manifest の同一性が要求された試行と一致しない状態。 |
| `terminal-replay` | 試行がすでに終了状態に達しているのに、クローズが繰り返された状態。 |

`started` は実行中の試行を示します。終了ステータスではありません。

オーケストレーターは、`HOST_SUBAGENT_ATTEMPT:` 行の `model=` の値でサブエージェントを起動します。`--tier` と `--tier-reason` は、理由を添えた手動の上書きの場合に限り渡します。タスク内容が tier を決めることはありません。ホストの tier 設定は、どの tier でもそのホストが対応するモデルを受け付けるため、deep tier で Sol や Astra を使えます。明示的な inherit は劣化として扱います。能力チェックはホストごとに異なります。失敗した場合は代替なしで拒否されます。

legacy 実行では `--close` と `--session` で試行をクローズします。guarded 実行では、既存の GAL パイプラインプロンプトの `--require-codex-stop-v1` JSON アクション入口を通じて、`kind: resume_host_subagent` と、永続的な checkpoint の公開後にドライバーが出力する正確なバインディングを使ってクローズします。

試行 ID は、`host_<pid>_<nanos>` のように英小文字、数字、アンダースコアだけを使います。オーケストレーターがこの ID を Codex の `spawn_agent` の `task_name` として渡し、この欄はほかの文字を受け付けないためです。セッション参照は、ホストツールが返したサブエージェントを指します。Claude Code では `claude:<agent-id>` です。Codex では `codex:<agent-path>` で、`spawn_agent` が返す `/root/<task_name>` のようなパスを使います。`codex:<rollout-id>` も使えます。クローズは、同じ作業ディレクトリで `session_meta` にその agent path または rollout ID が記録された Codex rollout を探します。Codex のサブエージェントは、自身の agent path 宛ての `agent_message` としてタスクを受け取り、クローズはこのメッセージを試行のバインディングとして受け入れます。クローズは rollout ID を `session_id` として記録します。セッション検証に失敗した場合、クローズは `working directory mismatch` や `attempt id absent from user message` のような型付きの原因を出力し、試行を `started` のまま残します。

記録レイアウトには、それぞれ別のバージョン規則があります。試行ログは `GAL-DISPATCH-LOG v1` ヘッダーと標準のディスパッチログフィールドを維持します。不変の JSON manifest は `state` と `sha256` を持つエンベロープで、manifest 自体に独立したスキーマバージョン欄はありません。`state` には、`attempt_id`、作業ツリーとプロンプトのパス、タスク、フェーズ、プロファイル、run ID、ホスト、モード、spec のパスとダイジェスト、レシート／ログ／スナップショットのパス、開始時刻、モデル、effort、tier とその出所、上書き理由、ポリシー状態、前提証拠の参照、失敗履歴、観測されたモデルと effort、プロンプト／レシート／ログ／スナップショット／作業ツリーのダイジェストが入ります。ホスト試行の証拠契約は v1 を使い、既存の v2 検証は変わりません。ベンダーの transcript と rollout のレイアウトはホストローカルの形式であり、安定したベンダー契約ではないため、未知のホストレイアウトは常に失敗として閉じます。

guarded コーディネーターの状態には独自の `schema_version` があります。その `host_subagent` 同一性には、checkpoint ID と revision、run ID、試行 ID、任意のセッション参照、最終状態のダイジェストが記録されます。型付きの `resume_host_subagent` バインディングには、checkpoint ID、期待される revision、run ID、タスク、フェーズ、試行 ID、セッション参照、プロンプトのダイジェスト、実行バインディングのダイジェスト、任意の commit が含まれます。ドライバーは、checkpoint の永続的な公開後に限り、正確なバインディングを出力します。legacy クローズは `--close` と `--session` を使い、guarded クローズは既存の `--require-codex-stop-v1` JSON アクションを使います。不正、古い、不完全、または未解決の試行は、PASS にもルート実行への移行にもなりません。

### アーキテクチャの概要

GAL は、パイプラインの特定のフェーズをセカンダリのヘッドレスコーディング CLI へ委譲することができます。委譲の設定は `~/.gal/config/config.json` 内の `executorRouting` で行います。スキーマ定義、組み合わせレジストリ、フォールバックロジックについては [設定](configuration.ja.md#エグゼキュータールーティングexecutorrouting) を参照してください。本節では実行メカニズム、診断、復旧について解説します。

### ロール名簿

| ロール | 所属グループ | スコープ |
| --- | --- | --- |
| `CODER` | pipeline | アクティブタスクで指定されたコード変更を実装します。`TESTER` とは異なるモデルで実行することが推奨されます。 |
| `TESTER` | pipeline | 公開 API や仕様に対するテストを作成・実行します。`CODER` とは異なるモデルで実行することが推奨されます。 |
| `AUDITOR` | pipeline | パフォーマンス特性やセキュリティ境界を評価します。`CODER` と同等以上の性能を持つモデルで実行する必要があります。 |
| `ARCHITECT` | planning | 設計のトレードオフや潜在的なアーキテクチャリスクを批判的に評価する計画レビュー。 |
| `ANALYST` | planning | ドメインルール、ROI、ユーザーワークフローを検証するビジネスロジック分析。 |
| `DESIGNER` | planning | ユーザー体験、UI スタイリング、開発者ツールのレビュー。 |
| `RELEASER` | planning | デプロイおよび DevOps フローの設計。読み取り専用の助言役。 |
| `RESEARCHER` | research | 3 つの並列ワーカー（`RESEARCHER#0`、`#1`、`#2`）の 1 つとして独立した調査を実行します。 |

`pipeline` グループはヘッドレス CLI ディスパッチを使用します。`planning` グループは Codex ネイティブエージェントのプロジェクションに `model` と `effort` を提供するだけです。ヘッドレスディスパッチは行わず、エグゼキューターも起動せず、`timeoutSecs`、`sshTarget`、`remoteWorkdir` は無視されます。`research` グループは、設定された最大 2 つのモデルにわたって並列のヘッドレスワーカーを実行します。

### アダプター CLI の挙動

- **opencode**: ファイル書き込み権限を付与するため `--agent build` を付与し、権限プロンプトを自動承認するため `--auto` を付与してディスパッチします。
- **GitHub Copilot**: プロンプトのトークン予算内に収めるため、`--no-custom-instructions` と `--disable-builtin-mcps` を付与します。また、dispatch が `~/.copilot/mcp-config.json` から解決したホスト MCP サーバーごとに `--disable-mcp-server <name>` を 1 つずつ付与します。Copilot Free では `model: auto` が必須であり、ローカル実行モードでのみ動作します。

### セキュリティ上の警告: 権限のバイパス

ヘッドレスディスパッチでは、ランタイム固有の権限オプションが使用されます。Claude Code および Google Antigravity には `--dangerously-skip-permissions`、opencode には `--auto`、GitHub Copilot には `--allow-all` を指定します。これらのオプションは、ローカルファイルシステムやターミナルに対する広範なアクセスを許可する可能性があります。GAL は Codex を `--dangerously-bypass-approvals-and-sandbox` 付きで起動するため、Codex の子プロセスもサンドボックス外で実行されます。Codex の Windows サンドボックスは現在、コマンドを実行する前に失敗します。詳細は [セットアップ](setup.ja.md#既知の問題-codex-windows-サンドボックスの障害) を参照してください。Codex のモード選択、拒否からの復旧、およびロールバックの詳細については、[セットアップ](setup.ja.md#codex-パイプライン実行のセットアップ) を参照してください。

エグゼキュータールーティングの有効化は、信頼できる開発環境かつ安全なリポジトリ内でのみ行ってください。信頼できないソースコードや未検証のタスク契約に対して自動ルーティングを実行してはなりません。エージェント向けのプロンプトには `git commit` や `git push` を実行しないよう指示が含まれていますが、この制約はプロンプト上の指示によるものであり、OS レベルの制御によって強制されているわけではありません。

### パイプライン実行順序と証拠

パイプラインの試行（attempt）は、同一の正規プロンプトに対する書き込み操作を prompt-writer リース（prompt-writer lease）を用いて直列化します。オーケストレーターは、プロンプトのスナップショットを読み込み変更可能なフェーズ状態を準備する前に、このリースを取得します。続いて receipt リースを取得し、レシートの配置、終了エビデンスの収集、およびプロセスのクリーンアップ完了が確認されるまで両方のリースを保持します。リースは取得時と逆の順序で解放されます。独立したプロンプト同士が同じ prompt-writer リースを共有することはありません。GAL がロックの経過時間のみに基づいて放棄されたリースを再取得（回収）することは決してありません。リースの復旧には、所有権の確認、プロセスツリーの終了確認、および承認された有界クリーンアップの実行が必要です。クリーンアップを確認できない場合、GAL はすべてのリースと復旧用成果物を保持します。

試行は receipt リースの保持中に、検証済みレシートのバイト列とそのダイジェストを取得・保存します。フェーズの書き戻し処理はこの不変スナップショットを直接利用し、解放済みのディスク上レシートパスを再読み込みすることはありません。最終ゲート検査では、ディスク上の証拠が取得したスナップショットと一致することを検証します。

パイプラインゲートは、現在の試行から得られた証拠のみを評価します。バイナリの利用可能性（availability）、プロセスの起動、プロバイダーの応答、プロセスの終了状態、レシートの検証、およびゲートの判定結果は、それぞれ独立した段階として評価されます。空のログ、実行中のワーカープロセス、あるいは応答のないアイドル状態だけで、成功や失敗が判定されることはありません。`PASS` 状態を得るには、3 つのパイプライン条件をすべて満たした上で、独立した監査を通過する必要があります。再試行時にはその試行自身の独立した証拠が必要であり、オーケストレーターの承認によって監査の欠落を代替することはできません。

ワークフローの継続（`from <task>` 経由）と、実行可能なディスパッチ（明示的なフェーズ引数およびタスク引数を使用）は、互いに独立したインターフェースです。不正または古い継続入力は、プロセス起動前の検証でエラーとなり、最初のタスクの実装へフォールバックすることはありません。stop-at などの実行境界の意図は、厳密にワークフロー層にとどまります。ツールの処理が yield した後は、オーケストレーターが元のホストセッションの完了を待ちます。この契約については、`gal-pipeline` スキルの `Headless Executor Dispatch` を参照してください。

```text
Availability: absent -> Missing; observed denial -> Denied
  incomplete or conflicting lookup -> Unknown; usable candidate -> launch
  -> provider response -> terminal result + current receipt -> smoke result
  [native approval only for an explicitly scoped external comparison]

Outer Codex permission context -> native approval when required
  -> validate input -> prompt-writer lease -> immutable prompt snapshot
  -> receipt lease -> started evidence -> spawn child
       -> child runs without the Codex sandbox
       -> terminal result -> receipt placement -> final evidence
  -> release leases in reverse order after confirmed cleanup
  -> three-condition PASS + independent audit -> next serial phase

Denied request -> truthful failure + recovery point
Unknown cleanup -> retain leases and artifacts; no automatic takeover
Host tool yield -> wait on the original host session
  -> if exit remains unconfirmed: write Interrupted Phase, keep output, do not re-dispatch
```

オフラインでの決定論的テストは、必須となるパイプライン検証のベースラインを構成します。任意のライブ統合確認を行うには、事前に承認された使い捨ての環境において明示的な opt-in が必要です。Desktop のモード選択、ユーザーログイン、実環境での動作に関する開発者受け入れテストは、独立した証拠として追跡されます。手動確認またはライブ確認が未実施の場合は、`PASS` ではなく **NOT RUN** と記録されますが、これによってパイプラインの実行や finalize がブロックされることはありません。これらの確認を実施する際は、[セットアップの開発者受け入れチェックリスト](setup.ja.md#開発者受け入れチェックリスト) に従ってください。

### リモートディスパッチ（SSH レーン）

リモート実行は、専用コマンドではなく `executorRouting` 内で直接設定されます。`/gal pipeline` ワークフローの挙動は、ワーカーがローカルで実行されるかリモート SSH セッション経由で実行されるかに関わらず完全に同一です。

**ホストの前提条件**（ディスパッチ前に手動で準備しておく必要があります）:

- 接続先システムは、非対話型の SSH 認証（`BatchMode=yes`）をサポートしている必要があります。パスワードやパスフレーズの対話プロンプトが表示された場合は接続失敗として扱われます。
- 接続先のリモートマシンに対象のコーディング CLI（`claude`、`codex`、`agy`、`opencode`）がインストールされ、ログイン済みである必要があります。リモートホスト上に `gal` バイナリを導入する必要はありません。
- ディスパッチを開始する前に、`remoteWorkdir` で指定されたリモート側のチェックアウトがローカル Git の `HEAD` コミットと一致し、クリーンな作業ツリーである必要があります。

**実行フロー**: リモートディスパッチは、SSH セッションを通じて同期的に実行されます。コード変更を伴うタスクが正常終了した後、GAL は未コミットのリモート側の変更を取得し、ローカルの管理リポジトリへ適用します。ローカルへの変更適用がクリーンに成功した後にのみ、GAL はリモートリポジトリをリセット（`git reset --hard && git clean -fd`）します。ローカルへの変更適用が失敗した場合は、トラブルシューティングのためにリモートリポジトリの状態がそのまま保持されます。

`remoteWorkdir` の設定値は、必ず **専用の GAL チェックアウト** を指すようにしてください。日常の開発で使用している作業ディレクトリを指定してはいけません。タスク完了後のクリーンアップ処理によって、未コミットの作業ファイルが恒久的に破棄されるためです。

**運用上の制約**: GitHub Copilot は CLI パラメーターの制限によりリモート SSH 実行をサポートしておらず、リモート Copilot ディスパッチを試みるとエラーになります。Windows のリモートホストは、実行環境として POSIX シェルを前提としているためサポートされていません。SSH 接続における自動再接続やタスク途中での再開はサポートされておらず、SSH セッションが切断された場合、タスクは直ちに失敗します。リモート実行は単一タスクを順番に実行します。各タスク完了後にローカルリポジトリの `HEAD` コミットが進むため、後続のリモートディスパッチを実行する前にリモート側のチェックアウトを再同期する必要があります。

### ワーカーの分離境界

`/gal pipeline` がタスクをヘッドレスエグゼキューターへディスパッチする際、セカンダリのエージェントはワークフローのオーケストレーターではなく、単一の実行ワーカーとして機能します。セカンダリのエージェントがリポジトリ内の指示（ワークフロー遵守マーカーなど）を「パイプラインコマンドを実行せよ」という指示であると誤認するのを防ぐため、GAL は以下のロール分離を徹底しています。

1. **タスク仕様のスコープ制限**: レンダリングされるタスク仕様には、`## Task Goal` の直前に `## Dispatched Worker Boundary` ブロックが埋め込まれます。このセクションは、エージェントの活動範囲を割り当てられたタスクのみに制限します。オーケストレーターゲートは中央で一元管理されるため、`gal` のサブコマンド（`gal pipeline-preflight`、`gal pipeline-handback-check`、`gal boundary-check`、`gal pipeline` を含む）を実行することは厳格に禁止されます。また、ワークフロースキルの読み込みも禁止されます。ディスパッチされたワーカーは、指定されたタスクの目標、ファイルの許可リスト、およびインラインの `## Agent Contract` にのみ従わなければなりません。
2. **リポジトリ指示の抑制**: セカンダリの CLI は、リポジトリ内の指示ファイル（`AGENTS.md` や `CLAUDE.md`）をシステムプロンプトへ自動ロードすることがよくあります。対応しているツールにおいて、GAL はリポジトリレベルの指示を抑制するフラグを渡し、自己完結したタスク仕様のみがモデルに与えられるようにします。

| エグゼキューター | リポジトリ指示の露出 | 抑制メカニズム | 検証バージョン | 挙動 |
| --- | --- | --- | --- | --- |
| **codex** | 露出（`AGENTS.md`） | `-c project_doc_max_bytes=0` | codex-cli 0.149.1 | `-c project_doc_max_bytes=0` を渡してローカルの `AGENTS.md` を抑制。ユーザーレベルの `~/.codex/AGENTS.md` は維持。 |
| **copilot** | 露出 | `--no-custom-instructions`, `--disable-builtin-mcps`, `--disable-mcp-server <name>` | Copilot CLI | リポジトリ指示、組み込み MCP、解決されたホスト MCP サーバーを無効化し、自己完結したプロンプトコンテキストを維持。 |
| **claude** | 露出（`CLAUDE.md`） | `--setting-sources user` | Claude Code 2.1.251 | `--setting-sources user` を渡してローカルの `CLAUDE.md` とリポジトリ設定を無視しつつ、ユーザー設定を維持。 |
| **opencode** | 未確定 | なし | opencode 1.18.25 | テスト時の調査がタイムアウトしたため、抑制フラグは未適用。分離状態は未検証として扱われます。 |

## ディスパッチのログ記録と診断

ディスパッチ処理は、タスクごとに以下の 2 層の永続的な運用ログを生成します。

### レイヤー 1: GAL エグゼキューターログ

エグゼキューターログは、サポートされているすべてのツールにわたって標準化された監査証跡を提供します。ログは `.dev/pipeline/<plan-slug>/<task>/`（手動ディスパッチの場合は `.dev/pipeline/<yyyymmdd>/test-direct/`）配下に、`<timestamp>-<attempt>-<task>-<phase>-<executor>.log` という命名規則で保存されます。ファイルヘッダーには終了ステータス、終了コード、解決されたモデル名、Git コミットハッシュ、プロバイダーのセッション識別子が記録されます。`---STDOUT---` セクションには、メッセージ、シェル実行、ファイル編集、トークン消費量など、プロバイダーの完全なイベントストリームが記録されます。エージェントの動作を評価する際は、必ずこれらのログを確認してください。

ログヘッダーは、実行結果を以下の標準終了ステータスに分類します。

| ステータス | 説明 |
| --- | --- |
| `completed` | プロセスが終了コード 0 で終了し、レシートが確認され、作業ツリーの変更が期待値と一致した状態。 |
| `no-receipt` | プロセスは終了コード 0 で終了したが、期待されるレシートファイルが存在しないか、空であるか、読み取れない状態。 |
| `workdir-escape` | 過去のログ分析用に残されている旧式ステータス。現在のディスパッチャーからは出力されません。 |
| `no-writeback` | プロセスは終了コード 0 で終了しレシートも返されたが、作業ディレクトリに変更が一切含まれていない状態。 |
| `timeout` | 設定されたタイムアウト制限を超過したため、プロセスが強制終了された状態。 |
| `timeout-no-output` | プロセスが出力を一切出力しないままタイムアウトした状態。 |
| `timeout-midrun` | プロセスが部分的な出力を出力した後にタイムアウトした状態。 |
| `disconnected-partial` | プロセスが 0 以外の終了コードで異常終了した状態。 |
| `unavailable` | 要求されたエグゼキューター CLI がシステムの `PATH` 上に見つからない状態。 |

本表は、エグゼキューターのディスパッチの終了ステータスを示します。ホストサブエージェントの試行は同じログヘッダーを書き込み、独自の終了ステータスとして `completed`、`no-writeback`、`host-session-invalid`、`lease-owner-mismatch`、`lease-missing`、`identity-mismatch`、`terminal-replay`、`placement-failed`、`model-tier-mismatch` を使います。なお、`started` は実行中を示すマーカーであり、終了ステータスではありません。

`no-writeback` ステータスには、以下の明確な判定境界が存在します。

- **フェーズの対象範囲**: `implement` フェーズに対してのみ厳格に適用されます。`audit` や `test` フェーズには適用されません。これらのフェーズは追跡対象ファイルを変更するのではなく、`.dev/pipeline/` 配下のレシートに結果を出力するためです。
- **コンテンツハッシュによる判定**: 実行の前後でファイルが同一の `git status --porcelain` マーカーを保持している場合（事前に変更されていたファイルなど）、変更検知は SHA-256 コンテンツハッシュを比較して、実質的な編集を誤って `no-writeback` と誤認しないようにします。
- **無視されたパス**: すべての変更が `.gitignore` 対象のパス内にとどまっている場合、作業ツリー監視はバージョン管理対象ファイルのみを追跡するため、実行は `no-writeback` として報告されます。
- **契約との結合**: ステータスの分類はフェーズの成果物契約に依存します。`implement` はバージョン管理対象ファイルを通じて成果物を提出し、`audit` と `test` はレシートを通じて成果物を提出します。

### `contract` および `contract_source` メタデータの追跡

プロンプト駆動のディスパッチでは、`Dispatch:` ログマーカーおよびヘッダーセクションに `contract=<path> contract_source=workdir|ancestor|exe-side|embedded` が記録されます。これらのフィールドは、ワーカーが実行したエージェント契約の正確な出所を特定します。

- `workdir` / `ancestor`: ローカルの GAL チェックアウトから読み込まれた契約。
- `exe-side`: 実行中の `gal` バイナリの隣にあるファイルから読み込まれた契約。
- `embedded`: `~/.gal/embedded-src` に展開された組み込みデフォルトから読み込まれた契約。

素の `gal dispatch` 実行や直接のタスク仕様ディスパッチでは、出所階層を評価しないため、これらのメタデータタグは省略されます。

### 推論エフォートメタデータ（`effort`）

出所追跡を伴うディスパッチでは、ログヘッダーに `effort=<value|(default)>` パラメーターが追加されます。この値は初期のルート選択時に決定され、タスク全体を通じて一定に保たれます。ルーティングを行わないディスパッチでは、後方互換性を保つためこのパラメーターは省略されます。ルーティングによって `OFFLOAD` ディレクティブが生成された場合、`gal dispatch-script` はオーケストレーター出力用の標準レポート行（`Dispatched: <phase> <T-NN> - <ROLE> as <executor>, model <model>, effort <effort>`）をレンダリングします。

### レイヤー 2: ネイティブセッションの再開

ログヘッダーにはプロバイダーの `session_id` が記録されており、操作者は各 CLI のネイティブインターフェースでセッションを再開できます。

| エグゼキューター | ネイティブ再開コマンド | 既定の可視性 |
| --- | --- | --- |
| **claude** | `claude --resume <session_id>` | セッション履歴に表示。 |
| **codex** | `codex resume <uuid>` | 既定では非表示。メニューに表示するには `--include-non-interactive` を、ディレクトリフィルターを無視するには `--all` を使用。 |
| **opencode** | `opencode run -s <session_id>`（または `opencode export <session_id>`） | セッション履歴（`opencode session list`）に表示。 |
| **copilot** | `copilot --resume=<session_id>` | `~/.copilot/session-store.db` に保存。ID による再開が可能だが一覧表示コマンドはなし。 |
| **agy** | `agy --conversation <uuid>` | `~/.gemini/antigravity-cli/brain/<uuid>/` に保存。ID による参照のみ。 |

自動検証のエビデンスとしてはレイヤー 1 のエグゼキューターログを参照してください。ネイティブのランタイムで手動介入が必要となった場合は、レイヤー 2 のコマンドを使用します。

### エグゼキュータースモークテスト（`gal doctor --executor-smoke`）

`gal doctor --executor-smoke` コマンドは、`codex`、`claude`、`copilot`、`agy`、`opencode` におけるヘッドレスディスパッチワークフローを包括的に検証します。独立したテストルーティング設定を使用して、1 行のレシートを書き込む最小限の検証タスクをディスパッチします。検証結果は `.dev/pipeline/<yyyymmdd>/test-executor-smoke-<HHMMSS>Z/local/` 配下に記録され、JSON 要約、可読ステータス表、生ログが出力されます。

| ステータスコード | 説明 |
| --- | --- |
| `PASS` | ディスパッチに成功し、レシートが検証された状態。 |
| `NOT_INSTALLED` | 対象の実行コンテキストで CLI の不在が確認された状態。コマンド探索が拒否された場合や結論が出ない場合は、未インストールとは判定されません。CLI をインストールし、`PATH` 上にあることを確認してください。 |
| `NOT_AUTHENTICATED` | CLI のログインが完了していない状態。該当ツールのログインコマンドを実行してください。 |
| `ACCESS_DENIED` | エグゼキューターの起動パスへのアクセスが拒否された状態。プロバイダーは起動されません。実行ファイルの権限と関連するポリシーを確認してください。 |
| `AVAILABILITY_UNKNOWN` | エグゼキューターの起動パスを解決できなかった状態。プロバイダーは起動されません。設定された実行ファイルのパスを確認してください。 |
| `AUTH_UNKNOWN` | 準備状態を事前に確認できなかった状態。有界な呼び出しを試行して実際の結果を判定します。コマンド探索の失敗のみで未インストールと判定することはありません。 |
| `UNSUPPORTED` | 指定されたエグゼキューター名がサポート対象の 5 つのツールのいずれにも該当しない状態。 |
| `CONFIG_ERROR` | ディスパッチ前に設定検証が失敗した状態（安全でない `--report-dir` など）。 |
| `CALL_FAILED` | エグゼキューターが 0 以外の終了コードで異常終了した状態。エグゼキューターログを確認してください。 |
| `NO_RECEIPT` | プロセスは終了コード 0 で終了したが、レシートファイルが生成されなかった状態。 |
| `TIMEOUT` | 実行がタイムアウト制限を超過した状態。モデルの応答が遅い場合は `--timeout` を調整してください。 |

リモートスモークテストを行うには、`--transport ssh --ssh-target <target> --remote-workdir <dedicated checkout>` を付与します。ホストの接続性およびログイン確認は、**リモートマシン上で直接** 実行されます。レポートは `.dev/pipeline/<yyyymmdd>/test-executor-smoke-<HHMMSS>Z/ssh/` に書き出されます。`--remote-workdir` には、管理リポジトリの Git `HEAD` と同期された専用のチェックアウトディレクトリを指定する必要があります。

| リモートステータスコード | 説明 | 推奨されるアクション |
| --- | --- | --- |
| `SSH_UNREACHABLE` | 非対話型の SSH 接続を確立できなかった状態。 | ホストのネットワーク疎通と SSH 鍵認証を確認してください。 |
| `REMOTE_GUARD_FAILED` | リモートチェックアウトが存在しないか、ダーティであるか、管理リポジトリの `HEAD` と不一致の状態。 | リモートチェックアウトをローカルの `HEAD` と一致させ、未追跡ファイルを削除してください。 |
| `REMOTE_FETCH_FAILED` | リモートコマンドは完了したが、レシートの取得に失敗した状態。 | エグゼキューターログとリモート側のレシートパスを確認してください。 |

リモートの Copilot は、トランスポートの制約により `UNSUPPORTED` を返します。スモークテストを合格（PASS）とするには、プロセスのクリーンな終了と、空でない有効なレシートファイルの取得の **双方** が満たされる必要があります。

`--strict` フラグを付けずに実行し、完了した場合、各行のステータスにかかわらず終了コードは 0 です。`--strict` を付けた場合、`PASS` 以外のステータスがあると終了コード 1 で終了します。設定エラーは、ディスパッチの前に終了コード 1 で終了します。たとえば、未対応の `--transport`、または `--ssh-target` と `--remote-workdir` の両方がない `ssh` トランスポートが該当します。テスト結果の正式な判定基準としては、診断表および JSON ログを参照してください。

## 使用量レポートと評価

実行後に使用量を確認するには、次のコマンドを使用します。契約は [`usage-accounting.md`](../../../plugins/gal-core/conventions/usage-accounting.md) です。

```text
gal usage collect --run <run-id> [--binding <path>] [--rates <path>]
gal usage report --run <run-id> [--snapshot <snapshot-id>] [--assessment <assessment-id>] [--json]
gal usage assess --run <run-id> --snapshot <snapshot-id> --input <path>
```

### 実行の収集とレポート

1. `gal usage collect --run <run-id>` を実行します。preflight が `--host-session` でオーケストレーターの紐付けを記録している場合、紐付けファイルは不要です。それ以外の場合は、`--binding` で実行の紐付けファイルを指定します。オーナーが提供するレートセットがある場合にのみ `--rates` を追加します。
2. `gal usage report --run <run-id>` を実行して、レポートを読み取ります。`--snapshot` を指定しない場合、レポートは最新の有効な事実スナップショットと、それを指定する最新の評価だけを使用します。
3. ツールがレポートを必要とする場合は、`--json` を追加します。JSON ドキュメントは標準出力に 1 つだけ出力されます。診断は標準エラー出力に出力されます。
4. 最終ターンが観測可能になった後に、`gal usage collect` を再度実行します。新しい収集で新しいスナップショットが作成されます。以前のスナップショットは変わりません。

テキストと JSON のどちらのレポートにも、オーケストレーターの項目があります。この項目は、紐付けられたホストセッション、またはオーケストレーターが利用できない理由を示します。単独の `gal dispatch` は、使用量を実行 ID `standalone-<attempt-id>` の下に記録し、その実行 ID を標準エラー出力に出力します。

レポートは証跡を収集せず、クォータも更新しません。会計が部分的または利用不可のレポートは、終了コード 0 で終了します。無効な選択は、終了コード 64 で終了します。ストレージまたは整合性の失敗は、ゼロ以外の終了コードで終了します。ディスパッチの終了コードが、会計のカバレッジを示すために変わることはありません。

### 失敗した試行と再開した試行

失敗した試行も使用量を保持します。再開は新しい試行を作成します。タスクの合計は、すべての試行で既知の使用量を合算します。

| 試行 | 結果 | 単位数 |
| --- | --- | --- |
| 1 | 失敗 | 100 |
| 2（再開） | 成功 | 60 |
| タスクの合計 | | 160 |

各試行は、それぞれ独自の結果判断を保持します。合計が、どちらの試行の結果も変えることはありません。

### 子を含む親の合計

親セッションは、子を含む合計を報告することがあります。レポートは子を 1 回だけ数えます。照合用のフィクスチャでは、親が 200 出力単位を報告し、子の 60 単位がその合計に含まれています。タスクの合計は 200 です。子を二重に数えると、合計は 260 になります。

### 不明なコスト

レートセットに、観測されたモデル、課金モード、単位に対応するレートがない場合、見積もりは不明になります。レポートは理由を保持し、ゼロとは表示しません。プロバイダーが報告した金額と確認済みの請求額は別々に表示され、どちらも見積もりには加算されません。

次の状態は例です。実際のラベルの文言はレポートによって異なります。

```text
estimate: unknown (no rate for provider codex, model gpt-x, billing mode standard, unit per_million_tokens)
```

### クォータレポートの例

クォータの行はアカウント容量のセクションに表示されます。タスクの合計に加算されることはありません。次の例は、レポートが各状態をどのように示すかを表しています。理由の文言とタイムスタンプはレポートのテストから取り、レイアウトは簡略化しています。

| ケース | レポートの表示 |
| --- | --- |
| ウィンドウの欠落 | ウィンドウは理由とともに不明と表示されます。理由の例は `provider sent no five-hour window` です。レポートは、欠けているウィンドウをゼロや無制限として扱うことはありません。 |
| 共有アカウントの参照 | 同じクォータ参照を返した 2 つの試行は、その参照を 1 回だけ表示し、両方の試行にリンクします。この参照は、どちらの試行でもコストにはなりません。 |
| 古い時刻、または不明な時刻 | プロバイダーの観測時刻がない場合、`observed_at_unix_ms: not recorded` と表示します。収集時刻は別に表示します。例は `collected_at_unix_ms: 1800000200000` です。鮮度は不明のままです。 |
| 未対応のソース | その行には、プロバイダーの識別情報と記録されたギャップが表示されます。GitHub Copilot CLI のギャップは、検証済みの CLI 1.0.98 に固定されています。 |

更新に失敗した場合は、以前の値を保持します。レコードは取得状態を失敗と示し、元の観測時刻を保持します。履歴レポートは保存された参照を使用し、現在のクォータを問い合わせることはありません。

### 遡及的な評価

評価は、既存のスナップショット内の貢献を分類します。スナップショットを変更することはありません。入力は、次のフィールドだけを含む JSON オブジェクトです。`useful_work_rule`、`author`、`session_id`、`model`、`supersedes`、`assignments`、`residuals` です。CLI がスナップショットとそのダイジェストを紐付けます。

```json
{
  "useful_work_rule": "accepted when the run kept the change, support when it informed the kept change",
  "author": "orchestrator",
  "session_id": "review-session-1",
  "model": "claude-haiku-5-5",
  "supersedes": null,
  "assignments": [
    {
      "contribution_id": "obs-child/output",
      "outcome": "accepted",
      "reason": "the child attempt produced the kept change",
      "evidence": ["child-session-diff"]
    }
  ],
  "residuals": [
    {
      "parent_contribution_id": "obs-parent/output",
      "covered_contribution_ids": ["obs-child/output"],
      "outcome": "support",
      "reason": "the parent's own turns explored the change",
      "evidence": ["parent-session-log"]
    }
  ]
}
```

```text
gal usage assess --run run-a --snapshot snap-a --input assessment.json
```

- 価格、トークン数、合計、クォータのフィールド、その他のフィールドを含む入力は、終了コード 64 で失敗します。
- 別のスナップショットを指定する明示的な評価は、不一致の診断とともに失敗します。
- 親の残差は子の貢献を対象とします。子の単位数を再び加算することはありません。

## ワークフローの復旧

### Finalize ゲート失敗の解決

`gal finalize-check` を実行し、各出力行を確認します。完全な検証では `authoritative-command`、`naming-gate`、`sync-idempotency`、`finalize-mode`、`project-source-doc-existence`、`project-md-bound`、`state-bound`、`contract-roster-parity`、`doc-link-resolution`、`working-tree-clean` の順に評価します。ルートに `plugins/gal-core/` があるリポジトリでは 10 項目、ない場合は `contract-roster-parity` と `doc-link-resolution` を除く 8 項目です。`pass` と `advisory` はレシートをブロックせず、`fail` と `not-run` はブロックします。full mode の finalize 失敗は owner の対応先に従って停止します。finalize で形式修復できるのは、新しく取得した繫結済み `hygiene-only` receipt の失敗・未実行行がすべて修復対象の `format` である場合だけです。修復は invocation 共通の 2 round 上限を使い、新しい検証済み再実行が必要です。`.dev` の衛生検出と修復の規則は、唯一の基準である[`.dev` 検出事項の規約](../../../plugins/gal-core/conventions/token-budget.md#dev-findings-hygiene-and-integrity)を参照してください。

`sync-idempotency` チェックは、候補となるアダプターのレンダリング結果が複数回のパスにわたって決定論的に一致することを検証するものであり、ディスクへの書き込みは行いません。投影されたファイルとディスク上の状態との差異はドリフトとして報告されますが、このゲート自体を失敗させることはありません。

`authoritative-command` チェックは実行終了コードとプロセスエラーを報告します。`working-tree-clean` チェックは未コミットのファイル数とステータス出力を報告します。その他のチェックは簡潔な要約ステータスを出力します。

衛生チェックは `project-source-doc-existence`、`state-bound`、`project-md-bound`、`durable-layer-commit`、`finalize-review-shape`、`contract-roster-parity`、`doc-link-resolution`、`working-tree-clean` の 8 項目を評価します。ルートに `plugins/gal-core/` がない場合は、最後の契約整合性とリンク解決の 2 項目を除く 6 項目です。計画ファイルを削除する前に、作業ツリーがクリーンであり、`### Finalize Review <date>` の構造が妥当であることを確認します。有限インデックスに関する `project-source-doc-existence`、`state-bound`、`project-md-bound` の検出結果は `advisory` であり、レシートをブロックしません。GAL は該当ゲートの前に `.dev/project.md` と `.dev/state.md` を正規化・修復し、同じチェックを一度再実行します。問題が残る場合、実行中の計画 slug をキーに `.dev/state.md` の `Hygiene debt:` 行を追加または更新し、引き継ぎと finalize の報告に含めます。再チェックが成功すると行を削除します。`fail`、`not-run`、証拠の欠落は引き続きブロックします。詳細は[唯一の規約](../../../plugins/gal-core/conventions/token-budget.md#dev-findings-hygiene-and-integrity)を参照してください。

ゲートの失敗はカテゴリ別に対処してください。

- 公式コマンド（Authoritative command）のエラー、環境の不備、または未コミットファイルが存在する場合は、個別に是正措置を講じてください。finalize の処理中にアドホックな修正を試みてはいけません。
- 他のパスも未マージでも、`gal state-merge` は `.dev/state.md` の対応する別々の場所を解決し、他の未マージパスには触れません。同じ場所の競合、権限を持つファイルの競合、対応範囲外の state の競合、および他のパスの競合は整合性問題として停止します。
- `### Finalize Review <date>` 内の空行や不正な行は `finalize-review-shape` を失敗させます。衛生チェックを再実行する前に、レビューを再実行して有効なテーブルエントリを生成してください。
- 計画ステータスが `DONE` ではない状態で未完了のタスクが残っている場合は、`/gal pipeline` に戻って残りのタスクを完了させてください。
- 計画が `DONE` とマークされているにもかかわらず未検証または矛盾した状態が残っている場合は、状態の破損とみなされます。直ちに作業を停止し、判明した事実を引き継ぎメモに記録してください。

失敗した finalize ゲートに対して、自動的なオーバーライドを適用したりレシートを手動で改ざんしたりすることは固く禁じられています。

コミット状態が完全にクリーンであることが確認された後にのみ、端末復旧手順を実行します。

1. `gal pipeline-preflight --terminal-reverify <prompt-path>` を実行します。
2. 検証が完了したら、ORCHESTRATOR が現在のプロセス内で目標逆行検証（goal-backward verification）を実行します。
3. `gal pipeline-handback-check <prompt-path>` を実行します。
4. `gal finalize-check <prompt-path>` を実行します。

### ワークツリーのランタイム引き継ぎ

`stable PATH shim` は `package-manager` が提供する入口です。コマンドを開始したディレクトリからワークツリーのルートを解決し、ランタイム状態を信頼する前に、そのルートの実行ファイルへ引き継ぎます。ルートは、GAL ソースチェックアウトである最も近い祖先ディレクトリです。見つからない場合は git のトップレベルディレクトリ、それもない場合は開始ディレクトリを使います。そのため、どのサブディレクトリから開始したコマンドも、ルートから開始したコマンドと同じバインディングを使います。解決した実行ファイルのルートは明示します。`explicit-root` はソース受け入れ実行が書き込める唯一の場所です。コーディネーターのバインディングが保護された実行の唯一の永続的な権威です。バインディングがない場合や信頼できない場合、処理を停止します。バインディングとレシートは、パスを一つの正規化形式で記録します。Windows の verbatim 接頭辞 `\\?\` を除き、区切り文字には `/` を使います。

有効なバインディングがないワークツリーでは、shim は `execution_binding_sha256` を含む引き継ぎ情報を返します。これは引き継ぎ情報またはレシートが保持するダイジェストです。コーディネーターは引き継ぎ情報と明示された実行ファイルのルートを検証してから、ワークツリーに `safe rebind` を実行します。試行が実行中の場合、チェックポイントが未消費の場合、または投影トランザクションが保留中の場合、`safe rebind` は拒否されます。再バインドに成功すると、新しい不変世代を公開し、以前の世代とその証拠を保持します。

ゲートレシートを受け入れる前に、レシートダイジェストが保存済みチェックポイントと現在のバインディングの両方に一致することを確認します。どちらかが一致しない場合、停止してバインディングを解決します。ワークツリーの復旧に GAL の再インストールや共有インストール状態の削除を使わないでください。

共有 shim は、パッケージマネージャーのリリースを通じてのみ変更されます。ソース更新では、不変世代と安全な再バインドを使い、共有 shim を置き換えません。

ゲートレシートは、`gal doctor --verify-receipt <receipt> --expect-scope <standalone|coordinator:<plan-scope>>` がバインドされた実行ファイルを通じて成功した後にのみ信頼してください。レシートのダイジェストが保存済みチェックポイントまたは現在のバインディングと一致しない場合は停止してください。レシートを信頼したり、ゲートを再実行したりしないでください。

```text
stable shim -> explicit executable root -> coordinator binding
     |                                  |
     +-> digest handback -> validate ---+
                                   | valid
                                   v
                           safe rebind -> immutable generation
                                   | unsafe
                                   v
                               hard stop
gate receipt -> checkpoint digest + binding digest -> accept or stop
```

### レシートリースの解放

レシートリースは、並行してディスパッチされた複数の処理が決定論的なレシートファイルを誤って上書きするのを防止します。`reason=receipt-preparation-failed` や `reason=remote-receipt-freshness-failed` などのメッセージは、アクティブまたは孤立したロックファイルの存在を示しています。これらはレシートディレクトリを無差別に削除してよい理由にはなりません。

標準エラー出力を確認し、該当するロックパス（`.dev/pipeline/.locks/<hash>.lock`、またはリモート実行の場合は `.dev/pipeline/.locks/<hash>.lockdir`）を特定してください。ロックを解除する前に、関連するローカルまたは SSH プロセスが実行中でないことを必ず確認してください。確認が取れたら、該当するロックファイル、またはロックディレクトリ内のリモート `owner` ファイルのみを削除します。**`.locks/` フォルダー全体を一括削除してはいけません。**

レシート失敗コード一覧:

| 失敗コード | 説明 |
| --- | --- |
| `remote-receipt-fetch-failed` | 対象のレシートが存在しないか、空であるか、シンボリックリンクであるか、読み取れない状態。 |
| `remote-receipt-fetch-timeout` | レシートのダウンロードがネットワークタイムアウト制限を超過した状態。 |
| `remote-receipt-fetch-read-failed` | パイプの読み取り失敗により、標準出力を完全にキャプチャできなかった状態。 |
| `remote-receipt-fetch-too-large` | レシート出力が 1 MiB のサイズ制限を超過した状態。 |
| `receipt-lease-cleanup-failed` | 通常のクリーンアップ処理においてローカルのロックファイルを削除できなかった状態。 |
| `remote-receipt-lease-cleanup-failed` | リモート側のオーナーロックを解除できなかったため、安全のため fail-closed した状態。 |
| `remote-receipt-lease-cleanup-timeout` | リモート側のオーナーロックの解除がタイムアウトしたが、補助 SSH プロセスの終了は確認済みの状態。 |
| `remote-receipt-lease-cleanup-timeout-unconfirmed` | リモート側のオーナーロックの解除がタイムアウトし、補助 SSH プロセスの終了を確認できなかった状態。 |
| `remote-receipt-lease-cleanup-failed-unconfirmed` | リモート側のオーナーロックの解除に失敗し、補助 SSH プロセスの終了を確認できなかった状態。 |
| `remote-receipt-fetch-timeout-unconfirmed` | レシートのダウンロードがタイムアウトし、補助 SSH プロセスの終了を確認できなかった状態。 |
| `remote-receipt-fetch-failed-unconfirmed` | レシートのダウンロードに失敗し、補助 SSH プロセスの終了を確認できなかった状態。 |
| `remote-receipt-freshness-failed` | リモートのレシートが鮮度チェックに合格しなかった状態。 |

末尾が `-unconfirmed` で終わるコードは、ローカルの補助 SSH プロセスが確認シグナルなしに終了したことを示します。なお、終了コード 73 は、特定の鮮度監視シグナルを伴わない限り、リース失敗ではなくプロセスの正常な戻り値を示します。

### 意味的 Write-Back 失敗の処理

プロンプト駆動のディスパッチ（`audit` または `test`）において、終了コード 0 はレシートが `<task>-<phase>.receipt.md` に正常に書き出されたことのみを意味します。タスクが完了したことや、プロンプトファイルが更新されたことを意味するわけではありません。プロンプト内容の更新は `gal` コントロールノードのみが担当し、ペイロードを安全側に倒した（fail-closed）意味的 write-back ゲートで検証します。

パイプラインに束縛された test および audit レシートは、1 行目を `### [T-NN] YYYY-MM-DD` で開始しなければなりません。エグゼキューターは、この見出しより前にメタデータや前置き文を出力してはいけません。コントロールノードは、メタデータが先にあるレシートを拒否し、実行プロンプトを変更しません。有効な test レシートは `## Test Results` の下に配置され、有効な audit レシートは `## Review Results` の下に配置されます。

**失敗時の兆候**: 検証に失敗した場合、`gal` は標準エラー出力に `phase-writeback semantic failure for task <T-NN>` を出力し、エラーの詳細をパイプラインログに記録した上で、0 以外の終了コードで終了します。ディスク上の実行プロンプトは変更されず、プロンプトの不変性が保護されます。

運用の対象範囲:

- ローカルおよび SSH ディスパッチの両方から届いたレシートが同一の意味的ゲートを通過するため、プロンプトの更新基準が一貫して保たれます。
- 生のタスク仕様（`PipelineInput::RawSpec`）および直接のディスパッチ（`gal dispatch`）は、権威あるプロンプトファイルを持たないため、write-back ゲートをバイパスします。

Write-Back エラーのトラブルシューティング:

- **古いエージェント契約**: エグゼキューターが不正な形式の Markdown を出力した場合（裁定マーカーの欠落や不正な見出しなど）、またはプロンプトファイルを直接編集しようとした場合は、`gal refresh` を実行して投影された契約を再同期してください。
- **不正なレシート構造**: `.dev/pipeline/<plan-slug>/...` 内のレシートファイルを確認するか、パイプラインログを参照して、`### [T-NN] YYYY-MM-DD` ヘッダーの欠落やタスク識別子の不一致などの構文エラーがないか調査してください。
- **タスクの再実行**: `gal refresh` で契約や書式のエラーを解消した後、`/gal pipeline` を再実行します。手動で実行プロンプト内に Markdown 片を貼り付けてはいけません。自動配置により、内容が適切な `## Test Results` または `## Review Results` 見出し配下に確実に挿入されます。

## 内部サブコマンドと自動化ゲート

以下のサブコマンドはワークフローから自動的に呼び出される内部コマンドであり、公開されている `/gal` コマンド体系の一部ではありません。これらは主に診断ログやゲート出力に表示されます。

| サブコマンド | ワークフローフェーズ | 主な機能 |
| --- | --- | --- |
| `gal planning-check` | Planning | 初期計画の構造的要件を検証します。`diagram-orientation` チェックは、`## Diagrams` 配下で向きが `LR`、`RL`、`BT` の Mermaid フローチャートを拒否します。 |
| `gal refining-check` | Refining | ソースプラン内の構造的整合性を検証します。 |
| `gal prompt-check` | Prompt Generation | プロンプトスキーマを検証します。`--assemble-dry-run` を付与してタスクの組み立てをテストできます。 |
| `gal planning-stamp` | Planning | 旧形式の計画との互換のために残るコマンドで、計画の権威スタンプを記録します。`--equivalence <prompt>` を付与して現地語計画と英語プロンプトの意味的等価性を検証します。 |
| `gal pipeline-preflight` | Pipeline Entry | パイプライン開始時の検証ゲート。コード変更を開始する前に必ず `pass` を返さなければなりません。復旧時は `--terminal-reverify` を使用します。 |
| `gal boundary-check` | Pre-commit | 変更されたファイルをタスクの許可リストと照合します。許可リストの起点は、`## Tasks` 内のタスク項目にバッククォートで示されたパスです。読む範囲は、次のタスク項目または `###` 見出しまでです。そのパスが `## Files to Create or Modify` のある行の対象であれば、その行のバッククォートのパスはすべて許可されます。項目にパスがない場合は、`## Files to Create or Modify` の全パスを使います。 |
| `gal pipeline-converge-check` | Task Convergence | 3 つの整合性チェックを実行します。3 つの面のチェックボックスが一致し、各面でタスクが完了としてマークされていること、タスクに記録されたコミットメモが実在するコミットに解決されること、`Current Task:` がそのタスクを指していないことを確認します。さらに、必要な各 phase（`implement`、`test`、`audit`）を、最新のディスパッチログから `dispatch-offload`、`recovered-in-conversation`、`in-conversation` のいずれかに分類します。終了していない試行や不正なログがあれば、レシートは fail-closed になります。 |
| `gal pipeline-handback-check` | Pipeline Exit | 各タスクが収束した後の次の継続判定を返します。判定は `continue`、`goal-verified`、`human-required`、`retry-ceiling`、`stop-at` のいずれかです。`goal_backward` checkpoint のレシートを消費します。終了判定では `continue_action: none` を返します。 |
| `gal pipeline-log append` | Global | パイプラインのイベントログに構造化エントリを追記します。 |
| `gal pipeline-clean` | Finalize | `.dev/pipeline/<plan-slug>/` 内の一時成果物をクリーンアップします。 |
| `gal state-merge` | Finalize | 計画キーに基づいて `.dev/state.md` のテーブルを決定論的にマージします。 |
| `gal finalize-check` | Finalize | finalize 実行時にゼロトラスト前提条件を検証します。 |
| `gal dispatch-script` | Dispatch | `OFFLOAD` ディレクティブを含むコントロールプレーンディスパッチブロックを生成します。 |
| `gal dispatch` | Dispatch | 出所追跡や write-back ゲートをバイパスする生の実行エントリーポイント。 |

メンテナー向けコマンド（`gal restore`、`gal dev-apply`、`gal release`、`gal release-notes`、`gal marketplace-snapshot`）については、[CONTRIBUTING.ja.md](CONTRIBUTING.ja.md#メンテナー用サブコマンド) を参照してください。

## その他のライフサイクル操作

### エージェント契約の解決

タスクをディスパッチする前に、`gal` は権威あるエージェント契約（`agents/golem-{implementer|tester|auditor}.agent.md`）を特定し、その内容を読み込んでタスク仕様内に直接埋め込みます。タスク仕様が完全に自己完結しているため、リモートエグゼキューターがコントロールノードのパスに直接アクセスする必要はありません。

契約の解決は、以下の優先順位に従って 4 つの階層を順番に探索します。

1. `workdir`: 正規化された `--workdir` パス、またはその直下の `plugins/gal-core` サブディレクトリ。
2. `ancestor`: GAL のソースルートとして認識される最も近い親ディレクトリ。
3. `exe-side`: 実行中の `gal` バイナリの隣にあるディレクトリ。
4. `embedded`: `~/.gal/embedded-src` に展開された組み込みソースファイル。

`workdir` が最優先されます。これにより、`PATH` 上で古いバイナリが動作している場合であっても、`plugins/gal-core/` を含むローカルチェックアウトの内容が常に優先されます。ソースチェックアウトを運用する際は、バージョン乖離を検出するために `Dispatch:` ログヘッダー内の `contract_source` エントリを確認してください。

解決失敗時の対処:

- **破損したソースルート**: 最優先のルートに読み取り不能または非 UTF-8 の契約が含まれている場合、ディスパッチは該当パスをエラーとして報告し直ちに停止します。自動的に下位の階層へフォールバックすることはありません。指定されたルート内のファイルエラーを解消してください。
- **組み込みソースのマテリアライズ失敗**: 上位のすべての階層が見つからず、組み込みコンテンツが存在するのにステージング、抽出、またはアトミックな swap に失敗した場合、GAL は失敗した操作、パス、および基礎となる I/O 原因を報告します。これは組み込みペイロードが存在しない場合とは異なります。パイプラインディスパッチ、`gal init`、`gal refresh`、および `gal render-adapters` は、既存のソース解決境界でこのエラーを保持します。示されたパスを確認し、GAL のホームディレクトリを修復するか、GAL を再インストールしてください。
- **有効なルートが見つからない場合**: すべての階層で契約が見つからなかった場合、ディスパッチは終了コード 1 で終了します。パッケージマネージャー経由で `gal` を再インストールするか、有効なソースチェックアウトからコマンドを実行してください。

素の `gal dispatch` 実行および生のタスク仕様ディスパッチは契約解決をバイパスするため、ログ内の `contract=` および `contract_source=` フィールドは省略されます。

### セッション Wrap-Up と Finalize の違い

`/gal wrap-up` コマンドは、計画を完了させることなくアクティブなセッションを一時停止します。引き継ぎメモを実行プロンプトへ統合し、`.dev/state.md` 内のセッション追跡情報を更新した上で変更をコミットし、サポート対象の任意のランタイムから作業を再開できるようにします。このコマンドが計画を閉じる（完了とする）ことはありません。完了した計画を終了するには `/gal finalize` を使用し、作業を途中で一時中断したい場合には `/gal wrap-up` を使用してください。

## Git ワークフロー支援機能

### コミットメッセージの自動生成（`gal commit-msg`）

`gal commit-msg` コマンドは、`git-commits` スキルおよび `git-commit-msg` コマンドのバックエンドとして動作します。差分本文やコミットメッセージの内容に左右されず、ステージングされたファイルパスと Git の状態から、コミットの種別（type）とスコープ（scope）を決定論的に導出します。

- `gal commit-msg --context`: モデルが参照できるよう、ステージングされた変更のコンテキスト（ファイル一覧、基本件名、差分ハンク）をフォーマットして出力します。
- `gal commit-msg --print`: 決定論的に導出された「type(scope): subject」形式の 1 行の件名を直接出力します。
- `gal commit-msg <file>`: Git の `commit-msg` フックとして機能し、既存の下書きを上書きすることなく、ステージングされた変更から空のコミットテンプレートを補完します。

`git-commit-msg` コマンドは、コミットを作成することなくメッセージ案を提示します。対照的に、`git-commits` スキルは、ユーザーの明示的な意図が確認された場合に実際のコミットまでを直接実行します。

### Git フィルター（`gal clean` および `gal smudge`）

オプションの `gal-config` フィルターは、Git で追跡されるリポジトリファイルからマシンローカルな設定を除去します。

```bash
git config filter.gal-config.clean "gal clean"
git config filter.gal-config.smudge "gal smudge"
```

Git はチェックアウトおよびコミット操作の際にこれらのフィルターを呼び出します。フィルターエラーによるコミット失敗を防ぐため、`gal` がシステムの `PATH` から常に利用可能であることを確認してください。

## バージョンと翻訳の確認

### バージョン情報（`gal update`）

`gal update` は、インストール済みのバージョンと、Cargo、WinGet、Homebrew それぞれのアップグレードコマンドを表示します。アップグレードは実行しません。`gal` をインストールしたパッケージマネージャーでアップグレードしてください。未知のオプションは使用法エラーになります。

### 翻訳の鮮度（`gal translation-freshness`）

リポジトリのルートで `gal translation-freshness` を実行します。引数なしでは、許可リストに含まれる英語文書とその翻訳を確認します。このガイドも対象に含まれます。翻訳ごとに `STATUS` と `NOTE` を含む行を表示し、最後に `current`、`stale`、`missing` の件数を集計します。ステータスの値は次のとおりです。

- `current`: 翻訳の `source_commit` の刻印が、英語の元文書の最新コミットと一致します。
- `stale`: 刻印が最新の元文書コミットと一致しません。注記は `source advanced to <hash>` になります。刻印が `PENDING` のようにコミットハッシュではない場合、注記は `unstamped (PENDING)` になります。
- `unexpected`: 許可リストにない文書の全文翻訳です。
- `missing`: 許可リストに含まれる文書に翻訳がありません。

レポートを表示した後、このコマンドは終了コード 0 で終了します。翻訳の更新が必要かどうかは、STATUS 列で判断してください。2 つのオプションで出力を変えられます。`--coverage` は英語のみ、または翻訳のみの文書を一覧にします。`--pair <source> <translation>` は 1 組だけを確認します。

## 視覚的ドキュメント支援機能

### 決定木チャート（`text-flowcharts`）

[`text-flowcharts`](../../../plugins/gal-core/skills/text-flowcharts/SKILL.md) スキルは、条件分岐ロジック、パイプライン、および状態遷移を記述するための、等幅 ASCII および Unicode 決定木を生成します。Claude Code では `/text-flowcharts` から呼び出すことができます。また、条件付き制御フローを説明する際や、フローチャートが明示的に要求された場合にも自動的に起動されます。

チャートは、開始点から順次条件判定を経て最終結果に至るまでの個別のレコードの流れをトレースします。視覚的な表記には以下の標準文字を使用します。

| フロー要素 | 表記文字 |
| --- | --- |
| 線および角 | `│ ─ ┌ ┐ └ ┘` |
| 分岐、合流、交差 | `├ ┤ ┬ ┴ ┼` |
| 方向矢印 | `▼ ▲ ▶ ◀` |
| 成功・合格 | `√` |
| 意図的なスキップ | `>>\|` |
| 拒絶・終了パス | `×` |

これらの文字は、特別なターミナルフォントを必要とせず、一般的な等幅フォント環境で綺麗に表示されます。GitHub の Pull Request、レビューコメント、ターミナルエミュレーター上でもズレを生じることなくレイアウトを維持できます。また、旧式のターミナルでの可読性を保つため絵文字（emoji）の使用を避けています。ワークフロー内に条件分岐が含まれる場合は、この決定木チャートを活用してください。単純な線形の手順については、番号付きリストで記述するほうが適しています。

## メッセージと成果物の文章品質

実際の送信前の草稿にある重要な事実の主張を、利用可能な具体的根拠と一つずつ照合します。裏付けのない主張は削除するか、適切な限定を加えます。完了を示す証拠がないことは、作業が未着手である証拠にはなりません。成果物だけを求められた場合は従います。ただし、優先度の高いホスト指示が別の出力を要求する場合は除きます。これは現在のエージェントによる意味レビューの義務であり、自動的な事実確認や出力遮断の保証ではありません。

進捗や専門エージェントの報告を含む全メッセージで、送信前の草稿を作成中に自己レビューし、正確さ、読みやすさ、意味の完全性を確認します。実際の受信者の知識に合わせて書きます。依頼、提供資料、実際のツール観測に基づく事実を、推論、仮定、提案、仮置きの内容、将来の約束と区別します。依頼ではなくツールから得たという理由だけで、事実を捏造と判定してはいけません。

設定済みのチェッカーが利用可能で、言語プロファイルが適切なら、送信前の草稿を検査します。指針の発見、読み込み、ツール実行、出力の遮断は別々に確認します。ファイル検査では、すでにストリーム送信した会話を検査できません。指摘のない結果は、設定済みの決定的な検査だけを証明します。事実の正確さや読みやすさの証明ではありません。

修正が必要なら、試行は最大 2 回にします。各修正を再検査し、事実と保護対象を維持します。意味を変える修正は拒否して元に戻します。助言だけが残る対話では、最後の意味的に有効な草稿を使用し、必要に応じて未解決の指摘を伝えます。任意のチェッカーが利用できなくても自己レビューを続け、利用可能性または配信への影響が変わるまで制約の通知は 1 回にします。チェッカー自身の状態メッセージを再帰的に検査せず、各メッセージに auditor を派遣しません。

完全なレポートを得るには、wrapper の `--files` より前に `--transport cli` または `--transport mcp` を指定します。経路指定は `--required` とも併用できます。既定は CLI です。公式 MCP 経路も同じ形式の指摘と実行メタデータを保持します。上流の診断出力だけでは、完全な GAL レポートになりません。MCP のスキーマ検証、タイムアウト、終了処理の失敗など、運用上の問題は終了コード `2` になります。

必須の成果物ゲートは、ハード指摘または運用上の失敗で停止します。利用不可、クラッシュ、タイムアウトも運用上の失敗です。別の明示的な必須条件がない限り、助言だけではゲートを止めません。阻害箇所と対処方法を報告します。ワークフローが `PROSE_AUDIT` を要求する場合、独立した意味レビューとして引き続き必須です。textlint では代替できません。開発者向けコマンドとレポートは [`CONTRIBUTING.md`](../../../CONTRIBUTING.md#writing-quality-checks) と [`tools/writing/README.md`](../../../tools/writing/README.md) を参照してください。
