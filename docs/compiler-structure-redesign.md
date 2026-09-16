# コンパイラのモジュール再編: motion の解体と出力バンドへの統一

2026-09-16。`crates/compiler` と `crates/js-oxc` のモジュール構成を、資産の分類ではなく
「どの契約の出力を所有するか」で切り直す。設計・実装エージェントへの引き渡し用。

**挙動は変えない。** 生成 CSS、生成名、診断、ソース変換は移動の前後で同一でなければならない。
出力を変える項目は §5 に分離し、移動とは別のコミットにする。

## 0. 前提

- 順序の契約は `docs/css-ordering.md` が唯一の出典。L1 宣言順、L2 ブロック順、L3 unit 順、
  L4 module 順、および L3 の 1 番目「宣言が所有する定義」。
- 生成名の契約は `docs/generated-identifiers.md`、マクロの表層は `docs/macro-contract.md`、
  view transition の意味論は `docs/view-transition-redesign.md`。
- 本編は構造だけを扱う。上の契約の内容は変えない。

### 0.1 用語と表層対応

用語は次の意味で使う。表層（Tailwind 由来の utility・variant と CSS 仕様の語）以外は
すべて内部語であり、表層に対応する Rust の型は doc に対応を書く。

| 語                     | 定義                                                                                                                            |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| definition             | 宣言された資産と、それに対して一度だけ出る CSS（L3-1）。マクロでは宣言 object。                                                 |
| declaration            | identity を持つ宣言箇所（`DeclarationContext` が指すもの）。                                                                    |
| use                    | definition を消費して CSS を寄与する記述。                                                                                      |
| utility（列）          | utility registry で解決されるトークン列。要素の宣言を生む。                                                                     |
| animation option（列） | animation option registry（`@stillwind-animation-option` と property mappings）で解決されるトークン列。animation の宣言を生む。 |
| identity / name        | `docs/generated-identifiers.md` の契約。                                                                                        |
| variant / condition    | variant 適用と、media・supports による条件 wrapper。                                                                            |
| compiler options       | ホストが渡すコンパイル設定（`CompilerOptions`）。                                                                               |

命名規約: 修飾なしの `options` を内部名に使わない（設定は compiler options、値は animation
options）。資産の俗称（motion、handle）を内部名に使わない。

マクロ引数と CSS の表層 → Rust の名前:

| 表層                                                                   | Rust                                                                                                          |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `style(base)`                                                          | `StyleInput::Composition { value, candidates }`                                                               |
| `style("…")` / utility テンプレート                                    | `ClassExpression::Utilities`（utility 列）                                                                    |
| `style(a && b)` / `a ? b : c` / `style([...])`                         | `ClassExpression::Conditional` / `Array`                                                                      |
| `{ "hover:": … }`                                                      | `ClassExpression::Variant { variant: StyleVariant::Prefix, inputs }`                                          |
| `{ class: value }`                                                     | `GroupEntry::Class(ClassExpression::ClassValue)`                                                              |
| `{ "aria-*" / "data-*": value }`                                       | `GroupEntry::Attribute(AttributeEntry)`                                                                       |
| `{ "--x": value }`                                                     | `GroupEntry::CustomProperty(CustomPropertyEntry)`                                                             |
| `style(scope)` / `style(ref)`                                          | `StyleInput::ScopeAttachment` / `RefAttachment`（動的版は `DynamicScopeAttachment` / `DynamicRefAttachment`） |
| `ref.anchor` / `.positionAnchor` / `.anchorScope` / `.container(type)` | `ClassExpression::RefProjection { definition, value, projection: RefProjection }`                             |
| `ref()` / `ref("variant:")` / ref タグ                                 | `StyleVariant::Ref { refs, prefix }`                                                                          |
| `ref.at(query)` / ref.at タグ                                          | `StyleVariant::Container { refs, query }`                                                                     |
| `style.subtree(name?)`                                                 | \x60SubtreeDefinition { binding, label, … }\x60（label が書かれた文字列）                                     |
| `style.ref(scope?, name?)`                                             | `RefDefinition { binding, label, scope, … }`                                                                  |
| `style.animation(definition)` の frame                                 | `AnimationFrame { selector, utilities }`（utility 列）。variant キーは `AnimationBranch`                      |
| `animation(options)`                                                   | `ClassExpression::AnimationUse { animation, options }`（option 列）                                           |
| `style.viewTransition(definition)`                                     | `ViewTransitionDefinition`: `group` は option 列、`old` / `new` / `image_pair` は utility 列                  |
| `transition.for(key?)`（`style()` の外）                               | `ViewTransitionKeyApplication { call_span, value, key, transitions }`                                         |
| `transition.for(key?)`（`style()` の中）/ キー付きバインディング       | `ViewTransitionName::Key(ViewTransitionKey::…)` / `ViewTransitionName::Value`                                 |
| `StillwindDeclarationIdentity` 引数                                    | IR 表現なし（checker メタデータ）                                                                             |

## 1. 現状の問題

### 1.1 motion は契約の名前ではなく資産の分類名

`compiler/utilities/mod.rs` は「分割は CSS ordering のレベルに従う」と明記し、
`declaration_order`（L1）と `emission`（L2）でそれを実装している。バンドの名前を持たないのは
`motion` だけ。`docs/css-ordering.md` の L3 は 1 番目に「animation と view-transition の定義」を
置いており、括り自体は契約に存在する。問題は名前と範囲で、`motion` は「2 つの資産は意味論的に
同じ」という主張に見える。

### 1.2 motion.rs は親子で軸が反転している

- `compiler/motion.rs`: `resolve_animation_use`、`resolve_motion_options` と option 条件の機構
  （`OptionVariants` / `ResolvedMotionOptions` / `ConditionedOption` / `OptionCondition` /
  `wrap_conditions`）、`emit_view_transition_definition`、`emit_view_transition_utilities`
- `compiler/motion/animation.rs`: `emit_animation_definition`（animation の定義）

親がほぼ view transition、子が animation の定義側で、どちらのファイルも「1 資産を端から端まで」
になっていない。呼び出し元も別で、`pipeline.rs` は定義側の 2 関数、`emission.rs` は
`resolve_animation_use` を取る。

### 1.3 motion_assets は決定を表していない

- `motion_assets` への直接挿入は 2 箇所だけ。`CallEmitter` の静的 VT use と
  `ClassExpression::AnimationUse` の処理。
- `runtime_assets` へは、同じ VT use の両方、`DynamicViewTransitionUse`、
  `DynamicAnimationUse`、`pipeline` の `.for` 適用が入る。
- `pipeline` は `runtime_assets` を clone し、call を歩いた後と `.for` 適用の後に、
  それぞれ `runtime_assets` を丸ごと extend する。

`motion_assets` が正しく見えるのはこの 2 回のコピーのおかげで、コピーを外すと動的 VT と
動的 animation の定義 CSS が消える。ref/subtree の identity も混ざる。集合の実体は
「定義 CSS を出す必要がある identity」であって motion ではない。

### 1.4 handles.rs に 3 つの責任が同居している

- identity の生成: `ref_identity` / `scope_identity` / `named_identity`
- call バンドの宣言生成: `render_handle_projection`（利用者は `emission.rs` の 2 箇所だけ）
- 全資産の export 到達性: `summary_runtime_assets`

`symbols.rs` は `use super::handles::{ref_identity, scope_identity}` としており、identity バンドの
中で依存が逆転している。名前も「handle = ref」という以前の呼び方のまま。

### 1.5 条件 wrapper の組み立てが複数箇所にある

media / supports の入れ子を作る同じ形が、`motion/animation.rs`（animation 定義）と
`motion.rs` の `wrap_conditions`（VT の option 条件と bucket）にある。全 step を扱う版は
`utilities/render.rs` と `style_variants.rs` にもある。列挙の解釈が散ると、variant を 1 つ
足すたびに全箇所を直すことになる。

### 1.6 実行時実体化の規約が 2 資産で違う

宣言の置換はどちらも `runtime_assets.contains(...)` で決まる。静的 animation use は
`runtime_assets` に入らないので宣言は `undefined` になり、静的 VT use は入るので `{v:...}` が
残る。VT 側は key が値を通ってくる `ViewTransitionName::Value` で値の読み出しが要るためだが、
それ以外の静的 use も含む保守的な近似になっている。§8 で決める。

## 2. 守るべき最小原則

- **P1 契約で切る。** ファイルは「どの契約の出力を所有するか」で切る。資産の分類では切らない。
- **P2 機構のあるディレクトリだけ。** ディレクトリは共有する機構があるときだけ作る。分類だけの
  共有ならフラットに置く。
- **P3 状態は 1 バンド。** 状態を持つ型の宣言は mod.rs に置き、impl は同じディレクトリに限る。
  フィールドは private のままにする（Rust の可視性は宣言モジュールとその子孫なので、バンド内の
  分割は可視性を広げない）。状態を持たない関数は契約の側へ出す。
- **P4 決定ごとに集合 1 つ。** 集合を写しで作らない。挿入は決定した場所で全部行う。
- **P5 契約は 1 箇所に。** 契約は docs にだけ書き、モジュールの doc はそれを参照して再掲しない。
- **P6 移動と意味変更を混ぜない。** 移動のコミットは出力を変えない。
- **P7 名前は契約の語彙から。** L1–L4、definitions、identities、options を使い、資産の俗称を
  作らない。

## 3. 具体案: compiler

```
crates/compiler/src/compiler/
  api.rs / pipeline.rs / output.rs     入口と結果
  definitions/                         L3 band 1: 宣言が所有し、宣言ごとに一度だけ出る CSS
    mod.rs                               バンドの契約（css-ordering.md L3-1 を参照）
    animation.rs                         emit_animation_definition と条件分岐（定義全体の置換）
    view_transition.rs                   4 bucket と group の option 条件
  animation_options.rs                 option 列の解決（animation use と VT group が共有）
  variants.rs                          variant 適用 → 条件 wrapper と conditions_only
  emission/                            call の walk（CallEmitter の宣言は mod.rs、フィールドは private）
    mod.rs                               CallEmitter / CallEmitterInputs の宣言と再エクスポート
    call.rs                              入口と骨格（run / StyleInput の dispatch / attachment / 診断と lint）
    class_value.rs                       ClassExpression の walk（utility・配列・条件・variant・object）
    ref_projection.rs                    ref projection（静的・動的）と projection の宣言生成
    animation_use.rs                     animation use（静的・動的）の宣言化
    view_transition.rs                   VT use（静的・動的）、view-transition-name、guard 付き class
  class_units.rs                       L3 の unit 順序、class identity、VT の要素側（slot）
  utilities/                           L1 / L2
  stylesheet/                          L4 の組み立て
  symbols.rs                           宣言 → 生成名と identity
  fragment/ expression_render.rs props.rs conditionals.rs style_variants.rs literal.rs lexer.rs
  react/ tests.rs
```

現在のファイルからの移動:

| 現在                                        | 移動後                                               | 理由                                       |
| ------------------------------------------- | ---------------------------------------------------- | ------------------------------------------ |
| `motion.rs` の VT 定義 3 関数と option 条件 | `definitions/view_transition.rs`                     | L3 band 1                                  |
| `motion.rs` の `resolve_animation_use`      | `emission/animation_use.rs`                          | use が寄与する宣言は call バンド           |
| `motion.rs` の `resolve_motion_options` 系  | `animation_options.rs`                               | animation use と VT group が共有する解決器 |
| `motion.rs` のテスト                        | `definitions/*.rs` と `animation_options.rs`         | 種別ごとに分ける                           |
| `motion/animation.rs`                       | `definitions/animation.rs`                           | L3 band 1                                  |
| `variants.rs`                               | `variants.rs` + 条件 wrapper                         | 列挙の解釈を 1 箇所へ                      |
| `handles.rs` の projection 生成             | `emission/ref_projection.rs`                         | call バンド。利用者は walker のみ          |
| `handles.rs` の identity 3 関数             | `symbols.rs`                                         | identity バンド。依存の逆転を解消          |
| `handles.rs` の `summary_runtime_assets`    | summary の問い `export_reachable_identities`（§5.2） | summary だけを読む関数                     |

呼び出し側は `pipeline.rs`、`definitions/view_transition.rs`、`emission/animation_use.rs` の
import が移動先を指すようになるだけで、処理の順序と引数は変えない。`handles.rs` は消える。

### 3.1 なぜ call バンドは 1 ディレクトリか

`emission` の walker は `CallEmitter` の状態を所有し、いま 1098 行ある（`emit_expression_inner`
425 行、`emit_input` 194 行）。per-asset の処理を受け入れる前に分ける。

分割は可視性を広げずにできる。`CallEmitter` の宣言（フィールド込み）を `emission/mod.rs` に
置くと、private フィールドは宣言モジュールとその子孫から見えるので、`emission/*.rs` の impl は
状態に触れられる。`compiler` の他のモジュールからは見えない。per-asset のハンドラを `emission/`
の外へ出す案は、フィールドを `pub(super)` にして compiler 全体へ見せることになるため取らない。

割り当て:

- `mod.rs`: バンドの契約、`CallEmitter` / `CallEmitterInputs` の宣言、再エクスポート。
- `call.rs`: `run` / `emit` / `emit_input`（StyleInput の dispatch、attachment、composition、
  診断と lint の記録、部品の push、`render_attribute` / `render_custom_property`）。
- `class_value.rs`: `emit_item` / `emit_expression` / `emit_expression_inner` / `emit_sequence`、
  `expression_position`、variant の document-scope lint。
- `ref_projection.rs`: `RefProjection` / `DynamicRefProjection` の腕、`handles.rs` から受け入れる
  `render_ref_projection`、`ref_class_read`。
- `animation_use.rs`: `AnimationUse` / `DynamicAnimationUse` の腕、`motion.rs` から受け入れる
  `resolve_animation_use`。
- `view_transition.rs`: `ViewTransitionUse` / `DynamicViewTransitionUse` の腕、
  `emit_view_transition_name` / `push_guarded_class` / `static_view_transition_name` /
  `report_view_transition_names`。
- `guard_steps` は `props.rs`（`GuardStep` の宣言元）へ。

### 3.2 命名

motion を外す移動と語彙の整理として、次の改名を適用済み（移動コミットの前の 1 コミット）。

| 現在                                             | 移動後                                                |
| ------------------------------------------------ | ----------------------------------------------------- |
| `resolve_motion_options`                         | `animation_options.rs` の `resolve_options`           |
| `ResolvedMotionOptions`                          | `animation_options.rs` の `ResolvedOptions`           |
| `motion_name_symbol`                             | `symbols.rs` の `definition_name_symbol`              |
| `record_motion_name`                             | `emission/call.rs` の `record_definition_name`        |
| `motion_css`（pipeline のローカル）              | `definition_css`                                      |
| `motion_assets`                                  | `required_definitions`（§5.1）                        |
| `summary_runtime_assets`                         | `ModuleSummary::export_reachable_identities`（§5.2）  |
| `crates/compiler/tests/static_compile/motion.rs` | `crates/compiler/tests/static_compile/definitions.rs` |

`OptionVariants` / `ConditionedOption` / `OptionCondition` は元から motion を含まない。
定義 kind の判定（§5.1 の seed の絞り込み）は `definitions/mod.rs` の `is_definition_kind` に
置き、バンドの所属規則をバンドが持つ。

handle は ref に置き換える（型・フィールド・関数・診断種別。診断コードは不変）。

| 旧                                                                        | 新                                                            |
| ------------------------------------------------------------------------- | ------------------------------------------------------------- |
| `HandleDefinition`                                                        | `RefDefinition`                                               |
| `HandleProjection` / `DynamicHandleProjection`                            | `RefProjection` / `DynamicRefProjection`                      |
| `HandleAttachment` / `DynamicHandleAttachment`                            | `RefAttachment` / `DynamicRefAttachment`                      |
| `HandleUse` / `handle_uses`                                               | `RefUse` / `ref_uses`                                         |
| `HandleFactsRequired`（診断種別）                                         | `RefFactsRequired`（コード SHTS001 は不変）                   |
| `ModuleIr::handle_definitions`                                            | `ref_definitions`                                             |
| `render_handle_projection` / `handle_projection_name`                     | `render_ref_projection` / `ref_projection_name`               |
| `handle_value` / `handle_class_read` / `map_handle` / `handle_references` | `ref_value` / `ref_class_read` / `map_ref` / `ref_references` |
| field `handles`（`StyleVariant` と動的 attachment / projection）          | `refs`                                                        |
| field `handle`（`ClassExpression::RefProjection`）                        | `definition`                                                  |

あわせて `ClassExpression::External` を `ClassValue` に、`RefDefinition` / `SubtreeDefinition` /
`AnimationDefinition` / `ViewTransitionDefinition` の field `name`（レキシカル束縛）を `binding`
にする。書かれた文字列は `label` のまま。

据え置き:

- モジュール `compiler/handles.rs` の名前は §3 の解体コミットまで維持する。

適用済み（追加）:

- canonical 文字列を `"ref:"` / `"dynamic-ref:"` に改名した。class identity のハッシュ入力
  なので生成 class 名が変わり、fixture の期待値と fixtures.json を更新した。
- fixture プロジェクトを `typed-ref-graph`（id・title・`src/refs.ts`）に改名した。
- docs の地の文とテスト名（§5.5）を更新した。

utility 列と option 列の newtype は不採用。両者は字句（`lex_utilities` と candidate）を共有し、
相違は解決器（`resolve_utility` / `resolve_animation_option`）と出力先だけにある。identifier
のような生成時のサニタイズやレンダー時のエスケープに相当する型固有の振る舞いがないため、
newtype は型区別だけを足すことになる。

## 4. 具体案: js-oxc

マクロが実際の境界なので軸は変えない。ただし `motion/` は解体する。animation と view
transition は互いを参照せず、共有するのは「call から消費される宣言」という分類だけなので、
ディレクトリにすると分類でまとめたことになる（P2）。

```
crates/js-oxc/src/
- motion/mod.rs
- motion/animation.rs        → animation.rs
- motion/view_transition.rs  → view_transition.rs
  refs/                        変更なし。ref と scope は projection と JSX 祖先検証を共有する
```

## 5. 意味の変更を伴う項目（移動とは別コミット）

### 5.1 motion_assets を required_definitions にする

決定ごとに集合を 1 つにする（P4）。

- `runtime_assets`: 実行時に JS 値として実体化する identity。宣言の置換を決める。ref/scope も入る。
- `required_definitions`: 定義 CSS を出す identity。L3 の 1 番目を決める。

挿入は次の 4 箇所で全部行い、写しは作らない。宣言の export 到達性（motion 種別のみ）、すべての
motion use（静的・動的とも）、`.for` 適用。これで clone と 2 回の extend が消える。

### 5.2 export 到達性を ModuleSummary の問いにする

`summary_runtime_assets` は `ModuleSummary` だけを読む純関数なので、
`ModuleSummary::export_reachable_identities` として summary 側の問いにする。compiler は
そこから 2 つの集合を seed する。配置は `ModuleSummary` の定義元（`crates/semantic`）を
推すが、compiler 側の reachability モジュールでもよい（§8）。

### 5.3 条件 wrapper を variants.rs に寄せる

media / supports の入れ子を組む関数を `variants.rs` に置き、`definitions/animation.rs`、
`definitions/view_transition.rs`（bucket と group option）から使う。全 step を扱う 2 箇所
（`utilities/render.rs`、`style_variants.rs`）は出力の形が違うので、step → 条件の写像だけを
共有できるか別途判断する（§8）。

### 5.4 診断コードを view の族へ寄せる

`InvalidViewTransitionVariant` は現在 `SHMOTION001` で、リポジトリ唯一の SHMOTION。族は
SHVIEW001 / SHVIEW002 が既にあるので `SHVIEW003` にする。docs の参照も追随する。

### 5.5 ドキュメントの追随

`docs/panic-audit.md` と `docs/rust-unit-test-audit.md` のファイルパス、
`docs/design-intent-improvement-plan.md` の `motion.rs` 参照、`docs/css-ordering.md` の L3-1 の
語（definitions）を更新する。§3.2 の handle → ref に合わせた `docs/compiler-design.md`、
`docs/golden-case.md`、`docs/review-fixtures.md`、`packages/vite/test/type-facts.test.mts` の
地の文は更新済み。

### 5.6 静的 ref projection を use-site の受信式に揃える（必須）

原則: 生成コードが実行時に読むのは、その値を示す **use-site の式**である。宣言サイトの束縛名を
式の代わりに使ってはならない。

`ClassExpression::RefProjection` は宣言の definition しか持たないため、emitter は宣言の束縛名を
式として書いている（設計スケッチ `docs/compiler-design.md` の
`StyleInput::Projection { reference, kind, origin }` が持つ `origin` を実装が落としている）。
利用側の名前が宣言名と異なると生成コードが壊れる。

- `import { primary as p } from "./refs"; style(p.anchor)` → `primary.c`（`primary` はスコープに無い）。
- `style(holder.trigger.anchor)` → `trigger.c`（member 受信でも同じ）。

修正: use-site の受信式 span を `RefProjection` に持たせ（`DynamicRefProjection.value` と同じ）、
emitter は `(受信式).c` を token 表で引く。静的と動的の差は表の大きさだけになる。動的側は
受信式を読むよう修正済みで、残っているのは静的側。生成コードが変わるので単独コミットとし、
aliased import と member 受信の回帰テストを追加する。実装後は §0.1 の `RefProjection` の行も
追随させる。

## 6. コミット分割案

1. `refactor: flatten the JavaScript motion extraction`（js-oxc、移動のみ）
2. `refactor: split the call band into an emission directory`（移動のみ）
3. `refactor: split motion into the definition band and shared resolvers`（compiler、移動のみ）
4. `refactor: dissolve handle helpers into symbols and call emission`（移動のみ）
5. `refactor: wrap conditions through one helper`（P4 の前段。挙動不変）
6. `refactor: name the definition reachability set`（§5.1）
7. `refactor: make export reachability a module summary query`（§5.2）
8. `chore: move the transition variant diagnostic into the view family`（§5.4）
9. `fix: read static ref projections from the use-site expression`（§5.6。生成コードが変わる）
10. `docs: record the compiler module layout`（§5.5）

この前に、§3.2 の改名を 1 コミット（`refactor: rename handle to ref and name to binding`）として置く。

## 7. 検証

- 移動のコミットは、`vp run fixtures:refresh` のあとに差分が出ないことで「出力が変わって
  いない」ことを示す。fixture の期待値を書き換える必要が出たら、そのコミットは移動ではない。
- Rust: `cargo test --workspace --lib --tests`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo doc --workspace --no-deps`（`-D warnings`）、`cargo fmt --all --check`。
- TS とホスト: `vp run check`、`vp run fmt`、core / host / vite のテスト。
- ブラウザ: `vp run verify:view-transition`（guard 付き use と option 条件の実挙動）。

## 8. 未決

- 静的 VT use を `runtime_assets` に入れ続けるか。値を読むのは動的 identity、動的 key、
  `ViewTransitionName::Value` だけなので絞れるが、出力（宣言が `undefined` になるか）が変わる。
  絞るなら単独のコミットにし、fixture の期待値を更新する。
- `utilities/render.rs` と `style_variants.rs` の step 解釈をどこまで共有するか。少なくとも
  media / supports の入れ子は共有する（§5.3）。
- `summary_runtime_assets` の最終的な置き場所。`crates/semantic` の
  `ModuleSummary::export_reachable_identities` を推すが、compiler 側の reachability
  モジュールでもよい。
