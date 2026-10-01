# Stillwind source-map performance research

2026-09-10。結論は **既存の編集・map契約を維持して線形化する案を先に採用し、oxc_sourcemapへの移行は分離する**。
本体のソース・依存は変更していない。試作、比較テスト、計測結果、再実行用Vite+タスクを追加した。

## 判断

| 案                                                        | 判断                     | 実験結果                                                                    |
| --------------------------------------------------------- | ------------------------ | --------------------------------------------------------------------------- |
| Rustの置換とUTF-16対応範囲を単一走査で生成                | 本体への移植を推奨       | 大量編集の二次的な処理を除去でき、コードと対応範囲が一致                    |
| CSSのURL探索をplaceholder走査＋Map検索にする              | 本体への移植を推奨       | 候補数に比例した全文検索を除去。CSSとinline mapがbyte単位で一致             |
| CSSの全文・全文字座標Mapを、mapping開始位置＋cursorに置換 | 本体への移植を推奨       | 多数のMapエントリ・配列割り当てを除去。共有ソースの順序変更・再利用にも一致 |
| JSで生成した全segmentをoxc_sourcemapへJSON転送            | 不採用                   | 符号化単体は速いが、実際のN-API＋JSON往復を含むと大きな入力で遅い           |
| Rustの編集から直接oxc_sourcemapを生成                     | 将来候補                 | 出力と全segmentが一致。ただしhost編集を残した全体設計が別途必要             |
| native mapとhost編集mapを新たに二段合成                   | 厳密互換の案として不採用 | GLB一致だけでは不十分。実験でLUB検索結果が29位置変化                        |

oxc_sourcemap自体にこの不一致があるのではない。直接生成したmapは一致している。
新たに追加した `@jridgewell/remapping` 合成段階が重複segmentを省くため、
「nativeでmapを返してhostで合成すれば既存と同じ」という移行方法が厳密には同値でない。

## 計測値

正確な数値・対象revision・本体ファイルのSHA-256は [results/timings.json](results/timings.json) に保存。
以下は同ファイルの2026-09-10の測定。Windows、Node 24.21.0、Rust release build。

| 入力／処理                                         | 現行アルゴリズム |     試作 |
| -------------------------------------------------- | ---------------: | -------: |
| Rust 1,000編集、227KB、コード＋対応範囲生成        |         17.01 ms | 0.168 ms |
| Rust 5,000編集、415KB、コード＋対応範囲生成        |        322.50 ms | 0.391 ms |
| CSS約204KB、URL候補256件、inline mapまで           |        149.80 ms | 13.83 ms |
| authored CSS 198KB、URLなし、inline mapまで        |         49.64 ms | 18.77 ms |
| 2.1MBのソースから小さなCSSを出力、process peak RSS |        258.0 MiB | 83.7 MiB |

Rustの入力には170KBの未編集末尾がある。`replace_range`の繰り返しによる末尾移動と、
編集ごとの先頭からのUTF-16再走査を意図的に顕在化させたstress case。
これはアプリ全体のコンパイルがこの倍率で速くなるという計測ではない。
小さい入力はsub-millisecondの揺らぎが大きく、100編集の最終sampleでは両者約0.67msだった。

CSSのURL件数比較では、約168KBの共通のunmapped paddingに、候補数に応じた短い規則を加える。
URLなしのdense exactケースも別に計測し、paddingだけに有利な変更でないことを確認した。
メモリケースは各実装を別processで起動し、同じ入力で `process.resourceUsage().maxRSS` を記録。
RSSはNodeランタイム等を含むprocess全体のpeakであり、mapだけの正味allocationではない。
`sourcesContent`は両方とも最終出力から省略する。出力は826 bytesで一致する。

### oxc_sourcemapの境界コスト

23万segmentの同じ入力について、builder＋JSON符号化はJS約13.97ms、Rust約3.24ms。
しかし `JSON.stringify(tokens) → N-API → Rust JSON parse/build/encode → JS JSON.parse`
まで含めると約37.79msになり、現行JSより遅かった。

元ソース＋1,000個の編集からnativeで直接code＋mapを生成する案では、Rust内部約0.475ms、
実際のN-API／JSONを含め約1.47ms。同じ入力の既存hostのmap生成だけも約1.47ms。
比較対象の仕事量は異なるため、これだけで全体の勝敗は決めない。
この実験はnative内部に元ソース・編集が既にある場合の将来性を示すが、
単に既存host loopをN-API呼び出しに替えるだけの明確な優位性は示していない。

`processRoundtripMs`も記録しているが、子process起動・7回の内部計測・入出力を含む。
N-APIの計測とは別物であり、採用判断の速度比には用いていない。

## 正しさの比較

最終runnerの `checks` は1,212。個々のassert数ではなく、完了した比較case数。

- JS：Unicode scalar境界を総当たりした単一置換198件＋seed固定の複数編集200件を、
  子processと実際のN-API addonの両方で比較。削除、空範囲、複数行置換、CRLF、BMP、astralを含む。
- CSS：seed固定400件。exact／anchor／unmapped、複数ソース、同一ソースの断片の逆順・反復、
  mapping一覧の逆順、URLの削除・長さ変更・改行を含む。
- prefixのあるliteral key、正規表現のメタ文字、Unicode key、未知のcompiler placeholder、空CSSを比較。
- JSのcodeと、mapのfile／sources／sourcesContent／デコード後の全segmentを比較。
  segment単位の一致は、特定のclass位置だけのspot checkより厳しい。
- CSSはinline mapまで含め文字列全体を比較。URL変更後の元行・列について独立した期待値も検査。
- oxc生成mapを既存trace-mapping consumerで正引き・逆引きし、上流map合成も比較。
- 実際のhost finalizerによるimport削除・runtime／CSS import追加を通す実験を別に実施。

`sourcemap:acceptance` は本体nativeをrelease buildし、現在の
`packages/host/tests/css-source-map.test.ts` をbaselineとcandidateそれぞれに対して実行する。
テストファイルは変更・複製保存せず、実行時にimport先だけ置き換える。4件×2、全成功。
[results/acceptance.json](results/acceptance.json) に実行結果を保存する。

### 二段合成で見つかった違い

native map＋host編集mapを合成した結果、直接生成時58 segmentが29 segmentに減った。
全生成位置の検索ではGLB（直前を選ぶ通常の検索）は差0、LUB（直後を選ぶ検索）は差29。
元のmapは複数行の置換内容全体に元call開始位置を記録している。
重複segmentを消すと、この密度を前提にしたLUB検索が別の元位置またはnullを返し得る。

実際のframeworkで不具合が発生したという報告ではない。ただし「既存とあらゆる検索で同値」
という採用条件を満たさない反例なので、この移行方法を合格にはしていない。

## 本体へ移植する場合の構造

### JS

`SourceTransform`の検証済み・整列済みの範囲を前から走査し、未編集sliceとreplacementを追記する。
同じ走査でoriginal／generatedのUTF-16累積位置を更新する。
未編集部分と削除された部分は元ソース内で重ならず、replacementも一度だけ数える。
ソートを除いてO(元ソース長＋出力長＋編集数)になる。

元ファイルID、UTF-8境界、範囲外、重複の検証と `TransformError` は既存のものを維持する。
この実験のRust kernelは正しいspanの性能を測るための試作で、入力エラーAPIを代替するものではない。
hostへ返す既存の4つのUTF-16 offsetの契約を維持すれば、host map生成・finalizer・上流合成を変更せずに移植できる。
`SourceFile::utf16_offset`自体は他の用途にもあるので、APIを削除する必要はない。

常時同期を強制する大きな編集データ構造は不要。
単一のreplacement rendererからcodeと対応範囲を返せば、重複したdelta計算・二重ソートを整理できる。
oxc版のbyte→行列変換も魔法のAPIではなく自前で必要だった。
元々VLQ符号化は `gen-mapping` に任せているため、クレートを入れるだけで大量の独自codecが消える状況ではない。

### CSS

`StylesheetMapping`のoriginとexact契約は維持する。
各sourceについて必要なmapping開始位置をソートし、一度の走査で開始座標を求める。
各mappingが独立cursorを持つので、出力中で元位置が逆行・反復しても問題ない。
exact区間内では元ソースcursorを前進させ、anchor区間では開始位置を維持する。
URL置換後のgenerated行・列は従来と同じ出力ループで進める。

本体のplaceholderは固定長hex fingerprintなので、固定パターンで走査しMapで存在を確認できる。
候補数に応じて巨大な正規表現を作る必要もない。
試作は公開関数の一般的なliteral keyにも対応するため、compiler形式以外ではescapeしたalternationにfallbackする。
一般keyについては任意に巨大な正規表現の性能まで線形とは主張しない。空keyは現在の処理でも進行しないため対象外。

このCSS試作は大きなper-character座標表を除去する代わりにcursor helperを追加しており、
単独ではソース行数が減ってはいない。性能・メモリの改善とコード量削減は区別する。
CSS originの逆照合やproject全体の規則索引は、この実験では変更・計測していない。

## 再実行

リポジトリの依存関係を `vp i` で用意した環境で、ルートから実行する。
実験用Rust crateは独立workspaceであり、本体のCargo.lockや依存関係を変更しない。

```powershell
vp run sourcemap:native-test
vp run sourcemap:research
vp run sourcemap:acceptance
```

`sourcemap:research` は実験CLIとN-API addonを先にrelease buildする。
計測タスクのcacheは無効。`results/timings.json` は実行ごとに更新する。
CSS/JS計測はwarmup後5回の中央値、Rust codec／emitterは7回の中央値。
Rust編集kernelは5回の中央値、memory workerは各実装1 process・1回の測定。
性能閾値をテストのassertにはせず、正しさのassertと分離している。

## 検証の限界と残る採用条件

この調査では本体の置換・map生成経路は未変更。
入力生成・全segment比較・実native acceptanceで反例を探したが、有限のテストだけで全入力の無欠陥は証明できない。
正しい利用の範囲は、不変の元ソースに対する有効なUTF-8範囲、整列可能な非重複の置換、
有効なsource indexとorigin、exact区間の元テキスト一致である。
複数の同位置挿入の優先順序を新たに定義する変更は含めない。

本体へ移植後には既存のRustエラー契約テスト、host/Viteテスト、frameworkのcold/warm source-map acceptanceを再実行する。
今回、frameworkの実アプリ全体のcold/warm buildやHMRは実行していない。
実験用N-API addonは計測目的であり、正式な公開APIではない。

参考：調査対象 [oxc_sourcemap 8.1.2](https://docs.rs/oxc_sourcemap/8.1.2/oxc_sourcemap/) と
[SourceMapBuilder](https://docs.rs/oxc_sourcemap/8.1.2/oxc_sourcemap/struct.SourceMapBuilder.html)。
複数ソースの登録・segment生成・符号化には使えるが、Stillwindのorigin判定や範囲編集を自動で行うものではない。
