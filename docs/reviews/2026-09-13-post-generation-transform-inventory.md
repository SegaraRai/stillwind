# 生成後の変換処理の棚卸し

調査日: 2026-09-13

## 結論と調査範囲

多くの処理は、値・宣言・selectorを構造として保持し、展開と合成を終えてから文字列化することで、生成時に処理できる。現在は書式調整、意味を変える変換、最終出力時に必要な処理が、文字列の後処理として混在している。

RustのCSS生成・レジストリ処理を中心に、関連するJS出力とホスト側の変換まで確認した。入力の一般的なデコード、パス正規化、テスト用の置換は主対象に含めていない。

以下の「不要」はCSSの動作に不要という意味で、出力文字列の一致を求めるテストとは区別する。「生成時へ移せる」は設計上の評価であり、移行実装や出力互換性の検証を済ませたという意味ではない。棚卸しではテストを実行していない。

コード位置は調査時点の目安。並行して進行している実装変更により、行番号や処理内容が変わり得る。

## 1. 書式調整・最適化が目的の処理

| 処理・実装                                                                                                                    | 生成段階でできるか                                                   | 必要性と理由                                                                                                                         |
| ----------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| [`space_var_fallbacks`](../../crates/compiler/src/compiler/utilities/declaration_order.rs)：`,var(` に空白を追加              | できる。値の出力時に区切り文字を決める                               | 動作には不要。出力表記を揃える目的                                                                                                   |
| [`canonical_builtin_body`](../../crates/design-system/src/registry/normalization.rs)：組み込み定義の空白正規化                | できる。組み込みレジストリの作成時に統一するか、共通の出力処理に集約 | 安定した表記には有用。ただし実行時に繰り返す必然性はない。トークンを区切る空白の保持は必要                                           |
| [`slash-space-span`](../../crates/design-system/src/evaluation/utilities/formatting.rs)：`span` を含む値の `/` 周辺に空白追加 | できる。gridの値を出力するときに処理                                 | 動作には不要。現在の値全体への置換より、区切りの `/` だけ出力する方が適切                                                            |
| [`empty-space`](../../crates/compiler/src/compiler/utilities/declaration_order.rs)：空の値を `--x: ;` にする                  | できる。空の宣言値を出力するときに決める                             | 空のカスタムプロパティ自体には意味があるが、空白を後から加える処理は書式上の都合                                                     |
| [`empty-var-fallback`](../../crates/design-system/src/evaluation/utilities/formatting.rs)：`var(--x,  )` → `var(--x,)`        | できる。空fallbackを表す値から直接生成                               | 基本的には表記統一。fallbackの存在を示すカンマは削除できない                                                                         |
| [`normalize_serialized_declaration`](../../crates/css/src/colors.rs)：特定の透明色表記を `#0000` に置換                       | できる。色やshorthandの出力時に正規化                                | 短縮・表記統一が目的。宣言全体への固定文字列置換は不要。特に「4個並んだ色を1個へ」はプロパティの文法を知る場所で判断すべき           |
| [`fold_calc_expressions`](../../crates/css/src/math/folding.rs)：計算式の簡約                                                 | できる。値の展開後、数式の構造を保持したまま簡約する                 | 有効なCSSの定数計算なら主に最適化・表記統一。複数段階で繰り返す必然性はない。型や有効性を保つ必要がある                              |
| [角度の文字列修復](../../crates/compiler/src/compiler/utilities/emission.rs)：`" deg"` → `"deg"`、`"deg*-1"` → `"deg * -1"`   | できる。数値と単位、数式を正しく出力する                             | 前者は不正な出力が生じるなら修正が必要だが、生成元で直すべき。後者は書式調整。現在も前者を必要とする生成例は今回特定できず、削除候補 |

## 2. 意味上必要だが、文字列化する前へ移せる処理

| 処理・実装                                                                                                                              | 生成段階でできるか                                          | 必要性と理由                                                                                                                                                                             |
| --------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`var-fallback`](../../crates/design-system/src/evaluation/utilities/formatting.rs)：`var(--x)` → `var(--x,)`                           | できる。対象の変数参照を生成するときに指定                  | 必要な場合がある。transformの任意成分が未定義でも、残りの成分を使えるようにする。ただし全参照への機械的追加ではなく、定義側で意図を示すべき                                              |
| [`zero-length`](../../crates/design-system/src/evaluation/utilities/formatting.rs)：`0` に `px` を付与                                  | できる。値に長さの型を保持して出力する                      | 用途依存で必要。単独の長さの `0` は省略可能でも、カスタムプロパティを数式で利用するときは数値と長さの区別が重要。単なる見た目の整形として扱うべきでない                                  |
| [`normalize_registered_var_fallbacks`](../../crates/design-system/src/evaluation/utilities/body.rs)：登録済み初期値と同じfallbackを削除 | 条件が確定すればできる。変数参照の生成時に判断              | 必須ではなく最適化。実際の登録や互換用初期値が保証される場合に限る。レジストリに初期値があることだけで削除してよいとは限らない                                                           |
| [`important`](../../crates/design-system/src/evaluation/utilities/formatting.rs)：宣言に `!important` を追加                            | できる。宣言の属性として保持する                            | 必要。important指定の意味を実装する。完成した文字列の再走査は不要                                                                                                                        |
| [`compose-space`](../../crates/compiler/src/compiler/utilities/declaration_order.rs)：同じプロパティの値を連結                          | 結合段階でできる。宣言のリストを保持して合成する            | 必要。transformなどで、後の宣言が前の値を上書きするのではなく、複数の成分を合成するため。個々のutility生成時には全成分が揃わない                                                         |
| [`prepend_missing_declarations`](../../crates/compiler/src/compiler/utilities/declaration_order.rs)：補助宣言の先頭追加                 | 結合段階でできる。明示宣言との優先順位を決めてから出力      | 必要。variantの補助宣言を補いつつ、明示指定を優先するため                                                                                                                                |
| [`insert_nested_declaration`](../../crates/compiler/src/compiler/utilities/emission.rs)：nested ruleの文字列挿入                        | できる。宣言と条件付きルールを順序付きで保持する            | 必要。条件付き上書きと宣言順序を表現するため。プロパティ名と次の `;` を探して挿入する必要はない                                                                                          |
| [selectorの `&` 置換](../../crates/compiler/src/compiler/utilities/render.rs)                                                           | できる。selectorテンプレートへ対象selectorを渡して出力      | 必要。utilityのクラスとvariantを合成する機能。文字列中の全 `&` を対象にする実装は避けられる。[style variant側](../../crates/compiler/src/compiler/style_variants.rs)にも同種の処理がある |
| [`substitute_variant_capture`](../../crates/design-system/src/evaluation/variants/substitution.rs)：capture・modifier置換               | できる。明示的なplaceholderを持つテンプレートとして展開する | capture展開は必要。ただし `:root`・`0n` の置換や `):[` の修復まで必要かは別。現在の組み込みnth系は型付きテンプレートを使っており、この旧形式を組み込みが必要とする根拠は確認できなかった |
| [`rewrite_shadow_color`](../../crates/css/src/colors.rs)：shadowの色の差し替え                                                          | できる。パースしたshadowの各成分から直接生成する            | 必要。色utilityによる上書きと元の色へのfallbackを実装するため。再serializeした文字列から色のsuffixを削る部分は不要にできる                                                               |
| [`rewrite_generated`](../../crates/compiler/src/options.rs)：名前空間prefixの全体置換                                                   | できる。生成するカスタムプロパティ名を共通の命名処理で作る  | prefix設定は必要。完成したCSS・JS全体への置換は不要。生成名とユーザーの文字列リテラルを区別できる構造が望ましい                                                                          |

### 書式と意味を区別する仕様上の根拠

- 空fallbackは「変数が無効・未定義なら何も代入しない」という意味を持つ。`var(--x,)` と `var(--x)` の違いは空白の有無とは異なる。[CSS Variables仕様](https://www.w3.org/TR/css-variables-1/#using-variables)
- `calc()` 内の `0` は自動的に長さにはならない。数式の簡約やカスタムプロパティの出力では型を保つ必要がある。これは、任意の数値ゼロへ無条件に `px` を付けてよいという意味ではない。[CSS Values仕様](https://www.w3.org/TR/css-values-4/#calc-type-checking)

## 3. 展開処理・最終出力の段階に理由があるもの

| 処理・実装                                                                                                                                                                       | 生成段階でできるか                                               | 必要性と理由                                                                                                               |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| [`--value()` などのDSL展開](../../crates/design-system/src/evaluation/values/placeholders.rs)、[theme wildcard展開](../../crates/design-system/src/evaluation/utilities/body.rs) | 現在も実質的には生成段階。テンプレートを構造化する余地はある     | 必要。candidate・modifier・themeから通常のCSSを生成するコンパイル処理                                                      |
| [色の互換分岐](../../crates/design-system/src/evaluation/utilities/declarations.rs)                                                                                              | 現在も生成段階で実施                                             | 対応ブラウザの方針次第で必要。旧形式の宣言とmodern形式の条件付き宣言を生成する                                             |
| [`@apply`・`@variant` 展開](../../crates/compiler/src/compiler/stylesheet/authored_directives.rs)                                                                                | authored stylesheetの解析・生成段階へ移せる                      | 必要。独自ディレクティブを通常のCSSへ変換するため。完成文字列の検索・置換である必要はない                                  |
| [使用する `@property` の選択](../../crates/compiler/src/compiler/stylesheet/properties.rs)、[keyframes抽出](../../crates/compiler/src/compiler/stylesheet/keyframes.rs)          | 使用情報を生成中に収集し、全体の組み立て時に出力できる           | 登録や使用するkeyframesの出力は必要。未使用分の除外は最適化。使用集合が揃う段階は必要だが、CSS文字列からの再抽出は減らせる |
| [`@layer properties;` の先頭追加](../../crates/compiler/src/renderer.rs)                                                                                                         | 最終組み立て時に直接生成できる                                   | レイヤー順序の確保に必要。`starts_with` 判定と後挿入は実装上の都合                                                         |
| [resource URLのplaceholder化](../../crates/compiler/src/module_compiler.rs) → [ホストでURL確定](../../packages/host/src/css-source-map.ts)                                       | placeholderは値生成時に作れる。最終URLはホスト側まで確定できない | 必要。ファイル解決・配信URL・バンドラの情報を使うため。後段処理を残す妥当な例。置換に伴うsource mapの位置調整も必要        |
| [HTML属性内の引用符変換・import文字列の `<` エスケープ](../../crates/compiler/src/runtime.rs)                                                                                    | 埋め込み先の文脈が確定した出力時に直接エンコード可能             | 必要な文脈がある。JSの内容がHTML側の構文を壊さないため。文脈を知らない早期生成段階へ一律には移せない                       |
| [ホストでCSS importを追加](../../packages/host/src/finalize.ts)                                                                                                                  | ホストの最終出力で直接生成可能                                   | stylesheet指定がある場合に必要。ホスト固有specifierを使うので後段にある理由がある                                          |
| [Playgroundの `export App;` 修復](../../packages/vite/src/index.ts)                                                                                                              | 入力側を標準構文にするか、明示的な構文変換として処理可能         | 通常のコンパイラには不要。Playground独自の入力許容のため。生成後の正規表現置換として残す根拠は弱い                         |

## 4. 確認した重複・矛盾と改善優先度

### 空白処理の重複と引用内容の保持

`combine_declarations_with_formats` では `comma-space` 分岐で `space_var_fallbacks` を呼んだ後、同じ関数を無条件で呼んでいる。調査時点では `declaration_order.rs` の121行目と127行目に相当する。

さらに `canonical_builtin_body` の末尾には、別の全体置換 `replace(",var(", ", var(")` が残っている。この置換は引用文字列にも作用するため、「引用内容を保持する」という関数の契約と矛盾する。引用を識別する前段の走査だけでは、末尾の全体置換による変更を防げない。

書式を保持する必要があるなら、値の出力処理へ集約する。不要なら、書式一致のテスト要件を確認したうえで削除する。

### `calc()` の処理箇所の分散

次の複数段階に計算式の簡約がある。

- `evaluation/utilities/formatting.rs` の `finalize_declaration`
- `compiler/utilities/declaration_order.rs` の宣言結合
- `compiler/utilities/render.rs` のrule描画
- `format_numeric_zero` 内の、ゼロの単位を考慮した簡約

展開後の値を一度だけ簡約する場所へ集約する方が明確。ただし、後続の展開や合成で新しい数式が生成されないことと、ゼロの型情報を保持できることを確認してから重複を除去する。

### `@stillwind-format` の責務混在

[tw-registry-metadata.css](../../packages/core/css/builtin-metadata/tw-registry-metadata.css) は空白、fallback追加、長さの型付与、transform合成を同じformatメタデータで扱っている。

書式設定、値の意味、宣言の結合規則に分ける必要がある。特に `var-fallback`・`zero-length`・`compose-space` は、任意の整形オプションと同じ扱いで無効化すべきではない。

### 生成物の修復がユーザーの値へ波及し得る

prefix、透明色、fallback、variantの連続置換は、生成した識別子やplaceholderだけを対象にできる形へ変えるべき。文字列化後では、生成物とユーザーの文字列リテラルの区別を失いやすい。

これは各経路の具体的な利用者影響をすべて再現したという判定ではなく、現在の変換範囲から見た設計上の懸念である。

## 5. 改善の方向と未検証事項

値・宣言・selectorを構造として保持し、展開と合成を終えてから一度だけ文字列化する。URL解決と埋め込み文脈のエンコードは、それぞれ情報が確定する境界に残す。

着手順の候補は以下。

1. 純粋な書式の要件と、意味を保つ要件を分離する。
2. 重複する空白処理と数式処理を整理する。
3. 角度の修復やvariantの旧placeholderについて、到達経路と必要な入力例を確認する。
4. 宣言の合成・優先度・値の型・変数参照を、文字列化前の構造へ移す。
5. 最終出力の境界に残す処理は、source mapと埋め込み文脈を含めて検証する。

削除候補の完全な未使用性、削除後の出力互換性、各懸念の実行時再現までは未検証。本書は調査時点の実装経路と仕様に基づく棚卸しであり、削除済み・安全確認済みを示すものではない。

## 実装反映（2026-09-13）

この棚卸しを受け、出力の空白・透明色・角度の修復と計算式最適化、登録初期値fallbackの削除を廃止した。空fallbackは定義側へ、ゼロの長さ型は宣言解決へ移動し、宣言合成・条件付きルール・selectorの展開も出力前の構造を使うように変更した。

現在の責務、削除したAPI、残る文字列ベースの意味処理、および比較テストの方針は [CSS出力の責務](../css-output-contract.md) を参照。この節より前の記述は調査時点の記録として残す。
