# Tailwind比較の推奨修正結果

修正前の作業を `d88fa7b` にコミットしてから実施した。既存の比較対象13,428件は不一致0件。内訳はCSS出力あり12,923件、空出力505件。既存の対象外3ケース（候補数0、0、2）は維持しており、Tailwind全機能の対応を意味しない。

## 実装した修正

- 任意値drop-shadowを共通filter合成経路に乗せ、色上書きと他filterとの合成を保持した。
- transform-cpu/gpuによる不要な変数登録を抑えた。bare transformの登録は定義側の明示依存として保持した。ring-offsetの登録も依存関係で選択する。
- builtin maskの登録済み初期値と等しいfallbackを定義側で省略した。任意のユーザーCSSへのfallback削除処理は追加していない。
- theme変数、keyframes、property登録、fallback初期化を `@stillwind-slot` で配置する。support条件はCSS側に移した。生成済み最終CSSをminifyして配置を修復する経路は追加していない。
- 既定keyframesはthemeレイヤー内に保持する。同名keyframesの優先順位はTailwind既定配置と異なるため、意図した契約差としてテストする。条件内のkeyframesを条件外へ持ち上げない。

比較器が許容する差と拒否する差、各入力adapterの制約は[比較契約](../tailwind-comparison-contract.md)および `tools/lib/tailwind-parity-contract.ts` のコメントに記載した。旧「追加property登録を許す」例外は削除した。単に不一致を除外する変更ではなく、layer本体・条件・fallback・登録・干渉する宣言順序の不一致を拒否する境界テストを追加した。

## 検証

- `vp run --filter @stillwind/tools tailwind:parity:analyze`: 13,428/13,428。property-orderは12,402候補・38ケースから545制約を検証。
- `vp run --filter @stillwind/tools tailwind:inventory`: 13,428件、不一致0。73回の直接コンパイル照合。新しい実行結果は `target/tailwind-inventory` に出力し、修正前の棚卸し資料を上書きしない。
- 比較契約テスト6件、Edgeブラウザー検証10件通過。filterはCanvasの描画比較（8-bit色変換の丸め差として各channel最大1を許容）、その他はcomputed valueを比較。同名keyframes競合は意図した差を確認する。
- Rust workspaceのlib/integration tests、doc tests、Clippy全targets、警告をエラー扱いしたrustdocが通過。
- native・host・Viteテスト、6プロジェクトのfixture再生成と整合確認が通過。

Rustテストは `cargo test --workspace --lib --tests --no-fail-fast` と `cargo test --workspace --doc` に分けた。引数なしのworkspace testは既存の同名example `projection_probe` の出力先競合があるため、この実行ではexampleをテスト対象に含めない。Clippyは全targetsを対象にした。

比較用正規化は一般のCSS意味同値性証明ではない。minify/prettifyは引き続きバンドラー側の責務であり、ブラウザー検証も上記の具体的な合成・継承・配置ケースを保証する。
