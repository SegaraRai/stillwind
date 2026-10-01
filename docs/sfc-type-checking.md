# SFC の TypeScript 型情報

Stillwind の checker は TypeScript 7.0.2 を使用する。Vue / Svelte / Astro の
converter は `@stillwind/projection` に隔離し、JavaScript SDK を必要とする処理には
同 package の TypeScript 6.0.3 を使う。Rust に TypeScript のバージョン依存はない。

## コンパイル経路と編集対象

1. host が原文を受け取り、必要な SFC を TS / TSX に投影する。
2. Rust が投影を正規化して scan する。Vue の挿入 receiver 等を除去した解析用
   buffer から、マクロ、型問い合わせ、意味依存 import を収集する。
3. scan の公開結果は原文位置に戻す。宣言 identity は生成時から原文位置を使う。
   checker に必要な箇所だけ、検証済みの投影範囲を追加で返す。
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
| checker の AST と shadow edit                                                | checker buffer の UTF-16 範囲                      |
| 診断の editor 位置                                                           | 原文 UTF-16 offset、およびゼロ始まり line / column |

Rust の AST 抽出中のみ正規化した解析 buffer の範囲が存在する。公開前に原文へ戻し、
対応を検証できない場合は投影や座標に対応する診断で拒否する。別モジュールの宣言 identity を
使用側の source map で再変換しない。生成シンボルの宣言位置も identity に従う。

## 型の伝播とエラー

`styleFlow` による独自の代入・関数フロー解析はない。TypeScript の推論と制御フローを
使い、style 合成、ref と boundary、keyframes、view transition の有限候補を取得する。

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
ユーザーが identity を偽造したり TS 診断抑制で意図的に契約を破る場合は保証対象外。

## Framework 固有の処理

- Vue: Volar の型付き投影を checker に渡す。Rust が取り除いた `__VLS_ctx.` の範囲を
  checker 問い合わせ時には含める。normal script と setup の import realm を保持する。
- Svelte: svelte2tsx の公式 shim / JSX 宣言を checker に含め、props、rune、store、each の
  型を使用する。DOM spread のために identity を消す adapter は不要。
- Astro: frontmatter と template は TSX。client script は元の script region ごとに
  独立した checker `.ts` buffer にする。converter が生成した関数内の client body は
  空白化し、位置を維持する。元 script と投影の対応は source map と本文一致で検証する。

## 依存関係

converter は遅延ロードする。`@stillwind/projection` の optional peers として、
利用する framework の組だけをインストールする。

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

比較実験は [checker comparison](../experiments/sfc-projection-research/checker-comparison.md)。
本番の結合テストは `packages/host/tests/sfc-types.test.ts` にあり、推移的 import、
Unicode 原文位置、編集後の snapshot、framework props / loops、各種 asset、型エラー、
Astro client script を検証する。
