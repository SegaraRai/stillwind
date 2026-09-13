# CSS出力の責務

Stillwindの互換性はCSSの意味で判定する。出力文字列の一致、空白、色の短縮表記、定数計算の短縮形は公開契約に含めない。minify・prettifyはバンドラーなどの最終出力側が担当する。

## 生成時に行う処理

- 組み込みCSSは値のトークン列を保持して読み込む。宣言の名前・値・区切りを出力するときに基本的な書式を決め、値全体の空白正規化はしない。
- transformの任意成分に必要な空fallbackは組み込み定義で指定する。任意値やユーザー定義の `var(--x)` へ一律にfallbackを追加しない。
- `zero-length` は値の型の指定として宣言解決時に適用する。非ゼロの計算式は最適化しない。
- 宣言結合ではカスケード順序と `compose-space` を扱う。条件付きルールは宣言境界に配置してから出力し、引用値のセミコロンやプロパティ名の部分一致を挿入位置に使わない。
- selectorテンプレートはCSSのnesting tokenだけを展開する。引用値・属性値・エスケープは保持する。
- variantのcaptureは明示的なplaceholderへ一度だけ展開する。`:root` や `0n` を暗黙のplaceholderとして修復しない。
- shadowの色を上書きする際は、パースした長さと色から生成する。再serializeしたshadowの末尾から色文字列を削除しない。

## 削除した出力後処理

生成済みCSSへのcalc簡約、fallbackの空白調整、gridのslash整形、空値の空白追加、透明色文字列の短縮、角度文字列の修復、登録済み初期値と同じfallbackの削除、および組み込み定義の空白minifyを削除した。

`@stillwind-format` の実効ルールは `zero-length` と `compose-space`。空fallbackは定義そのものに記述する。`@stillwind-format-functions` と組み込みbodyのcanonicalization APIは削除した。

## 最終組み立てと残る構造化対象

使用するtheme token・keyframes・property登録の選択、CSSレイヤーの配置、URLのホスト側確定、埋め込み文脈のエンコードは引き続き必要になる。これらはCSSの整形とは別の責務である。

名前空間prefixのCSS/JS全体置換、`@apply`/`@variant` の展開経路、supporting declarationの追加には既存の文字列ベースの経路が残る。今回それらを生成名や宣言の専用IRへ全面移行したわけではない。prefixは特にユーザー文字列との区別を含めた別の構造化対象である。

## 検証

宣言テストはテスト側で数式を評価し、トークンを比較する。引用文字列、トークン境界、空fallbackの有無、宣言順序は区別する。Tailwind fixtureとの比較ではテスト側のCSSパーサー・serializerも使用する。これらは保守的な比較であり、任意のCSSについて意味同値を証明する一般的な判定器ではない。比較器が扱えない差異を本体の文字列修復で埋めない。
