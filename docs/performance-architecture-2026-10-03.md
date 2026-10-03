# Sessionの設計とJSおよびCSSの性能改善方針

これは最適化前の調査記録である。実装と再測定の結果は[最適化レポート](/F:/Projects/shorth/docs/performance-optimization-2026-10-03.md)に記載した。

2026年10月3日に、session、compiler、Vite連携とCSS loaderを調査した。対象は1000 JSモジュールのフルビルド、watch開始、編集、およびCSS定義とCSSファイル数の増加である。source-map実装の採否とBrowser Mode移行は今回の検討対象から外した。

結論は、**成果物と不変なsnapshotをsessionに保持する設計は維持し、全体処理の繰り返しと深いcloneを先に減らす**ことである。同一入力の再コンパイルを省くキャッシュだけでは、初回ビルドと変更したファイルの編集は速くならない。まずsessionが現在の成果物を安く参照できる設計に整理する。その後、同じ改訂が再要求される経路について、再利用の効果と無効化条件を評価する。

本調査はproductionを変更していない。追加したコードは測定と再現のためのprobeである。

## 目標と測定範囲

ユーザーの回答に従い、watch開始は「監視を開始し、要求を受け付けるまで」とする。1000モジュールの初回コンパイル完了とページの初回表示は別に扱う。

| 指標       |      目標 | 合格判定に含める範囲                                                                                        |
| ---------- | --------: | ----------------------------------------------------------------------------------------------------------- |
| フルビルド |   5秒未満 | CSS読込、design system構築、1000モジュールの処理、JSとCSSの出力生成。native batch単体の時間も別に記録する。 |
| watch開始  |   1秒未満 | サーバーと監視の開始、CSSの初期化、要求受付。要求されていないJSはコンパイルしない。                         |
| watch編集  | 100ms未満 | 変更の受付から必要なコンパイル、診断更新、CSS再計画と送信まで。ブラウザーでの反映時間も別に測る。           |

今回のViteビルド測定は`build()`の呼出しから完了までであり、process起動、設定ファイルの読込、ディスクへの出力は含まない。watch開始の測定は`createServer()`と`listen()`の完了までである。既にimport済みのJSを使い、dependency discoveryとvocabularyファイルの生成を無効にした。全watch対象の初期走査が完了した時刻は分離していない。

watch編集の測定は実際のpluginのtransformを呼び、CSS planningと`hot.send()`までを含めた。filesystem eventの待ち時間、framework側の処理、WebSocketの配送、ブラウザーでの反映は含まない。`hotUpdate()`が先にcompileし、その後transformが再度走る経路もこの数値には含まない。したがって、今回の値だけで100msの目標を達成したとは判定しない。

目標の基準ケースは、1000モジュールに各8件の独立したstyle呼出しを持たせることを提案する。1件のケースも残し、モジュール数だけで負荷を表さない。literal、型による値の決定、compositionの依存グラフ、framework projectionを別に測る。CSSはbytes、定義数、ファイル数、importの形を別々に記録する。

## 現在のJS処理と目標との差

追加測定では、minimal registryで`style('flex')`を使った。各モジュールの出力を保持するViteビルドと、再利用したwatch sessionでの編集を測った。

| ケース                                  |     時間 | sampleと制約                                          |
| --------------------------------------- | -------: | ----------------------------------------------------- |
| native batch、1000モジュール、各1呼出し |   4.38秒 | 1 sample。loader生成とdocument CSSの生成は含まない。  |
| nativeを個別に1000回呼ぶ、各1呼出し     |  10.03秒 | 1 sample。Vite等のhost処理は含まない。                |
| Viteビルド、1000モジュール、各8呼出し   |  33.23秒 | 1 sample。CSS minifyを含み、`write:false`を使った。   |
| watchで1000モジュールを要求、各1呼出し  |  20.10秒 | 初期投入1回。transformと各回のCSS送信を含む。         |
| watchで1000モジュールを要求、各8呼出し  |  88.95秒 | 初期投入1回。watch開始時間とは別の値。                |
| watch編集、1000モジュール、各1呼出し    |  40.69ms | 5 sampleの中央値。                                    |
| watch編集、1000モジュール、各8呼出し    | 207.72ms | 5 sampleの中央値。最初のstyle呼出しを交互に変更した。 |

1 sampleの結果に表示される誤差0%は信頼性を意味しない。時間の規模と処理経路の差を確認するための値である。最適化後の合格判定には複数の独立したrunが必要になる。

フルビルドは5秒の目標を大きく超えている。watch編集も8呼出しのケースでは、ブラウザーに届く前に100msを超えている。JSを要求時に処理する設計はwatch開始を短くするが、要求後の長い待ち時間まで解決するものではない。

[ViteのbuildStart](/F:/Projects/shorth/packages/integration/src/vite/plugin.ts:50)はCSSとsessionを初期化する。[IntegrationSession](/F:/Projects/shorth/packages/integration/src/session.ts:9)は型checkerを必要時に作る。JSのcompileはtransform時に行う。この責務の分け方はwatch開始の目標と合っている。

## キャッシュを用いることは現行の設計に合うか

### 保持する状態と再計算を省くキャッシュを分ける

現行の境界は、hostがI/Oとresolutionを行い、frontendがscanとanalysisを作り、compilerが不変なdesign-system snapshotに対してartifactを作り、project sessionがproducerと成果物を管理する形である。[crate architecture](/F:/Projects/shorth/docs/crate-architecture.md:97)もこの分担を定めている。

sessionがartifactを保持するのは、再コンパイルを省くためだけではない。document CSSの組立て、producerの削除、共有ruleの所有、consumerの診断、watch失敗時のlast good outputにも必要である。この状態を取り除いて毎回全体をコンパイルする方針は、編集の目標と合わない。

| 再利用の単位                                 | 判断                 | 理由                                                                                                               |
| -------------------------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------ |
| 不変なdesign-system snapshot                 | 維持                 | sessionとcompilerは同じ解決済み定義を使える。`Arc` cloneは既にこの境界を表している。                               |
| 現在のartifactとlast good artifact           | 維持                 | projectの状態であり、CSSと診断と失敗保持に必要である。                                                             |
| 受理したanalysisのidentityとartifactの有効性 | 更新時に確定する     | 同じ不変な入力に対する有効性を、読むたびに求め直す必要はない。                                                     |
| 同一入力のartifact再コンパイル省略           | 限定して評価する     | 同一改訂の重複要求には有効だが、cold buildと変更入力には命中しない。                                               |
| CSSファイルの変更確認                        | 鮮度の契約を維持する | 更新通知だけに依存しない既存の読込経路がある。要求単位の共有と並列読込を先に検討する。                             |
| diskへの永続artifact cache                   | 当面導入しない       | process内のFileIdとspanを含む成果物を、そのまま別processで再利用できない。cold buildを速くする最初の対策ではない。 |

### 現在のkeyは有効性確認に費用がかかる

[ModuleCompiler::key_for()](/F:/Projects/shorth/crates/compiler/src/module_compiler.rs:62)はIR、summary、diagnostics、asset support等のDebug文字列を作ってhashする。[current_artifact()](/F:/Projects/shorth/crates/project/src/session.rs:298)と[stylesheet planning](/F:/Projects/shorth/crates/project/src/stylesheet_plan.rs:167)は、保持したanalysisからこのkeyを再計算する。

前の区間計測では、1000モジュールのfresh処理でcurrent artifactのkeyを503500回、同一入力の再投入で1003000回計算した。freshのkey計算は約9.73秒、lint更新は約10.14秒であり、前者は後者等に含まれる。compile本体の合計は約0.25秒だった。これらの重複する区間を足して改善率を計算してはいけない。

NAPIは毎回SourceFileを登録し直すため、同一テキストでもFileIdが変わり、今回の再投入ではartifact cacheに命中しなかった。ただし、**cache missを直すことと、大量のkey計算をなくすことは別の改善**である。cold buildの503500回には既存成果物を再利用する目的のcache hitがなくてもよい。有効性を安く読む設計にすれば、cold buildも速くできる。

推奨する形は、sessionが入力を受理する時に、そのanalysisと成果物の対応を確定することである。受理した不変なanalysisのstampと、公開したartifactのstampを保持する。読み取りはその対応を確認する。full keyを残す場合も、同じ受理済み入力から繰り返しDebug文字列を作らない。

stampはテキストのhashだけでは足りない。同じsourceでもfrontend IR、type facts、projection、stylesheet imports、adapter、compiler設定、naming等が変わる。[同一source revisionでIRを変更する既存テスト](/F:/Projects/shorth/crates/compiler/tests/module_compiler.rs:465)はその無効化を要求する。外部から渡された可変なanalysisを無条件に同一と扱わず、受理時の値をsessionが所有する必要がある。

同じ改訂のSourceFileやscanを保持することは、その後の選択肢になる。対象はsession内の文書改訂とし、URI、module identity、入力設定、依存する意味情報も対応させる。keyからFileIdだけを削除すると、診断やtransformが別の原文を指す可能性がある。永続cacheのidentityと、process内の原文identityを同じ仕組みに押し込めない。

### 全体の検証はbatchの最終graphに対して行う

[NAPI compile](/F:/Projects/shorth/crates/napi/src/session.rs:684)はscanをfinalizeしたanalysis集合を作り、loop内で再度finalizeする。各analysisは[attach_asset_support()](/F:/Projects/shorth/crates/ir/src/analysis.rs:144)で全入力のcompositionを検証する。1000モジュールではfinalizeとsupport attachmentが約7.61秒だった。各producerの更新はさらに全artifactからlint resolverを作る。

これはキャッシュより、処理を行う回数と責務の問題である。batchでanalysisを一度確定し、composition graphとassetの参照索引を共有し、更新後の最終graphで診断を確定する方向を推奨する。入力順に依存せず、cycleとconsumerのエラーを正しい原文に報告し、失敗したbatchを部分公開しないことが条件になる。

個別のVite transformは1000件のbatchとして到着するとは限らない。native batchだけを速くしても十分ではない。個別更新でも、変わらないproducerを検証し直さず、必要なconsumerの診断だけを更新できる状態管理が必要である。

## CSS定義数がsessionのcloneを重くする

[CompilationDependencies](/F:/Projects/shorth/crates/compiler/src/compiled_module.rs:68)は全utility、variant、propertyの名前を各artifactに記録する。[compiler](/F:/Projects/shorth/crates/compiler/src/module_compiler.rs:228)が集合を複製し、[DependencyIndex](/F:/Projects/shorth/crates/project/src/dependencies.rs:1)がdefinitionごとに全producerのmembershipを保持する。JSモジュール数をN、定義数をDとすると、保存量にはN×Dの項がある。

このindexは現在のreloadで選択的な再コンパイルに使われていない。[reload](/F:/Projects/shorth/crates/project/src/session.rs:530)は変更されたsnapshotに対して全analysisを更新する。リポジトリ内の`dependencies()`呼出しも調査したが、現時点ではRustの契約テストが主な利用者だった。public Rust APIである点は別に考慮する必要がある。

100モジュール、各1呼出しの状態で、使わないutilityを増やした結果は次のとおりである。

| 追加utility数 | freshの中央値 | 編集の中央値 |
| ------------: | ------------: | -----------: |
|            10 |       52.32ms |       2.17ms |
|           100 |       51.48ms |       2.78ms |
|          1000 |       77.67ms |      14.97ms |

別途nativeの区間計測を行い、同じ規模のsessionを3回編集した。次の値は各区間の中央値である。`commit`は新sessionへの代入と旧sessionの破棄を含む。

| 追加utility数 | native total | session clone |  commit | compile本体 |
| ------------: | -----------: | ------------: | ------: | ----------: |
|            10 |       3.40ms |        0.50ms |  0.28ms |      0.07ms |
|           100 |       3.43ms |        0.87ms |  0.43ms |      0.07ms |
|          1000 |      14.85ms |        8.67ms |  3.81ms |      0.10ms |
|         10000 |     211.43ms |      135.34ms | 66.98ms |      0.93ms |

CSS定義数の増加による編集の遅さは、主にcloneと破棄の区間に現れた。定義を使うcallを増やしていないため、utilityの解決自体を速くするだけではこの経路は改善しない。

全定義に対する保守的な依存を、各producerに名前集合として複製する設計は見直すべきである。全snapshotに依存するという事実を共有した値で表すか、現在の全量reloadに不要なdefinition別membershipを整理する。その際、compositionのconsumer関係まで削除しない。選択的なCSS reloadを導入するなら、実際の解決経路、未解決candidate、定義の追加、fallback、tokenの推移的依存等を記録する別の設計が必要になる。

transactionの原子性は維持する。不変なanalysis、CSS artifact、依存集合を共有する`Arc`等を使えば、session cloneでその内容をすべて複製する必要がなくなる。まず共有できる値の境界を整理し、その後に変更分をstageしてpublishする方式を検討する。read時のkey再計算と深いcloneを残したまま、さらに汎用cache層を重ねる方針は推奨しない。

## CSS量とimportの形を別に測る

### 平坦なCSSのdesign system構築

1ファイルに通常のrule、theme token、utilityをそれぞれ増やした。snapshot構築の中央値は次のとおりである。I/Oは含まない。

| 入力の種類     |  100件 |  1000件 |  10000件 | 10000件のbytes |
| -------------- | -----: | ------: | -------: | -------------: |
| 通常のCSS rule | 1.08ms | 11.03ms | 114.30ms |         441531 |
| theme token    | 0.80ms |  8.36ms |  78.74ms |         324670 |
| utility        | 0.91ms |  9.30ms |  90.17ms |         414661 |

10000件の通常ruleに対する空sessionの作成は約6.85ms、utilityのvocabulary生成は約0.76msだった。通常ruleと今回のtokenだけではvocabularyはほぼ増えていない。builtinの種類を増やす実験ではない。

この範囲では、平坦なCSSの増加は概ね入力規模に応じて処理時間を増やした。loaderは各sourceを検証し、importを展開し、展開後の全CSSを再びparseしてsnapshotとfingerprintを作る。[loader](/F:/Projects/shorth/crates/design-system/src/loading/state.rs:286)と[parse_theme](/F:/Projects/shorth/crates/design-system/src/theme/parser.rs:62)に複数の走査がある。より大きな入力では改善余地があるが、今回のJS処理の秒単位の問題より先に、CSS parse cacheを導入する根拠は得られていない。

### 小さいCSSでもファイル数で遅くなる

同じ小さいruleを別ファイルへ分けた。fanoutはrootが全ファイルを直接importし、chainは各ファイルが次をimportする。hostのreadはメモリ上で行ったため、この表はdiskの遅さを測っていない。

| ファイル数 | fanoutで個別に応答 | fanoutでnativeへbatch応答 | chainで個別に応答 |
| ---------: | -----------------: | ------------------------: | ----------------: |
|        100 |             3.61ms |                    1.07ms |            1.57ms |
|        300 |            24.80ms |                    3.39ms |            6.77ms |
|       1000 |           235.27ms |                   11.02ms |           50.60ms |

fanoutの1000件でも入力は約45KBである。[loadCssGraph()](/F:/Projects/shorth/packages/host/src/css.ts:25)はread完了ごとに`provide()`し、[loader](/F:/Projects/shorth/crates/design-system/src/loading/state.rs:262)はそのたびに到達済みgraphの順序と未完了requestを調べ直す。batch応答は同じsnapshotのvocabularyを返しながら、処理回数を減らした。

全I/Oの完了を待ってから一括処理する方式へ単純に変えてはいけない。既存テストは、遅い兄弟importを待つ間に先に読めた子のimportを開始する契約を守っている。完了済みの応答だけをまとめ、追加のrequestは直ちに開始する方式か、loader側のpendingとcycle検証を更新単位で処理する方式を検討する。chainでは展開途中の文字列とmappingを各祖先へ複製するため、depthに応じた保存量も別に検討する。

## watch開始とCSS変更確認

平坦なCSSを使い、1000 JSファイルを置いて一切要求しない場合の`listen()`までの中央値は、minimalで36.74ms、約267KBのfull registryで176.14ms、10000 utilityで249.24msだった。今回の制御した環境では1秒以内である。ただし、通常の設定、dependency discovery、cold process、framework起動、複数environmentの費用を含めて達成を確認する必要がある。

CSSをdisk上の複数ファイルへ分けると、別の問題が出た。rootを除くファイル数に対する中央値は次のとおりである。編集するJSは1モジュール、1呼出しである。

| CSSファイル数 | `listen()`まで | JS編集とCSS送信 |
| ------------: | -------------: | --------------: |
|           100 |        43.61ms |         15.51ms |
|          1000 |       513.47ms |        141.85ms |

[designSystemVersion()](/F:/Projects/shorth/packages/integration/src/vite/project.ts:167)は監視対象の全ファイルを順番にreadしてcontent hashを作る。通常のJS編集でも[compile](/F:/Projects/shorth/packages/integration/src/vite/project.ts:389)が確認し、[publish](/F:/Projects/shorth/packages/integration/src/vite/project.ts:368)もdocumentPlanを通じて再確認する。この入力では各巡回がrootを含む1001件を読み、1回のtransformで2巡する。141.85msのうち各処理が占める時間は個別には計測していない。

同じ1000個の小さいファイルをreadして同じ順序でhashする実験では、逐次処理が88.91ms、16件ずつの並列処理が14.70ms、64件ずつが12.01ms、256件ずつが12.22msだった。各方式でhashの一致を確認した。これは読込とhashだけの実験であり、Viteの編集全体を12msにできたという結果ではない。

ここでは、watcher通知だけに基づくcacheへ置き換える前に契約を確認する必要がある。[既存テスト](/F:/Projects/shorth/packages/integration/tests/vite/session.test.ts:154)はHMR接続のないenvironmentでも、新しいstylesheet要求がファイルの編集を観測することを要求している。`mtime`だけで内容の改訂を判断する方式も、同じ長さの短時間の書換えを確実に検出する保証にはならない。

第一候補は、内容を読む契約を保って読込を適度に並列化し、compileとpublishで同じ操作のsnapshot確認を共有することである。外部のstylesheet要求では必要な鮮度確認を残す。compileとpublishは一つのcoherentなgraph revisionを扱い、途中のCSS変更は次の操作として処理する。CSS更新、失敗したreload、clientとSSRの独立したsnapshot、連続した編集の順序を既存テストで確認する。watcherを使った変更受付への整理は、その後にrequest時の鮮度契約と合わせて設計する。

[hotUpdate()](/F:/Projects/shorth/packages/integration/src/vite/development.ts:24)には、先にcompileしてからJSの違いを判断する経路もある。その後のframework処理やtransformで同一改訂が再投入されるかを実際のHMR経路で測る。この重複を同じ改訂の成果物の再利用で省くことは、キャッシュを検討する具体的な用途になる。

## 推奨する実施順

| 順位 | 変更の方向                                                                                                                      | 主に改善する指標                   | 必要な検証                                                                                                   |
| ---: | ------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- | ------------------------------------------------------------------------------------------------------------ |
|    1 | 受理したanalysisとartifactの有効性を更新時に確定し、keyの再計算をなくす。batchのfinalize、composition検証とlint更新をまとめる。 | フルビルド、要求後の初回処理、編集 | 同一sourceで異なるIRやfacts、入力順、cycle、consumer診断、失敗batchの原子性を維持する。個別更新も測る。      |
|    2 | 全定義の依存membershipの複製を整理し、不変なanalysisとartifactをtransactionで共有する。                                         | CSS定義が多い場合の編集とビルド    | last good output、現在の診断、producer削除、共有rule、symbol collision、CSS reloadを維持する。保存量も測る。 |
|    3 | CSSの内容確認を並列化し、一つのcompileとpublishで確認結果を共有する。                                                           | CSSファイルが多い場合の編集        | HMR接続のない要求の鮮度、同時編集、reload失敗、clientとSSRを検証する。                                       |
|    4 | stylesheet fragmentとmappingの重なりをindex等で求め、全体を繰り返し走査しない。                                                 | 編集とdocument CSS生成             | owner、slot、producerとoccurrenceの順序、同一ruleの共有と削除を維持する。source-map実装の置換は含めない。    |
|    5 | CSS loaderの完了済み応答をまとめるか、graphの検証を更新単位にする。                                                             | CSS importが多い場合の開始         | 子importの開始を遅らせず、応答順、wrapper、cycle、失敗の契約を維持する。                                     |
|    6 | 同一改訂のscanと成果物をsession内で再利用する。                                                                                 | 同一入力の再要求とHMRでの重複処理  | 実際の重複を先に測る。source、入力設定、facts、依存改訂の無効化を保証する。                                  |

1と2はキャッシュ命中率を上げなくてもcold buildを改善する。3は内容確認の契約を保った小さな改善候補であり、並行して扱える。5もsnapshot全体のparse cacheを追加せずに改善できる。

5秒と100msは、現在の無駄の大きさから改善を目指す妥当な目標だと判断する。ただし、最適化後の達成時間は未測定である。型checkerが必要な大きい依存グラフ、frameworkの変換、CSSとJSを両方増やす実プロジェクトについては、さらに独立した測定が必要になる。以前の合成host graphでも100 producerの編集に約227msかかっており、literalの結果だけを合格条件に使わない。

合格判定には、1000×8呼出しの入力、full registry、CSSの10000定義と1000ファイル、compositionと型checkerを必要とする入力を測定群として残す。これらをすべて組み合わせた一つの巨大な入力にせず、各要因を分離した結果と、代表的な組合せの結果を両方記録する。中央値だけでなく遅いsampleを示し、fresh processと再利用中のprocessを分ける。サーバー内の処理が100msぎりぎりではブラウザーでの反映まで含めた目標を満たせないため、各区間に余裕を確保する。

## 証拠と再現

新しい結果と生sampleは[architecture-evidence.json](/F:/Projects/shorth/benchmarks/research/architecture-evidence.json)に保存した。前のnative profileは[evidence.json](/F:/Projects/shorth/benchmarks/research/evidence.json)を参照した。Node 24.21.0、Vitest 5.0.1、Windows x64、Ryzen 7 9800X3D、release addonで計測した。測定は他のbuildや検証と重ねなかった。

`vp run --filter @stillwind/benchmarks architecture`が新しい測定群を実行する。重い初期投入もあるため、個別の調査ではbenchmark fileとtest名を指定して実行する。nativeでCSS定義数の影響を分離するprobeは`registry-profile.mjs`である。profile addonの作成方法は[benchmark README](/F:/Projects/shorth/benchmarks/README.md)に記載した。

調査では32 test、66計測ケースを分けて実行し、成功を確認した。nativeのregistry profileは4種類の定義数に対して初期投入1回と編集3回を行った。productionの最適化、永続cacheの導入、キャッシュ契約の変更はまだ行っていない。

追加コードのTypeScript check、lint、format check、およびbenchmark packageの通常テスト6件も成功した。今回productionを変更していないため、workspace全体の通常検証とframeworkのbrowser acceptanceは再実行していない。
