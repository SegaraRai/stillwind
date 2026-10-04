# SFC の TypeScript 型情報

Stillwind の checker は TypeScript 7.0.2 を使用する。Vue / Svelte / Astro の
converter は `@stillwind/projection` に隔離し、JavaScript SDK を必要とする処理には
同 package の TypeScript 6.0.3 を使う。Rust に TypeScript のバージョン依存はない。

## コンパイル経路と編集対象

1. host が原文を受け取り、必要な SFC を TS / TSX に投影する。
2. Rust が投影を正規化して scan する。Vue の挿入 receiver 等を除去した解析用
   buffer から、マクロ、型問い合わせ、意味依存 import を収集する。
3. scan の公開結果は原文位置に戻す。宣言 identity は生成時から原文位置を使う。
   checker に必要な式、producer call、module specifier には、それぞれの AST role を
   検証した投影範囲を追加で返す。同じ原文の宣言が投影内に複製された場合も、
   identity は一つにまとめ、checker には各コピーの call 範囲を渡す。
4. host が意味依存をたどり、必要な TS / TSX / SFC、barrel、helper を準備する。
   ファイルの読み込み・非同期 converter は checker を呼ぶ前に行う。
5. checker 専用 buffer に宣言 identity の隠し引数と解決済み import を挿入する。
   TypeScript は正規化前の framework 投影を読むため、props、loop、store 等の
   framework 型環境を維持する。JS 入力には JSDoc 型 assertion を使う。
6. TS7 が問い合わせ範囲の型を返す。identity 関連診断を検査し、有限候補を取り出し、
   原文の式範囲に対応する type fact として Rust の finalize に渡す。
7. Rust が原文に対するマクロ置換と CSS を生成する。出力対象は `.vue` / `.svelte` /
   `.astro` のままなので、通常の framework compiler がこの結果を処理する。

1・2・5 の変換は解析用 buffer の生成であり、ユーザーのファイルは保存・編集しない。
実際の出力編集は 7。checker の隠し引数や phantom property は runtime に出力しない。

## 位置の契約

| データ                                                                       | 座標                                               |
| ---------------------------------------------------------------------------- | -------------------------------------------------- |
| identity、type probe、type fact、module specifier、公開 IR、診断の byte 範囲 | 原文 UTF-8 byte、半開区間                          |
| converter map、script realm、編集 region                                     | 原文／投影の UTF-16 code unit、半開区間            |
| native `checkerSpans`                                                        | 原文 UTF-8 範囲と対応する投影 UTF-16 範囲          |
| native `checkerBindings`                                                     | converter が生成した helper 参照の投影 UTF-16 範囲 |
| checker の AST と shadow edit                                                | checker buffer の UTF-16 範囲                      |
| 診断の editor 位置                                                           | 原文 UTF-16 offset、およびゼロ始まり line / column |

Rust の AST 抽出中のみ正規化した解析 buffer の範囲が存在する。公開前に原文へ戻し、
対応を検証できない場合は投影や座標に対応する診断で拒否する。別モジュールの宣言 identity を
使用側の source map で再変換しない。生成シンボルの宣言位置も identity に従う。

## 型の伝播とエラー

`styleFlow` による独自の代入・関数フロー解析はない。TypeScript の推論と制御フローを
使い、style 合成、ref と使用箇所の scope operand、keyframes、view transition の有限候補を取得する。

```ts
const a = style("flex");
const b = style("grid");
let selected: typeof a | typeof b = a;
if (flag) selected = b;
const result = style(selected, "items-center");

function identity<T>(value: T): T {
  return value;
}
const other = style(identity(flag ? a : b));
```

`let selected = a` のまま `b` を代入すると、隠し identity により型が非互換になる。
該当 checker 診断は `SHTS002` として原文位置で報告する。`any`、`unknown`、
`StyleResult<string>`、identity を消す関数引数等で必要な候補が求まらなければ
`SHTS003`、または native の未解決値診断で拒否する。型情報を失った候補を選択して
成功扱いにしない。通常のユーティリティ条件式は引き続き Rust の構造的 lowering を使う。
任意の TS 診断をすべて報告する型検査ツールとしては動作しない。

`StyleResult` の identity は `data-__stillwindStyleIdentity` という readonly の型専用
文字列 property。Svelte の attachment 用 symbol index signature と共存する。
ref、keyframes、view transition のブランドは symbol のまま保持する。
checker は登録済み producer の core module から正規の unique-symbol property を取得する。
型 alias が消えた場合もブランドから有限 identity を取り出し、登録済みの同じ asset 種別かを検証する。
ref と keyframes は callable な宣言型、view transition は callable な `for` を持つ宣言型を対象とする。
マクロの使用結果や variant は宣言候補として扱わない。
ユーザーが identity を偽造したり TS 診断抑制で意図的に契約を破る場合は保証対象外。

## Framework 固有の処理

- Vue: Volar の型付き投影を checker に渡す。Rust が取り除いた `__VLS_ctx.` の範囲を
  checker 問い合わせ時には含める。挿入された `.value`、unwrap、shorthand の展開、
  型名の修飾、constructor の括弧も、原文との対応を検証して正規化する。
  生成された宣言と scope は保持し、exposure、completion、narrowing が追加した
  import reference だけを除外する。normal script と setup の import realm を保持する。
  生成 helper の callee は checker 専用の import に置換する。名前は投影内の識別子と
  衝突しないものを選び、独立した仮想 module で global helper の型を取得する。
  ユーザーが書いた同名の関数や `globalThis` はこの型取得に影響しない。
- Svelte: svelte2tsx の公式 shim / JSX 宣言を checker に含め、props、rune、store、each の
  型を使用する。生成コードは TS として解析し、generic arrow function を保持する。
  生成された `__sveltets_2_*` と `__sveltets_$$*` の runtime 参照は、原文の参照との
  対応がないことを検証してから、Vue と同じ checker 用 import に置換する。
  ユーザーが書いた同名の参照と、変換器が生成したローカル関数は元の binding を使う。
  DOM spread のために identity を消す adapter は不要。
  slot prop にマクロ呼び出しを直接書くと、svelte2tsx が型宣言用に複製した呼び出しに
  source map を付けないため、投影検証で拒否する。script で style を宣言してから slot に渡す。
- Astro: frontmatter と template は TSX。client script は元の script region ごとに
  独立した checker `.ts` buffer にする。converter が生成した関数内の client body は
  空白化し、位置を維持する。元 script と投影の対応は source map と本文一致で検証する。
  TSX 変換には Go/WASM 版の `@astrojs/compiler` を使う。
  client script の generic arrow function は `<T,>` のように TSX と区別できる形で書く。
  `<T>` は converter が TSX にそのままコピーするため、Rust の投影検証で拒否する。

## 依存関係

converter は遅延ロードする。`@stillwind/projection` の optional peers として、
利用する framework の組だけをインストールする。
`@vue/language-core` の対応範囲は `>=3.0.2 <4.0.0`。この番号は Vue runtime の版ではない。
converter が global 型宣言を返す場合は、それを checker の仮想 `.d.ts` に含める。

Vue の変換形式は、host の dev dependency に固定した以下の版で検証する。
alias は `vue-language-core-3-0-2` のように版番号を含める。
[vue.test.ts](../packages/host/tests/projection/vue.test.ts) は
各版の実際の投影を Rust の scan、TS7 の型問い合わせ、コンパイルに通す。
TS / JS の式、原文の構文と座標、有限 identity、生成された参照とユーザーの参照を検証する。

| 固定した版 | 検証する変換形式                                                                         |
| ---------- | ---------------------------------------------------------------------------------------- |
| 3.0.2      | `__VLS_self` を生成する `setup()` の return に binding を公開する。                      |
| 3.0.6      | `__VLS_self` を生成する `setup` の arrow function で binding を公開する。                |
| 3.1.0      | `__VLS_Bindings` と `__VLS_ProxyRefs` を使う。                                           |
| 3.1.6      | `__VLS_SetupExposed` と `__VLS_ProxyRefs` を使う。                                       |
| 3.2.0      | `__VLS_SetupExposed` と `ShallowUnwrapRef` を使い、helper 型を外部ファイルから参照する。 |
| 3.3.12     | 直接参照、`.value`、`__VLS_unwrap` と `__VLS_withDotValue` を使う。                      |

Svelte の対応範囲は `svelte ^5.0.0` と `svelte2tsx ^0.7.56`。
[svelte.test.ts](../packages/host/tests/projection/svelte.test.ts) は
0.7.56 と 0.7.61 の変換器と、その版の公式型宣言を使用する。
版を含む alias は projection の dev dependency に固定し、TS6 SDK で実行する。
Rust の scan、TS7 の型問い合わせ、原文の編集、Svelte compiler による検証を通す。
宣言タグ、attachment、await の型注釈、nullable な each、snippet、module script、
生成 helper の名前衝突と、Unicode / CRLF の原文座標を検証する。

Astro の対応範囲は `@astrojs/compiler ^4.0.0`。
[astro.test.ts](../packages/host/tests/projection/astro.test.ts) は
host の dev dependency に固定した 4.0.0 を使用し、Rust の scan、TS7 の型問い合わせ、
原文の編集、Astro compiler による検証を通す。
frontmatter、fragment、client script の各形式、server / client のスコープ分離、
Unicode / CRLF の原文座標と型エラーを検証する。JSON、外部 script、空の script も含める。

| Framework | optional peers                                                 |
| --------- | -------------------------------------------------------------- |
| Vue       | `@vue/compiler-dom`, `@vue/compiler-sfc`, `@vue/language-core` |
| Svelte    | `svelte`, `svelte2tsx`                                         |
| Astro     | `@astrojs/compiler`                                            |

実験で使用した正確な版は比較結果 JSON に記録する。peer range は互換リリースを許容する。Vue / Astro の実行環境と宣言は利用プロジェクトに
インストールされている framework package を使う。TS6 SDK は projection の通常依存、
TS7 checker は host の通常依存で、同じ `typescript` インスタンスに統一しない。

TS6 一本化には従来の同期 CompilerHost API を使え、SDK 系統を一つにできる利点がある。
一方で TS7 でも型・AST・診断・仮想 filesystem・増分 snapshot が動作し、SFC 固有の
修正は両版で共通だった。現在の TS7 checker を TS6 へ移植する利点が確認できないため
TS7 を維持する。converter が TS7 SDK を扱えるようになれば projection 側を個別に移行できる。

現在のchecker統合は [type-facts.ts](../packages/host/src/type-facts.ts)、SFCの検証は [sfc-types.test.ts](../packages/host/tests/sfc-types.test.ts)。
本番の結合テストは `packages/host/tests/sfc-types.test.ts` にあり、推移的 import、
Unicode 原文位置、編集後の snapshot、framework props / loops、各種 asset、型エラー、
Astro client script を検証する。
