---
source: docs/glossary.md
lang: ja
source_commit: a6215edfb68294314a0836a3af48411d6465faf7
translated_at: 2026-09-23
type: Reference
title: ja 用語表示プロファイル
description: ja 翻訳の用語表示基準として、確定した英語用語を日本語文中で表記する形式を定める。
tags:
  - terminology
  - locale
  - presentation
  - ja
status: stable
---

# ja 用語表示プロファイル

本ドキュメントは翻訳ではなく、[`docs/glossary.md`](../../glossary.md) の日本語（ja）表示プロファイルである。確定した英語用語を日本語文脈でどのように表記するかを規定するものであり、用語の意味そのものを定義する権威は持たない。GAL 中核用語の定義は常に `docs/glossary.md` を唯一の情報源とする。両者に相違がある場合は、本ドキュメント側を修正する。

## 文書の役割

本ドキュメントは、`docs/i18n/ja/` 配下の全日本語ドキュメントで共有する表記基準を定義する。日本語ドキュメントの翻訳や修正を行う前に、まず本ドキュメントで該当用語の表記を確認すること。未登録で表記揺れが生じうる語は、作業前に本ドキュメントへ登録する。同一の英語用語に対し、すべての訳文で一貫した日本語表記を維持することを目的とする。

日本語の技術文書では、英語の外来語（カタカナ表記）と既存の漢語（漢字表記）の使い分け、ならびに長音符号や送り仮名の扱いで表記揺れが発生しやすい（例えば file は「ファイル」、document は「ドキュメント」または「文書」）。そのため、本ドキュメントでは**各用語の表示モードの指定**と**正書法（表記規則）の統一**を中心に取り扱う。

本ドキュメントでは、以下の 3 つの表と 2 つの慣例節により表記を規定する。

1. [GAL 中核用語表示表](#gal-中核用語表示表)：`docs/glossary.md` の Part B・Part C・Part D を完全に網羅する。行の追加・削除を行う前に、必ず `docs/glossary.md` に対応する登録が存在するか確認する。
2. [一般技術用語：採用表記一覧](#一般技術用語採用表記一覧)：GAL 固有ではない一般技術語のうち、日本語で表記が分かれやすい用語の採用表記を固定する。カタカナと漢字の選択、および正字法の揺れを抑えることを目的とする。
3. [訳文一貫性の補充語](#訳文一貫性の補充語)：`docs/glossary.md` に未登録で、訳文中に複数の表記が出現した語を扱う。表記のみを固定し、意味に関する権威は持たない。

各表を参照する前に、[表記の根拠と優先順位](#表記の根拠と優先順位) および [正字法の慣例](#正字法の慣例) を通読すること。

## 表記の根拠と優先順位

**本ドキュメントで定める表記は独自に造語するものではなく、公的機関や主要ベンダーの確立された用例に基づく。** 判断が分かれる場合は、以下の優先順位に従って表記を決定する。

1. **日本政府の公用文基準** — 一般的な漢字使用、送り仮名、外来語表記に関する最上位の根拠とする。「公用文作成の考え方」（令和4年 文化審議会建議）、「常用漢字表」、「外来語の表記」（内閣告示第2号）に準拠する。
2. **主要ベンダーのローカライズ基準** — 製品名、AI 関連用語、IT 固有用語など、公用文基準が扱わない領域の根拠とする。Microsoft、Anthropic、OpenAI、Google の公式日本語用例を優先する。製品名（Claude Code、Codex、Google Antigravity など）は原則として原文の英語表記を維持する。
3. **JTF 日本語標準スタイルガイド** — 上記の基準で判断が分かれる場合の補助基準（タイブレーク）として用いる。長音、送り仮名、和欧混在表記の細部を統一する。

この優先順位は、`docs/glossary.md` Part B の権威階層（標準化団体 → Linux Foundation AAIF → ベンダー）と同一の思想に基づく。意味の権威が英語正本に存在するのと同様に、日本語表記の権威も外部の確立された基準に準拠し、本ドキュメントはその方針を明文化したものである。

## 正字法の慣例

日本語ドキュメント全体において、以下の正字法を一貫して適用する。いずれも公的基準および主要ベンダーの用例に準拠した現代的な慣例である。

- **長音符号は語末でも省略しない。** 「サーバー」「ユーザー」「コンピューター」「メモリー」「フォルダー」「プリンター」とし、旧 JIS 方式の語末省略表記（サーバ、ユーザ、コンピュータ など）は使用しない。Microsoft のローカライズ方針（2008 年以降）および内閣告示「外来語の表記」に基づく現在の主流に合わせる。
- **送り仮名は内閣告示「送り仮名の付け方」に準拠する。** 「引数」「戻り値」「呼び出し」「読み込み」「書き込み」を採用し、「引き数」などの表記揺れを避ける。
- **確立された漢語概念には漢字を使用する。** 「設定」「実装」「変数」「関数」「配列」「文字列」「削除」「更新」「保存」など、日本の IT 分野で定着している漢語はカタカナに置き換えない（「セッティング」「インプリメント」などは使用しない）。
- **定訳となる漢語がなく、外来語として定着している概念にはカタカナを使用する。** ファイル、サーバー、プロセス、スレッド、キャッシュ、ネットワーク など。
- **公用文の指針に倣い、副詞・接続詞・形式名詞は原則としてひらがなで表記する。** 「および」「または」「ため」「とき」「こと」を使用し、「及び」「又は」「為」「時」「事」は避ける。

漢字とカタカナの選択は上記の基準によって語ごとに定まっているため、文書ごとに都度判断しない。判断に迷う用語は本ドキュメントの表に登録し、以降は当該表を参照する。

## 日本語の句読点の扱い

リポジトリ全体のセミコロンポリシーは、[docs/architecture.md の「Punctuation Policy: Semicolons Prohibited」](../../architecture.md#punctuation-policy-semicolons-prohibited) で定義されている。以下では、そのポリシーを日本語文で適用する方法だけを示す。日本語文でセミコロンを使いたくなる箇所は、一文の中に複数の完結した文意が詰め込まれている兆候である。内容が独立している場合は二文に分割する。主従関係または因果関係がある場合は、読点（、）と「そして」「しかし」「そのため」「一方で」などの適切な接続詞を用いて文構造を明確にする。

## 5 つの表示モード

`docs/glossary.md` の「Semantic Authority vs. Locale Presentation Profiles」節で定義された 5 つの表示モードについて、日本語における判断基準を以下に示す。

- **keep-en（原文保持）**：英語の原語をそのまま使用し、翻訳しない。すべての機械可読なリテラルがこれに該当し、コマンド名（`gal init`、`/gal pipeline`）、設定キー（`executorRouting`、`sshTarget`）、ファイルパス（`.dev/plans/`）、コード識別子、ロールファイル名（`golem-architect`）、製品名（GAL、Claude Code、Codex）が含まれる。これらを翻訳すると実際の入力内容との対応が失われるため、常に原文を保持する。
- **bilingual-first-use（初出時の併記）**：GAL 中核概念の既定モードである。同一文書内で最初に出現する箇所では「日本語表記 (English)」の形式を使用し、以降は日本語表記のみ、または英語原語のみとする。**同一文書内で同じ英語用語に対し、二種類以上の日本語表記を使用してはならない。** [GAL 中核用語表示表](#gal-中核用語表示表) にて各語の日本語表記を一意に固定する。
- **localized（現地化訳）**：GAL 固有ではない一般的な技術用語に適用する。日本語の慣用表記をそのまま使用し、英語の併記は行わない。
- **transliterated（音訳）**：意味ではなく音に基づいて現地表記へ転写するモードである。**GAL には現時点でこのモードに該当する用語は存在しない。** GAL 固有の名詞の多くは製品名やコード識別子であり、すべて keep-en として扱われる。
- **contextual（文脈依存）**：一部の語は、文脈によって表示方法が変化する。文がその用語自体について論じている場合（命名規則の説明やロール名の列挙など）は keep-en を優先する。文中で付随的に言及される概念にすぎない場合は、bilingual-first-use または localized として扱う。

## AI 関連語の表記方針

AI 分野の用語は急速に変化するが、**本プロファイルではその変化を直接追従しない。** 代わりに以下の二つの仕組みに委ねる。

- **意味の安定性は `docs/glossary.md` が担保する。** Part B において各語を権威ある規格（ISO/IEC 22989、Linux Foundation AAIF、各ベンダー）に紐付けて固定している。日本語プロファイルは独自に概念を定義したり造語を行ったりせず、表示モードの選択のみを担う。
- **陳腐化は staleness ゲートが検出する。** `docs/glossary.md` が AI 用語を追加または変更すると、本ファイルの `source_commit` と不一致になり、`gal translation-freshness` が本ファイルを stale として報告する。これにより該当箇所の再確認が促される。

AI 用語の**表記決定**は以下の原則に従う。日本の IT 分野にはすでに階層別の慣例が定着しており、都度の恣意的な判断は行わない。

| 分類 | 日本語の慣用処理 | 表示モード | 例 |
| --- | --- | --- | --- |
| 固有のプロトコル／製品名 | 常に英語を保持 | keep-en | MCP、AGENTS.md、Claude Code、Codex、LLM |
| 学術界で定着した漢語 | 漢字（意味が安定している用語） | localized | 推論、学習、生成、分類 |
| 定着した外来語 | カタカナ | bilingual-first-use | エージェント、プロンプト、チャットボット |
| 未定着の新語 | 原文保持またはカタカナ、無理な漢字造語は避ける | keep-en 優先 | agentic、coding agent |

日本語の技術文書では、「生成AI」（漢字＋アルファベット）や「AIエージェント」（アルファベット＋カタカナ）のように、文字種が混在した表記（和欧混在表記）が一般的に定着している。GAL においてもこの慣例に従い、無理な漢字の独自造語は行わない。

**結論として**、流動的な AI 用語については原則として **keep-en または初出時併記（bilingual-first-use）** を採用し、**独自の日本語訳を新造しない。** 日本の機械学習（ML）分野で定着している漢語（推論、学習、生成など）のみ漢字を使用する。

## GAL 中核用語表示表

本表は `docs/glossary.md` の Part B・Part C・Part D を完全に網羅する。「出典」列にてどの Part に登録されているかを明示している。canonical で一つの行にまとめられた概念は、日本語側の検索性に応じて複数の行へ分ける場合がある。各語の日本語表記は一意に固定される。「避ける表記」列には、過去に発生した誤用や表記揺れの例を記載している。並び順は英語見出し語のアルファベット順である。

### Part B — 外部概念

| 英語用語 | 出典 | 表示モード | ja 固定表記 | 避ける表記 |
| --- | --- | --- | --- | --- |
| `agentic coding tool` / `agentic development platform` | Part B（`coding agent` の別名） | keep-en | 原文保持。日本語で説明が必要な場合は「エージェント型コーディングツール」 | 独自の漢字訳、エージェント的ツール |
| `AGENTS.md standard` | Part B | bilingual-first-use | AGENTS.md 標準 (AGENTS.md standard) | AGENTS.md 規格 |
| `AI agent`（属概念） | Part B | bilingual-first-use | AIエージェント (AI agent) | AI エージェンシー、知的エージェント |
| `chatbot` / `AI assistant` | Part B | localized | チャットボット / AIアシスタント | 対話ボット、AI 秘書 |
| `coding agent` | Part B | bilingual-first-use | コーディングエージェント (coding agent) | プログラミングエージェント、コード作成エージェント |
| `MCP` | Part B | keep-en | 原文保持（初出時に「モデルコンテキストプロトコル」を併記可） | 訳語の単独使用 |
| `MCP client` | Part B | keep-en | 原文保持 | MCP クライアント端末 |
| `MCP host` | Part B | keep-en | 原文保持 | MCP ホスト機、MCP 主機 |
| `MCP server` | Part B | keep-en | 原文保持 | MCP サーバ（長音省略） |
| `model` / `LLM` | Part B、Part C（裸語保持） | keep-en（略語） | `LLM`（初出時に「大規模言語モデル」を併記可）。汎称は「モデル (model)」 | 大型言語モデル単独表記 |
| `surface` / `interface` | Part B | localized | インターフェース | インタフェース（長音省略） |

MCP 関連の用語群はすべて keep-en とする。`host`、`client`、`server` が MCP 仕様において厳密に定義された役割を持つためである。日本語の「ホスト／クライアント／サーバー」に翻訳すると、エージェント側が host であり、ツールを提供する側が server であるという関係性を誤認するリスクが高まる。原文を保持することで、`docs/glossary.md` Part B の定義との直接的な整合性を確保する。

`docs/glossary.md` の廃止用語一覧は、本ドキュメントには重複掲載しない。ネーミングゲートは `docs/glossary.md` を直接参照するため、重複掲載は陳腐化するコピーを増やす結果にしかならないためである。

### Part C — GAL 内部語彙

| 英語用語 | 出典 | 表示モード | ja 固定表記 | 避ける表記 |
| --- | --- | --- | --- | --- |
| `agent`（裸語保持、golem agent を指す） | Part C | bilingual-first-use | エージェント (agent) | 代理人、AIエージェント（golem agent の意味での混同） |
| `canonical root` | Part C | bilingual-first-use | 正規ルート (canonical root) | カノニカルルート、標準ルート単独表記 |
| `dispatch` | Part C | bilingual-first-use | ディスパッチ (dispatch) | 配送、割り当て単独表記 |
| `execution prompt` | Part C | bilingual-first-use | 実行プロンプト (execution prompt) | 実行指示書 |
| `executor` | Part C | bilingual-first-use | エグゼキューター (executor) | 実行者、実行プログラム |
| `executorRouting` | Part C | keep-en | 原文保持、日本語を付けない | いかなる訳語も付けない |
| `executorRouting.pipeline`（ロールグループ） | Part C | keep-en + 補足 | 原文保持、補足時は「`pipeline` ロールグループ」 | パイプライングループ単独表記、独自の意訳 |
| `executorRouting.planning`（ロールグループ） | Part C | keep-en + 補足 | 原文保持、補足時は「`planning` ロールグループ」 | プランニンググループ単独表記、独自の意訳 |
| `(file) projection backend` | Part C | bilingual-first-use | 投影バックエンド (projection backend) | マッピングバックエンド |
| `GAL MCP boundary` | Part C | bilingual-first-use | GAL の MCP 境界 (GAL MCP boundary) | MCP サーバー全体設定 |
| `_galProjection` | Part C | keep-en | 原文保持、日本語を付けない | いかなる訳語も付けない |
| `generated runtime adapter` | Part C | bilingual-first-use | アダプター (adapter)。完全な表現が必要な場合は「生成されたランタイムアダプター」 | アダプタ（長音省略） |
| `golem agent` | Part C | keep-en + 補足 | `golem` は原文保持、補足時は「golem エージェント」 | ゴーレムエージェント（訳語の単独使用） |
| `local-first` | Part C | bilingual-first-use | ローカルファースト (local-first) | ローカル優先単独表記 |
| `local-notes` | Part C | bilingual-first-use | ローカルノート (local-notes) | ローカルメモ |
| `Personal Enhancement` | Part C | bilingual-first-use | パーソナル拡張 (Personal Enhancement) | 個人強化、個人化拡張 |
| `pipeline` | Part C | bilingual-first-use | パイプライン (pipeline) | 流水線 |
| `plan-task ID` | Part C | bilingual-first-use | プランタスク ID (plan-task ID) | 計画タスク識別子 |
| `projection` | Part C | bilingual-first-use | 投影 (projection) | マッピング、プロジェクション単独表記 |
| `projection surface` | Part C | bilingual-first-use | 投影サーフェス (projection surface) | マッピングインターフェース |
| `runtime` | Part C | bilingual-first-use | ランタイム (runtime) | 実行時単独表記、ランタイム環境 |
| `secrets` | Part C | keep-en | 原文保持、日本語を付けない | いかなる訳語も付けない |
| `source plan` | Part C | bilingual-first-use | ソースプラン (source plan) | 元計画、原始計画 |
| `source root` | Part C | bilingual-first-use | ソースルート (source root) | 元ルート |
| `state` | Part C | localized | 状態 | ステート単独表記 |

- `state` は日本語表記が定着しているため、ここには「状態」が正解であることを確認する目的で記載している。
- `pipeline` には二つの階層が存在する。単独で出現する場合は GAL の実装パイプラインを指し「パイプライン (pipeline)」と表記する。`config.json#executorRouting.pipeline` として出現する場合は設定キーであるため、原文を保持して翻訳しない。文脈上双方に言及する場合は、どちらを指しているかを事前に明示すること。

### Part D — メンバー登録表

Part D は、GAL が対象とする 5 つの coding agent の登録表である。coding agent のカテゴリ名は日本語で「コーディングエージェント」とし、製品名は常に原文を保持する。

| 英語用語 | 出典 | 表示モード | ja 固定表記 | 避ける表記 |
| --- | --- | --- | --- | --- |
| `generated adapter file` | Part D | localized | 生成されたアダプターファイル | 生成アダプタファイル（長音省略） |
| `runtime key`（5 つのコードと製品名を含む） | Part D | localized + keep-en | 概念自体は「ランタイムキー」。コードと製品名は原文保持：`copilot`、`antigravity`（`agy`）、`codex`、`opencode`、`claude`、および GitHub Copilot、Google Antigravity、OpenAI Codex、opencode、Claude Code | 実行時キー、製品名のいかなる訳語 |
| `VALID_RUNTIMES` | Part D | keep-en | 原文保持、日本語を付けない | いかなる訳語も付けない |
| `vendor`（提供元企業名を含む） | Part D | localized + keep-en | 概念自体は「ベンダー」。提供元企業名は原文保持：GitHub、Microsoft、Google、OpenAI、Anthropic。`open source (SST)` は「オープンソースプロジェクト（SST）」 | 提供元、業者、ベンダ（長音省略） |

「避ける表記」列には、単純な誤字や長音省略のほか、日本語として意味は通じるものの本プロジェクトの統一表記として採用しない語が含まれる。欄が空の場合は、既知の誤用例が存在しないことを示す。

## 一般技術用語：採用表記一覧

以下は GAL 固有の用語ではなく、`docs/glossary.md` の管轄外である一般技術語を対象とする。日本語で表記が分かれやすい用語のみを列挙し、採用表記を固定する。すでに表記が一意に定着している語（アルゴリズム、デバッグ、クラス、パッケージ、リポジトリ など）は掲載していない。

### 開発・プログラミング

| 英語原文 (en-US) | 採用表記 | 避ける表記 |
| --- | --- | --- |
| argument / parameter | 引数 / パラメーター | 引き数、パラメタ |
| array | 配列 | アレイ単独表記 |
| boolean | 真偽値 | ブーリアン単独表記 |
| function | 関数 | ファンクション |
| implement | 実装 | インプリメント |
| nested | 入れ子 | ネスト単独表記 |
| return | 戻り値、返す | リターン単独表記 |
| variable / constant | 変数 / 定数 | — |

### ファイル・システム

| 英語原文 (en-US) | 採用表記 | 避ける表記 |
| --- | --- | --- |
| folder / directory | フォルダー / ディレクトリ | フォルダ（長音省略） |
| memory | メモリー | メモリ（長音省略） |
| server | サーバー | サーバ（長音省略） |
| user | ユーザー | ユーザ（長音省略）、利用者 |

### 工程・品質

| 英語原文 (en-US) | 採用表記 | 避ける表記 |
| --- | --- | --- |
| audit | 監査 | オーディット |
| compatible | 互換 | コンパチブル |
| configuration / settings | 設定 | 構成、コンフィグ単独表記 |
| default | デフォルト | 既定単独表記 |
| receipt | レシート | 受領記録単独表記 |
| review | レビュー | 審査単独表記 |

`user` は「ユーザー」に統一する。本プロジェクトでは IT 業界の一般的な慣例に合わせ、Microsoft および各ベンダーのローカライズ指針に準拠して「ユーザー」を採用する。また、概念の混同を避けるため、次の 3 語の区別を明記する。`gate` は「ゲート」、`slug` は「スラッグ」（`identifier` の「識別子」とは別概念）、`axis` は「軸」（`dimension` の「次元」とは別概念）とする。

## 訳文一貫性の補充語

以下の語は `docs/glossary.md` に未登録でありながら、訳文中で複数の表記が出現したものである。本表は表記のみを固定するものであり、**意味に関する権威は一切持たない**。これらの語が GAL の重要概念へ昇格した場合は、まず `docs/glossary.md` への登録を行い、その後に [GAL 中核用語表示表](#gal-中核用語表示表) へ移行すること。

| 英語用語 | 表示モード | ja 固定表記 | 避ける表記 |
| --- | --- | --- | --- |
| `contract` | localized | 契約 | コントラクト単独表記 |
| `fail-closed` | keep-en + 補足 | 初出時は「fail-closed（既定で遮断）」、以降は `fail-closed` | フェイルクローズ、閉塞失敗 |
| `finalize` / `land` | localized | 着地 | 確定、ランディング |
| `handoff notes` | localized | 引き継ぎメモ | ハンドオフノート |
| `headless` | localized | ヘッドレス | ヘッドなし、無頭 |
| `hunk`（diff） | keep-en | 原文保持、必要時は「hunk ヘッダー」 | ハンク、塊 |
| `loud failure` | localized | 明示的な失敗 | 大きな失敗、轟音失敗 |
| `orchestrated-only` | localized | オーケストレーター経由限定 | オーケストレーションのみ単独表記 |
| `skill` | localized | スキル | — |
| `spawn` | keep-en + 補足 | 初出時は「起動 (spawn)」、以降は「起動」 | スポーン、生成単独表記 |
| `steel-man` | bilingual-first-use | スティールマン論法 (steel-man) | 鉄人論法 |
| `worktree`（git） | localized | ワークツリー | 作業ツリー、作業木（直訳） |

このうち `fail-closed`、`finalize` / `land`、`orchestrated-only`、`loud failure`、`handoff notes` はすでに GAL の拘束的な契約文書に出現しており、性質上他の語よりも中核用語に近い。これらは `docs/glossary.md` への登録候補であり、登録後には中核用語表へ移行する。

## glossary.md との関係

本ドキュメントは表示上の指針のみを提供する。用語の意味の説明、命名規則（Part A）、用語登録表（Part B/C）、メンバー一覧（Part D）、および廃止用語一覧については、常に英語版の [`docs/glossary.md`](../../glossary.md) を正本とし、同ファイルが意味の権威を持つ。

保守の手順は以下の通りである。[GAL 中核用語表示表](#gal-中核用語表示表) は、`docs/glossary.md` の Part B/C/D を完全に網羅すること。`docs/glossary.md` で語彙が追加または削除された場合は、本ドキュメントでも対応する表示行を確認する。中核用語表の行を削除または書き換える前に、必ず `docs/glossary.md` に対応する登録が存在するか確認する。他の 2 つの表は `docs/glossary.md` の管轄外である一般語を対象としているため、独立して項目の増減が可能であるが、GAL 中核概念の意味を定義・改変する目的で使用してはならない。
