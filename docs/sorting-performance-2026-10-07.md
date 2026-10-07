# ソートの性能調査

有効なcandidateの1回の比較は、今回の入力ではnativeで約0.004〜0.021ms、Node WASMで約0.007〜0.036msだった。ソート全体では、逆順100要素のutility配列が約30〜61ms、100個のbase style配列が約2.5〜3.4秒かかる。base styleでは、候補を探すAST走査とasset検索が時間の大部分を占める。

canonical判定のキャッシュは維持している。この調査でproductionのソート処理は変更していない。再実行できるbenchmarkと、処理の内訳を調べるbenchmarkを追加した。

## 測定条件

Windows x64、AMD Ryzen 7 9800X3D、Node 24.21.0、Vitest 5.0.1を使用した。nativeとWASMはrelease buildを使用し、各taskを一つずつ実行した。WASMはNodeのWASI bindingであり、ブラウザのworkerやLSP通信は含まない。

通常の測定はwarmup 2回とsample 9回を使用した。1000-token文字列と内訳benchmarkはwarmup 1回とsample 3回を使用した。以下の時間は中央値、単位はmsである。実行間の差もあるため、細かな比率は固定した性能保証を表さない。

candidate比較は256回を一つのcallbackで実行し、batchの中央値を256で割った。sourceの測定は `EditorSnapshot.query(sort)` がeditsを返すまでを含む。CSSの読み込み、compilerの解析、`SourceIndex`の構築、LSP通信、editsの適用は含まない。freshでは各sampleの前にsnapshotを構築するが、その構築時間はtimerの外に置いている。

utilityのみのfixtureはgenerationがcompleteである。refとbase styleのfixtureはtype factsを無効にしており、generationはunverifiedな使用のためblockedになる。editorはnativeの構文上のasset identityとTypeScript binderを使って、それらの配列をソートできる。この測定は完全な型解析やcompilerのgenerationの費用を表さない。

## candidate比較

| 比較内容                        | native authored | native tailwind | Node WASM authored | Node WASM tailwind |
| ------------------------------- | --------------: | --------------: | -----------------: | -----------------: |
| `p-4` / `p-8`                   |          0.0038 |          0.0048 |             0.0070 |             0.0086 |
| theme color                     |          0.0093 |          0.0107 |             0.0157 |             0.0179 |
| colorのopacity modifier         |          0.0181 |          0.0205 |             0.0285 |             0.0356 |
| `md:hover:p-4` / `md:focus:p-8` |          0.0095 |          0.0128 |             0.0147 |             0.0165 |
| `hover:p-4` / 同じcandidate     |          0.0075 |          0.0105 |             0.0112 |             0.0132 |
| 未解決candidate                 |          0.0007 |          0.0007 |             0.0026 |             0.0027 |

`DesignSystemQuery.compare_candidates()`は、比較ごとに左右のutilityを解決し、variantの有効性と順序を調べる。同じ文字列同士の比較でも、この処理を実行する。candidateごとの順序キーや比較結果を保持するキャッシュはない。共有するテーマの索引と解析済みutility定義は再利用する。

authoredの順序は定義の位置を使う。tailwindの順序では、解決したdeclarationからpropertyの位置と数を求める。variantの順序では定義の検索、groupの位置、テーマ値の比較も実行する。比較のために、CSS生成にも使う完全なutility解決を繰り返している。

## ソート全体

次はpreviewキャッシュが空のsnapshotに対する測定である。utilityの入力は `p`、`m`、`w`、`h`、`gap` の異なる数値を含む。比較回数には、文字列で未解決candidateを判定するための自己比較も含む。

| 入力                   | candidate比較回数 | native authored | native tailwind | Node WASM authored | Node WASM tailwind |
| ---------------------- | ----------------: | --------------: | --------------: | -----------------: | -----------------: |
| 文字列30個、shuffle    |               138 |           0.808 |           0.925 |              1.396 |              1.546 |
| 配列30個、逆順         |               435 |           3.105 |           3.624 |              5.286 |              7.804 |
| 文字列100個、整列済み  |               199 |           1.250 |           1.845 |              2.273 |              2.328 |
| 文字列100個、逆順      |               199 |           1.162 |           1.337 |              1.937 |              2.732 |
| 文字列100個、shuffle   |               634 |           3.712 |           5.696 |              6.480 |              9.376 |
| 配列100個、整列済み    |                99 |           2.114 |           3.061 |              3.250 |              4.131 |
| 配列100個、逆順        |              4950 |          30.037 |          37.081 |             51.877 |             61.114 |
| 配列100個、shuffle     |              2309 |          14.442 |          18.809 |             28.035 |             36.229 |
| 文字列1000個、整列済み |              1999 |          20.617 |          22.062 |             29.488 |             34.879 |

文字列は、未解決candidateを境界として区間ごとにJSの `Array.sort()` を使う。逆順の入力でも、今回のruntimeでは整列済みの入力と同じ比較回数だった。

配列は挿入ソートを使う。逆順では `n(n-1)/2` 回比較する。100要素では4950回のnative呼び出しが発生し、そのたびにcandidateを再解決する。

整列済みの文字列では、変更の有無を調べる `every()` の各反復で文字列全体をregexで分割し直している。この確認はtoken数の二乗に比例する処理を含む。配列では、各文字列要素の再parseと、比較ごとのtoken list分割も繰り返す。

## refとbase style

次はauthoredの順序を使用し、識別子を逆順で並べた配列である。独立したbase styleは、Webrefの公開propertyから、productionの安全性判定に通るpropertyを選ぶ。それぞれ `[property:initial]` を使用する。干渉するbase styleは異なる `p-N` を使用し、順序を保持する。

| 入力               | CSS安全性判定回数 |   native | Node WASM |
| ------------------ | ----------------: | -------: | --------: |
| ref 10個           |                 0 |    0.334 |     0.311 |
| ref 30個           |                 0 |    1.535 |     1.752 |
| ref 100個          |                 0 |   45.062 |    38.722 |
| 独立したbase 10個  |                45 |    1.955 |     2.308 |
| 独立したbase 30個  |               435 |   54.535 |    53.601 |
| 独立したbase 100個 |              4950 | 3284.215 |  3392.780 |
| 干渉するbase 10個  |                 9 |    1.522 |     2.095 |
| 干渉するbase 30個  |                29 |   39.214 |    31.541 |
| 干渉するbase 100個 |                99 | 2451.569 |  2471.361 |

干渉するbaseは比較が99回でも約2.5秒かかる。別のnative計測で、同じ100個のbaseに対する1回の `sortSource()` の内訳を確認した。

| 処理                           |  回数 |                           計測時間 |
| ------------------------------ | ----: | ---------------------------------: |
| `SourceIndex.asset()`          | 20400 | `baseCandidates()`の時間に含まれる |
| `SourceIndex.baseCandidates()` |   200 |                           2463.983 |
| `canReorderStyles()`           |    99 |                              0.400 |
| `sortSource()`全体             |     1 |                           2503.153 |

この内訳は、通常のbenchmarkと別の1回を計測した結果である。関数に計測用wrapperを付けており、wrapperの費用も含む。通常のsampleを取る前に元の関数へ戻している。`baseCandidates()`の時間は、その内部で行うasset検索を含むため、assetの時間を別に足してはならない。

`pureBase()`とCSSを求める処理が、それぞれ `baseCandidates()` を呼ぶ。`baseCandidates()`は呼び出しごとにAST全体を走査し、各call expressionをassetと照合する。`asset()`はasset一覧を線形に検索し、照合のたびにsource全体をUTF-8へencodeして開始位置をUTF-16へ戻す。開始位置が一致した場合は終了位置も変換する。baseの数とsourceの長さが増えると、これらの処理が重なる。今回の内訳では `baseCandidates()` が全体の約98%を占めた。

refではAST全体を走査するbase候補検索を行わない。100個のrefに対する別の内訳計測では、asset検索が200回発生し、全体が約32.8msだった。refもasset検索と位置変換の索引を使うことで改善できる。

CSS安全性判定は呼び出しごとに両方のCSSをparseし、Webrefのpropertyを検索してshorthandとlogical propertyの干渉を調べる。判定が多い入力ではこの再parseも改善対象になる。custom property、nested rule、理解できないdeclarationは順序を保持するため、同じrankという理由だけでbaseを一括ソートしてはならない。

## 既存キャッシュの効果

candidate previewはsnapshot内で再利用する。ソートの比較、base候補検索、CSS安全性判定、明示的なsort要求の最終editsは再計算する。診断の `reports()` はmoduleごとの結果を保持するため、同じsnapshotから診断を読み直す経路ではソートを繰り返さない。

次はauthoredの順序を使用する100要素の入力である。repeatedは、sourceにeditsを適用せず、初回要求と同じsnapshot・同じsourceへ再度sortを要求する。キャッシュをすべて無効にした比較は行っていない。

| 入力                  | native fresh | native repeated | Node WASM fresh | Node WASM repeated |
| --------------------- | -----------: | --------------: | --------------: | -----------------: |
| 文字列、shuffle       |        3.712 |           3.500 |           6.480 |              6.214 |
| utility配列、整列済み |        2.114 |           0.993 |           3.250 |              1.263 |
| utility配列、逆順     |       30.037 |          28.868 |          51.877 |             58.569 |
| ref配列               |       45.062 |          37.241 |          38.722 |             37.652 |
| 干渉するbase配列      |     2451.569 |        2474.701 |        2471.361 |           2500.438 |
| 独立したbase配列      |     3284.215 |        3348.707 |        3392.780 |           3399.602 |

utility配列の初回preview生成はキャッシュで削減できる。比較やbaseのAST検索が大きな費用を占める入力では、再要求でも時間がほとんど減らない。文字列のソートはpreviewを使用しない。

## 改善する順序

1. `SourceIndex`の構築時にassetのUTF-16位置とAST nodeとの対応を索引化する。baseのcall expression、候補列、純粋なbaseかどうかもasset単位で保持する。`pureBase()`とCSSの取得が同じ候補列を再利用すれば、各baseについてAST全体を2回走査する処理を取り除ける。これは今回の最大の費用を扱う変更になる。
2. immutableなdesign-system snapshotに、variantのgroup位置などの順序metadataを準備する。実際に現れたcandidateは一度解決して順序キーを保持し、その後の比較でキーを使う。任意値を含む全candidateを事前列挙する必要はない。tailwindのproperty順、themeで決まるvariant値、無効なcandidate、解決エラーを保持する契約を先に定める。
3. utility区間を一括でRustへ渡し、candidateを一度ずつ解決してソートするAPIを検討する。JS/Rust間を比較ごとに往復する費用を減らせる。ref、base、コメント、未解決要素の境界はhostが保持する。
4. 配列の安全性を満たす区間で、比較回数を減らすalgorithmを使う。base同士の交換は部分的に許されるため、通常の `Array.sort()` への置換だけでは現在の契約を保てない。utilityだけの区間を先に扱い、干渉するbaseについては元の前後関係を制約として保持する。全pairの安全性確認が必要な場合、その確認自体には二乗の費用が残る。
5. 文字列のtoken化と配列内のtoken list分割を一度だけ行う。baseのCSSも一度parseしてpropertyの影響を保持し、安全性判定ではその結果を比較する。これらはcandidateの順序や交換の制約を変えずに重複処理を削減できる。

pairごとの比較結果だけをcacheしても、逆順100要素の初回では4950pairが異なるため、同じcandidateの解決を繰り返す費用が残る。candidateごとのキーとassetごとの索引を準備する方が、初回の要求にも効果がある。

## 再実行と根拠

- `vp run bench:sorting`: native、115件。
- `vp run bench:sorting:wasm`: Node WASM、同じ115件。
- `vp run bench:sorting:profile`: nativeでrefと干渉するbaseの内訳、6件。

すべての測定で、対象binding、結果の変更有無、繰り返し要求のeditsを検証した。比較回数を数える処理はtimerの外にあり、実際のsnapshotが返すeditsと一致することを確認した。通常のreportはignoredな `benchmarks/results/` に出力する。生のsample、実行環境、production sourceのSHA-256、測定時のbenchmark sourceのSHA-256は [sorting-evidence.json](../benchmarks/compiler/sorting-evidence.json) に保存している。測定後にbenchmarkの条件文へ波括弧と説明コメントを追加したが、測定する処理は変えていない。

関連する実装は [hostのソート](../packages/host/src/editor/sort.ts)、[sourceの索引](../packages/host/src/editor/source-index.ts)、[candidate query](../packages/host/src/editor/candidates.ts)、[snapshot](../packages/host/src/editor/snapshot.ts)、[compilerの比較](../crates/compiler/src/query/ordering.rs)、[utility解決](../crates/design-system/src/evaluation/utilities/resolve.rs)、[variantの順序](../crates/design-system/src/evaluation/variants/order.rs)、[CSSの安全性判定](../crates/napi/src/editor.rs)、[propertyの干渉](../crates/css/src/values/webref.rs) にある。

`vp run check` と `vp run test` は通過した。静的検証、新しいbenchmarkの型検査、Rust workspace、JS 920件、project fixtureを確認した。Rustとfixtureの検証は、sourceに変更がないためtask cacheの結果を使用した。
