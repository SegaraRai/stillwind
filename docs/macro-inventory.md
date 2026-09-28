# マクロの棚卸し: JS への lower と生成 CSS

Status: 2026-09-17 時点の実装。根拠は `packages/core/src`（マクロ面とランタイム）、
`crates/js-oxc`（抽出と検証）、`crates/compiler`（lower と CSS 生成）、
`fixtures/projects/*/expected`（実際の生成結果）。引数の契約は
[マクロ引数の契約](macro-contract.md)、出力形式の選択は
[style fragment folding](style-fragment-folding.md)、CSS の並びは
[CSS ordering](css-ordering.md) を参照。

## マクロ面

ユーザーが import するマクロは `style` ひとつで、その値と引数の構文が下位のマクロを成す。

| framework | マクロ import      | class 属性  | 返る style の形 |
| --------- | ------------------ | ----------- | --------------- |
| React     | `stillwind/react`  | `className` | object          |
| Preact    | `stillwind/preact` | `class`     | object          |
| Solid     | `stillwind/solid`  | `class`     | object          |
| Vue       | `stillwind/vue`    | `class`     | object          |
| Svelte    | `stillwind/svelte` | `class`     | string          |
| Astro     | `stillwind/astro`  | `class`     | string          |

認識は import binding 単位で、`style` という名前をローカルで使っていても import 由来でなければ
マクロではない（`crates/js-oxc/src/visit.rs`）。マクロ呼び出しはすべてコンパイル時に値へ
置換され、実行時に生成される CSS はない。宣言マクロ（ref / keyframes / view transition）は
宣言箇所 identity を持ち、同じ宣言の再評価は同じ identity を指す。

## 一覧

| マクロ               | 構文                                                                                            | 表すもの                                              | JS への lower                                                                                              | 生成する CSS                                                                           |
| -------------------- | ----------------------------------------------------------------------------------------------- | ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| props                | `style(...inputs)`                                                                              | 要素の props 一式                                     | props object / `stillwindClsx` / `stillwindMerge`                                                          | utility class 規則（base unit → 条件付き unit）                                        |
| ref 宣言             | `style.ref()`, `style.ref("label")`, `style.ref(boundary)`, `style.ref(boundary, "label")`      | DOM identity と境界                                   | `{c:"<marker>"}`（実行時参照が無ければ `undefined`）                                                       | なし（marker は class token のみ）                                                     |
| keyframes 宣言       | `style.keyframes({...})`                                                                        | アニメーション定義                                    | `{k:"<name>"}`（実行時参照が無ければ `undefined`）                                                         | `@keyframes <name>{...}`（条件分岐は media/supports で包む）                           |
| keyframes use        | `<kf>("<options>")`                                                                             | animation の適用                                      | class token（静的）/ `({name:class})[value.k]`（動的）                                                     | `.<class>{animation-name:<name>;<options>}`                                            |
| view transition 宣言 | `style.viewTransition({...})`                                                                   | transition の定義                                     | `{v:"<class token>"}`（同左）                                                                              | `::view-transition-group \| old \| new \| image-pair(.<token>)`                        |
| view transition use  | `style(vt)`, `style(vt.for(key))`                                                               | 参加要素のマークと名前                                | class token / `{v:...}[value.v]` 表引き / `{v,w}` / `stillwindViewTransitionKey(...)`（宣言箇所の `.for`） | `.<class>{view-transition-class:...}`、literal key は `view-transition-name` も        |
| ref 射影             | `ref.anchor`, `ref.positionAnchor`, `ref.anchorScope`, `ref.container("size" \| "inline-size")` | anchor / container の宣言                             | `({"<marker>":"<class> <marker>"})[ref.c]`                                                                 | `anchor-name` / `position-anchor` / `anchor-scope` / `container-name`+`container-type` |
| ref variant          | `` ref`hover:` ``, `ref()`, `ref("hover:")`, `ref.at("width >= 40rem")`                         | selector / container query の wrapper                 | JS には現れない（utility 側の class に吸収）                                                               | marker を参照する selector、`@container <name> (query)`                                |
| 入力キー             | `{ class: … }`, `{ "--x": v }`, `{ "aria-x": v }`, `{ "hover:": … }`                            | 外部 class / custom property / 属性 / variant wrapper | `stillwindClsx` の引数 / style チャネル / そのままの prop / selector                                       | variant は utility 規則へ、他は CSS なし                                               |

## 1. `style(...)`: props マクロ

### 入力の種類ごとの lower

| 入力                                    | JS                                                                                                    | CSS                                                              |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| utility 文字列（`"p-4"` など）          | 生成 class token                                                                                      | `.sw_xxxxx{…}`（utility の宣言）                                 |
| テンプレートリテラル                    | ref variant の補間だけを静的に解決し、結果の utility として扱う（`` `${ref`hover:`}:text-accent` ``） | 補間された variant の規則                                        |
| 配列                                    | 要素を再帰的に lower（穴は `null` 等として保持）                                                      | 要素それぞれの規則                                               |
| 条件（`&&` / 三項）                     | 条件式をそのまま保持し、選択される class だけを枝に置く                                               | 条件付き class unit（base の後）                                 |
| variant object（`{ "hover:": … }`）     | 内側の class を同じ class 値へ畳む                                                                    | utility 規則に variant selector を付ける                         |
| `{ class: … }`                          | `stillwindClsx(…)` の引数（配列・真偽値 map・式）                                                     | なし                                                             |
| `{ "--x": v }`                          | style チャネル（object は `{…}`、Svelte/Astro は `"--x:"+…`）で値は `stillwindCssValue(v)`            | なし（inline style）                                             |
| `{ "aria-x": v }`                       | そのままの prop（`"aria-busy":loading`）                                                              | なし                                                             |
| composition base（`style(base, …)`）    | `stillwindToFragment(base)` を merge の先頭引数へ                                                     | base が持つ規則（並びは composition 順）                         |
| ref の添付（`style(ref)`）              | class 値に marker token を追加                                                                        | なし（marker は selector から参照される）                        |
| 動的 ref の添付（`style(a ? r1 : r2)`） | `(value).c` の読み                                                                                    | なし                                                             |
| ref 射影                                | `({"<marker>":"<class> <marker>"})[ref.c]` の表引き                                                   | `anchor-name` などの宣言を持つ class unit                        |
| keyframes use                           | class token、動的候補は `({"<name>":"<class>"})[value.k]`（value は use の callee）                   | `animation-name` と option の宣言                                |
| view transition use                     | class token、動的候補は `({token: class})[value.v]`                                                   | `view-transition-class`（literal key なら name も）              |
| `if(...)` を含む utility                | 変更なし（class は静的なまま）                                                                        | helper custom property と `@supports` gate、または native `if()` |

### 呼び出し全体の出力形

表現可能性に応じて 3 形態のどれかへ縮約される（`crates/compiler/src/compiler/fragment/render.rs`）。

1. 直接 props object: `{className:"sw_oqsqz",…}`。record 全体が表現可能なとき。
2. `stillwindClsx(…)` を class 値に持つ props object: 条件や外部 class があり、他の式が
   チャネルを分断しないとき。
3. `stillwindMerge(…)`: symbol key の fragment（`[stillwindClassKey]` / `[stillwindStyleKey]` /
   `[stillwindCustomPropertiesKey]`）を並べる一般形。composition base を含むときは
   `stillwindToFragment(base)` が先頭に入る。

`fixtures/projects/utility-props-matrix/expected/src/SaveButton.tsx`:

```tsx
export const buttonProps = {
  className: stillwindClsx(
    "sw_oqsqz",
    loading && "sw_tk9jb",
    emphasized ? "sw_zw6ja" : "sw_26ya8",
    "legacy-button",
    loading && "is-loading",
  ),
  "aria-busy": loading,
  "data-emphasis": emphasized ? "high" : "normal",
};
```

### CSS

- 無条件の utility は 1 つの base unit（`.sw_xxxxx`）へまとまり、条件付き utility は
  それぞれ自分の unit を持つ。base unit の規則が常に先に並ぶ。
- variant は unit 内で「無条件ブロック → canonical variant 順のブロック」に分かれる（L2）。
- Tailwind 互換の relationship marker（`group/menu`、`peer/field`）は authored の class token の
  まま出力に残り、参照側の variant がその marker を selector で指す。

```css
@media (hover: hover) {
  .sw_xmfe5:is(:where(.group\/menu):hover *) {
    color: var(--color-brand);
  }
}
```

## 2. `style.ref(...)`: ref 宣言

引数は `()`、`("label")`、`(boundary)`、`(boundary, "label")`。boundary は静的に 1 つの
ref identity へ解決できることが要件（`crates/js-oxc/src/refs/definitions.rs`）。

| 状況                                            | lower                                     |
| ----------------------------------------------- | ----------------------------------------- |
| 値が実行時に必要（動的添付・射影・export など） | `const trigger = {c:"sw_trigger_fnrq8"};` |
| 値が実行時に不要（静的添付だけ）                | `const menu = undefined;`                 |

marker は `sw_[label_]<hash>` の class token で、規則を持たない。要素の class 値に載り、
selector 側（ref variant・boundary の `@container`・`:has()`）から参照される。

## 3. `style.keyframes(...)` と use

宣言は `label` と frame（`from` / `to` / `%`）を持ち、frame の値は utility 文字列、
variant key の値は完全置換の入れ子 object（`crates/js-oxc/src/keyframes.rs`）。

| 状況             | lower                                    |
| ---------------- | ---------------------------------------- |
| 値が実行時に必要 | `const enter = {k:"sw-kf-enter-7nc28"};` |
| 不要             | `const enter = undefined;`               |

use は宣言値の呼び出し `enter("duration-normal ease-standard both")`。呼び出し自体は
コンパイル時に消え、動的候補では値の `.k` を読む。宣言値は呼び出し可能な値ではないため、
class 入力として消費されない use は `KeyframesUseNotAClassInput`（SHANIMATION002）で拒否する。

| 候補                           | lower                                                                              |
| ------------------------------ | ---------------------------------------------------------------------------------- |
| 静的（1 宣言）                 | class token。ガード無しなら base unit へ畳まれる                                   |
| 動的（条件で選ばれる複数宣言） | `({"sw-kf-enter-7nc28":"sw_s674c","sw-kf-tym79":"sw_35jbw"})[selectedAnimation.k]` |

CSS は宣言バンドに `@keyframes sw-kf-…` を 1 回だけ書き（use か export が要求したときのみ）、
use 側は `animation-name` と option の宣言を class unit に書く。条件分岐は frame を継承せず
完全置換で、条件の数だけ `@keyframes` を `@media` / `@supports` で包んで並べる。

```css
@keyframes sw-kf-enter-7nc28 {
  from { … }
  to { … }
}
.sw_s674c {
  animation-name: sw-kf-enter-7nc28;
  animation-duration: var(--duration-normal);
  animation-timing-function: var(--ease-standard);
  animation-fill-mode: both;
}
```

## 4. `style.viewTransition(...)` と `.for(key)`

宣言は `label` と `group` / `old` / `new` / `imagePair`（utility 文字列。media / supports
条件 variant のみ可）を持ち、少なくとも 1 つの phase を要求する
（`crates/js-oxc/src/view_transition.rs`）。

| 状況             | lower                                             |
| ---------------- | ------------------------------------------------- |
| 値が実行時に必要 | `const pageTitle = {v:"sw-vt-page-title-vtl83"};` |
| 不要             | `undefined`                                       |

`.for(key)` は key が literal か式か、使う場所が `style()` の中か宣言箇所かで lower が分かれる。

| 場所                         | key                    | lower                                    | CSS                                                                                                 |
| ---------------------------- | ---------------------- | ---------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `style(vt.for(…))`           | literal                | class token                              | `view-transition-class` + `view-transition-name`（`<token>-<key>` を CSS identifier として escape） |
| `style(vt.for(…))`           | 式                     | class token + inline                     | `view-transition-class` のみ。name は `stillwindViewTransitionName("<token>", key)`                 |
| 宣言箇所（binding 初期化子） | literal（候補が 1 つ） | `{v:"<token>",w:"outside"}`              | なし（値が name を運ぶ）                                                                            |
| 宣言箇所（binding 初期化子） | 式（または候補が複数） | `stillwindViewTransitionKey(value, key)` | なし                                                                                                |

use の lower は identity が静的か動的かでも分かれる。動的候補は `({token: class})[value.v]` の
表引き（`style(selectedTransition)`）、literal key 付きなら候補ごとの class に name を焼き込む。
ガード付き use は自分の class が slot custom property（`--sw-vtc-…`）を埋め、base unit の
`view-transition-class` が空 fallback 付きの `var()` で参照する。

```css
.sw_mo7p2 {
  view-transition-class: sw-vt-page-title-vtl83 var(--sw-vtc-sgapg,);
}
.sw_wiq0e {
  --sw-vtc-sgapg: sw-vt-woqm1;
}
```

宣言の CSS は `.for` か use が要求したときだけ、宣言バンドへ 1 回書かれる。

```css
::view-transition-old(.sw-vt-page-title-vtl83) {
  opacity: calc(0 * 1%);
}
::view-transition-new(.sw-vt-page-title-vtl83) {
  opacity: calc(100 * 1%);
}
```

名前の供給源が無い call は `MissingViewTransitionName`、2 つ以上は
`DuplicateViewTransitionName`（どちらも既定 warn）になる。

## 5. ref 射影

`.anchor` / `.positionAnchor` / `.anchorScope` は member 形、`.container("size" | "inline-size")` は
literal 引数の call 形。射影の CSS は対象の class と ref marker の組み合わせを選択する。
JS は射影 class と ref marker をそれぞれ class 値に載せる。

| 射影                 | class unit が宣言する CSS                    |
| -------------------- | -------------------------------------------- |
| `.anchor`            | `anchor-name:--sw-a-…`                       |
| `.positionAnchor`    | `position-anchor:--sw-a-…`                   |
| `.anchorScope`       | `anchor-scope:--sw-a-…`                      |
| `.container("size")` | `container-name:sw-c-…;container-type:size;` |

`fixtures/projects/bounded-ref-layout/expected/src/Popover.tsx`:

```tsx
<button
  {...{
    className: stillwindClsx("sw_bm2ay sw_trigger_fnrq8", trigger.c),
  }}
>
  Open
</button>
```

## 6. ref variant

tag 付きテンプレート（`` ref`hover:` ``）、引数無し call（`ref()`）、literal 1 引数 call
（`ref("hover:")`）が prefix を、`ref.at(query)` / `` ref.at`query` `` が container query を表す。
補間を含む tag は `NonStaticRefVariant` で拒否する。

- prefix は `prefix:` に正規化され、variant object のキーや utility 文字列の補間として使える。
- selector は ref と boundary の marker を `:where()` で包み、boundary 付きは
  `&:is(:where(<boundary>):has(:where(<ref>)<variant>) *)` の形で boundary の内側だけに限定する。
  specificity には対象側の selector と ref に適用した variant が寄与する。
- container query は `@container <container name> (query)` を生成する。

```css
@media (hover: hover) {
  .sw_vr124:is(:where(.sw_menu_qgtyh):has(:where(.sw_trigger_fnrq8):hover) *) {
    color: var(--color-accent);
  }
}
@container sw-c-trigger-fnrq8 (width >= 40rem) {
  .sw_vr124 {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
```

## 7. 付随する構文

- `if(--flag):utility` variant と `--if(--flag,a,b)` 値は class を変えず、computed-value time の
  条件へ lower する。fallback では helper custom property（`--sw-if-…`）と
  `@supports (x: revert-rule) and (--<hash>: 0)` gate、`useCssIf: true` では native
  `if(style(--flag): …; else: …)` を生成する（[runtime-if.md](runtime-if.md)）。
- custom property の値は object 系 framework で `style:{"--x":stillwindCssValue(v)}`、
  Svelte / Astro で `style:"--x:"+stillwindCssValue(v)` の連結になる。
- composition base は `stillwindToFragment(base)` を経て merge 先頭に入り、base の規則が
  自 call の規則より先に並ぶ（[style-composition.md](style-composition.md)）。
- `stillwind/module-css` の import はモジュール固有 CSS の値へ置換される（マクロではなく
  運搬用の入口）。

## 生成名の体系

ビルトイン生成の prefix。ホスト提供 generator が返す body には prefix を付けず、
正規化（brand type のコンストラクタ）だけを通す（[generated-identifiers.md](generated-identifiers.md)）。

| 種別                  | 既定の形                                                                                | プール                          |
| --------------------- | --------------------------------------------------------------------------------------- | ------------------------------- |
| utility class         | `sw_<hash>`                                                                             | class token                     |
| ref marker            | `sw_[label_]<hash>`                                                                     | class token                     |
| anchor 名             | `sw-a-[label-]<hash>`（`--` 付きで出力）                                                | anchor 名                       |
| container 名          | `sw-c-[label-]<hash>`                                                                   | container 名                    |
| keyframes 名          | `sw-kf-[label-]<hash>`                                                                  | `@keyframes` / `animation-name` |
| view transition class | `sw-vt-[label-]<hash>`（token。名前は `token-key`）                                     | view transition class           |
| 条件 helper           | `sw-if[-<role letter>]-<hash>`（`r`=rule、`g`=gate、`v`=value、`n`=not、無印=fallback） | custom property                 |
| VT slot               | `sw-vtc-<hash>`（`--` 付きで出力）                                                      | custom property                 |
| 匿名レイヤ            | `sw-anon-…`                                                                             | `@layer`                        |

## 検証フィクスチャ

`vp run fixtures:check` が入力と expected の一致を検証する。棚卸しの根拠に使った
プロジェクトは次の通り。

- `utility-props-matrix`: utility・配列・条件・属性・custom property・外部 class。
- `bounded-ref-layout`: ref marker、boundary、anchor 射影、container、ref variant。
- `typed-ref-graph`: import 越しの ref の添付と射影。
- `keyframes-matrix`: 静的 / 動的 keyframes use と条件分岐。
- `view-transition-matrix`: 静的 / 動的 identity、literal / 式 key、宣言箇所の `.for`、
  複数 transition の 1 call、ガード付き use。
- `runtime-if-fallback` / `runtime-if-native`: `if(...)` の 2 戦略。
- `tailwind-relationships`: `group` / `peer` marker の互換経路。
