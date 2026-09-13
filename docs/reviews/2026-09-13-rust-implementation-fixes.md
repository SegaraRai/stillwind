# Rustコメント監査で判明した実装問題の修正

2026-09-12の[監査](2026-09-12-rust-comment-audit.md)と[改善メモ](../implementation-review.md)を受けて修正した。監査時点の観測JSONは保存し、修正後のCLI結果は[別ファイル](2026-09-13-rust-implementation-probes.json)に記録する。

## 対応内容

| 項目                           | 修正した挙動                                                                                                                                 | 主な箇所                                                                                                                                                    |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B1: 停止不能                   | 同じ展開経路上の再帰を検出し、深さ64・4096回の展開・生成テキスト1MiBで打ち切る。自己参照、相互参照、増幅入力を拒否。utilityとvariantで共通化 | [expansion.rs](../../crates/design-system/src/evaluation/values/expansion.rs)                                                                               |
| B2: calc文字列破壊             | CSS tokenで実際の関数を検索し、文字列、comment、URL、別identifierのsuffixを保持                                                              | [functions.rs](../../crates/css/src/functions.rs)、[folding.rs](../../crates/css/src/math/folding.rs)                                                       |
| B3: important文字列破壊        | top-level declaration境界でpriorityを付与し、既存priorityと末尾commentを保持。解決・結合・書式処理も共通境界を使用                           | [declarations.rs](../../crates/css/src/declarations.rs)、[declaration_order.rs](../../crates/compiler/src/compiler/utilities/declaration_order.rs)          |
| B4: variant上書き順            | shorthand・custom-variant block・variant blockを元offset順に統合。同一行やimportをまたいでも後の定義を優先                                   | [variants.rs](../../crates/design-system/src/registry/variants.rs)                                                                                          |
| B5: 不要motion CSS             | 静的参照・動的候補・exportから到達するanimation/view-transitionだけを公開CSSに残す。動的runtime値とは別集合で追跡                            | [pipeline.rs](../../crates/compiler/src/compiler/pipeline.rs)、[emission.rs](../../crates/compiler/src/compiler/emission.rs)                                |
| B6: 外部assetの偽Span          | 宣言のIdentityIdと使用箇所のSpanを分離。ローカル宣言だけをprojection時に変換し、外部宣言offsetにはローカルのfile idを付けない                | [model.rs](../../crates/ir/src/model.rs)、[spans.rs](../../crates/ir/src/spans.rs)、[ast.rs](../../crates/js-oxc/src/ast.rs)                                |
| B7: resetとfingerprintの不一致 | token別fingerprintをreset・override適用後のactive集合から作る                                                                                | [data.rs](../../crates/design-system/src/snapshot/data.rs)                                                                                                  |
| B8: 不変条件の未検証           | checkerのtag・外側kind・有限candidate・相関scopeを入口で検証。公開ルールのkeyもprojectへの登録時に現在のCSSから再計算                        | [symbolic.rs](../../crates/semantic/src/symbolic.rs)、[analyzer.rs](../../crates/js-oxc/src/analyzer.rs)、[session.rs](../../crates/project/src/session.rs) |
| B9: デコードoffset             | デコードでbyte長が変わる文字列では、誤った部分Spanを作らずliteral全体のprovenanceを返す                                                      | [lexer.rs](../../crates/syntax/src/lexer.rs)                                                                                                                |
| B10: 構文境界                  | 実際に破壊を生んでいた関数検索・priority付与・宣言結合の境界を修正。未知のCSS関数は保持する方針を維持                                        | B2/B3の共通CSS処理                                                                                                                                          |

`calc()`はutilityの最終書式処理のほか、compilerの宣言結合とrule renderingでも処理される。最初の入口だけではdata URL内部のセミコロンで分断された断片が後段で書き換わったため、宣言結合まで修正した。`,var(`の空白挿入もtoken検索に合わせ、文字列やURLの内容には適用しない。

## 維持する契約と互換性

- 再帰展開・上限超過は解決不能なoverloadとして扱い、他のoverload/builtinへのfallbackを維持する。候補全体が解決不能なら既存の`unknown-utility` lintをソース位置付きで返す。既定のCLI終了コードを新たにerrorへ変更してはいない。展開対象のない大きな文字列・data URLは生成量上限で拒否しない。
- 未使用motionも意味検証を行い、invalid utility等の診断は残す。CSSは到達可能なものだけを先頭へまとめるため、既存のkeyframes→utilityの順序を維持する。静的利用ではCSSを残し、不要なruntime producerは引き続き除去する。exportの候補がUniverseなら、そのkindの所有assetを保守的に残す。
- Rust IRの`AssetReference`は`identity`、使用箇所の`span`、`local_declaration`へ変更した。この公開Rust型を直接構築・直列化する利用者は新しい形へ合わせる必要がある。N-APIのchecker fact/asset identity payloadの形は変更していない。
- デコード用offset mapを新設したわけではない。escapeを含むliteralの診断は安全な範囲になるが、個別tokenより広くなる場合がある。
- 現行`ProjectSession::reload`は全module再コンパイルのまま。定義追加・探索失敗・namespace resetまで含む依存追跡を揃えずに、選択的reloadへ切り替えてはいない。
- 公開構造体を一律にprivate化してはいない。外部checker値の入口検証と、CSS登録時の再キー化によって実際の消費境界を保護する。
- 任意の未知CSS関数を禁止する変更は行っていない。`--unknown-placeholder()`は従来どおり保持する。全CSS helperを完全な構文parserへ置き換える作業や、既存のA1〜A7の独立した改善候補は今回の不具合修正に含めない。

## 回帰検証

[compilerの追加テスト](../../crates/compiler/tests/audit_regressions.rs)は、停止不能、文字列/data URLの保持、variantの記法混在、未使用・export・静的・動的motion、active token fingerprint、escape診断、外部identityとローカルprojectionを公開APIから検証する。

CSSの単体テストでは文字列・comment・URL・既存importantを、展開のテストでは循環・増幅・独立した同一subexpressionを確認する。loaderテストではimport後のvariant上書きを、checkerテストではfreshなrevisionでも内部kindが不正なら拒否することを、projectテストでは古いkeyを持つ異なるCSSが誤って共有されないことを確認する。

```powershell
$env:CARGO_INCREMENTAL='0'
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
$env:RUSTDOCFLAGS='-D warnings'
cargo doc --workspace --no-deps
cargo test -p stillwind-design-system audits_builtin_registry_reachability_from_parity_fixtures -- --ignored --nocapture
cargo build -p stillwind-cli
python tools/review-rust-comment-probes.py --check
```

最終結果の件数・追加確認は[検証結果](2026-09-13-rust-implementation-validation.md)を参照。
