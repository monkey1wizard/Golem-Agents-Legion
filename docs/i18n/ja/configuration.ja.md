---
source: docs/configuration.md
lang: ja
source_commit: c176446a028e431dce518eb4e1e6010046d04321
translated_at: 2026-10-23
type: Guide
title: 設定
description: `executorRouting`、ホストサブエージェントのモデル tier、計画言語、MCP ソース、個人レイヤー、local-notes のルーティングを含む `~/.gal/config/config.json` のマシンローカル設定を管理します。
tags:
  - configuration
  - routing
  - mcp
  - personal-layer
status: stable
---

# 設定

[English](../../configuration.md) · **日本語** · [繁體中文](../zh-Hant/configuration.zh-Hant.md)

## マシンローカル設定（`config.json`）

GAL はマシンローカルな設定を `~/.gal/config/config.json` に保存します。このファイル内の設定値は現在のマシン環境にのみ適用されます。バージョン管理対象のファイルやコマンドテンプレート、ソースコード内にマシンローカルな設定値を誤ってコミットしてはいけません。認識される主要キーには `galSkills`、`planLanguage`、`memoryHarvest`、`executorRouting`、`hostSubagentModels` があります。勤務時間の動作は `config.json` のオブジェクトではありません。エージェントは、ローカル設定で有効化されている場合に [working-hours.md](../../../plugins/gal-core/conventions/working-hours.md) で説明された値を適用します。`crates/` 配下のどの crate もこれらの値を読み取りません。GAL は各ランタイム自身の設定からプラグイン登録を検出します。既存の `pluginMode` プロパティは廃止済みとして無視され、`gal doctor` は古いキーとして報告します。`schemaVersion` は `config.json` ではなく `plugins.lock.json` に属します。`enabledPlugins` は GitHub Copilot 自身の `settings.json` にある設定で、GAL は登録の証拠として読み取ります。

非推奨となったキー（`devMode`、`galRoot`、`secrets`）は無視されます。`gal doctor` が `devMode` または `galRoot` を検出した場合は、処理を中断しない警告（non-blocking warning）を出力します。`secrets` キーは完全に廃止されました。これは過去の旧式 MCP ホスト書き込み機能用のものであり、現在の GAL が本ファイルから認証情報を読み取ることは一切ありません。

### 主要フィールド

| フィールド | 必須 | 型 | 説明 |
| --- | --- | --- | --- |
| `executorRouting` | 任意 | object | 利用グループごとに整理された役割別エグゼキュータールーティング。システムにおける唯一のルーティング権威です。 |
| `galSkills` | 任意 | string | ローカルスキルディレクトリの絶対パス。現在残されている唯一のマシンローカルパス置換キーです。 |
| `hostSubagentModels` | 任意 | object | ホストサブエージェントのディスパッチで使う、ホストと tier ごとのモデル上書き。[ホストサブエージェント (host subagent) ディスパッチモード](#ホストサブエージェント-host-subagent-ディスパッチモード)を参照してください。 |
| `planLanguage` | 任意 | string | 指示がない場合に使用される `.dev/plans/*.md` および `.dev/research/*.md` の既定の出力言語。デフォルトは `en` です。 |
| `memoryHarvest` | 任意 | object | `{ enabled: boolean }` 形式のプロバイダーメモリーハーベスティング設定。デフォルトは無効です。 |
| `quickfix` | 任意 | object | クイックフィックスレーンの上限値と Stop フックのスイッチ。[クイックフィックスレーン (quickfix)](#クイックフィックスレーン-quickfix)を参照してください。 |

コア設定では外部ノートバックエンドを管理しません。外部ノート仕様の詳細は `plugins/gal-core/conventions/optional-capabilities.md` を参照してください。リポジトリの調査出力は既定で `.dev/research/` に保存され、一時作業ファイルは `.dev/tmp` に配置されます。

## クイックフィックスレーン (quickfix)

`quickfix` キーは `~/.gal/config/config.json` の最上位に置きます。このキーは、[ワークフロー](workflows.ja.md#クイックフィックスレーン)で説明するクイックフィックスレーンの上限値を設定します。`gal quickfix-run`、`gal quickfix-check`、`gal quickfix-hook` がこのキーを読み取ります。ファイルまたはキーがない場合は、下表のデフォルト値を使います。

| フィールド | 型 | デフォルト | 説明 |
| --- | --- | --- | --- |
| `maxFiles` | 非負整数 | `3` | 1 回のクイックフィックス変更が対象にできるファイル数の上限。 |
| `maxLines` | 非負整数 | `50` | 1 回のクイックフィックス変更に含められる変更行数の上限。 |
| `maxAttempts` | 非負整数 | `2` | 同じ `HEAD` で、同じ失敗コマンドに対して開けるアテンプト数の上限。 |
| `hook` | boolean | `false` | `true` の場合、ホストの Stop フックは作業ツリーを、同じセッションの前回の Stop で保存したスナップショットと比較します。スナップショットが変わっていなければ停止を許可します。変わっていれば、フックはアテンプトを開こうとします。開けた場合、フックはホストの停止をブロックし、ホストはそのアテンプトを続けます。開くことを拒否された場合は停止を許可します。 |

```json
{
  "quickfix": { "maxFiles": 3, "maxLines": 50, "maxAttempts": 2, "hook": false }
}
```

形式が不正な `quickfix` キーはエラーになり、デフォルト値へのフォールバックはありません。次の場合はエラーです。`quickfix` の値がオブジェクトでない場合、上限値が非負整数でない場合、`hook` の値が `true` または `false` でない場合、上記 4 つ以外のフィールド名がある場合です。

## エグゼキュータールーティング（`executorRouting`）

### ホストサブエージェント (host subagent) ディスパッチモード

エグゼキュータールーティングが設定されている場合、ディスパッチモードの既定値は `routed` です。ロールにルートがある場合に適用されます。現在のセッションでの要求はモードを `host-subagent` に固定し、有効なルートより優先されます。finalize レビューは常にそのルートを使います。無効なルートは、ホストが固定されているとき、または finalize レビュー中にディスパッチを停止します。固定されていない通常のディスパッチでは、無効なルートは許容的なローダーで処理され、ロールに使えるエントリーがなければ `no-routing` を報告します。ルートがなくホストが固定されている場合、モードは `host-subagent` です。ルートもホストの固定もない場合、モードは `text` です。実際に出力される `DISPATCH_MODE` の値は、routed ディスパッチの `offload`、`host-subagent`、`text` です。正確なフレーズ、ホストの選択、試行のライフサイクル、権限ルール、PASS の要件は、[パイプライン skill](../../../plugins/gal-core/commands/gal-pipeline/SKILL.template.md#dispatch-mode) で定義されています。

ホストの能力チェックでは、選んだホストがサブエージェントを起動し、その完了証拠を受け取れることを確認する必要があります。Claude Code は `--host claude` を使います。能力のある Codex Desktop または Codex CLI は `--host codex` を使います。能力の証拠は Codex Desktop と Codex CLI で別々に扱います。実機の受け入れテストは Claude Code、Codex CLI 0.162.1、Codex Desktop 26.1013.31256 で合格しました。Codex のすべてのフェーズが `fork_turns: "none"` と指定のモデルと effort で新しい既定のサブエージェントを起動しました。能力チェックの失敗や無効なホストは、黙って代替することなく拒否されます。

組み込みのモデル対応では、Claude の light に Haiku、standard に Sonnet、deep に Opus を使い、明示的に指定できる deep の代替として Fable をサポートする。Codex の light は Luna、standard は Sol、deep は Astra である。モデルと effort を使えるかどうかは、ホストとアカウントによって決まる。deep tier は、タスク内容からではなく、理由を添えた手動の上書きによってのみ選ばれる。明示的な `inherit` は劣化した除外指定である。モデル tier ポリシーが適用されなかったことを記録し、モデル tier の受け入れ基準を満たさない。モデルや effort を選べない場合に、黙って inherit に置き換えてはいけない。

`hostSubagentModels` は `config.json` の最上位キーで、ホストと tier の対応を上書きします。形式は `{"codex"|"claude": {"light"|"standard"|"deep": {"model": "...", "effort": "..."}}}` です。制約は次のとおりです。

- 受け付けるホストは `codex` と `claude` だけです。受け付ける tier は `light`、`standard`、`deep` だけです。それ以外のホストや tier のキーはエラーになります。
- Codex のエントリーの `model` は `gpt-6-luna`、`gpt-6.1-sol`、`gpt-6-astra` のいずれかです。`effort` は必須で、`medium`、`high`、`xhigh` のいずれかを指定します。
- Claude のエントリーの `model` は `haiku`、`sonnet`、`opus`、`fable` のいずれかです。`effort` は省略できます。指定する場合は `low`、`medium`、`high`、`xhigh`、`max` のいずれかです。
- `inherit` モデルには `effort` を指定できません。
- 未対応のモデルや effort、オブジェクトではない値、形式の誤ったエントリーはエラーです。試行は停止し、既定の対応へは戻りません。

`OPEN` の手順が唯一の解決者である。解決したモデル、effort、tier、tier の出所、上書き理由を記録する。オーケストレーターは、`HOST_SUBAGENT_ATTEMPT:` の `model=` の値を起動時のモデルとして使う。理由を添えた手動の上書きの場合に限り、`--tier` と `--tier-reason` を渡す。guarded 継続では、会話による上書きを使わず、計算された tier を使う。

### ホストサブエージェントの tier 規則

タスク内容、宣言されたファイル、crate 数、保護パス、依存数は tier に影響しない。自動計算される tier は次のとおりである。

- **implement**: すべてのタスクで `light` である。再試行の場合は、後述のとおり `standard` に引き上げられる。
- **test**: 受け入れられた implement の tier より一段低く、下限は `light` である。
- **audit**: `standard`、受け入れられた implement の tier、受け入れられた test の tier のうち最も高いものである。
- **再試行**: 同じタスクと同じフェーズで過去に失敗した試行があると、自動計算された `light` は `standard` に引き上げられる。失敗がさらに増えても、自動計算された tier が `standard` を超えることはない。受け入れられた前提証拠または手動の上書きによってすでに `deep` である tier は、`deep` のまま維持される。
- **tier のない証拠**: 受け入れられた routed または text の証拠で tier が記録されていないものは、`standard` として扱われ、出所は `prerequisite-tier-unknown` である。したがって、tier のない implement の前提条件では test は `light` になり、audit は `standard` の下限を維持する。tier のないホストサブエージェントの証拠は引き続き無効である。
- **完了レビュー (finalize review)**: 上書きがない場合、ホストサブエージェントの完了レビューは `hostSubagentModels` を迂回し、モデル `inherit` を解決する。effort の上書きは行わない。試行ログには tier `standard`、出所 `finalize-review`、`model_tier_applied: no` が記録される。routed の完了レビューは AUDITOR ルートを変更せずに維持する。手動のホストサブエージェント上書きは、モデル対応を通じて解決される。tier を下げる上書きは引き続き無効である。
- **理由を添えた手動の上書き**: 理由が空でない場合、`--tier deep --tier-reason <reason>` はすべてのホストサブエージェントのフェーズで許可される。tier を下げる上書きの制限は変わらない。
- **助言**: 同じタスクと同じフェーズの先行試行が `standard` で失敗し、選択された tier がなお `standard` である場合、`OPEN` は安定した `HOST_SUBAGENT_ADVISORY:` の行を一行出力し、`--tier deep --tier-reason <reason>` を推奨する。助言は選択された tier も `HOST_SUBAGENT_ATTEMPT:` の記録も変更しない。

### ホストサブエージェントの起動契約

SPAWN の指示は、公開されたマニフェストの `spec_path`、試行 ID、フェーズ、タスク、許可されたファイル、レシートのパス、書き戻しの指示を送る。仕様の本文は送らない。メッセージは、サブエージェントに行動する前に仕様ファイル全体を読むよう指示する。CLOSE は引き続き試行マニフェストの `spec_sha256` を検証する。同じ指示により、サブエージェントの最終応答は 15 行以内とする。内容は結果とレシートのパスに加え、失敗時は最初のエラーとそのファイル及び行範囲である。応答にはファイル内容、差分、成功したテストの出力を含めない。受け入れの証拠はレシートと試行ログに残す。

### Preflight ディスパッチフラグ

`gal pipeline-preflight` は次のフラグでディスパッチモードを固定します。

- `--dispatch routed` または `--dispatch host-subagent` は、固定するモードを設定します。既定値は `routed` です。`--dispatch host-subagent` には `--host` が必要です。
- `--host claude` または `--host codex` はホストを固定します。それ以外の値は拒否されます。
- `--host-session <host>:<session-id>` はオーケストレーターのセッションを指定します。preflight は、そのセッションが固定されたホストと作業ディレクトリに一致することを検証します。参照は固定されたホストを使う必要があります。`--host-session` は `--terminal-reverify` と併用できません。指定しない場合、記録されるオーケストレーターの紐付けは利用できない状態になります。

### コンシューマーグループとロールエントリー

`executorRouting` オブジェクトは、エージェントロールを以下の 3 つのコンシューマーグループに分類して整理します。

- **`planning`**: `ARCHITECT`、`ANALYST`、`DESIGNER`、`RELEASER` を含むレビューグループ。このグループは Codex ネイティブエージェントのプロジェクションにのみ反映されます。[Planning グループの適用範囲](#planning-グループの適用範囲)を参照してください。
- **`pipeline`**: `CODER`、`TESTER`、`AUDITOR` を含むディスパッチグループ。このグループはヘッドレスディスパッチを使用します。
- **`research`**: 番号付きの調査ワーカーキーを持つリサーチグループ。このグループはヘッドレスディスパッチを使用します。[調査ルーティング](#調査ルーティング)を参照してください。

これら 3 つのグループは、唯一のルーティング権威である共有の `executors` ブロックから既定値を継承します。過去のフラットなルーティングスキーマは廃止されており、パースされず警告が出力されます。また、独立した外部ルーティング設定ファイルもサポートされていません。

リテラルのエグゼキューター名は `claude`、`codex`、`opencode`、`copilot`、`agy` です。`executor` の値は、これらのいずれかの名前か、`combinations` のエントリー名である必要があります。

`_` で始まるキーはコメントです。`combinations`、各コンシューマーグループ、`research`、および `executorRouting` 直下のトップレベルでは、GAL はこのキーを黙って読み飛ばします。`executors` ブロックでは読み飛ばしません。`executorRouting` 直下のフラットなロールキーは廃止されており、警告を出力します。それ以外の認識されないトップレベルキーも警告を出力します。

### ロールエントリーのスキーマ

ロールエントリーは、`executorRouting.pipeline.CODER` のように各グループの下に置きます。

| フィールド | 必須 | 型 | 説明 |
| --- | --- | --- | --- |
| `executor` | 必須 | string | リテラルのエグゼキューター名、または `combinations` の名前。 |
| `model` | 任意 | string | combination と `executors` の既定モデルを上書きします。 |
| `effort` | 任意 | string | 推論エフォート。combination と `executors` の既定 effort を上書きします。 |
| `timeoutSecs` | 任意 | positive integer | 既定のタイムアウトを上書きします。`0` は警告付きで無視されます。 |
| `sshTarget` | 任意 | string | リモートの SSH 接続先。`remoteWorkdir` と一緒に設定します。 |
| `remoteWorkdir` | 任意 | string | リモートの作業ディレクトリ。`sshTarget` と一緒に設定します。 |

ロールのキーは大文字と小文字を区別します。

- `pipeline` は `CODER`、`TESTER`、`AUDITOR` を受け付けます。
- `planning` は `ARCHITECT`、`ANALYST`、`DESIGNER`、`RELEASER` を受け付けます。
- `research` は、[調査ルーティング](#調査ルーティング)で説明する番号付きのワーカーキーだけを受け付けます。

誤ったグループに置かれたロール、または認識されないロールは、警告付きで読み飛ばされます。

### Planning グループの適用範囲

`planning` グループは、レビュー系の 4 ロール `ARCHITECT`、`ANALYST`、`DESIGNER`、`RELEASER` にのみ適用されます。キーは `ARCHITECT` のように大文字で指定してください。スラッシュコマンドの名前は小文字のままです。Codex エージェントのプロジェクション時に、GAL は各ロールで解決される `model` と `effort` を参照し、生成される Codex エージェント定義へ書き込みます。この参照はエグゼキューターを起動せず、作業のオフロードも行いません。

- `/gal <role>` の直接呼び出し（隔離モード）は、`ROUTING_SCOPE: codex-native-projection-only` と `ROUTING_NOTE` を出力します。この注記は次の 3 点を述べます。planning 設定は Codex ネイティブエージェントのプロジェクションにのみ反映されること、エグゼキューターは起動されないこと、実際のモデルはホストが決定することです。この文面は静的な契約の説明であり、設定が適用されたことの証明ではありません。
- `planning` ロールの `executor` キーは引き続き読み取られます。このキーは、参照で `model` と `effort` を取得する `combinations` エントリまたは `executors` エントリを指定します。優先順位は、ロール内のインライン値、combination、`executors` の既定値の順です。`planning` エントリのないロールはどちらの値も得られないため、Codex は自身のモデルを継承します。このキーは起動するプロセスを選択しません。
- `timeoutSecs`、`sshTarget`、`remoteWorkdir` は planning ロールには効果がありません。これらのロールではヘッドレスプロセスが実行されないためです。これらのフィールドは `pipeline` と `research` のロールにのみ設定してください。
- GAL は、Codex 以外のランタイムが解決された `model` や `effort` を適用することを保証しません。これらのランタイムは、planning 設定の有無にかかわらず、各自のネイティブサブエージェントの動作を維持します。
- `/<role> discuss` によるコンテキスト内相談はアクティブなセッションで実行され、エグゼキューターを起動しません。

### Finalize Review ルート

プランレベルの finalize レビューは `gal pipeline <execution-prompt> --phase finalize-review` で実行します。既存の `pipeline.AUDITOR` ルートを使用し、新しいロールや設定キーは追加しません。

- 設定ファイル、`executorRouting` キー、`pipeline` グループ、または `AUDITOR` エントリが本当に存在しない場合、ディスパッチは `no-routing` を報告します。このケースでは、`Review Independence: DEGRADED_SAME_RUNTIME` と記録するプロセス内レビューが許可されます。ホストサブエージェントモードを明示的に選び、ホストの能力チェックに合格した場合は、文書化された別のレビュー例外になります。
- 設定ファイルが読み取れない場合、または有効な JSON でない場合、ディスパッチは `routing-invalid` を報告して停止します。`executorRouting` または `pipeline` がオブジェクトでない場合も、同じ結果になります。`AUDITOR` エントリが不正な形式である場合、`executor` が空の場合、モデルを解決できない場合、`sshTarget` と `remoteWorkdir` の一方しか設定されていない場合も同じです。これらのケースでは GAL は同一ランタイムへフォールバックしません。このフェーズでは、GAL は他のロールのエントリを検査しません。
- エグゼキューターの欠落、起動の失敗、タイムアウト、ゼロ以外の終了コード、レシートの欠落または空、プロバイダーのセッション証跡の欠落でも、レビューは停止します。これらのケースでも同一ランタイムへはフォールバックしません。
- レビューが成功するには、新しい空でないレシート、`completed` で終わる試行ログ、プロバイダーのセッション証跡が必要です。プロダクトの差分は必要ありません。

ホストが固定されていない場合、他のディスパッチは従来どおりの動作を維持します。それらのディスパッチでは、読み取れない設定や不正な形式の設定は従来どおり `no-routing` へ縮退します。ホストが固定されている場合は、停止します。

### `combinations` レジストリ

`executorRouting.combinations` レジストリは、再利用可能なエグゼキューター設定を定義します。各エントリには `executor` と、任意のオーバーライド（`model`、`effort`、`timeoutSecs`、`sshTarget`、`remoteWorkdir`）を指定できます。combination の `executor` はリテラルのエグゼキューター名である必要があり、別の combination は指定できません。combination の名前をリテラルのエグゼキューター名と同じにすることはできません。不正な combination は警告付きで拒否されます。`planning` ロールでは `model` と `effort` のみが読み取られます。各ロールは `executor` プロパティに combination の名前を指定し、フィールド単位で設定を上書きできます。なお、`sshTarget` と `remoteWorkdir` は必ずセットで指定する必要があります。

### `executors` 既定値ブロック

`executorRouting.executors` の各エントリには、文字列の短縮表記または完全な設定オブジェクトのいずれかを指定できます。

| フィールド | 必須 | 型 | 説明 |
| --- | --- | --- | --- |
| `model` | 必須 | string | エグゼキューターの既定モデル識別子。 |
| `effort` | 任意 | string | 推論エフォート（reasoning effort）の既定値。 |
| `timeoutSecs` | 任意 | positive integer | 秒単位の実行タイムアウト。`0` は警告付きで無視されます。 |

ロール設定のフォールバック解決順序は決定論的です。
**インラインのロール設定 → Combination 基本設定 → `executors[final.executor]` → `None`**
モデル解決チェーンで未設定の場合は空文字列になります。ロールが明示的に `timeoutSecs: 0` を指定した場合、既定値へフォールバックせず `None`（タイムアウトなし）として解決されます。

### 調査ルーティング

`research` グループは `pipeline` と同様にヘッドレスディスパッチを使用します。対照的に、`planning` グループは検証専用であり、ヘッドレスディスパッチを呼び出すことはありません。調査ワークフローでは、`RESEARCHER#0`、`RESEARCHER#1`、`RESEARCHER#2` という 3 つの並列ワーカーを使用します。グループのキー `"1"` は `RESEARCHER#1` へ、キー `"2"` は `RESEARCHER#2` へルーティングされます。キー `"0"` は、ワーカー #0 が常にネイティブのサブエージェントであるため、警告付きで読み飛ばされます。`RESEARCHER` のようなロール名のキーは、警告付きで読み飛ばされます。`2` より大きいキーは読み込まれますが、調査ディスパッチはそれを読み取りません。これらのワーカーは互いのコンテキストが見えない独立した環境で並行実行されます。`research` エントリーがないワーカーは、`executors.agy` の既定値が設定されていればそれを使い、なければネイティブのサブエージェントで実行されます。すべてのワーカーが完了した後、オーケストレーターが調査結果を直接統合します。このプロセスでは中間の検証ディスパッチステップは存在せず、`executorRouting` に `verify` キーを必要とすることもありません。選択されたワーカーインスタンスに対してのみルーティングが行われます。

### リモート実行

リモート実行のパラメーターは、補助ファイルを使わずに `executorRouting` 内の各ロールへ直接設定します。`pipeline` グループと `research` グループのロールは、`sshTarget` と `remoteWorkdir` を同時に指定して、SSH 接続先エンドポイントおよびリポジトリの作業ディレクトリを割り当てます。`planning` グループのロールはヘッドレスプロセスを起動しないため、これらのフィールドは適用されません。これらの設定は機密性の高いマシンローカルなインフラ情報であるため、リポジトリ間で共有してはいけません。

認証情報やシークレットを `config.json` に配置することは固く禁止されています。GAL が設定ファイルから認証情報を抽出することはありません。

### Codex の承認とサンドボックス設定

Codex の承認ポリシーやサンドボックスは `config.toml` 内で設定するか、または Codex Desktop のタスクモードから選択します。コマンド実行の例外ルールは、`~/.codex/rules/*.rules` で別途設定します。これらの設定は GAL の `executorRouting` や `timeoutSecs` とは独立して管理されており、GAL が Codex 本体の設定を変更することはありません。`allow` ルールに合致する親プロセス側の信頼済みコマンド（RTK または GAL）は、承認プロンプトなしで親サンドボックス外で直接実行されます。このルールによって、ディスパッチされた子プロセスのサンドボックスが変更されることはありません。検証済みの設定、ルールの記述例、動作確認、復旧およびロールバックの手順については、[Codex パイプライン実行のセットアップ](setup.ja.md#codex-パイプライン実行のセットアップ) を参照してください。

ヘッドレスディスパッチの実行時、GAL は Codex の子プロセスを `--dangerously-bypass-approvals-and-sandbox` 付きで起動します。子プロセスは Codex のサンドボックス外で、他のエグゼキューターと同じ信頼レベルで実行されます。これは、Codex の Windows サンドボックスが現在コマンドを実行する前に失敗するためです。詳細は [setup.ja.md](setup.ja.md#既知の問題-codex-windows-サンドボックスの障害) を参照してください。親の Codex セッションの承認モードによって、子プロセスの起動方法が変わることはありません。拒否された操作が承認済みとして扱われることはなく、観測不能な権限状態は一貫して Unknown（不明）として分類されます。

## マシンローカルパスのプレースホルダー

Git フィルター処理（`gal clean` および `gal smudge`。詳細は [ワークフロー](workflows.ja.md) を参照）は、以下のプレースホルダーを使用してマシンローカルなパスを除去します。

| プレースホルダー | 定義 | 適用先 |
| --- | --- | --- |
| `<GAL_SKILLS>` | `galSkills` キーのローカルスキルディレクトリの絶対パス。 | `gal clean` と `gal smudge` の Git フィルター。 |

パスの置換は `galSkills` にのみ適用されます。他の設定キーがプレースホルダー置換の対象になることはありません。

## Model Context Protocol（MCP）サーバー

GAL は 2 つの異なるソースから MCP サーバー設定を集約し、`gal refresh` 実行時に `~/.gal/plugins/gal/` の正規ルート（canonical root）へ統合します。

| ソース | 配置場所 | 説明 |
| --- | --- | --- |
| **Core** | `plugins/gal-core/mcp.json` | GAL コアが必要とするバージョン管理対象のサーバー。既定では空であり、コア機能で必須となった場合にのみエントリが追加されます。 |
| **Personal** | `~/.gal/local/mcp.json` | ホストマシン固有のユーザー定義サーバー。追加の設定フラグは不要であり、ファイルの存在自体が有効化のトリガーとなります。 |

両ファイルともに、標準の `"servers"` 辞書スキーマを使用します。

### マージと生成フロー

`gal refresh` を実行すると、以下の手順で MCP サーバーが処理されます。

1. コア側の `mcp.json` にある `"servers"` 辞書を、`~/.gal/plugins/gal/.mcp.json` の `"mcpServers"` キー配下へ変換して書き込みます。
2. `~/.gal/local/mcp.json` が存在する場合、そのサーバー定義を `"mcpServers"` 辞書へ統合します。個人サーバー名がコアサーバー名と衝突した場合、個人側の定義は無視され、コア側の設定が優先されます。個人ファイルが存在しない場合や JSON が不正な場合でも、GAL はコアサーバーを保持したまま正常に処理を続行します。
3. 統合されたサーバー一覧を、Agent Plugins 標準フォーマットに従って `~/.gal/plugins/gal/mcp.json` に書き出します。このファイルでは、`"type": "http"` が `"streamable-http"` に書き換えられます。`stdio` や `sse` を含むその他の型は変更されません。`.mcp.json` ファイルでは `"http"` のまま残ります。

サーバー設定はパスプレースホルダーの置換を行いません。変更点は手順 3 の `type` の書き換えだけです。サーバー設定内の環境変数（`${ENV_VAR}`）は、プロセスの起動時に MCP ホストランタイムによって動的に解決されます。これらの環境変数は `config.json` ではなく、ホストのシェル環境で定義してください。

補助ファイルによる設定のオーバーライドには対応していません。コアサーバーを変更するには、`plugins/gal-core/mcp.json` を編集し、チェックアウトから `gal dev-apply` を実行してください。個人サーバーを変更するには、`~/.gal/local/mcp.json` を編集して `gal refresh` を再実行してください。Claude Code などのランタイムでは、設定の変更を反映させるためにプロセスの再起動またはプラグインの再読み込みが必要です。

個人の MCP レイヤーは、ローカルで `gal refresh` または `gal dev-apply` を実行した際に統合されます。公開マーケットプレイス用スナップショットのビルド成果物には、個人設定は一切含まれません。

### ランタイム対応マトリクス

GAL が外部アプリケーションのディレクトリに MCP サーバー設定を直接注入したり、サードパーティの設定ファイルを書き換えたりすることはありません。各ランタイムは、GAL をプラグインとして読み込む際にサーバーを検出します。

| ランタイム | 設定対象 | 統合状況 |
| --- | --- | --- |
| Claude Code | `.mcp.json` | ネイティブ対応。 |
| OpenAI Codex | `.mcp.json` | ネイティブ対応。 |
| GitHub Copilot | `mcp.json` | ネイティブ対応。 |
| Google Antigravity | なし | プラグインコンテナ内での動作は公式には未検証。 |
| opencode | なし | 未対応。opencode は Markdown プラグインを読み込まず、ファイルプロジェクションにも MCP 設定は含まれません。 |

「ネイティブ対応」とは、ランタイムのプラグインローダーが期待する設定ファイルを GAL が正しく生成することを意味します。実際に読み込まれるかどうかは各ランタイムのサポート状況に依存し、[セットアップ](setup.ja.md) に記載された登録手順を完了する必要があります。

GAL が `claude_desktop_config.json`、`~/.copilot/mcp-config.json`、`~/.codex/config.toml`、`opencode.json`、`mcp_config.json` など、ホスト管理の設定ファイルを改変することは一切ありません。これらはユーザー自身が管理するファイルです。システム全体で共有するツールは、各ランタイム独自の設定ファイルに直接記述してください。

### 設定例: カスタム MCP サーバー

以下は、`~/.gal/local/mcp.json` を通じてカスタム MCP サーバーを登録する設定例です。環境変数を変えることで、単一のツールの複数インスタンスを登録できます。以下の例では `postgres-mcp` パッケージを `uvx` 経由で実行しています。

```json
{
  "servers": {
    "postgres-app": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_APP_URI}" }
    },
    "postgres-analytics": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_ANALYTICS_URI}" }
    }
  }
}
```

この内容を `~/.gal/local/mcp.json` に保存し、シェル上で `POSTGRES_MCP_APP_URI` と `POSTGRES_MCP_ANALYTICS_URI` をエクスポートしてから `gal refresh` を実行すると、生成されるプラグインマニフェストに両方のデータベースサーバーが登録されます。

## 計画言語設定（`planLanguage`）

`planLanguage` キーは、プロンプトで明示的な指示がない場合に生成される `.dev/plans/*.md` および `.dev/research/*.md` の既定の出力言語を指定します。言語の解決階層は以下のとおりです。
**ユーザーの明示的な指示 → `planLanguage` の設定値 → プロンプト言語の自動検出 → デフォルトの `en`**
リポジトリレベルのドキュメント方針は、リポジトリメタデータの `PROJECT_LANGUAGE` によって個別に制御されます。モデル実行用のプロンプト（`.dev/plans/*.prompt.md`）は、複数モデル間での決定論的な実行性能を維持するため、常に厳格に英語（English）で生成されます。

`planLanguage` に英語以外の言語を指定した場合、以下の 3 層構造の計画ワークフローが有効になります。

1. **英語技術ドラフト**（`.dev/plans/<slug>.en.md`）: 正本となる正規の技術仕様を定義します。
2. **現地語作業計画**（`.dev/plans/<slug>.md`）: 人間がレビュー・編集可能な現地語ファイル。セクションヘッダー、ファイルパス、タスク識別子、ステータス判定は英語を維持し、説明文のみが現地語化されます。
3. **実行プロンプト**（`.dev/plans/<slug>.prompt.md`）: モデル実行用にコンパイルされた最終的な英語プロンプト。

現地語化された計画に対する手動編集は確実に保持されます。後続の計画コマンドが実行されると、読み取り専用の調整ステップ（reconciliation）で一時停止し、手動で加えられた変更を英語ドラフトへ逆反映（バックポート）します。手動編集が上書きされて失われることはありません。`/plan-to-prompt` が最終実行プロンプトをコンパイルする際、中間ドラフトは削除され、永続的な 2 つの計画ファイルのみが残ります。なお、`planLanguage: "en"` を指定した場合は、翻訳レイヤーをバイパスして直接英語で運用されます。

## 使用量記録の入力ファイル

`gal usage collect` を実行するとき、使用量記録は実行ごとに 2 つの任意の入力ファイルを読み取ります。どちらのファイルも `~/.gal/config/config.json` には属さず、トークンやコストのカウンターも含みません。契約は [`usage-accounting.md`](../../../plugins/gal-core/conventions/usage-accounting.md) です。

```text
gal usage collect --run <run-id> [--binding <path>] [--rates <path>]
```

### 紐付けファイル（`--binding`）

preflight がオーケストレーターの紐付けを記録している場合、`--binding` は省略できます。`gal pipeline-preflight` に `--host-session <host>:<session-id>` を渡すと、preflight は固定されたホストと作業ディレクトリに照らしてそのセッションを検証します。検証に成功すると、`--binding` なしの `gal usage collect --run <run-id>` は記録済みの紐付けから収集します。preflight が紐付けを記録していない場合、収集は理由を示して失敗します。その場合は `--binding` が必要です。

紐付けファイルは、実行、ホストセッション、ソースの時間枠を指定します。JSON オブジェクトで、次のフィールドだけを含みます。`run`、`host_session`、`billing_mode`、`role`、`window`、任意の `cursor` です。

```json
{
  "run": {
    "version": 1,
    "run_id": "run-a",
    "canonical_worktree": "C:/work/repo",
    "plan_scope": "plan-a",
    "host_session": "codex:sess-a",
    "host_window": "window-a"
  },
  "host_session": "codex:sess-a",
  "billing_mode": "standard",
  "role": {"kind": "orchestrator"},
  "window": {"started_at": "2026-10-20T00:00:00.000Z"}
}
```

- ほかのフィールドがあると、収集は失敗します。この規則には、`input_tokens` などのカウンターのフィールドや、評価のフィールドも含まれます。
- `run.run_id` の値は、`--run` の値と一致している必要があります。
- `host_session` が実行の識別情報にあるホストセッションと異なる場合、収集は紐付けの競合として失敗します。ソースを読み取る前に失敗します。

### レートセットのファイル（`--rates`）

レートファイルは、オーナーが提供するレートセットです。スキーマのバージョンは `1` です。GAL はレートを取得しません。

```json
{
  "schema_version": 1,
  "rate_set_id": "rates-a",
  "rates": [
    {
      "provider": "codex",
      "model": "gpt-x",
      "billing_mode": "standard",
      "category": "input",
      "unit": "per_million_tokens",
      "price": "2.5",
      "money_unit": {"currency": "USD"}
    }
  ]
}
```

- レートは、プロバイダー、観測されたモデル、課金モード、単位をキーにします。設定されたルーティング名が、観測されたモデルを置き換えることはありません。
- 価格は 10 進数の文字列です。負の価格は解析に失敗します。リクエスト単位の価格で、トークンのカウンターに値を付けることはできません。
- レートファイルがない場合、レートから導く推定はすべて不明のままです。例えば、レポートは `estimate: unknown` と表示します。
- レートセットを変更すると、新しいスナップショットが作成されます。以前のスナップショットは変わりません。レートセットを変更しても変わるのは見積もりだけです。再試行したタスクの 160 や、子を含む親の 200 などの単位の合計は変わりません。
- `gal usage report` は保存済みのレコードだけを読み取ります。`gal usage assess` は評価の入力を受け取り、紐付けファイルもレートファイルも必要としません。

## メモリーハーベスティング（`memoryHarvest`）

メモリーハーベスティングは、モデルがアクティブなセッション中に得た運用上の知見を抽出し、リポジトリファイルへ永続化することで、セッション再起動後もコンテキストを維持できるようにする機能です。

- **既定では無効**: 有効化するには `config.json#memoryHarvest.enabled: true` を設定します。省略された場合や `false` が設定されている場合、本機能は無効のままとなります。
- **範囲を限定した探索**: GAL が候補抽出のためにグローバルな会話履歴やローカルのセッションダンプ全体をパース、インデックス、スキャンすることは決してありません。
- **コンテキスト制約と匿名化**: 知見の候補は具体的なリポジトリファイルを参照している必要があります。また、生の会話ログ、シークレット、マシンローカルなパス、個人的なメモは事前に自動除去（サニタイズ）されます。
- **人間の承認が必須**: 抽出された候補は `/gal wrap-up` の実行時にのみ確認・承認できます。未承認の候補はディスクに書き込まれることなく破棄されます。承認されたエントリは、アクティブプランの引き継ぎメモに参考タスクコンテキストとして記録されます。
- **公式ドキュメントへの昇格**: メモリーハーベスティングで承認されても、直ちにドキュメントが改変されることはありません。承認された知見を `docs/` 配下の公式ドキュメントへ昇格させるには、`/gal finalize` 実行時に独立した検証を行う必要があります。

## ローカルカスタマイズ

### 個人ディレクトリルート（`~/.gal/local/`）

ユーザー固有の資産は `~/.gal/local/` に配置します。これらの資産は正規ルートへレンダリングされ、すべてのアクティブなランタイムへプロジェクションされます。GAL は `local/skills/`、`local/mcp.json`、`local/conventions/` から設定を読み込みますが、**ユーザー管理ディレクトリ内のファイルを改変、作成、削除することは決してありません**。複数マシン間でのファイル同期はユーザー自身が管理します。

#### ディレクトリ構成

```text
~/.gal/local/
  skills/
    <skill-name>/
      SKILL.md          ← 個人のスキル定義（gal からは読み取り専用）
  mcp.json              ← 個人の MCP サーバー定義（gal からは読み取り専用）
  conventions/
    <lang>.md           ← 個人の言語コーディング規約（gal からは読み取り専用）
```

#### 有効化の仕組み

設定フラグではなく、ファイルの存在そのものによって有効化されます。`~/.gal/local/` 配下にファイルを配置するだけで個人オーバーレイが有効になります。ディレクトリやファイルが存在しない場合、生成処理はコアの既定の挙動と完全に一致します。

#### 優先順位と競合解決

- 個人のスキルはコアスキルの後に評価されます。個人のスキルがコアスキルと同一の識別子を持つ場合、個人側のバージョンはスキップされ、コアスキルが優先されます。
- 個人の MCP サーバーは正規の `.mcp.json` へ統合されます。名前が衝突した場合はコア側の定義が優先されます。
- `gal doctor` を実行すると、個人のスキル数、アクティブな規約ファイル、および名前の競合によってスキップされた定義がレポートされます。

### コーディングスタイル規約

コアの GAL テンプレートが独自のコーディングスタイルを強制することはありません。対象リポジトリへのカスタムスタイルの適用は、以下の 3 つの方法で行うことができます。

#### ソース 1: 個人規約ファイル（リポジトリ全体に適用）

`plugins/gal-core/templates/csharp-convention.example.md` を `~/.gal/local/conventions/csharp.md` にコピーし、ルールを編集します。指定可能なファイル名は `csharp`、`typescript`、`go` に厳格に限定されています。`typescript` ファイルは、設定の `Language` に TypeScript または JavaScript のいずれかが指定されたプロジェクトに適用されますが、ファイル名自体は必ず `typescript` としてください。以前に一般的な名前を使用していた場合は、承認されたファイル名に変更してください。`.dev/project.md` の `Language` 設定と一致するリポジトリで `gal render-adapters` を実行すると、組み込み規約と同様にアダプターの指示ブロックへルールが注入されます。

#### ソース 2: インストール済みプラグインの検出（設定不要の自動連携）

使用しているコーディングランタイムに公式の言語プラグインがインストールされている場合、対象リポジトリで `gal render-adapters` を実行します。GAL はプラグインのメタデータとリポジトリの `Language` フィールドを照合し、インストールされたプラグインを参照する **Detected Language Skills** 指示ブロックをレンダリングします。GAL が外部プラグインのソースコードをコピーしたり、プラグインをインストール・更新・削除したりすることはありません。ランタイム拡張機能を新しく導入した後は、`gal render-adapters` を実行して参照情報を更新してください。

#### ソース 3: 個人スキル（オンデマンド読み込み）

`~/.gal/local/skills/<name>/SKILL.md` にカスタムスキルを作成します。Claude Code では新しいスキルディレクトリをインデックスするために再起動が必要です。他のランタイムでは次回の読み込みサイクルで自動的に検出されます。これらのスキルは名前で明示的に呼び出された場合にのみロードされるため、グローバルなコンテキストウィンドウを圧迫しません。

#### カスタマイズソースの使い分け

公式プラグインが導入された標準的な環境では、メンテナンス不要の「ソース 2」を選択してください。複数のプロジェクト間で一貫したチーム規約を適用したい場合は「ソース 1」を使用します。特定用途に特化したアドホックなエージェントワークフローには「ソース 3」が適しています。

#### フォールバックの挙動

リポジトリの `Language` メタデータに一致する規約ファイルやプラグインが存在しない場合、生成されるアダプターでは言語規約ブロックが省略されます。これは正常なフォールバック動作であり、エラーではありません。

#### 自動規約適用の無効化

ソース 1 およびソース 2 からの自動規約注入を無効にするには、`.dev/project.md` の `Tech Stack` 表に `| Personal Conventions | off |` という行を追加してください。公開リポジトリでは、マシンローカルな設定がコミット対象のアダプターに漏洩するのを防ぐため、この設定を有効にすることを推奨します。なお、無効化した場合でも「ソース 3」のスキルはオンデマンドで引き続き利用可能です。

### ローカルコマンドオーバーレイ（`SKILL.local.md`）

コマンドスキルをローカル環境独自にカスタマイズするには、`plugins/gal-core/commands/<command>/SKILL.local.md` を作成します。

- このファイルは `.gitignore` に登録されており、ユーザー管理の設定として扱われます。
- ビルド処理の過程で、`SKILL.local.md` の内容が自動生成される `SKILL.md` の末尾に追加されます。
- `plugins/gal-core/commands/<command>/SKILL.md` を直接編集してはいけません。そのファイルはプロジェクション時に自動的に再生成されます。
- `SKILL.local.md` は補助的なプロンプト指示の提供専用です。front matter ブロックを含めてはいけません。

### 外部ノート統合（`local-notes`）

外部の個人ノート環境との連携は完全にオプションです。GAL はリポジトリの状態と個人のプライベートノートボルト（vault）を厳格に分離しており、コア機能が外部ノートストレージに依存することはありません。詳細な仕様は [`optional-capabilities.md`](../../../plugins/gal-core/conventions/optional-capabilities.md) に記載されています。ノート統合は既定で無効化されており、ローカルバックエンドの利用可能性が確認された場合にのみ有効になります。

ノートバックエンドが利用できない場合、未設定の場合、またはオフラインの場合でも、システムはエラーを発生させることなく、標準のリポジトリワークフローへ安全に縮退（degrade）します。明示的に接続されたバックエンドのみが、承認されたワークフローのチェックポイントにおいて読み書きを行うことができます。ノート統合がコア機能の前提条件になることは決してありません。ノートバックエンドを持たないリポジトリも、プライベートボルトのコンテキスト参照を除き、接続済みリポジトリとまったく同様に機能します。

サポートされているバックエンド実装には、Obsidian（`coddingtonbear/obsidian-local-rest-api`）、Logseq（`ergut/mcp-logseq`）、Joplin（`joplin-mcp`）、汎用 Markdown ディレクトリ（`vault-mcp`）、および日中韓（CJK）テキスト検索に特化した `SeekLink` があります。各実装によって利用可能な機能範囲は異なります。

リポジトリの調査ドキュメントは、既定で `.dev/research/` 配下に保存されます。

### コンテキスト内ロールのディスパッチ

architect、analyst、designer、releaser の各スタンドアロンコマンドスキルは、コンテキスト内相談に `gal consult-script <role>` を使用します。このディスパッチはローカルで実行され、SSH の pipeline executor ルートは使用しません。旧形式の `/gal discuss <role>` はエラーとなり、代わりに使うスタンドアロンコマンドを示します。

## リポジトリ衛生チェック

`gal finalize-check` には `project-md-bound` があり、LF に正規化した UTF-8 で測定した `.dev/project.md` が 30,720 B の助言的上限を超えているかを報告します。`state-bound` は `.dev/state.md` の 16,384 B 上限と有限な表の形式を確認します。衛生上の検出結果はこの 2 つの有限インデックス内にある修復可能な問題です。対応するソース文書の形式やパスの検出結果も助言扱いです。計画、プロンプト、レシート、調査記録、承認証拠、ルートアダプター、未対応の state 競合、同じ箇所の競合は整合性上の検出結果です。衛生上の問題は `advisory` となり、レシートをブロックしません。ファイルを読み取れない場合と整合性上の問題は引き続き `fail` です。レシートでは `pass` と `advisory` を通過扱いにし、`fail` と `not-run` はブロックします。

パイプラインの事前チェックと finalize の完全モードゲートの前に、GAL は `.dev/project.md` と `.dev/state.md` をセクション単位で修復し、同じチェックを一度再実行します。他のファイルは自動修復しません。同じ呼び出しでゲートが失敗した後に修復を続けることもありません。再チェック後も問題が残る場合、実行中の計画 slug をキーに `.dev/state.md` の `Hygiene debt:` 行を追加または更新し、引き継ぎと finalize の報告に含めます。再チェックが成功したら行を削除します。`project-md-bound` の上限と、検出分類、修復、復元、債務、整合性の規則は[`.dev` 衛生・整合性規約](../../../plugins/gal-core/conventions/token-budget.md#dev-findings-hygiene-and-integrity)を唯一の基準とします。

他のパスも未マージの場合でも、`gal state-merge` は `.dev/state.md` の対応する別々のインデックス箇所を解決し、他の未マージパスには触れません。同じ箇所の競合、権限を持つファイルの競合、対応範囲外の競合は停止します。翻訳の出典パス、言語、出典コミットは文書の翻訳メタデータに記録し、規則の唯一の出典は英語版です。
