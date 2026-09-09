# Rustユニットテストの対象棚卸し

## 判定基準

手書きの `crates/*/src/**/*.rs` を対象に、呼び出し準備が概ね3文以内で、
短い文字列・数値・小さな構造体やコレクションを扱い、検証・変換・探索・計算を行う関数を選んだ。
構造体リテラルの整形上の改行は準備の手数に数えない。
大きなASTやコンパイラ全体の構築をヘルパーに隠して条件を満たす扱いにはしない。
単なる射影、委譲、データ格納、deriveによる実装は追加対象から除いた。

以下は対象の選別と検証内容の記録であり、分岐カバレッジ100%の主張ではない。
小さな内部関数は、同じファイルの上位関数のテストを通じて検証する場合もある。
既存の十分な同居テストは維持し、同じケースを追加して数を増やさない。

## 追加・補強した対象

| クレート／実装ファイル群                                                          | 主な関数・検証内容                                                                                                                     |
| --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| source `text.rs`                                                                  | `SourceFile::slice/position/utf16_offset`、パス正規化、`Span::len/is_empty`、`range_of`。誤ったFileId、UTF-8境界、絵文字、改行、空範囲 |
| source `lint.rs`                                                                  | `collect_lint_directives/filter_lint_findings`。引用符内のコメント、理由の除外、ルール選択、disable/enable、同一行・次行の適用範囲     |
| grammar `syntax.rs`, `lexer.rs`                                                   | 候補の保持、区切りの入れ子、引用符、エスケープ、不正な閉じ括弧・未閉鎖文字列、字句と元ソースのspan対応                                 |
| semantic `symbolic.rs`                                                            | `SymbolicValue::finite` の重複除去・種類の拒否、`RefAlternative::new` のref/scope制約                                                  |
| css `blocks.rs`, `functions.rs`, `directives.rs`                                  | 名前付きブロック、delimiter、トップレベルの分割、コメント除去、関数参照、フレームワークdirective除去                                   |
| css `colors.rs`, `math/*`                                                         | 色リストとalpha、文字列判定、数値構文木、演算優先順位、単位、符号、無効な式の保持、ゼロの書式                                          |
| css `values/*`                                                                    | CSS値型・プロパティ文法・例外、未知のhint、型互換性、var/gradient/content、Webrefの検索                                                |
| css `theme/*`                                                                     | 宣言・モード・名前空間、設定、import、参照・fallback、トークン上書きとreset、依存循環、keyframes抽出                                   |
| css `registry/*`, `custom_properties/*`, `builtins.rs`                            | 並び順、utility family展開、型付きvariant、書式規則、custom property参照・登録・activation、keyframesの名前選択                        |
| tailwind `syntax.rs`, `values/*`                                                  | delimiter、arbitrary値・変数・alpha、分数、型domain、value map、placeholder、色の互換出力                                              |
| tailwind `utilities/*`                                                            | 宣言分割、overload選択と併合、selector/media/supports、fallback、important、数値sort key、animation用プロパティ変換                    |
| tailwind `variants/*`                                                             | variant body、modifier、引用されたcapture、属性selector、数値と単位の比較、nested条件の付与                                            |
| tailwind `builtins/normalization.rs`, `registry.rs`                               | builtin本体の空白正規化、arbitrary propertyの完全一致・wildcard・未知の名前の優先順位                                                  |
| js-oxc `refs/definitions.rs`, `refs/validation.rs`, `motion/animation.rs`         | `is_marker_label`, `is_strict_ancestor`, `is_animation_frame`。不正なラベル、同一要素、祖先の方向、percent/keywordの境界               |
| core `fingerprint.rs`, `transforms.rs`, `module_compiler.rs`, `session.rs`        | 既知のBLAKE3値、ソース置換と周囲の保持、CSS artifact往復、共有ルールの参照数と削除                                                     |
| core `loader/imports.rs`, `loader/expansion.rs`                                   | import抽出・修飾子・エラー、合成ラッパーと元ソースの位置対応                                                                           |
| core `compiler/options.rs`, `symbols.rs`, `handles.rs`                            | prefix入替、identityと生成名、種類・spanの区別、projectionとscoped selector                                                            |
| core `compiler/stylesheet/*`, `compiler/utilities/*`, `design_system/registry.rs` | CSS区切り・keyframes、補助宣言とnested宣言の位置、診断の種類、定義のグループ化・順序・fingerprint                                      |
| napi `api.rs`                                                                     | `parse_options` のデフォルト、JSON構文エラーとオプション検証エラーの区別                                                               |

プロパティベーステストでは、UTF-8文字境界とUTF-16の整合、パス・fallback・数式の正規化の冪等性、
候補文字列と字句spanの保持、括弧の対応、整数演算の優先順位、分数の分子分母の保持、
数値sort比較の反対称性、ソース置換前後の非対象部分の保持、CSS artifactの往復、
symbolic値の順序・重複への不変性、spanとrangeの長さの整合を検証する。
生成範囲は小さく限定し、数値演算のオーバーフローや巨大な入力で別の契約を検査しない。

## 追加対象から除いた関数・構造体

「除外」は今回の小粒ユニットテスト追加からの除外であり、既存テストの削除を意味しない。
同一ファイル内の小さな補助関数をテストした場合も、以下の大きな入口は別扱いとする。

| 関数・構造体                                                                                                                                  | 除外理由／既存検証の位置付け                                                                                      |
| --------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `FileId`, `BytePos`, `Span::new`, `Spanned::new`, `SourceFile::id/path/text/full_span`, `offset`                                              | 包装、射影、単純な整数変換。`Span::len/is_empty` と実際の位置変換は対象に含めた                                   |
| `Diagnostic::new`, `LintFinding::new`, `ModuleIr::react`, `ModuleSummary::new`, データ専用のIR・TypeFact・エラー型                            | 格納・初期化・deriveが主体。エラーの識別はエラーを返す実装側で確認                                                |
| `SymbolicValue::exact/universe/finite_candidates`, `ExportedValue::kind`                                                                      | 包装・射影。有限集合の検証・正規化は `finite` で検証                                                              |
| CLI `argument/optional_argument/has_flag`, `read_source/load_design_system`, `run/main`                                                       | プロセス引数・ファイルI/O・実行環境が必要。既存CLI E2Eを維持                                                      |
| `OxcSourceAnalyzer::scan/finalize`, `ModuleScan`, `ModuleAnalysis`, `TypeScriptFactsProvider::update`                                         | AST・IR・型解析結果全体の入出力と複数段階の準備。既存analysisテストで検証                                         |
| js-oxc `parse/parse_with_type_facts`, `MacroExtractor` と `Visit` 実装                                                                        | Oxc arena、AST、binding、scope、型情報を横断する解析。`ParsedJavaScript` も大きな出力。既存抽出・連携テストを維持 |
| js-oxc `ast`, `bindings`, `state`, `refs/references`, `refs/projections`, `motion/view_transition` のASTを受けるメソッド                      | ASTノード・scoping・extractor状態の準備が必要。文字列や位置だけで判定する補助関数は別途対象に含めた               |
| js-oxc `module_export_name`, `lint_rule_for_code`                                                                                             | 名前または定数への単純な射影・対応表                                                                              |
| `DesignSystemLoader::provide/provide_many` と状態遷移、`CssSourceGraph`                                                                       | 複数ファイルの要求・供給とsnapshot生成を横断。既存loaderテストが未完了供給、ID枯渇等を検証                        |
| `DesignSystemSnapshot::from_source/from_loaded_theme` と各getter                                                                              | 前者は全registry構築、後者は射影。`generate_react_types/render_stylesheet` は全体の生成処理                       |
| `DesignSystemQuery::{resolve_candidate,complete_candidate,hover_candidate,diagnose_candidate}`                                                | snapshotとregistryを横断する検索・解決。既存design-systemテストを維持                                             |
| `ModuleCompiler::{key_for,compile}`, `CompiledModule::to_legacy`, `ProjectSession::{update_module,remove_module,reload,css}`                  | モジュール解析、依存関係、snapshot、キャッシュを横断。既存sessionテストを維持                                     |
| `ModuleCompiler::new/design_system`, `ProjectSession` のgetter、`CandidateResolver::resolve`, `CompileResult/CompiledModule::apply_to_source` | 初期化・射影・他実装への委譲。置換本体 `apply_transforms` は対象                                                  |
| core `compiler/api.rs`, `compile_with_snapshot`                                                                                               | 解析からCSS・React出力までの全体入口。既存compiler/fixtureテストを維持                                            |
| `render_react_props`, `render_react_types`, `render_stylesheet`, `render_used_properties`, `render_utility_rule/render_utility_rule_path`     | 複数の定義、解決済みutility、theme、生成コンテキストを扱う大きな入出力                                            |
| `emit_animation_definition`, `emit_view_transition_definition`, `resolve_animation_use`                                                       | IRの定義・参照と解決コンテキスト全体が必要。単純なanimation option変換は対象                                      |
| CSS `parse_theme/parse_tailwind_theme`, `tailwind_v4_preflight/tailwind_v4_keyframes`                                                         | theme全体やvendored CSS全体を返す。個別の宣言・参照・名前付きkeyframeの選択は対象                                 |
| Tailwind `builtin_definitions/builtin_utilities/builtin_variants/builtin_animation_options`, `builtins/metadata.rs` の取得関数                | 全builtin registryの初期化・cacheまたは大きなコレクションの取得。正規化とpriorityの計算は対象                     |
| Tailwind `resolve_utility`, variant解決全体                                                                                                   | theme/registryと複数の宣言・selector・nested ruleを横断。既存 `tests/*` の機能連携テストを維持                    |
| NAPI `compile_react*`, `summarize_react*`, `render_react_stylesheet`, `generate_react_types`, `binding_output`, loader/sessionの公開メソッド  | binding変換とRustコア全体の連携。既存bindingテストを維持。`fold_calc_expressions` はCSS実装への委譲               |
| 自動生成のWebref資産、Rust標準ライブラリ・依存クレート内部                                                                                    | 手書き実装の対象外。利用側の受理／拒否・境界値を検証                                                              |

## 配置と回帰修正

追加テストは実装ファイル内の `#[cfg(test)]` モジュールに配置した。
CSSの `tests/directives.rs`, `tests/properties.rs`, `tests/values.rs` から単独機能の14ケースを実装側へ移し、
元の重複ケースを削除した。複数機能を横断するケースはそのまま残した。
テスト込みの手書きRustファイルは900行以内を確認する。

境界ケースで見つかった実装の修正は以下の3件。

- 未閉鎖CSSコメントで、コメント前の文字列が二重に出力されていた。元の入力を保持する。
- 候補内の引用符付き文字列の括弧を構造delimiterと誤認していた。引用符・エスケープを区別する。
- builtin keyframesの選択が前方一致になっていた。`sp` で `spin` が選択されないよう完全な名前を照合する。

## 検証結果

`cargo test --workspace`: 318件成功、0件失敗、既存の1件ignored。
テストケースの純増は100件（移動した14件を除く）。
手書き `src` 内Rustファイルの最大は717行で、全ファイル900行以内。

## 再検証

同一のtargetディレクトリに対するCargoの実行は直列に行う。

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 除外対象の追加検証

以下の経路も追加検証した。Rustのテストは対応する実装ファイル内に置き、
Nodeバインディングの契約は `packages/native/test/loader.test.mjs` から公開APIを呼び出す。

- `tailwind_v4_keyframes`: 全4定義の順序、入れ子の本体、閉じ括弧、theme宣言の除外、名前指定版との整合。
- `ProjectSession::reload`: Watchで一部モジュールの再コンパイルが失敗した際、旧snapshot・CSS・全artifact・依存情報を維持。Buildでロード失敗した際はartifactと全依存indexを消去。双方とも後続の正常reloadで復旧。
- `DesignSystemQuery::complete_candidate`: theme token、breakpoint、variant、variant付きprefix、未知のvariant、重複のない整列、revisionの一致。
- motion生成: アニメーション／view transition定義にvariantを記載すると、それぞれ `SHANIMATION003` / `SHMOTION001` と正確なソースspanを返す。
- `NativeProjectSession::reload`: `ReloadCompilation` のエラーをNodeに返し、以前のCSSを維持し、正常reloadで復旧。
- `ProvidedTypeScriptFacts::update` → `NativeProjectSession::compile`: consumerをproducerより先に渡し、非空の型情報と別モジュールのassetラベルを統合。型情報なしの失敗から再compileで回復し、変換コードにもラベルが残ることを確認。

追加後の検証: Rust 323件成功・既存1件ignored、Node 5件成功。Rust fmt・Clippy、`vp run --filter @stillwind/native check`（ネイティブ再ビルド・型チェック・Nodeテスト）も成功。実装の変更はなく、テスト込み最大717行。
