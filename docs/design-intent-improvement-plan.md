# 設計意図監査に基づく改善計画

2026-09-14。実装前のアクション計画。対象コードは `f1b3170`。

## 根拠と現在の状態

本計画は、Codex task `01a09f41-0863-78d1-8a64-e77ea872fb56` におけるユーザーの
監査結果への回答を根拠とする。F1〜F6、L1〜L2、D1〜D2 はその監査項目の識別子。
参照会話「F4対応設計 siunners」については、ユーザーが今回提示した TLDR を採用する。
参照会話の assistant の追加提案、特に view transition の API 案は確定仕様に含めない。

既存コード・テスト・文書は現状確認の資料であり、それだけで制約の必要性を証明しない。
本ファイルの追加時点では、コード修正、ソース内 TODO の追加、bundler 実験は未実施。

## 決定事項

| 項目            | 改善方針                                                                                          | 実施区分                                                                               |
| --------------- | ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| F1              | utility の記述順依存を解消する                                                                    | 順序の定義と compiler 修正                                                             |
| F2              | 静的 spread・配列の穴の一律拒否を見直す                                                           | 構文正規化                                                                             |
| F3              | 固定 computed key の一律拒否を見直す                                                              | 構文正規化                                                                             |
| F4              | `ref.at()` の variant 化と有限候補の扱いを整理する                                                | variant 設計・実装                                                                     |
| F5              | ref label の ASCII 限定を見直す                                                                   | 識別子の検証・直列化                                                                   |
| F6              | React 固有規約を除去し、変換可能な契約違反を default error の lint にする                         | 診断と host policy                                                                     |
| L1              | projected source の checker facts 未対応を TODO で明示する                                        | TODO 追加                                                                              |
| L2              | per-module CSS 間の import による順序保証を別途調査し、未対応を TODO で明示する                   | 検証済み（2026-09-17）。3経路を実測し、結果を css-ordering.md とコード内コメントへ反映 |
| D1              | variant 内の `--*` object key は引き続き拒否する                                                  | 契約の明記・回帰テスト                                                                 |
| D2              | animation は定義 object の variant key で分岐する。frame 内 utility の variant は引き続き拒否する | variant 設計・emitter 修正                                                             |
| View transition | variant にする部分と identity/attachment を整理する                                               | §8 で範囲決定済み。lint と宣言 bucket を実装                                           |

## 検証で明らかになった設計上の注意点

方針を撤回する必要のある矛盾は見つかっていない。ただし、以下を省略すると新たな
過剰制約や意味の変更につながる。

### `&` の分類と CSS の配置可能性は別の判定

要素への紐付きは、variant の selector が適用先を表す nesting selector `&` を使うかで分類する。
属性値や文字列中の `&`、コメント、escape された文字は対象にしない。
通常 variant、任意 variant、展開された custom/compound variant に同じ判定を適用する。
正規化や capture 展開で導入された nesting selector も追跡する。

現行の `VariantApplication` と variant body parser は情報の保持方法に差があり、
任意 variant も一部の at-rule を selector template として返す。
さらに `selector_rule_from_variant_body` は `&` のない selector を拒否する。
既存の分類結果に bool を足すだけで完了とはせず、selector/at-rule の構造を保持する
共通の解決結果から依存を求める。各 compound branch の依存を失わない。

フラグは、たとえば `uses_subject: bool` とする。これは「identity を一切参照しない」
という意味ではない。named container を参照する `ref.at(query)` は ref identity を必要とするが、
`@container <name> (...) { @slot; }` 自体は適用先の `&` を必要としない。
参照 identity の有限性と、適用先 selector への依存を別に扱う。

| 解決後の構造                                  | 適用先の `&` への依存                       |
| --------------------------------------------- | ------------------------------------------- |
| `&:hover { @slot; }`                          | あり                                        |
| `@media (...) { &:hover { @slot; } }`         | あり                                        |
| `@media (...) { @slot; }`                     | なし                                        |
| `@supports (...) { @slot; }`                  | なし                                        |
| `@container <resolved-name> (...) { @slot; }` | なし。ref identity は別途必要               |
| 任意 selector / 任意 at-rule                  | 構文を解析し、nesting selector の使用を判定 |

`&` を持たないことだけで、あらゆる CSS 文脈へ直接挿入できるとは判定しない。
独立した selector、条件付き group rule、computed-value intrinsic は生成する構造が異なる。
許容判定は、要素への依存と、出力先に意味を保って配置できるかの両方で行う。
`&` の分類を口実に、適法に変換できるケースを再び一律拒否しない。

CSS の nesting selector は通常の文字とは区別される。
参照: [CSS Nesting](https://www.w3.org/TR/css-nesting-1/#nesting-selector)。

### animation は定義 object の variant key で分岐する

animation の frame 内 utility に variant を付ける形式は引き続き拒否する。
条件は frame と同じ階層の variant key で記述し、その値を animation 定義 object とする。

```ts
style.keyframes({
  from: "opacity-0 translate-y-2 scale-98",
  to: "opacity-100 translate-y-0 scale-100",
  "motion-reduce:": {
    from: "opacity-0",
    to: "opacity-100",
  },
});
```

variant key は通常の registry で解決し、適用先の `&` を必要としない条件を使用する。
任意 variant にも同じ判定を適用する。`motion-reduce:` は組み込みの reduced-motion 条件。
条件による rule wrapper は `@keyframes` の外側に配置し、frame の declaration-list には挿入しない。

同名の `@keyframes` を条件ごとに生成する場合、後の定義は先の定義と部分マージされない。
各条件で有効になる animation を完全な定義として生成する必要がある。
分岐 object は完全な定義として通常定義を置き換える。省略された frame は継承しない。
通常定義を先に生成し、条件付き定義は共通 variant 順と canonical identity の順に生成する。
複数条件が同時成立すれば後の完全定義が有効となる。入れ子は祖先の条件を積み重ねる。
この例から暗黙の frame/declaration マージ契約を追加しない。

`@keyframes` を条件で囲めるのは、この出力方式では media / supports 条件。`@container` 内の
name-defining rule は container 条件で制限されないため、animation 定義分岐では拒否する。
`&` 依存とは別の CSS 適用文脈の制約であり、通常 utility の `ref.at` は利用できる。
container による animation 切替には、別名 keyframes と利用側 animation-name の条件付き選択を
設計する必要がある。[CSS Conditional Rules §5.4](https://drafts.csswg.org/css-conditional-5/#container-rule)。

参照: [CSS Animations — Keyframes](https://www.w3.org/TR/css-animations-1/#keyframes)。

### lint 化は変換能力と契約違反を分離する

構文変換できる入力は、契約違反があっても compiler の構造上の失敗として扱わず、
`LintFinding` と出力を返す。既定 severity は error。
現行 CSS 設定の表現では `deny` が対応する。`warn` / `off`、既存の局所 suppression も扱う。

host は未指定 severity を現状 `warn` として扱う箇所があるため、finding の型だけを
移す変更では default error にならない。既定値を含む有効 policy を共通に決定し、
CLI/native、build、editor が同じ結果を扱うようにする。
既定 error による build 失敗と、compiler が出力を構築できない fatal error は区別する。

純粋性契約に基づく最適化の前提は維持する。lint の抑制だけで、契約に違反する副作用の
順序・回数を保存する実装を追加することにはしない。
変換不可能なケースは引き続き compile error とし、そのための複雑な変換経路を新設しない。
不可能性は具体的な使用文脈で判断し、AST の種類や Hook 風の名前だけで決めない。

## 改善アクション

### 1. F1: utility 適用順の共通化

- [x] `order_builtin_utilities` の built-in だけを並べ替える処理を見直す。
- [x] user utility、arbitrary property、複数ルールを返す utility、variant 付き utility まで
      入力順から独立した適用順を定義する。built-in 順と custom utility の相対位置・同順位の
      比較規則は実装前に明示し、コードの現在位置を根拠にしない。
- [x] 順序の情報を resolver/emitter/editor query で共有する。
- [ ] 文字列ソートと配列要素ソートを独立オプションとして editor に実装する。
      エディターソートのためだけの別順序表は作らない。
- [x] 同一 utility 集合について、文字列内・配列内・複数字句の順列で CSS の適用結果が変わらない
      回帰テストを用意する。生成 class 名の一致だけで判定しない。
- [x] 条件ユニットの順序、base→derived の依存順、属性・動的 custom property の後勝ちは保持する。
      これらの境界を跨ぐ単純な全体ソートは行わない。

対象: `crates/compiler/src/compiler/utilities/declaration_order.rs`、
`emission.rs`、design-system の順序 metadata、compiler query。

variant 順は import 展開後の CSS で built-in と user 定義を混ぜた最初の登録位置とする。
同名 variant の上書きでは位置を移動しない。関連 variant の group 内の値比較は保持する。
utility 順は project 設定の `utilityOrder: "authored"` または `utilityOrder: "tailwind"` で選ぶ。
既定は `authored`。variant 同様、built-in/user を混ぜた最初の登録位置を使う。
`tailwind` は Tailwind CSS 4.3.3 の解決後 AST と同じキーを使う。built-in / user / 任意 property を
区別せず、プロパティ順位の昇順列（同じ prefix の短い列は後）、宣言数の降順、数値を認識する
UTF-16 candidate 名比較の順に比較する。未知 property は順位列に入れず、宣言数には数える。
順位の上書きは `@stillwind-utility space-x-* (tw-sort: row-gap) { ... }` のヘッダーメタデータで指定する。CSS の特殊変数は使わない。Tailwind の同順位比較に合わせ、指定自体を宣言数相当のカウントに1つ加える。未知のプロパティ名では通常のプロパティ順位を使う。
互換性対応による追加ルールや rule 外の property/keyframes 定義を宣言数へ混入させない。
プロパティ表と比較処理の出典は Tailwind `property-order.ts` / `compile.ts` / `utils/compare.ts`。
`authored` の任意 property は従来の明示 property 順を使う。
同一 candidate の複数ルールは resolver が返す順を保つ。`@apply` も同じ比較関数を使用する。
`vp run @stillwind-internal/tools-tailwind#check:generated:tailwind-order` は Rust プロパティ表を照合する。
`vp run @stillwind-internal/tools-tailwind#check:generated:fixtures:tailwind-order` は Tailwind 本体で fixture を再照合し、
`vp run test:rust` がその fixture を使う Rust 差分テストを実行する。
`vp run generate:tailwind-order` はインストール済みの tailwindcss から
`tailwind_property_order.rs` の静的配列を生成する。
`vp run @stillwind-internal/tools-tailwind#generate:fixtures:tailwind-order` が fixture を別に生成する。
Rust ビルドはプロパティ表の生成に依存する。これらの順序データは上流 checkout を必要としない。
既存の解決未対応14候補を固定して検出し、
解決可能な候補の順位列・宣言数・全体の比較順を検証する。
`DesignSystemQuery::compare_candidates` が同じ resolver と比較関数を参照する。
未解決 utility / variant は `None` を返し、source の並べ替えや canonical 化は行わない。
検証: compiler の全 unit/integration test と query の順序テストを通過。editor のソート操作は未実装。

### 2. F2/F3: 静的構文の正規化

- [x] `style([...['a', 'b']])`、`style(...['a', 'b'])`、静的 object spread、配列の穴を
      内容を直接記述した形と同じ抽出経路へ乗せる。
- [x] `{'data-a': value}` と `{['data-a']: value}` を同じ既知キーとして扱う。
      固定 template key 等も同じ静的キー抽出の責務で判定する。
- [x] object spread の上書き、重複キー、クラス項目、属性項目を扱う際は、spread 元の object が
      JavaScript として評価される意味を保つ。単に全 property を連結して同じとみなさない。
- [x] 動的 spread の未解決、配列 iterator と clsx の意味の差、評価対象外の構造を区別する。
      既知の構造まで一律に拒否せず、解析・変換が成立しない入力は診断する。
- [x] raw/decoded 文字列の位置、Unicode、コメント、projection を含む source map を検証する。

対象: `crates/js-oxc/src/expressions.rs` と macro 引数抽出、関連 IR。
変数に保存した補間 utility 文字列の追跡は、この変更の要件に追加しない。

実装: literal spread を再帰展開し、固定キーの重複は最初のキー位置と最後の値を保持する。
検証: js-oxc 全テストと compiler の静的構文回帰テストを通過。Unicode キーと値の元ソース位置を確認。
projection テストで、絵文字・コメント・escaped utility と静的 spread を含む入力の
decoded 値、raw 文字列の span、属性値の span が元ファイルへ対応することを確認。

### 3. F5: label と CSS 識別子の扱い

- [x] `style.ref('項目')` のような有効な非 ASCII label を検討対象とする。
- [x] label、DOM class token、CSS selector、生成 identifier の表現を確認し、必要な escape を
      直列化境界で行う。validation の単純削除による不正な class/selector の生成を避ける。
- [x] Unicode・escape 対象文字・不正入力の区分を明文化し、命名の安定性と衝突検出を検証する。

対象: `crates/js-oxc/src/refs/definitions.rs`、compiler の `handles.rs` / `symbols.rs`。
許容範囲の詳細は CSS/DOM の契約から決める。ASCII 限定を CSS 上の必須条件とは説明しない。

label は開発者向けの可読な名前であり、round-trip や一意性を担わない。
空文字を含む任意の literal string を受け付け、空白だけを `_` に置き換える。
記号・既存の `_`・Unicode は保持する。CSS で表現できない NUL は置換文字にする。
CSS selector と生成 JavaScript の文字列はそれぞれの直列化境界で escape する。
identity のハッシュ計算は不変。Unicode・空白・記号付き ref の DOM/CSS 対応を検証する。

### 4. F6: 契約診断の移行

- [x] `use` / `useXxx` の呼び出し名判定など、React 固有の規約を Stillwind の共通 validator から除去する。
- [x] `MacroArgumentMutation` / `MacroExecutionBoundary` の現行検出対象を、変換可能な契約違反と
      実際に変換不可能な入力に分ける。既存 enum をそのまま移すだけにしない。
- [x] 前者は型付き `LintDiagnostic` と `LintFinding`、既定 error の有効 policy で扱う。
      通常の関数名、class/JSX などを一律に不可能とはみなさず、値式として保持できるか確認する。
- [x] `await` / `yield` 等も使用文脈で確認する。必要な ref の関係を保てない変換は compile error とし、
      そのケースのために変換機構を複雑化しない。
- [x] default error、warn、off、局所 suppression、editor の結果保持、build の失敗判定を検証する。
- [x] `docs/macro-contract.md` を、React の規約を前提としない Stillwind の引数契約へ更新する。

対象: `crates/js-oxc/src/contract.rs`、`crates/diagnostics`、policy の解決・公開、
`packages/host/src/compilation/compiler.ts` / `editor.ts`、CLI/native の出力。

実装: mutation と suspension/import は suppressible lint とし、既定 policy を `deny` とする。
class/JSX 自体と Hook 風の関数名では診断しない。構造抽出不能な入力は既存 compile error を維持する。
検証: js-oxc 全テスト、各 FW の生成構文、severity/suppression、host 全54テストを通過。

### 5. L1/L2: TODO と別途検証

- [x] L1 の拒否箇所に、projected facts の座標対応が未実装であることと、対応すべき処理を
      3行以内の TODO として記載する。入力全体が不正という説明にしない。
- [x] L2 の loader / standalone projection / explicit module CSS の拒否箇所に、
      CSS dependency transport と順序保証が未検証であることを3行以内の TODO として記載する。
- [x] L2 の実験を別途行う。派生 module の CSS が基底 module の CSS を import する形で、
      base→derived を保証できるか確認する。
- [x] 実験の結果を受け、順序保証は実装せず、3経路の拒否を撤去する（2026-09-17）。
      干渉する順序依存は lint `ambiguous-base-order` / `imported-base-interference` が報告する。

L1 は原文座標による projected facts 適用を実装済み。TS7 checker と各 converter の
契約は [SFC の TypeScript 型情報](sfc-type-checking.md) を参照。

L2 の対象: `packages/integration/src/shared/loader.cjs`、`packages/integration/src/vite/index.ts` の
standalone CSS projection と explicit module CSS の分岐（拒否は撤去済み。各経路は
自身の module の規則だけを運び、順序はホストの搬送に委ねる）。

L2 の実験表は以下を軸に作る。未検証セルを成功として扱わない。

| 軸       | ケース                                                                  |
| -------- | ----------------------------------------------------------------------- |
| bundler  | Vite、Webpack、Rspack/Rsbuild、Next.js/Turbopack の既存 adapter 経路    |
| 実行形態 | dev、production、SSR/hydration、lazy import                             |
| 依存形   | 直列、diamond、共有 base、条件付き possible bases、循環                 |
| 更新     | CSS 内容 HMR、依存辺の追加・削除、module 削除                           |
| 出力形式 | 自動 CSS import、CSS を値として取得する経路、standalone projection      |
| 確認対象 | import の解決・inlining・dedup、最終 CSS 順、computed style、source map |

CSS を値として取得する経路は、import 先の相対解決・収集と、利用者が複数 stylesheet を
挿入する順序まで区別する。CSS import が書けるだけでは、すべての挿入形態の保証にはならない。

### 6. F4/D2: variant の構造と利用文脈の統一

- [x] variant 解決結果に、適用先 `&` への依存を構造的に保持する。
- [x] 通常/任意/custom/compound variant、at-rule と selector の混在、quoted/escaped `&` を検証する。
- [x] `ref.at(query, utilities)` を、`ref.at(query)` が variant を返す規約へ変更する。
      `` ref.at`query` `` も同じ意味として扱う。utility は通常の wrapper/template の operand にする。
- [x] ref variant と同じ使用箇所で動作させる。最低限、次の2つを同じ意味として検証する。

```ts
style({ [ref.at("width > 40rem")]: { "hover:": "grid-cols-2" } });
style({ [ref.at`width > 40rem`]: { "hover:": "grid-cols-2" } });
```

- [x] call/tag の同値性、通常 variant との入れ子、通常 ref と同様の utility template 内での利用を検証する。
- [x] ref の有限候補、import/re-export、条件付き receiver を扱う。全候補の container 条件を
      無条件に適用せず、runtime の選択と対応させる。
- [x] API 型、抽出器、IR、compiler、runtime metadata、query と fixture を一貫して更新する。
      旧2引数 API の互換維持をこの計画の要件には追加しない。
- [x] `style.ref(label?)` は独立した ref identity を宣言する。
      包含関係は selector の使用箇所の `.in(scope)` に指定する。
- [x] animation 定義 object に variant key を追加し、その値を条件付き animation 定義として解析する。
      `from` / `to` / 百分率の値は variant なしの utility 文字列のままとする。
- [x] animation の通常定義、条件付き定義、条件の同時成立、任意 variant、要素依存 variant の拒否を
      検証する。frame 内の variant 拒否テストは保持する。
- [x] 条件付き keyframes の完全な定義を生成し、完全置換・共通 variant 順・入れ子条件の契約を適用する。view transition の utility bucket への緩和は後続の API 設計で決める。
- [x] 出力先の文脈を保持し、条件・selector・support dependency・property 使用情報を
      文字列化前に受け渡す。無関係な global selector へ frame declaration を漏出させない。

対象: design-system の variant 解決、`crates/ir/src/model.rs`、`crates/js-oxc/src/refs`、
compiler の `style_variants.rs` / `variants.rs` / `definitions/keyframes.rs`、公開 TypeScript API。

検証: workspace 全テスト、rustdoc（warnings を error）、host 全54テスト、Chrome の実動作を確認。
Chrome 検証は root で `vp run @stillwind/native#build` を実行後、
`e2e/css` で `vp run test:e2e`。通常 `0 → 1`、reduced motion で
省略した `from` が要素の基礎値を使う `.25 → .5` を検証する。

#### ref selector の scope 契約

`style.ref(label?)` は親を引数に取らない。`ref(state).in(scope)` の scope は raw ref、または
scope を付けていない ref call/tag とする。alias と有限候補は identity で解決する。
繰り返す `.in()` は元の ref predicate と対象要素に対する独立した AND 条件で、順序に依存しない。
operand 内の `.in()` / `.at()` は拒否する。raw ref の `.at()` と selector scope は
`style()` の key を通じて明示的に合成する。詳細は [ref use design](ref-use-design.md) を参照する。

### 7. D1 の拒否理由を固定する

`--*` object key は JavaScript の動的値を inline `style` で供給する入口とする。
variant wrapper 内では引き続き拒否し、stylesheet declaration へ自動的に移さない。
静的な値でもこの API の意味は変えない。

```ts
style("hover:[--x:var(--y)]", { "--y": value });
```

上のように、inline 値の供給と variant による CSS 側の変更を組み合わせる。

- [x] `variant_compatible` の拒否を保持し、理由をこの契約に合わせる。
- [x] 拒否ケースと、上記の組み合わせが成立するケースの両方を回帰テストにする。
- [x] F6 の lint 化を、D1 など無関係な DSL 入力エラーへ一括適用しない。

### 8. View transition の後続設計

- [x] name による participant の identity/対応付け、class による複数 participant の分類、
      要素への attachment、old/new/group/image-pair 等への styling を棚卸しする。
- [x] selector/条件の変換だけを行う部分と、identity・key・runtime attachment を生成する部分を分ける。
- [x] 単一要素、複数要素、keyed use、global/wildcard 対象の違いを整理してから API を決める。
- [x] `&` 依存と transition identity 依存を混同しない。参照会話の named VT = element-bound という
      分類を、そのまま新フラグへ移さない。

棚卸しで確定した範囲。

- `style()` は要素に紐づく宣言と宣言 identity に紐づく宣言だけを書く場とする。要素にも宣言にも
  紐づかない選択子は `DocumentScopedSelector` (`require-element-scope`、既定 `deny`) の lint とし、
  文書全体の規則（`::view-transition-*(*)`、`::view-transition`、`@view-transition`、
  `:active-view-transition-type()`）は stylesheet に書く。wildcard/global の第一級 variant は追加しない。
- 個別の遷移の擬似要素スタイルは宣言が持つ。`group` は animation option、`old` / `new` /
  `imagePair` は utility 文字列で、media / supports 条件 variant のみ付けられる。
- `vt.old` 等の use 側 variant 受信 API は**却下**。擬似要素は文書 root の下に生成されるため
  書いた要素がスコープにならず、効果が宣言の全 participant に及ぶ。フェーズのスタイルは宣言が持つ。
- name/class の著者指定は決定済み。name は `.for(key?)` と、解決済み宣言に
  `view-transition-name` が現れる経路のみ（同名の共有がグローバル CSS と接続する経路になる）。
  class は宣言 identity からの生成のみで著者向け API を持たない（任意プロパティ経路の扱いは
  `docs/view-transition-redesign.md` §6 の注意点）。
- `group` の option 条件は `docs/view-transition-redesign.md` §3.9 の option 単位の条件付き
  規則として実装済み（animation の定義 branch とは別形）。残りは繰り返しレンダリング /
  unstable key の lint。

後続設計（`docs/view-transition-redesign.md`）で実装済み。

- identity はクラストークン（`{v:"--sw-v-..."}`）。name は token と key から成る pairing key で、
  予約も一意性検査も行わない。同名を別の宣言が使うのは正当な pairing 手段である。
- `.for(key?)` は `style()` 内で消費するコンパイル時構文。リテラル key は CSS 宣言に、
  式 key は inline の名前になる。独立した keyed 値は実体化しない。
  - 動的 identity はトークンごとのクラス表になる。条件付き use は自分のクラスに専用の
    カスタムプロパティ（`--sw-vtc-*`）を書き、要素のクラスリスト側は `var(--slot,)` を並べる。
    解決後のリストは要素が宣言した slot の union になり、同時に成立した条件が互いを消さない。
- `group` の option 条件は option 単位の条件付き規則（§3.9。実装済み）。
- lint は `require-view-transition-name` / `single-view-transition-name`（既定 warn）。
  - 合成した base の遷移クラスはマージされないため `single-view-transition-class`（既定 warn）で
    知らせる（class unit を超えたマージは未対応）。
  - 残り: 繰り返しレンダリング / unstable key の lint。

## 実装順と完了判定

1. F1、F2/F3、F5、F6 をそれぞれ独立した変更として進める。D1 の契約を並行して固定する。
2. L1/L2 の TODO を追加する。L2 の実験は独立して実施する。
3. variant の構造・文脈を整備し、ref.at、有限候補、D2 の順に実装する。
4. view transition は §8 の決定に従い、lint と宣言 bucket の条件 variant・`imagePair` から着手する。
   use 側の variant 受信 API は決定後。

各項目は、元の拒否テストを書き換えただけでは完了としない。正当な入力の出力、必要な拒否、
順序、source map、host 経由の振る舞いを確認する。Rust の構造変更では早期に
`cargo check --workspace --all-targets` を行い、対象テスト、fmt、Clippy、rustdoc と
関連する host/framework の検証を実施する。フロントエンドの作業は Vite+ / pnpm を使用する。

本計画の検証はコード読解と CSS 仕様の照合まで。改善実装、bundler/browser 受け入れ、
新 API のビルド成功はまだ確認していない。
