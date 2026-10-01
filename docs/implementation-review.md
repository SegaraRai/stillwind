# 実装レビュー: 改善候補と該当箇所

Status: 初回棚卸しの記録。B項目の不具合修正は2026-09-13に実施した。
現在の対応内容・維持した契約・検証結果は[修正報告](reviews/2026-09-13-rust-implementation-fixes.md)を参照。
以下の問題記述と再現結果は、監査時点の状態を保存している。

修正済みの矛盾コメントは末尾の「修正済み」に記録する。この文書自体は製品仕様ではなく、
作業対象の一覧である。

## 改善候補

| ID  | 優先     | 対象                                                                                                      | 一行要約                                                        |
| --- | -------- | --------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| A1  | P2       | `crates/napi/src/api.rs:245`, `crates/napi/src/session.rs:196`, `crates/compiler/src/compiler/api.rs:127` | `semantic_revision` を `Debug` 表記のハッシュで作っている       |
| A2  | P2       | `crates/design-system/src/options.rs:89` と `crates/compiler/src/options.rs:104`                          | prefix置換の実装が2クレートに重複している                       |
| A3  | P2       | `crates/compiler/src/module_compiler.rs:479`                                                              | 定義依存を保守的に全件登録しており、選択的無効化が効かない      |
| A4  | P2       | `crates/css/src/colors.rs:191`                                                                            | 透明色の正規化が特定スペルの文字列置換に留まる                  |
| A5  | P3       | `crates/compiler/src/compiler/stylesheet/properties.rs:105`                                               | `@property` 互換 `@supports` ガードがコンパイラ内にハードコード |
| A6  | P3       | `crates/design-system/src/options.rs:68`                                                                  | prefix検証の `len() < 4` がメッセージから読み取れない           |
| A7  | P3       | `crates/napi/src/session.rs`                                                                              | `baseCss()` がAPI面に残っているが、リポジトリ内の呼び出しが無い |
| A8  | 対応済み | `crates/compiler/src/module_compiler.rs`                                                                  | `from_css` のdocを現在の役割に合わせて修正                      |

## 各項目

### A1: `semantic_revision` が `Debug` 表記由来

3箇所が `Fingerprint::of(format!("{facts:?}{assets:?}"))` の形で同じ値を作る。

- `crates/napi/src/api.rs:245`: `Fingerprint::of(format!("{type_facts:?}{assets:?}"))`
- `crates/napi/src/session.rs:196`: `Fingerprint::of(format!("{:?}{assets:?}", source.type_facts))`
- `crates/compiler/src/compiler/api.rs:127`: `Fingerprint::of(format!("{facts:?}{assets:?}"))`

問題は3つ。第一に、`TypeFact` と `AssetMetadata` の `Debug` 実装は安定性を契約していない
ため、フィールドを1つ足すだけでキャッシュが全無効化される。第二に、`Debug` が非決定的に
なれば再現性が壊れる。第三に、同じ意味の値が3箇所の独立した式で作られており、片方だけ
変えても気付けない。

推奨: ファクトの決定的な射影を `crates/semantic` に定義し（順序付きの列挙・数値化、または
専用の `Fingerprint` 入力型）、3箇所から共有する。`crates/source/src/fingerprint.rs` が
バイト列の内容アドレスを提供する。`Debug`文字列もバイト列なので、この関数自体の契約違反ではない。問題は入力の意味的な安定性と、アダプタ間での統一である。

補足: 現在の派生Debugから誤キャッシュを実行再現したわけではない。不要な無効化は懸念だが、任意のDebug実装や入力項目の欠落、ハッシュ衝突まで含めて「誤結果は起きない」とは保証できない。checker設定・versionそのものを直接hashしているわけでもない。

### A2: prefix置換の重複

- `crates/design-system/src/options.rs:89` `DesignSystemOptions::rewrite_generated`（`pub`）
- `crates/compiler/src/options.rs:104` `CompilerOptions::rewrite_generated`（`pub(crate)`）

両者は同一のセンチネル（`U+E000` / `U+E001`）を使う同一実装である。センチネルが必要な
理由自体は論理制約で、単純な逐次 `replace` ではユーザー設定のprefixが `--sw-` や `--tw-`
を含む場合に相互干渉する。問題は重複の側にある。

- 実際に使われるのは compiler 側だけである。呼び出し元は `crates/compiler/src/query.rs:144`、
  `crates/compiler/src/compiler/pipeline.rs:266,275,278`、
  `crates/compiler/src/compiler/stylesheet/template.rs:219`、
  `crates/compiler/src/compiler/stylesheet/properties.rs:40,55,58,65,75`。
- design-system 側はリポジトリ内の呼び出しがゼロで、テスト
  （`crates/design-system/src/options.rs:140`）だけが参照する。ただし `pub` なので
  外部ホストが使える。

推奨: 実装を1箇所（例: `crates/css`）に置き、両クレートがそこを呼ぶ。少なくとも
「2実装が同一の結果を返す」ことを検証するテストを追加する。design-system 側の可視性を
下げる案は公開APIの変更になるため、単独の変更として扱う。

注意: `serde` の `rename_all = "camelCase"` と `#[serde(default)]` により、
フィールド名・既定値・直列化は外部ホストとの契約になっている。フィールドの改名や
`Option` 化は互換性に影響する。

### A3: 定義依存が保守的全件

`crates/compiler/src/module_compiler.rs:479` で `utilities` / `variants` / `properties` に
`fingerprints().utilities.keys()` などの全定義を入れている。その場のコメントも
「Conservative definition dependencies are safe for full invalidation.」と述べる。

これは現在の依存収集粒度による保守性であり、クラス名の必然的制約ではない。生成クラスのidentityにはcandidate・wrapper・assetの綴りと呼び出し位置等を用い、解決後の宣言テキストそのものは用いない。未使用の定義が変わっただけで任意の出力が変わる、という以前の説明は誤り。選択的無効化には解決成功した定義だけでなく、失敗した探索や新規追加で結果が変わる名前も追跡する必要がある。現在のreloadは全moduleを再コンパイルしている。

一方で、選択的再コンパイルの土台はすでに揃っている。

- `crates/design-system/src/snapshot/data.rs:57` の `DefinitionFingerprints` は
  「テーマトークン名ごと」「ユーティリティ名グループごと」の粒度を持つ。
- 逆引き索引 `crates/project/src/dependencies.rs` は、この3種については全対応になる。

推奨: 解決時に一致した定義を記録し、`CompilationDependencies` の3集合へ流す。
`CompilationDependencies` は公開型なので、狭める変更は無効化挙動の変更として扱い、
「省略してはならない（不足は古いCSSを残す）」という既存契約を維持したまま狭める。

### A4: 透明色の正規化が文字列パターン依存

`crates/css/src/colors.rs:191` は `oklab(0% none none/0)` の4連結と
`oklab(0% none none/0)` の単体形のみを `#0000` へ置換する。呼び出し元は
`crates/compiler/src/compiler/utilities/render.rs:33` で、対象はコンパイラが生成した
宣言テキストに限られる。

限定されている理由は論理制約である。影リストを解析せず文字列置換で処理する以上、一般化すると
偽陽性が出る。現行lightningcssの特定の出力綴りに対応した処理であり、任意の入力に対する網羅性まではこの文字列置換から保証できない。

推奨: 影リストを構造として扱い、色成分のみを正規化する。文字列パターンへの依存が消え、
網羅性も上がる。対象は `shadow_with_color_override` / `shadow_with_color_fallback` の
呼び出し元（`crates/design-system` の shadow 系ユーティリティ）との境界になる。

### A5: `@property` 互換 `@supports` ガードの所在

`crates/compiler/src/compiler/stylesheet/properties.rs:105` が
`@layer properties{@supports (((-webkit-hyphens:none)) and (not (margin-trim:inline))) or ((-moz-orient:inline) and (not (color:rgb(from red r g b)))){…}}`
を文字列連結で組み立てる。この述語は Tailwind v4 の `@property` フォールバックと同型で、
`packages/core/css/**` 側にガードは存在しない。

`docs/css-ordering.md` が Tailwind v4 互換を要求として明記しているため、ガードの内容自体は
外部制約である。改善対象は置き場所と表現で、対象ブラウザの判別条件がコードから読み取れない。

推奨: ガードを名前付き定数にして由来をコメントで残すか、レジストリ側で「フォールバック初期値」を
宣言できるようにして著者CSSと同じ場所で保守する。

### A6: prefix検証の `len() < 4`

`crates/design-system/src/options.rs:68` の条件は
`!value.starts_with("--") || !value.ends_with('-') || value.len() < 4` で、実際の契約は
「`--` で始まり `-` で終わり、内部が空でない」。`len() < 4` はこの最後の条件を短く書いた
ものだが、エラーメッセージ（「must start with `--` and end with `-`」）には現れない。
`---`は開始・終了条件を満たすのに長さ条件で拒否され、その理由がメッセージから読み取れず、オプションを直列化して渡す
外部ホストが戸惑う。

推奨: メッセージを実際の条件に合わせるか、条件を `--` と末尾 `-` の間の非空チェックとして
書き直す。

### A7: `baseCss()` の未使用

ホスト側の`baseCss()`利用は見当たらないが、Rustの`StylesheetRenderer::render_base_stylesheet`はN-APIの`base_css`実装から呼ばれている。両方が未使用という以前の記述は誤り。公開APIには外部の消費者があり得るため、内部の使用箇所だけを根拠に削除しない。

- `packages/integration/src/vite/index.ts:476` は `baseCssWithMap()` を使う。
- `packages/native/tests/loader.test.ts:69` も `baseCssWithMap()` を使う。
- `baseCss()` は生成された `binding.d.ts` に現れるのみ。

推奨: 外部ホスト向けの互換APIとして残すのか、削除するのかを決める。残す場合は
「source map が不要な消費者向け」とdocに用途を書く。

### A8: `from_css` のdoc表現

`crates/compiler/src/module_compiler.rs:118` の「Lossless boundary between the existing
emitter and rule ownership.」は、git履歴にしか存在しない「以前のエミッタ」を前提にした
表現だった。今回のコメント監査で現在の役割を表す表現へ修正した。現在の挙動（`prelude`/`body` に分割し、`to_css` でバイト単位に復元する）は
docコメントと往復テストで検証済みなので、表現だけを現在形に直せばよい。

## 修正済み（矛盾コメント）

| 箇所                                                         | 修正前の問題                                                                                                                                                                                                                | 修正内容                                                                                                                                          |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/design-system/src/evaluation/builtins/registry.rs:1` | 「arbitrary-property の位置解決だけが位置を比較する唯一の場所」と主張。実際は `crates/design-system/src/evaluation/variants/order.rs` と `crates/compiler/src/compiler/utilities/emission.rs` も `builtin_order` を比較する | 比較がこのモジュール内で1箇所であることに限定し、他は各モジュールが比較すると明記                                                                 |
| `crates/project/src/dependencies.rs:1`                       | 「全カテゴリが exact で保守的近似ではない」と主張。実際は utilities / variants / properties が全件対応                                                                                                                      | 今回再訂正: candidates / theme_tokens / assetsにも構文上の参照・static/推移依存・宣言等が含まれる。全カテゴリが保守的で、集合ごとの収集方法を記述 |
| `crates/design-system/src/lib.rs:10`                         | 「`DesignSystemOptions::rewrite_generated` が両prefixを適用する」と主張。実際の書き換えは compiler 側の実装で、このメソッドはリポジトリ内で未使用                                                                           | 置換が生成テキスト書き換え時に一括で行われ、その入口は compiler 側にあると記述                                                                    |
| `crates/design-system/src/registry/mod.rs:1`                 | 「グローバルな `builtin_order`」「ソースオフセットを保存せよ」と主張。実際は variant がカウンタで、2つの順序空間がある                                                                                                      | 今回再訂正: builtinの位置付けとuser variantの記法別収集を区別。異なるuser記法間の著者順は保持されない（B4）                                       |
| `crates/compiler/src/options.rs:1`                           | `CompilerOptions::rewrite_generated` を「唯一の置換箇所」と主張。design-system に同一実装がある（A2）                                                                                                                       | このクレート内の置換入口であると限定して記述                                                                                                      |
| `crates/design-system/src/evaluation/tests/mod.rs:1`         | 「各スイートがローダ経由でCSSを読み込む」と主張。実際はメモリ上CSSとディスク由来fixtureの両方がある                                                                                                                         | メモリ上のCSSからスナップショットを作る場合が多いと書き分け                                                                                       |
| `crates/compiler/src/renderer.rs:109`                        | 「著者グローバル規則と Preflight は `render_base_stylesheet` に属する」と主張。実際は placement が決める                                                                                                                    | carrier 用の断片であることと placement の役割を記述                                                                                               |
| `crates/source/src/lib.rs:10`                                | 非公開モジュール `lint` への rustdoc リンク（`cargo doc` 警告）                                                                                                                                                             | リンクを外して文章化                                                                                                                              |
| `crates/napi/src/lib.rs:10`                                  | 非公開モジュール `session` への rustdoc リンク2件                                                                                                                                                                           | クラス名のみの記述に変更                                                                                                                          |
| `crates/semantic/src/type_facts.rs:17`                       | `TypeFacts` への未解決リンク                                                                                                                                                                                                | `super::TypeFacts` へ修正                                                                                                                         |
| `crates/project/src/session.rs:49`                           | 非公開フィールド `Self::rules` / `Self::contributions` への rustdoc リンク                                                                                                                                                  | 文章化                                                                                                                                            |
| `crates/compiler/src/module_compiler.rs:66`                  | 非公開メソッド `ModuleCssArtifact::from_css` への rustdoc リンク                                                                                                                                                            | コード表記に変更                                                                                                                                  |

## 2026-09-12 コメント監査で判明した実装上の課題

以下の実装変更は未実施。B1〜B5はCLIで実行再現、B6〜B10はソースと呼び出し経路の確認による改善候補である。[監査報告](reviews/2026-09-12-rust-comment-audit.md)と[再現結果](reviews/2026-09-12-rust-comment-probes.json)を併せて参照。

| ID  | 優先 | 問題                                                        | 根拠                                              |
| --- | ---- | ----------------------------------------------------------- | ------------------------------------------------- |
| B1  | P1   | theme値から同じplaceholderが再導入されると展開が停止しない  | CLI timeout + 進捗のないループ                    |
| B2  | P1   | 文字列内部の`calc()`を書き換える                            | CLI出力で文字列の値が変化                         |
| B3  | P1   | quoted semicolonに`!important`を挿入する                    | CLI出力で文字列の値が変化                         |
| B4  | P2   | user variantの混在記法で定義順が崩れる                      | 後のfocusより前のhoverが選択される                |
| B5  | P2   | 未使用animationのruntimeを消してもCSSが残る                 | bindingはundefined、keyframesは出力               |
| B6  | P2   | 外部assetのname spanが宣言offsetと現在のfile idを混在させる | asset_referenceの構築経路                         |
| B7  | P2   | 定義fingerprintと依存索引が将来の選択的無効化に不足         | raw token fingerprint、全件定義依存、新規名未収録 |
| B8  | P2   | publicデータ型の検証済みという保証が型で保護されない        | public fields/serdeと検証入口の分離               |
| B9  | P2   | デコード済み文字列のoffsetから原本Spanを直接作る            | JS escapeとsyntax lexerの座標系が異なる           |
| B10 | P3   | CSS処理の構文対応範囲と未知placeholderの扱いを統一する      | token処理と文字列走査の混在                       |

### B1: 再帰placeholderによる停止不能

[values/placeholders.rs](../crates/design-system/src/evaluation/values/placeholders.rs)の`resolve_css_placeholders`は`body.find("--value(")`から再検索し、置換が進んだかを確認しない。

```css
@utility x-* {
  color: --value(--color-*);
}
@theme inline {
  --color-loop: --value(--color- *);
}
```

`style('x-loop')`をCLIで処理すると3秒でtimeoutした。対照の`--color-red:red`は正常終了する。このケースでは置換結果が入力と同じであり、コード上も終了条件へ進まない。hostのbuild/watchを占有するためP1。

改善案: 展開を構造化し、参照スタックによる循環検知と展開量上限を設け、ソース付き診断を返す。同一文字列の検出だけでは相互循環や増幅を防げない。受け入れ条件は自己参照・相互参照・増幅入力の有界終了と、通常の多段展開を維持すること。

### B2: CSS文字列内部のcalc破壊

[math/folding.rs](../crates/css/src/math/folding.rs)の`fold_calc_expressions_with_zero_unit`は外側のquote状態を保持せず`calc(`を検索する。

`@utility x { content: "calc(1 + 1)"; }`に対する`style('x')`は`content: "2";`を生成した。exit codeは0、diagnosticsは空。数学関数ではなく文字列リテラルの内容を書き換えている。

改善案: 宣言値をCSS tokenとして走査し、実際のfunction tokenだけをfoldする。quoted文字列、escape、comment内の同じ綴りを保持するテストを追加する。入力が単なるテキストの場合も仕様を明確にする。

### B3: important付与で文字列内部のセミコロンを分割

[utilities/formatting.rs](../crates/design-system/src/evaluation/utilities/formatting.rs)の`important`は`split_terminator(';')`を使う。

`@utility x { content: "a;b"; }`に対する`style('x!')`は`content: "a!important;b"!important;`を生成した。exit codeは0、diagnosticsは空。文字列の値自体が変化する。

改善案: top-level declaration境界をtokenで認識してpriorityを付与する。同じ単純分割を使うmatching等もまとめて点検する。受け入れ条件にquoted semicolon、data URL、comment、既存important、複数宣言を含める。

### B4: variant定義の記法をまたぐ上書き順

[registry/variants.rs](../crates/design-system/src/registry/variants.rs)の`extract_custom_variants`はselector shorthand、custom-variant block、variant blockの順に収集する。解決側の逆順探索と組み合わさり、全体のソース順とは異なる。

```css
@utility x {
  color: red;
}
@variant state {
  &:hover {
    @slot;
  }
}
@custom-variant state (&:focus);
```

`style('state:x')`は`:hover`を生成する。後に記述された`:focus`が優先されない。コメントの「著者順」は訂正済み。どの記法を優先すべきかは仕様判断が必要だが、記法を等価な宣言形式として扱うなら不整合になる。

改善案: token抽出時に元offsetを保存して全形式をmergeするか、記法ごとの優先を仕様として明示する。同名定義、wildcard、importをまたぐ例で上書き順を検証する。

### B5: 未使用motionのCSS保持

[compiler/pipeline.rs](../crates/compiler/src/compiler/pipeline.rs)はanimation/view-transition宣言からCSSを生成した後でruntime利用に応じた置換を決める。

`const unused = style.keyframes({from:'x'}); export const value = 1;`では、`unused`が`undefined`へ変換され、classesも空だが、`@keyframes --sw-a-2242a443{from{opacity:0;}}`が出力された。

改善案: CSS生成前に到達可能なasset集合を計算し、公開・外部参照可能なassetをrootに含める。未使用ローカルassetだけを除去し、別moduleからの参照を壊さないようproject単位の所有権と整合させる。view-transitionにも同様の経路があるが、このCLI観測はanimationについてのもの。

### B6: 外部assetのspan provenance

[js-oxc/ast.rs](../crates/js-oxc/src/ast.rs)の`asset_reference`は`identity.start/end`（宣言moduleのoffset）へ`self.source.id()`（解析中bufferのid）を付けて`name.span`を作る。`module_path`と合わせたidentity metadataとして使う限り成立するが、通常のSourceFile slicingや診断へ渡せるSpanではない。

改善案: 外部identityと使用箇所のSpanを別フィールドにするか、宣言元file idを解決可能にする。「全Spanは原本のslice可能な範囲」という説明を将来の利用者が信じてしまわない型構造にする。今回は誤診断の実行例までは作成していない。

### B7: 選択的無効化の前提を整理する

[snapshot/data.rs](../crates/design-system/src/snapshot/data.rs)の`theme_index`はreset後のactive集合から作る一方、`DefinitionFingerprints.theme_tokens`はrawの`theme.tokens`からnameごとにcollectする。resetにより消えたtokenの個別fingerprintが残り得る。

[module_compiler.rs](../crates/compiler/src/module_compiler.rs)の定義依存は既存名を全件収録するが、その時点に存在しない新規定義名は索引にない。candidates/themes/assetsも最小のexact集合ではない。A3の説明は訂正済み。

現在の`ProjectSession::reload`は全moduleを再コンパイルするため、これだけで現行reloadのstale CSSを確認したという意味ではない。改善案はactive状態を反映したfingerprint、解決に失敗した名前・namespace resetの依存、および旧新snapshot差分の収集を揃えてから選択的reloadを導入すること。

### B8: constructorと公開データ契約

`SymbolicValue`/`RefAlternative`等は公開構造やserdeで構築でき、constructorの検証を全インスタンスの不変条件にはできない。`RefAlternative::new`もouter scope kindの検証を超えて、すべての内部candidateを再帰検証しているわけではない。`RuleKey`や`GeneratedRule`も公開フィールドを通してkeyと内容を不整合にできる。

改善案: transport用の未検証型と内部の検証済み型を分離するか、入口でvalidateして以降は変更不可にする。公開APIの互換性と外部checkerの責任範囲を先に決める。任意の利用者構築値でも安全に動くことが既に保証されている、とは扱わない。

### B9: デコード文字列と原本offset

[js-oxc/ast.rs](../crates/js-oxc/src/ast.rs)の文字列値はデコード済みだが、content spanは引用符を除いた原本範囲。[syntax/lexer.rs](../crates/syntax/src/lexer.rs)は文字列内のbyte offsetをspan.startへ加算する。`\u`等のescapeを含む場合、原本の同じ文字位置を指すとは限らない。

改善案: デコード時のoffset mapを保持するか、正確な部分Spanを構築できないときはliteral全体のprovenanceを使う。引用符・escape・多byte文字を組み合わせ、診断位置とshadow編集範囲を入口から確認する。これはコード上の座標契約の不一致であり、今回ユーザーに見える誤診断位置を再現したという報告ではない。

### B10: CSS構文走査と未知placeholderの方針

block抽出の一部はtokenを使うが、`strip_blocks`等や宣言分割には文字列走査が残る。custom-identやstring判定も完全なCSS parserではない。B2/B3はこの混在から実際に起きる例である。

また`@utility x { color: --unknown-placeholder(); }`はそのまま出力され、diagnosticsは空だった。これは「すべての未解決placeholderを拒否する」という旧コメントには反するが、未知のCSS関数を通す設計自体を不具合と断定するものではない。

改善案: コンパイラ専用macroの予約範囲と、透過的に保持するCSS構文の境界を定義する。既知macroの未解決だけを拒否する方針なら、その契約を入口と診断で統一する。汎用CSS関数を誤って拒否する全面的な未知名チェックは避ける。

### 補足: registry監査のcoverage判定

通常ignoredのreachability監査も明示実行し成功した。ただし36/1,292 utility定義はresolveしても、定義を除去したときの出力差が検出されなかった。現行assertionはutilityのmatch/resolveとexact duplicateを検査し、すべてのutilityが出力へ寄与することまでは要求していない。variantは91/91が寄与した。

改善候補はこの36件を、意図したfallback・同等な分岐・fixture不足に分類すること。観測だけで「不要な定義」と判断して削除しない。網羅監査を継続的なgateにしたい場合には、通常ignoredという実行方針と、何をcoverageと呼ぶかも合わせて決める。

## 確認コマンド

```powershell
$env:CARGO_INCREMENTAL='0'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo test --workspace
```

上記は従来の確認手順。2026-09-12の再監査の終了結果と追加テストは[検証結果](reviews/2026-09-12-rust-comment-validation.md)を参照。
