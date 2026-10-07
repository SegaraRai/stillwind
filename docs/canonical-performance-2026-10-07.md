# Canonical utility判定の性能

2026年10月7日に、canonical utilityの探索を、テーマ値の索引と生成CSSの索引を組み合わせる構成に変更した。通常の判定は、候補のutilityをすべて生成して比較する処理から、入力に一致しうる名前を取り出して検証する処理になった。

## 索引の責務と寿命

`ThemeIndex` は構築後に不変であり、元の宣言、有効なtoken、utility名の検索索引、解析済みのbodyとplaceholderを所有する。cloneは元データと解析準備を共有する。評価APIと `DesignSystemSnapshot` は同じ `ThemeIndex` を使用する。CSSの解析と元ファイルへのspanの対応付けは、内部のbuilderで完了してから公開する。

`BuiltinDefinitions` も不変であり、元の定義と検索索引を一つの所有者にまとめる。snapshotの名前別レジストリは、組み込み定義とプロジェクト定義を合わせた名前順の列挙、overloadの順序、フィンガープリントを扱うために残す。元の定義本体はArcで共有する。candidateの評価は各登録層の検索索引を使い、登録順を保つ。

`CanonicalIndex` はsnapshotのanalysisとして一度だけ準備する。snapshotのclone、editor query、module解析は同じ索引を使う。N-APIのloaderはCSS読み込み完了時に `prepare_canonical_index` を呼ぶため、最初のcanonical判定に準備時間が加わらない。CSSを変更すると新しいsnapshotが新しい索引を持つ。古いqueryは古いsnapshotで回答を続ける。

内部の `QueryContext` はsnapshotと実効設定を使ってCSSを評価する。事前準備APIとqueryのconstructorは、この同じコンテキストから共有索引を取得する。事前準備を省略したRustの直接利用でも、最初のqueryが索引を準備する。解析中に見つかったutilityのrequirementエラーは、対応するcandidateを評価した時点で返す。

索引のkeyはpx→remの有効性、root font size、生成CSSのprefixを含む。lintの重大度、class namespace、identifier generatorのrevisionやhash lengthはqueryの比較用CSSを変えないため、keyに含めない。

## 探索の経路

- テーマ値をそのまま宣言へコピーするutilityは、名前空間ごとの値索引を共有する。同じパレットをbackground、text、borderなどのutilityごとに再生成しない。
- 複雑なutilityは、生成CSSのtemplateを使う。各candidateのutility IRがtemplateと一致することを確認し、上書きや異なるbranchは有限のCSS索引で扱う。
- 数値spacingは無限の値を持つため、索引へ列挙せず、値とspacing scaleから候補を計算する。
- variantは、テーマに依存しない宣言を包んだCSSを索引化する。variant自身が宣言を持つ場合、複数のvariantがある場合、utilityにselectorやnested ruleがある場合は追加の候補を検証する。

いずれの経路でも、置換候補の完全な生成CSSと必要な@property登録を元のcandidateと比較する。値索引は余分な候補を返してよいが、一致しない候補をfixとして返してはいけない。

`[` がない入力は、現在のcanonical変換の対象となる任意値・任意property・任意variantを含まないため、探索を省略する。任意modifierだけを持つutilityも変換対象から外れる。modifierが値の差を消す場合や複雑な独自定義では、候補を追加して検証するため、すべてのCSS programに一定時間の上限があるわけではない。

## 測定結果

単位はms。nativeとNode WASMは1回のwarmup後に9 sampleを測った。Chrome WASMは20回のwarmup後に100 sampleを測った。各判定の前に新しいqueryを作るため、candidate結果とpreviewはキャッシュされていない。queryの構築と索引の準備は判定時間に含めない。

| 判定                      | native中央値 | Node WASM中央値 | Chrome WASM中央値 | Chrome WASM p95 |
| ------------------------- | -----------: | --------------: | ----------------: | --------------: |
| spacing `p-[16px]`        |        0.045 |           0.073 |             0.090 |           0.110 |
| 色の一致 `bg-[#f00]`      |        0.050 |           0.099 |             0.105 |           0.240 |
| 色の不一致 `bg-[#123456]` |        0.037 |           0.058 |             0.055 |           0.170 |
| 任意propertyの不一致      |        0.034 |           0.061 |             0.080 |           0.140 |
| 色とopacityの一致 `/50`   |        0.110 |           0.203 |             0.280 |           0.455 |
| 色とopacityの不一致 `/50` |        0.138 |           0.244 |             0.270 |           0.450 |
| 任意variantの不一致       |        0.056 |           0.111 |             0.145 |           0.305 |

代表判定の中央値と最大値、およびChromeのp95は1ms未満だった。modifier付きの判定などは、0.1msの目標を上回った。編集完了前の測定では、Chromeの任意propertyの不一致で1 sampleが1.125msだった。この測定も生のsampleを保存している。

| CSS revisionの読み込みと準備 | 中央値 | 最大値 |
| ---------------------------- | -----: | -----: |
| native                       |  254.2 |  282.1 |
| Node WASM                    |  345.9 |  365.3 |
| Chrome WASM                  |  294.2 |  355.1 |

準備時間はCSS解析を含む。Node WASMの中央値は300msの目安を上回った。Chromeの準備は中央値が300ms未満だったが、最大値は300msを超えた。JS/WASMの起動、workerの作成、LSP transport、editorの描画は測定に含まない。同じrevisionから新しいqueryを作る時間の中央値は、nativeで0.0029ms、Node WASMで0.0190msだった。

| 異なる値を持つパレットの色数 | nativeの不一致判定 | Node WASMの不一致判定 |
| ---------------------------- | -----------------: | --------------------: |
| 10                           |              0.017 |                 0.035 |
| 100                          |              0.025 |                 0.034 |
| 1000                         |              0.020 |                 0.055 |

パレットの測定も9 sampleの中央値である。判定時間は名前の数に比例して増えていない。100個の色を一つのmoduleで診断する時間は、nativeで同じ色を繰り返す場合に約1.15ms、すべて異なる場合に約6.71msだった。これらはsource解析を含み、LSP transportを含まない。

## 不変な所有者へ整理した前後の比較

[保存した基準値](../benchmarks/compiler/canonical-baseline-2026-10-07.json)は、テーマ値とCSSの索引を導入した後、不変なThemeIndexと共通QueryContextへ整理する前の測定である。以下の変化率は中央値の比較を示す。

| 測定                  |     nativeの前→後 | 変化率 |  Node WASMの前→後 | 変化率 |
| --------------------- | ----------------: | -----: | ----------------: | -----: |
| 読み込みと準備        | 201.300 → 254.241 | +26.3% | 280.574 → 345.900 | +23.3% |
| spacing               |     0.044 → 0.045 |  +2.5% |     0.085 → 0.073 | -14.3% |
| 色とopacityの一致     |     0.109 → 0.110 |  +0.4% |     0.197 → 0.203 |  +3.3% |
| 100個の異なる色の診断 |     4.563 → 6.708 | +47.0% |     6.648 → 9.188 | +38.2% |

準備はnativeで約26%、Node WASMで約23%増えた。100個の異なる色の診断では、20〜30%を超える増加を測定した。各測定は9 sampleによる比較であり、実行間のばらつきもある。整理による差と実行環境による差を、この測定だけで分離できるとは限らない。

先の有効な測定では、準備の中央値がnativeで258.3ms、Node WASMで391.1msだった。編集完了前の再測定では、nativeで230.5ms、Node WASMで333.1msだった。これらの測定と、編集完了前のRust sourceのSHA-256も `earlierRuns` に保持している。別の性能プロセスと重なった追加測定は比較から除外した。

## 再実行と根拠

- `vp run bench:compiler` はnativeのcanonical判定とcompilerのgraphを測る。
- `vp run bench:compiler:wasm` は同じcanonical判定をNode WASMで測る。
- `vp run bench:compiler:browser` はChrome WASMの判定p95 <1msと、準備の中央値 <300msを検証する。
- `vp run test:performance:full` はnativeの判定目標0.1ms、上限1ms、準備の上限300msを含む32件の性能検証を行う。0.1msはsoft targetであり、超過を記録する。

測定環境はWindows x64、Node 24.21.0、Chrome 154、Vitest 5.0.1である。nativeとWASMはrelease buildを使用した。時間を測るtaskは、他のbuildやtestが終了してから一つずつ実行した。

最新測定と先の有効な測定の生のsample、実行環境、base revision、変更したRust sourceのSHA-256は [canonical-evidence.json](../benchmarks/compiler/canonical-evidence.json) に保持している。パレットの測定は、最大1000個の異なる色を使用する。通常のJSON reportは `benchmarks/results/` に出力し、Gitでは追跡しない。

関連する実装は [canonical](../crates/compiler/src/query/canonical.rs)、[値索引](../crates/compiler/src/query/canonical/values.rs)、[共有する値索引](../crates/compiler/src/query/canonical/values/direct.rs)、[不変なテーマ](../crates/design-system/src/theme/index.rs)、[utilityの解析準備](../crates/design-system/src/evaluation/programs.rs)、[snapshotのanalysis](../crates/design-system/src/snapshot/analysis.rs) にある。

## 動作検証

`vp run check` と `vp run test` は通過した。Rust workspace全体、compilerの180件の単体テスト、JS全体920件、native/WASMの共通契約、WASM39件、実ブラウザ15件、プロジェクトfixture、性能検証32件を確認した。

Playgroundの静的サイト検証は再実行で通過した。初回はビルド・診断・HMRの操作が通った後、VS Codeのtokenization stateを検査する箇所から `Cannot set null/undefined state` を1件拾った。検証からこの例外を除外する変更はしていない。
