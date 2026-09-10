# SFC projection research

実装判断: Astro は `@astrojs/compiler.convertToTSX`、Svelte は `svelte2tsx` が有望。Vue は `@vue/language-core` の script 投影を利用できるが、template を同じ無加工の OXC マクロ解析に流すことはできない。

## 再実行

```powershell
cd F:\Projects\shorth\experiments\sfc-projection-research
vp i --frozen-lockfile
vp run verify
```

独立した pnpm workspace。タスクは vite.config.ts に定義。結果は results/research.json、実際の生成コードと map は results/outputs、置換確認は results/rewrite-verification.json。27 fixtures、11 converter 設定を実行。日本語・絵文字、CRLF、複数行、型引数、import alias、同一式の重複、template ローカル変数、複数 script、module-css import を含む。

## 実測

各言語 9 fixtures / 28 import・call span。成功数は「生成 AST の開始位置が期待する SFC の位置に戻り、生成 span のテキストが元 SFC と完全一致し、call は Identifier を callee に持つ」の件数。実際の import binding の解決までは検証していない。

| converter | version | map | 成功 span |
| --- | --- | --- | --- |
| @astrojs/compiler convertToTSX | 4.0.0 | v3 | 28/28 |
| 同上 includeScripts:false | 4.0.0 | v3 | 26/28 |
| @astrojs/compiler transform | 4.0.0 | v3 | 19/28 |
| @astrojs/compiler-rs convertToTSX | 0.4.0 | — | 全9件で未実装エラー |
| @astrojs/compiler-rs transform | 0.4.0 | v3 | 8/28 |
| svelte2tsx | 0.7.61 | v3 | 28/28 |
| svelte/compiler client / server | 5.57.0 | v3 | 23/28・24/28 |
| @vue/language-core | 3.3.11 | Volar range mappings | 21/28 |
| @vue/compiler-sfc compileScript | 3.5.42 | v3 | 21/28 |
| 同上 inlineTemplate:true | 3.5.42 | v3 | 21/28 |

全ての変換成功ファイルは oxc-parser 0.149.0 で構文エラーなし。TypeScript 6.0.3 を使用。これは OXC の JS binding での構文確認で、Stillwind Rust 本体の受け入れテストではない。

## map の品質と採用時の注意

- Astro TSX は開始位置と内容が28件とも一致。CRLF の複数行 import では内部の全境界が厳密に写るわけではないため、文字ごとの完全 map を前提にしない。includeScripts:false は client script の2 probe を失う。client/server の意味上のスコープと import binding は別途確認が必要。
- Svelte は全28 spanの内部 Unicode scalar 境界が一致。ただし排他的 end をそのまま map すると import 10件でずれる。`originalStart + generatedText.length` で範囲を作り、元文字列との完全一致で検証する方針と合う。最終文字の map は28件とも一致した。
- Vue language-core の script 系21件は成功。template 7件は `style('x')` が `__VLS_ctx.style('x')` に変わる。map の精度だけではマクロ import と直接 call の対応を取り戻せない。単に prefix を文字列置換すると shadowing やローカル変数を壊し得る。
- Vue の map は sourceOffsets/generatedOffsets/lengths 等の Volar データであり、v3 map ではない。今回 @volar/source-map 2.4.28 の toSourceLocation/toSourceRange で調査した。採用するなら host 側で範囲情報を橋渡しするか、未マップ区間と境界を保持する v3 変換が必要。その変換自体は今回未実装。
- runtime compiler は型削除・式の変形・整形や疎な mapping を含む。デバッガ向け map と、文字列一致を要求する編集用 map の適性は異なる。

## 推奨する境界

host が SFC を projection JS/TSX と map に変換し、Rust は OXC によるマクロ解析と元 SFC への span 編集を担当する。開始位置を map し、元 SFC の該当部分と投影テキストを完全一致で検証、不一致・未マップ・曖昧な位置は拒否する。同一テキスト検索で位置を推測しない。

JS の offset と source-map column は UTF-16、Rust OXC の span は UTF-8 byte offset。この境界を明示的に変換する必要がある。今回の JS binding 実験だけで Rust 側の変換正しさを証明したことにはならない。

Vue template まで対応する場合、host に macro binding を保つ投影処理が追加で必要。Rust に Vue 固有の `__VLS_ctx` を教えるより、host の Vue adapter で対処する設計が元の責務分離に合う。ただし、その処理の正当性と保守負担は追加検証事項。

import span は元 SFC の挿入・置換アンカーとして利用可能。module-css import を module 固有の virtual JS に差し替える案とも整合する。ただし、この実験は runtime/CSS 出力や自動 side-effect import 抑制の実装ではない。

## 置換確認の範囲

verify.mjs は map で対応できた77 spanを元 SFC 上で編集し、27 SFC 全てを再変換・OXC再パースして成功を確認する。import の module を virtual ID に置き換え、call は `{}` に置き換える。Vue template 7件は明示的に拒否する assertion がある。これは span 編集の構文確認であり、マクロ意味解析・型検査・実行時動作・任意の不正入力への保証ではない。fixture の正解位置を使った比較なので、一般入力用 mapper の完成実装でもない。

## 一次資料

- [Astro compiler](https://github.com/withastro/compiler)
- [Astro compiler-rs](https://github.com/withastro/compiler-rs)
- [svelte2tsx](https://github.com/sveltejs/language-tools/tree/master/packages/svelte2tsx)
- [Vue language tools](https://github.com/vuejs/language-tools)

上記に加え、固定した npm package の実際の公開 API と出力を確認した。最新一般の保証ではなく、表のバージョンと同梱 fixture に対する結果。
