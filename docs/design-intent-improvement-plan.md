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

| 項目            | 改善方針                                                                                          | 実施区分                   |
| --------------- | ------------------------------------------------------------------------------------------------- | -------------------------- |
| F1              | utility の記述順依存を解消する                                                                    | 順序の定義と compiler 修正 |
| F2              | 静的 spread・配列の穴の一律拒否を見直す                                                           | 構文正規化                 |
| F3              | 固定 computed key の一律拒否を見直す                                                              | 構文正規化                 |
| F4              | `ref.at()` の variant 化と有限候補の扱いを整理する                                                | variant 設計・実装         |
| F5              | ref/scope label の ASCII 限定を見直す                                                             | 識別子の検証・直列化       |
| F6              | React 固有規約を除去し、変換可能な契約違反を default error の lint にする                         | 診断と host policy         |
| L1              | projected source の checker facts 未対応を TODO で明示する                                        | TODO 追加                  |
| L2              | per-module CSS 間の import による順序保証を別途調査し、未対応を TODO で明示する                   | TODO 追加・別途実験        |
| D1              | variant 内の `--*` object key は引き続き拒否する                                                  | 契約の明記・回帰テスト     |
| D2              | animation は定義 object の variant key で分岐する。frame 内 utility の variant は引き続き拒否する | variant 設計・emitter 修正 |
| View transition | variant にする部分と identity/attachment を整理する                                               | 後続設計。API は未確定     |

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
style.animation({
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

適用順は既存の variant 順を第一キーとし、同じ variant 内では registry 順のない utility を
先に置く。registry 順を持つ utility はその順に置き、同順位は candidate 名の辞書順で決める。
同一 candidate の複数ルールは resolver が返す順を保つ。`@apply` も同じ比較関数を使用する。
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

- [x] `style.ref('項目')` / `style.scope('領域')` のような有効な非 ASCII label を検討対象とする。
- [x] label、DOM class token、CSS selector、生成 identifier の表現を確認し、必要な escape を
      直列化境界で行う。validation の単純削除による不正な class/selector の生成を避ける。
- [x] Unicode・escape 対象文字・不正入力の区分を明文化し、命名の安定性と衝突検出を検証する。

対象: `crates/js-oxc/src/refs/definitions.rs`、compiler の `handles.rs` / `symbols.rs`。
許容範囲の詳細は CSS/DOM の契約から決める。ASCII 限定を CSS 上の必須条件とは説明しない。

実装: label は空文字を含む任意の literal string を受け付ける。生成名では `_` を二重化し、
ASCII の英数字・ハイフン以外は `_hex_` に符号化する。非 ASCII は保持する。
これにより DOM token と CSS selector は同じ名前を使用できる。identity のハッシュ計算は不変。
検証: 符号化の衝突回避、Unicode・空白・記号付き ref/scope の生成 CSS と DOM 名の一致を確認。

### 4. F6: 契約診断の移行

- [x] `use` / `useXxx` の呼び出し名判定など、React 固有の規約を Stillwind の共通 validator から除去する。
- [x] `MacroArgumentMutation` / `MacroExecutionBoundary` の現行検出対象を、変換可能な契約違反と
      実際に変換不可能な入力に分ける。既存 enum をそのまま移すだけにしない。
- [x] 前者は型付き `LintKind` と `LintFinding`、既定 error の有効 policy で扱う。
      通常の関数名、class/JSX などを一律に不可能とはみなさず、値式として保持できるか確認する。
- [x] `await` / `yield` 等も使用文脈で確認する。必要な scope を保てない変換は compile error とし、
      そのケースのために変換機構を複雑化しない。
- [x] default error、warn、off、局所 suppression、editor の結果保持、build の失敗判定を検証する。
- [x] `docs/macro-contract.md` を、React の規約を前提としない Stillwind の引数契約へ更新する。

対象: `crates/js-oxc/src/contract.rs`、`crates/diagnostics`、policy の解決・公開、
`packages/host/src/compiler.ts` / `editor.ts`、CLI/native の出力。

実装: mutation と suspension/import は suppressible lint とし、既定 policy を `deny` とする。
class/JSX 自体と Hook 風の関数名では診断しない。構造抽出不能な入力は既存 compile error を維持する。
検証: js-oxc 全テスト、各 FW の生成構文、severity/suppression、host 全54テストを通過。

### 5. L1/L2: TODO と別途検証

- [x] L1 の拒否箇所に、projected facts の座標対応が未実装であることと、対応すべき処理を
      3行以内の TODO として記載する。入力全体が不正という説明にしない。
- [x] L2 の loader / standalone projection / explicit module CSS の拒否箇所に、
      CSS dependency transport と順序保証が未検証であることを3行以内の TODO として記載する。
- [ ] L2 の実験を別途行う。派生 module の CSS が基底 module の CSS を import する形で、
      base→derived を保証できるか確認する。現在の拒否は検証前に除去しない。

L1 の対象: `crates/js-oxc/src/analyzer.rs` の projected facts 分岐。

L2 の対象: `packages/bundler/loader.cjs`、`packages/vite/src/index.ts` の
standalone CSS projection と explicit module CSS の分岐。

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
- [x] `style.ref(scope)` の有限 scope 制限は別 subtask として設計する。`ref.at` の変更だけで
      F4 全体を完了と扱わない。ref 生成自体を variant にせず、scope 候補と ref の相関を保持する。
- [x] animation 定義 object に variant key を追加し、その値を条件付き animation 定義として解析する。
      `from` / `to` / 百分率の値は variant なしの utility 文字列のままとする。
- [x] animation の通常定義、条件付き定義、条件の同時成立、任意 variant、要素依存 variant の拒否を
      検証する。frame 内の variant 拒否テストは保持する。
- [x] 条件付き keyframes の完全な定義を生成し、完全置換・共通 variant 順・入れ子条件の契約を適用する。view transition の utility bucket への緩和は後続の API 設計で決める。
- [x] 出力先の文脈を保持し、条件・selector・support dependency・property 使用情報を
      文字列化前に受け渡す。無関係な global selector へ frame declaration を漏出させない。

対象: design-system の variant 解決、`crates/ir/src/model.rs`、`crates/js-oxc/src/refs`、
compiler の `style_variants.rs` / `variants.rs` / `motion.rs`、公開 TypeScript API。

検証: workspace 全テスト、rustdoc（warnings を error）、host 全54テスト、Chrome の実動作を確認。
Chrome 検証は root で `vp run @stillwind/native#build` を実行後、
`experiments/module-graph` で `vp run verify:animation`。通常 `0 → 1`、reduced motion で
省略した `from` が要素の基礎値を使う `.25 → .5` を検証する。

#### 有限 scope を受け取る ref 定義の後続実装

現行の `HandleDefinition.scope: Option<ScopeDefinition>` は、1つの相関候補を表す。
`handles_from_alternatives` は利用側の有限 scope 集合を既に候補ごとに展開するが、
定義側 `collect_ref_definition` は singleton を要求する。この要件は未解消。

後続実装では定義と利用候補を分ける。定義は1つの ref identity と有限 scope 集合、元の
scope 選択式を保持し、利用側は既存の `(ref identity, scope identity)` の組として扱う。
全 ref と全 scope の直積を作らない。scope 選択は ref の新しい identity を生成しない。

両候補の scope が同時に DOM に存在する場合も、実行時に選択された組だけが適用される必要がある。
そのため単純な selector の OR 展開だけで完了としない。組ごとの marker と実行時の選択表を
持たせる案を、singleton の marker、ref projections、同一 callsite identity と合わせて検証する。
static singleton では選択表を省き、dynamic の receiver は元の値式を一度だけ評価する。

受け入れテストは同時に存在する2つの scope、scope を返す factory、import/re-export、
条件分岐、projection、SSR/hydration、scoped selector の誤適用の不在を対象とする。
実装時には宣言用 IR・runtime descriptor・checker 型の対応を一括更新する。

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

- [ ] name による participant の identity/対応付け、class による複数 participant の分類、
      要素への attachment、old/new/group/image-pair 等への styling を棚卸しする。
- [ ] selector/条件の変換だけを行う部分と、identity・key・runtime attachment を生成する部分を分ける。
- [ ] 単一要素、複数要素、keyed use、global/wildcard 対象の違いを整理してから API を決める。
- [ ] `&` 依存と transition identity 依存を混同しない。参照会話の named VT = element-bound という
      分類を、そのまま新フラグへ移さない。

`vt.old` 等の API、`style.viewTransition()` の引数変更、name/class の統合・分離は未決定。
今回の variant 共通処理をその仮案に固定しない。

## 実装順と完了判定

1. F1、F2/F3、F5、F6 をそれぞれ独立した変更として進める。D1 の契約を並行して固定する。
2. L1/L2 の TODO を追加する。L2 の実験は独立して実施する。
3. variant の構造・文脈を整備し、ref.at、有限候補、D2 の順に実装する。
4. view transition の API は後続の設計決定後に着手する。

各項目は、元の拒否テストを書き換えただけでは完了としない。正当な入力の出力、必要な拒否、
順序、source map、host 経由の振る舞いを確認する。Rust の構造変更では早期に
`cargo check --workspace --all-targets` を行い、対象テスト、fmt、Clippy、rustdoc と
関連する host/framework の検証を実施する。フロントエンドの作業は Vite+ / pnpm を使用する。

本計画の検証はコード読解と CSS 仕様の照合まで。改善実装、bundler/browser 受け入れ、
新 API のビルド成功はまだ確認していない。
