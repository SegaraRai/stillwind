# JSとCSSの性能最適化結果

2026年10月3日に、sessionの成果物参照、トランザクション、診断更新、stylesheet計画、CSS読込を最適化した。1000モジュール・各8回の`style()`呼出しを使ったVite測定では、ビルド5秒、watch開始1秒、watch編集100msの目標を満たした。通常の全CSS定義と1万件の追加utility定義でも確認した。

今回の結果は、型checkerを必要としないliteral入力の測定である。ビルドはメモリへの出力とディスクへの出力を別に測り、watch編集はtransformからCSSの送信までを測る。process起動、ファイル監視の通知、ブラウザーでの適用は含まない。実プロジェクト全体の所要時間については、これらの区間を加えて判断する必要がある。

## 目標と結果

値は中央値であり、単位はmsである。ビルドは3 sample、watch開始と編集は5 sampleを使用した。変更前のビルドは1 sampleである。

| 条件                                               | 指標               | 目標 |  変更前 | 最適化後 | 最適化後の最大値 |
| -------------------------------------------------- | ------------------ | ---: | ------: | -------: | ---------------: |
| 1000 JSモジュール、各8呼出し、最小CSS定義          | Viteビルド         | 5000 | 33227.2 |   2973.0 |           3158.9 |
| 同じJS入力、通常の全CSS定義                        | Viteビルド         | 5000 |  未測定 |   3248.9 |           3352.1 |
| 同じJS入力、追加utility定義10000件                 | Viteビルド         | 5000 |  未測定 |   3167.1 |           3310.4 |
| 1000 JSファイル、要求されていない状態、最小CSS定義 | watch開始          | 1000 |    36.7 |     52.8 |            193.7 |
| 同じ状態、通常の全CSS定義                          | watch開始          | 1000 |   176.1 |    221.8 |            254.9 |
| 同じ状態、追加utility定義10000件                   | watch開始          | 1000 |   249.2 |    266.5 |            282.7 |
| 1000 JSモジュール、各8呼出し、最小CSS定義          | watch編集とCSS送信 |  100 |   207.7 |     53.5 |             55.7 |
| 同じJS入力、通常の全CSS定義                        | watch編集とCSS送信 |  100 |  未測定 |     90.7 |             99.0 |
| 同じJS入力、追加utility定義10000件                 | watch編集とCSS送信 |  100 |  未測定 |     58.1 |             68.7 |

watch開始は`createServer()`から`listen()`完了までを測った。JSのimport、dependency discovery、vocabularyファイルの生成は測定外である。既存のJSを先にimportした同じprocessで測っている。最大値も今回採ったsampleの最大値であり、将来の上限を保証する値ではない。通常の全CSS定義を使う編集は100msに近いため、監視通知とブラウザーの処理を含める場合の余裕は小さい。

ディスクへの出力を含むビルドも、同じ1000モジュール・各8呼出しで3回ずつ測定した。全条件で5秒未満だった。上表の変更前との比較はメモリへの出力同士であり、以下は追加の目標確認である。

| CSSの条件              | ディスクへの出力を含むビルドの中央値 |   最大値 |
| ---------------------- | -----------------------------------: | -------: |
| 最小CSS定義            |                             3033.5ms | 3107.8ms |
| 通常の全CSS定義        |                             3354.5ms | 3613.1ms |
| 追加utility定義10000件 |                             3375.8ms | 3416.1ms |

## CSSが増える場合

| 条件                                              | 指標                         |           変更前 | 最適化後 |
| ------------------------------------------------- | ---------------------------- | ---------------: | -------: |
| rootから1000 CSSファイルをimport、JSは1モジュール | watch開始                    |          513.5ms |  292.5ms |
| 同じCSS入力                                       | JS編集とCSS送信              |          141.8ms |   35.9ms |
| メモリ上のCSS importが1000件に分岐                | hostによる読込とsnapshot構築 |          235.3ms |   11.9ms |
| 100 JSモジュール、追加utility定義1000件           | native編集                   |           15.0ms |   0.32ms |
| 100 JSモジュール、追加utility定義10000件          | native編集                   | 同条件では未測定 |   0.33ms |

CSSファイル数のケースは実ファイルの読込を含む。メモリ上のimportのケースは、同じevent loop内で完了した応答をまとめる効果を分離する。実ファイルでは読込の完了が複数回に分かれるため、短縮率は異なる。

CSS定義はsnapshotごとに共有されるようになった。未使用の定義を増やしても、全定義の文字列と逆引きmembershipをモジュールごと、トランザクションごとに複製する必要がない。snapshotを最初に解析する処理は引き続き必要であり、その時間は開始時の測定に含まれる。

## 実装した変更

- `ProjectSession`は、受け付けた解析のartifact keyを更新時に確定する。成果物の参照とstylesheet計画は、そのkeyを比較し、変更していないIRを再ハッシュ化しない。ソース本文が同じでもIRが変われば再コンパイルする。
- 解析、成果物、stylesheetの寄与は`Arc`で共有する。lint更新は対象成果物を変更する前に分離するため、別のsession cloneが保持する成果物は変わらない。
- utility、variant、propertyの定義名集合はsnapshotに一つだけ保持する。公開された逆引き索引は要求時に現在の成果物から構築する。reloadは引き続き全解析を再コンパイルするため、通常のhost処理は索引の構築を必要としない。
- nativeのbatchは各入力を一度だけfinalizeし、閉じたcomposition graphを一度だけ検証する。各入力の診断は、その入力のソース位置を保持する。
- styleのlint検査は、更新するモジュールと必要な合成先を解析する。診断の再構築は、CSS、クラス、資源をtransport結果に変換せずに行う。ソース変換の検証は残している。
- stylesheetのfragmentに対するownerとslotの割当は、重なる範囲を索引から取得する。元のmapping順序、clip、CSSの順序を維持する。
- CSSの内容ハッシュは64ファイルずつ並列に読む。hashへの追加順、内容の検証、ファイル消失時の扱いを維持する。
- CSS loaderは読み終わった応答をevent loopごとにまとめて渡す。遅いsiblingを待たずに、読み終わったCSSの子importを開始する。

変更の中心は、不変な成果物を共有し、必要な解析範囲を狭めることである。processをまたぐ永続cacheや、JSの本文だけをkeyにした結果cacheは追加していない。この測定時点では、CSSの鮮度を要求時に内容から確認していた。その後、通常のwatchを監視イベントによる世代管理に変更した。現在の契約と追加測定は[watchの最適化結果](/F:/Projects/shorth/docs/performance-watch-2026-10-03.md)を参照する。上表の値と元の証拠ファイルは、この後続変更を含まない。

## 残る処理

元のcompiler benchmarkでも、1000モジュール・各8呼出しのnative初期投入は19.60秒から3.43秒、1モジュール編集は113.5msから4.27ms、documentのstylesheet計画は149.1msから56.6msへ短縮した。同一入力1000モジュールの再投入は37.25秒から3.54秒になったが、初期投入に近い処理が残っている。nativeでは再登録するソースに新しいfile IDを割り当てるため、同じ本文でも既存artifactへのcache hitにはならない。

watchの初回読込には別の課題がある。1000モジュール・各8呼出しを順にtransformし、各回CSSを公開するprobeは、最小CSSで26.5秒、通常の全CSS定義で59.1秒だった。最小CSSの変更前は89.0秒である。これは監視開始の時間には含めない。初回ページの読込を速くするには、要求が集中する間のCSS計画と送信をまとめる設計を検討する必要がある。

frameworkの前処理、型checkerを必要とする依存グラフ、実際のHMR経路、ブラウザーの適用時間は、この結果から目標達成を判定できない。特に`hotUpdate`と後続のtransformが同じ改訂を処理する経路は、ソース、facts、依存先、CSSの改訂を含む再利用条件を確認する必要がある。

## 証拠と検証

測定はNode 24.21.0、Vitest 5.0.1、Windows x64、Ryzen 7 9800X3D、release addonを使い、他のbuildや検証と重ねずに行った。測定用fixtureの作成とassertionはcallbackの外に置いた。ソースコードはmigration commit `6f6c22b5`から変更した未コミットの状態であり、[optimization-evidence.json](/F:/Projects/shorth/benchmarks/research/optimization-evidence.json)は変更したproductionファイルのSHA-256、raw sample、測定条件を保持する。

変更前の測定は[architecture-evidence.json](/F:/Projects/shorth/benchmarks/research/architecture-evidence.json)と[evidence.json](/F:/Projects/shorth/benchmarks/research/evidence.json)に保存した。[初期調査](/F:/Projects/shorth/docs/performance-architecture-2026-10-03.md)は、最適化前の構造と改善候補を記録している。

architectureの測定は37 testと72計測ケース、ディスク出力の追加測定は3 testと3計測ケース、`vp run bench:compiler`は27 testと27計測ケースに成功した。Viteビルドは最適化後に各CSS条件を3回ずつ測った。再現方法と計測範囲は[benchmark README](/F:/Projects/shorth/benchmarks/README.md)を参照する。

`vp run verify`と`vp run test:browser`に成功した。通常の全体検証にはRustの864 test、JSの286 test、型検査、lint、format、fixtures checkが含まれる。追加した回帰テストは、session cloneの独立性、同じ本文で変更したIRの無効化、unorderedな範囲の重なりと順序を検証する。既存テストは、composition cycle、consumer診断、失敗batch、CSSの再読込と鮮度、importの並行読込を確認する。
