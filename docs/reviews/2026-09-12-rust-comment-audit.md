# Rustコメント正確性監査（2026-09-12）

## 結果と範囲

監査開始時の変更済みRustファイル224件を対象に、追加コメントを実装・呼び出し元・既存テストと照合した。コメントは85ファイルで修正した。実装上の問題はコメントを正しい挙動に直したうえで、[実装レビュー](../implementation-review.md)のB1以降に分離した。実装の修正は今回行っていない。

今回の編集前に保存した各ファイルと編集後を比較し、コメント・空白を除いたRustトークン列に変更がないことを確認した。追加物は境界契約テスト11件、再現用Pythonスクリプト、この報告と修正差分・観測結果である。開始時点のユーザー変更にはimport整理、宣言位置の変更、テスト名変更、lint属性追加等も含まれており、HEADとの差分すべてを「コメントだけ」とは扱っていない。

[コメント修正の全差分](2026-09-12-rust-comment-corrections.md)は**今回の監査開始時点からの差分**であり、HEADとの差分ではない。途中の試行・撤回を含めず最終状態だけを掲載した。

## 主な訂正

| 領域                  | 誤っていた説明                                                       | 実装に合わせた説明・確認点                                                                                                                     |
| --------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| source / span         | `Spanned`の等価性は位置を無視する                                    | deriveされた等価性は値とSpanの両方を比較する。追加テストで確認                                                                                 |
| source / 座標         | 行末改行を指す座標は無効、UTF-16内部も扱える                         | 改行自体とEOFは指定可能。次行先頭を前行の列として指定することは不可。バイト変換はscalar境界を要求。LF/CRLF/絵文字で確認                        |
| projection            | 座標検索が文字列同一性まで検証し、重複区間は常に拒否される           | 座標検索とexact/fragment検証は別。対応が一致する重複区間は利用可能。v3の同一surrogate内部トークンは受理して編集anchorから除外                  |
| identity / cache      | `FileId`はキャッシュキーに一切入らない、ハッシュ一致は内容一致の証明 | プロセス内artifact keyには含まれる。永続identityとして単独使用すべきではない。衝突耐性は数学的同一性保証ではない                               |
| 公開データ型          | 型が常に検証済み状態を保証する                                       | 公開フィールド、tuple constructor、serde経由で検証を迂回できる。`RefAlternative::new`も内部candidateを再帰検証する契約ではない                 |
| semantic / summary    | 全フィールドがソース順、specifierは生の綴り                          | exportsは名前順、assetsは種類ごとの収集等がある。specifierはOxcがデコードした文字列値                                                          |
| frontend / checker    | integration diagnosticは常に非致命的                                 | frontendはfallbackできるがModuleCompilerはSHFACT001へ昇格し、出力を失敗扱いにする。古いfactsの追加テストで確認                                 |
| frontend / span       | asset identityはcall全体、Oxc spanは常に原本                         | 名前付きassetのidentityはbinding名の範囲。call_spanは置換対象。解析bufferはshadowの場合もあり、file idの付け替えだけでは原本座標に変換されない |
| syntax                | デコードした文字列内offsetを原本へそのまま戻せる                     | JS escapeで長さが異なり得る。lexerは与えられたSpanのendでデコード文字列長を制限するものではない                                                |
| theme                 | active tokenはソース順、通常のproject tokenは全件出る                | reset適用後に名前順。通常tokenは参照に応じて出力し、staticは別扱い。追加テストで確認                                                           |
| snapshot              | token fingerprintはactive token集合、builtins未指定はNone            | fingerprintはraw token列由来。builtins accessorは空registryでもSome。選択的無効化に進む際の課題はB7へ                                          |
| utility / fallback    | user定義が一致したらbuiltinには戻らない                              | user定義の解決失敗後はbuiltinを試す。追加テストで確認                                                                                          |
| utility / parser      | CSSの完全な構文検証や全placeholder排除を行う                         | custom-ident/string等は限定的な判定。Shadowは単一shadowの判定。placeholder拒否は既知名の集合に限定される                                       |
| utility / termination | placeholder置換は文字列を短くするので必ず停止                        | theme値が同じplaceholderを再導入すると停止しない。B1の実行再現あり                                                                             |
| variant / order       | 異なる記法でも一貫して著者順、最外wrapperがsortを決める              | user variantは記法別に収集。sortは該当registry位置等に依存し、最外wrapperだけでは決まらない。混在記法はB4で再現                                |
| class / emission      | クラスidentityは解決後CSSルールの内容                                | candidate・wrapper・assetのcanonical spelling等から生成。定義のCSS変更だけでは同じクラス名が残り得る                                           |
| motion / emission     | 未使用のanimationはCSSも生成されない                                 | 宣言から先にCSSを生成し、その後runtime bindingを除去する。B5で未使用keyframes出力を確認                                                        |
| rendering             | document rendering後はmodule_cssが消える                             | 成功時はmodule_cssを保持。失敗時はCSSとtransform等をクリア。追加テストで確認                                                                   |
| dependency / reload   | candidates/themes/assetsの依存はexact、reloadは選択的                | それぞれ構文上の参照・static/推移依存・宣言等を含む保守的集合。定義集合も全件登録。現在のreloadは全moduleを再コンパイル                        |
| N-API                 | transform mappingはbyte offset                                       | transform mappingはUTF-16、stylesheet mappingはbyte offset。絵文字を含むN-API入口のテストで確認                                                |
| N-API / reload        | 失敗時は常に同じ状態を維持する                                       | watchは旧状態保持、buildは成功moduleの集合を公開してからerrorを返す経路がある                                                                  |
| lint                  | 一度だけ集約、forbidなら生成を中止                                   | 分析後と生成後にfilter。該当optionはlintを追加してもcandidateの生成を続ける                                                                    |
| テストの説明          | JSを実行して検証、全fixtureがloader経由、差分を完全検出              | code inspectionと実行を区別。filesystemを使うhelperもあり、正規化比較は全text差分検出ではない                                                  |

CSSのresource URLは生の綴りではなくデコード値であること、同じowner/specifierのplaceholderは共有されることも追加テストで確認した。空のutilityは解決成功にはならず、rulesを返さないことも確認した。

## 確認方法と検証範囲

以下を実行した。最終的な件数と終了結果は[検証結果](2026-09-12-rust-comment-validation.md)に記録する。

```powershell
$env:CARGO_INCREMENTAL='0'
cargo fmt --all -- --check
cargo test --workspace
cargo test -p stillwind-design-system audits_builtin_registry_reachability_from_parity_fixtures -- --ignored --nocapture
cargo clippy --workspace --all-targets -- -D warnings
$env:RUSTDOCFLAGS='-D warnings'
cargo doc --workspace --no-deps
cargo test --manifest-path experiments/source-map-research/native/Cargo.toml
cargo fmt --manifest-path experiments/source-map-research/native/Cargo.toml -- --check
cargo build -p stillwind-cli
python tools/review-rust-comment-probes.py
```

追加テストは[compiler](../../crates/compiler/tests/comment_contracts.rs)の10件と[N-API](../../crates/napi/tests/comment_contracts.rs)の1件。確認対象は実際の公開入口とデータ契約であり、コメントの文字列に追従するテストではない。

問題の再現は[スクリプト](../../tools/review-rust-comment-probes.py)と[実行結果JSON](2026-09-12-rust-comment-probes.json)で独立して確認できる。再帰ケースは3秒で子プロセスをkill/waitする。これは停止不能ケースを通常のworkspaceテストに混ぜないための実行境界でもある。再現した不具合の出力を「正しい仕様」として固定するテストは追加していない。

この監査は現行実装との整合性を対象とする。すべてのCSS入力に対する意味的正しさ、ブラウザ実動作、外部checker・Vite・Nextとの新たな実環境受け入れを証明するものではない。コードから確認した懸念と実行再現できた問題は実装レビューで区別した。
