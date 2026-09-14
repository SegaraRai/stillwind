# マクロ引数の契約

Stillwind のマクロは UI を記述するための宣言である。React の render に求める純粋性と同様に、
値を計算する式は同じ入力から同じ意味の値を返し、外部へ観測可能な変更を起こさない。
この条件はコンポーネント全体だけでなく、**マクロ内の各引数式**に適用する。

## 実行時の値式

`style()` の条件式、`class`、カスタムプロパティ、`aria-*` / `data-*` の値、合成元 style、
動的な identity の選択式、`transition.for(key)` の receiver と key は次を満たす必要がある。

- 入力は評価中に安定しており、式はほかの引数が参照するデータを変更しない。
- 呼び出す関数、constructor、getter、proxy、iterator、値変換にも同じ条件を適用する。
- 関数内で新しく作った一時データの変更は許可する。ほかの引数と共有するデータの変更は許可しない。
- 評価順・評価回数・結果に不要な式の実行には依存しない。`PURE` コメントは不要。
- `Date.now()`、`Math.random()`、通信、DOM 書き換え、状態更新は外で実行し、その結果を渡す。
- Hooks、`await` / `yield`、dynamic import、class 初期化、JSX は外で評価し、その結果を渡す。
  Suspense や意図的な例外を使う制御も外で処理する。省略される式が投げる例外の観測は保証しない。
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

| マクロ／入力                                | 引数の契約                                                                 |
| ------------------------------------------- | -------------------------------------------------------------------------- |
| `style.scope(name?)`                        | 名前は省略または空でない英数字・`_`・`-` の文字列リテラル                  |
| `style.ref(scope?, name?)`                  | scope は解決可能な scope identity。名前は scope と同じ制約                 |
| `style.animation(definition)`               | 静的 object。`from` / `to` / 百分率のキーと utility 文字列リテラル         |
| `animation(options)`                        | options は文字列リテラル。receiver は有限の animation 候補                 |
| `style.viewTransition(definition)`          | 静的 object。`group` / `old` / `new` の utility 文字列リテラル             |
| `transition.for(key)`                       | transition は有限候補、key は純粋な `string` / `number` の値式             |
| `ref.anchor` / `.position` / `.anchorScope` | ref は有限候補。receiver は純粋な選択式                                    |
| `ref.container(type)`                       | type は `"size"` / `"inline-size"` の文字列リテラル                        |
| `ref.at(query, utilities)`                  | 単一 ref、文字列リテラル2個                                                |
| `ref()` / `ref("variant:")` / ref タグ      | prefix は静的。タグ内の補間は不可                                          |
| `style()` の utility / variant key          | 静的な CSS 記述。utility テンプレートの補間は所定の ref variant のみ       |
| 配列・variant object                        | 対応する入力を再帰的に記述。DSL の spread・配列の穴・getter・method は不可 |

コンパイル時に identity が確定すれば、receiver を実行時に読む必要がなくなる場合もある。
型定義にある `StillwindDeclarationIdentity` の追加引数は checker 用の内部情報であり、
ユーザーが副作用の実行場所として使う引数ではない。

入力の `class` は全 FW 共通。最終出力は React が `className`、ほかが `class`。
`style` 出力は Svelte / Astro が文字列、それ以外が object。入力に直接 `style` キーは指定しない。

## 最適化と診断

コンパイラはこの契約を前提に引数評価を並べ替え、空の条件分岐を除去し、配列を展開し、
隣接する `clsx` の変換を統合する。CSS の優先順位、クラスの連結順、重複プロパティの優先順位、
最終 props のキー順は維持する。値を持つ条件分岐は選択構造を維持し、別の実行スコープへ移動しない。

直接の代入・更新・delete は `MacroArgumentMutation` (`SHMACRO001`) として拒否する。
Hooks の慣用名 `use` / `useXxx` の呼び出しと明示的な実行境界は `MacroExecutionBoundary`
(`SHMACRO002`) として拒否する。これらはコンパイルエラーで、lint suppression では抑制できない。
通常の関数呼び出しの本体、Hook の別名、入力オブジェクト内部の実装までは証明しない。
診断がないことは純粋性の証明ではなく、ユーザーが契約を満たす責任を持つ。

API は `packages/core/src/index.ts`、検証は `crates/js-oxc/src/contract.rs`、
出力形式の選択は [fragment folding](style-fragment-folding.md) を参照。
