# 実装修正の検証結果（2026-09-13）

| 検証                                                                  | 結果                                  |
| --------------------------------------------------------------------- | ------------------------------------- |
| `cargo test --workspace`                                              | 成功: 456 passed、0 failed、1 ignored |
| 最後の公開ルールkey修正後: `cargo test -p stillwind-project`          | 成功: 14 passed                       |
| 最後のexport Universe保持後: `cargo test -p stillwind-compiler --lib` | 成功: 43 passed                       |
| `cargo clippy --workspace --all-targets -- -D warnings`               | 成功                                  |
| `RUSTDOCFLAGS=-D warnings`で`cargo doc --workspace --no-deps`         | 成功                                  |
| `cargo fmt --all -- --check` / `git diff --check`                     | 成功                                  |
| 通常ignoredのregistry監査を明示実行                                   | 成功: 1 passed                        |
| 最終CLIをbuildして`review-rust-comment-probes.py --check`             | 7ケースとも成功、timeoutなし          |

workspace全体の実行後に追加したproject公開境界の再キー化とexport Universeの保持は、それぞれ該当crateのテストと全workspaceのClippy/rustdocで再検証した。テスト件数を重複加算した合計は表示していない。

registry監査は13,421 candidatesを処理し、scope外4件、theme/candidate parse失敗0件。1,292 utility定義がmatch/resolveし、そのうち1,256が出力差へ寄与、91/91 variantが寄与した。初回監査と同じ集計であり、既存36件の非寄与utilityを今回削除したわけではない。

## CLIによる修正前後

| ケース                      | 監査時点                         | 修正後                                                      |
| --------------------------- | -------------------------------- | ----------------------------------------------------------- |
| 自己参照placeholder         | 3秒でtimeout                     | 正常に復帰しCSSを出さず、ソース位置付きunknown-utility lint |
| quoted calc                 | `"calc(1 + 1)"`が`"2"`へ変化     | 元の文字列を保持                                            |
| quoted semicolonとimportant | `"a;b"`が`"a!important;b"`へ変化 | `"a;b"!important`                                           |
| variant混在                 | 後のfocusより前のhoverを優先     | 後のfocusを優先                                             |
| 未使用animation             | runtime除去後もkeyframesが残る   | CSSなし                                                     |
| 未知CSS関数                 | 関数を保持                       | 保持を継続                                                  |
| 対照の通常theme展開         | redを出力                        | redを出力                                                   |

[実行結果JSON](2026-09-13-rust-implementation-probes.json)と[再実行スクリプト](../../tools/review-rust-comment-probes.py)を参照。通常のcompiler回帰テストはこれらに加えてdata URL、projection、外部identity、export、動的候補、decode診断、reset fingerprintを確認する。

これはRust APIおよび実CLIの検証であり、ブラウザや外部framework hostを新たに起動しての受け入れ検証ではない。
