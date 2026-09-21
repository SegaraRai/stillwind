# Tailwind CSS比較の不一致棚卸し — 2026-09-13

## 結果と読み方

現在の作業ツリーをTailwind CSS 4.3.3と比較した。**13,428候補出現のうち、既存ゲート相当の判定で4,893件が一致、8,535件が不一致**だった。同じ候補でもfixtureが異なれば別件として数える。これは互換性の欠陥数でも、ブラウザーでの意味的一致率でもない。

不一致の6,563件は空のthemeレイヤー、1,936件は登録済みプロパティの初期値と同じfallbackを含む差で、この2分類が不一致の99.58%を占める。ただし、**空レイヤーは後続CSSとのレイヤー順序を決めるため、無条件に書式差として消してはいけない**。

意味の違いとして優先して扱うべきものは、arbitrary drop-shadowの合成、theme変数とkeyframesのレイヤー配置、追加のカスタムプロパティ登録である。4件は比較用importアダプターの未対応、1件はホストによるURL解決前の比較だった。

今回追加したのは調査ツールと証跡・文書のみ。コアの修正や既存比較ゲートの合格条件変更は行っていない。前段の後処理削減を含む未コミット状態を調べており、変更前の同条件測定はしていない。したがって、以下をすべて今回のリファクタリングの回帰とは断定しない。

## 集計

### 自動分類の全件内訳

| 自動分類                           |       件数 | 意味                                                                 |
| ---------------------------------- | ---------: | -------------------------------------------------------------------- |
| `accepted`                         |      4,389 | 文字列一致または既存の宣言順序比較で一致                             |
| `accepted-empty`                   |        504 | 既存ゲートの空出力・候補なしの合格条件                               |
| `empty-theme-layer-only`           |      6,563 | 空のトップレベル`@layer theme;`を診断用に除くと一致                  |
| `registered-initial-fallback`      |      1,936 | 空themeレイヤーを除き、同一CSSの登録初期値と同じfallbackを除くと一致 |
| `property-registration-or-support` |         12 | `@property`・propertiesレイヤーの追加                                |
| `selector-or-rule-structure`       |         13 | theme変数10件、keyframes 3件の配置差                                 |
| `declaration-inventory`            |          1 | arbitrary drop-shadowの宣言構成差                                    |
| `declaration-value-or-order`       |          5 | 空白4件、URL placeholder 1件                                         |
| `input-adapter-exception`          |          4 | import修飾子の比較アダプター未対応                                   |
| `unresolved`                       |          1 | 候補なしの`@apply`出力を比較器が誤分類                               |
| **合計**                           | **13,428** | **一致4,893、不一致8,535**                                           |

分類は上から順に差を絞る排他的な診断であり、下位分類にも空レイヤー差などが併存する。診断用の変換は合格判定には使っていない。`unresolved`の1件を実態に合わせて移すと、空レイヤー関連は6,564件になる。

### 人手で確認した扱い

| 分類                          |      件数 | 次の対応                                                         |
| ----------------------------- | --------: | ---------------------------------------------------------------- |
| 空themeレイヤー・レイヤー予約 |     6,564 | 完成stylesheetとmodule carrierの比較境界を決める。単純削除しない |
| 登録初期値と同じfallback      |     1,936 | 対象環境と登録条件を限定した比較。コアのminify処理は復活させない |
| 空カスタムプロパティの空白    |         4 | 比較側でCSSの空値として扱う                                      |
| URL placeholder               |         1 | resource対応表を使い、ホスト解決を考慮して比較                   |
| importアダプター例外          |         4 | 比較基盤の未対応として明示                                       |
| 追加の`@property`・初期化     |        12 | 継承・初期値を含めて生成条件を検討                               |
| theme変数のレイヤー配置       |        10 | 生成時のtheme出力位置を検討                                      |
| keyframesのレイヤー配置       |         3 | 生成時のkeyframes出力位置を検討                                  |
| arbitrary drop-shadow         |         1 | builtin定義でfilter合成と色の上書きを揃える                      |
| **合計**                      | **8,535** |                                                                  |

## 意味に影響する差

### A. arbitrary drop-shadowの合成 — 1件

証跡ID **53**、`borders-effects.fixture.md / effects.borders-and-shadows`、`drop-shadow-[0_1px_2px_rgb(0_0_0/0.25)]`。入力は通常のtheme/utilities importのみで、fixture独自のutility上書きはない。

主要な宣言は次のように異なる。セレクター・登録・初期化は省略している。

```css
/* Tailwind */
--tw-drop-shadow-size: drop-shadow(0 1px 2px var(--tw-drop-shadow-color, #00000040));
--tw-drop-shadow: var(--tw-drop-shadow-size);
filter: var(--tw-blur,) var(--tw-brightness,) /* 他のfilter項 */ var(--tw-drop-shadow,);

/* Stillwind */
--tw-drop-shadow: 0 1px 2px #00000040;
filter: drop-shadow(var(--tw-drop-shadow));
```

Stillwindのarbitrary shadow分岐は共有filterチェーンを通らず、`--tw-drop-shadow-color`も参照しない。blur等との合成やdrop-shadow色の上書きに影響する。原因候補は[tw-effects.css](../../packages/core/css/builtin-utilities/tw-effects.css)の`--value([shadow])`分岐。出力後の文字列修正ではなく、builtin宣言の生成元を揃える対象。

### B. theme変数のレイヤー配置 — 10件

ID **13090, 13091, 13239, 13268, 13271, 13278, 13279, 13281, 13282, 13283**。

Tailwindはtheme変数を`@layer theme { :root, :host { ... } }`に出す一方、Stillwindは当該変数をレイヤー外の`:root,:host`に出す。値やutility本体が同じでも、別レイヤーから変数を上書きすると優先順位が変わる。通常宣言ではレイヤー外の宣言が名前付きレイヤー内より優先するため、書式差ではない。[CSS Cascade 5](https://www.w3.org/TR/css-cascade-5/#layer-order)

代表は`foundations.theme-overrides-and-static-variables`の`bg-brand`。生成結果に別の名前付きレイヤー内の`:root { --color-brand: ... }`を追加する比較が必要。調査箇所は[stylesheet/template.rs](../../crates/compiler/src/compiler/stylesheet/template.rs)の`CssOutputNode::Theme`と、入力からtheme tokenの所属位置を作る処理。

### C. keyframesのレイヤー配置 — 3件

ID **13140** `motion-safe:animate-spin`、**13289** `animate-spin`、**13290** `animate-pulse`。

Tailwindは該当`@keyframes`をレイヤー外に出し、Stillwindは`@layer theme`内に出す。同名keyframesがなければ今回の定義内容は同じだが、別レイヤーの同名定義との競合結果が変わる。CSS仕様は名前を定義するat-ruleにもレイヤー順序を適用する。[CSS Cascade 5 — name-defining at-rules](https://www.w3.org/TR/css-cascade-5/#layer-order)

同じく[stylesheet/template.rs](../../crates/compiler/src/compiler/stylesheet/template.rs)でthemeノードの中に`extract_authored_keyframes`の結果を出す経路を確認する対象。完成したCSSを再走査して移動する必要はない。

### D. 追加のプロパティ登録・初期化 — 12件

| 対象                      | 件数 | ID                              |
| ------------------------- | ---: | ------------------------------- |
| `transform-cpu`           |    2 | 452, 12627                      |
| `transform-gpu`           |    5 | 455, 12630, 13086, 13146, 13180 |
| `ring-offset-{0,1,2,4,8}` |    5 | 10766–10770                     |

transformではStillwindだけが`--tw-rotate-x/y/z`と`--tw-skew-x/y`を`syntax:"*"; inherits:false`で登録し、propertiesレイヤーの初期化も追加する。ring-offsetではwidth/color/shadowの登録と初期化が追加され、colorには`#fff`、shadowには`0 0 #0000`等の初期値が入る。

カスタムプロパティの登録は継承と初期値を変える。例えば単独のtransform出力に対して親で`--tw-rotate-x`を設定した場合、未登録の場合の継承と、登録済み`inherits:false`の場合は同じではない。対応するrotate utilityも生成されて両者が同じ登録を持つケースと分ける必要がある。[CSS Properties and Values API](https://www.w3.org/TR/css-properties-values-api-1/#behavior-of-custom-properties)

既存ゲートには「参照されるプロパティの追加登録」を許容する例外があるが、それ自体は意味的一致の証明ではない。今回は空themeレイヤー差もあるため、その例外では合格しなかった。[tw-transforms.css](../../packages/core/css/builtin-utilities/tw-transforms.css)、[tw-effects.css](../../packages/core/css/builtin-utilities/tw-effects.css)、登録の到達性判定が調査対象。

## 大量に出る差の扱い

### E. 空themeレイヤー — 6,563件 + 誤分類1件

最小の例は`flex`。

```css
/* Tailwind */
@layer utilities {
  .flex {
    display: flex;
  }
}
/* Stillwind */
@layer theme;
@layer utilities {
  .flex {
    display: flex;
  }
}
```

このCSSだけならdisplayは同じ。しかし後ろに`@layer theme { .flex { display: grid } }`を追加すると、Tailwind側はutilities→theme、Stillwind側はtheme→utilitiesのレイヤー順になる。空の宣言もレイヤー順を予約する。[CSS Cascade 5 — layer ordering](https://www.w3.org/TR/css-cascade-5/#layer-order)

Stillwindの[template.rs](../../crates/compiler/src/compiler/stylesheet/template.rs)には、module carrierから空のレイヤー本体を省いても順序宣言は残す契約が明記されている。比較側で無条件に空レイヤーを消すことも、コアでTailwind風に削ることも避け、完成stylesheet・分割carrier・後続CSSを含む比較境界を先に定める。

ID **13075** `directives.apply-order-sensitive-same-property`は候補なしのfixture。`opacity:.5;opacity:.75`は両者とも正しい順で生成され、diagnosticsも空。差は`@layer utilities;`と`@layer theme,utilities;`であり、既存ゲートがclassなし分岐で`unresolved`と報告するだけだった。`@apply`の未解決ではない。

### F. 登録初期値と同じfallback — 1,936件

すべて`default-completions.fixture.md`の`mask-*`。代表ID **6689** `mask-b-from-0`では、同一CSS内に`--tw-mask-bottom-from-color`の`initial-value:black`があり、Tailwindの`var(--tw-mask-bottom-from-color)`に対してStillwindは`var(--tw-mask-bottom-from-color,black)`を出す。他の位置・色も同様。空themeレイヤー差も併存する。

有効な同一登録が適用され、参照が有効な値に解決される条件ではfallbackは使用されない。この条件下の差を揃えるためにコアへfallback削除passを戻す必要はない。ただし登録非対応環境、条件付き登録、変数の循環・無効値、異なる登録による上書きまで含む無条件の同値性は今回証明していない。登録されたプロパティへの参照とfallbackにも仕様上の規則がある。[CSS Properties and Values API — substitution](https://www.w3.org/TR/css-properties-values-api-1/#substitution)

診断ツールは同一CSSから取得した登録初期値とfallbackがAST直列化後に一致するものだけを取り除いて分類した。これは差の所在を示す操作であり、全件を意味的一致として合格させる操作ではない。

## 比較基盤で扱う差

### G. 空値の空白 — 4件

ID **176, 5258** `drop-shadow-none`、**2939** `backdrop-blur-none`、**3566** `blur-none`。

`--tw-…: ;`対`--tw-…:;`。当該宣言の空値は同じで、未定義・guaranteed-invalid valueとは異なる。CSSの宣言値は前後空白を除き、空custom propertyも許される。比較時の空値の扱いを修正する対象で、生成後に空白を足す必要はない。[CSS Variables 1 — changes / empty values](https://www.w3.org/TR/css-variables-1/#changes)

### H. URLのホスト解決前placeholder — 1件

ID **13204** `mask-[url(mask.svg)]`。Tailwindの`url(mask.svg)`に対し、Stillwindは`url(__stillwind_js_resource_…__)`を返す。native出力の`resources`にはそのplaceholderに対応する`owner:"src/Parity.tsx"`, `specifier:"mask.svg"`が存在することを確認した。

これは[host/css-source-map.ts](../../packages/host/src/css-source-map.ts)等でホストが扱う資源参照。比較器はCSS文字列だけでなくresource対応表と相対URLの基準を扱う必要がある。この棚卸しでは実ファイルのバンドラー解決は検証していないため、ホスト統合の合格ともコアのmask未対応とも数えない。

### I. importアダプター例外 — 4件

| ID           | fixture / 候補                                          | エラー                                                         |
| ------------ | ------------------------------------------------------- | -------------------------------------------------------------- |
| 647, 648     | `custom-css.prefix-migration`: `tw:flex`, `tw:bg-brand` | `Unsupported fixture import modifier: layer(theme) prefix(tw)` |
| 13135, 13136 | `directives.tailwind-utilities`: `flex`, `bg-red-500`   | `Unsupported fixture import modifier: source(none)`            |

[stillwind-fixture-css.ts](../../tools/lib/stillwind-fixture-css.ts)は単独の`layer(...)`以外のimport修飾子を拒否する。これらはコアが候補を評価する前の例外。`source(none)`を明示候補の比較でどう扱うかと、prefixの移行仕様をそれぞれ定義する必要がある。既存ゲートの`native exception`という表示だけでは原因を区別できない。

## 再現・証跡

```powershell
vp run --filter @stillwind/tools tailwind:inventory
vp run --filter @stillwind/tools typecheck
```

[tailwind-inventory.mts](../../tools/tailwind-inventory.mts)はnativeをビルドしてから、入力CSSごとにdesign systemを読み込む。同一パスのモジュールを1候補ずつ差し替え、project sessionのCSSを取得する。最初と250使用ごとに既存`compileSource`経路とCSS・classes・diagnosticsを厳密に照合した。**18入力のセッション、71回の照合がすべて一致**した。例外となったimport入力も証跡に保存する。

比較前のコメント除去、`foldCalcExpressions`、Lightning CSS minify、セレクター置換、干渉しない宣言順の許容は既存ゲートに合わせている。これらは調査ツール側でありコアの出力契約ではない。追加登録例外のmetadata除去はASTで実装しているが、今回この例外による合格は0件。さらに既存ゲートの実際の追加登録判定関数で、CSSを取得できた不一致8,531件を独立に確認し、合格へ変わるものが0件であることを検証した。

保存先は[data/2026-09-13-tailwind-inventory](data/2026-09-13-tailwind-inventory)。再実行すると同じ保存先を更新する。

- [index.json](data/2026-09-13-tailwind-inventory/index.json): 全13,428件のID、fixture、候補、入力hash、自動分類、集計。
- [mismatches.jsonl.gz](data/2026-09-13-tailwind-inventory/mismatches.jsonl.gz): 不一致8,535件の生CSS、比較用CSS、diagnostics、resources。入力例外はエラーのみ。
- `*.input.css`: hashで参照するfixtureの元入力。
- [provenance.json](data/2026-09-13-tailwind-inventory/provenance.json): 今回実行のHEAD、native binary・比較ツールのSHA-256、検証結果。棚卸し実行後に採取した固定記録であり、再実行時には再採取が必要。

環境はNode **24.21.0**、Tailwind CSS **4.3.3**、Lightning CSS **1.33.0**。HEADは`6f2b48e204d5f43bed9e853efb3ad6fb18321356`だが、前述の未コミット変更を含む。全index行の分類集計、全不一致IDとgzip証跡の一対一対応、参照入力ファイルの存在を別途検証した。toolsのTypeScriptチェックも通過。

### 範囲と限界

- 既存ゲート同様、`@source / @config / @plugin / @reference`を含む入力を除外。除外は`directives.inline-source-brace-expansion-and-negation`（0候補）、`directives.reference-apply`（0候補）、`directives.legacy-config-and-plugin`（2候補）の3ケース。例外4件を追加除外して成功率を上げることはしていない。
- 単一候補比較であり、utility組合せ、親子継承、同名keyframes、後続レイヤー、ブラウザー間の振る舞いを全件実行したものではない。意味差の記述は出力・実装・CSS仕様に基づく。上記の競合例をブラウザーで実行する検証は次段階。
- 既存のコメント除去正規表現、calc folding、Lightning CSSの`errorRecovery:true`を踏襲するため、生成した生CSSの妥当性を証明しない。生CSSも証跡として残している。
- `accepted-empty`は、Tailwind空出力時にStillwindのdiagnosticsを許容する既存条件を含む。単純な「実装済み504件」ではない。
- 宣言順序は単純ソートしていない。重複プロパティやshorthand/longhand等の干渉順を保つ既存比較を使用。全候補を同時投入するproperty-order制約検査とブラウザー受入れは今回の単一候補棚卸しに含まない。

次の実装ではA〜Dを生成元・出力構造で扱い、G〜Iを比較基盤で扱う。E・Fは比較対象の文脈と対応環境を明示してから判定を設計する。文字列の見た目を揃えるためのコア後処理を戻す理由にはしない。

## 修正後の状態

この棚卸しは修正前の記録として保持する。推奨修正の実施結果と検証範囲は[修正報告](2026-09-13-tailwind-parity-fixes.md)を参照。
