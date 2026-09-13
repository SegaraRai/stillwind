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
  :root,
  :host {
    @stillwind-slot theme;
  }
  @stillwind-slot keyframes;
}
@layer utilities {
  @stillwind-slot utilities;
}
@layer properties {
  @supports (/* 対応環境の条件 */) {
    *,
    ::before,
    ::after,
    ::backdrop {
      @stillwind-slot property-initials;
    }
  }
}
@stillwind-slot properties;
```

`theme`はlive tokenの宣言、`property-initials`はfallback初期化の宣言を挿入する。この2種類はスタイルルール内に置き、セレクターもCSS側で指定する。`keyframes`は参照されたtheme animationおよび生成時に分類したlocal animation、`properties`は必要な登録、`utilities`は生成ルールを挿入する。ルール用slotを宣言位置へ置くこと、宣言用slotをルール位置へ置くことはエラーになる。`@supports`条件もbuiltin CSSにあり、登録そのものをfallback query内に閉じ込めない。

`@theme`は定義だけを提供し、その位置へ暗黙のrootルールを出力しない。独立したCSSも、必要なtheme・keyframes・登録・初期化のslotを明示する。出力する寄与があるのに対応slotが欠けていればエラーになる。以前のルール位置のtheme/property-initials slotは、上記のようにセレクターで包む必要がある。

local animationは生成時からkeyframes寄与として分離し、CSS文字列から再抽出しない。任意の生CSSに含まれるkeyframesは、そのCSSの位置・条件を保持する。

同じslotを複数置くと各位置に同じ選択済み寄与を出す。条件・レイヤーを跨いだ一括重複除去はしない。利用側が複数配置のカスケードを所有する。theme keyframesの定義順と定義元のsource mapを保持する。

## 登録要求の生成

`@stillwind-property`の`activation: dependency`は、他の登録からの依存・groupまたは明示要求によってだけ登録する。ring-offset補助値は単独utilityの宣言・参照だけで継承規則を変更しない。rotate/skewは宣言するutilityでgroupを活性化する。

宣言本文が同じでも必要なsupportが違うutilityは、定義内の`@stillwind-require-properties --name;`で依存を明示する。例えば`transform`は登録を要求するが、`transform-cpu/gpu`は単なる参照である。

解決段階から`CssContribution`・`GeneratedRule`・`MappedStylesheet`まで、要求はCSS本文と分離したmetadataとして運ぶ。`@apply`展開も依存情報を別に返す。輸送のためのdirective生成・再解析・削除は行わない。登録表にない要求はエラーになる。生成ルールの識別には本文だけでなく依存・keyframes配置も含めるため、同一本文の別契約を誤って集約せず、producer削除時に不要な依存も除去できる。

`StylesheetRenderer::render_mapped_stylesheet`と`ModuleCssArtifact::mapped`はこのmetadataを保持する。文字列だけを受け取るrendererは生CSS用であり、失われた依存や生成animationの分類を文字列から推測しない。`module_css`/`to_css()`は未組み立てCSSの確認用であり、生成物の再組み立てには構造化APIを使う。keyframesのsource mapは生成寄与に付属し、slotへのappendで座標を移す。

## 比較範囲

既存の`@source / @config / @plugin / @reference`を含む3入力ケースは引き続き別機能の対象。除外候補数は2であり、今回の不一致を除外して合格に変えたものではない。候補なしのCSSは出力を直接比較し、classがないだけで未解決と報告しない。

```powershell
vp run --filter @stillwind/tools tailwind:contract:test
vp run --filter @stillwind/tools tailwind:parity:analyze
```
