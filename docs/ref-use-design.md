# ref の明示的な provider use と class unit の設計

## 状態と適用範囲

`ref.asAnchor()` / `ref.asContainer()` を採用する設計判断を記述する。
これらの API は実装されている。使用例は公開 API を表す。
「確定事項」は API 契約、「lowering の方向」は共通処理の構造を表す。

ref attachment から `anchor-name` / `container-name` / `anchor-scope` を自動生成する案は採用しない。
provider 側の CSS は、その要素の `style()` に書かれた明示的な use から生成する。
現行の入力構造と出力経路は [style input design](style-input-design.md) を参照する。
marker の付与は attachment の責務として独立している。

## 確定事項

### 宣言・attachment・use

ref identity の宣言は `style.ref()` に統一する。`style.anchor()` や
`style.container()` という別の ref 宣言ファクトリーは導入しない。

| 操作                                         | 意味                                                               | marker の追加 |
| -------------------------------------------- | ------------------------------------------------------------------ | ------------- |
| `style.ref(...)`                             | ref identity と、指定されていれば bounding ref との関係を宣言する  | なし          |
| `style(ref)`                                 | 対象要素へ ref を attach する                                      | あり          |
| `ref.asAnchor(...)` を `style()` に渡す      | receiver のアンカー名を供給する CSS use を作る。scope の詳細は後述 | なし          |
| `ref.asContainer(...)` を `style()` に渡す   | receiver のコンテナー名と型設定を供給する CSS use を作る           | なし          |
| `style.anchorScope(...)` を `style()` に渡す | 対象要素に scope だけを供給する CSS use を作る                     | なし          |
| `ref.positionAnchor()` を `style()` に渡す   | receiver のアンカーを参照する CSS use                              | なし          |
| `ref.at(query)`                              | receiver のコンテナー名を参照する variant                          | なし          |

`positionAnchor()` は引数なしのメソッドで、consumer use を生成する。
`asXXX` は新しい ref identity を作らず、receiver の identity から生成名を得る。
`asContainer` の引数に同じ ref を重ねて渡す必要はない。

marker は ref をそのまま `style()` の入力にした attachment だけが追加する。
各 ref の marker は一つの対象要素を指す、という利用上の設計意図を維持する。
この意図は、DOM 全体の一意性をコンパイラが静的に証明・検証できるという保証を含まない。

### 想定する使用形

```ts
style(ref, ref.asAnchor());
style(ref, ref.asContainer());
style(ref, ref.asAnchor([scopeRef1, scopeRef2]), ref.asContainer("inline-size"));
style(ref, ref.asAnchor("all"), ref.asContainer("size scroll-state"));
```

`asAnchor()` は anchor-name だけを出力し、anchor-scope を省略する。
`asContainer()` は container-name だけを出力し、container-type を省略する。
後者は他の指定がなければ CSS 初期値の `normal` になる。省略によって他の CSS の値をリセットしない。

`asAnchor([a, b])` は receiver の anchor-name と、a・b の生成アンカー名を列挙した
anchor-scope を出力する。receiver 自身を scope に含めるのは配列に明示された場合だけである。
`asAnchor("all")` / `asAnchor("none")` は各値を明示出力する。

scope だけを供給する場合は次の形を使う。対象要素がアンカーである必要はない。

```ts
style(style.anchorScope([a, b]));
style(style.anchorScope("all"));
style(style.anchorScope("none"));
style("[anchor-scope:all]");
```

両 scope API とも、配列中の ref に有限の動的候補があれば全候補の名前を含める。
実行時に選ばれた ref 一つだけへ scope を絞らない。動的 receiver の選択は既存の
オブジェクトマッピングで解決し、scope の候補集合とは独立に扱う。
空配列と重複は Lint で警告する。任意長の実行時配列まで受け付けるという意味ではない。

attachment と CSS use は独立した条件で有効化できる。

```ts
style(
  attach && ref,
  enableAnchor && ref.asAnchor("all"),
  enableContainer && ref.asContainer("size"),
);
```

各条件が制御するのは、それぞれの marker または class unit の付与である。
`asXXX` の条件成立から marker の付与を補わない。
provider use は `.class_unit:where(.marker)` のように receiver の marker を要求する。
attachment の条件が偽なら、provider の class unit が付いても当該 CSS は適用されない。

```ts
style(cond ? ref1 : ref2, ref1.asContainer("size"), ref2.asContainer("inline-size"));
```

この形では、実際に付いた marker に対応する provider のみ有効になる。
gate は既存の selector 表現で扱え、追加の runtime 条件処理は必要ない。
既存 utility との出力順は保証せず、同じプロパティへの供給が衝突する場合は Lint で警告する。
consumer の `positionAnchor` と、receiver を持たない `style.anchorScope` にはこの gate を付けない。
scope 配列の a・b は範囲内のアンカーを指定するもので、scope 要素の marker を指定するものではない。

同じ要素で複数の provider use が同じプロパティを供給する利用は未定義とし、避けることを要求する。
値集合の自動合成は行わず、検出可能な場合は Lint の警告対象にする。
上記のように gate で排他的に適用される provider は、同時供給の競合には当たらない。

### 出力の所有者

`asAnchor` / `asContainer` の CSS は使用箇所の class unit に属する。
これらの use から `.marker { anchor-name: ... }` などの marker 単独ルールを生成しない。
名前や scope を供給する use と、position-anchor などの参照する use は、いずれも
keyframes use / view transition use と同系統の class unit lowering に載せる。

provider の明示設定は、その provider を読む consumer の有無や数に依存せず出力できる。
参照されていない provider 設定を consumer 不在だけを理由に除去する設計にはしない。
通常の到達不能な入力の除去などは、マクロの純粋性契約と既存の最適化規約に従う。

## 選択の理由

### 明示設定にする理由

`anchor-name` と `container-name` は、ref identity と命名設定から名前を決められる。
同じ ref を使う consumer がそれぞれその marker 向けルールを出しても、
各名前空間における同じ ref の生成名は一致させられる。

`anchor-scope` は、どのアンカー名を一つの範囲へ閉じるかという集合を必要とする。
異なる consumer が必要とする名前は一致するとは限らず、次のルールは集合の和にならない。

```css
.scope-marker {
  anchor-scope: --first;
}
.scope-marker {
  anchor-scope: --second;
}
```

通常のカスケードでは後の値が前の値を上書きする。
両方を指定するには `anchor-scope: --first, --second` という一つの値が必要になる。

自動生成を行うなら、dependent を把握して provider の集合を組み立てる方式か、
consumer の要求を構造化して最終 stylesheet で集合化する方式が必要になる。
後者では reverse traversal 自体を必須にしなくてもよいが、分割出力、code splitting、
モジュールの追加・削除を含め、同時に適用される CSS 全体の協調が残る。
anchor-scope のためにこの協調を導入する費用を避け、provider の出力を局所的に確定できる方式を選ぶ。

明示方式は、モジュール単位で出力を理解しやすくする一方、設定の記述量、指定忘れ、
未使用設定の残存を増やす。この代償を含めた選択であり、自動生成による補完は行わない。

`anchor-scope: all` は名前の列挙を省けるが、閉じる範囲が広く、境界自身のアンカー名や
外部との参照関係にも影響し得る。自動 scope の代用として一律に付ける根拠にはしない。
名前の定義と lookup の範囲は [CSS Anchor Positioning の anchor-scope 定義](https://drafts.csswg.org/css-anchor-position/#anchor-scope)
に従う。特定のグローバル名との参照が常に維持される、という保証はこの API に追加しない。

### ref の asXXX メソッドにする理由

現行の `style` の名前付きメンバーは `ref` / `keyframes` / `viewTransition` であり、
いずれも宣言を作る。use は宣言値自身から作る形になっている。
`keyframes(options)`、`transition.for(key)`、ref の projection と同じ位置に
`ref.asAnchor(...)` / `ref.asContainer(...)` を置く。

`as` は receiver に対応する要素側の CSS 上の役割を表す。
`positionAnchor` のように receiver を参照先として使う操作との区別ができ、
ref の補完候補から provider の設定を見つけられる。

| 検討した形                                                 | 採用しない理由                                                                 |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `style.anchor()` / `style.container()` という ref 宣言     | ref identity を宣言する入口を分けるため                                        |
| `style.anchor(ref)` / `style.container(ref)` という use    | receiver の役割を設定する操作を、ref 自身のメソッドに揃えるため                |
| `ref.withAnchor(...).withContainer(...)` という attachment | marker と CSS 設定の条件を結びつけ、条件の組合せと戻り値の意味を複雑にするため |

明示的な use を並べる方式なら、同じ条件にまとめる場合も、別の条件に分ける場合も、
既存の `style()` の配列・条件分岐で表現できる。

`style.anchorScope` は複数 ref の名前集合を扱い、特定の receiver の役割を設定しない。
そのため `style` 上に独立した use 生成 API を置く。名前付きメンバーがすべて宣言ファクトリーである、
という現行の傾向は新 API 全体の制約にはしない。

## 共通 lowering と最適化の方向

### 候補選択から class unit まで

各 use は概念上、一つの class unit を供給する。動的 receiver が有限候補を持つ場合は、
候補ごとの CSS を事前生成し、実行時には unit を選ぶ。

```js
({ marker1: "unit1", marker2: "unit2" })[ref.c];
```

これは生成コードの説明用表記である。`ref.c` はコンパイラ内部の表現であり公開 API ではない。
lookup のキーとして marker を読むことは、選んだ要素へその marker を追加することを意味しない。

共通処理の順序は次の形を目指す。

1. 各 use の候補 identity、CSS 寄与、CSS variant、JavaScript の condition、runtime payload を保持する。
2. 候補が一種類なら、CSS 選択を静的な寄与へ縮約する。
3. 同一 `style()`・同一 condition の静的寄与を、順序を維持できる region 内でまとめる。
4. 最終的な class unit の名前を割り当てる。動的選択が残る箇所には候補 unit の lookup を出す。

クラス名の割り当て前に縮約・集約できれば、物理的に生成したクラスを後から統合する必要はない。
静的寄与は `LiteralContribution`、動的候補の割り当ては `emit_candidate_units` が扱う。

class unit は `style()` 宣言単位で構築し、異なる `style()` 間で同じ CSS 規則を共有することを
前提にしない。base を guarded region より前に出す規約と、region 間のカスケード順を維持する。
同じ条件式の文字列というだけでは離れた region をまとめない。
間にある条件分岐や動的 unit に対する優先順位、CSS variant の適用先も保存する。
ただし既存 utility と provider use の衝突時の勝敗は保証対象に含めない。

### 共通化する部分と固有の意味論

候補 identity の解決、単一候補への縮約、静的寄与の集約、候補 unit の割り当て、lookup の生成を
ref use / keyframes use / view transition use の共通経路にする。
使用箇所の source span、必要な宣言、生成名の symbol、runtime payload はその過程で保持する。

各 use の固有の意味論は、共通処理に渡す寄与の構築と合成規則に残す。

| use             | 保持する固有の意味論                                                                            |
| --------------- | ----------------------------------------------------------------------------------------------- |
| ref             | provider と consumer の CSS プロパティ、明示 scope、ref と boundary の相関、attachment との分離 |
| keyframes       | keyframes 定義への依存、animation options からの宣言生成                                        |
| view transition | 複数 class token の合成、参加者の name、動的 key、variant 内の動的 name の slot、名前の診断     |

view transition の候補が一つでも key が動的なら、CSS 選択だけを縮約し、
参加者の name に必要な runtime 値は残す。複数の条件が同時に成立した場合の
view-transition-class の合成も保持する。

`view-transition-name` を直接 inline style に出す最適化を維持する。
他の use でも、custom property が単純な値の中継だけを担う場合は直接 inline 出力する。
gate、CSS variant、token 合成などに必要な経由は残し、共通 lowering は CSS と inline payload の両方を扱う。

## 実装経路

- [ref parser](../crates/js-oxc/src/refs/projections.rs) が provider use、scope-only use、有限候補の scope 集合を解析する。
- [ref emitter](../crates/compiler/src/compiler/emission/ref_projection.rs) が provider gate と CSS 宣言を生成する。
- [class lowering](../crates/compiler/src/compiler/emission/class_value.rs) の `contribute_rule` / `emit_candidate_units` が ref・keyframes・view transition の静的寄与と候補 lookup を扱う。
- [LiteralContribution](../crates/compiler/src/compiler/literal.rs) は複数 transition token と name を region 内で保持する。
- [ClassUnits](../crates/compiler/src/compiler/class_units.rs) は条件付き token の合成と CSS variant 内の slot を扱う。
- [provider Lint](../crates/compiler/src/compiler/emission/provider_lints.rs) が供給プロパティと条件・gate を比較し、衝突する二箇所を示す。
- [ref validation](../crates/js-oxc/src/refs/validation.rs) が attachment と provider の欠落を区別する。

## 旧 API からの移行

| 現行 API                               | 移行先・扱い                                                                        |
| -------------------------------------- | ----------------------------------------------------------------------------------- |
| `style.ref(...)` / `style(ref)`        | 宣言と attachment の入口として維持                                                  |
| `ref.anchor`                           | `ref.asAnchor()` へ置換。anchor-name だけを供給し、marker attachment は別途明示する |
| `ref.container(type)`                  | `ref.asContainer(type)` へ置換。use は marker を追加しない                          |
| `ref.anchorScope`                      | `style.anchorScope([ref])` へ置換。scope 要素に参照先 marker を追加しない           |
| `ref.positionAnchor()`                 | 引数なしのメソッドとして consumer use を生成し、class unit の経路で出力する         |
| `ref.at(query)` / ref selector variant | 参照先の名前・marker を使う役割を維持。provider の設定は明示 use が担う             |

`ref.anchorName` は現行 API に存在せず、削除対象の名前として扱わない。
旧 projection が暗黙に marker を付けていた箇所は、対象が本当に ref 要素である場合にのみ
`style(ref, ...)` へ移す。position-anchor を使う要素へ ref attachment を補う移行は行わない。

旧 `ref.anchorScope` を新しい `asAnchor([ref])` に機械置換すると、
scope の供給に加えて anchor-name まで供給する可能性がある。両者を等価と見なさない。
[bounded-ref-layout fixture](../fixtures/projects/bounded-ref-layout/input/src/Popover.tsx) のように
scope と anchor が別要素にあるケースを移行の基準にする。

## container-type の値と型定義

省略時に宣言を出さないこと、許容値を CSS 仕様に基づいて決めることは確定事項である。
[CSS Conditional Rules の定義](https://drafts.csswg.org/css-conditional-5/#propdef-container-type)
は `normal | [ [ size | inline-size ] || scroll-state ]`、初期値は `normal` である。
複合値の両順序を受け付けるなら、プロパティ固有の値は次の有限 union で表せる。

```ts
type ContainerTypeValue =
  | "normal"
  | "size"
  | "inline-size"
  | "scroll-state"
  | "size scroll-state"
  | "scroll-state size"
  | "inline-size scroll-state"
  | "scroll-state inline-size";
```

`normal scroll-state` や `size inline-size` は受け付けない。
この union は空白一つの API 表記を列挙する。CSS の任意の空白・大文字小文字まで文字列型に展開する必要はない。
複合値は grammar 順へ正規化する。省略と明示的な `normal` は区別して保持する。

[CSS Cascade の CSS-wide keyword](https://drafts.csswg.org/css-cascade-5/#defaulting-keywords)
も別の共通 union として許容する。確認時点の draft には `initial` / `inherit` /
`unset` / `revert` / `revert-layer` / `revert-rule` が含まれる。
公開型の `CssWideKeyword` にこれらを列挙する。
`var()` などの任意 CSS 式は有限 keyword union と別契約になるため、この案には含めない。

## 診断の契約

provider use が要求する attachment の欠落を診断する。
`style(ref.asAnchor())` に対して、同じ対象要素へ ref marker が供給されていないケースが該当する。
`asContainer` も同様である。gate によりその CSS が適用されないことを利用者に伝える。

consumer 側の設定漏れは別の診断である。`positionAnchor` に対する `asAnchor` の欠落、
`ref.at` に対する `asContainer` の欠落は、attachment が存在するだけでは解消しない。
[現行 ref validation](../crates/js-oxc/src/refs/validation.rs) にある provider 検査を新 API に合わせて更新する。
両診断を一つの「ref がない」というメッセージへまとめない。

診断の実装では、次の境界を守る。

- 単に同じ `style()` の引数に ref がないことだけでは欠落と断定しない。合成元の style や同じ対象要素への供給も考慮する。
- gate が一致しない分岐を許容する。動的 attachment の全分岐で各 provider が有効であることは要求しない。
- alias と有限候補を ref identity で照合する。異なる JSX 要素に同名 ref が登場することだけでは attachment の証拠にならない。
- 別モジュールや実行時 DOM の関係を証明できない場合、確認できる範囲を超えた断定をしない。
- `style.anchorScope` は receiver の attachment を要求しない。

## 出力順の設計

既存 utility と provider use の相対的な出力順は Stillwind として保証しない。
順序はバージョンアップや最適化によって変わり得るため、競合時の勝敗に依存する利用は避ける。
衝突は Lint で検出して警告する。通常 utility 同士の既存の順序規約は維持する。
現行経路の棚卸しと衝突診断・inline 出力最適化の設計は
[CSS contribution order design](css-contribution-order-design.md) に記載する。

## 入力・診断の範囲

- 空 scope 配列は警告し、scope 宣言を省略する。重複は identity で除去し、最初の出現順を保つ。
- scope 引数は `all` / `none` または配列リテラルで、各要素は有限 ref 候補へ解決できる式を受け付ける。
- container-type は省略または型に列挙した文字列リテラルを受け付ける。
- `positionAnchor()` は引数を受け付けない。
- property 衝突 Lint は同じ style 呼び出し内の供給を比較する。任意 selector による対象の重なりや別モジュールの DOM 関係は完全には検証しない。

## 検証する契約

以下を API と lowering の回帰検証の観点とする。

- attachment だけが marker を付け、静的・動的な provider / consumer use は marker を付けない。
- provider の出力は consumer の追加・削除に依存せず、明示した scope と型を保持する。
- alias、member receiver、モジュールをまたぐ有限候補で、参照名と lookup の identity が一致する。
- 単一候補の縮約と region 内の統合で、CSS variant、condition、保証対象の優先順位、runtime payload を保存する。
- view transition の動的 key と、同時に有効な複数 class token の合成を保存する。
- scope の省略と none、container-type の省略と normal を区別し、動的 scope の全候補を保持する。
- provider gate、consumer と scope-only use の gate 不要性、排他的 provider と同時供給の競合を検証する。
- attachment の欠落と consumer に対する provider の欠落を区別して診断する。
- utility と provider use の衝突警告を、静的・動的・条件付き・CSS variant 付きの各経路で検証する。
- 動的 view-transition-name の直接 inline 出力と、custom property が必要な適用制御・合成を保持する。
