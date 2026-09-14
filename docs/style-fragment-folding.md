# Contextual folding of style fragments

Status: implemented (2026-09-14). 以下の契約に基づく初期実装。

`style()` の生成 JS を、fragment の意味を保持する木として leaf から縮約する。
各 node に対し、親が要求する表現で縮約できるかを判定する。
呼び出し全体を出力形ごとの段階に分類しない。
CSS の class unit、identity、guard path、宣言順序は変更しない。

## 目的と対象

動的な式があるだけでは folding を拒否しない。値の評価回数・順序・条件と、
fragment runtime の合成結果を保持できるなら、オブジェクトや helper 呼び出しを削減する。

```ts
style("left-(--progress)", { "--progress": progress });
```

Svelte / Astro の目標出力（クラス名は説明用）:

```ts
{ class: "_s_x", style: `--progress:${stillwindCssValue(progress)}` }
```

React の目標出力:

```ts
{ className: "_s_x", style: { "--progress": stillwindCssValue(progress) } }
```

既存 helper の名前と変換契約を使用する。boolean・URL・文字列・数値の独自変換は追加しない。
一般の JS 最適化、変数定義をまたぐインライン化、CSS minify、CSS の共通化は対象外。

## 実装との対応

- `compiler/emission.rs`: utility lowering と評価構造の抽出。属性と helper call は構造を保持する。
- `compiler/props.rs`: call 単位の出力入力と class / guard の公開メタデータ。
- `compiler/fragment/expression.rs`: property、deferred helper、class tree、fragment の意味表現。
- `compiler/fragment/fold.rs`: record の field 計画と評価順序・表現形式の検査、隣接 record の部分縮約。
- `compiler/fragment/render.rs`: 選択済み計画の描画、必要な helper の登録と拒否 lint の報告。
- `compiler/expression_render.rs`: class-value の適合判定と描画。
- `runtime.rs`: helper binding と import の確定。
- `packages/core/src/runtime/fragment.ts`: folding の意味的な基準。
- `FrameworkSpec`: class キーと `StyleFormat::Object / String` の既存情報源。

React は className、他は class。React / Preact / Solid / Vue は style object、
Svelte / Astro は style string。この対応を folding 内で再定義しない。

以下の IR と Plan は概念表記。実装では `Fragment`、`Atom`、`Property`、`ValueExpr`、
`Emitted` と ordered `Field` に対応する。ClassValue の demand は `is_plain` による
構造判定で表し、公開 API や汎用探索器は追加しない。

## 保持する意味

1. 副作用がありうる式を含む record では、ユーザー式の回数・順序・条件を保持する。
   全式が副作用なしの場合は channel 間の評価順を変更できる。channel 内の合成順序は保持する。
2. `?:` と `&&` の guard を保持する。両枝が空で test も副作用なしなら分岐を削除する。
3. class は順序付き連結であり、集合ではない。空 class の省略も runtime と一致させる。
4. 通常属性は後勝ち。上書きされた値の評価まで削除してはいけない。
5. symbolic style payload は最後の non-null 値で全体を置換する。
   通常属性の style とも異なる。serialize 可能な payload が通常 style より優先される。
6. custom properties は名前ごとの後勝ちで、最後に materialized style に重なる。
   string FW では既存 style の後ろに追記され、object FW では object の上書きになる。
7. プロパティの有無、値の型、列挙順、style string の区切りを保持する。
   同じ CSS に見えるだけでは合格にしない。
8. 外部 props の `ToFragment` は opaque な操作として残す。getter、proxy、型エラー、
   foreign runtime の検査を object spread で代替しない。

## 中間表現

文字列化済みの JS から構造を読み戻さない。compiler 内部専用の小さな IR を導入する。
public な frontend IR に出力最適化の詳細を追加しない。

```text
FragmentExpr = Empty
             | Record(ordered contributions)
             | Sequence(children)
             | Branch(test, kind, consequent, alternate)
             | FromProps(source expression)

Contribution = Class(ClassExpr)
             | Attribute(static name, ValueExpr)
             | CustomProperty(static name, ValueExpr)
             | StylePayload(ValueExpr)

ValueExpr = Source(span)
          | Literal(value)
          | Helper(kind, arguments, origin)

ClassExpr = Static(text) | Value(ValueExpr)
          | Sequence(children) | Branch(...)
```

各 item は構築直後に関連 impl を置く。record は map にせず、入力順の列を保持する。
`Attribute("style", ...)` と `StylePayload(...)` と `CustomProperty(...)` は別の variant。
symbolic key は型の意味を runtime に渡す際の符号化であり、IR のキー文字列ではない。

`Helper` は binding 名を持たない。候補作成中に `RuntimeBindings::require` を呼ばない。
source span と import realm の anchor を保持し、選択済み出力だけが binding を要求する。

## 文脈と出力計画

```text
Demand = Fragment | ClassValue | Props
FoldContext = { demand, framework, import_realm }
Plan = { expression, representation, evaluation, shape }
```

- Fragment: 親が再合成できる形式。class / style / custom properties は symbolic key のまま。
- ClassValue: clsx が受け取れる class 専用の形式。props を混ぜない。
- Props: 呼び出しの根だけ。class / className と FW の style 表現へ変換する。

これらは最適化の段階や優劣ではなく、親が受け付ける表現の契約である。
一つの呼び出しの中で異なる demand が共存する。適用する規則は各 node の意味と demand から決まり、
静的 object、clsx、merge といった出力形の分類から逆算しない。

representation は enum で要求と照合する。途中の materialized props を fragment として渡すことを
型で防ぐ。根の demand が子にもそのまま伝播することはない。

shape は class の空値可能性、キーの重複、opaque payload、通常 style の有無など、
その subtree から証明できる情報だけを持つ。未知を推測で既知にしない。

evaluation は入力木で割り当てた occurrence ID の順序と guard を保持する。
同じ span / 同じ識別子でも別の occurrence を共有しない。Source、変換 helper、opaque 操作を
評価境界として扱う。短絡構造を持つ順序木として照合し、実行経路を列挙しない。
frontend は元の AST で簡易的に副作用を判定し、`ModuleIr::pure_expressions` に証明済みの
span を保持する。未知・欠落した情報は副作用ありとして扱う。projection では他の IR span と
同じ対応規則で元ソースへ写す。compiler はその情報を property、helper 引数、test に渡し、
class tree の副作用を子から集約する。

通常の call / new は副作用あり。`/*#__PURE__*/` / `/* @__PURE__ */` があれば呼び出し本体を
副作用なしとし、callee と引数は引き続き調べる。代入、更新、delete、await / yield、tagged
template、dynamic import、class 初期化、JSX は保守的に副作用ありとする。関数の生成では
実行されない引数初期化や関数本体を調べない。

getter・proxy・暗黙の型変換・iterator による効果や、純粋と判定された参照の TDZ 例外は
解析対象外。PURE 注釈も作者の契約として信頼する。副作用のある式が一つでも混ざる record
では純粋な読み取りも動かさないため、`x` を `x++` の前後へ移動しない。opaque props と
payload は引き続き縮約境界とする。

## 縮約アルゴリズム

1. CSS lowering を一度実行し、確定した class unit を FragmentExpr に載せる。
2. 子を必要な demand で bottom-up に縮約する。
3. 各 node で runtime による一般形を必ず一つ保持する。
4. 局所規則から候補を構築する。表現形式、shape、評価構造を検査する。
5. 不適合な候補だけを捨てる。元 node と成功済みの子の縮約は残す。
6. 直接 materialization が適合すれば採用し、それ以外は部分縮約済みの fragment を採用する。
7. 根の Props 計画を確定してから JS 描画・helper import・source mapping を生成する。

`NotFoldable` はコンパイルエラーではない。不正な元入力の診断を folding 失敗に変換してはならない。
一般形は根で一度 `stillwindMerge` する。子ごとの merge → ToFragment 往復は生成しない。
失敗候補が class allocation、CSS、helper import、診断を変更することはない。
候補は拒否理由を値として返せる。採用計画の確定後に、残った縮約阻害要因だけを
`LintFinding` へ変換する（後述）。

探索は node と demand ごとに一度行い、固定個数の候補に制限する。branch の分配展開、
部分集合探索、同値になるまでの反復 rewrite は行わない。概ね入力サイズに比例する構成にする。
初期実装は直接出力と一般形の固定された選択であり、数値 cost による探索はしない。
隣接 record の結合は評価 channel を増分追跡し、入力追加ごとに全 record を再検査しない。

## 初期の folding 規則

| 規則               | 成功条件と結果                                                                                               |
| ------------------ | ------------------------------------------------------------------------------------------------------------ |
| Sequence 正規化    | compiler 所有の配列境界のみ平坦化。空要素を除くが、その評価を削除しない                                      |
| 静的 class 連結    | 順序を保って文字列化。重複除去しない                                                                         |
| class 専用 subtree | 条件を保持した ClassExpr とし、必要時に一つの clsx へ。空 class の有無が一致しない直接 props 化は拒否        |
| Record 結合        | 隣接項を ordered record に統合。symbol ごとの payload 集約で評価順が変わる場合は拒否                         |
| Fragment 描画      | 静的キーの既知 record を symbolic object にする。custom properties は object のまま                          |
| Props 描画         | 既知 record の class、属性、custom properties を FW 形式へ。フィールドの配置で元の評価順を再現できる場合のみ |
| 部分縮約           | opaque base の前後や各 branch 内の既知部分を縮約し、親の一般形を維持                                         |

通常属性の重複は評価と列挙順を保持する object 構築で表現できる場合のみ許可する。
custom property 名の重複は、初期実装では直接 string 化を拒否する。
`--x:f();--x:g()` は、runtime が重複を解消した string と一致しないためである。
object 形式でも、先の値の評価とキーの初回挿入位置を保持することが条件になる。

opaque style payload と通常 style の競合は初期実装では一般形へ戻す。
生成済みの既知 payload を将来扱う場合も、通常属性との区別を消さずに専用規則で証明する。
temporary / IIFE を導入して無理に畳み込む処理は初期実装に含めない。
`await` / `yield` / `this` の文脈や投機評価を変更する wrapper は追加しない。

## 成功と失敗の例

提示例は Static class + 単一 CustomProperty なので直接 Props にできる。
親が Fragment を要求すると、同じ subtree は次の形になる。

```ts
{ [stillwindClassKey]: "_s_x",
  [stillwindCustomPropertiesKey]: { "--progress": stillwindCssValue(progress) } }
```

一方、次は CSS 変数を一つの style フィールドへ集めると f → g → h が f → h → g に変わる。

```ts
style({ "--x": f() }, { "data-value": g() }, { "--y": h() });
```

この全体 Props 化は拒否する。fragment 列を残し、順序を保持できる隣接部分だけを縮約する。
識別子であっても同じ規則を適用する。

## 縮約拒否の lint

評価順序によって直接 Props 化を拒否したケースは、既存の `LintFinding` 経路で報告可能にする。
単に merge が残ったことや、候補が一つ失敗したことを診断理由にしない。

- `fold.rs` の拒否結果に `FoldRejection::EvaluationOrder` を設け、対象 node、縮約操作、
  移動対象と間にある評価の occurrence / source span を保持する。
  その他の拒否理由も型で区別し、文字列の比較で lint 対象を判定しない。
- 最終計画でその阻害要因が残り、別候補で同じ縮約が達成されていない場合だけ報告する。
  親と子や複数 demand から同じ要因を報告しないよう、元 node と阻害 occurrence で重複を除く。
- diagnostics crate に `LintKind::StyleFoldEvaluationOrder` を定義する。
  抑制規則名は `style-fold-evaluation-order` とし、文面は
  「評価順序を維持するため、style の直接出力への縮約を省略しました」に相当する内容にする。
  検出と証拠の構築は compiler、文面と code は既存どおり diagnostics が所有する。
- primary label は対象の `style()` call とし、既存の call 単位の抑制を利用する。
  secondary labels はまとめたい CSS 変数と、その間の属性評価を示す。
  上の例では f → g → h の順序を維持する必要があることを note に記載する。
- 自動修正は提供しない。属性や式の並べ替えは挙動を変えうるため、単純な移動を推奨しない。
- `CompileResult.lint_findings` に追加し、既存の lint policy / `swlint-*` 抑制、
  host の severity と表示経路を通す。folding 後に生成する finding にも抑制が適用される位置で統合する。
  lint 自体は出力計画や CSS を変更せず、コンパイルエラーとして扱わない。

初期の公開 lint はこの評価順序の理由を対象にする。opaque payload や表現形式の不適合などは、
内部の拒否理由として保持しても自動的に公開 finding にはしない。

## 合成元と条件付き subtree

```ts
style(base, "left-(--progress)", { "--progress": progress });
```

この場合は `ToFragment(base)` を残す。後ろの既知部分は一つの symbolic fragment に縮約し、
根で merge する。base のキーや値を推測した直接 props 化はしない。

条件付き subtree は枝内で縮約できるが、test を class と style に複製しない。
`style(test() ? { "--x": f() } : { "--x": g() })` は初期実装では根の merge を残せる。
両枝の Props 化や条件の押し込みは、空値・キー有無まで証明する別規則として追加できる。

## 配置と実装手順

`compiler/fragment/` に責任をまとめる。

- `expression.rs`: semantic fragment とその構築操作。
- `fold.rs`: context、候補、適合判定、縮約規則。
- `fold.rs` 内の record 検査: occurrence 順序を照合し、branch の guard は木のまま維持。
- `render.rs`: 選択済み計画の symbolic / props 描画と helper 要求。
- `mod.rs`: module の役割と entry point の宣言のみ。

1. 既存出力を保って早期の文字列化を構造化する。CSS lowering は変更しない。
2. runtime の一般形を同じ IR から描画し、意味比較の基準にする。
3. record 結合、class 連結、文脈に応じた materialization の規則を実装する。
   custom properties の直接出力も同じ規則で扱う。
4. 部分縮約を追加し、呼び出し全体の出力形選択と user_code / user_value ベースの判定を削除する。

移行中の差分確認に旧出力を利用しても、完成後に二系統の最適化器は残さない。

## 検証と完了条件

同じ semantic IR から一般形と最適化形を出力し、runtime 実行結果を差分検証する。
比較対象は値だけでなく、Object.keys、キー有無、style の型と完全な文字列、評価ログ、
例外とその直前までのログ。dev / production と全 FW の設定を対象にする。

必須ケース:

- 提示例が全 FW で merge / fragment key なしになり、CssValue は一回だけ残る。
- boolean、URL、引用符、backtick、`${`、改行を含む値と安全な JS serialization。
- class の空値、重複属性、重複 custom property、通常 style と payload の競合。
- 条件・配列・null・false・ネスト、await / this、例外を投げる値変換。
- base の getter / proxy / 不正値、opaque payload、runtime instance の検査。
- 失敗候補から helper import が漏れず、最適化済みの子が維持されること。
- 評価順序の拒否例が `StyleFoldEvaluationOrder` を一件生成し、labels が元の式を指すこと。
  別候補で解消した拒否、単なる opaque 入力には出さず、既存 policy / 抑制が適用されること。
- CSS、class identity、guard path、generated symbol、source-map origin が変わらないこと。
- 大きい sequence と深い branch で候補数が有界なこと。fold の再適用で形が変わらないこと。

対象 Rust テスト、core fragment runtime テスト、FW ごとの生成コード実行テストを先に通す。
実装完了時は workspace の fmt / Clippy / rustdoc / tests、関連 host・framework acceptance、
fixture 検証を実行する。生成サイズと merge / object 数を測り、速度改善は実測した場合のみ報告する。

## 実行検証

`fragment/tests.rs` は record の拒否理由、重複キー、部分縮約、冪等性、大きい record を検証する。
同じ入力を folding なしでも描画し、`tests/fragment-execution.mjs` で全 FW の runtime と
開発／本番の両設定に対して実行比較する。値、キー順、style 型、評価・getter・変換ログ、
例外を比較する。`tests/static_compile/folding.rs` は実際の frontend からの直接出力と
lint span・抑制・helper alias・CSS identity を検証する。host テストでも生成 JS を実行する。

## 式のソースマップ

描画は `Code`（生成テキストと元 span の区間列）を返す。property value、条件の test、
composition base、helper の元ソース由来の引数は、文字列化の前から出現ごとの span を保持する。
結合時には生成位置だけを移動するため、並べ替えや同じ式の複数出現で元位置が混同されない。
空分岐の削除では、その式のマッピングも出力されない。

`SourceTransform.expressions` は置換内 UTF-8 区間を持ち、`render_transforms` が元ソースと
生成ソースの絶対 UTF-16 区間へ変換する。native binding はこれを host に渡し、host は
式内の各 code unit を元の位置へ、synthetic な構文は call 開始位置へ対応付ける。
生成順と元ソース順は一致を要求しない。SFC 属性の quote を entity 化するときは区間を再配置し、
entity 自体は元の一文字へ対応付ける。prefix の置換は synthetic な部分だけに適用する。
