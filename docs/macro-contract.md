# マクロ引数の契約

Stillwind のマクロは UI を記述するための宣言である。値を計算する式は同じ入力から同じ意味の値を返し、外部へ観測可能な変更を起こさない。
この条件はコンポーネント全体だけでなく、**マクロ内の各引数式**に適用する。

## フレームワークの入口と共通型

`style` は `stillwind/react`、`stillwind/preact`、`stillwind/solid`、`stillwind/vue`、
`stillwind/svelte`、`stillwind/astro` から取得する。各入口は `packages/core/src/frameworks/`
にあり、React は `className`、その他は `class` を返す。`style` 属性は Svelte と Astro では
文字列、React・Preact・Solid・Vue ではオブジェクトになる。

共通の引数型・宣言 identity・語彙は `stillwind/macro`（`packages/core/src/macro/`）が所有する。
生成された語彙ファイルはこのモジュールを拡張し、各フレームワークの型に共通で反映される。
コンパイル後の props 合成は `packages/core/src/runtime/` が担当する。

## 実行時の値式

`style()` の条件式、`class`、カスタムプロパティ、`aria-*` / `data-*` の値、合成元 style、
動的な identity の選択式、`transition.for(key)` の receiver と key は次を満たす必要がある。

- 入力は評価中に安定しており、式はほかの引数が参照するデータを変更しない。
- 呼び出す関数、constructor、getter、proxy、iterator、値変換にも同じ条件を適用する。
- 関数内で新しく作った一時データの変更は許可する。ほかの引数と共有するデータの変更は許可しない。
- 評価順・評価回数・結果に不要な式の実行には依存しない。`PURE` コメントは不要。
- `Date.now()`、`Math.random()`、通信、DOM 書き換え、状態更新は外で実行し、その結果を渡す。
- `await` / `yield`、dynamic import は外で評価し、その結果を渡す。
  省略される式が投げる例外の観測は保証しない。
- 関数名や class / JSX という構文自体に追加の規約はない。内部の値計算にも同じ純粋性契約を適用する。
- リアクティブ値の読み取りは現在の計算スコープ内で行う。マクロの値以外を目的とする購読や
  実行回数には依存しない。必要なら値の読み取りを同じスコープ内でマクロより前に記述する。

```ts
const props = style(isSelected(item) && "bg-accent", {
  class: buildClasses(state),
  "--width": calculateWidth(layout),
});
```

`buildClasses` 内部で新規配列に `push()` して返すことは許可する。
一方、次の引数間の通信は契約違反になる。

```ts
const classes = { active: true };
style({ class: classes }, { "data-value": (classes.active = false) });
```

非同期処理や状態更新が必要なら、順序を通常の JavaScript として指定する。

```ts
const value = await read();
updateState();
const props = style({ "--progress": value });
```

## 宣言と静的な構文

| マクロ／入力                                                            | 引数の契約                                                                                                                                                                         |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `style.ref(name?)`                                                      | 名前は省略または任意の文字列リテラル。空白は `_` にし、CSS 出力時に記号を escape                                                                                                   |
| `style.ref(boundary, name?)`                                            | boundary は静的に一つの ref identity に特定できること。名前は上の行と同じ制約                                                                                                      |
| `style.keyframes(definition)`                                           | 静的 object。frame は utility 文字列、variant key は完全置換の定義 object                                                                                                          |
| \x60keyframes(options)\x60                                              | options は文字列リテラル。receiver は有限の animation 候補。use は `style()` の class 入力としてのみ置ける                                                                         |
| `style.viewTransition(definition)`                                      | 静的 object。`group` は animation option、`old` / `new` / `imagePair` は utility 文字列リテラル。条件 variant は 4 bucket とも media / supports のみ可。値は `{v:"<class token>"}` |
| `transition.for(key?)`                                                  | receiver は有限の transition 候補。リテラル key はコンパイル時に畳み込み、式は実行時。名前は `serializeIdentifier(token + "-" + String(key))`                                      |
| `transition.for(key)`                                                   | transition は有限候補、key は純粋な `string` / `number` の値式                                                                                                                     |
| `ref.asAnchor(scope?)` / `.positionAnchor` / `style.anchorScope(scope)` | ref は有限候補。receiver は純粋な選択式                                                                                                                                            |
| `ref.asContainer(type?)`                                                | type は省略可能。CSS container-type の有限 keyword union の文字列リテラル                                                                                                          |
| `ref.at(query)` / ref.at タグ                                           | ref は有限候補。query は静的文字列で variant を返す                                                                                                                                |
| `ref()` / `ref("variant:")` / ref タグ                                  | prefix は静的。タグ内の補間は不可                                                                                                                                                  |
| `style()` の utility / variant key                                      | 静的な CSS 記述。utility テンプレートの補間は所定の ref variant のみ                                                                                                               |
| 配列・variant object                                                    | 対応する入力を再帰的に記述。literal spread と配列の穴に対応。getter・method は不可                                                                                                 |

`style()` の variant value には utility、ref projection、view transition use、
keyframes use と、それらの配列・条件分岐・variant object を置ける。
ref attachment、外部 `class`、inline custom property、attribute、style composition は
variant value に置けない。これらも `style()` の配列や条件分岐の中には置ける。
動的な view transition name を CSS variant 内で使う場合は、値を条件付きの
inline custom property に置き、`view-transition-name: var(...)` を variant の
CSS rule に出力する。詳細は [style input design](style-input-design.md) を参照。

コンパイル時に identity が確定すれば、receiver を実行時に読む必要がなくなる場合もある。
型定義にある `StillwindDeclarationIdentity` の追加引数は checker 用の内部情報であり、
ユーザーが副作用の実行場所として使う引数ではない。

入力の `class` は全 FW 共通。最終出力は React が `className`、ほかが `class`。
`style` 出力は Svelte / Astro が文字列、それ以外が object。入力に直接 `style` キーは指定しない。

## 最適化と診断

コンパイラはこの契約を前提に引数評価を並べ替え、空の条件分岐を除去し、配列を展開し、
隣接する `clsx` の変換を統合する。CSS の優先順位、クラスの連結順、重複プロパティの優先順位、
最終 props のキー順は維持する。値を持つ条件分岐は選択構造を維持し、別の実行スコープへ移動しない。

直接の代入・更新・delete は `MacroArgumentMutation` (`macro-argument-mutation`) の lint finding とする。
`await` / `yield` / dynamic import は `MacroExecutionBoundary` (`macro-execution-boundary`) とする。
要素にも宣言 identity にも紐づかない選択子（`[:host]:`、`[::view-transition-old(*)]:` など）は
`DocumentScopedSelector` (`require-element-scope`) の lint finding とする。その CSS は要素の有無に
かかわらず有効になるため、文書全体の規則は stylesheet に書く。
これらのルールの既定 severity は `deny`（error）。CSS の `@stillwind` による `warn` / `off` の設定と
`swlint-disable*` による局所抑制に対応する。lint finding があっても変換結果は保持し、
view transition はクラス（identity）でマークし、名前（pairing key）で参加する。名前の供給源が
無い call は `MissingViewTransitionName` (`require-view-transition-name`)、2つ以上ある call は
`DuplicateViewTransitionName` (`single-view-transition-name`) の lint finding とする。どちらも既定は
`warn` で、名前の値そのものは検査しない（別 call・stylesheet・ユーザーの inline style が見えないため）。
合成した base と自 call が別々の遷移クラスを宣言する場合は `SplitViewTransitionClasses`
(`single-view-transition-class`、既定 `warn`) とする。`view-transition-class` は単一プロパティなので
マージされず、derived のリストが base のリストを置き換える。
build host は有効 policy が `deny` の finding が残っていれば失敗とする。
抑制しても、契約に違反する副作用の順序・回数の保存を保証することにはならない。
構造の抽出や構文変換ができない場合は、引き続き抑制不可能なコンパイルエラーとする。
通常の関数呼び出しの本体や入力オブジェクト内部の実装までは証明しない。
診断がないことは純粋性の証明ではなく、ユーザーが契約を満たす責任を持つ。

API は `packages/core/src/macro/index.ts`、検証は `crates/js-oxc/src/contract.rs`、
出力形式の選択は [fragment folding](style-fragment-folding.md) を参照。

## Animation の条件分岐

```ts
style.keyframes({
  from: "opacity-0",
  to: "opacity-100",
  "motion-reduce:": { to: "opacity-100" },
});
```

分岐は完全置換で、通常定義の `from` は継承しない。入れ子は条件を積み重ね、同時成立時は
共通 variant 順の後の定義が有効になる。frame 内 utility の variant は使用できない。
定義の条件には、要素の selector を必要としない media / supports variant を使える。
`@container` は内側の `@keyframes` の有効性を制限しないため、定義分岐では拒否する。
詳しい根拠は [改善計画](design-intent-improvement-plan.md) を参照。
