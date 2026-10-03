# Compilerとhostの性能調査

2026年10月3日に、Vitest benchmarkの結果と実装を調査した。目的は、全量コンパイル、同じ入力の再投入、1ファイル編集、CSS planning、loaderとcheckerの初期化、source-map候補の改善余地を判断することである。最初に依頼されたVitest Browser Modeの移行候補も確認した。

benchmark移行は`6f6c22b53b078bb0c5ccf06ff79a2fd301b93636`でコミット済みである。本調査は最適化を実装していない。Rustの区間計測には一時的な変更を使い、計測用addonのビルド後に元のソースを復元した。

## 調査結果

- NAPI経由の同一入力再投入は、今回の入力ではartifact cacheに命中しなかった。新しいsource file IDがartifact keyを変える。
- 全量処理では、各モジュールの更新が全artifactのkeyを再計算する。composition検証も全入力に対して繰り返される。この2経路が大きな処理時間を占める。
- 1ファイル編集では、コンパイル本体よりsessionのclone、診断の全体更新、lint更新が重い。
- CSS planningでは、fragmentごとにmappingを走査する処理が主なコストである。
- checkerとloaderの再利用は有効である。ただし、元の小さなhost入力だけでは依存グラフの処理時間を判断できない。
- source-map候補は追加測定でも速かった。Browser Modeの第一候補はCSS cascade検証であり、frameworkのE2E全体を移す利点は小さい。

## 測定条件と証拠

環境はWindows x64、Node.js 24.21.0、Vitest 5.0.1、TypeScript 7.0.2、AMD Ryzen 7 9800X3Dである。CPUは8 core、16 logical processorを持つ。native addonはrelease profileでビルドした。benchmarkは順番に実行し、測定中に他の検証を実行しなかった。

元の46計測ケースは3〜9 sampleである。追加測定は通常15 sampleと5 warmupを使い、disk上の依存グラフは5 sampleと1 warmupを使った。統計と生sampleは[evidence.json](/F:/Projects/shorth/benchmarks/research/evidence.json)に保存した。時間の単位は、特記しない限りmsである。

元のJSONに記録されたGit revisionは`1b687b7f62d75166d10218d9d62b99da5927db4f`である。その測定時にはbenchmark移行が作業ツリーに存在し、後から上記のcommitに含めた。追加調査はそのcommitのproduction codeを使った。

元の測定と追加測定はwarmup数と実行順が異なる。JITとGCの状態も揃えていない。そのため、両測定の絶対値の差を最適化やrunner移行の効果として扱わない。source-map実装の比較には同じrun内の値を使う。

Rustのprofileは100・300・1000モジュールについて、freshと再投入を各2回、編集とplanningを各3回測定した。区間内には計測自体のコストが含まれる。`update_ms`は`style_lints_ms`等を含み、`current_key_ms`は複数の親区間に含まれる。これらを足すと重複する。Rust内部のtotalはNAPIの引数デコードと戻り値のJS変換をすべて含む値ではない。

## 全量コンパイルと同じ入力の再投入

元のbenchmarkの中央値は次のとおりである。各モジュールは8件の独立した`style()`呼び出しを持ち、CSSはminimal registryを使う。

| モジュール数 |    fresh | 同じ入力を再投入 | 1ファイル編集 |
| -----------: | -------: | ---------------: | ------------: |
|          100 |   187.67 |           335.22 |          6.88 |
|          300 |  1885.55 |          2509.93 |         22.21 |
|         1000 | 19598.34 |         37252.00 |        113.53 |

100から1000モジュールへの増加に対し、freshは約104倍、再投入は約111倍になった。区間計測でも同じ規模の時間を観測した。

### Cacheが命中しない理由

[HostPreparedSourceの入力登録](/F:/Projects/shorth/crates/napi/src/source_input.rs:451)は、呼び出しごとに`SourceRegistry::register()`を使う。[register](/F:/Projects/shorth/crates/source/src/text.rs:440)は新しいIDを発行する。[ModuleCompiler::key_for()](/F:/Projects/shorth/crates/compiler/src/module_compiler.rs:62)はそのIDを`source_file_id`に含める。IRのspanにもfile IDがあり、IRのDebug表現が`analysis_revision`の計算に入る。

計測では、再投入する100・300・1000モジュールがすべてcache missとなった。1000モジュールでは1000 miss、0 hitである。これは[ProjectSession::update_module()](/F:/Projects/shorth/crates/project/src/session.rs:366)にcache判定がないという意味ではない。Rust側で同じanalysisを渡す契約は[既存テスト](/F:/Projects/shorth/crates/project/tests/session_artifacts.rs:261)が検証している。NAPIが毎回入力を登録し直す経路でkeyが変わる。

対策の第一候補は、同じ文書改訂について既存の不変なSourceFileと解析結果を再利用することである。単にkeyからfile IDを削除すると、cached artifactのspanが古いSourceFileを指すままになる可能性がある。URI、source-map source、parser設定、projection、framework、type facts、stylesheet import等の変化を含めて改訂を管理する必要がある。失敗した試行の診断とlast good artifactを保持する契約も維持する。

### 全artifactのkeyを更新ごとに再計算する

[refresh_style_lints()](/F:/Projects/shorth/crates/project/src/session.rs:456)はresolverを作るために全artifactを参照する。各参照は[current_artifact()](/F:/Projects/shorth/crates/project/src/session.rs:298)を経由し、analysisからkeyを再計算する。key計算はIR等のDebug表現を作ってhashする。

1000モジュールのprofileで得た平均値は次のとおりである。区間は重複する。

| 指標                          |  fresh | 同じ入力を再投入 |
| ----------------------------- | -----: | ---------------: |
| native内部total               |  20769 |            36478 |
| モジュール更新区間            |  10850 |            24744 |
| そのうちlint更新              |  10138 |            23213 |
| current artifactのkey計算     |   9727 |            21977 |
| current artifactのkey計算回数 | 503500 |          1003000 |
| コンパイル本体の合計          |    249 |              301 |

freshではartifact集合が徐々に増える。再投入では最初から全artifactがあるため、各更新の参照対象が多い。これがfreshより再投入が遅くなる主な理由を説明する。

対策の候補は、改訂に対するartifactの有効性を一度検証して再利用し、batch内のlint更新をまとめることである。現時点の時刻や入力順に依存するcacheを増やすことは避ける。すべての入力のpublish後に最終graphの診断と出力を返す契約を守る。

### Composition検証を全入力に対して繰り返す

[NAPIのcompile loop](/F:/Projects/shorth/crates/napi/src/session.rs:706)は、各analysisに対して`attach_asset_support(&analyses)`を呼ぶ。[attach_asset_support()](/F:/Projects/shorth/crates/ir/src/analysis.rs:144)は全入力を使ってcompositionを検証する。[validate_composition()](/F:/Projects/shorth/crates/ir/src/composition.rs:74)は全style callからgraphを作る。今回の入力にはcomposition edgeがなくても、このgraph構築が繰り返される。

1000モジュールでfinalizeとsupport attachmentの合計は、freshで平均7606ms、再投入で9049msだった。全量処理を改善するにはこの経路も扱う必要がある。全入力のcomposition検証をbatchで共有し、各moduleに必要なasset supportだけをindexから選ぶ方向を推奨する。cycle、未解決base、framework不一致を検出し、診断を正しいconsumerのspanに結び付ける契約を維持する。

## 1ファイル編集とCSS planning

### 編集の主なコスト

1000モジュールを保持するsessionで、1モジュールを交互に編集したprofileの区間中央値は次のとおりである。各行は独立に中央値を取った値である。

| 区間            |     ms |
| --------------- | -----: |
| native内部total | 104.13 |
| sessionのclone  |  26.94 |
| 全体の診断更新  |  39.36 |
| lint更新        |  21.08 |
| コンパイル本体  |   0.28 |

1件の編集でもcurrent artifactのkeyを2002回計算した。[compileのtransaction](/F:/Projects/shorth/crates/napi/src/session.rs:684)がsessionをcloneし、[refresh_batches()](/F:/Projects/shorth/crates/napi/src/session.rs:775)が全current artifactから診断を作り直す。コンパイル本体だけを速くしても、大きな改善は見込めない。

artifactの有効性を再計算する回数を減らした後、変更したproducerと診断に影響するconsumerだけを更新する方向を推奨する。sessionのcloneは原子性とwatch時の失敗保持を担うため、削除する場合はtransaction journalや変更分のpublish等で同じ保証を提供する必要がある。

### Planningの主なコスト

元のplanning中央値は100モジュールで6.74ms、300で27.35ms、1000で149.06msだった。1000モジュールのprofileで得た区間中央値は次のとおりである。

| 区間                                     |    ms |
| ---------------------------------------- | ----: |
| source module indexとfragmentのparts生成 | 68.17 |
| artifactのstale判定                      | 19.98 |
| stylesheetのrender                       |  8.70 |
| ruleのsource origin付与                  |  7.82 |
| artifact CSSの組み立て                   |  3.59 |
| producerの順序決定                       |  0.43 |

[stylesheet_parts()](/F:/Projects/shorth/crates/project/src/stylesheet_plan.rs:315)は各fragmentについてmappingsとsegmentsを走査し、owner、slot、部分mappingを集める。fragment数とmapping数が増えると、同じ配列を繰り返し走査する。今回の独立したモジュールでは、topological orderの決定が主なコストではなかった。

generated rangeのindexまたは順序を使った走査で、fragmentとmappingの重なりを求める方向を推奨する。mappingの順序、重複するorigin、exact mappingのbyte範囲、slot、producer集合、occurrenceの順序を維持する。stale判定は全量処理と同じkey再計算の改善を共有できる。

domain側の計測区間合計は約108〜116ms、JSから測ったwall timeは約140〜150msだった。残りにはNAPIのtransport変換等が含まれるが、この調査では個別に分離していない。[planのtransport変換](/F:/Projects/shorth/crates/napi/src/session.rs:501)は各partと全体のsource mapを作るため、domain側の改善後に測る価値がある。

CSS carrier取得は元の1000モジュールケースでも中央値0.80msだった。現在の優先順位では、carrier専用の最適化より全量処理とdocument planningを先に扱う。

## Loaderとcheckerとhostの依存グラフ

元のfull-registry loader生成は中央値37.85ms、standalone compileは40.48msだった。追加測定では、既存loaderとsessionで1件の`style('flex')`を処理すると、minimal registryで0.11ms、full registryで0.82msだった。standaloneとは入力のcall数が異なるため厳密な速度比ではないが、registryの再読込を通常の更新経路から外す利点は明確である。

元のhost入力ではliteral編集が0.18ms、semantic編集が3.18ms、Vue・Svelte・Astro編集が約1msだった。追加したsemanticの区間計測では、平均値が次のようになった。

| 処理                  | 全体 | checker collect | native compile |
| --------------------- | ---: | --------------: | -------------: |
| 同じ入力              | 0.97 |            0.59 |           0.28 |
| redとblueを交互に編集 | 2.61 |            2.22 |           0.29 |

checkerは各caseで1回だけ起動し、21 snapshotのうち20件がincremental snapshotだった。restartはなかった。[TypeScriptFactsService.collect()](/F:/Projects/shorth/packages/host/src/type-facts.ts:342)は同じ入力でもsnapshotを更新し、diagnosticsとfactsを取り直す。入力が変わらない場合のfacts再利用には、依存ファイル、設定、type declaration、resolution、overlay、probeと宣言identity等の改訂を含める必要がある。

fresh serviceの追加測定は中央値47.11ms、平均49.93msだった。平均の内訳はservice生成21.13ms、最初のcompile28.57ms、close呼び出し0.23msである。最初のcompileにはcheckerのproject初期化等も入る。closeはsnapshotのdisposeとchildのkillを要求する処理であり、OS上の終了完了を待つ測定ではない。JavaScriptのdynamic importやconverterは既に読み込まれている。

さらに、disk上のproducerをimportする入力を測定した。producerはkeyframesをexportし、entryは各producerの宣言を使う。serviceとsessionを再利用した。編集caseはentryの末尾にcommentを追加し、importした宣言の内容は変えていない。

| producer数 | 同じ入力の中央値 | entry編集の中央値 |
| ---------: | ---------------: | ----------------: |
|         10 |             6.90 |              9.96 |
|        100 |           209.75 |            226.84 |

[compileModule()](/F:/Projects/shorth/packages/host/src/compiler.ts:156)は呼び出しごとに要求されたdependencyを読み、nativeにまとめて渡す。100 producerのケースではwarmupと5 sampleの合計で600回ずつreadとresolveが発生した。checkerのrestartはなかった。

この結果は実プロジェクト全体の測定ではなく、fan-outのある合成入力である。ただし、元の依存グラフを持たないhost benchmarkをそのまま開発時の応答時間と解釈できないことは確認できた。NAPIのbatch処理改善と、hostが保持するprepared sourceや依存改訂の再利用を一緒に設計することを推奨する。disk I/Oの時間割合はこのケースでは分離していない。

## Source map候補

productionは原文の各文字位置をMapに保存し、各CSS位置で全replacement keyを検索する。candidateはentryの開始位置を求めてcursorを進め、placeholderをpatternで検索する。両実装はCSSの各文字のmapping生成、VLQ encode、JSONとbase64の生成を引き続き行う。

1万ルールの追加測定で、各条件の中央値は次のとおりだった。各実装は15 sampleと5 warmupを使った。

| exact | sourcesContent | production | candidate | production時間 ÷ candidate時間 |
| ----- | -------------- | ---------: | --------: | -----------------------------: |
| false | false          |     135.72 |     64.32 |                           2.11 |
| false | true           |     136.63 |     74.96 |                           1.82 |
| true  | false          |     148.33 |     77.32 |                           1.92 |
| true  | true           |     151.82 |     82.59 |                           1.84 |

元の5 sampleの1万ルールケースは243.28msと103.53msで約2.35倍だった。追加調査の先行runも約2.1〜2.8倍だった。絶対値にはrun間の差があるが、同じrun内ではcandidateが一貫して速かった。

1000ルールで、CSSに現れないcanonical resource keyを増やす実験では次の中央値を得た。これは検索のmissを測る実験であり、実際のasset使用数を表すものではない。

| replacement key数 | production | candidate |
| ----------------: | ---------: | --------: |
|                 0 |      11.94 |      7.74 |
|                 8 |      12.01 |      8.87 |
|                64 |      18.92 |      6.03 |
|               256 |      49.94 |      5.79 |

candidateの小さいcaseにはばらつきとwarmup順序の影響があるため、keyを増やすと速くなるという結論には使わない。productionはkey数に応じて検索時間が増え、candidateの固定形式patternはその影響を避ける。

別プロセスで各実装を20回実行したNode CPU profileでは、productionの原文位置Mapを作るcallbackに約993msのleaf sampleが集まった。GCのleaf sampleはproductionが約535ms、candidateが約265msだった。profileは各1runであり、精密なallocation測定ではないが、候補が位置Mapの構築とallocationを減らすというコード上の説明に整合する。candidateでもmapping追加とencodeの時間が残る。

既存4テストと追加2テストが成功した。追加検証は、12 native producer、authored CSS、実際のresource placeholder、observedとcomplete coverage、sourcesContentの有無、および128通りのUnicode originとplaceholder prefixを扱った。既存テストはCRLF、複数origin、mapping gap、exactと非exact、upstream mapも扱う。

採用を推奨する。ただし、ここで確認したのはproductionとの観測可能な結果の一致であり、すべての入力に対する証明ではない。実装時はhostの既存source-map契約とfixture checksを維持し、browser acceptanceのCSS結果も確認する。候補の公開API、upstream source identity、任意placeholderの優先順を変える必要はない。

## Vitest Browser Modeへの移行候補

Browser Modeはテスト本体をブラウザーで実行する。[公式ガイド](https://vitest.dev/guide/browser/)はNodeとBrowserのprojectを分けられることを示している。[custom commands](https://vitest.dev/api/browser/commands)はNode側で実行できる。nativeコンパイルとファイル操作をNode側に残し、CSSとprops等のserializableな値だけをブラウザーへ渡せる。

| 対象                                  | 判断           | 根拠と移行内容                                                                                                                                                                                      |
| ------------------------------------- | -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| tools/tailwind/browser.test.ts        | 第一候補       | 手書きHTML、Chromium起動、debug port待機、WebSocketとCDPを独自管理している。CSS生成をNode commandへ分け、computed styleとcanvasの検証をBrowser Modeへ移す。                                         |
| e2e/css/tests/ref-scopes.test.ts      | 第二候補       | browser側はdocumentとscopeの操作が中心である。Node側でcompileとvm評価を行い、CSSとpropsを返せば移せる。既存scenario登録との接続を整理する必要がある。                                               |
| e2e/frameworksとFixtureApplication    | 現状維持を推奨 | 別のframework server、disposable workspace、build、SSR、hydration、navigation、HMRとdocument保持を検証する。test iframe中心のBrowser Modeへ移すと、既存driverを再構築する割に検証価値が増えにくい。 |
| host、native、integrationのNode tests | 現状維持       | NAPI、filesystem、compiler、build tool、型checkerが検証対象である。ブラウザーで動かす必要がない。                                                                                                   |
| Shape Lab等のDOM操作                  | 将来の追加候補 | pointer、keyboard、SVG座標、undo、localStorageとreloadの実際の動作を検証する価値がある。純粋なgeometryや保存形式のunit testはNodeに残す。                                                           |

Vite+は`vite-plus/test/browser`と`vite-plus/test/browser-playwright`をexportしていることをローカルで確認した。移行時も`defineConfig`とtest APIはVite+を使い、providerを使用中のVitest系列に揃える。[Playwright providerの公式設定](https://vitest.dev/config/browser/playwright)を基に、現在使うChromium系browserを指定する。

CSS cascade検証では、expectedとactualのそれぞれを独立したiframeへ置く必要がある。`@property`、layer、rootの継承、animationが別caseへ影響しないようにする。animationのpauseと時刻500ms、canvas filterのpixel比較、色変換の1 channel stepの許容、および意図的に異なるkeyframes結果を期待するcaseを維持する。computed CSSをDOM文字列のsnapshotに置き換えない。

この調査ではBrowser Modeへのコード移行とproviderの追加は行っていない。frameworkのacceptance matrixを再実行したという主張もしていない。

## 推奨する実施順と合格条件

| 順位 | 作業                                                        | 合格条件                                                                                                              |
| ---: | ----------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
|    1 | 同じ文書改訂のSourceFileとanalysisを再利用する              | NAPIの同一入力再投入でcache hitを確認する。文書・projectionの変更は正しくmissし、診断とsource mapが現在の原文を指す。 |
|    2 | keyの有効性確認、composition検証、lint更新をbatchで共有する | 1000 moduleで約100万回のkey計算を減らす。入力順、cycle、consumer diagnostics、失敗時のlast good artifactを維持する。  |
|    3 | source-map candidateをproductionへ採用する                  | Unicode、resource、mapping gap、upstreamとfixtureの契約を維持する。同じrunでproduction比較を行う。                    |
|    4 | 変更producerとconsumerの診断更新、transactionを改善する     | 1ファイル編集時の全体走査とcloneを減らす。原子性、失敗の保持、削除、CSS reloadを維持する。                            |
|    5 | stylesheet parts用のrange indexを導入する                   | owner、slot、mapping、順序が一致する。100・300・1000 moduleの伸びを再測定する。                                       |
|    6 | CSS cascade検証をBrowser Modeへ移す                         | 上記のiframe隔離、computed style、animation、pixel比較の契約を維持する。                                              |

loaderとcheckerを通常の更新で再作成しない方針は維持する。hostの依存ファイルcacheは、watchとresolutionの無効化条件を含めて1と2の設計に組み込む。秒単位の改善率や達成時間は、最適化を実装して再測定するまで約束しない。

## 再現と検証

追加benchmarkは`vp run --filter @stillwind/benchmarks research`で実行できる。native内部profileとNode CPU profileのコマンドは[benchmark README](/F:/Projects/shorth/benchmarks/README.md)に記載した。native profileを作る際は、同じRustソースに対する編集やbuildを並行して実行しない。

追加の正しさ検証は通常のbenchmark packageのtest taskに含まれる。元の3 benchmark taskの実行方法は変更していない。重いnative profileは少数の診断用測定であり、通常の検証では実行しない。

`vp run verify`は成功した。check、通常のtest、fixture checkを通過し、追加したsource-mapの正しさ検証も含まれる。`vp run verify:browser`も成功し、既存のCSS cascade検証を通過した。frameworkのE2E全体とWASM検証は今回実行していない。

追加benchmarkは17 assertion、25計測ケースが成功した。loader再利用の2ケースは両registryで有効な`flex`を使って再測定し、証拠にその値を保存した。source-mapの正しさ検証は既存4テストと追加2テストが成功した。計測後のRustソースは元の内容に復元されており、productionの最適化とBrowser Modeへの移行は未実施である。
