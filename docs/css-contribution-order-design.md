# CSS use と utility の出力順

## 状態

現行実装の棚卸しと、[ref use の確定契約](ref-use-design.md) に基づく衝突診断・出力最適化の設計。
ref API と同一 style 呼び出し内の衝突 Lint を実装している。既存 utility と provider use の相対的な出力順は保証せず、
バージョンアップや最適化によって変わり得る。競合時の勝敗に依存する利用は避け、衝突を Lint で警告する。

## 順序を保証しない背景

| 対象                                 | 順序キー・出力経路                                                                                                 | 影響                                                                                            |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| 通常 utility                         | [UtilityOrder](../crates/design-system/src/evaluation/utilities/order.rs) の Authored / Tailwind                   | Authored は登録位置、Tailwind は property metadata と宣言数などで比較する                       |
| 任意プロパティ utility               | [registry/order.rs](../crates/design-system/src/registry/order.rs) の `@stillwind-arbitrary-property`              | Authored では通常 utility と同じ登録位置の空間に参加できる                                      |
| ref projection                       | [ref_projection.rs](../crates/compiler/src/compiler/emission/ref_projection.rs) の `Authored(None)` と marker gate | キーは通常 utility より前だが、selector のある別グループへ送られる                              |
| keyframes use                        | [keyframes_use.rs](../crates/compiler/src/compiler/emission/keyframes_use.rs) の `Authored(None)`                  | 同じ出力グループでは通常 utility より前。静的は寄与を集約し、動的は候補ごとに unit を割り当てる |
| view transition の class / 静的 name | [ClassUnits::materialize_transitions](../crates/compiler/src/compiler/class_units.rs)                              | base token と slot 宣言へ変換し、他の寄与と同じ unit に格納する                                 |
| CSS variant 付き view transition     | 同メソッドで token ごとの slot に variant を適用する                                                               | CSS 条件を保持し、同じ JavaScript condition の region に集約できる                              |
| 動的 transition name                 | [view_transition.rs](../crates/compiler/src/compiler/emission/view_transition.rs) の `view_transition_emitted`     | variant 付きは custom property 経由、variant なしは inline style に直接 name を書く             |

`Authored(None)` は `Authored(Some(position))` より前であり、Tailwind のキーよりも前になる。
したがって synthetic use のキー自体は既に低い優先順位を表す。ただし、次の出力構造がその比較範囲を制限する。

### selector の有無による分離

[emit_utility_css](../crates/compiler/src/compiler/utilities/emission.rs) は、selector・variant・container がない
plain utility を先に出し、残りを後で出す。marker gate のある provider は後者に入る。
同一 unit でも、plain utility と gated provider の順序は UtilityOrder だけでは変えられない。

```css
/* 現行の分類をそのまま流用した場合の概念的な出力 */
.unit {
  container-type: normal;
}
.unit:where(.marker) {
  container-type: size;
}
```

この出力では両者が適用されると後者が勝つが、その勝敗は Stillwind の保証ではない。
gate のための runtime 処理や、utility 優先を成立させるための出力分類変更は不要である。

### class unit と region の順序

[ClassUnits](../crates/compiler/src/compiler/class_units.rs) は base の後に条件付き unit を出す。
[class_value.rs](../crates/compiler/src/compiler/emission/class_value.rs) は region ごとに寄与を集約する。
単一 unit 内を正しく並べても、動的 use や別 region の use が後に出れば、先の utility を上書きし得る。
class 属性内の token の並び順では解決しない。

### view transition の出力最適化

動的な `view-transition-name` を inline style に直接出す既存の最適化を維持する。
custom property が値を一度中継するだけなら、その経由は省く。
view transition 以外でも同じ条件を満たす場合は直接プロパティを inline style に出力する。

CSS variant による適用制御、marker gate、疑似要素への指定、複数 token の合成などに
stylesheet 側の宣言が必要なら、custom property は中継以外の役割を持つ。
その場合は必要な経路を残す。特に gated provider を無条件の inline 宣言へ移してはならない。
共通 lowering は CSS 寄与と inline payload の両方を扱い、すべての値を CSS 宣言へ統一することを目標にしない。

## 衝突診断

既存 utility と provider use が同じ対象に同時に適用され、同じプロパティへ書き込む場合は Lint で警告する。
同じ静的 region にある場合も、条件付き use や別 class unit に分かれている場合も、出力順を契約にしない。
順序指定用の新しい builtin CSS directive は導入しない。
通常 utility 同士の既存の順序規約は、この決定の変更対象に含めない。

```ts
style(ref, ref.asContainer("size"), "[container-type:normal]");
style(ref, "[container-type:normal]", enabled && ref.asContainer("size"));
```

両例とも container-type の供給が重なる場合を警告対象とする。
`asContainer()` が型を省略していれば container-type の書き込みはないため、
container-type utility との衝突にはならない。asAnchor の scope 省略も同様に扱う。

Lint は、各寄与に対象・書き込むプロパティ・条件・gate・source span を保持し、
class unit の統合や CSS の直列化で出所が失われる前に照合する。
shorthand と longhand の重なりも扱う。排他的な条件や gate を証明できる場合は衝突扱いしない。
別モジュールや実行時 DOM など、静的に照合できない範囲もあるため、警告がないことを無衝突の保証にはしない。
警告では競合する供給元を示し、どちらが勝つかを解決策として案内しない。

keyframes / view transition の寄与にも、同じ書き込み情報を持たせる設計が使える。
これらへの衝突 Lint の適用範囲は個別の意味論に合わせて整理する。
既存の transition name 重複診断や transition class token の合成は保持する。
provider と utility の契約から、すべての use 同士の合成規則まで変更したとは解釈しない。

## 共通 lowering の方向

1. 各 use が identity・必要な定義・宣言・適用条件・gate・runtime payload と診断用の出所を構築する。
2. 衝突を検査するための書き込み情報を保持し、有限候補の選択と単一候補への縮約を共通化する。
3. 同じ条件と region の静的寄与を集約し、transition token の合成など各機能の規則を適用する。
4. 値の供給に custom property の中継だけが残る場合は、直接 inline 出力する最適化を適用する。
5. 必要な class unit、CSS と inline payload を生成する。

共通化では通常 utility 同士の順序、CSS variant、nested rule、fallback、条件付き token 合成を保持する。
既存 utility と provider use の衝突時の勝敗を保存するための処理は追加しない。
宣言の統合には [declaration_order.rs](../crates/compiler/src/compiler/utilities/declaration_order.rs) の
既存の declaration format と合成規則、[rule_boundaries.rs](../crates/compiler/src/compiler/rule_boundaries.rs)
のカスケード保護を利用する。

## 共通化の責務

- ref の marker 読み出しを返す独自経路を、keyframes と同型の候補 unit lookup に置き換える。
- keyframes と view transition の静的／動的候補処理、view transition の通常入力／式入力の重複を共通化する。
- `LiteralContribution` の utilities だけを追加する処理と、transition metadata の別処理を、固有の合成規則を持つ共通寄与へ整理する。
- inline payload と CSS 寄与を共通 lowering で保持し、直接 inline 出力できる条件を各機能で再利用する。
- provider と utility の供給元情報を、出力の形に依存しない衝突 Lint へ渡す。

keyframes 定義への依存、animation options の解決、transition token の合成、ref identity と gate、
scope 名集合の構築は各機能の責務として残す。
base の transition token リストは `ClassUnits::finish` が出力する。条件付き token と name は
`materialize_transitions` が unit 内の宣言へ変換し、CSS variant を持つ寄与も共通の class 割り当てへ渡す。

## 後続実装の検証観点

- provider と utility の衝突を、静的・動的・条件付きの各経路で警告し、供給元を示す。
- 省略されたプロパティや、排他的だと分かる条件・gate に対して衝突を報告しない。
- 最適化によって診断用の出所が消えず、警告が変わらない。
- provider gate の異なるルールをまとめず、marker のない要素へ設定が漏れない。
- 通常 utility の相互順序、variant 順序、nested rule、shorthand の上書き関係を維持する。
- transition の複数 token と slot の合成を維持し、動的 name の直接 inline 出力も維持する。
- custom property が適用制御や合成に必要な場合は残し、単純な中継だけの場合は直接出力する。
- provider と utility の衝突時の勝敗を期待値として固定しない。
