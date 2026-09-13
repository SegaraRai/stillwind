# Tailwind比較の契約

StillwindはCSSの解釈を比較する。文字列の整形・minifyはコアの責務にしない。比較器は以下の明示した文脈を持つ。無条件のCSS同値性とは区別する。

実装は`tools/lib/tailwind-parity-contract.ts`。境界テストは`tools/tailwind-parity-contract.test.mts`。

## 許容する契約差

1. **レイヤー順の予約**：比較用の共通環境で`properties, theme, base, components, utilities`を事前に宣言する。この環境で重複するトップレベル順序宣言は比較から除く。Stillwindの生成物から空レイヤーを削除するわけではない。その他のレイヤー、レイヤー本体、その条件は保持する。入力順の保持やcarrierの順序はStillwind固有テストで別途検証する。
2. **theme由来keyframesの配置**：既定Stillwind CSSはkeyframes slotをthemeレイヤーに置く。Tailwindがレイヤー外へ出したtheme由来定義だけを、比較用の期待値ではthemeレイヤーに配置する。Stillwind側を移動して合格させない。定義名・内容・条件の差は失敗する。任意のauthored keyframesは対象外。この契約により同名keyframesの競合時はTailwind既定配置と異なり、利用側の上位レイヤーから上書きできる。レイヤー外のslotを選ぶ構成は別の配置契約になる。
3. **prefix移行**：Stillwindは生成したクラス名を管理するため、Tailwindの`prefix(tw)`付きfixtureは入力修飾子と候補の`tw:`を両コンパイラーへの投入前に対応付ける。これは移行後utilityの比較であり、StillwindがTailwindのprefix構文を受理するという保証ではない。異なるprefixの混在やprefixが欠けた候補は拒否する。
4. **明示候補とsource(none)**：fixtureは候補を明示的に渡すので、このimport修飾子を取り除いても候補集合は変わらない。自動ソース検出はこの比較の対象外。その他の未知のimport修飾子を黙って無視しない。
5. **ホスト資源参照**：nativeの`resources`に記録されたplaceholderのみをspecifierへ解決する。fixtureではCSSとJSの資源参照に共通の仮想基準を与える。実際のowner相対パス、出力URL、sourcemapはホスト統合テストで検証する。未知のplaceholderを消したり任意のURLを同一視したりしない。

空custom propertyの前後空白はCSSの空値として同一視する。空値、未定義、guaranteed-invalid valueは区別する。その他の表記は比較側のCSS parser・calc評価・Lightning CSSで正規化する。壊れたCSSをerror recoveryで合格させない。

## 許容しない差

- 不要・不足する`@property`登録、`inherits`、syntax、初期値、fallback初期化の違い。「参照されている追加登録なら許可」という旧例外は削除した。
- `var()` fallbackの任意削除。builtin maskの不要なfallbackは生成元で省略する。利用側CSSに書かれたfallbackは保持する。
- 宣言やselectorの欠落、異なる値、条件、importance、干渉する宣言の順序。
- theme変数をレイヤー外へ出すこと。既定構成ではtheme slot内に出す。
- 任意のkeyframesを移動すること、名前だけで定義を削除すること。

宣言順比較は既存のWebrefに基づく干渉判定を使い、shorthand/longhandと同一プロパティの順序を維持する。単一候補比較に加えて、合成時のproperty-order制約を検査する。

## 出力slot

```css
@layer properties, theme, base, components, utilities;
@layer theme {
  @stillwind-slot theme;
  @stillwind-slot keyframes;
}
@layer utilities {
  @stillwind-slot utilities;
}
@layer properties {
  @supports (/* 対応環境の条件 */) {
    @stillwind-slot property-initials;
  }
}
@stillwind-slot properties;
```

`theme`はlive tokenの`:root,:host`宣言、`keyframes`は参照されたtheme animationおよび生成fragmentのトップレベルlocal animation、`properties`は必要な登録、`property-initials`はそのfallback初期化ruleを挿入する。`@supports`条件はbuiltin CSSにあり、Rustに固定しない。登録そのものをfallback query内に閉じ込めない。

明示slotが存在する種類について、`@theme`の定義位置には出力しない。明示theme/keyframes slotを持たない独立したCSSでは`@theme`自体をその場の出力位置として使う。暗黙に末尾へ移動しない。必要な登録・初期化のslotが欠けていればエラーにする。

fragment内で明示的な条件に包まれたkeyframesは、その条件から取り出さない。

同じslotを複数置くと各位置に同じ選択済み寄与を出す。条件・レイヤーを跨いだ一括重複除去はしない。利用側が複数配置のカスケードを所有する。theme keyframesの定義順と定義元のsource mapを保持する。

## 登録要求の生成

`@stillwind-property`の`activation: dependency`は、他の登録からの依存・groupまたは明示要求によってだけ登録する。ring-offset補助値は単独utilityの宣言・参照だけで継承規則を変更しない。rotate/skewは宣言するutilityでgroupを活性化する。

宣言本文が同じでも必要なsupportが違うutilityは、定義内の`@stillwind-require-properties --name;`で依存を明示する。例えば`transform`は登録を要求するが、`transform-cpu/gpu`は単なる参照である。

解決段階では要求は宣言本文と分離したmetadataになる。CSS fragment APIの輸送では同名のsemantic directiveとして保持し、最終stylesheetのslot展開前に消費する。これは途中のコンパイラー表現のloweringであり、生成済みCSSの最適化passではない。要求は名前を登録表と照合し、不明な名前はエラーにする。`@apply`も同じ依存を運ぶ。最終CSSにdirectiveは残さず、除去区間のsource mapを補正する。

## 比較範囲

既存の`@source / @config / @plugin / @reference`を含む3入力ケースは引き続き別機能の対象。除外候補数は2であり、今回の不一致を除外して合格に変えたものではない。候補なしのCSSは出力を直接比較し、classがないだけで未解決と報告しない。

```powershell
vp run --filter @stillwind/tools tailwind:contract:test
vp run --filter @stillwind/tools tailwind:parity:analyze
```
