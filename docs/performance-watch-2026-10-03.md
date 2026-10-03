# WatchのCSS鮮度確認の最適化

2026年10月3日に、Viteの通常のwatchを、監視イベントによるCSSの世代管理へ変更した。CSSが変わっていないJS編集とstylesheet要求は、現在のsnapshotを再利用し、CSSファイルを読まない。CSS本文のcacheや部分的なCSS再構築は追加していない。

1000モジュール・各8呼出しでは、ディスク出力を含むビルドは約3.0〜3.25秒、watch開始は約8〜108ms、監視ありのtransformからCSS送信までは約43〜64msだった。実ファイルへの書込からCSS送信までは中央値116msであり、この範囲では100msの目標をまだ満たしていない。ブラウザーの後続要求とCSS適用も、この値には含まない。

## 目標と最終測定

単位はmsであり、値は中央値である。括弧内は採ったsampleの最大値であり、将来の上限を保証しない。ビルドは3 sample、開始と編集は5 sampleを使った。watch開始は`watchReadyMs`を使う。

| CSS条件                | ディスク出力を含むビルド、目標5000 | watch開始、目標1000 | 監視ありのJS transformとCSS送信、目標100 |
| ---------------------- | ---------------------------------: | ------------------: | ---------------------------------------: |
| 最小定義               |                    2994.2 (3150.2) |           7.6 (8.6) |                              42.9 (59.3) |
| 通常の全定義           |                    3247.5 (3276.6) |         50.5 (78.8) |                              63.9 (64.6) |
| 追加utility定義10000件 |                    3209.0 (3230.6) |       108.2 (138.1) |                              55.7 (63.7) |

`watch: null`のJS transformとCSS送信は、同じ順で40.6ms、73.2ms、48.9msだった。sample数が少なく、CSSファイル1個の条件では、内容ハッシュを省く効果をこの差だけから断定できない。通常のwatchでCSSを読まない契約は、読込回数を確認するテストが直接検証する。

CSSファイルが増える場合は、監視登録も含む開始時間と、未変更のCSSを読む処理の削減が重要になる。

| CSSのimport数 | 開始、目標1000 | JS transformとCSS送信、目標100 |
| ------------- | -------------: | -----------------------------: |
| 100           |    35.4 (45.3) |                    0.81 (0.85) |
| 1000          |  299.5 (386.7) |                    4.37 (4.80) |

前段の内容ハッシュを使う実装では、1000 CSSファイルのJS編集は中央値35.9msだった。監視イベントを使う最終実装では4.37msになった。追加の中間測定では、importごとに候補・解決先・実ファイルを重複して監視登録しており、開始が約545msだった。一つの読込内で同じpathの登録を省いた後は約300msになった。これらは異なる測定runの比較であり、単一要因だけを切り替えた実験ではない。

## 実ファイル編集で残る差

通常の全定義を使う実ファイル編集は、最終測定で中央値116.3ms、最大128.7msだった。元ソース情報を省く前の中間測定は中央値121.1msだった。変更後も100msには届いていない。

区間を追加した別runでは、watcher通知までの中央値は10.3ms、clientのhot-update hook開始までは31.6ms、CSS送信までは135.4msだった。このrunの総時間は111.6〜152.4msだった。監視通知だけでは超過を説明できず、hook開始後のcompile、全producerのstylesheet計画、CSS送信にも時間が残っている。区間の中央値同士の差は、特定sampleの所要時間ではない。

次の改善候補は、編集ごとに全producerのCSSを計画する処理である。現在の実装は全producerの順序と所有者を毎回確認し、全fragmentを組み立てて送る。変更していないfragmentを使う設計では、CSS内容だけでなく、選択したproducer、composition順序、snapshot、名前の予約、所有者の改訂を再利用条件に含める必要がある。この追加設計は今回の実装には含めていない。

1000モジュールの初回要求を順に処理して毎回CSSを送るpreloadは、通常の全定義で約35.7秒だった。要求受付の目標とは別に、初回ページ読込を改善する余地も残っている。CSS定義そのものの編集は、全CSS読込とsession reloadを行う。単一JS編集の100ms目標を満たした値から、この全体再構築の時間を推定することはできない。

## 鮮度と失敗時の契約

clientとSSRはそれぞれ、CSS依存先、監視対象、変更世代、適用した世代、最後に試した世代を持つ。watcherが対象ファイルの`add`、`change`、`unlink`を通知すると、そのenvironmentの変更世代が進む。ブラウザーのHMR接続は世代更新の条件ではない。

通常のwatchは、観測した監視イベントに対して鮮度を保証する。ディスクが変わっていても、watcherがまだ通知していない時点では、要求は現在のsnapshotを返せる。`watch: null`の場合は、従来どおり要求時に全CSS依存先の内容ハッシュを調べる。このfallbackは、監視通知を前提にしない。

未処理の変更があると、loaderはCSS全体を再読込する。読込中に変更世代が進んだ場合は、読込をやり直す。新しく解決するimportは、読込を始める前に監視する。解決に失敗した相対importも監視対象に加えるため、ファイルの作成や復元を観測できる。成功すると、snapshot、資源、依存先、監視対象、適用世代を一緒に公開する。成功した新しいCSS graphが参照しないimportは、そのenvironmentの世代を進めなくなる。

読込またはreloadに失敗すると、前回成功したCSSと失敗情報を保持する。通常のwatchは、同じ世代に対して再試行しない。次の対象イベントが届くと再試行できる。欠けていたimportの作成もhot-updateを開始するため、stylesheetの再要求を待たずに復旧したCSSを送信できる。

初期化、CSS更新、JS compile、stylesheet計画、CSS送信は、environmentごとの既存の作業queueで順序を揃える。並行要求は進行中の更新を待ち、その結果を使う。更新が失敗してもqueueは解放される。serverを閉じると、CSS世代を更新するlistenerを解除する。

source mapを使わないVite設定では、CSS計画の元ソース本文とmappingをJSへ渡さない。nativeの`planStylesheet(selection, false)`は、planと各partのmapを空にする。CSS、順序、所有者、資源、producer改訂は保持する。引数を省略したnative呼出しと、source mapを使うVite設定は、従来どおりmapを取得する。Rust内部では、CSSの所有者と分割位置を決めるためのsource位置を引き続き保持する。

## 計測範囲

比較するJS入力は1000モジュールであり、各モジュールは`style()`を8回呼ぶ。CSS条件は最小定義、通常の全定義、追加utility定義10000件である。型checker、development source map、dependency discoveryは無効にした。測定はrelease addonを使い、他のbuildやtestと重ねずに行った。

`vite/watch-edit`はtransformからCSS送信までを測る。通常のwatchと`watch: null`を別々に実行する。`vite/watch-event`は実ファイルへの書込から、実際のwatcherとViteのhot-update hookを通った`stillwind:css`送信までを測る。ブラウザーは接続しない。後続のJS要求とブラウザーのCSS適用は含まない。

watch開始は、1000 JSファイルを置いた状態で、`createServer()`から`listen()`完了とwatcherの最初の`ready`イベントの両方までを測る。JSは要求せず、必要時にコンパイルする。`listenMs`は要求受付まで、`watchReadyMs`は要求受付と監視の初期走査の完了までを記録する。CSSファイル数のケースは、rootから100ファイルまたは1000ファイルをimportし、JSは1モジュールにする。

## 証拠と検証

[watch-evidence.json](/F:/Projects/shorth/benchmarks/research/watch-evidence.json)は、変更したproductionファイルのSHA-256、各ケースの条件、raw sampleを保持する。[benchmark README](/F:/Projects/shorth/benchmarks/README.md)は再現コマンドを記載している。前段のnativeとビルドの最適化結果は[性能最適化結果](/F:/Projects/shorth/docs/performance-optimization-2026-10-03.md)に残している。

最終実装の`vp run verify`はRust 864 test、JS 290 test、型検査、lint、format、fixtures checkに成功した。追加したnativeの契約検証は、mapを省いたplanが元のplanと同じCSS、順序、所有者、改訂を返すことを確認する。既存の`watch: null`テストは内容ハッシュによる鮮度確認を検証する。通常のwatchの4 testは、未変更時のCSS読込0回、clientとSSRの独立性、HMR未接続の更新、失敗後の再試行抑制、importの追加と除外、削除と復元、読込中の変更、並行要求、要求なしでの復旧送信、server終了時のlistener解除を検証する。

最終測定は15 testと17計測ケースに成功した。区間を記録する追加の実ファイル編集測定も1 testに成功した。Node 24.21.0、Vitest 5.0.1、Windows x64、Ryzen 7 9800X3D、release addonを使った。processの起動とJS importは測定外であり、同じprocess内で必要なJSを読み込んでから測っている。source mapを有効にした場合の時間と、ブラウザーでの適用時間は未測定である。
