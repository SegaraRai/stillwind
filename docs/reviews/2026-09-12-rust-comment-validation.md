# 検証結果（2026-09-12）

すべてローカルの現行作業ツリーで実施した。テスト件数はCargoの各`test result`行を合計し、doc-testも含む。追加11件はworkspace合計に含まれる。

| 確認                                                                  | 結果                                                |
| --------------------------------------------------------------------- | --------------------------------------------------- |
| `cargo test --workspace`                                              | exit 0、440 passed、0 failed、1 ignored（39集計行） |
| 追加`comment_contracts`の個別実行                                     | exit 0、11 passed                                   |
| `cargo clippy --workspace --all-targets -- -D warnings`               | exit 0                                              |
| `RUSTDOCFLAGS=-D warnings`で`cargo doc --workspace --no-deps`         | exit 0                                              |
| `cargo fmt --all -- --check`                                          | exit 0                                              |
| source-map実験crateの`cargo test`                                     | exit 0、2 passed、0 failed                          |
| source-map実験crateの`cargo fmt -- --check`                           | exit 0                                              |
| 通常ignoredのregistry reachability監査を`--ignored --nocapture`で実行 | exit 0、1 passed、73.35秒                           |
| `cargo build -p stillwind-cli`                                        | exit 0                                              |
| `git diff --check`                                                    | exit 0                                              |
| 既存Rustファイルのコメント・空白を除いたトークン列                    | 今回の監査編集による変更なし                        |

変更済みRustの棚卸しは224ファイル、監査でコメントを修正したファイルは85件。新規テスト2ファイルは別に追加した。

通常ignoredの監査も次のコマンドで別途実行し成功した。workspace合計の「1 ignored」は通常実行時の表示であり、この監査を未検証のまま残したわけではない。

```powershell
cargo test -p stillwind-design-system audits_builtin_registry_reachability_from_parity_fixtures -- --ignored --nocapture
```

この監査は13,421 candidatesを処理し、4件をscope外として除外した。theme/candidate parse失敗は0、全1,292 utility定義がmatch・resolveし、91/91 variant定義が出力へ寄与した。utilityは1,256/1,292が寄与し、残り36件はresolveしても除去前後の出力差が出なかった。この36件を失敗扱いにするassertionは現行テストにないため、「全utilityが実際の出力に寄与した」とは解釈しない。

## 実装問題の実行再現

最終CLIビルド後に`python tools/review-rust-comment-probes.py`を実行し、[観測JSON](2026-09-12-rust-comment-probes.json)へ保存した。

- control: 正常終了しredを出力。
- recursive-placeholder: 3秒でtimeoutし、子プロセスを終了・回収。ソース上も同一文字列に戻るループを確認。
- quoted-calc: `"calc(1 + 1)"`が`"2"`になる。
- important-string-semicolon: `"a;b"`が`"a!important;b"`になる。
- mixed-variant-order: 後のfocusではなく前のhoverを出力。
- unused-motion-css: runtime bindingはundefinedになってもkeyframesが残る。
- unknown-placeholder: 未知関数をそのまま出力。これは限定的な拒否方針の観測であり、それ自体を不具合とは断定しない。

以上は正しい出力を期待する受け入れテストではなく、現行実装の問題・限界を観測する再現例。通常のテストが通ることと、これらの問題が解決したことは別である。
