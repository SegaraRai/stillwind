# ソートの分類と性能改善

入力の分類とutilityの順序を別々に処理する。canonical判定のキャッシュは維持している。

## 入力の分類

`sort.arrays` は、直接記述した配列、`style()` の引数列、オブジェクトのエントリーを分類する。既定は有効である。各refのmarkerとprojections、bases、utilities、custom props、attributesの順に並べる。refのグループは最初に現れた順を保ち、各グループ内ではmarkerをprojectionsより前に置く。marker同士とprojection同士も元順を保つ。その他の同じ分類内の入力は元順を保つ。

baseの本文、utility候補、生成CSSを調べず、nativeの宣言identityとTypeScript binderで入力種別を判定する。base引数の順序はCSS優先順位を決めない。ソート用のCSS干渉判定とbase本文の検査は削除した。複雑なbaseも単純なbaseと同じ分類になる。

未解決の式、spread、曖昧なコメント、元sourceとの対応を確認できない領域は、移動できる区間の境界になる。nested arrayやobjectは内部を先に処理する。複数の分類を含むnested containerは外側の移動の境界になる。

通常の分類はbucketへ一度ずつ追加するため、入力数に比例する。lint fixは交換する範囲ごとに抑制を確認する。任意の範囲を拒否できるpredicateを維持するため、この経路は隣接交換を使い、逆順の混在入力では二乗の交換回数になる。

## 独立した順序オプション

`sort.stringLiterals` はutility文字列とtemplate segment内部をcompiler順にソートする。既定は有効である。別々のutility配列要素は元順を保つ。未解決tokenは文字列内のソート区間を区切る。

`sort.attributes` は `aria-` と `data-` のproperty名をソートする。既定は無効で、他の二つのオプションとは独立して有効にできる。同名propertyの相対順は変えない。単独の属性名を持たないobjectやarrayは、属性名のソート区間を区切る。

## 重複処理の削減

`SourceIndex` はnative assetのUTF-8位置をsourceの一度の走査でUTF-16位置へ変換し、範囲をキーとするMapに保持する。nodeからassetへの検索結果も、この索引が表す変更のないsource graph内で再利用する。baseごとにAST全体を走査する処理はない。

compiler queryはcandidateを一度解決してutilityの順序キーとvariantのgroup位置・テーマ値を保持する。未解決の綴りも保持する。malformed requirementはエラーとして返し、結果をキャッシュしない。キーは一つのimmutableなquery内で使い、異なるテーマrevisionのqueryへ移さない。

hostは一つの文字列処理に現れたcandidateの集合を `candidateRanks()` へ渡す。Rustは準備したキーでbatchを並べて相対rankを返す。host内のtoken比較は数値rankを使い、比較ごとのbinding呼び出しとcandidate再解決を取り除く。rankはbatch内だけで有効であり、別batchのrankと比較しない。

token化は一度だけ行い、元のtoken列で変更の有無を確認する。明示的なsort要求はeditsを再構築する。診断の `reports()` はmoduleごとの結果を保持する。

## 測定の範囲

`bench:sorting` はNative、`bench:sorting:wasm` はNode WASI bindingを個別に測定する。queryのfresh測定はsnapshotをtimerの外で作り、初回のcandidateキー準備とedit構築を測る。repeated測定は同じsourceとsnapshotを使う。construction測定はsnapshotとsource索引の構築を含み、queryを含まない。CSS読み込み、compiler解析、LSP通信、ブラウザworker、editsの適用はすべて対象外である。

単一分類の配列は元順を保つため変更を返さない。混在入力のcaseで実際の移動も測定する。旧実装は分類内の入力まで並べ替えていたため、その配列の測定値は同じ出力を生成する処理の比較ではない。旧実装の [測定記録](sorting-performance-2026-10-07.md) と生のsamplesはそのまま保存する。

## 測定結果

Windows x64、AMD Ryzen 7 9800X3D、Node 24.21.0、Vitest 5.0.1、release bindingを使用し、各測定taskを個別に実行した。NativeとNode WASMはそれぞれ208ケースを通過した。通常はwarmup 2回、samples 9回で、1000入力のsourceと内訳はwarmup 1回、samples 3回である。以下は中央値、単位はmsである。

### 同じ出力を生成する文字列ソート

| 入力                          | Native 旧 | Native 現在 | Node WASM 旧 | Node WASM 現在 |
| ----------------------------- | --------: | ----------: | -----------: | -------------: |
| 100-token shuffle、authored   |     3.712 |       0.391 |        6.480 |          0.588 |
| 100-token shuffle、tailwind   |     5.696 |       0.493 |        9.376 |          0.974 |
| 1000-token 整列済み、authored |    20.617 |       2.478 |       29.488 |          4.005 |
| 1000-token 整列済み、tailwind |    22.062 |       2.991 |       34.879 |          6.567 |

### 分類による安定ソートと構築時間

| 入力・処理（authored）               | Native | Node WASM |
| ------------------------------------ | -----: | --------: |
| base 100個、初回                     |  0.288 |     0.296 |
| 異なるpropertyを持つbase 100個、初回 |  0.328 |     0.224 |
| ref 100個、初回                      |  0.391 |     0.381 |
| 混在入力 約100個、初回               |  0.322 |     0.299 |
| 混在入力 約1000個、初回              |  3.157 |     3.961 |
| 混在入力 約1000個、再要求            |  1.826 |     1.618 |
| 混在入力 約1000個、snapshot構築      |  4.736 |     9.163 |
| 属性 1000個、名前ソート初回          |  0.990 |     0.807 |
| 属性 1000個、snapshot構築            | 24.492 |    32.442 |

baseのみ、refのみの入力ではeditsが空になることを確認した。混在入力では分類間の移動が発生することを確認した。旧baseの測定は本文検査とCSS干渉判定を含み、独立したbaseを名前で並べ替えていた。現在はどちらの処理も実行しないため、旧時間との単純な速度比は同じ仕事の比較にならない。

### 順序キーの準備と再利用

| 処理（authored）                            |   Native | Node WASM |
| ------------------------------------------- | -------: | --------: |
| variant付き100候補、キーが空のbatch         | 0.865600 |  1.176400 |
| variant付き100候補、キー再利用のbatch       | 0.034700 |  0.246400 |
| variant付き1000候補、キーが空のbatch        | 7.707700 | 11.905900 |
| variant付き1000候補、キー再利用のbatch      | 0.428900 |  1.731400 |
| キー再利用後の比較1回（256回batchから換算） | 0.000348 |  0.002288 |

100候補batchの準備・ソート・binding往復を候補数で割ると、候補当たりは0.1ms未満だった。これはbatchの平均費用であり、個々の候補の最大時間を測った値ではない。cached比較も256回のbatchから換算している。任意のcustom utilityやvariantで同じ上限を保証する測定ではない。

通常のcandidate比較には0.01msのsoft targetと0.1msのhard limitをbatch単位で換算して設定した。sourceの100入力には1/5ms、1000入力には10/30ms、snapshot構築には30/300msを設定した。各測定はhard limitを通過した。少数のsamplesでは最大遅延や小さな差を判断できないため、上限は基準環境での大きな性能後退を検出するために使う。

## 再実行と根拠

`vp run bench:sorting`、`vp run bench:sorting:wasm`、`vp run bench:sorting:profile` を個別に実行する。一般の `test:performance` と `test:performance:full` もsortingの上限チェックを含む。

生のsamples、実行環境、測定境界、sourceのSHA-256、performance上限の判定結果は [sorting-optimization-evidence.json](../benchmarks/compiler/sorting-optimization-evidence.json) に保存した。旧測定の [sorting-evidence.json](../benchmarks/compiler/sorting-evidence.json) は変更していない。通常のreportsはignoredな `benchmarks/results/` に出力する。

静的検証 `vp run check`、全体テスト `vp run test`（Rust workspace、JS 929件、project fixture）、追加したvariant・aria/dataキーの回帰テスト、WASM・ブラウザ41件を確認した。一般のperformance taskも233件を通過した。ブラウザでは公開host APIを使うソートと、Worker内のLSPを検証した。追加の回帰修正は対象テストと最終静的検証で確認した。
