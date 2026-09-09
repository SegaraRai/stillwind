# Rust panic 棚卸し（2026-09-10）

## 修正状況

優先度高の2系統を修正済み。Oxc引数変換10箇所は `as_expression()` で既存の診断へ分岐し、未閉鎖CSSは `ThemeError::UnterminatedBlock` を返す。ブロック抽出の逆転範囲も防御した。spread10ケース、CSSの未閉鎖・正常ネスト・文字列とコメント中の括弧、直接抽出の回帰テストを追加した。

以下は修正前の棚卸し記録。行番号と候補件数は調査時点のもの。

対象はworkspaceの非テストRustコード。`cargo clippy --workspace --lib --bins` でテストをコンパイル対象から外し、build.rsはビルド時コードとして分離した。ソース検索、呼出元確認、既存debug rlibにリンクした独立プログラムで補完した。製品コード・既存テストの変更はしていない。

## 結論

優先修正は **spread引数のOxc変換（10箇所）** と **未閉鎖CSSブロックのスライス（1箇所）**。どちらも小さい通常入力からpanicを再現した。公開Span/lexer APIの算術overflowも2箇所で再現した。

明示的な候補は27箇所（unwrap 1、expect 20、unreachable! 6）。うち3箇所はbuild.rs、実行時コードは24箇所。直接のpanic!/todo!/unimplemented!は今回のコンパイル対象から検出されなかった。この27件はpanic経路の総数ではない。

## 再現した問題

### P1: JavaScriptのspread引数

`Argument::to_expression()` はOxc内部で `as_expression().unwrap()` を呼ぶ。引数個数の検証は SpreadElement を除外しない。以下の各入力で `Compiler::analyze_javascript` のpanicを確認した。すべて先頭に `import { style } from 'stillwind/react';` を付けた。

| 呼出箇所                                        | 再現入力                                                             |
| ----------------------------------------------- | -------------------------------------------------------------------- |
| crates/js-oxc/src/refs/definitions.rs:18        | `const x=style.scope(...args);`                                      |
| crates/js-oxc/src/refs/definitions.rs:61        | `const x=style.ref(...args);`                                        |
| crates/js-oxc/src/refs/definitions.rs:77        | `const x=style.ref(...args,'x');`                                    |
| crates/js-oxc/src/refs/definitions.rs:85        | `const s=style.scope(); const x=style.ref(s,...args);`               |
| crates/js-oxc/src/motion/animation.rs:86        | `const x=style.animation(...args);`                                  |
| crates/js-oxc/src/motion/animation.rs:50        | `const a=style.animation({from:'flex'}); const x=style(a(...args));` |
| crates/js-oxc/src/motion/view_transition.rs:108 | `const x=style.viewTransition(...args);`                             |
| crates/js-oxc/src/refs/projections.rs:45        | `const r=style.ref(); const x=style(r.container(...args));`          |
| crates/js-oxc/src/refs/projections.rs:113       | `const r=style.ref(); const x=style(r.at(...args,'flex'));`          |
| crates/js-oxc/src/refs/projections.rs:114       | `const r=style.ref(); const x=style(r.at('width > 1px',...args));`   |

対応案: `as_expression()` のNoneを診断として返す。visit.rs:150はSpreadElementを除外済み、expressions.rs:95はSpreadElement/Elisionを除外済みで、この2件は現状安全。

### P1: CSSの未閉鎖ブロック

`crates/css/src/blocks.rs:55` の `css[open + 1..close]`。閉じ括弧がないとcloseがopenのままで逆転範囲になる。

公開 `parse_tailwind_theme` に `@utility x {`、`@custom-variant x {`、`@stillwind-variant x {` を渡した3例すべてでpanicを確認した。上流のCSSパーサー呼出だけではこのケースを防げない。

対応案: 閉じ位置をOptionで保持し、未閉鎖を構造化エラーとして返す。これは同ファイルのunwrapとは別の問題。

### P2: 公開Spanの算術

- `crates/source/src/text.rs:52`: `Span::new(FileId(1), 2, 1).len()` で減算overflow。
- `crates/grammar/src/lexer.rs:48`: `Spanned::new("x".into(), Span::new(FileId(1), u32::MAX, u32::MAX))` をlex_utilitiesに渡すと加算overflow。47行にも同種の加算がある。

上記はdebugビルドで再現。overflow checkを無効化したビルドでは同じ演算はwrapし、不正な位置になる。Spanの妥当性検証とchecked演算・Result化が候補。

## unwrap / expect / unreachable 全件

| 場所                                                           | 種類        | 到達条件・判定                                                                                                                                |
| -------------------------------------------------------------- | ----------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| crates/core/build.rs:8                                         | expect      | ビルド時のディレクトリ読取失敗。I/Oエラーで到達可能。                                                                                         |
| crates/core/build.rs:10                                        | expect      | ビルド時のディレクトリエントリ取得失敗。                                                                                                      |
| crates/core/build.rs:17                                        | expect      | ビルド時のソース読取失敗。                                                                                                                    |
| crates/source/src/text.rs:22                                   | expect      | usize が u32::MAX を超える場合。SourceFile::new は全体長を入口で検証していない。                                                              |
| crates/grammar/src/lexer.rs:39                                 | expect      | 巨大なユーティリティ文字列の開始位置が u32 範囲外。                                                                                           |
| crates/grammar/src/lexer.rs:40                                 | expect      | 巨大なユーティリティ文字列の終了位置が u32 範囲外。                                                                                           |
| crates/css/src/blocks.rs:100                                   | unwrap      | offset < len を確認し、char.len_utf8() またはASCII固定長で進むため、現状のループでは Some が保証される。                                      |
| crates/css/src/registry/utilities.rs:130                       | expect      | String への文字列書込。現在の書式引数では fmt::Error は発生しない。                                                                           |
| crates/css/src/theme/parser.rs:137                             | expect      | CSSの位置が u32 範囲外。入口のサイズ検証・エラー化候補。                                                                                      |
| crates/js-oxc/src/expressions.rs:246                           | unreachable | animation_use の戻り値が2種類に限定されるという関数間契約。専用戻り値enumで表現可能。                                                         |
| crates/js-oxc/src/refs/definitions.rs:42                       | expect      | 直前の is_some_and 条件内なので Some を保証。if let に整理可能。                                                                              |
| crates/js-oxc/src/refs/definitions.rs:108                      | expect      | 直前の is_some_and 条件内なので Some を保証。if let に整理可能。                                                                              |
| crates/js-oxc/src/refs/validation.rs:91                        | expect      | 同じイテレータで scope.is_some() をfilter済み。filter_map に整理可能。                                                                        |
| crates/js-oxc/src/visit.rs:181                                 | expect      | 直前に Ref/Scope 種別を明示して組み立てるため、現在の RefAlternative 検証条件を満たす。                                                       |
| crates/tailwind/src/utilities/formatting.rs:13                 | expect      | String への文字列書込。現在の引数では fmt::Error は発生しない。                                                                               |
| crates/core/src/compiler/handles.rs:41                         | unreachable | 抽出器による & 接頭辞保証に依存。ModuleAnalysis.ir は公開されており、手動IR構築時の防御にはならない。                                         |
| crates/core/src/compiler/pipeline.rs:504                       | unreachable | 上流の同じループで各対象variantを処理してcontinueする制御フローに依存。                                                                       |
| crates/core/src/compiler/stylesheet/authored_directives.rs:116 | unreachable | 呼出元で Compound を1段展開する契約。ネストした Compound について型自体の保証はない。                                                         |
| crates/core/src/compiler/stylesheet/properties.rs:15           | expect      | CSSが解析可能という契約。公開 render_stylesheet(&str) にも接続する。試した不正CSS4例はerror recovery等でpanicせず、実入力による到達は未確認。 |
| crates/core/src/compiler/symbols.rs:91                         | unreachable | kind文字列が4種類という内部契約。AssetKind enumへの統一候補。                                                                                 |
| crates/core/src/compiler/utilities/render.rs:65                | unreachable | 呼出元で Compound を1段展開する契約。ネストした Compound は型で排除されていない。                                                             |
| crates/core/src/loader/expansion.rs:54                         | expect      | 元ソース位置のu32変換。巨大ソースに依存するサイズ前提。                                                                                       |
| crates/core/src/loader/expansion.rs:55                         | expect      | 元ソース終了位置のu32変換。加算とSpan::lenも前提に含む。                                                                                      |
| crates/core/src/loader/state.rs:258                            | expect      | pendingが空であることを確認してからexpandする内部状態契約。                                                                                   |
| crates/core/src/module_compiler.rs:68                          | expect      | 直前にCurlyBracketBlockを取得し、コールバックはOkのみを返す。エラー処理がCSS全体の妥当性を検証しているわけではない。                          |
| crates/napi/src/api.rs:186                                     | expect      | 入力ソースの件数がu32::MAX超。非常に大きい入力に限るが件数チェックはない。                                                                    |
| crates/napi/src/api.rs:196                                     | expect      | theme用の追加FileIdがu32範囲外。入力件数を事前検証する候補。                                                                                  |

## その他のpanic候補

- Clippyはindexing/slicingを67診断、arithmetic_side_effectsを289診断出した。これらは危険候補数であり、バグ数ではない。特に文字列スライスはこのlintだけで全件網羅できず、上記blocks.rs:55はソース確認で発見した。
- AST添字アクセスは、確認した呼出箇所では引数数・候補数の条件で保護されている。ただし前述のto_expressionの型変換条件は別問題。
- pipeline.rsのbranches[0]は最初の要素を作ってから追加のみ。declaration_order.rsの添字は同じ配列から生成した位置に依存する。
- loader/state.rsの添字とexpectはnodes/graph/pendingの整合性に依存。check_cycles/expandには深さ制限がなく、極端に深いimportグラフによるスタック枯渇の候補もある（再現未実施）。スタック枯渇は通常の回復可能なpanicとは区別する。
- session.rs:73のrules[key]はcontributionsとrulesの整合性に依存し、replace/removeが両方を更新している。
- js-oxcのtype_probes.borrow_mut()は4箇所。現在は一文内のinsertで借用が終了し、同時借用を確認していない。
- transforms.rs:42のreplace_rangeはSourceFile::sliceによる範囲/UTF-8検証と降順の重複チェックを通るため、未検証の直接置換ではない。
- usize→u32のexpectは共通のサイズ契約として整理すべき。巨大入力を実際に確保する試験はしていない。

## 検証と限界

`target/panic-spread-repro.rs`、`target/panic-css-repro.rs`、`target/panic-audit-repro.rs` が今回の独立再現プログラム。既存debug rlibへrustcでリンクして実行した。製品のテストコードを棚卸し対象に含めてはいない。

`target/panic-clippy.jsonl` は機械診断、`target/panic-items.json` は抽出した候補一覧。依存クレート内部を全面監査したものではなく、到達が確認できたOxc呼出を追加した。別feature/別target、任意入力に対するpanic不在の証明、全算術演算の個別証明は今回の範囲外。OOM等をすべてpanicとして数えていない。
